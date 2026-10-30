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
}

pub type Handler = Arc<dyn Fn(Request) -> Reply + Send + Sync>;

#[cfg(unix)]
impl Listener {
    pub fn bind() -> std::io::Result<Self> {
        Self::bind_with(None)
    }

    /// The web server answers from its tab registry; the TUI keeps its owner-loop channel.
    pub fn bind_web(handler: Handler) -> std::io::Result<Self> {
        Self::bind_with(Some(handler))
    }

    fn bind_with(handler: Option<Handler>) -> std::io::Result<Self> {
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
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        let (tx, incoming) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let active = Arc::new(AtomicUsize::new(0));
        std::thread::Builder::new().name("moai-ctl-listen".into()).spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if active.fetch_add(1, Ordering::AcqRel) >= 8 {
                            active.fetch_sub(1, Ordering::AcqRel);
                            // The accept loop never waits for a client to read its refusal.
                            let _ = stream.set_nonblocking(true);
                            let _ = write_reply(&mut stream, &Reply::error("busy", "too many control clients"));
                            continue;
                        }
                        let tx = tx.clone();
                        let handler = handler.clone();
                        let client_active = Arc::clone(&active);
                        let spawned = std::thread::Builder::new().name("moai-ctl-client".into()).spawn(move || {
                            struct Leave(Arc<AtomicUsize>);
                            impl Drop for Leave {
                                fn drop(&mut self) {
                                    self.0.fetch_sub(1, Ordering::AcqRel);
                                }
                            }
                            let _leave = Leave(client_active);
                            handle(&mut stream, &tx, handler.as_ref());
                        });
                        if spawned.is_err() {
                            active.fetch_sub(1, Ordering::AcqRel);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
        })?;
        Ok(Self { incoming, path, stop })
    }
}

#[cfg(not(unix))]
impl Listener {
    pub fn bind() -> std::io::Result<Self> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "control sockets are unavailable on this OS"))
    }

    pub fn bind_web(_: Handler) -> std::io::Result<Self> {
        Self::bind()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn handle(stream: &mut std::os::unix::net::UnixStream, tx: &mpsc::Sender<Incoming>, handler: Option<&Handler>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut line = Vec::new();
    let read = BufReader::new(&mut *stream).take(wire::LINE_MAX as u64 + 1).read_until(b'\n', &mut line);
    let reply = match read {
        Ok(_) if line.len() > wire::LINE_MAX => Reply::error("too_large", "control request exceeds 1 MiB"),
        Ok(_) if !line.ends_with(b"\n") => Reply::error("bad_input", "control request needs one JSON line"),
        Ok(_) => match serde_json::from_slice::<Request>(&line) {
            Ok(req) if req.proto != wire::PROTO => Reply::error("unsupported", "unknown control protocol"),
            Ok(req) if req.deadline_ms <= wire::now_ms() => Reply::error("timeout", "control request expired"),
            Ok(req) if handler.is_some() => handler.unwrap()(req),
            Ok(req) => {
                let wait = Duration::from_millis(req.deadline_ms.saturating_sub(wire::now_ms()).min(30_000));
                let (answer, rx) = mpsc::channel();
                if tx.send(Incoming { request: req, answer }).is_err() {
                    Reply::error("gone", "window closed")
                } else {
                    rx.recv_timeout(wait).unwrap_or_else(|_| Reply::error("timeout", "window did not answer in time"))
                }
            }
            Err(e) => Reply::error("bad_input", e.to_string()),
        },
        Err(e) => Reply::error("bad_input", e.to_string()),
    };
    let _ = write_reply(stream, &reply);
}

fn write_reply(stream: &mut impl Write, reply: &Reply) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, reply)?;
    stream.write_all(b"\n")
}
