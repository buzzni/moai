//! `moai add --from` 이 받는 마크다운을 읽는다. **순수 함수다** — 파일도
//! 터미널도 모른다.
//!
//! 사람이 "좋다" 고 한 순간 에이전트가 에픽 하나와 이슈 여섯을 **한 번의
//! 호출로** 만들 수 있어야 한다. 일곱 번 따로 부르면 에이전트가 중간에
//! 흘리고, 사람은 그 일곱 줄의 출력을 보며 지친다.
//!
//! 형식은 규칙 넷뿐이다.
//!
//! ```text
//! # 에픽 제목                     `#` 줄은 에픽
//! - [p1] 이슈 제목 #bug           `-` 줄은 바로 위 에픽의 이슈
//! - 이슈 제목                     [pN] 과 #태그 는 없어도 된다
//! ```
//!
//! **JSON 입력은 두지 않는다.** 에이전트가 Bash heredoc 안에서 중첩 JSON 을
//! 이스케이프하는 것이 마크다운을 쓰는 것보다 훨씬 잘 깨진다. 그리고 사람도
//! 이 마크다운은 손으로 쓴다.

use crate::model::{Issue, Kind, normalize_tag};

#[derive(Debug, PartialEq)]
pub struct Draft {
    pub kind: Kind,
    pub title: String,
    pub priority: Option<u8>,
    pub tags: Vec<String>,
    /// 몇 번째 에픽에 속하는가. 에픽 자신은 `None`.
    pub epic: Option<usize>,
}

/// 줄 끝의 `#태그` 들과 앞머리의 `[pN]` 을 떼어 낸다.
///
/// 태그는 **끝에서부터** 뗀다 — 제목 가운데의 `#` 은 제목의 일부다
/// (`--json 이 #1 에서 깨진다`).
fn split_parts(raw: &str, n: usize) -> Result<(String, Option<u8>, Vec<String>), String> {
    let mut rest = raw.trim();
    let mut priority = None;

    if let Some(after) = rest.strip_prefix('[') {
        let (tag, tail) = after
            .split_once(']')
            .ok_or_else(|| format!("{n}줄: `[` 를 닫지 않았다 — {raw:?}"))?;
        let p = tag
            .trim()
            .strip_prefix('p')
            .and_then(|d| d.parse::<u8>().ok())
            .filter(|p| *p <= crate::model::MAX_PRIORITY)
            .ok_or_else(|| {
                format!("{n}줄: `[{tag}]` 는 우선순위가 아니다. `[p0]`~`[p{}]`", crate::model::MAX_PRIORITY)
            })?;
        priority = Some(p);
        rest = tail.trim();
    }

    let mut tags = Vec::new();
    while let Some((head, last)) = rest.rsplit_once(char::is_whitespace) {
        match last.strip_prefix('#') {
            Some(t) if !t.is_empty() => {
                tags.push(normalize_tag(t));
                rest = head.trim_end();
            }
            _ => break,
        }
    }
    tags.reverse();

    if rest.is_empty() {
        return Err(format!("{n}줄: 제목이 없다 — {raw:?}"));
    }
    Ok((rest.to_string(), priority, tags))
}

/// 못 읽은 줄은 **전부** 모아 한 번에 말한다. 하나씩 고치게 하면 여섯 줄짜리
/// heredoc 을 여섯 번 다시 보낸다.
pub fn parse(src: &str) -> Result<Vec<Draft>, String> {
    let mut out: Vec<Draft> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut epic: Option<usize> = None;

    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let (kind, body) = match l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
            Some(b) => (Kind::Issue, b),
            None => match l.strip_prefix("# ") {
                Some(b) => (Kind::Epic, b),
                None => {
                    errors.push(format!(
                        "{n}줄: `# 에픽` 도 `- 이슈` 도 아니다 — {l:?}"
                    ));
                    continue;
                }
            },
        };
        if kind == Kind::Issue && epic.is_none() {
            errors.push(format!(
                "{n}줄: 어느 에픽의 이슈인지 알 수 없다. 위에 `# 에픽 제목` 을 둔다 — {l:?}"
            ));
            continue;
        }
        match split_parts(body, n) {
            Err(e) => errors.push(e),
            Ok((title, priority, tags)) => {
                if kind == Kind::Epic {
                    epic = Some(out.len());
                }
                out.push(Draft {
                    kind,
                    title,
                    priority,
                    tags,
                    epic: (kind == Kind::Issue).then_some(epic).flatten(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n      "));
    }
    if out.is_empty() {
        return Err("읽을 것이 없다. `# 에픽 제목` 과 `- 이슈 제목` 을 적는다".into());
    }
    Ok(out)
}

/// [`parse`] 의 반대. 에픽 하나와 그 멤버를 `add --from` 이 받는 마크다운으로
/// 되뽑는다 (`moai show <에픽> --as-plan`).
///
/// **이 파일에 둔다.** 형식을 아는 자리가 둘이 되면 되뽑은 것을 도로 넣을 때
/// 한쪽만 바뀐 형식에서 깨진다 — 짝을 같은 파일에 두고 되돌려 읽는 시험으로 묶는다.
///
/// - 형식이 받는 것만 낸다: 제목·`[pN]`·`#태그`. 막음(`blocked_by`)·담당·본문·
///   부모는 형식에 자리가 없어 빠진다 — 자식 이슈는 형제로 평평하게 선다. 없는 문법을 지어내면 `add --from` 이 그 줄을
///   거부해 되뽑은 것이 도로 안 들어간다 (막음은 moai-quxo 의 단계 의존과 함께)
/// - 우선순위는 **적힌 것만** 낸다. 기본값을 채워 쓰면 템플릿이 설정의 기본값을
///   박제한다
/// - 멤버는 받은 차례 그대로 적는다 — 차례를 정하는 것은 부르는 쪽이다
/// - 제목은 [`crate::text::one_line`] 을 지난다. 형식은 한 줄에 하나라 손으로 고친
///   파일의 줄바꿈이 든 제목은 다음 줄을 딴 이슈로 만들고, 제어문자는 이 글을
///   그대로 찍는 터미널을 다시 칠한다
pub fn render(epic: &Issue, members: &[&Issue]) -> String {
    std::iter::once(format!("# {}\n", body(epic)))
        .chain(members.iter().map(|m| format!("- {}\n", body(m))))
        .collect()
}

/// 글머리(`#`·`-`)를 뗀 한 줄 — [`split_parts`] 가 읽는 바로 그 자리.
fn body(i: &Issue) -> String {
    let mut s = String::new();
    if let Some(p) = i.priority {
        s.push_str(&format!("[p{p}] "));
    }
    s.push_str(&crate::text::one_line(&i.title));
    for t in &i.tags {
        s.push_str(&format!(" #{t}"));
    }
    s
}

/// [`render`] 가 쓴 줄을 [`parse`] 가 **다르게 읽는** 줄의 id.
///
/// 형식에 이스케이프가 없어 `[WIP] 반쯤` 은 우선순위로 읽혀 거부되고 `이슈 #12` 는
/// 끝 낱말이 태그로 먹힌다. render 안에서는 고칠 수 없다 — 고치면 parse 가 받는 것이
/// 바뀐다. 그래서 **조용히 틀리지 않고 이름을 댄다**(moai-nb8d.j61). 되뽑은 글은
/// 사람이 다듬는 틀이라 거절하지는 않는다.
///
/// 판정은 짝이 직접 한다 — 쓴 줄을 도로 읽어 견준다. 규칙을 따로 적으면 parse 가
/// 바뀌는 날 이쪽만 옛 규칙으로 남는다.
pub fn lossy<'a>(epic: &'a Issue, members: &[&'a Issue]) -> Vec<&'a str> {
    std::iter::once(epic)
        .chain(members.iter().copied())
        .filter(|i| {
            split_parts(&body(i), 0).ok() != Some((crate::text::one_line(&i.title), i.priority, i.tags.clone()))
        })
        .map(|i| i.id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Vec<Draft> {
        parse(src).unwrap()
    }

    fn issue(title: &str, kind: Kind, priority: Option<u8>, tags: &[&str]) -> Issue {
        let mut i = Issue::new("x-1".into(), title.into(), kind, crate::model::Status::new("todo"), "2026-09-14T00:00:00Z");
        i.priority = priority;
        i.tags = tags.iter().map(|t| t.to_string()).collect();
        i
    }

    /// **되뽑은 것은 도로 들어가야 한다.** 이 짝이 어긋나면 `--as-plan` 은
    /// 템플릿이 아니라 손으로 고쳐야 쓸 수 있는 글이다.
    #[test]
    fn what_render_writes_parse_reads_back() {
        let epic = issue("릴리스", Kind::Epic, Some(1), &["release"]);
        let a = issue("--json 이 #1 에서 깨진다", Kind::Issue, Some(0), &["bug", "parser"]);
        let b = issue("태그도 우선순위도 없다", Kind::Issue, None, &[]);
        let md = render(&epic, &[&a, &b]);
        assert_eq!(md, "# [p1] 릴리스 #release\n- [p0] --json 이 #1 에서 깨진다 #bug #parser\n- 태그도 우선순위도 없다\n");
        let got = one(&md);
        assert_eq!(got[0], Draft { kind: Kind::Epic, title: "릴리스".into(), priority: Some(1), tags: vec!["release".into()], epic: None });
        assert_eq!(got[1], Draft { kind: Kind::Issue, title: a.title.clone(), priority: Some(0), tags: vec!["bug".into(), "parser".into()], epic: Some(0) });
        assert_eq!(got[2], Draft { kind: Kind::Issue, title: b.title.clone(), priority: None, tags: vec![], epic: Some(0) });
    }

    /// 손으로 고친 파일의 제목에 줄바꿈이나 ESC 가 들어도 한 줄에 하나로 선다.
    #[test]
    fn a_title_that_spans_lines_stays_one_line() {
        let epic = issue("가", Kind::Epic, None, &[]);
        let bad = issue("앞\n뒤\u{1b}[2J", Kind::Issue, None, &[]);
        let md = render(&epic, &[&bad]);
        assert_eq!(md, "# 가\n- 앞  뒤[2J\n");
        assert_eq!(one(&md).len(), 2, "줄바꿈이 이슈를 하나 더 만들었다");
    }

    /// 형식이 담지 못하는 제목은 이름이 불린다. 앞머리 `[` 는 우선순위로, 끝의
    /// `#낱말` 은 태그로 읽혀 도로 넣으면 거부되거나 조용히 바뀐다.
    #[test]
    fn titles_the_format_cannot_hold_are_named() {
        let epic = issue("가", Kind::Epic, None, &[]);
        let mut wip = issue("[WIP] 반쯤", Kind::Issue, None, &[]);
        wip.id = "x-wip".into();
        let mut hash = issue("이슈 #12 를 고친다 #12", Kind::Issue, Some(1), &[]);
        hash.id = "x-hash".into();
        let mut fine = issue("--json 이 #1 에서 깨진다", Kind::Issue, Some(0), &["bug"]);
        fine.id = "x-fine".into();
        assert_eq!(lossy(&epic, &[&wip, &hash, &fine]), ["x-wip", "x-hash"]);

        // 짝이 실제로 그렇게 읽는지 — 판정이 헛짚은 것이 아님을 본다.
        let md = render(&epic, &[&wip]);
        assert!(parse(&md).is_err(), "{md}");
    }

    /// 멤버가 없는 에픽도 도로 들어가는 계획이다 — 에픽 줄 하나.
    #[test]
    fn an_empty_epic_is_one_line() {
        let md = render(&issue("빈 에픽", Kind::Epic, None, &[]), &[]);
        assert_eq!(md, "# 빈 에픽\n");
        assert_eq!(one(&md).len(), 1);
    }

    #[test]
    fn reads_an_epic_and_its_issues() {
        let got = one("# 저장 계층\n- [p1] 원자적으로 쓴다 #enhancement\n- 잘린 줄을 복구한다 #bug\n");
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], Draft { kind: Kind::Epic, title: "저장 계층".into(), priority: None, tags: vec![], epic: None });
        assert_eq!(got[1], Draft { kind: Kind::Issue, title: "원자적으로 쓴다".into(), priority: Some(1), tags: vec!["enhancement".into()], epic: Some(0) });
        assert_eq!(got[2].epic, Some(0));
    }

    /// 에픽이 둘이면 그다음 이슈는 **뒤엣것**에 붙는다.
    #[test]
    fn a_second_epic_takes_over() {
        let got = one("# 가\n- 첫째\n# 나\n- 둘째\n");
        assert_eq!(got[1].epic, Some(0));
        assert_eq!(got[3].epic, Some(2));
    }

    #[test]
    fn tags_come_off_the_end_only() {
        let got = one("# 가\n- --json 이 #1 에서 깨진다 #bug #parser\n");
        assert_eq!(got[1].title, "--json 이 #1 에서 깨진다", "제목 가운데의 # 을 태그로 봤다");
        assert_eq!(got[1].tags, ["bug", "parser"]);
    }

    #[test]
    fn tags_fold_like_everywhere_else() {
        assert_eq!(one("# 가\n- 제목 #Bug\n")[1].tags, ["bug"]);
    }

    #[test]
    fn blank_lines_and_star_bullets_are_fine() {
        let got = one("\n# 가\n\n* 별표도 받는다\n\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].title, "별표도 받는다");
    }

    /// 못 읽은 줄은 전부 모아 한 번에 말한다.
    #[test]
    fn every_refusal_names_its_line() {
        let e = parse("# 가\n이건 뭔가\n- [p9] 너무 큼\n- [열림] 숫자가 아님\n- \n").unwrap_err();
        for want in ["2줄", "3줄", "4줄", "5줄"] {
            assert!(e.contains(want), "{want} 가 없다 — {e}");
        }
    }

    /// 에픽 없이 시작하면 거부한다. 이 도구는 계획 없이 쌓이는 것을 막으려고
    /// 있고, 대량 생성이야말로 그 일이 제일 잘 일어나는 자리다.
    #[test]
    fn an_issue_without_an_epic_is_refused() {
        let e = parse("- 그냥 하나\n").unwrap_err();
        assert!(e.contains("어느 에픽의") && e.contains("1줄"), "{e}");
    }

    #[test]
    fn nothing_to_read_says_so() {
        assert!(parse("\n\n").unwrap_err().contains("읽을 것이 없다"));
    }
}
