//! 세는 일과 고르는 일. **순수 함수다** — `&[Issue]` 만 보고 아무것도 찍지 않는다.
//!
//! **여기가 TUI 와의 계약이다.** 나중에 `ratatui` 는 `view` 를 건너뛰고
//! 이 모듈과 `query` 를 직접 부른다. 무엇을 셀지 정하는 코드가 `cmd/` 나
//! `view.rs` 에 있으면 표면이 늘 때마다 같은 것을 다시 짜야 한다.

use crate::config::Config;
use crate::model::{Issue, Kind, days_since};
use serde::Serialize;
use std::collections::BTreeMap;

/// 에픽 하나(또는 "에픽 없음")의 집계. **저장하지 않는다** — 멤버 하나를
/// 닫을 때 에픽 줄까지 써야 한다면 그 필드는 파생값이고, 두 줄 쓰기 중간에
/// 죽으면 에픽이 영원히 거짓말한다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Roll {
    /// 에픽의 id. "에픽 없음" 묶음이면 `None`.
    pub id: Option<String>,
    pub title: String,
    /// 칸 이름 → 건수.
    pub counts: BTreeMap<String, usize>,
    pub total: usize,
    pub done: usize,
    /// 0~100. 멤버가 없으면 `None` — 0% 라고 말하면 "아직 안 한 에픽" 과
    /// "속을 안 채운 에픽" 이 구별되지 않는다.
    ///
    /// **리포트 시점의 값이다.** 스냅샷에는 저장하지 않는다 — 저장하면
    /// 멤버 하나 닫을 때 에픽 줄까지 써야 하고, 두 줄 쓰기 중간에 죽으면
    /// 영원히 거짓말한다.
    pub percent: Option<u8>,
}

/// 이슈 id → 그것이 속한 에픽의 **제목**. 화면이 필요한 것은 id 가 아니라
/// 제목이고, 소속 판정(`groups`)과 제목 찾기를 한 번에 끝내 둔다.
pub fn epic_labels(all: &[Issue]) -> BTreeMap<&str, String> {
    let titles: BTreeMap<&str, &str> = all.iter().map(|i| (i.id.as_str(), i.title.as_str())).collect();
    groups(all)
        .into_iter()
        .map(|(id, epic)| {
            (id, titles.get(epic).copied().unwrap_or("(없는 에픽)").to_string())
        })
        .collect()
}

pub fn is_epic(i: &Issue) -> bool {
    i.kind == Kind::Epic
}

/// `id` 의 직계 자식. 부모는 id 에서 유도되므로 접두 검사면 된다.
pub fn children_of<'a>(issues: &'a [Issue], id: &str) -> Vec<&'a Issue> {
    issues.iter().filter(|c| crate::id::parent_of(&c.id) == Some(id)).collect()
}

/// 그 에픽에 속한 이슈. 소속은 필드고 계층은 id 라, 둘은 직교한다.
pub fn members_of<'a>(issues: &'a [Issue], epic: &str) -> Vec<&'a Issue> {
    issues.iter().filter(|i| i.epic.as_deref() == Some(epic)).collect()
}

/// 이슈 id → 그것이 속한 에픽 id. 없으면 안 들어간다.
///
/// **자식은 소속을 조상에게서 물려받는다.** 부모가 에픽 A 에 있는데 자식이
/// 제 `epic` 을 안 적었다고 "에픽 없음" 으로 가면, 같은 일이 두 묶음에
/// 나타나고 어느 쪽이 참인지 화면만 봐서는 알 수 없다. 자식이 제 `epic` 을
/// 적었으면 그것이 이긴다 — 계층과 소속은 직교하므로 옮길 수 있어야 한다.
pub fn groups(all: &[Issue]) -> BTreeMap<&str, &str> {
    let by_id: BTreeMap<&str, &Issue> = all.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut out = BTreeMap::new();
    for i in all {
        let mut cur = i;
        loop {
            if let Some(e) = &cur.epic {
                out.insert(i.id.as_str(), e.as_str());
                break;
            }
            match crate::id::parent_of(&cur.id).and_then(|p| by_id.get(p)) {
                Some(p) => cur = p,
                None => break,
            }
        }
    }
    out
}

/// 에픽별 집계와, 마지막에 "에픽 없음" 묶음 하나.
///
/// **멤버가 없는 에픽도 줄을 갖는다.** 빠뜨리면 "계획만 세우고 안 채운 것"
/// 이 화면에서 사라져, 정확히 드러내야 할 것이 안 보인다.
/// **소속 없는 이슈도 묶음을 갖는다.** 같은 이유다.
pub fn rollup(issues: &[Issue], cfg: &Config) -> Vec<Roll> {
    let tally = |members: &[&Issue]| {
        let counts: BTreeMap<String, usize> = cfg
            .statuses
            .iter()
            .map(|s| (s.clone(), members.iter().filter(|i| i.status.as_str() == s).count()))
            .collect();
        let total = members.len();
        let done = members.iter().filter(|i| i.status.is_done()).count();
        let percent = (total > 0).then(|| (done * 100 / total) as u8);
        (counts, total, done, percent)
    };

    // 자식이 물려받은 소속까지 센다. 트리가 그리는 것과 같은 판정이어야
    // 머리글의 건수와 그 밑의 줄 수가 어긋나지 않는다.
    let group = groups(issues);
    let mut out: Vec<Roll> = issues
        .iter()
        .filter(|i| is_epic(i))
        .map(|e| {
            let members: Vec<&Issue> = issues
                .iter()
                .filter(|i| group.get(i.id.as_str()) == Some(&e.id.as_str()))
                .collect();
            let (counts, total, done, percent) = tally(&members);
            Roll { id: Some(e.id.clone()), title: e.title.clone(), counts, total, done, percent }
        })
        .collect();

    // 에픽이 아니면서 어느 에픽에도 안 딸린 것. 에픽 자신은 세지 않는다.
    let loose: Vec<&Issue> = issues
        .iter()
        .filter(|i| !is_epic(i) && !group.contains_key(i.id.as_str()))
        .collect();
    let (counts, total, done, percent) = tally(&loose);
    out.push(Roll { id: None, title: "에픽 없음".into(), counts, total, done, percent });
    out
}

/// 지금 집을 수 있는 일.
///
/// 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
/// 에이전트가 `moai show -s todo --type issue --parent none …` 를 매번
/// 조립하게 두면 조립할 때마다 규칙이 조금씩 달라진다.
pub fn ready<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    let group = groups(issues);
    // 자식은 소속을 조상에게서 물려받으므로 끝난 에픽의 손자도 집지 않는다.
    let done_epic = |i: &Issue| {
        group
            .get(i.id.as_str())
            .and_then(|e| issues.iter().find(|x| x.id == *e))
            .is_some_and(|e| e.status.is_done())
    };
    // 자식이 남아 있으면 부모는 직접 하는 일이 아니다.
    let has_open_child =
        |i: &Issue| children_of(issues, &i.id).iter().any(|c| !c.status.is_done());

    let progress: BTreeMap<Option<String>, u8> =
        rollup(issues, cfg).into_iter().map(|r| (r.id, r.percent.unwrap_or(0))).collect();

    let mut out: Vec<&Issue> = issues
        .iter()
        .filter(|i| {
            !is_epic(i)                              // 에픽 자체는 집는 게 아니다
                && i.status.as_str() == cfg.first_status()
                && !done_epic(i)
                && !has_open_child(i)
        })
        .collect();

    // 급한 것 → 끝나가는 에픽 → 오래된 것. 끝나가는 것을 먼저 집어야
    // 벌여 놓은 에픽이 줄어든다.
    let pct = |i: &Issue| {
        progress.get(&group.get(i.id.as_str()).map(|e| e.to_string())).copied().unwrap_or(0)
    };
    out.sort_by(|a, b| {
        let (pa, pb) = (pct(a), pct(b));
        a.priority()
            .cmp(&b.priority())
            .then_with(|| pb.cmp(&pa))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}


// ── moai status ──────────────────────────────────────────────────────
//
// 게이트를 없앤 자리를 메우는 것이 이 리포트 하나다. **아무것도 막지 않는다**
// — 막기 시작하면 그게 게이트고, 이전 시도가 정확히 그것으로 죽었다.
//
// 임계값은 이름 붙인 상수로 둔다. 지금 설정 시스템을 만들면 아무도 안 고치는
// 파일이 하나 늘 뿐이다. 실제로 거슬릴 때 config 로 뺀다.

/// review 에 이만큼 머물면 썩는 것으로 본다.
const REVIEW_STALE_DAYS: i64 = 3;
/// 집어 놓고 이만큼 안 건드리면 잊은 것으로 본다.
const WIP_STALE_DAYS: i64 = 2;
/// 한 번에 이보다 많이 벌이면 알린다.
const WIP_LIMIT: usize = 3;
/// 에픽 없는 이슈가 이 비율을 넘으면 알린다.
const NO_EPIC_RATIO: f64 = 0.15;
/// 비율이 낮아도 이 수를 넘으면 알린다.
const NO_EPIC_MIN: usize = 5;
/// 흐름을 재는 창.
const FLOW_DAYS: i64 = 7;

/// 드러난 것 하나. `kind` 가 **타입 붙은 열거값**이라 받는 쪽이 산문을
/// 파싱하지 않고 분기한다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Warning {
    pub kind: &'static str,
    pub count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// 데이터가 깨진 것. **이것만 비영 종료한다.**
    pub fatal: bool,
}

impl Warning {
    fn new(kind: &'static str, ids: Vec<String>) -> Warning {
        Warning {
            kind,
            count: ids.len(),
            ids,
            days: None,
            limit: None,
            ratio: None,
            hint: None,
            fatal: false,
        }
    }
    fn hint(mut self, h: &str) -> Warning {
        self.hint = Some(h.to_string());
        self
    }
    fn days(mut self, d: i64) -> Warning {
        self.days = Some(d);
        self
    }
    fn limit(mut self, n: usize) -> Warning {
        self.limit = Some(n);
        self
    }
    fn ratio(mut self, r: f64) -> Warning {
        self.ratio = Some(r);
        self
    }
    fn fatal(mut self) -> Warning {
        self.fatal = true;
        self
    }
}

/// 만드는 속도와 끝내는 속도. **한 줄로 전체 건강을 말하는 숫자다.**
#[derive(Debug, PartialEq, Serialize)]
pub struct Flow {
    pub days: i64,
    pub created: usize,
    pub done: usize,
    /// 양수면 쌓이는 중.
    pub net: i64,
}

/// `moai status` 가 낼 것 전부. 칸 이름을 담는 `model::Status` 와 다른 것이라
/// 이름을 달리 둔다 — 한 파일에서 둘이 만나면 어느 쪽인지 매번 헷갈린다.
#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub counts: BTreeMap<String, usize>,
    pub total: usize,
    pub epics: Vec<Roll>,
    pub warnings: Vec<Warning>,
    pub flow: Flow,
}

impl StatusReport {
    /// 깨진 데이터가 있는가. 종료 코드를 가르는 유일한 것.
    pub fn broken(&self) -> bool {
        self.warnings.iter().any(|w| w.fatal)
    }
}

fn ids_of(v: &[&Issue]) -> Vec<String> {
    v.iter().map(|i| i.id.clone()).collect()
}

/// `unreadable` 은 읽다 만난 줄 번호다 — 저장소가 아니라 부르는 쪽이 준다.
pub fn status(issues: &[Issue], unreadable: &[usize], cfg: &Config, now: &str) -> StatusReport {
    let work: Vec<&Issue> = issues.iter().filter(|i| !is_epic(i)).collect();
    let counts: BTreeMap<String, usize> = cfg
        .statuses
        .iter()
        .map(|s| (s.clone(), work.iter().filter(|i| i.status.as_str() == s).count()))
        .collect();

    let rolls = rollup(issues, cfg);
    let epics: Vec<Roll> = rolls.iter().filter(|r| r.id.is_some()).cloned().collect();
    let group = groups(issues);
    let mut warnings = Vec::new();

    // 1. 에픽에 안 붙은 것. 마일스톤이 아직 없으므로 **제일 중요한 신호**다
    //    — "물어보지 않고 만든 이슈" 의 지문이다.
    let loose: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| !i.status.is_done() && !group.contains_key(i.id.as_str()))
        .collect();
    let open = work.iter().filter(|i| !i.status.is_done()).count();
    let ratio = if open == 0 { 0.0 } else { loose.len() as f64 / open as f64 };
    if loose.len() >= NO_EPIC_MIN || (ratio >= NO_EPIC_RATIO && !loose.is_empty()) {
        warnings.push(
            Warning::new("no_epic", ids_of(&loose)).ratio(ratio).hint("moai show -e none"),
        );
    }

    // 2. review 에서 썩는 것. 게이트를 없앤 대가라 여기가 제일 먼저 곪는다.
    let rotting: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            i.status.as_str() == "review"
                && days_since(&i.status_since, now).is_some_and(|d| d > REVIEW_STALE_DAYS)
        })
        .collect();
    if !rotting.is_empty() {
        warnings.push(
            Warning::new("stale_review", ids_of(&rotting))
                .days(REVIEW_STALE_DAYS)
                .hint("moai show -s review --stale 3"),
        );
    }

    // 3. 한 번에 여러 개 벌인 것. AI 가 가장 잘 하는 실수다.
    let wip: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| !i.status.is_done() && i.status.as_str() != cfg.first_status())
        .collect();
    if wip.len() > WIP_LIMIT {
        warnings.push(Warning::new("wip_overload", ids_of(&wip)).limit(WIP_LIMIT));
    }

    // 4. 집어 놓고 잊은 것.
    let forgotten: Vec<&Issue> = wip
        .iter()
        .copied()
        .filter(|i| {
            i.status.as_str() != "review"
                && days_since(&i.status_since, now).is_some_and(|d| d > WIP_STALE_DAYS)
        })
        .collect();
    if !forgotten.is_empty() {
        warnings.push(Warning::new("stale_progress", ids_of(&forgotten)).days(WIP_STALE_DAYS));
    }

    // 5. 계획만 세우고 안 채운 것 / 채우고 안 접은 것.
    let empty: Vec<String> = rolls
        .iter()
        .filter(|r| r.id.is_some() && r.total == 0)
        .filter_map(|r| r.id.clone())
        .collect();
    if !empty.is_empty() {
        warnings.push(Warning::new("empty_epic", empty));
    }
    let finished: Vec<String> = rolls
        .iter()
        .filter(|r| r.percent == Some(100))
        .filter_map(|r| r.id.as_deref())
        .filter(|id| issues.iter().any(|e| e.id == *id && !e.status.is_done()))
        .map(str::to_string)
        .collect();
    if !finished.is_empty() {
        warnings.push(Warning::new("finished_epic", finished));
    }

    // 6. 머지를 잘못 푼 흔적. 여기부터는 드러내는 것을 넘어 고쳐야 할 것이다.
    let known: std::collections::BTreeSet<&str> = issues.iter().map(|i| i.id.as_str()).collect();
    let dangling: Vec<&Issue> = issues
        .iter()
        .filter(|i| i.epic.as_deref().is_some_and(|e| !known.contains(e)))
        .collect();
    if !dangling.is_empty() {
        warnings.push(Warning::new("dangling_epic", ids_of(&dangling)));
    }
    let orphans: Vec<&Issue> = issues
        .iter()
        .filter(|i| crate::id::parent_of(&i.id).is_some_and(|p| !known.contains(p)))
        .collect();
    if !orphans.is_empty() {
        warnings.push(Warning::new("orphan_child", ids_of(&orphans)));
    }
    // 모르는 필드는 **버리지 않고 들고 있다.** 들고 있다는 사실만 비춘다 —
    // 2단계 바이너리가 쓴 파일을 1단계가 만졌다는 뜻일 수 있다.
    let carrying: Vec<&Issue> = issues.iter().filter(|i| !i.rest.is_empty()).collect();
    if !carrying.is_empty() {
        warnings.push(Warning::new("unknown_field", ids_of(&carrying)));
    }

    // 7. 데이터가 깨진 것. **이것만 비영 종료한다.**
    let mut seen = std::collections::BTreeSet::new();
    let dups: Vec<String> = issues
        .iter()
        .filter(|i| !seen.insert(i.id.as_str()))
        .map(|i| i.id.clone())
        .collect();
    if !dups.is_empty() {
        warnings.push(Warning::new("duplicate_id", dups).fatal());
    }
    if !unreadable.is_empty() {
        let mut w = Warning::new("unreadable_line", Vec::new()).fatal();
        w.count = unreadable.len();
        warnings.push(w);
    }

    // 흐름. 만드는 속도가 끝내는 속도를 넘으면 쌓인다.
    let within = |at: &str| days_since(at, now).is_some_and(|d| (0..FLOW_DAYS).contains(&d));
    let created = work.iter().filter(|i| within(&i.created_at)).count();
    let closed =
        work.iter().filter(|i| i.status.is_done() && within(&i.status_since)).count();

    StatusReport {
        counts,
        total: work.len(),
        epics,
        warnings,
        flow: Flow {
            days: FLOW_DAYS,
            created,
            done: closed,
            net: created as i64 - closed as i64,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn make(id: &str, kind: Kind, status: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new(status), "2026-09-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str, status: &str) -> Issue {
        let mut i = make(id, Kind::Issue, status);
        i.epic = Some(epic.into());
        i
    }

    fn roll_of<'a>(rolls: &'a [Roll], id: Option<&str>) -> &'a Roll {
        rolls.iter().find(|r| r.id.as_deref() == id).expect("그 묶음이 없다")
    }

    #[test]
    fn rolls_up_members_by_column() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "in_progress"),
            member("argos-0002", "argos-0001", "done"),
            member("argos-0003", "argos-0001", "todo"),
            member("argos-0004", "argos-0001", "done"),
        ];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!((r.total, r.done), (3, 2));
        assert_eq!(r.percent, Some(66));
        assert_eq!(r.counts, BTreeMap::from([
            ("todo".to_string(), 1), ("in_progress".to_string(), 0),
            ("review".to_string(), 0), ("done".to_string(), 2),
        ]));
    }

    /// 빈 에픽도 줄을 갖는다. 안 그러면 "계획만 세우고 안 채운 것" 이 사라진다.
    #[test]
    fn an_empty_epic_still_gets_a_row() {
        let issues = vec![make("argos-0001", Kind::Epic, "todo")];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!(r.total, 0);
        assert_eq!(r.percent, None, "멤버 없는 에픽을 0% 라고 했다");
    }

    /// 소속 없는 것도 묶음을 갖는다. 에픽 자신은 그 묶음에 세지 않는다.
    #[test]
    fn loose_issues_get_their_own_row() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            make("argos-0002", Kind::Issue, "todo"),
            member("argos-0003", "argos-0001", "todo"),
        ];
        let rolls = rollup(&issues, &cfg());
        let loose = roll_of(&rolls, None);
        assert_eq!(loose.total, 1, "에픽 자신이 에픽 없음에 섞였다");
        assert_eq!(rolls.len(), 2);
    }

    /// 모든 멤버가 끝난 에픽은 100% 다 — "닫을 때가 됐다" 를 말할 근거.
    #[test]
    fn a_finished_epic_reads_full() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "in_progress"),
            member("argos-0002", "argos-0001", "done"),
        ];
        let rolls = rollup(&issues, &cfg());
        assert_eq!(roll_of(&rolls, Some("argos-0001")).percent, Some(100));
    }

    #[test]
    fn ready_excludes_what_is_not_pickable() {
        let mut done_epic = make("argos-0001", Kind::Epic, "done");
        done_epic.status = Status::new("done");
        let issues = vec![
            make("argos-00aa", Kind::Epic, "todo"),          // 에픽 자체
            done_epic,                                        // 끝난 에픽
            member("argos-0002", "argos-0001", "todo"),       // 끝난 에픽의 멤버
            make("argos-0003", Kind::Issue, "in_progress"),   // 이미 집은 것
            make("argos-0004", Kind::Issue, "todo"),          // 자식이 남은 부모
            make("argos-0004.aaa", Kind::Issue, "todo"),      // 그 자식 — 이건 집는다
            make("argos-0005", Kind::Issue, "todo"),          // 평범한 것
        ];
        let got: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0004.aaa", "argos-0005"], "{got:?}");
    }

    /// 자식이 다 끝나면 부모를 집을 수 있다.
    #[test]
    fn a_parent_becomes_ready_once_children_close() {
        let issues = vec![
            make("argos-0004", Kind::Issue, "todo"),
            make("argos-0004.aaa", Kind::Issue, "done"),
        ];
        let got: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0004"]);
    }

    /// 급한 것 먼저, 그다음 끝나가는 에픽 먼저.
    #[test]
    fn ready_sorts_urgent_then_nearly_finished() {
        let mut issues = vec![
            make("argos-0001", Kind::Epic, "todo"),  // 0% 에픽
            make("argos-0002", Kind::Epic, "todo"),  // 50% 에픽
            member("argos-000a", "argos-0002", "done"),
            member("argos-000b", "argos-0002", "todo"),
            member("argos-000c", "argos-0001", "todo"),
        ];
        issues.push({
            let mut i = make("argos-000d", Kind::Issue, "todo");
            i.priority = Some(0);
            i
        });
        let got: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        // p0 → 50% 에픽 멤버 → 0% 에픽 멤버
        assert_eq!(got, ["argos-000d", "argos-000b", "argos-000c"], "{got:?}");
    }

    /// 자식은 소속을 조상에게서 물려받는다. 안 그러면 같은 일이
    /// 에픽 밑과 "에픽 없음" 양쪽에 나타난다.
    #[test]
    fn children_inherit_their_ancestors_epic() {
        let mut own = make("argos-0003.bbb", Kind::Issue, "todo");
        own.epic = Some("argos-0009".into()); // 제 것을 적었으면 그것이 이긴다
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
            make("argos-0002.aaa.ccc", Kind::Issue, "todo"),
            member("argos-0003", "argos-0001", "todo"),
            own,
            make("argos-0004", Kind::Issue, "todo"), // 진짜 소속 없음
        ];
        let g = groups(&issues);
        assert_eq!(g.get("argos-0002.aaa"), Some(&"argos-0001"));
        assert_eq!(g.get("argos-0002.aaa.ccc"), Some(&"argos-0001"), "손자가 안 물려받았다");
        assert_eq!(g.get("argos-0003.bbb"), Some(&"argos-0009"), "제가 적은 것이 져 버렸다");
        assert_eq!(g.get("argos-0004"), None);
    }

    /// 끝난 에픽의 **손자**도 집지 않는다.
    #[test]
    fn ready_skips_grandchildren_of_a_finished_epic() {
        let mut epic = make("argos-0001", Kind::Epic, "done");
        epic.status = Status::new("done");
        let issues = vec![
            epic,
            member("argos-0002", "argos-0001", "done"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
        ];
        assert!(ready(&issues, &cfg()).is_empty(), "끝난 에픽 밑의 손자를 집었다");
    }

    fn kinds(st: &StatusReport) -> Vec<&str> {
        st.warnings.iter().map(|w| w.kind).collect()
    }

    fn at(i: Issue, created: &str, since: &str) -> Issue {
        Issue { created_at: created.into(), status_since: since.into(), ..i }
    }

    /// 깨끗하면 아무 말도 하지 않는다.
    #[test]
    fn a_clean_repo_warns_about_nothing() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert!(st.warnings.is_empty(), "{:?}", kinds(&st));
        assert!(!st.broken());
    }

    /// 에픽에 안 붙은 것이 제일 중요한 신호다 — 마일스톤이 아직 없으므로.
    #[test]
    fn loose_issues_are_the_headline() {
        let issues: Vec<Issue> =
            (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "todo")).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert_eq!(kinds(&st), ["no_epic"]);
        assert_eq!(st.warnings[0].count, 5);
        assert_eq!(st.warnings[0].hint.as_deref(), Some("moai show -e none"));

        // 끝난 것은 세지 않는다 — 이미 지나간 일이다
        let done: Vec<Issue> =
            (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "done")).collect();
        assert!(status(&done, &[], &cfg(), "2026-09-01T00:00:00Z").warnings.is_empty());
    }

    /// 게이트를 없앤 대가는 review 가 썩는 것이다. 날짜로 잰다.
    #[test]
    fn review_rot_is_measured_in_days() {
        let i = at(make("argos-0001", Kind::Issue, "review"), "2026-09-01T00:00:00Z", "2026-09-01T00:00:00Z");
        let issues = vec![i];
        let early = status(&issues, &[], &cfg(), "2026-09-03T00:00:00Z");
        assert!(!kinds(&early).contains(&"stale_review"));
        let st = status(&issues, &[], &cfg(), "2026-09-06T00:00:00Z");
        assert!(kinds(&st).contains(&"stale_review"), "{:?}", kinds(&st));
        assert_eq!(st.warnings.iter().find(|w| w.kind == "stale_review").unwrap().days, Some(3));
    }

    #[test]
    fn too_much_at_once_is_named() {
        let issues: Vec<Issue> =
            (0..4).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "in_progress")).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        let w = st.warnings.iter().find(|w| w.kind == "wip_overload").unwrap();
        assert_eq!((w.count, w.limit), (4, Some(3)));
        // 셋까지는 말하지 않는다
        let three = status(&issues[..3], &[], &cfg(), "2026-09-01T00:00:00Z");
        assert!(!kinds(&three).contains(&"wip_overload"));
    }

    #[test]
    fn plans_unfilled_and_work_unclosed_both_show() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),                 // 빈 에픽
            make("argos-0002", Kind::Epic, "in_progress"),          // 다 끝났는데 안 닫힘
            member("argos-0003", "argos-0002", "done"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        let k = kinds(&st);
        assert!(k.contains(&"empty_epic") && k.contains(&"finished_epic"), "{k:?}");
    }

    /// 머지를 잘못 푼 흔적. 깨진 것만 종료 코드를 바꾼다.
    #[test]
    fn only_broken_data_is_fatal() {
        let mut dangling = make("argos-0001", Kind::Issue, "todo");
        dangling.epic = Some("argos-9999".into());
        let issues = vec![dangling, make("argos-0002.aaa", Kind::Issue, "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        let k = kinds(&st);
        assert!(k.contains(&"dangling_epic") && k.contains(&"orphan_child"), "{k:?}");
        assert!(!st.broken(), "드러낼 것을 깨진 것으로 셌다");

        let dup = vec![make("argos-0001", Kind::Issue, "todo"), make("argos-0001", Kind::Issue, "todo")];
        assert!(status(&dup, &[], &cfg(), "2026-09-01T00:00:00Z").broken());
        assert!(status(&[], &[41], &cfg(), "2026-09-01T00:00:00Z").broken());
    }

    /// 만드는 속도가 끝내는 속도를 넘으면 쌓인다.
    #[test]
    fn flow_compares_making_with_finishing() {
        let issues = vec![
            at(make("argos-0001", Kind::Issue, "todo"), "2026-09-08T00:00:00Z", "2026-09-08T00:00:00Z"),
            at(make("argos-0002", Kind::Issue, "todo"), "2026-09-08T00:00:00Z", "2026-09-08T00:00:00Z"),
            at(make("argos-0003", Kind::Issue, "done"), "2026-09-08T00:00:00Z", "2026-09-09T00:00:00Z"),
            // 창 밖 — 안 센다
            at(make("argos-0004", Kind::Issue, "todo"), "2026-08-01T00:00:00Z", "2026-08-01T00:00:00Z"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-10T00:00:00Z");
        assert_eq!(st.flow, Flow { days: 7, created: 3, done: 1, net: 2 });
    }

    /// 에픽은 칸 집계에 세지 않는다 — 에픽은 하는 일이 아니다.
    #[test]
    fn the_board_counts_work_not_epics() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert_eq!(st.total, 1);
        assert_eq!(st.counts.get("todo"), Some(&1));
    }

    #[test]
    fn finds_children_and_members() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
            make("argos-0002.aaa.bbb", Kind::Issue, "todo"),
        ];
        let kids: Vec<&str> = children_of(&issues, "argos-0002").iter().map(|i| i.id.as_str()).collect();
        assert_eq!(kids, ["argos-0002.aaa"], "손자까지 직계로 셌다");
        let mem: Vec<&str> = members_of(&issues, "argos-0001").iter().map(|i| i.id.as_str()).collect();
        assert_eq!(mem, ["argos-0002"]);
    }
}
