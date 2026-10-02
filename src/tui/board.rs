//! 보드 — 목록의 줄을 **칸과 레인에 놓는 법**(moai-9nfw).
//!
//! **새 창이 아니라 목록의 배치다**(사용자 결정 2026-10-02). 커서는 여전히 `App::rows` 의 첨자이고, 거름망·
//! 보기·검색·`[NEW]`·상세가 목록과 한 벌이다. 여기는 그 줄이 **화면 어디에 서는가**와 **커서가 어디로 가는가**
//! 만 정한다.
//!
//! **조각이다.** `App` 도 설정도 터미널도 모른다 — 줄마다 어느 레인·어느 칸인지는 든 쪽이 재서 넘기고, 여기는
//! 그것을 자리로 편다. 그래서 키와 그림이 같은 자리를 보고([`Plan`]), 시험이 터미널 없이 돈다.
//! `report`·`query` 는 안 바뀐다 — 칸은 데이터의 칸이 아니라 화면의 칸이다.

use super::keys::Side;
use super::scroll::{HALF, Move, PAGE};

/// 카드 하나가 가져가는 가장 좁은 폭 — 테두리 안쪽에서 칸을 고르게 나눌 때 이보다 좁아지면 칸을 덜 세운다.
/// 머리(`id · 글리프 · pN`)가 `moai-9nfw.isu ⠼▸ p1` 로 스무 칸 남짓이라, 이보다 좁으면 id 가 잘려 카드가
/// 무엇인지 모른다.
pub const CARD_MIN: usize = 18;

/// 보드의 칸. **데이터의 칸이 아니라 화면의 칸이다**(사용자 결정) — idea 는 종류(`kind`), 미룸은 축
/// (`deferred_at`)이라 저장하는 것은 없다. 셋을 한 줄로 놓을 뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    Idea,
    /// 미룬 줄 — 제가 미뤘든 물려받았든(`nav::Index::shelved_at`).
    Shelved,
    Status(String),
}

/// 그 줄이 서는 칸 — `idea` 는 그 줄이 담아 둔 생각인가(종류를 든 쪽이 잰다: 조각은 이슈를 모른다).
/// **종류가 먼저, 축이 다음, 칸이 끝이다** — 미룬 idea 는 idea 칸에 선다: idea 는 아직 일이
/// 아니라 미룸이 뜻이 없다(`moai idea` 는 미룰 일이 아니라 담아 둔 것이다). 미룬 일은 칸이 `in_progress`
/// 여도 미룸 칸이다 — 지금 누가 손대는 줄이 아니다(`Site::spins` 가 미룬 줄을 안 돌리는 것과 같은 자).
pub fn column_of(idea: bool, shelved: bool, status: &str) -> Column {
    match (idea, shelved) {
        (true, _) => Column::Idea,
        (_, true) => Column::Shelved,
        _ => Column::Status(status.to_string()),
    }
}

/// 보드에 세울 칸과 그 차례 — `idea · 미룸 · 설정의 칸`. `shows` 가 숨긴 칸은 빠진다. **카드가 든 칸은 늘
/// 선다**: 검색은 보기가 숨긴 줄까지 드러내고(moai-qnkn), 설정이 모르는 칸에 선 줄도 있다(읽기는 관대하다) —
/// 그 칸을 빼면 그 줄이 보드에서 말없이 사라진다. 설정이 모르는 칸은 끝에 처음 만난 차례로 선다.
pub fn columns(statuses: &[String], cards: &[Column], shows: &dyn Fn(&Column) -> bool) -> Vec<Column> {
    let held = |c: &Column| cards.contains(c);
    let mut out: Vec<Column> = [Column::Idea, Column::Shelved]
        .into_iter()
        .chain(statuses.iter().map(|s| Column::Status(s.clone())))
        .filter(|c| shows(c) || held(c))
        .collect();
    for c in cards {
        if !out.contains(c) {
            out.push(c.clone());
        }
    }
    out
}

/// 줄 하나가 들어오는 자리 — 든 쪽이 잰 것이다. 레인은 첫 카드가 나온 차례로 센 번호고, 칸은
/// [`columns`] 의 첨자다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub lane: usize,
    pub column: usize,
}

/// 카드 하나가 선 자리.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub lane: usize,
    pub column: usize,
    /// 그 레인·그 칸에서 몇째인가(0 부터).
    pub nth: usize,
    /// 보드 전체를 한 장으로 본 캔버스의 윗줄 — 굴린 자리(`Scroll`)가 이 자로 잰다.
    pub top: usize,
}

/// 레인 하나가 캔버스에서 차지하는 줄.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    pub top: usize,
    /// 머리줄까지 넣은 높이 — 가장 긴 칸의 카드 수 × 카드 높이.
    pub height: usize,
}

/// 보드 한 장의 배치 — 줄마다 선 자리와 레인의 자리. **키와 그림이 이 하나를 읽는다**: 둘이 저마다 재면
/// `j` 가 가는 카드와 화면의 아래 카드가 갈린다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// 줄(`App::rows` 의 첨자)마다 선 자리.
    pub cards: Vec<Placed>,
    pub lanes: Vec<Lane>,
    /// 칸 수 — [`columns`] 가 낸 것.
    pub columns: usize,
    /// 카드 하나의 줄 수 — 머리·몸에 발줄이 서면 셋이다.
    pub card_h: usize,
    /// 레인마다 머리줄이 서는가 — 뿌리에 레인이 둘 이상일 때다.
    pub headed: bool,
    /// 캔버스 전체의 줄 수.
    pub height: usize,
}

impl Plan {
    /// 줄마다 들어온 자리(`slots`)를 캔버스의 자리로 편다. 레인 안의 칸마다 **줄의 차례 그대로** 쌓는다 —
    /// 차례는 목록이 이미 정한 것(`SPC s`)이라 여기서 다시 정하지 않는다.
    ///
    /// `headed` 면 레인마다 머리줄 한 줄이 먼저 선다. 레인 하나의 높이는 그 안의 **가장 긴 칸**이다.
    pub fn of(slots: &[Slot], columns: usize, card_h: usize, headed: bool) -> Plan {
        let lanes_n = slots.iter().map(|s| s.lane + 1).max().unwrap_or(0);
        // 레인·칸마다 쌓인 수.
        let mut stack = vec![vec![0usize; columns]; lanes_n];
        let mut nth = Vec::with_capacity(slots.len());
        for s in slots {
            let n = &mut stack[s.lane][s.column];
            nth.push(*n);
            *n += 1;
        }
        let head = usize::from(headed);
        let mut lanes = Vec::with_capacity(lanes_n);
        let mut top = 0;
        for counts in &stack {
            let height = head + counts.iter().copied().max().unwrap_or(0) * card_h;
            lanes.push(Lane { top, height });
            top += height;
        }
        let cards = slots
            .iter()
            .zip(nth)
            .map(|(s, nth)| Placed {
                lane: s.lane,
                column: s.column,
                nth,
                top: lanes[s.lane].top + head + nth * card_h,
            })
            .collect();
        Plan { cards, lanes, columns, card_h, headed, height: top }
    }

    /// 그 칸에 선 카드 수 — 칸 머리줄의 셈이다.
    pub fn count(&self, column: usize) -> usize {
        self.cards.iter().filter(|c| c.column == column).count()
    }

    /// 그 칸의 카드를 **위에서 아래로** — 레인을 건너 이어진다. `j`·`k` 가 걷는 줄이다.
    fn run(&self, column: usize) -> Vec<usize> {
        let mut run: Vec<usize> = (0..self.cards.len()).filter(|&n| self.cards[n].column == column).collect();
        run.sort_by_key(|&n| self.cards[n].top);
        run
    }

    /// 그 칸에서 `top` 에 가장 가까운 카드 — 칸을 옮길 때 높이를 지키는 자다. 같으면 위의 것.
    fn nearest(&self, column: usize, top: usize) -> Option<usize> {
        self.run(column).into_iter().min_by_key(|&n| (self.cards[n].top.abs_diff(top), self.cards[n].top))
    }

    /// 커서의 칸 안에서 `by` 장 간다 — 끝에서 멈춘다. 키(`j`·`k`·반 쪽·한 쪽)와 휠이 같은 걸음을 탄다:
    /// 둘이 저마다 적으면 끝에서 멈추는 자가 갈린다.
    fn along(&self, cursor: usize, by: isize) -> usize {
        let Some(here) = self.cards.get(cursor) else { return cursor };
        let run = self.run(here.column);
        let at = run.iter().position(|&n| n == cursor).unwrap_or(0);
        run.get(at.saturating_add_signed(by).min(run.len().saturating_sub(1))).copied().unwrap_or(cursor)
    }
}

/// 커서가 가는 길 — 칸 안의 이동이거나 옆 칸이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Go {
    Move(Move),
    Side(Side),
}

/// 커서를 옮긴다. 옮길 자리를 낸다 — 못 가면 제자리다.
///
/// - `j`·`k`(그리고 반 쪽·한 쪽)는 **그 칸 안에서** 간다. 레인을 건너 이어진다 — 칸은 위에서 아래로 한 줄이다.
///   반 쪽·한 쪽은 카드 수로 센다(`scroll::HALF`·`PAGE`) — 목록과 같은 걸음이라 `Tab` 하나에 키의 뜻이 안 바뀐다
/// - `gg`·`G` 는 그 칸의 맨 위·맨 아래 카드다
/// - `h`·`l` 은 **옆 칸**으로 가고 지금 높이에 가장 가까운 카드에 선다. 빈 칸은 건너뛴다 — 설 카드가 없다
pub fn step(plan: &Plan, cursor: usize, go: Go) -> usize {
    let Some(here) = plan.cards.get(cursor) else { return cursor };
    let along = |by: isize| plan.along(cursor, by);
    match go {
        Go::Move(Move::LineDown) | Go::Side(Side::Down) => along(1),
        Go::Move(Move::LineUp) | Go::Side(Side::Up) => along(-1),
        Go::Move(Move::HalfDown) => along(HALF as isize),
        Go::Move(Move::HalfUp) => along(-(HALF as isize)),
        Go::Move(Move::PageDown) => along(PAGE as isize),
        Go::Move(Move::PageUp) => along(-(PAGE as isize)),
        Go::Move(Move::Top) => plan.run(here.column).first().copied().unwrap_or(cursor),
        Go::Move(Move::Bottom) => plan.run(here.column).last().copied().unwrap_or(cursor),
        Go::Side(side) => {
            let ahead: Box<dyn Iterator<Item = usize>> = match side {
                Side::Right => Box::new(here.column + 1..plan.columns),
                _ => Box::new((0..here.column).rev()),
            };
            ahead.filter_map(|c| plan.nearest(c, here.top)).next().unwrap_or(cursor)
        }
    }
}

/// 휠 한 칸(moai-irrj) — **마우스가 선 칸에서** 커서를 옮긴다. 커서가 다른 칸에 있으면 먼저 그 칸의 가장 가까운
/// 카드로 건너온다: 굴린 칸과 커서가 선 칸이 다르면 굴린 손이 무엇을 움직였는지 안 보인다. 그 칸이 비었으면
/// 제자리다.
pub fn roll(plan: &Plan, cursor: usize, column: usize, by: isize) -> usize {
    let Some(here) = plan.cards.get(cursor) else { return cursor };
    if here.column != column {
        return plan.nearest(column, here.top).unwrap_or(cursor);
    }
    plan.along(cursor, by)
}

/// 화면에 서는 칸의 창 — `first` 부터 `count` 개.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub first: usize,
    pub count: usize,
}

impl Window {
    pub fn holds(&self, column: usize) -> bool {
        (self.first..self.first + self.count).contains(&column)
    }
}

/// 폭 `width` 에 칸 `columns` 개를 세울 창. 칸마다 [`CARD_MIN`] 을 못 받으면 덜 세운다 — **커서가 선 칸은
/// 늘 선다**(`holding`): 안 서면 커서가 화면 밖 카드에 선 채 상세만 그 카드를 말한다. 못 세운 칸은 그리는 쪽이
/// 제목 줄에 이름과 수로 댄다.
///
/// **창은 지난 자리(`was`, 지난 프레임의 첫 칸)에 머물고, 커서가 창 밖으로 나갈 때만 가장 적게 민다** —
/// 목록의 [`super::scroll::Scroll::reveal`] 과 같은 자다. 커서의 칸을 늘 오른쪽 끝에 붙이면 보이는 왼쪽 칸으로
/// `h` 를 눌러도 창이 한 칸 밀리고, 휠은 마우스 밑의 칸이 걸음마다 바뀌어 굴린 손 밑의 칸 대신 왼쪽 끝까지
/// 칸을 건너간다.
pub fn window(columns: usize, width: usize, holding: Option<usize>, was: usize) -> Window {
    let count = (width / CARD_MIN).clamp(1, columns.max(1)).min(columns);
    let mut first = was.min(columns - count);
    match holding {
        Some(c) if c < first => first = c,
        Some(c) if c >= first + count => first = c + 1 - count,
        _ => {}
    }
    Window { first, count }
}

/// 폭을 칸마다 고르게 나눈다 — 남는 칸은 앞 칸부터 하나씩 더 받는다.
pub fn widths(width: usize, count: usize) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let (each, left) = (width / count, width % count);
    (0..count).map(|n| each + usize::from(n < left)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(lane: usize, column: usize) -> Slot {
        Slot { lane, column }
    }

    #[test]
    fn the_three_axes_fall_into_one_row_of_columns() {
        assert_eq!(column_of(true, true, "in_progress"), Column::Idea, "미룬 idea 는 idea 칸이다");
        assert_eq!(column_of(false, true, "in_progress"), Column::Shelved, "미룬 일은 미룸 칸이다");
        assert_eq!(column_of(false, false, "review"), Column::Status("review".into()));
    }

    /// 숨긴 칸은 빠지지만 **카드가 든 칸은 선다** — 검색이 드러낸 done 카드와 설정이 모르는 칸의 카드가
    /// 보드에서 말없이 사라지지 않는다.
    #[test]
    fn a_hidden_column_still_stands_while_it_holds_a_card() {
        let statuses: Vec<String> = ["todo", "done"].map(String::from).to_vec();
        let hides_done = |c: &Column| *c != Column::Status("done".into());
        let names = |cs: Vec<Column>| {
            cs.into_iter()
                .map(|c| match c {
                    Column::Idea => "idea".to_string(),
                    Column::Shelved => "shelved".into(),
                    Column::Status(s) => s,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(names(columns(&statuses, &[], &hides_done)), ["idea", "shelved", "todo"]);
        let held = [Column::Status("done".into()), Column::Status("blocked".into())];
        assert_eq!(names(columns(&statuses, &held, &hides_done)), ["idea", "shelved", "todo", "done", "blocked"]);
    }

    /// 레인의 높이는 가장 긴 칸이고, 카드는 줄의 차례대로 쌓인다.
    #[test]
    fn cards_stack_in_row_order_and_a_lane_is_as_tall_as_its_longest_column() {
        let slots = [slot(0, 1), slot(0, 1), slot(0, 2), slot(1, 0)];
        let p = Plan::of(&slots, 3, 2, true);
        assert_eq!(p.lanes, [Lane { top: 0, height: 1 + 2 * 2 }, Lane { top: 5, height: 1 + 2 }]);
        assert_eq!(p.cards.iter().map(|c| (c.nth, c.top)).collect::<Vec<_>>(), [(0, 1), (1, 3), (0, 1), (0, 6)]);
        assert_eq!(p.height, 8);
        assert_eq!((p.count(0), p.count(1), p.count(2)), (1, 2, 1));
        // 머리줄이 없으면 그 한 줄이 안 든다.
        assert_eq!(Plan::of(&slots[..3], 3, 2, false).cards[1].top, 2);
    }

    /// `j`·`k` 는 칸 안에서 레인을 건너 가고, 끝에서 멈춘다.
    #[test]
    fn j_and_k_walk_one_column_across_lanes() {
        // 레인 0: 칸 0 에 둘, 칸 1 에 하나. 레인 1: 칸 0 에 하나.
        let p = Plan::of(&[slot(0, 0), slot(0, 1), slot(0, 0), slot(1, 0)], 2, 2, true);
        let down = |n| step(&p, n, Go::Move(Move::LineDown));
        let up = |n| step(&p, n, Go::Move(Move::LineUp));
        assert_eq!((down(0), down(2), down(3)), (2, 3, 3), "칸 0 을 위에서 아래로 — 끝에서 멈춘다");
        assert_eq!((up(3), up(2), up(0)), (2, 0, 0));
        assert_eq!(down(1), 1, "혼자 선 칸에서는 제자리다");
        assert_eq!(step(&p, 0, Go::Move(Move::Bottom)), 3);
        assert_eq!(step(&p, 3, Go::Move(Move::Top)), 0);
        assert_eq!(step(&p, 0, Go::Move(Move::PageDown)), 3, "한 쪽이 끝을 넘지 않는다");
    }

    /// `h`·`l` 은 옆 칸의 **가장 가까운 높이**로 가고 빈 칸은 건너뛴다.
    #[test]
    fn h_and_l_keep_the_height_and_skip_empty_columns() {
        // 칸 0 에 셋(윗줄 0·2·4), 칸 1 은 비고, 칸 2 에 둘(윗줄 0·2).
        let p = Plan::of(&[slot(0, 0), slot(0, 0), slot(0, 0), slot(0, 2), slot(0, 2)], 3, 2, false);
        assert_eq!(step(&p, 2, Go::Side(Side::Right)), 4, "아래 카드에서 오른쪽은 가장 가까운 아래 카드다");
        assert_eq!(step(&p, 0, Go::Side(Side::Right)), 3);
        assert_eq!(step(&p, 4, Go::Side(Side::Left)), 1, "같은 높이의 카드로 돌아온다");
        assert_eq!(step(&p, 3, Go::Side(Side::Right)), 3, "오른쪽 끝에서는 제자리다");
        assert_eq!(step(&p, 1, Go::Side(Side::Left)), 1, "왼쪽 끝에서는 제자리다");
    }

    #[test]
    fn an_empty_board_keeps_the_cursor_where_it_is() {
        let p = Plan::of(&[], 3, 2, false);
        assert_eq!(step(&p, 0, Go::Move(Move::LineDown)), 0);
        assert_eq!(roll(&p, 0, 1, 3), 0);
        assert_eq!(p.height, 0);
    }

    /// 휠은 마우스가 선 칸을 굴린다 — 커서가 딴 칸이면 먼저 그 칸으로 건너온다.
    #[test]
    fn the_wheel_moves_within_the_column_under_the_mouse() {
        let p = Plan::of(&[slot(0, 0), slot(0, 0), slot(0, 0), slot(0, 1)], 2, 2, false);
        assert_eq!(roll(&p, 0, 0, 3), 2, "끝에서 멈춘다");
        assert_eq!(roll(&p, 2, 0, -3), 0);
        assert_eq!(roll(&p, 2, 1, 3), 3, "딴 칸이면 그 칸으로 건너온다");
        assert_eq!(roll(&p, 3, 0, 1), 0, "가장 가까운 높이로 건너온다");
    }

    /// 칸마다 [`CARD_MIN`] 을 못 받으면 덜 세우되, 커서가 선 칸은 늘 선다.
    #[test]
    fn a_narrow_board_keeps_the_column_the_cursor_stands_in() {
        assert_eq!(window(6, 120, Some(0), 0), Window { first: 0, count: 6 });
        assert_eq!(window(6, 40, Some(0), 0), Window { first: 0, count: 2 });
        assert_eq!(window(6, 40, Some(4), 0), Window { first: 3, count: 2 });
        assert!(window(6, 40, Some(4), 0).holds(4));
        assert_eq!(window(6, 5, None, 0), Window { first: 0, count: 1 }, "아무리 좁아도 한 칸은 선다");
        assert_eq!(window(0, 80, None, 0), Window { first: 0, count: 0 });
        assert_eq!(window(6, 120, Some(0), 4), Window { first: 0, count: 6 }, "다 서는데 지난 자리만큼 밀었다");
        assert_eq!(widths(23, 3), [8, 8, 7]);
        assert!(widths(10, 0).is_empty());
    }

    /// **창은 지난 자리에 머문다** — 보이는 칸으로 옮기면 창이 안 밀리고, 창 밖으로 나갈 때만 가장 적게 민다.
    /// 커서의 칸을 늘 오른쪽 끝에 붙이던 때는 왼쪽 칸 위에서 굴린 휠이 걸음마다 마우스 밑의 칸을 바꿔 왼쪽 끝까지
    /// 건너갔다.
    #[test]
    fn the_window_stays_put_while_the_cursor_column_is_in_it() {
        // 칸 3·4 가 서 있고 커서가 3 으로 왔다 — 3 은 이미 보인다.
        assert_eq!(window(6, 40, Some(3), 3), Window { first: 3, count: 2 });
        // 2 로 가면 그만큼만 민다.
        assert_eq!(window(6, 40, Some(2), 3), Window { first: 2, count: 2 });
        // 오른쪽으로 나가도 그만큼만.
        assert_eq!(window(6, 40, Some(5), 2), Window { first: 4, count: 2 });
        // 칸이 줄어 지난 자리가 넘치면 끝에 맞춘다.
        assert_eq!(window(3, 40, Some(2), 5), Window { first: 1, count: 2 });
    }
}
