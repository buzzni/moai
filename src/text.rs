//! 글자 폭을 다루는 순수 유틸. **이슈의 뜻을 모른다.**
//!
//! 한글은 터미널에서 두 칸을 먹는다. `len()`·`chars().count()` 로 맞추면
//! 한글 제목이 섞인 표가 전부 어긋난다. CLI 표(`view`)와 TUI 가 같은 자를
//! 써야 두 표면이 같은 자리에서 잘린다.

use unicode_width::UnicodeWidthStr;

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// 표시 폭 기준으로 자르고 `…` 를 붙인다.
pub fn clip(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        let w = UnicodeWidthStr::width(c.encode_utf8(&mut [0u8; 4]) as &str);
        if width(&out) + w > max.saturating_sub(1) {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// 진행 막대에서 채울 칸 수. **CLI 와 TUI 가 같은 자를 쓴다** — 갈라지면
/// 같은 에픽이 두 표면에서 다른 진척으로 보인다.
pub fn bar_fill(percent: Option<u8>, cells: usize) -> usize {
    match percent {
        None => 0,
        Some(p) => (p as usize * cells).div_ceil(100).min(cells),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_counts_two_columns() {
        assert_eq!(width("abc"), 3);
        assert_eq!(width("한글"), 4);
        assert_eq!(width("한a"), 3);
    }

    /// 자른 것의 폭이 상한을 넘지 않는다 — 한 칸이라도 넘으면 표가 접힌다.
    #[test]
    fn clipping_never_exceeds_the_budget() {
        for s in ["짧다", "아주 긴 한글 제목이 여기 들어간다", "a very long ascii title here"] {
            for max in 1..20 {
                assert!(width(&clip(s, max)) <= max, "{s:?} @ {max} → {:?}", clip(s, max));
            }
        }
        assert_eq!(clip("짧다", 10), "짧다");
    }

    /// 0% 와 "멤버 없음" 은 다르다. 100% 는 꽉 차고, 1% 도 한 칸은 보인다.
    #[test]
    fn the_bar_never_lies_at_the_ends() {
        assert_eq!(bar_fill(None, 10), 0);
        assert_eq!(bar_fill(Some(0), 10), 0);
        assert_eq!(bar_fill(Some(1), 10), 1, "조금 한 것이 안 한 것처럼 보인다");
        assert_eq!(bar_fill(Some(100), 10), 10);
        assert_eq!(bar_fill(Some(99), 10), 10.min(bar_fill(Some(99), 10)));
        assert!(bar_fill(Some(200), 10) <= 10, "칸을 넘겼다");
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
