//! 보기 상태 — 세로 첫 줄, 가로 축, 고정 칸 폭, 맞춤. 터미널 없이 시험하는 순수 셈이다.
//!
//! 세로 밀림은 부르는 쪽 목록의 `Scroll` 과 같은 값이라 크레이트는 받기만 한다. 가로(축)는
//! 크레이트가 쥔다. 모든 동작은 "사람이 움직였다" 이므로 맞춤 깃발을 내린다 — 맞춤만 깃발을
//! 올리고, 깃발이 선 동안은 [`GanttState::refit`] 가 범위가 바뀔 때마다 다시 맞춘다.

use crate::axis::{Axis, DAY, Zoom};

/// 고정 칸의 기본 폭. 시안의 `ID st 제목` 이 들어갈 만한 값이다.
pub const DEFAULT_LABEL_WIDTH: u16 = 40;

/// 보기 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GanttState {
    /// 화면 맨 위 본문 줄의 번호. 부르는 쪽 목록의 첫 줄과 같은 값이다.
    pub first_row: usize,
    /// 가로 원점과 확대.
    pub axis: Axis,
    /// 고정 칸(이름) 폭. 분할선 끌기가 바꾼다. 그릴 때는 화면에 맞춰 잘린다([`crate::Layout`]).
    pub label_width: u16,
    /// 맞춤 상태인가. 사람이 움직이거나 확대하면 내려간다.
    pub fitting: bool,
}

impl Default for GanttState {
    fn default() -> Self {
        GanttState { first_row: 0, axis: Axis::new(Zoom::Week, 0), label_width: DEFAULT_LABEL_WIDTH, fitting: false }
    }
}

impl GanttState {
    pub fn new(axis: Axis) -> Self {
        GanttState { axis, ..Self::default() }
    }

    /// `n` 칸 가로로 민다 (양수면 미래 쪽).
    pub fn scroll_cells(&mut self, n: i64) {
        self.fitting = false;
        self.axis.scroll(n);
    }

    /// `n` 쪽 민다. 한 쪽은 날짜 칸 폭에서 한 칸을 뺀 값이다 — 건너뛴 쪽의 끝 칸이 다음 쪽 첫 칸으로
    /// 남아야 사람이 어디로 갔는지 잃지 않는다.
    pub fn scroll_pages(&mut self, n: i64, dates_width: u16) {
        self.scroll_cells(n * i64::from(dates_width.saturating_sub(1).max(1)));
    }

    /// 한 단계 더 고운 쪽으로. `anchor` 시각이 화면 칸 `col` 에 그대로 남는다 — 기준점은 커서 줄의
    /// 막대나 화면 가운데를 부르는 쪽이 고른다. 끝에 닿아 단계가 안 바뀌면 아무것도 안 바꾼다.
    pub fn zoom_in(&mut self, anchor: i64, col: f64) {
        self.set_zoom(self.axis.zoom.finer(), anchor, col);
    }

    /// 한 단계 더 거친 쪽으로. 규칙은 [`zoom_in`](Self::zoom_in) 과 같다.
    pub fn zoom_out(&mut self, anchor: i64, col: f64) {
        self.set_zoom(self.axis.zoom.coarser(), anchor, col);
    }

    fn set_zoom(&mut self, zoom: Zoom, anchor: i64, col: f64) {
        if zoom != self.axis.zoom {
            self.fitting = false;
            self.axis = self.axis.with_zoom(zoom, anchor, col);
        }
    }

    /// `now` 를 날짜 칸 가운데에 놓는다. 확대는 그대로다.
    pub fn to_today(&mut self, now: i64, dates_width: u16) {
        self.fitting = false;
        let col = f64::from(dates_width / 2);
        self.axis = self.axis.with_zoom(self.axis.zoom, now, col);
    }

    /// 맞춤을 켜고 바로 맞춘다. `range` 는 모든 줄의 `[가장 이른 시각, 가장 늦은 시각]` 이다.
    /// 범위가 없으면(줄에 날짜가 하나도 없다) 깃발만 서고 축은 그대로다.
    pub fn fit(&mut self, range: Option<(i64, i64)>, dates_width: u16) {
        self.fitting = true;
        self.apply_fit(range, dates_width);
    }

    /// 맞춤 상태일 때만 다시 맞춘다. 그릴 때마다 부르면 거르개가 바뀌어 범위가 달라져도 화면이
    /// 엉뚱한 곳에 남지 않는다. 맞춤이 아니면 아무것도 안 한다.
    pub fn refit(&mut self, range: Option<(i64, i64)>, dates_width: u16) {
        if self.fitting {
            self.apply_fit(range, dates_width);
        }
    }

    fn apply_fit(&mut self, range: Option<(i64, i64)>, width: u16) {
        let Some((lo, hi)) = range else { return };
        if width == 0 {
            return;
        }
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        let w = i64::from(width);
        // 앞에 한 칸 여유를 두고 날 경계로 내린다. 날 경계라야 머리글이 반듯하다.
        for zoom in Zoom::ALL.iter().rev() {
            let spc = zoom.secs_per_cell();
            let origin = (lo - spc).div_euclid(DAY) * DAY;
            // 끝 시각이 든 칸까지 (+1) 들어와야 한다.
            if (hi - origin).div_euclid(spc) < w {
                self.axis = Axis::new(*zoom, origin);
                return;
            }
        }
        // 가장 거친 단계로도 안 들어가면 시작이 보이게만 한다.
        self.axis = Axis::new(Zoom::Month, lo.div_euclid(DAY) * DAY);
    }

    /// 분할선 끌기. 값은 그대로 두고, 그릴 때 분할선 한 칸을 남기도록 화면에 맞춰 잘린다([`crate::Layout`]).
    pub fn set_label_width(&mut self, w: u16) {
        self.label_width = w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(zoom: Zoom, origin: i64) -> GanttState {
        GanttState::new(Axis::new(zoom, origin))
    }

    #[test]
    fn scrolling_clears_fit_and_moves_by_cells_and_pages() {
        let mut s = st(Zoom::Week, 10 * DAY);
        s.fitting = true;
        s.scroll_cells(2);
        assert!(!s.fitting);
        assert_eq!(s.axis.origin, 12 * DAY);
        s.scroll_pages(-1, 30); // 29칸
        assert_eq!(s.axis.origin, (12 - 29) * DAY);
        let mut m = st(Zoom::Month, 0);
        m.scroll_cells(1);
        assert_eq!(m.axis.origin, 7 * DAY);
        // 폭이 0·1 이어도 한 쪽은 한 칸 이상 간다.
        let mut z = st(Zoom::Week, 0);
        z.scroll_pages(1, 0);
        assert_eq!(z.axis.origin, DAY);
    }

    #[test]
    fn zoom_keeps_the_anchor_on_its_column() {
        let mut s = st(Zoom::Week, 100 * DAY);
        let anchor = 110 * DAY + 3 * 3_600;
        let col = s.axis.cell_of_secs(anchor).floor();
        s.zoom_in(anchor, col);
        assert_eq!(s.axis.zoom, Zoom::Day);
        // 확대 뒤에도 anchor 가 같은 칸에 있다.
        assert_eq!(s.axis.cell_of_secs(anchor).floor(), col);
        s.zoom_out(anchor, col);
        assert_eq!(s.axis.zoom, Zoom::Week);
        assert_eq!(s.axis.cell_of_secs(anchor).floor(), col);
    }

    #[test]
    fn zoom_at_the_ends_changes_nothing_and_keeps_fit() {
        let mut s = st(Zoom::Hour, 5);
        s.fitting = true;
        s.zoom_in(0, 3.0);
        assert!(s.fitting);
        assert_eq!(s.axis, Axis::new(Zoom::Hour, 5));
        let mut m = st(Zoom::Month, 5);
        m.zoom_out(0, 3.0);
        assert_eq!(m.axis, Axis::new(Zoom::Month, 5));
    }

    #[test]
    fn zoom_clears_fit() {
        let mut s = st(Zoom::Week, 0);
        s.fitting = true;
        s.zoom_out(0, 0.0);
        assert!(!s.fitting);
    }

    #[test]
    fn today_lands_in_the_middle_column() {
        let mut s = st(Zoom::Day, 0);
        let now = 200 * DAY + 5 * 3_600;
        s.to_today(now, 40);
        assert_eq!(s.axis.today_col(now, 40), Some(20));
        assert_eq!(s.axis.zoom, Zoom::Day);
    }

    #[test]
    fn fit_picks_the_finest_zoom_that_holds_the_range() {
        let lo = 1000 * DAY;
        // 3일: 앞 여유가 날 경계로 내려가 Hour 는 96칸을 쓴다. 100칸이면 들어간다.
        let mut s = GanttState::default();
        s.fit(Some((lo, lo + 3 * DAY)), 100);
        assert_eq!(s.axis.zoom, Zoom::Hour);
        assert!(s.fitting);
        // 20일: Hour 480칸, Day 80칸 (+여유) → 80칸에 안 들어가고 Week 20칸이 된다.
        s.fit(Some((lo, lo + 20 * DAY)), 80);
        assert_eq!(s.axis.zoom, Zoom::Week);
        // 범위가 처음과 끝을 모두 화면 안에 둔다.
        assert!(s.axis.today_col(lo, 80).is_some());
        assert!(s.axis.today_col(lo + 20 * DAY, 80).is_some());
        // 400일은 Month(칸 하나가 1주).
        s.fit(Some((lo, lo + 400 * DAY)), 80);
        assert_eq!(s.axis.zoom, Zoom::Month);
        // 원점은 날 경계고 시작보다 앞이다.
        assert_eq!(s.axis.origin.rem_euclid(DAY), 0);
        assert!(s.axis.origin < lo);
    }

    #[test]
    fn fit_with_a_range_too_wide_for_any_zoom_shows_the_start() {
        let lo = 1000 * DAY;
        let mut s = GanttState::default();
        s.fit(Some((lo, lo + 5000 * DAY)), 10);
        assert_eq!(s.axis, Axis::new(Zoom::Month, lo));
    }

    #[test]
    fn fit_without_range_or_width_only_raises_the_flag() {
        let mut s = st(Zoom::Day, 7 * DAY);
        s.fit(None, 80);
        assert!(s.fitting);
        assert_eq!(s.axis, Axis::new(Zoom::Day, 7 * DAY));
        s.fit(Some((0, DAY)), 0);
        assert_eq!(s.axis, Axis::new(Zoom::Day, 7 * DAY));
    }

    #[test]
    fn refit_follows_the_range_only_while_fitting() {
        let mut s = GanttState::default();
        s.fit(Some((0, 3 * DAY)), 80);
        let first = s.axis;
        // 거르개가 바뀌어 범위가 멀리 옮겨 갔다.
        s.refit(Some((300 * DAY, 303 * DAY)), 80);
        assert_ne!(s.axis, first);
        assert!(s.axis.today_col(300 * DAY, 80).is_some());
        // 사람이 움직이면 더는 따라가지 않는다.
        s.scroll_cells(1);
        let moved = s.axis;
        s.refit(Some((0, DAY)), 80);
        assert_eq!(s.axis, moved);
    }
}
