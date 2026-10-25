//! 위키 검색 — 페이지를 절로 쪼개 질의에 순위를 매긴다(moai-g8pt.ara).
//!
//! **순수 함수 하나다.** `&[Page]` 와 질의를 받아 답만 낸다 — 파일을 열지 않고, 찍지 않고, 점수표를 저장하지
//! 않는다(CLAUDE.md "`report` 와 `query` 는 순수 함수다", "파생값은 저장하지 않는다"). `wiki search` 와
//! `show <id>` 와 나중의 TUI·web 이 같은 자를 쓴다.
//!
//! - **절**은 [`Page::headings`] 의 줄에서 자른 조각이다 — 모든 단의 머리글이다. 첫 머리글 앞의 글은 페이지 제목의
//!   절이다(빈 줄뿐이면 안 세운다 — 세우면 제목 낱말만으로 맞는 빈 hit 가 선다). 앵커는 `Page::headings` 의 것을 그대로
//!   쓴다. 다시 짓지 않는다 — 짓는 자는 [`super::Anchors`] 하나다
//! - **토큰**은 소문자로 접어 글자·숫자·`_` 아닌 곳에서 가른 낱말이다(`merge-driver` → `merge`, `driver`;
//!   `archive_days` 는 하나). 한글·한자·가나는 글자 둘씩도 낸다 — 조사가 붙은 `검색은` 이 `검색` 과 맞으려면 든다
//! - **점수**는 BM25 다(`k1 = 1.2`, `b = 0.75`). 절의 필드는 둘 — 머리글(그 절의 머리글 + 페이지 제목, 둘이 같은
//!   낱말이면 한 번)과 본문 — 이고 머리글의 낱말은 무게 3 으로 센다(BM25F 를 얕게). 질의 낱말은 OR 지만 **맞은 서로
//!   다른 낱말 수를 먼저** 견주고 점수를 그다음에 견준다. 같으면 슬러그, 그다음 줄 차례다 — 차례는 결정적이다
//! - **낱말**은 질의의 원래 낱말이다 — 점수는 토큰으로 세지만, 맞은 낱말의 수([`Hit::distinct`]·[`Hit::rare`])는
//!   낱말로 센다. 한글·한자·가나 낱말은 그 토큰의 절반 이상이 든 절에서 맞은 것이고, 낱말 하나 맞지 않은 절은 hit 가
//!   아니다
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
/// 흔한 낱말의 문턱 — 절의 이 몫보다 많은 곳에 든 낱말은 흔하다([`Found::common`]). 이 저장소 docs/ 의 절
/// 187 개에서 `moai` 는 84%, `id` 는 43%, `show` 는 31% 에 들고 `wiki` 는 11%, `merge` 는 17% 다(moai-g8pt.5aw).
/// 낱말 목록이 아니라 몫으로 두는 까닭은 위키마다 흔한 낱말이 달라서다 — 이 저장소의 `moai` 가 남의 위키에서는
/// 그 제품 이름이다. **절 하나에만 든 낱말은 흔하지 않다** — 몫만 보면 절이 넷이 안 되는 작은 위키에서는 든 낱말이
/// 모두 흔해져 관련 절이 영영 안 선다(리뷰 moai-g8pt.9rd).
pub const COMMON_SHARE: f64 = 0.25;
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
    /// 점수는 이것으로 센다.
    pub terms: Vec<String>,
    /// 질의의 **원래 낱말**(접은 글), 겹침 없이 처음 나온 차례로 — 한글·한자·가나 덩어리 하나는 토큰이 여럿이어도
    /// 낱말 하나다. [`Hit::distinct`]·[`Hit::rare`] 는 이것을 센다.
    pub words: Vec<String>,
    /// `words` 가운데 흔한 낱말 — 절의 [`COMMON_SHARE`] 보다 많은(그리고 둘 이상의) 곳에서 맞은 것, `words` 의 차례로.
    pub common: Vec<String>,
    /// 순위 차례의 hit — 맞은 서로 다른 낱말 수, 점수, 슬러그, 줄 차례로 정렬하고 `limit` 에서 자른 것.
    pub hits: Vec<Hit<'a>>,
    /// 맞은 절의 수 — `limit` 이 자르기 **전**이다. 부르는 쪽이 "몇 중 몇" 을 대려고 다 받아 자르면 버릴 hit 의
    /// 발췌까지 짓는다.
    pub total: usize,
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
    /// 절에서 맞은 서로 다른 질의 낱말([`Found::words`])의 수. 점수보다 먼저 견준다. 한글·한자·가나 낱말은 그
    /// 토큰(글자 둘씩)의 절반 이상이 맞아야 맞은 것이다 — 토큰 하나짜리면 그 하나다. 0 인 절은 hit 가 아니다.
    pub distinct: usize,
    /// `distinct` 가운데 흔하지 않은 낱말의 수([`Found::common`] 밖). 차례에는 안 쓴다 — 관련 절의 문턱이 쓴다.
    pub rare: usize,
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

/// 0: 낱말 밖, 1: 한글·한자·가나, 2: 그 밖의 낱말 글자.
fn class(c: char) -> u8 {
    if is_cjk(c) {
        1
    } else if c.is_alphanumeric() || c == '_' {
        2
    } else {
        0
    }
}

/// 낱말을 소문자로 접어 `buf` 에 둔다. ASCII 면 제자리에서 접는다 — 흔한 길에서 낱말마다 String 을 짓지 않는다.
fn fold(w: &str, buf: &mut String) {
    buf.clear();
    if w.is_ascii() {
        buf.push_str(w);
        buf.make_ascii_lowercase();
    } else {
        buf.push_str(&w.to_lowercase());
    }
}

/// 글의 토큰을 차례로 `f(접은 글, 원문 범위)` 에 건넨다. 낱말은 글자·숫자·`_` 가 이어진 것이고, 그 안의
/// 한글·한자·가나 덩어리는 글자 둘씩(길이 1 이면 그 글자 하나) 낸다. 접은 글은 버퍼 하나를 돌려 쓴다 — 절을 세는
/// 걸음이 위키 전부를 지나므로, 글자 배열과 낱말마다의 String 을 짓던 판은 그 값을 낱말 수만큼 치렀다.
/// 한글·한자·가나에는 대소문자가 없어 접지 않는다.
fn each_token(s: &str, buf: &mut String, mut f: impl FnMut(&str, Range<usize>)) {
    let mut it = s.char_indices().peekable();
    while let Some((start, c)) = it.next() {
        match class(c) {
            0 => {}
            2 => {
                let mut end = start + c.len_utf8();
                while let Some(&(b, d)) = it.peek().filter(|&&(_, d)| class(d) == 2) {
                    end = b + d.len_utf8();
                    it.next();
                }
                fold(&s[start..end], buf);
                f(buf, start..end);
            }
            _ => {
                let mut prev = start;
                let mut single = true;
                while let Some(&(b, d)) = it.peek().filter(|&&(_, d)| class(d) == 1) {
                    let end = b + d.len_utf8();
                    buf.clear();
                    buf.push_str(&s[prev..end]);
                    f(buf, prev..end);
                    single = false;
                    prev = b;
                    it.next();
                }
                if single {
                    let end = start + c.len_utf8();
                    buf.clear();
                    buf.push_str(&s[start..end]);
                    f(buf, start..end);
                }
            }
        }
    }
}

/// 글을 토큰으로 푼다([`each_token`] 을 모은 것). 소문자로 접되 범위는 원문의 것이다.
fn tokens(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    each_token(s, &mut String::new(), |t, at| out.push(Tok { text: t.to_string(), at }));
    out
}

/// 두 글이 같은 토큰을 같은 차례로 내는가 — 머리글과 페이지 제목을 마크다운 기호와 상관없이 견준다. 제목은
/// 기호를 걷은 글이고(`# The \`moai\` CLI` → `The moai CLI`) 머리글은 원문이라 글자째로는 못 견준다.
pub(crate) fn same_words(a: &str, b: &str) -> bool {
    tokens(a).into_iter().map(|t| t.text).eq(tokens(b).into_iter().map(|t| t.text))
}

/// 머리글 줄의 글 — 인용의 `>`, 여는 `#` 와 닫는 `#` 를 걷는다(밑줄 꼴이면 그 줄 그대로다). 닫는 `#` 는
/// CommonMark 처럼 앞에 빈칸이 선 것만이다 — `## C#` 의 `#` 는 글이다.
fn heading_text(line: &str) -> String {
    let t = line.trim().trim_start_matches(|c: char| c == '>' || c.is_whitespace());
    let Some(rest) = t.strip_prefix('#') else { return t.trim().to_string() };
    let rest = rest.trim_start_matches('#').trim_end();
    let bare = rest.trim_end_matches('#');
    let rest = if bare.is_empty() || bare.ends_with([' ', '\t']) { bare } else { rest };
    rest.trim().to_string()
}

/// 절 하나 — 검색 중에만 산다. 줄은 페이지 글을 빌린다(`'t`) — 절마다 줄을 베끼지 않는다.
struct Section<'a, 't> {
    page: &'a Page,
    heading: Option<String>,
    anchor: Option<&'a str>,
    /// 절의 첫 줄(0 부터)과 그 줄들. 본문을 못 읽은 페이지의 절은 줄이 없다.
    start: usize,
    lines: &'t [&'t str],
    /// 머리글 필드와 본문 필드에서 질의 낱말마다 센 수(`terms` 의 차례), 그리고 필드의 길이.
    head: Vec<f64>,
    body: Vec<f64>,
    head_len: f64,
    body_len: f64,
}

/// 페이지를 절로 자른다. 첫 머리글 앞의 글이 있거나(또는 머리글이 아예 없으면) 페이지 제목의 절이 먼저 선다.
/// `all` 은 그 페이지 본문의 줄이고, 본문을 못 읽었으면 `None` 이다.
fn sections<'a, 't>(page: &'a Page, all: Option<&'t [&'t str]>) -> Vec<Section<'a, 't>> {
    let mk = |heading: Option<String>, anchor: Option<&'a str>, start: usize, lines: &'t [&'t str]| Section {
        page,
        heading,
        anchor,
        start,
        lines,
        head: Vec::new(),
        body: Vec::new(),
        head_len: 0.0,
        body_len: 0.0,
    };
    let Some(all) = all else { return vec![mk(None, None, 0, &[])] };
    let mut out = Vec::new();
    let first = page.headings.first().map_or(all.len(), |h| h.line.min(all.len()));
    // 첫 머리글 앞이 빈 줄뿐이면 절이 아니다 — 머리글 필드에 페이지 제목이 들어, 제목 낱말을 묻는 질의마다 발췌
    // 없는 `path:1` hit 가 그 페이지의 제목 절 곁에 하나 더 섰다(리뷰 moai-g8pt.9rd).
    if page.headings.is_empty() || all[..first].iter().any(|l| !l.trim().is_empty()) {
        out.push(mk(None, None, 0, &all[..first]));
    }
    for (n, h) in page.headings.iter().enumerate() {
        let from = h.line.min(all.len());
        let to = page.headings.get(n + 1).map_or(all.len(), |x| x.line.clamp(from, all.len()));
        let lines = &all[from..to];
        let text = lines.first().map(|l| heading_text(l)).unwrap_or_default();
        out.push(mk(Some(text), Some(h.anchor.as_str()), from, lines));
    }
    out
}

/// 질의 `query` 로 `pages` 의 절을 찾아 순위대로 `limit` 개까지 낸다. **순수 함수다.**
pub fn search<'a>(pages: &'a [Page], query: &str, limit: usize) -> Found<'a> {
    run(pages, query, limit, true)
}

/// [`search`] 와 같은 순위를 **발췌 없이** 낸다 — 모든 hit 의 `snippet` 이 없다. 순위를 다 받아 다시 고르는 쪽
/// (`show <id>` 의 관련 절, [`super::about`])이 버릴 hit 의 발췌까지 짓지 않게. **순수 함수다.**
pub fn rank<'a>(pages: &'a [Page], query: &str, limit: usize) -> Found<'a> {
    run(pages, query, limit, false)
}

/// 질의를 토큰으로 푼 것 — [`Found::terms`] 와 같다. 비었으면 찾을 것이 없는 질의다. **순수 함수다.**
pub fn terms(query: &str) -> Vec<String> {
    parse_query(query).0
}

/// 질의의 낱말 하나 — 접은 글과, 그것이 푼 토큰의 `terms` 속 자리(겹침 없이).
struct Word {
    text: String,
    pieces: Vec<usize>,
}

/// 질의를 토큰(`terms`)과 **원래의 낱말**로 푼다. 낱말은 [`each_token`] 이 가르는 덩어리 그대로다 — 글자·숫자·`_`
/// 덩어리는 토큰 하나라 낱말 하나고, 한글·한자·가나 덩어리는 글자 둘씩 낸 토큰 여럿이 낱말 하나다. 맞은 낱말을
/// 셀 때 토큰을 세면 `드라이버` 하나가 세 낱말로 선다(리뷰 moai-g8pt.9rd).
fn parse_query(query: &str) -> (Vec<String>, Vec<Word>) {
    let mut terms: Vec<String> = Vec::new();
    let mut words: Vec<Word> = Vec::new();
    let mut buf = String::new();
    let mut it = query.char_indices().peekable();
    while let Some((start, c)) = it.next() {
        let k = class(c);
        if k == 0 {
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(b, d)) = it.peek().filter(|&&(_, d)| class(d) == k) {
            end = b + d.len_utf8();
            it.next();
        }
        let run = &query[start..end];
        let mut pieces: Vec<usize> = Vec::new();
        each_token(run, &mut buf, |t, _| {
            let i = terms.iter().position(|x| x == t).unwrap_or_else(|| {
                terms.push(t.to_string());
                terms.len() - 1
            });
            if !pieces.contains(&i) {
                pieces.push(i);
            }
        });
        fold(run, &mut buf);
        if !words.iter().any(|w| w.text == buf) {
            words.push(Word { text: buf.clone(), pieces });
        }
    }
    (terms, words)
}

fn run<'a>(pages: &'a [Page], query: &str, limit: usize, snippets: bool) -> Found<'a> {
    let (terms, words) = parse_query(query);
    if terms.is_empty() {
        return Found { terms, words: Vec::new(), common: Vec::new(), hits: Vec::new(), total: 0 };
    }
    let want: HashSet<&str> = terms.iter().map(String::as_str).collect();
    let index: HashMap<&str, usize> = terms.iter().enumerate().map(|(i, t)| (t.as_str(), i)).collect();
    // 페이지 글은 여기 한 벌만 산다 — 절은 그 줄을 빌린다.
    let texts: Vec<Option<std::borrow::Cow<str>>> =
        pages.iter().map(|p| p.body.as_deref().map(|b| unified(unmarked(b)))).collect();
    let lines: Vec<Option<Vec<&str>>> = texts.iter().map(|t| t.as_deref().map(|t| t.split('\n').collect())).collect();
    let mut secs: Vec<Section> = pages.iter().zip(&lines).flat_map(|(p, l)| sections(p, l.as_deref())).collect();
    let mut buf = String::new();
    for s in &mut secs {
        // 머리글 필드는 이 절의 머리글과 페이지 제목이다. 머리글 줄은 본문 필드에서 뺀다 — 두 번 세지 않는다.
        // 토큰은 줄을 넘지 않으므로 줄마다 세는 것이 이어 붙여 세는 것과 같다.
        let (mut head, mut body) = (vec![0.0; terms.len()], vec![0.0; terms.len()]);
        let (mut head_len, mut body_len) = (0.0, 0.0);
        let mut count = |text: &str, field: &mut Vec<f64>, len: &mut f64| {
            each_token(text, &mut buf, |t, _| {
                *len += 1.0;
                if let Some(&i) = index.get(t) {
                    field[i] += 1.0;
                }
            })
        };
        count(&s.page.title, &mut head, &mut head_len);
        // 머리글이 페이지 제목과 같은 낱말이면(제목을 낸 맨 위 `# ` 의 절) 한 번만 센다 — 두 번 세던 판은 제목의
        // 낱말이 그 절에서 무게 6 으로 서서 다른 절의 3 을 거의 늘 이겼다(리뷰 moai-g8pt.9rd).
        if let Some(h) = s.heading.as_deref().filter(|h| !same_words(h, &s.page.title)) {
            count(h, &mut head, &mut head_len);
        }
        for l in s.lines.iter().skip(usize::from(s.heading.is_some())) {
            count(l, &mut body, &mut body_len);
        }
        (s.head, s.body, s.head_len, s.body_len) = (head, body, head_len, body_len);
    }
    let weighted = |s: &Section, i: usize| HEADING_WEIGHT * s.head[i] + s.body[i];
    let n = secs.len() as f64;
    let avg = secs.iter().map(|s| HEADING_WEIGHT * s.head_len + s.body_len).sum::<f64>() / n.max(1.0);
    let df: Vec<f64> = (0..terms.len()).map(|i| secs.iter().filter(|s| weighted(s, i) > 0.0).count() as f64).collect();
    let idf: Vec<f64> = df.iter().map(|df| (1.0 + (n - df + 0.5) / (df + 0.5)).ln()).collect();
    // 낱말이 절에서 맞았는가 — 토큰의 절반 이상이 들었으면 맞은 것이다. 글자·숫자 낱말은 토큰이 하나라 그 하나고,
    // `검색은` 은 `검색`·`색은` 가운데 하나로 넉넉하다(조사가 붙어도 맞는다). 맞은 낱말을 토큰으로 세던 판은
    // `드라이버` 하나가 세 낱말로 서서 낱말 둘의 문턱을 혼자 넘었다(리뷰 moai-g8pt.9rd).
    let hit = |s: &Section, w: &Word| 2 * w.pieces.iter().filter(|&&t| weighted(s, t) > 0.0).count() >= w.pieces.len();
    // **흔한 낱말**은 절의 [`COMMON_SHARE`] 보다 많은 곳에서 맞은 낱말이다 — 순위에서는 idf 가 이미 누르지만, 관련
    // 절을 고르는 쪽은 맞은 낱말의 수를 문턱으로 쓰므로 흔한 낱말 둘이 맞은 엉뚱한 절이 문턱을 넘는다(moai-g8pt.5aw).
    // 절 하나에만 든 낱말은 흔하지 않다 — 몫만 보면 절이 넷이 안 되는 위키에서 든 낱말이 다 흔해진다.
    let common: Vec<bool> = words
        .iter()
        .map(|w| secs.iter().filter(|s| hit(s, w)).count() as f64)
        .map(|df| df > 1.0 && df > COMMON_SHARE * n)
        .collect();
    let mut scored: Vec<(usize, usize, usize, f64)> = Vec::new();
    for (i, s) in secs.iter().enumerate() {
        let (mut distinct, mut rare) = (0, 0);
        for (w, common) in words.iter().zip(&common) {
            if hit(s, w) {
                distinct += 1;
                rare += usize::from(!common);
            }
        }
        if distinct == 0 {
            continue;
        }
        let len = HEADING_WEIGHT * s.head_len + s.body_len;
        let mut score = 0.0;
        for (t, idf) in idf.iter().enumerate() {
            let tf = weighted(s, t);
            if tf > 0.0 {
                score += idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len / avg.max(f64::MIN_POSITIVE)));
            }
        }
        scored.push((i, distinct, rare, score));
    }
    scored.sort_by(|a, b| {
        let (x, y) = (&secs[a.0], &secs[b.0]);
        b.1.cmp(&a.1)
            .then_with(|| b.3.total_cmp(&a.3))
            .then_with(|| x.page.slug.cmp(&y.page.slug))
            .then_with(|| x.start.cmp(&y.start))
    });
    let total = scored.len();
    let hits = scored
        .into_iter()
        .take(limit)
        .map(|(i, distinct, rare, score)| {
            let s = &secs[i];
            Hit {
                page: s.page,
                heading: s.heading.clone(),
                anchor: s.anchor,
                line: s.start + 1,
                distinct,
                rare,
                score,
                snippet: if snippets { snippet(s, &want) } else { None },
            }
        })
        .collect();
    let common = words.iter().zip(&common).filter(|(_, c)| **c).map(|(w, _)| w.text.clone()).collect();
    let words = words.into_iter().map(|w| w.text).collect();
    Found { terms, words, common, hits, total }
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
    let (text, marks) = fit(s.lines[n], want);
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
    // 앞에 남긴 폭(`w[lo..at]` 의 합)은 이어 센다 — 걸음마다 다시 더하면 폭 0 인 글자가 긴 줄에서 제곱이 든다.
    let mut lead = 0;
    while lo > 0 && used + w[lo - 1] <= budget && lead < LEAD_COLS {
        lo -= 1;
        used += w[lo];
        lead += w[lo];
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

    /// 토큰 — 가르는 곳, 접기(ASCII 와 그 밖), 한글 글자 둘씩, 원문 범위. 버퍼를 돌려 쓰는 걸음이 낱말마다 String 을
    /// 짓던 판과 같은 답을 낸다.
    #[test]
    fn tokens_split_fold_and_keep_the_source_ranges() {
        let s = "Merge-Driver 검색은 절 ÉCOLE ΟΔΟΣ snake_case 2026";
        let got: Vec<(String, &str)> = tokens(s).into_iter().map(|t| (t.text, &s[t.at])).collect();
        let want = [
            ("merge", "Merge"),
            ("driver", "Driver"),
            ("검색", "검색"),
            ("색은", "색은"),
            ("절", "절"),
            ("école", "ÉCOLE"),
            ("οδος", "ΟΔΟΣ"),
            ("snake_case", "snake_case"),
            ("2026", "2026"),
        ];
        let want: Vec<(String, &str)> = want.iter().map(|(a, b)| (a.to_string(), *b)).collect();
        // 끝의 Σ 는 `str::to_lowercase` 가 낱말 끝 꼴(ς)로 접는다 — 낱말째 접어야 그 답이 선다.
        let want: Vec<(String, &str)> =
            want.into_iter().map(|(a, b)| if b == "ΟΔΟΣ" { (b.to_lowercase(), b) } else { (a, b) }).collect();
        assert_eq!(got, want);
        assert!(tokens("  ,. ").is_empty());
        assert_eq!(tokens("가a나").iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["가", "a", "나"]);
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

    /// 맞은 낱말은 원래 낱말로 센다 — `드라이버` 는 토큰이 셋(`드라`·`라이`·`이버`)이어도 낱말 하나고, `병합 드라이버` 는
    /// 둘이다. 토큰을 세던 판은 `드라이버` 하나로 낱말 둘의 문턱을 넘었다(리뷰 moai-g8pt.9rd).
    #[test]
    fn a_korean_word_counts_once_however_many_pieces_it_has() {
        let pages = [page("d", "# 문서\n\n병합 드라이버를 깐다\n"), page("o", "# 다른\n\n드라이버만 있다\n")];
        let one = search(&pages, "드라이버", 5);
        assert_eq!((one.terms.len(), one.words.len()), (3, 1));
        assert!(one.hits.iter().all(|h| h.distinct == 1), "{:?}", one.hits);
        let two = search(&pages, "병합 드라이버", 5);
        assert_eq!(two.words, ["병합", "드라이버"]);
        assert_eq!((two.hits[0].page.slug.as_str(), two.hits[0].distinct), ("d", 2));
        assert_eq!((two.hits[1].page.slug.as_str(), two.hits[1].distinct), ("o", 1));
    }

    /// 한글 낱말은 토큰의 절반 이상이 든 절에서 맞는다 — 조사가 붙은 `검색은` 은 `검색` 하나로 맞고, `데이터베이스` 의
    /// 토큰 하나(`이스`)만 든 절은 hit 가 아니다.
    #[test]
    fn a_korean_word_matches_where_half_its_pieces_do() {
        let pages = [page("w", "# 위키\n\n검색은 절 단위로 한다\n"), page("x", "# 엑스\n\n이스 하나\n")];
        let h = &search(&pages, "검색은", 5).hits;
        assert_eq!((h.len(), h[0].distinct), (1, 1));
        assert!(search(&pages, "데이터베이스", 5).hits.is_empty(), "토큰 다섯 가운데 하나로 맞았다");
    }

    /// 제목을 낸 맨 위 `# ` 의 절은 제목을 한 번만 센다 — 제목 낱말이 그 절에서 무게 6 이던 판은, 같은 낱말을
    /// 머리글로 단 다른 절(무게 3)을 본문이 같아도 이겼다(리뷰 moai-g8pt.9rd).
    #[test]
    fn the_title_is_counted_once_in_the_section_under_it() {
        // 본문 없는 두 절 — 하나는 못 읽은 페이지의 제목 절(제목만), 하나는 제목을 낸 `# ` 의 절. 제목을 한 번 세면 둘의
        // 필드가 같아 점수가 같고, 두 번 세면 뒤의 것이 이긴다.
        let same = |pages: &[Page]| {
            let r = search(pages, "zebra", 5);
            assert_eq!(r.hits.len(), 2);
            assert!((r.hits[0].score - r.hits[1].score).abs() < 1e-9, "{:?}", r.hits);
        };
        same(&[unread("u", "Zebra"), page("a", "# Zebra\n")]);
        // 마크다운 기호가 든 제목도 같은 낱말이면 한 번이다 — 제목은 기호를 걷은 글이다.
        same(&[unread("u", "The zebra tool"), page("m", "# The `zebra` tool\n")]);
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
        assert!(!s.text.trim_end_matches('…').ends_with('e'), "결합 부호를 떼지 않는다");
    }

    #[test]
    fn a_short_line_is_trimmed_and_kept_whole() {
        let pages = [page("a", "# A\n\n    indented code word\n")];
        let s = search(&pages, "word", 5).hits[0].snippet.clone().unwrap();
        assert_eq!((s.text.as_str(), s.line), ("indented code word", 3));
    }

    /// 닫는 `#` 는 앞에 빈칸이 선 것만 걷는다 — `## C#` 의 `#` 는 글이다(리뷰 moai-g8pt.9rd). 인용 속 머리글은 `>` 를 뗀다.
    #[test]
    fn the_heading_keeps_a_hash_that_is_part_of_its_text() {
        assert_eq!(heading_text("## C#"), "C#");
        assert_eq!(heading_text("## Foo ##  "), "Foo");
        assert_eq!(heading_text("## #"), "");
        assert_eq!(heading_text("> ## Quoted"), "Quoted");
        assert_eq!(heading_text("Setext C#"), "Setext C#");
        let pages = [page("a", "# A\n\n## C#\n\nthe sharp language\n")];
        assert_eq!(top(&pages, "sharp"), [("a", Some("C#".into()))]);
    }

    /// 첫 머리글 앞이 빈 줄뿐이면 절이 아니다 — 제목 낱말 질의에 발췌 없는 `path:1` hit 가 하나 더 서지 않는다.
    #[test]
    fn a_blank_prelude_is_not_a_section() {
        let pages = [page("a", "\n\n# Zebra notes\n\nbody\n")];
        let r = search(&pages, "zebra", 5);
        assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
        assert_eq!((r.hits[0].line, r.hits[0].heading.as_deref()), (3, Some("Zebra notes")));
    }

    /// `total` 은 `limit` 이 자르기 전의 수다 — 부르는 쪽이 다 받아 자르지 않아도 "몇 중 몇" 을 댄다.
    #[test]
    fn total_counts_the_hits_before_the_limit() {
        let pages: Vec<Page> = ["a", "b", "c"].iter().map(|s| page(s, "# T\n\nsame words\n")).collect();
        let r = search(&pages, "same", 1);
        assert_eq!((r.hits.len(), r.total), (1, 3));
        assert_eq!(search(&pages, "", 1).total, 0);
        assert_eq!(terms("Merge-Driver merge"), ["merge", "driver"]);
    }

    /// 절 하나에만 든 낱말은 흔하지 않다 — 몫만 보면 절이 넷이 안 되는 위키에서 든 낱말이 다 흔해진다.
    #[test]
    fn a_word_in_one_section_is_never_common() {
        let pages = [page("a", "# A\n\nzebra here\n")];
        assert!(search(&pages, "zebra", 5).common.is_empty());
        let both = [page("a", "# A\n\nzebra\n"), page("b", "# B\n\nzebra\n")];
        assert_eq!(search(&both, "zebra", 5).common, ["zebra"], "둘 다에 든 낱말은 흔하다");
    }

    #[test]
    fn the_snippet_line_is_the_one_with_the_most_distinct_terms() {
        let pages = [page("a", "# A\n\nalpha alpha alpha\nalpha beta\nbeta\n")];
        let s = search(&pages, "alpha beta", 5).hits[0].snippet.clone().unwrap();
        assert_eq!((s.line, s.text.as_str()), (4, "alpha beta"));
    }
}
