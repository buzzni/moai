//! 하나를 펼치거나 목록을 낸다.
//!
//! `list` 를 따로 두지 않는다. 읽기 동사가 하나면 `--json`·필터·색 정책을
//! 한 번만 정의한다 — beads 는 `list` 52개 / `show` 9개 플래그로 갈라져
//! 겹치는 개념이 다른 이름을 갖는 값을 치르고 있다.

use super::{Ctx, Fail, R};
use crate::cli::ShowArgs;
use crate::model::{self, Issue, Kind};
use crate::query::{Filter, Hide, Raw, Sel};
use std::collections::BTreeSet;
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
    let crate::worktree::Gathered { load, origin, .. } = super::gather(&repo, args.worktree.worktree)?;
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
        if args.as_plan {
            return plan(ctx, &load.issues, issue, args.raw);
        }
        return one(ctx, &repo, &load.issues, issue, args.raw, &origin, args.worktree.worktree);
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
    // 되뽑을 에픽이 없다. 목록을 통째로 되뽑으면 에픽 없는 이슈가 `add --from` 에
    // 도로 안 들어가는 글이 된다.
    if args.as_plan {
        return Err(Fail::coded(
            "`--as-plan` 은 에픽 하나를 되뽑는다.\n      \
             `moai show <에픽> --as-plan` 처럼 하나를 집어서 쓴다"
                .to_string(),
            super::code::BAD_TARGET,
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
        grep_in: crate::query::GrepIn::All,
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
    // 한 번만 훑는다. 숨기는 규칙은 `Filter::hidden_by` 하나가 알고, 여기서는
    // 숨김을 끈 채(`all`·`ideas`) 걸러 놓고 까닭을 받아 세기만 한다 — 규칙을
    // 여기 다시 적으면 두 판단이 어긋나고, 실제로 어긋났다(moai-nnul).
    //
    // **숨긴 것도 센다.** 담아 둔 생각뿐인 저장소에서 `moai show` 가 그냥
    // "없다." 라고 하면, 방금 담은 사람은 파일이 비었다고 믿는다.
    let asked_deferred = filter.deferred.is_some();
    let wide = Filter { all: true, ideas: true, ..filter.clone() };
    let wh = crate::query::Where::of(&load.issues, &repo.config);
    let mut shown: Vec<Issue> = Vec::new();
    // 숨긴 줄과 까닭. **세는 것은 그린 뒤다** — 트리는 걸리지 않은 줄도 걸린
    // 자손의 조상이면 그리므로, 먼저 세면 방금 그린 줄을 숨겼다고 말한다.
    let mut hidden_rows: Vec<(usize, Hide)> = Vec::new();
    for (at, i) in load.issues.iter().enumerate().filter(|(_, i)| wide.matches(i, &now, &wh)) {
        match filter.hidden_by(i, &wh) {
            None => shown.push(i.clone()),
            Some(why) => hidden_rows.push((at, why)),
        }
    }
    let tally = |drawn: &BTreeSet<usize>| {
        let mut h = view::Hidden::default();
        for (at, why) in &hidden_rows {
            if !drawn.contains(at) {
                h.add(*why);
            }
        }
        h
    };
    crate::query::sort_for_display(&mut shown);

    if ctx.json {
        let rows: Vec<super::Row> =
            shown.iter().map(|i| super::Row::of(i, wh.states.get(i.id.as_str()).copied()).on(&origin)).collect();
        return super::json_line(&rows);
    }
    if args.tree {
        // **자리는 `nav` 가 정한다.** 트리와 탐색기가 자리를 따로 정하면
        // 어긋나고, 실제로 어긋났다 — 제 에픽이 부모와 다른 자식이 두 번
        // 나왔고 끊긴 참조를 가진 줄은 아예 사라졌다.
        let shown_ids: std::collections::BTreeSet<&str> =
            shown.iter().map(|i| i.id.as_str()).collect();
        let index = crate::nav::Index::of(&load.issues);
        let keep = |at: usize| shown_ids.contains(load.issues[at].id.as_str());
        let (mut out, drawn) = view::tree(
            &load.issues,
            &index,
            &keep,
            &report::rollup(&load.issues, &repo.config),
            &origin,
        );
        // **트리도 안 낸 것을 말한다.** 롤업 머리글은 `is_work` 로 세므로
        // 미뤄 둔 멤버까지 세는데, 그 줄은 여기서 빠진다 — 말하지 않으면
        // `0/2` 밑에 줄 하나만 서고 왜 하나가 없는지 아무도 모른다. 목록이
        // 요약 꼬리에 다는 것과 같은 말, 같은 자리(`Hidden`)에서 받는다.
        // **그린 줄은 안 센다** — 걸린 자손 때문에 선 조상이다(moai-wi67).
        if let Some(n) = tally(&drawn).note() {
            out.push(String::new());
            out.push(n);
        }
        return Ok(out);
    }
    Ok(view::list(
        &shown,
        &repo.config,
        tally(&BTreeSet::new()),
        &report::epic_labels(&load.issues),
        asked_deferred,
        &wh,
        &origin,
    ))
}

/// `--as-plan` — 에픽 하나를 `add --from` 이 받는 마크다운으로 되뽑는다.
///
/// 형식은 `draft::render` 가 안다. 여기는 멤버를 사람 화면과 같은 자
/// (`report::group_members`)로 고를 뿐이다 — 따로 고르면 `show <에픽>` 에 선 줄과
/// 되뽑은 줄이 어긋난다. 미뤄 둔 멤버도 든다: 틀을 다듬는 것은 사람이다.
///
/// 차례는 목록 차례다. 스냅샷은 id 차례라 만든 차례가 남아 있지 않다.
fn plan(ctx: &Ctx, all: &[Issue], epic: &Issue, raw: bool) -> R<Vec<String>> {
    if raw {
        return Err(Fail::coded(
            "`--as-plan` 과 `--raw` 는 같이 쓸 수 없다 — 되뽑은 계획에는 본문이 없다".to_string(),
            super::code::BAD_FILTER,
        ));
    }
    if epic.kind != Kind::Epic {
        return Err(Fail::coded(
            format!(
                "`{}` 는 {} 다. `--as-plan` 은 에픽을 되뽑는다\n      에픽 목록은 `moai show epic`",
                epic.id,
                epic.kind.as_str()
            ),
            super::code::BAD_TARGET,
        ));
    }
    let members = report::group_members(all, epic);
    let md = crate::draft::render(epic, &members);
    // 도로 못 들어가는 줄은 이름을 댄다. 종료 코드는 안 바꾼다 — 틀은 사람이 다듬는다.
    let lossy = crate::draft::lossy(epic, &members);
    if ctx.json {
        return super::json_line(&serde_json::json!({ "id": epic.id, "plan": md, "lossy": lossy }));
    }
    for id in &lossy {
        // 앞머리 `[`·끝의 `#낱말` 은 render 가 이스케이프한다(moai-a5pz). 여기 오는 것은 원래
        // 역슬래시를 든 제목처럼 이스케이프로도 못 담는 것뿐이라 까닭을 하나로 단정하지 않는다.
        eprintln!("moai: {id} 의 제목은 이 형식으로 도로 넣으면 달리 읽힌다 — 넣기 전에 고친다");
    }
    Ok(md.lines().map(str::to_string).collect())
}

fn one(
    ctx: &Ctx,
    repo: &Repo,
    all: &[Issue],
    issue: &Issue,
    raw: bool,
    origin: &crate::worktree::Origin,
    worktree: bool,
) -> R<Vec<String>> {
    // **에픽도 뒷줄로 푼다** — 펼친 줄을 고른 자(`Load::get`)와 같다(moai-e0ro).
    let epic = issue.epic.as_ref().and_then(|e| all.iter().rfind(|i| &i.id == e));
    let twins = report::duplicate_lines(all, &issue.id);
    let children = report::children_of(all, &issue.id);
    // **이력은 줄이 온 워크트리의 저널에서 읽는다** (`Origin::root`). 스냅샷은
    // 옆 워크트리의 줄을 내는데 이력만 이쪽에서 읽으면, 거기서 옮긴 칸이 이력에
    // 없어 상세의 머리글과 이력이 서로 다른 말을 한다.
    let journal = match origin.root(&issue.id) {
        None => repo.journal_of(&issue.id)?,
        Some(root) => Repo { root: root.to_path_buf(), config: repo.config.clone() }.journal_of(&issue.id)?,
    };
    // 이 줄과 자식을 계획에서 뺀 줄. **물려받은 미룸까지** — 미룬 에픽의 멤버를
    // 펼쳤을 때 표가 없으면 상세가 답하기로 한 "왜 ready 에 안 나오나" 가 빈다.
    // 묶음의 읽은 칸은 **이 줄과 자식에 대해서만** 센다 — 일 하나를 펼치는 흔한 길에서
    // 저장소 전부의 소속과 미룸을 걷는 것은 통째로 헛일이다(`group_states_of`).
    let near: Vec<&str> =
        std::iter::once(issue.id.as_str()).chain(children.iter().map(|c| c.id.as_str())).collect();
    // **집은 줄만 워크트리를 읽는다**(moai-6opu) — 안 집은 줄을 펼치는 흔한 길에서 옆 스냅샷을 다
    // 풀 까닭이 없다. 경로는 저장소 뿌리에서 잰다: 규약의 자리(`.claude/worktrees/<id>`)가 그대로 읽힌다.
    //
    // **딸린 워크트리에서는 겹쳐 볼 때만 잰다** — `status` 의 `stranded` 와 같은 까닭(moai-4370).
    // 그 스냅샷은 갈라질 때의 main 이라 그 뒤 main 에서 놓은 줄이 거기서는 아직 집혀 있고, 그것으로
    // 재면 멀쩡히 끝난 일에 "자리 없다" 를 붙여 감독이 그 말대로 남에게 다시 준다.
    let look = worktree || !crate::worktree::is_linked(&repo.root);
    let trees: Vec<report::Workplace> = if look && report::wip(all, &repo.config).iter().any(|i| i.id == issue.id) {
        // 뿌리도 같은 자로 푼다 — 워크트리 경로는 이미 푼 것이라(`worktree::canonical`) 심볼릭
        // 링크를 낀 뿌리로는 하나도 안 잘린다.
        let root = std::fs::canonicalize(&repo.root).unwrap_or_else(|_| repo.root.clone());
        crate::worktree::workplaces(&repo.root, &repo.config)
            .into_iter()
            .map(|mut t| {
                if let Ok(rel) = t.path.strip_prefix(&root) {
                    // 제 워크트리 안에서 펼치면 뿌리와 같은 자리다 — 빈 경로로 두면 사람 화면의
                    // `자리` 칸이 통째로 비고 `--json` 의 `path` 가 `""` 로 나간다.
                    t.path = if rel.as_os_str().is_empty() { std::path::PathBuf::from(".") } else { rel.to_path_buf() };
                }
                t
            })
            .collect()
    } else {
        Vec::new()
    };
    let places = (!trees.is_empty())
        .then(|| report::places(all, &repo.config, &trees).remove(&issue.id))
        .flatten();
    let seen = view::Seen {
        roots: report::deferred_roots(all),
        states: report::group_states_of(all, &repo.config, &near),
        origin: Some(origin),
        blocks: report::blocks_of(all, &repo.config, issue),
        places,
    };
    // **커밋은 저장하지 않고 git 에서 읽는다**(moai-1w2l) — 커밋 제목에 이 id 를 적은 것.
    // git 이 없거나 저장소 밖이면 칸을 비운다. 커밋 칸 하나 때문에 상세가 안 열리는 것이
    // 빈 칸보다 비싸다(그 모서리의 말투는 moai-mauw).
    // **커밋도 줄이 온 워크트리의 `HEAD` 에서 읽는다** — 이력과 같은 까닭. `--worktree` 로
    // 옆에서 집은 일을 펼치면 그 일을 고친 커밋은 저쪽 가지에만 있다.
    let commits = crate::git::commits_of(origin.root(&issue.id).unwrap_or(&repo.root), &[issue.id.as_str()])
        .ok()
        .and_then(|mut by_id| by_id.remove(&issue.id))
        .unwrap_or_default();

    if ctx.json {
        let ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        // **사람 화면과 같은 자로 고른다** (`report::group_members`). 담아 둔
        // 생각은 거기서 빠진다 — 찾으려면 `moai show --type idea -e <에픽>`. 한때
        // 여기만 에픽에 한해 제 `epic` 을 적은 줄을 내, 마일스톤은 키가 없고
        // 물려받은 자식은 화면에만 있었다(moai-qizs).
        let members: Vec<&str> =
            report::group_members(all, issue).iter().map(|m| m.id.as_str()).collect();
        let mut extra = vec![
            ("children", serde_json::to_string(&ids).map_err(|e| Fail::new(e.to_string()))?),
            ("journal", serde_json::to_string(&journal).map_err(|e| Fail::new(e.to_string()))?),
        ];
        if report::is_group(issue) {
            extra.push((
                "members",
                serde_json::to_string(&members).map_err(|e| Fail::new(e.to_string()))?,
            ));
        }
        // **기계 출력도 같은 것을 말한다.** `deferred_at` 은 제 줄에 적힌 것뿐이라,
        // 미룬 에픽의 멤버를 `--json` 으로 펼친 쪽은 그것이 계획 밖인 줄 모른다.
        if let Some(root) = seen.roots.get(issue.id.as_str()) {
            extra.push(("shelved_by", serde_json::to_string(root).map_err(|e| Fail::new(e.to_string()))?));
        }
        if let Some(n) = twins {
            extra.push(("duplicate_lines", n.to_string()));
        }
        // 사람 화면이 그리는 막음을 **같은 답으로** 낸다. `blocked_by` 는 적힌 id 뿐이라 막는지·
        // 풀렸는지·끊겼는지를 받는 쪽이 다시 가르게 된다. 막음이 없으면 키를 안 단다.
        // **키는 `blockers` 다** — `link A --blocks B` 가 "A 가 B 를 막는다" 이므로 B 에 단
        // `blocks` 는 받는 쪽이 "B 가 A 를 막는다" 로 거꾸로 읽는다.
        if !seen.blocks.is_empty() {
            extra.push(("blockers",serde_json::to_string(&seen.blocks).map_err(|e| Fail::new(e.to_string()))?));
        }
        // 트래커 커밋까지 **전부** 낸다 — `tracker` 표시가 붙으니 거를지는 받는 쪽이 정한다.
        // 사람 화면만 뺀다(`view::commits`). 커밋이 없으면 키를 안 단다.
        // 사람 화면의 `자리` 줄과 같은 답. 줄을 안 세우는 자리에서는 키도 안 단다.
        if let Some(p) = &seen.places {
            extra.push(("workplaces", serde_json::to_string(p).map_err(|e| Fail::new(e.to_string()))?));
        }
        if !commits.is_empty() {
            extra.push(("commits", serde_json::to_string(&commits).map_err(|e| Fail::new(e.to_string()))?));
        }
        return super::json_with(
            &super::Row::of(issue, seen.states.get(issue.id.as_str()).copied()).on(origin),
            &extra,
        );
    }

    // 이력은 언제나 맨 끝이다. 에픽이면 멤버를 그 **앞에** 끼운다.
    let mut out = view::detail(issue, epic, &children, &seen, &repo.config, &model::now(), raw);
    // 머리 두 줄(제목·칸) 바로 밑이다 — 본문을 읽기 전에 이 줄이 하나뿐이 아님을 안다.
    if let Some(n) = twins {
        out.insert(2.min(out.len()), view::duplicate_note(n));
    }
    // 묶음을 펼치면 그 밑에 무엇이 있는지까지 보여 준다 — 묶음 하나를 보는
    // 이유가 바로 그것이다. 마일스톤이면 에픽과 이슈가 같이 나온다.
    if report::is_group(issue) {
        // **베끼지 않는다.** 차례는 `view::members` 가 `nav` 에서 받아 정하므로
        // 여기서 필요한 것은 "누가 이 묶음의 멤버인가" 하나뿐이다.
        // **`--json` 이 내는 것과 같은 것을 그린다** — 둘 다
        // `report::group_members` 로 고른다.
        let mine: BTreeSet<&str> =
            report::group_members(all, issue).iter().map(|i| i.id.as_str()).collect();
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
                out.extend(view::members(all, &index, &keep, &rolls, &here, origin));
            }
        }
    }
    out.extend(view::commits(&commits));
    out.extend(view::history(&journal, &repo.config));
    Ok(out)
}
