//! The file window — `SPC g f` (moai-ffk9.rpf).
//!
//! **A covering window, like the wiki and statistics windows** (`tui::wiki`, `tui::stats`). It covers the list and
//! the detail whole and `Esc` gives the screen back as it was: the window never touches the cursor, the filter or
//! the detail. It reads only, nothing of it is kept in the config except the share of its dividing line
//! (`files_width`), and it is read fresh every time it opens.
//!
//! **Which checkout**: inside a project the one the session stands in (`Repo::here`), on the one list (`0`) the
//! project of the row under the cursor — the wiki and statistics windows' rule. The checkout is the jail: nothing
//! above its root stands, and `..`, `Bksp` and `h` stop there.
//!
//! **The loop never touches the disk.** Every directory read, every link read and the one `git` call for the
//! ignored directories run on threads the window starts ([`Worker`]); the loop only sends requests and takes
//! replies with `try_recv`. This machine's `/home` can stall, and a stalled read must leave the cursor moving. A
//! reply carries what it answers (the directory and the generation it was asked under, or the spread it belongs
//! to), so an answer for a directory that was folded, refolded or belongs to a closed window is dropped. Dropping
//! the window drops the request channel and the threads end on their own; nothing waits for them, so a read stuck
//! on the disk does not hold up closing.
//!
//! **Paths are never joined here for I/O.** The window holds checkout-relative keys (`src/tui`), and every read
//! goes through [`crate::files`] with the jail — `files::list`, `files::link_text`.
//!
//! **Rows are kept, not drawn from the tree each frame.** The flattened rows are rebuilt when the tree changes (a
//! reply, a fold, a step in or out) and the drawing only walks the rows on screen. The cursor and the open
//! directories are held by path, so rows that appear above the cursor do not move it onto another file.
//!
//! **The hand is the issue list's** — `j`·`k` and the other moves, `l`·`→` open one level and do nothing on an open
//! one, `h`·`←` fold or go to the parent row, `Tab` opens everything below (and folds it all back when open),
//! `Enter` steps into a directory (its first row is `..`) or goes to the right pane on a file, `Bksp` steps out.
//!
//! **`Tab` reads at most [`SPREAD_LIMIT`] new entries**, does not follow links and does not go into the directories
//! git ignores (`target/`, `node_modules/`) — a `Tab` on the root must not spend its budget there and leave the
//! source folded. Those directories still stand as rows, marked, and `l` or `Tab` on one opens it. The ignored list
//! is asked for once when the window opens, in the background; without it (not git, a timeout) nothing is skipped.
//!
//! **The right pane follows the cursor** (moai-ffk9.3fd): on a file it shows the text with line numbers, plain at
//! once and coloured when the highlighter is done, both on a reader thread of its own — a `Tab` spread on the lister
//! must not hold up the file under the cursor. **One read at a time, the newest only**: while one runs the cursor
//! only notes what it wants ([`Right`]); when the read ends the latest want is sent and the rows passed over are
//! never read. The last few files read are kept ([`KEPT`]) and stand at once when the cursor comes back, while the
//! reader checks their stamp and reads again only a file that changed. Long lines do not wrap — the pane scrolls
//! sideways (`H`·`L`, the sideways wheel) and the line numbers stay put.
//!
//! **`/` finds files and hides nothing** (moai-ffk9.rid). It matches the path below the tree's root, case aside
//! ([`files::matches`]), against an index walked once per root on a thread of its own ([`Worker::walk`]) — the ignored
//! directories asked for first and skipped, `.git` and links never entered, at most the read layer's 200,000 entries.
//! The answers stand as the walk comes in. Each file found gets its folders opened, so it stands as a row, its name's
//! matching letters reversed and a [`FOUND`] mark at the head of the row; every other row stays where it was. At most
//! [`FOUND_LIMIT`] are opened — the shallow ones first, so one deep folder cannot crowd out what sits near the root —
//! and the title says how many there are. The folders are read [`SEARCH_READS`] at a time, the one `n` aims at first,
//! so a hand's `l` is not stuck behind five hundred reads. Ending the search folds back what it opened and leaves
//! what the hand opened.

use super::input::Input;
use super::keys::{self, Browse, LEADER, Lookup, PROMPT, Prompt, Side as Toward};
use super::scroll::{self, Move, Scroll};
use super::{App, Mode, Row as ListRow, Seat, menu};
use crate::files::{self, Entry, Jail, Kind, Why};
use crate::i18n::{fill, say};
use crate::report::changes::Checkout;
use ratatui::crossterm::event::KeyEvent;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as Atomic};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// New entries one `Tab` may read before it stops and says so.
pub(super) const SPREAD_LIMIT: usize = 5_000;

/// How long the ignored-directory list may take — past it the window spreads without skipping.
const IGNORED_BUDGET: Duration = Duration::from_secs(10);

/// Replies taken in one step of the loop — a spread sends one per directory, and taking them all at once would
/// rebuild the rows once instead of hundreds of times, but must not hold the loop for thousands either.
const REPLIES_PER_POLL: usize = 256;

/// Files read lately that the right pane keeps — the cursor going back and forth over a few files shows them at once.
pub(super) const KEPT: usize = 8;

/// How long a read may take before the right pane says it is reading — a read that answers within a step of the loop
/// never shows the line, so moving over files does not flicker.
const SAY_READING_AFTER: Duration = Duration::from_millis(60);

/// Columns one notch of the sideways wheel scrolls the right pane.
pub(super) const PAN_WHEEL: usize = 4;

/// Files one search opens to and marks at most — past it the title gives the whole count (moai-ffk9.rid).
pub(super) const FOUND_LIMIT: usize = 500;

/// Folders the search has out on the reader at once — the rest wait their turn, so a folder the hand opens is read
/// after at most this many.
pub(super) const SEARCH_READS: usize = 16;

/// The mark at the head of a found file's row — colour never says it alone.
pub const FOUND: &str = "* ";

type ListFn = dyn Fn(&Jail, &str) -> Result<files::Listed, Why> + Send + Sync;
type IgnoredFn = dyn Fn(&Path) -> Option<Vec<String>> + Send + Sync;
type ReadFn = dyn Fn(&Jail, &str) -> Result<files::Text, Why> + Send + Sync;
type StampFn = dyn Fn(&Jail, &str) -> Option<files::Stamp> + Send + Sync;
type PaintFn = dyn Fn(&str, &str) -> Option<Vec<Vec<crate::highlight::Span>>> + Send + Sync;
type WalkFn =
    dyn Fn(&Jail, &str, &[String], &mut dyn FnMut(&[String]) -> bool) -> Result<files::Walked, Why> + Send + Sync;

/// What the window reads with — the real disk, or a stand-in a test puts in (a slow reader, a failing git).
#[derive(Clone)]
pub struct Disk {
    list: Arc<ListFn>,
    ignored: Arc<IgnoredFn>,
    read: Arc<ReadFn>,
    stamp: Arc<StampFn>,
    paint: Arc<PaintFn>,
    walk: Arc<WalkFn>,
}

impl Default for Disk {
    fn default() -> Disk {
        Disk {
            list: Arc::new(files::list),
            ignored: Arc::new(|root: &Path| crate::vcs::detect(root)?.ignored_dirs(root, Some(IGNORED_BUDGET)).ok()),
            read: Arc::new(files::read),
            stamp: Arc::new(files::stamp),
            paint: Arc::new(crate::highlight::lines),
            walk: Arc::new(|jail: &Jail, dir: &str, skip: &[String], take: &mut dyn FnMut(&[String]) -> bool| {
                files::walk(jail, dir, skip, take)
            }),
        }
    }
}

impl std::fmt::Debug for Disk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Disk")
    }
}

#[cfg(test)]
impl Disk {
    /// The disk with its directory reader and ignored-list answer replaced.
    pub(super) fn with(
        list: impl Fn(&Jail, &str) -> Result<files::Listed, Why> + Send + Sync + 'static,
        ignored: impl Fn(&Path) -> Option<Vec<String>> + Send + Sync + 'static,
    ) -> Disk {
        Disk { list: Arc::new(list), ignored: Arc::new(ignored), ..Disk::default() }
    }

    /// The same disk with its file reader replaced.
    pub(super) fn reading(
        self,
        read: impl Fn(&Jail, &str) -> Result<files::Text, Why> + Send + Sync + 'static,
    ) -> Disk {
        Disk { read: Arc::new(read), ..self }
    }

    /// The same disk with its highlighter replaced — a test holds the colour back to see the plain text first.
    pub(super) fn painting(
        self,
        paint: impl Fn(&str, &str) -> Option<Vec<Vec<crate::highlight::Span>>> + Send + Sync + 'static,
    ) -> Disk {
        Disk { paint: Arc::new(paint), ..self }
    }

    /// The same disk with the search's walk replaced — a test slows it down or counts it.
    pub(super) fn walking(
        self,
        walk: impl Fn(&Jail, &str, &[String], &mut dyn FnMut(&[String]) -> bool) -> Result<files::Walked, Why>
        + Send
        + Sync
        + 'static,
    ) -> Disk {
        Disk { walk: Arc::new(walk), ..self }
    }

    /// The same disk with the ignored list replaced — a test counts how often it is asked.
    pub(super) fn ignoring(self, ignored: impl Fn(&Path) -> Option<Vec<String>> + Send + Sync + 'static) -> Disk {
        Disk { ignored: Arc::new(ignored), ..self }
    }
}

/// The pane holding the focus. The tree is always on the left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    /// The file tree. **The window starts here.**
    #[default]
    Tree,
    /// The right pane — what the cursor stands on.
    Info,
}

/// Why a directory could not be read — kept as the cause, so the row is said in the screen's language when drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    Outside,
    Git,
    Missing,
    NotFile,
    NotDirectory,
    /// A file that is not text (a NUL byte, not UTF-8) — the right pane says so with its size.
    NotText,
    /// The system's own words for an I/O error.
    Io(String),
}

impl Trouble {
    fn of(why: &Why) -> Trouble {
        match why {
            Why::Outside => Trouble::Outside,
            Why::Git => Trouble::Git,
            Why::Missing => Trouble::Missing,
            Why::NotFile => Trouble::NotFile,
            Why::NotText => Trouble::NotText,
            Why::NotDirectory => Trouble::NotDirectory,
            Why::Io(e) => Trouble::Io(crate::text::one_line(&e.to_string())),
        }
    }

    /// The reason in words.
    pub fn said(&self, lang: crate::i18n::Lang) -> String {
        match self {
            Trouble::Outside => say(lang, "tui.files.why.outside").to_string(),
            Trouble::Git => say(lang, "tui.files.why.git").to_string(),
            Trouble::Missing => say(lang, "tui.files.why.missing").to_string(),
            Trouble::NotFile => say(lang, "tui.files.why.not_file").to_string(),
            Trouble::NotDirectory => say(lang, "tui.files.why.not_directory").to_string(),
            Trouble::NotText => say(lang, "tui.files.why.not_text").to_string(),
            Trouble::Io(d) => format!("{} — {d}", say(lang, "tui.files.why.io")),
        }
    }
}

/// One directory's answer.
#[derive(Debug, Clone)]
enum Read {
    /// Its entries in the read layer's order (directories first, then by name), the stamp it was read under —
    /// kept for the refresh member to compare (moai-ffk9.15v) — and the text of each link in it, by name.
    Listed {
        entries: Vec<Entry>,
        stamp: files::Stamp,
        links: BTreeMap<String, String>,
    },
    Failed(Trouble),
}

/// An open directory. `read` is `None` while its answer is out.
#[derive(Debug, Clone)]
struct Dir {
    generation: u64,
    read: Option<Read>,
}

enum Request {
    List {
        path: String,
        generation: u64,
    },
    /// Read `root` and every directory below it, breadth first, until [`SPREAD_LIMIT`] entries were read — not
    /// following links, not entering `skip`.
    Spread {
        id: u64,
        root: String,
        skip: HashSet<String>,
    },
}

enum Reply {
    Listed {
        path: String,
        generation: u64,
        read: Read,
    },
    Spread {
        id: u64,
        path: String,
        read: Read,
    },
    SpreadDone {
        id: u64,
        read: usize,
        stopped: bool,
    },
    /// The reader's first answer for a file. A text is followed by [`Reply::Painted`]; the other answers end the read.
    Opened {
        path: String,
        got: Got,
    },
    /// The colouring of the text just sent is in ([`Content::paint`]) — the read is over.
    Painted {
        path: String,
    },
    /// The search index's ignored list came — or git gave none and nothing is skipped (`known` false).
    Ignoring {
        id: u64,
        known: bool,
    },
    /// One batch of the search index's walk.
    Walked {
        id: u64,
        paths: Vec<Indexed>,
    },
    /// The walk ended: at the read layer's limit (`truncated`, after `seen` entries), or it could not start.
    WalkDone {
        id: u64,
        seen: usize,
        truncated: bool,
        failed: Option<Trouble>,
    },
}

/// One path of the search index, folded on the walker so the loop only compares.
#[derive(Debug, Clone)]
struct Indexed {
    /// Checkout-relative — a row's identity.
    path: String,
    /// The path below the index's root, lower-cased the way [`files::matches`] folds it — a char at a time.
    folded: String,
    /// Folders between the root and it — the found are ordered shallow first.
    depth: u16,
}

impl Indexed {
    fn of(root: &str, path: &str) -> Indexed {
        let rel = below(root, path);
        Indexed {
            path: path.to_owned(),
            folded: rel.chars().flat_map(char::to_lowercase).collect(),
            depth: u16::try_from(rel.matches('/').count()).unwrap_or(u16::MAX),
        }
    }
}

/// A path as it reads below `root` — the whole of it at the checkout's root.
fn below<'a>(root: &str, path: &'a str) -> &'a str {
    if root == "." { path } else { path.strip_prefix(root).and_then(|t| t.strip_prefix('/')).unwrap_or(path) }
}

/// Whether `path` lies strictly inside the folder `dir`.
fn inside(path: &str, dir: &str) -> bool {
    dir == "." && path != "." || path.strip_prefix(dir).is_some_and(|t| t.starts_with('/'))
}

/// A file to read — with the stamp of the copy the window keeps, if any: the reader then only checks it.
#[derive(Debug, Clone)]
struct Look {
    path: String,
    known: Option<files::Stamp>,
}

enum Got {
    /// The kept copy is still the file.
    Same,
    Text(Arc<Content>),
    Failed(Trouble),
}

/// A file's text as the right pane draws it — read, split and measured on the reader thread, so the loop only cuts
/// the lines on screen.
#[derive(Debug)]
pub struct Content {
    /// Checkout-relative path — the identity, with the stamp.
    pub path: String,
    pub stamp: files::Stamp,
    pub text: String,
    /// Each line's bytes in `text`, without its `\n` or a `\r` before it — the highlighter's lines, in its order.
    pub lines: Vec<Range<usize>>,
    /// Each line's width in screen cells, tabs spread ([`super::columns::width`]).
    pub widths: Vec<u32>,
    /// The widest line — the sideways scroll stops where it ends.
    pub widest: usize,
    /// The whole file's size; past the read layer's limit only its head was read (`truncated`).
    pub size: u64,
    pub truncated: bool,
    /// The colouring, set once by the reader after the plain text stood. `Some(None)`: the highlighter has none for
    /// this file (unknown kind, past its limit) and the text stays plain.
    pub paint: OnceLock<Option<Vec<Vec<crate::highlight::Span>>>>,
}

impl Content {
    fn of(path: String, t: files::Text) -> Content {
        let mut lines = Vec::new();
        let mut at = 0;
        for piece in t.text.split_inclusive('\n') {
            let body = piece.strip_suffix('\n').unwrap_or(piece);
            let body = body.strip_suffix('\r').unwrap_or(body);
            lines.push(at..at + body.len());
            at += piece.len();
        }
        let widths: Vec<u32> = lines
            .iter()
            .map(|r| u32::try_from(super::columns::width(&t.text[r.clone()])).unwrap_or(u32::MAX))
            .collect();
        let widest = widths.iter().copied().max().unwrap_or(0) as usize;
        Content {
            path,
            stamp: t.stamp,
            text: t.text,
            lines,
            widths,
            widest,
            size: t.size,
            truncated: t.truncated,
            paint: OnceLock::new(),
        }
    }

    /// One line's text.
    pub fn line(&self, i: usize) -> &str {
        &self.text[self.lines[i].clone()]
    }

    /// One line's colouring, once it came.
    pub fn paint(&self, i: usize) -> &[crate::highlight::Span] {
        self.paint.get().and_then(Option::as_ref).and_then(|p| p.get(i)).map_or(&[], Vec::as_slice)
    }
}

/// Read one file on the reader thread: check a kept copy's stamp, else read it, send the plain text, colour it and
/// say so. Gives `false` once the window is gone.
fn look(disk: &Disk, jail: Option<&Jail>, l: Look, out: &Sender<Reply>) -> bool {
    let Look { path, known } = l;
    let Some(jail) = jail else {
        return out.send(Reply::Opened { path, got: Got::Failed(Trouble::Missing) }).is_ok();
    };
    if let Some(known) = known
        && (disk.stamp)(jail, &path).as_ref() == Some(&known)
    {
        return out.send(Reply::Opened { path, got: Got::Same }).is_ok();
    }
    match (disk.read)(jail, &path) {
        Ok(t) => {
            let c = Arc::new(Content::of(path.clone(), t));
            if out.send(Reply::Opened { path: path.clone(), got: Got::Text(c.clone()) }).is_err() {
                return false;
            }
            let _ = c.paint.set((disk.paint)(&path, &c.text));
            out.send(Reply::Painted { path }).is_ok()
        }
        Err(why) => out.send(Reply::Opened { path, got: Got::Failed(Trouble::of(&why)) }).is_ok(),
    }
}

/// The threads behind one window: one reading directories in the order asked, and one asking git once for the
/// ignored directories.
struct Worker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    lister: Option<std::thread::JoinHandle<()>>,
    ignored: Option<(Receiver<Option<Vec<String>>>, std::thread::JoinHandle<()>)>,
    /// The file reader — its own thread, so a spread on the lister does not hold up the file under the cursor.
    looks: Sender<Look>,
    reader: Option<std::thread::JoinHandle<()>>,
    /// The search index's walk — a thread per index, so a walk of 200,000 entries holds up neither the lister (the
    /// found files' folders are read there) nor the reader. Its stop flag ends it between batches.
    walker: Option<std::thread::JoinHandle<()>>,
    stop: Option<Arc<AtomicBool>>,
    /// What a walker is started with: where replies go, the disk and the checkout.
    outgoing: Sender<Reply>,
    disk: Disk,
    checkout: Checkout,
}

impl Drop for Worker {
    /// The window is gone — a walk still running stops at its next batch.
    fn drop(&mut self) {
        self.stop_walk();
    }
}

/// A worker thread died: raise its panic on the loop, as the reread and commit threads do (`App::follow_commits`)
/// — the panic hook has already put the terminal back, and drawing on would draw on a dead terminal.
fn rethrow(handle: std::thread::JoinHandle<()>) {
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn child(dir: &str, name: &str) -> String {
    if dir == "." { name.to_owned() } else { format!("{dir}/{name}") }
}

fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(p, _)| p)
}

/// The folders strictly between the tree's root `here` and `path`, nearest the root first.
fn ancestors<'a>(here: &str, path: &'a str) -> impl Iterator<Item = &'a str> {
    let here = here.to_owned();
    path.match_indices('/').map(move |(i, _)| &path[..i]).filter(move |d| inside(d, &here))
}

/// The answer to one read, with the text of each link in it.
fn read_dir(list: &ListFn, jail: Option<&Jail>, path: &str) -> Read {
    let Some(jail) = jail else { return Read::Failed(Trouble::of(&Why::Missing)) };
    match list(jail, path) {
        Ok(listed) => {
            let links = listed
                .entries
                .iter()
                .filter(|e| matches!(e.kind, Kind::Link { .. }) && e.openable)
                .filter_map(|e| files::link_text(jail, &child(path, &e.name)).ok().map(|t| (e.name.clone(), t)))
                .collect();
            Read::Listed { entries: listed.entries, stamp: listed.stamp, links }
        }
        Err(why) => Read::Failed(Trouble::of(&why)),
    }
}

/// Whether a spread goes into this entry — a real directory with a name the jail can open. Links stay folded.
fn spreads_into(e: &Entry) -> bool {
    e.openable && matches!(e.kind, Kind::Dir)
}

impl Worker {
    fn start(checkout: &Checkout, disk: &Disk) -> Worker {
        let (requests, incoming) = mpsc::channel::<Request>();
        let (outgoing, replies) = mpsc::channel();
        let walking = outgoing.clone();
        let (looks, asked) = mpsc::channel::<Look>();
        let (reading, out, at) = (disk.clone(), outgoing.clone(), checkout.clone());
        let reader = std::thread::spawn(move || {
            let jail = Jail::of(&at);
            while let Ok(l) = asked.recv() {
                if !look(&reading, jail.as_ref(), l, &out) {
                    return;
                }
            }
        });
        let list = disk.list.clone();
        let at = checkout.clone();
        let lister = std::thread::spawn(move || {
            let jail = Jail::of(&at);
            while let Ok(request) = incoming.recv() {
                match request {
                    Request::List { path, generation } => {
                        let read = read_dir(&*list, jail.as_ref(), &path);
                        if outgoing.send(Reply::Listed { path, generation, read }).is_err() {
                            return;
                        }
                    }
                    Request::Spread { id, root, skip } => {
                        let mut pending = VecDeque::from([root]);
                        let mut read = 0usize;
                        let mut stopped = false;
                        while let Some(path) = pending.pop_front() {
                            if read >= SPREAD_LIMIT {
                                stopped = true;
                                break;
                            }
                            let answer = read_dir(&*list, jail.as_ref(), &path);
                            if let Read::Listed { entries, .. } = &answer {
                                read += entries.len();
                                pending.extend(
                                    entries
                                        .iter()
                                        .filter(|e| spreads_into(e))
                                        .map(|e| child(&path, &e.name))
                                        .filter(|p| !skip.contains(p)),
                                );
                            }
                            if outgoing.send(Reply::Spread { id, path, read: answer }).is_err() {
                                return;
                            }
                        }
                        if outgoing.send(Reply::SpreadDone { id, read, stopped }).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        let (told, heard) = mpsc::channel();
        let ask = disk.ignored.clone();
        let root = checkout.path.clone();
        let ignored = std::thread::spawn(move || {
            let _ = told.send(ask(&root));
        });
        Worker {
            requests,
            replies,
            lister: Some(lister),
            ignored: Some((heard, ignored)),
            looks,
            reader: Some(reader),
            walker: None,
            stop: None,
            outgoing: walking,
            disk: disk.clone(),
            checkout: checkout.clone(),
        }
    }

    /// Stop the walk running, if any — its later replies carry an old id and are dropped.
    fn stop_walk(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Atomic::Relaxed);
        }
    }

    /// Walk `root` for the search index `id`: ask for the ignored list (once per index, never per key), then walk
    /// skipping it, sending the paths in batches. The walk before is told to stop.
    fn walk(&mut self, id: u64, root: String) {
        self.stop_walk();
        // A walker that already ended may have died — raise it before its handle is replaced.
        if let Some(h) = self.walker.take_if(|h| h.is_finished()) {
            rethrow(h);
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = Some(stop.clone());
        let (disk, at, out) = (self.disk.clone(), self.checkout.clone(), self.outgoing.clone());
        self.walker = Some(std::thread::spawn(move || {
            let Some(jail) = Jail::of(&at) else {
                let failed = Some(Trouble::Missing);
                let _ = out.send(Reply::WalkDone { id, seen: 0, truncated: false, failed });
                return;
            };
            let skip = (disk.ignored)(&at.path);
            if stop.load(Atomic::Relaxed) || out.send(Reply::Ignoring { id, known: skip.is_some() }).is_err() {
                return;
            }
            let skip = skip.unwrap_or_default();
            let mut take = |batch: &[String]| {
                !stop.load(Atomic::Relaxed)
                    && out
                        .send(Reply::Walked { id, paths: batch.iter().map(|p| Indexed::of(&root, p)).collect() })
                        .is_ok()
            };
            let done = match (disk.walk)(&jail, &root, &skip, &mut take) {
                Ok(w) => Reply::WalkDone { id, seen: w.seen, truncated: w.truncated, failed: None },
                Err(why) => Reply::WalkDone { id, seen: 0, truncated: false, failed: Some(Trouble::of(&why)) },
            };
            let _ = out.send(done);
        }));
    }

    fn look(&self, l: Look) {
        // A failed send means the reader died; the next `reply` finds it finished and raises its panic.
        let _ = self.looks.send(l);
    }

    fn send(&self, r: Request) {
        // A failed send means the reader died; the next `reply` finds it finished and raises its panic.
        let _ = self.requests.send(r);
    }

    fn reply(&mut self) -> Option<Reply> {
        // The lister and the reader share the reply channel, so one's death shows as a finished thread, not a
        // disconnect. Neither ends on its own while the window stands — only by a panic, raised here. A walker does end
        // on its own; joining it then raises nothing.
        for slot in [&mut self.lister, &mut self.reader, &mut self.walker] {
            if slot.as_ref().is_some_and(std::thread::JoinHandle::is_finished)
                && let Some(h) = slot.take()
            {
                rethrow(h);
            }
        }
        self.replies.try_recv().ok()
    }

    /// The ignored list once it came — `Some(None)` when git could not give it.
    fn ignored(&mut self) -> Option<Option<Vec<String>>> {
        let (rx, _) = self.ignored.as_ref()?;
        let got = match rx.try_recv() {
            Ok(got) => got,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => None,
        };
        if let Some((_, h)) = self.ignored.take() {
            rethrow(h);
        }
        Some(got)
    }
}

/// The worker as the window holds it. **Shared, not copied**: [`Mode`] is cloned when a window goes behind the
/// suggestion list (`App::run`'s `Proposals`), and the copy that comes back must still hear its own replies.
#[derive(Clone)]
struct Shared(Arc<Mutex<Worker>>);

impl Shared {
    fn with<T>(&self, f: impl FnOnce(&mut Worker) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

impl PartialEq for Shared {
    fn eq(&self, other: &Shared) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The ignored directories, as git named them (without the trailing `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ignored {
    Waiting,
    Known(HashSet<String>),
    /// Git gave nothing — nothing is skipped or marked.
    Unknown,
}

/// A `Tab` in progress: its id (a reply of another spread is dropped), the directory it started on, and whether
/// it is still waiting on the ignored list before it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spread {
    id: u64,
    root: String,
    sent: bool,
    /// Directories the person folded while it ran — its later answers for them and below stay out.
    folded: Vec<String>,
}

/// A drawn row.
#[derive(Debug, Clone)]
pub enum Row {
    /// `..` — one level up. Stands only below the checkout's root.
    Up,
    Entry {
        /// Checkout-relative path — the row's identity.
        path: String,
        depth: usize,
        entry: Entry,
        /// An opened directory.
        open: bool,
        /// Git ignores it or a directory above it.
        ignored: bool,
        /// What a link holds, when it could be read.
        link: Option<String>,
    },
    /// The directory `of` is still being read.
    Loading { depth: usize, of: String },
    /// The directory `of` could not be read.
    Failed { depth: usize, of: String, why: Trouble },
}

/// Which row the cursor stands on, by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
enum At {
    Up,
    Path(String),
    /// The loading or failure row under that directory.
    Note(String),
}

impl Row {
    fn at(&self) -> At {
        match self {
            Row::Up => At::Up,
            Row::Entry { path, .. } => At::Path(path.clone()),
            Row::Loading { of, .. } | Row::Failed { of, .. } => At::Note(of.clone()),
        }
    }

    fn depth(&self) -> usize {
        match self {
            Row::Up => 0,
            Row::Entry { depth, .. } | Row::Loading { depth, .. } | Row::Failed { depth, .. } => *depth,
        }
    }
}

/// Whether the row's entry can be opened as a directory by hand — a directory, or a link to one inside the
/// checkout.
pub fn opens(e: &Entry) -> bool {
    e.openable && matches!(e.kind, Kind::Dir | Kind::Link { dir: true, inside: true })
}

/// What the right pane shows of a file.
#[derive(Debug, Clone)]
pub enum Shown {
    /// The cursor is not on a file — the pane tells of the row instead.
    Nothing,
    /// Its read is out. `said` once it took long enough to say so ([`SAY_READING_AFTER`]).
    Reading {
        path: String,
        since: Instant,
        said: bool,
    },
    Text(Arc<Content>),
    Failed {
        path: String,
        why: Trouble,
    },
}

impl Shown {
    fn path(&self) -> Option<&str> {
        match self {
            Shown::Nothing => None,
            Shown::Reading { path, .. } | Shown::Failed { path, .. } => Some(path),
            Shown::Text(c) => Some(&c.path),
        }
    }
}

/// The right pane's state: what it shows, where it is scrolled, the read out and the one wanted next, the files kept.
#[derive(Debug, Clone)]
pub struct Right {
    pub shown: Shown,
    /// The vertical scroll of the text.
    pub page: Scroll,
    /// The sideways scroll in screen cells — the first text column shown.
    pub pan: usize,
    /// The text columns of the last frame — `H`·`L` go half of it, and the end is measured with it.
    pub(super) room: usize,
    /// The read out — one at a time.
    running: Option<String>,
    /// What the cursor wants read after it, the newest only.
    wanted: Option<Look>,
    /// Lately read texts, the newest first, at most [`KEPT`].
    kept: VecDeque<Arc<Content>>,
}

impl Default for Right {
    fn default() -> Right {
        Right {
            shown: Shown::Nothing,
            page: Scroll::default(),
            pan: 0,
            room: 0,
            running: None,
            wanted: None,
            kept: VecDeque::new(),
        }
    }
}

impl Right {
    /// The furthest the text scrolls sideways — the widest line's end at the pane's right edge.
    fn pan_end(&self) -> usize {
        match &self.shown {
            Shown::Text(c) => c.widest.saturating_sub(self.room),
            _ => 0,
        }
    }

    /// Scroll sideways by `by` cells, within the text.
    pub(super) fn pan_by(&mut self, by: isize) {
        self.pan = self.pan.saturating_add_signed(by).min(self.pan_end());
    }

    fn keep(&mut self, c: Arc<Content>) {
        self.kept.retain(|k| k.path != c.path);
        self.kept.push_front(c);
        self.kept.truncate(KEPT);
    }

    /// The kept copy of a file, made the newest.
    fn kept(&mut self, path: &str) -> Option<Arc<Content>> {
        let at = self.kept.iter().position(|k| k.path == path)?;
        let c = self.kept.remove(at)?;
        self.kept.push_front(c.clone());
        Some(c)
    }

    /// Whether a read is out or due — the loop wakes fast to take it.
    fn busy(&self) -> bool {
        self.running.is_some() || matches!(self.shown, Shown::Reading { said: false, .. })
    }
}

/// Whether the right pane reads this entry as a file — a regular file, or a link to one inside the checkout, with a
/// name the jail can open. The rest is told of, not read.
pub fn reads(e: &Entry) -> bool {
    e.openable && matches!(e.kind, Kind::File | Kind::Link { dir: false, inside: true })
}

/// The search index: what one walk of the tree's root gave (moai-ffk9.rid).
#[derive(Debug, Clone)]
struct Index {
    /// The walk it is — replies of another are dropped.
    id: u64,
    /// The folder walked — the tree's root when it began. Another root walks again.
    root: String,
    /// Shared, not copied, when the window is cloned ([`Shared`]'s reason) — it holds up to 200,000 paths.
    entries: Arc<Vec<Indexed>>,
    walking: bool,
    /// Whether git named the ignored directories — `None` while asking. `Some(false)`: nothing is skipped.
    skipping: Option<bool>,
    failed: Option<Trouble>,
}

/// The search: the hung query, the index and what it found, and the folders it opened to show them.
#[derive(Debug, Clone, Default)]
struct Search {
    /// The hung query — trimmed, never blank.
    query: Option<String>,
    index: Option<Index>,
    /// What stands opened and marked, by (depth below the root, path) — shallow first, at most [`FOUND_LIMIT`].
    found: BTreeSet<(u16, String)>,
    /// All the index matched — more than `found` holds past the limit.
    total: usize,
    /// Files the index named that their folder, once read, did not hold — left out without a word.
    gone: HashSet<String>,
    /// Folders the search opened — ending it folds them back. One the hand folds leaves this set and is the hand's.
    opened: BTreeSet<String>,
    /// Found files whose folders were already queued — each file is aimed once per query.
    aimed: HashSet<String>,
    /// Folders waiting to be read, at most [`SEARCH_READS`] out at once.
    queue: VecDeque<String>,
    /// The cursor goes to the first file found once its row stands — set when the query changes, dropped when the hand
    /// moves the cursor.
    jump: bool,
}

/// The search field being typed in (`/`) — the wiki window's `Typing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typing {
    pub input: Input,
    /// The search hung before the field opened — `Esc` goes back to it.
    was: Option<String>,
    /// The row the cursor stood on — `Esc` puts it back there.
    from: At,
}

/// Keep `key` among the found when it is one of the [`FOUND_LIMIT`] first.
fn admit(found: &mut BTreeSet<(u16, String)>, depth: u16, path: &str) {
    if found.len() >= FOUND_LIMIT {
        match found.last() {
            Some((d, p)) if (depth, path) < (*d, p.as_str()) => {}
            _ => return,
        }
        found.pop_last();
    }
    found.insert((depth, path.to_owned()));
}

/// What one step of the loop brought.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Polled {
    /// Something on screen changed.
    pub changed: bool,
    /// A `Tab` stopped at its limit after reading this many entries.
    pub stopped: Option<usize>,
    /// The right pane changed — no row did.
    pub painted: bool,
    /// The search index stopped at the read layer's limit after this many entries.
    pub indexed: Option<usize>,
}

/// The window's state.
#[derive(Clone)]
pub struct Window {
    /// Project name — the title says it.
    pub project: String,
    /// The checkout shown, and the jail.
    pub checkout: Checkout,
    /// The running key tables, handed over when the window opened (moai-bzm3).
    pub keys: keys::Keymap,
    worker: Shared,
    /// Open directories by checkout-relative path; `"."` is the checkout's root and always open.
    dirs: BTreeMap<String, Dir>,
    generation: u64,
    /// The directory the tree stands in — `"."` is the checkout's root.
    pub here: String,
    at: At,
    /// Index of the cursor's row in [`Window::rows`] — derived from the identity on each rebuild.
    pub cursor: usize,
    /// The tree's scroll.
    pub list: Scroll,
    /// The tree's screen was scrolled away from the cursor (the wheel, half and whole pages) — the drawing does not
    /// pull it back to the cursor until a cursor key does (`wiki::Window::adrift`, moai-og9h).
    pub(super) adrift: bool,
    pub focus: Side,
    /// A pending prefix (`gg`, `Ctrl-w`) — the window's own, apart from the browse one.
    pub chord: keys::Chord,
    rows: Vec<Row>,
    ignored: Ignored,
    spread: Option<Spread>,
    /// Directories stepped into that were folded in their parent — stepping out folds them back, so the parent
    /// looks as it did (the list's `Enter` does not open the row either).
    stepped: Vec<String>,
    /// The right pane (moai-ffk9.3fd).
    pub right: Right,
    /// The `/` search (moai-ffk9.rid).
    search: Search,
    /// The search field, while it is typed in.
    pub typing: Option<Typing>,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("files::Window")
            .field("project", &self.project)
            .field("here", &self.here)
            .field("at", &self.at)
            .field("rows", &self.rows.len())
            .finish()
    }
}

impl PartialEq for Window {
    /// The same window — the same worker, standing in the same place.
    fn eq(&self, other: &Window) -> bool {
        self.worker == other.worker && self.here == other.here && self.at == other.at && self.focus == other.focus
    }
}

impl Eq for Window {}

impl Window {
    pub(super) fn new(project: String, checkout: Checkout, keys: keys::Keymap, disk: &Disk) -> Window {
        let worker = Shared(Arc::new(Mutex::new(Worker::start(&checkout, disk))));
        let mut w = Window {
            project,
            checkout,
            keys,
            worker,
            dirs: BTreeMap::new(),
            generation: 0,
            here: ".".into(),
            at: At::Up,
            cursor: 0,
            list: Scroll::default(),
            adrift: false,
            focus: Side::Tree,
            chord: keys::Chord::default(),
            rows: Vec::new(),
            ignored: Ignored::Waiting,
            spread: None,
            stepped: Vec::new(),
            right: Right::default(),
            search: Search::default(),
            typing: None,
        };
        w.open(".");
        w.rebuild();
        w
    }

    /// The rows as drawn.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The row under the cursor.
    pub fn current(&self) -> Option<&Row> {
        self.rows.get(self.cursor)
    }

    /// The path under the cursor, if it stands on an entry.
    pub fn current_path(&self) -> Option<&str> {
        match self.current()? {
            Row::Entry { path, .. } => Some(path),
            _ => None,
        }
    }

    /// The number of entries read for an open directory — the right pane says it.
    pub fn count(&self, path: &str) -> Option<usize> {
        match self.dirs.get(path)?.read.as_ref()? {
            Read::Listed { entries, .. } => Some(entries.len()),
            Read::Failed(_) => None,
        }
    }

    /// The stamp a directory was read under — the refresh member compares it (moai-ffk9.15v).
    #[allow(dead_code)] // moai-ffk9.15v reads it.
    pub fn stamp(&self, path: &str) -> Option<&files::Stamp> {
        match self.dirs.get(path)?.read.as_ref()? {
            Read::Listed { stamp, .. } => Some(stamp),
            Read::Failed(_) => None,
        }
    }

    pub fn is_open(&self, path: &str) -> bool {
        self.dirs.contains_key(path)
    }

    /// Something is still out: a directory read, the ignored list, a spread.
    pub fn waiting(&self) -> bool {
        self.spread.is_some()
            || self.search.index.as_ref().is_some_and(|i| i.walking)
            || self.ignored == Ignored::Waiting
            || self.dirs.values().any(|d| d.read.is_none())
            || self.right.busy()
    }

    /// Bring the right pane to the row under the cursor — called wherever the cursor lands ([`Window::move_to`],
    /// [`Window::rebuild`]). The same file stays as it is, scroll and all; another file stands at its top and left
    /// edge, a kept copy at once, and is asked for.
    fn aim(&mut self) {
        let path = match self.current() {
            Some(Row::Entry { path, entry, .. }) if reads(entry) => path.clone(),
            _ => {
                self.right.wanted = None;
                if !matches!(self.right.shown, Shown::Nothing) {
                    self.right.shown = Shown::Nothing;
                    self.right.page.rewind();
                    self.right.pan = 0;
                }
                return;
            }
        };
        if self.right.shown.path() == Some(path.as_str()) {
            return;
        }
        self.right.page.rewind();
        self.right.pan = 0;
        let kept = self.right.kept(&path);
        let known = kept.as_ref().map(|c| c.stamp.clone());
        self.right.shown = match kept {
            Some(c) => Shown::Text(c),
            None => Shown::Reading { path: path.clone(), since: Instant::now(), said: false },
        };
        // The file being read now needs no second read — its answer lands on the pane.
        self.right.wanted = (self.right.running.as_deref() != Some(path.as_str())).then_some(Look { path, known });
        self.send_look();
    }

    /// Send the wanted read when none is out.
    fn send_look(&mut self) {
        if self.right.running.is_some() {
            return;
        }
        let Some(l) = self.right.wanted.take() else { return };
        self.right.running = Some(l.path.clone());
        self.worker.with(|w| w.look(l));
    }

    /// A read ended: the next wanted goes out.
    fn read_done(&mut self, path: &str) {
        if self.right.running.as_deref() == Some(path) {
            self.right.running = None;
        }
        self.send_look();
    }

    fn next_generation(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// Open a directory and ask for it. An open one is left as it is.
    fn open(&mut self, path: &str) {
        if self.dirs.contains_key(path) {
            return;
        }
        self.ask(path);
    }

    /// Ask for a directory afresh — a reply under an older generation is dropped.
    fn ask(&mut self, path: &str) {
        let generation = self.next_generation();
        self.dirs.insert(path.to_owned(), Dir { generation, read: None });
        let path = path.to_owned();
        self.worker.with(|w| w.send(Request::List { path, generation }));
    }

    /// Fold a directory and everything below it — a later open reads it again. The root and the directories
    /// the tree stands in stay open.
    fn close(&mut self, path: &str) {
        let holds = |p: &str| p == path || p.strip_prefix(path).is_some_and(|t| t.starts_with('/'));
        if path == "." || holds(&self.here) {
            return;
        }
        self.dirs.retain(|p, _| !holds(p));
        // What the search opened in there is folded — opened again, by the hand, it is the hand's.
        self.search.opened.retain(|p| !holds(p));
        self.search.queue.retain(|p| !holds(p));
        // A spread whose root went is over; one still running keeps out of what was folded under it.
        match &mut self.spread {
            Some(s) if holds(&s.root) => self.spread = None,
            Some(s) => s.folded.push(path.to_owned()),
            None => {}
        }
    }

    fn ignored_path(&self, path: &str) -> bool {
        matches!(&self.ignored, Ignored::Known(set) if set.contains(path))
    }

    /// Rebuild the rows from the tree and put the cursor back on the row it stood on. When that row went, the
    /// cursor keeps its index — the next row — and takes that row's identity.
    fn rebuild(&mut self) {
        let mut rows = Vec::new();
        if self.here != "." {
            rows.push(Row::Up);
        }
        let above = self.here.split('/').scan(String::new(), |acc, part| {
            *acc = if acc.is_empty() { part.to_owned() } else { format!("{acc}/{part}") };
            Some(acc.clone())
        });
        let ignored = self.here != "." && above.into_iter().any(|p| self.ignored_path(&p));
        self.flatten(&self.here.clone(), 0, ignored, &mut rows);
        self.rows = rows;
        if let Some(path) = self.jump_target() {
            self.search.jump = false;
            self.at = At::Path(path);
        }
        match self.rows.iter().position(|r| r.at() == self.at) {
            Some(i) => self.cursor = i,
            None => {
                self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
                if let Some(r) = self.rows.get(self.cursor) {
                    self.at = r.at();
                }
            }
        }
        self.aim();
    }

    fn flatten(&self, dir: &str, depth: usize, ignored: bool, out: &mut Vec<Row>) {
        match self.dirs.get(dir).and_then(|d| d.read.as_ref()) {
            None => out.push(Row::Loading { depth, of: dir.to_owned() }),
            Some(Read::Failed(why)) => out.push(Row::Failed { depth, of: dir.to_owned(), why: why.clone() }),
            Some(Read::Listed { entries, links, .. }) => {
                for e in entries {
                    let path = child(dir, &e.name);
                    let ignored = ignored || self.ignored_path(&path);
                    let open = opens(e) && self.is_open(&path);
                    out.push(Row::Entry {
                        path: path.clone(),
                        depth,
                        entry: e.clone(),
                        open,
                        ignored,
                        link: links.get(&e.name).cloned(),
                    });
                    if open {
                        self.flatten(&path, depth + 1, ignored, out);
                    }
                }
            }
        }
    }

    /// Put the cursor on a row and bring the tree back to it.
    pub(super) fn move_to(&mut self, at: usize) {
        self.adrift = false;
        self.search.jump = false;
        self.cursor = at.min(self.rows.len().saturating_sub(1));
        if let Some(r) = self.rows.get(self.cursor) {
            self.at = r.at();
        }
        self.aim();
    }

    /// A move for the focused pane — the cursor on the tree; half and whole pages scroll the tree's screen and
    /// leave the cursor (the explorer's list, moai-og9h). On the right pane every move scrolls the text.
    pub fn step(&mut self, m: Move) {
        match self.focus {
            Side::Tree if matches!(m, Move::HalfUp | Move::HalfDown | Move::PageUp | Move::PageDown) => {
                let was = self.list.offset();
                self.list.go(m);
                self.adrift |= self.list.offset() != was;
            }
            Side::Tree => {
                let len = self.rows.len();
                self.move_to(scroll::cursor(m, self.cursor, || len));
            }
            Side::Info => self.right.page.go(m),
        }
    }

    /// `H`·`L` — scroll the text sideways by half the pane.
    pub fn pan(&mut self, toward: Toward) {
        let half = (self.right.room / 2).max(1) as isize;
        self.right.pan_by(if toward == Toward::Left { -half } else { half });
    }

    /// One wheel step over a pane — the pane under the pointer, not the focus. A pending prefix is dropped.
    pub(super) fn roll(&mut self, on: Side, by: isize) {
        self.chord.clear();
        if on == Side::Tree && by != 0 {
            let was = self.list.offset();
            self.list.by(by);
            self.adrift |= self.list.offset() != was;
        }
        if on == Side::Info {
            self.right.page.by(by);
        }
    }

    /// `l` — open the directory under the cursor one level. An open one, a file and `..` do nothing.
    pub fn expand(&mut self) {
        if let Some(Row::Entry { path, entry, open: false, .. }) = self.current()
            && opens(entry)
        {
            let path = path.clone();
            self.open(&path);
            self.rebuild();
        }
    }

    /// `h` — fold the open directory under the cursor; on anything else go to its parent row (`..` at the top of a
    /// directory stepped into). At the top of the checkout's root nothing happens.
    pub fn collapse(&mut self) {
        let Some(row) = self.current() else { return };
        if let Row::Entry { path, open: true, .. } = row {
            let path = path.clone();
            self.close(&path);
            return self.rebuild();
        }
        let parent = match row {
            Row::Up => return,
            Row::Entry { path, .. } => parent_of(path).to_owned(),
            Row::Loading { of, .. } | Row::Failed { of, .. } => of.clone(),
        };
        if row.depth() > 0 {
            if let Some(i) = self.rows.iter().position(|r| r.at() == At::Path(parent.clone())) {
                self.move_to(i);
            }
        } else if self.here != "." {
            self.move_to(0);
        }
    }

    /// `Tab` — open everything below the directory under the cursor, or fold it all back when it is open.
    pub fn spread(&mut self) {
        let Some(Row::Entry { path, entry, open, .. }) = self.current() else { return };
        if !opens(entry) {
            return;
        }
        let (path, open) = (path.clone(), *open);
        if open {
            self.close(&path);
            return self.rebuild();
        }
        // A spread still waiting is replaced; its root, asked for by the spread alone, is asked for on its own.
        if let Some(old) = self.spread.take()
            && self.dirs.get(&old.root).is_some_and(|d| d.read.is_none())
        {
            self.ask(&old.root);
        }
        let generation = self.next_generation();
        self.dirs.insert(path.clone(), Dir { generation, read: None });
        self.spread = Some(Spread { id: generation, root: path, sent: false, folded: Vec::new() });
        self.send_spread();
        self.rebuild();
    }

    /// Send the spread once the ignored list is in — it skips what that list names.
    fn send_spread(&mut self) {
        let skip = match &self.ignored {
            Ignored::Waiting => return,
            Ignored::Known(set) => set.clone(),
            Ignored::Unknown => HashSet::new(),
        };
        let Some(s) = self.spread.as_mut().filter(|s| !s.sent) else { return };
        s.sent = true;
        let (id, root) = (s.id, s.root.clone());
        self.worker.with(|w| w.send(Request::Spread { id, root, skip }));
    }

    /// `Enter` — step into the directory under the cursor (its first row is `..`, the cursor on the row after it),
    /// go to the right pane on anything else, step out on `..`.
    pub fn enter(&mut self) {
        let Some(row) = self.current() else { return };
        match row {
            Row::Up => self.leave(),
            Row::Entry { path, entry, .. } if opens(entry) => {
                let path = path.clone();
                if !self.is_open(&path) {
                    self.stepped.push(path.clone());
                    self.open(&path);
                }
                self.here = path;
                self.at = At::Up;
                self.rebuild();
                self.list.rewind();
                self.move_to(1);
                self.reroot();
            }
            Row::Entry { .. } => self.focus = Side::Info,
            _ => {}
        }
    }

    /// `Bksp` — step out one level, the cursor on the directory left. At the checkout's root nothing happens.
    pub fn leave(&mut self) {
        if self.here == "." {
            return;
        }
        let was = std::mem::take(&mut self.here);
        self.here = parent_of(&was).to_owned();
        self.open(&self.here.clone());
        if self.stepped.last() == Some(&was) {
            self.stepped.pop();
            self.close(&was);
        }
        self.at = At::Path(was);
        self.adrift = false;
        self.rebuild();
        self.reroot();
    }

    fn focus_next(&mut self) {
        self.focus = match self.focus {
            Side::Tree => Side::Info,
            Side::Info => Side::Tree,
        };
    }

    /// Take what came back. Rebuilds the rows once, after all of it.
    pub(super) fn poll(&mut self) -> Polled {
        let mut out = Polled::default();
        if let Some(got) = self.worker.with(Worker::ignored) {
            self.ignored = match got {
                Some(dirs) => Ignored::Known(dirs.iter().map(|d| d.trim_end_matches('/').to_owned()).collect()),
                None => Ignored::Unknown,
            };
            self.send_spread();
            out.changed = true;
        }
        for _ in 0..REPLIES_PER_POLL {
            let Some(reply) = self.worker.with(Worker::reply) else { break };
            match reply {
                Reply::Listed { path, generation, read } => {
                    if let Some(d) = self.dirs.get_mut(&path)
                        && d.generation == generation
                    {
                        d.read = Some(read);
                        self.drop_gone(&path);
                        out.changed = true;
                    }
                }
                Reply::Spread { id, path, read } => {
                    let Some(s) = self.spread.as_ref().filter(|s| s.id == id) else { continue };
                    // A directory lands only where its parent still stands open — the person may have folded the
                    // branch since. The root was opened when the spread began.
                    let folded = s
                        .folded
                        .iter()
                        .any(|f| path == *f || path.strip_prefix(f.as_str()).is_some_and(|t| t.starts_with('/')));
                    if folded || path != s.root && !self.dirs.get(parent_of(&path)).is_some_and(|d| d.read.is_some()) {
                        continue;
                    }
                    let generation = self.next_generation();
                    self.dirs.insert(path.clone(), Dir { generation, read: Some(read) });
                    self.drop_gone(&path);
                    out.changed = true;
                }
                Reply::SpreadDone { id, read, stopped } => {
                    if self.spread.as_ref().is_some_and(|s| s.id == id) {
                        self.spread = None;
                        out.changed = true;
                        if stopped {
                            out.stopped = Some(read);
                        }
                    }
                }
                Reply::Opened { path, got } => {
                    let here = self.right.shown.path() == Some(path.as_str());
                    match got {
                        Got::Same => self.read_done(&path),
                        // The text stands; the read goes on to its colour. A late answer for a file the cursor left is
                        // only kept.
                        Got::Text(c) => {
                            self.right.keep(c.clone());
                            if here {
                                self.right.shown = Shown::Text(c);
                                out.painted = true;
                            }
                        }
                        Got::Failed(why) => {
                            self.right.kept.retain(|k| k.path != path);
                            if here {
                                self.right.shown = Shown::Failed { path: path.clone(), why };
                                out.painted = true;
                            }
                            self.read_done(&path);
                        }
                    }
                }
                Reply::Painted { path } => {
                    out.painted |= self.right.shown.path() == Some(path.as_str());
                    self.read_done(&path);
                }
                Reply::Ignoring { id, known } => {
                    if let Some(i) = self.search.index.as_mut().filter(|i| i.id == id) {
                        i.skipping = Some(known);
                        out.changed = true;
                    }
                }
                Reply::Walked { id, paths } => {
                    if self.search.index.as_ref().is_some_and(|i| i.id == id) {
                        self.indexed(paths);
                        out.changed = true;
                    }
                }
                Reply::WalkDone { id, seen, truncated, failed } => {
                    if let Some(i) = self.search.index.as_mut().filter(|i| i.id == id) {
                        i.walking = false;
                        i.failed = failed;
                        out.changed = true;
                        if truncated {
                            out.indexed = Some(seen);
                        }
                    }
                }
            }
        }
        // The answers may have freed a slot for the search's folders.
        if out.changed {
            self.pump();
        }
        if let Shown::Reading { since, said: said @ false, .. } = &mut self.right.shown
            && since.elapsed() >= SAY_READING_AFTER
        {
            *said = true;
            out.painted = true;
        }
        if out.changed {
            self.rebuild();
        }
        out
    }

    // ---- the search (moai-ffk9.rid) ----

    /// The query the tree is marked with — the text being typed, else the hung search. Blank is none.
    pub fn query(&self) -> Option<&str> {
        let q = match &self.typing {
            Some(t) => Some(t.input.text()),
            None => self.search.query.as_deref(),
        };
        q.map(str::trim).filter(|q| !q.is_empty())
    }

    /// A search is hung and not being typed — `Esc` ends it before it closes the window, and `n`·`N` jump.
    pub fn searched(&self) -> bool {
        self.typing.is_none() && self.search.query.is_some()
    }

    /// Files opened and marked, and all the index matched.
    pub fn found_count(&self) -> (usize, usize) {
        (self.search.found.len(), self.search.total)
    }

    /// Entries indexed so far while the walk runs — `None` once it ended or before any search.
    pub fn indexing(&self) -> Option<usize> {
        self.search.index.as_ref().filter(|i| i.walking).map(|i| i.entries.len())
    }

    /// Git gave no ignored list for the index — nothing is skipped, and the title says so.
    pub fn unskipped(&self) -> bool {
        self.search.index.as_ref().is_some_and(|i| i.skipping == Some(false))
    }

    /// Why the index could not be walked.
    pub fn index_failed(&self) -> Option<&Trouble> {
        self.search.index.as_ref().and_then(|i| i.failed.as_ref())
    }

    /// Whether the row of `path` is a file found — and the matching bytes of its name, when the match reaches into the
    /// name (a match only in the folders above marks the row and paints nothing).
    pub fn found_in(&self, path: &str) -> Option<Option<Range<usize>>> {
        let q = self.query()?;
        let rel = below(&self.here, path);
        let depth = u16::try_from(rel.matches('/').count()).unwrap_or(u16::MAX);
        if !self.search.found.contains(&(depth, path.to_owned())) {
            return None;
        }
        let name_at = rel.rfind('/').map_or(0, |i| i + 1);
        let Some(r) = files::matches(q, rel) else { return Some(None) };
        let from = r.start.max(name_at);
        Some((from < r.end).then(|| from - name_at..r.end - name_at))
    }

    /// `/` — open the search field, with the hung search in it to change (the wiki window's way). The first search on
    /// a root walks its index.
    fn start_search(&mut self) {
        let was = self.search.query.clone();
        let input = Input::new(was.as_deref().unwrap_or_default());
        self.typing = Some(Typing { input, was, from: self.at.clone() });
        self.ensure_index();
    }

    /// Walk the tree's root for the index unless it already is.
    fn ensure_index(&mut self) {
        if self.search.index.as_ref().is_some_and(|i| i.root == self.here) {
            return;
        }
        let id = self.next_generation();
        let root = self.here.clone();
        self.worker.with(|w| w.walk(id, root.clone()));
        self.search.gone.clear();
        self.search.index =
            Some(Index { id, root, entries: Arc::new(Vec::new()), walking: true, skipping: None, failed: None });
    }

    /// The tree's root changed: an index of the old root is dropped — walked again at once when a search stands.
    fn reroot(&mut self) {
        if self.search.index.as_ref().is_none_or(|i| i.root == self.here) {
            return;
        }
        self.worker.with(Worker::stop_walk);
        self.search.index = None;
        if self.query().is_some() {
            self.ensure_index();
            self.refind(false);
        }
    }

    /// A key in the search field: the field takes it and the marks follow as typed; `Enter` hangs it, `Esc` goes back
    /// to the search before and puts the cursor back.
    fn type_key(&mut self, k: KeyEvent) {
        let Some(t) = &mut self.typing else { return };
        let was = t.input.text().to_owned();
        if t.input.key(k) {
            // A key that only moved the field's cursor finds nothing new.
            if t.input.text() != was {
                self.refind(true);
            }
            return;
        }
        match keys::lookup(PROMPT, &[k]) {
            Lookup::Run(Prompt::Apply) => {
                let text = t.input.text().trim().to_string();
                self.typing = None;
                // What it found is what the field found — nothing to find again.
                if text.is_empty() {
                    self.clear_search();
                } else {
                    self.search.query = Some(text);
                }
            }
            Lookup::Run(Prompt::Cancel) => {
                let (was, from) = (t.was.take(), t.from.clone());
                self.typing = None;
                self.search.query = was;
                self.refind(false);
                self.at = from;
                self.rebuild();
            }
            _ => {}
        }
    }

    /// Paste — into the search field when it is open, else nowhere.
    pub(super) fn paste(&mut self, s: &str) {
        if let Some(t) = &mut self.typing {
            t.input.paste(s);
            self.refind(true);
        }
    }

    /// `Esc` over a hung search — the marks go and what the search opened folds back.
    fn clear_search(&mut self) {
        self.search.query = None;
        self.search.jump = false;
        self.refind(false);
    }

    /// Match the query over the whole index afresh — when it changed. Folders opened for files no longer found fold
    /// back; the new ones are queued. `jump` sends the cursor to the first found.
    fn refind(&mut self, jump: bool) {
        let needle = self.query().map(str::to_lowercase);
        let s = &mut self.search;
        s.found.clear();
        s.total = 0;
        s.aimed.clear();
        s.queue.clear();
        if let (Some(needle), Some(index)) = (&needle, &s.index) {
            for e in index.entries.iter().filter(|e| e.folded.contains(needle.as_str())) {
                if !s.gone.contains(&e.path) {
                    s.total += 1;
                    admit(&mut s.found, e.depth, &e.path);
                }
            }
        }
        s.jump = jump && needle.is_some();
        let keep: HashSet<String> = self
            .search
            .found
            .iter()
            .flat_map(|(_, p)| ancestors(&self.here, p).map(str::to_owned).collect::<Vec<_>>())
            .collect();
        self.unwind(&keep);
        self.aim_found();
        self.pump();
        self.rebuild();
    }

    /// A batch of the walk: into the index, and matched at once when a search stands — the answers grow as it walks.
    fn indexed(&mut self, paths: Vec<Indexed>) {
        let needle = self.query().map(str::to_lowercase);
        let s = &mut self.search;
        let Some(index) = s.index.as_mut() else { return };
        if let Some(needle) = &needle {
            for e in paths.iter().filter(|e| e.folded.contains(needle.as_str())) {
                s.total += 1;
                admit(&mut s.found, e.depth, &e.path);
            }
        }
        Arc::make_mut(&mut index.entries).extend(paths);
        if needle.is_some() {
            self.aim_found();
        }
    }

    /// Queue the folders of the found files not aimed at yet, nearest the root first. A file whose folder is already
    /// read is checked against it at once ([`Window::drop_gone`]) — no reply will come for that folder.
    fn aim_found(&mut self) {
        let here = self.here.clone();
        let s = &mut self.search;
        let mut read = BTreeSet::new();
        for (_, path) in &s.found {
            if s.aimed.insert(path.clone()) {
                s.queue.extend(ancestors(&here, path).filter(|d| !self.dirs.contains_key(*d)).map(str::to_owned));
                let parent = parent_of(path);
                if self.dirs.get(parent).is_some_and(|d| d.read.is_some()) {
                    read.insert(parent.to_owned());
                }
            }
        }
        for dir in read {
            self.drop_gone(&dir);
        }
    }

    /// Send the queued folders while fewer than [`SEARCH_READS`] of the search's are out.
    fn pump(&mut self) {
        let out =
            |w: &Window| w.search.opened.iter().filter(|d| w.dirs.get(*d).is_some_and(|x| x.read.is_none())).count();
        let mut n = out(self);
        while n < SEARCH_READS {
            let Some(d) = self.search.queue.pop_front() else { break };
            if self.dirs.contains_key(&d) {
                continue;
            }
            self.ask(&d);
            self.search.opened.insert(d);
            n += 1;
        }
    }

    /// Fold back what the search opened, but `keep` — deepest first, so the cursor climbs to the folder that stays.
    /// A folder holding one the hand opened stays open: folding it would hide the hand's.
    fn unwind(&mut self, keep: &HashSet<String>) {
        let mut going: Vec<String> = self.search.opened.iter().filter(|d| !keep.contains(*d)).cloned().collect();
        going.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
        for d in going {
            if !self.search.opened.contains(&d) {
                continue;
            }
            if self.dirs.keys().any(|p| inside(p, &d) && !self.search.opened.contains(p)) {
                continue;
            }
            let climbs = match &self.at {
                At::Path(p) => inside(p, &d),
                At::Note(p) => *p == d || inside(p, &d),
                At::Up => false,
            };
            if climbs {
                self.at = At::Path(d.clone());
            }
            self.close(&d);
        }
    }

    /// A folder was read: the found files the index put in it that it does not hold are left out without a word —
    /// the index and the tree were read at different times.
    fn drop_gone(&mut self, dir: &str) {
        if self.search.found.is_empty() {
            return;
        }
        let Some(Read::Listed { entries, .. }) = self.dirs.get(dir).and_then(|d| d.read.as_ref()) else { return };
        let names: HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        let gone: Vec<(u16, String)> = self
            .search
            .found
            .iter()
            .filter(|(_, p)| parent_of(p) == dir && p.rsplit('/').next().is_some_and(|n| !names.contains(n)))
            .cloned()
            .collect();
        for key in gone {
            self.search.found.remove(&key);
            self.search.total = self.search.total.saturating_sub(1);
            self.search.gone.insert(key.1);
        }
    }

    /// Where a pending jump lands — the first found, in the found order, whose row stands.
    fn jump_target(&self) -> Option<String> {
        if !self.search.jump {
            return None;
        }
        let rank: HashMap<&str, usize> =
            self.search.found.iter().enumerate().map(|(i, (_, p))| (p.as_str(), i)).collect();
        self.rows
            .iter()
            .filter_map(|r| match r {
                Row::Entry { path, .. } => rank.get(path.as_str()).map(|&i| (i, path)),
                _ => None,
            })
            .min()
            .map(|(_, p)| p.clone())
    }

    /// `n`·`N` — to the next or the previous found row in the tree's order, round at the ends. With none standing yet,
    /// the first found is aimed at: its folders go to the front of the queue and the cursor lands when it stands.
    pub fn jump_found(&mut self, back: bool) {
        if !self.searched() || self.search.found.is_empty() {
            return;
        }
        let found: HashSet<&str> = self.search.found.iter().map(|(_, p)| p.as_str()).collect();
        let at: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Row::Entry { path, .. } if found.contains(path.as_str())))
            .map(|(i, _)| i)
            .collect();
        let to = if back {
            at.iter().rev().find(|&&i| i < self.cursor).or(at.last())
        } else {
            at.iter().find(|&&i| i > self.cursor).or(at.first())
        };
        match to.copied() {
            Some(i) => self.move_to(i),
            None => {
                let first = self.search.found.first().map(|(_, p)| p.clone());
                if let Some(first) = first {
                    let ahead: Vec<String> = ancestors(&self.here, &first).map(str::to_owned).collect();
                    self.search.queue.retain(|d| !ahead.contains(d));
                    for d in ahead.into_iter().rev() {
                        self.search.queue.push_front(d);
                    }
                    self.search.jump = true;
                    self.pump();
                    self.rebuild();
                }
            }
        }
    }

    pub(super) fn ctl_state(&self) -> serde_json::Value {
        serde_json::json!({
            "checkout": self.checkout.path, "here": self.here, "cursor": self.current_path(),
            "focus": if self.focus == Side::Tree { "tree" } else { "info" },
            "search": self.search.query,
        })
    }
}

impl App {
    /// `SPC g f` — open the window on the checkout the session stands in, or on the one list on the project of the
    /// row under the cursor. With neither a notice says why and nothing opens.
    pub(super) fn open_files(&mut self) {
        let lang = self.site.lang;
        let (project, path) = if !self.on_layer() {
            let Some(repo) = self.site.repo.as_ref() else {
                self.notice = Some(say(lang, "tui.files.no_project").to_string());
                return;
            };
            (self.project_name(), repo.here().to_path_buf())
        } else {
            let at = match self.current() {
                Some(ListRow::Project(at)) | Some(ListRow::Item(Seat::Place(at), ..)) => at,
                _ => {
                    self.notice = Some(say(lang, "tui.files.no_target").to_string());
                    return;
                }
            };
            let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
            (place.name.clone(), place.path.clone())
        };
        // The key and the branch are the checkout picker's (moai-ffk9.211); finding them reads git's files, and the
        // loop does not read.
        let checkout = Checkout { key: ".".into(), path, branch: String::new(), main: true, milestone: None };
        self.mode = Mode::Files(Box::new(Window::new(project, checkout, self.keys, &self.files_disk)));
    }

    /// The window is waiting on its threads — the loop wakes fast to take the answer (`App::loading`).
    pub(super) fn reading_files(&self) -> bool {
        matches!(&self.mode, Mode::Files(w) if w.waiting())
    }

    /// The step of [`App::follow`] for the window: take what its threads answered.
    pub(super) fn follow_files(&mut self) {
        let Mode::Files(w) = &mut self.mode else { return };
        let got = w.poll();
        self.follow_changed |= got.changed || got.painted;
        if let Some(n) = got.stopped {
            let expand = keys::said(w.keys.files, keys::Files::Expand);
            self.notice = Some(fill(
                say(self.site.lang, "tui.files.spread_stopped"),
                &[("n", &n.to_string()), ("expand", &expand)],
            ));
        }
        if let Some(n) = got.indexed {
            self.notice = Some(fill(say(self.site.lang, "tui.files.index_stopped"), &[("n", &n.to_string())]));
        }
    }

    /// One key on the window. **Closing only goes back to browsing** — the window touched nothing behind it.
    ///
    /// **SPC opens the menu** — the statistics and wiki windows' way (`App::wiki_key`); what stands there is the
    /// screen group (`keys::Ctx::files`). A move key that closes the menu moves the window's focused pane.
    ///
    /// Answers **whether this key clears the notice** — the wiki window's rule: a key that only touched the menu and
    /// a pending prefix do not, the window's keys, a screen pick and an unknown key do.
    pub(super) fn files_key(&mut self, k: KeyEvent) -> bool {
        // **While the search field is open it comes first** — the moves and SPC are letters (the wiki window's way).
        if let Mode::Files(w) = &mut self.mode
            && w.typing.is_some()
        {
            w.type_key(k);
            return true;
        }
        if menu::open(&self.chord) || LEADER.matches(k) {
            if let Mode::Files(w) = &mut self.mode {
                w.chord.clear();
            }
            let rows = self.rows();
            let ctx = self.key_ctx(&rows);
            let act = menu::feed(&mut self.chord, &ctx, k);
            // A prefix that closed the menu (`g`) stays on the browse chord — move it to the window's.
            if !menu::open(&self.chord) {
                let held = self.chord.held().to_vec();
                self.chord.clear();
                if let Mode::Files(w) = &mut self.mode {
                    for k in held {
                        w.chord.feed(self.keys.files, k);
                    }
                }
            }
            return match act {
                Some(Browse::Go(to)) => {
                    self.go(to, &rows);
                    true
                }
                Some(Browse::Help) => {
                    self.open_help(&rows);
                    false
                }
                Some(Browse::Step(m)) => {
                    self.run(Browse::Step(m), &rows);
                    false
                }
                Some(act @ (Browse::ProposalAccept | Browse::ProposalDecline | Browse::Proposals)) => {
                    self.run(act, &rows);
                    true
                }
                _ => false,
            };
        }
        let Mode::Files(w) = &mut self.mode else { return false };
        use keys::Files as F;
        match w.chord.feed(self.keys.files, k) {
            Some(F::Step(m)) => w.step(m),
            Some(F::Enter) if w.focus == Side::Tree => w.enter(),
            Some(F::Leave) if w.focus == Side::Tree => w.leave(),
            Some(F::Expand) if w.focus == Side::Tree => w.expand(),
            Some(F::Collapse) if w.focus == Side::Tree => w.collapse(),
            Some(F::ExpandAll) if w.focus == Side::Tree => w.spread(),
            Some(F::Enter | F::Leave | F::Expand | F::Collapse | F::ExpandAll) => {}
            Some(F::Pan(toward)) if w.focus == Side::Info => w.pan(toward),
            Some(F::Pan(_)) => {}
            Some(F::FocusNext) => w.focus_next(),
            Some(F::Focus(side)) => w.focus = if side == Toward::Left { Side::Tree } else { Side::Info },
            Some(F::Search) => w.start_search(),
            Some(F::NextFound) => w.jump_found(false),
            Some(F::PrevFound) => w.jump_found(true),
            // A hung search ends first, the window after — the list's `Esc` that clears the filter before it leaves.
            Some(F::Close) if w.searched() => w.clear_search(),
            Some(F::Close) => self.mode = Mode::Browse,
            None => {
                let idle = w.chord.held().is_empty();
                return idle && !self.help_from_window(k);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::{App, Mode, Pane, draw};
    use super::*;
    use crate::config::Config;
    use crate::model::{Issue, Kind as IssueKind, Status};
    use crate::nav::Path as NavPath;
    use crate::scratch::Scratch;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    /// A checkout with `.git`, a hidden file, a nested source tree, an ignored build directory and a link — and the
    /// explorer standing in it.
    fn files_app(name: &str) -> (Scratch, App) {
        let s = Scratch::new(&format!("tui-files-{name}"));
        let root = s.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("src/tui")).unwrap();
        std::fs::create_dir_all(root.join("target/debug/deps")).unwrap();
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        std::fs::write(root.join("README.md"), "# hi\n").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(root.join("src/tui/files.rs"), "//! files\n").unwrap();
        std::fs::write(root.join("target/debug/out"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("src", root.join("lnk")).unwrap();
        let issues = vec![Issue::new(
            "argos-0001".into(),
            "첫 일".into(),
            IssueKind::Issue,
            Status::new("todo"),
            "2026-09-01T00:00:00Z",
        )];
        let mut a = App::new(issues, cfg(), NavPath::new());
        a.site.repo = Some(crate::store::Repo::at(root.to_path_buf(), cfg()));
        a.files_disk = Disk::with(files::list, |_| Some(vec!["target/".to_string()]));
        (s, a)
    }

    fn window(a: &App) -> &Window {
        match &a.mode {
            Mode::Files(w) => w,
            other => panic!("the file window is not up — {other:?}"),
        }
    }

    /// Take the threads' answers until nothing is out — as the loop does on its fast step. The budget is wide: the first
    /// colouring in a test process loads the syntax set, which takes seconds in a debug build on a busy machine.
    fn settle(a: &mut App) {
        let began = Instant::now();
        loop {
            a.follow_files();
            if !a.reading_files() {
                return;
            }
            assert!(began.elapsed() < Duration::from_secs(60), "the window kept waiting — {:?}", a.mode);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn names(a: &App) -> Vec<String> {
        window(a)
            .rows()
            .iter()
            .map(|r| match r {
                Row::Up => "..".to_string(),
                Row::Entry { path, open, .. } => format!("{path}{}", if *open { " (open)" } else { "" }),
                Row::Loading { of, .. } => format!("<reading {of}>"),
                Row::Failed { of, .. } => format!("<failed {of}>"),
            })
            .collect()
    }

    /// Put the cursor on the row of that path.
    fn on(a: &mut App, path: &str) {
        let Mode::Files(w) = &mut a.mode else { panic!("no window") };
        let at = w.rows().iter().position(|r| matches!(r, Row::Entry { path: p, .. } if p == path));
        w.move_to(at.unwrap_or_else(|| panic!("no row {path} in {:?}", w.rows())));
    }

    fn here(a: &App) -> &str {
        &window(a).here
    }

    fn entry(name: &str, kind: Kind) -> Entry {
        Entry { name: name.into(), kind, size: 0, openable: true }
    }

    fn listed(entries: Vec<Entry>) -> files::Listed {
        files::Listed { entries, stamp: files::Stamp { modified: None, size: 0 } }
    }

    /// **`SPC g f` opens the window on the checkout and `Esc` gives the screen back as it was.** Every name stands but
    /// `.git` — the hidden file and the ignored build directory too — directories first.
    #[test]
    fn spc_g_f_opens_the_window_and_esc_gives_the_screen_back_as_it_was() {
        let (_s, mut a) = files_app("open");
        a.focus = Pane::Detail;
        a.list.by(0);
        let before = (a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail, a.list);
        a.hit("SPC g f");
        settle(&mut a);
        // A link to a directory inside sorts with the directories (`files::list`'s order).
        assert_eq!(names(&a), ["lnk", "src", "target", ".env", "README.md"]);
        assert_eq!((window(&a).focus, window(&a).cursor), (Side::Tree, 0));
        a.hit("j l");
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(
            (a.cursor, a.focus, a.hung.clone(), a.detail_open, a.detail, a.list),
            before,
            "the window moved the screen"
        );
    }

    /// The screens over the window: `SPC g l` closes it onto the list; the menu over it holds only the screen group and
    /// the key list, and a move key closing the menu moves the tree's cursor.
    #[test]
    fn spc_over_the_window_opens_only_the_screens() {
        use super::super::{keys::Screen, menu, view::Layout};
        let (_s, mut a) = files_app("menu");
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("SPC");
        let ctx = a.key_ctx(&a.rows());
        assert_eq!(ctx.screen(), Screen::Files);
        let root = menu::entries(a.chord.held(), &ctx, &[]);
        assert_eq!(root.iter().map(menu::Entry::text).collect::<Vec<_>>(), ["+화면 [파일]", "키 목록"]);
        assert_eq!(Browse::Filter.enabled(&ctx), Err(keys::Off::Quiet));
        assert_eq!(Browse::Raw.enabled(&ctx), Err(keys::Off::Quiet));
        a.hit("j");
        assert!(!menu::open(&a.chord));
        assert_eq!(window(&a).cursor, 1, "the move closing the menu did not move the cursor");
        a.hit("SPC g l");
        assert_eq!((&a.mode, a.layout), (&Mode::Browse, Layout::List), "SPC g l did not close the window");
        a.hit("SPC g f SPC g w");
        assert!(matches!(a.mode, Mode::Wiki(_)) || a.notice.is_some(), "SPC g w over the window did nothing");
    }

    /// **No project, no window** — a notice says why and the screen stays.
    #[test]
    fn no_project_opens_nothing_and_says_why() {
        let mut a = App::new(Vec::new(), cfg(), NavPath::new());
        a.hit("SPC g f");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(a.notice.as_deref(), Some(say(a.site.lang, "tui.files.no_project")));
    }

    /// **The checkout is the jail.** At the root no `..` stands and `Bksp`, `h` stay; one level down `..` is the first
    /// row and `Bksp`, `h` from the top and `Enter` on `..` come back with the cursor on the directory left.
    #[test]
    fn the_root_has_no_way_up_and_nothing_leaves_the_checkout() {
        let (_s, mut a) = files_app("jail");
        a.hit("SPC g f");
        settle(&mut a);
        let at_root = names(&a);
        assert!(!matches!(window(&a).rows()[0], Row::Up), "the root has a way up");
        for keys in ["Bksp", "h", "G h", "G Bksp", "g g h"] {
            a.hit(keys);
            settle(&mut a);
            assert_eq!((here(&a), names(&a)), (".", at_root.clone()), "{keys} left the root");
        }
        on(&mut a, "src");
        a.hit("Enter");
        settle(&mut a);
        assert_eq!(here(&a), "src");
        assert_eq!(names(&a), ["..", "src/tui", "src/main.rs"]);
        assert_eq!(window(&a).cursor, 1, "the cursor is not on the first entry");
        a.hit("Bksp");
        assert_eq!((here(&a), window(&a).current_path()), (".", Some("src")));
        on(&mut a, "src");
        a.hit("Enter");
        settle(&mut a);
        a.hit("h");
        assert!(matches!(window(&a).current(), Some(Row::Up)), "h at the top did not go to ..");
        a.hit("Enter");
        assert_eq!((here(&a), window(&a).current_path()), (".", Some("src")), "Enter on .. did not step out");
        a.hit("Bksp");
        assert_eq!((here(&a), names(&a)), (".", at_root), "Bksp at the root moved");
    }

    /// **The hand is the list's**: `l` opens one level and does nothing on an open one, `h` folds an open one and goes to
    /// the parent row from anything else, `Tab` opens everything below and folds it all back.
    #[test]
    fn l_h_and_tab_behave_as_on_the_list() {
        let (_s, mut a) = files_app("hand");
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        let opened = names(&a);
        assert_eq!(opened[1..5], ["src (open)", "src/tui", "src/main.rs", "target"]);
        a.hit("l");
        settle(&mut a);
        assert_eq!(names(&a), opened, "l on an open directory did something");
        on(&mut a, "src/main.rs");
        a.hit("h");
        assert_eq!(window(&a).current_path(), Some("src"), "h on a file did not go to the parent row");
        a.hit("h");
        assert_eq!(names(&a)[1], "src", "h on an open directory did not fold it");
        a.hit("Tab");
        settle(&mut a);
        assert_eq!(
            names(&a)[1..5],
            ["src (open)", "src/tui (open)", "src/tui/files.rs", "src/main.rs"],
            "Tab did not spread"
        );
        a.hit("Tab");
        assert_eq!(names(&a)[1], "src", "Tab on an open directory did not fold it");
        a.hit("l");
        settle(&mut a);
        assert_eq!(names(&a)[2], "src/tui", "Tab left a folded branch open below");
        // A file and `..` take no `l`, `Tab`; `Enter` on a file goes to the right pane.
        on(&mut a, "README.md");
        let before = names(&a);
        a.hit("l Tab");
        assert_eq!(names(&a), before);
        a.hit("Enter");
        assert_eq!(window(&a).focus, Side::Info);
        a.hit("Ctrl-w w");
        assert_eq!(window(&a).focus, Side::Tree);
    }

    /// **`Tab` skips what git ignores and does not follow links** — the rows stand, marked, and `l` on one opens it.
    /// Without the ignored list it skips nothing.
    #[test]
    fn tab_skips_ignored_directories_and_links() {
        let (s, mut a) = files_app("ignored");
        let top = s.path().join("top");
        std::fs::create_dir_all(top.join("target/deep")).unwrap();
        std::fs::create_dir_all(top.join("src/a")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("src", top.join("lnk")).unwrap();
        a.files_disk = Disk::with(files::list, |_| Some(vec!["top/target/".to_string()]));
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "top");
        a.hit("Tab");
        settle(&mut a);
        let rows = names(&a);
        assert!(rows.contains(&"top/src/a (open)".to_string()), "{rows:?}");
        assert!(rows.contains(&"top/target".to_string()), "the ignored directory left the rows: {rows:?}");
        assert!(!rows.iter().any(|r| r.starts_with("top/target/")), "Tab went into the ignored directory: {rows:?}");
        #[cfg(unix)]
        assert!(rows.contains(&"top/lnk".to_string()), "Tab followed the link: {rows:?}");
        let ignored = window(&a)
            .rows()
            .iter()
            .any(|r| matches!(r, Row::Entry { path, ignored: true, .. } if path == "top/target"));
        assert!(ignored, "the ignored directory is not marked");
        on(&mut a, "top/target");
        a.hit("l");
        settle(&mut a);
        assert!(names(&a).contains(&"top/target/deep".to_string()), "l did not open the ignored directory");

        // Without the list (no git, a timeout) the spread skips nothing.
        a.hit("Esc");
        a.files_disk = Disk::with(files::list, |_| None);
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "top");
        a.hit("Tab");
        settle(&mut a);
        assert!(
            names(&a).contains(&"top/target (open)".to_string()),
            "Tab did nothing without the list: {:?}",
            names(&a)
        );
    }

    /// **`Tab` stops at its limit and says so** — the reads stop, they do not run on past 5,000 entries.
    #[test]
    fn tab_stops_after_its_limit_of_entries() {
        let (_s, mut a) = files_app("limit");
        let reads = Arc::new(AtomicUsize::new(0));
        let entries = Arc::new(AtomicUsize::new(0));
        let (r, e) = (reads.clone(), entries.clone());
        a.files_disk = Disk::with(
            move |_, path| {
                if path == "." {
                    return Ok(listed(vec![entry("wide", Kind::Dir)]));
                }
                r.fetch_add(1, Ordering::SeqCst);
                // Every directory holds a hundred directories — far past the limit within two levels.
                let list: Vec<Entry> = (0..100).map(|n| entry(&format!("d{n:03}"), Kind::Dir)).collect();
                e.fetch_add(list.len(), Ordering::SeqCst);
                Ok(listed(list))
            },
            |_| Some(Vec::new()),
        );
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("Tab");
        settle(&mut a);
        let read = entries.load(Ordering::SeqCst);
        assert!((SPREAD_LIMIT..SPREAD_LIMIT + 100).contains(&read), "read {read} entries");
        let notice = a.notice.clone().unwrap_or_default();
        assert!(notice.contains(&read.to_string()) && notice.contains('l'), "{notice:?}");
        let calls = reads.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(reads.load(Ordering::SeqCst), calls, "the spread kept reading");
    }

    /// **The cursor never waits on a read** — with a directory that takes seconds, opening it shows a reading row at
    /// once and `j` moves on.
    #[test]
    fn the_cursor_never_waits_on_a_slow_read() {
        let (_s, mut a) = files_app("slow");
        a.files_disk = Disk::with(
            |jail, path| {
                if path == "src" {
                    std::thread::sleep(Duration::from_secs(2));
                }
                files::list(jail, path)
            },
            |_| Some(Vec::new()),
        );
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "src");
        let began = Instant::now();
        a.hit("l");
        a.follow_files();
        assert_eq!(names(&a)[1..3], ["src (open)", "<reading src>"]);
        a.hit("j j");
        a.follow_files();
        assert!(began.elapsed() < Duration::from_millis(500), "a key waited on the read: {:?}", began.elapsed());
        assert_eq!(window(&a).current_path(), Some("target"), "the cursor did not move past the reading row");
        assert!(a.loading(), "the loop is not told to take the answer");
    }

    /// **A stale answer is dropped**: a directory folded while its read was out stays folded when the answer comes, and
    /// an answer for a closed window does not land on the next one.
    #[test]
    fn a_stale_answer_lands_nowhere() {
        let (_s, mut a) = files_app("stale");
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        a.files_disk = Disk::with(
            move |jail, path| {
                if path == "." && c.fetch_add(1, Ordering::SeqCst) == 0 {
                    // The first window's root answers late and wrong.
                    std::thread::sleep(Duration::from_millis(300));
                    return Ok(listed(vec![entry("from-the-first-window", Kind::File)]));
                }
                if path == "src" {
                    std::thread::sleep(Duration::from_millis(200));
                }
                files::list(jail, path)
            },
            |_| Some(Vec::new()),
        );
        a.hit("SPC g f");
        a.hit("Esc");
        a.hit("SPC g f");
        settle(&mut a);
        std::thread::sleep(Duration::from_millis(400));
        a.follow_files();
        assert!(!names(&a).iter().any(|n| n == "from-the-first-window"), "{:?}", names(&a));
        on(&mut a, "src");
        a.hit("l h");
        std::thread::sleep(Duration::from_millis(300));
        settle(&mut a);
        assert_eq!(names(&a)[1], "src", "the late answer opened a folded directory");
    }

    /// **A dead reader raises its panic on the loop**, as the reread thread's does.
    #[test]
    fn a_dead_reader_raises_its_panic() {
        let (_s, mut a) = files_app("panic");
        a.files_disk = Disk::with(|_, _| panic!("the disk fell over"), |_| Some(Vec::new()));
        a.hit("SPC g f");
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let began = Instant::now();
            while began.elapsed() < Duration::from_secs(5) {
                a.follow_files();
                std::thread::sleep(Duration::from_millis(2));
            }
        }));
        let payload = caught.expect_err("the reader's panic was swallowed");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"the disk fell over"));
    }

    /// **Colour never carries the meaning alone**: in the drawn text without colour a directory has its fold mark and
    /// `/`, a link its `→`, an ignored directory its word; the root says there is no way up.
    #[test]
    fn the_text_alone_tells_directories_files_and_links_apart() {
        let (_s, mut a) = files_app("text");
        a.hit("SPC g f");
        settle(&mut a);
        let screen = draw::tests::render(&mut a, 100, 20).join("\n");
        assert!(screen.contains("▸ src/"), "{screen}");
        assert!(screen.contains("target/  무시됨"), "{screen}");
        #[cfg(unix)]
        assert!(screen.contains("lnk → src/"), "{screen}");
        assert!(screen.contains("README.md") && !screen.contains("README.md/"), "{screen}");
        assert!(!screen.contains(".git"), "{screen}");
        assert!(screen.contains(say(a.site.lang, "tui.files.root")), "{screen}");
        assert!(screen.contains("Enter 열기") && screen.contains("Esc 닫기"), "{screen}");
        // A control character in a name never reaches the terminal raw.
        let line = draw::tests::files_row_text(&Row::Entry {
            path: "a\u{1b}[31mb".into(),
            depth: 0,
            entry: Entry { name: "a\u{1b}[31mb".into(), kind: Kind::File, size: 0, openable: false },
            open: false,
            ignored: false,
            link: None,
        });
        assert!(!line.chars().any(char::is_control), "{line:?}");
    }

    /// **The line keeps a share of its own**, written once on release; the list's share does not move, and a window
    /// never dragged writes nothing.
    #[test]
    fn dragging_the_files_line_keeps_a_share_of_its_own() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let press =
            |a: &mut App, kind, column, row| a.mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
        let (s, mut a) = files_app("drag");
        let user = s.join("user.toml");
        a.user_config = Some(user.clone());
        a.list_width = Some(40);
        let _ = draw::tests::render(&mut a, 100, 20);
        let explorer = a.drawn.list;
        a.hit("SPC g f");
        settle(&mut a);
        let _ = draw::tests::render(&mut a, 100, 20);
        let d = a.drawn.files.clone().unwrap();
        assert_eq!(d.tree.width, explorer.width, "the undragged window did not follow the list's share");
        a.hit("Esc SPC g f");
        settle(&mut a);
        assert!(!user.exists(), "an undragged window wrote its share");

        let (line, y) = (d.tree.right() - 1, d.tree.y + 3);
        press(&mut a, MouseEventKind::Down(MouseButton::Left), line, y);
        assert!(a.dragging.is_some(), "the line was not caught");
        press(&mut a, MouseEventKind::Drag(MouseButton::Left), 59, y);
        let _ = draw::tests::render(&mut a, 100, 20);
        assert_eq!(a.drawn.files.clone().unwrap().tree.right() - 1, 59, "the line did not follow the hand");
        press(&mut a, MouseEventKind::Drag(MouseButton::Left), 61, y);
        assert!(!user.exists(), "the share was written while dragging");
        press(&mut a, MouseEventKind::Drag(MouseButton::Left), 59, y);
        press(&mut a, MouseEventKind::Up(MouseButton::Left), 59, y);
        assert_eq!((a.files_width, a.list_width), (Some(60), Some(40)), "the share is not the window's");
        let text = std::fs::read_to_string(&user).expect("the share was not written on release");
        assert!(text.contains("files_width = 60") && !text.contains("list_width = 60"), "{text}");
        a.hit("Esc");
        let _ = draw::tests::render(&mut a, 100, 20);
        assert_eq!(a.drawn.list, explorer, "the window's drag moved the explorer's line");
    }

    /// A click puts the focus and the cursor; the wheel scrolls the tree and leaves the cursor.
    #[test]
    fn a_click_picks_the_row_and_the_wheel_scrolls() {
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let press =
            |a: &mut App, kind, column, row| a.mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE });
        let (_s, mut a) = files_app("click");
        a.hit("SPC g f");
        settle(&mut a);
        let _ = draw::tests::render(&mut a, 100, 20);
        let d = a.drawn.files.clone().unwrap();
        press(&mut a, MouseEventKind::Down(MouseButton::Left), d.rows.x + 3, d.rows.y + 2);
        assert_eq!(window(&a).cursor, 2);
        let info = d.info.unwrap();
        press(&mut a, MouseEventKind::Down(MouseButton::Left), info.x + 3, info.y + 2);
        assert_eq!((window(&a).focus, window(&a).cursor), (Side::Info, 2));
        press(&mut a, MouseEventKind::ScrollDown, d.rows.x + 3, d.rows.y + 1);
        assert_eq!((window(&a).focus, window(&a).cursor), (Side::Info, 2), "the wheel moved the focus or the cursor");
    }

    /// **`moai ctl screen files` opens the window and `ctl state` says so.**
    #[test]
    fn ctl_screen_files_opens_the_window() {
        use crate::ctl::wire::{self, Verb};
        let (_s, mut a) = files_app("ctl");
        let reply = a.ctl(&wire::request(Verb::Screen { name: "files".into() }, Duration::from_secs(5)));
        assert!(reply.ok, "{reply:?}");
        window(&a);
        let state = a.ctl_state(false);
        assert_eq!((state["screen"].as_str(), state["mode"].as_str()), (Some("files"), Some("files")));
        assert_eq!(state["files"]["here"], ".");
        let mut none = App::new(Vec::new(), cfg(), NavPath::new());
        let refused = none.ctl(&wire::request(Verb::Screen { name: "files".into() }, Duration::from_secs(5)));
        assert_eq!(refused.code.as_deref(), Some("refused"), "{refused:?}");
    }

    // ---- the right pane (moai-ffk9.3fd) ----

    /// Open the window, stand on a file and take every answer.
    fn standing_on(a: &mut App, path: &str) {
        if !matches!(a.mode, Mode::Files(_)) {
            a.hit("SPC g f");
        }
        settle(a);
        on(a, path);
        settle(a);
    }

    /// The right pane's text inside its border, as drawn — with the place it stands in.
    fn pane(a: &mut App, w: u16, h: u16) -> (Vec<String>, ratatui::buffer::Buffer, ratatui::layout::Rect) {
        let buf = draw::tests::render_buf(a, w, h);
        let info = a.drawn.files.clone().and_then(|d| d.info).expect("the right pane did not stand");
        let inner = ratatui::layout::Rect {
            x: info.x + 1,
            y: info.y + 1,
            width: info.width.saturating_sub(2),
            height: info.height.saturating_sub(2),
        };
        let lines = (inner.y..inner.bottom())
            .map(|y| {
                let mut line = String::new();
                let mut skip = 0usize;
                for x in inner.x..inner.right() {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    let sym = buf[(x, y)].symbol();
                    line.push_str(sym);
                    skip = crate::text::width(sym).saturating_sub(1);
                }
                line.trim_end().to_string()
            })
            .collect();
        (lines, buf, inner)
    }

    fn shown(a: &App) -> &Shown {
        &window(a).right.shown
    }

    fn reads_counted(a: &mut App, delay: Duration) -> Arc<AtomicUsize> {
        let n = Arc::new(AtomicUsize::new(0));
        let c = n.clone();
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.reading(move |jail, path| {
            c.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(delay);
            files::read(jail, path)
        });
        n
    }

    /// **The right pane follows the cursor** and numbers the lines as they are; a hidden file reads like any other.
    #[test]
    fn the_right_pane_shows_the_file_under_the_cursor() {
        let (s, mut a) = files_app("content");
        std::fs::write(s.path().join("notes.txt"), "one\ntwo\n\nfour\n").unwrap();
        standing_on(&mut a, "README.md");
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 # hi");
        on(&mut a, "notes.txt");
        settle(&mut a);
        let (lines, ..) = pane(&mut a, 100, 20);
        assert_eq!(lines[..4], ["1 one", "2 two", "3", "4 four"], "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("# hi")), "the file left behind still stands: {lines:?}");
        on(&mut a, ".env");
        settle(&mut a);
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 SECRET=1", "the hidden file does not read like the others");
        // A directory tells of itself; `..` stands empty.
        on(&mut a, "src");
        assert!(matches!(shown(&a), Shown::Nothing));
        a.hit("Enter");
        settle(&mut a);
        a.hit("g g");
        assert!(pane(&mut a, 100, 20).0.iter().all(String::is_empty), "`..` is not empty");
    }

    /// **Moves on the right pane scroll the text; a file the cursor comes to stands at its top**, whatever the last
    /// one was scrolled to.
    #[test]
    fn the_text_scrolls_with_the_focus_and_a_new_file_starts_at_the_top() {
        let (s, mut a) = files_app("scroll");
        let long: String = (1..=100).map(|n| format!("line {n}\n")).collect();
        std::fs::write(s.path().join("long.txt"), &long).unwrap();
        standing_on(&mut a, "long.txt");
        let _ = pane(&mut a, 100, 20);
        a.hit("Enter j j j");
        assert_eq!(pane(&mut a, 100, 20).0[0], "  4 line 4");
        a.hit("Ctrl-w w j");
        assert_eq!(window(&a).current_path(), Some("README.md"), "j on the tree did not move the cursor");
        a.hit("k");
        settle(&mut a);
        assert_eq!(window(&a).right.page.offset(), 0, "the file came back scrolled");
        assert_eq!(pane(&mut a, 100, 20).0[0], "  1 line 1");
        a.hit("Enter G");
        let (lines, ..) = pane(&mut a, 100, 20);
        assert_eq!(lines.iter().rev().find(|l| !l.is_empty()).map(String::as_str), Some("100 line 100"));
    }

    /// **The table of what stands**: a file that is not text says so with its size and shows nothing of itself; a file
    /// past 1 MiB shows its head and says how large it is; the highlighter not knowing a file leaves its text plain.
    #[test]
    fn not_text_truncated_and_unknown_files_say_so() {
        let (s, mut a) = files_app("kinds");
        std::fs::write(s.path().join("blob.dat"), b"\x00\x01\x02garbage").unwrap();
        let big: String = (0..12_000).map(|n| format!("{n:0>99}\n")).collect();
        std::fs::write(s.path().join("big.txt"), &big).unwrap();
        std::fs::write(s.path().join("plain.veryunlikely"), "just words\n").unwrap();
        standing_on(&mut a, "blob.dat");
        let (lines, ..) = pane(&mut a, 100, 20);
        assert_eq!(lines[0], "글이 아니다 — 10 바이트", "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("garbage")), "{lines:?}");
        on(&mut a, "big.txt");
        settle(&mut a);
        a.hit("Enter G");
        let (lines, ..) = pane(&mut a, 100, 20);
        let said = format!("전체 {} 바이트", big.len());
        assert!(lines.iter().any(|l| l.contains("잘렸다") && l.contains(&said)), "{lines:?}");
        a.hit("Ctrl-w w");
        on(&mut a, "plain.veryunlikely");
        settle(&mut a);
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 just words");
        let Shown::Text(c) = shown(&a) else { panic!("{:?}", shown(&a)) };
        assert!(c.paint(0).is_empty() && c.paint.get().is_some(), "an unknown file got colour");
    }

    /// **Long lines scroll sideways to their end, not past it**; the numbers stay; `‹` and `›` tell what is hidden;
    /// a Hangul line cut mid-glyph leaves a blank and nothing after it moves; another file starts at the left edge.
    #[test]
    fn long_lines_scroll_sideways_to_their_end() {
        let (s, mut a) = files_app("pan");
        let wide = format!("start-{}-end", "abcdefghij".repeat(30));
        let hangul = "한글".repeat(40);
        std::fs::write(s.path().join("wide.txt"), format!("{wide}\n{hangul}\nshort\n")).unwrap();
        standing_on(&mut a, "wide.txt");
        let (lines, buf, inner) = pane(&mut a, 100, 20);
        assert!(lines[0].starts_with("1 start-") && lines[0].ends_with('›'), "{lines:?}");
        assert_eq!(buf[(inner.right() - 1, inner.y)].symbol(), "›", "the mark is not at the pane's end");
        assert!(!lines[0].contains("-end"));
        a.hit("L");
        assert_eq!(window(&a).right.pan, 0, "L moved the text while the tree held the focus");
        a.hit("Enter");
        for _ in 0..40 {
            a.hit("L");
            let _ = pane(&mut a, 100, 20);
        }
        let (lines, ..) = pane(&mut a, 100, 20);
        let w = window(&a);
        let Shown::Text(c) = &w.right.shown else { panic!() };
        assert_eq!(w.right.pan, c.widest - w.right.room, "the scroll did not stop at the widest line's end");
        assert!(lines[0].starts_with("1‹") && lines[0].ends_with("-end"), "{lines:?}");
        assert!(lines[2].starts_with("3‹") && !lines[2].contains("short"), "{lines:?}");
        // Mid-glyph: one column in, `한` is cut — a blank, then `글` on its own columns.
        let Mode::Files(w) = &mut a.mode else { panic!() };
        w.right.pan = 1;
        let (lines, buf, inner) = pane(&mut a, 100, 20);
        assert!(lines[1].starts_with("2‹ 글한글"), "{lines:?}");
        assert_eq!(buf[(inner.x + 3, inner.y + 1)].symbol(), "글");
        assert_eq!(buf[(inner.right() - 1, inner.y + 1)].symbol(), "›", "the Hangul line pushed its mark: {lines:?}");
        a.hit("H");
        assert_eq!(window(&a).right.pan, 0);
        a.hit("Ctrl-w w");
        a.hit("Enter L");
        let _ = pane(&mut a, 100, 20);
        assert!(window(&a).right.pan > 0);
        a.hit("Ctrl-w w");
        on(&mut a, "README.md");
        settle(&mut a);
        on(&mut a, "wide.txt");
        settle(&mut a);
        assert_eq!(window(&a).right.pan, 0, "another file kept the sideways scroll");
    }

    /// **`H`·`L` are the window's own**: the gantt's table still holds its own, and the file window's bar names them
    /// while the right pane has the focus.
    #[test]
    fn h_and_l_belong_to_the_window_and_the_gantt_keeps_its_own() {
        use super::super::keys::{Browse, Files as F, GANTT, Pan};
        let gantt = |label| GANTT.iter().find(|b| b.label == Some(label)).map(|b| b.act);
        assert_eq!((gantt("H"), gantt("L")), (Some(Browse::Gantt(Pan::Left)), Some(Browse::Gantt(Pan::Right))));
        let files = |label| keys::FILES.iter().find(|b| b.label == Some(label)).map(|b| b.act);
        assert_eq!((files("H"), files("L")), (Some(F::Pan(Toward::Left)), Some(F::Pan(Toward::Right))));
        let (_s, mut a) = files_app("bar");
        standing_on(&mut a, "README.md");
        a.hit("Enter");
        let screen = draw::tests::render(&mut a, 120, 20).join("\n");
        assert!(screen.contains("H·L 가로"), "{screen}");
    }

    /// **One read at a time, the newest only**: with every read slow, a hundred `j` never wait on one, and after them
    /// the reader read twice — the one running and the last one wanted. The pane stands on the cursor's file.
    #[test]
    fn a_hundred_moves_over_slow_files_read_twice() {
        let (s, mut a) = files_app("pile");
        let many = s.path().join("many");
        std::fs::create_dir_all(&many).unwrap();
        for n in 0..120 {
            std::fs::write(many.join(format!("f{n:03}.txt")), format!("file {n}\n")).unwrap();
        }
        // A read takes a second — a key that waited on one would show it plainly.
        let reads = reads_counted(&mut a, Duration::from_secs(1));
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "many");
        a.hit("Enter");
        settle(&mut a);
        let before = reads.load(Ordering::SeqCst);
        let began = Instant::now();
        a.hit("j");
        for _ in 0..99 {
            a.hit("j");
            a.follow_files();
        }
        assert!(began.elapsed() < Duration::from_millis(900), "a key waited on a read: {:?}", began.elapsed());
        assert_eq!(window(&a).current_path(), Some("many/f100.txt"));
        settle(&mut a);
        assert_eq!(reads.load(Ordering::SeqCst) - before, 2, "the passed-over files were read");
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 file 100");
    }

    /// **A late answer stands nowhere but its own file**: the slow read of one file lands after the cursor left it,
    /// and the pane shows only the file under the cursor.
    #[test]
    fn a_late_read_does_not_stand_on_the_next_file() {
        let (s, mut a) = files_app("late");
        std::fs::write(s.path().join("a.txt"), "AAA\n").unwrap();
        std::fs::write(s.path().join("b.txt"), "BBB\n").unwrap();
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.reading(|jail, path| {
            if path == "a.txt" {
                std::thread::sleep(Duration::from_millis(300));
            }
            files::read(jail, path)
        });
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "a.txt");
        on(&mut a, "b.txt");
        std::thread::sleep(Duration::from_millis(400));
        a.follow_files();
        let (lines, ..) = pane(&mut a, 100, 20);
        assert!(!lines.iter().any(|l| l.contains("AAA")), "{lines:?}");
        settle(&mut a);
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 BBB");
        // The late text was kept: coming back shows it at once, before any answer.
        on(&mut a, "a.txt");
        assert!(matches!(shown(&a), Shown::Text(c) if c.path == "a.txt"), "{:?}", shown(&a));
    }

    /// **A kept file is not read again while its stamp holds**, and is read again once the file changed.
    #[test]
    fn a_kept_file_is_read_again_only_when_it_changed() {
        let (s, mut a) = files_app("kept");
        let reads = reads_counted(&mut a, Duration::ZERO);
        standing_on(&mut a, "README.md");
        on(&mut a, ".env");
        settle(&mut a);
        let n = reads.load(Ordering::SeqCst);
        on(&mut a, "README.md");
        settle(&mut a);
        assert_eq!(reads.load(Ordering::SeqCst), n, "an unchanged kept file was read again");
        std::fs::write(s.path().join("README.md"), "# changed and longer\n").unwrap();
        on(&mut a, ".env");
        settle(&mut a);
        on(&mut a, "README.md");
        settle(&mut a);
        assert_eq!(pane(&mut a, 100, 20).0[0], "1 # changed and longer");
    }

    /// **The colour lands on its own letters** — on a line with Hangul, after a tab, and scrolled sideways.
    #[test]
    fn colour_stays_on_its_letters_through_hangul_tabs_and_pans() {
        let (s, mut a) = files_app("colour");
        // The third line is long enough to scroll sideways.
        let text = format!("let x = \"한글\"; // 주석\n\tlet y = 1; // 탭\n// {}\n", "w".repeat(200));
        std::fs::write(s.path().join("c.rs"), text).unwrap();
        standing_on(&mut a, "c.rs");
        let (lines, buf, inner) = pane(&mut a, 100, 20);
        assert_eq!(lines[..2], ["1 let x = \"한글\"; // 주석", "2     let y = 1; // 탭"], "{lines:?}");
        let fg = |x: u16, y: u16| buf[(inner.x + x, inner.y + y)].fg;
        // `l` of `let`, the plain `x`, `/` of the comment and its `주` — on the first line after the gutter (2 columns).
        let (kw, eq) = (fg(2, 0), fg(6, 0));
        let slash = lines[0].find("//").map(|b| crate::text::width(&lines[0][..b]) as u16).unwrap();
        let (comment, hangul) = (fg(slash, 0), fg(slash + 3, 0));
        assert_ne!(kw, eq, "the keyword is not coloured apart from a plain name");
        assert_eq!(comment, hangul, "the colour slid off the comment's Hangul");
        assert_ne!(comment, kw);
        // After the tab the keyword starts at column 4 of the text.
        assert_eq!(fg(2 + 4, 1), kw, "the tab shifted the colour");
        let Mode::Files(w) = &mut a.mode else { panic!() };
        w.right.pan = 2;
        let (_, buf, inner) = pane(&mut a, 100, 20);
        // Two columns in, the comment starts two columns earlier and `let` after the tab two earlier.
        assert_eq!(buf[(inner.x + slash - 2, inner.y)].symbol(), "/");
        assert_eq!(buf[(inner.x + slash - 2, inner.y)].fg, comment, "the pan moved the colour off the text");
        assert_eq!(buf[(inner.x + slash, inner.y)].fg, comment);
        assert_eq!(buf[(inner.x + 2 + 2, inner.y + 1)].symbol(), "l");
        assert_eq!(buf[(inner.x + 2 + 2, inner.y + 1)].fg, kw);
    }

    /// **The plain text stands first and the colour lands later without moving the scroll.**
    #[test]
    fn late_colour_leaves_the_scroll_where_it_was() {
        let (s, mut a) = files_app("paint");
        let text: String = (0..60).map(|n| format!("let value_{n} = \"{}\";\n", "x".repeat(150))).collect();
        std::fs::write(s.path().join("p.rs"), &text).unwrap();
        let (go, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.painting(move |name, text| {
            let _ = wait.lock().unwrap().recv_timeout(Duration::from_secs(10));
            crate::highlight::lines(name, text)
        });
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "p.rs");
        let began = Instant::now();
        while !matches!(shown(&a), Shown::Text(_)) {
            assert!(began.elapsed() < Duration::from_secs(5), "the plain text never came");
            a.follow_files();
            std::thread::sleep(Duration::from_millis(2));
        }
        let Shown::Text(c) = shown(&a) else { panic!() };
        assert!(c.paint.get().is_none(), "the colour came with the text");
        let _ = pane(&mut a, 100, 20);
        a.hit("Enter j j j L");
        let _ = pane(&mut a, 100, 20);
        let w = window(&a);
        let before = (w.right.page.offset(), w.right.pan);
        assert!(before.0 == 3 && before.1 > 0, "{before:?}");
        go.send(()).unwrap();
        settle(&mut a);
        let _ = pane(&mut a, 100, 20);
        let w = window(&a);
        let Shown::Text(c) = &w.right.shown else { panic!() };
        assert!(!c.paint(3).is_empty(), "the colour did not land");
        assert_eq!((w.right.page.offset(), w.right.pan), before, "the colour moved the scroll");
    }

    /// **The text never reaches the terminal raw** — an escape in a file is drawn as `?`, and the rest still stands.
    #[test]
    fn an_escape_in_the_text_is_drawn_harmless() {
        let (s, mut a) = files_app("escape");
        std::fs::write(s.path().join("esc.txt"), "a\u{1b}[2Jb\u{7}c\r\n").unwrap();
        standing_on(&mut a, "esc.txt");
        let (lines, buf, _) = pane(&mut a, 100, 20);
        assert_eq!(lines[0], "1 a?[2Jb?c");
        assert!(buf.content.iter().all(|c| !c.symbol().chars().any(char::is_control)), "a control char was drawn");
    }

    /// **The sideways wheel scrolls the text over the right pane** and nothing over the tree.
    #[test]
    fn the_sideways_wheel_scrolls_the_text() {
        use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
        let (s, mut a) = files_app("wheel");
        std::fs::write(s.path().join("wide.txt"), format!("{}\n", "w".repeat(400))).unwrap();
        standing_on(&mut a, "wide.txt");
        let _ = pane(&mut a, 100, 20);
        let d = a.drawn.files.clone().unwrap();
        let info = d.info.unwrap();
        let roll =
            |a: &mut App, kind, x, y| a.mouse(MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE });
        roll(&mut a, MouseEventKind::ScrollRight, d.rows.x + 2, d.rows.y + 1);
        assert_eq!(window(&a).right.pan, 0, "the wheel over the tree scrolled the text");
        roll(&mut a, MouseEventKind::ScrollRight, info.x + 3, info.y + 2);
        assert_eq!(window(&a).right.pan, PAN_WHEEL);
        roll(&mut a, MouseEventKind::ScrollLeft, info.x + 3, info.y + 2);
        assert_eq!(window(&a).right.pan, 0);
        assert_eq!(window(&a).focus, Side::Tree, "the wheel moved the focus");
    }

    // ---- the search (moai-ffk9.rid) ----

    /// Type text into whatever field is open — a key per letter.
    fn typed(a: &mut App, text: &str) {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};
        for c in text.chars() {
            a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    /// The paths of the entry rows, in order.
    fn paths(a: &App) -> Vec<String> {
        window(a)
            .rows()
            .iter()
            .filter_map(|r| if let Row::Entry { path, .. } = r { Some(path.clone()) } else { None })
            .collect()
    }

    /// Hang a search for `q` and take every answer.
    fn search(a: &mut App, q: &str) {
        a.hit("/");
        typed(a, q);
        a.hit("Enter");
        settle(a);
    }

    fn found(a: &App) -> Vec<String> {
        let w = window(a);
        paths(a).into_iter().filter(|p| w.found_in(p).is_some()).collect()
    }

    /// **A deep file stands and no row goes**: three folded levels down, the match opens its folders and the cursor
    /// lands on it; every row that stood before stands after.
    #[test]
    fn a_search_opens_the_folders_to_a_deep_file_and_hides_no_row() {
        let (s, mut a) = files_app("search-deep");
        std::fs::create_dir_all(s.path().join("deep/er/est")).unwrap();
        std::fs::write(s.path().join("deep/er/est/needle.txt"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        let before = paths(&a);
        assert!(!before.contains(&"deep/er".to_string()), "the premise — deep is folded");
        search(&mut a, "needle");
        let after = paths(&a);
        for p in &before {
            assert!(after.contains(p), "{p} left the tree: {after:?}");
        }
        for p in ["deep/er", "deep/er/est", "deep/er/est/needle.txt"] {
            assert!(after.contains(&p.to_string()), "{p} does not stand: {after:?}");
        }
        assert_eq!(window(&a).current_path(), Some("deep/er/est/needle.txt"), "the cursor is not on the file found");
        assert_eq!(window(&a).found_count(), (1, 1));
    }

    /// **The match is a piece of the path, case aside**: `TUI/Dr` finds `src/tui/draw.rs`; a blank query finds nothing;
    /// nothing inside `.git` is found.
    #[test]
    fn the_search_matches_the_path_case_aside_and_never_inside_git() {
        let (s, mut a) = files_app("search-match");
        std::fs::write(s.path().join("src/tui/draw.rs"), "x").unwrap();
        std::fs::write(s.path().join(".git/needle"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "TUI/Dr");
        assert_eq!(found(&a), ["src/tui/draw.rs"]);
        a.hit("Esc");
        search(&mut a, "   ");
        assert_eq!((window(&a).query(), window(&a).found_count()), (None, (0, 0)), "a blank query found something");
        assert!(matches!(a.mode, Mode::Files(_)), "Enter on a blank field closed the window");
        search(&mut a, "needle");
        assert_eq!(window(&a).found_count(), (0, 0), "a file inside .git was found");
        assert!(!paths(&a).iter().any(|p| p.contains(".git")));
    }

    /// **The mark is text, not colour**: without colour a found row reads `* name` and the rest have no mark; the
    /// matching letters of the name are reversed and the others are not; a match only in the folders above marks the
    /// row and paints nothing; a Hangul name is cut at its letters.
    #[test]
    fn a_found_row_is_marked_in_text_and_its_letters_reversed() {
        let (s, mut a) = files_app("search-mark");
        for d in ["j1", "j2"] {
            std::fs::create_dir_all(s.path().join(d)).unwrap();
            std::fs::write(s.path().join(d).join("x-key.txt"), "x").unwrap();
        }
        std::fs::write(s.path().join("x-key.md"), "x").unwrap();
        std::fs::write(s.path().join("한글파일.txt"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "x-key");
        a.hit("n");
        assert_ne!(window(&a).current_path(), Some("x-key.md"), "the premise — the cursor is off the row checked");
        let buf = draw::tests::render_buf(&mut a, 100, 24);
        // Cell by cell, so a column is a cell; the title is read from the text drawing.
        let lines: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect();
        let y = lines.iter().position(|l| l.contains("x-key.md")).expect("the found row does not stand");
        // The mark heads the row; the file's blank fold column follows, as on every file row.
        assert!(lines[y].contains("*   x-key.md"), "no mark: {:?}", lines[y]);
        let readme = lines.iter().find(|l| l.contains("README.md")).unwrap();
        assert!(!readme.contains('*'), "a row not found is marked: {readme:?}");
        let x = lines[y].find("x-key.md").map(|b| crate::text::width(&lines[y][..b]) as u16).unwrap();
        let reversed = |x: u16| buf[(x, y as u16)].modifier.contains(Modifier::REVERSED);
        assert!((x..x + 5).all(reversed), "the matching letters are not reversed");
        assert!(!(x + 5..x + 8).any(reversed), "letters past the match are reversed");
        let text = draw::tests::render(&mut a, 100, 24);
        assert!(screen_title(&text).contains("/x-key 3개 맞음"), "{:?}", screen_title(&text));
        // A match in the folder part only marks the row.
        a.hit("Esc");
        search(&mut a, "j1/");
        assert_eq!(window(&a).found_in("j1/x-key.txt"), Some(None));
        assert_eq!(window(&a).found_in("j1"), None, "the folder itself is found");
        a.hit("Esc");
        search(&mut a, "글파");
        let at = window(&a).found_in("한글파일.txt").expect("the Hangul name is not found");
        assert_eq!(at, Some("한".len().."한글파".len()));
        let _ = draw::tests::render(&mut a, 100, 24);
    }

    fn screen_title(lines: &[String]) -> String {
        lines.iter().find(|l| l.contains("파일 ·")).cloned().unwrap_or_default()
    }

    use ratatui::style::Modifier;

    /// **Typing never waits on the walk, and `Esc` answers while it runs** — the title says the index is being walked.
    #[test]
    fn typing_never_waits_on_a_slow_walk() {
        let (_s, mut a) = files_app("search-slow");
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.walking(|jail, dir, skip, take| {
            std::thread::sleep(Duration::from_secs(2));
            files::walk(jail, dir, skip, take)
        });
        a.hit("SPC g f");
        settle(&mut a);
        let began = Instant::now();
        a.hit("/");
        typed(&mut a, "main");
        a.follow_files();
        assert!(began.elapsed() < Duration::from_millis(500), "a key waited on the walk: {:?}", began.elapsed());
        assert!(a.loading(), "the loop is not told to take the walk");
        let screen = draw::tests::render(&mut a, 100, 20).join("\n");
        assert!(screen.contains("/main 0개 맞음 · 색인 0"), "{screen}");
        a.hit("Esc");
        assert!(window(&a).typing.is_none() && window(&a).query().is_none(), "Esc did not answer while walking");
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse, "the second Esc did not close the window");
        assert!(began.elapsed() < Duration::from_millis(1500), "Esc waited on the walk: {:?}", began.elapsed());
    }

    /// **At most 500 open, shallow first, and the title gives the whole count**; typing more narrows it.
    #[test]
    fn the_found_are_cut_at_five_hundred_shallow_first() {
        let (s, mut a) = files_app("search-limit");
        std::fs::create_dir_all(s.path().join("deep")).unwrap();
        for n in 0..600 {
            std::fs::write(s.path().join(format!("deep/hit{n:03}.txt")), "").unwrap();
        }
        std::fs::write(s.path().join("hit-top.txt"), "").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "hit");
        assert_eq!(window(&a).found_count(), (FOUND_LIMIT, 601));
        assert!(window(&a).found_in("hit-top.txt").is_some(), "the file at the root was crowded out");
        assert_eq!(found(&a).len(), FOUND_LIMIT, "more than the limit are marked");
        let lines = draw::tests::render(&mut a, 120, 20);
        assert!(screen_title(&lines).contains("601개 중 500개"), "{:?}", screen_title(&lines));
        a.hit("Esc");
        search(&mut a, "hit-");
        assert_eq!(window(&a).found_count(), (1, 1), "typing more did not narrow");
    }

    /// **The ignored directories are skipped by the search and still stand**; stepped into, one is searched; without
    /// git nothing is skipped and the title says so; the ignored list is asked once per index, never per key.
    #[test]
    fn the_search_skips_ignored_directories_unless_stepped_into() {
        let (_s, mut a) = files_app("search-ignored");
        let asked = Arc::new(AtomicUsize::new(0));
        let n = asked.clone();
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.ignoring(move |_| {
            n.fetch_add(1, Ordering::SeqCst);
            Some(vec!["target/".to_string()])
        });
        a.hit("SPC g f");
        settle(&mut a);
        let opened = asked.load(Ordering::SeqCst);
        search(&mut a, "out");
        assert_eq!(window(&a).found_count(), (0, 0), "a file inside the ignored target/ was found");
        assert!(paths(&a).contains(&"target".to_string()), "the ignored row left the tree");
        assert_eq!(asked.load(Ordering::SeqCst), opened + 1, "the ignored list was asked per key");
        a.hit("Esc");
        search(&mut a, "debug");
        assert_eq!(asked.load(Ordering::SeqCst), opened + 1, "the same root asked again");
        a.hit("Esc");
        on(&mut a, "target");
        a.hit("Enter");
        settle(&mut a);
        search(&mut a, "out");
        assert_eq!(found(&a), ["target/debug/out"], "inside target/ the search found nothing");
        assert_eq!(asked.load(Ordering::SeqCst), opened + 2);
        // Without git: nothing skipped, and the title says it.
        a.hit("Esc Esc");
        a.files_disk = Disk::with(files::list, |_| None);
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "out");
        assert_eq!(found(&a), ["target/debug/out"], "without git the search failed");
        let lines = draw::tests::render(&mut a, 120, 20);
        assert!(screen_title(&lines).contains("건너뛰는 것 없음"), "{:?}", screen_title(&lines));
    }

    /// **`n`·`N` go round the found rows in the tree's order** — the cursor lands only on rows found.
    #[test]
    fn n_and_shift_n_go_round_the_found_rows() {
        let (s, mut a) = files_app("search-jump");
        for d in ["j1", "j2"] {
            std::fs::create_dir_all(s.path().join(d)).unwrap();
            std::fs::write(s.path().join(d).join("x-key.txt"), "x").unwrap();
        }
        std::fs::write(s.path().join("x-key.md"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "x-key");
        let at = |a: &App| window(a).current_path().map(str::to_owned);
        assert_eq!(at(&a).as_deref(), Some("x-key.md"), "the cursor is not on the first found (the shallowest)");
        let mut went = Vec::new();
        for _ in 0..3 {
            a.hit("n");
            went.push(at(&a).unwrap());
        }
        assert_eq!(went, ["j1/x-key.txt", "j2/x-key.txt", "x-key.md"], "n did not go round");
        a.hit("N");
        assert_eq!(at(&a).as_deref(), Some("j2/x-key.txt"), "N did not go back");
        a.hit("g g N");
        assert_eq!(at(&a).as_deref(), Some("x-key.md"), "N from the top did not go round to the last");
    }

    /// **Ending the search folds back what it opened and leaves what the hand opened** — `Esc` ends it before it closes
    /// the window; a folder the search opened that the hand folded and opened again is the hand's.
    #[test]
    fn esc_ends_the_search_first_and_folds_only_what_it_opened() {
        let (s, mut a) = files_app("search-esc");
        for d in ["j1", "j2", "j3"] {
            std::fs::create_dir_all(s.path().join(d)).unwrap();
            std::fs::write(s.path().join(d).join("x-key.txt"), "x").unwrap();
        }
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "j3");
        a.hit("l");
        settle(&mut a);
        search(&mut a, "x-key");
        on(&mut a, "j1");
        a.hit("h l");
        settle(&mut a);
        let w = window(&a);
        assert!(w.is_open("j1") && w.is_open("j2") && w.is_open("j3"), "the premise");
        on(&mut a, "j2/x-key.txt");
        a.hit("Esc");
        let w = window(&a);
        assert!(matches!(a.mode, Mode::Files(_)), "one Esc closed the window over a search");
        assert!(!w.is_open("j2"), "the folder the search opened is still open");
        assert!(w.is_open("j1") && w.is_open("j3"), "a folder the hand opened was folded");
        assert_eq!(w.current_path(), Some("j2"), "the cursor did not climb to the folder that folded");
        assert!(w.found_in("j1/x-key.txt").is_none(), "the marks stayed");
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
    }

    /// **`Esc` in the field drops what was typed** and goes back to the search before, the cursor where it stood.
    #[test]
    fn esc_in_the_field_goes_back_to_the_search_before() {
        let (s, mut a) = files_app("search-cancel");
        std::fs::write(s.path().join("x-key.md"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "x-key");
        on(&mut a, "README.md");
        a.hit("/");
        assert_eq!(window(&a).typing.as_ref().map(|t| t.input.text()), Some("x-key"), "the field is not the search");
        typed(&mut a, "zzz");
        settle(&mut a);
        assert_eq!(window(&a).found_count(), (0, 0));
        a.hit("Esc");
        settle(&mut a);
        let w = window(&a);
        assert_eq!((w.query(), w.found_count()), (Some("x-key"), (1, 1)));
        assert_eq!(w.current_path(), Some("README.md"), "the cursor did not come back");
    }

    /// **A new root walks a new index** — stepped into a folder, the search does not answer from the old root's.
    #[test]
    fn a_stale_index_is_not_searched_after_stepping_in() {
        let (s, mut a) = files_app("search-root");
        std::fs::create_dir_all(s.path().join("other")).unwrap();
        std::fs::write(s.path().join("other/readme.txt"), "x").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "main");
        assert_eq!(found(&a), ["src/main.rs"]);
        a.hit("Esc");
        on(&mut a, "other");
        a.hit("Enter");
        settle(&mut a);
        search(&mut a, "readme");
        assert_eq!(found(&a), ["other/readme.txt"], "the old root's index answered");
        assert_eq!(window(&a).found_count(), (1, 1));
        // A search hung while stepping walks again for the new root.
        a.hit("Bksp");
        settle(&mut a);
        assert_eq!(window(&a).found_count(), (2, 2), "stepping out did not walk the root again");
    }

    /// **A file the index named but its folder does not hold is left out without a word.**
    #[test]
    fn a_file_gone_since_the_walk_is_left_out() {
        let (s, mut a) = files_app("search-gone");
        std::fs::create_dir_all(s.path().join("ghost")).unwrap();
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.walking(|_, _, _, take| {
            let _ = take(&["ghost".to_string(), "ghost/nothere.txt".to_string()]);
            Ok(files::Walked { seen: 2, truncated: false })
        });
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "nothere");
        assert_eq!(window(&a).found_count(), (0, 0));
        assert!(a.notice.is_none(), "{:?}", a.notice);
    }

    /// **The index stops at the read layer's limit and says so.**
    #[test]
    fn an_index_at_its_limit_says_so() {
        let (_s, mut a) = files_app("search-cut");
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.walking(|_, _, _, take| {
            let _ = take(&["README.md".to_string()]);
            Ok(files::Walked { seen: 200_000, truncated: true })
        });
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "read");
        let notice = a.notice.clone().unwrap_or_default();
        assert!(notice.contains("200000"), "{notice:?}");
        assert_eq!(found(&a), ["README.md"]);
    }

    /// **The search's folders go out a few at a time** — not five hundred reads at once on the reader.
    #[test]
    fn the_search_reads_its_folders_a_few_at_a_time() {
        let (s, mut a) = files_app("search-pump");
        for n in 0..40 {
            std::fs::create_dir_all(s.path().join(format!("d{n:02}"))).unwrap();
            std::fs::write(s.path().join(format!("d{n:02}/hit.txt")), "").unwrap();
        }
        a.hit("SPC g f");
        settle(&mut a);
        let disk = std::mem::take(&mut a.files_disk);
        // The window is open; the next one reads its folders slowly.
        a.hit("Esc");
        a.files_disk = Disk::with(
            |jail, path| {
                if path.starts_with('d') {
                    std::thread::sleep(Duration::from_millis(20));
                }
                files::list(jail, path)
            },
            |_| Some(Vec::new()),
        );
        drop(disk);
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("/");
        typed(&mut a, "hit");
        // The index comes in a batch or two; take it and look at what is out.
        let began = Instant::now();
        while window(&a).found_count().1 < 40 {
            assert!(began.elapsed() < Duration::from_secs(10), "the index never came");
            a.follow_files();
            let w = window(&a);
            let out = w.search.opened.iter().filter(|d| w.dirs.get(*d).is_some_and(|x| x.read.is_none())).count();
            assert!(out <= SEARCH_READS, "{out} folders out at once");
            std::thread::sleep(Duration::from_millis(1));
        }
        a.hit("Enter");
        settle(&mut a);
        assert_eq!(found(&a).len(), 40);
    }

    /// **The field lets the mouse go and takes a paste**; the hung search keeps the mouse.
    #[test]
    fn the_field_lets_the_mouse_go_and_takes_a_paste() {
        let (_s, mut a) = files_app("search-mouse");
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("/");
        assert!(!a.wants_mouse(), "the field holds the mouse");
        a.paste("mai");
        typed(&mut a, "n");
        a.hit("Enter");
        settle(&mut a);
        assert!(a.wants_mouse());
        assert_eq!(window(&a).query(), Some("main"));
        a.paste("x");
        assert_eq!(window(&a).query(), Some("main"), "a paste with no field changed the search");
        let lines = draw::tests::render(&mut a, 120, 20);
        let bar = lines.last().unwrap();
        assert!(bar.contains("n·N 다음/이전") && bar.contains("Esc 찾기 끝"), "{bar:?}");
    }
}
