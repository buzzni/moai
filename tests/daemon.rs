//! `moai daemon` 을 실제 바이너리로 돌린다(moai-5n6m.x8t).
//!
//! 시험마다 제 `MOAI_CTL_DIR` 을 `/tmp` 아래 짧은 자리에 세운다 — 소켓 경로에는 OS 상한(108)이 있고, 사람의
//! 데몬(`$XDG_RUNTIME_DIR/moai`)에 닿으면 안 된다. 띄운 데몬은 [`Daemon`] 이 떨어질 때 죽인다 — 단언이 넘어져도
//! 프로세스가 남지 않게.
#![cfg(unix)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_moai");

struct Fence(PathBuf);

impl Fence {
    fn new() -> Fence {
        use std::os::unix::fs::PermissionsExt;
        static NTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(format!("/tmp/moai-daemon-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::create_dir(path.join("home")).unwrap();
        Fence(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("daemon.socket")
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("MOAI_CONFIG", self.0.join("no-config.toml"))
            .env("MOAI_CTL_DIR", &self.0)
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("MOAI_ACTOR")
            .env("MOAI_LANG", "en")
            .env("MOAI_NO_UPDATE_CHECK", "1")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null());
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }

    /// 0 으로 끝나야 하는 `--json` 명령 — stdout 의 값 하나.
    fn json(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!("moai {args:?} 가 JSON 하나를 안 냈다: {e}\n{}", String::from_utf8_lossy(&out.stdout))
        })
    }

    /// 넘어져야 하는 `--json` 명령 — stderr 의 오류 객체.
    fn refused(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(!out.status.success(), "moai {args:?} 가 넘어지지 않았다: {}", String::from_utf8_lossy(&out.stdout));
        serde_json::from_slice(&out.stderr).unwrap_or_else(|e| {
            panic!("moai {args:?} 의 stderr 가 오류 객체가 아니다: {e}\n{}", String::from_utf8_lossy(&out.stderr))
        })
    }

    /// `daemon start --json` 을 띄우고 첫 줄(묶은 뒤에 나온다)을 기다린다.
    fn start(&self) -> (Daemon, Value) {
        let mut child =
            self.cmd(&["daemon", "start", "--json"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line);
        });
        let daemon = Daemon(child);
        let line = rx.recv_timeout(Duration::from_secs(20)).expect("데몬이 첫 줄을 안 냈다");
        let first: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("첫 줄이 JSON 이 아니다: {e} — {line:?}"));
        (daemon, first)
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 띄운 데몬 — 떨어질 때 아직 돌면 죽인다.
struct Daemon(Child);

impl Daemon {
    /// 멈춘 뒤 끝나기를 기다린다 — 종료 코드를 본다.
    fn wait(mut self) -> std::process::ExitStatus {
        for _ in 0..200 {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("데몬이 10초 안에 끝나지 않았다");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// 소켓에 붙은 연결 하나 — 줄을 보내고 줄을 받는다.
struct Wire {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Wire {
    fn open(path: &Path) -> Wire {
        let stream = UnixStream::connect(path).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Wire { reader: BufReader::new(stream.try_clone().unwrap()), writer: stream }
    }

    fn send_raw(&mut self, bytes: &[u8]) -> Value {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("답이 JSON 이 아니다: {e} — {line:?}"))
    }

    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.send_raw(format!("{line}\n").as_bytes())
    }
}

#[test]
fn start_status_hello_ping_and_stop() {
    let f = Fence::new();
    let before = f.json(&["daemon", "status", "--json"]);
    assert_eq!(before["running"], false, "{before}");
    assert_eq!(before["supported"], true, "{before}");
    assert!(before.get("pid").is_none(), "떠 있지 않은데 pid 가 섰다 — {before}");

    let (daemon, first) = f.start();
    assert_eq!(first["started"], true, "{first}");
    assert_eq!(first["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(first["socket"], f.socket().display().to_string());
    let pid = first["pid"].as_u64().unwrap();

    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true, "{status}");
    assert_eq!(status["pid"], pid, "{status}");
    assert_eq!(status["same_version"], true, "{status}");
    assert_eq!(status["proto"], 1, "{status}");

    let mut wire = Wire::open(&f.socket());
    let hello = wire.call(1, "hello", json!({ "proto": 1 }));
    assert_eq!(hello["result"]["version"], env!("CARGO_PKG_VERSION"), "{hello}");
    assert_eq!(hello["result"]["pid"], pid, "{hello}");
    // 같은 연결로 다음 요청을 받는다.
    let ping = wire.call(2, "ping", json!({}));
    assert_eq!(ping["result"], json!({}), "{ping}");
    assert_eq!(ping["id"], 2);

    let stopped = f.json(&["daemon", "stop", "--json"]);
    assert_eq!(stopped["stopped"], true, "{stopped}");
    assert_eq!(stopped["pid"], pid, "{stopped}");
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");
    assert!(!f.socket().exists(), "멈춘 데몬이 소켓을 남겼다");
    let after = f.json(&["daemon", "status", "--json"]);
    assert_eq!(after["running"], false, "{after}");
    assert_eq!(after["stale_socket"], false, "{after}");
    // 떠 있지 않을 때의 stop 도 0 이다.
    let again = f.json(&["daemon", "stop", "--json"]);
    assert_eq!(again["stopped"], false, "{again}");
}

#[test]
fn a_second_start_does_not_double_up() {
    let f = Fence::new();
    let (daemon, first) = f.start();
    let pid = first["pid"].clone();
    let second = f.json(&["daemon", "start", "--json"]);
    assert_eq!(second["started"], false, "둘째 start 가 새로 떴다고 한다 — {second}");
    assert_eq!(second["pid"], pid, "{second}");
    let text = f.run(&["daemon", "start"]);
    assert!(text.status.success());
    assert!(String::from_utf8_lossy(&text.stdout).contains("already running"), "{text:?}");
    assert_eq!(f.json(&["daemon", "status", "--json"])["pid"], pid, "떠 있는 데몬이 바뀌었다");
    f.json(&["daemon", "stop", "--json"]);
    assert!(daemon.wait().success());
}

#[test]
fn a_stale_socket_is_cleared_and_the_daemon_starts() {
    let f = Fence::new();
    // 묶었다가 닫은 리스너 — 파일은 남고 아무도 안 받는다. 죽은 데몬이 남긴 꼴이다.
    drop(UnixListener::bind(f.socket()).unwrap());
    assert!(f.socket().exists());
    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], false, "{status}");
    assert_eq!(status["stale_socket"], true, "{status}");
    assert!(f.socket().exists(), "status 가 남은 소켓을 지웠다 — 읽기는 쓰지 않는다");

    let (daemon, first) = f.start();
    assert_eq!(first["started"], true, "{first}");
    assert_eq!(f.json(&["daemon", "status", "--json"])["running"], true);
    f.json(&["daemon", "stop", "--json"]);
    assert!(daemon.wait().success());
}

#[test]
fn an_unknown_method_and_an_over_long_line_are_refused_over_the_wire() {
    let f = Fence::new();
    let (daemon, _) = f.start();
    let mut wire = Wire::open(&f.socket());
    let unknown = wire.call(1, "no.such.method", json!({}));
    assert_eq!(unknown["error"]["code"], -32601, "{unknown}");
    assert_eq!(unknown["id"], 1);

    let mut long = vec![b'x'; 1024 * 1024 + 1];
    long.push(b'\n');
    let refused = wire.send_raw(&long);
    assert_eq!(refused["error"]["code"], -32600, "{refused}");
    assert_eq!(refused["id"], Value::Null);
    // 그 연결은 그대로 산다.
    let ping = wire.call(2, "ping", json!({}));
    assert_eq!(ping["result"], json!({}), "{ping}");

    f.json(&["daemon", "stop", "--json"]);
    assert!(daemon.wait().success());
}

#[test]
fn a_daemon_of_another_version_is_named_and_left_running() {
    let f = Fence::new();
    // 옛 판처럼 답하는 가짜 데몬.
    let listener = UnixListener::bind(f.socket()).unwrap();
    let fake = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut line = String::new();
            if BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).is_err() {
                continue;
            }
            if line.contains("\"quit\"") {
                return;
            }
            let reply = json!({ "jsonrpc": "2.0", "id": 1, "result": { "proto": 1, "version": "0.0.1", "pid": 4242 } });
            let _ = writeln!(stream, "{reply}");
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
        }
    });
    let error = f.refused(&["daemon", "start", "--json"]);
    assert_eq!(error["code"], "version_mismatch", "{error}");
    let said = error["error"].as_str().or(error["message"].as_str()).unwrap_or_default().to_string();
    let text = error.to_string();
    assert!(text.contains("0.0.1") && text.contains(env!("CARGO_PKG_VERSION")), "두 판을 다 안 댄다 — {said} {text}");
    assert!(text.contains("moai daemon stop"), "다시 띄우는 길을 안 댄다 — {text}");
    assert!(f.socket().exists(), "다른 판의 소켓을 지웠다");

    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true, "{status}");
    assert_eq!(status["same_version"], false, "{status}");
    assert_eq!(status["version"], "0.0.1", "{status}");
    assert_eq!(status["pid"], 4242, "{status}");

    let mut quit = UnixStream::connect(f.socket()).unwrap();
    writeln!(quit, "{}", json!({ "quit": true })).unwrap();
    fake.join().unwrap();
}

#[test]
fn a_file_that_is_not_a_socket_is_left_alone() {
    let f = Fence::new();
    std::fs::write(f.socket(), "not a socket").unwrap();
    let error = f.refused(&["daemon", "start", "--json"]);
    assert_eq!(error["code"], "already_exists", "{error}");
    assert_eq!(std::fs::read_to_string(f.socket()).unwrap(), "not a socket", "남의 파일을 건드렸다");
}
