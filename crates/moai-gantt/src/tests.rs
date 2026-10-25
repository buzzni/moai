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

// ---- 의존 화살표 (moai-v53u.4xi) ----

/// 하루 한 칸(Week)인 날 `a..b` 의 계획 줄 — 칸 `a..b`.
fn bar(a: i64, b: i64) -> GanttRow {
    planned((100 + a) * DAY, (100 + b) * DAY)
}

const fn lk(from_row: usize, to_row: usize) -> Link {
    Link { from_row, to_row }
}

/// 연결선 화면. `shown` 줄을 `first` 번 줄부터 그린다.
fn view(rows: &[GanttRow], links: &[Link], focus: Option<usize>, first: usize, shown: u16, glyphs: Glyphs) -> Buffer {
    let area = Rect::new(0, 0, LW + 1 + W, HEADER_ROWS + shown);
    let mut buf = Buffer::empty(area);
    let mut st = GanttState::new(Axis::new(Zoom::Week, ORIGIN));
    st.label_width = LW;
    st.first_row = first;
    let g = Gantt { rows, links, focus, now: LONG_AGO, glyphs, ..Gantt::default() };
    g.render(area, &mut buf, &mut st);
    buf
}

fn lines(buf: &Buffer, n: u16) -> Vec<String> {
    (0..n).map(|i| line(buf, i)).collect()
}

fn simple(rows: &[GanttRow], links: &[Link], focus: Option<usize>) -> Vec<String> {
    let n = rows.len() as u16;
    lines(&view(rows, links, focus, 0, n, Glyphs::default()), n)
}

#[test]
fn one_blocker_draws_a_line_a_corner_and_the_arrow_before_the_focus_bar() {
    let rows = [bar(0, 2), bar(6, 9)];
    let b = view(&rows, &[lk(0, 1)], Some(1), 0, 2, Glyphs::default());
    assert_eq!(lines(&b, 2), ["░░──┐", "    └▶░░░"]);
    assert_eq!(cell(&b, 2, 0).fg, Color::Magenta);
    assert_eq!(cell(&b, 5, 1).fg, Color::Magenta);
}

#[test]
fn two_blockers_end_in_a_corner_then_a_tee() {
    let rows = [bar(0, 2), bar(0, 3), bar(6, 9)];
    assert_eq!(simple(&rows, &[lk(0, 2), lk(1, 2)], Some(2)), ["░░──┐", "░░░─┤", "    └▶░░░"]);
}

#[test]
fn a_blocker_below_the_focus_climbs_with_a_corner_up() {
    let rows = [bar(6, 9), bar(0, 2)];
    assert_eq!(simple(&rows, &[lk(1, 0)], Some(0)), ["    ┌▶░░░", "░░──┘"]);
}

#[test]
fn nothing_is_drawn_without_a_focus() {
    let rows = [bar(0, 2), bar(6, 9)];
    assert_eq!(simple(&rows, &[lk(0, 1)], None), ["░░", "      ░░░"]);
}

#[test]
fn the_focus_blocking_others_mirrors_the_shape() {
    // 커서 줄 끝 2, 막히는 줄의 가장 이른 시작 7 → 연결 칸 5.
    let rows = [bar(0, 2), bar(7, 10), bar(8, 11)];
    assert_eq!(simple(&rows, &[lk(0, 1), lk(0, 2)], Some(0)), ["░░───┐", "     ├▶░░░", "     └─▶░░░"]);
}

#[test]
fn offscreen_blockers_and_blocked_show_counts_at_the_edge_rows() {
    // 줄 0·1 은 위로 밀렸고 5 는 아래다. 화면은 줄 2..5 — 커서 3.
    let mut rows = vec![GanttRow::default(); 6];
    rows[3] = bar(6, 9);
    let links = [lk(0, 3), lk(1, 3), lk(5, 3)];
    let b = view(&rows, &links, Some(3), 2, 3, Glyphs::default());
    assert_eq!(lines(&b, 3), ["    ↑ 2", "    ├▶░░░", "    ↓ 1"]);
    assert_eq!(cell(&b, 4, 0).fg, Color::Magenta);
}

#[test]
fn the_count_moves_left_of_the_corner_when_the_focus_is_the_edge_row() {
    let mut rows = vec![GanttRow::default(); 3];
    rows[1] = bar(6, 9);
    let b = view(&rows, &[lk(0, 1)], Some(1), 1, 2, Glyphs::default());
    assert_eq!(lines(&b, 2), [" ↑ 1└▶░░░", ""]);
}

#[test]
fn overlapping_schedules_leave_only_the_mark_and_never_cover_the_bar() {
    // 막는 줄이 커서 줄의 시작을 넘어 겹친다 → 길이 없다. 표식은 날짜 칸 첫 칸에.
    let rows = [bar(3, 7), bar(6, 9)];
    assert_eq!(simple(&rows, &[lk(0, 1)], Some(1)), ["↓  ░░░░", "    └▶░░░"]);
    // 첫 칸이 막대면 표식도 못 놓는다.
    let rows = [bar(0, 7), bar(6, 9)];
    assert_eq!(simple(&rows, &[lk(0, 1)], Some(1))[0], "░░░░░░░");
    // 커서 줄 아래의 막는 줄이면 `↑`.
    let rows = [bar(6, 9), bar(3, 7)];
    assert_eq!(simple(&rows, &[lk(1, 0)], Some(0))[1], "↑  ░░░░");
}

#[test]
fn the_vertical_line_skips_a_bar_it_would_cross() {
    let rows = [bar(0, 2), bar(3, 6), bar(6, 9)];
    assert_eq!(simple(&rows, &[lk(0, 2)], Some(2)), ["░░──┐", "   ░░░", "    └▶░░░"]);
}

#[test]
fn ascii_glyphs_draw_the_same_shape() {
    let rows = [bar(0, 2), bar(6, 9)];
    let b = view(&rows, &[lk(0, 1)], Some(1), 0, 2, Glyphs::ascii());
    assert_eq!(lines(&b, 2), ["..--+", "    +>..."]);
    // 미룬 막는 줄의 막대는 연결선과 글자가 다르다 — 색 없이도 막대가 어디서 끝나는지 보인다.
    let rows = [GanttRow { deferred: true, ..bar(0, 2) }, bar(6, 9)];
    let b = view(&rows, &[lk(0, 1)], Some(1), 0, 2, Glyphs::ascii());
    assert_eq!(lines(&b, 2), ["~~--+", "    +>..."]);
}

#[test]
fn the_today_line_gives_way_to_a_link() {
    let rows = [bar(0, 2), bar(6, 9)];
    let links = [lk(0, 1)];
    // 기본 글리프와 부르는 쪽이 고른 오늘 글리프 둘 다 — 빈 칸 판정이 글자를 박아 두면 둘째가 길을 막는다.
    for today in [Glyphs::default().today, '|'] {
        let area = rows_of(2);
        let mut buf = Buffer::empty(area);
        let mut st = GanttState::new(Axis::new(Zoom::Week, ORIGIN));
        st.label_width = LW;
        let glyphs = Glyphs { today, ..Glyphs::default() };
        Gantt { rows: &rows, links: &links, focus: Some(1), now: 103 * DAY, glyphs, ..Gantt::default() }
            .render(area, &mut buf, &mut st);
        assert_eq!(line(&buf, 0), "░░──┐", "오늘 글리프 {today:?}");
        assert_eq!(line(&buf, 1), format!("   {today}└▶░░░"));
    }
}

#[test]
fn a_count_on_the_focus_edge_row_on_the_blocked_side_stays_visible() {
    // 커서 줄(1)이 화면 첫 줄이고 위로 밀린 줄 0 을 막는다. 길이 왼쪽(막대 끝)에서 와 꺾으니 개수는 오른쪽이다.
    let mut rows = vec![GanttRow::default(); 3];
    rows[1] = bar(0, 2);
    let b = view(&rows, &[lk(1, 0)], Some(1), 1, 2, Glyphs::default());
    assert_eq!(lines(&b, 2), ["░░┘↑ 1", ""]);
}

#[test]
fn a_count_on_an_edge_row_that_carries_a_path_keeps_its_arrow() {
    // 화면 첫 줄(1)이 길을 낸 막는 줄이고 그 위(0)에도 막는 줄이 있다 — 첫 줄은 세로줄에 이어지고 개수는 그 오른쪽.
    let mut rows = vec![GanttRow::default(); 4];
    rows[1] = bar(0, 2);
    rows[3] = bar(6, 9);
    let b = view(&rows, &[lk(0, 3), lk(1, 3)], Some(3), 1, 3, Glyphs::default());
    assert_eq!(lines(&b, 3), ["░░──┤↑ 1", "    │", "    └▶░░░"]);
}

#[test]
fn a_due_that_draws_no_diamond_is_not_part_of_the_bar() {
    // 계획이 있는 이슈는 기한에 ◆ 를 안 둔다 — 기한이 계획 끝보다 늦어도 막대는 계획에서 끝난다.
    let r = GanttRow { planned: Some((101 * DAY, 103 * DAY)), due: Some(115 * DAY), ..GanttRow::default() };
    assert_eq!(r.span(LONG_AGO), Some((101 * DAY, 103 * DAY)));
    let b = render(std::slice::from_ref(&r), Zoom::Week, LONG_AGO);
    assert_eq!(line(&b, 0), " ░░", "안 그린 기한을 `›` 로 가리키지 않는다");
    let mut st = GanttState::new(Axis::new(Zoom::Week, ORIGIN));
    st.label_width = LW;
    assert_eq!(hit(rows_of(1), &st, &[r], LONG_AGO, LW + 1 + 6, HEADER_ROWS), Hit::Empty { row: 0, day: 106 });
}

#[test]
fn a_squeezed_widget_never_writes_outside_its_area() {
    // 고정 칸이 날짜 칸을 다 먹은 폭(0 폭 날짜 칸)·0 폭·낮은 높이에서, 잘린 막대·기한 넘김·연결선이 든 줄을 그린다.
    let rows = [
        planned(90 * DAY, 105 * DAY),
        GanttRow { planned: Some((101 * DAY, 103 * DAY)), due: Some(103 * DAY), overdue: true, ..GanttRow::default() },
        GanttRow { kind: RowKind::Milestone, due: Some(130 * DAY), ..GanttRow::default() },
        bar(6, 9),
    ];
    let links = [lk(0, 3), lk(1, 3), lk(3, 2)];
    let full = Rect::new(0, 0, 30, 12);
    for zoom in Zoom::ALL {
        for w in 0..=LW + 3 {
            for h in 0..=HEADER_ROWS + 2 {
                let area = Rect::new(3, 2, w, h);
                let mut buf = Buffer::empty(full);
                let mut st = GanttState::new(Axis::new(zoom, ORIGIN));
                st.label_width = LW;
                let g = Gantt { rows: &rows, links: &links, focus: Some(3), now: 106 * DAY, ..Gantt::default() };
                g.render(area, &mut buf, &mut st);
                for y in 0..full.height {
                    for x in 0..full.width {
                        if !area.contains(ratatui::layout::Position { x, y }) {
                            assert_eq!(buf[(x, y)], Default::default(), "{zoom:?} {w}x{h}: ({x},{y}) 에 썼다");
                        }
                    }
                }
            }
        }
    }
}
