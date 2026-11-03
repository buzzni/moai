//! Bounded virtual kitty placements. Only the final frame's visible cells trigger uploads.
use crate::image::term::{self, Protocol, Size};
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
};

pub const PLACEHOLDER: char = '\u{10eeee}';
/// Pictures one body or wiki page draws; further pictures keep their text and links.
pub const PER_VIEW: usize = 8;
/// The detail and the wiki page lay out in the same frame, so the cache holds both views' pictures. With room for
/// only one view, the second view's pictures evicted the first's, the first re-laid and evicted the second's, and
/// every frame decoded and re-uploaded them all.
const LIMIT: usize = 2 * PER_VIEW;
/// The spacing of tmux client rechecks while a picture view is open — the explorer's follow step, not its fast
/// encoding step.
const REFRESH: std::time::Duration = std::time::Duration::from_millis(700);

#[derive(Default)]
pub struct Images {
    pub active: bool,
    pub setting: Option<String>,
    protocol: Option<Protocol>,
    client: Option<String>,
    interested: bool,
    /// A recheck on its way; `None` inside means tmux could not answer it ([`term::observe_terminal`]).
    refresh: Option<std::sync::mpsc::Receiver<Option<term::TerminalState>>>,
    refresh_at: Option<std::time::Instant>,
    geometry: (u16, u16, u16, u16),
    entries: VecDeque<Entry>,
    deleted: Vec<u32>,
    next: u32,
    worker: Option<Worker>,
    /// A layout asked for a picture that got no entry (the queue was full) or lost its pending entry to eviction.
    /// No held layout carries a placeholder for it, so `valid` never fails and no result will name it — the next
    /// [`Images::poll`] lays the views out again instead.
    retry: bool,
    #[cfg(test)]
    synchronous: bool,
    #[cfg(test)]
    terminal_probes: bool,
}
struct Worker {
    tx: std::sync::mpsc::SyncSender<Job>,
    rx: std::sync::mpsc::Receiver<Done>,
    handle: Option<std::thread::JoinHandle<()>>,
    /// Bumped whenever the cache drops every entry (a new geometry). Queued jobs from an older epoch are skipped,
    /// not decoded — a resize would otherwise make the new pictures wait behind up to `LIMIT` stale decodes.
    epoch: std::sync::Arc<std::sync::atomic::AtomicU32>,
}
struct Job {
    id: u32,
    path: PathBuf,
    cols: usize,
    geometry: (u16, u16, u16, u16),
    epoch: u32,
}
struct Done {
    id: u32,
    stamp: crate::store::Stamp,
    encoded: Option<(Size, Vec<u8>)>,
}
impl Worker {
    fn new() -> Self {
        let (tx, jobs) = std::sync::mpsc::sync_channel::<Job>(LIMIT);
        let (results, rx) = std::sync::mpsc::sync_channel(LIMIT);
        let epoch = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let current = std::sync::Arc::clone(&epoch);
        let handle = std::thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                // Its entry is already gone; the result would be discarded by id anyway.
                if job.epoch != current.load(std::sync::atomic::Ordering::Relaxed) {
                    continue;
                }
                let stamp = crate::store::stamp(&job.path);
                let encoded = prepare(&job.path, job.cols, job.geometry).and_then(|(img, size)| {
                    crate::image::png(&img).ok().map(|png| (size, term::kitty_virtual(&png, size, job.id)))
                });
                if results.send(Done { id: job.id, stamp, encoded }).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx, handle: Some(handle), epoch }
    }
}
struct Entry {
    path: PathBuf,
    stamp: crate::store::Stamp,
    cols: usize,
    size: Size,
    id: u32,
    payload: Vec<u8>,
    sent: bool,
    pending: bool,
}
impl Images {
    #[cfg(test)]
    pub(super) fn testing() -> Self {
        Self {
            active: true,
            setting: None,
            protocol: Some(Protocol::Kitty),
            client: None,
            interested: false,
            refresh: None,
            refresh_at: None,
            geometry: (80, 40, 640, 640),
            entries: VecDeque::new(),
            deleted: Vec::new(),
            next: 0,
            worker: None,
            retry: false,
            synchronous: true,
            terminal_probes: false,
        }
    }
    #[cfg(test)]
    pub(crate) fn testing_async() -> Self {
        let mut images = Self::testing();
        images.synchronous = false;
        images
    }
    /// Called before layout; pixel changes matter even if the terminal has the same cell count. True when the change
    /// dropped cached pictures: a pending one is in no held layout for `valid` to catch, so the caller lays the
    /// views out again. A cache that held nothing (images off, nothing drawn yet) has nothing to lay out again.
    pub fn geometry(&mut self, width: u16, height: u16) -> bool {
        let window = if self.off() { None } else { ratatui::crossterm::terminal::window_size().ok() };
        let pixels = window.map_or((0, 0), |w| (w.width, w.height));
        let geometry = (width, height, pixels.0, pixels.1);
        let changed = geometry != self.geometry;
        self.geometry = geometry;
        changed && self.clear_entries()
    }
    /// Whether there was anything to drop.
    fn clear_entries(&mut self) -> bool {
        let held = !self.entries.is_empty();
        self.deleted.extend(self.entries.drain(..).filter(|e| e.sent).map(|e| e.id));
        if let Some(worker) = &self.worker {
            worker.epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        held
    }
    /// Pictures are still on their way, or have to be asked for again: the loop polls at its fast step until they
    /// land instead of waiting out the file watch's step.
    pub fn busy(&self) -> bool {
        self.retry || self.entries.iter().any(|e| e.pending)
    }
    fn enabled(&mut self) -> bool {
        if !self.active {
            return false;
        }
        if self.protocol.is_none() {
            let state = term::terminal_state(self.setting.as_deref());
            self.protocol = Some(state.protocol);
            self.client = state.client;
            // That was a probe too: the follow cycle's recheck keeps its spacing behind it.
            self.refresh_at = Some(std::time::Instant::now() + REFRESH);
        }
        matches!(self.protocol, Some(Protocol::Kitty | Protocol::KittyTmux))
    }
    /// The painter already knows the local image targets in the displayed body/page. An old cached picture must
    /// not keep probing tmux after navigation to a picture-free view, raw mode or a collapsed detail pane.
    /// Coming back does not reset the spacing: holding a key over issues with and without pictures would otherwise
    /// start a probe per issue. Away longer than [`REFRESH`], the next recheck is already due.
    pub fn interest(&mut self, interested: bool) {
        self.interested = interested;
    }
    /// True when the frame owes the terminal something: another protocol, or uploads the new client never saw.
    fn apply_terminal(&mut self, state: term::TerminalState) -> bool {
        if self.protocol == Some(state.protocol) && self.client == state.client {
            return false;
        }
        let changed = if self.protocol != Some(state.protocol) {
            self.clear_entries();
            // These ids belonged to the previous protocol. In particular, never send kitty deletions to a new,
            // unsupported terminal, even when there were evictions waiting for a frame before it attached.
            self.deleted.clear();
            true
        } else {
            // Same protocol, another client: the payloads stay and only the uploads are owed again. Queued
            // deletions stay too — a client still attached beside it holds those ids, and `q=2` keeps a deletion
            // of an id this client never saw silent.
            let mut resend = false;
            for entry in &mut self.entries {
                resend |= std::mem::take(&mut entry.sent);
            }
            resend
        };
        self.protocol = Some(state.protocol);
        self.client = state.client;
        changed
    }
    fn refresh_terminal(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.refresh {
            match rx.try_recv() {
                Ok(answer) => {
                    self.refresh = None;
                    // `None`: tmux did not answer. Keep what is known rather than drop working previews.
                    if let Some(state) = answer
                        && self.interested
                        && self.active
                    {
                        changed = self.apply_terminal(state);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.refresh = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        // Use the existing follow cadence, not Images::busy's fast encoding cadence. At most one bounded probe
        // can run. Explicit overrides retain their no-probe contract, including when TMUX was inherited.
        let now = std::time::Instant::now();
        #[cfg(test)]
        let may_probe = self.terminal_probes;
        #[cfg(not(test))]
        let may_probe = true;
        use std::io::IsTerminal;
        // A supported direct terminal with no client is outside tmux (a window launched from a tmux shell). Its
        // stdout device never becomes a pane. Off or unknown must still be rechecked to detect a later attach.
        let outside_pane =
            self.client.is_none() && matches!(self.protocol, Some(Protocol::Kitty | Protocol::Iterm | Protocol::Sixel));
        if may_probe
            && self.active
            && std::io::stdout().is_terminal()
            && self.interested
            && !outside_pane
            && cfg!(unix)
            && std::env::var_os("TMUX").is_some()
            && term::auto_detects(self.setting.as_deref())
            && self.refresh.is_none()
            && self.refresh_at.is_none_or(|due| now >= due)
        {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let setting = self.setting.clone();
            if std::thread::Builder::new()
                .name("image-terminal".into())
                .spawn(move || {
                    let _ = tx.send(term::observe_terminal(setting.as_deref()));
                })
                .is_ok()
            {
                self.refresh = Some(rx);
            }
            self.refresh_at = Some(now + REFRESH);
        }
        changed
    }
    /// Known to draw nothing: not a terminal, or detection already answered another protocol. Unknown is not off.
    pub fn off(&self) -> bool {
        !self.active || self.protocol.is_some_and(|p| !matches!(p, Protocol::Kitty | Protocol::KittyTmux))
    }
    /// Only placeholder spans this cache made (an RGB id colour) are checked — text that happens to contain the
    /// placeholder character never makes a held body stale, and without kitty there is nothing to scan.
    pub fn valid(&self, lines: &[Line<'_>]) -> bool {
        if !matches!(self.protocol, Some(Protocol::Kitty | Protocol::KittyTmux)) || !self.active {
            return true;
        }
        lines.iter().flat_map(|l| &l.spans).all(|s| match s.style.fg {
            Some(fg @ Color::Rgb(..)) if s.content.contains(PLACEHOLDER) => {
                self.entries.iter().any(|e| fg == color(e.id))
            }
            _ => true,
        })
    }
    pub fn lines(&mut self, path: &Path, cols: usize) -> Vec<Line<'static>> {
        if cols == 0 || !self.enabled() {
            return Vec::new();
        }
        let found = self.entries.iter().position(|e| e.path == path && e.cols == cols);
        let index = if let Some(index) = found {
            let entry = self.entries.remove(index).unwrap();
            self.entries.push_back(entry);
            self.entries.len() - 1
        } else {
            // A process-specific 24-bit namespace; zero is never a valid id.
            if self.next == 0 {
                self.next = ((std::process::id().wrapping_mul(2654435761)) & 0xff_ffff).max(1);
            }
            let id = self.next;
            self.next = (self.next % 0xff_ffff) + 1;
            let worker = self.worker.get_or_insert_with(Worker::new);
            let epoch = worker.epoch.load(std::sync::atomic::Ordering::Relaxed);
            let job = Job { id, path: path.into(), cols, geometry: self.geometry, epoch };
            if worker.tx.try_send(job).is_err() {
                self.retry = true;
                return Vec::new();
            }
            if self.entries.len() == LIMIT
                && let Some(old) = self.entries.pop_front()
            {
                if old.sent {
                    self.deleted.push(old.id);
                }
                self.retry |= old.pending;
            }
            self.entries.push_back(Entry {
                path: path.into(),
                stamp: None,
                cols,
                size: Size { cols: 0, rows: 0, pixels: (0, 0) },
                id,
                payload: Vec::new(),
                sent: false,
                pending: true,
            });
            self.entries.len() - 1
        };
        // By id, not index: the test wait below polls, and a poll may drop other (stale) entries.
        let id = self.entries[index].id;
        #[cfg(test)]
        if self.synchronous {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while self.entries.iter().any(|e| e.id == id && e.pending) {
                self.poll();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        let Some(e) = self.entries.iter().find(|e| e.id == id) else { return Vec::new() };
        (0..e.size.rows as usize)
            .map(|row| {
                Line::from(Span::styled(
                    (0..e.size.cols as usize).map(|col| symbol(row, col)).collect::<String>(),
                    Style::new().fg(color(e.id)),
                ))
            })
            .collect()
    }
    /// File checks belong to the idle follow cadence, never the frame/layout path.
    /// Results are matched by unique placement id, so an old resize or eviction cannot install stale pixels.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.refresh_terminal() | std::mem::take(&mut self.retry);
        if let Some(worker) = &mut self.worker {
            if worker.handle.as_ref().is_some_and(|h| h.is_finished())
                && let Some(handle) = worker.handle.take()
                && let Err(payload) = handle.join()
            {
                std::panic::resume_unwind(payload);
            }
            while let Ok(done) = worker.rx.try_recv() {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.id == done.id) {
                    entry.stamp = done.stamp;
                    entry.pending = false;
                    if let Some((size, payload)) = done.encoded {
                        entry.size = size;
                        entry.payload = payload;
                    }
                    changed = true;
                }
            }
        }
        let mut stale = Vec::new();
        self.entries.retain(|e| {
            let keep = e.pending || crate::store::stamp(&e.path) == e.stamp;
            if !keep && e.sent {
                stale.push(e.id);
            }
            changed |= !keep;
            keep
        });
        self.deleted.extend(stale);
        changed
    }
    /// Runs after popups and search styles, before the backend emits the frame's diff.
    pub fn frame(&mut self, buffer: &mut Buffer, out: &mut impl Write) -> std::io::Result<()> {
        for id in self.deleted.drain(..) {
            write_graphics(out, &term::kitty_delete(id), self.protocol)?;
        }
        // One pass over the frame, not one per cached picture.
        let mut visible = vec![false; self.entries.len()];
        if !self.entries.is_empty() {
            for cell in &mut buffer.content {
                if !cell.symbol().contains(PLACEHOLDER) {
                    continue;
                }
                if let Some(n) = self.entries.iter().position(|e| cell.fg == color(e.id)) {
                    visible[n] = true;
                    cell.set_diff_option(CellDiffOption::ForcedWidth(std::num::NonZeroU16::new(1).unwrap()));
                    cell.modifier.remove(Modifier::DIM | Modifier::REVERSED | Modifier::HIDDEN);
                }
            }
        }
        for (e, visible) in self.entries.iter_mut().zip(visible) {
            if visible && !e.sent {
                write_graphics(out, &e.payload, self.protocol)?;
                e.sent = true;
            }
        }
        out.flush()
    }
    pub fn cleanup(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        for id in self.deleted.drain(..) {
            write_graphics(out, &term::kitty_delete(id), self.protocol)?;
        }
        for e in &mut self.entries {
            if e.sent {
                write_graphics(out, &term::kitty_delete(e.id), self.protocol)?;
                e.sent = false;
            }
        }
        out.flush()
    }
}
impl Drop for Images {
    fn drop(&mut self) {
        if self.active {
            let _ = self.cleanup(&mut std::io::stdout().lock());
        }
    }
}
fn prepare(path: &Path, cols: usize, geometry: (u16, u16, u16, u16)) -> Option<(image::DynamicImage, Size)> {
    let bytes = crate::image::read_input(path).ok()?;
    let img = crate::image::decode(&bytes).ok()?;
    let (w, h, pw, ph) = geometry;
    let cell =
        if w > 0 && h > 0 && pw > 0 && ph > 0 { (u32::from(pw / w).max(1), u32::from(ph / h).max(1)) } else { (8, 16) };
    let mut size = term::size(img.width(), img.height(), u32::try_from(cols).unwrap_or(u32::MAX), cell);
    let rows = u32::from((h / 2).clamp(1, 20));
    if size.rows > rows {
        let scale = (rows * cell.1) as f64 / size.pixels.1 as f64;
        size.pixels = ((size.pixels.0 as f64 * scale).round().max(1.) as u32, rows * cell.1);
        size.cols = size.pixels.0.div_ceil(cell.0);
        size.rows = rows;
    }
    Some((img.resize_exact(size.pixels.0, size.pixels.1, image::imageops::FilterType::Triangle), size))
}
fn write_graphics(out: &mut impl Write, payload: &[u8], protocol: Option<Protocol>) -> std::io::Result<()> {
    if protocol == Some(Protocol::KittyTmux) {
        out.write_all(&term::passthrough(payload))
    } else {
        out.write_all(payload)
    }
}
fn color(id: u32) -> Color {
    Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8)
}
pub(crate) fn symbol(row: usize, col: usize) -> String {
    format!("{PLACEHOLDER}{}{}", DIACRITICS[row], DIACRITICS[col])
}

// Kitty's gen/rowcolumn-diacritics.txt, Unicode 6.0. Explicit row AND column support isolated diffs.
const DIACRITICS: &[char] = &[
    '\u{0305}',
    '\u{030d}',
    '\u{030e}',
    '\u{0310}',
    '\u{0312}',
    '\u{033d}',
    '\u{033e}',
    '\u{033f}',
    '\u{0346}',
    '\u{034a}',
    '\u{034b}',
    '\u{034c}',
    '\u{0350}',
    '\u{0351}',
    '\u{0352}',
    '\u{0357}',
    '\u{035b}',
    '\u{0363}',
    '\u{0364}',
    '\u{0365}',
    '\u{0366}',
    '\u{0367}',
    '\u{0368}',
    '\u{0369}',
    '\u{036a}',
    '\u{036b}',
    '\u{036c}',
    '\u{036d}',
    '\u{036e}',
    '\u{036f}',
    '\u{0483}',
    '\u{0484}',
    '\u{0485}',
    '\u{0486}',
    '\u{0487}',
    '\u{0592}',
    '\u{0593}',
    '\u{0594}',
    '\u{0595}',
    '\u{0597}',
    '\u{0598}',
    '\u{0599}',
    '\u{059c}',
    '\u{059d}',
    '\u{059e}',
    '\u{059f}',
    '\u{05a0}',
    '\u{05a1}',
    '\u{05a8}',
    '\u{05a9}',
    '\u{05ab}',
    '\u{05ac}',
    '\u{05af}',
    '\u{05c4}',
    '\u{0610}',
    '\u{0611}',
    '\u{0612}',
    '\u{0613}',
    '\u{0614}',
    '\u{0615}',
    '\u{0616}',
    '\u{0617}',
    '\u{0657}',
    '\u{0658}',
    '\u{0659}',
    '\u{065a}',
    '\u{065b}',
    '\u{065d}',
    '\u{065e}',
    '\u{06d6}',
    '\u{06d7}',
    '\u{06d8}',
    '\u{06d9}',
    '\u{06da}',
    '\u{06db}',
    '\u{06dc}',
    '\u{06df}',
    '\u{06e0}',
    '\u{06e1}',
    '\u{06e2}',
    '\u{06e4}',
    '\u{06e7}',
    '\u{06e8}',
    '\u{06eb}',
    '\u{06ec}',
    '\u{0730}',
    '\u{0732}',
    '\u{0733}',
    '\u{0735}',
    '\u{0736}',
    '\u{073a}',
    '\u{073d}',
    '\u{073f}',
    '\u{0740}',
    '\u{0741}',
    '\u{0743}',
    '\u{0745}',
    '\u{0747}',
    '\u{0749}',
    '\u{074a}',
    '\u{07eb}',
    '\u{07ec}',
    '\u{07ed}',
    '\u{07ee}',
    '\u{07ef}',
    '\u{07f0}',
    '\u{07f1}',
    '\u{07f3}',
    '\u{0816}',
    '\u{0817}',
    '\u{0818}',
    '\u{0819}',
    '\u{081b}',
    '\u{081c}',
    '\u{081d}',
    '\u{081e}',
    '\u{081f}',
    '\u{0820}',
    '\u{0821}',
    '\u{0822}',
    '\u{0823}',
    '\u{0825}',
    '\u{0826}',
    '\u{0827}',
    '\u{0829}',
    '\u{082a}',
    '\u{082b}',
    '\u{082c}',
    '\u{082d}',
    '\u{0951}',
    '\u{0953}',
    '\u{0954}',
    '\u{0f82}',
    '\u{0f83}',
    '\u{0f86}',
    '\u{0f87}',
    '\u{135d}',
    '\u{135e}',
    '\u{135f}',
    '\u{17dd}',
    '\u{193a}',
    '\u{1a17}',
    '\u{1a75}',
    '\u{1a76}',
    '\u{1a77}',
    '\u{1a78}',
    '\u{1a79}',
    '\u{1a7a}',
    '\u{1a7b}',
    '\u{1a7c}',
    '\u{1b6b}',
    '\u{1b6d}',
    '\u{1b6e}',
    '\u{1b6f}',
    '\u{1b70}',
    '\u{1b71}',
    '\u{1b72}',
    '\u{1b73}',
    '\u{1cd0}',
    '\u{1cd1}',
    '\u{1cd2}',
    '\u{1cda}',
    '\u{1cdb}',
    '\u{1ce0}',
    '\u{1dc0}',
    '\u{1dc1}',
    '\u{1dc3}',
    '\u{1dc4}',
    '\u{1dc5}',
    '\u{1dc6}',
    '\u{1dc7}',
    '\u{1dc8}',
    '\u{1dc9}',
    '\u{1dcb}',
    '\u{1dcc}',
    '\u{1dd1}',
    '\u{1dd2}',
    '\u{1dd3}',
    '\u{1dd4}',
    '\u{1dd5}',
    '\u{1dd6}',
    '\u{1dd7}',
    '\u{1dd8}',
    '\u{1dd9}',
    '\u{1dda}',
    '\u{1ddb}',
    '\u{1ddc}',
    '\u{1ddd}',
    '\u{1dde}',
    '\u{1ddf}',
    '\u{1de0}',
    '\u{1de1}',
    '\u{1de2}',
    '\u{1de3}',
    '\u{1de4}',
    '\u{1de5}',
    '\u{1de6}',
    '\u{1dfe}',
    '\u{20d0}',
    '\u{20d1}',
    '\u{20d4}',
    '\u{20d5}',
    '\u{20d6}',
    '\u{20d7}',
    '\u{20db}',
    '\u{20dc}',
    '\u{20e1}',
    '\u{20e7}',
    '\u{20e9}',
    '\u{20f0}',
    '\u{2cef}',
    '\u{2cf0}',
    '\u{2cf1}',
    '\u{2de0}',
    '\u{2de1}',
    '\u{2de2}',
    '\u{2de3}',
    '\u{2de4}',
    '\u{2de5}',
    '\u{2de6}',
    '\u{2de7}',
    '\u{2de8}',
    '\u{2de9}',
    '\u{2dea}',
    '\u{2deb}',
    '\u{2dec}',
    '\u{2ded}',
    '\u{2dee}',
    '\u{2def}',
    '\u{2df0}',
    '\u{2df1}',
    '\u{2df2}',
    '\u{2df3}',
    '\u{2df4}',
    '\u{2df5}',
    '\u{2df6}',
    '\u{2df7}',
    '\u{2df8}',
    '\u{2df9}',
    '\u{2dfa}',
    '\u{2dfb}',
    '\u{2dfc}',
    '\u{2dfd}',
    '\u{2dfe}',
    '\u{2dff}',
    '\u{a66f}',
    '\u{a67c}',
    '\u{a67d}',
    '\u{a6f0}',
    '\u{a6f1}',
    '\u{a8e0}',
    '\u{a8e1}',
    '\u{a8e2}',
    '\u{a8e3}',
    '\u{a8e4}',
    '\u{a8e5}',
    '\u{a8e6}',
    '\u{a8e7}',
    '\u{a8e8}',
    '\u{a8e9}',
    '\u{a8ea}',
    '\u{a8eb}',
    '\u{a8ec}',
    '\u{a8ed}',
    '\u{a8ee}',
    '\u{a8ef}',
    '\u{a8f0}',
    '\u{a8f1}',
    '\u{aab0}',
    '\u{aab2}',
    '\u{aab3}',
    '\u{aab7}',
    '\u{aab8}',
    '\u{aabe}',
    '\u{aabf}',
    '\u{aac1}',
    '\u{fe20}',
    '\u{fe21}',
    '\u{fe22}',
    '\u{fe23}',
    '\u{fe24}',
    '\u{fe25}',
    '\u{fe26}',
    '\u{10a0f}',
    '\u{10a38}',
    '\u{1d185}',
    '\u{1d186}',
    '\u{1d187}',
    '\u{1d188}',
    '\u{1d189}',
    '\u{1d1aa}',
    '\u{1d1ab}',
    '\u{1d1ac}',
    '\u{1d1ad}',
    '\u{1d242}',
    '\u{1d243}',
    '\u{1d244}',
];
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};
    fn ready(images: &mut Images, path: &Path, cols: usize) -> Vec<Line<'static>> {
        images.lines(path, cols);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while images.entries.iter().any(|e| e.pending) {
            images.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        images.lines(path, cols)
    }
    fn images() -> Images {
        Images::testing()
    }
    fn picture(root: &Path, name: &str) -> PathBuf {
        let p = root.join(name);
        image::DynamicImage::new_rgb8(160, 160).save(&p).unwrap();
        p
    }
    #[test]
    fn painter_interest_controls_follow_for_wiki_views() {
        use crate::tui::{Mode, draw::Painter};
        let (root, mut app) = crate::tui::wiki::tests::wiki_app(
            "painter-interest",
            &[("README.md", "# Picture\n\n![shot](images/a.png)"), ("plain.md", "# Plain")],
        );
        std::fs::create_dir_all(root.join("docs/images")).unwrap();
        picture(&root.join("docs/images"), "a.png");
        app.images = Images::testing();
        app.hit("SPC g w");
        crate::tui::settle_reads(&mut app);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut painter = Painter::default();
        painter.draw(&mut terminal, &mut app).unwrap();
        assert!(app.images.interested, "drawn picture page must enable refresh");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Some(term::TerminalState { protocol: Protocol::KittyTmux, client: Some("new client".into()) }))
            .unwrap();
        app.images.refresh = Some(rx);
        assert!(app.follow(), "client transition must request a redraw through follow");
        assert!(matches!(app.images.protocol, Some(Protocol::KittyTmux)));
        let Mode::Wiki(w) = &mut app.mode else { panic!("wiki closed") };
        assert!(w.follow("plain"));
        crate::tui::settle_reads(&mut app);
        painter.draw(&mut terminal, &mut app).unwrap();
        assert!(!app.images.interested, "picture-free page must stop refresh");
        let Mode::Wiki(w) = &mut app.mode else { panic!("wiki closed") };
        assert!(w.follow("README"));
        crate::tui::settle_reads(&mut app);
        app.raw = true;
        painter.draw(&mut terminal, &mut app).unwrap();
        assert!(!app.images.interested, "raw view must stop refresh");
        app.raw = false;
        painter.draw(&mut terminal, &mut app).unwrap();
        assert!(app.images.interested, "returning to picture view must resume refresh");
        app.hit("Esc");
        painter.draw(&mut terminal, &mut app).unwrap();
        assert!(!app.images.interested, "hidden wiki must stop refresh");
        app.images.active = false;
    }

    #[cfg(unix)]
    #[test]
    fn tmux_refresh_pty_child() {
        let Ok(root) = std::env::var("MOAI_IMAGE_REFRESH_CHILD") else { return };
        let root = Path::new(&root);
        let state = root.join("state");
        let tty = std::env::var("MOAI_IMAGE_REFRESH_TTY").unwrap();
        // Rename into place: a probe running meanwhile must read the old reply or the new one, never half of one.
        let write_state = |text: &str| {
            let next = root.join("state.next");
            std::fs::write(&next, text).unwrap();
            std::fs::rename(&next, &state).unwrap();
        };
        let snapshot = |term: &str, permission: &str, client: usize| {
            write_state(&format!("{tty}\nMOAI_CLIENT\n{term}\n{permission}\n{client}\n100\n/dev/client-{client}\n"));
        };
        let queries = || std::fs::read_to_string(root.join("queries")).unwrap_or_default().lines().count();
        if let Ok(expected) = std::env::var("MOAI_DIRECT_PROTOCOL") {
            let expected = match expected.as_str() {
                "kitty" => Protocol::Kitty,
                "iterm" => Protocol::Iterm,
                "sixel" => Protocol::Sixel,
                _ => panic!("unknown direct protocol"),
            };
            write_state("/dev/null\n");
            let mut direct = Images::default();
            direct.active = true;
            direct.terminal_probes = true;
            let before = queries();
            direct.enabled(); // initial auto-detection probes the inherited server once
            assert_eq!(direct.protocol, Some(expected));
            assert_eq!(queries(), before + 1);
            assert!(direct.client.is_none());
            direct.interest(true);
            for _ in 0..10 {
                direct.refresh_at = None;
                direct.poll();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert_eq!(queries(), before + 1, "outside-pane {expected:?} repeatedly probed tmux");
            direct.active = false;
            return;
        }
        let path = picture(root, "image.png");
        let mut images = Images::default();
        images.active = true;
        images.terminal_probes = true;
        images.poll();
        assert_eq!(queries(), 0, "picture-free follow queried tmux");
        write_state(""); // Initial tmux response is unknown; later refresh must still attach.
        let mut unknown = Images::default();
        unknown.active = true;
        unknown.terminal_probes = true;
        assert!(unknown.lines(&path, 20).is_empty());
        assert!(unknown.off());
        unknown.interest(true);
        unknown.refresh_at = None;
        snapshot("xterm-ghostty", "all", 1);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while unknown.off() {
            unknown.poll();
            assert!(std::time::Instant::now() < until, "unknown initial response never refreshed");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        unknown.active = false;
        std::fs::remove_file(root.join("queries")).unwrap();
        snapshot("xterm-ghostty", "on", 1);
        assert!(images.lines(&path, 20).is_empty(), "on must retain text before encoding starts");
        assert!(images.worker.is_none(), "on queued image encoding");
        images.interest(true);
        snapshot("xterm-ghostty", "all", 1);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while images.off() {
            images.poll();
            assert!(std::time::Instant::now() < until, "changing on to all never enabled previews");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let lines = ready(&mut images, &path, 20);
        images.interest(true);
        let first = draw_picture(&mut images, lines.clone());
        assert!(String::from_utf8_lossy(&first).contains("tmux;"));
        let id = images.entries[0].id;
        snapshot("xterm-ghostty", "all", 2);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while images.entries[0].sent {
            images.poll();
            assert!(std::time::Instant::now() < until, "same-size reattach never reset sent");
            assert!(!images.busy(), "terminal refresh enabled fast polling");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(images.entries[0].id, id);
        assert_eq!(draw_picture(&mut images, lines.clone()), first);
        // tmux lists no pane (a timeout or a vanished server answers the same): unknown, so the working state stays.
        write_state("");
        let asked = queries();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while queries() == asked || images.refresh.is_some() {
            images.poll();
            assert!(std::time::Instant::now() < until, "no recheck ran against the silent tmux");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!images.off(), "an unanswered recheck turned previews off");
        assert_eq!(images.entries[0].id, id, "an unanswered recheck dropped the encoded picture");
        assert!(draw_picture(&mut images, lines).is_empty(), "an unanswered recheck re-uploaded");
        snapshot("xterm-256color", "all", 3);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !images.off() {
            images.poll();
            assert!(std::time::Instant::now() < until, "unsupported reattach retained kitty");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(images.lines(&path, 20).is_empty());
        assert!(draw_picture(&mut images, Vec::new()).is_empty());
        snapshot("xterm-ghostty", "all", 4);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while images.off() {
            images.poll();
            assert!(std::time::Instant::now() < until, "reattach to Ghostty never enabled previews");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let lines = ready(&mut images, &path, 20);
        assert!(!draw_picture(&mut images, lines).is_empty());
        snapshot("xterm-ghostty", "on", 4);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !images.off() {
            images.poll();
            assert!(std::time::Instant::now() < until, "changing all to on retained previews");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(images.lines(&path, 20).is_empty());
        assert!(draw_picture(&mut images, Vec::new()).is_empty());
        images.interest(false);
        // Let an already running query finish; no new queries may start after leaving this view.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while images.refresh.is_some() {
            images.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let count = queries();
        images.refresh_at = None;
        for _ in 0..10 {
            images.poll();
        }
        assert_eq!(queries(), count, "old cached pictures kept querying tmux");
        images.active = false;
    }
    #[cfg(unix)]
    #[test]
    fn tmux_refresh_uses_live_mock_replies_on_a_real_stdout_pty() {
        use std::{
            fs::File,
            os::{fd::FromRawFd, unix::fs::PermissionsExt},
            process::{Command, Stdio},
        };
        let root = crate::scratch::Scratch::new("tmux-refresh-pty");
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("tmux");
        std::fs::write(&script, "#!/bin/sh\nprintf 'query\\n' >> \"$MOAI_IMAGE_REFRESH_CHILD/queries\"\n/bin/cat \"$MOAI_IMAGE_REFRESH_CHILD/state\"\n").unwrap();
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (mut master, mut slave) = (-1, -1);
        let mut name: [libc::c_char; 256] = [0; 256];
        // SAFETY: openpty writes only to valid descriptors and the tty-name buffer.
        assert_eq!(
            unsafe {
                libc::openpty(&mut master, &mut slave, name.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut())
            },
            0
        );
        for fd in [master, slave] {
            // SAFETY: both descriptors were just opened by openpty.
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        }
        // SAFETY: openpty transferred ownership of two fresh descriptors and a NUL-terminated tty name.
        let (mut master, slave, tty) =
            unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave), std::ffi::CStr::from_ptr(name.as_ptr())) };
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut out = Vec::new();
            let mut chunk = [0; 4096];
            while let Ok(n) = master.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&chunk[..n]);
            }
            out
        });
        for (protocol, term, program) in [
            ("kitty", "xterm-kitty", ""),
            ("iterm", "xterm-256color", "iTerm.app"),
            ("iterm", "xterm-256color", "WezTerm"),
            ("sixel", "foot", ""),
        ] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tui::images::tests::tmux_refresh_pty_child", "--nocapture"])
                .env("MOAI_IMAGE_REFRESH_CHILD", root.path())
                .env("MOAI_IMAGE_REFRESH_TTY", tty.to_str().unwrap())
                .env("MOAI_DIRECT_PROTOCOL", protocol)
                .env("PATH", &bin)
                .env("TMUX", "/tmp/moai-zidx-fake-only,1,0")
                .env("TERM", term)
                .env("TERM_PROGRAM", program)
                .env_remove("MOAI_IMAGES")
                .env_remove("KITTY_WINDOW_ID")
                .env_remove("WT_SESSION")
                .env_remove("LC_TERMINAL")
                .stdin(Stdio::null())
                .stdout(slave.try_clone().unwrap())
                .stderr(Stdio::piped())
                .output()
                .unwrap();
            assert!(output.status.success(), "{protocol}: {}", String::from_utf8_lossy(&output.stderr));
        }
        std::fs::remove_file(root.join("queries")).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        let mut child = cmd
            .args(["--exact", "tui::images::tests::tmux_refresh_pty_child", "--nocapture"])
            .env("MOAI_IMAGE_REFRESH_CHILD", root.path())
            .env("MOAI_IMAGE_REFRESH_TTY", tty.to_str().unwrap())
            .env("PATH", bin)
            .env("TMUX", "/tmp/moai-zidx-fake-only,1,0")
            .env("TMUX_PANE", "%0")
            .env("TERM", "tmux-256color")
            .env_remove("MOAI_IMAGES")
            .stdin(Stdio::null())
            .stdout(slave)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(cmd);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= until {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("terminal refresh child timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        let stdout = reader.join().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    /// Feed a completed background probe through the same follow path as production, without changing process
    /// environment shared by parallel tests. The protocol remains kitty/tmux while the client identity changes.
    fn refreshed(images: &mut Images, protocol: Protocol, client: &str) -> bool {
        answered(images, Some(term::TerminalState { protocol, client: Some(client.into()) }))
    }
    fn answered(images: &mut Images, answer: Option<term::TerminalState>) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(answer).unwrap();
        images.refresh = Some(rx);
        images.interest(true);
        images.poll()
    }
    fn draw_picture(images: &mut Images, lines: Vec<Line<'static>>) -> Vec<u8> {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 10));
        use ratatui::widgets::Widget;
        Paragraph::new(lines).render(buffer.area, &mut buffer);
        let mut out = Vec::new();
        images.frame(&mut buffer, &mut out).unwrap();
        out
    }
    #[test]
    fn same_protocol_client_swap_reuploads_without_reencoding() {
        let root = crate::scratch::Scratch::new("tmux-reattach");
        let path = picture(&root, "one.png");
        let mut images = images();
        images.protocol = Some(Protocol::KittyTmux);
        images.client = Some("ghostty/client-1".into());
        let lines = ready(&mut images, &path, 20);
        let id = images.entries[0].id;
        let payload = draw_picture(&mut images, lines.clone());
        assert!(payload.starts_with(b"\x1bPtmux;"));
        assert!(images.entries[0].sent);
        assert!(!refreshed(&mut images, Protocol::KittyTmux, "ghostty/client-1"));
        assert!(draw_picture(&mut images, lines.clone()).is_empty());
        assert!(refreshed(&mut images, Protocol::KittyTmux, "ghostty/client-2"));
        assert_eq!(images.entries[0].id, id);
        assert!(!images.entries[0].pending, "reattach queued an unnecessary decode");
        assert_eq!(draw_picture(&mut images, lines), payload);
        images.active = false;
    }
    #[test]
    fn unsupported_tmux_reattach_drops_placements_and_queued_deletions() {
        let root = crate::scratch::Scratch::new("tmux-unsupported");
        let path = picture(&root, "one.png");
        let mut images = images();
        images.protocol = Some(Protocol::KittyTmux);
        images.client = Some("ghostty/client-1".into());
        let lines = ready(&mut images, &path, 20);
        draw_picture(&mut images, lines);
        images.deleted.push(123);
        assert!(refreshed(&mut images, Protocol::Off, "iterm/client-2"));
        assert!(images.off());
        assert!(images.entries.is_empty());
        assert!(images.deleted.is_empty());
        assert!(images.lines(&path, 20).is_empty(), "unsupported terminal retained placeholder rows");
        assert!(draw_picture(&mut images, Vec::new()).is_empty(), "sent graphics to an unsupported terminal");
        assert!(refreshed(&mut images, Protocol::KittyTmux, "ghostty/client-3"));
        let lines = ready(&mut images, &path, 20);
        assert!(!lines.is_empty());
        assert!(!draw_picture(&mut images, lines).is_empty());
        images.active = false;
    }
    #[test]
    fn an_unanswered_recheck_keeps_previews_and_pending_deletions() {
        let root = crate::scratch::Scratch::new("tmux-unanswered");
        let path = picture(&root, "one.png");
        let mut images = images();
        images.protocol = Some(Protocol::KittyTmux);
        images.client = Some("ghostty/client-1".into());
        let lines = ready(&mut images, &path, 20);
        let id = images.entries[0].id;
        draw_picture(&mut images, lines.clone());
        images.deleted.push(123);
        assert!(!answered(&mut images, None), "an unanswered recheck asked for a redraw");
        assert_eq!(images.protocol, Some(Protocol::KittyTmux));
        assert_eq!(images.client.as_deref(), Some("ghostty/client-1"));
        assert_eq!(images.entries[0].id, id);
        assert!(images.entries[0].sent);
        assert_eq!(images.deleted, [123]);
        images.active = false;
    }
    #[test]
    fn another_client_owes_only_what_was_uploaded() {
        let root = crate::scratch::Scratch::new("tmux-client-swap");
        let path = picture(&root, "one.png");
        let mut images = images();
        images.protocol = Some(Protocol::Off);
        images.client = Some("xterm/client-1".into());
        assert!(!refreshed(&mut images, Protocol::Off, "xterm/client-2"), "an unsupported swap laid views out");
        images.protocol = Some(Protocol::KittyTmux);
        images.client = Some("ghostty/client-2".into());
        ready(&mut images, &path, 20);
        assert!(!refreshed(&mut images, Protocol::KittyTmux, "ghostty/client-3"), "nothing uploaded, nothing owed");
        // An eviction's deletion still reaches a client attached beside the new one.
        images.deleted.push(123);
        refreshed(&mut images, Protocol::KittyTmux, "ghostty/client-4");
        assert_eq!(images.deleted, [123]);
        images.active = false;
    }
    #[test]
    fn completed_terminal_probe_is_ignored_after_leaving_picture_view() {
        let mut images = images();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Some(term::TerminalState { protocol: Protocol::Off, client: Some("other".into()) })).unwrap();
        images.refresh = Some(rx);
        images.interest(false);
        assert!(!images.poll());
        assert_eq!(images.protocol, Some(Protocol::Kitty));
        assert!(!images.busy(), "terminal observation must not enable fast polling");
        images.active = false;
    }
    #[test]
    fn asynchronous_results_invalidate_layout_and_old_resize_results_are_discarded() {
        let root = crate::scratch::Scratch::new("async-image");
        let p = picture(&root, "one.png");
        let mut images = images();
        images.synchronous = false;
        assert!(images.lines(&p, 20).is_empty(), "first layout must only enqueue");
        let old_id = images.entries[0].id;
        images.geometry(80, 6);
        assert!(images.lines(&p, 20).is_empty());
        assert_ne!(old_id, images.entries[0].id);
        let lines = ready(&mut images, &p, 20);
        assert_eq!(lines.len(), 3);
        assert_eq!(images.entries.len(), 1);
        assert!(!images.poll(), "idle follow must not request another frame");
        std::fs::write(&p, b"corrupt").unwrap();
        // Geometry/layout must not stat a cached file; follow alone invalidates it.
        assert!(!images.geometry(80, 6));
        assert_eq!(images.lines(&p, 20).len(), 3);
        assert!(images.poll());
        assert!(ready(&mut images, &p, 20).is_empty());
        assert!(!images.poll(), "failed encoding must stay cached until the file changes");
        images.active = false;
    }
    /// A picture that never got an entry (full queue) or lost its pending entry to eviction has no placeholder in
    /// any held layout and no result that names it — only the retry flag lays the view out again. One request past
    /// `LIMIT` takes one of the two paths whichever way the worker races.
    #[test]
    fn a_picture_refused_or_evicted_before_it_landed_lays_the_view_out_again() {
        let root = crate::scratch::Scratch::new("async-retry");
        let paths: Vec<_> = (0..=LIMIT).map(|n| picture(&root, &format!("{n}.png"))).collect();
        let mut images = images();
        images.synchronous = false;
        for p in &paths {
            assert!(images.lines(p, 20).is_empty());
        }
        assert!(images.retry && images.busy());
        assert!(images.poll(), "the lost picture must ask for another layout");
        assert!(!images.retry);
        images.active = false;
    }
    #[test]
    fn placeholder_cells_upload_once_scroll_cover_cleanup_and_resume() {
        let root = crate::scratch::Scratch::new("virtual-kitty");
        let path = picture(&root, "image.png");
        let mut images = images();
        let lines = ready(&mut images, &path, 20);
        let entry = &images.entries[0];
        let id = entry.id;
        assert_eq!(lines[0].spans[0].style.fg, Some(color(id)));
        assert!(lines[0].spans[0].content.starts_with(&symbol(0, 0)));
        assert!(lines[1].spans[0].content.starts_with(&symbol(1, 0)));
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        let mut out = Vec::new();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines.clone()), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        let first = String::from_utf8(out.clone()).unwrap();
        assert!(first.contains(&format!("a=T,U=1,f=100,i={id},c=20,r=10,q=2")));
        assert!(!first.ends_with('\n'));
        out.clear();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines.clone()).scroll((2, 0)), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert!(out.is_empty(), "scroll retransmitted image");
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new("help popup"), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert!(out.is_empty());
        assert!(!terminal.backend().buffer().content.iter().any(|c| c.symbol().contains(PLACEHOLDER)));
        images.cleanup(&mut out).unwrap();
        assert_eq!(out, term::kitty_delete(id));
        out.clear();
        terminal
            .draw(|f| {
                f.render_widget(Paragraph::new(lines), f.area());
                images.frame(f.buffer_mut(), &mut out).unwrap();
            })
            .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), first);
        images.active = false;
    }
    #[test]
    fn cache_is_bounded_and_replacement_invalidates_old_lines() {
        let root = crate::scratch::Scratch::new("virtual-limit");
        let mut images = images();
        images.geometry(80, 40);
        let p = picture(&root, "0.png");
        let old = ready(&mut images, &p, 20);
        let id = images.entries[0].id;
        images.entries[0].sent = true;
        for n in 1..=LIMIT {
            ready(&mut images, &picture(&root, &format!("{n}.png")), 20);
        }
        assert_eq!(images.entries.len(), LIMIT);
        assert!(!images.valid(&old));
        assert_eq!(images.deleted, vec![id]);
        let lines = ready(&mut images, &p, 20);
        assert!(images.valid(&lines));
        std::fs::write(&p, b"corrupt now").unwrap();
        assert!(images.poll());
        assert!(!images.valid(&lines));
        assert!(images.lines(&p, 20).is_empty());
        images.active = false;
    }
    /// The detail and the wiki page lay out in one frame. Each may draw `PER_VIEW` pictures at its own width, and
    /// neither may evict the other's — that turned every frame into a decode and re-upload of all of them.
    #[test]
    fn the_detail_and_the_wiki_page_keep_their_pictures_together() {
        let root = crate::scratch::Scratch::new("virtual-two-views");
        let mut images = images();
        let paths: Vec<_> = (0..PER_VIEW).map(|n| picture(&root, &format!("{n}.png"))).collect();
        let detail: Vec<_> = paths.iter().flat_map(|p| ready(&mut images, p, 20)).collect();
        let wiki: Vec<_> = paths.iter().flat_map(|p| ready(&mut images, p, 30)).collect();
        assert!(images.valid(&detail), "the wiki page evicted the detail's pictures");
        assert!(images.valid(&wiki));
        // Text that merely contains the placeholder character is not a stale picture.
        assert!(images.valid(&[Line::from(Span::styled(format!("a {PLACEHOLDER} b"), Style::new().fg(Color::Cyan)))]));
        images.active = false;
    }
    #[test]
    fn resize_rebuilds_placements_at_the_new_height() {
        let root = crate::scratch::Scratch::new("virtual-resize");
        let p = picture(&root, "image.png");
        let mut images = images();
        let old = ready(&mut images, &p, 20);
        assert!(images.geometry(80, 6));
        let new = ready(&mut images, &p, 20);
        assert!(!images.valid(&old));
        assert_eq!(new.len(), 3);
        images.active = false;
    }
    #[test]
    fn disabled_and_unsupported_images_keep_text_layout() {
        let root = crate::scratch::Scratch::new("virtual-off");
        let p = picture(&root, "one.png");
        assert!(Images::default().lines(&p, 20).is_empty());
        let mut images = images();
        images.protocol = Some(Protocol::Off);
        assert!(images.lines(&p, 20).is_empty());
        images.protocol = Some(Protocol::Kitty);
        let bad = root.join("bad.gif");
        std::fs::write(&bad, b"GIF89a").unwrap();
        assert!(images.lines(&bad, 20).is_empty());
        images.active = false;
    }
}
