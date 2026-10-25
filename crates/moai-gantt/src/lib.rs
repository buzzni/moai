//! ratatui 용 간트 차트 위젯.
//!
//! moai 의 타입을 모른다. 시각은 "지역 기준 초"(`i64`), 날짜는 "지역 기준 날 번호"(`i64`)로
//! 받고, 시간대 이동은 부르는 쪽이 한다. 색은 [`Palette`], 글리프는 [`Glyphs`] 로 받는다.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;

pub mod axis;
pub mod civil;
mod draw;
pub mod hit;
pub mod state;
pub mod style;

pub use axis::{Axis, Header, HeaderStyle, Label, Names, Zoom, iso_week, render_header, weekday};
pub use civil::{civil_from_days, days_from_civil};
pub use hit::{HEADER_ROWS, Hit, Layout, Part, bar_cells, drag_apply, drag_days, hit, span_cells};
pub use state::{DEFAULT_LABEL_WIDTH, GanttState};
pub use style::{Glyphs, Palette, Tone};

/// 줄의 종류. 계획 막대의 글리프(`░` 이슈, `▒` 묶음)와 `◆` 표식을 가른다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RowKind {
    #[default]
    Item,
    /// 에픽 같은 묶음.
    Group,
    /// 마일스톤 — 기한에 늘 `◆` 가 선다.
    Milestone,
}

/// 줄 하나. 시각은 지역 기준 초고, 모르는 것은 `None` 이다 — 모르는 것은 그리지 않는다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GanttRow {
    /// 고정 칸의 글. 스팬 색은 부르는 쪽이 정하고, 칸 폭에서 잘린다.
    pub label: Line<'static>,
    pub kind: RowKind,
    /// 실제 막대의 색을 고르는 칸.
    pub tone: Tone,
    /// 계획 `[시작, 기한 끝]`. 기한 끝은 기한이 든 날의 끝 시각(미포함)이다.
    pub planned: Option<(i64, i64)>,
    /// 실제 `(시작, 끝)`. 끝이 `None` 이면 아직 진행 중이라 [`Gantt::now`] 까지 그린다.
    /// 시작을 모르는 줄(`started_at` 없음)은 `None` 으로 둔다 — 실제 막대를 그리지 않는다.
    pub actual: Option<(i64, Option<i64>)>,
    /// 기한 — 계획의 끝과 같은 규약(그날의 끝 시각, 미포함). 계획 없이 기한만 있으면 `◆` 하나로 선다.
    pub due: Option<i64>,
    /// 기한을 넘겼나. 참이면 기한 다음 칸부터 `now` 까지 `▓`, 끝에 `!`. 판단은 부르는 쪽이 한다.
    pub overdue: bool,
    /// 미룬 줄 — 계획을 `┄` 로 흐리게 그린다.
    pub deferred: bool,
}

/// 행 사이의 의존. 행 번호는 `rows` 안의 자리다. 선은 뒤 이슈(moai-v53u.4xi)가 그린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub from_row: usize,
    pub to_row: usize,
}

/// 간트 위젯.
#[derive(Debug, Clone, Default)]
pub struct Gantt<'a> {
    pub rows: &'a [GanttRow],
    pub links: &'a [Link],
    /// 지금(지역 기준 초). 오늘 선·진행 중인 실제·기한 넘김의 끝이다.
    pub now: i64,
    /// 커서 줄의 번호(`rows` 안). 고정 칸 바탕이 바뀐다.
    pub focus: Option<usize>,
    /// 고정 칸 머리글(첫 줄).
    pub title: Line<'a>,
    /// 달 이름. 없으면 영어 약어다.
    pub names: Option<&'a Names>,
    pub palette: Palette,
    pub glyphs: Glyphs,
}

impl StatefulWidget for Gantt<'_> {
    type State = GanttState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut GanttState) {
        let l = Layout::of(area, state);
        let default_names;
        let names = match self.names {
            Some(n) => n,
            None => {
                default_names = Names::default();
                &default_names
            }
        };
        let header = state.axis.header(l.dates_body.width, self.now, names);
        let (pal, g) = (&self.palette, &self.glyphs);

        for y in l.splitter.top()..l.splitter.bottom() {
            buf[(l.splitter.x, y)].set_char(g.splitter).set_style(pal.splitter);
        }
        if l.labels_header.height > 0 {
            buf.set_line(l.labels_header.x, l.labels_header.y, &self.title, l.labels_header.width);
        }
        render_header(buf, l.dates_header, &header, &pal.header_style());

        let ctx = draw::Ctx { axis: &state.axis, header: &header, now: self.now, palette: pal, glyphs: g };
        let body = self.rows.iter().enumerate().skip(state.first_row).zip(l.dates_body.rows());
        for ((i, r), line) in body {
            let y = line.y;
            let labels = Rect::new(l.labels_body.x, y, l.labels_body.width, 1);
            if self.focus == Some(i) {
                buf.set_style(labels, pal.focus);
            }
            buf.set_line(labels.x, y, &r.label, labels.width);
            draw::row(buf, line, r, &ctx);
        }
        self.paint_links(buf, &l, &ctx);
    }
}

impl Gantt<'_> {
    /// 연결선 층 — 가장 위, **빈 칸에만** 긋는다([`draw::is_empty_cell`] 이 빈 칸을 가린다).
    /// 선 셈은 moai-v53u.4xi 가 여기에 채운다: `l.dates_body` 의 칸 `(x, y)` 는 `rows` 의
    /// `first_row + (y - dates_body.y)` 번 줄이고, 막대가 차지하는 칸은 [`GanttRow::span`] 과
    /// [`span_cells`] 가 맞히기와 같은 값으로 준다.
    fn paint_links(&self, _buf: &mut Buffer, _layout: &Layout, _ctx: &draw::Ctx) {
        let _ = (self.links, draw::is_empty_cell);
    }
}

#[cfg(test)]
mod tests;
