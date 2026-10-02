//! 시간대 고르는 창 — `SPC o t`(moai-3oz2).
//!
//! **위쪽의 [`Zones`] 는 조각이다.** `App` 도 터미널도 모른다. 이름 목록은 든 쪽이
//! [`crate::tz::names`] 로 읽어 넘기고, 고른 이름을 실제로 여는 것도([`crate::tz::Zone::load`])
//! 아래 "든 쪽" 의 `impl App` 이 한다 — 그래서 이 파일은 조각 목록(`input::NOT_COMPONENTS`)에 든다.
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
    /// 거른 결과 — [`Zones::all`] 의 첨자다. **글이 바뀔 때만 다시 센다**([`Zones::settle`]).
    /// 부를 때마다 거르던 판은 이름 1,200개를 글자 하나마다 여러 벌씩 소문자로 다시 지었다 —
    /// 그림도 커서도 한 걸음에 저마다 물어 본다(리뷰).
    hits: Vec<usize>,
    /// 지금 쓰고 있는 이름 — 줄 곁에 `지금` 이 선다.
    now: String,
    /// 거르는 글.
    pub typing: Input,
    /// 거른 목록에서의 커서.
    pub cursor: usize,
    /// 커서가 **선 줄을 가리키는가**(moai-pvpb.1yh). 지금 쓰는 이름이 목록에 없으면(`TZ=JST-9` 같은 규칙
    /// 글 — 목록은 tzdb 이름만 든다, 2026-10-01 사용자 결정) 연 자리에서 아무 줄도 안 가리킨다. 사람이
    /// 옮기거나 글을 쳐야 선다 — 그 전의 Enter 는 아무것도 안 고른다. 맨 위에 세워 두던 판은 글자를
    /// 안 친 Enter 가 첫 이름(`Africa/Abidjan`)을 골라 설정에 적었다.
    aimed: bool,
    /// 굴린 자리 — 탐색기의 목록과 같은 조각이다.
    pub list: Scroll,
    /// 목록이 빈 까닭 — tzdb 가 없다([`crate::tz::Trouble`], moai-77ap). **빈 목록만 내지
    /// 않는다**: 까닭 없이 비면 고르는 창이 "시간대가 없다" 로 서서 무엇이 잘못됐는지 아무
    /// 데도 안 적힌다.
    pub trouble: Option<crate::tz::Trouble>,
}

impl Zones {
    /// 연다. 커서는 **지금 쓰는 이름**에 선다 — 고르러 들어온 사람이 먼저 보는 것은 지금이다. 그 이름이
    /// 목록에 없으면 아무 줄도 안 가리킨다([`Zones::aimed`]).
    pub fn open(all: Vec<String>, trouble: Option<crate::tz::Trouble>, now: &str) -> Zones {
        let mut z = Zones {
            all,
            hits: Vec::new(),
            now: now.to_string(),
            typing: Input::default(),
            cursor: 0,
            aimed: false,
            list: Scroll::default(),
            trouble,
        };
        z.settle();
        if let Some(at) = z.shown().iter().position(|n| *n == now) {
            (z.cursor, z.aimed) = (at, true);
        }
        z
    }

    /// 커서가 가리키는 줄의 자리 — 아무 줄도 안 가리키거나 목록이 비었으면 없다. 그리는 쪽과 고르는 쪽
    /// ([`Zones::at`])이 이것 하나로 묻는다 — 따로 재면 화면에 칠해진 줄과 Enter 가 고르는 줄이 갈린다.
    pub fn selected(&self) -> Option<usize> {
        (self.aimed && !self.hits.is_empty()).then(|| self.cursor.min(self.hits.len() - 1))
    }

    /// 지금 쓰는 이름이 목록에 없어 커서가 아직 아무 줄도 안 가리키는가 — 창 밑에 그렇다고 한 줄 선다.
    /// 안 가리키는 커서는 연 자리에서만 생기므로(`open`) 그 까닭은 하나다. 목록이 통째로 비었으면 그쪽
    /// 까닭(`trouble`)이 선다.
    pub fn unlisted(&self) -> bool {
        !self.aimed && !self.all.is_empty()
    }

    /// 거른 목록. **대소문자를 안 가린다** — 이름은 `Asia/Seoul` 인데 손은 `asia` 를 친다.
    /// 거르는 일 자체는 [`Zones::settle`] 이 글이 바뀔 때만 한다.
    pub fn shown(&self) -> Vec<&str> {
        self.hits.iter().map(|n| self.all[*n].as_str()).collect()
    }

    /// 커서가 선 이름. 목록이 비었거나 아무 줄도 안 가리키면 없다 — 그때 Enter 는 아무것도 안 고른다.
    pub fn at(&self) -> Option<&str> {
        let n = self.hits.get(self.selected()?)?;
        Some(self.all[*n].as_str())
    }

    /// 이 이름이 지금 쓰는 것인가 — 줄 곁에 낱말을 단다.
    pub fn is_now(&self, name: &str) -> bool {
        name == self.now
    }

    /// 지금 쓰는 이름 — 목록에 없을 때 창 밑의 한 줄이 댄다([`Zones::unlisted`]).
    pub fn now(&self) -> &str {
        &self.now
    }

    /// 커서를 옮긴다. **끝에서 멈춘다** — 목록이 길어 돌아 나오면 어디에 있었는지를 잃는다.
    ///
    /// **걸음은 탐색기와 같은 자가 센다**([`super::scroll::cursor`], 리뷰) — 한 쪽은 칸 높이가
    /// 아니라 고정 걸음이다(`scroll::PAGE`). 창마다 다르게 세면 같은 키가 칸마다 다른 만큼
    /// 움직이는데, 그것을 안 하기로 한 까닭이 `scroll` 의 머리글에 적혀 있다.
    ///
    /// **아무 줄도 안 가리키던 커서는 첫걸음에 선다**([`Zones::aimed`]) — 목록 위의 빈자리에서 내려오듯이,
    /// 아래로 한 줄·위로·맨 위는 첫 줄에 서고, 맨 아래와 한 쪽 아래는 첫 줄에서 그만큼 간다.
    pub fn step(&mut self, m: Move) {
        let at = match (self.aimed, m) {
            (false, Move::LineDown | Move::LineUp | Move::Top | Move::PageUp) => 0,
            _ => super::scroll::cursor(m, self.cursor, || self.hits.len()),
        };
        self.cursor = at.min(self.hits.len().saturating_sub(1));
        self.aimed = true;
    }

    /// 거르는 글에 키 하나를 먹인다 — 먹었으면 `true`. **먹은 자리에서 곧바로 다시 거른다**
    /// ([`Zones::settle`]): 두 걸음으로 두면 부르는 쪽이 하나를 빠뜨리는 날 Enter 가 화면에 서 있지도
    /// 않은 줄을 고른다(리뷰). **글을 친 것은 고르러 나선 것이다** — 아무 줄도 안 가리키던 커서가 선다.
    pub fn key(&mut self, k: KeyEvent) -> bool {
        let ate = self.typing.key(k);
        if ate {
            self.aimed = true;
            self.settle();
        }
        ate
    }

    /// 거르는 글에 붙여 넣는다 — [`Zones::key`] 와 같은 약속이다.
    pub fn paste(&mut self, s: &str) {
        self.typing.paste(s);
        self.aimed = true;
        self.settle();
    }

    /// 글이 바뀌면 다시 거르고 커서를 목록 안으로 도로 들인다. **[`Zones::key`]·[`Zones::paste`]
    /// 가 제 손으로 부른다** — 안 부르면 좁힌 목록 밖에 커서가 남아 Enter 가 엉뚱한 줄을 고른다.
    pub fn settle(&mut self) {
        let q = self.typing.text().to_lowercase();
        self.hits = (0..self.all.len()).filter(|n| q.is_empty() || self.all[*n].to_lowercase().contains(&q)).collect();
        self.cursor = self.cursor.min(self.hits.len().saturating_sub(1));
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
        let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let step = match k.code {
            KeyCode::Down if plain => Some(super::scroll::Move::LineDown),
            KeyCode::Up if plain => Some(super::scroll::Move::LineUp),
            KeyCode::PageDown if plain => Some(super::scroll::Move::PageDown),
            KeyCode::PageUp if plain => Some(super::scroll::Move::PageUp),
            _ => None,
        };
        if let Some(m) = step {
            z.step(m);
            return;
        }
        // 글이 바뀌면 창이 제 손으로 다시 거르고 커서를 안으로 들인다([`Zones::key`]) — 안 들이면
        // Enter 가 화면에 서 있지도 않은 줄을 고른다.
        if z.key(k) {
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
            // **성했다고 알림을 지우지 않는다**(리뷰). `App::notice` 는 한 자리를 나눠 쓰는데,
            // 여기서 비우면 못 읽은 옆 스냅샷처럼 이 일과 무관한 배너가 시간대를 한 번 고른 것만으로
            // 사라진다. 앞선 시간대 알림은 [`App::key`] 가 키마다 걷으므로 여기서 비울 까닭도 없다.
            Ok(z) => self.zone = z,
            Err(why) => {
                self.zone = crate::tz::Zone::utc();
                self.notice = Some(crate::view::zone_trouble(self.site.lang, &why));
            }
        }
        // **걸어 둔 거름망은 이 시간대로 다시 건다**(리뷰 moai-efoc.ln9) — 날로 친 때(`created=2026-10-01`)는
        // 화면의 시간대로 재어 `keep` 에 접히므로([`super::App::apply`]), 안 걸면 상세는 새 시간대의 날을 그리는데
        // 목록은 다음 다시 읽기까지 옛 시간대의 날로 남는다. 커서는 다시 읽을 때처럼 붙든 줄에 다시 세운다.
        if self.hung.is_some() {
            let held = self.current().map(|r| self.anchor_of(&r));
            self.reapply();
            self.regrip(held);
        }
        // **수는 다시 세지 않는다**(moai-fgjj, 2026-09-23 사용자 결정) — 배너와 층의 수에 기한 판정이
        // 안 접혀 있어([`super::Surfaced`]) 그리는 걸음이 이 시간대로 잰다. 한때 여기서 셈을 다시
        // 돌렸고(그때 이름은 `recount`, 지금 [`super::App::count_all`])(리뷰), 그것으로도 프로젝트
        // 층의 줄과 도는 읽기는 못 고쳐 이 이슈가 섰다.
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
    }

    /// **지금 이름이 목록에 없으면 커서는 아무 줄도 안 가리킨다**(moai-pvpb.1yh) — `TZ=JST-9` 같은 규칙 글이다.
    /// 그때 Enter 는 아무것도 안 고른다(`at` 이 `None`). 맨 위에 세워 두던 판은 글자 없는 Enter 가 첫 이름을
    /// 골라 설정에 적었다. 옮기거나 글을 치면 선다 — 아래로 한 줄은 첫 줄이다.
    #[test]
    fn an_unlisted_zone_leaves_the_cursor_on_no_row() {
        let all = ["Africa/Abidjan", "Asia/Seoul", "UTC"].map(String::from).to_vec();
        let z = Zones::open(all.clone(), None, "JST-9");
        assert_eq!((z.at(), z.selected()), (None, None), "목록에 없는 이름에서 줄을 골라 두었다");
        assert!(z.unlisted());

        let mut moved = z.clone();
        moved.step(Move::LineDown);
        assert_eq!(moved.at(), Some("Africa/Abidjan"), "첫걸음이 첫 줄에 안 섰다");
        assert!(!moved.unlisted());
        let mut bottom = z.clone();
        bottom.step(Move::Bottom);
        assert_eq!(bottom.at(), Some("UTC"));

        let mut typed = z.clone();
        assert!(typed.key(KeyEvent::from(KeyCode::Char('s'))));
        assert_eq!(typed.at(), Some("Asia/Seoul"), "글을 쳤는데 커서가 안 섰다");
        let mut pasted = z;
        pasted.paste("utc");
        assert_eq!(pasted.at(), Some("UTC"));

        // 목록에 있는 이름에서 열면 그 줄이다 — 가르기 전과 같다.
        assert_eq!(Zones::open(all, None, "Asia/Seoul").at(), Some("Asia/Seoul"));
    }

    /// **대소문자를 안 가린다** — 이름은 `Asia/Seoul` 인데 손은 `asia` 를 친다.
    #[test]
    fn the_filter_ignores_case_and_matches_anywhere() {
        let mut z = zones();
        z.typing = Input::new("seo");
        z.settle();
        assert_eq!(z.shown(), ["Asia/Seoul"]);
        z.typing = Input::new("ASIA");
        z.settle();
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
    ///
    /// 걸음은 탐색기와 **같은 자**가 센다(`scroll::cursor`) — 한 쪽은 칸 높이가 아니라 고정
    /// 걸음이라, 짧은 목록에서는 한 번에 끝까지 간다.
    #[test]
    fn stepping_stops_at_both_ends() {
        let mut z = zones();
        z.step(Move::Top);
        assert_eq!(z.cursor, 0);
        z.step(Move::LineUp);
        assert_eq!(z.cursor, 0, "맨 위에서 위로 갔다");
        z.step(Move::Bottom);
        assert_eq!(z.cursor, 3);
        z.step(Move::LineDown);
        assert_eq!(z.cursor, 3, "맨 아래에서 아래로 갔다");
        z.step(Move::PageUp);
        assert_eq!(z.cursor, 0, "한 쪽 위가 목록 밖으로 나갔다");
        z.step(Move::LineDown);
        assert_eq!(z.cursor, 1);

        // 빈 목록에서도 걸음이 넘어가지 않는다.
        let mut empty = Zones::open(Vec::new(), None, "UTC");
        empty.step(Move::Bottom);
        assert_eq!((empty.cursor, empty.at()), (0, None));
        empty.step(Move::PageDown);
        assert_eq!((empty.cursor, empty.at()), (0, None));
    }
}
