//! 위키 — 저장소 안의 마크다운 페이지 묶음을 읽는다(moai-ihu4).
//!
//! 페이지는 `<wiki_dir>/**/*.md` 파일 하나다. 슬러그는 확장자를 뗀 상대 경로(`/` 로 가른다), 제목은 첫 1단
//! 제목이고 없으면 파일 줄기다. **아무것도 저장하지 않는다** — `issues.jsonl` 에는 한 글자도 안 들어가고, 목록·
//! 링크·id 는 부를 때마다 파일에서 읽는 파생값이다(CLAUDE.md "파생값은 저장하지 않는다").
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
use std::collections::HashSet;
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

/// 페이지 하나.
#[derive(Debug, Clone, PartialEq)]
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
    /// 다른 페이지로 가는 링크, 적힌 차례로.
    pub links: Vec<Link>,
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

/// 페이지에서 페이지로 가는 링크 하나.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    /// 링크의 글.
    pub text: String,
    /// 가리키는 페이지의 슬러그 — 링크를 적은 페이지의 폴더에서 푼다(`../x.md` → `x`).
    pub to: String,
    /// 그 슬러그의 페이지가 있는가.
    pub resolved: bool,
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
    for (rel, at) in files {
        wiki.pages.push(page_at(&at, &rel, &shown, &home, prefix, known));
    }
    let slugs: HashSet<String> = wiki.pages.iter().map(|p| p.slug.clone()).collect();
    for link in wiki.pages.iter_mut().flat_map(|p| p.links.iter_mut()) {
        link.resolved = slugs.contains(&link.to);
    }
    wiki.pages.sort_by(|a, b| {
        let rank = |p: &Page| HOMES.iter().position(|h| *h == p.slug).unwrap_or(HOMES.len());
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.slug.cmp(&b.slug))
    });
    Ok(wiki)
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

/// 파일 하나를 페이지로 — 연 손잡이로 크기를 재고, 상한 안이면 그 손잡이로 읽는다.
fn page_at(
    at: &Path,
    rel: &[String],
    shown: &[String],
    home: &Home,
    prefix: &str,
    known: &dyn Fn(&str) -> bool,
) -> Page {
    let file = rel.last().map_or("", String::as_str);
    let stem = file.strip_suffix(".md").unwrap_or(file);
    let slug =
        rel[..rel.len().saturating_sub(1)].iter().map(String::as_str).chain([stem]).collect::<Vec<_>>().join("/");
    let path = shown.iter().chain(rel).map(String::as_str).collect::<Vec<_>>().join("/");
    let mut page = Page {
        slug,
        title: stem.to_string(),
        path,
        bytes: 0,
        issues: Vec::new(),
        links: Vec::new(),
        conflict: false,
        error: None,
        body: None,
    };
    match read(at, home) {
        Ok((bytes, Some(body))) => {
            page.bytes = bytes;
            let parsed = parse(&page.slug, &body, prefix);
            if let Some(title) = parsed.title {
                page.title = title;
            }
            page.issues = parsed.ids.into_iter().map(|id| IssueRef { exists: known(&id), id }).collect();
            page.links = parsed.links.into_iter().map(|(text, to)| Link { text, to, resolved: false }).collect();
            page.conflict = parsed.conflict;
            page.body = Some(body);
        }
        Ok((bytes, None)) => {
            page.bytes = bytes;
            page.error = Some(Unread::TooLarge);
        }
        Err(why) => page.error = Some(why),
    }
    page
}

/// 페이지 파일을 **연 손잡이로** 재고 읽는다 — 크기가 상한을 넘으면 본문 없이 크기만 낸다. 이름으로 잰 뒤
/// 읽기 전에 파일이 갈려도 손잡이가 댄 크기까지만 읽는다([`crate::held::open_inside`]).
fn read(at: &Path, home: &Home) -> Result<(u64, Option<String>), Unread> {
    use std::io::Read;
    let f = crate::held::open_inside(at, home).map_err(|fell| match fell {
        Fell::Unheld(why) => Unread::Refused(why),
        Fell::Io(e) => Unread::Failed(e.to_string()),
    })?;
    let len = f.metadata().map_err(|e| Unread::Failed(e.to_string()))?.len();
    if len > TOO_LARGE {
        return Ok((len, None));
    }
    let mut buf = Vec::new();
    f.take(len).read_to_end(&mut buf).map_err(|e| Unread::Failed(e.to_string()))?;
    // `held::read_inside` 와 같은 말이다 — 같은 처지가 자리마다 다른 말로 서지 않게.
    let body = String::from_utf8(buf).map_err(|_| {
        Unread::Failed(
            std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8").to_string(),
        )
    })?;
    Ok((len, Some(body)))
}

/// 본문 하나에서 읽은 것 — [`parse`] 가 낸다.
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    /// 첫 1단 제목의 글. 비었으면 없다.
    pub title: Option<String>,
    /// 접두어가 맞는 id, 처음 나온 차례로 하나씩.
    pub ids: Vec<String>,
    /// `(글, 대상 슬러그)` — 페이지 링크만([`target`]), 같은 짝은 한 번.
    pub links: Vec<(String, String)>,
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
pub fn parse(slug: &str, body: &str, prefix: &str) -> Parsed {
    let mut out = Parsed { conflict: conflicted(body), ..Parsed::default() };
    let mut title: Option<String> = None;
    // 첫 1단 제목을 만났는가 — 그 제목이 비었어도 둘째로 넘어가지 않는다. "첫 `# ` 줄" 이다.
    let mut title_seen = false;
    let mut in_title = false;
    let mut code_block = 0u32;
    let mut text = String::new();
    // 링크 안에서 모으는 글 — 링크는 겹치지 않는다(CommonMark).
    let mut link: Option<(String, String)> = None;
    let ids = |s: &str, out: &mut Vec<String>| {
        for id in crate::git::ids_in(s) {
            if id.rsplit_once('-').is_some_and(|(p, _)| p == prefix) && !out.iter().any(|seen| seen == id) {
                out.push(id.to_string());
            }
        }
    };
    for event in Parser::new(body) {
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
            }
            other => {
                ids(&std::mem::take(&mut text), &mut out.ids);
                match other {
                    Event::Start(Tag::CodeBlock(_)) => code_block += 1,
                    Event::End(TagEnd::CodeBlock) => code_block = code_block.saturating_sub(1),
                    Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) if !title_seen => {
                        title_seen = true;
                        in_title = true;
                    }
                    Event::End(TagEnd::Heading(_)) if in_title => {
                        in_title = false;
                        out.title = title.take().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                    }
                    Event::SoftBreak | Event::HardBreak if in_title => {
                        title.get_or_insert_with(String::new).push(' ');
                    }
                    Event::Start(Tag::Link { dest_url, .. }) => link = Some((dest_url.to_string(), String::new())),
                    Event::End(TagEnd::Link) => {
                        if let Some((dest, words)) = link.take()
                            && let Some(to) = target(slug, &dest)
                        {
                            let pair = (words.trim().to_string(), to);
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

/// 링크의 주소를 **페이지 슬러그로** 푼다 — 페이지 링크가 아니면 `None`(2026-10-04 사용자 결정).
///
/// 페이지 링크는 상대 경로이고 `.md` 로 끝나며 위키 안에 떨어지는 것이다. 바깥 주소(`https:`·`mailto:`)·`#앵커`
/// 하나·`/` 로 시작하는 주소·`.md` 가 아닌 파일·`..` 으로 위키 뿌리 위로 나가는 것은 아니다. `#`·`?` 뒤는 떼고,
/// `%20` 같은 퍼센트 꼴은 풀어 파일 이름과 견준다. 링크를 적은 페이지(`from`)의 폴더에서 푼다 —
/// `guide/a` 의 `../x.md` 는 `x`, `b.md` 는 `guide/b` 다.
pub fn target(from: &str, dest: &str) -> Option<String> {
    if dest.is_empty() || dest.starts_with(['/', '#', '\\']) {
        return None;
    }
    // 주소의 꼴(`이름:`)이 첫 `/` 앞에 서면 바깥 주소다.
    if dest.split('/').next().is_some_and(|head| head.contains(':')) {
        return None;
    }
    let path = dest.split(['#', '?']).next().unwrap_or_default();
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
    Some(parts.join("/"))
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

    /// 페이지 링크만 슬러그로 푼다 — 폴더 기준, `#`·`?` 뗌, 퍼센트 풂. 바깥 주소·앵커·`.md` 아닌 것·위키 밖은 아니다.
    #[test]
    fn a_link_resolves_to_a_slug_only_when_it_is_a_page_link() {
        for (from, dest, want) in [
            ("explorer", "agents.md", Some("agents")),
            ("guide/a", "b.md", Some("guide/b")),
            ("guide/a", "../x.md", Some("x")),
            ("guide/a", "./deep/c.md#top", Some("guide/deep/c")),
            ("a", "my%20page.md", Some("my page")),
            ("a", "안내.md?x=1", Some("안내")),
            ("a", "Guide.md", Some("Guide")),
            ("a", "../README.md", None),
            ("guide/a", "../../x.md", None),
            ("a", "https://example.com/x.md", None),
            ("a", "mailto:a@b.md", None),
            ("a", "#section", None),
            ("a", "/docs/x.md", None),
            ("a", "../src/wiki.rs", None),
            ("a", "x.md/", None),
            ("a", "dir/.md", None),
            ("a", "", None),
        ] {
            assert_eq!(target(from, dest).as_deref(), want, "{from} → {dest}");
        }
        let p = parse("guide/a", "[B](b.md) [web](https://x.y) [B](b.md) [up](../top.md#s) [`code` text](c.md)\n", "m");
        let to: Vec<(&str, &str)> = p.links.iter().map(|(t, s)| (t.as_str(), s.as_str())).collect();
        assert_eq!(to, [("B", "guide/b"), ("up", "top"), ("code text", "guide/c")]);
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
        assert_eq!(w.find("out").unwrap().title, "out", "밖의 글을 제목으로 읽었다");
        assert_eq!(w.skipped, [Skipped { path: "docs/away".into(), why: Skip::DirLink }]);
        assert!(w.fell_short());
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
