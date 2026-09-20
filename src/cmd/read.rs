//! 읽었다고 표시한다(moai-u8oh).
//!
//! **트래커에 안 쓴다.** 읽음은 사람마다 다른 값이라 `.moai/issues.jsonl` 에 적으면 읽기만 해도
//! 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다. 적는 자리는 **그 저장소의 읽음 파일**
//! ([`crate::read_marks`], moai-omx7)이고, 적는 값은 **이슈 id → 본 줄의 `updated_at`**
//! (사용자 결정 2026-09-15, 값은 2026-09-19 에 본 때에서 바꿨다 — moai-lyc1) — 그 뒤에 줄이 바뀌면
//! 다시 안 읽음이 된다. 설정의 옛 `[read]` 는 겹쳐 보기만 하고 다시 안 적는다(사용자 결정 3).
//!
//! **읽음은 시키는 때만 선다.** `show` 로 열었다고, 탐색기에서 커서가 지나갔다고 서지 않는다 —
//! 스치듯 지나간 것을 읽었다고 적으면 이 표시가 곧 아무 말도 안 하게 된다.

use super::{Ctx, Fail, R};
use crate::cli::ReadArgs;
use crate::store::Repo;
use crate::style::{self, paint};
use std::collections::BTreeSet;

pub fn run(ctx: &Ctx, args: ReadArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    let now = crate::model::now();
    let path = super::project::writable_config()?;

    // 있는 id 는 **한 번 모아 견준다**(moai-j038.vna) — `Load::get` 은 줄 전부를 뒤에서부터 훑으므로
    // 받은 id 마다 부르면 `--all`·`-e` 가 줄 수의 제곱으로 느려진다(1만 줄에 안 읽은 5천이면 1초 가까이).
    let known: BTreeSet<&str> = load.issues.iter().map(|i| i.id.as_str()).collect();
    let mut want: Vec<String> = args.ids.clone();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    // `--all` 은 **내게 온 것 가운데** 안 읽은 것이다.
    //
    // **누군지는 여기서만 묻는다**(moai-u8oh.x85). 담당을 재는 것은 `--all` 뿐이고, id 를 받은
    // 길은 사람을 몰라도 제 설정에 적을 수 있다 — 저널에 안 쓰니 이름 없는 줄이 남을 자리도
    // 없다. 위에서 먼저 풀면 git 설정 없는 기계에서 `moai read <id>` 가 통째로 넘어졌다. 탐색기의
    // `r` 도 같은 자다 — 누군지 몰라도, 내게 온 줄이 아니어도 그 줄을 적는다.
    if args.all {
        let me = crate::model::actor(ctx.user.as_deref(), &repo.root)?;
        let me = crate::model::label(&me.name, Some(&me.email), crate::config::Naming::Full);
        // 적어 둔 읽음은 **여기서만** 든다 — 안 읽은 줄을 가르는 것은 `--all` 뿐이다. 읽음은 이 저장소의
        // 제 파일에 살고, 옛 `[read]` 는 겹쳐 본다(moai-omx7, 사용자 결정 2026-09-19).
        let legacy = crate::user_config::read(Some(&path)).read;
        let (seen, _) = crate::read_marks::read(&path, &repo.root, &legacy);
        want.extend(crate::query::unread(&load.issues, &me, &seen).into_iter().map(str::to_string));
    }
    // `-e <묶음>` 은 그 묶음 줄과 **그 밑에 그려진 것 전부** — 목록에서 `SPC m r` 이 부르는 것과 같은
    // 자(`nav::Index::under_group`)다. **없는 묶음은 말한다** — 조용히 빈 손으로 끝나면 사람은 오타를
    // 친 줄 모르고 다 적힌 줄 안다. **묶음이 아닌 줄은 적기 전에 거절한다** — 이슈를 주면 그 줄 하나만
    // 적고 0 으로 끝나, "그 밑까지" 를 시킨 사람은 다 적힌 줄 안다.
    if let Some(group) = &args.epic {
        match load.get(group) {
            None => {
                missing.insert(group.clone());
            }
            Some(g) if !crate::report::is_group(g) => {
                return Err(Fail::coded(
                    format!("`-e` 는 에픽·마일스톤을 받는다 — {group} 는 {} 다", g.kind.as_str()),
                    super::code::BAD_INPUT,
                ));
            }
            Some(_) => {
                let index = crate::nav::Index::of(&load.issues);
                want.extend(index.under_group(&load.issues, group).into_iter().map(|at| load.issues[at].id.clone()));
            }
        }
    }
    missing.extend(want.iter().filter(|id| !known.contains(id.as_str())).cloned());
    let targets: BTreeSet<&str> = want.iter().map(String::as_str).filter(|id| known.contains(id)).collect();

    // **걷을 때는 못 읽은 줄이 쓰는 id 도 지킨다**(리뷰) — `load.issues` 는 *이 바이너리가 이번에 읽어
    // 낸* 줄이지 트래커가 든 줄이 아니다. 한 줄이 깨진 동안 부른 `moai read` 하나가 그 이슈의 읽음을
    // 걷으면, 줄을 고친 뒤 [NEW] 가 되살아난다 — 조용한 손실이다. 그 id 를 아는 자는 이미 있다
    // ([`crate::store::Load::reserved_ids`]). **`missing` 은 그대로 `known` 으로 가른다** — 못 읽는 줄의
    // id 를 받아 놓고 적을 줄이 없으면 읽었다고도 못 찾았다고도 안 하는 것이 더 나쁘다.
    let reserved = load.reserved_ids();
    let keep: BTreeSet<&str> = known.iter().copied().chain(reserved.iter().map(String::as_str)).collect();

    // 적을 것이 없으면 읽음 파일에 손을 안 댄다 — 빈 쓰기 하나 때문에 설정 디렉터리와 락 파일이
    // 아직 아무것도 등록하지 않은 사람의 집에 생긴다.
    //
    // **이미 읽은 줄은 다시 안 적는다**(moai-j038.vna) — 본 뒤로 안 바뀐 줄을 다시 적으면 헛 쓰기고,
    // 옆 탐색기가 방금 적은 새 도장을 이 명령이 든 낡은 줄의 도장으로 덮을 수도 있다. 가르는 것은 **락
    // 안에서 읽은 표**다 — 락 밖에서 읽어 두면 그사이 옆에서 적은 것과 어긋난다. 탐색기의 `r`·`SPC m`
    // 도 같은 자로 가른다. 같은 id 의 줄은 **다 넘긴다** — 쌍둥이 가운데 늦은 도장을 적어야 [NEW] 가
    // 내린다(`query::read_marks_of`).
    let fresh: Vec<String> = if targets.is_empty() {
        Vec::new()
    } else {
        crate::read_marks::update(&path, &repo.root, |sheet| {
            let lines = load.issues.iter().filter(|i| targets.contains(i.id.as_str()));
            let marks = crate::query::read_marks_of(lines, &sheet.marks().0);
            let wrote = sheet.mark(&marks)?;
            // **트래커에 없는 id 를 여기서 걷는다**(moai-dt5q, 사용자 결정 2) — 이 자리는 트래커를 이미
            // 들고 있다. 닫힌 줄은 안 걷는다: 걷으면 그 줄이 다시 설 때 [NEW] 가 되살아나, 읽음의 뜻이
            // "본 적 있다" 에서 "최근에 본 적 있다" 로 바뀐다.
            sheet.prune(&keep);
            Ok(wrote)
        })?
    };

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

#[derive(serde::Serialize)]
struct Marked<'a> {
    read: &'a [String],
    missing: &'a [String],
    /// 이 명령이 돈 때다. `[read]` 에 적힌 값이 아니다 — 그것은 줄마다 본 줄의 `updated_at` 이다(moai-lyc1).
    at: &'a str,
}
