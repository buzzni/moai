//! 사람의 키 파일(`keys.toml`, moai-pxiy) — 기본 키 표([`super::keys`]) 위에 **바꿀 줄만** 덮어쓴 실행 중의 표.
//!
//! ```toml
//! [browse]
//! quit   = "SPC x"            # 키 열 하나
//! grep   = ["/", "SPC /"]     # 여럿. 첫째가 이름(바·메뉴·도움말이 댄다), 나머지는 숨은 별칭
//! jot    = "none"             # 떼기. [] 도 같다
//! [moves]                     # 이동은 다섯 표(탐색·고르기·통계·위키·링크)가 함께 쓴다
//! line_down = "n"
//! ```
//!
//! - **값 하나가 그 동작의 줄을 모두 갈아 끼운다** — 숨은 별칭(화살표)까지다. 화살표를 남기려면 배열에 함께 적는다
//! - **틀린 줄은 그 줄만 버린다**(2026-10-23 사용자 결정). 그 동작은 기본 키로 남고 [`Problem`] 이 그 줄을 댄다.
//!   TUI 는 열린다 — 키 파일의 오타 하나로 이슈를 못 보면 고장이다. 문법이 깨진 파일만 통째로 기본 표다
//! - **겹침은 다 덮어쓴 뒤에 잰다.** 두 키를 맞바꾸는 파일은 한 줄씩 보면 겹치지만 다 쓰고 나면 안 겹친다.
//!   겹치면 겹친 **사람의 줄**을 버리고 다시 잰다 — 기본 줄끼리는 시험이 이미 막는다
//! - **못 바꾸는 것이 있다**: Ctrl-C(탈출구), 맨 숫자(헤더의 `<n>`)·`SPC v <n>`(칸 번호), 글칸의 키, SPC 자체.
//!   이름이 없으니 파일에 적으면 모르는 키다
//!
//! **순수하다.** 파일을 읽는 것은 [`load`] 하나고, [`build`] 는 받은 글만 본다 — 시험이 터미널도 파일도 없이 돈다.

// 실행 중의 표를 `App` 이 들고 건네는 것은 moai-bzm3 다 — 그때까지 이 모듈을 부르는 것은 시험뿐이다.
#![cfg_attr(not(test), allow(dead_code))]

use super::keys::{
    ANYWHERE, BROWSE, Bind, Browse, CONFIRM, Confirm, Key, LEADER, LINKS, Link, Lookup, MENU, Menu, Order, PICK, Pick,
    SORTER, STATS, Screen, Side, Sorter, Stat, WIKI, Wiki, lookup, name_of_seq, parse_seq,
};
use super::scroll::Move;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// 파일의 자리. `MOAI_KEYS` → 사용자 설정([`crate::user_config::path_from`]) 곁의 `keys.toml`.
/// `MOAI_CONFIG=/t/c.toml` 이면 `/t/keys.toml` 이다 — 시험·격리가 설정 하나로 둘 다 옮긴다.
pub fn path_from(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(p) = env("MOAI_KEYS").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    crate::user_config::path_from(env).map(|c| c.with_file_name("keys.toml"))
}

/// 그 자리의 파일을 읽어 [`build`]. **없으면 기본 표다** — 알릴 것이 없다. 못 읽으면 기본 표에 그 까닭 하나.
pub fn load(path: Option<&Path>) -> (Keymap, Vec<Problem>) {
    let Some(path) = path else { return (Keymap::default(), Vec::new()) };
    match std::fs::read_to_string(path) {
        Ok(text) => build(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Keymap::default(), Vec::new()),
        Err(e) => {
            (Keymap::default(), vec![Problem { at: String::new(), line: None, why: Why::Unreadable(e.to_string()) }])
        }
    }
}

/// 실행 중의 표 — 파일이 바꾼 표는 새로 짓고, 안 바꾼 표는 기본 표 그대로다. **`&'static` 이다**: 시작 때 한 번
/// 지어 [`Box::leak`] 한다 — 한 프로세스에 한 번이라 샘이 아니고, 그래야 [`lookup`]·[`super::keys::label`] 이
/// 오늘의 서명 그대로 받는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keymap {
    pub browse: &'static [Bind<Browse>],
    pub menu: &'static [Bind<Menu>],
    pub sorter: &'static [Bind<Sorter>],
    pub pick: &'static [Bind<Pick>],
    pub stats: &'static [Bind<Stat>],
    pub wiki: &'static [Bind<Wiki>],
    pub links: &'static [Bind<Link>],
    pub confirm: &'static [Bind<Confirm>],
}

impl Default for Keymap {
    fn default() -> Keymap {
        Keymap {
            browse: BROWSE,
            menu: MENU,
            sorter: SORTER,
            pick: PICK,
            stats: STATS,
            wiki: WIKI,
            links: LINKS,
            confirm: CONFIRM,
        }
    }
}

/// 버린 줄 하나. **말이 아니라 자료다** — 화면의 한 줄과 `moai keys` 가 제 말로 편다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// `browse.quit` — 절만이면 `browse`, 파일 전체면 빈 글.
    pub at: String,
    /// 파일의 몇째 줄(1부터). 모르면 `None`.
    pub line: Option<usize>,
    pub why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// 파일을 못 읽었다(권한 따위). 기본 표다.
    Unreadable(String),
    /// TOML 문법이 깨졌다 — 어느 줄을 버릴지 못 가려 파일 전체를 버린다.
    Syntax(String),
    /// 모르는 절.
    UnknownSection,
    /// 절 이름 자리에 표가 아닌 것이 섰다.
    NotATable,
    /// 그 절에 없는 동작 — 못 바꾸는 것(숫자 줄)도 여기다.
    UnknownAction,
    /// 값이 키 열 글이나 그 배열이 아니다.
    NotKeys,
    /// 키 이름이 아니다 — 그 글.
    BadKey(String),
    /// 키 하나씩만 읽는 창에 키 열을 줬다 — 그 열.
    OneKey(String),
    /// 다른 줄과 겹쳐 눌러도 안 듣는다. `keys` 는 이 줄의 키 열, `with` 는 겹친 쪽(`browse.grep`·`Ctrl-C`·`SPC`).
    Clash { keys: String, with: String },
}

/// 파일의 절. 이동(`moves`)은 표가 아니라 다섯 표가 함께 쓰는 줄이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Browse,
    Moves,
    Menu,
    Sorter,
    Picker,
    Stats,
    Wiki,
    Links,
    Confirm,
}

impl Section {
    const ALL: [Section; 9] = [
        Section::Browse,
        Section::Moves,
        Section::Menu,
        Section::Sorter,
        Section::Picker,
        Section::Stats,
        Section::Wiki,
        Section::Links,
        Section::Confirm,
    ];

    fn name(self) -> &'static str {
        match self {
            Section::Browse => "browse",
            Section::Moves => "moves",
            Section::Menu => "menu",
            Section::Sorter => "sorter",
            Section::Picker => "picker",
            Section::Stats => "stats",
            Section::Wiki => "wiki",
            Section::Links => "links",
            Section::Confirm => "confirm",
        }
    }

    /// **키를 하나씩만 찾는 창** — 메뉴의 Esc·Bksp(`menu::feed`), 차례 편집 창(`App::sorter_key`), 물음. 키 열을
    /// 주면 그 줄은 영영 안 듣는다.
    fn one_key(self) -> bool {
        matches!(self, Section::Menu | Section::Sorter | Section::Confirm)
    }

    /// 그 절의 이름이 가리키는 동작이 있는가.
    fn knows(self, action: &str) -> bool {
        fn any<A: Named>(table: &[Bind<A>], action: &str) -> bool {
            table.iter().any(|b| matches!(b.act.name(), Name::Own(n) if n == action))
        }
        match self {
            Section::Browse => any(BROWSE, action),
            Section::Moves => MOVES.iter().any(|(n, _)| *n == action),
            Section::Menu => any(MENU, action),
            Section::Sorter => any(SORTER, action),
            Section::Picker => any(PICK, action),
            Section::Stats => any(STATS, action),
            Section::Wiki => any(WIKI, action),
            Section::Links => any(LINKS, action),
            Section::Confirm => any(CONFIRM, action),
        }
    }
}

/// 이동의 이름 — `[moves]` 절의 키다.
const MOVES: [(&str, Move); 8] = [
    ("line_down", Move::LineDown),
    ("line_up", Move::LineUp),
    ("top", Move::Top),
    ("bottom", Move::Bottom),
    ("half_down", Move::HalfDown),
    ("half_up", Move::HalfUp),
    ("page_down", Move::PageDown),
    ("page_up", Move::PageUp),
];

fn move_name(m: Move) -> &'static str {
    MOVES.iter().find(|(_, x)| *x == m).map(|(n, _)| *n).expect("이동은 모두 이름이 있다")
}

/// 동작이 파일에서 불리는 이름.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Name {
    /// 그 표의 절에서 — `browse.quit`.
    Own(String),
    /// `[moves]` 절에서.
    Move(Move),
    /// 못 바꾼다 — 맨 숫자·칸 번호.
    Fixed,
}

/// 동작의 이름. **열거형 변종의 snake_case 다** — 매개변수가 든 변종은 이어 붙인다(`focus_left`·`go_board`·
/// `sort_add_title`·`cell_tags`). 이름은 파일에 적히는 자료라, 화면 낱말을 다듬는 날에도 안 바뀐다.
trait Named: Copy + PartialEq + std::fmt::Debug + 'static {
    fn name(self) -> Name;
}

fn own(s: &str) -> Name {
    Name::Own(s.to_string())
}

fn side(s: Side) -> &'static str {
    match s {
        Side::Left => "left",
        Side::Right => "right",
        Side::Up => "up",
        Side::Down => "down",
    }
}

/// 차례의 이름. **[`Order::name`] 을 안 쓴다** — 그쪽은 `[tui] sort` 에 적는 낱말이라 칸을 `status` 라 부른다.
/// 여기는 변종의 이름이다(`sort_column`, 설계 노트의 초안 그대로).
fn order(o: Order) -> &'static str {
    match o {
        Order::Priority => "priority",
        Order::Created => "created",
        Order::Updated => "updated",
        Order::Column => "column",
        Order::Assignee => "assignee",
        Order::Title => "title",
        Order::Due => "due",
    }
}

impl Named for Browse {
    fn name(self) -> Name {
        use Browse::*;
        match self {
            Step(m) => Name::Move(m),
            Column(_) | Project(_) => Name::Fixed,
            Quit => own("quit"),
            FocusNext => own("focus_next"),
            FocusPrev => own("focus_prev"),
            Focus(s) => Name::Own(format!("focus_{}", side(s))),
            Enter => own("enter"),
            Leave => own("leave"),
            Expand => own("expand"),
            Collapse => own("collapse"),
            ExpandAll => own("expand_all"),
            Grep => own("grep"),
            Filter => own("filter"),
            Jot => own("jot"),
            Pick => own("pick"),
            Unregister => own("unregister"),
            Go(s) => Name::Own(format!(
                "go_{}",
                match s {
                    Screen::List => "list",
                    Screen::Board => "board",
                    Screen::Stats => "stats",
                    Screen::Wiki => "wiki",
                }
            )),
            ClearFilter => own("clear_filter"),
            Worktree => own("worktree"),
            Raw => own("raw"),
            Deferred => own("deferred"),
            Backlog => own("backlog"),
            Archived => own("archived"),
            ShowAll => own("show_all"),
            Sort(o) => Name::Own(format!("sort_{}", order(o))),
            SortAdd(o) => Name::Own(format!("sort_add_{}", order(o))),
            SortEdit => own("sort_edit"),
            Cell(f) => Name::Own(format!("cell_{}", f.name())),
            Detail => own("detail"),
            DetailAt => own("detail_at"),
            Timezone => own("timezone"),
            Mouse => own("mouse"),
            Read => own("read"),
            ReadAll => own("read_all"),
            ReadGroup => own("read_group"),
        }
    }
}

impl Named for Menu {
    fn name(self) -> Name {
        match self {
            Menu::Close => own("close"),
            Menu::Up => own("up"),
        }
    }
}

impl Named for Sorter {
    fn name(self) -> Name {
        own(match self {
            Sorter::Down => "down",
            Sorter::Up => "up",
            Sorter::Lower => "lower",
            Sorter::Raise => "raise",
            Sorter::Toggle => "toggle",
            Sorter::Flip => "flip",
            Sorter::Apply => "apply",
            Sorter::Default => "default",
            Sorter::Close => "close",
        })
    }
}

impl Named for Pick {
    fn name(self) -> Name {
        match self {
            Pick::Step(m) => Name::Move(m),
            Pick::Enter => own("enter"),
            Pick::Up => own("up"),
            Pick::Register => own("register"),
            Pick::Hidden => own("hidden"),
            Pick::Path => own("path"),
            Pick::Close => own("close"),
        }
    }
}

impl Named for Stat {
    fn name(self) -> Name {
        match self {
            Stat::Step(m) => Name::Move(m),
            Stat::Bucket => own("bucket"),
            Stat::Close => own("close"),
        }
    }
}

impl Named for Wiki {
    fn name(self) -> Name {
        match self {
            Wiki::Step(m) => Name::Move(m),
            Wiki::Enter => own("enter"),
            Wiki::Back => own("back"),
            Wiki::Search => own("search"),
            Wiki::FocusNext => own("focus_next"),
            Wiki::Focus(s) => Name::Own(format!("focus_{}", side(s))),
            Wiki::NextLink => own("next_link"),
            Wiki::PrevLink => own("prev_link"),
            Wiki::Close => own("close"),
        }
    }
}

impl Named for Link {
    fn name(self) -> Name {
        match self {
            Link::Step(m) => Name::Move(m),
            Link::Enter => own("enter"),
            Link::Close => own("close"),
        }
    }
}

impl Named for Confirm {
    fn name(self) -> Name {
        match self {
            Confirm::Yes => own("yes"),
        }
    }
}

/// 파일의 한 줄 — 읽고 모양을 잰 것. 겹침은 아직 안 쟀다.
#[derive(Debug, Clone)]
struct Entry {
    section: Section,
    action: String,
    /// 키 열들. 비었으면 그 동작의 키를 뗀다.
    seqs: Vec<Vec<KeyEvent>>,
    line: Option<usize>,
}

impl Entry {
    fn at(&self) -> String {
        format!("{}.{}", self.section.name(), self.action)
    }

    /// 이 줄이 그 표의 동작을 바꾸는가.
    fn aims<A: Named>(&self, table: Section, act: A) -> bool {
        match act.name() {
            Name::Own(n) => self.section == table && self.action == n,
            Name::Move(m) => self.section == Section::Moves && self.action == move_name(m),
            Name::Fixed => false,
        }
    }
}

/// 파일 글 → 실행 중의 표와 버린 줄. **순수하다** — 파일은 부르는 쪽([`load`])이 읽어 건넨다.
pub fn build(text: &str) -> (Keymap, Vec<Problem>) {
    let (entries, mut problems) = read(text);
    let mut live = vec![true; entries.len()];
    loop {
        let t = Tables::apply(&entries, &live);
        let blamed = t.clashes();
        if blamed.is_empty() {
            problems.sort_by_key(|p| p.line);
            return (t.freeze(&entries, &live), problems);
        }
        // **한 바퀴에 적어도 한 줄을 버린다** — 탓은 늘 살아 있는 사람의 줄에만 가니(기본 줄은 `from` 이 없다)
        // 바퀴는 줄 수만큼을 못 넘는다.
        for (i, why) in blamed {
            if live[i] {
                live[i] = false;
                problems.push(Problem { at: entries[i].at(), line: entries[i].line, why });
            }
        }
    }
}

/// 글을 줄로 읽는다 — 모양이 틀린 줄은 여기서 버린다.
fn read(text: &str) -> (Vec<Entry>, Vec<Problem>) {
    let mut entries = Vec::new();
    let mut problems = Vec::new();
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = match toml_edit::Document::parse(body) {
        Ok(d) => d,
        Err(e) => {
            let line = e.span().map(|s| line_of(body, s.start));
            // 라이브러리의 오류는 여러 줄 그림이다. 한 줄로 접는다(`user_config::Doc::parse` 와 같다).
            let said = e.message().split_whitespace().collect::<Vec<_>>().join(" ");
            problems.push(Problem { at: String::new(), line, why: Why::Syntax(said) });
            return (entries, problems);
        }
    };
    let line = |key: &toml_edit::Key| key.span().map(|s| line_of(body, s.start));
    let root = doc.as_table();
    for (key, item) in root.iter().filter_map(|(k, _)| root.get_key_value(k)) {
        let Some(section) = Section::ALL.into_iter().find(|s| s.name() == key.get()) else {
            problems.push(Problem { at: key.get().to_string(), line: line(key), why: Why::UnknownSection });
            continue;
        };
        let Some(table) = item.as_table_like() else {
            problems.push(Problem { at: key.get().to_string(), line: line(key), why: Why::NotATable });
            continue;
        };
        for (name, _) in table.iter() {
            let Some((name, value)) = table.get_key_value(name) else { continue };
            let at = format!("{}.{}", section.name(), name.get());
            let mut bad = |why| problems.push(Problem { at: at.clone(), line: line(name), why });
            if !section.knows(name.get()) {
                bad(Why::UnknownAction);
                continue;
            }
            match seqs(value, section) {
                Ok(seqs) => entries.push(Entry { section, action: name.get().to_string(), seqs, line: line(name) }),
                Err(why) => bad(why),
            }
        }
    }
    (entries, problems)
}

/// 값 → 키 열들. `"none"` 과 `[]` 는 뗀다.
fn seqs(value: &toml_edit::Item, section: Section) -> Result<Vec<Vec<KeyEvent>>, Why> {
    let words: Vec<&str> = match value.as_value() {
        Some(toml_edit::Value::String(s)) if s.value() == "none" => return Ok(Vec::new()),
        Some(toml_edit::Value::String(s)) => vec![s.value().as_str()],
        Some(toml_edit::Value::Array(a)) => {
            a.iter().map(|v| v.as_str().ok_or(Why::NotKeys)).collect::<Result<_, _>>()?
        }
        _ => return Err(Why::NotKeys),
    };
    words
        .into_iter()
        .map(|w| {
            let seq = parse_seq(w.trim()).ok_or_else(|| Why::BadKey(w.to_string()))?;
            if section.one_key() && seq.len() > 1 {
                return Err(Why::OneKey(w.to_string()));
            }
            Ok(seq)
        })
        .collect()
}

fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].matches('\n').count() + 1
}

/// 짓는 중의 한 줄 — 어느 사람의 줄에서 왔는지(`from`)를 든다. 겹침의 탓이 그리로 간다.
#[derive(Debug, Clone)]
struct Row<A> {
    seq: Vec<Key>,
    act: A,
    label: Option<String>,
    from: Option<usize>,
}

impl<A: Named> Row<A> {
    fn pressed(&self) -> Vec<KeyEvent> {
        self.seq.iter().map(|k| k.event()).collect()
    }

    /// 겹침 글에 대는 이 줄의 이름 — 이름 있는 동작이면 `browse.grep`, 못 바꾸는 줄이면 그 키(`1`).
    fn who(&self, table: Section) -> String {
        match self.act.name() {
            Name::Own(n) => format!("{}.{n}", table.name()),
            Name::Move(m) => format!("moves.{}", move_name(m)),
            Name::Fixed => name_of_seq(&self.seq),
        }
    }
}

/// 기본 표 위에 살아 있는 줄을 덮어쓴다. **그 동작의 첫 기본 줄 자리에** 사람의 줄을 세운다 — 메뉴의 차례가 표의
/// 차례라([`super::menu::entries`]) 키를 바꿔도 메뉴의 자리는 그대로다. 이름은 기본 줄에 이름이 있던 동작에만 단다
/// (숨은 별칭뿐이던 동작은 바뀌어도 숨는다 — 바의 몫이 그대로다).
///
/// `plain` 은 그 표가 수식키 없는 비글자 키를 받는 꼴이다([`Key::written`]). 기본 표에 같은 키가 있으면 그 꼴을 그대로
/// 쓴다 — 기본 키를 파일에 옮겨 적으면 아무것도 안 바뀐다.
fn apply<A: Named>(
    defaults: &'static [Bind<A>],
    table: Section,
    plain: fn(KeyCode) -> Key,
    entries: &[Entry],
    live: &[bool],
) -> Vec<Row<A>> {
    let key = |ev: KeyEvent| {
        let w = Key::written(ev, plain);
        defaults.iter().flat_map(|b| b.seq.iter()).copied().find(|k| k.event() == w.event()).unwrap_or(w)
    };
    let mut out = Vec::new();
    let mut done = vec![false; entries.len()];
    for b in defaults {
        let Some(i) = (0..entries.len()).find(|&i| live[i] && entries[i].aims(table, b.act)) else {
            out.push(Row { seq: b.seq.to_vec(), act: b.act, label: b.label.map(String::from), from: None });
            continue;
        };
        if std::mem::replace(&mut done[i], true) {
            continue;
        }
        let named = defaults.iter().any(|d| d.act == b.act && d.label.is_some());
        for (j, seq) in entries[i].seqs.iter().enumerate() {
            let seq: Vec<Key> = seq.iter().map(|ev| key(*ev)).collect();
            let label = (named && j == 0).then(|| name_of_seq(&seq));
            out.push(Row { seq, act: b.act, label, from: Some(i) });
        }
    }
    out
}

/// 표 안의 겹침 — `no_row_is_shadowed_or_a_prefix_of_another` 가 기본 표에 재던 것을 런타임으로. 줄마다 그 키를 눌러
/// 그 줄의 동작이 나오는가를 본다: 같은 열의 다른 동작, 그 열로 시작하는 더 긴 줄이 그것을 가린다.
fn shadows<A: Named>(rows: &[Row<A>], table: Section, out: &mut Vec<(usize, Why)>) {
    for (i, r) in rows.iter().enumerate() {
        let seq = r.pressed();
        for (j, h) in rows.iter().enumerate() {
            let hits = j != i
                && h.seq.len() >= seq.len()
                && h.seq.iter().zip(&seq).all(|(k, e)| k.matches(*e))
                && (h.seq.len() > seq.len() || h.act != r.act);
            if hits {
                blame(r, h, table, table, out);
            }
        }
    }
}

/// 두 줄이 겹쳤다 — 둘 중 사람의 줄에 탓을 단다.
fn blame<A: Named, B: Named>(a: &Row<A>, b: &Row<B>, at: Section, bt: Section, out: &mut Vec<(usize, Why)>) {
    if let Some(i) = a.from {
        out.push((i, Why::Clash { keys: name_of_seq(&a.seq), with: b.who(bt) }));
    }
    if let Some(i) = b.from {
        out.push((i, Why::Clash { keys: name_of_seq(&b.seq), with: a.who(at) }));
    }
}

/// 사람의 줄에서 그 키 중 하나라도 `dead` 면 탓을 단다 — 표 밖의 것이 먼저 받는 키다(Ctrl-C·SPC).
fn beaten<A: Named>(rows: &[Row<A>], dead: impl Fn(KeyEvent) -> bool, with: &str, out: &mut Vec<(usize, Why)>) {
    for r in rows.iter().filter(|r| r.from.is_some()) {
        if r.pressed().iter().any(|e| dead(*e)) {
            out.push((r.from.expect("사람의 줄"), Why::Clash { keys: name_of_seq(&r.seq), with: with.to_string() }));
        }
    }
}

/// Ctrl-C — 어느 모드에서든 표보다 먼저 받는다(`App::key`).
fn ctrl_c(e: KeyEvent) -> bool {
    matches!(lookup(ANYWHERE, &[e]), Lookup::Run(_))
}

/// 파일이 안 겨눈 표는 기본 표 그대로 낸다 — 새로 짓지도 새지도 않는다. **겨눴는가는 남은 줄이 아니라 파일의 줄로 가른다**:
/// 키를 떼기만 한 줄(`"none"`)은 사람의 줄을 하나도 안 남긴다.
fn freeze<A: Copy + 'static>(rows: Vec<Row<A>>, touched: bool, default: &'static [Bind<A>]) -> &'static [Bind<A>] {
    if !touched {
        return default;
    }
    let binds: Vec<Bind<A>> = rows
        .into_iter()
        .map(|r| Bind {
            seq: Box::leak(r.seq.into_boxed_slice()),
            act: r.act,
            label: r.label.map(|l| &*Box::leak(l.into_boxed_str())),
        })
        .collect();
    Box::leak(binds.into_boxed_slice())
}

/// 짓는 중의 표 전부.
struct Tables {
    browse: Vec<Row<Browse>>,
    menu: Vec<Row<Menu>>,
    sorter: Vec<Row<Sorter>>,
    pick: Vec<Row<Pick>>,
    stats: Vec<Row<Stat>>,
    wiki: Vec<Row<Wiki>>,
    links: Vec<Row<Link>>,
    confirm: Vec<Row<Confirm>>,
}

impl Tables {
    /// 표마다 비글자 키의 꼴은 그 표의 이동 줄(`moves!` 의 둘째 인자)과 같다.
    fn apply(e: &[Entry], live: &[bool]) -> Tables {
        Tables {
            browse: apply(BROWSE, Section::Browse, Key::any, e, live),
            menu: apply(MENU, Section::Menu, Key::any, e, live),
            sorter: apply(SORTER, Section::Sorter, Key::bare, e, live),
            pick: apply(PICK, Section::Picker, Key::bare, e, live),
            stats: apply(STATS, Section::Stats, Key::any, e, live),
            wiki: apply(WIKI, Section::Wiki, Key::any, e, live),
            links: apply(LINKS, Section::Links, Key::bare, e, live),
            confirm: apply(CONFIRM, Section::Confirm, Key::bare, e, live),
        }
    }

    /// 겹친 사람의 줄과 그 까닭. 같은 줄이 여러 번 나와도 된다 — 부르는 쪽이 처음 것만 센다.
    fn clashes(&self) -> Vec<(usize, Why)> {
        let mut out = Vec::new();
        shadows(&self.browse, Section::Browse, &mut out);
        shadows(&self.menu, Section::Menu, &mut out);
        shadows(&self.sorter, Section::Sorter, &mut out);
        shadows(&self.pick, Section::Picker, &mut out);
        shadows(&self.stats, Section::Stats, &mut out);
        shadows(&self.wiki, Section::Wiki, &mut out);
        shadows(&self.links, Section::Links, &mut out);
        shadows(&self.confirm, Section::Confirm, &mut out);
        // Ctrl-C 는 어느 표보다 먼저다 — 그 키를 쓴 줄은 영영 안 듣는다.
        beaten(&self.browse, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.menu, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.sorter, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.pick, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.stats, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.wiki, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.links, ctrl_c, "Ctrl-C", &mut out);
        beaten(&self.confirm, ctrl_c, "Ctrl-C", &mut out);
        let spc = |e: KeyEvent| LEADER.matches(e);
        // 통계·위키 창은 SPC 를 표보다 먼저 메뉴로 받는다(`App::stats_key`·`App::wiki_key`). 열린 메뉴에서 SPC 는 닫는
        // 키다(`menu::feed`) — 메뉴의 줄에도 못 준다.
        beaten(&self.stats, spc, "SPC", &mut out);
        beaten(&self.wiki, spc, "SPC", &mut out);
        beaten(&self.menu, spc, "SPC", &mut out);
        // **SPC 밑의 줄은 메뉴의 규칙을 지난다**(`menu::feed`) — 메뉴가 열린 뒤의 키가 SPC 면 메뉴가 닫히고, 메뉴의
        // 키(Esc·Bksp)면 닫히거나 한 층 오른다. 그 키를 쓴 SPC 밑 줄은 못 누른다. 기본 표에는
        // `menu::tests` 의 시험이 재던 것이다.
        for r in self.browse.iter().filter(|r| r.seq.first().is_some_and(|k| spc(k.event()))) {
            for e in &r.pressed()[1..] {
                if spc(*e)
                    && let Some(i) = r.from
                {
                    out.push((i, Why::Clash { keys: name_of_seq(&r.seq), with: "SPC".into() }));
                }
                for m in self.menu.iter().filter(|m| m.seq.len() == 1 && m.seq[0].matches(*e)) {
                    blame(r, m, Section::Browse, Section::Menu, &mut out);
                }
            }
        }
        out
    }

    fn freeze(self, e: &[Entry], live: &[bool]) -> Keymap {
        fn aimed<A: Named>(table: &[Bind<A>], at: Section, e: &[Entry], live: &[bool]) -> bool {
            table.iter().any(|b| e.iter().zip(live).any(|(e, l)| *l && e.aims(at, b.act)))
        }
        Keymap {
            browse: freeze(self.browse, aimed(BROWSE, Section::Browse, e, live), BROWSE),
            menu: freeze(self.menu, aimed(MENU, Section::Menu, e, live), MENU),
            sorter: freeze(self.sorter, aimed(SORTER, Section::Sorter, e, live), SORTER),
            pick: freeze(self.pick, aimed(PICK, Section::Picker, e, live), PICK),
            stats: freeze(self.stats, aimed(STATS, Section::Stats, e, live), STATS),
            wiki: freeze(self.wiki, aimed(WIKI, Section::Wiki, e, live), WIKI),
            links: freeze(self.links, aimed(LINKS, Section::Links, e, live), LINKS),
            confirm: freeze(self.confirm, aimed(CONFIRM, Section::Confirm, e, live), CONFIRM),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::keys::{label, name_of_key, parse};
    use super::*;
    use ratatui::crossterm::event::KeyModifiers as M;

    fn press(code: KeyCode, m: M) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    fn ch(c: char) -> KeyEvent {
        press(KeyCode::Char(c), M::NONE)
    }

    fn seq(name: &str) -> Vec<KeyEvent> {
        parse_seq(name).unwrap_or_else(|| panic!("`{name}` 를 키로 못 푼다"))
    }

    fn run<A: Copy>(table: &[Bind<A>], name: &str) -> Lookup<A> {
        lookup(table, &seq(name))
    }

    /// 탓의 자리와 까닭만 — 줄 번호는 따로 잰다.
    fn whys(p: &[Problem]) -> Vec<(&str, &Why)> {
        p.iter().map(|p| (p.at.as_str(), &p.why)).collect()
    }

    fn clash(keys: &str, with: &str) -> Why {
        Why::Clash { keys: keys.into(), with: with.into() }
    }

    /// 표 하나의 이름 있는 동작 — `(이름, 동작)`, 표의 차례로 한 번씩.
    fn names<A: Named>(table: &[Bind<A>]) -> Vec<(Name, A)> {
        let mut out: Vec<(Name, A)> = Vec::new();
        for b in table {
            let n = b.act.name();
            if n != Name::Fixed && !out.iter().any(|(_, a)| *a == b.act) {
                out.push((n, b.act));
            }
        }
        out
    }

    /// **이름은 한 절 안에서 한 동작이다** — 둘이 한 이름을 쓰면 파일의 한 줄이 어느 쪽을 바꾸는지 못 가린다. 셈은
    /// 사용자에게 보인 초안(moai-l8qb 노트)의 99 다.
    #[test]
    fn every_action_has_one_name_and_the_count_is_the_draft() {
        fn each<A: Named>(table: &[Bind<A>]) -> usize {
            let own: Vec<String> = names(table)
                .into_iter()
                .filter_map(|(n, _)| match n {
                    Name::Own(s) => Some(s),
                    _ => None,
                })
                .collect();
            let mut seen = own.clone();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), own.len(), "한 이름을 두 동작이 쓴다: {own:?}");
            own.len()
        }
        let counts = [
            each(BROWSE),
            MOVES.len(),
            each(MENU),
            each(SORTER),
            each(PICK),
            each(STATS),
            each(WIKI),
            each(LINKS),
            each(CONFIRM),
        ];
        assert_eq!(counts, [60, 8, 2, 9, 6, 2, 9, 2, 1]);
        // 이동은 다섯 표 모두에서 여덟이 다 선다 — `[moves]` 의 한 줄이 어느 표에서는 없는 동작이면 안 된다.
        fn moves<A: Named>(table: &[Bind<A>]) -> usize {
            names(table).iter().filter(|(n, _)| matches!(n, Name::Move(_))).count()
        }
        assert_eq!([moves(BROWSE), moves(PICK), moves(STATS), moves(WIKI), moves(LINKS)], [8; 5]);
    }

    /// 기본 표를 그대로 옮겨 적은 파일 — 이름 있는 동작마다 그 기본 키 전부.
    fn the_defaults_written_out() -> String {
        fn each<A: Named>(out: &mut String, section: &str, table: &[Bind<A>]) {
            out.push_str(&format!("[{section}]\n"));
            for (n, act) in names(table) {
                let Name::Own(n) = n else { continue };
                let keys: Vec<String> =
                    table.iter().filter(|b| b.act == act).map(|b| format!("{:?}", name_of_seq(b.seq))).collect();
                out.push_str(&format!("{n} = [{}]\n", keys.join(", ")));
            }
        }
        let mut out = String::from("[moves]\n");
        for (n, m) in MOVES {
            let keys: Vec<String> = BROWSE
                .iter()
                .filter(|b| b.act == Browse::Step(m))
                .map(|b| format!("{:?}", name_of_seq(b.seq)))
                .collect();
            out.push_str(&format!("{n} = [{}]\n", keys.join(", ")));
        }
        each(&mut out, "browse", BROWSE);
        each(&mut out, "menu", MENU);
        each(&mut out, "sorter", SORTER);
        each(&mut out, "picker", PICK);
        each(&mut out, "stats", STATS);
        each(&mut out, "wiki", WIKI);
        each(&mut out, "links", LINKS);
        each(&mut out, "confirm", CONFIRM);
        out
    }

    /// **기본 키를 파일에 옮겨 적으면 아무것도 안 바뀐다** — 동작마다 같은 키(같은 꼴로 견주는 [`Key`])와 같은
    /// 이름이 선다. 이름을 읽는 자([`parse_seq`])·적는 자([`name_of_seq`])·꼴을 고르는 자([`Key::written`])가
    /// 한 바퀴를 돈다. 겹침 검사가 기본 표를 잘못 잡아도 여기서 갈린다.
    #[test]
    fn writing_the_defaults_back_changes_nothing() {
        fn same<A: Named>(name: &str, built: &[Bind<A>], default: &[Bind<A>]) {
            for b in default {
                let mine = |t: &[Bind<A>]| -> Vec<(Vec<Key>, Option<String>)> {
                    t.iter().filter(|x| x.act == b.act).map(|x| (x.seq.to_vec(), x.label.map(String::from))).collect()
                };
                assert_eq!(mine(built), mine(default), "{name}: {:?}", b.act);
            }
            assert_eq!(built.len(), default.len(), "{name}");
        }
        let (map, problems) = build(&the_defaults_written_out());
        assert_eq!(problems, Vec::new());
        same("browse", map.browse, BROWSE);
        same("menu", map.menu, MENU);
        same("sorter", map.sorter, SORTER);
        same("picker", map.pick, PICK);
        same("stats", map.stats, STATS);
        same("wiki", map.wiki, WIKI);
        same("links", map.links, LINKS);
        same("confirm", map.confirm, CONFIRM);
    }

    /// 빈 파일은 기본 표 그대로다 — 새로 짓지도 않는다.
    #[test]
    fn an_empty_file_is_the_defaults() {
        let (map, problems) = build("# 아직 아무것도 안 바꿨다\n");
        assert!(problems.is_empty());
        assert_eq!(map, Keymap::default());
    }

    /// **바꾼 줄이 그 동작의 줄을 모두 갈아 끼운다** — 이름도 따라온다. 안 바꾼 동작은 그대로다.
    #[test]
    fn a_line_replaces_every_row_of_its_action() {
        let (map, problems) = build("[browse]\nquit = \"SPC x\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(run(map.browse, "SPC x"), Lookup::Run(Browse::Quit));
        assert_eq!(run(map.browse, "SPC q"), Lookup::Unknown);
        assert_eq!(label(map.browse, Browse::Quit), "SPC x");
        assert_eq!(run(map.browse, "SPC f"), Lookup::Run(Browse::Filter));
        // 안 바꾼 표는 기본 표 그대로다.
        assert_eq!(map.pick, PICK);
        // 별칭까지 간다 — `/` 하나만 적으면 메뉴의 `SPC /` 도 떨어진다.
        let (map, _) = build("[browse]\ngrep = \"?\"\n");
        assert_eq!(run(map.browse, "?"), Lookup::Run(Browse::Grep));
        assert_eq!(run(map.browse, "SPC /"), Lookup::Unknown);
    }

    /// **배열은 첫째가 이름이고 나머지는 숨은 별칭이다.** 메뉴의 차례도 그대로다 — 바꾼 줄은 그 동작의 첫 기본 줄 자리에 선다.
    #[test]
    fn the_first_of_several_keys_is_the_name() {
        let (map, problems) = build("[browse]\nfilter = [\"SPC F\", \"\\\\\"]\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(label(map.browse, Browse::Filter), "SPC F");
        assert_eq!(run(map.browse, "\\"), Lookup::Run(Browse::Filter));
        let at = |t: &[Bind<Browse>], a: Browse| t.iter().position(|b| b.act == a).unwrap();
        assert_eq!(at(map.browse, Browse::Filter), at(BROWSE, Browse::Filter));
        // 숨은 별칭뿐이던 동작은 바뀌어도 숨는다 — 바의 몫이 안 는다.
        let (map, _) = build("[browse]\nexpand = \"L\"\n");
        assert_eq!(label(map.browse, Browse::Expand), "");
        assert_eq!(run(map.browse, "L"), Lookup::Run(Browse::Expand));
    }

    /// `"none"` 과 `[]` 는 그 동작의 키를 뗀다.
    #[test]
    fn none_and_an_empty_list_unbind() {
        let (map, problems) = build("[browse]\njot = \"none\"\nfilter = []\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(run(map.browse, "SPC n"), Lookup::Unknown);
        assert_eq!(run(map.browse, "SPC f"), Lookup::Unknown);
        assert_eq!(label(map.browse, Browse::Jot), "");
        assert!(!map.browse.iter().any(|b| b.act == Browse::Filter));
    }

    /// **두 키를 맞바꾸는 파일은 통과한다** — 한 줄씩 보면 겹치지만 다 쓰고 나면 안 겹친다. 그래서 겹침은 다 덮어쓴
    /// 뒤에 잰다.
    #[test]
    fn swapping_two_keys_passes() {
        let (map, problems) = build("[browse]\nquit = \"r\"\nread = \"SPC q\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(run(map.browse, "r"), Lookup::Run(Browse::Quit));
        assert_eq!(run(map.browse, "SPC q"), Lookup::Run(Browse::Read));
    }

    /// **기본 키와 겹친 사람의 줄은 버린다** — 그 동작은 기본 키로 남고, 겹친 쪽을 댄다.
    #[test]
    fn a_line_that_clashes_with_a_default_is_dropped() {
        let (map, problems) = build("[browse]\ngrep = \"r\"\n");
        assert_eq!(whys(&problems), [("browse.grep", &clash("r", "browse.read"))]);
        assert_eq!(run(map.browse, "/"), Lookup::Run(Browse::Grep));
        assert_eq!(run(map.browse, "r"), Lookup::Run(Browse::Read));
        // 접두어도 겹침이다 — `g` 는 `gg` 를 기다리게 해서 영영 안 듣는다.
        let (map, problems) = build("[browse]\nquit = \"g\"\n");
        assert_eq!(whys(&problems), [("browse.quit", &clash("g", "moves.top"))]);
        assert_eq!(run(map.browse, "gg"), Lookup::Run(Browse::Step(Move::Top)));
        // 거꾸로, 긴 줄이 짧은 기본 줄 뒤에 서도 겹침이다.
        let (_, problems) = build("[browse]\nquit = \"r r\"\n");
        // 이름은 표의 철자로 댄다 — 같은 맨 글자 둘은 붙인다(`gg`).
        assert_eq!(whys(&problems), [("browse.quit", &clash("rr", "browse.read"))]);
        // 못 바꾸는 줄(맨 숫자)과 겹치면 그 키를 댄다.
        let (_, problems) = build("[browse]\nquit = \"1\"\n");
        assert_eq!(whys(&problems), [("browse.quit", &clash("1", "1"))]);
    }

    /// **사람의 줄 둘이 겹치면 둘 다 버린다** — 어느 쪽이 맞는지 도구가 고르지 않는다.
    #[test]
    fn two_lines_that_clash_are_both_dropped() {
        let (map, problems) = build("[browse]\nquit = \"x\"\njot = \"x\"\n");
        assert_eq!(
            whys(&problems),
            [("browse.quit", &clash("x", "browse.jot")), ("browse.jot", &clash("x", "browse.quit"))]
        );
        assert_eq!(run(map.browse, "SPC q"), Lookup::Run(Browse::Quit));
        assert_eq!(run(map.browse, "SPC n"), Lookup::Run(Browse::Jot));
    }

    /// **버린 줄이 기본 키로 돌아와 다른 사람의 줄과 겹치면 그 줄도 버린다** — 다시 잰다.
    #[test]
    fn a_line_restored_to_its_default_is_measured_again() {
        // quit 이 `1` 로 가려다 버려져 `SPC q` 로 돌아오고, 그 자리를 잡은 jot 이 그때 버려진다.
        let (map, problems) = build("[browse]\nquit = \"1\"\njot = \"SPC q\"\n");
        assert_eq!(
            whys(&problems),
            [("browse.quit", &clash("1", "1")), ("browse.jot", &clash("SPC q", "browse.quit"))]
        );
        assert_eq!(run(map.browse, "SPC q"), Lookup::Run(Browse::Quit));
        assert_eq!(run(map.browse, "SPC n"), Lookup::Run(Browse::Jot));
    }

    /// **이동은 다섯 표가 함께 쓴다** — `[moves]` 의 한 줄이 탐색·고르기·통계·위키·링크 모두를 바꾼다.
    #[test]
    fn a_move_changes_all_five_tables() {
        let (map, problems) = build("[moves]\nline_down = [\"n\", \"Down\"]\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(run(map.browse, "n"), Lookup::Run(Browse::Step(Move::LineDown)));
        assert_eq!(run(map.pick, "n"), Lookup::Run(Pick::Step(Move::LineDown)));
        assert_eq!(run(map.stats, "n"), Lookup::Run(Stat::Step(Move::LineDown)));
        assert_eq!(run(map.wiki, "n"), Lookup::Run(Wiki::Step(Move::LineDown)));
        assert_eq!(run(map.links, "n"), Lookup::Run(Link::Step(Move::LineDown)));
        assert_eq!(run(map.browse, "j"), Lookup::Unknown);
        assert_eq!(label(map.pick, Pick::Step(Move::LineDown)), "n");
        // **비글자 키는 그 표의 꼴을 따른다** — 탐색은 수식키를 안 보고(`Key::any`), 고르기 창은 거른다(`Key::bare`).
        let ctrl_down = press(KeyCode::Down, M::CONTROL);
        assert_eq!(lookup(map.browse, &[ctrl_down]), Lookup::Run(Browse::Step(Move::LineDown)));
        assert_eq!(lookup(map.pick, &[ctrl_down]), Lookup::Unknown);
    }

    /// **이동 줄이 한 표에서 겹치면 다섯 표 모두에서 버린다** — 표마다 갈리면 "함께 쓴다" 가 무너진다.
    #[test]
    fn a_move_that_clashes_in_one_table_is_dropped_everywhere() {
        let (map, problems) = build("[moves]\nline_down = \"b\"\n");
        assert_eq!(whys(&problems), [("moves.line_down", &clash("b", "stats.bucket"))]);
        assert_eq!(run(map.browse, "j"), Lookup::Run(Browse::Step(Move::LineDown)));
        assert_eq!(run(map.browse, "b"), Lookup::Unknown);
        assert_eq!(map.browse, BROWSE);
    }

    /// **Ctrl-C 는 못 준다** — 어느 모드에서든 표보다 먼저 받는 탈출구다.
    #[test]
    fn ctrl_c_cannot_be_given() {
        let (map, problems) = build("[browse]\nquit = \"Ctrl-c\"\n[picker]\nclose = \"Ctrl-C\"\n");
        assert_eq!(
            whys(&problems),
            [("browse.quit", &clash("Ctrl-c", "Ctrl-C")), ("picker.close", &clash("Ctrl-c", "Ctrl-C"))]
        );
        assert_eq!(run(map.browse, "SPC q"), Lookup::Run(Browse::Quit));
    }

    /// **SPC 밑의 줄은 메뉴의 키를 못 쓴다** — 메뉴가 열린 뒤의 Esc·Bksp·SPC 는 메뉴가 먼저 받는다(`menu::feed`). 메뉴의
    /// 키를 그 밑 줄의 글자로 바꿔도 같다. 통계·위키 창은 SPC 를 메뉴로 받는다.
    #[test]
    fn rows_under_spc_cannot_use_the_menu_keys() {
        let (_, problems) = build("[browse]\nfilter = \"SPC Esc\"\njot = \"SPC SPC\"\n");
        assert_eq!(
            whys(&problems),
            [("browse.filter", &clash("SPC Esc", "menu.close")), ("browse.jot", &clash("SPC SPC", "SPC"))]
        );
        let (map, problems) = build("[menu]\nclose = \"q\"\n");
        assert_eq!(whys(&problems), [("menu.close", &clash("q", "browse.quit"))]);
        assert_eq!(lookup(map.menu, &[press(KeyCode::Esc, M::NONE)]), Lookup::Run(Menu::Close));
        let (_, problems) = build("[stats]\nbucket = \"SPC\"\n[wiki]\nsearch = \"g SPC\"\n[menu]\nup = \"SPC\"\n");
        assert_eq!(
            whys(&problems),
            [
                ("stats.bucket", &clash("SPC", "SPC")),
                ("wiki.search", &clash("g SPC", "SPC")),
                ("menu.up", &clash("SPC", "SPC"))
            ]
        );
        // 메뉴를 다른 키로 바꾸는 것은 된다 — SPC 밑 줄이 그 글자를 안 쓰면.
        let (map, problems) = build("[menu]\nclose = \"Ctrl-g\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(lookup(map.menu, &[press(KeyCode::Char('g'), M::CONTROL)]), Lookup::Run(Menu::Close));
    }

    /// **키를 하나씩만 찾는 창에는 키 열을 못 준다** — 메뉴·차례 편집 창·물음.
    #[test]
    fn windows_that_read_one_key_refuse_a_sequence() {
        let (map, problems) =
            build("[menu]\nclose = \"g g\"\n[sorter]\ndown = \"SPC j\"\n[confirm]\nyes = [\"y\", \"o k\"]\n");
        assert_eq!(
            whys(&problems),
            [
                ("menu.close", &Why::OneKey("g g".into())),
                ("sorter.down", &Why::OneKey("SPC j".into())),
                ("confirm.yes", &Why::OneKey("o k".into()))
            ]
        );
        assert_eq!(map, Keymap::default());
    }

    /// 모양이 틀린 줄은 그 줄만 버린다 — 옆 줄은 산다.
    #[test]
    fn a_malformed_line_is_dropped_alone() {
        let text = "[browse]\nquit = \"SPC x\"\nbad = \"x\"\nproject_1 = \"!\"\ngrep = 3\njot = \"enter\"\nfilter = [\"SPC F\", 1]\n\
                    [nope]\nx = \"y\"\n";
        let (map, problems) = build(text);
        assert_eq!(
            whys(&problems),
            [
                ("browse.bad", &Why::UnknownAction),
                ("browse.project_1", &Why::UnknownAction),
                ("browse.grep", &Why::NotKeys),
                ("browse.jot", &Why::BadKey("enter".into())),
                ("browse.filter", &Why::NotKeys),
                ("nope", &Why::UnknownSection),
            ]
        );
        assert_eq!(run(map.browse, "SPC x"), Lookup::Run(Browse::Quit));
        assert_eq!(run(map.browse, "SPC n"), Lookup::Run(Browse::Jot));
        // 절 자리에 표가 아닌 것.
        let (_, problems) = build("browse = 1\n");
        assert_eq!(whys(&problems), [("browse", &Why::NotATable)]);
        // 이동은 그 절에서만 — 탐색의 절에 적으면 모르는 키다.
        let (_, problems) = build("[browse]\nline_down = \"n\"\n");
        assert_eq!(whys(&problems), [("browse.line_down", &Why::UnknownAction)]);
    }

    /// **줄 번호를 댄다** — 사람이 파일에서 그 줄을 찾는다. 절의 탈은 그 머리줄이다.
    #[test]
    fn a_problem_names_its_line() {
        let (_, problems) = build("# 내 키\n[browse]\nquit = \"SPC x\"\nbad = \"x\"\n\n[nope]\n");
        let lines: Vec<(&str, Option<usize>)> = problems.iter().map(|p| (p.at.as_str(), p.line)).collect();
        assert_eq!(lines, [("browse.bad", Some(4)), ("nope", Some(6))]);
        // 겹친 줄도 제 줄을 댄다.
        let (_, problems) = build("[browse]\n\ngrep = \"r\"\n");
        assert_eq!(problems[0].line, Some(3));
    }

    /// **문법이 깨진 파일은 통째로 기본 표다** — 어느 줄을 버릴지 못 가린다. 깨진 줄은 댄다.
    #[test]
    fn a_broken_file_is_the_defaults_with_one_problem() {
        let (map, problems) = build("[browse]\nquit = \"SPC x\"\njot = \n");
        assert_eq!(map, Keymap::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(matches!(problems[0].why, Why::Syntax(_)), "{problems:?}");
        assert_eq!(problems[0].line, Some(3));
        // 같은 키를 두 번 적은 것도 TOML 의 문법 오류다.
        let (map, problems) = build("[browse]\nquit = \"SPC x\"\nquit = \"SPC y\"\n");
        assert_eq!(map, Keymap::default());
        assert!(matches!(problems[..], [Problem { why: Why::Syntax(_), .. }]), "{problems:?}");
    }

    /// **Alt 가 왕복한다** — [`name_of_key`] 가 적은 이름을 [`parse`] 가 다시 읽는다. 파일의 `Alt-q` 는 Alt 를 정확히 견준다.
    #[test]
    fn alt_round_trips_and_matches_exactly() {
        for (name, code, m) in [
            ("Alt-x", KeyCode::Char('x'), M::ALT),
            ("Ctrl-Alt-x", KeyCode::Char('x'), M::CONTROL | M::ALT),
            ("Ctrl-w", KeyCode::Char('w'), M::CONTROL),
            ("Shift-Tab", KeyCode::Tab, M::SHIFT),
            ("Ctrl-Down", KeyCode::Down, M::CONTROL),
            ("F5", KeyCode::F(5), M::NONE),
        ] {
            assert_eq!(parse(name), Some(press(code, m)), "{name}");
            assert_eq!(name_of_key(press(code, m)), name);
        }
        assert_eq!(name_of_key(press(KeyCode::BackTab, M::NONE)), "Shift-Tab");
        // 글자에 Shift- 는 키가 아니다 — 대문자를 적는다. 표의 철자가 아니면 키가 아니다.
        for bad in ["Shift-a", "enter", "Ctrl-", "Hyper-x", "xy", ""] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        let (map, problems) = build("[browse]\nquit = \"Alt-q\"\n");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(lookup(map.browse, &[press(KeyCode::Char('q'), M::ALT)]), Lookup::Run(Browse::Quit));
        assert_eq!(lookup(map.browse, &[press(KeyCode::Char('q'), M::CONTROL | M::ALT)]), Lookup::Unknown);
        assert_eq!(lookup(map.browse, &[ch('q')]), Lookup::Unknown);
        assert_eq!(label(map.browse, Browse::Quit), "Alt-q");
    }

    /// 파일의 자리 — `MOAI_KEYS` 가 이기고, 없으면 사용자 설정 곁이다.
    #[test]
    fn the_file_sits_beside_the_user_config() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
        };
        assert_eq!(
            path_from(env(&[("MOAI_KEYS", "/k/my.toml"), ("MOAI_CONFIG", "/t/c.toml")])),
            Some("/k/my.toml".into())
        );
        // 가름선은 그 기계의 것이다 — `with_file_name` 이 잇는 조각을 같은 길로 잇는다.
        assert_eq!(
            path_from(env(&[("MOAI_KEYS", ""), ("MOAI_CONFIG", "/t/c.toml")])),
            Some(Path::new("/t").join("keys.toml"))
        );
    }

    /// 없는 파일은 알릴 것이 없다 — 기본 표다.
    #[test]
    fn a_missing_file_is_the_defaults_without_a_word() {
        let (map, problems) = load(Some(Path::new("/nonexistent/moai/keys.toml")));
        assert_eq!(map, Keymap::default());
        assert!(problems.is_empty());
        assert_eq!(load(None).0, Keymap::default());
    }
}
