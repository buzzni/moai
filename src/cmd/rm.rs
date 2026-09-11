//! 지운다. 에이전트가 잘못 만든 것을 치우는 길이다.
//!
//! 자식이나 에픽 멤버가 남아 끊긴 참조가 되는 것은 **막지 않고 알린다.**
//! `moai status` 가 끊긴 참조를 드러내므로 여기서 막을 이유가 없다.

use super::{Ctx, R};
use crate::cli::RmArgs;
use crate::model::{self, Issue, JournalEntry};
use crate::store::Repo;
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: RmArgs) -> R<Vec<String>> {
    let at = model::now();
    let by = model::actor();
    let repo = Repo::discover()?;

    let (gone, missing, dangling): (Vec<Issue>, Vec<String>, Vec<String>) =
        repo.with_write(|issues, _| {
            let (mut gone, mut missing, mut dangling) = (Vec::new(), Vec::new(), Vec::new());
            let mut entries = Vec::new();
            for id in &args.ids {
                let Some(at_idx) = issues.iter().position(|i| &i.id == id) else {
                    missing.push(id.clone());
                    continue;
                };
                let i = issues.remove(at_idx);
                entries.push(JournalEntry::removed(&i.id, &i.title, &at, &by));
                gone.push(i);
            }
            for i in issues.iter() {
                let orphan = crate::id::parent_of(&i.id).is_some_and(|p| gone.iter().any(|g| g.id == p));
                let lost = i.epic.as_deref().is_some_and(|e| gone.iter().any(|g| g.id == e));
                if orphan || lost {
                    dangling.push(i.id.clone());
                }
            }
            Ok((entries, (gone, missing, dangling)))
        })?;

    for id in &missing {
        super::note_partial();
        eprintln!("moai: {id} 를 못 찾았다");
    }
    if !dangling.is_empty() {
        eprintln!(
            "moai: 끊긴 참조가 {}건 남았다 — {}",
            dangling.len(),
            dangling.join(" ")
        );
    }

    if ctx.json {
        return super::json_line(&gone);
    }
    Ok(gone
        .iter()
        .map(|i| format!("{}  {}", paint(style::ID, &i.id), paint(style::DIM, &i.title)))
        .collect())
}
