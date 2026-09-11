//! 이슈에 메모를 남긴다. 내용은 저널에만 쌓인다.
//!
//! 스냅샷에 **메모를 적지는 않지만**, 다른 쓰기와 같은 길을 지나므로 파일이
//! 표준형이 아니었다면(손으로 고쳤거나 머지가 남긴 모양) 그때 표준형으로
//! 맞춰진다. 파일이 한 가지 모양만 갖게 하려고 일부러 그렇게 둔다.
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
    let entry = JournalEntry::note(&args.id, &text, &at, &by);

    repo.with_write(|issues, _| {
        if !issues.iter().any(|i| i.id == args.id) {
            return Err(format!("{} 를 못 찾았다", args.id));
        }
        Ok((vec![entry.clone()], ()))
    })?;

    // 저널에 적은 **그 줄을 그대로** 낸다. `serde_json::json!` 은 `Value` 를
    // 거치고 `Value` 의 맵은 알파벳 순이라, 파일과 `--json` 이 서로 다른
    // 순서를 말하게 된다 (`json_line` 의 설명 참조). `kind` 도 그때 사라진다.
    if ctx.json {
        return super::json_line(&entry);
    }
    // 여러 줄 메모도 한 줄에 하나씩 낸다 — 원소 하나가 한 줄이라는 약속.
    Ok(text
        .split('\n')
        .enumerate()
        .map(|(n, l)| match n {
            0 => format!("{}  {}", paint(style::ID, &args.id), paint(style::DIM, &format!("note: {l}"))),
            _ => format!("{}  {}", " ".repeat(args.id.chars().count()), paint(style::DIM, l)),
        })
        .collect())
}
