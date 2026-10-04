//! 위키 — 저장소 안의 마크다운 페이지 묶음을 읽는다(moai-ihu4).
//!
//! 페이지는 `<wiki_dir>/**/*.md` 파일 하나다. 슬러그는 확장자를 뗀 상대 경로(`/` 로 가른다), 제목은 첫 1단
//! 제목이고 없으면 파일 줄기다. **아무것도 저장하지 않는다** — `issues.jsonl` 에는 한 글자도 안 들어가고, 목록·
//! 링크·역링크·머리글 앵커·id 는 부를 때마다 파일에서 읽는 파생값이다(CLAUDE.md "파생값은 저장하지 않는다").
//!
//! **위키는 문서라 가지를 탄다.** 트래커는 딸린 워크트리에서 루트로 옮겨 가지만(`store::Repo::find_from`), 위키는
//! 이 체크아웃(`store::Repo::here`) 밑에서 읽는다 — 에픽이 고친 매뉴얼이 그 에픽의 diff 에 든다. 병합은 git 의
//! 기본 3-way 고 드라이버가 없다. 충돌 표시가 든 페이지는 원문 그대로 읽히고 [`Page::conflict`] 가 선다.
//!
//! 두 층이다. [`parse`]·[`target`]·[`conflicted`] 는 글만 받는 순수 함수고, [`dir_of`]·[`load`] 가 디렉터리를
//! 재고 걸어 파일을 읽는다 — 파일은 모두 [`crate::held`] 로 연다(FIFO·장치·체크아웃 밖 링크를 안 읽는다).
//! **화면 말은 모른다** — 거절과 못 읽은 까닭은 자료로 나가고, 글은 `view` 가 짓는다.

use crate::held::{Fell, Home, Unheld};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// 본문을 읽는 상한 — 이보다 큰 페이지는 목록에만 서고 본문은 안 읽는다([`Unread::TooLarge`]). 이 저장소에서
/// 가장 큰 생성 페이지(`docs/cli.md`)가 128KB 남짓이라 여덟 배를 둔다.
pub const TOO_LARGE: u64 = 1024 * 1024;

/// 홈 — 위키 뿌리의 이 페이지들이 목록 맨 위에 이 차례로 선다. 나머지는 제목순이다.
pub const HOMES: [&str; 2] = ["README", "index"];

/// 읽은 위키 — 차례대로 선 페이지와, 걷다가 페이지로 못 세운 자리.
#[derive(Debug, Default)]
pub struct Wiki {
    pub pages: Vec<Page>,
    /// 페이지일 수 있는데 못 세운 자리 — 이름이 UTF-8 이 아니거나, 디렉터리로 가는 링크거나, 못 연 디렉터리.
    /// 말없이 넘기면 그 밑의 페이지가 아무 자취 없이 목록에서 빠진다.
    pub skipped: Vec<Skipped>,
}

impl Wiki {
    /// 슬러그로 페이지 하나 — **글자째 같아야 한다**(대소문자를 가린다). `Guide.md` 의 슬러그는 `Guide` 다.
    pub fn find(&self, slug: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.slug == slug)
    }

    /// 못 읽은 것이 있는가 — 못 연 페이지([`Unread::TooLarge`] 는 빼고)와 못 세운 자리. 크기 상한은 일부러 안
    /// 읽은 것이라 못 읽은 것이 아니다.
    pub fn fell_short(&self) -> bool {
        !self.skipped.is_empty()
            || self.pages.iter().any(|p| matches!(p.error, Some(Unread::Refused(_) | Unread::Failed(_))))
    }
}

/// 사람 화면이 알림으로 세는 것(moai-ihu4.zdk) — 충돌 표시가 든 페이지, 페이지로 안 풀리는 링크, 그 페이지에 없는
/// 머리글을 가리키는 링크, 트래커에 없는 id. **아무것도 막지 않는다** — 세어 비출 뿐이고 종료 코드는 안 바뀐다.
/// `--json` 은 같은 것을 페이지마다 든다(`conflict`·`resolved`·`anchor_resolved`·`exists`) — 세는 자는 여기 하나라
/// 두 표면이 다른 수를 안 낸다.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Notices<'a> {
    /// 충돌 표시가 든 페이지의 슬러그.
    pub conflict: Vec<&'a str>,
    /// `(링크를 적은 페이지, 대상 슬러그)` — 짝마다 한 번. 글이 다른 두 링크가 같은 없는 페이지를 가리켜도
    /// 고칠 자리는 하나라 `README → gone` 이 두 번 서지 않는다.
    pub unresolved: Vec<(&'a str, &'a str)>,
    /// `(링크를 적은 페이지, 대상 슬러그, 앵커)` — 페이지는 있는데 그 앵커의 머리글이 없다(moai-tllo). 셋마다 한
    /// 번이다. 페이지부터 없는 링크는 [`Notices::unresolved`] 에만 선다 — 고칠 자리가 하나다. 본문을 못 읽은 페이지로
    /// 가는 앵커는 있는지 모르므로 여기 안 선다([`Page::holds`]).
    pub missed: Vec<(&'a str, &'a str, &'a str)>,
    /// `(id 를 댄 페이지, 그 id)`.
    pub missing: Vec<(&'a str, &'a str)>,
}

impl Notices<'_> {
    pub fn is_empty(&self) -> bool {
        self.conflict.is_empty() && self.unresolved.is_empty() && self.missed.is_empty() && self.missing.is_empty()
    }
}

/// 페이지들에서 알림을 센다 — 목록이면 위키 전부, `show` 면 그 페이지 하나다.
pub fn notices<'a>(pages: impl IntoIterator<Item = &'a Page>) -> Notices<'a> {
    let mut n = Notices::default();
    for p in pages {
        if p.conflict {
            n.conflict.push(&p.slug);
        }
        for pair in p.links.iter().filter(|l| !l.resolved).map(|l| (p.slug.as_str(), l.to.as_str())) {
            if !n.unresolved.contains(&pair) {
                n.unresolved.push(pair);
            }
        }
        for l in p.links.iter().filter(|l| l.resolved && l.anchor_resolved == Some(false)) {
            let Some(anchor) = l.anchor.as_deref() else { continue };
            let three = (p.slug.as_str(), l.to.as_str(), anchor);
            if !n.missed.contains(&three) {
                n.missed.push(three);
            }
        }
        n.missing.extend(p.issues.iter().filter(|i| !i.exists).map(|i| (p.slug.as_str(), i.id.as_str())));
    }
    n
}

/// 페이지 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// 위키 뿌리에서의 상대 경로에서 `.md` 를 뗀 것 — `guide/explorer`. 이름이 곧 id 다.
    pub slug: String,
    /// 첫 1단 제목의 글. 없거나 못 읽은 페이지면 파일 줄기다.
    pub title: String,
    /// 체크아웃에서의 상대 경로, `/` 로 가른다 — `docs/guide/explorer.md`.
    pub path: String,
    /// 파일 크기. 못 연 페이지면 0 이다 — 그때는 [`Page::error`] 가 선다.
    pub bytes: u64,
    /// 본문이 대는 이슈 id, 처음 나온 차례로 하나씩.
    pub issues: Vec<IssueRef>,
    /// 다른 페이지로 가는 링크, 적힌 차례로. 같은 페이지의 머리글로 가는 `#앵커` 도 여기 선다 — `to` 가 이 페이지다.
    pub links: Vec<Link>,
    /// 머리글마다 그 앵커와 선 줄, 적힌 차례로 — 모든 단의 머리글이다(GitHub 과 같다). 본문을 못 읽었으면 비었다.
    pub headings: Vec<Heading>,
    /// 이 페이지를 가리키는 페이지의 슬러그 — 역링크(moai-ogaw). [`load`] 가 다른 페이지의 [`Page::links`] 를 뒤집어
    /// 세고([`linked_from`]), 위키 목록의 차례로 가리키는 페이지마다 한 번이다.
    pub linked_from: Vec<String>,
    /// git 의 충돌 표시(`<<<<<<<` → `=======` → `>>>>>>>`)가 차례로 선 페이지.
    pub conflict: bool,
    /// 본문을 못 읽었다 — 없으면 다 읽었다.
    pub error: Option<Unread>,
    /// 원문. 못 읽었으면 없다.
    pub body: Option<String>,
}

/// 페이지가 댄 이슈 id 하나.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IssueRef {
    pub id: String,
    /// 그 id 의 줄이 트래커에 있는가.
    pub exists: bool,
}

impl Page {
    /// 이 페이지에 그 앵커의 머리글이 있는가 — 본문을 못 읽었으면 **모른다**(`None`). 링크의 `anchor_resolved` 와 위키
    /// 창의 고르기 창이 이것 하나로 묻는다.
    pub fn holds(&self, anchor: &str) -> Option<bool> {
        self.body.as_ref().map(|_| self.headings.iter().any(|h| h.anchor == anchor))
    }
}

/// 페이지에서 페이지로 가는 링크 하나.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    /// 링크의 글.
    pub text: String,
    /// 가리키는 페이지의 슬러그 — 링크를 적은 페이지의 폴더에서 푼다(`../x.md` → `x`). 같은 페이지의 `#앵커` 면
    /// 이 페이지다.
    pub to: String,
    /// 주소의 `#` 뒤, 퍼센트 꼴을 푼 것 — 그 페이지의 머리글 앵커([`anchor`])와 견준다. `#` 가 없거나 그 뒤가 비면 없다.
    #[serde(skip)]
    pub anchor: Option<String>,
    /// 그 슬러그의 페이지가 있는가.
    pub resolved: bool,
    /// 그 페이지에 그 앵커의 머리글이 있는가 — [`Link::anchor`] 가 설 때만 선다. 페이지가 없으면 거짓이고, 페이지의
    /// 본문을 못 읽었으면 모르므로 없다([`Page::holds`]).
    #[serde(skip)]
    pub anchor_resolved: Option<bool>,
}

/// 머리글 하나 — 앵커와, 본문(앞의 BOM 을 걷은 것)에서 그 머리글이 시작하는 줄(0 부터). 줄은 위키 창이 원문
/// 보기(`SPC v r`)에서 그 자리로 굴릴 때 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub anchor: String,
    pub line: usize,
}

/// 링크 주소가 가리키는 곳 — [`target`] 이 푼다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aim {
    /// 페이지 슬러그.
    pub to: String,
    /// `#` 뒤 — [`Link::anchor`].
    pub anchor: Option<String>,
}

/// 머리글 글 하나의 앵커 — GitHub 이 저장소 마크다운의 머리글에 다는 것과 같은 꼴이다(2026-10-04 사용자 결정,
/// moai-tllo). 그래서 `glossary.md#epic` 같은 링크가 GitHub 에서도 그 자리로 간다.
///
/// 소문자로 접고, 글자·숫자·`-`·`_`·빈칸만 남기고, 빈칸 하나하나를 `-` 로 바꾼다. 빈칸을 몰아 접지 않고 앞뒤의 `-`
/// 도 안 걷는다 — GitHub 이 그렇게 한다(`Foo  Bar` → `foo--bar`). 글자·숫자는 [`char::is_alphanumeric`] 이다: 한글·
/// 악센트 붙은 라틴 글자는 남고, 문장부호·기호·이모지는 빠진다. 결합 부호 몇은 GitHub 과 갈릴 수 있다 — 표준
/// 라이브러리에 유니코드 범주 표가 없다. **앵커를 짓는 자는 이것 하나다** — 같은 페이지의 둘째 머리글부터는
/// [`Anchors`] 가 번호를 붙인다.
pub fn anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// 한 페이지의 앵커를 차례로 짓는다 — 같은 앵커가 거듭되면 `-1`·`-2` 를 붙인다(GitHub 의 `github-slugger` 와 같다).
/// 붙여 지은 것이 이미 선 앵커와 같으면(`Foo`·`Foo-1`·`Foo`) 한 번 더 세어 셋이 다르게 선다.
#[derive(Debug, Default)]
pub struct Anchors(HashMap<String, usize>);

impl Anchors {
    pub fn next(&mut self, text: &str) -> String {
        let base = anchor(text);
        let mut out = base.clone();
        while self.0.contains_key(&out) {
            let n = self.0.entry(base.clone()).or_default();
            *n += 1;
            out = format!("{base}-{n}");
        }
        self.0.insert(out.clone(), 0);
        out
    }
}

/// 페이지 본문을 못 읽은 까닭 — 말이 아니라 자료다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// [`TOO_LARGE`] 를 넘는다. 일부러 안 읽은 것이다.
    TooLarge,
    /// 저장소가 든 파일이라 안 읽었다 — 체크아웃 밖·`.git/` 으로 가는 링크거나 보통 파일이 아니다(FIFO·장치).
    Refused(Unheld),
    /// io 가 진 것 — 운영체제가 낸 말 그대로다. UTF-8 이 아닌 본문도 여기다.
    Failed(String),
}

impl Unread {
    /// `--json` 의 `error.kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Unread::TooLarge => "too_large",
            Unread::Refused(_) => "refused",
            Unread::Failed(_) => "failed",
        }
    }
}

/// 걷다가 페이지로 못 세운 자리 — 체크아웃에서의 상대 경로와 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub path: String,
    pub why: Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    /// 이름이 UTF-8 이 아니다 — 슬러그는 글이라 지을 수 없다.
    NotUtf8,
    /// 디렉터리로 가는 링크다 — 안 따른다. 따르면 체크아웃 밖이나 제 조상으로 도는 고리를 걷는다.
    DirLink,
    /// 디렉터리를 못 열었다 — io 의 말.
    Unreadable(String),
}

impl Skip {
    /// `--json` 의 `skipped[].kind`(2026-10-04 사용자 결정, moai-ihu4.x94).
    pub fn kind(&self) -> &'static str {
        match self {
            Skip::NotUtf8 => "not_utf8",
            Skip::DirLink => "dir_link",
            Skip::Unreadable(_) => "unreadable",
        }
    }
}

/// `wiki_dir` 이 쓸 수 없는 자리다 — 말이 아니라 자료다. 글은 `view` 가 짓는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirTrouble {
    /// 그 자리에 아무것도 없다 — 위키를 아직 시작 안 했다. **실패가 아니다**(2026-10-04 사용자 결정).
    NoDir,
    /// 디렉터리가 아니다.
    NotADir,
    /// 체크아웃 밖이거나 `.git/` 안이다.
    Outside(Outside),
    /// 재거나 열다 io 가 졌다 — 운영체제가 낸 말 그대로다.
    Failed(String),
}

/// [`DirTrouble::Outside`] 의 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outside {
    /// 절대 경로다.
    Absolute,
    /// `..` 이 들었다.
    Parent,
    /// 링크가 체크아웃 밖이나 `.git/` 으로 풀린다. `.git` 을 글자로 적은 것도 여기다.
    Held(Unheld),
}

impl DirTrouble {
    /// `--json` 의 `error.kind` — `commits_error` 와 같은 `kind`/`said` 꼴이다.
    pub fn kind(&self) -> &'static str {
        match self {
            DirTrouble::NoDir => "no_dir",
            DirTrouble::NotADir => "not_a_dir",
            DirTrouble::Outside(_) => "outside",
            DirTrouble::Failed(_) => "failed",
        }
    }

    /// 못 읽은 것인가 — 종료 코드가 이것을 따른다. 없는 디렉터리는 아직 안 쓴 위키라 못 읽은 것이 아니다.
    pub fn fell(&self) -> bool {
        !matches!(self, DirTrouble::NoDir)
    }
}

/// `wiki_dir` 의 날글자를 **이 체크아웃(`here`) 안의 디렉터리로** 잰다 — 푼 자리를 낸다.
///
/// 설정은 이 값을 재지 않고 들기만 한다(`config::Config::wiki_dir`) — 거기서 거절하면 경로 오타 하나가 위키와
/// 상관없는 `moai status` 까지 세운다. 그래서 재는 자리가 여기 하나다.
///
/// - 절대 경로와 `..` 은 글자만 보고 거절한다 — 체크아웃 안으로 돌아오는 `../<이름>/docs` 도 받지 않는다
/// - `.git` 조각은 글자로도 링크를 푼 자리로도 거절한다([`crate::held::into_git`])
/// - 링크는 [`crate::held::place`] 로 잰다 — 체크아웃 안을 가리키면 따르고 밖이면 거절한다
/// - 빈 글과 `.` 은 체크아웃 뿌리다. 걷기가 `.` 으로 시작하는 이름을 건너뛰므로 `.git`·`.moai` 는 안 든다
/// - `\` 는 경로 구분자가 아니라 이름의 글자다 — `docs\wiki` 는 `docs/wiki` 로 안 읽는다
pub fn dir_of(here: &Path, raw: &str) -> Result<PathBuf, DirTrouble> {
    let p = Path::new(raw);
    if p.components().any(|c| matches!(c, Component::RootDir | Component::Prefix(_))) {
        return Err(DirTrouble::Outside(Outside::Absolute));
    }
    if p.components().any(|c| c == Component::ParentDir) {
        return Err(DirTrouble::Outside(Outside::Parent));
    }
    let joined = here.join(p);
    if crate::held::into_git(p) {
        return Err(DirTrouble::Outside(Outside::Held(Unheld::IntoGit { to: joined })));
    }
    let real = crate::held::place(&joined, &Home::of(here)).map_err(|u| DirTrouble::Outside(Outside::Held(u)))?;
    match std::fs::metadata(&real) {
        Ok(m) if m.is_dir() => Ok(real),
        Ok(_) => Err(DirTrouble::NotADir),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(DirTrouble::NoDir),
        Err(e) => Err(DirTrouble::Failed(e.to_string())),
    }
}

/// 이 체크아웃(`here`)의 위키를 읽는다. `raw` 는 `wiki_dir` 의 날글자, `prefix` 는 이슈 id 의 접두어, `known` 은
/// 그 id 의 줄이 트래커에 있는가를 답한다.
///
/// 위키 뿌리를 못 열면 [`DirTrouble`] 이다. 그 밑에서 못 연 디렉터리와 못 읽은 페이지는 멈추지 않고 [`Wiki`] 에
/// 적는다 — 한 파일 때문에 나머지 페이지를 못 보면 무엇이 잘못됐는지 볼 길도 같이 사라진다.
///
/// **페이지 하나를 볼 때도 모든 본문을 읽는다** — 역링크는 다른 페이지가 적은 링크라서다(2026-10-04 사용자 결정,
/// moai-ogaw: `moai wiki show` 도 역링크를 낸다). 그 결정 전에는 물은 페이지 하나만 열던 `load_one` 이 있었다.
pub fn load(here: &Path, raw: &str, prefix: &str, known: &dyn Fn(&str) -> bool) -> Result<Wiki, DirTrouble> {
    let dir = dir_of(here, raw)?;
    let home = Home::of(here);
    let shown: Vec<String> = Path::new(raw)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .filter(|c| *c != ".")
        .map(str::to_string)
        .collect();
    let mut files = Vec::new();
    let mut wiki = Wiki::default();
    let top = std::fs::read_dir(&dir).map_err(|e| DirTrouble::Failed(e.to_string()))?;
    walk(top, &dir, &mut Vec::new(), &shown, &mut files, &mut wiki.skipped);
    for (rel, at) in &files {
        wiki.pages.push(page_at(at, slug_of(rel), rel, &shown, &home, prefix, known));
    }
    // **슬러그는 이름만으로 선다** — 링크가 풀리는가는 못 읽은 페이지까지, 걸은 파일 모두의 이름과 견준다. 앵커는
    // 그 페이지의 머리글과 견주니 본문을 읽은 페이지만 답한다([`Page::holds`]).
    let at: HashMap<&str, usize> = wiki.pages.iter().enumerate().map(|(i, p)| (p.slug.as_str(), i)).collect();
    let answers: Vec<Vec<(bool, Option<bool>)>> = wiki
        .pages
        .iter()
        .map(|p| {
            p.links
                .iter()
                .map(|l| {
                    let page = at.get(l.to.as_str()).map(|&i| &wiki.pages[i]);
                    let anchor = l.anchor.as_deref().and_then(|a| page.map_or(Some(false), |q| q.holds(a)));
                    (page.is_some(), anchor)
                })
                .collect()
        })
        .collect();
    for (page, answers) in wiki.pages.iter_mut().zip(answers) {
        for (link, (resolved, anchor)) in page.links.iter_mut().zip(answers) {
            link.resolved = resolved;
            link.anchor_resolved = anchor;
        }
    }
    wiki.pages.sort_by(|a, b| {
        let rank = |p: &Page| HOMES.iter().position(|h| *h == p.slug).unwrap_or(HOMES.len());
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.slug.cmp(&b.slug))
    });
    // 차례를 세운 뒤에 센다 — 역링크는 위키 목록의 차례로 선다.
    let from = linked_from(&wiki.pages);
    for (page, from) in wiki.pages.iter_mut().zip(from) {
        page.linked_from = from;
    }
    Ok(wiki)
}

/// 페이지마다 그 페이지를 가리키는 페이지의 슬러그 — 역링크(moai-ogaw), `pages` 와 같은 차례로. **세는 자는 여기
/// 하나다** — `moai wiki ls`·`show` 와 탐색기의 위키 창이 이 값([`Page::linked_from`])을 읽어 셋이 다른 수를 안 낸다.
///
/// - `pages` 의 차례로, 가리키는 페이지마다 한 번 — 글이 다른 링크 둘로 가리켜도 한 번이다
/// - 제 자신을 가리키는 링크(`#앵커` 를 뗀 제 슬러그)는 안 센다 — 들어오는 길이 아니다
/// - 본문을 못 읽은 페이지([`Page::error`])는 링크가 없어 아무것도 안 가리킨다. 그러니 역링크는 **읽은 페이지가 적은
///   것만큼이다** — 그 페이지는 목록에 `error` 로 선다
///
/// 링크는 한 번만 훑는다 — 페이지마다 위키 전부의 링크를 다시 훑으면 페이지 수의 제곱이 들고, `show` 도 이 길을 탄다.
fn linked_from(pages: &[Page]) -> Vec<Vec<String>> {
    let at: HashMap<&str, usize> = pages.iter().enumerate().map(|(i, p)| (p.slug.as_str(), i)).collect();
    let mut from: Vec<Vec<String>> = vec![Vec::new(); pages.len()];
    for (i, p) in pages.iter().enumerate() {
        for l in &p.links {
            let Some(&to) = at.get(l.to.as_str()) else { continue };
            // 가리키는 페이지를 차례로 도니, 같은 페이지의 둘째 링크는 그 끝에 이미 섰다 — 끝만 보면 한 번이다.
            if to != i && from[to].last() != Some(&p.slug) {
                from[to].push(p.slug.clone());
            }
        }
    }
    from
}

/// 디렉터리 하나를 걸어 페이지 파일을 모은다 — 이름 차례로, `.` 으로 시작하는 이름은 건너뛴다.
///
/// **디렉터리 링크는 안 따른다**([`Skip::DirLink`]). 파일 링크는 모으고, 그것이 어디로 가는지는 읽는 자리
/// ([`crate::held::open_inside`])가 잰다.
fn walk(
    entries: std::fs::ReadDir,
    dir: &Path,
    rel: &mut Vec<String>,
    shown: &[String],
    files: &mut Vec<(Vec<String>, PathBuf)>,
    skipped: &mut Vec<Skipped>,
) {
    let mut entries: Vec<std::fs::DirEntry> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let lossy = name.to_string_lossy();
        if lossy.starts_with('.') {
            continue;
        }
        let at = dir.join(&name);
        let Ok(ft) = entry.file_type() else { continue };
        let is_dir = ft.is_dir() || (ft.is_symlink() && std::fs::metadata(&at).is_ok_and(|m| m.is_dir()));
        if !is_dir && !lossy.ends_with(".md") {
            continue;
        }
        let path_of = |rel: &[String], last: &str| {
            shown.iter().chain(rel).map(String::as_str).chain([last]).collect::<Vec<_>>().join("/")
        };
        let Some(name) = name.to_str() else {
            skipped.push(Skipped { path: path_of(rel, &lossy), why: Skip::NotUtf8 });
            continue;
        };
        if !is_dir {
            files.push((rel.iter().cloned().chain([name.to_string()]).collect(), at));
            continue;
        }
        if ft.is_symlink() {
            skipped.push(Skipped { path: path_of(rel, name), why: Skip::DirLink });
            continue;
        }
        match std::fs::read_dir(&at) {
            Ok(inner) => {
                rel.push(name.to_string());
                walk(inner, &at, rel, shown, files, skipped);
                rel.pop();
            }
            Err(e) => skipped.push(Skipped { path: path_of(rel, name), why: Skip::Unreadable(e.to_string()) }),
        }
    }
}

/// 걸은 파일의 슬러그 — 위키 뿌리에서의 조각을 `/` 로 잇고 끝 조각의 `.md` 를 뗀다. **슬러그를 짓는 자리는
/// 여기 하나다** — 페이지의 이름과, 링크가 풀리는가를 견줄 이름 모음이 같은 자로 선다.
fn slug_of(rel: &[String]) -> String {
    let (last, dirs) = rel.split_last().map_or(("", &[][..]), |(last, dirs)| (last.as_str(), dirs));
    let stem = last.strip_suffix(".md").unwrap_or(last);
    dirs.iter().map(String::as_str).chain([stem]).collect::<Vec<_>>().join("/")
}

/// 파일 하나를 페이지로 — 연 손잡이로 크기를 재고, 상한 안이면 그 손잡이로 읽는다.
fn page_at(
    at: &Path,
    slug: String,
    rel: &[String],
    shown: &[String],
    home: &Home,
    prefix: &str,
    known: &dyn Fn(&str) -> bool,
) -> Page {
    let file = rel.last().map_or("", String::as_str);
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let path = shown.iter().chain(rel).map(String::as_str).collect::<Vec<_>>().join("/");
    let (bytes, body) = read(at, home);
    let mut page = Page {
        slug,
        title: stem.to_string(),
        path,
        bytes,
        issues: Vec::new(),
        links: Vec::new(),
        headings: Vec::new(),
        linked_from: Vec::new(),
        conflict: false,
        error: None,
        body: None,
    };
    match body {
        Ok(Some(body)) => {
            let parsed = parse(&page.slug, &body, prefix);
            if let Some(title) = parsed.title {
                page.title = title;
            }
            page.issues = parsed.ids.into_iter().map(|id| IssueRef { exists: known(&id), id }).collect();
            page.links = parsed
                .links
                .into_iter()
                .map(|(text, Aim { to, anchor })| Link { text, to, anchor, resolved: false, anchor_resolved: None })
                .collect();
            page.headings = parsed.headings;
            page.conflict = parsed.conflict;
            page.body = Some(body);
        }
        Ok(None) => page.error = Some(Unread::TooLarge),
        Err(why) => page.error = Some(why),
    }
    page
}

/// 페이지 파일을 **연 손잡이로** 재고 읽는다 — `(크기, 본문)`. 크기는 그 손잡이가 댄 것이라 **열었으면 본문을 못
/// 읽었어도 선다**(UTF-8 이 아닌 본문) — 못 열었을 때만 0 이다. 본문은 크기가 상한을 넘으면 `None` 이다. 이름으로
/// 잰 뒤 읽기 전에 파일이 갈려도 손잡이가 댄 크기까지만 읽는다([`crate::held::open_inside`]).
fn read(at: &Path, home: &Home) -> (u64, Result<Option<String>, Unread>) {
    use std::io::Read;
    let failed = |e: std::io::Error| Unread::Failed(e.to_string());
    let f = match crate::held::open_inside(at, home) {
        Ok(f) => f,
        Err(Fell::Unheld(why)) => return (0, Err(Unread::Refused(why))),
        Err(Fell::Io(e)) => return (0, Err(failed(e))),
    };
    let len = match f.metadata() {
        Ok(m) => m.len(),
        Err(e) => return (0, Err(failed(e))),
    };
    if len > TOO_LARGE {
        return (len, Ok(None));
    }
    let mut buf = Vec::new();
    if let Err(e) = f.take(len).read_to_end(&mut buf) {
        return (len, Err(failed(e)));
    }
    // UTF-8 이 아닌 본문은 `held::read_inside` 와 같은 말로 선다 — 같은 처지가 자리마다 다른 말로 서지 않게.
    (len, crate::held::utf8(buf).map(Some).map_err(failed))
}

/// 본문 하나에서 읽은 것 — [`parse`] 가 낸다.
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    /// 첫 1단 제목의 글. 비었으면 없다.
    pub title: Option<String>,
    /// 접두어가 맞는 id, 처음 나온 차례로 하나씩.
    pub ids: Vec<String>,
    /// `(글, 가리키는 곳)` — 페이지 링크만([`target`]), 같은 짝은 한 번.
    pub links: Vec<(String, Aim)>,
    /// 머리글마다 앵커와 줄, 적힌 차례로([`Page::headings`]).
    pub headings: Vec<Heading>,
    /// [`conflicted`].
    pub conflict: bool,
}

/// 본문을 읽는다 — `slug` 는 링크를 풀 자리, `prefix` 는 이슈 id 의 접두어. **순수 함수다.**
///
/// - 제목은 첫 1단 제목이다(`# ` 줄과, 밑줄 꼴도). 울타리 안의 `# 주석` 은 제목이 아니다
/// - id 는 글과 인라인 코드에서 센다. **울타리·들여쓴 코드 블록 안은 예시로 읽는다**(2026-10-04 사용자 결정) —
///   `work` 줄이 울타리 안을 안 세는 것과 같은 자다. 접두어가 맞는 낱말만 id 다: `worktree-moai-xxxx` 는 가지
///   이름이지 id 가 아니다
/// - 링크는 [`target`] 이 페이지 링크로 푸는 것만 든다
/// - 머리글은 모든 단이 앵커를 받는다([`Anchors`]). 앵커를 짓는 글은 GitHub 이 화면에 그리는 글이다 — 인라인 코드는
///   백틱 없이, 링크는 글만, 그림의 대체글은 빼고, 줄바꿈은 줄바꿈 글자로(그래서 [`anchor`] 가 걷는다)
pub fn parse(slug: &str, body: &str, prefix: &str) -> Parsed {
    let body = unmarked(body);
    let mut out = Parsed { conflict: conflicted(body), ..Parsed::default() };
    let mut title: Option<String> = None;
    // 첫 1단 제목을 만났는가 — 그 제목이 비었어도 둘째로 넘어가지 않는다. "첫 `# ` 줄" 이다.
    let mut title_seen = false;
    let mut in_title = false;
    let mut code_block = 0u32;
    let mut text = String::new();
    // 링크 안에서 모으는 글 — 링크는 겹치지 않는다(CommonMark).
    let mut link: Option<(String, String)> = None;
    // 지금 모으는 머리글의 글과 그 머리글이 선 줄 — 머리글은 겹치지 않는다.
    let mut heading: Option<(String, usize)> = None;
    let mut anchors = Anchors::default();
    // 그림 안 — 그 대체글은 머리글의 앵커에 안 든다(GitHub 이 그리는 머리글 글에 그림은 글자로 서지 않는다).
    let mut image = 0u32;
    // `(이 바이트까지, 그 앞의 줄 수)` — 머리글은 적힌 차례로 오니 앞에서 센 데서 이어 센다.
    let mut seen = (0usize, 0usize);
    let mut line_at = |at: usize| {
        if at < seen.0 {
            return body[..at].matches('\n').count();
        }
        seen = (at, seen.1 + body[seen.0..at].matches('\n').count());
        seen.1
    };
    let ids = |s: &str, out: &mut Vec<String>| {
        for id in crate::git::ids_in(s) {
            if id.rsplit_once('-').is_some_and(|(p, _)| p == prefix) && !out.iter().any(|seen| seen == id) {
                out.push(id.to_string());
            }
        }
    };
    for (event, range) in Parser::new_ext(body, crate::markdown::OPTIONS).into_offset_iter() {
        match event {
            Event::Text(_) | Event::Code(_) if code_block > 0 => {}
            Event::Text(t) => {
                text.push_str(&t);
                if in_title {
                    title.get_or_insert_with(String::new).push_str(&t);
                }
                if let Some((_, words)) = &mut link {
                    words.push_str(&t);
                }
                if let Some((words, _)) = &mut heading
                    && image == 0
                {
                    words.push_str(&t);
                }
            }
            Event::Code(t) => {
                ids(&std::mem::take(&mut text), &mut out.ids);
                ids(&t, &mut out.ids);
                if in_title {
                    title.get_or_insert_with(String::new).push_str(&t);
                }
                if let Some((_, words)) = &mut link {
                    words.push_str(&t);
                }
                if let Some((words, _)) = &mut heading {
                    words.push_str(&t);
                }
            }
            other => {
                ids(&std::mem::take(&mut text), &mut out.ids);
                match other {
                    Event::Start(Tag::CodeBlock(_)) => code_block += 1,
                    Event::End(TagEnd::CodeBlock) => code_block = code_block.saturating_sub(1),
                    Event::Start(Tag::Heading { level, .. }) => {
                        if level == HeadingLevel::H1 && !title_seen {
                            title_seen = true;
                            in_title = true;
                        }
                        heading = Some((String::new(), line_at(range.start)));
                    }
                    Event::End(TagEnd::Heading(_)) => {
                        if in_title {
                            in_title = false;
                            out.title = title.take().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                        }
                        if let Some((words, line)) = heading.take() {
                            out.headings.push(Heading { anchor: anchors.next(&words), line });
                        }
                    }
                    Event::SoftBreak | Event::HardBreak => {
                        if in_title {
                            title.get_or_insert_with(String::new).push(' ');
                        }
                        if let Some((words, _)) = &mut heading {
                            words.push('\n');
                        }
                    }
                    Event::Start(Tag::Image { .. }) => image += 1,
                    Event::End(TagEnd::Image) => image = image.saturating_sub(1),
                    Event::Start(Tag::Link { dest_url, .. }) => link = Some((dest_url.to_string(), String::new())),
                    Event::End(TagEnd::Link) => {
                        if let Some((dest, words)) = link.take()
                            && let Some(aim) = target(slug, &dest)
                        {
                            let pair = (words.trim().to_string(), aim);
                            if !out.links.contains(&pair) {
                                out.links.push(pair);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ids(&text, &mut out.ids);
    out
}

/// 본문의 링크 전부 — `(글, 주소)`, 적힌 차례로, 같은 짝은 한 번. **순수 함수다.**
///
/// [`parse`] 가 드는 것은 페이지로 풀리는 링크뿐이다(`Page::links`, `--json` 의 `links`). 탐색기의 위키 창은 바깥
/// 주소까지 고르기 창에 세워야 해서(`(밖)`) 본문을 한 번 더 훑는다 — 설계 노트 moai-qunn 이 정한 자리다. 울타리·들여쓴
/// 코드 블록 안은 [`parse`] 와 같이 예시로 읽어 안 든다. 그림(`![…](…)`)은 링크가 아니다.
pub fn links_in(body: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut code_block = 0u32;
    let mut link: Option<(String, String)> = None;
    for event in Parser::new_ext(unmarked(body), crate::markdown::OPTIONS) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block += 1,
            Event::End(TagEnd::CodeBlock) => code_block = code_block.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) if code_block == 0 => {
                link = Some((dest_url.to_string(), String::new()));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, words)) = &mut link {
                    words.push_str(&t);
                }
            }
            // 줄을 넘은 링크 글은 그 자리가 빈칸이다 — 안 넣으면 `[the\nguide]` 가 `theguide` 로 선다.
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, words)) = &mut link {
                    words.push(' ');
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some((dest, words)) = link.take() {
                    let pair = (words.trim().to_string(), dest);
                    if !out.contains(&pair) {
                        out.push(pair);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// 본문에서 **앞의 BOM 을 걷은** 글 — 읽는 [`parse`] 와 그리는 `view::wiki::page` 가 이것 하나로 걷는다.
///
/// 윈도 편집기가 붙이는 그 한 글자가 남으면 첫 `# 제목` 이 제목으로 안 읽혀, 페이지가 파일 줄기를 제목으로 달고
/// 화면도 그 줄을 줄글로 그린다. 설정 파서(`config::entries`)가 걷는 것과 같은 까닭이다. 원문(`Page::body`,
/// `--json` 의 `body`)은 그대로다.
pub fn unmarked(body: &str) -> &str {
    body.strip_prefix('\u{feff}').unwrap_or(body)
}

/// 링크의 주소를 **페이지 슬러그와 앵커로** 푼다 — 페이지 링크가 아니면 `None`(2026-10-04 사용자 결정).
///
/// 페이지 링크는 상대 경로이고 `.md` 로 끝나며 위키 안에 떨어지는 것이다. 바깥 주소(`https:`·`mailto:`)·`/` 로
/// 시작하는 주소·`.md` 가 아닌 파일·`..` 으로 위키 뿌리 위로 나가는 것은 아니다. `?` 뒤는 떼고, `%20` 같은 퍼센트
/// 꼴은 풀어 파일 이름과 견준다. 링크를 적은 페이지(`from`)의 폴더에서 푼다 — `guide/a` 의 `../x.md` 는 `x`,
/// `b.md` 는 `guide/b` 다.
///
/// **`#` 뒤는 앵커다**(moai-tllo) — 퍼센트 꼴을 풀어 [`Aim::anchor`] 에 든다. 비면 앵커가 없는 것이다(GitHub 도 페이지
/// 맨 위로 간다). `#앵커` 하나뿐인 주소는 **이 페이지**의 머리글로 가는 링크다 — `#` 하나뿐이면 갈 머리글이 없어
/// 링크가 아니다.
pub fn target(from: &str, dest: &str) -> Option<Aim> {
    let (head, fragment) = match dest.split_once('#') {
        Some((head, fragment)) => (head, Some(unpercent(fragment)).filter(|f| !f.is_empty())),
        None => (dest, None),
    };
    if head.is_empty() {
        return fragment.map(|anchor| Aim { to: from.to_string(), anchor: Some(anchor) });
    }
    if head.starts_with(['/', '\\']) {
        return None;
    }
    // 주소의 꼴(`이름:`)이 첫 `/` 앞에 서면 바깥 주소다.
    if head.split('/').next().is_some_and(|first| first.contains(':')) {
        return None;
    }
    let path = head.split('?').next().unwrap_or_default();
    let path = unpercent(path);
    let stem = path.strip_suffix(".md")?;
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    let segs: Vec<&str> = stem.split('/').collect();
    if segs.last().is_none_or(|last| matches!(*last, "" | "." | "..")) {
        return None;
    }
    for seg in segs {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(Aim { to: parts.join("/"), anchor: fragment })
}

/// `%XX` 를 바이트로 푼다. 풀어 UTF-8 이 안 되거나 꼴이 어긋나면 받은 그대로다.
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// git 의 충돌 표시가 **차례로** 선 글인가 — 줄 머리의 `<<<<<<<`, 그 뒤의 `=======` 한 줄, 그 뒤의 `>>>>>>>`.
///
/// 울타리 안도 센다 — 충돌은 울타리 짝을 깨뜨리기도 해서 울타리를 믿으면 진짜 충돌을 놓친다. 대신 git 을
/// 설명하는 페이지가 표시를 예로 적으면 거짓으로 선다. 알림일 뿐이라 해롭지 않다(설계 노트 moai-qunn).
pub fn conflicted(body: &str) -> bool {
    let marker = |l: &str, m: &str| l == m || l.strip_prefix(m).is_some_and(|rest| rest.starts_with(' '));
    let mut stage = 0;
    for l in body.lines() {
        match stage {
            0 if marker(l, "<<<<<<<") => stage = 1,
            1 if l == "=======" => stage = 2,
            2 if marker(l, ">>>>>>>") => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn write(at: &Path, rel: &str, body: &str) {
        let p = at.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn none(_: &str) -> bool {
        false
    }

    /// 제목은 첫 1단 제목이다 — 울타리 안의 `# 주석` 도, 2단 제목도 아니다. 꾸밈은 걷고 글만 든다.
    #[test]
    fn the_title_is_the_first_level_one_heading() {
        let t = |body: &str| parse("a", body, "moai").title;
        assert_eq!(t("# The `moai` explorer\n\ntext\n").as_deref(), Some("The moai explorer"));
        assert_eq!(t("```sh\n# not a title\n```\n\n## Two\n\n# One\n").as_deref(), Some("One"));
        assert_eq!(t("Setext\n======\n").as_deref(), Some("Setext"));
        assert_eq!(t("no heading at all\n"), None);
        assert_eq!(t("#\n\n# Second\n"), None, "빈 첫 제목은 제목이 없는 것이다 — 둘째로 넘어가지 않는다");
        assert_eq!(t("# First\n\n# Second\n").as_deref(), Some("First"));
        assert_eq!(t("\u{feff}# Bom\n").as_deref(), Some("Bom"), "앞의 BOM 이 제목을 가렸다");
    }

    /// id 는 글과 인라인 코드에서 세고 코드 블록 안은 예시라 안 센다. 접두어가 맞는 낱말만, 처음 나온 차례로 하나씩.
    #[test]
    fn ids_come_from_prose_and_inline_code_with_this_prefix_only() {
        let body = "See moai-ab12 and `moai-cd34`. Again moai-ab12.\n\n\
                    Branch worktree-moai-ef56, child moai-ab12.x1y, other argos-gh78.\n\n\
                    ```sh\nmoai show moai-zz99\n```\n\n    moai-yy88 indented\n\n[link moai-kk11](x.md)\n";
        assert_eq!(parse("a", body, "moai").ids, ["moai-ab12", "moai-cd34", "moai-ab12.x1y", "moai-kk11"]);
        assert_eq!(parse("a", "a-b e-mail x86-64\n", "e").ids, ["e-mail"], "접두어가 맞으면 꼴만 보는 것이 맞다");
    }

    /// **링크 전부는 바깥 주소까지 든다** — 적힌 차례로, 같은 짝은 한 번, 코드 블록 안과 그림은 안 든다. 위키 창의
    /// 고르기 창이 읽는다(moai-o3cb).
    #[test]
    fn every_link_is_listed_outside_code() {
        let body = "\u{feff}# T\n\nSee [guide](guide.md), [site](https://example.com) and <https://x.org>.\n\n\
                    Again [guide](guide.md), [`code`](a.md#top), ![pic](p.png).\n\n\
                    ```md\n[not](x.md)\n```\n\n    [indented](y.md)\n";
        assert_eq!(
            links_in(body),
            [
                ("guide".to_string(), "guide.md".to_string()),
                ("site".to_string(), "https://example.com".to_string()),
                ("https://x.org".to_string(), "https://x.org".to_string()),
                ("code".to_string(), "a.md#top".to_string()),
            ]
        );
        // 링크 글이 줄을 넘으면 그 자리는 빈칸이다 — 낱말이 붙으면 고르기 창에 없는 낱말이 선다.
        assert_eq!(links_in("[the\nguide](guide.md)\n"), [("the guide".to_string(), "guide.md".to_string())]);
    }

    /// 페이지 링크만 슬러그로 푼다 — 폴더 기준, `?` 뗌, 퍼센트 풂. 바깥 주소·`.md` 아닌 것·위키 밖은 아니다. `#` 뒤는
    /// 앵커로 따로 들고(퍼센트를 풀어), `#앵커` 하나뿐인 주소는 이 페이지로 간다(moai-tllo).
    #[test]
    fn a_link_resolves_to_a_slug_only_when_it_is_a_page_link() {
        for (from, dest, want) in [
            ("explorer", "agents.md", Some(("agents", None))),
            ("guide/a", "b.md", Some(("guide/b", None))),
            ("guide/a", "../x.md", Some(("x", None))),
            ("guide/a", "./deep/c.md#top", Some(("guide/deep/c", Some("top")))),
            ("a", "my%20page.md", Some(("my page", None))),
            ("a", "안내.md?x=1", Some(("안내", None))),
            ("a", "안내.md?x=1#용어", Some(("안내", Some("용어")))),
            ("a", "Guide.md", Some(("Guide", None))),
            ("a", "glossary.md#epic", Some(("glossary", Some("epic")))),
            ("a", "glossary.md#%ED%95%9C%EA%B8%80", Some(("glossary", Some("한글")))),
            ("a", "glossary.md#", Some(("glossary", None))),
            ("a", "glossary.md#a:b", Some(("glossary", Some("a:b")))),
            ("guide/a", "#section", Some(("guide/a", Some("section")))),
            ("a", "#", None),
            ("a", "../README.md", None),
            ("guide/a", "../../x.md", None),
            ("a", "https://example.com/x.md", None),
            ("a", "https://example.com/x.md#top", None),
            ("a", "mailto:a@b.md", None),
            ("a", "/docs/x.md", None),
            ("a", "/docs/x.md#top", None),
            ("a", "../src/wiki.rs", None),
            ("a", "../src/wiki.rs#L3", None),
            ("a", "x.md/", None),
            ("a", "dir/.md", None),
            ("a", "", None),
        ] {
            let got = target(from, dest);
            assert_eq!(got.as_ref().map(|a| (a.to.as_str(), a.anchor.as_deref())), want, "{from} → {dest}");
        }
        let p = parse(
            "guide/a",
            "[B](b.md) [web](https://x.y) [B](b.md) [up](../top.md#s) [`code` text](c.md) [here](#s) [up](../top.md)\n",
            "m",
        );
        let to: Vec<(&str, &str, Option<&str>)> =
            p.links.iter().map(|(t, a)| (t.as_str(), a.to.as_str(), a.anchor.as_deref())).collect();
        assert_eq!(
            to,
            [
                ("B", "guide/b", None),
                ("up", "top", Some("s")),
                ("code text", "guide/c", None),
                ("here", "guide/a", Some("s")),
                ("up", "top", None),
            ],
            "같은 글의 링크도 앵커가 다르면 다른 링크다"
        );
    }

    /// **앵커는 GitHub 이 다는 것과 글자째 같다**(moai-tllo) — 아래 기대값은 GitHub 이 이 저장소의 파일
    /// (`docs/recovery.md`·`CLAUDE.md`·`CHANGELOG.md`·`README.md`)을 그린 HTML 의 `id="user-content-…"` 에서 옮겼다
    /// (2026-10-04, `gh api repos/buzzni/moai/contents/<파일> -H 'Accept: application/vnd.github.html'`). 소문자로 접고,
    /// 문장부호를 빼고, 빈칸을 하나하나 `-` 로 — 몰아 접지 않는다. 한글은 그대로다.
    #[test]
    fn anchors_are_the_ones_github_draws() {
        for (heading, want) in [
            ("Recovery", "recovery"),
            ("\"locked\"", "locked"),
            ("\"broken\": a line that will not parse", "broken-a-line-that-will-not-parse"),
            ("A conflicted issues.jsonl", "a-conflicted-issuesjsonl"),
            ("moai — 작업 규약", "moai--작업-규약"),
            ("이 프로젝트가 무엇을 뒤집었는가", "이-프로젝트가-무엇을-뒤집었는가"),
            ("일한 AI 를 남긴다", "일한-ai-를-남긴다"),
            ("[0.5.0] - 2026-10-04", "050---2026-10-04"),
            ("The --json contract", "the---json-contract"),
            ("SQL over the output", "sql-over-the-output"),
        ] {
            assert_eq!(anchor(heading), want, "{heading:?}");
        }
        // 같은 앵커가 거듭되면 `-1`·`-2` — `CHANGELOG.md` 의 절 머리가 GitHub 에서 받은 그대로다.
        let mut a = Anchors::default();
        let got: Vec<String> = ["Added", "Changed", "Fixed", "Added", "Changed", "Removed", "Fixed", "Added"]
            .iter()
            .map(|h| a.next(h))
            .collect();
        assert_eq!(got, ["added", "changed", "fixed", "added-1", "changed-1", "removed", "fixed-1", "added-2"]);
        // 붙여 지은 것이 이미 선 앵커면 한 번 더 센다 — 셋이 다르게 선다(`github-slugger` 와 같다).
        let mut a = Anchors::default();
        let got: Vec<String> = ["Foo", "Foo-1", "Foo", "Foo"].iter().map(|h| a.next(h)).collect();
        assert_eq!(got, ["foo", "foo-1", "foo-2", "foo-3"]);
    }

    /// **머리글마다 앵커와 줄** — 모든 단, 적힌 차례로. 앵커의 글은 GitHub 이 그리는 글이다: 인라인 코드는 백틱 없이,
    /// 링크는 글만, 그림의 대체글은 빼고. 코드 블록 안의 `#` 줄은 머리글이 아니고, 줄은 앞의 BOM 을 걷은 본문에서 센다.
    #[test]
    fn every_heading_gets_an_anchor_and_its_line() {
        let body = "\u{feff}# Glossary\n\nintro\n\n## `Next:` note\n\n```sh\n# not a heading\n```\n\n\
                    ### Link [to page](x.md) and ![pic](p.png) here\n\nSetext two\nlines\n----------\n\n\
                    ## Epic\n\n- ## in a list\n\n## Epic\n";
        let p = parse("glossary", body, "moai");
        let got: Vec<(&str, usize)> = p.headings.iter().map(|h| (h.anchor.as_str(), h.line)).collect();
        assert_eq!(
            got,
            [
                ("glossary", 0),
                ("next-note", 4),
                ("link-to-page-and--here", 10),
                ("setext-twolines", 12),
                ("epic", 16),
                ("in-a-list", 18),
                ("epic-1", 20),
            ]
        );
        assert_eq!(p.title.as_deref(), Some("Glossary"), "제목은 여전히 첫 1단 머리글이다");
        assert!(parse("a", "no heading\n", "m").headings.is_empty());
    }

    /// 충돌 표시는 셋이 차례로 서야 충돌이다. 하나만 선 줄이나 거꾸로 선 것은 아니다.
    #[test]
    fn conflict_markers_count_only_in_order() {
        assert!(conflicted("a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\nb\n"));
        assert!(conflicted("<<<<<<<\nx\n||||||| base\ny\n=======\nz\n>>>>>>>\n"), "diff3 꼴도 충돌이다");
        assert!(!conflicted("=======\n<<<<<<< a\n>>>>>>> b\n"));
        assert!(!conflicted("<<<<<<<< eight\n=======\n>>>>>>> x\n"), "여덟 개는 표시가 아니다");
        assert!(!conflicted("Title\n=======\n"));
        assert!(conflicted("<<<<<<< a\r\nx\r\n=======\r\ny\r\n>>>>>>> b\r\n"), "CRLF 도 같다");
    }

    /// 걷기 — `.md` 만, `.` 으로 시작하는 이름은 건너뛰고, 홈이 맨 위, 나머지는 제목순. 슬러그는 대소문자를
    /// 가르고 비ASCII 이름도 그대로다. 링크는 다 읽은 뒤에 풀린다.
    #[test]
    fn load_walks_pages_and_orders_home_first_then_by_title() {
        let s = Scratch::new("wiki-load");
        write(s.path(), "docs/README.md", "# Home\n\n[Zed](z.md) [gone](nope.md)\n");
        write(s.path(), "docs/index.md", "no title here\n");
        write(s.path(), "docs/z.md", "# alpha\n\nmoai-ab12 moai-cd34\n");
        write(s.path(), "docs/guide/Beta.md", "# Beta\n\n[home](../README.md)\n");
        write(s.path(), "docs/안내.md", "# 안내\n");
        write(s.path(), "docs/notes.txt", "# not a page\n");
        write(s.path(), "docs/.hidden/x.md", "# hidden\n");
        write(s.path(), "docs/.draft.md", "# hidden too\n");
        let known = |id: &str| id == "moai-ab12";
        let w = load(s.path(), "docs", "moai", &known).unwrap();
        let slugs: Vec<&str> = w.pages.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, ["README", "index", "z", "guide/Beta", "안내"]);
        let home = w.find("README").unwrap();
        assert_eq!((home.title.as_str(), home.path.as_str()), ("Home", "docs/README.md"));
        assert_eq!(
            home.links.iter().map(|l| (l.to.as_str(), l.resolved)).collect::<Vec<_>>(),
            [("z", true), ("nope", false)]
        );
        assert_eq!(w.find("index").unwrap().title, "index", "제목이 없으면 파일 줄기다");
        let z = w.find("z").unwrap();
        assert_eq!(
            z.issues,
            [IssueRef { id: "moai-ab12".into(), exists: true }, IssueRef { id: "moai-cd34".into(), exists: false }]
        );
        assert_eq!(z.bytes, "# alpha\n\nmoai-ab12 moai-cd34\n".len() as u64);
        assert_eq!(w.find("guide/Beta").unwrap().links[0].to, "README");
        assert!(w.find("guide/beta").is_none(), "슬러그는 대소문자를 가른다");
        assert!(w.skipped.is_empty() && !w.fell_short());
    }

    /// `wiki_dir` 을 쓰는 자리에서 잰다 — 없으면 `no_dir`(못 읽은 것이 아니다), 파일이면 `not_a_dir`, 절대·`..`·
    /// `.git`·밖으로 가는 링크면 `outside`. 빈 글과 `.` 은 체크아웃 뿌리다. `\` 는 구분자가 아니다.
    #[cfg(unix)]
    #[test]
    fn the_wiki_dir_is_measured_where_it_is_used() {
        let s = Scratch::new("wiki-dir");
        let away = Scratch::new("wiki-dir-away");
        std::fs::create_dir_all(s.join("docs/wiki")).unwrap();
        std::fs::create_dir_all(s.join(".git")).unwrap();
        std::fs::write(s.join("file"), "").unwrap();
        std::os::unix::fs::symlink(away.path(), s.join("out")).unwrap();
        std::os::unix::fs::symlink(s.join("docs/wiki"), s.join("in")).unwrap();
        std::os::unix::fs::symlink(s.join(".git"), s.join("git-link")).unwrap();
        let kind = |raw: &str| dir_of(s.path(), raw).map(|_| "ok").unwrap_or_else(|t| t.kind());
        for (raw, want) in [
            ("docs", "ok"),
            ("docs/wiki", "ok"),
            ("./docs", "ok"),
            ("", "ok"),
            (".", "ok"),
            ("in", "ok"),
            ("missing", "no_dir"),
            ("docs\\wiki", "no_dir"),
            ("file", "not_a_dir"),
            ("/etc", "outside"),
            ("../x", "outside"),
            ("docs/../docs", "outside"),
            (".git", "outside"),
            (".git/wiki", "outside"),
            ("out", "outside"),
            ("git-link", "outside"),
        ] {
            assert_eq!(kind(raw), want, "{raw:?}");
        }
        assert!(!DirTrouble::NoDir.fell() && DirTrouble::NotADir.fell());
        let root = load(s.path(), "", "moai", &none).unwrap();
        assert!(root.pages.is_empty(), "체크아웃 뿌리에서도 `.git` 은 안 걷는다");
    }

    /// 못 읽는 페이지도 목록에 선다 — 크기 상한을 넘은 것은 본문 없이 `too_large`, 밖으로 가는 링크·FIFO 는
    /// `refused`(멈추지 않는다), UTF-8 이 아닌 것은 `failed`. 디렉터리 링크는 안 따르고 걸러 둔 자리로 남는다.
    #[cfg(unix)]
    #[test]
    fn unreadable_pages_still_stand_and_say_why() {
        let s = Scratch::new("wiki-unread");
        let away = Scratch::new("wiki-unread-away");
        write(s.path(), "docs/ok.md", "# Ok\n");
        let big = format!("# Big\n{}", "x".repeat(TOO_LARGE as usize));
        write(s.path(), "docs/big.md", &big);
        write(away.path(), "secret.md", "# Secret\n");
        std::os::unix::fs::symlink(away.join("secret.md"), s.join("docs/out.md")).unwrap();
        std::os::unix::fs::symlink(s.join("docs/ok.md"), s.join("docs/alias.md")).unwrap();
        std::os::unix::fs::symlink(away.path(), s.join("docs/away")).unwrap();
        std::fs::write(s.join("docs/latin1.md"), b"# caf\xe9\n").unwrap();
        let fifo = s.join("docs/pipe.md");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: 널로 끝나는 경로와 권한 비트만 넘긴다.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "FIFO 를 못 지었다");
        let at = s.path().to_path_buf();
        let w = crate::held::tests::within("wiki FIFO", move || load(&at, "docs", "moai", &none).unwrap());
        let kind = |slug: &str| w.find(slug).unwrap().error.as_ref().map(Unread::kind);
        assert_eq!(kind("ok"), None);
        assert_eq!(kind("alias"), None, "안을 가리키는 링크는 따른다");
        assert_eq!(kind("big"), Some("too_large"));
        let b = w.find("big").unwrap();
        assert_eq!((b.bytes, b.body.is_none(), b.title.as_str()), (big.len() as u64, true, "big"));
        assert_eq!(kind("out"), Some("refused"));
        assert_eq!(kind("pipe"), Some("refused"));
        assert_eq!(kind("latin1"), Some("failed"));
        assert_eq!(w.find("latin1").unwrap().bytes, 7, "열었으면 본문을 못 읽어도 크기는 선다");
        assert_eq!(w.find("out").unwrap().bytes, 0, "못 연 페이지는 0 이다");
        assert_eq!(w.find("out").unwrap().title, "out", "밖의 글을 제목으로 읽었다");
        assert_eq!(w.skipped, [Skipped { path: "docs/away".into(), why: Skip::DirLink }]);
        assert!(w.fell_short());
    }

    /// 알림은 충돌 표시·풀리지 않는 링크·없는 id 셋을 페이지와 함께 센다. 풀리는 링크와 있는 id 는 안 센다.
    #[test]
    fn notices_count_conflicts_dangling_links_and_unknown_ids() {
        let s = Scratch::new("wiki-notices");
        write(s.path(), "docs/README.md", "# Home\n\n[a](a.md) [gone](gone.md) [again](gone.md) moai-ab12 moai-zz99\n");
        write(s.path(), "docs/a.md", "# A\n\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> b\n");
        let known = |id: &str| id == "moai-ab12";
        let w = load(s.path(), "docs", "moai", &known).unwrap();
        let n = notices(&w.pages);
        assert_eq!(n.conflict, ["a"]);
        assert_eq!(n.unresolved, [("README", "gone")], "같은 없는 페이지로 가는 링크 둘은 고칠 자리 하나다");
        assert_eq!(n.missing, [("README", "moai-zz99")]);
        assert!(notices(w.find("a")).missing.is_empty() && !notices(w.find("a")).is_empty());
        assert!(notices(&w.pages[..0]).is_empty());
    }

    /// **앵커는 그 페이지의 머리글과 견준다**(moai-tllo) — 있으면 `anchor_resolved` 가 참, 없으면 거짓이고 알림
    /// (`missed`)에 한 번 선다. 같은 페이지의 `#앵커` 도 같다. 페이지부터 없으면 앵커도 거짓이지만 알림은 페이지
    /// 쪽(`unresolved`) 하나다. 본문을 못 읽은 페이지의 앵커는 모르므로 `anchor_resolved` 가 없고 알림도 없다. 앵커가
    /// 없는 링크에는 둘 다 없고, 앵커가 하나 더 서도 역링크는 페이지마다 한 번이다.
    #[test]
    fn anchors_resolve_against_the_headings_of_the_page_they_name() {
        let s = Scratch::new("wiki-anchors");
        write(
            s.path(),
            "docs/README.md",
            "# Home\n\n## Start here\n\n[epic](glossary.md#epic) [gone](glossary.md#gone) [again](glossary.md#gone) \
             [top](glossary.md) [here](#start-here) [nowhere](#nowhere) [lost](lost.md#x) [big](big.md#y)\n",
        );
        write(s.path(), "docs/glossary.md", "# Glossary\n\n## Epic\n\n## Idea\n");
        write(s.path(), "docs/big.md", &format!("# Big\n\n## Y\n{}", "x".repeat(TOO_LARGE as usize)));
        let w = load(s.path(), "docs", "moai", &none).unwrap();
        let home = w.find("README").unwrap();
        let got: Vec<(&str, Option<&str>, bool, Option<bool>)> =
            home.links.iter().map(|l| (l.to.as_str(), l.anchor.as_deref(), l.resolved, l.anchor_resolved)).collect();
        assert_eq!(
            got,
            [
                ("glossary", Some("epic"), true, Some(true)),
                ("glossary", Some("gone"), true, Some(false)),
                ("glossary", Some("gone"), true, Some(false)),
                ("glossary", None, true, None),
                ("README", Some("start-here"), true, Some(true)),
                ("README", Some("nowhere"), true, Some(false)),
                ("lost", Some("x"), false, Some(false)),
                ("big", Some("y"), true, None),
            ]
        );
        let n = notices(&w.pages);
        assert_eq!(
            n.missed,
            [("README", "glossary", "gone"), ("README", "README", "nowhere")],
            "없는 머리글은 셋마다 한 번, 없는 페이지와 못 읽은 페이지는 빼고"
        );
        assert_eq!(n.unresolved, [("README", "lost")]);
        assert_eq!(w.find("glossary").unwrap().holds("idea"), Some(true));
        assert_eq!(w.find("big").unwrap().holds("y"), None, "못 읽은 본문의 머리글은 모른다");
        assert_eq!(w.find("glossary").unwrap().linked_from, ["README"], "앵커가 달라도 역링크는 한 번이다");
        assert!(home.linked_from.is_empty(), "제 머리글로 가는 링크는 들어오는 길이 아니다");
    }

    /// **역링크는 다른 페이지의 링크를 뒤집어 센다**(moai-ogaw) — 위키 목록의 차례로, 가리키는 페이지마다 한 번. 제
    /// 자신을 가리키는 링크·코드 블록 안의 링크·본문을 못 읽은 페이지의 링크는 안 센다. 못 읽은 페이지도 가리켜지면
    /// 역링크가 선다 — 그 페이지는 이름으로 서 있다.
    ///
    /// `m`(Mid)은 걷는 차례로는 `guide/z` 뒤고 제목 차례로는 앞이다 — 차례를 세우기 전에 세면 붉어진다.
    #[test]
    fn backlinks_invert_every_page_link_in_list_order() {
        let s = Scratch::new("wiki-backlinks");
        write(s.path(), "docs/README.md", "# Home\n\n[a](a.md) [again a](a.md) [gone](gone.md) [big](big.md)\n");
        write(s.path(), "docs/a.md", "# A\n\n[self](a.md#top) [zed](guide/z.md)\n");
        write(s.path(), "docs/guide/z.md", "# Zed\n\n[a](../a.md) [home](../README.md)\n");
        write(s.path(), "docs/m.md", "# Mid\n\n[a](a.md)\n");
        write(s.path(), "docs/code.md", "# Code\n\n```md\n[a](a.md)\n```\n");
        write(s.path(), "docs/big.md", &format!("# Big\n\n[a](a.md)\n{}", "x".repeat(TOO_LARGE as usize)));
        let w = load(s.path(), "docs", "moai", &none).unwrap();
        let from = |slug: &str| w.find(slug).unwrap().linked_from.clone();
        assert_eq!(
            from("a"),
            ["README", "m", "guide/z"],
            "목록 차례로, 글이 다른 두 링크는 한 번, 제 링크·코드·못 읽은 것은 빼고"
        );
        assert_eq!(from("README"), ["guide/z"]);
        assert_eq!(from("guide/z"), ["a"]);
        assert_eq!(from("big"), ["README"], "못 읽은 페이지도 가리켜지면 선다");
        assert!(from("code").is_empty() && from("m").is_empty());
        // 슬러그는 이름으로 선다 — 본문을 못 읽은 페이지로 가는 링크도 풀린다(걷어 낸 `load_one` 시험이 지키던 것).
        let to_big = w.find("README").unwrap().links.iter().find(|l| l.to == "big").map(|l| l.resolved);
        assert_eq!(to_big, Some(true), "못 읽은 페이지로 가는 링크가 안 풀렸다");
    }

    /// 크기 상한 바로 아래는 읽는다 — 상한은 "넘으면" 이다.
    #[test]
    fn a_page_right_at_the_limit_is_read() {
        let s = Scratch::new("wiki-limit");
        let body = format!("# Edge\n{}", "y".repeat(TOO_LARGE as usize - 7));
        assert_eq!(body.len() as u64, TOO_LARGE);
        write(s.path(), "docs/edge.md", &body);
        let w = load(s.path(), "docs", "moai", &none).unwrap();
        assert_eq!(w.find("edge").unwrap().error, None);
    }
}
