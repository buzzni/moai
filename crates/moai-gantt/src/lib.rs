//! ratatui 용 간트 차트 위젯 (뼈대).
//!
//! moai 의 타입을 모른다. 시각은 "지역 기준 초"(`i64`), 날짜는 "지역 기준 날 번호"(`i64`)로
//! 받고, 시간대 이동은 부르는 쪽이 한다. 아래 타입은 자리만 잡은 것이고 뒤 이슈들이 채운다.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;

pub mod axis;
pub mod civil;
pub mod hit;
pub mod state;

pub use axis::{Axis, Header, HeaderStyle, Label, Names, Zoom, iso_week, render_header, weekday};
pub use civil::{civil_from_days, days_from_civil};
pub use hit::{HEADER_ROWS, Hit, Layout, Part, bar_cells, drag_apply, drag_days, hit};
pub use state::{DEFAULT_LABEL_WIDTH, GanttState};

/// 가로 시간축. 보이는 날 범위를 든다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timeline {
    /// 첫 날 번호 (포함).
    pub start_day: i64,
    /// 끝 날 번호 (미포함).
    pub end_day: i64,
}

/// 막대 하나.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GanttRow {
    pub label: String,
    /// 지역 기준 초. 모르면 `None`.
    pub start: Option<i64>,
    pub end: Option<i64>,
}

/// 행 사이의 의존. 행 번호는 `rows` 안의 자리다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub from_row: usize,
    pub to_row: usize,
}

/// 간트 위젯. 아직 아무것도 그리지 않는다.
#[derive(Debug, Clone, Default)]
pub struct Gantt<'a> {
    pub timeline: Timeline,
    pub rows: &'a [GanttRow],
    pub links: &'a [Link],
}

impl StatefulWidget for Gantt<'_> {
    type State = GanttState;

    fn render(self, _area: Rect, _buf: &mut Buffer, _state: &mut GanttState) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_renders_nothing() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let mut st = GanttState::default();
        Gantt::default().render(buf.area, &mut buf, &mut st);
        assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 4, 1)));
    }
}
