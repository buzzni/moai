//! Unix 소켓의 줄 전송 — 줄 하나에 JSON-RPC 메시지 하나.
//!
//! **연결마다 스레드 하나, 그 연결의 요청은 그 스레드에서 차례로 하나씩**(moai-5n6m.x8t). 받는 스레드
//! (`serve` 를 부른 스레드)는 요청을 돌리지 않고, 연결 스레드는 연결이 끝나면 함께 끝난다 — 스레드를 모아 두고
//! 다시 쓰지 않는다. 그래서 요청 하나가 도는 스레드에서 다른 요청이 도는 일이 없고, 한 요청이 스레드에 남긴 것은
//! 그 연결 밖으로 새지 않는다. 라이브러리가 요청마다 스레드 하나를 요구해도(moai-5n6m.m0b) 이 자리가 그것을 진다.
//!
//! **상한에 차도 멈출 수 있다**(moai-uezh). [`MAX_CONNECTIONS`] 를 넘은 연결은 곧바로 거절하지 않고 수명 길
//! ([`LIFECYCLE`], [`LIFECYCLE_SLOTS`] 개)로 보낸다 — 새 스레드 하나가 짧은 시한([`LIFECYCLE_WAIT`]) 안에 줄 하나를
//! 읽어 `hello` 나 [`super::SHUTDOWN`] 이면 돌리고, 다른 메서드면 `busy` 로 거절하고 닫는다. 연결을 새는 클라이언트가
//! 상한을 다 쥐어도 `moai daemon status·stop·start` 가 데몬에 닿는다. 그 길도 새 스레드라 위의 약속은 그대로다.
//! 그 길마저 차면 받는 스레드가 `busy` 한 줄을 쓰고 닫는다. 두 거절의 `data` 에는 `pid` 가 실린다.
//!
//! **멈출 때는 듣기부터 닫고, 돌던 요청을 잃지 않는다**(moai-3hgm.uz0). 멈추기를 보면 소켓 파일을 지우고 듣는 소켓을
//! 닫은 뒤에 돌던 요청을 기다린다 — 그동안 `moai daemon status` 는 "떠 있지 않다" 고 답하고 `start` 는 자물쇠가
//! 풀리기를 기다린다. 요청은 줄을 읽은 뒤에 들이고([`Work`]), 들인 것만 돌린다. 쓰기는 끝까지, 읽기는 [`DRAIN`] 까지
//! 기다린다(사람의 결정 B). 들이지 못한 요청은 돌리지 않고 `stopping` 으로 거절한다.
//!
//! 사용자당 하나는 자물쇠 파일(`daemon.lock` 의 `flock`, `fs2`)이 지킨다. 데몬은 섬기는 내내 그것을 쥐고, 프로세스가
//! 죽으면 커널이 놓는다 — 그래서 자물쇠를 쥔 쪽이 본 소켓은 죽은 데몬의 것이다. 쥐지 않고 "붙어 보고 지우기"
//! 만 하면, 둘이 동시에 뜰 때 늦은 쪽이 먼저 뜬 쪽의 새 소켓을 지운다.

use super::rpc::{self, Methods, Prepared, RpcError};
use serde_json::json;
use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// 동시에 여는 연결의 상한. 넘은 연결은 수명 길([`LIFECYCLE`])로 간다.
pub const MAX_CONNECTIONS: usize = 32;
/// 상한을 넘어서도 섬기는 메서드 — 데몬을 찾고(`hello`) 멈추는(`daemon.shutdown`) 것. 둘 다 곧바로 답한다.
const LIFECYCLE: &[&str] = &["hello", super::SHUTDOWN];
/// 상한을 넘은 연결을 동시에 몇 개까지 수명 길로 받나. 넘으면 `busy` 한 줄을 쓰고 닫는다.
const LIFECYCLE_SLOTS: usize = 4;
/// 수명 길의 읽기·쓰기 시한 — 붙어서 말이 없는 연결이 그 자리를 오래 쥐지 않게. `client::TIMEOUT` 과 같은 값이다.
const LIFECYCLE_WAIT: Duration = Duration::from_secs(2);
/// 아무 말 없는 연결을 닫는 시한 — 잊힌 연결이 상한을 다 채우지 않게.
const IDLE: Duration = Duration::from_secs(600);
/// 답을 쓰는 시한 — 안 읽는 쪽에 스레드가 묶이지 않게.
const WRITE: Duration = Duration::from_secs(30);
/// 멈출 때 돌던 **읽기**가 끝나기를 기다리는 시한. 쓰기는 시한 없이 끝까지 기다린다([`Work::close`]).
const DRAIN: Duration = Duration::from_secs(5);
/// 받는 스레드가 `stop` 을 보는 간격 — 받을 연결이 없어도 이만큼마다 깬다. 멈추는 답을 쓴 연결이 제 소켓에
/// 붙어 곧바로 깨우지만, 그 붙기는 실패할 수 있다(소켓 파일이 지워졌다). 그때도 이 간격 안에 그친다.
/// 읽기만 돌던 데몬이면 `TICK` 과 [`DRAIN`] 의 합이 `moai daemon stop` 의 기다림(10초)보다 짧아야 한다. 쓰기는 시한이
/// 없으니 그보다 오래 걸린 쓰기는 `stop` 을 시한 넘김(`timeout`)으로 끝내고, 데몬은 그 쓰기를 마친 뒤에 끝난다 —
/// 쓰기는 `store` 의 자물쇠를 5초까지 기다린 뒤 도니 그럴 일은 드물다.
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

/// 섬긴다 — [`super::SHUTDOWN`] 이 오기까지. 멈추기를 보면 제 소켓을 지우고(그 자리가 아직 제 소켓일 때만) 듣기를
/// 닫은 뒤, 돌던 요청을 기다리고 돌아온다.
pub fn serve(bound: Bound, methods: Methods) -> std::io::Result<()> {
    serve_for(bound, methods, DRAIN)
}

/// [`serve`] 의 몸 — 읽기를 기다리는 시한(`drain`)을 받는다. 시험이 짧은 시한으로 부르려는 것이고, 사람이 고르는
/// 손잡이는 아니다.
fn serve_for(bound: Bound, mut methods: Methods, drain: Duration) -> std::io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    // 답을 쓴 뒤에 받기를 깨운다(`connection`) — 여기서 깨우면 답이 나가기 전에 프로세스가 끝날 수 있다. 받는
    // 스레드가 `TICK` 에 먼저 깨어 문을 닫아도, 이 요청은 이미 들어와 세어졌으니 `Work::close` 가 그 답을 기다린다.
    methods.register(super::SHUTDOWN, move |_, _| {
        flag.store(true, Ordering::Release);
        rpc::reply(&json!({ "pid": std::process::id() }))
    });
    let methods = Arc::new(methods);
    let open = Arc::new(AtomicUsize::new(0));
    let lane = Arc::new(AtomicUsize::new(0));
    let work = Arc::new(Work::default());
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
        // 상한 안이면 보통 연결, 넘었으면 수명 길, 그것도 찼으면 거절이다.
        let seat = Leave::enter(&open, MAX_CONNECTIONS)
            .map(|leave| (leave, false))
            .or_else(|| Leave::enter(&lane, LIFECYCLE_SLOTS).map(|leave| (leave, true)));
        let (leave, full) = match seat {
            Some(seat) => seat,
            None => {
                // 받는 스레드는 거절을 읽어 가기를 기다리지 않는다.
                let _ = stream.set_nonblocking(true);
                let _ = writeln!(stream, "{}", refusal("too many connections to the daemon"));
                continue;
            }
        };
        let (methods, stop, path, work) =
            (Arc::clone(&methods), Arc::clone(&stop), Arc::clone(&path), Arc::clone(&work));
        // 띄우지 못하면 닫힌 클로저가 `leave` 를 놓아 수가 돌아온다.
        let _ = std::thread::Builder::new().name("moai-daemon-conn".into()).spawn(move || {
            let _leave = leave;
            match full {
                false => connection(stream, &methods, &stop, &path, &work),
                true => lifecycle(stream, &methods, &stop, &path, &work),
            }
        });
    }
    // 멈추는 차례 — 소켓 파일, 듣는 소켓, 돌던 요청(moai-3hgm.uz0).
    //
    // 1. **파일부터 지운다.** 그 뒤로 붙는 쪽은 "없다"(NotFound)를 받아 `moai daemon status` 가 곧바로 "떠 있지 않다"
    //    로 답하고, `start` 는 이 프로세스가 쥔 자물쇠가 풀리기를 기다린다. 듣기를 열어 둔 채 기다리던 옛 판은 새
    //    연결이 대기열에 서서 `hello` 의 답을 못 받아, `status` 가 2초 뒤 "데몬이 아니다" 로 넘어졌다. 듣기를 먼저 닫고
    //    파일을 나중에 지우면 그 사이에 붙는 쪽이 ECONNREFUSED 를 받아 "죽은 데몬이 남긴 소켓" 으로 읽는다.
    // 2. **듣는 소켓을 닫는다.** 대기열에 섰던 연결 — 멈추는 답과 받는 스레드가 그것을 본 사이의 짧은 틈에 붙은 것 —
    //    은 답 없이 끊긴다. 받아 들여도 돌릴 수 없다: 문(`Work`)이 곧 닫힌다.
    // 3. **돌던 요청을 기다린다**(`Work::close`). 쓰기는 끝까지, 읽기는 `drain` 까지. 말없이 쉬는 연결은 세지 않으니
    //    기다리지 않는다.
    let Bound { listener, path: socket, ino } = bound;
    let removed = remove_if_same(&socket, ino);
    drop(listener);
    work.close(Instant::now() + drain);
    removed
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

/// 상한에 찬 데몬의 거절 — `data.code` 는 `busy`, `data.pid` 는 이 데몬이다. `id` 는 null 이다: 받는 스레드는 줄을
/// 읽기 전이고, 줄을 읽은 수명 길도 같은 꼴로 답한다 — 그 연결로는 요청 하나뿐이라 짝을 맞출 다른 답이 없다.
fn refusal(why: &str) -> String {
    rpc::refusal(&turned_away(crate::fail::code::BUSY, why))
}

/// 데몬이 돌리지 않은 요청의 오류 — `data` 에 `code` 와 이 데몬의 `pid` 를 싣는다.
fn turned_away(code: &str, why: &str) -> RpcError {
    let mut error = RpcError::moai(code, why);
    error.data = Some(json!({ "code": code, "pid": std::process::id() }));
    error
}

/// 센 자리 하나 — 떨어지면 수를 돌려놓는다.
struct Leave(Arc<AtomicUsize>);

impl Leave {
    /// `count` 가 `cap` 밑이면 하나를 세고 자리를 준다. 찼으면 세지 않고 `None`.
    fn enter(count: &Arc<AtomicUsize>, cap: usize) -> Option<Leave> {
        if count.fetch_add(1, Ordering::AcqRel) >= cap {
            count.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Leave(Arc::clone(count)))
    }
}
impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 돌고 있는 요청의 수(읽기·쓰기를 따로)와 "문을 닫았다" 는 표시 — 셋이 **한 자물쇠 아래**에 있다(moai-3hgm.uz0).
///
/// 연결 스레드는 줄을 읽은 **뒤에** [`Work::admit`] 으로 요청을 들인다. 읽기 전에 세면 말없이 쉬는 연결까지 세어 멈추는
/// 데몬이 그 연결을 기다린다. 읽은 뒤에 세면 "읽었는데 아직 안 셌다" 는 틈이 생기고, 옛 판은 그 틈에서 받는 스레드가
/// 0 을 보고 끝나 막 읽은 요청이 도중에 끊겼다. 이 판은 틈이 있어도 잃지 않는다 — 차례의 논증은 이렇다.
///
/// - 들이기(`admit`)와 문 닫기(`close`)가 같은 자물쇠를 쥐니 둘의 차례는 하나로 정해진다. 들이기가 먼저면 그 요청은
///   세어졌고 `close` 는 그 수가 줄기를 기다린다 — 쓰기는 끝까지, 읽기는 시한까지. 닫기가 먼저면 들이기가 닫힌 문을
///   보고 처리 함수를 부르지 않는다. 셋째 차례는 없다: "돌리기 시작했는데 아무도 안 기다린다" 가 생길 자리가 없다.
/// - 줄을 언제 읽었는지는 가르지 않는다. 가르는 것은 들이기가 닫기의 앞이냐 뒤냐뿐이라, 읽기와 들이기 사이의 틈이
///   얼마든 같은 말이 선다. 원자 수 둘로 "센 뒤 `stop` 을 다시 보기" 를 짜면 두 쪽의 읽기·쓰기가 모두 `SeqCst` 여야
///   같은 말이 서고, 그것이 깨져도 시험이 못 잡는다 — 자물쇠 하나가 그 논증을 대신 진다.
/// - 자리([`Running`])는 답을 쓴 **뒤에** 놓는다. 그래서 `close` 가 돌아왔으면 들인 쓰기의 답은 이미 소켓에 나갔다.
/// - 문은 받는 스레드가 멈추기를 본 **처음에** 닫는다 — 기다림의 끝이 아니라. 기다리는 동안 쉬던 연결로 새 요청이
///   와도 들이지 않고 `stopping` 으로 거절하니, 기다림은 멈출 때 돌던 것만 기다리고 시한 없는 쓰기가 줄줄이 들어와
///   멈추기를 늘이지 못한다. 멈추는 답이 나간 뒤 받는 스레드가 그것을 보기까지의 틈(깨우는 붙기로 곧바로, 늦어도
///   [`TICK`])에 들어온 요청은 들어왔으니 기다린다.
/// - 들이지 못한 스레드가 거절을 쓰는 것과 프로세스가 끝나는 것은 겨룬다 — 기다림이 끝나는 바로 그때 읽힌 줄은
///   거절도 못 받고 연결이 끊길 수 있다. 그래도 돌리지는 않았다: 쓰기에서 "답이 없으면 안 들었다" 가 선다.
#[derive(Default)]
struct Work {
    count: Mutex<Count>,
    changed: Condvar,
}

#[derive(Default)]
struct Count {
    reads: usize,
    writes: usize,
    closed: bool,
}

impl Work {
    fn lock(&self) -> MutexGuard<'_, Count> {
        // 자물쇠 안에서는 수를 더하고 빼기만 한다 — 터질 일이 없지만, 터졌어도 수는 맞으니 그대로 쓴다.
        self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 요청 하나를 들인다 — 쓰기면 쓰기로, 아니면 읽기로 센다. 문이 닫혔으면 세지 않고 `None`: 그 요청은 돌리지 않는다.
    fn admit(&self, write: bool) -> Option<Running<'_>> {
        let mut count = self.lock();
        if count.closed {
            return None;
        }
        match write {
            true => count.writes += 1,
            false => count.reads += 1,
        }
        Some(Running { work: self, write })
    }

    /// 문을 닫고, 들인 쓰기가 모두 끝나기를 — 읽기는 `reads_until` 까지만 — 기다린다(사람의 결정 B, moai-3hgm).
    /// 쓰기에 시한이 없는 까닭: 끊긴 쓰기는 부른 쪽이 그것이 들었는지 모른다. 읽기에 시한이 있는 까닭: 시한 없는
    /// 요청(moai-8z7o 의 1)이 멈추기를 영영 막는다 — 읽기는 끊겨도 잃는 것이 없다. 돌아온 뒤에도 시한에 걸린 읽기는
    /// 돌고 있을 수 있다. 프로세스가 끝나며 그것을 끊는다.
    fn close(&self, reads_until: Instant) {
        let mut count = self.lock();
        count.closed = true;
        loop {
            let left = reads_until.saturating_duration_since(Instant::now());
            if count.writes == 0 && (count.reads == 0 || left.is_zero()) {
                return;
            }
            count = match left.is_zero() {
                // 읽기의 시한은 지났다 — 남은 쓰기가 끝나며 깨운다.
                true => self.changed.wait(count).unwrap_or_else(PoisonError::into_inner),
                false => self.changed.wait_timeout(count, left).unwrap_or_else(PoisonError::into_inner).0,
            };
        }
    }
}

/// 들인 요청 하나의 자리 — **답을 쓴 뒤에** 놓는다. 놓으면 수를 돌려놓고 기다리는 `close` 를 깨운다. 처리 함수가
/// 터져도(`rpc` 가 잡는다) 이 자리는 처리 함수 밖에 있어 그대로 놓인다.
struct Running<'a> {
    work: &'a Work,
    write: bool,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        let mut count = self.work.lock();
        match self.write {
            true => count.writes -= 1,
            false => count.reads -= 1,
        }
        drop(count);
        self.work.changed.notify_all();
    }
}

/// 읽은 줄 하나를 돌린 결과.
enum Turn<'w> {
    /// 답할 줄(알림이면 없다)과, 답을 쓴 뒤에 놓을 자리.
    Ran(Option<String>, Option<Running<'w>>),
    /// 문이 닫혀 돌리지 않았다 — 이 거절 한 줄을 쓰고 연결을 닫는다.
    Refused(String),
}

/// 줄 하나 — 파싱하고, 들이고, 돌린다. 처리 함수가 없는 답(틀린 요청·없는 메서드)은 들이지 않는다: 돌리는 것이
/// 없어 멈추는 데몬이 기다릴 것도, 잃을 것도 없다.
fn turn<'w>(methods: &Methods, work: &'w Work, line: &[u8]) -> Turn<'w> {
    match methods.prepare(line) {
        Prepared::Answered(answer) => Turn::Ran(answer, None),
        Prepared::Call(call) => {
            match work.admit(call.writes()) {
                Some(running) => Turn::Ran(Some(call.run()), Some(running)),
                None => Turn::Refused(call.refuse(&turned_away(
                    crate::fail::code::STOPPING,
                    "the daemon is stopping; this request was not run",
                ))),
            }
        }
    }
}

/// 상한을 넘은 연결 하나 — 줄 하나만 읽어 [`LIFECYCLE`] 의 메서드면 돌리고, 아니면 `busy` 로 거절하고 닫는다.
/// 어느 쪽이든 그 연결의 다음 줄은 안 읽는다.
fn lifecycle(stream: UnixStream, methods: &Methods, stop: &AtomicBool, path: &Path, work: &Work) {
    if stream.set_read_timeout(Some(LIFECYCLE_WAIT)).is_err() || stream.set_write_timeout(Some(LIFECYCLE_WAIT)).is_err()
    {
        return;
    }
    let Ok(read_half) = stream.try_clone() else { return };
    // 시한은 줄 하나 전체에 건다 — 소켓의 읽기 시한은 `read` 한 번마다라, 몇 초마다 한 바이트씩 보내는 연결이
    // 자리를 끝없이 쥐면 넷이 수명 길을 막아 `status`·`stop` 이 다시 `busy` 에 갇힌다.
    let mut reader = BufReader::new(Deadline { stream: read_half, until: Instant::now() + LIFECYCLE_WAIT });
    let mut writer = stream;
    let (reply, running) = match rpc::read_line(&mut reader) {
        Ok(rpc::Line::Eof) | Err(_) => return,
        Ok(rpc::Line::Text(line)) if rpc::method_of(&line).is_some_and(|m| LIFECYCLE.contains(&m.as_str())) => {
            match turn(methods, work, &line) {
                Turn::Ran(reply, running) => (reply, running),
                // 요청 하나뿐인 길이라 거절도 같은 자리로 쓴다.
                Turn::Refused(refused) => (Some(refused), None),
            }
        }
        Ok(_) => (Some(refusal("too many connections to the daemon; only hello and daemon.shutdown are served")), None),
    };
    if let Some(reply) = reply {
        let _ = writeln!(writer, "{reply}").and_then(|()| writer.flush());
    }
    drop(running);
    if stop.load(Ordering::Acquire) {
        wake(path);
    }
}

/// 시한이 줄 하나 전체에 걸린 읽기 — `read` 마다 남은 시간을 소켓의 읽기 시한으로 다시 건다. 시한이 지나면 `TimedOut`.
struct Deadline {
    stream: UnixStream,
    until: Instant,
}

impl std::io::Read for Deadline {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = self.until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}

/// 받는 스레드를 곧바로 깨운다 — 제 소켓에 한 번 붙는다. 붙기가 실패해도(소켓 파일이 지워졌다) 받는 스레드는
/// [`TICK`] 안에 스스로 `stop` 을 본다.
fn wake(path: &Path) {
    let _ = UnixStream::connect(path);
}

/// 연결 하나 — 줄을 읽고, 돌리고, 답을 쓰고, 다음 줄을 읽는다. 앞 요청의 답을 쓰기 전에는 다음 줄을 안 읽는다.
fn connection(stream: UnixStream, methods: &Methods, stop: &AtomicBool, path: &Path, work: &Work) {
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
            Ok(rpc::Line::Text(line)) => match turn(methods, work, &line) {
                Turn::Ran(reply, running) => (reply, running),
                Turn::Refused(refused) => {
                    // **거절은 줄째 나가거나 아예 안 나간다**(리뷰 moai-3hgm.puw 7번). 막지 않는 쓰기로 내던 판은 앞의
                    // 답을 안 읽어 버퍼가 찬 쪽에 줄의 앞머리만 쓰고(`writeln!` 은 몸과 줄바꿈을 따로 쓴다) 닫아, 그쪽이
                    // 줄바꿈 없는 잘린 JSON 을 받았다. 그래서 다른 답과 같이 막는 쓰기로, 연결 첫머리에 건 [`WRITE`]
                    // 시한 안에서 쓴다. 줄 하나를 한 번에 쓴다 — 짧은 줄 하나가 소켓에 통째로 들거나 안 든다.
                    //
                    // 이 기다림은 멈추기를 늘이지 않는다: 들이지 못한 요청이라 자리([`Running`])가 없고, `Work::close`
                    // 는 이 스레드를 안 기다린다. 데몬은 기다림이 끝나면 이 쓰기가 섰든 아니든 끝나고, 그때 못 쓴 쪽은
                    // 거절 없이 깨끗한 EOF 를 받는다. 수명 길([`lifecycle`])도 같은 꼴로 막는 쓰기를 쓴다. 못 썼어도
                    // 돌리지 않은 것은 같다.
                    let _ = writer.write_all(format!("{refused}\n").as_bytes()).and_then(|()| writer.flush());
                    return;
                }
            },
        };
        if let Some(reply) = reply
            && writeln!(writer, "{reply}").and_then(|()| writer.flush()).is_err()
        {
            return;
        }
        drop(running);
        if stop.load(Ordering::Acquire) {
            wake(path);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 수명 길의 시한은 줄 하나 전체에 걸린다 — 시한보다 짧은 간격으로 한 바이트씩 흘리는 연결도 시한 안에 놓인다.
    /// 읽기 하나마다의 시한이던 판은 그런 연결 넷이 수명 길을 끝없이 쥐어 `status`·`stop` 을 `busy` 에 가뒀다.
    #[test]
    fn a_trickling_connection_leaves_the_lifecycle_lane_on_time() {
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let drip = std::thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < LIFECYCLE_WAIT * 3 {
                if theirs.write_all(b" ").is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let (stop, work) = (AtomicBool::new(false), Work::default());
        let started = Instant::now();
        lifecycle(ours, &Methods::new(), &stop, Path::new("/nonexistent/moai-test.sock"), &work);
        assert!(started.elapsed() < LIFECYCLE_WAIT * 2, "흘리는 연결이 수명 길을 {:?} 쥐었다", started.elapsed());
        drip.join().unwrap();
    }

    // ---- 멈출 때 돌던 요청(moai-3hgm.uz0) ------------------------------------------------------------------------
    //
    // 데몬을 이 프로세스 안에서 띄운다 — 시험용 메서드(잠자는 읽기·쓰기)를 달고 읽기의 시한을 짧게 준다(`serve_for`).
    // 여기서 "프로세스가 끝난다" 는 `serve_for` 가 돌아오는 것이다: 그 뒤의 일은 실제 데몬에서는 일어나지 않는다.

    use super::super::client::{self, Probe};
    use serde_json::Value;
    use std::io::BufRead as _;
    use std::sync::atomic::AtomicUsize as Nth;

    /// 시험용 메서드가 자는 시간과, 그보다 짧은 읽기의 시한.
    const SLOW: Duration = Duration::from_millis(2000);
    const SHORT_DRAIN: Duration = Duration::from_millis(300);

    /// 이 프로세스 안의 데몬 하나 — 제 자리(소켓 경로는 108 바이트 밑이라 `/tmp` 아래 짧게)와 처리 함수가 남긴 것.
    struct Daemon {
        dir: PathBuf,
        /// 끝까지 돈 처리 함수의 이름.
        finished: Arc<Mutex<Vec<&'static str>>>,
        server: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    }

    impl Daemon {
        fn start() -> Daemon {
            static NTH: Nth = Nth::new(0);
            let dir =
                PathBuf::from(format!("/tmp/moai-srv-{}-{}", std::process::id(), NTH.fetch_add(1, Ordering::Relaxed)));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir(&dir).unwrap();
            let finished = Arc::new(Mutex::new(Vec::new()));
            let mut methods = Methods::new();
            let mark = |name: &'static str| {
                let finished = Arc::clone(&finished);
                move || finished.lock().unwrap().push(name)
            };
            let (read, write, touch) = (mark("slow_read"), mark("slow_write"), mark("touch"));
            methods.register("slow_read", move |_, _| {
                std::thread::sleep(SLOW);
                read();
                rpc::reply(&json!("read"))
            });
            methods.register_write("slow_write", move |_, _| {
                std::thread::sleep(SLOW);
                write();
                rpc::reply(&json!("written"))
            });
            methods.register_write("touch", move |_, _| {
                touch();
                rpc::reply(&json!("touched"))
            });
            let bound = bind(&dir.join("s")).unwrap();
            let server = std::thread::spawn(move || serve_for(bound, methods, SHORT_DRAIN));
            Daemon { dir, finished, server: Some(server) }
        }

        fn socket(&self) -> PathBuf {
            self.dir.join("s")
        }

        fn finished(&self, name: &str) -> bool {
            self.finished.lock().unwrap().contains(&name)
        }

        fn stop(&self) {
            let bye = client::call(&self.socket(), super::super::SHUTDOWN, json!({})).unwrap().unwrap();
            assert_eq!(bye["pid"], std::process::id());
        }

        /// 멈추는 데몬이 소켓 파일을 지우기를 기다린다 — 지운 뒤에 문을 닫는다(`serve_for`).
        fn wait_until_unlinked(&self) {
            let started = Instant::now();
            while self.socket().exists() {
                assert!(started.elapsed() < Duration::from_secs(3), "멈추는 데몬이 소켓 파일을 안 지웠다");
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        /// `serve_for` 가 돌아오기를 기다린다 — 실제 데몬에서는 여기서 프로세스가 끝난다.
        fn join(&mut self) {
            self.server.take().unwrap().join().unwrap().unwrap();
        }

        fn joined(&self) -> bool {
            self.server.as_ref().is_none_or(|server| server.is_finished())
        }
    }

    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// 연결 하나 — 요청을 보내고 답 한 줄을 받는다.
    struct Wire {
        reader: std::io::BufReader<UnixStream>,
        writer: UnixStream,
    }

    impl Wire {
        fn open(path: &Path) -> Wire {
            let stream = UnixStream::connect(path).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            Wire { reader: std::io::BufReader::new(stream.try_clone().unwrap()), writer: stream }
        }

        fn send(&mut self, id: u64, method: &str) {
            let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": {} });
            writeln!(self.writer, "{line}").unwrap();
        }

        /// 답 한 줄 — 연결이 닫혔으면 `None`.
        fn answer(&mut self) -> Option<Value> {
            let mut line = String::new();
            match self.reader.read_line(&mut line).unwrap() {
                0 => None,
                _ => Some(serde_json::from_str(&line).unwrap()),
            }
        }

        fn call(&mut self, id: u64, method: &str) -> Value {
            self.send(id, method);
            self.answer().expect("답 없이 닫혔다")
        }
    }

    /// 멈추는 데몬이 돌던 쓰기를 기다리는 동안 소켓 파일은 이미 없다 — 새로 붙는 쪽은 "없다" 를 받아 `moai daemon
    /// status` 가 "떠 있지 않다" 로 답한다(`Probe::Absent`). 옛 판은 듣기를 열어 두어 새 연결이 대기열에 서서 답을 못
    /// 받고, `status` 가 2초 뒤 "데몬이 아니다" 로 넘어졌다.
    #[test]
    fn a_draining_daemon_has_already_removed_its_socket() {
        let mut daemon = Daemon::start();
        let mut writing = Wire::open(&daemon.socket());
        writing.send(1, "slow_write");
        std::thread::sleep(Duration::from_millis(200));
        daemon.stop();
        daemon.wait_until_unlinked();
        assert!(matches!(client::probe(&daemon.socket()), Probe::Absent), "기다리는 데몬에 아직 붙는다");
        let refused = UnixStream::connect(daemon.socket()).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::NotFound, "{refused}");
        assert!(!daemon.joined(), "돌던 쓰기를 안 기다리고 끝났다");
        assert_eq!(writing.answer().expect("쓰기의 답")["result"], "written");
        daemon.join();
    }

    /// 쓰기는 끝까지, 읽기는 시한까지(사람의 결정 B). 같은 길이의 둘을 따로 띄운 데몬에서 돌리다 멈춘다 — 쓰기의
    /// 데몬은 그 쓰기가 끝나고 **답이 나간 뒤에** 돌아오고, 읽기의 데몬은 시한에 돌아온다(그 읽기는 아직 돈다).
    #[test]
    fn a_write_outlives_the_drain_and_a_read_of_the_same_length_does_not() {
        let (mut writer, mut reader) = (Daemon::start(), Daemon::start());
        let mut writing = Wire::open(&writer.socket());
        let mut reading = Wire::open(&reader.socket());
        writing.send(1, "slow_write");
        reading.send(1, "slow_read");
        std::thread::sleep(Duration::from_millis(200));
        let asked = Instant::now();
        writer.stop();
        reader.stop();

        reader.join();
        assert!(!reader.finished("slow_read"), "읽기의 데몬이 그 읽기를 끝까지 기다렸다");
        assert!(asked.elapsed() < SLOW, "읽기의 데몬이 {:?} 뒤에야 돌아왔다", asked.elapsed());

        writer.join();
        assert!(writer.finished("slow_write"), "쓰기가 끝나기 전에 돌아왔다");
        // 돌아왔을 때 답은 이미 소켓에 있다 — 실제 데몬이라면 여기서 끝나도 받는다.
        writing.writer.shutdown(std::net::Shutdown::Write).unwrap();
        assert_eq!(writing.answer().expect("쓰기의 답")["result"], "written");
    }

    /// 문을 닫은 뒤에 읽힌 요청은 돌리지 않고 `stopping` 으로 거절한다 — 쉬던 연결로 기다리는 동안 온 쓰기다.
    /// 거절에는 그 요청의 `id` 와 데몬의 `pid` 가 실리고, 연결은 닫힌다.
    #[test]
    fn a_request_read_after_the_doors_close_is_refused_and_not_run() {
        let mut daemon = Daemon::start();
        let mut idle = Wire::open(&daemon.socket());
        assert_eq!(idle.call(1, "ping")["result"], json!({}));
        let mut writing = Wire::open(&daemon.socket());
        writing.send(1, "slow_write");
        std::thread::sleep(Duration::from_millis(200));
        daemon.stop();
        daemon.wait_until_unlinked();
        // 파일을 지운 다음 줄이 문을 닫는다 — 그 한 줄을 넉넉히 기다린다.
        std::thread::sleep(Duration::from_millis(100));
        let refused = idle.call(7, "touch");
        assert_eq!(refused["id"], 7, "{refused}");
        assert_eq!(refused["error"]["data"]["code"], crate::fail::code::STOPPING, "{refused}");
        assert_eq!(refused["error"]["data"]["pid"], std::process::id(), "{refused}");
        assert!(idle.answer().is_none(), "거절한 연결이 열려 있다");
        assert!(!daemon.finished("touch"), "거절한 요청을 돌렸다");
        assert_eq!(writing.answer().expect("쓰기의 답")["result"], "written");
        daemon.join();
        assert!(!daemon.finished("touch"));
    }

    /// **멈추는 데몬의 거절은 버퍼가 찬 연결에도 줄째 간다**(리뷰 moai-3hgm.puw 7번) — 앞의 답을 안 읽어 데몬에서 그쪽으로
    /// 가는 버퍼가 찬 연결이다. 그쪽이 읽어 자리가 나면 거절이 나가고, 앞의 답 뒤에 줄바꿈까지 든 한 줄로 선다. 막지 않는
    /// 쓰기로 내던 판은 거기서 못 쓰거나 앞머리만 쓰고 닫아, 그쪽은 쌓인 답 뒤에 거절 없는 EOF 나 잘린 줄을 받았다.
    #[test]
    fn a_stopping_refusal_to_a_full_socket_waits_for_room_and_goes_out_whole() {
        use std::io::Read as _;
        let (ours, theirs) = UnixStream::pair().unwrap();
        // 데몬 쪽에서 그쪽으로 가는 버퍼를 채운다 — 그쪽이 안 읽은 앞의 답들이다.
        let filler = ours.try_clone().unwrap();
        filler.set_nonblocking(true).unwrap();
        let mut queued = 0;
        loop {
            match (&filler).write(b"queued\n") {
                Ok(n) => queued += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("버퍼를 못 채웠다: {e}"),
            }
        }
        // O_NONBLOCK 은 복제한 fd 와 나눈다 — 연결은 막는 소켓을 받는다. 복제는 닫는다: 남으면 그쪽이 EOF 를 못 본다.
        filler.set_nonblocking(false).unwrap();
        drop(filler);
        let work = Arc::new(Work::default());
        work.close(Instant::now());
        let mut methods = Methods::new();
        methods.register_write("touch", |_, _| rpc::reply(&json!("touched")));
        let served = {
            let work = Arc::clone(&work);
            std::thread::spawn(move || {
                let nowhere = Path::new("/nonexistent/moai-test.sock");
                connection(ours, &methods, &AtomicBool::new(false), nowhere, &work);
            })
        };
        let line = json!({ "jsonrpc": "2.0", "id": 7, "method": "touch", "params": {} });
        writeln!(&theirs, "{line}").unwrap();
        // 거절이 찬 버퍼에 닿을 틈을 준다 — 옛 판은 여기서 못 쓰고 닫았다. 새 판은 이 틈과 상관없이 읽기를 기다린다.
        std::thread::sleep(Duration::from_millis(300));
        theirs.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut got = Vec::new();
        (&theirs).read_to_end(&mut got).unwrap();
        served.join().unwrap();
        let text = String::from_utf8(got).unwrap();
        let whole = text.strip_suffix('\n').expect("거절이 줄바꿈 없이 끝났다");
        let (earlier, last) = whole.rsplit_once('\n').expect("앞의 답 뒤에 줄이 없다");
        let refused: Value =
            serde_json::from_str(last).unwrap_or_else(|e| panic!("마지막 줄이 거절이 아니다: {e} — {last:?}"));
        assert_eq!(refused["id"], 7, "{refused}");
        assert_eq!(refused["error"]["data"]["code"], crate::fail::code::STOPPING, "{refused}");
        assert_eq!(earlier.len() + 1, queued, "앞의 답을 잃었다");
    }

    /// 막 읽은 요청과 멈추기가 겨룬다 — 쉬던 연결의 쓰기와 `daemon.shutdown` 을 함께 보낸다. 어느 쪽이 이기든 그 요청은
    /// 답을 받고, 돌았는가와 답이 맞는다: 돌았으면 그 답이 데몬이 돌아오기 전에 나갔고, 안 돌았으면 `stopping` 이다.
    /// 옛 판은 읽고 세기 전의 틈에서 데몬이 끝나 돌던 요청을 도중에 끊을 수 있었다.
    #[test]
    fn a_request_racing_the_stop_either_runs_and_answers_or_is_refused_unrun() {
        for round in 0..16 {
            let mut daemon = Daemon::start();
            let mut idle = Wire::open(&daemon.socket());
            assert_eq!(idle.call(1, "ping")["result"], json!({}));
            let socket = daemon.socket();
            let stopper = std::thread::spawn(move || client::call(&socket, super::super::SHUTDOWN, json!({})));
            // 판마다 조금씩 늦게 보낸다 — 이른 판은 문이 닫히기 전에, 늦은 판은 닫힌 뒤에 읽힌다.
            std::thread::sleep(Duration::from_micros(round * 50));
            idle.send(2, "touch");
            stopper.join().unwrap().unwrap().unwrap();
            daemon.join();
            let ran = daemon.finished("touch");
            // 데몬은 돌아왔다 — 돈 요청의 답은 이미 소켓에 있어야 한다. 거절은 그 뒤에 쓰일 수도 있으니 기다려 읽는다.
            let wait = if ran { Duration::from_millis(100) } else { Duration::from_secs(10) };
            idle.reader.get_ref().set_read_timeout(Some(wait)).unwrap();
            let answer = idle.answer().unwrap_or_else(|| panic!("{round}번째: 막 읽은 요청이 답 없이 닫혔다"));
            match ran {
                true => assert_eq!(answer["result"], "touched", "{round}번째: {answer}"),
                false => {
                    assert_eq!(answer["error"]["data"]["code"], crate::fail::code::STOPPING, "{round}번째: {answer}")
                }
            }
            // 데몬이 돌아온 뒤에 시작하는 요청은 없다 — 실제 데몬이라면 그것이 도중에 끊긴다.
            std::thread::sleep(Duration::from_millis(20));
            assert_eq!(daemon.finished("touch"), ran, "{round}번째: 데몬이 돌아온 뒤에 요청이 돌았다");
        }
    }

    /// 문의 차례 — 닫기 전에 들인 쓰기는 시한이 지나도 기다리고, 들인 읽기는 시한까지만 기다린다. 닫은 뒤에는 아무것도
    /// 들이지 않는다.
    #[test]
    fn the_door_waits_for_admitted_writes_and_admits_nothing_once_closed() {
        let work = Arc::new(Work::default());
        let write = work.admit(true).expect("열린 문");
        let read = work.admit(false).expect("열린 문");
        let closing = Arc::clone(&work);
        let closer = std::thread::spawn(move || closing.close(Instant::now() + Duration::from_millis(50)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(!closer.is_finished(), "들인 쓰기를 안 기다리고 닫았다");
        assert!(work.admit(false).is_none() && work.admit(true).is_none(), "닫는 중에 들였다");
        drop(write);
        closer.join().unwrap();
        // 시한이 지난 읽기는 아직 돈다 — 그 자리를 놓아도 수가 맞는다.
        drop(read);
        let count = work.lock();
        assert_eq!((count.reads, count.writes, count.closed), (0, 0, true));
    }
}
