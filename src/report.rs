//! 세는 일과 고르는 일. **순수 함수다** — `&[Issue]` 만 보고 아무것도 찍지 않는다.
//!
//! **여기가 TUI 와의 계약이다.** 나중에 `ratatui` 는 `view` 를 건너뛰고
//! 이 모듈과 `query` 를 직접 부른다. 무엇을 셀지 정하는 코드가 `cmd/` 나
//! `view.rs` 에 있으면 표면이 늘 때마다 같은 것을 다시 짜야 한다.

use crate::config::Config;
use crate::model::{Issue, Kind, days_since};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

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
    /// 그 묶음이 **서 있는 칸** ([`group_states`]). 채우는 곳은 `status` 하나고,
    /// 집계만 필요한 `rollup` 은 비워 둔다 — 칸은 미룬 멤버를 빼고 세므로 막대와
    /// 답이 다를 수 있다. `2/3` 인데 `done` 인 묶음은 남은 하나를 미뤄 접은 것이다.
    ///
    /// 기계 출력에서는 **줄이 쓰는 이름과 같은 이름**이다(`cmd::Row` 의
    /// `derived_status`) — 한 낱말이 두 이름으로 나가면 받는 쪽이 둘을 다 기억해야 한다.
    #[serde(rename = "derived_status", skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
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

/// 지금 계획에서 빼 둔 것인가. **끝난 줄은 미룬 것이 아니다.**
///
/// 미루기는 "지금 안 한다" 는 말이라 이미 끝난 일에는 걸 것이 없다. 여기서
/// 닫힌 것을 안 빼면 미뤘다 끝낸 줄이 보드의 `done` 칸과 흐름에서만 사라져
/// 롤업과 어긋난다 — 한 화면이 같은 두 이슈를 `done 1` 과 `2/2` 로 말한다.
///
/// **제 줄에 대한 미룸의 정의다.** 표면이 묻는 "계획에서 빠졌나" 는 이것이
/// 아니라 [`deferred_roots`] 다 — 부모·에픽·마일스톤에서 물려받은 것까지 친다.
/// 그래서 밖에 내지 않는다: 이것을 부르는 표면은 미룬 에픽의 멤버를 계획으로 읽는다.
fn is_put_off(i: &Issue) -> bool {
    i.is_deferred() && !closed_by_hand(i)
}

/// 적힌 칸이 닫힌 줄인가. **묶음은 적힌 칸으로 닫히지 않는다** — 묶음의 칸은
/// 멤버에서 읽는다([`group_states`]). 여기서 적힌 `done` 을 믿으면 손으로 `done`
/// 에 둔 에픽을 미뤄도 그 멤버가 미룸을 안 물려받아 `ready` 에 그대로 선다.
///
/// 읽은 칸으로 묻지 않는 까닭은 순환이다 — 읽은 칸이 미룸을 쓴다. 그래서 물려주는
/// 셈에서는 묶음의 칸을 아예 안 보고 미룸만 본다. 읽은 칸이 done 인 묶음은 셀 멤버가
/// 이미 끝났거나 따로 미룬 것이라, 그 묶음을 미룬 것으로 쳐도 멤버 쪽 답은 같다.
/// **묶음 제 줄**은 멤버를 다 센 뒤에 [`deferred_roots_in`] 이 따로 뺀다.
fn closed_by_hand(i: &Issue) -> bool {
    !is_group(i) && i.status.is_done()
}

/// 지금 계획에서 빠진 줄의 id — **제가 미뤘거나, 미룬 것 밑에 있는 것.**
///
/// 줄 하나만 보고는 못 정한다. 에픽이나 마일스톤이나 부모를 미뤘는데 그 밑의
/// 일이 `ready` 에 그 묶음 제목을 달고 그대로 서면, 미루기가 한 일은 머리글
/// 하나 지운 것뿐이다. 그래서 `groups`·`milestones` 처럼 **읽을 때 조상을
/// 탄다.** 스냅샷에 적지 않는다 — 에픽 하나를 미룰 때 멤버 줄까지 써야
/// 한다면 그것은 파생값이다.
///
/// 이 집합을 쓰는 자리는 "지금 할 수 있는 것" 을 묻는다 — 보드·`ready`·경고.
/// **롤업은 안 쓴다**: 다섯 중 둘을 미뤘다고 `3건짜리 에픽` 이 되면 미루는
/// 것이 계획을 고쳐 쓰는 일이 된다. **흐름도 안 쓴다**: 흐름은 "지난 이레에
/// 무엇이 있었나" 라, 오늘 미룬다고 그저께 만든 사실이 사라지면 미루기가
/// 쌓임 경고를 지우는 손잡이가 된다.
///
/// **끝난 줄은 물려받아도 들지 않는다.** [`is_put_off`] 와 같은 까닭이다.
pub fn put_off(all: &[Issue]) -> BTreeSet<&str> {
    // 미룬 줄이 하나도 없으면 물려받을 것도 없다 — 소속 지도를 세우지 않는다.
    // 훅이 도구 호출마다 `wip` 를 거쳐 이 길을 지난다.
    if !all.iter().any(is_put_off) {
        return BTreeSet::new();
    }
    deferred_roots_in(all, &groups(all), &milestones(all)).into_keys().collect()
}

/// 계획에서 빠진 줄 id → **실제로 `deferred_at` 을 든 줄** id.
///
/// 물려받은 줄에 `moai defer <그 줄> --undo` 를 시키면 "이미 그렇다" 로
/// 끝나고 아무것도 안 풀린다. 되돌리는 말을 대는 자리는 이것으로 미룬 곳을 댄다.
pub fn deferred_roots(all: &[Issue]) -> BTreeMap<&str, &str> {
    if !all.iter().any(is_put_off) {
        return BTreeMap::new();
    }
    deferred_roots_in(all, &groups(all), &milestones(all))
}

/// [`deferred_roots`] 와 같은 것. 소속 지도를 이미 가진 쪽(`query::Where`)이 두 번
/// 걷지 않게 받는다.
///
/// **읽은 칸이 done 인 묶음은 제 줄이 빠진다.** 끝난 줄은 미룬 것이 아니다
/// ([`is_put_off`]). 묶음이 끝났는지는 멤버를 다 센 뒤에야 알므로 물려주는 셈
/// (`closed_by_hand`)에서는 못 거르고 여기서 거른다 — 안 거르면 `moai status` 의
/// `미뤄 둔 것` 이 센 줄을 그 줄이 가리키는 `moai show --deferred` 가 done 으로
/// 숨기고, `moai mv` 는 끝난 묶음에 도로 집으라고 한다. **그 밑의 줄이 물려받은
/// 미룸은 그대로다** — 멤버 쪽 답은 이미 셌다.
pub fn deferred_roots_in<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, &'a str> {
    // **미룬 줄이 없으면 걷지 않는다.** 물려받을 것도 없고, 훅이 도구 호출마다
    // 이 길을 지나는데 미룬 줄 하나 없는 저장소가 보통이다.
    if !all.iter().any(is_put_off) {
        return BTreeMap::new();
    }
    let shelf = Shelf::new(all, epic_of, mile_of);
    let mut roots: BTreeMap<&str, &str> = all
        .iter()
        .filter(|i| !closed_by_hand(i))
        .filter_map(|i| shelf.nearest(i.id.as_str()).map(|r| (i.id.as_str(), r)))
        .collect();
    // 계획에서 빠진 묶음이 없으면 멤버를 셀 것도 없다.
    let shelved: Vec<&Issue> =
        roots.keys().filter_map(|id| shelf.by_id.get(id).copied()).filter(|g| is_group(g)).collect();
    if !shelved.is_empty() {
        let members = members_in(all, epic_of, mile_of);
        let done: Vec<&str> = shelved
            .into_iter()
            .filter(|g| reads_done(&counted(g, &members, Some(&shelf), &roots)))
            .map(|g| g.id.as_str())
            .collect();
        for id in done {
            roots.remove(id);
        }
    }
    roots
}

/// 줄 하나를 계획에서 빼는 미룸을 **가까운 것부터** 짚는 길 — 제 줄, 제가 든
/// 에픽·마일스톤, 그다음 부모. [`deferred_roots_in`] 은 첫째만 쓰고(그 줄을 뺀 곳),
/// [`counted`] 는 전부 본다(묶음 제 미룸 말고도 그 멤버를 빼는 까닭이 있나).
struct Shelf<'a, 'm> {
    by_id: BTreeMap<&'a str, &'a Issue>,
    rooted: BTreeSet<&'a str>,
    epic_of: &'m BTreeMap<&'a str, &'a str>,
    mile_of: &'m BTreeMap<&'a str, &'a str>,
}

impl<'a, 'm> Shelf<'a, 'm> {
    fn new(
        all: &'a [Issue],
        epic_of: &'m BTreeMap<&'a str, &'a str>,
        mile_of: &'m BTreeMap<&'a str, &'a str>,
    ) -> Shelf<'a, 'm> {
        let by_id: BTreeMap<&str, &Issue> = all.iter().map(|i| (i.id.as_str(), i)).collect();
        let rooted = rooted_thoughts(&by_id);
        Shelf { by_id, rooted, epic_of, mile_of }
    }

    fn own(&self, id: &'a str) -> Option<&'a str> {
        self.by_id.get(id).is_some_and(|x| is_put_off(x)).then_some(id)
    }

    /// 그 줄이 **든 묶음**의 미룸. **소속이 실제로 서는 곳만** 본다 — 에픽은 에픽에
    /// 안 들고(`nav` 는 에픽 밑에 에픽을 두지 않는다), 마일스톤은 뿌리에 서며,
    /// 종류가 틀린 참조는 `(길 잃음)` 이다. 그런 참조로 미룸을 받으면 저만 계획에서
    /// 빠지고 제 멤버는 남는다 — 에픽 줄이 든 엉뚱한 `epic` 이 실제로 그랬다.
    fn joined(&self, to: Option<&&'a str>, want: Kind) -> Option<&'a str> {
        to.copied().filter(|t| self.by_id.get(t).map(|x| x.kind) == Some(want)).and_then(|t| self.own(t))
    }

    /// 제 줄부터 부모를 타고 올라가며, 그 줄이나 그 줄의 에픽·마일스톤의 미룸을
    /// `visit` 에 넘긴다. `visit` 이 참을 내면 멈춘다. 부모가 다른 에픽에 있어도
    /// 부모가 빠지면 자식도 빠진다. **없는 부모는 넘지 않는다** — `groups`·
    /// `milestones` 도 거기서 멈추므로, 넘으면 그리지도 세지도 않는 조상의 미룸에
    /// 끌려 나간다.
    ///
    /// **뿌리로 올라간 생각인 조상은 제 미룸만 물려준다.** 소속이 그것을 지나
    /// 내려오지 않으므로(`groups`) 생각의 에픽이 미뤄졌다고 그 밑의 일을 빼면,
    /// 세지도 않는 에픽의 미룸을 받는 꼴이다. 그 위로는 더 오르지 않는다.
    /// 이슈 밑에 접힌 생각은 그대로 지나간다 — 그 밑의 일은 미룬 이슈 밑에 그려진다.
    fn walk(&self, id: &'a str, mut visit: impl FnMut(&'a str) -> bool) {
        let mut cur = Some(id);
        while let Some(at) = cur {
            if at != id && self.rooted.contains(at) {
                if let Some(r) = self.own(at) {
                    visit(r);
                }
                return;
            }
            let here = self.by_id.get(at).map(|x| x.kind);
            if let Some(r) = self.own(at)
                && visit(r)
            {
                return;
            }
            if matches!(here, Some(Kind::Issue | Kind::Idea))
                && let Some(r) = self.joined(self.epic_of.get(at), Kind::Epic)
                && visit(r)
            {
                return;
            }
            if matches!(here, Some(Kind::Issue | Kind::Idea | Kind::Epic))
                && let Some(r) = self.joined(self.mile_of.get(at), Kind::Milestone)
                && visit(r)
            {
                return;
            }
            cur = crate::id::parent_of(at).filter(|p| self.by_id.contains_key(p));
        }
    }

    /// 그 줄을 계획에서 뺀 가장 가까운 줄.
    fn nearest(&self, id: &'a str) -> Option<&'a str> {
        let mut found = None;
        self.walk(id, |r| {
            found = Some(r);
            true
        });
        found
    }

    /// 그 줄을 계획에서 빼는 줄 전부.
    fn every(&self, id: &'a str) -> Vec<&'a str> {
        let mut out = Vec::new();
        self.walk(id, |r| {
            out.push(r);
            false
        });
        out
    }
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

/// 지금 집고 있는 것. **첫 칸도 아니고 끝나지도 않은 일**이다.
///
/// 칸 이름을 박아 두지 않는다 — 칸은 config 가 정하므로 `in_progress` 를
/// 글자로 찾으면 칸 이름을 바꾼 저장소에서 이 판단이 조용히 빈다.
///
/// `ready` 의 아래쪽 줄과 훅이 접힌 뒤에 싣는 줄이 같은 집합이다. 두 벌로
/// 두면 한쪽만 고쳐지고, 그러면 화면이 같은 세션을 두 가지로 말한다.
pub fn wip<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    // **값싼 것을 먼저 거른다.** 훅이 도구 호출마다 여기를 지나므로, 집은 것이
    // 없으면 조상을 타는 셈(`put_off`)을 아예 안 돌린다.
    let held: Vec<&Issue> = issues
        .iter()
        .filter(|i| is_work(i) && !i.status.is_done() && i.status.as_str() != cfg.first_status())
        .collect();
    if held.is_empty() {
        return held;
    }
    let out = put_off(issues);
    held.into_iter().filter(|i| !out.contains(i.id.as_str())).collect()
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

/// 그 묶음(에픽·마일스톤)의 멤버. **사람 화면과 `--json` 이 같은 것을 부른다.**
///
/// 한때 `--json` 은 에픽에만, 그것도 제 `epic` 필드를 적은 줄만 냈고 사람
/// 화면은 마일스톤까지, 물려받은 자식까지 그렸다 — 에이전트와 사람이 같은
/// 에픽을 다르게 셌다(moai-qizs). 그래서 고르는 자를 여기 하나로 둔다.
///
/// - 소속은 **물려받은 것까지**다 (`groups`·`milestones`). 트리가 그리는 것과
///   같아야 한다
/// - **담아 둔 생각은 멤버가 아니다.** 머리글(`rollup` 은 `is_work` 로 센다)이
///   안 세는 줄을 멤버로 내면 받는 쪽이 계획에 없는 것을 계획으로 읽는다
/// - 묶음이 아닌 줄에는 멤버가 없다
/// - **트리가 그 밑에 둘 수 있는 종류만** 멤버다. 에픽 밑에는 이슈, 마일스톤
///   밑에는 이슈와 에픽. 소속 지도는 종류를 안 가려 에픽 줄이 든 `epic` 도
///   소속으로 치는데, `nav` 는 에픽을 제 마일스톤 밑에만 두므로 그것을 멤버로
///   내면 `--json` 만 화면에 없는 줄을 낸다
///
/// 차례는 목록과 같다.
pub fn group_members<'a>(all: &'a [Issue], group: &Issue) -> Vec<&'a Issue> {
    let holds = |k: Kind| match group.kind {
        Kind::Epic => k == Kind::Issue,
        Kind::Milestone => matches!(k, Kind::Issue | Kind::Epic),
        _ => false,
    };
    if !is_group(group) {
        return Vec::new();
    }
    let map = group_for(group.kind, all);
    let mut out: Vec<&Issue> = all
        .iter()
        .filter(|i| i.id != group.id && holds(i.kind))
        .filter(|i| map.get(i.id.as_str()) == Some(&group.id.as_str()))
        .collect();
    out.sort_by(|a, b| crate::query::display_order(a, b));
    out
}

/// 묶음(에픽·마일스톤) id → **멤버에서 읽은 칸.**
///
/// 묶음의 `status` 는 저장된 필드지만 뜻을 갖지 않는다. 멤버를 옮길 때 묶음
/// 줄을 같이 쓰면 파생값을 저장하는 것이고, 안 쓰면 손으로 둔 칸이 멤버의
/// 진행과 따로 논다 — 한 화면이 `· todo` 와 `멤버 1/2` 를 같이 냈다(moai-j3b3).
/// 그래서 칸도 롤업처럼 읽을 때 센다.
///
/// - 셀 멤버는 롤업과 같다 — 물려받은 소속까지, **일만**
/// - **미룬 멤버는 뺀다.** 남은 일이 미룬 것뿐이면 닫힌 것으로 읽는다 — 묶음을
///   접는 손잡이가 곧 남은 멤버의 `defer` 다. 롤업의 숫자는 빼지 않는다: 칸은
///   "지금 할 것이 남았나", 막대는 "계획 중 얼마나 했나" 를 말한다
/// - 단 **묶음 자신이 받은 미룸은 멤버를 빼는 까닭이 못 된다.** 에픽을 미루면
///   그 밑이 전부 물려받는데, 그것으로 빼면 절반 끝난 에픽이 미뤘다는 이유로
///   `done` 이 된다
/// - 셀 멤버가 없으면 첫 칸, 전부 끝났으면 `done`, 전부 첫 칸이면 첫 칸,
///   아니면 **시작한 칸** (`Config::started_status`). 멤버가 모두 `review` 여도
///   시작한 칸이다 — 묶음이 설 칸은 "안 했다·하는 중·끝났다" 셋이다
pub fn group_states<'a, 'c>(all: &'a [Issue], cfg: &'c Config) -> BTreeMap<&'a str, &'c str> {
    let (epic_of, mile_of) = (groups(all), milestones(all));
    let roots = deferred_roots_in(all, &epic_of, &mile_of);
    group_states_in(all, cfg, &epic_of, &mile_of, &roots)
}

/// [`group_states`] 를 **그 줄들 가운데 묶음이 있을 때만** 센다.
///
/// 하나를 펼치거나 쓰는 표면(`show <id>`·`add`·`edit`·`mv`)은 대개 일 하나를 보는데,
/// 그때 묶음 지도와 미룸 걷기는 통째로 헛일이다 — 10k 줄에서 `moai show <이슈>` 가
/// 그것만으로 갑절이 됐다.
pub fn group_states_of<'a, 'c>(
    all: &'a [Issue],
    cfg: &'c Config,
    ids: &[&str],
) -> BTreeMap<&'a str, &'c str> {
    if !all.iter().any(|i| is_group(i) && ids.contains(&i.id.as_str())) {
        return BTreeMap::new();
    }
    let mut states = group_states(all, cfg);
    states.retain(|id, _| ids.contains(id));
    states
}

/// 줄이 **서 있는** 칸 — 묶음이면 멤버에서 읽은 칸([`group_states`]), 아니면 제 칸.
///
/// 칸을 그리거나 세는 표면은 `i.status` 대신 이것을 묻는다. 묶음을 가르는
/// `if` 가 표면마다 있으면 하나는 반드시 빠진다.
///
/// **읽은 칸은 묶음만 받는다.** id 로만 짚으면 머지를 잘못 푼 파일에서 묶음과 id 가
/// 같은 일 줄이 그 묶음의 칸을 입는다 — 그리는 쪽은 `▸` 로 내는데 `ready` 는 그 줄의
/// 적힌 칸으로 골라, 한 화면이 같은 줄을 두 칸으로 말한다.
pub fn column<'x>(i: &'x Issue, states: &BTreeMap<&str, &'x str>) -> &'x str {
    stands(i, states).unwrap_or(i.status.as_str())
}

/// 묶음이 읽은 칸. 묶음이 아니거나 못 받았으면 없다.
fn stands<'x>(i: &Issue, states: &BTreeMap<&str, &'x str>) -> Option<&'x str> {
    is_group(i).then(|| states.get(i.id.as_str()).copied()).flatten()
}

/// [`group_states`] 와 같은 것. 소속 지도와 미룸을 이미 가진 쪽이 두 번 걷지 않게 받는다.
pub fn group_states_in<'a, 'c>(
    all: &'a [Issue],
    cfg: &'c Config,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
    roots: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, &'c str> {
    group_stands_in(all, cfg, epic_of, mile_of, roots)
        .into_iter()
        .map(|(id, s)| (id, s.column))
        .collect()
}

/// 묶음 하나를 읽은 것 — 서 있는 칸과, 그 칸의 셈이 마지막으로 움직인 때.
/// **저장하지 않는다** ([`group_states`] 가 까닭을 적었다).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stand<'a, 'c> {
    /// 서 있는 칸.
    pub column: &'c str,
    /// 셀 멤버 가운데 가장 늦게 칸을 옮긴 때. 셀 멤버가 없으면 묶음이 생긴 때다.
    /// **적힌 `status_since` 는 안 쓴다** — 아무 데서도 안 읽히는 칸의 시각이라,
    /// `--stale` 이 그것으로 재면 오늘 진행 중이 된 에픽을 "열흘째 멈춰 있다" 고 한다.
    pub since: &'a str,
}

/// [`group_states_in`] 과 같은 한 번의 셈에서 칸과 시각을 함께 낸다.
pub fn group_stands_in<'a, 'c>(
    all: &'a [Issue],
    cfg: &'c Config,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
    roots: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, Stand<'a, 'c>> {
    let members = members_in(all, epic_of, mile_of);
    // 미룬 줄이 없으면 뺄 멤버도 없다 — 조상을 타는 길을 아예 안 세운다.
    let shelf = (!roots.is_empty()).then(|| Shelf::new(all, epic_of, mile_of));
    all.iter()
        .filter(|g| is_group(g))
        .map(|g| {
            let counted = counted(g, &members, shelf.as_ref(), roots);
            let since = counted
                .iter()
                .map(|m| m.status_since.as_str())
                .max()
                .unwrap_or(g.created_at.as_str());
            (g.id.as_str(), Stand { column: column_of(&counted, cfg), since })
        })
        .collect()
}

/// (종류, 묶음 id) → 그 묶음의 일. **롤업과 같은 자다** — 물려받은 소속까지, 일만.
/// 목록을 한 번만 걷는다 — 묶음마다 걸으면 제곱이다. **종류로 가른다** — 에픽 지도가
/// 마일스톤 id 를 가리키는 틀린 참조를 마일스톤의 멤버로 세면 `rollup_of` 와 어긋난다.
fn members_in<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<(Kind, &'a str), Vec<&'a Issue>> {
    let mut members: BTreeMap<(Kind, &str), Vec<&Issue>> = BTreeMap::new();
    for i in all.iter().filter(|i| is_work(i)) {
        for (kind, map) in [(Kind::Epic, epic_of), (Kind::Milestone, mile_of)] {
            if let Some(g) = map.get(i.id.as_str()) {
                members.entry((kind, g)).or_default().push(i);
            }
        }
    }
    members
}

/// 묶음의 칸을 셀 멤버 — 미룬 멤버는 빼되, **묶음 제가 받은 미룸으로는 안 뺀다.**
///
/// 멤버를 빼는 미룸을 **전부** 본다(`Shelf::every`). 가까운 하나만 보면, 부모를 미뤄
/// 빠진 자식이 제 에픽을 미루는 순간 그 에픽의 미룸을 뿌리로 받아 도로 세어진다 —
/// 미루기 하나로 에픽의 칸이 `done` 에서 `in_progress` 로 되돌아갔다.
///
/// **끝난 멤버는 언제나 센다.** 끝난 일은 물려받아도 계획에서 안 빠지므로
/// (`closed_by_hand`) 애초에 `roots` 에 없다.
fn counted<'a>(
    g: &'a Issue,
    members: &BTreeMap<(Kind, &'a str), Vec<&'a Issue>>,
    shelf: Option<&Shelf<'a, '_>>,
    roots: &BTreeMap<&'a str, &'a str>,
) -> Vec<&'a Issue> {
    let of = members.get(&(g.kind, g.id.as_str())).map(Vec::as_slice).unwrap_or_default();
    let Some(shelf) = shelf else { return of.to_vec() };
    // 묶음을 계획에서 뺀 것들 — 제 미룸과 마일스톤·부모에게서 물려받은 것.
    let mine = shelf.every(g.id.as_str());
    of.iter()
        .copied()
        .filter(|m| {
            !roots.contains_key(m.id.as_str())
                || shelf.every(m.id.as_str()).iter().all(|s| mine.contains(s))
        })
        .collect()
}

/// 셀 멤버가 있고 전부 끝났는가 — 묶음의 칸이 `done` 으로 읽히는 조건.
fn reads_done(counted: &[&Issue]) -> bool {
    !counted.is_empty() && counted.iter().all(|m| m.status.is_done())
}

fn column_of<'c>(counted: &[&Issue], cfg: &'c Config) -> &'c str {
    if counted.iter().all(|m| m.status.as_str() == cfg.first_status()) {
        cfg.first_status() // 셀 멤버가 없을 때도 여기다
    } else if reads_done(counted) {
        crate::config::DONE
    } else {
        cfg.started_status()
    }
}

/// 그 묶음에 **끝난 일**이 하나라도 있는가.
///
/// 남은 멤버를 미뤄 묶음이 `done` 으로 서려면 이것이 참이어야 한다 — 끝난 멤버가
/// 하나도 없으면 전부 미뤄도 셀 멤버가 비어 첫 칸이다. `moai mv <묶음> done` 이
/// 접는 길을 일러 줄 때 그 갈림길을 여기서 묻는다.
pub fn has_finished_member(all: &[Issue], group: &Issue) -> bool {
    group_members(all, group).iter().any(|m| is_work(m) && m.status.is_done())
}

/// 아직 안 끝난 막음이 하나라도 있는가. 없는 이슈를 가리키는 것은 막지
/// 않는다 — 끊긴 참조는 `moai status` 가 드러내지 `ready` 가 영원히 막지 않는다.
///
/// 끝났는지는 **서 있는 칸**으로 본다(`states`, [`group_states`]). 막는 것이
/// 에픽이면 적힌 칸은 안 읽힌다 — 믿으면 진행 중인 에픽을 손으로 done 에 둔
/// 순간 막힌 일이 `ready` 에 서고, 다 끝난 에픽은 적힌 칸을 옮기기 전까지 영영 막는다.
pub fn is_blocked(i: &Issue, by_id: &BTreeMap<&str, &Issue>, states: &BTreeMap<&str, &str>) -> bool {
    i.blocked_by.iter().any(|b| by_id.get(b.as_str()).is_some_and(|x| !stands_done(x, states)))
}

/// 서 있는 칸이 `done` 인가 — 묶음이면 읽은 칸, 아니면 제 칸.
fn stands_done(i: &Issue, states: &BTreeMap<&str, &str>) -> bool {
    column(i, states) == crate::config::DONE
}

/// `blocker` 가 `blocked` 를 막으면 고리가 생기는가. **쓰기 전에** 막는다 —
/// 사후 검사로 두면 이미 고리가 든 파일을 누가 만들고, 그때는 어느 줄을
/// 끊을지 사람이 정해야 한다.
///
/// **멤버도 제 묶음을 막는다.** 묶음은 멤버가 다 끝나야 `done` 으로 서므로
/// ([`group_states`]), 묶음이 제 멤버를 막으면 둘 다 영영 안 끝난다 — 적힌 칸을
/// `done` 으로 옮겨 풀던 길은 이제 없다(`is_blocked` 는 읽은 칸을 본다). 그래서 그
/// 물림도 고리로 센다: 담아 둔 생각을 막지 못하게 하는 `cmd/link.rs` 와 같은 까닭이다.
/// 미룬 멤버는 칸 셈에서 빠지지만 미룸은 되돌리는 것이라 그것에 기대지 않는다.
pub fn creates_cycle(issues: &[Issue], blocker: &str, blocked: &str) -> bool {
    // `blocked_by` 는 막히는 쪽에 적힌다. 앞으로(막는 쪽 → 막히는 쪽) 되짚으려면
    // 방향을 뒤집은 지도가 있어야 한다.
    let mut forward: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for i in issues {
        for b in &i.blocked_by {
            forward.entry(b.as_str()).or_default().push(i.id.as_str());
        }
    }
    let kind_of: BTreeMap<&str, Kind> = issues.iter().map(|i| (i.id.as_str(), i.kind)).collect();
    let (epic_of, mile_of) = (groups(issues), milestones(issues));
    for (&(kind, g), of) in &members_in(issues, &epic_of, &mile_of) {
        if kind_of.get(g) == Some(&kind) {
            for m in of {
                forward.entry(m.id.as_str()).or_default().push(g);
            }
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
    let rooted = rooted_thoughts(&by_id);
    all.iter().filter_map(|i| epic_through(i, &by_id, &rooted).map(|e| (i.id.as_str(), e))).collect()
}

/// **뿌리로 올라간 생각** — 제 부모 밑에 접히지 않는 idea 의 id.
///
/// 소속·마일스톤·미룸은 이것을 지나 내려오지 않는다. 생각인 부모에서 무조건
/// 끊으면 자기를 안 그리는 에픽에 세는 일은 사라지지만(moai-14dm), **이슈 밑에
/// 접힌 생각**에서도 끊겨 그 자식이 에픽 안에 그려지면서 `에픽 없음` 으로
/// 세어지고, 부모를 미뤄도 `ready` 에 남는다. 끊는 까닭은 생각이라서가 아니라
/// 그리는 자리가 끊겨서다.
///
/// 접히는 조건은 `nav::home_of_work` 와 같은 자다 — 부모가 이슈나 생각이고,
/// 제 에픽이 부모가 넘기는 에픽과 같다. 부모는 id 가 더 짧으므로 짧은 것부터
/// 정하면 한 번 훑어 끝난다.
fn rooted_thoughts<'a>(by_id: &BTreeMap<&'a str, &'a Issue>) -> BTreeSet<&'a str> {
    let mut thoughts: Vec<&Issue> = by_id.values().copied().filter(|i| is_idea(i)).collect();
    thoughts.sort_by_key(|t| t.id.len());
    let mut rooted = BTreeSet::new();
    for t in thoughts {
        let folds = crate::id::parent_of(&t.id)
            .and_then(|p| by_id.get(p).copied())
            .filter(|p| matches!(p.kind, Kind::Issue | Kind::Idea))
            .is_some_and(|p| epic_through(t, by_id, &rooted) == passed_down(p, by_id, &rooted));
        if !folds {
            rooted.insert(t.id.as_str());
        }
    }
    rooted
}

/// 그 줄의 에픽 — 제가 적었거나 조상에게서 물려받은 것. 뿌리로 올라간
/// 생각에서 멈춘다. 생각 제 줄의 소속은 그대로 남는다.
fn epic_through<'a>(
    i: &'a Issue,
    by_id: &BTreeMap<&'a str, &'a Issue>,
    rooted: &BTreeSet<&str>,
) -> Option<&'a str> {
    let mut cur = i;
    loop {
        if let Some(e) = &cur.epic {
            return Some(e.as_str());
        }
        cur = crate::id::parent_of(&cur.id)
            .and_then(|p| by_id.get(p).copied())
            .filter(|p| !rooted.contains(p.id.as_str()))?;
    }
}

/// 그 줄이 자식에게 넘기는 에픽. 뿌리로 올라간 생각은 아무것도 안 넘긴다.
fn passed_down<'a>(
    p: &'a Issue,
    by_id: &BTreeMap<&'a str, &'a Issue>,
    rooted: &BTreeSet<&str>,
) -> Option<&'a str> {
    if rooted.contains(p.id.as_str()) { None } else { epic_through(p, by_id, rooted) }
}

/// 이슈 id → 그것이 속한 마일스톤 id.
///
/// **에픽이 마일스톤을 이긴다.** 제 에픽이 있으면 그 에픽의 마일스톤이고,
/// 이슈가 `milestone` 을 따로 적었어도 그것은 지지 않는다. 에픽이 없으면
/// **부모도 이긴다** — 부모 밑에 접힌 줄은 접힌 맨 위 줄([`fold_top`])의
/// 마일스톤을 따른다. 이슈마다 마일스톤을 적게 하면 에픽을
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
    let rooted = rooted_thoughts(&by_id);
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
            // 에픽이 없으면 **접힌 맨 위 줄에서부터** 센다. 제 줄에서 시작하면
            // 부모 밑에 그려진 자식이 제 마일스톤으로 세어진다(moai-uqoe).
            None => match fold_top(i, &by_id, &rooted) {
                Some(top) => top,
                None => continue,
            },
        };
        // 에픽·부모를 타고 올라가며 처음 만나는 마일스톤. 고리가 있어도
        // 멈추도록 걸음 수를 제한한다.
        for _ in 0..64 {
            if let Some(m) = &cur.milestone {
                out.insert(i.id.as_str(), m.as_str());
                break;
            }
            // 부모가 뿌리로 올라간 생각이면 거기서 멈춘다 — `groups` 와 같은 자다.
            let up = cur.epic.as_deref().and_then(|e| by_id.get(e)).or_else(|| {
                crate::id::parent_of(&cur.id)
                    .and_then(|p| by_id.get(p))
                    .filter(|p| !rooted.contains(p.id.as_str()))
            });
            match up {
                Some(next) if next.id != cur.id => cur = next,
                _ => break,
            }
        }
    }
    out
}

/// 에픽 없는 줄이 **접혀 그려지는 맨 위 줄** — `nav::home_of_work` 가 그 줄을
/// 부모 밑에 접는 동안 올라간다. 접히지 않으면 그 줄 자신이다.
///
/// **부모가 제 마일스톤을 이긴다.** 에픽이 이기는 것(moai-lhbh)과 같은
/// 까닭이다 — 트리는 자식을 부모 밑에 그리므로, 자식이 제 `milestone` 을 따로
/// 적었다고 그것으로 세면 `show <그 마일스톤>` 이 `멤버 0/1` 이라 말하면서 줄을
/// 못 내고 `--json` 만 그 자식을 낸다(moai-uqoe). 자식의 필드는 그대로 남아
/// `broken` 이 그것이 못 쓸 것을 가리키는지 계속 본다.
///
/// 접히는 조건은 `nav` 와 같은 자다 — 부모가 이슈나 생각이고 제 에픽이 부모가
/// 넘기는 것과 같다. 에픽이 없는 줄이면 뒤쪽은 저절로 참이다: 부모가 에픽을
/// 넘겼다면 이 줄이 그것을 물려받았을 것이고, 안 넘기는 부모는 에픽이 없거나
/// 뿌리로 올라간 생각뿐이다. 그래서 여기서는 부모의 종류만 본다.
///
/// **뿌리로 올라간 생각 밑에 접히면 `None` 이다.** 그 생각은 `(마일스톤 없음)`
/// 에 서므로 그 밑의 줄도 거기 그려진다 — 생각이 에픽을 타고 세는 마일스톤을
/// 물려주면 moai-14dm 이 마일스톤에서 되살아난다.
fn fold_top<'a>(
    i: &'a Issue,
    by_id: &BTreeMap<&'a str, &'a Issue>,
    rooted: &BTreeSet<&str>,
) -> Option<&'a Issue> {
    let mut cur = i;
    // id 가 줄어들며 올라가므로 고리가 없다.
    while let Some(p) = crate::id::parent_of(&cur.id)
        .and_then(|p| by_id.get(p).copied())
        .filter(|p| matches!(p.kind, Kind::Issue | Kind::Idea))
    {
        if rooted.contains(p.id.as_str()) {
            return None;
        }
        cur = p;
    }
    Some(cur)
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
            Roll {
                id: Some(e.id.clone()),
                title: e.title.clone(),
                counts,
                total,
                done,
                percent,
                column: None,
            }
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
    out.push(Roll { id: None, title: none.into(), counts, total, done, percent, column: None });
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
    let (roots, states) = blocking(issues, cfg, &group, &by_id);
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    let mut out: Vec<&Issue> = issues
        .iter()
        // 값싼 막음 검사를 먼저 한다 — `unblocked_pick` 은 자식을 찾느라 목록을 걷는다.
        .filter(|i| !is_blocked(i, &by_id, &states) && unblocked_pick(i, issues, cfg, &out_of_plan))
        .collect();

    let progress: BTreeMap<Option<String>, u8> =
        rollup(issues, cfg).into_iter().map(|r| (r.id, r.percent.unwrap_or(0))).collect();

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

/// 막음을 재는 데 드는 것 — 계획에서 빠진 줄(뺀 곳과 함께)과, 막는 묶음의 읽은 칸.
///
/// **필요할 때만 센다.** 미룬 줄이 없으면 물려받을 것도 없고, 묶음에 막힌 줄이 하나도
/// 없으면 읽은 칸을 물을 자리가 없다 — 읽은 칸을 묻는 것은 `is_blocked` 뿐이고, 그
/// 밖의 줄은 제 칸으로 답한다(`column`). 둘 다 조상과 소속을 타는 셈이라, 안 묻는
/// 저장소에서 그냥 돌리면 `moai ready` 가 부를 때마다 목록을 여러 벌 더 걷는다.
fn blocking<'a, 'c>(
    issues: &'a [Issue],
    cfg: &'c Config,
    epic_of: &BTreeMap<&'a str, &'a str>,
    by_id: &BTreeMap<&'a str, &'a Issue>,
) -> (BTreeMap<&'a str, &'a str>, BTreeMap<&'a str, &'c str>) {
    let shelved = issues.iter().any(is_put_off);
    let by_group = issues.iter().any(|i| {
        i.blocked_by.iter().any(|b| by_id.get(b.as_str()).is_some_and(|x| is_group(x)))
    });
    if !shelved && !by_group {
        return (BTreeMap::new(), BTreeMap::new());
    }
    let mile_of = milestones(issues);
    let roots = deferred_roots_in(issues, epic_of, &mile_of);
    let states = if by_group {
        group_states_in(issues, cfg, epic_of, &mile_of, &roots)
    } else {
        BTreeMap::new()
    };
    (roots, states)
}

/// 막음만 빼면 집을 수 있는가. `ready` 와 `held` 가 **같은 자로** 고른다 —
/// 둘이 따로 고르면 `held` 가 댄 줄이 막음을 풀어도 `ready` 에 안 올라온다.
fn unblocked_pick(
    i: &Issue,
    issues: &[Issue],
    cfg: &Config,
    out_of_plan: &BTreeSet<&str>,
) -> bool {
    // **끝난 에픽인지는 묻지 않는다.** 묶음의 칸은 멤버에서 읽으므로(`group_states`)
    // 읽은 칸이 done 인 묶음에는 집을 멤버가 이미 없고, 적힌 칸으로 물으면 손으로
    // done 에 둔 에픽의 남은 일이 까닭 없이 `ready` 에서 사라진다(moai-j3b3).
    // 자식이 남아 있으면 부모는 직접 하는 일이 아니다. **일만 센다** —
    // 이슈 밑에 담아 둔 생각 하나가 그 이슈를 `ready` 에서 지워 버리는데,
    // idea 는 어느 목록에도 안 나오므로 왜 사라졌는지 볼 방법이 없다.
    let has_open_child = || {
        children_of(issues, &i.id)
            .iter()
            .any(|c| is_work(c) && !out_of_plan.contains(c.id.as_str()) && !c.status.is_done())
    };
    is_work(i)                                   // 묶음도 생각도 집는 게 아니다
        && !out_of_plan.contains(i.id.as_str())  // 미뤄 둔 것과 그 밑도
        && i.status.as_str() == cfg.first_status()
        && !has_open_child()
}

/// 미뤄 둔 것에 막혀 못 집는 일 하나와, 그것을 막는 미뤄 둔 줄들.
#[derive(Debug)]
pub struct Held<'a> {
    pub issue: &'a Issue,
    pub by: Vec<&'a str>,
    /// `by` 를 풀려면 도로 집어야 할 줄 — 막는 줄이 미룬 에픽 밑이면 그 에픽.
    pub undo: Vec<&'a str>,
}

/// 막음만 아니면 집을 일인데, **안 끝난 막음 중 하나라도 미뤄 둔 것**인 줄.
///
/// 미룬 막음을 무시하지 않는다 — 미룬 일은 끝난 일이 아니라, 무시하면 안
/// 끝난 일 위에 선 것을 집으라고 내민다. 그렇다고 입을 다물면 `ready` 가
/// 까닭 없이 비고, 막는 줄은 어느 목록에도 없어 풀 길이 안 보인다.
/// 그래서 **드러내되 고르지는 않는다.** 도로 집을지 막음을 풀지는 사람 몫이다.
pub fn held<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<Held<'a>> {
    // 미룬 줄이 없으면 미룬 막음도 없다 — 소속 지도를 안 세운다.
    if !issues.iter().any(is_put_off) {
        return Vec::new();
    }
    let group = groups(issues);
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    let (roots, states) = blocking(issues, cfg, &group, &by_id);
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    // 값싼 막음 검사를 먼저 한다. `unblocked_pick` 은 자식을 찾느라 목록을
    // 한 번 걷는다 — 모든 줄에 먼저 부르면 `ready` 가 부를 때마다 제곱이다.
    let mut out: Vec<Held> = issues
        .iter()
        .filter_map(|i| {
            let by = deferred_blockers(i, &by_id, &out_of_plan, &states);
            (!by.is_empty()).then_some((i, by))
        })
        .filter(|(i, _)| unblocked_pick(i, issues, cfg, &out_of_plan))
        .map(|(i, by)| {
            let mut undo: Vec<&str> = by.iter().filter_map(|b| roots.get(b).copied()).collect();
            undo.sort_unstable();
            undo.dedup();
            Held { issue: i, by, undo }
        })
        .collect();
    out.sort_by(|a, b| crate::query::display_order(a.issue, b.issue));
    out
}

/// `i` 를 막는 것 중 안 끝났고 계획에서 빠진 것.
fn deferred_blockers<'a>(
    i: &Issue,
    by_id: &BTreeMap<&str, &'a Issue>,
    out_of_plan: &BTreeSet<&str>,
    states: &BTreeMap<&str, &str>,
) -> Vec<&'a str> {
    i.blocked_by
        .iter()
        .filter_map(|b| by_id.get(b.as_str()))
        .filter(|b| !stands_done(b, states) && out_of_plan.contains(b.id.as_str()))
        .map(|b| b.id.as_str())
        .collect()
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
    /// 고칠 것. **알림은 여기 없다.**
    ///
    /// 한때 한 배열에 섞고 `notice` 깃발로만 갈라, 사람 화면과 탐색기는 알림을
    /// 빼고 셌는데 `--json` 을 읽는 쪽과 Stop 훅은 `warnings` 를 통째로 셌다 —
    /// `moai idea add`·`moai defer` 를 부를 때마다 "경고가 늘었다" 로 세션이
    /// 붙들렸다(moai-c8lb). 받는 쪽마다 깃발을 기억하게 하느니 자리를 가른다.
    pub warnings: Vec<Warning>,
    /// 알려 주는 것 — 쌓인 생각, 미뤄 둔 것. 고칠 것이 있다는 말이 아니다.
    pub notices: Vec<Warning>,
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

/// 읽다 만난 못 읽는 줄 하나 — **읽어 낼 수 있었다면 그 줄이 쓰는 id.**
///
/// 줄 번호만 받던 때는 못 읽는 줄이 산 줄의 id 를 들고 있어도 `duplicate_id` 가
/// 안 섰다. 그 줄이 읽히게 되는 날 모든 쓰기가 막히고, 그때는 어느 줄인지 사람이
/// 찾아야 한다(moai-4dk4). `store` 를 모르게 두려고 여기 제 모양으로 받는다.
/// 줄 번호는 싣지 않는다 — 여기서는 수만 세고, 줄 번호는 `moai show` 가 낸다.
#[derive(Debug, Clone, Copy)]
pub struct Unreadable<'a> {
    pub id: Option<&'a str>,
}

/// `unreadable` 은 읽다 만난 못 읽는 줄이다 — 저장소가 아니라 부르는 쪽이 준다.
pub fn status(issues: &[Issue], unreadable: &[Unreadable], cfg: &Config, now: &str) -> StatusReport {
    let group = groups(issues);
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // **여기가 "지금 계획" 의 정의다.** 보드 수·모든 경고·흐름이 이 하나를
    // 지나므로, 미뤄 둔 것을 여기서 빼면 아래 전부에서 저절로 빠진다.
    // 묶음의 읽은 칸도 같은 자리에서 한 번만 받는다 — 둘 다 조상과 소속을 타는
    // 셈이라 따로 부르면 `moai status` 한 번에 같은 걸음을 두 벌 걷는다.
    let mile_of = milestones(issues);
    let roots = deferred_roots_in(issues, &group, &mile_of);
    let states = group_states_in(issues, cfg, &group, &mile_of, &roots);
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    let work: Vec<&Issue> =
        issues.iter().filter(|i| is_work(i) && !out_of_plan.contains(i.id.as_str())).collect();
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
    // **묶음 줄에는 읽은 칸을 곁들인다.** 막대(`3/5`)는 계획 중 얼마나 했나이고 칸은
    // 지금 할 것이 남았나라, 남은 멤버를 미뤄 접은 묶음은 `1/2` 인 채로 닫혀 있다 —
    // 세션이 여기서 시작하는데 그것을 안 말하면 접은 묶음과 굴러가는 묶음이 같아 보인다.
    let stood = |r: Roll| {
        let column = r.id.as_deref().and_then(|id| states.get(id)).map(|c| c.to_string());
        Roll { column, ..r }
    };
    let epics: Vec<Roll> = rolls.iter().filter(|r| r.id.is_some()).cloned().map(stood).collect();
    // 마일스톤을 하나도 안 쓰는 저장소에는 줄도 경고도 내지 않는다.
    let stones: Vec<Roll> = rollup_of(Kind::Milestone, issues, cfg)
        .into_iter()
        .filter(|r| r.id.is_some())
        .map(stood)
        .collect();
    let mut warnings = Vec::new();
    let mut notices = Vec::new();

    // 1. 에픽에 안 붙은 것. 마일스톤이 아직 없으므로 **제일 중요한 신호**다
    //    — "물어보지 않고 만든 이슈" 의 지문이다.
    let loose: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| !i.status.is_done() && !group.contains_key(i.id.as_str()))
        .collect();
    // **분모는 미룬 일까지 센다.** 에픽을 통째로 미루면 그 멤버만 `work` 에서 빠져,
    // 원래 있던 소속 없는 일 하나가 "열린 것의 100%" 로 선다 — 미루기 하나로 경고가
    // 늘어 `Stop` 이 세션을 붙들었다(moai-c8lb 와 같은 덫). 분자는 그대로 지금 계획만
    // 센다: 미룬 소속 없는 일로는 꾸짖지 않는다. 그래서 미루기는 비율을 못 올린다.
    let open = issues.iter().filter(|i| is_work(i) && !i.status.is_done()).count();
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
    // 미뤄 둔 것에 막힌 것은 **아래 2-3 이 제 이름으로** 말한다. 여기서도
    // 세면 같은 줄이 두 번 나오고, 이쪽 말로는 막는 줄을 어디서 찾는지 모른다.
    let by_deferred =
        |i: &Issue| !deferred_blockers(i, &by_id, &out_of_plan, &states).is_empty();
    let stuck: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            !i.status.is_done()
                && is_blocked(i, &by_id, &states)
                && !by_deferred(i)
                && days_since(&i.status_since, now).is_some_and(|d| d > BLOCKED_STALE_DAYS)
        })
        .collect();
    if !stuck.is_empty() {
        warnings.push(Warning::new("blocked_stale", ids_of(&stuck)).days(BLOCKED_STALE_DAYS));
    }

    // 2-3. 미뤄 둔 것에 막힌 것. **날짜를 안 기다린다** — 계획이 스스로
    //      모순된 자리라(지금 할 일이 지금 안 할 일을 기다린다) 사흘 둔다고
    //      풀리지 않는다. 막지는 않는다.
    let waiting: Vec<&Issue> =
        work.iter().copied().filter(|i| !i.status.is_done() && by_deferred(i)).collect();
    if !waiting.is_empty() {
        warnings.push(
            Warning::new("blocked_by_deferred", ids_of(&waiting)).hint("moai show --deferred"),
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
    //
    // **미뤄 둔 묶음은 꾸짖지 않는다.** 롤업이 `is_work` 로 세는 것은 미뤘다고
    // 계획이 줄면 안 되기 때문이지 미룬 것을 고발하라는 뜻이 아니다 — 여기서
    // 세면 "다음 분기에" 하고 통째로 미룬 에픽이 그 순간 `속이 빈 에픽` 으로
    // 서고, 미룰수록 잔소리가 는다. `put_off` 가 일에 대해 하는 일을 묶음에
    // 대해서도 하는 자리다. **물려받은 미룸도 친다** — 미룬 마일스톤 밑의
    // 에픽은 `미뤄 둔 것` 으로 세면서 `속이 빈 에픽` 으로도 꾸짖으면 안 된다.
    // 제 미룸을 따로 묻지 않는다: 빈 묶음은 읽은 칸이 첫 칸이라 `deferred_roots`
    // 에서 빠지지 않으므로, 미뤘으면 언제나 `out_of_plan` 에 든다.
    let empty: Vec<String> = rolls
        .iter()
        .filter(|r| r.id.is_some() && r.total == 0)
        .filter_map(|r| r.id.clone())
        .filter(|id| !out_of_plan.contains(id.as_str()))
        .collect();
    if !empty.is_empty() {
        warnings.push(Warning::new("empty_epic", empty));
    }
    // "다 끝났는데 안 닫힌 에픽" 은 없다 — 묶음의 칸은 멤버에서 읽으므로 100% 면
    // 곧 닫힌 것이다. 그 경고가 서 있으면 적힌 칸을 옮기라고 시키는데, 그 칸은
    // 이제 어디서도 읽히지 않는다(moai-j3b3).

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
    let piled =
        |i: &&Issue| is_idea(i) && !i.status.is_done() && !out_of_plan.contains(i.id.as_str());
    let count = issues.iter().filter(piled).count();
    if count >= IDEA_PILE {
        let oldest = issues
            .iter()
            .filter(piled)
            .filter_map(|i| days_since(&i.created_at, now))
            .max()
            .unwrap_or(0);
        notices.push(
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
    //
    //      **물려받은 것도 센다.** 미룬 에픽 밑의 멤버도 보드에서 빠지므로
    //      여기서 안 세면 가리키는 `--deferred` 가 내는 수와 어긋난다. 나이는
    //      제 줄에 적힌 시각만 있으니 그것으로 잰다.
    let shelved = |i: &&Issue| out_of_plan.contains(i.id.as_str());
    let count = issues.iter().filter(shelved).count();
    if count > 0 {
        let oldest = issues
            .iter()
            .filter(shelved)
            .filter_map(|i| i.deferred_at.as_deref().and_then(|at| days_since(at, now)))
            .max()
            .unwrap_or(0);
        notices.push(
            Warning::new("deferred", Vec::new())
                .count(count)
                .oldest(oldest)
                .notice()
                .hint("moai show --deferred"),
        );
    }

    // 7. 데이터가 깨진 것. **이것만 비영 종료한다.**
    //
    // **못 읽는 줄이 쓰는 id 도 같이 견준다.** 안 견주면 그 중복은 못 읽는 동안
    // 아무 데도 안 보이다가, 새 바이너리가 그 줄을 읽는 날 모든 쓰기를 막는다.
    // 드러내기만 한다 — 쓰기를 막으면 "못 읽는 줄은 들고 간다" 가 무너진다.
    let mut seen = BTreeSet::new();
    let dups: Vec<String> = issues
        .iter()
        .map(|i| i.id.as_str())
        .chain(unreadable.iter().filter_map(|u| u.id))
        .filter(|id| !seen.insert(*id))
        .map(str::to_string)
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
    //
    // **여기만 `is_work` 로 센다.** 위의 `work` 는 "지금 계획" 이고 이 줄은
    // "지난 이레에 있었던 일" 이라 묻는 것이 다르다 — 미뤄 둔 것을 여기서
    // 빼면 오늘 셋을 미루는 것만으로 `생성 5 · 쌓이는 중 +5` 가
    // `생성 2 · +2` 가 되어, 미루기가 쌓임 경고를 지우는 손잡이가 된다.
    let within = |at: &str| days_since(at, now).is_some_and(|d| (0..FLOW_DAYS).contains(&d));
    let happened: Vec<&Issue> = issues.iter().filter(|i| is_work(i)).collect();
    let created = happened.iter().filter(|i| within(&i.created_at)).count();
    let closed =
        happened.iter().filter(|i| i.status.is_done() && within(&i.status_since)).count();

    StatusReport {
        counts,
        total: work.len(),
        milestones: stones,
        epics,
        warnings,
        notices,
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

    /// 모든 멤버가 끝난 에픽은 100% 다 — 그 에픽의 읽은 칸이 done 인 근거.
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
        let issues = vec![
            make("argos-00aa", Kind::Epic, "todo"),          // 에픽 자체
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

    /// **적힌 칸이 done 인 에픽의 남은 일도 집는다.** 묶음의 칸은 멤버에서 읽으므로
    /// 손으로 done 에 둔 에픽은 닫힌 것이 아니다 — 그 밑의 손자를 까닭 없이
    /// `ready` 에서 빼면 어디서도 안 읽는 칸이 계획을 지운다(moai-j3b3).
    #[test]
    fn ready_ignores_a_grouping_closed_by_hand() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "done"),
            member("argos-0002", "argos-0001", "done"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
        ];
        let got: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0002.aaa"]);
        assert_eq!(group_states(&issues, &cfg()).get("argos-0001"), Some(&"in_progress"));
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
    fn an_unfilled_plan_shows_and_finished_work_is_not_scolded() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),                 // 빈 에픽
            make("argos-0002", Kind::Epic, "in_progress"),          // 다 끝났고 적힌 칸은 안 닫힘
            member("argos-0003", "argos-0002", "done"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z");
        let k = kinds(&st);
        // 다 끝난 묶음은 읽은 칸이 곧 done 이라 꾸짖을 것이 없다(moai-j3b3).
        assert_eq!(k, ["empty_epic"], "{k:?}");
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
        assert!(status(&[], &[Unreadable { id: None }], &cfg(), "2026-09-01T00:00:00Z").broken());
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
        let mem: Vec<&str> = group_members(&issues, &issues[0]).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(mem, ["argos-0002", "argos-0002.aaa", "argos-0002.aaa.bbb"], "물려받은 자식을 뺐다");
    }

    /// **멤버는 트리가 그리는 것과 같다** — 물려받은 자식까지, 마일스톤이면
    /// 그 밑의 에픽과 그 멤버까지, 담아 둔 생각은 빼고(moai-qizs).
    #[test]
    fn group_members_match_what_the_tree_draws() {
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.title = "v0.1".into();
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let mut thought = make("argos-0004", Kind::Idea, "todo");
        thought.epic = Some("argos-0002".into());
        // 에픽 줄이 든 `epic` — `nav` 는 에픽을 에픽 밑에 두지 않는다.
        let mut stray = make("argos-0005", Kind::Epic, "todo");
        stray.epic = Some("argos-0002".into());
        let issues = vec![
            stone,
            epic,
            member("argos-0003", "argos-0002", "todo"),
            make("argos-0003.aaa", Kind::Issue, "todo"),
            thought,
            stray,
        ];
        let ids = |v: Vec<&Issue>| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();

        assert_eq!(
            ids(group_members(&issues, &issues[1])),
            ["argos-0003", "argos-0003.aaa"],
            "에픽이 에픽을 멤버로 냈다"
        );
        assert_eq!(
            ids(group_members(&issues, &issues[0])),
            ["argos-0002", "argos-0003", "argos-0003.aaa", "argos-0005"],
            "마일스톤이 밑의 에픽과 그 멤버를 안 낸다"
        );
        assert!(group_members(&issues, &issues[2]).is_empty(), "묶음 아닌 줄이 멤버를 냈다");
    }

    // ── 묶음의 칸은 멤버에서 읽는다 (moai-j3b3) ─────────────────────

    fn put_off_line(mut i: Issue) -> Issue {
        i.deferred_at = Some("2026-09-02T00:00:00Z".into());
        i
    }

    fn state_of(issues: &[Issue], id: &str) -> String {
        group_states(issues, &cfg()).get(id).expect("묶음이 칸을 못 받았다").to_string()
    }

    /// 멤버 칸의 조합마다 한 줄. **손으로 둔 칸은 답에 안 든다** — 에픽 줄을
    /// 일부러 엉뚱한 칸에 둔다.
    #[test]
    fn a_group_reads_its_column_from_its_members() {
        for (stored, members, want) in [
            ("done", &[][..], "todo"),
            ("in_progress", &["todo", "todo"][..], "todo"),
            ("done", &["todo", "in_progress"][..], "in_progress"),
            ("todo", &["todo", "done"][..], "in_progress"),
            ("todo", &["review", "review"][..], "in_progress"),
            ("todo", &["done", "done"][..], "done"),
        ] {
            let mut issues = vec![make("argos-0001", Kind::Epic, stored)];
            for (n, st) in members.iter().enumerate() {
                issues.push(member(&format!("argos-000{}", n + 2), "argos-0001", st));
            }
            assert_eq!(state_of(&issues, "argos-0001"), want, "칸 {stored}, 멤버 {members:?}");
        }
    }

    /// 남은 일이 미룬 것뿐이면 닫힌 것이다. 미룬 것밖에 없고 끝난 것도 없으면
    /// 시작도 안 한 것이다.
    #[test]
    fn a_deferred_member_does_not_hold_its_group_open() {
        let epic = || make("argos-0001", Kind::Epic, "todo");
        let rest = || put_off_line(member("argos-0003", "argos-0001", "in_progress"));
        let closed = vec![epic(), member("argos-0002", "argos-0001", "done"), rest()];
        assert_eq!(state_of(&closed, "argos-0001"), "done");
        let only_shelved = vec![epic(), rest()];
        assert_eq!(state_of(&only_shelved, "argos-0001"), "todo");
        // 롤업은 미룬 것도 센다 — 칸과 막대가 말하는 것이 다르다.
        assert_eq!(roll_of(&rollup(&closed, &cfg()), Some("argos-0001")).percent, Some(50));
    }

    /// **묶음 자신이 받은 미룸으로는 멤버를 안 뺀다.** 절반 끝난 에픽을 미뤘다고
    /// `done` 이 되면 미루기가 닫기가 된다. 마일스톤에서 물려받은 미룸도 같다.
    /// 그러나 **그 안에서 따로 미룬 것은 뺀다** — 마일스톤 밑에서 에픽 하나를
    /// 미룬 것은 그 마일스톤 쪽에서 보면 멤버를 미룬 것과 같다.
    #[test]
    fn a_shelved_group_still_reads_its_whole_membership() {
        let with = |mile_off: bool, epic_off: bool| {
            let mut mile = make("argos-0009", Kind::Milestone, "todo");
            let mut epic = make("argos-0001", Kind::Epic, "todo");
            epic.milestone = Some("argos-0009".into());
            if mile_off {
                mile = put_off_line(mile);
            }
            if epic_off {
                epic = put_off_line(epic);
            }
            vec![
                mile,
                epic,
                member("argos-0002", "argos-0001", "done"),
                member("argos-0003", "argos-0001", "todo"),
            ]
        };
        let shelved_epic = with(false, true);
        assert_eq!(state_of(&shelved_epic, "argos-0001"), "in_progress", "제 미룸으로 멤버를 뺐다");
        assert_eq!(state_of(&shelved_epic, "argos-0009"), "done", "안에서 미룬 에픽이 마일스톤을 붙들었다");
        let shelved_mile = with(true, false);
        assert_eq!(state_of(&shelved_mile, "argos-0001"), "in_progress", "물려받은 미룸으로 멤버를 뺐다");
        assert_eq!(state_of(&shelved_mile, "argos-0009"), "in_progress", "제 미룸으로 멤버를 뺐다");

        let mut own = with(false, false);
        own[3] = put_off_line(own[3].clone());
        assert_eq!(state_of(&own, "argos-0001"), "done");
    }

    /// **손으로 done 에 둔 묶음도 미루면 그 밑이 물려받는다.** 적힌 칸은 묶음을
    /// 닫지 않는다 — 믿으면 미룬 에픽의 남은 일이 `ready` 에 그대로 선다.
    #[test]
    fn a_grouping_closed_by_hand_still_hands_down_its_deferral() {
        let issues = vec![
            put_off_line(make("argos-0001", Kind::Epic, "done")),
            member("argos-0002", "argos-0001", "todo"),
            member("argos-0003", "argos-0001", "done"),
        ];
        let out = put_off(&issues);
        assert!(out.contains("argos-0002"), "남은 멤버가 미룸을 안 받았다 — {out:?}");
        assert!(out.contains("argos-0001") && !out.contains("argos-0003"), "{out:?}");
        assert!(ready(&issues, &cfg()).is_empty());
    }

    /// **막는 것이 묶음이면 서 있는 칸으로 끝났는지 본다.** 적힌 칸을 믿으면 진행
    /// 중인 에픽을 손으로 done 에 둔 순간 막힌 일이 풀리고, 다 끝난 에픽은 영영 막는다.
    #[test]
    fn a_blocking_grouping_blocks_by_the_column_it_stands_in() {
        let blocked = |by: &str| {
            let mut i = make("argos-0009", Kind::Issue, "todo");
            i.blocked_by = vec![by.into()];
            i
        };
        let issues = vec![
            make("argos-0001", Kind::Epic, "done"),          // 적힌 칸만 닫힘
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0003", Kind::Epic, "todo"),          // 멤버가 다 끝남
            member("argos-0004", "argos-0003", "done"),
        ];
        let pick = |by: &str| {
            let mut all = issues.clone();
            all.push(blocked(by));
            ready(&all, &cfg()).iter().any(|i| i.id == "argos-0009")
        };
        assert!(!pick("argos-0001"), "진행 중인 에픽이 적힌 칸 때문에 안 막았다");
        assert!(pick("argos-0003"), "다 끝난 에픽이 적힌 칸 때문에 막았다");
    }

    /// 마일스톤도 같은 자로 읽는다 — 에픽을 거쳐 온 이슈와 물려받은 자식까지.
    /// 생각은 일이 아니라 안 센다.
    #[test]
    fn a_milestone_reads_the_same_way() {
        let mut epic = make("argos-0001", Kind::Epic, "done");
        epic.milestone = Some("argos-0009".into());
        let mut idea = make("argos-0005", Kind::Idea, "in_progress");
        idea.milestone = Some("argos-0009".into());
        let issues = vec![
            make("argos-0009", Kind::Milestone, "done"),
            epic,
            member("argos-0002", "argos-0001", "done"),
            make("argos-0002.aaa", Kind::Issue, "todo"),
            idea,
        ];
        assert_eq!(state_of(&issues, "argos-0009"), "in_progress");
        assert_eq!(state_of(&issues, "argos-0001"), "in_progress", "물려받은 자식을 안 셌다");
        assert!(!group_states(&issues, &cfg()).contains_key("argos-0002"), "일이 묶음 칸을 받았다");
    }

    /// 시작한 칸은 설정에서 온다. 두 칸짜리면 시작했어도 첫 칸이다.
    #[test]
    fn a_group_uses_the_configured_columns() {
        let two = Config::parse("prefix = \"argos\"\nstatuses = \"open,done\"\n").unwrap();
        let issues = vec![
            make("argos-0001", Kind::Epic, "done"),
            member("argos-0002", "argos-0001", "done"),
            member("argos-0003", "argos-0001", "open"),
        ];
        assert_eq!(group_states(&issues, &two).get("argos-0001"), Some(&"open"));
    }

    /// **묶음을 미뤄도 그 밑의 다른 미룸은 그대로다.** 멤버를 빼는 까닭을 가까운
    /// 하나로만 보면, 미룬 부모 밑의 자식이 제 에픽이 미뤄지는 순간 그 에픽을
    /// 뿌리로 받아 도로 세어진다 — 미루기 하나로 에픽이 `done` 에서 되돌아 나온다.
    #[test]
    fn shelving_a_group_does_not_bring_back_what_is_shelved_deeper() {
        let base = || {
            let mut epic = make("argos-0001", Kind::Epic, "todo");
            epic.milestone = Some("argos-0009".into());
            vec![
                make("argos-0009", Kind::Milestone, "todo"),
                epic,
                member("argos-0003", "argos-0001", "done"),
                put_off_line(member("argos-0002", "argos-0001", "todo")),
                // 미룬 부모의 자식 — 소속은 부모에게서 물려받는다.
                make("argos-0002.aaa", Kind::Issue, "todo"),
            ]
        };
        assert_eq!(state_of(&base(), "argos-0001"), "done");

        let mut shelved_epic = base();
        shelved_epic[1] = put_off_line(shelved_epic[1].clone());
        assert_eq!(state_of(&shelved_epic, "argos-0001"), "done", "제 미룸이 남의 미룸을 풀었다");

        let mut shelved_mile = base();
        shelved_mile[0] = put_off_line(shelved_mile[0].clone());
        assert_eq!(state_of(&shelved_mile, "argos-0001"), "done", "물려받은 미룸이 풀었다");
        assert_eq!(state_of(&shelved_mile, "argos-0009"), "done");
    }

    /// **읽은 칸이 done 인 묶음은 미뤄 둔 것으로 안 센다.** 끝난 줄은 미룬 것이
    /// 아니다 — 세면 `status` 의 알림이 센 줄을 `show --deferred` 가 done 으로
    /// 숨긴다. 그 밑의 남은 일은 그대로 물려받는다.
    #[test]
    fn a_grouping_that_reads_done_is_not_shelved() {
        let closed = vec![
            put_off_line(make("argos-0001", Kind::Epic, "todo")),
            member("argos-0002", "argos-0001", "done"),
        ];
        assert!(!put_off(&closed).contains("argos-0001"), "{:?}", put_off(&closed));

        let mut open = closed.clone();
        open.push(member("argos-0003", "argos-0001", "todo"));
        let out = put_off(&open);
        assert!(out.contains("argos-0001") && out.contains("argos-0003"), "{out:?}");
    }

    /// **묶음이 제 멤버를 막으면 고리다.** 묶음은 멤버가 다 끝나야 닫히므로
    /// (`group_states`) 둘 다 영영 안 끝난다 — 적힌 칸을 옮겨 푸는 길은 없다.
    #[test]
    fn creates_cycle_counts_membership() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-0008".into());
        let issues = vec![
            make("argos-0008", Kind::Milestone, "todo"),
            epic,
            member("argos-0002", "argos-0001", "todo"),
            make("argos-0009", Kind::Issue, "todo"),
        ];
        assert!(creates_cycle(&issues, "argos-0001", "argos-0002"), "에픽이 제 멤버를 막는다");
        assert!(creates_cycle(&issues, "argos-0008", "argos-0002"), "마일스톤도 같다");
        assert!(!creates_cycle(&issues, "argos-0001", "argos-0009"), "남을 막는 것까지 막았다");
        assert!(!creates_cycle(&issues, "argos-0002", "argos-0001"), "멤버가 제 묶음을 막는 것은 고리가 아니다");
    }

    /// **읽은 칸은 묶음만 받는다.** 머지를 잘못 푼 파일에서 같은 id 를 든 일 줄이
    /// 묶음의 칸을 입으면, 목록은 `▸` 로 그리는데 `ready` 는 그 줄을 집으라고 낸다.
    #[test]
    fn a_work_row_never_wears_a_groupings_column() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0001", Kind::Issue, "todo"), // 같은 id 를 든 일 (머지 흔적)
        ];
        let cfg = cfg();
        let states = group_states(&issues, &cfg);
        assert_eq!(column(&issues[0], &states), "in_progress");
        assert_eq!(column(&issues[2], &states), "todo", "일이 묶음의 칸을 입었다");
    }

    /// 묶음이 그 칸에 들어선 때도 멤버에서 읽는다 — `--stale` 이 재는 시각이다.
    /// 적힌 `status_since` 로 재면 오늘 진행 중이 된 에픽이 "열흘째" 가 된다.
    #[test]
    fn a_group_dates_its_column_from_its_members() {
        let mut epic = make("argos-0001", Kind::Epic, "todo"); // 생성 09-01
        epic.status_since = "2026-09-01T00:00:00Z".into();
        let mut moved = member("argos-0002", "argos-0001", "in_progress");
        moved.status_since = "2026-09-11T00:00:00Z".into();
        let empty = make("argos-0007", Kind::Epic, "todo");
        let issues = vec![epic, moved, empty];
        let (e, m) = (groups(&issues), milestones(&issues));
        let roots = deferred_roots_in(&issues, &e, &m);
        let cfg = cfg();
        let stands = group_stands_in(&issues, &cfg, &e, &m, &roots);
        assert_eq!(stands["argos-0001"].since, "2026-09-11T00:00:00Z");
        // 셀 멤버가 없으면 묶음이 생긴 때다 — 안 읽히는 칸의 시각은 안 쓴다.
        assert_eq!(stands["argos-0007"].since, "2026-09-01T00:00:00Z");
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
            !st.notices.iter().any(|w| w.kind == "idea_pile"),
            "몇 개 안 되는데 벌써 말한다 — 담을 때마다 잔소리가 는다"
        );

        let mut piled = quiet;
        piled.push(idea("argos-0009"));
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        let w = st
            .notices
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
        assert!(!st.notices.iter().any(|w| w.kind == "idea_pile"), "{:?}", st.notices);
    }
    /// **알림은 경고 수에 안 든다.** 배너가 "드러난 것 N건" 이라 말하는데
    /// 담아 둔 생각이 거기 들면, 담을수록 고칠 것이 늘었다고 말하게 된다.
    #[test]
    fn a_notice_is_not_counted_among_the_warnings() {
        let piled: Vec<Issue> = (0..IDEA_PILE).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert!(st.warnings.is_empty(), "알림이 고칠 것 자리에 섰다 — {:?}", st.warnings);
        assert_eq!(st.notices.len(), 1, "{:?}", st.notices);
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
        let kinds: Vec<&str> = st.notices.iter().map(|w| w.kind).collect();
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
        let w = st.notices.iter().find(|w| w.kind == "deferred").expect("미뤄 둔 에픽이 안 보인다");
        assert_eq!(w.count, 1);
        assert!(w.notice);
    }

    /// **미뤘다 끝낸 줄은 끝난 줄이다.** 보드가 그것을 done 칸에서 빼면, 같은
    /// 화면의 롤업은 `2/2` 인데 위의 칸은 `done 1` 이라 말한다 — 한 화면이
    /// 같은 두 이슈를 두 수로 세는 것이고, 어느 쪽도 못 믿게 된다.
    #[test]
    fn closing_a_deferred_row_puts_it_back_on_the_board() {
        let mut shelved = member("argos-0002", "argos-0001", "done");
        shelved.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            shelved,
            member("argos-0003", "argos-0001", "done"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!(st.counts.get("done"), Some(&2), "미뤘다 끝낸 줄을 보드가 잃었다 — {:?}", st.counts);
        assert_eq!(st.total, 2, "{:?}", st.counts);
        assert_eq!((r.done, r.total), (2, 2), "롤업과 보드가 다른 수를 말한다 — {r:?}");
        assert!(
            !st.notices.iter().any(|w| w.kind == "deferred"),
            "끝난 것을 아직 미뤄 둔 것이라 센다 — {:?}",
            st.notices
        );
    }

    /// **흐름은 지난 이레의 기록이지 지금 계획이 아니다.** 여기서 미뤄 둔 것을
    /// 빼면 오늘 셋을 미루는 것만으로 `쌓이는 중 +5` 가 `+2` 가 되어, 미루기가
    /// 경고를 지우는 손잡이가 된다.
    #[test]
    fn deferring_does_not_rewrite_what_already_happened() {
        let issues: Vec<Issue> =
            (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "todo")).collect();
        let before = status(&issues, &[], &cfg(), "2026-09-02T00:00:00Z").flow;
        let mut after = issues;
        for i in after.iter_mut().take(3) {
            i.deferred_at = Some("2026-09-02T00:00:00Z".into());
        }
        let after = status(&after, &[], &cfg(), "2026-09-02T00:00:00Z").flow;
        assert_eq!(before.created, 5, "{before:?}");
        assert_eq!(after.created, before.created, "미루자 만든 사실이 사라졌다 — {after:?}");
        assert_eq!(after.net, before.net, "{after:?}");
    }

    // ── 미룸은 물려받는다 ─────────────────────────────────────────────

    fn picks<'a>(issues: &'a [Issue]) -> Vec<&'a str> {
        ready(issues, &cfg()).iter().map(|i| i.id.as_str()).collect()
    }

    /// **묶음을 미루면 그 밑의 일도 지금 계획이 아니다.** 에픽 줄 하나만
    /// 목록에서 사라지고 멤버가 `ready` 에 그 에픽 제목을 달고 그대로 서면,
    /// 미루기가 한 일은 머리글 하나 지운 것뿐이다.
    #[test]
    fn members_of_a_deferred_epic_are_out_of_the_plan() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let issues = vec![
            epic,
            member("argos-0002", "argos-0001", "todo"),
            member("argos-0003", "argos-0001", "in_progress"),
            make("argos-0009", Kind::Issue, "todo"),
        ];
        assert_eq!(picks(&issues), ["argos-0009"], "미룬 에픽의 멤버가 집을 일로 올라왔다");
        assert!(wip(&issues, &cfg()).is_empty(), "미룬 에픽의 멤버를 벌여 놓은 것으로 셌다");

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert_eq!(st.total, 1, "미룬 에픽의 멤버를 보드가 셌다 — {:?}", st.counts);
        let w = st.notices.iter().find(|w| w.kind == "deferred").expect("미룬 것을 안 말한다");
        assert_eq!(w.count, 3, "물려받은 것을 안 세면 `--deferred` 가 내는 수와 어긋난다");
    }

    /// 마일스톤도 묶음이다. 에픽을 거쳐 물려받는다.
    #[test]
    fn a_deferred_milestone_takes_its_epics_members_along() {
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let issues = vec![stone, epic, member("argos-0003", "argos-0002", "todo")];
        assert!(picks(&issues).is_empty(), "미룬 마일스톤 밑의 일이 올라왔다");
    }

    /// 부모를 미루면 그 밑의 자식도 같이 빠진다. 자식이 남아 있으면 부모는
    /// 직접 하는 일이 아니라는 규칙과 짝이다 — 둘 중 하나만 서면 미룬 부모의
    /// 자식이 `ready` 맨 위에 선다.
    #[test]
    fn children_of_a_deferred_parent_are_out_of_the_plan() {
        let issues = vec![
            deferred("argos-0001", "todo"),
            make("argos-0001.aaa", Kind::Issue, "todo"),
            make("argos-0001.aaa.bbb", Kind::Issue, "todo"),
        ];
        assert!(picks(&issues).is_empty(), "{:?}", picks(&issues));
    }

    /// **끝난 줄은 물려받아도 미룬 것이 아니다.** 제 줄에 대한 규칙
    /// (`is_put_off`)과 같다 — 미룬 에픽 밑에서 끝낸 일이 보드의 done 칸에서
    /// 사라지면 롤업과 어긋난다.
    #[test]
    fn a_closed_member_of_a_deferred_epic_stays_on_the_board() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let issues = vec![epic, member("argos-0002", "argos-0001", "done")];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z");
        assert_eq!(st.counts.get("done"), Some(&1), "{:?}", st.counts);
    }

    // ── 생각은 소속을 물려주지 않는다 ────────────────────────────────

    /// **세는 자와 그리는 자가 같은 답을 본다.** 생각은 뿌리로 올라가므로
    /// 그 밑의 자식이 생각의 에픽을 받으면 `멤버 0/1` 밑에 줄이 없다.
    #[test]
    fn a_child_of_a_thought_does_not_inherit_its_grouping() {
        let stone = make("argos-0001", Kind::Milestone, "todo");
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let mut thought = make("argos-0003", Kind::Idea, "todo");
        thought.epic = Some("argos-0002".into());
        let child = make("argos-0003.aaa", Kind::Issue, "todo");
        let mut placed = make("argos-0003.bbb", Kind::Issue, "todo");
        placed.epic = Some("argos-0002".into());
        let issues = vec![stone, epic, thought, child, placed];

        let g = groups(&issues);
        assert_eq!(g.get("argos-0003"), Some(&"argos-0002"), "생각 제 소속을 잃었다");
        assert_eq!(g.get("argos-0003.aaa"), None, "생각의 에픽을 물려받았다");
        assert_eq!(g.get("argos-0003.bbb"), Some(&"argos-0002"), "제가 적은 에픽을 잃었다");
        assert_eq!(milestones(&issues).get("argos-0003.aaa"), None, "생각을 지나 마일스톤을 받았다");

        let rolls = rollup(&issues, &cfg());
        assert_eq!(roll_of(&rolls, Some("argos-0002")).total, 1, "{rolls:?}");
        assert_eq!(roll_of(&rolls, None).total, 1, "{rolls:?}");
    }

    /// **끊는 것은 뿌리로 올라간 생각뿐이다.** 이슈 밑에 접힌 생각은 그 이슈의
    /// 에픽 안에 그려지므로, 거기서도 끊으면 그 자식이 에픽 안에 그려지면서
    /// `에픽 없음` 으로 세어지고, 부모를 미뤄도 `ready` 에 남는다.
    #[test]
    fn a_thought_folded_under_work_still_passes_its_place_down() {
        let stone = make("argos-0001", Kind::Milestone, "todo");
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let mut work = make("argos-0003", Kind::Issue, "todo");
        work.epic = Some("argos-0002".into());
        let thought = make("argos-0003.aaa", Kind::Idea, "todo");
        let child = make("argos-0003.aaa.bbb", Kind::Issue, "todo");
        let mut issues = vec![stone.clone(), epic, work, thought, child];

        assert_eq!(groups(&issues).get("argos-0003.aaa.bbb"), Some(&"argos-0002"), "접힌 생각에서 소속이 끊겼다");
        assert_eq!(milestones(&issues).get("argos-0003.aaa.bbb"), Some(&"argos-0001"));
        assert_eq!(roll_of(&rollup(&issues, &cfg()), Some("argos-0002")).total, 2);

        issues[2].deferred_at = Some("2026-09-01T00:00:00Z".into());
        assert!(picks(&issues).is_empty(), "미룬 이슈 밑에 그려진 일이 올라왔다");

        // 에픽 없이 마일스톤에 바로 든 이슈 밑이어도 같다.
        let mut work = make("argos-0004", Kind::Issue, "todo");
        work.milestone = Some("argos-0001".into());
        let issues = vec![
            stone,
            work,
            make("argos-0004.aaa", Kind::Idea, "todo"),
            make("argos-0004.aaa.bbb", Kind::Issue, "todo"),
        ];
        assert_eq!(milestones(&issues).get("argos-0004.aaa.bbb"), Some(&"argos-0001"));
    }

    /// 생각의 에픽을 미뤄도 그 밑의 일은 안 빠진다 — 소속을 안 받았으니
    /// 미룸도 안 받는다. 생각 자신을 미룬 것은 받는다.
    #[test]
    fn a_thought_passes_down_only_its_own_deferral() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut thought = make("argos-0002", Kind::Idea, "todo");
        thought.epic = Some("argos-0001".into());
        let issues = vec![epic, thought.clone(), make("argos-0002.aaa", Kind::Issue, "todo")];
        assert_eq!(picks(&issues), ["argos-0002.aaa"], "세지도 않는 에픽의 미룸을 받았다");

        thought.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let issues = vec![thought, make("argos-0002.aaa", Kind::Issue, "todo")];
        assert!(picks(&issues).is_empty(), "미룬 생각 밑의 일이 올라왔다");
    }

    // ── 못 읽는 줄의 id ──────────────────────────────────────────────

    /// **못 읽는 줄이 산 줄의 id 를 들고 있으면 중복이다.** 줄 번호만 보던 때는
    /// `unreadable_line` 만 서고 `duplicate_id` 는 안 서, 그 줄이 읽히는 날에야
    /// 모든 쓰기가 막혔다(moai-4dk4).
    #[test]
    fn an_unreadable_line_reusing_a_live_id_is_a_duplicate() {
        let issues = vec![make("argos-0001", Kind::Issue, "todo")];
        let clash = [Unreadable { id: Some("argos-0001") }];
        let st = status(&issues, &clash, &cfg(), "2026-09-01T00:00:00Z");
        let w = st.warnings.iter().find(|w| w.kind == "duplicate_id").expect("중복을 못 봤다");
        assert_eq!(w.ids, ["argos-0001"]);
        assert!(w.fatal);

        let apart = [Unreadable { id: Some("argos-0002") }, Unreadable { id: None }];
        let st = status(&issues, &apart, &cfg(), "2026-09-01T00:00:00Z");
        assert!(!st.warnings.iter().any(|w| w.kind == "duplicate_id"), "{:?}", st.warnings);
    }

    // ── 미룬 것이 막고 있으면 까닭을 말한다 ──────────────────────────

    /// **미룬 막음은 여전히 막는다.** 미룬 일은 끝난 일이 아니다 — 무시하면
    /// 안 끝난 일 위에 선 것을 집으라고 내민다. 대신 `ready` 가 까닭 없이
    /// 비지 않도록 무엇이 막는지를 따로 낸다.
    #[test]
    fn work_held_by_a_deferred_blocker_is_named_not_offered() {
        let mut blocked = make("argos-0002", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0001".into()];
        let issues = vec![deferred("argos-0001", "todo"), blocked];
        assert!(picks(&issues).is_empty(), "미룬 막음을 끝난 것으로 봤다");

        let held = held(&issues, &cfg());
        assert_eq!(held.len(), 1, "막힌 까닭을 안 낸다");
        assert_eq!(held[0].issue.id, "argos-0002");
        assert_eq!(held[0].by, ["argos-0001"]);
    }

    /// 미룬 것이 막는 막음은 `blocked_stale` 이 아니라 제 이름으로 말한다 —
    /// 그쪽은 막는 줄이 어느 목록에도 없어 풀 방법을 못 댄다.
    #[test]
    fn status_names_a_deferred_blocker_instead_of_a_stale_block() {
        let mut blocked = make("argos-0002", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0001".into()];
        let issues = vec![deferred("argos-0001", "todo"), blocked];
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"blocked_stale"), "{kinds:?}");
        let w = st.warnings.iter().find(|w| w.kind == "blocked_by_deferred").expect("{kinds:?}");
        assert_eq!(w.ids, ["argos-0002"]);
        assert!(!w.notice && !w.fatal, "{w:?}");
    }

    /// 막는 것이 미룬 에픽 밑에 있어도 같다 — 물려받은 미룸도 미룸이다.
    #[test]
    fn an_inherited_deferral_also_holds() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut blocked = make("argos-0003", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0002".into()];
        let issues = vec![epic, member("argos-0002", "argos-0001", "todo"), blocked];
        let held = held(&issues, &cfg());
        assert_eq!(held.len(), 1);
        // 도로 집을 곳은 막는 멤버가 아니라 미룬 에픽이다 — 멤버에 `--undo` 는 헛손질이다.
        assert_eq!(held[0].by, ["argos-0002"]);
        assert_eq!(held[0].undo, ["argos-0001"]);
    }

    /// 미룬 마일스톤 밑의 빈 에픽은 `미뤄 둔 것` 으로 세면서 `속이 빈 에픽` 으로
    /// 꾸짖지 않는다. 미룰수록 잔소리가 느는 실패가 물려받은 자리에서 돌아온다.
    #[test]
    fn an_epic_under_a_deferred_milestone_is_not_scolded() {
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let st = status(&[stone, epic], &[], &cfg(), "2026-09-11T00:00:00Z");
        let kinds: Vec<&str> = st.warnings.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"empty_epic"), "{kinds:?}");
    }

    // ── 미룸은 소속이 서는 곳에서만 물려받는다 ──────────────────────────

    /// **에픽은 에픽에게서 미룸을 안 받는다.** 에픽 줄이 든 엉뚱한 `epic` 으로 받으면
    /// 그 에픽은 표에 `미룸` 이 서고 목록에서 숨는데, 제 멤버는 `ready` 에 남는다.
    #[test]
    fn an_epic_does_not_inherit_through_a_stray_epic_field() {
        let mut shelved = make("argos-0001", Kind::Epic, "todo");
        shelved.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut stray = make("argos-0002", Kind::Epic, "todo");
        stray.epic = Some("argos-0001".into());
        let issues = vec![shelved, stray, member("argos-0003", "argos-0002", "todo")];
        let out = put_off(&issues);
        assert!(!out.contains("argos-0002"), "에픽이 에픽에게서 미룸을 받았다 — {out:?}");
        assert_eq!(picks(&issues), ["argos-0003"]);
    }

    /// **없는 부모와 종류가 틀린 참조는 미룸을 안 넘긴다.** `groups` 는 없는 부모에서
    /// 멈추고 틀린 참조는 `(길 잃음)` 에 서므로, 넘어가서 받으면 그리지도 세지도 않는
    /// 조상의 미룸에 끌려 계획에서 빠진다.
    #[test]
    fn deferral_stops_where_membership_stops() {
        let mut lost = make("argos-0003", Kind::Issue, "todo");
        lost.epic = Some("argos-0001".into()); // 에픽 자리에 이슈
        let issues = vec![
            deferred("argos-0001", "todo"),
            make("argos-0001.aaa.bbb", Kind::Issue, "todo"), // 가운데 줄이 없다
            lost,
        ];
        let out: Vec<&str> = put_off(&issues).into_iter().collect();
        assert_eq!(out, ["argos-0001"], "소속이 안 서는 곳에서 미룸을 받았다");
    }

    /// **미루기가 `에픽 없음` 비율을 올리지 않는다.** 에픽을 통째로 미루면 분모만
    /// 줄어, 원래 있던 소속 없는 일 하나가 "열린 것의 100%" 로 서고 `Stop` 이
    /// "경고가 늘었다" 로 세션을 붙들었다.
    #[test]
    fn deferring_an_epic_does_not_raise_the_no_epic_ratio() {
        let mut issues = vec![make("argos-0001", Kind::Epic, "todo"), make("argos-0099", Kind::Issue, "todo")];
        issues.extend((2..=10).map(|n| member(&format!("argos-{n:04}"), "argos-0001", "todo")));
        let no_epic = |issues: &[Issue]| {
            status(issues, &[], &cfg(), "2026-09-11T00:00:00Z").warnings.iter().any(|w| w.kind == "no_epic")
        };
        assert!(!no_epic(&issues), "미루기 전부터 경고가 섰다 — 시험이 헛돈다");
        issues[0].deferred_at = Some("2026-09-01T00:00:00Z".into());
        assert!(!no_epic(&issues), "미루기 하나로 `에픽 없음` 경고가 섰다");
    }
}
