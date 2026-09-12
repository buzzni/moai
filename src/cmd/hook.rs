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

/// 도구 호출을 막을 때의 모양. **까닭이 곧 화면에 뜨는 글이다** — 막힌 쪽이
/// 읽고 그대로 고칠 수 있어야 한다.
#[derive(Serialize)]
struct Refusal<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: RefusalBody<'a>,
}

#[derive(Serialize)]
struct RefusalBody<'a> {
    #[serde(rename = "hookEventName")]
    event: &'a str,
    #[serde(rename = "permissionDecision")]
    decision: &'a str,
    #[serde(rename = "permissionDecisionReason")]
    reason: String,
}

/// 턴을 끝내지 않게 붙드는 모양.
#[derive(Serialize)]
struct Hold {
    decision: &'static str,
    reason: String,
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
    // **못 읽은 줄을 그대로 넘긴다.** 빈 슬라이스를 넘기면 보드에서
    // `unreadable_line` 경고만 조용히 빠지는데, 그것은 실린 보드 말고는
    // 에이전트가 알아낼 길이 없는 유일한 경고다 — 기준선도 같은 만큼
    // 낮게 잡혀 `Stop` 이 "늘었다" 를 영영 못 본다.
    let unreadable: Vec<usize> = load.errors.iter().map(|e| e.line).collect();

    let decision = match event {
        // 기준선만 적고 아무것도 싣지 않는다. 까닭은 `hook::Event` 에 있다.
        Event::SessionStart => {
            write_baseline(input, &repo, &load.issues, &unreadable);
            Decision::Pass
        }
        Event::UserPromptSubmit => once_per_session(input, &repo, "board", || {
            let now = model::now();
            let st = report::status(&load.issues, &unreadable, &repo.config, &now);
            let lines =
                view::status(&st, &load.issues, &repo.config, &now, ".moai/issues.jsonl");
            crate::hook::board(&lines)
        }),
        Event::PreCompact => crate::hook::carried(&load.issues, &repo.config),
        Event::PreToolUse => {
            match crate::hook::Call::read(input.tool_name.as_deref(), &input.tool_input) {
                crate::hook::Call::Shell(cmd) => {
                    // 규칙의 차례는 `guard_shell` 이 정한다. 여기는 껍데기의 자리만 준다.
                    let cwd = std::env::current_dir().unwrap_or_else(|_| repo.root.clone());
                    crate::hook::guard_shell(&load.issues, &repo.config, &repo.root, &cwd, cmd)
                }
                crate::hook::Call::Edits(path) => {
                    crate::hook::guard_edit(&load.issues, &repo.config, &repo.root, path)
                }
                crate::hook::Call::Review => {
                    crate::hook::guard_review(&load.issues, &repo.config)
                }
                crate::hook::Call::Other => Decision::Pass,
            }
        }
        // **이미 한 번 붙들었으면 보낸다.** 이 표를 안 보면 무한히 돈다.
        Event::Stop if input.stop_hook_active => Decision::Pass,
        Event::Stop => once_per_session(input, &repo, "stop", || {
            let now = model::now();
            let st = report::status(&load.issues, &unreadable, &repo.config, &now);
            let warnings: usize = st.warnings.iter().map(|w| w.count).sum();
            crate::hook::closing(&load.issues, &repo.config, warnings, baseline(input, &repo))
        }),
    };

    match decision {
        Decision::Pass => None,
        Decision::Context(context) => serde_json::to_string(&Out {
            specific: Specific { event: event.wire(), context },
        })
        .ok(),
        Decision::Deny(reason) => serde_json::to_string(&Refusal {
            specific: RefusalBody { event: event.wire(), decision: "deny", reason },
        })
        .ok(),
        Decision::Block(reason) => {
            serde_json::to_string(&Hold { decision: "block", reason }).ok()
        }
    }
}

/// 이 세션이 열릴 때 적어 둔 경고 수. 없으면 견줄 것이 없다.
fn baseline(input: &Input, repo: &Repo) -> Option<usize> {
    let path = session_file(input, repo, "warn")?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// 세션마다 한 번만. **매 프롬프트에 보드를 붙이면 그것대로 자리값을 잃는다.**
///
/// 표를 남기는 자리는 시스템 임시 디렉터리다. `.moai/` 에 두면 세션 부스러기가
/// 저장소에 쌓이고, 그것을 `.gitignore` 로 막는 일이 또 생긴다.
fn once_per_session(
    input: &Input,
    repo: &Repo,
    what: &str,
    make: impl FnOnce() -> Decision,
) -> Decision {
    let Some(path) = session_file(input, repo, what) else {
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
fn write_baseline(input: &Input, repo: &Repo, issues: &[crate::model::Issue], unreadable: &[usize]) {
    let Some(path) = session_file(input, repo, "warn") else {
        return;
    };
    let now = model::now();
    let st = report::status(issues, unreadable, &repo.config, &now);
    let n: usize = st.warnings.iter().map(|w| w.count).sum();
    let _ = std::fs::write(path, n.to_string());
}

/// 세션의 표가 놓이는 자리.
///
/// **저장소도 키에 든다.** 한 세션이 저장소 둘을 오가는 것은 드물지 않은데,
/// 세션 id 만으로 키를 잡으면 둘째 저장소는 첫째의 표를 보고 제 보드를
/// 영영 안 싣는다 — 조용히 빠지는 쪽이라 아무도 못 알아챈다.
fn session_file(input: &Input, repo: &Repo, what: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    let sid = input.session_id.as_deref()?;
    // 세션 id 가 경로 조각이 되므로 사람이 준 글자를 그대로 쓰지 않는다.
    let safe: String = sid
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    if safe.is_empty() {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo.dir().hash(&mut h);
    let at = h.finish();
    Some(std::env::temp_dir().join(format!("moai-hook-{safe}-{at:x}.{what}")))
}
