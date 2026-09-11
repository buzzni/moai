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

use crate::model::{Kind, normalize_tag};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Vec<Draft> {
        parse(src).unwrap()
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
