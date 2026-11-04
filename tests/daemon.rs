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

#[test]
fn the_read_methods_answer_as_the_cli_does() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let (_daemon, mut wire) = served(&f);

    let cases: Vec<(&str, Value, Vec<&str>)> = vec![
        ("status", json!({}), vec!["status"]),
        ("ready", json!({ "limit": 1 }), vec!["ready", "-n", "1"]),
        ("prime", json!({}), vec!["prime"]),
        ("show", json!({}), vec!["show"]),
        (
            "show",
            json!({ "status": "todo", "tag": "bug", "sort": "priority" }),
            vec!["show", "-s", "todo", "-t", "bug", "--sort", "priority"],
        ),
        (
            "show",
            json!({ "tree": true, "fields": ["id", "title"], "priority": 1 }),
            vec!["show", "--tree", "--fields", "id,title", "-p", "1"],
        ),
        (
            "stats",
            json!({ "by": ["status", "tag"], "bucket": "day", "last": 3 }),
            vec!["stats", "--by", "status,tag", "--bucket", "day", "--last", "3"],
        ),
        ("project.ls", json!({}), vec!["project", "ls"]),
    ];
    for (n, (method, mut params, argv)) in cases.into_iter().enumerate() {
        params["project"] = json!(project);
        params["actor"] = json!(ACTOR);
        let reply = wire.call(n as u64 + 1, method, params);
        let result = &reply["result"];
        assert!(result.is_object(), "{method}: {reply}");
        let mut want_argv = vec!["--user", ACTOR];
        want_argv.extend(&argv);
        let want = cli(&f, &root, &want_argv);
        assert_eq!(result["value"], want, "{method} {argv:?}");
        assert_eq!(result["partial"], false, "{method}");
        assert!(result["messages"].is_array(), "{method}: {result}");
    }
    // 한 id — 이슈 하나.
    let alpha = cli(&f, &root, &["show", "-g", "alpha", "--fields", "id"])[0]["id"].as_str().unwrap().to_string();
    let reply = wire.call(50, "show", json!({ "project": project, "id": alpha }));
    assert_eq!(reply["result"]["value"], cli(&f, &root, &["show", &alpha]), "{reply}");
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
