//! 마우스 — 누르기·휠·칸 끌기(moai-irrj).
//!
//! **키와 같은 길로 옮긴다.** 누른 칸으로 포커스가 가는 것은 `Tab` 과, 휠이 목록을 움직이는 것은 `j`·`k`
//! 와 같은 자리를 바꾼다 — 마우스만의 상태를 두면 키로 한 일과 마우스로 한 일이 따로 논다.
//!
//! **맞히는 바탕은 지난 프레임이 그린 자리다**([`Drawn`]). 키와 누르기는 하나마다 한 번 그리고 휠·끌기는 몰아
//! 받는데(`cmd::tui::rolls`), 몰아 받는 동안에는 화면도 지난 프레임 그대로라 맞히는 바탕은 늘 사람이 보고 있는
//! 그 화면이다. 키가 상세의 끝을 마지막으로 그린 줄 수로 가늠하는 것(`Scroll::go`)과 같은 결이다.
//!
//! **듣는 자리는 둘뿐이다**(사용자 결정 2026-10-01). 목록·상세를 둘러보는 동안(한눈 보기 `0` 도 같은 칸이다)과
//! 통계 창 위의 휠이다. 폼·묻는 칸·고르는 창·지우기 확인·글을 받는 칸이 떠 있으면 **마우스를 아예 놓는다**
//! ([`App::wants_mouse`], 리뷰 뒤 사용자 결정 2026-10-01) — 그 창들은 키로 다루는 자리고, 뒤의 목록을 누른 것이
//! 적던 글을 두고 커서를 옮기면 무엇에 대해 적던 것인지를 잃는다. 잡은 채 아무것도 안 하면 그 자리에서 터미널의
//! 가운데 단추 붙여넣기와 끌어서 글 고르기만 말없이 사라지므로, 놓아서 터미널의 것으로 돌려준다. SPC 메뉴는
//! 둘러보기 위에 잠깐 뜨는 것이라 잡은 채 아무것도 안 한다.

use super::{App, Mode, Pane, menu, scroll};
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

/// 잡은 선(moai-irrj.mhr) — 끄는 동안 `App::dragging` 이 든다.
///
/// **선은 잡은 자리에서 손이 간 만큼 간다**(리뷰). 맞닿은 두 테두리 줄이 다 잡히는데(`App::grab_at`) 손이 선 칸을 곧
/// 선의 자리로 읽으면, 뒤 칸의 첫 테두리를 잡고 한 칸 움직였을 때 선이 두 칸 뛴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grab {
    /// 손이 잡은 칸 — 가른 축의 좌표.
    from: u16,
    /// 그때 선이 선 칸 — 앞에 선 칸(왼쪽·위)의 끝 테두리.
    line: u16,
    /// 잡기 전의 몫. 손이 잡은 칸으로 돌아오면 이 몫으로 돌아간다 — 선을 누르다 손이 옆으로 미끄러진 것만으로는
    /// 처음값이 설정에 박히지 않는다(`App::list_width`).
    was: Option<u16>,
}

impl App {
    /// 지금 터미널이 마우스를 잡아야 하는가 — **루프가 이것대로 켜고 끈다**(`cmd::tui`).
    ///
    /// 놓아 두지 않았고(`SPC o m`) 마우스가 듣는 자리일 때만 잡는다 — 둘러보기와 통계 창이다. 폼·묻는 칸·고르는
    /// 창·지우기 확인·글을 받는 칸(`/`·`f`)이 뜨면 놓는다(리뷰 뒤 사용자 결정 2026-10-01): 잡은 채 두면 거기서 가운데
    /// 단추 붙여넣기(X11 PRIMARY)가 말없이 사라지고, 휠도 이 에픽 전에는 터미널이 화살표 키로 바꿔 고르는 창을
    /// 굴렸다. 놓으면 그 둘이 이 에픽 전 그대로다.
    pub fn wants_mouse(&self) -> bool {
        self.mouse_on && matches!(self.mode, Mode::Browse | Mode::Stats(_))
    }

    /// 마우스 사건 하나. **루프가 받은 그대로 넘긴다**(`cmd::tui::take`).
    ///
    /// 놓은 뒤에 온 사건은 버린다 — 놓는 글이 터미널에 닿기 전에 이미 길에 있던 것이다.
    pub fn mouse(&mut self, m: MouseEvent) {
        if !self.mouse_on {
            return;
        }
        // **끄는 동안 다른 사건이 오면 끌기는 끝났다**(리뷰) — 뗌을 놓치는 터미널이 있고(창 밖에서 떼면 안 알린다), 뗌이
        // 메뉴·폼·통계 창 위에서 오면 아래 갈래가 그것을 안 받는다. 그때 끈 몫을 적는다 — 안 그러면 그 끌기는 화면에만
        // 서고 다음 실행에서 사라진다. 키도 같은 일을 한다(`App::key`).
        if !matches!(m.kind, MouseEventKind::Drag(MouseButton::Left)) {
            self.drop_line();
        }
        // **창 밖의 자리는 안 받는다**(리뷰) — 터미널이 0 을 보내면(왼쪽·위 너머, 1006 을 모르는 터미널의 223 칸 너머)
        // crossterm 이 1 을 빼다 넘쳐 맨 끝 수가 온다. 어느 쪽 끝인지 모르니 선을 그리로 끌지 않는다.
        if m.column == u16::MAX || m.row == u16::MAX {
            return;
        }
        let at = Position::new(m.column, m.row);
        let wheel = match m.kind {
            MouseEventKind::ScrollUp => Some(-WHEEL),
            MouseEventKind::ScrollDown => Some(WHEEL),
            _ => None,
        };
        // **통계 창은 휠만 받는다**(사용자 결정 2026-10-01) — 목록·상세 자리를 통째로 덮은 창이라 그 위의 휠은
        // 창을 굴린다. 키(`j`·`k`)와 같은 굴린 자리를 옮기는 것은 창이 한다(`stats::Window::roll`).
        if let Mode::Stats(w) = &mut self.mode {
            if let Some(by) = wheel
                && self.drawn.body.contains(at)
            {
                self.notice = None;
                w.roll(by);
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
                match self.grab_at(at) {
                    Some(grab) => self.dragging = Some(grab),
                    None => self.press(at),
                }
            }
            (MouseEventKind::Drag(MouseButton::Left), _) => {
                if let Some(grab) = self.dragging {
                    self.drag_to(grab, at);
                }
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

    /// 그 자리에 선 칸. 누르기와 휠이 같은 자로 가른다 — 두 칸은 겹치지 않는다(`draw::split_body`).
    fn pane_at(&self, at: Position) -> Option<Pane> {
        let d = self.drawn;
        if d.detail.is_some_and(|r| r.contains(at)) {
            Some(Pane::Detail)
        } else if d.list.contains(at) {
            Some(Pane::Explorer)
        } else {
            None
        }
    }

    /// 왼쪽 단추를 눌렀다. **누른 칸으로 포커스가 가고**, 목록의 줄을 눌렀으면 커서가 그 줄로 간다
    /// (사용자 결정 2026-10-01). 두 번 누르기는 안 받는다 — 들어가기는 Enter·`l` 이다.
    ///
    /// 커서는 `j`·`k` 와 같은 길([`App::move_to`])로 옮긴다 — 다른 줄로 가면 상세가 첫 줄로 돌아간다.
    /// 줄이 없는 자리(마지막 줄 밑의 빈 곳·테두리·열 이름 줄)는 포커스만 옮긴다.
    fn press(&mut self, at: Position) {
        let Some(pane) = self.pane_at(at) else { return };
        self.focus = pane;
        let rows = self.drawn.rows;
        if pane == Pane::Explorer && rows.contains(at) {
            let n = self.list.offset() + usize::from(at.y - rows.y);
            if n < self.rows().len() {
                self.move_to(n);
            }
        }
    }

    /// 칸 사이의 선을 잡았는가 — 맞닿은 두 테두리 줄이다(moai-irrj.mhr). 상세가 안 섰으면 선이 없다.
    ///
    /// 앞에 선 칸(왼쪽·위)의 끝 테두리와 뒤에 선 칸의 첫 테두리, 둘 다 잡힌다 — 한 칸짜리 줄만 받으면 손이
    /// 한 칸 어긋날 때마다 그 칸을 누른 것이 된다. 상세가 위아래에 서면 그 줄은 칸의 제목 줄이기도 하다.
    /// 두 칸은 사이 없이 맞닿으므로(`draw::split_body`) 뒤 칸의 첫 테두리는 앞 칸이 끝난 바로 다음 칸이다.
    fn grab_at(&self, at: Position) -> Option<Grab> {
        let d = self.drawn;
        let detail = d.detail?;
        let front = if self.detail_at.first() { detail } else { d.list };
        let (p, end, was) = if self.detail_at.vertical() {
            (at.y, front.bottom(), self.list_height)
        } else {
            (at.x, front.right(), self.list_width)
        };
        let line = end.checked_sub(1)?;
        (d.body.contains(at) && (p == line || p == end)).then_some(Grab { from: p, line, was })
    }

    /// 잡은 선을 `at` 으로 끈다. **선이 손을 따라온다** — 잡은 자리에서 손이 간 만큼 선이 가고, 앞에 선 칸이 그 자리까지
    /// 차도록 목록의 몫을 센다(`draw::share_for`). 몸통 밖으로 끌어도 끝(`draw::SHARES`)과 두 칸의 바닥에서 멈추고, 그때
    /// 남는 몫도 화면에 선 그 몫이다. 적는 것은 놓을 때다([`App::drop_line`]).
    ///
    /// 몫은 백분율이라 100칸 넘는 몸통에서는 선이 손에서 한 칸 어긋날 수 있다(200칸 넘으면 두 칸 넘게씩 간다) — 설정에
    /// 사람이 읽고 고칠 수 있는 수로 남기는 값이고, 칸 단위로 들면 창 크기가 바뀔 때마다 뜻이 바뀐다.
    fn drag_to(&mut self, grab: Grab, at: Position) {
        let body = self.drawn.body;
        let vertical = self.detail_at.vertical();
        let (start, len, p) = if vertical { (body.y, body.height, at.y) } else { (body.x, body.width, at.x) };
        if len == 0 {
            return;
        }
        let share = if p == grab.from {
            grab.was
        } else {
            let line = (i32::from(grab.line) + i32::from(p) - i32::from(grab.from))
                .clamp(i32::from(start), i32::from(start) + i32::from(len) - 1);
            // 앞에 선 칸이 차지할 칸 수 — 선이 그 칸의 끝 테두리다.
            let front = (line - i32::from(start) + 1) as u16;
            let list = if self.detail_at.first() { len - front } else { front };
            Some(super::draw::share_for(body, self.detail_at, list))
        };
        if vertical {
            self.list_height = share;
        } else {
            self.list_width = share;
        }
    }

    /// 끌기를 놓는다 — **그때 한 번** 설정에 적는다(`App::save_look`). 끄는 동안의 칸마다 적으면 손짓 하나가
    /// 설정 파일을 수십 번 다시 쓰고, 옆 탐색기와 그만큼 자주 부딪친다. 끌던 것이 없으면 아무 일도 없다.
    ///
    /// **뗌 말고도 부른다** — 다음 마우스 사건([`App::mouse`])과 다음 키(`App::key`)가 놓친 뗌을 대신한다.
    pub(super) fn drop_line(&mut self) {
        if self.dragging.take().is_some() {
            self.save_look();
        }
    }

    /// 휠을 굴렸다. **마우스가 올라선 칸이 받는다** — 포커스는 안 옮긴다(사용자 결정 2026-10-01). 목록에
    /// 서 있으면서 상세를 읽어 내려가는 손짓이 그것이고, 굴렸다고 포커스가 튀면 다음 `j` 가 엉뚱한 칸을
    /// 움직인다.
    ///
    /// - 상세는 굴린다 — 끝과 첫 줄 밖으로는 안 나간다(`Scroll::by`)
    /// - 목록은 **커서를** 옮긴다 — 커서가 없는 줄을 굴려 보이게만 하면 상세는 그대로라, 굴려서 찾은
    ///   줄을 보려면 다시 눌러야 한다. 끝에서는 멈춘다 — `j`·`k` 와 같은 자다(`scroll::cursor_by`)
    fn wheel(&mut self, at: Position, by: isize) {
        match self.pane_at(at) {
            Some(Pane::Detail) => self.detail.by(by),
            Some(Pane::Explorer) => {
                let to = scroll::cursor_by(by, self.cursor, || self.rows().len());
                self.move_to(to);
            }
            None => {}
        }
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

        // 알림은 `g` 를 누른 **뒤에** 세운다 — `G` 같은 키가 이미 알림을 걷으므로, 앞에 세우면 누르기가 걷었는지를 못 본다.
        a.hit("G g");
        a.notice = Some("x".into());
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

    fn drag(a: &mut App, column: u16, row: u16) {
        a.mouse(event(MouseEventKind::Drag(MouseButton::Left), column, row));
    }

    fn release(a: &mut App, column: u16, row: u16) {
        a.mouse(event(MouseEventKind::Up(MouseButton::Left), column, row));
    }

    /// **칸 사이 선을 잡아 끌면 선이 손을 따라오고, 놓을 때 한 번 설정에 적힌다**(moai-irrj.mhr). 잡는 것은
    /// 누르기가 아니다 — 포커스도 커서도 안 옮긴다. 다음 실행이 끈 몫으로 뜬다.
    #[test]
    fn dragging_the_line_resizes_the_panes_and_is_kept_on_release() {
        let s = crate::scratch::Scratch::new("tui-mouse-drag");
        let user = s.join("user.toml");
        let mut a = drawn(5, 100, 20);
        a.user_config = Some(user.clone());
        let line = a.drawn.list.right() - 1;
        click(&mut a, line, 5);
        assert!(a.dragging.is_some(), "선을 못 잡았다");
        assert_eq!((a.focus, a.cursor), (Pane::Explorer, 0), "선을 잡은 것이 누르기로 읽혔다");
        drag(&mut a, 29, 6);
        super::super::draw::tests::render(&mut a, 100, 20);
        assert_eq!(a.list_width, Some(30));
        assert_eq!(a.drawn.list.right() - 1, 29, "선이 손을 안 따라왔다: {:?}", a.drawn);
        assert!(!user.exists(), "끄는 동안 설정을 적었다");
        release(&mut a, 29, 6);
        assert!(a.dragging.is_none());
        let text = std::fs::read_to_string(&user).expect("놓았는데 끈 몫이 설정에 안 적혔다");
        assert!(text.contains("list_width = 30") && !text.contains("list_height"), "{text}");

        // 놓은 뒤의 끌기는 아무것도 안 한다.
        drag(&mut a, 60, 6);
        assert_eq!(a.list_width, Some(30), "놓은 선이 따라왔다");

        let mut b = drawn(5, 100, 20);
        b.user_config = Some(user);
        b.load_look();
        assert_eq!(b.list_width, Some(30), "끈 몫이 다음 실행에 안 이어졌다");
    }

    /// **상세가 앞에 서면 선 앞의 칸이 상세다**, 위아래로 서면 높이의 몫을 따로 든다. 맞닿은 두 테두리 줄이
    /// 다 잡히고, 어느 쪽을 잡았든 **잡은 테두리가 손을 따라온다**. 끝까지 밀어도 다른 칸이 제 몫을 든다
    /// ([`super::super::draw::SHARES`]).
    #[test]
    fn the_line_follows_every_side_and_stops_short_of_the_edges() {
        use super::super::view::DetailAt;
        let mut a = drawn(5, 100, 30);
        a.detail_at = DetailAt::Left;
        super::super::draw::tests::render(&mut a, 100, 30);
        // 몸통 안의 한 줄 — 높은 창에서는 머리가 위에 선다.
        let y = a.drawn.body.y + 2;
        // 뒤 칸(목록)의 첫 테두리를 잡아 29 로 — 목록이 거기서 시작한다.
        let line = a.drawn.list.x;
        click(&mut a, line, y);
        drag(&mut a, 29, y);
        release(&mut a, 29, y);
        assert_eq!(a.list_width, Some(71), "상세가 왼쪽인데 목록의 몫을 앞 칸으로 셌다");
        super::super::draw::tests::render(&mut a, 100, 30);
        assert_eq!(a.drawn.list.x, 29, "잡은 테두리가 손을 안 따라왔다: {:?}", a.drawn);

        a.detail_at = DetailAt::Bottom;
        super::super::draw::tests::render(&mut a, 100, 30);
        let body = a.drawn.body;
        // 앞 칸(목록)의 끝 테두리를 잡아 몸통 높이의 반까지 — 목록이 그 줄까지 찬다.
        let line = a.drawn.list.bottom() - 1;
        click(&mut a, 10, line);
        let front = body.height / 2;
        drag(&mut a, 10, body.y + front - 1);
        release(&mut a, 10, body.y + front - 1);
        assert!(a.list_width == Some(71) && a.list_height.is_some(), "높이의 몫이 폭과 안 갈렸다: {body:?}");
        super::super::draw::tests::render(&mut a, 100, 30);
        assert_eq!(a.drawn.list.height, front, "선이 손을 안 따라왔다: {:?}", a.drawn);

        a.detail_at = DetailAt::Right;
        super::super::draw::tests::render(&mut a, 100, 30);
        let line = a.drawn.detail.expect("상세가 안 섰다").x;
        click(&mut a, line, y);
        drag(&mut a, 0, y);
        assert_eq!(a.list_width, Some(15), "왼쪽 끝을 지났다");
        drag(&mut a, 200, y);
        assert_eq!(a.list_width, Some(85), "오른쪽 끝을 지났다");
        release(&mut a, 200, y);
        super::super::draw::tests::render(&mut a, 100, 30);
        assert!(a.drawn.detail.is_some_and(|d| d.width >= 10), "상세가 바닥 밑으로 줄었다: {:?}", a.drawn);
    }

    /// **선을 누르다 손이 미끄러진 것은 끌기가 아니다**(리뷰) — 가른 축으로 안 움직였으면 몫이 그대로라 처음값이
    /// 설정에 박히지 않는다. 뒤 칸의 첫 테두리를 잡아 한 칸 옮기면 선도 한 칸만 간다.
    #[test]
    fn a_slip_on_the_line_is_not_a_drag() {
        let s = crate::scratch::Scratch::new("tui-mouse-slip");
        let user = s.join("user.toml");
        let mut a = drawn(5, 100, 20);
        a.user_config = Some(user.clone());
        let line = a.drawn.list.right() - 1;
        click(&mut a, line, 5);
        drag(&mut a, line, 6);
        release(&mut a, line, 6);
        assert_eq!(a.list_width, None, "옆으로 미끄러진 것이 몫을 세웠다");
        assert!(!std::fs::read_to_string(&user).unwrap_or_default().contains("list_width"), "처음값이 설정에 박혔다");

        // 뒤 칸(상세)의 첫 테두리를 잡아 오른쪽으로 한 칸.
        let back = line + 1;
        click(&mut a, back, 5);
        drag(&mut a, back + 1, 5);
        release(&mut a, back + 1, 5);
        super::super::draw::tests::render(&mut a, 100, 20);
        assert_eq!(a.drawn.list.right() - 1, line + 1, "한 칸 움직였는데 선이 한 칸 넘게 갔다: {:?}", a.drawn);
    }

    /// 뗀 것을 못 받아도(창 밖에서 단추를 떼면 안 알리는 터미널이 있다) **다음 누르기가 끈 몫을 적는다** —
    /// 안 그러면 그 끌기는 화면에만 서고 다음 실행에서 사라진다.
    #[test]
    fn a_lost_release_is_saved_by_the_next_click() {
        let s = crate::scratch::Scratch::new("tui-mouse-lost-up");
        let user = s.join("user.toml");
        let mut a = drawn(5, 100, 20);
        a.user_config = Some(user.clone());
        let line = a.drawn.list.right() - 1;
        click(&mut a, line, 5);
        drag(&mut a, 39, 5);
        assert!(!user.exists());
        let rows = a.drawn.rows;
        click(&mut a, rows.x + 2, rows.y + 1);
        assert!(a.dragging.is_none());
        assert_eq!(a.cursor, 1, "끌기 뒤의 누르기가 안 들었다");
        assert!(std::fs::read_to_string(&user).unwrap_or_default().contains("list_width = 40"));
    }

    /// **키도 놓친 뗌을 대신한다**(리뷰) — 뗌을 놓친 뒤 키만 쓰다 끝내도 끈 몫은 남는다. 휠도 같다.
    #[test]
    fn a_lost_release_is_saved_by_the_next_key_or_wheel_too() {
        let s = crate::scratch::Scratch::new("tui-mouse-lost-up-key");
        let user = s.join("user.toml");
        let mut a = drawn(5, 100, 20);
        a.user_config = Some(user.clone());
        let line = a.drawn.list.right() - 1;
        click(&mut a, line, 5);
        drag(&mut a, 39, 5);
        a.hit("j");
        assert!(a.dragging.is_none(), "키가 끌기를 안 끝냈다");
        assert!(std::fs::read_to_string(&user).unwrap_or_default().contains("list_width = 40"), "키 뒤에도 안 적혔다");

        // 다시 그려 선이 39 에 선 화면에서 한 번 더.
        super::super::draw::tests::render(&mut a, 100, 20);
        assert_eq!(a.drawn.list.right() - 1, 39);
        click(&mut a, 39, 5);
        drag(&mut a, 49, 5);
        let (lx, ly) = middle(a.drawn.list);
        roll(&mut a, true, lx, ly);
        assert!(a.dragging.is_none(), "휠이 끌기를 안 끝냈다");
        assert!(std::fs::read_to_string(&user).unwrap_or_default().contains("list_width = 50"), "휠 뒤에도 안 적혔다");
    }

    /// **설정에 남는 몫은 화면에 선 몫이다**(리뷰) — 상세의 바닥을 지나 끌면 선은 바닥에서 서는데, 몫이 손을 따라
    /// 85% 까지 가면 화면에 한 번도 안 선 몫이 남아 큰 창에서 그 몫이 선다.
    #[test]
    fn the_share_kept_is_the_one_on_screen() {
        use super::super::view::DetailAt;
        let mut a = drawn(30, 100, 20);
        a.detail_at = DetailAt::Bottom;
        super::super::draw::tests::render(&mut a, 100, 20);
        let body = a.drawn.body;
        let line = a.drawn.list.bottom() - 1;
        click(&mut a, 10, line);
        drag(&mut a, 10, body.bottom() - 1);
        release(&mut a, 10, body.bottom() - 1);
        super::super::draw::tests::render(&mut a, 100, 20);
        let seen = a.drawn.list.height;
        assert!(a.drawn.detail.is_some() && seen < body.height, "상세가 사라졌다: {:?}", a.drawn);
        // 같은 몫을 큰 창에 입혀도 목록은 작은 창에서 본 만큼의 비율만 든다.
        super::super::draw::tests::render(&mut a, 100, 60);
        let tall = a.drawn.body.height;
        let want = u32::from(tall) * u32::from(seen) / u32::from(body.height);
        assert!(
            u32::from(a.drawn.list.height).abs_diff(want) <= 1,
            "화면에 안 선 몫이 남았다: {:?} — {} 줄 / {tall} 줄",
            a.list_height,
            a.drawn.list.height
        );
    }

    /// **창 밖의 자리는 안 받는다**(리뷰) — 터미널이 0 을 보내면 crossterm 이 넘친 끝 수를 넘긴다. 그것으로 선을 끌면
    /// 선이 반대쪽 끝으로 튄다.
    #[test]
    fn a_report_from_outside_the_window_moves_nothing() {
        let mut a = drawn(5, 100, 20);
        let line = a.drawn.list.right() - 1;
        click(&mut a, line, 5);
        drag(&mut a, u16::MAX, 5);
        assert_eq!(a.list_width, None, "넘친 자리로 선을 끌었다");
        assert!(a.dragging.is_some(), "넘친 자리의 끌기가 잡은 선을 놓았다");
    }

    /// **설정의 몫은 관대하게 읽는다** — 범위 밖의 수는 끝으로 당겨 서고, 이 세션이 안 끄는 한 파일의 줄은 그대로다.
    /// 수가 아닌 값은 그 까닭을 한 줄로 대고 나머지 보기는 입힌다. 좁은 창에서 몫이 작아도 목록은 바닥을 지킨다.
    #[test]
    fn a_share_in_the_config_is_read_leniently_and_the_list_keeps_its_floor() {
        let s = crate::scratch::Scratch::new("tui-mouse-share-config");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\nlist_width = 5\nlist_height = \"tall\"\nsort = \"title\"\n").unwrap();
        let mut a = drawn(5, 100, 20);
        a.user_config = Some(user.clone());
        a.load_look();
        assert_eq!((a.list_width, a.list_height), (Some(15), None));
        let said = a.notice.clone().unwrap_or_default();
        assert!(
            said.contains("list_height") && said.contains(crate::i18n::say(a.site.lang, "look.want_number")),
            "{said}"
        );
        assert_eq!(a.order.by, super::super::keys::Order::Title, "틀린 키 하나로 나머지를 버렸다");
        a.hit("SPC v l Esc");
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(
            text.contains("list_width = 5") && text.contains("list_height = \"tall\""),
            "안 끈 몫을 고쳐 적었다\n{text}"
        );

        // 좁은 창 — 15% 는 4칸이지만 목록은 바닥(테두리 둘과 여덟 칸)을 든다.
        super::super::draw::tests::render(&mut a, 30, 20);
        assert!(a.drawn.list.width >= 10, "목록이 바닥 밑으로 줄었다: {:?}", a.drawn);
    }

    /// **마우스는 듣는 자리에서만 잡는다**(리뷰 뒤 사용자 결정 2026-10-01) — 둘러보기(메뉴가 떠도)와 통계 창이다.
    /// 글을 받는 칸·고르는 창이 뜨면 놓아 그 자리의 가운데 단추 붙여넣기와 끌어서 고르기를 터미널에 돌려주고, 닫으면
    /// 다시 잡는다. 놓아 둔 사람(`SPC o m`)에게는 어디서도 안 잡는다.
    #[test]
    fn the_mouse_is_caught_only_where_it_listens() {
        let mut a = drawn(5, 100, 20);
        assert!(a.wants_mouse(), "둘러볼 때 안 잡았다");
        a.hit("SPC");
        assert!(a.wants_mouse(), "메뉴가 뜨자 놓았다 — 메뉴 밑의 칸을 누르면 터미널이 글을 고른다");
        a.hit("Esc /");
        assert!(!a.wants_mouse(), "검색 칸에서 잡았다 — 가운데 단추 붙여넣기가 사라진다");
        a.hit("Esc SPC f");
        assert!(matches!(a.mode, Mode::Filter(_)), "거름망 칸이 안 열렸다: {:?}", a.mode);
        assert!(!a.wants_mouse(), "거름망 칸에서 잡았다");
        a.hit("Esc SPC o t");
        assert!(!a.wants_mouse(), "고르는 창에서 잡았다 — 휠이 이 에픽 전처럼 안 구른다");
        a.hit("Esc");
        assert!(a.wants_mouse(), "닫은 뒤 다시 안 잡았다");
        a.hit("SPC p s");
        assert!(a.wants_mouse(), "통계 창에서 놓았다 — 휠이 창을 못 굴린다");
        a.hit("Esc SPC o m Esc");
        assert!(!a.mouse_on && !a.wants_mouse(), "놓아 둔 마우스를 잡았다");
        a.hit("SPC p s");
        assert!(!a.wants_mouse(), "놓아 둔 마우스를 통계 창에서 잡았다");
    }
}
