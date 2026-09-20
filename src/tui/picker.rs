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
use super::keys::{Chord, Goto, Lookup, PATH, PICK, Pick, label, lookup};
use super::scroll::Scroll;
use ratatui::crossterm::event::KeyEvent;
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
    /// `g p` 로 연 경로 적기 칸.
    pub typing: Option<Input>,
    /// `g` 뒤를 기다리는 열(`gg` 맨 위·`g p` 경로 적기). **창 안에 둔다** — 창이 닫히면 함께 버려진다.
    pub chord: Chord,
    /// 마지막 키가 못 한 까닭(못 읽는 디렉터리·거절된 등록). **다음 키에 걷힌다.**
    pub error: Option<String>,
}

/// 창이 든 쪽에 시키는 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Stay,
    /// 이 디렉터리를 읽어 [`Picker::show`] 로 넣어 달라. 지금 디렉터리를 다시 읽는 것도 이것이다.
    Go(PathBuf),
    /// 지금 디렉터리를 **점 디렉터리 보이기를 이 값으로** 다시 읽어 달라. 보이기 설정은 읽기가
    /// 됐을 때 목록과 함께 넣는다(moai-v2jf) — 창이 먼저 뒤집으면 못 읽는 디렉터리에서 설정은
    /// 바뀌고 목록은 옛것이라 둘이 어긋난다.
    Hidden(bool),
    /// 이 디렉터리를 등록해 달라.
    Register(PathBuf),
    /// 닫는다. 적던 것이 없으니 묻지 않는다.
    Close,
}

/// `..` 에서 `a` 를 누른 까닭. 키 이름은 표에서 읽는다 — 등록 키를 옮기면 이 말도 따라온다.
pub fn up_is_not_a_project(lang: crate::i18n::Lang) -> String {
    crate::i18n::fill(crate::i18n::say(lang, "tui.pick.up_is_not_a_project"), &[
        ("key", &label(PICK, Pick::Register)),
    ])
}

/// `./` 에서 Enter 를 누른 까닭.
pub fn here_is_already_open(lang: crate::i18n::Lang) -> String {
    crate::i18n::fill(crate::i18n::say(lang, "tui.pick.here_is_already_open"), &[
        ("key", &label(PICK, Pick::Register)),
    ])
}

impl Picker {
    /// 첫 층으로 연다. 커서는 첫 하위 디렉터리에 선다 — 고르러 들어온 사람이 먼저 보는 것은 밑이다.
    pub fn new(at: Listing) -> Picker {
        let mut p = Picker {
            at,
            cursor: 0,
            list: Scroll::default(),
            show_hidden: false,
            typing: None,
            chord: Chord::default(),
            error: None,
        };
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

    /// 붙여 넣은 글(moai-od9q). **경로 칸으로 간다** — 열려 있으면 커서 자리에, 닫혀 있으면
    /// 붙인 글로 연다. 이 창에서 붙이는 글은 경로뿐이고, 키로 읽으면 `q` 가 창을 닫고 `a` 가
    /// 커서의 줄을 등록한다. 닫힌 칸을 지금 디렉터리로 채워 열지 않는다(`g p` 와 다르다) —
    /// 붙이는 경로는 대개 절대경로라 앞에 붙은 디렉터리를 사람이 지워야 한다. 상대경로여도
    /// Enter 가 지금 디렉터리에 붙인다.
    pub fn paste(&mut self, s: &str) {
        self.error = None;
        // 기다리던 `g` 는 버린다 — 칸을 닫은 뒤의 `g` 가 붙여넣기 전의 `g` 와 이어 맨 위로 뛰지 않게.
        self.chord.clear();
        self.typing.get_or_insert_with(|| Input::path("")).paste(s);
    }

    /// 키 하나. Ctrl-C 는 여기 오기 전에 든 쪽이 받는다 — 어느 모드에서든 나가는 길이다.
    ///
    /// - **j·k·gg·G·Ctrl-d/u/f/b** (↑↓·Home/End·PageUp/Down) 커서
    /// - **Enter·l·→** 들어가기 (`..` 이면 위로)
    /// - **Bksp·h·←** 한 층 위로
    /// - **a** 커서가 선 디렉터리를 등록 (`./` 이면 지금 디렉터리)
    /// - **.** 점 디렉터리 보이기·감추기
    /// - **g p** 경로 적기 — 지금 디렉터리를 채워 연다. 상대경로면 지금 디렉터리에 붙는다
    /// - **Esc·q** 닫기
    pub fn key(&mut self, k: KeyEvent, lang: crate::i18n::Lang) -> Act {
        self.error = None;
        if let Some(input) = &mut self.typing {
            if input.key(k) {
                return Act::Stay;
            }
            // 칸이 안 먹은 키는 표([`PATH`])가 가른다. Ctrl·Alt 붙은 키는 표에 없어 아무 일도 없다.
            return match lookup(PATH, &[k]) {
                Lookup::Run(Goto::Go) => {
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
                Lookup::Run(Goto::Cancel) => {
                    self.typing = None;
                    Act::Stay
                }
                Lookup::Pending | Lookup::Unknown => Act::Stay,
            };
        }
        // 키의 뜻은 표([`PICK`])에서 읽는다. 창의 줄은 vi 쪽 이동(Ctrl-d 등)말고는 Ctrl·Alt 없이
        // 누른 키라, 거들쇠가 붙은 키는 표에 없어 아무 일도 없다. `g` 는 다음 키를 기다린다 —
        // 뜻 없는 다음 키는 기다린 `g` 와 함께 버려진다([`Chord::feed`]).
        let Some(act) = self.chord.feed(PICK, k) else { return Act::Stay };
        if let Pick::Step(m) = act {
            self.cursor = super::scroll::cursor(m, self.cursor, || self.rows().len());
            return Act::Stay;
        }
        let row = self.current();
        match act {
            Pick::Step(_) => Act::Stay,
            Pick::Enter => match row {
                Some(r @ (Row::Up | Row::Dir(_))) => self.path_of(r).map_or(Act::Stay, Act::Go),
                // `./` 은 이미 여기다 — 들어갈 데가 없다. **조용히 먹지 않는다**: 아랫줄이
                // `Enter 들어가기` 를 대고 있고, 하위 디렉터리가 없는 자리에서는 커서가
                // 여기 서므로(`first_dir`) 아무 말 없으면 창이 멎은 줄 안다(`..` 의 `a` 와 같다).
                Some(Row::Here) => {
                    self.error = Some(here_is_already_open(lang));
                    Act::Stay
                }
                None => Act::Stay,
            },
            Pick::Up => self.path_of(Row::Up).map_or(Act::Stay, Act::Go),
            Pick::Register => match row {
                Some(Row::Up) => {
                    self.error = Some(up_is_not_a_project(lang));
                    Act::Stay
                }
                Some(r) => self.path_of(r).map_or(Act::Stay, Act::Register),
                None => Act::Stay,
            },
            // **뒤집지 않고 원하는 값만 댄다** — 넣는 것은 다시 읽기가 된 뒤 든 쪽이 한다.
            Pick::Hidden => Act::Hidden(!self.show_hidden),
            Pick::Path => {
                let mut text = self.at.dir.display().to_string();
                if !text.ends_with(std::path::MAIN_SEPARATOR) {
                    text.push(std::path::MAIN_SEPARATOR);
                }
                self.typing = Some(Input::path(&text));
                Act::Stay
            }
            Pick::Close => Act::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    /// 시험은 한국어로 잰다 — `ko` 표가 이 키들을 실제로 드는지도 함께 재는 자리다.
    const KO: crate::i18n::Lang = crate::i18n::Lang::Ko;

    fn press(p: &mut Picker, code: KeyCode) -> Act {
        p.key(KeyEvent::new(code, KeyModifiers::NONE), KO)
    }

    /// 경로 적기 칸을 연다 — `g p`.
    fn path(p: &mut Picker) -> Act {
        assert_eq!(press(p, KeyCode::Char('g')), Act::Stay);
        press(p, KeyCode::Char('p'))
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
        assert_eq!(p.error, Some(up_is_not_a_project(KO)));
        // 문구는 표에서 키 이름을 읽어 짓는다 — 옮기기 전과 한 글자도 같다(moai-nc7w).
        assert_eq!(up_is_not_a_project(KO), "`..` 은 등록하지 않는다 — 올라가서 `./` 에서 a");
        assert_eq!(here_is_already_open(KO), "`./` 은 지금 열어 둔 디렉터리다 — 여기를 등록하려면 a");
        press(&mut p, KeyCode::Down);
        assert_eq!(p.error, None, "까닭이 다음 키에 안 걷혔다");
        // Ctrl·Alt 붙은 `a` 는 등록이 아니다
        assert_eq!(p.key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL), KO), Act::Stay);
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

    /// `g p` 는 지금 디렉터리를 채운 칸을 연다. Enter 면 거기로, 상대경로는 지금 디렉터리에
    /// 붙는다. Esc 는 칸만 닫고 창은 둔다. **칸이 열린 동안 `q`·`a`·vi 키(`j`·`k`·`g`·`h`·`l`·`G`)는
    /// 글자다** — `g` 가 칸 안에서 다시 기다리면 경로에 `g` 를 못 친다.
    #[test]
    fn g_p_opens_a_path_field_whose_keys_are_letters() {
        let mut p = Picker::new(listing("/w", &["apps"]));
        path(&mut p);
        assert_eq!(p.typing.as_ref().map(Input::text), Some("/w/"));
        for c in "qajkgghlGp".chars() {
            assert_eq!(press(&mut p, KeyCode::Char(c)), Act::Stay);
        }
        assert!(!p.chord.waiting(), "칸 안의 `g` 가 기다린다");
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/qajkgghlGp".into()));
        assert_eq!(p.typing, None);

        path(&mut p);
        p.typing = Some(Input::path("apps/sub"));
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/apps/sub".into()), "상대경로가 지금 디렉터리에 안 붙었다");

        // **`~` 는 붙이지 않고 그대로 넘긴다** — 껍데기가 풀어 주는 철자라 경로 칸에
        // 가장 먼저 치는 것이 이것이고, 붙여 버리면 `…/~/work` 를 찾다 없다고 한다.
        // 홈이 어디인지는 드는 쪽(`register::expand_home`)이 안다 — 여기는 조각이다.
        for typed in ["~", "~/work/argos"] {
            path(&mut p);
            p.typing = Some(Input::path(typed));
            assert_eq!(press(&mut p, KeyCode::Enter), Act::Go(typed.into()), "`~` 를 지금 디렉터리에 붙였다");
        }
        // `~` 로 시작하지 않는 것은 그대로 붙는다 — `~x` 는 그냥 이름이다.
        path(&mut p);
        p.typing = Some(Input::path("~x"));
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/~x".into()));
        path(&mut p);
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Stay);
        assert_eq!(p.typing, None);
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Close, "칸을 닫은 뒤 Esc 가 창을 안 닫았다");
    }

    /// **붙여넣기는 경로 칸으로 간다**(moai-od9q). 칸이 열려 있으면 커서 자리에, 닫혀 있으면
    /// 붙인 글로 칸을 연다 — 창에서 붙이는 글은 경로뿐이고, 글자를 키로 읽으면 `q` 가 창을
    /// 닫고 `a` 가 엉뚱한 줄을 등록한다. 끝의 줄바꿈은 Enter 가 아니다.
    #[test]
    fn a_paste_goes_into_the_path_field() {
        let mut p = Picker::new(listing("/w", &["apps"]));
        p.paste("/srv/qa\n");
        assert_eq!(p.typing.as_ref().map(Input::text), Some("/srv/qa"), "붙인 경로로 칸을 안 열었다");
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/srv/qa".into()));

        path(&mut p);
        p.paste("apps");
        assert_eq!(press(&mut p, KeyCode::Enter), Act::Go("/w/apps".into()));
    }

    /// **경로 칸의 낱말 지우기는 `/` 에서 멈춘다**(moai-8dna) — 빈칸만 경계로 보면
    /// `/home/coder/work` 에서 Ctrl-W·Alt-Backspace 가 경로를 통째로 지운다. 한 층씩 지운다:
    /// 끝의 `/` 는 넘고, 앞 층의 `/` 는 남긴다. 빈칸은 여전히 경계다. `g p` 로 연 칸도,
    /// 붙여넣기로 연 칸도 같다.
    #[test]
    fn rubbing_a_word_in_the_path_field_stops_at_a_slash() {
        let alt_bksp = KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT);
        let ctrl_w = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        let text = |p: &Picker| p.typing.as_ref().map(|i| i.text().to_string());

        let mut p = Picker::new(listing("/home/coder/work", &[]));
        path(&mut p);
        assert_eq!(text(&p).as_deref(), Some("/home/coder/work/"));
        assert_eq!(p.key(alt_bksp, KO), Act::Stay);
        assert_eq!(text(&p).as_deref(), Some("/home/coder/"), "Alt-Backspace 가 한 층보다 많이 지웠다");
        p.key(ctrl_w, KO);
        assert_eq!(text(&p).as_deref(), Some("/home/"), "Ctrl-W 가 Alt-Backspace 와 다르게 지웠다");
        for c in "a b".chars() {
            press(&mut p, KeyCode::Char(c));
        }
        p.key(ctrl_w, KO);
        assert_eq!(text(&p).as_deref(), Some("/home/a "), "빈칸이 경계가 아니게 됐다");
        p.key(ctrl_w, KO);
        p.key(ctrl_w, KO);
        assert_eq!(text(&p).as_deref(), Some("/"));
        p.key(ctrl_w, KO);
        assert_eq!(text(&p).as_deref(), Some(""), "뿌리 `/` 가 안 지워졌다");
        press(&mut p, KeyCode::Esc);

        p.paste("/home/coder/work");
        p.key(alt_bksp, KO);
        assert_eq!(text(&p).as_deref(), Some("/home/coder/"), "붙여넣기로 연 칸이 경로 칸이 아니다");
    }

    /// **vi 이동**(moai-ob4c) — `j`·`k`, `gg`·`G`, Ctrl-d·Ctrl-u 가 커서를, `h`·`l` 이 위로·들어가기를
    /// 한다. 기다리는 `g` 뒤에 뜻 없는 키가 오면 둘 다 버린다.
    #[test]
    fn vi_keys_move_the_cursor_and_go_in_and_out() {
        let names: Vec<String> = (0..30).map(|n| format!("d{n:02}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut p = Picker::new(listing("/w", &refs));
        // 줄: `./`·`..`·d00…d29 — 여는 자리는 d00(2)
        assert_eq!(p.cursor, 2);
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(p.cursor, 3);
        press(&mut p, KeyCode::Char('k'));
        assert_eq!(p.cursor, 2);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        p.key(ctrl('d'), KO);
        assert_eq!(p.cursor, 2 + crate::tui::scroll::HALF);
        p.key(ctrl('u'), KO);
        assert_eq!(p.cursor, 2);
        p.key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT), KO);
        assert_eq!(p.current(), Some(Row::Dir(29)), "SHIFT 붙은 `G` 가 맨 아래로 안 갔다");
        assert_eq!(press(&mut p, KeyCode::Char('g')), Act::Stay);
        assert_eq!(p.current(), Some(Row::Dir(29)), "`g` 하나에 움직였다");
        press(&mut p, KeyCode::Char('g'));
        assert_eq!(p.cursor, 0, "`gg` 가 맨 위로 안 갔다");

        // 뜻 없는 둘째 키는 버린다 — `g` 뒤의 `j` 도, `a` 도 제 뜻을 안 한다
        p.cursor = 2;
        for c in ['x', 'j', 'a', 'q'] {
            press(&mut p, KeyCode::Char('g'));
            assert_eq!(press(&mut p, KeyCode::Char(c)), Act::Stay, "`g {c}` 가 무언가 했다");
            assert_eq!((p.cursor, p.typing.is_some(), p.chord.waiting()), (2, false, false), "`g {c}`");
        }

        assert_eq!(press(&mut p, KeyCode::Char('l')), Act::Go("/w/d00".into()));
        assert_eq!(press(&mut p, KeyCode::Char('h')), Act::Go("/".into()));
        // Esc 는 기다리던 `g` 를 버린다 — 창을 닫지 않는다
        press(&mut p, KeyCode::Char('g'));
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Stay, "`g` 뒤의 Esc 가 창을 닫았다");
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Close);
    }

    /// **붙여넣기는 기다리던 `g` 를 버린다** — 칸을 닫은 뒤의 `g` 가 붙이기 전의 `g` 와 이어
    /// 맨 위로 뛰지 않는다.
    #[test]
    fn a_paste_drops_a_waiting_g() {
        let mut p = Picker::new(listing("/w", &["a", "b"]));
        press(&mut p, KeyCode::Char('j'));
        press(&mut p, KeyCode::Char('g'));
        p.paste("x");
        assert!(!p.chord.waiting());
        press(&mut p, KeyCode::Esc);
        press(&mut p, KeyCode::Char('g'));
        assert_eq!(p.current(), Some(Row::Dir(1)), "붙이기 전의 `g` 와 이어졌다");
    }

    /// `.` 은 숨은 것 보이기를 **반대 값으로** 다시 읽으라고만 한다 — 창은 설정을 스스로 안
    /// 뒤집는다. 읽기가 된 뒤 든 쪽이 넣는다(moai-v2jf). Esc 가 닫는다 — 옛 숨은 `q` 는 걷었다(moai-en4u).
    #[test]
    fn dot_asks_for_the_other_hidden_setting_and_esc_closes() {
        let mut p = Picker::new(listing("/w", &[]));
        assert_eq!(press(&mut p, KeyCode::Char('.')), Act::Hidden(true));
        assert!(!p.show_hidden, "읽기 전에 창이 설정을 뒤집었다");
        p.show_hidden = true;
        assert_eq!(press(&mut p, KeyCode::Char('.')), Act::Hidden(false));
        assert!(p.show_hidden);
        assert_eq!(press(&mut p, KeyCode::Char('q')), Act::Stay, "걷은 q 가 닫았다");
        assert_eq!(press(&mut p, KeyCode::Esc), Act::Close);
    }
}
