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
/// 그것을 물려 연결의 읽기 시한이 꺼진다. 멈출 때는 제 소켓에 한 번 붙어 받기를 깨운다.
pub fn bind(path: &Path) -> std::io::Result<Bound> {
    use std::os::unix::fs::MetadataExt;
    let listener = UnixListener::bind(path)?;
    let ino = std::fs::symlink_metadata(path)?.ino();
    Ok(Bound { listener, path: path.to_path_buf(), ino })
}

/// 섬긴다 — [`super::SHUTDOWN`] 이 오기까지. 돌아오기 전에 제 소켓을 지운다.
pub fn serve(bound: Bound, mut methods: Methods) -> std::io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    // 답을 쓴 뒤에 받기를 깨운다(`connection`) — 여기서 깨우면 답이 나가기 전에 프로세스가 끝날 수 있다.
    methods.register(super::SHUTDOWN, move |_, _| {
        flag.store(true, Ordering::Release);
        Ok(json!({ "pid": std::process::id() }))
    });
    let methods = Arc::new(methods);
    let open = Arc::new(AtomicUsize::new(0));
    let busy = Arc::new(AtomicUsize::new(0));
    let path = Arc::new(bound.path.clone());
    for accepted in bound.listener.incoming() {
        if stop.load(Ordering::Acquire) {
            break;
        }
        // 받기가 실패한 것(EMFILE 따위)은 넘긴다 — 데몬의 수명을 끝낼 일이 아니다.
        let Ok(mut stream) = accepted else {
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

struct Leave(Arc<AtomicUsize>);
impl Drop for Leave {
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
        let reply = match rpc::read_line(&mut reader) {
            Ok(rpc::Line::Eof) | Err(_) => return,
            // 상한을 넘은 줄은 이미 버렸다 — 거절하고 이 연결로 다음 줄을 받는다.
            Ok(rpc::Line::TooLong) => Some(rpc::too_long()),
            Ok(rpc::Line::Text(line)) => {
                busy.fetch_add(1, Ordering::AcqRel);
                let reply = methods.handle(&line);
                busy.fetch_sub(1, Ordering::AcqRel);
                reply
            }
        };
        if let Some(reply) = reply
            && writeln!(writer, "{reply}").and_then(|()| writer.flush()).is_err()
        {
            return;
        }
        if stop.load(Ordering::Acquire) {
            // 받기를 깨운다 — 그 스레드가 `stop` 을 보고 그친다.
            let _ = UnixStream::connect(path);
            return;
        }
    }
}
