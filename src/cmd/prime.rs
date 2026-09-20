//! 에이전트가 세션 첫머리에 읽는 요약 한 판.
//!
//! **보드가 아니다.** `moai status` 는 이 저장소에서 13KB 를 넘고 경고·흐름·묶음 막대까지
//! 그리는데, 세션이 열릴 때와 맥락이 접힌 뒤에 묻는 것은 둘뿐이다 — 내가 무엇을 쥐고
//! 있었나, 다음은 무엇인가. 나머지는 읽는 쪽의 맥락을 그만큼 밀어낸다. beads 가 `bd prime`
//! 을 둔 까닭도 같다(MCP 스키마 10~50k 토큰 대 prime 1~2k, `moai-6k07` §1).
//!
//! **아무것도 막지 않는다.** 종료 코드는 언제나 0 이다 — `.moai` 가 없어도, 옆 워크트리를
//! 못 읽어도. 여기서 0 아닌 값을 내는 순간 이 요약을 세션 시작 훅에 건 사람의 세션이
//! "실패" 로 열리고, 그러면 이건 린트고 린트는 곧 게이트다(`moai status` 가 아무것도 안
//! 막는 것과 같은 자리).
//!
//! 무엇을 실을지는 [`crate::report::prime`] 이 정하고 어떻게 보일지는
//! [`crate::view::prime`] 이 정한다. 여기는 둘을 잇기만 한다.

use super::{Ctx, R};
use crate::i18n::say;
use crate::report;
use crate::store::Repo;
use crate::view;

/// `--json` 한 줄. **사람 쪽과 같은 것을 낸다** — 닫기 전 목록과 명령은 글이지만 여기서
/// 빼면 훅에 거는 쪽이 두 표면 중 하나를 못 믿게 된다.
#[derive(serde::Serialize)]
struct Said<'a> {
    held: Vec<super::Row<'a>>,
    ready: Vec<super::Row<'a>>,
    /// `ready` 에 안 실린 나머지 수. **늘 싣는다** — 0 이 "다 실었다" 는 뜻이다.
    rest: usize,
    /// 지금 도는 마일스톤. `ready --json` 과 같은 이름·같은 값이고, 없으면 키를 안 단다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    milestone: Vec<&'a str>,
    closing: Vec<&'a str>,
    commands: Vec<Line<'a>>,
    /// **트래커가 없을 때만 선다.** 빈 `held`·`ready` 만으로는 "할 일이 없다" 와 "여기엔
    /// 트래커가 없다" 가 같아 보인다 — `commits_error` 가 "아직 아무도 안 고쳤다" 와
    /// "여기서는 못 물어봤다" 를 가르는 것과 같은 자리다.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    no_tracker: bool,
}

#[derive(serde::Serialize)]
struct Line<'a> {
    run: &'a str,
    said: &'a str,
}

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let lang = ctx.lang();
    let closing = view::prime_closing(lang);
    let commands: Vec<Line> = view::prime_commands(lang).into_iter().map(|(run, said)| Line { run, said }).collect();

    // **`.moai` 밖이어도 0 이다.** `ready` 는 등록한 프로젝트마다 한눈 보기로 가지만 이 판은
    // "이 세션이 선 저장소" 한 벌이라 그 길이 없다 — 여러 프로젝트를 한 판에 접으면 "집은
    // 것" 이 어느 저장소의 것인지 잃는다. 시작하는 말만 대고 물러난다.
    let Some(repo) = Repo::find()? else {
        if ctx.json {
            return super::json_line(&Said {
                held: Vec::new(),
                ready: Vec::new(),
                rest: 0,
                milestone: Vec::new(),
                closing,
                commands,
                no_tracker: true,
            });
        }
        return Ok(vec![
            format!("# {}", say(lang, "prime.title")),
            String::new(),
            say(lang, "prime.no_repo").to_string(),
        ]);
    };
    let crate::worktree::Gathered { load, origin, .. } = super::gather(ctx, &repo, worktree)?;
    super::report_load_errors(lang, &repo.issues_path(), &load.errors);

    let p = report::prime(&load.issues, &repo.config);
    if ctx.json {
        return super::json_line(&Said {
            held: p.held.iter().map(|i| super::Row::of(i, None).on(&origin)).collect(),
            ready: p.picks.iter().map(|i| super::Row::of(i, None).on(&origin)).collect(),
            rest: p.rest,
            milestone: p.focus.running.iter().map(|m| m.id.as_str()).collect(),
            closing,
            commands,
            no_tracker: false,
        });
    }

    Ok(view::prime(&p, &report::epic_labels(&load.issues), view::Screen::new(lang).over(&origin)))
}
