//! 생각 담기를 **외부 편집기로** 적을 때의 파일 글(moai-08af). git 이 커밋 메시지를 받는
//! 모양 그대로다 — 첫 줄이 제목, 한 줄 띄우고 본문, `#` 줄은 안내라 걷는다.
//!
//! **조각이다.** 환경변수도 파일도 프로세스도 모른다: 무엇을 쓸지(`template`), 받은 글을
//! 어떻게 읽을지(`parse`), 어느 편집기를 어떤 인자로 띄울지(`pick`·`argv`)만 정한다.
//! 파일을 만들고 편집기를 띄우고 터미널을 내리는 것은 루프(`cmd::tui`)가 한다 — 그래야
//! 이 규칙들이 터미널 없이 시험된다.

use super::form::Target;
use std::ffi::OsString;

/// 파일에 먼저 적어 두는 글. **첫 줄은 비워 둔다** — 편집기가 첫 줄에 커서를 두므로
/// 곧바로 제목을 친다. 담을 곳의 이름과 경로는 한 줄로 접는다(남이 지은 이름이라
/// 줄바꿈·제어문자가 들 수 있고, 들면 그 뒷줄이 주석이 아니게 된다).
pub fn template(into: Option<&Target>, lang: crate::i18n::Lang) -> String {
    use crate::i18n::{fill, say};
    let place = match into {
        Some(t) => format!("{}  {}", crate::text::one_line(&t.name), crate::text::one_line(&t.path.display().to_string())),
        None => say(lang, "tui.jotfile.nowhere").to_string(),
    };
    format!(
        "\n# {}\n# {}\n# {}\n# {}\n",
        say(lang, "tui.jotfile.how_title"),
        say(lang, "tui.jotfile.how_comments"),
        say(lang, "tui.jotfile.how_cancel"),
        fill(say(lang, "tui.jotfile.into"), &[("place", &place)]),
    )
}

/// 안내 주석인가. **`#` 뒤가 빈칸이거나 줄 끝일 때만이다** — `## 설계`·`#tag` 는 본문이다.
/// git 처럼 `#` 로 시작하는 줄을 다 걷으면 생각의 본문에 흔한 마크다운 제목이 말없이
/// 사라진다. 줄 가운데의 `#` 은 늘 글이다.
fn is_comment(line: &str) -> bool {
    line == "#" || line.starts_with("# ") || line.starts_with("#\t")
}

/// 편집기에서 받은 글 → (제목, 본문). **제목이 비면 `None`** — 담지 않는다.
///
/// - 줄바꿈은 `\r\n`·`\r`·`\n` 모두다(윈도 편집기가 남긴 `\r` 이 제목 끝에 붙지 않게)
/// - 안내 주석([`is_comment`])을 걷는다
/// - 앞 빈 줄을 건너 첫 줄이 제목이다. 앞뒤 빈칸을 뗀다 — CLI `add` 와 같다
/// - 그 뒤 **앞 빈 줄을 떼고 끝 빈칸을 뗀** 것이 본문이다. 사이의 빈 줄은 그대로다.
///   다 떼고 빈 글이면 본문이 없다
pub fn parse(text: &str) -> Option<(String, Option<String>)> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = text.split('\n').filter(|l| !is_comment(l)).skip_while(|l| l.trim().is_empty());
    let title = lines.next()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let rest: Vec<&str> = lines.skip_while(|l| l.trim().is_empty()).collect();
    let body = rest.join("\n");
    let body = body.trim_end();
    Some((title, (!body.is_empty()).then(|| body.to_string())))
}

/// 띄울 편집기. `$VISUAL` → `$EDITOR` → `vi` → `nano`, 아무것도 없으면 `None`(안의 폼).
///
/// 환경변수로 **준 것은 찾지 않고 믿는다** — git 과 같다. 인자가 붙은 것(`code --wait`)은
/// 찾을 방법부터 셸 규칙이라, 여기서 반쯤 흉내 내면 되는 것을 못 된다고 한다. 없는
/// 명령이면 셸이 127 로 끝나고 루프가 "담지 않았다" 로 말한다. 빈칸뿐인 값은 없는 것이다.
/// 기본 편집기 둘은 `found` 가 참일 때만 고른다 — PATH 를 읽는 것은 부르는 쪽이다.
pub fn pick(visual: Option<&str>, editor: Option<&str>, found: impl Fn(&str) -> bool) -> Option<String> {
    [visual, editor]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| ["vi", "nano"].into_iter().find(|n| found(n)).map(str::to_string))
}

/// 편집기를 띄우는 `sh` 의 인자. `sh -c '<편집기> "$@"' <편집기> <파일>` — git 의
/// `launch_editor` 와 같은 모양이다.
///
/// **파일 경로는 셸 글에 넣지 않고 위치 인자로 넘긴다.** 따옴표로 싸서 글에 넣으면 경로의
/// `"`·`$`·`` ` `` 를 다시 막아야 하고, 하나를 빠뜨리면 임시 경로가 명령이 된다. 편집기
/// 글은 셸 규칙으로 쪼갠다 — `EDITOR="code --wait"` 가 그대로 선다. 둘째 인자(`$0`)는
/// 셸이 오류를 댈 때 쓰는 이름이다.
pub fn argv(editor: &str, file: &std::path::Path) -> Vec<OsString> {
    vec!["-c".into(), format!("{editor} \"$@\"").into(), editor.into(), file.as_os_str().to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target { path: "/work/argos".into(), name: "argos".into() }
    }

    fn parsed(s: &str) -> Option<(String, Option<String>)> {
        parse(s)
    }

    fn some(title: &str, body: Option<&str>) -> Option<(String, Option<String>)> {
        Some((title.into(), body.map(str::to_string)))
    }

    /// 파일을 안 고치고 닫으면 **아무것도 안 담긴다** — 안내는 전부 주석이고 첫 줄은 비었다.
    /// 제목만 치고 닫으면 그것이 담긴다.
    #[test]
    fn the_template_is_all_comments_and_names_the_target() {
        let t = template(Some(&target()), crate::i18n::Lang::Ko);
        assert!(t.starts_with('\n'), "첫 줄이 비어 있지 않다 — {t:?}");
        assert!(t.contains("# 담을 곳: argos  /work/argos"), "{t}");
        assert_eq!(parsed(&t), None, "안 고친 글이 담긴다");
        assert_eq!(parsed(&format!("떠오른 것{t}")), some("떠오른 것", None));
        assert!(template(None, crate::i18n::Lang::Ko).contains("담기지 않는다"));
    }

    /// 남이 지은 이름에 줄바꿈이 들어도 **안내 줄이 주석 밖으로 새지 않는다.**
    #[test]
    fn a_target_name_with_a_newline_stays_a_comment() {
        let t = template(Some(&Target { path: "/a\nb".into(), name: "이름\n제목이 될 줄".into() }), crate::i18n::Lang::Ko);
        assert_eq!(parsed(&t), None, "{t}");
    }

    #[test]
    fn title_then_one_blank_line_then_body() {
        let t = template(Some(&target()), crate::i18n::Lang::Ko);
        assert_eq!(parsed(&format!("  제목  \n\n첫 줄\n\n셋째 줄\n{t}")), some("제목", Some("첫 줄\n\n셋째 줄")));
        // 빈 줄 없이 이어 써도 둘째 줄부터 본문이다
        assert_eq!(parsed("제목\n본문"), some("제목", Some("본문")));
        // 앞 빈 줄은 건너고, 본문 앞뒤 빈 줄·끝 빈칸은 뗀다. 본문 줄의 앞 빈칸은 둔다
        assert_eq!(parsed("\n\n제목\n\n\n  들여 쓴 줄  \n\n \n"), some("제목", Some("  들여 쓴 줄")));
    }

    /// **`# ` 와 `#` 만 주석이다.** 마크다운 제목(`##`)·태그(`#tag`)·줄 가운데의 `#`·
    /// 들여 쓴 `#` 은 글이다.
    #[test]
    fn only_hash_space_lines_are_comments() {
        let text = "제목 #1\n# 걷힌다\n#\n## 설계\n#tag\n  # 들여 씀\n값 # 가운데\n#\t탭도 걷힌다\n";
        assert_eq!(parsed(text), some("제목 #1", Some("## 설계\n#tag\n  # 들여 씀\n값 # 가운데")));
        // 주석이 제목과 본문 사이에 끼어도 빈 줄로 치지 않는다
        assert_eq!(parsed("# 안내\n제목\n# 안내\n\n본문"), some("제목", Some("본문")));
    }

    #[test]
    fn an_empty_title_is_nothing() {
        for s in ["", "\n", "   \n\t\n", "# 주석만\n#\n", "\r\n# 주석\r\n  \r\n"] {
            assert_eq!(parsed(s), None, "{s:?}");
        }
    }

    /// 윈도 줄바꿈이 제목·본문 끝에 `\r` 로 붙지 않는다. 옛 맥의 `\r` 하나도 줄바꿈이다.
    #[test]
    fn crlf_and_cr_are_newlines() {
        assert_eq!(parsed("제목\r\n\r\n본문 1\r\n본문 2\r\n# 주석\r\n"), some("제목", Some("본문 1\n본문 2")));
        assert_eq!(parsed("제목\r\r본문\r"), some("제목", Some("본문")));
    }

    #[test]
    fn visual_then_editor_then_vi_then_nano() {
        let all = |_: &str| true;
        let none = |_: &str| false;
        assert_eq!(pick(Some("code --wait"), Some("vim"), all).as_deref(), Some("code --wait"));
        assert_eq!(pick(None, Some("hx"), all).as_deref(), Some("hx"));
        // 빈 값은 없는 것이다
        assert_eq!(pick(Some("  "), Some(""), all).as_deref(), Some("vi"));
        assert_eq!(pick(None, None, |n| n == "nano").as_deref(), Some("nano"));
        assert_eq!(pick(None, None, none), None, "아무것도 없는데 골랐다");
        // 준 것은 찾지 않고 믿는다
        assert_eq!(pick(None, Some("없는-편집기"), none).as_deref(), Some("없는-편집기"));
    }

    /// 경로는 **셸 글이 아니라 위치 인자다** — 빈칸·따옴표·`$` 가 든 경로가 명령으로 안 풀린다.
    #[test]
    fn the_file_goes_as_a_positional_argument() {
        let file = std::path::Path::new("/tmp/a b/\"$(rm)\".md");
        let args = argv("code --wait", file);
        assert_eq!(args, ["-c", "code --wait \"$@\"", "code --wait", "/tmp/a b/\"$(rm)\".md"].map(OsString::from));
    }
}
