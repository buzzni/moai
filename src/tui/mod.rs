//! 탐색기 화면. **읽기 전용이다.**
//!
//! 상태([`App`])와 그림([`draw`])을 나눈다. 상태 전이는 터미널 없이 시험되고,
//! 그림은 `TestBackend` 로 시험된다 — 둘 다 TTY 를 켜지 않는다.

pub mod draw;

use crate::config::Config;
use crate::model::Issue;
use crate::nav::{Entry, Index, Path, Seg};
use crate::query::{Filter, Raw, Where};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 목록의 한 줄. `..` 은 이슈가 아니므로 [`Entry`] 로는 못 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 한 층 위로. 뿌리가 아닐 때만 맨 앞에 선다 — MC 와 같다.
    Up,
    Item(Entry),
}

/// 무엇을 받고 있는가. 글을 받는 동안에는 이동키가 글자가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Browse,
    /// `/` 로 연 빠른 검색.
    Grep(String),
    /// `f` 로 연 거름망. **CLI 와 같은 `항목=값` 문법이다.**
    Filter(String),
}

pub struct App {
    pub issues: Vec<Issue>,
    pub index: Index,
    pub cfg: Config,
    pub path: Path,
    pub cursor: usize,
    pub mode: Mode,
    /// 지금 걸린 거름망. 사람이 적은 글 그대로도 들고 있어야 화면에 되비친다.
    pub filter_text: Option<String>,
    /// 이슈 첨자 → 걸렸는가. **거름망이 바뀔 때만 다시 센다** — 매 프레임
    /// `Filter::matches` 를 돌리면 `Where::of` 가 프레임마다 지도를 다시 만든다.
    keep: Vec<bool>,
    /// 층마다 커서를 기억한다. 들어갔다 나오면 **있던 자리로 돌아온다** —
    /// 매번 맨 위로 튕기면 형제 여럿을 훑는 일이 못 할 짓이 된다.
    remembered: Vec<usize>,
    pub quit: bool,
}

impl App {
    pub fn new(issues: Vec<Issue>, cfg: Config, path: Path) -> App {
        let index = Index::of(&issues);
        // 들어간 채로 시작하면(`--path`) 나올 층마다 기억 자리를 만들어 둔다.
        let remembered = vec![0; path.len()];
        let keep = vec![true; issues.len()];
        App {
            issues,
            index,
            cfg,
            path,
            cursor: 0,
            mode: Mode::Browse,
            filter_text: None,
            keep,
            remembered,
            quit: false,
        }
    }

    /// 거름망을 건다. 빈 글은 "거름망 없음" 이다.
    ///
    /// **`all` 을 켠다.** `Filter` 의 기본값은 done 을 숨기는데, 탐색기가
    /// 시키지도 않은 줄을 숨기면 파일이 사라진 것처럼 보인다. 열린 것만 보려면
    /// `status=todo` 라고 적으면 된다.
    pub fn apply(&mut self, mode: &Mode) -> Result<(), String> {
        let (text, raw) = match mode {
            Mode::Grep(q) => (q.clone(), Raw { grep: Some(q.clone()), all: true, ..Raw::default() }),
            Mode::Filter(q) => (
                q.clone(),
                Raw { filter: q.split_whitespace().map(str::to_string).collect(), all: true, ..Raw::default() },
            ),
            Mode::Browse => (String::new(), Raw::default()),
        };
        if text.trim().is_empty() {
            self.filter_text = None;
            self.keep = vec![true; self.issues.len()];
            return Ok(());
        }
        // **`Filter::build` 를 지난다.** 소문자 접기·태그 정규화·`항목=값` 해석이
        // 전부 거기 있고, 건너뛰면 CLI 와 TUI 가 같은 글을 다르게 읽는다.
        let filter = Filter::build(raw)?;
        let now = crate::model::now();
        let wh = Where::of(&self.issues);
        self.keep = self.issues.iter().map(|i| filter.matches(i, &now, &wh)).collect();
        self.filter_text = Some(match mode {
            Mode::Grep(_) => format!("/{text}"),
            _ => text,
        });
        Ok(())
    }

    pub fn clear_filter(&mut self) {
        self.filter_text = None;
        self.keep = vec![true; self.issues.len()];
    }

    /// 지금 디렉터리의 줄들.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        if !self.path.is_empty() {
            rows.push(Row::Up);
        }
        let keep = &self.keep;
        rows.extend(
            self.index
                .entries_where(&self.issues, &self.path, &|at| keep[at])
                .into_iter()
                .map(Row::Item),
        );
        rows
    }

    /// 커서가 가리키는 줄.
    pub fn current(&self) -> Option<Row> {
        self.rows().get(self.cursor).cloned()
    }

    pub fn key(&mut self, k: KeyEvent) {
        // 글을 받는 동안에는 이동키가 글자다. 먼저 가로챈다.
        if !matches!(self.mode, Mode::Browse) {
            self.typing(k);
            return;
        }
        let len = self.rows().len();
        match k.code {
            // raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. 안 받으면 길이 막힌다.
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('q') | KeyCode::F(10) => self.quit = true,
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => self.cursor = (self.cursor + 1).min(len.saturating_sub(1)),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len.saturating_sub(1),
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(10),
            KeyCode::PageDown => self.cursor = (self.cursor + 10).min(len.saturating_sub(1)),
            KeyCode::Enter | KeyCode::Right => self.enter(),
            KeyCode::Backspace | KeyCode::Left => self.leave(),
            KeyCode::Char('/') => self.mode = Mode::Grep(String::new()),
            KeyCode::Char('f') | KeyCode::F(7) => self.mode = Mode::Filter(String::new()),
            // 거름망이 걸려 있으면 Esc 가 그것을 푼다. 아니면 아무 일도 없다 —
            // Esc 로 화면이 꺼지면 실수 한 번에 하던 것이 날아간다.
            KeyCode::Esc => self.clear_filter(),
            _ => {}
        }
    }

    /// 글을 받는 중.
    fn typing(&mut self, k: KeyEvent) {
        let buf = match &mut self.mode {
            Mode::Grep(b) | Mode::Filter(b) => b,
            Mode::Browse => return,
        };
        match k.code {
            KeyCode::Char(c) => buf.push(c),
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Enter => {
                let mode = self.mode.clone();
                // 잘못 적은 것은 버리지 않고 그 자리에 둔다 — 지우고 다시 치게
                // 하면 긴 거름망일수록 고치기가 벌이 된다.
                if self.apply(&mode).is_ok() {
                    self.mode = Mode::Browse;
                    self.cursor = self.cursor.min(self.rows().len().saturating_sub(1));
                }
            }
            KeyCode::Esc => self.mode = Mode::Browse,
            _ => {}
        }
    }

    /// 지금 적고 있는 글에 대한 오류. 없으면 `None`.
    pub fn input_error(&self) -> Option<String> {
        match &self.mode {
            Mode::Filter(q) if !q.trim().is_empty() => Filter::build(Raw {
                filter: q.split_whitespace().map(str::to_string).collect(),
                all: true,
                ..Raw::default()
            })
            .err(),
            _ => None,
        }
    }

    fn enter(&mut self) {
        match self.current() {
            Some(Row::Up) => self.leave(),
            Some(Row::Item(Entry::Dir { seg, .. })) => {
                self.remembered.push(self.cursor);
                self.path.push(seg);
                self.cursor = 0;
            }
            // 잎은 들어갈 데가 없다. 상세는 오른쪽이 이미 보여 주고 있다.
            _ => {}
        }
    }

    fn leave(&mut self) {
        if self.path.pop().is_some() {
            self.cursor = self.remembered.pop().unwrap_or(0);
            // 기억한 자리가 낡았을 수 있다 (파일이 바뀌었거나 `--path` 로 시작했거나).
            self.cursor = self.cursor.min(self.rows().len().saturating_sub(1));
        }
    }

    /// 지금 어디인가. 뿌리는 `/`.
    pub fn crumbs(&self) -> String {
        if self.path.is_empty() {
            return "/".into();
        }
        let mut out = String::new();
        for seg in &self.path {
            out.push('/');
            out.push_str(&self.seg_label(seg));
        }
        out
    }

    /// id 를 제목으로 푼다. 없으면 id 그대로 — 끊긴 참조를 숨기지 않는다.
    pub fn title_of(&self, id: &str) -> String {
        match self.issues.iter().find(|i| i.id == id) {
            Some(i) => i.title.clone(),
            None => format!("{id}  (없다)"),
        }
    }

    fn seg_label(&self, seg: &Seg) -> String {
        let id = match seg {
            Seg::Milestone(None) => return "(마일스톤 없음)".into(),
            Seg::Lost => return "(길 잃음)".into(),
            Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => id,
        };
        match self.issues.iter().find(|i| &i.id == id) {
            Some(i) => i.title.clone(),
            None => id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn make(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str) -> Issue {
        let mut i = make(id, Kind::Issue);
        i.epic = Some(epic.into());
        i
    }

    fn app() -> App {
        let issues = vec![
            make("argos-0001", Kind::Epic),
            make("argos-0002", Kind::Epic),
            member("argos-0003", "argos-0001"),
            member("argos-0004", "argos-0001"),
            make("argos-0009", Kind::Issue),
        ];
        App::new(issues, cfg(), Path::new())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 지금 보이는 줄의 id 들 (`..` 은 뺀다).
    fn shown(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(e) => e.at().map(|at| a.issues[at].id.clone()),
                Row::Up => None,
            })
            .collect()
    }

    /// 뿌리에는 `..` 이 없다. 들어가면 생긴다.
    #[test]
    fn up_appears_only_below_the_root() {
        let mut a = app();
        assert!(!a.rows().contains(&Row::Up));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.rows().first(), Some(&Row::Up));
    }

    /// 들어갔다 나오면 **있던 자리로 돌아온다**. 매번 맨 위로 튕기면 못 쓴다.
    #[test]
    fn leaving_puts_the_cursor_back_where_it_was() {
        let mut a = app();
        a.key(key(KeyCode::Down)); // 두 번째 에픽
        assert_eq!(a.cursor, 1);
        a.key(key(KeyCode::Enter));
        assert_eq!((a.cursor, a.path.len()), (0, 1));
        a.key(key(KeyCode::Backspace));
        assert_eq!((a.cursor, a.path.len()), (1, 0), "있던 자리로 안 돌아왔다");
    }

    /// 커서는 목록 밖으로 못 나간다.
    #[test]
    fn the_cursor_stays_inside_the_list() {
        let mut a = app();
        for _ in 0..50 {
            a.key(key(KeyCode::Up));
        }
        assert_eq!(a.cursor, 0);
        for _ in 0..50 {
            a.key(key(KeyCode::Down));
        }
        assert_eq!(a.cursor, a.rows().len() - 1);
        a.key(key(KeyCode::Home));
        assert_eq!(a.cursor, 0);
        a.key(key(KeyCode::End));
        assert_eq!(a.cursor, a.rows().len() - 1);
    }

    /// 잎에서 Enter 는 아무 일도 하지 않는다 — 들어갈 데가 없다.
    #[test]
    fn entering_a_leaf_does_nothing() {
        let mut a = app();
        a.key(key(KeyCode::End)); // 소속 없는 이슈 (잎)
        let before = a.path.clone();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, before);
    }

    /// `..` 에서 Enter 는 나가기다.
    #[test]
    fn entering_the_up_row_leaves() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path.len(), 1);
        a.cursor = 0; // `..`
        a.key(key(KeyCode::Enter));
        assert!(a.path.is_empty());
    }

    #[test]
    fn quitting_works_every_documented_way() {
        for k in [
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            key(KeyCode::Char('q')),
            key(KeyCode::F(10)),
        ] {
            let mut a = app();
            a.key(k);
            assert!(a.quit, "{k:?} 로 못 나갔다");
        }
    }

    /// 뿌리는 `/`, 들어가면 **제목**으로 적는다 — id 를 적으면 사람이 그걸
    /// 다시 찾아봐야 한다.
    #[test]
    fn crumbs_read_as_titles() {
        let mut a = app();
        assert_eq!(a.crumbs(), "/");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.crumbs(), "/argos-0001 제목");
    }

    fn typed(a: &mut App, text: &str) {
        for c in text.chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
    }

    /// `/` 로 검색하면 안 걸린 잎은 사라지고, 걸린 것을 품은 디렉터리는 남는다.
    #[test]
    fn slash_searches_titles() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        assert!(matches!(a.mode, Mode::Grep(_)));
        typed(&mut a, "0004");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(a.filter_text.as_deref(), Some("/0004"));

        // 뿌리에는 그것을 품은 에픽만 남는다
        let ids = shown(&a);
        assert_eq!(ids, ["argos-0001"], "{ids:?}");
        // 그 안에 걸린 것이 있다
        a.key(key(KeyCode::Enter));
        assert_eq!(shown(&a), ["argos-0004"]);
    }

    /// 거름망은 **CLI 와 같은 문법**이다. 없는 항목은 그 자리에서 나무란다.
    #[test]
    fn f_takes_the_same_grammar_as_the_cli() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);

        // 잘못 적으면 걸리지 않고 그 자리에 남는다 — 지우고 다시 치게 하지 않는다
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "statu=todo");
        assert!(matches!(a.mode, Mode::Filter(_)), "잘못 적었는데 넘어갔다");
        assert!(a.input_error().is_some());
        assert_eq!(a.filter_text, None);
    }

    /// **거름망이 찾으려던 것을 숨기지 않는다.** 끝난 에픽도 걸린 멤버가 있으면
    /// 남는다. `Filter` 의 기본값이 done 을 숨기는 것에 걸려들지 않아야 한다.
    #[test]
    fn a_finished_epic_does_not_swallow_its_matching_members() {
        let mut issues = vec![
            make("argos-0001", Kind::Epic),
            member("argos-0003", "argos-0001"),
        ];
        issues[0].status = Status::new("done");
        let mut a = App::new(issues, cfg(), Path::new());
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "status=todo");
        assert_eq!(shown(&a), ["argos-0001"], "끝난 에픽이 걸린 멤버를 데리고 사라졌다");
    }

    /// Esc 는 거름망을 푼다. 화면을 끄지는 않는다 — 실수 한 번에 하던 것이
    /// 날아가면 안 된다.
    #[test]
    fn esc_clears_the_filter_but_never_quits() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        typed(&mut a, "0004");
        assert!(a.filter_text.is_some());
        a.key(key(KeyCode::Esc));
        assert_eq!(a.filter_text, None);
        assert!(!a.quit);
        assert_eq!(shown(&a).len(), 3);
    }

    /// 글을 받는 동안에는 이동키가 글자다 — `q` 를 쳤다고 꺼지면 못 쓴다.
    #[test]
    fn typing_does_not_trigger_browse_keys() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        for c in "quit".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert!(!a.quit);
        assert_eq!(a.mode, Mode::Grep("quit".into()));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.mode, Mode::Browse);
    }

    /// 빈 디렉터리에서도 무너지지 않는다.
    #[test]
    fn an_empty_repo_is_safe() {
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        assert!(a.rows().is_empty());
        assert_eq!(a.current(), None);
        for k in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace, KeyCode::End] {
            a.key(key(k));
        }
        assert_eq!(a.cursor, 0);
    }
}
