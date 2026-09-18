//! 담아 둔 생각을 펼친다.
//!
//! `idea add`·`idea show` 는 종류 고정 장치가 그대로 처리한다. 여기 있는 것은
//! idea 에만 있는 동사 하나다 — 에픽과 마일스톤에는 "펼친다" 가 없다.

use super::{Ctx, Fail, R};
use crate::cli::PromoteArgs;
use crate::draft::Shape;
use crate::model::{self, Issue, JournalEntry, Kind, Status};
use crate::store::Repo;
use crate::style::{self, paint};

/// 펼칠 수 없는 것을 펼치라 했을 때. **한 곳에서 만든다** — 연습과 진짜가
/// 같은 것을 거절하는데 문장이 둘이면, 어느 쪽을 봤느냐로 말이 달라진다.
fn not_an_idea(id: &str, i: &Issue) -> Fail {
    Fail::coded(
        format!("{id} 는 idea 가 아니라 {} 다 — 펼치면 까닭 없이 닫힌다", i.kind.as_str()),
        super::code::BAD_TARGET,
    )
}

/// `-e` 로 받은 것이 멤버를 받을 수 있는 에픽인가. **없거나 에픽이 아니면 거절한다** —
/// `add -e` 는 없는 에픽을 알리고 넘어가지만, 여기서는 idea 가 닫히므로 틀린 자리에 펼친
/// 것을 되돌릴 길이 도구 밖에만 남는다.
fn check_epic(issues: &[Issue], id: &str) -> R<()> {
    let e = issues.iter().find(|i| i.id == id).ok_or_else(|| Fail::not_found(id))?;
    if e.kind != Kind::Epic {
        return Err(Fail::coded(
            format!("{id} 는 에픽이 아니라 {} 다 — `-e` 는 멤버를 받을 에픽을 가리킨다", e.kind.as_str()),
            super::code::BAD_TARGET,
        ));
    }
    Ok(())
}

/// idea 하나를 에픽 하나 + 이슈 여럿으로 펼치고, 그 idea 를 닫는다.
/// `-e <에픽>` 이면 새 에픽 없이 이미 선 에픽의 멤버로 펼친다(moai-f3ml).
///
/// 받는 마크다운은 `add --from` 과 **같은 형식**이다. 형식이 둘이 되면
/// 에이전트가 어느 쪽 문법인지 매번 틀린다.
///
/// 하는 일은 셋이고, 한 번의 쓰기라 다 되거나 하나도 안 된다.
///
/// 1. 에픽과 이슈를 만든다 (`add --from` 과 같은 길)
/// 2. 그 idea 를 `done` 으로 옮긴다 — 펼쳐졌으므로 더 볼 것이 없다
/// 3. 저널에 무엇이 무엇에서 나왔는지 적는다
///
/// **3번을 필드로 만들지 않는다.** `idea.spawned = [에픽 id]` 를 들면 에픽을
/// 지울 때 idea 도 고쳐야 하고, 그건 파생값을 저장한 대가다.
pub fn promote(ctx: &Ctx, args: PromoteArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    // `add --from` 과 **한 길**이다 — 읽기·템플릿 채우기·형식 읽기(moai-cypw).
    // `-e` 면 에픽은 이미 있다 — 계획은 그 에픽에 넣을 이슈만 적는다(moai-f3ml).
    let shape = if args.epic.is_some() { Shape::Members } else { Shape::Plan };
    let drafts = crate::cmd::add::read_plan(&args.from, &args.var, shape)?;
    let into = args.epic.as_deref();

    // **연습은 저장소를 안 만진다.** AI 가 펼친 안을 사람이 한 번 보고
    // "좋다" 하는 자리라, 여기서 쓰면 그 "좋다" 가 뒤늦은 말이 된다.
    //
    // **읽는 것도 연습일 때뿐이다.** 진짜로 펼칠 때는 락 안에서 다시 읽고
    // 다시 보므로 여기서 본 것은 어차피 안 믿는다 — 파일만 두 번 훑을 뿐이다.
    // 게다가 `report_load_errors` 는 부분 실패 깃발을 세우므로, 쓰기 경로에
    // 두면 못 읽는 줄 하나가 **성공한 promote 를 실패로 보이게** 한다. 그것을
    // 실패로 읽은 쪽이 다시 부르면 같은 계획이 두 벌 생긴다.
    if args.dry_run {
        // **연습도 진짜와 같은 것을 본다.** 연습이 승인의 자리인데 거기서
        // 못 할 일을 하겠다고 말하면, 사람이 "좋다" 한 뒤에야 도구가 거절한다.
        let load = repo.read()?;
        // **깃발은 안 세운다.** 진짜 `promote` 는 못 읽는 줄을 그대로 들고
        // 넘어가 0 으로 끝나는데 연습만 1 로 끝나면, 그것을 거절로 읽은 쪽이
        // 도구가 기꺼이 해 줄 계획을 버린다. 어느 줄인지는 그대로 말한다.
        super::name_load_errors(&repo.issues_path(), &load.errors);
        let thought = load.get(&args.id).ok_or_else(|| Fail::not_found(&args.id))?;
        if !crate::report::is_idea(thought) {
            return Err(not_an_idea(&args.id, thought));
        }
        if let Some(e) = into {
            check_epic(&load.issues, e)?;
        }
        // **거절은 `--json` 보다 먼저다.** 못 할 일을 하겠다고 말하면 모양이
        // 무엇이든 거절이고, 뒤에 두면 연습이 조용히 "된다" 고 낸다.
        if ctx.json {
            return crate::cmd::add::json_rehearsal(&drafts, Some(&args.id), into);
        }
        let mut out = vec![paint(style::HEAD, "펼칠 것")];
        out.extend(drafts.iter().map(|d| crate::cmd::add::line_of(d, None)));
        out.push(String::new());
        out.push(crate::cmd::add::tally(&drafts));
        if let Some(e) = into {
            out.push(paint(style::DIM, &format!("{e} 의 멤버로 든다")));
        }
        out.push(paint(style::DIM, &format!("{} 는 done 으로 간다", args.id)));
        return Ok(out);
    }

    let by = model::actor(ctx.user.as_deref(), &repo.root)?;
    let (made, read): (Vec<Issue>, super::Read) = repo.with_write(|issues, cfg, reserved| {
        // 시각은 **락을 쥔 뒤에** 뜬다 — `mv` 와 같은 까닭이다. 밖에서 뜨면 이 닫기가 옆의 집기보다
        // 늦게 써져도 이른 시각을 들어, 생각의 끝이 시작보다 앞선다(리뷰 moai-u5bk.3wq).
        let at = model::now();
        // 펼칠 것이 정말 idea 인지 **먼저** 본다. 나중에 보면 만들어진 id 가
        // 오류 메시지에 실려 나가고, 받는 쪽은 그게 남은 줄 안다.
        //
        // 자리를 **한 번만** 찾는다. `create_drafts` 는 뒤에 밀어 넣기만 하니
        // 첨자가 밀리지 않고, 그래야 "방금 찾은 줄이 사라졌다" 같은 있지도
        // 않을 경우를 위한 `expect` 가 필요 없다.
        let at_idea =
            issues.iter().position(|i| i.id == args.id).ok_or_else(|| Fail::not_found(&args.id))?;
        let thought = &issues[at_idea];
        // 연습에서 이미 봤을 수도 있지만 다시 본다 — 그 사이에 누가 지우거나
        // 바꿨을 수 있고, 쓰기가 믿을 것은 락 안에서 읽은 것뿐이다.
        if !crate::report::is_idea(thought) {
            return Err(not_an_idea(&args.id, thought));
        }
        let title = thought.title.clone();
        let was = thought.status.clone();
        // 들 에픽도 락 안에서 다시 본다 — 연습과 진짜 사이에 지워졌을 수 있다.
        if let Some(e) = into {
            check_epic(issues, e)?;
        }
        // 담아 둔 생각의 담당을 **갈라진 채로** 물려준다. 펼친 계획의 임자가
        // 없으면 `ready` 가 집으라고 내면서 누가 집는지는 말하지 않는다.
        //
        // **`model::label` 로 합쳤다가 다시 가르지 않는다.** 그쪽은 화면에
        // 쓰는 말이라 되돌릴 수 없다 — 이름이 빈 줄(손으로 푼 머지가 남기는
        // 모양)은 `"메일"` 한 토막이 되고, 되가르는 쪽은 괄호가 없으니 그
        // 메일을 **이름 칸**에 넣고 메일을 버린다. 파일에는 갈라서, 화면에는
        // 합쳐서 — 여기는 파일 쪽이다.
        let heir = (thought.assignee.clone(), thought.assignee_email.clone());

        let (mut entries, made) =
            crate::cmd::add::create_drafts(issues, cfg, reserved, &drafts, into, &heir, &by, &at)?;

        // **어느 쪽에서 봐도 이어진다.** 펼친 계획에서 "어디서 나왔나" 를
        // 물을 수도, 담아 둔 생각에서 "무엇이 됐나" 를 물을 수도 있다.
        //
        // 뿌리로 선 것(제 에픽이 없는 것)이 펼친 계획의 머리다. **한 번만
        // 고른다** — 두 번 고르면 규칙이 둘이 되고, 갈라진 날 저널의 두 줄이
        // 서로 다른 것을 가리킨다.
        //
        // 선 에픽에 펼치면(`-e`) 뿌리가 없다 — 만든 이슈 하나하나가 머리다. 에픽에는 적지
        // 않는다: 그 에픽은 이 idea 에서 나온 것이 아니다.
        let grown: Vec<String> = made
            .iter()
            .filter(|i| into.is_some() || i.epic.is_none())
            .map(|i| i.id.clone())
            .collect();
        for top in &grown {
            entries.push(JournalEntry::note(
                top,
                &format!("{} 에서 펼쳤다 — {title}", args.id),
                &at,
                &by,
            ));
        }
        let done = Status::new(crate::config::DONE);
        let note = match into {
            Some(e) => format!("{e} 의 멤버 {} 로 펼쳤다", grown.join(" ")),
            None => format!("{} 로 펼쳤다 (이슈 {}건)", grown.join(" "), made.len() - grown.len()),
        };
        // **이미 닫힌 것을 또 닫지 않는다.** `done → done` 을 적으면 저널에
        // 일어나지도 않은 전이가 남고, `status_since` 가 움직여 "언제 닫혔나"
        // 가 마지막 `promote` 시각으로 밀린다. 적어 온 말은 그래도 버리지
        // 않는다 — `mv` 가 같은 자리에서 같은 규칙을 쓴다.
        if was == done {
            entries.push(JournalEntry::note(&args.id, &note, &at, &by));
        } else {
            entries.push(JournalEntry::status(&args.id, &was, &done, Some(note), &at, &by));
            // `mv` 와 **같은 길로** 옮긴다 — 시작·끝 시각까지(`Issue::move_to`, moai-38mh). 손으로
            // 칸만 옮기던 때는 펼쳐 닫힌 생각에만 `done_at` 이 안 섰다.
            issues[at_idea].move_to(done, &at, cfg);
        }
        // 펼치면 에픽이 선다 — 적힌 칸을 그대로 내면 받는 쪽이 안 읽히는 칸을 읽는다.
        let ids: Vec<&str> = made.iter().map(|i| i.id.as_str()).collect();
        let read = crate::cmd::read_of(issues, cfg, &ids);
        Ok((entries, (made, read)))
    })?;

    if ctx.json {
        // **닫힌 생각까지 낸다.** 사람 출력에는 `→ done` 이 있는데 기계
        // 출력에만 없으면 받는 쪽이 두 표면 중 하나를 못 믿게 된다 —
        // `mv --json` 이 옮긴 것 말고도 다 내는 것과 같은 까닭이다.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            made: Vec<super::Row<'a>>,
            promoted: &'a str,
            status: &'a str,
        }
        return super::json_line(&Out {
            made: made.iter().map(|i| super::Row::from(i, &read)).collect(),
            promoted: &args.id,
            status: crate::config::DONE,
        });
    }
    let mut out = vec![paint(style::HEAD, "펼침")];
    out.extend(drafts.iter().zip(&made).map(|(d, i)| crate::cmd::add::line_of(d, Some(&i.id))));
    out.push(String::new());
    out.push(crate::cmd::add::tally(&drafts));
    if let Some(e) = into {
        out.push(paint(style::DIM, &format!("{e} 의 멤버로 들었다")));
    }
    out.push(format!(
        "{}  {}",
        paint(style::ID, &args.id),
        paint(style::DIM, "→ done  (펼쳐졌으므로 더 볼 것이 없다)")
    ));
    Ok(out)
}
