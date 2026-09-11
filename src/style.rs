//! 색과 글리프.
//!
//! **철칙: 색이 혼자 뜻을 지지 않는다.** 모든 색에 글리프나 낱말이 붙는다.
//! `--no-color` 로도, 색맹인 눈으로도 같은 정보가 읽혀야 한다.
//!
//! 기본 16색만 쓴다. 256색·truecolor 를 쓰면 사용자의 터미널이 밝은 배경인지
//! 어두운 배경인지 모르는 채로 색을 고르게 된다. 기본색은 테마가 대신 고른다.

use anstyle::{AnsiColor, Color, Style};

const fn fg(c: AnsiColor) -> Style {
    Style::new().fg_color(Some(Color::Ansi(c)))
}

/// 다수파는 칠하지 않는다 — 다수파를 칠하면 신호가 죽는다.
pub const PLAIN: Style = Style::new();
pub const DIM: Style = Style::new().dimmed();

pub const TODO: Style = PLAIN;
pub const IN_PROGRESS: Style = fg(AnsiColor::BrightYellow).bold();
/// **가장 눈에 띄어야 한다** — 게이트를 없앤 대신 여기가 썩는다.
pub const REVIEW: Style = fg(AnsiColor::BrightMagenta);
pub const DONE: Style = fg(AnsiColor::Green).dimmed();
pub const OTHER: Style = fg(AnsiColor::Cyan);

pub const EPIC: Style = fg(AnsiColor::BrightBlue).bold();
pub const TAG: Style = fg(AnsiColor::Cyan).dimmed();
/// 목록의 에픽 열. 곁다리라 에픽 제목 자체보다 약하게.
pub const EPIC_REF: Style = fg(AnsiColor::BrightBlue).dimmed();
pub const ID: Style = DIM;
pub const ERROR: Style = fg(AnsiColor::Red);
pub const WARN: Style = fg(AnsiColor::BrightYellow);
pub const HEAD: Style = Style::new().bold();

pub const P0: Style = fg(AnsiColor::BrightRed).bold();
pub const P1: Style = fg(AnsiColor::Red);

/// 칸 이름 → 글리프. 모르는 칸도 글리프를 갖는다.
pub fn glyph(status: &str) -> &'static str {
    match status {
        "todo" => "·",
        "in_progress" => "▸",
        "review" => "?",
        "done" => "✓",
        _ => "○",
    }
}

pub fn status_style(status: &str) -> Style {
    match status {
        "todo" => TODO,
        "in_progress" => IN_PROGRESS,
        "review" => REVIEW,
        "done" => DONE,
        _ => OTHER,
    }
}

/// 기본값(p2)은 칠하지 않는다. 기본값을 칠하면 아무 뜻이 없다.
pub fn priority_style(p: u8) -> Style {
    match p {
        0 => P0,
        1 => P1,
        3 => DIM,
        _ => PLAIN,
    }
}

/// 칠한 글자. 색을 끄는 판단은 anstream 이 출력 시점에 한다.
pub fn paint(style: Style, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    if style == Style::new() {
        return text.to_string(); // 안 칠할 것에 이스케이프를 붙이지 않는다
    }
    format!("{}{text}{}", style.render(), anstyle::Reset.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 칸마다 글리프가 다르다 — 색을 꺼도 구별돼야 하기 때문이다.
    #[test]
    fn every_column_has_its_own_glyph() {
        let g: Vec<&str> = ["todo", "in_progress", "review", "done"].iter().map(|s| glyph(s)).collect();
        let uniq: std::collections::BTreeSet<_> = g.iter().collect();
        assert_eq!(uniq.len(), g.len(), "{g:?}");
        assert_eq!(glyph("설정으로_더한_칸"), "○");
    }

    #[test]
    fn paint_wraps_and_resets() {
        let s = paint(REVIEW, "x");
        assert!(s.starts_with('\u{1b}') && s.ends_with("\u{1b}[0m"), "{s:?}");
        assert!(s.contains('x'));
        assert_eq!(paint(REVIEW, ""), "");
    }

    /// 기본 우선순위는 안 칠한다.
    #[test]
    fn default_priority_is_unstyled() {
        assert_eq!(paint(priority_style(2), "p2"), "p2");
        assert_ne!(paint(priority_style(0), "p0"), "p0");
    }
}
