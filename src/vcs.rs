//! Read-only version-control operations shared by the CLI and the views.
//!
//! Only Git is recognised today; all paths passed to Git are literal pathspecs.

use crate::git;
use std::collections::BTreeMap;
use std::path::{Component, Path};
use std::time::Duration;

pub(crate) const PATCH_LIMIT: usize = 2 * 1024 * 1024;
const FILE_LIMIT: usize = 5_000;
// Lists need their complete count, even when only the first 5,000 entries are shown.
const LIST_LIMIT: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum Scope {
    Commits(Vec<String>),
    Branch,
    Working,
    #[allow(dead_code)] // The TUI and web changed-files view will use this scope.
    Whole,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    pub status: Status,
    pub adds: Option<usize>,
    pub dels: Option<usize>,
    pub binary: bool,
}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Changes {
    pub files: Vec<Change>,
    pub total: usize,
    pub truncated: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Patch {
    pub text: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum Backend {
    Git,
}
#[derive(Debug)]
pub enum Error {
    Git(git::Error),
    Timeout,
    Limit,
    InvalidRevision,
    InvalidPath,
    Io(std::io::Error),
}
impl From<git::Error> for Error {
    fn from(e: git::Error) -> Self {
        Self::Git(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn detect(root: &Path) -> Option<Backend> {
    root.ancestors().any(|p| p.join(".git").exists()).then_some(Backend::Git)
}
fn hex(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|c| c.is_ascii_hexdigit())
}
fn valid_path(s: &str) -> bool {
    !s.is_empty()
        && Path::new(s).components().all(|c| matches!(c, Component::Normal(n) if !n.eq_ignore_ascii_case(".git")))
}
fn run(root: &Path, args: &[&str], limit: usize, budget: Option<Duration>) -> Result<(Vec<u8>, bool), Error> {
    let mut all = vec!["--no-optional-locks", "--literal-pathspecs", "-c", "core.quotePath=false"];
    all.extend_from_slice(args);
    git::bounded(root, &all, limit, budget).ok_or(Error::Timeout)?.map_err(Error::Git)
}
fn records(root: &Path, args: &[&str], budget: Option<Duration>, take: impl FnMut(&[u8])) -> Result<(), Error> {
    let mut all = vec!["--no-optional-locks", "--literal-pathspecs", "-c", "core.quotePath=false"];
    all.extend_from_slice(args);
    git::records_bounded(root, &all, budget, take).ok_or(Error::Timeout)?.map_err(Error::Git)
}
fn text(root: &Path, args: &[&str], budget: Option<Duration>) -> Result<String, Error> {
    let (b, cut) = run(root, args, LIST_LIMIT, budget)?;
    if cut {
        return Err(Error::Limit);
    }
    Ok(String::from_utf8_lossy(&b).trim().to_owned())
}
fn head(root: &Path, budget: Option<Duration>) -> Result<String, Error> {
    let h = text(root, &["rev-parse", "--verify", "HEAD"], budget)?;
    if !hex(&h) {
        return Err(Error::InvalidRevision);
    }
    Ok(h)
}
/// The nearest merge base among trusted, full object IDs.
pub fn fork_point(root: &Path, candidates: &[String], budget: Option<Duration>) -> Result<Option<String>, Error> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let current = head(root, budget)?;
    let mut best: Option<(usize, String)> = None;
    for candidate in candidates {
        if !hex(candidate) {
            return Err(Error::InvalidRevision);
        }
        let base = match text(root, &["merge-base", &current, candidate], budget) {
            Ok(s) if hex(&s) => s,
            Ok(_) => continue,
            Err(Error::Git(git::Error::Failed(_))) => continue,
            Err(e) => return Err(e),
        };
        let range = format!("{base}..{current}");
        let count = text(root, &["rev-list", "--count", &range], budget)?.parse::<usize>().unwrap_or(usize::MAX);
        if best.as_ref().is_none_or(|(n, _)| count < *n) {
            best = Some((count, base));
        }
    }
    Ok(best.map(|(_, b)| b))
}
fn base(root: &Path, budget: Option<Duration>) -> Result<Option<String>, Error> {
    if !crate::worktree::is_linked(root) {
        return Ok(None);
    }
    let mut candidates = Vec::new();
    if let Some(main) = crate::worktree::main_root(root) {
        match head(&main, budget) {
            Ok(h) => candidates.push(h),
            Err(Error::Git(git::Error::Failed(_))) => {}
            Err(e) => return Err(e),
        }
    }
    let current = match text(root, &["symbolic-ref", "-q", "HEAD"], budget) {
        Ok(s) => s,
        Err(Error::Git(git::Error::Failed(_))) => String::new(),
        Err(e) => return Err(e),
    };
    let refs = text(root, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads/milestone/"], budget)?;
    for line in refs.lines() {
        if let Some((name, hash)) = line.split_once(' ')
            && name != current
            && hex(hash)
        {
            candidates.push(hash.to_owned());
        }
    }
    fork_point(root, &candidates, budget)
}
fn status(s: &str) -> Status {
    match s.as_bytes().first() {
        Some(b'A') => Status::Added,
        Some(b'D') => Status::Deleted,
        Some(b'R') => Status::Renamed,
        Some(b'C') => Status::Copied,
        Some(b'T') => Status::TypeChanged,
        Some(b'U') => Status::Conflicted,
        _ => Status::Modified,
    }
}
fn tracked_args<'a>(scope: &'a Scope, rev: &'a str, current: &'a str, mode: &'a str) -> Vec<&'a str> {
    // `diff-tree` ignores `--first-parent` and lists a merge against every parent, so a merge
    // would count the other side's files. `show` honours it, as the patch below does.
    let mut a = if matches!(scope, Scope::Commits(_)) {
        vec!["show", "--format=", "--no-show-signature", "--first-parent", "-m"]
    } else {
        vec!["diff"]
    };
    a.extend(["--no-ext-diff", "--no-textconv", "--no-color", "--relative", "-M", mode, "-z", rev]);
    if matches!(scope, Scope::Branch) {
        a.push(current);
    }
    a.push("--");
    a
}
fn revisions(root: &Path, scope: &Scope, budget: Option<Duration>) -> Result<Vec<String>, Error> {
    match scope {
        Scope::Commits(hashes) => {
            if hashes.iter().any(|h| !hex(h)) {
                return Err(Error::InvalidRevision);
            }
            Ok(hashes.clone())
        }
        Scope::Working => Ok(vec![head(root, budget)?]),
        Scope::Branch | Scope::Whole => Ok(base(root, budget)?.into_iter().collect()),
    }
}
fn untracked(root: &Path, budget: Option<Duration>) -> Result<Vec<String>, Error> {
    let mut paths = Vec::new();
    records(root, &["ls-files", "--others", "--exclude-standard", "-z"], budget, |raw| {
        let path = String::from_utf8_lossy(raw).into_owned();
        if valid_path(&path) {
            paths.push(path);
        }
    })?;
    // Git omits FIFOs, sockets and devices from ls-files. Walk directories
    // by descriptor without following links and ask Git whether each is ignored.
    paths.extend(special_files(root, budget)?);
    Ok(paths)
}
#[cfg(unix)]
fn special_files(root: &Path, budget: Option<Duration>) -> Result<Vec<String>, Error> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let ignored = Backend::Git.ignored_dirs(root, budget)?;
    let ignored: std::collections::HashSet<_> = ignored.iter().map(|s| s.trim_end_matches('/')).collect();
    let root_fd = std::fs::File::open(root)?;
    let mut pending = vec![String::new()];
    let mut found = Vec::new();
    'walk: while let Some(relative) = pending.pop() {
        let mut dir = root_fd.try_clone()?;
        for component in Path::new(&relative).components() {
            let Component::Normal(name) = component else { continue 'walk };
            let Ok(name) = CString::new(name.as_bytes()) else { continue 'walk };
            // SAFETY: dir is live and name is a NUL-terminated component.
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                continue 'walk;
            }
            // SAFETY: openat returned a new, owned descriptor.
            dir = unsafe { std::fs::File::from_raw_fd(fd) };
        }
        // /dev/fd names the open descriptor, so a rename cannot redirect this walk.
        for entry in std::fs::read_dir(format!("/dev/fd/{}", dir.as_raw_fd()))? {
            let entry = entry?;
            let os_name = entry.file_name();
            let Some(name) = os_name.to_str() else { continue };
            if name.eq_ignore_ascii_case(".git") {
                continue;
            }
            let path = if relative.is_empty() { name.to_owned() } else { format!("{relative}/{name}") };
            if !valid_path(&path) {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() && !ignored.contains(path.as_str()) {
                pending.push(path);
            } else if !kind.is_dir() && !kind.is_file() && !kind.is_symlink() {
                // check-ignore accepts literal paths after --, but rejects the
                // global --literal-pathspecs option used by diff commands.
                let literal = format!("./{path}");
                let result = git::bounded(
                    root,
                    &["--no-optional-locks", "check-ignore", "--quiet", "--", &literal],
                    1024,
                    budget,
                )
                .ok_or(Error::Timeout)?
                .map_err(Error::Git);
                match result {
                    Ok(_) => {}
                    Err(Error::Git(git::Error::Failed(why))) if why.is_empty() => found.push(path),
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(found)
}
#[cfg(not(unix))]
fn special_files(_root: &Path, _budget: Option<Duration>) -> Result<Vec<String>, Error> {
    Ok(Vec::new())
}

fn add_numstat(change: &mut Change, adds: &str, dels: &str) {
    if adds == "-" || dels == "-" {
        change.binary = true;
        change.adds = None;
        change.dels = None;
    } else if !change.binary {
        change.adds = adds.parse::<usize>().ok().map(|n| change.adds.unwrap_or(0).saturating_add(n));
        change.dels = dels.parse::<usize>().ok().map(|n| change.dels.unwrap_or(0).saturating_add(n));
    }
}
impl Backend {
    pub fn changes(self, root: &Path, scope: &Scope, budget: Option<Duration>) -> Result<Changes, Error> {
        let mut found = BTreeMap::<String, Change>::new();
        let revs = revisions(root, scope, budget)?;
        if matches!(scope, Scope::Whole) && revs.is_empty() {
            return Ok(Changes::default());
        }
        let current = if matches!(scope, Scope::Branch) { head(root, budget)? } else { String::new() };
        for rev in revs {
            let mut state: Option<(Status, Option<String>)> = None;
            records(root, &tracked_args(scope, &rev, &current, "--name-status"), budget, |raw| {
                let item = String::from_utf8_lossy(raw).into_owned();
                match state.take() {
                    None => state = Some((status(&item), None)),
                    Some((st, None)) if matches!(st, Status::Renamed | Status::Copied) => {
                        state = Some((st, Some(item)))
                    }
                    Some((st, old_path)) => {
                        found
                            .entry(item.clone())
                            .and_modify(|c| {
                                // Commits run oldest first: a later edit must not turn the file
                                // this issue created or renamed back into a plain modification.
                                let kept = st == Status::Modified
                                    && matches!(c.status, Status::Added | Status::Renamed | Status::Copied);
                                if !kept {
                                    c.status = st;
                                    c.old_path = old_path.clone();
                                }
                            })
                            .or_insert(Change {
                                path: item,
                                old_path,
                                status: st,
                                adds: None,
                                dels: None,
                                binary: false,
                            });
                    }
                }
            })?;
            let mut rename: Option<(String, String, bool)> = None;
            records(root, &tracked_args(scope, &rev, &current, "--numstat"), budget, |raw| {
                let item = String::from_utf8_lossy(raw).into_owned();
                if let Some((a, d, old_read)) = rename.take() {
                    if !old_read {
                        rename = Some((a, d, true));
                    } else if let Some(c) = found.get_mut(&item) {
                        add_numstat(c, &a, &d);
                    }
                } else {
                    let mut cols = item.splitn(3, '\t');
                    if let (Some(a), Some(d), Some(path)) = (cols.next(), cols.next(), cols.next()) {
                        if path.is_empty() {
                            rename = Some((a.to_owned(), d.to_owned(), false));
                        } else if let Some(c) = found.get_mut(path) {
                            add_numstat(c, a, d);
                        }
                    }
                }
            })?;
        }
        if matches!(scope, Scope::Working | Scope::Whole) {
            for path in untracked(root, budget)? {
                found.entry(path.clone()).or_insert(Change {
                    path,
                    old_path: None,
                    status: Status::Untracked,
                    adds: None,
                    dels: None,
                    binary: false,
                });
            }
        }
        let total = found.len();
        let mut files: Vec<_> = found.into_values().take(FILE_LIMIT).collect();
        for change in files.iter_mut().filter(|c| c.status == Status::Untracked) {
            untracked_counts(root, change);
        }
        Ok(Changes { files, total, truncated: total > FILE_LIMIT })
    }
    pub fn patch(
        self,
        root: &Path,
        scope: &Scope,
        path: Option<&str>,
        budget: Option<Duration>,
    ) -> Result<Patch, Error> {
        if path.is_some_and(|p| !valid_path(p)) {
            return Err(Error::InvalidPath);
        }
        let mut out = Vec::new();
        let mut truncated = false;
        let revs = revisions(root, scope, budget)?;
        if matches!(scope, Scope::Whole) && revs.is_empty() {
            return Ok(Patch::default());
        }
        let current = if matches!(scope, Scope::Branch) { head(root, budget)? } else { String::new() };
        for rev in revs {
            // `log.showSignature` would print verification text into the patch; the user's
            // `diff.noprefix`/`diff.mnemonicPrefix` would rename the a/ and b/ the parser matches.
            let mut args = if matches!(scope, Scope::Commits(_)) {
                vec!["show", "--format=", "--no-show-signature", "--root", "--first-parent", "-m"]
            } else {
                vec!["diff"]
            };
            args.extend([
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                "--relative",
                "-M",
                &rev,
            ]);
            if matches!(scope, Scope::Branch) {
                args.push(&current);
            }
            args.push("--");
            if let Some(p) = path {
                args.push(p);
            }
            let (data, cut) = run(root, &args, PATCH_LIMIT.saturating_sub(out.len()), budget)?;
            out.extend(data);
            truncated |= cut;
            if truncated {
                break;
            }
        }
        if !truncated && matches!(scope, Scope::Working | Scope::Whole) {
            for name in untracked(root, budget)?.into_iter().filter(|n| path.is_none_or(|p| p == n)) {
                let (data, cut) = untracked_patch(root, &name, PATCH_LIMIT.saturating_sub(out.len()))?;
                out.extend(data);
                truncated |= cut;
                if truncated {
                    break;
                }
            }
        }
        let mut text = String::from_utf8_lossy(&out).into_owned();
        if text.len() > PATCH_LIMIT {
            let mut end = PATCH_LIMIT;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            truncated = true;
        }
        Ok(Patch { text, truncated })
    }
    #[allow(dead_code)] // File search in the TUI and web will consume this.
    pub fn ignored_dirs(self, root: &Path, budget: Option<Duration>) -> Result<Vec<String>, Error> {
        let mut dirs = Vec::new();
        records(
            root,
            &["ls-files", "--others", "--ignored", "--exclude-standard", "--directory", "-z"],
            budget,
            |raw| {
                let path = String::from_utf8_lossy(raw).into_owned();
                if dirs.len() < FILE_LIMIT && path.ends_with('/') && valid_path(path.trim_end_matches('/')) {
                    dirs.push(path);
                }
            },
        )?;
        Ok(dirs)
    }
}

fn quoted(prefix: &str, name: &str) -> String {
    let s = format!("{prefix}{name}");
    if !s.bytes().any(|b| b < 32 || b == b'"' || b == b'\\' || b >= 127) {
        return s;
    }
    let mut out = String::from("\"");
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            32..=126 => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push('"');
    out
}
enum Opened {
    Regular(std::fs::File),
    Link,
    Other,
}
/// Open each component without following links; FIFOs cannot block the caller.
#[cfg(unix)]
fn open_regular(root: &Path, name: &str) -> std::io::Result<Opened> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let mut dir = std::fs::File::open(root)?;
    let components: Vec<_> = Path::new(name).components().collect();
    for (i, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else { return Ok(Opened::Other) };
        let part = CString::new(part.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let last = i + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: `part` is NUL-terminated and dir owns a live directory descriptor.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), part.as_ptr(), flags) };
        if fd < 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ELOOP) {
                return Ok(Opened::Link);
            }
            if e.raw_os_error() == Some(libc::ENOTDIR) {
                return Ok(Opened::Other);
            }
            return Err(e);
        }
        // SAFETY: openat returned a new, owned descriptor.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if last {
            return Ok(if file.metadata()?.is_file() { Opened::Regular(file) } else { Opened::Other });
        }
        dir = file;
    }
    Ok(Opened::Other)
}
#[cfg(not(unix))]
fn open_regular(root: &Path, name: &str) -> std::io::Result<Opened> {
    let root = root.canonicalize()?;
    let mut at = root.clone();
    for component in Path::new(name).components() {
        let Component::Normal(part) = component else { return Ok(Opened::Other) };
        at.push(part);
        if std::fs::symlink_metadata(&at)?.file_type().is_symlink() {
            return Ok(Opened::Link);
        }
    }
    let resolved = at.canonicalize()?;
    if !resolved.starts_with(&root) {
        return Ok(Opened::Other);
    }
    let file = std::fs::File::open(resolved)?;
    Ok(if file.metadata()?.is_file() { Opened::Regular(file) } else { Opened::Other })
}
fn untracked_counts(root: &Path, change: &mut Change) {
    use std::io::Read;
    let Ok(Opened::Regular(file)) = open_regular(root, &change.path) else { return };
    if !file.metadata().is_ok_and(|m| m.len() <= PATCH_LIMIT as u64) {
        return;
    }
    let mut data = Vec::new();
    if file.take(PATCH_LIMIT as u64 + 1).read_to_end(&mut data).is_err() || data.len() > PATCH_LIMIT {
        return;
    }
    if data.iter().take(8000).any(|b| *b == 0) {
        change.binary = true;
        return;
    }
    change.adds =
        Some(data.iter().filter(|&&b| b == b'\n').count() + usize::from(!data.is_empty() && !data.ends_with(b"\n")));
    change.dels = Some(0);
}
fn untracked_patch(root: &Path, name: &str, limit: usize) -> Result<(Vec<u8>, bool), Error> {
    use std::io::Read;
    let a = quoted("a/", name);
    let b = quoted("b/", name);
    let mut out = Vec::new();
    let mut cut = false;
    let mut append = |bytes: &[u8]| {
        let n = bytes.len().min(limit.saturating_sub(out.len()));
        out.extend_from_slice(&bytes[..n]);
        cut |= n < bytes.len();
    };
    append(format!("diff --git {a} {b}\nnew file mode 100644\n").as_bytes());
    let file = match open_regular(root, name)? {
        Opened::Regular(file) => file,
        other => {
            let kind = if matches!(other, Opened::Link) { "link" } else { "non-regular file" };
            append(format!("Binary files /dev/null and {b} differ ({kind})\n").as_bytes());
            return Ok((out, cut));
        }
    };
    let mut data = Vec::new();
    file.take(limit.saturating_add(1) as u64).read_to_end(&mut data)?;
    if data.iter().take(8000).any(|b| *b == 0) {
        append(format!("Binary files /dev/null and {b} differ\n").as_bytes());
        return Ok((out, cut));
    }
    if data.is_empty() {
        append(b"index 0000000..e69de29\n");
        return Ok((out, cut));
    }
    let too_long = data.len() > limit;
    if too_long {
        data.truncate(limit);
    }
    let lines = data.iter().filter(|&&b| b == b'\n').count() + usize::from(!data.is_empty() && !data.ends_with(b"\n"));
    append(format!("--- /dev/null\n+++ {b}\n@@ -0,0 +1,{lines} @@\n").as_bytes());
    for line in data.split_inclusive(|&b| b == b'\n') {
        append(b"+");
        append(line);
        if !line.ends_with(b"\n") && !too_long {
            append(b"\n\\ No newline at end of file\n");
        }
    }
    Ok((out, cut || too_long))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::run_git;
    use std::fs;
    #[test]
    fn working_changes_and_literal_paths() {
        let dir = crate::scratch::Scratch::new("vcs-working");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("old name"), "before\n").unwrap();
        fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::rename(root.join("old name"), root.join("new name")).unwrap();
        git(&["add", "-A"]);
        fs::write(root.join("--output=evil"), "untracked\n").unwrap();
        fs::create_dir(root.join("ignored")).unwrap();
        fs::write(root.join("ignored/x"), "x").unwrap();
        let before = fs::read(root.join(".git/index")).unwrap();
        let c = Backend::Git.changes(root, &Scope::Working, None).unwrap();
        assert!(
            c.files.iter().any(|f| f.path == "new name"
                && f.old_path.as_deref() == Some("old name")
                && f.status == Status::Renamed)
        );
        assert!(
            c.files.iter().any(|f| f.path == "--output=evil" && f.status == Status::Untracked && f.adds == Some(1))
        );
        fs::write(root.join("binary-new"), b"a\0b").unwrap();
        assert!(
            Backend::Git
                .changes(root, &Scope::Working, None)
                .unwrap()
                .files
                .iter()
                .any(|f| f.path == "binary-new" && f.binary && f.adds.is_none())
        );
        assert!(!c.files.iter().any(|f| f.path.starts_with("ignored/")));
        let p = Backend::Git.patch(root, &Scope::Working, Some("--output=evil"), None).unwrap();
        assert!(p.text.contains("+untracked"));
        assert!(!root.join("evil").exists());
        assert_eq!(before, fs::read(root.join(".git/index")).unwrap());
        assert_eq!(Backend::Git.ignored_dirs(root, None).unwrap(), ["ignored/"]);
        assert!(matches!(Backend::Git.patch(root, &Scope::Working, Some("../outside"), None), Err(Error::InvalidPath)));
    }
    #[cfg(unix)]
    #[test]
    fn untracked_symlinks_and_fifos_are_not_read() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;
        let dir = crate::scratch::Scratch::new("vcs-special");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        git(&["commit", "--allow-empty", "-qm", "base"]);
        let outside = crate::scratch::Scratch::new("vcs-secret");
        fs::write(outside.join("secret"), "SECRET OUTSIDE\n").unwrap();
        symlink(outside.join("secret"), root.join("link")).unwrap();
        let fifo = std::ffi::CString::new(root.join("fifo").as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo is a valid C path in the isolated test directory.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        fs::write(root.join(".gitignore"), ":(top)ignored\n").unwrap();
        for name in [":(top)ignored", ":(top)visible"] {
            let path = std::ffi::CString::new(root.join(name).as_os_str().as_bytes()).unwrap();
            // SAFETY: path is a valid C path in the isolated test directory.
            assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        }
        let list = Backend::Git.changes(root, &Scope::Working, None).unwrap();
        assert!(list.files.iter().any(|c| c.path == ":(top)visible"));
        assert!(!list.files.iter().any(|c| c.path == ":(top)ignored"));
        let p = Backend::Git.patch(root, &Scope::Working, None, Some(Duration::from_secs(2))).unwrap();
        assert!(!p.text.contains("SECRET OUTSIDE"));
        assert!(p.text.contains("link"));
        assert!(p.text.contains("non-regular file"));
    }
    #[test]
    fn a_project_inside_a_checkout_only_reports_its_own_relative_paths() {
        let dir = crate::scratch::Scratch::new("vcs-subdir");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/a"), "old\n").unwrap();
        fs::write(root.join("outside"), "old\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("sub/a"), "new\n").unwrap();
        fs::write(root.join("outside"), "new\n").unwrap();
        let sub = root.join("sub");
        let changes = Backend::Git.changes(&sub, &Scope::Working, None).unwrap();
        assert_eq!(changes.files.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["a"]);
        let patch = Backend::Git.patch(&sub, &Scope::Working, None, None).unwrap();
        assert!(patch.text.contains("a/a"));
        assert!(!patch.text.contains("outside"));
    }
    #[test]
    fn branch_and_whole_use_the_nearest_milestone_base() {
        let dir = crate::scratch::Scratch::new("vcs-branch");
        let main = dir.join("main");
        let side = dir.join("side");
        fs::create_dir(&main).unwrap();
        let git = |root: &Path, args: &[&str]| run_git(root, None, args);
        git(&main, &["init", "-q"]);
        fs::write(main.join("base"), "base\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-qm", "base"]);
        git(&main, &["branch", "milestone/m1"]);
        git(&main, &["worktree", "add", "-qb", "feature", side.to_str().unwrap(), "milestone/m1"]);
        fs::write(side.join("feature"), "committed\n").unwrap();
        git(&side, &["add", "."]);
        git(&side, &["commit", "-qm", "feature"]);
        fs::write(main.join("unrelated"), "main\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-qm", "main"]);
        fs::write(side.join("working"), "work\n").unwrap();
        let branch = Backend::Git.changes(&side, &Scope::Branch, None).unwrap();
        assert_eq!(branch.files.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["feature"]);
        let whole = Backend::Git.changes(&side, &Scope::Whole, None).unwrap();
        assert_eq!(whole.files.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["feature", "working"]);
        assert!(Backend::Git.changes(&main, &Scope::Branch, None).unwrap().files.is_empty());
    }
    #[test]
    fn picks_nearest_merge_base() {
        let dir = crate::scratch::Scratch::new("vcs-fork");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        git(&["commit", "--allow-empty", "-qm", "first"]);
        let first = head(root, None).unwrap();
        git(&["commit", "--allow-empty", "-qm", "second"]);
        let second = head(root, None).unwrap();
        git(&["commit", "--allow-empty", "-qm", "third"]);
        assert_eq!(fork_point(root, &[first, second.clone()], None).unwrap(), Some(second));
        assert!(fork_point(root, &[], None).unwrap().is_none());
    }
    #[test]
    fn change_list_is_limited_with_total() {
        let dir = crate::scratch::Scratch::new("vcs-many");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        git(&["commit", "--allow-empty", "-qm", "base"]);
        for n in 0..5001 {
            fs::write(root.join(format!("f{n:04}")), "").unwrap();
        }
        let c = Backend::Git.changes(root, &Scope::Working, None).unwrap();
        assert_eq!(c.total, 5001);
        assert_eq!(c.files.len(), 5000);
        assert!(c.truncated);
    }
    #[test]
    fn binary_numstat_and_external_driver() {
        let dir = crate::scratch::Scratch::new("vcs-binary");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("b"), b"a\0b").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        git(&["config", "diff.external", "touch SHOULD_NOT_RUN"]);
        fs::write(root.join("b"), b"c\0d").unwrap();
        let c = Backend::Git.changes(root, &Scope::Working, None).unwrap();
        assert!(c.files[0].binary);
        assert_eq!(c.files[0].adds, None);
        let _ = Backend::Git.patch(root, &Scope::Working, None, None).unwrap();
        assert!(!root.join("SHOULD_NOT_RUN").exists());
    }
    #[test]
    fn patch_limit_and_newline_names() {
        let dir = crate::scratch::Scratch::new("vcs-limit");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("한글\nname"), "before\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("한글\nname"), "x".repeat(PATCH_LIMIT + 100)).unwrap();
        let c = Backend::Git.changes(root, &Scope::Working, None).unwrap();
        assert_eq!(c.files[0].path, "한글\nname");
        let p = Backend::Git.patch(root, &Scope::Working, None, None).unwrap();
        assert!(p.truncated);
        assert!(p.text.len() <= PATCH_LIMIT);
        fs::write(root.join("한글\nname"), vec![0xff; 900_000]).unwrap();
        let p = Backend::Git.patch(root, &Scope::Working, None, None).unwrap();
        assert!(p.truncated);
        assert!(p.text.len() <= PATCH_LIMIT);
    }
    #[test]
    fn merges_use_the_first_parent_and_user_diff_config_keeps_the_prefixes() {
        let dir = crate::scratch::Scratch::new("vcs-merge");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("base"), "base\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        git(&["checkout", "-qb", "side"]);
        fs::write(root.join("side"), "side\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "side"]);
        git(&["checkout", "-q", "-"]);
        fs::write(root.join("main"), "main\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "main"]);
        git(&["merge", "-q", "--no-ff", "side", "-m", "merge"]);
        let merge = Scope::Commits(vec![head(root, None).unwrap()]);
        let listed = Backend::Git.changes(root, &merge, None).unwrap();
        assert_eq!(listed.files.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(), ["side"]);
        assert_eq!(listed.files[0].adds, Some(1));
        // An added file that a later commit edits is still the file this range added.
        let added = head(root, None).unwrap();
        fs::write(root.join("side"), "side\nmore\n").unwrap();
        git(&["commit", "-qam", "edit"]);
        let both = Scope::Commits(vec![added, head(root, None).unwrap()]);
        let side = Backend::Git.changes(root, &both, None).unwrap();
        assert_eq!(side.files[0].status, Status::Added);
        git(&["config", "diff.mnemonicPrefix", "true"]);
        git(&["config", "diff.noprefix", "true"]);
        fs::write(root.join("base"), "changed\n").unwrap();
        let patch = Backend::Git.patch(root, &Scope::Working, None, None).unwrap();
        let parsed = crate::diff::parse(&patch.text);
        assert_eq!(parsed[0].new_path.as_deref(), Some("base"), "{}", patch.text);
        assert_eq!(parsed[0].hunks.len(), 1);
    }
    #[test]
    fn root_commit_and_invalid_hash() {
        let dir = crate::scratch::Scratch::new("vcs-commit");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("a"), "hello\n").unwrap();
        git(&["add", "a"]);
        git(&["commit", "-qm", "root"]);
        let h = head(root, None).unwrap();
        let scope = Scope::Commits(vec![h.clone()]);
        assert!(Backend::Git.patch(root, &scope, None, None).unwrap().text.contains("+hello"));
        assert_eq!(Backend::Git.changes(root, &scope, None).unwrap().files[0].path, "a");
        fs::write(root.join("a"), "hello\nworld\n").unwrap();
        git(&["add", "a"]);
        git(&["commit", "-qm", "second"]);
        let both = Scope::Commits(vec![h, head(root, None).unwrap()]);
        let total = Backend::Git.changes(root, &both, None).unwrap();
        assert_eq!(total.files[0].adds, Some(2));
        assert_eq!(total.files[0].dels, Some(0));
        assert!(matches!(
            Backend::Git.changes(root, &Scope::Commits(vec!["--help".into()]), None),
            Err(Error::InvalidRevision)
        ));
    }
}
