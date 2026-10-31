use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

use super::wire::{self, Reply, Request};

pub struct Incoming {
    pub request: Request,
    pub answer: mpsc::Sender<Reply>,
}

/// The owner loop alone consumes requests and owns the application state.
pub struct Listener {
    pub incoming: mpsc::Receiver<Incoming>,
    path: std::path::PathBuf,
    stop: Arc<AtomicBool>,
    book: Option<Arc<super::proposal::Book>>,
}

pub type Handler = Arc<dyn Fn(Request) -> Reply + Send + Sync>;

struct Leave(Arc<AtomicUsize>);
impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(unix)]
impl Listener {
    pub fn bind_proposals(book: Arc<super::proposal::Book>) -> std::io::Result<Self> {
        Self::bind_with(None, Some(book))
    }

    /// The web server answers from its tab registry; the TUI keeps its owner-loop channel.
    pub fn bind_web(handler: Handler) -> std::io::Result<Self> {
        Self::bind_with(Some(handler), None)
    }

    fn bind_with(handler: Option<Handler>, book: Option<Arc<super::proposal::Book>>) -> std::io::Result<Self> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        use std::os::unix::net::UnixListener;
        let path = super::place::socket(&super::place::dir()?)?;
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "control socket path is occupied"));
            }
            if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "control socket is already listening"));
            }
            std::fs::remove_file(&path)?;
        }
        // **A blocking listener.** An idle window never wakes this thread, and the streams it accepts are blocking on
        // every platform — BSD and macOS copy O_NONBLOCK from the listener to each accepted socket, which turned the
        // client's read timeout off and cut large replies short with EAGAIN. `Drop` wakes the accept by connecting once.
        let listener = UnixListener::bind(&path)?;
        let (tx, incoming) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let active = Arc::new(AtomicUsize::new(0));
        let waiting = Arc::new(AtomicUsize::new(0));
        let keep_book = book.clone();
        std::thread::Builder::new().name("moai-ctl-listen".into()).spawn(move || {
            for accepted in listener.incoming() {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                // A refused accept (EMFILE and the like) passes; it must not end control for the window's lifetime.
                let Ok(mut stream) = accepted else {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                };
                if active.fetch_add(1, Ordering::AcqRel) >= 8 {
                    active.fetch_sub(1, Ordering::AcqRel);
                    // The accept loop never waits for a client to read its refusal.
                    let _ = stream.set_nonblocking(true);
                    let _ = write_reply(&mut stream, &Reply::error("busy", "too many control clients"));
                    continue;
                }
                let tx = tx.clone();
                let handler = handler.clone();
                let book = book.clone();
                let client_active = Arc::clone(&active);
                let client_waiting = Arc::clone(&waiting);
                let spawned = std::thread::Builder::new().name("moai-ctl-client".into()).spawn(move || {
                    let mut leave = Some(Leave(client_active));
                    handle(&mut stream, &tx, handler.as_ref(), book.as_deref(), &mut leave, &client_waiting);
                });
                if spawned.is_err() {
                    active.fetch_sub(1, Ordering::AcqRel);
                }
            }
        })?;
        Ok(Self { incoming, path, stop, book: keep_book })
    }
}

#[cfg(not(unix))]
impl Listener {
    pub fn bind_proposals(_: Arc<super::proposal::Book>) -> std::io::Result<Self> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "control sockets are unavailable on this OS"))
    }

    pub fn bind_web(_: Handler) -> std::io::Result<Self> {
        Self::bind_proposals(Arc::new(super::proposal::Book::new()))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake the blocking accept so its thread sees `stop` and lets the listener go.
        #[cfg(unix)]
        let _ = std::os::unix::net::UnixStream::connect(&self.path);
        if let Some(book) = &self.book {
            book.close();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn handle(
    stream: &mut std::os::unix::net::UnixStream,
    tx: &mpsc::Sender<Incoming>,
    handler: Option<&Handler>,
    book: Option<&super::proposal::Book>,
    leave: &mut Option<Leave>,
    waiting: &Arc<AtomicUsize>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut line = Vec::new();
    let read = BufReader::new(&mut *stream).take(wire::LINE_MAX as u64 + 1).read_until(b'\n', &mut line);
    let reply = match read {
        Ok(_) if line.len() > wire::LINE_MAX => Reply::error("too_large", "control request exceeds 1 MiB"),
        Ok(_) if !line.ends_with(b"\n") => Reply::error("bad_input", "control request needs one JSON line"),
        Ok(_) => match serde_json::from_slice::<Request>(&line) {
            Ok(req) if req.proto != wire::PROTO => Reply::error("unsupported", "unknown control protocol"),
            Ok(req) if req.deadline_ms <= wire::now_ms() => Reply::error("timeout", "control request expired"),
            Ok(req) if wait_seconds(&req) > 3600 => {
                Reply::error("bad_input", "proposal wait must be at most 3600 seconds")
            }
            Ok(req) if handler.is_some() => {
                if wait_seconds(&req) > 0 && !wait_slot(waiting, leave) {
                    Reply::error("busy", "too many proposal waiters")
                } else {
                    let wait = wait_seconds(&req) > 0;
                    let reply = handler.unwrap()(req);
                    if wait {
                        waiting.fetch_sub(1, Ordering::AcqRel);
                    }
                    reply
                }
            }
            Ok(req) => {
                let limit = Duration::from_millis(req.deadline_ms.saturating_sub(wire::now_ms()));
                let wait = wait_seconds(&req);
                // **The waiter slot comes before the window sees the request**, as on the web path: reserved after,
                // a refused `--wait` left its suggestion on screen with no id for the caller to answer or withdraw.
                if wait > 0 && !wait_slot(waiting, leave) {
                    Reply::error("busy", "too many proposal waiters")
                } else {
                    let (answer, rx) = mpsc::channel();
                    let reply = if tx.send(Incoming { request: req, answer }).is_err() {
                        Reply::error("gone", "window closed")
                    } else {
                        let first = rx
                            .recv_timeout(limit)
                            .unwrap_or_else(|_| Reply::error("timeout", "window did not answer in time"));
                        if wait == 0
                            || !first.ok
                            || first.data.as_ref().and_then(|d| d["state"].as_str()) != Some("pending")
                        {
                            first
                        } else {
                            let id = first.data.as_ref().and_then(|d| d["id"].as_str()).unwrap_or_default().to_string();
                            book.map_or(first, |book| book.wait(&id, wait))
                        }
                    };
                    if wait > 0 {
                        waiting.fetch_sub(1, Ordering::AcqRel);
                    }
                    reply
                }
            }
            Err(e) => Reply::error("bad_input", e.to_string()),
        },
        Err(e) => Reply::error("bad_input", e.to_string()),
    };
    let _ = write_reply(stream, &reply);
}

fn wait_seconds(req: &Request) -> u64 {
    match &req.verb {
        super::wire::Verb::Propose { wait, .. } | super::wire::Verb::Answer { wait, .. } => *wait,
        _ => 0,
    }
}

fn wait_slot(waiting: &Arc<AtomicUsize>, leave: &mut Option<Leave>) -> bool {
    if waiting.fetch_add(1, Ordering::AcqRel) >= 4 {
        waiting.fetch_sub(1, Ordering::AcqRel);
        return false;
    }
    drop(leave.take());
    true
}

fn write_reply(stream: &mut impl Write, reply: &Reply) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, reply)?;
    stream.write_all(b"\n")
}
