//! 그리기 시험 — 버퍼를 글자로 읽고, 색은 칸마다 읽는다.

use ratatui::style::{Color, Modifier};

use super::*;

const DAY: i64 = 86_400;
const H: i64 = 3_600;
const ORIGIN: i64 = 100 * DAY;
const W: u16 = 12;
const LW: u16 = 4;

fn rows_of(n: usize) -> Rect {
    Rect::new(0, 0, LW + 1 + W, HEADER_ROWS + n as u16)
}

fn render_with(rows: &[GanttRow], zoom: Zoom, now: i64, tweak: impl FnOnce(&mut Gantt, &mut GanttState)) -> Buffer {
    let area = rows_of(rows.len());
    let mut buf = Buffer::empty(area);
    let mut st = GanttState::new(Axis::new(zoom, ORIGIN));
    st.label_width = LW;
    let mut g = Gantt { rows, now, ..Gantt::default() };
    tweak(&mut g, &mut st);
    g.render(area, &mut buf, &mut st);
    buf
}

fn render(rows: &[GanttRow], zoom: Zoom, now: i64) -> Buffer {
    render_with(rows, zoom, now, |_, _| {})
}

/// 본문 `i` 번 줄의 날짜 칸 글자(오른쪽 공백은 뗀다).
fn line(buf: &Buffer, i: u16) -> String {
    let y = HEADER_ROWS + i;
    (LW + 1..LW + 1 + W).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string()
}

fn cell(buf: &Buffer, col: u16, i: u16) -> &ratatui::buffer::Cell {
    &buf[(LW + 1 + col, HEADER_ROWS + i)]
}

fn planned(s: i64, e: i64) -> GanttRow {
    GanttRow { planned: Some((s, e)), ..GanttRow::default() }
}

fn actual(s: i64, e: Option<i64>) -> GanttRow {
    GanttRow { actual: Some((s, e)), tone: Tone::Progress, ..GanttRow::default() }
}

/// 늘 화면 밖(과거)인 지금 — 오늘 선이 안 끼게.
const LONG_AGO: i64 = 0;

#[test]
fn one_planned_bar_per_zoom() {
    // Week: 하루 한 칸. 날 101~104 → 칸 1..4.
    let b = render(&[planned(101 * DAY, 104 * DAY)], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), " ░░░");
    // Day: 하루 4칸 → 12칸.
    let b = render(&[planned(100 * DAY + 6 * H, 101 * DAY)], Zoom::Day, LONG_AGO);
    assert_eq!(line(&b, 0), " ░░░");
    // Hour: 한 칸 한 시간.
    let b = render(&[planned(100 * DAY + 2 * H, 100 * DAY + 5 * H)], Zoom::Hour, LONG_AGO);
    assert_eq!(line(&b, 0), "  ░░░");
    // Month: 한 칸 7일.
    let b = render(&[planned(100 * DAY, 114 * DAY)], Zoom::Month, LONG_AGO);
    assert_eq!(line(&b, 0), "░░");
}

#[test]
fn group_plan_uses_its_own_glyph_and_the_row_label_is_clipped() {
    let mut r = planned(101 * DAY, 103 * DAY);
    r.kind = RowKind::Group;
    r.label = Line::from("가나다라마바");
    let b = render(&[r], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), " ▒▒");
    // 고정 칸 폭 4 에 두 칸짜리 글자 둘만 든다. 분할선은 그 오른쪽 한 칸이다.
    let labels: String = (0..LW).map(|x| b[(x, HEADER_ROWS)].symbol()).collect();
    assert_eq!(labels.replace(' ', ""), "가나");
    assert_eq!(b[(LW, HEADER_ROWS)].symbol(), "│");
}

#[test]
fn fractional_ends_use_eighths_and_the_left_end_is_reversed() {
    // Day: 한 칸 6시간. 9시간 = 1.5칸(왼쪽 4/8 비움), 26시간 15분 = 4.375칸(오른쪽 3/8).
    let b = render(&[actual(100 * DAY + 9 * H, Some(100 * DAY + 26 * H + 900))], Zoom::Day, LONG_AGO);
    assert_eq!(line(&b, 0), " ▌██▍");
    let prog = Palette::default().progress.fg;
    let left = cell(&b, 1, 0);
    assert!(left.modifier.contains(Modifier::REVERSED), "왼쪽 끝은 앞뒤 색을 바꿔 그린다");
    assert_eq!(left.fg, prog.unwrap());
    assert!(!cell(&b, 2, 0).modifier.contains(Modifier::REVERSED));
    assert_eq!(cell(&b, 4, 0).fg, prog.unwrap());
    // 45분(= 8분의 1칸 × 3 이 아니라 한 칸 6시간의 1/8) 단위로 보인다: 45분 → ▏.
    let b = render(&[actual(100 * DAY, Some(100 * DAY + 45 * 60))], Zoom::Day, LONG_AGO);
    assert_eq!(line(&b, 0), "▏");
}

#[test]
fn a_running_actual_runs_up_to_now_and_sits_over_the_plan() {
    let r = GanttRow { planned: Some((100 * DAY, 106 * DAY)), actual: Some((100 * DAY, None)), ..GanttRow::default() };
    let b = render(&[r], Zoom::Week, 103 * DAY + 12 * H);
    // 실제는 지금(3.5칸)까지 — 반 칸은 ▌. 오늘 선은 그 칸을 막대가 덮는다.
    assert_eq!(line(&b, 0), "███▌░░");
}

#[test]
fn clipping_marks_both_sides() {
    // 왼쪽으로 잘림.
    let b = render(&[planned(95 * DAY, 105 * DAY)], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), "‹░░░░");
    assert_eq!(cell(&b, 0, 0).fg, Palette::default().clip.fg.unwrap_or(Color::Reset));
    // 오른쪽으로 잘림 — 막대가 일부 보일 때.
    let b = render(&[planned(107 * DAY, 120 * DAY)], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), "       ░░░░›");
    // 화면 밖에 통째로 있는 줄도 어느 쪽에 있는지는 보인다.
    let b = render(&[planned(150 * DAY, 160 * DAY)], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), "           ›");
    let b = render(&[planned(10 * DAY, 20 * DAY)], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), "‹");
}

#[test]
fn overdue_runs_from_the_day_after_due_to_now_and_ends_in_a_bang() {
    let mut r = planned(101 * DAY, 103 * DAY);
    r.due = Some(103 * DAY);
    r.overdue = true;
    let b = render(&[r], Zoom::Week, 107 * DAY + H);
    assert_eq!(line(&b, 0), " ░░▓▓▓▓!");
    let red = Palette::default().overdue.fg.unwrap();
    assert_eq!(cell(&b, 3, 0).fg, red);
    assert_eq!(cell(&b, 7, 0).fg, red);
    // 기한과 지금이 같은 칸이면 `!` 하나만 선다.
    let mut r = planned(101 * DAY, 103 * DAY);
    r.due = Some(103 * DAY);
    r.overdue = true;
    let b = render(&[r], Zoom::Week, 103 * DAY + H);
    assert_eq!(line(&b, 0), " ░░!");
}

#[test]
fn the_today_line_shows_on_empty_cells_and_hides_under_a_bar() {
    let now = 105 * DAY + H;
    let rows = [GanttRow::default(), planned(103 * DAY, 108 * DAY), actual(104 * DAY, Some(107 * DAY))];
    let b = render(&rows, Zoom::Week, now);
    assert_eq!(line(&b, 0), "     ┊");
    assert_eq!(line(&b, 1), "   ░░░░░");
    assert_eq!(line(&b, 2), "    ███");
    assert_eq!(cell(&b, 5, 0).fg, Palette::default().today.fg.unwrap());
    // 머리글의 `▼` 도 같은 칸이다.
    assert_eq!(b[(LW + 1 + 5, 2)].symbol(), "▼");
}

#[test]
fn a_deferred_row_draws_its_plan_dimmed_with_a_different_glyph() {
    let mut r = planned(101 * DAY, 104 * DAY);
    r.deferred = true;
    let b = render(&[r], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), " ┄┄┄");
    assert!(cell(&b, 1, 0).modifier.contains(Modifier::DIM));
}

#[test]
fn a_milestone_and_a_due_only_row_stand_as_one_diamond() {
    let ms = GanttRow { kind: RowKind::Milestone, due: Some(104 * DAY), ..GanttRow::default() };
    let due_only = GanttRow { due: Some(103 * DAY), ..GanttRow::default() };
    let with_plan = GanttRow {
        kind: RowKind::Milestone,
        planned: Some((101 * DAY, 104 * DAY)),
        due: Some(104 * DAY),
        ..GanttRow::default()
    };
    // 기한이 든 날의 칸(그날 끝 직전)에 선다.
    let b = render(&[ms, due_only, with_plan], Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), "   ◆");
    assert_eq!(line(&b, 1), "  ◆");
    assert_eq!(line(&b, 2), " ▒▒◆");
    assert_eq!(cell(&b, 3, 0).fg, Palette::default().due.fg.unwrap());
    // 계획이 있는 이슈는 기한에 ◆ 를 안 둔다 — 막대의 끝이 기한이다.
    let item = GanttRow { planned: Some((101 * DAY, 104 * DAY)), due: Some(104 * DAY), ..GanttRow::default() };
    assert_eq!(line(&render(&[item], Zoom::Week, LONG_AGO), 0), " ░░░");
}

#[test]
fn an_unknown_start_draws_the_plan_only() {
    let r =
        GanttRow { planned: Some((101 * DAY, 104 * DAY)), actual: None, tone: Tone::Progress, ..GanttRow::default() };
    let b = render(&[r], Zoom::Week, 103 * DAY);
    assert_eq!(line(&b, 0), " ░░░");
}

#[test]
fn colors_read_per_cell_follow_tone_and_weekend() {
    let pal = Palette::default();
    let rows = [
        GanttRow { tone: Tone::Progress, ..actual(100 * DAY, Some(102 * DAY)) },
        GanttRow { tone: Tone::Review, ..actual(100 * DAY, Some(102 * DAY)) },
        GanttRow { tone: Tone::Done, ..actual(100 * DAY, Some(102 * DAY)) },
        planned(100 * DAY, 102 * DAY),
    ];
    let b = render(&rows, Zoom::Week, LONG_AGO);
    assert_eq!(cell(&b, 0, 0).fg, pal.progress.fg.unwrap());
    assert_eq!(cell(&b, 1, 1).fg, pal.review.fg.unwrap());
    assert_eq!(cell(&b, 0, 2).fg, pal.done.fg.unwrap());
    assert_eq!(cell(&b, 0, 3).fg, pal.planned.fg.unwrap());
    // 주말 바탕(Day): 막대 밑에서도 바탕은 산다.
    let r = actual(100 * DAY, Some(112 * DAY));
    let st = GanttState::new(Axis::new(Zoom::Day, ORIGIN));
    let h = st.axis.header(W, LONG_AGO, &Names::default());
    let we = h.weekend.iter().position(|w| *w).expect("12칸(3일) 안에 주말이 있어야 한다") as u16;
    let b = render(&[r], Zoom::Day, LONG_AGO);
    assert_eq!(cell(&b, we, 0).bg, pal.weekend.bg.unwrap());
    assert_eq!(cell(&b, we, 0).symbol(), "█");
    let other = h.weekend.iter().position(|w| !*w).unwrap() as u16;
    assert_ne!(cell(&b, other, 0).bg, pal.weekend.bg.unwrap());
}

#[test]
fn the_focus_row_tints_the_label_column_only() {
    let rows = [GanttRow::default(), GanttRow::default()];
    let b = render_with(&rows, Zoom::Week, LONG_AGO, |g, _| g.focus = Some(1));
    let tint = Palette::default().focus.bg.unwrap();
    assert_eq!(b[(0, HEADER_ROWS + 1)].bg, tint);
    assert_ne!(b[(0, HEADER_ROWS)].bg, tint);
    assert_ne!(cell(&b, 0, 1).bg, tint);
}

#[test]
fn rows_start_at_first_row() {
    let rows = [planned(100 * DAY, 101 * DAY), planned(101 * DAY, 102 * DAY)];
    let b = render_with(&rows, Zoom::Week, LONG_AGO, |_, s| s.first_row = 1);
    assert_eq!(line(&b, 0), " ░");
}

#[test]
fn ascii_glyphs_keep_every_mark() {
    let mut r = planned(101 * DAY, 103 * DAY);
    r.due = Some(103 * DAY);
    r.overdue = true;
    let b = render_with(&[r], Zoom::Week, 106 * DAY + H, |g, _| g.glyphs = Glyphs::ascii());
    assert_eq!(line(&b, 0), " ..%%%!");
    assert!(line(&b, 0).is_ascii());
    // 끝 조각 없이: 반 칸 넘게 걸친 칸만 채운다.
    let b = render_with(&[actual(100 * DAY + 9 * H, Some(100 * DAY + 26 * H))], Zoom::Day, LONG_AGO, |g, _| {
        g.glyphs = Glyphs::ascii()
    });
    assert_eq!(line(&b, 0), " ###");
}

#[test]
fn what_is_drawn_stays_inside_what_hit_testing_covers() {
    // 그리는 막대 글리프가 선 칸은 모두 맞히기의 `bar_cells(span)` 안에 든다.
    let now = 106 * DAY + 5 * H;
    let rows = [
        planned(95 * DAY, 103 * DAY),
        GanttRow {
            planned: Some((101 * DAY, 103 * DAY)),
            actual: Some((101 * DAY + 5 * H, None)),
            due: Some(103 * DAY),
            overdue: true,
            ..GanttRow::default()
        },
        GanttRow { kind: RowKind::Milestone, due: Some(109 * DAY), ..GanttRow::default() },
        planned(107 * DAY, 130 * DAY),
        GanttRow { deferred: true, ..planned(100 * DAY, 101 * DAY) },
    ];
    for zoom in [Zoom::Hour, Zoom::Day, Zoom::Week, Zoom::Month] {
        let b = render(&rows, zoom, now);
        let st = GanttState::new(Axis::new(zoom, ORIGIN));
        for (i, r) in rows.iter().enumerate() {
            let (c0, c1) = bar_cells(&st, r.span(now).unwrap());
            for col in 0..W {
                let s = cell(&b, col, i as u16).symbol();
                // 잘림 표식은 화면 밖 막대를 가리키는 힌트라 맞히기 칸 밖에도 설 수 있다.
                if !matches!(s, " " | "┊" | "‹" | "›") {
                    assert!((c0..c1).contains(&i64::from(col)), "{zoom:?} 줄 {i} 칸 {col} `{s}` 이 맞히기 밖이다");
                }
            }
        }
    }
}
