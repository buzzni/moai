//! 이슈 하나에 관한 위키 — `moai show <id>` 가 본문 밑에 내는 두 묶음(moai-g8pt.5aw).
//!
//! **순수 함수 하나다.** `&[Page]` 와 `&Issue`(와 부르는 쪽이 이미 읽은 그 줄의 에픽 id)를 받아 답만 낸다 — 파일을
//! 열지 않고 찍지 않는다(CLAUDE.md "`report` 와 `query` 는 순수 함수다"). 위키를 읽는 것은 `cmd::show` 고, 무엇을
//! 낼지는 여기서 가른다 — `cmd/*.rs` 에 이슈의 뜻을 가르는 `if` 를 두지 않는다.
//!
//! - **댄 페이지**(`named_in`) — 본문이 이 id 나 이 줄의 에픽 id 를 댄 페이지. 결정적이고 싸다. 둘 다 댔으면 제 id 로
//!   한 번만 선다 — 더 가까운 까닭이다
//! - **관련 절**(`related`) — 제목을 질의로 [`rank`] 한 절 가운데 흔하지 않은 낱말이 [`MIN_DISTINCT`] 개 맞은 것,
//!   페이지당 하나(같은 페이지의 절 셋이 자리를 다 먹지 않게), 댄 페이지는 빼고 [`RELATED`] 개까지
//!
//! **엉뚱한 절을 내미느니 안 내민다**(2026-10-25 사용자 결정, moai-g8pt.5aw). 첫 판은 제목과 태그를 질의로 두고 맞은
//! 낱말을 다 셌는데, 이 저장소의 열린 이슈 열 개에서 관련 절 여섯 가운데 맞는 것이 하나였다 — 문턱을 넘긴 것이
//! `moai`·`show`·`id` 같은 어디에나 든 낱말과 `enhancement` 같은 태그였다. 그래서 태그는 질의에서 빼고, 문턱은 흔한
//! 낱말([`super::search::COMMON_SHARE`])을 뺀 낱말로만 센다. 질의에 흔하지 않은 낱말이 하나도 없으면 관련 절이 없다.

use super::Page;
use super::search::{Hit, rank};
use crate::model::Issue;
use std::collections::HashSet;

/// 관련 절에 들려면 맞아야 하는 **흔하지 않은 서로 다른 질의 낱말**의 수([`Hit::rare`]). 질의의 흔하지 않은 낱말이
/// 이보다 적으면 그 수다(하나뿐이면 하나). 이 저장소의 열린 이슈 열 개로 잰 값이다 — 잰 수는 moai-g8pt.5aw 의 노트에 있다.
/// 세는 것은 **제목의 원래 낱말**이다([`super::search::Found::words`]) — 한글 낱말이 푼 글자 둘씩의 토큰이 아니다.
pub const MIN_DISTINCT: usize = 2;

/// 관련 절을 몇 개까지 내는가.
pub const RELATED: usize = 3;

/// 이 이슈에 관한 위키.
#[derive(Debug, Clone, PartialEq)]
pub struct About<'a> {
    /// 이 id 나 에픽 id 를 댄 페이지, 위키 목록의 차례로.
    pub named_in: Vec<Named<'a>>,
    /// 관련 절, 순위 차례로 — 페이지당 하나, `named_in` 의 페이지는 없다. 발췌는 없다([`rank`]).
    pub related: Vec<Hit<'a>>,
}

/// 댄 페이지 하나와, 어느 id 로 걸렸는가.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named<'a> {
    pub page: &'a Page,
    pub via: &'a str,
}

/// `issue` 에 관한 위키를 `pages` 에서 고른다. `epic` 은 그 줄이 선 에픽의 id 다(물려받은 것까지, 묶음 줄이면 없다)
/// — 소속은 저장소 전부를 봐야 답하니 부르는 쪽이 읽어 건넨다. **순수 함수다.**
pub fn about<'a>(pages: &'a [Page], issue: &'a Issue, epic: Option<&'a str>) -> About<'a> {
    let names = |p: &Page, id: &str| p.issues.iter().any(|r| r.id == id);
    let named_in: Vec<Named> = pages
        .iter()
        .filter_map(|p| {
            let via = if names(p, &issue.id) { Some(issue.id.as_str()) } else { epic.filter(|e| names(p, e)) };
            via.map(|via| Named { page: p, via })
        })
        .collect();
    // 발췌는 안 짓는다 — 다 받아 페이지당 하나로 거르고 셋에서 자르므로, 버릴 hit 가 거의 전부다.
    let found = rank(pages, &issue.title, usize::MAX);
    let need = (found.words.len() - found.common.len()).min(MIN_DISTINCT);
    let mut hits: Vec<Hit> = found.hits.into_iter().filter(|h| need > 0 && h.rare >= need).collect();
    // 흔하지 않은 낱말이 많이 맞은 절이 앞이다 — 같으면 `rank` 의 차례 그대로다(안정 정렬).
    hits.sort_by_key(|h| std::cmp::Reverse(h.rare));
    let mut seen: HashSet<&str> = named_in.iter().map(|n| n.page.slug.as_str()).collect();
    let related = hits.into_iter().filter(|h| seen.insert(h.page.slug.as_str())).take(RELATED).collect();
    About { named_in, related }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};
    use crate::wiki::{IssueRef, parse};

    fn page(slug: &str, body: &str) -> Page {
        let p = parse(slug, body, "moai");
        Page {
            slug: slug.into(),
            title: p.title.unwrap_or_else(|| slug.into()),
            path: format!("docs/{slug}.md"),
            bytes: body.len() as u64,
            issues: p.ids.into_iter().map(|id| IssueRef { id, exists: true }).collect(),
            links: Vec::new(),
            headings: p.headings,
            linked_from: Vec::new(),
            conflict: false,
            error: None,
            body: Some(body.into()),
        }
    }

    fn issue(id: &str, title: &str, tags: &[&str]) -> Issue {
        let mut i = Issue::new(id.into(), title.into(), Kind::Issue, Status::new("todo"), "2026-10-25T00:00:00Z");
        i.tags = tags.iter().map(|t| t.to_string()).collect();
        i
    }

    /// 질의 낱말이 흔해지지 않게 까는 페이지 — 낱말이 절의 넷에 하나보다 많은 곳에 들면 흔하다(`COMMON_SHARE`).
    fn with_filler(mut pages: Vec<Page>) -> Vec<Page> {
        pages.extend((0..20).map(|n| page(&format!("fill{n}"), &format!("# Fill {n}\n\nunrelated text\n"))));
        pages
    }

    fn slugs<'a>(a: &'a About) -> (Vec<(&'a str, &'a str)>, Vec<&'a str>) {
        (
            a.named_in.iter().map(|n| (n.page.slug.as_str(), n.via)).collect(),
            a.related.iter().map(|h| h.page.slug.as_str()).collect(),
        )
    }

    #[test]
    fn a_page_naming_the_id_or_its_epic_says_which() {
        let pages = [
            page("own", "# Own\n\nSee moai-aaaa.x1y for this.\n"),
            page("epic", "# Epic\n\nThe whole of moai-aaaa.\n"),
            page("both", "# Both\n\nmoai-aaaa and moai-aaaa.x1y.\n"),
            page("none", "# None\n\nNothing here.\n"),
        ];
        let i = issue("moai-aaaa.x1y", "zzz", &[]);
        let a = about(&pages, &i, Some("moai-aaaa"));
        assert_eq!(
            slugs(&a).0,
            [("own", "moai-aaaa.x1y"), ("epic", "moai-aaaa"), ("both", "moai-aaaa.x1y")],
            "제 id 가 에픽 id 보다 가깝다"
        );
        // 에픽이 없으면 에픽 id 를 댄 페이지는 안 걸린다.
        assert_eq!(slugs(&about(&pages, &i, None)).0, [("own", "moai-aaaa.x1y"), ("both", "moai-aaaa.x1y")]);
    }

    #[test]
    fn related_sections_are_one_per_page_and_skip_the_named_pages() {
        let pages = with_filler(vec![
            page(
                "merge",
                "# Merging\n\n## Merge driver install\n\nmerge driver here\n\n## Merge driver path\n\nmerge driver there\n",
            ),
            page("named", "# Named\n\nmoai-bbbb uses the merge driver.\n"),
            page("other", "# Other\n\nThe merge driver once more.\n"),
        ]);
        let i = issue("moai-bbbb", "merge driver", &[]);
        let a = about(&pages, &i, None);
        let (named, related) = slugs(&a);
        assert_eq!(named, [("named", "moai-bbbb")]);
        assert_eq!(related, ["merge", "other"], "페이지당 하나, 댄 페이지는 빠진다");
    }

    #[test]
    fn related_stops_at_three() {
        let pages =
            with_filler((0..5).map(|n| page(&format!("p{n}"), &format!("# P{n}\n\nmerge driver {n}\n"))).collect());
        let i = issue("moai-cccc", "merge driver", &[]);
        let a = about(&pages, &i, None);
        assert_eq!(a.related.len(), RELATED);
    }

    #[test]
    fn below_the_threshold_related_is_empty() {
        let pages =
            with_filler(vec![page("a", "# A\n\nOnly the merge word.\n"), page("b", "# B\n\nOnly the driver word.\n")]);
        // 두 낱말 질의인데 둘 다 든 절이 없다 — 엉뚱한 절을 내밀지 않는다.
        let two = issue("moai-dddd", "merge driver", &[]);
        let a = about(&pages, &two, None);
        assert!(a.related.is_empty() && a.named_in.is_empty(), "{a:?}");
        // 한 낱말 질의면 하나로 넉넉하다.
        let single = issue("moai-dddd", "merge", &[]);
        let one = about(&pages, &single, None);
        assert_eq!(slugs(&one).1, ["a"]);
    }

    #[test]
    fn tags_stay_out_of_the_query() {
        let pages = with_filler(vec![page("a", "# A\n\nThe archive moves rows.\n")]);
        let i = issue("moai-eeee", "archive", &["rows"]);
        let a = about(&pages, &i, None);
        assert_eq!(a.related.first().map(|h| (h.distinct, h.rare)), Some((1, 1)), "태그는 질의 낱말이 아니다");
    }

    /// 작은 위키에서도 관련 절이 선다 — 절 하나에만 든 낱말은 흔하지 않다(리뷰 moai-g8pt.9rd). 몫만 보던 판은 절이
    /// 넷이 안 되면 든 낱말을 모두 흔하다고 보아, 페이지 하나짜리 위키에서 관련 절이 영영 안 섰다.
    #[test]
    fn a_small_wiki_still_has_related_sections() {
        let pages = [page("readme", "# Readme\n\nThe merge driver lives here.\n")];
        let i = issue("moai-gggg", "merge driver", &[]);
        assert_eq!(slugs(&about(&pages, &i, None)).1, ["readme"]);
    }

    /// 문턱은 제목의 원래 낱말로 센다 — `드라이버` 하나만 든 절은 `드라이버 설치` 의 낱말 둘 문턱을 못 넘는다. 토큰으로
    /// 세던 판은 `드라`·`라이`·`이버` 셋으로 넘었다(리뷰 moai-g8pt.9rd).
    #[test]
    fn one_korean_word_does_not_pass_a_two_word_threshold() {
        let pages =
            with_filler(vec![page("a", "# 가\n\n드라이버만 있다\n"), page("b", "# 나\n\n드라이버 설치 방법\n")]);
        let i = issue("moai-hhhh", "드라이버 설치", &[]);
        assert_eq!(slugs(&about(&pages, &i, None)).1, ["b"]);
        let alone = issue("moai-hhhh", "드라이버", &[]);
        let a = about(&pages, &alone, None);
        assert!(a.related.iter().all(|h| h.rare == 1), "{a:?}");
    }

    /// 어디에나 든 낱말은 문턱을 못 채운다 — `moai`·`show` 둘이 맞은 절은 안 내밀고, 흔하지 않은 낱말이 맞은 절만
    /// 낸다. 질의가 흔한 낱말뿐이면 관련 절이 없다.
    #[test]
    fn common_words_do_not_count_toward_the_threshold() {
        let mut pages: Vec<Page> =
            (0..8).map(|n| page(&format!("p{n}"), &format!("# P{n}\n\nmoai show runs here {n}\n"))).collect();
        pages.push(page("hit", "# Hit\n\nmoai show draws the gantt chart\n"));
        let pages = with_filler(pages);
        let i = issue("moai-ffff", "moai show 가 gantt chart 를 그린다", &[]);
        let a = about(&pages, &i, None);
        assert_eq!(slugs(&a).1, ["hit"], "흔한 두 낱말만 맞은 절이 섰다 — {a:?}");
        assert_eq!(a.related[0].rare, 2);
        let only = issue("moai-ffff", "moai show", &[]);
        assert!(about(&pages, &only, None).related.is_empty(), "흔한 낱말뿐인 질의에 절을 냈다");
    }
}
