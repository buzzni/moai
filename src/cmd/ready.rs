//! 지금 집을 수 있는 일.
//!
//! 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
//! 무엇이 ready 인지는 `report::ready` 가 정하고, 어떻게 보일지는 `view` 가
//! 정한다. 여기는 둘을 잇기만 한다.

use super::{Ctx, R};
use crate::projects::{Entry, Overview, Seen};
use crate::report;
use crate::store::Repo;
use crate::view;

/// `limit` 은 `-n` 이다(moai-efoc) — **집을 것만 자른다.** `held`·`outside` 는 목록이 왜 짧은지를
/// 대는 자리라 통째로 남는다: 그것까지 자르면 막힌 까닭이 앞 n 줄만 남아 도는 고리가 그 밖의
/// 막음을 "없다" 로 읽는다.
pub fn run(ctx: &Ctx, worktree: bool, limit: Option<usize>) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트마다 집을 것. 안이면 아래 그대로다 (결정 3).
    let Some(repo) = Repo::find(|| ctx.lang())? else {
        return overview(ctx, worktree, limit);
    };
    let crate::worktree::Gathered { load, origin, .. } = super::gather(ctx, &repo, worktree)?;
    let load = crate::archive::context(&repo.root, load)?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);

    // **겹친 줄로 고른다.** 옆 워크트리에서 집은 일은 거기서 `in_progress` 로 서
    // 있으므로, 같은 자(`report::ready`)가 그것을 저절로 뺀다 — 여기에 "남이 집은
    // 것" 을 가르는 `if` 를 따로 두지 않는다.
    let (picks, focus) = report::ready_in(&load.issues, &repo.config);
    // **내 것과 남의 것을 가른다**(moai-0zjo, 2026-10-02 사용자 결정). 사람은 여기서 풀어 자료로
    // 건넨다 — `report` 는 git 설정을 안 연다. **가를 줄이 있을 때만, 한 번 푼다**: `git` 을 두 번 띄운다.
    let asked = std::cell::OnceCell::new();
    let me = || asked.get_or_init(|| super::me_at(ctx, &repo.root)).as_ref();
    let judge = if picks.is_empty() { None } else { me() };
    let (mut picks, mut others) = report::by_owner(picks, judge);
    // `ready` 의 차례는 `report::ready_in` 이 이미 세웠다 — 여기서는 끊기만 한다. 내 것이 아닌 줄도
    // 같은 `-n` 으로 자른다(사용자 결정) — 여러 사람이 쓰는 저장소에서 화면이 남의 일로 길어지지 않는다.
    let more = crate::query::cut(&mut picks, limit);
    let others_more = crate::query::cut(&mut others, limit);
    // 미뤄 둔 것·빈 묶음에 막혀 못 집는 것. 안 대면 `ready` 가 까닭 없이 빈다.
    let held = report::held(&load.issues, &repo.config);
    if ctx.json {
        // **집은 줄의 소속을 푼다**(moai-wuzi) — 계획이 세우는 멤버는 `epic` 을 안 적으므로
        // 적힌 필드만 실으면 이 목록이 그 줄을 에픽 없는 줄로 낸다. `derived_epic` 이 읽는다.
        let ids: Vec<&str> = picks.iter().chain(others.iter().map(|(i, _)| i)).map(|i| i.id.as_str()).collect();
        let epics = report::handed_of(&load.issues, &ids);
        // 가려진 줄을 가르는 지도(moai-53s2) — 소속 지도와 같은 id 만 묻는다.
        let kinds = report::Kinds::of_ids(&load.issues, &ids);
        // **객체로 감싼다**(moai-w6n2, 사람이 정한 출력 계약). 맨 배열이던 때는 막혀 못 집는
        // 일을 실을 자리가 없어, 에이전트는 `[]` 를 "할 일이 없다" 로 읽었다. `.moai` 밖
        // 한눈 보기가 프로젝트마다 `ready` 키를 쓰는 것과 같은 이름이다.
        #[derive(serde::Serialize)]
        struct Said<'a> {
            ready: Vec<super::Row<'a>>,
            /// 집을 수 있지만 **내 것이 아닌** 줄 — 남의 것(`owner: "theirs"`)과 담당 없는 것
            /// (`"unowned"`). **늘 싣는다** — 없으면 `[]` 다. 사람을 모르면 가르지 않아 늘 빈다.
            others: Vec<super::Other<super::Row<'a>>>,
            held: Vec<Waiting<'a>>,
            /// 지금 도는 마일스톤. 없으면 키를 안 단다.
            #[serde(skip_serializing_if = "Vec::is_empty")]
            milestone: Vec<&'a str>,
            /// 그 밖이라 이번에 안 낸 일. `p0` 은 안 든다 — 핫픽스는 밖에서도 낸다.
            #[serde(skip_serializing_if = "Vec::is_empty")]
            outside: Vec<&'a str>,
        }
        // 사람 화면의 held 줄과 같은 셋. 빈 목록은 안 싣는다 — 미룬 막음이면 `empty` 가,
        // 빈 묶음이면 `by`·`undo` 가 늘 비어 키만 늘린다.
        #[derive(serde::Serialize)]
        struct Waiting<'a> {
            id: &'a str,
            #[serde(skip_serializing_if = "<[&str]>::is_empty")]
            by: &'a [&'a str],
            #[serde(skip_serializing_if = "<[&str]>::is_empty")]
            undo: &'a [&'a str],
            #[serde(skip_serializing_if = "<[&str]>::is_empty")]
            empty: &'a [&'a str],
        }
        return super::json_line(&Said {
            ready: picks
                .iter()
                .map(|i| super::Row::of(i, |_| None, epics.get(i.id.as_str()).copied(), &kinds).on(&origin))
                .collect(),
            others: others
                .iter()
                .map(|(i, owner)| super::Other {
                    row: super::Row::of(i, |_| None, epics.get(i.id.as_str()).copied(), &kinds).on(&origin),
                    owner: *owner,
                })
                .collect(),
            // **왜 짧은지를 기계에도 댄다**(moai-q04l). 사람 화면이 한 줄로 대는 것을 여기서
            // 빼면, `ready --json` 으로 도는 고리는 도는 마일스톤이 목록을 줄인 것을 "할 일이
            // 없다" 로 읽는다 — `held` 를 객체로 감싼 것과 같은 까닭이다(moai-w6n2).
            // 도는 것이 없으면 키를 안 단다.
            milestone: focus.running.iter().map(|m| m.id.as_str()).collect(),
            outside: focus.outside.iter().map(|i| i.id.as_str()).collect(),
            held: held.iter().map(|h| Waiting { id: &h.issue.id, by: &h.by, undo: &h.undo, empty: &h.empty }).collect(),
        });
    }

    // 첫 칸도 아니고 끝나지도 않은 것 = 누군가 이미 잡고 있는 것. **그 가운데 내 것만 센다**(moai-0zjo 리뷰) —
    // 이 줄은 "그것부터 끝내라" 고 권하는데, 남이 집은 줄을 끝내라는 말은 규칙 5 가 막는 길로 보낸다.
    // `prime` 의 집은 것·훅 초점과 같은 자다(2026-10-02 사용자 결정). 모르면 가르지 않는다.
    let mut wip = report::wip(&load.issues, &repo.config);
    if !wip.is_empty()
        && let Some(me) = me()
    {
        wip.retain(|i| report::owner(me, i).is_none());
    }

    let screen = view::Screen::new(ctx.lang()).at(ctx.clock()).over(&origin);
    let others = view::Others {
        rows: &others,
        more: others_more,
        naming: repo.config.naming,
        column: repo.config.started_status(),
        from: repo.config.first_status(),
    };
    Ok(view::ready(&picks, more, others, &report::epic_labels(&load.issues), &wip, &held, &focus, screen))
}

/// 등록한 프로젝트마다 집을 수 있는 일. 무엇이 ready 인지는 프로젝트마다 같은 자
/// (`report::ready`)가 **그 프로젝트의 줄만 보고** 정한다 — 남의 프로젝트에서 집은
/// 일이 이쪽의 막음을 풀거나 걸지 않는다.
///
/// **못 읽는 줄이 있어도 0 으로 끝난다** (`status::overview` 와 같은 까닭). 그 줄은
/// 프로젝트 줄 밑에 수로 말하고, 어느 줄인지는 그 프로젝트의 `show` 가 낸다.
///
/// `-n` 은 **프로젝트마다** 자른다 — 한 프로젝트의 일이 다른 프로젝트의 몫을 밀어내지 않는다.
/// 사람 화면은 그 가운데서도 앞 몇 줄만 그리고(`view::projects_ready`) 나머지를 "N건 더" 로 센다 —
/// `-n` 에 잘린 줄도 그 셈에 든다([`view::Picks::more`]).
fn overview(ctx: &Ctx, worktree: bool, limit: Option<usize>) -> R<Vec<String>> {
    let (reg, projects) = super::registered(ctx, worktree)?;
    let seen: Vec<Seen<view::Picks>> = projects
        .iter()
        .map(|p| {
            p.seen(|repo, load| {
                // **저장소 안의 `ready` 와 같은 자다.** 도는 마일스톤이 목록을 줄였으면 그
                // 까닭도 함께 받는다 — 여기서 `ready` 만 부르면 한눈 보기의 목록만 말없이
                // 짧아지고, 그 짧아짐이 "할 일이 없다" 로 읽힌다.
                //
                // **문맥도 같다**(moai-bth3 리뷰) — 안쪽 `ready` 는 아카이브를 겹쳐 읽는다(`archive::context`). 여기만
                // 산 줄로 고르면 옮겨 둔 멤버로 도는 마일스톤을 안 도는 것으로 읽어, 안쪽이 밖으로 미룬 일을 집을 것으로
                // 내민다. 고른 줄은 다 산 줄이라(옮긴 줄은 닫혀 있다) 이 프로젝트의 줄로 되짚어 담는다.
                let archived = crate::archive::read(&repo.root).map(|l| l.issues).unwrap_or_default();
                let opaque: std::collections::BTreeSet<&str> =
                    load.errors.iter().filter_map(|e| e.id.as_deref()).collect();
                let context = report::with_archive(&load.issues, &archived, &opaque);
                let (picks, focus) = report::ready_in(&context, &repo.config);
                let live: std::collections::BTreeMap<&str, &crate::model::Issue> =
                    load.issues.iter().map(|i| (i.id.as_str(), i)).collect();
                let picks = live_rows(picks, &live);
                let focus = report::Focus {
                    running: live_rows(focus.running, &live),
                    outside: live_rows(focus.outside, &live),
                };
                // **저장소 안과 같은 자로 가른다**(moai-0zjo) — 사람은 그 프로젝트의 뿌리에서 푼다.
                // 여기서 안 가르면 한눈 보기가 남의 줄을 집을 것으로 내민다.
                let me = if picks.is_empty() { None } else { super::me_at(ctx, &repo.root) };
                let (mut picks, mut others) = report::by_owner(picks, me.as_ref());
                let more = crate::query::cut(&mut picks, limit);
                let others_more = crate::query::cut(&mut others, limit);
                // **지도는 기계 쪽만 짓는다**(리뷰) — `Picks::epics` 를 읽는 것은 `--json` 뿐인데,
                // 여기서 늘 지으면 사람이 보는 한눈 보기가 등록한 프로젝트마다 저장소 전체의
                // 소속을 한 벌씩 걷고 그대로 버린다.
                let (epics, kinds) = match ctx.json {
                    true => {
                        let ids: Vec<&str> =
                            picks.iter().chain(others.iter().map(|(i, _)| i)).map(|i| i.id.as_str()).collect();
                        (report::handed_of(&load.issues, &ids), report::Kinds::of_ids(&load.issues, &ids))
                    }
                    false => Default::default(),
                };
                view::Picks {
                    picks,
                    more,
                    others,
                    others_more,
                    column: repo.config.started_status(),
                    from: repo.config.first_status(),
                    focus,
                    unreadable: load.errors.len(),
                    epics,
                    kinds,
                    origin: &p.origin,
                    trouble: &p.trouble,
                }
            })
        })
        .collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Said<'a> {
            ready: Vec<super::Row<'a>>,
            /// 저장소 안의 `ready --json` 과 같은 키 — **늘 싣는다**.
            others: Vec<super::Other<super::Row<'a>>>,
            /// 저장소 안의 `ready --json` 과 같은 두 키 — 없으면 안 단다.
            #[serde(skip_serializing_if = "Vec::is_empty")]
            milestone: Vec<&'a str>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            outside: Vec<&'a str>,
            unreadable: usize,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            trouble: Vec<String>,
        }
        let entries = projects
            .iter()
            .zip(&seen)
            .map(|(p, s)| Entry {
                name: &p.name,
                path: &p.path,
                seen: s.map(|k| Said {
                    ready: k
                        .picks
                        .iter()
                        .map(|i| {
                            super::Row::of(i, |_| None, k.epics.get(i.id.as_str()).copied(), &k.kinds).on(&p.origin)
                        })
                        .collect(),
                    others: k
                        .others
                        .iter()
                        .map(|(i, owner)| super::Other {
                            row: super::Row::of(i, |_| None, k.epics.get(i.id.as_str()).copied(), &k.kinds)
                                .on(&p.origin),
                            owner: *owner,
                        })
                        .collect(),
                    milestone: k.focus.running.iter().map(|m| m.id.as_str()).collect(),
                    outside: k.focus.outside.iter().map(|i| i.id.as_str()).collect(),
                    unreadable: k.unreadable,
                    // 옆 워크트리의 문제와 설정의 탈은 **편 뒤에** 싣는다(moai-dpbi) — `status --json` 과 같은 자다.
                    trouble: p.trouble.iter().map(|t| view::trouble_line(ctx.lang(), t)).collect(),
                }),
            })
            .collect();
        let problems = view::settings_problems(reg, ctx.lang());
        let all = Overview { projects: entries, problems: &problems, config: reg.path.as_deref() };
        return super::json_line(&all);
    }
    Ok(view::projects_ready(&projects, &seen, reg, view::Screen::new(ctx.lang()).at(ctx.clock())))
}

/// 문맥(산 줄 + 아카이브)에서 고른 줄을 그 프로젝트의 산 줄로 되짚는다 — 한눈 보기의 그릇은 프로젝트가 든 줄을
/// 빌린다. 고르는 줄은 늘 산 줄이라(옮긴 줄은 닫혀 있다) 빠지는 것이 없다.
fn live_rows<'a>(
    rows: Vec<&crate::model::Issue>,
    live: &std::collections::BTreeMap<&str, &'a crate::model::Issue>,
) -> Vec<&'a crate::model::Issue> {
    rows.into_iter().filter_map(|i| live.get(i.id.as_str()).copied()).collect()
}
