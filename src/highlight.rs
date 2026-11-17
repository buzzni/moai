//! Syntax classes for file and hunk text. Rendering chooses the actual colours.
#![allow(dead_code)] // The TUI and web epics will consume this module.

use crate::diff::{FileDiff, LineKind};
use std::cell::Cell;
use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

const TEXT_LIMIT: usize = 256 * 1024;
const LINE_LIMIT: usize = 4096;

thread_local! {
    /// Set while this thread runs the parser under `catch_unwind` ([`Engine::line`]).
    static CATCHING: Cell<bool> = const { Cell::new(false) };
    /// A test makes the parser panic, to see the catch and the hooks around it (`tripping`).
    #[cfg(test)]
    static TRIP: Cell<bool> = const { Cell::new(false) };
}

/// Whether a panic on this thread now is a parser panic this module catches — the line then stands as plain text and
/// the program goes on. **A panic hook that puts the terminal back must let it pass** (`cmd::tui::modes_off_on_panic`,
/// review moai-vyu7.nsy 9): the hook runs before `catch_unwind` catches, so a hook that does not ask restores the
/// terminal under an explorer that keeps drawing.
pub fn catching() -> bool {
    CATCHING.with(Cell::get)
}

/// Run `f` with the parser panicking on every line — the caught path, made reachable for a test.
#[cfg(test)]
pub fn tripping<T>(f: impl FnOnce() -> T) -> T {
    let was = TRIP.with(|t| t.replace(true));
    let out = f();
    TRIP.with(|t| t.set(was));
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Constant,
    Tag,
    Attribute,
    Heading,
    Other,
}
impl Kind {
    /// Stable CSS-facing class name; the terminal maps the same kinds to 16 colours.
    pub fn name(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::String => "string",
            Self::Number => "number",
            Self::Keyword => "keyword",
            Self::Type => "type",
            Self::Function => "function",
            Self::Constant => "constant",
            Self::Tag => "tag",
            Self::Attribute => "attribute",
            Self::Heading => "heading",
            Self::Other => "other",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Span {
    pub kind: Kind,
    pub range: Range<usize>,
}
fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}
fn syntax<'a>(set: &'a SyntaxSet, name: &str, first: &str) -> Option<&'a SyntaxReference> {
    let file = Path::new(name).file_name().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = Path::new(file).extension().and_then(|s| s.to_str());
    set.find_syntax_by_extension(file).or_else(|| ext.and_then(|e| set.find_syntax_by_extension(e))).or_else(|| {
        let shebang = first.strip_prefix("#!")?.trim();
        let mut words = shebang.split_whitespace();
        let interpreter = Path::new(words.next()?).file_name()?.to_str()?;
        let interpreter = if interpreter == "env" {
            words.find(|word| !word.starts_with('-') && !word.contains('='))?
        } else {
            interpreter
        };
        let token = match interpreter {
            "sh" | "bash" | "zsh" | "dash" | "ksh" => "sh",
            "node" | "nodejs" => "js",
            value if value.starts_with("python") => "py",
            value => value,
        };
        set.find_syntax_by_token(token).or_else(|| set.find_syntax_by_first_line(first))
    })
}
fn prefixes() -> &'static [(Scope, Kind)] {
    static PREFIXES: OnceLock<Vec<(Scope, Kind)>> = OnceLock::new();
    PREFIXES.get_or_init(|| {
        [
            ("comment", Kind::Comment),
            ("string", Kind::String),
            ("constant.numeric", Kind::Number),
            ("keyword", Kind::Keyword),
            ("storage", Kind::Keyword),
            ("entity.name.type", Kind::Type),
            ("support.type", Kind::Type),
            ("entity.name.function", Kind::Function),
            ("support.function", Kind::Function),
            ("constant", Kind::Constant),
            ("entity.name.tag", Kind::Tag),
            ("entity.other.attribute-name", Kind::Attribute),
            ("markup.heading", Kind::Heading),
        ]
        .into_iter()
        .map(|(name, kind)| (Scope::new(name).expect("static scope"), kind))
        .collect()
    })
}
fn kind(stack: &ScopeStack) -> Kind {
    for scope in stack.as_slice().iter().rev() {
        for (prefix, kind) in prefixes() {
            if prefix.is_prefix_of(*scope) {
                return *kind;
            }
        }
    }
    Kind::Other
}
struct Engine<'a> {
    set: &'a SyntaxSet,
    state: ParseState,
    stack: ScopeStack,
}
impl<'a> Engine<'a> {
    fn new(set: &'a SyntaxSet, syntax: &SyntaxReference) -> Self {
        Self { set, state: ParseState::new(syntax), stack: ScopeStack::new() }
    }
    fn line(&mut self, text: &str) -> Vec<Span> {
        if text.len() > LINE_LIMIT {
            return Vec::new();
        }
        let before_state = self.state.clone();
        let before_stack = self.stack.clone();
        let was = CATCHING.with(|c| c.replace(true));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.try_line(text)));
        CATCHING.with(|c| c.set(was));
        match result {
            Ok(Some(spans)) => spans,
            _ => {
                self.state = before_state;
                self.stack = before_stack;
                Vec::new()
            }
        }
    }
    fn try_line(&mut self, text: &str) -> Option<Vec<Span>> {
        // Newline grammars expect the delimiter, while ranges belong to the visible line.
        #[cfg(test)]
        if TRIP.with(Cell::get) {
            panic!("the parser fell over (highlight::tripping)");
        }
        let mut input = String::with_capacity(text.len() + 1);
        input.push_str(text);
        input.push('\n');
        let operations = self.state.parse_line(&input, self.set).ok()?;
        let mut spans = Vec::new();
        let mut from = 0;
        for (offset, operation) in operations {
            let end = offset.min(text.len());
            if end < from || !text.is_char_boundary(end) {
                return None;
            }
            add(&mut spans, kind(&self.stack), from..end);
            self.stack.apply(&operation).ok()?;
            from = end;
        }
        add(&mut spans, kind(&self.stack), from..text.len());
        Some(spans)
    }
}
fn add(spans: &mut Vec<Span>, kind: Kind, range: Range<usize>) {
    if range.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.kind == kind
        && last.range.end == range.start
    {
        last.range.end = range.end;
        return;
    }
    spans.push(Span { kind, range });
}
/// Highlight file text in order, or return None when it is too large or unknown.
pub fn lines(name: &str, text: &str) -> Option<Vec<Vec<Span>>> {
    if text.len() > TEXT_LIMIT {
        return None;
    }
    let set = syntaxes();
    let first = text.split('\n').next().unwrap_or("");
    let syntax = syntax(set, name, first)?;
    let mut parser = Engine::new(set, syntax);
    Some(text.split_inclusive('\n').map(|line| parser.line(line.strip_suffix('\n').unwrap_or(line))).collect())
}
/// Each hunk has separate old and new parsers; context uses the new side.
pub fn diff(file: &FileDiff) -> Option<Vec<Vec<Vec<Span>>>> {
    if file.binary || file.hunks.is_empty() {
        return None;
    }
    let size = file.hunks.iter().flat_map(|h| &h.lines).fold(0usize, |n, line| n.saturating_add(line.text.len()));
    if size > TEXT_LIMIT {
        return None;
    }
    let name = file.new_path.as_deref().or(file.old_path.as_deref())?;
    let first = file.hunks.iter().flat_map(|h| &h.lines).next().map_or("", |l| l.text.as_str());
    let set = syntaxes();
    let syntax = syntax(set, name, first)?;
    Some(
        file.hunks
            .iter()
            .map(|hunk| {
                let mut old = Engine::new(set, syntax);
                let mut new = Engine::new(set, syntax);
                hunk.lines
                    .iter()
                    .map(|line| match line.kind {
                        LineKind::Context => {
                            let _ = old.line(&line.text);
                            new.line(&line.text)
                        }
                        LineKind::Delete => old.line(&line.text),
                        LineKind::Add => new.line(&line.text),
                        LineKind::Raw => Vec::new(),
                    })
                    .collect()
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_ranges(text: &str, rows: &[Vec<Span>]) {
        let lines: Vec<_> = text.split_inclusive('\n').map(|line| line.strip_suffix('\n').unwrap_or(line)).collect();
        assert_eq!(rows.len(), lines.len());
        for (line, spans) in lines.iter().zip(rows) {
            let mut end = 0;
            for span in spans {
                assert!(span.range.start >= end && span.range.end <= line.len());
                assert!(line.is_char_boundary(span.range.start) && line.is_char_boundary(span.range.end));
                end = span.range.end;
            }
        }
    }
    #[test]
    fn six_languages_have_semantic_spans() {
        for (name, text) in [
            ("main.rs", "let a = \"한글\"; // 주석\n"),
            ("app.ts", "const value: number = 12; // hi\n"),
            ("Cargo.toml", "key = \"value\" # hi\n"),
            ("README.md", "# Heading\n\n**bold**\n"),
            ("data.json", "{\"key\": true, \"n\": 3}\n"),
            ("run.sh", "echo \"hello\" # comment\n"),
        ] {
            let rows = lines(name, text).unwrap_or_else(|| panic!("no syntax for {name}"));
            assert_ranges(text, &rows);
            assert!(rows.iter().flatten().any(|span| span.kind != Kind::Other), "no semantic span for {name}");
        }
        assert!(lines("unknown.veryunlikely", "ordinary text\n").is_none());
        assert!(lines("script", "#!/usr/bin/env bash\necho hi\n").is_some());
    }
    #[test]
    fn multiline_comments_and_unicode_keep_byte_boundaries() {
        let text = "let x = \"한글\"; // 주석\n/* start\n계속 */\nlet y = 2;\n";
        let rows = lines("main.rs", text).unwrap();
        assert_ranges(text, &rows);
        assert!(rows[0].iter().any(|s| s.kind == Kind::String));
        assert!(rows[0].iter().any(|s| s.kind == Kind::Comment));
        assert!(rows[2].iter().any(|s| s.kind == Kind::Comment));
    }
    #[test]
    fn diff_sides_and_hunks_have_independent_states() {
        let patch = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,3 +1,3 @@\n-let a = \"open\n+let a = 1;\n /* open\n continuing */\n@@ -10 +10 @@\n-old\n+let z = 2;\n";
        let file = crate::diff::parse(patch).remove(0);
        let got = diff(&file).unwrap();
        assert_eq!(got.len(), file.hunks.len());
        for (h, spans) in file.hunks.iter().zip(&got) {
            assert_eq!(h.lines.len(), spans.len());
        }
        let one = file.hunks[0].lines[1].text.find('1').unwrap();
        assert!(!got[0][1].iter().any(|s| s.kind == Kind::String && s.range.contains(&one)));
        assert!(got[0][3].iter().any(|s| s.kind == Kind::Comment));
        let let_at = file.hunks[1].lines[1].text.find("let").unwrap();
        assert!(!got[1][1].iter().any(|s| s.kind == Kind::Comment && s.range.contains(&let_at)));
    }
    #[test]
    fn renamed_and_deleted_files_select_the_right_path() {
        let patch = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-let old = 1;\n+let new = 2;\n";
        let mut file = crate::diff::parse(patch).remove(0);
        file.old_path = Some("old.unknown-extension".into());
        file.new_path = Some("new.rs".into());
        let renamed = diff(&file).expect("new path chooses Rust");
        assert!(renamed[0][1].iter().any(|span| span.kind == Kind::Keyword));
        assert_eq!(renamed, diff(&file).unwrap());
        file.old_path = Some("old.rs".into());
        file.new_path = None;
        let deleted = diff(&file).expect("deleted file uses old path");
        assert!(deleted[0][0].iter().any(|span| span.kind == Kind::Keyword));
    }
    #[test]
    fn respects_limits_and_handles_unknown_input() {
        let long_line = "a".repeat(LINE_LIMIT + 1);
        let rows = lines("x.rs", &long_line).unwrap();
        assert!(rows[0].is_empty());
        assert!(lines("x.rs", &"a".repeat(TEXT_LIMIT + 1)).is_none());
        assert!(lines("x.rs", "").is_some());
        let _ = lines("x.rs", "\0\u{1b}😀\n");
        let patch = "diff --git a/a.rs b/a.rs\nBinary files a/a.rs and b/a.rs differ\n";
        assert!(diff(&crate::diff::parse(patch).remove(0)).is_none());
        let mut file =
            crate::diff::parse("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +1 @@\n+x\n").remove(0);
        file.hunks[0].lines[0].text = "a".repeat(TEXT_LIMIT + 1);
        assert!(diff(&file).is_none());
    }
    /// **A parser panic is caught and says so while it unwinds** — the line stands plain, and `catching` is true only
    /// inside the catch (the TUI's panic hook asks it, `cmd::tui`).
    #[test]
    fn a_parser_panic_is_caught_as_plain_text() {
        let rows = tripping(|| lines("a.rs", "let a = 1;\nlet b = 2;\n"));
        assert_eq!(rows, Some(vec![Vec::new(), Vec::new()]));
        assert!(!catching(), "the flag outlived the catch");
    }
}
