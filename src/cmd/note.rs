//! 이슈에 메모를 남긴다. 스냅샷을 바꾸지 않고 저널에만 쌓인다.
//!
//! 게이트를 없앤 자리를 실제로 메우는 것이 이것이다 — 다음 에이전트가 읽을
//! 발견사항을 남기는 채널이다. 승인을 구하는 것이 아니라 알리는 것이다.

use super::{Ctx, Fail, R};
use crate::cli::NoteArgs;
use crate::model::{self, JournalEntry};
use crate::store::Repo;
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: NoteArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let text = args.text.trim().to_string();
    if text.is_empty() {
        return Err(Fail::new("메모가 비었다"));
    }
    let at = model::now();
    let by = model::actor();

    repo.with_write(|issues, _| {
        if !issues.iter().any(|i| i.id == args.id) {
            return Err(format!("{} 를 못 찾았다", args.id));
        }
        Ok((vec![JournalEntry::note(&args.id, &text, &at, &by)], ()))
    })?;

    if ctx.json {
        return super::json_line(
            &serde_json::json!({"id": args.id, "ts": at, "text": text, "by": by}),
        );
    }
    Ok(vec![format!(
        "{}  {}",
        paint(style::ID, &args.id),
        paint(style::DIM, &format!("note: {text}"))
    )])
}
