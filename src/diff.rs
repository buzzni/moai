//! Unified-diff parsing and side-by-side alignment, independent of I/O and UI.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    ModeChanged,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub kind: FileKind,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}
impl Default for FileDiff {
    fn default() -> Self {
        Self { old_path: None, new_path: None, kind: FileKind::Modified, binary: false, hunks: Vec::new() }
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Hunk {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    pub section: String,
    pub lines: Vec<Line>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Context,
    Add,
    Delete,
    Raw,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Line {
    pub kind: LineKind,
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
    pub text: String,
    pub no_newline: bool,
}

/// Git's C-quoted path syntax represents bytes, so octal escapes are decoded
/// before interpreting UTF-8. Malformed input remains readable and never panics.
pub fn unquote(path: &str) -> String {
    if !(path.starts_with('"') && path.ends_with('"') && path.len() >= 2) {
        return path.to_owned();
    }
    let bytes = &path.as_bytes()[1..path.len() - 1];
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 == bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        match bytes[i] {
            b'n' => {
                out.push(b'\n');
                i += 1;
            }
            b't' => {
                out.push(b'\t');
                i += 1;
            }
            b'r' => {
                out.push(b'\r');
                i += 1;
            }
            b'b' => {
                out.push(8);
                i += 1;
            }
            b'f' => {
                out.push(12);
                i += 1;
            }
            b'v' => {
                out.push(11);
                i += 1;
            }
            b'a' => {
                out.push(7);
                i += 1;
            }
            b'\\' | b'"' => {
                out.push(bytes[i]);
                i += 1;
            }
            b'0'..=b'7' => {
                let mut n = 0u16;
                let mut count = 0;
                while i < bytes.len() && count < 3 && (b'0'..=b'7').contains(&bytes[i]) {
                    n = n * 8 + u16::from(bytes[i] - b'0');
                    i += 1;
                    count += 1;
                }
                out.push(n as u8);
            }
            _ => {
                out.push(b'\\');
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn path(s: &str, prefix: &str) -> Option<String> {
    let p = unquote(s.split('\t').next().unwrap_or(s));
    if p == "/dev/null" { None } else { Some(p.strip_prefix(prefix).unwrap_or(&p).to_owned()) }
}
fn header_paths(s: &str) -> (Option<String>, Option<String>) {
    if s.starts_with('"') {
        let mut escaped = false;
        let mut end = None;
        for (i, c) in s.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                end = Some(i + 1);
                break;
            }
        }
        if let Some(end) = end {
            return (path(&s[..end], "a/"), path(s[end..].trim_start(), "b/"));
        }
    }
    // Git does not quote ordinary spaces. The b/ marker is the best fallback
    // when ---/+++ or rename headers are absent (binary and mode-only diffs).
    for (i, _) in s.match_indices(" b/") {
        let old = path(&s[..i], "a/");
        let new = path(&s[i + 1..], "b/");
        if old == new {
            return (old, new);
        }
    }
    if let Some(i) = s.rfind(" b/") {
        return (path(&s[..i], "a/"), path(&s[i + 1..], "b/"));
    }
    (None, None)
}
fn span(s: &str, marker: char) -> Option<(usize, usize)> {
    let s = s.strip_prefix(marker)?;
    let (start, len) = s.split_once(',').unwrap_or((s, "1"));
    Some((start.parse().ok()?, len.parse().ok()?))
}
fn hunk_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ ")?;
    let (head, section) = rest.split_once(" @@")?;
    let mut fields = head.split_whitespace();
    let (old_start, old_len) = span(fields.next()?, '-')?;
    let (new_start, new_len) = span(fields.next()?, '+')?;
    Some(Hunk { old_start, old_len, new_start, new_len, section: section.trim_start().to_owned(), lines: Vec::new() })
}
/// Only complete input lines are consumed; a truncated final fragment is ignored.
pub fn parse(text: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut file: Option<FileDiff> = None;
    let mut active = false;
    let mut combined = false;
    let mut old = 0usize;
    let mut new = 0usize;
    let mut old_left = 0usize;
    let mut new_left = 0usize;
    for chunk in text.split_inclusive('\n') {
        let Some(line) = chunk.strip_suffix('\n') else { break };
        if let Some(header) = line.strip_prefix("diff --git ") {
            if let Some(f) = file.take() {
                files.push(f);
            }
            let (old_path, new_path) = header_paths(header);
            file = Some(FileDiff { old_path, new_path, ..FileDiff::default() });
            active = false;
            combined = false;
            continue;
        }
        if let Some(name) = line.strip_prefix("diff --cc ").or_else(|| line.strip_prefix("diff --combined ")) {
            if let Some(f) = file.take() {
                files.push(f);
            }
            let name = unquote(name);
            file = Some(FileDiff { old_path: Some(name.clone()), new_path: Some(name), ..FileDiff::default() });
            active = false;
            combined = false;
            continue;
        }
        if line.starts_with("\\ No newline at end of file") {
            if let Some(last) = file.as_mut().and_then(|f| f.hunks.last_mut()).and_then(|h| h.lines.last_mut()) {
                last.no_newline = true;
            }
            continue;
        }
        if line.starts_with("@@@ ") {
            let h = Hunk {
                old_start: 0,
                old_len: 0,
                new_start: 0,
                new_len: 0,
                section: line.to_owned(),
                lines: Vec::new(),
            };
            file.get_or_insert_with(FileDiff::default).hunks.push(h);
            active = true;
            combined = true;
            continue;
        }
        if let Some(h) = hunk_header(line) {
            combined = false;
            old = h.old_start;
            new = h.new_start;
            old_left = h.old_len;
            new_left = h.new_len;
            file.get_or_insert_with(FileDiff::default).hunks.push(h);
            active = true;
            continue;
        }
        if active && combined {
            if let Some(h) = file.as_mut().and_then(|f| f.hunks.last_mut()) {
                h.lines.push(Line {
                    kind: LineKind::Raw,
                    old_no: None,
                    new_no: None,
                    text: line.to_owned(),
                    no_newline: false,
                });
            }
            continue;
        }
        if active {
            let exhausted = old_left == 0 && new_left == 0;
            if exhausted && (line.starts_with("--- ") || line.starts_with("+++ ")) {
                active = false;
            } else if let Some((prefix, body)) = line.as_bytes().first().copied().zip(line.get(1..)) {
                let (kind, old_no, new_no) = match prefix {
                    b' ' => {
                        let nums = (LineKind::Context, Some(old), Some(new));
                        old = old.saturating_add(1);
                        new = new.saturating_add(1);
                        old_left = old_left.saturating_sub(1);
                        new_left = new_left.saturating_sub(1);
                        nums
                    }
                    b'-' => {
                        let nums = (LineKind::Delete, Some(old), None);
                        old = old.saturating_add(1);
                        old_left = old_left.saturating_sub(1);
                        nums
                    }
                    b'+' => {
                        let nums = (LineKind::Add, None, Some(new));
                        new = new.saturating_add(1);
                        new_left = new_left.saturating_sub(1);
                        nums
                    }
                    _ => continue,
                };
                if let Some(h) = file.as_mut().and_then(|f| f.hunks.last_mut()) {
                    h.lines.push(Line { kind, old_no, new_no, text: body.to_owned(), no_newline: false });
                }
                continue;
            } else {
                continue;
            }
        }
        if let Some(p) = line.strip_prefix("--- ") {
            if file.as_ref().is_some_and(|f| !f.hunks.is_empty()) {
                files.push(file.take().unwrap());
            }
            let f = file.get_or_insert_with(FileDiff::default);
            f.old_path = path(p, "a/");
            if f.old_path.is_none() {
                f.kind = FileKind::Added;
            }
        } else if let Some(p) = line.strip_prefix("+++ ") {
            let f = file.get_or_insert_with(FileDiff::default);
            f.new_path = path(p, "b/");
            if f.new_path.is_none() {
                f.kind = FileKind::Deleted;
            }
        } else if let Some(f) = file.as_mut() {
            if let Some(p) = line.strip_prefix("rename from ") {
                f.old_path = Some(unquote(p));
                f.kind = FileKind::Renamed;
            } else if let Some(p) = line.strip_prefix("rename to ") {
                f.new_path = Some(unquote(p));
                f.kind = FileKind::Renamed;
            } else if let Some(p) = line.strip_prefix("copy from ") {
                f.old_path = Some(unquote(p));
                f.kind = FileKind::Copied;
            } else if let Some(p) = line.strip_prefix("copy to ") {
                f.new_path = Some(unquote(p));
                f.kind = FileKind::Copied;
            } else if line.starts_with("new file mode ") {
                f.kind = FileKind::Added;
                f.old_path = None;
            } else if line.starts_with("deleted file mode ") {
                f.kind = FileKind::Deleted;
                f.new_path = None;
            } else if line.starts_with("old mode ") || line.starts_with("new mode ") {
                if f.kind == FileKind::Modified {
                    f.kind = FileKind::ModeChanged;
                }
            } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                f.binary = true;
            }
        }
    }
    if let Some(f) = file {
        files.push(f);
    }
    files
}
/// Align runs of removed and added lines by their positions within the run.
#[allow(dead_code)] // The TUI and web side-by-side views consume this.
pub fn pair(hunk: &Hunk) -> Vec<(Option<&Line>, Option<&Line>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < hunk.lines.len() {
        if matches!(hunk.lines[i].kind, LineKind::Context | LineKind::Raw) {
            out.push((Some(&hunk.lines[i]), Some(&hunk.lines[i])));
            i += 1;
            continue;
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        while i < hunk.lines.len() && !matches!(hunk.lines[i].kind, LineKind::Context | LineKind::Raw) {
            match hunk.lines[i].kind {
                LineKind::Delete => left.push(&hunk.lines[i]),
                LineKind::Add => right.push(&hunk.lines[i]),
                LineKind::Context | LineKind::Raw => {}
            }
            i += 1;
        }
        for n in 0..left.len().max(right.len()) {
            out.push((left.get(n).copied(), right.get(n).copied()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_numbers_and_pairs_runs() {
        let text = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -3,4 +3,2 @@ fn f\n context\n--- a\n-two\n-three\n+new\n\\ No newline at end of file\n@@ -20 +18 @@\n-old\n+fresh\n";
        let f = parse(text);
        assert_eq!(f.len(), 1);
        let h = &f[0].hunks[0];
        assert_eq!(h.section, "fn f");
        assert_eq!(h.lines[1].text, "-- a");
        assert_eq!((h.lines[1].old_no, h.lines[1].new_no), (Some(4), None));
        assert_eq!((h.lines[4].old_no, h.lines[4].new_no), (None, Some(4)));
        assert!(h.lines[4].no_newline);
        let pairs = pair(h);
        assert_eq!(pairs.len(), 4);
        assert!(pairs[0].0.is_some() && pairs[0].1.is_some());
        assert_eq!(pairs[1].0.unwrap().text, "-- a");
        assert_eq!(pairs[1].1.unwrap().text, "new");
        assert!(pairs[2].1.is_none() && pairs[3].1.is_none());
        assert_eq!(f[0].hunks[1].lines[0].old_no, Some(20));
    }
    #[test]
    fn quoted_paths_and_rename_are_decoded() {
        assert_eq!(unquote(r#""\353\202\230\t\"x""#), "나\t\"x");
        let text = "diff --git \"a/old\\tname\" \"b/\\353\\202\\230\\tnew\"\nsimilarity index 100%\nrename from \"old\\tname\"\nrename to \"\\353\\202\\230\\tnew\"\n";
        let f = parse(text);
        assert_eq!(f[0].kind, FileKind::Renamed);
        assert_eq!(f[0].old_path.as_deref(), Some("old\tname"));
        assert_eq!(f[0].new_path.as_deref(), Some("나\tnew"));
    }
    #[test]
    fn empty_added_binary_mode_and_truncation_are_preserved() {
        let s = "diff --git a/new b/new\nnew file mode 100644\nindex 0000000..e69de29\ndiff --git a/bin b/bin\nBinary files a/bin and b/bin differ\ndiff --git a/mode b/mode\nold mode 100644\nnew mode 100755\ndiff --git a/a b/a\n--- /dev/null\n+++ b/a\n@@ -0,0 +1 @@\n+complete\n+partial";
        let f = parse(s);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].kind, FileKind::Added);
        assert_eq!(f[0].old_path, None);
        assert!(f[1].binary);
        assert_eq!(f[2].kind, FileKind::ModeChanged);
        assert_eq!(f[3].hunks[0].lines.len(), 1);
        assert_eq!(f[3].hunks[0].lines[0].text, "complete");
    }
    #[test]
    fn combined_lines_are_preserved_without_panic() {
        let f = parse("diff --cc a\n@@@ -1,1 -1,1 +1,1 @@@\n- old\n++new\n");
        assert_eq!(f[0].hunks[0].lines[0].kind, LineKind::Raw);
        assert_eq!(f[0].hunks[0].lines[1].text, "++new");
        assert_eq!(pair(&f[0].hunks[0]).len(), 2);
    }
    #[test]
    fn odd_input_does_not_panic() {
        for s in [
            "",
            "@@@ -1,2 +1,2 @@@\n",
            "@@ -1 +1 @@\n+😀\n",
            "--- x\n+++ y\n@@ -999999999999999999999999 +1 @@\n",
            "💥\\\"\n",
        ] {
            let _ = parse(s);
            let _ = unquote(s);
        }
    }
}
