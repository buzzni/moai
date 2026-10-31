use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[cfg(unix)]
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

#[cfg(unix)]
pub fn windows() -> R<Vec<Window>> {
    windows_for_target(None)
}

/// Resolve an explicit window through its PID socket without contacting unrelated windows.
#[cfg(unix)]
pub fn windows_for_target(target: Option<&str>) -> R<Vec<Window>> {
    let dir = super::place::dir().map_err(|e| Fail::new(e.to_string()))?;
    windows_in(&dir, target, false)
}

/// Discover a reconnecting tab only when a proposal names its exact window.
#[cfg(unix)]
pub fn windows_for_proposal(target: Option<&str>) -> R<Vec<Window>> {
    let dir = super::place::dir().map_err(|e| Fail::new(e.to_string()))?;
    windows_in(&dir, target, true)
}

#[cfg(unix)]
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

#[cfg(unix)]
fn windows_in(dir: &Path, target: Option<&str>, proposal: bool) -> R<Vec<Window>> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let paths = if let Some(target) = target {
        let Some(pid) = target_pid(target) else { return Ok(Vec::new()) };
        vec![dir.join(format!("{pid}.sock"))]
    } else {
        std::fs::read_dir(dir)
            .map_err(|e| Fail::new(e.to_string()))?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|e| Fail::new(e.to_string())))
            .collect::<R<Vec<_>>>()?
    };
    let mut windows = Vec::new();
    for path in paths {
        if path.extension().is_none_or(|e| e != "sock") {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if !meta.file_type().is_socket() || meta.uid() != unsafe { libc::geteuid() } {
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
                            windows.push(window);
                        }
                    }
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound) => {
                // Recheck the path before removal; never follow or remove a foreign file.
                if let Ok(now) = std::fs::symlink_metadata(&path)
                    && now.file_type().is_socket()
                    && now.uid() == unsafe { libc::geteuid() }
                    && now.ino() == meta.ino()
                {
                    let _ = std::fs::remove_file(&path);
                }
            }
            _ => {}
        }
    }
    windows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(windows)
}

#[cfg(not(unix))]
pub fn windows() -> R<Vec<Window>> {
    Ok(Vec::new())
}

#[cfg(not(unix))]
pub fn windows_for_proposal(_: Option<&str>) -> R<Vec<Window>> {
    Ok(Vec::new())
}

#[cfg(not(unix))]
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

#[cfg(unix)]
pub fn send_at(path: &Path, request: &Request) -> std::io::Result<Reply> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(path)?;
    let ms = request.deadline_ms.saturating_sub(wire::now_ms()).max(1);
    stream.set_read_timeout(Some(Duration::from_millis(ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(ms)))?;
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n")?;
    let mut line = Vec::new();
    BufReader::new(stream).take(wire::LINE_MAX as u64 + 1).read_until(b'\n', &mut line)?;
    if line.len() > wire::LINE_MAX {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "control reply exceeds 1 MiB"));
    }
    serde_json::from_slice(&line).map_err(std::io::Error::other)
}

#[cfg(not(unix))]
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

    #[test]
    fn no_window_here_does_not_fall_through_to_another_repository() {
        let windows = vec![window("other", "/two", 1)];
        assert_eq!(pick(&windows, Some("/one"), None).unwrap_err().code, code::NO_WINDOW);
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
        assert_eq!(windows_in(&dir, Some("2.7"), false).unwrap()[0].id, "2.7");
        assert_eq!(windows_in(&dir, Some("2.7"), true).unwrap()[0].id, "2.7");
        owner.join().unwrap();
        assert_eq!(slow.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        for invalid in ["../1", "tui-../../1", "1", "2.x", "tui-0"] {
            assert!(windows_in(&dir, Some(invalid), false).unwrap().is_empty());
        }
        assert_eq!(target_pid("tui-42"), Some(42));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
