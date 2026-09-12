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

/// 지금 계획에 있는 일인가. **`is_work` 에서 미뤄 둔 것을 뺀다.**
///
/// 세는 자리는 대부분 이쪽을 쓴다 — 보드·`ready`·경고·흐름은 "지금 할 수
/// 있는 것" 을 묻는다. **롤업만 `is_work` 를 그대로 쓴다**: 다섯 중 둘을
/// 미뤘다고 `3건짜리 에픽` 이 되면 미루는 것이 계획을 고쳐 쓰는 일이 되고,
/// 멤버를 전부 미룬 에픽이 `속이 빈 에픽` 으로 고발당한다.
pub fn is_active(i: &Issue) -> bool {
    is_work(i) && !i.is_deferred()
}

/// 담아 둔 생각인가. **술어를 `cmd/` 에 두지 않는다** — 어떤 줄이 무엇인지
/// 정하는 코드가 거기 있으면 다음 표면이 같은 판단을 다시 짠다.
pub fn is_idea(i: &Issue) -> bool {
    i.kind == Kind::Idea
}

/// 밑에 무엇을 담는 것인가. **일이 아닌 것이 곧 묶음인 것은 아니다** —
/// idea 도 일이 아니지만 아무것도 담지 않는다. 둘을 한 술어로 묻던 자리가
/// 상세에 `멤버 0/0` 을 내, 채울 것이 없는 자리에 채울 것이 있다고 말했다.
pub fn is_group(i: &Issue) -> bool {
    matches!(i.kind, Kind::Epic | Kind::Milestone)
}

/// **일은 이슈가 한다.** 에픽도 마일스톤도 묶음이지 일이 아니다 — 세면
/// 보드의 `todo 6` 이 실제로 할 일 넷과 묶음 둘을 합친 수가 된다.
pub fn is_work(i: &Issue) -> bool {
    i.kind == Kind::Issue
}

/// `id` 의 직계 자식. 부모는 id 에서 유도되므로 접두 검사면 된다.
///
/// **차례는 목록과 같다.** 상세 한 화면에서 자식 줄은 id 순, 그 아래 멤버 줄은
/// 우선순위 순이면 규칙이 둘이 되어 보는 쪽이 어느 쪽도 못 믿는다.
pub fn children_of<'a>(issues: &'a [Issue], id: &str) -> Vec<&'a Issue> {
    let mut out: Vec<&Issue> =
        issues.iter().filter(|c| crate::id::parent_of(&c.id) == Some(id)).collect();
    out.sort_by(|a, b| crate::query::display_order(a, b));
    out
}

/// 그 에픽에 속한 이슈. 소속은 필드고 계층은 id 라, 둘은 직교한다.
pub fn members_of<'a>(issues: &'a [Issue], epic: &str) -> Vec<&'a Issue> {
    issues.iter().filter(|i| i.epic.as_deref() == Some(epic)).collect()
}

/// 아직 안 끝난 막음이 하나라도 있는가. 없는 이슈를 가리키는 것은 막지
/// 않는다 — 끊긴 참조는 `moai status` 가 드러내지 `ready` 가 영원히 막지 않는다.
pub fn is_blocked(i: &Issue, by_id: &BTreeMap<&str, &Issue>) -> bool {
    i.blocked_by.iter().any(|b| by_id.get(b.as_str()).is_some_and(|x| !x.status.is_done()))
}

/// `blocker` 가 `blocked` 를 막으면 고리가 생기는가. **쓰기 전에** 막는다 —
/// 사후 검사로 두면 이미 고리가 든 파일을 누가 만들고, 그때는 어느 줄을
/// 끊을지 사람이 정해야 한다.
pub fn creates_cycle(issues: &[Issue], blocker: &str, blocked: &str) -> bool {
    // `blocked_by` 는 막히는 쪽에 적힌다. 앞으로(막는 쪽 → 막히는 쪽) 되짚으려면
    // 방향을 뒤집은 지도가 있어야 한다.
    let mut forward: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for i in issues {
        for b in &i.blocked_by {
            forward.entry(b.as_str()).or_default().push(i.id.as_str());
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![blocked];
    while let Some(cur) = stack.pop() {
        if cur == blocker {
            return true;
        }
        if !seen.insert(cur) {
            continue;
        }
        stack.extend(forward.get(cur).into_iter().flatten());
    }
    false
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

/// 이슈 id → 그것이 속한 마일스톤 id.
///
/// **에픽이 마일스톤을 이긴다.** 제 에픽이 있으면 그 에픽의 마일스톤이고,
/// 이슈가 `milestone` 을 따로 적었어도 그것은 지지 않는다. 없으면 조상을
/// 타고 올라가며 처음 만나는 것이다. 이슈마다 마일스톤을 적게 하면 에픽을
/// 옮길 때 멤버를 전부 따라 고쳐야 하고, 반드시 하나는 빠뜨린다.
///
/// **자리를 정하는 자와 세는 자가 하나다.** 한때 `nav` 는 에픽을 이기게 하고
/// 여기서는 제 마일스톤을 이기게 해, `moai show <마일스톤>` 의 머리글이
/// `멤버 0/1` 이라 말하면서 목록에는 아무것도 못 내는 일이 있었다(moai-lhbh).
/// 자가 둘이면 둘은 언젠가 어긋난다 — 어느 쪽이 옳은지가 아니라 하나여야
/// 한다는 것이 요점이다.
pub fn milestones(all: &[Issue]) -> BTreeMap<&str, &str> {
    let by_id: BTreeMap<&str, &Issue> = all.iter().map(|i| (i.id.as_str(), i)).collect();
    let epic_of = groups(all);
    let mut out = BTreeMap::new();
    for i in all {
        // 에픽이 있으면 **거기서부터** 센다. 제 줄에서 시작하면 제 마일스톤이
        // 이기고, 그러면 트리는 에픽 밑에 두는데 셈만 딴 곳으로 간다.
        let mut cur = match epic_of.get(i.id.as_str()) {
            // **에픽으로 쓸 수 없는 것을 가리키면 제 마일스톤으로 되돌아가지
            // 않는다.** 그 줄은 `nav` 에서 `(길 잃음)` 으로 가므로, 되돌아가면
            // 머리글은 세는데 목록에는 없는 줄이 그대로 남는다 — moai-lhbh 와
            // 같은 어긋남이고, 고치려던 것이 참조 하나 어긋난 날 되살아난다.
            //
            // **못 쓸 것의 뜻은 `misplaced` 와 같다** — 없는 id 와 종류가 틀린
            // 것을 한 자로 잰다. 그쪽을 부르지 않는 것은 `misplaced` 가 이
            // 함수를 부르기 때문이고, 그래서 판정만 같은 모양으로 둔다.
            Some(e) => match by_id.get(e).filter(|e| e.kind == Kind::Epic) {
                Some(e) => e,
                None => continue,
            },
            None => i,
        };
        // 에픽·부모를 타고 올라가며 처음 만나는 마일스톤. 고리가 있어도
        // 멈추도록 걸음 수를 제한한다.
        for _ in 0..64 {
            if let Some(m) = &cur.milestone {
                out.insert(i.id.as_str(), m.as_str());
                break;
            }
            let up = cur
                .epic
                .as_deref()
                .and_then(|e| by_id.get(e))
                .or_else(|| crate::id::parent_of(&cur.id).and_then(|p| by_id.get(p)));
            match up {
                Some(next) if next.id != cur.id => cur = next,
                _ => break,
            }
        }
    }
    out
}

/// 소속 참조가 못 쓸 것인 까닭.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Misplace {
    /// 에픽 자리에 없는 것이나 에픽 아닌 것이 있다.
    Epic,
    /// 마일스톤 자리에 없는 것이나 마일스톤 아닌 것이 있다.
    Milestone,
}

/// 소속으로 쓸 수 없는 참조를 가진 줄.
///
/// **끊긴 것과 종류가 틀린 것을 한 자로 잰다.** 둘을 다른 자로 재면 탐색기가
/// `(길 잃음)` 에 넣은 줄에 대해 `moai status` 가 아무 말도 안 하고, 배너가
/// 가리킨 명령이 침묵한다 — 실제로 그랬다.
///
/// **자리를 정하는 쪽(`nav`)이 이것을 그대로 쓴다.** 술어를 양쪽에 따로 두면
/// 그 순간 자가 둘이 되고, 어느 쪽이 참인지 화면만 봐서는 알 수 없다.
/// 그래서 판정 차례도 `nav::Ctx::home` 과 같다 — 마일스톤은 뿌리라 볼 것이
/// 없고, 에픽은 제 마일스톤만, 이슈는 에픽을 먼저 보고 없으면 마일스톤을 본다.
pub fn misplaced(all: &[Issue]) -> BTreeMap<&str, Misplace> {
    let kind_of: BTreeMap<&str, Kind> = all.iter().map(|i| (i.id.as_str(), i.kind)).collect();
    let epic_of = groups(all);
    let mile_of = milestones(all);
    let usable = |id: Option<&&str>, kind: Kind| id.is_none_or(|id| kind_of.get(*id) == Some(&kind));

    let mut out = BTreeMap::new();
    for i in all {
        let id = i.id.as_str();
        let mile = || mile_of.get(id);
        match i.kind {
            // 뿌리에 선다. 가리키는 것이 없다.
            Kind::Milestone => {}
            Kind::Epic => {
                if !usable(mile(), Kind::Milestone) {
                    out.insert(id, Misplace::Milestone);
                }
            }
            // idea 도 같은 자를 받는다. 에픽을 안 적은 idea 는 아무것도 안
            // 가리키므로 여기 걸릴 것이 없고, 적었는데 그것이 에픽이 아니면
            // 일과 똑같이 드러나야 한다.
            Kind::Issue | Kind::Idea => match epic_of.get(id) {
                Some(e) if kind_of.get(*e) != Some(&Kind::Epic) => {
                    out.insert(id, Misplace::Epic);
                }
                // 에픽이 멀쩡하면 그 에픽의 마일스톤을 따르므로 여기서 안 본다.
                Some(_) => {}
                None if !usable(mile(), Kind::Milestone) => {
                    out.insert(id, Misplace::Milestone);
                }
                None => {}
            },
        }
    }
    out
}

/// 못 쓸 참조를 **든** 줄. 자리를 바꾸든 안 바꾸든 고쳐야 할 것은 같다.
///
/// [`misplaced`] 와 물음이 다르다. 저쪽은 "이 줄을 어디에 둘까" 이고 이쪽은
/// "이 줄이 못 쓸 것을 가리키나" 다. 그래서 **물려받은 것이 아니라 제가 적은
/// 것**을 본다 — 부모의 망가진 참조를 물려받은 자식은 고칠 데가 없다 — 대신
/// 종류를 가리지 않는다: 에픽 줄이 든 엉뚱한 `epic` 도 누군가 고쳐야 한다.
pub fn broken(all: &[Issue]) -> BTreeMap<&str, Misplace> {
    let kind_of: BTreeMap<&str, Kind> = all.iter().map(|i| (i.id.as_str(), i.kind)).collect();
    let usable = |id: &Option<String>, kind: Kind| {
        id.as_deref().is_none_or(|id| kind_of.get(id) == Some(&kind))
    };
    let mut out = BTreeMap::new();
    for i in all {
        if !usable(&i.epic, Kind::Epic) {
            out.insert(i.id.as_str(), Misplace::Epic);
        } else if !usable(&i.milestone, Kind::Milestone) {
            out.insert(i.id.as_str(), Misplace::Milestone);
        }
    }
    out
}

/// 그 종류의 소속 지도. 에픽과 마일스톤이 같은 코드를 지난다.
fn group_for(kind: Kind, all: &[Issue]) -> BTreeMap<&str, &str> {
    match kind {
        Kind::Milestone => milestones(all),
        _ => groups(all),
    }
}

/// 에픽별 집계와, 마지막에 "에픽 없음" 묶음 하나.
///
/// **멤버가 없는 에픽도 줄을 갖는다.** 빠뜨리면 "계획만 세우고 안 채운 것"
/// 이 화면에서 사라져, 정확히 드러내야 할 것이 안 보인다.
/// **소속 없는 이슈도 묶음을 갖는다.** 같은 이유다.
pub fn rollup(issues: &[Issue], cfg: &Config) -> Vec<Roll> {
    rollup_of(Kind::Epic, issues, cfg)
}

/// `kind` 가 에픽이든 마일스톤이든 같은 셈을 한다. **일은 이슈가 한다** —
/// 에픽은 어느 쪽 집계에도 세지 않는다.
pub fn rollup_of(kind: Kind, issues: &[Issue], cfg: &Config) -> Vec<Roll> {
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
    let group = group_for(kind, issues);
    // **묶음도 급한 것이 위로 온다.** 파일 순(=id 순)으로 두면 이슈 목록과
    // 차례가 달라, 같은 화면에서 규칙이 둘이 된다.
    let mut groupings: Vec<&Issue> = issues.iter().filter(|i| i.kind == kind).collect();
    groupings.sort_by(|a, b| crate::query::display_order(a, b));
    let mut out: Vec<Roll> = groupings
        .iter()
        .map(|e| {
            let members: Vec<&Issue> = issues
                .iter()
                .filter(|i| is_work(i) && group.get(i.id.as_str()) == Some(&e.id.as_str()))
                .collect();
            let (counts, total, done, percent) = tally(&members);
            Roll { id: Some(e.id.clone()), title: e.title.clone(), counts, total, done, percent }
        })
        .collect();

    // 어느 묶음에도 안 딸린 일. 에픽은 일이 아니라 묶음이라 세지 않는다.
    let loose: Vec<&Issue> = issues
        .iter()
        .filter(|i| is_work(i) && !group.contains_key(i.id.as_str()))
        .collect();
    let (counts, total, done, percent) = tally(&loose);
    let none = match kind {
        Kind::Milestone => "마일스톤 없음",
        _ => "에픽 없음",
    };
    out.push(Roll { id: None, title: none.into(), counts, total, done, percent });
    out
}

/// 지금 집을 수 있는 일.
///
/// 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
/// 에이전트가 `moai show -s todo --type issue --parent none …` 를 매번
/// 조립하게 두면 조립할 때마다 규칙이 조금씩 달라진다.
pub fn ready<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    let group = groups(issues);
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // 자식은 소속을 조상에게서 물려받으므로 끝난 에픽의 손자도 집지 않는다.
    let done_epic = |i: &Issue| {
        group
            .get(i.id.as_str())
            .and_then(|e| issues.iter().find(|x| x.id == *e))
            .is_some_and(|e| e.status.is_done())
    };
    // 자식이 남아 있으면 부모는 직접 하는 일이 아니다. **일만 센다** —
    // 이슈 밑에 담아 둔 생각 하나가 그 이슈를 `ready` 에서 지워 버리는데,
    // idea 는 어느 목록에도 안 나오므로 왜 사라졌는지 볼 방법이 없다.
    let has_open_child = |i: &Issue| {
        children_of(issues, &i.id).iter().any(|c| is_active(c) && !c.status.is_done())
    };

    let progress: BTreeMap<Option<String>, u8> =
        rollup(issues, cfg).into_iter().map(|r| (r.id, r.percent.unwrap_or(0))).collect();

    let mut out: Vec<&Issue> = issues
        .iter()
        .filter(|i| {
            is_active(i)                             // 묶음도 미뤄 둔 것도 집는 게 아니다
                && i.status.as_str() == cfg.first_status()
                && !done_epic(i)
                && !has_open_child(i)
                && !is_blocked(i, &by_id)
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
/// 막힌 채로 지금 칸에 이만큼 머물면 "계획이 멈춘 자리" 로 본다. **막힌
/// 기간이 아니라 지금 칸에 머문 기간이다** — 막 막힌 낡은 이슈를 "며칠째
/// 막혀 있다" 고 잘못 말하지 않으려면 `blocked_by` 를 적은 시각을 따로
/// 저장해야 하는데, 그건 이 이슈의 범위 밖이다.
const BLOCKED_STALE_DAYS: i64 = 3;
/// 한 번에 이보다 많이 벌이면 알린다.
const WIP_LIMIT: usize = 3;
/// 에픽 없는 이슈가 이 비율을 넘으면 알린다.
const NO_EPIC_RATIO: f64 = 0.15;
/// 비율이 낮아도 이 수를 넘으면 알린다.
const NO_EPIC_MIN: usize = 5;
/// 흐름을 재는 창.
const FLOW_DAYS: i64 = 7;
/// 담아 둔 생각이 이만큼 쌓이면 알린다.
///
/// **담는 비용을 0 으로 만들면 쌓인다.** 쌓이는 것 자체는 문제가 아니고,
/// 쌓인 줄 모르는 것이 문제다. 그래서 드러내기만 하고 아무것도 막지 않는다.
/// 임계값은 `NO_EPIC_MIN` 과 같은 자리에 이름 붙인 상수로 둔다 — `moai-pz7h`
/// 가 이것들을 config 로 뺄 때 같이 간다.
const IDEA_PILE: usize = 5;

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
    /// 가장 오래된 것의 나이. **`days` 와 다른 것이다** — 저쪽은 넘긴
    /// 임계값이고 이쪽은 실제 나이다. 한 필드에 두 뜻을 담으면 받는 쪽이
    /// `kind` 를 같이 보지 않고서는 숫자를 읽을 수 없다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest: Option<i64>,
    /// 고칠 것이 아니라 알려 주는 것. **`fatal` 옆에 데이터로 둔다** —
    /// 이 판단이 `view` 에만 있으면 `view` 를 건너뛴 표면(TUI·`--json`)이
    /// 알림을 경고로 세고, 담을수록 화면이 시끄러워진다.
    pub notice: bool,
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
            oldest: None,
            notice: false,
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
    /// 세는 것이 `ids` 와 다를 때. 드러낼 id 가 없는 줄이 있다.
    fn count(mut self, n: usize) -> Warning {
        self.count = n;
        self
    }
    fn oldest(mut self, d: i64) -> Warning {
        self.oldest = Some(d);
        self
    }
    fn notice(mut self) -> Warning {
        self.notice = true;
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub milestones: Vec<Roll>,
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

/// 경고에 딸린 id 들. **급한 것을 앞에 둔다** — 보는 쪽이 앞의 몇 개만 낸다
/// (`view` 는 3개에서 자른다). 파일 순으로 두면 `p0` 하나가 `p3` 셋 뒤에
/// 가려서 아예 보이지 않는다. 여기서는 차례가 무엇이 보이는지를 정한다.
fn ids_of(v: &[&Issue]) -> Vec<String> {
    let mut v: Vec<&&Issue> = v.iter().collect();
    v.sort_by(|a, b| crate::query::display_order(a, b));
    v.into_iter().map(|i| i.id.clone()).collect()
}

/// `unreadable` 은 읽다 만난 줄 번호다 — 저장소가 아니라 부르는 쪽이 준다.
pub fn status(issues: &[Issue], unreadable: &[usize], cfg: &Config, now: &str) -> StatusReport {
    // **여기가 "지금 계획" 의 정의다.** 보드 수·모든 경고·흐름이 이 하나를
    // 지나므로, 미뤄 둔 것을 여기서 빼면 아래 전부에서 저절로 빠진다.
    let work: Vec<&Issue> = issues.iter().filter(|i| is_active(i)).collect();
    // **한 번만 잰다.** 둘 다 이슈 전부의 물림을 타고 올라가므로, 경고마다
    // 다시 부르면 같은 걸음을 `moai status` 한 번에 여러 벌 걷는다.
    // `placed` 는 자리를 못 정하는 줄(`nav` 의 `(길 잃음)` 과 같은 집합),
    // `held` 는 못 쓸 참조를 든 줄이다 — 아래 6번이 둘을 합쳐 드러낸다.
    let placed = misplaced(issues);
    let held = broken(issues);
    let counts: BTreeMap<String, usize> = cfg
        .statuses
        .iter()
        .map(|s| (s.clone(), work.iter().filter(|i| i.status.as_str() == s).count()))
        .collect();

    let rolls = rollup(issues, cfg);
    let epics: Vec<Roll> = rolls.iter().filter(|r| r.id.is_some()).cloned().collect();
    // 마일스톤을 하나도 안 쓰는 저장소에는 줄도 경고도 내지 않는다.
    let stones: Vec<Roll> = rollup_of(Kind::Milestone, issues, cfg)
        .into_iter()
        .filter(|r| r.id.is_some())
        .collect();
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

    // 1-2. 마일스톤을 쓰기 시작했는데 거기 안 붙은 일. 마일스톤이 없는
    //      저장소에는 말하지 않는다 — 안 쓰는 기능으로 잔소리하지 않는다.
    if !stones.is_empty() {
        let mile = milestones(issues);
        // 종류가 틀린 참조는 **마일스톤이 있는 것이 아니다.** 그대로 세면
        // 그 줄이 "마일스톤 있음" 으로 빠져, 정작 드러내야 할 것이 숨는다.
        let outside: Vec<&Issue> = work
            .iter()
            .copied()
            .filter(|i| {
                !i.status.is_done()
                    && (!mile.contains_key(i.id.as_str())
                        || placed.get(i.id.as_str()) == Some(&Misplace::Milestone))
            })
            .collect();
        if !outside.is_empty() {
            warnings.push(
                Warning::new("no_milestone", ids_of(&outside))
                    .hint("moai show --milestone none"),
            );
        }
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

    // 2-2. 오래 막혀 있는 것. 막힌 채로 방치되는 것이 계획이 멈춘 자리다.
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    let stuck: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            !i.status.is_done()
                && is_blocked(i, &by_id)
                && days_since(&i.status_since, now).is_some_and(|d| d > BLOCKED_STALE_DAYS)
        })
        .collect();
    if !stuck.is_empty() {
        warnings.push(Warning::new("blocked_stale", ids_of(&stuck)).days(BLOCKED_STALE_DAYS));
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
    // **드러내는 쪽이 자리를 정하는 쪽보다 넓다.** `misplaced` 는 *자리를 못
    // 정하는* 줄만 고르므로 `nav` 의 `(길 잃음)` 과 같은 집합이고, 그것은
    // 그대로 지킨다 — 배너가 가리킨 명령이 침묵하면 안 된다. 다만 자리를
    // 바꾸지 않는 못 쓸 참조가 있다: 에픽 줄이 든 엉뚱한 `epic`, 제 에픽이
    // 멀쩡한 줄이 든 엉뚱한 `milestone`. 그것도 고쳐야 할 것은 같으므로
    // 여기서 합친다. 빼면 `moai rm` 이 "끊긴 참조가 남았다" 고 말한 그 줄에
    // 대해 `status` 가 그다음부터 영영 침묵한다.
    for (kind, why) in [("dangling_epic", Misplace::Epic), ("dangling_milestone", Misplace::Milestone)] {
        let hit: Vec<&Issue> = issues
            .iter()
            .filter(|i| {
                let id = i.id.as_str();
                placed.get(id) == Some(&why) || held.get(id) == Some(&why)
            })
            .collect();
        if !hit.is_empty() {
            warnings.push(Warning::new(kind, ids_of(&hit)));
        }
    }

    let orphans: Vec<&Issue> = issues
        .iter()
        .filter(|i| crate::id::parent_of(&i.id).is_some_and(|p| !known.contains(p)))
        .collect();
    if !orphans.is_empty() {
        warnings.push(Warning::new("orphan_child", ids_of(&orphans)));
    }
    let dangling_blockers: Vec<&Issue> = issues
        .iter()
        .filter(|i| i.blocked_by.iter().any(|b| !known.contains(b.as_str())))
        .collect();
    if !dangling_blockers.is_empty() {
        warnings.push(Warning::new("dangling_blocked_by", ids_of(&dangling_blockers)));
    }
    // 모르는 필드는 **버리지 않고 들고 있다.** 들고 있다는 사실만 비춘다 —
    // 2단계 바이너리가 쓴 파일을 1단계가 만졌다는 뜻일 수 있다.
    let carrying: Vec<&Issue> = issues.iter().filter(|i| !i.rest.is_empty()).collect();
    if !carrying.is_empty() {
        warnings.push(Warning::new("unknown_field", ids_of(&carrying)));
    }

    // 6-2. 쌓인 생각. **경고가 아니라 알림이다** — 고칠 것이 있다는 말이
    //      아니라, 담아 둔 것을 한 번 펼쳐 볼 때가 됐다는 말이다.
    //
    //      **id 를 싣지 않는다.** 다섯 건이 넘어야 뜨는 줄인데 거기에 제목
    //      셋을 더 달면, 정확히 "담을수록 화면이 시끄러워진다" 는 그 일이
    //      일어난다. 무엇이 쌓였는지는 `moai idea ls` 가 낸다.
    //      **미뤄 둔 생각은 안 센다.** 여기 세면 이 줄이 가리키는 `moai idea
    //      ls` 가 그것을 숨겨, 세어 놓고 못 보여 주는 수가 된다 — 미룬 것은
    //      아래 6-3 이 제 이름으로 말한다.
    let piled = |i: &&Issue| is_idea(i) && !i.status.is_done() && !i.is_deferred();
    let count = issues.iter().filter(piled).count();
    if count >= IDEA_PILE {
        let oldest = issues
            .iter()
            .filter(piled)
            .filter_map(|i| days_since(&i.created_at, now))
            .max()
            .unwrap_or(0);
        warnings.push(
            Warning::new("idea_pile", Vec::new())
                .count(count)
                .oldest(oldest)
                .notice()
                .hint("moai idea ls"),
        );
    }

    // 6-3. 미뤄 둔 것. **한 건부터 말한다** — idea 와 달리 미루는 것은 이미
    //      있는 일에 대한 한 번의 결정이라 자주 쌓이지 않고, 대신 보드에서
    //      통째로 사라지므로 여기가 그것이 보이는 **유일한 자리**다. 흐린 한
    //      줄이고 알림이라 꾸지람으로 읽히지 않는다.
    //
    //      **종류를 안 가린다.** 미루는 길(`moai defer`)은 무엇이든 받는데
    //      `is_work` 로 좁히면 미뤄 둔 에픽·생각이 목록에서만 사라지고 여기서
    //      한마디도 안 나온다 — 보이는 유일한 자리가 그 줄만 안 비추는 꼴이다.
    let put_off = |i: &&Issue| i.is_deferred() && !i.status.is_done();
    let count = issues.iter().filter(put_off).count();
    if count > 0 {
        let oldest = issues
            .iter()
            .filter(put_off)
            .filter_map(|i| i.deferred_at.as_deref().and_then(|at| days_since(at, now)))
            .max()
            .unwrap_or(0);
        warnings.push(
            Warning::new("deferred", Vec::new())
                .count(count)
                .oldest(oldest)
                .notice()
                .hint("moai show --deferred"),
        );
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
        // **어느 줄인지는 여기가 아니라 저기서 난다.** 배너는 수만 말할 수
        // 있으므로(줄 번호는 id 가 아니라 `ids` 에 실을 것이 아니다) 줄 번호와
        // 까닭을 내는 명령을 댄다 — 안 대면 고칠 길이 도구 밖에만 남는다.
        warnings.push(
            Warning::new("unreadable_line", Vec::new())
                .count(unreadable.len())
                .hint("moai show")
                .fatal(),
        );
    }

    // 흐름. 만드는 속도가 끝내는 속도를 넘으면 쌓인다.
    let within = |at: &str| days_since(at, now).is_some_and(|d| (0..FLOW_DAYS).contains(&d));
    let created = work.iter().filter(|i| within(&i.created_at)).count();
    let closed =
        work.iter().filter(|i| i.status.is_done() && within(&i.status_since)).count();

    StatusReport {
        counts,
        total: work.len(),
        milestones: stones,
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

    /// **종류가 틀린 참조도 드러낸다.** 에픽이 아닌 것을 에픽이라 가리키면
    /// 탐색기는 그 줄을 `(길 잃음)` 에 넣는데, `status` 가 아무 말도 안 하면
    /// 배너가 가리킨 명령이 침묵한다.
    #[test]
    fn a_reference_to_the_wrong_kind_is_reported() {
        let mut points_at_issue = make("argos-0004", Kind::Issue, "todo");
        points_at_issue.epic = Some("argos-0009".into()); // 에픽이 아니라 이슈다
        let mut points_at_epic = make("argos-0005", Kind::Issue, "todo");
        points_at_epic.milestone = Some("argos-0002".into()); // 마일스톤이 아니라 에픽이다
        let issues = vec![
            make("argos-0002", Kind::Epic, "todo"),
            points_at_issue,
            points_at_epic,
            make("argos-0009", Kind::Issue, "todo"),
        ];

        let bad = misplaced(&issues);
        assert_eq!(bad.get("argos-0004"), Some(&Misplace::Epic), "{bad:?}");
        assert_eq!(bad.get("argos-0005"), Some(&Misplace::Milestone), "{bad:?}");
        assert!(!bad.contains_key("argos-0002"), "멀쩡한 줄을 걸었다 — {bad:?}");

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&"dangling_epic"), "{kinds:?}");
        assert!(kinds.contains(&"dangling_milestone"), "{kinds:?}");
    }

    /// **묶음 행의 망가진 참조도 드러낸다.** 에픽이 제 `epic` 에 없는 id 를
    /// 들고 있으면 자리는 멀쩡해도 그 줄은 고쳐야 한다 — 자리를 정하는
    /// 술어(`misplaced`)와 드러내는 술어는 물음이 다르다.
    #[test]
    fn a_grouping_row_with_a_broken_reference_is_reported() {
        let mut epic = make("argos-e001", Kind::Epic, "todo");
        epic.epic = Some("argos-nope".into());
        let mut issue_with_bad_milestone = make("argos-0010", Kind::Issue, "todo");
        issue_with_bad_milestone.epic = Some("argos-e001".into());
        issue_with_bad_milestone.milestone = Some("argos-gone".into());
        let issues = vec![epic, issue_with_bad_milestone];

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        let ids: Vec<&String> = st
            .warnings
            .iter()
            .filter(|w| w.kind.starts_with("dangling"))
            .flat_map(|w| w.ids.iter())
            .collect();
        assert!(ids.iter().any(|id| *id == "argos-e001"), "에픽의 망가진 epic 을 안 말한다 — {ids:?}");
        assert!(
            ids.iter().any(|id| *id == "argos-0010"),
            "이슈의 망가진 milestone 을 안 말한다 — {ids:?}"
        );
    }

    /// 경고의 id 도 **급한 것이 앞**이다. 보는 쪽이 앞의 셋만 내므로,
    /// id 순으로 두면 급한 것이 뒤에 숨는다.
    #[test]
    fn dangling_warnings_put_the_urgent_first() {
        let mut rows = vec![make("argos-0001", Kind::Epic, "todo")];
        for (n, p) in [("argos-aaa1", 3), ("argos-aaa2", 3), ("argos-zzz9", 0)] {
            let mut i = make(n, Kind::Issue, "todo");
            i.epic = Some("argos-nope".into());
            i.priority = Some(p);
            rows.push(i);
        }
        let st = status(&rows, &[], &cfg(), "2026-09-11T00:00:00Z");
        let w = st.warnings.iter().find(|w| w.kind == "dangling_epic").expect("경고가 없다");
        assert_eq!(w.ids.first().map(String::as_str), Some("argos-zzz9"), "{:?}", w.ids);
    }

    /// 마일스톤 아닌 것을 가리킨 줄을 "마일스톤 있음" 으로 세지 않는다 —
    /// 그러면 `no_milestone` 이 그 줄을 오히려 빼 버린다.
    #[test]
    fn a_wrong_kind_milestone_does_not_count_as_having_one() {
        let mut bad = make("argos-0005", Kind::Issue, "todo");
        bad.milestone = Some("argos-0002".into()); // 에픽이다
        let issues = vec![
            make("argos-0001", Kind::Milestone, "todo"),
            make("argos-0002", Kind::Epic, "todo"),
            bad,
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        let no_mile = st.warnings.iter().find(|w| w.kind == "no_milestone");
        assert!(
            no_mile.is_some_and(|w| w.ids.contains(&"argos-0005".to_string())),
            "마일스톤 없는 것으로 안 셌다 — {:?}",
            st.warnings.iter().map(|w| (w.kind, &w.ids)).collect::<Vec<_>>()
        );
    }

    /// 에픽 표도 **급한 것이 위로** 온다. 이슈 목록은 이미 그런데 에픽만
    /// 파일 순(=id 순)이면, 한 화면에서 차례가 둘이라 보는 쪽이 규칙을 못 세운다.
    #[test]
    fn groupings_are_listed_urgent_first() {
        let mut low = make("argos-0001", Kind::Epic, "todo");
        low.priority = Some(3);
        let mut high = make("argos-0009", Kind::Epic, "todo"); // id 는 뒤인데 급하다
        high.priority = Some(0);
        let mid = make("argos-0005", Kind::Epic, "todo"); // 기본값 p2
        let issues = vec![low, mid, high];

        let rolls = rollup(&issues, &cfg());
        let ids: Vec<&str> = rolls.iter().filter_map(|r| r.id.as_deref()).collect();
        assert_eq!(ids, ["argos-0009", "argos-0005", "argos-0001"], "{ids:?}");

        // 묶음 없는 것은 우선순위가 없으니 늘 끝이다
        assert_eq!(rolls.last().unwrap().id, None);
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

    /// 막고 있는 것이 안 끝났으면 못 집는다. 끝나면 다시 집을 수 있다.
    #[test]
    fn ready_excludes_what_is_still_blocked() {
        let blocker_open = make("argos-0001", Kind::Issue, "todo");
        let mut blocked = make("argos-0002", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0001".into()];
        let open = [blocker_open, blocked.clone()];
        let got: Vec<&str> = ready(&open, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0001"], "{got:?}");

        let mut blocker_done = make("argos-0001", Kind::Issue, "done");
        blocker_done.status = Status::new("done");
        let done = [blocker_done, blocked];
        let got: Vec<&str> = ready(&done, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0002"], "끝난 막음은 더 이상 막지 않는다 — {got:?}");
    }

    /// 없는 이슈를 가리키는 `blocked_by` 는 막지 않는다 — 끊긴 참조는
    /// `moai status` 가 드러내지, `ready` 가 조용히 영원히 막지 않는다.
    #[test]
    fn a_dangling_blocker_does_not_block_forever() {
        let mut i = make("argos-0001", Kind::Issue, "todo");
        i.blocked_by = vec!["argos-9999".into()];
        let all = [i];
        let got: Vec<&str> = ready(&all, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0001"]);
    }

    #[test]
    fn creates_cycle_catches_direct_and_transitive_and_self() {
        let issues = vec![
            make("argos-0001", Kind::Issue, "todo"),
            {
                let mut i = make("argos-0002", Kind::Issue, "todo");
                i.blocked_by = vec!["argos-0001".into()]; // 0001 이 0002 를 막는다
                i
            },
        ];
        // 0002 가 0001 을 막으면 고리(0001→0002→0001).
        assert!(creates_cycle(&issues, "argos-0002", "argos-0001"));
        // 관계없는 방향은 고리가 아니다.
        assert!(!creates_cycle(&issues, "argos-0001", "argos-0003"));
        // 스스로를 막는 것도 고리로 본다.
        assert!(creates_cycle(&issues, "argos-0001", "argos-0001"));
    }

    /// 오래 막혀 있는 것을 `status` 가 드러낸다.
    #[test]
    fn status_warns_about_long_blocked_work() {
        let now = "2026-09-11T00:00:00Z";
        let mut stuck = make("argos-0002", Kind::Issue, "todo");
        stuck.blocked_by = vec!["argos-0001".into()];
        stuck.status_since = "2026-09-01T00:00:00Z".to_string(); // 열흘
        let issues = vec![make("argos-0001", Kind::Issue, "todo"), stuck];
        let st = status(&issues, &[], &cfg(), now);
        let w = st.warnings.iter().find(|w| w.kind == "blocked_stale").expect("경고가 없다");
        assert_eq!(w.ids, ["argos-0002"]);

        // 막 막힌 것은 아직 말하지 않는다.
        let mut fresh = make("argos-0002", Kind::Issue, "todo");
        fresh.blocked_by = vec!["argos-0001".into()];
        fresh.status_since = now.to_string();
        let issues = vec![make("argos-0001", Kind::Issue, "todo"), fresh];
        let st = status(&issues, &[], &cfg(), now);
        assert!(st.warnings.iter().all(|w| w.kind != "blocked_stale"));
    }

    /// 막는 쪽이 사라지면 `ready` 는 조용히 넘어가지만 `status` 는 드러낸다.
    #[test]
    fn status_warns_about_a_dangling_blocker() {
        let mut i = make("argos-0001", Kind::Issue, "todo");
        i.blocked_by = vec!["argos-9999".into()];
        let st = status(&[i], &[], &cfg(), "2026-09-11T00:00:00Z");
        let w = st.warnings.iter().find(|w| w.kind == "dangling_blocked_by").expect("경고가 없다");
        assert_eq!(w.ids, ["argos-0001"]);
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

    /// 마일스톤은 에픽을 거쳐 물려받는다. 이슈마다 적게 하면 에픽을 옮길 때
    /// 멤버를 전부 따라 고쳐야 하고, 반드시 하나는 빠뜨린다.
    #[test]
    fn milestones_come_down_through_epics() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-m001".into());
        let mut own = make("argos-0005", Kind::Issue, "todo");
        own.milestone = Some("argos-m002".into()); // 제 것이 이긴다
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            epic,
            member("argos-0002", "argos-0001", "todo"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
            own,
            make("argos-0009", Kind::Issue, "todo"),
        ];
        let m = milestones(&issues);
        assert_eq!(m.get("argos-0002"), Some(&"argos-m001"), "에픽을 안 거쳤다");
        assert_eq!(m.get("argos-0002.aaa"), Some(&"argos-m001"), "손자가 안 물려받았다");
        assert_eq!(m.get("argos-0005"), Some(&"argos-m002"), "제가 적은 것이 져 버렸다");
        assert_eq!(m.get("argos-0009"), None);
    }

    /// **에픽이 마일스톤을 이긴다.** 멤버가 제 `milestone` 을 따로 적어도
    /// 자리는 에픽이 정한다 — `nav` 가 그렇게 걸고, 세는 자와 거는 자가
    /// 갈라지면 `show <마일스톤>` 이 "멤버 1건" 이라 말하면서 그 1건을 못 낸다.
    #[test]
    fn an_epic_outranks_a_members_own_milestone() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-m001".into());
        let mut member_elsewhere = member("argos-0002", "argos-0001", "todo");
        member_elsewhere.milestone = Some("argos-m002".into());
        // 마일스톤을 안 가진 에픽. 그 멤버는 제 것을 적었어도 마일스톤 없음이다.
        let bare = make("argos-0003", Kind::Epic, "todo");
        let mut member_of_bare = member("argos-0004", "argos-0003", "todo");
        member_of_bare.milestone = Some("argos-m002".into());
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            make("argos-m002", Kind::Milestone, "todo"),
            epic,
            member_elsewhere,
            bare,
            member_of_bare,
        ];
        let m = milestones(&issues);
        assert_eq!(m.get("argos-0002"), Some(&"argos-m001"), "제 마일스톤이 에픽을 이겼다");
        assert_eq!(m.get("argos-0004"), None, "에픽에 없는 마일스톤이 멤버에게서 생겼다");
    }

    /// **없는 에픽을 가리킨 줄은 제 마일스톤으로도 안 센다.** `nav` 는 그 줄을
    /// `(길 잃음)` 에 넣으므로, 여기서 세면 `moai show <마일스톤>` 이
    /// `멤버 0/1` 이라 말하면서 그 1건을 목록에 못 내는 moai-lhbh 가 참조 하나
    /// 끊긴 날 그대로 되살아난다.
    #[test]
    fn a_dangling_epic_does_not_fall_back_to_its_own_milestone() {
        let mut lost = make("argos-0005", Kind::Issue, "todo");
        lost.epic = Some("argos-nope".into());
        lost.milestone = Some("argos-m001".into());
        let issues = vec![make("argos-m001", Kind::Milestone, "todo"), lost];
        assert_eq!(milestones(&issues).get("argos-0005"), None);
    }

    /// 종류가 틀린 에픽 참조도 같다. 이슈를 에픽 자리에 적으면 `nav` 는 그 줄을
    /// `(길 잃음)` 에 넣는데, 여기서 그 이슈의 마일스톤을 타고 올라가 세면
    /// `멤버 0/2` 라 말하면서 목록에는 하나만 나온다 — 실제로 그랬다.
    #[test]
    fn a_wrong_kind_epic_does_not_lend_its_milestone() {
        let mut host = make("argos-0002", Kind::Issue, "todo");
        host.milestone = Some("argos-m001".into());
        let mut lost = make("argos-0005", Kind::Issue, "todo");
        lost.epic = Some("argos-0002".into()); // 에픽이 아니라 이슈다
        let issues = vec![make("argos-m001", Kind::Milestone, "todo"), host, lost];
        let m = milestones(&issues);
        assert_eq!(m.get("argos-0002"), Some(&"argos-m001"));
        assert_eq!(m.get("argos-0005"), None, "에픽 아닌 것의 마일스톤을 빌려 왔다");
    }

    /// 묶음은 일이 아니다. 세면 보드의 숫자가 할 일과 묶음을 합친 것이 된다.
    #[test]
    fn groupings_are_not_work() {
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert_eq!(st.total, 1);
        assert_eq!(st.counts.get("todo"), Some(&1));
        assert!(ready(&issues, &cfg()).iter().all(|i| i.id == "argos-0002"));
    }

    /// 마일스톤을 안 쓰는 저장소에는 마일스톤 이야기를 꺼내지 않는다.
    #[test]
    fn milestones_stay_quiet_until_used() {
        let plain = vec![make("argos-0001", Kind::Issue, "todo")];
        let st = status(&plain, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert!(st.milestones.is_empty());
        assert!(!kinds(&st).contains(&"no_milestone"), "{:?}", kinds(&st));

        let mut used = plain.clone();
        used.push(make("argos-m001", Kind::Milestone, "todo"));
        let st = status(&used, &[], &cfg(), "2026-09-01T00:00:00Z");
        assert_eq!(st.milestones.len(), 1);
        assert!(kinds(&st).contains(&"no_milestone"));
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
    // ── idea 는 일이 아니다 ──────────────────────────────────────────
    //
    // **여기가 이 에픽에서 제일 조용히 틀어질 자리다.** 경고 하나가 idea 를
    // 세기 시작하면 사람이 생각을 담을수록 화면이 시끄러워지고, 그러면
    // 안 담게 된다. 저절로 빠지는 곳이 대부분이라 시험으로 못 박는다.

    fn idea(id: &str) -> Issue {
        make(id, Kind::Idea, "todo")
    }

    #[test]
    fn an_idea_never_comes_up_as_ready() {
        let issues = vec![idea("argos-0001"), make("argos-0009", Kind::Issue, "todo")];
        let picks: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(picks, ["argos-0009"], "담아 둔 생각이 집을 일로 올라왔다");
    }

    #[test]
    fn an_idea_is_not_counted_on_the_board() {
        let issues = vec![idea("argos-0001"), make("argos-0009", Kind::Issue, "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert_eq!(st.total, 1, "idea 를 이슈로 셌다");
        assert_eq!(st.counts.get("todo"), Some(&1), "{:?}", st.counts);
    }

    /// 에픽에 든 idea 도 진행률을 움직이지 않는다. 움직이면 생각을 담을수록
    /// 그 에픽이 덜 끝난 것으로 보인다.
    #[test]
    fn an_idea_does_not_move_an_epics_rollup() {
        let mut inside = idea("argos-0003");
        inside.epic = Some("argos-0001".into());
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "done"),
            inside,
        ];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!((r.total, r.done, r.percent), (1, 1, Some(100)), "{r:?}");
    }

    /// **에픽 없이 사는 것이 정상이다.** 여기 걸리면 담는 족족 잔소리가 는다.
    #[test]
    fn an_idea_without_an_epic_is_not_a_warning() {
        let issues: Vec<Issue> = (0..9).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"no_epic"), "{kinds:?}");
    }

    /// 방치 검사에도 안 걸린다. 담아 둔 생각은 오래 있는 것이 정상이라,
    /// 나이로 잔소리하면 오래된 저장소일수록 화면이 시끄러워진다.
    #[test]
    fn an_old_idea_is_not_rotting() {
        let mut old = idea("argos-0001");
        old.status = Status::new("review");
        let issues = vec![old];
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"stale_review"), "{kinds:?}");
        assert!(!kinds.contains(&"stale_progress"), "{kinds:?}");
        assert!(!kinds.contains(&"wip_overload"), "{kinds:?}");
    }

    /// 흐름도 일만 센다. idea 를 세면 "쌓이는 중" 이 담은 생각 수를 말하게 되고,
    /// 그 숫자를 보고 사람이 담기를 멈춘다.
    #[test]
    fn the_flow_line_counts_work_only() {
        let issues = vec![idea("argos-0001"), make("argos-0009", Kind::Issue, "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-02T00:00:00Z");
        assert_eq!(st.flow.created, 1, "{:?}", st.flow);
    }
    /// **쌓이는 것 자체는 문제가 아니고, 쌓인 줄 모르는 것이 문제다.** 담는
    /// 비용을 0 으로 만들었으니 쌓인다 — 그래서 `status` 가 한 줄로 비춘다.
    #[test]
    fn a_pile_of_ideas_shows_up_in_status() {
        let quiet: Vec<Issue> = (0..IDEA_PILE - 1).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&quiet, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert!(
            !st.warnings.iter().any(|w| w.kind == "idea_pile"),
            "몇 개 안 되는데 벌써 말한다 — 담을 때마다 잔소리가 는다"
        );

        let mut piled = quiet;
        piled.push(idea("argos-0009"));
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        let w = st
            .warnings
            .iter()
            .find(|w| w.kind == "idea_pile")
            .expect("쌓였는데 아무 말도 안 한다");
        assert_eq!(w.count, IDEA_PILE);
        assert_eq!(w.oldest, Some(10), "가장 오래된 것의 나이를 안 말한다 — {w:?}");
        assert_eq!(w.days, None, "임계값 자리에 나이를 담았다 — {w:?}");
        assert!(w.notice, "알림이 경고로 선다 — {w:?}");
        assert_eq!(w.hint.as_deref(), Some("moai idea ls"));
        // **막지 않는다.** 여기가 비영 종료를 하면 이건 린트고, 린트는 게이트다.
        assert!(!w.fatal);
        assert!(!st.broken());
    }

    /// 펼쳐서 닫은 생각은 더 이상 쌓인 것이 아니다. 세면 `promote` 를 쓸수록
    /// 잔소리가 늘어, 시킨 대로 한 사람이 벌을 받는다.
    #[test]
    fn a_promoted_idea_leaves_the_pile() {
        let mut piled: Vec<Issue> = (0..IDEA_PILE).map(|n| idea(&format!("argos-000{n}"))).collect();
        piled[0].status = Status::new("done");
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert!(!st.warnings.iter().any(|w| w.kind == "idea_pile"), "{:?}", st.warnings);
    }
    /// **알림은 경고 수에 안 든다.** 배너가 "드러난 것 N건" 이라 말하는데
    /// 담아 둔 생각이 거기 들면, 담을수록 고칠 것이 늘었다고 말하게 된다.
    #[test]
    fn a_notice_is_not_counted_among_the_warnings() {
        let piled: Vec<Issue> = (0..IDEA_PILE).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert_eq!(st.warnings.iter().filter(|w| !w.notice).count(), 0, "{:?}", st.warnings);
        assert!(!st.broken(), "알림으로 비영 종료한다");
    }
    // ── 미룬 것은 지금 계획이 아니다 ─────────────────────────────────

    fn deferred(id: &str, status: &str) -> Issue {
        let mut i = make(id, Kind::Issue, status);
        i.deferred_at = Some("2026-09-01T00:00:00Z".into());
        i
    }

    #[test]
    fn a_deferred_issue_never_comes_up_as_ready() {
        let issues = vec![deferred("argos-0001", "todo"), make("argos-0009", Kind::Issue, "todo")];
        let picks: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(picks, ["argos-0009"], "미뤄 둔 것이 집을 일로 올라왔다");
    }

    #[test]
    fn a_deferred_issue_is_not_counted_on_the_board() {
        let issues = vec![deferred("argos-0001", "todo"), make("argos-0009", Kind::Issue, "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert_eq!(st.total, 1, "미룬 것을 보드가 셌다");
        assert_eq!(st.counts.get("todo"), Some(&1), "{:?}", st.counts);
    }

    /// **미룬 것은 정의상 안 건드리는 것이다.** 방치·벌여 놓음 경고가 세기
    /// 시작하면 미룰수록 잔소리가 늘고, 그러면 안 미루고 그냥 쌓아 둔다.
    #[test]
    fn a_deferred_issue_is_not_nagged_about() {
        let issues: Vec<Issue> = (0..9)
            .map(|n| deferred(&format!("argos-000{n}"), if n % 2 == 0 { "in_progress" } else { "review" }))
            .collect();
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        for quiet in ["wip_overload", "stale_progress", "stale_review", "no_epic"] {
            assert!(!kinds.contains(&quiet), "{quiet} 가 미룬 것을 셌다 — {kinds:?}");
        }
    }

    /// 막힌 채로 미뤄 둔 것도 마찬가지다. 미루는 것이 그 막힘을 "계획이 멈춘
    /// 자리" 에서 빼는 일이다.
    #[test]
    fn a_deferred_blocker_is_not_a_stalled_plan() {
        let mut blocked = deferred("argos-0009", "todo");
        blocked.blocked_by = vec!["argos-0001".into()];
        let issues = vec![make("argos-0001", Kind::Issue, "todo"), blocked];
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"blocked_stale"), "{kinds:?}");
    }

    /// **에픽의 계획은 줄어들지 않는다.** 다섯 중 둘을 미뤘다고 `3건짜리
    /// 에픽` 이 되면, 미루는 것이 계획을 고쳐 쓰는 일이 된다. 보드는 "지금
    /// 할 수 있는 것" 을, 롤업은 "이 계획 전부" 를 센다.
    #[test]
    fn deferring_does_not_shrink_the_plan() {
        let mut inside = deferred("argos-0003", "todo");
        inside.epic = Some("argos-0001".into());
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "done"),
            inside,
        ];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!((r.total, r.done), (2, 1), "미뤘다고 계획이 줄었다 — {r:?}");
    }
    /// **세어 놓고 못 보여 주는 수를 만들지 않는다.** 미뤄 둔 생각을
    /// `idea_pile` 이 세면, 그 줄이 가리키는 `moai idea ls` 는 그것을 숨긴다.
    #[test]
    fn a_deferred_thought_leaves_the_pile_for_its_own_line() {
        let mut piled: Vec<Issue> = (0..IDEA_PILE).map(|n| idea(&format!("argos-000{n}"))).collect();
        piled[0].deferred_at = Some("2026-09-01T00:00:00Z".into());
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"idea_pile"), "숨길 것을 세었다 — {kinds:?}");
        assert!(kinds.contains(&"deferred"), "제 이름으로도 안 말한다 — {kinds:?}");
    }

    /// 미루는 길은 무엇이든 받는다. 드러내는 자리가 `is_work` 로 좁히면
    /// 미뤄 둔 에픽이 목록에서만 사라지고 아무 데서도 안 보인다 — 보이는
    /// 유일한 자리가 그 줄만 안 비추는 꼴이다.
    #[test]
    fn a_deferred_grouping_is_still_named() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let st = status(&[epic], &[], &cfg(), "2026-09-11T00:00:00Z");
        let w = st.warnings.iter().find(|w| w.kind == "deferred").expect("미뤄 둔 에픽이 안 보인다");
        assert_eq!(w.count, 1);
        assert!(w.notice);
    }
}
