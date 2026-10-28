//! 간트 — 줄마다 **계획한 구간과 실제 구간**을 읽는다(moai-xf0p.j1r). **순수 함수다** — 줄과 지금 시각, 부르는 쪽이
//! 이미 만든 색인·거름망만 본다. 아무것도 찍지 않고 파일도 열지 않는다.
//!
//! **화면은 이 값을 그리기만 한다.** TUI 도 web 도 날짜를 판단하는 `if` 를 갖지 않는다(CLAUDE.md "`report` 와 `query`
//! 는 순수 함수다") — 계획이 어디서 왔는지, 시작을 아는지, 늦었는지는 여기서 정하고, 화면은 날짜 칸에 놓는 일만 한다.
//!
//! # 읽는 법
//!
//! - **계획 구간**([`Planned`]) — 제 `starts_on`·`due_on`. 묶음(에픽·마일스톤)이 제 날을 안 적었으면 **멤버가 적은
//!   날을 다 덮는 폭**이다 — 가장 이른 날 ~ 가장 늦은 날이다. 빈 시작은 만든 날(기한보다 늦으면 기한 날),
//!   빈 끝은 시작 날로 채운다([`own_plan`]). 멤버에서 센 폭은 멤버의 채운 시작까지 덮고, 아무 멤버도 시작을 안 적었으면
//!   묶음을 만든 날과 가장 이른 멤버 날 중 이른 날에서 선다. 시작만 적은 멤버와 기한만 적은 멤버가 섞여도 막대는 거꾸로 서지 않는다(리뷰 — "가장 이른 시작 ~ 가장
//!   늦은 기한" 을 따로 재던 판은 `@10-27~` 과 `@10-22` 두 멤버로 끝이 시작보다 앞선 막대를 냈다). 어느 쪽에서 왔는지
//!   [`Origin`] 이 말한다 — 화면이 "멤버에서 센 값" 을 다르게 그린다. 제 날은 **한 덩이다**: 시작이든 기한이든 하나라도
//!   적었으면 그 줄의 구간은 그 줄의 것이고, 빈 쪽을 멤버로 채우지 않는다(채우면 사람이 적은 구간과 센 구간이 한 막대에
//!   섞인다). 저장하지 않는다 — 묶음의 구간은 파생값이라 읽을 때 멤버에서 센다(CLAUDE.md "파생값은 저장하지 않는다")
//! - **실제 구간**([`Actual`]) — `started_at` ~ (닫혔으면 닫힌 때, 아니면 열림 = 지금까지). **되돌린 줄은 열려
//!   있다**: `done_at` 이 남아도 지금 done 이 아니다([`super::stats::closed_at`] 와 같은 자). 묶음은 **멤버로 잰다** —
//!   멤버의 가장 이른 `started_at` 부터이고, 묶음 줄 자신의 `started_at` 은 안 쓴다(`Issue::started_at` 은 "누가 그
//!   줄에 `mv` 를 쳤나" 일 뿐이다)
//! - **모르는 것은 0 이 아니다** — `started_at` 이 없으면(이 필드 전에 집은 줄·옛 바이너리가 옮긴 줄) 구간이 없고,
//!   "시작 모름" 을 [`Actual::Unknown`] 으로 따로 낸다. 만든 때로도 0 분으로도 채우지 않는다. 끝이 시작보다 앞선 줄도
//!   모른다고 한다(`spent_in` 의 "음수는 안 센다" 와 같은 금). **묶음은 떠난 멤버 하나라도 시작을 모르면 모른다**(리뷰) —
//!   그 멤버가 더 일찍 시작했을 수 있어, 아는 시작 가운데 가장 이른 것은 상한일 뿐이다. 앞날로 멀리 적힌 도장
//!   (`future_timestamp`)도 모르는 것이다 — `moai stats` 가 재는 자(`stats::spans`)와 같다
//! - **시간대** — 저장값은 UTC 라 [`Actual`] 은 UTC 시각을 그대로 내고, 날짜 칸에 놓는 것은 화면이 `tz::Zone` 으로
//!   옮긴다. 계획의 날은 달력 날짜라 옮기지 않는다. **다만 늦었는가(`overdue`)와 열린 구간의 "오늘" 은 읽는 사람의 달로
//!   잰다**(리뷰, moai-h2th) — 보드의 `overdue`(`Dues`)와 상세(`view::due_of`)가 재는 그 자다. 화면은 날짜를 판단하지
//!   않으므로 그 날수를 나중에 고칠 길이 없다
//!
//! # 어디서 가져오나
//!
//! 새로 짠 규칙이 없다. 묶음의 멤버·진척은 목록이 그리는 그대로(`nav::Index::descendants`·`tally`, 목록이 폴더로 여는
//! 줄만), 칸·닫힌 때·미룸은 거르개가 읽는 그대로(`query::Where`), 묶음의 계획에서 미룬 멤버를 빼는 자는 보드가 묶음의
//! 칸을 읽는 자(`report::counted`, [`super::deferred_sources`]), 막음은 상세가 그리는 그대로([`super::BlockMaps`]) 부른다.
//! 이 모듈은 그것들을 줄마다 한 값으로 모을 뿐이다.

use super::{Block, BlockMaps, is_group, is_work};
use crate::config::{Config, DONE};
use crate::model::{FUTURE_SLACK_SECS, Issue, Kind, days_until, format_rfc3339, parse_date, parse_rfc3339};
use crate::nav::Index;
use crate::query::Where;
use crate::tz::Zone;
use serde::Serialize;
use std::collections::BTreeMap;

#[cfg(test)]
thread_local! {
    pub(crate) static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// 계획 구간이 어디서 왔나.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// 그 줄이 적은 `starts_on`·`due_on`.
    Own,
    /// 묶음이 제 날을 안 적어 멤버에서 센 값.
    Members,
}

/// 계획한 구간. 양끝은 달력 날짜(`YYYY-MM-DD`)다. 채운 쪽은 파생값이며 저장하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Planned {
    pub start: String,
    pub end: String,
    pub start_filled: bool,
    pub end_filled: bool,
    pub from: Origin,
}

/// 실제 구간. 시각은 UTC 다(RFC3339).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actual<'a> {
    /// 아직 첫 칸을 떠난 적이 없다 — 구간이 없다.
    NotStarted,
    /// 시작했는데 언제인지 모른다. 안 한 일도 0 분도 아니다.
    Unknown,
    /// `start` 부터. `end` 가 없으면 열려 있다 — 지금까지.
    Span { start: &'a str, end: Option<&'a str> },
}

/// 줄이 얼마나 갔나 — 묶음은 끝난 일 수, 나머지는 서 있는 칸.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Standing<'a> {
    /// 묶음의 끝난 일 / 일. **목록 줄이 세는 그 수다**(`nav::Index::tally`) — 셀 일이 없으면 `(0, 0)` 이다.
    Rollup { done: usize, total: usize },
    /// 일 하나의 서 있는 칸.
    Column { name: &'a str },
}

/// 간트의 한 줄. `at` 은 `all` 의 첨자다 — id 는 겹칠 수 있어(`duplicate_id`) 줄을 첨자로 가리킨다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row<'a> {
    /// 부르는 쪽이 적재한 줄의 첨자라 밖에 내지 않는다 — `tui --json --deep` 의 줄에는 제 id 가 선다.
    #[serde(skip)]
    pub at: usize,
    pub id: &'a str,
    pub kind: Kind,
    /// 계획한 구간. 날을 하나도 못 읽으면 없다.
    pub planned: Option<Planned>,
    pub actual: Actual<'a>,
    pub progress: Standing<'a>,
    /// 열린 줄의 **제** 기한이 지난 날수(양수). 안 지났거나 열린 줄이 아니거나 기한이 없으면 없다. **미룬 줄은 안 센다** —
    /// 보드의 `overdue` 경고가 계획 밖의 줄을 안 세는 것과 같은 자다(고칠 까닭이 없는 기한이다). 묶음은 멤버의 기한이
    /// 아니라 제 기한만 본다.
    pub overdue: Option<i64>,
    /// 계획에서 뺀 줄인가(제 미룸이나 물려받은 미룸) — 화면이 흐리게 그린다.
    pub deferred: bool,
    /// 지금 이 줄을 **막고 있는** 막음. 풀렸거나(`done`) 없는 id 는 안 든다.
    pub blocked_by: Vec<Block<'a>>,
}

/// 화면에 놓을 날의 폭 — 줄들의 계획·실제 가운데 가장 이른 날부터 가장 늦은 날까지(`YYYY-MM-DD`). 화면의 "맞춤" 이 쓴다.
/// 실제는 UTC 의 날이다 — 시간대로 옮기면 하루 어긋날 수 있어 화면이 여백을 둔다. 열린 구간은 읽는 사람의 오늘까지다.
/// 짓는 자는 [`range_of`] 하나다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Range {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gantt<'a> {
    /// `all` 와 같은 차례, 같은 길이다 — `rows[at].at == at`. 화면이 목록을 거르고 정하는 것은 제 일이다.
    pub rows: Vec<Row<'a>>,
    /// **적재한 줄 전부**의 폭이다. 날이 하나도 없으면 없다. 거른 목록에 맞추는 화면은 남긴 줄로 [`range_of`] 를 다시
    /// 부른다(리뷰) — 숨긴 줄(아카이브·미룬 것)의 날이 이 폭을 넓히므로, 그 폭에 맞추면 보이는 막대가 한쪽에 몰린다.
    pub range: Option<Range>,
}

/// 줄마다 계획·실제 구간을 읽는다. `index` 는 `all` 로 지은 색인(`nav::Index::of`)이고 `wh` 는 같은 줄로 지은 거르개
/// 문맥이다 — 탐색기는 적재 때 둘 다 이미 들고 있어 여기서 다시 짓지 않는다. `now` 는 RFC3339(UTC)고, `zone` 은 읽는
/// 사람의 시간대다 — 늦었는가와 오늘을 그 달로 잰다(보드의 `Dues::judge` 가 받는 그 둘이다). 거르개를 `'a` 로 빌리는 것은
/// 서 있는 칸·닫힌 때가 거기서 빌려 온 글이라서다 — 값(`Gantt`)이 사는 동안 거르개도 산다.
/// `blocks` and `shelved` must describe the same complete load (archives included):
/// archived rows can still be blockers or deferred ancestors. The caller builds them once.
pub fn gantt<'a>(
    all: &'a [Issue],
    index: &Index,
    wh: &'a Where<'a>,
    cfg: &Config,
    blocks: Option<&BlockMaps<'a, '_>>,
    shelved: &BTreeMap<&str, Vec<&str>>,
    now: &str,
    zone: &Zone,
) -> Gantt<'a> {
    #[cfg(test)]
    CALLS.set(CALLS.get() + 1);
    // These global maps belong to the caller's load, not to each report invocation.
    // Resolve each row's known start once, including the future-stamp check. Groups
    // reuse those facts instead of parsing every member again for each ancestor.
    let starts: Vec<Option<(&str, i64)>> = all
        .iter()
        .map(|i| {
            let text = i.started_at.as_deref()?;
            let at = parse_rfc3339(text)?;
            (!super::far_ahead(i, now)).then_some((text, at))
        })
        .collect();
    let now_at = parse_rfc3339(now);
    // **날 경계는 읽는 사람의 달에서 긋는다**(moai-h2th) — 한 번만 옮긴다.
    let today = zone.shift(now);
    let rows: Vec<Row<'a>> = all
        .iter()
        .enumerate()
        .map(|(at, i)| {
            // **목록이 폴더로 여는 묶음만 멤버를 거느린다**(`Index::is_dir`, 리뷰) — 같은 id 의 앞줄(`duplicate_id`)은
            // 잎으로 그려지는데, 자리로만 물으면 뒷줄의 멤버·셈을 함께 입어 같은 막대가 둘 선다(moai-sfml 이 목록에서 걷은 그것).
            let dir = (is_group(i) && index.is_dir(all, at)).then(|| index.dir_path(all, at));
            let members = dir.as_ref().map(|p| index.descendants(p)).unwrap_or_default();
            let deferred = wh.deferred(i);
            let column = wh.column(i);
            Row {
                at,
                id: i.id.as_str(),
                kind: i.kind,
                planned: planned_of(i, &members, all, wh, shelved, zone),
                actual: actual_of(at, column, &members, all, wh, cfg, &starts, now_at),
                progress: match is_group(i) {
                    true => {
                        let (done, total) = dir.as_ref().map(|p| index.tally(p)).unwrap_or_default();
                        Standing::Rollup { done, total }
                    }
                    false => Standing::Column { name: column },
                },
                overdue: overdue_of(i, column, deferred, &today),
                deferred,
                blocked_by: blocks
                    .map(|b| b.of(i).into_iter().filter(|b| b.blocker.blocks()).collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    let range = range_of(&rows, now, zone);
    Gantt { rows, range }
}

/// 계획 구간 — 제 날이 있으면 그것, 없는 묶음은 멤버가 적은 날을 다 덮는 폭(가장 이른 날 ~ 가장 늦은 날).
///
/// 멤버는 **이 묶음의 계획에 선 일과 묶음**만 센다.
/// - **담아 둔 생각(backlog)은 계획이 아니다**(리뷰) — 이슈 밑에 접혀 묶음 안에 그려져도 멤버로 안 센다. 멤버의 셈
///   (`report::group_members`)과 이 줄의 진척·실제 구간이 다 그렇게 센다 — 세면 담아 둔 생각 하나가 막대를 몇 달 늘린다
/// - **미룬 멤버는 빼되, 묶음 제가 받은 미룸으로는 안 뺀다**(리뷰) — 보드가 묶음의 칸을 읽는 자(`report::counted`)와
///   같다. 미룸의 출처를 안 가리던 판은 미룬 에픽의 멤버를 다 빼, 흐리게 그릴 그 에픽의 막대가 통째로 사라졌다. 제 줄을
///   따로 미룬 멤버는 묶음이 미뤄졌어도 뺀다
///
/// 멤버 날은 그 멤버가 **제 줄에서** 읽은 계획([`own_plan`])만 본다 — 에픽 멤버의 에픽이 센 값을 다시 세면 같은 날이 두 번
/// 든다(마일스톤의 멤버에는 에픽도 이슈도 이미 들어 있다). **폭은 멤버의 막대를 다 덮는다** — 시작만 적은 멤버와 기한만
/// 적은 멤버가 섞여도 시작이 끝보다 늦지 않고, 멤버의 채운 시작(그 멤버를 만든 날)도 폭 안에 든다(리뷰 — 채운 시작을 안
/// 세던 판은 묶음보다 먼저 만든 멤버의 막대가 묶음의 막대 앞에 섰다). 멤버가 아무도 시작을 안 적었으면 묶음을 만든 날도
/// 견준다. 끝은 늘 멤버가 적은 날이다 — 채운 시작은 제 기한을 안 넘고, 채운 끝은 제 시작이다.
fn planned_of(
    i: &Issue,
    members: &[usize],
    all: &[Issue],
    wh: &Where<'_>,
    shelved: &BTreeMap<&str, Vec<&str>>,
    zone: &Zone,
) -> Option<Planned> {
    if let Some(own) = own_plan(i, zone) {
        return Some(own);
    }
    // 이 묶음을 계획에서 뺀 미룸들 — 멤버가 그것만으로 빠졌으면 이 묶음의 계획에 그대로 든다.
    let mine: &[&str] = shelved.get(i.id.as_str()).map(Vec::as_slice).unwrap_or_default();
    let parked_with_me = |m: &Issue| shelved.get(m.id.as_str()).is_some_and(|s| s.iter().all(|r| mine.contains(r)));
    let planned = members
        .iter()
        .map(|&m| &all[m])
        .filter(|m| is_work(m) || is_group(m))
        .filter(|m| !wh.deferred(m) || parked_with_me(m));
    // 날짜는 고정폭이라 글로 견준다(`Issue::validate_fields` 의 `DateOrder` 와 같은 자). `written` 은 멤버가 적은 날만이다 —
    // 묶음의 시작이 멤버가 적은 어느 날보다 이르면 그 시작은 채운 날이다.
    let (mut days, mut written, mut starts, mut ends) = (Vec::new(), Vec::new(), false, false);
    for p in planned.filter_map(|m| own_plan(m, zone)) {
        starts |= !p.start_filled;
        ends |= !p.end_filled;
        written.extend(
            [(!p.start_filled).then(|| p.start.clone()), (!p.end_filled).then(|| p.end.clone())].into_iter().flatten(),
        );
        days.extend([p.start, p.end]);
    }
    let first = days.iter().min()?.clone();
    let last = days.iter().max()?.clone();
    let start = if !starts && let Some(made) = made_day(i, zone) { made.min(first) } else { first };
    let start_filled = !starts || written.iter().min().is_none_or(|w| start < *w);
    Some(Planned { start, end: last, start_filled, end_filled: !ends, from: Origin::Members })
}

/// 줄을 만든 날 — 읽는 사람의 달로 옮긴 `created_at` 의 날짜다. 못 읽는 시각은 없다고 한다: 관대한 읽기가 받은 글을 그대로
/// 자르면 달력에 없는 날(`2026-02-30`)이 막대의 끝에 선다([`range_of`] 의 오늘과 같은 금).
fn made_day(i: &Issue, zone: &Zone) -> Option<String> {
    let at = format_rfc3339(zone.local(parse_rfc3339(&i.created_at)?));
    at.get(..10).filter(|d| parse_date(d).is_some()).map(str::to_string)
}

/// 제 계획의 빈 쪽을 채운다. 끌기 미리보기도 이 자로 재되, 쓰기는 제 날짜만 옮긴다.
///
/// - 빈 시작은 만든 날([`made_day`])이고, 기한이 그보다 이르면 기한 날이다. 만든 날을 못 읽으면 기한 날이다
/// - 빈 끝은 시작 날이다
/// - **채움 표시는 "적힌 날이 없다" 하나만 뜻한다** — 화면이 그것으로 쓸 쪽과 되돌릴 명령을 고른다(web 의 끌기·`undoOf`).
///   시작이 기한보다 늦게 적힌 옛 줄(쓰기는 거절하지만 읽기는 관대하다)도 시작은 적힌 날이다: 채웠다고 하면 되돌리기가
///   `--start none` 이 되어 사람이 적은 시작을 지운다. 거꾸로 선 막대는 그리는 쪽이 시작의 하루로 줄인다
pub fn own_plan(i: &Issue, zone: &Zone) -> Option<Planned> {
    let (start, end) = (i.planned_start(), i.deadline());
    let start: String = match (start, end) {
        (None, None) => return None,
        (Some(s), _) => s.into(),
        (None, Some(e)) => made_day(i, zone).map_or_else(|| e.into(), |made| made.as_str().min(e).into()),
    };
    Some(Planned {
        end: end.map_or_else(|| start.clone(), str::to_string),
        start,
        start_filled: i.planned_start().is_none(),
        end_filled: end.is_none(),
        from: Origin::Own,
    })
}

/// 실제 구간. 일은 제 `started_at` 으로, 묶음은 멤버(일)의 가장 이른 `started_at` 으로 잰다. `column` 은 그 줄이 서 있는
/// 칸이고(묶음이면 멤버에서 읽은 칸), `now` 는 한 번 파싱한 UTC 초다 — 앞날로 멀리 적힌 도장을 가르는 데만 쓴다.
fn actual_of<'a>(
    at: usize,
    column: &str,
    members: &[usize],
    all: &'a [Issue],
    wh: &'a Where<'a>,
    cfg: &Config,
    starts: &[Option<(&'a str, i64)>],
    now: Option<i64>,
) -> Actual<'a> {
    let i = &all[at];
    // 묶음이면 멤버의 일, 아니면 그 줄 하나 — 시작을 아는 것/모르는 것을 같은 걸음으로 센다.
    let work: Vec<usize> = match is_group(i) {
        true => members.iter().copied().filter(|&m| is_work(&all[m])).collect(),
        false => vec![at],
    };
    // **시작은 읽히는 것만 센다** — 못 읽는 글(손으로 푼 줄)과 앞날로 멀리 적힌 줄(`future_timestamp`, `stats::spans` 가
    // 재는 금과 같다)의 시작은 모르는 것이다. 시계 오타 하나가 2099년에 시작한 막대와 70년짜리 폭을 세우지 않게.
    // **떠난 일 가운데 하나라도 시작을 모르면 구간도 모른다**(리뷰) — 그 일이 더 일찍 시작했을 수 있어, 아는 시작 가운데
    // 가장 이른 것은 상한일 뿐이다. 그것을 시작으로 내면 모르는 것을 "그보다 늦지 않다" 로 채운 셈이다(필드 전에 닫힌
    // 멤버가 대부분인 에픽이 나중에 집은 멤버 하나 때문에 0 분짜리 구간으로 섰다). 줄 하나면 늘 그랬던 답이다.
    if work.iter().any(|&w| starts[w].is_none() && wh.column(&all[w]) != cfg.first_status()) {
        return Actual::Unknown;
    }
    // 시작을 아는 일이 없는데 떠난 일도 없으면 시작 전이다. 시각은 **읽은 값으로** 견준다 — 관대한 읽기가 받은 꼴
    // (`2026-02-30T…` 은 3월 2일이다)에서는 글의 차례가 때의 차례가 아니다.
    let Some((start, start_at)) = work.iter().filter_map(|&w| starts[w]).min_by_key(|(_, at)| *at) else {
        return Actual::NotStarted;
    };
    // 닫혔는가는 **서 있는 칸**으로 묻는다 — 묶음은 멤버에서 읽은 칸이고, 되돌린 줄의 `done_at` 은 안 센다.
    if column != DONE {
        return Actual::Span { start, end: None };
    }
    // 닫혔는데 끝이 시작보다 앞서거나, 못 읽거나, 앞날로 멀리 적혔으면 구간을 못 낸다 — 열린 구간으로 그리면 끝난 일이
    // 아직 도는 것처럼 선다.
    let ahead = |t: i64| now.is_some_and(|n| t - n > FUTURE_SLACK_SECS);
    let closed =
        super::stats::closed_at(i, wh).filter(|e| parse_rfc3339(e).is_some_and(|e| e >= start_at && !ahead(e)));
    match closed {
        Some(end) => Actual::Span { start, end: Some(end) },
        None => Actual::Unknown,
    }
}

/// 열린 줄의 **제** 기한이 지난 날수. 미룬 줄은 안 센다. `today` 는 읽는 사람의 달로 옮긴 지금이다(`zone.shift(now)`) —
/// 보드의 `overdue`(`Dues::late_on`)와 상세(`view::due_of`)가 재는 그 자라, 자정 무렵에도 셋이 같은 날을 댄다.
fn overdue_of(i: &Issue, column: &str, deferred: bool, today: &str) -> Option<i64> {
    if column == DONE || deferred {
        return None;
    }
    days_until(i.deadline()?, today).filter(|d| *d < 0).map(|d| -d)
}

/// 줄들의 계획·실제 가운데 가장 이른 날 ~ 가장 늦은 날 — 화면의 "맞춤" 이 쓴다. 열린 구간은 읽는 사람의 오늘까지다.
///
/// **어느 줄로 잴지는 부르는 쪽이 고른다**(리뷰) — [`Gantt::range`] 는 적재한 줄 전부로 잰 것이고, 목록을 거른 화면은
/// 남긴 줄만 넘겨 다시 잰다. 화면이 그 셈(열린 구간은 오늘까지, 시각은 UTC 의 날)을 따로 적으면 날짜를 판단하는 `if` 가
/// 화면에 선다. 실제의 날은 **읽은 시각에서** 낸다 — 관대한 읽기가 받은 `2026-02-30T…` 를 글대로 자르면 달력에 없는 날이
/// 폭의 끝에 선다.
pub fn range_of<'r, 'a: 'r>(rows: impl IntoIterator<Item = &'r Row<'a>>, now: &str, zone: &Zone) -> Option<Range> {
    let day = |t: &str| parse_rfc3339(t).map(|s| format_rfc3339(s)[..10].to_string());
    let today = zone.shift(now).get(..10).filter(|d| parse_date(d).is_some()).map(str::to_string);
    let mut days: Vec<String> = Vec::new();
    for r in rows {
        if let Some(p) = &r.planned {
            days.extend([p.start.clone(), p.end.clone()]);
        }
        if let Actual::Span { start, end } = &r.actual {
            days.extend(day(start));
            match end {
                Some(e) => days.extend(day(e)),
                None => days.extend(today.clone()),
            }
        }
    }
    let from = days.iter().min()?.clone();
    let to = days.iter().max()?.clone();
    Some(Range { from, to })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    const NOW: &str = "2026-10-20T03:00:00Z";

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn row(id: &str, kind: Kind, column: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new(column), "2026-10-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str, column: &str) -> Issue {
        let mut i = row(id, Kind::Issue, column);
        i.epic = Some(epic.into());
        i
    }

    /// 시험은 거르개를 새어 둔다 — 값이 거르개를 빌려 서므로 두 쪽이 한 범위에 살아야 하는데, 시험마다 그것을 적으면 읽을
    /// 것이 가려진다.
    fn read<'a>(all: &'a [Issue]) -> Gantt<'a> {
        read_in(all, NOW, &Zone::utc())
    }

    /// [`read`] 를 다른 시각·시간대로.
    fn read_in<'a>(all: &'a [Issue], now: &str, zone: &Zone) -> Gantt<'a> {
        let cfg: &'static Config = Box::leak(Box::new(cfg()));
        let wh: &'a Where<'a> = Box::leak(Box::new(Where::of(all, cfg)));
        let blocks = all.iter().any(|i| !i.blocked_by.is_empty()).then(|| BlockMaps::over(all, cfg));
        let shelved = super::super::deferred_sources(all);
        gantt(all, &Index::of(all), wh, cfg, blocks.as_ref(), &shelved, now, zone)
    }

    fn by<'g, 'a>(g: &'g Gantt<'a>, id: &str) -> &'g Row<'a> {
        g.rows.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{id} 줄이 없다"))
    }

    #[test]
    fn missing_plan_edges_are_filled_without_changing_written_dates() {
        let mut due = row("argos-0001", Kind::Issue, "todo");
        due.created_at = "2026-10-27T15:30:00Z".into();
        due.due_on = Some("2026-10-30".into());
        let mut past = due.clone();
        past.id = "argos-0002".into();
        past.due_on = Some("2026-10-20".into());
        let mut start = row("argos-0003", Kind::Issue, "todo");
        start.starts_on = Some("2026-10-22".into());
        let mut backwards = row("argos-0004", Kind::Issue, "todo");
        backwards.starts_on = Some("2026-10-25".into());
        backwards.due_on = Some("2026-10-23".into());
        let all = vec![due, past, start, backwards];
        let zone = Zone::fixed("Asia/Seoul", 9 * 3600);
        let g = read_in(&all, "2026-10-28T16:00:00Z", &zone);
        let p = |at: usize| g.rows[at].planned.as_ref().unwrap();
        assert_eq!(
            (&*p(0).start, &*p(0).end, p(0).start_filled, p(0).end_filled),
            ("2026-10-28", "2026-10-30", true, false)
        );
        assert_eq!(read(&all).rows[0].planned.as_ref().unwrap().start, "2026-10-27");
        assert_eq!((&*p(1).start, &*p(1).end), ("2026-10-20", "2026-10-20"));
        assert_eq!(
            (&*p(2).start, &*p(2).end, p(2).start_filled, p(2).end_filled),
            ("2026-10-22", "2026-10-22", false, true)
        );
        // 거꾸로 적힌 시작도 적힌 날이다 — 채웠다고 하면 web 의 되돌리기가 `--start none` 으로 그 시작을 지운다.
        assert_eq!((&*p(3).start, &*p(3).end, p(3).start_filled), ("2026-10-25", "2026-10-23", false));
    }

    /// **만든 날은 읽은 시각에서 낸다**(리뷰) — 관대한 읽기가 받은 `2026-02-30T…` 를 글대로 자르면 달력에 없는 날이 막대의
    /// 시작과 폭의 끝에 서고, TUI 는 그 막대와 맞춤을 통째로 잃는다. 못 읽는 시각이면 기한 날에서 선다.
    #[test]
    fn a_filled_start_is_a_calendar_day() {
        let mut odd = row("argos-0001", Kind::Issue, "todo");
        odd.created_at = "2026-02-30T00:00:00Z".into();
        odd.due_on = Some("2026-10-30".into());
        let mut garbage = row("argos-0002", Kind::Issue, "todo");
        garbage.created_at = "어제".into();
        garbage.due_on = Some("2026-10-30".into());
        let all = vec![odd, garbage];
        let g = read(&all);
        assert_eq!(g.rows[0].planned.as_ref().unwrap().start, "2026-03-02");
        assert_eq!(g.rows[1].planned.as_ref().unwrap().start, "2026-10-30");
        assert_eq!(g.range, Some(Range { from: "2026-03-02".into(), to: "2026-10-30".into() }));
    }

    /// **묶음의 폭은 멤버의 채운 시작도 덮는다**(리뷰) — 묶음보다 먼저 만든 멤버가 기한만 적었으면 그 막대는 만든 날에서
    /// 서는데, 적힌 날만 세면 묶음의 막대가 그보다 늦게 서서 멤버가 묶음 밖에 그려진다.
    #[test]
    fn a_groups_span_covers_its_members_filled_starts() {
        let mut epic = row("argos-0001", Kind::Epic, "todo");
        epic.created_at = "2026-10-20T00:00:00Z".into();
        let mut old = member("argos-0001.aaa", "argos-0001", "todo"); // 2026-10-01 에 만든 줄
        old.due_on = Some("2026-10-25".into());
        let mut late = member("argos-0001.bbb", "argos-0001", "todo");
        late.starts_on = Some("2026-10-22".into());
        let all = vec![epic, old, late];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001.aaa").planned.as_ref().unwrap().start, "2026-10-01");
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-25".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            }),
            "멤버의 채운 시작이 묶음의 막대 밖에 섰다"
        );
    }

    #[test]
    fn a_group_fills_only_edges_no_member_wrote() {
        let mut epic = row("argos-0001", Kind::Epic, "todo");
        epic.created_at = "2026-10-29T00:00:00Z".into();
        let mut m = member("argos-0001.aaa", "argos-0001", "todo");
        m.due_on = Some("2026-10-23".into());
        let mut all = vec![epic, m];
        let g = read(&all);
        let p = g.rows[0].planned.as_ref().unwrap();
        // 멤버(2026-10-01 에 만든 줄)의 막대가 만든 날에서 서므로 묶음의 폭도 거기서 선다.
        assert_eq!((&*p.start, &*p.end, p.start_filled, p.end_filled), ("2026-10-01", "2026-10-23", true, false));
        all[1].created_at = "2026-10-30T00:00:00Z".into();
        let g = read(&all);
        let p = g.rows[0].planned.as_ref().unwrap();
        assert_eq!((&*p.start, &*p.end, p.start_filled, p.end_filled), ("2026-10-23", "2026-10-23", true, false));
        all[1].due_on = None;
        all[1].starts_on = Some("2026-10-22".into());
        let g = read(&all);
        let p = g.rows[0].planned.as_ref().unwrap();
        assert_eq!((&*p.start, &*p.end, p.start_filled, p.end_filled), ("2026-10-22", "2026-10-22", false, true));
    }

    /// 계획만 · 실제만 · 둘 다 · 아무것도 없는 일 하나씩.
    #[test]
    fn a_row_reads_its_plan_and_its_run_apart() {
        let mut planned = row("argos-0001", Kind::Issue, "todo");
        (planned.starts_on, planned.due_on) = (Some("2026-10-22".into()), Some("2026-10-30".into()));
        let mut ran = row("argos-0002", Kind::Issue, "in_progress");
        ran.started_at = Some("2026-10-10T01:00:00Z".into());
        let mut both = row("argos-0003", Kind::Issue, "done");
        both.due_on = Some("2026-10-18".into());
        both.started_at = Some("2026-10-12T01:00:00Z".into());
        both.status_since = "2026-10-16T05:00:00Z".into();
        both.done_at = Some("2026-10-16T05:00:00Z".into());
        let bare = row("argos-0004", Kind::Issue, "todo");
        let all = vec![planned, ran, both, bare];
        let g = read(&all);

        let a = by(&g, "argos-0001");
        assert_eq!(
            a.planned,
            Some(Planned {
                start: "2026-10-22".into(),
                end: "2026-10-30".into(),
                start_filled: false,
                end_filled: false,
                from: Origin::Own
            })
        );
        assert_eq!(a.actual, Actual::NotStarted);
        assert_eq!(a.progress, Standing::Column { name: "todo" });

        let b = by(&g, "argos-0002");
        assert_eq!(b.planned, None);
        // 닫히지 않았으니 열려 있다 — 끝이 없다.
        assert_eq!(b.actual, Actual::Span { start: "2026-10-10T01:00:00Z", end: None });

        let c = by(&g, "argos-0003");
        assert_eq!(
            c.planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-18".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Own
            })
        );
        assert_eq!(c.actual, Actual::Span { start: "2026-10-12T01:00:00Z", end: Some("2026-10-16T05:00:00Z") });
        assert_eq!(c.overdue, None, "닫힌 줄은 기한을 넘기지 않는다");

        let d = by(&g, "argos-0004");
        assert_eq!(
            (d.planned.clone(), d.actual.clone(), d.overdue, d.deferred),
            (None, Actual::NotStarted, None, false)
        );
        assert!(d.blocked_by.is_empty());
        // 폭은 모든 줄의 가장 이른 날에서 가장 늦은 날까지 — 열린 줄(b)은 오늘까지다.
        assert_eq!(g.range, Some(Range { from: "2026-10-01".into(), to: "2026-10-30".into() }));
    }

    /// 시작을 모르는 줄은 구간이 없고 "시작 모름" 이 따로 선다 — 만든 때도 0 분도 아니다.
    #[test]
    fn an_unknown_start_is_not_a_zero_and_not_the_creation_time() {
        // 첫 칸을 떠났는데 `started_at` 이 없는 줄(필드 전에 집은 줄)과, 닫혔는데 없는 줄.
        let moved = row("argos-0001", Kind::Issue, "in_progress");
        let mut shut = row("argos-0002", Kind::Issue, "done");
        shut.done_at = Some("2026-10-05T00:00:00Z".into());
        shut.status_since = "2026-10-05T00:00:00Z".into();
        let fresh = row("argos-0003", Kind::Issue, "todo");
        // 읽히지 않는 시작도 모른다.
        let mut junk = row("argos-0004", Kind::Issue, "in_progress");
        junk.started_at = Some("어제".into());
        let all = vec![moved, shut, fresh, junk];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Unknown);
        assert_eq!(by(&g, "argos-0002").actual, Actual::Unknown);
        assert_eq!(by(&g, "argos-0003").actual, Actual::NotStarted);
        assert_eq!(by(&g, "argos-0004").actual, Actual::Unknown);
        // 날이 하나도 없으면 폭도 없다 — 만든 때(2026-10-01)로 채우지 않는다.
        assert_eq!(g.range, None);
    }

    /// 되돌린 줄은 `done_at` 이 남아도 지금 열려 있다 — 끝이 없다. 시작이 끝보다 늦은 줄은 모른다.
    #[test]
    fn a_reopened_row_is_open_and_a_backwards_one_is_unknown() {
        let mut reopened = row("argos-0001", Kind::Issue, "in_progress");
        reopened.started_at = Some("2026-10-10T00:00:00Z".into());
        reopened.done_at = Some("2026-10-12T00:00:00Z".into());
        let mut backwards = row("argos-0002", Kind::Issue, "done");
        backwards.started_at = Some("2026-10-15T00:00:00Z".into());
        backwards.done_at = Some("2026-10-12T00:00:00Z".into());
        backwards.status_since = "2026-10-12T00:00:00Z".into();
        let all = vec![reopened, backwards];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Span { start: "2026-10-10T00:00:00Z", end: None });
        assert_eq!(by(&g, "argos-0002").actual, Actual::Unknown, "끝이 시작보다 앞선 구간을 냈다");
    }

    /// 묶음의 계획은 제 값이 있으면 그것, 없으면 멤버에서 센다 — 어느 쪽인지 함께 낸다.
    #[test]
    fn a_group_plans_from_its_own_dates_or_from_its_members() {
        let mut own = row("argos-0001", Kind::Epic, "todo");
        own.due_on = Some("2026-11-30".into());
        let mut a = member("argos-0001.aaa", "argos-0001", "todo");
        (a.starts_on, a.due_on) = (Some("2026-10-25".into()), Some("2026-11-05".into()));

        let counted = row("argos-0002", Kind::Epic, "todo");
        let mut b = member("argos-0002.bbb", "argos-0002", "todo");
        (b.starts_on, b.due_on) = (Some("2026-10-22".into()), Some("2026-10-28".into()));
        let mut c = member("argos-0002.ccc", "argos-0002", "todo");
        (c.starts_on, c.due_on) = (Some("2026-10-24".into()), Some("2026-11-10".into()));
        let d = member("argos-0002.ddd", "argos-0002", "todo"); // 날이 없는 멤버는 폭을 안 건드린다

        let empty = row("argos-0003", Kind::Epic, "todo");
        let all = vec![own, a, counted, b, c, d, empty];
        let g = read(&all);

        // 제 값은 한 덩이다 — 기한만 적었으면 시작을 멤버(2026-10-25)에서 채우지 않는다.
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-11-30".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Own
            })
        );
        assert_eq!(
            by(&g, "argos-0002").planned,
            Some(Planned {
                start: "2026-10-22".into(),
                end: "2026-11-10".into(),
                start_filled: false,
                end_filled: false,
                from: Origin::Members
            })
        );
        assert_eq!(by(&g, "argos-0003").planned, None);
        // 묶음의 진척은 목록 줄이 세는 그 수다.
        assert_eq!(by(&g, "argos-0002").progress, Standing::Rollup { done: 0, total: 3 });
        assert_eq!(by(&g, "argos-0003").progress, Standing::Rollup { done: 0, total: 0 });
    }

    /// 마일스톤의 멤버에는 에픽도 이슈도 든다 — 에픽의 날도, 에픽 밑 이슈의 날도 센다. 같은 날을 두 번 세도 값은 같다.
    #[test]
    fn a_milestone_reads_the_epics_and_issues_under_it() {
        let mut stone = row("argos-m001", Kind::Milestone, "todo");
        stone.due_on = None;
        let mut epic = row("argos-e001", Kind::Epic, "todo");
        epic.milestone = Some("argos-m001".into());
        epic.due_on = Some("2026-12-01".into());
        let mut a = member("argos-e001.aaa", "argos-e001", "todo");
        a.starts_on = Some("2026-10-27".into());
        let all = vec![stone, epic, a];
        let g = read(&all);
        // 에픽이 기한만 적었으니 그 막대는 만든 날(2026-10-01)에서 선다 — 마일스톤의 폭이 그것까지 덮는다.
        assert_eq!(
            by(&g, "argos-m001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-12-01".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            })
        );
    }

    /// 묶음의 실제는 멤버로 잰다 — 묶음 줄 자신의 `started_at` 은 안 쓴다. 닫히면 닫힌 때에서 끝난다.
    #[test]
    fn a_groups_run_is_its_members_not_its_own_stamp() {
        let mut epic = row("argos-0001", Kind::Epic, "in_progress");
        epic.started_at = Some("2026-09-01T00:00:00Z".into()); // 아무도 안 읽는 값
        let mut a = member("argos-0001.aaa", "argos-0001", "done");
        a.started_at = Some("2026-10-12T00:00:00Z".into());
        a.status_since = "2026-10-14T00:00:00Z".into();
        a.done_at = Some("2026-10-14T00:00:00Z".into());
        let mut b = member("argos-0001.bbb", "argos-0001", "in_progress");
        b.started_at = Some("2026-10-10T00:00:00Z".into());
        let all = vec![epic, a, b];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Span { start: "2026-10-10T00:00:00Z", end: None });
        assert_eq!(by(&g, "argos-0001").progress, Standing::Rollup { done: 1, total: 2 });

        // 마지막 멤버가 닫히면 묶음이 닫힌다 — 끝은 마지막으로 닫힌 때다.
        let mut all = all;
        all[2].status = Status::new("done");
        all[2].status_since = "2026-10-18T00:00:00Z".into();
        all[2].done_at = Some("2026-10-18T00:00:00Z".into());
        let g = read(&all);
        assert_eq!(
            by(&g, "argos-0001").actual,
            Actual::Span { start: "2026-10-10T00:00:00Z", end: Some("2026-10-18T00:00:00Z") }
        );
        assert_eq!(by(&g, "argos-0001").progress, Standing::Rollup { done: 2, total: 2 });
    }

    /// 묶음의 멤버가 시작을 하나도 안 적었다 — 떠난 멤버가 있으면 "모름", 아무도 안 떠났으면 시작 전이다.
    #[test]
    fn a_group_with_no_known_start_says_so() {
        let epic = row("argos-0001", Kind::Epic, "todo");
        let a = member("argos-0001.aaa", "argos-0001", "in_progress"); // 떠났는데 시작이 없다
        let other = row("argos-0002", Kind::Epic, "todo");
        let b = member("argos-0002.bbb", "argos-0002", "todo");
        let all = vec![epic, a, other, b];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Unknown);
        assert_eq!(by(&g, "argos-0002").actual, Actual::NotStarted);
    }

    /// 미룬 멤버의 날은 묶음의 계획이 아니다 — 센 구간에서 빠지고, 그 줄은 흐리게 그리라고 `deferred` 가 선다. 미룬 줄은
    /// 기한이 지나도 늦었다고 안 센다(보드의 `overdue` 와 같은 자).
    #[test]
    fn a_deferred_member_leaves_the_plan_and_is_never_late() {
        let epic = row("argos-0001", Kind::Epic, "todo");
        let mut a = member("argos-0001.aaa", "argos-0001", "todo");
        a.due_on = Some("2026-10-30".into());
        let mut parked = member("argos-0001.bbb", "argos-0001", "todo");
        (parked.starts_on, parked.due_on) = (Some("2026-09-01".into()), Some("2026-10-01".into()));
        parked.deferred_at = Some("2026-10-05T00:00:00Z".into());
        let all = vec![epic, a, parked];
        let g = read(&all);
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-30".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            }),
            "미룬 멤버의 날이 묶음의 계획을 늘렸다"
        );
        let p = by(&g, "argos-0001.bbb");
        assert!(p.deferred && !by(&g, "argos-0001.aaa").deferred);
        assert_eq!(p.overdue, None, "미룬 줄을 늦었다고 셌다");
        // 묶음을 미루면 멤버도 물려받아 미룬다.
        let mut all = all;
        all[0].deferred_at = Some("2026-10-06T00:00:00Z".into());
        let g = read(&all);
        assert!(by(&g, "argos-0001.aaa").deferred, "미룸이 멤버에게 안 내려갔다");
        // **묶음 제가 받은 미룸으로는 멤버를 안 뺀다**(리뷰) — 흐리게 그릴 미룬 에픽의 막대가 그대로 선다. 제 줄을 따로
        // 미룬 멤버는 여전히 빠진다(`report::counted` 와 같은 자).
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-30".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            }),
            "미룬 에픽이 제 막대를 잃었거나 따로 미룬 멤버를 셌다"
        );
    }

    /// **담아 둔 생각은 묶음의 계획이 아니다**(리뷰) — 멤버 이슈 밑에 접혀 에픽 안에 그려져도 그 날이 에픽의 막대를
    /// 안 늘린다. 진척·실제 구간이 일만 세는 것과 같다. 그 생각의 줄은 제 날로 제 막대를 든다.
    #[test]
    fn a_parked_thought_does_not_stretch_its_groups_plan() {
        let epic = row("argos-0001", Kind::Epic, "todo");
        let mut a = member("argos-0001.aaa", "argos-0001", "todo");
        (a.starts_on, a.due_on) = (Some("2026-10-22".into()), Some("2026-10-28".into()));
        let mut idea = row("argos-0001.aaa.bbb", Kind::Backlog, "todo");
        (idea.starts_on, idea.due_on) = (Some("2027-05-01".into()), Some("2027-06-01".into()));
        let all = vec![epic, a, idea];
        // 목록은 그 생각을 에픽 밑에 그린다 — 그래야 이 시험이 무엇을 재는 것이다.
        let index = Index::of(&all);
        assert!(index.descendants(&index.dir_path(&all, 0)).contains(&2), "생각이 에픽 밑에 안 그려졌다");
        let g = read(&all);
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-22".into(),
                end: "2026-10-28".into(),
                start_filled: false,
                end_filled: false,
                from: Origin::Members
            })
        );
        assert_eq!(
            by(&g, "argos-0001.aaa.bbb").planned,
            Some(Planned {
                start: "2027-05-01".into(),
                end: "2027-06-01".into(),
                start_filled: false,
                end_filled: false,
                from: Origin::Own
            })
        );
    }

    /// **멤버에서 센 폭은 거꾸로 서지 않는다**(리뷰) — 시작만 적은 멤버와 그보다 이른 기한만 적은 멤버가 섞이면, 시작과
    /// 기한을 따로 재던 판은 끝이 시작보다 앞선 막대를 냈다. 폭은 멤버가 적은 날을 다 덮는다.
    #[test]
    fn a_groups_counted_span_covers_every_member_date() {
        let epic = row("argos-0001", Kind::Epic, "todo");
        let mut late_start = member("argos-0001.aaa", "argos-0001", "todo");
        late_start.starts_on = Some("2026-10-27".into());
        let mut early_due = member("argos-0001.bbb", "argos-0001", "todo");
        early_due.due_on = Some("2026-10-22".into());
        let other = row("argos-0002", Kind::Epic, "todo");
        let mut x = member("argos-0002.aaa", "argos-0002", "todo");
        (x.starts_on, x.due_on) = (Some("2026-10-20".into()), Some("2026-10-25".into()));
        let mut y = member("argos-0002.bbb", "argos-0002", "todo");
        y.starts_on = Some("2026-11-01".into());
        let all = vec![epic, late_start, early_due, other, x, y];
        let g = read(&all);
        // 기한만 적은 멤버(2026-10-01 에 만든 줄)의 막대는 만든 날에서 선다 — 폭이 그것까지 덮는다.
        assert_eq!(
            by(&g, "argos-0001").planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-27".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            })
        );
        assert_eq!(
            by(&g, "argos-0002").planned,
            Some(Planned {
                start: "2026-10-20".into(),
                end: "2026-11-01".into(),
                start_filled: false,
                end_filled: false,
                from: Origin::Members
            }),
            "멤버의 시작이 묶음의 막대 밖에 섰다"
        );
    }

    /// **떠난 멤버 하나라도 시작을 모르면 묶음의 구간도 모른다**(리뷰) — 아는 시작 가운데 가장 이른 것은 상한일 뿐이다.
    #[test]
    fn a_group_with_one_unknown_start_is_unknown() {
        let epic = row("argos-0001", Kind::Epic, "in_progress");
        let mut old = member("argos-0001.aaa", "argos-0001", "done"); // 필드 전에 닫힌 줄 — 시작이 없다
        (old.status_since, old.done_at) = ("2026-09-12T00:00:00Z".into(), Some("2026-09-12T00:00:00Z".into()));
        let mut new = member("argos-0001.bbb", "argos-0001", "in_progress");
        new.started_at = Some("2026-10-10T00:00:00Z".into());
        let all = vec![epic, old, new];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Unknown, "모르는 시작을 아는 시작으로 채웠다");
        assert_eq!(by(&g, "argos-0001.bbb").actual, Actual::Span { start: "2026-10-10T00:00:00Z", end: None });
    }

    /// **앞날로 멀리 적힌 도장은 모르는 것이다**(리뷰) — `moai stats` 가 재는 금(`future_timestamp`)과 같다. 시계 오타
    /// 하나가 2099년에 시작한 막대와 70년짜리 폭을 세우지 않는다.
    #[test]
    fn a_far_future_stamp_is_unknown_and_stays_out_of_the_range() {
        let mut typo = row("argos-0001", Kind::Issue, "in_progress");
        typo.started_at = Some("2099-01-01T00:00:00Z".into());
        let mut fine = row("argos-0002", Kind::Issue, "in_progress");
        fine.started_at = Some("2026-10-10T00:00:00Z".into());
        let all = vec![typo, fine];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").actual, Actual::Unknown);
        assert_eq!(g.range, Some(Range { from: "2026-10-10".into(), to: "2026-10-20".into() }));
    }

    /// **늦었는가와 오늘은 읽는 사람의 달로 잰다**(리뷰, moai-h2th) — 보드의 `overdue` 와 상세가 재는 자와 같다. 서울의
    /// 새벽 1시는 UTC 로 아직 어제라, UTC 로 재면 하루 지난 줄이 안 늦었다고 선다.
    #[test]
    fn overdue_and_today_are_the_readers_day() {
        let now = "2026-10-20T16:00:00Z"; // 서울은 10-21 01:00
        let mut due = row("argos-0001", Kind::Issue, "in_progress");
        due.due_on = Some("2026-10-20".into());
        due.started_at = Some("2026-10-18T00:00:00Z".into());
        let all = vec![due];
        let seoul = Zone::fixed("Asia/Seoul", 9 * 3600);
        let g = read_in(&all, now, &seoul);
        assert_eq!(g.rows[0].overdue, Some(1), "읽는 사람의 오늘(10-21)로 재지 않았다");
        assert_eq!(g.range.as_ref().map(|r| r.to.as_str()), Some("2026-10-21"), "열린 구간의 오늘이 UTC 의 날이다");
        let g = read_in(&all, now, &Zone::utc());
        assert_eq!(g.rows[0].overdue, None, "UTC 의 오늘은 기한 당일이다");
    }

    /// **같은 id 의 앞줄은 멤버를 거느리지 않는다**(리뷰, moai-sfml) — 목록이 그 줄을 잎으로 그리므로, 자리로만 물어 뒷줄의
    /// 멤버·셈을 입히면 같은 막대가 둘 선다.
    #[test]
    fn the_front_twin_of_a_group_holds_no_members() {
        let front = row("argos-0001", Kind::Epic, "todo");
        let back = row("argos-0001", Kind::Epic, "todo");
        let mut m = member("argos-0001.aaa", "argos-0001", "todo");
        m.due_on = Some("2026-10-30".into());
        let all = vec![front, back, m];
        let g = read(&all);
        assert_eq!((&g.rows[0].progress, &g.rows[0].planned), (&Standing::Rollup { done: 0, total: 0 }, &None));
        assert_eq!(g.rows[1].progress, Standing::Rollup { done: 0, total: 1 });
        assert_eq!(
            g.rows[1].planned,
            Some(Planned {
                start: "2026-10-01".into(),
                end: "2026-10-30".into(),
                start_filled: true,
                end_filled: false,
                from: Origin::Members
            })
        );
    }

    /// **맞춤의 폭은 부르는 쪽이 고른 줄로도 잰다**(리뷰) — 숨긴 줄의 날이 폭을 넓히지 않게, 거른 화면은 남긴 줄만 넘긴다.
    #[test]
    fn the_range_can_be_measured_over_the_rows_a_screen_keeps() {
        let mut old = row("argos-0001", Kind::Issue, "todo");
        (old.starts_on, old.due_on) = (Some("2025-10-01".into()), Some("2025-10-05".into()));
        let mut now_ = row("argos-0002", Kind::Issue, "todo");
        (now_.starts_on, now_.due_on) = (Some("2026-10-22".into()), Some("2026-10-30".into()));
        let all = vec![old, now_];
        let g = read(&all);
        assert_eq!(g.range, Some(Range { from: "2025-10-01".into(), to: "2026-10-30".into() }));
        let kept = range_of(g.rows.iter().filter(|r| r.id == "argos-0002"), NOW, &Zone::utc());
        assert_eq!(kept, Some(Range { from: "2026-10-22".into(), to: "2026-10-30".into() }));
    }

    /// 열린 줄의 제 기한이 지났으면 지난 날수를 낸다 — 당일은 안 지났다. 닫힌 줄은 안 센다. 묶음은 제 기한만 본다.
    #[test]
    fn an_open_row_past_its_own_deadline_is_late_by_whole_days() {
        let mut late = row("argos-0001", Kind::Issue, "in_progress");
        late.due_on = Some("2026-10-17".into()); // NOW 는 10-20 03:00 UTC — 사흘 지났다
        let mut today = row("argos-0002", Kind::Issue, "todo");
        today.due_on = Some("2026-10-20".into());
        let mut shut = row("argos-0003", Kind::Issue, "done");
        shut.due_on = Some("2026-10-01".into());
        let mut epic = row("argos-0004", Kind::Epic, "todo");
        let mut slow = member("argos-0004.aaa", "argos-0004", "todo");
        slow.due_on = Some("2026-10-02".into());
        epic.due_on = None;
        let all = vec![late, today, shut, epic, slow];
        let g = read(&all);
        assert_eq!(by(&g, "argos-0001").overdue, Some(3));
        assert_eq!(by(&g, "argos-0002").overdue, None, "기한 당일을 늦었다고 셌다");
        assert_eq!(by(&g, "argos-0003").overdue, None);
        assert_eq!(by(&g, "argos-0004").overdue, None, "묶음이 멤버의 기한으로 늦었다고 섰다");
        assert_eq!(by(&g, "argos-0004.aaa").overdue, Some(18));
    }

    /// 지금 막고 있는 막음만 낸다 — 풀린 막음(done)과 없는 id 는 안 든다. 막는 줄이 열려 있으면 그 상태가 선다.
    #[test]
    fn only_what_blocks_now_is_listed() {
        let open = row("argos-0001", Kind::Issue, "in_progress");
        let shut = row("argos-0002", Kind::Issue, "done");
        let mut blocked = row("argos-0003", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0001".into(), "argos-0002".into(), "argos-zzzz".into()];
        let all = vec![open, shut, blocked];
        let g = read(&all);
        let ids: Vec<(&str, super::super::Blocker)> =
            by(&g, "argos-0003").blocked_by.iter().map(|b| (b.id, b.blocker)).collect();
        assert_eq!(ids, vec![("argos-0001", super::super::Blocker::Open)]);
        assert!(by(&g, "argos-0001").blocked_by.is_empty());
    }

    /// 값은 그대로 기계 출력이 된다 — web 이 같은 값을 읽는다(moai-d86e.b2m). 갈래는 `kind` 로 서고 없는 쪽은 `null` 이다.
    #[test]
    fn the_value_serialises_for_a_machine_surface() {
        let mut a = row("argos-0001", Kind::Issue, "in_progress");
        a.started_at = Some("2026-10-10T01:00:00Z".into());
        a.due_on = Some("2026-10-30".into());
        let e = row("argos-0002", Kind::Epic, "todo");
        let all = vec![a, e];
        let v = serde_json::to_value(read(&all)).unwrap();
        let first = &v["rows"][0];
        assert_eq!(first["actual"], serde_json::json!({"kind": "span", "start": "2026-10-10T01:00:00Z", "end": null}));
        assert_eq!(
            first["planned"],
            serde_json::json!({"start": "2026-10-01", "end": "2026-10-30", "start_filled": true, "end_filled": false, "from": "own"})
        );
        assert_eq!(first["progress"], serde_json::json!({"kind": "column", "name": "in_progress"}));
        assert_eq!(v["rows"][1]["progress"], serde_json::json!({"kind": "rollup", "done": 0, "total": 0}));
        assert_eq!(v["range"], serde_json::json!({"from": "2026-10-01", "to": "2026-10-30"}));
    }

    /// 행은 `all` 과 같은 차례·같은 길이고, 같은 id 를 든 줄도 첨자로 가려진다.
    #[test]
    fn rows_follow_the_input_order() {
        let all = vec![row("argos-0002", Kind::Issue, "todo"), row("argos-0001", Kind::Issue, "todo")];
        let g = read(&all);
        assert_eq!(g.rows.iter().map(|r| (r.at, r.id)).collect::<Vec<_>>(), vec![(0, "argos-0002"), (1, "argos-0001")]);
    }
}
