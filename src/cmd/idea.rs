//! 담아 둔 생각을 펼친다.
//!
//! `idea add`·`idea show` 는 종류 고정 장치가 그대로 처리한다. 여기 있는 것은
//! idea 에만 있는 동사 하나다 — 에픽과 마일스톤에는 "펼친다" 가 없다.

use super::{Ctx, Fail, R};
use crate::cli::PromoteArgs;
use crate::draft;
use crate::model::{self, Issue, JournalEntry, Status};
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

/// idea 하나를 에픽 하나 + 이슈 여럿으로 펼치고, 그 idea 를 닫는다.
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
    let src = crate::cmd::add::read_source(&args.from)?;
    let drafts = draft::parse(&src).map_err(|e| Fail::coded(e, super::code::BAD_INPUT))?;

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
        super::report_load_errors(&repo.issues_path(), &load.errors);
        let thought = load.get(&args.id).ok_or_else(|| Fail::not_found(&args.id))?;
        if !crate::report::is_idea(thought) {
            return Err(not_an_idea(&args.id, thought));
        }
        // **거절은 `--json` 보다 먼저다.** 못 할 일을 하겠다고 말하면 모양이
        // 무엇이든 거절이고, 뒤에 두면 연습이 조용히 "된다" 고 낸다.
        if ctx.json {
            return crate::cmd::add::json_rehearsal(&drafts, Some(&args.id));
        }
        let mut out = vec![paint(style::HEAD, "펼칠 것")];
        out.extend(drafts.iter().map(|d| crate::cmd::add::line_of(d, None)));
        out.push(String::new());
        out.push(crate::cmd::add::tally(&drafts));
        out.push(paint(style::DIM, &format!("{} 는 done 으로 간다", args.id)));
        return Ok(out);
    }

    let at = model::now();
    let by = model::actor(ctx.user.as_deref())?;
    let made: Vec<Issue> = repo.with_write(|issues, cfg, reserved| {
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
            crate::cmd::add::create_drafts(issues, cfg, reserved, &drafts, &heir, &by, &at)?;

        // **어느 쪽에서 봐도 이어진다.** 펼친 계획에서 "어디서 나왔나" 를
        // 물을 수도, 담아 둔 생각에서 "무엇이 됐나" 를 물을 수도 있다.
        //
        // 뿌리로 선 것(제 에픽이 없는 것)이 펼친 계획의 머리다. **한 번만
        // 고른다** — 두 번 고르면 규칙이 둘이 되고, 갈라진 날 저널의 두 줄이
        // 서로 다른 것을 가리킨다.
        let grown: Vec<String> =
            made.iter().filter(|i| i.epic.is_none()).map(|i| i.id.clone()).collect();
        for top in &grown {
            entries.push(JournalEntry::note(
                top,
                &format!("{} 에서 펼쳤다 — {title}", args.id),
                &at,
                &by,
            ));
        }
        let done = Status::new(crate::config::DONE);
        let note = format!("{} 로 펼쳤다 (이슈 {}건)", grown.join(" "), made.len() - grown.len());
        // **이미 닫힌 것을 또 닫지 않는다.** `done → done` 을 적으면 저널에
        // 일어나지도 않은 전이가 남고, `status_since` 가 움직여 "언제 닫혔나"
        // 가 마지막 `promote` 시각으로 밀린다. 적어 온 말은 그래도 버리지
        // 않는다 — `mv` 가 같은 자리에서 같은 규칙을 쓴다.
        if was == done {
            entries.push(JournalEntry::note(&args.id, &note, &at, &by));
        } else {
            entries.push(JournalEntry::status(&args.id, &was, &done, Some(note), &at, &by));
            let thought = &mut issues[at_idea];
            thought.status = done;
            thought.status_since = at.clone();
            thought.updated_at = at.clone();
        }
        Ok((entries, made))
    })?;

    if ctx.json {
        // **닫힌 생각까지 낸다.** 사람 출력에는 `→ done` 이 있는데 기계
        // 출력에만 없으면 받는 쪽이 두 표면 중 하나를 못 믿게 된다 —
        // `mv --json` 이 옮긴 것 말고도 다 내는 것과 같은 까닭이다.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            made: &'a [Issue],
            promoted: &'a str,
            status: &'a str,
        }
        return super::json_line(&Out {
            made: &made,
            promoted: &args.id,
            status: crate::config::DONE,
        });
    }
    let mut out = vec![paint(style::HEAD, "펼침")];
    out.extend(drafts.iter().zip(&made).map(|(d, i)| crate::cmd::add::line_of(d, Some(&i.id))));
    out.push(String::new());
    out.push(crate::cmd::add::tally(&drafts));
    out.push(format!(
        "{}  {}",
        paint(style::ID, &args.id),
        paint(style::DIM, "→ done  (펼쳐졌으므로 더 볼 것이 없다)")
    ));
    Ok(out)
}
