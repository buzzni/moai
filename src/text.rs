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
}
