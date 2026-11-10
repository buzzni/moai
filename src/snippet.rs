//! Shared snippet path and file policy for the web and terminal viewers.
use std::io::Read;
use std::path::Path;

pub const MAX_BYTES: u64 = 256 * 1024;
#[derive(Clone, Copy)]
pub enum From<'a> {
    Issue,
    Wiki { page_rel: &'a str },
}
#[derive(Debug, PartialEq)]
pub enum Why {
    Refused,
    Missing,
    TooLarge,
}
#[derive(Debug, PartialEq)]
pub enum DetailWhy {
    Policy(Why),
    NotText,
}
impl std::convert::From<Why> for DetailWhy {
    fn from(value: Why) -> Self {
        Self::Policy(value)
    }
}
pub struct Opened {
    pub path: String,
    pub text: String,
}
pub struct Context<'a> {
    pub base: &'a Path,
    pub from: From<'a>,
}

pub fn sensitive(rel: &str) -> bool {
    let parts: Vec<_> = rel.split('/').collect();
    let name = parts.last().copied().unwrap_or("");
    parts.iter().any(|p| matches!(*p, ".git" | ".moai" | ".ssh" | ".aws" | ".azure" | ".kube"))
        || parts.windows(2).any(|p| p == [".docker", "config.json"])
        || matches!(name, ".env" | ".npmrc" | ".netrc" | ".git-credentials")
        || name.starts_with(".env.")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || [".pem", ".key", ".p12"].iter().any(|s| name.ends_with(s))
        || name.contains(".tfstate")
}

pub fn resolve(written: &str, from: From<'_>) -> Result<String, Why> {
    if written.is_empty() || written.contains(['\\', ':', '%']) || written.chars().any(char::is_control) {
        return Err(Why::Refused);
    }
    let rooted = written.starts_with('/') || written.starts_with("@/");
    let raw = written.strip_prefix("@/").unwrap_or(written).trim_start_matches('/');
    let mut parts: Vec<&str> = Vec::new();
    if !rooted && let From::Wiki { page_rel } = from {
        let page = resolve(page_rel, From::Issue)?;
        // Store the page's components separately because `page` owns their bytes.
        let parent = page.rsplit_once('/').map_or("", |(p, _)| p);
        return resolve(&format!("{parent}/{raw}"), From::Issue);
    }
    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or(Why::Refused)?;
            }
            _ => parts.push(part),
        }
    }
    let rel = parts.join("/");
    if rel.is_empty() || sensitive(&rel) { Err(Why::Refused) } else { Ok(rel) }
}

fn read_detail(base: &Path, rel: &str) -> Result<String, DetailWhy> {
    // Revalidate at the I/O boundary, including the actual target of an internal symlink.
    let rel = resolve(rel, From::Issue)?;
    let base = crate::path::canonical(base).map_err(|_| Why::Missing)?;
    let path = crate::path::canonical(base.join(rel)).map_err(|_| Why::Missing)?;
    let actual = path.strip_prefix(&base).map_err(|_| Why::Refused)?;
    let actual = actual.to_str().ok_or(Why::Refused)?.replace('\\', "/");
    if sensitive(&actual) {
        return Err(Why::Refused.into());
    }
    let file = crate::held::open_inside(&path, &crate::held::Home::of(&base)).map_err(|_| Why::Missing)?;
    if file.metadata().map_err(|_| Why::Missing)?.len() > MAX_BYTES {
        return Err(Why::TooLarge.into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| Why::Missing)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Why::TooLarge.into());
    }
    if bytes.contains(&0) {
        return Err(DetailWhy::NotText);
    }
    String::from_utf8(bytes).map_err(|_| DetailWhy::NotText)
}

pub fn read(base: &Path, rel: &str) -> Result<String, Why> {
    read_detail(base, rel).map_err(|e| match e {
        DetailWhy::Policy(why) => why,
        DetailWhy::NotText => Why::Missing,
    })
}

// Recognize the paired marker styles supported by the snippet plugin.
fn marker(line: &str) -> Option<(bool, &str, u8)> {
    let s = line.trim();
    let (s, style) = if let Some(x) = s.strip_prefix("//") {
        (x.trim(), 0)
    } else if let Some(x) = s.strip_prefix("/*").and_then(|x| x.strip_suffix("*/")) {
        (x.trim(), 1)
    } else if let Some(x) = s.strip_prefix("<!--").and_then(|x| x.strip_suffix("-->")) {
        (x.trim(), 2)
    } else if let Some(x) = s.strip_prefix("::") {
        (x.trim(), 3)
    } else if let Some(x) = s.strip_prefix("#pragma ") {
        (x.trim(), 4)
    } else if let Some(x) = s.strip_prefix('#') {
        (x.trim(), 5)
    } else {
        return None;
    };
    let s = s.strip_prefix('#').unwrap_or(s).trim_start();
    let (end, rest) = if let Some(x) = s.strip_prefix("endregion").or_else(|| s.strip_prefix("EndRegion")) {
        (true, x)
    } else if let Some(x) = s.strip_prefix("region").or_else(|| s.strip_prefix("Region")) {
        (false, x)
    } else {
        return None;
    };
    let name = rest.trim();
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((end, name, style))
}
pub fn region(text: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let Some((start, style)) = lines.iter().enumerate().find_map(|(i, l)| match marker(l) {
        Some((false, n, s)) if n == name => Some((i, s)),
        _ => None,
    }) else {
        return text.to_owned();
    };
    let Some(end) = (start + 1..lines.len())
        .find(|&i| matches!(marker(lines[i]), Some((true, n, s)) if s == style && (n.is_empty() || n == name)))
    else {
        return text.to_owned();
    };
    let selected: Vec<_> = lines[start + 1..end].iter().copied().filter(|l| marker(l).is_none()).collect();
    let indent = selected
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let mut result = selected.iter().map(|l| l.get(indent..).unwrap_or("")).collect::<Vec<_>>().join("\n");
    if !selected.is_empty() {
        result.push('\n');
    }
    result
}
impl Context<'_> {
    pub fn open_detail(&self, written: &str, region_name: Option<&str>) -> Result<Opened, DetailWhy> {
        let path = resolve(written, self.from)?;
        let text = read_detail(self.base, &path)?;
        let text = region_name.map_or(text.clone(), |name| region(&text, name));
        Ok(Opened { path, text })
    }
    pub fn open(&self, written: &str, region_name: Option<&str>) -> Result<Opened, Why> {
        let path = resolve(written, self.from)?;
        let text = read(self.base, &path)?;
        let text = region_name.map_or(text.clone(), |name| region(&text, name));
        Ok(Opened { path, text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_case_sensitive_denials() {
        for bad in [
            "../../x",
            "C:/x",
            "a\\b",
            "%2e%2e/x",
            "file:x",
            ".git/config",
            ".moai/issues.jsonl",
            "src/.git/HEAD",
            ".env",
            ".env.local",
            "id_rsa",
            "a/b/key.pem",
            "x/.ssh/config",
            "infra/prod.tfstate",
        ] {
            assert_eq!(resolve(bad, From::Issue), Err(Why::Refused), "{bad}");
        }
        assert!(resolve("../../x", From::Wiki { page_rel: "docs/a.md" }).is_err());
        for path in ["src/x.rs", "./src/x.rs", "/src/x.rs", "@/src/x.rs"] {
            assert_eq!(resolve(path, From::Issue).unwrap(), "src/x.rs");
        }
        assert_eq!(resolve("../../src/x.rs", From::Wiki { page_rel: "docs/guide/a.md" }).unwrap(), "src/x.rs");
        assert!(resolve(".ENV", From::Issue).is_ok());
    }
    #[test]
    fn matching_regions_are_dedented_and_nested_markers_removed() {
        for (start, end) in [
            ("// #region a", "// #endregion a"),
            ("<!-- #region a -->", "<!-- #endregion a -->"),
            ("#pragma region a", "#pragma endregion a"),
        ] {
            let text = format!("before\n{start}\n  one\n  // #region b\n    two\n  // #endregion b\n{end}\nafter\n");
            assert_eq!(region(&text, "a"), "one\n  two\n");
            assert_eq!(region(&text, "missing"), text);
        }
    }
    #[test]
    fn bytes_are_bounded_and_text_only() {
        let root = crate::scratch::Scratch::new("snippet-bytes");
        std::fs::write(root.join("x"), vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(read(root.path(), "x"), Err(Why::TooLarge));
        for bytes in [&b"a\0b"[..], &b"\xff"[..]] {
            std::fs::write(root.join("x"), bytes).unwrap();
            assert_eq!(read(root.path(), "x"), Err(Why::Missing));
        }
    }
    #[cfg(unix)]
    #[test]
    fn links_cannot_reach_outside_or_sensitive_targets_and_fifo_does_not_block() {
        use std::os::unix::fs::symlink;
        let root = crate::scratch::Scratch::new("snippet-links");
        let outside = crate::scratch::Scratch::new("snippet-outside");
        std::fs::write(outside.join("x"), "secret").unwrap();
        symlink(outside.join("x"), root.join("outside")).unwrap();
        assert_eq!(read(root.path(), "outside"), Err(Why::Refused));
        std::fs::write(root.join(".env"), "secret").unwrap();
        symlink(root.join(".env"), root.join("alias")).unwrap();
        assert_eq!(read(root.path(), "alias"), Err(Why::Refused));
        assert!(std::process::Command::new("mkfifo").arg(root.join("pipe")).status().unwrap().success());
        assert_eq!(read(root.path(), "pipe"), Err(Why::Missing));
    }
}
