//! 위키 검색 — 페이지를 절로 쪼개 질의에 순위를 매긴다(moai-g8pt.ara).
//!
//! **순수 함수 하나다.** `&[Page]` 와 질의를 받아 답만 낸다 — 파일을 열지 않고, 찍지 않고, 점수표를 저장하지
//! 않는다(CLAUDE.md "`report` 와 `query` 는 순수 함수다", "파생값은 저장하지 않는다"). `wiki search` 와
//! `show <id>` 와 나중의 TUI·web 이 같은 자를 쓴다.
//!
//! - **절**은 [`Page::headings`] 의 줄에서 자른 조각이다 — 모든 단의 머리글이다. 첫 머리글 앞의 글은 페이지 제목의
//!   절이다. 앵커는 `Page::headings` 의 것을 그대로 쓴다. 다시 짓지 않는다 — 짓는 자는 [`super::Anchors`] 하나다
//! - **토큰**은 소문자로 접어 글자·숫자·`_` 아닌 곳에서 가른 낱말이다(`merge-driver` → `merge`, `driver`;
//!   `archive_days` 는 하나). 한글·한자·가나는 글자 둘씩도 낸다 — 조사가 붙은 `검색은` 이 `검색` 과 맞으려면 든다
//! - **점수**는 BM25 다(`k1 = 1.2`, `b = 0.75`). 절의 필드는 둘 — 머리글(그 절의 머리글 + 페이지 제목)과 본문 —
//!   이고 머리글의 낱말은 무게 3 으로 센다(BM25F 를 얕게). 질의 낱말은 OR 지만 **맞은 서로 다른 낱말 수를 먼저**
//!   견주고 점수를 그다음에 견준다. 같으면 슬러그, 그다음 줄 차례다 — 차례는 결정적이다
//! - 어간 추출·불용어·질의 문법은 없다. 흔한 낱말은 idf 가 누른다
//!
//! 본문을 못 읽은 페이지(`Page::body` 없음)는 제목만으로 맞는다 — 빠뜨리지 않는다. 그 hit 는 발췌가 없다.

use super::{Page, unified, unmarked};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const K1: f64 = 1.2;
const B: f64 = 0.75;
/// 머리글 필드의 낱말 하나가 본문 몇 번만큼 무거운가.
const HEADING_WEIGHT: f64 = 3.0;
/// 발췌 한 줄의 최대 표시 폭(칸).
const SNIPPET_COLS: usize = 160;
/// 잘린 발췌에서 첫 맞은 자리 앞에 남기는 표시 폭 — 맞은 자리가 왼쪽 끝에 붙지 않게.
const LEAD_COLS: usize = 40;

/// 찾은 결과.
#[derive(Debug, Clone, PartialEq)]
pub struct Found<'a> {
    /// 질의를 토큰으로 푼 것, 겹침 없이 처음 나온 차례로. 비었으면 질의가 비었던 것이다 — 부르는 쪽이 가린다.
    pub terms: Vec<String>,
    /// 순위 차례의 hit — 맞은 서로 다른 낱말 수, 점수, 슬러그, 줄 차례로 정렬하고 `limit` 에서 자른 것.
    pub hits: Vec<Hit<'a>>,
}

/// 절 하나의 hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit<'a> {
    pub page: &'a Page,
    /// 절의 머리글 글. 첫 머리글 앞의 절(페이지 제목의 절)이면 없다.
    pub heading: Option<String>,
    /// 절의 앵커 — [`Page::headings`] 의 것 그대로. `heading` 이 없으면 없다.
    pub anchor: Option<&'a str>,
    /// 절이 시작하는 줄(**1 부터** — `path:line` 이 가리키는 줄). 머리글 줄이고, 첫 머리글 앞의 절이면 1 이다.
    pub line: usize,
    /// 절에서 맞은 서로 다른 질의 낱말의 수. 점수보다 먼저 견준다.
    pub distinct: usize,
    pub score: f64,
    /// 발췌 한 줄. 본문을 못 읽은 페이지와, 보일 줄이 없는 절이면 없다.
    pub snippet: Option<Snippet>,
}

/// 발췌 한 줄 — 마크다운 기호를 걷지 않은 원문 그대로다(`path:line` 과 맞는다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// 본문에서의 줄(1 부터).
    pub line: usize,
    /// 앞뒤 빈칸을 걷고 표시 폭 160 칸에 맞춘 글. 잘렸으면 끝에 `…` 가 붙는다.
    pub text: String,
    /// `text` 안에서 맞은 자리의 바이트 범위들(겹치면 합치고, 차례대로) — 사람 화면이 굵게 칠할 곳이다.
    pub marks: Vec<Range<usize>>,
}

/// 낱말 하나와 그것이 선 자리(원문 바이트 범위).
struct Tok {
    text: String,
    at: Range<usize>,
}

/// 한글·한자·가나 — 낱말 사이를 안 가르는 글에서 글자 둘씩 낸다.
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF // 한글
        | 0x3040..=0x30FF                                    // 가나
        | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F) // 한자
}

/// 글을 토큰으로 푼다. 낱말은 글자·숫자·`_` 가 이어진 것이고, 그 안의 한글·한자·가나 덩어리는 글자 둘씩(길이
/// 1 이면 그 글자 하나) 낸다. 소문자로 접되 범위는 원문의 것이다.
fn tokens(s: &str) -> Vec<Tok> {
    // 0: 낱말 밖, 1: 한글·한자·가나, 2: 그 밖의 낱말 글자.
    let class = |c: char| {
        if is_cjk(c) {
            1
        } else if c.is_alphanumeric() || c == '_' {
            2
        } else {
            0
        }
    };
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let end_of = |i: usize| chars.get(i + 1).map_or(s.len(), |&(b, _)| b);
    let fold = |r: Range<usize>| s[r].to_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let k = class(chars[i].1);
        let mut j = i;
        while j + 1 < chars.len() && class(chars[j + 1].1) == k {
            j += 1;
        }
        let run = chars[i].0..end_of(j);
        match k {
            0 => {}
            2 => out.push(Tok { text: fold(run.clone()), at: run }),
            _ if i == j => out.push(Tok { text: fold(run.clone()), at: run }),
            _ => {
                for w in i..j {
                    let at = chars[w].0..end_of(w + 1);
                    out.push(Tok { text: fold(at.clone()), at });
                }
            }
        }
        i = j + 1;
    }
    out
}

/// 머리글 줄의 글 — `#` 와 닫는 `#` 를 걷는다(밑줄 꼴이면 그 줄 그대로다).
fn heading_text(line: &str) -> String {
    line.trim().trim_start_matches('#').trim_end_matches('#').trim().to_string()
}

/// 절 하나 — 검색 중에만 산다.
struct Section<'a> {
    page: &'a Page,
    heading: Option<String>,
    anchor: Option<&'a str>,
    /// 절의 첫 줄(0 부터)과 그 줄들. 본문을 못 읽은 페이지의 절은 줄이 없다.
    start: usize,
    lines: Vec<String>,
    /// 머리글 필드와 본문 필드에서 질의 낱말마다 센 수, 그리고 필드의 길이.
    head: HashMap<String, f64>,
    body: HashMap<String, f64>,
    head_len: f64,
    body_len: f64,
}

/// 페이지를 절로 자른다. 첫 머리글 앞의 글이 있거나(또는 머리글이 아예 없으면) 페이지 제목의 절이 먼저 선다.
fn sections<'a>(page: &'a Page) -> Vec<Section<'a>> {
    let mk = |heading: Option<String>, anchor: Option<&'a str>, start: usize, lines: Vec<String>| Section {
        page,
        heading,
        anchor,
        start,
        lines,
        head: HashMap::new(),
        body: HashMap::new(),
        head_len: 0.0,
        body_len: 0.0,
    };
    let Some(body) = &page.body else { return vec![mk(None, None, 0, Vec::new())] };
    let text = unified(unmarked(body));
    let all: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let first = page.headings.first().map_or(all.len(), |h| h.line.min(all.len()));
    if first > 0 || page.headings.is_empty() {
        out.push(mk(None, None, 0, all[..first].iter().map(|l| l.to_string()).collect()));
    }
    for (n, h) in page.headings.iter().enumerate() {
        let from = h.line.min(all.len());
        let to = page.headings.get(n + 1).map_or(all.len(), |x| x.line.clamp(from, all.len()));
        let lines: Vec<String> = all[from..to].iter().map(|l| l.to_string()).collect();
        let text = lines.first().map(|l| heading_text(l)).unwrap_or_default();
        out.push(mk(Some(text), Some(h.anchor.as_str()), from, lines));
    }
    out
}

/// 질의 `query` 로 `pages` 의 절을 찾아 순위대로 `limit` 개까지 낸다. **순수 함수다.**
pub fn search<'a>(pages: &'a [Page], query: &str, limit: usize) -> Found<'a> {
    let mut terms: Vec<String> = Vec::new();
    for t in tokens(query) {
        if !terms.contains(&t.text) {
            terms.push(t.text);
        }
    }
    if terms.is_empty() {
        return Found { terms, hits: Vec::new() };
    }
    let want: HashSet<&str> = terms.iter().map(String::as_str).collect();
    let mut secs: Vec<Section> = pages.iter().flat_map(sections).collect();
    for s in &mut secs {
        // 머리글 필드는 이 절의 머리글과 페이지 제목이다. 머리글 줄은 본문 필드에서 뺀다 — 두 번 세지 않는다.
        let mut head_text = s.page.title.clone();
        if let Some(h) = &s.heading {
            head_text.push('\n');
            head_text.push_str(h);
        }
        let skip = usize::from(s.heading.is_some());
        let body_text = s.lines.iter().skip(skip).map(String::as_str).collect::<Vec<_>>().join("\n");
        for (text, field, len) in [(head_text, &mut s.head, &mut s.head_len), (body_text, &mut s.body, &mut s.body_len)]
        {
            for t in tokens(&text) {
                *len += 1.0;
                if want.contains(t.text.as_str()) {
                    *field.entry(t.text).or_insert(0.0) += 1.0;
                }
            }
        }
    }
    let weighted = |s: &Section, t: &str| {
        HEADING_WEIGHT * s.head.get(t).copied().unwrap_or(0.0) + s.body.get(t).copied().unwrap_or(0.0)
    };
    let n = secs.len() as f64;
    let avg = secs.iter().map(|s| HEADING_WEIGHT * s.head_len + s.body_len).sum::<f64>() / n.max(1.0);
    let idf: Vec<f64> = terms
        .iter()
        .map(|t| {
            let df = secs.iter().filter(|s| weighted(s, t) > 0.0).count() as f64;
            (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
        })
        .collect();
    let mut scored: Vec<(usize, usize, f64)> = Vec::new();
    for (i, s) in secs.iter().enumerate() {
        let len = HEADING_WEIGHT * s.head_len + s.body_len;
        let (mut distinct, mut score) = (0, 0.0);
        for (t, idf) in terms.iter().zip(&idf) {
            let tf = weighted(s, t);
            if tf > 0.0 {
                distinct += 1;
                score += idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / avg.max(f64::MIN_POSITIVE)));
            }
        }
        if distinct > 0 {
            scored.push((i, distinct, score));
        }
    }
    scored.sort_by(|a, b| {
        let (x, y) = (&secs[a.0], &secs[b.0]);
        b.1.cmp(&a.1)
            .then_with(|| b.2.total_cmp(&a.2))
            .then_with(|| x.page.slug.cmp(&y.page.slug))
            .then_with(|| x.start.cmp(&y.start))
    });
    let hits = scored
        .into_iter()
        .take(limit)
        .map(|(i, distinct, score)| {
            let s = &secs[i];
            Hit {
                page: s.page,
                heading: s.heading.clone(),
                anchor: s.anchor,
                line: s.start + 1,
                distinct,
                score,
                snippet: snippet(s, &want),
            }
        })
        .collect();
    Found { terms, hits }
}

/// 절에서 서로 다른 질의 낱말이 가장 많이 든 줄 하나 — 같으면 앞 줄. 맞은 줄이 없으면(제목으로만 맞았다) 절의 첫
/// 빈 줄 아닌 줄이고, 보일 줄이 없으면 없다.
fn snippet(s: &Section, want: &HashSet<&str>) -> Option<Snippet> {
    let mut best: Option<(usize, usize)> = None; // (줄, 서로 다른 낱말 수)
    for (n, l) in s.lines.iter().enumerate() {
        let hit: HashSet<String> =
            tokens(l).into_iter().map(|t| t.text).filter(|t| want.contains(t.as_str())).collect();
        if !hit.is_empty() && best.is_none_or(|(_, d)| hit.len() > d) {
            best = Some((n, hit.len()));
        }
    }
    let n = match best {
        Some((n, _)) => n,
        None => s.lines.iter().position(|l| !l.trim().is_empty())?,
    };
    let (text, marks) = fit(&s.lines[n], want);
    Some(Snippet { line: s.start + n + 1, text, marks })
}

/// 한 줄을 발췌로 다듬는다 — 앞뒤 빈칸을 걷고, 표시 폭 160 칸을 넘으면 첫 맞은 자리 둘레로 자르고 `…` 를 붙인다.
/// grapheme 을 가르지 않는다. 마크는 다듬은 글 안의 바이트 범위다.
fn fit(line: &str, want: &HashSet<&str>) -> (String, Vec<Range<usize>>) {
    let line = line.trim();
    let mut marks: Vec<Range<usize>> =
        tokens(line).into_iter().filter(|t| want.contains(t.text.as_str())).map(|t| t.at).collect();
    marks.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for r in marks {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    if line.width() <= SNIPPET_COLS {
        return (line.to_string(), merged);
    }
    let gs: Vec<(usize, &str)> = line.grapheme_indices(true).collect();
    let w: Vec<usize> = gs.iter().map(|g| g.1.width()).collect();
    let first = merged.first().map_or(0, |r| r.start);
    let at = gs.iter().rposition(|g| g.0 <= first).unwrap_or(0);
    // 앞뒤의 `…` 에 두 칸을 남긴다.
    let budget = SNIPPET_COLS - 2;
    let (mut lo, mut hi, mut used) = (at, at + 1, w[at]);
    while lo > 0 && used + w[lo - 1] <= budget && w[lo..at].iter().sum::<usize>() < LEAD_COLS {
        lo -= 1;
        used += w[lo];
    }
    while hi < gs.len() && used + w[hi] <= budget {
        used += w[hi];
        hi += 1;
    }
    // 오른쪽 끝에 닿았으면 남은 폭을 왼쪽에 쓴다.
    while hi == gs.len() && lo > 0 && used + w[lo - 1] <= budget {
        lo -= 1;
        used += w[lo];
    }
    let from = gs[lo].0;
    let to = gs.get(hi).map_or(line.len(), |g| g.0);
    let (head, tail) = (if lo > 0 { "…" } else { "" }, if hi < gs.len() { "…" } else { "" });
    let text = format!("{head}{}{tail}", &line[from..to]);
    let marks = merged
        .into_iter()
        .filter_map(|r| {
            let (a, b) = (r.start.max(from), r.end.min(to));
            (a < b).then(|| a - from + head.len()..b - from + head.len())
        })
        .collect();
    (text, marks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::parse;

    /// 본문을 읽은 페이지 — 제목·머리글·앵커는 [`parse`] 가 짓는 그대로다.
    fn page(slug: &str, body: &str) -> Page {
        let p = parse(slug, body, "moai");
        Page {
            slug: slug.into(),
            title: p.title.unwrap_or_else(|| slug.into()),
            path: format!("docs/{slug}.md"),
            bytes: body.len() as u64,
            issues: Vec::new(),
            links: Vec::new(),
            headings: p.headings,
            linked_from: Vec::new(),
            conflict: false,
            error: None,
            body: Some(body.into()),
        }
    }

    fn unread(slug: &str, title: &str) -> Page {
        Page { title: title.into(), body: None, headings: Vec::new(), ..page(slug, "") }
    }

    fn top<'a>(pages: &'a [Page], q: &str) -> Vec<(&'a str, Option<String>)> {
        search(pages, q, 50).hits.into_iter().map(|h| (h.page.slug.as_str(), h.heading)).collect()
    }

    #[test]
    fn terms_are_the_query_unfolded_and_empty_when_it_is_blank() {
        let pages = [page("a", "# A\n\nhello\n")];
        assert_eq!(search(&pages, "Merge-Driver archive_days merge", 5).terms, ["merge", "driver", "archive_days"]);
        let blank = search(&pages, "  ,. ", 5);
        assert!(blank.terms.is_empty() && blank.hits.is_empty());
        assert!(search(&pages, "", 5).hits.is_empty());
    }

    #[test]
    fn one_word_two_words_and_a_word_that_is_nowhere() {
        let pages = [
            page("a", "# Alpha\n\n## Install\n\nrun the driver\n\n## Other\n\nnothing here\n"),
            page("b", "# Beta\n\nmerge driver notes\n"),
        ];
        assert_eq!(top(&pages, "install"), [("a", Some("Install".into()))]);
        let two = top(&pages, "merge driver");
        assert_eq!(two[0], ("b", Some("Beta".into())), "두 낱말이 다 든 절이 앞이다");
        assert_eq!(two[1], ("a", Some("Install".into())));
        assert_eq!(two.len(), 2);
        assert!(search(&pages, "zzzz", 5).hits.is_empty());
        let one = &search(&pages, "install", 5).hits[0];
        assert_eq!((one.line, one.anchor, one.distinct), (3, Some("install"), 1));
        let s = one.snippet.as_ref().unwrap();
        assert_eq!((s.line, s.text.as_str()), (3, "## Install"), "맞은 줄이 머리글 줄이다");
    }

    #[test]
    fn code_fences_are_searched_and_marked_in_place() {
        let pages = [page("a", "# A\n\n```sh\n  moai merge-driver --install\n```\n")];
        let h = &search(&pages, "merge-driver install", 5).hits[0];
        let s = h.snippet.as_ref().unwrap();
        assert_eq!((s.line, s.text.as_str()), (4, "moai merge-driver --install"));
        let marked: Vec<&str> = s.marks.iter().map(|r| &s.text[r.clone()]).collect();
        assert_eq!(marked, ["merge", "driver", "install"], "`-` 로 갈린 낱말은 각자 칠한다");
    }

    #[test]
    fn a_korean_word_with_a_particle_matches_by_bigrams() {
        let pages = [page("w", "# 위키\n\n검색은 절 단위로 한다\n"), page("x", "# 다른\n\n전혀 상관없는 글\n")];
        assert_eq!(top(&pages, "검색"), [("w", Some("위키".into()))]);
        assert_eq!(top(&pages, "검색은"), [("w", Some("위키".into()))]);
        assert_eq!(top(&pages, "절"), [("w", Some("위키".into()))], "길이 1 인 낱말은 그 글자 하나다");
        let s = search(&pages, "검색", 5).hits[0].snippet.clone().unwrap();
        assert_eq!(&s.text[s.marks[0].clone()], "검색");
    }

    #[test]
    fn a_word_in_the_heading_beats_one_in_the_body() {
        let pages = [
            page("body", "# Notes\n\n## Misc\n\nthe zebra is here\n"),
            page("head", "# Notes\n\n## Zebra\n\nnothing in this section\n"),
        ];
        let r = top(&pages, "zebra");
        assert_eq!(r[0], ("head", Some("Zebra".into())));
        assert_eq!(r[1], ("body", Some("Misc".into())));
    }

    #[test]
    fn a_short_section_with_both_words_beats_a_long_one_with_one_word_many_times() {
        let long = format!("# L\n\n{}\n", "alpha ".repeat(40));
        let pages = [page("long", &long), page("short", "# S\n\nalpha beta\n")];
        let r = search(&pages, "alpha beta", 5);
        assert_eq!(r.hits[0].page.slug, "short");
        // 점수만으로는 긴 쪽이 이길 수 있는 꼴이어야 시험이 뜻이 있다 — 낱말 수가 점수보다 먼저다.
        assert_eq!((r.hits[0].distinct, r.hits[1].distinct), (2, 1));
    }

    #[test]
    fn a_repeated_heading_gets_its_anchor_from_the_page_headings() {
        let pages = [page("a", "# A\n\n## Setup\n\nfirst\n\n## Setup\n\nsecond\n")];
        let r = search(&pages, "setup", 5);
        let anchors: Vec<_> = r.hits.iter().map(|h| (h.anchor, h.line)).collect();
        assert!(anchors.contains(&(Some("setup"), 3)) && anchors.contains(&(Some("setup-1"), 7)), "{anchors:?}");
    }

    #[test]
    fn a_page_without_a_body_matches_by_title_alone_with_no_snippet() {
        let pages = [unread("big", "Giant manual"), page("a", "# A\n\ntext\n")];
        let r = search(&pages, "manual", 5);
        assert_eq!(r.hits.len(), 1);
        let h = &r.hits[0];
        assert_eq!((h.page.slug.as_str(), h.heading.as_deref(), h.anchor, h.line), ("big", None, None, 1));
        assert!(h.snippet.is_none());
    }

    #[test]
    fn limit_cuts_and_equal_scores_keep_a_fixed_order() {
        let pages: Vec<Page> = ["c", "a", "b"].iter().map(|s| page(s, "# T\n\n## Same\n\nsame words\n")).collect();
        let all = search(&pages, "same", 10);
        let by_page: Vec<_> = all.hits.iter().map(|h| (h.page.slug.as_str(), h.line)).collect();
        let mut sorted = by_page.clone();
        sorted.sort();
        assert_eq!(by_page, sorted, "점수가 같으면 슬러그, 그다음 줄 차례다");
        assert_eq!(search(&pages, "same", 2).hits.len(), 2);
        assert!(search(&pages, "same", 0).hits.is_empty());
        assert_eq!(search(&pages, "same", 2), search(&pages, "same", 2), "부를 때마다 같다");
    }

    #[test]
    fn a_long_line_is_cut_around_the_first_hit_without_splitting_a_grapheme() {
        let line = format!("{} needle {}", "가나다라 ".repeat(60), "e\u{301}".repeat(120));
        let pages = [page("a", &format!("# A\n\n{line}\n"))];
        let s = search(&pages, "needle", 5).hits[0].snippet.clone().unwrap();
        assert!(s.text.starts_with('…') && s.text.ends_with('…'), "{}", s.text);
        assert!(s.text.width() <= SNIPPET_COLS, "{}", s.text.width());
        assert_eq!(&s.text[s.marks[0].clone()], "needle");
        assert!(s.text.trim_end_matches('…').chars().last() != Some('e'), "결합 부호를 떼지 않는다");
    }

    #[test]
    fn a_short_line_is_trimmed_and_kept_whole() {
        let pages = [page("a", "# A\n\n    indented code word\n")];
        let s = search(&pages, "word", 5).hits[0].snippet.clone().unwrap();
        assert_eq!((s.text.as_str(), s.line), ("indented code word", 3));
    }

    #[test]
    fn the_snippet_line_is_the_one_with_the_most_distinct_terms() {
        let pages = [page("a", "# A\n\nalpha alpha alpha\nalpha beta\nbeta\n")];
        let s = search(&pages, "alpha beta", 5).hits[0].snippet.clone().unwrap();
        assert_eq!((s.line, s.text.as_str()), (4, "alpha beta"));
    }
}
