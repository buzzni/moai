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

/// 락 안에서 챙겨 나오는 것. **이름을 붙여 둔다** — 같은 모양의 지도 둘을 튜플로
/// 늘어놓으면 자리를 바꿔 적어도 컴파일러가 안 잡는다.
struct Edited {
    issue: Issue,
    epic: Option<Issue>,
    children: Vec<Issue>,
    /// 계획에서 빠진 줄 → 뺀 줄. 고친 줄과 그 자식만.
    shelved: Vec<(String, String)>,
    /// 묶음 → 멤버에서 읽은 칸. 고친 줄과 그 자식만.
    read: super::Read,
    changed: bool,
}

pub fn run(ctx: &Ctx, args: EditArgs) -> R<Vec<String>> {
    fail_if_nothing(&args)?;
    if let Some(t) = &args.title {
        super::refuse_if_flag_like(t.trim())?;
    }
    let repo = Repo::discover()?;
    let body = super::add::read_body(args.body.clone())?;
    let at = model::now();

    let done: Edited = repo.with_write(|issues, cfg, _| {
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
            // **읽은 칸은 바뀐 것이 없어도 낸다.** 되풀이해 부르는 것이 흔한데, 그때만
            // 키가 사라지면 받는 쪽은 그 줄이 묶음이 아닌 줄 알고 적힌 칸을 읽는다.
            let read = super::read_of(issues, cfg, &[before.id.as_str()]);
            return Ok((
                vec![],
                Edited {
                    issue: before,
                    epic: None,
                    children: Vec::new(),
                    shelved: Vec::new(),
                    read,
                    changed: false,
                },
            ));
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
        // 상세가 그리는 줄 — 고친 줄과 그 자식. 미룸과 읽은 칸을 같은 자로 고른다.
        let near: Vec<&str> =
            std::iter::once(out.id.as_str()).chain(children.iter().map(|c| c.id.as_str())).collect();
        // 상세가 미룸을 말하려면 **물려받은 것까지** 필요하다 — 미룬 에픽으로 옮기는
        // 순간 그 줄이 계획에서 빠진다. 같은 까닭으로 락 안에서 본 모습으로 잰다.
        let shelved: Vec<(String, String)> = crate::report::deferred_roots(issues)
            .into_iter()
            .filter(|(id, _)| near.contains(id))
            .map(|(id, root)| (id.to_string(), root.to_string()))
            .collect();
        // 묶음의 칸도 멤버에서 읽는다 — 제 줄만 들고 나가면 상세가 손으로 둔 칸을 그린다.
        let read = super::read_of(issues, cfg, &near);
        Ok((vec![], Edited { issue: out, epic, children, shelved, read, changed: true }))
    })?;

    let Edited { issue: edited, epic, children, shelved, read, changed } = done;
    if ctx.json {
        return super::json_line(&super::Row::from(&edited, &read));
    }
    if !changed {
        return Ok(vec![format!(
            "{}  {}",
            crate::style::paint(crate::style::ID, &edited.id),
            crate::style::paint(crate::style::DIM, "바뀐 것이 없다")
        )]);
    }
    let children: Vec<&Issue> = children.iter().collect();
    let seen = view::Seen {
        roots: shelved.iter().map(|(id, root)| (id.as_str(), root.as_str())).collect(),
        states: read.iter().map(|(id, col)| (id.as_str(), col.as_str())).collect(),
    };
    Ok(view::detail(&edited, epic.as_ref(), &children, &seen, &repo.config, &at, false))
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
