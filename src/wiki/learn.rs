//! 교훈 페이지 — `moai wiki learn` 이 짓는 글(moai-4s5n.n7j). **순수 함수다** — 파일도 트래커도 모른다.
//!
//! 교훈 하나가 `<wiki_dir>/lessons/<slug>.md` 파일 하나다(에픽 moai-4s5n 계획 때 정한 자리). 한 파일에 여러
//! 세션이 덧붙이면 가지마다 같은 끝 줄에 붙어 병합이 충돌하니 나눈다. 목차는 두지 않는다 — 파생값이고,
//! `moai wiki ls`·`moai wiki search` 가 낸다.
//!
//! **슬러그의 자는 여기 하나다** — 위키 스킬이 새 파일 이름을 가르치는 글([`SLUG_RULE`])과 이 명령이 재는
//! 자([`is_slug`])가 같은 자리에 산다. 따로 적으면 스킬이 가르친 이름을 명령이 거절하는 날이 온다.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

/// 교훈 페이지가 서는 하위 디렉터리 — 위키 뿌리 밑이다. 설정 키를 두지 않는다(에픽 moai-4s5n).
pub const LESSONS: &str = "lessons";

/// 교훈 본문에 반드시 서는 머리글 — 다음 사람이 읽을 것은 이 절이다.
pub const DO_INSTEAD: &str = "Do this instead";

/// 새 페이지 파일 이름의 자를 사람 말로 — 위키 스킬(`guide::wiki`)과 이 명령의 거절문이 이 글을 쓴다. 재는 자는
/// [`is_slug`] 다.
pub const SLUG_RULE: &str = "lowercase ASCII kebab case";

/// [`SLUG_RULE`] 를 재는 자 — 소문자 ASCII 와 숫자의 토막을 `-` 하나로 잇는다(`grep-exit-code`). 빈 글, 앞뒤의
/// `-`, 겹친 `--` 는 아니다. 어느 기계에서나 같은 슬러그로 읽히고 셸 낱말로 감쌀 일이 없다.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// 제목에서 슬러그를 짓는다 — **제목이 ASCII 일 때만이다.** 한국어 제목을 소리로 옮겨 짓지 않는다: 옮긴 이름은
/// 아무도 찾지 못한다. 그때는 `None` 이고 부르는 쪽이 `--slug` 를 대라고 한다. 글자·숫자 아닌 것의 줄은 `-`
/// 하나가 되고 앞뒤의 `-` 는 걷는다. 그렇게 해서 빈 글이면(`'!!!'`) 역시 `None` 이다.
pub fn slug_from(title: &str) -> Option<String> {
    if !title.is_ascii() {
        return None;
    }
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    is_slug(&out).then_some(out)
}

/// 위키 뿌리에서의 교훈 페이지 슬러그 — `moai wiki show` 가 받는 그 이름이다(`lessons/<slug>`).
pub fn page_slug(slug: &str) -> String {
    format!("{LESSONS}/{slug}")
}

/// 교훈이 나온 이슈에 다는 노트 — `Lesson: lessons/<slug>`. 되풀이를 적는 줄도 이 꼴이다(에픽 moai-4s5n 고리 4).
pub fn note(slug: &str) -> String {
    format!("Lesson: {}", page_slug(slug))
}

/// 짓지 않는 까닭 — 말이 아니라 자료다. 글은 `view::wiki` 가 짓는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// 제목이 비었거나 여러 줄이다 — 제목은 `# ` 한 줄이 된다.
    Title,
    /// 본문이 비었다.
    Empty,
    /// 본문이 1단 제목으로 시작한다 — 제목은 인자 하나다.
    OpensWithTitle,
    /// `## Do this instead` 머리글이 없다.
    NoDoInstead,
}

/// 교훈 페이지의 글 전부 — 제목 한 줄, 빈 줄, 본문, 빈 줄, `Learned in: <id>`, 끝에 줄바꿈 하나. 이슈가 없으면
/// (`--issue none`) `Learned in:` 줄이 안 선다.
///
/// **같은 것을 받으면 같은 바이트다.** 본문 앞의 빈 줄과 끝의 빈칸은 걷는다 — heredoc 의 끝 줄바꿈 하나가 있고
/// 없고로 파일이 갈리지 않게. 본문 안은 한 글자도 안 바꾼다. 제목은 앞뒤 빈칸만 걷는다.
pub fn page(title: &str, body: &str, issue: Option<&str>) -> Result<String, Refused> {
    let title = title.trim();
    if title.is_empty() || title.contains(['\n', '\r']) {
        return Err(Refused::Title);
    }
    let body = trimmed(body);
    if body.is_empty() {
        return Err(Refused::Empty);
    }
    check(body)?;
    let mut out = format!("# {title}\n\n{body}\n");
    if let Some(id) = issue {
        out.push_str(&format!("\nLearned in: {id}\n"));
    }
    Ok(out)
}

/// 앞의 빈 줄과 끝의 빈칸을 걷는다 — 첫 줄의 들여쓰기는 남긴다(들여 쓴 코드 블록일 수 있다).
fn trimmed(body: &str) -> &str {
    let body = body.trim_end();
    let mut start = 0;
    for line in body.split_inclusive('\n') {
        if !line.trim().is_empty() {
            break;
        }
        start += line.len();
    }
    &body[start..]
}

/// 본문의 꼴을 잰다 — 첫 블록이 1단 제목이면 거절, 2단 `Do this instead` 머리글이 없으면 거절. 울타리 안의
/// `## Do this instead` 는 머리글이 아니다 — 마크다운으로 읽으니 저절로 그렇다.
fn check(body: &str) -> Result<(), Refused> {
    let mut in_h2 = false;
    let mut words = String::new();
    let mut found = false;
    // 첫 사건은 늘 첫 블록의 `Start` 다 — 그것이 1단 제목이면 본문이 제목으로 연다.
    for (n, event) in Parser::new_ext(body, crate::markdown::OPTIONS).enumerate() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                if n == 0 && level == HeadingLevel::H1 {
                    return Err(Refused::OpensWithTitle);
                }
                in_h2 = level == HeadingLevel::H2;
                words.clear();
            }
            Event::Text(t) | Event::Code(t) if in_h2 => words.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                found |= in_h2 && words.trim().eq_ignore_ascii_case(DO_INSTEAD);
                in_h2 = false;
            }
            _ => {}
        }
    }
    if found { Ok(()) } else { Err(Refused::NoDoInstead) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "## What happened\nx\n\n## Do this instead\ny";

    #[test]
    fn a_slug_is_lowercase_ascii_kebab_case() {
        for ok in ["grep-exit-code", "a", "v2-merge", "x1"] {
            assert!(is_slug(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "a--b", "Grep", "a_b", "a b", "한글", "a/b", "a.md"] {
            assert!(!is_slug(bad), "{bad}");
        }
    }

    #[test]
    fn an_ascii_title_makes_the_slug_and_a_korean_one_does_not() {
        assert_eq!(
            slug_from("Never end a check script with a grep that may find nothing").as_deref(),
            Some("never-end-a-check-script-with-a-grep-that-may-find-nothing")
        );
        assert_eq!(slug_from("  `git add` before -- commit!  ").as_deref(), Some("git-add-before-commit"));
        assert_eq!(slug_from("검사 스크립트를 grep 으로 끝내지 않는다"), None);
        assert_eq!(slug_from("!!!"), None);
    }

    #[test]
    fn the_page_is_the_form_byte_for_byte() {
        let want = "# Title\n\n## What happened\nx\n\n## Do this instead\ny\n\nLearned in: moai-abcd\n";
        assert_eq!(page(" Title ", BODY, Some("moai-abcd")).unwrap(), want);
        // 앞의 빈 줄과 끝의 줄바꿈은 걷는다 — 같은 것을 받으면 같은 바이트다.
        assert_eq!(page("Title", &format!("\n\n{BODY}\n\n"), Some("moai-abcd")).unwrap(), want);
        assert_eq!(page("Title", BODY, None).unwrap(), "# Title\n\n## What happened\nx\n\n## Do this instead\ny\n");
    }

    #[test]
    fn the_form_is_held_strictly() {
        assert_eq!(page("T", "## Why\nx", None), Err(Refused::NoDoInstead));
        assert_eq!(page("T", "# T\n\n## Do this instead\ny", None), Err(Refused::OpensWithTitle));
        assert_eq!(page("T", "T\n===\n\n## Do this instead\ny", None), Err(Refused::OpensWithTitle));
        // 울타리 안의 머리글은 머리글이 아니다.
        assert_eq!(page("T", "```\n## Do this instead\n```", None), Err(Refused::NoDoInstead));
        // 3단이나 1단의 같은 글은 그 절이 아니다.
        assert_eq!(page("T", "x\n\n### Do this instead\ny", None), Err(Refused::NoDoInstead));
        assert_eq!(page("", BODY, None), Err(Refused::Title));
        assert_eq!(page("a\nb", BODY, None), Err(Refused::Title));
        assert_eq!(page("T", "  \n\n", None), Err(Refused::Empty));
        // 본문 가운데의 1단 제목은 막지 않는다 — 막는 것은 여는 제목이다.
        assert!(page("T", "x\n\n# Later\n\n## Do this instead\ny", None).is_ok());
    }
}
