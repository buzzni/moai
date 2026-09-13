//! 보드 · 경고 · 흐름.
//!
//! 게이트를 없앤 자리를 메우는 것이 이 명령 하나다. 무엇을 드러낼지는
//! `report::status` 가 정하고, 여기는 잇기만 한다.
//!
//! **종료 코드는 데이터가 깨졌을 때만 0 이 아니다.** 경고로 비영 종료하면
//! 에이전트가 이것을 실패로 읽고, 그러면 이건 린트고, 린트는 곧 게이트다.

use super::{Ctx, R};
use crate::model;
use crate::report;
use crate::store::Repo;
use crate::projects::{Entry, Overview, Seen};
use crate::view;

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트를 한눈에. **안이면 아래 그대로다** — 등록 목록을
    // 읽지도 않는다(결정 3: `.moai` 안의 CLI 는 그 프로젝트만 본다).
    let Some(repo) = Repo::find()? else {
        return overview(ctx, worktree);
    };
    let crate::worktree::Gathered { load, origin, .. } = super::gather(&repo, worktree)?;
    // **그 줄이 쓰는 id** 까지 넘긴다 — id 가 있어야 산 줄과의 중복이
    // 드러난다(moai-4dk4).
    // 옆에서만 온 줄과 겹친 id 는 중복이 아니다 (`Origin::unreadable`).
    let unreadable: Vec<report::Unreadable> = origin
        .unreadable(load.errors.iter().map(|e| e.id.as_deref()))
        .into_iter()
        .map(|id| report::Unreadable { id })
        .collect();
    let now = model::now();
    let st = report::status(&load.issues, &unreadable, &repo.config, &now);

    if st.broken() {
        super::note_partial();
    }
    if ctx.json {
        // **겹쳐 봤을 때만 키를 단다.** 늘 달면 `--worktree` 없이 부른 쪽도 빈
        // 지도를 받아 "겹쳐 봤는데 옆에 아무것도 없다" 로 읽는다.
        if worktree {
            let branches = serde_json::to_string(&origin.branches()).map_err(|e| super::Fail::new(e.to_string()))?;
            return super::json_with(&st, &[("branches", branches)]);
        }
        return super::json_line(&st);
    }
    Ok(view::status(
        &st,
        &load.issues,
        &repo.config,
        &now,
        ".moai/issues.jsonl",
        &origin,
    ))
}

/// 등록한 프로젝트마다 보드 요약. **프로젝트마다 따로 센다** — 줄을 한데 모으지 않는다.
///
/// **깨진 프로젝트가 있어도 0 으로 끝난다.** 한 프로젝트 안의 `status` 가 깨진
/// 데이터로 비영 종료하는 것은 제 파일이라서다. 여기서 보는 것은 남의 저장소일 수
/// 있고, 그것 하나로 한눈 보기 전체가 실패로 읽히면 나머지를 못 믿는다.
fn overview(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let (reg, projects) = super::registered("status", worktree)?;
    let now = model::now();
    let seen: Vec<Seen<view::Board>> = projects
        .iter()
        .map(|p| {
            p.seen(|repo, load| {
                let unreadable: Vec<report::Unreadable> =
                    load.errors.iter().map(|e| report::Unreadable { id: e.id.as_deref() }).collect();
                view::Board {
                    cfg: &repo.config,
                    status: report::status(&load.issues, &unreadable, &repo.config, &now),
                    picked: report::wip(&load.issues, &repo.config),
                }
            })
        })
        .collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Said<'a> {
            status: &'a report::StatusReport,
            picked: Vec<super::Row<'a>>,
        }
        let entries = projects
            .iter()
            .zip(&seen)
            .map(|(p, s)| Entry {
                name: &p.name,
                path: &p.path,
                seen: s.map(|b| Said {
                    status: &b.status,
                    picked: b.picked.iter().map(|i| super::Row::of(i, None)).collect(),
                }),
            })
            .collect();
        let all = Overview { projects: entries, problems: &reg.problems, config: reg.path.as_deref() };
        return super::json_line(&all);
    }
    Ok(view::projects_status(&projects, &seen, &reg))
}
