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
use crate::view;

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let crate::worktree::Gathered { load, origin, .. } = super::gather(&repo, worktree)?;
    // **그 줄이 쓰는 id** 까지 넘긴다 — id 가 있어야 산 줄과의 중복이
    // 드러난다(moai-4dk4).
    let unreadable: Vec<report::Unreadable> =
        load.errors.iter().map(|e| report::Unreadable { id: e.id.as_deref() }).collect();
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
