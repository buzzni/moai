//! 이슈를 만든다. 승인을 묻지 않는다 — 이 도구에 게이트는 없다.

use super::{Ctx, Fail, R};
use crate::cli::AddArgs;
use crate::draft::{self, Draft};
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
    if let Some(from) = &args.from {
        return bulk(ctx, &repo, from, args.dry_run);
    }
    let Some(title) = args.title.clone().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    else {
        return Err(Fail::new(
            "제목이 없다. `moai add \"제목\"` 또는 `moai add --from -` 이다",
        ));
    };
    super::refuse_if_flag_like(&title)?;
    let body = read_body(args.body)?;
    let kind = kind_override.or(args.kind).unwrap_or_default();
    let status = Status::new(
        args.status.clone().unwrap_or_else(|| repo.config.first_status().to_string()),
    );
    // 칸 검사는 id 를 뽑기 **전에** 한다. 나중에 하면 쓰이지도 않은 id 가
    // 오류 메시지에 실려 나가고, 받는 쪽은 그게 만들어진 줄 안다.
    repo.config.require_known(status.as_str()).map_err(|e| Fail::coded(e, super::code::BAD_STATUS))?;
    let at = model::now();
    let by = model::actor();

    let made: Issue = repo.with_write(|issues, cfg| {
        let taken = taken_ids(issues);

        let id = match &args.parent {
            Some(p) => {
                if !issues.iter().any(|i| &i.id == p) {
                    return Err(Fail::coded(
                        format!("{p} 를 못 찾았다 — 부모가 없는 자식은 만들지 않는다"),
                        super::code::NOT_FOUND,
                    ));
                }
                crate::id::generate_child(
                    p,
                    &taken,
                    &crate::id::seed(&title),
                )
            }
            None => crate::id::generate(&cfg.prefix, &taken, &crate::id::seed(&title)),
        };

        // 없는 에픽을 가리키는 것은 막지 않고 **알려만 준다** — 에픽을 나중에
        // 만드는 순서가 실제로 있고, 끊긴 참조는 `moai status` 가 드러낸다.
        if let Some(e) = &args.epic
            && !issues.iter().any(|i| &i.id == e)
        {
            eprintln!("moai: {e} 라는 에픽이 아직 없다. 그대로 넣는다");
        }

            let mut issue = Issue::new(id, title.clone(), kind, status.clone(), &at);
        issue.epic = args.epic.clone();
        issue.milestone = args.milestone.clone();
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

/// 마크다운 한 덩이에서 에픽과 이슈를 **한 번에** 만든다.
///
/// 하나씩 만들면 에이전트가 중간에 흘리고, 중간에 죽으면 반만 남은 계획이
/// 남는다. 한 번의 쓰기라 다 되거나 하나도 안 된다.
fn bulk(ctx: &Ctx, repo: &Repo, from: &str, dry_run: bool) -> R<Vec<String>> {
    let src = match from {
        "-" => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .map_err(|e| Fail::new(format!("stdin: {e}")))?;
            s
        }
        path => std::fs::read_to_string(path)
            .map_err(|e| Fail::new(format!("{path}: {e}")))?,
    };
    let drafts = draft::parse(&src).map_err(|e| Fail::coded(e, super::code::BAD_INPUT))?;

    if dry_run {
        // 만들지 않으므로 id 가 없다. 무엇이 어디에 붙는지만 보여 준다.
        let mut out = vec![paint(style::HEAD, "만들 것")];
        out.extend(drafts.iter().map(|d| line_of(d, None)));
        out.push(String::new());
        out.push(tally(&drafts));
        return Ok(out);
    }

    let at = model::now();
    let by = model::actor();
    let made: Vec<Issue> = repo.with_write(|issues, cfg| {
        let mut taken = taken_ids(issues);
        let mut ids: Vec<String> = Vec::with_capacity(drafts.len());
        let mut entries = Vec::new();
        let mut made = Vec::new();

        for (n, d) in drafts.iter().enumerate() {
            let id = crate::id::generate(&cfg.prefix, &taken, &crate::id::seed(&format!("{n}{}", d.title)));
            taken.insert(id.clone());
            ids.push(id.clone());

            let mut issue = Issue::new(id, d.title.clone(), d.kind, Status::new(cfg.first_status()), &at);
            issue.priority = d.priority;
            issue.tags = d.tags.clone();
            issue.epic = d.epic.map(|at| ids[at].clone());
            issue.normalize();
            issue.validate(cfg)?;
            entries.push(JournalEntry::create(&issue.id, &issue.title, &at, &by));
            made.push(issue.clone());
            issues.push(issue);
        }
        Ok((entries, made))
    })?;

    if ctx.json {
        return super::json_line(&made);
    }
    let mut out = vec![paint(style::HEAD, "만듦")];
    out.extend(drafts.iter().zip(&made).map(|(d, i)| line_of(d, Some(&i.id))));
    out.push(String::new());
    out.push(tally(&drafts));
    Ok(out)
}

fn line_of(d: &Draft, id: Option<&str>) -> String {
    let head = match id {
        Some(id) => paint(style::ID, id),
        None => String::new(),
    };
    let mark = match d.kind {
        Kind::Issue => paint(
            style::priority_style(d.priority.unwrap_or(model::DEFAULT_PRIORITY)),
            &format!("p{}", d.priority.unwrap_or(model::DEFAULT_PRIORITY)),
        ),
        _ => paint(style::EPIC, d.kind.as_str()),
    };
    let indent = if d.epic.is_some() { "    " } else { "  " };
    let tags = if d.tags.is_empty() {
        String::new()
    } else {
        format!("   {}", paint(style::TAG, &d.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")))
    };
    format!("{indent}{head}  {mark}  {}{tags}", d.title).trim_end().to_string()
}

fn tally(drafts: &[Draft]) -> String {
    let epics = drafts.iter().filter(|d| d.kind == Kind::Epic).count();
    paint(style::DIM, &format!("에픽 {epics} · 이슈 {}", drafts.len() - epics))
}
