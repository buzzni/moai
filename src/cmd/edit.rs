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

pub fn run(ctx: &Ctx, args: EditArgs) -> R<Vec<String>> {
    fail_if_nothing(&args)?;
    if let Some(t) = &args.title {
        super::refuse_if_flag_like(t.trim())?;
    }
    let repo = Repo::discover()?;
    let body = super::add::read_body(args.body.clone())?;
    let at = model::now();

    let (edited, epic, children, changed): (Issue, Option<Issue>, Vec<Issue>, bool) =
        repo.with_write(|issues, cfg| {
        let Some(i) = issues.iter_mut().find(|i| i.id == args.id) else {
            return Err(Fail::not_found(&args.id));
        };
        let before = i.clone();

        if let Some(t) = &args.title {
            i.title = t.trim().to_string();
        }
        // `--body` 를 적었으면 적은 대로 된다. **빈 것도 적은 것이다** —
        // `-b ""` 든 빈 stdin 이든 지운다는 뜻이고, 둘이 갈리면 파이프로
        // 본문을 만들어 넣는 쪽이 옛 본문을 지우지 못한다.
        if args.body.is_some() {
            i.body = body.clone();
        }
        for t in &args.tag {
            let t = model::normalize_tag(t);
            if !t.is_empty() && !i.tags.contains(&t) {
                i.tags.push(t);
            }
        }
        if !args.untag.is_empty() {
            let drop: Vec<String> = args.untag.iter().map(|t| model::normalize_tag(t)).collect();
            i.tags.retain(|t| !drop.contains(t));
        }
        if let Some(e) = &args.epic {
            i.epic = super::clearable(e);
        }
        if let Some(m) = &args.milestone {
            i.milestone = super::clearable(m);
        }
        if let Some(p) = args.priority {
            i.priority = Some(p);
        }
        if let Some(a) = &args.assignee {
            (i.assignee, i.assignee_email) = match super::clearable(a) {
                Some(v) => model::split_assignee(&v),
                None => (None, None),
            };
        }

        i.normalize();
        i.validate(cfg)?;
        // 바뀐 것이 없어도 실패가 아니다. `mv` 가 이미 그 칸일 때 0 으로
        // 끝나는 것과 같아야 한다 — 되풀이해 부르는 것이 흔하고, 그때
        // 한쪽만 1 로 끝나면 받는 쪽이 재시도를 못 짠다.
        let changed = *i != before;
        if !changed {
            return Ok((vec![], (before, None, Vec::new(), false)));
        }
        i.updated_at = at.clone();
        let out = i.clone();

        // 상세를 그릴 재료를 **여기서** 챙긴다. 락을 놓은 뒤 파일을 다시 읽으면
        // 1만 줄을 두 번 파싱하고(측정: 한 번 더 읽는 데만 25%), 그 틈에 남이
        // 쓴 것이 섞여 방금 쓴 이슈와 주변이 어긋난다.
        let epic = out.epic.as_ref().and_then(|e| issues.iter().find(|x| &x.id == e).cloned());
        // 없는 에픽은 막지 않고 알려만 준다 — 끊긴 참조는 `moai status` 가 드러낸다.
        if let Some(e) = &out.epic
            && epic.is_none()
        {
            eprintln!("moai: {e} 라는 에픽이 없다. 그대로 둔다");
        }
        let children: Vec<Issue> = issues
            .iter()
            .filter(|c| crate::id::parent_of(&c.id) == Some(out.id.as_str()))
            .cloned()
            .collect();
        Ok((vec![], (out, epic, children, true)))
    })?;

    if ctx.json {
        return super::json_line(&edited);
    }
    if !changed {
        return Ok(vec![format!(
            "{}  {}",
            crate::style::paint(crate::style::ID, &edited.id),
            crate::style::paint(crate::style::DIM, "바뀐 것이 없다")
        )]);
    }
    let children: Vec<&Issue> = children.iter().collect();
    Ok(view::detail(&edited, epic.as_ref(), &children, &[], &at, false))
}

fn fail_if_nothing(args: &EditArgs) -> R<()> {
    let touched = args.title.is_some()
        || args.body.is_some()
        || !args.tag.is_empty()
        || !args.untag.is_empty()
        || args.epic.is_some()
        || args.milestone.is_some()
        || args.priority.is_some()
        || args.assignee.is_some();
    touched.then_some(()).ok_or_else(|| {
        Fail::new("무엇을 고칠지 적지 않았다. `moai edit --help` 가 고칠 수 있는 것을 낸다")
    })
}
