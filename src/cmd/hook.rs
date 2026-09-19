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
            crate::hook::Away::default()
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
            // 누구의 것인지 모르는 줄은 싣지 않는다 — 남의 일을 "압축 전부터 집고 있다" 로 떠안긴다(moai-4jsy).
            let mut away = away();
            if !crate::hook::held(&load.issues, &repo.config, &away).is_empty() {
                away.unsure.extend(unsure_of(input, &repo, &load.issues));
            }
            crate::hook::carried(&load.issues, &repo.config, &away)
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
            st.notices.extend(crate::cmd::init::agents_notice(&repo.root, false));
            st.notices.extend(crate::cmd::init::dotfile_notice(&repo.root, false));
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
            let decision = settle(input, &repo, &load.issues, &away, &|issues, away| match call {
                // 규칙의 차례는 `guard_shell_in` 이 정한다. 여기는 껍데기의 자리와 제 토막만 준다.
                Call::Shell(cmd) => crate::hook::guard_shell_in(issues, &repo.config, away, &repo.root, &cwd, cmd, &mine),
                Call::Edits(path) => crate::hook::guard_edit(issues, &repo.config, away, &repo.root, path),
                Call::Review => crate::hook::guard_review(issues, &repo.config, away),
                Call::Other => Decision::Pass,
            });
            // 답마다 **그 답을 낸 트래커의 main** 으로 겨눈다 — 합친 뒤에 겨누면 남의 줄을 제 main 으로 보낸다.
            let decision = toward_main(decision, &repo, &load.issues);
            // 다른 트래커를 가리키는 토막은 **그 트래커가 본다**(moai-23ky). 판정을 잇는 차례는
            // `Decision::then` 이 정한다 — 막으면 남의 트래커는 묻지 않고, 남이 막으면 제 비춤을 버린다.
            let mut decision = decision;
            if let Call::Shell(cmd) = call {
                for (n, other) in there.iter().enumerate() {
                    decision = decision.then(|| {
                        let Ok(load) = other.read() else { return Decision::Pass };
                        let away = || {
                            if report::wip(&load.issues, &other.config).is_empty() {
                                crate::hook::Away::default()
                            } else {
                                crate::worktree::away(&other.root)
                            }
                        };
                        let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                        let said = settle(input, other, &load.issues, &away, &|issues, away| {
                            crate::hook::guard_moai(issues, &other.config, away, cmd, &only)
                        });
                        // 그 트래커가 딸린 워크트리면 그 main 으로 — main 에 선 세션이 `cd <워크트리> &&`
                        // 로 친 줄도 워크트리의 스냅샷에 쓰면 병합에서 겨룬다(리뷰 moai-ju21.70g).
                        toward_main(said, other, &load.issues)
                    });
                }
            }
            // **막지 않은 집기는 이 세션의 것으로 적는다**(moai-4jsy). 판정 뒤에 적는다 — 막힌 명령은
            // 안 돈다. 돌다 진 집기(`--from` 이 낡았다)도 적히는데, 진 쪽이 늦게 적었으면 그 줄은 이긴
            // 쪽에게 "모름" 이 된다 — 모름은 풀기만 하니 이긴 쪽의 `Stop` 이 그 줄로 안 붙들 뿐이다.
            if let Call::Shell(cmd) = call
                && !decision.blocks()
            {
                record_picks(input, &repo, &crate::hook::picked_in(cmd, &repo.config, &mine));
                for (n, other) in there.iter().enumerate() {
                    let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                    record_picks(input, other, &crate::hook::picked_in(cmd, &other.config, &only));
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
                away.unsure.extend(unsure_of(input, &repo, &load.issues));
            }
            // **에픽이 닫히는지는 옆까지 겹친 줄로 잰다**(moai-8ema). 트래커는 main 에서 쓰므로 딸린
            // 워크트리의 스냅샷은 갈라진 때에 멈춰 있다 — 그 사이 main 에서 끝낸 멤버를 아직 벌여 놓은
            // 것으로 읽으면, 미루는 순간 에픽이 닫히는 마지막 멤버에 "지금 안 할 것이면" 을 그냥 댄다.
            // 집은 것이 남을 때만 읽는다.
            let latest = (!crate::hook::held(&load.issues, &repo.config, &away).is_empty())
                .then(|| crate::worktree::fresh(&repo, load.issues.clone()))
                .flatten()
                .map(|(fresh, _)| fresh);
            let held = crate::hook::closing(
                &load.issues,
                latest.as_deref().unwrap_or(&load.issues),
                &repo.config,
                &away,
                warnings,
                baseline(input, &repo),
            );
            toward_main(held, &repo, &load.issues)
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

/// **딸린 워크트리에서는 내미는 명령을 주 워크트리의 트래커로 겨눈다**(moai-gyqh). 트래커는 main
/// 에서 쓴다 — 맨 `moai` 를 그대로 치면 워크트리의 스냅샷만 바뀌어, 루트는 집은 채로 남고 훅은 제
/// 스냅샷을 보고 조용해진다. 거절문도 같다 — 시킨 대로 친 줄이 병합에서 스냅샷을 겨루게 한다.
/// 말할 것이 있을 때만 자리를 잰다. `issues` 는 그 글을 낸 트래커의 줄이다.
///
/// **겨눌 곳이 그 줄을 알 때만 겨눈다**(리뷰 moai-ju21.70g). main 에 트래커가 없으면(이 가지에서 처음
/// `init` 했다) 아무 줄도 안 겨누고, main 이 모르는 id(워크트리에서 맨 `moai` 로 세운 줄)를 든 줄은
/// 그대로 둔다([`crate::hook::unsynced`]) — 겨누던 판은 시킨 대로 친 명령이 "못 찾았다"·"저장소가
/// 아니다" 로 끝났다.
fn toward_main(decision: Decision, repo: &Repo, issues: &[model::Issue]) -> Decision {
    if decision == Decision::Pass {
        return decision;
    }
    let Some(main) = crate::worktree::main_root(&repo.root) else { return decision };
    let Ok(Some(there)) = crate::store::read_snapshot(&main.join(".moai").join("issues.jsonl")) else {
        return decision;
    };
    let local = crate::hook::unsynced(issues, &there.issues);
    decision.map_text(|why| crate::hook::toward(why, &main, &|w| local.contains(w)))
}

/// 판정하되, **막으면 옆 워크트리와 겹쳐 한 번 더 본다**(moai-w2iy).
///
/// 워크트리의 스냅샷은 main 에서 방금 세우고 집은 줄을 모른다 — 그것만 보고 막으면 시킨 대로
/// 한 일이 막힌다. 겹쳐 봐도 막힐 때만 막고, 까닭은 제 스냅샷의 것을 낸다(고칠 명령이 이 자리의
/// 트래커에 듣는다). **겹쳐 보기는 막을 때만 치른다** — 지나가는 호출은 전과 같은 값이다.
///
/// **다시 본 판정이 안 막으면 풀린 것이다** — 비추는 줄(`Context`)도 푼 답이라 그대로 낸다.
/// `Pass` 만 풀린 것으로 치던 판은 `idea add` 하나를 곁들인 명령줄을 낡은 스냅샷의 거절로 도로
/// 막았다(moai-dw63.e31) — 그 거절은 이미 집은 일을 집으라고 시켰다.
fn settle(
    input: &Input,
    repo: &Repo,
    issues: &[model::Issue],
    away: &dyn Fn() -> crate::hook::Away,
    judge: &dyn Fn(&[model::Issue], &crate::hook::Away) -> Decision,
) -> Decision {
    let base = away();
    let first = judge(issues, &base);
    if first == Decision::Pass {
        return first;
    }
    if first.blocks()
        && let Some((fresh, names)) = crate::worktree::fresh(repo, issues.to_vec())
    {
        // 겹친 줄의 옆 이름만 바꾼다 — 제 이름과 겨루는 자는 그대로다(moai-m62u).
        let again = judge(&fresh, &crate::hook::Away { names, ..base.clone() });
        if !again.blocks() {
            return again;
        }
    }
    // **누구의 것인지 모르는 줄을 빼고 한 번 더 본다**(moai-ntl6, 사용자 결정 B). 좁은 초점으로도
    // 막히면 그 까닭을 낸다 — 옆 워크트리가 쥐었을 일을 초점으로 대지 않는다. **비추는 줄도 같은
    // 자로 좁힌다** — 막지도 붙들지도 않기로 한 줄의 에픽을 제 물음으로 비추면, 그 세션을 남의
    // 에픽에 세우는 길로 보낸다. 좁힌 초점은 풀기만 한다: 비추기만 하던 명령을 좁혀서 막지는 않는다.
    let unsure = unsure_of(input, repo, issues);
    if unsure.is_empty() {
        return first;
    }
    let mut narrow = base;
    narrow.unsure.extend(unsure);
    let again = judge(issues, &narrow);
    if again.blocks() && !first.blocks() { first } else { again }
}

/// 옆 딸린 워크트리가 쥐었을 수 있거나 다른 세션이 집어 **누구의 것인지 모르는** 집은 줄(`hook::unsure`).
fn unsure_of(input: &Input, repo: &Repo, issues: &[model::Issue]) -> BTreeSet<String> {
    let (elsewhere, own) = crate::worktree::held_elsewhere(&repo.root, issues, &repo.config);
    crate::hook::unsure(issues, &repo.config, &elsewhere, &own, &read_picks(input, repo))
}

/// 세션의 집기를 적는 자리(moai-4jsy) — **main 워크트리의 트래커로 키를 잡는다.** 집기는 루트에서
/// 치고 일은 딸린 워크트리에서 하므로, 트래커 자리로 잡으면 루트에서 적은 것을 워크트리에서 못 읽는다.
/// 세션마다 파일 하나다 — 여럿이 한 파일에 덧붙이면 겨룬다.
fn picks_dir(repo: &Repo) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let main = crate::worktree::main_root(&repo.root).unwrap_or_else(|| repo.root.clone());
    let main = std::fs::canonicalize(&main).unwrap_or(main);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    main.hash(&mut h);
    std::env::temp_dir().join(format!("moai-picks-{:x}", h.finish()))
}

/// 이 세션이 `ids` 를 지금 집었다고 적는다. 못 적으면 조용히 넘어간다 — 빠진 기록은 전과 같은 판정이다.
fn record_picks(input: &Input, repo: &Repo, ids: &[String]) {
    use std::io::Write;
    let Some(sid) = ids.first().and_then(|_| safe_sid(input)) else { return };
    let dir = picks_dir(repo);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    // **벽시계로 적는다 — `model::now` 가 아니다.** 그쪽은 `MOAI_NOW` 로 멈춰 초 단위라, 두 세션의
    // 집기가 같은 때로 서서 누가 마지막인지 못 가린다. 이 기록은 트래커가 아니라 이 기계의 표다.
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else { return };
    let now = now.as_nanos();
    let lines: String = ids.iter().map(|id| format!("{now}\t{id}\n")).collect();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(sid)) {
        let _ = f.write_all(lines.as_bytes());
    }
}

/// 적어 둔 집기를 이 세션의 눈으로 가른다 — 줄마다 **마지막으로** 집은 세션이 이긴다. 같은 때에 다른
/// 세션 둘이 집었으면 누구의 것인지 모르는 쪽(`theirs`)으로 둔다 — 그쪽은 풀기만 한다.
fn read_picks(input: &Input, repo: &Repo) -> crate::hook::Picks {
    let mut out = crate::hook::Picks::default();
    let Some(me) = safe_sid(input) else { return out };
    let Ok(dir) = std::fs::read_dir(picks_dir(repo)) else { return out };
    // id → (때, 이긴 세션). 같은 때에 다른 세션이 또 있으면 `None`.
    let mut last: std::collections::BTreeMap<String, (u128, Option<String>)> = Default::default();
    for entry in dir.filter_map(Result::ok) {
        let sid = entry.file_name().to_string_lossy().into_owned();
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        for line in text.lines() {
            let Some((at, id)) = line.split_once('\t') else { continue };
            let Ok(at) = at.parse::<u128>() else { continue };
            match last.get_mut(id) {
                Some((t, who)) if at == *t => {
                    if who.as_deref() != Some(sid.as_str()) {
                        *who = None;
                    }
                }
                Some((t, _)) if at < *t => {}
                _ => {
                    last.insert(id.to_string(), (at, Some(sid.clone())));
                }
            }
        }
    }
    for (id, (_, who)) in last {
        if who.as_deref() == Some(me.as_str()) {
            out.mine.insert(id);
        } else {
            out.theirs.insert(id);
        }
    }
    out
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
