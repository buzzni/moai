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
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

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
    let unreadable = load.unreadable();
    // **옆 워크트리가 쥔 일은 제 초점이 아니다** (`hook::held`). 워크트리 목록은 집은 것이
    // 있을 때만 읽는다 — 훅은 도구 호출마다 돌고, 집은 것이 없으면 뺄 것도 없다.
    let away = || {
        if report::wip(&load.issues, &repo.config).is_empty() {
            std::collections::BTreeSet::new()
        } else {
            crate::worktree::away(&repo.root)
        }
    };

    let decision = match event {
        // **접힌 뒤는 같은 세션이다.** 기준선을 다시 적으면 접기 전에 늘린
        // 경고가 물려받은 빚에 묻혀 `Stop` 이 못 본다. 대신 보드의 표를
        // 지워 다음 프롬프트가 보드를 다시 싣게 한다 — 접힐 때 같이 떨어졌다.
        Event::SessionStart if input.source.as_deref() == Some("compact") => {
            // **없을 때만 적는다.** 여는 훅이 안 돌았던 세션(도중에 심었거나 임시
            // 디렉터리가 비워졌다)은 여기서 적지 않으면 `Stop` 이 끝까지 견줄 것이 없다.
            if baseline(input, &repo).is_none() {
                write_baseline(input, &repo, &load.issues, &unreadable);
            }
            if let Some(path) = session_file(input, &repo, "board") {
                let _ = std::fs::remove_file(path);
            }
            crate::hook::carried(&load.issues, &repo.config, &away())
        }
        // 기준선만 적고 아무것도 싣지 않는다. 까닭은 `hook::Event` 에 있다.
        Event::SessionStart => {
            write_baseline(input, &repo, &load.issues, &unreadable);
            Decision::Pass
        }
        Event::UserPromptSubmit => once_per_session(input, &repo, "board", || {
            let now = model::now();
            let st = report::status(&load.issues, &unreadable, &repo.config, &now);
            let lines =
                view::status(
                    &st,
                    &load.issues,
                    &repo.config,
                    &now,
                    ".moai/issues.jsonl",
                    &crate::worktree::Origin::default(),
                    0,
                );
            crate::hook::board(&lines)
        }),
        Event::PreToolUse => {
            use crate::hook::Call;
            let call = Call::read(input.tool_name.as_deref(), &input.tool_input);
            let cwd = std::env::current_dir().unwrap_or_else(|_| repo.root.clone());
            let (routes, there) = match call {
                Call::Shell(cmd) => route(&repo, cmd, &cwd),
                _ => (Vec::new(), Vec::new()),
            };
            let mine = |k: usize| !matches!(routes.get(k), Some(r) if *r != Route::Here);
            let decision = settle(&repo, &load.issues, &away, &|issues, away| match call {
                // 규칙의 차례는 `guard_shell_in` 이 정한다. 여기는 껍데기의 자리와 제 토막만 준다.
                Call::Shell(cmd) => crate::hook::guard_shell_in(issues, &repo.config, away, &repo.root, &cwd, cmd, &mine),
                Call::Edits(path) => crate::hook::guard_edit(issues, &repo.config, away, &repo.root, path),
                Call::Review => crate::hook::guard_review(issues, &repo.config, away),
                Call::Other => Decision::Pass,
            });
            // 다른 트래커를 가리키는 토막은 **그 트래커가 본다**(moai-23ky).
            let mut decision = decision;
            if let Call::Shell(cmd) = call {
                for (n, other) in there.iter().enumerate() {
                    if decision != Decision::Pass {
                        break;
                    }
                    let Ok(theirs) = other.read() else { continue };
                    let away = || {
                        if report::wip(&theirs.issues, &other.config).is_empty() {
                            BTreeSet::new()
                        } else {
                            crate::worktree::away(&other.root)
                        }
                    };
                    let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                    decision = settle(other, &theirs.issues, &away, &|issues, away| {
                        crate::hook::guard_moai(issues, &other.config, away, cmd, &only)
                    });
                }
            }
            decision
        }
        // **이미 한 번 붙들었으면 보낸다.** 이 표를 안 보면 무한히 돈다.
        Event::Stop if input.stop_hook_active => Decision::Pass,
        Event::Stop => once_per_session(input, &repo, "stop", || {
            let now = model::now();
            let st = report::status(&load.issues, &unreadable, &repo.config, &now);
            // **고칠 것만 센다.** 알림(쌓인 생각·미뤄 둔 것)은 `notices` 에 따로
            // 있다 — 여기 섞이던 때 `defer` 만 해도 "경고가 늘었다" 로 세션이
            // 붙들렸다(moai-c8lb). 기준선도 같은 자로 잰다.
            let warnings: usize = st.warnings.iter().map(|w| w.count).sum();
            // **누구의 것인지 모르는 줄로는 붙들지 않는다**(moai-ntl6). 집은 것이 남을 때만 옆
            // 스냅샷을 읽는다 — `Stop` 은 턴마다 돈다.
            let mut away = away();
            if !crate::hook::held(&load.issues, &repo.config, &away).is_empty() {
                away.extend(unsure_of(&repo, &load.issues));
            }
            crate::hook::closing(&load.issues, &repo.config, &away, warnings, baseline(input, &repo))
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

/// 판정하되, **막으면 옆 워크트리와 겹쳐 한 번 더 본다**(moai-w2iy).
///
/// 워크트리의 스냅샷은 main 에서 방금 세우고 집은 줄을 모른다 — 그것만 보고 막으면 시킨 대로
/// 한 일이 막힌다. 겹쳐 봐도 막힐 때만 막고, 까닭은 제 스냅샷의 것을 낸다(고칠 명령이 이 자리의
/// 트래커에 듣는다). **겹쳐 보기는 막을 때만 치른다** — 지나가는 호출은 전과 같은 값이다.
fn settle(
    repo: &Repo,
    issues: &[model::Issue],
    away: &dyn Fn() -> BTreeSet<String>,
    judge: &dyn Fn(&[model::Issue], &BTreeSet<String>) -> Decision,
) -> Decision {
    let first = judge(issues, &away());
    let Decision::Deny(_) = first else { return first };
    if let Some((fresh, fresh_away)) = crate::worktree::fresh(repo, issues.to_vec())
        && judge(&fresh, &fresh_away) == Decision::Pass
    {
        return Decision::Pass;
    }
    // **누구의 것인지 모르는 줄을 빼고 한 번 더 본다**(moai-ntl6, 사용자 결정 B). 좁은 초점으로도
    // 막히면 그 까닭을 낸다 — 옆 워크트리가 쥐었을 일을 초점으로 대지 않는다.
    let unsure = unsure_of(repo, issues);
    if unsure.is_empty() {
        return first;
    }
    let mut narrow = away();
    narrow.extend(unsure);
    match judge(issues, &narrow) {
        deny @ Decision::Deny(_) => deny,
        Decision::Pass => Decision::Pass,
        _ => first,
    }
}

/// 옆 딸린 워크트리가 쥐었을 수 있어 **누구의 것인지 모르는** 집은 줄(`hook::unsure`).
fn unsure_of(repo: &Repo, issues: &[model::Issue]) -> BTreeSet<String> {
    let (elsewhere, own) = crate::worktree::held_elsewhere(&repo.root, issues, &repo.config);
    crate::hook::unsure(issues, &repo.config, &elsewhere, &own)
}

/// 껍데기 토막 하나를 판정할 트래커.
///
/// **누구의 눈으로 보는가는 딸린 워크트리가 정한다.** 세션과 가리킨 곳 중 딸린 워크트리가 있으면
/// 그 워크트리의 일이다 — 이름이 곧 거기서 하는 일이다. main 은 모두의 집기가 모이는 자리라 그
/// 눈으로는 워크트리의 일이 "옆의 것" 이 된다. 그래서 워크트리 세션이 `-C <main>` 으로 가리키면
/// 세션의 눈(`Here`)으로 보고 — 안 그러면 `-C <main>` 한 번으로 규칙 1 을 넘는다 — main 에 선
/// 세션이 `cd <워크트리>` 로 들어가면 그 워크트리의 눈(`There`)으로 본다. 에이전트 스레드는 자리가
/// main 으로 돌아와 늘 이 모양으로 친다.
#[derive(Debug, PartialEq)]
enum Route {
    /// 세션 자리의 트래커 — 같은 저장소의 main 워크트리를 가리켜도 여기다(`worktree::same_repo`).
    Here,
    /// 가리킨 자리에 트래커가 없다. `moai` 가 스스로 실패하니 아무도 판정하지 않는다.
    Nowhere,
    /// 다른 트래커 — `route` 가 돌려준 목록의 자리.
    There(usize),
}

/// 토막마다 판정할 트래커를 가른다(`hook::aimed`). 가리킨 곳이 없으면 디스크를 안 짚는다 —
/// 대부분의 호출은 `-C`·`cd` 가 없어 여기서 아무것도 안 읽는다.
fn route(repo: &Repo, cmd: &str, cwd: &Path) -> (Vec<Route>, Vec<Repo>) {
    let same = |a: &Path, b: &Path| a == b || std::fs::canonicalize(a).ok().zip(std::fs::canonicalize(b).ok()).is_some_and(|(x, y)| x == y);
    let mut there: Vec<Repo> = Vec::new();
    let routes = crate::hook::aimed(cmd, cwd)
        .into_iter()
        .map(|dir| {
            let Some(dir) = dir else { return Route::Here };
            let Ok(Some(found)) = Repo::find_from(&dir).map(|r| r.filter(|_| dir.is_dir())) else {
                return Route::Nowhere;
            };
            if same(&found.root, &repo.root)
                || (!crate::worktree::is_linked(&found.root) && crate::worktree::same_repo(&found.root, &repo.root))
            {
                return Route::Here;
            }
            match there.iter().position(|r| same(&r.root, &found.root)) {
                Some(n) => Route::There(n),
                None => {
                    there.push(found);
                    Route::There(there.len() - 1)
                }
            }
        })
        .collect();
    (routes, there)
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
fn write_baseline(
    input: &Input,
    repo: &Repo,
    issues: &[crate::model::Issue],
    unreadable: &[report::Unreadable],
) {
    let Some(path) = session_file(input, repo, "warn") else {
        return;
    };
    let now = model::now();
    let st = report::status(issues, unreadable, &repo.config, &now);
    // `Stop` 과 같은 자 — 알림은 안 센다.
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
