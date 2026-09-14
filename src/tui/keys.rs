//! 키 → 동작 표(moai-nc7w). **키의 뜻은 여기 한 곳에 적는다.**
//!
//! 키 처리(`App::key`·[`super::picker::Picker::key`]·[`super::form::Form::key`])는 여기서
//! 동작을 찾아 실행하고, 층의 거절문은 [`Browse::enabled`] 가 내고, 키 바·안내 문구는
//! [`label`] 로 키 이름을 읽는다. 갈라져 있던 때에는 한쪽만 고친 키가 다른 쪽에서 옛 뜻으로
//! 남았다 — "Ctrl·Alt 붙은 키가 층의 거절을 우회" (moai-4ia9.wpz) 가 그 갈라짐에서 왔다.
//!
//! **조각이다.** `KeyEvent` 와 표만 안다 — `App`·터미널·저장소를 모른다. 켜짐을 가르는 값
//! ([`Ctx`])은 든 쪽이 재서 넘긴다. 그래서 시험이 터미널 없이 표를 훑는다.
//!
//! **한 줄은 키의 열이다**([`Bind::seq`]). `gg`·`g p`·`SPC t w` 같은 접두어를 줄 하나로 적고
//! [`lookup`] 이 [`Lookup::Pending`] 으로 "더 기다린다" 를 낸다. 기다리는 동안의 열은 든 쪽이
//! [`Chord`] 로 들고 다음 키를 붙여 다시 부른다.

use super::scroll::Move;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

/// 키 하나의 모양. 수식키는 **`care` 안에서만** 견준다 — `care` 밖의 수식키는 붙어 와도 같다.
///
/// - **글자는 SHIFT 를 떼고 견준다.** 터미널은 `G` 를 `Char('G')`+SHIFT 로 보낸다. SHIFT 까지
///   견주면 `G` 가 안 먹는다. 그래서 글자 키에 SHIFT 를 요구하는 줄은 만들지 않는다
/// - **`BackTab` 은 `Tab`+SHIFT 로 편다.** 터미널에 따라 Shift-Tab 이 둘 중 하나로 온다 —
///   한 줄([`Key::shift`])이 둘 다 받는다
/// - **글자 키의 Ctrl·Alt 는 정확히 견준다**([`Key::plain`]·[`Key::ctrl`]). Ctrl-A 가 `a` 로
///   먹으면 등록 창이 뜬다. 오늘 수식키를 안 보는 비글자 키(화살표·Enter·F키)는
///   [`Key::any`] 로 그대로 둔다
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    code: KeyCode,
    want: KeyModifiers,
    care: KeyModifiers,
}

impl Key {
    /// Ctrl·Alt 없이 누른 글자.
    pub const fn plain(c: char) -> Key {
        Key::bare(KeyCode::Char(c))
    }

    /// Ctrl 과 함께 누른 글자. **Alt 는 안 본다** — 오늘 Ctrl-C·Ctrl-S 가 그렇다.
    pub const fn ctrl(c: char) -> Key {
        Key { code: KeyCode::Char(c), want: KeyModifiers::CONTROL, care: KeyModifiers::CONTROL }
    }

    /// Ctrl 과 함께, **Alt 없이** 누른 글자 — vi 이동(Ctrl-d·Ctrl-u·Ctrl-f·Ctrl-b). 새로 더하는
    /// 글자 조합은 정확히 견준다: Ctrl-Alt-d 가 반 쪽을 내리면 터미널이 Alt 를 Esc 로 보낼 때
    /// 사람이 안 친 이동이 생긴다. Ctrl-C·Ctrl-S 는 옛 모양 그대로 [`Key::ctrl`] 이다.
    pub const fn chord(c: char) -> Key {
        Key { code: KeyCode::Char(c), want: KeyModifiers::CONTROL, care: CTRL_ALT }
    }

    /// Ctrl·Alt 없이 누른 키.
    pub const fn bare(code: KeyCode) -> Key {
        Key { code, want: KeyModifiers::NONE, care: CTRL_ALT }
    }

    /// 수식키를 안 보는 키.
    pub const fn any(code: KeyCode) -> Key {
        Key { code, want: KeyModifiers::NONE, care: KeyModifiers::NONE }
    }

    /// Ctrl 없이 누른 키. Alt 는 안 본다 — 글칸의 Enter·Esc 가 오늘 그렇다.
    pub const fn unctrl(code: KeyCode) -> Key {
        Key { code, want: KeyModifiers::NONE, care: KeyModifiers::CONTROL }
    }

    /// SHIFT 와 함께 누른 비글자 키(Shift-Tab). 그 밖의 수식키는 안 본다.
    pub const fn shift(code: KeyCode) -> Key {
        Key { code, want: KeyModifiers::SHIFT, care: KeyModifiers::SHIFT }
    }

    /// SHIFT 없이 누른 비글자 키. 그 밖의 수식키는 안 본다.
    pub const fn unshift(code: KeyCode) -> Key {
        Key { code, want: KeyModifiers::NONE, care: KeyModifiers::SHIFT }
    }

    pub fn matches(self, k: KeyEvent) -> bool {
        let (code, mods) = normal(k);
        code == self.code && mods.intersection(self.care) == self.want
    }

    /// 이 키를 누른 모양 — 원하는 수식키만 붙인다. 메뉴가 하위 접두어의 열을 만들어 표에서
    /// 다시 찾을 때 쓴다.
    pub fn event(self) -> KeyEvent {
        KeyEvent::new(self.code, self.want)
    }

    /// 사람에게 대는 한 키의 이름 — 메뉴의 줄과 테두리(`SPC t`).
    pub fn name(self) -> String {
        name_of(self.code)
    }
}

/// 한 키의 이름. [`parse`] 가 거꾸로 읽는 이름과 같다 — SPC 는 `SPC`, 글자는 그 글자.
pub fn name_of(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "SPC".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Backspace => "Bksp".into(),
        KeyCode::Tab => "Tab".into(),
        other => format!("{other:?}"),
    }
}

/// 터미널마다 갈리는 모양을 한 모양으로 편다.
fn normal(k: KeyEvent) -> (KeyCode, KeyModifiers) {
    match k.code {
        KeyCode::Char(_) => (k.code, k.modifiers.difference(KeyModifiers::SHIFT)),
        KeyCode::BackTab => (KeyCode::Tab, k.modifiers.union(KeyModifiers::SHIFT)),
        _ => (k.code, k.modifiers),
    }
}

/// 표의 한 줄 — 키의 열 → 동작.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bind<A: 'static> {
    pub seq: &'static [Key],
    pub act: A,
    /// 사람에게 대는 키 이름. **`None` 은 숨은 별칭이다** — 눌러도 되지만 바에도 문구에도 안
    /// 적는다(화살표·`h`·`l`·BackTab·`SPC /`). 이름은 그 키로 다시 읽혀야 한다 —
    /// 시험이 이름을 키 열로 풀어(`gg`·`g p`) 이 줄의 동작이 나오는지 본다.
    pub label: Option<&'static str>,
}

/// 줄 하나를 적는다. `seq` 를 함수 인자로 받으면 `&[..]` 가 `'static` 으로 안 늘어나 매크로다.
macro_rules! row {
    ($act:expr, $label:expr, $($k:expr),+) => {
        Bind { seq: &[$($k),+], act: $act, label: $label }
    };
}

/// 찾은 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup<A> {
    Run(A),
    /// 이 열로 시작하는 더 긴 줄이 있다 — 다음 키를 붙여 다시 부른다.
    Pending,
    /// 이 표에서 뜻이 없다.
    Unknown,
}

/// 키의 열로 동작을 찾는다. **더 긴 줄이 먼저다** — 같은 열이 동작이면서 접두어이면 표의
/// 잘못이고 시험(`no_row_is_shadowed_or_a_prefix_of_another`)이 잡는다.
pub fn lookup<A: Copy>(table: &[Bind<A>], seq: &[KeyEvent]) -> Lookup<A> {
    let hits: Vec<&Bind<A>> = table
        .iter()
        .filter(|b| b.seq.len() >= seq.len() && b.seq.iter().zip(seq).all(|(key, k)| key.matches(*k)))
        .collect();
    if hits.iter().any(|b| b.seq.len() > seq.len()) {
        return Lookup::Pending;
    }
    hits.first().map_or(Lookup::Unknown, |b| Lookup::Run(b.act))
}

/// 접두어를 기다리는 동안의 키 열(`gg` 의 첫 `g`). **든 쪽의 상태 안에 둔다** — 탐색이면
/// `App`, 고르기 창이면 [`super::picker::Picker`]. `Mode` 로 두면 탐색이 아닌 모드는 전부 글칸
/// 취급이라 기다리는 `g` 뒤의 `g` 가 글자로 샌다.
///
/// **시계가 없다.** vim 은 `timeoutlen` 으로 풀지만, 시계를 넣으면 조각이 시간을 알아야 하고
/// (순수성 가드) 시험이 기다려야 한다. 뜻 없는 다음 키가 열을 버린다 — 그 키도 버린다. `g`
/// 다음의 `j` 를 `j` 로 살리면 "모르는 키 무시" 가 열 하나에서만 달라진다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chord {
    held: Vec<KeyEvent>,
}

impl Chord {
    /// 키 하나를 붙여 찾는다. 동작이면 열을 비우고 낸다. 더 기다리면 열을 들고 `None`.
    /// 뜻이 없으면 **열을 버리고** `None` — 다음 키는 처음부터 다시 찾는다.
    pub fn feed<A: Copy>(&mut self, table: &[Bind<A>], k: KeyEvent) -> Option<A> {
        self.held.push(k);
        match lookup(table, &self.held) {
            Lookup::Run(act) => {
                self.held.clear();
                Some(act)
            }
            Lookup::Pending => None,
            Lookup::Unknown => {
                self.held.clear();
                None
            }
        }
    }

    /// 기다리는 중인가.
    #[cfg(test)]
    pub fn waiting(&self) -> bool {
        !self.held.is_empty()
    }

    /// 기다리는 열. **SPC 로 시작하면 메뉴가 열린 것이고, 열이 곧 메뉴의 층이다**
    /// ([`super::menu`]).
    pub fn held(&self) -> &[KeyEvent] {
        &self.held
    }

    /// 표를 찾지 않고 키 하나를 붙인다 — 메뉴가 켜진 하위 접두어로 한 층 내려갈 때.
    pub fn push(&mut self, k: KeyEvent) {
        self.held.push(k);
    }

    /// 마지막 키를 뗀다 — 메뉴의 Bksp(한 층 위). 뿌리에서 떼면 열이 비어 메뉴가 닫힌다.
    pub fn pop(&mut self) {
        self.held.pop();
    }

    /// 기다리던 열을 버린다. **키가 아닌 길로 자리가 바뀌면 부른다** — 붙여넣기, 글칸으로
    /// 넘어간 모드. 남겨 두면 한참 뒤의 `g` 가 맨 위로 뛴다.
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

/// 접두어를 누르고 기다리는 동안 **이어 누를 수 있는 키** — `(다음 키 이름, 동작)`, 표의 차례대로.
/// 이름 붙은 줄만 댄다(숨은 별칭은 바에도 안 적는다). 키 바의 `g → g 맨 위` 가 이것을 읽는다 —
/// 기다리는 `g` 가 화면에 안 보이면 다음 키가 왜 안 먹는지 모른다(moai-k3yi).
pub fn next_keys<A: Copy + PartialEq>(table: &[Bind<A>], held: &[KeyEvent]) -> Vec<(String, A)> {
    let mut out: Vec<(String, A)> = Vec::new();
    for b in table.iter().filter(|b| {
        b.label.is_some() && b.seq.len() == held.len() + 1 && b.seq.iter().zip(held).all(|(key, k)| key.matches(*k))
    }) {
        let name = b.seq[held.len()].name();
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, b.act));
        }
    }
    out
}

/// 이동 하나의 낱말. 바는 이동을 `j·k 이동` 으로 묶어 대지만, 기다리는 `g` 뒤에 무엇이 오는지는
/// 한 이동씩 대야 한다(`g 맨 위`).
pub fn move_word(m: Move) -> &'static str {
    match m {
        Move::LineUp => "위",
        Move::LineDown => "아래",
        Move::HalfUp => "반 쪽 위",
        Move::HalfDown => "반 쪽 아래",
        Move::PageUp => "한 쪽 위",
        Move::PageDown => "한 쪽 아래",
        Move::Top => "맨 위",
        Move::Bottom => "맨 아래",
    }
}

/// 동작에 붙은 키 이름. 여럿이면 `·` 로 잇는다(`j·k`).
pub fn label<A: Copy + PartialEq>(table: &[Bind<A>], act: A) -> String {
    labels(table, &[act])
}

/// 동작 여럿의 키 이름을 `·` 로 잇는다(`j·k`).
pub fn labels<A: Copy + PartialEq>(table: &[Bind<A>], acts: &[A]) -> String {
    let names: Vec<&str> =
        acts.iter().flat_map(|act| table.iter().filter(move |b| b.act == *act).filter_map(|b| b.label)).collect();
    names.join("·")
}

/// 어느 모드에서든 — 글을 받는 중에도 — 먼저 받는 키.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anywhere {
    /// raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. 안 받으면 멈춘 화면에서 나갈 길이 없다.
    Quit,
}

pub const ANYWHERE: &[Bind<Anywhere>] = &[row!(Anywhere::Quit, Some("Ctrl-C"), Key::ctrl('c'))];

/// 탐색(목록·상세)과 프로젝트 층의 동작.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browse {
    Quit,
    FocusNext,
    FocusPrev,
    /// 이동 하나를 **포커스 칸에** 준다 — 목록이면 커서, 상세면 굴리기.
    Step(Move),
    Enter,
    Leave,
    Grep,
    Filter,
    Jot,
    Pick,
    Unregister,
    ClearFilter,
    Reload,
    Worktree,
    Raw,
    /// 설정의 n 번째 칸(0부터)을 보이고 숨긴다(moai-fmv5). **칸 이름이 설정에서 오므로 글자가
    /// 아니라 번호로 누른다** — 글자로 두면 설정에 따라 키가 겹친다.
    Column(u8),
    /// done 을 보이고 숨긴다 — 가장 자주 누를 것이라 번호와 따로 선다.
    Done,
    /// 미룬 것을 보이고 숨긴다. 칸이 아니라 `deferred_at` 축이다.
    Deferred,
    ShowAll,
    /// 목록 차례를 고른다(moai-55cp). 이미 고른 것을 다시 누르면 거꾸로 선다.
    Sort(Order),
    /// 목록 줄의 열 하나를 켜고 끈다(moai-g7p8).
    Cell(super::view::Field),
}

/// 목록 차례. **조각이라 `query::SortKey` 를 모른다** — `App` 이 둘을 잇는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    Priority,
    Created,
    Updated,
    Column,
    Assignee,
    Title,
}

impl Order {
    pub fn word(self) -> &'static str {
        match self {
            Order::Priority => "우선순위",
            Order::Created => "생성",
            Order::Updated => "수정",
            Order::Column => "칸",
            Order::Assignee => "담당",
            Order::Title => "제목",
        }
    }
}

/// 칸 토글에 번호를 줄 수 있는 칸 수 — `1`~`9`. 넘는 칸은 번호가 없고 `SPC s a` 로만 돌아온다.
pub const NUMBERED: usize = 9;

/// 탐색의 이동(moai-ob4c, 키 지도 moai-hudg). **vi 키에 이름을 붙이고 화살표·Home·End·PgUp/Dn
/// 은 숨은 별칭이다** — 바가 한 이름만 대야 좁은 창에서 덜 떨어진다. 드나들기만 거꾸로다:
/// `Enter`·`Bksp` 가 이름이고 `l`·`h` 가 별칭이다. 층의 거절문이 "Enter 로 들어가서" 를
/// 대는데, 둘을 다 이름으로 두면 문구가 `Enter·l 로` 가 되고 80칸 바가 네 칸을 더 먹는다.
macro_rules! moves {
    ($step:path, $plain:path) => {
        [
            row!($step(Move::LineDown), Some("j"), Key::plain('j')),
            row!($step(Move::LineUp), Some("k"), Key::plain('k')),
            row!($step(Move::Top), Some("gg"), Key::plain('g'), Key::plain('g')),
            row!($step(Move::Bottom), Some("G"), Key::plain('G')),
            row!($step(Move::HalfDown), Some("Ctrl-d"), Key::chord('d')),
            row!($step(Move::HalfUp), Some("Ctrl-u"), Key::chord('u')),
            row!($step(Move::PageDown), Some("Ctrl-f"), Key::chord('f')),
            row!($step(Move::PageUp), Some("Ctrl-b"), Key::chord('b')),
            row!($step(Move::LineDown), None, $plain(KeyCode::Down)),
            row!($step(Move::LineUp), None, $plain(KeyCode::Up)),
            row!($step(Move::Top), None, $plain(KeyCode::Home)),
            row!($step(Move::Bottom), None, $plain(KeyCode::End)),
            row!($step(Move::PageDown), None, $plain(KeyCode::PageDown)),
            row!($step(Move::PageUp), None, $plain(KeyCode::PageUp)),
        ]
    };
}

/// SPC 메뉴를 여는 키(moai-7sjm).
pub const LEADER: Key = Key::plain(' ');

/// **바로 누르는 키는 이동·드나들기·포커스·`/`·Esc 뿐이다**(키 지도 moai-hudg). 그 밖의 동작은
/// SPC 뒤에 선다 — 한 글자 단축키(`q`·`f`·`n`)를 실수로 누를 때마다 앱이 끝나거나 칸이 열리던
/// 것이 까닭이다. 바로 끝내는 길은 [`ANYWHERE`] 의 Ctrl-C 하나다. F키와 숨은 별칭(`r`·`m`·F7·
/// Delete)도 걷었다.
///
/// **SPC 로 시작하는 줄의 차례가 곧 메뉴의 차례다**([`super::menu::entries`]). 메뉴는 목록을
/// 따로 적지 않고 이 줄들을 읽는다 — 메뉴에 선 것과 실제로 도는 것이 갈릴 수 없다.
pub const BROWSE: &[Bind<Browse>] = {
    use Browse::*;
    use KeyCode as C;
    const MOVES: [Bind<Browse>; 14] = moves!(Browse::Step, Key::any);
    &[
        row!(FocusNext, Some("Tab"), Key::unshift(C::Tab)),
        row!(FocusPrev, None, Key::shift(C::Tab)),
        MOVES[0],
        MOVES[1],
        MOVES[2],
        MOVES[3],
        MOVES[4],
        MOVES[5],
        MOVES[6],
        MOVES[7],
        MOVES[8],
        MOVES[9],
        MOVES[10],
        MOVES[11],
        MOVES[12],
        MOVES[13],
        row!(Enter, Some("Enter"), Key::any(C::Enter)),
        row!(Enter, None, Key::any(C::Right)),
        row!(Enter, None, Key::plain('l')),
        row!(Leave, Some("Bksp"), Key::any(C::Backspace)),
        row!(Leave, None, Key::any(C::Left)),
        row!(Leave, None, Key::plain('h')),
        row!(Grep, Some("/"), Key::plain('/')),
        row!(ClearFilter, Some("Esc"), Key::any(C::Esc)),
        // `/` 는 바로 누르는 키이면서 메뉴에도 선다 — 이름은 바로 누르는 쪽 하나만 댄다.
        row!(Grep, None, LEADER, Key::plain('/')),
        row!(Filter, Some("SPC f"), LEADER, Key::plain('f')),
        row!(Jot, Some("SPC n"), LEADER, Key::plain('n')),
        row!(Reload, Some("SPC r"), LEADER, Key::plain('r')),
        row!(Quit, Some("SPC q"), LEADER, Key::plain('q')),
        row!(Pick, Some("SPC p a"), LEADER, Key::plain('p'), Key::plain('a')),
        row!(Unregister, Some("SPC p d"), LEADER, Key::plain('p'), Key::plain('d')),
        row!(Worktree, Some("SPC t w"), LEADER, Key::plain('t'), Key::plain('w')),
        row!(Raw, Some("SPC t r"), LEADER, Key::plain('t'), Key::plain('r')),
        row!(Done, Some("SPC s d"), LEADER, Key::plain('s'), Key::plain('d')),
        row!(Deferred, Some("SPC s z"), LEADER, Key::plain('s'), Key::plain('z')),
        row!(ShowAll, Some("SPC s a"), LEADER, Key::plain('s'), Key::plain('a')),
        // 번호 줄은 첫 줄만 이름을 단다 — 도움말이 `SPC s 1` 과 "번호가 차례로 는다" 로 한 번에
        // 대고, 메뉴는 이름이 아니라 키(`next.name()`)와 설정의 칸 이름을 세운다.
        row!(Column(0), Some("SPC s 1"), LEADER, Key::plain('s'), Key::plain('1')),
        row!(Column(1), None, LEADER, Key::plain('s'), Key::plain('2')),
        row!(Column(2), None, LEADER, Key::plain('s'), Key::plain('3')),
        row!(Column(3), None, LEADER, Key::plain('s'), Key::plain('4')),
        row!(Column(4), None, LEADER, Key::plain('s'), Key::plain('5')),
        row!(Column(5), None, LEADER, Key::plain('s'), Key::plain('6')),
        row!(Column(6), None, LEADER, Key::plain('s'), Key::plain('7')),
        row!(Column(7), None, LEADER, Key::plain('s'), Key::plain('8')),
        row!(Column(8), None, LEADER, Key::plain('s'), Key::plain('9')),
        row!(Sort(Order::Priority), Some("SPC o p"), LEADER, Key::plain('o'), Key::plain('p')),
        row!(Sort(Order::Created), Some("SPC o c"), LEADER, Key::plain('o'), Key::plain('c')),
        row!(Sort(Order::Updated), Some("SPC o u"), LEADER, Key::plain('o'), Key::plain('u')),
        row!(Sort(Order::Column), Some("SPC o s"), LEADER, Key::plain('o'), Key::plain('s')),
        row!(Sort(Order::Assignee), Some("SPC o a"), LEADER, Key::plain('o'), Key::plain('a')),
        row!(Sort(Order::Title), Some("SPC o t"), LEADER, Key::plain('o'), Key::plain('t')),
        row!(Cell(super::view::Field::Id), Some("SPC c i"), LEADER, Key::plain('c'), Key::plain('i')),
        row!(Cell(super::view::Field::Priority), Some("SPC c p"), LEADER, Key::plain('c'), Key::plain('p')),
        row!(Cell(super::view::Field::Assignee), Some("SPC c a"), LEADER, Key::plain('c'), Key::plain('a')),
        row!(Cell(super::view::Field::Created), Some("SPC c c"), LEADER, Key::plain('c'), Key::plain('c')),
        row!(Cell(super::view::Field::Updated), Some("SPC c u"), LEADER, Key::plain('c'), Key::plain('u')),
        row!(Cell(super::view::Field::Tally), Some("SPC c n"), LEADER, Key::plain('c'), Key::plain('n')),
        row!(Cell(super::view::Field::Tags), Some("SPC c g"), LEADER, Key::plain('c'), Key::plain('g')),
    ]
};

/// 켜짐을 가르는 값. **든 쪽이 잰다** — 여기는 `App` 을 모른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ctx {
    /// 프로젝트 층에 섰나.
    pub layer: bool,
    /// 포커스가 목록에 있나.
    pub list_focus: bool,
    /// 커서가 선 줄이 **들어갈 데가 없다** — 잎(일 한 줄)이거나 줄이 없다. `..`·디렉터리·층의
    /// 프로젝트는 들어간다(`..` 은 나간다).
    pub leaf: bool,
    /// **나갈 데가 없다** — 프로젝트 뿌리인데 층이 없거나, 층에 섰다. 층이 있는 프로젝트 뿌리는
    /// 뿌리가 아니다: Bksp 가 층으로 올라간다(`App::leave` → `climb`).
    pub root: bool,
    pub worktree: bool,
    pub raw: bool,
    /// 번호를 받은 칸 수 — 설정의 칸 수와 [`NUMBERED`] 중 작은 것.
    pub columns: usize,
    /// 숨긴 칸 — n 번째 비트가 설정의 n 번째 칸. 이름을 들지 않는다: 이 값은 복사로 다닌다.
    pub hidden: u16,
    pub done_hidden: bool,
    pub deferred_hidden: bool,
    /// 고른 차례와 거꾸로인가.
    pub order: Order,
    pub order_reversed: bool,
    /// 켜 둔 목록 열.
    pub fields: super::view::Fields,
    /// `Tab`·Shift-Tab 이 가는 칸의 이름.
    pub next_pane: &'static str,
    pub prev_pane: &'static str,
}

/// 켜지지 않은 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Off {
    /// 아무 일도 없다. 드나드는 키가 상세에서 그렇다 — 오늘 그 자리에서 말이 없다.
    Quiet,
    /// 왜 안 되는지 한 줄. **조용히 먹으면 고장 난 것으로 보인다.**
    Why(String),
}

impl Browse {
    /// 이 자리에서 되는가. **키 처리·키 바·SPC 메뉴가 같은 판정을 읽는다** — 키 처리는
    /// 까닭을 알림으로 대고, 바와 메뉴는 켜진 것만 세운다. 메뉴에 안 선 키는 메뉴에서 눌러도
    /// 모르는 키와 같다([`super::menu::feed`]).
    ///
    /// - 드나드는 키는 **목록 포커스를 탄다.** 상세를 읽다가 누른 Enter·←가 목록을 옮기면
    ///   보던 이슈가 바뀐다
    /// - 해제는 층의 줄에서만, 목록 포커스로 — 지금 선 프로젝트를 빼는 일이 안 생긴다
    /// - 층에서 프로젝트 안의 줄에 매인 키(거름망·검색·워크트리)는 켜지지 않는다. 바로 누르는
    ///   `/` 는 까닭을 댄다. 메뉴의 `SPC f`·`SPC t w` 는 층에서 메뉴에 안 서므로 까닭을 댈
    ///   자리가 없다 — 워크트리는 말없이 꺼 두고, 거름망은 검색과 한 문장을 쓴다. `n` 은 층에서도
    ///   듣는다 — 커서의 프로젝트에 담는다(moai-fccv)
    pub fn enabled(self, c: &Ctx) -> Result<(), Off> {
        use Browse::*;
        match self {
            Enter | Leave if !c.list_focus => Err(Off::Quiet),
            // 커서에서 되는 키만(moai-k3yi): 잎의 Enter·뿌리의 Bksp 는 아무 일도 없다. 까닭을 대지
            // 않는다 — 들어갈 데 없는 줄에서 Enter 가 조용한 것은 파일 관리자와 같고, 바가 그 키를
            // 안 적으므로 "적힌 키가 안 듣는다" 가 안 생긴다.
            Enter if c.leaf => Err(Off::Quiet),
            Leave if c.root => Err(Off::Quiet),
            Unregister if !(c.layer && c.list_focus) => Err(Off::Quiet),
            Grep | Filter if c.layer => {
                Err(Off::Why(format!("거름망은 프로젝트 안의 줄에 건다 — {} 로 들어가서 건다", label(BROWSE, Enter))))
            }
            Worktree if c.layer => Err(Off::Quiet),
            // 보기는 프로젝트 안의 줄에 건다 — 층에서는 그룹째 메뉴에 안 선다(`menu::live`).
            Column(_) | Done | Deferred | ShowAll | Sort(_) | Cell(_) if c.layer => Err(Off::Quiet),
            Column(n) if usize::from(n) >= c.columns => Err(Off::Quiet),
            _ => Ok(()),
        }
    }

    /// SPC 메뉴의 낱말. 바([`Browse::what`])보다 자리가 넉넉해 뜻을 풀어 쓴다 — 켜고 끄는 것은
    /// 무엇을 켜고 끄는지만 대고, 지금 상태는 [`Browse::state`] 가 따로 낸다.
    pub fn menu_word(self, c: &Ctx) -> &'static str {
        use Browse::*;
        match self {
            Jot => "생각 담기",
            Reload => "다시 읽기",
            Pick => "등록",
            Unregister => "목록에서 빼기",
            Worktree => "워크트리 겹쳐 보기",
            Raw => "원문↔그리기",
            Deferred => "미룸",
            ShowAll => "모두 보이기",
            Sort(o) => o.word(),
            Cell(f) => f.word(),
            _ => self.what(c),
        }
    }

    /// 켜고 끄는 것의 **지금 상태** 낱말. 색이 혼자 뜻을 지지 않는다 — 켜진 것을 색으로만
    /// 칠하면 색 없는 터미널에서 어느 쪽인지 모른다.
    pub fn state(self, c: &Ctx) -> Option<&'static str> {
        match self {
            Browse::Worktree => Some(if c.worktree { "[켜짐]" } else { "[꺼짐]" }),
            Browse::Raw => Some(if c.raw { "[원문]" } else { "[그리기]" }),
            Browse::Column(n) => Some(shown(c.hidden & (1 << n) != 0)),
            Browse::Done => Some(shown(c.done_hidden)),
            Browse::Deferred => Some(shown(c.deferred_hidden)),
            // 고른 차례에만 붙는다 — 방향은 낱말로 댄다.
            Browse::Sort(o) if o == c.order => Some(if c.order_reversed { "[● 거꾸로]" } else { "[● 차례]" }),
            Browse::Cell(f) => Some(shown(!c.fields.shows(f))),
            _ => None,
        }
    }

    /// 사람에게 대는 낱말. **켜고 끄는 것은 지금 상태로 가는 곳을 댄다** — 색이 혼자 뜻을
    /// 지지 않는다.
    pub fn what(self, c: &Ctx) -> &'static str {
        use Browse::*;
        match self {
            Quit => "끝내기",
            FocusNext => c.next_pane,
            FocusPrev => c.prev_pane,
            // 상세에서는 커서가 없다 — 굴린다.
            Step(_) if !c.list_focus => "굴리기",
            Step(_) => "이동",
            Enter => "들어가기",
            Leave => "나가기",
            Grep => "검색",
            Filter => "거름망",
            Jot => "담기",
            Pick if c.layer => "등록",
            Pick => "프로젝트 등록",
            Unregister => "해제",
            ClearFilter => "풀기",
            Reload => "갱신",
            Worktree if c.worktree => "워크트리 끄기",
            Worktree => "워크트리",
            Raw if c.raw => "그리기",
            Raw => "원문",
            // 칸의 이름은 설정에서 온다 — 메뉴가 이름을 붙인다(`menu::entries`).
            Column(_) => "칸",
            Done => "done",
            Deferred => "미룸",
            ShowAll => "모두",
            Sort(_) => "정렬",
            Cell(_) => "열",
        }
    }
}

fn shown(hidden: bool) -> &'static str {
    if hidden { "[숨김]" } else { "[보임]" }
}

/// SPC 메뉴가 열린 동안 **표보다 먼저** 받는 키(moai-7sjm). Esc 는 탐색에서 거름망을 풀지만,
/// 메뉴가 열렸으면 메뉴를 닫는 것이 먼저다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Close,
    /// 한 층 위. 뿌리에서는 닫는다.
    Up,
}

pub const MENU: &[Bind<Menu>] = &[
    row!(Menu::Close, Some("Esc"), Key::any(KeyCode::Esc)),
    row!(Menu::Up, Some("Bksp"), Key::any(KeyCode::Backspace)),
];

impl Menu {
    pub fn what(self) -> &'static str {
        match self {
            Menu::Close => "닫기",
            Menu::Up => "위로",
        }
    }
}

/// 검색·거름망·누군지 묻는 칸 — 칸이 안 먹은 키.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    Apply,
    Cancel,
}

pub const PROMPT: &[Bind<Prompt>] = &[
    row!(Prompt::Apply, Some("Enter"), Key::unctrl(KeyCode::Enter)),
    row!(Prompt::Cancel, Some("Esc"), Key::unctrl(KeyCode::Esc)),
];

/// 고르기 창(moai-plvy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// 커서 이동 — 탐색의 목록과 같은 키([`moves!`]).
    Step(Move),
    Enter,
    Up,
    Register,
    Hidden,
    Path,
    Close,
}

/// 창은 **Ctrl·Alt 붙은 키를 거른다** — 줄이 모두 `bare` 다. 넷만 다르다: vi 의 쪽 이동
/// (Ctrl-d·Ctrl-u·Ctrl-f·Ctrl-b)은 탐색과 같은 [`Key::chord`] 다.
///
/// **경로 적기는 `g p` 다**(옛 `g`). `g` 가 `gg`(맨 위)의 접두어가 되어 한 키 동작으로 둘 수
/// 없다 — 같은 열이 동작이면서 접두어이면 표의 잘못이다. `g` 뒤에 "가는 곳" 을 잇는 모양은
/// helix 의 goto·yazi 의 `g` 계열과 같다(moai-gaum).
pub const PICK: &[Bind<Pick>] = {
    use KeyCode as C;
    use Pick::*;
    const MOVES: [Bind<Pick>; 14] = moves!(Pick::Step, Key::bare);
    &[
        MOVES[0],
        MOVES[1],
        MOVES[2],
        MOVES[3],
        MOVES[4],
        MOVES[5],
        MOVES[6],
        MOVES[7],
        MOVES[8],
        MOVES[9],
        MOVES[10],
        MOVES[11],
        MOVES[12],
        MOVES[13],
        row!(Enter, Some("Enter"), Key::bare(C::Enter)),
        row!(Enter, None, Key::bare(C::Right)),
        row!(Enter, None, Key::plain('l')),
        row!(Up, Some("Bksp"), Key::bare(C::Backspace)),
        row!(Up, None, Key::bare(C::Left)),
        row!(Up, None, Key::plain('h')),
        row!(Register, Some("a"), Key::plain('a')),
        row!(Hidden, Some("."), Key::plain('.')),
        row!(Path, Some("g p"), Key::plain('g'), Key::plain('p')),
        row!(Close, Some("Esc"), Key::bare(C::Esc)),
        row!(Close, None, Key::plain('q')),
    ]
};

impl Pick {
    pub fn what(self, show_hidden: bool) -> &'static str {
        use Pick::*;
        match self {
            Step(_) => "이동",
            Enter => "들어가기",
            Up => "위로",
            Register => "등록",
            Hidden if show_hidden => "숨은 것 감추기",
            Hidden => "숨은 것",
            Path => "경로 적기",
            Close => "닫기",
        }
    }
}

/// 고르기 창의 경로 칸 — 칸이 안 먹은 키.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goto {
    Go,
    Cancel,
}

pub const PATH: &[Bind<Goto>] = &[
    row!(Goto::Go, Some("Enter"), Key::bare(KeyCode::Enter)),
    row!(Goto::Cancel, Some("Esc"), Key::bare(KeyCode::Esc)),
];

/// 생각 담기 폼(moai-11s4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jot {
    Save,
    /// 제목 ↔ 본문.
    Switch,
    Close,
    /// 제목 칸의 Enter — 본문으로. **칸이 먼저 받는다**: 본문은 Enter 로 줄을 나눈다.
    Next,
}

pub const JOT: &[Bind<Jot>] = {
    use Jot::*;
    use KeyCode as C;
    &[
        // F2 는 걷었다(키 지도 moai-hudg) — F키는 전부 걷는다.
        row!(Save, Some("Ctrl-S"), Key::ctrl('s')),
        row!(Save, None, Key::ctrl('S')),
        row!(Switch, Some("Tab"), Key::any(C::Tab)),
        row!(Close, Some("Esc"), Key::any(C::Esc)),
        row!(Next, Some("Enter"), Key::unctrl(C::Enter)),
    ]
};

impl Jot {
    /// `title` — 포커스가 제목 칸에 있나.
    pub fn what(self, title: bool) -> &'static str {
        match self {
            Jot::Save => "담기",
            Jot::Switch if title => "본문",
            Jot::Switch => "제목",
            Jot::Next if title => "본문으로",
            Jot::Next => "줄 나누기",
            Jot::Close => "닫기",
        }
    }
}

/// "버릴까"·"뺄까" 에 답하는 키. **`y` 만 예다** — 다른 키는 그만둔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Yes,
}

pub const CONFIRM: &[Bind<Confirm>] =
    &[row!(Confirm::Yes, Some("y"), Key::plain('y')), row!(Confirm::Yes, None, Key::plain('Y'))];

/// 사람이 적는 키 이름을 키 **열**로 푼다 — `g p` 는 띄어 적은 둘, `gg` 는 같은 글자 둘.
/// 키 이름이 아니면 `None`. 같은 글자 둘만 붙여 읽는다 — `Tab` 같은 이름을 글자 셋으로 읽지
/// 않고, 도움말의 낱말(`to`)이 키 열로 잡히지 않게.
#[cfg(test)]
pub fn parse_seq(name: &str) -> Option<Vec<KeyEvent>> {
    if name.contains(' ') {
        return name.split(' ').map(parse).collect();
    }
    if let Some(k) = parse(name) {
        return Some(vec![k]);
    }
    let cs: Vec<char> = name.chars().collect();
    match cs[..] {
        [a, b] if a == b && a.is_ascii_alphabetic() => Some(vec![KeyEvent::new(KeyCode::Char(a), KeyModifiers::NONE); 2]),
        _ => None,
    }
}

/// 사람이 적는 키 이름을 키로 푼다 — 표의 이름과 도움말의 낱말이 정말 그 키인지 시험이 잰다.
/// 키 이름이 아니면 `None`.
#[cfg(test)]
pub fn parse(name: &str) -> Option<KeyEvent> {
    use KeyModifiers as M;
    let named = |code| Some(KeyEvent::new(code, M::NONE));
    if let Some(rest) = name.strip_prefix("Ctrl-") {
        let mut cs = rest.chars();
        return match (cs.next(), cs.next()) {
            (Some(c), None) => Some(KeyEvent::new(KeyCode::Char(c.to_ascii_lowercase()), M::CONTROL)),
            _ => None,
        };
    }
    match name {
        "Enter" => named(KeyCode::Enter),
        "Esc" => named(KeyCode::Esc),
        "Tab" => named(KeyCode::Tab),
        "Shift-Tab" => Some(KeyEvent::new(KeyCode::Tab, M::SHIFT)),
        "BackTab" => named(KeyCode::BackTab),
        "Bksp" | "Backspace" => named(KeyCode::Backspace),
        "Delete" => named(KeyCode::Delete),
        "Up" => named(KeyCode::Up),
        "Down" => named(KeyCode::Down),
        "Left" => named(KeyCode::Left),
        "Right" => named(KeyCode::Right),
        "Home" => named(KeyCode::Home),
        "End" => named(KeyCode::End),
        "PageUp" | "PgUp" => named(KeyCode::PageUp),
        "PageDown" | "PgDn" => named(KeyCode::PageDown),
        "SPC" => named(KeyCode::Char(' ')),
        _ => {
            if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                return named(KeyCode::F(n));
            }
            let mut cs = name.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) if c.is_ascii_graphic() => named(KeyCode::Char(c)),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    fn one<A: Copy>(table: &[Bind<A>], k: KeyEvent) -> Lookup<A> {
        lookup(table, &[k])
    }

    /// 그 줄의 키를 실제로 누른 모양으로 만든다 — 원하는 수식키만 붙인다.
    fn pressed(key: &Key) -> KeyEvent {
        KeyEvent::new(key.code, key.want)
    }

    /// **대문자는 SHIFT 가 붙어 온다.** `G` 줄은 SHIFT 붙은 `G` 도, 안 붙은 `G` 도 받고 `g` 는
    /// 안 받는다.
    #[test]
    fn a_capital_matches_with_or_without_shift() {
        let big = Key::plain('G');
        assert!(big.matches(with(KeyCode::Char('G'), KeyModifiers::SHIFT)));
        assert!(big.matches(press(KeyCode::Char('G'))));
        assert!(!big.matches(press(KeyCode::Char('g'))));
        assert!(!Key::plain('g').matches(with(KeyCode::Char('G'), KeyModifiers::SHIFT)));
        // 오늘 표에서도 — SHIFT 붙은 `/`·SPC 는 그 키다(자판에 따라 SHIFT 가 붙어 온다).
        assert_eq!(one(BROWSE, with(KeyCode::Char('/'), KeyModifiers::SHIFT)), Lookup::Run(Browse::Grep));
        assert_eq!(one(CONFIRM, with(KeyCode::Char('Y'), KeyModifiers::SHIFT)), Lookup::Run(Confirm::Yes));
    }

    /// **글자 키의 Ctrl·Alt 는 정확히 견준다.** Ctrl-A 가 등록 창을, Ctrl-D 가 "뺄까" 를
    /// 띄우면 터미널에서 손에 익은 키가 엉뚱한 일을 한다.
    #[test]
    fn ctrl_and_alt_on_letters_are_exact() {
        for c in ['a', 'n', 'q', 'w', '/', 'j', 'p', 'g', 'h', 'l'] {
            for m in [KeyModifiers::CONTROL, KeyModifiers::ALT, CTRL_ALT] {
                assert_eq!(one(BROWSE, with(KeyCode::Char(c), m)), Lookup::Unknown, "{m:?}-{c}");
                assert_eq!(one(PICK, with(KeyCode::Char(c), m)), Lookup::Unknown, "창 {m:?}-{c}");
            }
        }
        // vi 의 쪽 이동은 Ctrl 만 — Alt 가 함께 붙으면 아니다(`Key::chord`).
        for (c, m) in [('d', Move::HalfDown), ('u', Move::HalfUp), ('f', Move::PageDown), ('b', Move::PageUp)] {
            assert_eq!(one(BROWSE, with(KeyCode::Char(c), KeyModifiers::CONTROL)), Lookup::Run(Browse::Step(m)), "Ctrl-{c}");
            assert_eq!(one(PICK, with(KeyCode::Char(c), KeyModifiers::CONTROL)), Lookup::Run(Pick::Step(m)), "창 Ctrl-{c}");
            for mods in [KeyModifiers::ALT, CTRL_ALT] {
                assert_eq!(one(BROWSE, with(KeyCode::Char(c), mods)), Lookup::Unknown, "{mods:?}-{c}");
                assert_eq!(one(PICK, with(KeyCode::Char(c), mods)), Lookup::Unknown, "창 {mods:?}-{c}");
            }
        }
        assert_eq!(one(ANYWHERE, with(KeyCode::Char('c'), KeyModifiers::CONTROL)), Lookup::Run(Anywhere::Quit));
        // Ctrl-C 는 Alt 를 안 본다 — 오늘 그렇다.
        assert_eq!(one(ANYWHERE, with(KeyCode::Char('c'), CTRL_ALT)), Lookup::Run(Anywhere::Quit));
        assert_eq!(one(ANYWHERE, press(KeyCode::Char('c'))), Lookup::Unknown);
        assert_eq!(one(ANYWHERE, with(KeyCode::Char('c'), KeyModifiers::ALT)), Lookup::Unknown);
        assert_eq!(one(CONFIRM, with(KeyCode::Char('y'), KeyModifiers::CONTROL)), Lookup::Unknown);
        assert_eq!(one(JOT, with(KeyCode::Char('s'), KeyModifiers::CONTROL)), Lookup::Run(Jot::Save));
        assert_eq!(one(JOT, press(KeyCode::Char('s'))), Lookup::Unknown);
    }

    /// **Shift-Tab 은 두 모양으로 온다** — `BackTab` 과 SHIFT 붙은 `Tab`. 한 줄이 둘 다 받는다.
    #[test]
    fn back_tab_and_shift_tab_are_one_key() {
        assert_eq!(one(BROWSE, press(KeyCode::BackTab)), Lookup::Run(Browse::FocusPrev));
        assert_eq!(one(BROWSE, with(KeyCode::BackTab, KeyModifiers::SHIFT)), Lookup::Run(Browse::FocusPrev));
        assert_eq!(one(BROWSE, with(KeyCode::Tab, KeyModifiers::SHIFT)), Lookup::Run(Browse::FocusPrev));
        assert_eq!(one(BROWSE, press(KeyCode::Tab)), Lookup::Run(Browse::FocusNext));
        assert_eq!(one(JOT, press(KeyCode::BackTab)), Lookup::Run(Jot::Switch));
        assert_eq!(one(JOT, with(KeyCode::Tab, KeyModifiers::SHIFT)), Lookup::Run(Jot::Switch));
    }

    /// **접두어는 기다린다.** 조그만 표로 잰다 — `g g` 는 맨 위, `SPC t w` 는 셋째 키에 선다.
    /// 모르는 둘째 키는 뜻이 없다. 실제 표의 `gg`·`g p` 도 같다.
    #[test]
    fn a_prefix_waits_for_the_rest_of_its_row() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Fx {
            Top,
            Bottom,
            Worktree,
        }
        const FX: &[Bind<Fx>] = &[
            row!(Fx::Top, Some("gg"), Key::plain('g'), Key::plain('g')),
            row!(Fx::Bottom, Some("G"), Key::plain('G')),
            row!(Fx::Worktree, Some("SPC t w"), Key::plain(' '), Key::plain('t'), Key::plain('w')),
        ];
        let g = press(KeyCode::Char('g'));
        let sp = press(KeyCode::Char(' '));
        let t = press(KeyCode::Char('t'));
        assert_eq!(lookup(FX, &[g]), Lookup::Pending);
        assert_eq!(lookup(FX, &[g, g]), Lookup::Run(Fx::Top));
        assert_eq!(lookup(FX, &[g, press(KeyCode::Char('x'))]), Lookup::Unknown);
        assert_eq!(lookup(FX, &[with(KeyCode::Char('G'), KeyModifiers::SHIFT)]), Lookup::Run(Fx::Bottom));
        assert_eq!(lookup(FX, &[sp]), Lookup::Pending);
        assert_eq!(lookup(FX, &[sp, t]), Lookup::Pending);
        assert_eq!(lookup(FX, &[sp, t, press(KeyCode::Char('w'))]), Lookup::Run(Fx::Worktree));
        assert_eq!(lookup(FX, &[press(KeyCode::Char('x'))]), Lookup::Unknown);

        let p = press(KeyCode::Char('p'));
        assert_eq!(lookup(BROWSE, &[g]), Lookup::Pending);
        assert_eq!(lookup(BROWSE, &[g, g]), Lookup::Run(Browse::Step(Move::Top)));
        assert_eq!(lookup(BROWSE, &[g, p]), Lookup::Unknown, "탐색에는 `g p` 가 없다");
        assert_eq!(lookup(PICK, &[g]), Lookup::Pending);
        assert_eq!(lookup(PICK, &[g, g]), Lookup::Run(Pick::Step(Move::Top)));
        assert_eq!(lookup(PICK, &[g, p]), Lookup::Run(Pick::Path));
        assert_eq!(lookup(PICK, &[g, press(KeyCode::Char('j'))]), Lookup::Unknown);
    }

    /// **기다리는 열은 [`Chord`] 가 든다.** 동작이 나오면 비고, 뜻 없는 다음 키는 열과 함께
    /// 버려진다 — 버린 뒤의 `g` 하나는 옛 `g` 와 이어지지 않는다.
    #[test]
    fn a_chord_holds_a_prefix_and_drops_it_on_an_unknown_key() {
        let g = press(KeyCode::Char('g'));
        let mut c = Chord::default();
        assert_eq!(c.feed(BROWSE, g), None);
        assert!(c.waiting());
        assert_eq!(c.feed(BROWSE, g), Some(Browse::Step(Move::Top)));
        assert!(!c.waiting());

        for k in [press(KeyCode::Char('x')), press(KeyCode::Char('j')), press(KeyCode::Esc), press(KeyCode::Char(' '))] {
            assert_eq!(c.feed(BROWSE, g), None);
            assert_eq!(c.feed(BROWSE, k), None, "`g` 뒤의 {k:?} 가 제 뜻을 했다");
            assert!(!c.waiting(), "`g` 뒤의 {k:?} 가 열을 안 버렸다");
        }
        assert_eq!(c.feed(BROWSE, g), None, "버린 열의 `g` 와 이어졌다");
        c.clear();
        assert!(!c.waiting());
        // 한 키짜리는 기다리지 않는다
        assert_eq!(c.feed(BROWSE, press(KeyCode::Char('j'))), Some(Browse::Step(Move::LineDown)));
        assert_eq!(c.feed(PICK, g), None);
        assert_eq!(c.feed(PICK, press(KeyCode::Char('p'))), Some(Pick::Path));
    }

    /// **vi 이동**(moai-ob4c, 키 지도 moai-hudg) — 탐색과 고르기 창이 같은 키로 같은 이동을
    /// 낸다. 대문자 `G` 는 SHIFT 가 붙어 와도 맨 아래다. `h`·`l` 은 나가기·들어가기.
    #[test]
    fn vi_movement_is_the_same_in_browse_and_the_picker() {
        use KeyCode as C;
        let ctrl = |c| with(C::Char(c), KeyModifiers::CONTROL);
        let g = press(C::Char('g'));
        let cases: Vec<(Vec<KeyEvent>, Move)> = vec![
            (vec![press(C::Char('j'))], Move::LineDown),
            (vec![press(C::Down)], Move::LineDown),
            (vec![press(C::Char('k'))], Move::LineUp),
            (vec![press(C::Up)], Move::LineUp),
            (vec![g, g], Move::Top),
            (vec![press(C::Home)], Move::Top),
            (vec![with(C::Char('G'), KeyModifiers::SHIFT)], Move::Bottom),
            (vec![press(C::Char('G'))], Move::Bottom),
            (vec![press(C::End)], Move::Bottom),
            (vec![ctrl('d')], Move::HalfDown),
            (vec![ctrl('u')], Move::HalfUp),
            (vec![ctrl('f')], Move::PageDown),
            (vec![press(C::PageDown)], Move::PageDown),
            (vec![ctrl('b')], Move::PageUp),
            (vec![press(C::PageUp)], Move::PageUp),
        ];
        for (seq, m) in cases {
            assert_eq!(lookup(BROWSE, &seq), Lookup::Run(Browse::Step(m)), "탐색 {seq:?}");
            assert_eq!(lookup(PICK, &seq), Lookup::Run(Pick::Step(m)), "창 {seq:?}");
        }
        assert_eq!(one(BROWSE, press(C::Char('h'))), Lookup::Run(Browse::Leave));
        assert_eq!(one(BROWSE, press(C::Char('l'))), Lookup::Run(Browse::Enter));
        assert_eq!(one(PICK, press(C::Char('h'))), Lookup::Run(Pick::Up));
        assert_eq!(one(PICK, press(C::Char('l'))), Lookup::Run(Pick::Enter));
        // 소문자 `g` 하나는 맨 아래가 아니다 — SHIFT 를 떼고 견주어도 글자는 다르다.
        assert_eq!(one(BROWSE, with(C::Char('g'), KeyModifiers::SHIFT)), Lookup::Pending);
    }

    /// **어느 줄도 가려지지 않고, 어느 줄도 다른 줄의 접두어가 아니다.** 줄마다 그 키를 눌러
    /// 그 줄의 동작이 나오는지 본다 — 앞줄이 먹어 버리거나 더 긴 줄이 기다리게 하면 여기서 갈린다.
    #[test]
    fn no_row_is_shadowed_or_a_prefix_of_another() {
        fn each<A: Copy + PartialEq + std::fmt::Debug>(name: &str, table: &[Bind<A>]) {
            for b in table {
                let seq: Vec<KeyEvent> = b.seq.iter().map(pressed).collect();
                assert_eq!(lookup(table, &seq), Lookup::Run(b.act), "{name}: {:?}", b.seq);
            }
        }
        each("anywhere", ANYWHERE);
        each("browse", BROWSE);
        each("prompt", PROMPT);
        each("pick", PICK);
        each("path", PATH);
        each("jot", JOT);
        each("confirm", CONFIRM);
        each("menu", MENU);
    }

    /// **적힌 키 이름은 그 키다.** 이름을 키로 풀어 표에서 찾으면 그 줄의 동작이 나온다 — 바에
    /// `F5 갱신` 이라 적어 두고 표에서는 F5 를 걷는 일이 여기서 갈린다.
    #[test]
    fn every_label_is_the_key_it_names() {
        fn each<A: Copy + PartialEq + std::fmt::Debug>(name: &str, table: &[Bind<A>]) {
            for b in table.iter().filter(|b| b.label.is_some()) {
                let label = b.label.unwrap();
                let seq = parse_seq(label).unwrap_or_else(|| panic!("{name}: `{label}` 를 키로 못 푼다"));
                assert_eq!(lookup(table, &seq), Lookup::Run(b.act), "{name}: `{label}`");
            }
        }
        each("anywhere", ANYWHERE);
        each("browse", BROWSE);
        each("prompt", PROMPT);
        each("pick", PICK);
        each("path", PATH);
        each("jot", JOT);
        each("confirm", CONFIRM);
        each("menu", MENU);
        assert_eq!(label(JOT, Jot::Save), "Ctrl-S");
        assert_eq!(labels(BROWSE, &[Browse::Step(Move::LineDown), Browse::Step(Move::LineUp)]), "j·k");
        assert_eq!(label(BROWSE, Browse::Step(Move::Top)), "gg", "숨은 별칭 Home 이 이름에 섰다");
        assert_eq!(label(BROWSE, Browse::Enter), "Enter", "숨은 별칭 `l` 이 이름에 섰다 — 층의 거절문이 `Enter·l 로` 가 된다");
        assert_eq!(label(PICK, Pick::Path), "g p");
        assert_eq!(label(BROWSE, Browse::Quit), "SPC q");
        assert_eq!(label(BROWSE, Browse::Grep), "/", "숨은 별칭 `SPC /` 가 이름에 섰다 — 층의 거절문·붙여넣기 안내가 둘을 댄다");
    }

    /// **옮기기 전 코드가 받던 키가 같은 동작이 된다**(moai-gaum 의 키 목록). 수식키가 붙은 모양도
    /// 옛 코드가 받던 그대로다 — 비글자 키는 Ctrl·Alt 를 안 봤고, 창은 통째로 걸렀다.
    ///
    /// **일부러 바꾼 뜻**(moai-ob4c, 키 지도 moai-hudg): `j`·`k` 는 상세 굴리기에서 포커스 칸
    /// 이동으로, `b`(상세 한 쪽)는 뜻이 없어졌고, 창의 `g` 는 경로 적기에서 `gg`·`g p` 의 접두어로
    /// 갔다. **moai-7sjm 이 한 글자 단축키와 F키를 SPC 메뉴 뒤로 옮겼다** — 옛 키는 이제 뜻이 없고,
    /// 같은 동작이 SPC 열에서 나온다.
    #[test]
    fn the_old_inventory_maps_to_the_same_actions() {
        use Browse as B;
        use KeyCode as C;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let browse = [
            (press(C::Tab), Lookup::Run(B::FocusNext)),
            (with(C::Tab, ctrl), Lookup::Run(B::FocusNext)),
            (press(C::BackTab), Lookup::Run(B::FocusPrev)),
            (with(C::Tab, KeyModifiers::SHIFT), Lookup::Run(B::FocusPrev)),
            (press(C::Up), Lookup::Run(B::Step(Move::LineUp))),
            (with(C::Down, ctrl), Lookup::Run(B::Step(Move::LineDown))),
            (press(C::Home), Lookup::Run(B::Step(Move::Top))),
            (press(C::End), Lookup::Run(B::Step(Move::Bottom))),
            (press(C::PageUp), Lookup::Run(B::Step(Move::PageUp))),
            (with(C::PageDown, KeyModifiers::SHIFT), Lookup::Run(B::Step(Move::PageDown))),
            (press(C::Enter), Lookup::Run(B::Enter)),
            (press(C::Right), Lookup::Run(B::Enter)),
            (press(C::Backspace), Lookup::Run(B::Leave)),
            (press(C::Left), Lookup::Run(B::Leave)),
            (press(C::Char('/')), Lookup::Run(B::Grep)),
            (press(C::Esc), Lookup::Run(B::ClearFilter)),
            // 옮겼다(moai-7sjm) — 바로 누르던 키는 뜻이 없다. **`q` 는 더는 안 끝낸다.**
            (press(C::Char('q')), Lookup::Unknown),
            (press(C::F(10)), Lookup::Unknown),
            (with(C::F(10), ctrl), Lookup::Unknown),
            (press(C::Char('f')), Lookup::Unknown),
            (press(C::F(7)), Lookup::Unknown),
            (press(C::Char('n')), Lookup::Unknown),
            (press(C::Char('a')), Lookup::Unknown),
            (press(C::Char('d')), Lookup::Unknown),
            (press(C::Delete), Lookup::Unknown),
            (press(C::F(5)), Lookup::Unknown),
            (press(C::Char('r')), Lookup::Unknown),
            (press(C::Char('w')), Lookup::Unknown),
            (press(C::F(3)), Lookup::Unknown),
            (press(C::Char('m')), Lookup::Unknown),
            (press(C::Char(' ')), Lookup::Pending),
            // 진행 바탕 토글 `p` 는 main 이 기능째 걷었다(moai-u3r2).
            (press(C::Char('p')), Lookup::Unknown),
            // 뜻을 바꿨다(moai-ob4c) — 포커스 칸 이동. 옛 상세 한 쪽(SPC·`b`)은 걷었다.
            (press(C::Char('j')), Lookup::Run(B::Step(Move::LineDown))),
            (press(C::Char('k')), Lookup::Run(B::Step(Move::LineUp))),
            (press(C::Char('b')), Lookup::Unknown),
            (with(C::Char('a'), ctrl), Lookup::Unknown),
            (with(C::Char('d'), alt), Lookup::Unknown),
            (press(C::Char('x')), Lookup::Unknown),
            (press(C::F(1)), Lookup::Unknown),
        ];
        for (k, want) in browse {
            assert_eq!(one(BROWSE, k), want, "탐색 {k:?}");
        }
        let sp = press(C::Char(' '));
        let ch = |c| press(C::Char(c));
        let menu = [
            (vec![sp, ch('q')], B::Quit),
            (vec![sp, ch('/')], B::Grep),
            (vec![sp, ch('f')], B::Filter),
            (vec![sp, ch('n')], B::Jot),
            (vec![sp, ch('r')], B::Reload),
            (vec![sp, ch('p'), ch('a')], B::Pick),
            (vec![sp, ch('p'), ch('d')], B::Unregister),
            (vec![sp, ch('t'), ch('w')], B::Worktree),
            (vec![sp, ch('t'), ch('r')], B::Raw),
            (vec![sp, ch('s'), ch('d')], B::Done),
            (vec![sp, ch('s'), ch('z')], B::Deferred),
            (vec![sp, ch('s'), ch('a')], B::ShowAll),
            (vec![sp, ch('s'), ch('1')], B::Column(0)),
            (vec![sp, ch('s'), ch('9')], B::Column(8)),
            (vec![sp, ch('o'), ch('p')], B::Sort(Order::Priority)),
            (vec![sp, ch('o'), ch('u')], B::Sort(Order::Updated)),
            (vec![sp, ch('o'), ch('t')], B::Sort(Order::Title)),
            (vec![sp, ch('c'), ch('a')], B::Cell(super::super::view::Field::Assignee)),
            (vec![sp, ch('c'), ch('g')], B::Cell(super::super::view::Field::Tags)),
        ];
        for (seq, act) in menu {
            assert_eq!(lookup(BROWSE, &seq), Lookup::Run(act), "메뉴 {seq:?}");
        }
        let pick = [
            (press(C::Up), Lookup::Run(Pick::Step(Move::LineUp))),
            (press(C::PageDown), Lookup::Run(Pick::Step(Move::PageDown))),
            (with(C::Up, ctrl), Lookup::Unknown),
            (press(C::Enter), Lookup::Run(Pick::Enter)),
            (press(C::Right), Lookup::Run(Pick::Enter)),
            (with(C::Enter, alt), Lookup::Unknown),
            (press(C::Backspace), Lookup::Run(Pick::Up)),
            (press(C::Left), Lookup::Run(Pick::Up)),
            (press(C::Char('a')), Lookup::Run(Pick::Register)),
            (press(C::Char('.')), Lookup::Run(Pick::Hidden)),
            // 뜻을 바꿨다(moai-ob4c) — 경로 적기는 `g p`, `g` 하나는 기다린다.
            (press(C::Char('g')), Lookup::Pending),
            (press(C::Esc), Lookup::Run(Pick::Close)),
            (press(C::Char('q')), Lookup::Run(Pick::Close)),
            (with(C::Esc, ctrl), Lookup::Unknown),
        ];
        for (k, want) in pick {
            assert_eq!(one(PICK, k), want, "창 {k:?}");
        }
        let path = [
            (press(C::Enter), Lookup::Run(Goto::Go)),
            (press(C::Esc), Lookup::Run(Goto::Cancel)),
            (with(C::Enter, alt), Lookup::Unknown),
            (with(C::Esc, ctrl), Lookup::Unknown),
        ];
        for (k, want) in path {
            assert_eq!(one(PATH, k), want, "경로 칸 {k:?}");
        }
        let prompt = [
            (press(C::Enter), Lookup::Run(Prompt::Apply)),
            (with(C::Enter, alt), Lookup::Run(Prompt::Apply)),
            (press(C::Esc), Lookup::Run(Prompt::Cancel)),
            (with(C::Enter, ctrl), Lookup::Unknown),
            (press(C::Tab), Lookup::Unknown),
        ];
        for (k, want) in prompt {
            assert_eq!(one(PROMPT, k), want, "글칸 {k:?}");
        }
        let jot = [
            (with(C::Char('s'), ctrl), Lookup::Run(Jot::Save)),
            (with(C::Char('S'), ctrl | KeyModifiers::SHIFT), Lookup::Run(Jot::Save)),
            (with(C::Char('s'), CTRL_ALT), Lookup::Run(Jot::Save)),
            (press(C::F(2)), Lookup::Unknown),
            (press(C::Tab), Lookup::Run(Jot::Switch)),
            (press(C::BackTab), Lookup::Run(Jot::Switch)),
            (press(C::Esc), Lookup::Run(Jot::Close)),
            (press(C::Enter), Lookup::Run(Jot::Next)),
            (with(C::Enter, ctrl), Lookup::Unknown),
        ];
        for (k, want) in jot {
            assert_eq!(one(JOT, k), want, "폼 {k:?}");
        }
    }

    /// **`moai tui --help` 에 적힌 키는 표에 있다.** 도움말은 손으로 쓴 글로 두되(지어내면 글이
    /// 나빠진다), 글에서 키 이름으로 읽히는 낱말을 전부 뽑아 어느 표에서 뜻이 있는지 본다 — 키를
    /// 걷고 도움말을 안 고치면 여기서 이름을 대며 멈춘다.
    #[test]
    fn every_key_the_help_names_is_in_a_table() {
        let named = help_keys();
        for (word, k) in &named {
            assert!(known(k) != Lookup::Unknown, "도움말이 `{word}` 를 대는데 어느 키 표에도 없다");
        }
        let words: Vec<&str> = named.iter().map(|(w, _)| w.as_str()).collect();
        assert!(!words.iter().any(|w| w.starts_with('F') && parse(w).is_some()), "도움말이 걷은 F키를 댄다: {words:?}");
        for must in [
            "Ctrl-C", "SPC q", "SPC f", "SPC n", "SPC r", "SPC /", "SPC p a", "SPC p d", "SPC t w", "SPC t r", "SPC", "Enter",
            "Backspace", "Shift-Tab", "j", "k", "h", "l", "gg", "G", "Ctrl-d", "Ctrl-u", "Ctrl-f", "Ctrl-b", "Ctrl-S", "Esc", "/", "g p",
            ".",
        ] {
            assert!(words.contains(&must), "도움말에서 `{must}` 를 못 뽑았다 — 뽑기가 헛돈다: {words:?}");
        }
    }

    /// **표에 이름 붙은 키는 `moai tui --help` 가 댄다**(moai-3l4l) — 위 시험의 거꾸로다. 위만 있으면
    /// 표에 키를 더하고 도움말을 안 고쳐도 지나가, 도움말이 표의 절반만 대는 채로 낡는다(F5·F3·`/`·
    /// `f`·`w` 가 그렇게 빠져 있었다). 숨은 별칭(`label: None`)은 안 본다 — 바에도 메뉴에도 안
    /// 서는 키라 도움말이 대지 않는 것이 맞다. 이름은 **키 열로** 견준다: 표의 `Bksp` 를 도움말은
    /// `Backspace` 로 적는다.
    ///
    /// **표마다 그 표를 말하는 문단 안에서만 찾는다**(moai-psqc). 도움말 어디에든 있으면 되던
    /// 때는 여러 표가 함께 쓰는 Enter·Esc·Tab·Bksp 가 한 번만 적혀도 모든 표에서 지나가, 검색
    /// 칸의 Enter 가 빠져도 못 잡았다(리뷰 moai-d5vn.txv). 어느 문단인지는 [`SECTIONS`] 가 문단의
    /// 첫머리 말로 댄다.
    #[test]
    fn every_key_a_table_names_is_in_the_help() {
        let missing = missing_in(&tui_help());
        assert!(missing.is_empty(), "표에 이름 붙은 키를 `moai tui --help` 의 제 문단이 안 댄다: {missing:?}");
    }

    /// **다른 표의 글로는 지나가지 않는다** — 범위가 헛돌지 않는다는 증명. 지울 말마다 그 키는
    /// 도움말 전체에 여전히 있으므로(옛 시험은 못 잡았다) 제 범위만 보는 표가 빠졌다고 말해야 한다.
    ///
    /// - 다른 문단에만 남은 키: 생각 담기의 Tab ↔ 목록의 Tab
    /// - **같은 문단을 같은 키로 나눠 쓰는 표는 양쪽으로 서로를 덮는다**(사용자와 정함, 리뷰
    ///   moai-psqc.172). 좁힌 표(PROMPT·PATH·MENU)는 제 문장만 보고([`SENTENCES`]), 나머지 표의
    ///   글에서는 그 문장을 걷어 낸다 — 한쪽만 좁히면 목록(BROWSE)의 Enter 가 빠져도 검색 칸
    ///   문장의 Enter 로 지나갔다
    #[test]
    fn a_key_told_only_in_another_paragraph_does_not_count() {
        let help = tui_help();
        assert!(missing_in(&help).is_empty(), "고치기 전 도움말부터 빠진 키가 있다");
        // (지울 말, 바꿀 말, 도움말 전체엔 남아야 하는 키, 빠졌다고 해야 하는 것)
        for (phrase, instead, still, want) in [
            // 다른 문단에만 남은 키 — 목록 문단의 Tab.
            ("Tab 이 둘 사이를 옮기고", "둘 사이를 옮기고", &["Tab"][..], &["JOT: Tab"][..]),
            // 좁힌 표 — 같은 문단의 목록 Enter·Esc 로 지나가면 안 된다.
            ("검색·거름망 칸은 Enter 로 걸고 Esc 로 그만둔다.", "검색·거름망 칸은 그 칸에서 걸고 그만둔다.", &["Enter", "Esc"][..], &["PROMPT: Enter", "PROMPT: Esc"][..]),
            // 좁힌 표 — 같은 문단의 고르기 창 Enter·Esc 로 지나가면 안 된다.
            ("(Enter 로 가고 Esc 로", "(가고", &["Enter", "Esc"][..], &["PATH: Enter", "PATH: Esc"][..]),
            // 거꾸로 — 목록이 검색 칸 문장의 Enter, SPC 메뉴 문장의 Backspace 로 지나가면 안 된다.
            ("Enter·l 로 들어가고", "l 로 들어가고", &["Enter"][..], &["BROWSE: Enter"][..]),
            ("Backspace·h 로 나온다", "h 로 나온다", &["Backspace"][..], &["BROWSE: Bksp"][..]),
            // 거꾸로 — 고르기 창이 경로 칸 문장의 Esc 로 지나가면 안 된다.
            ("창은 Esc 로 닫는다", "창은 닫는다", &["Esc"][..], &["PICK: Esc"][..]),
        ] {
            assert!(help.contains(phrase), "시험이 지울 말 `{phrase}` 이 도움말에 없다 — 문장이 바뀌었으면 여기도 고친다");
            let broken = help.replace(phrase, instead);
            let everywhere: Vec<String> = keys_in(&broken).into_iter().map(|(w, _)| w).collect();
            for k in still {
                assert!(everywhere.iter().any(|w| w == k), "도움말 전체에서 {k} 가 사라져 증명이 안 된다 — 옛 시험도 잡았을 경우다");
            }
            let missing = missing_in(&broken);
            for w in want {
                assert!(missing.iter().any(|m| m == w), "`{phrase}` 를 지웠는데 {w} 를 못 잡았다: {missing:?}");
            }
        }
    }

    /// 목록 문단의 첫머리 말.
    const LIST: &str = "j·k 나 화살표로 이동";
    /// SPC 메뉴 문단.
    const SPC: &str = "그 밖의 동작은 SPC";
    /// 고르기 창·경로 칸·목록에서 빼기 문단.
    const PICKER: &str = "SPC p a 는 디렉터리를 골라";
    /// 생각 담기 문단.
    const JOTTING: &str = "SPC n 은 프로젝트 안";

    /// 같은 문단을 **같은 키로** 나눠 쓰는 표 → (그 문단, 그 표를 말하는 문장의 첫머리 말). 문장은
    /// 그 말부터 첫 `.` 까지이고 **그 문단 안에서만** 찾는다 — 도움말 어디든 찾으면 같은 말이 앞선
    /// 문단에 생긴 날 엉뚱한 문장을 본다.
    ///
    /// 여기 든 표는 제 문장만 보고, **같은 문단의 다른 표는 이 문장을 걷어 낸 글을 본다** — 둘 중
    /// 한쪽만 좁히면 남은 쪽이 좁힌 표의 Enter·Esc·Bksp 로 지나간다(리뷰 moai-psqc.172).
    const SENTENCES: &[(&str, &str, &str)] = &[
        // 목록(BROWSE)과 Enter·Esc 를 나눠 쓴다.
        ("PROMPT", LIST, "검색·거름망 칸은"),
        // 목록(BROWSE)과 Esc·Bksp 를 나눠 쓴다.
        ("MENU", SPC, "메뉴는 그 자리에서"),
        // 고르기 창(PICK)과 Enter·Esc 를 나눠 쓴다. `g p` 는 창의 것이라 괄호부터 잡는다.
        ("PATH", PICKER, "적는 칸을 연다("),
    ];

    /// [`SENTENCES`] 에 없는 표 → 그 표를 말하는 문단들. 문단은 빈 줄로 나눈 덩어리다.
    ///
    /// - 고르기 창(PICK)의 **이동 키만** 목록 문단도 본다 — 도움말이 "이동은 목록과 같은 j·k·gg·G"
    ///   로 쪽 이동(Ctrl-d 따위)을 목록 문단에 맡긴다. Enter·Esc·Bksp 까지 빌리면 목록의 것으로 지나간다
    /// - 확인(CONFIRM)의 `y` 는 목록에서 빼기(고르기 창 문단)와 생각 담기 문단 둘에서 묻는다 — 한
    ///   번 적히면 된다
    const SECTIONS: &[(&str, &[&str])] = &[
        ("ANYWHERE", &[SPC]),
        ("BROWSE", &[LIST, SPC]),
        ("PICK", &[PICKER]),
        ("JOT", &[JOTTING]),
        ("CONFIRM", &[PICKER, JOTTING]),
    ];

    /// `head` 로 시작하는 문단.
    fn paragraph<'h>(help: &'h str, head: &str) -> &'h str {
        help.split("\n\n")
            .find(|p| p.trim_start().starts_with(head))
            .unwrap_or_else(|| panic!("`{head}` 로 시작하는 문단을 못 찾았다 — 도움말 문단이 바뀌었으면 SECTIONS·SENTENCES 를 고친다"))
    }

    /// `par` 문단 안에서 `head` 부터 첫 `.` 까지.
    fn sentence<'h>(help: &'h str, par: &str, head: &str) -> &'h str {
        let p = paragraph(help, par);
        let start = p.find(head).unwrap_or_else(|| panic!("`{par}` 문단에 `{head}` 로 시작하는 문장이 없다 — 도움말 문장이 바뀌었으면 SENTENCES 를 고친다"));
        let rest = &p[start..];
        &rest[..rest.find('.').map_or(rest.len(), |e| e + 1)]
    }

    /// 문단들을 잇되 **좁힌 표의 문장은 걷어 낸** 글 — 옆 표의 같은 키로 지나가지 않게.
    fn scope(help: &str, heads: &[&str]) -> String {
        heads
            .iter()
            .map(|h| {
                let mut text = paragraph(help, h).to_string();
                for (_, par, head) in SENTENCES.iter().filter(|(_, par, _)| par == h) {
                    text = text.replace(sentence(help, par, head), "");
                }
                text
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// `help` 에서 표마다 제 범위([`SENTENCES`]·[`SECTIONS`])가 안 대는 이름 붙은 키 — `표: 이름`.
    fn missing_in(help: &str) -> Vec<String> {
        let tables: [(&str, Vec<(&'static str, &'static [Key])>); 8] = [
            ("ANYWHERE", named(ANYWHERE)),
            ("BROWSE", named(BROWSE)),
            ("MENU", named(MENU)),
            ("PROMPT", named(PROMPT)),
            ("PICK", named(PICK)),
            ("PATH", named(PATH)),
            ("JOT", named(JOT)),
            ("CONFIRM", named(CONFIRM)),
        ];
        // 고르기 창이 목록 문단에서 빌리는 이동 키 — 표와 같은 매크로에서 읽는다.
        // `const` 로 받는다 — 매크로의 `&[…]` 는 상수 자리에서만 `'static` 이다(표도 그렇게 받는다).
        const MOVES: [Bind<Pick>; 14] = moves!(Pick::Step, Key::bare);
        let moves: Vec<&str> = MOVES.iter().filter_map(|b| b.label).collect();
        let told = |text: &str, seq: &[Key]| {
            keys_in(text).into_iter().any(|(_, k)| k.len() == seq.len() && seq.iter().zip(&k).all(|(key, ev)| key.matches(*ev)))
        };
        let mut missing = Vec::new();
        for (table, rows) in tables {
            let own = match SENTENCES.iter().find(|(t, ..)| *t == table) {
                Some((_, par, head)) => sentence(help, par, head).to_string(),
                None => {
                    let heads = SECTIONS.iter().find(|(t, _)| *t == table).map(|(_, h)| *h).unwrap_or_else(|| panic!("{table} 의 범위가 SECTIONS·SENTENCES 에 없다"));
                    scope(help, heads)
                }
            };
            let borrowed = format!("{own}\n\n{}", scope(help, &[LIST]));
            for (name, seq) in rows {
                let text = if table == "PICK" && moves.contains(&name) { borrowed.as_str() } else { own.as_str() };
                if !told(text, seq) {
                    missing.push(format!("{table}: {name}"));
                }
            }
        }
        missing
    }

    fn tui_help() -> String {
        use clap::CommandFactory;
        let cli = crate::cli::Cli::command();
        cli.find_subcommand("tui").and_then(|c| c.get_after_help()).map(|h| h.to_string()).expect("tui 도움말이 없다")
    }

    /// 표의 이름 붙은 줄 — (이름, 키 열).
    fn named<A>(table: &[Bind<A>]) -> Vec<(&'static str, &'static [Key])> {
        table.iter().filter_map(|b| b.label.map(|l| (l, b.seq))).collect()
    }

    fn bare<A>(l: Lookup<A>) -> Lookup<()> {
        match l {
            Lookup::Run(_) => Lookup::Run(()),
            Lookup::Pending => Lookup::Pending,
            Lookup::Unknown => Lookup::Unknown,
        }
    }

    /// 어느 표에서든 그 열의 뜻. 모르면 `Unknown`.
    fn known(k: &[KeyEvent]) -> Lookup<()> {
        let tables = [
            bare(lookup(ANYWHERE, k)),
            bare(lookup(MENU, k)),
            bare(lookup(BROWSE, k)),
            bare(lookup(PICK, k)),
            bare(lookup(JOT, k)),
            bare(lookup(PROMPT, k)),
            bare(lookup(PATH, k)),
            bare(lookup(CONFIRM, k)),
        ];
        if tables.contains(&Lookup::Run(())) {
            Lookup::Run(())
        } else if tables.contains(&Lookup::Pending) {
            Lookup::Pending
        } else {
            Lookup::Unknown
        }
    }

    /// 도움말에서 키 이름으로 읽히는 낱말을 전부 뽑는다 — (적힌 낱말, 키 열).
    ///
    /// **띄어 적은 열(`SPC t w`·`g p`)을 한 낱말로 묶는다**: 어느 표에서 접두어로 읽히는 낱말 뒤에
    /// 한 글자 낱말이 이어지면, 열이 접두어인 동안 붙인다. `.` 은 그 자체로 키다(고르기 창의 숨은 것)
    /// — **띄어 쓴 자리에 홀로 선** `` `.` `` 만 키로 읽는다. 문장 끝의 `.` 은 떼고, `줄).` 처럼 괄호
    /// 뒤에 남은 `.` 도 키가 아니다 — 그것까지 읽으면 도움말이 `.` 을 안 대도 시험이 지나간다.
    fn help_keys() -> Vec<(String, Vec<KeyEvent>)> {
        keys_in(&tui_help())
    }

    /// [`help_keys`] 와 같되 **주어진 글에서** 뽑는다 — 표마다 제 문단만 보려고 나눴다(moai-psqc).
    fn keys_in(help: &str) -> Vec<(String, Vec<KeyEvent>)> {
        let words: Vec<&str> = help
            .split_whitespace()
            .flat_map(|token| -> Vec<&str> {
                if token.trim_matches('`') == "." {
                    return vec!["."];
                }
                token
                    .split(|c: char| "·,()`".contains(c))
                    .map(|w| w.trim_end_matches(['.', '|']))
                    .filter(|w| !w.is_empty() && *w != ".")
                    .collect()
            })
            .collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < words.len() {
            let mut word = words[i].to_string();
            if let Some(mut k) = parse_seq(&word) {
                while known(&k) == Lookup::Pending
                    && let Some(next) = words.get(i + 1).filter(|w| w.len() == 1 && w.is_ascii())
                    && let Some(more) = parse(next)
                {
                    k.push(more);
                    word = format!("{word} {next}");
                    i += 1;
                }
                out.push((word, k));
            }
            i += 1;
        }
        out
    }

    fn layer() -> Ctx {
        Ctx { layer: true, list_focus: true, ..Ctx::default() }
    }

    /// **층의 거절문은 옮기기 전과 한 글자도 같다.** 문구 속 키 이름은 표에서 읽는다. 바로 누르는
    /// 키로 남은 `/` 만 까닭을 댄다.
    #[test]
    fn the_layer_refuses_with_the_same_words() {
        let filter = Off::Why("거름망은 프로젝트 안의 줄에 건다 — Enter 로 들어가서 건다".into());
        assert_eq!(Browse::Grep.enabled(&layer()), Err(filter.clone()));
        assert_eq!(Browse::Filter.enabled(&layer()), Err(filter));
        // 워크트리는 메뉴에만 있고 층의 메뉴에 안 선다 — 까닭을 댈 자리가 없어 말없이 꺼 둔다(moai-7sjm).
        assert_eq!(Browse::Worktree.enabled(&layer()), Err(Off::Quiet));
        assert_eq!(Browse::Jot.enabled(&layer()), Ok(()), "층의 `n` 은 커서의 프로젝트에 담는다");
        assert_eq!(Browse::Unregister.enabled(&layer()), Ok(()));
        let detail = Ctx { list_focus: false, ..layer() };
        assert_eq!(Browse::Unregister.enabled(&detail), Err(Off::Quiet));
        assert_eq!(Browse::Enter.enabled(&detail), Err(Off::Quiet));
        let inside = Ctx { list_focus: true, ..Ctx::default() };
        assert_eq!(Browse::Unregister.enabled(&inside), Err(Off::Quiet), "프로젝트 안에서 해제가 켜졌다");
        assert_eq!(Browse::Worktree.enabled(&inside), Ok(()));
    }

    /// **드나드는 키는 커서가 선 줄을 탄다**(moai-k3yi). 잎의 Enter 와 뿌리의 Bksp 는 조용히
    /// 꺼진다 — 바와 키 처리가 이 한 판정을 읽는다. 둘은 서로를 끄지 않는다.
    #[test]
    fn enter_and_leave_follow_the_cursor_row() {
        let base = Ctx { list_focus: true, ..Ctx::default() };
        assert_eq!(Browse::Enter.enabled(&base), Ok(()));
        assert_eq!(Browse::Leave.enabled(&base), Ok(()));
        let leaf = Ctx { leaf: true, ..base };
        assert_eq!(Browse::Enter.enabled(&leaf), Err(Off::Quiet), "잎에서 Enter 가 켜졌다");
        assert_eq!(Browse::Leave.enabled(&leaf), Ok(()), "잎이 나가기를 껐다");
        let root = Ctx { root: true, ..base };
        assert_eq!(Browse::Leave.enabled(&root), Err(Off::Quiet), "뿌리에서 Bksp 가 켜졌다");
        assert_eq!(Browse::Enter.enabled(&root), Ok(()), "뿌리가 들어가기를 껐다");
        // 커서의 사실은 드나드는 키만 끈다 — 다른 동작은 그대로다.
        let both = Ctx { leaf: true, root: true, ..base };
        for act in [Browse::Grep, Browse::Filter, Browse::Jot, Browse::Reload, Browse::Quit, Browse::Step(Move::Top)] {
            assert_eq!(act.enabled(&both), Ok(()), "{act:?}");
        }
    }

    /// **기다리는 `g` 뒤에 이어 누를 키는 표에서 읽는다** — 이름 붙은 줄만, 숨은 별칭 없이.
    #[test]
    fn next_keys_after_g_come_from_the_table() {
        let g = [KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE)];
        assert_eq!(next_keys(BROWSE, &g), vec![("g".to_string(), Browse::Step(Move::Top))]);
        assert_eq!(next_keys(PICK, &g), vec![("g".to_string(), Pick::Step(Move::Top)), ("p".to_string(), Pick::Path)]);
        assert!(next_keys(BROWSE, &[]).iter().all(|(n, _)| n != "Right" && n != "l"), "숨은 별칭을 댄다");
    }
}
