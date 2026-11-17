//! Filesystem access for the upcoming TUI and web file views.
#![allow(dead_code)] // The TUI and web epics will consume this module.

use crate::report::changes::Checkout;
use std::io::{self, Read};
use std::ops::Range;
#[cfg(unix)]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const TEXT_LIMIT: usize = 1024 * 1024;
const WALK_LIMIT: usize = 200_000;
const BATCH: usize = 256;

#[derive(Debug, Clone)]
pub struct Jail {
    root: PathBuf,
    home: crate::held::Home,
    #[cfg(unix)]
    root_fd: std::sync::Arc<std::fs::File>,
}
impl Jail {
    pub fn of(checkout: &Checkout) -> Option<Self> {
        let root = crate::path::canonical(&checkout.path).ok()?;
        #[cfg(not(unix))]
        if !root.is_dir() {
            return None;
        }
        let home = crate::held::Home::of(&root);
        #[cfg(unix)]
        let root_fd = {
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&root)
                .ok()?;
            let opened = crate::path::canonical(format!("/dev/fd/{}", file.as_raw_fd())).ok()?;
            if opened != root {
                return None;
            }
            std::sync::Arc::new(file)
        };
        Some(Self {
            root,
            home,
            #[cfg(unix)]
            root_fd,
        })
    }
}
#[derive(Debug)]
pub enum Why {
    Outside,
    Git,
    Missing,
    NotFile,
    NotDirectory,
    NotText,
    Io(io::Error),
}
impl From<io::Error> for Why {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound { Self::Missing } else { Self::Io(e) }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub size: u64,
}
impl Stamp {
    fn of(m: &std::fs::Metadata) -> Self {
        Self { modified: m.modified().ok(), size: m.len() }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    Link { dir: bool, inside: bool },
    Other,
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    pub openable: bool,
}
#[derive(Debug)]
pub struct Listed {
    pub entries: Vec<Entry>,
    pub stamp: Stamp,
}
#[derive(Debug)]
pub struct Text {
    pub text: String,
    pub size: u64,
    pub truncated: bool,
    pub stamp: Stamp,
}
#[derive(Debug, Clone, Copy)]
pub struct Walked {
    pub seen: usize,
    pub truncated: bool,
}

fn key_of(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        value.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        value
    }
}
fn normal(written: &str) -> Result<PathBuf, Why> {
    if written.is_empty() || written.chars().any(char::is_control) {
        return Err(Why::Outside);
    }
    #[cfg(windows)]
    let text = written.replace('\\', "/");
    #[cfg(not(windows))]
    let text = written.to_owned();
    #[cfg(windows)]
    if text.starts_with("//") || text.as_bytes().get(1) == Some(&b':') {
        return Err(Why::Outside);
    }
    let mut parts = Vec::new();
    for part in text.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(Why::Outside);
                }
            }
            _ if part.eq_ignore_ascii_case(".git") => return Err(Why::Git),
            _ => parts.push(part),
        }
    }
    Ok(parts.iter().collect())
}
fn resolve(jail: &Jail, path: &str) -> Result<PathBuf, Why> {
    let rel = normal(path)?;
    let at = jail.root.join(rel);
    match crate::held::place(&at, &jail.home) {
        Ok(_) => {}
        Err(crate::held::Unheld::Outside { .. }) => return Err(Why::Outside),
        Err(crate::held::Unheld::IntoGit { .. }) => return Err(Why::Git),
        Err(_) => return Err(Why::NotFile),
    }
    let actual = crate::path::canonical(&at).map_err(Why::from)?;
    let relative = actual.strip_prefix(&jail.root).map_err(|_| Why::Outside)?;
    if crate::held::into_git(relative) {
        return Err(Why::Git);
    }
    Ok(actual)
}
/// Open the checked canonical path component by component without following a
/// replacement link. FIFO and device opens are nonblocking.
#[cfg(unix)]
fn open_checked(jail: &Jail, actual: &Path, directory: bool) -> Result<std::fs::File, Why> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let rel = actual.strip_prefix(&jail.root).map_err(|_| Why::Outside)?;
    if crate::held::into_git(rel) {
        return Err(Why::Git);
    }
    let mut file = jail.root_fd.try_clone().map_err(Why::from)?;
    let components: Vec<_> = rel.components().collect();
    for (i, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else { return Err(Why::Outside) };
        let name = CString::new(name.as_bytes()).map_err(|_| Why::Outside)?;
        let is_dir = i + 1 < components.len() || directory;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if is_dir { libc::O_DIRECTORY } else { 0 };
        // SAFETY: file is a live directory handle, and name is a C string for one component.
        let fd = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ELOOP) {
                return Err(Why::Outside);
            }
            if e.raw_os_error() == Some(libc::ENOTDIR) {
                return Err(Why::NotDirectory);
            }
            return Err(Why::from(e));
        }
        // SAFETY: openat returned a new, owned descriptor.
        file = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(file)
}
#[cfg(not(unix))]
fn open_checked(jail: &Jail, actual: &Path, _directory: bool) -> Result<std::fs::File, Why> {
    crate::held::open_inside(actual, &jail.home).map_err(|e| match e {
        crate::held::Fell::Unheld(crate::held::Unheld::Outside { .. }) => Why::Outside,
        crate::held::Fell::Unheld(crate::held::Unheld::IntoGit { .. }) => Why::Git,
        crate::held::Fell::Unheld(_) => Why::NotFile,
        crate::held::Fell::Io(e) => Why::from(e),
    })
}
#[cfg(unix)]
fn directory_entries(file: &std::fs::File, _actual: &Path) -> io::Result<std::fs::ReadDir> {
    use std::os::fd::AsRawFd;
    std::fs::read_dir(format!("/dev/fd/{}", file.as_raw_fd()))
}
#[cfg(not(unix))]
fn directory_entries(_file: &std::fs::File, actual: &Path) -> io::Result<std::fs::ReadDir> {
    std::fs::read_dir(actual)
}

#[cfg(unix)]
fn inside_dir(jail: &Jail, path: &Path) -> bool {
    open_checked(jail, path, true).is_ok_and(|f| f.metadata().is_ok_and(|m| m.is_dir()))
}
#[cfg(not(unix))]
fn inside_dir(jail: &Jail, path: &Path) -> bool {
    crate::held::place(path, &jail.home).is_ok_and(|p| p.is_dir())
}

pub fn list(jail: &Jail, dir: &str) -> Result<Listed, Why> {
    let actual = resolve(jail, dir)?;
    #[cfg(unix)]
    let handle = open_checked(jail, &actual, true)?;
    #[cfg(unix)]
    let meta = handle.metadata().map_err(Why::from)?;
    #[cfg(not(unix))]
    let meta = std::fs::metadata(&actual).map_err(Why::from)?;
    if !meta.is_dir() {
        return Err(Why::NotDirectory);
    }
    let stamp = Stamp::of(&meta);
    #[cfg(unix)]
    let listing = directory_entries(&handle, &actual).map_err(Why::from)?;
    #[cfg(not(unix))]
    let listing = std::fs::read_dir(&actual).map_err(Why::from)?;
    collect_listing(jail, stamp, listing)
}
fn collect_listing(jail: &Jail, stamp: Stamp, listing: std::fs::ReadDir) -> Result<Listed, Why> {
    let mut entries = Vec::new();
    for entry in listing {
        let entry = entry.map_err(Why::from)?;
        let raw_name = entry.file_name();
        let openable = raw_name.to_str().is_some_and(|s| !s.chars().any(char::is_control));
        let name = raw_name.to_string_lossy().into_owned();
        if name.eq_ignore_ascii_case(".git") {
            continue;
        }
        let ty = entry.file_type().map_err(Why::from)?;
        let meta = std::fs::symlink_metadata(entry.path()).map_err(Why::from)?;
        let kind = if ty.is_dir() {
            Kind::Dir
        } else if ty.is_file() {
            Kind::File
        } else if ty.is_symlink() {
            let target =
                crate::held::place(&entry.path(), &jail.home).ok().and_then(|p| crate::path::canonical(p).ok());
            let inside =
                target.as_ref().is_some_and(|p| p.strip_prefix(&jail.root).is_ok_and(|r| !crate::held::into_git(r)));
            let dir = inside && target.as_ref().is_some_and(|p| inside_dir(jail, p));
            Kind::Link { dir, inside }
        } else {
            Kind::Other
        };
        entries.push(Entry { name, kind, size: meta.len(), openable });
    }
    let rank = |k: &Kind| !matches!(k, Kind::Dir | Kind::Link { dir: true, inside: true });
    entries.sort_by(|a, b| {
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.as_bytes().cmp(b.name.as_bytes()))
    });
    Ok(Listed { entries, stamp })
}
pub fn read(jail: &Jail, path: &str) -> Result<Text, Why> {
    let actual = resolve(jail, path)?;
    let file = open_checked(jail, &actual, false)?;
    let meta = file.metadata().map_err(Why::from)?;
    if !meta.is_file() {
        return Err(Why::NotFile);
    }
    let size = meta.len();
    let stamp = Stamp::of(&meta);
    let mut bytes = Vec::new();
    file.take(TEXT_LIMIT as u64 + 1).read_to_end(&mut bytes).map_err(Why::from)?;
    let truncated = size > TEXT_LIMIT as u64 || bytes.len() > TEXT_LIMIT;
    bytes.truncate(TEXT_LIMIT);
    if bytes.contains(&0) {
        return Err(Why::NotText);
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) if truncated && e.utf8_error().error_len().is_none() => {
            let valid = e.utf8_error().valid_up_to();
            let mut bytes = e.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).map_err(|_| Why::NotText)?
        }
        Err(_) => return Err(Why::NotText),
    };
    Ok(Text { text, size, truncated, stamp })
}
pub fn stamp(jail: &Jail, path: &str) -> Option<Stamp> {
    let actual = resolve(jail, path).ok()?;
    #[cfg(unix)]
    let meta = open_checked(jail, &actual, false).ok()?.metadata().ok()?;
    #[cfg(not(unix))]
    let meta = std::fs::metadata(actual).ok()?;
    (meta.is_file() || meta.is_dir()).then(|| Stamp::of(&meta))
}
/// Walk in bounded batches. Returning false from `take` stops immediately.
pub fn walk(jail: &Jail, dir: &str, skip: &[String], mut take: impl FnMut(&[String]) -> bool) -> Result<Walked, Why> {
    walk_limit(jail, dir, skip, WALK_LIMIT, &mut take)
}
fn walk_limit(
    jail: &Jail,
    dir: &str,
    skip: &[String],
    limit: usize,
    take: &mut impl FnMut(&[String]) -> bool,
) -> Result<Walked, Why> {
    let start = normal(dir)?;
    let start_key = key_of(&start);
    let skip: Vec<_> = skip
        .iter()
        .map(|s| s.trim_end_matches('/'))
        .filter(|s| !s.is_empty() && start_key != *s && !start_key.strip_prefix(*s).is_some_and(|r| r.starts_with('/')))
        .collect();
    // No link, including an intermediate one, is followed by the walker.
    #[cfg(unix)]
    let start_path = jail.root.join(&start);
    #[cfg(unix)]
    let _start_handle = open_checked(jail, &start_path, true)?;
    #[cfg(not(unix))]
    {
        let mut p = jail.root.clone();
        for c in start.components() {
            p.push(c);
            if std::fs::symlink_metadata(&p).map_err(Why::from)?.file_type().is_symlink() {
                return Err(Why::Outside);
            }
        }
    }
    let mut pending = vec![start];
    let mut seen = 0;
    let mut batch = Vec::new();
    while let Some(dir) = pending.pop() {
        let key = if dir.as_os_str().is_empty() { ".".to_owned() } else { key_of(&dir) };
        #[cfg(unix)]
        let listed = {
            let at = jail.root.join(&dir);
            let handle = open_checked(jail, &at, true)?;
            let stamp = Stamp::of(&handle.metadata().map_err(Why::from)?);
            let listing = directory_entries(&handle, &at).map_err(Why::from)?;
            collect_listing(jail, stamp, listing)?
        };
        #[cfg(not(unix))]
        let listed = list(jail, &key)?;
        for entry in listed.entries {
            let path = if key == "." { entry.name } else { format!("{key}/{}", entry.name) };
            if matches!(entry.kind, Kind::Dir)
                && skip.iter().any(|s| {
                    path == s.trim_end_matches('/')
                        || path.strip_prefix(s.trim_end_matches('/')).is_some_and(|r| r.starts_with('/'))
                })
            {
                continue;
            }
            if seen == limit {
                if !batch.is_empty() {
                    let _ = take(&batch);
                }
                return Ok(Walked { seen, truncated: true });
            }
            seen += 1;
            if entry.openable && matches!(entry.kind, Kind::Dir) {
                pending.push(PathBuf::from(&path));
            }
            batch.push(path);
            if batch.len() == BATCH {
                if !take(&batch) {
                    return Ok(Walked { seen, truncated: false });
                }
                batch.clear();
            }
        }
    }
    if !batch.is_empty() {
        let _ = take(&batch);
    }
    Ok(Walked { seen, truncated: false })
}
/// Case-insensitive substring search, returning boundaries in the original UTF-8.
pub fn matches(query: &str, path: &str) -> Option<Range<usize>> {
    if query.is_empty() {
        return None;
    }
    let needle = query.to_lowercase();
    let mut folded = String::new();
    let mut ranges = Vec::new();
    for (start, c) in path.char_indices() {
        let begin = folded.len();
        folded.extend(c.to_lowercase());
        ranges.push((begin, folded.len(), start, start + c.len_utf8()));
    }
    let begin = folded.find(&needle)?;
    let end = begin + needle.len();
    let first = ranges.iter().find(|r| r.1 > begin)?;
    let last = ranges.iter().rev().find(|r| r.0 < end)?;
    Some(first.2..last.3)
}
pub fn ext_of(name: &str) -> &str {
    Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn jail(root: &Path) -> Jail {
        Jail::of(&Checkout {
            key: ".".into(),
            path: root.to_owned(),
            branch: "main".into(),
            main: true,
            milestone: None,
        })
        .unwrap()
    }
    #[test]
    fn lists_and_reads_without_hiding_normal_names() {
        let dir = crate::scratch::Scratch::new("files-basic");
        let root: &Path = &dir;
        fs::create_dir(root.join("folder")).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "secret").unwrap();
        fs::write(root.join(".env"), "visible").unwrap();
        #[cfg(unix)]
        fs::write(root.join("a:b% name"), "yes").unwrap();
        fs::write(root.join("id_rsa"), "key").unwrap();
        fs::write(root.join("binary"), b"a\0b").unwrap();
        fs::write(root.join("invalid"), [0xff]).unwrap();
        let j = jail(root);
        let listed = list(&j, ".").unwrap();
        assert_eq!(listed.entries[0].name, "folder");
        assert!(!listed.entries.iter().any(|e| e.name.eq_ignore_ascii_case(".git")));
        assert_eq!(read(&j, ".env").unwrap().text, "visible");
        #[cfg(unix)]
        assert_eq!(read(&j, "a:b% name").unwrap().text, "yes");
        assert_eq!(read(&j, "id_rsa").unwrap().text, "key");
        assert!(matches!(read(&j, ".git/config"), Err(Why::Git)));
        assert!(matches!(read(&j, "../outside"), Err(Why::Outside)));
        assert!(matches!(read(&j, "folder/../../outside"), Err(Why::Outside)));
        assert!(matches!(read(&j, "/etc/passwd"), Err(Why::Missing)));
        assert!(matches!(read(&j, "binary"), Err(Why::NotText)));
        assert!(matches!(read(&j, "invalid"), Err(Why::NotText)));
        assert!(matches!(read(&j, "folder"), Err(Why::NotFile)));
        let before = stamp(&j, ".env").unwrap();
        fs::write(root.join(".env"), "longer content").unwrap();
        assert_ne!(before, stamp(&j, ".env").unwrap());
        assert!(stamp(&j, "missing").is_none());
    }
    #[cfg(unix)]
    #[test]
    fn non_utf8_names_are_visible_but_not_openable() {
        use std::os::unix::ffi::OsStringExt;
        let dir = crate::scratch::Scratch::new("files-names");
        let root: &Path = &dir;
        fs::write(root.join(std::ffi::OsString::from_vec(vec![0xff])), "hidden bytes").unwrap();
        fs::write(root.join("a\\b"), "backslash").unwrap();
        let j = jail(root);
        let entries = list(&j, ".").unwrap().entries;
        assert!(entries.iter().any(|e| !e.openable));
        assert_eq!(read(&j, "a\\b").unwrap().text, "backslash");
    }
    #[test]
    fn walk_can_be_cancelled_between_batches() {
        let dir = crate::scratch::Scratch::new("files-cancel");
        let root: &Path = &dir;
        for n in 0..300 {
            fs::write(root.join(format!("f{n:03}")), "").unwrap();
        }
        let mut batches = 0;
        let result = walk(&jail(root), ".", &[], |_| {
            batches += 1;
            false
        })
        .unwrap();
        assert_eq!(batches, 1);
        assert_eq!(result.seen, BATCH);
        assert!(!result.truncated);
    }
    #[test]
    fn text_limit_keeps_utf8_boundaries() {
        let dir = crate::scratch::Scratch::new("files-limit");
        let root: &Path = &dir;
        let mut value = "x".repeat(TEXT_LIMIT - 1);
        value.push('한');
        value.push('z');
        fs::write(root.join("large"), &value).unwrap();
        let text = read(&jail(root), "large").unwrap();
        assert!(text.truncated);
        assert_eq!(text.text.len(), TEXT_LIMIT - 1);
        assert_eq!(text.size, value.len() as u64);
    }
    #[cfg(unix)]
    #[test]
    fn links_git_and_fifos_cannot_escape_the_jail() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;
        let dir = crate::scratch::Scratch::new("files-links");
        let root: &Path = &dir;
        let outside = crate::scratch::Scratch::new("files-outside");
        fs::write(outside.join("secret"), "OUTSIDE").unwrap();
        fs::create_dir(root.join("inside")).unwrap();
        fs::write(root.join("inside/ok"), "inside").unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "GIT").unwrap();
        symlink(outside.path(), root.join("out-dir")).unwrap();
        symlink(outside.join("secret"), root.join("out-file")).unwrap();
        symlink("inside/ok", root.join("in-file")).unwrap();
        symlink("inside", root.join("in-dir")).unwrap();
        symlink(".git/config", root.join("git-link")).unwrap();
        symlink("loop-b", root.join("loop-a")).unwrap();
        symlink("loop-a", root.join("loop-b")).unwrap();
        let fifo = std::ffi::CString::new(root.join("fifo").as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo is a valid path in the isolated test directory.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let j = jail(root);
        assert_eq!(read(&j, "in-file").unwrap().text, "inside");
        assert_eq!(list(&j, "in-dir").unwrap().entries[0].name, "ok");
        assert!(matches!(read(&j, "out-file"), Err(Why::Outside)));
        assert!(matches!(list(&j, "out-dir"), Err(Why::Outside)));
        assert!(matches!(read(&j, "out-dir/secret"), Err(Why::Outside)));
        assert!(matches!(read(&j, "git-link"), Err(Why::Git)));
        assert!(matches!(read(&j, "fifo"), Err(Why::NotFile)));
        assert!(stamp(&j, "fifo").is_none());
        assert!(read(&j, "loop-a").is_err());
        assert!(stamp(&j, "out-file").is_none());
        let entries = list(&j, ".").unwrap().entries;
        assert!(entries.iter().any(|e| e.name == "out-dir" && matches!(e.kind, Kind::Link { inside: false, .. })));
        assert!(entries.iter().any(|e| e.name == "in-dir" && matches!(e.kind, Kind::Link { dir: true, inside: true })));
        let mut paths = Vec::new();
        walk(&j, ".", &[], |b| {
            paths.extend_from_slice(b);
            true
        })
        .unwrap();
        assert!(!paths.iter().any(|p| p == "out-dir/secret" || p == "in-dir/ok"));
        assert!(walk(&j, "in-dir", &[], |_| true).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn replacing_a_directory_link_cannot_redirect_reads() {
        use std::os::unix::fs::symlink;
        let dir = crate::scratch::Scratch::new("files-race");
        let root: &Path = &dir;
        let outside = crate::scratch::Scratch::new("files-race-outside");
        fs::create_dir(root.join("inside")).unwrap();
        fs::write(root.join("inside/secret"), "inside").unwrap();
        fs::write(outside.join("secret"), "OUTSIDE").unwrap();
        fs::write(outside.join("OUTSIDE_NAME"), "x").unwrap();
        symlink("inside", root.join("link")).unwrap();
        let base = root.to_path_buf();
        let external = outside.path().to_path_buf();
        let worker = std::thread::spawn(move || {
            for n in 0..500 {
                let temp = base.join(format!("swap-{n}"));
                let target = if n % 2 == 0 { external.as_path() } else { Path::new("inside") };
                symlink(target, &temp).unwrap();
                fs::rename(temp, base.join("link")).unwrap();
            }
        });
        let j = jail(root);
        for _ in 0..500 {
            if let Ok(text) = read(&j, "link/secret") {
                assert_eq!(text.text, "inside");
            }
            if let Ok(listed) = list(&j, "link") {
                assert!(listed.entries.iter().all(|e| e.name == "secret"));
            }
        }
        worker.join().unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn replacing_the_checkout_root_cannot_redirect_a_walk() {
        use std::os::unix::fs::symlink;
        let scratch = crate::scratch::Scratch::new("files-root-race");
        let root = scratch.join("project");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("inside"), "ok").unwrap();
        let outside = crate::scratch::Scratch::new("files-root-outside");
        fs::write(outside.join("OUTSIDE_NAME"), "x").unwrap();
        let j = jail(&root);
        fs::rename(&root, scratch.join("moved")).unwrap();
        symlink(outside.path(), &root).unwrap();
        let mut paths = Vec::new();
        if walk(&j, ".", &[], |batch| {
            paths.extend_from_slice(batch);
            true
        })
        .is_ok()
        {
            assert!(!paths.iter().any(|p| p == "OUTSIDE_NAME"));
        }
        assert!(read(&j, "OUTSIDE_NAME").is_err());
    }
    #[test]
    fn walk_skips_only_the_named_paths_and_is_bounded() {
        let dir = crate::scratch::Scratch::new("files-walk");
        let root: &Path = &dir;
        for p in ["target/deep", "target2", "a/target", ".git/hidden"] {
            fs::create_dir_all(root.join(p)).unwrap();
        }
        for p in ["target/deep/x", "target2/x", "a/target/x", ".git/hidden/x"] {
            fs::write(root.join(p), "x").unwrap();
        }
        let j = jail(root);
        let skip = vec!["target/".into()];
        let mut paths = Vec::new();
        walk(&j, ".", &skip, |b| {
            paths.extend_from_slice(b);
            true
        })
        .unwrap();
        assert!(!paths.iter().any(|p| p.starts_with("target/")));
        assert!(paths.contains(&"target2/x".to_owned()));
        assert!(paths.contains(&"a/target/x".to_owned()));
        assert!(!paths.iter().any(|p| p.contains(".git")));
        let mut inside = Vec::new();
        walk(&j, "target", &skip, |b| {
            inside.extend_from_slice(b);
            true
        })
        .unwrap();
        assert!(inside.contains(&"target/deep/x".to_owned()));
        let mut limited = Vec::new();
        let result = walk_limit(&j, ".", &[], 3, &mut |b| {
            limited.extend_from_slice(b);
            true
        })
        .unwrap();
        assert_eq!(result.seen, 3);
        assert!(result.truncated);
        assert_eq!(limited.len(), 3);
        assert_eq!(matches("TUI/Dr", "src/tui/draw.rs"), Some(4..10));
        assert_eq!(matches("", "x"), None);
        let p = "가나다.rs";
        let r = matches("나다", p).unwrap();
        assert_eq!(&p[r], "나다");
    }
}
