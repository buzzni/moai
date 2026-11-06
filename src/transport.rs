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
pub use pipe::{Listener, Stream, pipe_name};
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

/// 그 자리의 것이 이 사용자의 소켓인가 — 종류와 주인. 남의 파일은 지우지도 붙지도 않는다.
#[cfg(unix)]
pub fn is_own(_path: &Path, meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // SAFETY: geteuid 는 실패하지 않는다.
    meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() }
}

/// 표시 파일이 그 자리의 pipe 이름을 담았을 때만 제 것이다 — 이름에 이 사용자의 SID 가 들어 주인도 함께 본다.
#[cfg(windows)]
pub fn is_own(path: &Path, meta: &std::fs::Metadata) -> bool {
    meta.is_file() && pipe_name(path).is_ok_and(|name| std::fs::read_to_string(path).is_ok_and(|text| text == name))
}

/// 그 자리의 파일이 아직 그 파일인가를 가르는 값 — 보고 지우는 사이에 남이 세운 것을 지우지 않으려고 본다.
#[cfg(unix)]
pub fn identity(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino()
}

/// 안정된 Rust 에 파일 번호가 없어 만든 시각을 쓴다 — 100ns 단위라 지우고 다시 세운 표시 파일과 안 겹친다.
#[cfg(windows)]
pub fn identity(meta: &std::fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    meta.creation_time()
}
