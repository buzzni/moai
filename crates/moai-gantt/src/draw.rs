//! 날짜 칸 한 줄 그리기. 칠하는 차례(아래가 위를 덮는다)는 이슈 moai-v53u.7qp 가 정한 것이다:
//! 빈 칸 → 주말 바탕 → 오늘 → 계획 → 실제 → 기한 넘김 → 표식 → 연결선(빈 칸에만).
//!
//! 막대가 차지하는 칸은 [`crate::GanttRow::span`] 과 [`crate::bar_cells`] 가 맞히기에 쓰는 것과
//! 같은 합집합 안에 든다 — 시험이 그것을 견준다.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::axis::{Axis, Header};
use crate::style::{Glyphs, Palette};
use crate::{GanttRow, RowKind};

/// 한 줄을 그리는 데 드는 것을 묶는다 — 인자가 열이 넘어가지 않게.
pub(crate) struct Ctx<'a> {
    pub axis: &'a Axis,
    pub header: &'a Header,
    pub now: i64,
    pub palette: &'a Palette,
    pub glyphs: &'a Glyphs,
    /// 화면 맨 위 본문 줄의 번호.
    pub first_row: usize,
}

/// 칸 하나를 덮어쓴다. 앞 스타일(글자색·꾸밈)은 지우되 바탕은 이어받아 주말 바탕이 막대 밑에서 산다.
fn put(buf: &mut Buffer, x: u16, y: u16, ch: char, style: Style) {
    let cell = &mut buf[(x, y)];
    let bg = cell.bg;
    cell.set_char(ch);
    cell.set_style(Style::reset().patch(style).bg(style.bg.unwrap_or(bg)));
}

/// 날짜 칸 좌표 `col`(음수·넘침 가능) 을 영역 안일 때만 칸 좌표로 바꾼다.
fn at(area: Rect, col: i64) -> Option<u16> {
    (0..i64::from(area.width)).contains(&col).then(|| area.x + col as u16)
}

/// 칸이 아직 비었나 — 연결선이 막대 위를 안 긋게.
pub(crate) fn is_empty_cell(buf: &Buffer, x: u16, y: u16) -> bool {
    matches!(buf[(x, y)].symbol(), " " | "┊" | ":")
}

pub(crate) fn row(buf: &mut Buffer, area: Rect, r: &GanttRow, c: &Ctx) {
    let (pal, g, y) = (c.palette, c.glyphs, area.y);
    // 주말 바탕 → 오늘.
    for (col, we) in c.header.weekend.iter().enumerate() {
        if *we && let Some(x) = at(area, col as i64) {
            buf[(x, y)].set_style(pal.weekend);
        }
    }
    if let Some(x) = c.header.today.and_then(|t| at(area, i64::from(t))) {
        put(buf, x, y, g.today, pal.today);
    }
    let cell = |t: i64| c.axis.cell_of_secs(t);
    let floor = |t: i64| cell(t).floor() as i64;

    // 계획.
    if let Some((s, e)) = r.planned {
        let (ch, st) = match (r.deferred, r.kind) {
            (true, _) => (g.deferred, pal.deferred),
            (false, RowKind::Item) => (g.planned, pal.planned),
            (false, _) => (g.planned_group, pal.planned),
        };
        let (c0, c1) = crate::span_cells(c.axis, (s, e.max(s)));
        for col in c0..c1 {
            if let Some(x) = at(area, col) {
                put(buf, x, y, ch, st);
            }
        }
    }
    // 실제. 시작을 모르면 r.actual 이 없다 — 안 한 일로도 0분으로도 그리지 않는다.
    if let Some((s, e)) = r.actual {
        actual(buf, area, c, (cell(s), cell(e.unwrap_or(c.now).max(s))), pal.tone(r.tone));
    }
    // 기한 넘김: 기한이 든 칸 다음부터 지금 칸까지 `▓`, 지금 칸이 `!`.
    if r.overdue
        && let Some(due) = r.due
    {
        // 기한 끝이 칸 경계면 다음 칸이고, 경계가 아니면(Month) 기한이 든 칸에서 시작한다.
        let a = floor(due);
        let b = floor(c.now).max(a);
        for col in a..=b {
            if let Some(x) = at(area, col) {
                let ch = if col == b { g.overdue_end } else { g.overdue };
                put(buf, x, y, ch, pal.overdue);
            }
        }
    }
    // 표식. 마일스톤과 기한만 있는 줄은 `◆` 하나로 선다.
    if let Some(due) = r.due
        && (r.kind == RowKind::Milestone || r.planned.is_none())
        && let Some(x) = at(area, floor(due - 1))
    {
        put(buf, x, y, g.due, pal.due);
    }
    // 화면 밖으로 잘림.
    if let Some((lo, hi)) = r.span(c.now) {
        if floor(lo) < 0 {
            put(buf, area.x, y, g.clip_left, pal.clip);
        }
        if cell(hi).ceil() as i64 > i64::from(area.width) && area.width > 0 {
            put(buf, area.x + area.width - 1, y, g.clip_right, pal.clip);
        }
    }
}

/// 실제 막대 `[a, b)`(칸 실수). 오른쪽 끝은 왼쪽에 붙는 8분의 1 블록이고, 왼쪽 끝은 같은 블록을
/// 앞·뒤 색을 바꿔(`REVERSED`) 그린다 — 오른쪽에 붙는 8분의 1 글리프가 `▕`·`▐` 둘뿐이라서다.
fn actual(buf: &mut Buffer, area: Rect, c: &Ctx, (a, b): (f64, f64), st: Style) {
    let g = c.glyphs;
    let b = if b <= a { a + 0.125 } else { b };
    let (ia, ib) = (a.floor() as i64, b.floor() as i64);
    let sole = ia == ib;
    for col in ia.max(-1)..=ib.min(i64::from(area.width)) {
        let s = if col == ia { a - ia as f64 } else { 0.0 };
        let e = if col == ib { b - ib as f64 } else { 1.0 };
        if e <= s {
            continue;
        }
        let (Some(x), y) = (at(area, col), area.y) else { continue };
        let (ns, ne) = ((s * 8.0).round() as usize, (e * 8.0).round() as usize);
        let Some(eighths) = g.eighths else {
            // 끝 조각을 못 그리는 글리프 집합: 반 칸 넘게 걸친 칸만 채운다(한 칸짜리는 늘 채운다).
            if e - s >= 0.5 || sole {
                put(buf, x, y, g.actual, st);
            }
            continue;
        };
        let left = |n: usize| eighths[n.clamp(1, 7) - 1];
        match (ns, ne) {
            (0, 8) => put(buf, x, y, g.actual, st),
            // 오른쪽 끝: 왼쪽에서 `ne` 조각. 아주 짧아도 한 조각은 남긴다.
            (0, n) => put(buf, x, y, left(n), st),
            // 왼쪽 끝: 비는 앞 `ns` 조각을 뒤집어 그린다. 거의 다 비면(8) 칸을 안 칠한다.
            (n, 8) if n < 8 => put(buf, x, y, left(n), st.add_modifier(Modifier::REVERSED)),
            (_, 8) => {}
            // 한 칸 안에 든 짧은 막대: 왼쪽 정렬로 길이만 맞춘다.
            (s, e) => put(buf, x, y, left(e.saturating_sub(s)), st),
        }
    }
}
