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
    // **`--match` 는 `--line --yes` 의 것이다.** 없이 준 해시는 아무것도 안 묶는다 — 보여 주기만 하는 길이나
    // id 로 지우는 길에서 받아 두면 친 사람은 그 해시에 묶어 지운 줄로 읽는다. 거절문은 두 길 다에 맞게
    // `--line` 과 `--yes` 를 함께 댄다 — `--yes` 만 대던 글은 id 로 지우는 길에서 `yes_without_line` 과 서로를
    // 가리키며 돌았다(리뷰 moai-mo9v.w12).
    if args.matches.is_some() && !args.yes {
        return Err(Fail::coded(crate::i18n::say(ctx.lang(), "refuse.match_without_yes"), super::code::BAD_INPUT));
    }
    let repo = super::open_repo(ctx)?;
    if let Some(n) = args.line {
        // **해시 없는 `--yes` 는 파일을 읽기 전에 거절한다** — 빈 값과 해시 꼴이 아닌 값(오타·빈 변수)도 같다.
        // 락 안까지 가져가면 "줄이 움직였거나 바뀌었다" 로 거절해, 움직이지 않은 줄을 두고 다시 보라고 한다
        // (리뷰 moai-mo9v.w12). 거절문이 대는 보는 명령에 `-C` 가 붙을 수 있어 뿌리를 연 뒤에 잰다.
        let want = match (args.yes, args.matches.as_deref().map(str::trim)) {
            (false, _) => None,
            (true, Some(h)) if is_hash(h) => Some(h),
            (true, _) => {
                let cmd = to_type(ctx, &repo, &format!("rm --line {n}"));
                return Err(Fail::coded(
                    crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.yes_without_match"), &[("cmd", &cmd)]),
                    super::code::BAD_INPUT,
                ));
            }
        };
        return line(ctx, &repo, n, want);
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
            // **끊긴 id 는 "지운 줄이 있었다" 가 아니라 "지운 뒤 그 id 의 줄이 하나도 안 선다" 다.**
            // 같은 id 가 두 줄이면(`duplicate_id`) `rm <id>` 는 앞줄만 걷어 내고 쌍둥이가 남는다 — 그 id 를
            // 가리키던 자식·멤버·막힌 줄은 여전히 닿는 곳이 있다. 지운 id 만 보고 세면 끊기지 않은 것을
            // 끊겼다고 알려, 알림이 거짓이 되고 `--json` 을 읽는 뒤처리가 헛 고침을 한다. 부모·에픽·막는 줄
            // 셋이 이 집합 하나로 답한다.
            let cut: Vec<&str> =
                gone.iter().map(|g| g.id.as_str()).filter(|id| !issues.iter().any(|i| i.id == *id)).collect();
            for i in issues.iter() {
                let orphan = crate::id::parent_of(&i.id).is_some_and(|p| cut.contains(&p));
                let lost = i.epic.as_deref().is_some_and(|e| cut.contains(&e));
                let unblocked = i.blocked_by.iter().any(|b| cut.contains(&b.as_str()));
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
    /// 원문의 짧은 해시([`fingerprint`]). `--yes --match` 에 그대로 준다. 지웠으면 지우기 전에 잰 값이다.
    #[serde(rename = "match")]
    matches: String,
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
/// **`--yes` 는 보여 준 원문에 묶인다**(moai-6nha, 2026-09-29 사용자 결정). 줄이 늘거나 줄면 번호가
/// 움직이고 — 앞의 `--line` 한 번도 그렇다 — 번호만 보고 지우면 보여 준 것과 다른 못 읽는 줄이 간다.
/// 그래서 보여 줄 때 원문의 짧은 해시와 그대로 칠 명령을 내고, `--yes` 는 그 해시(`--match`)가 있어야
/// 하며, 락 안에서 다시 읽은 그 번호의 원문이 해시와 다르면 거절한다. 지운 뒤에도 원문을 찍는다 —
/// 저널 쓰기가 실패했거나 64KB 에서 잘린 줄은 그 화면 말고는 온전히 남는 곳이 없다.
///
/// **해시가 다르면 보여 준 줄이 지금 선 번호를 댄다**(리뷰 moai-mo9v.w12). 그 번호에 지금 선 줄의 해시를
/// 대거나 같은 번호를 다시 보라고 하면 거기 선 딴 줄을 지울 인자가 다 나와, 한 번 다시 치는 것으로 아무도
/// 안 본 줄이 갔다(재 봤다). 거절문이 지우는 명령을 안 대는 것과 같은 까닭이다.
///
/// `want` 는 `--yes` 가 없으면 `None`, 있으면 `--match` 로 받은 해시다 — 꼴은 [`run`] 이 이미 쟀다.
fn line(ctx: &Ctx, repo: &crate::store::Repo, n: usize, want: Option<&str>) -> R<Vec<String>> {
    use crate::store::LoadError;
    let lang = ctx.lang();
    // 번호에 id 를 곁들여야 산 줄의 깨진 쌍둥이를 골라낸다. 파일에서 온 글이다 — `id::in_value` 는
    // 제어문자를 안 거른다.
    let named = |e: &LoadError| match e.id.as_deref() {
        Some(id) => format!("{} ({})", e.line, crate::text::one_line(id)),
        None => e.line.to_string(),
    };
    let listed = |v: Vec<String>| if v.is_empty() { "-".to_string() } else { v.join(", ") };
    // 거절문은 그 순간 읽은 못 읽는 줄로 짓는다.
    let refuse = |errors: &[LoadError]| -> Fail {
        let lines = listed(errors.iter().map(named).collect());
        Fail::coded(
            crate::i18n::fill(
                crate::i18n::say(lang, "refuse.line_not_unreadable"),
                &[("line", &n.to_string()), ("lines", &lines)],
            ),
            super::code::NOT_FOUND,
        )
    };

    let (e, matches) = if let Some(want) = want {
        let at = model::now();
        let by = model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, lang))?;
        repo.with_write_lines(
            || ctx.lang(),
            |_, unread, _, _| {
                // 락 안에서 다시 읽은 줄로 찾고 거절문도 그것으로 짓는다 — 밖에서 본 번호는 낡았을 수 있다.
                let Some(k) = unread.iter().position(|e| e.line == n) else { return Err(refuse(unread)) };
                // 해시는 대소문자를 안 가린다 — 손으로 옮겨 친 값이 대문자여도 같은 줄이다.
                let shown = |e: &LoadError| fingerprint(e, unread).eq_ignore_ascii_case(want);
                if !shown(&unread[k]) {
                    let went = listed(unread.iter().filter(|e| shown(e)).map(named).collect());
                    return Err(Fail::coded(
                        crate::i18n::fill(
                            crate::i18n::say(lang, "refuse.line_does_not_match"),
                            &[("line", &n.to_string()), ("match", want), ("went", &went)],
                        ),
                        super::code::NOT_FOUND,
                    ));
                }
                let matches = fingerprint(&unread[k], unread);
                let e = unread.remove(k);
                Ok((vec![JournalEntry::removed_line(e.id.as_deref(), &e.text, &at, &by)], (e, matches)))
            },
        )?
    } else {
        let mut errors = repo.read()?.errors;
        let Some(k) = errors.iter().position(|e| e.line == n) else { return Err(refuse(&errors)) };
        let matches = fingerprint(&errors[k], &errors);
        (errors.swap_remove(k), matches)
    };
    let dry_run = want.is_none();
    let l = Line { line: n, id: e.id.as_deref(), text: &e.text, matches, dry_run };
    if ctx.json {
        return super::json_line(&l);
    }
    // **원문은 두 경우 다 찍는다**(리뷰 moai-mo9v.1ln). 지운 뒤에 안 찍으면 번호가 다른 줄에 떨어진 것을
    // 아무도 못 보고, 저널 쓰기가 실패했거나 64KB 에서 잘린 줄은 어디에도 온전히 안 남는다. 파일에서 온
    // 글이라 제어문자를 걷어 그린다(`text::one_line`) — 바이트 그대로는 `--json` 의 `text` 에 있다.
    let mut out = vec![head(ctx, &l), format!("  {}", crate::text::one_line(&e.text))];
    if dry_run {
        let cmd = to_type(ctx, repo, &format!("rm --line {n} --yes --match {}", l.matches));
        out.push(crate::i18n::fill(crate::i18n::say(lang, "rm.line_ask"), &[("cmd", &cmd)]));
    } else if e.text.len() > model::MAX_TEXT_BYTES {
        // 저널의 `rm` 줄은 `model::fit_bytes` 가 자른 앞머리만 든다(`JournalEntry::removed_line`).
        out.push(crate::i18n::say(lang, "rm.line_cut").to_string());
    }
    Ok(out)
}

/// 못 읽는 줄 원문의 짧은 해시 — `--yes` 를 보여 준 줄에 묶는 `--match` 의 값(moai-6nha).
///
/// FNV-1a 32비트([`crate::text::fnv1a32`])를 16진 8자로 낸다. 적대적 입력을 막는 자리가 아니라 번호가
/// 밀려 **다른** 못 읽는 줄에 떨어진 것을 가르는 자리라, 충돌에 강할 까닭이 없다. 바이트는 파일의
/// 원문 그대로다 — 화면에 그린 글(`text::one_line`)로 세면 제어문자만 다른 두 줄이 같은 값이 된다.
///
/// 원문 말고 두 가지를 더 잰다(리뷰 moai-mo9v.w12).
///
/// - **같은 바이트의 못 읽는 줄이 몇 벌인가**(`all` 에서 센다). 바이트만 재던 동안은 똑같은 줄 두 벌 가운데
///   하나를 지운 뒤 같은 명령을 다시 치면, 그 번호로 밀려 들어온 둘째 벌이 같은 해시로 말없이 갔다 — 새
///   moai 가 쓴 줄이 손으로 푼 머지에 두 벌 남은 경우 한 벌은 남겨야 하는 줄이다. 벌 수가 해시에 들면 첫
///   지우기가 그 수를 바꿔, 다시 친 명령은 거절된다
/// - **꼬리의 `\r` 하나는 줄 끝 표시로 뗀다** — `merge_driver::keyed` 와 같은 자다. `str::lines` 는 `\n` 앞의
///   `\r` 만 떼므로 끝 줄바꿈 없이 `\r` 로 끝나는 파일의 마지막 줄만 그것을 달고 서는데, 다음 쓰기가 그 줄을
///   `…\r\n` 으로 되써 `\r` 이 떨어지면 아무도 안 건드린 줄의 해시가 바뀌어 `--yes` 가 거절됐다
fn fingerprint(e: &crate::store::LoadError, all: &[crate::store::LoadError]) -> String {
    fn body(t: &str) -> &str {
        t.strip_suffix('\r').unwrap_or(t)
    }
    let copies = all.iter().filter(|o| body(&o.text) == body(&e.text)).count();
    format!("{:08x}", crate::text::fnv1a32(format!("{copies}\n{}", body(&e.text)).as_bytes()))
}

/// `--match` 로 받는 꼴 — [`fingerprint`] 가 내는 16진 8자다.
fn is_hash(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// 따라 칠 명령 — **이 부름이 연 트래커로 가는 꼴로** 짓는다(리뷰 moai-mo9v.w12).
///
/// `-C` 로 옮겨 왔거나 체크아웃 뿌리 밖에서 불렀으면 `-C <그 체크아웃>` 을, `MOAI_HERE=1` 로 워크트리의 제
/// 트래커를 열었으면 그 손잡이를 붙인다. 빼고 대면 따라 친 명령이 딴 트래커를 여는데, 워크트리가 갈라진
/// 직후처럼 그 트래커의 같은 번호에 같은 바이트가 서 있으면 해시까지 맞아 보여 준 적 없는 파일의 줄이
/// 갔다(재 봤다 — 워크트리에서 본 줄을 루트의 트래커에서 지웠다). 뿌리는 [`crate::store::Repo::here`] 로
/// 잰다 — 루트의 트래커로 옮겨 온 워크트리에서는 맨 명령도 같은 곳으로 옮겨 간다. `-C` 를 붙이는 규칙은
/// [`crate::report::Warning::cli_hint`] 한 자리다.
fn to_type(ctx: &Ctx, repo: &crate::store::Repo, tail: &str) -> String {
    let at = crate::cmd::init::away_root(repo.here(), ctx.chdir);
    let cmd = crate::report::Warning::cli_hint(at.as_deref(), tail);
    match crate::store::here_wanted() {
        true => format!("MOAI_HERE=1 {cmd}"),
        false => cmd,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::LoadError;

    fn unread(text: &str) -> LoadError {
        LoadError { line: 1, message: String::new(), text: text.to_string(), id: None }
    }

    /// 한 벌뿐인 줄의 해시.
    fn alone(text: &str) -> String {
        let e = unread(text);
        fingerprint(&e, std::slice::from_ref(&e))
    }

    /// **미리 보기가 낸 해시는 `--match` 가 받는 꼴이다** — `--yes` 가 그 값을 제 손으로 거절하면 보여 준
    /// 명령이 영영 안 돈다. 빈 값과 오타는 해시가 아니다(리뷰 moai-mo9v.w12).
    #[test]
    fn the_hash_the_preview_prints_is_one_match_takes() {
        assert!(is_hash(&alone("쓰레기")));
        assert!(is_hash("1A2B3C4D"), "대문자로 옮겨 친 해시를 안 받는다");
        for bad in ["", " ", "1a2b3c4", "1a2b3c4d5", "zzzzzzzz", "0x1a2b3c", "1a2b 3c4"] {
            assert!(!is_hash(bad), "{bad:?} 를 해시로 받았다");
        }
    }

    /// **다음 쓰기가 떼는 꼬리 `\r` 은 해시를 안 바꾼다**(리뷰 moai-mo9v.w12). `\r` 로 끝나고 끝 줄바꿈이 없는
    /// 파일의 마지막 줄은 그것을 달고 읽히는데, 쓰기 한 번이 그 줄을 `…\r\n` 으로 되써 다음 읽기에서 `\r` 이
    /// 떨어진다 — 아무도 안 건드린 줄이다.
    #[test]
    fn a_trailing_carriage_return_does_not_move_the_hash() {
        assert_eq!(alone("쓰레기\r"), alone("쓰레기"));
        assert_ne!(alone("쓰레기 "), alone("쓰레기"), "꼬리 빈칸까지 뗐다 — 뗄 것은 줄 끝 표시 하나다");
    }

    /// **같은 바이트의 줄이 몇 벌인지가 해시에 든다**(리뷰 moai-mo9v.w12). 두 벌 가운데 하나를 지우면 남은
    /// 한 벌의 해시가 바뀌어, 같은 명령을 다시 쳐도 그 벌은 안 간다.
    #[test]
    fn removing_one_of_two_copies_changes_the_hash_of_the_other() {
        let two = [unread("쓰레기"), unread("쓰레기")];
        assert_eq!(fingerprint(&two[0], &two), fingerprint(&two[1], &two), "같은 두 벌이 다른 해시를 냈다");
        assert_ne!(fingerprint(&two[1], &two), alone("쓰레기"), "벌 수가 해시에 안 들었다");
    }
}
