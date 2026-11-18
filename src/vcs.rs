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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    pub status: Status,
    pub adds: Option<usize>,
    pub dels: Option<usize>,
    pub binary: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
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
    git::records_bounded(root, &all, budget, Some(1024 * 1024), take).ok_or(Error::Timeout)?.map_err(Error::Git)
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
        Scope::Branch => Ok(base(root, budget)?.into_iter().collect()),
        Scope::Whole => Ok(vec![match base(root, budget)? {
            Some(base) => base,
            None => head(root, budget)?,
        }]),
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
    let deadline = budget.and_then(|d| std::time::Instant::now().checked_add(d));
    let left = || deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()));
    let ignored = Backend::Git.ignored_dirs(root, left())?;
    let ignored: std::collections::HashSet<_> = ignored.iter().map(|s| s.trim_end_matches('/')).collect();
    let root_fd = std::fs::File::open(root)?;
    let mut pending = vec![String::new()];
    let mut found = Vec::new();
    'walk: while let Some(relative) = pending.pop() {
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            return Err(Error::Timeout);
        }
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
        for entry in crate::files::directory_entries(&dir)? {
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return Err(Error::Timeout);
            }
            let os_name = entry.name;
            let Some(name) = os_name.to_str() else { continue };
            if name.eq_ignore_ascii_case(".git") {
                continue;
            }
            let path = if relative.is_empty() { name.to_owned() } else { format!("{relative}/{name}") };
            if !valid_path(&path) {
                continue;
            }
            let kind = entry.mode & libc::S_IFMT;
            if kind == libc::S_IFDIR && !ignored.contains(path.as_str()) {
                pending.push(path);
            } else if kind != libc::S_IFDIR && kind != libc::S_IFREG && kind != libc::S_IFLNK {
                // check-ignore accepts literal paths after --, but rejects the
                // global --literal-pathspecs option used by diff commands.
                let literal = format!("./{path}");
                let result = git::bounded(
                    root,
                    &["--no-optional-locks", "check-ignore", "--quiet", "--", &literal],
                    1024,
                    left(),
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
fn untracked_of(changes: &Changes) -> Vec<String> {
    changes.files.iter().filter(|c| c.status == Status::Untracked).map(|c| c.path.clone()).collect()
}
/// How many bytes of names one patch read may put on Git's command line. Windows caps a whole command line at 32,767
/// characters; this leaves room for the rest of the arguments and the root's path.
pub(crate) const PATHSPEC_LIMIT: usize = 16 * 1024;
fn past_pathspec_limit(paths: &[String]) -> bool {
    paths.iter().fold(0usize, |n, p| n.saturating_add(p.len() + 1)) > PATHSPEC_LIMIT
}
/// Whether the pathspec `spec` — a file, or a folder with its trailing `/` — takes `name`.
fn covers(spec: &str, name: &str) -> bool {
    match spec.strip_suffix('/') {
        Some(dir) => name.strip_prefix(dir).is_some_and(|t| t.starts_with('/')),
        None => spec == name,
    }
}
// Filesystem operations can block independently of Git (for example on a dead mount).
// A timed caller must be released even when such a system call cannot be interrupted.
fn timed<T: Send + 'static>(
    budget: Option<Duration>,
    work: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let Some(budget) = budget else { return work() };
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .spawn(move || {
            let _ = tx.send(work());
        })
        .map_err(Error::Io)?;
    rx.recv_timeout(budget).map_err(|_| Error::Timeout)?
}
impl Backend {
    pub fn changes(self, root: &Path, scope: &Scope, budget: Option<Duration>) -> Result<Changes, Error> {
        let root = root.to_path_buf();
        let scope = scope.clone();
        timed(budget, move || self.changes_inner(&root, &scope, budget))
    }
    fn changes_inner(self, root: &Path, scope: &Scope, budget: Option<Duration>) -> Result<Changes, Error> {
        let deadline = budget.and_then(|d| std::time::Instant::now().checked_add(d));
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
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return Err(Error::Timeout);
            }
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
        self.patch_known(root, scope, path.map(|p| vec![p.to_owned()]), budget, None, true)
    }
    pub fn patch_with_changes(
        self,
        root: &Path,
        scope: &Scope,
        path: Option<&str>,
        budget: Option<Duration>,
        changes: &Changes,
    ) -> Result<Patch, Error> {
        self.patch_known(root, scope, path.map(|p| vec![p.to_owned()]), budget, Some(untracked_of(changes)), true)
    }
    /// The patch of some files and folders only — a folder is named with its trailing `/` and takes every file below
    /// it, untracked ones too. Give a renamed file by both of its names: Git narrows to the pathspec before it looks
    /// for renames, so the new name alone reads as a whole new file. `changes`, when given, names the untracked files
    /// as [`Backend::patch_with_changes`] does. An empty list is refused rather than read as "everything".
    ///
    /// A folder narrowed with rows that hold no rename is read without rename detection: past `diff.renameLimit` the
    /// whole-scope list skips inexact renames, and the narrowed read, with fewer candidates, would pair a deleted and
    /// an added file the list shows apart — their hunks would no longer match the rows.
    pub fn patch_of(
        self,
        root: &Path,
        scope: &Scope,
        paths: &[String],
        budget: Option<Duration>,
        changes: Option<&Changes>,
    ) -> Result<Patch, Error> {
        if paths.is_empty() {
            return Err(Error::InvalidPath);
        }
        let folder = paths.iter().any(|p| p.ends_with('/'));
        let renames = !folder
            || changes.is_none_or(|c| c.files.iter().any(|f| matches!(f.status, Status::Renamed | Status::Copied)));
        self.patch_known(root, scope, Some(paths.to_vec()), budget, changes.map(untracked_of), renames)
    }
    fn patch_known(
        self,
        root: &Path,
        scope: &Scope,
        paths: Option<Vec<String>>,
        budget: Option<Duration>,
        known: Option<Vec<String>>,
        renames: bool,
    ) -> Result<Patch, Error> {
        let root = root.to_path_buf();
        let scope = scope.clone();
        timed(budget, move || self.patch_inner(&root, &scope, paths.as_deref(), budget, known, renames))
    }
    fn patch_inner(
        self,
        root: &Path,
        scope: &Scope,
        paths: Option<&[String]>,
        budget: Option<Duration>,
        known: Option<Vec<String>>,
        renames: bool,
    ) -> Result<Patch, Error> {
        let deadline = budget.and_then(|d| std::time::Instant::now().checked_add(d));
        // A folder is checked without its one trailing slash; every name — an old name from the list too — is
        // someone's text and reaches Git only as a literal pathspec after `--`.
        if paths.is_some_and(|ps| ps.iter().any(|p| !valid_path(p.strip_suffix('/').unwrap_or(p)))) {
            return Err(Error::InvalidPath);
        }
        // Past the limit the names do not go on the command line: the scope is read whole and the callers keep their
        // files' hunks, as they did before. A slow answer is better than a wrong one.
        let narrow = paths.filter(|ps| !past_pathspec_limit(ps));
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
                if renames || narrow.is_none() { "-M" } else { "--no-renames" },
                &rev,
            ]);
            if matches!(scope, Scope::Branch) {
                args.push(&current);
            }
            args.push("--");
            if let Some(ps) = narrow {
                args.extend(ps.iter().map(String::as_str));
            }
            let (data, cut) = run(root, &args, PATCH_LIMIT.saturating_sub(out.len()), budget)?;
            out.extend(data);
            truncated |= cut;
            if truncated {
                break;
            }
        }
        if !truncated && matches!(scope, Scope::Working | Scope::Whole) {
            let names = match known {
                Some(names) => names,
                None => untracked(root, budget)?,
            };
            for name in names.into_iter().filter(|n| paths.is_none_or(|ps| ps.iter().any(|p| covers(p, n)))) {
                if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                    return Err(Error::Timeout);
                }
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
                if path.ends_with('/') && valid_path(path.trim_end_matches('/')) {
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

/// The four letters a screen puts in a changed path's first column. Git's eight kinds fold into them: copied and
/// untracked are new files, type-changed and conflicted are changed ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Mark {
    #[serde(rename = "M")]
    Modified,
    #[serde(rename = "A")]
    Added,
    #[serde(rename = "D")]
    Deleted,
    #[serde(rename = "R")]
    Renamed,
}
impl Mark {
    pub fn of(status: Status) -> Self {
        match status {
            Status::Modified | Status::TypeChanged | Status::Conflicted => Self::Modified,
            Status::Added | Status::Untracked | Status::Copied => Self::Added,
            Status::Deleted => Self::Deleted,
            Status::Renamed => Self::Renamed,
        }
    }
    /// The letter the TUI's file tree and diff heads print (moai-p7nr.c67); the web sends the value itself.
    pub fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }
}
/// Every changed file's mark and every ancestor directory's, read once for a whole list. A directory stands `A` when
/// the base's tree does not have it, `D` when the base has it and the disk does not, and `M` otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub files: BTreeMap<String, Mark>,
    pub dirs: BTreeMap<String, Mark>,
    /// The list was cut at its limit: the directories above the files left out carry no mark, which means "unknown",
    /// not "unchanged".
    pub truncated: bool,
}
/// What the directories are, read beside the list. `None` when it could not be read — the directories then stand `M`:
/// saying less is cheaper than saying something false.
#[derive(Debug, Clone, Default)]
pub struct DirFacts {
    /// Candidates the base's tree does not hold.
    pub not_at_base: std::collections::HashSet<String>,
    /// Candidates the checkout's disk does not hold as a directory (a link in a directory's place counts as missing).
    pub not_on_disk: std::collections::HashSet<String>,
}
/// The directories above each changed file's path — never above a renamed file's old path — deepest last.
fn ancestors(changes: &Changes) -> std::collections::BTreeSet<String> {
    let mut dirs = std::collections::BTreeSet::new();
    for change in &changes.files {
        let mut at = change.path.as_str();
        while let Some(i) = at.rfind('/') {
            at = &at[..i];
            if at.is_empty() || !dirs.insert(at.to_owned()) {
                break;
            }
        }
    }
    dirs
}
/// The marks of a list and its directories, from facts read beforehand. Pure, so the TUI's file window and the web
/// build the same map ([`Backend::marks`] reads the facts for both).
pub fn marks(changes: &Changes, facts: Option<&DirFacts>) -> Marks {
    let files = changes.files.iter().map(|c| (c.path.clone(), Mark::of(c.status))).collect();
    let dirs = ancestors(changes)
        .into_iter()
        .map(|dir| {
            let mark = match facts {
                Some(f) if f.not_at_base.contains(&dir) => Mark::Added,
                Some(f) if f.not_on_disk.contains(&dir) => Mark::Deleted,
                _ => Mark::Modified,
            };
            (dir, mark)
        })
        .collect();
    Marks { files, dirs, truncated: changes.truncated }
}
/// Whether `dir` (relative, `/`-separated) stands under `root` as a directory, every component opened without following
/// a link. `None` when the disk would not say.
#[cfg(unix)]
fn dir_on_disk(root: &Path, dir: &str) -> Option<bool> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let mut at = std::fs::File::open(root).ok()?;
    for component in Path::new(dir).components() {
        let Component::Normal(part) = component else { return Some(false) };
        let part = CString::new(part.as_bytes()).ok()?;
        // SAFETY: `part` is NUL-terminated and `at` owns a live directory descriptor.
        let fd = unsafe {
            libc::openat(
                at.as_raw_fd(),
                part.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let e = std::io::Error::last_os_error().raw_os_error();
            return matches!(e, Some(libc::ENOENT | libc::ENOTDIR | libc::ELOOP)).then_some(false);
        }
        // SAFETY: openat returned a new, owned descriptor.
        at = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Some(true)
}
#[cfg(not(unix))]
fn dir_on_disk(root: &Path, dir: &str) -> Option<bool> {
    let mut at = root.to_path_buf();
    for component in Path::new(dir).components() {
        let Component::Normal(part) = component else { return Some(false) };
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            Ok(_) => return Some(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(false),
            Err(_) => return None,
        }
    }
    Some(true)
}
impl Backend {
    /// The marks of `changes`, a [`Scope::Whole`] list of `root`. Two facts are read for the directories above the
    /// files — one Git call for the base's tree, whatever the count, and the disk without following links — inside
    /// `budget`. When the tree cannot be read, the list still gets its marks and the directories stand `M`; a directory
    /// the disk will not answer for stands `M` by itself.
    pub fn marks(self, root: &Path, changes: &Changes, budget: Option<Duration>) -> Marks {
        let candidates = ancestors(changes);
        if candidates.is_empty() {
            return marks(changes, None);
        }
        let owned = root.to_path_buf();
        let facts = timed(budget, move || self.dir_facts(&owned, candidates, budget)).ok();
        marks(changes, facts.as_ref())
    }
    fn dir_facts(
        self,
        root: &Path,
        candidates: std::collections::BTreeSet<String>,
        budget: Option<Duration>,
    ) -> Result<DirFacts, Error> {
        let deadline = budget.and_then(|d| std::time::Instant::now().checked_add(d));
        let left = || deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()));
        // The base `Scope::Whole` compares against: the fork point in a linked worktree, HEAD in the main checkout.
        let Some(rev) = revisions(root, &Scope::Whole, left())?.pop() else { return Err(Error::InvalidRevision) };
        // A name that is not UTF-8 reached the list through a lossy decode: its text names neither the tree's entry nor
        // the disk's, so neither fact can be read for it and it stands `M`.
        let lossy = |d: &str| d.contains(char::REPLACEMENT_CHARACTER);
        let mut not_at_base = candidates.clone();
        // `ls-tree` lists the tree's directories relative to `root`, the same base `--relative` gives the list.
        records(root, &["ls-tree", "-r", "-d", "--name-only", "-z", &rev], left(), |raw| {
            not_at_base.remove(String::from_utf8_lossy(raw).as_ref());
        })?;
        not_at_base.retain(|d| !lossy(d));
        let mut not_on_disk = std::collections::HashSet::new();
        for dir in candidates.iter().filter(|d| !not_at_base.contains(*d) && !lossy(d)) {
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return Err(Error::Timeout);
            }
            // A directory the disk would not answer for (no permission, say) stands `M` alone; the others keep theirs.
            if dir_on_disk(root, dir) == Some(false) {
                not_on_disk.insert(dir.clone());
            }
        }
        Ok(DirFacts { not_at_base: not_at_base.into_iter().collect(), not_on_disk })
    }
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
    fn filesystem_worker_releases_a_timed_caller() {
        let start = std::time::Instant::now();
        let result = timed(Some(Duration::from_millis(10)), || {
            std::thread::sleep(Duration::from_millis(300));
            Ok(())
        });
        assert!(matches!(result, Err(Error::Timeout)));
        assert!(start.elapsed() < Duration::from_millis(250));
    }
    #[test]
    fn main_whole_includes_working_changes_and_known_patch_paths() {
        let dir = crate::scratch::Scratch::new("whole-main");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("tracked"), "old\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("tracked"), "new\n").unwrap();
        fs::write(root.join("new"), "untracked\n").unwrap();
        let c = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        assert_eq!(c.total, 2);
        fs::write(root.join("appeared-later"), "later\n").unwrap();
        let patch = Backend::Git.patch_with_changes(root, &Scope::Whole, None, None, &c).unwrap();
        assert!(patch.text.contains("+new") && patch.text.contains("+untracked"));
        assert!(!patch.text.contains("appeared-later"));
        assert!(Backend::Git.patch(root, &Scope::Whole, None, None).unwrap().text.contains("appeared-later"));
        assert!(matches!(Backend::Git.changes(root, &Scope::Whole, Some(Duration::ZERO)), Err(Error::Timeout)));
    }
    #[test]
    fn ignored_directories_are_not_cut_at_the_changed_file_limit() {
        let dir = crate::scratch::Scratch::new("ignored-many");
        let root: &Path = &dir;
        run_git(root, None, &["init", "-q"]);
        fs::write(root.join(".gitignore"), "ignored*/\n").unwrap();
        for n in 0..5001 {
            let path = root.join(format!("ignored{n:04}"));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("file"), "").unwrap();
        }
        let dirs = Backend::Git.ignored_dirs(root, None).unwrap();
        assert_eq!(dirs.len(), 5001);
        assert!(dirs.iter().any(|d| d == "ignored5000/"));
    }
    #[test]
    fn long_commit_body_does_not_break_commit_selection() {
        let dir = crate::scratch::Scratch::new("long-commit-body");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        let message = root.join("message");
        fs::write(&message, format!("subject\n\n{}\n\nRefs: moai-test123\n", "x".repeat(1024 * 1024 + 100))).unwrap();
        git(&["commit", "--allow-empty", "-q", "-F", message.to_str().unwrap()]);
        let rows = git::commits_within(root, "moai-test123", None).unwrap().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject, "subject");
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

    fn change(path: &str, status: Status) -> Change {
        Change { path: path.into(), old_path: None, status, adds: None, dels: None, binary: false }
    }
    fn dirs_of(m: &Marks) -> Vec<(&str, &str)> {
        m.dirs.iter().map(|(d, k)| (d.as_str(), k.letter())).collect()
    }
    #[test]
    fn eight_kinds_fold_into_four_letters() {
        use Status::*;
        let folded: Vec<_> = [Modified, TypeChanged, Conflicted, Added, Untracked, Copied, Deleted, Renamed]
            .into_iter()
            .map(|s| Mark::of(s).letter())
            .collect();
        assert_eq!(folded, ["M", "M", "M", "A", "A", "A", "D", "R"]);
        assert_eq!(serde_json::to_value(Mark::Renamed).unwrap(), "R");
    }
    #[test]
    fn directories_take_the_facts_and_fall_back_to_m() {
        let mut moved = change("new/deep/file", Status::Renamed);
        moved.old_path = Some("old/file".into());
        let changes = Changes {
            files: vec![change("kept/edited", Status::Modified), moved, change("gone/x", Status::Deleted)],
            total: 3,
            truncated: false,
        };
        let facts = DirFacts {
            not_at_base: ["new".into(), "new/deep".into()].into_iter().collect(),
            not_on_disk: ["gone".into()].into_iter().collect(),
        };
        let m = marks(&changes, Some(&facts));
        assert_eq!(m.files["new/deep/file"], Mark::Renamed);
        assert_eq!(dirs_of(&m), [("gone", "D"), ("kept", "M"), ("new", "A"), ("new/deep", "A")]);
        // The old name's directory is not a candidate.
        assert!(!m.dirs.contains_key("old"));
        // Facts that could not be read never make an `A` or a `D` up.
        let blind = marks(&changes, None);
        assert!(blind.dirs.values().all(|k| *k == Mark::Modified));
        assert_eq!(blind.files.len(), 3);
    }
    #[test]
    fn directory_marks_read_the_base_tree_and_the_disk() {
        let dir = crate::scratch::Scratch::new("vcs-marks");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        for f in ["grow/a", "shrink/a", "shrink/b", "gone/a", "gone/sub/b", "leftover/a", "move/a", "lnk/a"] {
            fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
            fs::write(root.join(f), format!("{f}\none\ntwo\nthree\n")).unwrap();
        }
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("grow/new"), "new\n").unwrap();
        fs::remove_file(root.join("shrink/b")).unwrap();
        fs::remove_dir_all(root.join("gone")).unwrap();
        fs::remove_file(root.join("leftover/a")).unwrap();
        fs::write(root.join("leftover/x.log"), "ignored\n").unwrap();
        fs::create_dir_all(root.join("fresh/inner")).unwrap();
        fs::write(root.join("fresh/inner/f"), "f\n").unwrap();
        fs::create_dir_all(root.join("-x\ny")).unwrap();
        fs::write(root.join("-x\ny/f"), "f\n").unwrap();
        fs::create_dir(root.join("to")).unwrap();
        git(&["mv", "move/a", "to/a"]);
        // A link where a directory stood is read as no directory, never followed out of the checkout.
        fs::remove_dir_all(root.join("lnk")).unwrap();
        #[cfg(unix)]
        let outside = crate::scratch::Scratch::new("vcs-marks-outside");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&*outside, root.join("lnk")).unwrap();
        let c = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(root, &c, None);
        assert_eq!(m.files["to/a"], Mark::Renamed, "{c:?}");
        assert_eq!(m.files["fresh/inner/f"], Mark::Added);
        let got: BTreeMap<_, _> = dirs_of(&m).into_iter().collect();
        assert_eq!(got["grow"], "M", "a directory that only gained a file is not new");
        assert_eq!(got["shrink"], "M", "a directory that lost one file is not gone");
        assert_eq!(got["gone"], "D");
        assert_eq!(got["gone/sub"], "D");
        assert_eq!(got["leftover"], "M", "ignored leftovers keep the directory on disk");
        assert_eq!(got["fresh"], "A");
        assert_eq!(got["fresh/inner"], "A");
        assert_eq!(got["-x\ny"], "A");
        assert_eq!(got["to"], "A");
        assert!(!got.contains_key("move"));
        #[cfg(unix)]
        assert_eq!(got["lnk"], "D");
        assert!(!m.truncated);
        // Without the facts — here, no time left — the list keeps its marks and the directories stand `M`.
        let blind = Backend::Git.marks(root, &c, Some(Duration::ZERO));
        assert_eq!(blind.files, m.files);
        assert!(blind.dirs.values().all(|k| *k == Mark::Modified));
    }
    #[test]
    fn directory_marks_compare_against_the_fork_point_and_the_project() {
        let dir = crate::scratch::Scratch::new("vcs-marks-branch");
        let main = dir.join("main");
        let side = dir.join("side");
        fs::create_dir_all(main.join("proj/old")).unwrap();
        let git = |root: &Path, args: &[&str]| run_git(root, None, args);
        git(&main, &["init", "-q"]);
        fs::write(main.join("proj/old/a"), "a\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-qm", "base"]);
        git(&main, &["branch", "milestone/m1"]);
        git(&main, &["worktree", "add", "-qb", "feature", side.to_str().unwrap(), "milestone/m1"]);
        fs::create_dir_all(side.join("proj/made")).unwrap();
        fs::write(side.join("proj/made/b"), "b\n").unwrap();
        fs::write(side.join("proj/old/c"), "c\n").unwrap();
        git(&side, &["add", "."]);
        git(&side, &["commit", "-qm", "feature"]);
        // Committed on the branch: HEAD has the directory, the fork point does not.
        let c = Backend::Git.changes(&side, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(&side, &c, None);
        assert_eq!(dirs_of(&m), [("proj", "M"), ("proj/made", "A"), ("proj/old", "M")]);
        // A project inside the checkout reads relative paths on both sides.
        let sub = side.join("proj");
        let c = Backend::Git.changes(&sub, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(&sub, &c, None);
        assert_eq!(dirs_of(&m), [("made", "A"), ("old", "M")]);
        // The main checkout has no fork point and compares against HEAD.
        fs::create_dir(main.join("proj/new")).unwrap();
        fs::write(main.join("proj/new/x"), "x\n").unwrap();
        let c = Backend::Git.changes(&main, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(&main, &c, None);
        assert_eq!(dirs_of(&m), [("proj", "M"), ("proj/new", "A")]);
    }
    /// The hunks of `name` in a patch: by its new name, or by its old one when it was deleted.
    fn hunks_of(text: &str, name: &str) -> Vec<crate::diff::Hunk> {
        crate::diff::parse(text)
            .into_iter()
            .filter(|f| {
                f.new_path.as_deref() == Some(name) || (f.new_path.is_none() && f.old_path.as_deref() == Some(name))
            })
            .flat_map(|f| f.hunks)
            .collect()
    }
    fn counted(hunks: &[crate::diff::Hunk]) -> (usize, usize) {
        use crate::diff::LineKind;
        let lines = hunks.iter().flat_map(|h| &h.lines);
        let adds = lines.clone().filter(|l| l.kind == LineKind::Add).count();
        (adds, lines.filter(|l| l.kind == LineKind::Delete).count())
    }
    fn body(tag: &str) -> String {
        (0..20).map(|n| format!("{tag} line {n}\n")).collect()
    }
    #[test]
    fn one_renamed_file_reads_by_both_names_in_every_scope() {
        let dir = crate::scratch::Scratch::new("vcs-rename-one");
        let main = dir.join("main");
        let side = dir.join("side");
        fs::create_dir(&main).unwrap();
        let git = |root: &Path, args: &[&str]| run_git(root, None, args);
        git(&main, &["init", "-q"]);
        fs::write(main.join("-old\nname"), body("a")).unwrap();
        fs::write(main.join("other"), "other\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-qm", "base"]);
        git(&main, &["branch", "milestone/m1"]);
        git(&main, &["worktree", "add", "-qb", "feature", side.to_str().unwrap(), "milestone/m1"]);
        // One commit renames, the next edits three lines under the new name.
        git(&side, &["mv", "--", "-old\nname", "new"]);
        git(&side, &["commit", "-qm", "rename"]);
        let renamed = head(&side, None).unwrap();
        fs::write(side.join("new"), body("a").replacen("a line 3", "b line 3", 1).replacen("a line 9", "b 9", 1))
            .unwrap();
        fs::write(side.join("other"), "changed\n").unwrap();
        git(&side, &["commit", "-qam", "edit"]);
        let edited = head(&side, None).unwrap();
        let names = vec!["new".to_owned(), "-old\nname".to_owned()];
        let scopes = [
            Scope::Commits(vec![renamed.clone(), edited.clone()]),
            Scope::Branch,
            Scope::Whole,
            Scope::Commits(vec![edited.clone()]),
        ];
        for scope in &scopes {
            let list = Backend::Git.changes(&side, scope, None).unwrap();
            let row = list.files.iter().find(|c| c.path == "new").unwrap();
            let got = Backend::Git.patch_of(&side, scope, &names, None, Some(&list)).unwrap();
            let whole = Backend::Git.patch(&side, scope, None, None).unwrap();
            let hunks = hunks_of(&got.text, "new");
            assert_eq!(hunks, hunks_of(&whole.text, "new"), "{scope:?}");
            assert_eq!(Some(counted(&hunks).0), row.adds, "{scope:?}");
            assert_eq!(Some(counted(&hunks).1), row.dels, "{scope:?}");
            assert!(counted(&hunks).0 < 5, "{scope:?} read the renamed file as a whole new one: {}", got.text);
            assert!(!got.text.contains("changed"), "{scope:?} read the whole scope");
        }
        // A commit at a time, as `moai diff` reads a commit range: both names on each still give each its hunks.
        let first = Backend::Git.patch_of(&side, &Scope::Commits(vec![renamed]), &names, None, None).unwrap();
        assert!(first.text.contains("rename from"), "{}", first.text);
        let second = Backend::Git.patch_of(&side, &Scope::Commits(vec![edited]), &names, None, None).unwrap();
        assert_eq!(counted(&hunks_of(&second.text, "new")), (2, 2));
        // Working tree: an unstaged rename stays a deleted file and an untracked one; staged, both names find it.
        git(&side, &["mv", "new", "newer"]);
        fs::write(side.join("newer"), body("a")).unwrap();
        let list = Backend::Git.changes(&side, &Scope::Working, None).unwrap();
        let row = list.files.iter().find(|c| c.path == "newer").unwrap();
        assert_eq!(row.status, Status::Renamed);
        let names = vec!["newer".to_owned(), "new".to_owned()];
        let got = Backend::Git.patch_of(&side, &Scope::Working, &names, None, Some(&list)).unwrap();
        assert_eq!(counted(&hunks_of(&got.text, "newer")), (row.adds.unwrap(), row.dels.unwrap()));
    }
    #[test]
    fn a_folder_reads_its_files_untracked_and_crossing_renames_only() {
        let dir = crate::scratch::Scratch::new("vcs-folder");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        for f in ["src/a", "src/in-out", "out-in", "src2/x", "top"] {
            fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
            fs::write(root.join(f), body(f)).unwrap();
        }
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("src/a"), body("src/a").replacen("line 2", "LINE 2", 1)).unwrap();
        fs::write(root.join("src2/x"), "next door\n").unwrap();
        fs::write(root.join("top"), "top changed\n").unwrap();
        git(&["mv", "src/in-out", "moved-out"]);
        git(&["mv", "out-in", "src/moved-in"]);
        fs::create_dir(root.join("src/deep")).unwrap();
        fs::write(root.join("src/deep/new"), "untracked below\n").unwrap();
        fs::write(root.join("src2/new"), "untracked next door\n").unwrap();
        fs::write(root.join("loose"), "untracked outside\n").unwrap();
        let list = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        let below = |p: &str| p.strip_prefix("src").is_some_and(|t| t.starts_with('/'));
        let mut rows = list.clone();
        rows.files.retain(|c| below(&c.path) || c.old_path.as_deref().is_some_and(below));
        let mut ask = vec!["src/".to_owned()];
        for c in &rows.files {
            ask.extend(std::iter::once(&c.path).chain(&c.old_path).filter(|n| !below(n)).cloned());
        }
        assert_eq!(ask, ["src/", "moved-out", "out-in"]);
        let got = Backend::Git.patch_of(root, &Scope::Whole, &ask, None, Some(&rows)).unwrap();
        let whole = Backend::Git.patch_with_changes(root, &Scope::Whole, None, None, &list).unwrap();
        for c in &rows.files {
            assert_eq!(hunks_of(&got.text, &c.path), hunks_of(&whole.text, &c.path), "{}", c.path);
            assert_eq!(counted(&hunks_of(&got.text, &c.path)), (c.adds.unwrap(), c.dels.unwrap()), "{}", c.path);
        }
        assert!(got.text.contains("untracked below"));
        for outside in ["next door", "untracked next door", "untracked outside", "top changed"] {
            assert!(!got.text.contains(outside), "{outside} is not in src/");
        }
        // Past the limit the names stay off the command line: the scope is read whole, still the same hunks.
        let many: Vec<String> =
            ask.iter().cloned().chain((0..PATHSPEC_LIMIT / 8).map(|n| format!("x{n:06}"))).collect();
        assert!(past_pathspec_limit(&many));
        assert!(!past_pathspec_limit(&ask));
        let fell = Backend::Git.patch_of(root, &Scope::Whole, &many, None, Some(&rows)).unwrap();
        assert!(fell.text.contains("top changed"), "past the limit the scope is read whole");
        assert!(!fell.text.contains("untracked outside"), "the untracked files still follow the names");
        for c in &rows.files {
            assert_eq!(hunks_of(&fell.text, &c.path), hunks_of(&got.text, &c.path), "{}", c.path);
        }
    }
    #[test]
    fn a_folder_pairs_no_rename_its_list_did_not() {
        let dir = crate::scratch::Scratch::new("vcs-folder-rename-limit");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/x"), body("x")).unwrap();
        fs::write(root.join("out"), body("out")).unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        // Two inexact pairs are past a limit of one: the whole list skips them. The folder alone has one pair.
        git(&["config", "diff.renameLimit", "1"]);
        for (from, to, tag) in [("src/x", "src/y", "x"), ("out", "out2", "out")] {
            fs::remove_file(root.join(from)).unwrap();
            fs::write(root.join(to), body(tag).replacen("line 4", "LINE 4", 1)).unwrap();
        }
        git(&["add", "-A"]);
        let list = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        let mut rows = list.clone();
        rows.files.retain(|c| c.path.starts_with("src/"));
        assert!(rows.files.iter().all(|c| !matches!(c.status, Status::Renamed)), "{rows:?}");
        let got = Backend::Git.patch_of(root, &Scope::Whole, &["src/".to_owned()], None, Some(&rows)).unwrap();
        let whole = Backend::Git.patch_with_changes(root, &Scope::Whole, None, None, &list).unwrap();
        for c in &rows.files {
            assert_eq!(hunks_of(&got.text, &c.path), hunks_of(&whole.text, &c.path), "{}: {}", c.path, got.text);
            assert_eq!(counted(&hunks_of(&got.text, &c.path)), (c.adds.unwrap(), c.dels.unwrap()), "{}", c.path);
        }
    }
    #[test]
    fn names_reach_git_only_as_literal_paths() {
        let dir = crate::scratch::Scratch::new("vcs-names");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        fs::write(root.join("a"), "a\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("a"), "b\n").unwrap();
        for bad in ["../x", ".git/config", "", "/abs", "a/../../x", "/"] {
            let names = vec!["a".to_owned(), bad.to_owned()];
            assert!(
                matches!(Backend::Git.patch_of(root, &Scope::Working, &names, None, None), Err(Error::InvalidPath)),
                "{bad:?}"
            );
        }
        assert!(matches!(Backend::Git.patch_of(root, &Scope::Working, &[], None, None), Err(Error::InvalidPath)));
        let names = vec!["a".to_owned(), "--output=evil".to_owned(), ":(top)a".to_owned()];
        let got = Backend::Git.patch_of(root, &Scope::Working, &names, None, None).unwrap();
        assert!(got.text.contains("+b"));
        assert!(!root.join("evil").exists());
    }
    #[test]
    fn a_cut_list_says_so_in_its_marks() {
        let changes = Changes { files: vec![change("a/b", Status::Untracked)], total: FILE_LIMIT + 1, truncated: true };
        assert!(marks(&changes, None).truncated);
    }
    /// A directory the disk will not open (read permission taken away) stands `M` by itself — the other directories
    /// keep their `A` and `D`. Run as root the disk opens it anyway, and the marks are the same.
    #[cfg(unix)]
    #[test]
    fn a_directory_the_disk_will_not_open_stands_m_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::scratch::Scratch::new("vcs-marks-locked");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        for f in ["locked/a", "gone/a"] {
            fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
            fs::write(root.join(f), "a\n").unwrap();
        }
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(root.join("locked/a"), "b\n").unwrap();
        fs::remove_dir_all(root.join("gone")).unwrap();
        fs::create_dir(root.join("fresh")).unwrap();
        fs::write(root.join("fresh/f"), "f\n").unwrap();
        // Search, no read: git still reaches `locked/a` by name, the directory itself will not open.
        struct Unlock(std::path::PathBuf);
        impl Drop for Unlock {
            fn drop(&mut self) {
                let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
            }
        }
        let _unlock = Unlock(root.join("locked"));
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o311)).unwrap();
        let c = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(root, &c, None);
        let got: BTreeMap<_, _> = dirs_of(&m).into_iter().collect();
        assert_eq!(got, BTreeMap::from([("fresh", "A"), ("gone", "D"), ("locked", "M")]), "{c:?}");
    }
    /// A directory whose name is not UTF-8 reaches the list through a lossy decode; neither fact can be read for that
    /// text, so it stands `M` — never an `A` the base's tree would contradict.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_directory_named_outside_utf8_stands_m() {
        use std::os::unix::ffi::OsStrExt;
        let dir = crate::scratch::Scratch::new("vcs-marks-bytes");
        let root: &Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        let odd = root.join(std::ffi::OsStr::from_bytes(b"odd\xff"));
        fs::create_dir(&odd).unwrap();
        fs::write(odd.join("a"), "a\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        fs::write(odd.join("a"), "b\n").unwrap();
        let c = Backend::Git.changes(root, &Scope::Whole, None).unwrap();
        let m = Backend::Git.marks(root, &c, None);
        assert_eq!(dirs_of(&m), [("odd\u{fffd}", "M")], "{c:?}");
    }
}
