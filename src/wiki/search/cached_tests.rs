//! The pre-index algorithm is a test-only oracle; the original 31 ranking tests stay untouched.
use super::*;

struct Counted<'a, 't> {
    section: Section<'a, 't>,
    head: Vec<f64>,
    body: Vec<f64>,
    head_len: f64,
    body_len: f64,
}

impl<'a, 't> std::ops::Deref for Counted<'a, 't> {
    type Target = Section<'a, 't>;
    fn deref(&self) -> &Self::Target {
        &self.section
    }
}

#[test]
fn checkout_index_matches_the_previous_algorithm() {
    let wiki = crate::wiki::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), "docs", "moai", &|_| true).unwrap();
    assert!(!wiki.pages.is_empty());
    let index = Index::of(&wiki.pages);
    for query in
        ["", "zzqqxx", "read worktree", "moai show id", "merge driver", "위키 검색은", "archive_days", "release --json"]
    {
        for limit in [0, 1, 10, usize::MAX] {
            assert_eq!(
                index.search(&wiki.pages, query, limit),
                legacy(&wiki.pages, query, limit, true),
                "{query:?}, {limit}"
            );
            assert_eq!(
                index.rank(&wiki.pages, query, limit),
                legacy(&wiki.pages, query, limit, false),
                "{query:?}, {limit}"
            );
        }
    }
}

fn legacy<'a>(pages: &'a [Page], query: &str, limit: usize, snippets: bool) -> Found<'a> {
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
    let mut secs: Vec<Counted> = pages
        .iter()
        .zip(&lines)
        .flat_map(|(p, l)| sections(p, l.as_deref()))
        .map(|section| Counted { section, head: Vec::new(), body: Vec::new(), head_len: 0.0, body_len: 0.0 })
        .collect();
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
    let weighted = |s: &Counted, i: usize| HEADING_WEIGHT * s.head[i] + s.body[i];
    let n = secs.len() as f64;
    let avg = secs.iter().map(|s| HEADING_WEIGHT * s.head_len + s.body_len).sum::<f64>() / n.max(1.0);
    let df: Vec<f64> = (0..terms.len()).map(|i| secs.iter().filter(|s| weighted(s, i) > 0.0).count() as f64).collect();
    let idf: Vec<f64> = df.iter().map(|df| (1.0 + (n - df + 0.5) / (df + 0.5)).ln()).collect();
    // 낱말이 절에서 맞았는가 — 토큰의 절반 이상이 들었으면 맞은 것이다. 글자·숫자 낱말은 토큰이 하나라 그 하나고,
    // `검색은` 은 `검색`·`색은` 가운데 하나로 넉넉하다(조사가 붙어도 맞는다). 맞은 낱말을 토큰으로 세던 판은
    // `드라이버` 하나가 세 낱말로 서서 낱말 둘의 문턱을 혼자 넘었다(리뷰 moai-g8pt.9rd).
    let hit = |s: &Counted, w: &Word| 2 * w.pieces.iter().filter(|&&t| weighted(s, t) > 0.0).count() >= w.pieces.len();
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
