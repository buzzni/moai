//! 읽었다고 표시한다(moai-u8oh).
//!
//! **트래커에 안 쓴다.** 읽음은 사람마다 다른 값이라 `.moai/issues.jsonl` 에 적으면 읽기만 해도
//! 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다. 내 설정의 `[read]` 표에 **이슈 id → 지금**을
//! 적는다(사용자 결정 2026-09-15) — 그 뒤에 줄이 바뀌면 다시 안 읽음이 된다.
//!
//! **읽음은 시키는 때만 선다.** `show` 로 열었다고, 탐색기에서 커서가 지나갔다고 서지 않는다 —
//! 스치듯 지나간 것을 읽었다고 적으면 이 표시가 곧 아무 말도 안 하게 된다.

use super::{Ctx, Fail, R};
use crate::cli::ReadArgs;
use crate::store::Repo;
use crate::style::{self, paint};
use std::collections::BTreeMap;

pub fn run(ctx: &Ctx, args: ReadArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    let now = crate::model::now();
    let me = crate::model::actor(ctx.user.as_deref(), &repo.root)?;
    let Some(path) = crate::user_config::path() else {
        return Err(Fail::new(
            "읽음을 적을 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 이 다 없다".to_string(),
        ));
    };

    let seen = crate::user_config::read(Some(&path)).read;
    let mut want: Vec<String> = args.ids.clone();
    // `--all` 은 **안 읽은 것만** 센다 — 이미 읽은 줄까지 다시 적으면 헛 쓰기가 되고, 그 줄의
    // "언제 봤나" 가 오늘로 밀려 그 뒤에 바뀐 것을 못 가른다.
    if args.all {
        want.extend(crate::query::unread(&load.issues, &format!("{} ({})", me.name, me.email), &seen).into_iter().map(str::to_string));
    }
    // `-e <에픽>` 은 그 에픽의 멤버와 그 밑까지 — 목록에서 `SPC r r` 이 하는 것과 같은 자다.
    if let Some(epic) = &args.epic {
        want.extend(under(&load.issues, epic));
    }
    let missing: Vec<String> = want.iter().filter(|id| load.get(id).is_none()).cloned().collect();
    let marks: BTreeMap<String, String> =
        want.iter().filter(|id| load.get(id).is_some()).map(|id| (id.clone(), now.clone())).collect();
    let fresh: Vec<String> = marks.keys().filter(|id| seen.get(*id) != Some(&now)).cloned().collect();

    crate::user_config::update(&path, |doc| doc.mark_read(&marks))?;

    // **없는 줄은 `--json` 에서도 말한다** — 기계로 읽는 쪽은 `missing` 으로, 사람은 stderr 로.
    // 하나가 없다고 나머지를 안 적지 않는다(`defer` 와 같은 자리다).
    for id in &missing {
        eprintln!("`{id}` 라는 줄이 없다");
    }
    if ctx.json {
        return super::json_line(&Marked { read: &fresh, missing: &missing, at: &now });
    }
    if fresh.is_empty() {
        return Ok(vec!["읽음으로 적을 것이 없다".to_string()]);
    }
    Ok(fresh
        .iter()
        .map(|id| format!("{}  {}", paint(style::ID, id), paint(style::DIM, "읽음")))
        .collect())
}

/// 그 묶음에 딸린 것 전부 — 멤버와 그 자식. 묶음 자신도 읽은 것으로 친다.
fn under(issues: &[crate::model::Issue], epic: &str) -> Vec<String> {
    issues
        .iter()
        .filter(|i| {
            i.id == epic
                || i.epic.as_deref() == Some(epic)
                || std::iter::successors(crate::id::parent_of(&i.id), |id| crate::id::parent_of(id)).any(|p| p == epic)
        })
        .map(|i| i.id.clone())
        .collect()
}

#[derive(serde::Serialize)]
struct Marked<'a> {
    read: &'a [String],
    missing: &'a [String],
    at: &'a str,
}
