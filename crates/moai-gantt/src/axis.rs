//! 시간 축 — 확대 단계, 시각 ↔ 칸, 머리글. 터미널 없이 시험하는 순수 셈이다.
//!
//! 시각은 지역 기준 초, 날은 지역 기준 날 번호다(시간대는 부르는 쪽 몫). 하루는 칸 하나가
//! 아니라 칸 **범위**다 — 날 N 의 칸은 `[cell(N)..cell(N+1))` 이고, 그래서 칸 → 날은 내림이다.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::civil::{civil_from_days, days_from_civil};

/// 하루의 초. 크레이트가 이 한 곳에서 읽는다.
pub(crate) const DAY: i64 = 86_400;

/// 확대 단계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    Hour,
    Day,
    Week,
    Month,
}

impl Zoom {
    /// 거친 쪽에서 고운 쪽 차례. 확대 키가 이 줄을 오간다.
    pub const ALL: [Zoom; 4] = [Zoom::Month, Zoom::Week, Zoom::Day, Zoom::Hour];

    /// 칸 하나가 몇 초인가 — 단계마다의 칸 수는 여기 한 곳에서만 정한다.
    /// Hour 는 하루 24칸, Day 는 하루 4칸(6시간), Week 는 하루 1칸, Month 는 한 칸이 1주.
    pub const fn secs_per_cell(self) -> i64 {
        match self {
            Zoom::Hour => 3_600,
            Zoom::Day => 6 * 3_600,
            Zoom::Week => DAY,
            Zoom::Month => 7 * DAY,
        }
    }

    /// 하루가 몇 칸인가 (Month 는 1/7).
    pub fn cells_per_day(self) -> f64 {
        DAY as f64 / self.secs_per_cell() as f64
    }

    /// 한 단계 더 고운 쪽. 끝이면 제자리.
    pub fn finer(self) -> Zoom {
        let i = Self::ALL.iter().position(|z| *z == self).unwrap_or(0);
        Self::ALL[(i + 1).min(Self::ALL.len() - 1)]
    }

    /// 한 단계 더 거친 쪽. 끝이면 제자리.
    pub fn coarser(self) -> Zoom {
        let i = Self::ALL.iter().position(|z| *z == self).unwrap_or(0);
        Self::ALL[i.saturating_sub(1)]
    }

    /// 주말 바탕은 하루가 여러 칸인 단계에서만 뜻이 있다.
    fn shows_weekend(self) -> bool {
        matches!(self, Zoom::Hour | Zoom::Day)
    }
}

/// 달 이름. 부르는 쪽이 i18n 으로 채우고, 기본은 영어 약어다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub months: [String; 12],
}

impl Default for Names {
    fn default() -> Self {
        const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        Names { months: EN.map(String::from) }
    }
}

/// 보기 원점과 확대. `origin` 은 첫 칸이 시작하는 시각이다 — 가로로 밀면 이 값이 바뀐다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Axis {
    pub zoom: Zoom,
    pub origin: i64,
}

impl Axis {
    pub fn new(zoom: Zoom, origin: i64) -> Self {
        Axis { zoom, origin }
    }

    /// 시각 → 칸(실수). 정수부가 칸 번호, 소수부가 칸 안의 위치다.
    pub fn cell_of_secs(&self, t: i64) -> f64 {
        (t - self.origin) as f64 / self.zoom.secs_per_cell() as f64
    }

    /// 날 번호 → 그날이 차지하는 칸 범위 `[시작, 끝)`.
    pub fn cells_of_day(&self, day: i64) -> (f64, f64) {
        (self.cell_of_secs(day * DAY), self.cell_of_secs((day + 1) * DAY))
    }

    /// 칸(실수) → 시각. 맞히기가 쓴다.
    pub fn secs_at_cell(&self, cell: f64) -> i64 {
        self.origin + (cell * self.zoom.secs_per_cell() as f64).floor() as i64
    }

    /// 칸 → 그 칸이 든 날 번호.
    pub fn day_at_cell(&self, cell: f64) -> i64 {
        self.secs_at_cell(cell).div_euclid(DAY)
    }

    /// `n` 칸 가로로 민다 (양수면 미래 쪽).
    pub fn scroll(&mut self, n: i64) {
        self.origin += n * self.zoom.secs_per_cell();
    }

    /// 확대를 바꾸되 `anchor` 시각이 같은 화면 칸 `col` 에 남게 원점을 다시 잡는다.
    /// 기준점은 커서 줄의 막대나 화면 가운데를 부르는 쪽이 고른다.
    pub fn with_zoom(&self, zoom: Zoom, anchor: i64, col: f64) -> Axis {
        let origin = anchor - (col * zoom.secs_per_cell() as f64).round() as i64;
        Axis { zoom, origin }
    }

    /// `now` 가 든 칸. 화면 `width` 밖이면 `None`.
    pub fn today_col(&self, now: i64, width: u16) -> Option<u16> {
        let c = self.cell_of_secs(now).floor();
        (c >= 0.0 && c < f64::from(width)).then_some(c as u16)
    }

    /// 머리글 두 줄과 오늘·주말 정보. `now` 는 오늘 표시에만 쓴다.
    pub fn header(&self, width: u16, now: i64, names: &Names) -> Header {
        let (up, low) = match self.zoom {
            Zoom::Hour => (Unit::DayHead, Unit::Hour3),
            Zoom::Day => (Unit::MonthYear, Unit::Day),
            Zoom::Week => (Unit::MonthYear, Unit::IsoWeek),
            Zoom::Month => (Unit::Year, Unit::Month),
        };
        let weekend = (0..width)
            .map(|c| {
                self.zoom.shows_weekend() && {
                    let dow = weekday(self.day_at_cell(f64::from(c)));
                    dow >= 5
                }
            })
            .collect();
        Header {
            upper: self.labels(up, width, names),
            lower: self.labels(low, width, names),
            weekend,
            today: self.today_col(now, width),
        }
    }

    /// 단위 하나의 라벨들. 보이는 폭(왼쪽이 잘렸으면 0 부터)에 통째로 안 들어가는 글은 뺀다.
    fn labels(&self, unit: Unit, width: u16, names: &Names) -> Vec<Label> {
        let spc = self.zoom.secs_per_cell();
        let end = self.origin + i64::from(width) * spc;
        let col = |t: i64| self.cell_of_secs(t).round() as i64;
        let mut out: Vec<Label> = Vec::new();
        let mut free = 0i64; // 앞 라벨이 끝난 칸. 이보다 왼쪽에는 못 놓는다.
        let mut start = unit.floor(self.origin);
        while start < end {
            let next = unit.next(start);
            let from = col(start).max(0).max(free);
            let to = col(next).min(i64::from(width));
            let text = unit.text(start, names);
            let w = Span::raw(text.as_str()).width() as i64;
            if from + w <= to {
                out.push(Label { col: from as u16, text });
                free = from + w;
            }
            start = next;
        }
        out
    }
}

/// 머리글 한 줄의 글 하나. `col` 은 영역 왼쪽부터의 칸이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub col: u16,
    pub text: String,
}

/// 머리글 데이터. 그리는 일은 [`render_header`] 와 뒤의 위젯이 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// 위 줄: 큰 단위(달·해·날).
    pub upper: Vec<Label>,
    /// 아래 줄: 작은 단위(날·ISO 주·달·3시간).
    pub lower: Vec<Label>,
    /// 칸마다 주말인가. Day·Hour 에서만 참이다 — 바탕만 바꾸는 꾸밈이다.
    pub weekend: Vec<bool>,
    /// 오늘 칸. 머리 아래 줄에 `▼`, 본문 빈 칸에는 `┊` 를 이 칸에 둔다(막대가 덮으면 막대가 이긴다).
    pub today: Option<u16>,
}

/// 머리글을 그릴 때의 꾸밈. 팔레트는 위젯이 주고, 여기는 자리만 받는다.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderStyle {
    pub label: Style,
    pub weekend: Style,
    pub today: Style,
}

/// 머리글 세 줄(위·아래·오늘)을 `area` 맨 위에 그린다. 높이가 모자라면 아래 줄부터 빠진다.
pub fn render_header(buf: &mut Buffer, area: Rect, h: &Header, st: &HeaderStyle) {
    let put = |buf: &mut Buffer, y: u16, labels: &[Label]| {
        for l in labels {
            buf.set_stringn(area.x + l.col, y, &l.text, usize::from(area.width - l.col.min(area.width)), st.label);
        }
    };
    if area.height > 0 {
        put(buf, area.y, &h.upper);
    }
    if area.height > 1 {
        put(buf, area.y + 1, &h.lower);
    }
    for (c, we) in h.weekend.iter().enumerate().take(usize::from(area.width)) {
        if *we {
            // 주말은 아래 두 줄의 바탕만 바꾼다. 글리프가 없어도 날은 라벨이 말한다.
            for y in 1..area.height.min(3) {
                buf[(area.x + c as u16, area.y + y)].set_style(st.weekend);
            }
        }
    }
    // 라벨·주말처럼 영역 폭 안에서만 그린다 — 더 넓게 지은 머리글을 받아도 영역 밖에 안 쓴다.
    if let Some(c) = h.today.filter(|c| *c < area.width)
        && area.height > 2
    {
        buf.set_string(area.x + c, area.y + 2, "▼", st.today);
    }
}

/// 라벨이 되는 단위. 위 줄과 아래 줄이 같은 틀로 돈다.
#[derive(Clone, Copy)]
enum Unit {
    Hour3,
    Day,
    DayHead,
    IsoWeek,
    Month,
    MonthYear,
    Year,
}

impl Unit {
    /// `t` 가 든 단위의 시작 시각.
    fn floor(self, t: i64) -> i64 {
        let day = t.div_euclid(DAY);
        match self {
            Unit::Hour3 => t.div_euclid(3 * 3_600) * 3 * 3_600,
            Unit::Day | Unit::DayHead => day * DAY,
            Unit::IsoWeek => (day - weekday(day)) * DAY,
            Unit::Month | Unit::MonthYear => {
                let (y, m, _) = civil_from_days(day);
                days_from_civil(y, m, 1) * DAY
            }
            Unit::Year => days_from_civil(civil_from_days(day).0, 1, 1) * DAY,
        }
    }

    /// 다음 단위의 시작 시각.
    fn next(self, start: i64) -> i64 {
        let day = start.div_euclid(DAY);
        match self {
            Unit::Hour3 => start + 3 * 3_600,
            Unit::Day | Unit::DayHead => start + DAY,
            Unit::IsoWeek => start + 7 * DAY,
            Unit::Month | Unit::MonthYear => {
                let (y, m, _) = civil_from_days(day);
                let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                days_from_civil(y, m, 1) * DAY
            }
            Unit::Year => days_from_civil(civil_from_days(day).0 + 1, 1, 1) * DAY,
        }
    }

    fn text(self, start: i64, names: &Names) -> String {
        let day = start.div_euclid(DAY);
        let (y, m, d) = civil_from_days(day);
        let mon = &names.months[m as usize - 1];
        match self {
            Unit::Hour3 => format!("{:02}", start.rem_euclid(DAY) / 3_600),
            Unit::Day => d.to_string(),
            Unit::DayHead => format!("{mon} {d}"),
            Unit::IsoWeek => format!("W{}", iso_week(day).1),
            Unit::Month => mon.clone(),
            Unit::MonthYear => format!("{mon} {y}"),
            Unit::Year => y.to_string(),
        }
    }
}

/// 요일. 월요일 0 … 일요일 6. 날 번호 0(1970-01-01)은 목요일이다.
pub fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

/// ISO 8601 (주 연도, 주 번호). 주는 월요일에 시작하고, 그 주의 목요일이 든 해가 주 연도다.
pub fn iso_week(day: i64) -> (i64, u32) {
    let thursday = day - weekday(day) + 3;
    let year = civil_from_days(thursday).0;
    (year, ((thursday - days_from_civil(year, 1, 1)) / 7 + 1) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i64, m: u32, dd: u32) -> i64 {
        days_from_civil(y, m, dd)
    }

    /// 머리글 한 줄을 글로 — 라벨을 칸에 놓는다.
    fn line(labels: &[Label], width: usize) -> String {
        let mut s: Vec<char> = vec![' '; width];
        for l in labels {
            for (i, c) in l.text.chars().enumerate() {
                s[usize::from(l.col) + i] = c;
            }
        }
        s.into_iter().collect()
    }

    /// 버퍼의 한 줄을 글로. 시험이 화면처럼 읽는다.
    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    #[test]
    fn cells_per_zoom_are_set_in_one_place() {
        assert_eq!(Zoom::Hour.cells_per_day(), 24.0);
        assert_eq!(Zoom::Day.cells_per_day(), 4.0);
        assert_eq!(Zoom::Week.cells_per_day(), 1.0);
        assert!((Zoom::Month.cells_per_day() - 1.0 / 7.0).abs() < 1e-12);
    }

    #[test]
    fn day_to_cell_range_per_zoom() {
        let o = d(2026, 10, 20) * DAY;
        let day = d(2026, 10, 22);
        for (z, start, end) in [
            (Zoom::Hour, 48.0, 72.0),
            (Zoom::Day, 8.0, 12.0),
            (Zoom::Week, 2.0, 3.0),
            (Zoom::Month, 2.0 / 7.0, 3.0 / 7.0),
        ] {
            let (s, e) = Axis::new(z, o).cells_of_day(day);
            assert!((s - start).abs() < 1e-9 && (e - end).abs() < 1e-9, "{z:?}: {s} {e}");
        }
    }

    #[test]
    fn cell_to_day_is_floor_so_a_whole_day_range_maps_back() {
        let o = d(2026, 10, 20) * DAY;
        for z in Zoom::ALL {
            let a = Axis::new(z, o);
            let day = d(2026, 10, 27);
            let (s, e) = a.cells_of_day(day);
            assert_eq!(a.day_at_cell(s), day, "{z:?} 첫 칸");
            // 끝 칸은 미포함이다 — 바로 앞은 같은 날, 끝 자체는 다음 날.
            assert_eq!(a.day_at_cell(e - 1e-6), day, "{z:?} 끝 직전");
            assert_eq!(a.day_at_cell(e), day + 1, "{z:?} 끝");
        }
    }

    #[test]
    fn cell_to_day_before_the_origin_counts_back() {
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        assert_eq!(a.day_at_cell(-1.0), d(2026, 10, 19));
        assert_eq!(a.secs_at_cell(4.0), a.origin + DAY);
    }

    #[test]
    fn zoom_keeps_the_anchor_on_the_same_column() {
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        let anchor = d(2026, 10, 24) * DAY + 9 * 3_600;
        let col = a.cell_of_secs(anchor);
        for z in Zoom::ALL {
            let b = a.with_zoom(z, anchor, col);
            let tol = 1.0 / (z.secs_per_cell() as f64); // 반올림 1초
            assert!((b.cell_of_secs(anchor) - col).abs() <= tol, "{z:?}");
        }
    }

    #[test]
    fn scroll_moves_by_whole_cells() {
        let mut a = Axis::new(Zoom::Week, 0);
        a.scroll(3);
        assert_eq!(a.origin, 3 * DAY);
        assert_eq!(a.cell_of_secs(3 * DAY), 0.0);
    }

    #[test]
    fn zoom_steps_stop_at_the_ends() {
        assert_eq!(Zoom::Day.finer(), Zoom::Hour);
        assert_eq!(Zoom::Hour.finer(), Zoom::Hour);
        assert_eq!(Zoom::Day.coarser(), Zoom::Week);
        assert_eq!(Zoom::Month.coarser(), Zoom::Month);
    }

    #[test]
    fn today_column_is_the_cell_holding_now() {
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        let now = d(2026, 10, 23) * DAY + 14 * 3_600; // 14시 → 그날의 셋째 칸
        assert_eq!(a.today_col(now, 48), Some(3 * 4 + 2));
        assert_eq!(a.today_col(now, 14), None);
        assert_eq!(a.today_col(a.origin - 1, 48), None);
    }

    #[test]
    fn iso_weeks_across_the_year_boundary() {
        assert_eq!(iso_week(d(2026, 10, 20)), (2026, 43));
        assert_eq!(iso_week(d(2026, 12, 31)), (2026, 53));
        assert_eq!(iso_week(d(2027, 1, 1)), (2026, 53));
        assert_eq!(iso_week(d(2027, 1, 4)), (2027, 1));
        assert_eq!(iso_week(d(2021, 1, 3)), (2020, 53));
        assert_eq!(iso_week(d(2024, 12, 30)), (2025, 1));
    }

    #[test]
    fn weekday_is_monday_zero() {
        assert_eq!(weekday(d(2026, 10, 19)), 0);
        assert_eq!(weekday(d(2026, 10, 24)), 5);
        assert_eq!(weekday(d(2026, 10, 25)), 6);
    }

    #[test]
    fn day_header_labels_each_day_and_the_month_once() {
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        let h = a.header(48, d(2026, 10, 23) * DAY, &Names::default());
        assert_eq!(line(&h.upper, 48), format!("{:<48}", "Oct 2026"));
        assert_eq!(line(&h.lower, 48), "20  21  22  23  24  25  26  27  28  29  30  31  ");
        // 24·25 일(토·일)만 주말이다 — 칸 16..24.
        let we: Vec<usize> = (0..48).filter(|c| h.weekend[*c]).collect();
        assert_eq!(we, (16..24).chain(44..48).collect::<Vec<_>>());
        assert_eq!(h.today, Some(12)); // 23일 0시 = 셋째 날의 첫 칸
    }

    #[test]
    fn month_rollover_drops_the_label_that_does_not_fit() {
        // 10/29 부터 4일 = 16칸. 10월은 12칸, 11월은 4칸뿐이라 "Nov 2026"(8) 은 통째로 빠진다.
        let a = Axis::new(Zoom::Day, d(2026, 10, 29) * DAY);
        let h = a.header(16, 0, &Names::default());
        assert_eq!(h.upper, vec![Label { col: 0, text: "Oct 2026".into() }]);
        assert_eq!(line(&h.lower, 16), "29  30  31  1   ");
    }

    #[test]
    fn year_rollover_keeps_both_labels_when_both_fit() {
        let a = Axis::new(Zoom::Day, d(2026, 12, 30) * DAY);
        let h = a.header(24, 0, &Names::default());
        let up: Vec<_> = h.upper.iter().map(|l| (l.col, l.text.as_str())).collect();
        assert_eq!(up, vec![(0, "Dec 2026"), (8, "Jan 2027")]);
    }

    #[test]
    fn a_label_for_a_unit_cut_on_the_left_sticks_to_column_zero() {
        // 원점이 10월 한가운데여도 10월 이름은 첫 칸에 선다.
        let a = Axis::new(Zoom::Day, d(2026, 10, 15) * DAY);
        let h = a.header(40, 0, &Names::default());
        assert_eq!(h.upper[0], Label { col: 0, text: "Oct 2026".into() });
    }

    #[test]
    fn leap_day_shows_up_in_february_2024() {
        let a = Axis::new(Zoom::Day, d(2024, 2, 28) * DAY);
        let h = a.header(16, 0, &Names::default());
        assert_eq!(line(&h.lower, 16), "28  29  1   2   ");
        // 2025 년 2월에는 29 일이 없다.
        let b = Axis::new(Zoom::Day, d(2025, 2, 28) * DAY);
        assert_eq!(line(&b.header(16, 0, &Names::default()).lower, 16), "28  1   2   3   ");
    }

    #[test]
    fn week_header_shows_iso_weeks_and_drops_the_cut_one() {
        let a = Axis::new(Zoom::Week, d(2026, 10, 19) * DAY);
        let h = a.header(14, 0, &Names::default());
        assert_eq!(line(&h.lower, 14), "W43    W44    ");
        // 폭 9 면 W44 는 둘째 주의 2칸뿐이라 통째로 뺀다 — "W4" 를 남기지 않는다.
        let h = a.header(9, 0, &Names::default());
        assert_eq!(line(&h.lower, 9), "W43      ");
        assert!(h.weekend.iter().all(|w| !w), "Week 에는 주말 바탕이 없다");
    }

    #[test]
    fn week_header_across_the_year_boundary() {
        let a = Axis::new(Zoom::Week, d(2026, 12, 28) * DAY);
        let h = a.header(14, 0, &Names::default());
        assert_eq!(line(&h.lower, 14), "W53    W1     ");
    }

    #[test]
    fn month_zoom_has_years_above_and_months_below() {
        let a = Axis::new(Zoom::Month, d(2026, 10, 5) * DAY);
        let h = a.header(40, 0, &Names::default());
        assert_eq!(h.upper[0], Label { col: 0, text: "2026".into() });
        assert!(h.upper.iter().any(|l| l.text == "2027"));
        let low: Vec<_> = h.lower.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(&low[..3], ["Oct", "Nov", "Dec"]);
    }

    #[test]
    fn hour_header_has_day_above_and_three_hour_marks_below() {
        let a = Axis::new(Zoom::Hour, d(2026, 10, 23) * DAY);
        let h = a.header(48, 0, &Names::default());
        let up: Vec<_> = h.upper.iter().map(|l| (l.col, l.text.as_str())).collect();
        assert_eq!(up, vec![(0, "Oct 23"), (24, "Oct 24")]);
        assert_eq!(line(&h.lower, 48).trim_end(), "00 03 06 09 12 15 18 21 00 03 06 09 12 15 18 21");
    }

    #[test]
    fn month_names_come_from_the_caller() {
        let mut n = Names::default();
        n.months[9] = "10월".into();
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        assert_eq!(a.header(48, 0, &n).upper[0].text, "10월 2026");
    }

    #[test]
    fn render_puts_labels_weekend_background_and_the_today_marker() {
        use ratatui::style::Color;
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        let now = d(2026, 10, 23) * DAY + 14 * 3_600;
        let h = a.header(48, now, &Names::default());
        let st = HeaderStyle { weekend: Style::new().bg(Color::DarkGray), ..Default::default() };
        let mut buf = Buffer::empty(Rect::new(0, 0, 48, 3));
        let area = buf.area;
        render_header(&mut buf, area, &h, &st);
        assert_eq!(row(&buf, 0), format!("{:<48}", "Oct 2026"));
        assert_eq!(row(&buf, 1), "20  21  22  23  24  25  26  27  28  29  30  31  ");
        assert_eq!(row(&buf, 2), format!("{:14}▼{:33}", "", ""));
        assert_eq!(buf[(16, 1)].bg, Color::DarkGray);
        assert_eq!(buf[(16, 2)].bg, Color::DarkGray);
        assert_eq!(buf[(15, 1)].bg, Color::Reset);
        assert_eq!(buf[(16, 0)].bg, Color::Reset, "위 줄은 주말 바탕을 안 받는다");
    }

    #[test]
    fn render_keeps_the_today_marker_inside_a_narrower_area() {
        // 48칸으로 지은 머리글(오늘 칸 14)을 10칸 영역에 그린다 — 영역 밖의 칸에는 아무것도 안 쓴다.
        let a = Axis::new(Zoom::Day, d(2026, 10, 20) * DAY);
        let h = a.header(48, d(2026, 10, 23) * DAY + 14 * 3_600, &Names::default());
        assert_eq!(h.today, Some(14));
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        render_header(&mut buf, Rect::new(0, 0, 10, 3), &h, &HeaderStyle::default());
        assert_eq!(row(&buf, 2), format!("{:20}", ""));
    }
}
