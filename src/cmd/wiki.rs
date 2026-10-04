//! `moai wiki ls`·`moai wiki show` — 위키를 읽는다(moai-ihu4).
//!
//! **여기는 잇기만 한다.** 페이지를 걷고 푸는 것은 `wiki` 가, 사람 화면은 `view::wiki` 가 한다. 위키는 이
//! 체크아웃(`Repo::here`) 밑에서 읽고, 페이지가 댄 id 가 있는지는 트래커(딸린 워크트리면 옮겨 간 루트의 것)에
//! 묻는다 — 위키는 가지를 타는 문서고 트래커는 저장소에 하나다.
//!
//! **종료 코드는 못 읽었을 때만 비영이다.** 위키 디렉터리가 아직 없는 것은 안 쓴 위키라 0 이고(2026-10-04 사용자
//! 결정), 크기 상한을 넘은 페이지는 일부러 안 읽은 것이라 목록에서는 0 이다. 그 밖에 못 연 디렉터리·페이지가
//! 하나라도 있으면 목록은 다 내고 비영으로 끝난다(`note_partial`).

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
    Ok(crate::view::wiki::page(ctx.lang(), page, wiki.uncounted(slug)))
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
    let dir = repo.config.wiki_dir.clone();
    let known = |id: &str| ids.contains(id);
    let wiki = wiki::load(repo.here(), &dir, &repo.config.prefix, &known);
    Ok((dir, wiki))
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
            let skipped = w
                .skipped
                .iter()
                .map(|s| SkippedOut {
                    path: &s.path,
                    kind: s.why.kind(),
                    said: crate::view::wiki::skip_reason(lang, s),
                })
                .collect();
            super::json_line(&Listed { dir, pages, skipped })
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
struct Told {
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
            error: p.error.as_ref().map(|u| Told { kind: u.kind(), said: crate::view::wiki::unread(lang, p, u) }),
            body: None,
        }
    }
}
