//! `moai wiki ls`·`moai wiki show` — 위키를 읽는다(moai-ihu4).
//!
//! **여기는 잇기만 한다.** 페이지를 걷고 푸는 것은 `wiki` 가, 사람 화면은 `view::wiki` 가 한다. 위키는 이
//! 체크아웃(`Repo::here`) 밑에서 읽고, 페이지가 댄 id 가 있는지는 트래커(딸린 워크트리면 옮겨 간 루트의 것)에
//! 묻는다 — 위키는 가지를 타는 문서고 트래커는 저장소에 하나다.
//!
//! **종료 코드는 못 읽었을 때만 비영이다.** 위키 디렉터리가 아직 없는 것은 안 쓴 위키라 0 이고(2026-10-04 사용자
//! 결정), 크기 상한을 넘은 페이지는 일부러 안 읽은 것이라 목록에서는 0 이다. 그 밖에 못 열었거나 끝까지 못
//! 읽은 디렉터리, 종류를 못 읽은 이름, 못 읽은 페이지가 하나라도 있으면 목록은 다 내고 비영으로
//! 끝난다(`note_partial`).

use super::{Ctx, R, note_partial, tell};
use crate::fail::{Fail, code};
use crate::i18n::Lang;
use crate::wiki::{self, DirTrouble, IssueRef, Link, Page, Wiki};
use std::collections::HashSet;

pub fn ls(ctx: &Ctx) -> R<Vec<String>> {
    let (dir, wiki) = read(ctx)?;
    let wiki = match wiki {
        Ok(w) => w,
        Err(t) => return trouble(ctx, &dir, &t, false),
    };
    for s in &wiki.skipped {
        tell(&crate::view::wiki::skipped(ctx.lang(), s));
    }
    if wiki.fell_short() {
        note_partial();
    }
    if ctx.json {
        return listed_json(ctx.lang(), &dir, &Ok(wiki));
    }
    Ok(crate::view::wiki::list(ctx.lang(), &dir, &wiki))
}

pub fn show(ctx: &Ctx, slug: &str) -> R<Vec<String>> {
    // 위키 전부를 읽는다 — 역링크는 다른 페이지가 적은 링크다(moai-ogaw). **종료 코드는 물은 페이지만 본다**: 다른
    // 페이지를 못 읽어 역링크가 덜 셌어도 그 페이지는 `moai wiki ls` 가 대고, 여기서 비영이면 옆 파일 하나가 멀쩡한
    // 페이지의 `show` 를 실패로 세운다. 덜 셌다는 것은 종료 코드 대신 `linked_from_partial` 과 화면의 한 줄이
    // 말한다(moai-mdzx.jty).
    let (dir, wiki) = read(ctx)?;
    let wiki = match wiki {
        Ok(w) => w,
        Err(t) => return trouble(ctx, &dir, &t, true),
    };
    let Some(page) = wiki.find(slug) else {
        return Err(Fail::coded(crate::view::wiki::no_page(ctx.lang(), slug, &dir), code::NOT_FOUND));
    };
    // 본문을 못 읽은 페이지 — 크기 상한이라도 물은 것은 본문이다.
    if page.error.is_some() {
        note_partial();
    }
    if ctx.json {
        return page_json(ctx.lang(), &wiki, page);
    }
    Ok(crate::view::wiki::page(ctx.lang(), page, wiki.uncounted(&page.slug)))
}

/// `moai wiki search <낱말…>` — 위키를 절 단위로 찾아 순위대로 낸다(moai-g8pt). 낱말은 빈칸 하나로 이어 한 질의다.
///
/// **종료 코드는 `ls` 와 같은 자다** — 결과가 없어도, 위키 디렉터리가 아직 없어도 0 이고, 못 읽은 페이지·건너뛴
/// 자리가 있으면 찾은 것은 다 내고 비영이다. 질의가 토큰 하나도 안 내면 사용 오류다.
pub fn search(ctx: &Ctx, words: &[String], limit: usize) -> R<Vec<String>> {
    let query = words.join(" ");
    // 위키를 읽기 전에 거절한다 — 질의가 틀린 것은 위키와 무관하다.
    if wiki::search::terms(&query).is_empty() {
        return Err(Fail::coded(crate::view::wiki::search_usage(ctx.lang()), code::BAD_INPUT));
    }
    // **트래커는 안 읽는다**(리뷰 moai-g8pt.9rd) — 검색은 페이지가 댄 id 가 트래커에 있는가(`IssueRef::exists`)를
    // 안 묻는다. `read` 로 읽던 판은 트래커의 못 읽는 줄 하나가 위키 검색을 비영으로 세우고 그 줄을 stderr 에 댔다.
    let repo = super::open_repo(ctx)?;
    let (dir, wiki) = read_in(&repo, &|_| true);
    let wiki = match wiki {
        Ok(w) => w,
        Err(t) => return trouble(ctx, &dir, &t, false),
    };
    for s in &wiki.skipped {
        tell(&crate::view::wiki::skipped(ctx.lang(), s));
    }
    if wiki.fell_short() {
        note_partial();
    }
    // `total` 은 `search` 가 자르기 전에 센다 — 다 받아 자르면 버릴 hit 의 발췌까지 짓는다.
    // `-n 0` 은 하나로 본다 — 빈 답은 "맞은 절 없음" 과 같은 얼굴이다.
    let found = wiki::search::search(&wiki.pages, &query, limit.max(1));
    let total = found.total;
    if ctx.json {
        let hits = found.hits.iter().map(HitOut::of).collect();
        let unread = wiki
            .pages
            .iter()
            .filter_map(|p| Some(UnreadOut { slug: &p.slug, path: &p.path, error: told_unread(ctx.lang(), p)? }))
            .collect();
        let skipped = skipped_out(ctx.lang(), &wiki.skipped);
        return super::json_line(&Searched {
            dir: &dir,
            query: &query,
            terms: &found.terms,
            hits,
            total,
            unread,
            skipped,
        });
    }
    Ok(crate::view::wiki::search(ctx.lang(), &dir, &query, &found.hits, total))
}

/// `moai wiki search --json` — `hits` 는 늘 선다(없으면 `[]`). `score` 는 순위를 보이는 값이지 계약이 아니다.
///
/// `unread`·`skipped` 는 `wiki ls --json` 의 페이지 `error` 와 `skipped` 를 같은 꼴·같은 키로 옮긴 것이고, 없으면 키가
/// 없다(리뷰 moai-g8pt.9rd). 못 읽은 페이지·건너뛴 자리가 있으면 종료 코드가 비영인데, 까닭이 stderr 의 사람 말에만
/// 서면 기계는 멀쩡해 보이는 `hits` 와 비영 종료만 받는다 — `Listed::skipped` 와 같은 까닭이다(moai-ihu4.x94).
#[derive(serde::Serialize)]
struct Searched<'a> {
    dir: &'a str,
    query: &'a str,
    terms: &'a [String],
    hits: Vec<HitOut<'a>>,
    total: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unread: Vec<UnreadOut<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedOut<'a>>,
}

/// 본문을 못 읽은 페이지 하나 — 그 페이지는 제목으로만 찾혔다. `error` 는 `wiki ls --json` 의 그 페이지 `error` 다
/// (크기 상한의 `too_large` 도 든다 — 그것만으로는 종료 코드가 0 이다).
#[derive(serde::Serialize)]
struct UnreadOut<'a> {
    slug: &'a str,
    path: &'a str,
    error: Told,
}

/// hit 하나. `heading`·`anchor` 는 첫 머리글 앞의 절이면 없고, `snippet`·`snippet_line` 은 보일 줄이 없으면(본문을
/// 못 읽은 페이지) 없다 — 없음이 답이다(`wiki ls --json` 의 `anchor` 와 같은 약속). 줄은 모두 1 부터다.
#[derive(serde::Serialize)]
struct HitOut<'a> {
    slug: &'a str,
    path: &'a str,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    heading: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor: Option<&'a str>,
    line: usize,
    score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    snippet: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snippet_line: Option<usize>,
}

impl<'a> HitOut<'a> {
    fn of(h: &'a wiki::search::Hit<'a>) -> HitOut<'a> {
        HitOut {
            slug: &h.page.slug,
            path: &h.page.path,
            title: &h.page.title,
            heading: h.heading.as_deref(),
            anchor: h.anchor,
            line: h.line,
            // 소수 둘로 — 부동소수 꼬리가 줄마다 다른 글자로 서지 않게.
            score: (h.score * 100.0).round() / 100.0,
            snippet: h.snippet.as_ref().map(|s| s.text.as_str()),
            snippet_line: h.snippet.as_ref().map(|s| s.line),
        }
    }
}

/// 트래커를 열고(id 가 있는가를 물을 자리) 이 체크아웃의 위키를 읽는다 — `wiki_dir` 의 날글자와 함께.
fn read(ctx: &Ctx) -> R<(String, Result<Wiki, DirTrouble>)> {
    let repo = super::open_repo(ctx)?;
    let load = repo.read()?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);
    // **못 읽는 줄이 쓰는 id 도 있는 id 다** — 그 줄은 파일에 있다. 빼면 그 id 를 댄 페이지가 "없는 id" 로 선다.
    let reserved = load.reserved_ids();
    let ids: HashSet<&str> =
        load.issues.iter().map(|i| i.id.as_str()).chain(reserved.iter().map(String::as_str)).collect();
    Ok(read_in(&repo, &|id: &str| ids.contains(id)))
}

/// 이미 연 트래커 곁에서 이 체크아웃의 위키를 읽는다 — `wiki_dir` 의 날글자와 함께. `known` 은 그 id 의 줄이
/// 트래커에 있는가를 답한다. `moai show <id>` 가 이것으로 읽는다(moai-g8pt.5aw) — 이미 읽은 트래커를 다시 열지 않는다.
pub(crate) fn read_in(repo: &crate::store::Repo, known: &dyn Fn(&str) -> bool) -> (String, Result<Wiki, DirTrouble>) {
    let dir = repo.config.wiki_dir.clone();
    let wiki = wiki::load(repo.here(), &dir, &repo.config.prefix, known);
    (dir, wiki)
}

/// 위키 디렉터리를 못 쓴 까닭을 기계의 꼴로 — `moai show <id> --json` 의 `wiki_error` 다(moai-g8pt.5aw). **없는
/// 디렉터리는 까닭이 아니다** — 아직 안 쓴 위키라 두 빈 배열이 답이다(`DirTrouble::fell`). `kind` 는 `wiki ls --json`
/// 의 `error.kind` 와 같은 낱말이다.
pub(crate) fn trouble_told(lang: Lang, dir: &str, t: &DirTrouble) -> Option<Told> {
    t.fell().then(|| Told { kind: t.kind(), said: crate::view::wiki::dir_trouble(lang, dir, t) })
}

/// `moai show <id> --json` 의 `wiki` — 두 배열은 늘 선다(비었으면 `[]`). 목록 `show [거르개] --json` 은 안 싣는다:
/// 줄마다 위키 전부를 걷게 된다. 관련 절은 `wiki search --json` 의 hit 와 한 꼴이다 — 발췌 없이 매긴 순위
/// ([`wiki::search::rank`])라 `snippet`·`snippet_line` 이 안 선다. 꼴을 따로 두면 두 표면의 키가 갈린다.
#[derive(serde::Serialize)]
pub(crate) struct AboutOut<'a> {
    named_in: Vec<NamedOut<'a>>,
    related: Vec<HitOut<'a>>,
}

/// 이 이슈를 댄 페이지 — `via` 는 걸린 id 다(제 id 나 에픽 id).
#[derive(serde::Serialize)]
struct NamedOut<'a> {
    slug: &'a str,
    path: &'a str,
    title: &'a str,
    via: &'a str,
}

impl<'a> AboutOut<'a> {
    pub(crate) fn of(a: &'a wiki::about::About<'a>) -> AboutOut<'a> {
        AboutOut {
            named_in: a
                .named_in
                .iter()
                .map(|n| NamedOut { slug: &n.page.slug, path: &n.page.path, title: &n.page.title, via: n.via })
                .collect(),
            related: a.related.iter().map(HitOut::of).collect(),
        }
    }
}

/// 위키 디렉터리를 못 쓴다. `--json` 은 `{"dir","error"}` 를 stdout 에 내고, 사람 화면은 없는 디렉터리면 한 줄로
/// 알리고 나머지는 실패로 선다. 페이지를 물었으면(`asked`) 없는 디렉터리도 그 페이지를 못 낸 것이라 비영이다.
fn trouble(ctx: &Ctx, dir: &str, t: &DirTrouble, asked: bool) -> R<Vec<String>> {
    let fell = t.fell() || asked;
    if ctx.json {
        if fell {
            note_partial();
        }
        return listed_json(ctx.lang(), dir, &Err(t.clone()));
    }
    let said = crate::view::wiki::dir_trouble(ctx.lang(), dir, t);
    if fell {
        let code = if asked && !t.fell() { code::NOT_FOUND } else { code::ERROR };
        return Err(Fail::coded(said, code));
    }
    Ok(vec![said])
}

/// `moai wiki ls --json` 의 한 줄 — 위키를 읽었으면 `{"dir","pages"}`, 디렉터리를 못 쓰면 `{"dir","error"}`.
///
/// **moai-wiki 스킬이 이 키 이름을 가르친다**(`guide::wiki`). 그 글이 대는 키를 이 출력에 묶는 시험
/// (`guide::tests::the_wiki_skill_teaches_the_keys_the_wiki_prints`)이 이 함수를 부른다 — 명령과 시험이 같은 자로
/// 짓지 않으면 그 시험은 손으로 옮긴 꼴과 견주게 되고, 키가 바뀌어도 초록이다(moai-ihu4.l2e).
pub(crate) fn listed_json(lang: Lang, dir: &str, read: &Result<Wiki, DirTrouble>) -> R<Vec<String>> {
    match read {
        Ok(w) => {
            let pages = w.pages.iter().map(|p| PageOut::of(lang, p)).collect();
            super::json_line(&Listed { dir, pages, skipped: skipped_out(lang, &w.skipped) })
        }
        Err(t) => {
            let said = crate::view::wiki::dir_trouble(lang, dir, t);
            super::json_line(&Refused { dir, error: Told { kind: t.kind(), said } })
        }
    }
}

/// `moai wiki show <slug> --json` 의 한 줄 — 목록의 페이지에 원문 `body` 와, 역링크를 덜 셌을 때 `linked_from_partial`
/// 을 더한 것. `w` 는 그 페이지가 든 위키다 — 덜 셌는가는 다른 페이지가 답한다([`Wiki::uncounted`]). [`listed_json`]
/// 과 같은 까닭으로 스킬의 키를 재는 시험이 함께 부른다.
pub(crate) fn page_json(lang: Lang, w: &Wiki, p: &Page) -> R<Vec<String>> {
    let partial = w.uncounted(&p.slug) > 0;
    super::json_line(&PageOut { body: p.body.as_deref(), linked_from_partial: partial, ..PageOut::of(lang, p) })
}

/// `moai wiki ls --json` — **moai-wiki 스킬이 이 키 이름을 읽는다**(moai-ihu4 본문의 계약). 키를 바꾸면 그 에픽에
/// 알린다.
#[derive(serde::Serialize)]
struct Listed<'a> {
    dir: &'a str,
    pages: Vec<PageOut<'a>>,
    /// 걷다가 페이지로 못 세운 자리 — 없으면 키가 없다(2026-10-04 사용자 결정, moai-ihu4.x94). 이것이 서면 종료
    /// 코드가 비영인데, 까닭이 stderr 의 사람 말에만 서면 기계는 성해 보이는 `pages` 와 비영 종료만 받는다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedOut<'a>>,
}

/// 걷다가 건너뛴 자리들의 `--json` 꼴 — `wiki ls` 와 `wiki search` 가 같은 자로 짓는다.
fn skipped_out(lang: Lang, skipped: &[wiki::Skipped]) -> Vec<SkippedOut<'_>> {
    skipped
        .iter()
        .map(|s| SkippedOut { path: &s.path, kind: s.why.kind(), said: crate::view::wiki::skip_reason(lang, s) })
        .collect()
}

/// 페이지 본문을 못 읽은 까닭의 `--json` 꼴 — 다 읽었으면 없다. `wiki ls`·`show` 의 `error` 와 `wiki search` 의
/// `unread[].error` 가 같은 자로 짓는다.
fn told_unread(lang: Lang, p: &Page) -> Option<Told> {
    p.error.as_ref().map(|u| Told { kind: u.kind(), said: crate::view::wiki::unread(lang, p, u) })
}

/// 건너뛴 자리 하나 — `path` 는 체크아웃에서의 상대 경로, `kind` 는 `dir_link`·`not_utf8`·`unreadable`.
#[derive(serde::Serialize)]
struct SkippedOut<'a> {
    path: &'a str,
    kind: &'static str,
    said: String,
}

/// 위키 디렉터리를 못 쓸 때 — `pages` 대신 `error` 가 선다.
#[derive(serde::Serialize)]
struct Refused<'a> {
    dir: &'a str,
    error: Told,
}

/// `commits_error` 와 같은 꼴이다 — 가르는 것은 `kind` 고 `said` 는 사람이 읽는 한 줄이다.
#[derive(serde::Serialize)]
pub(crate) struct Told {
    kind: &'static str,
    said: String,
}

/// 페이지 하나. `error` 는 본문을 못 읽은 페이지에만 서고 없으면 다 읽은 것이다 — `commits_error`·`journal_error`
/// 와 같은 약속이다(2026-10-04 사용자 결정). `body` 는 `show` 만 낸다 — 늘 원문이다. `linked_from` 은 이 페이지를
/// 가리키는 페이지의 슬러그고 **늘 선다** — 아무도 안 가리키면 `[]` 다(2026-10-04 사용자 결정, moai-ogaw).
///
/// `linked_from_partial` 도 `show` 만 낸다 — 다른 페이지를 못 읽었거나 걷기가 건너뛴 자리가 있어 `linked_from` 이
/// 덜 섰을 수 있을 때만 `true` 로 서고, 다 셌으면 키가 없다(2026-10-04 사용자 결정, moai-mdzx.jty). 목록은 안 단다 —
/// 못 읽은 페이지의 `error` 와 `skipped` 가 그 줄에 이미 서서, 줄마다 달면 같은 말을 페이지 수만큼 되풀이한다.
#[derive(serde::Serialize)]
struct PageOut<'a> {
    slug: &'a str,
    title: &'a str,
    path: &'a str,
    bytes: u64,
    issues: &'a [IssueRef],
    links: &'a [Link],
    linked_from: &'a [String],
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    linked_from_partial: bool,
    conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Told>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
}

impl<'a> PageOut<'a> {
    /// 목록의 줄 — `body`·`linked_from_partial` 은 비었다. `show` 는 [`page_json`] 이 둘을 채운다.
    fn of(lang: Lang, p: &'a Page) -> PageOut<'a> {
        PageOut {
            slug: &p.slug,
            title: &p.title,
            path: &p.path,
            bytes: p.bytes,
            issues: &p.issues,
            links: &p.links,
            linked_from: &p.linked_from,
            linked_from_partial: false,
            conflict: p.conflict,
            error: told_unread(lang, p),
            body: None,
        }
    }
}

/// `moai wiki learn` — 교훈 페이지를 새로 짓고, 그 교훈이 나온 이슈에 `Lesson:` 노트를 단다(moai-4s5n.n7j).
///
/// **쓰는 차례는 페이지 먼저, 노트 나중이다** — "스냅샷 먼저, 저널 나중" 과 같은 결이다. 노트가 지면 페이지는
/// 남기고 비영으로 끝나며 다시 칠 `moai note` 줄을 낸다: 빠진 노트가 사라진 교훈보다 싸다.
///
/// - 페이지는 **이 체크아웃**(`Repo::here`)의 위키에 선다 — 위키는 가지를 탄다. 노트는 트래커로 간다(딸린
///   워크트리면 루트의 것)
/// - 파일은 `create_new` 로 연다. 이미 있으면 거절하고 그 파일을 안 건드린다 — 두 세션이 같은 슬러그를 고른
///   경합도 여기서 하나만 이긴다. 페이지를 고치는 동사가 아니다(`cli::WikiCmd`)
/// - 위키 디렉터리 자체가 없으면 짓지 않는다 — `wiki_dir` 이 틀렸을 수 있다. 그 밑의 `lessons/` 는 짓는다
/// - **쓰기 전에 다 잰다** — 꼴·슬러그·이슈·사람·이미 선 파일. 그 뒤에 지는 것은 디스크뿐이다
/// - 커밋하지 않는다. 끝 줄이 브리프 7-4 와 같은 커밋 줄([`crate::guide::wiki_commit`])을 댄다
pub fn learn(ctx: &Ctx, args: crate::cli::LearnArgs) -> R<Vec<String>> {
    use crate::wiki::learn;
    let lang = ctx.lang();
    // 꼴은 디스크도 트래커도 안 보고 잰다 — 틀린 부름이 저장소를 열 까닭이 없다. stdin 은 여기서 읽는다(락은 노트를
    // 달 때 `with_write` 안에서만 잡힌다).
    let slug = match &args.slug {
        Some(s) if learn::is_slug(s) => s.clone(),
        Some(s) => return Err(Fail::coded(crate::view::wiki::learn_bad_slug(lang, s), code::BAD_INPUT)),
        None => learn::slug_from(&args.title)
            .ok_or_else(|| Fail::coded(crate::view::wiki::learn_no_slug(lang, args.title.trim()), code::BAD_INPUT))?,
    };
    let body = super::add::read_body(args.body)?.unwrap_or_default();
    // 꼴은 이슈를 모른 채 먼저 잰다 — 틀린 본문이 사람을 묻거나 트래커를 읽을 까닭이 없다.
    learn::page(&args.title, &body, None)
        .map_err(|why| Fail::coded(crate::view::wiki::learn_refused(lang, &why), code::BAD_INPUT))?;
    let repo = super::open_repo(ctx)?;

    // 자리 — 이 체크아웃의 위키 밑 `lessons/<slug>.md`. 위키 디렉터리는 [`wiki::dir_of`] 가 잰다(체크아웃 안,
    // `.git/` 밖, 디렉터리).
    let raw = repo.config.wiki_dir.clone();
    let dir = match wiki::dir_of(repo.here(), &raw) {
        Ok(d) => d,
        Err(wiki::DirTrouble::NoDir) => {
            return Err(Fail::coded(crate::view::wiki::learn_no_wiki(lang, &raw), code::NOT_FOUND));
        }
        Err(t) => return Err(Fail::coded(crate::view::wiki::dir_trouble(lang, &raw, &t), code::ERROR)),
    };
    // 노트를 달 줄 — `none` 이면 없다. 없는 id 는 페이지를 쓰기 **전에** 거절한다: 오타 하나로 노트 없는 페이지가
    // 남으면 그 거절은 고칠 줄이 아니라 치울 파일을 낸다.
    let target = match args.issue.as_deref() {
        Some("none") => None,
        given => {
            let by = crate::model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, lang))?;
            let load = repo.read()?;
            super::report_load_errors(lang, &repo.issues_path(), &load.errors);
            let id = match given {
                Some(id) if load.issues.iter().any(|i| i.id == id) => id.to_string(),
                Some(id) => return Err(Fail::not_found(id, lang)),
                // **`prime` 이 "쥐었다" 고 대는 그 줄이다**(`report::held_by`) — 고르는 자를 둘로 두지 않는다.
                None => {
                    let me = crate::query::Me::of(&by);
                    let held = crate::report::held_by(&load.issues, &repo.config, Some(&me));
                    match held.as_slice() {
                        [one] => one.id.clone(),
                        many => {
                            let ids: Vec<&str> = many.iter().map(|i| i.id.as_str()).collect();
                            return Err(Fail::coded(crate::view::wiki::learn_which_issue(lang, &ids), code::BAD_INPUT));
                        }
                    }
                }
            };
            Some((id, by))
        }
    };
    let issue = target.as_ref().map(|(id, _)| id.as_str());
    // 위에서 잰 꼴이라 지지 않는다 — 다른 것은 `Learned in:` 줄 하나다.
    let text = learn::page(&args.title, &body, issue)
        .map_err(|why| Fail::coded(crate::view::wiki::learn_refused(lang, &why), code::BAD_INPUT))?;

    let shown: Vec<&str> = std::path::Path::new(&raw)
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => n.to_str(),
            _ => None,
        })
        .collect();
    let file = format!("{slug}.md");
    let path = shown.iter().copied().chain([learn::LESSONS, file.as_str()]).collect::<Vec<_>>().join("/");
    let note = learn::note(&slug);
    let exists = || Fail::coded(crate::view::wiki::learn_exists(lang, &path, issue, &note), code::ALREADY_EXISTS);
    let lessons = dir.join(learn::LESSONS);
    let at = lessons.join(&file);
    // 링크도 센다 — 끝이 없는 링크도 그 자리는 섰다(`create_new` 도 그것을 안 따르고 진다).
    if std::fs::symlink_metadata(&at).is_ok() {
        return Err(exists());
    }

    if args.dry_run {
        return said(ctx, &path, &slug, issue, false, &note, &raw, Some(&text));
    }

    // `lessons/` 를 짓는다 — 체크아웃 밖을 가리키는 링크면 안 짓는다(아카이브 디렉터리와 같은 자, `held::place_dir`).
    let home = crate::held::Home::of(repo.here());
    let lessons = crate::held::place_dir(&lessons, &home)
        .map_err(|u| Fail::coded(crate::held::refused(lang, &lessons, &u), code::BROKEN))?;
    std::fs::create_dir_all(&lessons)
        .map_err(|e| Fail::coded(crate::view::wiki::learn_failed(lang, &path, &e.to_string()), code::ERROR))?;
    write_new(&lessons.join(&file), &text).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => exists(),
        _ => Fail::coded(crate::view::wiki::learn_failed(lang, &path, &e.to_string()), code::ERROR),
    })?;

    // 노트 — 페이지는 이미 섰다. 지면 페이지는 남기고, 다시 칠 줄을 내며 비영으로 끝난다.
    if let Some((id, by)) = &target {
        super::note::append(&repo, lang, id, &note, by)
            .map_err(|e| Fail::coded(crate::view::wiki::learn_unnoted(lang, &path, id, &note, &e.message), e.code))?;
    }
    said(ctx, &path, &slug, issue, target.is_some(), &note, &raw, None)
}

/// 새 파일에 글을 쓴다 — **있으면 진다**(`create_new`). 쓰다 지면 반쯤 쓴 파일을 걷는다: 꼴이 깨진 교훈이 남으면
/// 다음 부름이 "이미 있다" 로 막힌다. 쓰고 나면 `sync` 한다 — 노트가 그 페이지를 가리키는데 페이지가 디스크에
/// 없으면 안 된다.
fn write_new(at: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(at)?;
    let wrote = f.write_all(text.as_bytes()).and_then(|()| f.sync_all());
    if wrote.is_err() {
        drop(f);
        let _ = std::fs::remove_file(at);
    }
    wrote
}

/// `moai wiki learn --json` — `issue` 는 `--issue none` 이면 없다. `dry_run`·`text` 는 `--dry-run` 에만 선다.
#[derive(serde::Serialize)]
struct Learned<'a> {
    path: &'a str,
    slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    issue: Option<&'a str>,
    noted: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

/// 쓴 것(또는 쓸 것)을 낸다 — `dry` 는 `--dry-run` 의 글이다.
fn said(
    ctx: &Ctx,
    path: &str,
    slug: &str,
    issue: Option<&str>,
    noted: bool,
    note: &str,
    dir: &str,
    dry: Option<&str>,
) -> R<Vec<String>> {
    if ctx.json {
        return super::json_line(&Learned {
            path,
            slug: crate::wiki::learn::page_slug(slug),
            issue,
            noted,
            dry_run: dry.is_some(),
            text: dry,
        });
    }
    let dir = if dir.is_empty() { "." } else { dir };
    let commit = crate::guide::wiki_commit(&crate::text::shell_word(dir));
    Ok(crate::view::wiki::learned(ctx.lang(), path, issue, note, &commit, dry))
}
