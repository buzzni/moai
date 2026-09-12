//! 하나를 펼치거나 목록을 낸다.
//!
//! `list` 를 따로 두지 않는다. 읽기 동사가 하나면 `--json`·필터·색 정책을
//! 한 번만 정의한다 — beads 는 `list` 52개 / `show` 9개 플래그로 갈라져
//! 겹치는 개념이 다른 이름을 갖는 값을 치르고 있다.

use super::{Ctx, Fail, R};
use crate::cli::ShowArgs;
use crate::model::{self, Issue, Kind};
use crate::query::{Filter, Raw, Sel};
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
        Some("milestone") => Ok(Target::OfKind(Kind::Milestone)),
        Some("idea") => Ok(Target::OfKind(Kind::Idea)),
        Some(t) if crate::id::is_valid(t) => Ok(Target::One(t.to_string())),
        // 조용히 0건을 내지 않는다. 모르는 값은 거부하고 있는 것을 나열한다.
        Some(t) => Err(Fail::coded(
            format!(
                "`{t}` 는 id 도 종류도 아니다. 종류: issue, epic, milestone, idea\n      \
                 id 로 찾으려면 접두어까지 적는다"
            ),
            super::code::BAD_TARGET,
        )),
    }
}

/// 담당의 `me` 를 지금 사람으로 바꾼다. **`Filter::build` 뒤에 한다.**
///
/// `query` 는 순수 함수라 지금 사람이 누구인지 모르므로 푸는 일은 여기 몫이다.
/// 다만 argv 를 넘기기 *전에* 풀면 두 군데가 어긋난다 — `--filter assignee=me`
/// 는 `build` 안에서야 항이 되므로 손이 닿지 않아 `me` 라는 이름을 찾게 되고,
/// 미리 푼 `이름 (메일)` 은 뒤이어 쉼표로 다시 쪼개져 이름에 쉼표가 든 사람을
/// 영영 못 찾는다. 쪼개진 뒤의 항을 바꾸면 두 문제가 같이 없어진다.
fn resolve_me(sel: &mut [Sel], ctx: &Ctx) -> R<()> {
    for one in sel {
        if let Sel::Is(v) = one
            && v == "me"
        {
            let me = model::actor(ctx.user.as_deref())?;
            *one = Sel::Is(format!("{} ({})", me.name, me.email));
        }
    }
    Ok(())
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
        (!a.assignee.is_empty(), "-a"),
        (a.kind.is_some(), "--type"),
        (a.grep.is_some(), "-g"),
        (a.stale.is_some(), "--stale"),
        (a.all, "--all"),
        (a.deferred, "--deferred"),
        (!a.filter.is_empty(), "--filter"),
        // `--milestone` 도 아래에서 `Filter::build` 로 넘어간다. 여기 빠져
        // 있으면 `moai show <id> --milestone <m>` 이 걸러지지 않은 그 이슈를
        // 그대로 내고, 부르는 쪽은 그 마일스톤에 든 것이라고 믿는다.
        (!a.milestone.is_empty(), "--milestone"),
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
            .ok_or_else(|| Fail::coded(format!("{id} 를 못 찾았다"), super::code::NOT_FOUND))?;
        return one(ctx, &repo, &load.issues, issue, args.raw);
    }

    // **`--raw` 도 조용히 버리지 않는다.** 본문은 하나를 펼칠 때만 나오므로
    // 목록 자리의 `--raw` 는 아무 일도 하지 않는다. 말없이 먹으면 부르는 쪽은
    // 원문을 받았다고 믿는다 — 위의 필터와 같은 까닭이다.
    if args.raw {
        return Err(Fail::coded(
            "`--raw` 는 본문이 나오는 자리에서만 뜻이 있다.\n      \
             `moai show <id> --raw` 처럼 하나를 집어서 쓴다"
                .to_string(),
            "bad_filter",
        ));
    }

    let a = args.filter;
    let kind = match target {
        Target::OfKind(k) => Some(k),
        _ => a.kind,
    };
    // argv 를 그대로 옮겨 담을 뿐이다. 뜻을 정하는 것은 `query` 다.
    let mut filter = Filter::build(Raw {
        status: a.status,
        tag: a.tag,
        no_tag: a.no_tag,
        epic: a.epic,
        milestone: a.milestone,
        parent: a.parent,
        priority: a.priority,
        assignee: a.assignee,
        kind,
        grep: a.grep,
        stale: a.stale,
        all: a.all,
        ideas: false,
        deferred: a.deferred,
        filter: a.filter,
    })
    .map_err(|e| Fail::coded(e, super::code::BAD_FILTER))?;
    resolve_me(&mut filter.assignee, ctx)?;

    // 모르는 칸은 거부한다. 조용히 0건을 내면 `-s in-progress` 같은 오타가
    // "그 칸은 비었다" 와 구별되지 않는다 — `add`·`mv` 는 이미 거부한다.
    for s in &filter.status {
        repo.config.require_known(s).map_err(|e| Fail::coded(e, super::code::BAD_STATUS))?;
    }

    let now = model::now();
    // 한 번만 훑는다. done 을 숨기는 규칙은 `Filter` 하나가 알고, 여기서는
    // 그 규칙을 끈 채(`all`) 걸러 놓고 숨긴 것을 세기만 한다 — 두 번 훑으면
    // 두 판단이 어긋날 자리가 생긴다.
    let hide_done = !filter.all && filter.status.is_empty();
    let hide_ideas = !filter.ideas;
    // 미뤄 둔 것도 done 과 같은 자리에서 빠진다. `--deferred` 로 콕 집어
    // 물었으면 그때는 그것만 보는 것이라 숨길 것이 없다.
    let hide_deferred = filter.deferred.is_none() && !filter.all;
    // **숨긴 것도 센다.** 담아 둔 생각뿐인 저장소에서 `moai show` 가 그냥
    // "없다." 라고 하면, 방금 담은 사람은 파일이 비었다고 믿는다 — done 을
    // 숨길 때 그 수를 말하는 것과 같은 규칙이다.
    let asked_deferred = filter.deferred.is_some();
    let wide = Filter { all: true, ideas: true, ..filter };
    let wh = crate::query::Where::of(&load.issues);
    let mut shown: Vec<Issue> = Vec::new();
    let mut hidden = view::Hidden::default();
    for i in load.issues.iter().filter(|i| wide.matches(i, &now, &wh)) {
        // **꼬리가 대는 낱말이 실제로 그 줄을 내야 한다.** 한때 첫 까닭으로
        // 갈랐는데, 그러면 닫아 둔 생각이 `idea N건 숨김 — --type idea` 로
        // 서고 그 명령은 done 을 여전히 숨겨 아무것도 안 낸다 — `idea_pile`
        // 이 피한 "세어 놓고 못 보여 주는 수" 가 여기 그대로 있었다.
        //
        // 그래서 **한 낱말로 열리는 것만 그 낱말 밑에 센다.**
        //   `--type idea` 는 idea 만 연다 (done·미룸은 그대로 숨긴다)
        //   `--deferred`  는 미룸을 열고 생각까지 같이 연다 (done 은 아니다)
        //   `--all`       은 done 과 미룸을 연다 (생각은 아니다)
        // 어느 하나로도 안 열리는 것(닫아 둔 생각)은 세지 않는다. 못 보여 줄
        // 수를 대느니 말을 안 하는 편이 낫다.
        let by_idea = hide_ideas && report::is_idea(i);
        let by_deferred = hide_deferred && i.is_deferred();
        let by_done = hide_done && i.status.is_done();
        match (by_idea, by_deferred, by_done) {
            (false, false, false) => shown.push(i.clone()),
            (true, false, false) => hidden.ideas += 1,
            (_, true, false) => hidden.deferred += 1,
            (false, _, true) => hidden.done += 1,
            _ => {}
        }
    }
    crate::query::sort_for_display(&mut shown);

    if ctx.json {
        return super::json_line(&shown);
    }
    if args.tree {
        // **자리는 `nav` 가 정한다.** 트리와 탐색기가 자리를 따로 정하면
        // 어긋나고, 실제로 어긋났다 — 제 에픽이 부모와 다른 자식이 두 번
        // 나왔고 끊긴 참조를 가진 줄은 아예 사라졌다.
        let shown_ids: std::collections::BTreeSet<&str> =
            shown.iter().map(|i| i.id.as_str()).collect();
        let index = crate::nav::Index::of(&load.issues);
        let keep = |at: usize| shown_ids.contains(load.issues[at].id.as_str());
        let mut out = view::tree(
            &load.issues,
            &index,
            &keep,
            &report::rollup(&load.issues, &repo.config),
        );
        // **트리도 안 낸 것을 말한다.** 롤업 머리글은 `is_work` 로 세므로
        // 미뤄 둔 멤버까지 세는데, 그 줄은 여기서 빠진다 — 말하지 않으면
        // `0/2` 밑에 줄 하나만 서고 왜 하나가 없는지 아무도 모른다. 목록이
        // 요약 꼬리에 다는 것과 같은 말, 같은 자리(`Hidden`)에서 받는다.
        if let Some(n) = hidden.note() {
            out.push(String::new());
            out.push(n);
        }
        return Ok(out);
    }
    Ok(view::list(
        &shown,
        &repo.config,
        hidden,
        &report::epic_labels(&load.issues),
        asked_deferred,
    ))
}

fn one(ctx: &Ctx, repo: &Repo, all: &[Issue], issue: &Issue, raw: bool) -> R<Vec<String>> {
    let epic = issue.epic.as_ref().and_then(|e| all.iter().find(|i| &i.id == e));
    let children = report::children_of(all, &issue.id);
    let journal = repo.journal_of(&issue.id)?;

    if ctx.json {
        let ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        // **담아 둔 생각은 멤버로 안 낸다.** 소속은 필드로 남지만 화면도
        // (`nav` 가 에픽 밑에 안 걸어서) 머리글도(`rollup` 이 `is_work` 로
        // 세서) 그것을 멤버로 치지 않는다 — 기계 출력만 치면 받는 쪽이 계획에
        // 없는 줄을 계획으로 읽는다. 찾으려면 `moai show --type idea -e <에픽>`.
        let members: Vec<&str> = report::members_of(all, &issue.id)
            .iter()
            .filter(|m| !report::is_idea(m))
            .map(|m| m.id.as_str())
            .collect();
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
    let mut out = view::detail(issue, epic, &children, &[], &repo.config, &model::now(), raw);
    // 묶음을 펼치면 그 밑에 무엇이 있는지까지 보여 준다 — 묶음 하나를 보는
    // 이유가 바로 그것이다. 마일스톤이면 에픽과 이슈가 같이 나온다.
    if report::is_group(issue) {
        let group = match issue.kind {
            Kind::Milestone => report::milestones(all),
            _ => report::groups(all),
        };
        // **베끼지 않는다.** 차례는 `view::members` 가 `nav` 에서 받아 정하므로
        // 여기서 필요한 것은 "누가 이 묶음의 멤버인가" 하나뿐이다. 한때 여기서
        // `sort_for_display` 로 다시 세웠는데, 그 차례는 쓰이는 데가 없었다.
        // **`--json` 이 세는 것과 같은 것을 그린다.** 담아 둔 생각은 멤버가
        // 아니다 — 머리글(`rollup` 은 `is_work` 로 센다)도 기계 출력도 그것을
        // 안 세는데 사람 화면만 그리면, `멤버 0/1` 밑에 줄 둘이 서서 어느
        // 숫자를 믿어야 할지 알 수 없다.
        let mine: std::collections::BTreeSet<&str> = all
            .iter()
            .filter(|i| group.get(i.id.as_str()) == Some(&issue.id.as_str()) && i.id != issue.id)
            .filter(|i| !report::is_idea(i))
            .map(|i| i.id.as_str())
            .collect();
        let roll = report::rollup_of(issue.kind, all, &repo.config)
            .into_iter()
            .find(|r| r.id.as_deref() == Some(issue.id.as_str()));
        if let Some(r) = &roll {
            out.push(format!(
                "  멤버   {}/{}  {}{}",
                r.done,
                r.total,
                view::bar(r.percent),
                match r.percent {
                    None => String::new(),
                    Some(p) => format!("  {p}%"),
                }
            ));
        }
        // **자리를 못 찾으면 아무것도 내지 않는다.** 뿌리로 되돌리면 그 에픽의
        // 멤버라며 저장소 전부를 낸다 — 없는 답보다 틀린 답이 비싸다.
        if !mine.is_empty() {
            let index = crate::nav::Index::of(all);
            let here = index.find(&issue.id).map(|at| {
                let mut p = index.home_of(at).clone();
                p.push(index.seg_of(all, at));
                p
            });
            if let Some(here) = here {
                out.push(String::new());
                let keep = |at: usize| mine.contains(all[at].id.as_str());
                // **에픽 집계를 건넨다.** 이 밑에 머리글을 갖는 것은 에픽뿐이고
                // (마일스톤 밑의 에픽, 에픽 밑에는 없다), 빈 것을 건네면 그 줄이
                // 집계를 잃어 `에픽 1건` 처럼 나온다 — 같은 에픽이 `moai show
                // --tree` 와 다르게 읽힌다.
                let rolls = report::rollup(all, &repo.config);
                out.extend(view::members(all, &index, &keep, &rolls, &here));
            }
        }
    }
    out.extend(view::history(&journal, &repo.config));
    Ok(out)
}
