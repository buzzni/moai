//! Read-only supervisor utilities. No session is launched or controlled here.
use super::{Ctx, Fail, R};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::collections::BTreeSet;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor {
    ts: String,
    seen: BTreeSet<String>,
}

pub fn watch(ctx: &Ctx, id: &str, since: &str, cursor: Option<&str>, heads: &[String]) -> R<Vec<String>> {
    if !crate::id::is_valid(id) || heads.is_empty() || heads.iter().any(String::is_empty) {
        return Err(Fail::new("watch needs a valid id and nonempty note heads"));
    }
    let mut cursor = match cursor {
        Some(encoded) => {
            let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| Fail::new("invalid watch cursor"))?;
            serde_json::from_slice::<Cursor>(&bytes).map_err(|_| Fail::new("invalid watch cursor"))?
        }
        None => Cursor { ts: since.to_owned(), seen: BTreeSet::new() },
    };
    let stamp = cursor.ts.as_bytes();
    if stamp.len() != 20
        || [4, 7, 10, 13, 16, 19].iter().zip(b"--T::Z").any(|(&i, &b)| stamp[i] != b)
        || stamp.iter().enumerate().any(|(i, b)| ![4, 7, 10, 13, 16, 19].contains(&i) && !b.is_ascii_digit())
    {
        return Err(Fail::new("watch requires a UTC timestamp YYYY-MM-DDTHH:MM:SSZ"));
    }
    let repo = super::open_repo(ctx)?;
    if !repo.read()?.issues.iter().any(|issue| issue.id == id) {
        return Err(Fail::not_found(id, ctx.lang()));
    }
    loop {
        let notes = repo.journal_of(id);
        if !crate::store::journal_unread().is_empty() {
            return Err(Fail::new("watch could not read the journal"));
        }
        for note in notes {
            if note.ts < cursor.ts {
                continue;
            }
            let key = serde_json::to_string(&note).map_err(|e| Fail::new(e.to_string()))?;
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

#[allow(clippy::too_many_arguments)]
pub fn message(
    ctx: &Ctx,
    id: &str,
    model: &str,
    effort: &str,
    base: &str,
    root_branch: &str,
    from: &str,
    alongside: &str,
    milestone: &str,
    writing: &str,
    person: &str,
) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let rows = repo.read()?.issues;
    let row = rows.iter().find(|r| r.id == id).ok_or_else(|| Fail::not_found(id, ctx.lang()))?;
    let root = repo.root.display();
    let path = format!("{root}/.claude/moai-plugin/skills/moai-supervise/references/worker-codex.md");
    let text = format!(
        "You are handed epic {id} — {}. It is already unfolded; do not promote.\nRead {path} and follow its steps from step 2.\nRead first: moai show {id}\nModel: {model}\nEffort: {effort}\nWorker: codex\nSubagents: none\nWriting: {writing}\nWork running alongside: {alongside}\nRoot branch: {root_branch}\nBase branch: {base}\nMilestone: {milestone}\nDue: {}\nRoot: {root}\nPerson: {person}\nWorktree: aim every command at the worktree absolute path (workdir or cd) — never edit the root checkout. Link the worktree target to /tmp/cargo-target/<name> before the first build.\nfrom: {from}",
        row.title,
        row.due_on.as_deref().unwrap_or("none")
    );
    if ctx.json { super::json_line(&serde_json::json!({"message": text})) } else { Ok(vec![text]) }
}
