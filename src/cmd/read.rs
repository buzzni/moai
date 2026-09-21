//! 읽었다고 표시한다(moai-u8oh).
//!
//! **트래커에 안 쓴다.** 읽음은 사람마다 다른 값이라 `.moai/issues.jsonl` 에 적으면 읽기만 해도
//! 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다. 적는 자리는 **그 저장소의 읽음 파일**
//! ([`crate::read_marks`], moai-omx7)이고, 적는 값은 **이슈 id → 본 줄의 `updated_at`**
//! (사용자 결정 2026-09-15, 값은 2026-09-19 에 본 때에서 바꿨다 — moai-lyc1) — 그 뒤에 줄이 바뀌면
//! 다시 안 읽음이 된다. 설정의 옛 `[read]` 는 겹쳐 보기만 하고 다시 안 적는다(사용자 결정 3) —
//! **걷지도 않는다**(moai-jlc8, 2026-09-21 사용자 결정). 안 걷는 값과 그 저울은
//! [`crate::read_marks::read`] 에 적어 두었다.
//!
//! **읽음은 시키는 때만 선다.** `show` 로 열었다고, 탐색기에서 커서가 지나갔다고 서지 않는다 —
//! 스치듯 지나간 것을 읽었다고 적으면 이 표시가 곧 아무 말도 안 하게 된다.

use super::{Ctx, Fail, R};
use crate::cli::ReadArgs;
use crate::store::Repo;
use crate::style::{self, paint};
use std::collections::BTreeSet;

pub fn run(ctx: &Ctx, args: ReadArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let load = repo.read()?;
    let now = crate::model::now();
    let path = super::project::writable_config(ctx)?;

    // 있는 id 는 **한 번 모아 견준다**(moai-j038.vna) — `Load::get` 은 줄 전부를 뒤에서부터 훑으므로
    // 받은 id 마다 부르면 `--all`·`-e` 가 줄 수의 제곱으로 느려진다(1만 줄에 안 읽은 5천이면 1초 가까이).
    let known: BTreeSet<&str> = load.issues.iter().map(|i| i.id.as_str()).collect();
    let mut want: Vec<String> = args.ids.clone();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    // **같은 까닭을 두 번 대지 않는다**(리뷰). `--all` 은 읽는 길과 쓰는 길을 한 판에 지나는데, 둘 다
    // 같은 `place_of` 의 한 줄을 물고 온다 — 그대로 내던 판은 한 번 난 탈을 두 줄로 세워, 사람은 두 번
    // 났다고 읽고 로그를 세는 쪽은 두 건으로 센다.
    let mut said: BTreeSet<String> = BTreeSet::new();
    // 시킨 id 가운데 **사람이 적은 값에 막혀 못 적은 것**(moai-l5ue). 글자 차례로 낸다.
    let mut held: BTreeSet<String> = BTreeSet::new();
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
        // **설정은 [`super::Ctx::registry`] 로 읽는다**(리뷰) — 한 번 판 것을 들고 있어, 같은 명령의 다른
        // 자리(말을 고르는 `ctx.lang()`)가 다시 물어도 파일을 두 번 안 판다(moai-u8cs 가 걷어 낸 그것이고,
        // `Ctx::registry` 의 머리글이 `cmd::read` 를 이름으로 댄다).
        //
        // **설정을 여는 것은 이 `--all` 길뿐이다.** 위의 `writable_config(ctx)` 는 자리만 풀고 파일은 안
        // 연다 — 거기 든 `ctx.lang()` 은 자리를 못 찾았을 때만 돈다. 한때 이 주석이 "위에서 이미 그 문을
        // 지난다" 고 적어, `moai read <id>` 에 말을 미리 묻는 한 줄을 더해도 값이 없는 것처럼 보였다 —
        // 실제로는 설정을 안 열던 길에 파싱 한 번이 얹혀, 설정이 FIFO 면 그 명령이 멈췄다(moai-rtji 리뷰).
        let legacy = &ctx.registry().read;
        let marks = crate::read_marks::read(&path, &repo.root, legacy);
        // **못 든 까닭은 말한다**(리뷰) — 삼키던 판은 못 읽는 읽음 파일 하나로 `--all` 이 내게 온 것을
        // 통째로 "안 읽음" 으로 세어 도장을 다시 찍으면서, 왜 그랬는지를 어디에도 안 남겼다. 막지는
        // 않는다 — 종료 코드는 못 찾은 id 만 움직인다(#a-partial).
        say_why(&marks.problems, ctx, &mut said);
        want.extend(crate::query::unread(&load.issues, &me, &marks.seen).into_iter().map(str::to_string));
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
                // 이 명령의 거절과 몸통이 **한 말로 선다**(리뷰) — 없는 id 만 말묶음에서 오고
                // 나머지가 박혀 있으면 한 판의 stderr 와 stdout 이 두 말로 갈린다.
                let said = crate::i18n::fill(
                    crate::i18n::say(ctx.lang(), "refuse.read_not_a_group"),
                    &[("id", group), ("is", g.kind.as_str())],
                );
                return Err(Fail::coded(said, super::code::BAD_INPUT));
            }
            Some(_) => {
                let index = crate::nav::Index::of(&load.issues);
                want.extend(index.under_group(&load.issues, group).into_iter().map(|at| load.issues[at].id.clone()));
            }
        }
    }
    missing.extend(want.iter().filter(|id| !known.contains(id.as_str())).cloned());
    let targets: BTreeSet<&str> = want.iter().map(String::as_str).filter(|id| known.contains(id)).collect();

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
        // 걷을 것을 **쓸 때만** 센다 — 적을 것이 없는 판에서 줄 수만큼 집합을 짓지 않는다(리뷰).
        let keep = keep_for_prune(&repo, &load);
        // **집합은 닫은 글 밖에서 한 번 짓는다**(리뷰) — [`crate::read_marks::update`] 는 그 글을 두 번
        // 돌릴 수 있어(짓기 전 재 보기), 안에서 지으면 줄 수만큼의 짓기가 판마다 두 번 선다.
        let keep: Option<BTreeSet<&str>> = keep.as_ref().map(|k| k.iter().map(String::as_str).collect());
        // **말은 멈췄을 때만 묻는다**(moai-rtji 리뷰) — `update` 는 묻는 길을 받아 거절할 때만, 읽음 파일의
        // 락을 놓은 뒤에 부른다. 값으로 넘기던 판은 아무것도 안 거절하는 판(`--json` 으로 끝나는 길까지)에도
        // 사용자 설정을 열어 파싱했다 — [`super::open_repo`] 가 적어 둔 그 덫이다.
        let lang = || ctx.lang();
        let wrote = crate::read_marks::update(&path, &repo.root, lang, |sheet| {
            let lines = load.issues.iter().filter(|i| targets.contains(i.id.as_str()));
            let marks = crate::query::read_marks_of(lines, &sheet.marks().0);
            let wrote = sheet.mark(&marks)?;
            // **트래커에 없는 id 를 여기서 걷는다**(moai-dt5q, 사용자 결정 2) — 이 자리는 트래커를 이미
            // 들고 있다. 닫힌 줄은 안 걷는다: 걷으면 그 줄이 다시 설 때 [NEW] 가 되살아나, 읽음의 뜻이
            // "본 적 있다" 에서 "최근에 본 적 있다" 로 바뀐다.
            if let Some(keep) = &keep {
                sheet.prune(keep);
            }
            Ok(wrote)
        })?;
        // **적는 자리를 못 골랐으면 말한다**(moai-ajh2) — 위의 `--all` 이 읽는 길에 대는 것과 같은 줄이고,
        // 값을 잃는 쪽은 이쪽이다. 떨어진 판의 도장은 도구가 짓는 **대기 자리**에 가고, 다음 성한 쓰기가
        // 그것을 합치고 다 앉았으면 지운다(`read_marks::spool_at`, moai-bdej·moai-wd5u) — 이미 도장이 선
        // 사람에게도 그렇다. 못 앉힌 것이 있으면 남기고 그 까닭을 여기서 댄다.
        // 막지는 않는다 — 종료 코드는 못 찾은 id 만 움직인다(#a-partial).
        say_why(&wrote.problems, ctx, &mut said);
        // **막힌 id 는 이름으로 낸다**(리뷰 moai-kuib.g9c 9번). `Sheet::mark` 이 그 id 만 건너뛰게
        // 되면서(moai-l5ue) 시킨 id 가 `read` 에도 `missing` 에도 안 서고, 남는 것은 `problems` 의
        // 산문뿐이었다 — 사람 없이 도는 고리(`examples/bash-agent`)는 그것을 "적혔다" 로 세고 지나가
        // 그 줄이 영영 [NEW] 로 선다. `ready --json` 의 `held` 와 같은 꼴로 낸다.
        held.extend(wrote.problems.iter().flat_map(|w| match w {
            crate::read_marks::SheetTrouble::Held { ids, .. } => ids.clone(),
            _ => Vec::new(),
        }));
        wrote.value
    };

    // **없는 줄은 `--json` 에서도 말한다** — 기계로 읽는 쪽은 `missing` 으로, 사람은 stderr 로.
    // 하나가 없다고 나머지를 안 적지 않되, **비영으로 끝난다**(#a-partial) — `mv`·`defer`·`rm` 과
    // 같은 자리다. 0 으로 끝나면 고리를 짜는 쪽이 오타 친 id 를 적힌 것으로 세고 넘어간다.
    let missing: Vec<String> = missing.into_iter().collect();
    for id in &missing {
        super::note_partial();
        // `mv`·`defer` 와 **한 키다**(`refuse.not_found`, moai-95g1) — 손으로 지으면 이 명령만 옛 말로 남는다.
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(ctx.lang(), "refuse.not_found"), &[("id", id)]));
    }
    if ctx.json {
        // **기계도 그 까닭을 받는다**(moai-ajh2). stderr 한 줄만 내던 판은 고리를 짜는 쪽이 그것을
        // 못 봤다 — 사람 없이 도는 고리는 stderr 를 버리기 일쑤라, 옛 철자 자리에 찍힌 도장을
        // "적혔다" 로 세고 지나갔다. **늘 서는 배열**이다: 빈 것이 "탈이 없었다" 는 답이라 받는
        // 쪽이 키를 안 가른다. 종료 코드는 그대로 못 찾은 id 만 움직인다 — 막는 값이 아니다.
        let problems: Vec<&str> = said.iter().map(String::as_str).collect();
        let held: Vec<&str> = held.iter().map(String::as_str).collect();
        return super::json_line(&Marked {
            read: &fresh,
            missing: &missing,
            held: &held,
            problems: &problems,
            at: &now,
        });
    }
    if fresh.is_empty() {
        return Ok(vec![crate::i18n::say(ctx.lang(), "read.nothing").to_string()]);
    }
    let marked = crate::i18n::say(ctx.lang(), "read.marked");
    Ok(fresh.iter().map(|id| format!("{}  {}", paint(style::ID, id), paint(style::DIM, marked))).collect())
}

/// 읽음을 들고 적다 만난 까닭을 stderr 로 — **같은 글은 한 번만.**
///
/// **한 줄로 접는다**([`crate::text::one_line`]) — 이 글은 사람의 경로와 `io::Error` 를 품어(`settle`)
/// 줄바꿈이나 제어문자가 들 수 있고, 여기 낸 줄은 줄 단위로 읽힌다. 접지 않으면 한 까닭이 여러 줄로
/// 흩어져 어디까지가 한 줄인지가 흐려진다(`cmd::project` 와 같은 자).
///
/// **본 글을 적어 둔다** — `--all` 은 읽는 길과 쓰는 길을 한 판에 지나고 둘이 같은 자리 고르기를 물고
/// 오므로, 그대로 내면 한 번 난 탈이 두 줄로 선다.
///
/// **글은 여기서 편다**(moai-rtji) — 읽음 모듈은 자료만 내고([`crate::read_marks::SheetTrouble`]) 화면
/// 말을 모른다. 같은 글을 한 번만 내는 잣대는 **편 글**이다: 두 길이 같은 자료를 내면 같은 글이 된다.
///
/// **말은 댈 것이 있을 때만 묻는다**([`Ctx`] 째로 받는다, 리뷰) — 빈 판이 거의 모든 판인데, 말을 인자로
/// 받으면 그 판마다 사용자 설정을 연다([`super::open_repo`] 가 적어 둔 덫).
fn say_why(whys: &[crate::read_marks::SheetTrouble], ctx: &Ctx, said: &mut BTreeSet<String>) {
    for why in whys {
        let line = crate::text::one_line(&crate::view::sheet_trouble(ctx.lang(), why));
        if said.insert(line.clone()) {
            eprintln!("moai: {line}");
        }
    }
}

/// 걷을 때 **지킬 id 전부** — 되면 `Some`, 이 스냅샷이 트래커 전부를 봤다고 말 못 하면 `None` 이고
/// 그때는 **안 걷는다**(리뷰).
///
/// `prune` 은 "여기 없는 id" 를 지운다. 그러니 `load.issues` 가 트래커의 전부가 아닌 자리마다 조용한
/// 손실이 되고, 그 자리가 셋이다.
///
/// - **못 읽은 줄이 쥔 id.** [`crate::store::Load::reserved_ids`] 가 안다 — 더해서 지킨다
/// - **JSON 조차 아닌 줄.** 그 줄은 제 id 도 못 내놓아(`LoadError::id` 가 `None`) 무엇을 지킬지 모른다.
///   모르는 채로 걷으면 줄을 고친 뒤 [NEW] 가 되살아나므로 아예 안 걷는다
/// - **옆 워크트리에만 있는 줄.** 탐색기는 겹쳐 보기를 켠 채 그 줄에 도장을 찍는데
///   (`worktree::gather`), 같은 파일을 걷는 이쪽이 루트의 줄만 세면 그 도장이 `moai read` 한 번에
///   걷힌다 — 두 표면이 한 파일을 쓰니 세는 자도 같아야 한다. 옆을 못 읽었으면(`trouble`) 역시 안 걷고,
///   **아예 못 찾았으면(`unfound`) 도 그렇다**(리뷰) — 그때는 `trouble` 이 빈 채로 옆이 통째로 안 보인다
///
/// 옆을 훑는 값은 **쓸 때만** 치른다(부르는 쪽이 `targets` 가 빈 판에서 안 부른다).
fn keep_for_prune(repo: &Repo, load: &crate::store::Load) -> Option<BTreeSet<String>> {
    if load.errors.iter().any(|e| e.id.is_none()) {
        return None;
    }
    let beside = crate::worktree::gather(repo, true).ok()?;
    // **못 찾은 것도 못 읽은 것이다**(리뷰) — `unfound` 는 "옆을 아예 못 셌다"(git 이 없거나 저장소가
    // 아니다)는 뜻이라, 이때 `trouble` 은 빌 수밖에 없고 `sides` 도 비어 제 줄만 남는다. 그것을 걷을
    // 기준으로 삼으면 옆에만 있는 줄의 도장이 조용히 지워진다 — 조용한 손실은 이 도구가 못 견디는
    // 유일한 실패다. 못 봤으면 안 걷는다.
    if !beside.trouble.is_empty() || beside.unfound.is_some() {
        return None;
    }
    Some(beside.load.issues.iter().map(|i| i.id.clone()).chain(load.reserved_ids()).collect())
}

#[derive(serde::Serialize)]
struct Marked<'a> {
    read: &'a [String],
    missing: &'a [String],
    /// 시킨 id 가운데 **사람이 적은 값에 막혀 못 적은 것**(moai-l5ue, 리뷰 moai-kuib.g9c 9번).
    /// `read` 에도 `missing` 에도 안 서는 셋째 답이다 — `ready --json` 의 `held` 와 같은 꼴이다.
    ///
    /// **늘 서는 배열**이고 빈 것이 정상이다. **종료 코드는 안 움직인다** — 막힌 줄은 사람이 적어 둔
    /// 값에서 오고, 그 한 줄로 `moai read --all` 이 비영으로 끝나면 고리는 매 판을 실패로 읽는다
    /// ("남의 낡은 줄 하나가 모든 쓰기를 막으면", CLAUDE.md). 다시 해 볼 id 를 고리가 이 배열에서 든다.
    held: &'a [&'a str],
    /// 읽음을 들고 적다 만난 까닭 — stderr 로 낸 그 글들이다([`say_why`]). 빈 배열이 정상이다.
    problems: &'a [&'a str],
    /// 이 명령이 돈 때다. `[read]` 에 적힌 값이 아니다 — 그것은 줄마다 본 줄의 `updated_at` 이다(moai-lyc1).
    at: &'a str,
}
