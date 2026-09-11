//! 하나를 펼치거나 목록을 낸다.
//!
//! `list` 를 따로 두지 않는다. 읽기 동사가 하나면 `--json`·필터·색 정책을
//! 한 번만 정의한다 — beads 는 `list` 52개 / `show` 9개 플래그로 갈라져
//! 겹치는 개념이 다른 이름을 갖는 값을 치르고 있다.

use super::{Ctx, Fail, R};
use crate::cli::ShowArgs;
use crate::model::{self, Issue, Kind};
use crate::query::{Filter, Raw};
use crate::report;
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

/// 목록 자리에서만 뜻이 있는 플래그가 왔는가. 온 것 중 첫 이름을 돌려준다.
fn first_given(a: &crate::cli::FilterArgs) -> Option<&'static str> {
    [
        (!a.status.is_empty(), "-s"),
        (!a.tag.is_empty(), "-t"),
        (!a.no_tag.is_empty(), "--no-tag"),
        (!a.epic.is_empty(), "-e"),
        (!a.parent.is_empty(), "--parent"),
        (!a.priority.is_empty(), "-p"),
        (a.kind.is_some(), "--type"),
        (a.grep.is_some(), "-g"),
        (a.stale.is_some(), "--stale"),
        (a.all, "--all"),
        (!a.filter.is_empty(), "--filter"),
    ]
    .into_iter()
    .find_map(|(given, name)| given.then_some(name))
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
        // 필터를 조용히 버리지 않는다. 하나를 콕 집었으면 거를 것이 없고,
        // 버린 채로 그 하나를 내면 부르는 쪽은 걸러진 결과라고 믿는다.
        if let Some(flag) = args.tree.then_some("--tree").or_else(|| first_given(&args.filter)) {
            return Err(Fail::coded(
                format!(
                    "`{id}` 하나를 펼치는 자리에는 `{flag}` 를 쓸 수 없다.\n      \
                     거르려면 id 없이 `moai show {flag} …` 다"
                ),
                "bad_filter",
            ));
        }
        let issue = load
            .get(id)
            .ok_or_else(|| Fail::coded(format!("{id} 를 못 찾았다"), "not_found"))?;
        return one(ctx, &repo, &load.issues, issue);
    }

    let a = args.filter;
    let kind = match target {
        Target::OfKind(k) => Some(k),
        _ => a.kind,
    };
    // argv 를 그대로 옮겨 담을 뿐이다. 뜻을 정하는 것은 `query` 다.
    let filter = Filter::build(Raw {
        status: a.status,
        tag: a.tag,
        no_tag: a.no_tag,
        epic: a.epic,
        parent: a.parent,
        priority: a.priority,
        kind,
        grep: a.grep,
        stale: a.stale,
        all: a.all,
        filter: a.filter,
    })
    .map_err(|e| Fail::coded(e, "bad_filter"))?;

    // 모르는 칸은 거부한다. 조용히 0건을 내면 `-s in-progress` 같은 오타가
    // "그 칸은 비었다" 와 구별되지 않는다 — `add`·`mv` 는 이미 거부한다.
    for s in &filter.status {
        repo.config.require_known(s).map_err(|e| Fail::coded(e, "bad_status"))?;
    }

    let now = model::now();
    // 한 번만 훑는다. done 을 숨기는 규칙은 `Filter` 하나가 알고, 여기서는
    // 그 규칙을 끈 채(`all`) 걸러 놓고 숨긴 것을 세기만 한다 — 두 번 훑으면
    // 두 판단이 어긋날 자리가 생긴다.
    let hide_done = !filter.all && filter.status.is_empty();
    let wide = Filter { all: true, ..filter };
    let mut shown: Vec<Issue> = Vec::new();
    let mut hidden = 0usize;
    for i in load.issues.iter().filter(|i| wide.matches(i, &now)) {
        if hide_done && i.status.is_done() {
            hidden += 1;
        } else {
            shown.push(i.clone());
        }
    }
    crate::query::sort_for_display(&mut shown);

    if ctx.json {
        return super::json_line(&shown);
    }
    if args.tree {
        return Ok(view::tree(
            &shown,
            &report::rollup(&load.issues, &repo.config),
            &report::groups(&load.issues),
        ));
    }
    Ok(view::list(&shown, &repo.config, hidden, &report::epic_labels(&load.issues)))
}

fn one(ctx: &Ctx, repo: &Repo, all: &[Issue], issue: &Issue) -> R<Vec<String>> {
    let epic = issue.epic.as_ref().and_then(|e| all.iter().find(|i| &i.id == e));
    let children = report::children_of(all, &issue.id);
    let journal = repo.journal_of(&issue.id)?;

    if ctx.json {
        let ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        let members: Vec<&str> =
            report::members_of(all, &issue.id).iter().map(|m| m.id.as_str()).collect();
        let mut extra = vec![
            ("children", serde_json::to_string(&ids).map_err(|e| Fail::new(e.to_string()))?),
            ("journal", serde_json::to_string(&journal).map_err(|e| Fail::new(e.to_string()))?),
        ];
        if report::is_epic(issue) {
            extra.push((
                "members",
                serde_json::to_string(&members).map_err(|e| Fail::new(e.to_string()))?,
            ));
        }
        return super::json_with(issue, &extra);
    }

    // 이력은 언제나 맨 끝이다. 에픽이면 멤버를 그 **앞에** 끼운다.
    let mut out = view::detail(issue, epic, &children, &[], &model::now());
    // 에픽을 펼치면 속한 이슈까지 보여 준다 — 에픽 하나를 보는 이유가
    // 그 밑에 무엇이 있는지 알려는 것이다.
    if report::is_epic(issue) {
        let groups = report::groups(all);
        let mine: Vec<Issue> = all
            .iter()
            .filter(|i| groups.get(i.id.as_str()) == Some(&issue.id.as_str()))
            .cloned()
            .collect();
        let roll = report::rollup(all, &repo.config)
            .into_iter()
            .find(|r| r.id.as_deref() == Some(issue.id.as_str()));
        if let Some(r) = roll {
            out.push(format!(
                "  멤버   {}/{}  {}{}",
                r.done,
                r.total,
                view::bar(r.percent()),
                match r.percent() {
                    None => String::new(),
                    Some(p) => format!("  {p}%"),
                }
            ));
        }
        if !mine.is_empty() {
            out.push(String::new());
            out.extend(view::members(&mine));
        }
    }
    out.extend(view::history(&journal));
    Ok(out)
}
