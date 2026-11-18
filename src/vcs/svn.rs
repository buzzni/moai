//! The svn backend: a working copy's changes, its patch and its ignored directories (moai-0d0s.s6g).
//!
//! **It only reads.** Every call goes through [`crate::svn`], which can spawn nothing but `info`·`status`·`diff`·
//! `log` and gives `diff` `--internal-diff` (no user `diff-cmd`) and `status` `--ignore-externals`. Measured on svn
//! 1.14.3: `status` and `diff` leave `.svn` — names, sizes, mtimes, `wc.db` bytes — as they found it.
//!
//! What svn says, as measured on 1.14.3, and what this module does with it:
//! - `status --xml` carries no node kind. A path the disk holds as a directory is one; every `deleted`/`missing` entry
//!   is opened with `info --depth infinity`, which does give kinds — an `svn rm`'d directory is reported as that one
//!   line, its files are not. A copied (`svn mv`'d) directory is opened the same way: its unmodified files are not in
//!   `status` either, while `diff --show-copies-as-adds` prints each as added
//! - `status` does not walk an unversioned directory; [`walk`] does, without following links and skipping `.svn`
//! - entries in a changelist stand under `<changelist>`, not `<target>`
//! - a root that is not versioned answers `status` with exit 0 — a warning, or `.` as unversioned — so that case is
//!   asked of `info`, which fails
//! - a working-copy `diff` reads its targets literally, with no peg ([`crate::svn::Arg::Local`]). Narrowed to a file
//!   inside an `svn rm`'d or copied directory it prints nothing, so such a name is read through its directory and the
//!   other files' sections are dropped ([`keep_sections`])
//! - the scopes: `working` is BASE against the working copy; `commits` reads each revision `r<n>` from the
//!   repository, the root's URL at its BASE — the revisions themselves come from [`crate::svn::commits_within`]
//!   (moai-0d0s.4o8). `branch` and `whole` go by where the working copy stands ([`crate::svn::layout`], moai-0d0s.x27):
//!   on a branch copied from somewhere, `branch` is that fork point against each node's BASE (`./@BASE` — not the
//!   root's revision, which `svn commit` leaves behind until an update), and `whole` the fork
//!   point against the working copy with untracked files from `status`; on the trunk `branch` is empty; with no fork
//!   point known (no convention, a branch made by `svn mkdir`) `branch` is [`super::NoBase`] with its reason. Wherever
//!   no fork point is known, `whole` is `working` — no server asked ([`reach`])
//! - a URL-against-working-copy `diff` fails outright (`E000002 Can't open file`) while a file it would read is missing
//!   (`!`, removed without `svn rm`), and a fork-side path that did not exist at the fork point fails with `E160005`.
//!   Both reach the caller as svn's failure: a list without them would be a wrong one

use super::{Change, Changes, Error, FILE_LIMIT, LIST_LIMIT, PATCH_LIMIT, Patch, Scope, Status};
use crate::svn::{Arg, Sub};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};
use std::time::{Duration, Instant};

/// What a deadline leaves, read between calls.
struct Deadline(Option<Instant>);
impl Deadline {
    fn of(budget: Option<Duration>) -> Self {
        Self(budget.and_then(|d| Instant::now().checked_add(d)))
    }
    fn left(&self) -> Option<Duration> {
        self.0.map(|d| d.saturating_duration_since(Instant::now()))
    }
    fn check(&self) -> Result<(), Error> {
        if self.0.is_some_and(|d| Instant::now() >= d) { Err(Error::Timeout) } else { Ok(()) }
    }
}

/// Whether the scope can be asked: `commits` names each revision `r<number>` or the whole call is
/// [`Error::InvalidRevision`] before svn is spawned. Whether `branch` is served depends on the working copy ([`reach`]).
fn served(scope: &Scope) -> Result<(), Error> {
    match scope {
        Scope::Working | Scope::Whole | Scope::Branch => Ok(()),
        Scope::Commits(revs) => revisions(revs).map(drop),
    }
}

/// A revision as the `commits` scope names it — `r` and one to ten digits, not zero (moai-0d0s.4o8). Nothing else
/// reaches svn's command line: not `12`, not `r`, not `r12 --diff-cmd=x`.
fn revision(word: &str) -> Option<u64> {
    let digits = word.strip_prefix('r')?;
    let shaped = (1..=10).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit());
    shaped.then(|| digits.parse().ok()).flatten().filter(|&n| n > 0)
}
fn revisions(revs: &[String]) -> Result<Vec<u64>, Error> {
    revs.iter().map(|r| revision(r).ok_or(Error::InvalidRevision)).collect()
}

/// svn's word for "this target is not in that revision's history": the path was made later (`E160013`, measured on
/// 1.14.3), or cannot be traced to it (`E195012`). Such a revision has nothing here — it is no failure.
const NOT_IN_HISTORY: &[&str] = &["E160013", "E195012"];

/// One revision's diff against the one before it, read from the repository — **never from the working copy**: the
/// target is the root's URL pegged at its BASE revision, so svn follows that path's history (a branch's copy back to
/// trunk) and a revision that touched only another branch prints nothing. Server calls keep their 30-second limit.
fn revision_diff(
    root: &Path,
    here: &crate::svn::Info,
    rev: u64,
    deadline: &Deadline,
) -> Result<(Vec<u8>, bool), Error> {
    let args = [
        Arg::Flag("--show-copies-as-adds"),
        Arg::Value("-c", rev.to_string()),
        Arg::Url(&here.url, Some(here.revision)),
    ];
    match crate::svn::run(root, Sub::Diff, &args, PATCH_LIMIT, deadline.left()) {
        Err(crate::svn::Error::Failed(why)) if crate::svn::codes(&why).any(|c| NOT_IN_HISTORY.contains(&c)) => {
            Ok((Vec::new(), false))
        }
        read => Ok(read?),
    }
}

/// The files the revisions changed, from each one's text diff, oldest first as given: added and deleted by svn's
/// `(nonexistent)` label, a later edit keeping a file added earlier added (as Git's list does), lines summed. A diff
/// past [`PATCH_LIMIT`] drops its last section and marks the list cut — it is not whole.
fn commits_changes(root: &Path, revs: &[u64], budget: Option<Duration>) -> Result<Changes, Error> {
    let deadline = Deadline::of(budget);
    let here = crate::svn::info(root, "", deadline.left())?;
    let mut rows: BTreeMap<String, Change> = BTreeMap::new();
    let mut cut_any = false;
    for &rev in revs {
        deadline.check()?;
        let (out, cut) = revision_diff(root, &here, rev, &deadline)?;
        diff_rows(&out, cut, &mut rows);
        if cut {
            cut_any = true;
            break;
        }
    }
    let total = rows.len();
    let files: Vec<_> = rows.into_values().take(FILE_LIMIT).collect();
    Ok(Changes { files, total, truncated: cut_any || total > FILE_LIMIT })
}

/// The files one svn text diff holds, folded into `rows`: added and deleted by svn's `(nonexistent)` label, a file
/// added by an earlier diff staying added through a later edit (as Git's list does), lines summed. A diff cut at
/// [`PATCH_LIMIT`] drops its last section, which may be partial — the caller marks the list cut.
fn diff_rows(out: &[u8], cut: bool, rows: &mut BTreeMap<String, Change>) {
    let mut parsed = crate::diff::parse(&String::from_utf8_lossy(out));
    if cut {
        parsed.pop();
    }
    for f in parsed {
        let Some(path) = f.new_path.as_deref().or(f.old_path.as_deref()).and_then(clean) else { continue };
        let status = match (&f.old_path, &f.new_path) {
            (None, _) => Status::Added,
            (_, None) => Status::Deleted,
            _ => Status::Modified,
        };
        let change = rows
            .entry(path.clone())
            .and_modify(|c| {
                let kept = status == Status::Modified && c.status == Status::Added;
                if !kept {
                    c.status = status;
                }
            })
            .or_insert_with(|| row(path, status));
        if f.binary {
            change.binary = true;
            change.adds = None;
            change.dels = None;
        } else if !change.binary {
            let lines = f.hunks.iter().flat_map(|h| &h.lines);
            let adds = lines.clone().filter(|l| l.kind == crate::diff::LineKind::Add).count();
            let dels = lines.filter(|l| l.kind == crate::diff::LineKind::Delete).count();
            change.adds = Some(change.adds.unwrap_or(0).saturating_add(adds));
            change.dels = Some(change.dels.unwrap_or(0).saturating_add(dels));
        }
    }
}

/// What `branch` and `whole` read, by where the working copy stands (moai-0d0s.x27).
enum Reach {
    /// The trunk's `branch`: nothing, as a Git main checkout's — no failure.
    Nothing,
    /// `whole` with no fork point known: the `working` scope, as before this member, no server asked.
    Working,
    /// The fork point — its URL (the working copy's place below the branch root joined on) and revision.
    Fork { old: String, rev: u64 },
}

/// Where `branch` (`branch` true) or `whole` reads from. Only a working copy on a branch copied from somewhere asks the
/// server — once per branch root, then the cache answers ([`crate::svn::fork_of`]). The trunk and a working copy of no
/// convention read `info` alone, so they answer with the repository out of reach.
fn reach(root: &Path, branch: bool, deadline: &Deadline) -> Result<Reach, Error> {
    use crate::svn::Layout;
    let here = crate::svn::info(root, "", deadline.left())?;
    let on = match crate::svn::layout(&here.relative_url) {
        Layout::Trunk if branch => return Ok(Reach::Nothing),
        Layout::Unknown if branch => return Err(Error::NoBase(super::NoBase::Convention)),
        Layout::Trunk | Layout::Unknown => return Ok(Reach::Working),
        Layout::Branch(b) => b,
    };
    match crate::svn::fork_of(root, &here, &on, deadline.left())? {
        None if branch => Err(Error::NoBase(super::NoBase::NotCopied)),
        None => Ok(Reach::Working),
        Some(fork) => {
            let path = [fork.path, on.sub].concat();
            let old = crate::svn::url(&here.repos_root, &path).ok_or(Error::InvalidPath)?;
            Ok(Reach::Fork { old, rev: fork.rev })
        }
    }
}

/// One diff from the fork point: to the working copy's BASE (`branch` — what is committed, node by node, never the
/// edits: a commit made here counts before any `svn update`) or to the working copy itself (`whole`, which `diff` only
/// reads). Read whole; callers cut the sections they need.
fn fork_diff(root: &Path, old: &str, rev: u64, base: bool, deadline: &Deadline) -> Result<(Vec<u8>, bool), Error> {
    let new = if base { Arg::NewBase("") } else { Arg::New("") };
    let args = [Arg::Flag("--show-copies-as-adds"), Arg::Old(old, rev), new];
    Ok(crate::svn::run(root, Sub::Diff, &args, PATCH_LIMIT, deadline.left())?)
}

/// `branch` or `whole`: the rows of the fork point's one diff, and for `whole` the untracked files `status` lists —
/// tracked files come from the diff alone, so a file changed on the branch and again locally is one row.
fn ranged_changes(root: &Path, branch: bool, budget: Option<Duration>) -> Result<Changes, Error> {
    let deadline = Deadline::of(budget);
    let (old, rev) = match reach(root, branch, &deadline)? {
        Reach::Nothing => return Ok(Changes::default()),
        Reach::Working => return changes_inner(root, deadline.left()),
        Reach::Fork { old, rev } => (old, rev),
    };
    let (out, cut) = fork_diff(root, &old, rev, branch, &deadline)?;
    let mut rows = BTreeMap::new();
    diff_rows(&out, cut, &mut rows);
    if !branch {
        for path in list(root, &deadline)?.untracked() {
            rows.entry(path.clone()).or_insert_with(|| row(path, Status::Untracked));
        }
    }
    let total = rows.len();
    let mut files: Vec<_> = rows.into_values().take(FILE_LIMIT).collect();
    for change in files.iter_mut().filter(|c| c.status == Status::Untracked) {
        deadline.check()?;
        super::untracked_counts(root, change);
    }
    Ok(Changes { files, total, truncated: cut || total > FILE_LIMIT })
}

/// `branch` or `whole`'s patch: the fork point's one diff as svn printed it, narrowed here to the files and folders
/// asked ([`keep_sections`]), and for `whole` the untracked files after it in Git's form.
fn ranged_patch(
    root: &Path,
    branch: bool,
    paths: Option<&[String]>,
    budget: Option<Duration>,
    known: Option<Vec<String>>,
) -> Result<Patch, Error> {
    let deadline = Deadline::of(budget);
    let (old, rev) = match reach(root, branch, &deadline)? {
        Reach::Nothing => return Ok(Patch::default()),
        Reach::Working => return patch_inner(root, paths, deadline.left(), known),
        Reach::Fork { old, rev } => (old, rev),
    };
    let (data, cut) = fork_diff(root, &old, rev, branch, &deadline)?;
    let mut out = match paths {
        Some(specs) => keep_sections(&data, specs),
        None => data,
    };
    let mut truncated = cut;
    if !branch && !truncated {
        let names = match known {
            Some(names) => names,
            None => list(root, &deadline)?.untracked(),
        };
        untracked_into(root, names, paths, &mut out, &mut truncated, &deadline)?;
    }
    Ok(capped(out, truncated))
}

/// The revisions' diffs one after another, each whole from the server and narrowed here to the files and folders
/// asked ([`keep_sections`]) — names never reach svn. Past [`PATCH_LIMIT`] the text is cut and says so.
fn commits_patch(
    root: &Path,
    revs: &[u64],
    paths: Option<&[String]>,
    budget: Option<Duration>,
) -> Result<Patch, Error> {
    let deadline = Deadline::of(budget);
    let here = crate::svn::info(root, "", deadline.left())?;
    let mut out = Vec::new();
    let mut truncated = false;
    for &rev in revs {
        deadline.check()?;
        let (data, cut) = revision_diff(root, &here, rev, &deadline)?;
        match paths {
            Some(specs) => out.extend(keep_sections(&data, specs)),
            None => out.extend(data),
        }
        truncated |= cut;
        if truncated || out.len() > PATCH_LIMIT {
            break;
        }
    }
    Ok(capped(out, truncated))
}

/// The text of a patch, cut at [`PATCH_LIMIT`] on a character boundary.
fn capped(out: Vec<u8>, mut truncated: bool) -> Patch {
    let mut text = String::from_utf8_lossy(&out).into_owned();
    if text.len() > PATCH_LIMIT {
        let mut end = PATCH_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        truncated = true;
    }
    Patch { text, truncated }
}

/// Whether a relative, `/`-separated path is one svn may name and this crate may open: plain components only — no
/// empty one, no `.`, no `..`, no root or drive, no `.svn`. A `\` counts as a separator, so `..\x` is refused too.
fn valid(path: &str) -> bool {
    !path.is_empty()
        && path.split(['/', '\\']).all(|c| !c.is_empty() && c != "." && c != ".." && !c.eq_ignore_ascii_case(".svn"))
        && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}
/// A path svn printed, with `\` read as `/` (Windows), or `None` when it is not [`valid`].
fn clean(raw: &str) -> Option<String> {
    let path = raw.replace('\\', "/");
    valid(&path).then_some(path)
}
/// A caller's file or folder (one trailing `/`) is valid by the same rule.
fn valid_spec(spec: &str) -> bool {
    valid(spec.strip_suffix('/').unwrap_or(spec))
}

fn row(path: String, status: Status) -> Change {
    Change { path, old_path: None, status, adds: None, dels: None, binary: false }
}

/// One `status --xml` entry, as written.
#[derive(Debug, Default, PartialEq, Eq)]
struct Entry {
    path: String,
    item: String,
    props: String,
    copied: bool,
    tree_conflicted: bool,
}
fn status_entries(text: &str) -> Vec<Entry> {
    let doc = crate::svn::xml::parse(text);
    let Some(status) = doc.child("status") else { return Vec::new() };
    let groups = status.children("target").chain(status.children("changelist"));
    groups
        .flat_map(|g| g.children("entry"))
        .filter_map(|e| {
            let wc = e.child("wc-status")?;
            Some(Entry {
                path: e.attr("path")?.to_owned(),
                item: wc.attr("item").unwrap_or_default().to_owned(),
                props: wc.attr("props").unwrap_or_default().to_owned(),
                copied: wc.attr("copied") == Some("true"),
                tree_conflicted: wc.attr("tree-conflicted") == Some("true"),
            })
        })
        .collect()
}

/// The working copy's changed files, every one, and the directories whose files `diff` reports only through them.
#[derive(Debug, Default)]
struct Listing {
    rows: BTreeMap<String, Change>,
    /// `svn rm`'d, missing and copied directories opened with `info`.
    roots: Vec<String>,
}
impl Listing {
    fn untracked(&self) -> Vec<String> {
        self.rows.values().filter(|c| c.status == Status::Untracked).map(|c| c.path.clone()).collect()
    }
}

fn list(root: &Path, deadline: &Deadline) -> Result<Listing, Error> {
    let (out, cut) = crate::svn::run(root, Sub::Status, &[Arg::Flag("--xml")], LIST_LIMIT, deadline.left())?;
    if cut {
        return Err(Error::Limit);
    }
    let entries = status_entries(&String::from_utf8_lossy(&out));
    // An unversioned root answers with exit 0 — a warning and no entry, or `.` as unversioned. `info` says why.
    let root_unversioned = entries.iter().any(|e| e.path == "." && e.item == "unversioned");
    if entries.is_empty() || root_unversioned {
        crate::svn::info(root, "", deadline.left())?;
        if root_unversioned {
            return Err(Error::Svn(crate::svn::Error::Failed(format!(
                "'{}' is not under version control",
                root.display()
            ))));
        }
    }
    let mut listing = Listing::default();
    let mut open = Vec::new();
    let mut unversioned = Vec::new();
    for e in entries {
        let Some(path) = clean(&e.path) else { continue };
        let dir = super::dir_on_disk(root, &path) == Some(true);
        let status = if e.tree_conflicted {
            Status::Conflicted
        } else {
            match e.item.as_str() {
                "unversioned" if dir => {
                    unversioned.push(path);
                    continue;
                }
                "unversioned" => Status::Untracked,
                // A copied directory's unmodified files are not listed; a plainly added one's are, each on its own line.
                "added" | "replaced" if dir && e.copied => {
                    open.push((path, Status::Added));
                    continue;
                }
                "added" => Status::Added,
                "replaced" => Status::Modified,
                // Inside a copy, `diff --show-copies-as-adds` prints the file as added: the row says the same.
                "modified" | "obstructed" | "incomplete" if e.copied => Status::Added,
                "modified" | "obstructed" | "incomplete" => Status::Modified,
                "deleted" | "missing" => {
                    open.push((path, Status::Deleted));
                    continue;
                }
                "conflicted" => Status::Conflicted,
                "normal" if e.props == "conflicted" => Status::Conflicted,
                // Properties alone: a file without hunks. A directory's properties are not a file change.
                "normal" if e.props == "modified" => Status::Modified,
                _ => continue,
            }
        };
        // Files only: a directory's own line is no row.
        if !dir {
            listing.rows.insert(path.clone(), row(path, status));
        }
    }
    opened(root, &open, &mut listing, deadline)?;
    for dir in unversioned {
        for path in walk(root, &dir, deadline)? {
            listing.rows.entry(path.clone()).or_insert_with(|| row(path, Status::Untracked));
        }
    }
    Ok(listing)
}

/// `info --depth infinity` over the entries `status` gives no kind or no files for: every file below (or the entry
/// itself) takes the entry's status unless `status` listed it already; a directory among them is a root.
fn opened(root: &Path, open: &[(String, Status)], listing: &mut Listing, deadline: &Deadline) -> Result<(), Error> {
    let mut at = 0;
    while at < open.len() {
        // Names on one command line stay under the limit Git's reads keep (Windows caps a whole line).
        let mut end = at;
        let mut bytes = 0usize;
        while end < open.len() && (end == at || bytes + open[end].0.len() + 4 <= super::PATHSPEC_LIMIT) {
            bytes += open[end].0.len() + 4;
            end += 1;
        }
        let batch = &open[at..end];
        at = end;
        let mut args = vec![Arg::Flag("--xml"), Arg::Value("--depth", "infinity".into())];
        args.extend(batch.iter().map(|(p, _)| Arg::Path(p)));
        let (out, cut) = crate::svn::run(root, Sub::Info, &args, LIST_LIMIT, deadline.left())?;
        if cut {
            return Err(Error::Limit);
        }
        let doc = crate::svn::xml::parse(&String::from_utf8_lossy(&out));
        let Some(info) = doc.child("info") else { continue };
        for e in info.children("entry") {
            let Some(path) = e.attr("path").and_then(clean) else { continue };
            // The status of the entry this path stands under — the longest one, should two nest.
            let Some((_, status)) = batch
                .iter()
                .filter(|(p, _)| *p == path || path.strip_prefix(p.as_str()).is_some_and(|t| t.starts_with('/')))
                .max_by_key(|(p, _)| p.len())
            else {
                continue;
            };
            match e.attr("kind") {
                Some("file") => {
                    listing.rows.entry(path.clone()).or_insert_with(|| row(path, *status));
                }
                Some("dir") if batch.iter().any(|(p, _)| *p == path) => listing.roots.push(path),
                _ => {}
            }
        }
    }
    Ok(())
}

/// The files below an unversioned directory, which `status` reports as one line: every component opened without
/// following a link, `.svn` skipped, inside the deadline. svn's ignore rules are not applied here.
#[cfg(unix)]
fn walk(root: &Path, top: &str, deadline: &Deadline) -> Result<Vec<String>, Error> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let root_fd = std::fs::File::open(root)?;
    let mut pending = vec![top.to_owned()];
    let mut found = Vec::new();
    'walk: while let Some(relative) = pending.pop() {
        deadline.check()?;
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
            deadline.check()?;
            let Some(name) = entry.name.to_str() else { continue };
            let path = format!("{relative}/{name}");
            if !valid(&path) {
                continue;
            }
            if entry.mode & libc::S_IFMT == libc::S_IFDIR {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    Ok(found)
}
#[cfg(not(unix))]
fn walk(root: &Path, top: &str, deadline: &Deadline) -> Result<Vec<String>, Error> {
    let mut pending = vec![top.to_owned()];
    let mut found = Vec::new();
    while let Some(relative) = pending.pop() {
        deadline.check()?;
        let Ok(entries) = std::fs::read_dir(root.join(&relative)) else { continue };
        for entry in entries.flatten() {
            deadline.check()?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            let path = format!("{relative}/{name}");
            if !valid(&path) {
                continue;
            }
            // `file_type` does not follow a link: a link to a directory is listed, never walked.
            match entry.file_type() {
                Ok(t) if t.is_dir() => pending.push(path),
                Ok(_) => found.push(path),
                Err(_) => {}
            }
        }
    }
    Ok(found)
}

/// The tracked files' line counts and binary flags, from one text diff of the scope. Past [`PATCH_LIMIT`] the last
/// section read may be partial and the files after it are not read: they keep `None`.
fn counts(root: &Path, rows: &mut BTreeMap<String, Change>, deadline: &Deadline) -> Result<(), Error> {
    let (out, cut) =
        crate::svn::run(root, Sub::Diff, &[Arg::Flag("--show-copies-as-adds")], PATCH_LIMIT, deadline.left())?;
    let mut parsed = crate::diff::parse(&String::from_utf8_lossy(&out));
    if cut {
        parsed.pop();
    }
    for f in parsed {
        let Some(path) = f.new_path.as_deref().or(f.old_path.as_deref()).and_then(clean) else { continue };
        let Some(change) = rows.get_mut(&path) else { continue };
        if f.binary {
            change.binary = true;
            change.adds = None;
            change.dels = None;
        } else if !change.binary {
            let lines = f.hunks.iter().flat_map(|h| &h.lines);
            let adds = lines.clone().filter(|l| l.kind == crate::diff::LineKind::Add).count();
            let dels = lines.filter(|l| l.kind == crate::diff::LineKind::Delete).count();
            change.adds = Some(change.adds.unwrap_or(0).saturating_add(adds));
            change.dels = Some(change.dels.unwrap_or(0).saturating_add(dels));
        }
    }
    Ok(())
}

pub(super) fn changes(root: &Path, scope: &Scope, budget: Option<Duration>) -> Result<Changes, Error> {
    served(scope)?;
    let root = root.to_path_buf();
    if let Scope::Commits(revs) = scope {
        let revs = revisions(revs)?;
        return super::timed(budget, move || commits_changes(&root, &revs, budget));
    }
    if let Scope::Branch | Scope::Whole = scope {
        let branch = matches!(scope, Scope::Branch);
        return super::timed(budget, move || ranged_changes(&root, branch, budget));
    }
    super::timed(budget, move || changes_inner(&root, budget))
}
fn changes_inner(root: &Path, budget: Option<Duration>) -> Result<Changes, Error> {
    let deadline = Deadline::of(budget);
    let mut rows = list(root, &deadline)?.rows;
    if rows.values().any(|c| c.status != Status::Untracked) {
        counts(root, &mut rows, &deadline)?;
    }
    let total = rows.len();
    let mut files: Vec<_> = rows.into_values().take(FILE_LIMIT).collect();
    for change in files.iter_mut().filter(|c| c.status == Status::Untracked) {
        deadline.check()?;
        super::untracked_counts(root, change);
    }
    Ok(Changes { files, total, truncated: total > FILE_LIMIT })
}

/// The `diff` targets for narrowed specs: only a spec that covers a tracked changed file reaches svn — a name svn does
/// not version fails the whole call (`E155010`), and one that covers nothing tracked has no section anyway. A name
/// under an opened directory is read through the outermost such directory. A target inside another is dropped, so no
/// section is printed twice.
fn targets(specs: &[String], listing: &Listing) -> Vec<String> {
    let tracked: Vec<&str> =
        listing.rows.values().filter(|c| c.status != Status::Untracked).map(|c| c.path.as_str()).collect();
    let under = |dir: &str, name: &str| name.strip_prefix(dir).is_some_and(|t| t.starts_with('/'));
    let mut chosen = BTreeSet::new();
    for spec in specs {
        if !tracked.iter().any(|t| super::covers(spec, t)) {
            continue;
        }
        let name = spec.strip_suffix('/').unwrap_or(spec);
        let through = listing.roots.iter().filter(|r| *r == name || under(r, name)).min_by_key(|r| r.len());
        chosen.insert(through.map_or(name, String::as_str).to_owned());
    }
    chosen.iter().filter(|t| !chosen.iter().any(|o| under(o, t))).cloned().collect()
}

/// The sections of an svn diff whose `Index:` path a spec covers. A hunk line never starts with `I`, so an
/// `Index: ` at a line's start is always a section head.
fn keep_sections(text: &[u8], specs: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut keep = false;
    for line in text.split_inclusive(|&b| b == b'\n') {
        if let Some(name) = line.strip_prefix(b"Index: ") {
            let name = String::from_utf8_lossy(name);
            let name = name.trim_end_matches(['\n', '\r']);
            keep = clean(name).is_some_and(|n| specs.iter().any(|s| super::covers(s, &n)));
        }
        if keep {
            out.extend_from_slice(line);
        }
    }
    out
}

/// The patch of the scope, or of some files and folders (a folder with its trailing `/`). Tracked files are svn's
/// text diff as printed; untracked files follow in Git's form ([`super::untracked_patch`]). `known` names the
/// untracked files of a list read before, as [`super::Backend::patch_with_changes`] does for Git.
pub(super) fn patch(
    root: &Path,
    scope: &Scope,
    paths: Option<Vec<String>>,
    budget: Option<Duration>,
    known: Option<Vec<String>>,
) -> Result<Patch, Error> {
    served(scope)?;
    if paths.as_ref().is_some_and(|ps| ps.iter().any(|p| !valid_spec(p))) {
        return Err(Error::InvalidPath);
    }
    let root = root.to_path_buf();
    if let Scope::Commits(revs) = scope {
        let revs = revisions(revs)?;
        return super::timed(budget, move || commits_patch(&root, &revs, paths.as_deref(), budget));
    }
    if let Scope::Branch | Scope::Whole = scope {
        let branch = matches!(scope, Scope::Branch);
        return super::timed(budget, move || ranged_patch(&root, branch, paths.as_deref(), budget, known));
    }
    super::timed(budget, move || patch_inner(&root, paths.as_deref(), budget, known))
}
fn patch_inner(
    root: &Path,
    paths: Option<&[String]>,
    budget: Option<Duration>,
    known: Option<Vec<String>>,
) -> Result<Patch, Error> {
    let deadline = Deadline::of(budget);
    // Past the limit the names stay off the command line and the scope is read whole, as Git's reads do.
    let narrow = paths.filter(|ps| !super::past_pathspec_limit(ps));
    let mut listing = None;
    let chosen = match narrow {
        None => None,
        Some(specs) => {
            let read = list(root, &deadline)?;
            let chosen = targets(specs, &read);
            listing = Some(read);
            Some(chosen)
        }
    };
    let mut out = Vec::new();
    let mut truncated = false;
    if chosen.as_ref().is_none_or(|t| !t.is_empty()) {
        let mut args = vec![Arg::Flag("--show-copies-as-adds")];
        args.extend(chosen.iter().flatten().map(|t| Arg::Local(t)));
        let (data, cut) = crate::svn::run(root, Sub::Diff, &args, PATCH_LIMIT, deadline.left())?;
        out = match narrow {
            Some(specs) => keep_sections(&data, specs),
            None => data,
        };
        truncated = cut;
    }
    if !truncated {
        let names = match (known, listing) {
            (Some(names), _) => names,
            (None, Some(read)) => read.untracked(),
            (None, None) => list(root, &deadline)?.untracked(),
        };
        untracked_into(root, names, paths, &mut out, &mut truncated, &deadline)?;
    }
    Ok(capped(out, truncated))
}

/// The untracked files among `names` that `paths` (all when `None`) cover, appended in Git's form until the patch is
/// full.
fn untracked_into(
    root: &Path,
    names: Vec<String>,
    paths: Option<&[String]>,
    out: &mut Vec<u8>,
    truncated: &mut bool,
    deadline: &Deadline,
) -> Result<(), Error> {
    for name in names.into_iter().filter(|n| paths.is_none_or(|ps| ps.iter().any(|p| super::covers(p, n)))) {
        deadline.check()?;
        let (data, cut) = super::untracked_patch(root, &name, PATCH_LIMIT.saturating_sub(out.len()))?;
        out.extend(data);
        *truncated |= cut;
        if *truncated {
            break;
        }
    }
    Ok(())
}

/// The ignored directories, each with its trailing `/`, and `.svn/` always: `status --no-ignore` entries marked
/// `ignored` that the disk holds as a directory. File search and the recursive expand skip them.
pub(super) fn ignored_dirs(root: &Path, budget: Option<Duration>) -> Result<Vec<String>, Error> {
    let root = root.to_path_buf();
    super::timed(budget, move || {
        let args = [Arg::Flag("--xml"), Arg::Flag("--no-ignore")];
        let (out, cut) = crate::svn::run(&root, Sub::Status, &args, LIST_LIMIT, budget)?;
        if cut {
            return Err(Error::Limit);
        }
        let mut dirs = vec![".svn/".to_owned()];
        for e in status_entries(&String::from_utf8_lossy(&out)) {
            if e.item == "ignored"
                && let Some(path) = clean(&e.path)
                && super::dir_on_disk(&root, &path) == Some(true)
            {
                dirs.push(format!("{path}/"));
            }
        }
        Ok(dirs)
    })
}

#[cfg(test)]
mod tests {
    use super::super::Backend;
    use super::*;
    use crate::svn::tests::{Repo, admin_snapshot, commit_files, svn_in, svn_ready};
    use std::fs;

    fn paths(c: &Changes) -> Vec<(&str, Status)> {
        c.files.iter().map(|f| (f.path.as_str(), f.status)).collect()
    }
    fn find<'a>(c: &'a Changes, path: &str) -> &'a Change {
        c.files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("{path} not in {c:?}"))
    }
    /// The hunks of `name` in a patch, by the rule `changes::owns` uses.
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
    fn paths_svn_prints_are_plain_components_only() {
        for good in ["a", "a/b", "한글 이름/x", "-r", "a@b", "--output=evil", "sp ace"] {
            assert!(valid(good), "{good:?}");
        }
        for bad in ["", ".", "..", "../x", "a/../b", "/abs", "a//b", "a/", ".svn", "x/.SVN/wc.db", "a/./b", "..\\x"] {
            assert!(!valid(bad), "{bad:?}");
        }
        assert_eq!(clean("sub\\x.txt").as_deref(), Some("sub/x.txt"));
        assert_eq!(clean("..\\..\\etc"), None);
        assert!(valid_spec("src/") && !valid_spec("src//") && !valid_spec("/"));
    }

    #[test]
    fn status_xml_reads_targets_changelists_and_attributes_in_any_order() {
        let text = "<?xml version=\"1.0\"?><status><target path=\".\"><entry path=\"a\"><wc-status props=\"none\" item=\"modified\" revision=\"2\"></wc-status></entry><entry path=\"d\"><wc-status copied=\"true\" item=\"added\" props=\"none\"/></entry></target><changelist name=\"cl\"><entry path=\"b\"><wc-status tree-conflicted=\"true\" item=\"normal\" props=\"modified\"></wc-status></entry></changelist></status>";
        let got = status_entries(text);
        assert_eq!(got.len(), 3);
        assert_eq!((got[0].path.as_str(), got[0].item.as_str()), ("a", "modified"));
        assert!(got[1].copied && !got[1].tree_conflicted);
        assert_eq!(got[2].path, "b");
        assert!(got[2].tree_conflicted && got[2].props == "modified");
        assert!(status_entries("<status><target").is_empty());
    }

    #[test]
    fn sections_are_kept_by_their_index_path() {
        let text = b"Index: d/a\n=====\n--- d/a\t(revision 1)\n+++ d/a\t(nonexistent)\n@@ -1 +0,0 @@\n-a\nIndex: d/b\n=====\n@@ -1 +0,0 @@\n-b\nIndex: e\n=====\n@@ -1 +1 @@\n-x\n+y\n";
        let kept = String::from_utf8(keep_sections(text, &["d/b".into(), "e".into()])).unwrap();
        assert!(!kept.contains("-a") && kept.contains("-b") && kept.contains("+y"), "{kept}");
        let kept = String::from_utf8(keep_sections(text, &["d/".into()])).unwrap();
        assert!(kept.contains("-a") && kept.contains("-b") && !kept.contains("+y"), "{kept}");
    }

    #[test]
    fn narrowed_targets_skip_untracked_names_and_go_through_opened_directories() {
        let mut listing = Listing::default();
        for (p, s) in [("m", Status::Modified), ("gone/a", Status::Deleted), ("gone/in/b", Status::Deleted)] {
            listing.rows.insert(p.into(), row(p.into(), s));
        }
        listing.rows.insert("new/u".into(), row("new/u".into(), Status::Untracked));
        listing.roots.push("gone".into());
        let ask = |specs: &[&str]| targets(&specs.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &listing);
        assert_eq!(ask(&["m"]), ["m"]);
        assert!(ask(&["new/u", "new/", "nothing"]).is_empty(), "svn never sees an unversioned name");
        assert_eq!(ask(&["gone/in/b"]), ["gone"]);
        assert_eq!(ask(&["gone/in/", "gone/a", "m"]), ["gone", "m"]);
    }

    #[test]
    fn detection_and_choice() {
        let dir = crate::scratch::Scratch::new("vcs-svn-detect");
        let both = dir.join("both");
        let git_above = dir.join("git-above");
        let svn_above = dir.join("svn-above");
        let only = dir.join("only");
        let file = dir.join("file");
        for d in [&both, &git_above, &svn_above, &only, &file] {
            fs::create_dir_all(d).unwrap();
        }
        fs::create_dir(both.join(".git")).unwrap();
        fs::create_dir(both.join(".svn")).unwrap();
        fs::create_dir(git_above.join(".git")).unwrap();
        fs::create_dir_all(git_above.join("wc/.svn")).unwrap();
        fs::create_dir(svn_above.join(".svn")).unwrap();
        fs::create_dir_all(svn_above.join("repo/.git")).unwrap();
        fs::create_dir(only.join(".svn")).unwrap();
        fs::write(file.join(".svn"), "not a working copy").unwrap();
        let all = Detected { git: true, svn: true };
        for root in [both.clone(), git_above.join("wc"), svn_above.join("repo")] {
            assert_eq!(super::super::detected(&root), all, "{}", root.display());
            assert_eq!(super::super::detect(&root), Some(Backend::Git), "git is the default: {}", root.display());
        }
        assert_eq!(super::super::detect(&only), Some(Backend::Svn));
        assert_eq!(super::super::detect(&only.join("deep")), Some(Backend::Svn));
        // A file named `.svn` is no working copy. (The scratch directory may itself sit inside a Git checkout.)
        assert!(!super::super::detected(&file).svn);
        use super::super::{Asked, Detected, choose};
        let svn_asked = Asked { config: Some(Backend::Svn), ..Asked::default() };
        assert_eq!(choose(all, svn_asked), Some(Backend::Svn));
        assert_eq!(choose(all, Asked::default()), Some(Backend::Git));
        let git_only = Detected { git: true, svn: false };
        assert_eq!(choose(git_only, svn_asked), Some(Backend::Git), "an undetected ask is passed over");
        let order = Asked { flag: Some(Backend::Git), screen: Some(Backend::Svn), config: Some(Backend::Svn) };
        assert_eq!(choose(all, order), Some(Backend::Git), "the flag is strongest");
        let order = Asked { flag: None, screen: Some(Backend::Git), config: Some(Backend::Svn) };
        assert_eq!(choose(all, order), Some(Backend::Git), "the screen beats the file");
        assert_eq!(choose(Detected::default(), svn_asked), None);
        assert!(all.has(Backend::Svn) && !git_only.has(Backend::Svn));
    }

    #[test]
    fn the_vcs_table_is_kept_as_written_and_read_where_it_is_used() {
        use super::super::configured;
        let parse = |src: &str| crate::config::Config::parse(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        let c = parse("prefix = \"a\"\n[vcs]\nbackend = \"svn\"  # this checkout\n");
        assert_eq!(configured(&c.vcs).backend, Some(Backend::Svn));
        assert_eq!(configured(&c.vcs).unknown, None);
        assert_eq!(configured(&parse("prefix = \"a\"\n[vcs]\nbackend = \"git\"\n").vcs).backend, Some(Backend::Git));
        // An unknown value never stops the parse: it reads as unset, and says what was written.
        let c = parse("prefix = \"a\"\n[vcs]\nbackend = \"hg\"\n");
        assert_eq!(configured(&c.vcs), super::super::Configured { backend: None, unknown: Some("\"hg\"".into()) });
        let c = parse("prefix = \"a\"\n[vcs]\nbackend = svn\n");
        assert_eq!(configured(&c.vcs).unknown.as_deref(), Some("svn"));
        // A flat `backend`, or one under another table, is not this setting.
        for src in ["prefix = \"a\"\nbackend = \"svn\"\n", "prefix = \"a\"\n[models]\nbackend = \"svn\"\n"] {
            assert_eq!(configured(&parse(src).vcs), super::super::Configured::default(), "{src}");
        }
        assert_eq!(configured(&parse("prefix = \"a\"\n").vcs), super::super::Configured::default());
    }

    /// The working copy every svn test below starts from: trunk at r2 with a handful of files.
    fn working_copy(name: &str) -> (Repo, std::path::PathBuf) {
        let repo = Repo::new(name);
        let wc = repo.checkout("trunk", "wc");
        commit_files(
            &wc,
            &[
                ("m.txt", "a\nb\nc\n"),
                ("del.txt", "d\n"),
                ("gone.txt", "g\n"),
                ("mv.txt", &body("mv")),
                ("bin", "\0\x01"),
                ("tree/in/q", "q\n"),
                ("tree/r", "r\n"),
                ("copy-me/x", "x\n"),
            ],
            "base",
        );
        svn_in(&wc, &["propset", "-q", "svn:mime-type", "application/octet-stream", "./bin@"]);
        svn_in(&wc, &["commit", "-q", "-m", "binary"]);
        svn_in(&wc, &["update", "-q"]);
        (repo, wc)
    }

    #[test]
    fn working_changes_are_sorted_into_their_kinds() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-kinds");
        fs::write(wc.join("m.txt"), "a\nB\nc\nd\n").unwrap();
        svn_in(&wc, &["rm", "-q", "./del.txt@"]);
        fs::remove_file(wc.join("gone.txt")).unwrap();
        svn_in(&wc, &["mv", "-q", "./mv.txt@", "moved.txt"]);
        fs::write(wc.join("new.txt"), "n\n").unwrap();
        svn_in(&wc, &["add", "-q", "./new.txt@"]);
        fs::write(wc.join("bin"), "\0\x02\x03").unwrap();
        svn_in(&wc, &["rm", "-q", "./tree@"]);
        svn_in(&wc, &["cp", "-q", "./copy-me@", "copied"]);
        fs::create_dir_all(wc.join("loose/deep")).unwrap();
        fs::write(wc.join("loose/deep/f"), "u\n").unwrap();
        fs::write(wc.join("loose/g"), "v\nw").unwrap();
        fs::write(wc.join("top"), "t\n").unwrap();
        // Ignored files and directories stay out; the directory's own property change is no file change.
        svn_in(&wc, &["propset", "-q", "svn:ignore", "build\n*.log", "./@"]);
        fs::create_dir(wc.join("build")).unwrap();
        fs::write(wc.join("build/out"), "o\n").unwrap();
        fs::write(wc.join("x.log"), "l\n").unwrap();
        // A changelist moves an entry under `<changelist>` in the XML.
        svn_in(&wc, &["changelist", "-q", "cl", "./m.txt@"]);
        let before = admin_snapshot(&wc);
        let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
        use Status::*;
        assert_eq!(
            paths(&c),
            [
                ("bin", Modified),
                ("copied/x", Added),
                ("del.txt", Deleted),
                ("gone.txt", Deleted),
                ("loose/deep/f", Untracked),
                ("loose/g", Untracked),
                ("m.txt", Modified),
                ("moved.txt", Added),
                ("mv.txt", Deleted),
                ("new.txt", Added),
                ("top", Untracked),
                ("tree/in/q", Deleted),
                ("tree/r", Deleted),
            ],
            "{c:?}"
        );
        assert_eq!(c.total, 13);
        assert!(c.files.iter().all(|f| f.old_path.is_none()), "a move is a deletion and an addition");
        let m = find(&c, "m.txt");
        assert_eq!((m.adds, m.dels, m.binary), (Some(2), Some(1), false));
        let b = find(&c, "bin");
        assert!(b.binary && b.adds.is_none() && b.dels.is_none(), "{b:?}");
        assert_eq!((find(&c, "moved.txt").adds, find(&c, "mv.txt").dels), (Some(20), Some(20)));
        assert_eq!(find(&c, "loose/g").adds, Some(2));
        assert_eq!(find(&c, "gone.txt").adds, None, "svn prints no diff for a missing file");
        // Every row's hunks are found in the patch by the rule the surfaces use.
        let p = Backend::Svn.patch_with_changes(&wc, &Scope::Working, None, None, &c).unwrap();
        for f in c.files.iter().filter(|f| !f.binary && f.path != "gone.txt") {
            let h = hunks_of(&p.text, &f.path);
            assert!(!h.is_empty(), "{} has no hunk in\n{}", f.path, p.text);
            assert_eq!((Some(counted(&h).0), Some(counted(&h).1)), (f.adds, f.dels), "{}", f.path);
        }
        // On the trunk `whole` is `working` and `branch` is empty, as a Git main checkout's (moai-0d0s.x27).
        let whole = Backend::Svn.changes(&wc, &Scope::Whole, None).unwrap();
        assert_eq!(whole, c);
        assert_eq!(Backend::Svn.patch(&wc, &Scope::Whole, None, None).unwrap().text, p.text);
        assert_eq!(Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap(), Changes::default());
        assert!(Backend::Svn.patch(&wc, &Scope::Branch, None, None).unwrap().text.is_empty());
        // `commits` is served since moai-0d0s.4o8: no revisions, no changes.
        assert_eq!(Backend::Svn.changes(&wc, &Scope::Commits(vec![]), None).unwrap(), Changes::default());
        // Directory marks read no facts: every directory stands `M`.
        let marks = Backend::Svn.marks(&wc, &c, None);
        assert!(marks.dirs.values().all(|m| *m == super::super::Mark::Modified));
        assert_eq!(marks.files["copied/x"], super::super::Mark::Added);
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");
        assert!(matches!(Backend::Svn.changes(&wc, &Scope::Working, Some(Duration::ZERO)), Err(Error::Timeout)));
    }

    #[test]
    fn hostile_names_come_through_and_reach_svn_literally() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-names");
        let mut names = vec!["--output=evil", "a@b", "-r", "sp ace.txt", "한글 이름.txt", "x@1"];
        if cfg!(unix) {
            names.push("q\"uote.txt");
        }
        commit_files(&wc, &names.iter().map(|n| (*n, "old\n")).collect::<Vec<_>>(), "names");
        fs::write(wc.join("x"), "the file x@1 is not\n").unwrap();
        svn_in(&wc, &["add", "-q", "./x@"]);
        for n in &names {
            fs::write(wc.join(n), "new\n").unwrap();
        }
        fs::write(wc.join("--untracked"), "u\n").unwrap();
        let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
        for n in &names {
            assert_eq!((find(&c, n).status, find(&c, n).adds, find(&c, n).dels), (Status::Modified, Some(1), Some(1)));
        }
        assert_eq!(find(&c, "--untracked").status, Status::Untracked);
        for n in &names {
            let got = Backend::Svn.patch(&wc, &Scope::Working, Some(n), None).unwrap();
            assert_eq!(counted(&hunks_of(&got.text, n)), (1, 1), "{n}: {}", got.text);
            assert_eq!(crate::diff::parse(&got.text).len(), 1, "{n} read another file: {}", got.text);
        }
        let got = Backend::Svn.patch(&wc, &Scope::Working, Some("--untracked"), None).unwrap();
        assert!(got.text.contains("+u"), "{}", got.text);
        assert!(!wc.join("evil").exists(), "--output=evil was read as an option");
        for bad in ["../x", ".svn/wc.db", "", "/abs", "a/../../x", "sub\\..\\..\\x"] {
            assert!(
                matches!(Backend::Svn.patch(&wc, &Scope::Working, Some(bad), None), Err(Error::InvalidPath)),
                "{bad:?}"
            );
        }
        let names = vec!["a@b".to_owned(), "--output=evil".to_owned()];
        let got = Backend::Svn.patch_of(&wc, &Scope::Working, &names, None, Some(&c)).unwrap();
        assert_eq!(crate::diff::parse(&got.text).len(), 2, "{}", got.text);
    }

    #[test]
    fn a_narrowed_patch_holds_its_files_only() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-narrow");
        fs::write(wc.join("m.txt"), "changed\n").unwrap();
        fs::write(wc.join("tree/r"), "r2\n").unwrap();
        svn_in(&wc, &["rm", "-q", "./tree/in@"]);
        fs::create_dir(wc.join("tree/fresh")).unwrap();
        fs::write(wc.join("tree/fresh/u"), "untracked below\n").unwrap();
        fs::write(wc.join("loose"), "untracked outside\n").unwrap();
        svn_in(&wc, &["mv", "-q", "./copy-me@", "moved-dir"]);
        let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
        let whole = Backend::Svn.patch_with_changes(&wc, &Scope::Working, None, None, &c).unwrap();
        // A file inside an `svn rm`'d directory and an unmodified file inside a moved one keep their hunks.
        for name in ["tree/in/q", "moved-dir/x", "copy-me/x", "m.txt"] {
            let got = Backend::Svn.patch_of(&wc, &Scope::Working, &[name.to_owned()], None, Some(&c)).unwrap();
            assert_eq!(hunks_of(&got.text, name), hunks_of(&whole.text, name), "{name}: {}", got.text);
            assert!(!hunks_of(&got.text, name).is_empty(), "{name}");
            assert_eq!(crate::diff::parse(&got.text).len(), 1, "{name} read another file: {}", got.text);
        }
        let got = Backend::Svn.patch_of(&wc, &Scope::Working, &["tree/".to_owned()], None, Some(&c)).unwrap();
        for name in ["tree/in/q", "tree/r", "tree/fresh/u"] {
            assert_eq!(hunks_of(&got.text, name), hunks_of(&whole.text, name), "{name}: {}", got.text);
            assert!(!hunks_of(&got.text, name).is_empty(), "{name}: {}", got.text);
        }
        for outside in ["changed", "untracked outside", "moved-dir"] {
            assert!(!got.text.contains(outside), "{outside} is not in tree/: {}", got.text);
        }
        // A folder that holds only untracked files never reaches svn, which would refuse it.
        let got = Backend::Svn.patch_of(&wc, &Scope::Working, &["tree/fresh/".to_owned()], None, None).unwrap();
        assert!(got.text.contains("untracked below") && !got.text.contains("Index:"), "{}", got.text);
        // An unchanged file and a name nothing has give an empty patch, not a failure.
        let got = Backend::Svn.patch_of(&wc, &Scope::Working, &["del.txt".into(), "nothing".into()], None, None);
        assert!(got.unwrap().text.is_empty());
    }

    #[test]
    fn a_patch_past_the_limit_and_a_long_list_are_cut() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-limit");
        fs::write(wc.join("m.txt"), "x\n".repeat(PATCH_LIMIT / 2 + 100)).unwrap();
        fs::write(wc.join("mv.txt"), "changed\n").unwrap();
        let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
        // `m.txt` sorts first in svn's diff and fills it: its count may be partial and `mv.txt` is not read.
        assert_eq!(find(&c, "m.txt").adds, None);
        assert_eq!(find(&c, "mv.txt").adds, None);
        let p = Backend::Svn.patch(&wc, &Scope::Working, None, None).unwrap();
        assert!(p.truncated && p.text.len() <= PATCH_LIMIT);
        fs::write(wc.join("m.txt"), "short\n").unwrap();
        for n in 0..FILE_LIMIT + 1 {
            fs::write(wc.join(format!("f{n:04}")), "").unwrap();
        }
        let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
        assert_eq!(c.total, FILE_LIMIT + 3);
        assert_eq!(c.files.len(), FILE_LIMIT);
        assert!(c.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn untracked_links_and_fifos_are_not_read_or_followed() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-special");
        let outside = crate::scratch::Scratch::new("vcs-svn-secret");
        fs::write(outside.join("secret"), "SECRET OUTSIDE\n").unwrap();
        symlink(outside.join("secret"), wc.join("link")).unwrap();
        fs::create_dir(wc.join("fresh")).unwrap();
        symlink(&*outside, wc.join("fresh/out-dir")).unwrap();
        fs::write(wc.join("fresh/f"), "f\n").unwrap();
        let fifo = std::ffi::CString::new(wc.join("fifo").as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo is a valid C path in the isolated test directory.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let c = Backend::Svn.changes(&wc, &Scope::Working, Some(Duration::from_secs(20))).unwrap();
        let names: Vec<_> = c.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(names, ["fifo", "fresh/f", "fresh/out-dir", "link"], "{c:?}");
        let p = Backend::Svn.patch(&wc, &Scope::Working, None, Some(Duration::from_secs(20))).unwrap();
        assert!(!p.text.contains("SECRET OUTSIDE"), "{}", p.text);
        assert!(p.text.contains("non-regular file") && p.text.contains("(link)"), "{}", p.text);
    }

    #[test]
    fn ignored_directories_carry_their_slash_and_svn_s_own() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-ignored");
        svn_in(&wc, &["propset", "-q", "svn:ignore", "build\ntarget\n*.log", "./@"]);
        svn_in(&wc, &["propset", "-q", "svn:ignore", "cache", "./tree@"]);
        for d in ["build/deep", "target", "tree/cache"] {
            fs::create_dir_all(wc.join(d)).unwrap();
        }
        fs::write(wc.join("x.log"), "l\n").unwrap();
        let before = admin_snapshot(&wc);
        let dirs = Backend::Svn.ignored_dirs(&wc, None).unwrap();
        assert_eq!(dirs, [".svn/", "build/", "target/", "tree/cache/"]);
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");
    }

    #[test]
    fn reading_writes_nothing_and_runs_no_diff_cmd() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-readonly");
        fs::write(wc.join("m.txt"), "changed\n").unwrap();
        fs::write(wc.join("loose"), "u\n").unwrap();
        svn_in(&wc, &["rm", "-q", "./tree@"]);
        // A `diff-cmd` in svn's config runs on every `diff` that lacks `--internal-diff`. The config directory is set
        // for this thread only, so no other test's svn reads it — the reads below take no budget and stay here.
        let scratch = wc.parent().unwrap();
        let marker = scratch.join("DIFF_CMD_RAN");
        let script = scratch.join("diff-cmd.sh");
        fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let config = scratch.join("svn-config");
        fs::create_dir(&config).unwrap();
        fs::write(config.join("config"), format!("[helpers]\ndiff-cmd = {}\n", script.display())).unwrap();
        fs::write(config.join("servers"), "").unwrap();
        fs::write(config.join("README.txt"), "").unwrap();
        let before = admin_snapshot(&wc);
        crate::svn::tests::with_config(&config, || {
            let c = Backend::Svn.changes(&wc, &Scope::Working, None).unwrap();
            assert!(find(&c, "m.txt").adds.is_some(), "the diff was read: {c:?}");
            let p = Backend::Svn.patch_with_changes(&wc, &Scope::Working, None, None, &c).unwrap();
            assert!(p.text.contains("+changed"), "{}", p.text);
            Backend::Svn.patch(&wc, &Scope::Working, Some("m.txt"), None).unwrap();
            Backend::Svn.patch_of(&wc, &Scope::Working, &["tree/".into()], None, Some(&c)).unwrap();
            Backend::Svn.ignored_dirs(&wc, None).unwrap();
        });
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");
        assert!(!marker.exists(), "svn ran the user's diff-cmd");
        // The trap is armed: an svn `diff` without `--internal-diff` under the same config does run it.
        let mut cmd = std::process::Command::new("svn");
        cmd.current_dir(&wc).args(["diff", "--non-interactive", "--config-dir"]).arg(&config);
        let _ = cmd.stdin(std::process::Stdio::null()).output();
        assert!(marker.exists(), "the test's diff-cmd never runs, so it proves nothing");
    }

    #[test]
    fn a_project_below_the_working_copy_root_reports_its_own_paths() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = working_copy("vcs-svn-sub");
        fs::write(wc.join("tree/r"), "changed\n").unwrap();
        fs::write(wc.join("m.txt"), "outside\n").unwrap();
        fs::write(wc.join("tree/new"), "n\n").unwrap();
        let sub = wc.join("tree");
        let c = Backend::Svn.changes(&sub, &Scope::Working, None).unwrap();
        assert_eq!(paths(&c), [("new", Status::Untracked), ("r", Status::Modified)]);
        let p = Backend::Svn.patch(&sub, &Scope::Working, None, None).unwrap();
        assert!(p.text.contains("Index: r\n") && !p.text.contains("outside"), "{}", p.text);
        // A root svn does not version is one error, not an empty list.
        fs::create_dir(wc.join("plain")).unwrap();
        fs::write(wc.join("plain/f"), "f\n").unwrap();
        let got = Backend::Svn.changes(&wc.join("plain"), &Scope::Working, None);
        assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Failed(_)))), "{got:?}");
        let outside = crate::scratch::Scratch::new("vcs-svn-none");
        let got = Backend::Svn.changes(&outside, &Scope::Working, None);
        assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Failed(_)))), "{got:?}");
    }

    #[test]
    fn a_revision_is_r_and_digits_and_nothing_else_reaches_svn() {
        for (word, rev) in [("r1", Some(1)), ("r12", Some(12)), ("r4294967296", Some(4_294_967_296))] {
            assert_eq!(revision(word), rev, "{word}");
        }
        for bad in ["r12 --diff-cmd=x", "-c", "12", "r", "r0", "r12345678901", "rx", "r-1", "r1 2", "R12", "r+1", "r１"]
        {
            assert_eq!(revision(bad), None, "{bad:?}");
            let scope = Scope::Commits(vec!["r3".into(), bad.into()]);
            // Refused before any program runs: the root need not even be a working copy.
            let nowhere = Path::new("/nonexistent-moai-root");
            assert!(matches!(Backend::Svn.changes(nowhere, &scope, None), Err(Error::InvalidRevision)), "{bad:?}");
            assert!(matches!(Backend::Svn.patch(nowhere, &scope, None, None), Err(Error::InvalidRevision)), "{bad:?}");
        }
        // Git takes hashes only: an svn revision is no revision there.
        let dir = crate::scratch::Scratch::new("vcs-svn-git-rev");
        let scope = Scope::Commits(vec!["r12".into()]);
        assert!(matches!(Backend::Git.changes(&dir, &scope, None), Err(Error::InvalidRevision)));
        assert!(matches!(Backend::Git.patch(&dir, &scope, None, None), Err(Error::InvalidRevision)));
    }

    #[test]
    fn commits_read_each_revision_from_the_repository() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("vcs-svn-commits");
        let wc = repo.checkout("trunk", "wc");
        commit_files(&wc, &[("a.txt", "a\nb\nc\n"), ("gone.txt", "g\n")], "r2 base");
        fs::write(wc.join("a.txt"), "a\nB\nc\nd\n").unwrap();
        fs::write(wc.join("new.txt"), "n1\nn2\n").unwrap();
        svn_in(&wc, &["add", "-q", "./new.txt@"]);
        svn_in(&wc, &["rm", "-q", "./gone.txt@"]);
        svn_in(&wc, &["commit", "-q", "-m", "r3 work"]);
        repo.branch("x"); // r4: another branch only
        commit_files(&wc, &[("sub/s.txt", "s\n")], "r5 sub");
        // svn marks a file with NUL bytes binary itself when it is added.
        commit_files(&wc, &[("bin", "\0\x01")], "r6 bin");
        fs::write(wc.join("bin"), "\0\x02").unwrap();
        svn_in(&wc, &["commit", "-q", "-m", "r7 binary edit"]);
        svn_in(&wc, &["update", "-q"]);
        // Local changes are not a revision's.
        fs::write(wc.join("a.txt"), "local\n").unwrap();
        let before = admin_snapshot(&wc);
        let commits = |revs: &[&str]| Scope::Commits(revs.iter().map(|r| r.to_string()).collect());

        let c = Backend::Svn.changes(&wc, &commits(&["r3"]), None).unwrap();
        assert_eq!(paths(&c), [("a.txt", Status::Modified), ("gone.txt", Status::Deleted), ("new.txt", Status::Added)]);
        assert_eq!((find(&c, "a.txt").adds, find(&c, "a.txt").dels), (Some(2), Some(1)));
        assert_eq!((find(&c, "new.txt").adds, find(&c, "gone.txt").dels), (Some(2), Some(1)));
        // The patch is svn's own `diff -c` of the root's URL at BASE, as printed.
        let p = Backend::Svn.patch(&wc, &commits(&["r3"]), None, None).unwrap();
        let url = format!("{}/trunk@7", repo.url);
        let svn_says =
            svn_in(&wc, &["diff", "--internal-diff", "--ignore-properties", "--show-copies-as-adds", "-c", "3", &url]);
        assert_eq!(p.text, svn_says);
        assert!(!p.truncated && !p.text.contains("local"));
        for f in &c.files {
            assert_eq!(
                (Some(counted(&hunks_of(&p.text, &f.path)).0), Some(counted(&hunks_of(&p.text, &f.path)).1)),
                (f.adds, f.dels)
            );
        }
        // Narrowed: read whole, cut here.
        let one = Backend::Svn.patch_of(&wc, &commits(&["r3"]), &["new.txt".into()], None, None).unwrap();
        assert_eq!(crate::diff::parse(&one.text).len(), 1, "{}", one.text);
        assert!(one.text.contains("+n2"));

        // Oldest first: a file added then edited stays added, lines summed; added then removed is removed.
        let both = Backend::Svn.changes(&wc, &commits(&["r2", "r3"]), None).unwrap();
        assert_eq!(
            (find(&both, "a.txt").status, find(&both, "a.txt").adds, find(&both, "a.txt").dels),
            (Status::Added, Some(5), Some(1))
        );
        assert_eq!(find(&both, "gone.txt").status, Status::Deleted);
        let p = Backend::Svn.patch(&wc, &commits(&["r2", "r3"]), None, None).unwrap();
        assert_eq!(p.text.matches("Index: a.txt").count(), 2);

        // A binary file is flagged and carries no counts.
        let bin = Backend::Svn.changes(&wc, &commits(&["r7"]), None).unwrap();
        assert!(find(&bin, "bin").binary && find(&bin, "bin").adds.is_none(), "{bin:?}");

        // A revision of another branch, and one before the path existed, are empty — not failures.
        assert_eq!(Backend::Svn.changes(&wc, &commits(&["r4"]), None).unwrap(), Changes::default());
        assert!(Backend::Svn.patch(&wc, &commits(&["r4"]), None, None).unwrap().text.is_empty());
        let sub = wc.join("sub");
        assert_eq!(Backend::Svn.changes(&sub, &commits(&["r3"]), None).unwrap(), Changes::default());
        assert!(Backend::Svn.patch(&sub, &commits(&["r3"]), None, None).unwrap().text.is_empty());
        let in_sub = Backend::Svn.changes(&sub, &commits(&["r5"]), None).unwrap();
        assert_eq!(paths(&in_sub), [("s.txt", Status::Added)], "paths are the project's own");

        assert!(matches!(Backend::Svn.changes(&wc, &commits(&["r3"]), Some(Duration::ZERO)), Err(Error::Timeout)));
        assert!(matches!(Backend::Svn.patch(&wc, &commits(&["r3"]), None, Some(Duration::ZERO)), Err(Error::Timeout)));
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");

        // The repository gone: the patch is `offline`, one error.
        fs::remove_dir_all(repo.scratch.join("repo")).unwrap();
        let got = Backend::Svn.patch(&wc, &commits(&["r3"]), None, None);
        assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Offline(_)))), "{got:?}");
        let got = Backend::Svn.changes(&wc, &commits(&["r3"]), None);
        assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Offline(_)))), "{got:?}");
    }

    /// The encoded URL of `path` under the repository — the crate's own builder.
    fn url_of(repo: &Repo, path: &[&str]) -> String {
        crate::svn::url(&repo.url, &path.iter().map(|p| p.to_string()).collect::<Vec<_>>()).expect("url")
    }
    /// Checks `path` out to `<scratch>/<dir>`, any name in it.
    fn checkout_at(repo: &Repo, path: &[&str], dir: &str) -> std::path::PathBuf {
        let wc = repo.scratch.join(dir);
        svn_in(&repo.scratch, &["checkout", "-q", &format!("{}@", url_of(repo, path)), &wc.to_string_lossy()]);
        wc
    }

    /// trunk r2 → `branches/<name>` r3 → two branch commits r4·r5 → one trunk commit r6, then on the branch's working
    /// copy an uncommitted edit, an added file and an untracked one. `sub` is a directory in every revision.
    fn branched(test: &str, name: &str, sub: &str) -> (Repo, std::path::PathBuf, std::path::PathBuf) {
        let repo = Repo::new(test);
        let trunk = repo.checkout("trunk", "trunk-wc");
        let s = |f: &str| format!("{sub}/{f}");
        commit_files(
            &trunk,
            &[("a.txt", "a\nb\nc\n"), ("gone.txt", "g\n"), ("same.txt", "same\n"), (&s("s.txt"), "s\n")],
            "r2 base",
        );
        let from = format!("{}/trunk", repo.url);
        // svn reads a copy's target `%40` as a peg too: the empty peg after it keeps the name whole.
        let to = format!("{}@", url_of(&repo, &["branches", name]));
        svn_in(&repo.scratch, &["copy", "-q", "-m", "r3 branch", &from, &to]);
        let wc = checkout_at(&repo, &["branches", name], "branch-wc");
        commit_files(&wc, &[("a.txt", "a\nB\nc\n"), ("new1.txt", "n1\n")], "r4 on the branch");
        svn_in(&wc, &["rm", "-q", "./gone.txt@"]);
        commit_files(&wc, &[(&s("s.txt"), "s\nbranch s\n")], "r5 on the branch");
        commit_files(&trunk, &[("a.txt", "a\nb\nc\ntrunk\n"), ("trunk.txt", "t\n"), (&s("t.txt"), "t\n")], "r6 trunk");
        svn_in(&wc, &["update", "-q"]);
        fs::write(wc.join("a.txt"), "a\nB\nc\nlocal\n").unwrap();
        fs::write(wc.join("added.txt"), "added\n").unwrap();
        svn_in(&wc, &["add", "-q", "./added.txt@"]);
        fs::write(wc.join("u.txt"), "untracked\n").unwrap();
        fs::write(wc.join(s("local.txt")), "in sub\n").unwrap();
        fs::write(wc.join(s("s.txt")), "s\nbranch s\nlocal s\n").unwrap();
        (repo, trunk, wc)
    }

    /// Every row's hunks are in the patch by the rule the surfaces use, with the row's counts; a file is one section.
    fn rows_match_patch(c: &Changes, p: &Patch) {
        for f in c.files.iter().filter(|f| !f.binary) {
            let h = hunks_of(&p.text, &f.path);
            assert!(!h.is_empty(), "{} has no hunk in\n{}", f.path, p.text);
            assert_eq!((Some(counted(&h).0), Some(counted(&h).1)), (f.adds, f.dels), "{}", f.path);
            let sections = crate::diff::parse(&p.text).iter().filter(|d| hunks_of_one(d, &f.path)).count();
            assert_eq!(sections, 1, "{} is in {sections} sections:\n{}", f.path, p.text);
        }
    }
    fn hunks_of_one(f: &crate::diff::FileDiff, name: &str) -> bool {
        f.new_path.as_deref() == Some(name) || (f.new_path.is_none() && f.old_path.as_deref() == Some(name))
    }

    #[test]
    fn branch_reads_the_fork_point_to_base_and_whole_adds_the_working_copy() {
        if !svn_ready() {
            return;
        }
        let (repo, trunk, wc) = branched("vcs-svn-branch", "x", "sub");
        let before = admin_snapshot(&wc);
        use Status::*;

        let b = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
        assert_eq!(
            paths(&b),
            [("a.txt", Modified), ("gone.txt", Deleted), ("new1.txt", Added), ("sub/s.txt", Modified)],
            "the two branch commits only: no trunk commit after the fork, nothing from before it, nothing uncommitted"
        );
        assert_eq!((find(&b, "a.txt").adds, find(&b, "a.txt").dels), (Some(1), Some(1)));
        let bp = Backend::Svn.patch(&wc, &Scope::Branch, None, None).unwrap();
        assert!(!bp.text.contains("local") && !bp.text.contains("+trunk") && !bp.truncated, "{}", bp.text);
        rows_match_patch(&b, &bp);
        let one = Backend::Svn.patch_of(&wc, &Scope::Branch, &["sub/".into()], None, Some(&b)).unwrap();
        assert_eq!(crate::diff::parse(&one.text).len(), 1, "{}", one.text);

        let w = Backend::Svn.changes(&wc, &Scope::Whole, None).unwrap();
        assert_eq!(
            paths(&w),
            [
                ("a.txt", Modified),
                ("added.txt", Added),
                ("gone.txt", Deleted),
                ("new1.txt", Added),
                ("sub/local.txt", Untracked),
                ("sub/s.txt", Modified),
                ("u.txt", Untracked),
            ],
            "committed, uncommitted and untracked in one list"
        );
        assert_eq!((find(&w, "a.txt").adds, find(&w, "a.txt").dels), (Some(2), Some(1)), "one row, both changes");
        let wp = Backend::Svn.patch_with_changes(&wc, &Scope::Whole, None, None, &w).unwrap();
        assert!(wp.text.contains("+local") && wp.text.contains("+untracked") && !wp.text.contains("+trunk"));
        rows_match_patch(&w, &wp);
        assert_eq!(Backend::Svn.patch(&wc, &Scope::Whole, None, None).unwrap().text, wp.text);
        let narrowed = Backend::Svn.patch_of(&wc, &Scope::Whole, &["sub/".into()], None, Some(&w)).unwrap();
        assert_eq!(hunks_of(&narrowed.text, "sub/s.txt"), hunks_of(&wp.text, "sub/s.txt"));
        assert!(narrowed.text.contains("+in sub") && !narrowed.text.contains("+local\n"), "{}", narrowed.text);
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");

        // The trunk: `branch` is empty — no failure — and `whole` is `working`.
        fs::write(trunk.join("a.txt"), "on trunk\n").unwrap();
        fs::write(trunk.join("loose"), "l\n").unwrap();
        let trunk_before = admin_snapshot(&trunk);
        assert_eq!(Backend::Svn.changes(&trunk, &Scope::Branch, None).unwrap(), Changes::default());
        assert!(Backend::Svn.patch(&trunk, &Scope::Branch, None, None).unwrap().text.is_empty());
        let working = Backend::Svn.changes(&trunk, &Scope::Working, None).unwrap();
        let working_patch = Backend::Svn.patch(&trunk, &Scope::Working, None, None).unwrap().text;
        assert_eq!(Backend::Svn.changes(&trunk, &Scope::Whole, None).unwrap(), working);
        assert_eq!(Backend::Svn.patch(&trunk, &Scope::Whole, None, None).unwrap().text, working_patch);

        for scope in [Scope::Branch, Scope::Whole] {
            let late = Backend::Svn.changes(&wc, &scope, Some(Duration::ZERO));
            assert!(matches!(late, Err(Error::Timeout)), "{scope:?}: {late:?}");
            let late = Backend::Svn.patch(&wc, &scope, None, Some(Duration::ZERO));
            assert!(matches!(late, Err(Error::Timeout)), "{scope:?}: {late:?}");
        }

        // The repository gone: the branch's two scopes are `offline`, no hang; `working` and the trunk still answer.
        fs::remove_dir_all(repo.scratch.join("repo")).unwrap();
        for scope in [Scope::Branch, Scope::Whole] {
            let got = Backend::Svn.changes(&wc, &scope, Some(Duration::from_secs(20)));
            assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Offline(_)))), "{scope:?}: {got:?}");
            let got = Backend::Svn.patch(&wc, &scope, None, None);
            assert!(matches!(got, Err(Error::Svn(crate::svn::Error::Offline(_)))), "{scope:?}: {got:?}");
        }
        assert!(Backend::Svn.changes(&wc, &Scope::Working, None).unwrap().files.iter().any(|f| f.path == "a.txt"));
        assert_eq!(Backend::Svn.changes(&trunk, &Scope::Whole, None).unwrap(), working);
        assert_eq!(Backend::Svn.patch(&trunk, &Scope::Whole, None, None).unwrap().text, working_patch);
        assert_eq!(Backend::Svn.changes(&trunk, &Scope::Branch, None).unwrap(), Changes::default());
        assert_eq!((admin_snapshot(&wc), admin_snapshot(&trunk)), (before, trunk_before), ".svn changed");
    }

    /// **A commit made in the working copy is in `branch` before any `svn update`** — `svn commit` leaves the root's
    /// revision behind (a mixed-revision working copy), so the new side is each node's BASE, never `<URL>@<root's
    /// revision>`, which read the branch as empty here. The local edit stays out, and `.svn` is as it was.
    #[test]
    fn branch_takes_a_commit_made_here_before_any_update() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("vcs-svn-branch-mixed");
        let trunk = repo.checkout("trunk", "trunk-wc");
        commit_files(&trunk, &[("a.txt", "a\n")], "r2 base");
        repo.branch("x");
        let wc = repo.checkout("branches/x", "branch-wc");
        let base = crate::svn::info(&wc, "", None).unwrap().revision;
        commit_files(&wc, &[("new.txt", "n\n")], "r4 on the branch");
        assert_eq!(crate::svn::info(&wc, "", None).unwrap().revision, base, "svn commit leaves the root's revision");
        fs::write(wc.join("a.txt"), "a\nlocal\n").unwrap();
        let before = admin_snapshot(&wc);
        let b = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
        assert_eq!(paths(&b), [("new.txt", Status::Added)], "the commit made here, not the local edit");
        let p = Backend::Svn.patch(&wc, &Scope::Branch, None, None).unwrap();
        assert!(p.text.contains("+n\n") && !p.text.contains("+local"), "{}", p.text);
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");
    }

    #[test]
    fn a_branch_subdirectory_with_hostile_names_reports_its_own_paths() {
        if !svn_ready() {
            return;
        }
        let (name, sub) = ("한글 x@#?%", "s p@1#");
        let (_repo, _trunk, wc) = branched("vcs-svn-branch-names", name, sub);
        let root = wc.join(sub);
        let before = admin_snapshot(&wc);
        assert_eq!(
            crate::svn::layout_at(&root, None).unwrap().branch_name(),
            Some(name),
            "the one place that says which branch this is"
        );
        let b = Backend::Svn.changes(&root, &Scope::Branch, None).unwrap();
        assert_eq!(paths(&b), [("s.txt", Status::Modified)], "inside the subdirectory only, its own paths");
        let bp = Backend::Svn.patch(&root, &Scope::Branch, None, None).unwrap();
        assert!(bp.text.contains("+branch s") && !bp.text.contains("local s"), "{}", bp.text);
        let w = Backend::Svn.changes(&root, &Scope::Whole, None).unwrap();
        assert_eq!(paths(&w), [("local.txt", Status::Untracked), ("s.txt", Status::Modified)]);
        assert_eq!((find(&w, "s.txt").adds, find(&w, "s.txt").dels), (Some(2), Some(0)));
        let wp = Backend::Svn.patch_with_changes(&root, &Scope::Whole, None, None, &w).unwrap();
        rows_match_patch(&w, &wp);
        assert!(!wp.text.contains("a.txt") && !wp.text.contains("t.txt"), "{}", wp.text);
        // The branch root itself, by the same names.
        let all = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
        assert_eq!(all.files.len(), 4, "{all:?}");
        assert!(find(&all, &format!("{sub}/s.txt")).adds == Some(1));
        assert_eq!(admin_snapshot(&wc), before, ".svn changed");
    }

    #[test]
    fn without_a_fork_point_branch_says_why_and_whole_is_working() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("vcs-svn-nobase");
        svn_in(&repo.scratch, &["mkdir", "-q", "-m", "src", &format!("{}/src", repo.url)]);
        svn_in(&repo.scratch, &["mkdir", "-q", "-m", "plain", &format!("{}/branches/plain", repo.url)]);
        let trunk = repo.checkout("trunk", "trunk-wc");
        commit_files(&trunk, &[("a.txt", "a\n")], "trunk");
        svn_in(
            &repo.scratch,
            &["copy", "-q", "-m", "tag", &format!("{}/trunk", repo.url), &url_of(&repo, &["tags", "1.0"])],
        );
        let mut places = Vec::new();
        for (path, dir) in
            [(&["src"][..], "src-wc"), (&["branches", "plain"], "plain-wc"), (&["tags", "1.0"], "tag-wc")]
        {
            let wc = checkout_at(&repo, path, dir);
            commit_files(&wc, &[("f.txt", "f\n")], &format!("{dir} file"));
            svn_in(&wc, &["update", "-q"]);
            fs::write(wc.join("f.txt"), "changed\n").unwrap();
            fs::write(wc.join("loose"), "u\n").unwrap();
            places.push(wc);
        }
        let [src, plain, tag] = [&places[0], &places[1], &places[2]];
        let reason = |wc: &Path| match Backend::Svn.changes(wc, &Scope::Branch, None) {
            Err(Error::NoBase(why)) => Some(why),
            other => panic!("{}: {other:?}", wc.display()),
        };
        assert_eq!(reason(src), Some(super::super::NoBase::Convention), "no convention: neither empty nor a list");
        assert_eq!(reason(tag), Some(super::super::NoBase::Convention), "a tag is no branch");
        assert_eq!(reason(plain), Some(super::super::NoBase::NotCopied), "a branch made by mkdir, told apart");
        assert!(matches!(
            Backend::Svn.patch(plain, &Scope::Branch, None, None),
            Err(Error::NoBase(super::super::NoBase::NotCopied))
        ));
        let same = |wc: &Path| {
            let working = Backend::Svn.changes(wc, &Scope::Working, None).unwrap();
            assert_eq!(working.files.len(), 2, "{working:?}");
            assert_eq!(Backend::Svn.changes(wc, &Scope::Whole, None).unwrap(), working, "{}", wc.display());
            let text = Backend::Svn.patch(wc, &Scope::Working, None, None).unwrap().text;
            assert_eq!(Backend::Svn.patch(wc, &Scope::Whole, None, None).unwrap().text, text, "{}", wc.display());
        };
        for wc in [src, plain, tag] {
            same(wc);
        }
        // With the repository gone, the working copies of no convention still answer `whole`; a branch made by
        // `svn mkdir` does too once its creation is known.
        fs::remove_dir_all(repo.scratch.join("repo")).unwrap();
        for wc in [src, plain, tag] {
            same(wc);
        }
        assert_eq!(reason(src), Some(super::super::NoBase::Convention));
    }

    #[test]
    fn the_fork_point_is_kept_beside_the_revision_table_and_read_from_there() {
        if !svn_ready() {
            return;
        }
        let (_repo, _trunk, wc) = branched("vcs-svn-forks", "x", "sub");
        commit_files(&wc, &[("named.txt", "n\n")], "feat: named (moai-ab12)");
        svn_in(&wc, &["update", "-q"]);
        let dir = crate::scratch::Scratch::new("vcs-svn-forks-cache");
        let here = crate::svn::info(&wc, "", None).unwrap();
        let file = crate::svn::cache::path_in(&dir, &here.uuid).unwrap();
        crate::svn::cache::tests::with_dir(Some(&dir), || {
            let commits = crate::svn::commits_within(&wc, "moai-ab12", None).unwrap();
            assert_eq!(commits.len(), 1);
            let table = crate::svn::cache::read(&file, &here.uuid);
            let b = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
            assert_eq!(find(&b, "a.txt").status, Status::Modified);
            let kept = crate::svn::cache::read(&file, &here.uuid);
            assert_eq!((kept.through, &kept.revs, &kept.checked_at), (table.through, &table.revs, &table.checked_at));
            let created = kept.fork("/branches/x").expect("the fork point is kept");
            assert_eq!(created.rev, 3);
            assert_eq!(created.from, Some(crate::svn::Fork { path: vec!["trunk".into()], rev: 2 }));
            // Read from the file, not asked again: a fork point moved back to r1 (an empty trunk) shows in the list.
            let mut moved = kept.clone();
            moved.set_fork(
                "/branches/x",
                &crate::svn::Created { rev: 3, from: Some(crate::svn::Fork { path: vec!["trunk".into()], rev: 1 }) },
            );
            crate::svn::cache::write(&file, &moved).unwrap();
            let b = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
            assert_eq!(find(&b, "a.txt").status, Status::Added, "{b:?}");
            // An entry out of shape is dropped and asked again — never put on svn's command line.
            let mut bad = moved.clone();
            bad.forks.insert("/branches/x".into(), serde_json::json!({"created": 3, "from": "/../x", "rev": 2}));
            crate::svn::cache::write(&file, &bad).unwrap();
            let b = Backend::Svn.changes(&wc, &Scope::Branch, None).unwrap();
            assert_eq!(find(&b, "a.txt").status, Status::Modified);
            assert_eq!(crate::svn::cache::read(&file, &here.uuid).fork("/branches/x"), Some(created));
        });
    }
}
