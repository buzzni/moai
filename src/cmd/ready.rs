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

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트마다 집을 것. 안이면 아래 그대로다 (결정 3).
    let Some(repo) = Repo::find(|| ctx.lang())? else {
        return overview(ctx, worktree);
    };
    let crate::worktree::Gathered { load, origin, .. } = super::gather(ctx, &repo, worktree)?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);

    // **겹친 줄로 고른다.** 옆 워크트리에서 집은 일은 거기서 `in_progress` 로 서
    // 있으므로, 같은 자(`report::ready`)가 그것을 저절로 뺀다 — 여기에 "남이 집은
    // 것" 을 가르는 `if` 를 따로 두지 않는다.
    let (picks, focus) = report::ready_in(&load.issues, &repo.config);
    // 미뤄 둔 것·빈 묶음에 막혀 못 집는 것. 안 대면 `ready` 가 까닭 없이 빈다.
    let held = report::held(&load.issues, &repo.config);
    if ctx.json {
        // **객체로 감싼다**(moai-w6n2, 사람이 정한 출력 계약). 맨 배열이던 때는 막혀 못 집는
        // 일을 실을 자리가 없어, 에이전트는 `[]` 를 "할 일이 없다" 로 읽었다. `.moai` 밖
        // 한눈 보기가 프로젝트마다 `ready` 키를 쓰는 것과 같은 이름이다.
        #[derive(serde::Serialize)]
        struct Said<'a> {
            ready: Vec<super::Row<'a>>,
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
            ready: picks.iter().map(|i| super::Row::of(i, None).on(&origin)).collect(),
            // **왜 짧은지를 기계에도 댄다**(moai-q04l). 사람 화면이 한 줄로 대는 것을 여기서
            // 빼면, `ready --json` 으로 도는 고리는 도는 마일스톤이 목록을 줄인 것을 "할 일이
            // 없다" 로 읽는다 — `held` 를 객체로 감싼 것과 같은 까닭이다(moai-w6n2).
            // 도는 것이 없으면 키를 안 단다.
            milestone: focus.running.iter().map(|m| m.id.as_str()).collect(),
            outside: focus.outside.iter().map(|i| i.id.as_str()).collect(),
            held: held.iter().map(|h| Waiting { id: &h.issue.id, by: &h.by, undo: &h.undo, empty: &h.empty }).collect(),
        });
    }

    // 첫 칸도 아니고 끝나지도 않은 것 = 누군가 이미 잡고 있는 것.
    let wip = report::wip(&load.issues, &repo.config);

    let screen = view::Screen::new(ctx.lang()).at(ctx.zone()).over(&origin);
    Ok(view::ready(&picks, &report::epic_labels(&load.issues), &wip, &held, &focus, screen))
}

/// 등록한 프로젝트마다 집을 수 있는 일. 무엇이 ready 인지는 프로젝트마다 같은 자
/// (`report::ready`)가 **그 프로젝트의 줄만 보고** 정한다 — 남의 프로젝트에서 집은
/// 일이 이쪽의 막음을 풀거나 걸지 않는다.
///
/// **못 읽는 줄이 있어도 0 으로 끝난다** (`status::overview` 와 같은 까닭). 그 줄은
/// 프로젝트 줄 밑에 수로 말하고, 어느 줄인지는 그 프로젝트의 `show` 가 낸다.
fn overview(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let (reg, projects) = super::registered(ctx, worktree)?;
    let seen: Vec<Seen<view::Picks>> = projects
        .iter()
        .map(|p| {
            p.seen(|repo, load| {
                // **저장소 안의 `ready` 와 같은 자다.** 도는 마일스톤이 목록을 줄였으면 그
                // 까닭도 함께 받는다 — 여기서 `ready` 만 부르면 한눈 보기의 목록만 말없이
                // 짧아지고, 그 짧아짐이 "할 일이 없다" 로 읽힌다.
                let (picks, focus) = report::ready_in(&load.issues, &repo.config);
                view::Picks { picks, focus, unreadable: load.errors.len(), origin: &p.origin, trouble: &p.trouble }
            })
        })
        .collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Said<'a> {
            ready: Vec<super::Row<'a>>,
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
                    ready: k.picks.iter().map(|i| super::Row::of(i, None).on(&p.origin)).collect(),
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
    Ok(view::projects_ready(&projects, &seen, reg, view::Screen::new(ctx.lang()).at(ctx.zone())))
}
