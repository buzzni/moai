//! 탐색기 화면. **읽기 전용이다.**
//!
//! 상태([`App`])와 그림([`draw`])을 나눈다. 상태 전이는 터미널 없이 시험되고,
//! 그림은 `TestBackend` 로 시험된다 — 둘 다 TTY 를 켜지 않는다.

pub mod draw;

use crate::config::Config;
use crate::model::Issue;
use crate::nav::{Entry, Index, Path, Seg};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 목록의 한 줄. `..` 은 이슈가 아니므로 [`Entry`] 로는 못 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 한 층 위로. 뿌리가 아닐 때만 맨 앞에 선다 — MC 와 같다.
    Up,
    Item(Entry),
}

pub struct App {
    pub issues: Vec<Issue>,
    pub index: Index,
    pub cfg: Config,
    pub path: Path,
    pub cursor: usize,
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
        App { issues, index, cfg, path, cursor: 0, remembered, quit: false }
    }

    /// 지금 디렉터리의 줄들.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        if !self.path.is_empty() {
            rows.push(Row::Up);
        }
        rows.extend(self.index.entries(&self.issues, &self.path).into_iter().map(Row::Item));
        rows
    }

    /// 커서가 가리키는 줄.
    pub fn current(&self) -> Option<Row> {
        self.rows().get(self.cursor).cloned()
    }

    pub fn key(&mut self, k: KeyEvent) {
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
            KeyCode::Backspace | KeyCode::Left | KeyCode::Esc => self.leave(),
            _ => {}
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
