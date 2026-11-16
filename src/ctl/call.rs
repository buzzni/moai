use std::path::{Path, PathBuf};
#[cfg(any(unix, windows))]
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[cfg(any(unix, windows))]
use super::wire::{self, Verb};
use super::wire::{Reply, Request};
use crate::fail::{Fail, R, code};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    pub kind: String,
    pub pid: u32,
    pub version: String,
    pub proto: u32,
    pub root: String,
    pub here: String,
    pub layer: bool,
    pub screen: String,
    pub last_input_ago_ms: u64,
    #[serde(skip)]
    pub socket: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct Skipped {
    /// Written lossily: a socket directory that is not UTF-8 must not panic `ctl ls --json` (`json!` unwraps).
    #[serde(serialize_with = "lossy")]
    pub socket: PathBuf,
    pub reason: &'static str,
    #[serde(skip)]
    code: &'static str,
    /// What made a `gone` socket unreadable, for the selection error only.
    #[serde(skip)]
    detail: Option<String>,
}

impl Skipped {
    /// The listing names three reasons (`docs/ctl.md`); `code` keeps what a selection needing this socket fails with.
    fn new(socket: PathBuf, code: &'static str, detail: Option<String>) -> Skipped {
        let reason = match code {
            code::BUSY => "busy",
            code::TIMEOUT => "timeout",
            _ => "gone",
        };
        Skipped { socket, reason, code, detail }
    }
}

fn lossy<S: serde::Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&path.to_string_lossy())
}

#[derive(Debug, Default)]
pub struct Discovery {
    pub windows: Vec<Window>,
    pub skipped: Vec<Skipped>,
}

impl Discovery {
    /// **Selection needs every socket answered** (moai-pwxx.kah). A skipped socket reports no id, repository or last
    /// input, so a pick among the rest could aim at an older window, and `-t` would read its own window as absent.
    fn require_complete(self) -> R<Vec<Window>> {
        let Some(skipped) = self.skipped.first() else { return Ok(self.windows) };
        let what = match skipped.code {
            code::BUSY => "is busy; retry control discovery".to_string(),
            code::TIMEOUT => "did not answer in time; retry control discovery".to_string(),
            // A protocol mismatch does not pass with a retry.
            code::UNSUPPORTED => "speaks another control protocol; use the moai version that opened it".to_string(),
            _ => match &skipped.detail {
                Some(detail) => format!("closed or could not answer ({detail}); retry control discovery"),
                None => "closed or could not answer; retry control discovery".to_string(),
            },
        };
        Err(Fail::coded(format!("The window at {} {what}.", skipped.socket.display()), skipped.code))
    }
}

#[cfg(any(unix, windows))]
pub fn windows() -> R<Discovery> {
    let dir = super::place::dir().map_err(|e| Fail::new(e.to_string()))?;
    windows_in(&dir, None, false)
}

/// Resolve an explicit window through its PID socket without contacting unrelated windows.
#[cfg(any(unix, windows))]
pub fn windows_for_target(target: Option<&str>) -> R<Vec<Window>> {
    let dir = super::place::dir().map_err(|e| Fail::new(e.to_string()))?;
    windows_in(&dir, target, false)?.require_complete()
}

/// Discover a reconnecting tab only when a proposal names its exact window.
#[cfg(any(unix, windows))]
pub fn windows_for_proposal(target: Option<&str>) -> R<Vec<Window>> {
    let dir = super::place::dir().map_err(|e| Fail::new(e.to_string()))?;
    windows_in(&dir, target, true)?.require_complete()
}

#[cfg(any(unix, windows))]
fn target_pid(target: &str) -> Option<u32> {
    let pid = if let Some(pid) = target.strip_prefix("tui-") {
        pid
    } else {
        let (pid, tab) = target.split_once('.')?;
        tab.parse::<u64>().ok()?;
        pid
    };
    pid.parse::<u32>().ok().filter(|pid| *pid > 0)
}

#[cfg(any(unix, windows))]
fn windows_in(dir: &Path, target: Option<&str>, proposal: bool) -> R<Discovery> {
    let paths = if let Some(target) = target {
        let Some(pid) = target_pid(target) else { return Ok(Discovery::default()) };
        vec![dir.join(format!("{pid}.sock"))]
    } else {
        std::fs::read_dir(dir)
            .map_err(|e| Fail::new(e.to_string()))?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|e| Fail::new(e.to_string())))
            .collect::<R<Vec<_>>>()?
    };
    let mut result = Discovery::default();
    for path in paths {
        if path.extension().is_none_or(|e| e != "sock") {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if !crate::transport::is_own(&path, &meta) {
            continue;
        }
        let mut req = wire::request(Verb::Hello, Duration::from_millis(1200));
        req.target = target.filter(|_| proposal).map(str::to_owned);
        match send_at(&path, &req) {
            Ok(reply) if reply.ok => {
                if let Some(data) = reply.data {
                    let entries = data
                        .get("windows")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                        .unwrap_or_else(|| vec![data]);
                    for entry in entries {
                        if let Ok(mut window) = serde_json::from_value::<Window>(entry)
                            && target.is_none_or(|id| window.id == id)
                        {
                            window.socket = path.clone();
                            result.windows.push(window);
                        }
                    }
                }
            }
            Ok(reply) => {
                let (code, detail) = match reply.code.as_deref() {
                    // A full window and one whose person just left the editor are both live.
                    Some("busy" | "person_busy") => (code::BUSY, None),
                    Some("timeout") => (code::TIMEOUT, None),
                    Some("unsupported") => (code::UNSUPPORTED, None),
                    other => (code::GONE, Some(format!("replied {}", other.unwrap_or("without a code")))),
                };
                result.skipped.push(Skipped::new(path, code, detail));
            }
            // Windows: every pipe instance stayed taken (`transport::pipe`), the daemon probe's `Busy`.
            Err(e) if e.kind() == std::io::ErrorKind::ResourceBusy => {
                result.skipped.push(Skipped::new(path, code::BUSY, None));
            }
            // The read timed out before the window's own `timeout` reply arrived — the same window, not a gone one.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                result.skipped.push(Skipped::new(path, code::TIMEOUT, None));
            }
            // Another user's pipe (`transport::pipe` checks the owner) is not our window; a Windows marker pending
            // deletion also reads as access denied, and its window is already closing. A sharing violation is not this
            // kind (std leaves it uncategorized) and lists as `gone` below.
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound) => {
                // Recheck the path before removal; never follow or remove a foreign file.
                if let Ok(now) = std::fs::symlink_metadata(&path)
                    && crate::transport::is_own(&path, &now)
                    && crate::transport::identity(&now) == crate::transport::identity(&meta)
                {
                    let _ = std::fs::remove_file(&path);
                }
            }
            Err(e) => result.skipped.push(Skipped::new(path, code::GONE, Some(e.to_string()))),
        }
    }
    result.windows.sort_by(|a, b| a.id.cmp(&b.id));
    result.skipped.sort_by(|a, b| a.socket.cmp(&b.socket));
    Ok(result)
}

#[cfg(not(any(unix, windows)))]
pub fn windows() -> R<Discovery> {
    Ok(Discovery::default())
}

#[cfg(not(any(unix, windows)))]
pub fn windows_for_proposal(_: Option<&str>) -> R<Vec<Window>> {
    Ok(Vec::new())
}

#[cfg(not(any(unix, windows)))]
pub fn windows_for_target(_: Option<&str>) -> R<Vec<Window>> {
    Ok(Vec::new())
}

/// The caller's repository root, spelled the way windows report theirs: `Repo::find`, the opener every command uses,
/// so a linked worktree reads its main checkout as explorers (`site.repo.root`) and web tabs (`Repo::opened_root`) do.
/// A repository that cannot be opened is an error — reading it as "outside any repository" would aim the request
/// at another repository's window.
pub fn current_root(lang: crate::i18n::Lang) -> R<Option<String>> {
    Ok(crate::store::Repo::find(|| lang)?.map(|r| r.root.display().to_string()))
}

pub fn pick<'a>(windows: &'a [Window], root: Option<&str>, want: Option<&str>) -> R<&'a Window> {
    let chosen = if let Some(id) = want {
        windows.iter().find(|w| w.id == id)
    } else {
        windows.iter().filter(|w| root.is_none_or(|r| w.root == r)).min_by_key(|w| w.last_input_ago_ms)
    };
    chosen.ok_or_else(|| Fail::coded("No open window here. See moai ctl ls --all, then use -t <id>.", code::NO_WINDOW))
}

#[cfg(any(unix, windows))]
pub fn send_at(path: &Path, request: &Request) -> std::io::Result<Reply> {
    exchange(crate::transport::Stream::connect(path)?, request)
}

/// One request and one reply. **The reply is read even when the write breaks.** A window with no free client slot writes
/// `busy` and closes without reading (`serve`), and that refusal is still in the receive buffer. Without this a live
/// window that filled up after `ctl` listed it read as gone (moai-t0ys.0dk).
#[cfg(any(unix, windows))]
fn exchange(mut stream: crate::transport::Stream, request: &Request) -> std::io::Result<Reply> {
    use crate::transport::{armed, broken};
    use std::io::{BufRead, BufReader, Read, Write};
    let ms = request.deadline_ms.saturating_sub(wire::now_ms()).max(1);
    armed(stream.set_read_timeout(Some(Duration::from_millis(ms))))?;
    armed(stream.set_write_timeout(Some(Duration::from_millis(ms))))?;
    let mut line = serde_json::to_vec(request)?;
    line.push(b'\n');
    let cut = match stream.write_all(&line) {
        Ok(()) => None,
        Err(e) if broken(&e) => Some(e),
        Err(e) => return Err(e),
    };
    line.clear();
    let read = BufReader::new(stream).take(wire::LINE_MAX as u64 + 1).read_until(b'\n', &mut line);
    if let (Some(cut), true) = (cut, line.is_empty()) {
        return Err(cut);
    }
    read?;
    if line.len() > wire::LINE_MAX {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "control reply exceeds 1 MiB"));
    }
    serde_json::from_slice(&line).map_err(std::io::Error::other)
}

#[cfg(not(any(unix, windows)))]
pub fn send_at(_: &Path, _: &Request) -> std::io::Result<Reply> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "control sockets are unavailable on this OS"))
}

pub fn send(window: &Window, request: &Request) -> R<Reply> {
    let mut directed = request.clone();
    directed.target = Some(window.id.clone());
    send_at(&window.socket, &directed)
        .map_err(|e| Fail::coded(format!("window {} is gone: {e}", window.id), code::GONE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: &str, root: &str, ago: u64) -> Window {
        Window {
            id: id.into(),
            kind: "tui".into(),
            pid: 1,
            version: "test".into(),
            proto: 1,
            root: root.into(),
            here: root.into(),
            layer: false,
            screen: "list".into(),
            last_input_ago_ms: ago,
            socket: PathBuf::new(),
        }
    }

    #[test]
    fn the_target_is_the_latest_window_of_this_repository() {
        let windows = vec![window("a", "/one", 300), window("b", "/one", 20), window("c", "/two", 1)];
        assert_eq!(pick(&windows, Some("/one"), None).unwrap().id, "b");
        assert_eq!(pick(&windows, Some("/one"), Some("a")).unwrap().id, "a");
        assert_eq!(pick(&windows, None, None).unwrap().id, "c");
    }

    /// A window with no free client slot writes `busy` and closes without reading the request. That refusal stays
    /// readable, so a closed socket must not read as a window that is gone (moai-t0ys.0dk).
    #[test]
    #[cfg(any(unix, windows))]
    fn a_refusal_written_before_the_close_is_read() {
        use std::io::Write;
        let (ours, mut theirs) = crate::transport::Stream::pair().unwrap();
        serde_json::to_writer(&mut theirs, &Reply::error("busy", "too many control clients")).unwrap();
        theirs.write_all(b"\n").unwrap();
        drop(theirs);
        let reply = exchange(ours, &wire::request(Verb::Hello, Duration::from_secs(1))).unwrap();
        assert_eq!(reply.code.as_deref(), Some("busy"));
    }

    #[test]
    fn no_window_here_does_not_fall_through_to_another_repository() {
        let windows = vec![window("other", "/two", 1)];
        assert_eq!(pick(&windows, Some("/one"), None).unwrap_err().code, code::NO_WINDOW);
    }

    #[test]
    #[cfg(unix)]
    fn discovery_reports_busy_and_unreadable_replies() {
        use std::io::Write;
        use std::os::unix::net::UnixListener;
        struct TempDir(PathBuf);
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = TempDir(std::env::temp_dir().join(format!("moai-pwxx-{}-{}", std::process::id(), wire::now_ms())));
        std::fs::create_dir(&dir.0).unwrap();
        let dir = &dir.0;
        let listener = UnixListener::bind(dir.join("2.sock")).unwrap();
        let owner = std::thread::spawn(move || {
            for refusal in [
                Some("busy"),
                Some("busy"),
                Some("timeout"),
                None,
                Some("silent"),
                Some("person_busy"),
                Some("unsupported"),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                match refusal {
                    // A window whose owner loop never answers: hold the connection until the caller gives up.
                    Some("silent") => {
                        let _ = std::io::Read::read_to_end(&mut stream, &mut Vec::new());
                    }
                    Some(code) => {
                        serde_json::to_writer(&mut stream, &Reply::error(code, "refused")).unwrap();
                        stream.write_all(b"\n").unwrap();
                    }
                    None => {}
                }
            }
        });
        let listed = windows_in(dir, None, false).unwrap();
        assert!(listed.windows.is_empty());
        assert_eq!(listed.skipped[0].reason, "busy");
        assert_eq!(listed.skipped[0].socket, dir.join("2.sock"));
        assert_eq!(listed.require_complete().unwrap_err().code, code::BUSY);
        assert_eq!(windows_in(dir, Some("2.7"), false).unwrap().require_complete().unwrap_err().code, code::BUSY);
        assert_eq!(windows_in(dir, Some("2.7"), false).unwrap().require_complete().unwrap_err().code, code::TIMEOUT);
        let gone = windows_in(dir, Some("2.7"), false).unwrap().require_complete().unwrap_err();
        assert_eq!(gone.code, code::GONE);
        // The transport error behind `gone` reaches the person; the old message dropped it.
        assert!(gone.message.contains("could not answer ("), "{}", gone.message);
        assert_eq!(windows_in(dir, None, false).unwrap().require_complete().unwrap_err().code, code::TIMEOUT);
        assert_eq!(windows_in(dir, Some("2.7"), false).unwrap().require_complete().unwrap_err().code, code::BUSY);
        let other = windows_in(dir, Some("2.7"), false).unwrap();
        assert_eq!(other.skipped[0].reason, "gone");
        let other = other.require_complete().unwrap_err();
        assert_eq!(other.code, code::UNSUPPORTED);
        // A protocol mismatch does not pass with a retry, so the message does not ask for one.
        assert!(
            other.message.contains("another control protocol") && !other.message.contains("retry"),
            "{}",
            other.message
        );
        owner.join().unwrap();
    }

    /// `ctl ls --json` builds its reply with `json!`, which unwraps: a socket path that is not UTF-8 must still serialize.
    #[test]
    #[cfg(unix)]
    fn a_skipped_socket_that_is_not_utf8_still_serializes() {
        use std::os::unix::ffi::OsStrExt;
        let socket = PathBuf::from(std::ffi::OsStr::from_bytes(b"/run/moai-\xff/2.sock"));
        let value = serde_json::to_value(Skipped::new(socket, code::BUSY, None)).unwrap();
        assert_eq!(value["reason"], "busy");
        assert!(value["socket"].as_str().unwrap().ends_with("/2.sock"));
    }

    #[test]
    #[cfg(unix)]
    fn explicit_targets_skip_unrelated_sockets_and_preserve_proposal_hello() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("moai-4km9-target-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let slow = UnixListener::bind(dir.join("1.sock")).unwrap();
        slow.set_nonblocking(true).unwrap();
        let target = UnixListener::bind(dir.join("2.sock")).unwrap();
        let owner = std::thread::spawn(move || {
            for expected in [None, Some("2.7")] {
                let (mut stream, _) = target.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let req: Request = serde_json::from_str(&line).unwrap();
                assert_eq!(req.target.as_deref(), expected);
                let mut tab = window("2.7", "/one", 0);
                tab.kind = "web".into();
                tab.pid = 2;
                let reply = Reply::success(serde_json::json!({"windows": [tab]}));
                serde_json::to_writer(&mut stream, &reply).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
        assert_eq!(windows_in(&dir, Some("2.7"), false).unwrap().windows[0].id, "2.7");
        assert_eq!(windows_in(&dir, Some("2.7"), true).unwrap().windows[0].id, "2.7");
        owner.join().unwrap();
        assert_eq!(slow.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        for invalid in ["../1", "tui-../../1", "1", "2.x", "tui-0"] {
            assert!(windows_in(&dir, Some(invalid), false).unwrap().windows.is_empty());
        }
        assert_eq!(target_pid("tui-42"), Some(42));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
