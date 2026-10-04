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
pub fn letter(lang: Lang, s: &Stored) -> Vec<String> {
    let l = &s.letter;
    let mut head = fill(
        say(lang, "mail.letter_head"),
        &[("id", &paint(style::ID, &s.id)), ("from", &one_line(&l.from)), ("at", &one_line(&l.sent_at))],
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
            reader: None,
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
        let lines = crate::style::plain(&letter(Lang::En, &s).join("\n"));
        assert!(!lines.contains('\u{1b}') && !lines.contains('\u{7}'), "{lines:?}");
        assert!(lines.contains("    one\n    ") && lines.contains("two"), "{lines:?}");
    }
}
