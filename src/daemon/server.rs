//! Unix 소켓의 줄 전송 — 줄 하나에 JSON-RPC 메시지 하나.
//!
//! **연결마다 스레드 하나, 그 연결의 요청은 그 스레드에서 차례로 하나씩**(moai-5n6m.x8t). 받는 스레드
//! (`serve` 를 부른 스레드)는 요청을 돌리지 않고, 연결 스레드는 연결이 끝나면 함께 끝난다 — 스레드를 모아 두고
//! 다시 쓰지 않는다. 그래서 요청 하나가 도는 스레드에서 다른 요청이 도는 일이 없고, 한 요청이 스레드에 남긴 것은
//! 그 연결 밖으로 새지 않는다. 라이브러리가 요청마다 스레드 하나를 요구해도(moai-5n6m.m0b) 이 자리가 그것을 진다.
//!
//! 사용자당 하나는 자물쇠 파일(`daemon.lock` 의 `flock`, `fs2`)이 지킨다. 데몬은 섬기는 내내 그것을 쥐고, 프로세스가
//! 죽으면 커널이 놓는다 — 그래서 자물쇠를 쥔 쪽이 본 소켓은 죽은 데몬의 것이다. 쥐지 않고 "붙어 보고 지우기"
//! 만 하면, 둘이 동시에 뜰 때 늦은 쪽이 먼저 뜬 쪽의 새 소켓을 지운다.

use super::rpc::{self, Methods, RpcError};
use serde_json::json;
use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// 동시에 여는 연결의 상한. 넘으면 오류 한 줄(`data.code` 가 `busy`)을 주고 닫는다.
pub const MAX_CONNECTIONS: usize = 32;
/// 아무 말 없는 연결을 닫는 시한 — 잊힌 연결이 상한을 다 채우지 않게.
const IDLE: Duration = Duration::from_secs(600);
/// 답을 쓰는 시한 — 안 읽는 쪽에 스레드가 묶이지 않게.
const WRITE: Duration = Duration::from_secs(30);
/// 멈출 때 돌던 요청이 끝나기를 기다리는 시한.
const DRAIN: Duration = Duration::from_secs(5);
/// 받는 스레드가 `stop` 을 보는 간격 — 받을 연결이 없어도 이만큼마다 깬다. 멈추는 답을 쓴 연결이 제 소켓에
/// 붙어 곧바로 깨우지만, 그 붙기는 실패할 수 있다(소켓 파일이 지워졌다). 그때도 이 간격 안에 그친다.
/// `TICK` 과 [`DRAIN`] 의 합이 `moai daemon stop` 의 기다림(10초)보다 짧아야 한다.
const TICK: Duration = Duration::from_millis(500);

/// `daemon.lock` 을 쥔다. 남이 쥐었으면 `None`. 놓는 것은 돌려받은 파일을 닫는 것(drop)이다.
pub fn lock(socket: &Path) -> std::io::Result<Option<std::fs::File>> {
    use fs2::FileExt as _;
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(socket.with_file_name("daemon.lock"))?;
    // `store` 의 쓰기 자물쇠와 같은 길(`fs2`, 유닉스에서는 flock)이다.
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// 이 사용자의 소켓인가 — 종류와 주인. 남의 파일은 지우지도 붙지도 않는다.
pub fn is_own_socket(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // SAFETY: geteuid 는 실패하지 않는다.
    meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() }
}

/// 그 자리의 파일이 아직 `ino` 의 그 소켓일 때만 지운다 — 보고 지우는 사이에 남이 세운 것을 지우지 않는다.
pub fn remove_if_same(path: &Path, ino: u64) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if is_own_socket(&meta) && meta.ino() == ino => std::fs::remove_file(path),
        _ => Ok(()),
    }
}

/// 묶은 소켓. [`serve`] 가 끝나며 제 소켓을 지운다.
pub struct Bound {
    listener: UnixListener,
    path: PathBuf,
    ino: u64,
}

/// 소켓을 묶는다. 그 자리에 남은 것은 부르는 쪽이 자물쇠를 쥐고 먼저 치운다.
///
/// **막는 받기다** — `ctl::serve` 와 같은 까닭: 받는 소켓에 O_NONBLOCK 을 걸면 BSD·macOS 는 받은 연결에도
/// 그것을 물려 연결의 읽기 시한이 꺼진다. 그래서 [`serve`] 는 `poll(2)` 로 [`TICK`] 까지만 기다렸다가, 받을
/// 연결이 섰을 때만 막는 `accept` 를 부른다.
pub fn bind(path: &Path) -> std::io::Result<Bound> {
    use std::os::unix::fs::MetadataExt;
    let listener = UnixListener::bind(path)?;
    let ino = std::fs::symlink_metadata(path)?.ino();
    Ok(Bound { listener, path: path.to_path_buf(), ino })
}

/// 섬긴다 — [`super::SHUTDOWN`] 이 오기까지. 돌아오기 전에 제 소켓을 지운다(그 자리가 아직 제 소켓일 때만).
pub fn serve(bound: Bound, mut methods: Methods) -> std::io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    // 답을 쓴 뒤에 받기를 깨운다(`connection`) — 여기서 깨우면 답이 나가기 전에 프로세스가 끝날 수 있다. 받는
    // 스레드가 `TICK` 에 먼저 깨어도 답을 쓰기까지는 `Running` 이 세어 두어 `DRAIN` 이 기다린다.
    methods.register(super::SHUTDOWN, move |_, _| {
        flag.store(true, Ordering::Release);
        Ok(json!({ "pid": std::process::id() }))
    });
    let methods = Arc::new(methods);
    let open = Arc::new(AtomicUsize::new(0));
    let busy = Arc::new(AtomicUsize::new(0));
    let path = Arc::new(bound.path.clone());
    while !stop.load(Ordering::Acquire) {
        // 받기가 실패한 것(EMFILE 따위)은 넘긴다 — 데몬의 수명을 끝낼 일이 아니다.
        match pending(&bound.listener, TICK) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Ok((mut stream, _)) = bound.listener.accept() else {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        };
        if open.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            open.fetch_sub(1, Ordering::AcqRel);
            // 받는 스레드는 거절을 읽어 가기를 기다리지 않는다.
            let _ = stream.set_nonblocking(true);
            let refusal = rpc::refusal(&RpcError::moai(crate::fail::code::BUSY, "too many connections to the daemon"));
            let _ = writeln!(stream, "{refusal}");
            continue;
        }
        let leave = Leave(Arc::clone(&open));
        let (methods, stop, path, busy) =
            (Arc::clone(&methods), Arc::clone(&stop), Arc::clone(&path), Arc::clone(&busy));
        // 띄우지 못하면 닫힌 클로저가 `leave` 를 놓아 수가 돌아온다.
        let _ = std::thread::Builder::new().name("moai-daemon-conn".into()).spawn(move || {
            let _leave = leave;
            connection(stream, &methods, &stop, &path, &busy);
        });
    }
    // 돌던 요청은 끝나게 둔다 — 말없이 기다리는 연결은 기다리지 않는다.
    let deadline = Instant::now() + DRAIN;
    while busy.load(Ordering::Acquire) > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    remove_if_same(&bound.path, bound.ino)
}

/// 받을 연결이 섰는가 — `wait` 까지만 기다린다. 신호에 끊긴 기다림은 "아직 없다" 로 읽는다.
///
/// 섰다고 답한 뒤의 `accept` 는 막히지 않는다: 그 연결을 가져갈 다른 받는 쪽이 없고, 붙은 쪽이 그새 닫았어도
/// Unix 소켓은 그 연결을 대기열에 둔 채 넘긴다(받은 쪽이 곧 EOF 를 읽는다).
fn pending(listener: &UnixListener, wait: Duration) -> std::io::Result<bool> {
    use std::os::unix::io::AsRawFd;
    let mut fd = libc::pollfd { fd: listener.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    let ms = libc::c_int::try_from(wait.as_millis()).unwrap_or(libc::c_int::MAX);
    // SAFETY: 살아 있는 `pollfd` 하나를 길이 1 로 넘긴다 — poll 은 그 `revents` 만 쓴다.
    match unsafe { libc::poll(&mut fd, 1, ms) } {
        0 => Ok(false),
        n if n > 0 && fd.revents & libc::POLLIN != 0 => Ok(true),
        n if n > 0 => Err(std::io::Error::other(format!("poll revents {:#x}", fd.revents))),
        _ => match std::io::Error::last_os_error() {
            e if e.kind() == std::io::ErrorKind::Interrupted => Ok(false),
            e => Err(e),
        },
    }
}

struct Leave(Arc<AtomicUsize>);
impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 돌고 있는 요청 하나 — 처리 함수에 들기 전에 세고, **답을 쓴 뒤에** 놓는다. 멈추는 데몬은 이 수가 0 이 되기를
/// 기다리니(`DRAIN`), `daemon.shutdown` 의 답이 나가기 전에 받는 스레드가 `stop` 을 보아도(`TICK`) 프로세스가
/// 그 답보다 먼저 끝나지 않는다.
struct Running<'a>(&'a AtomicUsize);

impl<'a> Running<'a> {
    fn start(busy: &'a AtomicUsize) -> Running<'a> {
        busy.fetch_add(1, Ordering::AcqRel);
        Running(busy)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 연결 하나 — 줄을 읽고, 돌리고, 답을 쓰고, 다음 줄을 읽는다. 앞 요청의 답을 쓰기 전에는 다음 줄을 안 읽는다.
fn connection(stream: UnixStream, methods: &Methods, stop: &AtomicBool, path: &Path, busy: &AtomicUsize) {
    if stream.set_read_timeout(Some(IDLE)).is_err() || stream.set_write_timeout(Some(WRITE)).is_err() {
        return;
    }
    let Ok(read_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read_half);
    let mut writer = stream;
    loop {
        let (reply, running) = match rpc::read_line(&mut reader) {
            Ok(rpc::Line::Eof) | Err(_) => return,
            // 상한을 넘은 줄은 이미 버렸다 — 거절하고 이 연결로 다음 줄을 받는다.
            Ok(rpc::Line::TooLong) => (Some(rpc::too_long()), None),
            Ok(rpc::Line::Text(line)) => {
                let running = Running::start(busy);
                (methods.handle(&line), Some(running))
            }
        };
        if let Some(reply) = reply
            && writeln!(writer, "{reply}").and_then(|()| writer.flush()).is_err()
        {
            return;
        }
        drop(running);
        if stop.load(Ordering::Acquire) {
            // 받기를 곧바로 깨운다 — 그 스레드가 `stop` 을 보고 그친다. 붙기가 실패해도(소켓 파일이 지워졌다)
            // 받는 스레드는 [`TICK`] 안에 스스로 `stop` 을 본다.
            let _ = UnixStream::connect(path);
            return;
        }
    }
}
