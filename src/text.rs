//! 글자 폭을 다루는 순수 유틸. **이슈의 뜻을 모른다.**
//!
//! 한글은 터미널에서 두 칸을 먹는다. `len()`·`chars().count()` 로 맞추면
//! 한글 제목이 섞인 표가 전부 어긋난다. CLI 표(`view`)와 TUI 가 같은 자를
//! 써야 두 표면이 같은 자리에서 잘린다.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// 표시 폭 기준으로 자르고 `…` 를 붙인다.
///
/// **`max` 가 0이면 빈 글이다.** `…` 한 칸도 상한을 넘기는 것이고, 폭을 빼서
/// 몫을 구하는 쪽은 0을 곧잘 건네므로 여기서 한 칸을 넘기면 그 줄이 테두리를
/// 넘는다 — 넘친 줄은 위젯이 말없이 잘라 내서 잘렸다는 표시마저 사라진다.
pub fn clip(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    // 쌓인 것을 들고 간다. 줄마다 `width(&out)` 을 다시 재면 글자 수의 제곱이
    // 되고, 이것은 줄마다·프레임마다 돈다.
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > max - 1 {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// 진행 막대에서 채울 칸 수. **CLI 와 TUI 가 같은 자를 쓴다** — 갈라지면
/// 같은 에픽이 두 표면에서 다른 진척으로 보인다.
///
/// **양 끝에서 거짓말하지 않는다.** 1% 는 한 칸이 보여야 하고(`div_ceil`),
/// 99% 는 꽉 차면 안 된다 — 꽉 찬 막대는 끝난 것을 뜻하고, 안 끝난 것을
/// 끝난 것으로 보이게 하는 쪽이 그 반대보다 비싸다.
pub fn bar_fill(percent: Option<u8>, cells: usize) -> usize {
    match percent {
        None => 0,
        Some(p) if p >= 100 => cells,
        Some(p) => (p as usize * cells).div_ceil(100).min(cells.saturating_sub(1)),
    }
}

/// 화면에 그리기 전에 제어문자를 걷어낸다.
///
/// 본문은 손으로 고칠 수 있는 파일에서 온다. ESC 가 든 줄을 그대로 그리면
/// **그 줄이 화면을 다시 칠한다** — 커서를 옮기고 색을 바꾸고 지운다.
/// 읽기는 관대하되 그리기는 엄해야 하는 자리다. 줄바꿈과 탭은 남긴다.
pub fn sanitize(s: &str) -> String {
    s.chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).collect()
}

/// 앞을 잘라 **뒤를 남긴다** — `…/apps/a`. 경로처럼 뒤가 값진 글에 쓴다.
///
/// [`clip`] 과 한 자로 잰다(같은 `…`, 같은 0 규칙, 같은 글자 단위 폭). 따로 두면 한쪽만
/// 고쳐져 같은 폭에서 두 표면이 달리 잘린다 — 그 둘을 한자리에 두려고 있는 모듈이다.
pub fn clip_front(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut back: Vec<char> = Vec::new();
    let mut used = 1; // `…`
    for c in s.chars().rev() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > max {
            break;
        }
        used += w;
        back.push(c);
    }
    std::iter::once('…').chain(back.into_iter().rev()).collect()
}

/// 한 줄에 세우는 글 — 제어문자를 걷고 여러 줄이면 사이를 두 칸으로 접는다.
///
/// **[`sanitize`] 는 줄바꿈과 탭을 남긴다** — 본문은 여러 줄이 정상이라서다. 한 줄짜리
/// 자리(목록의 한 줄·배너·창의 아랫줄)에 그대로 쓰면 뒤가 다음 줄로 흘러 옆 항목의
/// 줄과 갈리지 않는다. 그 자리는 전부 이것을 지난다 — 자리마다 따로 접으면 한쪽은
/// 접고 한쪽은 안 접어 같은 글이 화면마다 달리 선다.
///
/// **이미 한 줄이면 앞뒤를 안 깎는다.** 경로·이름도 이것을 지나는데(`a<LF>b` 라는
/// 디렉터리도 등록된다) 끝에 공백이 든 디렉터리 이름을 깎으면 다른 디렉터리를 보인다.
/// 여러 줄일 때만 줄마다 깎아 잇는다 — 들여 쓴 둘째 줄이 달린 오류 말의 모양이다.
pub fn one_line(s: &str) -> String {
    if !s.contains('\n') {
        // `\r` 같은 제어문자는 `sanitize` 가 걷는다. 남는 것은 탭뿐이다.
        return sanitize(s).replace('\t', " ");
    }
    let joined = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("  ");
    sanitize(&joined).replace('\t', " ")
}

/// 붙여 넣어 그대로 돌 수 있게 감싼다. 공백이나 껍데기가 뜻을 붙이는 글자가
/// 있으면 작은따옴표로 — 안 감싸면 `~/My Projects/argos` 가 두 인자로 갈라진다.
///
/// **안내에 경로를 넣는 곳은 이것을 지난다** (`project add`·한눈 보기). 자리마다
/// 따로 두면 한쪽은 안전한 글자를, 한쪽은 위험한 글자를 세어 같은 경로를 달리 감싼다.
///
/// **`-` 로 시작하는 것은 감싼다.** 글자 자체는 껍데기에 뜻이 없지만, 안내가 내는 것은
/// 명령줄이라 맨 앞의 `-` 는 그것을 **인자가 아니라 플래그로** 만든다 — `-rf` 라는
/// 디렉터리의 `moai project rm -rf` 는 clap 이 플래그로 읽고 경로는 없다고 한다.
///
/// **원문을 받는다 — [`one_line`] 을 지난 글을 넣지 않는다.** `one_line` 은 화면용이라
/// 탭·줄바꿈을 빈칸으로 바꾸고, 그것을 감싸 붙여 넣으면 없는 디렉터리를 가리킨다
/// (moai-0cl3). 제어문자가 든 글은 `$'…'` 로 바이트 그대로 적는다 — 한 줄에 서고
/// 터미널에 날것의 제어문자를 흘리지 않으면서 bash·zsh 가 원문으로 푼다.
pub fn shell_word(s: &str) -> String {
    if s.chars().any(char::is_control) {
        let mut out = String::from("$'");
        for c in s.chars() {
            match c {
                '\\' => out.push_str(r"\\"),
                '\'' => out.push_str(r"\'"),
                '\n' => out.push_str(r"\n"),
                '\t' => out.push_str(r"\t"),
                // 바이트마다 `\xHH` 다. `\u` 는 로캘을 타고, C1(U+0080~)을 `\xHH` 한 자로 적으면
                // UTF-8 두 바이트가 아니라 날 바이트 하나가 된다.
                c if c.is_control() => {
                    let mut buf = [0u8; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        out.push_str(&format!(r"\x{b:02x}"));
                    }
                }
                c => out.push(c),
            }
        }
        out.push('\'');
        return out;
    }
    // `=` 는 첫 낱말이 아니면 껍데기에 뜻이 없다 — 안내가 내는 경로는 늘 인자 자리다.
    let plain = !s.is_empty()
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | ',' | ':' | '@' | '%' | '='));
    if plain { s.to_string() } else { format!("'{}'", s.replace('\'', r"'\''")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_word_quotes_only_what_the_shell_would_split() {
        assert_eq!(shell_word("/home/raven/work/argos"), "/home/raven/work/argos");
        assert_eq!(shell_word("/home/raven/작업/argos"), "/home/raven/작업/argos");
        assert_eq!(shell_word("/home/raven/My Projects"), "'/home/raven/My Projects'");
        assert_eq!(shell_word("/a/it's"), r"'/a/it'\''s'");
        // 맨 앞의 `-` 는 안내가 낸 명령줄에서 플래그가 된다.
        assert_eq!(shell_word("-rf"), "'-rf'");
        assert_eq!(shell_word("--json"), "'--json'");
        assert_eq!(shell_word("/w/my-repo"), "/w/my-repo", "가운데 `-` 까지 감쌌다");
        assert_eq!(shell_word("/w/a=b"), "/w/a=b");
        // 제어문자는 `$'…'` 로 — 빈칸으로 바꾸면 다른 디렉터리다(moai-0cl3).
        assert_eq!(shell_word("/w/a\tb"), r"$'/w/a\tb'");
        assert_eq!(shell_word("/w/it's\nx"), r"$'/w/it\'s\nx'");
        assert_eq!(shell_word("/w/\u{1b}[2J"), r"$'/w/\x1b[2J'");
        assert_eq!(shell_word("/w/\u{85}"), r"$'/w/\xc2\x85'");
        assert!(!shell_word("/w/a\nb").chars().any(char::is_control), "한 줄 자리에 제어문자가 샜다");
    }

    /// **감싼 것을 껍데기가 풀면 원문이다.** 모양만 보는 단언은 따옴표 규칙을 잘못 외워도
    /// 통과한다 — 실제 bash 에 돌려 본다. bash 가 없는 기계에서는 건너뛴다.
    #[test]
    fn shell_word_round_trips_through_bash() {
        let Ok(probe) = std::process::Command::new("bash").arg("-c").arg("true").status() else { return };
        if !probe.success() {
            return;
        }
        for s in ["/w/My Projects", "/w/it's", "-rf", "/w/a\tb", "/w/a\nb", "/w/\u{1b}[2J", "/w/\u{85}끝 ", "/w/\\x41", "/w/$HOME", "/w/`id`"] {
            let out = std::process::Command::new("bash")
                .arg("-c")
                .arg(format!("printf %s {}", shell_word(s)))
                .output()
                .unwrap();
            assert_eq!(String::from_utf8(out.stdout).unwrap(), s, "{:?} 를 bash 가 다르게 풀었다", shell_word(s));
        }
    }

    /// 한 줄짜리 자리에 여러 줄이 흘러들지 않는다.
    #[test]
    fn one_line_folds_every_break_and_drops_control_characters() {
        assert_eq!(one_line("한 줄"), "한 줄");
        assert_eq!(one_line("첫 줄\n  둘째 줄  \n\n셋째"), "첫 줄  둘째 줄  셋째");
        assert_eq!(one_line("탭\t섞임"), "탭 섞임");
        assert_eq!(one_line("지움\u{1b}[2J"), "지움[2J");
        // 한 줄이면 앞뒤를 안 깎는다 — 끝에 공백이 든 디렉터리 이름이 다른 이름으로 보이면 안 된다.
        assert_eq!(one_line("/w/끝에 공백 "), "/w/끝에 공백 ");
        // 줄바꿈이 든 경로도 한 줄로 선다.
        assert_eq!(one_line("/w/a\nb"), "/w/a  b");
    }

    #[test]
    fn korean_counts_two_columns() {
        assert_eq!(width("abc"), 3);
        assert_eq!(width("한글"), 4);
        assert_eq!(width("한a"), 3);
    }

    /// 자른 것의 폭이 상한을 넘지 않는다 — 한 칸이라도 넘으면 표가 접힌다.
    /// **0 도 센다.** 몫을 빼서 구하는 쪽이 0을 건네고, 거기서 한 칸이 새면
    /// 줄이 테두리를 넘어 위젯이 말없이 잘라 낸다.
    #[test]
    fn clipping_never_exceeds_the_budget() {
        for s in ["짧다", "아주 긴 한글 제목이 여기 들어간다", "a very long ascii title here"] {
            for max in 0..20 {
                assert!(width(&clip(s, max)) <= max, "{s:?} @ {max} → {:?}", clip(s, max));
            }
        }
        assert_eq!(clip("짧다", 10), "짧다");
        assert_eq!(clip("짧다", 0), "");
    }

    /// 앞을 자르는 쪽도 같은 자로 잰다 — 상한을 넘지 않고 뒤를 남긴다.
    #[test]
    fn clipping_the_front_keeps_the_tail_within_the_budget() {
        for s in ["/w/one", "/home/raven/작업/아주/깊은/경로/여기", "짧다"] {
            for max in 0..20 {
                let got = clip_front(s, max);
                assert!(width(&got) <= max, "{s:?} @ {max} → {got:?}");
            }
        }
        assert_eq!(clip_front("/w/one", 10), "/w/one");
        assert_eq!(clip_front("/w/apps/a", 7), "…apps/a");
        assert_eq!(clip_front("/w/one", 0), "");
    }

    /// 0% 와 "멤버 없음" 은 다르다. 100% 만 꽉 차고, 1% 도 한 칸은 보인다.
    #[test]
    fn the_bar_never_lies_at_the_ends() {
        assert_eq!(bar_fill(None, 10), 0);
        assert_eq!(bar_fill(Some(0), 10), 0);
        assert_eq!(bar_fill(Some(1), 10), 1, "조금 한 것이 안 한 것처럼 보인다");
        assert_eq!(bar_fill(Some(100), 10), 10);
        assert_eq!(bar_fill(Some(99), 10), 9, "거의 다 한 것이 다 한 것처럼 보인다");
        assert!(bar_fill(Some(200), 10) <= 10, "칸을 넘겼다");
        assert_eq!(bar_fill(Some(50), 0), 0, "칸이 없으면 채울 것도 없다");
    }

    /// 파일에 든 ESC 가 화면을 다시 칠하게 두지 않는다.
    #[test]
    fn control_characters_never_reach_the_screen() {
        assert_eq!(sanitize("보통 글"), "보통 글");
        assert_eq!(sanitize("줄\n바꿈\t탭"), "줄\n바꿈\t탭");
        assert_eq!(sanitize("여기\u{1b}[2J지움"), "여기[2J지움");
        assert_eq!(sanitize("벨\u{7}"), "벨");
    }
}
