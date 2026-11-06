//! 데몬과 `moai ctl` 이 쓰는 로컬 전송 — 유닉스는 Unix 소켓, Windows 는 named pipe(moai-0yo3).
//!
//! **이름은 달라도 쓰는 꼴은 하나다.** [`Listener`] 와 [`Stream`] 은 유닉스에서 표준 라이브러리의 그 타입 자체고,
//! Windows 의 `pipe` 는 같은 메서드를 낸다. 그래서 그 위의 서버·클라이언트에는 운영체제 갈래가 없다.
//!
//! **자리는 어디서나 파일 경로다.** 유닉스의 소켓 파일 자리에 Windows 는 pipe 이름을 담은 표시 파일을 둔다 — 있음·
//! 죽음의 판정(`NotFound`·`ConnectionRefused`)과 `moai ctl` 의 `*.sock` 나열이 그래서 같은 코드로 선다.

#[cfg(windows)]
#[path = "transport/pipe.rs"]
mod pipe;

#[cfg(windows)]
pub use pipe::{Listener, Stream};
#[cfg(unix)]
pub use std::os::unix::net::{UnixListener as Listener, UnixStream as Stream};

use std::path::Path;
use std::time::Duration;

/// 받을 연결이 섰는가 — `wait` 까지만 기다린다. 신호에 끊긴 기다림은 "아직 없다" 로 읽는다.
///
/// 섰다고 답한 뒤의 `accept` 는 막히지 않는다: 그 연결을 가져갈 다른 받는 쪽이 없고, 붙은 쪽이 그새 떠나도
/// Unix 소켓은 그 연결을 대기열에 둔 채 넘긴다(받은 쪽이 곧 EOF 를 읽는다).
#[cfg(unix)]
pub fn pending(listener: &Listener, wait: Duration) -> std::io::Result<bool> {
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

#[cfg(windows)]
pub fn pending(listener: &Listener, wait: Duration) -> std::io::Result<bool> {
    listener.pending(wait)
}

/// 건너편이 끊었는가.
///
/// **A hang-up, not end-of-input.** A caller of the line protocol (docs/ctl.md) may shut down its writing half once
/// its request is sent (`nc -N -U`, `shutdown(SHUT_WR)`) and still read the reply; a peeked EOF told it `gone` at
/// once, and a byte it left unread hid a real close. POLLHUP comes only when both halves are shut. POLLIN is asked
/// for because macOS reports the hang-up only through a read filter. A zero timeout leaves blocking mode alone.
#[cfg(unix)]
pub fn hung_up(stream: &Stream) -> bool {
    use std::os::fd::AsRawFd;
    let mut fd = libc::pollfd { fd: stream.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    let ready = unsafe { libc::poll(&mut fd, 1, 0) };
    ready > 0 && fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}

#[cfg(windows)]
pub fn hung_up(stream: &Stream) -> bool {
    stream.hung_up()
}

/// 시한을 건 결과. **macOS 는 건너편이 이미 닫은 소켓의 시한을 `EINVAL` 로 거절한다**(moai-3e4m, moai-t0ys) — 꽉 찬 데몬이 거절 한 줄을 쓰고
/// 닫은 뒤에 클라이언트가 여기 오면 그 거절을 읽기도 전에 넘어져, `status` 가 `busy` 대신 "데몬이 아니다" 로 답했다. 데몬 쪽은
/// 쓰고 곧바로 닫은 요청을 안 읽고 버렸다. 닫힌 소켓의 읽기와 쓰기는 안 막히니 시한이 없어도 된다.
///
/// **macOS 밖에서는 그대로 낸다.** 거기서 `InvalidInput` 은 0 시한 같은 부른 쪽의 잘못이고, 삼키면 연결이 말없이 시한 없이 돈다.
pub fn armed(set: std::io::Result<()>) -> std::io::Result<()> {
    match set {
        Err(e) if cfg!(target_os = "macos") && e.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
        set => set,
    }
}

/// 쓰기가 끊겼는가 — 건너편이 먼저 닫았다는 말이고, 닫기 전에 쓴 답은 받는 버퍼에 남아 있다. 부른 쪽은 답을 읽어 본 뒤에 판단한다.
/// macOS 는 그 닫기와 겹친 쓰기를 `EPIPE` 말고 `ENOTCONN` 으로도 끊는다(moai-t0ys).
pub fn broken(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionReset, NotConnected};
    matches!(e.kind(), BrokenPipe | ConnectionReset | NotConnected)
}

/// 그 자리의 것이 이 사용자의 소켓인가 — 종류와 주인. 남의 파일은 지우지도 붙지도 않는다.
#[cfg(unix)]
pub fn is_own(_path: &Path, meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // SAFETY: geteuid 는 실패하지 않는다.
    meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() }
}

/// 표시 파일이 이 사용자의 pipe 이름을 담았을 때만 제 것이다 — 이름에 이 사용자의 SID 가 들어 주인도 함께 본다.
#[cfg(windows)]
pub fn is_own(path: &Path, meta: &std::fs::Metadata) -> bool {
    meta.is_file() && pipe::marked_name(path).is_ok_and(|name| name.is_some())
}

/// 그 자리의 파일이 아직 그 파일인가를 가르는 값 — 보고 지우는 사이에 남이 세운 것을 지우지 않으려고 본다.
#[cfg(unix)]
pub fn identity(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino()
}

/// 안정된 Rust 에 파일 번호가 없어 마지막 쓴 시각을 쓴다 — 표시 파일은 세울 때 한 번만 쓴다. **만든 시각은 못 쓴다**:
/// NTFS 의 터널링이 지운 이름을 15초 안에 다시 세우면 옛 만든 시각을 물려준다(실기에서 쟀다).
#[cfg(windows)]
pub fn identity(meta: &std::fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    meta.last_write_time()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 시험마다 제 자리 — 떨어질 때 지운다.
    struct Place(PathBuf);

    impl Place {
        fn new(name: &str) -> Place {
            let dir = std::env::temp_dir().join(format!("moai-transport-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Place(dir)
        }
    }

    impl Drop for Place {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 만든 시각은 터널링이 물려줘 이 값이 같았다 — 그러면 보고 지우는 사이에 남이 세운 표시 파일을 지운다.
    #[test]
    fn a_marker_put_back_under_the_same_name_is_another_marker() {
        let place = Place::new("identity");
        let path = place.0.join("7.sock");
        std::fs::write(&path, "first").unwrap();
        let before = identity(&std::fs::symlink_metadata(&path).unwrap());
        std::thread::sleep(Duration::from_millis(20));
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "second").unwrap();
        assert_ne!(identity(&std::fs::symlink_metadata(&path).unwrap()), before);
    }

    /// 받는 쪽이 늦은 것은 죽은 것이 아니다 — 죽었다고 읽으면 `moai ctl ls` 가 산 창의 표시 파일을 지운다.
    #[test]
    fn a_listener_with_no_free_instance_reads_as_busy_not_dead() {
        let place = Place::new("busy");
        let path = place.0.join("daemon.socket");
        let _listener = Listener::bind(&path).unwrap();
        let _first = Stream::connect(&path).unwrap();
        let second = Stream::connect(&path).unwrap_err();
        assert_eq!(second.kind(), std::io::ErrorKind::ResourceBusy, "{second}");
    }

    /// 듣기가 사라지면 그때가 "아무도 안 받는다" 다.
    #[test]
    fn a_marker_with_no_listener_reads_as_refused() {
        let place = Place::new("dead");
        let path = place.0.join("daemon.socket");
        drop(Listener::bind(&path).unwrap());
        assert_eq!(Stream::connect(&path).unwrap_err().kind(), std::io::ErrorKind::ConnectionRefused);
    }

    /// 같은 파일을 다른 철자로 불러도 같은 pipe 다 — 이름을 자리 글에서 다시 지으면 `C:/x` 와 `C:\x` 가 갈린다.
    #[test]
    fn the_marker_names_the_pipe_however_the_path_is_spelled() {
        let place = Place::new("spelling");
        let path = place.0.join("daemon.socket");
        let _listener = Listener::bind(&path).unwrap();
        let other = PathBuf::from(path.display().to_string().replace('\\', "/"));
        assert!(is_own(&other, &std::fs::symlink_metadata(&other).unwrap()));
        Stream::connect(&other).unwrap();
    }

    /// 이 사용자의 pipe 를 대지 않은 파일은 남의 것이다.
    #[test]
    fn a_file_that_names_no_pipe_of_ours_is_not_ours() {
        let place = Place::new("foreign");
        for (name, bytes) in [("text", &b"not a socket"[..]), ("binary", &[0xff, 0xfe, 0x00][..]), ("empty", &[][..])] {
            let path = place.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            assert!(!is_own(&path, &std::fs::symlink_metadata(&path).unwrap()), "{name}");
            assert_eq!(Stream::connect(&path).unwrap_err().kind(), std::io::ErrorKind::ConnectionRefused, "{name}");
        }
    }

    /// 안 읽은 바이트가 끊김을 가리지 않는다 — 가리면 `--wait` 의 부름이 떠난 뒤에도 자리를 쥔 채 시한까지 간다.
    #[test]
    fn a_hang_up_is_seen_past_an_unread_byte() {
        use std::io::Write as _;
        let (ours, mut theirs) = Stream::pair().unwrap();
        assert!(!hung_up(&ours));
        theirs.write_all(b"unread").unwrap();
        assert!(!hung_up(&ours), "붙어 있는 쪽을 끊겼다고 읽었다");
        drop(theirs);
        assert!(hung_up(&ours));
    }
}
