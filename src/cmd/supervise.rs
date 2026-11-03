//! Read-only supervisor utilities. No session is launched or controlled here.
use super::{Ctx, Fail, R};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::collections::BTreeSet;

/// Where a watch resumes: the journal second it stopped in and the lines of that second it
/// already handled. `id` binds the cursor to the row it was read from — a cursor carried over to
/// another row would skip that row's notes up to a second that means nothing there.
///
/// **A line is held by its digest, not its text.** The cursor goes back on the command line and
/// through the supervisor's context on every event; a multi-kilobyte report written in that
/// second would ride along each time and can outgrow one argument (`E2BIG`).
#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor {
    id: String,
    ts: String,
    seen: BTreeSet<String>,
}

/// One journal line's identity within a second — identical lines are one event.
fn digest(note: &crate::model::JournalEntry) -> R<String> {
    let line = serde_json::to_vec(note).map_err(|e| Fail::new(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(&ring::digest::digest(&ring::digest::SHA256, &line).as_ref()[..16]))
}

pub fn watch(ctx: &Ctx, id: &str, since: &str, cursor: Option<&str>, heads: &[String]) -> R<Vec<String>> {
    if !crate::id::is_valid(id) || heads.is_empty() || heads.iter().any(String::is_empty) {
        return Err(Fail::new("watch needs a valid id and nonempty note heads"));
    }
    let mut cursor = match cursor {
        Some(encoded) => {
            let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| Fail::new("invalid watch cursor"))?;
            let cursor = serde_json::from_slice::<Cursor>(&bytes).map_err(|_| Fail::new("invalid watch cursor"))?;
            if cursor.id != id {
                return Err(Fail::new(format!("this watch cursor belongs to {}, not {id}", cursor.id)));
            }
            cursor
        }
        None => Cursor { id: id.to_owned(), ts: since.to_owned(), seen: BTreeSet::new() },
    };
    if crate::model::parse_rfc3339(&cursor.ts).is_none() {
        return Err(Fail::new("watch requires a UTC timestamp YYYY-MM-DDTHH:MM:SSZ"));
    }
    let repo = super::open_repo(ctx)?;
    if !repo.read()?.issues.iter().any(|issue| issue.id == id) {
        return Err(Fail::not_found(id, ctx.lang()));
    }
    let want = BTreeSet::from([id]);
    loop {
        // Parse only the lines that can name this id (or hide it behind a `\u` escape) — the journal
        // is read again every two seconds, and decoding all of it each time is most of the work.
        let notes =
            repo.journal_by_id(&want, |line| line.contains(id) || line.contains("\\u")).remove(id).unwrap_or_default();
        if !crate::store::journal_unread().is_empty() {
            return Err(Fail::new("watch could not read the journal"));
        }
        for note in notes {
            if note.ts < cursor.ts {
                continue;
            }
            let key = digest(&note)?;
            if note.ts == cursor.ts && cursor.seen.contains(&key) {
                continue;
            }
            if note.ts > cursor.ts {
                cursor.ts.clone_from(&note.ts);
                cursor.seen.clear();
            }
            cursor.seen.insert(key);
            let head = note.text.as_deref().unwrap_or("").lines().next().unwrap_or("");
            if note.kind == "note"
                && heads.iter().any(|prefix| {
                    head.starts_with(prefix) || (prefix == "진행 중 결정이 필요합니다" && head.contains(prefix))
                })
            {
                let encoded =
                    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).map_err(|e| Fail::new(e.to_string()))?);
                return super::json_line(
                    &serde_json::json!({"id": id, "ts": note.ts, "head": head, "cursor": encoded}),
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// The session fields the supervisor supplies; the row gives the rest.
pub struct Fields<'a> {
    pub model: &'a str,
    pub effort: &'a str,
    pub base: &'a str,
    pub root_branch: &'a str,
    pub from: &'a str,
    pub steps: Option<&'a std::path::Path>,
    pub alongside: &'a str,
    pub milestone: &'a str,
    pub writing: Option<&'a str>,
    pub person: &'a str,
    pub subdir: Option<&'a str>,
}

/// The Codex assignment, in the slots of `guide::message`. The first line is the one the supervisor
/// sends a Claude worker — an epic from step 2, a backlog item from step 1 — with the steps file in
/// it, so the two assignments never point at different steps.
pub fn message(ctx: &Ctx, id: &str, f: &Fields) -> R<Vec<String>> {
    use crate::model::Kind;
    let repo = super::open_repo(ctx)?;
    let rows = repo.read()?.issues;
    let row = rows.iter().find(|r| r.id == id).ok_or_else(|| Fail::not_found(id, ctx.lang()))?;
    let root = repo.root.display().to_string();
    // **The steps file is where the skill was planted.** A plugin installed at user scope lives in
    // Claude Code's plugin cache, not under the root — a path that does not exist sends Codex to read
    // nothing, so it is refused here, naming the flag that fixes it.
    let steps = match f.steps {
        Some(path) => path.to_path_buf(),
        None => repo.root.join(crate::skill::DIR).join("skills/moai-supervise/references/worker-codex.md"),
    };
    if !steps.is_file() {
        return Err(Fail::new(format!(
            "no steps file at {} — pass --steps <the moai-supervise skill's references/worker-codex.md>",
            steps.display()
        )));
    }
    let steps = std::path::absolute(&steps).unwrap_or(steps).display().to_string();
    let first = match row.kind {
        Kind::Epic => crate::guide::PLAN_FIRST.replace("<epic>", id),
        Kind::Backlog => crate::guide::BACKLOG_FIRST.replace("<id>", id),
        other => {
            return Err(Fail::new(format!(
                "supervise message takes an epic or a backlog item — {id} is {}",
                other.as_str()
            )));
        }
    }
    .replace("<steps file>", &steps)
    .replace("<title>", &row.title);
    // `Writing:` is the person's value, never one this command picks (`moai writing`).
    let writing = match f.writing {
        Some(w) => w.to_owned(),
        None => {
            ctx.registry().writing.with_repo(&repo.config.writing).0.values.get("detail").cloned().unwrap_or_default()
        }
    };
    let subdir = f.subdir.map(|s| format!("Subdir: {s}\n")).unwrap_or_default();
    let beside = crate::guide::BESIDE.replace("<other work>", f.alongside);
    // Validate only here: a bad key or value in this optional table must not disable unrelated
    // commands. (A line the file parser itself cannot read — a multi-line array, an unbalanced
    // quote — still stops every command, like any malformed line in this file.)
    // **A misspelt key is refused, not skipped** — skipping it drops the repository's instruction
    // without a word, which is the very miss this setting exists to prevent (moai-n6fs).
    let mut extra = String::new();
    let mut written = None;
    for entry in &repo.config.supervise {
        if entry.key != "extra_lines" {
            return Err(Fail::new(format!(
                "[supervise] {} on line {} is not a setting — the table takes only extra_lines",
                entry.key, entry.line
            )));
        }
        if let Some(first) = written.replace(entry.line) {
            return Err(Fail::new(format!(
                "[supervise] extra_lines is written twice, on lines {first} and {}",
                entry.line
            )));
        }
        let lines: Vec<String> = serde_json::from_str(&entry.raw).map_err(|_| {
            Fail::new(format!(
                "[supervise] extra_lines on line {} must be a single-line array of double-quoted strings",
                entry.line
            ))
        })?;
        // One line each, and nothing a terminal acts on: the message is pasted into a worker's
        // pane, where an escape sequence could end the bracketed paste and submit half of it.
        if lines.iter().any(|line| line.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))) {
            return Err(Fail::new(format!(
                "[supervise] extra_lines on line {}: each entry must be one line without control characters",
                entry.line
            )));
        }
        extra =
            lines.into_iter().map(|line| format!("{}\n", line.replace("<id>", id).replace("<root>", &root))).collect();
    }
    let text = format!(
        "{first}\nRead first: moai show {id}\nModel: {}\nEffort: {}\nWorker: codex\nSubagents: none\nWriting: {writing}\n{beside}\nRoot branch: {}\nBase branch: {}\nMilestone: {}\nDue: {}\nRoot: {root}\n{subdir}Person: {}\nWorktree: aim every command at the worktree absolute path (workdir or cd) — never edit the root checkout.\n{extra}from: {}",
        f.model,
        f.effort,
        f.root_branch,
        f.base,
        f.milestone,
        row.due_on.as_deref().unwrap_or("none"),
        f.person,
        f.from,
    );
    if ctx.json { super::json_line(&serde_json::json!({"message": text})) } else { Ok(vec![text]) }
}
