//! The id → revision table and the file that keeps it (moai-0d0s.4o8).
//!
//! The `commits` scope asks "which revisions name this id". svn answers that only from the server's log, so the
//! answer is kept in the person's cache directory — one file per repository, named by its UUID:
//! `$XDG_CACHE_HOME/moai/svn/<UUID>.json` when that variable is absolute, else `<home>/.cache/moai/svn/<UUID>.json`.
//! Nothing is written under `.moai` or the working copy, and no snapshot or journal field carries any of it —
//! the table is derived from the server's log and can always be read again.
//!
//! What the file holds and why:
//! - `version`, `uuid`: a file of another version or another repository is no table at all — it is rebuilt
//! - `through`: the last revision read. The next read asks `through + 1 ..= HEAD` only
//! - `checked_at`: when the table was last found complete up to HEAD. The server is asked again only [`POLL_SECS`]
//!   later (a time ahead of now asks at once). **Unset means the first fill never finished** — such a table is never
//!   answered from, since an id's revisions may sit in the part not read yet
//! - `revs`: the revisions whose subject or `Refs:`·`Closes:`·`Fixes:` lines carry an id-shaped word, with the
//!   subject and those words. Words are read by the rule Git's commit table uses ([`crate::git::words`],
//!   [`crate::git::trailed`]) and kept when [`crate::git::ids_in`] takes them, whatever project they belong to —
//!   one repository may hold several projects and ids may be made later
//! - `forks`: branch fork points, filled by moai-0d0s.x27. This member keeps it as it was read
//! - any other key is kept as read, so an older binary does not drop what a newer one wrote
//!
//! Writes go to a fresh temporary file beside the target and are renamed over it — a reader sees the old file or the
//! new one, never half of either. There is no lock: two processes may both fill, the last rename wins and what it
//! lacks is read again next time. A write that fails is not an error — the table read is still the answer.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The file's format. A file of another version is rebuilt, not read.
pub(crate) const VERSION: u64 = 1;
/// Revisions asked of the server in one `log` while filling.
pub(crate) const CHUNK: u64 = 2_000;
/// The least time between two asks of the server for new revisions.
pub(crate) const POLL_SECS: i64 = 60;

/// A revision that names ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Rev {
    pub rev: u64,
    pub subject: String,
    pub tokens: Vec<String>,
}

/// The table as the file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Table {
    pub version: u64,
    pub uuid: String,
    pub through: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub revs: Vec<Rev>,
    /// moai-0d0s.x27's fork points. Kept as read.
    #[serde(default)]
    pub forks: serde_json::Map<String, serde_json::Value>,
    /// Keys this binary does not know, kept as read.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Table {
    pub(crate) fn new(uuid: &str) -> Table {
        Table {
            version: VERSION,
            uuid: uuid.to_owned(),
            through: 0,
            checked_at: None,
            revs: Vec::new(),
            forks: serde_json::Map::new(),
            extra: serde_json::Map::new(),
        }
    }

    /// Whether the table was ever found complete up to HEAD — only such a table may answer.
    pub(crate) fn complete(&self) -> bool {
        self.checked_at.is_some()
    }

    /// The file's bytes. Reading them back and writing again gives the same bytes.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec(self).unwrap_or_default();
        out.push(b'\n');
        out
    }

    /// The table these bytes hold, or `None` when they are not one this binary can trust for `uuid`: unreadable,
    /// another version or repository, or a value out of shape (a revision past `through`, out of order, a word that is
    /// no id, an unreadable time).
    pub(crate) fn of_bytes(bytes: &[u8], uuid: &str) -> Option<Table> {
        let table: Table = serde_json::from_slice(bytes).ok()?;
        let sane = table.version == VERSION
            && table.uuid == uuid
            && table.checked_at.as_deref().is_none_or(|t| crate::model::parse_rfc3339(t).is_some())
            && table.revs.windows(2).all(|w| w[0].rev < w[1].rev)
            && table.revs.iter().all(|r| {
                (1..=table.through).contains(&r.rev)
                    && !r.tokens.is_empty()
                    && r.tokens.iter().all(|t| crate::git::ids_in(t).eq([t.as_str()]))
            });
        sane.then_some(table)
    }

    /// Whether the server need not be asked now: the table is complete and was checked less than [`POLL_SECS`] ago.
    /// A check time ahead of `now` (a clock set back) asks at once.
    pub(crate) fn fresh(&self, now: i64) -> bool {
        self.checked_at
            .as_deref()
            .and_then(crate::model::parse_rfc3339)
            .is_some_and(|at| at <= now && now - at < POLL_SECS)
    }

    /// The revisions that name `id`, newest first — the order [`crate::git::commits_within`] gives. A revision is
    /// written `r<number>`; tracker revisions are marked, not dropped.
    pub(crate) fn commits(&self, id: &str) -> Vec<crate::git::Commit> {
        self.revs
            .iter()
            .rev()
            .filter(|r| r.tokens.iter().any(|t| t == id))
            .map(|r| crate::git::Commit {
                hash: format!("r{}", r.rev),
                subject: r.subject.clone(),
                tracker: r.subject.starts_with(crate::git::TRACKER),
            })
            .collect()
    }
}

/// One log message read: its subject (the first line) and the ids it names, by Git's rule — the subject's words
/// and the words of its `Refs:`·`Closes:`·`Fixes:` lines, never a word of plain body text. `None` when it names none.
pub(crate) fn rev_of(rev: u64, message: &str) -> Option<Rev> {
    let (subject, body) = message.split_once('\n').unwrap_or((message, ""));
    let subject = subject.trim_end_matches('\r');
    let mut tokens: Vec<String> = Vec::new();
    for t in crate::git::ids_in(subject).chain(crate::git::trailed(body).flat_map(crate::git::ids_in)) {
        if !tokens.iter().any(|k| k == t) {
            tokens.push(t.to_owned());
        }
    }
    (!tokens.is_empty()).then(|| Rev { rev, subject: subject.to_owned(), tokens })
}

/// A UUID as `svn info` prints it: 36 characters, hex digits and hyphens at 8·13·18·23. Nothing else names a file.
pub(crate) fn valid_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// The directory the tables live in, from an environment: `$XDG_CACHE_HOME/moai/svn` when that is absolute, else
/// `<home>/.cache/moai/svn` with home as the user configuration finds it. `None` when neither is known — then every
/// read asks the server.
pub(crate) fn dir_from(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let xdg = env("XDG_CACHE_HOME").map(PathBuf::from).filter(|p| p.is_absolute());
    let base = match xdg {
        Some(x) => x,
        None => crate::user_config::home_here(&env).filter(|h| h.is_absolute())?.join(".cache"),
    };
    Some(base.join("moai").join("svn"))
}

/// The directory this process keeps its tables in. Test builds keep theirs in their own scratch — a test never
/// writes the person's `~/.cache`.
#[cfg(not(test))]
pub(crate) fn dir() -> Option<PathBuf> {
    dir_from(|k| std::env::var_os(k))
}
#[cfg(test)]
pub(crate) fn dir() -> Option<PathBuf> {
    tests::dir_override()
        .unwrap_or_else(|| Some(crate::scratch::base().join(format!("moai-svn-cache-{}", std::process::id()))))
}

/// The table file for `uuid` under `dir`, or `None` when the UUID is not one.
pub(crate) fn path_in(dir: &Path, uuid: &str) -> Option<PathBuf> {
    valid_uuid(uuid).then(|| dir.join(format!("{uuid}.json")))
}

/// The table at `path` for `uuid`, or a new one when the file is missing, is not a regular file or cannot be
/// trusted ([`Table::of_bytes`]).
pub(crate) fn read(path: &Path, uuid: &str) -> Table {
    let regular = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file());
    regular
        .then(|| std::fs::read(path).ok())
        .flatten()
        .and_then(|b| Table::of_bytes(&b, uuid))
        .unwrap_or_else(|| Table::new(uuid))
}

/// Writes the table: its directory made private (0700), a new private (0600) temporary file beside the target, then
/// one rename. A failure leaves the old file as it was.
pub(crate) fn write(path: &Path, table: &Table) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NTH: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or_else(|| std::io::Error::other("no directory"))?;
    private_dir(dir)?;
    let name = path.file_name().ok_or_else(|| std::io::Error::other("no file name"))?.to_string_lossy();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let temp = dir.join(format!(".{name}.{}.{nanos}.{}.tmp", std::process::id(), NTH.fetch_add(1, Ordering::Relaxed)));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let written = options.open(&temp).and_then(|mut f| {
        f.write_all(&table.to_bytes())?;
        f.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&temp, path));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    renamed
}

fn private_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
        std::fs::DirBuilder::new().mode(0o700).recursive(true).create(dir)?;
        let meta = std::fs::symlink_metadata(dir)?;
        if !meta.is_dir() {
            return Err(std::io::Error::other(format!("{} is not a directory", dir.display())));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)
}

/// What one `log` over a range gave.
#[derive(Debug)]
pub(crate) enum Fetched {
    /// The revisions in the range that name ids.
    Revs(Vec<Rev>),
    /// The output passed its limit: ask a smaller range.
    Cut,
}

/// Fills the table from `through + 1` up to `head`, [`CHUNK`]-sized ranges at a time (`chunk` in tests), handing each
/// filled range to `save`. A range whose output is cut is asked again at half the size; a single revision still cut
/// is passed over as naming nothing. Done, the table is complete as of `now`. A `head` below `through` (the
/// repository was replaced) starts again from r1. A failed `fetch` stops the fill with its error — what was filled is
/// already saved.
pub(crate) fn fill(
    table: &mut Table,
    head: u64,
    chunk: u64,
    now: &str,
    mut fetch: impl FnMut(u64, u64) -> Result<Fetched, crate::svn::Error>,
    mut save: impl FnMut(&Table),
) -> Result<(), crate::svn::Error> {
    if head < table.through {
        table.through = 0;
        table.revs.clear();
        table.checked_at = None;
    }
    let mut size = chunk.max(1);
    while table.through < head {
        let from = table.through + 1;
        let to = head.min(table.through.saturating_add(size));
        match fetch(from, to)? {
            Fetched::Revs(mut revs) => {
                revs.retain(|r| (from..=to).contains(&r.rev) && !r.tokens.is_empty());
                revs.sort_by_key(|r| r.rev);
                revs.dedup_by_key(|r| r.rev);
                table.revs.extend(revs);
                table.through = to;
            }
            Fetched::Cut if size > 1 => {
                size /= 2;
                continue;
            }
            Fetched::Cut => table.through = from,
        }
        save(table);
    }
    table.checked_at = Some(now.to_owned());
    save(table);
    Ok(())
}

/// What a stopped fill answers. A table found complete before keeps answering — a minute-old table beats no answer.
/// Otherwise the stop is the answer: a server out of reach, or one call past its own server time, is `offline`. A
/// fill the whole read's time ran out on (`out_of_time` — the caller's budget, or the read's overall cap) once some of
/// the table was read is [`crate::svn::Error::Filling`]: the next call carries on from where this one stopped.
pub(crate) fn settle(table: &Table, stop: crate::svn::Error, out_of_time: bool) -> Result<(), crate::svn::Error> {
    if table.complete() {
        return Ok(());
    }
    Err(if out_of_time && table.through > 0 { crate::svn::Error::Filling } else { stop })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use crate::svn::Error;

    thread_local! {
        static DIR: std::cell::RefCell<Option<Option<PathBuf>>> = const { std::cell::RefCell::new(None) };
    }

    pub(crate) fn dir_override() -> Option<Option<PathBuf>> {
        DIR.with(|d| d.borrow().clone())
    }

    /// `f` runs with `dir` as the cache directory — **on this thread only**; `None` is "no directory known".
    pub(crate) fn with_dir<T>(dir: Option<&Path>, f: impl FnOnce() -> T) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                DIR.with(|d| *d.borrow_mut() = None);
            }
        }
        DIR.with(|d| *d.borrow_mut() = Some(dir.map(Path::to_path_buf)));
        let _reset = Reset;
        f()
    }

    const UUID: &str = "0f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a";
    const T0: &str = "2026-11-18T00:00:00Z";

    fn at(secs: i64) -> String {
        crate::model::format_rfc3339(crate::model::parse_rfc3339(T0).unwrap() + secs)
    }
    fn now(secs: i64) -> i64 {
        crate::model::parse_rfc3339(T0).unwrap() + secs
    }

    #[test]
    fn the_table_takes_subject_and_trailer_ids_only() {
        let r = rev_of(3, "feat: x (moai-ab12)\n\nnot moai-cd34 in prose\nRefs: moai-ef56\n  closes: moai-gh78.\n")
            .expect("names ids");
        assert_eq!(r.subject, "feat: x (moai-ab12)");
        assert_eq!(r.tokens, ["moai-ab12", "moai-ef56", "moai-gh78"]);
        assert!(rev_of(4, "plain\n\nmentions moai-ab12 in the body\n").is_none(), "a body id is not taken");
        let only_trailer = rev_of(5, "plain\n\nRefs: moai-ab12\n").unwrap();
        assert_eq!(only_trailer.tokens, ["moai-ab12"]);
        assert!(rev_of(6, "flags --json and -c only").is_none());
        assert_eq!(rev_of(7, "x\r\nRefs: moai-ab12\r\n").unwrap().subject, "x");

        let mut t = Table::new(UUID);
        t.through = 9;
        for (rev, msg) in [
            (2, "work on moai-ab12.xyz"),
            (3, "branch worktree-moai-ab12"),
            (4, "chore(tracker): pick moai-ab12"),
            (5, "feat: thing (moai-ab12)"),
            (6, "x\n\nRefs: moai-ab12"),
        ] {
            t.revs.push(rev_of(rev, msg).unwrap());
        }
        let got = t.commits("moai-ab12");
        let hashes: Vec<_> = got.iter().map(|c| (c.hash.as_str(), c.tracker)).collect();
        assert_eq!(
            hashes,
            [("r6", false), ("r5", false), ("r4", true)],
            "newest first, a child or branch name is not the id"
        );
        assert_eq!(t.commits("moai-ab12.xyz")[0].hash, "r2");
    }

    #[test]
    fn the_cache_path_needs_an_absolute_place_and_a_real_uuid() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
        };
        assert_eq!(dir_from(env(&[("XDG_CACHE_HOME", "/x/c"), ("HOME", "/h")])), Some(PathBuf::from("/x/c/moai/svn")));
        assert_eq!(
            dir_from(env(&[("XDG_CACHE_HOME", "rel"), ("HOME", "/h")])),
            Some(PathBuf::from("/h/.cache/moai/svn"))
        );
        assert_eq!(dir_from(env(&[("XDG_CACHE_HOME", "")])), None, "no home: no cache, no failure");
        assert_eq!(dir_from(env(&[("HOME", "relative")])), None);
        assert_eq!(dir_from(env(&[])), None);
        let dir = Path::new("/c");
        assert_eq!(path_in(dir, UUID), Some(dir.join(format!("{UUID}.json"))));
        for bad in [
            "",
            "../x",
            "../../../../etc/passwd-0000-0000-0000-000000000",
            "0f6c1c1e/2a5d-4b8e-9a43-1f0c3d2e5b6a",
            "0F6C1C1E-2A5D-4B8E-9A43-1F0C3D2E5B6",
            "zf6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a",
        ] {
            assert_eq!(path_in(dir, bad), None, "{bad:?}");
        }
        assert!(valid_uuid("0F6C1C1E-2A5D-4B8E-9A43-1F0C3D2E5B6A"));
    }

    fn sample() -> Table {
        let mut t = Table::new(UUID);
        t.through = 12;
        t.checked_at = Some(at(0));
        t.revs.push(rev_of(3, "a 한글 \"quote\" (moai-ab12)").unwrap());
        t.revs.push(rev_of(12, "b\n\nFixes: moai-cd34").unwrap());
        t.forks.insert("branches/x".into(), serde_json::json!({"from": "trunk", "rev": 2}));
        t.extra.insert("later".into(), serde_json::json!([1, 2]));
        t
    }

    #[test]
    fn a_table_reads_back_byte_for_byte_and_keeps_what_it_does_not_know() {
        let t = sample();
        let bytes = t.to_bytes();
        let back = Table::of_bytes(&bytes, UUID).expect("reads");
        assert_eq!(back, t);
        assert_eq!(back.to_bytes(), bytes, "read and written again, the bytes are the same");
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\"forks\":{\"branches/x\"") && text.contains("\"later\":[1,2]"), "{text}");
    }

    #[test]
    fn a_table_that_cannot_be_trusted_is_none() {
        let good = String::from_utf8(sample().to_bytes()).unwrap();
        assert!(Table::of_bytes(good.as_bytes(), UUID).is_some());
        let other = "1f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a";
        assert!(Table::of_bytes(good.as_bytes(), other).is_none(), "another repository");
        for bad in [
            "{".to_owned(),
            String::new(),
            good.replace("\"version\":1", "\"version\":2"),
            good.replace("\"rev\":12", "\"rev\":13"),
            good.replace("\"rev\":12", "\"rev\":\"12\""),
            good.replace("\"rev\":3", "\"rev\":-3"),
            good.replace("\"rev\":3", "\"rev\":0"),
            good.replace("\"rev\":12", "\"rev\":3"),
            good.replace("\"moai-cd34\"", "\"--diff-cmd=x\""),
            good.replace("[\"moai-cd34\"]", "[]"),
            good.replace(&at(0), "yesterday"),
        ] {
            assert!(Table::of_bytes(bad.as_bytes(), UUID).is_none(), "{bad}");
        }
    }

    #[test]
    fn the_file_is_private_atomic_and_a_bad_one_is_rebuilt() {
        let scratch = Scratch::new("svn-cache-file");
        let dir = scratch.join("home/.cache/moai/svn");
        let path = path_in(&dir, UUID).unwrap();
        assert_eq!(read(&path, UUID), Table::new(UUID), "a missing file is a new table");
        let t = sample();
        write(&path, &t).expect("write");
        assert_eq!(read(&path, UUID), t);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600);
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(dir.parent().unwrap()), 0o700);
            // A directory left open is closed again.
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            write(&path, &t).unwrap();
            assert_eq!(mode(&dir), 0o700);
        }
        // A write that died before its rename leaves a temporary file: the table stands as it was.
        std::fs::write(dir.join(format!(".{UUID}.json.1.2.3.tmp")), "{\"version\":1,\"uu").unwrap();
        assert_eq!(read(&path, UUID), t);
        let names = crate::scratch::names(&dir);
        assert_eq!(names.iter().filter(|n| n.ends_with(".json")).count(), 1, "{names:?}");
        // A file that was cut, or of another repository, reads as a new table, and the next write replaces it.
        std::fs::write(&path, &t.to_bytes()[..20]).unwrap();
        assert_eq!(read(&path, UUID), Table::new(UUID));
        assert_eq!(read(&path, "1f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a").through, 0);
        write(&path, &t).unwrap();
        assert_eq!(read(&path, UUID), t);
        // A directory in the file's place is no table and no panic.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(read(&path, UUID), Table::new(UUID));
        assert!(write(&path, &t).is_err(), "the write fails and says so; the caller goes on");
    }

    #[test]
    fn writers_racing_on_one_file_leave_one_whole_table() {
        let scratch = Scratch::new("svn-cache-race");
        let path = path_in(&scratch.join("moai/svn"), UUID).unwrap();
        let tables: Vec<Table> = (1..=8u64)
            .map(|n| {
                let mut t = sample();
                t.through = 100 + n;
                t
            })
            .collect();
        let (all, path) = (&tables, &path);
        std::thread::scope(|s| {
            for t in all {
                s.spawn(move || {
                    for _ in 0..20 {
                        write(path, t).expect("write");
                        let seen = read(path, UUID);
                        assert!(all.contains(&seen), "a reader saw a table nobody wrote: {seen:?}");
                    }
                });
            }
        });
        assert!(tables.contains(&read(path, UUID)));
        let names = crate::scratch::names(path.parent().unwrap());
        assert_eq!(names, [format!("{UUID}.json")], "no temporary file is left behind");
    }

    /// A server of `head` revisions, every third naming `moai-ab12`, that records each range asked.
    fn server(asked: &std::cell::RefCell<Vec<(u64, u64)>>) -> impl FnMut(u64, u64) -> Result<Fetched, Error> + '_ {
        move |from, to| {
            asked.borrow_mut().push((from, to));
            Ok(Fetched::Revs(
                (from..=to).filter(|r| r % 3 == 0).filter_map(|r| rev_of(r, &format!("r{r} moai-ab12"))).collect(),
            ))
        }
    }

    #[test]
    fn a_fill_asks_only_the_revisions_it_has_not_read() {
        let asked = std::cell::RefCell::new(Vec::new());
        let saved = std::cell::RefCell::new(Vec::new());
        let mut t = Table::new(UUID);
        fill(&mut t, 10, 4, &at(0), server(&asked), |t| saved.borrow_mut().push(t.through)).unwrap();
        assert_eq!(*asked.borrow(), [(1, 4), (5, 8), (9, 10)], "from r1 up, a chunk at a time");
        assert_eq!(*saved.borrow(), [4, 8, 10, 10], "each chunk is saved as it lands");
        assert_eq!((t.through, t.complete()), (10, true));
        assert_eq!(t.commits("moai-ab12").len(), 3);
        // r11 lands: only r11 is asked.
        asked.borrow_mut().clear();
        fill(&mut t, 11, 4, &at(61), server(&asked), |_| {}).unwrap();
        assert_eq!(*asked.borrow(), [(11, 11)]);
        // Nothing new: no log at all, yet the check time moves.
        asked.borrow_mut().clear();
        fill(&mut t, 11, 4, &at(200), server(&asked), |_| {}).unwrap();
        assert!(asked.borrow().is_empty());
        assert_eq!(t.checked_at.as_deref(), Some(at(200).as_str()));
        // A HEAD below what was read: the repository was replaced — start again.
        asked.borrow_mut().clear();
        fill(&mut t, 5, 4, &at(300), server(&asked), |_| {}).unwrap();
        assert_eq!(*asked.borrow(), [(1, 4), (5, 5)]);
        assert_eq!(t.commits("moai-ab12").iter().map(|c| c.hash.as_str()).collect::<Vec<_>>(), ["r3"]);
    }

    #[test]
    fn a_cut_range_is_asked_again_smaller() {
        let asked = std::cell::RefCell::new(Vec::new());
        let mut t = Table::new(UUID);
        let fetch = |from: u64, to: u64| {
            asked.borrow_mut().push((from, to));
            // r3 alone is too long to read; any range holding it is cut.
            if (from..=to).contains(&3) {
                Ok(Fetched::Cut)
            } else {
                Ok(Fetched::Revs(vec![rev_of(to, "moai-ab12").unwrap()]))
            }
        };
        fill(&mut t, 6, 4, &at(0), fetch, |_| {}).unwrap();
        // (1, 2) reads; (3, 4) is cut and halved to (3, 3), still cut and passed over; the size stays one.
        assert_eq!(*asked.borrow(), [(1, 4), (1, 2), (3, 4), (3, 3), (4, 4), (5, 5), (6, 6)]);
        assert_eq!(t.through, 6);
    }

    #[test]
    fn the_server_is_asked_once_a_minute_and_at_once_when_the_clock_went_back() {
        let mut t = Table::new(UUID);
        assert!(!t.fresh(now(0)), "a table never completed always asks");
        t.checked_at = Some(at(0));
        assert!(t.fresh(now(0)) && t.fresh(now(59)));
        assert!(!t.fresh(now(60)) && !t.fresh(now(61)));
        assert!(!t.fresh(now(-5)), "a check time ahead of now asks at once");
    }

    #[test]
    fn a_stopped_first_fill_is_no_answer_and_the_next_carries_on() {
        let asked = std::cell::RefCell::new(Vec::new());
        let saved = std::cell::RefCell::new(None::<Table>);
        let mut t = Table::new(UUID);
        let mut calls = 0;
        let stop = fill(
            &mut t,
            6,
            2,
            &at(0),
            |from, to| {
                calls += 1;
                if calls > 1 { Err(Error::Timeout) } else { server(&asked)(from, to) }
            },
            |t| *saved.borrow_mut() = Some(t.clone()),
        )
        .unwrap_err();
        let kept = saved.borrow().clone().expect("the first chunk was saved");
        assert_eq!((kept.through, kept.complete()), (2, false));
        assert!(matches!(settle(&t, stop, true), Err(Error::Filling)), "never a partial table");
        // The next call picks up at r3, not r1.
        asked.borrow_mut().clear();
        let mut t = kept;
        fill(&mut t, 6, 2, &at(1), server(&asked), |_| {}).unwrap();
        assert_eq!(*asked.borrow(), [(3, 4), (5, 6)]);
        assert!(t.complete());
    }

    #[test]
    fn a_stop_answers_from_a_complete_table_or_says_offline() {
        let empty = Table::new(UUID);
        let offline = || Error::Offline(Some("svn: E170013: Unable to connect".into()));
        assert!(matches!(settle(&empty, offline(), false), Err(Error::Offline(Some(_)))));
        // A server that does not answer within its time and nothing read: offline, not "filling".
        assert!(matches!(settle(&empty, Error::Offline(None), false), Err(Error::Offline(None))));
        assert!(matches!(settle(&empty, Error::Timeout, false), Err(Error::Timeout)));
        assert!(matches!(settle(&empty, Error::Timeout, true), Err(Error::Timeout)), "out of time before any chunk");
        let mut partial = Table::new(UUID);
        partial.through = 2;
        assert!(
            matches!(settle(&partial, offline(), false), Err(Error::Offline(_))),
            "out of reach mid-fill is offline"
        );
        assert!(
            matches!(settle(&partial, Error::Offline(None), false), Err(Error::Offline(None))),
            "one call too slow"
        );
        assert!(matches!(settle(&partial, Error::Offline(None), true), Err(Error::Filling)), "the read's cap");
        assert!(matches!(settle(&partial, Error::Timeout, true), Err(Error::Filling)));
        // A table complete before answers whatever stopped the refresh.
        let mut done = Table::new(UUID);
        done.through = 4;
        done.checked_at = Some(at(-600));
        for stop in [offline(), Error::Offline(None), Error::Timeout, Error::Failed("x".into())] {
            assert!(settle(&done, stop, true).is_ok());
        }
        // A server timeout mid-fill: the chunks read before it are saved.
        let saved = std::cell::RefCell::new(0);
        let mut t = Table::new(UUID);
        let mut calls = 0;
        let asked = std::cell::RefCell::new(Vec::new());
        let stop = fill(
            &mut t,
            8,
            2,
            &at(0),
            |f, to| {
                calls += 1;
                if calls == 3 { Err(Error::Offline(None)) } else { server(&asked)(f, to) }
            },
            |t| *saved.borrow_mut() = t.through,
        )
        .unwrap_err();
        assert_eq!(*saved.borrow(), 4);
        assert!(matches!(settle(&t, stop, false), Err(Error::Offline(None))));
    }
}
