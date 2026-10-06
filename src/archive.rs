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
    crate::held::read_inside(file, &crate::held::Home::of(root)).map_err(|e| {
        Fail::new(match e {
            crate::held::Fell::Unheld(e) => crate::held::spelled(file, &e),
            crate::held::Fell::Io(e) => format!("{}: {e}", file.display()),
        })
    })
}

pub fn read(root: &Path) -> R<Load> {
    let mut out = Load::default();
    for file in files(root)? {
        let mut load = parse_issues(&source(root, &file)?);
        for error in &mut load.errors {
            error.message = format!("{}: {}", file.display(), error.message);
        }
        out.issues.append(&mut load.issues);
        out.errors.append(&mut load.errors);
    }
    out.issues.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// The normal serialized shape needs no JSON parsing. Hand-written shapes use
/// the existing ID reader so moving a key cannot silently make its ID reusable.
pub fn id_counts(root: &Path) -> R<BTreeMap<String, usize>> {
    let mut out = BTreeMap::new();
    for file in files(root)? {
        for line in source(root, &file)?.lines() {
            let id = line.trim_start().strip_prefix(r#"{"id":""#).and_then(|s| s.split_once('"').map(|(id, _)| id));
            let id = match id.filter(|id| crate::id::is_valid(id) && !id.contains('\\')) {
                Some(id) => Some(id.to_string()),
                None => crate::id::id_of(line),
            };
            if let Some(id) = id {
                *out.entry(id).or_default() += 1;
            }
        }
    }
    Ok(out)
}

pub fn ids(root: &Path) -> R<BTreeSet<String>> {
    Ok(id_counts(root)?.into_keys().collect())
}

pub fn marks(root: &Path) -> R<Vec<(PathBuf, crate::store::Stamp)>> {
    let mut paths = vec![dir(root)];
    paths.extend(files(root)?);
    Ok(paths
        .into_iter()
        .map(|p| {
            let stamp = crate::store::stamp(&p);
            (p, stamp)
        })
        .collect())
}

pub fn read_all(root: &Path, active: Load) -> R<Load> {
    let mut archived = read(root)?;
    // Retain duplicates for diagnostics, with the live row winning Load::get.
    archived.issues.extend(active.issues);
    archived.issues.sort_by(|a, b| a.id.cmp(&b.id));
    archived.errors.extend(active.errors);
    Ok(archived)
}

/// Keep connected epic/member and parent/child bundles together. Milestones
/// stay live. An old closed member never leaves a bundle that is still open.
pub fn eligible(issues: &[Issue], cfg: &Config, now: &str) -> Vec<Issue> {
    if cfg.archive_days <= 0 {
        return Vec::new();
    }
    let stands = report::group_stands(issues, cfg);
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
        if !eligible || counts[i.id.as_str()] > 1 {
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
pub fn append(root: &Path, rows: &[Issue], cfg: &Config) -> R<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let archived = read(root)?;
    for row in rows {
        let twins: Vec<_> = archived.issues.iter().filter(|old| old.id == row.id).collect();
        if twins.iter().any(|old| *old != row)
            || twins.len() > 1
            || archived.errors.iter().any(|e| e.id.as_deref() == Some(&row.id))
        {
            return Err(Fail::new(format!("{}: conflicting archive row; active row kept", row.id)));
        }
    }
    let stands = report::group_stands(rows, cfg);
    let mut by_year: BTreeMap<String, Vec<&Issue>> = BTreeMap::new();
    for row in rows.iter().filter(|row| archived.get(&row.id).is_none()) {
        let finished = row.done_at.as_deref().unwrap_or_else(|| {
            stands.get(&(row.kind, row.id.as_str())).map_or(row.status_since.as_str(), |s| s.entered)
        });
        let year = finished.get(..4).filter(|y| y.bytes().all(|b| b.is_ascii_digit())).unwrap_or("unknown");
        by_year.entry(year.to_string()).or_default().push(row);
    }
    if by_year.is_empty() {
        return Ok(());
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
        lines.sort_by_cached_key(|line| crate::id::id_of(line).unwrap_or_default());
        writes.push((file, format!("{}\n", lines.join("\n"))));
    }
    for (file, text) in writes {
        crate::store::write_atomic_inside(&file, text.as_bytes(), root)?;
    }
    Ok(())
}

/// Remove selected rows after the live snapshot commits, still under its lock.
/// If cleanup fails, both copies remain and status reports the duplicate.
pub fn remove_ids(root: &Path, ids: &BTreeSet<String>) -> R<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut writes = Vec::new();
    for file in files(root)? {
        let src = source(root, &file)?;
        let mut kept = String::new();
        for line in src.split_inclusive('\n') {
            let remove = serde_json::from_str::<Issue>(line).is_ok_and(|i| ids.contains(&i.id));
            if !remove {
                kept.push_str(line);
            }
        }
        if kept != src {
            crate::store::measure_inside(&file, root)?;
            writes.push((file, kept));
        }
    }
    for (file, kept) in writes {
        crate::store::write_atomic_inside(&file, kept.as_bytes(), root)?;
    }
    Ok(())
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
        assert_eq!(id_counts(&s).unwrap()["argos-a001"], 1);
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
        assert!(append(&s, &[changed], &cfg()).is_err());
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

    #[cfg(unix)]
    #[test]
    fn archive_storage_rejects_outside_links_and_fifos() {
        use std::os::unix::fs::symlink;
        let s = scratch("archive-held");
        let away = Scratch::new("archive-away");
        fs::write(away.join("2025.jsonl"), "untouched").unwrap();
        symlink(away.join("2025.jsonl"), path(&s, "2025")).unwrap();
        assert!(read(&s).is_err());
        assert!(ids(&s).is_err());
        assert!(append(&s, &[row("argos-a001", Kind::Issue, "done")], &cfg()).is_err());
        assert_eq!(fs::read_to_string(away.join("2025.jsonl")).unwrap(), "untouched");
        fs::remove_file(path(&s, "2025")).unwrap();
        let fifo = path(&s, "2025");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(read(&s).is_err());
    }
}
