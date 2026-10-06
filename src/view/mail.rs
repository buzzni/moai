//! 편지의 사람 화면(moai-h8tn) — `moai inbox` 와 훅이 맥락에 싣는 글이 **이 한 자리**에서 선다. 둘이 따로
//! 그리면 같은 편지가 화면과 세션에서 다른 꼴로 선다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! **남이 쓴 글이라 제어문자를 걷는다** — 머리와 제목은 `text::one_line`, 본문은 줄을 남기는 `text::sanitize`
//! 다. 터미널을 움직이는 바이트가 그대로 나가면 받는 사람의 화면을 보낸 사람이 그린다.

use crate::i18n::{Lang, fill, say};
use crate::mail::Stored;
use crate::style::{self, paint};
use crate::text::{one_line, sanitize};

/// 편지 본문을 글자 자리로 센 것(moai-m81b) — 넘겨 보는 자리(`moai inbox <id> --from <n>`)와 훅의 자른 표가 **이
/// 하나**로 센다. 따로 세면 자른 표가 댄 자리에서 `inbox` 가 다른 글자를 낸다.
///
/// 자리는 **보이는 본문**(제어문자를 걷은 것, [`sanitize`])의 0 부터 센 글자다(2026-10-05 사용자 결정). 줄 번호로 세면
/// 줄바꿈 없는 긴 편지에서 한 줄이 출력 상한을 넘어 다시 잘린다. 바이트로 세면 자리가 글자 가운데에 설 수 있고, 에이전트
/// 마다 다른 자(UTF-16·UTF-8)로 세면 같은 편지의 자리가 둘이 된다.
pub struct Body {
    text: String,
    /// 글자마다 그 글자가 시작하는 바이트, 끝에 글의 길이 하나를 더 — 글자 자리를 바이트로 바꾸는 표다.
    at: Vec<usize>,
}

impl Body {
    pub fn of(s: &Stored) -> Body {
        let text = sanitize(&s.letter.body);
        let at = text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).collect();
        Body { text, at }
    }

    /// 글자 수.
    pub fn len(&self) -> usize {
        self.at.len() - 1
    }

    /// `from`..`to` 글자 — 끝을 넘는 자리는 끝으로 접는다.
    fn part(&self, from: usize, to: usize) -> &str {
        let to = to.min(self.len());
        &self.text[self.at[from.min(to)]..self.at[to]]
    }

    /// `from` 부터 `fits` 가 받는 가장 먼 끝 — `from` 조차 안 받으면 `None` 이다. **반씩 나눠 찾는다** — 그린 글은 끝이
    /// 멀수록 길어지기만 한다. 글자마다 다시 그리면 64KB 편지에서 6만 번이다.
    ///
    /// **줄 사이에서 끊으면 그 줄바꿈까지 이쪽이 가진다** — 끝에 선 줄바꿈 하나는 그린 글을 안 늘려(`lines` 가 끝의 빈
    /// 줄을 안 낸다) 가장 먼 끝이 늘 그 뒤다. 그래서 이어 보는 쪽이 빈 줄로 시작하는 것은 본문이 쓴 빈 줄뿐이고, 그 자리를
    /// 따로 건너뛰지 않는다 — 건너뛰면 끊은 자리 바로 뒤의 빈 줄 하나가 사라진다.
    pub fn reach(&self, from: usize, fits: impl Fn(usize) -> bool) -> Option<usize> {
        if from > self.len() || !fits(from) {
            return None;
        }
        let (mut lo, mut hi) = (from, self.len());
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        Some(lo)
    }
}

/// 편지 하나 — 머리 줄(id·보낸 이·때, 읽었으면 그 표), 제목, 답하는 편지, 네 칸 들여 쓴 본문.
///
/// **되돌아온 편지는 머리 줄이 다르다**(moai-ew4o.l3n) — 보낸 이가 나라 "누가 보냈다" 는 뜻이 없고, 읽을 것은 "그 이가
/// 읽기 전에 떠났다" 다. 그 일을 다시 맡길지는 받은 쪽이 정한다.
///
/// **때는 보는 사람의 시간대로 적는다**(`view::stamp`, moai-p5az) — 다른 화면이 모두 그렇게 적는데 편지만 UTC 글자를
/// 그대로 내던 판은, 같은 화면(훅이 싣는 보드와 편지)에서 몇 시간 어긋난 시각이 나란히 섰다(리뷰 moai-h8tn.x4l).
pub fn letter(lang: Lang, zone: &crate::tz::Zone, s: &Stored) -> Vec<String> {
    framed(lang, zone, s, &sanitize(&s.letter.body), None)
}

/// 편지 하나의 본문 `from`..`to` 글자(moai-m81b) — 머리 줄과 제목은 늘 선다. 처음부터가 아니면 어디서부터인지 한 줄을
/// 단다: 머리 줄만 보고는 앞에서 이어 본 쪽인지 모른다. `body` 는 `s` 의 것이다([`Body::of`]).
pub fn part(lang: Lang, zone: &crate::tz::Zone, s: &Stored, body: &Body, from: usize, to: usize) -> Vec<String> {
    framed(lang, zone, s, body.part(from, to), (from > 0).then(|| (from, body.len())))
}

/// [`letter`]·[`part`] 가 함께 그리는 꼴 — 머리 줄·제목·답하는 편지, 앞에서 이어 본 쪽이면 그 자리(`after` — 건넌 글자
/// 수와 본문의 글자 수), 네 칸 들여 쓴 본문 `text`. **통째로 그리는 편지는 글자 자리 표([`Body`])를 안 짓는다**(리뷰
/// moai-54yc.fay) — 훅은 기다리는 편지마다 이 글을 그려 재므로, 표를 지으면 글자마다 8 바이트를 쓰지도 않고 쌓는다.
fn framed(lang: Lang, zone: &crate::tz::Zone, s: &Stored, text: &str, after: Option<(usize, usize)>) -> Vec<String> {
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
    if let Some((from, len)) = after {
        let (at, len) = (from.to_string(), len.to_string());
        out.push(format!("  {}", paint(style::DIM, &fill(say(lang, "mail.page_from"), &[("at", &at), ("len", &len)]))));
    }
    out.extend(text.lines().map(|line| format!("    {line}")));
    out
}

/// 넘겨 보는 한 쪽(moai-m81b) — 본문 `from` 글자부터 `budget` 바이트 안에 드는 데까지. 남은 것이 있으면 이어 볼 명령을
/// 끝 줄로 댄다. 그 명령은 `--as <나>` 를 단다 — 훅의 자른 표와 같은 까닭이다(셸에서 제 장을 못 찾는 세션).
///
/// **끝 줄의 자리는 미리 떼어 둔다** — 가장 긴 수(본문의 글자 수)로 잰다. 끝 줄을 넣고 재면 마지막 쪽에서는 그 줄이
/// 빠져 재는 값이 줄어들어, 반씩 나눠 찾는 [`Body::reach`] 의 "멀수록 길다" 가 깨진다.
/// **한 쪽은 한 글자라도 나아간다** — 손으로 놓은 편지의 머리 줄이 혼자 `budget` 을 넘어도 같은 자리를 되풀이해 대지 않는다.
pub fn page(lang: Lang, zone: &crate::tz::Zone, s: &Stored, me: &str, from: usize, budget: usize) -> Vec<String> {
    let body = Body::of(s);
    let len = body.len();
    let more = |left: usize, at: usize| {
        let (left, at) = (left.to_string(), at.to_string());
        let said = fill(say(lang, "mail.page_more"), &[("left", &left), ("id", &s.id), ("at", &at), ("me", me)]);
        format!("  {}", paint(style::DIM, &said))
    };
    let size = |lines: &[String]| lines.iter().map(|l| l.len() + 1).sum::<usize>();
    let room = budget.saturating_sub(more(len, len).len() + 1);
    let fits = |to: usize| size(&part(lang, zone, s, &body, from, to)) <= room;
    let to = match body.reach(from, fits) {
        Some(to) if to > from => to,
        _ => from.saturating_add(1).min(len),
    };
    let mut out = part(lang, zone, s, &body, from, to);
    if to < len {
        out.push(more(len - to, to));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::{Letter, VERSION};

    fn stored(subject: &str, body: &str) -> Stored {
        Stored {
            id: "20261004-061203-00000001".into(),
            mailbox: "b".into(),
            reader: None,
            returned: false,
            letter: Letter {
                v: VERSION,
                to: "b".into(),
                from: "a\u{1b}[2J".into(),
                subject: subject.into(),
                body: body.into(),
                sent_at: "2026-10-04T06:12:03Z".into(),
                reply_to: None,
                rest: Default::default(),
            },
        }
    }

    /// 본문의 제어문자는 걷히고 줄은 남는다 — 보낸 이가 받는 화면을 못 움직인다.
    #[test]
    fn a_letter_cannot_drive_the_terminal() {
        let s = stored("hi\u{1b}]0;x\u{7}", "one\n\u{1b}[31mtwo");
        let lines = crate::style::plain(&letter(Lang::En, &crate::tz::Zone::utc(), &s).join("\n"));
        assert!(!lines.contains('\u{1b}') && !lines.contains('\u{7}'), "{lines:?}");
        assert!(lines.contains("    one\n    ") && lines.contains("two"), "{lines:?}");
    }

    /// **통째로 그린 편지는 처음부터 끝까지의 쪽과 같다**(리뷰 moai-54yc.fay) — 훅은 고를 때 통째로 그린 글로 재고 자를
    /// 때 쪽으로 그린 글로 재니, 둘이 갈리면 자른 자리가 어긋난다. 머리 줄이 갈리는 꼴(읽음 표·되돌아온 편지·답하는
    /// 편지)과 걷히는 제어문자·끝의 빈 줄을 함께 든다.
    #[test]
    fn a_whole_letter_is_one_part_from_the_start() {
        let utc = crate::tz::Zone::utc();
        let mut s = stored("subj", "one\r\n\u{1b}[31mtwo\n\n");
        for (reader, returned, reply_to) in [(None, false, None), (Some("w1"), true, Some("20261004-061203-00000009"))]
        {
            s.reader = reader.map(Into::into);
            s.returned = returned;
            s.letter.reply_to = reply_to.map(Into::into);
            let body = Body::of(&s);
            assert_eq!(letter(Lang::En, &utc, &s), part(Lang::En, &utc, &s, &body, 0, body.len()), "{s:?}");
        }
    }

    /// 끝 줄이 대는 자리를 따라 끝까지 넘겨 본 본문과 쪽 수 — 쪽마다 `budget` 안에 드는지 잰다.
    fn walk(s: &Stored, budget: usize) -> (String, usize) {
        let utc = crate::tz::Zone::utc();
        let shown = sanitize(&s.letter.body);
        let (mut from, mut got, mut pages) = (0, String::new(), 0);
        loop {
            let lines = page(Lang::En, &utc, s, "w1", from, budget);
            let size: usize = lines.iter().map(|l| l.len() + 1).sum();
            assert!(size <= budget, "쪽이 상한을 넘었다 — {size}");
            pages += 1;
            if from > 0 {
                let len = shown.chars().count().to_string();
                let said = fill(say(Lang::En, "mail.page_from"), &[("at", &from.to_string()), ("len", &len)]);
                assert_eq!(crate::style::plain(&lines[2]), format!("  {said}"), "이어 본 자리를 안 댔다");
            }
            let text: Vec<&str> = lines.iter().filter_map(|l| l.strip_prefix("    ")).collect();
            let text = text.join("\n");
            got.push_str(&text);
            let Some(next) = lines.last().and_then(|l| l.split("--from ").nth(1)) else { break };
            let next: usize = next.split(' ').next().unwrap().parse().unwrap();
            // 끝 줄은 그대로 치면 다음 쪽이 나오는 명령이다 — 셸에서 제 장을 못 찾는 세션도 치게 `--as` 를 단다.
            let named = format!("moai inbox {} --from {next} --as w1", s.id);
            assert!(lines.last().is_some_and(|l| l.contains(&named)), "끝 줄이 다음 쪽의 명령이 아니다 — {lines:?}");
            let to = from + text.chars().count();
            assert!(next == to || (next == to + 1 && shown.chars().nth(to) == Some('\n')), "{from}..{to} 뒤의 {next}");
            if next == to + 1 {
                got.push('\n');
            }
            from = next;
        }
        (got, pages)
    }

    /// **쪽을 이어 붙이면 본문 그대로다**(moai-m81b) — 끝 줄이 댄 자리에서 이어 보면 빠지거나 겹치는 글자가 없고, 쪽마다
    /// 상한 안에 든다. 이어 본 쪽은 앞에서 몇 글자를 건넜는지 댄다. 걷히는 제어문자(`\r`)가 들어 자리를 날 본문으로 세면
    /// 어긋난다.
    ///
    /// **끊는 자리가 빈 줄 바로 앞에 서도 그 빈 줄은 남는다** — 상한을 한 바이트씩 바꿔 가며 끊는 자리를 모든 곳에 세운다.
    /// 줄 사이에서 끊긴 자리의 줄바꿈을 한 번 더 건너뛰던 판은 그 뒤의 빈 줄 하나를 잃었다.
    #[test]
    fn pages_join_back_into_the_body() {
        let body: String =
            (0..600).map(|k| if k % 9 == 0 { "\n".to_string() } else { format!("줄 {k:03} abc 가나다\r\n") }).collect();
        let (got, pages) = walk(&stored("big", &body), 1_500);
        let shown = sanitize(&body);
        assert!(pages > 5, "쪽이 적다 — {pages}");
        // 본문 끝의 줄바꿈 하나는 어느 쪽에도 안 보인다 — 줄로 그리는 꼴(`lines`)이 늘 그랬다.
        assert_eq!(got, shown.strip_suffix('\n').unwrap_or(&shown), "이어 붙인 쪽이 본문과 다르다");
        let body = "aaaa\n\nbbbb\n\n\ncccc 가나\n\ndd\n".repeat(6);
        let s = stored("s", &body);
        for budget in 260..420 {
            let (got, _) = walk(&s, budget);
            assert_eq!(got, body.strip_suffix('\n').unwrap(), "상한 {budget} 에서 이어 붙인 쪽이 본문과 다르다");
        }
    }

    /// **한 쪽은 한 글자라도 나아간다**(moai-m81b) — 손으로 놓은 편지의 제목이 혼자 상한을 넘어도 같은 자리를 되풀이해
    /// 대지 않는다. 대면 `--from` 을 따라가는 에이전트가 같은 쪽을 끝없이 부른다.
    #[test]
    fn a_page_always_moves_on() {
        let utc = crate::tz::Zone::utc();
        let s = stored(&"t".repeat(5_000), &"b".repeat(100));
        let lines = page(Lang::En, &utc, &s, "w1", 10, 1_000);
        let next: usize =
            lines.last().unwrap().split("--from ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
        assert_eq!(next, 11, "자리가 안 나아갔다 — {lines:?}");
    }
}
