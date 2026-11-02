//! Machine-local heartbeat I/O. The tracker and journal are never written here.
use crate::{
    cli::Dialect,
    config::Config,
    hook::{Event, Picks},
    model::Issue,
    report::{Agent, Beat, Pulse},
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// `cmd::hook::picks_dir` 를 트래커의 뿌리로 — `cmd::hook::add_picked` 의 손이 `Repo` 를 안 붙들고 물을 때 잰다.
pub(crate) fn picks_dir_at(root: &Path) -> Option<PathBuf> {
    let config = crate::user_config::path().filter(|p| p.is_absolute())?;
    Some(config.parent()?.join("picks").join(format!("{:016x}", picks_key(root))))
}

/// `cmd::hook::picks_dir` 의 이름 — 트래커의 자리를 센 값이다.
///
/// **`MOAI_HERE` 로 루트에 안 옮긴 딸린 워크트리는 제 뿌리로 센다**(moai-59k3.yo6). 그 워크트리와
/// 루트는 `tracker_place` 가 같은 값(공용 디렉터리, 꼭대기에서 뿌리까지)을 내는데 보는 스냅샷은
/// 다르다 — 한 자리에 적던 판은 루트의 남의 집기 하나가 워크트리의 `held()` 를 비지 않게 만들어
/// 규칙 2 를 껐다. 옮길 루트가 있는데([`crate::worktree::tracker_root`]) 이 뿌리가 거기 없다는 것이
/// 곧 갈라 놓은 트래커다. 맨몸 저장소의 워크트리는 옮길 루트가 없어 전처럼 한 자리를 함께 쓴다.
pub(crate) fn picks_key(root: &Path) -> u64 {
    let shared = match crate::worktree::tracker_root(root) {
        Some(_) => None,
        None => crate::worktree::tracker_place(root),
    };
    match shared {
        Some((common, rel)) => crate::text::fnv1a64_from(
            crate::text::fnv1a64_from(crate::text::fnv1a64(common.as_os_str().as_encoded_bytes()), &[0]),
            rel.as_os_str().as_encoded_bytes(),
        ),
        None => crate::text::fnv1a64(crate::path::real(root).as_os_str().as_encoded_bytes()),
    }
}

/// `cmd::hook::safe_sid` 의 속 — 훅 입력이 아닌 자리(`moai mv` 의 환경, `cmd::hook::record_own_picks`)도 **같은 조각**을 지어야
/// 훅이 그 기록을 제 세션의 것으로 읽는다.
pub(crate) fn safe_sid_of(sid: &str) -> Option<String> {
    let safe: String = sid.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    (!safe.is_empty()).then_some(safe)
}

/// **오래 안 적힌 세션의 기록은 치운다**(리뷰 moai-3k2d.1df). 읽는 쪽(`cmd::hook::read_picks`)은 판정마다 이 디렉터리를
/// 통째로 읽는데, 세션마다 파일이 하나씩 쌓이고 아무도 안 지우면 그 값이 기계가 떠 있는 동안 는다. 두 주 넘게
/// 한 줄도 안 적은 세션의 기록을 지운다 — 지운 줄은 기록이 없던 때처럼 판정한다. 적을 때만 치운다 — 드물다.
///
/// **다른 트래커의 자리도 치운다**(리뷰 moai-59k3.4c3). temp 에 둘 때는 기계가 치웠지만 설정 곁은 아무도
/// 안 치운다 — 지운 워크트리·옮긴 저장소의 자리는 다시 안 적혀, 제 자리만 치우던 판에서는 영영 남았다.
/// 옆 자리는 오래된 기록만 지우고, 빈 디렉터리는 걷는다(`remove_dir` 는 빈 것만 지운다).
pub(crate) fn prune_records(home: &Path, dir: &Path, sid: &str) {
    const KEEP: std::time::Duration = std::time::Duration::from_secs(14 * 24 * 60 * 60);
    let stale = |entry: &std::fs::DirEntry| {
        entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP)
    };
    let sweep = |at: &Path, keep: Option<&str>| {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for entry in entries.filter_map(Result::ok) {
            if keep.is_some_and(|k| entry.file_name() == k) {
                continue;
            }
            if stale(&entry) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    };
    sweep(dir, Some(sid));
    let Ok(others) = std::fs::read_dir(home) else { return };
    for entry in others.filter_map(Result::ok) {
        if entry.path() == dir || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        sweep(&entry.path(), None);
        let _ = std::fs::remove_dir(entry.path());
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn beats_dir_at(root: &Path) -> Option<PathBuf> {
    let picks = picks_dir_at(root)?;
    Some(picks.parent()?.parent()?.join("beats").join(picks.file_name()?))
}

pub fn beat(root: &Path, sid: &str, agent: Dialect, event: Event) {
    use std::io::Write;
    let Some(dir) = beats_dir_at(root) else { return };
    let Some(home) = dir.parent() else { return };
    let fresh = !home.exists();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    #[cfg(unix)]
    if fresh {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700));
    }
    let _ = fresh;
    let tmp = dir.join(format!(".{}-{}", sid, std::process::id()));
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut open, libc::O_NOFOLLOW);
    if let Ok(mut file) = open.open(&tmp) {
        if writeln!(file, "{}\t{}", agent_of(agent).as_str(), event.wire()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(sid));
        }
        let _ = std::fs::remove_file(&tmp);
    }
    // Tool calls write one file only; cleanup belongs to infrequent turn events.
    if event != Event::PreToolUse {
        prune_records(home, &dir, sid);
    }
}

fn agent_of(agent: Dialect) -> Agent {
    match agent {
        Dialect::Claude => Agent::Claude,
        Dialect::Codex => Agent::Codex,
        Dialect::Antigravity => Agent::Antigravity,
    }
}

fn regular_text(path: &Path) -> Option<(std::fs::Metadata, String)> {
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let mut open = std::fs::OpenOptions::new();
    open.read(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut open, libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let mut file = open.open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    use std::io::Read;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some((meta, text))
}

pub fn read(root: &Path, issues: &[Issue], cfg: &Config) -> Pulse {
    let pulse = Pulse { threshold_secs: cfg.session_lost_minutes.saturating_mul(60), ..Default::default() };
    if cfg.session_lost_minutes == 0 {
        return pulse;
    }
    let (Some(picks), Some(beats)) = (picks_dir_at(root), beats_dir_at(root)) else { return pulse };
    read_at(&picks, &beats, issues, cfg, pulse)
}

fn read_at(picks: &Path, beats: &Path, issues: &[Issue], cfg: &Config, mut pulse: Pulse) -> Pulse {
    // Apply the shared wip rule first, including deferred ancestors and eclipsed rows: with nothing held there is
    // no pick record or heartbeat to open, and a holder of a row that is not held work never reads its beat.
    let eligible: std::collections::BTreeSet<_> =
        crate::report::wip(issues, cfg).into_iter().map(|i| i.id.as_str()).collect();
    if eligible.is_empty() {
        return pulse;
    }
    let files: Vec<_> = std::fs::read_dir(picks)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| Some((e.file_name().to_str()?.to_owned(), regular_text(&e.path())?.1)))
        .collect();
    let rows: BTreeMap<_, _> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // A terminal takeover changes updated_at without moving the column or writing a session pick. Compare the
    // existing tracker-clock stamp, not the wall-clock ordering nonce (MOAI_NOW may differ). Legacy records without
    // a stamp use their wall time. This cutoff has no status-move slack: a later snapshot write ends the observation.
    // Filter before choosing a holder, so an old certain pick cannot hide a fresh tentative pick after that write.
    let accepts = |id: &str, holder: &crate::hook::Holder| {
        let picked_at = holder.stamp_secs.or_else(|| i64::try_from(holder.at_nanos / 1_000_000_000).ok());
        !matches!((picked_at, rows.get(id).and_then(|i| crate::model::parse_rfc3339(&i.updated_at))),
            (Some(picked_at), Some(updated_at)) if updated_at > picked_at)
    };
    let holders = Picks::holders(
        files.iter().map(|(s, t)| (s, t)),
        &|id| rows.get(id).and_then(|i| crate::model::parse_rfc3339(&i.status_since)),
        &accepts,
    );
    let mut cache = BTreeMap::new();
    for (id, holder) in holders {
        if !eligible.contains(id.as_str()) || safe_sid_of(&holder.session).as_deref() != Some(holder.session.as_str()) {
            continue;
        }
        let beat = cache.entry(holder.session.clone()).or_insert_with(|| {
            let (meta, text) = regular_text(&beats.join(&holder.session))?;
            let (agent, event) = text.strip_suffix('\n')?.split_once('\t')?;
            let agent = match agent {
                "claude" => Agent::Claude,
                "codex" => Agent::Codex,
                "antigravity" => Agent::Antigravity,
                _ => return None,
            };
            if !["SessionStart", "UserPromptSubmit", "PreToolUse", "Stop"].contains(&event) {
                return None;
            }
            let last_seen_secs = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
            Some(Beat { session: holder.session.clone(), agent, last_event: event.into(), last_seen_secs })
        });
        if let Some(beat) = beat {
            pulse.beats.insert(id, beat.clone());
        }
    }
    pulse
}

/// Only the state transition is folded: healthy heartbeats never invalidate a view.
/// This path reads directory entries and metadata only, never heartbeat contents or the tracker.
///
/// **Who holds a row is folded too** — the pick records' names and sizes. Taking over a silent row in the same column
/// (`moai mv <id> <col> --from <col>`) writes a pick and may write nothing to the tracker; when the new session's
/// heartbeat was already alive, no beat changes state either, and the web kept the row paused until something else
/// moved. Pick files only grow by appends, so a size is enough; they change only on picks, not on every tool call.
pub fn fingerprint(root: &Path, cfg: &Config, now: i64) -> u64 {
    if cfg.session_lost_minutes == 0 {
        return 0;
    }
    let Some(dir) = beats_dir_at(root) else { return 0 };
    let beats = fingerprint_at(&dir, cfg.session_lost_minutes.saturating_mul(60), now);
    match picks_dir_at(root) {
        Some(picks) => picks_fingerprint_at(&picks, beats),
        None => beats,
    }
}

fn picks_fingerprint_at(dir: &Path, seed: u64) -> u64 {
    let mut sizes = BTreeMap::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            sizes.insert(entry.file_name().to_string_lossy().into_owned(), meta.len());
        }
    }
    sizes.into_iter().fold(crate::text::fnv1a64_from(seed, &[1]), |hash, (sid, len)| {
        crate::text::fnv1a64_from(crate::text::fnv1a64_from(hash, sid.as_bytes()), &len.to_le_bytes())
    })
}

fn fingerprint_at(dir: &Path, threshold: i64, now: i64) -> u64 {
    let mut states = BTreeMap::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let Some(secs) = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        else {
            continue;
        };
        states.insert(name, now.saturating_sub(secs) > threshold);
    }
    states.into_iter().fold(0, |hash, (sid, lost)| {
        crate::text::fnv1a64_from(crate::text::fnv1a64_from(hash, sid.as_bytes()), &[0, u8::from(lost)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(path: &Path, seconds: u64) {
        std::fs::File::open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn heartbeat_fingerprint_changes_only_on_state_transitions() {
        let temp = crate::scratch::Scratch::new("pulse-fingerprint");
        let file = temp.path().join("sid");
        std::fs::write(&file, "claude\tStop\n").unwrap();
        at(&file, 10000);
        let alive = fingerprint_at(temp.path(), 1800, 11800);
        assert_ne!(alive, fingerprint_at(temp.path(), 1800, 11801));
        at(&file, 11000);
        assert_eq!(alive, fingerprint_at(temp.path(), 1800, 11801));
        let disabled = Config::parse("prefix = \"moai\"\nsession_lost_minutes = 0\n").unwrap();
        assert_eq!(fingerprint(temp.path(), &disabled, 11801), 0);
        std::fs::write(temp.path().join(".temporary"), "partial").unwrap();
        assert_eq!(alive, fingerprint_at(temp.path(), 1800, 11801));
    }

    #[test]
    fn a_new_pick_changes_the_fingerprint_without_a_state_transition() {
        let temp = crate::scratch::Scratch::new("pulse-picks-fingerprint");
        let picks = temp.path().join("picks");
        std::fs::create_dir(&picks).unwrap();
        let empty = picks_fingerprint_at(&picks, 7);
        std::fs::write(picks.join("sid"), Picks::line(100, Some(1000), "moai-a", true)).unwrap();
        let one = picks_fingerprint_at(&picks, 7);
        assert_ne!(empty, one);
        assert_eq!(one, picks_fingerprint_at(&picks, 7), "nothing moved, nothing changes");
        let mut more = Picks::line(100, Some(1000), "moai-a", true);
        more.push_str(&Picks::line(200, Some(1100), "moai-a", true));
        std::fs::write(picks.join("sid"), more).unwrap();
        assert_ne!(one, picks_fingerprint_at(&picks, 7), "an appended pick is a new holder to read");
    }

    #[test]
    fn heartbeat_read_uses_the_holder_and_mtime_and_ignores_symlinks() {
        let temp = crate::scratch::Scratch::new("pulse-read");
        let picks = temp.path().join("picks");
        let beats = temp.path().join("beats");
        std::fs::create_dir(&picks).unwrap();
        std::fs::create_dir(&beats).unwrap();
        let row: Issue = serde_json::from_str(r#"{"id":"moai-a","title":"A","status":"in_progress","status_since":"1970-01-01T02:46:40Z","created_at":"1970-01-01T00:00:00Z","updated_at":"1970-01-01T02:46:40Z"}"#).unwrap();
        std::fs::write(picks.join("sid"), Picks::line(100, Some(10000), "moai-a", true)).unwrap();
        let file = beats.join("sid");
        std::fs::write(&file, "codex\tPreToolUse\n").unwrap();
        at(&file, 10000);
        let cfg = Config::parse("prefix = \"moai\"\n").unwrap();
        let read = || {
            read_at(
                &picks,
                &beats,
                std::slice::from_ref(&row),
                &cfg,
                Pulse { threshold_secs: 1800, ..Default::default() },
            )
        };
        let pulse = read();
        let beat = &pulse.beats["moai-a"];
        assert_eq!(beat.last_seen_secs, 10000);
        assert_eq!(beat.agent, Agent::Codex);
        assert!(crate::report::session_of(&row, &cfg, &pulse, 11801).unwrap().lost);
        #[cfg(unix)]
        {
            std::fs::remove_file(&file).unwrap();
            std::os::unix::fs::symlink(picks.join("sid"), &file).unwrap();
            assert!(read().beats.is_empty());
        }
    }
    #[test]
    fn a_later_snapshot_write_ends_a_hold_until_a_new_pick() {
        let temp = crate::scratch::Scratch::new("pulse-snapshot-takeover");
        let picks = temp.path().join("picks");
        let beats = temp.path().join("beats");
        std::fs::create_dir(&picks).unwrap();
        std::fs::create_dir(&beats).unwrap();
        let cfg = Config::parse("prefix = \"moai\"\n").unwrap();
        let mut row = Issue::new(
            "moai-a".into(),
            "A".into(),
            crate::model::Kind::Issue,
            crate::model::Status::new("in_progress"),
            "1970-01-01T02:46:40Z",
        );
        let record = picks.join("sid");
        // The ordering nonce deliberately differs from the tracker clock: compare the stored tracker stamp.
        std::fs::write(&record, Picks::line(99_000_000_000_000, Some(10000), &row.id, true)).unwrap();
        let file = beats.join("sid");
        std::fs::write(&file, "claude\tStop\n").unwrap();
        at(&file, 10000);
        let read = |row: &Issue| {
            read_at(
                &picks,
                &beats,
                std::slice::from_ref(row),
                &cfg,
                Pulse { threshold_secs: 1800, ..Default::default() },
            )
        };
        let pulse = read(&row);
        assert!(crate::report::session_of(&row, &cfg, &pulse, 11801).unwrap().lost);
        assert!(!crate::report::session_of(&row, &cfg, &pulse, 10001).unwrap().lost);
        row.updated_at = crate::model::format_rfc3339(10001);
        assert!(read(&row).beats.is_empty(), "one second after a pick is already a later write");
        at(&file, 13600);
        assert!(read(&row).beats.is_empty(), "a heartbeat alone must not reclaim a stale pick");
        std::fs::write(&record, Picks::line(99_000_000_000_001, Some(10001), &row.id, true)).unwrap();
        let pulse = read(&row);
        assert!(!crate::report::session_of(&row, &cfg, &pulse, 13601).unwrap().lost);
        assert!(crate::report::session_of(&row, &cfg, &pulse, 15401).unwrap().lost);
        // A stale certain pick must not mask a new tentative pick by a successor.
        std::fs::write(&record, Picks::line(99_000_000_000_000, Some(10000), &row.id, true)).unwrap();
        std::fs::write(picks.join("next"), Picks::line(99_000_000_000_001, Some(10001), &row.id, false)).unwrap();
        std::fs::write(beats.join("next"), "codex\tPreToolUse\n").unwrap();
        at(&beats.join("next"), 13600);
        assert_eq!(read(&row).beats[&row.id].session, "next");
        std::fs::remove_file(picks.join("next")).unwrap();
        // Legacy records have no tracker-clock stamp; their existing wall timestamp is the only comparison.
        std::fs::write(&record, Picks::line(10_000_000_000_000, None, &row.id, true)).unwrap();
        assert!(read(&row).beats.is_empty());
    }
}
