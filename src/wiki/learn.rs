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

/// 제목에서 짓는 슬러그의 상한(바이트) — 넘으면 그 앞의 `-` 에서 자른다. 파일 이름의 상한(NAME_MAX 255)보다 한참
/// 밑이라, `--dry-run` 이 "쓸 것이다" 라고 한 이름을 진짜 부름이 "이름이 너무 길다" 로 거절하지 않는다(리뷰
/// moai-4s5n.u8x). `--slug` 로 준 이름은 자르지 않는다 — 사람이 고른 이름이다.
pub const SLUG_MAX: usize = 100;

/// 제목에서 슬러그를 짓는다 — **제목이 ASCII 일 때만이다.** 한국어 제목을 소리로 옮겨 짓지 않는다: 옮긴 이름은
/// 아무도 찾지 못한다. 그때는 `None` 이고 부르는 쪽이 `--slug` 를 대라고 한다. 글자·숫자 아닌 것의 줄은 `-`
/// 하나가 되고 앞뒤의 `-` 는 걷는다. 그렇게 해서 빈 글이면(`'!!!'`) 역시 `None` 이다. [`SLUG_MAX`] 를 넘으면 토막
/// 가운데가 아니라 그 앞의 `-` 에서 자른다 — 같은 제목은 늘 같은 이름이다.
pub fn slug_from(title: &str) -> Option<String> {
    if !title.is_ascii() {
        return None;
    }
    let mut built = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            built.push(c.to_ascii_lowercase());
        } else if !built.is_empty() && !built.ends_with('-') {
            built.push('-');
        }
    }
    let mut out = built.trim_end_matches('-');
    if out.len() > SLUG_MAX {
        // ASCII 라 바이트 자리가 곧 글자 자리다. `-` 가 상한 안에 없는 한 낱말이면 상한에서 자른다.
        let head = &out[..=SLUG_MAX];
        out = head.rfind('-').map_or(&out[..SLUG_MAX], |at| &head[..at]);
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
    /// 본문이 덩이(울타리·HTML 덩이)를 연 채 끝난다 — 그 뒤에 붙는 `Learned in:` 줄까지 그 덩이에 든다.
    OpenBlock,
}

/// 교훈 페이지의 글 전부 — 제목 한 줄, 빈 줄, 본문, 빈 줄, `Learned in: <id>`, 끝에 줄바꿈 하나. 이슈가 없으면
/// (`--issue none`) `Learned in:` 줄이 안 선다.
///
/// **같은 것을 받으면 같은 바이트다.** 본문 앞의 빈 줄과 끝의 빈칸은 걷는다 — heredoc 의 끝 줄바꿈 하나가 있고
/// 없고로 파일이 갈리지 않게. 첫 줄의 들여쓰기는 남긴다(들여 쓴 코드 덩이일 수 있다). 파일 머리의 BOM 도 걷는다 —
/// 남으면 `# 제목` 이 제목으로 안 읽혀 여는 제목의 거절을 지나고 페이지 가운데에 U+FEFF 가 선다(리뷰
/// moai-4s5n.u8x, `cmd::add::read_msg` 와 같은 자). 본문 안은 한 글자도 안 바꾼다. 제목은 앞뒤 빈칸만 걷는다.
pub fn page(title: &str, body: &str, issue: Option<&str>) -> Result<String, Refused> {
    let title = title.trim();
    if title.is_empty() || title.contains(['\n', '\r']) {
        return Err(Refused::Title);
    }
    let body = crate::text::blank_lines_trimmed(super::unmarked(body));
    if body.is_empty() {
        return Err(Refused::Empty);
    }
    check(body)?;
    // **본문이 연 덩이는 본문 안에서 닫힌다**(리뷰 moai-4s5n.u8x) — 닫히지 않은 울타리나 HTML 덩이는 그 뒤의
    // `Learned in:` 줄까지 삼켜, 페이지가 제 이슈를 안 댄다(위키는 울타리 안의 id 를 안 센다). 이슈가 없는 부름도 같은
    // 자로 잰다 — 꼴은 이슈를 모른 채 먼저 재고, 끝이 열린 채 커밋되는 페이지는 그것만으로 깨진 글이다.
    if !closes(body) {
        return Err(Refused::OpenBlock);
    }
    let mut out = format!("# {title}\n\n{body}\n");
    if let Some(id) = issue {
        out.push_str(&format!("\n{LEARNED_IN} {id}\n"));
    }
    Ok(out)
}

/// 페이지 끝에서 이슈를 대는 줄의 머리.
const LEARNED_IN: &str = "Learned in:";

/// 본문 뒤에 빈 줄을 두고 붙인 한 줄이 **맨 바깥의 제 문단으로** 서는가 — 그러면 본문이 연 덩이는 다 닫혔다. 위키를
/// 읽는 자와 같은 마크다운 읽기([`crate::markdown::OPTIONS`])로 잰다.
fn closes(body: &str) -> bool {
    let probe = format!("{body}\n\n{LEARNED_IN}");
    let (mut depth, mut paragraph, mut text) = (0usize, false, String::new());
    for event in Parser::new_ext(&probe, crate::markdown::OPTIONS) {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    paragraph = matches!(tag, Tag::Paragraph);
                    text.clear();
                }
                depth += 1;
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Text(t) => text.push_str(&t),
            _ => {}
        }
    }
    paragraph && text == LEARNED_IN
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

    /// **본문이 연 덩이는 거절한다**(리뷰 moai-4s5n.u8x) — 닫히지 않은 울타리나 HTML 덩이는 뒤에 붙는 `Learned in:` 줄을
    /// 삼켜, 페이지가 제 이슈를 안 댄다. 닫은 울타리, 목록·인용으로 끝나는 본문은 그대로 받는다.
    #[test]
    fn a_body_that_leaves_a_block_open_is_refused() {
        for open in
            ["## Do this instead\n```sh\nset -e", "## Do this instead\ny\n\n<!-- later", "## Do this instead\n~~~\nx"]
        {
            assert_eq!(page("T", open, Some("moai-abcd")), Err(Refused::OpenBlock), "{open}");
        }
        for closed in [
            "## Do this instead\n```sh\nset -e\n```",
            "## Do this instead\n- one\n- two",
            "## Do this instead\n> quoted",
            "## Do this instead\n\n    indented code",
        ] {
            let text = page("T", closed, Some("moai-abcd")).unwrap_or_else(|e| panic!("{closed}: {e:?}"));
            assert!(text.ends_with("\n\nLearned in: moai-abcd\n"), "{text}");
        }
    }

    /// 파일 머리의 BOM 은 글이 아니다 — 걷고 잰다. 남기던 판은 BOM 뒤의 `# 제목` 을 줄글로 읽어 여는 제목의 거절을 지났다.
    #[test]
    fn a_leading_bom_is_not_part_of_the_body() {
        assert_eq!(page("T", "\u{feff}# Own\n\n## Do this instead\ny", None), Err(Refused::OpensWithTitle));
        assert_eq!(page("T", "\u{feff}## Do this instead\ny", None).unwrap(), "# T\n\n## Do this instead\ny\n");
    }

    /// 긴 제목의 슬러그는 [`SLUG_MAX`] 안에서 `-` 에 맞춰 자른다 — 파일 이름의 상한에 안 닿게, 같은 제목은 같은 이름으로.
    #[test]
    fn a_long_title_makes_a_bounded_slug() {
        let title = vec!["word"; 80].join(" ");
        let slug = slug_from(&title).unwrap();
        assert!(slug.len() <= SLUG_MAX && is_slug(&slug) && slug.ends_with("word"), "{slug}");
        assert_eq!(slug_from(&title), Some(slug));
        let one = "x".repeat(SLUG_MAX + 20);
        assert_eq!(slug_from(&one).map(|s| s.len()), Some(SLUG_MAX), "`-` 가 없는 한 낱말은 상한에서 자른다");
    }
}
