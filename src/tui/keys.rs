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
//! **한 줄은 키의 열이다**([`Bind::seq`]). 오늘 표에는 한 키짜리만 있지만, `g g`·`SPC t w`
//! 같은 접두어를 줄 하나로 적을 수 있고 [`lookup`] 이 [`Lookup::Pending`] 으로 "더 기다린다"
//! 를 낸다. 기다리는 동안의 열은 든 쪽이 들고 다음 키를 붙여 다시 부른다.

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
    /// 적는다(`q`·`r`·`m`·F7·Delete·→·←·BackTab). 이름은 그 키로 다시 읽혀야 한다 — 시험이
    /// 이름을 키로 풀어 이 줄의 동작이 나오는지 본다.
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

/// 동작에 붙은 키 이름. 여럿이면 `·` 로 잇는다(`Ctrl-S·F2`).
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
    /// 이동키 하나를 포커스 칸에 준다 — 어느 키였는지는 든 쪽이 키로 가른다.
    Step,
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
    DetailDown,
    DetailUp,
    DetailPageDown,
    DetailPageUp,
}

pub const BROWSE: &[Bind<Browse>] = {
    use Browse::*;
    use KeyCode as C;
    &[
        row!(Quit, None, Key::plain('q')),
        row!(Quit, Some("F10"), Key::any(C::F(10))),
        row!(FocusNext, Some("Tab"), Key::unshift(C::Tab)),
        row!(FocusPrev, None, Key::shift(C::Tab)),
        row!(Step, None, Key::any(C::Up)),
        row!(Step, None, Key::any(C::Down)),
        row!(Step, None, Key::any(C::Home)),
        row!(Step, None, Key::any(C::End)),
        row!(Step, None, Key::any(C::PageUp)),
        row!(Step, None, Key::any(C::PageDown)),
        row!(Enter, Some("Enter"), Key::any(C::Enter)),
        row!(Enter, None, Key::any(C::Right)),
        row!(Leave, Some("Bksp"), Key::any(C::Backspace)),
        row!(Leave, None, Key::any(C::Left)),
        row!(Grep, Some("/"), Key::plain('/')),
        row!(Filter, Some("f"), Key::plain('f')),
        row!(Filter, None, Key::any(C::F(7))),
        row!(Jot, Some("n"), Key::plain('n')),
        row!(Pick, Some("a"), Key::plain('a')),
        row!(Unregister, Some("d"), Key::plain('d')),
        row!(Unregister, None, Key::any(C::Delete)),
        row!(ClearFilter, Some("Esc"), Key::any(C::Esc)),
        row!(Reload, Some("F5"), Key::any(C::F(5))),
        row!(Reload, None, Key::plain('r')),
        row!(Worktree, Some("w"), Key::plain('w')),
        row!(Raw, Some("F3"), Key::any(C::F(3))),
        row!(Raw, None, Key::plain('m')),
        row!(DetailDown, Some("j"), Key::plain('j')),
        row!(DetailUp, Some("k"), Key::plain('k')),
        row!(DetailPageDown, None, Key::plain(' ')),
        row!(DetailPageUp, None, Key::plain('b')),
    ]
};

/// 켜짐을 가르는 값. **든 쪽이 잰다** — 여기는 `App` 을 모른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ctx {
    /// 프로젝트 층에 섰나.
    pub layer: bool,
    /// 포커스가 목록에 있나.
    pub list_focus: bool,
    pub worktree: bool,
    pub raw: bool,
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
    /// 이 자리에서 되는가. **키 처리·키 바·(뒤의) SPC 메뉴가 같은 판정을 읽는다** — 키
    /// 처리는 까닭을 알림으로 대고, 바와 메뉴는 켜진 것만 세운다.
    ///
    /// - 드나드는 키는 **목록 포커스를 탄다.** 상세를 읽다가 누른 Enter·←가 목록을 옮기면
    ///   보던 이슈가 바뀐다
    /// - 해제는 층의 줄에서만, 목록 포커스로 — 지금 선 프로젝트를 빼는 일이 안 생긴다
    /// - 층에서 프로젝트 안의 줄에 매인 키(거름망·검색·워크트리)는 까닭을 댄다. `n` 은
    ///   층에서도 듣는다 — 커서의 프로젝트에 담는다(moai-fccv)
    pub fn enabled(self, c: &Ctx) -> Result<(), Off> {
        use Browse::*;
        match self {
            Enter | Leave if !c.list_focus => Err(Off::Quiet),
            Unregister if !(c.layer && c.list_focus) => Err(Off::Quiet),
            Grep | Filter if c.layer => {
                Err(Off::Why(format!("거름망은 프로젝트 안의 줄에 건다 — {} 로 들어가서 건다", label(BROWSE, Enter))))
            }
            Worktree if c.layer => Err(Off::Why(format!(
                "워크트리 겹쳐 보기는 프로젝트 안에서 켠다 — {} 로 들어가서 {}",
                label(BROWSE, Enter),
                label(BROWSE, Worktree)
            ))),
            _ => Ok(()),
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
            Step => "이동",
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
            DetailDown | DetailUp | DetailPageDown | DetailPageUp => "굴리기",
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
    /// 이동키 — 어느 키였는지는 [`super::scroll::cursor`] 가 가른다.
    Step,
    Enter,
    Up,
    Register,
    Hidden,
    Path,
    Close,
}

/// 창은 **Ctrl·Alt 붙은 키를 전부 거른다** — 줄이 모두 `bare` 다.
pub const PICK: &[Bind<Pick>] = {
    use KeyCode as C;
    use Pick::*;
    &[
        row!(Step, None, Key::bare(C::Up)),
        row!(Step, None, Key::bare(C::Down)),
        row!(Step, None, Key::bare(C::Home)),
        row!(Step, None, Key::bare(C::End)),
        row!(Step, None, Key::bare(C::PageUp)),
        row!(Step, None, Key::bare(C::PageDown)),
        row!(Enter, Some("Enter"), Key::bare(C::Enter)),
        row!(Enter, None, Key::bare(C::Right)),
        row!(Up, Some("Bksp"), Key::bare(C::Backspace)),
        row!(Up, None, Key::bare(C::Left)),
        row!(Register, Some("a"), Key::plain('a')),
        row!(Hidden, Some("."), Key::plain('.')),
        row!(Path, Some("g"), Key::plain('g')),
        row!(Close, Some("Esc"), Key::bare(C::Esc)),
        row!(Close, None, Key::plain('q')),
    ]
};

impl Pick {
    pub fn what(self, show_hidden: bool) -> &'static str {
        use Pick::*;
        match self {
            Step => "이동",
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
        row!(Save, Some("Ctrl-S"), Key::ctrl('s')),
        row!(Save, None, Key::ctrl('S')),
        row!(Save, Some("F2"), Key::any(C::F(2))),
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
        for c in ['a', 'd', 'f', 'n', 'q', 'w', '/', 'j', 'p'] {
            for m in [KeyModifiers::CONTROL, KeyModifiers::ALT, CTRL_ALT] {
                assert_eq!(one(BROWSE, with(KeyCode::Char(c), m)), Lookup::Unknown, "{m:?}-{c}");
                assert_eq!(one(PICK, with(KeyCode::Char(c), m)), Lookup::Unknown, "창 {m:?}-{c}");
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

    /// **접두어는 기다린다.** 오늘 표에는 접두어가 없어 조그만 표로 잰다 — `g g` 는 맨 위,
    /// `SPC t w` 는 셋째 키에 선다. 모르는 둘째 키는 뜻이 없다.
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
    }

    /// **적힌 키 이름은 그 키다.** 이름을 키로 풀어 표에서 찾으면 그 줄의 동작이 나온다 — 바에
    /// `F5 갱신` 이라 적어 두고 표에서는 F5 를 걷는 일이 여기서 갈린다.
    #[test]
    fn every_label_is_the_key_it_names() {
        fn each<A: Copy + PartialEq + std::fmt::Debug>(name: &str, table: &[Bind<A>]) {
            for b in table.iter().filter(|b| b.label.is_some()) {
                let label = b.label.unwrap();
                let k = parse(label).unwrap_or_else(|| panic!("{name}: `{label}` 를 키로 못 푼다"));
                assert_eq!(lookup(table, &[k]), Lookup::Run(b.act), "{name}: `{label}`");
            }
        }
        each("anywhere", ANYWHERE);
        each("browse", BROWSE);
        each("prompt", PROMPT);
        each("pick", PICK);
        each("path", PATH);
        each("jot", JOT);
        each("confirm", CONFIRM);
        assert_eq!(label(JOT, Jot::Save), "Ctrl-S·F2");
        assert_eq!(labels(BROWSE, &[Browse::DetailDown, Browse::DetailUp]), "j·k");
        assert_eq!(label(BROWSE, Browse::Quit), "F10", "숨은 별칭 `q` 가 이름에 섰다");
    }

    /// **옮기기 전 코드가 받던 키가 같은 동작이 된다**(moai-gaum 의 키 목록). 수식키가 붙은 모양도
    /// 옛 코드가 받던 그대로다 — 비글자 키는 Ctrl·Alt 를 안 봤고, 창은 통째로 걸렀다.
    #[test]
    fn the_old_inventory_maps_to_the_same_actions() {
        use Browse as B;
        use KeyCode as C;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let browse = [
            (press(C::Char('q')), Lookup::Run(B::Quit)),
            (press(C::F(10)), Lookup::Run(B::Quit)),
            (with(C::F(10), ctrl), Lookup::Run(B::Quit)),
            (press(C::Tab), Lookup::Run(B::FocusNext)),
            (with(C::Tab, ctrl), Lookup::Run(B::FocusNext)),
            (press(C::BackTab), Lookup::Run(B::FocusPrev)),
            (with(C::Tab, KeyModifiers::SHIFT), Lookup::Run(B::FocusPrev)),
            (press(C::Up), Lookup::Run(B::Step)),
            (with(C::Down, ctrl), Lookup::Run(B::Step)),
            (press(C::Home), Lookup::Run(B::Step)),
            (press(C::End), Lookup::Run(B::Step)),
            (press(C::PageUp), Lookup::Run(B::Step)),
            (with(C::PageDown, KeyModifiers::SHIFT), Lookup::Run(B::Step)),
            (press(C::Enter), Lookup::Run(B::Enter)),
            (press(C::Right), Lookup::Run(B::Enter)),
            (press(C::Backspace), Lookup::Run(B::Leave)),
            (press(C::Left), Lookup::Run(B::Leave)),
            (press(C::Char('/')), Lookup::Run(B::Grep)),
            (press(C::Char('f')), Lookup::Run(B::Filter)),
            (press(C::F(7)), Lookup::Run(B::Filter)),
            (press(C::Char('n')), Lookup::Run(B::Jot)),
            (press(C::Char('a')), Lookup::Run(B::Pick)),
            (press(C::Char('d')), Lookup::Run(B::Unregister)),
            (press(C::Delete), Lookup::Run(B::Unregister)),
            (press(C::Esc), Lookup::Run(B::ClearFilter)),
            (press(C::F(5)), Lookup::Run(B::Reload)),
            (press(C::Char('r')), Lookup::Run(B::Reload)),
            (press(C::Char('w')), Lookup::Run(B::Worktree)),
            (press(C::F(3)), Lookup::Run(B::Raw)),
            (press(C::Char('m')), Lookup::Run(B::Raw)),
            // 진행 바탕 토글 `p` 는 main 이 기능째 걷었다(moai-u3r2).
            (press(C::Char('p')), Lookup::Unknown),
            (press(C::Char('j')), Lookup::Run(B::DetailDown)),
            (press(C::Char('k')), Lookup::Run(B::DetailUp)),
            (press(C::Char(' ')), Lookup::Run(B::DetailPageDown)),
            (press(C::Char('b')), Lookup::Run(B::DetailPageUp)),
            (with(C::Char('a'), ctrl), Lookup::Unknown),
            (with(C::Char('d'), alt), Lookup::Unknown),
            (press(C::Char('x')), Lookup::Unknown),
            (press(C::F(1)), Lookup::Unknown),
        ];
        for (k, want) in browse {
            assert_eq!(one(BROWSE, k), want, "탐색 {k:?}");
        }
        let pick = [
            (press(C::Up), Lookup::Run(Pick::Step)),
            (press(C::PageDown), Lookup::Run(Pick::Step)),
            (with(C::Up, ctrl), Lookup::Unknown),
            (press(C::Enter), Lookup::Run(Pick::Enter)),
            (press(C::Right), Lookup::Run(Pick::Enter)),
            (with(C::Enter, alt), Lookup::Unknown),
            (press(C::Backspace), Lookup::Run(Pick::Up)),
            (press(C::Left), Lookup::Run(Pick::Up)),
            (press(C::Char('a')), Lookup::Run(Pick::Register)),
            (press(C::Char('.')), Lookup::Run(Pick::Hidden)),
            (press(C::Char('g')), Lookup::Run(Pick::Path)),
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
            (press(C::F(2)), Lookup::Run(Jot::Save)),
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
        use clap::CommandFactory;
        let cli = crate::cli::Cli::command();
        let help = cli.find_subcommand("tui").and_then(|c| c.get_after_help()).map(|h| h.to_string()).expect("tui 도움말이 없다");
        let mut named = Vec::new();
        for word in help.split(|c: char| c.is_whitespace() || "·/,()`".contains(c)) {
            let word = word.trim_end_matches(['.', '|']);
            let Some(k) = parse(word) else { continue };
            let known = lookup(ANYWHERE, &[k]) != Lookup::Unknown
                || lookup(BROWSE, &[k]) != Lookup::Unknown
                || lookup(PICK, &[k]) != Lookup::Unknown
                || lookup(JOT, &[k]) != Lookup::Unknown;
            assert!(known, "도움말이 `{word}` 를 대는데 어느 키 표에도 없다");
            named.push(word);
        }
        for must in ["F10", "q", "Enter", "Backspace", "Shift-Tab", "j", "a", "d", "n", "Ctrl-S", "F2", "Esc"] {
            assert!(named.contains(&must), "도움말에서 `{must}` 를 못 뽑았다 — 뽑기가 헛돈다: {named:?}");
        }
    }

    fn layer() -> Ctx {
        Ctx { layer: true, list_focus: true, ..Ctx::default() }
    }

    /// **층의 거절문은 옮기기 전과 한 글자도 같다.** 문구 속 키 이름은 표에서 읽는다.
    #[test]
    fn the_layer_refuses_with_the_same_words() {
        let filter = Off::Why("거름망은 프로젝트 안의 줄에 건다 — Enter 로 들어가서 건다".into());
        assert_eq!(Browse::Grep.enabled(&layer()), Err(filter.clone()));
        assert_eq!(Browse::Filter.enabled(&layer()), Err(filter));
        assert_eq!(
            Browse::Worktree.enabled(&layer()),
            Err(Off::Why("워크트리 겹쳐 보기는 프로젝트 안에서 켠다 — Enter 로 들어가서 w".into()))
        );
        assert_eq!(Browse::Jot.enabled(&layer()), Ok(()), "층의 `n` 은 커서의 프로젝트에 담는다");
        assert_eq!(Browse::Unregister.enabled(&layer()), Ok(()));
        let detail = Ctx { list_focus: false, ..layer() };
        assert_eq!(Browse::Unregister.enabled(&detail), Err(Off::Quiet));
        assert_eq!(Browse::Enter.enabled(&detail), Err(Off::Quiet));
        let inside = Ctx { list_focus: true, ..Ctx::default() };
        assert_eq!(Browse::Unregister.enabled(&inside), Err(Off::Quiet), "프로젝트 안에서 해제가 켜졌다");
        assert_eq!(Browse::Worktree.enabled(&inside), Ok(()));
    }
}
