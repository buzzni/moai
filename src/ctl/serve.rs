#[cfg(unix)]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::sync::atomic::AtomicUsize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[cfg(unix)]
use std::time::Duration;
use std::time::Instant;

#[cfg(unix)]
use super::wire;
use super::wire::{Reply, Request};

pub struct Incoming {
    pub request: Request,
    pub answer: mpsc::Sender<Reply>,
}

/// The owner loop alone consumes requests and owns the application state.
pub struct Listener {
    pub incoming: mpsc::Receiver<Incoming>,
    #[cfg(unix)]
    wake: std::os::unix::net::UnixStream,
    path: std::path::PathBuf,
    stop: Arc<AtomicBool>,
    book: Option<Arc<super::proposal::Book>>,
    editing: Arc<Editing>,
}

#[derive(Default)]
struct Editing {
    active: AtomicBool,
    hello: Mutex<Option<(Reply, Instant)>>,
}

impl Editing {
    #[cfg(unix)]
    fn answer(&self, request: &Request) -> Option<Reply> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }
        if !matches!(request.verb, super::wire::Verb::Hello) {
            return Some(Reply::error("person_busy", "the person is in their editor"));
        }
        let mut snapshot = self.hello.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
        if let Some(data) = snapshot.0.data.as_mut() {
            let ago = data["last_input_ago_ms"].as_u64().unwrap_or_default();
            data["last_input_ago_ms"] = ago.saturating_add(snapshot.1.elapsed().as_millis() as u64).into();
        }
        Some(snapshot.0)
    }
}

/// Owner-loop wakeups and editor suspension; application state stays on the owner thread.
impl Listener {
    /// Wait for terminal input or a newly queued owner-loop request. Crossterm still parses and consumes every
    /// terminal event on the owner thread; this socket only interrupts its idle wait.
    pub fn poll_input(&self, timeout: std::time::Duration) -> std::io::Result<bool> {
        use ratatui::crossterm::event;
        if event::poll(std::time::Duration::ZERO)? {
            return Ok(true);
        }
        #[cfg(unix)]
        {
            use std::io::IsTerminal;
            use std::os::fd::AsRawFd;
            // Match crossterm's input source even when stdin is redirected. Crossterm opens /dev/tty once; so does
            // this wait, rather than once per turn of the owner loop.
            static TTY: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
            let terminal = if std::io::stdin().is_terminal() {
                std::io::stdin().as_raw_fd()
            } else if let Some(tty) = TTY.get() {
                tty.as_raw_fd()
            } else {
                let tty = std::fs::File::open("/dev/tty")?;
                TTY.get_or_init(|| tty).as_raw_fd()
            };
            self.wait_input(terminal, timeout, || event::poll(std::time::Duration::ZERO))
        }
        #[cfg(not(unix))]
        event::poll(timeout)
    }

    /// One idle wait on the terminal and the wake socket; `parsed` then asks crossterm whether an event came of it.
    ///
    /// **A terminal that stays readable while crossterm reports nothing is not waited on again this turn.** Crossterm's
    /// source is edge-triggered and returns as soon as its 1 KiB read yields an event, so input past that read stays in
    /// the terminal and is reported only when more arrives. This level-triggered wait would return at once on every turn
    /// and spin the owner loop until the next keypress; the rest of the turn waits on the wake socket alone instead, as
    /// crossterm's own wait slept until new input.
    #[cfg(unix)]
    fn wait_input(
        &self,
        terminal: std::os::fd::RawFd,
        timeout: Duration,
        mut parsed: impl FnMut() -> std::io::Result<bool>,
    ) -> std::io::Result<bool> {
        let started = Instant::now();
        let typed = self.wait_on(Some(terminal), timeout)?;
        if parsed()? {
            return Ok(true);
        }
        if typed && self.wait_on(Some(terminal), Duration::ZERO)? {
            self.wait_on(None, timeout.saturating_sub(started.elapsed()))?;
            return parsed();
        }
        Ok(false)
    }

    /// Wait for the terminal (when given) or the wake socket, drain the wake bytes, and say whether the terminal stirred.
    #[cfg(unix)]
    fn wait_on(&self, terminal: Option<std::os::fd::RawFd>, timeout: Duration) -> std::io::Result<bool> {
        use std::os::fd::AsRawFd;
        // A negative descriptor is skipped by poll(2) and reports no events.
        let mut fds = [
            libc::pollfd { fd: terminal.unwrap_or(-1), events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: self.wake.as_raw_fd(), events: libc::POLLIN, revents: 0 },
        ];
        // Round up: a fractional millisecond must not turn an idle wait into a busy loop.
        let ms = timeout.as_nanos().div_ceil(1_000_000);
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms.min(i32::MAX as u128) as i32) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            // A SIGWINCH delivered to this thread interrupts poll; let crossterm read its resize notification. One that
            // lands on another thread, or just before this poll, is seen at the end of the wait.
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        let mut wake = &self.wake;
        let mut bytes = [0; 256];
        loop {
            match wake.read(&mut bytes) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(ready > 0 && fds[0].revents != 0)
    }

    /// Cache Hello before handing the terminal to the editor. Directed actions then return person_busy.
    pub fn editor_begin(&self, hello: Reply) {
        *self.editing.hello.lock().unwrap_or_else(|e| e.into_inner()) = Some((hello, Instant::now()));
        self.editing.active.store(true, Ordering::Release);
    }

    pub fn editor_end(&self) {
        self.editing.active.store(false, Ordering::Release);
        *self.editing.hello.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub type Handler = Arc<dyn Fn(Request, &dyn Fn() -> bool) -> Reply + Send + Sync>;

#[cfg(unix)]
struct Leave(Arc<AtomicUsize>);
#[cfg(unix)]
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
        let (wake, notify) = std::os::unix::net::UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        notify.set_nonblocking(true)?;
        let notify = Arc::new(notify);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let active = Arc::new(AtomicUsize::new(0));
        let waiting = Arc::new(AtomicUsize::new(0));
        let keep_book = book.clone();
        let editing = Arc::new(Editing::default());
        let listen_editing = Arc::clone(&editing);
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
                let notify = Arc::clone(&notify);
                let handler = handler.clone();
                let book = book.clone();
                let client_active = Arc::clone(&active);
                let client_waiting = Arc::clone(&waiting);
                let client_editing = Arc::clone(&listen_editing);
                let spawned = std::thread::Builder::new().name("moai-ctl-client".into()).spawn(move || {
                    let mut leave = Some(Leave(client_active));
                    handle(
                        &mut stream,
                        &tx,
                        handler.as_ref(),
                        book.as_deref(),
                        &mut leave,
                        &client_waiting,
                        &client_editing,
                        Some(&notify),
                    );
                });
                if spawned.is_err() {
                    active.fetch_sub(1, Ordering::AcqRel);
                }
            }
        })?;
        Ok(Self { incoming, wake, path, stop, book: keep_book, editing })
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
    editing: &Editing,
    notify: Option<&std::os::unix::net::UnixStream>,
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
            Ok(req) if editing.active.load(Ordering::Acquire) => {
                editing.answer(&req).unwrap_or_else(|| Reply::error("person_busy", "the person is in their editor"))
            }
            Ok(req) if handler.is_some() => {
                if wait_seconds(&req) > 0 && !wait_slot(waiting, leave) {
                    Reply::error("busy", "too many proposal waiters")
                } else {
                    let wait = wait_seconds(&req) > 0;
                    let reply = handler.unwrap()(req, &|| client_connected(stream));
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
                        if let Some(mut notify) = notify {
                            // Nonblocking: a full socket already carries a wakeup.
                            let _ = notify.write_all(&[1]);
                        }
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
                            book.map_or(first, |book| book.wait_while(&id, wait, || client_connected(stream)))
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

#[cfg(unix)]
fn client_connected(stream: &std::os::unix::net::UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    // **A hang-up, not end-of-input.** A caller of the line protocol (docs/ctl.md) may shut down its writing half once
    // its request is sent (`nc -N -U`, `shutdown(SHUT_WR)`) and still read the reply; a peeked EOF told it `gone` at
    // once, and a byte it left unread hid a real close. POLLHUP comes only when both halves are shut. POLLIN is asked
    // for because macOS reports the hang-up only through a read filter. A zero timeout leaves blocking mode alone.
    let mut fd = libc::pollfd { fd: stream.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    let ready = unsafe { libc::poll(&mut fd, 1, 0) };
    !(ready > 0 && fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0)
}

#[cfg(unix)]
fn wait_seconds(req: &Request) -> u64 {
    match &req.verb {
        super::wire::Verb::Propose { wait, .. } | super::wire::Verb::Answer { wait, .. } => *wait,
        _ => 0,
    }
}

#[cfg(unix)]
fn wait_slot(waiting: &Arc<AtomicUsize>, leave: &mut Option<Leave>) -> bool {
    if waiting.fetch_add(1, Ordering::AcqRel) >= 4 {
        waiting.fetch_sub(1, Ordering::AcqRel);
        return false;
    }
    drop(leave.take());
    true
}

#[cfg(unix)]
fn write_reply(stream: &mut impl Write, reply: &Reply) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, reply)?;
    stream.write_all(b"\n")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::ctl::wire::Verb;

    /// A listener with only its wake pair, no socket file or accept thread; the second value is its writer, kept alive by
    /// the caller as the accept thread keeps it (`lessons/keep-notification-writers-alive-in-idle-tests`).
    fn wake_listener(name: &str) -> (Listener, mpsc::Sender<Incoming>, std::os::unix::net::UnixStream) {
        let (wake, notify) = std::os::unix::net::UnixStream::pair().unwrap();
        wake.set_nonblocking(true).unwrap();
        notify.set_nonblocking(true).unwrap();
        let (tx, incoming) = mpsc::channel();
        let listener = Listener {
            incoming,
            wake,
            path: std::env::temp_dir().join(format!("moai-{name}-unused-{}", std::process::id())),
            stop: Arc::new(AtomicBool::new(false)),
            book: None,
            editing: Arc::new(Editing::default()),
        };
        (listener, tx, notify)
    }

    #[test]
    fn a_queued_request_wakes_once_and_leaves_no_fast_idle_tail() {
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;
        let (listener, tx, notify) = wake_listener("4km9-wake");
        let _keep_notify = notify.try_clone().unwrap();
        // An empty, open terminal descriptor stays quiet without using the person's terminal.
        let (terminal, _keep_terminal) = UnixStream::pair().unwrap();
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            let req = wire::request(Verb::Hello, Duration::from_secs(5));
            serde_json::to_writer(&mut client, &req).unwrap();
            client.write_all(b"\n").unwrap();
            handle(
                &mut server,
                &tx,
                None,
                None,
                &mut None,
                &Arc::new(AtomicUsize::new(0)),
                &Editing::default(),
                Some(&notify),
            );
        });
        let start = Instant::now();
        assert!(!listener.wait_on(Some(terminal.as_raw_fd()), Duration::from_secs(3)).unwrap());
        assert!(start.elapsed() < Duration::from_secs(1), "a queued Hello waited for the idle timer");
        let turn = listener.incoming.try_recv().unwrap();
        turn.answer.send(Reply::success(serde_json::json!({}))).unwrap();
        worker.join().unwrap();
        let idle = Instant::now();
        listener.wait_on(Some(terminal.as_raw_fd()), Duration::from_millis(50)).unwrap();
        assert!(idle.elapsed() >= Duration::from_millis(40), "a serviced read left an idle wakeup behind");
    }

    /// Input crossterm left unread (past its 1 KiB read, edge-triggered) keeps the terminal level-readable while crossterm
    /// reports nothing. The wait must still sleep out its turn, and a queued request must still cut it short.
    #[test]
    fn a_terminal_crossterm_left_unread_does_not_spin_the_idle_wait() {
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;
        let (listener, _tx, notify) = wake_listener("4km9-stalled");
        let keep_notify = notify.try_clone().unwrap();
        let (terminal, mut typed) = UnixStream::pair().unwrap();
        typed.write_all(b"x").unwrap();
        let start = Instant::now();
        assert!(!listener.wait_input(terminal.as_raw_fd(), Duration::from_millis(60), || Ok(false)).unwrap());
        assert!(start.elapsed() >= Duration::from_millis(50), "a readable terminal with nothing parsed spun the loop");
        let poke = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            (&notify).write_all(&[1]).unwrap();
        });
        let start = Instant::now();
        assert!(!listener.wait_input(terminal.as_raw_fd(), Duration::from_secs(3), || Ok(false)).unwrap());
        assert!(start.elapsed() < Duration::from_secs(1), "a queued request waited behind a stalled terminal");
        poke.join().unwrap();
        // An event crossterm does parse still ends the wait at once.
        let start = Instant::now();
        assert!(listener.wait_input(terminal.as_raw_fd(), Duration::from_secs(3), || Ok(true)).unwrap());
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(keep_notify);
    }

    #[test]
    fn an_editor_keeps_the_window_listed_and_refuses_actions_without_waiting_for_the_loop() {
        use std::os::unix::net::UnixStream;
        let editing = Editing::default();
        *editing.hello.lock().unwrap() = Some((
            Reply::success(serde_json::json!({
                "id": "tui-42", "root": "/repo", "screen": "list", "last_input_ago_ms": 7,
            })),
            Instant::now(),
        ));
        editing.active.store(true, Ordering::Release);
        let (tx, incoming) = mpsc::channel();
        let waiting = Arc::new(AtomicUsize::new(0));
        let ask = |verb| {
            let (mut client, mut server) = UnixStream::pair().unwrap();
            let request = wire::request(verb, Duration::from_secs(2));
            serde_json::to_writer(&mut client, &request).unwrap();
            client.write_all(b"\n").unwrap();
            let mut leave = None;
            handle(&mut server, &tx, None, None, &mut leave, &waiting, &editing, None);
            let mut line = String::new();
            BufReader::new(client).read_line(&mut line).unwrap();
            serde_json::from_str::<Reply>(&line).unwrap()
        };
        let listed = ask(Verb::Hello);
        assert_eq!(listed.data.unwrap()["id"], "tui-42");
        assert_eq!(ask(Verb::State { rows: false }).code.as_deref(), Some("person_busy"));
        assert!(
            matches!(incoming.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "an action reached the paused owner loop"
        );
    }

    #[test]
    fn four_disconnected_waiters_release_their_slots_for_a_fifth() {
        use std::os::unix::net::UnixStream;
        let book = Arc::new(super::super::proposal::Book::new());
        let waiting = Arc::new(AtomicUsize::new(0));
        let editing = Arc::new(Editing::default());
        let (tx, incoming) = mpsc::channel::<Incoming>();
        let owner_book = Arc::clone(&book);
        let owner = std::thread::spawn(move || {
            for turn in incoming {
                let Verb::Propose { text, ttl, .. } = turn.request.verb else { panic!("expected proposal") };
                let answer = owner_book.add("window", &text, None, ttl, None);
                turn.answer.send(answer).unwrap();
            }
        });
        let ask = |label: &str| {
            let (mut client, server) = UnixStream::pair().unwrap();
            let request = wire::request(
                Verb::Propose { text: label.into(), then: None, ttl: 60, wait: 30 },
                Duration::from_secs(5),
            );
            serde_json::to_writer(&mut client, &request).unwrap();
            client.write_all(b"\n").unwrap();
            (client, server)
        };
        let mut handles = Vec::new();
        let started = Instant::now();
        for n in 0..4 {
            let (client, mut server) = ask(&format!("disconnected {n}"));
            let tx = tx.clone();
            let book = Arc::clone(&book);
            let waiting = Arc::clone(&waiting);
            let editing = Arc::clone(&editing);
            handles.push(std::thread::spawn(move || {
                let mut leave = None;
                handle(&mut server, &tx, None, Some(&book), &mut leave, &waiting, &editing, None);
            }));
            drop(client);
        }
        for handle in handles {
            handle.join().unwrap();
        }
        // Without the disconnect check each would hold its slot to the end of its 30-second wait and still join.
        assert!(started.elapsed() < Duration::from_secs(10), "disconnected waiters kept their slots");
        assert_eq!(waiting.load(Ordering::Acquire), 0);
        let (client, mut server) = ask("live fifth");
        let fifth_tx = tx.clone();
        let fifth_book = Arc::clone(&book);
        let fifth_waiting = Arc::clone(&waiting);
        let fifth_editing = Arc::clone(&editing);
        let fifth = std::thread::spawn(move || {
            let mut leave = None;
            handle(&mut server, &fifth_tx, None, Some(&fifth_book), &mut leave, &fifth_waiting, &fifth_editing, None);
        });
        let until = Instant::now() + Duration::from_secs(2);
        while book.rows().len() < 5 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(book.rows().len(), 5);
        book.finish("window/5", "accepted", None);
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&line).unwrap().data.unwrap()["state"], "accepted");
        fifth.join().unwrap();
        drop(tx);
        owner.join().unwrap();
    }

    #[test]
    fn web_waiters_also_release_slots_after_four_callers_disconnect() {
        use std::os::unix::net::UnixStream;
        let tabs = Arc::new(crate::web::ctl::Tabs::new());
        let joined = tabs.join(std::path::Path::new("/tmp"), false, None).unwrap();
        let of = Arc::clone(&tabs);
        let handler: Handler = Arc::new(move |request, connected| of.handle_while(request, connected));
        let waiting = Arc::new(AtomicUsize::new(0));
        let editing = Arc::new(Editing::default());
        let (tx, _) = mpsc::channel();
        let ask = |label: &str| {
            let (mut client, server) = UnixStream::pair().unwrap();
            let mut request = wire::request(
                Verb::Propose { text: label.into(), then: None, ttl: 60, wait: 30 },
                Duration::from_secs(5),
            );
            request.target = Some(joined.id.clone());
            serde_json::to_writer(&mut client, &request).unwrap();
            client.write_all(b"\n").unwrap();
            (client, server)
        };
        let mut handles = Vec::new();
        let started = Instant::now();
        for n in 0..4 {
            let (client, mut server) = ask(&format!("web disconnected {n}"));
            let handler = Arc::clone(&handler);
            let waiting = Arc::clone(&waiting);
            let editing = Arc::clone(&editing);
            let tx = tx.clone();
            handles.push(std::thread::spawn(move || {
                let mut leave = None;
                handle(&mut server, &tx, Some(&handler), None, &mut leave, &waiting, &editing, None);
            }));
            drop(client);
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(started.elapsed() < Duration::from_secs(10), "disconnected waiters kept their slots");
        assert_eq!(waiting.load(Ordering::Acquire), 0);
        for _ in 0..4 {
            assert!(matches!(
                joined.incoming.recv_timeout(Duration::from_secs(2)),
                Ok(crate::web::ctl::Push::Proposal { .. })
            ));
        }

        let (client, mut server) = ask("web live fifth");
        let fifth_handler = Arc::clone(&handler);
        let fifth_waiting = Arc::clone(&waiting);
        let fifth_editing = Arc::clone(&editing);
        let fifth_tx = tx.clone();
        let fifth = std::thread::spawn(move || {
            let mut leave = None;
            handle(
                &mut server,
                &fifth_tx,
                Some(&fifth_handler),
                None,
                &mut leave,
                &fifth_waiting,
                &fifth_editing,
                None,
            );
        });
        let crate::web::ctl::Push::Proposal { entry } = joined.incoming.recv_timeout(Duration::from_secs(2)).unwrap()
        else {
            panic!("fifth proposal was not sent")
        };
        tabs.proposal_reply(&joined.id, &joined.token, entry["id"].as_str().unwrap(), true, None).unwrap();
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&line).unwrap().data.unwrap()["state"], "accepted");
        fifth.join().unwrap();
        assert_eq!(waiting.load(Ordering::Acquire), 0);
    }

    /// A caller that shuts down its writing half after its request line is still reading: it is not a disconnect.
    /// Linux only — macOS reports a read-side EOF as a hang-up, so there it is treated as before.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_half_closed_waiter_still_gets_its_answer() {
        use std::os::unix::net::UnixStream;
        let book = Arc::new(super::super::proposal::Book::new());
        let waiting = Arc::new(AtomicUsize::new(0));
        let editing = Editing::default();
        let (tx, incoming) = mpsc::channel::<Incoming>();
        let owner_book = Arc::clone(&book);
        let owner = std::thread::spawn(move || {
            for turn in incoming {
                let Verb::Propose { text, ttl, .. } = turn.request.verb else { panic!("expected proposal") };
                turn.answer.send(owner_book.add("window", &text, None, ttl, None)).unwrap();
            }
        });
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let request =
            wire::request(Verb::Propose { text: "half".into(), then: None, ttl: 60, wait: 30 }, Duration::from_secs(5));
        serde_json::to_writer(&mut client, &request).unwrap();
        client.write_all(b"\n").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let answering = Arc::clone(&book);
        let answer = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(2);
            while answering.rows().is_empty() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            // Past a few disconnect checks, so a half-close read as a hang-up has already answered `gone`.
            std::thread::sleep(Duration::from_millis(500));
            answering.finish("window/1", "accepted", None);
        });
        let mut leave = None;
        handle(&mut server, &tx, None, Some(&book), &mut leave, &waiting, &editing, None);
        answer.join().unwrap();
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&line).unwrap().data.unwrap()["state"], "accepted");
        drop(tx);
        owner.join().unwrap();
    }
}
