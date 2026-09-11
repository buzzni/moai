//! 하나를 펼치거나 목록을 낸다.
//!
//! `list` 를 따로 두지 않는다. 읽기 동사가 하나면 `--json`·필터·색 정책을
//! 한 번만 정의한다 — beads 는 `list` 52개 / `show` 9개 플래그로 갈라져
//! 겹치는 개념이 다른 이름을 갖는 값을 치르고 있다.

use super::{Ctx, Fail, R};
use crate::cli::ShowArgs;
use crate::model::{self, Issue, Kind};
use crate::store::Repo;
use crate::view;

/// 대상이 무엇으로 읽히는가. id 는 `-` 를 품고 종류 낱말은 품지 않아
/// 어휘가 겹치지 않는다.
enum Target {
    One(String),
    OfKind(Kind),
    All,
}

fn resolve(target: Option<&str>) -> R<Target> {
    match target {
        None => Ok(Target::All),
        Some("issue") => Ok(Target::OfKind(Kind::Issue)),
        Some("epic") => Ok(Target::OfKind(Kind::Epic)),
        Some("milestone") => Err(Fail::new("마일스톤은 아직 없다 (2단계)")),
        Some(t) if crate::id::is_valid(t) => Ok(Target::One(t.to_string())),
        // 조용히 0건을 내지 않는다. 모르는 값은 거부하고 있는 것을 나열한다.
        Some(t) => Err(Fail::coded(
            format!(
                "`{t}` 는 id 도 종류도 아니다. 종류: issue, epic\n      \
                 id 로 찾으려면 접두어까지 적는다"
            ),
            "bad_target",
        )),
    }
}

pub fn run(ctx: &Ctx, args: ShowArgs, kind_filter: Option<Kind>) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    super::report_load_errors(&repo.issues_path(), &load.errors);

    let target = match kind_filter {
        // `moai epic show <id>` 는 그 id 를 그대로 본다. 종류는 목록일 때만 거른다.
        Some(k) if args.target.is_none() => Target::OfKind(k),
        _ => resolve(args.target.as_deref())?,
    };

    if let Target::One(id) = &target {
        let issue = load
            .get(id)
            .ok_or_else(|| Fail::coded(format!("{id} 를 못 찾았다"), "not_found"))?;
        return one(ctx, &repo, &load.issues, issue);
    }

    let kind = match target {
        Target::OfKind(k) => Some(k),
        _ => None,
    };
    let mut shown: Vec<Issue> = load
        .issues
        .iter()
        .filter(|i| kind.is_none_or(|k| i.kind == k))
        .cloned()
        .collect();
    let total = shown.len();
    if !args.all {
        shown.retain(|i| !i.status.is_done());
    }
    let hidden = total - shown.len();

    if ctx.json {
        return super::json_line(&shown);
    }
    Ok(view::list(&shown, &repo.config, hidden))
}

fn one(ctx: &Ctx, repo: &Repo, all: &[Issue], issue: &Issue) -> R<Vec<String>> {
    let epic = issue.epic.as_ref().and_then(|e| all.iter().find(|i| &i.id == e));
    let children: Vec<&Issue> = all
        .iter()
        .filter(|c| crate::id::parent_of(&c.id) == Some(issue.id.as_str()))
        .collect();
    let journal = repo.journal_of(&issue.id)?;

    if ctx.json {
        let ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        return super::json_with(
            issue,
            &[
                ("children", serde_json::to_string(&ids).map_err(|e| Fail::new(e.to_string()))?),
                ("journal", serde_json::to_string(&journal).map_err(|e| Fail::new(e.to_string()))?),
            ],
        );
    }
    Ok(view::detail(issue, epic, &children, &journal, &model::now()))
}
