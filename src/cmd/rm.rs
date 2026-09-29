//! 지운다. 에이전트가 잘못 만든 것을 치우는 길이다.
//!
//! 자식이나 에픽 멤버가 남아 끊긴 참조가 되는 것은 **막지 않고 알린다.**
//! `moai status` 가 끊긴 참조를 드러내므로 여기서 막을 이유가 없다.

use super::{Ctx, Fail, R};
use crate::cli::RmArgs;
use crate::model::{self, Issue, JournalEntry};
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: RmArgs) -> R<Vec<String>> {
    // **`--yes` 는 `--line` 의 것이다.** 붙여 친 사람은 "한 번 묻는 문" 을 기대하는데 id 로 지우는
    // 길에는 그 문이 없다 — 지나 보내면 묻는 줄 알았던 지우기가 말없이 일어난다.
    if args.yes && args.line.is_none() {
        return Err(Fail::coded(crate::i18n::say(ctx.lang(), "refuse.yes_without_line"), super::code::BAD_INPUT));
    }
    let repo = super::open_repo(ctx)?;
    if let Some(n) = args.line {
        return line(ctx, &repo, n, args.yes);
    }
    let at = model::now();
    let by = model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, ctx.lang()))?;

    let (gone, missing, dangling): (Vec<Issue>, Vec<String>, Vec<String>) = repo.with_write(
        || ctx.lang(),
        |issues, _, _| {
            let (mut gone, mut missing, mut dangling) = (Vec::new(), Vec::new(), Vec::new());
            let mut entries = Vec::new();
            for id in &args.ids {
                // 같은 id 를 두 번 적은 것은 실패가 아니다. 인자 목록은 glob·
                // xargs·에이전트가 짓는 것이라 중복이 흔하고, 지워 놓고
                // "못 찾았다" 며 비영으로 끝내면 부르는 쪽이 되돌리려 든다.
                if gone.iter().any(|g: &Issue| &g.id == id) {
                    continue;
                }
                let Some(at_idx) = issues.iter().position(|i| &i.id == id) else {
                    missing.push(id.clone());
                    continue;
                };
                let i = issues.remove(at_idx);
                entries.push(JournalEntry::removed(&i.id, &i.title, &at, &by));
                gone.push(i);
            }
            for i in issues.iter() {
                let orphan = crate::id::parent_of(&i.id).is_some_and(|p| gone.iter().any(|g| g.id == p));
                let lost = i.epic.as_deref().is_some_and(|e| gone.iter().any(|g| g.id == e));
                let unblocked = i.blocked_by.iter().any(|b| gone.iter().any(|g| &g.id == b));
                if orphan || lost || unblocked {
                    dangling.push(i.id.clone());
                }
            }
            Ok((entries, (gone, missing, dangling)))
        },
    )?;

    for id in &missing {
        super::note_partial();
        // 없는 id 는 **어느 명령에서나 한 낱말이다**([`Fail::not_found`], moai-95g1) — 리뷰가
        // 넷째 사본으로 짚은 자리다. 나머지 `rm` 몸통은 아직 한국어다(idea moai-a7qo).
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.not_found"), &[("id", id)]));
    }
    if !dangling.is_empty() {
        eprintln!(
            "moai: {}",
            crate::i18n::fill(
                crate::i18n::say(ctx.lang(), "rm.dangling_left"),
                &[("n", &dangling.len().to_string()), ("ids", &dangling.join(" "))]
            )
        );
    }

    if ctx.json {
        // 끊긴 참조는 지운 쪽이 알아야 할 결과다. 사람에게만 말하고 기계에는
        // 안 말하면, 그 뒤처리를 할 쪽이 바로 그 기계다.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            removed: &'a [Issue],
            missing: &'a [String],
            dangling: &'a [String],
        }
        return super::json_line(&Out { removed: &gone, missing: &missing, dangling: &dangling });
    }
    Ok(gone.iter().map(|i| format!("{}  {}", paint(style::ID, &i.id), paint(style::DIM, &i.title))).collect())
}

/// `--line` 이 내는 줄. 보여 주기만 한 경우와 지운 경우가 **한 꼴**이고 `dry_run` 이 가른다 — 두 꼴로
/// 두면 받는 쪽이 `--yes` 를 붙였는지부터 다시 따져야 한다.
///
/// **가르는 키를 `removed` 로 안 짓는다**(리뷰 moai-mo9v.1ln). 같은 명령의 `moai rm <id> --json` 에서
/// `removed` 는 지운 줄의 배열이라, 여기서 같은 키가 불리언으로 서면 두 모드를 한 파서로 읽는 쪽이 깨진다
/// (`jq '.removed | length'`). 미리 보기와 실행을 가르는 이름은 `add --dry-run`·`skill install --dry-run` 이
/// 이미 쓰는 `dry_run` 이다.
#[derive(serde::Serialize)]
struct Line<'a> {
    line: usize,
    /// 그 줄이 쓰는 id. 못 읽었으면 `null` 이다 — 지어내지 않는다([`crate::store::LoadError::id`]).
    id: Option<&'a str>,
    /// 파일에 있던 그대로의 원문. 지웠으면 이것이 되살릴 바이트다.
    text: &'a str,
    /// `--yes` 가 없어 보여 주기만 했다.
    dry_run: bool,
}

/// **못 읽는 줄을 줄 번호로 지운다**(moai-mo9v.3yp, 2026-09-29 사용자 결정).
///
/// 못 읽는 줄은 `Issue` 가 아니라 id 로 부를 수 없고(id 조차 없는 줄도 있다), `store::with_write` 는
/// 그 줄을 들고 되쓰기만 한다 — 사람이 그 줄을 치우려면 편집기로 파일을 열어야 했다. 번호는
/// `moai show` 가 대는 그 번호다(`store::LoadError::line`).
///
/// **되돌릴 수 없어 한 번 묻는다.** `--yes` 가 없으면 그 줄을 보여 주고 아무것도 안 쓴다. 묻는
/// 꼴을 터미널의 y/N 으로 두지 않은 것은 에이전트와 파이프에서도 같은 문이어야 해서다.
///
/// **못 읽는 줄만 지운다.** 번호가 못 읽는 줄을 가리키지 않으면(읽히는 줄·빈 줄·파일 밖) 거절하고, 지금
/// 못 읽는 줄을 번호와 id 로 댄다. 락 안에서 다시 읽은 번호로 찾으므로, 보여 준 뒤 옆 세션의 쓰기로
/// 줄이 밀려 그 번호에 산 이슈가 서면 그 이슈는 안 지운다.
///
/// **거절문은 지우는 명령을 안 댄다**(리뷰 moai-mo9v.1ln). 한때 그 번호의 읽히는 줄을 가리켜
/// `moai rm <id>` 를 댔는데, 번호가 밀린 경우 그 id 는 아무도 겨누지 않은 옆 이슈였고 같은 id 가 두 줄이면
/// `rm <id>` 는 앞줄을 지운다 — 따라 친 쪽이 남기려던 줄을 잃는다. `duplicate_id` 경고가 지우는 명령
/// 대신 보는 명령을 대는 것과 같은 까닭이다(moai-pp9i.gtc). 그 줄이 무엇인지 알려고 파일을 한 번 더
/// 읽던 길도 함께 걷었다 — `store` 밖의 둘째 읽기였고, 락 밖에서는 두 벌의 파일을 섞어 말했다.
///
/// **번호가 다른 못 읽는 줄에 떨어지는 것은 못 막는다.** 줄이 늘거나 줄면 번호가 움직이고 — 앞의
/// `--line` 한 번도 그렇다 — `--yes` 는 보여 준 원문을 모른다. 막으려면 보여 준 줄에 묶는 새 인자가
/// 있어야 해서 사람이 정할 일로 남겼다. 그래서 지운 뒤에도 원문을 찍어 무엇이 갔는지 그 자리에서 보인다.
fn line(ctx: &Ctx, repo: &crate::store::Repo, n: usize, yes: bool) -> R<Vec<String>> {
    let lang = ctx.lang();
    // 거절문은 그 순간 읽은 못 읽는 줄로 짓는다. 번호에 id 를 곁들여야 산 줄의 깨진 쌍둥이를 골라낸다.
    let refuse = |errors: &[crate::store::LoadError]| -> Fail {
        let now: Vec<String> = errors
            .iter()
            .map(|e| match e.id.as_deref() {
                // 파일에서 온 글이다 — `id::in_value` 는 제어문자를 안 거른다.
                Some(id) => format!("{} ({})", e.line, crate::text::one_line(id)),
                None => e.line.to_string(),
            })
            .collect();
        let lines = if now.is_empty() { "-".to_string() } else { now.join(", ") };
        Fail::coded(
            crate::i18n::fill(
                crate::i18n::say(lang, "refuse.line_not_unreadable"),
                &[("line", &n.to_string()), ("lines", &lines)],
            ),
            super::code::NOT_FOUND,
        )
    };

    let e = if yes {
        let at = model::now();
        let by = model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, lang))?;
        repo.with_write_lines(
            || ctx.lang(),
            |_, unread, _, _| {
                // 락 안에서 다시 읽은 줄로 찾고 거절문도 그것으로 짓는다 — 밖에서 본 번호는 낡았을 수 있다.
                let Some(k) = unread.iter().position(|e| e.line == n) else { return Err(refuse(unread)) };
                let e = unread.remove(k);
                Ok((vec![JournalEntry::removed_line(e.id.as_deref(), &e.text, &at, &by)], e))
            },
        )?
    } else {
        let mut errors = repo.read()?.errors;
        let Some(k) = errors.iter().position(|e| e.line == n) else { return Err(refuse(&errors)) };
        errors.swap_remove(k)
    };
    let l = Line { line: n, id: e.id.as_deref(), text: &e.text, dry_run: !yes };
    if ctx.json {
        return super::json_line(&l);
    }
    // **원문은 두 경우 다 찍는다**(리뷰 moai-mo9v.1ln). 지운 뒤에 안 찍으면 번호가 다른 줄에 떨어진 것을
    // 아무도 못 보고, 저널 쓰기가 실패했거나 64KB 에서 잘린 줄은 어디에도 온전히 안 남는다. 파일에서 온
    // 글이라 제어문자를 걷어 그린다(`text::one_line`) — 바이트 그대로는 `--json` 의 `text` 에 있다.
    let mut out = vec![head(ctx, &l), format!("  {}", crate::text::one_line(&e.text))];
    if !yes {
        out.push(crate::i18n::say(lang, "rm.line_ask").to_string());
    } else if e.text.len() > model::MAX_TEXT_BYTES {
        // 저널의 `rm` 줄은 `model::fit_bytes` 가 자른 앞머리만 든다(`JournalEntry::removed_line`).
        out.push(crate::i18n::say(lang, "rm.line_cut").to_string());
    }
    Ok(out)
}

/// `line 812 (argos-0001):` 또는 `removed line 812 (argos-0001):` — id 를 못 읽은 줄은 괄호가 안 선다.
fn head(ctx: &Ctx, l: &Line<'_>) -> String {
    // id 도 파일에서 온 글이다 — 제어문자를 걷어 그린다(`text::one_line`).
    let id = l.id.map(|id| format!(" ({})", paint(style::ID, &crate::text::one_line(id)))).unwrap_or_default();
    // 키를 `say` 에 글자째 적는다 — 표와 소스를 견주는 시험이 그 꼴로만 찾는다.
    let said = match l.dry_run {
        true => crate::i18n::say(ctx.lang(), "rm.line_shown"),
        false => crate::i18n::say(ctx.lang(), "rm.line_removed"),
    };
    crate::i18n::fill(said, &[("line", &l.line.to_string()), ("id", &id)])
}
