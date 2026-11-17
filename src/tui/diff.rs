//! The diff pane (moai-ffk9.41b) — one piece the file window's right pane (moai-ffk9.211) and the issue list's diff
//! pane (moai-ffk9.cij) both stand up. This file holds what it shows and how it is read; `draw::diff_pane` draws it.
//!
//! **Two subjects, one pane** ([`Source`]): a checkout's whole branch (fork point → working tree, narrowed to one
//! file) for the file window, and an issue's scopes as `moai diff <id>` reads them (`changes::offers` →
//! `Prepared::read`, the same calls) for the list. **One scope at a time** — `h`·`l` go to the one before or after;
//! the first shown is the first that is not empty, `moai diff`'s pick without `--scope`.
//!
//! **The loop never runs git.** A want ([`View::show`], a scope key) goes to a job thread; while one runs the newest
//! want waits and the ones between are never read — the cursor run down a hundred rows starts two reads, not a
//! hundred. An answer carries the want it answers and stands only if that is still what the pane wants; the others
//! are kept ([`KEPT`]) so the cursor coming back shows them at once while a fresh read goes out. Closing the pane
//! drops what is running without waiting for it; an answer it sends later is never shown.
//!
//! **Plain first, colour after.** The job sends the parsed patch, then colours it file by file
//! (`highlight::diff`) into the very copy it sent — the colour cannot land on another scope's lines, and the scroll does
//! not move when it comes.
//!
//! **What is drawn is what is on screen.** The rows (file heads, hunk heads, lines, the pairs of the side-by-side
//! view), the widest line and the number column's width are measured once on the job thread; a frame only cuts the
//! rows it shows ([`render`]). Lines do not wrap: the text scrolls sideways (`H`·`L`, the sideways wheel) under line
//! numbers and `+`·`-` marks that stay put, both halves of the side-by-side view together, and never past the widest
//! line's end.
//!
//! **Colour never says it alone.** `+`·`-` and the number columns carry the added and removed colours, the change kind
//! is a letter (`M`·`A`·`D`·`R`·`?`), and the text keeps its syntax colours; no line background is painted — in sixteen
//! colours it fights the syntax colours. A file the highlighter does not know takes the added or removed colour whole.
//!
//! **Foreign text** — paths, hunk sections, lines — goes through [`columns::cut`], which never lets a control character
//! reach the terminal.

use super::columns;
use super::keys::{self, Side as Toward};
use super::scroll::{Move, Scroll};
use crate::diff::{self as patch, FileDiff, FileKind, LineKind};
use crate::highlight;
use crate::i18n::{Lang, fill, say};
use crate::model::Issue;
use crate::report::changes::{Checkout, Offer, Scope};
use crate::store::Repo;
use crate::style;
use crate::vcs::{self, Change, Status};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long one read may take — git's budget, the screen's (`changes::offers` leaves it to the caller).
pub const BUDGET: Duration = Duration::from_secs(10);

/// How long a read may run before the pane says it is reading — one that answers within a step never flickers.
const SAY_READING_AFTER: Duration = Duration::from_millis(60);

/// Answers kept for the cursor coming back.
pub const KEPT: usize = 8;

/// The narrowest pane the side-by-side view is drawn in; below it the pane draws unified and the title says why. The
/// setting stays as it is.
pub const SPLIT_MIN: usize = 80;

/// Columns one notch of the sideways wheel scrolls.
pub const PAN_WHEEL: usize = 4;

/// A patch this close to the read layer's limit was cut there (`vcs::PATCH_LIMIT`) — `changes::Answer` folds the
/// list's cut and the patch's into one flag, and the pane says the two apart.
const CUT_NEAR: usize = 64 * 1024;

/// How the lines stand — the person's choice, `[tui] diff_layout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    #[default]
    Unified,
    /// Old on the left, new on the right, paired by `diff::pair`.
    Split,
}

impl Layout {
    /// The config word.
    pub const fn word(self) -> &'static str {
        match self {
            Layout::Unified => "unified",
            Layout::Split => "split",
        }
    }

    /// A config word. An unknown one is `None` and the explorer stands unified — reading is lenient.
    pub fn parse(word: &str) -> Option<Layout> {
        match word {
            "unified" => Some(Layout::Unified),
            "split" => Some(Layout::Split),
            _ => None,
        }
    }

    pub const fn toggled(self) -> Layout {
        match self {
            Layout::Unified => Layout::Split,
            Layout::Split => Layout::Unified,
        }
    }

    /// The screen's word.
    pub fn said(self, lang: Lang) -> &'static str {
        match self {
            Layout::Unified => say(lang, "tui.diff.unified"),
            Layout::Split => say(lang, "tui.diff.split"),
        }
    }
}

/// What the pane shows the changes of.
#[derive(Clone)]
pub enum Source {
    /// A checkout's whole branch — its fork point to the working tree, untracked files with it — narrowed to one file
    /// when `path` is given (the file window). **A `path` ending in `/` names a directory** (moai-ffk9.211): the pane
    /// then holds every changed file below it — the file window's cursor standing on a folder.
    Checkout { checkout: Checkout, path: Option<String> },
    /// An issue's scopes as `moai diff <id>` reads them (the list). `repo` names the project — on the one list each
    /// project is its own repository. The issues are the explorer's, handed over as they are.
    #[allow(dead_code)] // The list's diff pane (moai-ffk9.cij) stands it up; the file window reads checkouts only.
    Issue { repo: Repo, issues: Arc<Vec<Issue>>, id: String },
}

impl Source {
    /// The identity — what a want and an answer are matched by. The issues are not in it: a reread of the tracker
    /// does not make another subject.
    fn key(&self) -> String {
        match self {
            Source::Checkout { checkout, path } => {
                format!("checkout\0{}\0{}", checkout.path.display(), path.as_deref().unwrap_or("\0"))
            }
            Source::Issue { repo, id, .. } => format!("issue\0{}\0{id}", repo.here().display()),
        }
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key().replace('\0', " "))
    }
}

/// What a want and an answer are matched by: the subject and the scope asked (`None`: the first not empty).
type WantId = (String, Option<Scope>);

/// A read the pane wants.
#[derive(Debug, Clone)]
struct Want {
    id: WantId,
    source: Source,
}

/// Why there is nothing to show — the read layer's kinds (`git::Told`): `no_git`, `not_a_repo`, `timeout`,
/// `stream`, `encoding`, `failed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    pub kind: &'static str,
    pub said: String,
}

/// One changed file: what the list said of it and its parsed patch, with the colouring once it came.
#[derive(Debug)]
pub struct Part {
    pub change: Change,
    pub file: FileDiff,
    /// Set once, by the job, after the plain text stood. `Some(None)`: the highlighter has nothing for this file.
    paint: OnceLock<Option<Vec<Vec<Vec<highlight::Span>>>>>,
}

impl Part {
    fn new(change: Change, file: FileDiff) -> Part {
        Part { change, file, paint: OnceLock::new() }
    }

    /// A line's colouring — `None` while it has not come or when the file has none: the text then takes the added
    /// or removed colour whole.
    fn spans(&self, hunk: usize, line: usize) -> Option<&[highlight::Span]> {
        self.paint.get()?.as_ref()?.get(hunk)?.get(line).map(Vec::as_slice)
    }

    /// Whether the colouring came (with or without colours).
    #[cfg(test)]
    pub fn painted(&self) -> bool {
        self.paint.get().is_some()
    }
}

/// One drawn row, by index into the parts — measured once on the job thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    /// A file's head: kind letter, path, counts.
    File(u32),
    /// Why a file has no lines — binary, or only its mode changed.
    Note(u32),
    Hunk(u32, u32),
    /// A unified line.
    One(u32, u32, u32),
    /// A side-by-side row — the old line and the new one, either may be missing.
    Pair(u32, u32, Option<u32>, Option<u32>),
    /// The patch was cut at the read layer's limit.
    Cut,
}

/// One answer: a scope read, measured for drawing.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The scope read (an issue's), `None` for a checkout's whole branch or when there was none.
    pub scope: Option<Scope>,
    /// The issue's scopes in their order — `h`·`l` walk them.
    pub offers: Vec<Offer>,
    pub parts: Vec<Part>,
    /// Files in the scope — more than the parts when the list stopped at the read layer's 5,000.
    pub total: usize,
    pub list_cut: bool,
    /// The patch stopped at the read layer's 2 MiB: the last file may have come halfway, the ones after not at all.
    pub patch_cut: bool,
    pub failed: Option<Failed>,
    /// The file asked for is not among the scope's changes.
    pub unchanged: bool,
    /// The number column's width — the largest line number of the whole scope, so the text does not shift sideways
    /// between files.
    digits: usize,
    /// The widest line in cells, tabs spread — the sideways scroll ends there.
    widest: usize,
    unified: Vec<Row>,
    split: Vec<Row>,
    /// The file heads' rows, per layout — `Tab`·`Shift-Tab` jump between them.
    heads: [Vec<usize>; 2],
}

/// Where a line sits in its hunk's slice — `diff::pair` hands out references.
fn index_in(lines: &[patch::Line], l: &patch::Line) -> u32 {
    let size = std::mem::size_of::<patch::Line>().max(1);
    ((l as *const patch::Line as usize - lines.as_ptr() as usize) / size) as u32
}

fn no_newline_note(lang: Lang) -> &'static str {
    say(lang, "tui.diff.no_newline")
}

impl Loaded {
    fn failed(kind: &'static str, said: String) -> Loaded {
        Loaded { failed: Some(Failed { kind, said }), ..Loaded::default() }
    }

    /// Lay the rows out and measure — once, on the job thread.
    fn measure(&mut self, lang: Lang) {
        let note = columns::width(no_newline_note(lang)) + 1;
        let mut digits = 1;
        let mut widest = 0;
        let (mut unified, mut split) = (Vec::new(), Vec::new());
        let mut heads: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        for (i, part) in self.parts.iter().enumerate() {
            let fi = i as u32;
            heads[0].push(unified.len());
            heads[1].push(split.len());
            unified.push(Row::File(fi));
            split.push(Row::File(fi));
            if part.file.binary || part.file.hunks.is_empty() {
                unified.push(Row::Note(fi));
                split.push(Row::Note(fi));
                continue;
            }
            for (h, hunk) in part.file.hunks.iter().enumerate() {
                let hi = h as u32;
                unified.push(Row::Hunk(fi, hi));
                split.push(Row::Hunk(fi, hi));
                for (l, line) in hunk.lines.iter().enumerate() {
                    unified.push(Row::One(fi, hi, l as u32));
                    let n = line.old_no.max(line.new_no).unwrap_or(0);
                    digits = digits.max(n.to_string().len());
                    widest = widest.max(columns::width(&line.text) + if line.no_newline { note } else { 0 });
                }
                for (old, new) in patch::pair(hunk) {
                    split.push(Row::Pair(
                        fi,
                        hi,
                        old.map(|l| index_in(&hunk.lines, l)),
                        new.map(|l| index_in(&hunk.lines, l)),
                    ));
                }
            }
        }
        if self.patch_cut {
            unified.push(Row::Cut);
            split.push(Row::Cut);
        }
        self.digits = digits;
        self.widest = widest;
        self.unified = unified;
        self.split = split;
        self.heads = heads;
    }

    fn rows(&self, layout: Layout) -> &[Row] {
        match layout {
            Layout::Unified => &self.unified,
            Layout::Split => &self.split,
        }
    }

    fn heads(&self, layout: Layout) -> &[usize] {
        &self.heads[usize::from(layout == Layout::Split)]
    }

    /// The rows drawn in that layout.
    pub fn len(&self, layout: Layout) -> usize {
        self.rows(layout).len()
    }

    /// The part a row belongs to.
    fn part_of(&self, row: Row) -> Option<usize> {
        match row {
            Row::File(i) | Row::Note(i) | Row::Hunk(i, _) | Row::One(i, ..) | Row::Pair(i, ..) => Some(i as usize),
            Row::Cut => None,
        }
    }

    /// How many files came before the patch was cut — the last one with lines.
    fn reached(&self) -> usize {
        self.parts.iter().rposition(|p| !p.file.hunks.is_empty()).map_or(0, |i| i + 1)
    }
}

/// Reads one scope. The real one runs git ([`read`]); a test puts in a slow or counting one.
pub type ReadFn = dyn Fn(&Source, Option<Scope>, Option<Duration>, Lang) -> Loaded + Send + Sync;
/// Colours one file — `highlight::diff`, or a test's stand-in.
pub type PaintFn = dyn Fn(&FileDiff) -> Option<Vec<Vec<Vec<highlight::Span>>>> + Send + Sync;

/// The read layer's error as the pane says it — `changes::told`'s mapping, for the reads this file makes itself.
pub(super) fn told(root: &std::path::Path, lang: Lang, e: vcs::Error) -> Failed {
    let words = |kind: &'static str, key: &'static str| Failed { kind, said: say(lang, key).to_owned() };
    match e {
        vcs::Error::Git(e) => {
            let t = e.told(root, lang);
            Failed { kind: t.kind, said: t.said }
        }
        vcs::Error::Io(e) => {
            let t = crate::git::Error::Stream(e).told(root, lang);
            Failed { kind: t.kind, said: t.said }
        }
        vcs::Error::Timeout => words("timeout", "diff.timeout"),
        vcs::Error::Limit => words("failed", "diff.limit"),
        vcs::Error::InvalidRevision => words("failed", "diff.invalid_revision"),
        vcs::Error::InvalidPath => words("failed", "diff.invalid_path"),
    }
}

/// The kind the parser would give a file the list names.
fn kind_of(status: Status) -> FileKind {
    match status {
        Status::Added | Status::Untracked => FileKind::Added,
        Status::Deleted => FileKind::Deleted,
        Status::Renamed => FileKind::Renamed,
        Status::Copied => FileKind::Copied,
        Status::Modified | Status::TypeChanged | Status::Conflicted => FileKind::Modified,
    }
}

/// A file's diff from the list's line and its hunks — the highlighter picks the language by the new path, the old one
/// for a deleted file.
fn file_of(change: &Change, hunks: Vec<patch::Hunk>) -> FileDiff {
    let kind = kind_of(change.status);
    let new_path = (kind != FileKind::Deleted).then(|| change.path.clone());
    let old_path = match kind {
        FileKind::Added => None,
        FileKind::Renamed | FileKind::Copied => change.old_path.clone().or_else(|| Some(change.path.clone())),
        _ => Some(change.path.clone()),
    };
    FileDiff { old_path, new_path, kind, binary: change.binary, hunks }
}

/// The real reader: a checkout's whole branch through `vcs`, an issue's scope through `changes` — the calls
/// `moai diff` makes, so the two surfaces answer alike.
pub fn read(source: &Source, scope: Option<Scope>, budget: Option<Duration>, lang: Lang) -> Loaded {
    match source {
        Source::Checkout { checkout, path } => whole(checkout, path.as_deref(), budget, lang),
        Source::Issue { repo, issues, id } => issue(repo, issues, id, scope, budget, lang),
    }
}

fn issue(
    repo: &Repo,
    issues: &[Issue],
    id: &str,
    scope: Option<Scope>,
    budget: Option<Duration>,
    lang: Lang,
) -> Loaded {
    use crate::changes::RequestError;
    let started = Instant::now();
    let prepared = crate::changes::offers(repo, issues, id, budget, lang);
    let left = || budget.map(|b| b.saturating_sub(started.elapsed()));
    // A scope chosen by hand can be gone by now (the commit table moved): the first not empty stands instead.
    let answer = match prepared.read(scope, None, false, left()) {
        Err(RequestError::Scope(_)) => prepared.read(None, None, false, left()),
        other => other,
    };
    let Ok(answer) = answer else { return Loaded::default() };
    let list_cut = answer.total > answer.files.len();
    let patch_cut = answer.truncated && (!list_cut || answer.patch.len() + CUT_NEAR >= vcs::PATCH_LIMIT);
    let failed = answer.vcs_error.map(|t| Failed { kind: t.kind, said: t.said });
    let parts = answer
        .files
        .into_iter()
        .map(|f| {
            let file = file_of(&f.change, f.hunks.unwrap_or_default());
            Part::new(f.change, file)
        })
        .collect();
    Loaded {
        scope: answer.scope,
        offers: answer.offers,
        parts,
        total: answer.total,
        list_cut,
        patch_cut,
        failed,
        ..Loaded::default()
    }
}

/// A checkout's whole branch, narrowed to `path`. The list is read whole (it names the kind and the counts, and
/// whether the file changed at all); the patch only for that file — or, for a renamed one, whole: git finds a rename
/// only when both names are in the pathspec.
fn whole(checkout: &Checkout, path: Option<&str>, budget: Option<Duration>, lang: Lang) -> Loaded {
    let root = checkout.path.as_path();
    let started = Instant::now();
    let left = || budget.map(|b| b.saturating_sub(started.elapsed()));
    let Some(backend) = vcs::detect(root) else {
        let e = vcs::Error::Git(crate::git::Error::Failed("not a git repository".into()));
        let f = told(root, lang, e);
        return Loaded::failed(f.kind, f.said);
    };
    let mut changes = match backend.changes(root, &vcs::Scope::Whole, left()) {
        Ok(c) => c,
        Err(e) => {
            let f = told(root, lang, e);
            return Loaded::failed(f.kind, f.said);
        }
    };
    let list_cut = changes.truncated;
    let total = changes.total;
    let mut ask = None;
    // A folder takes every file below it, by either name. Its patch is read whole and cut to those files after: a
    // folder pathspec would leave out the untracked files below it (`vcs` matches those by their whole name).
    let folder = path.and_then(|p| p.strip_suffix('/'));
    let below = |p: &str, dir: &str| p.strip_prefix(dir).is_some_and(|t| t.starts_with('/'));
    if let Some(dir) = folder {
        changes.files.retain(|c| below(&c.path, dir) || c.old_path.as_deref().is_some_and(|o| below(o, dir)));
        if changes.files.is_empty() {
            return Loaded { unchanged: true, total, ..Loaded::default() };
        }
    } else if let Some(p) = path {
        changes.files.retain(|c| c.path == p || c.old_path.as_deref() == Some(p));
        if changes.files.is_empty() {
            return Loaded { unchanged: true, total, ..Loaded::default() };
        }
        if !changes.files.iter().any(|c| matches!(c.status, Status::Renamed | Status::Copied)) {
            ask = Some(p);
        }
    }
    let got = match backend.patch_with_changes(root, &vcs::Scope::Whole, ask, left(), &changes) {
        Ok(p) => p,
        Err(e) => {
            let f = told(root, lang, e);
            return Loaded::failed(f.kind, f.said);
        }
    };
    let mut text = got.text;
    if got.truncated && !text.ends_with('\n') {
        text.truncate(text.rfind('\n').map_or(0, |i| i + 1));
    }
    let parsed = patch::parse(&text);
    let parts: Vec<Part> = changes
        .files
        .into_iter()
        .map(|change| {
            let hunks = parsed
                .iter()
                .filter(|f| {
                    f.new_path.as_deref() == Some(&change.path)
                        || (f.new_path.is_none() && f.old_path.as_deref() == Some(&change.path))
                })
                .flat_map(|f| f.hunks.iter().cloned())
                .collect();
            let file = file_of(&change, hunks);
            Part::new(change, file)
        })
        .collect();
    let shown = parts.len();
    Loaded {
        total: if folder.is_some() {
            shown
        } else if path.is_some() {
            1
        } else {
            total
        },
        list_cut: path.is_none() && list_cut,
        patch_cut: got.truncated,
        parts,
        ..Loaded::default()
    }
}

/// What the job threads say.
enum Reply {
    Loaded {
        serial: u64,
        id: WantId,
        loaded: Arc<Loaded>,
    },
    /// One more file got its colours.
    Painted {
        serial: u64,
    },
}

/// A read out.
struct Job {
    serial: u64,
    want: Want,
    handle: JoinHandle<()>,
}

/// The pane's threads: one read at a time, the newest want waiting, the colouring going on after an answer.
struct Work {
    out: Sender<Reply>,
    replies: Receiver<Reply>,
    running: Option<Job>,
    wanted: Option<Want>,
    /// Jobs that answered and still colour, and jobs dropped by closing — reaped as they end.
    after: Vec<JoinHandle<()>>,
    serial: u64,
    /// Answers at or below this serial were asked before the pane closed; they never stand.
    floor: u64,
    /// What the pane wants now — a job colours only while its want is this one.
    current: Arc<Mutex<Option<WantId>>>,
    read: Arc<ReadFn>,
    paint: Arc<PaintFn>,
    lang: Lang,
}

/// A thread ended: raise its panic on the loop, as the reread does (`App::follow_commits`).
fn rethrow(handle: JoinHandle<()>) -> bool {
    match handle.join() {
        Ok(()) => true,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl Work {
    fn new(read: Arc<ReadFn>, paint: Arc<PaintFn>, lang: Lang) -> Work {
        let (out, replies) = mpsc::channel();
        Work {
            out,
            replies,
            running: None,
            wanted: None,
            after: Vec::new(),
            serial: 0,
            floor: 0,
            current: Arc::new(Mutex::new(None)),
            read,
            paint,
            lang,
        }
    }

    fn set_current(&self, id: Option<WantId>) {
        *self.current.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = id;
    }

    /// The pane wants this read. While one runs it waits — only the newest does.
    fn ask(&mut self, want: Want) {
        self.set_current(Some(want.id.clone()));
        if self.running.as_ref().is_some_and(|j| j.want.id == want.id) {
            self.wanted = None;
        } else if self.running.is_some() {
            self.wanted = Some(want);
        } else {
            self.start(want);
        }
    }

    fn start(&mut self, want: Want) {
        self.serial += 1;
        let serial = self.serial;
        let (out, read, paint, current, lang) =
            (self.out.clone(), self.read.clone(), self.paint.clone(), self.current.clone(), self.lang);
        let (source, id) = (want.source.clone(), want.id.clone());
        let handle = std::thread::spawn(move || {
            let mut loaded = read(&source, id.1, Some(BUDGET), lang);
            loaded.measure(lang);
            let loaded = Arc::new(loaded);
            if out.send(Reply::Loaded { serial, id: id.clone(), loaded: loaded.clone() }).is_err() {
                return;
            }
            for part in &loaded.parts {
                let wanted = current.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() == Some(&id);
                if !wanted {
                    return;
                }
                let _ = part.paint.set(paint(&part.file));
                if out.send(Reply::Painted { serial }).is_err() {
                    return;
                }
            }
        });
        self.running = Some(Job { serial, want, handle });
    }

    /// Let go of everything out: what runs ends on its own and is never shown.
    fn drop_all(&mut self) {
        self.floor = self.serial;
        self.wanted = None;
        self.set_current(None);
        if let Some(job) = self.running.take() {
            self.after.push(job.handle);
        }
    }

    /// Take the answers, reap the threads that ended, start the next read. Gives the answers for the current serials
    /// and whether colour came.
    fn take(&mut self) -> (Vec<(WantId, Arc<Loaded>)>, bool) {
        let ended = self.running.as_ref().is_some_and(|j| j.handle.is_finished());
        let mut got = Vec::new();
        let mut painted = false;
        while let Ok(reply) = self.replies.try_recv() {
            match reply {
                Reply::Loaded { serial, .. } | Reply::Painted { serial } if serial <= self.floor => {}
                Reply::Loaded { serial, id, loaded } => {
                    if self.running.as_ref().is_some_and(|j| j.serial == serial)
                        && let Some(job) = self.running.take()
                    {
                        self.after.push(job.handle);
                    }
                    got.push((id, loaded));
                }
                Reply::Painted { .. } => painted = true,
            }
        }
        // A read that ended without answering: a panic is raised; one that vanished quietly is asked again.
        if ended && let Some(job) = self.running.take() {
            rethrow(job.handle);
            let still =
                self.current.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() == Some(&job.want.id);
            if still && self.wanted.is_none() {
                self.wanted = Some(job.want);
            }
        }
        let (done, going): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.after).into_iter().partition(|h| h.is_finished());
        self.after = going;
        for h in done {
            rethrow(h);
        }
        if self.running.is_none()
            && let Some(want) = self.wanted.take()
        {
            self.start(want);
        }
        (got, painted)
    }

    fn busy(&self) -> bool {
        self.running.is_some() || self.wanted.is_some() || !self.after.is_empty()
    }
}

/// The threads as the pane holds them — shared, not copied: a window holding the pane is cloned behind the suggestion
/// list and the copy that comes back must still hear its answers (`files::Shared`).
#[derive(Clone)]
struct Shared(Arc<Mutex<Work>>);

impl Shared {
    fn with<T>(&self, f: impl FnOnce(&mut Work) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

/// What the pane shows.
#[derive(Debug, Clone)]
pub enum Shown {
    /// Nothing asked — the owner has no subject (a project head, `..`).
    Nothing,
    /// The read is out. `said` once it took long enough to say so.
    Reading {
        since: Instant,
        said: bool,
    },
    Loaded(Arc<Loaded>),
}

/// The pane's state. The owner says what to show ([`View::show`]), calls [`View::follow`] each step of the loop and
/// draws it with `draw::diff_pane`.
#[derive(Clone)]
pub struct View {
    source: Option<Source>,
    /// The scope chosen by hand — `None`: the first not empty.
    chosen: Option<Scope>,
    pub shown: Shown,
    /// The scopes of the last answer and the one it read — the title and `h`·`l` read them, also while the next reads.
    offers: Vec<Offer>,
    at: Option<Scope>,
    pub page: Scroll,
    /// The sideways scroll in cells — the first text column shown.
    pub pan: usize,
    /// The text columns of the last frame — `H`·`L` go half of it and the end is measured with it.
    room: usize,
    /// The layout the last frame drew — after the narrow pane's fallback. Keys and the wheel go by it.
    drawn: Layout,
    kept: VecDeque<(WantId, Arc<Loaded>)>,
    work: Shared,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("diff::View").field("source", &self.source).field("chosen", &self.chosen).finish()
    }
}

impl View {
    /// A pane reading with git.
    #[allow(dead_code)] // The list's diff pane (moai-ffk9.cij); the file window reads through its `files::Disk`.
    pub fn new(lang: Lang) -> View {
        View::with(Arc::new(read), Arc::new(highlight::diff), lang)
    }

    /// A pane reading and colouring with these — a test's slow or counting stand-ins.
    pub fn with(read: Arc<ReadFn>, paint: Arc<PaintFn>, lang: Lang) -> View {
        View {
            source: None,
            chosen: None,
            shown: Shown::Nothing,
            offers: Vec::new(),
            at: None,
            page: Scroll::default(),
            pan: 0,
            room: 0,
            drawn: Layout::Unified,
            kept: VecDeque::new(),
            work: Shared(Arc::new(Mutex::new(Work::new(read, paint, lang)))),
        }
    }

    /// The subject now shown.
    pub fn source(&self) -> Option<&Source> {
        self.source.as_ref()
    }

    /// The scopes and the one shown — the title says them.
    pub fn offers(&self) -> (&[Offer], Option<Scope>) {
        (&self.offers, self.at)
    }

    fn want(&self) -> Option<Want> {
        let source = self.source.clone()?;
        Some(Want { id: (source.key(), self.chosen), source })
    }

    /// Show the changes of this subject — the owner calls it as its cursor lands. **The same subject again does
    /// nothing** (its data is refreshed for the next read); another one starts at its first line and its first scope
    /// that is not empty. `None` empties the pane.
    pub fn show(&mut self, source: Option<Source>) {
        if source.as_ref().map(Source::key) == self.source.as_ref().map(Source::key) {
            self.source = source;
            return;
        }
        self.source = source;
        self.chosen = None;
        self.offers.clear();
        self.at = None;
        self.rewind();
        self.ask();
    }

    /// Read the shown subject again — the pane coming back on. The scroll stays.
    #[allow(dead_code)] // The refresh (moai-ffk9.15v) and the list's pane (moai-ffk9.cij) call it.
    pub fn reread(&mut self) {
        self.ask();
    }

    /// The pane is off: let go of the read out (its answer is never shown) and of the subject. Nothing waits.
    pub fn close(&mut self) {
        self.source = None;
        self.chosen = None;
        self.shown = Shown::Nothing;
        self.work.with(Work::drop_all);
    }

    fn rewind(&mut self) {
        self.page.rewind();
        self.pan = 0;
    }

    /// Ask for what the pane wants: a kept answer stands at once, and a fresh read goes out either way.
    fn ask(&mut self) {
        let Some(want) = self.want() else {
            self.shown = Shown::Nothing;
            self.work.with(|w| {
                w.wanted = None;
                w.set_current(None);
            });
            return;
        };
        let kept = self.kept.iter().find(|(id, _)| *id == want.id).map(|(_, l)| l.clone());
        match kept {
            Some(l) => self.stand(l),
            // Another subject or scope's lines must not stand under this one's title while it reads.
            None => self.shown = Shown::Reading { since: Instant::now(), said: false },
        }
        self.work.with(|w| w.ask(want));
    }

    fn stand(&mut self, l: Arc<Loaded>) {
        self.offers = l.offers.clone();
        self.at = l.scope;
        self.shown = Shown::Loaded(l);
    }

    fn keep(&mut self, id: WantId, l: Arc<Loaded>) {
        self.kept.retain(|(k, _)| *k != id);
        self.kept.push_front((id, l));
        self.kept.truncate(KEPT);
    }

    /// Take what the threads answered — the step of the owner's loop. Gives whether the pane changed.
    pub fn follow(&mut self) -> bool {
        let mut changed = false;
        if let Shown::Reading { since, said } = &mut self.shown
            && !*said
            && since.elapsed() >= SAY_READING_AFTER
        {
            *said = true;
            changed = true;
        }
        let (got, painted) = self.work.with(Work::take);
        let now = self.want().map(|w| w.id);
        for (id, loaded) in got {
            self.keep(id.clone(), loaded.clone());
            if Some(&id) == now.as_ref() {
                // Never rewinds — a reread of the same scope keeps the scroll; the next frame clamps it.
                self.stand(loaded);
                changed = true;
            }
        }
        changed | painted
    }

    /// Whether a read or colouring is out — the loop wakes fast to take it.
    pub fn busy(&self) -> bool {
        matches!(self.shown, Shown::Reading { said: false, .. }) || self.work.with(|w| w.busy())
    }

    /// The answer shown, if any.
    pub fn loaded(&self) -> Option<&Arc<Loaded>> {
        match &self.shown {
            Shown::Loaded(l) => Some(l),
            _ => None,
        }
    }

    /// The text room of a frame `width` cells wide in that layout — numbers and marks taken off.
    fn text_room(digits: usize, layout: Layout, width: usize) -> usize {
        match layout {
            Layout::Unified => width.saturating_sub(2 * digits + 3),
            Layout::Split => (width.saturating_sub(1) / 2).saturating_sub(digits + 2),
        }
    }

    /// The layout a pane this wide draws — side by side only from [`SPLIT_MIN`] on.
    pub fn fits(layout: Layout, width: usize) -> Layout {
        if layout == Layout::Split && width < SPLIT_MIN { Layout::Unified } else { layout }
    }

    /// Measure a frame before drawing it: `layout` is what is drawn (after [`View::fits`]). A layout other than the
    /// last frame's keeps the file at the top in view; the scroll and the sideways scroll are cut to the end.
    pub fn frame(&mut self, layout: Layout, width: usize, height: usize) {
        // Nothing to measure while reading — measuring "no rows" would cut a reread's scroll to the top before the
        // answer came.
        let Shown::Loaded(l) = &self.shown else {
            self.drawn = layout;
            return;
        };
        let l = l.clone();
        if layout != self.drawn {
            let top = l.rows(self.drawn).get(self.page.offset()).copied();
            let file = top.and_then(|r| l.part_of(r));
            self.drawn = layout;
            self.page.fit(height, l.len(layout));
            if let Some(&head) = file.and_then(|i| l.heads(layout).get(i)) {
                self.page.go(Move::Top);
                self.page.by(head as isize);
            }
        }
        self.page.fit(height, l.len(layout));
        self.room = View::text_room(l.digits, layout, width);
        self.pan = self.pan.min(l.widest.saturating_sub(self.room));
    }

    /// The furthest the text scrolls sideways.
    fn pan_end(&self) -> usize {
        self.loaded().map_or(0, |l| l.widest.saturating_sub(self.room))
    }

    /// Scroll sideways by `by` cells, within the widest line.
    pub fn pan_by(&mut self, by: isize) {
        self.pan = self.pan.saturating_add_signed(by).min(self.pan_end());
    }

    /// One wheel step.
    pub fn roll(&mut self, by: isize) {
        self.page.by(by);
    }

    /// One key of the pane's table ([`keys::DIFF`]) — the owner hands it over while the pane has the focus.
    pub fn key(&mut self, act: keys::Diff) {
        use keys::Diff as D;
        match act {
            D::Step(m) => self.page.go(m),
            D::NextFile => self.jump(false),
            D::PrevFile => self.jump(true),
            D::NextScope => self.step_scope(1),
            D::PrevScope => self.step_scope(-1),
            D::Pan(toward) => {
                let half = (self.room / 2).max(1) as isize;
                self.pan_by(if toward == Toward::Left { -half } else { half });
            }
        }
    }

    /// `Tab`·`Shift-Tab` — the next or the previous file's head to the top.
    fn jump(&mut self, back: bool) {
        let Some(l) = self.loaded() else { return };
        let top = self.page.offset();
        let heads = l.heads(self.drawn);
        let to = if back { heads.iter().rev().find(|&&h| h < top) } else { heads.iter().find(|&&h| h > top) };
        if let Some(&to) = to {
            self.page.go(Move::Top);
            self.page.by(to as isize);
        }
    }

    /// `h`·`l` — the scope before or after. With one scope nothing happens; the ends do not wrap.
    fn step_scope(&mut self, by: isize) {
        if self.offers.len() < 2 {
            return;
        }
        let Some(at) = self.offers.iter().position(|o| Some(o.scope) == self.at) else { return };
        let Some(to) = at.checked_add_signed(by).filter(|&i| i < self.offers.len()) else { return };
        self.chosen = Some(self.offers[to].scope);
        self.at = self.chosen;
        self.rewind();
        self.ask();
    }
}

/// The screen word of a scope.
pub fn scope_word(scope: Scope, lang: Lang) -> &'static str {
    match scope {
        Scope::Commits => say(lang, "tui.diff.scope.commits"),
        Scope::Branch => say(lang, "tui.diff.scope.branch"),
        Scope::Working => say(lang, "tui.diff.scope.working"),
    }
}

/// The title's words: the scopes (the shown one in brackets when there are more), whose they are, the file count and
/// the layout — and that the pane fell back to unified when it is too narrow for the person's side-by-side.
pub fn title(v: &View, chosen: Layout, drawn: Layout, lang: Lang) -> String {
    let mut parts = vec![say(lang, "tui.diff.title").to_string()];
    let (offers, at) = v.offers();
    match offers {
        [] => {
            if let Some(Source::Checkout { checkout, .. }) = v.source() {
                parts.push(say(lang, "tui.diff.scope.whole").to_string());
                if !checkout.branch.is_empty() {
                    parts.push(format!("{} {}", style::BRANCH_GLYPH, crate::text::one_line(&checkout.branch)));
                }
            }
        }
        [one] => parts.push(scope_word(one.scope, lang).to_string()),
        many => parts.push(
            many.iter()
                .map(|o| {
                    let w = scope_word(o.scope, lang);
                    if Some(o.scope) == at { format!("[{w}]") } else { w.to_string() }
                })
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
    if let Some(o) = offers.iter().find(|o| Some(o.scope) == at) {
        if let Some(b) = &o.branch {
            parts.push(format!("{} {}", style::BRANCH_GLYPH, crate::text::one_line(b)));
        }
        if let Some(owner) = &o.owner {
            parts.push(fill(say(lang, "tui.diff.of"), &[("id", &crate::text::one_line(owner))]));
        }
    }
    if let Some(l) = v.loaded()
        && l.failed.is_none()
        && !l.unchanged
    {
        let n = l.total.to_string();
        parts.push(if l.list_cut {
            fill(say(lang, "tui.diff.files_of"), &[("shown", &l.parts.len().to_string()), ("n", &n)])
        } else {
            fill(say(lang, "tui.diff.files"), &[("n", &n)])
        });
    }
    if chosen == drawn {
        parts.push(drawn.said(lang).to_string());
    } else {
        parts.push(say(lang, "tui.diff.too_narrow").to_string());
    }
    parts.join(" · ")
}

/// The one line the pane says when it has no rows: reading, why it failed, unchanged, no changes.
pub fn said(v: &View, lang: Lang) -> Option<(String, Style)> {
    let warn = super::draw::from_anstyle(style::WARN);
    match &v.shown {
        Shown::Reading { said: false, .. } => None,
        Shown::Reading { said: true, .. } => Some((say(lang, "tui.diff.reading").to_string(), super::draw::dim())),
        Shown::Nothing => Some((say(lang, "tui.diff.empty").to_string(), super::draw::dim())),
        Shown::Loaded(l) => {
            if let Some(f) = &l.failed {
                let words = match f.kind {
                    "no_git" => say(lang, "tui.diff.why.no_git").to_string(),
                    "not_a_repo" => say(lang, "tui.diff.why.not_a_repo").to_string(),
                    "timeout" => say(lang, "tui.diff.why.timeout").to_string(),
                    _ => fill(say(lang, "tui.diff.why.failed"), &[("why", &f.said)]),
                };
                return Some((words, warn));
            }
            if l.parts.is_empty() {
                let words = if l.unchanged { say(lang, "tui.diff.unchanged") } else { say(lang, "tui.diff.empty") };
                return Some((words.to_string(), super::draw::dim()));
            }
            None
        }
    }
}

/// The colours of a line's kind: the number columns and the mark, and the text when it has no syntax colours.
fn tone(kind: LineKind) -> Style {
    match kind {
        LineKind::Add => super::draw::from_anstyle(style::DIFF_ADD),
        LineKind::Delete => super::draw::from_anstyle(style::DIFF_DEL),
        LineKind::Context | LineKind::Raw => super::draw::dim(),
    }
}

fn mark(kind: LineKind) -> &'static str {
    match kind {
        LineKind::Add => "+",
        LineKind::Delete => "-",
        LineKind::Context | LineKind::Raw => " ",
    }
}

/// The change kind's letter — the CLI's (`moai diff --stat`).
pub fn letter(status: Status) -> &'static str {
    match status {
        Status::Added => "A",
        Status::Modified => "M",
        Status::Deleted => "D",
        Status::Renamed => "R",
        Status::Copied => "C",
        Status::TypeChanged => "T",
        Status::Untracked => "?",
        Status::Conflicted => "U",
    }
}

/// Spans cut to `room` cells, `…` where they stop short — for the heads, which do not scroll sideways.
fn clipped(spans: Vec<(String, Style)>, room: usize) -> Vec<Span<'static>> {
    let whole: usize = spans.iter().map(|(t, _)| columns::width(t)).sum();
    let (room, more) = if whole > room { (room.saturating_sub(1), true) } else { (room, false) };
    let mut out = Vec::new();
    let mut used = 0;
    for (text, look) in spans {
        if used >= room {
            break;
        }
        let cut = columns::cut(&text, &[], look, 0, room - used);
        used += cut.iter().map(|s| columns::width(&s.content)).sum::<usize>();
        out.extend(cut);
    }
    if more {
        out.push(Span::styled("…", super::draw::dim()));
    }
    out
}

/// A line's text cut to `room` cells from `pan`: its syntax colours on the plain style, or — without colours — the
/// kind's colour whole. `‹` stands before it when scrolled and the line has text; `›` after it when more is to the right.
fn text_spans(part: &Part, h: usize, i: usize, pan: usize, room: usize, lang: Lang) -> (bool, Vec<Span<'static>>) {
    let line = &part.file.hunks[h].lines[i];
    let mut paint: Vec<(std::ops::Range<usize>, Style)> = Vec::new();
    let base = match part.spans(h, i) {
        Some(spans) => {
            paint.extend(spans.iter().map(|s| (s.range.clone(), super::draw::from_anstyle(style::syntax(s.kind)))));
            Style::default()
        }
        None if line.kind == LineKind::Context || line.kind == LineKind::Raw => Style::default(),
        None => tone(line.kind),
    };
    let text: std::borrow::Cow<str> = if line.no_newline {
        let at = line.text.len();
        let t = format!("{} {}", line.text, no_newline_note(lang));
        paint.push((at..t.len(), super::draw::dim()));
        t.into()
    } else {
        line.text.as_str().into()
    };
    let wide = columns::width(&text);
    let more = wide > pan + room;
    let mut spans = columns::cut(&text, &paint, base, pan, if more { room.saturating_sub(1) } else { room });
    if more && room > 0 {
        spans.push(Span::styled("›", super::draw::dim()));
    }
    (pan > 0 && wide > 0, spans)
}

fn number(n: Option<usize>, digits: usize) -> String {
    n.map_or_else(|| " ".repeat(digits), |n| format!("{n:>digits$}"))
}

/// One half of a side-by-side row, exactly `w` cells: its number, the mark, the text — or blanks when the other half
/// has the line alone.
fn half(
    l: &Loaded,
    part: &Part,
    h: usize,
    at: Option<u32>,
    old: bool,
    w: usize,
    pan: usize,
    lang: Lang,
) -> Vec<Span<'static>> {
    let Some(i) = at.map(|i| i as usize) else { return vec![Span::raw(" ".repeat(w))] };
    let line = &part.file.hunks[h].lines[i];
    let look = tone(line.kind);
    let no = if old { line.old_no } else { line.new_no };
    let room = w.saturating_sub(l.digits + 2);
    let (scrolled, text) = text_spans(part, h, i, pan, room, lang);
    let mut out = vec![
        Span::styled(number(no, l.digits), look),
        Span::styled(if scrolled { "‹" } else { " " }, super::draw::dim()),
        Span::styled(mark(line.kind), look),
    ];
    out.extend(text);
    let used: usize = out.iter().map(|s| columns::width(&s.content)).sum();
    if used < w {
        out.push(Span::raw(" ".repeat(w - used)));
    }
    out
}

/// The rows `top..top + height` of an answer in a pane `width` cells wide, scrolled `pan` cells — **pure**: the
/// drawing calls it with what is on screen, and the tests read its text without a terminal.
pub fn render(
    l: &Loaded,
    layout: Layout,
    width: usize,
    top: usize,
    height: usize,
    pan: usize,
    lang: Lang,
) -> Vec<Line<'static>> {
    let rows = l.rows(layout);
    let d = l.digits;
    let mut out = Vec::new();
    for &row in rows.iter().skip(top).take(height) {
        let line = match row {
            Row::File(i) => {
                let part = &l.parts[i as usize];
                let c = &part.change;
                let head = super::draw::from_anstyle(style::HEAD);
                let mut spans = vec![
                    (letter(c.status).to_string(), super::draw::from_anstyle(style::diff_status(c.status)).patch(head)),
                    (" ".into(), Style::default()),
                ];
                match (&c.old_path, c.status) {
                    (Some(old), Status::Renamed | Status::Copied) => {
                        spans.push((old.clone(), head));
                        spans.push((" → ".into(), super::draw::dim()));
                        spans.push((c.path.clone(), head));
                    }
                    _ => spans.push((c.path.clone(), head)),
                }
                if c.binary {
                    spans.push(("  ".into(), Style::default()));
                    spans.push((say(lang, "tui.diff.binary_word").to_string(), super::draw::dim()));
                } else if let (Some(a), Some(r)) = (c.adds, c.dels) {
                    spans.push(("  ".into(), Style::default()));
                    spans.push((format!("+{a}"), super::draw::from_anstyle(style::DIFF_ADD)));
                    spans.push((" ".into(), Style::default()));
                    spans.push((format!("−{r}"), super::draw::from_anstyle(style::DIFF_DEL)));
                }
                Line::from(clipped(spans, width))
            }
            Row::Note(i) => {
                let part = &l.parts[i as usize];
                let why = if part.file.binary || part.change.binary {
                    say(lang, "tui.diff.binary")
                } else {
                    say(lang, "tui.diff.no_lines")
                };
                Line::from(clipped(vec![(format!("  {why}"), super::draw::dim())], width))
            }
            Row::Hunk(i, h) => {
                let hunk = &l.parts[i as usize].file.hunks[h as usize];
                let text = if hunk.section.starts_with("@@@") {
                    hunk.section.clone()
                } else {
                    let head =
                        format!("@@ -{},{} +{},{} @@", hunk.old_start, hunk.old_len, hunk.new_start, hunk.new_len);
                    if hunk.section.is_empty() { head } else { format!("{head} {}", hunk.section) }
                };
                Line::from(clipped(vec![(text, super::draw::from_anstyle(style::DIFF_HUNK))], width))
            }
            Row::One(i, h, n) => {
                let part = &l.parts[i as usize];
                let (h, n) = (h as usize, n as usize);
                let line = &part.file.hunks[h].lines[n];
                let look = tone(line.kind);
                let room = width.saturating_sub(2 * d + 3);
                let (scrolled, text) = text_spans(part, h, n, pan, room, lang);
                let mut spans = if line.kind == LineKind::Raw {
                    vec![Span::raw(" ".repeat(2 * d + 1))]
                } else {
                    vec![
                        Span::styled(number(line.old_no, d), look),
                        Span::raw(" "),
                        Span::styled(number(line.new_no, d), look),
                    ]
                };
                spans.push(Span::styled(if scrolled { "‹" } else { " " }, super::draw::dim()));
                spans.push(Span::styled(mark(line.kind), look));
                spans.extend(text);
                Line::from(spans)
            }
            Row::Pair(i, h, old, new) => {
                let part = &l.parts[i as usize];
                let left = width.saturating_sub(1) / 2;
                let right = width.saturating_sub(1 + left);
                let mut spans = half(l, part, h as usize, old, true, left, pan, lang);
                spans.push(Span::styled("│", super::draw::dim()));
                spans.extend(half(l, part, h as usize, new, false, right, pan, lang));
                Line::from(spans)
            }
            Row::Cut => {
                let said = fill(
                    say(lang, "tui.diff.cut"),
                    &[("shown", &l.reached().to_string()), ("n", &l.total.max(l.parts.len()).to_string())],
                );
                Line::from(clipped(vec![(said, super::draw::from_anstyle(style::WARN))], width))
            }
        };
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse;
    use ratatui::style::Color;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const EN: Lang = Lang::En;

    /// A part as the read layer would give it for a parsed file.
    fn part(f: FileDiff) -> Part {
        let status = match f.kind {
            FileKind::Added => Status::Added,
            FileKind::Deleted => Status::Deleted,
            FileKind::Renamed => Status::Renamed,
            FileKind::Copied => Status::Copied,
            FileKind::Modified | FileKind::ModeChanged => Status::Modified,
        };
        let count = |k| f.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == k).count();
        let change = Change {
            path: f.new_path.clone().or_else(|| f.old_path.clone()).unwrap_or_default(),
            old_path: matches!(f.kind, FileKind::Renamed | FileKind::Copied).then(|| f.old_path.clone()).flatten(),
            status,
            adds: (!f.binary).then(|| count(LineKind::Add)),
            dels: (!f.binary).then(|| count(LineKind::Delete)),
            binary: f.binary,
        };
        Part::new(change, f)
    }

    fn loaded(patch: &str) -> Loaded {
        let parts: Vec<Part> = parse(patch).into_iter().map(part).collect();
        let mut l = Loaded { total: parts.len(), parts, ..Loaded::default() };
        l.measure(EN);
        l
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    fn all(l: &Loaded, layout: Layout, width: usize, pan: usize) -> Vec<String> {
        texts(&render(l, layout, width, 0, l.len(layout), pan, EN))
    }

    fn cells(s: &str) -> usize {
        unicode_width::UnicodeWidthStr::width(s)
    }

    /// The style of the cell at column `x` of a rendered line.
    fn style_at(line: &Line, x: usize) -> Style {
        let mut col = 0;
        for s in &line.spans {
            for c in s.content.chars() {
                let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if col == x || (w == 2 && col + 1 == x) {
                    return s.style;
                }
                col += w;
            }
        }
        panic!("no cell {x} in {line:?}")
    }

    const PATCH: &str = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -10,4 +10,2 @@ fn f()\n keep\n-one\n-two\n-three\n+new\n";

    /// **Unified: `+`·`-` and the parser's numbers stand in plain text** — with every colour taken away the kind is
    /// still read, and the old and new numbers are the parser's.
    #[test]
    fn unified_lines_carry_their_marks_and_the_parsers_numbers() {
        let l = loaded(PATCH);
        let got = all(&l, Layout::Unified, 60, 0);
        assert_eq!(got[0], "M src/a.rs  +1 −3");
        assert_eq!(got[1], "@@ -10,4 +10,2 @@ fn f()");
        assert_eq!(got[2].trim_end(), "10 10  keep");
        assert_eq!(got[3].trim_end(), "11    -one");
        assert_eq!(got[5].trim_end(), "13    -three");
        assert_eq!(got[6].trim_end(), "   11 +new");
        let lines = &l.parts[0].file.hunks[0].lines;
        assert_eq!((lines[1].old_no, lines[1].new_no), (Some(11), None));
        assert_eq!((lines[4].old_no, lines[4].new_no), (None, Some(11)));
    }

    /// **A rename stands `old → new`; a binary file has a head and a reason, no lines.**
    #[test]
    fn a_rename_says_both_names_and_a_binary_file_has_no_lines() {
        let patch = "diff --git a/old.rs b/new.rs\nsimilarity index 90%\nrename from old.rs\nrename to new.rs\n--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/pic.png b/pic.png\nBinary files a/pic.png and b/pic.png differ\n";
        let l = loaded(patch);
        let got = all(&l, Layout::Unified, 60, 0);
        assert_eq!(got[0], "R old.rs → new.rs  +1 −1");
        let at = got.iter().position(|g| g.starts_with("M pic.png")).expect("no binary head");
        assert_eq!(got[at + 1].trim(), say(EN, "tui.diff.binary"));
        assert_eq!(got.len(), at + 2, "a binary file grew lines: {got:?}");
    }

    /// **Side by side: three removed and one added are three rows**, the added line opposite the first removed and
    /// blanks opposite the other two.
    #[test]
    fn three_removed_one_added_pair_into_three_rows() {
        let l = loaded(PATCH);
        let got = all(&l, Layout::Split, 81, 0);
        let rows: Vec<&String> = got.iter().skip(3).collect();
        assert_eq!(rows.len(), 3, "{got:?}");
        let (left, right) = (40, 40);
        let halves = |r: &str| {
            let mid = r.char_indices().find(|&(i, _)| cells(&r[..i]) == left).map(|(i, _)| i).unwrap();
            (r[..mid].to_string(), r[mid + '│'.len_utf8()..].to_string())
        };
        let (l0, r0) = halves(rows[0]);
        assert!(l0.contains("-one") && r0.contains("+new"), "{l0:?} | {r0:?}");
        for row in &rows[1..] {
            let (l, r) = halves(row);
            assert!(l.contains('-'), "{l:?}");
            assert_eq!(r, " ".repeat(right), "the other side is not blank");
        }
    }

    /// **The middle line stands in the same column on every row** — Hangul takes two cells, and a cut by bytes or chars
    /// would move it.
    #[test]
    fn the_middle_line_holds_its_column_through_hangul() {
        let long = "가".repeat(60);
        let patch = format!(
            "diff --git a/k.txt b/k.txt\n--- a/k.txt\n+++ b/k.txt\n@@ -1,3 +1,3 @@\n 한글 문맥\n-옛 {long}\n+새 줄\n-abc\n+{long}x\n"
        );
        let l = loaded(&patch);
        for pan in [0, 1, 3, 7] {
            for row in all(&l, Layout::Split, 90, pan).iter().skip(2) {
                let at = row.find('│').unwrap_or_else(|| panic!("no middle in {row:?}"));
                assert_eq!(cells(&row[..at]), 44, "pan {pan}: {row:?}");
                assert_eq!(cells(row), 90, "pan {pan}: {row:?}");
            }
        }
    }

    /// **A pane narrower than 80 draws unified and says so; the setting stays.**
    #[test]
    fn a_narrow_pane_falls_back_to_unified_and_says_so() {
        assert_eq!(View::fits(Layout::Split, SPLIT_MIN - 1), Layout::Unified);
        assert_eq!(View::fits(Layout::Split, SPLIT_MIN), Layout::Split);
        let mut v = shown_with(loaded(PATCH));
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 12)).unwrap();
        let chosen = Layout::Split;
        term.draw(|f| super::super::draw::diff_pane(f, &mut v, f.area(), true, chosen, "", EN)).unwrap();
        let buf = term.backend().buffer();
        let row = |y: u16| (0..60u16).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>();
        assert!(row(0).contains("unified — too narrow"), "{}", row(0));
        assert!(!row(4).contains('│'), "a half stood in a narrow pane: {}", row(4));
        assert!(row(4).contains("11    -one"), "{}", row(4));
        assert_eq!(chosen, Layout::Split);
        assert_eq!(v.drawn, Layout::Unified);
    }

    /// A view standing on one answer, with no thread behind it.
    fn shown_with(l: Loaded) -> View {
        let mut v = View::with(Arc::new(|_, _, _, _| Loaded::default()), Arc::new(|_| None), EN);
        v.shown = Shown::Loaded(Arc::new(l));
        v
    }

    fn long_patch() -> String {
        let long: String = (0..30).map(|i| format!("w{i:02}\t")).collect::<String>() + "END";
        format!("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n-short\n+{long}\n same\n")
    }

    /// **The end of a long line is reachable, the numbers and marks stay put, the scroll stops at the widest end** —
    /// tabs spread to four cells in the measure as in the cut.
    #[test]
    fn panning_reaches_the_end_and_keeps_numbers_and_marks() {
        let mut v = shown_with(loaded(&long_patch()));
        let widest = v.loaded().unwrap().widest;
        assert_eq!(widest, 30 * 4 + 3, "tabs were not measured at four cells");
        v.frame(Layout::Unified, 40, 10);
        for _ in 0..100 {
            v.key(keys::Diff::Pan(Toward::Right));
        }
        assert_eq!(v.pan, widest - v.room, "the scroll ran past the widest line's end");
        let l = v.loaded().unwrap().clone();
        let got = texts(&render(&l, Layout::Unified, 40, 0, 10, v.pan, EN));
        let added = got.iter().find(|g| g.contains("‹+")).unwrap_or_else(|| panic!("{got:?}"));
        assert!(added.ends_with("END"), "the end is not seen: {added:?}");
        assert!(added.starts_with("  1‹+"), "the number or mark scrolled away: {added:?}");
        // `l` is the scope, not sideways; `L` is sideways, not the scope.
        let pan = v.pan;
        v.key(keys::Diff::NextScope);
        assert_eq!(v.pan, pan, "l scrolled sideways");
        v.key(keys::Diff::Pan(Toward::Left));
        assert!(v.pan < pan);
        assert!(v.chosen.is_none(), "L changed the scope");
    }

    /// **Both halves scroll together.**
    #[test]
    fn side_by_side_halves_pan_together() {
        let patch = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-0123456789abcdefghijklmnopqrstuvwxyzOLD\n+0123456789abcdefghijklmnopqrstuvwxyzNEW\n";
        let l = loaded(patch);
        let got = all(&l, Layout::Split, 80, 10);
        let row = &got[2];
        let (left, right) = row.split_once('│').unwrap();
        assert!(left.contains("‹-abcdef") && right.contains("‹+abcdef"), "{row:?}");
    }

    fn painted(mut l: Loaded) -> Loaded {
        for p in &mut l.parts {
            let paint = highlight::diff(&p.file);
            p.paint = OnceLock::from(paint);
        }
        l
    }

    /// **With syntax colours, the numbers and the mark keep the added and removed colours and the syntax colour lands
    /// on its own letters** — after the mark, a tab, Hangul, and a sideways scroll.
    #[test]
    fn syntax_colour_lands_on_its_letters_beside_the_kind_colours() {
        let patch =
            "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-\t// 옛 주석\n+\tlet 한 = \"값\"; // 새\n";
        let l = painted(loaded(patch));
        assert!(l.parts[0].painted());
        let lines = render(&l, Layout::Unified, 60, 0, 10, 0, EN);
        let (del, add) = (&lines[2], &lines[3]);
        let red = Style::new().fg(Color::Red);
        let green = Style::new().fg(Color::Green);
        assert_eq!(style_at(del, 0), red, "the old number");
        assert_eq!(style_at(del, 4), red, "the - mark");
        assert_eq!(style_at(add, 2), green, "the new number");
        assert_eq!(style_at(add, 4), green, "the + mark");
        // `let` sits after the 4-cell tab: prefix 5 (2·1 + 3) + 4.
        let keyword = super::super::draw::from_anstyle(style::syntax(highlight::Kind::Keyword));
        assert_eq!(style_at(add, 9), keyword, "{add:?}");
        let text: String = add.spans.iter().map(|s| s.content.as_ref()).collect();
        let at = text.find('"').unwrap();
        let string = super::super::draw::from_anstyle(style::syntax(highlight::Kind::String));
        assert_eq!(style_at(add, cells(&text[..at])), string, "the string after Hangul shifted: {add:?}");
        let comment = super::super::draw::from_anstyle(style::syntax(highlight::Kind::Comment));
        assert_eq!(style_at(del, 9), comment);
        // Scrolled two cells, `let` starts two cells earlier.
        let panned = render(&l, Layout::Unified, 60, 0, 10, 2, EN);
        assert_eq!(style_at(&panned[3], 7), keyword, "{:?}", panned[3]);
    }

    /// **Side by side, each half takes its own line's colours** — never the other half's.
    #[test]
    fn each_half_takes_its_own_lines_colours() {
        let mut l = loaded("diff --git a/a.x b/a.x\n--- a/a.x\n+++ b/a.x\n@@ -1 +1 @@\n-aaaa\n+bbbb\n");
        let spans = vec![vec![
            vec![highlight::Span { kind: highlight::Kind::Comment, range: 0..4 }],
            vec![highlight::Span { kind: highlight::Kind::Number, range: 0..4 }],
        ]];
        l.parts[0].paint = OnceLock::from(Some(spans));
        let line = &render(&l, Layout::Split, 80, 2, 1, 0, EN)[0];
        let comment = super::super::draw::from_anstyle(style::syntax(highlight::Kind::Comment));
        let number = super::super::draw::from_anstyle(style::syntax(highlight::Kind::Number));
        assert_eq!(style_at(line, 3), comment, "{line:?}");
        assert_eq!(style_at(line, 39 + 1 + 3), number, "{line:?}");
    }

    /// **A file with no colours takes the added or removed colour whole.**
    #[test]
    fn a_file_without_colours_is_coloured_by_its_kind() {
        let mut l = loaded("diff --git a/a.zz b/a.zz\n--- a/a.zz\n+++ b/a.zz\n@@ -1 +1 @@\n-aaaa\n+bbbb\n");
        l.parts[0].paint = OnceLock::from(None);
        let lines = render(&l, Layout::Unified, 40, 0, 10, 0, EN);
        assert_eq!(style_at(&lines[2], 6), Style::new().fg(Color::Red));
        assert_eq!(style_at(&lines[3], 6), Style::new().fg(Color::Green));
    }

    /// **Nothing raw reaches the terminal** — an ESC in a line or a path is drawn as `?`.
    #[test]
    fn control_characters_in_a_patch_never_reach_the_terminal() {
        let l = loaded(
            "diff --git a/e\x1b]0;x\x07 b/e\x1b]0;x\x07\n--- a/e\n+++ b/e\n@@ -1 +1 @@ \x1b[2J\n-\x1b[31mred\n+ok\r\n",
        );
        for layout in [Layout::Unified, Layout::Split] {
            for row in all(&l, layout, 90, 0) {
                assert!(!row.chars().any(char::is_control), "{row:?}");
            }
        }
    }

    /// **Cut is said**: a patch cut at the limit ends in a line naming how far it came; a list cut says the whole
    /// count in the title.
    #[test]
    fn a_cut_patch_and_a_cut_list_say_so() {
        let mut l = loaded(PATCH);
        l.patch_cut = true;
        l.total = 7;
        l.measure(EN);
        let got = all(&l, Layout::Unified, 80, 0);
        assert_eq!(got.last().unwrap(), &fill(say(EN, "tui.diff.cut"), &[("shown", "1"), ("n", "7")]));
        l.list_cut = true;
        let v = shown_with(l);
        let title = title(&v, Layout::Unified, Layout::Unified, EN);
        assert!(title.contains("1 of 7 files"), "{title}");
    }

    /// **No git and not a repository are said apart, and a scope with no change is not an empty pane.**
    #[test]
    fn failures_and_the_empty_scope_have_their_own_words() {
        let said_of = |l: Loaded| said(&shown_with(l), EN).map(|(t, _)| t);
        let no_git = said_of(Loaded::failed("no_git", "x".into()));
        let not_repo = said_of(Loaded::failed("not_a_repo", "x".into()));
        assert!(no_git.is_some() && no_git != not_repo, "{no_git:?} {not_repo:?}");
        assert_eq!(said_of(Loaded::default()).as_deref(), Some(say(EN, "tui.diff.empty")));
        let unchanged = Loaded { unchanged: true, ..Loaded::default() };
        assert_eq!(said_of(unchanged).as_deref(), Some(say(EN, "tui.diff.unchanged")));
    }

    /// **`Tab` brings the next file's head to the top, `Shift-Tab` the one before** — in either layout.
    #[test]
    fn tab_jumps_between_file_heads() {
        let two = format!("{PATCH}diff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n-x\n+y\n");
        let mut v = shown_with(loaded(&two));
        v.frame(Layout::Unified, 80, 3);
        v.key(keys::Diff::NextFile);
        let l = v.loaded().unwrap().clone();
        assert_eq!(l.rows(Layout::Unified)[v.page.offset()], Row::File(1));
        v.key(keys::Diff::PrevFile);
        assert_eq!(v.page.offset(), 0);
        // Turning the layout keeps that file at the top.
        v.key(keys::Diff::NextFile);
        v.frame(Layout::Split, 80, 3);
        assert_eq!(l.rows(Layout::Split)[v.page.offset()], Row::File(1));
    }

    fn checkout(path: &str) -> Checkout {
        Checkout { key: ".".into(), path: PathBuf::from(path), branch: "dev".into(), main: true, milestone: None }
    }

    fn source(path: &str) -> Option<Source> {
        Some(Source::Checkout { checkout: checkout("/w/a"), path: Some(path.into()) })
    }

    fn settle(v: &mut View) {
        let began = Instant::now();
        loop {
            v.follow();
            if !v.busy() {
                return;
            }
            assert!(began.elapsed() < Duration::from_secs(30), "the pane kept waiting");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// A reader that counts its calls, waits while `hold` is up, and marks its answer with the call's number and the
    /// subject's path (in `total` and the one part's path).
    fn counting(hold: Arc<AtomicBool>, calls: Arc<AtomicUsize>) -> Arc<ReadFn> {
        Arc::new(move |src: &Source, scope, _, _| {
            let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
            while hold.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            let Source::Checkout { path, .. } = src else { unreachable!() };
            let p = path.clone().unwrap_or_default();
            let mut l = loaded(&format!("diff --git a/{p} b/{p}\n--- a/{p}\n+++ b/{p}\n@@ -1 +1 @@\n-a\n+b\n"));
            l.total = n;
            l.scope = scope;
            l
        })
    }

    fn shown_path(v: &View) -> Option<(String, usize)> {
        v.loaded().map(|l| (l.parts.first().map(|p| p.change.path.clone()).unwrap_or_default(), l.total))
    }

    /// **A hundred wants while one read runs start two reads, not a hundred**, the newest one — and the keys never wait.
    #[test]
    fn a_hundred_wants_make_two_reads() {
        let (hold, calls) = (Arc::new(AtomicBool::new(true)), Arc::new(AtomicUsize::new(0)));
        let mut v = View::with(counting(hold.clone(), calls.clone()), Arc::new(|_| None), EN);
        for i in 0..100 {
            v.show(source(&format!("f{i}.rs")));
            v.follow();
            let began = Instant::now();
            v.key(keys::Diff::Step(Move::LineDown));
            v.key(keys::Diff::Pan(Toward::Right));
            assert!(began.elapsed() < Duration::from_millis(50), "a key waited on the read");
        }
        hold.store(false, Ordering::SeqCst);
        settle(&mut v);
        assert_eq!(calls.load(Ordering::SeqCst), 2, "the reads between were not dropped");
        assert_eq!(shown_path(&v), Some(("f99.rs".into(), 2)));
    }

    /// **A slow first answer coming after the fast second one does not stand in its place.**
    #[test]
    fn a_late_answer_for_an_old_want_does_not_stand() {
        let (hold, calls) = (Arc::new(AtomicBool::new(true)), Arc::new(AtomicUsize::new(0)));
        let mut v = View::with(counting(hold.clone(), calls.clone()), Arc::new(|_| None), EN);
        v.show(source("slow.rs"));
        v.show(source("fast.rs"));
        hold.store(false, Ordering::SeqCst);
        settle(&mut v);
        assert_eq!(shown_path(&v).map(|p| p.0).as_deref(), Some("fast.rs"));
        // The cursor coming back shows the kept copy at once, and reads again.
        v.show(source("slow.rs"));
        assert_eq!(shown_path(&v).map(|p| p.0).as_deref(), Some("slow.rs"), "the kept copy did not stand at once");
        settle(&mut v);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    /// **An answer coming after the pane closed never stands — not even once it opens again on the same subject.**
    #[test]
    fn an_answer_after_closing_never_stands() {
        let (hold, calls) = (Arc::new(AtomicBool::new(true)), Arc::new(AtomicUsize::new(0)));
        let mut v = View::with(counting(hold.clone(), calls.clone()), Arc::new(|_| None), EN);
        v.show(source("a.rs"));
        while calls.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
        v.close();
        v.show(source("a.rs"));
        hold.store(false, Ordering::SeqCst);
        settle(&mut v);
        assert_eq!(shown_path(&v), Some(("a.rs".into(), 2)), "the answer asked before closing stood");
    }

    /// **The colouring of one scope never lands on another's lines** — a slow colouring of the first is let go when the
    /// pane moves on, and the second's lines carry their own.
    #[test]
    fn colour_follows_its_own_answer() {
        let calls = Arc::new(AtomicUsize::new(0));
        let read = counting(Arc::new(AtomicBool::new(false)), calls);
        let hold = Arc::new(AtomicBool::new(true));
        let held = hold.clone();
        let paint: Arc<PaintFn> = Arc::new(move |f: &FileDiff| {
            let first = f.new_path.as_deref() == Some("one.rs");
            while first && held.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            let kind = if first { highlight::Kind::Comment } else { highlight::Kind::Number };
            Some(
                f.hunks
                    .iter()
                    .map(|h| h.lines.iter().map(|_| vec![highlight::Span { kind, range: 0..1 }]).collect())
                    .collect(),
            )
        });
        let mut v = View::with(read, paint, EN);
        v.show(source("one.rs"));
        let began = Instant::now();
        while v.loaded().is_none() {
            v.follow();
            assert!(began.elapsed() < Duration::from_secs(10));
        }
        v.show(source("two.rs"));
        hold.store(false, Ordering::SeqCst);
        settle(&mut v);
        let l = v.loaded().unwrap();
        assert_eq!(l.parts[0].change.path, "two.rs");
        assert_eq!(l.parts[0].spans(0, 0).unwrap()[0].kind, highlight::Kind::Number);
    }

    /// **A read that dies raises its panic on the loop**, as the reread does.
    #[test]
    fn a_dead_read_raises_its_panic() {
        let read: Arc<ReadFn> = Arc::new(|_, _, _, _| panic!("the read fell over"));
        let mut v = View::with(read, Arc::new(|_| None), EN);
        v.show(source("a.rs"));
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let began = Instant::now();
            while began.elapsed() < Duration::from_secs(10) {
                v.follow();
                std::thread::sleep(Duration::from_millis(1));
            }
        }));
        let why = caught.expect_err("the panic was swallowed");
        assert_eq!(why.downcast_ref::<&str>(), Some(&"the read fell over"));
    }

    /// **`h`·`l` walk the scopes; with one, they do nothing.** Switching rewinds the scroll and reads the new scope.
    #[test]
    fn scopes_step_with_h_and_l_and_one_scope_stays() {
        let offer = |scope| Offer { scope, checkout: None, branch: None, owner: None };
        let (hold, calls) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let read = counting(hold, calls.clone());
        let read: Arc<ReadFn> = Arc::new(move |s: &Source, scope, b, lang| {
            let mut l = read(s, scope, b, lang);
            l.offers = vec![offer(Scope::Commits), offer(Scope::Working)];
            l.scope = Some(scope.unwrap_or(Scope::Commits));
            l
        });
        let mut v = View::with(read, Arc::new(|_| None), EN);
        v.show(source("a.rs"));
        settle(&mut v);
        v.key(keys::Diff::PrevScope);
        settle(&mut v);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "h at the first scope read something");
        v.key(keys::Diff::NextScope);
        settle(&mut v);
        assert_eq!(v.offers().1, Some(Scope::Working));
        assert_eq!(v.loaded().unwrap().scope, Some(Scope::Working));
        let t = title(&v, Layout::Unified, Layout::Unified, EN);
        assert!(t.contains("commits [uncommitted]"), "{t}");
        // One scope: nothing.
        let mut one = shown_with(loaded(PATCH));
        one.offers = vec![offer(Scope::Working)];
        one.at = Some(Scope::Working);
        one.key(keys::Diff::NextScope);
        assert!(one.chosen.is_none() && matches!(one.shown, Shown::Loaded(_)));
    }

    /// **The colouring coming does not move the scroll.**
    #[test]
    fn colour_coming_keeps_the_scroll() {
        let calls = Arc::new(AtomicUsize::new(0));
        let read = counting(Arc::new(AtomicBool::new(false)), calls);
        let hold = Arc::new(AtomicBool::new(true));
        let held = hold.clone();
        let paint: Arc<PaintFn> = Arc::new(move |f: &FileDiff| {
            while held.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            highlight::diff(f)
        });
        let mut v = View::with(read, paint, EN);
        v.show(source("a.rs"));
        while v.loaded().is_none() {
            v.follow();
        }
        v.frame(Layout::Unified, 20, 1);
        v.key(keys::Diff::Step(Move::LineDown));
        v.key(keys::Diff::Pan(Toward::Right));
        let at = (v.page.offset(), v.pan);
        hold.store(false, Ordering::SeqCst);
        settle(&mut v);
        v.frame(Layout::Unified, 20, 1);
        assert!(v.loaded().unwrap().parts[0].painted());
        assert_eq!((v.page.offset(), v.pan), at);
    }

    /// **A secret file's diff stands like any other** — `.env` is read and drawn as `x.txt` is, through git's whole
    /// branch of a real checkout.
    #[test]
    fn a_secret_files_diff_stands_like_any_other() {
        use crate::git::tests::run_git;
        let dir = crate::scratch::Scratch::new("tui-diff-secret");
        let root: &std::path::Path = &dir;
        let git = |args: &[&str]| run_git(root, None, args);
        git(&["init", "-q"]);
        std::fs::write(root.join(".env"), "A=1\n").unwrap();
        std::fs::write(root.join("x.txt"), "A=1\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        std::fs::write(root.join(".env"), "A=2\n").unwrap();
        std::fs::write(root.join("x.txt"), "A=2\n").unwrap();
        let mut c = checkout("/");
        c.path = root.to_path_buf();
        let draw = |path: &str| {
            let l = whole(&c, Some(path), Some(BUDGET), EN);
            assert!(l.failed.is_none(), "{:?}", l.failed);
            let mut l = l;
            l.measure(EN);
            all(&l, Layout::Unified, 60, 0).into_iter().map(|r| r.replace(path, "<file>")).collect::<Vec<_>>()
        };
        let secret = draw(".env");
        assert_eq!(secret, draw("x.txt"));
        assert!(secret.iter().any(|r| r.contains("-A=1")) && secret.iter().any(|r| r.contains("+A=2")), "{secret:?}");
        // A file the branch did not change is said so.
        std::fs::write(root.join("same.txt"), "s\n").unwrap();
        git(&["add", "same.txt"]);
        git(&["commit", "-qm", "same"]);
        assert!(whole(&c, Some("same.txt"), Some(BUDGET), EN).unchanged);
    }

    /// **Not a repository is said as that**, through the whole-branch read.
    #[test]
    fn a_folder_outside_git_says_not_a_repository() {
        let base = crate::scratch::base();
        let mut c = checkout("/");
        c.path = base.to_path_buf();
        let l = whole(&c, None, Some(BUDGET), EN);
        assert_eq!(l.failed.map(|f| f.kind), Some("not_a_repo"));
    }

    /// **`SPC o s` stands only where a diff does**, and its menu line says the layout in a word.
    #[test]
    fn spc_o_s_stands_only_where_a_diff_does() {
        use super::super::keys::{Browse, Ctx};
        let off = Ctx { lang: EN, ..Ctx::default() };
        assert!(Browse::DiffLayout.enabled(&off).is_err());
        let on = Ctx { diff: true, ..off };
        assert!(Browse::DiffLayout.enabled(&on).is_ok());
        assert!(Browse::DiffLayout.enabled(&Ctx { files: true, ..on }).is_ok(), "over the file window's diff");
        let split = Ctx { diff_split: true, ..on };
        assert_eq!(Browse::DiffLayout.state(&split), Some(say(EN, "tui.diff.split").into()));
    }

    /// **`SPC o s` writes `diff_layout`; never pressed, nothing is written; an unknown word stands unified and stays.**
    #[test]
    fn the_layout_is_kept_only_once_pressed() {
        use super::super::App;
        use crate::config::Config;
        let s = crate::scratch::Scratch::new("tui-diff-layout");
        let user = s.path().join("user.toml");
        std::fs::write(&user, "[tui]\ndiff_layout = \"sideways\"\nmouse = true\n").unwrap();
        let cfg = || Config::parse("prefix = \"argos\"\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), crate::nav::Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        assert_eq!(a.diff_layout, Layout::Unified, "an unknown word did not stand unified");
        a.hit("SPC o m");
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("diff_layout = \"sideways\""), "a save that did not press SPC o s rewrote it: {text}");
        let rows = a.rows();
        a.run(super::super::keys::Browse::DiffLayout, &rows);
        assert_eq!(a.diff_layout, Layout::Split);
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("diff_layout = \"split\""), "{text}");
        let mut b = App::new(Vec::new(), cfg(), crate::nav::Path::new());
        b.user_config = Some(user);
        b.load_look();
        assert_eq!(b.diff_layout, Layout::Split, "the layout did not carry to the next run");
    }
}
