//! 제목·본문·태그·에픽·우선순위·담당을 고친다.
//!
//! **저널에 적지 않는다.** 필드 변경까지 적기 시작하면 저널은 이벤트 로그가
//! 되고, 그러면 "스냅샷 대신 저걸 접으면 되지 않나" 가 반드시 돌아온다.
//! 이전 시도가 거기서 시작했다. 상태 전이만 저널이 갖는다.

use super::{Ctx, Fail, R};
use crate::cli::EditArgs;
use crate::model::{self, Issue};
use crate::store::Repo;
use crate::view;

/// `none` 은 "지운다" 는 뜻이다. 제목이 `none` 인 이슈를 만들 일은 없다.
fn clearable(v: &str) -> Option<String> {
    (v != "none").then(|| v.to_string())
}

pub fn run(ctx: &Ctx, args: EditArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let body = super::add::read_body(args.body.clone())?;
    let at = model::now();

    let edited: Issue = repo.with_write(|issues, cfg| {
        let Some(i) = issues.iter_mut().find(|i| i.id == args.id) else {
            return Err(format!("{} 를 못 찾았다", args.id));
        };
        let before = i.clone();

        if let Some(t) = &args.title {
            i.title = t.trim().to_string();
        }
        if body.is_some() {
            i.body = body.clone();
        } else if args.body.as_deref() == Some("") {
            i.body = None;
        }
        for t in &args.tag {
            let t = t.trim().trim_start_matches('#').to_string();
            if !t.is_empty() && !i.tags.contains(&t) {
                i.tags.push(t);
            }
        }
        if !args.untag.is_empty() {
            let drop: Vec<String> =
                args.untag.iter().map(|t| t.trim().trim_start_matches('#').to_string()).collect();
            i.tags.retain(|t| !drop.contains(t));
        }
        if let Some(e) = &args.epic {
            i.epic = clearable(e);
        }
        if let Some(p) = args.priority {
            i.priority = Some(p);
        }
        if let Some(a) = &args.assignee {
            i.assignee = clearable(a);
        }

        i.normalize();
        i.validate(cfg)?;
        if *i == before {
            return Err(format!("{} 는 바뀐 것이 없다", args.id));
        }
        i.updated_at = at.clone();
        let out = i.clone();

        // 없는 에픽은 막지 않고 알려만 준다 — 끊긴 참조는 `moai status` 가 드러낸다.
        if let Some(e) = &out.epic
            && !issues.iter().any(|x| &x.id == e)
        {
            eprintln!("moai: {e} 라는 에픽이 없다. 그대로 둔다");
        }
        Ok((vec![], out))
    })?;

    if ctx.json {
        return super::json_line(&edited);
    }
    let load = repo.read()?;
    let epic = edited.epic.as_ref().and_then(|e| load.get(e));
    let children: Vec<&Issue> = load
        .issues
        .iter()
        .filter(|c| crate::id::parent_of(&c.id) == Some(edited.id.as_str()))
        .collect();
    Ok(view::detail(&edited, epic, &children, &[], &at))
}

pub fn fail_if_nothing(args: &EditArgs) -> R<()> {
    let touched = args.title.is_some()
        || args.body.is_some()
        || !args.tag.is_empty()
        || !args.untag.is_empty()
        || args.epic.is_some()
        || args.priority.is_some()
        || args.assignee.is_some();
    touched.then_some(()).ok_or_else(|| {
        Fail::new("무엇을 고칠지 적지 않았다. `moai edit --help` 가 고칠 수 있는 것을 낸다")
    })
}
