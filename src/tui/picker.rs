//! 디렉터리 고르기 창(moai-plvy) — 프로젝트 층의 `a` 가 연다.
//!
//! **조각이다.** 디렉터리를 읽지 않는다 — 읽어 온 한 층([`Listing`])을 받아 줄로 세우고,
//! 키를 받아 **무엇을 할지만** 돌려준다([`Act`]). 읽기(`register::list_dir`)와 쓰기
//! (`projects::add`)는 든 쪽(`App`)이 한다. 여기서 파일 시스템을 알면 키 시험이 임시
//! 디렉터리 없이 못 돌고, 창이 무엇을 보여 줄지와 디스크가 어떤지가 한 함수에 엉킨다.
//!
//! **한 층씩이다.** 들어갈 때마다 든 쪽이 그 한 층만 읽어 [`Picker::show`] 로 넣는다 —
//! 미리 훑어 두지 않는다. 홈 밑을 통째로 훑으면 첫 화면이 디스크 크기만큼 늦고, 사람이
//! 보지도 않을 디렉터리 이름까지 읽는다.

use super::input::Input;
use super::scroll::Scroll;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};

/// 하위 디렉터리 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dent {
    pub name: String,
    /// 그 안에 `.moai` 가 있나. **없어도 고를 수 있다** — 등록한 뒤 `moai init` 하면 보인다.
    pub moai: bool,
    /// 이미 등록돼 있나(링크를 푼 경로로도 견준 것).
    pub registered: bool,
}

/// 든 쪽이 읽어 온 한 층.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// 링크를 푼 절대경로 — 등록하면 적힐 철자와 같다.
    pub dir: PathBuf,
    /// 하위 디렉터리, 이름순. 상한까지만 든다.
    pub entries: Vec<Dent>,
    /// 상한에 걸려 안 세운 수. 0 이 아니면 창이 그렇다고 말한다 — 조용히 자르면 거기 없는 줄 안다.
    pub cut: usize,
    /// 감춰 둔 점 디렉터리의 수. 보이기를 켰으면 0 이다.
    pub hidden: usize,
    /// 지금 디렉터리에 `.moai` 가 있나.
    pub moai: bool,
    /// 지금 디렉터리가 이미 등록돼 있나.
    pub registered: bool,
}

/// 창의 한 줄.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// 지금 디렉터리 그 자체(`./`). 여기서 `a` 면 지금 디렉터리를 등록한다 — 모노레포에 들어와
    /// 그 뿌리를 등록하려고 한 층 올라가 다시 고르게 하지 않는다.
    Here,
    /// 한 층 위로. 뿌리(`/`)에는 없다.
    Up,
    /// `entries` 의 첨자.
    Dir(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    pub at: Listing,
    pub cursor: usize,
    /// 목록의 굴린 자리 — 탐색기의 목록과 같은 조각이다.
    pub list: Scroll,
    /// 점 디렉터리도 보이나. **꺼진 채 연다** — 홈에서 열면 `.cache`·`.local` 이 줄을 먼저 먹는다.
    pub show_hidden: bool,
    /// `g` 로 연 경로 적기 칸.
    pub typing: Option<Input>,
    /// 마지막 키가 못 한 까닭(못 읽는 디렉터리·거절된 등록). **다음 키에 걷힌다.**
    pub error: Option<String>,
}

/// 창이 든 쪽에 시키는 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Stay,
    /// 이 디렉터리를 읽어 [`Picker::show`] 로 넣어 달라. 지금 디렉터리를 다시 읽는 것도 이것이다.
    Go(PathBuf),
    /// 이 디렉터리를 등록해 달라.
    Register(PathBuf),
    /// 닫는다. 적던 것이 없으니 묻지 않는다.
    Close,
}

/// `..` 에서 `a` 를 누른 까닭.
pub const UP_IS_NOT_A_PROJECT: &str = "`..` 은 등록하지 않는다 — 올라가서 `./` 에서 a";

/// `./` 에서 Enter 를 누른 까닭.
pub const HERE_IS_ALREADY_OPEN: &str = "`./` 은 지금 열어 둔 디렉터리다 — 여기를 등록하려면 a";

impl Picker {
    /// 첫 층으로 연다. 커서는 첫 하위 디렉터리에 선다 — 고르러 들어온 사람이 먼저 보는 것은 밑이다.
    pub fn new(at: Listing) -> Picker {
        let mut p = Picker { at, cursor: 0, list: Scroll::default(), show_hidden: false, typing: None, error: None };
        p.cursor = p.first_dir();
        p
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Here];
        if self.at.dir.parent().is_some() {
            rows.push(Row::Up);
        }
        rows.extend((0..self.at.entries.len()).map(Row::Dir));
        rows
    }

    pub fn current(&self) -> Option<Row> {
        self.rows().get(self.cursor).copied()
    }

    /// 그 줄이 가리키는 디렉터리.
    pub fn path_of(&self, row: Row) -> Option<PathBuf> {
        match row {
            Row::Here => Some(self.at.dir.clone()),
            Row::Up => self.at.dir.parent().map(Path::to_path_buf),
            Row::Dir(i) => self.at.entries.get(i).map(|d| self.at.dir.join(&d.name)),
        }
    }

    fn first_dir(&self) -> usize {
        self.rows().iter().position(|r| matches!(r, Row::Dir(_))).unwrap_or(0)
    }

    /// 새로 읽은 층을 넣는다. **커서는 정체(경로)로 따라간다:**
    /// - 같은 디렉터리를 다시 읽었으면(등록 표시를 고치려고, 숨은 것 보이기) 보던 줄에
    /// - 한 층 올라왔으면 **방금 나온 디렉터리에** — 탐색기의 Bksp 와 같다
    /// - 아니면 첫 하위 디렉터리에
    pub fn show(&mut self, at: Listing) {
        let want = if at.dir == self.at.dir {
            self.current().and_then(|r| self.path_of(r))
        } else if self.at.dir.parent() == Some(at.dir.as_path()) {
            Some(self.at.dir.clone())
        } else {
            None
        };
        let moved = at.dir != self.at.dir;
        self.at = at;
        let found = want.and_then(|w| self.rows().into_iter().position(|r| self.path_of(r).as_deref() == Some(w.as_path())));
        self.cursor = found.unwrap_or_else(|| self.first_dir());
        if moved {
            self.list.rewind();
        }
        self.error = None;
    }

    /// 키 하나. Ctrl-C 는 여기 오기 전에 든 쪽이 받는다 — 어느 모드에서든 나가는 길이다.
    ///
    /// - **↑↓·PageUp/Down·Home/End** 커서
    /// - **Enter·→** 들어가기 (`..` 이면 위로)
    /// - **Bksp·←** 한 층 위로
    /// - **a** 커서가 선 디렉터리를 등록 (`./` 이면 지금 디렉터리)
    /// - **.** 점 디렉터리 보이기·감추기
    /// - **g** 경로 적기 — 지금 디렉터리를 채워 연다. 상대경로면 지금 디렉터리에 붙는다
    /// - **Esc·q** 닫기
    pub fn key(&mut self, k: KeyEvent) -> Act {
        self.error = None;
        if let Some(input) = &mut self.typing {
            if input.key(k) {
                return Act::Stay;
            }
            if k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
                return Act::Stay;
            }
            return match k.code {
                KeyCode::Enter => {
                    let text = input.text().trim().to_string();
                    self.typing = None;
                    // **`~` 는 든 쪽이 푼다.** 여기는 조각이라 홈을 모른다(환경을 안 본다).
                    // 붙여 버리면 `~` 가 하위 디렉터리 이름이 되어 `…/~/work/argos` 를
                    // 찾다 실패하고, 사람은 없는 디렉터리로 읽는다 — 껍데기가 풀어 주는
                    // 철자라 경로 칸에 가장 먼저 치는 것이 이것이다.
                    match text.as_str() {
                        "" => Act::Stay,
                        t if t == "~" || t.starts_with("~/") => Act::Go(PathBuf::from(t)),
                        t => Act::Go(self.at.dir.join(t)),
                    }
                }
                KeyCode::Esc => {
                    self.typing = None;
                    Act::Stay
                }
                _ => Act::Stay,
            };
        }
        if k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return Act::Stay;
        }
        if let Some(at) = super::scroll::cursor(k, self.cursor, || self.rows().len()) {
            self.cursor = at;
            return Act::Stay;
        }
        let row = self.current();
        match k.code {
            KeyCode::Enter | KeyCode::Right => match row {
                Some(r @ (Row::Up | Row::Dir(_))) => self.path_of(r).map_or(Act::Stay, Act::Go),
                // `./` 은 이미 여기다 — 들어갈 데가 없다. **조용히 먹지 않는다**: 아랫줄이
                // `Enter 들어가기` 를 대고 있고, 하위 디렉터리가 없는 자리에서는 커서가
                // 여기 서므로(`first_dir`) 아무 말 없으면 창이 멎은 줄 안다(`..` 의 `a` 와 같다).
                Some(Row::Here) => {
                    self.error = Some(HERE_IS_ALREADY_OPEN.into());
                    Act::Stay
                }
                None => Act::Stay,
            },
            KeyCode::Backspace | KeyCode::Left => self.path_of(Row::Up).map_or(Act::Stay, Act::Go),
            KeyCode::Char('a') => match row {
                Some(Row::Up) => {
                    self.error = Some(UP_IS_NOT_A_PROJECT.into());
                    Act::Stay
                }
                Some(r) => self.path_of(r).map_or(Act::Stay, Act::Register),
                None => Act::Stay,
            },
            KeyCode::Char('.') => {
                self.show_hidden = !self.show_hidden;
                Act::Go(self.at.dir.clone())
            }
            KeyCode::Char('g') => {
                let mut text = self.at.dir.display().to_string();
                if !text.ends_with(std::path::MAIN_SEPARATOR) {
                    text.push(std::path::MAIN_SEPARATOR);
                }
                self.typing = Some(Input::new(&text));
                Act::Stay
            }
            KeyCode::Esc | KeyCode::Char('q') => Act::Close,
            _ => Act::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(p: &mut Picker, code: KeyCode) -> Act {
        p.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn dent(name: &str) -> Dent {
        Dent { name: name.into(), moai: false, registered: false }
    }

    fn listing(dir: &str, names: &[&str]) -> Listing {
        Listing {
            dir: dir.into(),
            entries: names.iter().map(|n| dent(n)).collect(),
            cut: 0,
            hidden: 0,
            moai: false,
            registered: false,
        }
    }

    /// 여는 순간 커서는 첫 하위 디렉터리에 선다. 뿌리(`/`)에는 `..` 이 없다.
    #[test]
    fn it_opens_on_the_first_subdirectory_and_the_root_has_no_way_up() {
        let p = Picker::new(listing("/w", &["apps", "libs"]));
        assert_eq!(p.rows(), [Row::Here, Row::Up, Row::Dir(0), Row::Dir(1)]);
        assert_eq!(p.current(), Some(Row::Dir(0)));
        let root = Picker::new(listing("/", &[]));
        assert_eq!(root.rows(), [Row::Here]);
        assert_eq!(root.current(), Some(Row::Here), "빈 뿌리에서 커서가 줄 밖에 섰다");
    }

    /// Enter·→ 는 들어가기, Bksp·← 는 위로, `..` 의 Enter 도 위로. **디렉터리를 읽는 것은
    /// 든 쪽이다** — 창은 어디로 갈지만 댄다.
    #[test]
    fn moving_says_where_to_go_and_never_reads() {
        let mut p = Picker::new(listing("/w", &["apps", "libs"]));
        assert_eq!(press(&mut p, KeyCode::Down), Act::Stay);
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/libs".into()));
        assert_eq!(press(&mut p, KeyCode::Right), Act::Go("/w/libs".into()));
        assert_eq!(press(&mut p, KeyCode::Backspace), Act::Go("/".into()));
        assert_eq!(press(&mut p, KeyCode::Left), Act::Go("/".into()));
        p.cursor = 1;
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/".into()), "`..` 의 Enter 가 위로 안 갔다");
        p.cursor = 0;
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Stay, "`./` 의 Enter 가 무언가 했다");
        assert_eq!(p.at, listing("/w", &["apps", "libs"]), "창이 제 층을 스스로 바꿨다");
    }

    /// `a` 는 커서가 선 디렉터리를, `./` 이면 지금 디렉터리를 등록하라고 한다. `..` 은 거절하고
    /// 까닭을 단다 — 다음 키에 걷힌다.
    #[test]
    fn a_registers_the_highlighted_directory_and_refuses_the_way_up() {
        let mut p = Picker::new(listing("/w/mono", &["apps"]));
        assert_eq!(press(&mut p, KeyCode::Char('a')), Act::Register("/w/mono/apps".into()));
        p.cursor = 0;
        assert_eq!(press(&mut p, KeyCode::Char('a')), Act::Register("/w/mono".into()));
        p.cursor = 1;
        assert_eq!(press(&mut p, KeyCode::Char('a')), Act::Stay);
        assert_eq!(p.error.as_deref(), Some(UP_IS_NOT_A_PROJECT));
        press(&mut p, KeyCode::Down);
        assert_eq!(p.error, None, "까닭이 다음 키에 안 걷혔다");
        // Ctrl·Alt 붙은 `a` 는 등록이 아니다
        assert_eq!(p.key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)), Act::Stay);
    }

    /// **새 층을 넣으면 커서는 정체로 따라간다** — 다시 읽으면 보던 줄에, 올라오면 나온
    /// 디렉터리에, 내려가면 첫 하위 디렉터리에.
    #[test]
    fn show_keeps_the_cursor_on_what_it_was_looking_at() {
        let mut p = Picker::new(listing("/w/mono/apps", &["a", "b", "c"]));
        press(&mut p, KeyCode::End);
        assert_eq!(p.current(), Some(Row::Dir(2)));
        // 같은 층을 다시 읽었다 — 위에 줄 하나가 생겨도 보던 `c` 에 선다
        let mut again = listing("/w/mono/apps", &["0", "a", "b", "c"]);
        again.entries[3].registered = true;
        p.show(again);
        assert_eq!(p.current(), Some(Row::Dir(3)));
        // 올라오면 방금 나온 `apps` 에
        p.show(listing("/w/mono", &["apps", "docs", "libs"]));
        assert_eq!(p.path_of(p.current().unwrap()), Some("/w/mono/apps".into()));
        // 내려가면 첫 하위 디렉터리에
        p.show(listing("/w/mono/libs", &["x"]));
        assert_eq!(p.current(), Some(Row::Dir(0)));
    }

    /// `g` 는 지금 디렉터리를 채운 칸을 연다. Enter 면 거기로, 상대경로는 지금 디렉터리에
    /// 붙는다. Esc 는 칸만 닫고 창은 둔다. **칸이 열린 동안 `q`·`a` 는 글자다.**
    #[test]
    fn g_opens_a_path_field_whose_keys_are_letters() {
        let mut p = Picker::new(listing("/w", &["apps"]));
        press(&mut p, KeyCode::Char('g'));
        assert_eq!(p.typing.as_ref().map(Input::text), Some("/w/"));
        for c in "qa".chars() {
            assert_eq!(press(&mut p, KeyCode::Char(c)), Act::Stay);
        }
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/qa".into()));
        assert_eq!(p.typing, None);

        press(&mut p, KeyCode::Char('g'));
        p.typing = Some(Input::new("apps/sub"));
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/apps/sub".into()), "상대경로가 지금 디렉터리에 안 붙었다");

        // **`~` 는 붙이지 않고 그대로 넘긴다** — 껍데기가 풀어 주는 철자라 경로 칸에
        // 가장 먼저 치는 것이 이것이고, 붙여 버리면 `…/~/work` 를 찾다 없다고 한다.
        // 홈이 어디인지는 드는 쪽(`register::expand_home`)이 안다 — 여기는 조각이다.
        for typed in ["~", "~/work/argos"] {
            press(&mut p, KeyCode::Char('g'));
            p.typing = Some(Input::new(typed));
            assert_eq!(press(&mut p, KeyCode::Enter), Act::Go(typed.into()), "`~` 를 지금 디렉터리에 붙였다");
        }
        // `~` 로 시작하지 않는 것은 그대로 붙는다 — `~x` 는 그냥 이름이다.
        press(&mut p, KeyCode::Char('g'));
        p.typing = Some(Input::new("~x"));
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/~x".into()));
        press(&mut p, KeyCode::Char('g'));
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Stay);
        assert_eq!(p.typing, None);
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Close, "칸을 닫은 뒤 Esc 가 창을 안 닫았다");
    }

    /// `.` 은 숨은 것 보이기를 뒤집고 같은 층을 다시 읽으라고 한다. Esc·q 는 닫는다.
    #[test]
    fn dot_toggles_hidden_and_esc_or_q_closes() {
        let mut p = Picker::new(listing("/w", &[]));
        assert_eq!(press(&mut p, KeyCode::Char('.')), Act::Go("/w".into()));
        assert!(p.show_hidden);
        assert_eq!(press(&mut p, KeyCode::Char('.')), Act::Go("/w".into()));
        assert!(!p.show_hidden);
        assert_eq!(press(&mut p, KeyCode::Char('q')), Act::Close);
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Close);
    }
}
