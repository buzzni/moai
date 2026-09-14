//! 지금 집을 수 있는 일.
//!
//! 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
//! 무엇이 ready 인지는 `report::ready` 가 정하고, 어떻게 보일지는 `view` 가
//! 정한다. 여기는 둘을 잇기만 한다.

use super::{Ctx, R};
use crate::report;
use crate::store::Repo;
use crate::projects::{Entry, Overview, Seen};
use crate::view;

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트마다 집을 것. 안이면 아래 그대로다 (결정 3).
    let Some(repo) = Repo::find()? else {
        return overview(ctx, worktree);
    };
    let crate::worktree::Gathered { load, origin, .. } = super::gather(&repo, worktree)?;
    super::report_load_errors(&repo.issues_path(), &load.errors);

    // **겹친 줄로 고른다.** 옆 워크트리에서 집은 일은 거기서 `in_progress` 로 서
    // 있으므로, 같은 자(`report::ready`)가 그것을 저절로 뺀다 — 여기에 "남이 집은
    // 것" 을 가르는 `if` 를 따로 두지 않는다.
    let picks = report::ready(&load.issues, &repo.config);
    if ctx.json {
        let rows: Vec<super::Row> = picks.iter().map(|i| super::Row::of(i, None).on(&origin)).collect();
        return super::json_line(&rows);
    }

    // 첫 칸도 아니고 끝나지도 않은 것 = 누군가 이미 잡고 있는 것.
    let wip = report::wip(&load.issues, &repo.config);
    // 미뤄 둔 것에 막혀 못 집는 것. 안 대면 `ready` 가 까닭 없이 빈다.
    let held = report::held(&load.issues, &repo.config);

    Ok(view::ready(&picks, &report::epic_labels(&load.issues), &wip, &held, &origin))
}

/// 등록한 프로젝트마다 집을 수 있는 일. 무엇이 ready 인지는 프로젝트마다 같은 자
/// (`report::ready`)가 **그 프로젝트의 줄만 보고** 정한다 — 남의 프로젝트에서 집은
/// 일이 이쪽의 막음을 풀거나 걸지 않는다.
///
/// **못 읽는 줄이 있어도 0 으로 끝난다** (`status::overview` 와 같은 까닭). 그 줄은
/// 프로젝트 줄 밑에 수로 말하고, 어느 줄인지는 그 프로젝트의 `show` 가 낸다.
fn overview(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let (reg, projects) = super::registered(worktree)?;
    let seen: Vec<Seen<view::Picks>> = projects
        .iter()
        .map(|p| {
            p.seen(|repo, load| view::Picks {
                picks: report::ready(&load.issues, &repo.config),
                unreadable: load.errors.len(),
                origin: &p.origin,
                trouble: &p.trouble,
            })
        })
        .collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Said<'a> {
            ready: Vec<super::Row<'a>>,
            unreadable: usize,
            #[serde(skip_serializing_if = "<[String]>::is_empty")]
            trouble: &'a [String],
        }
        let entries = projects
            .iter()
            .zip(&seen)
            .map(|(p, s)| Entry {
                name: &p.name,
                path: &p.path,
                seen: s.map(|k| Said {
                    ready: k.picks.iter().map(|i| super::Row::of(i, None).on(&p.origin)).collect(),
                    unreadable: k.unreadable,
                    trouble: &p.trouble,
                }),
            })
            .collect();
        let all = Overview { projects: entries, problems: &reg.problems, config: reg.path.as_deref() };
        return super::json_line(&all);
    }
    Ok(view::projects_ready(&projects, &seen, &reg))
}
