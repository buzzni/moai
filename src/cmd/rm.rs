//! 지운다. 에이전트가 잘못 만든 것을 치우는 길이다.
//!
//! 자식이나 에픽 멤버가 남아 끊긴 참조가 되는 것은 **막지 않고 알린다.**
//! `moai status` 가 끊긴 참조를 드러내므로 여기서 막을 이유가 없다.

use super::{Ctx, R};
use crate::cli::RmArgs;
use crate::model::{self, Issue, JournalEntry};
use crate::style::{self, paint};

pub fn run(ctx: &Ctx, args: RmArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx.lang())?;
    let at = model::now();
    let by = model::actor(ctx.user.as_deref(), &repo.root)?;

    let (gone, missing, dangling): (Vec<Issue>, Vec<String>, Vec<String>) = repo.with_write(|issues, _, _| {
        let (mut gone, mut missing, mut dangling) = (Vec::new(), Vec::new(), Vec::new());
        let mut entries = Vec::new();
        for id in &args.ids {
            // 같은 id 를 두 번 적은 것은 실패가 아니다. 인자 목록은 glob·
            // xargs·에이전트가 짓는 것이라 중복이 흔하고, 지워 놓고
            // "못 찾았다" 며 비영으로 끝내면 부르는 쪽이 되돌리려 든다.
            if gone.iter().any(|g: &Issue| &g.id == id) {
                continue;
            }
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
            let unblocked = i.blocked_by.iter().any(|b| gone.iter().any(|g| &g.id == b));
            if orphan || lost || unblocked {
                dangling.push(i.id.clone());
            }
        }
        Ok((entries, (gone, missing, dangling)))
    })?;

    for id in &missing {
        super::note_partial();
        // 없는 id 는 **어느 명령에서나 한 낱말이다**([`Fail::not_found`], moai-95g1) — 리뷰가
        // 넷째 사본으로 짚은 자리다. 나머지 `rm` 몸통은 아직 한국어다(idea moai-a7qo).
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.not_found"), &[("id", id)]));
    }
    if !dangling.is_empty() {
        eprintln!("moai: 끊긴 참조가 {}건 남았다 — {}", dangling.len(), dangling.join(" "));
    }

    if ctx.json {
        // 끊긴 참조는 지운 쪽이 알아야 할 결과다. 사람에게만 말하고 기계에는
        // 안 말하면, 그 뒤처리를 할 쪽이 바로 그 기계다.
        #[derive(serde::Serialize)]
        struct Out<'a> {
            removed: &'a [Issue],
            missing: &'a [String],
            dangling: &'a [String],
        }
        return super::json_line(&Out { removed: &gone, missing: &missing, dangling: &dangling });
    }
    Ok(gone.iter().map(|i| format!("{}  {}", paint(style::ID, &i.id), paint(style::DIM, &i.title))).collect())
}
