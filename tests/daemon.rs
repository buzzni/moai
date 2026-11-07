//! `moai daemon` 을 실제 바이너리로 돌린다(moai-5n6m.x8t).
//!
//! 시험마다 제 `MOAI_CTL_DIR` 을 `/tmp` 아래 짧은 자리에 세운다 — 소켓 경로에는 OS 상한(108)이 있고, 사람의
//! 데몬(`$XDG_RUNTIME_DIR/moai`)에 닿으면 안 된다. 띄운 데몬은 [`Daemon`] 이 떨어질 때 죽인다 — 단언이 넘어져도
//! 프로세스가 남지 않게.
#![cfg(any(unix, windows))]

// 라이브러리는 이것을 안 내놓는다 — 시험만 쓰려고 공개 표면을 늘리지 않고 같은 소스를 묶는다.
#[allow(dead_code)]
#[path = "../src/transport.rs"]
mod transport;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;
use transport::{Listener, Stream};

const BIN: &str = env!("CARGO_BIN_EXE_moai");

struct Fence(PathBuf);

impl Fence {
    fn new() -> Fence {
        static NTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Windows 는 푼 철자로 선다 — `TEMP` 가 8.3 꼴(CI 러너의 `RUNNER~1`)이면 CLI 는 지금 자리의 그 철자를, 데몬은 푼
        // 철자를 대어, 두 답을 글자째 견주는 시험이 그 러너에서만 진다.
        let base = if cfg!(unix) {
            PathBuf::from("/tmp")
        } else {
            let real = std::fs::canonicalize(std::env::temp_dir()).unwrap();
            PathBuf::from(real.to_string_lossy().trim_start_matches(r"\\?\"))
        };
        let path = base.join(format!("moai-daemon-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
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
            // 옮김을 끄는 손잡이도 시험이 준 것만 — 돌리는 셸에서 새면 CLI 쪽 견주기가 워크트리의 제 트래커에 쓴다.
            .env_remove("MOAI_HERE")
            // git 의 사람은 시험이 준 것만 — 돌리는 셸의 `GIT_CONFIG_GLOBAL` 이 새면 `no_actor` 를 재는 시험이 헛돈다.
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
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
        self.start_with(&[])
    }

    /// [`Fence::start`] 에 환경을 더 세워 띄운다 — 데몬을 띄운 셸의 값이 요청에 새는가를 잰다.
    fn start_with(&self, env: &[(&str, &str)]) -> (Daemon, Value) {
        let mut cmd = self.cmd(&["daemon", "start", "--json"]);
        cmd.envs(env.iter().copied());
        Self::first_line(cmd)
    }

    /// `start` 와 같되, 데몬이 SIGINT 를 무시한 채로 뜬다 — 스크립트의 `moai daemon start &` 가 그렇게 띄운다.
    #[cfg(unix)]
    fn start_ignoring_sigint(&self) -> (Daemon, Value) {
        use std::os::unix::process::CommandExt as _;
        let mut cmd = self.cmd(&["daemon", "start", "--json"]);
        // SAFETY: signal(2) is async-signal-safe, so it may run between fork and exec.
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGINT, libc::SIG_IGN);
                Ok(())
            });
        }
        Self::first_line(cmd)
    }

    fn first_line(mut cmd: Command) -> (Daemon, Value) {
        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
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
    reader: BufReader<Stream>,
    writer: Stream,
}

impl Wire {
    fn open(path: &Path) -> Wire {
        let stream = Stream::connect(path).unwrap();
        // 꽉 찬 데몬은 거절을 쓰고 곧바로 닫는다 — 그 닫기가 먼저면 macOS 는 이 시한을 거절한다.
        transport::armed(stream.set_read_timeout(Some(Duration::from_secs(10)))).unwrap();
        Wire { reader: BufReader::new(stream.try_clone().unwrap()), writer: stream }
    }

    /// 답 한 줄을 파싱하지 않고 그대로 — 바이트를 견줄 때 쓴다.
    fn send_text(&mut self, bytes: &[u8]) -> String {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        line
    }

    fn send_raw(&mut self, bytes: &[u8]) -> Value {
        let line = self.send_text(bytes);
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("답이 JSON 이 아니다: {e} — {line:?}"))
    }

    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.send_raw(format!("{line}\n").as_bytes())
    }

    /// `call` 과 같되 답 줄을 파싱하지 않은 글로 돌려준다.
    fn call_text(&mut self, id: u64, method: &str, params: Value) -> String {
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.send_text(format!("{line}\n").as_bytes())
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

/// Windows 의 신호 자리 — Ctrl-Break 를 받은 데몬도 `moai daemon stop` 과 같이 끝난다. Ctrl-C 는 제 프로세스 묶음을 따로
/// 세운 자식에게 못 보내고, 처리기는 둘을 같은 갈래로 받는다. 시험을 돌리는 쪽에 콘솔이 없으면 하나 얻어 나눈다.
#[cfg(windows)]
#[test]
fn ctrl_break_stops_cleanly() {
    use std::os::windows::process::CommandExt as _;
    use windows_sys::Win32::System::Console::{AllocConsole, CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
    // 이미 콘솔이 있으면 실패하고 그것을 그대로 쓴다.
    // SAFETY: 인자 없는 호출이다.
    let _ = unsafe { AllocConsole() };
    let f = Fence::new();
    let mut cmd = f.cmd(&["daemon", "start", "--json"]);
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    let (daemon, first) = Fence::first_line(cmd);
    let pid = u32::try_from(first["pid"].as_u64().unwrap()).unwrap();
    // SAFETY: 이 시험이 막 띄운 자식의 프로세스 묶음에만 보낸다.
    assert_ne!(unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) }, 0);
    assert!(daemon.wait().success(), "Ctrl-Break 뒤 깨끗이 안 끝났다");
    assert!(!f.socket().exists(), "Ctrl-Break 가 표시 파일을 남겼다");
    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], false);
    assert_eq!(status["stale_socket"], false);
}

#[cfg(unix)]
#[test]
fn sigterm_and_sigint_stop_cleanly() {
    for signal in [libc::SIGTERM, libc::SIGINT] {
        let f = Fence::new();
        let (daemon, first) = f.start();
        let pid = first["pid"].as_i64().unwrap() as libc::pid_t;
        // SAFETY: pid is the child just started in this test, and signal is valid.
        assert_eq!(unsafe { libc::kill(pid, signal) }, 0);
        assert!(daemon.wait().success(), "signal {signal} did not exit successfully");
        assert!(!f.socket().exists(), "signal {signal} left a socket");
        let status = f.json(&["daemon", "status", "--json"]);
        assert_eq!(status["running"], false);
        assert_eq!(status["stale_socket"], false);
    }
}

/// SIGINT 를 무시한 채 뜬 데몬은 SIGINT 에 안 멈추고(터미널의 Ctrl-C 는 그 스크립트의 것이다), SIGTERM 에는 깨끗이 멈춘다.
#[cfg(unix)]
#[test]
fn an_ignored_sigint_stays_ignored() {
    let f = Fence::new();
    let (daemon, first) = f.start_ignoring_sigint();
    let pid = first["pid"].as_i64().unwrap() as libc::pid_t;
    // SAFETY: pid is the child just started in this test.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGINT) }, 0);
    std::thread::sleep(Duration::from_millis(1500));
    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true, "SIGINT stopped a daemon that started with it ignored: {status}");
    // SAFETY: as above.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    assert!(daemon.wait().success(), "SIGTERM did not exit successfully");
    assert!(!f.socket().exists(), "SIGTERM left a socket");
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
    let silent: Vec<Stream> = (0..4).map(|_| Stream::connect(f.socket()).unwrap()).collect();
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
    assert!(Stream::connect(f.socket()).is_err(), "지운 소켓에 아직 붙는다 — 깨우는 붙기가 실패하는 자리가 아니다");
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

/// 멈추는 데몬이 돌던 읽기를 기다리는 동안 `moai daemon status` 는 "떠 있지 않다" 로 0 이다 — 데몬은 기다리기 전에
/// 소켓 파일부터 지운다(moai-3hgm.uz0). 옛 판은 기다리는 동안 듣기를 열어 두어 새 연결이 대기열에 서서 답을 못 받고,
/// `status` 가 2초 뒤 "데몬이 아니다" 로 넘어졌다. 돌던 읽기는 5초(`DRAIN`)까지 기다리고 끊는다 — 그 읽기는 안 읽히는
/// 큰 답(소켓 버퍼를 넘는 `show`)이라 데몬의 쓰기 시한(30초)까지 막혀 있을 것이었다.
#[test]
fn status_says_not_running_while_a_stopping_daemon_waits_for_a_read() {
    let f = Fence::new();
    let root = f.project("big");
    let body = "x".repeat(60_000);
    for n in 0..24 {
        let mut child = f
            .cmd(&["-C", root.to_str().unwrap(), "add", &format!("big {n}"), "--user", ACTOR, "-b", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(body.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let (mut daemon, first) = f.start();
    let pid = first["pid"].as_u64().unwrap();
    // 답은 1MB 를 넘는다 — 안 읽으면 데몬의 쓰기가 소켓 버퍼에 막혀 이 읽기가 도는 채로 선다.
    let mut stuck = Wire::open(&f.socket());
    let show = json!({ "jsonrpc": "2.0", "id": 1, "method": "show", "params": { "project": root, "all": true } });
    stuck.writer.write_all(format!("{show}\n").as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let asked = std::time::Instant::now();
    let stop = f.cmd(&["daemon", "stop", "--json"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    while f.socket().exists() {
        assert!(asked.elapsed() < Duration::from_secs(3), "멈추는 데몬이 소켓 파일을 안 지웠다");
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], false, "{status}");
    assert_eq!(status["stale_socket"], false, "{status}");
    assert!(status.get("pid").is_none(), "{status}");
    assert!(daemon.0.try_wait().unwrap().is_none(), "status 를 물을 때 데몬이 이미 끝났다 — 기다리는 사이를 못 봤다");

    let out = stop.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stopped: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stopped["stopped"], true, "{stopped}");
    assert_eq!(stopped["pid"], pid, "{stopped}");
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");
    let took = asked.elapsed();
    assert!(took >= Duration::from_secs(4), "돌던 읽기를 {took:?} 만 기다렸다 — DRAIN 은 5초다");
    assert!(took < Duration::from_secs(9), "돌던 읽기를 {took:?} 기다렸다 — 읽기는 5초에 끊는다");
    drop(stuck);
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
    drop(Listener::bind(f.socket()).unwrap());
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
    let listener = Listener::bind(f.socket()).unwrap();
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
    assert!(text.contains("service manager"), "서비스 관리자를 통한 재시작을 안 댄다 — {text}");
    assert!(f.socket().exists(), "다른 판의 소켓을 지웠다");

    let status = f.json(&["daemon", "status", "--json"]);
    assert_eq!(status["running"], true, "{status}");
    assert_eq!(status["same_version"], false, "{status}");
    assert_eq!(status["version"], "0.0.1", "{status}");
    assert_eq!(status["pid"], 4242, "{status}");
    let human = f.run(&["daemon", "status"]);
    assert!(String::from_utf8_lossy(&human.stdout).contains("service manager"));

    let mut quit = Stream::connect(f.socket()).unwrap();
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
    let text = wire.call_text(n, method, sent);
    let reply: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("답이 JSON 이 아니다: {e} — {text:?}"));
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
    // 값만이 아니라 바이트도 — `value` 는 CLI 가 찍은 줄 그대로다(키 차례 포함, moai-3hgm.whp). 줄이 여럿이면 그 줄들의 배열.
    #[derive(serde::Deserialize)]
    struct Line {
        result: Result,
    }
    #[derive(serde::Deserialize)]
    struct Result {
        value: Box<serde_json::value::RawValue>,
    }
    let carried: Line = serde_json::from_str(&text).unwrap();
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    let printed = if lines.len() == 1 { lines[0].to_string() } else { format!("[{}]", lines.join(",")) };
    assert_eq!(carried.result.value.get(), printed, "value 의 바이트 ≠ CLI stdout — {ctx}");
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
    // `exit` 는 늘 서고 CLI 의 종료 코드다(moai-3hgm.k7s).
    assert_eq!(result["exit"], json!(out.status.code().expect("시그널로 끝났다")), "exit ≠ 종료 코드 — {ctx}");
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
        ("show", json!({ "group": "epic" }), vec!["show", "--group", "epic"]),
        ("show", json!({ "group": "assignee" }), vec!["show", "--group", "assignee"]),
        ("show", json!({ "sort": "people" }), vec!["show", "--sort", "people"]),
        ("show", json!({ "sort": "release" }), vec!["show", "--sort", "release"]),
        // 프리셋이 고른 묶기(`people` → `assignee`)보다 준 `group` 이 이긴다 — CLI 의 `--group` 과 같은 자리다.
        ("show", json!({ "sort": "people", "group": "status" }), vec!["show", "--sort", "people", "--group", "status"]),
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
    assert_eq!(reply["result"]["exit"], 1, "{reply}");
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
    let group = wire.call(10, "show", json!({ "project": project, "group": "nope" }));
    assert_eq!(group["error"]["code"], -32602, "{group}");
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
    assert_eq!(first["result"]["exit"], 0, "{first}");
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

// ---- 쓰기 메서드(moai-3hgm.k7s) ----------------------------------------------------------------------------------

/// 쓰기 요청 하나 — `project` 를 채워 보낸다. `actor` 는 `params` 에 준 것만 간다: 사람을 모르는 길을 재려면 빼고
/// 보낸다. 같음 시험(moai-3hgm.pmc)도 이것으로 보낸다.
fn write(wire: &mut Wire, root: &Path, n: u64, method: &str, params: Value) -> Value {
    let mut sent = params;
    sent["project"] = json!(root.to_str().unwrap());
    wire.call(n, method, sent)
}

/// 선 쓰기의 `result` — 봉투의 네 키가 다 섰고 `exit` 가 0 인지 본다.
fn wrote(wire: &mut Wire, root: &Path, n: u64, method: &str, params: Value) -> Value {
    let reply = write(wire, root, n, method, params.clone());
    let result = reply["result"].clone();
    assert!(result.is_object(), "{method} {params}: {reply}");
    assert_eq!(result["exit"], 0, "{method} {params}: {reply}");
    assert_eq!(result["partial"], false, "{method} {params}: {reply}");
    assert!(result["messages"].is_array(), "{method} {params}: {reply}");
    result
}

/// CLI 로 읽은 줄 하나(`show <id> --json`).
fn row(f: &Fence, root: &Path, id: &str) -> Value {
    cli(f, root, &["show", id])
}

/// **일곱 메서드가 CLI 의 쓰기 경로로 쓴다** — 쓴 것을 CLI 의 `show` 가 그대로 읽는다. 메서드마다 하나씩, 그리고 CLI 가
/// stderr 와 종료 코드로 말하는 두 갈래(남의 줄을 집은 것, `from` 에 진 집기)가 `messages`·`exit` 로 선다.
#[test]
fn the_write_methods_write_what_the_cli_reads() {
    let f = Fence::new();
    let root = f.project("proj");
    let actor = json!(ACTOR);
    let (_daemon, mut wire) = served(&f);
    let id_of = |title: &str| id_titled(&f, &root, title);
    let (alpha, beta) = (id_of("alpha"), id_of("beta"));

    // add — 종류 없이, 그리고 `kind` 로 `epic add`·`backlog add` 를.
    let made = wrote(
        &mut wire,
        &root,
        1,
        "add",
        json!({ "actor": actor, "title": "gamma", "tag": "x,y", "priority": 1, "body": "-" }),
    );
    let gamma = made["value"]["id"].as_str().unwrap().to_string();
    let shown = row(&f, &root, &gamma);
    assert_eq!((&shown["title"], &shown["tags"], &shown["priority"]), (&json!("gamma"), &json!(["x", "y"]), &json!(1)));
    assert_eq!(shown["body"], "-", "글 자리의 `-` 는 글이다 — {shown}");
    assert_eq!(shown["assignee_email"], "tester@example.com", "{shown}");
    let epic = wrote(&mut wire, &root, 2, "add", json!({ "actor": actor, "kind": "epic", "title": "Storage" }));
    assert_eq!(epic["value"]["kind"], "epic", "{epic}");
    let idea = wrote(&mut wire, &root, 3, "add", json!({ "actor": actor, "kind": "backlog", "title": "an idea" }));
    assert_eq!(idea["value"]["kind"], "backlog", "{idea}");

    // mv — 집기, 그리고 본 칸이 달라진 뒤의 늦은 집기는 `stale`·`partial`·`exit` 1 이다(CLI 의 종료 코드 1).
    wrote(&mut wire, &root, 4, "mv", json!({ "actor": actor, "id": alpha, "to": "in_progress", "from": "todo" }));
    assert_eq!(row(&f, &root, &alpha)["status"], "in_progress");
    let late = write(&mut wire, &root, 5, "mv", json!({ "actor": actor, "id": alpha, "to": "review", "from": "todo" }));
    let late = &late["result"];
    assert_eq!((&late["exit"], &late["partial"]), (&json!(1), &json!(true)), "{late}");
    assert_eq!(late["value"]["stale"][0]["id"], json!(alpha), "{late}");
    assert!(!late["messages"].as_array().unwrap().is_empty(), "진 집기가 아무 말도 안 했다 — {late}");
    assert_eq!(row(&f, &root, &alpha)["status"], "in_progress", "진 집기가 옮겼다");

    // 남의 줄 — 막지 않고 옮기며, CLI 가 stderr 에 내는 한 줄이 `messages` 에 선다(훅 규칙 5 는 API 에 없다).
    let theirs = wrote(
        &mut wire,
        &root,
        6,
        "add",
        json!({ "actor": actor, "title": "theirs", "assignee": "Lee (lee@example.com)" }),
    );
    let theirs = theirs["value"]["id"].as_str().unwrap().to_string();
    let moved = wrote(&mut wire, &root, 7, "mv", json!({ "actor": actor, "id": theirs, "to": "in_progress" }));
    assert_eq!(moved["value"]["theirs"][0]["id"], json!(theirs), "{moved}");
    assert!(moved["messages"].as_array().unwrap().iter().any(|m| m.as_str().unwrap().contains(&theirs)), "{moved}");
    let taken = wrote(
        &mut wire,
        &root,
        8,
        "mv",
        json!({ "actor": actor, "id": theirs, "to": "review", "take": true, "msg": "Lee said yes" }),
    );
    assert_eq!(taken["value"]["taken"][0]["id"], json!(theirs), "{taken}");
    assert_eq!(row(&f, &root, &theirs)["assignee_email"], "tester@example.com");

    // edit·note·defer·link·rm.
    wrote(
        &mut wire,
        &root,
        9,
        "edit",
        json!({ "actor": actor, "id": beta, "title": "beta two", "tag": ["ui"], "due": "2026-12-01" }),
    );
    let shown = row(&f, &root, &beta);
    assert_eq!(
        (&shown["title"], &shown["tags"], &shown["due_on"]),
        (&json!("beta two"), &json!(["ui"]), &json!("2026-12-01"))
    );
    let noted = wrote(&mut wire, &root, 10, "note", json!({ "actor": actor, "id": beta, "text": "-" }));
    assert_eq!((&noted["value"]["text"], &noted["value"]["by_email"]), (&json!("-"), &json!("tester@example.com")));
    wrote(&mut wire, &root, 11, "defer", json!({ "actor": actor, "id": [beta], "msg": "next quarter" }));
    assert!(row(&f, &root, &beta)["deferred_at"].is_string(), "미루지 않았다");
    wrote(&mut wire, &root, 12, "defer", json!({ "actor": actor, "id": beta, "undo": true }));
    assert!(row(&f, &root, &beta).get("deferred_at").is_none(), "되돌리지 않았다");
    wrote(&mut wire, &root, 13, "link", json!({ "actor": actor, "id": alpha, "blocks": beta }));
    assert_eq!(row(&f, &root, &beta)["blocked_by"], json!([alpha]));
    wrote(&mut wire, &root, 14, "rm", json!({ "actor": actor, "id": gamma }));
    let gone = f.refused(&["-C", root.to_str().unwrap(), "--json", "show", &gamma]);
    assert_eq!(gone["code"], "not_found", "{gone}");
}

/// **틀린 요청은 `-32602`, 명령이 넘어지면 `-32000` 에 CLI 의 `code`** — 같은 부름을 CLI 로 쳐서 코드를 견준다.
#[test]
fn a_bad_write_is_invalid_params_and_a_failed_one_carries_the_cli_code() {
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let alpha = id_titled(&f, &root, "alpha");
    let (_daemon, mut wire) = served(&f);

    for (n, (method, params, named)) in [
        ("add", json!({ "title": "t", "from": "-" }), "from"),
        ("add", json!({ "title": "t", "priority": "high" }), "priority"),
        ("mv", json!({ "id": alpha }), "to"),
        ("mv", json!({ "id": alpha, "to": "done", "take": "yes" }), "take"),
        ("edit", json!({ "id": alpha, "shift": 1, "start": "+1" }), "shift"),
        ("note", json!({ "id": alpha, "txt": "typo" }), "txt"),
        ("defer", json!({ "id": alpha, "undo": "no" }), "undo"),
        ("rm", json!({}), "id"),
        ("link", json!({ "id": alpha, "blocks": { "x": 1 } }), "blocks"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut sent = params.clone();
        sent["actor"] = json!(ACTOR);
        let got = write(&mut wire, &root, n as u64 + 1, method, sent);
        assert_eq!(got["error"]["code"], -32602, "{method} {params}: {got}");
        assert!(got["error"]["message"].as_str().unwrap().contains(named), "{method} {params}: 키를 안 댄다 — {got}");
    }

    // 명령이 넘어진 것 — 같은 부름을 CLI 로 쳐서 `code` 와 말을 견준다.
    for (n, (method, params, argv)) in [
        ("note", json!({ "id": "proj-zzzz", "text": "x" }), vec!["note", "proj-zzzz", "x"]),
        ("mv", json!({ "id": alpha, "to": "doing" }), vec!["mv", &alpha, "doing"]),
        ("add", json!({ "title": "t", "priority": 7 }), vec!["add", "t", "-p", "7"]),
        ("add", json!({}), vec!["add"]),
        ("edit", json!({ "id": alpha }), vec!["edit", &alpha]),
        ("rm", json!({ "line": 99 }), vec!["rm", "--line", "99"]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut sent = params.clone();
        sent["actor"] = json!(ACTOR);
        let got = write(&mut wire, &root, 100 + n as u64, method, sent);
        let mut all = vec!["-C", project, "--json", "--user", ACTOR];
        all.extend_from_slice(&argv);
        let want = f.refused(&all);
        assert_eq!(got["error"]["code"], -32000, "{method} {params}: {got}");
        assert_eq!(got["error"]["data"]["code"], want["code"], "{method} {params}: {got} / {want}");
        assert_eq!(got["error"]["message"], want["error"], "{method} {params}: {got} / {want}");
    }
}

/// **요청의 `actor` → 그 프로젝트의 git 설정 → `no_actor`** — 데몬을 띄운 셸의 `MOAI_ACTOR` 는 안 읽는다. 걸어 둔
/// 프로젝트는 git 저장소가 아니고 시험의 git 설정에 사람이 없다. 같은 요청에 `actor` 를 주면 선다.
#[test]
fn a_write_from_nobody_known_is_no_actor_whatever_the_daemon_was_started_with() {
    let f = Fence::new();
    let root = f.project("proj");
    let alpha = id_titled(&f, &root, "alpha");
    let (_daemon, _) = f.start_with(&[("MOAI_ACTOR", "Daemon (daemon@example.com)")]);
    let mut wire = Wire::open(&f.socket());

    for (n, (method, params)) in [
        ("add", json!({ "title": "who" })),
        ("note", json!({ "id": alpha, "text": "who" })),
        ("mv", json!({ "id": alpha, "to": "in_progress" })),
    ]
    .into_iter()
    .enumerate()
    {
        let got = write(&mut wire, &root, n as u64 + 1, method, params.clone());
        assert_eq!(got["error"]["code"], -32000, "{method} {params}: {got}");
        assert_eq!(got["error"]["data"]["code"], "no_actor", "데몬의 MOAI_ACTOR 를 읽었다 — {method}: {got}");
    }
    let named = wrote(&mut wire, &root, 9, "note", json!({ "id": alpha, "text": "me", "actor": ACTOR }));
    assert_eq!(named["value"]["by_email"], "tester@example.com", "{named}");
}

/// **링크 철자로 걸어 둔 프로젝트도 CLI 와 같은 자리를 댄다**(moai-3hgm.k7s) — `-C <링크>` 의 CLI 는 물리 경로에 서서
/// 그 뿌리를 말에 댄다. 데몬이 설정의 철자를 그대로 쓰면 같은 쓰기의 말이 링크를 댄다. 남의 줄을 집는 `mv` 의 말이
/// 뿌리를 댄다(`moai -C <뿌리> mv … --take`).
#[cfg(unix)]
#[test]
fn a_project_registered_through_a_symlink_names_the_root_the_cli_names() {
    let f = Fence::new();
    let root = f.project("proj");
    let link = f.0.join("proj-link");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    std::fs::write(f.0.join("config.toml"), format!("[[project]]\npath = {:?}\n", link.to_str().unwrap())).unwrap();
    let theirs = |n: &str| {
        let out =
            f.run(&["-C", root.to_str().unwrap(), "add", n, "--user", ACTOR, "-a", "Lee (lee@example.com)", "-q"]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let (one, two) = (theirs("one"), theirs("two"));
    let (_daemon, mut wire) = served(&f);

    let got = wrote(&mut wire, &link, 1, "mv", json!({ "actor": ACTOR, "id": one, "to": "in_progress" }));
    let out = cli_with(&f, &link, &["mv", &two, "in_progress"], &[]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let said = String::from_utf8_lossy(&out.stderr).replace(&two, &one);
    let messages: Vec<&str> = got["messages"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    assert_eq!(messages, said.lines().collect::<Vec<_>>(), "링크로 걸어 둔 프로젝트의 말이 CLI 와 다르다");
    assert!(said.contains(root.to_str().unwrap()), "CLI 가 물리 경로를 안 댔다 — 견줄 거리가 없다: {said}");
}

// ---- 쓰기의 같음·동시 쓰기·락 시한·워크트리·멈춤(moai-3hgm.pmc) ----------------------------------------------------

/// `.moai` 밑의 파일 전부 — `.moai` 에서 본 자리 → 바이트. 락 파일은 뺀다: 쓰기가 잡았다 놓은 자리일 뿐 쓴 것이 아니다
/// (`cmd::request` 시험의 `files_of` 와 같은 자다).
fn files_of(moai: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(at: &Path, base: &Path, into: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(at).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                walk(&path, base, into);
            } else if path.file_name().is_some_and(|n| n != "lock") {
                into.insert(path.strip_prefix(base).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut into = std::collections::BTreeMap::new();
    walk(moai, moai, &mut into);
    into
}

/// `.moai` 를 찍어 둔 그대로 되돌린다 — CLI 가 쓴 뒤 데몬이 **같은 시작 트래커에서** 다시 쓰게.
fn put_back(moai: &Path, snap: &std::collections::BTreeMap<String, Vec<u8>>) {
    std::fs::remove_dir_all(moai).unwrap();
    for (rel, bytes) in snap {
        let at = moai.join(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }
}

/// 스냅샷에 선 id 들.
fn ids_of(files: &std::collections::BTreeMap<String, Vec<u8>>) -> std::collections::BTreeSet<String> {
    let snap = files.get("issues.jsonl").map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    snap.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|row| row["id"].as_str().map(str::to_string))
        .collect()
}

/// 답 줄의 `result.value` 를 파싱하지 않은 바이트로 — CLI 의 stdout 과 글자째 견준다(moai-3hgm.whp).
fn carried_value(text: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Line {
        result: Inner,
    }
    #[derive(serde::Deserialize)]
    struct Inner {
        value: Box<serde_json::value::RawValue>,
    }
    let line: Line = serde_json::from_str(text).unwrap_or_else(|e| panic!("답에 value 가 없다: {e} — {text}"));
    line.result.value.get().to_string()
}

/// CLI 가 넘어진 판과 데몬의 오류를 견준다 — stderr 의 마지막 줄은 `{"code", "error"}` 오류 객체이고, 그 앞의 줄은
/// `error.data.messages` 다(없으면 키가 없다). 코드는 `-32000`, `data.code` 와 말은 CLI 의 것이다.
fn same_failure(reply: &Value, stderr: &str, ctx: &str) {
    let mut lines: Vec<&str> = stderr.lines().collect();
    let last = lines.pop().unwrap_or_else(|| panic!("넘어진 CLI 가 stderr 에 아무것도 안 냈다 — {ctx}"));
    let said: Value =
        serde_json::from_str(last).unwrap_or_else(|e| panic!("오류가 JSON 이 아니다: {e} — {ctx}: {last}"));
    let error = &reply["error"];
    assert_eq!(error["code"], -32000, "{ctx}: {reply}");
    assert_eq!(error["data"]["code"], said["code"], "{ctx}: {reply}");
    assert_eq!(error["message"], said["error"], "{ctx}: {reply}");
    match lines.is_empty() {
        true => assert!(
            error["data"].get("messages").is_none(),
            "CLI 가 말없이 넘어졌는데 messages 가 섰다 — {ctx}: {reply}"
        ),
        false => assert_eq!(error["data"]["messages"], json!(lines), "error.data.messages ≠ stderr — {ctx}"),
    }
}

/// 데몬이 쓴 `.moai` 와 CLI 가 쓴 `.moai` 가 파일째·바이트째 같다 — 다르면 그 파일의 처음 갈린 줄을 댄다(통째로 찍으면
/// 스냅샷 전체가 두 번 나와 어디가 갈렸는지 안 보인다).
fn same_files(
    got: &std::collections::BTreeMap<String, String>,
    want: &std::collections::BTreeMap<String, String>,
    ctx: &str,
) {
    assert_eq!(
        got.keys().collect::<Vec<_>>(),
        want.keys().collect::<Vec<_>>(),
        "데몬과 CLI 가 남긴 파일이 다르다 — {ctx}"
    );
    for (name, text) in got {
        let theirs = &want[name];
        if text == theirs {
            continue;
        }
        let (mut a, mut b) = (text.split_inclusive('\n'), theirs.split_inclusive('\n'));
        let at = (1..).find(|_| a.next() != b.next()).unwrap();
        let line = |t: &str| t.split_inclusive('\n').nth(at - 1).map(str::to_string);
        panic!("{name} 의 {at}째 줄이 다르다 — {ctx}\n데몬: {:?}\nCLI:  {:?}", line(text), line(theirs));
    }
}

/// 같음 표의 기대 — 둘 다 같은 것을 낸다, 또는 clap 이 거절하는 것이라 CLI 는 2 로 끝나고 데몬은 `-32602` 다(둘 다
/// 아무것도 안 쓴다).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Want {
    Same,
    Refused,
}

/// **같은 쓰기를 같은 시작 트래커에서 CLI 와 데몬으로**(moai-3hgm.pmc) — `moai -C <root> --json --user <사람> <argv>` 를
/// 한 번 치고, `.moai` 를 되돌린 뒤 같은 쓰기를 소켓으로 보낸다. 견주는 것은 쓴 뒤의 `.moai` 바이트(스냅샷·저널)와
/// `value` 바이트 대 stdout, `messages` 대 stderr, `exit` 대 종료 코드다. 남는 것은 데몬이 쓴 것이다.
///
/// - `root` 는 요청의 `project`(= `-C` 의 자리), `tracker` 는 쓰기가 닿는 `.moai` 의 주인 — 딸린 워크트리면 둘이 갈린다
/// - 새 줄의 id 는 시각과 프로세스로 뽑아 두 판이 다르다. 데몬 쪽의 새 id 를 CLI 의 것으로 바꿔 견주고, id 차례로 선
///   스냅샷은 줄을 골라 견준다(`cmd::request` 시험의 `same` 과 같다)
/// - `params` 에 `actor` 를 안 주면 둘 다 [`ACTOR`] 다. `"actor": null` 이면 CLI 도 `--user` 없이 친다 — 모르는 사람의 길
/// - `lang` 은 CLI 의 `MOAI_LANG` 이다
/// - CLI 는 `CLAUDE_CODE_SESSION_ID` 를 걷고 친다 — 그 값으로 집기를 적는 것은 CLI 만의 일이라(moai-3hgm.25e) 견줄 것이
///   아니고, 시험을 돌리는 셸에 따라 갈리면 안 된다
fn same_write(
    f: &Fence,
    wire: &mut Wire,
    (root, tracker): (&Path, &Path),
    n: u64,
    (method, params, argv, want): (&str, &Value, &[&str], Want),
) -> Value {
    let ctx = format!("{method} {params} / moai {argv:?}");
    let moai = tracker.join(".moai");
    let actor = match params.get("actor") {
        Some(given) => given.as_str().map(str::to_string),
        None => Some(ACTOR.to_string()),
    };
    let lang = params.get("lang").and_then(Value::as_str).unwrap_or("en");
    let before = files_of(&moai);

    let mut all = vec!["-C".to_string(), root.display().to_string(), "--json".to_string()];
    if let Some(actor) = &actor {
        all.extend(["--user".to_string(), actor.clone()]);
    }
    all.extend(argv.iter().map(|a| a.to_string()));
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    let out = f.cmd(&all).env("MOAI_LANG", lang).env_remove("CLAUDE_CODE_SESSION_ID").output().unwrap();
    let cli_after = files_of(&moai);
    put_back(&moai, &before);

    let mut sent = params.clone();
    sent["project"] = json!(root.to_str().unwrap());
    if params.get("actor").is_none() {
        sent["actor"] = json!(ACTOR);
    }
    let text = wire.call_text(n, method, sent);
    let reply: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("답이 JSON 이 아니다: {e} — {text:?}"));
    let got_after = files_of(&moai);

    // 새 줄 — 데몬의 id 를 CLI 의 것으로 바꿔 견준다.
    let (old, theirs, mine) = (ids_of(&before), ids_of(&cli_after), ids_of(&got_after));
    let theirs: Vec<&String> = theirs.difference(&old).collect();
    let mine: Vec<&String> = mine.difference(&old).collect();
    assert_eq!(theirs.len(), mine.len(), "새 줄의 수가 다르다 — {ctx}");
    assert!(theirs.len() <= 1, "새 줄이 여럿이면 짝을 못 짓는다 — {ctx}");
    let swap = |s: &str| match (mine.first(), theirs.first()) {
        (Some(m), Some(t)) => s.replace(m.as_str(), t.as_str()),
        _ => s.to_string(),
    };
    let made = !theirs.is_empty();
    let canon = |files: &std::collections::BTreeMap<String, Vec<u8>>, swapped: bool| {
        files
            .iter()
            .map(|(k, v)| {
                let text = String::from_utf8_lossy(v).into_owned();
                let mut text = if swapped { swap(&text) } else { text };
                if made && k == "issues.jsonl" {
                    let mut lines: Vec<&str> = text.lines().collect();
                    lines.sort_unstable();
                    text = lines.join("\n");
                }
                (k.clone(), text)
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    same_files(&canon(&got_after, true), &canon(&cli_after, false), &ctx);

    let code = out.status.code().expect("CLI 가 시그널로 끝났다");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    if want == Want::Refused {
        assert_eq!(code, 2, "clap 이 거절할 줄 알았다 — {ctx}\n{stderr}");
        assert_eq!(reply["error"]["code"], -32602, "{ctx}: {reply}");
        assert_eq!(cli_after, before, "clap 에 거절된 CLI 가 썼다 — {ctx}");
        assert_eq!(got_after, before, "-32602 로 거절한 데몬이 썼다 — {ctx}");
        return reply;
    }
    assert_ne!(code, 2, "CLI 를 clap 이 거절했다 — 표가 틀렸다: {ctx}\n{stderr}");
    if stdout.is_empty() {
        assert_eq!(code, 1, "{ctx}\n{stderr}");
        same_failure(&reply, &swap(&stderr), &ctx);
        return reply;
    }
    let result = &reply["result"];
    assert!(result.is_object(), "CLI 는 썼는데 데몬은 넘어졌다 — {ctx}: {reply}");
    let lines: Vec<&str> = stdout.lines().collect();
    let printed = if lines.len() == 1 { lines[0].to_string() } else { format!("[{}]", lines.join(",")) };
    assert_eq!(swap(&carried_value(&text)), printed, "value 의 바이트 ≠ CLI stdout — {ctx}");
    let said: String = result["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("messages 가 배열이 아니다 — {ctx}"))
        .iter()
        .map(|m| format!("{}\n", m.as_str().unwrap()))
        .collect();
    assert_eq!(swap(&said), stderr, "messages ≠ stderr — {ctx}");
    assert_eq!(result["exit"], json!(code), "exit ≠ 종료 코드 — {ctx}");
    assert_eq!(result["partial"], json!(code != 0), "partial ≠ 종료 코드 — {ctx}");
    reply
}

/// CLI 로 하나를 만들고 그 id 를 — `add -q` 는 id 만 찍는다.
fn made(f: &Fence, root: &Path, args: &[&str]) -> String {
    let mut all = vec!["-C", root.to_str().unwrap(), "--user", ACTOR];
    all.extend_from_slice(args);
    all.push("-q");
    let out = f.run(&all);
    assert!(out.status.success(), "moai {all:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// **쓰기 일곱이 데몬으로도 CLI 와 같다**(moai-3hgm.pmc) — 읽기의 같음 시험(`the_read_methods_answer_as_the_cli_does`)의
/// 쓰기 판이다. 같은 시작 트래커에서 같은 쓰기를 `moai -C <root> --json …` 와 소켓의 메서드로 하고 [`same_write`] 가
/// 스냅샷·저널 바이트, `value` 바이트, `messages`, `exit` 를 견준다. 표는 차례대로 돌아 앞의 쓰기가 뒤의 시작 트래커다.
///
/// 판마다 CLI 가 다르게 말하는 갈래를 든다 — 종류 넷과 태그·본문의 `add`, 없는 에픽 밑(알림), 남의 줄·담당 없는 줄의
/// 집기(`take` 와 없이), `from` 에 진 집기와 미루기(부분 실패), 없는 id 가 섞인 것, 본문을 지우는 `edit`, 명령이 넘어진
/// 것(없는 칸·우선순위 7·없는 id·`no_actor`), clap 이 거절하는 것(`-32602`), 한국어 말.
#[test]
fn the_write_methods_write_what_the_cli_writes() {
    let f = Fence::new();
    let root = f.project("proj");
    let alpha = id_titled(&f, &root, "alpha");
    let beta = id_titled(&f, &root, "beta");
    let epic = made(&f, &root, &["epic", "add", "Storage"]);
    let ms = made(&f, &root, &["milestone", "add", "v1"]);
    let member = made(&f, &root, &["add", "member", "-e", &epic]);
    let theirs = made(&f, &root, &["add", "theirs", "-a", "Lee (lee@example.com)"]);
    let unowned = made(&f, &root, &["add", "unowned", "-a", "none"]);
    let bodied = made(&f, &root, &["add", "bodied", "-b", "a body\nsecond line", "-t", "x,y"]);
    let later = made(&f, &root, &["add", "later"]);
    let (blocker, blocked) = (made(&f, &root, &["add", "blocker"]), made(&f, &root, &["add", "blocked"]));
    let (doomed, doomed_too) = (made(&f, &root, &["add", "doomed"]), made(&f, &root, &["add", "doomed too"]));
    let (_daemon, mut wire) = served(&f);
    let zzzz = "proj-zzzz";
    use Want::{Refused, Same};

    let cases: Vec<(&str, Value, Vec<&str>, Want)> = vec![
        // add — 종류 넷, 태그·본문·우선순위, 칸·담당·날, 에픽·부모 밑, 없는 에픽 밑(알림).
        ("add", json!({ "title": "plain" }), vec!["add", "plain"], Same),
        ("add", json!({ "kind": "issue", "title": "an issue" }), vec!["issue", "add", "an issue"], Same),
        ("add", json!({ "kind": "epic", "title": "Epic two" }), vec!["epic", "add", "Epic two"], Same),
        ("add", json!({ "kind": "milestone", "title": "v2" }), vec!["milestone", "add", "v2"], Same),
        (
            "add",
            json!({ "kind": "backlog", "title": "an idea", "body": "why\nand how" }),
            vec!["backlog", "add", "an idea", "-b", "why\nand how"],
            Same,
        ),
        (
            "add",
            json!({ "title": "tagged", "tag": "x,y", "priority": 0, "body": "line one\nline two" }),
            vec!["add", "tagged", "-t", "x,y", "-p", "0", "-b", "line one\nline two"],
            Same,
        ),
        (
            "add",
            json!({ "title": "tagged twice", "tag": ["b", "a,c"] }),
            vec!["add", "tagged twice", "-t", "b", "-t", "a,c"],
            Same,
        ),
        (
            "add",
            json!({ "title": "in the epic", "epic": epic, "status": "review", "assignee": "none",
                    "start": "2026-11-10", "due": "2026-11-20" }),
            vec![
                "add",
                "in the epic",
                "-e",
                &epic,
                "-s",
                "review",
                "-a",
                "none",
                "--start",
                "2026-11-10",
                "--due",
                "2026-11-20",
            ],
            Same,
        ),
        ("add", json!({ "title": "a child", "parent": alpha }), vec!["add", "a child", "--parent", &alpha], Same),
        (
            "add",
            json!({ "title": "in a release", "milestone": ms }),
            vec!["add", "in a release", "--milestone", &ms],
            Same,
        ),
        ("add", json!({ "title": "lost", "epic": zzzz }), vec!["add", "lost", "-e", zzzz], Same),
        (
            "add",
            json!({ "title": "그의 일", "assignee": "Lee (lee@example.com)", "lang": "ko" }),
            vec!["add", "그의 일", "-a", "Lee (lee@example.com)"],
            Same,
        ),
        ("add", json!({ "title": "p7", "priority": 7 }), vec!["add", "p7", "-p", "7"], Same),
        ("add", json!({}), vec!["add"], Same),
        ("add", json!({ "title": "p256", "priority": 256 }), vec!["add", "p256", "-p", "256"], Refused),
        ("add", json!({ "title": "high", "priority": "high" }), vec!["add", "high", "-p", "high"], Refused),
        // mv — 집기, `from` 에 진 집기, 남의 줄(말), 담당 없는 줄을 `take`, 남의 줄을 `take`, 없는 id 가 섞인 것, 없는 칸.
        (
            "mv",
            json!({ "id": alpha, "to": "in_progress", "from": "todo" }),
            vec!["mv", &alpha, "in_progress", "--from", "todo"],
            Same,
        ),
        (
            "mv",
            json!({ "id": alpha, "to": "review", "from": "todo" }),
            vec!["mv", &alpha, "review", "--from", "todo"],
            Same,
        ),
        ("mv", json!({ "id": theirs, "to": "in_progress" }), vec!["mv", &theirs, "in_progress"], Same),
        (
            "mv",
            json!({ "id": unowned, "to": "in_progress", "from": "todo", "take": true, "msg": "Kim said yes", "lang": "ko" }),
            vec!["mv", &unowned, "in_progress", "--from", "todo", "--take", "-m", "Kim said yes"],
            Same,
        ),
        (
            "mv",
            json!({ "id": theirs, "to": "review", "take": true, "msg": "Lee said yes" }),
            vec!["mv", &theirs, "review", "--take", "-m", "Lee said yes"],
            Same,
        ),
        ("mv", json!({ "id": [beta, zzzz], "to": "done" }), vec!["mv", &beta, zzzz, "done"], Same),
        (
            "mv",
            json!({ "id": member, "to": "done", "msg": "finished" }),
            vec!["mv", &member, "done", "-m", "finished"],
            Same,
        ),
        ("mv", json!({ "id": beta, "to": "doing" }), vec!["mv", &beta, "doing"], Same),
        // edit — 본문 지우기, 태그 더하고 빼기, 제목·우선순위·날·마일스톤, 그것을 걷기, 바뀐 것 없음, 없는 id, clap 의 짝 규칙.
        ("edit", json!({ "id": bodied, "body": "" }), vec!["edit", &bodied, "-b", ""], Same),
        (
            "edit",
            json!({ "id": bodied, "tag": "z", "untag": ["x"] }),
            vec!["edit", &bodied, "-t", "z", "--untag", "x"],
            Same,
        ),
        (
            "edit",
            json!({ "id": bodied, "title": "renamed", "priority": 3, "due": "2026-12-01", "milestone": ms }),
            vec!["edit", &bodied, "--title", "renamed", "-p", "3", "--due", "2026-12-01", "--milestone", &ms],
            Same,
        ),
        (
            "edit",
            json!({ "id": bodied, "due": "none", "milestone": "none" }),
            vec!["edit", &bodied, "--due", "none", "--milestone", "none"],
            Same,
        ),
        ("edit", json!({ "id": bodied, "tag": "z" }), vec!["edit", &bodied, "-t", "z"], Same),
        ("edit", json!({ "id": bodied }), vec!["edit", &bodied], Same),
        ("edit", json!({ "id": zzzz, "title": "x" }), vec!["edit", zzzz, "--title", "x"], Same),
        (
            "edit",
            json!({ "id": bodied, "shift": 1, "start": "+1" }),
            vec!["edit", &bodied, "--shift", "1", "--start", "+1"],
            Refused,
        ),
        // note — 한 줄, 여러 줄, 글 없이, 없는 id, 모르는 사람.
        (
            "note",
            json!({ "id": alpha, "text": "found it in the lexer" }),
            vec!["note", &alpha, "found it in the lexer"],
            Same,
        ),
        ("note", json!({ "id": alpha, "text": "two\nlines", "lang": "ko" }), vec!["note", &alpha, "two\nlines"], Same),
        ("note", json!({ "id": alpha }), vec!["note", &alpha], Same),
        ("note", json!({ "id": zzzz, "text": "x" }), vec!["note", zzzz, "x"], Same),
        ("note", json!({ "id": alpha, "text": "who", "actor": null }), vec!["note", &alpha, "who"], Same),
        // defer — 미루기와 되돌리기, `from` 에 진 미루기.
        ("defer", json!({ "id": later, "msg": "next quarter" }), vec!["defer", &later, "-m", "next quarter"], Same),
        ("defer", json!({ "id": later, "undo": true }), vec!["defer", &later, "--undo"], Same),
        ("defer", json!({ "id": later, "from": "review" }), vec!["defer", &later, "--from", "review"], Same),
        // link — 막기·풀기·없는 id.
        ("link", json!({ "id": blocker, "blocks": blocked }), vec!["link", &blocker, "--blocks", &blocked], Same),
        ("link", json!({ "id": blocker, "unblocks": [blocked] }), vec!["link", &blocker, "--unblocks", &blocked], Same),
        ("link", json!({ "id": blocker, "blocks": zzzz }), vec!["link", &blocker, "--blocks", zzzz], Same),
        // rm — 하나, 없는 id 가 섞인 것, 없는 못 읽는 줄.
        ("rm", json!({ "id": doomed }), vec!["rm", &doomed], Same),
        ("rm", json!({ "id": [doomed_too, zzzz] }), vec!["rm", &doomed_too, zzzz], Same),
        ("rm", json!({ "line": 99 }), vec!["rm", "--line", "99"], Same),
    ];
    // 표가 재는 갈래가 실제로 섰는지 — 하나라도 안 서면 그 갈래는 견주지 않은 것이다.
    let mut seen = std::collections::BTreeSet::new();
    for (n, (method, params, argv, want)) in cases.iter().enumerate() {
        let reply = same_write(&f, &mut wire, (&root, &root), n as u64 + 1, (method, params, argv, *want));
        let result = &reply["result"];
        if result["exit"] == 1 {
            seen.insert("partial");
        }
        if result["messages"].as_array().is_some_and(|m| !m.is_empty()) {
            seen.insert("messages");
        }
        if reply["error"]["code"] == -32000 {
            seen.insert("failed");
        }
        if reply["error"]["code"] == -32602 {
            seen.insert("invalid params");
        }
    }
    for branch in ["failed", "invalid params", "messages", "partial"] {
        assert!(seen.contains(branch), "표가 재려던 갈래 `{branch}` 가 한 번도 안 섰다 — {seen:?}");
    }
}

/// `.moai/journal` 의 줄 전부 — 사람마다의 파일을 이어서.
fn journal_lines(moai: &Path) -> Vec<String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(moai.join("journal")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    files
        .iter()
        .flat_map(|p| std::fs::read_to_string(p).unwrap().lines().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

/// **데몬의 연결 여럿과 CLI 여럿이 한 트래커에 함께 써도 잃는 줄이 없다**(moai-3hgm.pmc) — `CLAUDE.md` 의 "동시 쓰기
/// 시험을 지우지 않는다" 에 데몬이 더하는 하나다. 데몬은 한 프로세스에서 쓰기를 스레드로 함께 돌린다. `flock` 은 열린
/// 파일마다라(`store` 의 `Lock::inside` 는 쓰기마다 락 파일을 새로 연다) 데몬의 스레드끼리도, 데몬과 CLI 프로세스도
/// 서로를 막는다 — 한 fd 를 나눠 쥐는 판이면 서로를 안 막아, 마지막에 rename 한 쪽이 앞의 줄을 지운다.
///
/// 끝난 뒤 만든 줄과 노트가 하나씩 다 있고, 스냅샷 줄 수와 저널 줄 수가 쓴 만큼 늘었고, id 가 안 겹친다. 쓰기마다의
/// 성패는 모았다가 끝에 댄다 — 진 쪽(`locked`)이 있으면 그것이 누구였는지 이름으로 보이게.
#[test]
fn daemon_connections_and_cli_processes_writing_at_once_lose_nothing() {
    const WIRES: usize = 6;
    const PROCS: usize = 4;
    const ROUNDS: usize = 5;
    let f = Fence::new();
    let root = f.project("proj");
    let alpha = id_titled(&f, &root, "alpha");
    let moai = root.join(".moai");
    let snapshot = || std::fs::read_to_string(moai.join("issues.jsonl")).unwrap();
    let (rows_before, journal_before) = (snapshot().lines().count(), journal_lines(&moai).len());
    let (_daemon, _) = f.start();
    // 연결은 먼저 다 연다 — 스레드 안에서 열다 넘어지면 남은 스레드가 출발선에서 영영 기다린다.
    let wires: Vec<Wire> = (0..WIRES).map(|_| Wire::open(&f.socket())).collect();
    let start = std::sync::Barrier::new(WIRES + PROCS);

    let lost: Vec<String> = std::thread::scope(|s| {
        let mut runs = Vec::new();
        for (w, mut wire) in wires.into_iter().enumerate() {
            let (start, root, alpha) = (&start, &root, &alpha);
            runs.push(s.spawn(move || {
                start.wait();
                let mut lost = Vec::new();
                for r in 0..ROUNDS {
                    for (n, method, params) in [
                        (2 * r, "add", json!({ "actor": ACTOR, "title": format!("wire {w} row {r}") })),
                        (
                            2 * r + 1,
                            "note",
                            json!({ "actor": ACTOR, "id": alpha, "text": format!("wire {w} note {r}") }),
                        ),
                    ] {
                        let reply = write(&mut wire, root, n as u64, method, params.clone());
                        if reply["result"]["exit"] != 0 {
                            lost.push(format!("wire {w} {method} {params}: {reply}"));
                        }
                    }
                }
                lost
            }));
        }
        for p in 0..PROCS {
            let (start, root, alpha, f) = (&start, &root, &alpha, &f);
            runs.push(s.spawn(move || {
                start.wait();
                let mut lost = Vec::new();
                for r in 0..ROUNDS {
                    let (title, text) = (format!("cli {p} row {r}"), format!("cli {p} note {r}"));
                    for args in [vec!["add", title.as_str()], vec!["note", alpha.as_str(), text.as_str()]] {
                        let mut all = vec!["-C", root.to_str().unwrap(), "--json", "--user", ACTOR];
                        all.extend_from_slice(&args);
                        let out = f.run(&all);
                        if !out.status.success() {
                            lost.push(format!("cli {p} {args:?}: {}", String::from_utf8_lossy(&out.stderr)));
                        }
                    }
                }
                lost
            }));
        }
        runs.into_iter().flat_map(|r| r.join().unwrap()).collect()
    });
    assert!(lost.is_empty(), "함께 친 쓰기가 넘어졌다:\n{}", lost.join("\n"));

    let writers: Vec<String> =
        (0..WIRES).map(|w| format!("wire {w}")).chain((0..PROCS).map(|p| format!("cli {p}"))).collect();
    let rows: Vec<Value> = snapshot().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let journal = journal_lines(&moai);
    let added = writers.len() * ROUNDS;
    assert_eq!(rows.len(), rows_before + added, "{added} 줄을 함께 만들었는데 스냅샷이 {}줄이다", rows.len());
    assert_eq!(journal.len(), journal_before + 2 * added, "저널 줄 수가 쓴 만큼 안 늘었다");
    let ids: std::collections::BTreeSet<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), rows.len(), "id 가 겹쳤다");
    let notes: Vec<Value> = journal.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    for who in &writers {
        for r in 0..ROUNDS {
            let (title, text) = (format!("{who} row {r}"), format!("{who} note {r}"));
            let made = rows.iter().filter(|row| row["title"] == title.as_str()).count();
            assert_eq!(made, 1, "`{title}` 이 스냅샷에 {made}번 섰다");
            let noted = notes.iter().filter(|n| n["kind"] == "note" && n["text"] == text.as_str()).count();
            assert_eq!(noted, 1, "`{text}` 가 저널에 {noted}번 섰다");
            let created = notes.iter().filter(|n| n["kind"] == "create" && n["title"] == title.as_str()).count();
            assert_eq!(created, 1, "`{title}` 의 create 줄이 저널에 {created}번 섰다");
        }
    }
}

/// **락을 시한(5초)보다 오래 쥔 채 데몬에 쓰면 `-32000` 에 CLI 의 `locked` 다** — 말없이 넘어가지 않는다. 같은 쥔 락
/// 아래에서 CLI 와 데몬을 함께 돌려(시험이 5초 한 번으로 끝나게) 둘 다 물러나는지, 코드·말·앞선 알림이 같은지, 아무것도
/// 안 썼는지 본다. 락을 놓으면 데몬의 다음 쓰기는 선다 — 물러난 쓰기가 데몬에 쥔 것을 남기지 않는다.
#[test]
fn a_write_held_past_the_lock_timeout_is_locked_as_on_the_cli() {
    use fs2::FileExt;
    let f = Fence::new();
    let root = f.project("proj");
    let alpha = id_titled(&f, &root, "alpha");
    let (_daemon, mut wire) = served(&f);
    let moai = root.join(".moai");
    let before = files_of(&moai);
    let lock = std::fs::OpenOptions::new().read(true).write(true).open(moai.join("lock")).unwrap();
    lock.lock_exclusive().unwrap();

    let asked = std::time::Instant::now();
    let cli = f
        .cmd(&["-C", root.to_str().unwrap(), "--json", "--user", ACTOR, "note", &alpha, "held"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let reply = write(&mut wire, &root, 1, "note", json!({ "actor": ACTOR, "id": alpha, "text": "held" }));
    let out = cli.wait_with_output().unwrap();
    let waited = asked.elapsed();
    FileExt::unlock(&lock).unwrap();

    assert!(waited >= Duration::from_secs(5), "락 시한(5초) 전에 {waited:?} 만에 답했다 — 락을 안 기다렸다: {reply}");
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "쥔 락 아래에서 CLI 가 썼다");
    same_failure(&reply, &String::from_utf8(out.stderr).unwrap(), "note under a held lock");
    assert_eq!(reply["error"]["data"]["code"], "locked", "{reply}");
    same_files(&texts(files_of(&moai)), &texts(before), "물러난 쓰기가 무엇인가를 썼다");

    let after = wrote(&mut wire, &root, 2, "note", json!({ "actor": ACTOR, "id": alpha, "text": "after" }));
    assert_eq!(after["value"]["text"], "after", "{after}");
}

fn texts(files: std::collections::BTreeMap<String, Vec<u8>>) -> std::collections::BTreeMap<String, String> {
    files.into_iter().map(|(k, v)| (k, String::from_utf8_lossy(&v).into_owned())).collect()
}

/// 딸린 워크트리 하나를 든 저장소 — 주 체크아웃 `main` 에 트래커를 심어 줄 둘(`alpha`, 남의 줄 `theirs`)과 함께 커밋하고
/// `side` 로 갈라 낸다. 워크트리의 `.moai` 는 그 커밋의 것이다. 아무것도 걸어 두지 않는다 — `(main, side, alpha, theirs)`.
fn with_a_worktree(f: &Fence) -> (PathBuf, PathBuf, String, String) {
    let main = f.0.join("main");
    std::fs::create_dir(&main).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=Tester", "-c", "user.email=tester@example.com"])
            .args(args)
            .current_dir(&main)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    let out = f.run(&["-C", main.to_str().unwrap(), "init"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let alpha = made(f, &main, &["add", "alpha"]);
    let theirs = made(f, &main, &["add", "theirs", "-a", "Lee (lee@example.com)"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    let side = f.0.join("side");
    git(&["worktree", "add", "-q", "-b", "side", side.to_str().unwrap()]);
    (main, side, alpha, theirs)
}

/// **걸어 둔 워크트리를 `project` 로 주면 루트의 트래커에 쓰고, 그 자리를 `messages` 에 댄다** — `moai -C <워크트리>`
/// 의 CLI 가 stderr 에 내는 그 한 줄이다(`lib::redirected`, moai-y7go). 데몬은 걸어 둔 자리만 받는다 — 루트만 걸어 둔
/// 채 워크트리를 대면 `not_registered` 이고 아무것도 안 쓴다. 워크트리의 `.moai` 는 갈라져 나온 그대로다.
#[test]
fn a_write_through_a_registered_worktree_lands_in_the_main_checkout_as_on_the_cli() {
    let f = Fence::new();
    let (main, side, alpha, theirs) = with_a_worktree(&f);
    let out = f.run(&["project", "add", main.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (_daemon, mut wire) = served(&f);
    let (main_moai, side_moai) = (main.join(".moai"), side.join(".moai"));
    let side_before = files_of(&side_moai);

    // 루트만 걸어 두었다 — 워크트리는 걸어 둔 자리가 아니다.
    let main_before = files_of(&main_moai);
    let refused = write(&mut wire, &side, 1, "note", json!({ "actor": ACTOR, "id": alpha, "text": "not yet" }));
    assert_eq!(refused["error"]["data"]["code"], "not_registered", "{refused}");
    assert_eq!(files_of(&main_moai), main_before, "걸어 두지 않은 워크트리로 썼다");

    let out = f.run(&["project", "add", side.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let to_root = main_moai.display().to_string();
    for (n, (method, params, argv)) in [
        ("note", json!({ "id": alpha, "text": "from the worktree" }), vec!["note", &alpha, "from the worktree"]),
        ("add", json!({ "title": "made in the worktree" }), vec!["add", "made in the worktree"]),
        ("mv", json!({ "id": theirs, "to": "in_progress" }), vec!["mv", &theirs, "in_progress"]),
        (
            "edit",
            json!({ "id": alpha, "title": "alpha two", "lang": "ko" }),
            vec!["edit", &alpha, "--title", "alpha two"],
        ),
    ]
    .iter()
    .enumerate()
    {
        let reply = same_write(&f, &mut wire, (&side, &main), n as u64 + 2, (method, params, argv, Want::Same));
        let messages = reply["result"]["messages"].as_array().unwrap();
        assert!(
            messages.iter().any(|m| m.as_str().unwrap().contains(&to_root)),
            "{method}: 루트로 옮겨 쓴 자리를 안 댔다 — {reply}"
        );
    }
    assert_eq!(files_of(&side_moai), side_before, "워크트리의 .moai 를 고쳤다");
}

/// **데몬을 띄운 셸의 `MOAI_HERE=1` 은 요청에 안 선다**(리뷰 moai-3hgm.puw 3번) — 데몬의 `MOAI_ACTOR` 를 안 읽는 것과
/// 같은 자다(moai-3hgm.k7s). 요청에는 그 손잡이를 줄 길이 없으니, 걸어 둔 워크트리로 간 요청은 그것 없이 친 CLI 처럼
/// 루트의 트래커를 읽고 쓰고, 따라 칠 명령(`rm` 의 `line` 이 내는 것, `edit` 의 날짜 되돌리기)에도 `MOAI_HERE=1` 이
/// 안 붙는다. 읽던 판은 워크트리의 제 `.moai` 에 써 옮김이 막으려던 갈라진 스냅샷을 지었고, `rm` 의 거절문이
/// `MOAI_HERE=1 moai -C <워크트리> rm --line 1` 을 댔다.
#[test]
fn the_daemons_own_moai_here_does_not_keep_a_request_on_the_worktrees_tracker() {
    let f = Fence::new();
    let (main, side, alpha, _) = with_a_worktree(&f);
    for dir in [&main, &side] {
        let out = f.run(&["project", "add", dir.to_str().unwrap()]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let (_daemon, _) = f.start_with(&[("MOAI_HERE", "1")]);
    let mut wire = Wire::open(&f.socket());
    let side_moai = side.join(".moai");
    let side_before = files_of(&side_moai);
    for (n, (method, params, argv)) in [
        ("note", json!({ "id": alpha, "text": "from the worktree" }), vec!["note", &alpha, "from the worktree"]),
        ("add", json!({ "title": "made in the worktree" }), vec!["add", "made in the worktree"]),
        ("edit", json!({ "id": alpha, "due": "2026-12-01" }), vec!["edit", &alpha, "--due", "2026-12-01"]),
        // 보여 준 해시 없이 `yes` 만 준 지우기는 넘어지며 따라 칠 보기 명령을 댄다(`rm::to_type`).
        ("rm", json!({ "line": 1, "yes": true }), vec!["rm", "--line", "1", "--yes"]),
    ]
    .iter()
    .enumerate()
    {
        let reply = same_write(&f, &mut wire, (&side, &main), n as u64 + 1, (method, params, argv, Want::Same));
        assert!(!reply.to_string().contains("MOAI_HERE"), "{method}: 요청이 안 바란 손잡이를 댔다 — {reply}");
    }
    assert_eq!(files_of(&side_moai), side_before, "데몬의 MOAI_HERE 로 워크트리의 .moai 에 썼다");

    // 제 `.moai` 가 없는 딸린 워크트리 — 걸어 둔 프로젝트를 푸는 자리(`daemon::project::resolve`)는 통을 걸기 전이라 따로
    // 섰다. 손잡이를 따르던 판은 요청이 옮겨 갈 이 자리를 `not_a_project` 로 거절했다.
    let bare = f.0.join("bare");
    let out = Command::new("git")
        .args(["worktree", "add", "-q", "--detach", bare.to_str().unwrap()])
        .current_dir(&main)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(out.status.success(), "git worktree add: {}", String::from_utf8_lossy(&out.stderr));
    std::fs::remove_dir_all(bare.join(".moai")).unwrap();
    let out = f.run(&["project", "add", bare.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (params, argv) =
        (json!({ "id": alpha, "text": "from a bare worktree" }), ["note", &alpha, "from a bare worktree"]);
    same_write(&f, &mut wire, (&bare, &main), 9, ("note", &params, &argv, Want::Same));
    assert!(!bare.join(".moai").exists(), "제 .moai 없는 워크트리에 트래커를 세웠다");
}

/// 그 프로세스가 이 파일을 열어 쥐고 있나 — `/proc/<pid>/fd` 의 장치·아이노드로 견준다(이름이 아니라 파일로).
#[cfg(target_os = "linux")]
fn opened(pid: u64, file: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::metadata(e.path()).ok())
        .any(|m| m.dev() == file.dev() && m.ino() == file.ino())
}

/// **멈추는 데몬은 락을 기다리던 쓰기를 끝내고 그 답을 보낸다**(moai-3hgm.uz0, 사람의 결정 B) — uz0 의 시험은 시험만의
/// 메서드로 늦췄고, 여기는 실제 쓰기 메서드를 시험이 쥔 락으로 늦춘다. 데몬이 그 락 파일을 연 것(`/proc/<pid>/fd`)을 본
/// 뒤에 멈추기를 보낸다 — 그 쓰기는 들어와 돌고 있다. 소켓 파일이 지워진 뒤에도 데몬이 살아 있는지, 그때 이미 열려 있던
/// 다른 연결의 새 쓰기는 돌지 않고 `stopping` 인지 본다. 락을 놓으면 기다리던 쓰기가 서서 답이 오고, 데몬은 0 으로
/// 끝난다. 쥐는 것은 1초 안팎이라 `store` 의 락 기다림(5초) 안에 든다.
#[cfg(target_os = "linux")]
#[test]
fn a_write_waiting_on_the_lock_when_the_daemon_stops_still_finishes_and_answers() {
    write_on_the_lock_outlives_the_stop(|bye, pid| {
        let stopped = bye.call(1, "daemon.shutdown", json!({}));
        assert_eq!(stopped["result"]["pid"], pid, "{stopped}");
    });
}

/// SIGTERM 도 같다 — `systemctl stop` 이 멈추는 길이다. 돌던 쓰기는 끝까지 가고, 데몬은 0 으로 끝난다(moai-3hgm.vjv).
#[cfg(target_os = "linux")]
#[test]
fn a_write_waiting_on_the_lock_when_sigterm_arrives_still_finishes_and_answers() {
    write_on_the_lock_outlives_the_stop(|_, pid| {
        // SAFETY: pid 는 이 시험이 방금 띄운 자식이고 신호는 유효하다.
        assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) }, 0);
    });
}

#[cfg(target_os = "linux")]
fn write_on_the_lock_outlives_the_stop(stop: impl FnOnce(&mut Wire, u64)) {
    use fs2::FileExt;
    let f = Fence::new();
    let root = f.project("proj");
    let project = root.to_str().unwrap();
    let alpha = id_titled(&f, &root, "alpha");
    let (daemon, first) = f.start();
    let pid = first["pid"].as_u64().unwrap();
    // 연결은 먼저 다 연다 — 멈추기가 시작되면 소켓 파일이 지워져 새로 못 붙는다.
    let mut writer = Wire::open(&f.socket());
    let mut idle = Wire::open(&f.socket());
    let mut bye = Wire::open(&f.socket());
    assert_eq!(idle.call(1, "ping", json!({}))["result"], json!({}), "쉬는 연결이 연결 스레드에 안 들었다");
    let moai = root.join(".moai");
    let lock = std::fs::OpenOptions::new().read(true).write(true).open(moai.join("lock")).unwrap();
    lock.lock_exclusive().unwrap();
    let held = lock.metadata().unwrap();

    let note = |id: u64, text: &str| {
        let params = json!({ "project": project, "actor": ACTOR, "id": alpha, "text": text });
        format!("{}\n", json!({ "jsonrpc": "2.0", "id": id, "method": "note", "params": params }))
    };
    writer.writer.write_all(note(1, "written while stopping").as_bytes()).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(3);
    while !opened(pid, &held) {
        assert!(std::time::Instant::now() < until, "데몬이 쥔 락을 3초 안에 안 열었다 — 쓰기가 안 돌았다");
        std::thread::sleep(Duration::from_millis(5));
    }

    stop(&mut bye, pid);
    let asked = std::time::Instant::now();
    while f.socket().exists() {
        assert!(asked.elapsed() < Duration::from_secs(3), "멈추는 데몬이 소켓 파일을 안 지웠다");
        std::thread::sleep(Duration::from_millis(10));
    }
    // 문(`Work`)은 소켓 파일을 지우고 듣기를 닫은 바로 뒤에 닫힌다 — 그 틈을 넉넉히 넘긴다.
    std::thread::sleep(Duration::from_millis(200));
    let mut daemon = daemon;
    assert!(daemon.0.try_wait().unwrap().is_none(), "락을 기다리던 쓰기를 두고 데몬이 끝났다");
    let late = idle.send_raw(note(2, "sent after the stop").as_bytes());
    assert_eq!(late["error"]["data"]["code"], "stopping", "멈추는 데몬이 새 쓰기를 들였다 — {late}");
    assert_eq!(late["id"], 2, "{late}");
    assert!(daemon.0.try_wait().unwrap().is_none(), "락을 기다리던 쓰기를 두고 데몬이 끝났다");

    FileExt::unlock(&lock).unwrap();
    let mut line = String::new();
    writer.reader.read_line(&mut line).unwrap();
    let reply: Value =
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("기다리던 쓰기의 답이 없다: {e} — {line:?}"));
    assert_eq!(reply["id"], 1, "{reply}");
    assert_eq!(reply["result"]["exit"], 0, "{reply}");
    assert_eq!(reply["result"]["value"]["text"], "written while stopping", "{reply}");
    assert!(daemon.wait().success(), "멈춘 데몬이 0 으로 끝나지 않았다");

    let journal = journal_lines(&moai);
    assert!(journal.iter().any(|l| l.contains("written while stopping")), "답한 쓰기가 저널에 없다");
    assert!(!journal.iter().any(|l| l.contains("sent after the stop")), "`stopping` 으로 거절한 쓰기가 섰다");
}

/// `moai daemon install·uninstall` — **가짜 관리자만 있는 PATH 로 돈다**(moai-upq5). 진짜 `systemctl`, `launchctl` 에 닿으면
/// 시험이 사람의 로그인에 서비스를 남기므로, 이 모듈의 명령은 모두 [`Managed::cmd`] 를 지난다.
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod service {
    use super::*;

    const LINUX: bool = cfg!(target_os = "linux");
    const TOOL: &str = if LINUX { "systemctl" } else { "launchctl" };

    #[derive(Clone, Copy, PartialEq)]
    enum Manager {
        /// 등록을 받아 실제 데몬을 띄운다.
        Starts,
        /// 등록을 받지만 데몬은 안 뜬다.
        Idles,
        /// 등록을 거절한다.
        Refuses,
        /// PATH 에 없다.
        Missing,
    }

    struct Managed(Fence);

    impl Managed {
        fn new(manager: Manager) -> Managed {
            let f = Fence::new();
            let bin = f.0.join("bin");
            std::fs::create_dir(&bin).unwrap();
            if manager != Manager::Missing {
                let (calls, loaded) = (f.0.join("calls"), f.0.join("loaded"));
                let (calls, loaded) = (calls.display(), loaded.display());
                let start = match manager {
                    Manager::Starts => format!(": > '{loaded}'; '{BIN}' daemon start --json >/dev/null 2>&1 &\nexit 0"),
                    Manager::Idles => format!(": > '{loaded}'; exit 0"),
                    _ => "echo 'Failed to connect to bus' >&2; exit 1".to_string(),
                };
                let body = format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{calls}'\ncase \"$*\" in\n\
                     *'enable --now'* | bootstrap*)\n{start}\n;;\n\
                     *'disable --now'* | bootout*)\n'{BIN}' daemon stop >/dev/null 2>&1; /bin/rm -f '{loaded}'; exit 0\n;;\n\
                     print*)\n[ -e '{loaded}' ]; exit $?\n;;\nesac\nexit 0\n"
                );
                // 이 프로세스가 쓴 파일을 바로 띄우면 옆 시험의 fork 와 겹쳐 ETXTBSY 다 — 자식이 옮겨 적게 한다.
                let source = f.0.join("fake.src");
                std::fs::write(&source, body).unwrap();
                let tool = bin.join(TOOL);
                assert!(Command::new("cp").arg(&source).arg(&tool).status().unwrap().success());
                assert!(Command::new("chmod").arg("755").arg(&tool).status().unwrap().success());
            }
            Managed(f)
        }

        fn cmd(&self, args: &[&str]) -> Command {
            let mut cmd = self.0.cmd(args);
            cmd.env("PATH", self.0.0.join("bin"));
            cmd
        }

        fn run(&self, args: &[&str]) -> Output {
            self.cmd(args).output().unwrap()
        }

        fn json(&self, args: &[&str]) -> Value {
            let out = self.run(args);
            assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            serde_json::from_slice(&out.stdout).unwrap()
        }

        fn refused(&self, args: &[&str]) -> Value {
            let out = self.run(args);
            assert!(
                !out.status.success(),
                "moai {args:?} 가 넘어지지 않았다: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            serde_json::from_slice(&out.stderr).unwrap()
        }

        fn file(&self) -> PathBuf {
            let home = self.0.0.join("home");
            match LINUX {
                true => home.join(".config/systemd/user/moai-daemon.service"),
                false => home.join("Library/LaunchAgents/com.buzzni.moai-daemon.plist"),
            }
        }

        fn calls(&self) -> String {
            std::fs::read_to_string(self.0.0.join("calls")).unwrap_or_default()
        }

        fn place(&self, text: &str) {
            std::fs::create_dir_all(self.file().parent().unwrap()).unwrap();
            std::fs::write(self.file(), text).unwrap();
        }
    }

    impl Drop for Managed {
        fn drop(&mut self) {
            let _ = self.0.run(&["daemon", "stop"]);
        }
    }

    fn real_bin() -> String {
        std::fs::canonicalize(BIN).unwrap().display().to_string()
    }

    #[test]
    fn install_registers_starts_and_reads_the_same_the_second_time() {
        let m = Managed::new(Manager::Starts);
        let first = m.json(&["daemon", "install", "--json"]);
        assert_eq!(first["installed"], true, "{first}");
        assert_eq!(first["manager"], if LINUX { "systemd" } else { "launchd" }, "{first}");
        assert_eq!(first["service"], m.file().display().to_string(), "{first}");
        assert_eq!(first["exe"], real_bin(), "{first}");
        assert_eq!(first["running"], true, "{first}");
        assert_eq!(first["same_version"], true, "{first}");
        assert!(first["pid"].is_u64(), "{first}");

        let text = std::fs::read_to_string(m.file()).unwrap();
        assert!(text.contains("moai daemon install wrote this file"), "{text}");
        assert!(text.contains(&real_bin()), "{text}");
        // 소켓 디렉터리를 사람이 정했으니(MOAI_CTL_DIR) 서비스도 같은 곳을 본다.
        assert!(text.contains("MOAI_CTL_DIR"), "{text}");
        assert!(text.contains(&m.0.0.display().to_string()), "{text}");
        if LINUX {
            assert!(
                m.calls().contains("--user daemon-reload\n--user enable --now moai-daemon.service\n"),
                "{}",
                m.calls()
            );
        } else {
            assert!(m.calls().contains("bootstrap gui/"), "{}", m.calls());
        }

        let status = m.json(&["daemon", "status", "--json"]);
        assert_eq!(status["service"], m.file().display().to_string(), "{status}");
        assert_eq!(status["running"], true, "{status}");

        let second = m.json(&["daemon", "install", "--json"]);
        assert_eq!(second["installed"], false, "두 번째는 이미 등록된 것이다 — {second}");
        assert_eq!(std::fs::read_to_string(m.file()).unwrap(), text, "두 번째가 파일을 바꿨다");
    }

    /// 서비스는 이 셸의 디렉터리에서 돌지 않는다 — 상대 경로를 그대로 적으면 다른 소켓을 본다.
    #[test]
    fn a_relative_socket_directory_is_written_as_an_absolute_one() {
        let m = Managed::new(Manager::Idles);
        let out = m.cmd(&["daemon", "install"]).env("MOAI_CTL_DIR", "rel").output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = std::fs::read_to_string(m.file()).unwrap();
        let want = m.0.0.join("rel").display().to_string();
        assert!(text.contains(&want), "상대 경로를 그대로 적었다\n{text}");
    }

    /// launchd 는 그룹이 쓸 수 있는 plist 를 안 읽는다 — 사람의 umask 를 따르면 그 자리에서만 등록이 실패한다.
    #[test]
    fn the_service_file_ignores_a_loose_umask() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::os::unix::process::CommandExt as _;
        let m = Managed::new(Manager::Starts);
        let mut cmd = m.cmd(&["daemon", "install", "--json"]);
        // SAFETY: umask(2) is async-signal-safe, so it may run between fork and exec.
        unsafe {
            cmd.pre_exec(|| {
                libc::umask(0o002);
                Ok(())
            });
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let mode = std::fs::metadata(m.file()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644, "{mode:o}");
    }

    #[test]
    fn install_succeeds_when_no_daemon_answers_yet() {
        let m = Managed::new(Manager::Idles);
        let out = m.run(&["daemon", "install"]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(said.contains("It starts when you log in"), "{said}");
        assert!(said.contains("no daemon answers"), "{said}");
        assert!(m.file().exists());
    }

    #[test]
    fn a_file_moai_did_not_write_is_refused_both_ways_and_left_as_it_was() {
        let m = Managed::new(Manager::Starts);
        let mine = format!("[Service]\nExecStart={} daemon start\n", real_bin());
        m.place(&mine);
        for verb in ["install", "uninstall"] {
            let err = m.refused(&["daemon", verb, "--json"]);
            assert_eq!(err["code"], "already_exists", "{verb}: {err}");
            assert_eq!(std::fs::read_to_string(m.file()).unwrap(), mine, "{verb} 가 사람의 파일을 바꿨다");
        }
        assert_eq!(m.calls(), "", "거절했는데 관리자를 불렀다");
        // 파일이 있으면 등록된 것이다 — 손으로 쓴 것도 그렇다.
        assert_eq!(m.json(&["daemon", "status", "--json"])["service"], m.file().display().to_string());
    }

    #[test]
    fn a_service_of_another_moai_is_not_taken_over() {
        let m = Managed::new(Manager::Starts);
        m.json(&["daemon", "install", "--json"]);
        let other = std::fs::read_to_string(m.file()).unwrap().replace(&real_bin(), "/other/place/moai");
        m.place(&other);
        let err = m.refused(&["daemon", "install", "--json"]);
        assert_eq!(err["code"], "already_exists", "{err}");
        assert!(err["error"].as_str().is_some_and(|s| s.contains("uninstall")), "고칠 길을 안 댄다 — {err}");
        assert_eq!(std::fs::read_to_string(m.file()).unwrap(), other);
    }

    #[test]
    fn a_refused_registration_leaves_no_file() {
        let m = Managed::new(Manager::Refuses);
        let err = m.refused(&["daemon", "install", "--json"]);
        assert_eq!(err["code"], "service_failed", "{err}");
        let said = err["error"].as_str().unwrap();
        assert!(said.contains("Failed to connect to bus"), "관리자의 말을 잃었다 — {said}");
        assert!(said.contains("The CLI works without the daemon"), "{said}");
        assert!(!m.file().exists(), "실패했는데 파일이 남아 등록된 것으로 읽힌다");
        assert_eq!(m.json(&["daemon", "status", "--json"]).get("service"), None);
    }

    #[test]
    fn no_service_manager_is_the_same_failure() {
        let m = Managed::new(Manager::Missing);
        let err = m.refused(&["daemon", "install", "--json"]);
        assert_eq!(err["code"], "service_failed", "{err}");
        assert!(!m.file().exists());
    }

    #[test]
    fn uninstall_stops_removes_and_is_quiet_the_second_time() {
        let m = Managed::new(Manager::Starts);
        m.json(&["daemon", "install", "--json"]);
        let gone = m.json(&["daemon", "uninstall", "--json"]);
        assert_eq!(gone["uninstalled"], true, "{gone}");
        assert_eq!(gone["service"], m.file().display().to_string(), "{gone}");
        assert!(!m.file().exists());
        let status = m.json(&["daemon", "status", "--json"]);
        assert_eq!(status["running"], false, "서비스를 지웠는데 데몬이 돈다 — {status}");
        assert_eq!(status.get("service"), None, "{status}");

        let again = m.json(&["daemon", "uninstall", "--json"]);
        assert_eq!(again["uninstalled"], false, "{again}");
        let out = m.run(&["daemon", "uninstall"]);
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("to remove"), "{}", String::from_utf8_lossy(&out.stdout));
    }
}

/// Windows 의 등록은 다음 에픽이다(`moai-bq5p`). 그때까지 말없이 넘어가지 않고 코드로 멈춘다.
#[cfg(windows)]
#[test]
fn service_registration_is_unsupported_on_windows_for_now() {
    let f = Fence::new();
    for verb in ["install", "uninstall"] {
        let err = f.refused(&["daemon", verb, "--json"]);
        assert_eq!(err["code"], "unsupported", "{verb}: {err}");
    }
    assert_eq!(f.json(&["daemon", "status", "--json"]).get("service"), None);
}
