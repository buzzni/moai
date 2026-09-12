//! 담아 둔 생각을 펼친다.
//!
//! `idea add`·`idea show` 는 종류 고정 장치가 그대로 처리한다. 여기 있는 것은
//! idea 에만 있는 동사 하나다 — 에픽과 마일스톤에는 "펼친다" 가 없다.

use super::{Ctx, Fail, R};
use crate::cli::PromoteArgs;
use crate::draft;
use crate::model::{self, Issue, JournalEntry, Kind, Status};
use crate::store::Repo;
use crate::style::{self, paint};

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
    let src = read_source(&args.from)?;
    let drafts = draft::parse(&src).map_err(|e| Fail::coded(e, super::code::BAD_INPUT))?;

    // **연습은 저장소를 안 만진다.** AI 가 펼친 안을 사람이 한 번 보고
    // "좋다" 하는 자리라, 여기서 쓰면 그 "좋다" 가 뒤늦은 말이 된다.
    if args.dry_run {
        let mut out = vec![paint(style::HEAD, "펼칠 것")];
        out.extend(drafts.iter().map(|d| crate::cmd::add::line_of(d, None)));
        out.push(String::new());
        out.push(crate::cmd::add::tally(&drafts));
        out.push(paint(style::DIM, &format!("{} 는 done 으로 간다", args.id)));
        return Ok(out);
    }

    let at = model::now();
    let by = model::actor(ctx.user.as_deref())?;
    let made: Vec<Issue> = repo.with_write(|issues, cfg| {
        // 펼칠 것이 정말 idea 인지 **먼저** 본다. 나중에 보면 만들어진 id 가
        // 오류 메시지에 실려 나가고, 받는 쪽은 그게 남은 줄 안다.
        let thought = issues
            .iter()
            .find(|i| i.id == args.id)
            .ok_or_else(|| Fail::coded(format!("{} 를 못 찾았다", args.id), super::code::NOT_FOUND))?;
        if thought.kind != Kind::Idea {
            return Err(Fail::coded(
                format!(
                    "{} 는 idea 가 아니라 {} 다 — 펼치면 까닭 없이 닫힌다",
                    args.id,
                    thought.kind.as_str()
                ),
                super::code::BAD_TARGET,
            ));
        }
        let title = thought.title.clone();
        let was = thought.status.clone();
        // 담아 둔 생각의 담당을 물려준다. 펼친 계획의 임자가 없으면 `ready` 가
        // 집으라고 내면서 누가 집는지는 말하지 않는다.
        let heir = thought.assignee.as_ref().map(|name| match &thought.assignee_email {
            Some(mail) => format!("{name} ({mail})"),
            None => name.clone(),
        });
        drop(thought);

        let (mut entries, made) =
            crate::cmd::add::create_drafts(issues, cfg, &drafts, heir.as_deref(), &by, &at)?;

        // **어느 쪽에서 봐도 이어진다.** 펼친 계획에서 "어디서 나왔나" 를
        // 물을 수도, 담아 둔 생각에서 "무엇이 됐나" 를 물을 수도 있다.
        for top in made.iter().filter(|i| i.epic.is_none()) {
            entries.push(JournalEntry::note(
                &top.id,
                &format!("{} 에서 펼쳤다 — {title}", args.id),
                &at,
                &by,
            ));
        }
        let grown: Vec<&str> =
            made.iter().filter(|i| i.epic.is_none()).map(|i| i.id.as_str()).collect();
        let done = Status::new(crate::config::DONE);
        entries.push(JournalEntry::status(
            &args.id,
            &was,
            &done,
            Some(format!("{} 로 펼쳤다 (이슈 {}건)", grown.join(" "), made.len() - grown.len())),
            &at,
            &by,
        ));

        let thought = issues.iter_mut().find(|i| i.id == args.id).expect("방금 찾은 줄이 사라졌다");
        thought.status = done;
        thought.status_since = at.clone();
        thought.updated_at = at.clone();
        Ok((entries, made))
    })?;

    if ctx.json {
        return super::json_line(&made);
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

/// `-` 이면 stdin. `add --from` 과 같은 규칙이다.
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
