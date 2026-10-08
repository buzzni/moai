//! `moai update` — 받을 저장소의 `install.sh` 를 받아 도는 바이너리의 자리에 돌린다(moai-zsfr.2p9).
//!
//! **여기는 잇기만 한다.** 무엇을 돌릴지는 [`crate::update`] 가 고르고(시험이 그물 없이 잰다), 여기는
//! 설정을 읽어 넘기고, 받고, `sh` 를 띄운다.
//!
//! 차례가 계약이다 — **그물 전에 다 잰다.** 저장소 값이 틀렸거나 못 올리는 자리면 아무것도 안 받고
//! 비영으로 멈춘다. `--dry-run` 도 같은 문을 지난다: 거절될 명령을 "돌릴 것" 이라 찍지 않는다.
//!
//! **종료 코드는 `sh` 의 것이다**(`cmd::exit_with`). `install.sh` 가 고른 코드를 1 로 접으면 부른 쪽이
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
    let reg = ctx.registry();
    // **설정의 틀린 값은 막는다** — 판 묻기는 기본 저장소로 내려가지만(`latest::repo_lenient`), 이쪽은 그
    // 저장소의 스크립트를 셸에 흘린다. 환경이 이기는 자리라도 막는다: 틀린 설정은 고칠 것이고, 그것을
    // 덮은 채 돌리면 다음 판에 같은 자리에서 다시 넘어진다.
    if let Some(why) = reg.update_problems.iter().find(|p| p.about_repo()) {
        return Err(Fail::coded(crate::view::bad_repo_config(lang, reg, why), code::BAD_INPUT));
    }
    let repo = crate::latest::repo_from(env, reg.update_repo.as_deref())
        .map_err(|why| Fail::coded(crate::view::bad_repo_env(lang, &why), code::BAD_INPUT))?;
    let exe = std::env::current_exe().map_err(|e| {
        Fail::coded(
            crate::i18n::fill(crate::i18n::say(lang, "update.no_exe"), &[("said", &e.to_string())]),
            code::ERROR,
        )
    })?;
    // **푼 자리다** — 링크로 선 `~/bin/moai` 를 올리면 링크가 아니라 그 끝의 파일을 갈아야 한다.
    let exe = crate::path::real(&exe);
    if let Some(why) = crate::update::refusal(&exe, crate::latest::SERVED, crate::update::writable) {
        return Err(Fail::coded(crate::view::update_refused(lang, &why), code::NOT_UPDATABLE));
    }
    let dir = exe.parent().unwrap_or(std::path::Path::new("/"));
    let plan = crate::update::plan(&repo, crate::update::script_url(env, &repo), dir, a.version.as_deref());
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
                &[("url", &plan.script), ("dir", &plan.dir.display().to_string())]
            )
        ));
    }
    let status = sh(&plan, script, ctx.json).map_err(|e| {
        Fail::coded(crate::i18n::fill(crate::i18n::say(lang, "update.no_sh"), &[("said", &e.to_string())]), code::ERROR)
    })?;
    // 신호로 끝났으면 코드가 없다 — 그때는 실패로만 센다.
    let code = status.code().unwrap_or(1);
    exit_with(u8::try_from(code).unwrap_or(1));
    if ctx.json {
        return Ok(vec![said(&plan, Some(code)).to_string()]);
    }
    Ok(Vec::new())
}

/// `sh -s -- …` 를 띄우고 스크립트를 표준 입력으로 흘린다 — 사람이 치던 `curl … | sh -s -- …` 의 뒷반이다.
///
/// **`--json` 이면 `install.sh` 의 표준 출력을 stderr 로 돌린다.** stdout 은 JSON 값 하나만 서는 자리다.
/// 사람 화면에서는 그대로 흘린다 — 받는 동안 무엇을 하는지 보이는 것이 그 스크립트의 일이다.
///
/// **스크립트는 딴 실에서 쓴다.** `sh -s` 는 읽으며 돌리므로, 스크립트가 파이프 한 칸보다 크면 쓰는 쪽이
/// 막혀 기다리기 전에 서로를 붙든다.
fn sh(plan: &crate::update::Plan, script: Vec<u8>, json: bool) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = Command::new("sh");
    cmd.args(&plan.args).env(crate::latest::REPO_VAR, &plan.repo).stdin(Stdio::piped());
    if json {
        cmd.stdout(std::io::stderr());
    }
    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().expect("stdin 을 파이프로 열었다");
    // 스크립트가 일찍 끝나 파이프를 닫으면 쓰기가 실패한다 — 그때의 답은 `sh` 의 종료 코드가 든다.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&script);
    });
    let status = child.wait();
    let _ = writer.join();
    status
}

/// `--json` 의 값 하나 — 돌릴 것과, 돌렸으면 `sh` 의 종료 코드.
fn said(plan: &crate::update::Plan, code: Option<i32>) -> serde_json::Value {
    let mut command = vec!["sh".to_string()];
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
