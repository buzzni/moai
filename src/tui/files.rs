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

use super::keys::{self, Browse, LEADER, Side as Toward};
use super::scroll::{self, Move, Scroll};
use super::{App, Mode, Row as ListRow, Seat, menu};
use crate::files::{self, Entry, Jail, Kind, Why};
use crate::i18n::{fill, say};
use crate::report::changes::Checkout;
use ratatui::crossterm::event::KeyEvent;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// New entries one `Tab` may read before it stops and says so.
pub(super) const SPREAD_LIMIT: usize = 5_000;

/// How long the ignored-directory list may take — past it the window spreads without skipping.
const IGNORED_BUDGET: Duration = Duration::from_secs(10);

/// Replies taken in one step of the loop — a spread sends one per directory, and taking them all at once would
/// rebuild the rows once instead of hundreds of times, but must not hold the loop for thousands either.
const REPLIES_PER_POLL: usize = 256;

type ListFn = dyn Fn(&Jail, &str) -> Result<files::Listed, Why> + Send + Sync;
type IgnoredFn = dyn Fn(&Path) -> Option<Vec<String>> + Send + Sync;

/// What the window reads with — the real disk, or a stand-in a test puts in (a slow reader, a failing git).
#[derive(Clone)]
pub struct Disk {
    list: Arc<ListFn>,
    ignored: Arc<IgnoredFn>,
}

impl Default for Disk {
    fn default() -> Disk {
        Disk {
            list: Arc::new(files::list),
            ignored: Arc::new(|root: &Path| crate::vcs::detect(root)?.ignored_dirs(root, Some(IGNORED_BUDGET)).ok()),
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
        Disk { list: Arc::new(list), ignored: Arc::new(ignored) }
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
    /// The system's own words for an I/O error.
    Io(String),
}

impl Trouble {
    fn of(why: &Why) -> Trouble {
        match why {
            Why::Outside => Trouble::Outside,
            Why::Git => Trouble::Git,
            Why::Missing => Trouble::Missing,
            Why::NotFile | Why::NotText => Trouble::NotFile,
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
    Listed { path: String, generation: u64, read: Read },
    Spread { id: u64, path: String, read: Read },
    SpreadDone { id: u64, read: usize, stopped: bool },
}

/// The threads behind one window: one reading directories in the order asked, and one asking git once for the
/// ignored directories.
struct Worker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    lister: Option<std::thread::JoinHandle<()>>,
    ignored: Option<(Receiver<Option<Vec<String>>>, std::thread::JoinHandle<()>)>,
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
        Worker { requests, replies, lister: Some(lister), ignored: Some((heard, ignored)) }
    }

    fn send(&self, r: Request) {
        // A failed send means the reader died; the next `reply` finds the disconnect and raises its panic.
        let _ = self.requests.send(r);
    }

    fn reply(&mut self) -> Option<Reply> {
        match self.replies.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                if let Some(h) = self.lister.take() {
                    rethrow(h);
                }
                None
            }
        }
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

/// What one step of the loop brought.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Polled {
    /// Something on screen changed.
    pub changed: bool,
    /// A `Tab` stopped at its limit after reading this many entries.
    pub stopped: Option<usize>,
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
        self.spread.is_some() || self.ignored == Ignored::Waiting || self.dirs.values().any(|d| d.read.is_none())
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
        match self.rows.iter().position(|r| r.at() == self.at) {
            Some(i) => self.cursor = i,
            None => {
                self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
                if let Some(r) = self.rows.get(self.cursor) {
                    self.at = r.at();
                }
            }
        }
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
        self.cursor = at.min(self.rows.len().saturating_sub(1));
        if let Some(r) = self.rows.get(self.cursor) {
            self.at = r.at();
        }
    }

    /// A move for the focused pane — the cursor on the tree; half and whole pages scroll the tree's screen and
    /// leave the cursor (the explorer's list, moai-og9h). The right pane has nothing to scroll yet.
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
            Side::Info => {}
        }
    }

    /// One wheel step over a pane — the pane under the pointer, not the focus. A pending prefix is dropped.
    pub(super) fn roll(&mut self, on: Side, by: isize) {
        self.chord.clear();
        if on == Side::Tree && by != 0 {
            let was = self.list.offset();
            self.list.by(by);
            self.adrift |= self.list.offset() != was;
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
                    self.dirs.insert(path, Dir { generation, read: Some(read) });
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
            }
        }
        if out.changed {
            self.rebuild();
        }
        out
    }

    pub(super) fn ctl_state(&self) -> serde_json::Value {
        serde_json::json!({
            "checkout": self.checkout.path, "here": self.here, "cursor": self.current_path(),
            "focus": if self.focus == Side::Tree { "tree" } else { "info" },
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
        self.follow_changed |= got.changed;
        if let Some(n) = got.stopped {
            let expand = keys::said(w.keys.files, keys::Files::Expand);
            self.notice = Some(fill(
                say(self.site.lang, "tui.files.spread_stopped"),
                &[("n", &n.to_string()), ("expand", &expand)],
            ));
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
            Some(F::FocusNext) => w.focus_next(),
            Some(F::Focus(side)) => w.focus = if side == Toward::Left { Side::Tree } else { Side::Info },
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

    /// Take the threads' answers until nothing is out — as the loop does on its fast step.
    fn settle(a: &mut App) {
        let began = Instant::now();
        loop {
            a.follow_files();
            if !a.reading_files() {
                return;
            }
            assert!(began.elapsed() < Duration::from_secs(10), "the window kept waiting — {:?}", a.mode);
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
}
