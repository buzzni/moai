//! Explicit archive storage. Every mutation runs under the repository write lock.

use crate::config::Config;
use crate::fail::{Fail, R};
use crate::model::{Issue, Kind};
use crate::report;
use crate::store::{Load, parse_issues};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn dir(root: &Path) -> PathBuf {
    root.join(".moai/archive")
}

pub fn path(root: &Path, year: &str) -> PathBuf {
    dir(root).join(format!("{year}.jsonl"))
}

fn files(root: &Path) -> R<Vec<PathBuf>> {
    let home = crate::held::Home::of(root);
    let at = crate::held::place_dir(&dir(root), &home).map_err(|e| Fail::new(crate::held::spelled(&dir(root), &e)))?;
    let entries = match fs::read_dir(&at) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Fail::new(format!("{}: {e}", at.display()))),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Fail::new(format!("{}: {e}", at.display())))?;
        if entry.path().extension().is_some_and(|x| x == "jsonl") {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

fn source(root: &Path, file: &Path) -> R<String> {
    crate::held::read_inside(file, &crate::held::Home::of(root)).map_err(|e| fell(file, e))
}

/// [`source`] without the UTF-8 check — the same place, measured the same way.
fn bytes(root: &Path, file: &Path) -> R<Vec<u8>> {
    crate::held::read_bytes_inside(file, &crate::held::Home::of(root)).map_err(|e| fell(file, e))
}

fn fell(file: &Path, e: crate::held::Fell) -> Fail {
    Fail::new(match e {
        crate::held::Fell::Unheld(e) => crate::held::spelled(file, &e),
        crate::held::Fell::Io(e) => format!("{}: {e}", file.display()),
    })
}

/// A line without the byte-order mark an editor may put in front of a file. [`parse_issues`] reads past it, so every
/// other reader of archive lines must too — a cleanup that saw the mark as part of the row kept the copy it was meant
/// to remove (moai-bth3 review).
fn unmarked(line: &str) -> &str {
    line.strip_prefix('\u{feff}').unwrap_or(line)
}

/// The id a line names. The shape a write produces starts with its id, so the head answers without JSON parsing;
/// any other line goes through [`crate::id::id_of`], the one reader of an id (moai-ijfy), so moving a key never makes
/// an id reusable.
fn named(line: &str) -> Option<String> {
    let line = unmarked(line);
    let head = line.trim_start().strip_prefix(r#"{"id":""#).and_then(|s| s.split_once('"').map(|(id, _)| id));
    match head.filter(|id| crate::id::is_valid(id) && !id.contains('\\')) {
        Some(id) => Some(id.to_string()),
        None => crate::id::id_of(line),
    }
}

/// The archive ids a write must not mint again, and the sources it could not read (moai-bth3 review).
#[derive(Debug, Default)]
pub struct Reserved {
    pub ids: BTreeSet<String>,
    /// What was said about each archive source that could not be read at all — a FIFO, a link out of the
    /// checkout, a directory. Its ids are unknown.
    pub unread: Vec<String>,
}

/// Every write runs this under the repository lock, so it scans lines instead of parsing rows ([`named`]). A file
/// that is not UTF-8 still reserves the ids in it — ids are ASCII and survive a lossy decode — so one bad byte never
/// makes its ids reusable. An unreadable source never stops the write (ui8); it comes back in `unread`.
pub fn reserve(root: &Path) -> Reserved {
    let mut out = Reserved::default();
    let files = match files(root) {
        Ok(files) => files,
        Err(e) => {
            out.unread.push(e.message);
            return out;
        }
    };
    for file in files {
        match bytes(root, &file) {
            Ok(b) => out.ids.extend(String::from_utf8_lossy(&b).lines().filter_map(named)),
            Err(e) => out.unread.push(e.message),
        }
    }
    out
}

pub fn read(root: &Path) -> R<Load> {
    let mut out = Load::default();
    let files = match files(root) {
        Ok(files) => files,
        Err(e) => {
            out.errors.push(read_error(dir(root), e.message));
            return Ok(out);
        }
    };
    for file in files {
        let src = match source(root, &file) {
            Ok(src) => src,
            Err(e) => {
                out.errors.push(read_error(file, e.message));
                continue;
            }
        };
        let mut load = parse_issues(&src);
        for error in &mut load.errors {
            error.source = Some(file.clone());
        }
        out.issues.append(&mut load.issues);
        out.errors.append(&mut load.errors);
    }
    out.issues.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn read_error(file: PathBuf, message: String) -> crate::store::LoadError {
    crate::store::LoadError { source: Some(file), line: 0, message, text: String::new(), id: None }
}

/// Reserve IDs from both readable rows and opaque rows that still name an ID.
pub fn id_counts(load: &Load) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for id in load.issues.iter().map(|i| i.id.as_str()).chain(load.errors.iter().filter_map(|e| e.id.as_deref())) {
        *out.entry(id.to_string()).or_default() += 1;
    }
    out
}

#[cfg(test)]
pub fn ids(root: &Path) -> R<BTreeSet<String>> {
    Ok(id_counts(&read(root)?).into_keys().collect())
}

pub fn marks(root: &Path) -> R<Vec<(PathBuf, crate::store::Stamp)>> {
    let mut paths = vec![dir(root)];
    // An archive directory that cannot be listed is named by the readers; watching it never stops the explorer (ui8).
    paths.extend(files(root).unwrap_or_default());
    Ok(paths
        .into_iter()
        .map(|p| {
            let stamp = crate::store::stamp(&p);
            (p, stamp)
        })
        .collect())
}

/// Active rows win over stale archive copies when selecting current work — the rule of [`report::with_archive`],
/// readable and unreadable live ids alike, applied to rows this already owns.
pub fn context(root: &Path, active: Load) -> R<Load> {
    let mut archived = read(root)?;
    let live: BTreeSet<String> = active.issues.iter().map(|i| i.id.clone()).chain(active.reserved_ids()).collect();
    archived.issues.retain(|i| !live.contains(&i.id));
    archived.issues.extend(active.issues);
    archived.issues.sort_by(|a, b| a.id.cmp(&b.id));
    archived.errors.extend(active.errors);
    Ok(archived)
}

pub fn read_all(root: &Path, active: Load) -> R<Load> {
    let mut archived = read(root)?;
    // Retain duplicates for diagnostics, with the live row winning Load::get.
    archived.issues.extend(active.issues);
    archived.issues.sort_by(|a, b| a.id.cmp(&b.id));
    archived.errors.extend(active.errors);
    Ok(archived)
}

/// Ids repeated inside the archive or shared with a live row — `live` holds the readable live rows and the
/// unreadable live lines that still name an id.
pub fn collisions(live: &BTreeSet<&str>, archived: &Load) -> Vec<String> {
    id_counts(archived).into_iter().filter(|(id, n)| *n > 1 || live.contains(id.as_str())).map(|(id, _)| id).collect()
}

/// Existing restored rows also need context when their parent or group stayed archived.
pub fn needs_context(active: &[Issue], wanted: &BTreeSet<String>) -> bool {
    let by_id: BTreeMap<&str, &Issue> = active.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut pending: Vec<&str> = wanted.iter().map(String::as_str).collect();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(i) = by_id.get(id) else {
            return true;
        };
        if report::is_group(i) {
            return true;
        }
        pending.extend([crate::id::parent_of(&i.id), i.epic.as_deref(), i.milestone.as_deref()].into_iter().flatten());
    }
    false
}

/// Group rows reopen from their derived column; only requested rows are staged. An id the live file holds — as a
/// readable row or as an unreadable line (`opaque`, say from a newer binary) — is never staged: restoring the stale
/// archive copy beside it would write a live duplicate and shadow the newer row (moai-bth3 review).
pub fn restoring(
    active: &[Issue],
    archived: &[Issue],
    wanted: &BTreeSet<String>,
    opaque: &BTreeSet<&str>,
    cfg: &Config,
) -> Vec<Issue> {
    let active_ids: BTreeSet<&str> = active.iter().map(|i| i.id.as_str()).collect();
    let staged: Vec<&Issue> = archived
        .iter()
        .filter(|i| wanted.contains(&i.id) && !active_ids.contains(i.id.as_str()) && !opaque.contains(i.id.as_str()))
        .collect();
    if staged.is_empty() {
        return Vec::new();
    }
    let stands = report::group_stands(archived, cfg);
    staged
        .into_iter()
        .map(|i| {
            let mut row = i.clone();
            if let Some(stand) = stands.get(&(i.kind, i.id.as_str())) {
                row.status = crate::model::Status::new(stand.column);
                row.status_since = stand.entered.into();
            }
            row
        })
        .collect()
}

/// Failed guards and moves to done keep the selected row in the archive. A row taken over stays live even when it
/// did not move: the new assignee exists only in the live snapshot, and dropping it would leave the journal's
/// `Taken-over` note with nothing behind it (moai-bth3 review).
pub fn finish_restoring(
    active: &mut Vec<Issue>,
    staged: &BTreeSet<String>,
    moved: &[Issue],
    taken: &BTreeSet<String>,
) -> BTreeSet<String> {
    let restored: BTreeSet<String> = moved
        .iter()
        .filter(|i| !i.status.is_done())
        .map(|i| i.id.clone())
        .chain(taken.iter().cloned())
        .filter(|id| staged.contains(id))
        .collect();
    active.retain(|i| !staged.contains(&i.id) || restored.contains(&i.id));
    restored
}

/// Rows whose archive twin differs, repeats, or stands as an unreadable line. An identical single twin is not a
/// conflict: it is an interrupted move that the next run finishes.
pub fn conflicts(rows: &[Issue], archived: &Load) -> BTreeSet<String> {
    rows.iter()
        .filter(|row| {
            let twins: Vec<_> = archived.issues.iter().filter(|old| old.id == row.id).collect();
            twins.iter().any(|old| *old != *row)
                || twins.len() > 1
                || archived.errors.iter().any(|e| e.id.as_deref() == Some(&row.id))
        })
        .map(|row| row.id.clone())
        .collect()
}

/// What `moai archive` moves now: the eligible bundles that hold no conflicting row. The preview (`--dry-run`), the
/// `archive_pending` notice and the move itself all ask this, so the preview never promises a bundle the move keeps
/// live (moai-bth3 review).
///
/// Group columns are read with the archived members in view, the way every board reads them: an epic row restored
/// on its own stands done from its archived members, so it goes back with the next run instead of being counted as
/// archived by the board while the move never picks it.
pub fn movable(issues: &[Issue], archived: &Load, cfg: &Config, now: &str) -> Vec<Issue> {
    let context = report::with_archive(issues, &archived.issues, &BTreeSet::new());
    eligible_except(issues, &context, cfg, now, &conflicts(issues, archived))
}

/// Keep connected epic/member and parent/child bundles together. Milestones
/// stay live. An old closed member never leaves a bundle that is still open.
/// `stood` holds the rows group columns are read from — `issues` with archived rows beside it.
fn eligible_except(
    issues: &[Issue],
    stood: &[Issue],
    cfg: &Config,
    now: &str,
    excluded: &BTreeSet<String>,
) -> Vec<Issue> {
    if cfg.archive_days <= 0 {
        return Vec::new();
    }
    let stands = report::group_stands(stood, cfg);
    let handing = report::Handing::of(issues);
    let by_id: BTreeMap<&str, usize> = issues.iter().enumerate().map(|(at, i)| (i.id.as_str(), at)).collect();
    let mut parents: Vec<usize> = (0..issues.len()).collect();
    fn root(parents: &mut [usize], at: usize) -> usize {
        if parents[at] != at {
            parents[at] = root(parents, parents[at]);
        }
        parents[at]
    }
    for (at, i) in issues.iter().enumerate() {
        if i.kind == Kind::Milestone {
            continue;
        }
        for linked in [handing.at(i), crate::id::parent_of(&i.id)].into_iter().flatten() {
            if let Some(&other) = by_id.get(linked)
                && issues[other].kind != Kind::Milestone
            {
                let a = root(&mut parents, at);
                let b = root(&mut parents, other);
                parents[a] = b;
            }
        }
    }
    let mut blocked = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for i in issues {
        *counts.entry(i.id.as_str()).or_insert(0usize) += 1;
    }
    for (at, i) in issues.iter().enumerate() {
        let eligible = match i.kind {
            Kind::Milestone => false,
            Kind::Epic => stands
                .get(&(Kind::Epic, i.id.as_str()))
                .is_some_and(|s| report::archived(s.column, s.entered, now, cfg.archive_days)),
            _ => report::archived(i.status.as_str(), &i.status_since, now, cfg.archive_days),
        };
        if !eligible || counts[i.id.as_str()] > 1 || excluded.contains(&i.id) {
            blocked.insert(root(&mut parents, at));
        }
    }
    issues
        .iter()
        .enumerate()
        .filter(|(at, _)| !blocked.contains(&root(&mut parents, *at)))
        .map(|(_, i)| i.clone())
        .collect()
}

/// Prepare all destinations before writing any. Existing identical rows recover
/// an interrupted active-file write; conflicting or unreadable twins stay live.
pub fn append(root: &Path, rows: &[Issue], cfg: &Config) -> R<Vec<Issue>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let archived = read(root)?;
    // A conflicting member keeps its entire connected bundle live. Other
    // bundles can still move; eligibility is always decided as a bundle.
    let rows = movable(rows, &archived, cfg, &crate::model::now());
    let stands = report::group_stands(&rows, cfg);
    let mut by_year: BTreeMap<String, Vec<&Issue>> = BTreeMap::new();
    for row in rows.iter().filter(|row| archived.get(&row.id).is_none()) {
        let finished = row.done_at.as_deref().unwrap_or_else(|| {
            stands.get(&(row.kind, row.id.as_str())).map_or(row.status_since.as_str(), |s| s.entered)
        });
        let year = finished.get(..4).filter(|y| y.bytes().all(|b| b.is_ascii_digit())).unwrap_or("unknown");
        by_year.entry(year.to_string()).or_default().push(row);
    }
    if by_year.is_empty() {
        return Ok(rows);
    }
    let home = crate::held::Home::of(root);
    let at = crate::held::place_dir(&dir(root), &home).map_err(|e| Fail::new(crate::held::spelled(&dir(root), &e)))?;
    fs::create_dir_all(at).map_err(|e| Fail::new(format!("{}: {e}", dir(root).display())))?;
    let mut writes = Vec::new();
    for (year, rows) in by_year {
        let file = path(root, &year);
        crate::store::measure_inside(&file, root)?;
        let mut lines: Vec<String> =
            if file.exists() { source(root, &file)?.lines().map(str::to_string).collect() } else { Vec::new() };
        lines.extend(rows.into_iter().map(|row| serde_json::to_string(row).expect("Issue serializes")));
        // Sorting the raw lines keeps unknown fields and malformed rows intact.
        lines.sort_by_cached_key(|line| named(line).unwrap_or_default());
        writes.push((file, format!("{}\n", lines.join("\n"))));
    }
    for (file, text) in writes {
        crate::store::write_staged(&file, text.as_bytes(), root)?;
    }
    Ok(rows)
}

/// What `moai archive --drop` found.
#[derive(Debug, Default)]
pub struct Dropped {
    /// Archive lines removed.
    pub removed: usize,
    /// What was said about archive sources that could not be read; copies there were not checked.
    pub unread: Vec<String>,
    /// `created_at` of archived rows that carry the id but are another issue. When one stands, nothing is removed.
    pub others: Vec<String>,
}

/// Remove the archive copies of a live row — run after the live snapshot passed its checks, under the same lock as
/// archive and restore. A copy is the same issue: same kind and `created_at` as the live row. An archived row that only
/// shares the id is another issue (an id minted while its file could not be read, or minted by another clone), and
/// dropping it would erase that issue for good, so nothing is removed and the caller names it (moai-bth3 review). An
/// unreadable line naming the id cannot be compared and goes as a broken copy.
pub fn drop_copy(root: &Path, live: &Issue) -> R<Dropped> {
    let others: Vec<String> = read(root)?
        .issues
        .into_iter()
        .filter(|i| i.id == live.id && (i.kind != live.kind || i.created_at != live.created_at))
        .map(|i| i.created_at)
        .collect();
    if !others.is_empty() {
        return Ok(Dropped { others, ..Dropped::default() });
    }
    let (removed, unread) = remove_lines(root, |line| named(line).is_some_and(|id| id == live.id))?;
    Ok(Dropped { removed, unread, others })
}

/// Remove restored rows after the live snapshot commits, still under its lock.
/// If cleanup fails, both copies remain and status reports the duplicate.
pub fn remove_ids(root: &Path, ids: &BTreeSet<String>) -> R<()> {
    if ids.is_empty() {
        return Ok(());
    }
    remove_lines(root, |line| serde_json::from_str::<Issue>(unmarked(line)).is_ok_and(|i| ids.contains(&i.id)))
        .map(drop)
}

/// Rewrite every archive file holding a line `remove` picks; returns how many went and what was said about the files
/// that could not be read. Such a file is skipped: no reader sees a row in it, and stopping on it left restore cleanup
/// and `--drop` stuck on an unrelated broken file, with the repair outside the tool (moai-bth3 review). Only a file
/// this rewrites can fail the call, and every one is measured before the first write.
fn remove_lines(root: &Path, remove: impl Fn(&str) -> bool) -> R<(usize, Vec<String>)> {
    let (mut writes, mut unread, mut removed) = (Vec::new(), Vec::new(), 0);
    for file in files(root)? {
        let src = match source(root, &file) {
            Ok(src) => src,
            Err(e) => {
                unread.push(e.message);
                continue;
            }
        };
        let mut kept = String::new();
        for line in src.split_inclusive('\n') {
            if remove(line) {
                removed += 1;
            } else {
                kept.push_str(line);
            }
        }
        if kept != src {
            crate::store::measure_inside(&file, root)?;
            writes.push((file, kept));
        }
    }
    for (file, kept) in writes {
        crate::store::write_staged(&file, kept.as_bytes(), root)?;
    }
    Ok((removed, unread))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use crate::scratch::Scratch;
    const OLD: &str = "2025-12-01T00:00:00Z";
    const NOW: &str = "2026-02-01T00:00:00Z";

    fn row(id: &str, kind: Kind, status: &str) -> Issue {
        Issue::new(id.into(), id.into(), kind, Status::new(status), OLD)
    }
    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }
    fn scratch(name: &str) -> Scratch {
        let s = Scratch::new(name);
        fs::create_dir_all(dir(&s)).unwrap();
        fs::write(s.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        fs::write(s.join(".moai/issues.jsonl"), "").unwrap();
        s
    }
    /// What would move with nothing archived yet.
    fn eligible(rows: &[Issue], cfg: &Config, now: &str) -> Vec<Issue> {
        movable(rows, &Load::default(), cfg, now)
    }
    fn put(root: &Path, rows: &[Issue]) {
        let text: String = rows.iter().map(|row| format!("{}\n", serde_json::to_string(row).unwrap())).collect();
        fs::write(path(root, "2025"), text).unwrap();
    }

    #[test]
    fn archive_never_splits_a_live_epic_or_parent_review_bundle() {
        let rows = vec![
            row("argos-e001", Kind::Epic, "done"),
            row("argos-e001.aaa", Kind::Issue, "done"),
            row("argos-e001.bbb", Kind::Issue, "todo"),
            row("argos-i001", Kind::Issue, "done"),
            row("argos-i001.rrr", Kind::Issue, "review"),
            row("argos-m001", Kind::Milestone, "done"),
            row("argos-d001", Kind::Issue, "done"),
        ];
        assert_eq!(eligible(&rows, &cfg(), NOW).iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["argos-d001"]);
        let mut rows = rows;
        rows[2].status = Status::new("done");
        rows[4].status = Status::new("done");
        assert_eq!(eligible(&rows, &cfg(), NOW).len(), 6);
    }

    #[test]
    fn archive_keeps_recent_members_and_duplicate_rows_live() {
        let mut fresh = row("argos-e001.bbb", Kind::Issue, "done");
        fresh.status_since = NOW.into();
        let rows = vec![row("argos-e001", Kind::Epic, "todo"), row("argos-e001.aaa", Kind::Issue, "done"), fresh];
        assert!(eligible(&rows, &cfg(), NOW).is_empty());
        assert!(
            eligible(&[row("argos-i001", Kind::Issue, "done"), row("argos-i001", Kind::Issue, "done")], &cfg(), NOW)
                .is_empty()
        );
        let mut no_archive = cfg();
        no_archive.archive_days = 0;
        assert!(eligible(&[row("argos-i001", Kind::Issue, "done")], &no_archive, NOW).is_empty());
    }

    #[test]
    fn archive_ids_are_reserved_under_the_repository_write_lock() {
        let s = scratch("archive-ids");
        put(&s, &[row("argos-a001", Kind::Issue, "done")]);
        let repo = crate::store::Repo::at(s.to_path_buf(), cfg());
        repo.with_write(
            || crate::i18n::Lang::En,
            |_, _, reserved| {
                assert!(reserved.contains("argos-a001"));
                Ok((vec![], ()))
            },
        )
        .unwrap();
        assert_eq!(id_counts(&read(&s).unwrap())["argos-a001"], 1);
    }

    #[test]
    fn archive_append_recovers_an_interrupted_move_without_losing_a_changed_live_row() {
        let s = scratch("archive-recovery");
        let row = row("argos-a001", Kind::Issue, "done");
        append(&s, std::slice::from_ref(&row), &cfg()).unwrap();
        let first = fs::read(path(&s, "2025")).unwrap();
        append(&s, std::slice::from_ref(&row), &cfg()).unwrap();
        assert_eq!(fs::read(path(&s, "2025")).unwrap(), first);
        let mut changed = row;
        changed.title = "changed after interruption".into();
        assert!(append(&s, &[changed], &cfg()).unwrap().is_empty());
        assert_eq!(fs::read(path(&s, "2025")).unwrap(), first);
    }

    #[test]
    fn archive_cleanup_preserves_unreadable_rows_and_unknown_fields_verbatim() {
        let s = scratch("archive-opaque");
        put(&s, &[row("argos-a001", Kind::Issue, "done")]);
        let opaque = "not JSON\n{\"id\":\"argos-b001\",\"future\":true}\n";
        let mut src = fs::read_to_string(path(&s, "2025")).unwrap();
        src.push_str(opaque);
        fs::write(path(&s, "2025"), src).unwrap();
        remove_ids(&s, &BTreeSet::from(["argos-a001".into()])).unwrap();
        assert_eq!(fs::read_to_string(path(&s, "2025")).unwrap(), opaque);
        assert!(ids(&s).unwrap().contains("argos-b001"));
    }

    #[test]
    fn committed_restore_keeps_both_cleanup_and_journal_diagnostics() {
        let s = scratch("archive-post-commit");
        let repo = crate::store::Repo::at(s.to_path_buf(), cfg());
        fs::write(s.join(".moai/journal"), "not a directory").unwrap();
        let by = crate::model::Actor::parse("Raven (raven@example.com)").unwrap();
        let result = repo
            .with_write_after(
                || crate::i18n::Lang::En,
                |rows, _, _, _| {
                    rows.push(row("argos-a001", Kind::Issue, "todo"));
                    Ok((vec![crate::model::JournalEntry::note("argos-a001", "keep this note", NOW, &by)], 42))
                },
                |_| Err(Fail::new("cleanup failed")),
            )
            .unwrap();
        assert_eq!(result, 42);
        assert!(repo.read().unwrap().get("argos-a001").is_some());
        let mine = |notes: Vec<(PathBuf, String)>| -> Vec<String> {
            notes.into_iter().filter(|(root, _)| root == s.path()).map(|(_, said)| said).collect()
        };
        // Each diagnostic keeps its own channel: a lost journal line is worded as lost history, a failed
        // cleanup is not (it would tell the person their kept history is gone).
        let lost = mine(crate::store::journal_misses());
        let noted = mine(crate::store::write_notes());
        assert_eq!(
            (lost.len(), noted.len()),
            (1, 1),
            "one committed-write diagnostic covered another: {lost:?} {noted:?}"
        );
        assert!(lost[0].contains("argos-a001") && lost[0].contains("moai note"));
        assert!(noted[0].contains("cleanup failed") && !noted[0].contains("moai note"));
    }

    /// The write path reserves ids by scanning, not parsing: a file that is not UTF-8 still reserves what it names, a
    /// byte-order mark does not hide the first row, and a source that cannot be read at all is named instead.
    #[cfg(unix)]
    #[test]
    fn archive_reservation_scans_damaged_files_and_names_unreadable_ones() {
        let s = scratch("archive-reserve");
        let mut bytes = b"\xef\xbb\xbf{\"id\":\"argos-a001\",\"title\":\"marked\"}\n".to_vec();
        bytes.extend_from_slice(b"{\"id\":\"argos-b001\",\"title\":\"a bad \xff byte\"}\n");
        fs::write(path(&s, "2025"), bytes).unwrap();
        let fifo = std::ffi::CString::new(path(&s, "2024").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let reserved = reserve(&s);
        assert!(reserved.ids.contains("argos-a001") && reserved.ids.contains("argos-b001"), "{reserved:?}");
        assert_eq!(reserved.unread.len(), 1, "{reserved:?}");
        assert!(reserved.unread[0].contains("2024.jsonl"));
    }

    /// Cleanup reads lines the way [`read`] does: a byte-order mark in front of the copy does not keep it, and a
    /// file that cannot be read is skipped instead of failing the whole cleanup.
    #[cfg(unix)]
    #[test]
    fn archive_cleanup_reads_past_marks_and_skips_unreadable_files() {
        let s = scratch("archive-cleanup-mark");
        put(&s, &[row("argos-a001", Kind::Issue, "done")]);
        let marked = format!("\u{feff}{}", fs::read_to_string(path(&s, "2025")).unwrap());
        fs::write(path(&s, "2025"), marked).unwrap();
        assert!(read(&s).unwrap().get("argos-a001").is_some());
        fs::write(path(&s, "2024"), [0xff, 0xfe]).unwrap();
        remove_ids(&s, &BTreeSet::from(["argos-a001".into()])).unwrap();
        assert_eq!(fs::read_to_string(path(&s, "2025")).unwrap(), "");
    }

    /// `--drop` removes copies of the live row only. An archived row that merely shares the id is another issue.
    #[test]
    fn archive_drop_keeps_another_issue_under_the_same_id() {
        let s = scratch("archive-drop-other");
        let archived = row("argos-a001", Kind::Issue, "done");
        put(&s, std::slice::from_ref(&archived));
        let mut other = archived.clone();
        other.created_at = NOW.into();
        let refused = drop_copy(&s, &other).unwrap();
        assert_eq!((refused.removed, refused.others.as_slice()), (0, [OLD.to_string()].as_slice()));
        assert!(read(&s).unwrap().get("argos-a001").is_some());
        let mut copy = archived;
        copy.title = "edited since".into();
        assert_eq!(drop_copy(&s, &copy).unwrap().removed, 1);
        assert!(read(&s).unwrap().get("argos-a001").is_none());
    }

    /// Archive writes stage their temp files in `.moai/`, which the ignore rule `.moai/*.tmp.*` covers — a crash in
    /// the middle leaves nothing beside the year file for `git add` to pick up. Every temp name beside the year file is
    /// taken here, so a write that still lands staged elsewhere.
    #[test]
    fn archive_writes_stage_their_temp_files_under_the_ignored_directory() {
        let s = scratch("archive-staged");
        put(&s, &[row("argos-a001", Kind::Issue, "done")]);
        let moved = [row("argos-b001", Kind::Issue, "done")];
        let written = crate::store::with_tmp_names_taken(&dir(&s), "2025.jsonl", || append(&s, &moved, &cfg()));
        assert_eq!(written.unwrap().len(), 1);
        assert!(ids(&s).unwrap().contains("argos-b001"));
        let ids_b = BTreeSet::from(["argos-b001".to_string()]);
        crate::store::with_tmp_names_taken(&dir(&s), "2025.jsonl", || remove_ids(&s, &ids_b)).unwrap();
        assert!(!ids(&s).unwrap().contains("argos-b001"));
    }

    #[cfg(unix)]
    #[test]
    fn archive_storage_rejects_outside_links_and_fifos() {
        use std::os::unix::fs::symlink;
        let s = scratch("archive-held");
        let away = Scratch::new("archive-away");
        fs::write(away.join("2025.jsonl"), "untouched").unwrap();
        symlink(away.join("2025.jsonl"), path(&s, "2025")).unwrap();
        assert!(!read(&s).unwrap().errors.is_empty());
        assert!(ids(&s).unwrap().is_empty());
        assert!(append(&s, &[row("argos-a001", Kind::Issue, "done")], &cfg()).is_err());
        assert_eq!(fs::read_to_string(away.join("2025.jsonl")).unwrap(), "untouched");
        fs::remove_file(path(&s, "2025")).unwrap();
        let fifo = path(&s, "2025");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(!read(&s).unwrap().errors.is_empty());
    }
}
