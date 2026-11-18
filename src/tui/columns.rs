//! Cutting a line of foreign text to the columns on screen (moai-ffk9.3fd) — the file pane's horizontal scroll, and the
//! diff pane's (moai-ffk9.41b) after it.
//!
//! **Columns are screen cells**, not bytes and not chars: a Hangul syllable takes two, a tab stretches to the next stop
//! ([`TAB`]). The colouring comes as byte ranges of the text *before* the tabs are spread (`highlight::Span`), so the cut
//! walks the text once, keeping both the byte index (for the colour) and the column (for the cut) — mixing the two
//! shifts the colour by the tabs' width.
//!
//! **Nothing raw reaches the terminal.** A control character (ESC, CR, …) is drawn as `?` in one cell: drawn as-is the
//! terminal would run it, dropped it would make two different lines look alike. A wide character cut by either edge
//! becomes blanks for the cells that stand, so nothing after it shifts. A zero-width character at the left edge is
//! dropped — its base is hidden and it would join whatever stands in the cell before.
//!
//! **Pure** — text in, spans out; the drawing calls it for the lines on screen only.

use ratatui::style::Style;
use ratatui::text::Span;
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

/// A tab reaches the next multiple of this column.
pub const TAB: usize = 4;

/// The cells a char takes when it starts at column `col`.
fn cells(c: char, col: usize) -> usize {
    match c {
        '\t' => TAB - col % TAB,
        c if c.is_control() => 1,
        c => c.width().unwrap_or(1),
    }
}

/// The width of a line in screen cells, tabs spread.
pub fn width(text: &str) -> usize {
    text.chars().fold(0, |col, c| col + cells(c, col))
}

/// The cells `from..from + room` of `text`. `paint` gives byte ranges of `text` in order (they may stop short of the
/// end and leave gaps); what no range covers takes `base`. A range past the end of the text is cut to it.
pub fn cut(text: &str, paint: &[(Range<usize>, Style)], base: Style, from: usize, room: usize) -> Vec<Span<'static>> {
    let end = from + room;
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut push = |s: &str, style: Style| match out.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(s),
        _ => out.push(Span::styled(s.to_string(), style)),
    };
    let mut ranges = paint.iter().peekable();
    let mut col = 0usize;
    let mut buf = [0u8; 4];
    for (at, c) in text.char_indices() {
        if col >= end {
            break;
        }
        while ranges.next_if(|(r, _)| r.end <= at).is_some() {}
        let style = match ranges.peek() {
            Some((r, s)) if r.contains(&at) => *s,
            _ => base,
        };
        let w = cells(c, col);
        let (start, stop) = (col, col + w);
        col = stop;
        if w == 0 {
            if start > from && start < end {
                push(c.encode_utf8(&mut buf), style);
            }
            continue;
        }
        if stop <= from {
            continue;
        }
        let whole = start >= from && stop <= end;
        if whole && c != '\t' {
            let glyph = if c.is_control() { "?" } else { c.encode_utf8(&mut buf) };
            push(glyph, style);
        } else {
            // A tab, or a wide char cut by an edge: blanks for the cells that stand.
            let shown = stop.min(end) - start.max(from);
            push(&" ".repeat(shown), style);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn red() -> Style {
        Style::new().fg(Color::Red)
    }

    #[test]
    fn tabs_reach_the_next_stop() {
        assert_eq!(width("\tx"), 5);
        assert_eq!(width("ab\tx"), 5);
        assert_eq!(width("abcd\tx"), 9);
        assert_eq!(text(&cut("a\tb", &[], Style::new(), 0, 10)), "a   b");
    }

    /// **The colour follows the bytes through the tabs** — a span after a tab lands on its own letters.
    #[test]
    fn colour_after_a_tab_stays_on_its_letters() {
        let line = "\tlet x";
        let paint = [(1..4, red())];
        let got = cut(line, &paint, Style::new(), 0, 20);
        assert_eq!(got, vec![Span::raw("    "), Span::styled("let", red()), Span::raw(" x")]);
        let panned = cut(line, &paint, Style::new(), 5, 20);
        assert_eq!(panned, vec![Span::styled("et", red()), Span::raw(" x")]);
    }

    /// **A wide char cut by an edge leaves blanks**, so nothing after it moves and no half glyph stands.
    #[test]
    fn a_cut_wide_char_becomes_blanks() {
        let line = "가나다라";
        assert_eq!(text(&cut(line, &[], Style::new(), 1, 4)), " 나 ");
        assert_eq!(text(&cut(line, &[], Style::new(), 2, 4)), "나다");
        assert_eq!(width(&text(&cut(line, &[], Style::new(), 1, 5))), 5);
    }

    /// **Colour on Hangul uses byte ranges**: the comment span after a Hangul word starts where its bytes do.
    #[test]
    fn colour_on_hangul_keeps_byte_ranges() {
        let line = "한글 // 주석";
        let at = line.find("//").unwrap();
        let paint = [(at..line.len(), red())];
        let got = cut(line, &paint, Style::new(), 0, 40);
        assert_eq!(got, vec![Span::raw("한글 "), Span::styled("// 주석", red())]);
        // `글` is cut by the left edge (a blank in its own colour), then the space and `//` in theirs.
        let panned = cut(line, &paint, Style::new(), 3, 4);
        assert_eq!(panned, vec![Span::raw("  "), Span::styled("//", red())]);
    }

    /// **Control characters never reach the terminal** — ESC and CR stand as `?`.
    #[test]
    fn control_characters_stand_as_question_marks() {
        let got = text(&cut("a\u{1b}[31mb\r", &[], Style::new(), 0, 20));
        assert_eq!(got, "a?[31mb?");
        assert!(!got.chars().any(char::is_control));
    }

    #[test]
    fn a_range_past_the_end_is_harmless() {
        let got = cut("ab", &[(0..10, red())], Style::new(), 0, 5);
        assert_eq!(got, vec![Span::styled("ab", red())]);
        assert!(cut("ab", &[], Style::new(), 5, 5).is_empty());
    }
}
