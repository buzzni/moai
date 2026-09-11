//! 탐색 모델 — 이슈 더미를 디렉터리처럼 보는 법.
//!
//! **터미널도 ratatui 도 모른다.** `report`·`query` 와 같은 자리의 순수 모듈이라
//! TTY 없이 시험된다. TUI 는 이것 위에 그림만 그린다.
//!
//! # 자리를 정하는 법은 하나다
//!
//! 층마다 "이 디렉터리에는 이런 것들이 온다" 는 규칙을 따로 쓰면, 규칙 넷이
//! 서로 어긋나 같은 이슈가 두 곳에 나오거나 아예 사라진다. 그래서 여기는
//! **[`Index::home_of`] 하나**만 정하고, 목록([`Index::entries`])은 그것의
//! 역상으로 만든다. 그러면 "한 노드는 정확히 한 번 나타난다" 가 지켜야 할
//! 규칙이 아니라 **증명되는 성질**이 된다 — `every_issue_lands_exactly_once`.
//!
//! # 에픽이 마일스톤보다 세다
//!
//! 이슈의 `milestone` 은 제 것이 에픽의 것을 이긴다(`report::milestones`).
//! 하지만 **자리**를 정할 때는 에픽을 따른다 — 이슈는 제 에픽 밑에 걸리고,
//! 에픽은 제 마일스톤 밑에 걸린다. 그러지 않으면 에픽 하나가 두 마일스톤
//! 밑에 나타나야 하고, 디렉터리가 두 곳에 있는 탐색기는 못 쓴다.
//! 그래서 제 마일스톤을 따로 적은 이슈는 **에픽을 따라 놓이고**, 그 사실은
//! 상세 패널이 제 마일스톤을 그대로 보여 주는 것으로 드러낸다.

use crate::model::{Issue, Kind};
use std::collections::BTreeMap;

/// 경로 한 마디.
///
/// `Milestone(None)` 은 `(마일스톤 없음)` 바구니다. 에픽 없는 이슈는 따로
/// 바구니를 두지 않고 제 마일스톤(또는 뿌리)에 파일처럼 그냥 놓인다 — MC 에서
/// 파일과 디렉터리가 한 목록에 나란히 있는 것과 같다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Milestone(Option<String>),
    Epic(String),
    Issue(String),
    /// 자리를 정할 수 없는 것들. 없는 에픽을 가리키거나, 에픽이 아닌 것을
    /// 에픽이라 가리키는 줄이 여기 온다. **조용히 사라지게 두지 않는다** —
    /// 탐색기에서 항목이 없어지는 것은 줄이 빠지는 것보다 나쁘다.
    Lost,
}

pub type Path = Vec<Seg>;

/// 목록의 한 줄. `at` 은 **id 가 아니라 `issues` 의 첨자다** — 중복 id 는 이
/// 도구가 거부하지 않고 드러내기만 하는 상태(`duplicate_id`)라 실재할 수 있고,
/// id 로 가리키면 그 둘이 조용히 하나로 접힌다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// 들어갈 수 있는 것. `at` 은 그 에픽·마일스톤 자신의 줄, 바구니면 `None`.
    Dir { seg: Seg, at: Option<usize> },
    /// 더 들어갈 데 없는 일.
    Leaf { at: usize },
}

impl Entry {
    pub fn at(&self) -> Option<usize> {
        match self {
            Entry::Dir { at, .. } => *at,
            Entry::Leaf { at } => Some(*at),
        }
    }
}

/// 한 번 만들어 두고 쓰는 색인.
///
/// **`String` 을 소유한다.** `report::groups` 는 `&[Issue]` 를 빌린
/// `BTreeMap<&str,&str>` 을 주는데, 그것과 이슈를 한 구조체에 같이 담으면
/// 자기참조 구조체가 된다.
pub struct Index {
    /// 이슈 첨자 → 그것이 걸리는 자리.
    homes: Vec<Path>,
    /// 이슈 첨자 → 제 밑에 걸린 것이 있는가. **미리 센다** — `entries` 는 매
    /// 프레임 불리므로 그때 세면 목록 하나 그리는 데 O(이슈 수²) 다.
    has_kids: Vec<bool>,
    has_milestones: bool,
}

impl Index {
    /// 적재·갱신 때 한 번만 만든다. 매 프레임 만들 것이 아니다.
    pub fn of(issues: &[Issue]) -> Index {
        // 소속 판정은 새로 짜지 않는다. `report` 가 상속 규칙을 이미 갖고 있고,
        // 둘이 갈라지면 목록이 세는 곳과 그리는 곳이 어긋난다.
        let epic_of: BTreeMap<String, String> = crate::report::groups(issues)
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let milestone_of: BTreeMap<String, String> = crate::report::milestones(issues)
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let kind_of: BTreeMap<&str, Kind> = issues.iter().map(|i| (i.id.as_str(), i.kind)).collect();
        let has_milestones = issues.iter().any(|i| i.kind == Kind::Milestone);

        // id → 첨자. 이게 없으면 부모를 찾을 때마다 전체를 훑어 O(이슈 수²·깊이) 다.
        let by_id: BTreeMap<&str, usize> = issues.iter().enumerate().map(|(at, i)| (i.id.as_str(), at)).collect();

        let ctx = Ctx { issues, epic_of: &epic_of, milestone_of: &milestone_of, kind_of: &kind_of, by_id: &by_id, has_milestones };
        let homes: Vec<Path> = (0..issues.len()).map(|at| ctx.home(at)).collect();

        let parents: std::collections::BTreeSet<&str> = homes
            .iter()
            .filter_map(|h| match h.last() {
                Some(Seg::Issue(id)) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let has_kids = issues.iter().map(|i| parents.contains(i.id.as_str())).collect();
        Index { homes, has_kids, has_milestones }
    }

    /// 마일스톤을 하나도 안 쓰는 저장소는 그 층을 통째로 건너뛴다.
    /// `report::status` 가 이미 같은 원칙으로 마일스톤에 침묵한다 — 안 쓰는
    /// 것 때문에 모든 경로가 `(마일스톤 없음)/` 으로 시작하게 두지 않는다.
    pub fn has_milestones(&self) -> bool {
        self.has_milestones
    }

    /// 그 이슈가 걸리는 **단 하나의** 자리.
    pub fn home_of(&self, at: usize) -> &Path {
        &self.homes[at]
    }

    /// 그 자리를 home 으로 갖는 것들. **`home_of` 의 역상이다** — 목록 규칙을
    /// 따로 쓰지 않는 것이 빠짐도 겹침도 없음을 보장하는 유일한 이유다.
    pub fn entries(&self, issues: &[Issue], path: &Path) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        let mut buckets: Vec<Seg> = Vec::new();

        for (at, home) in self.homes.iter().enumerate() {
            // 바로 이 자리에 사는 것
            if home == path {
                out.push(match issues[at].kind {
                    Kind::Milestone => Entry::Dir { seg: Seg::Milestone(Some(issues[at].id.clone())), at: Some(at) },
                    Kind::Epic => Entry::Dir { seg: Seg::Epic(issues[at].id.clone()), at: Some(at) },
                    Kind::Issue if self.has_kids[at] => {
                        Entry::Dir { seg: Seg::Issue(issues[at].id.clone()), at: Some(at) }
                    }
                    Kind::Issue => Entry::Leaf { at },
                });
                continue;
            }
            // 이 자리 **바로 밑의 바구니**에 사는 것 — 바구니는 제 줄이 없으므로
            // 사는 것이 있을 때만 생긴다.
            if home.len() == path.len() + 1
                && home.starts_with(path)
                && matches!(home[path.len()], Seg::Milestone(None) | Seg::Lost)
                && !buckets.contains(&home[path.len()])
            {
                buckets.push(home[path.len()].clone());
            }
        }

        out.sort_by_key(|e| sort_key(issues, e));
        // 바구니는 늘 끝에. 정상인 것이 먼저 보여야 한다.
        buckets.sort_by_key(|s| matches!(s, Seg::Lost));
        out.extend(buckets.into_iter().map(|seg| Entry::Dir { seg, at: None }));
        out
    }

    /// 화면에 낼 이름. 바구니는 제 줄이 없으므로 여기서 이름을 얻는다.
    pub fn label(&self, issues: &[Issue], e: &Entry) -> String {
        match e.at() {
            Some(at) => issues[at].title.clone(),
            None => match e {
                Entry::Dir { seg: Seg::Milestone(None), .. } => "(마일스톤 없음)".into(),
                Entry::Dir { seg: Seg::Lost, .. } => "(길 잃음)".into(),
                _ => String::new(),
            },
        }
    }
}

/// 디렉터리 먼저, 그다음 우선순위, 그다음 id. `moai show` 의 차례와 같은 뜻이다.
fn sort_key(issues: &[Issue], e: &Entry) -> (u8, u8, String) {
    let at = e.at().expect("바구니는 여기 오지 않는다");
    let dir = u8::from(matches!(e, Entry::Leaf { .. }));
    (dir, issues[at].priority(), issues[at].id.clone())
}

/// `Index::of` 안에서만 쓰는 계산판. 빌린 지도를 들고 다니므로 밖으로 나가지 않는다.
struct Ctx<'a> {
    issues: &'a [Issue],
    epic_of: &'a BTreeMap<String, String>,
    milestone_of: &'a BTreeMap<String, String>,
    kind_of: &'a BTreeMap<&'a str, Kind>,
    by_id: &'a BTreeMap<&'a str, usize>,
    has_milestones: bool,
}

impl Ctx<'_> {
    fn is(&self, id: &str, kind: Kind) -> bool {
        self.kind_of.get(id) == Some(&kind)
    }

    /// 그 이슈가 걸리는 단 하나의 자리.
    fn home(&self, at: usize) -> Path {
        let me = &self.issues[at];
        match me.kind {
            // 마일스톤은 뿌리에 선다.
            Kind::Milestone => Vec::new(),
            // 에픽은 제 마일스톤 밑에. 마일스톤을 안 쓰는 저장소면 뿌리에.
            Kind::Epic => self.under_milestone(&me.id),
            Kind::Issue => self.home_of_work(at),
        }
    }

    fn under_milestone(&self, id: &str) -> Path {
        if !self.has_milestones {
            return Vec::new();
        }
        match self.milestone_of.get(id) {
            // 없는 마일스톤을 가리키는 것은 조용히 뿌리로 보내지 않는다.
            Some(m) if !self.is(m, Kind::Milestone) => vec![Seg::Lost],
            Some(m) => vec![Seg::Milestone(Some(m.clone()))],
            None => vec![Seg::Milestone(None)],
        }
    }

    fn home_of_work(&self, at: usize) -> Path {
        let me = &self.issues[at];
        // 부모가 실재하고 **같은 에픽**이면 부모 밑에 접힌다. 에픽이 다르면
        // 제 에픽으로 간다 — 롤업이 세는 곳과 화면이 그리는 곳을 맞추기 위해서다.
        if let Some(p) = crate::id::parent_of(&me.id)
            && let Some(&pat) = self.by_id.get(p)
            && self.issues[pat].kind == Kind::Issue
            && self.epic_of.get(&me.id) == self.epic_of.get(p)
        {
            let mut path = self.home_of_work(pat);
            path.push(Seg::Issue(p.to_string()));
            return path;
        }
        match self.epic_of.get(&me.id) {
            // 없는 에픽, 또는 에픽이 아닌 것을 에픽이라 가리키는 줄.
            Some(e) if !self.is(e, Kind::Epic) => vec![Seg::Lost],
            Some(e) => {
                let mut path = self.under_milestone(e);
                path.push(Seg::Epic(e.clone()));
                path
            }
            // 에픽이 없으면 제 마일스톤(또는 뿌리)에 파일처럼 놓인다.
            None => self.under_milestone(&me.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn make(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    fn epic_of(id: &str, epic: &str) -> Issue {
        let mut i = make(id, Kind::Issue);
        i.epic = Some(epic.into());
        i
    }

    /// 모든 줄이 어딘가에 **정확히 한 번** 나타난다. 이 모듈의 존재 이유다.
    ///
    /// 뿌리부터 훑어 모은 첨자의 다중집합이 `0..issues.len()` 과 같아야 한다.
    fn walk(index: &Index, issues: &[Issue]) -> Vec<usize> {
        fn go(index: &Index, issues: &[Issue], path: &Path, out: &mut Vec<usize>) {
            for e in index.entries(issues, path) {
                if let Some(at) = e.at() {
                    out.push(at);
                }
                if let Entry::Dir { seg, .. } = &e {
                    let mut deeper = path.clone();
                    deeper.push(seg.clone());
                    go(index, issues, &deeper, out);
                }
            }
        }
        let mut out = Vec::new();
        go(index, issues, &Vec::new(), &mut out);
        out.sort();
        out
    }

    fn assert_exactly_once(issues: &[Issue]) {
        let index = Index::of(issues);
        let seen = walk(&index, issues);
        let want: Vec<usize> = (0..issues.len()).collect();
        assert_eq!(seen, want, "빠졌거나 겹쳤다");
    }

    /// 병적인 자료를 전부 한 더미에 넣고 빠짐도 겹침도 없음을 못 박는다.
    #[test]
    fn every_issue_lands_exactly_once() {
        let mut own_milestone = epic_of("argos-0007", "argos-0002");
        own_milestone.milestone = Some("argos-0009".into()); // 에픽의 것과 다르다
        let mut child_elsewhere = make("argos-0005.aa1", Kind::Issue);
        child_elsewhere.epic = Some("argos-0003".into()); // 부모와 다른 에픽
        let mut epic_in_milestone = make("argos-0002", Kind::Epic);
        epic_in_milestone.milestone = Some("argos-0001".into());

        let issues = vec![
            make("argos-0001", Kind::Milestone),
            epic_in_milestone,
            make("argos-0003", Kind::Epic),           // 마일스톤 없는 에픽
            epic_of("argos-0004", "argos-0002"),      // 평범한 멤버
            epic_of("argos-0005", "argos-0002"),      // 자식을 가진 멤버
            make("argos-0005.aa1", Kind::Issue),      // 그 자식 (에픽 물려받음)
            own_milestone,                             // 제 마일스톤을 따로 적은 것
            epic_of("argos-0008", "argos-zzzz"),      // 없는 에픽 → 길 잃음
            make("argos-0010", Kind::Issue),          // 아무 데도 안 딸린 것
            make("argos-0011.bb2", Kind::Issue),      // 부모 줄이 없는 고아
            child_elsewhere,                           // 제 에픽이 부모와 다른 자식
        ];
        assert_exactly_once(&issues);
    }

    /// 마일스톤을 안 쓰는 저장소는 그 층이 아예 없다. 지금 이 저장소가 그렇다.
    #[test]
    fn a_repo_without_milestones_has_no_milestone_level() {
        let issues = vec![
            make("argos-0002", Kind::Epic),
            epic_of("argos-0004", "argos-0002"),
            make("argos-0010", Kind::Issue), // 에픽 없는 것은 뿌리에 파일처럼
        ];
        let index = Index::of(&issues);
        assert!(!index.has_milestones());

        let root = index.entries(&issues, &Vec::new());
        assert_eq!(
            root,
            vec![
                Entry::Dir { seg: Seg::Epic("argos-0002".into()), at: Some(0) },
                Entry::Leaf { at: 2 },
            ]
        );
        assert_exactly_once(&issues);
    }

    /// 마일스톤을 쓰면 뿌리는 마일스톤이고, 딸리지 않은 것은 바구니로 간다.
    #[test]
    fn milestones_become_the_root_when_used() {
        let mut epic = make("argos-0002", Kind::Epic);
        epic.milestone = Some("argos-0001".into());
        let issues = vec![
            make("argos-0001", Kind::Milestone),
            epic,
            epic_of("argos-0004", "argos-0002"),
            make("argos-0003", Kind::Epic), // 마일스톤 없는 에픽
        ];
        let index = Index::of(&issues);
        assert!(index.has_milestones());

        let root = index.entries(&issues, &Vec::new());
        assert_eq!(
            root,
            vec![
                Entry::Dir { seg: Seg::Milestone(Some("argos-0001".into())), at: Some(0) },
                Entry::Dir { seg: Seg::Milestone(None), at: None },
            ]
        );

        let inside = index.entries(&issues, &vec![Seg::Milestone(Some("argos-0001".into()))]);
        assert_eq!(inside, vec![Entry::Dir { seg: Seg::Epic("argos-0002".into()), at: Some(1) }]);
        assert_exactly_once(&issues);
    }

    /// 자식은 부모 밑에 접힌다. **단 제 에픽이 부모와 다르면 제 에픽으로 간다** —
    /// 그러지 않으면 롤업이 세는 곳과 화면이 그리는 곳이 어긋난다.
    #[test]
    fn a_child_nests_under_its_parent_unless_its_epic_differs() {
        let mut elsewhere = make("argos-0004.aa1", Kind::Issue);
        elsewhere.epic = Some("argos-0003".into());
        let issues = vec![
            make("argos-0002", Kind::Epic),
            make("argos-0003", Kind::Epic),
            epic_of("argos-0004", "argos-0002"),
            make("argos-0004.bb2", Kind::Issue), // 에픽을 물려받는다 → 부모 밑
            elsewhere,                            // 제 에픽이 다르다 → 그 에픽 밑
        ];
        let index = Index::of(&issues);

        assert_eq!(index.home_of(3), &vec![Seg::Epic("argos-0002".into()), Seg::Issue("argos-0004".into())]);
        assert_eq!(index.home_of(4), &vec![Seg::Epic("argos-0003".into())]);
        assert_exactly_once(&issues);
    }

    /// 없는 에픽을 가리키는 줄은 사라지지 않고 `(길 잃음)` 으로 모인다.
    #[test]
    fn a_dangling_epic_reference_goes_to_the_lost_bucket() {
        let issues = vec![
            make("argos-0002", Kind::Epic),
            epic_of("argos-0008", "argos-zzzz"), // 없는 에픽
            epic_of("argos-0009", "argos-0008"), // 에픽이 아닌 것을 에픽이라 가리킨다
        ];
        let index = Index::of(&issues);
        assert_eq!(index.home_of(1), &vec![Seg::Lost]);
        assert_eq!(index.home_of(2), &vec![Seg::Lost]);

        let root = index.entries(&issues, &Vec::new());
        assert!(root.contains(&Entry::Dir { seg: Seg::Lost, at: None }), "{root:?}");
        assert_exactly_once(&issues);
    }

    /// 부모 줄이 없는 고아도 자리를 갖는다.
    #[test]
    fn an_orphan_child_still_gets_a_home() {
        let issues = vec![make("argos-0002", Kind::Epic), epic_of("argos-0011.bb2", "argos-0002")];
        let index = Index::of(&issues);
        assert_eq!(index.home_of(1), &vec![Seg::Epic("argos-0002".into())]);
        assert_exactly_once(&issues);
    }

    /// 빈 저장소가 무너지지 않는다.
    #[test]
    fn an_empty_repo_lists_nothing() {
        let index = Index::of(&[]);
        assert!(index.entries(&[], &Vec::new()).is_empty());
    }

    /// 중복 id 가 있어도 둘 다 보인다. 첨자로 가리키기 때문이다.
    #[test]
    fn duplicate_ids_both_stay_visible() {
        let issues = vec![make("argos-0010", Kind::Issue), make("argos-0010", Kind::Issue)];
        assert_exactly_once(&issues);
    }
}
