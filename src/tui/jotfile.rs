//! 생각 담기를 **외부 편집기로** 적을 때의 파일 글(moai-08af). git 이 커밋 메시지를 받는
//! 모양 그대로다 — 첫 줄이 제목, 한 줄 띄우고 본문, `#` 줄은 안내라 걷는다.
//!
//! **조각이다.** 환경변수도 파일도 프로세스도 모른다: 무엇을 쓸지(`template`), 받은 글을
//! 어떻게 읽을지(`parse`), 어느 편집기를 어떤 인자로 띄울지(`pick`·`argv`·`words`), 그 인자를 Git Bash 의
//! `sh` 에 어떻게 싸 넘길지(`msys_quoted`), 어느 낱말이 싸지 않아도 MSYS2 를 그대로 지나는지(`msys_word`)만 정한다.
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
        Some(t) => {
            format!("{}  {}", crate::text::one_line(&t.name), crate::text::one_line(&t.path.display().to_string()))
        }
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
/// 명령이면 셸이 127 로 끝나고(셸 없이 띄우는 판은 못 띄웠다는 오류다) 루프가 "담지 않았다" 로 말한다.
/// 빈칸뿐인 값은 없는 것이다.
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

/// Git for Windows 의 `sh` 에 넘길 낱말 하나를 **MSYS2 의 규칙으로** 싼다(moai-6kbo) — Git 의 `quote_arg_msys2`
/// (`compat/mingw.c`)를 옮긴 것이다. 러스트의 `Command` 는 낱말을 MSVC 의 규칙으로 싸는데, MSYS2 의 런타임은 명령
/// 줄을 제 규칙으로 다시 쪼개 `'` 를 따옴표로 먹고 `\\` 를 반으로 줄인다 — `C:\Users\o'brien\…` 의 임시 경로가
/// 없는 파일이 되어 적은 생각이 "제목이 비었다" 로 사라졌다. 그래서 부르는 쪽은 이것을 `raw_arg` 로 넘긴다.
///
/// - 빈칸(` `·`\t`·`\n`·`\r`)·`\`·`"`·`{`·`'`·`?`·`*`·`~` 가 없으면 그대로다. 있으면 큰따옴표로 싸고, 그 안의 `\` 와
///   `"` 앞에만 `\` 를 둔다. 빈칸은 git 의 `isspace` 넷이고 MSYS2 가 낱말을 가르는 글자도 그 넷이다 — 러스트의
///   `is_ascii_whitespace` 는 `\x0C` 를 더 쳐 git 과 답이 갈린다(리뷰 moai-j9gv.g3t)
/// - 빈 낱말은 `""` 다 — 안 싸면 낱말 하나가 사라진다
///
/// UTF-16 단위로 받는 까닭은 Windows 의 낱말이 그것이라서다(`encode_wide`) — 짝 없는 서로게이트도 그대로 지나고,
/// 리눅스에서는 `str::encode_utf16` 으로 잰다. 고르는 글자가 모두 ASCII 라 단위 하나씩 봐도 된다.
///
/// **큰따옴표 안의 `\"`·`\\` 를 푸는 것은 MSYS2 의 glob 이라, 이 꼴은 glob 이 그 낱말을 읽을 때만 선다**(리뷰
/// moai-j9gv.g3t). `MSYS=noglob` 이면 런타임이 따옴표만 걷고 `\` 를 안 풀어 `-c` 글이 깨지고, `LANG` 이 옛
/// 문자셋(`ja_JP.SJIS` 따위)인데 그 문자셋에 없는 글자가 든 낱말은 glob 이 못 읽어 따옴표째 넘어간다. `args` 가 싸던
/// MSVC 의 꼴(moai-6kbo 전)도 git 이 그 `sh` 에 넘기는 꼴도 같은 데서 깨진다 — 다만 빈칸 없는 임시 경로는 그때 안 싸여
/// 뒤의 것을 피했다.
#[cfg(any(not(unix), test))]
pub fn msys_quoted(arg: impl IntoIterator<Item = u16>) -> Vec<u16> {
    let arg: Vec<u16> = arg.into_iter().collect();
    let special = |u: u16| u8::try_from(u).is_ok_and(|b| MSYS2_SPECIAL.contains(&b) || b == b'~');
    if !arg.is_empty() && !arg.iter().any(|&u| special(u)) {
        return arg;
    }
    let mut out = Vec::with_capacity(arg.len() + 2);
    out.push(u16::from(b'"'));
    for u in arg {
        if u == u16::from(b'\\') || u == u16::from(b'"') {
            out.push(u16::from(b'\\'));
        }
        out.push(u);
    }
    out.push(u16::from(b'"'));
    out
}

/// MSYS2 의 런타임에 넘길 낱말에서 [`msys_quoted`] 가 쓰는 git 의 목록과 [`msys_word`] 가 쓰는 잰 목록이 함께 고르는
/// 글자 — 빈칸 넷(git 의 `isspace`)·`\`·`"`·`{`·`'`·`?`·`*`. 앞의 것은 여기에 `~` 를 더하고 뒤의 것은 `[` 를 더한다.
/// 둘이 따로 적으면 한쪽만 고쳐진다(리뷰 moai-j9gv.8oh).
#[cfg(any(not(unix), test))]
const MSYS2_SPECIAL: &[u8] = b" \t\n\r\\\"{'?*";

/// 이 낱말이 **싸지 않아도 MSYS2 의 런타임을 그대로 지나는가**(moai-l3pp). 거기서 지어진 프로그램(Git 의 `sh.exe`·
/// `nano.exe`)은 명령 줄을 제 규칙으로 다시 쪼갠다. 2026-10-12 MinGit 2.56 의 `sh.exe` 로 wine 에서 잰 것이다.
///
/// - 어디 있든 바뀌는 글자 — 빈칸·`'`·`"`·`{`(따옴표와 중괄호를 먹는다), `*`·`?`·`[`(맞는 파일이 있으면 펼친다), `\`(glob
///   이 그 낱말을 읽으면 둘을 하나로 준다 — 드라이브·UNC 꼴은 피하지만 가리지 않고 막는다)
/// - 맨 앞에서만 바뀌는 글자 — `~`(그 자리가 있으면 집 디렉터리로 펼친다), `@`(그 이름의 파일이 있으면 그 글로 바뀐다).
///   빈 낱말은 사라진다. 처음 잰 `~/x` 는 없는 자리라 지났다(리뷰 moai-j9gv.8oh)
/// - 그대로 지난 것 — 가운데의 `~`(8.3 짧은 이름의 `JOHNSM~1`)·한글·`$`·`%`·`&`·`(`·`;`·`]`·`}`·`^`·맨 앞의 `-`
///
/// 편집기에 넘길 임시 경로는 이것이 참인 자리에서 고른다 — 그 낱말은 MSVC 의 꼴로도 MSYS2 의 꼴로도 안 싸여, 어느 편집기가
/// 받든 옛 `LANG` 문자셋에서든 그대로 닿는다(2026-10-12 사람 결정). [`msys_quoted`] 와 갈리는 것은 가운데의 `~`(그쪽은
/// 싼다)와 `[`·맨 앞의 `@`(그쪽은 안 싼다)다 — 그쪽은 git 과 같은 답을 내려고 git 의 목록을 그대로 쓴다.
#[cfg(any(not(unix), test))]
pub fn msys_word(arg: impl IntoIterator<Item = u16>) -> bool {
    let mut arg = arg.into_iter().peekable();
    arg.peek().is_some_and(|&u| u != u16::from(b'~') && u != u16::from(b'@'))
        && !arg.any(|u| u8::try_from(u).is_ok_and(|b| MSYS2_SPECIAL.contains(&b) || b == b'['))
}

/// `sh` 없이 띄우는 데서 편집기 글을 쪼갠 것 — 첫 낱말과 나머지(moai-j9gv.cl6). **빈칸으로 쪼갠다** —
/// 따옴표를 풀려면 껍데기 규칙을 반쯤 흉내 내야 하고([`pick`] 이 안 하는 까닭과 같다), 빈칸이 든
/// 자리의 편집기는 `PATH` 에 그 디렉터리를 두면 이름 하나로 선다. 빈 글이면 첫 낱말도 빈 글이다.
///
/// **하나만 예외다 — 큰따옴표로 싼 첫 낱말은 다음 큰따옴표까지가 프로그램이다**(리뷰 moai-j9gv.4s3). Windows 가
/// 명령 줄에서 프로그램 이름을 읽는 규칙이고, 편집기가 사는 `C:\Program Files\…` 를 적는 흔한 꼴이다
/// (`"C:\Program Files\Notepad++\notepad++.exe" -multiInst`). `"` 는 파일 이름에 못 들어, 지금 못 뜨는 글만 바뀐다.
/// 닫는 따옴표가 없으면 예외가 아니다.
#[cfg(any(not(unix), test))]
pub fn words(editor: &str) -> (&str, Vec<&str>) {
    if let Some((program, rest)) = editor.trim_start().strip_prefix('"').and_then(|r| r.split_once('"')) {
        return (program, rest.split_whitespace().collect());
    }
    let mut all = editor.split_whitespace();
    (all.next().unwrap_or(""), all.collect())
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
        let t =
            template(Some(&Target { path: "/a\nb".into(), name: "이름\n제목이 될 줄".into() }), crate::i18n::Lang::Ko);
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

    /// `sh` 없이 띄우는 데서는 빈칸으로 쪼갠다 — 첫 낱말이 프로그램, 나머지가 인자다(moai-j9gv.cl6). 큰따옴표로 싼
    /// 첫 낱말만 통째로 프로그램이다.
    #[test]
    fn without_sh_the_editor_splits_on_blanks() {
        assert_eq!(words("code --wait"), ("code", vec!["--wait"]));
        assert_eq!(words("  notepad  "), ("notepad", vec![]));
        assert_eq!(words("vim -u NONE -n"), ("vim", vec!["-u", "NONE", "-n"]));
        assert_eq!(words(""), ("", vec![]));
        // 큰따옴표로 싼 첫 낱말은 다음 큰따옴표까지가 프로그램이다 — 빈칸이 든 자리를 그대로 적은 꼴이다(리뷰 moai-j9gv.4s3).
        let quoted = r#""C:\Program Files\Notepad++\notepad++.exe" -multiInst -nosession"#;
        assert_eq!(words(quoted), (r"C:\Program Files\Notepad++\notepad++.exe", vec!["-multiInst", "-nosession"]));
        assert_eq!(words(r#"  "C:\a b\ed.exe""#), (r"C:\a b\ed.exe", vec![]));
        // 닫는 따옴표가 없으면 예외가 아니다 — 빈칸으로 쪼갠다.
        assert_eq!(words(r#""C:\a b\ed.exe -w"#), (r#""C:\a"#, vec![r"b\ed.exe", "-w"]));
    }

    /// **Git Bash 의 `sh` 에는 MSYS2 의 규칙으로 싼다**(moai-6kbo) — Git 의 `quote_arg_msys2` 와 같은 답이다. `'` 든 임시
    /// 경로가 큰따옴표 안에 들어 그 `sh` 가 따옴표로 안 먹고, `\` 는 둘로 적혀 반으로 줄어도 하나가 남는다.
    #[test]
    fn the_msys_sh_gets_its_own_quoting() {
        let q = |s: &str| String::from_utf16(&msys_quoted(s.encode_utf16())).unwrap();
        assert_eq!(q("-c"), "-c", "쌀 것 없는 낱말을 쌌다");
        assert_eq!(q("code"), "code");
        assert_eq!(q(""), r#""""#, "빈 낱말이 사라진다");
        assert_eq!(q(r#"code --wait "$@""#), r#""code --wait \"$@\"""#);
        assert_eq!(
            q(r"C:\Users\o'brien\AppData\Local\Temp\moai-backlog.md"),
            r#""C:\\Users\\o'brien\\AppData\\Local\\Temp\\moai-backlog.md""#,
            "`'` 든 임시 경로를 그 `sh` 가 쪼갠다"
        );
        assert_eq!(q(r"C:/a\\b"), r#""C:/a\\\\b""#, "`\\\\` 가 반으로 줄어 하나가 된다");
        for glob in ["~/x", "a?b", "a*b", "{a,b}"] {
            assert_eq!(q(glob), format!("\"{glob}\""), "{glob}: 그 `sh` 가 펼친다");
        }
        // 고르는 글자 하나만 든 낱말도 싼다 — 위의 경로·편집기 글은 `\`·빈칸이 함께 들어 `'`·`"`·빈칸 하나를 빼도 붉어지지
        // 않는다. 빈칸은 git 의 `isspace` 넷이고, `\x0C`·`\x0B` 는 MSYS2 가 낱말을 안 가르는 글자라 git 처럼 안 싼다(리뷰
        // moai-j9gv.g3t).
        for one in ["a b", "a\tb", "a\nb", "a\rb", "a'b"] {
            assert_eq!(q(one), format!("\"{one}\""), "{one:?}: 그 `sh` 가 쪼갠다");
        }
        assert_eq!(q(r#"a"b"#), r#""a\"b""#);
        assert_eq!(q("a\x0Cb\x0Bc"), "a\x0Cb\x0Bc", "git 이 안 싸는 글자를 쌌다");
        // 끝의 `\` 도 둘로 적는다 — 닫는 따옴표를 먹지 않는다.
        assert_eq!(q(r"C:\dir\"), r#""C:\\dir\\""#);
        // 짝 없는 서로게이트(Windows 의 낱말에 설 수 있다)는 단위째 지나간다.
        assert_eq!(msys_quoted([0xD800, u16::from(b'a')]), [0xD800, u16::from(b'a')]);
        assert_eq!(msys_quoted([0xD800, u16::from(b' ')]), [34, 0xD800, 32, 34]);
    }

    /// **싸지 않아도 MSYS2 를 그대로 지나는 낱말**(moai-l3pp) — 2026-10-12 MinGit 의 `sh.exe` 로 잰 표다. 고르는 글자는
    /// 하나씩 잰다 — 하나를 빼도 붉어진다. 맨 앞의 `~`·`@` 는 그 자리·파일이 있으면 바뀌고 빈 낱말은 사라진다(리뷰
    /// moai-j9gv.8oh). `/` 로 쓴 임시 경로와 8.3 짧은 이름의 `~` 는 지난다.
    #[test]
    fn some_words_pass_msys2_unquoted() {
        let w = |s: &str| msys_word(s.encode_utf16());
        for passes in [
            "C:/Users/OBRIEN~1/AppData/Local/Temp/moai-backlog-1-0.md",
            "//srv/share/T/moai-backlog-1-0.md",
            "moai-backlog-1-0.md",
            "C:/Users/레이븐/T",
            "a$b%c&d(e);f",
            "C:/a]b}c^d",
            "-x",
            "x@y",
        ] {
            assert!(w(passes), "{passes}: 지나는 낱말을 막았다");
        }
        for fails in [
            "a b", "a\tb", "a\nb", "a\rb", "o'brien", "a\"b", "a{b}", r"C:\x", "a*b", "a?b", "a[b]", "~/x", "~",
            "@resp", "",
        ] {
            assert!(!w(fails), "{fails:?}: MSYS2 가 바꾸는 낱말을 그대로 넘긴다");
        }
        // 짝 없는 서로게이트는 단위째 본다 — 고르는 글자가 모두 ASCII 다.
        assert!(msys_word([0xD800, u16::from(b'a')]));
        assert!(!msys_word([0xD800, u16::from(b' ')]));
    }
}
