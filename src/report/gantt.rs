//! 간트 — 줄마다 **계획한 구간과 실제 구간**을 읽는다(moai-xf0p.j1r). **순수 함수다** — 줄과 지금 시각, 부르는 쪽이
//! 이미 만든 색인·거름망만 본다. 아무것도 찍지 않고 파일도 열지 않는다.
//!
//! **화면은 이 값을 그리기만 한다.** TUI 도 web 도 날짜를 판단하는 `if` 를 갖지 않는다(CLAUDE.md "`report` 와 `query`
//! 는 순수 함수다") — 계획이 어디서 왔는지, 시작을 아는지, 늦었는지는 여기서 정하고, 화면은 날짜 칸에 놓는 일만 한다.
//!
//! # 읽는 법
//!
//! - **계획 구간**([`Planned`]) — 제 `starts_on`·`due_on`. 묶음(에픽·마일스톤)이 제 날을 안 적었으면 **멤버의 가장
//!   이른 시작 ~ 가장 늦은 기한**이다. 어느 쪽에서 왔는지 [`Origin`] 이 말한다 — 화면이 "멤버에서 센 값" 을 다르게
//!   그린다. 제 날은 **한 덩이다**: 시작이든 기한이든 하나라도 적었으면 그 줄의 구간은 그 줄의 것이고, 빈 쪽을
//!   멤버로 채우지 않는다(채우면 사람이 적은 구간과 센 구간이 한 막대에 섞인다). 저장하지 않는다 — 묶음의 구간은 파생값
//!   이라 읽을 때 멤버에서 센다(CLAUDE.md "파생값은 저장하지 않는다")
//! - **실제 구간**([`Actual`]) — `started_at` ~ (닫혔으면 닫힌 때, 아니면 열림 = 지금까지). **되돌린 줄은 열려
//!   있다**: `done_at` 이 남아도 지금 done 이 아니다([`super::stats::closed_at`] 와 같은 자). 묶음은 **멤버로 잰다** —
//!   멤버의 가장 이른 `started_at` 부터이고, 묶음 줄 자신의 `started_at` 은 안 쓴다(`Issue::started_at` 은 "누가 그
//!   줄에 `mv` 를 쳤나" 일 뿐이다)
//! - **모르는 것은 0 이 아니다** — `started_at` 이 없으면(이 필드 전에 집은 줄·옛 바이너리가 옮긴 줄) 구간이 없고,
//!   "시작 모름" 을 [`Actual::Unknown`] 으로 따로 낸다. 만든 때로도 0 분으로도 채우지 않는다. 끝이 시작보다 앞선 줄도
//!   모른다고 한다(`spent_in` 의 "음수는 안 센다" 와 같은 금)
//! - **시간대는 안 옮긴다.** 저장값은 UTC 라 [`Actual`] 은 UTC 시각을 그대로 내고, 날짜 칸에 놓는 것은 화면이
//!   `tz::Zone` 으로 옮긴다. 계획의 날은 달력 날짜라 옮기지 않는다
//!
//! # 어디서 가져오나
//!
//! 새로 짠 규칙이 없다. 묶음의 멤버·진척은 목록이 그리는 그대로(`nav::Index::descendants`·`tally`), 칸·닫힌 때·미룸은
//! 거르개가 읽는 그대로(`query::Where`), 막음은 상세가 그리는 그대로([`super::BlockMaps`]) 부른다. 이 모듈은 그것들을
//! 줄마다 한 값으로 모을 뿐이다.

// **부르는 표면은 이 에픽 밖에서 온다**(TUI 의 간트 위젯, web 은 moai-d86e.b2m) — 그 전까지 바이너리에는 부르는 곳이 없어
// `dead_code` 가 운다. 표면이 이 값을 읽는 날 이 줄을 걷는다. 시험은 이미 이 함수를 돈다.
#![allow(dead_code)]

use super::{Block, BlockMaps, is_group, is_work};
use crate::config::{Config, DONE};
use crate::model::{Issue, Kind, days_until, parse_rfc3339};
use crate::nav::Index;
use crate::query::Where;
use serde::Serialize;

/// 계획 구간이 어디서 왔나.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// 그 줄이 적은 `starts_on`·`due_on`.
    Own,
    /// 묶음이 제 날을 안 적어 멤버에서 센 값.
    Members,
}

/// 계획한 구간. 달력 날짜(`YYYY-MM-DD`)고, **한쪽만 있을 수 있다** — 기한만 적은 줄이 보통이다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Planned<'a> {
    pub start: Option<&'a str>,
    pub end: Option<&'a str>,
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
    pub at: usize,
    pub id: &'a str,
    pub kind: Kind,
    /// 계획한 구간. 날을 하나도 못 읽으면 없다.
    pub planned: Option<Planned<'a>>,
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

/// 화면에 놓을 날의 폭 — 모든 줄의 계획·실제 가운데 가장 이른 날부터 가장 늦은 날까지(`YYYY-MM-DD`). 화면의 "맞춤" 이 쓴다.
/// 실제는 UTC 의 날이다 — 시간대로 옮기면 하루 어긋날 수 있어 화면이 여백을 둔다. 열린 구간은 오늘까지다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Range {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gantt<'a> {
    /// `all` 와 같은 차례, 같은 길이다 — `rows[at].at == at`. 화면이 목록을 거르고 정하는 것은 제 일이다.
    pub rows: Vec<Row<'a>>,
    /// 날이 하나도 없으면 없다.
    pub range: Option<Range>,
}

/// 줄마다 계획·실제 구간을 읽는다. `index` 는 `all` 로 지은 색인(`nav::Index::of`)이고 `wh` 는 같은 줄로 지은 거르개
/// 문맥이다 — 탐색기는 적재 때 둘 다 이미 들고 있어 여기서 다시 짓지 않는다. `now` 는 RFC3339(UTC)다. 거르개를 `'a` 로
/// 빌리는 것은 서 있는 칸·닫힌 때가 거기서 빌려 온 글이라서다 — 값(`Gantt`)이 사는 동안 거르개도 산다.
pub fn gantt<'a>(all: &'a [Issue], index: &Index, wh: &'a Where<'a>, cfg: &Config, now: &str) -> Gantt<'a> {
    // 막음 지도는 막힌 줄이 있을 때만 짓는다 — 저장소 전부를 훑는 셈이다.
    let blocks = all.iter().any(|i| !i.blocked_by.is_empty()).then(|| BlockMaps::over(all, cfg));
    let rows: Vec<Row<'a>> = all
        .iter()
        .enumerate()
        .map(|(at, i)| {
            let members = members_of(all, index, at);
            let deferred = wh.deferred(i);
            let column = wh.column(i);
            Row {
                at,
                id: i.id.as_str(),
                kind: i.kind,
                planned: planned_of(i, &members, all, wh),
                actual: actual_of(i, &members, all, wh, cfg),
                progress: match is_group(i) {
                    true => {
                        let (done, total) = index.tally(&index.dir_path(all, at));
                        Standing::Rollup { done, total }
                    }
                    false => Standing::Column { name: column },
                },
                overdue: overdue_of(i, column, deferred, now),
                deferred,
                blocked_by: blocks
                    .as_ref()
                    .map(|b| b.of(i).into_iter().filter(|b| b.blocker.blocks()).collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    let range = range_of(&rows, now);
    Gantt { rows, range }
}

/// 묶음 밑에 그려진 줄의 첨자 — 목록이 그리는 그대로다. 묶음이 아니면 비었다.
fn members_of(all: &[Issue], index: &Index, at: usize) -> Vec<usize> {
    match is_group(&all[at]) {
        true => index.descendants(&index.dir_path(all, at)),
        false => Vec::new(),
    }
}

/// 계획 구간 — 제 날이 있으면 그것, 없는 묶음은 멤버의 가장 이른 시작 ~ 가장 늦은 기한.
///
/// 멤버는 **계획에 선 줄**만 센다: 미룬 멤버의 날은 이 묶음의 계획이 아니라서, 세면 안 하기로 한 일이 막대를 늘린다.
/// 멤버 날은 그 멤버가 **제 줄에** 적은 것만 본다 — 에픽 멤버의 에픽이 센 값을 다시 세면 같은 날이 두 번 든다(마일스톤의
/// 멤버에는 에픽도 이슈도 이미 들어 있다).
fn planned_of<'a>(i: &'a Issue, members: &[usize], all: &'a [Issue], wh: &'a Where<'a>) -> Option<Planned<'a>> {
    let (start, end) = (i.planned_start(), i.deadline());
    if start.is_some() || end.is_some() {
        return Some(Planned { start, end, from: Origin::Own });
    }
    // 날짜는 고정폭이라 글로 견준다(`Issue::validate_fields` 의 `DateOrder` 와 같은 자).
    let planned = members.iter().map(|&m| &all[m]).filter(|m| !wh.deferred(m));
    let (mut first, mut last): (Option<&str>, Option<&str>) = (None, None);
    for m in planned {
        if let Some(s) = m.planned_start() {
            first = Some(first.map_or(s, |f| f.min(s)));
        }
        if let Some(d) = m.deadline() {
            last = Some(last.map_or(d, |l| l.max(d)));
        }
    }
    (first.is_some() || last.is_some()).then_some(Planned { start: first, end: last, from: Origin::Members })
}

/// 실제 구간. 일은 제 `started_at` 으로, 묶음은 멤버(일)의 가장 이른 `started_at` 으로 잰다.
fn actual_of<'a>(i: &'a Issue, members: &[usize], all: &'a [Issue], wh: &'a Where<'a>, cfg: &Config) -> Actual<'a> {
    // 묶음이면 멤버의 일, 아니면 그 줄 하나 — 시작을 아는 것/모르는 것을 같은 걸음으로 센다.
    let own = [i];
    let work: Vec<&Issue> = match is_group(i) {
        true => members.iter().map(|&m| &all[m]).filter(|m| is_work(m)).collect(),
        false => own.to_vec(),
    };
    // **시작은 읽히는 것만 센다** — 못 읽는 글(손으로 푼 줄)은 모르는 것이다.
    let start = work.iter().filter_map(|w| w.started_at.as_deref().filter(|s| parse_rfc3339(s).is_some())).min();
    let Some(start) = start else {
        // 시작을 적은 일이 하나도 없다. 첫 칸을 떠난 일이 있으면 "시작했는데 모른다", 없으면 시작 전이다.
        let left = work.iter().any(|w| wh.column(w) != cfg.first_status());
        return match left || (!is_group(i) && wh.column(i) == DONE) {
            true => Actual::Unknown,
            false => Actual::NotStarted,
        };
    };
    // 닫혔는가는 **서 있는 칸**으로 묻는다 — 묶음은 멤버에서 읽은 칸이고, 되돌린 줄의 `done_at` 은 안 센다.
    let end = match wh.column(i) == DONE {
        true => match super::stats::closed_at(i, wh) {
            Some(e) => Some(e).filter(|e| parse_rfc3339(e).zip(parse_rfc3339(start)).is_some_and(|(e, s)| e >= s)),
            None => None,
        },
        false => None,
    };
    // 닫혔는데 끝이 시작보다 앞서거나 못 읽으면 구간을 못 낸다 — 열린 구간으로 그리면 끝난 일이 아직 도는 것처럼 선다.
    if wh.column(i) == DONE && end.is_none() {
        return Actual::Unknown;
    }
    Actual::Span { start, end }
}

/// 열린 줄의 **제** 기한이 지난 날수. 미룬 줄은 안 센다.
fn overdue_of(i: &Issue, column: &str, deferred: bool, now: &str) -> Option<i64> {
    if column == DONE || deferred {
        return None;
    }
    days_until(i.deadline()?, now).filter(|d| *d < 0).map(|d| -d)
}

/// 모든 줄의 계획·실제 가운데 가장 이른 날 ~ 가장 늦은 날. 열린 구간은 오늘까지다.
fn range_of(rows: &[Row<'_>], now: &str) -> Option<Range> {
    let today = now.get(..10).filter(|d| crate::model::parse_date(d).is_some());
    let mut days: Vec<&str> = Vec::new();
    for r in rows {
        if let Some(p) = &r.planned {
            days.extend(p.start);
            days.extend(p.end);
        }
        if let Actual::Span { start, end } = &r.actual {
            days.extend(start.get(..10));
            match end {
                Some(e) => days.extend(e.get(..10)),
                None => days.extend(today),
            }
        }
    }
    let from = days.iter().min()?;
    let to = days.iter().max()?;
    Some(Range { from: from.to_string(), to: to.to_string() })
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
        let cfg: &'static Config = Box::leak(Box::new(cfg()));
        let wh: &'a Where<'a> = Box::leak(Box::new(Where::of(all, cfg)));
        gantt(all, &Index::of(all), wh, cfg, NOW)
    }

    fn by<'g, 'a>(g: &'g Gantt<'a>, id: &str) -> &'g Row<'a> {
        g.rows.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{id} 줄이 없다"))
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
        assert_eq!(a.planned, Some(Planned { start: Some("2026-10-22"), end: Some("2026-10-30"), from: Origin::Own }));
        assert_eq!(a.actual, Actual::NotStarted);
        assert_eq!(a.progress, Standing::Column { name: "todo" });

        let b = by(&g, "argos-0002");
        assert_eq!(b.planned, None);
        // 닫히지 않았으니 열려 있다 — 끝이 없다.
        assert_eq!(b.actual, Actual::Span { start: "2026-10-10T01:00:00Z", end: None });

        let c = by(&g, "argos-0003");
        assert_eq!(c.planned, Some(Planned { start: None, end: Some("2026-10-18"), from: Origin::Own }));
        assert_eq!(c.actual, Actual::Span { start: "2026-10-12T01:00:00Z", end: Some("2026-10-16T05:00:00Z") });
        assert_eq!(c.overdue, None, "닫힌 줄은 기한을 넘기지 않는다");

        let d = by(&g, "argos-0004");
        assert_eq!(
            (d.planned.clone(), d.actual.clone(), d.overdue, d.deferred),
            (None, Actual::NotStarted, None, false)
        );
        assert!(d.blocked_by.is_empty());
        // 폭은 모든 줄의 가장 이른 날에서 가장 늦은 날까지 — 열린 줄(b)은 오늘까지다.
        assert_eq!(g.range, Some(Range { from: "2026-10-10".into(), to: "2026-10-30".into() }));
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
            Some(Planned { start: None, end: Some("2026-11-30"), from: Origin::Own })
        );
        assert_eq!(
            by(&g, "argos-0002").planned,
            Some(Planned { start: Some("2026-10-22"), end: Some("2026-11-10"), from: Origin::Members })
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
        assert_eq!(
            by(&g, "argos-m001").planned,
            Some(Planned { start: Some("2026-10-27"), end: Some("2026-12-01"), from: Origin::Members })
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
            Some(Planned { start: None, end: Some("2026-10-30"), from: Origin::Members }),
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
        assert_eq!(first["planned"], serde_json::json!({"start": null, "end": "2026-10-30", "from": "own"}));
        assert_eq!(first["progress"], serde_json::json!({"kind": "column", "name": "in_progress"}));
        assert_eq!(v["rows"][1]["progress"], serde_json::json!({"kind": "rollup", "done": 0, "total": 0}));
        assert_eq!(v["range"], serde_json::json!({"from": "2026-10-10", "to": "2026-10-30"}));
    }

    /// 행은 `all` 과 같은 차례·같은 길이고, 같은 id 를 든 줄도 첨자로 가려진다.
    #[test]
    fn rows_follow_the_input_order() {
        let all = vec![row("argos-0002", Kind::Issue, "todo"), row("argos-0001", Kind::Issue, "todo")];
        let g = read(&all);
        assert_eq!(g.rows.iter().map(|r| (r.at, r.id)).collect::<Vec<_>>(), vec![(0, "argos-0002"), (1, "argos-0001")]);
    }
}
