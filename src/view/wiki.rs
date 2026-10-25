//! 위키의 사람 화면(moai-ihu4) — `moai wiki ls`·`moai wiki show`. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 슬러그·제목·경로는 저장소가 든 파일에서 온 글이라 그리기 전에 제어문자를 걷는다(`text::one_line`). 본문은
//! 이슈 본문과 같은 자(`view::body_lines`)로 그린다 — 같은 마크다운이 두 표면에서 다르게 서지 않게.

use crate::held;
use crate::i18n::{Lang, fill, say};
use crate::style::{self, paint};
use crate::text::{one_line, width};
use crate::wiki::about::About;
use crate::wiki::search::Hit;
use crate::wiki::seen;
use crate::wiki::{DirTrouble, Notices, Outside, Page, Skip, Skipped, Unread, Wiki};

/// `wiki_dir` 을 화면에 — 빈 글은 체크아웃 뿌리라 `.` 으로 댄다.
fn shown(dir: &str) -> String {
    let d = one_line(dir);
    if d.is_empty() { ".".into() } else { d }
}

/// `moai wiki ls` — 머리 한 줄, 그 밑에 페이지마다 `슬러그  제목`. 못 읽은 페이지는 곁에 `!` 와 까닭이 선다.
pub fn list(lang: Lang, dir: &str, w: &Wiki, lessons: &crate::wiki::seen::Lessons) -> Vec<String> {
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
        // 교훈 페이지 곁에 서로 다른 이슈 수(moai-4s5n.c0n) — 노트에서 읽은 값이다.
        if seen::is_lesson(&p.slug) {
            let n = lessons.get(&p.slug).map_or(0, Vec::len);
            line.push_str(&format!("  {}", paint(style::DIM, &format!("×{n}"))));
        }
        if let Some(u) = &p.error {
            // 옮겨 치라고 내미는 명령이라 슬러그는 원문을 셸 낱말로 감싼다(`text::quoted`) — `my page` 를 그대로
            // 대면 `moai wiki show my page` 가 낱말 둘로 갈려 clap 이 거절한다.
            let mark = match u {
                Unread::TooLarge => say(lang, "wiki.mark_too_large").to_string(),
                Unread::Refused(_) | Unread::Failed(_) => {
                    fill(say(lang, "wiki.mark_unread"), &[("slug", &crate::text::quoted(&p.slug))])
                }
            };
            line.push_str(&format!("  {}", paint(style::WARN, &format!("! {mark}"))));
        }
        out.push(line);
    }
    let n = crate::wiki::notices(&w.pages);
    // 페이지 없는 `Lesson:` 노트 — 다른 가지에서 아직 안 든 페이지일 수 있어 "이 체크아웃에 없다" 고만 말한다.
    let unpaged = seen::unpaged(lessons, |s| w.find(s).is_some());
    if !n.is_empty() || !unpaged.is_empty() {
        out.push(String::new());
        out.extend(notice_lines(lang, &n, true));
        if !unpaged.is_empty() {
            let which = unpaged.iter().take(3).map(|s| one_line(s)).collect::<Vec<_>>();
            let mut which = which.join(", ");
            if unpaged.len() > 3 {
                which.push_str(&format!(", +{}", unpaged.len() - 3));
            }
            let text = fill(say(lang, "wiki.notice_unpaged"), &[("n", &unpaged.len().to_string()), ("which", &which)]);
            out.push(paint(style::WARN, &format!("! {text}")));
        }
    }
    out
}

/// 질의가 비었을 때의 거절문 — 고칠 명령 한 줄이 같이 선다.
pub fn search_usage(lang: Lang) -> String {
    say(lang, "wiki.search_usage").to_string()
}

/// `moai wiki search` — hit 마다 두 줄(`path:line  제목 › 머리글`, 발췌), 빈 줄, 끝 줄(몇 개 중 몇 개와 다음 명령).
/// 맞은 자리의 굵은 칠은 꾸밈이다 — 색을 빼도 글은 그대로 서고 뜻은 칠에 없다. 결과가 없으면 그 말과 `wiki ls` 다.
pub fn search(lang: Lang, dir: &str, query: &str, hits: &[Hit], total: usize) -> Vec<String> {
    let Some(first) = hits.first() else {
        return vec![fill(say(lang, "wiki.search_none"), &[("query", &one_line(query)), ("dir", &shown(dir))])];
    };
    let mut out = Vec::new();
    for h in hits {
        let mut head = format!(
            "{}  {}",
            paint(style::ID, &format!("{}:{}", one_line(&h.page.path), h.line)),
            one_line(&h.page.title)
        );
        if let Some(t) = section_of(h) {
            head.push_str(&format!(" › {}", one_line(t)));
        }
        out.push(head);
        if let Some(s) = &h.snippet {
            out.push(format!("    {}", marked(&s.text, &s.marks)));
        }
    }
    out.push(String::new());
    out.push(fill(
        say(lang, "wiki.search_foot"),
        &[
            ("n", &hits.len().to_string()),
            ("total", &total.to_string()),
            ("slug", &crate::text::quoted(&first.page.slug)),
        ],
    ));
    out
}

/// `moai show <id>` 의 위키 토막(moai-g8pt.5aw) — 빈 줄, 머리, 댄 페이지(`path   제목`), 관련 절
/// (`path:line   제목 › 머리글`), 다음 명령 한 줄. 둘 다 비면 아무 줄도 없다 — 토막째 안 선다. 낱말 칸은 둘 가운데
/// 넓은 쪽에 맞추고, 같은 묶음의 둘째 줄부터는 낱말 자리를 비운다.
pub fn about(lang: Lang, a: &About) -> Vec<String> {
    let first = a
        .named_in
        .first()
        .map(|n| n.page.slug.as_str())
        .or_else(|| a.related.first().map(|h| h.page.slug.as_str()))
        .or_else(|| a.lessons.first().map(|l| l.slug));
    let Some(first) = first else {
        return Vec::new();
    };
    let (named, related, own) =
        (say(lang, "wiki.about_named"), say(lang, "wiki.about_related"), say(lang, "wiki.about_lessons"));
    let wide = width(named).max(width(related)).max(width(own));
    let mark = |slug: &str| {
        if seen::is_lesson(slug) {
            format!("  {}", paint(style::DIM, &format!("[{}]", say(lang, "wiki.lesson_mark"))))
        } else {
            String::new()
        }
    };
    let label = |word: &str, n: usize| {
        let w = if n == 0 { word } else { "" };
        let painted = if w.is_empty() { String::new() } else { paint(style::DIM, w) };
        format!("  {painted}{}   ", " ".repeat(wide - width(w)))
    };
    let mut out = vec![String::new(), paint(style::HEAD, say(lang, "wiki.about_head"))];
    // 이 이슈의 노트가 가리킨 교훈이 맨 앞이다 — 이 일을 하다 겪은 것이라 가장 가깝다. 페이지가 이 체크아웃에 없으면
    // 슬러그만 선다.
    for (n, l) in a.lessons.iter().enumerate() {
        let (path, title) = match l.page {
            Some(p) => (one_line(&p.path), format!("   {}", one_line(&p.title))),
            None => (one_line(l.slug), String::new()),
        };
        out.push(format!("{}{}{title}{}", label(own, n), paint(style::ID, &path), mark(l.slug)));
    }
    for (n, x) in a.named_in.iter().enumerate() {
        out.push(format!(
            "{}{}   {}{}",
            label(named, n),
            paint(style::ID, &one_line(&x.page.path)),
            one_line(&x.page.title),
            mark(&x.page.slug)
        ));
    }
    for (n, h) in a.related.iter().enumerate() {
        let mut line = format!(
            "{}{}   {}",
            label(related, n),
            paint(style::ID, &format!("{}:{}", one_line(&h.page.path), h.line)),
            one_line(&h.page.title)
        );
        if let Some(t) = section_of(h) {
            line.push_str(&format!(" › {}", one_line(t)));
        }
        line.push_str(&mark(&h.page.slug));
        out.push(line);
    }
    out.push(format!(
        "  {}   {}",
        " ".repeat(wide),
        paint(style::DIM, &fill(say(lang, "wiki.about_hint"), &[("slug", &crate::text::quoted(first))]))
    ));
    out
}

/// `제목 › 머리글` 의 머리글 — 페이지 제목과 같은 글이면 없다. 제목 절(첫 `# `)의 머리글은 제목 그 자체라
/// `Windows › Windows` 처럼 같은 말이 두 번 섰다(리뷰 moai-g8pt.9rd). `--json` 의 `heading` 은 그대로다.
fn section_of<'h>(h: &'h Hit) -> Option<&'h str> {
    h.heading.as_deref().filter(|t| *t != h.page.title)
}

/// 발췌에 맞은 자리를 굵게 — 조각마다 제어문자를 걷는다(바이트 범위가 걷기에 밀리지 않게 칠하기 전에 가른다).
fn marked(text: &str, marks: &[std::ops::Range<usize>]) -> String {
    let (mut out, mut at) = (String::new(), 0);
    for m in marks {
        // 범위가 글자 경계가 아니면 칠을 건너뛴다 — 칠은 꾸밈이라 글이 먼저다.
        if m.start < at || m.end > text.len() || !text.is_char_boundary(m.start) || !text.is_char_boundary(m.end) {
            continue;
        }
        out.push_str(&one_line(&text[at..m.start]));
        out.push_str(&paint(style::STRONG, &one_line(&text[m.clone()])));
        at = m.end;
    }
    out.push_str(&one_line(&text[at..]));
    out
}

/// `moai wiki show <slug>` — 머리(슬러그·제목·경로), 빈 줄, 그린 본문. 본문을 못 읽었으면 그 까닭이 본문 자리에 선다.
/// 꼬리에 이 페이지를 가리키는 페이지의 슬러그가 한 줄로 선다(moai-ogaw) — 아무도 안 가리키면 줄이 없다. 슬러그는
/// `moai wiki show` 에 그대로 건넬 이름이라 제목이 아니라 슬러그고, 목록의 `!` 표시처럼 셸 낱말로 감싼다
/// (`text::quoted`) — `my page` 가 맨글로 서면 옮겨 친 명령이 낱말 둘로 갈린다.
///
/// `uncounted` 는 역링크를 세지 못한 자리의 수다(`Wiki::uncounted`). 0 이 아니면 그 밑에 한 줄이 더 선다 — 아무도
/// 안 가리켜 역링크 줄이 없을 때도 선다: 그때가 "아무도 안 가리킨다" 와 "못 셌다" 가 같아 보이는 자리다(moai-mdzx.jty).
pub fn page(lang: Lang, p: &Page, uncounted: usize, seen: &[seen::Seen]) -> Vec<String> {
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
        (Some(body), _) => out.extend(crate::view::body_lines(crate::wiki::unmarked(body))),
        (None, Some(u)) => {
            let said = fill(say(lang, "wiki.body_unread"), &[("said", &unread(lang, p, u))]);
            out.push(paint(style::WARN, &format!("! {said}")));
        }
        (None, None) => {}
    }
    if !p.linked_from.is_empty() {
        let which =
            p.linked_from.iter().map(|s| paint(style::ID, &crate::text::quoted(s))).collect::<Vec<_>>().join(", ");
        out.push(String::new());
        out.push(fill(say(lang, "wiki.linked_from"), &[("which", &which)]));
    }
    if uncounted > 0 {
        if p.linked_from.is_empty() {
            out.push(String::new());
        }
        out.push(paint(style::DIM, &fill(say(lang, "wiki.linked_from_partial"), &[("n", &uncounted.to_string())])));
    }
    // 교훈이 선 이슈 — 최근 먼저, 이슈 id·제목·날짜(moai-4s5n.c0n).
    if !seen.is_empty() {
        out.push(String::new());
        out.push(paint(style::HEAD, &fill(say(lang, "wiki.seen_head"), &[("n", &seen.len().to_string())])));
        let wide = seen.iter().map(|s| width(&s.id)).max().unwrap_or(0);
        for s in seen {
            let gap = " ".repeat(wide - width(&s.id));
            let day = s.at.get(..10).unwrap_or(&s.at);
            out.push(format!(
                "  {}{gap}  {}  {}",
                paint(style::ID, &one_line(&s.id)),
                paint(style::DIM, &one_line(day)),
                one_line(&s.title)
            ));
        }
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
    let mut out = Vec::new();
    // 갈래 하나 — 빈 갈래는 줄이 없다. 말은 부르는 자리가 `say(lang, "…")` 로 글자째 넘긴다: `i18n` 의 시험이 그
    // 글자를 훑어 표와 견주므로 키를 여기 숨기지 않는다.
    let mut push = |head: &str, items: Vec<String>| {
        if items.is_empty() {
            return;
        }
        let mut which = items.iter().take(SHOWN).map(|s| one_line(s)).collect::<Vec<_>>().join(", ");
        if items.len() > SHOWN {
            which.push_str(&format!(", +{}", items.len() - SHOWN));
        }
        let text = fill(head, &[("n", &items.len().to_string()), ("which", &which)]);
        out.push(paint(style::WARN, &format!("! {text}")));
    };
    push(say(lang, "wiki.notice_conflict"), n.conflict.iter().map(|s| s.to_string()).collect());
    push(
        say(lang, "wiki.notice_unresolved"),
        n.unresolved.iter().map(|(slug, to)| if whole { format!("{slug} → {to}") } else { to.to_string() }).collect(),
    );
    // 링크 주소처럼 댄다 — 같은 페이지의 머리글이면 `#앵커` 하나, 다른 페이지면 `슬러그#앵커`(moai-tllo).
    push(
        say(lang, "wiki.notice_missed"),
        n.missed
            .iter()
            .map(|(slug, to, anchor)| {
                let aim = if slug == to { format!("#{anchor}") } else { format!("{to}#{anchor}") };
                if whole { format!("{slug} → {aim}") } else { aim }
            })
            .collect(),
    );
    push(
        say(lang, "wiki.notice_missing"),
        n.missing.iter().map(|(slug, id)| if whole { format!("{id} ({slug})") } else { id.to_string() }).collect(),
    );
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

/// 걷다가 페이지로 못 세운 자리 한 줄 — stderr 로 나간다. `<자리>: <까닭>`.
pub fn skipped(lang: Lang, s: &Skipped) -> String {
    format!("{}: {}", one_line(&s.path), skip_reason(lang, s))
}

/// 건너뛴 까닭만 — `--json` 의 `skipped[].said` 다. 자리는 곁의 `path` 가 든다.
pub fn skip_reason(lang: Lang, s: &Skipped) -> String {
    match &s.why {
        Skip::NotUtf8 => say(lang, "wiki.skip_not_utf8").to_string(),
        Skip::DirLink => say(lang, "wiki.skip_dir_link").to_string(),
        Skip::Unreadable(said) => fill(say(lang, "wiki.skip_unreadable"), &[("said", &one_line(said))]),
    }
}

/// `moai wiki learn` 이 짓지 않는 까닭(moai-4s5n.n7j) — 꼴이 틀렸으면 꼴 전부를 낸다. 거절문이 고칠 명령을 그대로
/// 내야 고치는 쪽이 문서를 다시 찾지 않는다.
pub fn learn_refused(lang: Lang, why: &crate::wiki::learn::Refused) -> String {
    use crate::wiki::learn::Refused;
    match why {
        Refused::Title => say(lang, "wiki.learn_bad_title").to_string(),
        Refused::OpensWithTitle => say(lang, "wiki.learn_opens_with_title").to_string(),
        Refused::Empty | Refused::NoDoInstead => {
            std::iter::once(say(lang, "wiki.learn_form")).chain(LESSON_FORM).collect::<Vec<_>>().join("\n")
        }
    }
}

/// 교훈 본문의 꼴 — 거절문이 통째로 낸다. 명령과 머리글은 옮겨 칠 글이라 번역하지 않는다(위키의 머리글은 영어다).
///
/// **들여 쓰지 않는다** — 옮겨 친 heredoc 의 닫는 줄이 들여써지면 셸이 끝을 못 찾는다. 표시는 `MD`·`EOF` 가 아니다 — 노트나
/// 커밋 메시지의 바깥 heredoc 에 인용하면 그쪽을 일찍 닫는다(`every_help_heredoc_is_copyable` 의 자).
const LESSON_FORM: [&str; 8] = [
    "moai wiki learn '<the lesson, as an instruction>' --issue <id> -b - <<'LESSON'",
    "## What happened",
    "<what went wrong>",
    "## Why",
    "<why it went wrong>",
    "## Do this instead",
    "<what to do next time>",
    "LESSON",
];

/// 제목이 ASCII 가 아니라 슬러그를 못 짓는다 — `--slug` 를 단 명령을 그대로 낸다.
pub fn learn_no_slug(lang: Lang, title: &str) -> String {
    let head = fill(say(lang, "wiki.learn_no_slug"), &[("rule", crate::wiki::learn::SLUG_RULE)]);
    format!("{head}\n  moai wiki learn {} --slug <name> …", crate::text::quoted(title))
}

/// `--slug` 가 자에 안 맞는다.
pub fn learn_bad_slug(lang: Lang, slug: &str) -> String {
    fill(
        say(lang, "wiki.learn_bad_slug"),
        &[("slug", &crate::text::quoted(slug)), ("rule", crate::wiki::learn::SLUG_RULE)],
    )
}

/// 위키 디렉터리가 아직 없다 — `learn` 은 그것을 짓지 않는다(`wiki_dir` 이 틀렸을 수 있다).
pub fn learn_no_wiki(lang: Lang, dir: &str) -> String {
    fill(say(lang, "wiki.learn_no_wiki"), &[("dir", &shown(dir))])
}

/// 집은 줄이 없거나 여럿이라 노트를 달 줄을 모른다 — `ids` 가 비었으면 없는 것이다.
pub fn learn_which_issue(lang: Lang, ids: &[&str]) -> String {
    if ids.is_empty() {
        return say(lang, "wiki.learn_none_held").to_string();
    }
    fill(say(lang, "wiki.learn_many_held"), &[("n", &ids.len().to_string()), ("ids", &ids.join(", "))])
}

/// 그 자리에 페이지가 이미 섰다 — 되풀이면 노트를, 틀렸으면 그 파일을 고치라고 댄다.
pub fn learn_exists(lang: Lang, path: &str, id: Option<&str>, note: &str) -> String {
    let path = one_line(path);
    let cmd = format!("moai note {} {}", id.unwrap_or("<id>"), crate::text::quoted(note));
    [
        fill(say(lang, "wiki.learn_exists"), &[("path", &path)]),
        format!("  {}", fill(say(lang, "wiki.learn_exists_again"), &[("cmd", &cmd)])),
        format!("  {}", fill(say(lang, "wiki.learn_exists_edit"), &[("path", &path)])),
    ]
    .join("\n")
}

/// 디렉터리를 못 짓거나 파일을 못 썼다 — 반쯤 쓴 파일은 걷었다.
pub fn learn_failed(lang: Lang, path: &str, said: &str) -> String {
    fill(say(lang, "wiki.learn_failed"), &[("path", &one_line(path)), ("said", &one_line(said))])
}

/// 페이지는 섰는데 노트를 못 달았다 — 다시 칠 줄을 낸다.
pub fn learn_unnoted(lang: Lang, path: &str, id: &str, note: &str, said: &str) -> String {
    [
        fill(say(lang, "wiki.learn_unnoted"), &[("path", &one_line(path)), ("id", id), ("said", &one_line(said))]),
        format!("  {}", say(lang, "wiki.learn_unnoted_again")),
        format!("  moai note {id} {}", crate::text::quoted(note)),
    ]
    .join("\n")
}

/// `moai wiki learn` 의 사람 화면 — 쓴 자리, 단 노트, 그리고 커밋 줄. `dry` 면 쓸 자리와 글 전부다.
pub fn learned(
    lang: Lang,
    path: &str,
    issue: Option<&str>,
    note: &str,
    commit: &str,
    dry: Option<&str>,
) -> Vec<String> {
    let path = one_line(path);
    if let Some(text) = dry {
        let mut out = vec![paint(style::HEAD, &fill(say(lang, "wiki.learn_would"), &[("path", &path)]))];
        if let Some(id) = issue {
            out.push(fill(say(lang, "wiki.learn_would_note"), &[("id", id), ("note", note)]));
        }
        out.push(String::new());
        out.extend(text.lines().map(str::to_string));
        return out;
    }
    let mut out = vec![fill(say(lang, "wiki.learn_wrote"), &[("path", &path)])];
    if let Some(id) = issue {
        out.push(fill(say(lang, "wiki.learn_noted"), &[("id", &paint(style::ID, id)), ("note", note)]));
    }
    out.push(say(lang, "wiki.learn_commit").to_string());
    out.extend(commit.lines().map(|l| format!("  {l}")));
    out
}
