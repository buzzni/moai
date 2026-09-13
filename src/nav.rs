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
//! 이슈는 제 에픽 밑에 걸리고, 에픽은 제 마일스톤 밑에 걸린다. 그러지 않으면
//! 에픽 하나가 두 마일스톤 밑에 나타나야 하고, 디렉터리가 두 곳에 있는
//! 탐색기는 못 쓴다. 그래서 제 마일스톤을 따로 적은 이슈도 **에픽을 따라
//! 놓인다**.
//!
//! **세는 쪽도 같은 규칙이다** — `report::milestones` 역시 에픽을 이기게 한다.
//! 한때 자리는 에픽이, 셈은 제 마일스톤이 이겨 머리글이 말하는 수와 눈에
//! 보이는 줄 수가 어긋났다(moai-lhbh).

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
    /// id → 첨자. 화면은 에픽·마일스톤·막는 것을 제목으로 풀어 내는데, 그때마다
    /// 전체를 훑으면 프레임 하나에 이슈 수에 비례한 훑기가 여러 번 돈다.
    by_id: BTreeMap<String, usize>,
    /// id → **그 줄이 실제로 선** 마일스톤 — `homes` 의 `Milestone(Some(..))` 마디.
    /// `report::milestones` 를 그대로 들면 안 된다: 생각은 에픽의 마일스톤으로
    /// 세면서도 `(마일스톤 없음)` 에 서므로, 세는 지도를 그리면 상세가 탐색기가
    /// 두지 않은 마일스톤을 댄다. 자리에서 읽으면 둘은 짓는 법으로 같다.
    milestone_of: BTreeMap<String, String>,
    /// id → 그 줄을 계획에서 뺀 줄(`report::deferred_roots`). 상세가 물려받은
    /// 미룸을 말하는 데 쓴다 — 프레임마다 조상을 다시 타지 않게.
    deferred_root: BTreeMap<String, String>,
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
        let has_milestones = issues.iter().any(|i| i.kind == Kind::Milestone);
        // **자리를 못 정하는 참조는 `report` 가 정한다.** 여기에 술어를 하나 더
        // 두면 자가 둘이 되고, 탐색기가 `(길 잃음)` 에 넣은 줄에 대해
        // `moai status` 가 침묵하는 일이 그렇게 생겼다.
        let misplaced: BTreeMap<String, crate::report::Misplace> = crate::report::misplaced(issues)
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        // id → 첨자. 이게 없으면 부모를 찾을 때마다 전체를 훑어 O(이슈 수²·깊이) 다.
        let by_id: BTreeMap<&str, usize> = issues.iter().enumerate().map(|(at, i)| (i.id.as_str(), at)).collect();

        let ctx = Ctx {
            issues,
            epic_of: &epic_of,
            milestone_of: &milestone_of,
            by_id: &by_id,
            misplaced: &misplaced,
            has_milestones,
        };
        let homes: Vec<Path> = (0..issues.len()).map(|at| ctx.home(at)).collect();

        // **첨자로 센다.** id 로 세면 같은 id 를 단 줄 둘이 나란히 디렉터리가
        // 되고, 자식은 `by_id` 가 고른 한 줄 밑에만 걸리므로 그 자식이 두 곳에
        // 나타난다 — 이 모듈이 증명하기로 한 성질(`every_issue_lands_exactly_once`)이
        // 바로 거기서 깨진다. 중복 id 는 이 도구가 거부하지 않고 드러내기만 하는
        // 상태(`duplicate_id`)라 실재한다.
        let mut has_kids = vec![false; issues.len()];
        for h in &homes {
            if let Some(Seg::Issue(id)) = h.last()
                && let Some(&at) = by_id.get(id.as_str())
            {
                has_kids[at] = true;
            }
        }
        let by_id = by_id.into_iter().map(|(id, at)| (id.to_string(), at)).collect();
        // 상세가 그리는 마일스톤은 **그 줄이 선 경로에서** 읽는다. 위의 셈 지도와
        // 자리가 갈리는 줄이 있다 — 생각은 에픽의 마일스톤을 세면서도 `(마일스톤
        // 없음)` 에 선다.
        let placed = issues
            .iter()
            .zip(&homes)
            .filter_map(|(i, home)| {
                home.iter().find_map(|seg| match seg {
                    Seg::Milestone(Some(m)) => Some((i.id.clone(), m.clone())),
                    _ => None,
                })
            })
            .collect();
        let deferred_root = crate::report::deferred_roots(issues)
            .into_iter()
            .map(|(id, root)| (id.to_string(), root.to_string()))
            .collect();
        Index { homes, has_kids, by_id, milestone_of: placed, deferred_root }
    }

    /// 그 줄이 **실제로 선** 마일스톤. 제 줄의 `milestone` 이 아니다 — 에픽이
    /// 마일스톤을 이기고 생각은 마일스톤 밑에 서지 않으므로, 제 값은 그 줄이 선
    /// 자리와 다를 수 있다.
    pub fn milestone_of(&self, id: &str) -> Option<&str> {
        self.milestone_of.get(id).map(String::as_str)
    }

    /// 그 줄을 계획에서 뺀 줄 — 제가 미뤘으면 저 자신, 물려받았으면 미룬 조상·묶음.
    pub fn deferred_root(&self, id: &str) -> Option<&str> {
        self.deferred_root.get(id).map(String::as_str)
    }

    /// 그 줄이 경로에서 갖는 마디.
    ///
    /// **한 곳에서만 정한다.** 자리를 정하는 곳과 들어가는 곳이 갈라지면
    /// `--path` 가 여는 데와 Enter 가 여는 데가 달라진다.
    pub fn seg_of(&self, issues: &[Issue], at: usize) -> Seg {
        match issues[at].kind {
            Kind::Milestone => Seg::Milestone(Some(issues[at].id.clone())),
            Kind::Epic => Seg::Epic(issues[at].id.clone()),
            // idea 는 묶음이 아니다 — 일과 같은 자리에 잎으로 선다.
            Kind::Issue | Kind::Idea => Seg::Issue(issues[at].id.clone()),
        }
    }

    /// 들어갈 수 있는가.
    ///
    /// **비었는지로 묻지 않는다.** 멤버 없는 에픽도 디렉터리다 — 비었다고
    /// 대신 부모를 열면 `--path <빈 에픽>` 이 그 에픽의 형제들을 돌려주고,
    /// 그 답을 다시 훑는 쪽은 제자리를 돌며 끝나지 않는다.
    pub fn is_dir(&self, issues: &[Issue], at: usize) -> bool {
        // **묶음인가를 여기서 다시 정하지 않는다.** `report::is_group` 이 그
        // 물음의 자리고, 손으로 벌여 적으면 종류가 하나 더 늘던 날 한쪽만
        // 고쳐져 탐색기와 상세가 다른 것을 묶음이라 부른다.
        //
        // **id 가 가리키는 줄만 폴더다.** 경로 마디는 id 라, 같은 id 의 묶음 줄
        // 둘이 다 폴더가 되면 같은 마디를 밀어 넣고 그 밑의 멤버가 두 폴더에
        // 한 번씩 그려진다(moai-sfml). 어느 줄이 폴더인지는 `by_id` 가 정한
        // 차례 — 뒷줄 — 를 따른다. `report::milestones` 가 중복 에픽의
        // 마일스톤을 뒷줄로 세는 것(moai-0prl)과, 이슈 중복에서 자식이 걸리는
        // 줄(`has_kids`)과 같은 자다. 가려진 앞줄은 잎으로 남아 **보인다** —
        // 깨진 자료는 숨기지 않고 `duplicate_id` 가 따로 드러낸다.
        self.find(&issues[at].id) == Some(at)
            && (crate::report::is_group(&issues[at]) || self.has_kids[at])
    }

    /// id 로 줄을 찾는다. 화면이 프레임마다 부르므로 훑지 않는다.
    pub fn find(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// 그 이슈가 걸리는 **단 하나의** 자리.
    pub fn home_of(&self, at: usize) -> &Path {
        &self.homes[at]
    }

    /// 그 자리를 home 으로 갖는 것들. **`home_of` 의 역상이다** — 목록 규칙을
    /// 따로 쓰지 않는 것이 빠짐도 겹침도 없음을 보장하는 유일한 이유다.
    pub fn entries(&self, issues: &[Issue], path: &Path) -> Vec<Entry> {
        self.entries_where(issues, path, &|_| true)
    }

    /// 거르고 나서의 목록.
    ///
    /// **거름망은 잎에 걸고, 디렉터리는 걸린 자손이 있으면 남긴다.** 디렉터리
    /// 자신에게만 걸면 끝난 에픽이 사라지면서 **그 밑의 걸린 이슈까지 통째로**
    /// 안 보인다 — 찾으려던 것을 거름망이 숨기는 셈이다. 반대로 자손만 보면
    /// `type=epic` 같은 물음에 에픽이 제 손으로 사라진다. 그래서 둘 중
    /// 하나라도 걸리면 남긴다.
    pub fn entries_where(
        &self,
        issues: &[Issue],
        path: &Path,
        keep: &dyn Fn(usize) -> bool,
    ) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        let mut buckets: Vec<Seg> = Vec::new();

        for (at, home) in self.homes.iter().enumerate() {
            // 바로 이 자리에 사는 것
            if home == path {
                if !self.kept(at, issues, keep) {
                    continue;
                }
                out.push(if self.is_dir(issues, at) {
                    Entry::Dir { seg: self.seg_of(issues, at), at: Some(at) }
                } else {
                    Entry::Leaf { at }
                });
                continue;
            }
            // 이 자리 **바로 밑의 바구니**에 사는 것 — 바구니는 제 줄이 없으므로
            // 사는 것이 있을 때만 생긴다.
            if home.starts_with(path)
                && let Some(seg) = home.get(path.len())
                && matches!(seg, Seg::Milestone(None) | Seg::Lost)
                && keep(at)
                && !buckets.contains(seg)
            {
                buckets.push(seg.clone());
            }
        }

        // 디렉터리 먼저, 그 안에서는 **목록과 같은 차례**.
        out.sort_by(|a, b| {
            let dir = |e: &Entry| u8::from(matches!(e, Entry::Leaf { .. }));
            dir(a).cmp(&dir(b)).then_with(|| {
                let at = |e: &Entry| e.at().expect("바구니는 여기 오지 않는다");
                crate::query::display_order(&issues[at(a)], &issues[at(b)])
            })
        });
        // 바구니는 늘 끝에. 정상인 것이 먼저 보여야 한다.
        buckets.sort_by_key(|s| matches!(s, Seg::Lost));
        out.extend(buckets.into_iter().map(|seg| Entry::Dir { seg, at: None }));
        out
    }

    /// 저 자신이 걸렸거나, 제 밑에 걸린 것이 있는가.
    fn kept(&self, at: usize, issues: &[Issue], keep: &dyn Fn(usize) -> bool) -> bool {
        if keep(at) {
            return true;
        }
        // **폴더만 밑을 본다.** 잎의 마디 밑에 사는 것은 없어야 하지만, 같은 id 의
        // 가려진 줄은 폴더인 쌍둥이와 마디가 같아 그 멤버를 제 자손으로 센다.
        if !self.is_dir(issues, at) {
            return false;
        }
        let mut under = self.homes[at].clone();
        under.push(self.seg_of(issues, at));
        // **훑다 말고 멈춘다.** `descendants` 로 받으면 첫 하나를 보기도 전에
        // 자손 전부를 담는 Vec 이 생기고, 그것이 거름망에 걸러진 줄마다 한 번씩
        // 프레임마다 반복된다.
        self.homes.iter().enumerate().any(|(d, h)| h.starts_with(&under) && keep(d))
    }

    /// 그 자리 **밑에 걸린 모든 것**. 바로 밑뿐 아니라 더 깊은 것까지.
    ///
    /// 디렉터리의 요약은 이것으로 센다. 규칙은 `report` 와 하나다 — 한때
    /// 자리를 정하는 쪽은 에픽을, 세는 쪽은 제 마일스톤을 이기게 해 머리글이
    /// 말하는 수와 눈에 보이는 줄 수가 어긋났다(moai-lhbh). 지금은
    /// `report::milestones` 도 에픽을 이기게 하므로 둘이 같은 집합을 센다.
    pub fn descendants(&self, path: &Path) -> Vec<usize> {
        // **직속 자식의 home 은 그 경로와 같다.** `len() >` 로 거르면 바로 밑의
        // 것이 통째로 빠져, 멤버가 셋인 에픽이 "자식 없음" 이라고 나온다.
        self.homes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.starts_with(path))
            .map(|(at, _)| at)
            .collect()
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

/// `Index::of` 안에서만 쓰는 계산판. 빌린 지도를 들고 다니므로 밖으로 나가지 않는다.
struct Ctx<'a> {
    issues: &'a [Issue],
    epic_of: &'a BTreeMap<String, String>,
    milestone_of: &'a BTreeMap<String, String>,
    by_id: &'a BTreeMap<&'a str, usize>,
    misplaced: &'a BTreeMap<String, crate::report::Misplace>,
    has_milestones: bool,
}

impl Ctx<'_> {

    /// 그 이슈가 걸리는 단 하나의 자리.
    fn home(&self, at: usize) -> Path {
        let me = &self.issues[at];
        // 자리를 못 정하는 줄은 여기서 갈라져 나간다. **한 자로 잰다.**
        if self.lost(&me.id) {
            return vec![Seg::Lost];
        }
        match me.kind {
            // 마일스톤은 뿌리에 선다.
            Kind::Milestone => Vec::new(),
            // 에픽은 제 마일스톤 밑에. 마일스톤을 안 쓰는 저장소면 뿌리에.
            Kind::Epic => self.under_milestone(&me.id),
            // idea 는 대개 에픽 없이 산다 — 에픽 없는 일과 같은 자리다.
            Kind::Issue | Kind::Idea => self.home_of_work(at),
        }
    }

    /// 그 줄의 참조가 못 쓸 것인가. **`report` 가 정한 그대로 묻는다** —
    /// 여기서 다시 판정하면 `moai status` 가 드러내는 집합과 `(길 잃음)`
    /// 바구니가 갈라진다.
    fn lost(&self, id: &str) -> bool {
        self.misplaced.contains_key(id)
    }

    fn under_milestone(&self, id: &str) -> Path {
        if !self.has_milestones {
            return Vec::new();
        }
        match self.milestone_of.get(id) {
            Some(m) => vec![Seg::Milestone(Some(m.clone()))],
            None => vec![Seg::Milestone(None)],
        }
    }

    fn home_of_work(&self, at: usize) -> Path {
        let me = &self.issues[at];
        // 부모가 실재하고 **같은 에픽**이면 부모 밑에 접힌다. 에픽이 다르면
        // 제 에픽으로 간다 — 롤업이 세는 곳과 화면이 그리는 곳을 맞추기 위해서다.
        // 부모로 설 수 있는 것은 **잎으로 서는 것** 전부다. idea 도 `seg_of`
        // 와 `home` 에서 일과 같은 자리를 받으므로 여기서만 빼면, idea 밑에
        // 만든 자식이 부모를 잃고 뿌리로 떠오른다 — 그러면 `has_kids` 도
        // 안 서서 그 idea 는 열리지도 않는다.
        //
        // 생각인 부모는 **그려진 자리의 에픽**으로 견준다. 뿌리로 올라간 생각은
        // 에픽 없이 사는 자리고, 이슈 밑에 접힌 생각은 그 이슈의 에픽 안에 산다 —
        // `report::groups` 가 소속을 끊는 곳도 앞의 것뿐이다. 그래서 에픽을 안
        // 적은 자식은 언제나 그 밑에 접히고, 제 에픽을 적은 자식은 그것이 그려진
        // 자리와 다를 때만 제 에픽으로 간다.
        if let Some(p) = crate::id::parent_of(&me.id)
            && let Some(&pat) = self.by_id.get(p)
            && matches!(self.issues[pat].kind, Kind::Issue | Kind::Idea)
        {
            // **부모가 사는 자리를 그대로 쓴다.** `home_of_work` 로 곧장
            // 내려가면 부모가 제 참조 때문에 `(길 잃음)` 으로 갈라진 것을
            // 못 보고 옛 자리를 셈해, 자식만 닿는 길 없는 자리에 남는다 —
            // 에픽에서와 같은 실패다.
            let mut path = self.home(pat);
            let rooted_thought =
                crate::report::is_idea(&self.issues[pat]) && !matches!(path.last(), Some(Seg::Issue(_)));
            let passed = if rooted_thought { None } else { self.epic_of.get(p) };
            if self.epic_of.get(&me.id) == passed {
                path.push(Seg::Issue(p.to_string()));
                return path;
            }
        }
        // **생각은 계획 계층에 걸리지 않는다.** 걸면 에픽 상세가 `멤버 0/0` 을
        // 낸 바로 밑에 그 줄을 그리고, 트리와 TUI 는 `자식 없음` 이라 말하면서
        // 줄은 낸다 — 자리를 정하는 자와 세는 자가 갈라진 꼴이다(moai-lhbh).
        //
        // **에픽만 떼면 반만 뗀 것이다.** 마일스톤은 `report::milestones` 가
        // 에픽을 타고 물려주므로, 에픽에서만 빼면 그 줄이 에픽의 마일스톤
        // 바구니로 떨어져 `moai show <마일스톤>` 이 똑같이 머리글과 어긋난다.
        //
        // **세는 쪽은 못 바꾼다.** 진행률이 생각을 세기 시작하면 담을수록 그
        // 묶음이 덜 끝난 것으로 보인다. 그래서 자리를 맞춘다. 소속은 필드로
        // 그대로 남아 `moai show --type idea -e <에픽>` 이 찾아낸다.
        if crate::report::is_idea(me) {
            return if self.has_milestones { vec![Seg::Milestone(None)] } else { Vec::new() };
        }
        match self.epic_of.get(&me.id) {
            Some(e) => {
                // **에픽이 사는 자리 밑으로 간다.** 여기서 마일스톤을 다시
                // 셈하면, 에픽이 제 참조 때문에 `(길 잃음)` 으로 갈라진 날
                // 멤버만 옛 자리에 남는다 — 그 자리는 뿌리에서 닿는 길이 없어
                // (바구니는 `Milestone(None)` 과 `Lost` 뿐이다) 그 줄들이
                // 트리에서도 탐색기에서도 통째로 사라진다. 실제로 그랬다.
                let mut path =
                    if self.lost(e) { vec![Seg::Lost] } else { self.under_milestone(e) };
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

    /// **자리를 잃은 에픽의 멤버도 어딘가에 있다.**
    ///
    /// 에픽이 제 마일스톤 때문에 `(길 잃음)` 으로 갈라질 때 멤버가 옛 자리에
    /// 남으면, 그 자리는 뿌리에서 닿는 길이 없어(바구니는 `Milestone(None)` 과
    /// `Lost` 뿐이다) 멤버가 트리에서도 탐색기에서도 통째로 사라진다.
    /// 위의 더미가 이 짝(길 잃은 **디렉터리** + 그 밑의 줄)을 안 담고 있어
    /// 실제로 한 번 빠져나갔다.
    #[test]
    fn the_members_of_a_lost_epic_are_still_reachable() {
        let mut lost_epic = make("argos-0002", Kind::Epic);
        lost_epic.milestone = Some("argos-zzzz".into()); // 없는 마일스톤
        let issues = vec![
            make("argos-0001", Kind::Milestone), // 마일스톤 층이 서도록
            lost_epic,
            epic_of("argos-0004", "argos-0002"),
            epic_of("argos-0005", "argos-0002"),
            make("argos-0005.aa1", Kind::Issue), // 그 자식까지
        ];
        assert_exactly_once(&issues);
    }

    /// **자리를 잃은 부모의 자식도 어딘가에 있다.** 위와 같은 실패가 물림
    /// 쪽에도 있다 — 부모가 제 마일스톤 때문에 갈라졌는데 자식은 제 참조가
    /// 멀쩡하면, 자식만 부모의 옛 자리에 남아 닿는 길이 없어진다.
    #[test]
    fn the_children_of_a_lost_parent_are_still_reachable() {
        let mut lost_parent = make("argos-0010", Kind::Issue);
        lost_parent.milestone = Some("argos-zzzz".into()); // 없는 마일스톤
        let issues = vec![
            make("argos-0001", Kind::Milestone),
            lost_parent,
            make("argos-0010.aa1", Kind::Issue), // 제 참조는 멀쩡한 자식
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

    /// **이슈 밑에 접힌 생각의 자식은 그 생각 밑에 접힌다** — 에픽을 안 적었든,
    /// 그 이슈와 같은 에픽을 적었든. 뿌리로 올라간 생각만 에픽 없는 자리로
    /// 견주므로, 제 에픽을 적은 자식은 그 생각의 에픽이 아니라 제 에픽으로 간다.
    #[test]
    fn a_child_of_a_folded_thought_stays_in_its_epic() {
        let mut thought_in_epic = make("argos-0005", Kind::Idea);
        thought_in_epic.epic = Some("argos-0002".into());
        let issues = vec![
            make("argos-0002", Kind::Epic),
            epic_of("argos-0004", "argos-0002"),
            make("argos-0004.aa1", Kind::Idea),                // 이슈 밑에 접힌 생각
            make("argos-0004.aa1.bb2", Kind::Issue),           // 에픽을 안 적은 자식
            epic_of("argos-0004.aa1.cc3", "argos-0002"),       // 같은 에픽을 적은 자식
            thought_in_epic,                                   // 뿌리로 올라간 생각
            epic_of("argos-0005.dd4", "argos-0002"),           // 제 에픽을 적은 자식
        ];
        let index = Index::of(&issues);
        let under = vec![
            Seg::Epic("argos-0002".into()),
            Seg::Issue("argos-0004".into()),
            Seg::Issue("argos-0004.aa1".into()),
        ];
        assert_eq!(index.home_of(3), &under);
        assert_eq!(index.home_of(4), &under);
        assert_eq!(index.home_of(6), &vec![Seg::Epic("argos-0002".into())]);
        assert_exactly_once(&issues);
    }

    /// **마일스톤이 세는 멤버는 탐색기가 그 밑에 그리는 줄과 같다.** 제 마일스톤을
    /// 따로 적은 자식이 부모 밑에 그려지면서 제 마일스톤으로 세어져,
    /// `show <마일스톤>` 이 `멤버 0/1` 이라 말하고 줄은 못 냈다(moai-uqoe).
    /// 접힌 모양마다 한 줄씩 — 이슈 밑, 접힌 생각 밑, 뿌리로 올라간 생각 밑.
    /// 에픽 줄이 딴 데를 가리키는 모양도 — 끊긴 `epic`, 에픽 안 에픽, 이슈 밑 id.
    /// 멤버가 에픽 줄의 필드를 다시 읽어 그 줄이 선 곳과 다르게 세어졌다(moai-0prl).
    #[test]
    fn a_milestone_counts_what_it_draws() {
        let own = |id: &str, kind: Kind, stone: &str| {
            let mut i = make(id, kind);
            i.milestone = Some(stone.into());
            i
        };
        let mut rooted_thought = own("argos-0006", Kind::Idea, "argos-0001");
        rooted_thought.epic = Some("argos-0003".into()); // 에픽이 v0.1 이 아니다 → 뿌리로
        let issues = vec![
            make("argos-0001", Kind::Milestone),
            make("argos-0002", Kind::Milestone),
            own("argos-0003", Kind::Epic, "argos-0002"),
            own("argos-0004", Kind::Issue, "argos-0001"),
            own("argos-0004.aa1", Kind::Issue, "argos-0002"), // 부모 밑에 접힌다
            own("argos-0004.aa1.bb2", Kind::Issue, "argos-0002"), // 손자도
            own("argos-0004.cc3", Kind::Idea, "argos-0002"), // 이슈 밑에 접힌 생각
            own("argos-0004.cc3.dd4", Kind::Issue, "argos-0002"), // 그 밑의 일
            rooted_thought,
            own("argos-0006.ee5", Kind::Issue, "argos-0001"), // 뿌리로 올라간 생각 밑
            pointing(own("argos-0007", Kind::Epic, "argos-0001"), "argos-zzzz"), // 끊긴 epic
            epic_of("argos-0008", "argos-0007"),
            pointing(own("argos-0009", Kind::Epic, "argos-0001"), "argos-0003"), // v0.2 에픽 안
            epic_of("argos-0010", "argos-0009"),
            own("argos-0004.ff6", Kind::Epic, "argos-0002"), // v0.1 이슈 밑에 id 로
            epic_of("argos-0011", "argos-0004.ff6"),
        ];
        assert_counts_what_it_draws(&issues);
        let index = Index::of(&issues);
        assert_eq!(index.home_of(9), &vec![Seg::Milestone(None), Seg::Issue("argos-0006".into())]);
        for (at, stone) in [(10, "argos-0001"), (11, "argos-0001"), (12, "argos-0001"), (13, "argos-0001"), (14, "argos-0002"), (15, "argos-0002")] {
            assert_eq!(index.home_of(at).first(), Some(&Seg::Milestone(Some(stone.into()))), "{}", issues[at].id);
        }
        assert_exactly_once(&issues);
    }

    fn pointing(mut i: Issue, epic: &str) -> Issue {
        i.epic = Some(epic.into());
        i
    }

    /// 마일스톤마다 `group_members` 가 세는 줄과 탐색기가 그 밑에 그리는 줄을 견준다.
    fn assert_counts_what_it_draws(issues: &[Issue]) {
        let index = Index::of(issues);
        for stone in issues.iter().filter(|i| i.kind == Kind::Milestone) {
            let mut drawn: Vec<&str> = index
                .descendants(&vec![Seg::Milestone(Some(stone.id.clone()))])
                .into_iter()
                .map(|at| &issues[at])
                .filter(|i| matches!(i.kind, Kind::Issue | Kind::Epic))
                .map(|i| i.id.as_str())
                .collect();
            drawn.sort();
            let mut counted: Vec<&str> =
                crate::report::group_members(issues, stone).iter().map(|i| i.id.as_str()).collect();
            counted.sort();
            assert_eq!(counted, drawn, "{} 가 그리는 것과 세는 것이 갈린다 — {issues:#?}", stone.id);
        }
        // 에픽도 같다. **폴더인 줄만** 본다 — 같은 id 의 가려진 줄은 잎이라 밑이 없다
        // (moai-sfml). 에픽 밑에는 일만 그려지고 세어진다. id 부모가 에픽인 줄이
        // 그 에픽에 드는 것(moai-9t3l)도 여기서 그리는 자와 세는 자가 한 번 더 만난다.
        for (at, epic) in issues.iter().enumerate().filter(|(_, i)| i.kind == Kind::Epic) {
            if index.find(&epic.id) != Some(at) {
                continue;
            }
            let mut path = index.home_of(at).clone();
            path.push(Seg::Epic(epic.id.clone()));
            let mut drawn: Vec<&str> = index
                .descendants(&path)
                .into_iter()
                .map(|d| &issues[d])
                .filter(|i| i.kind == Kind::Issue)
                .map(|i| i.id.as_str())
                .collect();
            drawn.sort();
            let mut counted: Vec<&str> =
                crate::report::group_members(issues, epic).iter().map(|i| i.id.as_str()).collect();
            counted.sort();
            assert_eq!(counted, drawn, "{} 가 그리는 것과 세는 것이 갈린다 — {issues:#?}", epic.id);
        }
    }

    /// **무작위 더미로 같은 대조를 돌린다.** 표로 적은 모양은 누가 떠올린 것뿐이다 —
    /// moai-uqoe·moai-0prl 은 둘 다 리뷰가 무작위 대조로 찾았다. 종류·부모·에픽·
    /// 마일스톤 참조를 멀쩡한 것, 종류 틀린 것, 없는 것으로 섞는다. 시계도 난수
    /// 크레이트도 없이 씨앗을 고정해 늘 같은 더미를 만든다.
    #[test]
    fn a_milestone_counts_what_it_draws_on_random_piles() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut roll = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..3000 {
            let mut issues: Vec<Issue> = Vec::new();
            for n in 0..(2 + roll(12)) {
                let mut kind = [Kind::Milestone, Kind::Epic, Kind::Epic, Kind::Issue, Kind::Issue, Kind::Idea][roll(6)];
                // 중복 id 도 섞는다 — 머지를 잘못 푼 파일은 이 도구가 거부하지 않고
                // 드러내기만 하는 실재 상태(`duplicate_id`)고, 같은 id 의 에픽 줄
                // 둘이 멤버를 두 번 그린 것(moai-sfml)은 이 더미가 그런 줄을 안
                // 만들어 못 잡았다. **종류는 앞줄의 것을 따른다** — 종류까지 다른
                // 중복은 `report` 의 id 지도들이 한 줄의 판정을 다른 줄에 흘리는
                // 따로 된 문제라 여기서 섞으면 이 대조가 그것을 대신 앓는다(moai-2m9p).
                let id = match roll(4) {
                    0 if !issues.is_empty() => format!("{}.a{n:02}", issues[roll(issues.len())].id),
                    1 if !issues.is_empty() => {
                        let twin = &issues[roll(issues.len())];
                        kind = twin.kind;
                        twin.id.clone()
                    }
                    _ => format!("argos-{n:04}"),
                };
                let mut i = make(&id, kind);
                let pick = |roll: &mut dyn FnMut(usize) -> usize| match roll(4) {
                    0 | 1 => None,
                    2 if !issues.is_empty() => Some(issues[roll(issues.len())].id.clone()),
                    _ => Some("argos-zzzz".to_string()),
                };
                i.epic = pick(&mut roll);
                i.milestone = pick(&mut roll);
                issues.push(i);
            }
            assert_counts_what_it_draws(&issues);
            assert_exactly_once(&issues);
        }
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

    /// 밑에 걸린 것을 셀 때 **바로 밑의 것**도 세어야 한다. 직속 자식의 home 은
    /// 그 디렉터리 경로와 길이가 같아서, 깊이로 거르면 통째로 빠진다.
    #[test]
    fn descendants_count_the_direct_children_too() {
        let issues = vec![
            make("argos-0001", Kind::Epic),
            epic_of("argos-0004", "argos-0001"),
            epic_of("argos-0005", "argos-0001"),
            make("argos-0005.aa1", Kind::Issue), // 손자
            make("argos-0009", Kind::Issue),     // 남
        ];
        let index = Index::of(&issues);
        let mut got = index.descendants(&vec![Seg::Epic("argos-0001".into())]);
        got.sort();
        assert_eq!(got, [1, 2, 3], "바로 밑 또는 더 깊은 것이 빠졌다");
        assert_eq!(index.descendants(&Vec::new()).len(), issues.len(), "뿌리는 전부다");
    }

    /// **거름망이 찾으려던 것을 숨기지 않는다.** 끝난 에픽 자신은 안 걸려도
    /// 그 밑에 걸린 이슈가 있으면 에픽 줄이 남아야 한다 — 안 그러면 에픽이
    /// 사라지면서 걸린 멤버까지 통째로 안 보인다.
    #[test]
    fn a_directory_survives_if_anything_under_it_matches() {
        let mut done_epic = make("argos-0001", Kind::Epic);
        done_epic.status = Status::new("done");
        let issues = vec![
            done_epic,
            epic_of("argos-0004", "argos-0001"), // todo 인 멤버
            make("argos-0009", Kind::Issue),     // 딸린 데 없는 todo
        ];
        let index = Index::of(&issues);
        // "todo 인 것만" — 에픽 자신은 안 걸린다
        let todo = |at: usize| issues[at].status.as_str() == "todo";
        let root = index.entries_where(&issues, &Vec::new(), &todo);
        assert!(
            root.iter().any(|e| e.at() == Some(0)),
            "끝난 에픽이 사라지면서 걸린 멤버까지 숨겼다 — {root:?}"
        );
        // 그 안에는 걸린 멤버만 남는다
        let inside = index.entries_where(&issues, &vec![Seg::Epic("argos-0001".into())], &todo);
        assert_eq!(inside.len(), 1);
    }

    /// 반대쪽도 막는다 — 자손만 보면 `type=epic` 같은 물음에 에픽이 제 손으로
    /// 사라진다. 저 자신이 걸리면 밑이 비어도 남는다.
    #[test]
    fn a_directory_that_matches_itself_survives_an_empty_subtree() {
        let issues = vec![make("argos-0001", Kind::Epic), make("argos-0009", Kind::Issue)];
        let index = Index::of(&issues);
        let epics = |at: usize| issues[at].kind == Kind::Epic;
        let root = index.entries_where(&issues, &Vec::new(), &epics);
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].at(), Some(0));
    }

    /// 아무것도 안 걸리면 빈 목록이다 (빈 바구니가 남지 않는다).
    #[test]
    fn nothing_matching_leaves_nothing_behind() {
        let issues = vec![make("argos-0001", Kind::Epic), epic_of("argos-0008", "argos-zzzz")];
        let index = Index::of(&issues);
        assert!(index.entries_where(&issues, &Vec::new(), &|_| false).is_empty());
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

    /// 중복 id 중 **자식을 실제로 가진 쪽만** 디렉터리다. id 로 세면 둘 다
    /// 디렉터리가 되고, 둘 다 같은 마디(`Seg::Issue(id)`)를 밀어 넣으므로
    /// 그 자식이 두 곳에 나타난다.
    #[test]
    fn a_duplicate_id_does_not_clone_its_child() {
        let issues = vec![
            make("argos-0010", Kind::Issue),
            make("argos-0010", Kind::Issue),
            make("argos-0010.aa1", Kind::Issue),
        ];
        let index = Index::of(&issues);
        let dirs = index
            .entries(&issues, &Vec::new())
            .into_iter()
            .filter(|e| matches!(e, Entry::Dir { .. }))
            .count();
        assert_eq!(dirs, 1, "같은 id 의 줄 둘이 나란히 디렉터리가 됐다");
        assert_exactly_once(&issues);
    }

    /// **멤버 없는 에픽도 디렉터리다.** 비었다고 잎이 되면 `--path` 가 그
    /// 에픽 대신 부모를 열고, 훑는 쪽은 제자리를 돈다.
    #[test]
    fn an_empty_epic_is_still_a_directory() {
        let issues = vec![make("argos-0001", Kind::Epic), make("argos-0009", Kind::Issue)];
        let index = Index::of(&issues);
        assert!(index.is_dir(&issues, 0), "빈 에픽이 잎이 됐다");
        assert!(!index.is_dir(&issues, 1), "자식 없는 이슈가 디렉터리가 됐다");
        assert!(index.entries(&issues, &vec![Seg::Epic("argos-0001".into())]).is_empty());
    }

    /// **같은 id 의 에픽 줄 둘이 마일스톤을 달리 들어도 세는 줄과 그리는 줄이 같다.**
    /// 멤버는 뒷줄로 에픽을 찾는데 지도가 앞줄이 받은 값을 남기면, 탐색기는 그
    /// 값으로 멤버를 그리고 셈은 뒷줄의 빈 값으로 뺀다. 중복 id 는 머지를 잘못
    /// 풀면 실재한다(`duplicate_id`). 두 차례 모두 본다.
    #[test]
    fn a_duplicate_epic_id_counts_what_it_draws() {
        let own = |stone: Option<&str>| {
            let mut e = make("argos-0001", Kind::Epic);
            e.milestone = stone.map(Into::into);
            e
        };
        for (a, b) in [(Some("argos-m001"), None), (None, Some("argos-m001"))] {
            let issues =
                vec![make("argos-m001", Kind::Milestone), own(a), own(b), epic_of("argos-0002", "argos-0001")];
            assert_counts_what_it_draws(&issues);
            assert_exactly_once(&issues);
        }
    }

    /// **같은 id 의 묶음 줄 둘이 멤버를 두 번 그리지 않는다.** 묶음은 비어도
    /// 디렉터리라(`an_empty_epic_is_still_a_directory`) 둘 다 디렉터리가 되면
    /// 같은 마디(`Seg::Epic(id)`)를 밀어 넣고, 멤버가 두 폴더 밑에 한 번씩
    /// 나타난다(moai-sfml). 이슈 중복을 첨자로 막은 것과 같은 자리다.
    #[test]
    fn a_duplicate_group_id_does_not_clone_its_members() {
        let mut with_stone = make("argos-0001", Kind::Epic);
        with_stone.milestone = Some("argos-m001".into());
        let issues = vec![
            make("argos-m001", Kind::Milestone),
            with_stone,
            make("argos-0001", Kind::Epic),
            epic_of("argos-0002", "argos-0001"),
        ];
        assert_exactly_once(&issues);
        let index = Index::of(&issues);
        // 폴더는 **id 가 가리키는 줄**(뒷줄)이다 — `report::milestones` 와
        // `by_id` 가 뒷줄을 이기게 하는 것과 같은 차례. 앞줄은 잎으로 남아
        // 사라지지 않는다.
        assert_eq!(index.find("argos-0001"), Some(2));
        assert!(index.is_dir(&issues, 2));
        assert!(!index.is_dir(&issues, 1), "가려진 에픽 줄이 폴더가 됐다");

        let mut loose = make("argos-0003", Kind::Issue);
        loose.milestone = Some("argos-m001".into());
        let stones = vec![make("argos-m001", Kind::Milestone), make("argos-m001", Kind::Milestone), loose];
        assert_exactly_once(&stones);

        // **가려진 줄은 멤버 덕에 거름망을 지나지 않는다.** 그 줄은 폴더가 아니라
        // 멤버를 거느리지 않는다 — 멤버만 걸리는 물음에 그 잎이 따라 서면, 걸리지
        // 않은 줄이 걸린 것처럼 보인다.
        let pair = vec![make("argos-0001", Kind::Epic), make("argos-0001", Kind::Epic), epic_of("argos-0002", "argos-0001")];
        let index = Index::of(&pair);
        let member = |at: usize| at == 2;
        assert_eq!(
            index.entries_where(&pair, &Vec::new(), &member),
            vec![Entry::Dir { seg: Seg::Epic("argos-0001".into()), at: Some(1) }]
        );
    }
}
