//! 데몬 — 로컬 읽기 API (moai-5n6m).
//!
//! - `rpc` 는 JSON-RPC 2.0 **메시지 층**이다. 전송(Unix 소켓의 줄, 나중의 HTTP)을 모르고 `cmd`·`store` 의 어떤
//!   타입도 이름 대지 않는다.
//! - `server` 는 Unix 소켓의 줄 전송이다 — 연결마다 스레드 하나, 그 연결의 요청은 그 스레드에서 하나씩.
//! - `client` 는 붙는 쪽이다 — `moai daemon start·stop·status` 가 떠 있는 데몬을 찾고 멈추는 데 쓴다.
//!
//! 명령을 메서드에 묶는 자리는 [`methods`] 하나다. 소켓을 모르는 Windows 에서는 `rpc` 만 서고 아무도 안 불러
//! dead_code 가 선다 — 그쪽에서만 걷는다.

#[cfg_attr(not(unix), allow(dead_code))]
pub mod rpc;

#[cfg(unix)]
pub mod changes;
#[cfg(unix)]
pub mod client;
#[cfg(unix)]
pub mod project;
#[cfg(unix)]
mod read;
#[cfg(unix)]
pub mod server;

/// 데몬을 멈추는 메서드. 판이 달라도 이름은 그대로여야 한다 — 새 바이너리의 `moai daemon stop` 이 옛 판 데몬을
/// 멈추는 길이 이것뿐이다.
#[cfg(unix)]
pub const SHUTDOWN: &str = "daemon.shutdown";

/// 데몬이 섬기는 메서드 표. `hello`·`ping` 위에 읽기 메서드 여섯(`read::register`, moai-5n6m.q16)을 단다 — 다음
/// 메서드도 여기서 단다. 데몬의 수명에 딸린 [`SHUTDOWN`] 은 `server::serve` 가 제 손으로 단다.
#[cfg(unix)]
pub fn methods() -> rpc::Methods {
    let mut methods = rpc::Methods::new();
    read::register(&mut methods);
    methods
}

/// 사용자당 하나인 소켓의 자리 — `moai ctl` 과 같은 디렉터리(`ctl::place::dir`)의 `daemon.socket`.
///
/// **확장자가 `.sock` 이 아니다.** `moai ctl ls` 는 그 디렉터리의 `*.sock` 을 모두 창으로 여겨 ctl 의 `hello` 를
/// 보낸다 — 데몬이 거기 서면 `ctl ls` 마다 데몬에 붙어 답을 기다리고, 죽은 데몬의 소켓은 ctl 이 지운다.
/// 이름이 갈려야 두 표면이 서로를 모른다.
#[cfg(unix)]
pub fn socket_path() -> std::io::Result<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let path = crate::ctl::place::dir()?.join("daemon.socket");
    // `sockaddr_un` 의 상한 — `ctl::place::socket` 과 같은 값이다.
    let limit = if cfg!(target_os = "macos") { 104 } else { 108 };
    if path.as_os_str().as_bytes().len() >= limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("daemon socket path is too long: {}", path.display()),
        ));
    }
    Ok(path)
}
