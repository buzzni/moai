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

    // **규칙 4 는 자리도 트래커도 묻기 전에 본다**(moai-zis7) — 사람의 tmux 서버는 트래커와 무관하다.
    // 트래커를 찾은 뒤에만 보던 판은 스크래치패드로 `cd` 해 둔 세션(그 자리가 이미 지워졌어도)의
    // `tmux kill-server` 를 그대로 보냈고, 막을 때마다 옆 워크트리를 겹쳐 다시 재는 값(`settle`)을
    // 치렀다 — 스냅샷이 바뀌어도 답이 같은 판정이다(리뷰 moai-ju21.70g).
    if event == Event::PreToolUse
        && let crate::hook::Call::Shell(cmd) = crate::hook::Call::read(input.tool_name.as_deref(), &input.tool_input)
        && let refusal @ Decision::Deny(_) = crate::hook::guard_tmux(cmd)
    {
        return Ok(answer(event, refusal).into_iter().collect());
    }

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
    // **옮겨 갈 루트를 못 읽어도 규칙은 선다**(리뷰 moai-71ht.i1u). 트래커가 루트로 옮겨 가면서
    // (moai-y7go) 루트의 깨진 `config.toml` 하나가 저장소의 **모든** 워크트리에서 훅을 조용히
    // 껐다 — 고장의 크기가 규칙의 크기가 되면 안 된다. `moai` 자신은 그 자리에서 크게 실패하고
    // (사람이 그것을 본다), 훅은 이 자리의 트래커로 선다. 읽는 것은 갈라진 스냅샷이지만 아무
    // 말도 안 하는 것보다 낫다.
    let cwd = std::env::current_dir().ok()?;
    let repo = match Repo::discover() {
        Ok(repo) => repo,
        Err(_) => Repo::find_here(&cwd).ok()??,
    };
    let load = repo.read().ok()?;
    // **못 읽은 줄을 그대로 넘긴다.** 빈 슬라이스를 넘기면 보드에서
    // `unreadable_line` 경고만 조용히 빠지는데, 그것은 실린 보드 말고는
    // 에이전트가 알아낼 길이 없는 유일한 경고다 — 기준선도 같은 만큼
    // 낮게 잡혀 `Stop` 이 "늘었다" 를 영영 못 본다.
    let unreadable = load.unreadable();

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
            // 누구의 것인지 모르는 줄은 싣지 않는다 — 남의 일을 "압축 전부터 집고 있다" 로 떠안긴다(moai-4jsy).
            // `Stop` 이 붙드는 것과 같은 자로 잰다([`releasing`]).
            let (away, latest) = releasing(input, &repo, &load.issues);
            crate::hook::carried(&load.issues, latest.as_deref().unwrap_or(&load.issues), &repo.config, &away)
        }
        // 기준선만 적고 아무것도 싣지 않는다. 까닭은 `hook::Event` 에 있다.
        Event::SessionStart => {
            write_baseline(input, &repo, &load.issues, &unreadable);
            Decision::Pass
        }
        Event::UserPromptSubmit => once_per_session(input, &repo, "board", || {
            let now = model::now();
            let mut st = report::status(&load.issues, &unreadable, &repo.config, &now);
            // `moai status` 와 같은 알림을 싣는다(`agents_notice`) — 낡은 AGENTS.md 를 모르고
            // 시작하는 것이 바로 이 보드를 받는 새 세션이다. 세션의 셸 자리는 stdin 의 `cwd` 라
            // 이미 여기로 옮겨 왔다(`-C` 가 아니다).
            st.notices.extend(crate::cmd::init::agents_notice(repo.here(), false));
            st.notices.extend(crate::cmd::init::dotfile_notice(repo.here(), false));
            // 보드가 **정말 읽은 파일**을 댄다(`cmd::status::source_of` 와 같은 자) — 워크트리
            // 세션의 보드는 루트의 트래커에서 온다(moai-y7go).
            let source = crate::cmd::status::source_of(&repo);
            let lines =
                view::status(
                    &st,
                    &load.issues,
                    &repo.config,
                    &now,
                    &source,
                    &crate::worktree::Origin::default(),
                    0,
                );
            crate::hook::board(&lines)
        }),
        Event::PreToolUse => {
            use crate::hook::Call;
            let call = Call::read(input.tool_name.as_deref(), &input.tool_input);
            let cwd = cwd.clone();
            let (routes, there) = match call {
                Call::Shell(cmd) => route(&repo, cmd, &cwd),
                _ => (Vec::new(), Vec::new()),
            };
            let mine = |k: usize| !matches!(routes.get(k), Some(r) if *r != Route::Here);
            // **내미는 줄은 그 토막이 겨눈 트래커를 댄다**(moai-v9sa, 사용자 결정) — 사람이 친 `-C` 의
            // 글자가 아니라 [`route`] 가 푼 자리다. `Repo::find_from` 이 딸린 워크트리를 루트로 옮기니
            // (moai-y7go) 거절문이 워크트리의 스냅샷을 겨누는 길이 닫히고, `moai -C .`·`cd src && moai -C ..`
            // 처럼 어디서 쳤느냐에 따라 달라지는 상대 경로도 풀려 나온다.
            let toward = |k: usize| match routes.get(k) {
                Some(Route::There(n)) => there.get(*n).map(|r: &Repo| r.root.as_path()),
                _ => None,
            };
            let decision = settle(input, &repo, &load.issues, away_of(&repo, &load.issues), &|issues, away| match call {
                // 규칙의 차례는 `guard_shell_in` 이 정한다. 여기는 껍데기의 자리와 제 토막만 준다.
                // **세는 자리는 세션이 선 체크아웃이다**(moai-y7go) — 트래커는 루트로 옮겨 가지만
                // (`Repo::find_from`) 고치는 파일은 이 워크트리의 것이다. `repo.root` 로 세던 판은
                // 워크트리의 파일이 죄다 루트의 `.claude/worktrees/…` 밑으로 보여 규칙 2 가 통째로 꺼졌다.
                Call::Shell(cmd) => crate::hook::guard_shell_in(issues, &repo.config, away, repo.here(), &cwd, cmd, &mine, &toward),
                Call::Edits(path) => crate::hook::guard_edit(issues, &repo.config, away, repo.here(), path),
                Call::Review => crate::hook::guard_review(issues, &repo.config, away),
                Call::Other => Decision::Pass,
            });
            // 다른 트래커를 가리키는 토막은 **그 트래커가 본다**(moai-23ky). 판정을 잇는 차례는
            // `Decision::then` 이 정한다 — 막으면 남의 트래커는 묻지 않고, 남이 막으면 제 비춤을 버린다.
            let mut decision = decision;
            if let Call::Shell(cmd) = call {
                for (n, other) in there.iter().enumerate() {
                    decision = decision.then(|| {
                        let Ok(load) = other.read() else { return Decision::Pass };
                        let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                        settle(input, other, &load.issues, away_of(other, &load.issues), &|issues, away| {
                            crate::hook::guard_moai(issues, &other.config, away, cmd, &only, &|_| Some(other.root.as_path()))
                        })
                    });
                }
                // **한국어 글에는 다듬기를 비춘다**(moai-6rrb). 막는 답이 이긴다 — 막힌 명령은 글을
                // 안 넣었다. 트래커가 없는 자리를 가리킨 토막도 안 넣는다 — 그 `moai` 는 스스로 실패한다.
                // 깔렸는지는 비출 때만 장부를 읽는다.
                decision = decision.then(|| {
                    match crate::hook::korean_write(cmd, &|k| routes.get(k) != Some(&Route::Nowhere)) {
                        Some(at) => crate::hook::korean_notice(&at, &crate::cmd::skill::korean_missing(&repo.root)),
                        None => Decision::Pass,
                    }
                });
            }
            // **막지 않은 집기는 이 세션의 것으로 적는다**(moai-4jsy). 판정 뒤에 적는다 — 막힌 명령은
            // 안 돈다. **`--from` 에 질 집기는 안 적는다**(`hook::picked_in`) — 그 토막이 겨눈 트래커의 지금
            // 칸을 댄다. 적던 판은 진 쪽을 마지막으로 집은 세션으로 세어, 진 쪽이 이긴 쪽의 줄로 붙들리고
            // 막혔다(리뷰 moai-3k2d.1df). 명령이 돌기 전과 도는 사이에 옆이 집는 틈은 남는다.
            if let Call::Shell(cmd) = call
                && !decision.blocks()
            {
                let dirs = std::cell::OnceCell::new();
                let stands = |k: usize, id: &str| -> Option<String> {
                    let at = |issues: &[model::Issue]| issues.iter().find(|i| i.id == id).map(|i| i.status.as_str().to_string());
                    match dirs.get_or_init(|| crate::hook::aimed(cmd, &cwd)).get(k).cloned().flatten() {
                        None => at(&load.issues),
                        Some(dir) => at(&Repo::find_from(&dir).ok()??.read().ok()?.issues),
                    }
                };
                record_picks(input, &repo, &crate::hook::picked_in(cmd, &repo.config, &mine, &stands));
                for (n, other) in there.iter().enumerate() {
                    let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                    record_picks(input, other, &crate::hook::picked_in(cmd, &other.config, &only, &stands));
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
            // **누구의 것인지 모르는 줄로는 붙들지 않는다**(moai-ntl6). 에픽이 닫히는지와 집은 줄이 아직
            // 집혀 있는지는 옆까지 겹친 줄로 잰다(moai-8ema). 둘 다 [`releasing`] 이 잰다.
            let (away, latest) = releasing(input, &repo, &load.issues);
            crate::hook::closing(
                &load.issues,
                latest.as_deref().unwrap_or(&load.issues),
                &repo.config,
                &away,
                warnings,
                baseline(input, &repo),
            )
        }),
    };
    answer(event, decision)
}

/// 판정을 계약 JSON 한 줄로 옮긴다 — `Pass` 는 아무 말도 안 한다.
fn answer(event: Event, decision: Decision) -> Option<String> {
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
/// 한 일이 막힌다. 겹쳐 봐도 막힐 때만 막고, **까닭은 마지막으로 다시 본 판의 것**이다
/// (moai-15c2) — 겹친 줄과 모름을 함께 본 판이 지금 가장 참에 가깝다. 고칠 명령이 이 자리의
/// 트래커에 듣는 것은 [`crate::hook::Toward`] 가 따로 지킨다.
/// **겹쳐 보기는 막을 때만 치른다** — 지나가는 호출은 전과 같은 값이다.
///
/// **다시 본 판정이 안 막으면 풀린 것이다** — 비추는 줄(`Context`)도 푼 답이라 그대로 낸다.
/// `Pass` 만 풀린 것으로 치던 판은 `idea add` 하나를 곁들인 명령줄을 낡은 스냅샷의 거절로 도로
/// 막았다(moai-dw63.e31) — 그 거절은 이미 집은 일을 집으라고 시켰다.
fn settle(
    input: &Input,
    repo: &Repo,
    issues: &[model::Issue],
    base: crate::hook::Away,
    judge: &dyn Fn(&[model::Issue], &crate::hook::Away) -> Decision,
) -> Decision {
    let first = judge(issues, &base);
    if first == Decision::Pass {
        return first;
    }
    // **겹침과 모름을 한 판에 얹어 한 번 다시 본다**(moai-15c2, 사용자 결정). 따로 보던 판은 둘이
    // 함께일 때 풀릴 것을 못 풀었다 — 겹친 줄의 판정은 모름을 안 뺐고, 모름을 뺀 판정은 낡은 제
    // 스냅샷으로 쟀다. **겹친 줄은 겹친 목록의 이름으로 잰다** — 옆 이름도 제 이름도 그 목록에서
    // 읽는다(moai-m62u). `base` 는 제 스냅샷에 집은 줄이 없으면 워크트리를 안 읽어([`away_of`]) 제
    // 이름이 비는데, 겹쳐 보는 까닭이 바로 그 스냅샷이 main 의 집기를 모를 때다 — 빈 이름으로
    // 재던 판은 제 이름 워크트리의 멤버를 옆 에픽 워크트리에 넘겨 규칙 2 로 막았다(리뷰
    // moai-3k2d.1df). **겹쳐 보기는 막을 때만 치른다** — 비추기만 하는 답은 제 줄로 좁히기만 한다.
    let overlaid = first.blocks().then(|| crate::worktree::fresh(repo, issues.to_vec())).flatten();
    let seen = overlaid.is_some();
    // **빌려 쓴다** — 겹치지 않은 판의 줄은 부르는 쪽의 것 그대로다. 통째로 베끼던 판은 막거나 비추는
    // 호출마다 스냅샷 전체를 복제했고, 훅은 도구 호출마다 돈다.
    let (rows, mut narrow): (std::borrow::Cow<'_, [model::Issue]>, _) = match overlaid {
        Some((fresh, beside)) => (std::borrow::Cow::Owned(fresh), beside),
        None => (std::borrow::Cow::Borrowed(issues), base),
    };
    // **겹쳐 보기만으로 풀리면 거기서 끝낸다** — 모름을 재는 값(옆 스냅샷을 다시 읽고 세션의 기록을
    // 훑는 일)을 안 치른다. 겹치지 않은 판은 `first` 가 곧 이 답이라 다시 재지 않는다.
    let wide = if seen { judge(&rows, &narrow) } else { first };
    if wide == Decision::Pass {
        return wide;
    }
    // **모르는 줄도 같은 판에서 뺀다**(moai-ntl6, 사용자 결정 B) — 옆 워크트리가 쥐었을 일을 초점으로
    // 대지 않는다. **비추는 줄도 같은 자로 좁힌다** — 막지도 붙들지도 않기로 한 줄의 에픽을 제 물음으로
    // 비추면, 그 세션을 남의 에픽에 세우는 길로 보낸다(`idea promote -e <남의 에픽>`).
    let added = add_unsure(input, repo, &rows, &mut narrow);
    if !added {
        return wide;
    }
    // 막히면 **그 판정의 까닭**을 낸다 — 겹친 줄과 모름을 함께 본 판이 지금 가장 참에 가깝다.
    let again = judge(&rows, &narrow);
    // **좁힌 초점은 풀기만 한다**: 덜 좁힌 판이 안 막는데 좁혀서 막으면 덜 좁힌 답을 낸다. 둘을 함께
    // 본 판이 겹쳐 보기가 푼 것을 도로 막던 자리다 — 모름은 초점에서 빼기만 하고, 초점이 비면 규칙 2 가
    // 막는다. 사용자 결정 moai-4jsy 의 "새로 막는 일은 없다" 가 여기에도 선다.
    //
    // **`first` 를 따로 다시 대지 않는다** — `first` 가 안 막으면 겹쳐 보지도 않아(`first.blocks()`)
    // `wide` 가 곧 `first` 다. 두 자로 적던 판은 같은 것을 두 번 물었다.
    if again.blocks() && !wide.blocks() { wide } else { again }
}

/// **옆 워크트리가 쥔 일은 제 초점이 아니다** (`hook::held`). 워크트리 목록은 집은 것이 있을 때만
/// 읽는다 — 훅은 도구 호출마다 돌고, 집은 것이 없으면 뺄 것도 없다.
fn away_of(repo: &Repo, issues: &[model::Issue]) -> crate::hook::Away {
    if report::wip(issues, &repo.config).is_empty() {
        return crate::hook::Away::default();
    }
    // **제 이름은 세션이 선 체크아웃에서 읽는다 — 트래커의 자리가 아니다**(moai-y7go). 트래커를
    // 찾는 길이 딸린 워크트리를 루트로 옮기므로(`Repo::find_from`) `repo.root` 는 늘 루트다. 거기서
    // 이름을 읽으면 워크트리 안에서 도는 세션이 제 이름을 잃고, 제 워크트리가 쥔 일이 통째로 "옆의
    // 것" 이 되어 규칙 2 가 그 자리의 쓰기를 막는다.
    crate::worktree::away(repo.here())
}

/// `away` 에 **누구의 것인지 모르는** 집은 줄(`hook::unsure`)을 더한다 — 옆 딸린 워크트리가 쥐었을 수
/// 있거나 다른 세션이 마지막으로 집은 줄이다. 이 세션이 마지막으로 집은 줄도 함께 싣는다(`Away::picked`) —
/// 모르는 줄 밑에 있어도 제 것이다. 제 이름은 `away` 의 것을 쓴다. 더한 것이 없으면 거짓이다.
fn add_unsure(input: &Input, repo: &Repo, issues: &[model::Issue], away: &mut crate::hook::Away) -> bool {
    let elsewhere = crate::worktree::held_elsewhere(repo.here(), issues, &repo.config);
    let picks = read_picks(input, repo, issues);
    let unsure = crate::hook::unsure(issues, &repo.config, &elsewhere, &away.own, &picks);
    if unsure.is_empty() {
        return false;
    }
    away.unsure.extend(unsure);
    away.picked.extend(picks.mine);
    true
}

/// 풀기만 하는 판정(`Stop` 과 접힌 뒤 싣는 것)의 자 — 누구의 것인지 모르는 줄을 더한 `Away`
/// ([`add_unsure`])와, 집은 줄이 아직 집혀 있는지·에픽이 닫히는지 되짚을 겹친 줄(`worktree::fresh`,
/// moai-8ema·리뷰 moai-dw63.nzw)이다. 둘 다 집은 것이 남을 때만 잰다 — `Stop` 은 턴마다 돈다.
///
/// **두 자리가 한 자를 쓴다**(리뷰 moai-3k2d.1df) — 따로 적던 판은 접힌 뒤 싣는 쪽이 되짚기를 빠뜨려,
/// main 이 이미 닫은 줄을 "압축 전부터 집고 있다" 로 실었다.
fn releasing(input: &Input, repo: &Repo, issues: &[model::Issue]) -> (crate::hook::Away, Option<Vec<model::Issue>>) {
    let mut away = away_of(repo, issues);
    if crate::hook::held(issues, &repo.config, &away).is_empty() {
        return (away, None);
    }
    let still = !add_unsure(input, repo, issues, &mut away) || !crate::hook::held(issues, &repo.config, &away).is_empty();
    let latest = still.then(|| crate::worktree::fresh(repo, issues.to_vec())).flatten().map(|(fresh, _)| fresh);
    (away, latest)
}

/// 세션의 집기를 적는 자리(moai-4jsy) — **같은 저장소의 같은 트래커면 어느 워크트리에서 재도 같은
/// 자리다**(`worktree::tracker_place` — 공용 git 디렉터리와 그 안의 트래커 자리). 집기는 루트에서 치고
/// 일은 딸린 워크트리에서 하므로, 워크트리마다 제 뿌리로 잡으면 루트에서 적은 것을 워크트리에서 못
/// 읽는다. main 워크트리의 뿌리로 잡던 판은 main 이 없는 맨몸 저장소에서 그렇게 갈렸다(리뷰
/// moai-3k2d.1df). 세션마다 파일 하나다 — 여럿이 한 파일에 덧붙이면 겨룬다.
///
/// **std 의 해셔를 안 쓴다**(moai-2vrw, `text::fnv1a64`) — 세션마다 따로 뜨는 훅이 같은 자리를 찾아야
/// 하는데, `DefaultHasher` 는 러스트 판마다 값이 달라져도 된다고 문서가 밝혀 다시 빌드한 바이너리가 옛
/// 기록을 못 찾는다.
fn picks_dir(repo: &Repo) -> std::path::PathBuf {
    let key = match crate::worktree::tracker_place(&repo.root) {
        Some((common, rel)) => crate::text::fnv1a64_from(
            crate::text::fnv1a64_from(crate::text::fnv1a64(common.as_os_str().as_encoded_bytes()), &[0]),
            rel.as_os_str().as_encoded_bytes(),
        ),
        None => {
            let root = std::fs::canonicalize(&repo.root).unwrap_or_else(|_| repo.root.clone());
            crate::text::fnv1a64(root.as_os_str().as_encoded_bytes())
        }
    };
    std::env::temp_dir().join(format!("moai-picks-{key:016x}"))
}

/// 이 세션이 `ids` 를 지금 집었다고 적는다. 못 적으면 조용히 넘어간다 — 빠진 기록은 전과 같은 판정이다.
fn record_picks(input: &Input, repo: &Repo, ids: &[(String, bool)]) {
    use std::io::Write;
    if ids.is_empty() {
        return;
    }
    let Some(sid) = safe_sid(input) else { return };
    let dir = picks_dir(repo);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    // **벽시계로 적는다 — `model::now` 가 아니다.** 그쪽은 `MOAI_NOW` 로 멈춰 초 단위라, 두 세션의
    // 집기가 같은 때로 서서 누가 마지막인지 못 가린다. 이 기록은 트래커가 아니라 이 기계의 표다.
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else { return };
    let now = now.as_nanos();
    // 줄의 칸 시각(`status_since`)과 견줄 때는 같은 시계로 잰다 — `MOAI_NOW` 가 서면 그것이다.
    let stamp = crate::model::parse_rfc3339(&model::now());
    let lines: String = ids.iter().map(|(id, sure)| crate::hook::Picks::line(now, stamp, id, *sure)).collect();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(&sid)) {
        let _ = f.write_all(lines.as_bytes());
    }
    prune_picks(&dir, &sid);
}

/// **오래 안 적힌 세션의 기록은 치운다**(리뷰 moai-3k2d.1df). 읽는 쪽([`read_picks`])은 판정마다 이 디렉터리를
/// 통째로 읽는데, 세션마다 파일이 하나씩 쌓이고 아무도 안 지우면 그 값이 기계가 떠 있는 동안 는다. 두 주 넘게
/// 한 줄도 안 적은 세션의 기록을 지운다 — 지운 줄은 기록이 없던 때처럼 판정한다. 적을 때만 치운다 — 드물다.
fn prune_picks(dir: &Path, sid: &str) {
    const KEEP: std::time::Duration = std::time::Duration::from_secs(14 * 24 * 60 * 60);
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name() == sid {
            continue;
        }
        let stale = entry.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age > KEEP);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 적어 둔 집기를 읽어 이 세션의 눈으로 가른다 — 가르는 자는 `hook::Picks::fold` 다. 여기는 읽기만 한다.
/// 기록이 끝났는지는 `issues` 의 칸 시각으로 잰다.
fn read_picks(input: &Input, repo: &Repo, issues: &[model::Issue]) -> crate::hook::Picks {
    let Some(me) = safe_sid(input) else { return Default::default() };
    let Ok(dir) = std::fs::read_dir(picks_dir(repo)) else { return Default::default() };
    // **보통 파일만 읽는다** — 임시 디렉터리라 파이프가 서 있으면 여는 자리에서 훅이 멈추고, 링크는 남의
    // 파일을 읽힌다(리뷰 moai-3k2d.1df). 적는 쪽은 보통 파일만 만든다.
    let files = dir.filter_map(Result::ok).filter(|entry| entry.file_type().is_ok_and(|t| t.is_file())).filter_map(|entry| {
        let text = std::fs::read_to_string(entry.path()).ok()?;
        Some((entry.file_name().to_string_lossy().into_owned(), text))
    });
    let since: std::collections::BTreeMap<&str, i64> = issues
        .iter()
        .filter_map(|i| Some((i.id.as_str(), crate::model::parse_rfc3339(&i.status_since)?)))
        .collect();
    crate::hook::Picks::fold(&me, files, &|id| since.get(id).copied())
}

/// 세션 id 를 경로 조각으로 — 사람이 준 글자를 그대로 쓰지 않는다.
fn safe_sid(input: &Input) -> Option<String> {
    let safe: String = input
        .session_id
        .as_deref()?
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    (!safe.is_empty()).then_some(safe)
}

/// 껍데기 토막 하나를 판정할 트래커.
///
/// **같은 저장소 안은 모두 세션의 눈이다**(moai-y7go). 트래커를 찾는 길이 딸린 워크트리를 루트로
/// 옮기므로([`crate::store::Repo::find_from`]) `-C <워크트리>`·`cd <워크트리>` 는 세션이 이미 보는
/// 그 트래커를 가리킨다 — 판정도 쓰기도 한 파일이다. 워크트리 세션이 `-C <루트>` 로 가리켜도 같다:
/// 안 그러면 `-C <루트>` 한 번으로 규칙 1 을 넘는다. 누구의 초점인가는 여전히 **이름**이 가른다
/// (`away_of` 가 `Repo::here` 로 읽는다), 가리킨 디렉터리가 아니다.
///
/// [`Route::There`] 는 그래서 **다른 저장소**의 트래커만 받는다 — 등록한 옆 프로젝트다(moai-23ky).
/// 한때 같은 저장소의 옆 워크트리도 여기로 갔는데, 그때는 그 워크트리의 `.moai` 가 따로 있었다.
#[derive(Debug, PartialEq)]
enum Route {
    /// 세션 자리의 트래커 — 같은 저장소 안이면 어디를 가리켜도 여기다(`worktree::same_repo`).
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
            // **없는 자리는 어디인지 모른다 — 세션의 눈으로 본다.** `Nowhere` 로 보내던 판은
            // `mkdir d && moai -C d add`·`mkdir d && cd d && moai add` 를 아무도 판정하지 않았는데,
            // 실행할 때는 `d` 가 있어 `moai` 가 위로 찾아 이 트래커에 세운다 — 규칙 1 이 샜다.
            // `cd /없는곳; moai add` 도 `cd` 가 실패해 세션 자리에서 돈다.
            if !dir.is_dir() {
                return Route::Here;
            }
            let Ok(Some(found)) = Repo::find_from(&dir) else {
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
    // 세션 id 가 경로 조각이 되므로 사람이 준 글자를 그대로 쓰지 않는다.
    let safe = safe_sid(input)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo.dir().hash(&mut h);
    let at = h.finish();
    Some(std::env::temp_dir().join(format!("moai-hook-{safe}-{at:x}.{what}")))
}
