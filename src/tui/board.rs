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
    /// 그 줄이 선 칸. **`None` 은 칸을 가로지르는 머리줄이다** — 한눈 보기의 프로젝트 머리줄(moai-oagj.vcj)은 카드가
    /// 아니지만 커서가 선다(사용자 결정). 어느 칸에도 안 들고 보드 폭을 다 쓴다.
    pub column: Option<usize>,
    /// 그 줄의 줄 수 — 카드는 머리·몸에 발줄이 서면 셋이고 머리줄은 하나다. **카드마다 다르다**(moai-oagj.y88): 남의
    /// 카드는 켠 열이 없어도 발줄을 달고 내 카드는 두 줄 그대로다.
    pub height: usize,
    /// 그 줄이 든 프로젝트 — 한눈 보기에서는 프로젝트마다 다르고, 프로젝트 안에서는 늘 0 이다. `h`·`l` 은 이 안에서만
    /// 옆 칸을 찾는다: 이 프로젝트의 옆 칸이 비었다고 다른 프로젝트의 카드로 건너가면 눌러 온 길로 못 돌아간다.
    pub group: usize,
}

/// 줄 하나가 선 자리 — 카드거나 머리줄([`Slot::column`] 이 `None`)이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub lane: usize,
    pub column: Option<usize>,
    /// 그 레인·그 칸에서 몇째인가(0 부터).
    pub nth: usize,
    /// 보드 전체를 한 장으로 본 캔버스의 윗줄 — 굴린 자리(`Scroll`)가 이 자로 잰다.
    pub top: usize,
    /// 그 줄의 줄 수([`Slot::height`]).
    pub height: usize,
    pub group: usize,
}

/// 레인 하나가 캔버스에서 차지하는 줄.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lane {
    pub top: usize,
    /// 머리줄까지 넣은 높이 — 가장 긴 칸의 카드 높이를 더한 것.
    pub height: usize,
    /// 맨 위에 레인 머리줄(마일스톤의 이름) 한 줄이 서는가. **레인마다 다르다**(moai-oagj.vcj) — 한눈 보기에서는
    /// 마일스톤을 쓰는 프로젝트의 레인에만 서고, 프로젝트 머리줄의 레인에는 안 선다.
    pub head: bool,
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
    /// 캔버스 전체의 줄 수.
    pub height: usize,
}

impl Plan {
    /// 줄마다 들어온 자리(`slots`)를 캔버스의 자리로 편다. 레인 안의 칸마다 **줄의 차례 그대로** 쌓는다 —
    /// 차례는 목록이 이미 정한 것(`SPC s`)이라 여기서 다시 정하지 않는다.
    ///
    /// `heads[n]` 이면 레인 `n` 에 머리줄 한 줄이 먼저 선다. 레인 하나의 높이는 그 안의 **가장 긴 칸**이다 — 카드의
    /// 높이가 저마다라(moai-oagj.y88) 장 수가 아니라 줄 수로 잰다. 칸을 가로지르는 머리줄은 칸 밖의 자리 하나에
    /// 쌓인다.
    pub fn of(slots: &[Slot], columns: usize, heads: &[bool]) -> Plan {
        let lanes_n = slots.iter().map(|s| s.lane + 1).max().unwrap_or(0);
        // 레인·칸마다 쌓인 장 수와 줄 수. 카드의 자리는 그 칸에서 앞 카드들이 차지한 줄 밑이다. 끝 자리(`columns`)가
        // 칸을 가로지르는 머리줄의 것이다.
        let mut stack = vec![vec![(0usize, 0usize); columns + 1]; lanes_n];
        let mut below = Vec::with_capacity(slots.len());
        for s in slots {
            let (n, lines) = &mut stack[s.lane][s.column.unwrap_or(columns)];
            below.push((*n, *lines));
            *n += 1;
            *lines += s.height;
        }
        let mut lanes = Vec::with_capacity(lanes_n);
        let mut top = 0;
        for (n, counts) in stack.iter().enumerate() {
            let head = heads.get(n).copied().unwrap_or(false);
            let height = usize::from(head) + counts.iter().map(|&(_, lines)| lines).max().unwrap_or(0);
            lanes.push(Lane { top, height, head });
            top += height;
        }
        let cards = slots
            .iter()
            .zip(below)
            .map(|(s, (nth, lines))| {
                let lane = lanes[s.lane];
                Placed {
                    lane: s.lane,
                    column: s.column,
                    nth,
                    top: lane.top + usize::from(lane.head) + lines,
                    height: s.height,
                    group: s.group,
                }
            })
            .collect();
        Plan { cards, lanes, columns, height: top }
    }

    /// 그 칸에 선 카드 수 — 칸 머리줄의 셈이다. 칸을 가로지르는 머리줄은 안 센다.
    pub fn count(&self, column: usize) -> usize {
        self.cards.iter().filter(|c| c.column == Some(column)).count()
    }

    /// 그 칸의 카드를 **위에서 아래로** — 레인을 건너 이어진다. `j`·`k` 가 걷는 줄이다. **칸을 가로지르는 머리줄은
    /// 모든 칸의 길에 든다**(moai-oagj.vcj, 사용자 결정) — 그 프로젝트에서 그 칸의 맨 위 카드에서 `k` 가 머리줄에
    /// 선다. 아래 레인의 맨 위 카드에서는 윗 레인의 같은 칸 카드로 간다 — 칸은 레인을 건너 한 줄이다. 칸이
    /// 없으면(`None` — 아직 아무 카드에도 안 서 본 머리줄) 머리줄만의 길이다.
    fn run(&self, column: Option<usize>) -> Vec<usize> {
        let mut run: Vec<usize> =
            (0..self.cards.len()).filter(|&n| self.cards[n].column.is_none_or(|c| Some(c) == column)).collect();
        run.sort_by_key(|&n| self.cards[n].top);
        run
    }

    /// 그 칸에서 `top` 에 가장 가까운 카드 — 칸을 옮길 때 높이를 지키는 자다. 같으면 위의 것. **`group` 을 주면
    /// 그 프로젝트 안에서만 찾는다** — `h`·`l` 의 울타리다([`step`]). 휠은 없으면 울타리 없이 다시 찾는다([`roll`]).
    fn nearest(&self, column: usize, top: usize, group: Option<usize>) -> Option<usize> {
        (0..self.cards.len())
            .filter(|&n| self.cards[n].column == Some(column) && group.is_none_or(|g| self.cards[n].group == g))
            .min_by_key(|&n| (self.cards[n].top.abs_diff(top), self.cards[n].top))
    }

    /// 머리줄에서 한 걸음 — 아래(`down`)면 그 프로젝트의 카드로, 위면 앞 프로젝트의 카드로 간다. **`hint` 의 칸에
    /// 카드가 있으면 그 칸이고**(`k` 로 올라온 칸으로 돌아간다), 없으면 가장 가까운 줄의 카드 — 아래로는 맨 위의,
    /// 위로는 맨 아래의 것이다. 칸 길(`run`)만 타면 처음 선 머리줄의 칸(`idea`)이 비었을 때 `j` 가 그 프로젝트의 카드를
    /// 통째로 건너 다음 머리줄로 간다. 그 프로젝트에 카드가 없으면(접혔거나 빈 프로젝트) 옆 머리줄이다. `hint` 가
    /// 없으면(아직 아무 카드에도 안 섰다) 처음부터 가장 가까운 카드다.
    fn off_banner(&self, cursor: usize, down: bool, hint: Option<usize>) -> usize {
        let Some(here) = self.cards.get(cursor) else { return cursor };
        // 걸음 쪽에 선 줄과 그 거리 — 아래로든 위로든 가까운 것이 먼저다.
        let ahead = |n: usize| if down { self.cards[n].top > here.top } else { self.cards[n].top < here.top };
        let dist = |n: usize| self.cards[n].top.abs_diff(here.top);
        // 넘어갈 머리줄 — 그 사이가 이 걸음이 닿는 카드의 자리다.
        let fence =
            (0..self.cards.len()).filter(|&n| self.cards[n].column.is_none() && ahead(n)).min_by_key(|&n| dist(n));
        let cards: Vec<usize> = (0..self.cards.len())
            .filter(|&n| self.cards[n].column.is_some() && ahead(n) && fence.is_none_or(|f| dist(n) < dist(f)))
            .collect();
        let near = |n: &usize| (dist(*n), self.cards[*n].column);
        cards
            .iter()
            .copied()
            .filter(|&n| hint.is_some() && self.cards[n].column == hint)
            .min_by_key(near)
            .or_else(|| cards.iter().copied().min_by_key(near))
            .or(fence)
            .unwrap_or(cursor)
    }

    /// 커서의 칸 안에서 `by` 장 간다 — 끝에서 멈춘다. 키(`j`·`k`·반 쪽·한 쪽)와 휠이 같은 걸음을 탄다:
    /// 둘이 저마다 적으면 끝에서 멈추는 자가 갈린다. 머리줄에 선 커서는 `hint` 의 칸을 걷는다([`step`]).
    fn along(&self, cursor: usize, by: isize, hint: Option<usize>) -> usize {
        let Some(here) = self.cards.get(cursor) else { return cursor };
        let run = self.run(here.column.or(hint));
        let at = run.iter().position(|&n| n == cursor).unwrap_or(0);
        run.get(at.saturating_add_signed(by).min(run.len().saturating_sub(1))).copied().unwrap_or(cursor)
    }

    /// `by` 걸음 간다 — 끝에서 멈춘다. **한 걸음은 `j`·`k` 의 걸음이다**: 카드에서는 그 칸의 길로([`Plan::along`]),
    /// 머리줄에서는 그 프로젝트의 카드로([`Plan::off_banner`]). 반 쪽·한 쪽도 그 걸음을 그만큼 되풀이한다(리뷰) — 칸
    /// 길로 한 번에 건너던 때는 머리줄에서 누른 반 쪽이 그 칸에 카드가 없는 프로젝트를 통째로 건너 다음 머리줄에
    /// 섰다. 지나온 카드의 칸이 다음 머리줄의 `hint` 다 — 키로 걸을 때 `App::board_column` 이 서는 것과 같다.
    fn walk(&self, cursor: usize, by: isize, hint: Option<usize>) -> usize {
        let (mut at, mut hint) = (cursor, hint);
        for _ in 0..by.unsigned_abs() {
            let Some(here) = self.cards.get(at) else { break };
            hint = here.column.or(hint);
            let next = match here.column {
                Some(_) => self.along(at, by.signum(), hint),
                None => self.off_banner(at, by > 0, hint),
            };
            if next == at {
                break;
            }
            at = next;
        }
        at
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
///   반 쪽·한 쪽은 카드 수로 센다(`scroll::HALF`·`PAGE`) — 목록과 같은 걸음이라 `Tab` 하나에 키의 뜻이 안 바뀐다.
///   `j`·`k` 의 걸음을 그만큼 되풀이한다([`Plan::walk`]) — 머리줄을 지나도 `j` 를 거듭 누른 것과 같은 자리다
/// - `gg`·`G` 는 그 칸의 맨 위·맨 아래 카드다
/// - `h`·`l` 은 **옆 칸**으로 가고 지금 높이에 가장 가까운 카드에 선다. 빈 칸은 건너뛴다 — 설 카드가 없다.
///   같은 프로젝트 안에서만 찾는다
/// - 칸을 가로지르는 머리줄(한눈 보기의 프로젝트)에는 칸이 없어 **`hint` 의 칸을 걷는다** — 부르는 쪽이 커서가 마지막으로
///   선 카드의 칸을 들고 있다. `k` 로 머리줄에 올라온 칸으로 `j` 가 돌아가고, 그 칸이 그 프로젝트에서 비었으면 가장
///   가까운 카드다([`Plan::off_banner`]). 아직 아무 카드에도 안 섰으면 `hint` 가 없다 — `j`·`k`·반 쪽·한 쪽은 가장
///   가까운 카드로, 맨 위아래는 머리줄만의 길로 간다. 그 자리의 `h`·`l` 은 프로젝트를 접고 펴는 키라 여기 오지
///   않는다 — 와도 제자리다
pub fn step(plan: &Plan, cursor: usize, go: Go, hint: Option<usize>) -> usize {
    let Some(here) = plan.cards.get(cursor) else { return cursor };
    let walk = |by: isize| plan.walk(cursor, by, hint);
    let column = here.column.or(hint);
    match go {
        Go::Move(Move::LineDown) | Go::Side(Side::Down) => walk(1),
        Go::Move(Move::LineUp) | Go::Side(Side::Up) => walk(-1),
        Go::Move(Move::HalfDown) => walk(HALF as isize),
        Go::Move(Move::HalfUp) => walk(-(HALF as isize)),
        Go::Move(Move::PageDown) => walk(PAGE as isize),
        Go::Move(Move::PageUp) => walk(-(PAGE as isize)),
        Go::Move(Move::Top) => plan.run(column).first().copied().unwrap_or(cursor),
        Go::Move(Move::Bottom) => plan.run(column).last().copied().unwrap_or(cursor),
        Go::Side(side) => {
            let Some(column) = here.column else { return cursor };
            let ahead: Box<dyn Iterator<Item = usize>> = match side {
                Side::Right => Box::new(column + 1..plan.columns),
                _ => Box::new((0..column).rev()),
            };
            ahead.filter_map(|c| plan.nearest(c, here.top, Some(here.group))).next().unwrap_or(cursor)
        }
    }
}

/// 휠 한 칸(moai-irrj) — **마우스가 선 칸에서** 커서를 옮긴다. 커서가 다른 칸에 있으면 먼저 그 칸의 가장 가까운
/// 카드로 건너온다: 굴린 칸과 커서가 선 칸이 다르면 굴린 손이 무엇을 움직였는지 안 보인다. 그 칸이 비었으면
/// 제자리다.
///
/// **머리줄에 선 커서는 그 칸의 길을 굴린 쪽으로 걷는다** — 머리줄은 모든 칸의 길에 든다([`Plan::run`]). 그 프로젝트의
/// 카드로 늘 건너오던 때는(리뷰) 위로 굴린 휠이 머리줄과 그 밑 카드 사이를 오가 앞 프로젝트로 못 올라갔고, 그 칸에
/// 카드가 없는 프로젝트의 머리줄에서는 아래로도 못 갔다.
///
/// **건너올 때는 같은 프로젝트의 카드가 먼저고, 없으면 그 칸의 어느 카드든 가장 가까운 것이다**(리뷰) — 프로젝트의
/// 울타리(`group`)는 `h`·`l` 의 것이다. 휠까지 그 울타리를 타던 때는 다른 프로젝트의 카드만 선 칸을 굴려도 커서가
/// 꼼짝 않았다.
pub fn roll(plan: &Plan, cursor: usize, column: usize, by: isize) -> usize {
    let Some(here) = plan.cards.get(cursor) else { return cursor };
    if here.column.is_some_and(|c| c != column) {
        return plan
            .nearest(column, here.top, Some(here.group))
            .or_else(|| plan.nearest(column, here.top, None))
            .unwrap_or(cursor);
    }
    plan.along(cursor, by, Some(column))
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

    /// 두 줄짜리 카드 — 발줄 없는 카드다.
    fn slot(lane: usize, column: usize) -> Slot {
        Slot { lane, column: Some(column), height: 2, group: 0 }
    }

    /// 칸 길로 한 걸음 — 머리줄이 없는 보드라 `hint` 는 뜻이 없다.
    fn step(p: &Plan, cursor: usize, go: Go) -> usize {
        super::step(p, cursor, go, None)
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
        let p = Plan::of(&slots, 3, &[true, true]);
        assert_eq!(
            p.lanes,
            [Lane { top: 0, height: 1 + 2 * 2, head: true }, Lane { top: 5, height: 1 + 2, head: true }]
        );
        assert_eq!(p.cards.iter().map(|c| (c.nth, c.top)).collect::<Vec<_>>(), [(0, 1), (1, 3), (0, 1), (0, 6)]);
        assert_eq!(p.height, 8);
        assert_eq!((p.count(0), p.count(1), p.count(2)), (1, 2, 1));
        // 머리줄이 없으면 그 한 줄이 안 든다.
        assert_eq!(Plan::of(&slots[..3], 3, &[]).cards[1].top, 2);
    }

    /// **카드의 높이는 저마다다**(moai-oagj.y88) — 발줄을 단 카드 밑의 카드는 그만큼 내려서고, 레인은 장 수가 아니라
    /// 줄 수가 가장 많은 칸만큼 높다.
    #[test]
    fn a_card_with_a_foot_pushes_the_next_one_down_and_the_lane_counts_lines() {
        let tall = |lane, column| Slot { lane, column: Some(column), height: 3, group: 0 };
        // 칸 0: 셋째 줄이 선 카드 위에 두 줄 카드. 칸 1: 두 줄 카드 둘 — 장 수는 같고 줄 수는 칸 0 이 많다.
        let slots = [tall(0, 0), slot(0, 0), slot(0, 1), slot(0, 1), slot(1, 1)];
        let p = Plan::of(&slots, 2, &[true, true]);
        assert_eq!(
            p.cards.iter().map(|c| (c.top, c.height)).collect::<Vec<_>>(),
            [(1, 3), (4, 2), (1, 2), (3, 2), (7, 2)]
        );
        assert_eq!(
            p.lanes,
            [Lane { top: 0, height: 1 + 3 + 2, head: true }, Lane { top: 6, height: 1 + 2, head: true }]
        );
        // 옆 칸으로 옮기는 자는 줄로 잰 높이다 — 칸 1 의 둘째 카드(윗줄 3)에서 왼쪽은 윗줄 4 의 카드다.
        assert_eq!(step(&p, 3, Go::Side(Side::Left)), 1);
    }

    /// `j`·`k` 는 칸 안에서 레인을 건너 가고, 끝에서 멈춘다.
    #[test]
    fn j_and_k_walk_one_column_across_lanes() {
        // 레인 0: 칸 0 에 둘, 칸 1 에 하나. 레인 1: 칸 0 에 하나.
        let p = Plan::of(&[slot(0, 0), slot(0, 1), slot(0, 0), slot(1, 0)], 2, &[true, true]);
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
        let p = Plan::of(&[slot(0, 0), slot(0, 0), slot(0, 0), slot(0, 2), slot(0, 2)], 3, &[]);
        assert_eq!(step(&p, 2, Go::Side(Side::Right)), 4, "아래 카드에서 오른쪽은 가장 가까운 아래 카드다");
        assert_eq!(step(&p, 0, Go::Side(Side::Right)), 3);
        assert_eq!(step(&p, 4, Go::Side(Side::Left)), 1, "같은 높이의 카드로 돌아온다");
        assert_eq!(step(&p, 3, Go::Side(Side::Right)), 3, "오른쪽 끝에서는 제자리다");
        assert_eq!(step(&p, 1, Go::Side(Side::Left)), 1, "왼쪽 끝에서는 제자리다");
    }

    /// **한눈 보기의 프로젝트 머리줄은 칸을 가로지르고 커서가 선다**(moai-oagj.vcj, 사용자 결정) — 레인 맨 위 카드에서
    /// `k` 가 그 머리줄로 가고, 머리줄에서 `j` 는 올라온 칸으로 돌아간다. `h`·`l` 은 같은 프로젝트 안에서만 옆 칸을
    /// 찾는다.
    #[test]
    fn a_project_header_spans_the_columns_and_sits_on_every_column_path() {
        let banner = |lane, group| Slot { lane, column: None, height: 1, group };
        let card = |lane, column, group| Slot { lane, column: Some(column), height: 2, group };
        // 0: 프로젝트 A 머리줄, 1·2: A 의 칸 0 카드 둘, 3: A 의 칸 1 카드, 4: 프로젝트 B 머리줄, 5: B 의 칸 0 카드.
        let slots = [banner(0, 0), card(1, 0, 0), card(1, 0, 0), card(1, 1, 0), banner(2, 1), card(3, 0, 1)];
        let p = Plan::of(&slots, 2, &[false, true, false, false]);
        assert_eq!(p.cards.iter().map(|c| c.top).collect::<Vec<_>>(), [0, 2, 4, 2, 6, 7]);
        assert_eq!(p.height, 9);
        assert_eq!((p.count(0), p.count(1)), (3, 1), "머리줄을 칸의 셈에 넣었다");
        let go = |n, m, hint| super::step(&p, n, Go::Move(m), Some(hint));
        assert_eq!(go(1, Move::LineUp, 0), 0, "레인 맨 위 카드의 k 가 머리줄로 안 갔다");
        assert_eq!(go(3, Move::LineUp, 1), 0, "옆 칸의 맨 위 카드도 같은 머리줄로 간다");
        assert_eq!(go(0, Move::LineDown, 1), 3, "머리줄의 j 가 올라온 칸으로 안 돌아갔다");
        assert_eq!(go(0, Move::LineDown, 0), 1);
        assert_eq!(go(2, Move::LineDown, 0), 4, "프로젝트의 끝 카드에서 j 가 다음 머리줄로 안 갔다");
        assert_eq!(go(4, Move::LineDown, 0), 5);
        assert_eq!(go(4, Move::LineDown, 1), 5, "그 칸이 비었다고 그 프로젝트의 카드를 건너뛰었다");
        assert_eq!(go(4, Move::LineUp, 1), 3, "머리줄의 k 가 앞 프로젝트의 그 칸 끝 카드로 안 갔다");
        assert_eq!(go(4, Move::LineUp, 0), 2);
        assert_eq!(go(0, Move::LineUp, 0), 0, "맨 위 머리줄에서 k 가 움직였다");
        assert_eq!(go(5, Move::LineDown, 0), 5, "끝 카드에서 j 가 움직였다");
        assert_eq!(go(5, Move::Top, 0), 0, "gg 가 맨 위 머리줄로 안 갔다");
        // 옆 칸은 같은 프로젝트 안에서만 — B 의 칸 1 은 비었으니 A 의 카드로 건너가지 않는다.
        assert_eq!(step(&p, 5, Go::Side(Side::Right)), 5, "다른 프로젝트의 카드로 건너갔다");
        assert_eq!(step(&p, 3, Go::Side(Side::Left)), 1);
        assert_eq!(step(&p, 0, Go::Side(Side::Right)), 0, "머리줄에서 옆 칸으로 갔다");
        // 휠은 머리줄의 커서를 그 프로젝트의 카드로 건너오게 한다.
        assert_eq!(roll(&p, 4, 0, 1), 5);
        assert_eq!(roll(&p, 4, 1, 1), 4, "그 프로젝트에 없는 칸이면 제자리다");
    }

    #[test]
    fn an_empty_board_keeps_the_cursor_where_it_is() {
        let p = Plan::of(&[], 3, &[]);
        assert_eq!(step(&p, 0, Go::Move(Move::LineDown)), 0);
        assert_eq!(roll(&p, 0, 1, 3), 0);
        assert_eq!(p.height, 0);
    }

    /// 휠은 마우스가 선 칸을 굴린다 — 커서가 딴 칸이면 먼저 그 칸으로 건너온다.
    #[test]
    fn the_wheel_moves_within_the_column_under_the_mouse() {
        let p = Plan::of(&[slot(0, 0), slot(0, 0), slot(0, 0), slot(0, 1)], 2, &[]);
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

    /// **휠은 머리줄을 굴린 쪽으로 지나간다**(리뷰) — 머리줄은 모든 칸의 길에 든다([`Plan::run`]). 그 프로젝트의 카드로 늘
    /// 건너오던 때는 위로 굴린 휠이 머리줄과 그 밑 카드 사이를 오가 앞 프로젝트로 못 올라갔고, 그 칸에 카드가 없는
    /// 프로젝트의 머리줄에서는 아래로도 못 갔다.
    #[test]
    fn the_wheel_passes_a_project_header_in_the_way_it_rolls() {
        let banner = |lane, group| Slot { lane, column: None, height: 1, group };
        let card = |lane, column, group| Slot { lane, column: Some(column), height: 2, group };
        // 0: A 머리줄, 1: A 의 칸 0, 2: B 머리줄, 3: B 의 칸 0, 4: C 머리줄, 5: C 의 칸 1 뿐, 6: D 머리줄, 7: D 의 칸 0.
        let slots = [
            banner(0, 0),
            card(1, 0, 0),
            banner(2, 1),
            card(3, 0, 1),
            banner(4, 2),
            card(5, 1, 2),
            banner(6, 3),
            card(7, 0, 3),
        ];
        let p = Plan::of(&slots, 2, &[]);
        assert_eq!(roll(&p, 3, 0, -1), 2);
        assert_eq!(roll(&p, 2, 0, -1), 1, "위로 굴린 휠이 머리줄에서 그 밑 카드로 되돌아갔다");
        assert_eq!(roll(&p, 3, 0, 1), 4);
        assert_eq!(roll(&p, 4, 0, 1), 6, "그 칸에 카드가 없는 프로젝트의 머리줄에서 휠이 멈췄다");
        assert_eq!(roll(&p, 6, 0, 1), 7);
    }

    /// **아직 아무 카드에도 안 서 본 머리줄의 `j` 는 가장 가까운 카드다**(리뷰) — 처음값으로 첫 칸(idea)을 들던 때는 맨
    /// 아래 `(마일스톤 없음)` 레인의 idea 로 내려가, 그 위의 마일스톤 레인을 통째로 건넜다. 칸을 들고 있으면 그 칸이다.
    #[test]
    fn a_header_with_no_column_yet_steps_to_the_nearest_card() {
        // 레인 1(마일스톤): 칸 2 의 카드. 레인 2(마일스톤 없음): 칸 0 의 idea.
        let slots = [
            Slot { lane: 0, column: None, height: 1, group: 0 },
            Slot { lane: 1, column: Some(2), height: 2, group: 0 },
            Slot { lane: 2, column: Some(0), height: 2, group: 0 },
        ];
        let p = Plan::of(&slots, 3, &[false, true, true]);
        assert_eq!(super::step(&p, 0, Go::Move(Move::LineDown), None), 1, "가장 가까운 카드로 안 갔다");
        assert_eq!(super::step(&p, 0, Go::Move(Move::LineDown), Some(0)), 2, "들고 있던 칸으로 안 갔다");
    }

    /// **휠은 그 프로젝트에 없는 칸이어도 그 칸의 카드로 건너온다**(리뷰) — 프로젝트의 울타리는 `h`·`l` 의 것이다. 휠까지
    /// 그 울타리를 타던 때는 다른 프로젝트의 카드만 선 칸을 굴려도 커서가 꼼짝 않았다.
    #[test]
    fn the_wheel_crosses_into_a_column_only_another_project_has() {
        let banner = |lane, group| Slot { lane, column: None, height: 1, group };
        let card = |lane, column, group| Slot { lane, column: Some(column), height: 2, group };
        // 0: A 머리줄, 1: A 의 칸 0, 2: B 머리줄, 3: B 의 칸 1.
        let slots = [banner(0, 0), card(1, 0, 0), banner(2, 1), card(3, 1, 1)];
        let p = Plan::of(&slots, 2, &[]);
        assert_eq!(roll(&p, 1, 1, 1), 3, "다른 프로젝트의 카드만 선 칸을 굴렸는데 커서가 꼼짝 않았다");
        assert_eq!(step(&p, 1, Go::Side(Side::Right)), 1, "l 이 다른 프로젝트의 카드로 건너갔다");
    }

    /// **머리줄에서 누른 반 쪽·한 쪽은 `j` 를 그만큼 누른 것과 같다**(리뷰) — 그 칸의 길로 한 번에 건너던 때는 그 칸에
    /// 카드가 없는(아직 아무 칸도 안 든 머리줄이면 어느 프로젝트든) 프로젝트를 통째로 건너 다음 머리줄에 섰다.
    #[test]
    fn half_a_page_from_a_header_walks_into_its_project() {
        let banner = |lane, group| Slot { lane, column: None, height: 1, group };
        let card = |lane, column, group| Slot { lane, column: Some(column), height: 2, group };
        // 0: A 머리줄, 1~6: A 의 칸 1, 7: B 머리줄, 8: B 의 칸 1.
        let mut slots = vec![banner(0, 0)];
        slots.extend(std::iter::repeat_n(card(1, 1, 0), 6));
        slots.extend([banner(2, 1), card(3, 1, 1)]);
        let p = Plan::of(&slots, 2, &[]);
        let go = |n, m| super::step(&p, n, Go::Move(m), None);
        assert_eq!(go(0, Move::HalfDown), 5, "반 쪽이 그 프로젝트를 건넜다");
        assert_eq!(go(0, Move::PageDown), 8, "한 쪽이 머리줄을 지나 다음 프로젝트로 안 갔다");
        assert_eq!(go(7, Move::HalfUp), 2, "위로 반 쪽이 앞 프로젝트로 안 들어갔다");
    }
}
