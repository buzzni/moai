//! 이슈를 만든다. 승인을 묻지 않는다 — 이 도구에 게이트는 없다.

use super::{Ctx, Fail, R};
use crate::cli::AddArgs;
use crate::model::{self, Issue, JournalEntry, Kind, Status};
use crate::store::{Repo, taken_ids};
use crate::style::{self, paint};

/// `-` 이면 stdin. `add` 와 `edit` 이 같은 규칙을 쓴다.
pub fn read_body(arg: Option<String>) -> R<Option<String>> {
    match arg.as_deref() {
        None => Ok(None),
        Some("-") => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .map_err(|e| Fail::new(format!("stdin: {e}")))?;
            let s = s.trim_end_matches('\n').to_string();
            Ok((!s.is_empty()).then_some(s))
        }
        Some(b) => Ok((!b.is_empty()).then(|| b.to_string())),
    }
}

pub fn run(ctx: &Ctx, args: AddArgs, kind_override: Option<Kind>) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let body = read_body(args.body)?;
    let kind = kind_override.or(args.kind).unwrap_or_default();
    let status = Status::new(
        args.status.clone().unwrap_or_else(|| repo.config.first_status().to_string()),
    );
    // 칸 검사는 id 를 뽑기 **전에** 한다. 나중에 하면 쓰이지도 않은 id 가
    // 오류 메시지에 실려 나가고, 받는 쪽은 그게 만들어진 줄 안다.
    repo.config.require_known(status.as_str()).map_err(|e| Fail::coded(e, "bad_status"))?;
    let at = model::now();
    let by = model::actor();

    let made: Issue = repo.with_write(|issues, cfg| {
        let taken = taken_ids(issues);

        let id = match &args.parent {
            Some(p) => {
                if !issues.iter().any(|i| &i.id == p) {
                    return Err(format!("{p} 를 못 찾았다 — 부모가 없는 자식은 만들지 않는다"));
                }
                crate::id::generate_child(p, &taken, &crate::id::seed(&args.title))
            }
            None => crate::id::generate(&cfg.prefix, &taken, &crate::id::seed(&args.title)),
        };

        // 없는 에픽을 가리키는 것은 막지 않고 **알려만 준다** — 에픽을 나중에
        // 만드는 순서가 실제로 있고, 끊긴 참조는 `moai status` 가 드러낸다.
        if let Some(e) = &args.epic
            && !issues.iter().any(|i| &i.id == e)
        {
            eprintln!("moai: {e} 라는 에픽이 아직 없다. 그대로 넣는다");
        }

        let mut issue = Issue::new(id, args.title.trim().to_string(), kind, status.clone(), &at);
        issue.epic = args.epic.clone();
        issue.tags = args.tag.iter().map(|t| model::normalize_tag(t)).collect();
        issue.priority = args.priority;
        issue.assignee = args.assignee.clone();
        issue.body = body.clone();
        issue.normalize();
        issue.validate(cfg)?;

        let entry = JournalEntry::create(&issue.id, &issue.title, &at, &by);
        issues.push(issue.clone());
        Ok((vec![entry], issue))
    })?;

    if ctx.json {
        return super::json_line(&made);
    }
    if args.quiet {
        return Ok(vec![made.id]);
    }

    let st = style::status_style(made.status.as_str());
    let mut line = format!(
        "{}  {}  {}  {}",
        paint(style::ID, &made.id),
        paint(style::priority_style(made.priority()), &format!("p{}", made.priority())),
        paint(st, style::glyph(made.status.as_str())),
        paint(
            if made.kind == Kind::Issue { style::PLAIN } else { style::EPIC },
            &made.title
        ),
    );
    if !made.tags.is_empty() {
        let tags = made.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
        line.push_str(&format!("   {}", paint(style::TAG, &tags)));
    }
    if let Some(e) = &made.epic {
        line.push_str(&format!("   {}", paint(style::DIM, e)));
    }
    Ok(vec![line])
}
