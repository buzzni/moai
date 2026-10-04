//! 위키의 사람 화면(moai-ihu4) — `moai wiki ls`·`moai wiki show`. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 슬러그·제목·경로는 저장소가 든 파일에서 온 글이라 그리기 전에 제어문자를 걷는다(`text::one_line`). 본문은
//! 이슈 본문과 같은 자(`view::body_lines`)로 그린다 — 같은 마크다운이 두 표면에서 다르게 서지 않게.

use crate::held;
use crate::i18n::{Lang, fill, say};
use crate::style::{self, paint};
use crate::text::{one_line, width};
use crate::wiki::{DirTrouble, Outside, Page, Skip, Skipped, Unread, Wiki};

/// `wiki_dir` 을 화면에 — 빈 글은 체크아웃 뿌리라 `.` 으로 댄다.
fn shown(dir: &str) -> String {
    let d = one_line(dir);
    if d.is_empty() { ".".into() } else { d }
}

/// `moai wiki ls` — 머리 한 줄, 그 밑에 페이지마다 `슬러그  제목`. 못 읽은 페이지는 곁에 `!` 와 까닭이 선다.
pub fn list(lang: Lang, dir: &str, w: &Wiki) -> Vec<String> {
    let dir = shown(dir);
    if w.pages.is_empty() {
        return vec![fill(say(lang, "wiki.empty"), &[("dir", &dir)])];
    }
    let n = w.pages.len().to_string();
    let mut out = vec![paint(style::HEAD, &fill(say(lang, "wiki.head"), &[("dir", &dir), ("n", &n)]))];
    let slugs: Vec<String> = w.pages.iter().map(|p| one_line(&p.slug)).collect();
    let wide = slugs.iter().map(|s| width(s)).max().unwrap_or(0);
    for (p, slug) in w.pages.iter().zip(&slugs) {
        let gap = " ".repeat(wide - width(slug));
        let mut line = format!("  {}{gap}  {}", paint(style::ID, slug), one_line(&p.title));
        if let Some(u) = &p.error {
            let mark = match u {
                Unread::TooLarge => say(lang, "wiki.mark_too_large").to_string(),
                Unread::Refused(_) | Unread::Failed(_) => fill(say(lang, "wiki.mark_unread"), &[("slug", slug)]),
            };
            line.push_str(&format!("  {}", paint(style::WARN, &format!("! {mark}"))));
        }
        out.push(line);
    }
    out
}

/// `moai wiki show <slug>` — 머리(슬러그·제목·경로), 빈 줄, 그린 본문. 본문을 못 읽었으면 그 까닭이 본문 자리에 선다.
pub fn page(lang: Lang, p: &Page) -> Vec<String> {
    let mut out = vec![
        format!("{}  {}", paint(style::ID, &one_line(&p.slug)), paint(style::HEAD, &one_line(&p.title))),
        paint(style::DIM, &one_line(&p.path)),
        String::new(),
    ];
    match (&p.body, &p.error) {
        (Some(body), _) => out.extend(crate::view::body_lines(body)),
        (None, Some(u)) => {
            let said = fill(say(lang, "wiki.body_unread"), &[("said", &unread(lang, p, u))]);
            out.push(paint(style::WARN, &format!("! {said}")));
        }
        (None, None) => {}
    }
    out
}

/// 본문을 못 읽은 까닭 한 줄 — `--json` 의 `error.said` 이기도 하다.
pub fn unread(lang: Lang, p: &Page, u: &Unread) -> String {
    match u {
        Unread::TooLarge => fill(say(lang, "wiki.too_large"), &[("bytes", &p.bytes.to_string())]),
        Unread::Refused(why) => held::said(lang, why),
        Unread::Failed(said) => one_line(said),
    }
}

/// 위키 디렉터리를 못 쓴다 — `--json` 의 `error.said` 이기도 하다.
pub fn dir_trouble(lang: Lang, dir: &str, t: &DirTrouble) -> String {
    let dir = shown(dir);
    // 키를 도우미에 숨기지 않는다 — `i18n` 의 시험이 `say(lang, "…")` 을 글자로 훑어 표와 견준다.
    let d = [("dir", dir.as_str())];
    match t {
        DirTrouble::NoDir => fill(say(lang, "wiki.no_dir"), &d),
        DirTrouble::NotADir => fill(say(lang, "wiki.not_a_dir"), &d),
        DirTrouble::Outside(Outside::Absolute) => fill(say(lang, "wiki.absolute"), &d),
        DirTrouble::Outside(Outside::Parent) => fill(say(lang, "wiki.parent"), &d),
        DirTrouble::Outside(Outside::Held(why)) => {
            fill(say(lang, "wiki.held"), &[("dir", &dir), ("why", &held::said(lang, why))])
        }
        DirTrouble::Failed(said) => fill(say(lang, "wiki.failed"), &[("dir", &dir), ("said", &one_line(said))]),
    }
}

/// 없는 슬러그.
pub fn no_page(lang: Lang, slug: &str, dir: &str) -> String {
    fill(say(lang, "wiki.no_page"), &[("slug", &one_line(slug)), ("dir", &shown(dir))])
}

/// 걷다가 페이지로 못 세운 자리 한 줄 — stderr 로 나간다.
pub fn skipped(lang: Lang, s: &Skipped) -> String {
    let path = one_line(&s.path);
    match &s.why {
        Skip::NotUtf8 => fill(say(lang, "wiki.skip_not_utf8"), &[("path", &path)]),
        Skip::DirLink => fill(say(lang, "wiki.skip_dir_link"), &[("path", &path)]),
        Skip::Unreadable(said) => {
            fill(say(lang, "wiki.skip_unreadable"), &[("path", &path), ("said", &one_line(said))])
        }
    }
}
