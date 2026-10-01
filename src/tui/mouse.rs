//! 마우스 — 누르기·휠·칸 끌기(moai-irrj).
//!
//! **키와 같은 길로 옮긴다.** 누른 칸으로 포커스가 가는 것은 `Tab` 과, 휠이 목록을 움직이는 것은 `j`·`k`
//! 와 같은 자리를 바꾼다 — 마우스만의 상태를 두면 키로 한 일과 마우스로 한 일이 따로 논다.
//!
//! **맞히는 바탕은 지난 프레임이 그린 자리다**([`Drawn`]). 루프는 사건 하나마다 한 번 그리므로 그 자리는
//! 한 걸음 넘게 낡지 않는다 — 키가 상세의 끝을 마지막으로 그린 줄 수로 재는 것(`Scroll::go`)과 같은 결이다.
//!
//! **듣는 자리는 둘뿐이다**(사용자 결정 2026-10-01). 목록·상세를 둘러보는 동안(한눈 보기 `0` 도 같은 칸이다)과
//! 통계 창 위의 휠이다. 폼·묻는 칸·고르는 창·지우기 확인·글을 받는 칸·SPC 메뉴가 떠 있으면 아무것도 안 한다 —
//! 그 창들은 키로 다루는 자리고, 뒤의 목록을 누른 것이 적던 글을 두고 커서를 옮기면 무엇에 대해 적던 것인지를 잃는다.

use super::{App, Mode, Pane, menu};
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

/// 휠 한 칸에 가는 줄 수. 대개의 터미널·편집기가 그만큼 간다. **목록과 상세가 같은 걸음이다** — 칸마다
/// 다르면 같은 손짓의 뜻이 마우스가 선 칸에 따라 바뀐다(`scroll::PAGE` 와 같은 까닭).
pub const WHEEL: isize = 3;

/// 지난 프레임이 그린 자리. **쓰는 곳은 `draw::screen` 하나다** — 여기는 읽기만 한다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Drawn {
    /// 목록과 상세가 나눠 쓰는 몸통. 통계 창·폼은 이 자리를 통째로 덮는다.
    pub body: Rect,
    /// 목록 칸 — 테두리까지.
    pub list: Rect,
    /// 목록의 줄이 서는 자리 — 테두리와 열 이름 줄을 뺀 안쪽. 이 자리의 n 번째 줄이 굴린 자리(`App::list`)에서
    /// n 만큼 내려간 줄이다.
    pub rows: Rect,
    /// 상세 칸 — 테두리까지. 숨겼거나 접혀 안 섰으면 없다(`draw::split_body`).
    pub detail: Option<Rect>,
}

impl App {
    /// 마우스 사건 하나. **루프가 받은 그대로 넘긴다**(`cmd::tui::take`).
    ///
    /// 놓은 뒤에 온 사건은 버린다 — 놓는 글이 터미널에 닿기 전에 이미 길에 있던 것이다.
    pub fn mouse(&mut self, m: MouseEvent) {
        if !self.mouse {
            return;
        }
        let at = Position::new(m.column, m.row);
        let wheel = match m.kind {
            MouseEventKind::ScrollUp => Some(-WHEEL),
            MouseEventKind::ScrollDown => Some(WHEEL),
            _ => None,
        };
        // **통계 창은 휠만 받는다**(사용자 결정 2026-10-01) — 목록·상세 자리를 통째로 덮은 창이라 그 위의 휠은
        // 창을 굴린다. 키(`j`·`k`)와 같은 굴린 자리를 옮기고, 기다리던 `g` 는 버린다.
        if let Mode::Stats(w) = &mut self.mode {
            if let Some(by) = wheel
                && self.drawn.body.contains(at)
            {
                self.notice = None;
                w.chord.clear();
                w.scroll.by(by);
            }
            return;
        }
        // 메뉴가 열린 동안은 메뉴가 키를 기다린다 — 뒤의 칸을 누른 것으로 메뉴를 닫지 않는다.
        if self.mode != Mode::Browse || menu::open(&self.chord) {
            return;
        }
        match (m.kind, wheel) {
            (MouseEventKind::Down(MouseButton::Left), _) => {
                self.acted();
                self.press(at);
            }
            (_, Some(by)) => {
                self.acted();
                self.wheel(at, by);
            }
            _ => {}
        }
    }

    /// 마우스로 무언가 한다 — **키 하나를 누른 것과 같이 센다**(`App::key`). 쓰기의 알림은 다음 누름에
    /// 걷히고, 기다리던 열(`g`·`Ctrl-w`)은 버린다: 누르기를 사이에 둔 `g` 와 `g` 가 `gg` 로 이으면
    /// 사람이 친 적 없는 맨 위로 가기가 선다.
    fn acted(&mut self) {
        self.notice = None;
        self.chord.clear();
    }

    /// 왼쪽 단추를 눌렀다. **누른 칸으로 포커스가 가고**, 목록의 줄을 눌렀으면 커서가 그 줄로 간다
    /// (사용자 결정 2026-10-01). 두 번 누르기는 안 받는다 — 들어가기는 Enter·`l` 이다.
    ///
    /// 커서는 `j`·`k` 와 같은 길([`App::move_to`])로 옮긴다 — 다른 줄로 가면 상세가 첫 줄로 돌아간다.
    /// 줄이 없는 자리(마지막 줄 밑의 빈 곳·테두리·열 이름 줄)는 포커스만 옮긴다.
    fn press(&mut self, at: Position) {
        let d = self.drawn;
        if d.detail.is_some_and(|r| r.contains(at)) {
            self.focus = Pane::Detail;
            return;
        }
        if !d.list.contains(at) {
            return;
        }
        self.focus = Pane::Explorer;
        if d.rows.contains(at) {
            let n = self.list.offset() + usize::from(at.y - d.rows.y);
            if n < self.rows().len() {
                self.move_to(n);
            }
        }
    }

    /// 휠을 굴렸다. **마우스가 올라선 칸이 받는다** — 포커스는 안 옮긴다(사용자 결정 2026-10-01). 목록에
    /// 서 있으면서 상세를 읽어 내려가는 손짓이 그것이고, 굴렸다고 포커스가 튀면 다음 `j` 가 엉뚱한 칸을
    /// 움직인다.
    ///
    /// - 상세는 굴린다 — 끝과 첫 줄 밖으로는 안 나간다(`Scroll::by`)
    /// - 목록은 **커서를** 옮긴다 — 커서가 없는 줄을 굴려 보이게만 하면 상세는 그대로라, 굴려서 찾은
    ///   줄을 보려면 다시 눌러야 한다. 끝에서는 멈춘다(`scroll::cursor` 와 같다)
    fn wheel(&mut self, at: Position, by: isize) {
        let d = self.drawn;
        if d.detail.is_some_and(|r| r.contains(at)) {
            self.detail.by(by);
            return;
        }
        if !d.list.contains(at) {
            return;
        }
        let Some(last) = self.rows().len().checked_sub(1) else { return };
        self.move_to(self.cursor.saturating_add_signed(by).min(last));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};
    use ratatui::crossterm::event::KeyModifiers;

    /// 줄 `n` 개짜리 탐색기를 `w`×`h` 로 한 번 그린다 — 맞히는 자리(`App::drawn`)가 선다.
    fn drawn(n: usize, w: u16, h: u16) -> App {
        let issues = (1..=n)
            .map(|k| {
                let id = format!("argos-{k:04}");
                Issue::new(id.clone(), format!("{id} title"), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z")
            })
            .collect();
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), crate::nav::Path::new());
        super::super::draw::tests::render(&mut a, w, h);
        a
    }

    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }
    }

    fn roll(a: &mut App, down: bool, column: u16, row: u16) {
        let kind = if down { MouseEventKind::ScrollDown } else { MouseEventKind::ScrollUp };
        a.mouse(event(kind, column, row));
    }

    fn click(a: &mut App, column: u16, row: u16) {
        a.mouse(event(MouseEventKind::Down(MouseButton::Left), column, row));
    }

    /// 칸 한가운데.
    fn middle(r: Rect) -> (u16, u16) {
        (r.x + r.width / 2, r.y + r.height / 2)
    }

    /// **누른 칸으로 포커스가 가고, 누른 줄로 커서가 간다**(moai-irrj.yzm). 굴린 목록에서는 굴린 만큼 더해
    /// 센다 — 화면의 셋째 줄이 목록의 셋째 줄이 아니다. 줄이 없는 빈 곳은 포커스만 옮긴다.
    #[test]
    fn a_click_focuses_the_pane_and_puts_the_cursor_on_the_row() {
        let mut a = drawn(40, 100, 20);
        let rows = a.drawn.rows;
        assert!(rows.height > 3, "목록의 줄 자리가 안 섰다: {:?}", a.drawn);
        let (dx, dy) = middle(a.drawn.detail.expect("상세가 안 섰다"));
        click(&mut a, dx, dy);
        assert_eq!(a.focus, Pane::Detail, "상세를 눌렀는데 포커스가 안 갔다");

        click(&mut a, rows.x + 2, rows.y + 2);
        assert_eq!((a.focus, a.cursor), (Pane::Explorer, 2), "셋째 줄을 눌렀다");

        // 굴린 목록 — 맨 끝으로 가 그린 뒤 화면의 첫 줄을 누르면 굴린 자리가 커서다.
        a.hit("G");
        super::super::draw::tests::render(&mut a, 100, 20);
        let top = a.list.offset();
        assert!(top > 0, "목록이 안 굴렀다");
        click(&mut a, rows.x + 2, rows.y);
        assert_eq!(a.cursor, top, "굴린 만큼을 안 더했다");
    }

    /// 마지막 줄 밑의 빈 곳·테두리는 **커서를 안 옮긴다** — 없는 줄에 커서를 세우면 상세가 빈다.
    #[test]
    fn a_click_below_the_last_row_only_focuses_the_list() {
        let mut a = drawn(3, 100, 20);
        a.hit("j");
        a.focus = Pane::Detail;
        let rows = a.drawn.rows;
        click(&mut a, rows.x + 2, rows.bottom() - 1);
        assert_eq!((a.focus, a.cursor), (Pane::Explorer, 1), "빈 곳을 눌러 커서가 움직였다");
        a.focus = Pane::Detail;
        let list = a.drawn.list;
        click(&mut a, list.x, list.y);
        assert_eq!((a.focus, a.cursor), (Pane::Explorer, 1), "테두리를 눌러 커서가 움직였다");
    }

    /// **듣는 것은 둘러볼 때뿐이다**(사용자 결정 2026-10-01) — 글을 받는 칸·메뉴가 떠 있으면 뒤의 목록을
    /// 눌러도 아무 일이 없다. 오른쪽·가운데 단추도 아무 일이 없다. 누르기는 키 하나처럼 알림을 걷고
    /// 기다리던 `g` 를 버린다 — 누르기를 사이에 둔 `g` 와 `g` 가 맨 위로 가지 않는다.
    #[test]
    fn a_click_does_nothing_over_a_menu_or_a_prompt_and_counts_as_a_keypress() {
        let mut a = drawn(10, 100, 20);
        let rows = a.drawn.rows;
        a.hit("SPC");
        click(&mut a, rows.x + 2, rows.y + 4);
        assert_eq!(a.cursor, 0, "메뉴가 열렸는데 목록이 움직였다");
        a.hit("Esc /");
        click(&mut a, rows.x + 2, rows.y + 4);
        assert_eq!(a.cursor, 0, "글을 받는 중에 목록이 움직였다");
        a.hit("Esc");
        a.mouse(event(MouseEventKind::Down(MouseButton::Right), rows.x + 2, rows.y + 4));
        assert_eq!(a.cursor, 0, "오른쪽 단추가 커서를 옮겼다");

        a.notice = Some("x".into());
        a.hit("G g");
        click(&mut a, rows.x + 2, rows.y + 4);
        assert_eq!((a.cursor, a.notice.as_deref()), (4, None), "알림이 안 걷혔다");
        a.hit("g");
        assert_eq!(a.cursor, 4, "누르기 앞의 g 와 뒤의 g 가 gg 로 이었다");
    }

    /// **휠은 마우스가 올라선 칸이 받고 포커스는 그대로다**(moai-irrj.6on). 목록 위에서는 커서가 세 줄씩
    /// 가고 끝에서 멈춘다. 상세 위에서는 목록에 포커스가 선 채로 상세가 굴러, 다음 `j` 는 여전히 목록을
    /// 움직인다.
    #[test]
    fn the_wheel_moves_the_pane_under_the_pointer_and_leaves_the_focus() {
        let mut a = drawn(5, 100, 20);
        a.site.issues[0].body = Some((1..=80).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n\n"));
        super::super::draw::tests::render(&mut a, 100, 20);
        let (lx, ly) = middle(a.drawn.list);
        let (dx, dy) = middle(a.drawn.detail.expect("상세가 안 섰다"));

        roll(&mut a, true, dx, dy);
        assert_eq!((a.focus, a.cursor, a.detail.offset()), (Pane::Explorer, 0, 3), "상세가 세 줄 안 굴렀다");
        roll(&mut a, false, dx, dy);
        roll(&mut a, false, dx, dy);
        assert_eq!(a.detail.offset(), 0, "첫 줄 위로 굴렀다");
        a.hit("j");
        assert_eq!(a.cursor, 1, "휠이 포커스를 옮겼다");

        roll(&mut a, true, lx, ly);
        assert_eq!(a.cursor, 4, "목록 위의 휠이 커서를 세 줄 안 옮겼다");
        roll(&mut a, true, lx, ly);
        assert_eq!(a.cursor, 4, "끝을 지났다");
        roll(&mut a, false, lx, ly);
        assert_eq!(a.cursor, 1);
        roll(&mut a, false, lx, ly);
        assert_eq!(a.cursor, 0, "첫 줄 위로 갔다");
    }

    /// **통계 창 위의 휠은 창을 굴린다**(사용자 결정 2026-10-01) — 키(`j`·`k`)가 옮기는 그 자리다. 덮인 목록은
    /// 안 움직인다. 고르는 창처럼 키로만 다루는 창 위에서는 휠도 아무것도 안 한다.
    #[test]
    fn the_wheel_scrolls_the_stats_window_and_nothing_over_a_picker() {
        let mut a = drawn(30, 100, 12);
        a.hit("SPC p s");
        super::super::draw::tests::render(&mut a, 100, 12);
        let (bx, by) = middle(a.drawn.body);
        let at = |a: &App| match &a.mode {
            Mode::Stats(w) => w.scroll.offset(),
            other => panic!("통계 창이 안 열렸다: {other:?}"),
        };
        roll(&mut a, true, bx, by);
        assert_eq!((at(&a), a.cursor), (3, 0), "휠이 통계 창을 안 굴렸다");
        roll(&mut a, false, bx, by);
        assert_eq!(at(&a), 0);

        a.hit("Esc SPC o t");
        let Mode::Zone(z) = &a.mode else { panic!("시간대 고르는 창이 안 열렸다: {:?}", a.mode) };
        let before = (z.cursor, z.list.offset());
        roll(&mut a, true, bx, by);
        let Mode::Zone(z) = &a.mode else { panic!("휠이 고르는 창을 닫았다") };
        assert_eq!(((z.cursor, z.list.offset()), a.cursor), (before, 0), "고르는 창 위의 휠이 무언가 움직였다");
    }
}
