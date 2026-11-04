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
            .env("MOAI_CONFIG", self.0.join("config.toml"))
            .env("MOAI_CTL_DIR", &self.0)
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("MOAI_ACTOR")
            .env("MOAI_LANG", "en")
            .env("MOAI_NOW", "2026-11-04T12:00:00Z")
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

/// 연결을 새는 클라이언트가 상한(32)을 다 쥐어도 `status` 가 pid 를 대고 `stop` 이 데몬을 멈춘다(moai-uezh). 상한을
/// 넘은 연결에서는 `hello`·`daemon.shutdown` 만 돌고 나머지는 `busy` 와 pid 로 거절된다.
#[test]
fn a_daemon_at_its_connection_cap_still_answers_status_and_stops() {
    let f = Fence::new();
    let (daemon, first) = f.start();
    let pid = first["pid"].as_u64().unwrap();
    // 하나하나 답을 받아 둔다 — 그러면 서른둘 모두 연결 스레드에 들어 상한을 센 뒤다.
    let held: Vec<Wire> = (0..32)
        .map(|n| {
            let mut wire = Wire::open(&f.socket());
            assert_eq!(wire.call(n, "ping", json!({}))["result"], json!({}), "{n}번째 연결");
            wire
        })
        .collect();
    let mut over = Wire::open(&f.socket());
    let refused = over.call(99, "ping", json!({}));
    assert_eq!(refused["error"]["data"]["code"], "busy", "상한을 넘은 ping 이 돌았다 — {refused}");
    assert_eq!(refused["error"]["data"]["pid"], pid, "{refused}");

    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true, "{status}");
    assert_eq!(status["pid"], pid, "{status}");
    let again = f.json(&["daemon", "start", "--json"]);
    assert_eq!(again["started"], false, "{again}");
    let stopped = f.json(&["daemon", "stop", "--json"]);
    assert_eq!(stopped["stopped"], true, "{stopped}");
    assert_eq!(stopped["pid"], pid, "{stopped}");
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");
    assert!(!f.socket().exists(), "멈춘 데몬이 소켓을 남겼다");
    drop(held);
}

/// 상한을 넘은 연결의 수명 길(넷)마저 말없는 연결이 쥐면 `hello` 도 못 받는다 — 그때 `status` 는 `busy` 로 넘어지되
/// 데몬의 pid 를 댄다. 수명 길은 2초 안에 말이 없는 연결을 닫으니, 그 뒤에는 다시 닿는다(moai-uezh).
#[test]
fn a_daemon_too_full_even_for_hello_is_named_by_pid_and_frees_up() {
    let f = Fence::new();
    let (daemon, first) = f.start();
    let pid = first["pid"].as_u64().unwrap();
    let held: Vec<Wire> = (0..32)
        .map(|n| {
            let mut wire = Wire::open(&f.socket());
            assert_eq!(wire.call(n, "ping", json!({}))["result"], json!({}), "{n}번째 연결");
            wire
        })
        .collect();
    // 받는 스레드는 붙은 차례로 받는다 — 말없는 넷이 수명 길을 먼저 쥐고, 다섯째는 곧바로 거절된다.
    let silent: Vec<UnixStream> = (0..4).map(|_| UnixStream::connect(f.socket()).unwrap()).collect();
    // 데몬은 요청을 안 읽고 거절을 쓰고 닫는다 — 그 닫기가 먼저면 이쪽 쓰기가 끊기니, 쓰기는 넘기고 답을 읽는다.
    let mut fifth = Wire::open(&f.socket());
    let _ = fifth.writer.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"hello\",\"params\":{\"proto\":1}}\n");
    let mut line = String::new();
    fifth.reader.read_line(&mut line).unwrap();
    let refused: Value =
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("거절이 JSON 이 아니다: {e} — {line:?}"));
    assert_eq!(refused["error"]["data"]["code"], "busy", "수명 길이 찼는데 hello 가 돌았다 — {refused}");
    assert_eq!(refused["error"]["data"]["pid"], pid, "{refused}");
    let error = f.refused(&["daemon", "status", "--json"]);
    assert_eq!(error["code"], "busy", "{error}");
    assert!(error.to_string().contains(&format!("pid {pid}")), "거절이 pid 를 안 댄다 — {error}");
    // 말없는 넷은 시한(2초)에 닫힌다 — 그 뒤 stop 이 닿는다.
    std::thread::sleep(Duration::from_millis(2500));
    let stopped = f.json(&["daemon", "stop", "--json"]);
    assert_eq!(stopped["pid"], pid, "{stopped}");
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");
    drop((held, silent));
}

/// 멈추는 답을 쓴 연결은 제 소켓에 붙어 받는 스레드를 깨운다 — 그 파일이 지워졌으면 붙기가 실패한다. 그래도 받는
/// 스레드는 제 간격(`TICK`)마다 `stop` 을 보아 그친다(moai-uezh). 파일이 없으니 `moai daemon stop` 은 데몬을
/// 못 찾는다 — 멈추는 길은 이미 열린 연결로 보내는 `daemon.shutdown` 이다.
#[test]
fn a_daemon_whose_socket_file_was_removed_still_stops() {
    let f = Fence::new();
    let (daemon, mut wire) = served(&f);
    let pid = wire.call(1, "hello", json!({ "proto": 1 }))["result"]["pid"].clone();
    std::fs::remove_file(f.socket()).unwrap();
    assert!(UnixStream::connect(f.socket()).is_err(), "지운 소켓에 아직 붙는다 — 깨우는 붙기가 실패하는 자리가 아니다");
    let bye = wire.call(2, "daemon.shutdown", json!({}));
    assert_eq!(bye["result"]["pid"], pid, "{bye}");
    let asked = std::time::Instant::now();
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");
    assert!(
        asked.elapsed() < Duration::from_secs(5),
        "멈추기까지 {:?} — 받는 스레드가 stop 을 늦게 봤다",
        asked.elapsed()
    );
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

// ---- 읽기 메서드(moai-5n6m.q16) ---------------------------------------------------------------------------------

impl Fence {
    /// 걸어 둔 프로젝트 하나(이슈 둘) — 등록은 제 `MOAI_CONFIG` 에 한다.
    fn project(&self, name: &str) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir(&dir).unwrap();
        let d = dir.to_str().unwrap();
        for args in [
            vec!["-C", d, "init"],
            vec!["-C", d, "add", "alpha", "--user", ACTOR, "-p", "1", "-t", "bug"],
            vec!["-C", d, "add", "beta", "--user", ACTOR],
            vec!["project", "add", d],
        ] {
            let out = self.run(&args);
            assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        }
        dir
    }
}

const ACTOR: &str = "Tester (tester@example.com)";

/// 데몬을 띄우고 연결 하나를 연다.
fn served(f: &Fence) -> (Daemon, Wire) {
    let (daemon, _) = f.start();
    (daemon, Wire::open(&f.socket()))
}

/// CLI 의 `--json` stdout 한 줄.
fn cli(f: &Fence, root: &Path, args: &[&str]) -> Value {
    let mut all = vec!["-C", root.to_str().unwrap(), "--json"];
    all.extend_from_slice(args);
    f.json(&all)
}

/// CLI 를 환경만 덮어 돌린다 — 화면 말·시계를 요청마다 바꿔 견주려는 것이다.
fn cli_with(f: &Fence, root: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut all = vec!["-C", root.to_str().unwrap(), "--json", "--user", ACTOR];
    all.extend_from_slice(args);
    let mut cmd = f.cmd(&all);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

/// 견주기용 프로젝트 — 에픽·마일스톤·태그·메모·미룸·지운 줄·옆 워크트리의 더 늦은 고침을 모두 든다.
/// 시계는 열두 시로 못 박혀 있고, 옆 워크트리의 고침만 열두 시 반이라 겹친 화면에서 이긴다.
fn rich(f: &Fence, name: &str) -> PathBuf {
    let dir = f.0.join(name);
    std::fs::create_dir(&dir).unwrap();
    let d = dir.to_str().unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=Tester", "-c", "user.email=tester@example.com"])
            .args(args)
            .current_dir(&dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    let moai = |args: &[&str]| {
        let mut all = vec!["-C", d];
        all.extend_from_slice(args);
        let out = f.run(&all);
        assert!(out.status.success(), "moai {all:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let id_of = |title: &str| -> String {
        let rows = f.json(&["-C", d, "--json", "show", "--all", "--fields", "id,title"]);
        rows.as_array().unwrap().iter().find(|r| r["title"] == title).unwrap()["id"].as_str().unwrap().to_string()
    };
    moai(&["init"]);
    moai(&["milestone", "add", "v1", "--user", ACTOR]);
    let ms = id_of("v1");
    moai(&["epic", "add", "Storage", "--user", ACTOR, "--milestone", &ms]);
    let epic = id_of("Storage");
    moai(&["add", "parser bug", "--user", ACTOR, "-p", "1", "-t", "bug", "-t", "parser", "-e", &epic]);
    moai(&["add", "ui polish", "--user", ACTOR, "-t", "ui", "-t", "bug", "-p", "3"]);
    moai(&["add", "later thing", "--user", ACTOR, "-a", "none"]);
    moai(&["add", "doomed", "--user", ACTOR]);
    moai(&["add", "swamped", "--user", ACTOR, "-p", "0"]);
    let (parser, later, doomed, swamped) =
        (id_of("parser bug"), id_of("later thing"), id_of("doomed"), id_of("swamped"));
    moai(&["note", &parser, "found the cause in the lexer", "--user", ACTOR]);
    moai(&["mv", &parser, "in_progress", "--user", ACTOR]);
    moai(&["defer", &later, "-m", "next quarter", "--user", ACTOR]);
    moai(&["rm", &doomed, "--user", ACTOR]);
    moai(&["mv", &swamped, "in_progress", "--user", ACTOR]);
    moai(&["mv", &swamped, "review", "--user", ACTOR]);
    moai(&["project", "add", d]);
    // 옆 워크트리 — 올라온 사본과 달라진 줄 하나를 더 늦은 때에 둔다.
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    let side = f.0.join(format!("{name}-side"));
    git(&["worktree", "add", "-q", "-b", "wx", side.to_str().unwrap()]);
    let out = f
        .cmd(&["-C", side.to_str().unwrap(), "mv", &swamped, "done", "--user", ACTOR])
        .env("MOAI_NOW", "2026-11-04T12:30:00Z")
        .env("MOAI_HERE", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

/// 요청 하나와 CLI 한 번의 견줌 — 값은 JSON 값으로, `messages` 는 stderr 줄로, `partial` 은 종료 코드로.
fn compare(f: &Fence, wire: &mut Wire, root: &Path, n: u64, method: &str, params: &Value, argv: &[&str]) {
    let mut sent = params.clone();
    sent["project"] = json!(root.to_str().unwrap());
    sent["actor"] = json!(ACTOR);
    let lang = params.get("lang").and_then(Value::as_str).unwrap_or("en");
    let reply = wire.call(n, method, sent);
    let out = cli_with(f, root, argv, &[("MOAI_LANG", lang)]);
    let ctx = format!("{method} {params} / moai {argv:?}");
    if out.stdout.is_empty() {
        // CLI 가 넘어졌다 — API 도 같은 코드로 넘어지고 stderr 줄을 곁에 싣는다.
        let said: Value =
            serde_json::from_slice(&out.stderr).unwrap_or_else(|e| panic!("오류가 JSON 이 아니다: {e} — {ctx}"));
        assert_eq!(reply["error"]["code"], -32000, "{ctx}: {reply}");
        assert_eq!(reply["error"]["data"]["code"], said["code"], "{ctx}: {reply}");
        assert_eq!(reply["error"]["message"], said["error"], "{ctx}: {reply}");
        return;
    }
    let want: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("CLI 가 JSON 을 안 냈다: {e} — {ctx}\n{}", String::from_utf8_lossy(&out.stderr)));
    let result = &reply["result"];
    assert!(result.is_object(), "{ctx}: {reply}");
    assert_eq!(result["value"], want, "{ctx}");
    let stderr: Vec<String> = String::from_utf8_lossy(&out.stderr)
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let messages: Vec<String> = result["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("messages 가 배열이 아니다: {ctx}"))
        .iter()
        .flat_map(|m| m.as_str().unwrap().lines().map(|l| l.trim_end().to_string()).collect::<Vec<_>>())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(messages, stderr, "messages ≠ stderr — {ctx}");
    assert_eq!(result["partial"], json!(!out.status.success()), "partial ≠ 종료 코드 — {ctx}");
}

#[test]
fn the_read_methods_answer_as_the_cli_does() {
    let f = Fence::new();
    let root = rich(&f, "proj");
    let (_daemon, mut wire) = served(&f);

    // (메서드, 요청 필드, CLI 인자). 거르개: 쉼표는 "또는", 배열은 "그리고".
    let mut cases: Vec<(&str, Value, Vec<&str>)> = vec![
        ("status", json!({}), vec!["status"]),
        ("status", json!({ "worktree": true }), vec!["status", "--worktree"]),
        ("status", json!({ "lang": "ko" }), vec!["status"]),
        ("ready", json!({}), vec!["ready"]),
        ("ready", json!({ "limit": 1 }), vec!["ready", "-n", "1"]),
        ("ready", json!({ "worktree": true }), vec!["ready", "--worktree"]),
        ("ready", json!({ "lang": "ko", "worktree": true }), vec!["ready", "--worktree"]),
        ("prime", json!({}), vec!["prime"]),
        ("prime", json!({ "worktree": true }), vec!["prime", "--worktree"]),
        ("show", json!({}), vec!["show"]),
        ("show", json!({ "all": true }), vec!["show", "--all"]),
        ("show", json!({ "archived": true, "all": true }), vec!["show", "--all", "--archived"]),
        ("show", json!({ "deferred": true }), vec!["show", "--deferred"]),
        ("show", json!({ "worktree": true }), vec!["show", "--worktree"]),
        ("show", json!({ "worktree": true, "all": true, "tree": true }), vec!["show", "--worktree", "--all", "--tree"]),
        ("show", json!({ "status": "todo,in_progress" }), vec!["show", "-s", "todo,in_progress"]),
        ("show", json!({ "status": ["todo", "in_progress"] }), vec!["show", "-s", "todo", "-s", "in_progress"]),
        ("show", json!({ "tag": "bug,ui" }), vec!["show", "-t", "bug,ui"]),
        ("show", json!({ "tag": ["bug", "parser"] }), vec!["show", "-t", "bug", "-t", "parser"]),
        ("show", json!({ "no_tag": "ui" }), vec!["show", "--no-tag", "ui"]),
        ("show", json!({ "priority": 1 }), vec!["show", "-p", "1"]),
        ("show", json!({ "priority": "0,1" }), vec!["show", "-p", "0,1"]),
        ("show", json!({ "assignee": ["none", "me"] }), vec!["show", "-a", "none", "-a", "me"]),
        ("show", json!({ "grep": "lexer" }), vec!["show", "-g", "lexer"]),
        ("show", json!({ "type": "epic" }), vec!["show", "--type", "epic"]),
        ("show", json!({ "type": "milestone", "tree": true }), vec!["show", "--type", "milestone", "--tree"]),
        ("show", json!({ "stale": 0 }), vec!["show", "--stale", "0"]),
        ("show", json!({ "since": "2026-11-04T11:00:00Z" }), vec!["show", "--since", "2026-11-04T11:00:00Z"]),
        ("show", json!({ "filter": ["status=todo"] }), vec!["show", "--filter", "status=todo"]),
        ("show", json!({ "tree": true }), vec!["show", "--tree"]),
        ("show", json!({ "tree": true, "lang": "ko" }), vec!["show", "--tree"]),
        ("show", json!({ "removed": true }), vec!["show", "--removed"]),
        ("show", json!({ "fields": ["id", "title"] }), vec!["show", "--fields", "id,title"]),
        ("show", json!({ "fields": "id,status" }), vec!["show", "--fields", "id,status"]),
        ("show", json!({ "sort": "priority,id" }), vec!["show", "--sort", "priority,id"]),
        ("show", json!({ "sort": "priority", "reverse": true }), vec!["show", "--sort", "priority", "--reverse"]),
        (
            "show",
            json!({ "sort": "id", "limit": 2, "fields": "id" }),
            vec!["show", "--sort", "id", "-n", "2", "--fields", "id"],
        ),
        ("stats", json!({}), vec!["stats"]),
        ("stats", json!({ "by": ["status", "tag"] }), vec!["stats", "--by", "status,tag"]),
        (
            "stats",
            json!({ "by": "assignee", "bucket": "day", "last": 3 }),
            vec!["stats", "--by", "assignee", "--bucket", "day", "--last", "3"],
        ),
        ("stats", json!({ "bucket": "week", "last": 2 }), vec!["stats", "--bucket", "week", "--last", "2"]),
        ("stats", json!({ "status": "todo", "tag": "bug" }), vec!["stats", "-s", "todo", "-t", "bug"]),
        ("stats", json!({ "lang": "ko", "by": "status" }), vec!["stats", "--by", "status"]),
        ("project.ls", json!({}), vec!["project", "ls"]),
        ("project.ls", json!({ "lang": "ko" }), vec!["project", "ls"]),
    ];
    // 쪽 넘기기 — 첫 쪽의 마지막 id 가 둘째 쪽의 `after` 다.
    let page = cli_with(&f, &root, &["show", "--sort", "id", "-n", "2", "--fields", "id"], &[]);
    let first: Value = serde_json::from_slice(&page.stdout).unwrap();
    let last = first.as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    cases.push((
        "show",
        json!({ "sort": "id", "limit": 2, "after": last, "fields": "id" }),
        vec!["show", "--sort", "id", "-n", "2", "--after", &last, "--fields", "id"],
    ));
    // 한 id — 이슈 하나(글·이력·덧붙은 층까지).
    let rows = cli_with(&f, &root, &["show", "--all", "--fields", "id,title"], &[]);
    let rows: Value = serde_json::from_slice(&rows.stdout).unwrap();
    let id = rows.as_array().unwrap().iter().find(|r| r["title"] == "parser bug").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    cases.push(("show", json!({ "id": id }), vec!["show", &id]));
    cases.push(("show", json!({ "id": id, "worktree": true }), vec!["show", &id, "--worktree"]));
    cases.push(("show", json!({ "id": id, "lang": "ko" }), vec!["show", &id]));

    // 겹쳐 본 길이 밋밋한 길과 다르다는 것부터 — 같지 않은 것을 견주지 않으면 이 시험은 아무것도 안 잰다.
    let plain = cli_with(&f, &root, &["show", "--all"], &[]);
    let over = cli_with(&f, &root, &["show", "--all", "--worktree"], &[]);
    assert_ne!(plain.stdout, over.stdout, "옆 워크트리의 고침이 안 겹쳤다 — 견줄 거리가 없다");

    for (n, (method, params, argv)) in cases.iter().enumerate() {
        compare(&f, &mut wire, &root, n as u64 + 1, method, params, argv);
    }
}

#[test]
fn a_torn_snapshot_line_is_carried_as_messages_and_partial() {
    let f = Fence::new();
    let root = f.project("proj");
    let snapshot = root.join(".moai/issues.jsonl");
    let mut text = std::fs::read_to_string(&snapshot).unwrap();
    text.push_str("{this line is torn\n");
    std::fs::write(&snapshot, text).unwrap();
    let (_daemon, mut wire) = served(&f);

    // CLI 는 값을 내고도 1 로 끝나며 stderr 에 못 읽은 줄을 댄다 — 그 둘이 `partial` 과 `messages` 다.
    let out = cli_with(&f, &root, &["show"], &[]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!out.stderr.is_empty());
    for (n, (method, params, argv)) in [
        ("show", json!({}), vec!["show"]),
        ("status", json!({}), vec!["status"]),
        ("ready", json!({}), vec!["ready"]),
        ("stats", json!({}), vec!["stats"]),
        ("show", json!({ "lang": "ko" }), vec!["show"]),
    ]
    .iter()
    .enumerate()
    {
        compare(&f, &mut wire, &root, n as u64 + 1, method, params, argv);
    }
    let reply = wire.call(9, "show", json!({ "project": root.to_str().unwrap(), "actor": ACTOR }));
    assert_eq!(reply["result"]["partial"], true, "{reply}");
    assert!(!reply["result"]["messages"].as_array().unwrap().is_empty(), "{reply}");
}

#[test]
fn an_unregistered_or_missing_project_is_refused() {
    let f = Fence::new();
    let _root = f.project("proj");
    let loose = f.0.join("loose");
    std::fs::create_dir(&loose).unwrap();
    let (_daemon, mut wire) = served(&f);

    let missing = wire.call(1, "status", json!({}));
    assert_eq!(missing["error"]["code"], -32602, "{missing}");
    assert!(missing["error"]["message"].as_str().unwrap().contains("project"), "{missing}");

    let unknown = wire.call(2, "status", json!({ "project": loose.to_str().unwrap() }));
    assert_eq!(unknown["error"]["code"], -32000, "{unknown}");
    assert_eq!(unknown["error"]["data"]["code"], "not_registered", "{unknown}");
    // 상대 경로는 걸어 둔 것으로 치지 않는다.
    let relative = wire.call(3, "status", json!({ "project": "proj" }));
    assert_eq!(relative["error"]["data"]["code"], "not_registered", "{relative}");
}

#[test]
fn an_unknown_key_a_bad_lang_tz_or_value_is_invalid_params() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let (_daemon, mut wire) = served(&f);

    let typo = wire.call(1, "show", json!({ "project": project, "staus": "todo" }));
    assert_eq!(typo["error"]["code"], -32602, "{typo}");
    assert!(typo["error"]["message"].as_str().unwrap().contains("staus"), "키 이름을 안 댄다 — {typo}");
    // 읽기 메서드는 argv 를 받지 않는다.
    let argv = wire.call(2, "show", json!({ "project": project, "argv": ["--all"] }));
    assert_eq!(argv["error"]["code"], -32602, "{argv}");
    let lang = wire.call(3, "status", json!({ "project": project, "lang": "xx" }));
    assert_eq!(lang["error"]["code"], -32602, "{lang}");
    let tz = wire.call(4, "status", json!({ "project": project, "tz": "Mars/Base" }));
    assert_eq!(tz["error"]["code"], -32602, "{tz}");
    let kind = wire.call(5, "show", json!({ "project": project, "type": "nope" }));
    assert_eq!(kind["error"]["code"], -32602, "{kind}");
    let limit = wire.call(6, "ready", json!({ "project": project, "limit": 0 }));
    assert_eq!(limit["error"]["code"], -32602, "{limit}");
    let shape = wire.call(7, "show", json!({ "project": project, "tree": "yes" }));
    assert_eq!(shape["error"]["code"], -32602, "{shape}");
    // 환경 키도 글이 아니면 거절한다 — 말없이 빠지면 데몬의 사람·말로 답한다.
    let who = wire.call(9, "ready", json!({ "project": project, "actor": { "name": "Kim", "email": "k@x" } }));
    assert_eq!(who["error"]["code"], -32602, "{who}");
    assert!(who["error"]["message"].as_str().unwrap().contains("actor"), "{who}");
    let lang_shape = wire.call(10, "changes", json!({ "project": project, "lang": ["ko"] }));
    assert_eq!(lang_shape["error"]["code"], -32602, "{lang_shape}");
    // 열거형은 CLI 처럼 글자 크기를 가린다. 위 끝을 넘은 값은 그 끝을 댄다.
    let axis = wire.call(11, "stats", json!({ "project": project, "by": "STATUS" }));
    assert_eq!(axis["error"]["code"], -32602, "{axis}");
    let last = wire.call(12, "stats", json!({ "project": project, "last": 1001 }));
    assert!(last["error"]["message"].as_str().unwrap().contains("1000"), "{last}");
    // 좋은 lang 은 화면 말을 바꾼다.
    let ko = wire.call(8, "status", json!({ "project": project, "lang": "ko" }));
    assert!(ko["result"]["value"].is_object() || ko["result"]["value"].is_array(), "{ko}");
}

#[test]
fn a_cli_failure_is_carried_with_its_code() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let (_daemon, mut wire) = served(&f);

    let want = f.refused(&["-C", project, "--json", "show", "no-such-id"]);
    let got = wire.call(1, "show", json!({ "project": project, "id": "no-such-id" }));
    assert_eq!(got["error"]["code"], -32000, "{got}");
    assert_eq!(got["error"]["data"]["code"], want["code"], "{got} / {want}");
}

#[test]
fn changes_over_the_socket_sees_what_the_cli_writes() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let ids =
        |v: &Value| -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() };
    let (_daemon, mut wire) = served(&f);

    let id_of = |title: &str| -> String {
        let rows = cli(&f, &root, &["show", "--all", "--fields", "id,title"]);
        rows.as_array().unwrap().iter().find(|r| r["title"] == title).unwrap()["id"].as_str().unwrap().to_string()
    };
    let (alpha, beta) = (id_of("alpha"), id_of("beta"));

    // 커서가 없으면 처음 — 전체를 다시 읽으라는 `reset` 과 첫 커서. 봉투는 읽기 메서드와 같다.
    let first = wire.call(1, "changes", json!({ "project": project }));
    let value = &first["result"]["value"];
    assert_eq!(value["reset"], true, "{first}");
    assert_eq!(first["result"]["partial"], false, "{first}");
    assert_eq!(first["result"]["messages"], json!([]), "{first}");
    let cursor = value["cursor"].as_str().unwrap().to_string();

    // CLI 로 쓴다 — 하나 짓고, 하나에 메모를 붙이고, 하나를 지운다.
    for args in [
        vec!["-C", project, "add", "gamma", "--user", ACTOR],
        vec!["-C", project, "note", &alpha, "a memo", "--user", ACTOR],
        vec!["-C", project, "rm", &beta, "--user", ACTOR],
    ] {
        let out = f.run(&args);
        assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let gamma = id_of("gamma");

    let second = wire.call(2, "changes", json!({ "project": project, "cursor": cursor }));
    let value = &second["result"]["value"];
    assert_eq!(value["reset"], false, "{second}");
    let mut changed = ids(&value["changed"]);
    changed.sort();
    let mut want = vec![alpha.clone(), gamma];
    want.sort();
    assert_eq!(changed, want, "지은 줄과 메모가 붙은 줄이 바뀜이다 — {second}");
    assert_eq!(ids(&value["removed"]), [beta], "{second}");

    // 새 커서로 이어 가면 같은 것을 두 번 알리지 않는다.
    let next = value["cursor"].as_str().unwrap().to_string();
    let third = wire.call(3, "changes", json!({ "project": project, "cursor": next }));
    assert_eq!(third["result"]["value"]["changed"], json!([]), "{third}");
    assert_eq!(third["result"]["value"]["removed"], json!([]), "{third}");
    assert_eq!(third["result"]["value"]["reset"], false, "{third}");

    // 못 읽는 커서는 오류가 아니라 reset 이다. 다른 메서드처럼 모르는 키·안 걸어 둔 프로젝트는 거절한다.
    let junk = wire.call(4, "changes", json!({ "project": project, "cursor": "not-a-cursor" }));
    assert_eq!(junk["result"]["value"]["reset"], true, "{junk}");
    let typo = wire.call(5, "changes", json!({ "project": project, "cursr": "x" }));
    assert_eq!(typo["error"]["code"], -32602, "{typo}");
    let shape = wire.call(6, "changes", json!({ "project": project, "cursor": 5 }));
    assert_eq!(shape["error"]["code"], -32602, "{shape}");
    let none = wire.call(7, "changes", json!({}));
    assert_eq!(none["error"]["code"], -32602, "{none}");
    let loose = wire.call(8, "changes", json!({ "project": f.0.display().to_string() }));
    assert_eq!(loose["error"]["data"]["code"], "not_registered", "{loose}");
}

// ---- changes 가 기억한 스냅샷과 견준다(moai-8ts9) ------------------------------------------------------------------

/// `changes` 한 번 — `(커서, 바뀜, 사라짐, reset)`. 바뀜은 id 순이다.
fn poll(wire: &mut Wire, n: u64, project: &str, cursor: Option<&str>) -> (String, Vec<String>, Vec<String>, bool) {
    let mut params = json!({ "project": project });
    if let Some(c) = cursor {
        params["cursor"] = json!(c);
    }
    let reply = wire.call(n, "changes", params);
    let value = &reply["result"]["value"];
    let ids = |v: &Value| -> Vec<String> {
        v.as_array().unwrap_or_else(|| panic!("{reply}")).iter().map(|x| x.as_str().unwrap().to_string()).collect()
    };
    (
        value["cursor"].as_str().unwrap().to_string(),
        ids(&value["changed"]),
        ids(&value["removed"]),
        value["reset"] == true,
    )
}

/// 에픽 하나(멤버 둘)와 마일스톤 하나를 더 든 걸어 둔 프로젝트 — `(뿌리, 에픽, 멤버 둘, 마일스톤)`.
fn grouped(f: &Fence) -> (PathBuf, String, [String; 2], String) {
    let root = f.project("proj");
    let d = root.to_str().unwrap().to_string();
    let plan = f.0.join("plan.md");
    std::fs::write(&plan, "# grouped\n- one\n- two\n").unwrap();
    for args in [
        vec!["-C", &d, "add", "--from", plan.to_str().unwrap(), "--user", ACTOR],
        vec!["-C", &d, "milestone", "add", "v1", "--user", ACTOR],
    ] {
        let out = f.run(&args);
        assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let rows = cli(f, &root, &["show", "--all", "--fields", "id,title,kind"]);
    let id = |kind: &str, title: &str| -> String {
        rows.as_array().unwrap().iter().find(|r| r["kind"] == kind && r["title"] == title).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    (root.clone(), id("epic", "grouped"), [id("issue", "one"), id("issue", "two")], id("milestone", "v1"))
}

/// 걸어 둔 프로젝트에서 제목으로 id 를 찾는다.
fn id_titled(f: &Fence, root: &Path, title: &str) -> String {
    let rows = cli(f, root, &["show", "--all", "--fields", "id,title"]);
    rows.as_array().unwrap().iter().find(|r| r["title"] == title).unwrap()["id"].as_str().unwrap().to_string()
}

/// **`updated_at` 을 옛 값으로 둔 채 들어온 줄도 선다**(리뷰 `moai-5n6m.e9f` 의 4). 다른 clone 에서 한 시간 전에 고친
/// 줄이 `git pull` 로 들어온 꼴이다 — 창(`updated_at`)으로 거르던 판은 바뀜으로도 `reset` 으로도 안 냈다.
#[test]
fn changes_sees_a_row_pulled_in_with_an_old_stamp() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let alpha = id_titled(&f, &root, "alpha");
    let (_daemon, mut wire) = served(&f);
    let (cursor, _, _, reset) = poll(&mut wire, 1, project, None);
    assert!(reset);

    let path = root.join(".moai/issues.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    let pulled: String = text
        .lines()
        .map(|line| {
            let mut row: Value = serde_json::from_str(line).unwrap();
            if row["id"] == alpha.as_str() {
                row["title"] = json!("alpha from a pull");
                row["updated_at"] = json!("2026-11-04T11:00:00Z");
            }
            format!("{row}\n")
        })
        .collect();
    std::fs::write(&path, pulled).unwrap();
    let (_, changed, removed, reset) = poll(&mut wire, 2, project, Some(&cursor));
    assert_eq!((changed, removed, reset), (vec![alpha], vec![], false), "옛 도장으로 들어온 고침을 놓쳤다");
}

/// **파생값만 바뀐 줄도 선다**(리뷰의 6). 마지막 멤버가 닫히면 에픽의 칸이, 에픽이 마일스톤에 들면 멤버가 물려받는
/// 마일스톤이 바뀐다 — 그 값을 든 줄은 파일에서 그대로다. 제 줄이 바뀐 id 만 내던 판은 그 줄을 안 냈다.
#[test]
fn changes_names_rows_whose_derived_values_moved() {
    let f = Fence::new();
    let (root, epic, [one, two], milestone) = grouped(&f);
    let project = root.to_str().unwrap();
    let (_daemon, mut wire) = served(&f);
    let (cursor, _, _, reset) = poll(&mut wire, 1, project, None);
    assert!(reset);

    for id in [&one, &two] {
        let out = f.run(&["-C", project, "mv", id, "done", "--user", ACTOR]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let (cursor, changed, _, reset) = poll(&mut wire, 2, project, Some(&cursor));
    assert!(!reset);
    assert!(changed.contains(&epic), "마지막 멤버가 닫혔는데 에픽이 안 섰다 — {changed:?}");

    let out = f.run(&["-C", project, "edit", &epic, "--milestone", &milestone, "--user", ACTOR]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (_, changed, _, reset) = poll(&mut wire, 3, project, Some(&cursor));
    assert!(!reset);
    for member in [&one, &two] {
        assert!(changed.contains(member), "에픽을 마일스톤에 넣었는데 {member} 가 안 섰다 — {changed:?}");
    }
}

/// **기억에서 밀려난 지문은 `reset` 이다**(프로젝트마다 최근 8벌). 커서에 상태를 싣던 판은 그대로 이어 갔다.
#[test]
fn changes_resets_an_evicted_cursor() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let alpha = id_titled(&f, &root, "alpha");
    let (_daemon, mut wire) = served(&f);
    let (old, _, _, _) = poll(&mut wire, 1, project, None);
    let (same, changed, _, reset) = poll(&mut wire, 2, project, Some(&old));
    assert_eq!((changed, reset), (vec![], false));
    assert_eq!(same, old, "안 바뀐 스냅샷이 다른 커서를 냈다");

    // 아홉 번 바뀌며 이어 가면 첫 스냅샷은 밀려난다.
    let mut cursor = old.clone();
    for n in 0..9u64 {
        let title = format!("alpha {n}");
        let out = f.run(&["-C", project, "edit", &alpha, "--title", &title, "--user", ACTOR]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let (next, changed, _, reset) = poll(&mut wire, 10 + n, project, Some(&cursor));
        assert_eq!((changed, reset), (vec![alpha.clone()], false));
        cursor = next;
    }
    let (_, changed, removed, reset) = poll(&mut wire, 30, project, Some(&old));
    assert!(reset && changed.is_empty() && removed.is_empty(), "밀려난 지문이 reset 이 아니다");
    assert!(!poll(&mut wire, 31, project, Some(&cursor)).3, "지금 커서가 살아 있지 않다");
}

/// **기억은 데몬의 것이다** — 다시 띄운 데몬은 옛 커서를 모르고, 스냅샷이 같아도 `reset` 이다(2026-11-04 사람이
/// 받아들인 대가). 서버에 아무것도 안 두던 판은 그대로 이어 갔다.
#[test]
fn changes_resets_after_a_restart() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let (daemon, mut wire) = served(&f);
    let (cursor, _, _, _) = poll(&mut wire, 1, project, None);
    assert!(!poll(&mut wire, 2, project, Some(&cursor)).3);

    drop(wire);
    let stopped = f.json(&["daemon", "stop", "--json"]);
    assert_eq!(stopped["stopped"], true, "{stopped}");
    assert!(daemon.wait().success());
    let (_daemon, mut wire) = served(&f);
    let (_, changed, removed, reset) = poll(&mut wire, 1, project, Some(&cursor));
    assert!(reset && changed.is_empty() && removed.is_empty(), "다시 띄운 데몬이 옛 커서로 이어 갔다");
}
