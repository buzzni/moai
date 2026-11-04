//! `moai daemon start·stop·status` — 사용자당 하나인 로컬 데몬을 띄우고, 멈추고, 묻는다(moai-5n6m.x8t).
//!
//! 소켓·자물쇠·섬기기는 `crate::daemon` 에 있고 여기는 그 결과를 사람의 말과 `--json` 으로 옮긴다.
//!
//! - **`start` 는 앞에 머문다.** 뒤로 가는 판을 두지 않는다 — systemd·launchd 는 앞에 머무는 프로세스를 맡기 쉽고,
//!   셸에서는 `&` 면 된다. 스스로 떨어져 나가는(fork·setsid) 길을 두면 그 길의 오류를 알릴 자리가 없어진다.
//! - **이미 떠 있으면 둘째를 띄우지 않는다.** 같은 판이면 그렇다고 알리고 0, 다른 판이면 두 판을 대고
//!   `version_mismatch` 로 멈춘다 — 말없이 갈아 끼우지 않는다. 오래 사는 프로세스와 갈아 끼운 바이너리가 갈리는
//!   것(`src/web/run.rs` 의 `exe_of` 가 자식으로 피한 일)을 사람이 알고 다시 띄우게 한다.
//! - **`stop` 은 메서드다**(`daemon.shutdown`) — 신호가 아니라. 전송과 무관하고 Windows 의 named pipe 에도 그대로 선다.
//! - **`status` 는 읽기만 한다.** 남은 소켓도 지우지 않는다 — 지우는 것은 `start` 다.

use super::{Ctx, R};
use crate::cli::DaemonCmd;
use crate::fail::{Fail, code};
use crate::i18n::{fill, say};
use serde::Serialize;

pub fn run(ctx: &Ctx, cmd: DaemonCmd) -> R<Vec<String>> {
    match cmd {
        DaemonCmd::Start => os::start(ctx),
        DaemonCmd::Stop => os::stop(ctx),
        DaemonCmd::Status => os::status(ctx),
    }
}

/// `status --json` 의 꼴. 떠 있지 않으면 `pid`·`version`·`proto`·`same_version` 이 없다 — 없는 것이 답이다.
#[derive(Serialize)]
struct Status {
    /// 이 OS 에 데몬이 있는가. Windows 는 `false` 다.
    supported: bool,
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    socket: Option<String>,
    /// 죽은 데몬이 남긴 소켓이 서 있다. `moai daemon start` 가 치운다.
    stale_socket: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proto: Option<u64>,
    /// 떠 있는 데몬이 이 바이너리와 같은 판·`proto` 인가.
    #[serde(skip_serializing_if = "Option::is_none")]
    same_version: Option<bool>,
    this_version: &'static str,
    this_proto: u32,
}

impl Status {
    fn new(supported: bool, socket: Option<String>) -> Status {
        Status {
            supported,
            running: false,
            socket,
            stale_socket: false,
            pid: None,
            version: None,
            proto: None,
            same_version: None,
            this_version: env!("CARGO_PKG_VERSION"),
            this_proto: crate::daemon::rpc::PROTO,
        }
    }
}

fn render_status(ctx: &Ctx, s: &Status) -> R<Vec<String>> {
    if ctx.json {
        return super::json_line(s);
    }
    let lang = ctx.lang();
    let mut lines = Vec::new();
    if !s.supported {
        lines.push(say(lang, "daemon.unsupported").to_string());
        return Ok(lines);
    }
    let socket = s.socket.as_deref().unwrap_or_default();
    if s.running {
        let pid = s.pid.map_or_else(|| "?".to_string(), |p| p.to_string());
        let version = s.version.as_deref().unwrap_or("?");
        let proto = s.proto.map_or_else(|| "?".to_string(), |p| p.to_string());
        lines.push(fill(say(lang, "daemon.running"), &[("pid", &pid), ("version", version), ("proto", &proto)]));
        lines.push(fill(say(lang, "daemon.socket"), &[("socket", socket)]));
        if s.same_version == Some(false) {
            lines.push(fill(say(lang, "daemon.restart"), &[("this", s.this_version)]));
        }
    } else {
        lines.push(fill(say(lang, "daemon.not_running"), &[("socket", socket)]));
        if s.stale_socket {
            lines.push(say(lang, "daemon.stale").to_string());
        }
    }
    Ok(lines)
}

/// 데몬이 정한 실패에 사람의 말을 입힌다.
fn failed(said: &str, vars: &[(&str, &str)], code: &'static str) -> Fail {
    Fail::coded(fill(said, vars), code)
}

#[cfg(not(unix))]
mod os {
    use super::*;

    pub fn start(ctx: &Ctx) -> R<Vec<String>> {
        Err(failed(say(ctx.lang(), "daemon.unsupported"), &[], code::UNSUPPORTED))
    }

    pub fn stop(ctx: &Ctx) -> R<Vec<String>> {
        Err(failed(say(ctx.lang(), "daemon.unsupported"), &[], code::UNSUPPORTED))
    }

    /// 묻는 것은 막지 않는다 — `ctl ls` 가 빈 목록으로 답하듯 "없다" 로 답하고 0 이다.
    pub fn status(ctx: &Ctx) -> R<Vec<String>> {
        render_status(ctx, &Status::new(false, None))
    }
}

#[cfg(unix)]
mod os {
    use super::*;
    use crate::daemon::client::{self, Hello, Probe};
    use crate::daemon::{SHUTDOWN, rpc, server};
    use crate::i18n::Lang;
    use std::io::Write as _;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// 남의 `start` 가 자물쇠를 쥔 채 아직 소켓을 안 묶었을 때, 또는 멈추는 데몬이 아직 안 끝났을 때 기다리는 시한.
    const WAIT: Duration = Duration::from_secs(5);
    /// `stop` 이 데몬이 끝나기를 기다리는 시한 — 데몬은 돌던 읽기를 5초까지, 쓰기는 끝까지 기다린다(`server::Work`).
    const STOP_WAIT: Duration = Duration::from_secs(10);
    const POLL: Duration = Duration::from_millis(50);

    #[derive(Serialize)]
    struct Started<'a> {
        started: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        version: &'a str,
        proto: u64,
        socket: &'a str,
    }

    #[derive(Serialize)]
    struct Stopped<'a> {
        stopped: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        socket: &'a str,
    }

    fn socket_path(lang: Lang) -> R<PathBuf> {
        crate::daemon::socket_path()
            .map_err(|e| failed(say(lang, "daemon.no_place"), &[("said", &e.to_string())], code::ERROR))
    }

    fn shown(path: &Path) -> String {
        crate::text::one_line(&path.display().to_string())
    }

    /// 자리에 선 파일이 제 소켓이 아니면 거절한다 — 지우지도 붙지도 않는다.
    fn own_or_refuse(lang: Lang, path: &Path) -> R<Option<std::fs::Metadata>> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if server::is_own_socket(&meta) => Ok(Some(meta)),
            Ok(_) => Err(failed(say(lang, "daemon.occupied"), &[("socket", &shown(path))], code::ALREADY_EXISTS)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(failed(say(lang, "daemon.no_place"), &[("said", &e.to_string())], code::ERROR)),
        }
    }

    fn pid_text(pid: Option<u32>) -> String {
        pid.map_or_else(|| "?".to_string(), |p| p.to_string())
    }

    /// 떠 있는 데몬을 만났다 — 같은 판이면 알리고 0, 다른 판이면 두 판을 대고 멈춘다.
    fn already(ctx: &Ctx, path: &Path, hello: &Hello) -> R<Vec<String>> {
        let lang = ctx.lang();
        let socket = shown(path);
        if !hello.same_as_this() {
            return Err(failed(
                say(lang, "daemon.other_version"),
                &[("running", &hello.version), ("pid", &pid_text(hello.pid)), ("this", env!("CARGO_PKG_VERSION"))],
                code::VERSION_MISMATCH,
            ));
        }
        if ctx.json {
            return super::super::json_line(&Started {
                started: false,
                pid: hello.pid,
                version: &hello.version,
                proto: hello.proto,
                socket: &socket,
            });
        }
        Ok(vec![fill(
            say(lang, "daemon.already"),
            &[("pid", &pid_text(hello.pid)), ("version", &hello.version), ("socket", &socket)],
        )])
    }

    fn not_a_daemon(lang: Lang, path: &Path, why: &str) -> Fail {
        failed(say(lang, "daemon.foreign"), &[("socket", &shown(path)), ("said", why)], code::ERROR)
    }

    fn busy(lang: Lang, path: &Path, pid: Option<u32>) -> Fail {
        failed(say(lang, "daemon.busy"), &[("socket", &shown(path)), ("pid", &pid_text(pid))], code::BUSY)
    }

    pub fn start(ctx: &Ctx) -> R<Vec<String>> {
        let lang = ctx.lang();
        let path = socket_path(lang)?;
        let no_lock =
            |e: std::io::Error| failed(say(lang, "daemon.no_place"), &[("said", &e.to_string())], code::ERROR);
        // **자물쇠가 먼저다** — 쥔 쪽만 소켓을 치우고 묶는다. 못 쥐면 떠 있는(또는 뜨는 중인) 데몬이 있다.
        let deadline = Instant::now() + WAIT;
        let lock = loop {
            if let Some(lock) = server::lock(&path).map_err(no_lock)? {
                break lock;
            }
            match client::probe(&path) {
                Probe::Running(hello) => return already(ctx, &path, &hello),
                Probe::Busy(pid) => return Err(busy(lang, &path, pid)),
                _ if Instant::now() >= deadline => {
                    return Err(failed(
                        say(lang, "daemon.starting_elsewhere"),
                        &[("socket", &shown(&path))],
                        code::BUSY,
                    ));
                }
                _ => std::thread::sleep(POLL),
            }
        };
        // 자물쇠를 쥐었으니 남은 소켓은 죽은 데몬의 것이다. 그래도 종류·주인을 보고 붙어 본 뒤에 지운다.
        if let Some(meta) = own_or_refuse(lang, &path)? {
            match client::probe(&path) {
                Probe::Running(hello) => return already(ctx, &path, &hello),
                Probe::Busy(pid) => return Err(busy(lang, &path, pid)),
                Probe::Foreign(why) => return Err(not_a_daemon(lang, &path, &why)),
                Probe::Absent => {}
                Probe::Dead => server::remove_if_same(&path, meta.ino())
                    .map_err(|e| failed(say(lang, "daemon.no_place"), &[("said", &e.to_string())], code::ERROR))?,
            }
        }
        let bound = server::bind(&path)
            .map_err(|e| failed(say(lang, "daemon.bind_failed"), &[("said", &e.to_string())], code::ERROR))?;
        let socket = shown(&path);
        let first = match ctx.json {
            true => serde_json::to_string(&Started {
                started: true,
                pid: Some(std::process::id()),
                version: env!("CARGO_PKG_VERSION"),
                proto: u64::from(rpc::PROTO),
                socket: &socket,
            })
            .map_err(|e| Fail::new(e.to_string()))?,
            false => fill(
                say(lang, "daemon.listening"),
                &[
                    ("socket", &socket),
                    ("pid", &std::process::id().to_string()),
                    ("version", env!("CARGO_PKG_VERSION")),
                ],
            ),
        };
        // **첫 줄은 묶은 뒤에 낸다** — 띄운 쪽은 이 줄을 듣고 붙는다(`cmd::web::run` 과 같은 차례). 못 냈으면
        // 섬기지 않는다: 받는 쪽이 닫힌 것은 조용히 그친다.
        let wrote = {
            let mut out = anstream::stdout().lock();
            writeln!(out, "{first}").and_then(|()| out.flush())
        };
        match wrote {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => return Ok(Vec::new()),
            Err(e) => return Err(Fail::new(fill(say(lang, "warn.stdout_lost"), &[("why", &e.to_string())]))),
        }
        let served = server::serve(bound, crate::daemon::methods());
        // 소켓을 지운 뒤에 자물쇠를 놓는다 — 놓인 자물쇠를 본 `stop` 이 이 데몬이 끝났다고 읽는다.
        drop(lock);
        served.map_err(|e| failed(say(lang, "daemon.bind_failed"), &[("said", &e.to_string())], code::ERROR))?;
        Ok(Vec::new())
    }

    pub fn stop(ctx: &Ctx) -> R<Vec<String>> {
        let lang = ctx.lang();
        let path = socket_path(lang)?;
        let socket = shown(&path);
        let hello = match client::probe(&path) {
            Probe::Running(hello) => hello,
            Probe::Absent | Probe::Dead => {
                if ctx.json {
                    return super::super::json_line(&Stopped { stopped: false, pid: None, socket: &socket });
                }
                return Ok(vec![fill(say(lang, "daemon.not_running"), &[("socket", &socket)])]);
            }
            Probe::Busy(pid) => return Err(busy(lang, &path, pid)),
            Probe::Foreign(why) => return Err(not_a_daemon(lang, &path, &why)),
        };
        match client::call(&path, SHUTDOWN, serde_json::json!({})) {
            Ok(Ok(_)) => {}
            // 붙어 본 뒤 이 부름까지의 틈에 그 데몬이 먼저 멈추기 시작했다 — 끝나기를 아래에서 기다린다.
            called if already_stopping(&called) => {}
            Ok(Err(e)) => return Err(not_a_daemon(lang, &path, &e.message)),
            Err(e) => return Err(not_a_daemon(lang, &path, &e.to_string())),
        }
        // 자물쇠가 풀리면 그 데몬은 끝났다. 그 사이 남이 새 데몬을 띄웠으면(다른 pid) 그것도 끝난 것으로 읽는다.
        let deadline = Instant::now() + STOP_WAIT;
        loop {
            if let Ok(Some(_free)) = server::lock(&path) {
                break;
            }
            if let Probe::Running(now) = client::probe(&path)
                && now.pid != hello.pid
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err(failed(
                    say(lang, "daemon.stop_timeout"),
                    &[("pid", &pid_text(hello.pid)), ("secs", &STOP_WAIT.as_secs().to_string())],
                    code::TIMEOUT,
                ));
            }
            std::thread::sleep(POLL);
        }
        if ctx.json {
            return super::super::json_line(&Stopped { stopped: true, pid: hello.pid, socket: &socket });
        }
        Ok(vec![fill(say(lang, "daemon.stopped"), &[("pid", &pid_text(hello.pid))])])
    }

    pub fn status(ctx: &Ctx) -> R<Vec<String>> {
        let lang = ctx.lang();
        let path = socket_path(lang)?;
        let meta = own_or_refuse(lang, &path)?;
        let mut s = Status::new(true, Some(shown(&path)));
        match client::probe(&path) {
            Probe::Running(hello) => {
                s.running = true;
                s.same_version = Some(hello.same_as_this());
                s.pid = hello.pid;
                s.version = Some(hello.version);
                s.proto = Some(hello.proto);
            }
            Probe::Absent => {}
            Probe::Dead => s.stale_socket = meta.is_some(),
            Probe::Busy(pid) => return Err(busy(lang, &path, pid)),
            Probe::Foreign(why) => return Err(not_a_daemon(lang, &path, &why)),
        }
        render_status(ctx, &s)
    }

    /// `stop` 이 보낸 멈추라는 부름이 **이미 멈추는 데몬**에 닿았는가(moai-3hgm.uz0 의 리뷰). 멈추는 데몬은 돌던 요청을
    /// 기다리기 전에 소켓 파일부터 지우고 문을 닫는다 — 붙어 본(`probe`) 뒤 이 부름까지의 틈에 옆의 `stop` 이 먼저
    /// 닿았으면 붙기가 "없다"(NotFound)·"안 받는다"(ECONNREFUSED)로 지거나, 붙었어도 `stopping` 으로 거절된다. 그것을
    /// "데몬이 아니다" 로 읽던 판은 같은 데몬을 멈추던 둘째 `stop` 을 1 로 끝냈다. 잘못 고른 판도 거짓 성공은 안 된다 —
    /// 부르는 쪽은 자물쇠가 풀리기를 기다리고, 안 풀리면 `timeout` 으로 끝난다.
    fn already_stopping(called: &std::io::Result<Result<serde_json::Value, rpc::RpcError>>) -> bool {
        match called {
            Ok(Ok(_)) => false,
            Ok(Err(e)) => {
                e.data.as_ref().and_then(|d| d.get("code")).and_then(serde_json::Value::as_str) == Some(code::STOPPING)
            }
            Err(e) => matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 멈추는 데몬에 닿은 부름만 "이미 멈춘다" 다 — 다른 거절과 다른 전송 실패는 그대로 "데몬이 아니다" 로 간다.
        #[test]
        fn only_a_daemon_already_stopping_is_read_as_stopping() {
            let refused = |data: serde_json::Value| -> std::io::Result<Result<serde_json::Value, rpc::RpcError>> {
                Ok(Err(rpc::RpcError { code: rpc::code::MOAI, message: "no".into(), data: Some(data) }))
            };
            let io = |kind: std::io::ErrorKind| -> std::io::Result<Result<serde_json::Value, rpc::RpcError>> {
                Err(std::io::Error::from(kind))
            };
            assert!(already_stopping(&refused(serde_json::json!({ "code": code::STOPPING, "pid": 1 }))));
            assert!(already_stopping(&io(std::io::ErrorKind::NotFound)));
            assert!(already_stopping(&io(std::io::ErrorKind::ConnectionRefused)));
            assert!(!already_stopping(&refused(serde_json::json!({ "code": code::BUSY }))));
            assert!(!already_stopping(&Ok(Err(rpc::RpcError::new(rpc::code::INTERNAL, "boom")))));
            assert!(!already_stopping(&io(std::io::ErrorKind::InvalidData)));
            assert!(!already_stopping(&io(std::io::ErrorKind::TimedOut)));
            assert!(!already_stopping(&Ok(Ok(serde_json::json!({ "pid": 1 })))));
        }
    }
}
