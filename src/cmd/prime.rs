//! 에이전트가 세션 첫머리에 읽는 요약 한 판.
//!
//! **보드가 아니다.** `moai status` 는 이 저장소에서 13KB 를 넘고 경고·흐름·묶음 막대까지
//! 그리는데, 세션이 열릴 때와 맥락이 접힌 뒤에 묻는 것은 둘뿐이다 — 내가 무엇을 쥐고
//! 있었나, 다음은 무엇인가. 나머지는 읽는 쪽의 맥락을 그만큼 밀어낸다. beads 가 `bd prime`
//! 을 둔 까닭도 같다(MCP 스키마 10~50k 토큰 대 prime 1~2k, `moai-6k07` §1).
//!
//! **아무것도 막지 않는다.** 종료 코드는 언제나 0 이다 — `.moai` 가 없어도, 있는데 못 열어도,
//! 옆 워크트리를 못 읽어도. 없는 것과 못 연 것은 판이 가른다(moai-yivo.6je). 여기서 0 아닌 값을 내는 순간 이 요약을 세션 시작 훅에 건 사람의 세션이
//! "실패" 로 열리고, 그러면 이건 린트고 린트는 곧 게이트다(`moai status` 가 아무것도 안
//! 막는 것과 같은 자리).
//!
//! 무엇을 실을지는 [`crate::report::prime`] 이 정하고 어떻게 보일지는
//! [`crate::view::prime`] 이 정한다. 여기는 둘을 잇기만 한다.

use super::{Ctx, R};
use crate::report;
use crate::store::Repo;
use crate::view;

/// `--json` 한 줄. **사람 쪽과 같은 것을 낸다** — 닫기 전 목록과 명령은 글이지만 여기서
/// 빼면 훅에 거는 쪽이 두 표면 중 하나를 못 믿게 된다.
#[derive(serde::Serialize)]
struct Said<'a> {
    /// 지금 집은 일. **`ready --json` 의 `held` 가 아니다** — 그쪽은 미뤄 둔 것에 막혀 *못*
    /// 집는 일이라 뜻이 정반대고, 같은 이름을 쓰면 `ready` 에 맞춰 짠 고리가 제가 쥔 줄을
    /// "막혔으니 건너뛴다" 로 읽는다. 이름은 `status` 가 같은 값에 쓰는 `picked` 다.
    picked: Vec<Brief<'a>>,
    ready: Vec<Brief<'a>>,
    /// `ready` 에 안 실린 나머지 수. **늘 싣는다** — 0 이 "다 실었다" 는 뜻이다.
    rest: usize,
    /// 집을 수 있지만 **내 것이 아닌** 줄(moai-0zjo) — `ready --json` 의 `others` 와 같은 이름·같은
    /// `owner` 다. 줄 모양은 이 판의 [`Brief`] 에 담당을 곁들인 [`Whose`] 다. **늘 싣는다**.
    others: Vec<super::Other<Whose<'a>>>,
    /// `others` 에 안 실린 나머지 수 — `rest` 와 같은 약속으로 **늘 싣는다**.
    others_rest: usize,
    /// 지금 도는 마일스톤. `ready --json` 과 같은 이름·같은 값이고, 없으면 키를 안 단다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    milestone: Vec<&'a str>,
    /// 그 마일스톤 밖이라 이번에 안 낸 일 — `ready --json` 과 같은 이름·같은 값이다.
    ///
    /// **`milestone` 만 싣고 이것을 빼지 않는다**(리뷰). `rest` 는 이미 걸러진 목록에서 세므로
    /// 마일스톤이 뺀 일은 어디에도 안 세어진다 — 멤버가 다 막힌 마일스톤이 돌면 `ready:[]`
    /// `rest:0` 이 나가고, 밖에 스무 건이 있어도 받는 쪽은 "할 일이 없다" 로 읽는다. `ready`
    /// 가 그 한 줄을 대는 까닭(moai-q04l)이 이 표면에서 그대로 되살아난 자리다.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    outside: Vec<&'a str>,
    closing: Vec<&'a str>,
    commands: Vec<Line<'a>>,
    /// **트래커가 없을 때만 선다.** 빈 `picked`·`ready` 만으로는 "할 일이 없다" 와 "여기엔
    /// 트래커가 없다" 가 같아 보인다 — `commits_error` 가 "아직 아무도 안 고쳤다" 와
    /// "여기서는 못 물어봤다" 를 가르는 것과 같은 자리다.
    ///
    /// **못 연 트래커에는 서지 않는다**(moai-yivo.6je) — 그쪽은 `tracker_error` 다. 둘 다 이 키로 내던
    /// 판은 링크나 깨진 설정 하나로 멈춘 저장소를 "트래커 없음" 으로 읽혀, 받는 쪽이 `moai init` 을 불렀다.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    no_tracker: bool,
    /// **트래커를 못 열었을 때만 선다**(moai-yivo.6je) — 여기서 친 다른 명령이 멈추며 낼 `code`(`broken` 이면
    /// 손으로 고칠 파일이다)와 사람이 읽을 한 줄. 종료 코드는 그래도 0 이다 — 그 약속은 이 판의 것이고,
    /// 가르는 것은 이 키다. 꼴은 `show --json` 의 `commits_error` 처럼 객체 하나다.
    #[serde(skip_serializing_if = "Option::is_none")]
    tracker_error: Option<TrackerError<'a>>,
}

/// [`Said::tracker_error`] 의 값. **가르는 것은 `code` 다** — 다른 명령의 `--json` 거절(`{"error","code"}`)과
/// 같은 낱말이라, 받는 쪽이 같은 갈래로 읽는다. `said` 는 사람이 읽을 한 줄이라 그 낱말에 기대지 않는다.
#[derive(serde::Serialize)]
struct TrackerError<'a> {
    code: &'static str,
    said: &'a str,
}

/// `others` 의 한 줄 — [`Brief`] 에 **그 줄의 담당**을 곁들인다(moai-0zjo 리뷰). 이 판이 남의 줄을 대는
/// 까닭은 사람에게 물으라는 것인데, 누구의 것인지가 빠지면 받는 쪽이 `show` 를 한 번 더 부른다. 키는
/// [`super::Row`] 가 파일에서 펴는 것과 같은 둘(`assignee`·`assignee_email`)이고, 없으면 키가 없다 —
/// 담당 없는 줄은 `owner` 가 `unowned` 다. `picked`·`ready` 의 줄은 다 내 것이라 여기 들지 않는다.
#[derive(serde::Serialize)]
struct Whose<'a> {
    #[serde(flatten)]
    row: Brief<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee_email: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct Line<'a> {
    run: &'a str,
    said: &'a str,
}

/// 이 판이 내미는 줄 하나. **[`super::Row`] 를 그대로 안 싣는다**(리뷰).
///
/// `Row` 는 줄 전체를 펴므로 본문이 통째로 따라온다 — 이 저장소에서 `prime --json` 이
/// 11KB 였다. 사람 쪽 2.8KB 의 네 배고 보드(13KB)에 가깝다. [`crate::report::PRIME_PICKS`]
/// 의 자름은 **줄 수**만 묶지 크기를 안 묶는데, 그 문서는 두 표면이 같이 잘려 기계 쪽이
/// 보드만큼 길어지지 않는다고 적고 있었다 — 그 말이 서려면 줄도 짧아야 한다.
///
/// 본문이 필요하면 `moai show <id> --json` 이 낸다. 이 판이 답하는 것은 둘뿐이다 — 무엇을
/// 쥐었나, 다음은 무엇인가. `ready --json` 의 `held` 가 제 모양(`{id,by,undo,empty}`)을
/// 쓰는 것과 같은 자리다: 모든 표면이 `Row` 를 써야 한다는 약속은 없다.
#[derive(serde::Serialize)]
struct Brief<'a> {
    id: &'a str,
    title: &'a str,
    status: &'a str,
    priority: u8,
    /// 줄이 **제 몸에 적은** 에픽. 없으면 키를 안 단다 — [`super::Row`] 의 같은 키와 한 뜻이다.
    #[serde(skip_serializing_if = "Option::is_none")]
    epic: Option<&'a str>,
    /// 그 줄이 **든** 에픽(`report::handed_of`) — 적어 놓았든 id 로 졌든. [`super::Row::derived_epic`]
    /// 과 한 키, 한 뜻이다(moai-wuzi, 2026-09-23 사용자 결정).
    ///
    /// **한때 이 값이 `epic` 에 실렸다.** 그때는 이 표면만 물려받은 소속을 풀고 줄을 내는 다른
    /// 표면은 적힌 필드만 실어, 한 바이너리가 "이 줄은 어느 에픽인가" 에 키마다 다른 답을 했다.
    /// 이제 `epic` 은 어디서나 파일에 적힌 그대로고, 푼 값은 어디서나 이 키다.
    #[serde(skip_serializing_if = "Option::is_none")]
    derived_epic: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    tags: &'a [String],
    /// 다른 워크트리에서 온 줄이면 그 브랜치 — [`super::Row`] 와 같은 약속이다.
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<&'a str>,
}

impl<'a> Brief<'a> {
    fn of(
        i: &'a crate::model::Issue,
        epics: &std::collections::BTreeMap<&'a str, &'a str>,
        kinds: &crate::report::Kinds<'_>,
        origin: &'a crate::worktree::Origin,
    ) -> Brief<'a> {
        Brief {
            id: &i.id,
            title: &i.title,
            status: i.status.as_str(),
            priority: i.priority(),
            epic: i.epic.as_deref(),
            // **[`super::Row`] 와 한 자로 낸다**(`report::stands_in`) — 지도를 그대로 읽으면
            // 이 표면만 제 차례를 갖는다.
            derived_epic: crate::report::stands_in(kinds, i, epics.get(i.id.as_str()).copied()),
            tags: &i.tags,
            branch: origin.branch(&i.id),
        }
    }
}

/// 트래커가 없거나 못 열었을 때의 한 판. **두 표면이 같은 것을 낸다** — 사람 쪽만 닫기 전 목록과
/// 명령을 빼던 판은 [`Said`] 가 내건 약속을 제자리에서 어겼다.
///
/// `refused` 가 못 연 까닭이다(moai-yivo.6je). **없는 것과 못 연 것을 가른다** — 둘을 한 판("`.moai` 가
/// 없다, `moai init` 이 심는다")으로 내던 때는 세션을 여는 에이전트가 `init` 을 불렀고, `init` 은 다
/// 괜찮다고 답했다. 까닭은 **판에 싣고 stderr 에는 안 낸다** — 세션 시작 훅은 stdout 만 맥락에 싣고,
/// 터미널의 사람에게는 같은 글이 두 번 선다.
fn bare(ctx: &Ctx, lang: crate::i18n::Lang, refused: Option<&super::Fail>) -> R<Vec<String>> {
    if ctx.json {
        return super::json_line(&Said {
            picked: Vec::new(),
            ready: Vec::new(),
            rest: 0,
            others: Vec::new(),
            others_rest: 0,
            milestone: Vec::new(),
            outside: Vec::new(),
            closing: view::prime_closing(lang),
            commands: lines(lang),
            no_tracker: refused.is_none(),
            tracker_error: refused.map(|e| TrackerError { code: e.code, said: &e.message }),
        });
    }
    Ok(view::prime_bare(lang, refused.map(|e| e.message.as_str())))
}

fn lines(lang: crate::i18n::Lang) -> Vec<Line<'static>> {
    view::prime_commands(lang).into_iter().map(|(run, said)| Line { run, said }).collect()
}

pub fn run(ctx: &Ctx, worktree: bool) -> R<Vec<String>> {
    let lang = ctx.lang();

    // **`.moai` 밖이어도 0 이다.** `ready` 는 등록한 프로젝트마다 한눈 보기로 가지만 이 판은
    // "이 세션이 선 저장소" 한 벌이라 그 길이 없다 — 여러 프로젝트를 한 판에 접으면 "집은
    // 것" 이 어느 저장소의 것인지 잃는다. 시작하는 말만 대고 물러난다.
    //
    // **깨진 트래커에서도 0 이다.** 설정 한 줄이 못 읽히거나 스냅샷 한 줄이 깨졌다고 `?` 로
    // 넘어지면, 이것을 세션 시작 훅에 건 쪽의 세션이 통째로 "실패" 로 열린다 — 여기 적힌
    // "언제나 0" 이 실제로 서려면 그 길이 없어야 한다. 6~7 세션이 한 `.moai` 를 같이 쓰는
    // 저장소에서 그 한 줄은 잠깐 깨졌다 낫는 것이고, 그동안 모든 세션이 실패로 열리면 안 된다.
    //
    // **다만 "없다" 로 접지 않는다**(moai-yivo.6je). 못 연 까닭은 판이 대고(사람 쪽은 첫 줄, `--json` 은
    // `tracker_error`), `moai init` 을 시키지 않는다 — 트래커는 거기 있고, `init` 은 다 괜찮다고 답한다.
    let repo = match Repo::find(|| lang) {
        Ok(Some(repo)) => repo,
        Ok(None) => return bare(ctx, lang, None),
        Err(e) => return bare(ctx, lang, Some(&e)),
    };
    let gathered = match super::gather(ctx, &repo, worktree) {
        Ok(g) => g,
        Err(e) => return bare(ctx, lang, Some(&e)),
    };
    let crate::worktree::Gathered { load, origin, .. } = gathered;
    // **못 읽는 줄은 말만 한다.** [`super::name_load_errors`] 는 `report_load_errors` 와 같은
    // 글을 내면서 부분 실패 깃발을 안 세운다 — `ready` 는 "답을 덜 냈다" 를 종료 코드로 말하는
    // 것이 맞고, 세션을 여는 이 판은 그 반대다.
    super::name_load_errors(lang, &repo.issues_path(), &load.errors);

    // **내 것만 낸다**(moai-0zjo, 2026-10-02 사용자 결정) — 집은 것도 다음 일도. 사람은 여기서 풀어
    // 자료로 건넨다. **열린 줄이 하나도 없으면 안 푼다** — 집은 것도 집을 것도 없는 판에 `git` 을 두 번
    // 띄울 까닭이 없다. 모르면 가르지 않는다(`report::by_owner`).
    let open = load.issues.iter().any(|i| !i.status.is_done());
    let me = if open { super::me_at(ctx, &repo.root) } else { None };
    let p = report::prime(&load.issues, &repo.config, me.as_ref());
    if ctx.json {
        // 소속은 **물려받은 것까지 푼다** — 자식의 에픽은 부모에게서 오므로, 줄에 적힌
        // `epic` 만 실으면 자식 줄이 에픽 없는 것으로 나간다. 사람 쪽이 제목을 내는 자와
        // 같은 지도다(`epic_labels` 가 이것 위에 제목을 얹는다).
        //
        // **넘겨받는 값을 묻는다**(`report::handed_of`, 리뷰 moai-jk2u.o78) — 접은 지도
        // (`report::groups`)는 같은 id 의 뒷줄이 **적은** `epic` 까지 들어, 제 `epic` 을 안 적은
        // 앞줄이 그것을 입는다. 이 키를 내는 표면은 둘인데(`super::Row::of` 와 여기) 한쪽만
        // 옮기면 한 바이너리가 `derived_epic` 에 두 답을 낸다 — `ready --json` 과 `show --json`
        // 은 id 부모의 에픽을 내는데 여기만 쌍둥이의 에픽을 냈다. 물은 줄만 묻는 것은
        // `ready --json` 과 같은 자리, 같은 까닭이다.
        let ids: Vec<&str> =
            p.held.iter().chain(p.picks.iter()).chain(p.others.iter().map(|(i, _)| i)).map(|i| i.id.as_str()).collect();
        let epics = report::handed_of(&load.issues, &ids);
        // **가려진 줄을 가르는 지도**(moai-53s2) — 소속 지도와 나란히 둔다. `wip` 은 가려진 줄을
        // 빼므로 오늘 여기에 그런 줄이 실릴 길은 없지만, 값을 내는 자(`report::stands_in`)가
        // 지도를 물으니 여기가 그것을 대는 자리다.
        let kinds = report::Kinds::of(&load.issues);
        return super::json_line(&Said {
            picked: p.held.iter().map(|i| Brief::of(i, &epics, &kinds, &origin)).collect(),
            ready: p.picks.iter().map(|i| Brief::of(i, &epics, &kinds, &origin)).collect(),
            rest: p.rest,
            others: p
                .others
                .iter()
                .map(|(i, owner)| super::Other {
                    // **빈 담당은 없는 것이다**(`report::owner` 와 같은 자) — 손으로 푼 머지의 `""` 를
                    // 그대로 내면 `unowned` 인 줄에 빈 담당이 선다.
                    row: Whose {
                        row: Brief::of(i, &epics, &kinds, &origin),
                        assignee: i.assignee.as_deref().filter(|a| !a.trim().is_empty()),
                        assignee_email: i
                            .assignee_email
                            .as_deref()
                            .filter(|_| i.assignee.as_deref().is_some_and(|a| !a.trim().is_empty())),
                    },
                    owner: *owner,
                })
                .collect(),
            others_rest: p.others_rest,
            milestone: p.focus.running.iter().map(|m| m.id.as_str()).collect(),
            outside: p.focus.outside.iter().map(|i| i.id.as_str()).collect(),
            closing: view::prime_closing(lang),
            commands: lines(lang),
            no_tracker: false,
            tracker_error: None,
        });
    }

    let screen = view::Screen::new(lang).at(ctx.clock()).over(&origin);
    Ok(view::prime(&p, &report::epic_labels(&load.issues), &repo.config, screen))
}
