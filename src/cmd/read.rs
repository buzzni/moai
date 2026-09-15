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
use std::collections::{BTreeMap, BTreeSet};

pub fn run(ctx: &Ctx, args: ReadArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    let now = crate::model::now();
    let Some(path) = crate::user_config::path() else {
        return Err(Fail::new(
            "읽음을 적을 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 이 다 없다".to_string(),
        ));
    };

    let seen = crate::user_config::read(Some(&path)).read;
    let mut want: Vec<String> = args.ids.clone();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    // `--all` 은 **안 읽은 것만** 센다 — 이미 읽은 줄까지 다시 적으면 헛 쓰기가 되고, 그 줄의
    // "언제 봤나" 가 오늘로 밀려 그 뒤에 바뀐 것을 못 가른다.
    //
    // **누군지는 여기서만 묻는다**(moai-u8cs 리뷰). 담당을 재는 것은 `--all` 뿐이고, id 를 받은
    // 길은 사람을 몰라도 제 설정에 적을 수 있다 — 저널에 안 쓰니 이름 없는 줄이 남을 자리도
    // 없다. 위에서 먼저 풀면 git 설정 없는 기계에서 `moai read <id>` 가 통째로 넘어졌고,
    // 같은 일을 하는 탐색기의 `r` 은(`App::me` 가 `Option`) 그대로 돌아 둘이 어긋났다.
    if args.all {
        let me = crate::model::actor(ctx.user.as_deref(), &repo.root)?;
        let me = format!("{} ({})", me.name, me.email);
        want.extend(crate::query::unread(&load.issues, &me, &seen).into_iter().map(str::to_string));
    }
    // `-e <에픽>` 은 그 에픽의 멤버와 그 밑까지 — 목록에서 `SPC m r` 이 하는 것과 같은 자다.
    // **없는 묶음은 말한다** — 조용히 빈 손으로 끝나면 사람은 오타를 친 줄 모르고 다 적힌 줄 안다.
    if let Some(group) = &args.epic {
        if load.get(group).is_none() {
            missing.insert(group.clone());
        }
        want.extend(under(&load.issues, group));
    }
    missing.extend(want.iter().filter(|id| load.get(id).is_none()).cloned());
    let marks: BTreeMap<String, String> =
        want.iter().filter(|id| load.get(id).is_some()).map(|id| (id.clone(), now.clone())).collect();
    let fresh: Vec<String> = marks.keys().filter(|id| seen.get(*id) != Some(&now)).cloned().collect();

    // 적을 것이 없으면 설정 파일에 손을 안 댄다 — 빈 쓰기 하나 때문에 설정 디렉터리와 락 파일이
    // 아직 아무것도 등록하지 않은 사람의 집에 생긴다.
    if !marks.is_empty() {
        crate::user_config::update(&path, |doc| doc.mark_read(&marks))?;
    }

    // **없는 줄은 `--json` 에서도 말한다** — 기계로 읽는 쪽은 `missing` 으로, 사람은 stderr 로.
    // 하나가 없다고 나머지를 안 적지 않되, **비영으로 끝난다**(#a-partial) — `mv`·`defer`·`rm` 과
    // 같은 자리다. 0 으로 끝나면 고리를 짜는 쪽이 오타 친 id 를 적힌 것으로 세고 넘어간다.
    let missing: Vec<String> = missing.into_iter().collect();
    for id in &missing {
        super::note_partial();
        eprintln!("moai: {id} 를 못 찾았다");
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

/// 그 묶음에 딸린 것 전부 — 멤버와 그 밑. 묶음 자신도 읽은 것으로 친다.
///
/// **소속은 `report::groups`·`report::milestones` 에 묻는다**(moai-u8cs 리뷰) — 줄의 `epic` 필드를
/// 손으로 보면 *물려받은* 소속이 통째로 빠진다. `moai add --parent <멤버>` 로 선 자식은 제
/// `epic` 을 안 적고 부모에게서 받으므로(`epic_through`), 필드만 보던 때는 `-e <에픽>` 이
/// "그 밑까지" 를 못 지켜 멤버의 자식·리뷰가 안 읽은 채 남았다. 마일스톤도 같은 자리였다 —
/// `-e <마일스톤>` 이 묶음 줄 하나만 적고 멤버를 다 흘렸다. `epic_from_parent` 가 적어 둔
/// 그대로다: *소속을 따로 재면 둘은 언젠가 어긋난다.*
///
/// id 조상은 그래도 따로 훑는다 — `--parent <묶음>` 으로 선 줄 가운데 소속을 안 받는 것
/// (에픽 밑의 에픽 등, `report::joins`)이 있고, 그것도 이 묶음 밑에 그려진다.
fn under(issues: &[crate::model::Issue], group: &str) -> Vec<String> {
    let epics = crate::report::groups(issues);
    let milestones = crate::report::milestones(issues);
    issues
        .iter()
        .filter(|i| {
            let id = i.id.as_str();
            id == group
                || epics.get(id) == Some(&group)
                || milestones.get(id) == Some(&group)
                || std::iter::successors(crate::id::parent_of(id), |id| crate::id::parent_of(id)).any(|p| p == group)
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
