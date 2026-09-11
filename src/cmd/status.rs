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

pub fn run(ctx: &Ctx) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    let unreadable: Vec<usize> = load.errors.iter().map(|e| e.line).collect();
    let now = model::now();
    let st = report::status(&load.issues, &unreadable, &repo.config, &now);

    if st.broken() {
        super::note_partial();
    }
    if ctx.json {
        return super::json_line(&st);
    }
    Ok(view::status(
        &st,
        &load.issues,
        &repo.config,
        &now,
        ".moai/issues.jsonl",
    ))
}
