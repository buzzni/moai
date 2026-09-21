//! 시간대 고르는 창 — `SPC o t`(moai-3oz2).
//!
//! **조각이다.** `App` 도 터미널도 모른다. 이름 목록은 든 쪽이 [`crate::tz::names`] 로 읽어
//! 넘기고, 고른 이름을 실제로 여는 것도([`crate::tz::Zone::load`]) 든 쪽이 한다.
//!
//! **고르는 것은 이름뿐이다.** 여기서 시간대를 열지 않는 까닭은 목록이 천 줄이 넘어서다 —
//! 줄마다 TZif 를 파면 창을 여는 데 그만큼이 든다.
//!
//! **거르는 글이 늘 선다.** 이 기계의 tzdb 는 이름을 1,200개쯤 든다. 글자를 쳐서 좁히지
//! 못하면 `Asia/Seoul` 을 찾는 길이 `j` 를 수백 번 누르는 것뿐이다.

use super::input::Input;
use super::scroll::{Move, Scroll};

/// 창의 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zones {
    /// 이 기계가 아는 이름 전부, 이름순.
    all: Vec<String>,
    /// 지금 쓰고 있는 이름 — 줄 곁에 `지금` 이 선다.
    now: String,
    /// 거르는 글.
    pub typing: Input,
    /// 거른 목록에서의 커서.
    pub cursor: usize,
    /// 굴린 자리 — 탐색기의 목록과 같은 조각이다.
    pub list: Scroll,
    /// 목록이 빈 까닭 — tzdb 가 없다([`crate::tz::Trouble`], moai-77ap). **빈 목록만 내지
    /// 않는다**: 까닭 없이 비면 고르는 창이 "시간대가 없다" 로 서서 무엇이 잘못됐는지 아무
    /// 데도 안 적힌다.
    pub trouble: Option<crate::tz::Trouble>,
}

impl Zones {
    /// 연다. 커서는 **지금 쓰는 이름**에 선다 — 고르러 들어온 사람이 먼저 보는 것은 지금이다.
    pub fn open(all: Vec<String>, trouble: Option<crate::tz::Trouble>, now: &str) -> Zones {
        let mut z =
            Zones { all, now: now.to_string(), typing: Input::default(), cursor: 0, list: Scroll::default(), trouble };
        z.cursor = z.shown().iter().position(|n| *n == now).unwrap_or(0);
        z
    }

    /// 거른 목록. **대소문자를 안 가린다** — 이름은 `Asia/Seoul` 인데 손은 `asia` 를 친다.
    pub fn shown(&self) -> Vec<&str> {
        let q = self.typing.text().to_lowercase();
        self.all.iter().map(String::as_str).filter(|n| q.is_empty() || n.to_lowercase().contains(&q)).collect()
    }

    /// 커서가 선 이름. 목록이 비었으면 없다.
    pub fn at(&self) -> Option<&str> {
        let rows = self.shown();
        rows.get(self.cursor.min(rows.len().saturating_sub(1))).copied()
    }

    /// 이 이름이 지금 쓰는 것인가 — 줄 곁에 낱말을 단다.
    pub fn is_now(&self, name: &str) -> bool {
        name == self.now
    }

    /// 커서를 옮긴다. **끝에서 멈춘다** — 목록이 길어 돌아 나오면 어디에 있었는지를 잃는다.
    pub fn step(&mut self, m: Move, page: usize) {
        let n = self.shown().len();
        if n == 0 {
            self.cursor = 0;
            return;
        }
        let last = n - 1;
        let page = page.max(1);
        self.cursor = match m {
            Move::LineDown => (self.cursor + 1).min(last),
            Move::LineUp => self.cursor.saturating_sub(1),
            Move::PageDown => (self.cursor + page).min(last),
            Move::PageUp => self.cursor.saturating_sub(page),
            Move::Top => 0,
            Move::Bottom => last,
            // 반 쪽짜리 걸음도 목록 안에서만 돈다.
            Move::HalfDown => (self.cursor + page / 2).min(last),
            Move::HalfUp => self.cursor.saturating_sub(page / 2),
        }
        .min(last);
    }

    /// 글이 바뀌면 커서를 목록 안으로 도로 들인다. **글자를 칠 때마다 부른다** — 안 부르면
    /// 좁힌 목록 밖에 커서가 남아 Enter 가 엉뚱한 줄을 고른다.
    pub fn settle(&mut self) {
        let n = self.shown().len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
    }
}

// ── 든 쪽 ────────────────────────────────────────────────────────────
//
// 창의 상태와 규칙은 위의 조각이 들고, 여기는 그 창이 시킨 것을 `App` 에 한다 — `register` 가
// 창(`picker`)과 갈라선 것과 같은 꼴이다.

use super::keys::{Lookup, PROMPT, Prompt, lookup};
use super::{App, Mode};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    /// 시간대 창의 키 하나.
    ///
    /// **차례가 정해져 있다.** 먼저 목록을 옮기는 키(위·아래·PgUp/PgDn), 그다음 거르는 칸
    /// ([`super::input::Input`]), 마지막이 Enter·Esc다. 칸을 먼저 두면 `j`·`k` 가 글자로 찍혀
    /// 목록을 걸을 길이 없고, 목록을 먼저 두되 글자까지 먹으면 이름을 칠 수가 없다.
    ///
    /// 그래서 **걷는 키는 화살표와 PgUp/PgDn 뿐이다** — vi 의 `j`·`k` 는 여기서 글자다.
    pub(super) fn pick_zone(&mut self, k: KeyEvent) {
        let Mode::Zone(z) = &mut self.mode else { return };
        // 한 쪽은 창이 마지막으로 그려진 높이다 — 안 그렸으면 한 줄이다(걸음이 멈추지는 않는다).
        let page = z.list.page().max(1);
        let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let step = match k.code {
            KeyCode::Down if plain => Some(super::scroll::Move::LineDown),
            KeyCode::Up if plain => Some(super::scroll::Move::LineUp),
            KeyCode::PageDown if plain => Some(super::scroll::Move::PageDown),
            KeyCode::PageUp if plain => Some(super::scroll::Move::PageUp),
            _ => None,
        };
        if let Some(m) = step {
            z.step(m, page);
            return;
        }
        if z.typing.key(k) {
            // 글이 바뀌면 커서를 좁힌 목록 안으로 도로 들인다 — 안 들이면 Enter 가 화면에
            // 서 있지도 않은 줄을 고른다.
            z.settle();
            return;
        }
        match lookup(PROMPT, &[k]) {
            Lookup::Run(Prompt::Apply) => {
                let picked = z.at().map(str::to_string);
                self.mode = Mode::Browse;
                if let Some(name) = picked {
                    self.set_zone(&name);
                }
            }
            Lookup::Run(Prompt::Cancel) => self.mode = Mode::Browse,
            // Tab 은 여기 뜻이 없다 — 찾을 자리를 돌리는 검색 칸의 것이다.
            _ => {}
        }
    }

    /// 고른 이름으로 바꾸고 설정에 적는다.
    ///
    /// **못 푼 이름은 UTC 로 떨어지고 한 줄로 알린다**(moai-77ap) — 막지 않는다. 그래도 **고른
    /// 이름은 설정에 적는다**: 지금 기계에 그 자료가 없다고 사람이 고른 것을 지우면, zoneinfo 가
    /// 있는 기계로 옮겼을 때 그 설정이 사라져 있다. 읽기가 관대한 것과 같은 자다.
    pub(super) fn set_zone(&mut self, name: &str) {
        match crate::tz::Zone::load(name) {
            Ok(z) => {
                self.zone = z;
                self.notice = None;
            }
            Err(why) => {
                self.zone = crate::tz::Zone::utc();
                self.notice = Some(crate::view::zone_trouble(self.site.lang, &why));
            }
        }
        // 적는 것은 **고른 이름**이지 떨어진 UTC 가 아니다.
        self.saved_zone = Some(name.to_string());
        self.save_look();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zones() -> Zones {
        let all = ["Asia/Seoul", "Asia/Tokyo", "Europe/Paris", "UTC"].map(String::from).to_vec();
        Zones::open(all, None, "Asia/Tokyo")
    }

    /// **커서는 지금 쓰는 이름에 선다** — 고르러 들어온 사람이 먼저 보는 것은 지금이다.
    #[test]
    fn it_opens_on_the_zone_in_use() {
        let z = zones();
        assert_eq!(z.at(), Some("Asia/Tokyo"));
        assert!(z.is_now("Asia/Tokyo") && !z.is_now("UTC"));
        // 모르는 이름에서 열면 맨 위다 — 없는 줄에 커서를 둘 수는 없다.
        let z = Zones::open(vec!["UTC".into()], None, "Mars/Olympus");
        assert_eq!(z.at(), Some("UTC"));
    }

    /// **대소문자를 안 가린다** — 이름은 `Asia/Seoul` 인데 손은 `asia` 를 친다.
    #[test]
    fn the_filter_ignores_case_and_matches_anywhere() {
        let mut z = zones();
        z.typing = Input::new("seo");
        assert_eq!(z.shown(), ["Asia/Seoul"]);
        z.typing = Input::new("ASIA");
        assert_eq!(z.shown(), ["Asia/Seoul", "Asia/Tokyo"]);
    }

    /// **좁힌 목록 밖에 커서를 남기지 않는다.** 남기면 Enter 가 엉뚱한 줄을 고르고, 그 줄은
    /// 화면에 서 있지도 않다.
    #[test]
    fn the_cursor_comes_back_inside_when_the_filter_narrows() {
        let mut z = zones();
        z.cursor = 3;
        assert_eq!(z.at(), Some("UTC"));
        z.typing = Input::new("asia");
        z.settle();
        assert_eq!(z.cursor, 1);
        assert_eq!(z.at(), Some("Asia/Tokyo"));
        // 하나도 안 맞으면 고를 것이 없다 — Enter 가 아무 일도 안 하도록 `None` 이다.
        z.typing = Input::new("zzz");
        z.settle();
        assert_eq!(z.shown(), Vec::<&str>::new());
        assert_eq!(z.at(), None);
    }

    /// **끝에서 멈춘다** — 목록이 천 줄이 넘어, 돌아 나오면 어디에 있었는지를 잃는다.
    #[test]
    fn stepping_stops_at_both_ends() {
        let mut z = zones();
        z.step(Move::Top, 10);
        assert_eq!(z.cursor, 0);
        z.step(Move::LineUp, 10);
        assert_eq!(z.cursor, 0, "맨 위에서 위로 갔다");
        z.step(Move::Bottom, 10);
        assert_eq!(z.cursor, 3);
        z.step(Move::LineDown, 10);
        assert_eq!(z.cursor, 3, "맨 아래에서 아래로 갔다");
        z.step(Move::PageUp, 2);
        assert_eq!(z.cursor, 1);

        // 빈 목록에서도 걸음이 넘어가지 않는다.
        let mut empty = Zones::open(Vec::new(), None, "UTC");
        empty.step(Move::Bottom, 10);
        assert_eq!((empty.cursor, empty.at()), (0, None));
    }
}
