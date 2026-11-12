//! `moai update` — 받을 저장소의 `install.sh` 를 받아 도는 바이너리의 자리에 돌린다(moai-zsfr.2p9).
//! Windows 에서는 `install.ps1` 을 받아 PowerShell 로 돌린다(moai-j9gv.3fl).
//!
//! **여기는 잇기만 한다.** 무엇을 돌릴지는 [`crate::update`] 가 고르고(시험이 그물 없이 잰다), 여기는
//! 설정을 읽어 넘기고, 받고, 셸(`sh` 나 `powershell`)을 띄운다.
//!
//! 차례가 계약이다 — **그물 전에 다 잰다.** 저장소 값이 틀렸거나 못 올리는 자리면 아무것도 안 받고
//! 비영으로 멈춘다. `--dry-run` 도 같은 문을 지난다: 거절될 명령을 "돌릴 것" 이라 찍지 않는다.
//!
//! **종료 코드는 그 셸의 것이다**(`cmd::exit_with`). 설치 스크립트가 고른 코드를 1 로 접으면 부른 쪽이
//! 그것을 못 가른다.

use super::{Ctx, R, exit_with, tell};
use crate::cli::UpdateArgs;
use crate::fail::{Fail, code};
use std::io::Write as _;
use std::process::{Command, Stdio};

pub fn run(ctx: &Ctx, a: UpdateArgs) -> R<Vec<String>> {
    let lang = ctx.lang();
    let env = |k: &str| std::env::var_os(k);
    // 판은 `sh` 에 날 인자로 가므로 셸이 가르지 않는다. 다만 `-` 로 시작하면 `install.sh` 가 플래그로 읽어
    // 다른 일을 한다 — 그것은 태그가 아니다.
    if let Some(v) = a.version.as_deref()
        && (v.is_empty() || v.starts_with('-'))
    {
        return Err(Fail::coded(
            crate::i18n::fill(crate::i18n::say(lang, "update.bad_version"), &[("raw", &format!("{v:?}"))]),
            code::BAD_INPUT,
        ));
    }
    let repo = chosen_repo(lang, ctx.registry(), env)?;
    let exe = std::env::current_exe().map_err(|e| {
        Fail::coded(
            crate::i18n::fill(crate::i18n::say(lang, "update.no_exe"), &[("said", &e.to_string())]),
            code::ERROR,
        )
    })?;
    // **푼 자리다** — 링크로 선 `~/bin/moai` 를 올리면 링크가 아니라 그 끝의 파일을 갈아야 한다. Windows 의
    // `\\?\` 는 푸는 자가 걷는다([`crate::path::canonical`]) — 거절문·`--json` 의 `dir`·인자가 한 철자를 댄다.
    let exe = crate::path::real(&exe);
    let runner = crate::update::Runner::native(&std::env::temp_dir());
    if let Some(why) = crate::update::refusal(&exe, crate::latest::SERVED, crate::update::writable) {
        return Err(Fail::coded(crate::view::update_refused(lang, &why, &runner), code::NOT_UPDATABLE));
    }
    let dir = exe.parent().unwrap_or(std::path::Path::new("/"));
    let script = crate::update::script_url(env, &repo, &runner);
    let plan = crate::update::plan(&repo, script, dir, a.version.as_deref(), runner);
    if a.dry_run {
        if ctx.json {
            return Ok(vec![said(&plan, None).to_string()]);
        }
        return Ok(crate::view::update_dry(lang, &plan));
    }
    let script = crate::latest::fetch_script(&plan.script).map_err(|why| {
        let said = if why.said.is_empty() { why.kind.name().to_string() } else { why.said };
        Fail::coded(
            crate::i18n::fill(crate::i18n::say(lang, "update.fetch_failed"), &[("url", &plan.script), ("said", &said)]),
            code::ERROR,
        )
    })?;
    if !ctx.json {
        tell(&format!(
            "moai: {}",
            crate::i18n::fill(
                crate::i18n::say(lang, "update.running"),
                &[("script", plan.runner.script()), ("url", &plan.script), ("dir", &plan.dir.display().to_string())]
            )
        ));
    }
    let status = sh(&plan, script, ctx.json).map_err(|e| {
        Fail::coded(
            crate::i18n::fill(
                crate::i18n::say(lang, "update.no_sh"),
                &[("program", plan.runner.program()), ("said", &e.to_string())],
            ),
            code::ERROR,
        )
    })?;
    // 신호로 끝났으면 코드가 없다 — 그때는 실패로만 센다.
    let code = status.code().unwrap_or(1);
    exit_with(u8::try_from(code).unwrap_or(1));
    #[cfg(any(unix, windows))]
    let seen = (code == 0).then(|| daemon_restart(&exe)).flatten();
    #[cfg(any(unix, windows))]
    let restart = seen.as_ref().map(|seen| seen.old.clone());
    #[cfg(not(any(unix, windows)))]
    let restart: Option<(String, u64)> = None;
    // 다른 판의 데몬을 찾은 때만 더 본다 — 서비스가 없는 사람의 `moai update` 는 관리자를 안 부르고 출력도 그대로다.
    #[cfg(any(unix, windows))]
    let restarted = seen.as_ref().map_or(ServiceRestart::NotDone, |seen| restart_service(&exe, seen));
    #[cfg(not(any(unix, windows)))]
    let restarted = ServiceRestart::NotDone;
    if !ctx.json {
        let line = match (&restarted, &restart) {
            (ServiceRestart::Done, _) => Some(crate::i18n::say(lang, "update.daemon_restarted").to_string()),
            (ServiceRestart::Unanswered, _) => {
                Some(crate::i18n::say(lang, "update.daemon_restart_pending").to_string())
            }
            (ServiceRestart::NotDone, Some((version, pid))) => Some(crate::i18n::fill(
                crate::i18n::say(lang, "update.daemon_restart"),
                &[("version", version), ("pid", &pid.to_string())],
            )),
            (ServiceRestart::NotDone, None) => None,
        };
        if let Some(line) = line {
            tell(&format!("moai: {line}"));
        }
    }
    if code == 0 && !ctx.json {
        tell(&format!("moai: {}", crate::i18n::say(lang, "update.refresh_projects")));
    }
    if ctx.json {
        let mut result = said(&plan, Some(code));
        if matches!(restarted, ServiceRestart::Done) {
            result["daemon_restarted"] = true.into();
        } else if restart.is_some() {
            result["daemon_restart"] = true.into();
        }
        return Ok(vec![result.to_string()]);
    }
    Ok(Vec::new())
}

/// 새 바이너리가 한 번 한 답 — 옛 데몬(알림용)과, 다시 시작한 뒤 무엇이 답해야 새 판인지.
#[cfg(any(unix, windows, test))]
#[derive(Debug, PartialEq)]
struct Seen {
    /// 옛 데몬의 판과 pid.
    old: (String, u64),
    /// 새 바이너리가 푼 소켓·판·`proto`. 하나라도 못 읽으면 `None` — 확인할 수 없는 재시작은 안 한다.
    target: Option<Target>,
}

#[cfg(any(unix, windows, test))]
#[derive(Debug, PartialEq)]
struct Target {
    socket: std::path::PathBuf,
    version: String,
    proto: u64,
}

/// 새 바이너리를 **한 번** 띄워 데몬의 상태를 읽는다. 다시 시작한 뒤의 기다림은 프로세스를 더 띄우지 않는다.
#[cfg(any(unix, windows))]
fn daemon_restart(exe: &std::path::Path) -> Option<Seen> {
    let out = Command::new(exe).args(["daemon", "status", "--json"]).stdin(Stdio::null()).output().ok()?;
    out.status.success().then_some(())?;
    seen_from_status(&out.stdout)
}

/// 등록된 서비스를 다시 시작해 본 결과.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
enum ServiceRestart {
    /// 부르지 않았거나 관리자가 거절했다. 옛 데몬이 그대로일 수 있어 지금까지의 알림을 낸다.
    NotDone,
    /// 관리자는 받았는데 시한 안에 새 판이 안 답했다. 옛 데몬은 이미 멈췄을 수 있어 "아직 돈다" 고 말하면 거짓이다.
    Unanswered,
    /// 새 판이 답했다.
    Done,
}

/// 등록된 서비스를 새 판으로 다시 시작하고 새 판이 답할 때까지 기다린다(moai-upq5.1g6). `moai daemon stop` 으로 하지 않는다 —
/// 깨끗한 종료는 관리자가 다시 안 띄운다. 등록 상태를 바꾸는 호출은 이 길에 없다.
///
/// 기다림은 프로세스를 띄우지 않고 `client::probe` 로 새 바이너리가 댄 소켓에 `hello` 를 묻는다(moai-tloc.wt9).
/// 붙기 한 번이 시한(2초)까지 걸릴 수 있어 5초가 지난 뒤에는 새 붙기를 시작하지 않는다.
#[cfg(any(unix, windows))]
fn restart_service(exe: &std::path::Path, seen: &Seen) -> ServiceRestart {
    use crate::daemon::service::{Service, run};
    use std::time::{Duration, Instant};
    let Some(target) = &seen.target else {
        return ServiceRestart::NotDone;
    };
    let Ok(service) = Service::native() else {
        return ServiceRestart::NotDone;
    };
    if !service.restart(exe, &run) {
        return ServiceRestart::NotDone;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let crate::daemon::client::Probe::Running(hello) = crate::daemon::client::probe(&target.socket)
            && answers(&hello, target)
        {
            return ServiceRestart::Done;
        }
        if Instant::now() >= deadline {
            return ServiceRestart::Unanswered;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 답한 데몬이 새 바이너리의 판인가 — 판이 같아도 `proto` 가 다르면 아니다.
#[cfg(any(unix, windows, test))]
fn answers(hello: &crate::daemon::client::Hello, target: &Target) -> bool {
    hello.version == target.version && hello.proto == target.proto
}

#[cfg(any(unix, windows, test))]
fn seen_from_status(bytes: &[u8]) -> Option<Seen> {
    let status: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if status["running"] != true || status["same_version"] != false {
        return None;
    }
    let old = (status["version"].as_str()?.to_owned(), status["pid"].as_u64()?);
    let target = (|| {
        Some(Target {
            socket: status["socket"].as_str()?.into(),
            version: status["this_version"].as_str()?.to_owned(),
            proto: status["this_proto"].as_u64()?,
        })
    })();
    Some(Seen { old, target })
}

/// 받을 저장소 — **막는 자는 여기 하나다.** `moai update` 가 이것으로 고르고, 탐색기의 새 판 안내
/// (`cmd::tui`)는 이것이 `Err` 인 자리에서 `moai update` 를 안 댄다 — 안내가 대는 명령이 거절되는 판을 안
/// 만든다.
///
/// - **설정의 틀린 값은 막는다** — 판 묻기는 기본 저장소로 내려가지만(`latest::repo_lenient`), 이쪽은 그
///   저장소의 스크립트를 셸에 흘린다. 환경이 이기는 자리라도 막는다: 틀린 설정은 고칠 것이고, 그것을
///   덮은 채 돌리면 다음 판에 같은 자리에서 다시 넘어진다
/// - **못 읽은 설정도 막는다** — 깨진 TOML·못 여는 파일은 `[update] repo` 를 적었는지조차 모른다. 그대로
///   기본 저장소로 가면 포크를 고른 사람이 본가의 판을 깐다. `MOAI_REPO` 를 준 판은 설정을 안 보므로 돈다
pub(crate) fn chosen_repo(
    lang: crate::i18n::Lang,
    reg: &crate::user_config::Registry,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> R<String> {
    use crate::user_config::Trouble;
    if let Some(why) = reg.update_problems.iter().find(|p| p.about_repo()) {
        return Err(Fail::coded(crate::view::bad_repo_config(lang, reg, why), code::BAD_INPUT));
    }
    let env_names = env(crate::latest::REPO_VAR).is_some_and(|v| !v.is_empty());
    if !env_names
        && matches!(reg.trouble, Some(Trouble::Reading | Trouble::Unreadable | Trouble::Broken))
        && let Some(why) = reg.problems.first()
    {
        let said = crate::view::config_problem(lang, reg.path.as_deref(), why);
        return Err(Fail::coded(
            crate::i18n::fill(crate::i18n::say(lang, "update.bad_repo_config"), &[("said", &said)]),
            code::BAD_INPUT,
        ));
    }
    crate::latest::repo_from(env, reg.update_repo.as_deref())
        .map_err(|why| Fail::coded(crate::view::bad_repo_env(lang, &why), code::BAD_INPUT))
}

/// `sh -s -- …` 를 띄우고 스크립트를 표준 입력으로 흘린다 — 사람이 치던 `curl … | sh -s -- …` 의 뒷반이다.
/// Windows 면 스크립트를 [`Runner::PowerShell`](crate::update::Runner) 의 파일에 적고 `powershell -File` 로
/// 돌린 뒤 그 파일을 걷는다.
///
/// **`--json` 이면 설치 스크립트의 표준 출력을 stderr 로 돌린다.** stdout 은 JSON 값 하나만 서는 자리다.
/// 사람 화면에서는 그대로 흘린다 — 받는 동안 무엇을 하는지 보이는 것이 그 스크립트의 일이다.
///
/// **스크립트는 딴 실에서 쓴다.** `sh -s` 는 읽으며 돌리므로, 스크립트가 파이프 한 칸보다 크면 쓰는 쪽이
/// 막혀 기다리기 전에 서로를 붙든다.
fn sh(plan: &crate::update::Plan, script: Vec<u8>, json: bool) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = Command::new(plan.runner.program());
    cmd.args(&plan.args).env(crate::latest::REPO_VAR, &plan.repo);
    // **판은 `--version` 하나로 고른다** — 설치 스크립트는 `MOAI_VERSION` 도 읽으므로, 셸에 그것이 서 있으면
    // `--version` 없이 부른 판이 "최신" 이 아니라 그 판을 깔고 `--dry-run` 의 줄과도 갈린다.
    cmd.env_remove("MOAI_VERSION");
    if json {
        cmd.stdout(std::io::stderr());
    }
    if let crate::update::Runner::PowerShell { file } = &plan.runner {
        // **새로 만든 파일에만 적는다**(리뷰). 이름이 프로세스 번호라 미리 알 수 있다 — `fs::write` 는 그 자리에
        // 먼저 선 파일이나 링크를 따라가 덮는다. 앞 판이 남긴 같은 이름은 걷고, 그래도 서 있으면 멈춘다. 못 적은
        // 것은 자리를 대어 "띄우지 못했다" 와 가른다.
        let at = |e: std::io::Error| std::io::Error::new(e.kind(), format!("{}: {e}", file.display()));
        // **지우는 일은 Drop 이 맡고, Ctrl-C 는 moai 가 살아남아 받는다**(moai-vjdq.chk). 콘솔의 Ctrl-C 는 moai 와
        // 자식 powershell 둘 다에 간다 — Drop 은 프로세스가 죽으면 안 돈다. 그래서 자식이 끝날 때까지 moai 쪽만
        // 처리기로 삼켜 파일을 걷고 나서 나간다. `SetConsoleCtrlHandler(NULL, TRUE)` 로 무시하지 않는 까닭은 그
        // 플래그가 자식에게 상속돼 powershell 까지 Ctrl-C 를 못 받기 때문이다 — 처리기 함수는 상속되지 않는다.
        // 파일을 만들기 전에 서고, 일찍 실패하는 길(`?`)도 같은 Drop 이 지나간다.
        let _guard = TempScript::new(file);
        let _ = std::fs::remove_file(file);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(file)
            .and_then(|mut f| f.write_all(&script))
            .map_err(at)?;
        // `install.ps1` 은 표준 입력을 안 읽는다 — 이 프로세스의 것을 물려주면 묻는 것이 없어도 쥔다.
        let status = cmd.stdin(Stdio::null()).status();
        return status;
    }
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().expect("stdin 을 파이프로 열었다");
    // 스크립트가 일찍 끝나 파이프를 닫으면 쓰기가 실패한다 — 그때의 답은 `sh` 의 종료 코드가 든다.
    // **실을 못 띄워도 패닉하지 않는다**(`latest::spawn` 과 같은 자) — 그러면 받은 스크립트 대신 빈 입력을 읽은
    // `sh` 가 0 으로 끝나고 moai 는 101 로 죽는다. 못 띄웠으면 `sh` 를 거두고 그 까닭을 댄다.
    let writer = match std::thread::Builder::new().name("moai-update".into()).spawn(move || {
        let _ = stdin.write_all(&script);
    }) {
        Ok(writer) => writer,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    let status = child.wait();
    let _ = writer.join();
    status
}

/// 임시 스크립트 한 장의 임자 — 떨어지면 파일을 걷는다(못 지워도 임시 자리라 막지 않는다). Windows 에서는
/// 살아 있는 동안 Ctrl-C 를 삼켜 Drop 이 돌 수 있게 한다.
struct TempScript<'a>(&'a std::path::Path);

impl<'a> TempScript<'a> {
    fn new(file: &'a std::path::Path) -> Self {
        #[cfg(windows)]
        ctrl::swallow(true);
        TempScript(file)
    }
}

impl Drop for TempScript<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
        #[cfg(windows)]
        ctrl::swallow(false);
    }
}

#[cfg(windows)]
mod ctrl {
    type Handler = unsafe extern "system" fn(u32) -> i32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<Handler>, add: i32) -> i32;
    }
    /// CTRL_C_EVENT(0) 만 처리한 것으로 돌려 기본 동작(종료)을 막는다. **Ctrl-Break 는 지나보낸다**(리뷰
    /// moai-vjdq.tg6 9번) — `install.ps1` 이 멈춰 Ctrl-C 를 안 받으면 창을 닫는 것 말고 나갈 길이 그것뿐이다.
    /// 그 길로 나가면 임시 파일이 남지만, 갇히는 것보다 싸다.
    unsafe extern "system" fn on_ctrl(kind: u32) -> i32 {
        (kind == 0) as i32
    }
    pub fn swallow(on: bool) {
        // SAFETY: 서명이 kernel32 의 것과 같고, 처리기는 인자만 읽는다.
        unsafe {
            SetConsoleCtrlHandler(Some(on_ctrl), on as i32);
        }
    }
}

/// `--json` 의 값 하나 — 돌릴 것과, 돌렸으면 그 셸의 종료 코드.
fn said(plan: &crate::update::Plan, code: Option<i32>) -> serde_json::Value {
    let mut command = vec![plan.runner.program().to_string()];
    command.extend(plan.args.iter().map(|a| a.to_string_lossy().into_owned()));
    let mut v = serde_json::json!({
        "repo": plan.repo,
        "script": plan.script,
        "dir": plan.dir.display().to_string(),
        "command": command,
        "dry_run": code.is_none(),
    });
    if let Some(code) = code {
        v["code"] = code.into();
    }
    v
}

#[cfg(test)]
mod daemon_tests {
    use super::{Seen, Target, answers, seen_from_status};
    use crate::daemon::client::Hello;

    #[test]
    fn restart_notice_only_for_a_running_different_version() {
        let old = br#"{"running":true,"same_version":false,"version":"0.51.0","pid":42}"#;
        assert_eq!(seen_from_status(old), Some(Seen { old: ("0.51.0".into(), 42), target: None }));
        for status in [
            &br#"{"running":false}"#[..],
            &br#"{"running":true,"same_version":true,"version":"0.51.0","pid":42}"#[..],
            &br#"{"running":true,"same_version":false}"#[..],
            &b"broken"[..],
        ] {
            assert_eq!(seen_from_status(status), None);
        }
    }

    #[test]
    fn the_target_needs_the_socket_and_both_versions() {
        let full = r#""socket":"/s/d.sock","this_version":"1.2.3","this_proto":1"#;
        let head = r#"{"running":true,"same_version":false,"version":"0.51.0","pid":42"#;
        let seen = seen_from_status(format!("{head},{full}}}").as_bytes()).unwrap();
        assert_eq!(seen.target, Some(Target { socket: "/s/d.sock".into(), version: "1.2.3".into(), proto: 1 }));
        for missing in [r#""socket""#, r#""this_version""#, r#""this_proto""#] {
            let kept: Vec<&str> = full.split(',').filter(|part| !part.starts_with(missing)).collect();
            let seen = seen_from_status(format!("{head},{}}}", kept.join(",")).as_bytes()).unwrap();
            assert_eq!(seen.target, None, "{missing} is missing");
            assert_eq!(seen.old, ("0.51.0".into(), 42));
        }
    }

    #[test]
    fn the_answer_counts_only_with_the_same_version_and_proto() {
        let target = Target { socket: "/s".into(), version: "1.2.3".into(), proto: 2 };
        let hello = |version: &str, proto| Hello { pid: Some(1), version: version.into(), proto };
        assert!(answers(&hello("1.2.3", 2), &target));
        assert!(!answers(&hello("1.2.2", 2), &target), "an old version");
        assert!(!answers(&hello("1.2.3", 1), &target), "the same version, another proto");
    }
}
