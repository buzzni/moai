//! 훅의 입출력. **판단은 여기 없다** — `hook` 모듈이 정하고 여기는 stdin 을
//! 풀고 저장소를 읽고 답을 계약 JSON 으로 옮긴다.
//!
//! ## 이 명령은 절대 실패하지 않는다
//!
//! 무엇이 어긋나도 빈 출력과 종료 코드 0 이다. 훅이 에러를 뱉으면 매 세션
//! 시작이 시끄럽고, 그러면 사람이 훅을 꺼 버린다 — 꺼진 규칙은 없는 규칙이다.
//! 저장소가 아니어도, stdin 이 JSON 이 아니어도, 줄이 깨져 있어도 조용히
//! 지나간다. `moai status` 가 아무것도 막지 않는 것과 같은 이유다.

use super::{Ctx, R};
use crate::hook::{Decision, Event, Input};
use crate::report;
use crate::store::Repo;
use crate::{model, view};
use serde::Serialize;
use std::io::Read;

/// 계약이 받는 모양. 이벤트 이름이 안에 한 번 더 들어간다.
#[derive(Serialize)]
struct Out<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: Specific<'a>,
}

#[derive(Serialize)]
struct Specific<'a> {
    #[serde(rename = "hookEventName")]
    event: &'a str,
    #[serde(rename = "additionalContext")]
    context: String,
}

pub fn run(_ctx: &Ctx, event: Event) -> R<Vec<String>> {
    // 색은 언제나 끈다. 훅의 stdout 은 사람이 아니라 파서가 읽는다 —
    // 이스케이프가 한 바이트라도 섞이면 계약 JSON 이 통째로 버려진다.
    anstream::ColorChoice::Never.write_global();

    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return Ok(Vec::new());
    }
    let input: Input = serde_json::from_str(&raw).unwrap_or_default();

    // **자리는 stdin 이 정한다.** 훅 프로세스가 어디서 도는지는 아무도
    // 약속하지 않았다 — 시험판이 제 cwd 로 상대 경로를 풀다가 저장소 안의
    // 파일을 저장소 밖으로 보아 규칙이 통째로 샜다.
    if let Some(cwd) = input.cwd.as_deref()
        && std::env::set_current_dir(cwd).is_err()
    {
        return Ok(Vec::new());
    }

    Ok(decide(event, &input).map(|line| vec![line]).unwrap_or_default())
}

/// 답을 내되, 못 내면 아무 말도 하지 않는다.
fn decide(event: Event, input: &Input) -> Option<String> {
    let repo = Repo::discover().ok()?;
    let load = repo.read().ok()?;

    let decision = match event {
        // 기준선만 적고 아무것도 싣지 않는다. 까닭은 `hook::Event` 에 있다.
        Event::SessionStart => {
            write_baseline(input, &load.issues, &repo.config);
            Decision::Pass
        }
        Event::UserPromptSubmit => once_per_session(input, || {
            let now = model::now();
            let st = report::status(&load.issues, &[], &repo.config, &now);
            let lines =
                view::status(&st, &load.issues, &repo.config, &now, ".moai/issues.jsonl");
            crate::hook::board(&lines)
        }),
        Event::PreCompact => crate::hook::carried(&load.issues, &repo.config),
        // 규칙은 아직 이 자리에 없다. 없는 동안은 통과다 — 반쯤 선 규칙이
        // 막는 것이 안 서는 규칙보다 나쁘다.
        Event::PreToolUse | Event::Stop => Decision::Pass,
    };

    match decision {
        Decision::Pass => None,
        Decision::Context(context) => serde_json::to_string(&Out {
            specific: Specific { event: event.wire(), context },
        })
        .ok(),
    }
}

/// 세션마다 한 번만. **매 프롬프트에 보드를 붙이면 그것대로 자리값을 잃는다.**
///
/// 표를 남기는 자리는 시스템 임시 디렉터리다. `.moai/` 에 두면 세션 부스러기가
/// 저장소에 쌓이고, 그것을 `.gitignore` 로 막는 일이 또 생긴다.
fn once_per_session(input: &Input, make: impl FnOnce() -> Decision) -> Decision {
    let Some(path) = session_file(input, "board") else {
        // 누구인지 모르면 한 번을 보장할 수 없다. **그러면 싣지 않는다** —
        // 한 번 빠지는 것이 매 프롬프트 도배보다 싸다.
        return Decision::Pass;
    };
    if path.exists() {
        return Decision::Pass;
    }
    let decision = make();
    if decision != Decision::Pass {
        let _ = std::fs::write(&path, "");
    }
    decision
}

/// `Stop` 이 견줄 기준선 — 세션이 열릴 때의 경고 수.
///
/// 훅이 여는 세션마다 덮어쓴다. 재개도 새 세션이고, 재개 시점의 경고가
/// 그 세션이 물려받은 빚이다.
fn write_baseline(input: &Input, issues: &[crate::model::Issue], cfg: &crate::config::Config) {
    let Some(path) = session_file(input, "warn") else {
        return;
    };
    let now = model::now();
    let st = report::status(issues, &[], cfg, &now);
    let n: usize = st.warnings.iter().map(|w| w.count).sum();
    let _ = std::fs::write(path, n.to_string());
}

fn session_file(input: &Input, what: &str) -> Option<std::path::PathBuf> {
    let sid = input.session_id.as_deref()?;
    // 세션 id 가 경로 조각이 되므로 사람이 준 글자를 그대로 쓰지 않는다.
    let safe: String = sid
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(std::env::temp_dir().join(format!("moai-hook-{safe}.{what}")))
}
