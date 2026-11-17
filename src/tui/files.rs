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
//!
//! **It refreshes itself, measuring only what is seen** (moai-ffk9.15v). At the refresh's pace ([`super::pace`]) the
//! lister takes the stamp of every open folder the tree shows — [`SWEEP_LIMIT`] at most per look, the rest on the next
//! ones in turn — and reads again only those whose stamp moved; the reader checks the file the text pane shows. The
//! sweep goes in [`SWEEP_PIECE`]-folder pieces and lets the person's reads go first between them. A folder read again
//! is merged by path: the open folders, the cursor and the scroll stay; a folder gone leaves the tree without a word.
//! While the changed files stand (`SPC v c`) their list is asked of git again at its own pace, and the diff pane reads
//! itself again ([`diff::View::refresh`]); the window's git runs one at a time behind one gate ([`Disk::gated`]), so a
//! refresh and the person's read never run two gits together. A stamp is taken before what it stamps is read, and one
//! within [`RACY`] of the read is not trusted — the next look reads it again (git's "racy" rule): two changes inside one
//! tick of the file system's clock leave the stamp alone. Nothing of it wakes the loop early or says a word, and a
//! look that found nothing draws nothing.

use super::diff;
use super::input::Input;
use super::keys::{self, Browse, LEADER, Lookup, PROMPT, Prompt, Side as Toward};
use super::pace::{self, Clock, Pace};
use super::scroll::{self, Move, Scroll};
use super::{App, Mode, Row as ListRow, Seat, menu};
use crate::files::{self, Entry, Jail, Kind, Why};
use crate::i18n::{Lang, fill, say};
use crate::nav::Twig;
use crate::report::changes::Checkout;
use crate::vcs;
use ratatui::crossterm::event::KeyEvent;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as Atomic};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;
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

/// Folders one refresh look stamps at most — more open, the next looks go on from where it stopped (moai-ffk9.15v).
pub(super) const SWEEP_LIMIT: usize = 2_000;

/// Folders the sweep stamps between two looks at the person's requests — a folder the hand opens waits for this many
/// stamps at most, not for the whole sweep.
const SWEEP_PIECE: usize = 32;

/// A stamp this close to the moment it was taken is not trusted: the file system's clock ticks in seconds on some
/// disks, and a second change inside the tick leaves the stamp as it was. Such a folder or file is read again on the
/// next look.
const RACY: Duration = Duration::from_secs(2);

/// Whether a stamp taken now may still hide a change ([`RACY`]).
fn racy(stamp: &files::Stamp) -> bool {
    stamp.modified.is_some_and(|m| SystemTime::now().duration_since(m).map_or(true, |d| d < RACY))
}

/// The folders one sweep stamps: all of them while they are few, else [`SWEEP_LIMIT`] going on after `after` and round
/// the end — so every open folder is stamped within a few looks. Gives them and where the next look goes on.
fn turn(open: &[&str], after: Option<&str>, limit: usize) -> (Vec<String>, Option<String>) {
    if open.len() <= limit {
        return (open.iter().map(|p| (*p).to_owned()).collect(), None);
    }
    let start = after.map_or(0, |a| open.partition_point(|p| *p <= a));
    let picked: Vec<String> = (0..limit).map(|k| open[(start + k) % open.len()].to_owned()).collect();
    let next = picked.last().cloned();
    (picked, next)
}

type ListFn = dyn Fn(&Jail, &str) -> Result<files::Listed, Why> + Send + Sync;
type IgnoredFn = dyn Fn(&Path) -> Option<Vec<String>> + Send + Sync;
type ReadFn = dyn Fn(&Jail, &str) -> Result<files::Text, Why> + Send + Sync;
type StampFn = dyn Fn(&Jail, &str) -> Option<files::Stamp> + Send + Sync;
type PaintFn = dyn Fn(&str, &str) -> Option<Vec<Vec<crate::highlight::Span>>> + Send + Sync;
type WalkFn =
    dyn Fn(&Jail, &str, &[String], &mut dyn FnMut(&[String]) -> bool) -> Result<files::Walked, Why> + Send + Sync;
type PlacesFn = dyn Fn(&Path) -> Places + Send + Sync;
type ChangesFn = dyn Fn(&Checkout, Lang) -> Result<vcs::Changes, String> + Send + Sync;
type DiffReadFn = diff::ReadFn;

/// How long the changed-files list may take — git's budget, as the diff pane's ([`diff::BUDGET`]).
const CHANGES_BUDGET: Duration = diff::BUDGET;

/// The checkouts the picker offers (moai-ffk9.211) and the worktree marks the detail's issue is found by
/// (`report::changes::issue_of`) — read on a thread: both read git's files.
#[derive(Debug, Clone, Default)]
pub struct Places {
    /// `worktree::checkouts` — the main checkout first, then the linked ones, the milestone worktrees among them.
    /// **Only these can be picked**: there is no way to hand the window a path of one's own.
    pub checkouts: Vec<Checkout>,
    pub workplaces: Vec<crate::report::Workplace>,
    /// Which of them the window was opened on — measured on the thread (the paths are compared resolved).
    pub here: Option<usize>,
}

/// Read the places for the checkout at `here`: its main checkout's list, and which one `here` is.
fn places(here: &Path) -> Places {
    let checkouts = crate::worktree::checkouts(here);
    let real = crate::path::canonical(here).ok();
    let at = checkouts.iter().position(|c| Some(&c.path) == real.as_ref() || c.path == here);
    Places { workplaces: crate::worktree::workplaces(here, true), checkouts, here: at }
}

/// The changed files of a checkout's whole branch — fork point to the working tree, untracked ones with it. The
/// main checkout has no fork point and gives what is not committed (`vcs::Scope::Whole`).
fn changes(c: &Checkout, lang: Lang) -> Result<vcs::Changes, String> {
    let root = c.path.as_path();
    let Some(git) = vcs::detect(root) else { return Err(say(lang, "tui.diff.why.not_a_repo").to_string()) };
    git.changes(root, &vcs::Scope::Whole, Some(CHANGES_BUDGET)).map_err(|e| diff::told(root, lang, e).said)
}

/// What the window reads with — the real disk, or a stand-in a test puts in (a slow reader, a failing git).
#[derive(Clone)]
pub struct Disk {
    list: Arc<ListFn>,
    ignored: Arc<IgnoredFn>,
    read: Arc<ReadFn>,
    stamp: Arc<StampFn>,
    paint: Arc<PaintFn>,
    walk: Arc<WalkFn>,
    places: Arc<PlacesFn>,
    changes: Arc<ChangesFn>,
    diff_read: Arc<DiffReadFn>,
    diff_paint: Arc<diff::PaintFn>,
    /// What time it is — the refresh's pace and the sweep's measure (moai-ffk9.15v).
    clock: Clock,
}

impl Disk {
    /// The same disk with its git calls — the changed-files list and the diff pane's read — behind one gate of their
    /// own (moai-ffk9.15v): a refresh's `git` and the person's never run together, the later one waits on its thread.
    /// Each window takes its own gate; the loop never waits on it.
    fn gated(&self) -> Disk {
        let gate = Arc::new(Mutex::new(()));
        let (changes, g) = (self.changes.clone(), gate.clone());
        let (read, h) = (self.diff_read.clone(), gate);
        Disk {
            changes: Arc::new(move |c: &Checkout, lang: Lang| {
                let _held = g.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                changes(c, lang)
            }),
            diff_read: Arc::new(
                move |s: &diff::Source,
                      scope: Option<crate::report::changes::Scope>,
                      budget: Option<Duration>,
                      lang| {
                    let _held = h.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    read(s, scope, budget, lang)
                },
            ),
            ..self.clone()
        }
    }
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
            places: Arc::new(places),
            changes: Arc::new(changes),
            diff_read: Arc::new(diff::read),
            diff_paint: Arc::new(crate::highlight::diff),
            clock: pace::real(),
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
        // No git behind a test's checkout: it is the only one, and nothing in it changed — the tests that read either
        // put their own in ([`Disk::placing`], [`Disk::changing`]).
        let places = |root: &Path| Places {
            checkouts: vec![Checkout {
                key: ".".into(),
                path: root.to_path_buf(),
                branch: "main".into(),
                main: true,
                milestone: None,
            }],
            workplaces: Vec::new(),
            here: Some(0),
        };
        Disk {
            list: Arc::new(list),
            ignored: Arc::new(ignored),
            places: Arc::new(places),
            changes: Arc::new(|_: &Checkout, _: Lang| Ok(vcs::Changes::default())),
            ..Disk::default()
        }
    }

    /// The same disk with the checkout list replaced.
    pub(super) fn placing(self, places: impl Fn(&Path) -> Places + Send + Sync + 'static) -> Disk {
        Disk { places: Arc::new(places), ..self }
    }

    /// The same disk with the changed-files list replaced — a test slows it down or names what changed.
    pub(super) fn changing(
        self,
        changes: impl Fn(&Checkout, Lang) -> Result<vcs::Changes, String> + Send + Sync + 'static,
    ) -> Disk {
        Disk { changes: Arc::new(changes), ..self }
    }

    /// The same disk with the diff pane's reader replaced.
    pub(super) fn diffing(
        self,
        read: impl Fn(&diff::Source, Option<crate::report::changes::Scope>, Option<Duration>, Lang) -> diff::Loaded
        + Send
        + Sync
        + 'static,
    ) -> Disk {
        Disk { diff_read: Arc::new(read), diff_paint: Arc::new(|_: &crate::diff::FileDiff| None), ..self }
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

    /// The same disk with its stamp taker replaced — a test counts the stamps or moves them.
    pub(super) fn stamping(self, stamp: impl Fn(&Jail, &str) -> Option<files::Stamp> + Send + Sync + 'static) -> Disk {
        Disk { stamp: Arc::new(stamp), ..self }
    }

    /// The same disk on a hand-moved clock.
    pub(super) fn clocking(self, clock: Clock) -> Disk {
        Disk { clock, ..self }
    }
}

/// The pane holding the focus. The tree is always on the left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    /// The file tree. **The window starts here.**
    #[default]
    Tree,
    /// The right pane — what the cursor stands on: the file's text, or its diff while `SPC v D` is on.
    Info,
    /// The checkout's issue (moai-ffk9.211) — above or left of the diff while both are on.
    Detail,
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
    /// Git could not list the changed files — its words (moai-ffk9.211).
    Changes(String),
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
            Trouble::Changes(d) => crate::text::one_line(d),
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
        /// The stamp was too fresh to trust ([`RACY`]) — the refresh reads the folder again whatever its stamp says.
        racy: bool,
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
    /// A refresh look (moai-ffk9.15v): stamp these folders and read again the ones that moved.
    Sweep(Sweep),
}

/// A refresh look on the lister — taken in pieces of [`SWEEP_PIECE`], the person's requests going first between them.
struct Sweep {
    id: u64,
    /// Each open folder with the generation it was read under, its stamp then and whether that stamp was too fresh.
    dirs: VecDeque<(String, u64, files::Stamp, bool)>,
    /// The folders read again so far.
    listed: Vec<(String, u64, Read)>,
    /// The time spent stamping and reading — not the time it waited for the person's reads.
    took: Duration,
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
    /// The checkouts to pick from (moai-ffk9.211).
    Places {
        id: u64,
        places: Places,
    },
    /// The checkout's changed files (moai-ffk9.211), and how long git took (moai-ffk9.15v).
    Changes {
        id: u64,
        got: Result<vcs::Changes, String>,
        took: Duration,
    },
    /// A read found the checkout's root gone — a worktree removed while the window stood.
    Gone,
    /// A refresh look ended: the folders that moved, read again (moai-ffk9.15v).
    Swept {
        id: u64,
        listed: Vec<(String, u64, Read)>,
        took: Duration,
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

/// A file to read — with the copy the window keeps, if any: the reader then only checks its stamp (the refresh reads
/// again a copy whose stamp was too fresh to trust, [`RACY`]).
#[derive(Debug, Clone)]
struct Look {
    path: String,
    was: Option<Arc<Content>>,
    /// The refresh's look at the text shown (moai-ffk9.15v) — coloured before it is sent, so it never flashes plain.
    refresh: bool,
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
    /// The stamp was too fresh to trust when read ([`RACY`]) — the reader clears it once a later read finds the same.
    racy: AtomicBool,
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
        let fresh = racy(&t.stamp);
        Content {
            path,
            stamp: t.stamp,
            text: t.text,
            lines,
            widths,
            widest,
            size: t.size,
            truncated: t.truncated,
            racy: AtomicBool::new(fresh),
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
    let Look { path, was, refresh } = l;
    let Some(jail) = jail else {
        return out.send(Reply::Opened { path, got: Got::Failed(Trouble::Missing) }).is_ok();
    };
    // A stamp too fresh to trust is read again by the refresh's look; the cursor coming back trusts it — the refresh
    // looks at it on its next step anyway.
    if let Some(was) = &was
        && !(refresh && was.racy.load(Atomic::Relaxed))
        && (disk.stamp)(jail, &path).as_ref() == Some(&was.stamp)
    {
        return out.send(Reply::Opened { path, got: Got::Same }).is_ok();
    }
    match (disk.read)(jail, &path) {
        Ok(t) => {
            // The same text: a fresh stamp read again, or a file only touched — nothing to draw, nothing to colour.
            if let Some(was) = was.as_ref().filter(|w| w.text == t.text && w.truncated == t.truncated) {
                if was.stamp == t.stamp {
                    was.racy.store(racy(&t.stamp), Atomic::Relaxed);
                    return out.send(Reply::Opened { path, got: Got::Same }).is_ok();
                }
                let c = Content::of(path.clone(), t);
                if let Some(p) = was.paint.get() {
                    let _ = c.paint.set(p.clone());
                }
                let c = Arc::new(c);
                return out.send(Reply::Opened { path: path.clone(), got: Got::Text(c) }).is_ok()
                    && out.send(Reply::Painted { path }).is_ok();
            }
            let c = Arc::new(Content::of(path.clone(), t));
            if refresh {
                let _ = c.paint.set((disk.paint)(&path, &c.text));
            }
            if out.send(Reply::Opened { path: path.clone(), got: Got::Text(c.clone()) }).is_err() {
                return false;
            }
            if !refresh {
                let _ = c.paint.set((disk.paint)(&path, &c.text));
            }
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
    /// One-off reads — the checkout list, the changed files — each on a thread of its own, reaped as they end.
    probes: Vec<std::thread::JoinHandle<()>>,
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
            let racy = racy(&listed.stamp);
            Read::Listed { entries: listed.entries, stamp: listed.stamp, links, racy }
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
        let (stamp, clock) = (disk.stamp.clone(), disk.clock.clone());
        let at = checkout.clone();
        let lister = std::thread::spawn(move || {
            let jail = Jail::of(&at);
            // Requests taken while a sweep was cut short — they go before the rest of it.
            let mut backlog: VecDeque<Request> = VecDeque::new();
            loop {
                let request = match backlog.pop_front() {
                    Some(r) => r,
                    None => match incoming.recv() {
                        Ok(r) => r,
                        Err(_) => return,
                    },
                };
                match request {
                    Request::Sweep(mut sweep) => {
                        let began = clock();
                        for _ in 0..SWEEP_PIECE {
                            let Some((path, generation, known, fresh)) = sweep.dirs.pop_front() else { break };
                            // Stamped first, read after: a change between the two shows on the next look.
                            let now = jail.as_ref().and_then(|j| stamp(j, &path));
                            if !fresh && now.as_ref() == Some(&known) {
                                continue;
                            }
                            let read = read_dir(&*list, jail.as_ref(), &path);
                            if path == "."
                                && matches!(read, Read::Failed(Trouble::Missing))
                                && !at.path.is_dir()
                                && outgoing.send(Reply::Gone).is_err()
                            {
                                return;
                            }
                            sweep.listed.push((path, generation, read));
                        }
                        sweep.took += clock().saturating_duration_since(began);
                        if sweep.dirs.is_empty() {
                            let Sweep { id, listed, took, .. } = sweep;
                            if outgoing.send(Reply::Swept { id, listed, took }).is_err() {
                                return;
                            }
                            continue;
                        }
                        // **The person's reads go first** — what came in while this piece ran is read before the next.
                        // The window gone, the sweep ends here.
                        loop {
                            match incoming.try_recv() {
                                Ok(r) => backlog.push_back(r),
                                Err(TryRecvError::Empty) => break,
                                Err(TryRecvError::Disconnected) => return,
                            }
                        }
                        backlog.push_back(Request::Sweep(sweep));
                    }
                    Request::List { path, generation } => {
                        let read = read_dir(&*list, jail.as_ref(), &path);
                        // A missing folder in a checkout that is itself gone: the window goes back to the main one.
                        if matches!(read, Read::Failed(Trouble::Missing))
                            && !at.path.is_dir()
                            && outgoing.send(Reply::Gone).is_err()
                        {
                            return;
                        }
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
            probes: Vec::new(),
        }
    }

    /// Read the checkouts to pick from, for the window opened at `here`.
    fn places(&mut self, id: u64, here: std::path::PathBuf) {
        let (read, out) = (self.disk.places.clone(), self.outgoing.clone());
        self.probes.push(std::thread::spawn(move || {
            let _ = out.send(Reply::Places { id, places: read(&here) });
        }));
    }

    /// List the checkout's changed files.
    fn changes(&mut self, id: u64, lang: Lang) {
        let (read, out, at) = (self.disk.changes.clone(), self.outgoing.clone(), self.checkout.clone());
        let clock = self.disk.clock.clone();
        self.probes.push(std::thread::spawn(move || {
            let began = clock();
            let got = read(&at, lang);
            let took = clock().saturating_duration_since(began);
            let _ = out.send(Reply::Changes { id, got, took });
        }));
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
        // A one-off read ends on its own once it sent; joining raises its panic if it died before.
        let mut i = 0;
        while i < self.probes.len() {
            if self.probes[i].is_finished() {
                rethrow(self.probes.swap_remove(i));
            } else {
                i += 1;
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
        /// Where it hangs in the tree — the list's twig shape (moai-p7nr.xj3); its depth is the row's.
        twig: Twig,
        entry: Entry,
        /// An opened directory.
        open: bool,
        /// Git ignores it or a directory above it.
        ignored: bool,
        /// What a link holds, when it could be read.
        link: Option<String>,
        /// How the file changed — only in the changed-files tree (`SPC v c`, moai-ffk9.211).
        change: Option<vcs::Change>,
    },
    /// The directory `of` is still being read — the last of that directory's rows.
    Loading { twig: Twig, of: String },
    /// The directory `of` could not be read — the last of that directory's rows.
    Failed { twig: Twig, of: String, why: Trouble },
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
            Row::Entry { twig, .. } | Row::Loading { twig, .. } | Row::Failed { twig, .. } => twig.depth(),
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
    /// The read out is the refresh's look at the text shown (moai-ffk9.15v) — alone, it does not wake the loop.
    refreshing: bool,
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
            refreshing: false,
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

    /// Whether a read is out or due — the loop wakes fast to take it. The refresh's look alone does not count; a want
    /// waiting behind it does.
    fn busy(&self) -> bool {
        (self.running.is_some() && (!self.refreshing || self.wanted.is_some()))
            || matches!(self.shown, Shown::Reading { said: false, .. })
    }
}

/// Whether the right pane reads this entry as a file — a regular file, or a link to one inside the checkout, with a
/// name the jail can open. The rest is told of, not read.
pub fn reads(e: &Entry) -> bool {
    e.openable && matches!(e.kind, Kind::File | Kind::Link { dir: false, inside: true })
}

/// Whether the right pane reads this row's text — an entry it [`reads`], unless the changed tree lists it as deleted:
/// that file is not on this checkout, so the pane says so in a word instead of failing on the read (moai-ffk9.9m0).
pub fn shows_text(r: &Row) -> bool {
    matches!(r, Row::Entry { entry, change, .. } if reads(entry) && !deleted(change.as_ref()))
}

/// The changed tree lists this file as deleted on the checkout.
pub fn deleted(change: Option<&vcs::Change>) -> bool {
    change.is_some_and(|c| c.status == vcs::Status::Deleted)
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
    /// The refresh read a folder again since the walk (moai-ffk9.15v) — the next search walks afresh. What the search
    /// found stays as it is till then.
    stale: bool,
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

/// The changed-files list as the window holds it (moai-ffk9.211).
#[derive(Debug, Clone)]
pub enum Listing {
    Reading,
    Read(Arc<vcs::Changes>),
    Failed(String),
}

/// "Changed files only" — `SPC v c` (moai-ffk9.211). **The tree beneath is left as it was**: the folders the hand
/// opened, the place stepped into and the search stay in [`Window`] and stand again when it is turned off. The changed
/// tree is built from the list's paths alone — no folder is read, and a deleted file has none to read.
#[derive(Debug, Clone)]
struct Changed {
    on: bool,
    /// The list — read when the window opens, when this is turned on and when the checkout is picked. Kept while a
    /// new read is out, so turning it on again does not blank the tree.
    list: Listing,
    /// The read out — a reply of another is dropped.
    id: u64,
    /// The read out is the refresh's (moai-ffk9.15v): it does not wake the loop, a failure keeps the list held, and the
    /// same list draws nothing.
    refreshing: bool,
    /// The cursor and the tree's scroll before it was turned on — turning it off stands there again unless the file
    /// the cursor stood on in the changed tree stands in the whole one.
    before: Option<(At, usize)>,
    /// Turning it on stood the diff pane up (moai-p7nr.sw0) — turning it off takes the pane down again. A hand on the
    /// pane in between (`SPC v D`, `moai ctl view diff`) drops it: the pane is the person's from then on. The window's
    /// own, like the pane: never in the config, gone with the window.
    raised_diff: bool,
}

/// The checkout picker — `SPC v w` over the window (moai-ffk9.211). The list is [`Window::places`], read afresh as
/// the picker opens; until it comes the picker says it is reading.
#[derive(Debug, Clone, Default)]
pub struct Pick {
    pub cursor: usize,
    pub list: Scroll,
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
    /// The checkout was gone and the window went back to the main one — its key, for the notice.
    pub gone: Option<String>,
}

/// The automatic refresh's state (moai-ffk9.15v): the stamp sweep's pace and the one out, where the next sweep goes on,
/// and the changed-files list's pace — git's own, counted apart.
#[derive(Debug, Clone)]
struct Refresh {
    pace: Pace,
    sweep: Option<u64>,
    from: Option<String>,
    git: Pace,
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
    // ---- moai-ffk9.211 ----
    /// What the window reads with — kept to start a worker for another checkout.
    disk: Disk,
    lang: Lang,
    /// Whose issues the detail reads — the project the window opened on (`App::site_of_seat`).
    pub seat: Seat,
    /// Where the window was opened — the checkout list is read from there, whichever checkout is picked since.
    opened_at: std::path::PathBuf,
    /// The checkouts and the worktree marks — `None` until the first read came.
    pub places: Option<Places>,
    /// The checkout list read out, if any.
    places_id: u64,
    /// The checkout picker, while open.
    pub pick: Option<Pick>,
    changed: Changed,
    /// The right pane's detail and diff (`SPC v d`·`SPC v D` over the window). **The window's own**: the list's
    /// detail and diff settings are not touched, nothing is kept in the config, and closing the window forgets them.
    pub detail_on: bool,
    pub diff_on: bool,
    pub diff: diff::View,
    /// The detail's scroll.
    pub detail: Scroll,
    /// Both are on and the right pane is too small for two — the detail stands alone and the diff folds (the
    /// person's answer). Set by the last frame; the toggles keep their values.
    pub folded: bool,
    /// The window is too narrow for any right pane — the tree stands alone, and the diff, on or not, is not drawn.
    /// Set by the last frame, like `folded`.
    narrow: bool,
    refresh: Refresh,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("files::Window")
            .field("project", &self.project)
            .field("here", &self.here)
            .field("at", &self.at)
            .field("rows", &self.rows.len())
            .field("checkout", &self.checkout.key)
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
    pub(super) fn new(
        project: String,
        checkout: Checkout,
        keys: keys::Keymap,
        disk: &Disk,
        seat: Seat,
        lang: Lang,
    ) -> Window {
        let disk = &disk.gated();
        let worker = Shared(Arc::new(Mutex::new(Worker::start(&checkout, disk))));
        let opened_at = checkout.path.clone();
        let diff = diff::View::with(disk.diff_read.clone(), disk.diff_paint.clone(), lang).clocked(disk.clock.clone());
        let now = (disk.clock)();
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
            disk: disk.clone(),
            lang,
            seat,
            opened_at,
            places: None,
            places_id: 0,
            pick: None,
            changed: Changed {
                on: false,
                list: Listing::Reading,
                id: 0,
                refreshing: false,
                before: None,
                raised_diff: false,
            },
            detail_on: false,
            diff_on: false,
            diff,
            detail: Scroll::default(),
            folded: false,
            narrow: false,
            refresh: Refresh { pace: Pace::new(now), sweep: None, from: None, git: Pace::new(now) },
        };
        w.open(".");
        w.rebuild();
        w.ask_places();
        w.ask_changes(false);
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

    pub fn is_open(&self, path: &str) -> bool {
        self.dirs.contains_key(path)
    }

    /// Something is still out: a directory read, the ignored list, a spread.
    pub fn waiting(&self) -> bool {
        self.places_id != 0
            || (self.changed.id != 0 && !self.changed.refreshing)
            || self.diff.busy()
            || self.spread.is_some()
            || self.search.index.as_ref().is_some_and(|i| i.walking)
            || self.ignored == Ignored::Waiting
            || self.dirs.values().any(|d| d.read.is_none())
            || self.right.busy()
    }

    /// Bring the right pane to the row under the cursor — called wherever the cursor lands ([`Window::move_to`],
    /// [`Window::rebuild`]). The same file stays as it is, scroll and all; another file stands at its top and left
    /// edge, a kept copy at once, and is asked for.
    fn aim(&mut self) {
        self.aim_diff();
        let path = match self.current() {
            Some(r @ Row::Entry { path, .. }) if shows_text(r) => path.clone(),
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
        self.right.shown = match &kept {
            Some(c) => Shown::Text(c.clone()),
            None => Shown::Reading { path: path.clone(), since: Instant::now(), said: false },
        };
        // The file being read now needs no second read — its answer lands on the pane.
        self.right.wanted =
            (self.right.running.as_deref() != Some(path.as_str())).then_some(Look { path, was: kept, refresh: false });
        self.send_look();
    }

    /// Send the wanted read when none is out.
    fn send_look(&mut self) {
        if self.right.running.is_some() {
            return;
        }
        let Some(l) = self.right.wanted.take() else { return };
        self.right.running = Some(l.path.clone());
        self.right.refreshing = l.refresh;
        self.worker.with(|w| w.look(l));
    }

    /// A read ended: the next wanted goes out.
    fn read_done(&mut self, path: &str) {
        if self.right.running.as_deref() == Some(path) {
            self.right.running = None;
            self.right.refreshing = false;
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
        if self.changed.on {
            return self.rebuild_changed();
        }
        let mut rows = Vec::new();
        if self.here != "." {
            rows.push(Row::Up);
        }
        let above = self.here.split('/').scan(String::new(), |acc, part| {
            *acc = if acc.is_empty() { part.to_owned() } else { format!("{acc}/{part}") };
            Some(acc.clone())
        });
        let ignored = self.here != "." && above.into_iter().any(|p| self.ignored_path(&p));
        self.flatten(&self.here.clone(), &[], ignored, &mut rows);
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

    /// The rows under `dir`. `kin` is the twig of the directory's own row — empty for the directory the tree stands
    /// in — and each row under it adds whether a sibling follows it, **known before the row is pushed** so the twig is
    /// drawn in one pass (moai-p7nr.xj3). A loading or failure row is the directory's only, and so its last, row.
    fn flatten(&self, dir: &str, kin: &[bool], ignored: bool, out: &mut Vec<Row>) {
        let twig = |more: bool| Twig::from_kin(kin.iter().copied().chain([more]).collect());
        match self.dirs.get(dir).and_then(|d| d.read.as_ref()) {
            None => out.push(Row::Loading { twig: twig(false), of: dir.to_owned() }),
            Some(Read::Failed(why)) => {
                out.push(Row::Failed { twig: twig(false), of: dir.to_owned(), why: why.clone() })
            }
            Some(Read::Listed { entries, links, .. }) => {
                for (n, e) in entries.iter().enumerate() {
                    let path = child(dir, &e.name);
                    let ignored = ignored || self.ignored_path(&path);
                    let open = opens(e) && self.is_open(&path);
                    let more = n + 1 < entries.len();
                    out.push(Row::Entry {
                        path: path.clone(),
                        twig: twig(more),
                        entry: e.clone(),
                        open,
                        ignored,
                        link: links.get(&e.name).cloned(),
                        change: None,
                    });
                    if open {
                        let mut below = kin.to_vec();
                        below.push(more);
                        self.flatten(&path, &below, ignored, out);
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
            Side::Info if self.diff_stands() => self.diff.key(keys::Diff::Step(m)),
            Side::Info => self.right.page.go(m),
            Side::Detail => self.detail.go(m),
        }
    }

    /// `H`·`L` — scroll the text sideways by half the pane.
    pub fn pan(&mut self, toward: Toward) {
        if self.diff_stands() {
            return self.diff.key(keys::Diff::Pan(toward));
        }
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
        match on {
            Side::Info if self.diff_stands() => self.diff.roll(by),
            Side::Info => self.right.page.by(by),
            Side::Detail => self.detail.by(by),
            Side::Tree => {}
        }
    }

    /// The sideways wheel over the right pane — the diff's text or the file's.
    pub(super) fn pan_wheel(&mut self, by: isize) {
        if self.diff_stands() {
            self.diff.pan_by(by * diff::PAN_WHEEL as isize);
        } else {
            self.right.pan_by(by * PAN_WHEEL as isize);
        }
    }

    /// `l` — open the directory under the cursor one level. An open one, a file and `..` do nothing.
    pub fn expand(&mut self) {
        // The changed tree stands open whole — it has nothing folded and reads no folder.
        if self.changed.on {
            return;
        }
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
        if let Row::Entry { path, open: true, .. } = row
            && !self.changed.on
        {
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
        if self.changed.on {
            return;
        }
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
        // The changed tree is not stepped into — its folders all stand open; a file still goes to the right pane.
        if self.changed.on {
            if matches!(row, Row::Entry { entry, .. } if !matches!(entry.kind, Kind::Dir)) {
                self.focus = Side::Info;
            }
            return;
        }
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
        if self.here == "." || self.changed.on {
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

    /// The panes that stand, in the order `Ctrl-w w` goes round: the tree, the detail when on, then the file's text or
    /// its diff — the diff not while it is folded, the text not while the detail stands alone.
    pub fn panes(&self) -> Vec<Side> {
        let mut out = vec![Side::Tree];
        if self.detail_on {
            out.push(Side::Detail);
        }
        if !self.detail_on || (self.diff_on && !self.folded) {
            out.push(Side::Info);
        }
        out
    }

    fn focus_next(&mut self) {
        let panes = self.panes();
        let at = panes.iter().position(|p| *p == self.focus).unwrap_or(0);
        self.focus = panes[(at + 1) % panes.len()];
    }

    /// `Ctrl-w h`·`l` — the tree, or the first pane right of it.
    fn focus_toward(&mut self, toward: Toward) {
        let panes = self.panes();
        self.focus = if toward == Toward::Left { Side::Tree } else { panes.get(1).copied().unwrap_or(Side::Tree) };
    }

    /// A pane that went (a toggle, a fold) does not keep the focus — the tree takes it.
    pub(super) fn keep_focus(&mut self) {
        if !self.panes().contains(&self.focus) {
            self.focus = Side::Tree;
        }
    }

    /// Whether the right pane shows the diff now — on, not folded under the detail, and a right pane stands at all.
    pub fn diff_stands(&self) -> bool {
        self.diff_on && !self.folded && !self.narrow
    }

    /// The frame measured whether the diff folds under the detail, and whether any right pane stands
    /// (`draw::files_window`). **A diff that is not drawn reads nothing** — a fold or a window too narrow lets its read
    /// go; room again aims it at the cursor.
    pub(super) fn set_room(&mut self, folded: bool, narrow: bool) {
        if (self.folded, self.narrow) != (folded, narrow) {
            (self.folded, self.narrow) = (folded, narrow);
            self.aim_diff();
        }
    }

    // ---- the checkout, the changed files, the detail and the diff (moai-ffk9.211) ----

    /// Read the checkout list afresh — at the window's opening and as the picker opens.
    fn ask_places(&mut self) {
        let id = self.next_generation();
        self.places_id = id;
        let here = self.opened_at.clone();
        self.worker.with(|w| w.places(id, here));
    }

    /// List the checkout's changed files afresh — the list on screen stays while it reads. `refresh`: the refresh asks
    /// (moai-ffk9.15v), not the person.
    fn ask_changes(&mut self, refresh: bool) {
        let id = self.next_generation();
        self.changed.id = id;
        self.changed.refreshing = refresh;
        let lang = self.lang;
        self.worker.with(|w| w.changes(id, lang));
    }

    /// The diff pane follows the cursor: a file's changes, a folder's (every changed file below it), nothing on `..`
    /// or a note row. Off, folded under the detail, or with no room for a right pane, it holds nothing and reads
    /// nothing — the list's pane's rule (`App::aim_diff`): no `git` runs for a pane no one sees. It reads again once it
    /// stands ([`Window::set_room`]).
    fn aim_diff(&mut self) {
        if !self.diff_stands() {
            if self.diff.source().is_some() {
                self.diff.close();
            }
            return;
        }
        let path = match self.current() {
            Some(Row::Entry { path, entry, .. }) if matches!(entry.kind, Kind::Dir) => Some(format!("{path}/")),
            Some(Row::Entry { path, .. }) => Some(path.clone()),
            _ => None,
        };
        let source = path.map(|path| diff::Source::Checkout { checkout: self.checkout.clone(), path: Some(path) });
        self.diff.show(source);
    }

    /// `SPC v D` over the window — a hand on the pane, so the changed files only no longer takes it down.
    pub fn toggle_diff(&mut self) {
        self.changed.raised_diff = false;
        self.set_diff(!self.diff_on);
    }

    /// Stand the diff pane up or take it down. Standing, it reads what the cursor is on — unless it has no room
    /// ([`Window::aim_diff`]).
    fn set_diff(&mut self, on: bool) {
        self.diff_on = on;
        if on {
            self.aim_diff();
        } else {
            self.diff.close();
        }
        self.keep_focus();
    }

    /// `SPC v d` over the window.
    pub fn toggle_detail(&mut self) {
        self.detail_on = !self.detail_on;
        self.detail.rewind();
        self.keep_focus();
    }

    /// Whether the tree stands in the changed files only.
    pub fn changed_only(&self) -> bool {
        self.changed.on
    }

    /// The changed-files list.
    pub fn listing(&self) -> &Listing {
        &self.changed.list
    }

    /// `SPC v c` — the changed files only, or the whole tree again as it was. Turned on it stands the diff pane up
    /// when it is down, and turned off it takes down only the pane it stood up (moai-p7nr.sw0).
    pub fn toggle_changed(&mut self) {
        if self.changed.on {
            let at = self.at.clone();
            let before = self.changed.before.take();
            self.changed.on = false;
            // Down before the tree is built again, so the right pane goes back to the file's text there.
            if std::mem::take(&mut self.changed.raised_diff) {
                self.set_diff(false);
            }
            self.at = at.clone();
            self.rebuild();
            // The file the cursor stood on stands in the whole tree: the cursor stays on it. Else back to where it was.
            if self.rows.iter().all(|r| r.at() != at)
                && let Some((was, top)) = before
            {
                self.at = was;
                self.rebuild();
                self.list.rewind();
                self.list.by(top as isize);
            }
            self.adrift = false;
            return;
        }
        self.changed.before = Some((self.at.clone(), self.list.offset()));
        self.changed.on = true;
        self.typing = None;
        self.list.rewind();
        self.adrift = false;
        self.ask_changes(false);
        self.rebuild();
        // Aimed once the cursor stands on the changed tree's rows, never on the path it left.
        if !self.diff_on {
            self.changed.raised_diff = true;
            self.set_diff(true);
        }
    }

    /// The changed-files tree: every changed path with its folders, always open — folders first, then by name, as the
    /// read layer orders a folder.
    fn rebuild_changed(&mut self) {
        let mut rows = Vec::new();
        match &self.changed.list {
            Listing::Reading => rows.push(Row::Loading { twig: Twig::default(), of: ".".into() }),
            Listing::Failed(why) => {
                rows.push(Row::Failed { twig: Twig::default(), of: ".".into(), why: Trouble::Changes(why.clone()) });
            }
            Listing::Read(list) => {
                let mut tree: BTreeMap<String, (BTreeSet<String>, Vec<&vcs::Change>)> = BTreeMap::new();
                for c in &list.files {
                    let mut dir = ".".to_string();
                    for part in c.path.split('/').collect::<Vec<_>>().split_last().map_or(&[][..], |(_, d)| d) {
                        let next = child(&dir, part);
                        tree.entry(dir).or_default().0.insert(next.clone());
                        dir = next;
                    }
                    tree.entry(dir).or_default().1.push(c);
                }
                // A folder's rows are its folders, then its files — its last row is its last file, or its last
                // folder when it has none (moai-p7nr.xj3).
                fn walk(
                    tree: &BTreeMap<String, (BTreeSet<String>, Vec<&vcs::Change>)>,
                    dir: &str,
                    kin: &[bool],
                    out: &mut Vec<Row>,
                ) {
                    let Some((dirs, files)) = tree.get(dir) else { return };
                    let count = dirs.len() + files.len();
                    let twig = |n: usize| Twig::from_kin(kin.iter().copied().chain([n + 1 < count]).collect());
                    for (n, d) in dirs.iter().enumerate() {
                        let name = d.rsplit('/').next().unwrap_or(d).to_owned();
                        out.push(Row::Entry {
                            path: d.clone(),
                            twig: twig(n),
                            entry: Entry { name, kind: Kind::Dir, size: 0, openable: true },
                            open: true,
                            ignored: false,
                            link: None,
                            change: None,
                        });
                        let mut below = kin.to_vec();
                        below.push(n + 1 < count);
                        walk(tree, d, &below, out);
                    }
                    let mut files = files.clone();
                    files.sort_by(|a, b| a.path.cmp(&b.path));
                    for (n, c) in files.into_iter().enumerate() {
                        let name = c.path.rsplit('/').next().unwrap_or(&c.path).to_owned();
                        out.push(Row::Entry {
                            path: c.path.clone(),
                            twig: twig(dirs.len() + n),
                            entry: Entry { name, kind: Kind::File, size: 0, openable: true },
                            open: false,
                            ignored: false,
                            link: None,
                            change: Some(c.clone()),
                        });
                    }
                }
                walk(&tree, ".", &[], &mut rows);
            }
        }
        self.rows = rows;
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

    /// `SPC v w` — open the checkout picker, the list read afresh.
    pub fn open_pick(&mut self) {
        self.ask_places();
        let cursor = self.picked().unwrap_or(0);
        self.pick = Some(Pick { cursor, list: Scroll::default() });
    }

    /// Which checkout of the list the window shows — by path.
    pub fn picked(&self) -> Option<usize> {
        self.places.as_ref()?.checkouts.iter().position(|c| c.path == self.checkout.path)
    }

    /// A key on the picker: the moves, `Enter` picks, `Esc` closes. Everything else is quiet.
    fn pick_key(&mut self, act: Option<keys::Files>) {
        use keys::Files as F;
        let n = self.places.as_ref().map_or(0, |p| p.checkouts.len());
        let Some(p) = self.pick.as_mut() else { return };
        match act {
            Some(F::Step(m)) => p.cursor = scroll::cursor(m, p.cursor, || n),
            Some(F::Enter) => {
                let to = self.places.as_ref().and_then(|l| l.checkouts.get(p.cursor)).cloned();
                self.pick = None;
                if let Some(to) = to {
                    self.switch(to);
                }
            }
            Some(F::Close) => self.pick = None,
            _ => {}
        }
    }

    /// Stand on another checkout. **Everything of the one before goes**: the tree, the cursor, the search and its
    /// index, the text and the diff held, and the reads out — a new worker answers on a new channel, so a late answer
    /// of the old one is never even read. The jail is the new checkout's root: a worktree made outside the repository
    /// (`../x`) is its own root, not "outside". The toggles and the focus stay.
    pub fn switch(&mut self, to: Checkout) {
        if to.path == self.checkout.path {
            self.checkout = to;
            return;
        }
        self.worker = Shared(Arc::new(Mutex::new(Worker::start(&to, &self.disk))));
        self.checkout = to;
        self.dirs.clear();
        self.here = ".".into();
        self.at = At::Up;
        self.cursor = 0;
        self.list = Scroll::default();
        self.adrift = false;
        self.chord.clear();
        self.ignored = Ignored::Waiting;
        self.spread = None;
        self.stepped.clear();
        self.right = Right::default();
        self.search = Search::default();
        self.typing = None;
        self.changed.list = Listing::Reading;
        self.changed.before = None;
        self.places_id = 0;
        // A sweep out on the old worker answers on its channel, which is never read again — left standing, its id
        // would hold `refresh_step` off for the rest of the window's life (a removed worktree's `Gone` comes mid-sweep).
        self.refresh.sweep = None;
        self.refresh.from = None;
        self.diff.close();
        self.detail.rewind();
        self.open(".");
        self.ask_changes(false);
        self.rebuild();
    }

    /// The main checkout of the list — where the window goes back to when the one it stood on was removed.
    fn main_checkout(&self) -> Option<Checkout> {
        self.places.as_ref()?.checkouts.iter().find(|c| c.main).cloned()
    }

    /// Take what came back. Rebuilds the rows once, after all of it.
    pub(super) fn poll(&mut self) -> Polled {
        let mut out = Polled::default();
        let mut gone = false;
        out.painted |= self.diff.follow();
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
                    let refresh = self.right.refreshing && self.right.running.as_deref() == Some(path.as_str());
                    match got {
                        // The refresh could not read a file it read before (the disk stalled, a permission): the text
                        // stands and the next look tries again, without a word.
                        Got::Failed(Trouble::Io(_)) if refresh => self.read_done(&path),
                        Got::Text(c) if refresh => {
                            self.right.keep(c.clone());
                            if here {
                                self.right.shown = Shown::Text(c);
                                self.refresh.pace.wake();
                                out.painted = true;
                            }
                        }
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
                Reply::Places { id, places } => {
                    if id == self.places_id {
                        self.places_id = 0;
                        // The checkout standing takes its key, branch and kind from the list: a picked one is a row of
                        // it already; the one the window opened on was found by the thread, its path compared resolved.
                        let standing = places.checkouts.iter().find(|c| c.path == self.checkout.path).or_else(|| {
                            (self.checkout.path == self.opened_at)
                                .then(|| places.here.and_then(|i| places.checkouts.get(i)))
                                .flatten()
                        });
                        if let Some(c) = standing {
                            self.checkout = c.clone();
                            self.aim_diff();
                        }
                        if let Some(p) = self.pick.as_mut() {
                            let at = places.checkouts.iter().position(|c| c.path == self.checkout.path);
                            p.cursor = at.unwrap_or(0);
                        }
                        self.places = Some(places);
                        out.painted = true;
                    }
                }
                Reply::Changes { id, got, took } => {
                    if id == self.changed.id {
                        self.changed.id = 0;
                        let refresh = std::mem::take(&mut self.changed.refreshing);
                        let held = matches!(self.changed.list, Listing::Read(_));
                        let same = match (&got, &self.changed.list) {
                            (Ok(new), Listing::Read(old)) => new == old.as_ref(),
                            // A refresh that failed keeps the list held; the next look tries again, without a word.
                            (Err(_), _) => refresh && held,
                            _ => false,
                        };
                        if refresh {
                            self.refresh.git.done((self.disk.clock)(), took, !same);
                        }
                        if !same {
                            self.changed.list = match got {
                                Ok(c) => Listing::Read(Arc::new(c)),
                                Err(why) => Listing::Failed(why),
                            };
                            out.changed |= self.changed.on;
                            out.painted = true;
                        }
                    }
                }
                Reply::Gone => gone = true,
                Reply::Swept { id, listed, took } => {
                    if self.refresh.sweep != Some(id) {
                        continue;
                    }
                    self.refresh.sweep = None;
                    let moved = self.merge(listed);
                    self.refresh.pace.done((self.disk.clock)(), took, moved);
                    if moved {
                        out.changed = true;
                        if let Some(i) = self.search.index.as_mut() {
                            i.stale = true;
                        }
                    }
                }
            }
        }
        // The checkout was removed while the window stood: back to the main one, with a word.
        if gone
            && !self.checkout.main
            && let Some(main) = self.main_checkout()
        {
            out.gone = Some(self.checkout.key.clone());
            self.switch(main);
            return out;
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
        self.refresh_step();
        out
    }

    // ---- the automatic refresh (moai-ffk9.15v) ----

    /// A key on the window — the refresh goes back to its shortest rest.
    pub(super) fn wake(&mut self) {
        self.refresh.pace.wake();
        self.refresh.git.wake();
        self.diff.wake();
    }

    /// A refresh look is out — the sweep, the text's, the changed list's or the diff's.
    #[cfg(test)]
    pub(super) fn refreshing(&self) -> bool {
        self.refresh.sweep.is_some()
            || self.right.refreshing
            || (self.changed.id != 0 && self.changed.refreshing)
            || self.diff.refreshing()
    }

    /// Whether the text pane stands — neither the detail nor the diff over it.
    fn text_shown(&self) -> bool {
        !self.detail_on && !self.diff_on
    }

    /// One step of the refresh: a stamp sweep when due and none is out, the changed-files list while it stands, the diff
    /// pane's own. Only what is seen is measured, and the git calls take turns.
    fn refresh_step(&mut self) {
        let now = (self.disk.clock)();
        if self.refresh.sweep.is_none() && self.refresh.pace.due(now) {
            self.sweep(now);
        }
        if self.changed.on && self.changed.id == 0 && self.diff.idle() && self.refresh.git.due(now) {
            self.ask_changes(true);
        }
        if self.diff_stands() {
            self.diff.refresh(self.changed.id == 0);
        }
    }

    /// Send a sweep of the open folders the tree shows — the one stood in and every open one below it, read and not
    /// failed — and look at the file the text pane shows.
    fn sweep(&mut self, now: Instant) {
        let here = self.here.clone();
        let open: Vec<&str> = if self.changed.on {
            Vec::new()
        } else {
            self.dirs
                .iter()
                .filter(|(p, d)| (**p == here || inside(p, &here)) && matches!(d.read, Some(Read::Listed { .. })))
                .map(|(p, _)| p.as_str())
                .collect()
        };
        let (take, next) = turn(&open, self.refresh.from.as_deref(), SWEEP_LIMIT);
        self.refresh.from = next;
        let dirs: VecDeque<_> = take
            .into_iter()
            .filter_map(|p| match self.dirs.get(&p) {
                Some(Dir { generation, read: Some(Read::Listed { stamp, racy, .. }) }) => {
                    Some((p, *generation, stamp.clone(), *racy))
                }
                _ => None,
            })
            .collect();
        if dirs.is_empty() {
            self.refresh.pace.done(now, Duration::ZERO, false);
        } else {
            let id = self.next_generation();
            self.refresh.sweep = Some(id);
            self.worker.with(|w| w.send(Request::Sweep(Sweep { id, dirs, listed: Vec::new(), took: Duration::ZERO })));
        }
        // The file the text shows — on the reader, when the person has nothing out there.
        if let Shown::Text(c) = &self.right.shown
            && self.text_shown()
            && self.right.running.is_none()
            && self.right.wanted.is_none()
        {
            self.right.wanted = Some(Look { path: c.path.clone(), was: Some(c.clone()), refresh: true });
            self.send_look();
        }
    }

    /// Take a sweep's folders in: each still open under the generation it was stamped at takes its new entries — by
    /// path, so the open folders below, the cursor and the scroll stay. A folder gone leaves the tree; one that could
    /// not be read keeps what it held. Gives whether a row changed.
    fn merge(&mut self, listed: Vec<(String, u64, Read)>) -> bool {
        let mut moved = false;
        for (path, generation, read) in listed {
            // Folded, refolded or read again by the hand since: the answer is old.
            let Some(d) = self.dirs.get_mut(&path).filter(|d| d.generation == generation && d.read.is_some()) else {
                continue;
            };
            match read {
                Read::Failed(Trouble::Missing) => {
                    self.forget(&path);
                    moved = true;
                }
                Read::Failed(_) => {}
                new @ Read::Listed { .. } => {
                    let same = match (&d.read, &new) {
                        (
                            Some(Read::Listed { entries: a, links: x, .. }),
                            Read::Listed { entries: b, links: y, .. },
                        ) => a == b && x == y,
                        _ => false,
                    };
                    // The same entries: only the stamp is new — nothing to draw.
                    d.read = Some(new);
                    if !same {
                        self.prune(&path);
                        self.drop_gone(&path);
                        moved = true;
                    }
                }
            }
        }
        moved
    }

    /// A folder read again: the open folders it no longer holds are forgotten.
    fn prune(&mut self, dir: &str) {
        let Some(Read::Listed { entries, .. }) = self.dirs.get(dir).and_then(|d| d.read.as_ref()) else { return };
        let still: HashSet<&str> = entries.iter().filter(|e| opens(e)).map(|e| e.name.as_str()).collect();
        // Only the keys below `dir` are looked at — a look that read two thousand folders must not scan every open one
        // for each.
        let below = if dir == "." { String::new() } else { format!("{dir}/") };
        let gone: Vec<String> = self
            .dirs
            .range::<str, _>((std::ops::Bound::Included(below.as_str()), std::ops::Bound::Unbounded))
            .map(|(k, _)| k)
            .take_while(|k| k.starts_with(&below))
            .filter(|k| *k != "." && parent_of(k) == dir && k.rsplit('/').next().is_some_and(|n| !still.contains(n)))
            .cloned()
            .collect();
        for g in gone {
            self.forget(&g);
        }
    }

    /// A folder is gone: it and what was open below it fold away. Standing in it, the tree steps out to the folder
    /// above, the cursor where it was in that folder.
    fn forget(&mut self, dir: &str) {
        if dir == "." {
            return;
        }
        let holds = |p: &str| p == dir || inside(p, dir);
        self.stepped.retain(|p| !holds(p));
        if holds(&self.here) {
            self.here = parent_of(dir).to_owned();
            self.open(&self.here.clone());
            self.at = At::Path(dir.to_owned());
            self.close(dir);
            return self.reroot();
        }
        self.close(dir);
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
        self.typing.is_none() && self.search.query.is_some() && !self.changed.on
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
        // The changed tree is short and its own; the search is the whole tree's and waits under it.
        if self.changed.on {
            return;
        }
        let was = self.search.query.clone();
        let input = Input::new(was.as_deref().unwrap_or_default());
        self.typing = Some(Typing { input, was, from: self.at.clone() });
        // The refresh read folders again since the walk: this search walks afresh (moai-ffk9.15v).
        if self.search.index.as_ref().is_some_and(|i| i.stale) {
            self.worker.with(Worker::stop_walk);
            self.search.index = None;
        }
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
        self.search.index = Some(Index {
            id,
            root,
            entries: Arc::new(Vec::new()),
            walking: true,
            skipping: None,
            failed: None,
            stale: false,
        });
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
        let focus = match self.focus {
            Side::Tree => "tree",
            Side::Info => "info",
            Side::Detail => "detail",
        };
        serde_json::json!({
            "checkout": self.checkout.path, "here": self.here, "cursor": self.current_path(),
            "focus": focus,
            "search": self.search.query,
            "branch": self.checkout.branch, "changed_only": self.changed.on,
            "detail": self.detail_on, "diff": self.diff_on,
        })
    }
}

impl App {
    /// `SPC g f` — open the window on the checkout the session stands in, or on the one list on the project of the
    /// row under the cursor. With neither a notice says why and nothing opens.
    pub(super) fn open_files(&mut self) {
        let lang = self.site.lang;
        let (project, path, seat) = if !self.on_layer() {
            let Some(repo) = self.site.repo.as_ref() else {
                self.notice = Some(say(lang, "tui.files.no_project").to_string());
                return;
            };
            (self.project_name(), repo.here().to_path_buf(), Seat::Here)
        } else {
            let at = match self.current() {
                Some(ListRow::Project(at)) | Some(ListRow::Item(Seat::Place(at), ..)) => at,
                _ => {
                    self.notice = Some(say(lang, "tui.files.no_target").to_string());
                    return;
                }
            };
            let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
            (place.name.clone(), place.path.clone(), Seat::Place(at))
        };
        // The key, the branch and whether it is the main checkout come with the checkout list, read on a thread
        // (moai-ffk9.211) — finding them reads git's files, and the loop does not read.
        let checkout = Checkout { key: ".".into(), path, branch: String::new(), main: true, milestone: None };
        let w = Window::new(project, checkout, self.keys, &self.files_disk, seat, lang);
        self.mode = Mode::Files(Box::new(w));
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
        if let Some(key) = got.gone {
            self.notice = Some(fill(say(self.site.lang, "tui.files.checkout_gone"), &[("key", &key)]));
        }
    }

    /// The issue the file window's detail shows (moai-ffk9.211) — the picked checkout's, as `moai diff` finds it
    /// (`report::changes::issue_of`): an epic worktree's epic, a milestone worktree's milestone, a worktree marked by
    /// one row that row. `None` while the checkout list is read, and for a checkout no issue holds (the main one).
    /// The row index is the last of that id — the load's rule.
    pub(super) fn files_issue(&self) -> Option<(Seat, usize)> {
        let Mode::Files(w) = &self.mode else { return None };
        let places = w.places.as_ref()?;
        let site = self.site_of_seat(w.seat)?;
        let issue = crate::report::changes::issue_of(&site.issues, &w.checkout, &places.workplaces)?;
        let at = site.issues.iter().rposition(|i| i.id == issue.id)?;
        Some((w.seat, at))
    }

    /// The picker's rows' issue titles — each checkout's issue, as the detail finds it.
    pub(super) fn files_pick_titles(&self) -> Vec<Option<String>> {
        let Mode::Files(w) = &self.mode else { return Vec::new() };
        let Some(places) = w.places.as_ref() else { return Vec::new() };
        let site = self.site_of_seat(w.seat);
        places
            .checkouts
            .iter()
            .map(|c| {
                let issues = site.map_or(&[][..], |s| &s.issues[..]);
                crate::report::changes::issue_of(issues, c, &places.workplaces).map(|i| i.title.clone())
            })
            .collect()
    }

    /// One key on the window. **Closing only goes back to browsing** — the window touched nothing behind it.
    ///
    /// **SPC opens the menu** — the statistics and wiki windows' way (`App::wiki_key`); what stands there is the
    /// screen group (`keys::Ctx::files`). A move key that closes the menu moves the window's focused pane.
    ///
    /// Answers **whether this key clears the notice** — the wiki window's rule: a key that only touched the menu and
    /// a pending prefix do not, the window's keys, a screen pick and an unknown key do.
    pub(super) fn files_key(&mut self, k: KeyEvent) -> bool {
        // A key on the window: the refresh goes back to its shortest rest (moai-ffk9.15v).
        if let Mode::Files(w) = &mut self.mode {
            w.wake();
        }
        // **While the search field is open it comes first** — the moves and SPC are letters (the wiki window's way).
        if let Mode::Files(w) = &mut self.mode
            && w.typing.is_some()
        {
            w.type_key(k);
            return true;
        }
        // **The checkout picker comes next while open** — the window's moves, `Enter` to pick, `Esc` to close it.
        if let Mode::Files(w) = &mut self.mode
            && w.pick.is_some()
        {
            let act = w.chord.feed(self.keys.files, k);
            w.pick_key(act);
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
                // **This window's own toggles** (moai-ffk9.211) — the same keys mean the list's on the list; here they
                // touch neither `App::worktree` nor `App::detail_open`, and nothing is written to the config.
                Some(act @ (Browse::Worktree | Browse::Changed | Browse::Detail | Browse::Diff)) => {
                    let Mode::Files(w) = &mut self.mode else { return true };
                    match act {
                        // The picker is a window of its own: the menu closes under it.
                        Browse::Worktree => {
                            w.open_pick();
                            self.chord.clear();
                        }
                        Browse::Detail => w.toggle_detail(),
                        _ => {
                            let was = w.diff_on;
                            if act == Browse::Changed {
                                w.toggle_changed();
                            } else {
                                w.toggle_diff();
                            }
                            // A pane stood up — by hand or by the changed files only — that folds under the detail
                            // says so.
                            if !was && w.diff_on && w.folded {
                                let detail = keys::said(self.keys.browse, Browse::Detail);
                                self.notice =
                                    Some(fill(say(self.site.lang, "tui.files.diff_folded"), &[("detail", &detail)]));
                            }
                        }
                    }
                    true
                }
                // The detail's place and the diff layout are the person's, over the window as on the list.
                Some(act @ (Browse::DetailAt | Browse::DiffLayout)) => {
                    self.run(act, &rows);
                    true
                }
                _ => false,
            };
        }
        let Mode::Files(w) = &mut self.mode else { return false };
        use keys::Files as F;
        // **The diff pane's keys first while it has the focus** (moai-ffk9.41b's rule): `Tab`·`Shift-Tab` go from file
        // to file, `h`·`l` from scope to scope. The moves stay the window's — `gg` is a prefix there.
        if w.focus == Side::Info
            && w.diff_stands()
            && w.chord.held().is_empty()
            && let Lookup::Run(act) = keys::lookup(self.keys.diff, &[k])
            && !matches!(act, keys::Diff::Step(_))
        {
            w.diff.key(act);
            return true;
        }
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
            Some(F::Focus(side)) => w.focus_toward(side),
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

    /// The screens over the window: `SPC g l` closes it onto the list; the menu over it holds the screen group, the
    /// window's own view toggles and options (moai-ffk9.211) and the key list — nothing of the list's — and a move key
    /// closing the menu moves the tree's cursor.
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
        assert_eq!(
            root.iter().map(menu::Entry::text).collect::<Vec<_>>(),
            ["+화면 [파일]", "+보기", "+옵션", "키 목록"]
        );
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
            twig: Twig::default(),
            entry: Entry { name: "a\u{1b}[31mb".into(), kind: Kind::File, size: 0, openable: false },
            open: false,
            ignored: false,
            link: None,
            change: None,
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

    /// **With the diff's focus the bar names the diff's keys** (moai-ffk9.9m0), as it names the text's `H`·`L` — files,
    /// scopes, sideways and the layout; at 80 columns the pane's own three and the way out still stand.
    #[test]
    fn the_bar_names_the_diff_keys_while_the_diff_has_the_focus() {
        let (_s, mut a) = files_app("bar-diff");
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(|_, _, _, _| diff::Loaded::default());
        standing_on(&mut a, "README.md");
        a.hit("SPC v D Esc");
        settle(&mut a);
        a.hit("Ctrl-w w");
        assert_eq!(window(&a).focus, Side::Info);
        let bar = |a: &mut App, w| draw::tests::render(a, w, 20).last().cloned().unwrap_or_default();
        let ours = ["Tab·Shift-Tab 파일", "h·l 범위", "H·L 가로", "SPC o s diff 배치"];
        let wide = bar(&mut a, 160);
        for k in ours {
            assert!(wide.contains(k), "{k:?} is not on the bar: {wide:?}");
        }
        let narrow = bar(&mut a, 80);
        assert_eq!(window(&a).focus, Side::Info, "the premise — the diff stands at 80 columns");
        for k in [&ours[..3], &["Ctrl-w w", "SPC 메뉴"]].concat() {
            assert!(narrow.contains(k), "{k:?} is not on the 80-column bar: {narrow:?}");
        }
        // The tree's focus names none of them.
        a.hit("Ctrl-w w");
        let tree = bar(&mut a, 160);
        assert!(ours.iter().all(|k| !tree.contains(k)), "the tree's focus named the diff's keys: {tree:?}");
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
        // The mark follows the change mark's two columns; a file has no fold column (moai-p7nr.xj3).
        assert!(lines[y].contains("  * x-key.md"), "no mark: {:?}", lines[y]);
        let readme = lines.iter().find(|l| l.contains("README.md")).unwrap();
        assert!(!readme.contains('*'), "a row not found is marked: {readme:?}");
        let x = lines[y].find("x-key.md").map(|b| crate::text::width(&lines[y][..b]) as u16).unwrap();
        let reversed = |x: u16| buf[(x, y as u16)].modifier.contains(Modifier::REVERSED);
        assert!((x..x + 5).all(reversed), "the matching letters are not reversed");
        assert!(!(x + 5..x + 8).any(reversed), "letters past the match are reversed");
        // One level down the twig stands between the mark and the name, and the reversed letters move with it.
        // Off the cursor's row, whose whole line is reversed.
        let y = lines
            .iter()
            .position(|l| l.contains("* └─ x-key.txt") && !l.contains('>'))
            .expect("no found row under a folder");
        let x = lines[y].find("x-key.txt").map(|b| crate::text::width(&lines[y][..b]) as u16).unwrap();
        let reversed = |x: u16| buf[(x, y as u16)].modifier.contains(Modifier::REVERSED);
        assert!((x..x + 5).all(reversed), "the matching letters under a folder are not reversed: {:?}", lines[y]);
        assert!(!(x + 5..x + 8).any(reversed) && !reversed(x - 1), "the reversal drifted: {:?}", lines[y]);
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

    /// Each row's text, by its path — `<reading …>` and `<failed …>` for the note rows.
    fn row_texts(a: &App) -> HashMap<String, String> {
        names(a).into_iter().zip(window(a).rows().iter().map(draw::tests::files_row_text)).collect()
    }

    /// **The tree hangs from the list's twig** (moai-p7nr.xj3): a row with a sibling after it is `├─`, the last `└─`,
    /// an ancestor with siblings after it draws `│` below and the last one a blank; the rows of the directory the tree
    /// stands in have none. The head keeps two columns for the change mark, the twig comes before the fold mark and a
    /// file has no fold column. A row being read hangs as its directory's last.
    #[test]
    fn the_file_tree_hangs_from_the_lists_twig() {
        let (s, mut a) = files_app("twig");
        std::fs::write(s.path().join("src/tui/wiki.rs"), "//! wiki\n").unwrap();
        a.hit("SPC g f");
        settle(&mut a);
        on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        on(&mut a, "src/tui");
        a.hit("l");
        // The read is out: its row hangs as the folder's last.
        let reading = row_texts(&a);
        assert_eq!(
            reading["<reading src/tui>"],
            format!("  │  └─ {}", say(Lang::Ko, "tui.files.reading")),
            "{reading:?}"
        );
        settle(&mut a);
        let t = row_texts(&a);
        assert_eq!(t["src (open)"], "  ▾ src/", "a row at depth 0 has a twig: {t:?}");
        assert_eq!(t["src/tui (open)"], "  ├─ ▾ tui/", "{t:?}");
        assert_eq!(t["src/tui/files.rs"], "  │  ├─ files.rs", "{t:?}");
        assert_eq!(t["src/tui/wiki.rs"], "  │  └─ wiki.rs", "{t:?}");
        assert_eq!(t["src/main.rs"], "  └─ main.rs", "{t:?}");
        assert_eq!(t["README.md"], "  README.md", "the change mark's columns are not kept: {t:?}");
        on(&mut a, "src/tui");
        a.hit("h");
        on(&mut a, "src/main.rs");
        // The twig is the list's look, and it stands on the selected row too.
        let w = window(&a);
        let row = w.rows().iter().find(|r| matches!(r, Row::Entry { path, .. } if path == "src/main.rs")).unwrap();
        let line = draw::tests::files_row_line(row);
        let twig = line.spans.iter().find(|s| s.content.contains("└─")).expect("no twig span");
        assert_eq!(twig.style, draw::dim(), "the twig is not dimmed as the list's is");
        let screen = draw::tests::render(&mut a, 100, 20).join("\n");
        assert!(screen.contains("└─ main.rs"), "the selected row lost its twig: {screen}");
        // Stepping into a folder: its rows are depth 0 again.
        on(&mut a, "src");
        a.hit("Enter");
        settle(&mut a);
        let t = row_texts(&a);
        assert_eq!(t["src/main.rs"], "  main.rs", "{t:?}");
        assert_eq!(t[".."], format!("  ..  {}", say(Lang::Ko, "tui.files.up")), "{t:?}");
    }

    /// **A twig under a last folder** leaves the ancestor's column blank, and a refresh that drops the last file makes
    /// the one before it the last — the twig is measured on every rebuild, never kept.
    #[test]
    fn a_last_folder_draws_no_line_below_and_the_twig_follows_the_disk() {
        let (s, mut a) = files_app("twig-last");
        std::fs::create_dir_all(s.path().join("zz/in")).unwrap();
        std::fs::write(s.path().join("zz/in/a.txt"), "a").unwrap();
        std::fs::write(s.path().join("zz/in/b.txt"), "b").unwrap();
        let clock = clocked(&mut a);
        a.hit("SPC g f");
        settle(&mut a);
        // `in` is `zz`'s only row, so its last: the rows below it draw a blank in its column.
        on(&mut a, "zz");
        a.hit("l");
        settle(&mut a);
        on(&mut a, "zz/in");
        a.hit("l");
        settle(&mut a);
        let t = row_texts(&a);
        assert_eq!(t["zz/in (open)"], "  └─ ▾ in/", "{t:?}");
        assert_eq!(t["zz/in/a.txt"], "     ├─ a.txt", "a last folder draws a line below it: {t:?}");
        assert_eq!(t["zz/in/b.txt"], "     └─ b.txt", "{t:?}");
        std::fs::remove_file(s.path().join("zz/in/b.txt")).unwrap();
        look(&mut a, &clock, Duration::from_secs(2));
        assert!(
            !names(&a).contains(&"zz/in/b.txt".to_string()),
            "the refresh did not see the file go: {:?}",
            names(&a)
        );
        let t = row_texts(&a);
        assert_eq!(t["zz/in/a.txt"], "     └─ a.txt", "a stale twig stands after the last file went: {t:?}");
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

    // ---- the checkout, the changed files, the detail and the diff (moai-ffk9.211) ----

    fn git(dir: &Path, args: &[&str]) {
        crate::git::tests::run_git(dir, None, args);
    }

    fn change(path: &str, status: vcs::Status) -> vcs::Change {
        vcs::Change { path: path.into(), old_path: None, status, adds: Some(3), dels: Some(1), binary: false }
    }

    /// A checkout made of these files, beside the test's main one.
    fn other_checkout(s: &Scratch, name: &str, files: &[&str]) -> Checkout {
        let root = s.path().join(name);
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, format!("{f}\n")).unwrap();
        }
        Checkout {
            key: format!(".worktrees/{name}"),
            path: root,
            branch: format!("worktree-{name}"),
            main: false,
            milestone: None,
        }
    }

    /// The main checkout and these, as the checkout list — with the worktree marks given.
    fn placing(a: &mut App, s: &Scratch, others: Vec<Checkout>, workplaces: Vec<crate::report::Workplace>) {
        let main = Checkout {
            key: ".".into(),
            path: s.path().to_path_buf(),
            branch: "main".into(),
            main: true,
            milestone: None,
        };
        let mut checkouts = vec![main];
        checkouts.extend(others);
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.placing(move |_| Places {
            checkouts: checkouts.clone(),
            workplaces: workplaces.clone(),
            here: Some(0),
        });
    }

    /// Pick the n-th checkout through the picker, as a person does.
    fn pick(a: &mut App, n: usize) {
        a.hit("SPC v w");
        settle(a);
        assert!(window(a).pick.is_some(), "SPC v w did not open the picker");
        assert!(!menu::open(&a.chord), "the menu stayed open over the picker");
        a.hit("g g");
        for _ in 0..n {
            a.hit("j");
        }
        a.hit("Enter");
        settle(a);
    }

    /// **The picker lists what git lists, the main checkout first and the milestone worktree with its word**; picking
    /// a worktree made outside the repository stands its files as the tree's root — not "outside" — and `..`, `Bksp`
    /// and `h` stop there. Read through the real checkout list and the real changed-files list.
    #[test]
    fn the_picker_offers_what_git_lists_and_the_pick_is_the_new_root() {
        let s = Scratch::new("tui-files-pick-git");
        let main = s.path().join("main");
        std::fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q"]);
        std::fs::write(main.join(".gitignore"), ".worktrees/\n").unwrap();
        std::fs::write(main.join("keep.txt"), "keep\n").unwrap();
        std::fs::write(main.join("edit.txt"), "old\n").unwrap();
        std::fs::write(main.join("gone.txt"), "gone\n").unwrap();
        git(&main, &["add", "."]);
        git(&main, &["commit", "-qm", "base"]);
        git(&main, &["worktree", "add", "-q", ".worktrees/milestone-m1", "-b", "milestone/m1"]);
        let outside = s.path().join("outside");
        git(&main, &["worktree", "add", "-q", outside.to_str().unwrap(), "-b", "feat"]);
        std::fs::write(outside.join("feat.txt"), "feat\n").unwrap();
        git(&outside, &["add", "."]);
        git(&outside, &["commit", "-qm", "feat"]);
        std::fs::remove_file(outside.join("gone.txt")).unwrap();
        std::fs::write(main.join("edit.txt"), "new\n").unwrap();
        std::fs::write(main.join("new.txt"), "new\n").unwrap();

        let mut a = App::new(Vec::new(), cfg(), NavPath::new());
        a.site.repo = Some(crate::store::Repo::at(main.clone(), cfg()));
        a.files_disk = Disk::with(files::list, |_| None).placing(places).changing(changes);
        a.hit("SPC g f");
        settle(&mut a);
        let listed = window(&a).places.clone().expect("the checkout list did not come");
        assert!(listed.checkouts[0].main, "the main checkout is not the first row: {:?}", listed.checkouts);
        assert_eq!(listed.checkouts.len(), 3, "{:?}", listed.checkouts);
        let milestone = listed.checkouts.iter().position(|c| c.milestone.is_some()).expect("no milestone worktree");
        let out = listed.checkouts.iter().position(|c| c.branch == "feat").expect("no outside worktree");
        assert!(
            window(&a).checkout.main && !window(&a).checkout.branch.is_empty(),
            "the window did not take its checkout's name"
        );
        // On the main checkout the changed files are what is not committed — no fork point is no failure.
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert_eq!(names(&a), ["edit.txt", "new.txt"], "the main checkout's changed files");
        a.hit("SPC v c Esc");
        // The picker, drawn: the milestone worktree says so, the one shown says it is now.
        a.hit("SPC v w");
        settle(&mut a);
        let screen = draw::tests::render(&mut a, 140, 24).join("\n");
        assert!(screen.contains("마일스톤") && screen.contains("[지금]") && screen.contains("⎇"), "{screen}");
        a.hit("Esc");
        assert!(window(&a).pick.is_none(), "Esc did not close the picker");
        pick(&mut a, milestone);
        assert!(window(&a).checkout.milestone.is_some());
        pick(&mut a, out);
        let w = window(&a);
        assert_eq!(w.checkout.branch, "feat");
        assert!(names(&a).contains(&"feat.txt".to_string()), "the tree is not the picked checkout's: {:?}", names(&a));
        assert!(
            !names(&a).iter().any(|n| n.starts_with("<failed")),
            "the outside worktree was refused: {:?}",
            names(&a)
        );
        // Its root has no way further up.
        a.hit("Bksp h");
        assert_eq!(here(&a), ".");
        assert!(!window(&a).rows().iter().any(|r| matches!(r, Row::Up)));
        // Its changed files: the commit and the deletion — not what did not change.
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert_eq!(names(&a), ["feat.txt", "gone.txt"]);
        let rows: Vec<String> = window(&a).rows().iter().map(draw::tests::files_changed_row_text).collect();
        assert!(rows[0].starts_with("A ") && rows[1].starts_with("D "), "no kind letter without the colour: {rows:?}");
    }

    /// **Picking another checkout drops everything of the one before** — a folder answer, a file's text and a diff
    /// that come late for the old checkout never stand on the new one.
    #[test]
    fn a_late_answer_of_the_checkout_before_never_stands() {
        let (s, mut a) = files_app("pick-stale");
        let b = other_checkout(&s, "b", &["b.txt", "lib/x.rs"]);
        placing(&mut a, &s, vec![b], Vec::new());
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = Disk {
            list: Arc::new(|jail: &Jail, path: &str| {
                if path == "src" {
                    std::thread::sleep(Duration::from_millis(400));
                }
                files::list(jail, path)
            }),
            ..disk
        }
        .reading(|jail, path| {
            if path == "README.md" {
                std::thread::sleep(Duration::from_millis(400));
            }
            files::read(jail, path)
        })
        .diffing(|src, _, _, _| {
            let diff::Source::Checkout { path, .. } = src else { return diff::Loaded::default() };
            if path.as_deref() == Some("README.md") {
                std::thread::sleep(Duration::from_millis(400));
            }
            diff::Loaded::default()
        });
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("SPC v D Esc");
        on(&mut a, "README.md");
        on(&mut a, "src");
        a.hit("l");
        pick(&mut a, 1);
        std::thread::sleep(Duration::from_millis(500));
        settle(&mut a);
        assert_eq!(names(&a), ["lib", "b.txt"], "the old checkout's rows stand on the new one");
        assert!(window(&a).right.shown.path().is_none_or(|p| p != "README.md"), "the old file's text stands");
        let source = window(&a).diff.source().map(|s| format!("{s:?}")).unwrap_or_default();
        assert!(!source.contains("README.md"), "the old file's diff stands: {source}");
    }

    /// **The changed files only**: only what changed stands, with its folders open, a deleted and an untracked file
    /// among them; turned off, the tree, its open folders and the cursor are as they were.
    #[test]
    fn changed_only_shows_the_changes_and_gives_the_tree_back() {
        let (_s, mut a) = files_app("changed");
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.changing(|_, _| {
            let files = vec![
                change("README.md", vcs::Status::Modified),
                change("src/tui/files.rs", vcs::Status::Added),
                change("src/old.rs", vcs::Status::Deleted),
                change("new.txt", vcs::Status::Untracked),
            ];
            Ok(vcs::Changes { total: files.len(), files, truncated: false })
        });
        standing_on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        on(&mut a, "src/main.rs");
        let before = (names(&a), window(&a).cursor);
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert_eq!(
            names(&a),
            ["src (open)", "src/tui (open)", "src/tui/files.rs", "src/old.rs", "README.md", "new.txt"],
            "the changed tree"
        );
        let text: Vec<String> = window(&a).rows().iter().map(draw::tests::files_changed_row_text).collect();
        assert!(text[2].starts_with("A ") && text[2].contains("+3 −1"), "{text:?}");
        assert!(text[3].starts_with("D ") && text[4].starts_with("M ") && text[5].starts_with("? "), "{text:?}");
        let title = screen_title(&draw::tests::render(&mut a, 120, 20));
        assert!(title.contains("바뀐 파일만 4"), "{title}");
        // A deleted file has no text — the pane says so in a plain word and reads nothing (moai-ffk9.9m0). The diff the
        // changed files only stood up (moai-p7nr.sw0) goes down by hand first.
        a.hit("SPC v D Esc");
        on(&mut a, "src/old.rs");
        settle(&mut a);
        assert!(matches!(shown(&a), Shown::Nothing), "the pane read a deleted file: {:?}", shown(&a));
        let (lines, buf, info) = pane(&mut a, 120, 20);
        assert!(lines.iter().any(|l| l.contains("이 체크아웃에서 지운 파일 — SPC v D")), "{lines:?}");
        let warn = draw::from_anstyle(crate::style::WARN).fg;
        let said = (info.y + 1..info.bottom() - 1)
            .flat_map(|y| (info.x + 1..info.right() - 1).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() != " ")
            .all(|(x, y)| buf[(x, y)].fg != warn.unwrap_or_default());
        assert!(said, "the deleted file is told of in a failure's colour: {lines:?}");
        // The row stands dimmed in the tree.
        let rows = draw::tests::render_buf(&mut a, 120, 20);
        let tree = a.drawn.files.clone().unwrap().tree;
        let dimmed = (tree.y..tree.bottom()).any(|y| {
            let cells: Vec<_> = (tree.x..tree.right()).map(|x| &rows[(x, y)]).collect();
            cells.windows(6).any(|w| {
                w.iter().map(|c| c.symbol()).collect::<String>() == "old.rs"
                    && w.iter().all(|c| c.fg == ratatui::style::Color::DarkGray)
            })
        });
        assert!(dimmed, "the deleted file's row is not dimmed");
        // Back: the open folders and the cursor as they were — the cursor stood on a file not in the changed tree.
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert_eq!((names(&a), window(&a).cursor), before, "the tree did not come back as it was");
        // A file standing in both keeps the cursor.
        a.hit("SPC v c Esc");
        settle(&mut a);
        on(&mut a, "README.md");
        a.hit("SPC v c Esc");
        assert_eq!(window(&a).current_path(), Some("README.md"));
    }

    /// Nothing changed: one line says so.
    #[test]
    fn no_changes_says_so() {
        let (_s, mut a) = files_app("changed-none");
        a.hit("SPC g f SPC v c Esc");
        settle(&mut a);
        assert!(names(&a).is_empty());
        let screen = draw::tests::render(&mut a, 100, 20).join("\n");
        assert!(screen.contains("(변경 없음)"), "{screen}");
    }

    /// **The cursor never waits on the changed files** — a slow git leaves `j` moving.
    #[test]
    fn a_slow_changed_list_never_holds_the_cursor() {
        let (_s, mut a) = files_app("changed-slow");
        let disk = std::mem::take(&mut a.files_disk);
        a.files_disk = disk.changing(|_, _| {
            std::thread::sleep(Duration::from_millis(1500));
            Ok(vcs::Changes::default())
        });
        a.hit("SPC g f");
        let began = Instant::now();
        a.hit("SPC v c Esc j j k");
        assert!(began.elapsed() < Duration::from_millis(500), "the keys waited on git: {:?}", began.elapsed());
        assert!(matches!(window(&a).rows(), [Row::Loading { .. }]), "{:?}", names(&a));
        settle(&mut a);
    }

    /// The right side's four ways (the member's table), by the panes drawn.
    fn sides(a: &mut App, w: u16, h: u16) -> (bool, bool, bool) {
        draw::tests::render(a, w, h);
        let d = a.drawn.files.clone().expect("the window did not draw");
        (d.detail.is_some(), d.diff.is_some(), d.info.is_some())
    }

    /// **The right pane is the table**: the text with both off, the diff alone, the detail alone, both — the detail
    /// first (above or left) on the axis of the detail's place, for each of the four places.
    #[test]
    fn the_right_side_follows_the_two_toggles_and_the_detail_place() {
        use super::super::view::DetailAt;
        let (_s, mut a) = files_app("sides");
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(|_, _, _, _| diff::Loaded::default());
        standing_on(&mut a, "README.md");
        assert_eq!(sides(&mut a, 160, 40), (false, false, true));
        assert_eq!(pane(&mut a, 160, 40).0[0], "1 # hi", "both off is the file's text");
        a.hit("SPC v D Esc");
        assert_eq!(sides(&mut a, 160, 40), (false, true, true));
        let screen = draw::tests::render(&mut a, 160, 40).join("\n");
        assert!(screen.contains(" diff · "), "the diff pane's title: {screen}");
        a.hit("SPC v D d Esc");
        assert_eq!(sides(&mut a, 160, 40), (true, false, true));
        let screen = draw::tests::render(&mut a, 160, 40).join("\n");
        assert!(screen.contains("이 체크아웃은 이슈에 매이지 않았다"), "{screen}");
        a.hit("SPC v D Esc");
        for place in DetailAt::ALL {
            a.detail_at = place;
            draw::tests::render(&mut a, 160, 40);
            let d = a.drawn.files.clone().unwrap();
            let (detail, diff) = (d.detail.unwrap(), d.diff.unwrap());
            if place.vertical() {
                assert!(
                    detail.y < diff.y && detail.x == diff.x,
                    "{place:?}: the detail is not above: {detail:?} {diff:?}"
                );
            } else {
                assert!(
                    detail.x < diff.x && detail.y == diff.y,
                    "{place:?}: the detail is not left: {detail:?} {diff:?}"
                );
            }
        }
        // `Ctrl-w w` goes round the three.
        a.detail_at = DetailAt::Right;
        let mut seen = Vec::new();
        for _ in 0..3 {
            a.hit("Ctrl-w w");
            seen.push(window(&a).focus);
        }
        assert_eq!(seen, [Side::Detail, Side::Info, Side::Tree]);
    }

    /// **Too small for both: the detail stays and the diff folds** — the toggles keep their values, the focus does not
    /// stay on the folded pane, and turning the diff on there says why it does not show.
    #[test]
    fn too_small_for_both_folds_the_diff_and_keeps_the_detail() {
        let (_s, mut a) = files_app("fold");
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(|_, _, _, _| diff::Loaded::default());
        standing_on(&mut a, "README.md");
        a.hit("SPC v d D Esc");
        draw::tests::render(&mut a, 160, 40);
        a.hit("Ctrl-w l Ctrl-w w");
        assert_eq!(window(&a).focus, Side::Info);
        // 30 columns: the right side has fewer than the two floors.
        assert_eq!(sides(&mut a, 30, 40), (true, false, true), "the diff stood, or the detail folded");
        let w = window(&a);
        assert!(w.detail_on && w.diff_on && w.folded, "a toggle changed");
        assert_ne!(w.focus, Side::Info, "the focus stayed on the folded diff");
        a.hit("SPC v D D Esc");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("접었다")), "{:?}", a.notice);
        // Room again: both stand.
        assert_eq!(sides(&mut a, 160, 40), (true, true, true));
    }

    /// **The window's toggles are its own**: `SPC v d`, `SPC v D` and `SPC v c` over it touch neither the list's detail
    /// nor the config, and closing the window forgets them.
    #[test]
    fn the_window_toggles_leave_the_list_and_the_config_alone() {
        let (s, mut a) = files_app("toggles");
        let user = s.path().join("user.toml");
        std::fs::write(&user, "[tui]\nmouse = true\n").unwrap();
        a.user_config = Some(user.clone());
        let before = (a.detail_open, a.worktree, a.diff_layout);
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("SPC v d D c Esc");
        settle(&mut a);
        let w = window(&a);
        assert!(w.detail_on && w.diff_on && w.changed_only());
        assert_eq!((a.detail_open, a.worktree, a.diff_layout), before, "a list setting moved");
        assert_eq!(std::fs::read_to_string(&user).unwrap(), "[tui]\nmouse = true\n", "the config was written");
        a.hit("Esc");
        assert_eq!(a.mode, Mode::Browse);
        a.hit("SPC g f");
        let w = window(&a);
        assert!(!w.detail_on && !w.diff_on && !w.changed_only(), "the window kept its toggles");
        // On the list the same keys keep their meaning.
        a.hit("Esc SPC v d Esc");
        assert_ne!(a.detail_open, before.0, "SPC v d on the list lost its meaning");
    }

    /// **The changed files only stands the diff pane up and takes down only its own** (moai-p7nr.sw0): turned on it
    /// stands the pane and reads the cursor's file in the changed tree; turned off it takes the pane down and the text
    /// comes back. A pane up by hand before stays; a hand on the pane in between — `SPC v D` or `moai ctl view diff` —
    /// keeps the person's value. Nothing reaches the list or the config, and closing the window forgets it.
    #[test]
    fn the_changed_files_only_stands_the_diff_up_and_takes_down_only_its_own() {
        use crate::ctl::wire::{self, Verb};
        let (s, mut a) = files_app("changed-diff");
        let user = s.path().join("user.toml");
        std::fs::write(&user, "[tui]\nmouse = true\n").unwrap();
        a.user_config = Some(user.clone());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        a.files_disk = std::mem::take(&mut a.files_disk)
            .changing(|_, _| {
                let files =
                    vec![change("README.md", vcs::Status::Modified), change("src/main.rs", vcs::Status::Modified)];
                Ok(vcs::Changes { total: files.len(), files, truncated: false })
            })
            .diffing(move |src, _, _, _| {
                if let diff::Source::Checkout { path, .. } = src {
                    log.lock().unwrap().push(path.clone().unwrap_or_default());
                }
                diff::Loaded::default()
            });
        let _t = clocked(&mut a);
        let list = (a.diff_open, a.detail_open);
        standing_on(&mut a, "README.md");
        draw::tests::render(&mut a, 160, 40);
        // On: the pane stands and reads the cursor's file of the changed tree.
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert!(window(&a).diff_stands(), "the diff pane did not stand");
        assert_eq!(asked.lock().unwrap().last().map(String::as_str), Some("README.md"), "the diff read nothing");
        a.hit("SPC v");
        let menu = draw::tests::render(&mut a, 160, 40);
        a.hit("Esc");
        let row = menu.iter().find(|l| l.contains(say(Lang::Ko, "tui.menu.diff"))).expect("no diff row in the menu");
        assert!(row.contains(say(Lang::Ko, "tui.state.shown")), "the menu says the pane is down: {row}");
        // Off: the pane it stood up goes and the file's text comes back.
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert!(!window(&a).diff_on, "the pane it stood up stayed");
        assert!(matches!(shown(&a), Shown::Text(_)), "the text did not come back: {:?}", shown(&a));
        // A pane up by hand before stays.
        a.hit("SPC v D c Esc");
        settle(&mut a);
        a.hit("SPC v c Esc");
        assert!(window(&a).diff_on, "a pane up by hand was taken down");
        // Up by the mode, down by hand: it stays down; down and up again by hand: it stays up.
        a.hit("SPC v D Esc");
        a.hit("SPC v c D Esc");
        a.hit("SPC v c Esc");
        assert!(!window(&a).diff_on, "the hand's off was turned over");
        a.hit("SPC v c D D Esc");
        a.hit("SPC v c Esc");
        assert!(window(&a).diff_on, "the hand's on was taken down");
        // `moai ctl view diff off` is a hand too.
        a.hit("SPC v D Esc");
        a.hit("SPC v c Esc");
        settle(&mut a);
        let reply = a.ctl(&wire::request(Verb::View { name: "diff".into(), on: false }, Duration::from_secs(5)));
        assert!(reply.ok, "{reply:?}");
        let reply = a.ctl(&wire::request(Verb::View { name: "diff".into(), on: true }, Duration::from_secs(5)));
        assert!(reply.ok, "{reply:?}");
        a.hit("SPC v c Esc");
        assert!(window(&a).diff_on, "ctl's on was taken down");
        // Nothing leaks: the list's panes and the config stand as they were; a new window remembers nothing.
        assert_eq!((a.diff_open, a.detail_open, window(&a).detail_on), (list.0, list.1, false), "a list pane moved");
        assert_eq!(std::fs::read_to_string(&user).unwrap(), "[tui]\nmouse = true\n", "the config was written");
        a.hit("SPC v D c Esc");
        a.hit("Esc SPC g f");
        settle(&mut a);
        a.hit("SPC v c c Esc");
        assert!(!window(&a).diff_on, "the window kept the mode's pane past closing");
    }

    /// **No room, no read**: turned on in a window too narrow for a right pane, the changed files only reads no diff;
    /// over a detail with no room for both, the pane folds and says so, and the focus stays on the tree.
    #[test]
    fn the_changed_files_only_reads_no_diff_it_cannot_show() {
        let (_s, mut a) = files_app("changed-narrow");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        a.files_disk = std::mem::take(&mut a.files_disk)
            .changing(|_, _| {
                let files = vec![change("README.md", vcs::Status::Modified)];
                Ok(vcs::Changes { total: files.len(), files, truncated: false })
            })
            .diffing(move |src, _, _, _| {
                if let diff::Source::Checkout { path, .. } = src {
                    log.lock().unwrap().push(path.clone().unwrap_or_default());
                }
                diff::Loaded::default()
            });
        let _t = clocked(&mut a);
        standing_on(&mut a, "README.md");
        draw::tests::render(&mut a, 16, 40);
        a.hit("SPC v c Esc");
        settle(&mut a);
        draw::tests::render(&mut a, 16, 40);
        settle(&mut a);
        assert!(window(&a).diff_on, "the pane was not turned on");
        assert!(asked.lock().unwrap().is_empty(), "a diff with no room ran git: {:?}", asked.lock().unwrap());
        a.hit("SPC v c Esc");
        // Over the detail, 30 columns: the diff folds — and says so.
        a.hit("SPC v d Esc");
        draw::tests::render(&mut a, 30, 40);
        a.notice = None;
        a.hit("SPC v c Esc");
        draw::tests::render(&mut a, 30, 40);
        let w = window(&a);
        assert!(w.diff_on && w.folded, "the premise — the pane folded");
        assert_ne!(w.focus, Side::Info, "the focus stands on the folded pane");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("접었다")), "the fold went unsaid: {:?}", a.notice);
    }

    /// **The detail is the picked checkout's issue**: an epic worktree's epic, not a member; the main checkout none.
    #[test]
    fn the_detail_is_the_checkout_issue() {
        let (s, mut a) = files_app("detail");
        let epic = Issue::new(
            "argos-0002".into(),
            "파일 화면 에픽".into(),
            IssueKind::Epic,
            Status::new("in_progress"),
            "2026-09-01T00:00:00Z",
        );
        let mut member = Issue::new(
            "argos-0003".into(),
            "멤버 일".into(),
            IssueKind::Issue,
            Status::new("in_progress"),
            "2026-09-01T00:00:00Z",
        );
        member.epic = Some("argos-0002".into());
        let mut issues = a.site.issues.clone();
        issues.extend([epic, member]);
        let mut b = App::new(issues, cfg(), NavPath::new());
        b.site.repo = a.site.repo.take();
        b.files_disk = std::mem::take(&mut a.files_disk);
        let mut a = b;
        let wt = other_checkout(&s, "moai-epic", &["e.txt"]);
        let workplace = crate::report::Workplace {
            path: ".worktrees/moai-epic".into(),
            branch: "worktree-moai-epic".into(),
            names: ["argos-0002".to_string()].into_iter().collect(),
            marked: ["argos-0003".to_string()].into_iter().collect(),
            broken: false,
        };
        placing(&mut a, &s, vec![wt], vec![workplace]);
        a.hit("SPC g f");
        settle(&mut a);
        a.hit("SPC v d Esc");
        let detail = |a: &mut App| {
            let buf = draw::tests::render_buf(a, 160, 40);
            let r = a.drawn.files.clone().and_then(|d| d.detail).expect("no detail");
            (r.y..r.bottom())
                .map(|y| {
                    let mut line = String::new();
                    let mut skip = 0usize;
                    for x in r.x..r.right() {
                        if skip > 0 {
                            skip -= 1;
                            continue;
                        }
                        let sym = buf[(x, y)].symbol();
                        line.push_str(sym);
                        skip = crate::text::width(sym).saturating_sub(1);
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let text = detail(&mut a);
        assert!(text.contains("이 체크아웃은 이슈에 매이지 않았다") && !text.contains("argos-"), "{text}");
        pick(&mut a, 1);
        let text = detail(&mut a);
        let head: Vec<&str> = text.lines().skip(1).take(3).collect();
        assert!(head.iter().any(|l| l.contains("argos-0002")), "the epic does not head the detail: {text}");
        assert!(!head.iter().any(|l| l.contains("argos-0003")), "a member heads the detail: {text}");
    }

    /// **A worktree removed while the window stands**: the next read finds it gone, and the window goes back to the
    /// main checkout with a word.
    #[test]
    fn a_removed_worktree_goes_back_to_the_main_checkout() {
        let (s, mut a) = files_app("gone");
        let b = other_checkout(&s, "gone-wt", &["d/x.txt"]);
        let root = b.path.clone();
        placing(&mut a, &s, vec![b], Vec::new());
        a.hit("SPC g f");
        settle(&mut a);
        pick(&mut a, 1);
        assert_eq!(names(&a), ["d"]);
        std::fs::remove_dir_all(&root).unwrap();
        on(&mut a, "d");
        a.hit("l");
        settle(&mut a);
        assert!(window(&a).checkout.main, "the window stayed on the removed worktree");
        assert!(names(&a).contains(&"README.md".to_string()), "{:?}", names(&a));
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("gone-wt")), "{:?}", a.notice);
    }

    /// **A sweep out when the checkout changes does not stop the refresh** — its answer goes to the old worker's
    /// channel, which is never read again (a removed worktree's `Gone` comes in the middle of one). The new checkout's
    /// folders are still swept on the next look.
    #[test]
    fn a_sweep_out_as_the_checkout_changes_leaves_the_refresh_running() {
        let (s, mut a) = files_app("refresh-switch");
        let b = other_checkout(&s, "switch-wt", &["d/x.txt"]);
        let root = b.path.clone();
        placing(&mut a, &s, vec![b.clone()], Vec::new());
        let t = clocked(&mut a);
        a.hit("SPC g f");
        settle(&mut a);
        let Mode::Files(w) = &mut a.mode else { panic!("no window") };
        // The look the old worker is still on — its `Swept` never comes back.
        w.refresh.sweep = Some(u64::MAX);
        w.switch(b);
        settle(&mut a);
        assert_eq!(names(&a), ["d"]);
        std::fs::write(root.join("new.txt"), "").unwrap();
        look(&mut a, &t, Duration::from_secs(2));
        assert!(names(&a).contains(&"new.txt".to_string()), "the refresh stopped after the switch: {:?}", names(&a));
    }

    /// **A diff folded under the detail reads nothing** — the cursor moving over files starts no `git`, and room again
    /// reads the file under the cursor.
    #[test]
    fn a_folded_diff_reads_nothing_until_it_stands() {
        let (_s, mut a) = files_app("diff-folded");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(move |src, _, _, _| {
            if let diff::Source::Checkout { path, .. } = src {
                log.lock().unwrap().push(path.clone().unwrap_or_default());
            }
            diff::Loaded::default()
        });
        // The clock stands: no refresh comes due to read on its own.
        let _t = clocked(&mut a);
        standing_on(&mut a, "README.md");
        a.hit("SPC v d Esc");
        // 30 columns: the right side cannot hold the detail and the diff.
        draw::tests::render(&mut a, 30, 40);
        a.hit("SPC v D Esc");
        settle(&mut a);
        on(&mut a, "src");
        settle(&mut a);
        assert!(window(&a).folded && window(&a).diff_on);
        assert!(asked.lock().unwrap().is_empty(), "a folded diff ran git: {:?}", asked.lock().unwrap());
        draw::tests::render(&mut a, 160, 40);
        settle(&mut a);
        assert_eq!(*asked.lock().unwrap(), ["src/"], "the diff did not read once it stood");
    }

    /// **A diff with no right pane to stand in reads nothing** — a window too narrow for any right pane draws the tree
    /// alone, so the cursor moving over files starts no `git` though the diff is on; room again reads the cursor's file.
    #[test]
    fn a_diff_too_narrow_to_stand_reads_nothing_until_it_stands() {
        let (_s, mut a) = files_app("diff-narrow");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(move |src, _, _, _| {
            if let diff::Source::Checkout { path, .. } = src {
                log.lock().unwrap().push(path.clone().unwrap_or_default());
            }
            diff::Loaded::default()
        });
        let _t = clocked(&mut a);
        standing_on(&mut a, "README.md");
        // 16 columns: no right pane stands, only the tree.
        draw::tests::render(&mut a, 16, 40);
        assert!(a.drawn.files.as_ref().is_some_and(|d| d.info.is_none()), "the premise — a right pane stood");
        a.hit("SPC v D Esc");
        settle(&mut a);
        on(&mut a, "src");
        settle(&mut a);
        draw::tests::render(&mut a, 16, 40);
        settle(&mut a);
        assert!(window(&a).diff_on && !window(&a).diff_stands());
        assert!(asked.lock().unwrap().is_empty(), "a diff no one sees ran git: {:?}", asked.lock().unwrap());
        draw::tests::render(&mut a, 160, 40);
        settle(&mut a);
        assert_eq!(*asked.lock().unwrap(), ["src/"], "the diff did not read once it stood");
    }

    /// The diff pane follows the cursor through the window: a file's path, a folder's with a `/`.
    #[test]
    fn the_diff_follows_the_cursor_onto_files_and_folders() {
        let (_s, mut a) = files_app("diff-aim");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(move |src, _, _, _| {
            if let diff::Source::Checkout { path, .. } = src {
                log.lock().unwrap().push(path.clone().unwrap_or_default());
            }
            diff::Loaded::default()
        });
        standing_on(&mut a, "README.md");
        a.hit("SPC v D Esc");
        settle(&mut a);
        on(&mut a, "src");
        settle(&mut a);
        assert_eq!(*asked.lock().unwrap(), ["README.md", "src/"]);
        assert!(a.key_ctx(&a.rows()).diff, "SPC o s does not stand over the window's diff");
    }

    // ---- the automatic refresh (moai-ffk9.15v) ----

    fn hand_clock() -> (Arc<Mutex<Instant>>, Clock) {
        let t = Arc::new(Mutex::new(Instant::now()));
        let c = t.clone();
        (t, Arc::new(move || *c.lock().unwrap()))
    }

    fn later(t: &Mutex<Instant>, by: Duration) {
        *t.lock().unwrap() += by;
    }

    /// Move the clock by `by` and take the refresh look that may be due to its end — as the loop's ordinary steps do.
    /// Gives whether any step said the screen changed.
    fn look(a: &mut App, t: &Mutex<Instant>, by: Duration) -> bool {
        later(t, by);
        let mut changed = false;
        let began = Instant::now();
        loop {
            a.follow_changed = false;
            a.follow_files();
            changed |= a.follow_changed;
            if !window(a).refreshing() && !a.reading_files() {
                return changed;
            }
            assert!(began.elapsed() < Duration::from_secs(60), "the refresh kept running — {:?}", a.mode);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn clocked(a: &mut App) -> Arc<Mutex<Instant>> {
        let (t, clock) = hand_clock();
        a.files_disk = std::mem::take(&mut a.files_disk).clocking(clock);
        t
    }

    /// **An open folder shows what came and what went after a look — the place stays.** The open folders stay open, a
    /// file above the cursor does not move it, and when its file goes the cursor takes the row that came to its place.
    /// Nothing is read before the rest is over.
    #[test]
    fn an_open_folder_follows_the_disk_and_the_place_stays() {
        let (s, mut a) = files_app("refresh-tree");
        let t = clocked(&mut a);
        standing_on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        on(&mut a, "src/tui");
        a.hit("l");
        settle(&mut a);
        on(&mut a, "src/main.rs");
        std::fs::write(s.path().join("src/aaa.rs"), "").unwrap();
        std::fs::write(s.path().join("src/tui/new.rs"), "").unwrap();
        look(&mut a, &t, Duration::from_millis(1900));
        assert!(!names(&a).contains(&"src/aaa.rs".to_string()), "a look before its rest");
        assert!(look(&mut a, &t, Duration::from_millis(100)), "the look said nothing changed");
        let rows = names(&a);
        for want in ["src (open)", "src/tui (open)", "src/tui/new.rs", "src/aaa.rs", "src/main.rs"] {
            assert!(rows.contains(&want.to_string()), "{want} missing: {rows:?}");
        }
        assert_eq!(window(&a).current_path(), Some("src/main.rs"), "a file above moved the cursor");
        // The cursor's file goes: the row after it takes its place.
        std::fs::remove_file(s.path().join("src/main.rs")).unwrap();
        look(&mut a, &t, Duration::from_secs(2));
        assert!(!names(&a).contains(&"src/main.rs".to_string()));
        assert!(window(&a).cursor < window(&a).rows().len(), "the cursor points past the rows");
        assert_eq!(window(&a).current_path(), Some("target"), "the cursor did not take the next row");
        // A folder gone leaves the tree without a word.
        std::fs::remove_dir_all(s.path().join("src/tui")).unwrap();
        look(&mut a, &t, Duration::from_secs(2));
        assert!(!names(&a).iter().any(|n| n.starts_with("src/tui")), "{:?}", names(&a));
        assert!(!window(&a).is_open("src/tui") && a.notice.is_none());
    }

    /// **Standing in a folder that goes, the tree steps out to the folder above.**
    #[test]
    fn a_folder_stood_in_that_goes_steps_out() {
        let (s, mut a) = files_app("refresh-here");
        let t = clocked(&mut a);
        standing_on(&mut a, "src");
        a.hit("Enter");
        settle(&mut a);
        on(&mut a, "src/tui");
        a.hit("Enter");
        settle(&mut a);
        assert_eq!(here(&a), "src/tui");
        std::fs::remove_dir_all(s.path().join("src/tui")).unwrap();
        // The folder stood in is stamped first; its parent's look takes the row away.
        look(&mut a, &t, Duration::from_secs(2));
        look(&mut a, &t, Duration::from_secs(2));
        assert_eq!(here(&a), "src", "the tree stayed in a folder that went");
        assert_eq!(names(&a), ["..", "src/main.rs"]);
    }

    /// **A hung search keeps what it found through a look; the next search walks afresh** — the index is only marked
    /// old, never walked again by the refresh.
    #[test]
    fn a_hung_search_stays_and_the_next_one_walks_afresh() {
        let (s, mut a) = files_app("refresh-search");
        let t = clocked(&mut a);
        let walks = Arc::new(AtomicUsize::new(0));
        let w = walks.clone();
        a.files_disk = std::mem::take(&mut a.files_disk).walking(move |jail, dir, skip, take| {
            w.fetch_add(1, Ordering::SeqCst);
            files::walk(jail, dir, skip, take)
        });
        a.hit("SPC g f");
        settle(&mut a);
        search(&mut a, "main");
        assert_eq!(found(&a), ["src/main.rs"]);
        std::fs::write(s.path().join("main2.txt"), "").unwrap();
        look(&mut a, &t, Duration::from_secs(2));
        assert!(paths(&a).contains(&"main2.txt".to_string()), "the look did not read the root again");
        assert_eq!(found(&a), ["src/main.rs"], "the look changed what the search found");
        assert_eq!(walks.load(Ordering::SeqCst), 1, "the refresh walked the index");
        a.hit("/");
        a.hit("Enter");
        settle(&mut a);
        assert_eq!(walks.load(Ordering::SeqCst), 2, "the next search did not walk afresh");
        assert_eq!(found(&a), ["src/main.rs", "main2.txt"]);
    }

    /// **The text pane reads a changed file again and keeps its scroll**; a shorter file is drawn to its end.
    #[test]
    fn the_text_follows_its_file_and_keeps_its_scroll() {
        let (s, mut a) = files_app("refresh-text");
        let t = clocked(&mut a);
        let long: String = (0..100).map(|i| format!("line {i}\n")).collect();
        std::fs::write(s.path().join("README.md"), &long).unwrap();
        standing_on(&mut a, "README.md");
        pane(&mut a, 100, 20);
        a.hit("Enter");
        a.hit("j j j j j j j j j j");
        assert_eq!(window(&a).right.page.offset(), 10);
        std::fs::write(s.path().join("README.md"), format!("changed\n{long}")).unwrap();
        assert!(look(&mut a, &t, Duration::from_secs(2)), "the change did not draw");
        assert!(matches!(shown(&a), Shown::Text(c) if c.text.starts_with("changed")), "{:?}", shown(&a));
        assert_eq!(window(&a).right.page.offset(), 10, "the scroll went back to the top");
        assert!(pane(&mut a, 100, 20).0[0].contains("line 9"), "the text drawn is not where the scroll stood");
        std::fs::write(s.path().join("README.md"), "short\n").unwrap();
        look(&mut a, &t, Duration::from_secs(2));
        assert!(pane(&mut a, 100, 20).0[0].ends_with("short"), "a shorter file was not pulled to its end");
    }

    /// **A slow look rests ten times its length, counted from its end**; a closed window and a folded folder are not
    /// measured.
    #[test]
    fn a_slow_look_rests_ten_times_and_nothing_unseen_is_measured() {
        let (_s, mut a) = files_app("refresh-slow");
        let (t, clock) = hand_clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let (l, tt) = (log.clone(), t.clone());
        a.files_disk = std::mem::take(&mut a.files_disk).clocking(clock).stamping(move |jail, path| {
            l.lock().unwrap().push(path.to_owned());
            if path == "." {
                later(&tt, Duration::from_secs(3));
            }
            files::stamp(jail, path)
        });
        standing_on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        a.hit("h");
        look(&mut a, &t, Duration::from_secs(2));
        assert_eq!(*log.lock().unwrap(), ["."], "a folded folder was stamped");
        // The look took 3 s and ended at 5 s: the next is due at 35 s, not at 32 s (counted from its start).
        look(&mut a, &t, Duration::from_secs(29));
        assert_eq!(log.lock().unwrap().len(), 1, "a 3 s look rested less than 30 s");
        look(&mut a, &t, Duration::from_secs(1));
        assert_eq!(log.lock().unwrap().len(), 2, "the look after the rest did not come");
        a.hit("Esc");
        later(&t, Duration::from_secs(600));
        a.follow_files();
        std::thread::sleep(Duration::from_millis(50));
        a.follow_files();
        assert_eq!(log.lock().unwrap().len(), 2, "a closed window was measured");
    }

    /// The test window over a root of `n` folders, each empty, with a stamp taker that logs and waits `slow` per folder.
    /// The root's stamp moves on every look — its folder is read again each time and found the same.
    fn wide_app(name: &str, n: usize, slow: Duration) -> (Scratch, App, Arc<Mutex<Instant>>, Arc<Mutex<Vec<String>>>) {
        let (s, mut a) = files_app(name);
        let (t, clock) = hand_clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        let moves = AtomicUsize::new(1);
        a.files_disk = Disk::with(
            move |_, path| {
                if path == "." {
                    return Ok(listed((0..n).map(|i| entry(&format!("d{i:04}"), Kind::Dir)).collect()));
                }
                Ok(listed(Vec::new()))
            },
            |_| Some(Vec::new()),
        )
        .clocking(clock)
        .stamping(move |_, path| {
            l.lock().unwrap().push(path.to_owned());
            if path != "." {
                std::thread::sleep(slow);
                return Some(files::Stamp { modified: None, size: 0 });
            }
            Some(files::Stamp { modified: None, size: moves.fetch_add(1, Ordering::SeqCst) as u64 })
        });
        a.hit("SPC g f");
        settle(&mut a);
        (s, a, t, log)
    }

    fn open_all(a: &mut App, n: usize) {
        let Mode::Files(w) = &mut a.mode else { panic!("no window") };
        for i in 0..n {
            w.open(&format!("d{i:04}"));
        }
        settle(a);
    }

    /// **A look stamps at most 2,000 folders**; the next looks go on from there, so all are stamped in turn.
    #[test]
    fn a_look_stamps_two_thousand_folders_at_most_and_goes_round() {
        let (_s, mut a, t, log) = wide_app("refresh-many", 5_000, Duration::ZERO);
        open_all(&mut a, 5_000);
        let mut seen = HashSet::new();
        for _ in 0..3 {
            log.lock().unwrap().clear();
            assert!(!look(&mut a, &t, Duration::from_secs(10)), "a look that found nothing changed the screen");
            let got = log.lock().unwrap().clone();
            assert!(!got.is_empty() && got.len() <= SWEEP_LIMIT, "one look stamped {} folders", got.len());
            seen.extend(got);
        }
        assert_eq!(seen.len(), 5_001, "three looks did not go round every open folder");
    }

    /// **The person's read goes first** — a folder opened while a long look runs is read after a piece of it, not at its
    /// end; and the keys never wait.
    #[test]
    fn the_persons_read_goes_before_the_look() {
        let (_s, mut a, t, _log) = wide_app("refresh-yield", 1_000, Duration::from_millis(5));
        open_all(&mut a, 400);
        later(&t, Duration::from_secs(2));
        a.follow_files();
        assert!(window(&a).refreshing(), "no look started");
        assert!(!a.loading(), "a look alone wakes the loop fast");
        on(&mut a, "d0900");
        let began = Instant::now();
        a.hit("l j k");
        assert!(began.elapsed() < Duration::from_millis(200), "a key waited on the look");
        while window(&a).count("d0900").is_none() {
            a.follow_files();
            assert!(began.elapsed() < Duration::from_secs(10), "the folder was never read");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(began.elapsed() < Duration::from_millis(1_000), "the read waited {:?} on the look", began.elapsed());
        assert!(window(&a).refreshing(), "the look ended before the read — the test proves nothing");
    }

    /// **An answer for a folder folded while the look ran is dropped** — it stays folded, and opening it again reads it.
    /// **The stamp is the one taken before the read**: a change right after a read shows on the next look.
    #[test]
    fn a_folded_folder_stays_folded_and_a_change_after_the_read_is_not_lost() {
        let (_s, mut a) = files_app("refresh-fold");
        let (t, clock) = hand_clock();
        let names_in = Arc::new(Mutex::new(vec!["a.rs".to_string()]));
        let stamp = |n: usize| files::Stamp { modified: None, size: n as u64 };
        let (held, slow) = (names_in.clone(), Arc::new(AtomicBool::new(false)));
        let (n2, s2) = (names_in.clone(), slow.clone());
        a.files_disk = Disk::with(
            move |_, path| {
                if path == "." {
                    return Ok(listed(vec![entry("src", Kind::Dir)]));
                }
                let mut held = held.lock().unwrap();
                let entries = held.iter().map(|n| entry(n, Kind::File)).collect();
                let read = files::Listed { entries, stamp: stamp(held.len()) };
                // A file comes right after the first read: the stamp kept must be the one taken before it.
                if held.len() == 1 {
                    held.push("b.rs".into());
                }
                Ok(read)
            },
            |_| Some(Vec::new()),
        )
        .clocking(clock)
        .stamping(move |_, path| {
            if path == "src" && s2.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(300));
            }
            Some(stamp(if path == "." { 0 } else { n2.lock().unwrap().len() }))
        });
        standing_on(&mut a, "src");
        a.hit("l");
        settle(&mut a);
        assert_eq!(names(&a), ["src (open)", "src/a.rs"]);
        look(&mut a, &t, Duration::from_secs(2));
        assert_eq!(names(&a), ["src (open)", "src/a.rs", "src/b.rs"], "the change after the read was lost");
        // Folded while its look is out: the answer lands nowhere.
        names_in.lock().unwrap().push("c.rs".into());
        slow.store(true, Ordering::SeqCst);
        later(&t, Duration::from_secs(2));
        a.follow_files();
        assert!(window(&a).refreshing());
        on(&mut a, "src");
        a.hit("h");
        look(&mut a, &t, Duration::ZERO);
        assert_eq!(names(&a), ["src"], "a folded folder opened again with the look's answer");
        a.hit("l");
        settle(&mut a);
        assert!(names(&a).contains(&"src/c.rs".to_string()), "{:?}", names(&a));
    }

    /// **Quiet looks rest longer, up to 10 s; a key on the window brings the rest back to 2 s.** Nothing found draws
    /// nothing.
    #[test]
    fn quiet_looks_slow_down_and_a_key_speeds_them_up() {
        let (_s, mut a, t, log) = wide_app("refresh-quiet", 3, Duration::ZERO);
        let looks = |log: &Mutex<Vec<String>>| log.lock().unwrap().iter().filter(|p| *p == ".").count();
        for by in [2, 4, 8, 10, 10] {
            assert!(!look(&mut a, &t, Duration::from_secs(by)), "a look that found nothing drew");
        }
        assert_eq!(looks(&log), 5);
        look(&mut a, &t, Duration::from_secs(2));
        assert_eq!(looks(&log), 5, "the rest stayed at 2 s after quiet looks");
        look(&mut a, &t, Duration::from_secs(7));
        assert_eq!(looks(&log), 5, "the rest did not grow to 10 s");
        a.hit("j");
        look(&mut a, &t, Duration::ZERO);
        assert_eq!(looks(&log), 6, "a key left the rest at 10 s");
    }

    /// **The changed files follow the worktree** — a file changed shows after a look; the same list draws nothing; a
    /// git that fails keeps the list and says nothing.
    #[test]
    fn the_changed_files_follow_the_worktree_quietly() {
        let (_s, mut a) = files_app("refresh-changed");
        let t = clocked(&mut a);
        let list = Arc::new(Mutex::new(vec![change("README.md", vcs::Status::Modified)]));
        let fail = Arc::new(AtomicBool::new(false));
        let (l, f) = (list.clone(), fail.clone());
        a.files_disk = std::mem::take(&mut a.files_disk).changing(move |_, _| {
            if f.load(Ordering::SeqCst) {
                return Err("git broke".into());
            }
            let files = l.lock().unwrap().clone();
            Ok(vcs::Changes { total: files.len(), files, truncated: false })
        });
        standing_on(&mut a, "README.md");
        a.hit("SPC v c Esc");
        settle(&mut a);
        assert_eq!(names(&a), ["README.md"]);
        list.lock().unwrap().push(change("src/main.rs", vcs::Status::Modified));
        assert!(look(&mut a, &t, Duration::from_secs(2)), "the new change did not draw");
        assert_eq!(names(&a), ["src (open)", "src/main.rs", "README.md"]);
        assert_eq!(window(&a).current_path(), Some("README.md"), "the cursor left its file");
        assert!(!look(&mut a, &t, Duration::from_secs(2)), "the same list drew");
        fail.store(true, Ordering::SeqCst);
        assert!(!look(&mut a, &t, Duration::from_secs(10)), "a failing git drew");
        assert_eq!(names(&a), ["src (open)", "src/main.rs", "README.md"], "a failing git took the list");
        assert!(a.notice.is_none(), "a failing git said so: {:?}", a.notice);
    }

    /// **The diff pane reads itself again** while it stands — and not once it is off.
    #[test]
    fn the_windows_diff_follows_and_stops_when_off() {
        let (_s, mut a) = files_app("refresh-diff");
        let t = clocked(&mut a);
        let head = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n";
        let patch = Arc::new(Mutex::new(format!("{head}@@ -1 +1 @@\n-a\n+b\n")));
        let calls = Arc::new(AtomicUsize::new(0));
        let (p, n) = (patch.clone(), calls.clone());
        a.files_disk = std::mem::take(&mut a.files_disk).diffing(move |_, _, _, lang| {
            n.fetch_add(1, Ordering::SeqCst);
            diff::sample(&p.lock().unwrap(), None, Vec::new(), lang)
        });
        standing_on(&mut a, "README.md");
        a.hit("SPC v D Esc");
        settle(&mut a);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        *patch.lock().unwrap() = format!("{head}@@ -1 +1,2 @@\n-a\n+b\n+c\n");
        assert!(look(&mut a, &t, Duration::from_secs(2)), "the changed diff did not draw");
        assert_eq!(window(&a).diff.loaded().map(|l| l.len(diff::Layout::Unified)), Some(5));
        a.hit("SPC v D Esc");
        let at = calls.load(Ordering::SeqCst);
        look(&mut a, &t, Duration::from_secs(60));
        assert_eq!(calls.load(Ordering::SeqCst), at, "the diff turned off was read again");
    }

    /// **The window's gits never run two at once** — the refresh's and the person's take turns, and the keys never
    /// wait on either.
    #[test]
    fn the_windows_gits_take_turns() {
        let (_s, mut a) = files_app("refresh-gate");
        let t = clocked(&mut a);
        let (out, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let run = {
            let (out, most) = (out.clone(), most.clone());
            move || {
                let n = out.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(n, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(30));
                out.fetch_sub(1, Ordering::SeqCst);
            }
        };
        let (r1, r2) = (run.clone(), run);
        let flip = Arc::new(AtomicUsize::new(0));
        let (f1, f2) = (flip.clone(), flip);
        a.files_disk = std::mem::take(&mut a.files_disk)
            .changing(move |_, _| {
                r1();
                // A list that changes every time, so every look has something to do.
                let n = f1.fetch_add(1, Ordering::SeqCst);
                let mut files =
                    vec![change("README.md", vcs::Status::Modified), change("src/main.rs", vcs::Status::Added)];
                files[0].adds = Some(n);
                Ok(vcs::Changes { total: files.len(), files, truncated: false })
            })
            .diffing(move |_, _, _, lang| {
                r2();
                let n = f2.load(Ordering::SeqCst);
                diff::sample(
                    &format!("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+{n}\n"),
                    None,
                    Vec::new(),
                    lang,
                )
            });
        standing_on(&mut a, "README.md");
        a.hit("SPC v c Esc SPC v D Esc");
        settle(&mut a);
        for i in 0..6 {
            later(&t, Duration::from_secs(10));
            a.follow_files();
            let began = Instant::now();
            on(&mut a, if i % 2 == 0 { "src/main.rs" } else { "README.md" });
            assert!(began.elapsed() < Duration::from_millis(100), "a move waited on git");
            look(&mut a, &t, Duration::ZERO);
            settle(&mut a);
        }
        assert_eq!(most.load(Ordering::SeqCst), 1, "two gits ran at once");
    }

    /// **The refresh never writes git's index** — a `git add` beside it never finds `index.lock`, and the index is
    /// byte for byte what it was.
    #[test]
    fn the_refresh_leaves_the_git_index_alone() {
        let s = Scratch::new("tui-files-refresh-git");
        let root = s.path().to_path_buf();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-qm", "base"]);
        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        let index = root.join(".git/index");
        let stamp = |p: &Path| (std::fs::read(p).unwrap(), std::fs::metadata(p).unwrap().modified().unwrap());
        let before = stamp(&index);
        let mut a = App::new(Vec::new(), cfg(), NavPath::new());
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        let (t, clock) = hand_clock();
        a.files_disk = Disk::default().clocking(clock);
        standing_on(&mut a, "a.txt");
        a.hit("SPC v c Esc SPC v D Esc");
        settle(&mut a);
        for i in 0..3 {
            std::fs::write(root.join("a.txt"), format!("two\n{i}\n")).unwrap();
            look(&mut a, &t, Duration::from_secs(10));
            assert!(!root.join(".git/index.lock").exists(), "a look left index.lock");
        }
        assert!(stamp(&index) == before, "the refresh wrote the index");
        std::fs::write(root.join("b.txt"), "b\n").unwrap();
        git(&root, &["add", "b.txt"]);
    }

    /// **The side-pane rule, one function for the file window and the list's diff pane** (`draw::side_split`): alone,
    /// either takes the room whole; both — the detail first, stacked when its place is top or bottom and side by side
    /// when left or right; too small for the two floors, the detail stays whole and the diff folds.
    #[test]
    fn the_side_rule_puts_the_detail_first_and_folds_the_diff_when_small() {
        use super::super::view::DetailAt;
        use draw::{Sides, side_split};
        use ratatui::layout::Rect;
        let at = Rect::new(10, 5, 100, 40);
        assert_eq!(side_split(at, DetailAt::Right, false, false, None), Sides::default());
        assert_eq!(side_split(at, DetailAt::Right, true, false, None).detail, Some(at));
        assert_eq!(side_split(at, DetailAt::Top, false, true, None).diff, Some(at));
        for place in DetailAt::ALL {
            let s = side_split(at, place, true, true, None);
            let (d, f) = (s.detail.unwrap(), s.diff.unwrap());
            assert!(!s.folded);
            if place.vertical() {
                assert_eq!((d.x, d.width, f.x, f.width), (at.x, at.width, at.x, at.width), "{place:?}");
                assert_eq!((d.y, d.bottom(), f.bottom()), (at.y, f.y, at.bottom()), "{place:?}");
            } else {
                assert_eq!((d.y, d.height, f.y, f.height), (at.y, at.height, at.y, at.height), "{place:?}");
                assert_eq!((d.x, d.right(), f.right()), (at.x, f.x, at.right()), "{place:?}");
            }
        }
        // A share is the detail's, pulled in so the diff keeps its floor.
        let s = side_split(at, DetailAt::Right, true, true, Some(99));
        assert!(s.diff.unwrap().width >= 10, "{s:?}");
        // Too small for both: the detail whole, the diff folded.
        let narrow = Rect::new(0, 0, 19, 40);
        assert_eq!(
            side_split(narrow, DetailAt::Left, true, true, None),
            Sides { detail: Some(narrow), diff: None, folded: true }
        );
        let low = Rect::new(0, 0, 100, 11);
        assert_eq!(
            side_split(low, DetailAt::Bottom, true, true, None),
            Sides { detail: Some(low), diff: None, folded: true }
        );
        assert!(!side_split(Rect::new(0, 0, 100, 12), DetailAt::Bottom, true, true, None).folded);
    }
}
