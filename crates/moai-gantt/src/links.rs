//! 의존 화살표의 길 찾기 — 버퍼를 모르는 순수 셈이다. 커서 줄 하나의 연결만 그린다: 모든 줄의
//! 화살표를 그리면 칸 격자에서 엉킨다.
//!
//! 방향: `Link { from_row, to_row }` 의 `from_row` 가 막는 쪽(blocker)이고 `to_row` 가 막히는 쪽이다
//! (`blocked_by` 의 끝-시작 관계 — `from` 이 끝나야 `to` 가 시작한다).
//!
//! 모양(시안): 막는 줄은 막대 끝 다음 칸부터 `─` 로 달려 연결 칸에서 꺾고, 연결 칸을 `│` 로 내려가
//! 커서 줄에서 `└▶` 로 막대 시작 앞에 닿는다. 커서 줄이 막는 줄은 같은 꼴을 거울로 — 커서 줄 끝에서
//! 나가 연결 칸에서 꺾고 그 줄 시작 앞에서 `▶` 로 닿는다. 칸마다 뻗은 팔(L·R·U·D)을 모아 두고
//! 글리프는 그리는 쪽이 팔 모양으로 고른다 — 가장 먼 줄이 `┐`, 사이의 줄이 `┤` 가 되는 것이 그 덕이다.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::Link;

pub(crate) const L: u8 = 1;
pub(crate) const R: u8 = 2;
pub(crate) const U: u8 = 4;
pub(crate) const D: u8 = 8;

/// 팔 모양이 고르는 글리프. 팔이 하나뿐이면 곧은 줄이다.
pub(crate) fn glyph(arms: u8, g: &crate::LinkGlyphs) -> char {
    match arms {
        a if a == L | D => g.down_left,
        a if a == L | U => g.up_left,
        a if a == R | D => g.down_right,
        a if a == R | U => g.up_right,
        a if a == L | U | D => g.tee_left,
        a if a == R | U | D => g.tee_right,
        a if a & (L | R) != 0 && a & (U | D) == 0 => g.h,
        _ => g.v,
    }
}

/// 칸 하나에 서는 것. 팔이 없는 칸은 화살촉(`▶`)뿐이다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Piece {
    pub arms: u8,
    pub arrow: bool,
}

/// 막는 줄이 막힌 길이라 고정 칸 쪽에 남기는 표식. 늘 **커서 줄을 향한다** — 위의 줄은 `Down`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
    Up,
    Down,
}

/// 화면 밖 줄의 수. 위 줄은 본문 첫 줄에, 아래 줄은 본문 끝 줄에 `col` 부터 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct More {
    pub row: usize,
    pub col: i64,
    pub dir: Dir,
    pub n: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Route {
    /// `(줄 번호, 날짜 칸 좌표)` → 그 칸에 설 것. 좌표는 영역 밖일 수 있다(그리는 쪽이 자른다).
    pub cells: BTreeMap<(usize, i64), Piece>,
    /// 길이 막혀 표식만 남기는 줄. 표식의 방향은 커서 줄 쪽이다.
    pub marks: Vec<(usize, Dir)>,
    pub more: Vec<More>,
}

impl Route {
    fn arm(&mut self, row: usize, col: i64, arms: u8) {
        self.cells.entry((row, col)).or_default().arms |= arms;
    }
}

/// `n` 개 중 위·아래로 갈린 줄을 센 것과 길이 닿는 줄.
struct Side {
    /// 화면 안에서 길을 그릴 줄 `(줄, 막대 칸 [시작, 끝))`.
    seen: Vec<(usize, Option<(i64, i64)>)>,
    above: usize,
    below: usize,
}

fn side(others: &[usize], vis: &Range<usize>, focus: usize, ext: &impl Fn(usize) -> Option<(i64, i64)>) -> Side {
    let mut s = Side { seen: Vec::new(), above: 0, below: 0 };
    for &r in others {
        if r < vis.start {
            s.above += 1;
        } else if r >= vis.end {
            s.below += 1;
        } else if r != focus {
            s.seen.push((r, ext(r)));
        }
    }
    s
}

/// 커서 줄 `focus` 의 연결을 찾는다.
///
/// - `ext(row)`: 줄의 막대가 차지하는 칸 `[시작, 끝)`. 끝은 계획 끝과 실제 끝 중 늦은 쪽이다.
///   없으면(날짜 없는 줄) 길을 못 그린다.
/// - `vis`: 화면에 든 줄 번호.
/// - `occupied(row, col)`: 그 칸에 이미 막대나 표식이 있나. 길이 지날 칸이 막혔는지만 본다.
pub(crate) fn route(
    focus: usize,
    links: &[Link],
    ext: impl Fn(usize) -> Option<(i64, i64)>,
    vis: Range<usize>,
    occupied: impl Fn(usize, i64) -> bool,
) -> Route {
    let mut out = Route::default();
    let Some((fs, fe)) = ext(focus).filter(|_| vis.contains(&focus)) else { return out };
    let pick = |is_to: bool| {
        let mut v: Vec<usize> = links
            .iter()
            .filter(|l| l.from_row != l.to_row)
            .filter_map(|l| match is_to {
                true => (l.to_row == focus).then_some(l.from_row),
                false => (l.from_row == focus).then_some(l.to_row),
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let (blockers, blocked) = (pick(true), pick(false));

    // 막는 쪽: 연결 칸은 커서 막대 시작 두 칸 앞이다 — 꺾는 칸 하나, 화살촉 한 칸.
    let sd = side(&blockers, &vis, focus, &ext);
    let c = fs - 2;
    let mut rows = vec![focus];
    for &(r, e) in &sd.seen {
        rows.push(r);
        // 길 = 막대 끝 다음 칸부터 연결 칸까지. 끝이 연결 칸을 넘으면(겹친 일정) 지날 자리가 없다.
        let free = e.is_some_and(|(_, end)| end <= c && (end..=c).all(|x| !occupied(r, x)));
        if free {
            let end = e.map_or(c, |p| p.1);
            for x in end..c {
                out.arm(r, x, L | R);
            }
            out.arm(r, c, L);
        } else {
            out.marks.push((r, if r < focus { Dir::Down } else { Dir::Up }));
        }
    }
    spine(&mut out, c, rows, focus, &vis, &sd);
    if !sd.seen.is_empty() || sd.above + sd.below > 0 {
        out.arm(focus, c, R);
        out.cells.entry((focus, c + 1)).or_default().arrow = true;
    }

    // 막히는 쪽: 커서 막대 끝에서 나가, 가장 이른 목표의 시작 두 칸 앞에서 꺾는다.
    let sd = side(&blocked, &vis, focus, &ext);
    let min_start = sd.seen.iter().filter_map(|(_, e)| e.map(|p| p.0)).min().unwrap_or(fe + 2);
    let c2 = (min_start - 2).max(fe);
    let mut rows = vec![focus];
    for &(r, e) in &sd.seen {
        rows.push(r);
        // 목표 줄: 연결 칸 다음부터 시작 앞 칸의 화살촉까지. 시작이 너무 가까우면 못 닿는다.
        let free = e.is_some_and(|(start, _)| start >= c2 + 2 && (c2..start).all(|x| !occupied(r, x)));
        if let Some((start, _)) = e.filter(|_| free) {
            out.arm(r, c2, R);
            for x in c2 + 1..start - 1 {
                out.arm(r, x, L | R);
            }
            out.cells.entry((r, start - 1)).or_default().arrow = true;
        } else {
            out.marks.push((r, if r < focus { Dir::Down } else { Dir::Up }));
        }
    }
    spine(&mut out, c2, rows, focus, &vis, &sd);
    if !sd.seen.is_empty() || sd.above + sd.below > 0 {
        for x in fe..c2 {
            out.arm(focus, x, L | R);
        }
        out.arm(focus, c2, L);
    }
    out
}

/// 연결 칸의 세로줄과 화면 밖 개수. 세로줄은 가장 먼 줄(화면 밖이 있으면 화면 끝 안쪽 줄)까지 닿는다.
/// 개수는 끝 줄에 쓰므로 그 칸 하나는 비운다.
fn spine(out: &mut Route, col: i64, rows: Vec<usize>, focus: usize, vis: &Range<usize>, s: &Side) {
    if rows.len() == 1 && s.above + s.below == 0 {
        return;
    }
    let mut lo = rows.iter().copied().min().unwrap_or(focus);
    let mut hi = rows.iter().copied().max().unwrap_or(focus);
    let last = vis.end.saturating_sub(1);
    // 개수가 앉는 줄까지 세로줄이 이어지는 것으로 셈하되, 그 줄의 칸은 개수가 쓰니 비운다.
    let (top, bottom) = (s.above > 0, s.below > 0);
    if top {
        lo = vis.start;
        out.more.push(More { row: vis.start, col, dir: Dir::Up, n: s.above });
    }
    if bottom {
        hi = last;
        out.more.push(More { row: last, col, dir: Dir::Down, n: s.below });
    }
    for r in lo..=hi {
        let label_row = r != focus && ((top && r == vis.start) || (bottom && r == last));
        if !label_row {
            // 커서 줄이 개수 줄이기도 하면 위·아래 팔은 화면 밖으로 나간다.
            let up = r > lo || (top && r == vis.start);
            let down = r < hi || (bottom && r == last);
            let arms = if up { U } else { 0 } | if down { D } else { 0 };
            out.arm(r, col, arms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 줄 번호 → 막대 칸. 시험은 줄마다 `(시작, 끝)` 을 직접 준다.
    fn run(
        focus: usize,
        links: &[(usize, usize)],
        bars: &[Option<(i64, i64)>],
        vis: Range<usize>,
        wall: &[(usize, i64)],
    ) -> Route {
        let links: Vec<Link> = links.iter().map(|&(from_row, to_row)| Link { from_row, to_row }).collect();
        route(focus, &links, |r| bars.get(r).copied().flatten(), vis, |r, c| wall.contains(&(r, c)))
    }

    fn arms(r: &Route, row: usize, col: i64) -> u8 {
        r.cells.get(&(row, col)).map_or(0, |p| p.arms)
    }

    #[test]
    fn one_blocker_runs_to_the_column_two_before_the_focus_start() {
        // 막는 줄 0 은 칸 [1,4) 에서 끝나고, 커서 줄 1 은 칸 8 에서 시작한다 → 연결 칸 6.
        let bars = [Some((1, 4)), Some((8, 12))];
        let r = run(1, &[(0, 1)], &bars, 0..2, &[]);
        assert_eq!(arms(&r, 0, 4), L | R);
        assert_eq!(arms(&r, 0, 5), L | R);
        assert_eq!(arms(&r, 0, 6), L | D);
        assert_eq!(arms(&r, 1, 6), R | U);
        assert!(r.cells[&(1, 7)].arrow);
        assert!(r.marks.is_empty() && r.more.is_empty());
    }

    #[test]
    fn the_farthest_blocker_turns_and_the_nearer_one_tees() {
        let bars = [Some((0, 2)), Some((0, 5)), Some((9, 12))];
        let r = run(2, &[(0, 2), (1, 2)], &bars, 0..3, &[]);
        assert_eq!(arms(&r, 0, 7), L | D);
        assert_eq!(arms(&r, 1, 7), L | U | D);
        assert_eq!(arms(&r, 2, 7), R | U);
    }

    #[test]
    fn a_blocker_below_the_focus_climbs() {
        let bars = [Some((9, 12)), Some((0, 3))];
        let r = run(0, &[(1, 0)], &bars, 0..2, &[]);
        assert_eq!(arms(&r, 1, 7), L | U);
        assert_eq!(arms(&r, 0, 7), R | D);
    }

    #[test]
    fn offscreen_blockers_are_counted_at_the_edge_row() {
        let bars = vec![Some((0, 2)); 10];
        let mut b = bars.clone();
        b[5] = Some((9, 12));
        // 화면은 줄 3..8, 커서 5. 막는 줄 0·1 은 위, 9 는 아래.
        let r = run(5, &[(0, 5), (1, 5), (9, 5)], &b, 3..8, &[]);
        assert_eq!(
            r.more,
            vec![More { row: 3, col: 7, dir: Dir::Up, n: 2 }, More { row: 7, col: 7, dir: Dir::Down, n: 1 },]
        );
        // 세로줄은 화면 끝 줄 바로 안쪽까지 간다.
        assert_eq!(arms(&r, 4, 7), U | D);
        assert_eq!(arms(&r, 3, 7), 0);
        assert_eq!(arms(&r, 5, 7), R | U | D);
        assert_eq!(arms(&r, 6, 7), U | D);
        assert_eq!(arms(&r, 7, 7), 0);
    }

    #[test]
    fn a_path_blocked_by_a_bar_leaves_only_a_mark() {
        let bars = [Some((0, 2)), Some((9, 12))];
        let r = run(1, &[(0, 1)], &bars, 0..2, &[(0, 4)]);
        assert_eq!(r.marks, vec![(0, Dir::Down)]);
        assert_eq!(arms(&r, 0, 2), 0);
        // 세로줄은 남는다 — 가는 곳은 이어져 있다.
        assert_eq!(arms(&r, 0, 7), D);
    }

    #[test]
    fn overlapping_schedules_leave_a_mark_too() {
        let bars = [Some((0, 9)), Some((8, 12))];
        let r = run(1, &[(0, 1)], &bars, 0..2, &[]);
        assert_eq!(r.marks, vec![(0, Dir::Down)]);
    }

    #[test]
    fn the_focus_as_the_blocking_side_mirrors() {
        // 커서 0 은 [1,4) 에서 끝나고, 막히는 줄 1·2 는 칸 9·12 에서 시작한다 → 연결 칸 7.
        let bars = [Some((1, 4)), Some((9, 11)), Some((12, 14))];
        let r = run(0, &[(0, 1), (0, 2)], &bars, 0..3, &[]);
        assert_eq!(arms(&r, 0, 4), L | R);
        assert_eq!(arms(&r, 0, 7), L | D);
        assert_eq!(arms(&r, 1, 7), R | U | D);
        assert_eq!(arms(&r, 2, 7), R | U);
        assert_eq!(arms(&r, 2, 8), L | R);
        assert!(r.cells[&(1, 8)].arrow && r.cells[&(2, 11)].arrow);
    }

    #[test]
    fn no_dates_on_the_focus_or_no_links_draws_nothing() {
        let bars = [Some((0, 2)), None];
        assert_eq!(run(1, &[(0, 1)], &bars, 0..2, &[]), Route::default());
        assert!(!run(0, &[(0, 1)], &[Some((0, 2)), Some((5, 6))], 0..2, &[]).cells.is_empty());
        assert_eq!(run(0, &[], &bars, 0..2, &[]), Route::default());
        // 커서 줄이 화면 밖이면 그리지 않는다.
        assert_eq!(run(5, &[(0, 5)], &[Some((0, 2)); 6], 0..3, &[]), Route::default());
    }

    #[test]
    fn a_blocker_without_dates_leaves_a_mark() {
        let bars = [None, Some((9, 12))];
        let r = run(1, &[(0, 1)], &bars, 0..2, &[]);
        assert_eq!(r.marks, vec![(0, Dir::Down)]);
    }
}
