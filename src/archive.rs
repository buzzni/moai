//! Archived issue rows live in yearly JSONL files under `.moai/archive`.
//!
//! The archive is an explicit, operator-run storage move. Normal reads use only
//! the active snapshot; callers that need history ask for `read_all`.

use crate::config::Config;
use crate::model::{Issue, Kind};
use crate::report;
use crate::store::{Load, parse_issues};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DIR: &str = "archive";

pub fn dir(root: &Path) -> PathBuf {
    root.join(".moai").join(DIR)
}

pub fn path(root: &Path, year: &str) -> PathBuf {
    dir(root).join(format!("{year}.jsonl"))
}

fn year(issue: &Issue) -> Option<&str> {
    issue
        .done_at
        .as_deref()
        .or(Some(issue.status_since.as_str()))
        .and_then(|s| s.get(..4))
        .filter(|y| y.as_bytes().iter().all(u8::is_ascii_digit))
}

pub fn read(root: &Path) -> Load {
    let mut out = Load::default();
    let Ok(entries) = fs::read_dir(dir(root)) else { return out };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    for file in files {
        let Ok(src) = fs::read_to_string(&file) else { continue };
        let mut load = parse_issues(&src);
        out.issues.append(&mut load.issues);
        out.errors.append(&mut load.errors);
    }
    out.issues.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn ids(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = fs::read_dir(dir(root)) else { return out };
    for entry in entries.flatten() {
        let file = entry.path();
        if file.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let Ok(src) = fs::read_to_string(file) else { continue };
        for line in src.lines() {
            let Some(rest) = line.strip_prefix(r#"{"id":"#) else { continue };
            let Some((id, _)) = rest.split_once('"') else { continue };
            if crate::id::is_valid(id) {
                out.insert(id.to_string());
            }
        }
    }
    out
}

pub fn read_all(root: &Path, active: Load) -> Load {
    let archived = read(root);
    let mut issues = active.issues;
    issues.extend(archived.issues);
    issues.sort_by(|a, b| a.id.cmp(&b.id));
    let mut errors = active.errors;
    errors.extend(archived.errors);
    Load { issues, errors }
}

/// A group is moved as one unit. Milestones remain in the active snapshot so
/// they continue to provide release metadata even when an epic is archived.
pub fn eligible(issues: &[Issue], cfg: &Config, now: &str) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut groups = BTreeSet::new();
    let mut epic_members = BTreeSet::new();
    let stands = report::group_stands(issues, cfg);
    for i in issues.iter().filter(|i| i.kind == Kind::Epic) {
        let members = report::group_members(issues, i);
        epic_members.extend(members.iter().map(|member| member.id.clone()));
        let closed = stands
            .get(&(Kind::Epic, i.id.as_str()))
            .is_some_and(|s| report::archived(s.column, s.entered, now, cfg.archive_days));
        if closed && members.iter().all(|m| m.status.is_done()) {
            groups.insert(i.id.clone());
            out.push(i.clone());
            out.extend(members.into_iter().cloned());
        }
    }
    for i in issues {
        if i.kind == Kind::Milestone
            || groups.iter().any(|g| i.id == *g || crate::id::parent_of(&i.id) == Some(g.as_str()))
            || epic_members.contains(&i.id)
        {
            continue;
        }
        if report::archived(i.status.as_str(), &i.status_since, now, cfg.archive_days) {
            out.push(i.clone());
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

pub fn render(rows: &[Issue]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&serde_json::to_string(row).expect("Issue serializes"));
        out.push('\n');
    }
    out
}

pub fn append(root: &Path, rows: &[Issue]) -> std::io::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(dir(root))?;
    let mut by_year: std::collections::BTreeMap<&str, Vec<&Issue>> = std::collections::BTreeMap::new();
    for row in rows {
        by_year.entry(year(row).unwrap_or("unknown")).or_default().push(row);
    }
    for (y, rows) in by_year {
        let mut file = fs::OpenOptions::new().create(true).append(true).open(path(root, y))?;
        file.write_all(render(&rows.iter().cloned().cloned().collect::<Vec<_>>()).as_bytes())?;
        file.sync_data()?;
    }
    Ok(())
}

pub fn remove_ids(root: &Path, ids: &BTreeSet<String>) -> std::io::Result<Vec<Issue>> {
    let Ok(entries) = fs::read_dir(dir(root)) else { return Ok(Vec::new()) };
    let mut restored = Vec::new();
    for entry in entries.flatten() {
        let file = entry.path();
        if file.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let Ok(src) = fs::read_to_string(&file) else { continue };
        let mut kept = String::new();
        let mut changed = false;
        for line in src.lines() {
            if let Ok(issue) = serde_json::from_str::<Issue>(line)
                && ids.contains(&issue.id)
            {
                restored.push(issue);
                changed = true;
                continue;
            }
            kept.push_str(line);
            kept.push('\n');
        }
        if changed {
            let tmp = file.with_extension("jsonl.tmp");
            fs::write(&tmp, kept)?;
            fs::rename(tmp, &file)?;
        }
    }
    Ok(restored)
}
