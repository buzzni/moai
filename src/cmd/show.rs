//! 하나를 펼치거나 목록을 낸다.
//!
//! `list` 를 따로 두지 않는다. 읽기 동사가 하나면 `--json`·필터·색 정책을
//! 한 번만 정의한다 — beads 는 `list` 52개 / `show` 9개 플래그로 갈라져
//! 겹치는 개념이 다른 이름을 갖는 값을 치르고 있다.

use super::{Ctx, Fail, R};
use crate::cli::ShowArgs;
use crate::model::{self, Issue, Kind};
use crate::query::{Filter, Hide, Raw, Sel};
use crate::report;
use crate::store::Repo;
use crate::view;
use std::collections::BTreeSet;

/// 대상이 무엇으로 읽히는가. id 는 `-` 를 품고 종류 낱말은 품지 않아
/// 어휘가 겹치지 않는다.
enum Target {
    One(String),
    OfKind(Kind),
    All,
}

fn resolve(target: Option<&str>, lang: crate::i18n::Lang) -> R<Target> {
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
                "{}\n      {}",
                crate::i18n::fill(crate::i18n::say(lang, "refuse.show_target"), &[("target", t)]),
                crate::i18n::say(lang, "refuse.show_target_how"),
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
fn resolve_me(sel: &mut [Sel], ctx: &Ctx, root: &std::path::Path) -> R<()> {
    for one in sel {
        if let Sel::Is(v) = one
            && v == "me"
        {
            let me = model::actor(ctx.user.as_deref(), root).map_err(|e| Fail::no_actor(&e, ctx.lang()))?;
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
    let repo = super::open_repo(ctx)?;
    let crate::worktree::Gathered { load, origin, sides, mine, .. } =
        super::gather(ctx, &repo, args.worktree.worktree)?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);

    let target = match kind_filter {
        // `moai epic show <id>` 는 그 id 를 그대로 본다. 종류는 목록일 때만 거른다.
        Some(k) if args.target.is_none() => Target::OfKind(k),
        _ => resolve(args.target.as_deref(), ctx.lang())?,
    };

    if let Target::One(id) = &target {
        // 필터를 조용히 버리지 않는다. 하나를 콕 집었으면 거를 것이 없고,
        // 버린 채로 그 하나를 내면 부르는 쪽은 걸러진 결과라고 믿는다.
        if let Some(flag) = args.tree.then_some("--tree").or_else(|| first_given(&args.filter)) {
            return Err(Fail::coded(
                format!(
                    "{}\n      {}",
                    crate::i18n::fill(
                        crate::i18n::say(ctx.lang(), "refuse.show_filter_on_one"),
                        &[("id", id), ("flag", flag)]
                    ),
                    crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.show_filter_how"), &[("flag", flag)]),
                ),
                "bad_filter",
            ));
        }
        // 없는 id 는 **어느 명령에서나 한 낱말이다**([`Fail::not_found`], moai-95g1) — 손으로
        // 같은 글을 지어 두면 `show` 만 옛 말로 남는다(리뷰).
        let issue = load.get(id).ok_or_else(|| Fail::not_found(id, ctx.lang()))?;
        if args.as_plan {
            return plan(ctx, &load.issues, issue, args.raw);
        }
        // 겹치며 이미 판 옆 스냅샷을 그대로 넘긴다 — 자리를 물을 때 같은 파일을 다시 안 판다(moai-kos1).
        // **제 스냅샷도 같이 넘긴다**(moai-mafv) — 바로 위에서 판 그 파일이다.
        let dug = crate::worktree::dug(&sides, &mine);
        return one(ctx, &repo, &load.issues, issue, args.raw, &origin, args.worktree.worktree, &dug);
    }

    // **`--raw` 도 조용히 버리지 않는다.** 본문은 하나를 펼칠 때만 나오므로
    // 목록 자리의 `--raw` 는 아무 일도 하지 않는다. 말없이 먹으면 부르는 쪽은
    // 원문을 받았다고 믿는다 — 위의 필터와 같은 까닭이다.
    if args.raw {
        return Err(Fail::coded(
            format!(
                "{}\n      {}",
                crate::i18n::say(ctx.lang(), "refuse.show_raw_on_list"),
                crate::i18n::say(ctx.lang(), "refuse.show_raw_how"),
            ),
            "bad_filter",
        ));
    }
    // 되뽑을 에픽이 없다. 목록을 통째로 되뽑으면 에픽 없는 이슈가 `add --from` 에
    // 도로 안 들어가는 글이 된다.
    if args.as_plan {
        return Err(Fail::coded(
            format!(
                "{}\n      {}",
                crate::i18n::say(ctx.lang(), "refuse.as_plan_on_list"),
                crate::i18n::say(ctx.lang(), "refuse.as_plan_how"),
            ),
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
    resolve_me(&mut filter.assignee, ctx, &repo.root)?;

    // 모르는 칸은 거부한다. 조용히 0건을 내면 `-s in-progress` 같은 오타가
    // "그 칸은 비었다" 와 구별되지 않는다 — `add`·`mv` 는 이미 거부한다.
    //
    // **어느 줄이 선 칸이면 받는다** — `--from` 과 같은 술어다(moai-hym7, 사람이 정했다).
    // `config` 에서 칸 이름을 고친 뒤 옛 이름에 선 줄은 옮길 수는 있는데 못 찾으면,
    // 읽기가 쓰기보다 엄해져 "읽기는 관대하고 쓰기는 엄하다" 가 뒤집힌다.
    for s in &filter.status {
        if !crate::report::knows_column(&load.issues, &repo.config, s) {
            let why = super::unknown_column(s, &repo.config);
            return Err(Fail::coded(crate::view::no_such_column(ctx.lang(), &why), super::code::BAD_STATUS));
        }
    }

    let now = model::now();
    // 한 번만 훑는다. 숨기는 규칙은 `Filter::hidden_by` 하나가 알고, 여기서는
    // 숨김을 끈 채(`all`·`ideas`) 걸러 놓고 까닭을 받아 세기만 한다 — 규칙을
    // 여기 다시 적으면 두 판단이 어긋나고, 실제로 어긋났다(moai-nnul).
    //
    // **숨긴 것도 센다.** 담아 둔 생각뿐인 저장소에서 `moai show` 가 그냥
    // "없다." 라고 하면, 방금 담은 사람은 파일이 비었다고 믿는다.
    // **꼬리의 미룸 낱말은 물은 것을 따른다**(moai-pkvw) — 미룬 것만 물었으면 줄마다 같은 낱말이
    // 붙어 봐야 자리만 먹는다. 결과의 내용으로 정하지 않는 까닭은 [`view::Asked`] 에 있다.
    let asked = view::Asked { deferred: filter.deferred.is_some() };
    let wide = Filter { all: true, ideas: true, ..filter.clone() };
    // **소속 지도는 한 벌이다**(moai-g0zx) — 거름망과 트리의 색인·에픽 굴림이 저마다 지으면
    // `groups` 가 한 명령에 세 벌 돈다. 지도를 빌려 쓰는 둘을 먼저 짓고, 그것을 제 필드로 들고
    // 사는 거름망(`Where::from_soil`)이 마지막에 지도를 받아 간다.
    let soil = crate::report::Soil::of(&load.issues);
    let tree_now = args.tree && !ctx.json;
    let index = tree_now.then(|| crate::nav::Index::in_soil(&load.issues, &soil));
    let rolls = tree_now.then(|| report::rollup_in(&load.issues, &repo.config, &soil));
    let wh = crate::query::Where::from_soil(&load.issues, &repo.config, soil);
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
        // **일한 AI 는 목록에서도 나온다**(moai-p8qj). 닫힌 500건의 토큰을 더하려고
        // `show <id> --json` 을 500번 부르면 저널 전체를 500번 읽는다 — 여기서는 뿌리마다
        // 한 번 읽어 id 로 가른다.
        let work = work_by_id(&repo, &origin, &shown);
        // **못 읽은 저널은 저널을 읽은 **뒤**에 묻는다** — `work_by_id` 가 그 읽기다. 뿌리마다
        // 한 번 접어 두고 줄마다 그 줄의 뿌리 것을 빌린다(`home`): 줄 수만큼 글을 다시 짓지 않고,
        // 겹쳐 온 줄은 제 워크트리의 실패만 달고 선다.
        let unread = crate::store::journal_unread();
        let errors: std::collections::BTreeMap<&std::path::Path, Vec<super::JournalError>> = shown
            .iter()
            .map(|i| home(&repo, &origin, &i.id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|root| (root, super::journal_errors(ctx.lang(), &unread, Some(root))))
            .collect();
        let rows: Vec<Listed> = shown
            .iter()
            .map(|i| Listed {
                row: super::Row::of(i, wh.states.get(i.id.as_str()).copied()).on(&origin),
                journal_error: errors.get(home(&repo, &origin, &i.id)).map_or(&[], Vec::as_slice),
                // **키는 늘 선다**(moai-2l8n) — 하나를 펼칠 때와 같은 약속이다. 빈 배열은
                // "이 일을 한 AI 를 아무도 안 적었다" 는 사실이고, 키가 없으면 되쓴 줄의
                // 옛 `work` 가 그 자리에서 거짓을 싣는다.
                work: work.get(i.id.as_str()).map_or(&[], Vec::as_slice),
            })
            .collect();
        return super::json_line(&rows);
    }
    // **문은 지도를 지은 그 자다**(`tree_now`) — `args.tree` 로 다시 적으면 아래의 `expect` 가
    // 멀리 떨어진 `if ctx.json` 의 되돌아감에 기대게 되고, 그 차례를 건드리는 날 CLI 가 터진다.
    // **화면은 한 번 짓는다** — 트리와 목록은 갈라져 서지만 같은 맥락으로 그리므로, 두 자리에서
    // 따로 지으면 한쪽만 고치는 날 같은 명령의 두 표면이 다른 말이나 다른 출처로 선다.
    let screen = view::Screen::new(ctx.lang()).at(ctx.zone()).over(&origin);
    if tree_now {
        // **자리는 `nav` 가 정한다.** 트리와 탐색기가 자리를 따로 정하면
        // 어긋나고, 실제로 어긋났다 — 제 에픽이 부모와 다른 자식이 두 번
        // 나왔고 끊긴 참조를 가진 줄은 아예 사라졌다.
        let shown_ids: std::collections::BTreeSet<&str> = shown.iter().map(|i| i.id.as_str()).collect();
        // 위에서 지도 한 벌로 지은 것이다 — `tree_now` 가 참일 때만 서 있다.
        let (index, rolls) = (index.expect("트리 색인"), rolls.expect("에픽 굴림"));
        let keep = |at: usize| shown_ids.contains(load.issues[at].id.as_str());
        let (mut out, drawn) = view::tree(&load.issues, &index, &keep, &rolls, screen);
        // **트리도 안 낸 것을 말한다.** 롤업 머리글은 `is_work` 로 세므로
        // 미뤄 둔 멤버까지 세는데, 그 줄은 여기서 빠진다 — 말하지 않으면
        // `0/2` 밑에 줄 하나만 서고 왜 하나가 없는지 아무도 모른다. 목록이
        // 요약 꼬리에 다는 것과 같은 말, 같은 자리(`Hidden`)에서 받는다.
        // **그린 줄은 안 센다** — 걸린 자손 때문에 선 조상이다(moai-wi67).
        if let Some(n) = tally(&drawn).note(ctx.lang()) {
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
        asked,
        &wh,
        screen,
    ))
}

/// 목록의 줄 하나 — 줄에 `work` 를 곁들인다(moai-p8qj).
///
/// [`super::Row`] 가 이미 [`super::OURS`] 를 걷었으므로(되써 넣은 줄의 옛 `work`) 한 객체에
/// 같은 키가 둘 서지 않는다. 하나를 펼치는 쪽은 [`super::json_with`] 로 같은 키를 붙인다 —
/// 거기는 `journal`·`commits` 까지 붙이는 자리라 모양이 다를 뿐, 이름과 뜻은 하나다.
///
/// **곁들이는 키는 [`super::OURS`] 에 있어야 한다** — 필드를 더하면 `OURS` 에도 더한다. 시험
/// (`every_key_listed_adds_is_in_ours`)이 붉어져 잡는다.
#[derive(serde::Serialize)]
struct Listed<'a> {
    #[serde(flatten)]
    row: super::Row<'a>,
    work: &'a [model::Work],
    /// 이 줄의 이력이 **덜 왔다**(moai-f2lc). 그 줄의 뿌리에서 못 읽은 저널이 있을 때만 선다 —
    /// 없는 것이 곧 "이 줄의 이력은 다 왔다" 라, `commits_error` 와 같은 약속이다.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    journal_error: &'a [super::JournalError],
}

/// 줄이 온 워크트리의 moai 뿌리 — **이력(저널)을 거기서 읽는다**(`Origin::root`). 스냅샷은
/// 옆에서 온 줄을 내는데 이력만 이쪽에서 읽으면 저쪽에서 옮긴 칸·적은 `model:` 줄이 빈다.
/// 커밋은 [`commit_home`] 이 따로 고른다 — 저널은 트래커 곁에 살고 `HEAD` 는 체크아웃의 것이다.
///
/// 하나를 펼칠 때([`one`])와 목록([`work_by_id`])이 **이 한 자로** 고른다 — 따로 적던 때는 한쪽만
/// 이쪽 뿌리로 돌려도 아무 시험도 안 붉어졌다(리뷰 moai-u5bk.3wq).
fn home<'a>(repo: &'a Repo, origin: &'a crate::worktree::Origin, id: &str) -> &'a std::path::Path {
    origin.root(id).unwrap_or(&repo.root)
}

/// 그 줄의 **커밋**을 물을 체크아웃. 저널과 갈린다(moai-y7go, 리뷰 moai-71ht.jlh) — 겹쳐 온 줄은
/// 저쪽 워크트리의 것이지만, 이 트래커의 줄은 저널이 루트에 있어도 `git log` 가 물을 `HEAD` 는
/// **이 세션이 선 체크아웃**의 것이다. `repo.root` 로 묻던 판은 워크트리 안에서 방금 한 커밋이
/// 루트의 `HEAD` 에 없어 `commits` 를 빈 배열로 냈다 — `commits_error` 없이 비는 것은 AGENTS.md
/// 가 "그 id 를 적은 커밋이 없다" 로 못박은 값이라, 있는 일을 없다고 말한 셈이다.
///
/// **반대쪽 눈가림은 [`crate::git::table`] 이 막는다**(리뷰 moai-71ht 셋째 판) — 그쪽이 `HEAD` 와
/// 주 체크아웃의 `HEAD` 를 함께 걸어, 갈라진 뒤 본 가지에 합쳐진 커밋도 이 자리에서 보인다. 여기서
/// 고르는 것은 **어느 체크아웃에게 묻는가** 하나다.
fn commit_home<'a>(repo: &'a Repo, origin: &'a crate::worktree::Origin, id: &str) -> &'a std::path::Path {
    origin.root(id).unwrap_or_else(|| repo.here())
}

/// 그 뿌리의 저널을 읽을 저장소. 설정은 이쪽 것을 빌린다 — 저널을 읽는 데는 안 쓴다.
fn at_home(repo: &Repo, root: &std::path::Path) -> Repo {
    Repo::at(root.to_path_buf(), repo.config.clone())
}

/// 낼 줄들의 `work` — **저널을 뿌리마다 한 번** 읽고, 그 가운데 `model:` 줄을 들 수 있는 줄만
/// 푼다(`model::may_hold_work`). 답은 하나를 펼칠 때(`model::work_of(&journal)`)와 같다 — 거른
/// 줄은 `work` 를 못 내는 줄뿐이고, 남은 줄의 차례는 그대로다.
fn work_by_id(
    repo: &Repo,
    origin: &crate::worktree::Origin,
    shown: &[Issue],
) -> std::collections::BTreeMap<String, Vec<model::Work>> {
    let mut by_root: std::collections::BTreeMap<&std::path::Path, BTreeSet<&str>> = Default::default();
    for i in shown {
        by_root.entry(home(repo, origin, &i.id)).or_default().insert(i.id.as_str());
    }
    let mut out = std::collections::BTreeMap::new();
    for (root, ids) in by_root {
        for (id, journal) in at_home(repo, root).journal_by_id(&ids, model::may_hold_work) {
            out.insert(id, model::work_of(&journal));
        }
    }
    out
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
        return Err(Fail::coded(crate::i18n::say(ctx.lang(), "refuse.as_plan_with_raw"), super::code::BAD_FILTER));
    }
    if epic.kind != Kind::Epic {
        return Err(Fail::coded(
            format!(
                "{}\n      {}",
                crate::i18n::fill(
                    crate::i18n::say(ctx.lang(), "refuse.as_plan_not_an_epic"),
                    &[("id", &epic.id), ("is", epic.kind.as_str())]
                ),
                crate::i18n::say(ctx.lang(), "refuse.as_plan_epics"),
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
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "show.lossy_title"), &[("id", id)]));
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
    dug: &crate::worktree::Dug<'_>,
) -> R<Vec<String>> {
    // **에픽도 뒷줄로 푼다** — 펼친 줄을 고른 자(`Load::get`)와 같다(moai-e0ro).
    let epic = issue.epic.as_ref().and_then(|e| all.iter().rfind(|i| &i.id == e));
    let twins = report::duplicate_lines(all, &issue.id);
    let children = report::children_of(all, &issue.id);
    // **이력은 줄이 온 워크트리의 저널에서 읽는다** (`Origin::root`). 스냅샷은
    // 옆 워크트리의 줄을 내는데 이력만 이쪽에서 읽으면, 거기서 옮긴 칸이 이력에
    // 없어 상세의 머리글과 이력이 서로 다른 말을 한다.
    let journal_root = home(repo, origin, &issue.id);
    let journal = at_home(repo, journal_root).journal_of(&issue.id);
    // 이 줄과 자식을 계획에서 뺀 줄. **물려받은 미룸까지** — 미룬 에픽의 멤버를
    // 펼쳤을 때 표가 없으면 상세가 답하기로 한 "왜 ready 에 안 나오나" 가 빈다.
    // 묶음의 읽은 칸은 **이 줄과 자식에 대해서만** 센다 — 일 하나를 펼치는 흔한 길에서
    // 저장소 전부의 소속과 미룸을 걷는 것은 통째로 헛일이다(`group_states_of`).
    let near: Vec<&str> = std::iter::once(issue.id.as_str()).chain(children.iter().map(|c| c.id.as_str())).collect();
    // **집은 줄만 워크트리를 읽는다**(moai-6opu) — 안 집은 줄을 펼치는 흔한 길에서 옆 스냅샷을 다
    // 풀 까닭이 없다. 언제 재는지(딸린 워크트리에서는 겹쳐 볼 때만)는 `worktree::workplaces` 가
    // 한 곳에서 정한다 — 명령마다 두었더니 `status` 와 여기가 서로 다른 답을 냈다(moai-6opu.p65).
    //
    // 경로는 **main 워크트리의 꼭대기**에서 잰 것이다(moai-fygk, `worktree::workplaces` 가 잰다):
    // 규약의 자리(`.claude/worktrees/<id>`)가 어느 자리에서 펼치든 같은 글자로 나와, 그대로
    // `EnterWorktree` 에 옮길 수 있다. `status` 의 못 읽은 워크트리와 같은 자다 — 부르는 쪽마다
    // 따로 재던 때는 빈 경로를 다루는 법이 갈렸다.
    // **자리 판정의 재료는 한 벌이다**([`report::Footing`], moai-rviv) — 문(`placeable`), 스냅샷을
    // 팔지 고르는 문(`workplaces`), 그리고 판정(`places`)이 저마다 집은 줄을 고르고 소속 지도를
    // 지었다. 게을러서, 아래 문이 닫히면 한 벌도 안 짓는다.
    let footing = report::Footing::of(all, &repo.config);
    let trees: Vec<report::Workplace> = if report::placeable_in(&footing, issue) {
        // **자리는 세션이 선 체크아웃에서 잰다**(리뷰 moai-71ht.jlh 사용자 결정) — 트래커는 루트로
        // 옮겨 가지만(`Repo::find_from`) "여기가 어디냐" 는 여전히 이 체크아웃이다. 루트로 재던 판은
        // 워크트리 안에서도 자리를 파고 제 워크트리를 옆으로 세어, 겹쳐 보지 않을 때는 안 판다는
        // 결정(moai-6opu)이 조용히 꺼졌다.
        crate::worktree::workplaces_in(repo.here(), worktree, &footing, dug)
    } else {
        Vec::new()
    };
    // 워크트리가 없으면 `places` 가 아무 키도 안 내므로(moai-tbin) 여기 가드를 따로 두지 않는다 —
    // 두면 "자리를 물을 수 있는가" 를 재는 자가 둘이 된다.
    let places = report::places_in(&footing, &trees, &model::now()).remove(&issue.id);
    let seen = view::Seen {
        roots: report::deferred_roots(all),
        states: report::group_states_of(all, &repo.config, &near),
        screen: view::Screen::new(ctx.lang()).at(ctx.zone()).over(origin),
        blocks: report::blocks_of(all, &repo.config, issue),
        places,
    };
    // **커밋은 저장하지 않고 git 에서 읽는다**(moai-1w2l) — 커밋 제목에 이 id 를 적은 것.
    // git 이 없거나 저장소 밖이거나 커밋이 하나도 없으면 **말없이** 칸을 비운다(moai-mauw).
    // 커밋 칸 하나 때문에 상세가 안 열리는 것이 빈 칸보다 비싸고, git 없이 moai 를 쓰는 것은
    // 고장이 아니라 흔한 쓰임이라 `show` 마다 한 줄씩 탓하면 그것이 잔소리다.
    // **커밋도 줄이 온 워크트리의 `HEAD` 에서 읽는다** — 이력과 같은 까닭. `--worktree` 로
    // 옆에서 집은 일을 펼치면 그 일을 고친 커밋은 저쪽 가지에만 있다.
    // **탐색기와 같은 자로 읽는다**(moai-hws2) — `git::table` 하나가 이력 전부를 걷는다. 생성일에서
    // 끊던 때는 날짜가 거꾸로 선 커밋 하나가 그 밑을 통째로 가려, 같은 물음에 두 표면이 다른 답을 냈다.
    // **기계에게는 그 침묵을 가른다**(moai-rzsv) — `--json` 은 `commits` 를 늘 내고, git 을 못 읽었을
    // 때만 `commits_error` 를 단다. 사람 화면은 그대로다.
    let root = commit_home(repo, origin, &issue.id);
    let (commits, commits_error) = match crate::git::table(root, &[issue.id.as_str()]) {
        Ok(mut by_id) => (by_id.remove(&issue.id).unwrap_or_default(), None),
        Err(e) => (Vec::new(), Some(e.told(root, ctx.lang()))),
    };

    if ctx.json {
        let ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        // **사람 화면과 같은 자로 고른다** (`report::group_members`). 담아 둔
        // 생각은 거기서 빠진다 — 찾으려면 `moai show --type idea -e <에픽>`. 한때
        // 여기만 에픽에 한해 제 `epic` 을 적은 줄을 내, 마일스톤은 키가 없고
        // 물려받은 자식은 화면에만 있었다(moai-qizs).
        let mut extra = vec![
            ("children", serde_json::to_string(&ids).map_err(|e| Fail::new(e.to_string()))?),
            ("journal", serde_json::to_string(&journal).map_err(|e| Fail::new(e.to_string()))?),
        ];
        // **묶음일 때만 멤버를 고른다** — `group_members` 는 저장소 전체로 지도를 짓는다. 일 하나를
        // `--json` 으로 펼치는 흔한 길에서 그것을 짓고 버리던 자리다.
        //
        // **멤버를 한 번만 고른다**(리뷰, moai-g0zx 와 같은 까닭) — 아래 `spent` 가 같은 묶음의
        // 멤버를 묻는데, 저마다 `group_members` 를 부르면 한 판이 소속 지도를 두 벌 짓는다.
        let mut spent = None;
        if report::is_group(issue) {
            let rows = report::group_members(all, issue);
            let members: Vec<&str> = rows.iter().map(|m| m.id.as_str()).collect();
            extra.push(("members", serde_json::to_string(&members).map_err(|e| Fail::new(e.to_string()))?));
            spent = (issue.kind == Kind::Milestone).then(|| report::spent_in(&rows));
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
            extra.push(("blockers", serde_json::to_string(&seen.blocks).map_err(|e| Fail::new(e.to_string()))?));
        }
        // 사람 화면의 `자리` 줄과 같은 답. 줄을 안 세우는 자리에서는 키도 안 단다.
        if let Some(p) = &seen.places {
            // **자리와 그 뜻을 따로 낸다** — 목록이 비어 있는 까닭이 셋이다(방금 집었다·못 읽은
            // 워크트리가 있다·자리가 없다). 사람 화면이 가르는 것을 받는 쪽도 가를 수 있어야 한다.
            extra.push(("workplaces", serde_json::to_string(p.at()).map_err(|e| Fail::new(e.to_string()))?));
            extra.push(("place", serde_json::to_string(p.word()).map_err(|e| Fail::new(e.to_string()))?));
        }
        // 트래커 커밋까지 **전부** 낸다 — `tracker` 표시가 붙으니 거를지는 받는 쪽이 정한다.
        // 사람 화면만 뺀다(`view::commits`).
        //
        // **키는 늘 선다**(moai-rzsv, 2026-09-15 사용자 결정). 빈 배열도 사실이고, 그것과 "git 을
        // 못 읽었다" 를 가르는 것이 `commits_error` 다 — 없으면 정말 아무도 그 id 를 안 적은 것이다.
        // 없으면 키를 안 다는 `blockers` 와 다른 까닭: 막음은 이슈가 제 파일에 적는 값이라 없음이
        // 곧 답이지만, 커밋은 **바깥에 물어서** 오므로 못 물은 것과 없는 것이 다르다.
        extra.push(("commits", serde_json::to_string(&commits).map_err(|e| Fail::new(e.to_string()))?));
        if let Some(why) = &commits_error {
            extra.push(("commits_error", serde_json::to_string(why).map_err(|e| Fail::new(e.to_string()))?));
        }
        // **이력이 덜 왔으면 그렇다고 말한다**(moai-f2lc). 여기까지 오면 저널은 이미 읽혔다 —
        // 못 읽은 파일은 `journal` 에서 말없이 빠져 있고, 그 침묵을 가르는 자가 이 키다.
        //
        // **종료 코드만으로는 못 가른다.** `cmd::PARTIAL` 은 못 읽는 스냅샷 줄과 진 `mv` 겨룸까지
        // 함께 쓰는 한 깃발이라(moai-6924 가 그것 하나로 버티던 자리다), 받는 쪽은 "이 판이 온전치
        // 않다" 까지만 안다. 어느 이슈의 **이력이** 덜 왔는지는 이 키만 말한다.
        //
        // **키가 없으면 다 왔다.** `commits_error` 와 같은 약속이고, `commits` 와 달리 빈 배열을
        // 안 내는 까닭은 `journal` 이 이미 늘 서기 때문이다 — 빈 `journal` 옆의 이 키가 있고 없고가
        // 곧 "이력이 없다" 와 "못 읽었다" 의 가름이다.
        let unread = super::journal_errors(ctx.lang(), &crate::store::journal_unread(), Some(journal_root));
        if !unread.is_empty() {
            extra.push(("journal_error", serde_json::to_string(&unread).map_err(|e| Fail::new(e.to_string()))?));
        }
        // 이 일을 한 AI — 노트의 `model:` 줄을 읽은 값(moai-8f2g). 저장하지 않는다.
        //
        // **키는 늘 선다**(2026-09-18 사용자 결정). 한 이슈에 여러 세션·모델이 줄을 남기니 배열이고,
        // 없으면 키를 안 다는 모양은 되쓰기에서 옛 키가 `rest` 에 남아 거짓을 싣는다(moai-2l8n).
        extra.push(("work", serde_json::to_string(&model::work_of(&journal)).map_err(|e| Fail::new(e.to_string()))?));
        // 마일스톤에 **든 시간**(moai-wfup). 사람 화면과 같은 값을 같은 자리에서 읽는다 —
        // 저장하지 않으므로 키는 마일스톤 줄에만 선다. 목록(`show [거르개] --json`)에는 안
        // 싣는다: 줄마다 멤버 지도를 다시 짓는 셈이라 한 판이 저장소를 n 번 훑는다.
        if let Some(sp) = spent {
            extra.push(("spent", serde_json::to_string(&sp).map_err(|e| Fail::new(e.to_string()))?));
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
        out.insert(2.min(out.len()), view::duplicate_note(n, ctx.lang()));
    }
    // 묶음을 펼치면 그 밑에 무엇이 있는지까지 보여 준다 — 묶음 하나를 보는
    // 이유가 바로 그것이다. 마일스톤이면 에픽과 이슈가 같이 나온다.
    if report::is_group(issue) {
        // **베끼지 않는다.** 차례는 `view::members` 가 `nav` 에서 받아 정하므로
        // 여기서 필요한 것은 "누가 이 묶음의 멤버인가" 하나뿐이다.
        // **`--json` 이 내는 것과 같은 것을 그린다** — 둘 다
        // `report::group_members` 로 고른다.
        let rows = report::group_members(all, issue);
        let mine: BTreeSet<&str> = rows.iter().map(|i| i.id.as_str()).collect();
        // **소속 지도는 한 벌이다**(moai-g0zx) — 목록 쪽(`run`)과 같은 까닭이다. 이 밑에서
        // 머리글의 굴림·색인·멤버 굴림 셋이 저마다 지으면 `groups` 가 한 번 펼치는 데 세 벌
        // 돈다(마일스톤이면 `milestones` 가 안에서 또 지어 네 벌이다).
        let soil = report::Soil::of(all);
        let eclipsed = soil.eclipsed();
        let group = match issue.kind {
            Kind::Milestone => &soil.milestone,
            _ => &soil.epic,
        };
        // **미룬 수는 보드와 같은 자에서 온다**([`report::Stand::deferred`], moai-zxwj) — 분모는
        // 미룬 멤버를 그대로 세므로, 그 수가 안 줄어드는 까닭을 여기서도 댄다. 따로 세면 같은
        // 묶음을 보드와 상세가 다른 수로 말한다.
        let deferred = soil.stands(all, &repo.config).get(issue.id.as_str()).map(|s| s.deferred);
        let roll = report::rollup_of_in(issue.kind, all, &repo.config, group, &eclipsed)
            .into_iter()
            .find(|r| r.id.as_deref() == Some(issue.id.as_str()))
            .map(|r| report::Roll { deferred, ..r });
        if let Some(r) = &roll {
            out.push(format!(
                "  {}   {}/{}  {}{}{}",
                view::members_label(ctx.lang()),
                r.done,
                r.total,
                view::bar(r.percent),
                match r.percent {
                    None => String::new(),
                    Some(p) => format!("  {p}%"),
                },
                view::set_aside(r.deferred, ctx.lang()),
            ));
        }
        // **마일스톤에만 든 시간을 곁들인다**(moai-wfup, 2026-09-22 사용자 결정). 에픽의
        // 기간은 아직 묻는 자리가 아니고, `report::spent_in` 은 어느 묶음의 멤버에나 서므로
        // 그때 여는 것은 이 한 줄이다. 보드에는 안 낸다 — 마일스톤 표가 이미 좁다.
        //
        // 멤버는 위에서 한 번 고른 것을 그대로 쓴다 — 여기서 다시 물으면 소속 지도가 한 판에
        // 두 벌 선다(리뷰).
        if issue.kind == Kind::Milestone
            && let Some(line) = view::spent(&report::spent_in(&rows), ctx.lang())
        {
            out.push(line);
        }
        // **자리를 못 찾으면 아무것도 내지 않는다.** 뿌리로 되돌리면 그 에픽의
        // 멤버라며 저장소 전부를 낸다 — 없는 답보다 틀린 답이 비싸다.
        if !mine.is_empty() {
            let index = crate::nav::Index::in_soil(all, &soil);
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
                let rolls = report::rollup_in(all, &repo.config, &soil);
                // **상세가 이미 든 화면을 그대로 쓴다**(리뷰) — `seen.screen` 이 바로 그 값이고
                // [`view::Screen`] 은 `Copy` 다. 여기서 다시 지으면 한 번 펼치는 화면 안에 같은
                // 맥락을 짓는 자리가 둘이 된다.
                out.extend(view::members(all, &index, &keep, &rolls, &here, seen.screen));
            }
        }
    }
    // **말은 명령 층이 한 번 풀어 준다**(`Ctx::lang`) — 위의 `seen.screen` 이 든 것과 같은
    // 값이다. 머리글만 다른 말로 서면 한 번 펼친 화면 안에서 말이 갈린다.
    out.extend(view::commits(&commits, ctx.lang()));
    out.extend(view::history(&journal, &repo.config, seen.screen));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`Listed` 가 줄 곁에 다는 키도 `cmd::OURS` 에 있다**(moai-qn5d) — 없으면 `show --json` 을
    /// 되써 넣은 줄의 같은 이름이 안 걷혀 한 객체에 둘 선다. `Listed` 는 `json_with` 를 안 지나
    /// 그쪽 확인이 안 선다 — `Listed` 에 필드를 더하면 여기서 붉어진다(리뷰 moai-u5bk.3wq).
    #[test]
    fn every_key_listed_adds_is_in_ours() {
        let i = Issue::new(
            "argos-0001".into(),
            "제목".into(),
            Kind::Issue,
            model::Status::new("todo"),
            "2026-09-11T04:12:03Z",
        );
        let listed = Listed { row: super::super::Row::of(&i, None), work: &[], journal_error: &[] };
        let added = super::super::keys_beyond(&i, &listed);
        assert!(added.iter().any(|k| k == "work"), "곁들인 키를 못 셌다 — {added:?}");
        for k in &added {
            assert!(super::super::OURS.contains(&k.as_str()), "`Listed` 가 곁들이는 {k} 가 `OURS` 에 없다");
        }
    }
}
