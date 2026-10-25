//! 화면 배치와 맞히기, 끌기 셈. 그리기와 맞히기가 같은 [`Layout`] 을 쓰므로 어긋날 수 없다.
//!
//! 전부 순수 함수다 — 좌표는 터미널 절대 좌표(`area` 가 (0,0) 이 아니어도 된다)이고, 값을 쓰는 일은
//! 부르는 쪽이 한다.

use ratatui::layout::{Position, Rect};

use crate::GanttRow;
use crate::axis::Zoom;
use crate::state::GanttState;

const DAY: i64 = 86_400;

/// 머리글 줄 수: 위 줄·아래 줄·오늘(`▼`). [`crate::Axis::header`] 의 세 줄과 같다.
pub const HEADER_ROWS: u16 = 3;

/// 위젯 영역의 칸 나누기. 왼쪽 고정 칸, 한 칸짜리 분할선, 오른쪽 날짜 칸이고, 위 세 줄이 머리글이다.
/// 분할선은 머리글 줄에도 이어진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub labels_header: Rect,
    pub labels_body: Rect,
    /// 분할선 한 열. 머리글부터 맨 아래까지다.
    pub splitter: Rect,
    pub dates_header: Rect,
    pub dates_body: Rect,
}

impl Layout {
    /// `label_width` 는 분할선 한 칸을 남기도록 잘린다. 영역이 아주 좁으면 날짜 칸이 0 폭이 된다.
    pub fn new(area: Rect, label_width: u16) -> Self {
        let lw = label_width.min(area.width.saturating_sub(1));
        let sx = area.x + lw;
        let dw = area.width.saturating_sub(lw + 1);
        let hh = HEADER_ROWS.min(area.height);
        let bh = area.height - hh;
        let by = area.y + hh;
        Layout {
            labels_header: Rect::new(area.x, area.y, lw, hh),
            labels_body: Rect::new(area.x, by, lw, bh),
            splitter: Rect::new(sx, area.y, area.width.min(1), area.height),
            dates_header: Rect::new(sx + 1, area.y, dw, hh),
            dates_body: Rect::new(sx + 1, by, dw, bh),
        }
    }

    /// 상태의 고정 칸 폭으로 나눈다.
    pub fn of(area: Rect, state: &GanttState) -> Self {
        Self::new(area, state.label_width)
    }

    /// 날짜 칸의 폭 — 맞춤·쪽 이동이 쓴다.
    pub fn dates_width(&self) -> u16 {
        self.dates_body.width
    }
}

/// 막대의 어느 부분인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Start,
    Body,
    End,
}

/// 맞힌 결과. `row` 는 부르는 쪽 목록의 줄 번호(`first_row` 를 더한 값)다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// 위젯 밖이거나, 마지막 줄 아래, 줄이 아닌 자리.
    None,
    /// 막대. 한 칸짜리는 `Body`(끝을 잡을 수 없다), 두 칸은 `Start`·`End`, 세 칸 이상은 양 끝 한 칸씩이
    /// `Start`·`End` 고 나머지가 `Body` 다. 화면 밖으로 잘린 끝은 끝이 아니라서 `Body` 다.
    Bar { row: usize, part: Part },
    /// 막대 없는 날짜 칸. `day` 는 그 칸이 든 날 번호다.
    Empty { row: usize, day: i64 },
    /// 머리글(날짜 축). 끌면 좌우로 밀린다.
    Axis { day: i64 },
    /// 고정 칸과 날짜 칸 사이의 선.
    Splitter,
    /// 고정 칸의 줄.
    Label { row: usize },
}

impl GanttRow {
    /// 맞히기가 보는 막대의 크기 `[시작, 끝]`: 그려지는 것 전부의 **합집합**이다 — 계획, 실제(끝이
    /// 없으면 `now` 까지), 기한 표식(기한이 든 날의 칸), 기한 넘김(기한~`now`). 어느 하나라도 있으면
    /// 그 칸을 누를 수 있어야 하고, 막대 없이 표식뿐인 칸도 누를 수 있다. 끝이 시작보다 앞이면 시작으로
    /// 맞춘다. 그리는 쪽이 모양을 바꾸면 이 함수를 같이 바꾼다(`draw` 시험이 그리기와 견준다).
    pub fn span(&self, now: i64) -> Option<(i64, i64)> {
        let mut parts: Vec<(i64, i64)> = Vec::with_capacity(4);
        if let Some((s, e)) = self.planned {
            parts.push((s, e.max(s)));
        }
        if let Some((s, e)) = self.actual {
            parts.push((s, e.unwrap_or(now).max(s)));
        }
        if let Some(d) = self.due {
            // 기한은 그날 끝 시각(미포함)이다 — 표식은 그날의 마지막 칸에 선다.
            parts.push((d - 1, d));
            if self.overdue {
                // `!` 는 지금이 든 칸에 서므로 끝을 1초 넘겨 그 칸을 포함시킨다.
                parts.push((d, now.max(d) + 1));
            }
        }
        let lo = parts.iter().map(|p| p.0).min()?;
        let hi = parts.iter().map(|p| p.1).max()?;
        Some((lo, hi))
    }
}

/// 막대가 차지하는 칸 `[첫 칸, 끝 칸)` (왼쪽이 화면 밖이면 음수). 시각이 한 점이어도 한 칸은 차지한다.
pub fn span_cells(axis: &crate::Axis, span: (i64, i64)) -> (i64, i64) {
    let c0 = axis.cell_of_secs(span.0).floor() as i64;
    let c1 = axis.cell_of_secs(span.1).ceil() as i64;
    (c0, c1.max(c0 + 1))
}

/// [`span_cells`] 를 보기 상태의 축으로.
pub fn bar_cells(state: &GanttState, span: (i64, i64)) -> (i64, i64) {
    span_cells(&state.axis, span)
}

/// `(x, y)` 에 무엇이 있는가. `now` 는 끝나지 않은 실제와 기한 넘김의 끝이다. `rows` 는 부르는 쪽 목록 전체이고, 본문 첫 줄이 `state.first_row` 다.
pub fn hit(area: Rect, state: &GanttState, rows: &[GanttRow], now: i64, x: u16, y: u16) -> Hit {
    let p = Position { x, y };
    if !area.contains(p) {
        return Hit::None;
    }
    let l = Layout::of(area, state);
    if l.splitter.contains(p) {
        return Hit::Splitter;
    }
    if l.dates_header.contains(p) {
        return Hit::Axis { day: state.axis.day_at_cell(f64::from(x - l.dates_header.x)) };
    }
    let in_labels = l.labels_body.contains(p);
    if !in_labels && !l.dates_body.contains(p) {
        return Hit::None; // 고정 칸 머리글
    }
    let row = state.first_row + usize::from(y - l.dates_body.y.max(l.labels_body.y));
    let Some(r) = rows.get(row) else { return Hit::None };
    if in_labels {
        return Hit::Label { row };
    }
    let col = i64::from(x - l.dates_body.x);
    if let Some(span) = r.span(now) {
        let (c0, c1) = bar_cells(state, span);
        if (c0..c1).contains(&col) {
            let n = c1 - c0;
            let part = if n < 2 {
                Part::Body
            } else if col == c0 {
                Part::Start
            } else if col == c1 - 1 {
                Part::End
            } else {
                Part::Body
            };
            return Hit::Bar { row, part };
        }
    }
    Hit::Empty { row, day: state.axis.day_at_cell(col as f64) }
}

/// 끌기의 날 수: 누른 칸과 지금 칸의 차이를 날로 바꿔 **가장 가까운 정수 날**로 반올림한다(반은 0 에서
/// 멀어지는 쪽). 그래서 Month(칸 하나가 7일)는 칸마다 7일, Week 는 1일, Day(6시간 칸)는 두 칸이 1일이고
/// 한 칸은 0일, Hour 는 열두 칸이 1일이다. 하루보다 잘게 놓는 일은 없다 — 막대의 값이 날 단위라서다.
pub fn drag_days(zoom: Zoom, press_col: i64, cur_col: i64) -> i64 {
    let secs = (cur_col - press_col) * zoom.secs_per_cell();
    (secs as f64 / DAY as f64).round() as i64
}

/// 끌어서 나온 새 `[시작, 끝]`(시각). `Body` 는 둘을 함께, `Start`·`End` 는 한쪽만 `days` 날 옮긴다.
/// 시작이 끝을 넘는 끌기는 **막는다**(`None`) — 눌러 둔 값 그대로 두라는 뜻이다. 같은 값(0 길이)은 된다.
/// 시각의 시·분은 그대로 두고 날만 옮긴다.
pub fn drag_apply(part: Part, span: (i64, i64), days: i64) -> Option<(i64, i64)> {
    let d = days * DAY;
    let (s, e) = match part {
        Part::Body => (span.0 + d, span.1 + d),
        Part::Start => (span.0 + d, span.1),
        Part::End => (span.0, span.1 + d),
    };
    (s <= e).then_some((s, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axis::Axis;

    fn row(s: i64, e: i64) -> GanttRow {
        GanttRow { planned: Some((s, e)), ..GanttRow::default() }
    }

    fn st(zoom: Zoom, origin: i64) -> GanttState {
        let mut s = GanttState::new(Axis::new(zoom, origin));
        s.label_width = 10;
        s
    }

    // 영역 (5,2) 크기 40x10: 고정 칸 x 5..15, 분할선 x 15, 날짜 칸 x 16..45. 머리글 y 2..5, 본문 y 5..12.
    const AREA: Rect = Rect { x: 5, y: 2, width: 40, height: 10 };

    #[test]
    fn layout_splits_labels_splitter_dates() {
        let l = Layout::new(AREA, 10);
        assert_eq!(l.labels_body, Rect::new(5, 5, 10, 7));
        assert_eq!(l.splitter, Rect::new(15, 2, 1, 10));
        assert_eq!(l.dates_header, Rect::new(16, 2, 29, 3));
        assert_eq!(l.dates_body, Rect::new(16, 5, 29, 7));
        assert_eq!(l.dates_width(), 29);
        // 고정 칸이 영역보다 넓으면 분할선을 남기고 잘린다.
        let l = Layout::new(AREA, 99);
        assert_eq!(l.labels_body.width, 39);
        assert_eq!(l.dates_body.width, 0);
        assert_eq!(l.splitter.x, 44);
    }

    #[test]
    fn bar_parts_at_week_zoom() {
        // Week: 하루 한 칸. 날 10 의 칸 = 0. 막대 날 12..15 → 칸 2..5.
        let s = st(Zoom::Week, 10 * DAY);
        let rows = [row(12 * DAY, 15 * DAY)];
        let at = |col: u16| hit(AREA, &s, &rows, 0, 16 + col, 5);
        assert_eq!(at(1), Hit::Empty { row: 0, day: 11 });
        assert_eq!(at(2), Hit::Bar { row: 0, part: Part::Start });
        assert_eq!(at(3), Hit::Bar { row: 0, part: Part::Body });
        assert_eq!(at(4), Hit::Bar { row: 0, part: Part::End });
        assert_eq!(at(5), Hit::Empty { row: 0, day: 15 });
    }

    #[test]
    fn short_bars_have_no_end_to_grab_when_one_cell() {
        let s = st(Zoom::Week, 10 * DAY);
        // 한 점(12일 정각): 한 칸 Body.
        let rows = [row(12 * DAY, 12 * DAY), row(12 * DAY, 14 * DAY)];
        assert_eq!(hit(AREA, &s, &rows, 0, 18, 5), Hit::Bar { row: 0, part: Part::Body });
        assert_eq!(hit(AREA, &s, &rows, 0, 19, 5), Hit::Empty { row: 0, day: 13 });
        // 두 칸: Start·End.
        assert_eq!(hit(AREA, &s, &rows, 0, 18, 6), Hit::Bar { row: 1, part: Part::Start });
        assert_eq!(hit(AREA, &s, &rows, 0, 19, 6), Hit::Bar { row: 1, part: Part::End });
    }

    #[test]
    fn hit_follows_each_zoom() {
        let o = 100 * DAY;
        let rows = [row(o + DAY, o + 2 * DAY)]; // 하루 길이
        let area = Rect::new(5, 2, 100, 10);
        // Hour: 24칸 — 24..48.
        let s = st(Zoom::Hour, o);
        assert_eq!(hit(area, &s, &rows, 0, 16 + 24, 5), Hit::Bar { row: 0, part: Part::Start });
        assert_eq!(hit(area, &s, &rows, 0, 16 + 47, 5), Hit::Bar { row: 0, part: Part::End });
        // Day: 4칸 — 4..8, 날 101 은 칸 4..8.
        let s = st(Zoom::Day, o);
        assert_eq!(hit(area, &s, &rows, 0, 16 + 4, 5), Hit::Bar { row: 0, part: Part::Start });
        assert_eq!(hit(area, &s, &rows, 0, 16 + 7, 5), Hit::Bar { row: 0, part: Part::End });
        assert_eq!(hit(area, &s, &rows, 0, 16 + 8, 5), Hit::Empty { row: 0, day: 102 });
        // Month: 한 칸이 7일. 하루 막대는 한 칸 (칸 0, 날 101 은 칸 0 에 든다).
        let s = st(Zoom::Month, o);
        assert_eq!(hit(area, &s, &rows, 0, 16, 5), Hit::Bar { row: 0, part: Part::Body });
        assert_eq!(hit(area, &s, &rows, 0, 17, 5), Hit::Empty { row: 0, day: 107 });
    }

    #[test]
    fn a_bar_clipped_at_the_left_has_no_start_cell() {
        let s = st(Zoom::Week, 10 * DAY);
        let rows = [row(5 * DAY, 12 * DAY)]; // 칸 -5..2
        assert_eq!(hit(AREA, &s, &rows, 0, 16, 5), Hit::Bar { row: 0, part: Part::Body });
        assert_eq!(hit(AREA, &s, &rows, 0, 17, 5), Hit::Bar { row: 0, part: Part::End });
    }

    #[test]
    fn rows_offset_by_first_row_and_label_column() {
        let mut s = st(Zoom::Week, 0);
        s.first_row = 3;
        let rows: Vec<_> = (0..5).map(|_| row(0, DAY)).collect();
        assert_eq!(hit(AREA, &s, &rows, 0, 6, 5), Hit::Label { row: 3 });
        assert_eq!(hit(AREA, &s, &rows, 0, 6, 6), Hit::Label { row: 4 });
        // 5번 줄은 없다.
        assert_eq!(hit(AREA, &s, &rows, 0, 6, 7), Hit::None);
        assert_eq!(hit(AREA, &s, &rows, 0, 20, 7), Hit::None);
        // 날짜가 없는 줄은 막대 없이 Empty.
        let rows = [GanttRow::default()];
        s.first_row = 0;
        assert_eq!(hit(AREA, &s, &rows, 0, 16, 5), Hit::Empty { row: 0, day: 0 });
    }

    #[test]
    fn header_splitter_and_edges() {
        let s = st(Zoom::Week, 10 * DAY);
        let rows = [row(0, DAY)];
        for y in 2..5 {
            assert_eq!(hit(AREA, &s, &rows, 0, 16 + 3, y), Hit::Axis { day: 13 });
        }
        // 분할선은 머리글 줄에도 이어진다.
        assert_eq!(hit(AREA, &s, &rows, 0, 15, 2), Hit::Splitter);
        assert_eq!(hit(AREA, &s, &rows, 0, 15, 11), Hit::Splitter);
        // 고정 칸 머리글은 아무것도 아니다.
        assert_eq!(hit(AREA, &s, &rows, 0, 6, 3), Hit::None);
        // 영역 밖: 왼쪽·위·오른쪽 끝 다음·아래 끝 다음.
        assert_eq!(hit(AREA, &s, &rows, 0, 4, 5), Hit::None);
        assert_eq!(hit(AREA, &s, &rows, 0, 20, 1), Hit::None);
        assert_eq!(hit(AREA, &s, &rows, 0, 45, 5), Hit::None);
        assert_eq!(hit(AREA, &s, &rows, 0, 20, 12), Hit::None);
        // 가장자리 칸은 안이다.
        assert_eq!(hit(AREA, &s, &rows, 0, 44, 5), Hit::Empty { row: 0, day: 10 + 28 });
    }

    #[test]
    fn a_squeezed_area_never_panics() {
        let s = st(Zoom::Week, 0);
        let rows = [row(0, DAY)];
        let tiny = Rect::new(3, 3, 1, 2);
        for x in 0..6 {
            for y in 0..8 {
                let _ = hit(tiny, &s, &rows, 0, x, y);
            }
        }
        assert_eq!(hit(Rect::new(0, 0, 0, 0), &s, &rows, 0, 0, 0), Hit::None);
    }

    #[test]
    fn drag_days_follow_the_zoom() {
        assert_eq!(drag_days(Zoom::Week, 4, 9), 5);
        assert_eq!(drag_days(Zoom::Week, 9, 4), -5);
        assert_eq!(drag_days(Zoom::Month, 0, 2), 14);
        // Day: 4칸 = 1일. 1칸은 0일, 2칸은 반올림해 1일, 6칸은 1.5 → 2일.
        assert_eq!(drag_days(Zoom::Day, 0, 1), 0);
        assert_eq!(drag_days(Zoom::Day, 0, 2), 1);
        assert_eq!(drag_days(Zoom::Day, 0, 4), 1);
        assert_eq!(drag_days(Zoom::Day, 0, 6), 2);
        assert_eq!(drag_days(Zoom::Day, 0, -2), -1);
        // Hour: 24칸 = 1일.
        assert_eq!(drag_days(Zoom::Hour, 0, 11), 0);
        assert_eq!(drag_days(Zoom::Hour, 0, 12), 1);
        assert_eq!(drag_days(Zoom::Hour, 0, 48), 2);
    }

    #[test]
    fn drag_apply_moves_body_together_and_one_end_alone() {
        let span = (10 * DAY + 3_600, 14 * DAY);
        assert_eq!(drag_apply(Part::Body, span, 2), Some((12 * DAY + 3_600, 16 * DAY)));
        assert_eq!(drag_apply(Part::Start, span, -1), Some((9 * DAY + 3_600, 14 * DAY)));
        assert_eq!(drag_apply(Part::End, span, 3), Some((10 * DAY + 3_600, 17 * DAY)));
        assert_eq!(drag_apply(Part::Body, span, 0), Some(span));
    }

    #[test]
    fn drag_apply_refuses_start_after_end() {
        let span = (10 * DAY, 12 * DAY);
        assert_eq!(drag_apply(Part::Start, span, 3), None);
        assert_eq!(drag_apply(Part::End, span, -3), None);
        // 같아지는 것은 된다.
        assert_eq!(drag_apply(Part::Start, span, 2), Some((12 * DAY, 12 * DAY)));
        assert_eq!(drag_apply(Part::End, span, -2), Some((10 * DAY, 10 * DAY)));
        // 몸통은 길이가 그대로라 늘 된다.
        assert!(drag_apply(Part::Body, span, -50).is_some());
    }
}
