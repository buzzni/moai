//! 이슈를 만든다. 승인을 묻지 않는다 — 이 도구에 게이트는 없다.

use super::{Ctx, Fail, R};
use crate::cli::AddArgs;
use crate::draft::{self, Draft};
use crate::model::{self, Actor, Issue, JournalEntry, Kind, Status};
use crate::store::{self, Repo, taken_ids};
use crate::style::{self, paint};

/// 담당을 정한다. **`-a` 를 안 주면 만든 사람이 담당이다** — 이름 없는 줄이
/// 쌓이는 것이 기본값이면 나중에 누가 무엇을 들고 있는지 아무도 모른다.
/// 담당 없이 만들려면 `-a none` 이다 (`edit` 이 비우는 법과 같다).
fn assignee_of(arg: Option<&str>, by: &Actor) -> (Option<String>, Option<String>) {
    match arg.map(super::clearable) {
        Some(Some(v)) => model::split_assignee(&v),
        Some(None) => (None, None),
        None => by.as_assignee(),
    }
}

/// `--from` 이 받는 한 덩이. `-` 이면 stdin, 아니면 파일이다.
///
/// [`read_plan`] 만 부른다 — `add --from` 과 `idea promote --from` 은 그쪽 한 길로 읽는다. 여기를
/// 따로 부르면 템플릿 채우기를 건너뛴 계획이 선다.
fn read_source(from: &str) -> R<String> {
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

/// `--from` 의 계획 한 덩이를 읽어 **형식을 읽고 템플릿 변수를 채운다**(moai-cypw).
///
/// `add --from` 과 `idea promote --from` 이 **이것만** 부른다 — 한쪽만 `--var` 를 받거나 한쪽만
/// 거절 문장이 달라지지 않게. 읽기와 채우기는 `draft::fill` 한 자리다 — 값은 형식을 안 지나 늘
/// 제목 글자다.
///
/// `--var` 는 `이름=값` 이다. `=` 이 없거나 이름이 비거나 변수 이름의 모양(`draft::is_var_name`)이
/// 아니면 그 인자를 대며 거절하고, **같은 이름을 두 번 주면 거절한다** — 어느 값이 이길지 조용히
/// 고르면 템플릿이 사람이 안 적은 계획을 찍는다. 이 거절과 `fill` 의 거절은 모두 `bad_input` 이다.
pub fn read_plan(from: &str, vars: &[String], shape: draft::Shape, lang: crate::i18n::Lang) -> R<Vec<draft::Draft>> {
    use crate::i18n::{fill, say};
    let bad = |msg: String| Fail::coded(msg, super::code::BAD_INPUT);
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    // 틀린 `--var` 는 **전부** 모아 한 번에 말한다 — `fill`·`parse` 가 줄을 그렇게 말하는 것과 같다.
    let mut errors: Vec<String> = Vec::new();
    for raw in vars {
        let Some((name, value)) = raw.split_once('=').filter(|(n, _)| !n.is_empty()) else {
            errors.push(fill(say(lang, "refuse.var_not_a_pair"), &[("raw", raw)]));
            continue;
        };
        // **갈래마다 제 키를 적는다** — 키를 변수로 넘기면 소스를 훑는 시험의 눈에서 사라진다.
        let why = if !draft::is_var_name(name) {
            // 이름은 계획이 변수로 읽는 모양 그대로다 — 안 그러면 `{{버전}}` 이 든 계획에 `--var 버전=…` 을
            // 준 사람이 "계획에 없는 변수" 라는 틀린 말을 듣는다.
            say(lang, "refuse.var_name_shape")
        } else if value.contains(['\n', '\r']) {
            // 제목은 한 줄이다 — 줄바꿈이 든 값은 여러 줄 제목을 세운다.
            say(lang, "refuse.var_value_lines")
        } else if value.trim().is_empty() {
            // **빈 값은 거절한다**(사람이 정했다, 리뷰 moai-cypw.nn4). 셸 변수가 비어 `--var version=$VERSION`
            // 이 빈 값이 되면 "릴리스 " 같은 반쯤 채운 제목이 조용히 선다 — 전부 필수가 막으려던 그것이다.
            say(lang, "refuse.var_value_empty")
        } else if pairs.iter().any(|(k, _)| *k == name) {
            say(lang, "refuse.var_twice")
        } else {
            pairs.push((name, value));
            continue;
        };
        errors.push(fill(why, &[("name", name)]));
    }
    if !errors.is_empty() {
        return Err(bad(errors.join("\n      ")));
    }
    let src = read_source(from)?;
    draft::fill_as(&src, &pairs, shape, lang).map_err(bad)
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
    let repo = super::open_repo(ctx)?;
    if let Some(from) = &args.from {
        // **마크다운은 에픽과 이슈를 낸다.** `#` 이 에픽이고 `-` 가 이슈라는
        // 뜻이 형식에 박혀 있어 종류 고정 장치가 여기까지 못 온다. 다른
        // 네임스페이스는 그래도 뜻이 통하지만(`epic add --from` 은 에픽을
        // 낸다) idea 는 정반대다 — 일로 세지 않으려고 담은 것이 그대로
        // 보드에 선다. 조용히 그렇게 하느니 어디로 가야 하는지 말한다.
        // **두 철자를 한 자리에서 막는다.** `--type idea` 만 지나가면 그쪽이
        // 그대로 보드에 이슈를 만든다.
        //
        // **마크다운이 못 내는 종류는 전부 막는다.** 하나만 막으면 나머지가
        // 조용히 딴것을 만든다 — `moai milestone add --from -` 이 실제로
        // 마일스톤 없이 에픽과 이슈를 냈다. 거절 목록이 아니라 "이 형식이
        // 내는 것"(`draft::parse` → 에픽·이슈)의 뒷면이고, **그래서 통과
        // 목록으로 적는다**: `_ => {}` 로 닫으면 종류가 하나 더 붙는 날 그것이
        // 또 조용히 지나가지만, 남김없이 적으면 그날 컴파일러가 이 자리를
        // 이름으로 댄다.
        match kind_override.or(args.kind) {
            None | Some(Kind::Issue) | Some(Kind::Epic) => {}
            Some(Kind::Idea) => {
                let lang = ctx.lang();
                return Err(Fail::coded(
                    format!(
                        "{}\n      {}",
                        crate::i18n::say(lang, "refuse.plan_is_not_an_idea"),
                        crate::i18n::say(lang, "refuse.plan_is_not_an_idea_how"),
                    ),
                    super::code::BAD_INPUT,
                ));
            }
            Some(Kind::Milestone) => {
                let lang = ctx.lang();
                return Err(Fail::coded(
                    format!(
                        "{}\n      {}",
                        crate::i18n::say(lang, "refuse.plan_has_no_milestone"),
                        crate::i18n::say(lang, "refuse.plan_has_no_milestone_how"),
                    ),
                    super::code::BAD_INPUT,
                ));
            }
        }
        return bulk(ctx, &repo, from, &args.var, args.dry_run, args.assignee.clone(), args.milestone.as_deref());
    }
    // **`--var` 도 `--from` 이 있어야 뜻이 있다**(moai-cypw) — 아래 `--dry-run` 과 같은 까닭이다.
    // 조용히 버리면 템플릿을 채운 줄 안 사람이 `{{이름}}` 이 아닌 제목 하나를 만든다.
    if !args.var.is_empty() {
        // idea 가 템플릿을 쓰는 길은 펼치기다 — 위의 `--from` 거절이 idea 에 가리키는 곳과 같게 댄다.
        let lang = ctx.lang();
        let how = match kind_override.or(args.kind) {
            Some(Kind::Idea) => crate::i18n::say(lang, "refuse.var_needs_from_idea"),
            _ => crate::i18n::say(lang, "refuse.var_needs_from_add"),
        };
        return Err(Fail::coded(
            format!("{}\n      `{how}`", crate::i18n::say(lang, "refuse.var_needs_from")),
            super::code::BAD_INPUT,
        ));
    }
    // **연습이라 적힌 명령이 쓰면 안 된다.** 여기 닿았다는 것은 `--from` 이
    // 없다는 뜻이고, 하나짜리에는 연습 길이 없어 `--dry-run` 이 그대로 만들고
    // 있었다 — 막는 줄 알고 부른 명령이 쓰는 것보다 나쁜 것은 없다.
    // `clap` 에 맡길 수 없다: `requires = "from"` 은 제목이 있으면(=`from` 과
    // conflicts) 못 채울 요구로 보고 조용히 건너뛴다.
    if args.dry_run {
        let lang = ctx.lang();
        return Err(Fail::coded(
            format!(
                "{}\n      {}",
                crate::i18n::say(lang, "refuse.dry_run_needs_from"),
                crate::i18n::say(lang, "refuse.dry_run_needs_from_how"),
            ),
            super::code::BAD_INPUT,
        ));
    }
    let Some(title) = args.title.clone().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) else {
        return Err(Fail::new(crate::i18n::say(ctx.lang(), "refuse.add_no_title")));
    };
    super::refuse_if_flag_like(&title, ctx.lang())?;
    let body = read_body(args.body)?;
    let kind = kind_override.or(args.kind).unwrap_or_default();
    let status = Status::new(args.status.clone().unwrap_or_else(|| repo.config.first_status().to_string()));
    // 칸 검사는 id 를 뽑기 **전에** 한다. 나중에 하면 쓰이지도 않은 id 가
    // 오류 메시지에 실려 나가고, 받는 쪽은 그게 만들어진 줄 안다.
    repo.config
        .require_known(status.as_str())
        .map_err(|e| Fail::coded(crate::view::no_such_column(ctx.lang(), &e), super::code::BAD_STATUS))?;
    let at = model::now();
    let by = model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, ctx.lang()))?;

    // 만든 줄과, 그것이 묶음이면 **멤버에서 읽은 칸.** 에픽을 먼저 만들고 멤버를
    // 나중에 다는 순서가 흔하지만 그 반대도 있다 — 이미 멤버가 있는 에픽을 뒤늦게
    // 만들면 만든 줄이 처음부터 `in_progress` 로 선다.
    let (made, read): (Issue, super::Read) = repo.with_write(
        || ctx.lang(),
        |issues, cfg, reserved| {
            if let Some(p) = &args.parent
                && !issues.iter().any(|i| &i.id == p)
            {
                return Err(Fail::coded(
                    crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.no_such_parent"), &[("id", p)]),
                    super::code::NOT_FOUND,
                ));
            }
            let id = store::new_id(issues, cfg, reserved, args.parent.as_deref(), &title);

            // 없는 에픽을 가리키는 것은 막지 않고 **알려만 준다** — 에픽을 나중에
            // 만드는 순서가 실제로 있고, 끊긴 참조는 `moai status` 가 드러낸다.
            if let Some(e) = &args.epic
                && !issues.iter().any(|i| &i.id == e)
            {
                eprintln!(
                    "moai: {}",
                    crate::i18n::fill(crate::i18n::say(ctx.lang(), "add.no_such_epic_yet"), &[("id", e)])
                );
            }

            let mut issue = Issue::new(id, title.clone(), kind, status.clone(), &at);
            issue.epic = args.epic.clone();
            issue.milestone = args.milestone.clone();
            issue.tags = args.tag.iter().map(|t| model::normalize_tag(t)).collect();
            issue.priority = args.priority;
            // 기한 둘(moai-tfcp) — 만들 때 바로 준다. 안 주면 `milestone add` 뒤에 `edit` 를
            // 한 번 더 쳐야 하고, 그 사이의 줄은 기한 없는 마일스톤으로 한 번 커밋된다.
            // `none` 은 여기서 뜻이 없다(새 줄에 비울 것이 없다) — 빈 값은 `normalize` 가
            // 지우고, 꼴·종류·차례는 락 안에서 `Repo::write_locked` 가 거절한다
            // (`store::admit` 은 검사를 안 한다, moai-yve0).
            issue.starts_on = args.start.clone();
            issue.due_on = args.due.clone();
            (issue.assignee, issue.assignee_email) = assignee_of(args.assignee.as_deref(), &by);
            issue.body = body.clone();
            let (entry, issue) = store::admit(issues, cfg, issue, &by)?;
            let read = super::read_of(issues, cfg, &[issue.id.as_str()]);
            Ok((vec![entry], (issue, read)))
        },
    )?;

    if ctx.json {
        return super::json_line(&super::Row::from(&made, &read));
    }
    if args.quiet {
        return Ok(vec![made.id]);
    }

    // **만든 줄도 서 있는 칸으로 그린다.** 묶음의 적힌 칸은 어디서도 안 읽히므로
    // (`report::column`), 여기서만 그것을 그리면 `moai add --type epic -s done` 이
    // 낸 `✓` 를 바로 다음 `moai show` 가 `· todo` 로 뒤집는다.
    let states: std::collections::BTreeMap<&str, &str> =
        read.iter().map(|(id, col)| (id.as_str(), col.as_str())).collect();
    let col = crate::report::column(&made, &states);
    let st = style::status_style(col);
    let mut line = format!(
        "{}  {}  {}  {}",
        paint(style::ID, &made.id),
        paint(style::priority_style(made.priority()), &format!("p{}", made.priority())),
        paint(st, style::glyph(col)),
        paint(
            // 묶음만 묶음 색이다. idea 는 담는 것이 아니라 담기는 것이라
            // 여기서 갈라지면 만든 순간부터 에픽처럼 보인다.
            if crate::report::is_group(&made) { style::EPIC } else { style::PLAIN },
            &made.title
        ),
    );
    if !made.tags.is_empty() {
        line.push_str(&format!("   {}", paint(style::TAG, &crate::view::tags_of(&made))));
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
fn bulk(
    ctx: &Ctx,
    repo: &Repo,
    from: &str,
    vars: &[String],
    dry_run: bool,
    assignee: Option<String>,
    milestone: Option<&str>,
) -> R<Vec<String>> {
    let drafts = read_plan(from, vars, draft::Shape::Plan, ctx.lang())?;
    // **마일스톤은 계획의 뿌리로 간다**(moai-xoyg) — 에픽에 서고 이슈는 물려받는다.
    let rooted = Rooted { milestone, body: None };

    if dry_run {
        check_plan(&drafts, milestone, ctx.lang())?;
        if ctx.json {
            return json_rehearsal(&drafts, None, None, milestone);
        }
        // 만들지 않으므로 id 가 없다. 무엇이 어디에 붙는지만 보여 준다.
        let mut out = vec![paint(style::HEAD, crate::i18n::say(ctx.lang(), "add.will_make"))];
        out.extend(drafts.iter().map(|d| line_of(d, None)));
        out.push(String::new());
        out.push(tally(&drafts, ctx.lang()));
        out.extend(milestone_line(milestone, ctx.lang()));
        return Ok(out);
    }

    let at = model::now();
    let by = model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, ctx.lang()))?;
    let who = assignee_of(assignee.as_deref(), &by);
    let (made, read): (Vec<Issue>, super::Read) = repo.with_write(
        || ctx.lang(),
        |issues, cfg, reserved| {
            let (entries, made) = create_drafts(issues, cfg, reserved, &drafts, None, rooted, &who, &by, &at)?;
            let ids: Vec<&str> = made.iter().map(|i| i.id.as_str()).collect();
            let read = super::read_of(issues, cfg, &ids);
            Ok((entries, (made, read)))
        },
    )?;

    if ctx.json {
        let rows: Vec<super::Row> = made.iter().map(|i| super::Row::from(i, &read)).collect();
        return super::json_line(&rows);
    }
    let mut out = vec![paint(style::HEAD, crate::i18n::say(ctx.lang(), "add.made"))];
    out.extend(drafts.iter().zip(&made).map(|(d, i)| line_of(d, Some(&i.id))));
    out.push(String::new());
    out.push(tally(&drafts, ctx.lang()));
    // **만든 줄에서 읽는다** — `idea promote` 와 한 자리다(리뷰). 적은 값을 그대로 찍으면
    // 같은 한 줄을 한쪽은 argv 로, 한쪽은 파일로 셈해, 쓰기에 정규화가 붙는 날 둘이 갈린다.
    out.extend(milestone_line(stood_on(&made), ctx.lang()));
    Ok(out)
}

/// 만든 줄에서 **뿌리가 선 마일스톤**을 읽는다 — 적은 값이 아니라 써진 값이다.
///
/// `add --from` 과 `idea promote` 가 이 한 자리를 쓴다(리뷰). 뿌리가 없으면(`-e <에픽>`)
/// `None` 이고, 그것이 그대로 답이다 — 그 에픽이 이미 임자다.
///
/// **뿌리는 최상위 id 다**(moai-exh7). 멤버는 에픽의 자식 id 를 받고 제 `epic` 을 안 적으므로
/// (`create_drafts`), 그 필드로 뿌리를 가르던 자는 멤버를 전부 뿌리로 읽는다.
pub fn stood_on(made: &[Issue]) -> Option<&str> {
    made.iter().find(|i| crate::id::parent_of(&i.id).is_none()).and_then(|i| i.milestone.as_deref())
}

/// 계획의 **뿌리**(제 에픽이 없는 줄)가 받아 갈 것. 멤버는 거기서 물려받으므로 여기서 안 적는다 —
/// 멤버마다 적으면 그것이 파생값을 저장하는 것이고, 에픽을 옮기는 날 멤버가 안 따라온다.
///
/// `add --from --milestone` 이 마일스톤을 주고(moai-xoyg), `idea promote` 가 담아 둔 생각의
/// 마일스톤과 본문을 준다(moai-07v1). **`-e <에픽>` 으로 선 에픽에 펼칠 때는 뿌리가 없어**
/// 아무것도 안 서는데, 그것이 맞는 답이다 — 그 에픽이 이미 임자다.
///
/// **둘이 뿌리를 세는 법이 다르다**(리뷰). 계획은 `#` 줄을 여럿 받으므로 뿌리도 여럿일 수
/// 있는데, 마일스톤은 멤버가 물려받는 값이라 뿌리마다 서야 하고 본문은 물려받는 값이
/// 아니라 **첫 뿌리 하나**만 든다 — 뿌리마다 적으면 64KB 짜리 글이 에픽 수만큼 베껴진다.
///
/// **`Default` 를 안 단다.** 아무것도 안 적는다는 뜻의 `Rooted::default()` 가 있으면 필드가
/// 하나 붙는 날 `..Default::default()` 가 그것을 말없이 삼킨다 — 지금은 둘 다 통째로 적는
/// 자리라 필드가 붙으면 컴파일러가 그 둘을 이름으로 댄다.
#[derive(Clone, Copy)]
pub struct Rooted<'a> {
    pub milestone: Option<&'a str>,
    pub body: Option<&'a str>,
}

/// 초안 묶음을 실제 이슈로 빚어 `issues` 에 밀어 넣는다. **쓰기 트랜잭션
/// 안에서 부른다** — 다 되거나 하나도 안 된다는 성질이 그 트랜잭션에서 온다.
///
/// `add --from` 과 `idea promote` 가 이 한 길을 같이 쓴다. 둘이 갈라지면
/// 같은 마크다운이 어느 쪽으로 들어왔느냐에 따라 다른 이슈가 된다.
///
/// 담당은 **이미 갈라진 채로** 받는다 (`(이름, 메일)`). 부르는 쪽이 `-a` 를
/// 풀든 남의 줄에서 물려받든, 여기 닿을 때는 파일에 쓸 모양이어야 한다 —
/// 화면용 `"이름 (메일)"` 한 줄로 받으면 되가르는 데서 이름 없는 줄의 메일이
/// 이름 칸으로 넘어간다.
pub fn create_drafts(
    issues: &mut Vec<Issue>,
    cfg: &crate::config::Config,
    reserved: &std::collections::BTreeSet<String>,
    drafts: &[Draft],
    into: Option<&str>,
    rooted: Rooted<'_>,
    who: &(Option<String>, Option<String>),
    by: &Actor,
    at: &str,
) -> R<(Vec<JournalEntry>, Vec<Issue>)> {
    let mut taken = taken_ids(issues, reserved);
    let mut ids: Vec<String> = Vec::with_capacity(drafts.len());
    let mut entries = Vec::new();
    let mut made = Vec::new();
    // **본문은 첫 뿌리가 가져간다** — 아래에서 `take` 하므로 둘째 뿌리부터는 빈손이다.
    let mut body = rooted.body;

    for (n, d) in drafts.iter().enumerate() {
        // 이 초안이 들 에픽 — 초안이 가리킨 `#` 줄(차례 번호를 방금 만든 id 로 바꾼다)이거나,
        // 이미 선 에픽(`into`, `Shape::Members`)이다. 닫힌 이름을 `at` 으로 두면 시각을 담은
        // 인자 `at` 을 가려, 같은 줄에서 같은 이름이 두 가지를 뜻한다.
        let under: Option<String> = d.epic.map(|nth| ids[nth].clone()).or_else(|| into.map(str::to_string));
        let seed = crate::id::seed(&format!("{n}{}", d.title));
        // **멤버는 에픽의 자식 id 를 받는다**(moai-s8go, 2026-09-23 사용자 결정) — 과제든
        // 리뷰든 한 주제이기 때문이다. 만드는 자는 이미 있었고(`id::generate_child`), 계획을
        // 세우는 이 길만 그것을 안 불렀다 — `--parent <에픽>` 으로 세우는 리뷰 이슈는 처음부터
        // 자식이었다. **옛 줄의 id 는 안 건드린다**: 새로 펼치는 계획만 이 모양을 받는다.
        let id = match under.as_deref() {
            Some(p) => crate::id::generate_child(p, &taken, &seed),
            None => crate::id::generate(&cfg.prefix, &taken, &seed),
        };
        taken.insert(id.clone());
        ids.push(id.clone());

        let mut issue = Issue::new(id, d.title.clone(), d.kind, Status::new(cfg.first_status()), at);
        issue.priority = d.priority;
        issue.tags = d.tags.clone();
        // **소속은 id 하나에만 선다**(moai-exh7, 2026-09-23 사용자 결정). 멤버의 id 가 에픽 밑에
        // 붙으므로 `report::groups` 가 "부모가 에픽이면 그 에픽이 소속"(moai-9t3l)으로 이미
        // 물려준다 — 여기서 `epic` 을 또 적으면 같은 사실이 두 자리에 서고, 그것이 파생값을
        // 저장하는 일이다. `-e <에픽>` 으로 선 에픽에 펼칠 때도 같다(`into` 도 자식으로 민다).
        //
        // **뿌리는 `under` 가 없는 줄이다** — 만든 줄의 `epic` 으로 재던 때는 이 필드를 비우는
        // 순간 멤버가 전부 뿌리로 읽혀, 마일스톤과 본문이 줄마다 한 벌씩 섰다.
        if under.is_none() {
            // **뿌리에만 적는다**(moai-xoyg·moai-07v1). 멤버는 이 뿌리에서 물려받으니 여기
            // 적으면 같은 값이 줄마다 한 벌씩 서고, 에픽을 옮기는 날 그 줄들이 옛 자리에 남는다.
            issue.milestone = rooted.milestone.map(str::to_string);
            // **본문은 첫 뿌리에만 간다**(리뷰). 마일스톤은 멤버가 물려받으니 뿌리마다 서야
            // 하지만 본문은 물려받는 값이 아니다 — 계획은 `#` 줄을 여럿 받으므로
            // (`draft::Shape::Plan`) 뿌리마다 적으면 같은 글이 에픽 수만큼 베껴진다. 재 봤다:
            // 64KB 짜리 생각을 에픽 200개 계획으로 펼치면 스냅샷이 13MB 가 됐고, 매 쓰기가
            // 전체 재작성이라 그 뒤의 모든 명령이 그것을 다시 찍는다. 베낀 글은 한쪽을 고친 날
            // 나머지가 옛 글로 남아, 왜 한 묶음인지를 묻는 쪽이 어느 것을 믿을지 모른다.
            //
            // **덮어쓰지 않고 채운다** — 초안이 언젠가 제 본문을 들면, `= None` 은 그것을
            // 말없이 지운다.
            if let Some(text) = body.take() {
                issue.body = Some(text.to_string());
            }
        }
        (issue.assignee, issue.assignee_email) = who.clone();
        let (entry, issue) = store::admit(issues, cfg, issue, by)?;
        entries.push(entry);
        made.push(issue);
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
        format!("   {}", paint(style::TAG, &crate::view::tag_line(&d.tags)))
    };
    format!("{indent}{head}  {mark}  {}{tags}", d.title).trim_end().to_string()
}

/// 계획의 제목들이 상한 안인가 — **연습이 진짜와 같은 것을 보게 하는 자**(moai-5229).
///
/// 연습은 사람이 "좋다" 하는 자리다(AGENTS.md 갈림길 3). 크기를 안 재던 판은 66KB 계획에 0 으로
/// 끝나며 `만들 것` 을 찍고, 같은 부름을 진짜로 하면 거절했다 — 승인한 뒤에 도구가 거절하는 꼴이다.
///
/// 재는 자는 진짜와 **한 자리**다([`crate::model::check_text_size`]). 여기에 따로 적으면 상한이
/// 두 곳에 서고, 한쪽만 고치는 날 이 어긋남이 그대로 돌아온다. 가리키는 말도 같다 —
/// 연습에도 진짜에도 아직 id 가 없다.
///
/// **여기서 거른다고 연습과 진짜가 같아진 것은 아니다.** 진짜는 `store::with_write` 에서
/// `Issue::validate` 도 지나고 사람 이름도 푼다 — 쉼표가 든 태그(`#bug,perf`)나 git 사용자 정보가
/// 없는 기계는 아직 연습을 지나 진짜에서 거절당한다. 그 자리를 닫는 길은 검사를 하나씩 옮겨
/// 적는 것이 아니라 초안을 **id 를 뽑기 전에** 이슈로 빚어 한 번에 재는 것이다.
///
/// **마일스톤의 모양도 여기서 잰다**(리뷰). `--milestone` 이 계획에 닿은 날(moai-xoyg) 이
/// 어긋남이 다시 열렸다 — `moai add --from - --milestone none --dry-run` 이 0 으로 끝나며
/// `에픽은 마일스톤 none 에 선다` 를 찍는데, 같은 부름을 진짜로 하면 `Issue::validate_fields`
/// 가 1 로 거절한다. 가리키는 줄도 말도 진짜와 한 자리에서 나온다 —
/// `store::write_locked` 가 같은 `Invalid::GroupId` 를 같은 `At::Unwritten` 으로 낸다.
pub fn check_plan(drafts: &[Draft], milestone: Option<&str>, lang: crate::i18n::Lang) -> R<()> {
    for d in drafts {
        // 가리키는 낱말도 진짜와 한 자리다 — `store::with_write` 가 같은 자리에서 `"title"` 을 준다.
        // 한글로 두던 판은 영어로 옮긴 거절문 안에 낱말 하나만 한국어로 남아, 연습과 진짜가 같은
        // 칸을 다른 이름으로 불렀다(리뷰).
        crate::model::check_text_size(|| crate::model::unwritten(&d.title), "title", &d.title)?;
    }
    // 진짜가 거절하는 줄은 **이 마일스톤을 받을 첫 뿌리**다(`create_drafts` 가 밀어 넣는 차례).
    // 뿌리가 없는 꼴(`Shape::Members`)에는 마일스톤도 안 오지만, 그래도 가리킬 줄은 댄다.
    if let Some(m) = milestone
        && !crate::id::is_valid(m)
    {
        let at = drafts.iter().find(|d| d.epic.is_none()).or_else(|| drafts.first());
        let at = store::At::Unwritten(crate::model::fit_title(at.map_or("", |d| d.title.as_str())));
        let why = model::Invalid::GroupId { field: model::Field::Milestone, value: format!("{m:?}") };
        // 코드도 진짜와 같다 — `store::Trouble::Invalid` 가 `code::ERROR` 로 나간다.
        return Err(Fail::new(crate::view::invalid(lang, &at, &why)));
    }
    Ok(())
}

/// 연습의 기계 출력. **`--dry-run` 도 `--json` 을 지킨다** — 연습은 계획을
/// 미리 보는 자리인데 거기서만 사람 글이 나오면, 미리 보는 쪽은 파싱에
/// 실패하고 결국 진짜로 만들어 보고서야 계획을 읽는다.
///
/// `add --from` 과 `idea promote` 가 이 한 자리를 같이 쓴다. 둘이 갈라지면
/// 같은 마크다운이 어느 동사로 들어왔느냐에 따라 다른 모양이 되고, 미리
/// 검사하는 코드가 두 벌 필요해진다. `promoted` 는 펼칠 때만 붙는다.
///
/// `dry_run` 을 적어 두는 것은 **id 가 없는 까닭**이 거기서 나오기 때문이다.
/// 진짜 출력은 만든 줄을 그대로 내므로, 이 깃발이 두 모양을 가른다.
pub fn json_rehearsal(
    drafts: &[Draft],
    promoted: Option<&str>,
    into: Option<&str>,
    milestone: Option<&str>,
) -> R<Vec<String>> {
    /// 기본 우선순위는 **여기서 풀어 낸다.** `null` 을 내면 받는 쪽이 기본값을
    /// 다시 알아야 하고, 그러면 그 값이 두 곳에 적힌다. 우선순위가 없는 종류
    /// (에픽·마일스톤)에는 아예 내지 않는다 — 없는 것에 기본값을 씌우면
    /// 에픽이 `p2` 인 줄 안다.
    #[derive(serde::Serialize)]
    struct DraftOut<'a> {
        kind: &'a str,
        title: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<u8>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tags: &'a Vec<String>,
        /// 몇 번째 초안이 제 에픽인가. 에픽 자신은 없다 — 아직 id 가 없으므로
        /// 자리로 말하는 수밖에 없고, 그 자리는 `drafts` 의 첨자다.
        #[serde(skip_serializing_if = "Option::is_none")]
        epic: Option<usize>,
    }
    #[derive(serde::Serialize)]
    struct Out<'a> {
        dry_run: bool,
        drafts: Vec<DraftOut<'a>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        promoted: Option<&'a str>,
        /// 이슈가 들 이미 선 에픽 (`idea promote -e`). 초안의 `epic` 은 첨자라 거기에 못 적는다
        #[serde(skip_serializing_if = "Option::is_none")]
        into: Option<&'a str>,
        /// 뿌리인 에픽이 설 마일스톤. **없으면 안 낸다** — 그 없음이 답이다(AGENTS.md).
        /// 본문은 안 낸다: [`milestone_line`] 에 그 까닭이 있다
        #[serde(skip_serializing_if = "Option::is_none")]
        milestone: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<&'a str>,
    }
    super::json_line(&Out {
        dry_run: true,
        drafts: drafts
            .iter()
            .map(|d| DraftOut {
                kind: d.kind.as_str(),
                title: &d.title,
                // **적어 온 것은 그대로 낸다.** 마크다운은 `# [p1] 제목` 도
                // 받고 `create_drafts` 는 그것을 에픽에도 그대로 쓰므로,
                // 종류로 잘라 내면 연습이 진짜보다 적게 말한다 — 연습을 진짜와
                // 견주는 쪽이 안 적힌 값을 기본값으로 읽는다.
                // 안 적혔을 때만 종류를 본다: 우선순위가 없는 종류에 기본값을
                // 씌우면 에픽이 `p2` 인 줄 안다.
                priority: d.priority.or_else(|| (d.kind == Kind::Issue).then_some(model::DEFAULT_PRIORITY)),
                tags: &d.tags,
                epic: d.epic,
            })
            .collect(),
        promoted,
        into,
        milestone,
        // 펼치면 그 생각이 닫힌다는 말은 진짜 출력과 **같은 낱말**로 한다.
        status: promoted.map(|_| crate::config::DONE),
    })
}

/// 만든(또는 만들) 에픽이 어느 마일스톤에 서는지 한 줄. **연습도 진짜도 같은 줄을 낸다** —
/// 마일스톤은 도는 판에서 `ready` 가 무엇을 먼저 내주는지를 가르는 값이라, 조용히 서면
/// 계획을 세운 쪽이 그것을 `moai show` 로 한 번 더 확인해야 한다.
///
/// **본문은 여기서 안 낸다**(moai-07v1). 그것은 펼치는 idea 가 이미 들고 있는 글이고
/// (`moai show <idea>`), 64KB 짜리 본문을 연습이 한 번 더 찍으면 계획이 그 글에 묻힌다.
pub fn milestone_line(milestone: Option<&str>, lang: crate::i18n::Lang) -> Option<String> {
    let id = milestone?;
    let said = crate::i18n::fill(crate::i18n::say(lang, "add.on_milestone"), &[("id", id)]);
    Some(paint(style::DIM, &said))
}

pub fn tally(drafts: &[Draft], lang: crate::i18n::Lang) -> String {
    let epics = drafts.iter().filter(|d| d.kind == Kind::Epic).count();
    let said = crate::i18n::fill(
        crate::i18n::say(lang, "add.tally"),
        &[("epics", &epics.to_string()), ("issues", &(drafts.len() - epics).to_string())],
    );
    paint(style::DIM, &said)
}
