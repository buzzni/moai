//! Explicit archive storage. Every mutation runs under the repository write lock.

use crate::config::Config;
use crate::fail::{Fail, R};
use crate::model::{Issue, Kind};
use crate::report;
use crate::store::{Load, parse_issues};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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

/// The part of the archive a board over `live` can read — what `Stop` and its baseline count with, at the end of
/// every agent turn (moai-i9ji). Parsing every archived row there made each turn end slower as the archive grew.
///
/// **Rows are joined by the ids they name**: a line's own id and every id above it, and every string value that
/// reads as an id (its epic, milestone and blockers, among others). Starting from the ids the live rows name — the
/// same reach, plus the ids of unreadable live lines — every line that shares an id with what has been reached comes
/// in, and what it names is reached in turn. So what comes back is whole: every row tied to the live snapshot, the
/// archived members of a live epic or milestone, and everything those rows reach, at any distance. A row outside that
/// is never a parent, group, member or blocker of a live row, and the board's warnings name live rows only
/// (`report::status_with_archive`), so they come out the same as over the whole archive.
///
/// Three kinds of line come in whatever they name, so the id collisions (`collisions`) come out the same too:
/// - a line the scan cannot vouch for ([`Scan::head`]) is parsed for its own ids,
/// - an id at the head of two lines — a duplicate inside the archive,
/// - a line with a `milestone` value — milestones stay live, so an archived one is a hand edit, and the board reads
///   every milestone row for dues and for whether the repository uses milestones at all.
///
/// A line left out is read for its head id alone, and that id stands on no other line and on no live row, so it can
/// never collide. **Every unreadable line still comes back in `errors`, reached or not** — `archive_unreadable` is broken
/// data that `Stop` counts the way `moai status` does (moai-5y2a), so the count here is [`read`]'s. A line left out is
/// not parsed for that: [`crate::store::readable`] checks its shape without building the row, and only a line that
/// fails it is parsed for the error. The order of what comes back is [`read`]'s: file, then line, then a stable sort
/// by id.
pub fn around<'a>(root: &Path, live: &[Issue], opaque: impl IntoIterator<Item = &'a str>) -> Load {
    let mut out = Load::default();
    let files = match files(root) {
        Ok(files) => files,
        Err(e) => {
            out.errors.push(read_error(dir(root), e.message));
            return out;
        }
    };
    let mut sources = Vec::new();
    for file in files {
        match source(root, &file) {
            Ok(src) => sources.push((file, src)),
            Err(e) => out.errors.push(read_error(file, e.message)),
        }
    }
    // Every non-blank line, as `parse_issues` walks it: the file's byte-order mark comes off once, at its head.
    let mut lines: Vec<(usize, usize, &str, Scan)> = Vec::new();
    for (f, (_, src)) in sources.iter().enumerate() {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        for (n, text) in src.lines().enumerate().filter(|(_, t)| !t.trim().is_empty()) {
            lines.push((f, n, text, scan(text)));
        }
    }
    // The lines the scan cannot vouch for are parsed now, and what the parse names joins what the scan saw.
    let mut parsed: BTreeMap<usize, Result<Issue, crate::store::LoadError>> = BTreeMap::new();
    for (k, (_, n, text, s)) in lines.iter_mut().enumerate() {
        if s.head.is_some() {
            continue;
        }
        let row = crate::store::parse_line(*n, text);
        match &row {
            Ok(i) => s.ids.extend(named_by(i)),
            Err(e) => s.ids.extend(e.id.iter().cloned()),
        }
        parsed.insert(k, row);
    }
    // Hashed, not ordered: nothing here is read in order, and these maps hold an entry per archived row.
    let mut heads: HashMap<&str, usize> = HashMap::new();
    let mut by_id: HashMap<&str, Vec<usize>> = HashMap::new();
    for (k, (_, _, _, s)) in lines.iter().enumerate() {
        if let Some(h) = &s.head {
            *heads.entry(h.as_str()).or_default() += 1;
        }
        for id in &s.ids {
            by_id.entry(id.as_str()).or_default().push(k);
        }
    }
    let mut picked = vec![false; lines.len()];
    let mut queue: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, (_, _, _, s))| s.head.as_ref().is_none_or(|h| heads[h.as_str()] > 1) || s.milestone)
        .map(|(k, _)| k)
        .collect();
    let seeds: Vec<String> = live.iter().flat_map(named_by).chain(opaque.into_iter().map(str::to_string)).collect();
    let mut reached: HashSet<&str> = HashSet::new();
    let mut ids: Vec<&str> = seeds.iter().map(String::as_str).collect();
    loop {
        while let Some(id) = ids.pop() {
            if reached.insert(id) {
                queue.extend(by_id.get(id).into_iter().flatten().copied().filter(|k| !picked[*k]));
            }
        }
        let Some(k) = queue.pop() else { break };
        if !std::mem::replace(&mut picked[k], true) {
            ids.extend(lines[k].3.ids.iter().map(String::as_str));
        }
    }
    for (k, (f, n, text, _)) in lines.iter().enumerate() {
        // A line left out is only asked whether it reads ([`crate::store::readable`]), and parsed only when it does not.
        // Every line that does not read comes back, reached or not — `archive_unreadable` is broken data.
        let row = match picked[k] {
            true => parsed.remove(&k).unwrap_or_else(|| crate::store::parse_line(*n, text)),
            false if crate::store::readable(text) => continue,
            false => crate::store::parse_line(*n, text),
        };
        match row {
            Ok(issue) if picked[k] => out.issues.push(issue),
            Ok(_) => {}
            Err(mut e) => {
                e.source = Some(sources[*f].0.clone());
                out.errors.push(e);
            }
        }
    }
    // A file that could not be read went in before every line above; [`read`] names it in its file's turn. The sort is
    // stable and the files were walked in path order, so this puts each one back where `read` has it.
    out.errors.sort_by(|a, b| a.source.cmp(&b.source));
    out.issues.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The ids a row names, each with every id above it — the joints [`around`] follows.
fn named_by(i: &Issue) -> Vec<String> {
    let mut out = Vec::new();
    let named = std::iter::once(i.id.as_str())
        .chain(i.epic.as_deref())
        .chain(i.milestone.as_deref())
        .chain(i.blocked_by.iter().map(String::as_str));
    named.for_each(|id| with_parents(id, &mut out));
    out
}

/// `id` and every id above it, onto `out`.
fn with_parents(id: &str, out: &mut Vec<String>) {
    out.extend(std::iter::successors(Some(id), |id| crate::id::parent_of(id)).map(str::to_string));
}

/// What one archive line names, read without parsing it into a row ([`around`]).
struct Scan {
    /// The id at the head, when the line has the one shape a write produces — `{"id":"` first, an id spelled in the
    /// charset ids are minted from, and no second `"id"` key. Only then is it the id the parse gives the row, or
    /// [`crate::id::id_of`] the unreadable line, whenever either gives one. `None` sends the line to the parser.
    head: Option<String>,
    /// Every string value that reads as an id ([`crate::id::is_valid`]) — references, the head among them — each
    /// with every id above it. Escaped strings are decoded first, so an id spelled with escapes is still seen; a
    /// title or a body that happens to read as an id only brings in more than is needed.
    ids: Vec<String>,
    /// A string value spells `milestone` — the line may be a milestone row.
    milestone: bool,
}

/// Walks the line's JSON strings by their quotes. On a line `serde_json` reads, the strings it finds are exactly the
/// line's strings; on any other line the row is unreadable, and what the walk says about it only widens what
/// [`around`] brings in.
fn scan(line: &str) -> Scan {
    let b = line.as_bytes();
    let (mut ids, mut keys, mut milestone) = (Vec::new(), 0usize, false);
    let mut k = 0;
    while k < b.len() {
        if b[k] != b'"' {
            k += 1;
            continue;
        }
        let start = k + 1;
        let (mut end, mut escaped) = (start, false);
        while end < b.len() && b[end] != b'"' {
            if b[end] == b'\\' {
                escaped = true;
                end += 1;
            }
            end += 1;
        }
        if end >= b.len() {
            break;
        }
        k = end + 1;
        let text = match escaped {
            false => std::borrow::Cow::Borrowed(&line[start..end]),
            true => match serde_json::from_str::<String>(&line[start - 1..=end]) {
                Ok(s) => std::borrow::Cow::Owned(s),
                Err(_) => continue,
            },
        };
        if line[k..].trim_start().starts_with(':') {
            keys += usize::from(text == "id");
            continue;
        }
        milestone |= text == "milestone";
        if crate::id::is_valid(&text) {
            with_parents(&text, &mut ids);
        }
    }
    let minted = |id: &&str| id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'.');
    let head = line
        .trim_start()
        .strip_prefix(r#"{"id":""#)
        .and_then(|s| s.split_once('"').map(|(id, _)| id))
        .filter(|id| keys == 1 && minted(id) && crate::id::is_valid(id))
        .map(str::to_string);
    Scan { head, ids, milestone }
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
    walks_out(active, wanted.iter().map(String::as_str), false, report::is_group)
}

/// A write's references reach past the live snapshot — `wanted`, or a parent, epic, milestone or blocker above it,
/// is an id no live row holds (moai-tzzt). Only then does the write read the archive: an epic, parent or blocker that
/// `moai archive` moved is still a row, and the board already reads it as context. Unlike [`needs_context`] a live
/// group does not count — reference checks read the row, not its members, and parsing the whole archive under the
/// lock on every `add -e` is what pushed concurrent writes past the lock timeout (moai-bth3 review).
pub fn reaches_out(active: &[Issue], wanted: &[&str]) -> bool {
    walks_out(active, wanted.iter().copied(), true, |_| false)
}

/// The walk [`needs_context`] and [`reaches_out`] share: from `wanted` up through every parent, epic and milestone
/// (and every blocker, with `blockers`), true at the first id no live row holds or the first row `stop` picks.
fn walks_out<'a>(
    active: &'a [Issue],
    wanted: impl IntoIterator<Item = &'a str>,
    blockers: bool,
    stop: impl Fn(&Issue) -> bool,
) -> bool {
    let by_id: BTreeMap<&str, &Issue> = active.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut pending: Vec<&str> = wanted.into_iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(i) = by_id.get(id) else {
            return true;
        };
        if stop(i) {
            return true;
        }
        pending.extend([crate::id::parent_of(&i.id), i.epic.as_deref(), i.milestone.as_deref()].into_iter().flatten());
        if blockers {
            pending.extend(i.blocked_by.iter().map(String::as_str));
        }
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

/// Failed guards and moves to done keep the selected row in the archive. A row changed in place — `kept` — stays live
/// even when it did not move: a row taken over has its new assignee, and a row deferred or undone (moai-b6w3) its plan, only
/// in the live snapshot, and dropping it would throw the write away while the journal and the screen report it
/// (moai-bth3 review).
pub fn finish_restoring(
    active: &mut Vec<Issue>,
    staged: &BTreeSet<String>,
    moved: &[Issue],
    kept: &BTreeSet<String>,
) -> BTreeSet<String> {
    let restored: BTreeSet<String> = moved
        .iter()
        .filter(|i| !i.status.is_done())
        .map(|i| i.id.clone())
        .chain(kept.iter().cloned())
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

    /// **`Stop` reads only the archive that reaches the live rows**(moai-i9ji) — and what the board says over it is what it
    /// says over the whole archive: the same warnings, the same id collisions. A row nothing reaches stays unparsed, and so
    /// does a broken line nothing reaches — a full parse reports that one. Each kind of line [`around`] must bring in
    /// stands here once: reached through an id above it, through a reached row's reference, through a reference spelled
    /// with escapes, through an unreadable live line's id, a duplicate inside the archive, an archived milestone row, and
    /// a line whose id is not at its head.
    #[test]
    fn around_reads_only_the_archive_that_reaches_the_live_rows() {
        let s = scratch("archive-around");
        let mut live = row("argos-l001", Kind::Issue, "todo");
        live.epic = Some("argos-e001".into());
        live.blocked_by = vec!["argos-b001".into()];
        let mut blocker = row("argos-b001", Kind::Issue, "done");
        blocker.epic = Some("argos-e002".into());
        let mut stone = row("argos-m009", Kind::Milestone, "todo");
        stone.due_on = Some("2026-01-01".into());
        let mut escaped = row("argos-x001", Kind::Issue, "done");
        escaped.epic = Some("argos-e001".into());
        let line = |i: &Issue| serde_json::to_string(i).unwrap();
        let text = [
            line(&row("argos-e001", Kind::Epic, "done")),
            line(&row("argos-e001.aaa", Kind::Issue, "done")),
            line(&blocker),
            line(&row("argos-e002", Kind::Epic, "done")),
            line(&row("argos-o001", Kind::Issue, "done")),
            line(&row("argos-u001", Kind::Issue, "done")),
            line(&row("argos-w001", Kind::Issue, "done")),
            line(&row("argos-w001", Kind::Issue, "todo")),
            line(&stone),
            line(&escaped).replace(r#""epic":"argos-e001""#, r#""epic":"argos-e\u0030\u00301""#),
            line(&row("argos-n001", Kind::Issue, "done")).replacen(r#"{"id":"#, r#"{ "id": "#, 1),
            r#"{"id":"argos-zzzz","title":"#.to_string(),
            r#"{"id":"argos-zzzy","title":"no status"}"#.to_string(),
        ];
        assert!(text[9].contains(r"\u0030"), "{}", text[9]);
        fs::write(path(&s, "2025"), text.join("\n") + "\n").unwrap();
        let unreadable = [report::Unreadable { id: Some("argos-o001") }];
        let near = around(&s, std::slice::from_ref(&live), unreadable.iter().filter_map(|u| u.id));
        let ids: Vec<&str> = near.issues.iter().map(|i| i.id.as_str()).collect();
        let want = [
            "argos-b001",
            "argos-e001",
            "argos-e001.aaa",
            "argos-e002",
            "argos-m009",
            "argos-n001",
            "argos-o001",
            "argos-w001",
            "argos-w001",
            "argos-x001",
        ];
        assert_eq!(ids, want);
        // Every unreadable line comes back, reached or not — `archive_unreadable` is a warning (moai-5y2a). The second
        // one is JSON but not a row, so only a check of the row's shape sees it.
        let all = read(&s).unwrap();
        let spots = |l: &Load| l.errors.iter().map(|e| (e.source.clone(), e.line, e.id.clone())).collect::<Vec<_>>();
        assert_eq!(all.errors.len(), 2, "{:?}", all.errors);
        assert_eq!(spots(&near), spots(&all));
        let live_ids = BTreeSet::from(["argos-l001", "argos-o001"]);
        assert_eq!(collisions(&live_ids, &near), collisions(&live_ids, &all));
        assert_eq!(collisions(&live_ids, &near), ["argos-o001", "argos-w001"]);
        let board = |archived: &[Issue]| {
            let zone = crate::tz::Zone::utc();
            let st =
                report::status_with_archive(std::slice::from_ref(&live), archived, &unreadable, &cfg(), NOW, &zone);
            serde_json::to_string(&st.warnings).unwrap()
        };
        assert_eq!(board(&near.issues), board(&all.issues));
        assert!(board(&near.issues).contains("milestone_overdue"), "{}", board(&near.issues));
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
