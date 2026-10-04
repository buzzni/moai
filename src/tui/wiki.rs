//! 위키 창 — `SPC g w`(moai-o3cb). 목록과 상세 자리를 **통째로** 덮고, Esc 로 닫으면 보던 자리 그대로다 — 통계 창
//! (`tui::stats`)과 같은 덮는 창이다.
//!
//! **읽기만 한다**(2026-10-04 사용자 결정). 페이지를 걷고 푸는 것은 [`crate::wiki`] 가 하고 — `moai wiki ls` 와 같은
//! 자다 — 여기는 연 순간 그것을 한 번 불러 들고 다니며 고르고 굴리기만 한다. **설정에 안 남는다**: 열 때마다 새로
//! 읽고, 닫으면 버린다. 다시 읽기(`App::follow`)도 창을 안 건드린다 — 페이지는 가지를 타는 파일이라 트래커의 쓰기로
//! 바뀌지 않고, 보던 페이지가 읽는 사이에 바뀌어 커서가 튀는 것보다 다시 여는 키 하나가 싸다.
//!
//! **배치**: 왼쪽 페이지 목록(홈 먼저, `wiki::load` 의 차례 그대로), 오른쪽 본문. **본문은 목록 커서를 따라간다** —
//! 탐색기의 상세 칸과 같다(2026-10-04 사용자 결정). 목록의 `Enter` 는 본문 칸으로 가고, 칸 사이는 `Ctrl-w w` 다.
//!
//! **찾기(`/`)는 이 창의 페이지만 본다** — 제목과 본문에 그 글이 든 페이지로 목록을 좁히고, 본문 칸은 맞은 글을
//! 칠한다. 치는 동안 좁혀지고 Enter 가 걸고 Esc 가 친 것을 버린다(본 화면의 `/` 와 같은 손). 걸린 찾기는 창의 Esc 가
//! 먼저 푼다(2026-10-04 사용자 결정). 본 화면의 `/` 와 `moai show -g` 는 위키를 안 본다 — 트래커의 줄을 찾는 자리다.
//!
//! **길찾기는 고르기 창이다**([`Choose`]) — 본문 칸의 `Enter` 가 그 페이지의 링크와 이슈 id 를 한 창에 세운다. 페이지를
//! 고르면 그리로 건너가고, id 를 고르면 창을 닫고 그 줄에 선다(`App::land` — 한눈 보기에서 열었으면 그 프로젝트로 들어가
//! 선다, 2026-10-04 사용자 결정). 풀리지 않는 링크와 위키 밖 주소, 트래커에 없는 id 도 창에 서고 고르면 알림만 낸다 —
//! 말없이 빠지면 그 링크가 왜 안 가는지 볼 자리가 없다. 본문 안에서 링크를 `Tab` 으로 도는 길은 기획이 안 골랐다: 펴진
//! 줄에는 주소가 표식으로만 남아 다시 세야 한다(moai-qunn 노트 C).
//!
//! **되돌아가기는 링크로 건너온 길만 되감는다**([`Window::back`]) — 목록에서 커서를 옮긴 것은 고른 것이라 자취에
//! 안 남는다. 자취가 남았어도 Esc 는 창을 닫는다(같은 결정): 되감는 키는 `Bksp`·`h` 하나다.
//!
//! **어느 위키인가**: 프로젝트 안이면 그 체크아웃(`Repo::here`)의 위키 — 딸린 워크트리에서 띄웠으면 그 가지의
//! 페이지다(`moai wiki ls` 와 같다). 한눈 보기(`0`)면 **커서가 선 줄의 프로젝트**다(통계 창과 같은 규칙). 여러
//! 프로젝트의 위키를 한 목록에 섞지 않는다.

use super::input::Input;
use super::keys::{self, Browse, Chord, LEADER, LINKS, Lookup, PROMPT, Prompt, WIKI};
use super::layer::Depth;
use super::menu;
use super::scroll::{self, Move, Scroll};
use super::{App, Landing, Mode, Pane, Row, Seat, Veil};
use crate::i18n::{fill, say};
use crate::wiki::{DirTrouble, Page, Wiki};
use ratatui::crossterm::event::KeyEvent;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 페이지가 댄 id 가운데 트래커에 있는 것의 제목 — 고르기 창이 id 곁에 댄다. 연 순간 그 줄들에서 한 번 모은다.
type Titles = BTreeMap<String, String>;

/// 포커스가 선 칸. 목록이 늘 왼쪽, 본문이 오른쪽이다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    /// 왼쪽 페이지 목록. **여기서 시작한다** — 탐색기와 같다.
    #[default]
    List,
    /// 오른쪽 본문.
    Page,
}

/// 창의 상태 — 연 순간 읽은 페이지들과 커서·굴린 자리·건너온 자취.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// 어느 프로젝트의 위키인가 — 프로젝트 이름.
    pub project: String,
    /// `wiki_dir` 의 날글자 — 제목이 댄다.
    pub dir: String,
    /// 한눈 보기에서 열었으면 그 프로젝트의 자리. 고른 id 가 그 프로젝트로 들어가 선다(2026-10-04 사용자 결정).
    /// **첨자가 아니라 경로다** — 창이 떠 있는 동안 층의 줄이 다시 서면 첨자는 다른 프로젝트를 가리킨다.
    pub from: Option<PathBuf>,
    pages: Vec<Page>,
    /// 찾기가 견줄 글 — 페이지마다 `(제목, 본문)` 을 소문자로 접은 것, [`Window::pages`] 와 같은 차례. **연 순간 한 번
    /// 접는다** — 창은 그 사이 글이 안 바뀌는데, 견줄 때마다 접으면 커서·그림·치는 키마다 위키 전부(생성 페이지
    /// `docs/cli.md` 만 128KB)를 몇 번씩 다시 접는다.
    folded: Vec<(String, Option<String>)>,
    /// 걷다가 페이지로 못 세운 자리 수 — 목록 밑에 선다. 말없이 빠지면 그 밑의 페이지가 없는 줄 안다.
    pub skipped: usize,
    /// 목록에서 선 줄 — 보이는 페이지([`Window::shown`])의 차례다.
    pub cursor: usize,
    /// 목록의 굴린 자리 — 그림이 커서를 드러낸다. 굴려 떼어 놓았으면([`Window::adrift`]) 안 드러낸다.
    pub list: Scroll,
    /// 목록 칸의 화면을 굴려 커서에서 떼어 놓았는가 — 휠·반 쪽·한 쪽이 목록 칸을 굴리면 선다(moai-og9h, 사용자 결정
    /// 2026-10-04 — 탐색기의 `App::adrift` 와 같은 규칙). **선 동안 그림은 커서를 드러내지 않는다**: 고른 페이지가 화면
    /// 밖으로 나가도 커서와 본문은 그대로다. 걷는 자는 커서를 옮기는 길 하나([`Window::move_to`] — 키·누르기·링크·
    /// 되돌아가기)와 목록을 다시 세우는 찾기([`Window::refilter`])다. 창이 떠 있는 동안 페이지를 다시 안 읽으니
    /// 탐색기처럼 줄의 정체를 들 까닭이 없다.
    pub(super) adrift: bool,
    /// 본문의 굴린 자리.
    pub page: Scroll,
    pub focus: Side,
    /// 링크로 건너오기 전의 페이지와 그때 굴린 자리 — 되돌아가면 읽던 줄로 돌아간다.
    trail: Vec<(String, usize)>,
    /// 접두어(`gg`·`Ctrl-w`)를 기다리는 열 — 탐색의 열과 따로다(통계 창과 같다).
    pub chord: Chord,
    /// 본문 칸에 선 페이지를 펴 둔 한 벌 — 그리는 쪽(`draw::wiki_page`)이 채우고 읽는다. 값일 뿐이라 없어도 그림이 같다.
    pub(super) laid: Option<super::draw::WikiLaid>,
    /// 걸린 찾기 — 친 글 그대로. 비면 없다.
    search: Option<String>,
    /// 찾는 글을 치는 중 — 칸과, 칸을 열기 전의 찾기. Esc 가 그 찾기로 돌아간다.
    pub typing: Option<Typing>,
    /// 연 순간의 id 제목.
    titles: Titles,
    /// 링크·id 고르기 창 — 본문 칸의 `Enter` 가 연다.
    pub choose: Option<Choose>,
}

/// 링크와 id 를 고르는 창 — 연 순간의 페이지에서 지은 고를 것과 커서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choose {
    pub items: Vec<Target>,
    pub cursor: usize,
    pub list: Scroll,
    /// 접두어(`gg`)를 기다리는 열 — 창의 열과 따로다. 키 하나씩 찾으면 `g` 가 늘 접두어라 `gg` 가 안 선다.
    pub chord: Chord,
}

/// 고를 것 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 위키 안의 페이지로 가는 링크 — `found` 가 거짓이면 그 슬러그의 파일이 없다(`(없음)`).
    Page { text: String, to: String, found: bool },
    /// 위키 밖 주소 — 바깥 URL·앵커·`.md` 아닌 파일·위키 뿌리 위(`(밖)`). 고르면 주소를 알림으로 댄다.
    Out { text: String, dest: String },
    /// 이슈 id — `title` 이 없으면 트래커에 그 줄이 없다(`(없음)`).
    Issue { id: String, title: Option<String> },
}

/// 찾는 글을 치는 칸(`/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typing {
    pub input: Input,
    /// 칸을 열기 전에 걸려 있던 찾기 — Esc 가 되돌린다.
    was: Option<String>,
    /// 칸을 열 때 보던 페이지 — **치는 동안 돌아갈 자리는 늘 이것이다**([`Window::typed_slug`]). 치는 동안 커서는 사람이
    /// 고른 적이 없다: 친 글자 하나가 그 페이지를 가려 커서가 첫 줄로 가도, 지우면 그 페이지로 돌아오고 Esc 도 그리로
    /// 간다. 지금 선 페이지를 따르면 가렸다 다시 보이는 사이에 보던 페이지를 잃는다.
    from: Option<String>,
}

impl Window {
    fn new(project: String, dir: String, from: Option<PathBuf>, wiki: Wiki, titles: Titles) -> Window {
        let folded =
            wiki.pages.iter().map(|p| (p.title.to_lowercase(), p.body.as_deref().map(str::to_lowercase))).collect();
        Window {
            project,
            dir,
            from,
            pages: wiki.pages,
            folded,
            skipped: wiki.skipped.len(),
            cursor: 0,
            list: Scroll::default(),
            adrift: false,
            page: Scroll::default(),
            focus: Side::List,
            trail: Vec::new(),
            chord: Chord::default(),
            laid: None,
            search: None,
            typing: None,
            titles,
            choose: None,
        }
    }

    /// 본문 칸의 `Enter` — 커서가 선 페이지의 링크(적힌 차례)와 id(처음 나온 차례)로 고르기 창을 연다. 고를 것이
    /// 없으면 거짓이다 — 부르는 쪽이 알림으로 댄다.
    fn open_links(&mut self) -> bool {
        let Some(p) = self.current() else { return false };
        let mut items: Vec<Target> = Vec::new();
        for (text, dest) in p.body.as_deref().map(crate::wiki::links_in).unwrap_or_default() {
            items.push(match crate::wiki::target(&p.slug, &dest) {
                Some(to) => {
                    let found = self.pages.iter().any(|q| q.slug == to);
                    Target::Page { text, to, found }
                }
                None => Target::Out { text, dest },
            });
        }
        for r in &p.issues {
            let title = r.exists.then(|| self.titles.get(&r.id).cloned().unwrap_or_default());
            items.push(Target::Issue { id: r.id.clone(), title });
        }
        if items.is_empty() {
            return false;
        }
        self.choose = Some(Choose { items, cursor: 0, list: Scroll::default(), chord: Chord::default() });
        true
    }

    /// 지금 찾는 글 — 치는 중이면 칸의 글(치는 대로 좁힌다), 아니면 걸린 찾기. 빈 글은 없다.
    pub fn query(&self) -> Option<&str> {
        let q = match &self.typing {
            Some(t) => Some(t.input.text()),
            None => self.search.as_deref(),
        };
        q.filter(|q| !q.trim().is_empty())
    }

    /// 걸린 찾기가 있는가 — 치는 중이 아닐 때의 Esc 가 그것을 푼다.
    pub fn searched(&self) -> bool {
        self.typing.is_none() && self.query().is_some()
    }

    /// 목록에 선 페이지들, 차례대로 — 찾는 글이 있으면 **제목이나 본문에 그 글이 든** 페이지만이다. 견주는 법은
    /// 거름망과 같은 `to_lowercase` 다(`query::Filter`) — 페이지 쪽은 연 순간 접어 둔 것([`Window::folded`])이다. 못
    /// 읽은 페이지는 제목으로만 걸린다.
    pub fn shown(&self) -> Vec<&Page> {
        let Some(q) = self.query().map(str::to_lowercase) else { return self.pages.iter().collect() };
        self.pages
            .iter()
            .zip(&self.folded)
            .filter(|(_, (title, body))| title.contains(&q) || body.as_deref().is_some_and(|b| b.contains(&q)))
            .map(|(p, _)| p)
            .collect()
    }

    /// 찾는 글이 바뀐 뒤 커서를 다시 세운다 — **보던 페이지가 남았으면 그 페이지에** 선다. 빠졌으면 첫 줄이고, 본문은
    /// 첫 줄로 돌아간다.
    ///
    /// 굴려 떼어 둔 화면([`Window::adrift`])은 걷는다 — 굴린 자리는 좁히기 전 목록의 줄이다.
    fn refilter(&mut self, was: Option<String>) {
        self.adrift = false;
        let at = was.and_then(|slug| self.shown().iter().position(|p| p.slug == slug));
        match at {
            Some(at) => self.cursor = at,
            None => {
                self.cursor = 0;
                self.page.rewind();
            }
        }
    }

    /// `/` — 찾는 칸을 연다. 걸린 찾기가 있으면 그 글로 연다(본 화면의 `/` 는 빈 칸이지만, 이 창은 한 칸뿐이라 고쳐
    /// 치는 쪽이 흔하다).
    fn start_search(&mut self) {
        let was = self.search.clone();
        let from = self.current().map(|p| p.slug.clone());
        self.typing = Some(Typing { input: Input::new(was.as_deref().unwrap_or_default()), was, from });
    }

    /// 치는 동안 커서를 다시 세울 페이지 — 칸을 열 때 보던 페이지([`Typing::from`]), 그것이 없었으면 지금 선 페이지다.
    fn typed_slug(&self) -> Option<String> {
        self.typing.as_ref().and_then(|t| t.from.clone()).or_else(|| self.current().map(|p| p.slug.clone()))
    }

    /// 치는 칸의 키 하나 — 칸이 먹으면 치는 대로 좁히고, 안 먹은 Enter 는 걸고 Esc 는 열기 전으로 돌아간다.
    fn type_key(&mut self, k: KeyEvent) {
        let slug = self.typed_slug();
        let Some(t) = &mut self.typing else { return };
        if t.input.key(k) {
            return self.refilter(slug);
        }
        match keys::lookup(PROMPT, &[k]) {
            Lookup::Run(Prompt::Apply) => {
                let text = t.input.text().trim().to_string();
                self.search = (!text.is_empty()).then_some(text);
                self.typing = None;
                self.refilter(slug);
            }
            Lookup::Run(Prompt::Cancel) => {
                self.search = t.was.take();
                self.typing = None;
                self.refilter(slug);
            }
            _ => {}
        }
    }

    /// 붙여넣기 — 치는 칸이 있으면 거기 넣고 좁힌다. 없으면 아무 일도 없다(통계 창과 같다).
    pub(super) fn paste(&mut self, s: &str) {
        let slug = self.typed_slug();
        if let Some(t) = &mut self.typing {
            t.input.paste(s);
            self.refilter(slug);
        }
    }

    /// 걸린 찾기를 푼다 — 보던 페이지에 그대로 선다.
    fn clear_search(&mut self) {
        let slug = self.current().map(|p| p.slug.clone());
        self.search = None;
        self.refilter(slug);
    }

    /// 커서가 선 페이지 — 본문 칸이 그리는 것.
    pub fn current(&self) -> Option<&Page> {
        self.shown().get(self.cursor).copied()
    }

    /// 건너온 자취가 남았는가 — 바가 `Bksp` 를 댈지 이것으로 가른다.
    pub fn can_go_back(&self) -> bool {
        !self.trail.is_empty()
    }

    /// 휠 한 칸(moai-o3cb) — **포인터 아래 칸이 받는다**(2026-10-04 사용자 결정, 탐색기와 같다): 목록이든 본문이든 그
    /// 칸의 화면을 굴린다. 목록 위의 휠은 한때 커서를 옮겼다 — 탐색기 목록의 2026-10-01 결정을 따랐던 것이고, 탐색기와
    /// 함께 뒤집었다(moai-og9h, [`Window::roll_list`]). 포커스는 안 옮기고, 기다리던 접두어는 버린다 — 휠을 사이에 둔
    /// `g` 와 `g` 가 `gg` 로 이으면 사람이 친 적 없는 맨 위로 가기가 선다(`stats::Window::roll` 과 같은 까닭).
    pub(super) fn roll(&mut self, on: Side, by: isize) {
        self.chord.clear();
        match on {
            Side::List => self.roll_list(|s| s.by(by)),
            Side::Page => self.page.by(by),
        }
    }

    /// 이동 하나를 **포커스 칸에** 준다 — 목록이면 커서, 본문이면 굴리기. 탐색기의 `App::step` 과 같은 자다: 목록의
    /// 반 쪽·한 쪽은 커서가 아니라 화면을 굴린다(moai-og9h).
    pub fn step(&mut self, m: Move) {
        match self.focus {
            Side::List if matches!(m, Move::HalfUp | Move::HalfDown | Move::PageUp | Move::PageDown) => {
                self.roll_list(|s| s.go(m));
            }
            Side::List => {
                let len = self.shown().len();
                self.move_to(scroll::cursor(m, self.cursor, || len));
            }
            Side::Page => self.page.go(m),
        }
    }

    /// 목록 칸의 화면만 굴린다 — 휠과 반 쪽·한 쪽이다(moai-og9h, 탐색기의 `App::roll` 과 같은 규칙). 커서와 본문은
    /// 그대로고, 굴렀으면 [`Window::adrift`] 를 세워 다음 그림이 커서를 드러내느라 화면을 되돌리지 않게 한다. 안
    /// 굴렀으면(끝에 닿았거나 목록이 칸에 다 든다) 새로 뗄 것이 없다.
    fn roll_list(&mut self, roll: impl FnOnce(&mut Scroll)) {
        let was = self.list.offset();
        roll(&mut self.list);
        if self.list.offset() != was {
            self.adrift = true;
        }
    }

    /// 커서를 옮기고, 다른 페이지로 갔으면 **본문을 첫 줄로 되돌린다** — 탐색기의 `App::move_to` 와 같은 까닭이다.
    ///
    /// 굴려 떼어 둔 목록([`Window::adrift`])은 걷는다 — 옮겼든 끝이라 제자리든 커서 키 하나가 화면을 커서로
    /// 되돌린다. 링크·되돌아가기도 이 길로 그 페이지에 서니 함께 드러난다.
    pub(super) fn move_to(&mut self, at: usize) {
        self.adrift = false;
        if at != self.cursor {
            self.page.rewind();
        }
        self.cursor = at;
    }

    /// 그 슬러그의 페이지로 커서를 옮긴다 — 있으면 참. **걸린 찾기가 그 페이지를 가렸으면 찾기를 푼다** — 링크를 따라
    /// 간 사람은 그 페이지를 보러 간 것이고, 안 풀면 건너뛸 데가 없어 링크가 죽는다.
    fn land_on(&mut self, slug: &str) -> bool {
        if !self.pages.iter().any(|p| p.slug == slug) {
            return false;
        }
        if !self.shown().iter().any(|p| p.slug == slug) {
            self.search = None;
        }
        let Some(at) = self.shown().iter().position(|p| p.slug == slug) else { return false };
        self.move_to(at);
        true
    }

    /// 링크를 따라 그 페이지로 간다 — 지금 페이지와 읽던 자리를 자취에 민다. 그 페이지가 없으면 아무것도 안 바꾸고
    /// 거짓이다.
    pub(super) fn follow(&mut self, to: &str) -> bool {
        let Some(here) = self.current().map(|p| p.slug.clone()) else { return false };
        let read = self.page.offset();
        if !self.land_on(to) {
            return false;
        }
        self.trail.push((here, read));
        // 같은 페이지로 가는 링크(`#앵커` 를 뗀 제 자신)도 자취에 남는다 — 되돌아가면 읽던 줄로 온다.
        self.page.rewind();
        true
    }

    /// 건너오기 전의 페이지로 한 걸음 — 읽던 줄로 돌아간다. 자취가 비었으면 거짓이다.
    pub fn back(&mut self) -> bool {
        while let Some((slug, read)) = self.trail.pop() {
            if self.land_on(&slug) {
                self.page.rewind();
                self.page.by(read as isize);
                return true;
            }
        }
        false
    }

    /// 다른 칸으로 — 칸이 둘이라 다음과 앞이 같다.
    fn focus_next(&mut self) {
        self.focus = match self.focus {
            Side::List => Side::Page,
            Side::Page => Side::List,
        };
    }
}

impl App {
    /// `SPC g w` — 창을 연다. 읽을 위키를 못 정하거나 못 읽으면, 또 페이지가 하나도 없으면 알림 한 줄로 까닭을 대고
    /// 안 연다 — 통계 창이 못 셀 때와 같다(2026-10-04, 일꾼이 정했다 — 빈 창에는 할 일이 없다).
    pub(super) fn open_wiki(&mut self) {
        let lang = self.site.lang;
        if !self.on_layer() {
            let Some(repo) = self.site.repo.as_ref() else {
                self.notice = Some(say(lang, "tui.wiki.no_project").to_string());
                return;
            };
            let read = read_wiki(repo, &self.site.issues, &self.site.unreadable);
            let (dir, name) = (repo.config.wiki_dir.clone(), self.project_name());
            return self.show_wiki(name, dir, None, read);
        }
        // **한눈 보기에는 "지금 선 프로젝트" 가 없다 — 커서가 댄다**(`App::open_stats` 와 같은 규칙).
        let at = match self.current() {
            Some(Row::Project(at)) | Some(Row::Item(Seat::Place(at), ..)) => at,
            _ => {
                self.notice = Some(say(lang, "tui.wiki.no_target").to_string());
                return;
            }
        };
        let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
        let (name, path) = (place.name.clone(), place.path.clone());
        // **펼쳐 든 줄이 있으면 그 저장소와 줄을 쓴다** — id 가 있는가는 화면에 선 그 줄로 잰다.
        if let Some(site) = self.site_of_seat(Seat::Place(at))
            && let Some(repo) = site.repo.as_ref()
        {
            let read = read_wiki(repo, &site.issues, &site.unreadable);
            let dir = repo.config.wiki_dir.clone();
            return self.show_wiki(name, dir, Some(path), read);
        }
        // **접혀 아직 안 읽은 프로젝트는 그 자리에서 연다** — 통계 창과 같은 길이다(`App::open_stats`). 못 열 때만
        // 그 줄을 고쳐 세우는 길(`open_place`)로 간다 — 까닭을 알림으로 대는 자가 거기 하나다.
        let repo = match crate::projects::open_shallow(&path, lang) {
            Ok(repo) => repo,
            Err(_) => {
                let Some(repo) = self.open_place(at, Depth::Lean) else { return };
                repo
            }
        };
        // id 가 있는가만 물으면 되니 스냅샷만 읽는다 — 겹쳐 보기와 요약은 위키가 안 쓴다.
        match repo.read() {
            Ok(load) => {
                let unreadable: Vec<Option<String>> = load.reserved_ids().into_iter().map(Some).collect();
                let read = read_wiki(&repo, &load.issues, &unreadable);
                let dir = repo.config.wiki_dir.clone();
                self.show_wiki(name, dir, Some(path), read);
            }
            Err(e) => {
                self.notice = Some(fill(say(lang, "tui.wiki.unread"), &[("name", &name), ("why", &e.message)]));
            }
        }
    }

    /// 읽은 위키로 창을 세운다 — 못 쓰는 디렉터리나 빈 위키면 알림으로 까닭을 댄다. **창을 세우는 자리는 여기
    /// 하나다** — 프로젝트 안과 한눈 보기의 두 갈래가 다 이리 온다.
    fn show_wiki(
        &mut self,
        name: String,
        dir: String,
        from: Option<PathBuf>,
        read: Result<(Wiki, Titles), DirTrouble>,
    ) {
        let lang = self.site.lang;
        let (wiki, titles) = match read {
            Ok(w) => w,
            // 말은 `moai wiki ls` 와 한 자다(`view::wiki::dir_trouble`) — 없는 디렉터리면 첫 페이지 자리까지 댄다.
            Err(t) => {
                self.notice = Some(crate::view::wiki::dir_trouble(lang, &dir, &t));
                return;
            }
        };
        if wiki.pages.is_empty() {
            let mut said = crate::view::wiki::list(lang, &dir, &wiki).join(" ");
            // **못 걸은 자리가 있으면 그 수도 댄다** — 창이 안 서니 목록 밑의 수([`Window::skipped`])가 설 자리가 없다.
            // 빼면 "첫 페이지는 README.md" 만 남아, 디렉터리 링크 밑에 둔 페이지가 정말 없는 줄 안다.
            if !wiki.skipped.is_empty() {
                let n = wiki.skipped.len().to_string();
                said = format!("{said} · {}", fill(say(lang, "tui.wiki.skipped"), &[("n", &n)]));
            }
            self.notice = Some(said);
            return;
        }
        // **연 키는 탐색의 열을 비운 뒤에 온다** — 메뉴로 열었으면 메뉴가 이미 닫혔다(`menu::feed`).
        self.mode = Mode::Wiki(Box::new(Window::new(name, dir, from, wiki, titles)));
    }

    /// 고르기 창의 키 하나(본문 칸의 `Enter` 가 연 창). 고른 것이 페이지면 그리로 건너가고, id 면 창을 닫고 그 줄에 서며,
    /// 갈 데가 없는 것은 알림으로 까닭을 댄다. 어느 쪽이든 고르면 고르기 창은 닫힌다.
    ///
    /// **이 키가 알림을 걷는가**를 답한다([`App::wiki_key`] 와 같은 규칙) — 둘째 키를 기다리는 `g` 만 거짓이다.
    fn choose_key(&mut self, k: KeyEvent) -> bool {
        let lang = self.site.lang;
        let Mode::Wiki(w) = &mut self.mode else { return false };
        let Some(c) = &mut w.choose else { return false };
        let picked = match c.chord.feed(LINKS, k) {
            Some(keys::Link::Step(m)) => {
                let len = c.items.len();
                c.cursor = scroll::cursor(m, c.cursor, || len);
                return true;
            }
            Some(keys::Link::Close) => {
                w.choose = None;
                return true;
            }
            Some(keys::Link::Enter) => c.items.get(c.cursor).cloned(),
            // 뜻 없는 키는 열을 버리므로([`keys::Chord::feed`]) 남은 열이 곧 "기다린다" 다.
            None => return c.chord.held().is_empty(),
        };
        w.choose = None;
        // 말의 키는 글자째 적는다 — `i18n` 의 시험이 `say(lang, "…")` 을 훑어 표와 견준다.
        let one = crate::text::one_line;
        match picked {
            Some(Target::Page { to, found: true, .. }) => {
                w.follow(&to);
            }
            Some(Target::Page { to, found: false, .. }) => {
                self.notice = Some(fill(say(lang, "tui.wiki.missing_page"), &[("to", &one(&to))]));
            }
            Some(Target::Out { dest, .. }) => {
                self.notice = Some(fill(say(lang, "tui.wiki.outside"), &[("dest", &one(&dest))]));
            }
            Some(Target::Issue { id, title: None }) => {
                self.notice = Some(fill(say(lang, "tui.wiki.missing_id"), &[("id", &one(&id))]));
            }
            Some(Target::Issue { id, title: Some(_) }) => {
                let from = w.from.clone();
                self.wiki_land(&id, from);
            }
            None => {}
        }
        true
    }

    /// 고른 id 의 줄에 선다 — 창을 닫고 목록으로 돌아가 `App::land` 가 그 줄의 집으로 간다. **한눈 보기에서 연
    /// 창이면 그 프로젝트로 먼저 들어간다**(2026-10-04 사용자 결정) — 헤더 번호가 들어가는 길(`App::enter_project`)과
    /// 같다. 못 들어가면 그 길이 단 알림이 까닭을 댄다. 거름망·보기가 그 줄을 가렸으면 **무엇이 가렸는지 가려** 그것을
    /// 푸는 키만 대고 커서는 그대로 둔다 — 쓰기 뒤의 착지와 같은 갈래다(moai-fmv5): 거름망이 없는데 Esc 를 대거나,
    /// 아카이브라 가린 줄에 모두 보이기를 대면 누른 키가 아무것도 안 한다. 보기 쪽 말은 [`App::veiled_row`] 가 짓는다.
    fn wiki_land(&mut self, id: &str, from: Option<PathBuf>) {
        let lang = self.site.lang;
        self.mode = Mode::Browse;
        self.focus = Pane::Explorer;
        if let Some(path) = from {
            let Some(at) = self.layer.as_ref().and_then(|l| l.places.iter().position(|p| p.path == path)) else {
                self.notice = Some(fill(say(lang, "tui.wiki.gone"), &[("id", id)]));
                return;
            };
            self.enter_project(at);
            if self.layer.as_ref().and_then(super::layer::Layer::number) != Some(at + 1) {
                return;
            }
        }
        match self.land(id) {
            Landing::Shown => {}
            Landing::Hidden => {
                let clear = keys::label(keys::BROWSE, Browse::ClearFilter);
                let filtered = || fill(say(lang, "tui.wiki.veiled_filter"), &[("id", id), ("clear", &clear)]);
                let told = match self.site.index.find(id).map(|at| (at, self.site.veil(at))) {
                    // **둘 다 가렸으면 둘 다 댄다** — 하나만 대면 그 키를 눌러도 다른 쪽에 여전히 가린다(moai-2kyl).
                    Some((at, Veil { filtered: true, viewed: true })) => {
                        format!("{} · {}", filtered(), self.veiled_row(id, at))
                    }
                    Some((at, Veil { filtered: false, viewed: true })) => self.veiled_row(id, at),
                    Some((_, Veil { filtered: true, viewed: false })) => filtered(),
                    // 둘 다 안 가렸는데 줄이 안 섰다 — 오늘은 닿지 않는 갈래다. 누를 키를 대면 아무것도 안 한다(moai-1jay).
                    _ => fill(say(lang, "tui.wiki.gone"), &[("id", id)]),
                };
                self.notice = Some(told);
            }
            Landing::Missing => self.notice = Some(fill(say(lang, "tui.wiki.gone"), &[("id", id)])),
        }
    }

    /// 위키 창의 키 하나. **닫으면 탐색으로 돌아갈 뿐이다** — 커서·걸린 거름망·상세는 창이 안 건드려 그대로다.
    ///
    /// **SPC 는 메뉴를 연다** — 통계 창과 같은 자([`App::stats_key`] 의 몸)로 받고, 서는 것은 화면 고르기(`SPC g`)와
    /// 원문·그린 글(`SPC v r`)이다(`keys::Ctx::wiki`). 메뉴를 닫는 이동키는 그 자리에서 **창의 포커스 칸을** 움직인다.
    ///
    /// **이 키가 알림을 걷는가**를 답한다 — 통계 창과 같은 규칙이다: 메뉴만 만진 키와 둘째 키를 기다리는 열은 거짓,
    /// 창의 키와 화면을 고른 키, 모르는 키는 참이다.
    pub(super) fn wiki_key(&mut self, k: KeyEvent) -> bool {
        // **치는 동안은 칸이 먼저다** — 이동키도 SPC 도 글자다(본 화면의 글칸과 같다).
        if let Mode::Wiki(w) = &mut self.mode
            && w.typing.is_some()
        {
            w.type_key(k);
            return true;
        }
        // **고르기 창이 떠 있으면 그것이 키를 받는다** — SPC 도 그 창에서는 뜻이 없다(고르기 창의 표에 없다).
        if let Mode::Wiki(w) = &self.mode
            && w.choose.is_some()
        {
            return self.choose_key(k);
        }
        if menu::open(&self.chord) || LEADER.matches(k) {
            if let Mode::Wiki(w) = &mut self.mode {
                w.chord.clear();
            }
            let rows = self.rows();
            let ctx = self.key_ctx(&rows);
            let act = menu::feed(&mut self.chord, &ctx, k);
            // 메뉴를 닫은 접두어(`g`)는 탐색의 열에 남는다 — 창의 열로 옮겨 둘째 키가 잇게 한다.
            if !menu::open(&self.chord) {
                let held = self.chord.held().to_vec();
                self.chord.clear();
                if let Mode::Wiki(w) = &mut self.mode {
                    for k in held {
                        w.chord.feed(WIKI, k);
                    }
                }
            }
            return match act {
                Some(Browse::Go(to)) => {
                    self.go(to, &rows);
                    true
                }
                Some(Browse::Step(m)) => {
                    if let Mode::Wiki(w) = &mut self.mode {
                        w.step(m);
                    }
                    false
                }
                // **원문·그린 글은 탐색과 한 값이다**(`App::raw`) — 설정에 안 남는 값이라 창이 따로 들 까닭이 없고, 닫고
                // 돌아간 상세도 같은 꼴로 선다. 줄 수가 바뀌니 두 굴린 자리를 첫 줄로 돌린다(탐색의 `B::Raw` 와 같다).
                // 메뉴는 열린 채로 남는다(`Browse::stateful`).
                Some(Browse::Raw) => {
                    self.raw = !self.raw;
                    self.detail.rewind();
                    if let Mode::Wiki(w) = &mut self.mode {
                        w.page.rewind();
                    }
                    true
                }
                _ => false,
            };
        }
        let Mode::Wiki(w) = &mut self.mode else { return false };
        match w.chord.feed(WIKI, k) {
            Some(keys::Wiki::Step(m)) => w.step(m),
            // 목록에서는 본문 칸으로 — 본문은 이미 커서의 페이지다. 본문에서는 고르기 창이다.
            Some(keys::Wiki::Enter) if w.focus == Side::List => w.focus = Side::Page,
            Some(keys::Wiki::Enter) => {
                if !w.open_links() {
                    self.notice = Some(say(self.site.lang, "tui.wiki.no_links").to_string());
                }
            }
            // 자취가 없으면 아무 일도 없다 — 바도 그때는 `Bksp` 를 안 댄다.
            Some(keys::Wiki::Back) => {
                w.back();
            }
            Some(keys::Wiki::Search) => w.start_search(),
            Some(keys::Wiki::FocusNext) => w.focus_next(),
            Some(keys::Wiki::Focus(side)) => {
                w.focus = if side == keys::Side::Left { Side::List } else { Side::Page };
            }
            // 걸린 찾기를 먼저 푼다 — 본 화면의 Esc 가 거름망을 푸는 것과 같은 손이다(2026-10-04 사용자 결정).
            Some(keys::Wiki::Close) if w.searched() => w.clear_search(),
            Some(keys::Wiki::Close) => self.mode = Mode::Browse,
            // 뜻 없는 키는 열을 버리므로([`keys::Chord::feed`]) 남은 열이 곧 "기다린다" 다.
            None => return w.chord.held().is_empty(),
        }
        true
    }
}

/// 저장소 하나의 위키를 읽는다 — `wiki_dir` 은 그 저장소의 설정, id 가 있는가는 `issues` 와 못 읽는 줄의 id 로 잰다.
/// **못 읽는 줄이 쓰는 id 도 있는 id 다** — `moai wiki ls` 와 같은 셈이다(`cmd::wiki::read`).
///
/// 페이지가 댄 id 가운데 줄이 있는 것은 제목을 함께 모은다([`Titles`]) — 고르기 창이 id 곁에 댄다. 못 읽는 줄의 id 는
/// 제목이 없어 빈 글이다.
fn read_wiki(
    repo: &crate::store::Repo,
    issues: &[crate::model::Issue],
    unreadable: &[Option<String>],
) -> Result<(Wiki, Titles), DirTrouble> {
    let ids: std::collections::HashSet<&str> =
        issues.iter().map(|i| i.id.as_str()).chain(unreadable.iter().flatten().map(String::as_str)).collect();
    let known = |id: &str| ids.contains(id);
    let wiki = crate::wiki::load(repo.here(), &repo.config.wiki_dir, &repo.config.prefix, &known)?;
    let named: std::collections::HashSet<&str> =
        wiki.pages.iter().flat_map(|p| p.issues.iter()).filter(|r| r.exists).map(|r| r.id.as_str()).collect();
    let titles =
        issues.iter().filter(|i| named.contains(i.id.as_str())).map(|i| (i.id.clone(), i.title.clone())).collect();
    Ok((wiki, titles))
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::{App, Mode, Pane, draw};
    use super::{Side, Window};
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path;
    use crate::scratch::Scratch;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    /// 위키 페이지 셋을 든 임시 저장소와 그 저장소를 든 탐색기 — 홈(`README`)이 둘째 페이지와 이슈 하나를 댄다.
    pub(in crate::tui) fn wiki_app(name: &str, pages: &[(&str, &str)]) -> (Scratch, App) {
        let s = Scratch::new(&format!("tui-wiki-{name}"));
        std::fs::create_dir_all(s.path().join("docs")).unwrap();
        for (file, body) in pages {
            let at = s.path().join("docs").join(file);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, body).unwrap();
        }
        let issues = vec![Issue::new(
            "argos-0001".into(),
            "첫 일".into(),
            Kind::Issue,
            Status::new("todo"),
            "2026-09-01T00:00:00Z",
        )];
        let mut a = App::new(issues, cfg(), Path::new());
        a.site.repo = Some(crate::store::Repo::at(s.path().to_path_buf(), cfg()));
        (s, a)
    }

    /// 홈과 두 페이지 — 홈이 `guide` 를 링크하고 `guide` 가 `notes/deep` 을 링크한다.
    pub(in crate::tui) const PAGES: &[(&str, &str)] = &[
        ("README.md", "# Home\n\nRead the [guide](guide.md) and argos-0001.\n"),
        ("guide.md", "# Guide\n\nGo [deeper](notes/deep.md).\n\nLine\n\nLine\n"),
        ("notes/deep.md", "# Deep\n\nThe bottom.\n"),
    ];

    pub(in crate::tui) fn window(a: &App) -> &Window {
        match &a.mode {
            Mode::Wiki(w) => w,
            other => panic!("위키 창이 안 열렸다 — {other:?}"),
        }
    }

    fn slug(a: &App) -> String {
        window(a).current().map(|p| p.slug.clone()).unwrap_or_default()
    }

    /// **`SPC g w` 가 창을 열고, Esc 는 보던 화면을 그대로 돌려준다** — 커서·포커스·상세를 창이 안 건드린다. 목록은
    /// 홈이 먼저고 나머지는 제목순이며, 본문은 목록 커서를 따라간다.
    #[test]
    fn spc_g_w_opens_the_window_and_esc_gives_the_screen_back_as_it_was() {
        let (_s, mut a) = wiki_app("open", PAGES);
        a.focus = Pane::Detail;
        let before = (a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail);
        a.hit("SPC g w");
        let w = window(&a);
        assert_eq!(w.shown().iter().map(|p| p.slug.as_str()).collect::<Vec<_>>(), ["README", "notes/deep", "guide"]);
        assert_eq!((w.focus, w.cursor, w.dir.as_str(), w.from.clone()), (Side::List, 0, "docs", None));
        a.hit("j");
        assert_eq!(slug(&a), "notes/deep", "본문이 커서를 안 따라갔다");
        a.hit("G");
        assert_eq!(slug(&a), "guide");
        // `q` 는 창을 안 닫는다 — 탐색에서 아무것도 안 하는 글자다(moai-en4u).
        a.hit("q");
        window(&a);
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!((a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail), before, "창이 보던 자리를 바꿨다");
    }

    /// **목록의 Enter 는 본문 칸으로 간다** — 이동키는 포커스 칸을 움직인다(목록이면 커서, 본문이면 굴리기). 칸 사이는
    /// `Ctrl-w w`, `Ctrl-w h`·`l` 은 그쪽 칸이다.
    #[test]
    fn enter_moves_to_the_page_and_moves_go_to_the_focused_pane() {
        let (_s, mut a) = wiki_app("focus", PAGES);
        a.hit("SPC g w G Enter");
        assert_eq!(window(&a).focus, Side::Page);
        a.hit("j");
        assert_eq!((window(&a).cursor, window(&a).page.offset()), (2, 1), "본문에서 j 가 커서를 옮겼다");
        a.hit("Ctrl-w w");
        assert_eq!(window(&a).focus, Side::List);
        a.hit("Ctrl-w l");
        assert_eq!(window(&a).focus, Side::Page);
        a.hit("Ctrl-w h");
        assert_eq!(window(&a).focus, Side::List);
        // 다른 페이지로 가면 본문은 첫 줄로 돌아간다.
        a.hit("k");
        assert_eq!(window(&a).page.offset(), 0, "다른 페이지로 갔는데 굴린 자리가 남았다");
    }

    /// **되돌아가기는 링크로 건너온 길만 되감는다** — 읽던 줄로 돌아온다. 자취가 비면 `Bksp` 는 아무 일도 없고 창은
    /// 남는다. 자취가 있어도 Esc 는 창을 닫는다(2026-10-04 사용자 결정).
    #[test]
    fn back_unwinds_only_the_links_followed_and_esc_closes_anyway() {
        let (_s, mut a) = wiki_app("back", PAGES);
        a.hit("SPC g w");
        let Mode::Wiki(w) = &mut a.mode else { unreachable!() };
        w.page.by(1);
        assert!(w.follow("guide"));
        assert!(w.follow("notes/deep"));
        assert!(!w.follow("nope"), "없는 페이지로 갔다");
        assert_eq!(slug(&a), "notes/deep");
        a.hit("Bksp");
        assert_eq!(slug(&a), "guide");
        a.hit("h");
        assert_eq!((slug(&a), window(&a).page.offset()), ("README".to_string(), 1), "읽던 줄로 안 돌아왔다");
        assert!(window(&a).trail.is_empty());
        a.hit("Bksp");
        assert_eq!(slug(&a), "README", "자취가 빈 Bksp 가 무언가 했다");
        let Mode::Wiki(w) = &mut a.mode else { unreachable!() };
        w.follow("guide");
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse, "자취가 남았다고 Esc 가 창을 안 닫았다");
    }

    /// **위키 창 위의 SPC 는 화면 고르기와 원문·그린 글만 연다**(통계 창과 같은 자) — `SPC g l`·`b` 는 창을 닫고 그
    /// 배치로 서고, `SPC g w` 는 새로 읽어 처음으로 돌아가며, `SPC g s` 는 통계 창으로 간다. 메뉴의 이동키는 메뉴를
    /// 닫고 포커스 칸을 움직인다.
    #[test]
    fn spc_over_the_window_opens_only_the_screen_menu_and_raw() {
        use super::super::{keys::Screen, menu, view::Layout};
        let (_s, mut a) = wiki_app("menu", PAGES);
        a.hit("SPC g w SPC");
        assert!(menu::open(&a.chord));
        let ctx = a.key_ctx(&a.rows());
        assert_eq!(ctx.screen(), Screen::Wiki);
        let root = menu::entries(a.chord.held(), &ctx, &[]);
        assert_eq!(root.iter().map(menu::Entry::text).collect::<Vec<_>>(), ["+화면 [위키]", "+보기"]);
        a.hit("v");
        let v = menu::entries(a.chord.held(), &a.key_ctx(&a.rows()), &[]);
        assert_eq!(v.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["r"], "SPC v 밑에 원문 말고 다른 것이 섰다");
        a.hit("Esc");
        window(&a);

        a.hit("SPC j");
        assert!(!menu::open(&a.chord), "j 가 메뉴를 안 닫았다");
        assert_eq!(window(&a).cursor, 1, "메뉴를 닫은 j 가 커서를 안 옮겼다");
        a.hit("SPC g w");
        assert_eq!(window(&a).cursor, 0, "SPC g w 가 창을 새로 안 읽었다");
        a.hit("SPC g s");
        assert!(matches!(a.mode, Mode::Stats(_)), "위키 창의 SPC g s 가 통계로 안 갔다");
        a.hit("SPC g w");
        window(&a);
        a.hit("SPC g b");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::Board), "SPC g b 가 창을 닫고 보드로 안 갔다");
        a.hit("SPC g w SPC g l");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::List));
    }

    /// **못 열면 알림으로 까닭을 대고 안 연다** — 저장소가 없는 화면, 위키 디렉터리가 없는 저장소, 페이지가 하나도
    /// 없는 위키. 말은 `moai wiki ls` 와 같은 자다.
    #[test]
    fn nothing_to_read_says_why_and_opens_nothing() {
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.hit("SPC g w");
        assert_eq!(a.mode, Mode::Browse);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("위키")), "{:?}", a.notice);

        let (s, mut a) = wiki_app("empty", &[]);
        a.hit("SPC g w");
        assert_eq!(a.mode, Mode::Browse);
        assert!(
            a.notice.as_deref().is_some_and(|n| n.contains("README.md")),
            "빈 위키가 첫 자리를 안 댔다: {:?}",
            a.notice
        );
        // **못 걸은 자리만 있는 위키는 그 수를 함께 댄다** — 창이 안 서니 목록 밑의 수도 없다. 빼면 "첫 페이지는
        // README.md" 만 남아 페이지가 정말 없는 줄 안다.
        std::fs::create_dir_all(s.path().join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(s.path().join("elsewhere"), s.path().join("docs/linked")).unwrap();
        a.notice = None;
        a.hit("SPC g w");
        assert_eq!(a.mode, Mode::Browse);
        assert!(
            a.notice.as_deref().is_some_and(|n| n.contains("README.md") && n.contains("moai wiki ls")),
            "못 걸은 자리를 안 댔다: {:?}",
            a.notice
        );
        std::fs::remove_file(s.path().join("docs/linked")).unwrap();
        std::fs::remove_dir(s.path().join("docs")).unwrap();
        a.notice = None;
        a.hit("SPC g w");
        assert_eq!(a.mode, Mode::Browse);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("docs/")), "없는 디렉터리를 안 댔다: {:?}", a.notice);
    }

    /// **창 위의 메뉴도 알림을 탐색과 같은 자로 다룬다** — 메뉴만 만진 키와 기다리는 접두어는 알림을 안 걷고, 창의
    /// 키는 걷는다.
    #[test]
    fn the_menu_over_the_window_keeps_the_notice_until_a_key_acts() {
        let (_s, mut a) = wiki_app("notice", PAGES);
        a.hit("SPC g w");
        let said = "알림".to_string();
        a.notice = Some(said.clone());
        for keys in ["SPC", "g", "Bksp", "Esc", "SPC j", "Ctrl-w"] {
            a.hit(keys);
            assert_eq!(a.notice.as_deref(), Some(said.as_str()), "{keys:?} 가 알림을 걷었다");
        }
        a.hit("w");
        assert_eq!(a.notice, None, "창의 키가 알림을 안 걷었다");
    }

    fn press(a: &mut App, kind: ratatui::crossterm::event::MouseEventKind, column: u16, row: u16) {
        use ratatui::crossterm::event::{KeyModifiers, MouseEvent};
        a.mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
    }

    fn click(a: &mut App, column: u16, row: u16) {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        press(a, MouseEventKind::Down(MouseButton::Left), column, row);
    }

    fn wheel(a: &mut App, down: bool, column: u16, row: u16) {
        use ratatui::crossterm::event::MouseEventKind;
        press(a, if down { MouseEventKind::ScrollDown } else { MouseEventKind::ScrollUp }, column, row);
    }

    /// **목록 칸과 본문 칸이 선다** — 목록은 제목, 본문 칸의 머리는 제목과 경로, 안은 그린 마크다운이다. 바는 듣는 키만
    /// 댄다: 자취가 없으면 `Bksp` 가 없고, 링크로 건너오면 선다. 어떤 크기에서도 터지지 않고, 좁으면 목록만 선다.
    #[test]
    fn the_window_draws_the_pages_and_the_page_under_the_cursor() {
        let (_s, mut a) = wiki_app("draw", PAGES);
        a.hit("SPC g w");
        let lines = draw::tests::render(&mut a, 200, 24);
        let screen = lines.join("\n");
        assert!(screen.contains(" 위키 · ") && screen.contains(" · docs/ "), "목록 머리가 없다\n{screen}");
        assert!(screen.contains("Guide") && screen.contains("Deep"), "목록에 페이지가 없다\n{screen}");
        assert!(screen.contains(" Home · docs/README.md "), "본문 칸 머리가 없다\n{screen}");
        assert!(screen.contains("Read the guide (guide.md) and argos-0001."), "본문이 안 그려졌다\n{screen}");
        assert!(!screen.contains("[guide](guide.md)"), "마크다운이 원문으로 섰다\n{screen}");
        let bar = lines.last().unwrap();
        assert!(bar.contains("Enter 읽기") && bar.contains("Esc 닫기") && bar.contains("SPC 메뉴"), "{bar:?}");
        assert!(!bar.contains("Bksp"), "자취가 없는데 Bksp 를 댔다: {bar:?}");
        let Mode::Wiki(w) = &mut a.mode else { unreachable!() };
        w.follow("guide");
        let bar = draw::tests::render(&mut a, 100, 24).last().cloned().unwrap();
        assert!(bar.contains("Bksp 뒤로"), "자취가 있는데 Bksp 를 안 댔다: {bar:?}");

        for (w, h) in [(1, 1), (4, 3), (12, 6), (19, 8), (20, 8), (30, 10), (80, 24), (160, 60)] {
            let screen = draw::tests::render(&mut a, w, h).join("\n");
            assert!(matches!(a.mode, Mode::Wiki(_)), "{w}x{h} 에서 창이 닫혔다");
            if w < 20 {
                assert_eq!(a.drawn.wiki.and_then(|d| d.page), None, "{w}칸에 본문이 섰다\n{screen}");
            }
        }
        // 좁아 본문이 접히면 포커스는 목록으로 돌아온다 — 안 보이는 칸을 굴리는 키가 생기지 않게.
        a.hit("Enter");
        let _ = draw::tests::render(&mut a, 19, 10);
        assert_eq!(window(&a).focus, Side::List);
    }

    /// **되돌아가 읽던 줄이 지금 칸의 끝을 지났으면 끝에서 그린다** — 굴린 자리를 재기(`Scroll::fit`) 전에 줄을 자르면 그
    /// 그림 한 장이 아래를 빈 채로 서고, 다음 키까지 그대로다(상세의 그림과 같은 차례).
    #[test]
    fn going_back_past_the_end_of_a_taller_pane_draws_from_the_end() {
        let long: String = (1..=40).map(|n| format!("- L{n:02}\n")).collect();
        let long = format!("# Long\n\n{long}");
        let (_s, mut a) = wiki_app("back-fit", &[("README.md", "# Home\n\n[long](long.md)\n"), ("long.md", &long)]);
        a.hit("SPC g w j Enter");
        let _ = draw::tests::render(&mut a, 100, 12);
        a.hit("G");
        let _ = draw::tests::render(&mut a, 100, 12);
        assert!(window(&a).page.offset() > 20, "끝으로 안 굴렀다");
        let Mode::Wiki(w) = &mut a.mode else { unreachable!() };
        assert!(w.follow("README"));
        let _ = draw::tests::render(&mut a, 100, 60);
        a.hit("Bksp");
        let screen = draw::tests::render(&mut a, 100, 60).join("\n");
        assert!(screen.contains("L01") && screen.contains("L40"), "칸이 다 담는 페이지를 읽던 줄부터 잘랐다\n{screen}");
    }

    /// **원문 토글이 창의 본문을 바꾼다**(`SPC v r`) — 메뉴는 열린 채 남고, 굴린 자리는 첫 줄로 돌아간다.
    #[test]
    fn spc_v_r_shows_the_page_as_written() {
        let (_s, mut a) = wiki_app("raw", PAGES);
        a.hit("SPC g w SPC v r");
        assert!(super::super::menu::open(&a.chord), "원문 토글이 메뉴를 닫았다");
        a.hit("Esc");
        let screen = draw::tests::render(&mut a, 160, 24).join("\n");
        assert!(screen.contains("Read the [guide](guide.md) and argos-0001."), "원문이 안 섰다\n{screen}");
        a.hit("SPC v r Esc");
        let screen = draw::tests::render(&mut a, 160, 24).join("\n");
        assert!(!screen.contains("[guide](guide.md)"), "그린 글로 안 돌아왔다\n{screen}");
    }

    /// **못 읽은 페이지와 충돌 표시가 든 페이지는 낱말로 달리고**, 본문 칸은 `moai wiki show` 와 같은 말을 한다 — 충돌은
    /// 원문 그대로에 경고 한 줄, 못 읽은 것은 까닭 한 줄.
    #[test]
    fn unread_and_conflicted_pages_say_so() {
        let conflicted = "# Merge\n\n<<<<<<< ours\nmine\n=======\ntheirs\n>>>>>>> theirs\n";
        let (s, mut a) = wiki_app("odd", &[("README.md", "# Home\n"), ("merge.md", conflicted)]);
        std::fs::write(s.path().join("docs/bad.md"), [0xff, 0xfe, 0x00]).unwrap();
        a.hit("SPC g w");
        let titles: Vec<String> = window(&a).shown().iter().map(|p| p.title.clone()).collect();
        assert_eq!(titles, ["Home", "bad", "Merge"]);
        let screen = draw::tests::render(&mut a, 120, 24).join("\n");
        assert!(screen.contains("bad  ! 못 읽음"), "못 읽은 페이지에 낱말이 없다\n{screen}");
        assert!(screen.contains("Merge  ! 충돌 표시"), "충돌 페이지에 낱말이 없다\n{screen}");
        a.hit("j");
        let screen = draw::tests::render(&mut a, 120, 24).join("\n");
        assert!(screen.contains("! 본문을 못 읽었다"), "못 읽은 까닭이 본문 칸에 없다\n{screen}");
        a.hit("j");
        let screen = draw::tests::render(&mut a, 120, 24).join("\n");
        assert!(screen.contains("! 충돌 표시") && screen.contains("<<<<<<< ours"), "충돌이 원문으로 안 섰다\n{screen}");
    }

    /// **누른 칸으로 포커스가 가고 목록의 줄을 누르면 그 페이지다**. **휠은 포인터 아래 칸이 받고 포커스는 안 옮긴다**
    /// (2026-10-04 사용자 결정). 목록 위의 휠은 커서를 안 옮긴다(moai-og9h) — 굴리는 것은
    /// `the_wheel_and_half_pages_roll_the_list_and_leave_the_page` 가 잰다. 메뉴가 열린 채 창을 누르면 메뉴를 닫고 그
    /// 누르기를 한다(moai-m6ni).
    #[test]
    fn a_click_picks_the_pane_and_the_page_and_the_wheel_moves_the_pane_under_it() {
        let (_s, mut a) = wiki_app("mouse", PAGES);
        a.hit("SPC g w");
        let _ = draw::tests::render(&mut a, 100, 24);
        let d = a.drawn.wiki.expect("창의 자리가 안 섰다");
        let page = d.page.expect("본문 칸이 안 섰다");
        // 목록의 셋째 줄 — 낮은 창에서 굴릴 만큼 긴 Guide 다.
        click(&mut a, d.rows.x + 2, d.rows.y + 2);
        assert_eq!((window(&a).cursor, window(&a).focus), (2, Side::List));
        click(&mut a, page.x + 3, page.y + 2);
        assert_eq!(window(&a).focus, Side::Page, "본문을 눌렀는데 포커스가 안 갔다");
        // 휠은 포인터 아래 칸 — 본문에 포커스가 있어도 목록 위의 휠은 목록 칸의 것이고, 커서는 안 옮긴다.
        wheel(&mut a, false, d.rows.x + 2, d.rows.y);
        assert_eq!((window(&a).cursor, window(&a).focus), (2, Side::Page), "목록 위의 휠이 커서나 포커스를 옮겼다");
        let _ = draw::tests::render(&mut a, 100, 8);
        let page = a.drawn.wiki.and_then(|d| d.page).expect("낮은 창에 본문 칸이 안 섰다");
        wheel(&mut a, true, page.x + 3, page.y + 1);
        assert!(window(&a).page.offset() > 0, "본문 위의 휠이 안 굴렸다");
        // 메뉴가 열린 채 목록 첫 줄을 누르면 메뉴가 닫히고 커서가 간다.
        a.hit("SPC");
        let _ = draw::tests::render(&mut a, 100, 24);
        let d = a.drawn.wiki.unwrap();
        click(&mut a, d.rows.x + 2, d.rows.y);
        assert!(!super::super::menu::open(&a.chord), "창을 눌렀는데 메뉴가 안 닫혔다");
        assert_eq!((window(&a).cursor, window(&a).focus), (0, Side::List));
        // **놓은 동안 온 사건은 버린다** — 찾는 칸과 고르기 창이 떠 있으면 마우스를 놓는데, 놓는 글이 터미널에 닿기 전에
        // 길에 있던 휠·누르기가 커서를 옮기면 고르기 창의 링크가 다른 페이지 머리 밑에 선다.
        a.hit("Enter Enter");
        assert!(window(&a).choose.is_some() && !a.wants_mouse());
        wheel(&mut a, true, d.rows.x + 2, d.rows.y);
        click(&mut a, d.rows.x + 2, d.rows.y + 2);
        assert_eq!((window(&a).cursor, window(&a).focus), (0, Side::Page), "고르기 창이 뜬 동안 마우스가 먹었다");
        a.hit("Esc /");
        wheel(&mut a, true, d.rows.x + 2, d.rows.y);
        assert_eq!(window(&a).cursor, 0, "찾는 칸이 열린 동안 휠이 먹었다");
    }

    /// **목록 칸의 휠과 반 쪽·한 쪽은 화면만 굴린다**(moai-og9h, 사용자 결정 2026-10-04 — 탐색기 목록과 같은 규칙). 커서와
    /// 본문은 그대로고, 고른 페이지가 화면 밖으로 나가도 그림이 화면을 그리로 되돌리지 않는다. 다음 커서 키와 목록을
    /// 다시 세우는 찾기가 화면을 커서로 되돌린다. 한때 목록 위의 휠은 커서를 세 줄씩 옮겼다.
    #[test]
    fn the_wheel_and_half_pages_roll_the_list_and_leave_the_page() {
        let mut files: Vec<(String, String)> =
            vec![("README.md".into(), format!("# Home\n\n{}", "line\n\n".repeat(20)))];
        files.extend((1..=30).map(|n| (format!("p{n:02}.md"), format!("# Page {n:02}\n"))));
        let files: Vec<(&str, &str)> = files.iter().map(|(f, b)| (f.as_str(), b.as_str())).collect();
        let (_s, mut a) = wiki_app("roll", &files);
        a.hit("SPC g w");
        let _ = draw::tests::render(&mut a, 100, 12);
        let d = a.drawn.wiki.expect("창의 자리가 안 섰다");
        let page = d.page.expect("본문 칸이 안 섰다");
        wheel(&mut a, true, page.x + 3, page.y + 1);
        let read = window(&a).page.offset();
        assert!(read > 0, "시험의 전제 — 본문이 굴렀다");
        let shown = |a: &App| window(a).list.shows(window(a).cursor, 1);

        wheel(&mut a, true, d.rows.x + 2, d.rows.y);
        assert_eq!(window(&a).list.offset(), 3, "목록 위의 휠이 화면을 세 줄 안 굴렸다");
        assert_eq!(
            (slug(&a), window(&a).page.offset()),
            ("README".to_string(), read),
            "목록 위의 휠이 커서나 본문을 움직였다"
        );
        let lines = draw::tests::render(&mut a, 100, 12);
        assert_eq!(window(&a).list.offset(), 3, "그림이 화면 밖의 커서를 드러내느라 굴린 화면을 되돌렸다");
        assert!(!shown(&a), "시험의 전제 — 고른 페이지가 화면 밖이다");
        let top = usize::from(d.rows.y);
        assert!(lines[top].contains("Page 03"), "굴린 화면의 첫 줄이 아니다\n{}", lines.join("\n"));
        a.hit("j");
        let _ = draw::tests::render(&mut a, 100, 12);
        assert_eq!(slug(&a), "p01", "`j` 가 커서가 선 페이지에서 안 움직였다");
        assert!(shown(&a), "`j` 뒤에도 화면이 굴린 자리에 남았다 — {}", window(&a).list.offset());

        let before = window(&a).list.offset();
        a.hit("Ctrl-d");
        assert_eq!(
            (window(&a).list.offset(), slug(&a)),
            (before + super::super::scroll::HALF, "p01".to_string()),
            "Ctrl-d 가 화면만 반 쪽 안 굴렸다"
        );
        a.hit("PageDown");
        assert_eq!(slug(&a), "p01", "PageDown 이 커서를 옮겼다");
        let _ = draw::tests::render(&mut a, 100, 12);
        assert!(!shown(&a), "시험의 전제 — 고른 페이지가 화면 밖이다");
        a.hit("/ p Enter");
        let _ = draw::tests::render(&mut a, 100, 12);
        assert_eq!(slug(&a), "p01", "시험의 전제 — 찾기가 보던 페이지에 그대로 선다");
        assert!(shown(&a), "찾기가 다시 세운 목록이 고른 페이지를 화면 밖에 두었다 — {}", window(&a).list.offset());
    }

    fn slugs(a: &App) -> Vec<String> {
        window(a).shown().iter().map(|p| p.slug.clone()).collect()
    }

    /// **`/` 는 제목과 본문을 찾아 목록을 좁힌다** — 치는 대로 좁히고(대소문자를 안 가린다), Enter 가 걸고, Esc 는
    /// 친 것을 버리고 열기 전으로 돌아간다. 걸린 찾기는 창의 Esc 가 먼저 풀고, 다음 Esc 가 창을 닫는다(2026-10-04 사용자
    /// 결정). 좁혀도 보던 페이지가 남으면 거기 선다.
    #[test]
    fn slash_narrows_the_pages_by_title_and_body() {
        let (_s, mut a) = wiki_app("search", PAGES);
        a.hit("SPC g w G");
        assert_eq!(slug(&a), "guide");
        a.hit("/ l");
        assert_eq!(slugs(&a), ["guide"], "치는 대로 안 좁혔다 — `Line` 은 guide 본문에만 있다");
        a.hit("Esc");
        assert_eq!(slugs(&a), ["README", "notes/deep", "guide"], "Esc 가 친 것을 안 버렸다");
        assert_eq!(slug(&a), "guide", "좁혔다 푼 뒤 보던 페이지를 잃었다");
        // **보던 페이지를 가린 글을 쳐도 Esc 는 열기 전의 페이지로 돌아간다** — 치는 동안 커서는 사람이 고른 적이 없다.
        a.hit("/ H o m e");
        assert_eq!(slugs(&a), ["README"]);
        a.hit("Bksp Bksp Bksp Bksp");
        assert_eq!(slug(&a), "guide", "넓혀 다시 보이는 보던 페이지로 안 돌아갔다");
        a.hit("H o m e Esc");
        assert_eq!(slug(&a), "guide", "칸의 Esc 가 열기 전의 페이지로 안 돌아갔다");

        a.hit("/ D E E P Enter");
        assert_eq!(slugs(&a), ["notes/deep", "guide"], "제목(Deep)과 본문(deeper)을 대소문자 없이 찾지 않았다");
        assert_eq!(slug(&a), "guide");
        assert!(window(&a).searched());
        // 걸린 채 다시 `/` 를 누르면 그 글로 열고, Esc 는 그 찾기로 돌아간다.
        a.hit("/ Bksp Bksp Bksp Bksp z Esc");
        assert_eq!(slugs(&a), ["notes/deep", "guide"], "칸의 Esc 가 걸려 있던 찾기로 안 돌아갔다");
        // 창의 Esc 는 찾기부터 푼다.
        a.hit("Esc");
        assert_eq!((slugs(&a).len(), slug(&a).as_str()), (3, "guide"), "Esc 가 찾기를 안 풀었다");
        window(&a);
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
    }

    /// **찾는 동안의 화면** — 아래에 칸이 서고, 하나도 안 걸리면 목록과 칸 곁이 그렇다고 댄다. 걸린 찾기는 목록 머리에
    /// 서고, 본문은 맞은 글을 칠한다(글자만 보는 이 시험은 칠은 못 보고 머리를 본다). 치는 동안은 마우스를 놓는다.
    #[test]
    fn searching_shows_the_field_and_the_hung_search() {
        let (_s, mut a) = wiki_app("search-draw", PAGES);
        a.hit("SPC g w / q q q");
        assert!(!a.wants_mouse(), "치는 동안 마우스를 쥐고 있다");
        let lines = draw::tests::render(&mut a, 120, 24);
        let screen = lines.join("\n");
        let bar = lines.last().unwrap();
        assert!(bar.contains("페이지 찾기") && bar.contains("qqq"), "찾는 칸이 안 섰다: {bar:?}");
        assert!(screen.contains("그 글이 든 페이지가 없다"), "안 걸린 것을 안 댔다\n{screen}");
        a.hit("Bksp Bksp Bksp g u i d e Enter");
        assert!(a.wants_mouse());
        let lines = draw::tests::render(&mut a, 120, 24);
        let screen = lines.join("\n");
        assert!(screen.contains(" · /guide "), "걸린 찾기가 머리에 없다\n{screen}");
        assert!(lines.last().unwrap().contains("Esc 풀기"), "Esc 가 찾기를 푼다고 안 댔다: {:?}", lines.last());
        // 붙여넣기는 치는 칸에만 들어간다.
        a.paste("x");
        assert_eq!(slugs(&a), ["README", "guide"], "칸 없이 붙여넣기가 찾기를 바꿨다");
        a.hit("/ Bksp Bksp Bksp Bksp Bksp");
        a.paste("deep");
        assert_eq!(slugs(&a), ["notes/deep", "guide"]);
    }

    /// **링크를 따라 간 페이지가 찾기에 가렸으면 찾기를 푼다** — 안 풀면 건너뛸 데가 없어 링크가 죽는다.
    #[test]
    fn following_a_link_past_the_search_clears_it() {
        let (_s, mut a) = wiki_app("search-follow", PAGES);
        a.hit("SPC g w / H o m e Enter");
        assert_eq!(slugs(&a), ["README"]);
        let Mode::Wiki(w) = &mut a.mode else { unreachable!() };
        assert!(w.follow("guide"));
        assert_eq!((slug(&a).as_str(), window(&a).searched()), ("guide", false));
    }

    fn choose(a: &App) -> &super::Choose {
        window(a).choose.as_ref().expect("고르기 창이 안 열렸다")
    }

    /// **본문의 Enter 는 그 페이지의 링크와 id 를 고르는 창이다** — 링크는 적힌 차례, id 는 그 뒤. 페이지를 고르면
    /// 그리로 건너가고 `Bksp` 가 되돌린다. Esc 는 고르기 창만 닫는다.
    #[test]
    fn enter_on_the_page_lists_links_and_ids_and_a_page_link_goes_there() {
        use super::Target;
        let (_s, mut a) = wiki_app("links", PAGES);
        a.hit("SPC g w Enter Enter");
        assert_eq!(
            choose(&a).items,
            [
                Target::Page { text: "guide".into(), to: "guide".into(), found: true },
                Target::Issue { id: "argos-0001".into(), title: Some("첫 일".into()) },
            ]
        );
        assert!(!a.wants_mouse(), "고르기 창이 뜬 동안 마우스를 쥐고 있다");
        let screen = draw::tests::render(&mut a, 120, 24).join("\n");
        assert!(screen.contains(" 링크 · Home ") && screen.contains("guide → guide"), "고르기 창이 안 섰다\n{screen}");
        assert!(screen.contains("argos-0001  첫 일"), "id 곁에 제목이 없다\n{screen}");
        a.hit("Esc");
        assert!(window(&a).choose.is_none() && slug(&a) == "README", "Esc 가 고르기 창 말고 다른 것을 했다");
        // **`gg` 는 둘째 키를 기다린다** — 도움말이 고르기 창의 이동으로 대는 키다. 키 하나씩 찾으면 `g` 가 늘 접두어라
        // 맨 위로 가는 길이 없다.
        a.hit("Enter G");
        assert_eq!(choose(&a).cursor, 1);
        let said = "알림".to_string();
        a.notice = Some(said.clone());
        a.hit("g");
        assert_eq!(a.notice.as_deref(), Some(said.as_str()), "기다리는 g 가 알림을 걷었다");
        a.hit("g");
        assert_eq!(choose(&a).cursor, 0, "고르기 창의 gg 가 맨 위로 안 갔다");
        a.hit("Esc Enter Enter");
        assert_eq!(slug(&a), "guide", "페이지 링크를 골랐는데 안 건너갔다");
        assert_eq!(window(&a).focus, Side::Page);
        a.hit("Bksp");
        assert_eq!(slug(&a), "README");
    }

    /// **갈 데가 없는 것도 창에 서고, 고르면 알림만 낸다** — 없는 페이지·위키 밖 주소는 `(없음)`·`(밖)` 으로 달리고,
    /// 트래커에 없는 id 도 `(없음)` 이다. 고를 것이 하나도 없는 페이지에서는 Enter 가 그렇다고 댄다.
    #[test]
    fn dead_links_stand_in_the_list_and_only_say_why() {
        let body = "# Odd\n\n[gone](gone.md), [site](https://example.com), argos-0099.\n";
        let (_s, mut a) = wiki_app("dead", &[("README.md", body), ("plain.md", "# Plain\n\nNothing.\n")]);
        a.hit("SPC g w Enter Enter");
        let screen = draw::tests::render(&mut a, 120, 24).join("\n");
        assert!(screen.contains("gone → gone  (없음)"), "없는 페이지에 낱말이 없다\n{screen}");
        assert!(screen.contains("site → https://example.com  (밖)"), "밖 주소에 낱말이 없다\n{screen}");
        assert!(screen.contains("argos-0099  (없음)"), "없는 id 에 낱말이 없다\n{screen}");
        a.hit("Enter");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("gone")), "{:?}", a.notice);
        assert_eq!((slug(&a).as_str(), window(&a).choose.is_none()), ("README", true));
        a.hit("Enter j Enter");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("https://example.com")), "{:?}", a.notice);
        a.hit("Enter G Enter");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("argos-0099")), "{:?}", a.notice);
        assert!(matches!(a.mode, Mode::Wiki(_)), "없는 id 가 창을 닫았다");
        a.hit("Ctrl-w h j Enter Enter");
        assert!(window(&a).choose.is_none());
        assert!(
            a.notice.as_deref().is_some_and(|n| n.contains("대지 않는다")),
            "고를 것이 없다고 안 댔다: {:?}",
            a.notice
        );
    }

    /// **id 를 고르면 창을 닫고 그 줄에 선다** — 포커스는 목록이다. 거름망이 그 줄을 가렸으면 무엇이 가렸는지 대고 커서는
    /// 그대로다.
    #[test]
    fn picking_an_id_closes_the_window_and_lands_on_the_row() {
        let (_s, a) = wiki_app("land", PAGES);
        let row = |id: &str, title: &str| {
            Issue::new(id.into(), title.into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let mut b = App::new(vec![row("argos-0001", "첫 일"), row("argos-0002", "둘째 일")], cfg(), Path::new());
        b.site.repo = a.site.repo.clone();
        let mut a = b;
        a.cursor = a.rows().len() - 1;
        a.hit("SPC g w Enter Enter G Enter");
        assert_eq!(a.mode, Mode::Browse, "id 를 골랐는데 창이 안 닫혔다");
        assert_eq!(a.focus, Pane::Explorer);
        let on = a.current().and_then(|r| match r {
            super::super::Row::Item(_, e, _) => e.at().map(|at| a.site.issues[at].id.clone()),
            _ => None,
        });
        assert_eq!(on.as_deref(), Some("argos-0001"), "고른 id 의 줄에 안 섰다");

        a.apply(&Mode::Filter(super::super::Input::new("grep=둘째"))).unwrap();
        a.hit("SPC g w Enter Enter G Enter");
        assert_eq!(a.mode, Mode::Browse);
        let notice = a.notice.clone().unwrap_or_default();
        assert!(notice.contains("argos-0001") && notice.contains("Esc"), "{notice:?}");
        assert!(!notice.contains("SPC v a"), "거름망만 가렸는데 보기를 푸는 키를 댔다: {notice:?}");
    }

    /// **보기만 가린 줄에는 보기를 푸는 키만 댄다**(moai-fmv5 와 같은 까닭) — 거름망이 없는데 Esc 를 대면 누른 키가
    /// 아무것도 안 한다. 쓰기 뒤의 착지와 같은 자([`App::veiled_row`])로 가른다.
    #[test]
    fn an_id_hidden_by_the_view_alone_names_only_the_view() {
        let (_s, a) = wiki_app("veiled", PAGES);
        let mut done =
            Issue::new("argos-0001".into(), "끝난 일".into(), Kind::Issue, Status::new("done"), "2026-09-01T00:00:00Z");
        done.status_since = "2026-09-30T00:00:00Z".into();
        let mut b = App::new(vec![done], cfg(), Path::new());
        b.site.repo = a.site.repo.clone();
        let mut a = b;
        a.hit("SPC g w Enter Enter G Enter");
        assert_eq!(a.mode, Mode::Browse);
        let notice = a.notice.clone().unwrap_or_default();
        assert!(notice.contains("argos-0001") && notice.contains("SPC v a"), "{notice:?}");
        assert!(!notice.contains("Esc"), "거름망이 없는데 Esc 를 댔다: {notice:?}");
    }
}
