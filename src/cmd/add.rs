//! 이슈를 만든다. 승인을 묻지 않는다 — 이 도구에 게이트는 없다.

use super::{Ctx, Fail, R};
use crate::cli::AddArgs;
use crate::draft::{self, Draft};
use crate::model::{self, Actor, Issue, JournalEntry, Kind, Status};
use crate::store::{Repo, taken_ids};
use crate::style::{self, paint};

/// 담당을 정한다. **`-a` 를 안 주면 만든 사람이 담당이다** — 이름 없는 줄이
/// 쌓이는 것이 기본값이면 나중에 누가 무엇을 들고 있는지 아무도 모른다.
/// 담당 없이 만들려면 `-a none` 이다 (`edit` 이 비우는 법과 같다).
fn assignee_of(arg: Option<&str>, by: &Actor) -> (Option<String>, Option<String>) {
    match arg.map(super::clearable) {
        Some(Some(v)) => model::split_assignee(&v),
        Some(None) => (None, None),
        None => (Some(by.name.clone()), Some(by.email.clone())),
    }
}

/// `--from` 이 받는 한 덩이. `-` 이면 stdin, 아니면 파일이다.
///
/// **`add --from` 과 `idea promote --from` 이 이 한 길을 같이 쓴다.** 갈라지면
/// 한쪽만 파일을 받거나 한쪽만 오류 문장이 달라진다.
pub fn read_source(from: &str) -> R<String> {
    match from {
        "-" => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                .map_err(|e| Fail::new(format!("stdin: {e}")))?;
            Ok(s)
        }
        path => std::fs::read_to_string(path).map_err(|e| Fail::new(format!("{path}: {e}"))),
    }
}

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
        // **마크다운은 에픽과 이슈를 낸다.** `#` 이 에픽이고 `-` 가 이슈라는
        // 뜻이 형식에 박혀 있어 종류 고정 장치가 여기까지 못 온다. 다른
        // 네임스페이스는 그래도 뜻이 통하지만(`epic add --from` 은 에픽을
        // 낸다) idea 는 정반대다 — 일로 세지 않으려고 담은 것이 그대로
        // 보드에 선다. 조용히 그렇게 하느니 어디로 가야 하는지 말한다.
        // **두 철자를 한 자리에서 막는다.** `--type idea` 만 지나가면 그쪽이
        // 그대로 보드에 이슈를 만든다.
        if kind_override.or(args.kind) == Some(Kind::Idea) {
            return Err(Fail::coded(
                "생각은 제목 하나로 담는다 — `moai idea add \"반짝 떠오른 것\"`\n      \
                 마크다운으로 에픽과 이슈를 펼치는 것은 `moai idea promote <id> --from -` 다"
                    .to_string(),
                super::code::BAD_INPUT,
            ));
        }
        return bulk(ctx, &repo, from, args.dry_run, args.assignee.clone());
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
    let by = model::actor(ctx.user.as_deref())?;

    let made: Issue = repo.with_write(|issues, cfg, reserved| {
        let taken = taken_ids(issues, reserved);

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
        (issue.assignee, issue.assignee_email) = assignee_of(args.assignee.as_deref(), &by);
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
            // 묶음만 묶음 색이다. idea 는 담는 것이 아니라 담기는 것이라
            // 여기서 갈라지면 만든 순간부터 에픽처럼 보인다.
            if crate::report::is_group(&made) { style::EPIC } else { style::PLAIN },
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
fn bulk(ctx: &Ctx, repo: &Repo, from: &str, dry_run: bool, assignee: Option<String>) -> R<Vec<String>> {
    let src = read_source(from)?;
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
    let by = model::actor(ctx.user.as_deref())?;
    let made: Vec<Issue> = repo.with_write(|issues, cfg, reserved| {
        create_drafts(issues, cfg, reserved, &drafts, assignee.as_deref(), &by, &at)
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

/// 초안 묶음을 실제 이슈로 빚어 `issues` 에 밀어 넣는다. **쓰기 트랜잭션
/// 안에서 부른다** — 다 되거나 하나도 안 된다는 성질이 그 트랜잭션에서 온다.
///
/// `add --from` 과 `idea promote` 가 이 한 길을 같이 쓴다. 둘이 갈라지면
/// 같은 마크다운이 어느 쪽으로 들어왔느냐에 따라 다른 이슈가 된다.
pub fn create_drafts(
    issues: &mut Vec<Issue>,
    cfg: &crate::config::Config,
    reserved: &std::collections::BTreeSet<String>,
    drafts: &[Draft],
    assignee: Option<&str>,
    by: &Actor,
    at: &str,
) -> R<(Vec<JournalEntry>, Vec<Issue>)> {
    let mut taken = taken_ids(issues, reserved);
    let mut ids: Vec<String> = Vec::with_capacity(drafts.len());
    let mut entries = Vec::new();
    let mut made = Vec::new();

    for (n, d) in drafts.iter().enumerate() {
        let id = crate::id::generate(&cfg.prefix, &taken, &crate::id::seed(&format!("{n}{}", d.title)));
        taken.insert(id.clone());
        ids.push(id.clone());

        let mut issue = Issue::new(id, d.title.clone(), d.kind, Status::new(cfg.first_status()), at);
        issue.priority = d.priority;
        issue.tags = d.tags.clone();
        // 초안이 든 것은 **차례 번호**다 — 방금 만든 id 로 바꿔 넣는다.
        // 닫힌 이름을 `at` 으로 두면 시각을 담은 인자 `at` 을 가려, 같은
        // 줄에서 같은 이름이 두 가지를 뜻한다.
        issue.epic = d.epic.map(|nth| ids[nth].clone());
        (issue.assignee, issue.assignee_email) = assignee_of(assignee, by);
        issue.normalize();
        issue.validate(cfg)?;
        entries.push(JournalEntry::create(&issue.id, &issue.title, at, by));
        made.push(issue.clone());
        issues.push(issue);
    }
    Ok((entries, made))
}

pub fn line_of(d: &Draft, id: Option<&str>) -> String {
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

pub fn tally(drafts: &[Draft]) -> String {
    let epics = drafts.iter().filter(|d| d.kind == Kind::Epic).count();
    paint(style::DIM, &format!("에픽 {epics} · 이슈 {}", drafts.len() - epics))
}
