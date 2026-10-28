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
    let (repo, load, dir, wiki) = read(ctx)?;
    let wiki = match wiki {
        Ok(w) => w,
        Err(t) => return trouble(ctx, repo.here(), &dir, &t, false),
    };
    for s in &wiki.skipped {
        tell(&crate::view::wiki::skipped(ctx.lang(), s));
    }
    if wiki.fell_short() {
        note_partial();
    }
    // **교훈이 선 이슈는 노트에서 읽는다**(moai-4s5n.c0n) — 저장하지 않고, 아무것도 막지 않는다. **쓸 데가 있을 때만
    // 읽는다**(리뷰 moai-4s5n.u8x) — 저널 전부와 아카이브 전부를 걷는 값이다. `--json` 의 `seen_count` 는 교훈 페이지에만
    // 0 아닌 값이 서고(노트는 교훈 슬러그만 가리킨다), 사람 화면은 빈 위키에서 노트를 안 쓴다(페이지 없는 노트의 알림은
    // 페이지가 있는 목록 밑에만 선다).
    let wanted = match ctx.json {
        true => wiki.pages.iter().any(|p| wiki::seen::is_lesson(&p.slug)),
        false => !wiki.pages.is_empty(),
    };
    let lessons = if wanted { lessons_seen(ctx.lang(), &repo, load)? } else { Default::default() };
    if ctx.json {
        return listed_json(ctx.lang(), repo.here(), &dir, &Ok(wiki), &lessons);
    }
    Ok(crate::view::wiki::list(ctx.lang(), &dir, &wiki, &lessons))
}

/// 교훈마다 선 이슈 — 아카이브된 줄의 노트도 센다(`moai stats` 의 `work` 가 아카이브까지 세는 것과 같은 까닭: 오래전
/// 교훈의 이력이 말없이 빠지지 않게). 저널은 `Lesson:` 이 들 수 있는 줄만 푼다(`model::may_hold_lesson`).
///
/// `load` 는 [`read`] 가 이미 푼 산 스냅샷이다 — 여기서 다시 풀던 판은 한 부름에 스냅샷을 두 번 풀었고, 그 사이에 떨어진
/// 쓰기를 한쪽만 봤다(리뷰 moai-4s5n.u8x). 산 줄만 보니 옆 워크트리를 겹치지 않는다(`Origin` 의 빈 값).
fn lessons_seen(lang: Lang, repo: &crate::store::Repo, load: crate::store::Load) -> R<wiki::seen::Lessons> {
    let load = crate::archive::read_all(&repo.root, load)?;
    // **아카이브의 못 읽는 줄은 말한다** — 그 줄의 노트가 셈에서 빠진다(`moai stats` 와 같은 자). 산 파일의 것은 [`read`]
    // 가 이미 댔다. 부분 실패 깃발은 안 세운다: 교훈 셈은 목록의 덤이고, 목록의 종료 코드는 위키를 못 읽었을 때만
    // 비영이다(모듈 머리).
    let archived: Vec<crate::store::LoadError> = load.errors.iter().filter(|e| e.source.is_some()).cloned().collect();
    super::name_load_errors(lang, &repo.issues_path(), &archived);
    let journal = super::show::journal_of_rows(repo, &Default::default(), &load.issues, crate::model::may_hold_lesson);
    Ok(wiki::seen::seen_in(&load.issues, &journal))
}

pub fn show(ctx: &Ctx, slug: &str) -> R<Vec<String>> {
    // 위키 전부를 읽는다 — 역링크는 다른 페이지가 적은 링크다(moai-ogaw). **종료 코드는 물은 페이지만 본다**: 다른
    // 페이지를 못 읽어 역링크가 덜 셌어도 그 페이지는 `moai wiki ls` 가 대고, 여기서 비영이면 옆 파일 하나가 멀쩡한
    // 페이지의 `show` 를 실패로 세운다. 덜 셌다는 것은 종료 코드 대신 `linked_from_partial` 과 화면의 한 줄이
    // 말한다(moai-mdzx.jty).
    let (repo, load, dir, wiki) = read(ctx)?;
    let wiki = match wiki {
        Ok(w) => w,
        Err(t) => return trouble(ctx, repo.here(), &dir, &t, true),
    };
    let Some(page) = wiki.find(slug) else {
        return Err(Fail::coded(crate::view::wiki::no_page(ctx.lang(), slug, &dir), code::NOT_FOUND));
    };
    // 본문을 못 읽은 페이지 — 크기 상한이라도 물은 것은 본문이다.
    if page.error.is_some() {
        note_partial();
    }
    // 저널은 교훈 페이지를 물었을 때만 연다 — 다른 페이지의 `seen` 은 늘 빈 배열이다.
    let seen = match wiki::seen::is_lesson(&page.slug) {
        true => lessons_seen(ctx.lang(), &repo, load)?.remove(&page.slug).unwrap_or_default(),
        false => Vec::new(),
    };
    if ctx.json {
        return page_json(ctx.lang(), &wiki, page, &seen);
    }
    Ok(crate::view::wiki::page(ctx.lang(), page, wiki.uncounted(&page.slug), &seen, ctx.zone()))
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
        Err(t) => return trouble(ctx, repo.here(), &dir, &t, false),
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
        let abs = dir_abs(repo.here(), &dir, None);
        return super::json_line(&Searched {
            dir: &dir,
            dir_abs: abs.as_deref(),
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
    /// `wiki ls --json` 의 `dir_abs` 와 같은 값이다 — 거절의 꼴(`{dir,dir_abs,error}`)과 한 키로 서게.
    #[serde(skip_serializing_if = "Option::is_none")]
    dir_abs: Option<&'a str>,
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

/// 트래커를 열고(id 가 있는가를 물을 자리) 이 체크아웃의 위키를 읽는다 — `wiki_dir` 의 날글자와 함께. 푼 산 스냅샷도
/// 함께 낸다 — 교훈 셈([`lessons_seen`])이 그것을 다시 풀지 않게.
fn read(ctx: &Ctx) -> R<(crate::store::Repo, crate::store::Load, String, Result<Wiki, DirTrouble>)> {
    let repo = super::open_repo(ctx)?;
    let load = repo.read()?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);
    // **못 읽는 줄이 쓰는 id 도 있는 id 다** — 그 줄은 파일에 있다. 빼면 그 id 를 댄 페이지가 "없는 id" 로 선다.
    // **아카이브로 옮긴 줄의 id 도 있는 id 다**(리뷰 moai-4s5n.u8x) — `moai show <id>` 가 그 줄을 연다. 빼던 판은
    // `Learned in: <id>` 로 끝나는 교훈 페이지마다, 그 이슈가 아카이브로 간 날부터 "없는 id" 알림을 영영 세웠다. 줄을
    // 풀지 않고 id 만 훑는다(`archive::reserve`, 쓰기가 새 id 를 피하는 그 자).
    let reserved = load.reserved_ids();
    let archived = crate::archive::reserve(&repo.root).ids;
    let (dir, wiki) = {
        let ids: HashSet<&str> = load
            .issues
            .iter()
            .map(|i| i.id.as_str())
            .chain(reserved.iter().map(String::as_str))
            .chain(archived.iter().map(String::as_str))
            .collect();
        read_in(&repo, &|id: &str| ids.contains(id))
    };
    Ok((repo, load, dir, wiki))
}

/// 이미 연 트래커 곁에서 이 체크아웃의 위키를 읽는다 — `wiki_dir` 의 날글자와 함께. `known` 은 그 id 의 줄이
/// 트래커에 있는가를 답한다. `moai show <id>` 가 이것으로 읽는다(moai-g8pt.5aw) — 이미 읽은 트래커를 다시 열지 않는다.
pub(crate) fn read_in(repo: &crate::store::Repo, known: &dyn Fn(&str) -> bool) -> (String, Result<Wiki, DirTrouble>) {
    let dir = repo.config.wiki_dir.clone();
    let wiki = wiki::load(repo.here(), &dir, &repo.config.prefix, known);
    (dir, wiki)
}

/// 위키 디렉터리를 못 쓴 까닭을 기계의 꼴로 — `moai show <id> --json` 의 `wiki_error` 다(moai-g8pt.5aw). **없는
/// 디렉터리는 까닭이 아니다** — 아직 안 쓴 위키라 빈 `named_in`·`related` 가 답이다(`DirTrouble::fell`). `lessons` 는
/// 그 줄의 노트에서 읽어 위키가 없어도 선다. `kind` 는 `wiki ls --json` 의 `error.kind` 와 같은 낱말이다.
pub(crate) fn trouble_told(lang: Lang, dir: &str, t: &DirTrouble) -> Option<Told> {
    t.fell().then(|| Told { kind: t.kind(), said: crate::view::wiki::dir_trouble(lang, dir, t) })
}

/// `moai show <id> --json` 의 `wiki` — 세 배열은 늘 선다(비었으면 `[]`). 목록 `show [거르개] --json` 은 안 싣는다:
/// 줄마다 위키 전부를 걷게 된다. 관련 절은 `wiki search --json` 의 hit 와 한 꼴이다 — 발췌 없이 매긴 순위
/// ([`wiki::search::rank`])라 `snippet`·`snippet_line` 이 안 선다. 꼴을 따로 두면 두 표면의 키가 갈린다.
#[derive(serde::Serialize)]
pub(crate) struct AboutOut<'a> {
    named_in: Vec<NamedOut<'a>>,
    related: Vec<RelatedOut<'a>>,
    /// 이 이슈의 `Lesson:` 노트가 가리킨 슬러그 — 늘 선다(moai-4s5n.c0n).
    lessons: Vec<&'a str>,
}

/// 관련 절 하나 — `wiki search` 의 hit 에 교훈 페이지의 표시 `lesson` 이 더해진다. 교훈일 때만 서고 없으면 아니다.
#[derive(serde::Serialize)]
struct RelatedOut<'a> {
    #[serde(flatten)]
    hit: HitOut<'a>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    lesson: bool,
    /// 이 절의 머리글이 페이지 제목 그 자체인가(제목 절) — 그때만 서고, 화면은 `제목 › 머리글` 의 머리글을 안 단다
    /// ([`crate::view::wiki::section_of`]). `heading` 은 적힌 그대로라 **web 상세가 이것을 읽는다**(리뷰 moai-xp7l.mug) —
    /// 낱말을 브라우저에서 다시 가르던 판은 토큰 자가 달라(`moai`의 위키, 이스케이프·강조·링크) CLI 와 탐색기가 안 다는
    /// 머리글을 달았다.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    title_heading: bool,
}

/// 이 이슈를 댄 페이지 — `via` 는 걸린 id 다(제 id 나 에픽 id).
#[derive(serde::Serialize)]
struct NamedOut<'a> {
    slug: &'a str,
    path: &'a str,
    title: &'a str,
    via: &'a str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    lesson: bool,
}

impl<'a> AboutOut<'a> {
    pub(crate) fn of(a: &'a wiki::about::About<'a>) -> AboutOut<'a> {
        AboutOut {
            named_in: a
                .named_in
                .iter()
                .map(|n| NamedOut {
                    slug: &n.page.slug,
                    path: &n.page.path,
                    title: &n.page.title,
                    via: n.via,
                    lesson: wiki::seen::is_lesson(&n.page.slug),
                })
                .collect(),
            related: a
                .related
                .iter()
                .map(|h| RelatedOut {
                    hit: HitOut::of(h),
                    lesson: wiki::seen::is_lesson(&h.page.slug),
                    title_heading: h.heading.is_some() && crate::view::wiki::section_of(h).is_none(),
                })
                .collect(),
            lessons: a.lessons.iter().map(|l| l.slug).collect(),
        }
    }
}

/// 위키 디렉터리를 못 쓴다. `--json` 은 `{"dir","error"}` 를 stdout 에 내고, 사람 화면은 없는 디렉터리면 한 줄로
/// 알리고 나머지는 실패로 선다. 페이지를 물었으면(`asked`) 없는 디렉터리도 그 페이지를 못 낸 것이라 비영이다.
fn trouble(ctx: &Ctx, here: &std::path::Path, dir: &str, t: &DirTrouble, asked: bool) -> R<Vec<String>> {
    let fell = t.fell() || asked;
    if ctx.json {
        if fell {
            note_partial();
        }
        return listed_json(ctx.lang(), here, dir, &Err(t.clone()), &Default::default());
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
///
/// `here` 는 이 체크아웃의 뿌리다 — `dir_abs` 를 그것에 잇는다([`dir_abs`]).
pub(crate) fn listed_json(
    lang: Lang,
    here: &std::path::Path,
    dir: &str,
    read: &Result<Wiki, DirTrouble>,
    lessons: &wiki::seen::Lessons,
) -> R<Vec<String>> {
    let abs = dir_abs(here, dir, read.as_ref().err());
    let dir_abs = abs.as_deref();
    match read {
        Ok(w) => {
            let pages = w
                .pages
                .iter()
                .map(|p| PageOut { seen_count: Some(lessons.get(&p.slug).map_or(0, Vec::len)), ..PageOut::of(lang, p) })
                .collect();
            super::json_line(&Listed { dir, dir_abs, pages, skipped: skipped_out(lang, &w.skipped) })
        }
        Err(t) => {
            let said = crate::view::wiki::dir_trouble(lang, dir, t);
            super::json_line(&Refused { dir, dir_abs, error: Told { kind: t.kind(), said } })
        }
    }
}

/// 이 체크아웃의 위키 디렉터리 절대 경로 — `wiki ls --json` 의 `dir_abs` 다(moai-xp7l.ml4). `dir` 은 트래커 뿌리
/// 기준 상대 경로라, 그것을 쓰려는 에이전트는 기준 자리를 따로 알아야 했다. 링크를 풀지 않고 글자로 잇는다 —
/// `dir` 을 적은 그대로의 자리다. **`outside` 면 없다** — 체크아웃 밖이나 `.git/` 안을 가리키는 자리를 쓸 자리로
/// 내밀지 않는다. 디렉터리가 아직 없는(`no_dir`) 것은 선다: 첫 페이지를 지을 자리다.
///
/// 글자로 접는 자는 [`crate::path::lexical`] 하나다(`./docs` 의 `.` 를 걷는다 — 절대 경로와 `..` 는 `outside` 라 여기 안
/// 온다). **UTF-8 이 아닌 자리는 안 낸다**(리뷰 moai-xp7l.mug) — 손실 변환한 글은 `U+FFFD` 가 든, 아무 데도 없는 자리를
/// 대고, 그 자리에 쓰라고 배운 에이전트는 엉뚱한 디렉터리를 짓는다.
fn dir_abs(here: &std::path::Path, dir: &str, trouble: Option<&DirTrouble>) -> Option<String> {
    if matches!(trouble, Some(DirTrouble::Outside(_))) {
        return None;
    }
    // **구분자는 `/` 로 통일한다** — Windows 의 `C:\...\docs` 를 그대로 내면 `--json` 의 계약(`.../docs` 로 끝난다)이
    // 플랫폼마다 달라지고, 스킬이 그 글을 다시 이어 붙일 때도 섞인다. Windows 도 `C:/...` 를 받아 준다.
    crate::path::lexical(&here.join(dir))
        .to_str()
        .map(|s| if cfg!(windows) { s.replace('\\', "/") } else { s.to_string() })
}

/// `moai wiki show <slug> --json` 의 한 줄 — 목록의 페이지에 원문 `body` 와, 역링크를 덜 셌을 때 `linked_from_partial`
/// 을 더한 것. `w` 는 그 페이지가 든 위키다 — 덜 셌는가는 다른 페이지가 답한다([`Wiki::uncounted`]). [`listed_json`]
/// 과 같은 까닭으로 스킬의 키를 재는 시험이 함께 부른다.
pub(crate) fn page_json(lang: Lang, w: &Wiki, p: &Page, seen: &[wiki::seen::Seen]) -> R<Vec<String>> {
    let partial = w.uncounted(&p.slug) > 0;
    let seen =
        seen.iter().map(|s| SeenOut { id: &s.id, at: &s.at, by: &s.by, by_email: s.by_email.as_deref() }).collect();
    super::json_line(&PageOut {
        body: p.body.as_deref(),
        linked_from_partial: partial,
        seen: Some(seen),
        ..PageOut::of(lang, p)
    })
}

/// `wiki show --json` 의 `seen` 한 칸 — 이 교훈이 선 이슈, 그 이슈에서 가장 나중에 적은 때와 사람(moai-4s5n.c0n). 사람은
/// 저널 줄처럼 `by` 와 `by_email` 로 갈라 든다 — 메일이 없는 옛 줄이면 `by_email` 이 없다(`journal`·`work` 와 같은 자).
#[derive(serde::Serialize)]
struct SeenOut<'a> {
    id: &'a str,
    at: &'a str,
    by: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    by_email: Option<&'a str>,
}

/// `moai wiki ls --json` — **moai-wiki 스킬이 이 키 이름을 읽는다**(moai-ihu4 본문의 계약). 키를 바꾸면 그 에픽에
/// 알린다.
#[derive(serde::Serialize)]
struct Listed<'a> {
    dir: &'a str,
    /// 이 체크아웃의 위키 디렉터리 절대 경로 — [`dir_abs`]. `outside` 가 아니면 늘 선다.
    #[serde(skip_serializing_if = "Option::is_none")]
    dir_abs: Option<&'a str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    dir_abs: Option<&'a str>,
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
    /// 서로 다른 이슈 수 — `ls` 만 내고 **늘 선다**(교훈이 아닌 페이지도 0). 노트에서 읽은 값이고 저장하지 않는다.
    #[serde(skip_serializing_if = "Option::is_none")]
    seen_count: Option<usize>,
    /// 이 교훈이 선 이슈, 최근 먼저 — `show` 만 내고 **늘 선다**(교훈이 아닌 페이지는 `[]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    seen: Option<Vec<SeenOut<'a>>>,
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
            seen_count: None,
            seen: None,
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
/// - **쓰기 전에 다 잰다** — 꼴·슬러그·이슈·사람(쓸 때만 — `--dry-run` 은 안 묻는다)·이미 선 파일·`lessons/` 의 자리.
///   그 뒤에 지는 것은 디스크뿐이다
/// - 커밋하지 않는다. 끝 줄이 브리프 7-4 와 같은 커밋 줄([`crate::guide::wiki_commit`])을 댄다
pub fn learn(ctx: &Ctx, args: crate::cli::LearnArgs) -> R<Vec<String>> {
    use crate::wiki::learn;
    let lang = ctx.lang();
    // **`-` 로 여는 제목도 받는다**(moai-xp7l.c4h) — `- not a flag` 같은 교훈을 clap 이 모르는 깃발로 거절했다. `add`
    // 의 제목과 같은 자다: 빈칸 없는 `-x` 는 깃발을 잘못 친 것으로 보고 `--` 뒤에서만 받는다. 훅의 규칙 2 는 이 부름을
    // 자리 인자 `wiki learn` 으로 알아보고, `--` 뒤의 토막은 깃발로 안 읽는다(`hook::flag_words`) — 제목이 `--dry-run`
    // 이어도 쓰는 부름이다.
    super::refuse_if_flag_like(args.title.trim(), super::FlagLike::Title("wiki learn"), lang)?;
    // 꼴은 디스크도 트래커도 안 보고 잰다 — 틀린 부름이 저장소를 열 까닭이 없다. stdin 은 여기서 읽는다(락은 노트를
    // 달 때 `with_write` 안에서만 잡힌다).
    let body = super::add::read_body(args.body)?.unwrap_or_default();
    // 꼴은 이슈를 모른 채 먼저 잰다 — 틀린 본문이 사람을 묻거나 트래커를 읽을 까닭이 없다. **제목도 슬러그보다 먼저
    // 잰다**(리뷰 moai-4s5n.u8x) — 빈 제목에 "ASCII 가 아니다" 를 대고, 그대로 치면 또 거절될 `--slug` 줄을 내밀던 판이 있었다.
    learn::page(&args.title, &body, None)
        .map_err(|why| Fail::coded(crate::view::wiki::learn_refused(lang, &why), code::BAD_INPUT))?;
    let slug = match &args.slug {
        Some(s) if learn::is_slug(s) => s.clone(),
        Some(s) => return Err(Fail::coded(crate::view::wiki::learn_bad_slug(lang, s), code::BAD_INPUT)),
        None => learn::slug_from(&args.title)
            .ok_or_else(|| Fail::coded(crate::view::wiki::learn_no_slug(lang, args.title.trim()), code::BAD_INPUT))?,
    };
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
    // **누군지는 쓸 때만 묻는다**(리뷰 moai-4s5n.u8x) — `--dry-run` 은 아무것도 안 써서 사람을 안 묻는다(`add --from
    // --dry-run` 과 같은 자, CLAUDE.md "누군지 모르면 쓰기는 멈추고, 읽기는 묻지 않는다"). `--issue none` 은 노트가 없다.
    let by = match (args.issue.as_deref(), args.dry_run) {
        (Some("none"), _) | (_, true) => None,
        _ => Some(crate::model::actor(ctx.user.as_deref(), &repo.root).map_err(|e| Fail::no_actor(&e, lang))?),
    };
    // 노트를 달 줄 — `none` 이면 없다. 없는 id 는 페이지를 쓰기 **전에** 거절한다: 오타 하나로 노트 없는 페이지가
    // 남으면 그 거절은 고칠 줄이 아니라 치울 파일을 낸다.
    let target: Option<String> = match args.issue.as_deref() {
        Some("none") => None,
        given => {
            let load = repo.read()?;
            // **말만 하고 부분 실패 깃발은 안 세운다** — 쓰기와 짝을 이루는 읽기다([`super::name_load_errors`], `backlog
            // promote` 와 같은 자). 세우던 판은 남의 못 읽는 줄 하나에 페이지도 노트도 다 쓴 부름을 1 로 끝내, 실패로 읽은
            // 쪽이 다시 불러 `already_exists` 에 막히거나 다른 슬러그로 같은 교훈을 또 썼다(리뷰 moai-4s5n.u8x).
            super::name_load_errors(lang, &repo.issues_path(), &load.errors);
            Some(match given {
                Some(id) if load.issues.iter().any(|i| i.id == id) => id.to_string(),
                // 못 읽는 산 줄과 아카이브된 줄에도 노트를 단다([`super::note::append`], moai-xp7l.82p) — 노트가 락 안에서
                // 묻는 자(`store::Reserved`)와 같은 답이라야, 여기서 받은 id 를 노트가 `not_found` 로 물리지 않는다.
                Some(id)
                    if load.reserved_ids().contains(id) || crate::archive::reserve(&repo.root).ids.contains(id) =>
                {
                    id.to_string()
                }
                Some(id) => return Err(Fail::not_found(id, lang)),
                // **`prime` 이 "쥐었다" 고 대는 그 줄이다**(`report::held_by`) — 고르는 자도, 고르는 판도 같다: 아카이브의
                // 맥락(미룬 에픽·부모)까지 얹은 판이다(`cmd::prime`). 산 줄만 넘기던 판은 아카이브에 든 미룬 에픽 밑에서 다시
                // 연 줄을, prime 은 안 대는데 집은 것으로 셌다(리뷰 moai-4s5n.u8x). 사람을 모르면(미리보기) 가르지 않는다 —
                // `prime` 과 같은 약속이다.
                None => {
                    let me = match &by {
                        Some(a) => Some(crate::query::Me::of(a)),
                        None => super::me_at(ctx, &repo.root),
                    };
                    let all = crate::archive::context(&repo.root, load)?;
                    let held = crate::report::held_by(&all.issues, &repo.config, me.as_ref());
                    match held.as_slice() {
                        [one] => one.id.clone(),
                        many => {
                            let ids: Vec<&str> = many.iter().map(|i| i.id.as_str()).collect();
                            return Err(Fail::coded(crate::view::wiki::learn_which_issue(lang, &ids), code::BAD_INPUT));
                        }
                    }
                }
            })
        }
    };
    let issue = target.as_deref();
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
    // 커밋 줄이 댈 위키 디렉터리 — 체크아웃에서 잰 글자다(`./docs` 는 `docs`, 뿌리면 빈 글).
    let wiki_dir = shown.join("/");
    let note = learn::note(&slug);
    let exists = || Fail::coded(crate::view::wiki::learn_exists(lang, &path, issue, &note), code::ALREADY_EXISTS);
    let lessons = dir.join(learn::LESSONS);
    let at = lessons.join(&file);
    // 링크도 센다 — 끝이 없는 링크도 그 자리는 섰다(`create_new` 도 그것을 안 따르고 진다).
    if std::fs::symlink_metadata(&at).is_ok() {
        return Err(exists());
    }
    // `lessons/` 의 자리를 잰다 — 체크아웃 밖을 가리키는 링크면 안 짓는다(아카이브 디렉터리와 같은 자, `held::place_dir`).
    // **미리보기 앞에서 잰다**(리뷰 moai-4s5n.u8x) — 뒤에서 재던 판은 `--dry-run` 이 "쓸 것이다" 라고 한 자리를 진짜 부름이
    // 거절했다. 재기만 하고 짓지는 않는다.
    let home = crate::held::Home::of(repo.here());
    let lessons = crate::held::place_dir(&lessons, &home)
        .map_err(|u| Fail::coded(crate::held::refused(lang, &lessons, &u), code::BROKEN))?;

    if args.dry_run {
        return said(ctx, &path, &slug, issue, false, &note, &wiki_dir, Some(&text));
    }

    // `lessons/` 를 짓는다. **이 부름이 지은 것은 쓰기가 지면 걷는다** — `remove_dir` 은 빈 디렉터리만 지우니 남의 페이지는
    // 안 다친다. 안 걷던 판은 "남긴 것은 없다" 고 말하면서 빈 `lessons/` 를 남겼다(리뷰 moai-4s5n.u8x).
    let made = std::fs::symlink_metadata(&lessons).is_err();
    std::fs::create_dir_all(&lessons)
        .map_err(|e| Fail::coded(crate::view::wiki::learn_failed(lang, &path, &e.to_string()), code::ERROR))?;
    if let Err(e) = write_new(&lessons.join(&file), &text, repo.here()) {
        if made {
            let _ = std::fs::remove_dir(&lessons);
        }
        return Err(match e.kind() {
            std::io::ErrorKind::AlreadyExists => exists(),
            _ => Fail::coded(crate::view::wiki::learn_failed(lang, &path, &e.to_string()), code::ERROR),
        });
    }

    // 노트 — 페이지는 이미 섰다. 지면 페이지는 남기고, 다시 칠 줄을 내며 비영으로 끝난다. **코드는 `error` 다**(리뷰
    // moai-4s5n.u8x) — 안의 코드(`locked`·`not_found`·`broken`)는 "아무것도 안 바뀌었으니 같은 부름을 다시 쳐도 된다" 는
    // 말인데 페이지는 이미 섰다. 그 코드를 믿고 같은 부름을 다시 친 쪽은 `already_exists` 에 막혀 노트를 영영 못 단다.
    let noted = match (issue, &by) {
        (Some(id), Some(by)) => {
            super::note::append(&repo, lang, id, &note, by).map_err(|e| {
                Fail::coded(crate::view::wiki::learn_unnoted(lang, &path, id, &note, &e.message), code::ERROR)
            })?;
            true
        }
        _ => false,
    };
    said(ctx, &path, &slug, issue, noted, &note, &wiki_dir, None)
}

/// 새 파일에 글을 쓴다 — **있으면 진다.** 노트가 그 페이지를 가리키는데 페이지가 디스크에 없으면 안 되니 `sync` 한다.
///
/// **다 쓴 글만 그 이름에 선다**(moai-xp7l.c4h) — 임시 파일에 다 쓰고 적은 뒤 `hard_link` 로 이름을 세운다. 링크는 이름이
/// 이미 있으면 지니 `create_new` 와 같은 "하나만 이긴다" 다. 이름에 바로 쓰던 판은 쓰는 중에 죽으면(SIGKILL) 빈 페이지가
/// 남아 그 슬러그의 `learn` 을 "이미 있다" 로 영영 막았다.
///
/// **임시 파일은 그 체크아웃(`checkout`)의 `.moai/` 에 짓는다**(리뷰 moai-xp7l.mug) — 커밋되는 파일의 임시 자리와 같은
/// 자다([`crate::store::write_staged`], moai-3akx). 페이지 곁에 짓던 판은 쓰다 죽은 찌꺼기를 위키에 남겨, `learn` 이 내미는
/// `git add -- <위키>` 가 그것을 커밋에 실었다 — `.moai/*.tmp.*` 는 `init` 이 심는 무시 줄이 덮는다. 그 자리가 링크를 다 푼
/// 뒤 체크아웃 밖이거나 `.git` 안이면 안 쓴다([`crate::store::stage_inside`] — `write_staged` 가 고르는 자와 같다). 이름은
/// [`crate::store::tmp_names`] 로 짓고 **있는 파일은 지우지 않는다**(moai-ydm7.976) — pid 가 같은 남의 쓰기가 아직 쓰는
/// 파일일 수 있다. 거기서 못 짓거나 링크를 못 걸면(`.moai/` 가 없다, 다른 파일시스템, 링크를 모르는 파일시스템) 옛 길 —
/// 이름에 바로 쓰고, 지면 걷는다. **"이미 있다" 는 링크가 진 것뿐이다** — 임시 이름이 막힌 것은 페이지가 있다는 말이 아니다.
fn write_new(at: &std::path::Path, text: &str, checkout: &std::path::Path) -> std::io::Result<()> {
    match staged(at, text, checkout) {
        Some(Ok(())) => {}
        Some(Err(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(e),
        // 임시 파일을 못 지었거나 링크를 못 건다 — 이름에 바로 쓴다.
        _ => write_synced(at, text)?,
    }
    // **이름도 디스크에 적는다**(리뷰 moai-4s5n.u8x) — `sync_all` 은 파일의 글만 적고 디렉터리에 선 이름은 안 적는다.
    // `store` 가 새 저널 파일과 갈아끼운 스냅샷에 같은 까닭으로 디렉터리를 적는다. 방금 지었을 수 있는 `lessons/` 의
    // 이름은 그 위 디렉터리에 선다.
    #[cfg(unix)]
    for d in at.ancestors().skip(1).take(2) {
        if let Ok(d) = std::fs::File::open(d) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// 체크아웃의 `.moai/` 에 임시 파일을 지어 글을 다 쓰고 적은 뒤 `at` 에 링크한다 — 임시 파일은 어느 쪽이든 걷는다. 지을
/// 임시 파일이 없었으면 `None`, 지었으면 링크의 답이다. 이름이 막혔으면([`crate::store::tmp_names`] 의 다음 이름) 다음
/// 이름으로 간다.
fn staged(at: &std::path::Path, text: &str, checkout: &std::path::Path) -> Option<std::io::Result<()>> {
    let stage = checkout.join(".moai");
    if !crate::store::stage_inside(&stage, checkout) {
        return None;
    }
    let name = at.file_name()?.to_str()?;
    for tmp in crate::store::tmp_names(name).map(|n| stage.join(n)) {
        match write_synced(&tmp, text) {
            Ok(()) => {
                let linked = std::fs::hard_link(&tmp, at);
                let _ = std::fs::remove_file(&tmp);
                return Some(linked);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// `at` 을 새로 지어(`create_new`) 글을 다 쓰고 적는다. 지은 뒤에 지면 반쯤 쓴 파일을 걷는다 — 못 지었으면 남의 것일
/// 수 있으니 안 건드린다.
fn write_synced(at: &std::path::Path, text: &str) -> std::io::Result<()> {
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

/// 쓴 것(또는 쓸 것)을 낸다 — `dry` 는 `--dry-run` 의 글이고, `dir` 은 체크아웃에서 잰 위키 디렉터리다(뿌리면 빈 글).
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
    // **위키가 체크아웃 뿌리면 `lessons` 를 댄다**(리뷰 moai-4s5n.u8x) — `.` 을 대던 판의 `git add -- .`·`git commit … -- .`
    // 은 체크아웃의 고침을 다 그 `docs(wiki):` 커밋에 실었다.
    let dir = if dir.is_empty() { crate::wiki::learn::LESSONS } else { dir };
    let commit = crate::guide::wiki_commit(&crate::text::shell_word(dir));
    Ok(crate::view::wiki::learned(ctx.lang(), path, issue, note, &commit, dry))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **"이미 있다" 는 이름이 섰을 때뿐이다**(리뷰 moai-xp7l.mug) — 임시 이름이 다 막혀도(남의 것이라 안 지운다) 이름에 바로
    /// 쓰고, 막은 자리는 안 건드린다. 쓴 뒤에는 임시 파일이 안 남고, 같은 이름을 다시 쓰면 그때 진다.
    #[test]
    fn write_new_loses_only_to_the_name_itself() {
        let s = crate::scratch::Scratch::new("wiki-write-new");
        let (stage, lessons) = (s.path().join(".moai"), s.path().join("docs/lessons"));
        std::fs::create_dir_all(&stage).unwrap();
        std::fs::create_dir_all(&lessons).unwrap();
        let a = lessons.join("a.md");
        crate::store::with_tmp_names_taken(&stage, "a.md", || write_new(&a, "# A\n", s.path())).unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "# A\n");
        let b = lessons.join("b.md");
        write_new(&b, "# B\n", s.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "# B\n");
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 0, "임시 파일이 남았다");
        let again = write_new(&b, "# other\n", s.path()).unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "# B\n", "선 페이지를 건드렸다");
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 0, "진 쓰기가 임시 파일을 남겼다");
    }

    /// `.moai` 가 체크아웃 밖을 가리키는 링크면 거기에 임시 파일을 안 짓는다(리뷰 moai-xp7l.mug) — `store::write_staged` 가
    /// 임시 자리를 고르는 자와 같다. 교훈의 글이 체크아웃 밖이나 `.git` 안에 남을 길을 안 연다. 페이지는 이름에 바로 쓴다.
    #[cfg(unix)]
    #[test]
    fn write_new_stages_nowhere_outside_the_checkout() {
        let s = crate::scratch::Scratch::new("wiki-write-new-link");
        let away = crate::scratch::Scratch::new("wiki-write-new-away");
        let lessons = s.path().join("docs/lessons");
        std::fs::create_dir_all(&lessons).unwrap();
        std::os::unix::fs::symlink(away.path(), s.path().join(".moai")).unwrap();
        let c = lessons.join("c.md");
        assert!(staged(&c, "# C\n", s.path()).is_none(), "체크아웃 밖의 `.moai` 에 임시 파일을 지으려 했다");
        assert!(!c.exists());
        write_new(&c, "# C\n", s.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&c).unwrap(), "# C\n");
        assert_eq!(std::fs::read_dir(away.path()).unwrap().count(), 0, "체크아웃 밖에 무엇이 남았다");
        // 체크아웃 안의 `.moai` 는 임시 자리다.
        std::fs::remove_file(s.path().join(".moai")).unwrap();
        std::fs::create_dir(s.path().join(".moai")).unwrap();
        assert!(matches!(staged(&lessons.join("d.md"), "# D\n", s.path()), Some(Ok(()))));
    }
}
