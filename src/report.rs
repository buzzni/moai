//! 세는 일과 고르는 일. **순수 함수다** — `&[Issue]` 만 보고 아무것도 찍지 않는다.
//!
//! **여기가 TUI 와의 계약이다.** 나중에 `ratatui` 는 `view` 를 건너뛰고
//! 이 모듈과 `query` 를 직접 부른다. 무엇을 셀지 정하는 코드가 `cmd/` 나
//! `view.rs` 에 있으면 표면이 늘 때마다 같은 것을 다시 짜야 한다.

use crate::config::Config;
use crate::model::{Issue, Kind};
use std::collections::BTreeMap;

/// 에픽 하나(또는 "에픽 없음")의 집계. **저장하지 않는다** — 멤버 하나를
/// 닫을 때 에픽 줄까지 써야 한다면 그 필드는 파생값이고, 두 줄 쓰기 중간에
/// 죽으면 에픽이 영원히 거짓말한다.
#[derive(Debug, Clone, PartialEq)]
pub struct Roll {
    /// 에픽의 id. "에픽 없음" 묶음이면 `None`.
    pub id: Option<String>,
    pub title: String,
    /// 칸 이름 → 건수. config 의 칸 차례를 따른다.
    pub counts: Vec<(String, usize)>,
    pub total: usize,
    pub done: usize,
}

impl Roll {
    /// 0~100. 멤버가 없으면 `None` — 0% 라고 말하면 "아직 안 한 에픽" 과
    /// "속을 안 채운 에픽" 이 구별되지 않는다.
    pub fn percent(&self) -> Option<u8> {
        (self.total > 0).then(|| (self.done * 100 / self.total) as u8)
    }
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
        let counts: Vec<(String, usize)> = cfg
            .statuses
            .iter()
            .map(|s| (s.clone(), members.iter().filter(|i| i.status.as_str() == s).count()))
            .collect();
        (counts, members.len(), members.iter().filter(|i| i.status.is_done()).count())
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
            let (counts, total, done) = tally(&members);
            Roll { id: Some(e.id.clone()), title: e.title.clone(), counts, total, done }
        })
        .collect();

    // 에픽이 아니면서 어느 에픽에도 안 딸린 것. 에픽 자신은 세지 않는다.
    let loose: Vec<&Issue> = issues
        .iter()
        .filter(|i| !is_epic(i) && !group.contains_key(i.id.as_str()))
        .collect();
    let (counts, total, done) = tally(&loose);
    out.push(Roll { id: None, title: "에픽 없음".into(), counts, total, done });
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
        rollup(issues, cfg)
            .into_iter()
            .map(|r| {
                let p = r.percent().unwrap_or(0);
                (r.id, p)
            })
            .collect();

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
        assert_eq!(r.percent(), Some(66));
        assert_eq!(r.counts, [("todo".into(), 1), ("in_progress".into(), 0), ("review".into(), 0), ("done".into(), 2)]);
    }

    /// 빈 에픽도 줄을 갖는다. 안 그러면 "계획만 세우고 안 채운 것" 이 사라진다.
    #[test]
    fn an_empty_epic_still_gets_a_row() {
        let issues = vec![make("argos-0001", Kind::Epic, "todo")];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!(r.total, 0);
        assert_eq!(r.percent(), None, "멤버 없는 에픽을 0% 라고 했다");
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
        assert_eq!(roll_of(&rolls, Some("argos-0001")).percent(), Some(100));
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
