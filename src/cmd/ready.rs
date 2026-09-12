//! 지금 집을 수 있는 일.
//!
//! 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
//! 무엇이 ready 인지는 `report::ready` 가 정하고, 어떻게 보일지는 `view` 가
//! 정한다. 여기는 둘을 잇기만 한다.

use super::{Ctx, R};
use crate::model::Issue;
use crate::report;
use crate::store::Repo;
use crate::view;

pub fn run(ctx: &Ctx) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    super::report_load_errors(&repo.issues_path(), &load.errors);

    let picks = report::ready(&load.issues, &repo.config);
    if ctx.json {
        return super::json_line(&picks);
    }

    // 첫 칸도 아니고 끝나지도 않은 것 = 누군가 이미 잡고 있는 것.
    let wip: Vec<&Issue> = load
        .issues
        .iter()
        .filter(|i| {
            report::is_active(i)
                && !i.status.is_done()
                && i.status.as_str() != repo.config.first_status()
        })
        .collect();

    Ok(view::ready(&picks, &report::epic_labels(&load.issues), &wip))
}
