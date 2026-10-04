//! 위키의 사람 화면(moai-ihu4) — `moai wiki ls`·`moai wiki show`. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 슬러그·제목·경로는 저장소가 든 파일에서 온 글이라 그리기 전에 제어문자를 걷는다(`text::one_line`). 본문은
//! 이슈 본문과 같은 자(`view::body_lines`)로 그린다 — 같은 마크다운이 두 표면에서 다르게 서지 않게.

use crate::held;
use crate::i18n::{Lang, fill, say};
use crate::style::{self, paint};
use crate::text::{one_line, width};
use crate::wiki::{DirTrouble, Notices, Outside, Page, Skip, Skipped, Unread, Wiki};

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
    let n = crate::wiki::notices(&w.pages);
    if !n.is_empty() {
        out.push(String::new());
        out.extend(notice_lines(lang, &n, true));
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
        // **충돌 표시가 든 페이지는 원문 그대로다**(설계 노트 moai-qunn) — 그리면 표시가 제목·줄글로 섞여 어디가
        // 갈렸는지 안 보인다. 줄마다 제어문자만 걷는다.
        (Some(body), _) if p.conflict => {
            out.push(paint(
                style::WARN,
                &format!("! {}", fill(say(lang, "wiki.conflict_raw"), &[("path", &one_line(&p.path))])),
            ));
            out.push(String::new());
            out.extend(
                crate::text::sanitize(body)
                    .lines()
                    .map(|l| if l.is_empty() { String::new() } else { format!("  {l}") }),
            );
        }
        (Some(body), _) => out.extend(crate::view::body_lines(body)),
        (None, Some(u)) => {
            let said = fill(say(lang, "wiki.body_unread"), &[("said", &unread(lang, p, u))]);
            out.push(paint(style::WARN, &format!("! {said}")));
        }
        (None, None) => {}
    }
    // 충돌은 본문 머리에 이미 섰다 — 꼬리에는 링크와 id 만 센다.
    let n = Notices { conflict: Vec::new(), ..crate::wiki::notices([p]) };
    if !n.is_empty() {
        out.push(String::new());
        out.extend(notice_lines(lang, &n, false));
    }
    out
}

/// 알림을 줄로 — 갈래마다 한 줄, 셈과 처음 몇 개. 셋을 넘으면 `+<나머지>` 로 접는다. 목록(`whole`)은 어느
/// 페이지의 것인지 대고(`README → nope`, `moai-zz99 (README)`), 페이지 하나를 볼 때는 무엇만 댄다.
fn notice_lines(lang: Lang, n: &Notices, whole: bool) -> Vec<String> {
    const SHOWN: usize = 3;
    let which = |items: Vec<String>| {
        let mut said = items.iter().take(SHOWN).map(|s| one_line(s)).collect::<Vec<_>>().join(", ");
        if items.len() > SHOWN {
            said.push_str(&format!(", +{}", items.len() - SHOWN));
        }
        said
    };
    let line = |text: String| paint(style::WARN, &format!("! {text}"));
    let mut out = Vec::new();
    if !n.conflict.is_empty() {
        let items = n.conflict.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (count, which) = (items.len().to_string(), which(items));
        out.push(line(fill(say(lang, "wiki.notice_conflict"), &[("n", &count), ("which", &which)])));
    }
    if !n.unresolved.is_empty() {
        let items = n.unresolved.iter().map(|(slug, to)| if whole { format!("{slug} → {to}") } else { to.to_string() });
        let items = items.collect::<Vec<_>>();
        let (count, which) = (items.len().to_string(), which(items));
        out.push(line(fill(say(lang, "wiki.notice_unresolved"), &[("n", &count), ("which", &which)])));
    }
    if !n.missing.is_empty() {
        let items = n.missing.iter().map(|(slug, id)| if whole { format!("{id} ({slug})") } else { id.to_string() });
        let items = items.collect::<Vec<_>>();
        let (count, which) = (items.len().to_string(), which(items));
        out.push(line(fill(say(lang, "wiki.notice_missing"), &[("n", &count), ("which", &which)])));
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
