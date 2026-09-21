//! 보드 · 경고 · 흐름.
//!
//! 게이트를 없앤 자리를 메우는 것이 이 명령 하나다. 무엇을 드러낼지는
//! `report::status` 가 정하고, 여기는 잇기만 한다.
//!
//! **종료 코드는 데이터가 깨졌을 때만 0 이 아니다.** 경고로 비영 종료하면
//! 에이전트가 이것을 실패로 읽고, 그러면 이건 린트고, 린트는 곧 게이트다.

use super::{Ctx, R};
use crate::model;
use crate::projects::{Entry, Overview, Seen};
use crate::report;
use crate::store::Repo;
use crate::view;

/// `status --json` 이 보고서에 덧붙이는 키(`run`). 보고서는 필드가 선언된 것뿐이라 걷을 것이
/// 없다 — 덧붙이는 자리 곁에 목록을 둔다.
impl super::Appendable for report::StatusReport {
    const APPENDED: &'static [&'static str] = &["unreadable_worktrees", "broken_worktrees", "branches"];
}

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트를 한눈에. **안이면 아래 그대로다** — 등록 목록을
    // 읽지도 않는다(결정 3: `.moai` 안의 CLI 는 그 프로젝트만 본다).
    let Some(repo) = Repo::find(|| ctx.lang())? else {
        return overview(ctx, worktree);
    };
    let crate::worktree::Gathered { load, origin, trouble, unfound, swept, sides, mine, .. } =
        super::gather(ctx, &repo, worktree)?;
    // stderr 에 한 줄씩 낸 것의 수 — 보드가 "문제 없다" 로 그 말을 뒤집지 않게 넘긴다(moai-cuw2).
    let trouble = trouble.len() + usize::from(unfound.is_some());
    // **그 줄이 쓰는 id** 까지 넘긴다 — id 가 있어야 산 줄과의 중복이
    // 드러난다(moai-4dk4).
    // 옆에서만 온 줄과 겹친 id 는 중복이 아니다 (`Origin::unreadable`).
    let unreadable: Vec<report::Unreadable> = origin
        .unreadable(load.errors.iter().map(|e| e.id.as_deref()))
        .into_iter()
        .map(|id| report::Unreadable { id })
        .collect();
    let now = model::now();
    let mut st = report::status(&load.issues, &unreadable, &repo.config, &now);
    // **자리 없는 집은 줄은 여기서만 싣는다**(moai-4370) — 까닭은 `report::stranded`. 치명이 아니라
    // 아래 종료 코드는 안 바뀐다. 언제 재는지는 `worktree::workplaces` 가 정한다 — 딸린 워크트리
    // 안에서 겹쳐 보지 않았으면 빈 목록이 오고, 그러면 `stranded` 가 조용하다.
    //
    // **넘기는 것은 옆을 실제로 겹쳤는가다**(`swept`), 시킨 깃발이 아니다(리뷰 moai-3lul.kt0 다시 본 판).
    // `--worktree` 를 줬어도 git 을 못 불러 못 겹쳤으면 줄은 딸린 워크트리의 스냅샷(갈라질 때의 main)
    // 뿐이라, 겹친 것으로 재면 main 에서 이미 끝낸 일을 "자리 없다" 로 댄다. 탐색기(`tui::placed`)와
    // 밖 한눈 보기가 같은 자로 잰다.
    //
    // **자리는 세션이 선 체크아웃에서 잰다**(`repo.here()`, 리뷰 moai-71ht 셋째 판) — `show` 와 같은
    // 자다. 트래커의 자리로 재던 판은 루트로 옮겨 간 워크트리 안에서 `status` 만 자리를 파, 같은
    // 자리에서 `show` 는 아무 말도 안 하는데 보드는 `자리 없다` 를 댔다(moai-6opu.p65 가 한 곳에
    // 모아 둔 판단이 부르는 쪽마다 다른 뿌리를 받아 또 갈렸다).
    // 겹치며 이미 판 옆 스냅샷을 넘긴다(moai-kos1) — `--worktree` 면 `gather` 가 그 파일을
    // 방금 열어 풀었고, 안 겹쳐 봤으면 비어 있어 예전 그대로다.
    // **제 스냅샷도 같이 넘긴다**(moai-mafv) — 그것은 `--worktree` 와 상관없이 방금 판 것이다.
    let (lost, unread) = crate::worktree::stranded_at_in(
        repo.here(),
        &repo.config,
        &load.issues,
        swept,
        &now,
        &crate::worktree::dug(&sides, &mine),
    );
    st.warnings.extend(lost);
    // **못 읽은 워크트리는 한 줄씩 말한다**(moai-lt7h) — 자리 판정에서 그 워크트리는 "아무도
    // 없다" 가 아니라 "모른다" 로 빠지므로(`report::Place::Unknown`), 말이 없으면 경고가 조용한
    // 까닭을 알 길이 없다. 옆 워크트리의 문제로 세는 자리는 `gather` 와 같다 — 종료 코드는
    // 안 바꾸고, 보드가 "문제 없다" 로 이 말을 뒤집지 않게만 한다.
    //
    // **가지와 경로를 함께 댄다.** 가지만 대면 떼어 낸 HEAD 의 이름은 커밋 앞 일곱 자라
    // (`Workplace::branch`) 같은 커밋에 선 워크트리 둘이 글자까지 같아져 보는 쪽이 하나로 읽고,
    // 경로만 대면 `gather` 의 `⎇ <가지>` 와 낱말이 갈린다. 경로를 어디서 재는지는
    // `worktree::workplaces` 가 정한다 — `show` 와 같은 자다.
    // **`gather` 가 이미 낸 것은 두 번 안 낸다** — `--worktree` 면 그쪽이 옆 스냅샷을 빠짐없이
    // 열어 같은 워크트리를 `⎇ <가지>: …` 로 냈다. 두 번 내면 stderr 에 같은 워크트리가 낱말만
    // 바꿔 두 줄로 서고, 보드의 `옆 워크트리 문제 N건` 이 하나를 둘로 세어 보는 쪽이 두 곳이
    // 깨진 줄로 읽는다.
    //
    // **`--worktree` 만으로는 못 가른다**(리뷰 moai-ya06) — `gather` 는 git 을 불러 옆을 세고
    // (`others_of`), 여기 `trees` 는 git 이 적어 둔 파일만 읽는다(`on_disk`). git 이 없거나
    // `worktree list` 가 실패하면 `gather` 는 `unfound` 하나만 내고 워크트리를 **한 곳도**
    // 대지 않는데, 이쪽은 그대로 찾아 낸다 — 그때 입을 다물면 깨진 워크트리를 아무도 안 말하고
    // `stranded` 까지 조용해진다. 그쪽이 실제로 셌을 때만 접는다 — 그 자는 `Gathered::swept` 하나고,
    // 밖 한눈 보기(`view::projects_status`)도 같은 것을 읽는다.
    //
    // **여기서 대는 것은 판정을 가렸는지와 상관없이 못 읽은 것 전부다**(사용자 결정 2026-09-18,
    // 리뷰 moai-rgz9.7vt). 깨진 스냅샷은 고칠 사람이 있어야 고쳐지는데, 이름이 집은 줄을 가리킨다는
    // 까닭으로 입을 다물면 그 워크트리는 어느 화면에도 안 선다. "못 셌다" 쪽은 `Unread::blinding`
    // 이 따로 센다. **판 것에 매이지 않는다**(moai-giz3) — 이름만으로 자리가 다 잡혀 스냅샷을
    // 안 푸는 길에서도 `workplaces_in` 이 파일을 열어 보고 깨진 것을 세운다.
    let said_already = swept;
    if !said_already {
        for t in &unread.all {
            // 글은 `view` 한 자리에서 짓는다(moai-dpbi) — 밖 한눈 보기가 같은 줄을 낸다.
            eprintln!("{}", view::unread_worktree(ctx.lang(), &t.branch, &t.path));
        }
    }
    // 센 것은 **낸 것뿐이다** — `gather` 가 이미 낸 줄은 `trouble` 에 이미 들어 있다.
    let trouble = trouble + if said_already { 0 } else { unread.all.len() };
    // **낡은 AGENTS.md 블록은 알림이다**(moai-mj45, 2026-09-14 사용자 결정). 언제 서고 무엇을
    // 대는지는 `agents_notice` 가 정하고, 훅의 보드가 같은 것을 싣는다. 한눈 보기(`.moai` 밖)는
    // 남의 저장소라 안 본다.
    st.notices.extend(crate::cmd::init::agents_notice(repo.here(), ctx.chdir));
    // 빠진 딸린 파일 규칙도 같은 자리다(moai-2f99) — `init` 이 한 번 말하고 마는 것을 여기가 잇는다.
    st.notices.extend(crate::cmd::init::dotfile_notice(repo.here(), ctx.chdir));
    // **머지 드라이버의 상태도 여기서 댄다**(moai-2ewr·moai-9khu). 언제 무엇이 서는지는
    // `merge_driver::notice` 가 정하고, 훅의 보드가 같은 것을 싣는다 — 위의 `agents_notice` 와
    // 같은 자리다. 그 사실이 이 화면 말고는 설 데가 없어서 여기 있다.
    st.notices.extend(crate::cmd::merge_driver::notice(&repo, ctx.chdir));

    // **설정에 적은 말이 틀렸으면 여기서 댄다**(리뷰 moai-80qw). `Doc::lang` 이 그 줄을 짓는
    // 까닭은 "오타가 조용히 영어가 되면 고친 설정이 왜 안 듣는지 알 길이 없다" 였는데
    // (리뷰 moai-slfv.vrw), 저장소 **안**의 이 명령은 등록 목록을 안 읽으므로(위의 결정 3)
    // 그 줄이 닿는 자리가 없었다 — 세션이 시작하는 화면이 바로 여기다.
    //
    // **긴 말은 stderr 로, 보드에는 셈만.** 어느 파일의 어느 값인지는 한 줄이 길어 보드의
    // 표를 밀어내고, 그렇다고 stderr 로만 내면 보드가 "드러난 문제 없다" 로 방금 한 말을
    // 뒤집는다(moai-cuw2). **알림이지 경고가 아니다** — 계획이 아니라 설치가 어긋난 것이라
    // `agents_stale` 과 같은 자리고, 종료 코드는 안 바뀐다. 말을 고를 때 이미 읽은 것이라
    // 설정을 다시 읽지 않는다.
    // 글은 말을 고른 뒤에 편다(moai-dpbi) — 설정을 읽는 길은 말을 모른다(`user_config::LangTrouble`).
    let reg = ctx.registry();
    let said = &reg.lang_problems;
    for why in said {
        eprintln!("{}", view::problem(ctx.lang(), reg.path.as_deref(), why));
    }
    if !said.is_empty() {
        st.notices.push(report::Warning::user_config(said.len()));
    }

    if st.broken() {
        super::note_partial();
    }
    if ctx.json {
        // **못 읽은 워크트리는 기계에게도 댄다**(리뷰 moai-ya06). 그런 워크트리가 하나라도 있으면
        // 자리 판정이 통째로 `모른다` 로 접혀 `stranded` 가 조용해지는데(`report::places` 의
        // `blind`), 여기 키가 없으면 받는 쪽은 "자리 잃은 일이 없다" 와 "못 셌다" 를 못 가른다 —
        // 감독 스킬이 이 목록의 `stranded` 로 죽은 세션의 일을 거두므로, 그 침묵이 곧 일을
        // 영영 안 거두는 것이 된다. 사람 화면은 `옆 워크트리 문제 N건` 으로 이미 가르고, `show
        // --json` 도 같은 사실을 `place` 로 낸다 — 가르는 것을 받는 쪽도 가를 수 있어야 한다.
        //
        // **여기 드는 것은 판정을 가린 워크트리뿐이다**(`Unread::blinding`, moai-rgz9) — 못 읽어도
        // 이름이 집은 줄을 가리키는 워크트리는 판정을 안 가리니 안 든다. 키 이름은 이미 나간
        // 값이라 그대로 두지만 "못 읽은 워크트리 전부" 가 아니다 — 그쪽은 위에서 stderr 에 한
        // 줄씩 내고 기계에는 아래 `broken_worktrees` 가 댄다.
        //
        // **없으면 키를 안 단다** — 빈 목록을 늘 달면 그것이 "다 읽었다" 인지 "안 재 봤다" 인지가
        // 다시 두 뜻이 된다. `--worktree` 여부와 무관하게 단다: `gather` 의 `⎇` 줄은 stderr 라
        // 기계가 읽는 자리에는 어느 쪽에서도 이 사실이 없었다.
        let mut extra = Vec::new();
        if !unread.blinding.is_empty() {
            extra.push((
                "unreadable_worktrees",
                serde_json::to_string(&unread.blinding).map_err(|e| super::Fail::new(e.to_string()))?,
            ));
        }
        // **깨진 스냅샷 전부는 곁의 키로 댄다**(moai-zah3, 2026-09-18 사용자 결정). 위의 키는 판정을
        // 가린 것만 담아, 이름이 집은 줄을 가리키는 워크트리의 깨진 스냅샷은 어느 JSON 에도 안
        // 섰다 — 고칠 사람이 있어야 고쳐지는데 감독 스킬과 `examples/bash-agent` 는 `--json` 으로
        // 돈다. 위 키의 뜻은 이미 나간 값이라 안 바꾼다. 없으면 키를 안 다는 것도 같은 까닭이다.
        if !unread.all.is_empty() {
            extra.push((
                "broken_worktrees",
                serde_json::to_string(&unread.all).map_err(|e| super::Fail::new(e.to_string()))?,
            ));
        }
        // **겹쳐 봤을 때만 키를 단다.** 늘 달면 `--worktree` 없이 부른 쪽도 빈
        // 지도를 받아 "겹쳐 봤는데 옆에 아무것도 없다" 로 읽는다.
        if worktree {
            extra.push((
                "branches",
                serde_json::to_string(&origin.branches()).map_err(|e| super::Fail::new(e.to_string()))?,
            ));
        }
        if extra.is_empty() {
            return super::json_line(&st);
        }
        return super::json_with(&st, &extra);
    }
    Ok(view::status(
        &st,
        &load.issues,
        &repo.config,
        &now,
        &source_of(&repo),
        trouble,
        view::Screen::new(ctx.lang()).over(&origin),
    ))
}

/// 보드가 **정말 읽은 파일**을 머리에 댄다 — 딸린 워크트리 안에서는 그 자리의 `.moai` 가 아니라
/// 루트의 트래커다(moai-y7go). 늘 `.moai/issues.jsonl` 로 적던 판은 그 워크트리의 갈라질 때 스냅샷을
/// 가리켜, 시킨 대로 그 파일을 열어 본 쪽이 보드와 다른 줄을 보고 보드가 거짓말한다고 읽었다
/// (리뷰 moai-71ht 셋째 판의 훑기). 자리에서 잰 상대 경로라 `moai: … 루트의 트래커에 썼다` 와 같은
/// 파일을 가리킨다.
pub fn source_of(repo: &crate::store::Repo) -> String {
    let here = repo.here();
    match here == repo.root {
        true => ".moai/issues.jsonl".to_string(),
        false => crate::worktree::told_from(here, &repo.issues_path()),
    }
}

/// 등록한 프로젝트마다 보드 요약. **프로젝트마다 따로 센다** — 줄을 한데 모으지 않는다.
///
/// **깨진 프로젝트가 있어도 0 으로 끝난다.** 한 프로젝트 안의 `status` 가 깨진
/// 데이터로 비영 종료하는 것은 제 파일이라서다. 여기서 보는 것은 남의 저장소일 수
/// 있고, 그것 하나로 한눈 보기 전체가 실패로 읽히면 나머지를 못 믿는다.
fn overview(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    // **등록한 것이 없어도, 설정이 깨져 목록이 비어도 0 이다**(moai-ynsb, 2026-09-14 사람의 결정).
    // 세션은 `status` 로 시작한다 — 제 파일이 아닌 것(아직 없는 등록·사람의 설정)으로 비영
    // 종료하면 도구가 고장 난 것으로 읽히고, 같은 자리의 맨몸 `moai`·`project ls` 는 이미 0 이다.
    // 말은 그대로 댄다: 저장소가 아니라는 것, 등록하는 길, 목록이 빈 까닭. `ready` 는
    // [`super::registered`] 로 여전히 멈춘다 — 그쪽 계약은 따로 정한다. `tui --json` 은 같은 객체로
    // 0 이다(moai-yxae).
    let reg = ctx.registry();
    if reg.projects.is_empty() {
        if ctx.json {
            // 설정의 탈은 **편 뒤에** 싣는다(moai-dpbi) — 화면과 같은 목록이다(`view::settings_problems`).
            let problems = view::settings_problems(reg, ctx.lang());
            let none: Overview<()> =
                Overview { projects: Vec::new(), problems: &problems, config: reg.path.as_deref() };
            return super::json_line(&none);
        }
        return Ok(super::nothing_registered(reg, ctx.lang()).message.lines().map(str::to_string).collect());
    }
    let projects = crate::projects::open_with(reg, worktree, ctx.lang());
    let now = model::now();
    // **셈은 프로젝트마다 나란히 한다**(moai-b7o3) — 자리 판정이 옆 스냅샷을 파면 그 값이 프로젝트
    // 마다 더해진다. 보드는 연 프로젝트를 빌리므로 그 스레드에서 곧바로 짓는다 — 연 것만 가르는 자는
    // `Project::seen` 하나다(셈을 따로 모았다가 다시 맞추면 두 가름이 어긋날 자리가 생긴다).
    let seen: Vec<Seen<view::Board>> = crate::projects::each(&projects, |p| {
        p.seen(|repo, load| {
            // 옆에서만 온 줄과 겹친 id 는 중복이 아니다 (`Origin::unreadable`, `run` 과 같다).
            let unreadable: Vec<report::Unreadable> = p
                .origin
                .unreadable(load.errors.iter().map(|e| e.id.as_deref()))
                .into_iter()
                .map(|id| report::Unreadable { id })
                .collect();
            // **자리도 여기서 잰다**(moai-p3bs) — 안쪽 `moai status` 와 같은 자
            // (`worktree::stranded_at`). 한때 이 화면에만 없어, 프로젝트 밖에서 보드를 보는
            // 사람은 죽은 세션의 일을 영영 못 봤다. 옆 워크트리를 겹치는지는 부른 쪽을 따르되, 재는
            // 자는 **실제로 겹쳤는가**다(`Project::swept`) — 안쪽 `run` 과 같다.
            let mut status = report::status(&load.issues, &unreadable, &repo.config, &now);
            // 자리를 재는 자리는 **등록한 그 체크아웃**이다(`repo.here()`) — 안쪽 `run` 과 같다.
            let (lost, unread) = crate::worktree::stranded_at(repo.here(), &repo.config, &load.issues, p.swept, &now);
            status.warnings.extend(lost);
            view::Board {
                cfg: &repo.config,
                status,
                picked: report::wip(&load.issues, &repo.config),
                origin: &p.origin,
                trouble: &p.trouble,
                // **못 읽은 워크트리는 여기서도 센다**(리뷰 moai-p3bs.op2) — 밖에서는 `gather`
                // 가 겹쳐 보지 않으면 옆 스냅샷을 아예 안 열어 `trouble` 이 비고, 그러면 죽은
                // 세션과 못 읽는 워크트리가 함께 있는 저장소가 "드러난 문제 없다" 로 선다.
                // 목록 둘을 넘긴다 — 사람 화면은 못 읽은 것 전부를 한 줄씩 대고(`unread`),
                // `--json` 은 둘을 따로 낸다(`unreadable_worktrees` 는 판정을 가린 것 `blind`,
                // `broken_worktrees` 는 전부 `unread`). 안쪽 `status` 와 같은 가름이다.
                // `trouble` 이 이미 낸 것인지는 `swept` 가 가른다 — 이것도 안쪽과 같은 자다.
                unread: unread.all,
                blind: unread.blinding,
                swept: p.swept,
            }
        })
    });

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Said<'a> {
            status: &'a report::StatusReport,
            picked: Vec<super::Row<'a>>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            trouble: Vec<String>,
            /// **못 읽은 워크트리는 기계에게도 댄다** — 안쪽 `status --json` 과 같은 키·같은 모양
            /// (리뷰 moai-ya06). 그런 워크트리가 있으면 자리 판정이 통째로 `모른다` 로 접혀
            /// `stranded` 가 조용해지는데, 여기 키가 없으면 밖에서 읽는 쪽은 "자리 잃은 일이
            /// 없다" 와 "못 셌다" 를 못 가른다 — 감독 스킬이 이 목록으로 죽은 세션의 일을 거두므로
            /// 그 침묵이 곧 일을 영영 안 거두는 것이 된다. 없으면 키를 안 단다.
            #[serde(skip_serializing_if = "<[report::Workplace]>::is_empty")]
            unreadable_worktrees: &'a [report::Workplace],
            /// 깨진 스냅샷 **전부** — 안쪽 `status --json` 의 같은 키와 같다(moai-zah3). 없으면 안 단다.
            #[serde(skip_serializing_if = "<[report::Workplace]>::is_empty")]
            broken_worktrees: &'a [report::Workplace],
        }
        let entries = projects
            .iter()
            .zip(&seen)
            .map(|(p, s)| Entry {
                name: &p.name,
                path: &p.path,
                seen: s.map(|b| Said {
                    status: &b.status,
                    picked: b.picked.iter().map(|i| super::Row::of(i, None).on(&p.origin)).collect(),
                    // 옆 워크트리의 문제도 **편 뒤에** 싣는다(moai-dpbi) — 사람 화면과 같은 글이다.
                    trouble: p.trouble.iter().map(|t| view::trouble_line(ctx.lang(), t)).collect(),
                    unreadable_worktrees: &b.blind,
                    broken_worktrees: &b.unread,
                }),
            })
            .collect();
        let problems = view::settings_problems(reg, ctx.lang());
        let all = Overview { projects: entries, problems: &problems, config: reg.path.as_deref() };
        return super::json_line(&all);
    }
    Ok(view::projects_status(&projects, &seen, reg, view::Screen::new(ctx.lang())))
}
