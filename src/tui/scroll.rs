//! 굴리기. **굴릴 수 있는 것이면 무엇이든 이것 하나로 굴린다.**
//!
//! 한때 두 벌이었다 — 왼쪽 목록은 위젯의 `ListState` 가 훑는 자리를 들고, 오른쪽
//! 상세는 `scroll: u16` 하나를 들고 그림이 잘라 도로 넣었다. 키도 끝의 뜻도
//! 달랐고, 굴릴 것이 남았다는 표시는 상세에만 제목 글로 붙어 있었다. 칸이 하나
//! 더 생기면 세 벌째를 지어야 했다.
//!
//! **순수하다.** 이동([`Move`])과 잰 값(높이·길이)을 받아 제 상태만 바꾼다.
//! 터미널도 저장소도 모른다 — 그래서 시험이 터미널 없이 돈다(moai-0k1p).
//! 높이와 길이는 그림이 재서 [`Scroll::fit`] 으로 넣는다. 줄 수는 폭에 달렸고
//! 폭은 그려야 나오기 때문이다.

/// PageUp/Down 한 번에 가는 줄 수. 목록과 상세가 **같은 걸음**이다 — 칸마다
/// 다르면 Tab 한 번에 같은 키의 뜻이 바뀐다.
pub const PAGE: usize = 10;

/// Ctrl-d·Ctrl-u 한 번에 가는 줄 수 — 한 쪽의 반. vim 은 창 높이의 반을 가지만, 여기는
/// 한 쪽([`PAGE`])이 이미 높이와 무관한 고정 걸음이라 반 쪽도 그것의 반이다. 높이를 따르게
/// 하면 목록 커서는 제 칸 높이를 모르고([`cursor`] 는 잰 값을 안 읽는다) 두 칸의 걸음이 갈린다.
pub const HALF: usize = PAGE / 2;

/// 이동 하나. **어느 키였는지는 여기서 모른다** — 키 표(`keys`)가 키를 이것으로 풀고,
/// 굴리는 칸과 커서가 이것을 받는다. 키로 받던 때에는 `j`·Ctrl-d 를 더하려면 이 조각이
/// 키 모양을 다시 알아야 했다(moai-ob4c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    LineUp,
    LineDown,
    HalfUp,
    HalfDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

impl Move {
    /// 몇 줄 가는가. 음수면 위로. 끝으로 가는 둘은 `None` 이다 — 줄 수가 아니라 자리다.
    fn delta(self) -> Option<isize> {
        Some(match self {
            Move::LineUp => -1,
            Move::LineDown => 1,
            Move::HalfUp => -(HALF as isize),
            Move::HalfDown => HALF as isize,
            Move::PageUp => -(PAGE as isize),
            Move::PageDown => PAGE as isize,
            Move::Top | Move::Bottom => return None,
        })
    }
}

/// 한 칸의 굴린 자리. 굴린 자리·보이는 높이·전체 길이를 함께 든다 — 셋이 한
/// 곳에 있어야 "끝을 지났나" 와 "더 있나" 를 같은 자로 잰다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scroll {
    offset: usize,
    height: usize,
    len: usize,
    /// 높이와 길이가 **지금 든 내용을 잰 것인가.** 다른 것을 보게 되면
    /// ([`Scroll::rewind`]) 옛 내용의 길이로 끝을 재면 안 된다 — 짧은 이슈를 보다
    /// 긴 이슈로 옮겨 그리기 전에 `End` 를 누르면 짧은 쪽의 끝에 선다. 그래서 잰
    /// 값을 모르는 동안에는 끝을 막지 않고, 다음 [`Scroll::fit`] 이 자른다.
    measured: bool,
}

impl Scroll {
    /// 맨 위에 보이는 줄.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// 굴릴 수 있는 끝. **내용이 칸보다 짧으면 0 이다** — 짧은 것은 아예 안 구른다.
    fn last(&self) -> usize {
        self.len.saturating_sub(self.height)
    }

    /// 키가 넘지 못할 끝. 잰 값을 모르면 막지 않는다 — 막으면 틀린 끝에 선다.
    fn wall(&self) -> usize {
        if self.measured { self.last() } else { usize::MAX }
    }

    /// 그림이 잰 높이와 길이를 들인다. **끝을 지난 자리는 잘라 넣는다.**
    ///
    /// 그리는 데만 자르고 큰 수를 남겨 두면, 끝까지 굴린 뒤 위로 가는 키를 눌러도
    /// 그 수가 끝 밑으로 내려올 때까지 화면이 한 칸도 안 움직인다 — 키가 죽은
    /// 것처럼 보인다. 창을 줄였다 키웠을 때 옛 수가 살아나 끝으로 튀는 것도 같은
    /// 뿌리다.
    pub fn fit(&mut self, height: usize, len: usize) {
        self.height = height;
        self.len = len;
        self.measured = true;
        self.offset = self.offset.min(self.last());
    }

    /// 첫 줄로 되돌린다. **다른 것을 보게 되면 부른다** — 굴린 자리가 남아 있으면
    /// 새로 본 것을 첫 줄부터 못 본다. 잰 값도 옛 내용의 것이라 버린다.
    pub fn rewind(&mut self) {
        self.offset = 0;
        self.measured = false;
    }

    /// 몇 줄 굴린다. 음수면 위로. 끝과 첫 줄 밖으로는 안 나간다.
    pub fn by(&mut self, delta: isize) {
        let to = self.offset.saturating_add_signed(delta);
        self.offset = to.min(self.wall());
    }

    /// 이동 하나를 받는다.
    ///
    /// 끝은 **마지막으로 잰 길이**로 잰다. 루프는 키 하나마다 한 번 그리므로 잰 값이
    /// 한 걸음 넘게 낡지 않고, 낡았어도 다음 [`Scroll::fit`] 이 바로잡는다. 다른 것을
    /// 보게 된 뒤 아직 안 쟀으면 끝을 막지 않는다(`measured`).
    pub fn go(&mut self, m: Move) {
        match m.delta() {
            Some(d) => self.by(d),
            None if m == Move::Top => self.offset = 0,
            None => self.offset = self.wall(),
        }
    }

    /// 그 줄이 보이게 가장 적게 굴린다. **커서가 있는 칸이 쓴다** — 커서는 칸
    /// 안에서 움직이고, 칸 밖으로 나갈 때만 따라 굴린다. 매번 커서를 맨 위나 맨
    /// 아래에 붙이면 그 위나 아래를 한 줄도 못 본다.
    ///
    /// [`Scroll::fit`] 다음에 부른다 — 높이를 모르면 무엇이 보이는지도 모른다.
    pub fn reveal(&mut self, at: usize) {
        if self.height == 0 {
            return;
        }
        if at < self.offset {
            self.offset = at;
        } else if at >= self.offset + self.height {
            self.offset = at + 1 - self.height;
        }
        self.offset = self.offset.min(self.last());
    }

    /// 칸 위로 숨은 줄 수.
    pub fn above(&self) -> usize {
        self.offset.min(self.last())
    }

    /// 칸 아래로 숨은 줄 수.
    pub fn below(&self) -> usize {
        self.last() - self.above()
    }

    /// **굴릴 것이 남았으면 그렇다고 말하는 글.** 없으면 `None`.
    ///
    /// 잘린 줄이 조용히 사라지면 보는 쪽은 그것이 끝인 줄 안다. 화살표만 두지 않고
    /// 줄 수를 붙인다 — 색이 혼자 뜻을 지지 않듯 글리프도 혼자 지지 않는다.
    /// 끝까지 굴렸으면 `끝` 이라 적는다: 표시가 사라지기만 하면 굴릴 것이 없는
    /// 짧은 글과 끝까지 굴린 긴 글이 같아 보인다.
    pub fn mark(&self) -> Option<String> {
        match (self.above(), self.below()) {
            (0, 0) => None,
            (0, down) => Some(format!("↓ {down}줄")),
            (up, 0) => Some(format!("↑ {up}줄 · 끝")),
            (up, down) => Some(format!("↑ {up}줄 · ↓ {down}줄")),
        }
    }
}

/// 커서를 옮긴다. 옮길 자리를 낸다.
///
/// **목록의 길이는 필요할 때만 센다**(`last`). 목록을 세는 데 이슈 전부를 훑고
/// 정렬까지 하므로, 위로 가는 키에도 미리 세면 그 값이 그대로 버려진다. 길이를
/// 잰 값([`Scroll::fit`])에서 읽지 않는 까닭도 있다 — 다시 읽기가 그림 사이에
/// 목록을 줄이면 낡은 길이가 커서를 목록 밖에 세운다.
pub fn cursor(m: Move, at: usize, len: impl FnOnce() -> usize) -> usize {
    let last = || len().saturating_sub(1);
    match m.delta() {
        Some(d) if d < 0 => at.saturating_add_signed(d),
        Some(d) => at.saturating_add_signed(d).min(last()),
        None if m == Move::Top => 0,
        None => last(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(height: usize, len: usize) -> Scroll {
        let mut s = Scroll::default();
        s.fit(height, len);
        s
    }

    const MOVES: [Move; 8] = [
        Move::LineDown,
        Move::HalfDown,
        Move::PageDown,
        Move::Bottom,
        Move::LineUp,
        Move::HalfUp,
        Move::PageUp,
        Move::Top,
    ];

    /// 빈 것은 굴러가지 않고 표시도 없다.
    #[test]
    fn empty_content_never_scrolls() {
        for mut s in [sized(10, 0), sized(0, 0)] {
            for k in MOVES {
                s.go(k);
                assert_eq!(s.offset(), 0, "{k:?} 에 빈 것이 굴렀다");
            }
            s.by(5);
            s.reveal(3);
            assert_eq!((s.offset(), s.mark()), (0, None));
        }
    }

    /// 칸보다 짧은 것은 **아예 안 구른다.** 끝이 칸 안에 이미 보인다.
    #[test]
    fn content_shorter_than_the_viewport_does_not_scroll() {
        for len in [1, 9, 10] {
            let mut s = sized(10, len);
            for k in MOVES {
                s.go(k);
                assert_eq!(s.offset(), 0, "{len}줄짜리가 {k:?} 에 굴렀다");
            }
            // 키가 아닌 길(몇 줄 굴리기·커서 따라가기)도 같은 끝을 본다
            s.by(len as isize);
            s.reveal(len - 1);
            assert_eq!(s.offset(), 0, "{len}줄짜리가 by·reveal 에 굴렀다");
            assert_eq!(s.mark(), None, "{len}줄짜리에 더 있다고 한다");
        }
    }

    /// 끝을 지나 굴리지 않는다. **끝에서 위로 가는 키가 곧바로 듣는다** — 넘친 수가
    /// 남아 있으면 그 수가 끝 밑으로 내려올 때까지 화면이 안 움직인다.
    #[test]
    fn the_end_is_a_wall_and_up_answers_at_once() {
        let mut s = sized(10, 25);
        for _ in 0..100 {
            s.go(Move::LineDown);
        }
        assert_eq!(s.offset(), 15, "끝을 지나 굴렀다");
        s.by(1_000);
        assert_eq!(s.offset(), 15);
        s.go(Move::LineUp);
        assert_eq!(s.offset(), 14, "끝에서 위로 가는 키가 죽었다");
        s.go(Move::Bottom);
        assert_eq!(s.offset(), 15, "End 가 끝에 안 닿았다");
        s.go(Move::Top);
        assert_eq!(s.offset(), 0);
        s.by(-3);
        assert_eq!(s.offset(), 0, "첫 줄 위로 굴렀다");
    }

    /// PageUp/Down 은 [`PAGE`] 줄씩 가고 끝에서 멈춘다.
    #[test]
    fn page_keys_step_a_page_and_stop_at_the_edges() {
        let mut s = sized(5, 28);
        s.go(Move::PageDown);
        assert_eq!(s.offset(), PAGE);
        s.go(Move::PageDown);
        assert_eq!(s.offset(), 2 * PAGE);
        s.go(Move::PageDown);
        assert_eq!(s.offset(), 23, "끝을 지나 한 쪽을 넘겼다");
        s.go(Move::PageUp);
        assert_eq!(s.offset(), 13);
        s.go(Move::PageUp);
        s.go(Move::PageUp);
        assert_eq!(s.offset(), 0);
    }

    /// 내용이 줄거나 칸이 커지면 **다음에 잴 때 끝으로 잘린다.** 창을 줄였다
    /// 키웠을 때 옛 수가 살아나 끝으로 튀지 않는다.
    #[test]
    fn a_new_measure_clamps_the_offset() {
        let mut s = sized(10, 100);
        s.go(Move::Bottom);
        assert_eq!(s.offset(), 90);
        s.fit(10, 30);
        assert_eq!(s.offset(), 20, "내용이 줄었는데 끝을 지나 있다");
        s.fit(40, 30);
        assert_eq!(s.offset(), 0, "칸이 내용보다 커졌는데 굴려져 있다");
        s.fit(10, 30);
        assert_eq!(s.offset(), 0, "옛 수가 살아났다");
    }

    /// **다른 것을 보게 되면 옛 길이로 끝을 재지 않는다.** 짧은 것을 보다 긴 것으로
    /// 옮겨 그리기 전에 `End` 를 누르면, 옛 길이로는 짧은 쪽의 끝에 선다. 모르는 동안은
    /// 막지 않고 다음에 잴 때 자른다 — 그 뒤로는 위로 가는 키가 곧바로 듣는다.
    #[test]
    fn after_a_rewind_the_old_length_is_not_the_wall() {
        let mut s = sized(10, 12);
        s.rewind();
        s.go(Move::Bottom);
        s.fit(10, 50);
        assert_eq!(s.offset(), 40, "옛 길이의 끝에 섰다");
        s.go(Move::LineUp);
        assert_eq!(s.offset(), 39, "잰 뒤에 ↑ 가 곧바로 안 듣는다");

        // 잰 적이 없는 것도 같다 — 그리기 전에 누른 키는 다음에 잴 때 잘린다
        let mut s = Scroll::default();
        for _ in 0..80 {
            s.go(Move::PageDown);
        }
        s.fit(10, 0);
        assert_eq!((s.offset(), s.mark()), (0, None));
    }

    /// 굴릴 것이 남았으면 **표시가 선다** — 아래로 몇 줄, 위로 몇 줄, 끝.
    #[test]
    fn the_mark_says_how_much_is_hidden() {
        let mut s = sized(10, 22);
        assert_eq!(s.mark().as_deref(), Some("↓ 12줄"));
        s.by(4);
        assert_eq!(s.mark().as_deref(), Some("↑ 4줄 · ↓ 8줄"));
        s.go(Move::Bottom);
        assert_eq!(s.mark().as_deref(), Some("↑ 12줄 · 끝"), "끝까지 굴렸는데 끝이라 안 한다");
        assert_eq!((s.above(), s.below()), (12, 0));
    }

    /// 커서를 따라 굴릴 때는 **칸 밖으로 나갈 때만** 가장 적게 굴린다.
    #[test]
    fn reveal_scrolls_only_as_far_as_needed() {
        let mut s = sized(5, 20);
        s.reveal(3);
        assert_eq!(s.offset(), 0, "보이는 줄인데 굴렀다");
        s.reveal(7);
        assert_eq!(s.offset(), 3, "커서를 맨 아랫줄에 세우지 않았다");
        s.reveal(5);
        assert_eq!(s.offset(), 3, "칸 안에서 움직였는데 굴렀다");
        s.reveal(1);
        assert_eq!(s.offset(), 1, "커서를 맨 윗줄에 세우지 않았다");
        s.reveal(19);
        assert_eq!(s.offset(), 15);
        // 목록이 줄면 끝으로 당긴다 — 빈 줄을 보여 주며 서 있지 않는다
        s.fit(5, 8);
        s.reveal(7);
        assert_eq!(s.offset(), 3);
    }

    /// 커서는 목록 밖으로 못 나간다. 길이는 **필요할 때만** 센다.
    #[test]
    fn the_cursor_stays_inside_and_counts_only_when_needed() {
        let never = || -> usize { panic!("위로 가는 키에 목록을 셌다") };
        assert_eq!(cursor(Move::LineUp, 0, never), 0);
        assert_eq!(cursor(Move::PageUp, 4, never), 0);
        assert_eq!(cursor(Move::HalfUp, 7, never), 2);
        assert_eq!(cursor(Move::Top, 4, never), 0);
        assert_eq!(cursor(Move::LineDown, 4, || 5), 4);
        assert_eq!(cursor(Move::PageDown, 1, || 30), 11);
        assert_eq!(cursor(Move::PageDown, 25, || 30), 29);
        assert_eq!(cursor(Move::HalfDown, 1, || 30), 6);
        assert_eq!(cursor(Move::HalfDown, 27, || 30), 29);
        assert_eq!(cursor(Move::Bottom, 0, || 30), 29);
        assert_eq!(cursor(Move::Bottom, 0, || 0), 0, "빈 목록에서 넘쳤다");
    }

    /// **반 쪽은 한 쪽의 반이다**(Ctrl-d·Ctrl-u) — 끝에서 멈추는 것도 한 쪽과 같다.
    #[test]
    fn half_page_moves_half_a_page_and_stops_at_the_edges() {
        assert_eq!(HALF * 2, PAGE);
        let mut s = sized(5, 28);
        s.go(Move::HalfDown);
        assert_eq!(s.offset(), HALF);
        for _ in 0..10 {
            s.go(Move::HalfDown);
        }
        assert_eq!(s.offset(), 23, "끝을 지나 반 쪽을 넘겼다");
        s.go(Move::HalfUp);
        assert_eq!(s.offset(), 23 - HALF);
        for _ in 0..10 {
            s.go(Move::HalfUp);
        }
        assert_eq!(s.offset(), 0);
    }
}
