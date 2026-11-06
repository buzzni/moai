//! Windows named pipe 를 `UnixListener`·`UnixStream` 의 꼴로 쓴다(moai-0yo3.356).
//!
//! - **겹친 입출력(overlapped)이다.** 막는 pipe 핸들에는 읽기 시한이 없고, 한 핸들의 읽기가 쓰기를 막는다. 데몬은
//!   한 연결을 읽는 쪽과 쓰는 쪽으로 쥐고 양쪽에 시한을 건다.
//! - **남에게 안 열린다.** 기본 보안 설명자는 Everyone 에게 읽기를 준다 — 그래서 이 사용자 하나만 든 DACL 을 직접
//!   건다. 주인도 이 사용자로 박는다: 관리자 권한으로 뜨면 기본 주인이 Administrators 라, 붙는 쪽의 주인 검사가
//!   제 데몬을 거절한다. 원격은 `PIPE_REJECT_REMOTE_CLIENTS` 가 막는다.
//! - **이름을 먼저 차지한 남을 믿지 않는다.** 첫 인스턴스는 `FILE_FLAG_FIRST_PIPE_INSTANCE` 로 세우고, 붙는 쪽은
//!   pipe 의 주인을 본다 — 이름은 누구나 만들 수 있어서다.
//! - **받는 인스턴스는 늘 하나 서 있다.** 연결을 넘기기 전에 다음 인스턴스를 세운다. 그래서 `ERROR_PIPE_BUSY` 는
//!   거의 안 나고, 듣기를 닫은 뒤의 붙기는 `ConnectionRefused` 로 읽힌다.
//! - **절반 닫기는 없다.** pipe 에 `shutdown(SHUT_WR)` 이 없어 [`Stream`] 도 그것을 안 낸다.

use std::ffi::c_void;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_HANDLE_EOF, ERROR_IO_PENDING,
    ERROR_NO_DATA, ERROR_OPERATION_ABORTED, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED,
    GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
    SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile,
    SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, PeekNamedPipe, WaitNamedPipeW,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, INFINITE, OpenProcessToken, SetEvent, WaitForSingleObject,
};

/// pipe 의 버퍼 크기. 넘는 쓰기는 읽는 쪽을 기다린다 — 시한이 그 기다림을 끊는다.
const BUFFER: u32 = 64 * 1024;
/// 붙을 때 받는 인스턴스가 비었으면 기다리는 시한 — 연결을 넘기는 짧은 틈만 넘는다.
const BUSY_WAIT: Duration = Duration::from_millis(200);

/// 닫힐 때 함께 닫는 핸들.
#[derive(Debug)]
struct Owned(HANDLE);

// SAFETY: 커널 핸들은 스레드에 안 묶인다.
unsafe impl Send for Owned {}
// SAFETY: 위와 같다 — 여러 스레드가 함께 쓰는 것은 겹친 입출력이 받는다.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: 이 값이 쥔 유일한 핸들이고 여기서 한 번 닫는다.
        unsafe { CloseHandle(self.0) };
    }
}

fn last() -> Error {
    Error::last_os_error()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn millis(wait: Option<Duration>) -> u32 {
    wait.map_or(INFINITE, |d| u32::try_from(d.as_millis()).unwrap_or(INFINITE - 1).min(INFINITE - 1))
}

fn event() -> Result<Owned> {
    // SAFETY: 이름 없는 수동 리셋 이벤트를 만든다 — 인자에 살아 있어야 할 포인터가 없다.
    let handle = unsafe { CreateEventW(null(), 1, 0, null()) };
    if handle.is_null() { Err(last()) } else { Ok(Owned(handle)) }
}

/// `ConvertSidToStringSidW` 가 돌려준 글을 옮기고 놓는다.
fn sid_text(sid: PSID) -> Result<String> {
    let mut text: *mut u16 = null_mut();
    // SAFETY: `sid` 는 부르는 쪽이 쥔 올바른 SID 고, 돌려받은 글은 아래에서 `LocalFree` 로 놓는다.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(last());
    }
    // SAFETY: 성공한 호출이 NUL 로 끝나는 글을 돌려주었다.
    let owned = unsafe {
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    };
    // SAFETY: 위 호출이 할당한 글이다.
    unsafe { LocalFree(text.cast()) };
    Ok(owned)
}

fn read_user_sid() -> Result<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: 이 프로세스의 의사 핸들로 토큰을 연다.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(last());
    }
    let token = Owned(token);
    let mut size = 0u32;
    // SAFETY: 크기만 묻는 첫 호출이다. 버퍼가 없어 실패로 끝나는 것이 정상이다.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut size) };
    // `u64` 로 잡는다 — `TOKEN_USER` 는 포인터를 담아 바이트 버퍼의 맞춤으로는 모자란다.
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: 위에서 받은 크기 이상의 버퍼다.
    if unsafe { GetTokenInformation(token.0, TokenUser, buffer.as_mut_ptr().cast(), size, &mut size) } == 0 {
        return Err(last());
    }
    // SAFETY: 성공한 호출이 버퍼 앞머리에 `TOKEN_USER` 를 썼다.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    sid_text(user.User.Sid)
}

/// 이 프로세스 사용자의 SID 글. 프로세스가 사는 동안 안 바뀌니 한 번만 읽는다.
fn user_sid() -> Result<&'static str> {
    static SID: OnceLock<std::result::Result<String, i32>> = OnceLock::new();
    match SID.get_or_init(|| read_user_sid().map_err(|e| e.raw_os_error().unwrap_or(0))) {
        Ok(sid) => Ok(sid),
        Err(code) => Err(Error::from_raw_os_error(*code)),
    }
}

/// 그 자리의 pipe 이름. 자리 글을 해시해 넣어 `MOAI_CTL_DIR` 이 다른 데몬이 서로 안 만난다.
pub fn pipe_name(path: &Path) -> Result<String> {
    // FNV-1a — 이름을 가르는 데만 쓴다. 표준의 `DefaultHasher` 는 판마다 값이 달라도 된다고 적혀 있어 안 쓴다.
    let text = path.to_string_lossy().to_lowercase();
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    Ok(format!(r"\\.\pipe\moai-{}-{hash:016x}", user_sid()?))
}

/// 이 사용자에게만 열린 보안 설명자.
struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    fn own() -> Result<Descriptor> {
        let sid = user_sid()?;
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})"));
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: NUL 로 끝나는 글을 넘기고, 돌려받은 설명자는 `Drop` 이 놓는다.
        let made = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if made == 0 { Err(last()) } else { Ok(Descriptor(descriptor)) }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: 변환 호출이 할당한 설명자다.
        unsafe { LocalFree(self.0) };
    }
}

fn instance(name: &[u16], first: bool) -> Result<Owned> {
    let descriptor = Descriptor::own()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
    // SAFETY: 이름은 NUL 로 끝나고 `attributes` 는 이 호출 동안 살아 있다.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            BUFFER,
            BUFFER,
            0,
            &attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE { Err(last()) } else { Ok(Owned(handle)) }
}

/// 이미 만든 pipe 의 클라이언트 쪽을 연다.
fn open(name: &[u16]) -> std::result::Result<Owned, u32> {
    // SAFETY: 이름은 NUL 로 끝난다. `SECURITY_IDENTIFICATION` 은 서버가 이 쪽을 가장하지 못하게 한다.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        // SAFETY: 바로 앞 호출의 실패를 읽는다.
        Err(unsafe { GetLastError() })
    } else {
        Ok(Owned(handle))
    }
}

/// 붙은 pipe 의 주인이 이 사용자인가.
fn owned_by_me(handle: HANDLE) -> Result<bool> {
    let mut owner: PSID = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: 살아 있는 핸들을 묻고, 돌려받은 설명자는 아래에서 놓는다. `owner` 는 그 설명자 안을 가리킨다.
    let said = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if said != 0 {
        return Err(Error::from_raw_os_error(said as i32));
    }
    let text = sid_text(owner);
    // SAFETY: 위 호출이 할당한 설명자다.
    unsafe { LocalFree(descriptor) };
    Ok(text? == user_sid()?)
}

#[derive(Debug)]
struct Shared {
    handle: Owned,
    read_timeout: Mutex<Option<Duration>>,
    write_timeout: Mutex<Option<Duration>>,
    nonblocking: AtomicBool,
}

/// 연결 하나. 복제본은 시한까지 함께 쥔다 — Unix 소켓의 `try_clone` 과 같다.
#[derive(Debug)]
pub struct Stream {
    shared: Arc<Shared>,
}

impl Stream {
    fn of(handle: Owned) -> Stream {
        Stream {
            shared: Arc::new(Shared {
                handle,
                read_timeout: Mutex::new(None),
                write_timeout: Mutex::new(None),
                nonblocking: AtomicBool::new(false),
            }),
        }
    }

    /// 그 자리의 데몬·창에 붙는다. 표시 파일이 없으면 `NotFound`, 있는데 아무도 안 받으면 `ConnectionRefused` 다.
    pub fn connect(path: impl AsRef<Path>) -> Result<Stream> {
        let path = path.as_ref();
        std::fs::symlink_metadata(path)?;
        let name = wide(&pipe_name(path)?);
        let deadline = Instant::now() + BUSY_WAIT;
        let handle = loop {
            match open(&name) {
                Ok(handle) => break handle,
                Err(ERROR_FILE_NOT_FOUND) => return Err(ErrorKind::ConnectionRefused.into()),
                Err(ERROR_PIPE_BUSY) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(ErrorKind::ConnectionRefused.into());
                    }
                    // SAFETY: 이름은 NUL 로 끝난다. 시한이 끝나 돌아와도 위 `open` 이 다시 답한다.
                    unsafe { WaitNamedPipeW(name.as_ptr(), millis(Some(left))) };
                }
                Err(code) => return Err(Error::from_raw_os_error(code as i32)),
            }
        };
        if !owned_by_me(handle.0)? {
            return Err(Error::new(ErrorKind::PermissionDenied, "the pipe belongs to another user"));
        }
        Ok(Stream::of(handle))
    }

    /// 서로 붙은 한 쌍 — `UnixStream::pair` 와 같다. 이름은 이 프로세스 안에서만 쓰고 표시 파일이 없다.
    pub fn pair() -> Result<(Stream, Stream)> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            r"\\.\pipe\moai-pair-{}-{}-{}",
            user_sid()?,
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let name = wide(&name);
        let armed = Armed::new(instance(&name, true)?)?;
        let client = open(&name).map_err(|code| Error::from_raw_os_error(code as i32))?;
        Ok((Stream::of(armed.take_connected()?), Stream::of(client)))
    }

    pub fn try_clone(&self) -> Result<Stream> {
        Ok(Stream { shared: Arc::clone(&self.shared) })
    }

    pub fn set_read_timeout(&self, wait: Option<Duration>) -> Result<()> {
        *self.shared.read_timeout.lock().unwrap_or_else(PoisonError::into_inner) = wait;
        Ok(())
    }

    pub fn set_write_timeout(&self, wait: Option<Duration>) -> Result<()> {
        *self.shared.write_timeout.lock().unwrap_or_else(PoisonError::into_inner) = wait;
        Ok(())
    }

    pub fn set_nonblocking(&self, on: bool) -> Result<()> {
        self.shared.nonblocking.store(on, Ordering::Relaxed);
        Ok(())
    }

    /// 읽을 바이트 수. 끊긴 pipe 는 `None` 이다.
    fn available(&self) -> Result<Option<u32>> {
        let mut bytes = 0u32;
        // SAFETY: 살아 있는 핸들에서 남은 수만 묻는다.
        if unsafe { PeekNamedPipe(self.shared.handle.0, null_mut(), 0, null_mut(), &mut bytes, null_mut()) } != 0 {
            return Ok(Some(bytes));
        }
        // SAFETY: 바로 앞 호출의 실패를 읽는다.
        match unsafe { GetLastError() } {
            ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED => Ok(None),
            code => Err(Error::from_raw_os_error(code as i32)),
        }
    }

    /// 건너편이 끊었는가 — 읽을 것이 남았어도 끊긴 것만 "끊겼다" 다.
    pub fn hung_up(&self) -> bool {
        !matches!(self.available(), Ok(Some(_)))
    }

    /// 겹친 입출력 하나를 시한 안에 끝낸다. 시한이 지나면 걷어 내고 `TimedOut` 이다.
    fn finish(&self, wait: Option<Duration>, start: impl FnOnce(*mut OVERLAPPED) -> i32) -> Result<u32> {
        let event = event()?;
        // SAFETY: 영으로 채운 `OVERLAPPED` 는 올바른 첫 값이다.
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0;
        let handle = self.shared.handle.0;
        if start(&mut overlapped) == 0 {
            // SAFETY: 바로 앞 호출의 실패를 읽는다.
            let code = unsafe { GetLastError() };
            if code != ERROR_IO_PENDING {
                return Err(Error::from_raw_os_error(code as i32));
            }
        }
        let mut moved = 0u32;
        // SAFETY: 이 함수는 입출력이 끝나기 전에 안 돌아간다 — `overlapped` 와 버퍼가 그동안 산다.
        unsafe {
            if WaitForSingleObject(event.0, millis(wait)) == WAIT_OBJECT_0 {
                if GetOverlappedResult(handle, &overlapped, &mut moved, 0) != 0 {
                    return Ok(moved);
                }
                return Err(last());
            }
            CancelIoEx(handle, &overlapped);
            // 걷어 내기와 끝남이 겹칠 수 있다 — 끝난 입출력의 바이트를 버리면 줄이 잘린다.
            if GetOverlappedResult(handle, &overlapped, &mut moved, 1) != 0 {
                return Ok(moved);
            }
            match GetLastError() {
                ERROR_OPERATION_ABORTED => Err(ErrorKind::TimedOut.into()),
                code => Err(Error::from_raw_os_error(code as i32)),
            }
        }
    }

    fn read_some(&self, buf: &mut [u8]) -> Result<usize> {
        if self.shared.nonblocking.load(Ordering::Relaxed) {
            match self.available()? {
                None => return Ok(0),
                Some(0) => return Err(ErrorKind::WouldBlock.into()),
                Some(_) => {}
            }
        }
        let wait = *self.shared.read_timeout.lock().unwrap_or_else(PoisonError::into_inner);
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let handle = self.shared.handle.0;
        // SAFETY: `finish` 가 끝날 때까지 `buf` 가 산다.
        let read = self.finish(wait, |o| unsafe { ReadFile(handle, buf.as_mut_ptr(), len, null_mut(), o) });
        match read {
            Ok(n) => Ok(n as usize),
            Err(e) => match e.raw_os_error().map(|c| c as u32) {
                // 건너편이 닫은 것은 오류가 아니라 끝이다 — 소켓의 EOF 와 같다.
                Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED | ERROR_HANDLE_EOF) => Ok(0),
                _ => Err(e),
            },
        }
    }

    fn write_some(&self, buf: &[u8]) -> Result<usize> {
        let wait = match self.shared.nonblocking.load(Ordering::Relaxed) {
            true => Some(Duration::ZERO),
            false => *self.shared.write_timeout.lock().unwrap_or_else(PoisonError::into_inner),
        };
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let handle = self.shared.handle.0;
        // SAFETY: `finish` 가 끝날 때까지 `buf` 가 산다.
        let wrote = self.finish(wait, |o| unsafe { WriteFile(handle, buf.as_ptr(), len, null_mut(), o) });
        match wrote {
            Ok(n) => Ok(n as usize),
            Err(e) => match e.raw_os_error().map(|c| c as u32) {
                Some(ERROR_BROKEN_PIPE | ERROR_NO_DATA | ERROR_PIPE_NOT_CONNECTED) => Err(ErrorKind::BrokenPipe.into()),
                // 막지 않는 쓰기의 시한은 0 이다 — 소켓이라면 "자리가 없다" 다.
                _ if e.kind() == ErrorKind::TimedOut && self.shared.nonblocking.load(Ordering::Relaxed) => {
                    Err(ErrorKind::WouldBlock.into())
                }
                _ => Err(e),
            },
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_some(buf)
    }
}

impl Read for &Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_some(buf)
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.write_some(buf)
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Write for &Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.write_some(buf)
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// 연결을 기다리는 인스턴스 하나.
struct Armed {
    handle: Option<Owned>,
    event: Owned,
    /// 커널이 입출력 내내 이 주소를 쥔다 — 그래서 상자에 든다.
    overlapped: Box<OVERLAPPED>,
    /// `ConnectNamedPipe` 가 끝나지 않아 커널이 `overlapped` 를 쥐고 있다.
    waiting: bool,
}

// SAFETY: 핸들과 `OVERLAPPED` 의 주소는 스레드에 안 묶이고, `Listener` 가 자물쇠 안에서만 만진다.
unsafe impl Send for Armed {}

impl Armed {
    fn new(handle: Owned) -> Result<Armed> {
        let event = event()?;
        // SAFETY: 영으로 채운 `OVERLAPPED` 는 올바른 첫 값이다.
        let mut overlapped: Box<OVERLAPPED> = Box::new(unsafe { std::mem::zeroed() });
        overlapped.hEvent = event.0;
        // SAFETY: `overlapped` 는 상자에 들어 `Armed` 가 사는 동안 주소가 안 바뀐다.
        let waiting = unsafe {
            if ConnectNamedPipe(handle.0, &mut *overlapped) != 0 {
                false
            } else {
                match GetLastError() {
                    ERROR_IO_PENDING => true,
                    // 만들기와 기다리기 사이에 붙었다 — 이벤트가 안 서니 손으로 세운다.
                    ERROR_PIPE_CONNECTED => {
                        SetEvent(event.0);
                        false
                    }
                    code => return Err(Error::from_raw_os_error(code as i32)),
                }
            }
        };
        Ok(Armed { handle: Some(handle), event, overlapped, waiting })
    }

    /// 붙은 핸들을 꺼낸다. 끝나기 전에 부르면 끝나기를 기다린다.
    fn take_connected(mut self) -> Result<Owned> {
        if self.waiting {
            let handle = self.handle.as_ref().map_or(null_mut(), |h| h.0);
            let mut moved = 0u32;
            // SAFETY: 살아 있는 핸들과 그 입출력의 `overlapped` 다.
            let done = unsafe { GetOverlappedResult(handle, &*self.overlapped, &mut moved, 1) };
            self.waiting = false;
            if done == 0 {
                return Err(last());
            }
        }
        self.handle.take().ok_or_else(|| Error::from(ErrorKind::NotConnected))
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        if self.waiting
            && let Some(handle) = &self.handle
        {
            let mut moved = 0u32;
            // SAFETY: 입출력을 걷어 내고 끝나기를 기다린다 — 그 전에 `overlapped` 를 놓으면 커널이 놓인 자리에 쓴다.
            unsafe {
                CancelIoEx(handle.0, &*self.overlapped);
                GetOverlappedResult(handle.0, &*self.overlapped, &mut moved, 1);
            }
        }
    }
}

/// 받는 쪽. 닫히면 기다리던 인스턴스도 함께 닫힌다. 표시 파일은 `UnixListener` 의 소켓 파일처럼 부르는 쪽이 지운다.
pub struct Listener {
    name: Vec<u16>,
    armed: Mutex<Option<Armed>>,
}

impl Listener {
    /// 그 자리에 묶는다. 이미 누가 서 있으면 `AddrInUse` 다.
    pub fn bind(path: impl AsRef<Path>) -> Result<Listener> {
        let path = path.as_ref();
        let text = pipe_name(path)?;
        let name = wide(&text);
        let first = instance(&name, true).map_err(|e| match e.raw_os_error().map(|c| c as u32) {
            // `FILE_FLAG_FIRST_PIPE_INSTANCE` 는 이미 있는 이름을 이것으로 거절한다.
            Some(ERROR_ACCESS_DENIED | ERROR_PIPE_BUSY) => Error::from(ErrorKind::AddrInUse),
            _ => e,
        })?;
        let armed = Armed::new(first)?;
        std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => ErrorKind::AddrInUse.into(),
            _ => e,
        })?;
        std::fs::write(path, text)?;
        Ok(Listener { name, armed: Mutex::new(Some(armed)) })
    }

    fn arm(&self, armed: &mut Option<Armed>) -> Result<()> {
        if armed.is_none() {
            *armed = Some(Armed::new(instance(&self.name, false)?)?);
        }
        Ok(())
    }

    /// 받을 연결이 섰는가 — `wait` 까지만 기다린다.
    pub fn pending(&self, wait: Duration) -> Result<bool> {
        let event = {
            let mut armed = self.armed.lock().unwrap_or_else(PoisonError::into_inner);
            self.arm(&mut armed)?;
            armed.as_ref().map_or(null_mut::<c_void>(), |a| a.event.0)
        };
        // SAFETY: 이벤트는 받는 스레드가 `accept` 하기 전까지 산다 — 받는 스레드는 이것을 부른 스레드 하나다.
        Ok(unsafe { WaitForSingleObject(event, millis(Some(wait))) } == WAIT_OBJECT_0)
    }

    /// 받는 대로 내는 끝없는 줄 — `UnixListener::incoming` 과 같다.
    pub fn incoming(&self) -> impl Iterator<Item = Result<Stream>> + '_ {
        std::iter::repeat_with(|| self.accept().map(|(stream, ())| stream))
    }

    /// 연결 하나를 받는다. 넘기기 전에 다음 인스턴스를 세운다 — 그 틈에 붙는 쪽이 비지 않게.
    pub fn accept(&self) -> Result<(Stream, ())> {
        let mut armed = self.armed.lock().unwrap_or_else(PoisonError::into_inner);
        self.arm(&mut armed)?;
        let connected = armed.take().ok_or_else(|| Error::from(ErrorKind::NotConnected))?.take_connected();
        // 못 세웠으면 다음 `pending` 이 다시 세운다 — 그동안 붙는 쪽은 안 받는 것으로 읽는다.
        let _ = self.arm(&mut armed);
        Ok((Stream::of(connected?), ()))
    }
}
