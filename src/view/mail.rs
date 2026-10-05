//! 편지의 사람 화면(moai-h8tn) — `moai inbox` 와 훅이 맥락에 싣는 글이 **이 한 자리**에서 선다. 둘이 따로
//! 그리면 같은 편지가 화면과 세션에서 다른 꼴로 선다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! **남이 쓴 글이라 제어문자를 걷는다** — 머리와 제목은 `text::one_line`, 본문은 줄을 남기는 `text::sanitize`
//! 다. 터미널을 움직이는 바이트가 그대로 나가면 받는 사람의 화면을 보낸 사람이 그린다.

use crate::i18n::{Lang, fill, say};
use crate::mail::Stored;
use crate::style::{self, paint};
use crate::text::{one_line, sanitize};

/// 편지 하나 — 머리 줄(id·보낸 이·때, 읽었으면 그 표), 제목, 답하는 편지, 네 칸 들여 쓴 본문.
///
/// **되돌아온 편지는 머리 줄이 다르다**(moai-ew4o.l3n) — 보낸 이가 나라 "누가 보냈다" 는 뜻이 없고, 읽을 것은 "그 이가
/// 읽기 전에 떠났다" 다. 그 일을 다시 맡길지는 받은 쪽이 정한다.
///
/// **때는 보는 사람의 시간대로 적는다**(`view::stamp`, moai-p5az) — 다른 화면이 모두 그렇게 적는데 편지만 UTC 글자를
/// 그대로 내던 판은, 같은 화면(훅이 싣는 보드와 편지)에서 몇 시간 어긋난 시각이 나란히 섰다(리뷰 moai-h8tn.x4l).
pub fn letter(lang: Lang, zone: &crate::tz::Zone, s: &Stored) -> Vec<String> {
    let l = &s.letter;
    // 키는 `say` 에 글자째 적는다 — 소스가 부르는 키를 i18n 시험이 그 글자로 센다.
    let (said, who) = if s.returned {
        (say(lang, "mail.returned_head"), ("to", &l.to))
    } else {
        (say(lang, "mail.letter_head"), ("from", &l.from))
    };
    let mut head = fill(
        said,
        &[("id", &paint(style::ID, &s.id)), (who.0, &one_line(who.1)), ("at", &crate::view::stamp(&l.sent_at, zone))],
    );
    if s.reader.is_some() {
        head.push_str(&format!("  {}", paint(style::DIM, say(lang, "mail.read_mark"))));
    }
    let mut out = vec![head, format!("  {}", one_line(&l.subject))];
    if let Some(r) = l.reply_to.as_deref() {
        out.push(format!("  {}", paint(style::DIM, &fill(say(lang, "mail.in_reply"), &[("id", &one_line(r))]))));
    }
    out.extend(sanitize(&l.body).lines().map(|line| format!("    {line}")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::{Letter, VERSION};

    /// 본문의 제어문자는 걷히고 줄은 남는다 — 보낸 이가 받는 화면을 못 움직인다.
    #[test]
    fn a_letter_cannot_drive_the_terminal() {
        let s = Stored {
            id: "20261004-061203-00000001".into(),
            mailbox: "b".into(),
            reader: None,
            returned: false,
            letter: Letter {
                v: VERSION,
                to: "b".into(),
                from: "a\u{1b}[2J".into(),
                subject: "hi\u{1b}]0;x\u{7}".into(),
                body: "one\n\u{1b}[31mtwo".into(),
                sent_at: "2026-10-04T06:12:03Z".into(),
                reply_to: None,
                rest: Default::default(),
            },
        };
        let lines = crate::style::plain(&letter(Lang::En, &crate::tz::Zone::utc(), &s).join("\n"));
        assert!(!lines.contains('\u{1b}') && !lines.contains('\u{7}'), "{lines:?}");
        assert!(lines.contains("    one\n    ") && lines.contains("two"), "{lines:?}");
    }
}
