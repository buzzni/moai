//! 세는 일과 고르는 일. **순수 함수다** — `&[Issue]` 만 보고 아무것도 찍지 않는다.
//!
//! **여기가 TUI 와의 계약이다.** 나중에 `ratatui` 는 `view` 를 건너뛰고
//! 이 모듈과 `query` 를 직접 부른다. 무엇을 셀지 정하는 코드가 `cmd/` 나
//! `view.rs` 에 있으면 표면이 늘 때마다 같은 것을 다시 짜야 한다.

use crate::config::Config;
use crate::model::{FUTURE_SLACK_SECS, Issue, Kind, days_since};
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
    /// `total` 가운데 **계획에서 뺀** 멤버의 수([`Stand::deferred`], moai-zxwj).
    ///
    /// **분모는 그대로다**(2026-09-21 사용자 결정). 막대는 "한때 내건 것 중 얼마나 했나" 라
    /// 미룬 멤버도 세고, 그래서 그 넷을 영영 안 해도 100% 가 안 된다 — 그것이 정한 값이다.
    /// 대신 **수가 왜 안 줄어드는지를 화면에 세우는 자**가 이 값이다. 없으면 읽는 쪽은
    /// `102/110` 에서 여덟이 남은 줄 알고, 그 가운데 넷은 아무도 집을 일이 아니라는 것을
    /// 보드만 보고는 알 길이 없다.
    ///
    /// `column` 과 **같은 자리에서 채운다** — 둘 다 [`Stand`] 에서 온다. 집계만 필요한
    /// `rollup` 은 비워 둔다: 이 값은 미룬 뿌리를 타고 올라가야 알고([`counted`]), 그 셈은
    /// 묶음의 칸을 읽는 걸음이 이미 걷는다. 여기서 다시 걸으면 같은 것을 세는 자가 둘이 된다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deferred: Option<usize>,
}

/// 한 줄이 속한 에픽 — 제목이거나, 적힌 id 의 줄이 없거나.
///
/// **말이 아니라 자료다**(moai-ivt9 의 결을 잇는다). 한때 못 찾은 것을 `report.epic_gone`
/// 의 글로 여기서 지어 냈는데, 그 글이 제 괄호를 달고 있어 감싸는 쪽마다 표기가 갈렸다 —
/// [`crate::view::prime`] 은 한 번 더 감싸 `((없는 에픽))` 을 냈고 `ready`·`list` 는 안
/// 감쌌다. 표기를 정하는 자리는 그리는 쪽이고, 여기는 **찾았는가**만 말한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpicLabel<'a> {
    /// 찾았다 — 그 에픽의 제목.
    Named(&'a str),
    /// `epic` 은 적혔는데 그 id 의 줄이 이 파일에 없다.
    Gone,
}

/// 줄 → 그것이 속한 에픽([`EpicLabel`]). 화면이 필요한 것은 id 가 아니라
/// 제목이고, 소속 판정([`groups`])과 제목 찾기를 한 번에 끝내 둔다.
///
/// **지도가 아니라 묻는 자다**(moai-7iyc.rt6). 한때 `(id, 종류)` 를 키로 한 지도였는데, 그러면
/// 같은 id 를 같은 종류로 든 줄 둘이 한 칸을 나눠 써 앞줄이 뒷줄의 에픽 제목을 달았다 — 목록의
/// 에픽 칸만 `derived_epic`·`-e`·트리와 다른 답을 냈다. 줄을 받아 [`stands_in`] 에 묻는 꼴이라야
/// **그 줄에 적힌 `epic`** 이 답이 될 수 있다. 가려진 줄이 쌍둥이의 제목을 달던 자리(moai-b5lu)도
/// 같은 자가 함께 막는다 — 키에 종류를 싣던 우회가 필요 없어졌다.
pub struct EpicLabels<'a> {
    /// 줄 id → 그 줄의 제목. **에픽만 담지 않는다** — `epic` 이 에픽 아닌 줄을 가리키는 것은
    /// 못 쓸 참조([`Misplace::Epic`])지 없는 참조가 아니라, 그때도 가리킨 줄의 제목을 댄다.
    /// [`EpicLabel::Gone`] 은 **그 id 의 줄이 이 파일에 없을 때**다.
    named: BTreeMap<&'a str, &'a str>,
    /// 소속을 id 에 진 줄의 답([`groups`]). **든 꼴 그대로 나른다**([`Kinds`] 와 같은 결) —
    /// 이 지도를 이미 든 쪽(`query::Where`)이 제목만 얹으려고 `groups` 를 한 벌 더 짓지
    /// 않게 한다(moai-jk2u.pl6).
    placed: Placing<'a>,
    /// 가려진 줄을 가르는 지도([`Kinds`]).
    kinds: Kinds<'a>,
}

impl<'a> EpicLabels<'a> {
    /// 그 **줄**이 든 에픽의 표. 어느 에픽에도 안 서면 `None` 이다.
    ///
    /// 묶음 줄도, 가려진 줄도 여기서 `None` 이다 — 판정은 [`stands_in`] 하나가 한다.
    pub fn at<'s>(&'s self, i: &'s Issue) -> Option<EpicLabel<'s>> {
        let epic = stands_in(&self.kinds, i, self.placed.at(i.id.as_str()))?;
        Some(self.named.get(epic).map_or(EpicLabel::Gone, |t| EpicLabel::Named(t)))
    }

    /// 제목만 든 표 — 소속을 id 에 진 줄이 없는 자리(그리기 시험)가 쓴다. 줄은 제 `epic` 으로
    /// 답을 내고, 그 id 가 여기 없으면 [`EpicLabel::Gone`] 이다.
    #[cfg(test)]
    pub fn titled(named: BTreeMap<&'a str, &'a str>) -> EpicLabels<'a> {
        EpicLabels { named, placed: Placing::Own(BTreeMap::new()), kinds: Kinds::no_twins() }
    }
}

/// [`EpicLabels`] 를 짓는다.
pub fn epic_labels(all: &[Issue]) -> EpicLabels<'_> {
    EpicLabels { named: titles(all), placed: Placing::Own(groups(all)), kinds: Kinds::of(all) }
}

/// [`epic_labels`] 와 같은 것. 소속 지도와 종류 지도를 **빌려서** 짓는다 — `moai show` 는 바로
/// 옆에서 거름망이 그 둘을 들고 있는데, 제목을 얹으려고 `groups` 와 [`Kinds`] 를 한 벌씩 더
/// 지었다(moai-jk2u.pl6). 종류 지도는 [`Kinds::lend`] 로 받는다.
///
/// **`moai ready` 는 아직 이 길이 아니다**(리뷰 moai-jk2u.m60) — 그쪽 사람 길은 거름망을 아예
/// 안 지어 빌릴 것이 없다. 빌려 줄 쪽이 생기면 그때 옮긴다.
pub fn epic_labels_over<'a>(
    all: &'a [Issue],
    placed: &'a BTreeMap<&'a str, &'a str>,
    kinds: Kinds<'a>,
) -> EpicLabels<'a> {
    EpicLabels { named: titles(all), placed: Placing::Kept(placed), kinds }
}

fn titles(all: &[Issue]) -> BTreeMap<&str, &str> {
    all.iter().map(|i| (i.id.as_str(), i.title.as_str())).collect()
}

/// 소속 지도를 **든 꼴 그대로** — [`Kinds`] 와 같은 결이고 같은 까닭이다. 옮겨 담으면
/// 줄마다 한 칸인 지도를 표면마다 한 벌씩 짓는다.
enum Placing<'a> {
    Own(BTreeMap<&'a str, &'a str>),
    Kept(&'a BTreeMap<&'a str, &'a str>),
}

impl<'a> Placing<'a> {
    fn at(&self, id: &str) -> Option<&'a str> {
        match self {
            Placing::Own(m) => m.get(id).copied(),
            Placing::Kept(m) => m.get(id).copied(),
        }
    }
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
    let (epic_of, mile_of) = ties(all);
    deferred_roots_in(all, &epic_of, &mile_of).into_keys().collect()
}

/// 계획에서 빠진 줄 id → **실제로 `deferred_at` 을 든 줄** id.
///
/// 물려받은 줄에 `moai defer <그 줄> --undo` 를 시키면 "이미 그렇다" 로
/// 끝나고 아무것도 안 풀린다. 되돌리는 말을 대는 자리는 이것으로 미룬 곳을 댄다.
pub fn deferred_roots(all: &[Issue]) -> BTreeMap<&str, &str> {
    if !all.iter().any(is_put_off) {
        return BTreeMap::new();
    }
    let (epic_of, mile_of) = ties(all);
    deferred_roots_in(all, &epic_of, &mile_of)
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
    deferred_roots_over(all, &Lines::of(all), epic_of, mile_of)
}

/// [`deferred_roots_in`] 과 같은 것. [`Lines`] 를 이미 든 쪽([`Soil`])이 그것을 다시 짓지
/// 않게 받는다. **문은 여기 하나다** — 부르는 쪽에 같은 문을 또 세우면 미룬 줄이 있는
/// 저장소에서 목록을 한 벌 더 훑는다(리뷰 moai-jk2u.m60).
pub fn deferred_roots_over<'a>(
    all: &'a [Issue],
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, &'a str> {
    fold_roots(all, &shelved_over(all, lines, epic_of, mile_of))
}

/// 줄마다의 답을 **id 로 접는다 — 뒷줄이 이긴다**(2026-09-23 사용자 결정, moai-u3ta).
/// [`folded_by_id`] 와 같은 차례고, `None` 이 앞줄의 답을 지우는 것도 같다 — 넣기만 하면
/// 미룬 릴리스에 든 앞줄의 미룸이 산 릴리스에 든 뒷줄에 그대로 남는다.
///
/// **id 밖에 없는 표면의 답이 이것이다** — `moai show <id>` 의 `shelved_by`, `defer`·`mv`·
/// `idea`·`edit` 이 대는 "여기를 푸세요" 말. `moai show <id>` 가 찍는 줄이 `by_id` 가 고르는
/// 뒷줄이므로, 그 줄의 답이 아닌 것을 곁들이면 한 화면이 제 말을 뒤집는다.
fn fold_roots<'a>(all: &'a [Issue], shelved: &[Option<&'a str>]) -> BTreeMap<&'a str, &'a str> {
    // **살아 있는 검사다**(리뷰 moai-jk2u.hr4). `debug_assert` 는 릴리스에서 빠지고, 그러면
    // 아래 `zip` 이 짧은 쪽에 맞춰 조용히 잘라 뒷줄들이 답을 통째로 잃는다 — 자리로 짝지은
    // 배열을 따로 받는 자리에서는 어긋남이 틀린 답이 아니라 없는 답이 된다.
    assert_eq!(all.len(), shelved.len(), "판정이 줄과 자리를 맞춰야 한다");
    fold_by_id(all, shelved.iter().copied())
}

/// 줄마다의 판정을 **id 로 접는다 — 같은 id 의 뒷줄이 이긴다.** `milestones` 와 같은 차례다.
/// 넣기만 하면 종류가 다른 앞줄의 판정이 뒷줄에 남는다(moai-2m9p, [`eclipsed`]).
///
/// **접는 몸은 하나다**(리뷰 moai-jk2u.m60): 차례를 두 곳에 적으면 시험이 보는 지도와
/// `moai status` 가 내는 지도가 언젠가 갈린다. 축이 둘이 되면서([`fold_roots`]) 그 하나를
/// 여기 둔다 — 판정의 타입만 다르고 차례는 같은 글이다.
fn fold_by_id<V>(all: &[Issue], verdict: impl IntoIterator<Item = Option<V>>) -> BTreeMap<&str, V> {
    let mut out = BTreeMap::new();
    for (i, m) in all.iter().zip(verdict) {
        match m {
            Some(m) => out.insert(i.id.as_str(), m),
            None => out.remove(i.id.as_str()),
        };
    }
    out
}

/// **줄마다** — 그 줄을 계획에서 뺀 가장 가까운 줄, 없으면 계획 안이다. `all` 과 **자리가
/// 같은** 배열이다([`under_lost`] 의 `lost` 와 같은 꼴).
///
/// 접은 지도([`deferred_roots_over`])는 id 로 짠 것이라 같은 id 의 줄 둘을 못 가른다. 담는
/// 꼴이 `filter_map().collect()` 이던 동안은 "이 줄은 안 미뤄졌다" 를 적을 길조차 없어서, 한
/// 쌍둥이가 받은 미룸이 다른 줄에 말없이 갔다 — 미룬 릴리스에 든 앞줄 때문에 산 릴리스에 든
/// 뒷줄이 `ready` 에서도 `held` 에서도 사라졌다(moai-u3ta, 리뷰 moai-jk2u.m60 3번).
///
/// **새는 방향은 한쪽뿐이었다.** `filter_map` 은 `Some` 만 적으므로 샘은 늘 "미뤘다" 쪽으로
/// 갔고, 계획 밖인 줄이 계획 안으로 읽힌 적은 없다.
pub fn shelved_over<'a>(
    all: &'a [Issue],
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> Vec<Option<&'a str>> {
    // 문은 여기에도 둔다 — [`Lines`] 를 이미 든 쪽이 부르므로 짓는 값은 안 아끼지만,
    // 미룬 줄 없는 저장소에서 목록을 한 번 더 걷는 것은 그대로 아낀다.
    if !all.iter().any(is_put_off) {
        return vec![None; all.len()];
    }
    let shelf = Shelf::new(lines, epic_of, mile_of);
    let mut out: Vec<Option<&str>> =
        all.iter().map(|i| (!closed_by_hand(i)).then(|| shelf.nearest(i)).flatten()).collect();
    // **읽은 칸이 done 인 묶음은 제 줄이 빠진다.** 끝난 줄은 미룬 것이 아니다([`is_put_off`]).
    // 묶음이 끝났는지는 멤버를 다 센 뒤에야 알므로 물려주는 셈(`closed_by_hand`)에서는 못
    // 거르고 여기서 거른다 — 안 거르면 `moai status` 의 `미뤄 둔 것` 이 센 줄을 그 줄이
    // 가리키는 `moai show --deferred` 가 done 으로 숨기고, `moai mv` 는 끝난 묶음에 도로
    // 집으라고 한다. **그 밑의 줄이 물려받은 미룸은 그대로다** — 멤버 쪽 답은 이미 셌다.
    // **자리로 지운다**(리뷰 moai-jk2u.hr4). id 로 지우던 때는 둘이 어긋났다 — 끝난 묶음마다
    // 목록을 다시 걸어 미룬 묶음 수에 줄 수를 곱한 만큼 글자를 견줬고(그 수는 릴리스 하나를
    // 접을 때마다 는다), 한 줄이 done 으로 읽혔다는 이유로 같은 id 의 **다른** 묶음 줄까지
    // 계획 안으로 돌려놨다. 읽은 칸은 `(종류, id)` 로 센 멤버에 제 줄의 미룸을 얹어 나오므로
    // 줄마다 갈릴 수 있다.
    for at in settled_groups(all, &out, lines, epic_of, &shelf) {
        out[at] = None;
    }
    out
}

/// [`shelved_over`] 와 같은 것. 소속 지도만 든 쪽이 [`Lines`] 를 안 짓게 하는 짧은 길이다.
pub fn shelved_in<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> Vec<Option<&'a str>> {
    if !all.iter().any(is_put_off) {
        return vec![None; all.len()];
    }
    shelved_over(all, &Lines::of(all), epic_of, mile_of)
}

/// [`shelved_over`] 와 같은 것. 아무것도 안 든 쪽의 짧은 길이다.
pub fn shelved(all: &[Issue]) -> Vec<Option<&str>> {
    if !all.iter().any(is_put_off) {
        return vec![None; all.len()];
    }
    let (epic_of, mile_of) = ties(all);
    shelved_in(all, &epic_of, &mile_of)
}

/// 계획에서 빠진 줄을 **자리로** 짚는 집합 — 줄을 가리키는 바늘로 담는다.
///
/// [`counted`] 가 멤버마다 조상을 다시 타지 않게 하는 재료다(리뷰 moai-jk2u.hr4). 옛
/// `roots.contains_key` 는 지도 한 번 짚기였는데 줄로 옮기며 `Shelf::nearest` 한 벌이 되어,
/// 멤버 1만 9천짜리 저장소의 `moai status` 가 미룬 줄 하나에 98ms 늘었다 — 값을 치르는 것은
/// 흔한 **계획 안인** 멤버 쪽이다(멈출 미룸이 없어 사슬을 끝까지 탄다). id 가 아니라 바늘로
/// 담으므로 같은 id 의 두 줄이 안 섞인다.
fn off_rows<'a>(all: &'a [Issue], shelved: &[Option<&'a str>]) -> BTreeSet<*const Issue> {
    all.iter().zip(shelved).filter(|(_, r)| r.is_some()).map(|(i, _)| std::ptr::from_ref(i)).collect()
}

/// 읽은 칸이 `done` 이라 **제 줄은 계획 밖으로 안 세는** 묶음 — 그 **자리**를 낸다.
fn settled_groups<'a>(
    all: &'a [Issue],
    shelved: &[Option<&'a str>],
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    shelf: &Shelf<'a, '_>,
) -> Vec<usize> {
    // 계획에서 빠진 묶음이 없으면 멤버를 셀 것도 없다.
    let groups: Vec<(usize, &Issue)> = all
        .iter()
        .enumerate()
        .zip(shelved)
        .filter(|((_, i), r)| r.is_some() && is_group(i))
        .map(|((at, i), _)| (at, i))
        .collect();
    if groups.is_empty() {
        return Vec::new();
    }
    let off = off_rows(all, shelved);
    let members = members_in(all, epic_of, lines, &kinds(all));
    groups.into_iter().filter(|(_, g)| reads_done(&counted(g, &members, Some(shelf), &off))).map(|(at, _)| at).collect()
}

/// 계획에서 빠졌는가가 **줄마다 갈리는** id. 성한 저장소에서는 빈다 — 그때는 접은 지도가 곧
/// 줄마다의 답이라, 줄을 든 쪽도 지도를 짚어 답할 수 있다([`crate::query::Where::deferred`]).
///
/// **되묻지 못하는 것은 묶음 줄이지 묶음이 선 id 가 아니다**(리뷰 moai-jk2u.hr4). 묶음이 계획
/// 밖인가에는 읽은 칸이 걸리고([`settled_groups`]) 그 칸은 멤버를 id 로 세어 나온 값이라 줄로
/// 되물을 수 없다 — 그래서 [`crate::query::Where::deferred`] 가 **묶음 줄에서만** 접은 지도로
/// 간다. 한때 여기서 id 를 통째로 뺐는데, 그러면 묶음과 id 를 나눠 쓴 **일 줄**까지 제 답을
/// 잃었다: 미룬 에픽에 든 일 줄이 같은 id 의 에픽 줄 때문에 `moai show --deferred` 에서
/// 사라지고, 그러면서 `moai status` 의 `미뤄 둔 것` 은 그 줄을 세고 `moai ready` 는 안 내줘
/// 한 줄이 어느 목록에도 안 서는 채로 셈에만 남았다.
pub fn torn_ids<'a>(all: &'a [Issue], shelved: &[Option<&'a str>]) -> BTreeSet<&'a str> {
    assert_eq!(all.len(), shelved.len(), "판정이 줄과 자리를 맞춰야 한다");
    let mut seen: BTreeMap<&str, bool> = BTreeMap::new();
    let mut torn = BTreeSet::new();
    for (i, r) in all.iter().zip(shelved) {
        let id = i.id.as_str();
        if seen.insert(id, r.is_some()).is_some_and(|was| was != r.is_some()) {
            torn.insert(id);
        }
    }
    torn
}

/// **줄 하나**를 계획에서 뺀 가장 가까운 줄 — [`shelved_over`] 가 그 줄에 내리는 그 답이다.
/// 지도가 든 답이 이 줄의 것이 아닐 때([`torn_ids`]) 줄을 든 쪽이 이것으로 되묻는다.
///
/// **묶음 줄에는 안 쓴다** — 묶음이 계획 밖인가에는 [`settled_groups`] 가 id 로 재는 읽은 칸이
/// 걸려 여기서는 못 되묻는다. 부르는 쪽([`crate::query::Where::deferred`])이 그 줄을 가른다.
pub fn shelved_at_line<'a>(
    i: &'a Issue,
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> Option<&'a str> {
    (!closed_by_hand(i)).then(|| Shelf::new(lines, epic_of, mile_of).nearest(i)).flatten()
}

/// 계획에서 빠진 줄 id → 그 줄을 **계획에 도로 넣으려면 풀어야 할 미룸 전부**, 가까운 것부터.
///
/// [`deferred_roots`] 는 가장 가까운 하나를 댄다 — "어디 밑에서 빠졌나" 를 말하는 자리는
/// 그것이면 된다. 그러나 도로 집는 말을 대는 자리가 그것만 대면, 제 줄도 미뤘고 미룬
/// 에픽에도 든 줄은 하나를 풀고도 여전히 빠진 채 그제야 다음을 댄다(moai-phzi).
///
/// 키는 [`deferred_roots`] 와 같다. **읽은 칸이 done 인 묶음의 미룸도 댄다** — 그 묶음 줄만
/// 계획 밖으로 안 셀 뿐([`deferred_roots_in`]), 그 밑의 줄은 가까운 미룸을 풀면 그 묶음을
/// 뿌리로 받아 여전히 빠진다(리뷰 moai-ha03.qlr). 걸음이 같은 묶음을 두 번 짚어도 한 번만 댄다.
pub fn deferred_sources(all: &[Issue]) -> BTreeMap<&str, Vec<&str>> {
    if !all.iter().any(is_put_off) {
        return BTreeMap::new();
    }
    let (epic_of, mile_of) = ties(all);
    let roots = deferred_roots_in(all, &epic_of, &mile_of);
    deferred_sources_in(all, &epic_of, &mile_of, &roots)
}

/// [`deferred_sources`] 와 같은 것. 소속 지도와 계획 밖 줄을 이미 가진 쪽(`status`)이 두 번
/// 걷지 않게 받는다 — 거기서 다시 부르면 묶음 멤버 셈과 미룸 걸음을 한 번 더 한다.
pub fn deferred_sources_in<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
    roots: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    // 계획에서 빠진 줄이 없으면 짚을 것도 없다 — 답은 아래 걸음과 같은 빈 지도인데, [`Shelf`] 는
    // 목록을 통째로 한 번 걷는다. 미룬 줄 하나 없는 저장소가 보통이다(`deferred_roots_in` 과 같은 문).
    if roots.is_empty() {
        return BTreeMap::new();
    }
    let lines = Lines::of(all);
    let shelf = Shelf::new(&lines, epic_of, mile_of);
    // **id 를 든 줄을 다 걷는다**(리뷰 moai-jk2u.m60) — [`deferred_roots_in`] 이 id 를 그
    // 지도에 올린 것은 **어느 한 줄의** 걸음이라, 여기서 `by_id` 로 줄 하나를 되찾으면 그 줄이
    // 아닐 수 있다. 미룬 에픽을 제 `epic` 에 적은 앞줄이 id 를 올렸는데 뒷줄에는 소속이 없던
    // 판에서, 되찾은 뒷줄의 걸음이 비어 `moai ready` 가 `moai defer  --undo` 를 냈다 — id 가
    // 빠진 채로 사람 앞에 선 명령이고, `--json` 의 `undo` 키는 아예 없었다.
    let mut out: BTreeMap<&str, Vec<&str>> = roots.keys().map(|&id| (id, Vec::new())).collect();
    // 거르개는 [`deferred_roots_in`] 과 같다 — 거기가 안 센 줄이 여기서 미룸을 대면, 그 id 를
    // 계획 밖으로 민 것이 아닌 줄을 풀라고 말한다.
    for i in all.iter().filter(|i| !closed_by_hand(i)) {
        let Some(every) = out.get_mut(i.id.as_str()) else { continue };
        // 걸음은 같은 묶음을 두 번 짚는다 — 자식과 부모가 같은 에픽에 들면 둘 다에서.
        // 거르지 않으면 `moai defer E E --undo` 를 댄다.
        for r in shelf.every(i) {
            // **done 으로 읽은 묶음의 미룸도 댄다.** 그 묶음 줄만 계획 밖으로 안 셀 뿐, 그
            // 밑의 줄은 가까운 미룸을 풀면 그 묶음을 뿌리로 받아 여전히 빠진다 — 거르면
            // 하나를 풀고서야 다음을 댄다(moai-phzi). 첫째가 늘 가까운 것이라 비지도 않는다.
            if !every.contains(&r) {
                every.push(r);
            }
        }
    }
    out
}

/// 줄 하나를 계획에서 빼는 미룸을 **가까운 것부터** 짚는 길 — 제 줄, 제가 든
/// 에픽·마일스톤, 그다음 부모. [`deferred_roots_in`] 은 첫째만 쓰고(그 줄을 뺀 곳),
/// [`counted`] 는 전부 본다(묶음 제 미룸 말고도 그 멤버를 빼는 까닭이 있나).
struct Shelf<'a, 'm> {
    lines: &'m Lines<'a>,
    epic_of: &'m BTreeMap<&'a str, &'a str>,
    mile_of: &'m BTreeMap<&'a str, &'a str>,
}

impl<'a, 'm> Shelf<'a, 'm> {
    /// **[`Lines`] 는 받는다** — 여기서 지으면 `status` 한 번에 [`groups`]·[`milestones_over`]
    /// 와 나란히 세 벌이 선다(moai-jk2u.pl6, `지도는 한 벌이다` moai-g0zx).
    fn new(
        lines: &'m Lines<'a>,
        epic_of: &'m BTreeMap<&'a str, &'a str>,
        mile_of: &'m BTreeMap<&'a str, &'a str>,
    ) -> Shelf<'a, 'm> {
        Shelf { lines, epic_of, mile_of }
    }

    /// id 로 짚은 줄의 미룸 — **조상에게만 쓴다.** 조상의 답은 그 조상 줄의 것이고,
    /// 그 줄을 고르는 자는 `by_id`(뒷줄이 이긴다)라 [`groups`] 와 같은 자다.
    fn own(&self, id: &'a str) -> Option<&'a str> {
        self.lines.at(id).is_some_and(is_put_off).then_some(id)
    }

    /// 그 줄이 **든 묶음**의 미룸. **소속이 실제로 서는 곳만** 본다 — 에픽은 에픽에
    /// 안 들고(`nav` 는 에픽 밑에 에픽을 두지 않는다), 마일스톤은 뿌리에 서며,
    /// 종류가 틀린 참조는 `(길 잃음)` 이다. 그런 참조로 미룸을 받으면 저만 계획에서
    /// 빠지고 제 멤버는 남는다 — 에픽 줄이 든 엉뚱한 `epic` 이 실제로 그랬다.
    ///
    /// **값으로 받는다** — 지도를 짚은 `Option<&&str>` 을 받던 때는 줄마다 잰 답과 지도를 짚은
    /// 답이 부르는 자리에서 똑같이 생겨서, 축 하나만 줄로 옮기고 다른 축을 지도에 둔 것을
    /// 컴파일러도 읽는 사람도 못 봤다(리뷰 moai-jk2u.m60 — 마일스톤 축이 그렇게 남았다).
    fn joined(&self, to: Option<&'a str>, want: Kind) -> Option<&'a str> {
        to.filter(|t| self.lines.at(t).map(|x| x.kind) == Some(want)).and_then(|t| self.own(t))
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
    ///
    /// **첫 걸음은 줄이고 그 위는 id 다**(moai-jk2u.olh). 소속은 이제 줄마다 갈리는데
    /// ([`joined_in`], 2026-09-23 사용자 결정) 이 걸음이 id 하나로만 올라, 제 `epic` 을
    /// 적은 앞줄이 미룬 에픽의 멤버로 세어지면서 그 미룸은 안 물려받았다 — `show <에픽>
    /// --json` 의 `members` 는 그 줄을 드는데 `ready` 는 그대로 내주고 `show --deferred`
    /// 는 끝내 안 냈다. AGENTS.md 가 못박은 "묶음을 미루면 그 밑이 함께 빠진다" 가
    /// 거기서 거짓이 된다. 조상은 그대로 id 로 오른다 — 조상의 소속은 그 조상 줄의
    /// 것이고 `by_id` 로 고른 뒷줄이 [`groups`] 가 고른 그 줄이다.
    fn walk(&self, from: &'a Issue, mut visit: impl FnMut(&'a str) -> bool) {
        let start = from.id.as_str();
        let mut cur = Some(start);
        while let Some(at) = cur {
            if at != start && self.lines.rooted.contains(at) {
                if let Some(r) = self.own(at) {
                    visit(r);
                }
                return;
            }
            // 첫 걸음의 재료는 물은 줄 제 것이다. 그 위는 `by_id` 가 고른 줄이다.
            let line = if at == start { Some(from) } else { self.lines.at(at) };
            let here = line.map(|x| x.kind);
            // **`line` 이 고른 그 줄에 묻는다** — 줄을 한 번 고르고 축마다 다시 고르면, 축
            // 하나가 딴 줄을 보게 되는 것이 컴파일에 안 걸린다(리뷰 moai-jk2u.m60).
            let own = line.filter(|l| is_put_off(l)).map(|_| at);
            if let Some(r) = own
                && visit(r)
            {
                return;
            }
            let epic = match line {
                Some(i) if at == start => joined_in(i, || self.epic_of.get(at).copied()),
                _ => self.epic_of.get(at).copied(),
            };
            if matches!(here, Some(Kind::Issue | Kind::Idea))
                && let Some(r) = self.joined(epic, Kind::Epic)
                && visit(r)
            {
                return;
            }
            // **마일스톤 축도 첫 걸음은 줄이다**(리뷰 moai-jk2u.m60). 에픽 축만 줄로 옮기고
            // 이 줄을 `mile_of` 에 둔 동안, 제 `milestone` 에 미룬 릴리스를 적은 앞줄이 그
            // 릴리스의 멤버로 세어지면서 미룸은 안 물려받았다 — `show <릴리스>` 는 그 줄을
            // 멤버로 드는데 `ready` 는 그대로 내주고 `show --deferred` 는 끝내 안 냈다. 거꾸로
            // 아무것도 안 적은 앞줄이 뒷줄의 미룬 릴리스를 입어 `ready` 에서도 `held` 에서도
            // 사라지기도 했다. 값은 [`stood_at_line`] 이 낸다 — `mile_of` 를 짓는 그 몸이라
            // id 가 하나뿐인 줄에서는 글자째 같은 답이다.
            let stone = match line {
                Some(i) if at == start => stood_at_line(i, epic, self.lines),
                _ => self.mile_of.get(at).copied(),
            };
            if matches!(here, Some(Kind::Issue | Kind::Idea | Kind::Epic))
                && let Some(r) = self.joined(stone, Kind::Milestone)
                && visit(r)
            {
                return;
            }
            cur = crate::id::parent_of(at).filter(|p| self.lines.at(p).is_some());
        }
    }

    /// 그 줄을 계획에서 뺀 가장 가까운 줄.
    fn nearest(&self, from: &'a Issue) -> Option<&'a str> {
        let mut found = None;
        self.walk(from, |r| {
            found = Some(r);
            true
        });
        found
    }

    /// 그 줄을 계획에서 빼는 줄 전부.
    fn every(&self, from: &'a Issue) -> Vec<&'a str> {
        let mut out = Vec::new();
        self.walk(from, |r| {
            out.push(r);
            false
        });
        out
    }
}

/// id 로 줄을 찾는 지도와 **뿌리로 올라간 생각**([`rooted_thoughts`]) — 늘 함께 다니는
/// 둘이다. [`groups`]·[`milestones_in`]·[`Shelf`] 가 저마다 지어 들던 것이고,
/// [`Placed::Milestone`] 이 줄마다 답하게 되면서 그 자리에도 필요해졌다(moai-jk2u.wvn).
///
/// **둘을 한 자리에 묶는 까닭은 차례 때문이다.** `rooted` 는 `by_id` 로 지어지므로 따로
/// 들면 짝이 안 맞는 둘을 넘길 수 있고, 그러면 소속이 그 생각에서 끊기는 자리가 표면마다
/// 갈린다. 지어 두면 값도 한 벌이다.
/// **자리 지도는 안 든다**(리뷰 moai-jk2u.m60). 한때 `id → 그 줄의 첨자` 를 곁에 지었는데,
/// 읽는 자는 [`under_lost`] 하나이고 그마저 "길 잃은 줄이 하나라도 있는가" 문 뒤에 선다 —
/// 길 잃은 줄 없는 흔한 저장소에서는 [`Lines`] 를 짓는 열댓 자리가 모두 그 지도를 짓고
/// 아무도 안 읽었다. 필요한 한 자리에서 짓는다.
#[derive(Default)]
pub struct Lines<'a> {
    by_id: BTreeMap<&'a str, &'a Issue>,
    rooted: BTreeSet<&'a str>,
}

impl<'a> Lines<'a> {
    pub fn of(all: &'a [Issue]) -> Lines<'a> {
        let by_id: BTreeMap<&str, &Issue> = all.iter().map(|i| (i.id.as_str(), i)).collect();
        let rooted = rooted_thoughts(&by_id);
        Lines { by_id, rooted }
    }

    /// 그 id 를 **마지막으로 든 줄** — 같은 id 가 둘이면 뒷줄이다. 조상을 짚는 자리가
    /// 이것을 쓴다([`Shelf::walk`] 의 오름과 같은 자다).
    fn at(&self, id: &str) -> Option<&'a Issue> {
        self.by_id.get(id).copied()
    }
}

/// 담아 둔 생각인가. **술어를 `cmd/` 에 두지 않는다** — 어떤 줄이 무엇인지
/// 정하는 코드가 거기 있으면 다음 표면이 같은 판단을 다시 짠다.
pub fn is_idea(i: &Issue) -> bool {
    i.kind == Kind::Idea
}

/// 그 칸 이름을 **이 저장소가 아는가** — `config` 가 대거나, 어느 줄이 실제로 거기 서
/// 있거나(moai-hym7, 사람이 정했다).
///
/// 오타는 그대로 갈린다. 거절이 노리는 것은 오타지 낡음이 아니다 — `config` 에서 칸
/// 이름을 고치면 옛 이름에 선 줄이 남는데, 그 이름을 아무 데서도 안 받으면 그 줄은
/// 찾을 수도 집을 수도 없다. 쓰기(`--from`)와 읽기(`show -s`·탐색기 필터)가 **같은
/// 술어**를 쓴다 — 읽기가 쓰기보다 엄하면 옮길 수는 있는데 못 찾는 줄이 생긴다.
pub fn knows_column(all: &[Issue], cfg: &Config, name: &str) -> bool {
    cfg.knows(name) || all.iter().any(|i| i.status.as_str() == name)
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
///
/// **가려진 줄([`eclipsed`])은 집은 일이 아니다**(moai-es40, 사용자 결정 2026-09-18) — `ready`·
/// `held` 가 그 줄을 집을 일로 안 내는 것(moai-lg2t)과 같은 자다. 그 줄은 트리에서 `(길 잃음)` 에
/// 서고 어느 묶음에도 안 드는데, 여기만 세면 보드의 벌여 놓은 셈·`ready` 아래 줄·훅 초점이 그 줄을
/// 집은 일로 댄다. 대가는 훅 초점이 그 줄을 못 싣는 것이다 — 그 id 는 `duplicate_id` 로 파일째 쓰기가
/// 막혀, 고치기 전에는 그 줄로 일할 수 없다. **자리를 묻는 쪽([`places`]·[`stranded`])만 그 줄까지
/// 본다**([`started`]).
pub fn wip<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    let held = started(issues, cfg);
    if held.is_empty() {
        return held;
    }
    let eclipsed = eclipsed(issues);
    held.into_iter().filter(|i| !eclipsed(i)).collect()
}

/// **시작한 칸에 선 일 줄** — 미룬 것만 빼고, 쌍둥이에게 가려진 줄([`eclipsed`])도 든다. [`wip`] 는
/// "지금 하는 일" 을 묻고 이것은 "그 칸을 옮겨 놓은 줄이 어디서 일하나" 를 묻는다(moai-es40, 사용자
/// 결정). 머지가 같은 id 의 에픽 뒷줄을 남기면 앞줄의 집힌 이슈는 가려지는데, 그 세션이 죽었으면 그
/// 줄을 대는 것은 [`stranded`] 뿐이다 — 여기서 걸렀다면 `duplicate_id` 하나 때문에 버려진 칸이 영영
/// 안 보인다(리뷰 moai-ya06). 자리 셈은 넷(`places`·`blinding`·`placeable`·`stranded`)이 **이 하나**로
/// 되짚는다 — 하나라도 `wip` 을 쓰면 `stranded` 가 센 줄의 자리를 `show` 가 모른다. 그 넷에 스냅샷을
/// 대는 쪽(`worktree::workplaces` 의 문과 옆 스냅샷이 쥔 줄)도 이것으로 잰다(리뷰 moai-r8gw.b4s) —
/// 거기서 `wip` 으로 재면 문이 닫히거나 쥔 줄에서 빠져, 스냅샷으로만 찾을 수 있는 그 줄이 자리를 잃는다.
pub fn started<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    // **값싼 것을 먼저 거른다.** 훅이 도구 호출마다 여기를 지나므로, 집은 것이
    // 없으면 조상을 타는 셈(`put_off`)도 종류 지도(`eclipsed`)도 아예 안 돌린다.
    let held = in_started_columns(issues, cfg);
    if held.is_empty() {
        return Vec::new();
    }
    let out = shelved(issues);
    held.into_iter().filter(|(k, _)| out[*k].is_none()).map(|(_, i)| i).collect()
}

/// [`started`] 와 같은 것. **이미 잰 소속 지도를 받는다**([`Footing`]) — 미룬 줄을 빼는 걸음이
/// 조상과 소속을 타므로, 자리를 묻는 넷이 저마다 이것을 부르면 한 명령에 소속 지도가 네 벌 선다.
pub fn started_in<'a>(
    issues: &'a [Issue],
    cfg: &Config,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
) -> Vec<&'a Issue> {
    let held = in_started_columns(issues, cfg);
    if held.is_empty() {
        return Vec::new();
    }
    let out = shelved_in(issues, epic_of, mile_of);
    held.into_iter().filter(|(k, _)| out[*k].is_none()).map(|(_, i)| i).collect()
}

/// 시작한 칸에 선 일 줄 — **미룸은 아직 안 뺐다.** [`started`] 와 [`started_in`] 의 값싼 첫 걸음이다.
///
/// **자리를 함께 낸다**(moai-u3ta). 미룸을 빼는 자가 줄마다 답하는 배열이라([`shelved`]) 그
/// 답을 짚으려면 자리가 있어야 한다 — id 로 짚으면 같은 id 의 앞줄이 뒷줄의 답을 입어, 산
/// 에픽에 든 줄을 집고도 훅이 "아무것도 안 집었다" 고 말한다.
fn in_started_columns<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<(usize, &'a Issue)> {
    issues.iter().enumerate().filter(|(_, i)| is_work(i) && cfg.is_started(i.status.as_str())).collect()
}

/// 이 줄이 `names` 가 가리키는 일인가 — 그 줄 자신이나 조상이 들었거나, 그 에픽·마일스톤이
/// 들었다. 워크트리 이름(`worktree::names`)으로 **그 워크트리의 일**을 가르는 자 하나다 — 훅의
/// 초점(`hook::held`)과 집은 줄의 자리([`places`])가 같은 자로 재야, 초점에서 뺀 줄을 자리 없다고
/// 하거나 그 반대가 되지 않는다.
pub fn claimed<'a>(issues: &'a [Issue], names: &'a BTreeSet<String>) -> impl Fn(&Issue) -> bool + 'a {
    let (epics, stones) = if names.is_empty() { Default::default() } else { ties(issues) };
    move |i: &Issue| claims(&epics, &stones, names, i)
}

/// 이름이 줄을 가리키는지 재는 **소속 지도 한 벌** — 줄 → 에픽([`groups`]), 줄 → 마일스톤([`milestones_in`]).
/// 에픽 지도는 한 번만 짓는다 — [`milestones`] 는 제 안에서 `groups` 를 다시 짓는다. 이름 여럿을 같은
/// 줄들에 잴 때(훅의 옆 이름과 모르는 줄, `hook::theirs`) 이 한 벌로 잰다.
pub(crate) fn ties(issues: &[Issue]) -> (BTreeMap<&str, &str>, BTreeMap<&str, &str>) {
    // **[`Lines`] 도 한 벌이다**(moai-jk2u.pl6) — 따로 부르면 둘이 저마다 짓는다.
    let lines = Lines::of(issues);
    let epics = groups_over(issues, &lines);
    let stones = milestones_over(issues, &epics, &lines);
    (epics, stones)
}

/// [`claimed`] 의 몸통 — **이미 푼 소속 지도**로 잰다. 워크트리가 여럿이면 지도는 하나고 이름만
/// 바뀌므로([`places`]), 워크트리마다 `groups`·`milestones` 를 다시 지으면 `moai status` 한 번이
/// 같은 걸음을 워크트리 수만큼 걷는다 — [`status`] 가 "한 번만 잰다" 고 적어 둔 것과 같은 까닭이다.
///
/// 안 읽은 줄의 "내게 온 것"(`query::unread`)도 이 걸음을 쓴다 — 마일스톤 지도를 비워 넘겨서(moai-j038.vna).
pub(crate) fn claims(
    epics: &BTreeMap<&str, &str>,
    stones: &BTreeMap<&str, &str>,
    names: &BTreeSet<String>,
    i: &Issue,
) -> bool {
    nearness(epics, stones, names, i).is_some()
}

/// `names` 가 이 줄을 **얼마나 가까이** 가리키는가 — 작을수록 구체적이다. 못 가리키면 `None`.
///
/// 차례는 제 id(0) → 가까운 조상(1, 2, …) → 에픽 → 마일스톤이다(moai-m62u, 사용자 결정). 한 줄을
/// 여러 워크트리의 이름이 가리키면 **가장 가까운 이름이 이긴다** — 머지하고 안 치운 `worktree-<에픽>`
/// 이 뒤이어 집은 멤버를, 그 멤버를 제 이름으로 띄운 워크트리보다 먼저 쥐던 판은 그 워크트리의
/// 세션을 규칙 2 로 막았다. 겨루는 곳이 둘이다 — 훅의 초점([`crate::hook::Away`])과 줄의 자리([`places`]).
pub(crate) fn nearness(
    epics: &BTreeMap<&str, &str>,
    stones: &BTreeMap<&str, &str>,
    names: &BTreeSet<String>,
    i: &Issue,
) -> Option<usize> {
    // 조상은 깊어 봐야 몇 칸이다 — 에픽·마일스톤은 그 뒤에 선다.
    const EPIC: usize = 1 << 16;
    const STONE: usize = 1 << 17;
    if names.is_empty() {
        return None;
    }
    std::iter::successors(Some(i.id.as_str()), |id| crate::id::parent_of(id))
        .enumerate()
        .filter_map(|(depth, id)| {
            if names.contains(id) {
                Some(depth)
            } else if epics.get(id).is_some_and(|e| names.contains(*e)) {
                Some(EPIC)
            } else if stones.get(id).is_some_and(|m| names.contains(*m)) {
                Some(STONE)
            } else {
                None
            }
        })
        .min()
}

/// 옆 이름(`away`)이 이 줄을 **제 이름(`own`)보다 가까이** 가리키는가 — 그러면 옆의 일이다.
/// 같은 거리면 제 것이다(moai-m62u, 사용자 결정): 그 줄을 제 이름으로 띄운 워크트리가 둘이면
/// 어느 쪽 세션도 그 일을 못 집는 것보다, 둘 다 제 것으로 보는 편이 덜 틀린다.
///
/// [`claims`] 처럼 **이미 푼 소속 지도**([`ties`])로 잰다 — 훅(`hook::theirs`)은 같은 지도로 모르는 줄까지
/// 잰다. 둘이 제 지도를 따로 짓던 판은 한 번 판정에 두 벌을 지었다(리뷰 moai-3k2d.1df).
pub(crate) fn claims_over(
    epics: &BTreeMap<&str, &str>,
    stones: &BTreeMap<&str, &str>,
    away: &BTreeSet<String>,
    own: &BTreeSet<String>,
    i: &Issue,
) -> bool {
    match nearness(epics, stones, away, i) {
        None => false,
        Some(there) => nearness(epics, stones, own, i).is_none_or(|here| there < here),
    }
}

/// 살아 있는 딸린 워크트리 하나 — 집은 일이 서 있을 수 있는 자리(moai-ir8q).
///
/// **스냅샷에 적지 않는다.** 집기는 main 에서 커밋하니 적히는 곳은 늘 main 이고, 워크트리를
/// 치운 뒤에도 적힌 자리는 남아 거짓말한다 — 그러면 그것을 바로잡는 명령이 필요해진다. 그래서
/// 부르는 쪽(`worktree::workplaces`)이 git 이 적어 둔 파일에서 그때그때 읽어 여기 담고, 판정은
/// 이 자료와 `&[Issue]` 만 받는 순수 함수([`places`]·[`stranded`])가 한다.
#[derive(Debug, Clone, Serialize)]
pub struct Workplace {
    /// 워크트리의 꼭대기 — **main 워크트리의 꼭대기에서 잰 상대 경로**다(`worktree::workplaces`).
    /// main 밖에 만든 워크트리는 `../` 로 올라가서 재고, main 이 없는 맨몸 저장소는 그 저장소
    /// 디렉터리에서 잰다(moai-xpd7·moai-3aec). 여기서는 워크트리를 가르는 이름으로만 쓴다.
    ///
    /// **글자로 낸다** — `PathBuf` 를 그대로 직렬화하면 UTF-8 이 아닌 경로에서 serde 가 통째로
    /// 실패해, 그 워크트리와 아무 상관 없는 줄의 `moai show --json` 까지 안 열린다. 사람 화면은
    /// `display()` 로 이미 너그럽다 — 두 화면이 같은 자리에서 갈라지지 않게 여기도 너그럽게 낸다.
    #[serde(serialize_with = "lossy_path")]
    pub path: std::path::PathBuf,
    /// 브랜치. 떼어 낸 HEAD 면 커밋 앞 일곱 자.
    pub branch: String,
    /// 이름이 가리키는 id 후보 — 디렉터리 이름, 브랜치, `worktree-` 를 뗀 브랜치.
    #[serde(skip)]
    pub names: BTreeSet<String>,
    /// 그 워크트리의 스냅샷에 벌여 놓인 줄. 이름이 id 가 아닌 워크트리(에이전트 격리 따위)가 쥔
    /// 일을 가른다. **이름이 집은 줄을 하나라도 가리키는 워크트리에는 안 쓴다**([`places`]) —
    /// 갈라질 때 main 에 벌여 놓였던 남의 일이 다 물려 들어와 있어서다.
    #[serde(skip)]
    pub holds: BTreeSet<String>,
    /// 그 워크트리에서 **main 보다** 늦게 옮기거나 고친 줄. 물려받은 것이 아니라 그 워크트리가
    /// 실제로 만진 흔적이라 이름과 상관없이 쓴다.
    #[serde(skip)]
    pub touched: BTreeSet<String>,
    /// 그 워크트리가 **제 손으로 적어 둔 집기**(`worktree::note_held` 의 `moai-held`). 짐작이 아니라
    /// 기록이라 [`places`] 에서 **이름에게 지지 않는다** — 이름은 "이 일은 저 워크트리 것이겠다" 는
    /// 짐작이고, 이것은 "여기서 집었다" 는 그 자리의 말이다. 안 치운 에픽 워크트리가 있다는 까닭으로
    /// 이것을 버리던 판은, 그 에픽의 멤버를 집은 에이전트 격리 워크트리를 화면에서 통째로 지웠다
    /// (리뷰 moai-71ht 셋째 판의 훑기).
    ///
    /// **언제나 읽는다** — 옆 스냅샷과 달리 작은 파일 하나라(`crate::worktree::workplaces` 의 문)
    /// 이름으로 자리가 다 잡히는 빠른 길에서도 읽는다. 그래야 답이 남의 줄에 따라 흔들리지 않는다.
    #[serde(skip)]
    pub marked: BTreeSet<String>,
    /// 워크트리가 뜬 시각(RFC3339). 이름이 id 가 아닌 워크트리의 `holds` 에서 **뜨기 직전에 집은
    /// 줄만** 고르는 데 쓴다([`places`]). 못 읽었으면 `None` 이고, 그러면 `holds` 를 다 믿는다.
    #[serde(skip)]
    pub born: Option<String>,
    /// 그 워크트리의 스냅샷을 **열어 보려 했는데 못 읽었다**(moai-lt7h) — 권한이 없거나 파일
    /// 자리에 엉뚱한 것이 섰다. `holds`·`touched` 가 비어 있는 것이 "아무도 거기서 일 안 한다" 가
    /// 아니라 "모른다" 라는 뜻이다. 안 열어 봤으면 거짓이다 — 열어 볼 까닭이 없으면 안 연다
    /// ([`crate::worktree::workplaces`]). **스냅샷이 아예 없는 워크트리도 거짓이다** — 거기 적힐
    /// 수 있는 줄이 없어 "안 쥐었다" 가 사실이다(`crate::worktree::holds`). **열리는데 줄이 안
    /// 풀리는 것도 거짓이다** — 머지 충돌이 그렇다. 까닭과 값은 `crate::worktree::holds` 에 있다.
    #[serde(skip)]
    pub unknown: bool,
    /// **그 스냅샷을 못 읽었다** — 파일 자체의 사실이다. [`Workplace::unknown`] 과 가른다
    /// (moai-giz3): 저것은 *그래서 무엇을 쥐었는지 모른다* 는 판정 쪽 사실이라, 이름만으로 자리가
    /// 다 잡혀 스냅샷을 아예 안 파는 길([`crate::worktree::workplaces_in`] 의 문)에서는 깨진
    /// 파일이 있어도 서지 않았다. 그러면 같은 저장소를 `moai status` 는 조용히 지나고
    /// `moai status --worktree` 는 그 워크트리를 대, 두 화면이 같은 상태를 달리 말한다
    /// (moai-7p48 의 재현). 깨진 것은 고칠 사람이 있어야 고쳐지므로 그 사실은 판정과 무관하게
    /// 선다 — `crate::worktree::Unread::all` 이 이것으로 센다.
    ///
    /// **`--json` 에 안 싣는다** — 이 값을 싣는 자리(`status --json` 의 `broken_worktrees`)가
    /// 이미 "깨진 것들" 이라, 줄마다 참을 한 번 더 적는 셈이다. 늘린 키는 되무를 수 없다.
    #[serde(skip)]
    pub broken: bool,
}

/// 집은 줄 하나가 **어디에 서 있는가**(moai-xn9n·moai-lt7h). `status` 의 경고와 `show` 의 `자리`
/// 줄이 같은 답을 받는다 — 한때 앞의 것에만 한 시간 틈이 있어, 방금 집은 줄을 `show` 가 버려진
/// 것처럼 냈다.
#[derive(Debug, Clone)]
pub enum Place<'a> {
    /// 여기서 돈다. 빈 목록으로는 오지 않는다.
    At(Vec<&'a Workplace>),
    /// 자리는 안 보이지만 **방금 집었다** — 규약은 집기를 커밋한 뒤 워크트리를 띄운다.
    Fresh,
    /// 자리는 안 보이는데 **못 읽은 워크트리가 있다.** 거기일 수 있다.
    Unknown,
    /// 집었는데 일하는 워크트리가 없다. [`stranded`] 가 세는 것이 이것뿐이다.
    Lost,
}

impl<'a> Place<'a> {
    /// 기계가 읽는 한 낱말 — `--json` 의 `place`.
    pub fn word(&self) -> &'static str {
        match self {
            Place::At(_) => "at",
            Place::Fresh => "fresh",
            Place::Unknown => "unknown",
            Place::Lost => "lost",
        }
    }

    /// 얼마나 확실한 자리인가 — **작을수록 확실하다.** 줄 하나를 가르는 자리([`settle`] 의
    /// `match`)와 묶음이 멤버 여럿에서 하나를 고르는 자리가 **같은 차례를 써야 한다** — 갈리면
    /// 에픽이 `모른다` 라고 하는데 그 멤버는 `방금 집었다` 라고 한다. 차례를 저쪽에 한 벌 더
    /// 적어 두었더니 변형을 더할 때 `match` 만 컴파일 오류로 잡히고 이쪽은 조용히 지나갔다.
    fn rank(&self) -> u8 {
        match self {
            Place::At(_) => 0,
            Place::Unknown => 1,
            Place::Fresh => 2,
            Place::Lost => 3,
        }
    }

    /// 서 있는 워크트리들 — `At` 이 아니면 비어 있다. `--json` 의 `workplaces`.
    pub fn at(&self) -> &[&'a Workplace] {
        match self {
            Place::At(v) => v,
            _ => &[],
        }
    }
}

/// 경로를 **글자로** 낸다 ([`Workplace::path`]) — serde 의 `Path` 는 UTF-8 이 아닌 경로에서
/// 실패하는데, 그 실패가 `moai show --json` 을 통째로 못 열게 한다.
fn lossy_path<S: serde::Serializer>(p: &std::path::Path, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&p.to_string_lossy())
}

/// 자리 판정이 재는 것 **한 벌** — 집은 줄([`started`], id 로 접은 것)과 소속 지도([`ties`]).
///
/// 자리를 묻는 넷([`places`]·[`blinding`]·[`stranded`]·[`placeable_in`])과 스냅샷을 팔지 고르는 문
/// (`worktree::workplaces`)이 저마다 이 둘을 지어, `moai status`·`moai show` 한 번에 같은 걸음을
/// 네댓 벌 걸었다(moai-rviv). 걸음이 값싸지 않다 — 집은 줄을 고르는 데 미룸이 조상과 소속을 타고
/// ([`put_off`]), 그 미룸이 다시 소속 지도를 재료로 쓴다.
///
/// **쓸 때 짓는다.** 자리를 묻는 길에는 재료를 하나도 안 보고 끝나는 흔한 갈래가 있다 — 볼
/// 워크트리가 없는 저장소, 딸린 워크트리에서 겹쳐 보지 않을 때(`worktree::workplaces` 가 바로
/// 돌아선다). 미리 지으면 그 갈래가 소속 지도와 미룸 걸음을 헛되이 치르고, 그것을 피하려고 부르는
/// 쪽이 "그 갈래인가" 를 제 손으로 다시 재면 그 판단이 두 곳에 서게 된다(moai-6opu.p65 가 문을
/// `workplaces` 한 곳에 모은 까닭이다). 게으르게 두면 문은 그대로 한 곳이고 그 갈래는 공짜다.
///
/// **판정은 여기 없다.** 이것은 재료일 뿐이고 무엇이 어디에 서는가는 여전히 [`settle`] 이 정한다 —
/// 재료를 나르는 그릇에 판정을 얹으면 그릇을 안 든 표면이 둘째 답을 갖는다.
pub struct Footing<'a, 'c> {
    all: &'a [Issue],
    cfg: &'c Config,
    laid: std::cell::OnceCell<Laid<'a>>,
}

/// [`Footing`] 이 실제로 잰 것. 밖으로 나가지 않는다 — 부르는 쪽은 `Footing` 만 들고 다닌다.
struct Laid<'a> {
    /// 집은 줄 — **같은 id 는 한 번만, 뒷줄이 선다**(`Load::get` 과 같은 자). 줄마다 세면 한
    /// 워크트리가 한 id 에 두 번 서서 받는 쪽이 "두 곳에서 돌고 있다" 로 읽는다(moai-ddtg).
    picked: BTreeMap<&'a str, &'a Issue>,
    /// 줄 → 그 줄이 든 에픽([`groups`]).
    epics: BTreeMap<&'a str, &'a str>,
    /// 줄 → 그 줄이 선 마일스톤([`milestones_in`]).
    stones: BTreeMap<&'a str, &'a str>,
}

impl<'a, 'c> Footing<'a, 'c> {
    /// 아직 아무것도 안 잰다 — 처음 물을 때 한 번 잰다.
    pub fn of(all: &'a [Issue], cfg: &'c Config) -> Footing<'a, 'c> {
        Footing { all, cfg, laid: std::cell::OnceCell::new() }
    }

    fn laid(&self) -> &Laid<'a> {
        self.laid.get_or_init(|| {
            let (epics, stones) = ties(self.all);
            // 지도를 **먼저** 짓고 그것으로 집은 줄을 고른다 — [`started`] 를 그냥 부르면 미룸을
            // 빼는 걸음이 제 안에서 같은 지도를 한 벌 더 짓는다([`started_in`]).
            let picked =
                started_in(self.all, self.cfg, &epics, &stones).into_iter().map(|i| (i.id.as_str(), i)).collect();
            Laid { picked, epics, stones }
        })
    }

    /// 집은 줄 — id 로 접은 것.
    pub fn picked(&self) -> &BTreeMap<&'a str, &'a Issue> {
        &self.laid().picked
    }

    /// 재료를 잰 줄 전부.
    pub fn all(&self) -> &'a [Issue] {
        self.all
    }

    /// 그 줄이 **든 에픽**([`groups`]) — 제 `epic` 을 적었으면 그것, 아니면 물려받은 것.
    ///
    /// 계획이 세우는 멤버는 소속을 id 에 지고 `epic` 을 안 적으므로(moai-exh7), 적힌 필드만 보는
    /// 쪽은 그 줄을 에픽 없는 줄로 읽는다 — 상세의 `에픽` 줄이 통째로 빠진 자리가 거기다(리뷰).
    /// **이미 지은 지도에서 읽는다**: 이 셈은 `picked` 를 지으며 한 번에 잰 것이라 덤이 없다.
    ///
    /// **묶음 줄에는 안 쓴다**(moai-fg0t) — 에픽 줄이 든 `epic` 은 소속이 아니고, 트리도 `-e` 도
    /// 그 줄을 에픽 밑에 안 둔다. 부르는 쪽이 가른다.
    pub fn epic_of(&self, id: &str) -> Option<&'a str> {
        self.laid().epics.get(id).copied()
    }

    /// 어느 칸이 시작한 칸인가를 아는 설정 — **옆 워크트리의 스냅샷을 재는 쪽**
    /// (`worktree::holds`)이 제 줄이 아닌 목록에 같은 자를 대려고 받는다.
    pub fn cfg(&self) -> &'c Config {
        self.cfg
    }

    /// 이 줄이 `names` 가 가리키는 일인가([`claims`]) — 워크트리마다 바뀌는 것은 이름뿐이라
    /// 지도는 한 벌이다.
    pub fn claims(&self, names: &BTreeSet<String>, i: &Issue) -> bool {
        let laid = self.laid();
        claims(&laid.epics, &laid.stones, names, i)
    }
}

// 바이너리는 재료를 든 `_in` 을 부른다([`Footing`], moai-rviv) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
/// 집은 줄([`started`]) → 그 일이 [`어디에 서 있는가`](Place). **자리가 안 보이는 줄도 키를 받는다** —
/// 키가 없는 것은 안 집은 줄이다. 받는 쪽이 "안 집음" 과 "집었는데 안 보임" 을 한 지도로 가른다.
/// 집은 멤버를 둔 묶음도 키를 받는다(멤버에서 굴려 올린다, [`settle`]).
///
/// **워크트리가 하나도 없으면 아무 키도 없다**(moai-tbin) — 물을 자리 자체가 없다. 거기서 집은
/// 줄을 `Lost` 로 채우면, 워크트리를 안 쓰는 저장소(그쪽이 기본이다)의 모든 집은 줄에 빨간
/// "일하는 워크트리가 안 보인다" 가 붙는다. 한때 그 판정이 부르는 쪽의 `trees.is_empty()` 가드
/// 두 벌로만 서 있어, 이 순수 함수를 새로 부르는 표면은 그 가드를 세 벌째 적어야 했다.
///
/// **자리가 안 보이는 까닭까지 여기서 가른다**(moai-xn9n) — 방금 집어 아직 워크트리가 안 뜬
/// 것(`STRANDED_GRACE_SECS`, `Place::Fresh`), 못 읽은 워크트리가 있어 모르는 것(`Place::Unknown`),
/// 정말 자리를 잃은 것(`Place::Lost`)이다. `now` 를 받는 까닭이 이것이다. 한때 이 셋을 [`stranded`]
/// 만 갈라, `show` 의 `자리` 줄이 방금 집은 줄을 버려진 것처럼 냈다 — 두 표면이 한 답을 내려면
/// 가르는 자가 하나여야 한다.
///
/// **이름이 집은 줄을 하나라도 가리키는 워크트리는 스냅샷의 벌여 놓인 줄([`Workplace::holds`])로
/// 안 가른다.** 규약상
/// 워크트리는 main 에서 뜨므로 그 스냅샷에는 갈라질 때 main 에 벌여 놓였던 줄이 전부 있다 —
/// 그것을 세면 세션이 죽어 자리를 잃은 줄도 그 뒤에 뜬 워크트리 아무 데나 "자리" 로 잡혀,
/// 새 워크트리가 하나 뜨는 순간 `stranded` 에서 사라진다.
pub fn places<'a>(issues: &[Issue], cfg: &Config, trees: &'a [Workplace], now: &str) -> BTreeMap<String, Place<'a>> {
    places_in(&Footing::of(issues, cfg), trees, now)
}

/// [`places`] 와 같은 것. **이미 잰 재료를 받는다**([`Footing`]) — 한 명령 안에서 자리를 묻는
/// 표면이 여럿이면(`show` 의 `placeable`·`workplaces`·여기) 그 재료는 하나여야 한다(moai-rviv).
pub fn places_in<'a>(footing: &Footing<'_, '_>, trees: &'a [Workplace], now: &str) -> BTreeMap<String, Place<'a>> {
    // 아래 둘은 **빠른 길일 뿐 판정이 아니다** — 아래를 다 돌아도 [`settle`] 이 같은 답(빈 지도)을
    // 낸다. 계약을 쥔 자는 거기 하나고, 여기는 헛일을 아낄 뿐이라 갈릴 것이 없다. **워크트리를
    // 먼저 본다** — 재료를 아직 안 지었으면(게으른 [`Footing`]) 이 길은 그것을 안 짓고 지난다.
    if trees.is_empty() {
        return settle(&BTreeMap::new(), BTreeMap::new(), trees, now, &BTreeMap::new(), &BTreeMap::new(), false);
    }
    let Laid { picked, epics, stones } = footing.laid();
    let mut found: BTreeMap<&str, Vec<&Workplace>> = picked.keys().map(|id| (*id, Vec::new())).collect();
    if picked.is_empty() {
        return settle(picked, found, trees, now, &BTreeMap::new(), &BTreeMap::new(), false);
    }
    // **굴릴 곳은 어느 워크트리가 있느냐와 무관하다**(moai-oepz) — 집은 멤버를 둔 묶음은 그
    // 멤버가 `At` 이든 `Lost` 든 키를 받는다. 한때 이 줄 위에서 일찍 돌아, 문서와 `placeable` 은
    // "집은 멤버를 둔 묶음은 키를 받는다" 라고 하는데 그 길에서만 안 받는 셋째 답이 있었다.
    let rolls = rollups(footing.all(), picked, epics);
    // 못 읽은 워크트리가 판정을 가리는가 — [`blinding`] 과 같은 자(`nameless`)로 워크트리마다 잰다.
    let mut blind = false;
    for t in trees {
        let named = |i: &Issue| claims(epics, stones, &t.names, i);
        let nameless = nameless(epics, stones, picked.values().copied(), t);
        blind |= t.unknown && nameless;
        let born = t.born.as_deref().and_then(crate::model::parse_rfc3339);
        // 이름 없는 워크트리가 쥔 일은 **뜨기 한 시간 전부터 그 뒤로 움직인 줄**이다(사용자 결정,
        // moai-ir8q.beq 와 moai-40ht.hom). 규약은 집고 곧바로 워크트리를 띄우므로 그보다 한참 전에
        // 집혀 물려 들어온 줄은 그 워크트리의 일이 아니다.
        //
        // **윗금은 두지 않는다.** `status_since` 는 집은 때가 아니라 **칸이 선 때**라 칸을 옮길
        // 때마다 새로 선다 — 윗금을 두면 워크트리가 뜬 뒤 main 에서 `in_progress → review` 로
        // 옮긴 산 일이 그 자리를 잃고, 세션을 시작할 때 떠 놓고 일을 나중에 받는 에이전트
        // 격리 워크트리는 아예 아무것도 못 쥔다.
        //
        // **`started_at` 으로 재지 않는다**(moai-hav1, 2026-09-19에 재고 그대로 두기로 했다).
        // 그 필드(moai-u5bk)는 줄이 **처음** 첫 칸을 떠난 때라 되돌렸다 다시 집어도 안 바뀐다 —
        // 여기서 묻는 것은 "이 워크트리가 뜰 무렵 이 줄이 움직였나" 이므로, 옛 줄을 오늘 다시 집어
        // 에이전트 워크트리에서 하는 흔한 길이 통째로 자리를 잃는다. 되돌리기 어려운 쪽은 그쪽이다.
        // 이 필드 전에 집힌 줄이 `None` 인 것도 그대로다 — 모르는 값으로 자리를 가를 수는 없다.
        let fresh = |i: &Issue| match (born, crate::model::parse_rfc3339(&i.status_since)) {
            (Some(b), Some(s)) => b - s <= STRANDED_GRACE_SECS,
            _ => true,
        };
        for (&id, &i) in picked {
            let here = named(i)
                || t.marked.contains(id)
                || t.touched.contains(id)
                || (nameless && t.holds.contains(id) && fresh(i));
            if here && let Some(at) = found.get_mut(id) {
                at.push(t);
            }
        }
    }
    // **이름이 가리키는 줄은 그 이름의 워크트리만 낸다**(사용자 결정, 리뷰 moai-ya06.44t).
    // 스냅샷은 자리를 못 찾은 줄이 있을 때만 파므로(`worktree::workplaces` 의 문), 안 좁히면 한
    // 저장소의 같은 상태에 두 답이 난다 — 상관없는 딴 줄 하나가 자리를 잃으면 그때부터 이 줄에
    // 둘째 자리가 붙는다. 답이 남의 줄에 따라 흔들리느니, 이름이 답한 줄은 이름만으로 답한다.
    // **가장 가까운 이름만 낸다**(moai-m62u) — 안 치운 에픽 워크트리와 그 멤버를 제 이름으로 띄운
    // 워크트리가 같이 서면 일하는 곳은 뒤의 것이다. 훅의 초점([`claims_over`])과 같은 자다.
    for (&id, &i) in picked {
        let near: Vec<(&Workplace, usize)> =
            trees.iter().filter_map(|t| nearness(epics, stones, &t.names, i).map(|n| (t, n))).collect();
        let best = near.iter().map(|(_, n)| *n).min();
        let mut by_name: Vec<&Workplace> = near.iter().filter(|(_, n)| Some(*n) == best).map(|(t, _)| *t).collect();
        if by_name.is_empty() {
            continue;
        }
        // **제 손으로 적어 둔 집기는 이름에게 안 밀린다**([`Workplace::marked`], 리뷰 moai-71ht 셋째
        // 판의 훑기). 좁히는 까닭은 스냅샷으로 **짐작한** 둘째 자리가 남의 줄에 따라 흔들리는 것이었다
        // — 표식은 짐작이 아니고 늘 읽으므로 흔들릴 것이 없다. 버리던 판은 안 치운 에픽 워크트리
        // 하나가 그 에픽의 멤버를 집은 에이전트 격리 워크트리를 화면에서 지워, 이어받는 세션을 아무도
        // 없는 자리로 보냈다.
        for t in trees.iter().filter(|t| t.marked.contains(id)) {
            if !by_name.iter().any(|w| std::ptr::eq(*w, t)) {
                by_name.push(t);
            }
        }
        found.insert(id, by_name);
    }
    // **묶음의 자리에는 그 묶음 이름의 워크트리도 선다**(사용자 결정 2026-09-19, moai-t3yj). 멤버마다
    // 제 이름 워크트리가 있으면 굴림은 그것만 내는데(moai-m62u), 가지와 커밋 안 한 일이 사는 곳은
    // 에픽 워크트리이고 이어받는 세션은 `moai show <에픽>` 의 자리를 읽어 들어간다.
    let named: BTreeMap<&str, Vec<&Workplace>> = rolls
        .keys()
        .filter_map(|g| {
            let here: Vec<&Workplace> = trees.iter().filter(|t| t.names.contains(*g)).collect();
            (!here.is_empty()).then_some((*g, here))
        })
        .collect();
    settle(picked, found, trees, now, &rolls, &named, blind)
}

/// **이름이 집은 줄을 하나도 못 가리키는 워크트리인가** — 그러면 스냅샷으로 가르고([`places`]),
/// 못 읽었으면 판정을 가린다([`blinding`]). 가리키는지는 [`claims`] 로 잰다 — 줄 자신·조상·그
/// 에픽·마일스톤이다. 규약의 워크트리 이름이 에픽 id 라(CLAUDE.md "워크트리") 집은 id 와 바로 같은
/// 이름만 보면, 멤버를 쥔 에픽 워크트리 하나를 못 읽는 것만으로 저장소 전체가 "모른다" 로 접혀
/// 자리를 잃은 딴 줄이 `stranded` 에서 빠진다(moai-1i9d).
///
/// "이 이름을 쓰는 줄이 있는가" 로 묻지 않는다 — 끝난 일의 이름으로 뜬 워크트리가 그 자리에서
/// 다른 일을 하고 있어도 이름 있는 것으로 세어져 스냅샷으로 가르는 길이 통째로 닫힌다.
fn nameless<'i>(
    epics: &BTreeMap<&str, &str>,
    stones: &BTreeMap<&str, &str>,
    mut picked: impl Iterator<Item = &'i Issue>,
    t: &Workplace,
) -> bool {
    !picked.any(|i| claims(epics, stones, &t.names, i))
}

/// **자리 판정을 가리는 못 읽은 워크트리들** — 못 읽었고([`Workplace::unknown`]) 이름이 집은 줄을
/// 하나도 못 가리키는 것이다(사용자 결정, 리뷰 moai-ya06.44t). 이름이 집은 줄을 가리키는 워크트리는
/// 못 읽어도 그 줄이 이미 `At` 이라 가릴 것이 없다.
///
/// **"자리를 다 못 셌다" 를 대는 자리만 이것을 센다** — `status --json` 의 `unreadable_worktrees`,
/// `tui --json` 의 같은 키, 층 상세의 꼬리말이다. "이 스냅샷이 깨졌다" 를 대는 자리(`status` 의
/// stderr·한눈 보기의 `옆 워크트리 문제`·층의 `!`)는 못 읽은 것 **전부**를 센다
/// (`worktree::Unread::all`, 사용자 결정 2026-09-18) — 두 수는 다르다.
///
/// [`places`] 가 `Unknown` 을 세우는 자와 **같아야** 한다(moai-rgz9). 한때 못 읽은 워크트리를
/// 다 세어, 판정을 안 가리는 제 이름 워크트리 하나로 화면마다 "다 못 셌다" 가 섰다.
// 바이너리는 재료를 든 `_in` 을 부른다([`Footing`], moai-rviv) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn blinding<'a>(issues: &[Issue], cfg: &Config, trees: &'a [Workplace]) -> Vec<&'a Workplace> {
    blinding_in(&Footing::of(issues, cfg), trees)
}

/// [`blinding`] 과 같은 것. **이미 잰 재료를 받는다**([`Footing`]) — `worktree::stranded_at` 은
/// 바로 앞에서 [`places`] 로 같은 줄을 세므로, 여기서 다시 지으면 한 셈에 재료가 두 벌 선다.
pub fn blinding_in<'a>(footing: &Footing<'_, '_>, trees: &'a [Workplace]) -> Vec<&'a Workplace> {
    // 값싼 것을 먼저 — 못 읽은 워크트리가 없으면 재료([`Footing`])를 안 짓는다. 자리를 잃은 줄이
    // 없어 [`stranded_in`] 이 재료를 안 지은 채 지나가는 길이 흔하다.
    if !trees.iter().any(|t| t.unknown) {
        return Vec::new();
    }
    // 집은 줄이 없으면 가릴 판정도 없다 — [`places`] 의 빠른 길(`blind = false`)과 같은 답이다.
    // 안 거르면 `nameless` 가 빈 목록에 참을 내, 못 읽은 워크트리를 다 "가린다" 로 센다.
    //
    // **같은 id 의 줄을 접어 세도 답이 같다** — [`claims`] 는 줄에서 `id` 만 보고 소속은
    // `epics`·`stones` 지도에서 읽는데, 그 지도는 이미 뒷줄이 이긴다(`groups`). `claims` 가 줄의
    // 필드를 직접 보게 되면 접은 것과 안 접은 것이 갈리니 그때 다시 본다.
    let Laid { picked, epics, stones } = footing.laid();
    if picked.is_empty() {
        return Vec::new();
    }
    trees.iter().filter(|t| t.unknown && nameless(epics, stones, picked.values().copied(), t)).collect()
}

/// 묶음 id → **그 묶음으로 자리를 굴려 올릴 집은 멤버들**([`settle`]).
///
/// 멤버를 고르는 자는 [`group_members`] 와 **같아야 한다** — `placeable` 이 그것으로 `show` 의
/// 문을 여닫으므로, 여기가 더 너그러우면 `status` 만 세는 id 가 생기고 덜 너그러우면 `show` 가
/// 워크트리를 다 풀고도 할 말이 없다. 그래서 둘 다 [`members_in`] 을 쓴다: `(종류, 묶음 id)` 로
/// 갈라 **에픽 자리에 적힌 마일스톤 id** 같은 끊긴 참조에 키를 주지 않고, 가려진 줄(`eclipsed`)도
/// 뺀다. 손으로 `epics`·`stones` 를 거꾸로 타던 판은 그 둘을 못 갈라, 되짚는 자가 둘이 됐다.
///
/// **집힌 묶음은 건너뛴다** — 같은 id 의 쌍둥이(묶음 줄 하나, 집힌 이슈 하나)가 있으면 그 줄이
/// 제 워크트리에서 찾은 자리를 굴림이 갈아치운다.
fn rollups<'a>(
    issues: &'a [Issue],
    picked: &BTreeMap<&str, &Issue>,
    epics: &BTreeMap<&'a str, &'a str>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let kind_of = kinds(issues);
    let members = members_in(issues, epics, &Lines::of(issues), &kind_of);
    let eclipsed = |i: &Issue| is_eclipsed(&kind_of, i);
    let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for g in issues.iter().filter(|g| is_group(g) && !eclipsed(g)) {
        if picked.contains_key(g.id.as_str()) {
            continue;
        }
        let mine: Vec<&str> = members
            .get(&(g.kind, g.id.as_str()))
            .into_iter()
            .flatten()
            .map(|m| m.id.as_str())
            .filter(|id| picked.contains_key(id))
            .collect();
        if !mine.is_empty() {
            out.insert(g.id.as_str(), mine);
        }
    }
    out
}

/// 찾은 자리를 [`Place`] 로 굳히고 **묶음의 자리를 멤버에서 굴려 올린다**(moai-0h8m).
///
/// 워크트리 이름은 규약상 **에픽 id** 라(`CLAUDE.md` "워크트리"), 이어받는 세션이 `moai show <에픽>`
/// 을 먼저 읽는다 — 거기 자리가 없으면 어디로 들어갈지 못 정한다. 묶음 자체는 집히지 않으므로
/// (`wip` 은 일만 낸다) 멤버의 답을 합친다: 하나라도 서 있으면 거기고, 아니면 모름·방금·잃음 차례다.
fn settle<'a>(
    picked: &BTreeMap<&str, &Issue>,
    found: BTreeMap<&str, Vec<&'a Workplace>>,
    trees: &'a [Workplace],
    now: &str,
    rolls: &BTreeMap<&str, Vec<&str>>,
    named: &BTreeMap<&str, Vec<&'a Workplace>>,
    blind: bool,
) -> BTreeMap<String, Place<'a>> {
    // **볼 워크트리가 없으면 아무 답도 안 낸다**([`places`]) — "없다" 는 찾아보고 못 찾았을 때의
    // 말이다. 판정이 여기 있어야 부르는 표면마다 같은 가드를 다시 적지 않는다.
    if trees.is_empty() {
        return BTreeMap::new();
    }
    // `blind` 는 [`places`] 가 [`blinding`] 과 같은 자(`nameless`)로 잰 것이다 — 못 읽은 워크트리가
    // 가리는 것은 그 이름이 아무 집은 줄도 안 가리킬 때뿐이다(사용자 결정, 리뷰 moai-ya06.44t).
    // 못 읽었다고 저장소의 다른 줄까지 "모른다" 로 덮으면 치우지 않은 깨진 워크트리 하나가 경고를
    // 영영 잠재운다.
    let now_s = crate::model::parse_rfc3339(now);
    // **틈은 한 곳에서 잰다**(moai-xn9n). 시각을 못 읽으면 틈을 줄 까닭도 못 재니 안 준다.
    let just_picked = |i: &Issue| match (now_s, crate::model::parse_rfc3339(&i.status_since)) {
        (Some(n), Some(s)) => n - s < STRANDED_GRACE_SECS,
        _ => false,
    };
    let mut out: BTreeMap<String, Place> = found
        .into_iter()
        .map(|(id, at)| {
            // **모름이 방금보다 앞이다**(사용자 결정, 리뷰 moai-ya06.44t). 못 읽은 워크트리가
            // 있는데 "방금 집었다" 고 하면, 이어받는 세션이 "아직 안 뜨었구나" 하고 이미 거기서
            // 도는 일에 둘째 워크트리를 띄운다. 모르는 것은 모른다고 한다.
            let place = match (at.is_empty(), blind, just_picked(picked[id])) {
                (false, ..) => Place::At(at),
                (true, true, _) => Place::Unknown,
                (true, false, true) => Place::Fresh,
                (true, false, false) => Place::Lost,
            };
            (id.to_string(), place)
        })
        .collect();
    let rolled: Vec<(String, Place)> = rolls
        .iter()
        .filter_map(|(g, members)| {
            let mine: Vec<&Place> = members.iter().filter_map(|m| out.get(*m)).collect();
            // 차례는 줄 하나를 가르는 자(위의 `match`)와 같다 — 한 벌 더 적으면 갈려, 에픽이
            // `모른다` 라고 하는데 그 멤버는 `방금 집었다` 라고 한다([`Place::rank`]).
            let best = mine.iter().copied().min_by_key(|p| p.rank())?;
            // 그 묶음 이름의 워크트리(거리 0)는 멤버가 어디 섰든 함께 낸다(moai-t3yj) — 멤버가 다 제
            // 이름 워크트리로 가도 묶음의 자리에서 안 빠진다.
            let ours: &[&Workplace] = named.get(g).map_or(&[], Vec::as_slice);
            let rolled = match best {
                // 여러 멤버가 서로 다른 워크트리에 서 있으면 그 전부를 낸다 — 한 곳만 내면
                // 이어받는 세션이 나머지를 못 본다.
                Place::At(_) => {
                    // **차례는 `trees` 에서 읽는다.** 멤버의 답을 이어 붙이면 차례가 멤버 id 를
                    // 따라가, 같은 워크트리 짝을 `show <일>` 과 `show <에픽>` 이 서로 다른 차례로
                    // 낸다 — 멤버에서는 집합만 받고 줄 세우는 것은 여기서 한 번 한다. 겹친 것도
                    // 이 길에서 한 번씩만 선다(`dedup` 은 잇닿은 것만 걷어내는데, 멤버마다 자리
                    // 차례가 달라 같은 워크트리가 떨어져 두 번 든다).
                    let here: BTreeSet<&std::path::Path> =
                        mine.iter().flat_map(|p| p.at().iter()).chain(ours).map(|t| t.path.as_path()).collect();
                    Place::At(trees.iter().filter(|t| here.contains(t.path.as_path())).collect())
                }
                // 멤버가 아무 데도 안 섰어도 묶음 이름의 워크트리가 있으면 거기다 — 그 자리에 가지와
                // 커밋 안 한 일이 있고, 멤버를 아직 못 집은 세션이 읽는 것이 이 줄이다.
                _ if !ours.is_empty() => {
                    Place::At(trees.iter().filter(|t| ours.iter().any(|o| o.path == t.path)).collect())
                }
                other => (*other).clone(),
            };
            Some(((*g).to_string(), rolled))
        })
        .collect();
    // **집은 줄의 자리는 덮지 않는다** — `rollups` 가 집힌 id 를 이미 뺐다. 같은 id 의 쌍둥이
    // (묶음 줄 하나, 집힌 이슈 하나)가 있으면 그 줄이 제 워크트리에서 찾은 자리가 선다.
    out.extend(rolled);
    out
}

/// 이 줄에 **자리를 물을 수 있는가** — 집은 일이거나, 집은 멤버를 둔 묶음이다([`places`]).
///
/// 부르는 쪽(`cmd/show`)이 워크트리를 읽기 전에 이것으로 판다. 안 집은 줄 하나를 펼치는 흔한 길이
/// 옆 스냅샷을 다 푸는 값을 치르지 않게 하는 문인데, 그 판정은 이슈의 뜻이라 여기 둔다.
///
/// **재료를 든 꼴 하나뿐이다**([`Footing`], moai-rviv) — 곁의 `places`·`blinding`·`stranded` 는
/// 시험이 부르는 짧은 길을 곁에 두지만, 이쪽은 부르는 시험이 없어 `#[cfg(test)]` 짝이 그대로
/// 죽은 코드 경고가 됐다. 시험이 필요해지면 그때 `Footing::of` 를 그 시험에서 부른다.
pub fn placeable_in(footing: &Footing<'_, '_>, i: &Issue) -> bool {
    let picked = footing.picked();
    if picked.contains_key(i.id.as_str()) {
        return true;
    }
    is_group(i) && group_members(footing.all(), i).iter().any(|m| picked.contains_key(m.id.as_str()))
}

/// 방금 집은 줄에 워크트리가 뜰 틈. 규약은 main 에서 집기를 커밋한 **뒤** 워크트리를 띄우므로,
/// 그 사이에 부른 `status`(감독이 배정 전에 부른다)가 "자리 없음" 으로 읽으면 멀쩡히 일을 시작한
/// 세션의 줄을 남에게 다시 준다.
const STRANDED_GRACE_SECS: i64 = 3600;

/// 집었는데 **일하는 워크트리가 없는** 줄(moai-4370). 세션이 죽으면 칸은 `in_progress` 로 남는다.
///
/// - **워크트리를 하나도 안 쓰는 저장소에서는 조용하다** — 거기서는 모든 일이 main 에서 돌아,
///   집은 줄마다 떠든다
/// - 미룬 것은 [`started`] 가 이미 뺐다
/// - **막지 않는다.** 치명이 아니라 종료 코드를 안 바꾼다. 고치는 손은 셋이고 사람이 고른다 —
///   이어 할 것이면 새 워크트리를 띄우고, 놓을 것이면 `todo` 로, 지금 안 할 것이면 미룬다
/// - **`status()` 에 안 넣는다.** 훅의 기준선과 `Stop` 이 `status()` 의 경고를 세는데, 이것은 남의
///   세션이 워크트리를 치우는 것만으로 늘어 제 일과 상관없이 세션을 붙든다. `moai status` 만 싣는다
// 바이너리는 재료를 든 `_in` 을 부른다([`Footing`], moai-rviv) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn stranded(issues: &[Issue], cfg: &Config, trees: &[Workplace], now: &str) -> Option<Warning> {
    stranded_in(&Footing::of(issues, cfg), trees, now)
}

/// [`stranded`] 와 같은 것. **이미 잰 재료를 받는다**([`Footing`]) — `worktree::stranded_at` 은
/// 스냅샷을 팔지 고르는 문(`workplaces`)에서 같은 줄을 이미 셌다(moai-rviv).
pub fn stranded_in(footing: &Footing<'_, '_>, trees: &[Workplace], now: &str) -> Option<Warning> {
    // 워크트리가 없으면 [`places`] 가 아무 키도 안 낸다 — 가드를 여기 다시 적지 않는다(moai-tbin).
    let at = places_in(footing, trees, now);
    // 자리 잃은 것이 없으면 되짚을 것도 없다 — [`started`] 를 한 벌 더 세지 않는다(`places` 가 방금 셌다).
    if !at.values().any(|p| matches!(p, Place::Lost)) {
        return None;
    }
    // **집은 줄로 되짚는다.** [`places`] 는 묶음 id 에도 키를 주므로(멤버에서 굴려 올린다) 그것을
    // 걸러야 하는데, **줄 전체로 되짚으면 안 된다** — 그 지도는 뒷줄이 이기니 같은 id 의 묶음
    // 쌍둥이가 앞줄의 집힌 이슈를 가리고, 그것을 종류로 거르면 **자리를 잃은 산 줄이 조용히
    // 빠진다**(머지가 남긴 `duplicate_id` 하나로 그 줄이 영영 안 보인다). [`started`] 로 되짚으면
    // 묶음 id 는 애초에 없어 거를 것도 없다 — 거르는 자가 하나다. `wip` 이 아니다: 그쪽은 가려진
    // 줄을 뺀다(moai-es40). 되짚는 그 집합이 [`places`] 가 방금 센 바로 그것이다([`Footing::picked`]).
    let picked = footing.picked();
    // **`Lost` 만 센다.** 방금 집은 것(`Fresh`)과 못 읽은 워크트리가 있어 모르는 것(`Unknown`)은
    // 자리 없음이 아니다 — 둘을 세면 산 일을 남에게 다시 주는 쪽으로 틀린다.
    let lost: Vec<&Issue> = at
        .iter()
        .filter(|(_, p)| matches!(p, Place::Lost))
        .filter_map(|(id, _)| picked.get(id.as_str()).copied())
        .collect();
    // **나이는 안 싣는다**(`Warning::ages`). 저것은 **날**로 재는데 이 판정의 문턱은 한 시간이라,
    // 하루가 안 된 것에 다 `0일` 이 붙어 "방금 집었다" 로 읽힌다. 안 실으면 보는 쪽이 칸 나이를
    // 내는데 그 값이 똑같으므로 화면은 그대로고, `--json` 에서 속이는 키 하나가 준다.
    (!lost.is_empty())
        .then(|| Warning::new("stranded", ids_of(&lost)).hint("moai mv <id> todo  ·  moai defer <id> -m '<why>'"))
}

/// 같은 id 를 쓰는 줄이 **둘 이상이면** 그 수. 하나뿐이면 `None`.
///
/// 상세는 뒷줄을 연다(`store::Load::get`). 앞줄을 말없이 가리면 깨진 파일을 보는
/// 사람이 제 줄이 사라진 줄 알므로, 펼친 자리에서 한 줄로 드러낸다 — `status` 의
/// `duplicate_id` 와 같은 사실을 그 id 하나에 대해 말하는 것이다(moai-e0ro).
pub fn duplicate_lines(issues: &[Issue], id: &str) -> Option<usize> {
    Some(issues.iter().filter(|i| i.id == id).count()).filter(|n| *n > 1)
}

/// `id` 의 직계 자식. 부모는 id 에서 유도되므로 접두 검사면 된다.
///
/// **차례는 목록과 같다.** 상세 한 화면에서 자식 줄은 id 순, 그 아래 멤버 줄은
/// 우선순위 순이면 규칙이 둘이 되어 보는 쪽이 어느 쪽도 못 믿는다.
pub fn children_of<'a>(issues: &'a [Issue], id: &str) -> Vec<&'a Issue> {
    let mut out: Vec<&Issue> = issues.iter().filter(|c| crate::id::parent_of(&c.id) == Some(id)).collect();
    out.sort_by(|a, b| crate::query::display_order(a, b));
    out
}

/// 상세가 **자식 줄로 그릴** 것 — [`children_of`] 에서 그 묶음의 멤버를 뺀 것.
///
/// 계획이 세우는 멤버가 에픽의 자식 id 를 받으면서(moai-s8go) 같은 줄이 머리의 `자식` 줄과
/// 밑의 멤버 칸에 두 번 섰다 — 멤버 일곱짜리 에픽을 펼치면 같은 일곱 줄이 두 벌이다. 멤버
/// 칸이 칸·우선순위·태그까지 말하니 자식 줄이 더 주는 것이 없고, 멤버가 아닌 자식(담아 둔
/// 생각 등)은 그대로 선다(moai-vndz, 2026-09-23 사용자 결정).
///
/// **`&[Issue]` 에 대한 순수 함수로 여기 둔다** — `view::detail` 을 그리는 표면이 둘이라
/// (`cmd::show`·`cmd::edit`) 한쪽에만 두면 같은 에픽을 두 명령이 다르게 낸다. 실제로 그랬다:
/// `moai show <에픽>` 은 걸렀는데 `moai edit <에픽>` 은 안 걸렀다(리뷰).
///
/// 뺀 줄은 **부르는 쪽이 반드시 다른 자리에 그린다.** 한 화면에서 말없이 사라지는 줄이 있으면
/// 이것은 고침이 아니라 가림이다.
///
/// **id 가 아니라 줄로 가른다**(리뷰). 같은 id 를 든 줄이 둘일 수 있고([`duplicate_lines`]),
/// 그때 [`group_members`] 는 이긴 줄 하나만 멤버로 낸다 — id 로 거르면 그 판정이 진 줄에도
/// 옮아붙어, 멤버도 자식도 아닌 채 상세에서 통째로 사라진다. 깨진 자료는 숨기지 않고
/// `duplicate_id` 가 따로 드러낸다는 것이 이 도구의 약속이다(`nav` 가 가려진 앞줄을 잎으로
/// 남기는 것과 같은 까닭). 두 목록 다 같은 `all` 을 훑어 얻은 참조라 줄 비교가 정확하다.
pub fn kin_of<'a>(children: &[&'a Issue], members: &[&Issue]) -> Vec<&'a Issue> {
    let mine: BTreeSet<*const Issue> = members.iter().map(|m| std::ptr::from_ref::<Issue>(m)).collect();
    children.iter().copied().filter(|c| !mine.contains(&std::ptr::from_ref::<Issue>(c))).collect()
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
    // **묶음이 아니면 지도도 안 짓는다** — `eclipsed` 는 줄 전체로 지도를 하나 짓는데, 일 하나를
    // 펼치는 흔한 길(`cmd/show`·`placeable`)은 여기서 곧바로 돌아선다.
    if !is_group(group) {
        return Vec::new();
    }
    // **종류 지도는 한 벌이다** — 가려짐 판정과 [`Placed`] 가 같은 것을 본다.
    let kind_of = kinds(all);
    let eclipsed = |i: &Issue| is_eclipsed(&kind_of, i);
    // 종류가 다른 쌍둥이에게 id 가 가려진 묶음 줄도 멤버가 없다 — 그 id 를 가리키는
    // 줄은 쌍둥이의 것이다([`is_eclipsed`]).
    if eclipsed(group) {
        return Vec::new();
    }
    // **소속은 [`Placed`] 에 묻는다**(moai-7iyc.rt6) — 지도를 곧바로 짚으면 같은 id 를 든
    // 앞줄이 뒷줄의 에픽을 입어, 머리글의 롤업(`0/1`)과 그 밑에 그려지는 줄 수가 갈린다.
    //
    // **[`Lines`] 는 그 축에서만 짓는다** — 에픽 굴림은 그것을 안 보므로([`Placed::at`]),
    // 에픽 하나를 펼치는 흔한 길이 한 벌을 안 치른다. **빈 것을 끼워 넣지 않는다**:
    // 갈래가 재료를 안 들면 [`Placed::Milestone`] 은 아예 못 선다(리뷰 moai-7iyc.cmo 15번).
    let epic_of = groups(all);
    let lines = match group.kind {
        Kind::Epic => None,
        _ => Some(Lines::of(all)),
    };
    let placed = match &lines {
        Some(lines) => Placed::milestone(&epic_of, lines),
        None => Placed::epic(&epic_of),
    };
    let mut out: Vec<&Issue> = all
        .iter()
        .filter(|i| i.id != group.id && holds(i.kind) && !eclipsed(i))
        .filter(|i| placed.at(i) == Some(group.id.as_str()))
        .collect();
    out.sort_by(|a, b| crate::query::display_order(a, b));
    out
}

/// 묶음에 **든 시간** — 닫힌 멤버의 `started_at`·`done_at` 에서 지금 센다(moai-wfup).
///
/// **저장하지 않는다.** 마일스톤 줄에 총합을 적어 두면 멤버를 하나 닫을 때마다 남의 줄을 써야
/// 하고, 그것이 곧 파생값을 저장하는 것이다 — beads 가 `is_blocked` 를 컬럼으로 들고 있다가
/// `bd recompute-blocked` 를 만들어야 했던 자리다.
///
/// **`closed` 와 `measured` 를 따로 낸다.** `started_at` 이 없으면 **모르는 것**이고(moai-38mh),
/// 0 분으로도 안 한 일로도 세지 않는다 — 어림값에 그 수를 함께 대야 빠진 멤버가 조용히 0 이
/// 안 된다("닫힌 41 중 34 를 잰 값"). 한 수로 뭉치면 절반만 잰 마일스톤이 절반만 일한 것으로
/// 읽힌다.
///
/// **묶음 멤버는 안 센다**(`is_work`). 에픽 줄의 두 값은 "누가 그 줄에 `mv` 를 쳤나" 일 뿐이고
/// (`Issue::started_at`), 그 에픽의 이슈는 마일스톤 지도가 물려받아 이미 여기 들어 있다 — 함께
/// 세면 같은 일이 두 번 더해진다.
///
/// **멤버의 자식은 시간을 두 번 안 싣는다**(리뷰). `is_work` 는 종류만 보는데 자식도
/// `Kind::Issue` 라, 소속 지도가 부모를 타고 자식까지 이 묶음에 넣는다 — 그리고 이 저장소의
/// 규약이 리뷰를 자식으로 세우므로(`AGENTS.md` 의 훅 규칙 3) 그 겹침은 드문 판이 아니라 흔한
/// 판이다. 자식의 시작·끝은 부모의 구간 **안**에 들어 있어, 둘을 같이 더하면 한 시간 일한 것이
/// 두 시간으로 선다.
///
/// **접는 것은 부모의 구간이 실제로 세졌을 때뿐이다.** 부모가 아직 안 닫혔거나 시작이 안 적혀
/// 아무 구간도 못 냈으면 그 자식이 이 묶음에 있는 유일한 증거고, 그때까지 접으면 리뷰만 닫힌
/// 마일스톤이 "잰 것이 없다" 로 선다. **`closed` 는 접지 않는다** — 자식도 닫힌 일이고, 그 수는
/// 막대(`Roll::done`)와 같아야 한 화면이 두 수로 말하지 않는다.
///
/// **벽시계는 든 품이 아니다.** 세션 여럿이 같이 도는 저장소라 겹친 시간이 이중으로 세지고,
/// 사람 답을 기다린 시간과 리뷰가 돈 시간이 다 들어 있다. 내는 쪽이 그 말을 함께 적는다.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Spent {
    /// 닫힌 멤버 수 — `done` 에 선 일.
    pub closed: usize,
    /// 그중 시작과 끝이 **둘 다 적힌** 멤버 수. 분모가 아니라 "몇을 쟀나" 다.
    pub measured: usize,
    /// 잰 멤버의 소요를 더한 값(분).
    pub minutes: i64,
    /// 잰 멤버 소요의 중앙값(분). 잰 것이 없으면 없다.
    ///
    /// **평균이 아니다**(moai-wfup 의 실측) — 사분위 폭이 중앙값보다 넓어 한쪽으로 긴 꼬리가
    /// 평균을 끌고 간다. 중앙값도 어림이라, 읽는 쪽에 잰 수를 함께 준다.
    pub median: Option<i64>,
}

/// [`Spent`] 를 센다. 묶음이 아닌 줄에는 빈 값이다.
// 바이너리는 멤버를 이미 쥔 [`spent_in`] 을 부른다 — 이 꼴은 시험의 짧은 길이다
// ([`misplaced`]·`rollup_of` 와 같은 자리).
#[cfg(test)]
pub fn spent_on(all: &[Issue], group: &Issue) -> Spent {
    spent_in(&group_members(all, group))
}

/// [`spent_on`] 과 같은 것. 멤버를 이미 쥔 쪽([`crate::cmd`] 의 `show`)이 소속 지도를 다시 짓지
/// 않게 받는다 — `group_stands_in`·`rollup_of_in` 과 같은 꼴이다(moai-g0zx). `group_members` 는
/// 저장소 전체로 `eclipsed` 와 소속 지도를 새로 짓는데, 상세 한 판은 그것을 이미 쥐고 있다.
pub fn spent_in(members: &[&Issue]) -> Spent {
    // **닫힌 멤버는 다 센다** — 자식도 닫힌 일이고, 막대(`Roll::done`)가 세는 것과 같아야 한다.
    // 자식을 여기서도 빼면 상세가 `멤버 2/2` 옆에 "닫힌 1" 을 내, 한 화면이 두 수로 말한다(리뷰).
    // 접는 것은 **시간**뿐이다.
    //
    // **조상이 먼저 서야 한다.** id 는 부모가 제 자식의 앞머리라(`<부모>.<자식>`) 오름차순이면
    // 조상이 언제나 앞이고, 그래야 자식이 제 부모의 판정을 보고 정한다.
    let mut done: Vec<&&Issue> = members.iter().filter(|m| is_work(m) && m.status.is_done()).collect();
    done.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    let mut spans: Vec<i64> = Vec::new();
    // 제 구간이 이미 세진 줄 — 그 밑은 그 구간 안이다.
    let mut inside: BTreeSet<&str> = BTreeSet::new();
    for m in &done {
        // **부모의 구간이 실제로 세졌을 때만 접는다**(리뷰). 부모가 아직 안 닫혔거나 시작이
        // 안 적혀 아무것도 못 냈으면, 그 자식의 구간이 이 묶음에 있는 **유일한 증거**다 —
        // 무턱대고 접으면 리뷰만 닫힌 마일스톤이 "잰 것이 없다" 로 서고, 실제로 그랬다.
        if crate::id::parent_of(&m.id).is_some_and(|p| inside.contains(p)) {
            // 손자도 같은 구간 안이다 — 접힌 자식을 타고 이어진다.
            inside.insert(m.id.as_str());
            continue;
        }
        // **음수는 안 센다.** 손으로 푼 줄이나 되돌렸다 다시 닫은 줄에서 끝이 시작보다 앞설 수
        // 있는데, 그런 줄을 0 으로 눌러 담으면 "0분에 했다" 가 잰 값으로 선다 — 모르는 것은
        // 모른다고 한다.
        let (Some(s), Some(d)) = (m.started_at.as_deref(), m.done_at.as_deref()) else { continue };
        let (Some(s), Some(d)) = (crate::model::parse_rfc3339(s), crate::model::parse_rfc3339(d)) else { continue };
        if d < s {
            continue;
        }
        spans.push((d - s) / 60);
        inside.insert(m.id.as_str());
    }
    let closed = done.len();
    spans.sort_unstable();
    // 짝수 개면 가운데 둘의 평균이다 — 내림으로 자른다(분 단위라 한 분 아래는 뜻이 없다).
    let median = match spans.len() {
        0 => None,
        n if n % 2 == 1 => Some(spans[n / 2]),
        n => Some((spans[n / 2 - 1] + spans[n / 2]) / 2),
    };
    Spent { closed, measured: spans.len(), minutes: spans.iter().sum(), median }
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
///   아니면 **시작한 멤버가 선 칸 중 설정 차례로 가장 앞 칸**(moai-p415) — 묶음은 제일
///   덜 간 일만큼 가 있다. 시작한 멤버가 없으면(끝난 것과 첫 칸뿐) 설정의 첫 시작 칸
///   (`Config::started_status`)이다. 칸 자리로만 고르면 칸이 더 있는 설정
///   (`todo,blocked,in_progress,review,done`)에서 멤버가 in_progress 인 에픽이 `blocked` 로 읽혔다
pub fn group_states<'a, 'c>(all: &'a [Issue], cfg: &'c Config) -> BTreeMap<&'a str, &'c str> {
    group_stands(all, cfg).into_iter().map(|(id, s)| (id, s.column)).collect()
}

/// [`group_states`] 와 같은 셈에서 **칸과 곁들이([`Stand`])를 함께** 낸다. 소속 지도를
/// 따로 안 든 쪽이 부른다 — 든 쪽(탐색기의 적재)은 [`Soil::stands`] 다.
pub fn group_stands<'a, 'c>(all: &'a [Issue], cfg: &'c Config) -> BTreeMap<&'a str, Stand<'a, 'c>> {
    let lines = Lines::of(all);
    let epic_of = groups_over(all, &lines);
    let mile_of = milestones_over(all, &epic_of, &lines);
    let shelved = shelved_over(all, &lines, &epic_of, &mile_of);
    group_stands_in(all, cfg, &lines, &epic_of, &mile_of, &kinds(all), &shelved)
}

/// [`group_states`] 를 **그 줄들 가운데 묶음이 있을 때만** 센다.
///
/// 하나를 펼치거나 쓰는 표면(`show <id>`·`add`·`edit`·`mv`)은 대개 일 하나를 보는데,
/// 그때 묶음 지도와 미룸 걷기는 통째로 헛일이다 — 10k 줄에서 `moai show <이슈>` 가
/// 그것만으로 갑절이 됐다.
pub fn group_states_of<'a, 'c>(all: &'a [Issue], cfg: &'c Config, ids: &[&str]) -> BTreeMap<&'a str, &'c str> {
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
pub fn column<'x>(kind_of: &Kinds<'_>, i: &'x Issue, states: &BTreeMap<&str, &'x str>) -> &'x str {
    stands_on(kind_of, i, || states.get(i.id.as_str()).copied()).unwrap_or(i.status.as_str())
}

/// 줄 하나가 입는 **읽은 칸** — `--json` 의 `derived_status` 가 내는 그 값이다. 묶음이
/// 아니거나 못 받았으면 없다.
///
/// **자는 하나다** — [`stands_in`] 이 소속에 대해 그런 것과 같다. 칸을 내는 자([`column()`])와
/// `--json` 의 키를 다는 자([`crate::cmd::Row::of`])가 저마다 `if` 를 적으면 하나는 반드시
/// 빠진다.
///
/// **가려진 묶음 줄은 칸을 안 입는다**(moai-7iyc.5fz). 칸 지도는 id 로 짠 것이라 마일스톤으로
/// 한 번, 에픽으로 한 번 선 id 에서는 뒷줄 — 그 id 의 뜻을 정하는 줄([`is_eclipsed`]) — 의 칸
/// 하나만 든다. 그것을 앞줄에도 입히면 `show --json` 이 가려진 마일스톤 줄에
/// `derived_status: in_progress` 를 달고 `show -s in_progress` 가 그 줄을 고르는데, 같은 창의
/// 트리는 그 줄을 `(길 잃음)` 에 제 `todo` 로 그린다 — 한 화면이 같은 줄을 두 칸으로 말한다.
/// **문이 [`crate::cmd::Row::of`] 가 아니라 여기 선다**: 고르는 자(`-s`)와 칸을 내는 자가 같은
/// 자리에서 갈려야 둘이 안 어긋난다.
///
/// **든 것이 무엇이든 같은 문을 지난다**(리뷰) — 탐색기는 칸 하나가 아니라 그 칸의 셈
/// ([`Stand`])을 통째로 짚으므로, `&str` 로 못박아 두면 `도는가`·`기다림`·`미룬 수` 셋이 이 문을
/// 못 지나 저마다 `is_group` 을 다시 적는다. 실제로 그 셋만 가려진 줄에 쌍둥이의 값을 냈다.
///
/// **지도는 안 짚고 받는다**(리뷰) — 묶음이 아닌 줄에서 곧바로 물러나는 자라, 값으로 받으면 그
/// 흔한 갈래에서도 짚기가 먼저 돈다. 고친 [`joined_in`] 과 같은 꼴이고, 이 고침 앞의
/// `is_group(i).then(|| states.get(…))` 이 원래 그랬다.
pub fn stands_on<T>(kind_of: &Kinds<'_>, i: &Issue, read: impl FnOnce() -> Option<T>) -> Option<T> {
    (is_group(i) && !kind_of.eclipses(i)).then(read).flatten()
}

/// 묶음 하나를 읽은 것 — 서 있는 칸과, 그 칸의 셈이 마지막으로 움직인 때.
/// **저장하지 않는다** ([`group_states`] 가 까닭을 적었다).
#[derive(Debug, Clone, PartialEq)]
pub struct Stand<'a, 'c> {
    /// 서 있는 칸.
    pub column: &'c str,
    /// 셀 멤버 가운데 **칸을 가장 늦게 옮긴 때**(멤버의 `status_since`). 셀 멤버가 없으면
    /// 묶음이 생긴 때다. **묶음 제 줄에 적힌 `status_since` 는 안 쓴다** — 아무 데서도 안
    /// 읽히는 칸의 시각이라, `--stale` 이 그것으로 재면 오늘 진행 중이 된 에픽을 "열흘째
    /// 멈춰 있다" 고 한다.
    ///
    /// **미루거나 도로 집은 때(`planned_at`)는 안 센다**(moai-cxk8). 이 시각은 방치와 막힘을
    /// 재는 시계라, 세면 멤버를 `defer`→`--undo` 하는 것만으로 에픽의 `--stale` 과 그 에픽에
    /// 막힌 줄의 `blocked_stale` 이 새로 선다 — 미루기가 경고를 지우는 손잡이가 된다. 미뤄
    /// 둔 동안에도 막음은 이어졌다(미룬 막음도 막는다, moai-2sea).
    ///
    /// **소속을 옮긴 때도 안 센다**(moai-bbzg) — 같은 까닭이다. `edit -e` 로 들어온 옛 멤버는 제
    /// 옛 칸 시각으로 세므로, 끝난 묶음이 그것으로 다시 열려도 이 시각은 안 움직인다.
    pub since: &'a str,
    /// 셀 멤버 가운데 **적힌 칸이 시작한 칸**(`Config::is_started` — 첫 칸도 done 도 아님)인 일이 있는가 —
    /// 지금 누가 그 묶음 밑에서 손대고 있다는 말.
    ///
    /// 읽은 칸만으로는 이 말을 못 한다. 묶음은 멤버 하나가 끝나고 나머지가 첫 칸이기만
    /// 해도 시작한 칸으로 읽힌다(`column_of`) — 막대로는 "반쯤 했다" 지 "하는 중" 이 아니다.
    /// 탐색기가 묶음을 돌릴지를 이것으로 가른다(moai-x5eg).
    ///
    /// **셀 멤버에서만 찾는다.** 미뤄 칸 셈에서 빠진 멤버가 `in_progress` 에 있어도
    /// 묶음은 그것으로 안 바쁘다 — 칸이 그 멤버를 안 세는데 곁들이만 세면, 한 줄이
    /// `todo` 로 서면서 도는 글리프를 낸다. 시작한 칸이 첫 칸과 같은 두 칸짜리 설정에서는
    /// 시작했다는 말 자체가 없으므로 언제나 거짓이다.
    pub busy: bool,
    /// 이 묶음이 막을 때 **무엇을 기다리는가**([`Waiting`]). 칸만으로는 모른다.
    pub waiting: Waiting,
    /// [`Waiting::Shelved`] 일 때 **칸 셈에서 미뤄 뺀 안 끝난 멤버** — 그 밖에는 비었다.
    ///
    /// 칸은 "지금 할 것이 남았나" 를 말하므로 뺀 멤버를 안 센다. 그런데 그 칸으로 막음을
    /// 풀면, 같은 미룬 멤버에 곧장 막힌 줄은 held 로 서고 묶음 너머로 막힌 줄은 `ready`
    /// 에 선다(moai-0gxf). 미룬 일은 끝난 일이 아니다 — 막음은 이것으로 그 멤버를 댄다
    /// ([`blocker`]).
    pub aside: Vec<&'a str>,
    /// 셀 멤버 가운데 끝난 것의 몫(0~100). 셀 멤버가 없으면 `None`.
    ///
    /// **칸과 같은 자다** — 미룬 멤버를 뺀다. 롤업의 막대(`Roll::percent`)는 "계획 중 얼마나
    /// 했나" 라 미룬 멤버도 세지만, `ready` 가 끝나가는 에픽을 먼저 세울 때 묻는 것은
    /// "몇 번 더 집으면 닫히나" 다. 막대로 재면 한 번에 닫히는 에픽이 뒤에 섰다(moai-ha03).
    pub progress: Option<u8>,
    /// 멤버 가운데 **칸 셈에서 미뤄 뺀** 수 — `of.len() - counted.len()`(moai-zxwj).
    ///
    /// 막대(`Roll::percent`)의 분모는 미룬 멤버를 그대로 세는데(2026-09-21 사용자 결정), 그러면
    /// `102/110` 의 남은 여덟 가운데 넷은 아무도 집을 일이 아니라는 것이 화면에 없다. 그 수를
    /// 대는 자가 이것이고, `Roll::deferred` 가 여기서 온다.
    ///
    /// **[`Stand::aside`] 와 다른 것을 센다.** 저쪽은 센 멤버가 다 끝났을 때만 서는 목록이라
    /// ([`Waiting::Shelved`]) 아직 굴러가는 묶음에서는 미룬 멤버가 있어도 비어 있다. 막대 곁에
    /// 댈 수는 언제나 서야 하므로 여기서 따로 센다.
    ///
    /// **제가 계획 밖인 묶음은 0 이다.** 그 밑은 통째로 계획 밖이라 셀 것이 따로 없고, 그 말은
    /// 묶음 제 줄이 `put_off` 로 이미 한다 — `0/N  미룬 N` 으로 제 미룸을 멤버의 미룸인 양 두
    /// 번 말하지 않는다.
    ///
    /// **[`counted`] 의 차만으로는 모자라다**(리뷰). 그쪽은 묶음 제 미룸으로 빠진 멤버만 도로
    /// 세므로, 제 줄도 미뤘고 **멤버 하나가 따로 또 미뤄진** 판에서는 그 하나만 남아 `0/2
    /// 미룬 1  미룸` 이 선다 — 둘 다 계획 밖인데 화면은 하나가 아직 집을 것이라고 말한다.
    /// 그래서 셈 앞에서 묶음 제 미룸을 먼저 가른다.
    pub deferred: usize,
}

/// 묶음이 막을 때 기다리는 것.
///
/// 묶음의 칸은 "지금 할 것이 남았나" 를 말하므로, 막음을 가르는 데는 모자란다 — 칸이
/// `done` 이어도 미룬 멤버를 기다릴 수 있고(moai-0gxf), 첫 칸이어도 기다릴 일이 하나도
/// 없을 수 있다(moai-1c2l). 둘 다 **막되 까닭을 댄다**([`held`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waiting {
    /// 제 칸대로다 — 센 멤버에 안 끝난 일이 있거나, 전부 끝났다.
    Live,
    /// 센 멤버에 안 끝난 일이 없는데 미뤄 뺀 안 끝난 멤버가 있다([`Stand::aside`]).
    Shelved,
    /// 멤버가 하나도 없다 — 채우기 전에는 영영 안 풀린다.
    Nothing,
}

/// 멤버(`of`)와 칸 셈에 든 멤버(`counted`)로 [`Waiting`] 과 뺀 안 끝난 멤버를 정한다.
fn waiting_in<'a>(of: &[&'a Issue], counted: &[&'a Issue]) -> (Waiting, Vec<&'a str>) {
    if of.is_empty() {
        return (Waiting::Nothing, Vec::new());
    }
    if counted.iter().any(|m| !m.status.is_done()) {
        return (Waiting::Live, Vec::new());
    }
    // 센 멤버가 전부 끝났거나 없다 — 안 끝난 멤버는 곧 미뤄 뺀 멤버다.
    let aside: Vec<&str> = of.iter().filter(|m| !m.status.is_done()).map(|m| m.id.as_str()).collect();
    let waiting = if aside.is_empty() { Waiting::Live } else { Waiting::Shelved };
    (waiting, aside)
}

/// 제 칸대로가 아닌 묶음 → 기다리는 것과 뺀 멤버. `Live` 인 묶음은 안 든다.
pub type Waits<'a> = BTreeMap<&'a str, (Waiting, Vec<&'a str>)>;

fn waiting_of(id: &str, waits: &Waits) -> Waiting {
    waits.get(id).map_or(Waiting::Live, |w| w.0)
}

/// [`group_states_in`] 과 같은 한 번의 셈에서 칸과 시각을 함께 낸다.
pub fn group_stands_in<'a, 'c>(
    all: &'a [Issue],
    cfg: &'c Config,
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    mile_of: &BTreeMap<&'a str, &'a str>,
    kind_of: &BTreeMap<&'a str, Kind>,
    shelved: &[Option<&'a str>],
) -> BTreeMap<&'a str, Stand<'a, 'c>> {
    let members = members_in(all, epic_of, lines, kind_of);
    // 미룬 줄이 없으면 뺄 멤버도 없다 — 조상을 타는 길을 아예 안 세운다. **줄마다의 답으로
    // 묻는다**(moai-u3ta) — 접은 지도로 묻던 때는 그 지도가 뒷줄의 답만 들어, 미룬 에픽에 든
    // 앞줄 하나뿐인 저장소에서 비어 버렸다. 그러면 여기가 길을 안 세우고 그 줄이 멤버 셈에
    // 그대로 든다. 목록을 한 번 더 훑지 않는다(리뷰 moai-jk2u.hr4) — 이 집합이 곧 그 답이다.
    let off = off_rows(all, shelved);
    let shelf = (!off.is_empty()).then(|| Shelf::new(lines, epic_of, mile_of));
    all.iter()
        .filter(|g| is_group(g))
        .map(|g| {
            let counted = counted(g, &members, shelf.as_ref(), &off);
            let since = counted.iter().map(|m| m.status_since.as_str()).max().unwrap_or(g.created_at.as_str());
            // 칸 자리가 아니라 뜻으로 묻는다(moai-p415) — 두 칸짜리 설정에는 시작한 칸이 없어 거짓이다.
            // **설정이 아는 칸만 센다** — `column_of` 가 설정의 칸에서만 고르므로, 모르는 칸
            // 멤버로 바쁘다고 하면 묶음은 대신 선 시작 칸에서 돌고 그 밑에 도는 줄은 없다.
            let busy = counted.iter().any(|m| {
                let s = m.status.as_str();
                cfg.knows(s) && cfg.is_started(s)
            });
            let column = column_of(&counted, cfg);
            let of = members.get(&(g.kind, g.id.as_str())).map(Vec::as_slice).unwrap_or_default();
            let (waiting, aside) = waiting_in(of, &counted);
            let finished = counted.iter().filter(|m| m.status.is_done()).count();
            let progress = (!counted.is_empty()).then(|| (finished * 100 / counted.len()) as u8);
            // **막대가 세는 것에서 칸이 세는 것을 뺀 것**이다(moai-zxwj) — 두 셈이 이미 여기
            // 나란히 서 있으므로 새로 걷는 걸음이 없다. 둘 중 하나만 고치는 날 이 수가 0 이 되어
            // 화면이 먼저 말한다.
            //
            // **제가 계획 밖인 묶음은 세지 않는다**(리뷰). `counted` 는 묶음 제 미룸으로 빠진
            // 멤버를 도로 세므로, 제 줄도 미뤘고 멤버 하나가 따로 또 미뤄진 판에서는 그 차가
            // 1 이다 — 둘 다 계획 밖인데 `0/2  미룬 1  미룸` 이 서서 하나는 아직 집을 것이라고
            // 말한다. 그 줄이 할 말은 `put_off` 하나다.
            //
            // **이것도 줄에 묻는다**(리뷰 moai-jk2u.hr4) — 접은 지도로 묻던 때는 같은 id 의 산
            // 묶음 줄이 미룬 줄의 답을 지워, 계획 밖인 앞줄이 `0/2  미룬 1  미룸` 을 내며 아직
            // 집을 것이 있다고 말했다.
            let deferred = match off.contains(&std::ptr::from_ref(g)) {
                true => 0,
                false => of.len() - counted.len(),
            };
            (g.id.as_str(), Stand { column, since, busy, waiting, aside, progress, deferred })
        })
        .collect()
}

/// 막음을 가르는 데 드는 묶음 쪽 재료 — 읽은 칸과, 제 칸대로가 아닌 묶음이 기다리는
/// 것([`Waits`]). 한 번의 셈에서 둘로 가른다.
fn split_stands<'a, 'c>(stands: BTreeMap<&'a str, Stand<'a, 'c>>) -> (BTreeMap<&'a str, &'c str>, Waits<'a>) {
    let mut states = BTreeMap::new();
    let mut waits = BTreeMap::new();
    for (id, s) in stands {
        states.insert(id, s.column);
        if s.waiting != Waiting::Live {
            waits.insert(id, (s.waiting, s.aside));
        }
    }
    (states, waits)
}

/// (종류, 묶음 id) → 그 묶음의 일. **롤업과 같은 자다** — 물려받은 소속까지, 일만.
/// 목록을 종류마다 한 번 걷는다 — 묶음마다 걸으면 제곱이다. **종류로 가른다** — 에픽 지도가
/// 마일스톤 id 를 가리키는 틀린 참조를 마일스톤의 멤버로 세면 `rollup_of` 와 어긋난다.
fn members_in<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    lines: &Lines<'a>,
    kind_of: &BTreeMap<&'a str, Kind>,
) -> BTreeMap<(Kind, &'a str), Vec<&'a Issue>> {
    let eclipsed = |i: &Issue| is_eclipsed(kind_of, i);
    let mut members = BTreeMap::new();
    for placed in [Placed::epic(epic_of), Placed::milestone(epic_of, lines)] {
        for (g, of) in work_under(all, &placed, &eclipsed) {
            members.insert((placed.kind(), g), of);
        }
    }
    members
}

/// 묶음 id → [`Placed`] 가 그 묶음에 두는 **일**. [`members_in`] 과 [`rollup_of`] 가 같은
/// 걸음을 쓴다(moai-isyy) — 롤업이 묶음마다 목록을 다시 걸으면 에픽 E 개에 줄 N 개가 E×N 이고,
/// 1만 줄에서 `moai status` 의 대부분이 거기 들었다. 셈의 자도 하나가 된다: 일만, 가려진 줄은 빼고.
///
/// **소속은 [`Placed::at`] 에 묻는다** — 지도를 곧바로 짚으면 같은 id 를 든 앞줄이 뒷줄의
/// 에픽을 입어, `derived_epic` 이 내는 답과 갈린다(moai-7iyc.rt6).
fn work_under<'a>(
    all: &'a [Issue],
    placed: &Placed<'_, 'a>,
    eclipsed: &impl Fn(&Issue) -> bool,
) -> BTreeMap<&'a str, Vec<&'a Issue>> {
    let mut out: BTreeMap<&str, Vec<&Issue>> = BTreeMap::new();
    for i in all.iter().filter(|i| is_work(i) && !eclipsed(i)) {
        if let Some(g) = placed.at(i) {
            out.entry(g).or_default().push(i);
        }
    }
    out
}

/// 묶음의 칸을 셀 멤버 — 미룬 멤버는 빼되, **묶음 제가 받은 미룸으로는 안 뺀다.**
///
/// 멤버를 빼는 미룸을 **전부** 본다(`Shelf::every`). 가까운 하나만 보면, 부모를 미뤄
/// 빠진 자식이 제 에픽을 미루는 순간 그 에픽의 미룸을 뿌리로 받아 도로 세어진다 —
/// 미루기 하나로 에픽의 칸이 `done` 에서 `in_progress` 로 되돌아갔다.
///
/// **끝난 멤버는 언제나 센다.** 끝난 일은 물려받아도 계획에서 안 빠진다(`closed_by_hand`).
///
/// **멤버가 계획 밖인가는 줄에 묻는다**(moai-u3ta). 접은 지도(`roots`)를 짚어 빠른 길을
/// 삼던 때는 같은 id 의 앞줄이 뒷줄의 답을 입어, 미룬 에픽에 든 앞줄이 셈에 그대로 들거나
/// 산 에픽에 든 뒷줄이 셈에서 빠졌다.
///
/// **값싼 문은 그대로 둔다**([`off_rows`], 리뷰 moai-jk2u.hr4) — 그 문을 `Shelf::nearest` 로
/// 바꾸던 때는 멤버마다 조상을 탔고, 그 값은 미룬 줄 수가 아니라 **멤버 수**에 붙었다. 바늘로
/// 짚으면 줄마다의 답을 그대로 쓰면서 걸음이 하나도 안 는다. `closed_by_hand` 를 따로 안
/// 묻는 것도 그래서다 — 끝난 줄은 [`shelved_over`] 가 이미 `None` 으로 내어 이 집합에 없다.
fn counted<'a>(
    g: &'a Issue,
    members: &BTreeMap<(Kind, &'a str), Vec<&'a Issue>>,
    shelf: Option<&Shelf<'a, '_>>,
    off: &BTreeSet<*const Issue>,
) -> Vec<&'a Issue> {
    let of = members.get(&(g.kind, g.id.as_str())).map(Vec::as_slice).unwrap_or_default();
    let Some(shelf) = shelf else { return of.to_vec() };
    // 묶음을 계획에서 뺀 것들 — 제 미룸과 마일스톤·부모에게서 물려받은 것.
    let mine = shelf.every(g);
    of.iter()
        .copied()
        .filter(|m| !off.contains(&std::ptr::from_ref(*m)) || shelf.every(m).iter().all(|s| mine.contains(s)))
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
        // 시작한 멤버가 선 칸 중 설정 차례로 가장 앞 칸(moai-p415). 없으면(끝난 것과 첫 칸뿐)
        // 설정의 첫 시작 칸이다 — 반쯤 했지만 아무도 손대지 않은 자리를 말할 칸이 따로 없다.
        cfg.statuses
            .iter()
            .map(String::as_str)
            .find(|s| cfg.is_started(s) && counted.iter().any(|m| m.status.as_str() == *s))
            .unwrap_or(cfg.started_status())
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
///
/// `waits` 는 제 칸대로가 아닌 묶음이다([`Waits`]) — 칸이 `done` 이어도 미룬 멤버를
/// 기다리면 아직 막는다.
pub fn is_blocked(i: &Issue, by_id: &BTreeMap<&str, &Issue>, states: &BTreeMap<&str, &str>, waits: &Waits) -> bool {
    // 미룸은 막는가를 바꾸지 않는다 — 미룬 막음도 막는다([`Blocker::blocks`]).
    i.blocked_by.iter().any(|b| blocker(standing(b, by_id, states), false, waiting_of(b, waits)).blocks())
}

/// `blocked_by` 에 적힌 막음 하나가 지금 무엇인가.
///
/// **막는가의 뜻은 여기 하나다.** `ready`·`held`·`status` 와 탐색기 상세가 이것으로
/// 가른다. 한때 탐색기가 [`is_blocked`] 를 손으로 베껴, 없는 id 를 가리키는 막음을
/// `ready` 는 안 막힌 것으로 고르는데 상세는 "막힘" 이라 그렸다(moai-af64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocker {
    /// 안 끝났다 — 막는다.
    Open,
    /// 안 끝났고 계획에서 빠졌다 — **그래도 막는다.** 미룬 일은 끝난 일이 아니다([`held`]).
    Deferred,
    /// 서 있는 칸이 `done` 이다 — 풀렸다.
    Done,
    /// 그런 id 가 없다 — 막지 않는다. 끊긴 참조는 `moai status` 가
    /// `dangling_blocked_by` 로 드러내지, `ready` 가 영원히 막지 않는다.
    Missing,
    /// 멤버가 하나도 없는 묶음이다 — **막는다**(moai-1c2l). 끊긴 참조와 달리 채울 자리가
    /// 있다: 막음을 걸어 둔 빈 에픽은 대개 아직 안 채운 계획이다. 풀지 않는 대신 `ready`
    /// 가 비었다고 댄다([`held`]).
    Empty,
}

impl Blocker {
    /// 이 막음이 막히는 쪽을 `ready` 에서 빼는가.
    pub fn blocks(self) -> bool {
        matches!(self, Blocker::Open | Blocker::Deferred | Blocker::Empty)
    }
}

/// 막음 하나를 가른다. `column` 은 막는 줄이 **서 있는** 칸([`column`] — 묶음이면 읽은
/// 칸)이고 그 id 가 없으면 `None`, `out_of_plan` 은 그 줄이 계획에서 빠졌는가
/// ([`deferred_roots`] 에 드는가)다. `waiting` 은 막는 줄이 묶음일 때 무엇을 기다리는가
/// ([`Waiting`], 묶음이 아니면 `Live`) — 미뤄 뺀 멤버만 기다리면 칸이 `done` 이어도
/// 미룬 막음이고, 멤버가 없으면 빈 막음이다.
///
/// **묶음 제 미룸이 빈 막음보다 먼저다** — 도로 집는 말이 곧 풀 길이다.
///
/// **답만 여기서 정하고 재료는 부르는 쪽이 댄다.** 탐색기는 서 있는 칸과 미룸을 적재
/// 때 이미 세어 들고 있어, 저장소 전부를 받는 꼴로 두면 프레임마다 그 셈을 다시 한다.
pub fn blocker(column: Option<&str>, out_of_plan: bool, waiting: Waiting) -> Blocker {
    match column {
        None => Blocker::Missing,
        // 묶음 제 미룸이 미룬 멤버보다 먼저다 — 멤버를 도로 집어도 묶음이 미뤄져 있으면
        // 안 풀린다. 읽은 칸이 done 인 묶음은 `deferred_roots` 에 안 드므로 여기 안 걸린다.
        Some(c) if out_of_plan && c != crate::config::DONE => Blocker::Deferred,
        Some(_) if waiting == Waiting::Shelved => Blocker::Deferred,
        Some(crate::config::DONE) => Blocker::Done,
        Some(_) if out_of_plan => Blocker::Deferred,
        Some(_) if waiting == Waiting::Nothing => Blocker::Empty,
        Some(_) => Blocker::Open,
    }
}

/// 줄 하나의 막음 하나를 가른 것 — CLI 상세가 그리는 재료([`blocks_of`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block<'a> {
    pub id: &'a str,
    /// 막는 줄. 없는 id 면 `None`.
    #[serde(skip)]
    pub issue: Option<&'a Issue>,
    /// [`blocker`] 의 답.
    #[serde(rename = "state")]
    pub blocker: Blocker,
    /// 막는 줄을 계획에서 뺀 줄([`deferred_roots`]). 계획에 있으면 `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<&'a str>,
    /// 막는 묶음이 미뤄 뺀 멤버만 기다리면 그 멤버들([`Stand::aside`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aside: Vec<&'a str>,
}

/// 줄 하나가 적은 막음(`blocked_by`)을 **적힌 차례대로** 하나씩 가른다.
///
/// **`ready` 가 고르는 그 자로 가른다**(`blocking` → [`blocker`]) — 상세가 규칙을 손으로
/// 베끼면 없는 id 를 가리키는 막음을 `ready` 는 안 막힌 것으로 고르는데 상세는 "막힘" 이라
/// 그린다(moai-af64 에서 탐색기가 실제로 그랬다). 막음이 없으면 소속 지도를 안 세운다 —
/// 일 하나를 펼치는 흔한 길이다.
pub fn blocks_of<'a>(all: &'a [Issue], cfg: &Config, i: &'a Issue) -> Vec<Block<'a>> {
    if i.blocked_by.is_empty() {
        return Vec::new();
    }
    let group = groups(all);
    let by_id: BTreeMap<&str, &Issue> = all.iter().map(|x| (x.id.as_str(), x)).collect();
    let (off, states, waits, _) = blocking(all, cfg, &group, &by_id);
    // 막는 줄은 **id 로** 불린다 — 그쪽 답은 접은 지도의 것이다(moai-u3ta).
    let roots = fold_roots(all, &off);
    i.blocked_by
        .iter()
        .map(|b| {
            let issue = by_id.get(b.as_str()).copied();
            let root = roots.get(b.as_str()).copied();
            let (waiting, aside) = waits.get(b.as_str()).map_or((Waiting::Live, Vec::new()), |(w, a)| (*w, a.clone()));
            let blocker = blocker(issue.map(|x| column_by_id(x, &states)), root.is_some(), waiting);
            Block { id: b.as_str(), issue, blocker, root, aside }
        })
        .collect()
}

/// id 로 막는 줄을 찾아 그 서 있는 칸을 댄다. 없으면 `None`.
fn standing<'x>(id: &str, by_id: &BTreeMap<&str, &'x Issue>, states: &BTreeMap<&str, &'x str>) -> Option<&'x str> {
    by_id.get(id).map(|x| column_by_id(x, states))
}

/// [`column()`] 을 **`by_id` 로 찾아 온 줄**에 댄다.
///
/// **여기에는 쌍둥이가 없다**([`Kinds::no_twins`]) — `by_id` 가 고르는 줄은 늘 그 id 의 뒷줄,
/// 곧 그 id 의 뜻을 정하는 줄이라 제 지도에 가려질 수가 없다([`is_eclipsed`]). 막음은 id 로만
/// 줄을 찾으므로(`blocked_by` 에 적히는 것이 id 다) 막음을 가르는 자리는 모두 이쪽이다.
fn column_by_id<'x>(i: &'x Issue, states: &BTreeMap<&str, &'x str>) -> &'x str {
    column(&Kinds::no_twins(), i, states)
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
    let lines = Lines::of(issues);
    let epic_of = groups_over(issues, &lines);
    for (&(kind, g), of) in &members_in(issues, &epic_of, &lines, &kind_of) {
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
///
/// **부모가 에픽이면 그 에픽이 소속이다** (moai-9t3l). `--parent <에픽>` 으로 만든
/// 자식은 id 가 에픽 밑에 붙는데, "부모의 에픽을 물려받는다" 만으로는 부모가 에픽
/// 자신일 때 물려줄 것이 없어 그 줄이 `에픽 없음` 으로 빠졌다 — 에픽 전체를 보는
/// 리뷰를 세울 때마다 났다. 적힌 필드는 안 바꾼다. 소속은 파생값이라 읽을 때 정한다.
/// 차례는 물려받는 소속과 같다: 제 `epic` → 가까운 조상의 `epic` → 에픽인 조상.
/// 받는 줄은 [`joins`] 뿐이다.
pub fn groups(all: &[Issue]) -> BTreeMap<&str, &str> {
    groups_over(all, &Lines::of(all))
}

/// [`groups`] 와 같은 것. [`Lines`] 를 이미 든 쪽이 그것을 두 번 짓지 않게 받는다
/// (moai-jk2u.pl6, `지도는 한 벌이다` moai-g0zx).
pub fn groups_over<'a>(all: &'a [Issue], lines: &Lines<'a>) -> BTreeMap<&'a str, &'a str> {
    let (by_id, rooted) = (&lines.by_id, &lines.rooted);
    // **못 받은 줄도 지도를 쓴다** — `milestones` 와 같은 까닭이다. 받은 줄만 적으면
    // 같은 id 의 앞줄이 받은 에픽이 뒷줄에 흘러, 에픽 없는 뒷줄이 남의 에픽에
    // 그려지고 세어진다(moai-2m9p).
    let mut out = BTreeMap::new();
    let mut handed = Handed::new();
    for i in all {
        match epic_through(i, by_id, rooted, &mut handed) {
            Some(e) => out.insert(i.id.as_str(), e),
            None => out.remove(i.id.as_str()),
        };
    }
    out
}

/// 조상이 id 자식에게 **넘기는 소속**을 한 번 도는 동안 id 마다 한 번만 푸는 쪽지
/// (moai-1pra). 계획이 세우는 멤버는 `epic` 을 안 적고 id 로 소속을 지므로
/// ([`groups`]), 줄마다 조상을 걷는 값이 이 지도의 값이 되었다 — 멤버 서른이 같은
/// 에픽을 물으면 그 답은 한 번만 풀면 된다.
///
/// **쪽지는 그 한 번만 산다.** 파생값을 저장하지 않는다는 결정은 `issues.jsonl` 에
/// 대한 것이고, 여기는 `groups` 한 번이 끝나면 함께 사라진다.
///
/// 앞자리(`last`)는 **줄이 id 순으로 온다는 데 기대지 않는 지름길**이다. 파일이
/// id 로 정렬돼 있어 한 에픽의 멤버가 잇따라 오면 지도를 안 짚고 끝나고, 섞여 와도
/// 빗나간 한 번의 견줌만 치른다.
struct Handed<'a> {
    seen: BTreeMap<&'a str, Option<&'a str>>,
    last: Option<(&'a str, Option<&'a str>)>,
}

impl<'a> Handed<'a> {
    fn new() -> Handed<'a> {
        Handed { seen: BTreeMap::new(), last: None }
    }

    /// `from` 이 제 id 자식에게 넘기는 에픽. [`epic_through`] 의 오름을 그 조상에서
    /// 끊어 낸 조각이라 답이 같은 줄끼리 쪽지를 나눠 쓴다.
    ///
    /// 오르며 지난 줄은 **모두 같은 답을 받는다** — 제 `epic` 도 없고 뿌리로 올라간
    /// 생각도 에픽도 아니어서 넘어온 줄들이라, 그 답이 곧 제 답이다.
    fn from(
        &mut self,
        from: &'a str,
        by_id: &BTreeMap<&'a str, &'a Issue>,
        rooted: &BTreeSet<&str>,
    ) -> Option<&'a str> {
        if let Some((id, answer)) = self.last
            && id == from
        {
            return answer;
        }
        let mut walked: Vec<&'a str> = Vec::new();
        let mut at = Some(from);
        let answer = loop {
            let Some(id) = at else { break None };
            if let Some(hit) = self.seen.get(id) {
                break *hit;
            }
            walked.push(id);
            let Some(cur) = by_id.get(id).copied() else { break None };
            // **없는 부모와 뿌리로 올라간 생각에서 멈춘다** — [`epic_through`] 가
            // 조상을 짚을 때 거르던 그 자다. 차례도 같다: 뿌리 판정이 먼저다.
            if rooted.contains(id) {
                break None;
            }
            if cur.kind == Kind::Epic {
                break Some(cur.id.as_str());
            }
            if let Some(e) = &cur.epic {
                break Some(e.as_str());
            }
            at = crate::id::parent_of(&cur.id);
        };
        for id in walked {
            self.seen.insert(id, answer);
        }
        self.last = Some((from, answer));
        answer
    }
}

/// [`groups`] 를 **물은 줄에 대해서만** 낸다 — `--json` 의 `derived_epic` 이 읽는 지도다
/// (moai-wuzi). [`group_states_of`] 와 같은 꼴이고 같은 까닭이다: 줄 하나를 쓰는
/// 표면(`add`·`mv`·`edit`)이 락 안에서 저장소 전체의 지도를 들고 나가지 않는다.
///
/// **소속을 id 에 진 줄이 물은 것 가운데 하나도 없으면 지도를 안 짓는다.** `epic` 을 적은
/// 줄의 답은 그 줄이 들고 있고([`stands_in`] 이 그것을 먼저 읽는다), 묶음 줄은
/// 소속을 안 받는다([`joins`]) — 남는 것은 id 부모에게서 받는 줄뿐이다. 문의 조건은
/// [`epic_through`] 가 지도를 묻는 줄의 **위집합**이라야 한다 — 좁히면 지도가 낼 답이 있는
/// 줄에서 빈 지도가 돌아와, 같은 물음에 표면마다 다른 답이 선다(리뷰).
///
/// 물은 id 는 **집합으로 든다** — `ids` 를 줄마다 훑으면 아래 둘이 `줄 수 × 물은 수` 가 된다.
/// `ready --json` 은 집을 줄 전부를 묻는 자리라 그 곱이 이 함수의 값 거의 전부였다(리뷰).
///
/// **이 지도만으로는 답이 아니다.** 문이 닫히면 빈 지도가 돌아오는데 그것은 "에픽이 없다" 가
/// 아니라 "안 지었다" 다 — 답을 내는 자는 [`stands_in`] 이고, 그쪽이 줄의 `epic` 을 먼저
/// 읽는다. 이 지도를 혼자 짚으면 제 `epic` 을 적은 옛 멤버가 에픽 없는 줄로 나온다.
pub fn groups_of<'a>(all: &'a [Issue], ids: &[&str]) -> BTreeMap<&'a str, &'a str> {
    let want: BTreeSet<&str> = ids.iter().copied().collect();
    if want.is_empty() {
        return BTreeMap::new();
    }
    let from_id = |i: &Issue| joins(i) && i.epic.is_none() && crate::id::parent_of(&i.id).is_some();
    if !all.iter().any(|i| from_id(i) && want.contains(i.id.as_str())) {
        return BTreeMap::new();
    }
    let mut out = groups(all);
    out.retain(|id, _| want.contains(id));
    out
}

/// 줄 하나가 **든 에픽** — `--json` 의 `derived_epic` 이 내는 그 값이다(moai-wuzi).
/// 적힌 `epic` 이 먼저고([`epic_through`] 가 거기서 돌아서는 것과 같은 차례다), 없으면
/// `placed` 가 답이다 — [`groups`]·[`groups_of`] 가 그 줄에 준 값이다. 묶음 줄에는 안
/// 선다([`joins`], moai-fg0t).
///
/// **자는 하나다.** 이 키를 내는 표면이 둘인데([`crate::cmd::Row::of`] 와
/// `cmd::prime::Brief`), 저마다 이 차례를 적으면 한 바이너리가 한 키에 두 답을 낸다 —
/// moai-wuzi 가 없앤 자리가 바로 그것이다. 판단이 `report` 에 사는 것도 같은 까닭이다.
///
/// **가려진 줄은 어느 묶음에도 안 선다**(moai-53s2, 2026-09-23 사용자 결정). 소속 지도는
/// id 로 짠 것이라 종류가 다른 쌍둥이에게 가려진 줄([`is_eclipsed`])에 쌍둥이의 값을
/// 주는데, 트리는 그 줄을 `(길 잃음)` 에 두고 `query` 는 `-e` 로도 `-e none` 으로도 안
/// 고르며 롤업은 어느 묶음에도 안 센다 — `show --json` 목록만 그 줄에 쌍둥이의 에픽을
/// 달아, 한 바이너리가 "이 줄은 어느 에픽인가" 에 표면마다 다른 답을 했다.
/// **문을 여기 둔다**: `show` 목록 하나를 고치면 가려진 줄을 싣는 다음 표면이 같은
/// 구멍을 다시 연다. `kind_of` 는 [`Kinds`] 다 — 지도를 안 대는 자리는 [`Kinds::no_twins`]
/// 라는 이름을 적어야 하고, 그 이름을 적는 자리는 왜 쌍둥이가 못 서는지를 함께 댄다.
///
/// 적힌 `epic` 을 든 가려진 줄도 값을 안 낸다. `query` 가 그 줄을 제 `epic` 으로도 안
/// 고르므로([`is_eclipsed`] 를 거르개보다 먼저 보는 자리) 여기서 내면 둘이 또 갈린다.
/// 파일에 적힌 값 자체는 `--json` 의 `epic` 키에 그대로 남아, 고칠 곳이 화면에서
/// 사라지지는 않는다.
pub fn stands_in<'a>(kind_of: &Kinds<'_>, i: &'a Issue, placed: Option<&'a str>) -> Option<&'a str> {
    if kind_of.eclipses(i) {
        return None;
    }
    joined_in(i, || placed)
}

/// [`stands_in`] 에서 가려짐만 뺀 몸 — **가려진 줄을 이미 따로 거른 자리**가 부른다
/// ([`work_under`]·[`rollup_of_in`]·[`EpicLabels::at`]).
///
/// **차례는 [`groups`] 가 값을 짓는 차례와 같다**([`epic_through`]): 적힌 `epic` 이 먼저고,
/// 없으면 지도가 답이다. 그래서 id 가 하나뿐인 줄에서는 두 답이 **글자째 같다** — 지도의 값이
/// 곧 그 줄의 `epic` 이다. 갈리는 것은 같은 id 를 든 줄이 둘일 때뿐이고, 그때 지도는 뒷줄의
/// 값 하나만 들 수 있다(2026-09-23 사용자 결정, moai-7iyc.rt6).
///
/// 그 결정 전에는 지도를 짚는 자리(`query` 의 `-e`, 롤업, 트리, 목록의 에픽 칸)가 앞줄에 뒷줄의
/// 에픽을 주고 `derived_epic` 만 제 값을 내, 한 바이너리가 "이 줄은 어느 에픽인가" 에 표면마다
/// 다른 답을 했다. **자를 하나로 모은 것이지 `적힌 것이 먼저다` 를 옮긴 것이 아니다.**
///
/// **지도는 안 짚고 받는다**(리뷰) — 이 자는 묶음 줄에서 곧바로 물러나고 제 `epic` 을 적은 줄에서는
/// 지도를 안 보는데, 값으로 받으면 그 두 갈래에서도 짚기가 먼저 돈다. 이슈 1만 건에서 `moai status`
/// 한 번이 이 자를 6만 번 부르고 그 절반이 짚은 값을 버렸다.
pub(crate) fn joined_in<'a>(i: &'a Issue, placed: impl FnOnce() -> Option<&'a str>) -> Option<&'a str> {
    joins(i).then(|| i.epic.as_deref().or_else(placed)).flatten()
}

/// 줄마다 **선 묶음**을 답하는 자 — 소속 지도를 id 로 짚던 자리가 이것을 든다(moai-7iyc.rt6).
///
/// 지도는 id 로 짠 것이라 같은 id 를 든 줄이 둘이면 앞줄이 뒷줄의 값을 입는다. 소속은 그 줄에
/// 적힌 `epic` 이 먼저이므로([`joined_in`]) 그 자리에서 답이 갈리고, 그리는 쪽과 세는 쪽이
/// 지도를 저마다 짚으면 한 화면이 `0/1` 짜리 에픽 밑에 줄 둘을 그린다.
///
/// **축마다 재료가 다르니 갈래로 든다**(리뷰). 에픽 굴림과 마일스톤 굴림이 같은 걸음
/// ([`work_under`])을 쓰는데 답을 내는 차례가 축마다 다르므로, 그 차례를 갈래가 들고 산다.
///
/// 한때 네 필드를 든 구조체라 에픽 축이 안 보는 자리에 빈 지도 둘(`EMPTY_PLACES`·`EMPTY_KINDS`)을
/// 끼워 넣었는데, 그 꼴은 [`Kinds::no_twins`] 가 없애려던 바로 그것이다 — 에픽 갈래가 언젠가
/// 종류를 묻게 되면 빈 지도가 "아무것도 에픽이 아니다" 를 조용히 답해, 멤버가 통째로
/// `어느 묶음에도 안 딸린 것` 으로 떨어진다.
pub enum Placed<'m, 'a> {
    /// 에픽 축 — 소속 지도 하나면 된다.
    Epic(&'m BTreeMap<&'a str, &'a str>),
    /// 마일스톤 축 — 에픽이 마일스톤을 이기므로 에픽 지도를 들고, 그 위는 줄을 타고
    /// 오르므로 [`Lines`] 를 든다.
    ///
    /// **마일스톤 지도를 안 든다**(moai-jk2u.wvn). 에픽 없는 줄의 답은 그 줄 제
    /// `milestone` 과 조상에서 오는데, 지도는 id 로 짠 것이라 같은 id 의 앞줄이 뒷줄의
    /// 릴리스를 입었다 — `show <마일스톤>` 이 둘 다 내면서 앞줄이 적은 값은 어느 표면에서도
    /// 못 읽혔다. 값은 [`stood_at_line`] 이 줄마다 낸다.
    Milestone { epic_of: &'m BTreeMap<&'a str, &'a str>, lines: &'m Lines<'a> },
}

impl<'m, 'a> Placed<'m, 'a> {
    /// 에픽 축 — 그 줄에 적힌 `epic` 이 먼저고, 없으면 지도가 답이다([`joined_in`]).
    pub fn epic(epic_of: &'m BTreeMap<&'a str, &'a str>) -> Placed<'m, 'a> {
        Placed::Epic(epic_of)
    }

    /// 마일스톤 축 — **에픽이 마일스톤을 이긴다**([`milestones_in`]). 에픽을 든 줄은 그 에픽이
    /// 선 곳이고(못 쓸 에픽을 들었으면 어디에도 안 선다), 에픽이 없는 줄은 제 줄에서부터 센다.
    pub fn milestone(epic_of: &'m BTreeMap<&'a str, &'a str>, lines: &'m Lines<'a>) -> Placed<'m, 'a> {
        Placed::Milestone { epic_of, lines }
    }

    /// 그 묶음 종류의 자 — 굴림([`rollup_of_in`])·멤버([`group_members`])·[`Soil::placed`] 가
    /// 같은 갈래를 저마다 적던 자리다(리뷰). 세 벌이면 [`Placed::milestone`] 에 재료가 하나
    /// 더 붙는 날 두 곳만 고쳐도 컴파일된다.
    ///
    /// **에픽이 아닌 것은 마일스톤 축이다.** 묶음은 둘뿐이고([`is_group`]) 부르는 쪽이 이미
    /// 걸렀다 — 묶음이 아닌 종류로 부르면 답이 빈다.
    pub fn of(kind: Kind, epic_of: &'m BTreeMap<&'a str, &'a str>, lines: &'m Lines<'a>) -> Placed<'m, 'a> {
        match kind {
            Kind::Epic => Placed::epic(epic_of),
            _ => Placed::milestone(epic_of, lines),
        }
    }

    /// 이 자가 세는 묶음의 종류.
    pub fn kind(&self) -> Kind {
        match self {
            Placed::Epic(_) => Kind::Epic,
            Placed::Milestone { .. } => Kind::Milestone,
        }
    }

    /// 이 줄이 선 묶음. 어디에도 안 서면 `None`.
    pub fn at(&self, i: &'a Issue) -> Option<&'a str> {
        match self {
            Placed::Epic(epic_of) => joined_in(i, || epic_of.get(i.id.as_str()).copied()),
            Placed::Milestone { epic_of, lines } => {
                stood_at_line(i, joined_in(i, || epic_of.get(i.id.as_str()).copied()), lines)
            }
        }
    }
}

/// 그 **줄**이 선 마일스톤 — [`Placed::Milestone`] 의 몸이자 `--milestone` 거르개가 묻는 자다.
///
/// **자는 하나다.** 거르는 쪽(`query::Where::milestone_of`)과 세는 쪽([`work_under`])이 저마다
/// 이 차례를 적으면 `moai show <마일스톤>` 이 멤버로 세는 줄을 `moai show --milestone <그것>` 이
/// 안 내고 남의 릴리스에 낸다 — 자리를 정하는 자와 세는 자가 갈린 moai-lhbh 의 꼴이다(리뷰).
///
/// `epic` 은 그 줄이 **든 에픽**([`joined_in`]·[`stands_in`])이지 지도를 곧바로 짚은 값이
/// 아니다 — 지도는 id 로 짠 것이라 같은 id 를 든 앞줄이 뒷줄의 에픽을 타고 남의 마일스톤으로
/// 세어진다(moai-7iyc.rt6).
///
/// **마일스톤 지도를 안 짚는다**(moai-jk2u.wvn). 에픽 없는 줄이 그 지도로 돌아가면 같은
/// 갈림이 한 걸음 뒤에서 다시 난다 — 지도는 id 로 짠 것이라 에픽 없는 앞줄이 뒷줄의
/// 릴리스를 입고, 앞줄이 제 `milestone` 에 적은 값은 어느 표면에서도 안 읽힌다.
/// 여기가 [`milestones_in`] 이 줄마다 세는 그 몸이라, 지도의 값과 이 값은 늘 같다.
pub(crate) fn stood_at_line<'a>(i: &'a Issue, epic: Option<&'a str>, lines: &Lines<'a>) -> Option<&'a str> {
    match i.kind {
        // 에픽 줄은 제 필드뿐이고([`milestone_stood`]), 마일스톤 줄은 어느 마일스톤에도
        // 안 든다(moai-8tav). 둘 다 `joins` 가 걸러 `epic` 이 `None` 으로 오므로, 종류를
        // 여기서 다시 갈라야 아래 마지막 갈래로 안 떨어진다.
        Kind::Epic => milestone_stood(i),
        Kind::Milestone => None,
        Kind::Issue | Kind::Idea => match epic {
            // 에픽으로 못 쓸 것을 들었으면 어디에도 안 선다 — 그 줄은 `(길 잃음)` 으로 가므로
            // 제 `milestone` 으로 되돌아가면 머리글은 세는데 목록에 없는 줄이 남는다.
            Some(e) => lines.at(e).filter(|e| e.kind == Kind::Epic).and_then(milestone_stood),
            // 에픽이 없으면 **접힌 맨 위 줄에서부터** 센다. 제 줄에서 시작하면
            // 부모 밑에 그려진 자식이 제 마일스톤으로 세어진다(moai-uqoe).
            None => fold_top(i, &lines.by_id, &lines.rooted).and_then(|top| climb(top, &lines.by_id, &lines.rooted)),
        },
    }
}

/// 제 `epic` 필드 없이 id 부모에게서 오는 소속 — `(에픽, 부모 id)`. 제 `epic` 을
/// 적었거나 소속이 없으면 `None` 이다.
///
/// `edit -e none` 이 필드를 비워도 이 소속은 남는다(moai-w5gz) — id 는 옮기지 못하니
/// 부모 밑에 선 줄의 소속은 필드가 아니라 자리에서 온다. 조용하면 사람은 뺀 줄 안다.
/// 답은 [`groups`] 에서 읽는다 — 소속을 따로 재면 둘은 언젠가 어긋난다. 같은 id 가
/// 둘이면 `groups` 처럼 뒷줄이 선다.
pub fn epic_from_parent<'a>(all: &'a [Issue], id: &str) -> Option<(&'a str, &'a str)> {
    let line = all.iter().rev().find(|i| i.id == id)?;
    if line.epic.is_some() {
        return None;
    }
    let epic = *groups(all).get(id)?;
    Some((epic, crate::id::parent_of(&line.id)?))
}

/// 마일스톤을 정한 자리 — 적은 대로 안 선 줄을 **옮기려면 어디를 고치는가**.
///
/// 자리마다 옮기는 길이 달라 넷으로 가른다. 길을 `cmd` 가 다시 재면 안내가 셈과 어긋난다.
#[derive(Debug, PartialEq)]
pub enum Above<'a> {
    /// 제 에픽 — 에픽을 옮기거나 그 에픽의 마일스톤을 고치면 따라간다.
    Epic(&'a str),
    /// 에픽으로 **못 쓸** 소속 — 없는 id 거나 에픽이 아닌 줄이다. 줄은 `(길 잃음)` 에 서므로
    /// ([`misplaced`]) 그 id 의 마일스톤을 고쳐도 안 따라온다. 에픽을 옮기는 길만 있다.
    Lost(&'a str),
    /// 그 줄의 `milestone` 이 이 줄 대신 읽히는 id 조상 — 값을 든 조상이거나, 아무도 값을
    /// 안 들었으면 접힌 맨 위 줄([`fold_top`])이다. **바로 위 부모가 아니다** — 손자가 조부의
    /// 마일스톤을 받을 때 바로 위 부모를 대면, 그 부모를 고치라는 안내는 아무것도 안 바꾼다.
    Parent(&'a str),
    /// id 가 그 줄 밑에 서 있어 **필드로는 못 옮긴다** — 마일스톤 줄(`--parent <마일스톤>`)이거나
    /// 뿌리로 올라간 생각([`rooted_thoughts`]). 그 줄의 필드를 고치라고 대면 아무것도 안 바뀐다.
    Pinned(&'a str),
}

/// `--milestone <wrote>` 를 적은 줄이 **적은 대로 안 서면** — `(선 마일스톤, 정한 자리)`.
/// 적은 대로 섰으면 `None` 이다(`wrote` 가 `None` 이면 비우라고 적은 것이다). 적은 것과 선 것이
/// 같으면 말할 것이 없다 — 이긴 쪽이 마침 같은 곳에 서 있다.
///
/// **선 마일스톤이 없어도 자리는 댄다** — 마일스톤 없는 에픽의 멤버가 `--milestone X` 를 적으면
/// X 는 필드에만 남고 줄은 `(마일스톤 없음)` 에 선다(moai-mhxf). 그것도 에픽에게 진 것이다.
/// `edit --milestone none` 이 필드를 비워도 소속이 남는 것(moai-0lmn)과 같은 어긋남이다 —
/// [`milestones`] 는 에픽이 이기고 부모도 이긴다. [`epic_from_parent`] 와 같은 모양이다.
///
/// 답은 `milestones` 에서 읽고, 정한 자리만 같은 차례(에픽 → 접힌 맨 위 줄)로 가린다.
/// **옮기는 길은 적은 것에 따라 갈린다** — 마일스톤 줄 밑에 id 로 선 줄은 비워서는 못 끊지만,
/// 그 사이에 접힌 맨 위 줄이 따로 있으면 그 줄의 필드가 마일스톤 조상을 이겨 다른 마일스톤으로는
/// 옮겨진다. 에픽 줄과 마일스톤 줄은 제 필드에만 서거나 어디에도 안 서므로 언제나 `None` 이다.
pub fn milestone_from_above<'a>(
    all: &'a [Issue],
    id: &str,
    wrote: Option<&str>,
) -> Option<(Option<&'a str>, Above<'a>)> {
    let line = all.iter().rev().find(|i| i.id == id)?;
    if !joins(line) {
        return None;
    }
    // **에픽 지도는 한 번만 짓는다**(moai-oxup) — `milestones` 가 안에서 다시 지었다. `edit --milestone`
    // 마다 락 안에서 도는 길이다.
    let epic_of = groups(all);
    let milestone = milestones_in(all, &epic_of).get(id).copied();
    if milestone == wrote {
        return None;
    }
    let by_id: BTreeMap<&str, &Issue> = all.iter().map(|i| (i.id.as_str(), i)).collect();
    if let Some(e) = epic_of.get(id) {
        // 못 쓸 에픽의 멤버는 `(길 잃음)` 에 선다 — `milestones` 가 그 줄에 값을 안 주는 것과
        // 같은 자(없는 id·종류가 틀린 것)다. 그 id 의 마일스톤을 고치라고 대면 헛말이다.
        let usable = by_id.get(e).is_some_and(|x| x.kind == Kind::Epic);
        return Some((milestone, if usable { Above::Epic(e) } else { Above::Lost(e) }));
    }
    let rooted = rooted_thoughts(&by_id);
    let Some(top) = fold_top(line, &by_id, &rooted) else {
        // 뿌리로 올라간 생각 밑에 접혔다 — 그 밑은 `(마일스톤 없음)` 에 서고(`fold_top`), 어느
        // 필드를 고쳐도 안 옮겨진다. 조용하면 `show --milestone X` 가 그 줄을 말없이 못 낸다.
        let thought = std::iter::successors(crate::id::parent_of(&line.id), |p| crate::id::parent_of(p))
            .find(|p| rooted.contains(p))?;
        return Some((milestone, Above::Pinned(thought)));
    };
    // 값을 든 줄은 `climb` 이 읽는 그 줄이다 — 자를 따로 두면 안내가 셈과 어긋난다.
    // 그 줄이 제 줄이면 제 필드가 답이다(접히지 않은 줄의 제 `milestone`).
    // 아무도 값을 안 들었으면 정한 것은 접힌 맨 위 줄이다 — 제 필드는 거기서 안 읽힌다.
    match stood_at(top, &by_id, &rooted) {
        Some(source) if source.id == line.id => None,
        // id 가 마일스톤 밑이다. 비우는 것은 못 끊는다. 다른 마일스톤은 접힌 맨 위 줄이 제 줄이
        // 아니면 그 줄에 적어 옮긴다 — `stood_at` 이 마일스톤 조상보다 그 줄의 필드를 먼저 읽는다.
        Some(source) if source.kind == Kind::Milestone => Some((
            milestone,
            match wrote {
                Some(_) if top.id != line.id => Above::Parent(top.id.as_str()),
                _ => Above::Pinned(source.id.as_str()),
            },
        )),
        Some(source) => Some((milestone, Above::Parent(source.id.as_str()))),
        None if top.id != line.id => Some((milestone, Above::Parent(top.id.as_str()))),
        None => None,
    }
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
    // **쪽지는 여기서 따로 든다**([`Handed`]) — 다 찬 `rooted` 로 도는 [`groups`] 의 것과
    // 섞지 않는다. 차는 중인데도 쪽지가 서는 것은 **묻는 조상이 이미 정해졌기 때문이다**:
    // 오름은 `t` 의 조상만 짚고 조상의 id 는 반드시 더 짧아, 길이 순으로 도는 이 고리에서
    // 그 줄은 앞서 정해졌다(생각 아닌 줄은 `rooted` 에 아예 안 든다). 길이 순 정렬을
    // 걷어내면 이 쪽지부터 거짓이 된다.
    let mut handed = Handed::new();
    for t in thoughts {
        let folds = crate::id::parent_of(&t.id)
            .and_then(|p| by_id.get(p).copied())
            // **이 거르개는 값을 나른다**(리뷰) — 걷어낸 `passed_down` 은 [`epic_through`] 를
            // 거쳐 `joins` 로 한 번 더 걸렀는데 [`Handed::from`] 에는 그 걸음이 없다. 여기가
            // 열리면 에픽 밑에 바로 담긴 생각이 `folds` 로 읽혀 `rooted` 에 안 들고, 그 밑이
            // 통째로 에픽을 물려받는다.
            .filter(|p| matches!(p.kind, Kind::Issue | Kind::Idea))
            .is_some_and(|p| {
                // 부모가 넘기는 것은 [`Handed::from`] 이 그대로 낸다 — 뿌리로 올라간 생각에서
                // 끊는 것도, 제 `epic` 과 에픽인 조상의 차례도 그 안에 있다. 여기서 다시 쓰면
                // 자가 둘이 되고, 그 둘은 언젠가 어긋난다.
                epic_through(t, by_id, &rooted, &mut handed) == handed.from(p.id.as_str(), by_id, &rooted)
            });
        if !folds {
            rooted.insert(t.id.as_str());
        }
    }
    rooted
}

/// 그 줄의 에픽 — 제가 적었거나 조상에게서 물려받은 것, 또는 에픽인 조상 그 자신.
/// 뿌리로 올라간 생각에서 멈춘다. 생각 제 줄의 소속은 그대로 남는다.
/// **없는 부모는 넘지 않는다** — 지운 에픽의 자식은 `에픽 없음` 이고, 끊긴 id 는
/// `orphan_child` 가 드러낸다. 가리키는 필드가 없으니 `(길 잃음)` 이 아니다.
///
/// **묶음 줄에는 에픽이 없다** (moai-k9yb, moai-fg0t). [`joins`] 가 안 받는 줄이 조상을
/// 타고 오르면, 에픽인 부모는 거르지만 부모 이슈가 든 `epic` 은 그대로 받아
/// `epic add --parent <에픽 E 안의 이슈>` 가 `show -e E` 에만 서고 트리에서는
/// 뿌리에 섰다. 제 `epic` 필드도 같다 — `nav` 는 에픽을 에픽 밑에 두지 않으므로 그 필드로
/// 소속을 주면 `-e` 거름망만 트리에 없는 줄을 고른다. 필드는 지우지 않고, 못 쓸 참조로
/// [`broken`] 이 드러낸다(사용자 결정, 2026-09-18: 에픽 중첩 대신 경고).
fn epic_through<'a>(
    i: &'a Issue,
    by_id: &BTreeMap<&'a str, &'a Issue>,
    rooted: &BTreeSet<&str>,
    handed: &mut Handed<'a>,
) -> Option<&'a str> {
    if !joins(i) {
        return None;
    }
    if let Some(e) = &i.epic {
        return Some(e.as_str());
    }
    // 제 줄 위로는 한 걸음도 제 것이 아니다 — 조상이 넘기는 답은 그 조상을 부모로 둔
    // 모든 줄에 같으므로 [`Handed`] 가 한 번만 푼다.
    handed.from(crate::id::parent_of(&i.id)?, by_id, rooted)
}

/// id 부모인 묶음을 소속으로 **받는** 줄 — 이슈와 생각. 트리가 묶음 밑에 둘 수 있는
/// 줄이 이것들이다.
///
/// 묶음 줄은 안 받는다. 에픽 줄은 제 `milestone` 에만 서고(moai-0prl) `nav` 는 에픽을
/// 에픽 밑에 두지 않으므로, `moai epic add --parent <에픽>` 이 그 에픽을 소속으로
/// 받으면 `moai show -e <바깥 에픽>` 만 트리에 없는 줄을 고른다. 마일스톤은 뿌리에 선다.
/// 생각은 받되 그 밑에 그려지지는 않는다 — `-e <에픽>` 을 적은 생각과 같은 자리다.
fn joins(i: &Issue) -> bool {
    matches!(i.kind, Kind::Issue | Kind::Idea)
}

/// 이슈 id → 그것이 속한 마일스톤 id.
///
/// **에픽이 마일스톤을 이긴다.** 제 에픽이 있으면 그 에픽이 선 마일스톤이고,
/// 이슈가 `milestone` 을 따로 적었어도 그것은 지지 않는다. 에픽이 없으면
/// **부모도 이긴다** — 부모 밑에 접힌 줄은 접힌 맨 위 줄([`fold_top`])의
/// 마일스톤을 따른다. 이슈마다 마일스톤을 적게 하면 에픽을
/// 옮길 때 멤버를 전부 따라 고쳐야 하고, 반드시 하나는 빠뜨린다.
///
/// **에픽 줄은 제 `milestone` 에만 선다.** `nav` 는 에픽을 제 마일스톤 밑에만
/// 두고 에픽 밑에도 이슈 밑에도 두지 않으므로, 에픽 줄이 든 `epic` 과 id 부모는
/// 그 줄의 자리가 아니다. 그것을 타고 올라가 에픽 줄을 딴 곳에 세우면 트리는
/// 따라가지만, 이긴다는 규칙의 까닭(그 밑에 그려진다)이 없는 자리에서 제
/// 필드를 지운다. 못 쓸 `epic` 은 `broken` 이 드러낸다 — `misplaced` 가 에픽을
/// 제 마일스톤으로만 재는 것과 같은 차례다.
///
/// **자리를 정하는 자와 세는 자가 하나다.** 한때 `nav` 는 에픽을 이기게 하고
/// 여기서는 제 마일스톤을 이기게 해, `moai show <마일스톤>` 의 머리글이
/// `멤버 0/1` 이라 말하면서 목록에는 아무것도 못 내는 일이 있었다(moai-lhbh).
/// 자가 둘이면 둘은 언젠가 어긋난다 — 어느 쪽이 옳은지가 아니라 하나여야
/// 한다는 것이 요점이다. 그래서 멤버는 에픽의 필드를 다시 읽지 않고 **에픽
/// 줄이 받은 값**을 받는다. 다시 읽던 때에는 에픽 줄이 끊긴 `epic` 때문에
/// `(마일스톤 없음)` 에 서는데 멤버만 그 필드로 세어졌다(moai-0prl).
pub fn milestones(all: &[Issue]) -> BTreeMap<&str, &str> {
    milestones_in(all, &groups(all))
}

/// [`milestones`] 와 같은 것. 에픽 지도를 이미 가진 쪽([`Soil`])이 그것을 두 번 짓지 않게 받는다.
pub fn milestones_in<'a>(all: &'a [Issue], epic_of: &BTreeMap<&'a str, &'a str>) -> BTreeMap<&'a str, &'a str> {
    milestones_over(all, epic_of, &Lines::of(all))
}

/// [`milestones_in`] 과 같은 것. [`Lines`] 를 이미 든 쪽이 그것을 두 번 짓지 않게 받는다.
///
/// **몸은 [`stood_at_line`] 하나다**(moai-jk2u.wvn). 지도를 짓는 차례와 줄마다 묻는 차례를
/// 따로 적으면 그 둘은 언젠가 어긋나고, 어긋나는 자리는 `show <마일스톤>` 의 머리글과 그
/// 밑의 목록이다 — moai-lhbh 가 바로 그 꼴이었다.
pub fn milestones_over<'a>(
    all: &'a [Issue],
    epic_of: &BTreeMap<&'a str, &'a str>,
    lines: &Lines<'a>,
) -> BTreeMap<&'a str, &'a str> {
    let mut out = BTreeMap::new();
    for i in all {
        // **소속은 그 줄에 적힌 `epic` 이 먼저다**([`joined_in`], moai-7iyc.rt6) — 지도를
        // 곧바로 짚으면 같은 id 의 앞줄이 뒷줄의 에픽을 타고 남의 마일스톤으로 세어진다.
        let got = stood_at_line(i, joined_in(i, || epic_of.get(i.id.as_str()).copied()), lines);
        // **못 받은 줄도 지도를 쓴다 — 같은 id 의 뒷줄이 이긴다.** 멤버는 에픽 줄을
        // `by_id`(뒷줄이 이긴다)로 찾는데, 받은 줄만 적으면 같은 id 의 앞줄이 받은
        // 값이 남아 `nav::under_milestone(e)` 는 그 값으로 그리고 멤버는 뒷줄의
        // 빈 값으로 세어진다 — 머지를 잘못 푼 파일에서 moai-0prl 이 되살아난다.
        match got {
            Some(m) => out.insert(i.id.as_str(), m),
            None => out.remove(i.id.as_str()),
        };
    }
    out
}

/// 에픽 줄이 선 마일스톤 — 제 필드뿐이다. [`milestones`] 가 에픽 줄 자신에게도,
/// 그 에픽을 지나는 줄에게도 이것 하나를 준다. 두 곳이 따로 읽으면 자가 둘이다.
pub(crate) fn milestone_stood(epic: &Issue) -> Option<&str> {
    epic.milestone.as_deref()
}

/// 에픽 없는 맨 위 줄에서 id 부모를 타고 올라가며 처음 만나는 마일스톤.
///
/// 이 길에는 `epic` 을 든 줄이 없다 — 있었다면 `groups` 가 그것을 물려줘 에픽
/// 가지로 갔다. 그래서 부모만 탄다. id 가 줄어들며 올라가므로 고리가 없다.
/// **에픽 줄을 만나면 그 에픽이 선 곳에서 멈춘다**(`milestone_stood`) — 에픽 줄은 제
/// 부모에게서 마일스톤을 받지 않으므로, 넘어가면 에픽 줄과 다른 값을 낸다. 받는
/// 줄([`joins`])이 에픽을 거쳐 여기 오는 일은 이제 없다 — `groups` 가 에픽인 조상을
/// 소속으로 줘 에픽 가지로 갔다.
///
/// **마일스톤인 조상을 만나면 그 마일스톤이다** — 부모가 에픽이면 그 에픽이듯
/// (moai-9t3l). `--parent <마일스톤>` 의 자식이 `(마일스톤 없음)` 으로 빠지지 않는다.
/// 제 `milestone` 이 먼저다: 에픽에서 제 `epic` 이 먼저인 것과 같은 차례다.
/// **여기 오는 줄은 늘 일이다**([`joins`] 가 받는 이슈·생각). 마일스톤 줄은 오지 않는다 —
/// [`milestones`] 가 그 종류에 값을 안 준다(moai-8tav). 이슈 밑에 id 로 선 마일스톤 줄도
/// 그렇다. 에픽 줄은 [`milestones`]·[`milestone_from_above`] 가 먼저 제 필드로 돌아간다.
/// 한때 받는 줄인지를 인자로 넘겼는데 늘 참이라 걷었다(moai-dejq) — 거짓일 수 없는 가드는
/// "마일스톤 줄도 여기 온다" 는 없는 길을 읽는 사람에게 말한다.
fn climb<'a>(top: &'a Issue, by_id: &BTreeMap<&'a str, &'a Issue>, rooted: &BTreeSet<&str>) -> Option<&'a str> {
    let at = stood_at(top, by_id, rooted)?;
    match at.kind {
        Kind::Epic => milestone_stood(at),
        Kind::Milestone => Some(at.id.as_str()),
        _ => at.milestone.as_deref(),
    }
}

/// [`climb`] 이 마일스톤을 읽는 **그 줄** — 에픽 줄, 마일스톤인 조상, 또는 제
/// `milestone` 을 든 줄. [`milestone_from_above`] 가 넘긴 자리를 댈 때도 이것을 쓴다.
fn stood_at<'a>(top: &'a Issue, by_id: &BTreeMap<&'a str, &'a Issue>, rooted: &BTreeSet<&str>) -> Option<&'a Issue> {
    let mut cur = top;
    loop {
        if cur.kind == Kind::Epic {
            return Some(cur);
        }
        // `top` 은 언제나 이슈나 생각이다(부르는 쪽이 일만 넘기고 `fold_top` 도 그 종류로만
        // 오른다) — 그래서 여기 서는 마일스톤은 늘 조상이다.
        if cur.kind == Kind::Milestone {
            return Some(cur);
        }
        if cur.milestone.is_some() {
            return Some(cur);
        }
        // 부모가 뿌리로 올라간 생각이면 거기서 멈춘다 — `groups` 와 같은 자다.
        cur = crate::id::parent_of(&cur.id)
            .and_then(|p| by_id.get(p).copied())
            .filter(|p| !rooted.contains(p.id.as_str()))?;
    }
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
fn fold_top<'a>(i: &'a Issue, by_id: &BTreeMap<&'a str, &'a Issue>, rooted: &BTreeSet<&str>) -> Option<&'a Issue> {
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

/// **같은 id 의 뜻을 정하는 줄과 종류가 다른 가려진 줄**인가.
///
/// 중복 id 는 이 도구가 거부하지 않고 드러내기만 하는 상태(`duplicate_id`)고, id 로
/// 짠 지도(`groups`·`milestones`·`misplaced`)는 모두 뒷줄 — `by_id` 가 고르는 줄 — 이
/// 이긴다. 그 값은 **그 줄의 종류로** 셈한 것이다: 마일스톤 줄이 든 `epic`, 에픽 줄이
/// 받은 제 `milestone`, 생각이 `(마일스톤 없음)` 에 서면서 세는 마일스톤. 종류가 다른
/// 쌍둥이가 그 값을 제 종류로 읽으면 그리는 자리와 세는 묶음이 갈리고, 한때는 생각
/// 줄의 길 잃음 판정이 마일스톤 줄에 흘러 그 마일스톤이 통째로 `(길 잃음)` 에
/// 끌려갔다(moai-2m9p).
///
/// 그래서 그런 줄은 **어느 묶음에도 세지 않고** `nav` 는 `(길 잃음)` 에 둔다 — 제 뜻을
/// 담을 지도 칸이 없으니 자리를 정할 수 없는 줄이다. 지우지 않으므로 보이고,
/// `duplicate_id` 가 그 id 를 따로 드러낸다. 같은 종류의 쌍둥이는 같은 값을 같은 자로
/// 읽으므로 여기 안 걸린다.
pub fn eclipsed(all: &[Issue]) -> impl Fn(&Issue) -> bool + '_ {
    let kind_of = kinds(all);
    move |i| is_eclipsed(&kind_of, i)
}

/// id → 그 id 를 **마지막으로 든 줄**의 종류. 가려짐 판정([`is_eclipsed`])이 보는 지도다.
pub fn kinds(all: &[Issue]) -> BTreeMap<&str, Kind> {
    all.iter().map(|i| (i.id.as_str(), i.kind)).collect()
}

/// 가려짐을 가르는 지도를 **든 꼴 그대로** 나르는 그릇 (moai-53s2, 리뷰).
///
/// 두 가지를 같이 푼다.
///
/// - **빌린 것을 안 옮겨 담는다.** 탐색기는 `BTreeMap<String, Kind>` 를 들고 있고 `Soil` 은
///   `BTreeMap<&str, Kind>` 를 낸다. 한 꼴로 좁히면 든 꼴이 다른 쪽이 꼴을 맞추느라 지도를
///   새로 짓는데, 탐색기의 거름망은 **키마다** 그것을 부른다(`tui::Ground::here`) — 이슈
///   1만 건에서 가장 큰 지도를 걸음마다 짓고 버린다.
/// - **"쌍둥이가 없다" 를 낱말로 적게 한다.** 빈 지도를 그냥 넘기면 그것이 새 표면이 베끼는
///   꼴이 되어, 지도를 안 댄 표면이 시험 전부가 푸른 채로 이 구멍을 다시 연다. 여기서는
///   [`Kinds::no_twins`] 라는 이름을 적어야 하고, 그 이름을 적는 자리는 **왜 없는지**를
///   함께 댄다.
#[derive(Default)]
pub enum Kinds<'a> {
    /// 쌍둥이가 설 수 없는 자리 — 부르는 쪽이 그 까닭을 댄다([`Kinds::no_twins`]).
    #[default]
    NoTwins,
    /// 이 자리에서 지은 지도 ([`kinds`]·[`kinds_of`]).
    Own(BTreeMap<&'a str, Kind>),
    /// 남이 들고 있는 지도를 **빌린 것** — 옮겨 담지 않는다.
    Kept(&'a BTreeMap<String, Kind>),
    /// [`Kinds::Own`] 을 든 쪽이 그것을 **빌려 주는 꼴**. [`Kinds::Kept`] 는 탐색기가 든
    /// `String` 지도의 것이라 `&str` 로 짠 지도는 못 빌렸고, 그래서 지도를 이미 든 명령이
    /// 제목을 얹으려고 한 벌을 더 지었다(리뷰 moai-jk2u.m60).
    Borrowed(&'a BTreeMap<&'a str, Kind>),
}

impl<'a> Kinds<'a> {
    /// 저장소 전체의 지도 ([`kinds`]).
    pub fn of(all: &'a [Issue]) -> Kinds<'a> {
        Kinds::Own(kinds(all))
    }

    /// 물은 줄만의 지도 ([`kinds_of`]).
    pub fn of_ids(all: &'a [Issue], ids: &[&str]) -> Kinds<'a> {
        Kinds::Own(kinds_of(all, ids))
    }

    /// 남이 든 지도를 빌린다.
    pub fn kept(map: &'a BTreeMap<String, Kind>) -> Kinds<'a> {
        Kinds::Kept(map)
    }

    /// 제가 든 지도를 **빌려 준다** — 옮겨 담지 않는다. 지도를 이미 든 쪽(`query::Where`)이
    /// 그것을 달라는 자리에 준다.
    pub fn lend(&'a self) -> Kinds<'a> {
        match self {
            Kinds::NoTwins => Kinds::NoTwins,
            Kinds::Own(m) => Kinds::Borrowed(m),
            Kinds::Kept(m) => Kinds::Kept(m),
            Kinds::Borrowed(m) => Kinds::Borrowed(m),
        }
    }

    /// **쌍둥이가 설 수 없다고 아는 자리.** 부르는 쪽은 그 까닭을 곁에 적는다 — 지금 서는
    /// 자리 둘은 `store::with_write` 가 중복 id 에 쓰기를 통째로 물리는 쓰기 경로와,
    /// `Load::get` 이 늘 뒷줄을 여는 `show <id>` 다. 까닭을 못 대면 지도를 지어야 한다.
    pub const fn no_twins() -> Kinds<'a> {
        Kinds::NoTwins
    }

    /// 그 줄이 종류가 다른 쌍둥이에게 id 가 가려졌는가 ([`is_eclipsed`]).
    pub fn eclipses(&self, i: &Issue) -> bool {
        match self {
            Kinds::NoTwins => false,
            Kinds::Own(m) => is_eclipsed(m, i),
            Kinds::Kept(m) => is_eclipsed(*m, i),
            Kinds::Borrowed(m) => is_eclipsed(*m, i),
        }
    }
}

/// [`kinds`] 를 **물은 줄에 대해서만** 낸다 — [`groups_of`] 와 같은 꼴이고 같은 까닭이다
/// (moai-53s2). [`stands_in`] 이 가려짐을 보려면 지도가 있어야 하는데, `ready --json` 은
/// 제가 낼 줄 몇 개만 물으므로 저장소 전체의 종류 지도를 지을 까닭이 없다.
///
/// **좁혀도 답이 같다.** [`is_eclipsed`] 는 그 줄의 id 하나만 짚고, 여기 담기는 값은
/// [`kinds`] 와 똑같이 **그 id 를 마지막으로 든 줄**의 종류다 — 물은 id 가 아닌 줄이
/// 빠져도 물은 줄의 답은 안 바뀐다.
pub fn kinds_of<'a>(all: &'a [Issue], ids: &[&str]) -> BTreeMap<&'a str, Kind> {
    let want: BTreeSet<&str> = ids.iter().copied().collect();
    if want.is_empty() {
        return BTreeMap::new();
    }
    all.iter().filter(|i| want.contains(i.id.as_str())).map(|i| (i.id.as_str(), i.kind)).collect()
}

/// [`eclipsed`] 의 판정 — `kind_of` 는 [`kinds`] 의 지도다(빌린 id 든 소유한 id 든). **판정은 여기
/// 하나다**(moai-xemz 리뷰): 지도를 이미 가진 쪽(`Soil`·`query::Where`·탐색기의 `tui::Ground`)이 저마다
/// 몸을 다시 적으면, 쌍둥이를 가르는 자가 하나만 바뀐 날 CLI 와 탐색기가 같은 줄을 달리 고른다.
pub fn is_eclipsed<K: std::borrow::Borrow<str> + Ord>(kind_of: &BTreeMap<K, Kind>, i: &Issue) -> bool {
    kind_of.get(i.id.as_str()).is_some_and(|k| *k != i.kind)
}

/// **파일 전체를 훑어야 아는 것을 한 걸음으로 잰다**(moai-fbdg) — 소속(에픽·마일스톤), 물려받은
/// 미룸, 가려진 쌍둥이, 못 쓸 참조, 길 잃음 밑에 접힌 줄. 지도 하나가 다음 지도의 재료라 따로 부르면
/// `groups` 만 서너 번 돈다: `milestones`·`misplaced` 가 저마다 다시 짓고, 탐색기는 그 위에 색인
/// (`nav::Index`)·묶음 칸(`group_stands`)·거름망(`query::Where`)이 또 한 벌씩 지었다 — 이슈 1만 건에서
/// 거름망 한 번이 300ms 였다.
///
/// **칸(`Config`)은 안 든다.** 여기까지는 설정을 모르고 재는 것이고, 묶음이 선 칸만 설정을 보므로
/// [`Soil::stands`] 로 따로 낸다 — 설정 없이 자리만 정하는 쪽(`nav::Index::of`)이 그 값을 안 치른다.
pub struct Soil<'a> {
    /// 줄 id → 그 줄이 든 에픽([`groups`]).
    pub epic: BTreeMap<&'a str, &'a str>,
    /// 줄 id → 그 줄이 선 마일스톤([`milestones`]).
    pub milestone: BTreeMap<&'a str, &'a str>,
    /// 계획에서 빠진 줄 → 그것을 뺀 줄([`deferred_roots`]). **id 로 접은 것이라 뒷줄이
    /// 이긴다** — id 밖에 없는 표면(막는 줄, `shelved_by`, 도로 집을 곳)의 답이다.
    pub roots: BTreeMap<&'a str, &'a str>,
    /// 줄마다 — 그 줄을 계획에서 뺀 줄([`shelved_over`]). `all` 과 **자리가 같다**.
    /// 줄을 든 쪽은 이것을 짚는다 — 위의 `roots` 는 같은 id 의 앞줄에 뒷줄의 답을 준다
    /// (moai-u3ta).
    pub shelved: Vec<Option<&'a str>>,
    /// id → 그 id 를 마지막으로 든 줄의 종류([`kinds`]).
    pub kinds: BTreeMap<&'a str, Kind>,
    /// 못 쓸 소속 참조를 든 줄([`misplaced`]).
    pub lost: BTreeMap<&'a str, Misplace>,
    /// 길 잃은 줄 **밑에 접힌** 줄([`under_lost`]).
    pub folded: BTreeSet<&'a str>,
    /// 줄을 id 로 찾는 지도와 뿌리로 올라간 생각([`Lines`]) — **한 벌이다**(moai-jk2u.pl6).
    /// [`milestones_over`]·[`deferred_roots_over`]·[`Shelf`]·[`Placed::Milestone`] 이 저마다
    /// 지어 들던 것이라, `status` 한 번에 같은 걸음이 네 번 돌았다.
    pub lines: Lines<'a>,
}

impl<'a> Soil<'a> {
    pub fn of(all: &'a [Issue]) -> Soil<'a> {
        let lines = Lines::of(all);
        let epic = groups_over(all, &lines);
        let milestone = milestones_over(all, &epic, &lines);
        // 문은 [`deferred_roots_over`] 안에 이미 있다 — 여기서 한 번 더 세우면 미룬 줄이 있는
        // 저장소에서 목록을 한 벌 더 훑고, 없는 저장소에서 아끼는 것은 그쪽이 이미 아낀다.
        let shelved = shelved_over(all, &lines, &epic, &milestone);
        let roots = fold_roots(all, &shelved);
        let kinds = kinds(all);
        // **판정은 줄마다 한 번이다**(moai-jk2u.pl6) — `lost` 는 그것을 id 로 접은 지도고
        // (뒷줄이 이긴다), `folded` 는 줄마다의 값을 그대로 쓴다. 둘을 따로 재던 때는 같은
        // 판정을 줄마다 두세 번 쳤다.
        let verdict: Vec<Option<Misplace>> = all.iter().map(|i| misplace_of(i, &kinds, &epic, &lines)).collect();
        let lost = folded_by_id(all, verdict.iter().copied());
        // 길 잃음은 `nav::Ctx::home` 과 같다 — 못 쓸 참조를 든 줄과 가려진 쌍둥이. **줄마다
        // 묻는다** — `lost` 는 id 로 짠 지도라 같은 id 의 앞줄이 뒷줄의 판정을 입는데, 접는
        // 자와 자리를 정하는 자는 그 줄의 것을 봐야 한다.
        let adrift: Vec<bool> = all.iter().zip(&verdict).map(|(i, m)| m.is_some() || is_eclipsed(&kinds, i)).collect();
        let folded = under_lost(all, &lines, &epic, &adrift);
        Soil { epic, milestone, roots, shelved, kinds, lost, folded, lines }
    }

    /// 그 줄이 종류가 다른 쌍둥이에게 id 가 가려졌는가([`is_eclipsed`]).
    pub fn eclipsed(&self) -> impl Fn(&Issue) -> bool + '_ {
        |i: &Issue| is_eclipsed(&self.kinds, i)
    }

    /// 그 축에서 **줄마다 선 묶음**을 답하는 자([`Placed`]).
    pub fn placed(&self, kind: Kind) -> Placed<'_, 'a> {
        Placed::of(kind, &self.epic, &self.lines)
    }

    /// 그 **줄**의 소속 참조가 못 쓸 것인가([`misplace_of`]) — `lost` 는 id 로 짠 지도라 같은
    /// id 의 앞줄이 뒷줄의 판정을 입는다. 자리를 정하는 쪽(`nav::Ctx::adrift`)과 드러내는 쪽이
    /// 한 자를 써야 트리의 `(길 잃음)` 과 `moai status` 가 같은 줄을 말한다(리뷰).
    pub fn adrift(&self, i: &Issue) -> Option<Misplace> {
        misplace_of(i, &self.kinds, &self.epic, &self.lines)
    }

    /// 묶음이 **선 칸과 곁들이**([`group_stands`]) — 여기서만 설정을 본다.
    pub fn stands<'c>(&self, all: &'a [Issue], cfg: &'c Config) -> BTreeMap<&'a str, Stand<'a, 'c>> {
        group_stands_in(all, cfg, &self.lines, &self.epic, &self.milestone, &self.kinds, &self.shelved)
    }
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
// 바이너리는 [`Soil::of`] 에서 줄마다 한 번 재고 [`folded_by_id`] 로 접는다 — 이 꼴은 시험의
// 짧은 길이다(moai-oxup). **접는 몸은 하나다**(리뷰 moai-jk2u.m60): 차례를 두 곳에 적으면
// 시험이 보는 지도와 `moai status` 가 내는 지도가 언젠가 갈린다.
#[cfg(test)]
pub fn misplaced(all: &[Issue]) -> BTreeMap<&str, Misplace> {
    let epic_of = groups(all);
    let lines = Lines::of(all);
    let kind_of = kinds(all);
    folded_by_id(all, all.iter().map(|i| misplace_of(i, &kind_of, &epic_of, &lines)))
}

/// 줄마다의 자리 판정을 id 로 접는다 — [`fold_by_id`] 가 그 몸이다.
fn folded_by_id(all: &[Issue], verdict: impl IntoIterator<Item = Option<Misplace>>) -> BTreeMap<&str, Misplace> {
    fold_by_id(all, verdict)
}

/// `misplaced` 가 **줄 하나에** 내리는 그 판정.
///
/// **술어는 하나다.** 자리를 정하는 쪽(`nav::Ctx::home`)이 이것을 그대로 부른다 — 양쪽에
/// 따로 적으면 탐색기가 `(길 잃음)` 에 넣은 줄을 `moai status` 가 입 다물고 지나간다.
///
/// **줄마다 다를 수 있다**(moai-7iyc.rt6). 소속은 그 줄에 적힌 `epic` 이 먼저라([`joined_in`])
/// 같은 id 를 든 줄 둘이 서로 다른 참조를 들면 판정도 갈린다. [`folded_by_id`] 의 지도는 id 로
/// 짠 것이라 뒷줄의 판정 하나만 들 수 있고, **자리를 정하는 쪽은 줄마다 물어야 한다** — 앞줄이
/// 못 쓸 에픽을 들었는데 뒷줄의 판정(`없음`)을 입으면, 그 줄은 없는 에픽 바구니로 가 어느
/// 자리에도 안 그려진다(무작위 대조가 잡았다).
/// **마일스톤 쪽도 줄마다 묻는다**(moai-jk2u.wvn). 지도는 id 로 짠 것이라 같은 id 의 앞줄이
/// 뒷줄의 릴리스로 판정됐다 — 끊긴 참조를 든 앞줄이 성한 뒷줄의 값으로 "멀쩡하다" 가 되어
/// `(길 잃음)` 을 벗어나는데, 그 줄을 그리는 자는 줄마다 답하므로 닿는 길 없는 자리에 남는다.
/// 무작위 대조(`nav::tests::a_milestone_counts_what_it_draws_on_random_piles`)가 그 판을 냈다.
pub(crate) fn misplace_of<'a>(
    i: &'a Issue,
    kind_of: &BTreeMap<&str, Kind>,
    epic_of: &BTreeMap<&str, &str>,
    lines: &Lines<'a>,
) -> Option<Misplace> {
    let id = i.id.as_str();
    let usable = |id: Option<&str>, kind: Kind| id.is_none_or(|id| kind_of.get(id) == Some(&kind));
    match i.kind {
        // 뿌리에 선다. 가리키는 것이 없다.
        Kind::Milestone => None,
        Kind::Epic => (!usable(milestone_stood(i), Kind::Milestone)).then_some(Misplace::Milestone),
        // idea 도 같은 자를 받는다. 에픽을 안 적은 idea 는 아무것도 안
        // 가리키므로 여기 걸릴 것이 없고, 적었는데 그것이 에픽이 아니면
        // 일과 똑같이 드러나야 한다.
        Kind::Issue | Kind::Idea => match joined_in(i, || epic_of.get(id).copied()) {
            Some(e) if kind_of.get(e) != Some(&Kind::Epic) => Some(Misplace::Epic),
            // 에픽이 멀쩡하면 그 에픽의 마일스톤을 따르므로 여기서 안 본다.
            Some(_) => None,
            // 에픽이 없으면 조상을 타고 오른 값이다([`stood_at_line`]) — 지도가 그 줄에
            // 담던 바로 그 답이라 판정이 안 바뀐다.
            None if !usable(stood_at_line(i, None, lines), Kind::Milestone) => Some(Misplace::Milestone),
            None => None,
        },
    }
}

/// 제가 길을 잃지는 않았지만 **길 잃은 줄 밑에 접힌** 줄 — 트리가 그 줄을 부모 밑, 곧
/// `(길 잃음)` 바구니 안에 그린다(moai-uni2, 사용자와 정함: 길 잃은 부모 밑에 접힌다).
///
/// 흔한 모양은 끊긴 에픽을 든 생각 밑의 일이다. 생각은 소속을 안 넘기므로 자식은 에픽이
/// 없다고 셈해지는데, 트리는 그 줄을 길 잃은 생각 밑에 접는다 — status 가 "에픽 없는
/// 이슈" 로 세면 고칠 수 없는 줄에 `-e none` 을 가리킨다. 고칠 곳은 부모의 끊긴 참조
/// 하나이고 `dangling_epic` 이 그것을 댄다.
///
/// **접는 자는 `nav::Ctx::home_of_work` 와 같다** — 부모가 이슈나 생각이고, 제 소속이
/// 부모가 넘기는 것과 같다. 생각인 부모는 뿌리로 올라갔거나 제가 길을 잃었으면(그려진
/// 자리가 이슈 밑이 아니면) 아무것도 안 넘긴다. 자를 따로 두면 트리와 status 가 또 갈린다.
/// 길 잃음은 부르는 쪽이 준다 — `nav::Ctx::home` 처럼 가려진 쌍둥이도 거기 든다.
/// **판정은 미리 잰 값으로 받는다**(moai-jk2u.pl6). 술어로 받던 때는 `.any()` 한 번과 고리에서
/// 한 번, 그리고 조상마다 또 한 번 쳐서 1만 줄에서 `misplace_of` 가 1만 번에서 2~3만 번이
/// 됐다. `lost` 는 `all` 과 **자리가 같은** 배열이고, 조상의 자리는 여기서 짓는 지도가 댄다.
pub fn under_lost<'a>(
    all: &'a [Issue],
    lines: &Lines<'a>,
    epic_of: &BTreeMap<&'a str, &'a str>,
    lost: &[bool],
) -> BTreeSet<&'a str> {
    debug_assert_eq!(all.len(), lost.len(), "판정이 줄과 자리를 맞춰야 한다");
    // 길 잃은 줄이 없으면 그 밑에 접힐 줄도 없다.
    if !lost.contains(&true) {
        return BTreeSet::new();
    }
    // 조상의 판정 — `by_id` 가 고르는 뒷줄의 것이다. 자리를 못 찾으면 길을 안 잃은 것으로 친다
    // (`by_id` 에 없는 id 는 아래 걸음이 이미 걸렀다).
    //
    // **여기서 짓는다**(리뷰 moai-jk2u.m60) — [`Lines`] 에 얹어 두면 그것을 짓는 열댓 자리가
    // 모두 이 지도를 짓는데, 읽는 자는 여기 하나고 그마저 위 문을 지난 저장소에서만 선다.
    let seat: BTreeMap<&str, usize> = all.iter().enumerate().map(|(k, i)| (i.id.as_str(), k)).collect();
    let lost_at = |i: &Issue| seat.get(i.id.as_str()).is_some_and(|&k| lost[k]);
    let mut out = BTreeSet::new();
    for i in all.iter().enumerate().filter(|(k, _)| !lost[*k]).map(|(_, i)| i) {
        let mut cur = i;
        while let Some(p) = crate::id::parent_of(&cur.id)
            .and_then(|p| lines.at(p))
            .filter(|p| matches!(p.kind, Kind::Issue | Kind::Idea))
        {
            // **소속은 줄마다 묻는다**(리뷰, [`joined_in`]) — `nav::Ctx::home_of_work` 가 같은
            // 물음을 `Ctx::epic_at` 으로 옮긴 자리다. 여기만 지도를 짚으면 접는 자와 그리는 자가
            // 갈려, 트리가 `(길 잃음)` 안에 그린 줄을 `no_epic` 이 밖에서 또 센다.
            let passed = match is_idea(p) && (lines.rooted.contains(p.id.as_str()) || lost_at(p)) {
                true => None,
                false => joined_in(p, || epic_of.get(p.id.as_str()).copied()),
            };
            if joined_in(cur, || epic_of.get(cur.id.as_str()).copied()) != passed {
                break;
            }
            if lost_at(p) {
                out.insert(i.id.as_str());
                break;
            }
            cur = p;
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
///
/// **묶음 줄의 `epic` 은 가리키는 것이 멀쩡해도 못 쓴다** (moai-fg0t). 묶음 줄은 에픽에
/// 안 들므로([`epic_through`]) 그 필드는 아무 자리도 안 정한다 — 말없이 두면 적은 사람은
/// 에픽 밑에 넣은 줄 안다.
// 바이너리는 지도를 든 `_in` 을 부른다([`Soil`], moai-oxup) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn broken(all: &[Issue]) -> BTreeMap<&str, Misplace> {
    broken_in(all, &kinds(all))
}

/// [`broken`] 와 같은 것. 종류 지도를 이미 가진 쪽([`Soil`])이 그것을 다시 짓지 않게 받는다.
pub fn broken_in<'a>(all: &'a [Issue], kind_of: &BTreeMap<&'a str, Kind>) -> BTreeMap<&'a str, Misplace> {
    let usable = |id: &Option<String>, kind: Kind| id.as_deref().is_none_or(|id| kind_of.get(id) == Some(&kind));
    let mut out = BTreeMap::new();
    for i in all {
        if !usable(&i.epic, Kind::Epic) || (is_group(i) && i.epic.is_some()) {
            out.insert(i.id.as_str(), Misplace::Epic);
        } else if !usable(&i.milestone, Kind::Milestone) {
            out.insert(i.id.as_str(), Misplace::Milestone);
        }
    }
    out
}

/// 에픽별 집계와, 마지막에 "에픽 없음" 묶음 하나.
///
/// **멤버가 없는 에픽도 줄을 갖는다.** 빠뜨리면 "계획만 세우고 안 채운 것"
/// 이 화면에서 사라져, 정확히 드러내야 할 것이 안 보인다.
/// **소속 없는 이슈도 묶음을 갖는다.** 같은 이유다.
// 바이너리는 지도를 든 [`rollup_in`] 을 부른다(moai-g0zx) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn rollup(issues: &[Issue], cfg: &Config) -> Vec<Roll> {
    rollup_of(Kind::Epic, issues, cfg)
}

/// [`rollup`] 과 같은 것. **이미 잰 지도를 받는다**([`Soil`]) — `moai show --tree` 는 바로 옆에서
/// 거름망과 색인을 같은 지도로 짓는다(moai-g0zx).
pub fn rollup_in(issues: &[Issue], cfg: &Config, soil: &Soil<'_>) -> Vec<Roll> {
    rollup_of_in(issues, cfg, &Placed::epic(&soil.epic), &soil.eclipsed())
}

/// `kind` 가 에픽이든 마일스톤이든 같은 셈을 한다. **일은 이슈가 한다** —
/// 에픽은 어느 쪽 집계에도 세지 않는다.
// 바이너리는 지도를 든 [`rollup_of_in`] 을 부른다(moai-g0zx) — 이 꼴은 시험의 짧은 길이다.
#[cfg(test)]
pub fn rollup_of(kind: Kind, issues: &[Issue], cfg: &Config) -> Vec<Roll> {
    let lines = Lines::of(issues);
    let epic_of = groups_over(issues, &lines);
    let kind_of = kinds(issues);
    // 종류 지도는 한 벌이다 — `eclipsed(issues)` 는 제 안에서 또 하나 짓는다.
    rollup_of_in(issues, cfg, &Placed::of(kind, &epic_of, &lines), &|i: &Issue| is_eclipsed(&kind_of, i))
}

/// [`rollup_of`] 와 같은 것. **소속을 답하는 자와 가려짐을 받는다**(moai-oxup) — `status` 는 에픽과
/// 마일스톤을 둘 다 굴리는데, 저마다 지으면 `groups` 가 그것만으로 세 번 돈다(`milestones` 가 안에서
/// 또 짓는다). 굴릴 종류는 [`Placed`] 가 제 갈래로 들고 온다([`Placed::kind`]).
pub fn rollup_of_in(
    issues: &[Issue],
    cfg: &Config,
    placed: &Placed<'_, '_>,
    eclipsed: &impl Fn(&Issue) -> bool,
) -> Vec<Roll> {
    let kind = placed.kind();
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
    // **묶음도 급한 것이 위로 온다.** 파일 순(=id 순)으로 두면 이슈 목록과
    // 차례가 달라, 같은 화면에서 규칙이 둘이 된다.
    let mut groupings: Vec<&Issue> = issues.iter().filter(|i| i.kind == kind && !eclipsed(i)).collect();
    groupings.sort_by(|a, b| crate::query::display_order(a, b));
    let members = work_under(issues, placed, eclipsed);
    let mut out: Vec<Roll> = groupings
        .iter()
        .map(|e| {
            let of = members.get(e.id.as_str()).map(Vec::as_slice).unwrap_or_default();
            let (counts, total, done, percent) = tally(of);
            Roll {
                id: Some(e.id.clone()),
                title: e.title.clone(),
                counts,
                total,
                done,
                percent,
                column: None,
                deferred: None,
            }
        })
        .collect();

    // 어느 묶음에도 안 딸린 일. 에픽은 일이 아니라 묶음이라 세지 않는다.
    // **여기도 [`Placed::at`] 에 묻는다** — 지도만 짚으면 앞줄이 뒷줄의 소속을 입어, 위에서 묶음
    // 밑에 센 그 줄이 여기 "어느 묶음에도 안 딸린 것" 에 한 번 더 선다(moai-7iyc.rt6).
    let loose: Vec<&Issue> = issues.iter().filter(|i| is_work(i) && !eclipsed(i) && placed.at(i).is_none()).collect();
    let (counts, total, done, percent) = tally(&loose);
    // **이 줄의 이름은 여기서 안 짓는다**(리뷰) — `id` 가 `None` 인 것이 이미 "어느 묶음에도
    // 안 딸린 것" 이라는 말이고, 화면에 설 낱말은 `view` 가 제 말묶음에서 고른다
    // (`view::tree` 의 `ready.no_epic`). 여기서 지으면 `report` 가 화면 말을 알아야 하고,
    // 그것은 이 층이 `&[Issue]` 에 대한 순수 함수라는 계약(CLAUDE.md)이 막는 자리다.
    out.push(Roll { id: None, title: String::new(), counts, total, done, percent, column: None, deferred: None });
    out
}

/// 지금 집을 수 있는 일.
///
/// 필터 하나로 될 것을 명령으로 두는 이유는 **의견을 한 곳에 박기 위해서**다.
/// 에이전트가 `moai show -s todo --type issue --parent none …` 를 매번
/// 조립하게 두면 조립할 때마다 규칙이 조금씩 달라진다.
pub fn ready<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    ready_in(issues, cfg).0
}

/// 도는 마일스톤이 [`ready`] 에 한 일 — 그 마일스톤들과, 그것 때문에 이번에 **안 낸** 밖의 일.
///
/// **저장하지 않는다.** 둘 다 줄들을 지금 읽어 낸 값이라, 필드로 적는 순간 멤버를 하나 집거나
/// 놓을 때마다 남의 줄을 다시 써야 한다 — beads 가 `is_blocked` 를 컬럼으로 들고 있다가
/// `bd recompute-blocked` 를 만들어야 했던 그 자리다.
#[derive(Debug, Default)]
pub struct Focus<'a> {
    /// 도는 마일스톤의 줄. 없으면 비었고, 그러면 `ready` 는 예전과 같다. **id 가 아니라 줄을
    /// 든다** — 대는 쪽이 제목을 함께 내야 사람이 어느 마일스톤인지 안다.
    pub running: Vec<&'a Issue>,
    /// 도는 마일스톤 밖이라 이번에 안 낸 일. **`p0` 은 안 든다** — 핫픽스는 밖에서도 집는다.
    pub outside: Vec<&'a Issue>,
}

/// 도는 마일스톤 — **멤버에서 읽는다**(moai-493a, 2026-09-20 사용자 결정).
///
/// **읽은 칸이 첫 칸도 `done` 도 아닌 마일스톤**이다 — 묶음의 칸을 정하는 자와 같은 자
/// ([`Stand::column`], `Config::is_started`). 사람이 손으로 여닫는 축을 새로 두지 않는 까닭은
/// 그것이 **칸과 따로 도는 둘째 어휘**여서다 — 여는 것을 잊으면 규칙이 영영 안 서고, 닫는 것을
/// 잊으면 영영 안 풀린다. 칸은 이미 멤버에서 읽으므로 새 필드도 새 명령도 없이 같은 말을 한다.
///
/// **[`Stand::busy`] 로 재지 않는다**(리뷰 moai-493a.2om 1·2). 그쪽은 "지금 누가 손대고 있는가"
/// 라, 집은 멤버를 닫고 다음 멤버를 집기 **전**의 틈에서 꺼진다 — 하필 다음에 무엇을 집을지
/// 고르는 그 순간에 규칙이 사라져 밖의 일이 통째로 돌아왔다. 그 틈에 `moai mv <멤버> done` 이
/// 내는 "이로써 풀림" 줄까지 밖의 일을 전부 세었다. 읽은 칸은 첫 멤버를 집는 순간 서서 마지막
/// 멤버가 닫힐 때까지 그대로다.
///
/// **두 칸짜리 설정에는 도는 마일스톤이 없다.** "시작했지만 안 끝났다" 를 말할 칸이 없어
/// (`Config::started_status`) 이 규칙 자체가 서지 않는다 — 그 설정에서는 `ready` 도 보드도
/// 예전과 같다.
///
/// **미뤄 둔 마일스톤은 안 센다.** 계획에서 뺀 것이 밖의 일을 밀어내면, 미루기가 도리어 규칙을
/// 세우는 손잡이가 된다.
fn running_in<'a>(
    cfg: &Config,
    by_id: &BTreeMap<&'a str, &'a Issue>,
    stands: &BTreeMap<&'a str, Stand<'a, '_>>,
    out_of_plan: &BTreeSet<&str>,
) -> Vec<&'a Issue> {
    stands
        .iter()
        .filter(|(id, s)| cfg.is_started(s.column) && !out_of_plan.contains(*id))
        .filter_map(|(id, _)| by_id.get(id).copied())
        .filter(|m| m.kind == Kind::Milestone)
        .collect()
}

/// [`ready`] 와 같은 것. **목록이 왜 짧은지도 함께 낸다**(moai-q04l).
///
/// 도는 마일스톤이 있으면 그 밖의 일은 `p0` 만 남는다(2026-09-20 사용자 결정) — 밖의 일을
/// 새로 집지 않는다는 규칙이 여기 한 곳에 선다. **막지는 않는다**: 뺀 줄을 [`Focus::outside`]
/// 로 함께 내어 부르는 쪽이 그것을 말할 수 있게 한다. 훅이 집기를 거절하면 그것은 게이트고,
/// 게이트를 안 늘리는 것이 이 도구의 자리다.
pub fn ready_in<'a>(issues: &'a [Issue], cfg: &Config) -> (Vec<&'a Issue>, Focus<'a>) {
    picks_in(issues, cfg, true)
}

/// `moai prime` 한 판이 읽어 낸 것 — **집은 것과 다음에 집을 것**.
///
/// 세션 첫머리와 접힌 뒤에 다시 주입되는 요약이라, 보드([`status`])가 아니라 이것이다.
/// 보드는 13KB 를 넘고 경고·흐름·묶음 막대까지 그리는데, 그 자리가 묻는 것은 "내가 무엇을
/// 쥐고 있었나, 다음은 무엇인가" 둘뿐이다 — 나머지는 읽는 쪽의 맥락을 그만큼 밀어낸다.
///
/// **저장하지 않는다.** 여기 든 것은 전부 지금 줄들에서 읽어 낸 값이고, 고르는 자는 이미
/// 있는 둘([`wip`]·[`ready_in`])이다. 새 자를 세우면 `prime` 이 대는 "다음 일" 과
/// `moai ready` 가 내미는 줄이 갈린다.
#[derive(Debug)]
pub struct Prime<'a> {
    /// 지금 집은 일 — [`wip`] 와 같은 자다.
    pub held: Vec<&'a Issue>,
    /// 다음에 집을 것. [`ready_in`] 의 앞에서 [`PRIME_PICKS`] 개.
    pub picks: Vec<&'a Issue>,
    /// `picks` 에 안 실린 나머지 수. **0 이 아니면 잘렸다는 뜻**이라, 받는 쪽이 이 판을
    /// "집을 것이 셋뿐" 으로 안 읽는다.
    pub rest: usize,
    /// 도는 마일스톤이 목록에 한 일 — [`ready_in`] 이 낸 그대로다.
    pub focus: Focus<'a>,
}

/// `prime` 이 내미는 다음 일의 수. **셋이다** — 요약 한 판의 값은 짧다는 것이고, 더 보는 말은
/// `moai ready` 하나다. 이 자름은 사람 쪽과 `--json` 이 **같이** 쓴다: 사람 화면만 자르면
/// 기계가 읽는 판이 보드만큼 길어져 이 명령이 선 까닭이 사라진다.
///
/// **다만 이 수가 묶는 것은 줄 수지 크기가 아니다**(리뷰). 한때 `--json` 이 줄을 통째로
/// (본문까지) 펴 11KB 였다 — 셋으로 잘렸는데도 사람 쪽의 네 배고 보드에 가까웠다. 크기를
/// 묶는 것은 `cmd::prime::Brief` 이고, 둘이 함께 서야 위의 말이 참이 된다.
pub const PRIME_PICKS: usize = 3;

/// [`Prime`] 을 읽어 낸다.
pub fn prime<'a>(issues: &'a [Issue], cfg: &Config) -> Prime<'a> {
    let (all, focus) = ready_in(issues, cfg);
    let rest = all.len().saturating_sub(PRIME_PICKS);
    Prime { held: wip(issues, cfg), picks: all.into_iter().take(PRIME_PICKS).collect(), rest, focus }
}

/// **막음만 보는 목록** — 도는 마일스톤은 안 본다. [`unblocked`] 가 이것으로 두 판을 견준다.
///
/// 마일스톤 우선은 **막음이 아니다**(리뷰 moai-493a.2om 2). 거른 목록으로 견주면, 마일스톤이
/// 끝나는 순간 밖의 일 전부가 "이로써 풀림" 으로 서서 아무도 안 막던 줄을 막혔던 것으로 말한다.
fn ready_unfocused<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    picks_in(issues, cfg, false).0
}

fn picks_in<'a>(issues: &'a [Issue], cfg: &Config, focused: bool) -> (Vec<&'a Issue>, Focus<'a>) {
    // **소속 지도는 한 벌이다**([`ties`]) — 에픽 지도와 마일스톤 지도를 따로 부르면 뒤의 것이
    // 앞의 것을 제 안에서 다시 짓는다.
    let lines = Lines::of(issues);
    let group = groups_over(issues, &lines);
    let mile_of = milestones_over(issues, &group, &lines);
    // **줄 지도도 한 벌이다** — [`Lines`] 가 이미 든 그 지도다(리뷰 moai-jk2u.m60).
    let by_id = &lines.by_id;
    // **막음과 차례가 한 번의 셈을 쓴다.** 끝나가는지를 롤업으로 따로 재면 미룬 멤버를 세는
    // 자와 안 세는 자가 한 명령 안에 둘이 된다(moai-ha03). 롤업도 같은 걸음을 걸었으므로
    // 늘어나는 셈은 없다. **도는 마일스톤도 같은 셈에서 읽는다** — 따로 재면 `ready` 가 빼는
    // 자와 `status` 가 비추는 자가 갈린다.
    // **줄마다의 답을 먼저 잰다**(moai-u3ta) — 내주는 문은 줄에 묻고([`unblocked_pick`]), 도는
    // 마일스톤을 고르는 자는 id 에 묻는다. 두 답이 한 번의 셈에서 나와야 갈리지 않는다.
    let off = shelved_over(issues, &lines, &group, &mile_of);
    let roots = fold_roots(issues, &off);
    let kind_of = kinds(issues);
    let stands = group_stands_in(issues, cfg, &lines, &group, &mile_of, &kind_of, &off);
    let progress: BTreeMap<&str, u8> = stands.iter().map(|(id, s)| (*id, s.progress.unwrap_or(0))).collect();
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    let running = match focused {
        true => running_in(cfg, by_id, &stands, &out_of_plan),
        false => Vec::new(),
    };
    let (states, waits) = split_stands(stands);
    // **종류 지도는 한 벌이다**(리뷰) — `eclipsed(issues)` 는 제 안에서 [`kinds`] 를 또 짓는데
    // 위에서 이미 하나 지었다. `rollups`·`group_members` 가 같은 자리에서 걷어낸 그것이다.
    let eclipsed = |i: &Issue| is_eclipsed(&kind_of, i);
    let mut out: Vec<&Issue> = issues
        .iter()
        .enumerate()
        // 값싼 막음 검사를 먼저 한다 — `unblocked_pick` 은 자식을 찾느라 목록을 걷는다.
        .filter(|(k, i)| !is_blocked(i, by_id, &states, &waits) && unblocked_pick(i, *k, issues, cfg, &off, &eclipsed))
        .map(|(_, i)| i)
        .collect();

    // **도는 마일스톤 안인가.** 소속은 물려받은 것까지다(`milestones`) — 에픽의 마일스톤이
    // 그 멤버의 것이므로, 이슈마다 마일스톤을 다시 적은 저장소가 아니어도 선다.
    //
    // **소속은 [`Placed`] 에 묻는다**(리뷰) — 이 문이 지도를 곧바로 짚던 때는, `moai show <마일스톤>`
    // 이 `멤버 0/2` 로 세어 그린 그 줄을 `moai ready` 가 `밖에 남았다` 며 안 냈다. 배정의 문이라
    // 한 번 틀리면 일이 안 나간다.
    let in_stone = Placed::milestone(&group, &lines);
    let inside = |i: &Issue| in_stone.at(i).is_some_and(|m| running.iter().any(|r| r.id == m));
    // **`p0` 은 마일스톤과 무관하게 남는다**(사용자 결정 2) — 핫픽스 자리다. 그 자리가 없으면
    // 마일스톤이 도는 동안 밖에서 터진 것을 고칠 길이 도구 밖에만 남는다.
    let mut outside = Vec::new();
    if !running.is_empty() {
        let (picks, held) = out.into_iter().partition(|i| inside(i) || i.priority() == 0);
        out = picks;
        outside = held;
    }

    // 급한 것 → 도는 마일스톤 안 → 끝나가는 에픽 → 오래된 것. 끝나가는 것을 먼저 집어야
    // 벌여 놓은 에픽이 줄어든다.
    //
    // **급한 것이 먼저고 마일스톤은 그다음이다.** 뒤집으면 밖에 선 `p0` 핫픽스가 마일스톤 안의
    // `p2` 밑으로 내려간다 — 핫픽스를 밖에서도 집기로 한 결정이 차례에서 도로 무너진다.
    // **에픽도 줄마다 묻는다**(리뷰, [`joined_in`]) — 지도를 곧바로 짚으면 같은 id 의 앞줄이
    // 뒷줄의 에픽으로 줄을 서, 화면이 대는 소속과 차례를 정하는 소속이 갈린다.
    let pct = |i: &Issue| {
        joined_in(i, || group.get(i.id.as_str()).copied()).and_then(|e| progress.get(e)).copied().unwrap_or(0)
    };
    out.sort_by(|a, b| {
        let (pa, pb) = (pct(a), pct(b));
        a.priority()
            .cmp(&b.priority())
            .then_with(|| inside(b).cmp(&inside(a)))
            .then_with(|| pb.cmp(&pa))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    outside.sort_by(|a, b| a.priority().cmp(&b.priority()).then_with(|| a.id.cmp(&b.id)));
    (out, Focus { running, outside })
}

/// **이로써 풀린 일** — 쓰기 전(`before`)에는 [`ready`] 가 아니었고 쓴 뒤(`after`)에는
/// ready 인 일(moai-942k). `moai mv <id> done` 이 한 줄로 댄다.
///
/// **고르는 자는 `ready` 하나다.** 막음·미룸·열린 자식·묶음 칸의 규칙을 여기서 다시 재면
/// `ready` 가 내는 것과 이 줄이 대는 것이 갈린다 — 두 번 불러 견준다. 차례는 `after` 의
/// `ready` 차례 그대로다. **저장하지 않는다** — 막힌 줄의 "풀렸나" 는 막는 줄을 닫을
/// 때마다 달라지는 파생값이다.
pub fn unblocked<'a>(before: &[Issue], after: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    let was: BTreeSet<&str> = ready_unfocused(before, cfg).into_iter().map(|i| i.id.as_str()).collect();
    ready_unfocused(after, cfg).into_iter().filter(|i| !was.contains(i.id.as_str())).collect()
}

/// 닫는 쓰기가 **연 것 셋** — 이로써 집을 수 있게 된 일, 이제 닫을 수 있는 부모, 같은
/// 에픽의 다음 일(moai-j4xs). `moai mv <id> done` 이 한 줄씩 댄다.
///
/// **셋이 겹치지 않는다.** [`unblocked`] 는 첫 칸의 줄만 세고([`ready`]), [`Freed::closable`] 은
/// 이미 시작한 칸의 줄만 세며, [`Freed::next`] 는 [`Freed::unblocked`] 에 든 id 를 뺀다 —
/// 한 줄이 두 자리에서 두 번 불리면 읽는 쪽이 그것을 두 건으로 센다.
///
/// **저장하지 않는다.** 셋 다 두 스냅샷을 지금 견준 값이다 — 필드로 적으면 이슈 A 를 닫을
/// 때 A 이외의 줄을 써야 하고, 그것이 beads 의 `is_blocked`·`bd recompute-blocked` 다.
#[derive(Debug)]
pub struct Freed<'a> {
    /// 쓰기 전에는 [`ready`] 가 아니었고 쓴 뒤에는 ready 인 일 — [`unblocked`] 그대로다.
    pub unblocked: Vec<&'a Issue>,
    /// **이제 닫을 수 있는 부모** — 마지막 안 끝난 일 자식이 이 쓰기로 닫혔는데, 부모가
    /// 이미 시작한 칸에 서 있어 [`unblocked`] 에는 안 드는 줄.
    ///
    /// 묶음(에픽·마일스톤)은 여기 안 든다 — 칸을 멤버에서 읽어 저절로 서고, 그것은
    /// `mv` 가 `stands` 로 이미 댄다. 여기 드는 것은 **제 칸을 제가 드는 부모 이슈**뿐이다.
    pub closable: Vec<&'a Issue>,
    /// 닫은 줄과 **같은 에픽에서 다음에 집을 것.** 이미 ready 이던 줄이라 두 판을 견주는
    /// [`unblocked`] 에는 안 드는데, 멤버 하나를 닫은 자리에서 가장 자주 묻는 것이 이것이다.
    /// 에픽마다 하나다 — 목록을 내는 것은 `moai ready` 의 일이다.
    pub next: Vec<&'a Issue>,
}

/// [`Freed`] 를 읽어 낸다. `closed` 는 이 쓰기가 실제로 `done` 으로 옮긴 id 들이다.
///
/// **[`ready`] 를 세 번 센다**(`before` 한 번, `after` 를 거르개 없이·있이 각각 한 번). 닫는
/// 쓰기에서만 도는 자리라 그 값을 치른다 — [`Freed::next`] 가 도는 마일스톤을 봐야 하고
/// ([`ready_in`]), [`unblocked`] 는 봐서는 안 되기 때문이다([`ready_unfocused`]): 마일스톤이
/// 끝나는 순간 밖의 일 전부가 "풀림" 으로 서면 아무도 안 막던 줄을 막혔던 것으로 말한다.
pub fn freed<'a>(before: &[Issue], after: &'a [Issue], cfg: &Config, closed: &[&str]) -> Freed<'a> {
    let unblocked = unblocked(before, after, cfg);
    Freed { closable: closable(before, after, cfg), next: next_of(after, cfg, closed, &unblocked), unblocked }
}

/// 마지막 안 끝난 일 자식이 이 쓰기로 닫힌 부모 — **이미 시작한 칸에 선 것만**.
///
/// 첫 칸의 부모는 [`unblocked`] 가 이미 낸다([`unblocked_pick`] 의 `has_open_child`). 여기서
/// 그것까지 세면 같은 줄이 두 번 불린다.
fn closable<'a>(before: &[Issue], after: &'a [Issue], cfg: &Config) -> Vec<&'a Issue> {
    // **미룬 자식은 안 끝난 자식이 아니다** — `unblocked_pick` 이 `ready` 에서 쓰는 자와 같다.
    // 이것을 안 맞추면 미룬 자식 하나가 남은 부모를 여기서는 "닫을 수 있다" 로, `ready` 에서는
    // "아직 자식이 있다" 로 말한다. **그래서 여기도 줄에 묻는다**(리뷰 moai-jk2u.hr4) — 그쪽이
    // 자리로 옮겨 갈 때 여기만 접은 지도에 남으면 그 약속이 글로만 남는다. 술어는 아예 한
    // 몸이다([`has_open_child`]) — 글로 맺은 약속을 부르는 자리로 옮긴다.
    let (was_off, now_off) = (shelved(before), shelved(after));
    // **가려진 줄은 여기 안 든다** — `unblocked_pick` 과 [`wip`] 가 그 줄을 집은 일로 안 세는
    // 것과 같은 자다(moai-es40). 안 맞추면 쌍둥이에게 자리를 뺏긴 부모에게 "닫으면 된다" 고
    // 대는데, 그 id 는 `duplicate_id` 로 파일째 쓰기가 막혀 있어 시킨 명령을 도구가 제 손으로
    // 거절한다 — 덫을 하나 놓는 일이다.
    let eclipsed = eclipsed(after);
    after
        .iter()
        .enumerate()
        .filter(|(k, p)| {
            is_work(p)
                && now_off[*k].is_none()
                && !eclipsed(p)
                && cfg.is_started(p.status.as_str())
                && !has_open_child(after, &p.id, &now_off)
                && has_open_child(before, &p.id, &was_off)
        })
        .map(|(_, p)| p)
        .collect()
}

/// 닫은 줄들의 에픽마다 **다음에 집을 것 하나**. 차례는 [`ready_in`] 의 차례 그대로다.
fn next_of<'a>(after: &'a [Issue], cfg: &Config, closed: &[&str], said: &[&'a Issue]) -> Vec<&'a Issue> {
    // **소속을 먼저 묻고 차례는 나중에 잰다.** 에픽 없는 줄을 닫은 것은 여기서 할 말이
    // 없는데([`groups`] 에 안 든다), 차례를 먼저 재면 그 판이 빈 답을 내려고 스냅샷을 한 번
    // 더 걷는다 — 닫는 쓰기는 락을 쥔 자리라 헛걸음 한 판이 그대로 락 시간이다.
    let epic_of = groups(after);
    // **닫은 줄의 소속도 줄마다 묻는다**(리뷰, [`joined_in`]) — 지도만 짚으면 제 `epic` 을 적은
    // 앞줄을 닫았을 때 뒷줄의 빈 값이 나와, `moai show <에픽>` 이 멤버로 세는 그 줄을 닫고도
    // "이 에픽의 다음" 이 아무 말도 안 한다. 아래 고르는 자와 한 자를 쓴다.
    // **닫은 줄 하나에 에픽 하나다**(리뷰) — `cmd::mv` 가 닫는 것은 그 id 의 **앞줄**이므로
    // (`issues.iter_mut().find`) 여기서 찾는 것도 앞줄이다. 그 id 를 든 줄을 모두 걷으면 한 번
    // 닫은 일에 `이 에픽의 다음` 이 둘 선다.
    let epics: BTreeSet<&str> = closed
        .iter()
        .filter_map(|id| after.iter().find(|i| i.id.as_str() == *id))
        .filter_map(|i| joined_in(i, || epic_of.get(i.id.as_str()).copied()))
        .collect();
    if epics.is_empty() {
        return Vec::new();
    }
    let (picks, _) = ready_in(after, cfg);
    let already: BTreeSet<&str> = said.iter().map(|i| i.id.as_str()).collect();
    // **한 줄은 한 에픽에만 든다**([`groups`] 는 id 마다 에픽 하나를 낸다). 그래서 서로 다른
    // 에픽이 같은 줄을 고를 수 없고, 겹침을 거르는 자리는 위의 `already` 하나뿐이다 —
    // `unblocked` 에 이미 선 줄을 또 대지 않는 것이 실제로 막아야 할 겹침이다.
    epics
        .into_iter()
        .filter_map(|e| {
            // **소속은 줄마다 묻는다**(리뷰, [`joined_in`]) — `ready` 가 고른 줄을 여기서 지도로
            // 다시 가르면 같은 id 의 앞줄이 뒷줄의 에픽으로 딸려 와, 안 그 에픽인 줄을 댄다.
            picks
                .iter()
                .find(|p| {
                    joined_in(p, || epic_of.get(p.id.as_str()).copied()) == Some(e) && !already.contains(p.id.as_str())
                })
                .copied()
        })
        .collect()
}

/// 막음을 재는 데 드는 것 — 계획에서 빠진 줄(뺀 곳과 함께)과, 막는 묶음의 읽은 칸.
///
/// **필요할 때만 센다.** 미룬 줄이 없으면 물려받을 것도 없고, 묶음에 막힌 줄이 하나도
/// 없으면 읽은 칸을 물을 자리가 없다 — 읽은 칸을 묻는 것은 `is_blocked` 뿐이고, 그
/// 밖의 줄은 제 칸으로 답한다(`column`). 둘 다 조상과 소속을 타는 셈이라, 안 묻는
/// 저장소에서 그냥 돌리면 `moai ready` 가 부를 때마다 목록을 여러 벌 더 걷는다.
///
/// **제가 지은 마일스톤 지도를 함께 낸다**(moai-g0zx) — 뒤에 같은 지도가 또 필요한 쪽([`held`] 의
/// [`deferred_sources_in`])이 밖에서 미리 지으면, 여기가 안에서 한 벌을 더 지어 한 명령에 둘이 된다.
/// 위의 빠른 길로 돌아설 때는 비어 있는데, 그때는 `roots` 도 비어 받는 쪽이 그것으로 아무것도 안 짓는다.
fn blocking<'a, 'c>(
    issues: &'a [Issue],
    cfg: &'c Config,
    epic_of: &BTreeMap<&'a str, &'a str>,
    by_id: &BTreeMap<&'a str, &'a Issue>,
) -> (Vec<Option<&'a str>>, BTreeMap<&'a str, &'c str>, Waits<'a>, BTreeMap<&'a str, &'a str>) {
    let shelved = issues.iter().any(is_put_off);
    let by_group =
        issues.iter().any(|i| i.blocked_by.iter().any(|b| by_id.get(b.as_str()).is_some_and(|x| is_group(x))));
    if !shelved && !by_group {
        return (vec![None; issues.len()], BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    }
    // **받은 에픽 지도 위에 쌓는다** — `milestones` 를 그냥 부르면 그 안에서 `groups` 를 한 벌
    // 더 지어, 지도를 건네받은 보람이 없다([`milestones_in`]).
    let lines = Lines::of(issues);
    let mile_of = milestones_over(issues, epic_of, &lines);
    // **줄마다 잰다**(moai-u3ta) — 내주는 문([`unblocked_pick`])은 줄에 묻고, 묶음 칸과 막는
    // 줄을 대는 자는 id 에 묻는다. 접는 것은 한 번이라 걷는 걸음이 늘지 않는다.
    let off = shelved_over(issues, &lines, epic_of, &mile_of);
    let (states, waits) = if by_group {
        split_stands(group_stands_in(issues, cfg, &lines, epic_of, &mile_of, &kinds(issues), &off))
    } else {
        (BTreeMap::new(), BTreeMap::new())
    };
    (off, states, waits, mile_of)
}

/// 막음만 빼면 집을 수 있는가. `ready` 와 `held` 가 **같은 자로** 고른다 —
/// 둘이 따로 고르면 `held` 가 댄 줄이 막음을 풀어도 `ready` 에 안 올라온다.
///
/// **가려진 줄([`eclipsed`])은 집을 일이 아니다** (moai-lg2t). 그 줄은 트리에서
/// `(길 잃음)` 에 서고 어느 묶음에도 안 드는데, `ready` 만 그것을 집으라고 내밀며
/// 에픽 칸에 `에픽 없음` 을 달았다. 그 id 는 `duplicate_id` 로 파일째 쓰기가 막혀
/// 집어도 `mv` 가 거절한다 — 자리 없는 줄을 내밀 까닭이 없다.
///
/// **계획 밖인가는 줄에 묻는다**(moai-u3ta) — `shelved` 는 `issues` 와 자리가 같은 배열이고
/// `at` 은 물은 줄의 자리다. 접은 지도를 짚던 때는 같은 id 의 앞줄이 미룬 에픽에 들었다는
/// 이유로 산 에픽에 든 뒷줄이 `ready` 에서 사라졌고, 그 줄은 `held` 에도 안 서서 도구 안에서는
/// 왜 사라졌는지 볼 길이 없었다.
fn unblocked_pick(
    i: &Issue,
    at: usize,
    issues: &[Issue],
    cfg: &Config,
    shelved: &[Option<&str>],
    eclipsed: &impl Fn(&Issue) -> bool,
) -> bool {
    // **끝난 에픽인지는 묻지 않는다.** 묶음의 칸은 멤버에서 읽으므로(`group_states`)
    // 읽은 칸이 done 인 묶음에는 집을 멤버가 이미 없고, 적힌 칸으로 물으면 손으로
    // done 에 둔 에픽의 남은 일이 까닭 없이 `ready` 에서 사라진다(moai-j3b3).
    // 자식이 남아 있으면 부모는 직접 하는 일이 아니다. **일만 센다** —
    // 이슈 밑에 담아 둔 생각 하나가 그 이슈를 `ready` 에서 지워 버리는데,
    // idea 는 어느 목록에도 안 나오므로 왜 사라졌는지 볼 방법이 없다.
    is_work(i)                                   // 묶음도 생각도 집는 게 아니다
        && shelved[at].is_none()                 // 미뤄 둔 것과 그 밑도
        && !eclipsed(i)                          // 쌍둥이에게 자리를 뺏긴 줄도
        && i.status.as_str() == cfg.first_status()
        && !has_open_child(issues, &i.id, shelved)
}

/// 그 줄 밑에 **아직 안 끝난 일 자식**이 있는가 — 미룬 자식은 안 끝난 자식이 아니다.
///
/// **[`unblocked_pick`] 과 [`closable`] 이 한 몸을 쓴다**(리뷰 moai-jk2u.hr4). 둘이 안 맞으면
/// 미룬 자식 하나가 남은 부모를 한쪽에서는 "닫을 수 있다" 로, `ready` 에서는 "아직 자식이
/// 있다" 로 말한다. 글로만 맺어 두던 약속이라, 한쪽만 줄로 옮기고 다른 쪽을 지도에 두어도
/// 컴파일에 안 걸렸다.
///
/// **자리로 묻는다** — `shelved` 는 `issues` 와 자리가 같은 배열이다. [`children_of`] 를 쓰지
/// 않는 것은 그쪽이 `Vec` 을 지어 차례까지 매기기 때문이다: 여기는 하나만 찾으면 끝난다.
fn has_open_child(issues: &[Issue], id: &str, shelved: &[Option<&str>]) -> bool {
    issues.iter().enumerate().any(|(k, c)| {
        crate::id::parent_of(&c.id) == Some(id) && is_work(c) && shelved[k].is_none() && !c.status.is_done()
    })
}

/// 미뤄 둔 것에 막혀 못 집는 일 하나와, 그것을 막는 미뤄 둔 줄들.
#[derive(Debug)]
pub struct Held<'a> {
    pub issue: &'a Issue,
    pub by: Vec<&'a str>,
    /// `by` 를 풀려면 도로 집어야 할 줄 — 막는 줄이 미룬 에픽 밑이면 그 에픽.
    pub undo: Vec<&'a str>,
    /// 막는 것 중 **멤버가 하나도 없는 묶음**([`Blocker::Empty`]). 도로 집을 것이 없으니
    /// `undo` 에 안 든다 — 채우거나 막음을 풀어야 풀린다.
    pub empty: Vec<&'a str>,
}

/// 막음만 아니면 집을 일인데, **안 끝난 막음 중 하나라도 미뤄 둔 것**인 줄.
///
/// 미룬 막음을 무시하지 않는다 — 미룬 일은 끝난 일이 아니라, 무시하면 안
/// 끝난 일 위에 선 것을 집으라고 내민다. 그렇다고 입을 다물면 `ready` 가
/// 까닭 없이 비고, 막는 줄은 어느 목록에도 없어 풀 길이 안 보인다.
/// 그래서 **드러내되 고르지는 않는다.** 도로 집을지 막음을 풀지는 사람 몫이다.
pub fn held<'a>(issues: &'a [Issue], cfg: &Config) -> Vec<Held<'a>> {
    // 막음이 하나도 없으면 막혀 못 집는 일도 없다 — 소속 지도를 안 세운다.
    if !issues.iter().any(|i| !i.blocked_by.is_empty()) {
        return Vec::new();
    }
    let group = groups(issues);
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // **마일스톤 지도도 `blocking` 에게서 받는다** — 밖에서 [`ties`] 로 미리 지으면 `blocking` 이
    // 제 안에서 같은 것을 한 벌 더 짓는다.
    let (off, states, waits, mile_of) = blocking(issues, cfg, &group, &by_id);
    // 막는 줄은 **id 로** 불린다(`blocked_by`) — 그쪽 답은 접은 지도의 것이다(moai-u3ta).
    let roots = fold_roots(issues, &off);
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    // **도로 집을 곳은 방금 잰 것으로 짓는다** — [`deferred_sources`] 를 그냥 부르면 소속 지도
    // 두 벌과 `blocking` 이 이미 낸 `roots` 를 통째로 다시 짓는다.
    let sources = deferred_sources_in(issues, &group, &mile_of, &roots);
    let eclipsed = eclipsed(issues);
    // 값싼 막음 검사를 먼저 한다. `unblocked_pick` 은 자식을 찾느라 목록을
    // 한 번 걷는다 — 모든 줄에 먼저 부르면 `ready` 가 부를 때마다 제곱이다.
    let mut out: Vec<Held> = issues
        .iter()
        .enumerate()
        .filter_map(|(k, i)| {
            let (by, empty) = holding(i, &by_id, &out_of_plan, &states, &waits);
            (!by.is_empty() || !empty.is_empty()).then_some((k, i, by, empty))
        })
        .filter(|(k, i, _, _)| unblocked_pick(i, *k, issues, cfg, &off, &eclipsed))
        .map(|(_, i, by, empty)| {
            // **풀어야 할 미룸을 다 댄다**(moai-g2a1). 가까운 하나만 대면 그것을 풀고도 여전히
            // 막힌 채 그제야 다음을 댄다(moai-phzi). 제가 미뤄진 묶음이 멤버도 다 미뤘으면
            // 묶음만 풀어서는 이번엔 그 멤버에 막히므로, 뺀 멤버의 미룸까지 댄다.
            let mut undo: Vec<&str> = by
                .iter()
                .flat_map(|b| {
                    let aside = match waits.get(b) {
                        Some((Waiting::Shelved, aside)) => aside.as_slice(),
                        _ => &[],
                    };
                    std::iter::once(*b).chain(aside.iter().copied())
                })
                .flat_map(|x| sources.get(x).into_iter().flatten().copied())
                .collect();
            undo.sort_unstable();
            undo.dedup();
            Held { issue: i, by, undo, empty }
        })
        .collect();
    out.sort_by(|a, b| crate::query::display_order(a.issue, b.issue));
    out
}

/// `i` 를 막는 것 중 **까닭을 따로 대야 하는 것** — (안 끝났고 계획에서 빠진 것, 멤버가
/// 없는 묶음).
///
/// 막는 것이 미뤄 뺀 멤버만 기다리는 묶음이면 **묶음 대신 그 멤버를** 댄다 — 도로 집을
/// 곳은 그 멤버의 미룸이지 미룬 적 없는 묶음이 아니다. 둘로 한 멤버에 닿으면 한 번만 댄다.
fn holding<'a>(
    i: &Issue,
    by_id: &BTreeMap<&str, &'a Issue>,
    out_of_plan: &BTreeSet<&str>,
    states: &BTreeMap<&str, &str>,
    waits: &Waits<'a>,
) -> (Vec<&'a str>, Vec<&'a str>) {
    let (mut by, mut empty): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    for x in i.blocked_by.iter().filter_map(|b| by_id.get(b.as_str()).copied()) {
        let id = x.id.as_str();
        let wait = waits.get(id);
        let waiting = wait.map_or(Waiting::Live, |w| w.0);
        match blocker(Some(column_by_id(x, states)), out_of_plan.contains(id), waiting) {
            Blocker::Deferred => {
                // 묶음을 제가 미뤘으면 그 묶음을 댄다([`blocker`] 와 같은 순서).
                let named = match wait {
                    Some((Waiting::Shelved, aside)) if !out_of_plan.contains(id) => aside.clone(),
                    _ => vec![id],
                };
                for n in named {
                    if !by.contains(&n) {
                        by.push(n);
                    }
                }
            }
            Blocker::Empty => empty.push(id),
            Blocker::Open | Blocker::Done | Blocker::Missing => {}
        }
    }
    (by, empty)
}

// ── moai status ──────────────────────────────────────────────────────
//
// 게이트를 없앤 자리를 메우는 것이 이 리포트 하나다. **아무것도 막지 않는다**
// — 막기 시작하면 그게 게이트고, 이전 시도가 정확히 그것으로 죽었다.
//
// 임계값은 `.moai/config.toml` 의 `status_*` 키다([`crate::config::Thresholds`], moai-pz7h).
// 한 줄도 안 적으면 옛 상수 그대로고, 여기 있던 이름 붙인 상수는 그 기본값으로 옮겨 갔다.
// **설정으로 뺐어도 막는 것은 여전히 없다** — 값을 낮춰 잔소리를 늘려도 종료 코드는 그대로다.

// 막힌 채로 며칠 서 있으면 "계획이 멈춘 자리" 인가(`status_blocked_days`). 재는 것은 [`blocked_since`] —
// 제 계획 자리가 바뀐 때와 **지금 막는 줄이 다시 선 때** 중 늦은 것이다(moai-xib6).
// `blocked_by` 를 적은 시각은 여전히 안 잰다 — 오래된 두 줄 사이에 오늘 막음을 걸면
// 곧장 선다. 그것을 재려면 그 시각을 스냅샷에 적어야 하는데, 링크 하나에 시각을 달
// 만큼 거슬린 적이 아직 없다.
//
// **소속을 옮긴 때도 안 잰다**(moai-bbzg, 사용자와 정함). `moai edit <id> -e <끝난 에픽>` 으로
// 옛 일을 넣어 에픽이 다시 열려도 그 에픽의 [`Stand::since`] 는 멤버의 옛 칸 시각이라, 그 에픽에
// 막힌 줄이 곧장 선다 — `moai add` 로 새 멤버를 만들 때만 새로 선다. 소속 이동 시각을 세면 멤버를
// 뺐다 도로 넣는 것만으로 막힘 시계가 0일로 돌아가, 미루기에서 막은 손잡이(moai-cxk8)가 소속
// 쪽에 다시 생긴다.

/// 막힌 줄이 **지금 막힌 채로 선 때** — 제 칸을 옮긴 때(`status_since`)와, 지금 막는
/// 줄들이 다시 선 때 중 **가장 이른 것** 가운데 늦은 것.
///
/// 막는 줄 쪽은 가장 이른 것을 쓴다 — 오래 막아 온 줄이 하나라도 있으면 그동안 줄곧 막혀
/// 있었다. 가장 늦은 것을 쓰면 막는 줄 둘 중 하나가 오늘 칸을 옮기는 것만으로 열흘 막힌
/// 줄이 경고에서 사라진다.
///
/// 막는 줄이 선 때는 일이면 그 줄의 칸을 옮긴 때(done 에서 되돌아 나왔으면 그때), 묶음이면
/// 읽은 칸의 셈이 움직인 때([`Stand::since`])다. 막힌 줄의 칸 나이로만 재면, 끝난 에픽에
/// 멤버를 더하는 순간 그 에픽에 막힌 오래된 줄이 곧장 "N일째" 로 섰다(moai-xib6).
///
/// **미루거나 도로 집은 때(`planned_at`)는 어느 쪽에서도 안 센다**(moai-cxk8). 세면 막힌
/// 줄이나 막는 줄을 `defer`→`--undo` 하는 것만으로 열흘 막힘이 0일로 돌아간다 —
/// `planned_at` 을 `status_since` 와 따로 둔 까닭(미루기가 방치 경고를 지우는 손잡이)이
/// 경고 쪽으로 돌아온다.
/// - 막는 쪽: 미룬 막음도 막으므로(moai-2sea) 미뤄 둔 동안에도 막힘은 이어졌고
///   `blocked_by_deferred` 로 드러나 있었다. 그래서 미뤘던 묶음을 도로 집으면 그것에 막힌
///   줄은 곧장 "N일째" 로 선다 — 그 막힘은 실제로 이어졌던 것이다.
/// - 막힌 줄 제 쪽: 미뤄 둔 줄은 계획 밖이라 경고에 안 드는데, 그동안에도 막음은 안
///   풀렸다. 도로 집는 순간 곧장 "N일째" 로 서는 것이 참말이다.
///
/// **파생값이라 저장하지 않는다** — 막는 줄을 옮길 때 막힌 줄을 같이 쓰게 된다.
fn blocked_since<'a>(
    i: &'a Issue,
    by_id: &BTreeMap<&str, &'a Issue>,
    states: &BTreeMap<&str, &str>,
    waits: &Waits,
    group_since: &BTreeMap<&str, &'a str>,
) -> &'a str {
    i.blocked_by
        .iter()
        .filter_map(|b| by_id.get(b.as_str()).copied())
        .filter(|x| blocker(Some(column_by_id(x, states)), false, waiting_of(&x.id, waits)).blocks())
        .map(|x| match is_group(x) {
            true => group_since.get(x.id.as_str()).copied().unwrap_or(x.created_at.as_str()),
            false => x.status_since.as_str(),
        })
        .min()
        .map_or(i.status_since.as_str(), |b| b.max(i.status_since.as_str()))
}
/// 줄이 든 시각 일곱 가운데 하나라도 지금보다 [`FUTURE_SLACK_SECS`] 넘게 뒤인가.
///
/// 도구는 제 시계로만 적으므로 그런 시각은 손으로 고친 줄이나 크게 틀린 시계에서 온다.
/// 나이는 0 아래로 안 내려가서(`days_since`, moai-fix6) 목록에서는 "오늘" 로 숨는다.
/// 못 읽는 시각은 여기서 따지지 않는다 — 읽기는 관대하다.
///
/// **시작·끝 시각도 본다**(moai-38mh). 시작은 한 번 적고 안 덮으므로, 틀린 시계로 집은 줄은
/// 다음 이동이 `status_since`·`updated_at` 을 바로잡은 뒤에도 그 값만 남아 소요를 음수로 만든다.
fn far_ahead(i: &Issue, now: &str) -> bool {
    let Some(now) = crate::model::parse_rfc3339(now) else { return false };
    [
        Some(i.created_at.as_str()),
        Some(i.updated_at.as_str()),
        Some(i.status_since.as_str()),
        i.deferred_at.as_deref(),
        i.planned_at.as_deref(),
        i.started_at.as_deref(),
        i.done_at.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(crate::model::parse_rfc3339)
    .any(|t| t - now > FUTURE_SLACK_SECS)
}

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
    /// id → **판정에 쓴 날수**. 날짜로 거는 경고(`stale_review`·`blocked_stale`·
    /// `stale_progress`)와, 나이가 제 뜻을 갖는 `blocked_by_deferred`(막는 쪽을 **미룬 지**
    /// 며칠 — 지금 할 일이 지금 안 할 일을 기다린 날수, moai-hcx3)와, 기한으로 거는
    /// `milestone_overdue`·`milestone_due_soon` 만 싣는다. 안 실린 경고의 나이 열은 칸 나이다.
    ///
    /// **앞의 넷은 지나간 날수(양수)이고 기한 둘은 남은 날수다**(moai-tfcp) — 지난 기한은
    /// 음수다([`crate::model::days_until`] 과 같은 부호). 부호로 갈리므로 받는 쪽이 `kind` 를
    /// 같이 안 봐도 그 수 하나로 앞뒤를 읽는다: 크기만 싣고 갈래로 뜻을 가르면 "2일 지남" 과
    /// "2일 남음" 이 `--json` 에서 같은 값이 된다(리뷰).
    ///
    /// 보이는 쪽이 나이를 새로 재면 판정과 표시가 갈라진다 — `blocked_stale` 은 막음이 다시
    /// 선 때([`blocked_since`])로 재는데 목록이 제 칸 나이를 내면, 칸에 30일 선 줄이 "3일
    /// 넘게 막힘" 밑에 "30일" 로 섰다(moai-7azq). 잰 자리가 싣고 보이는 쪽은 읽기만 한다.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub ages: BTreeMap<String, i64>,
    /// 고칠 것이 아니라 알려 주는 것. **`fatal` 옆에 데이터로 둔다** —
    /// 이 판단이 `view` 에만 있으면 `view` 를 건너뛴 표면(TUI·`--json`)이
    /// 알림을 경고로 세고, 담을수록 화면이 시끄러워진다.
    pub notice: bool,
    /// 데이터가 깨진 것. **이것만 비영 종료한다.**
    pub fatal: bool,
}

impl Warning {
    /// **id 는 한 번씩만 담는다** — 처음 나온 자리(급한 차례, [`ids_of`])를 지킨다.
    ///
    /// 경고가 가리키는 것은 줄이 아니라 id 다. 사람이 고치러 부르는 손잡이가 id 뿐이고,
    /// 같은 id 의 줄이 둘이면(`duplicate_id`) 줄마다 id 지도를 묻는 판정이 그 id 를 두 번
    /// 담아 `N건` 이 줄 수로 부풀었다(moai-ddtg). 거르는 자리를 경고마다 두면 새 경고가
    /// 그것을 잊으므로 여기 한 곳에 둔다. 중복 줄 자체는 `duplicate_id` 가 따로 말한다.
    fn new(kind: &'static str, ids: Vec<String>) -> Warning {
        let mut seen = BTreeSet::new();
        let ids: Vec<String> = ids.into_iter().filter(|id| seen.insert(id.clone())).collect();
        Warning {
            kind,
            count: ids.len(),
            ids,
            days: None,
            limit: None,
            ratio: None,
            hint: None,
            oldest: None,
            ages: BTreeMap::new(),
            notice: false,
            fatal: false,
        }
    }
    /// 줄마다 판정에 쓴 나이를 싣는다. **잰 시각을 받는다** — 판정과 같은 시각으로 재야
    /// 표시가 판정과 안 갈라진다. 담긴 id 만 싣는다.
    ///
    /// **같은 id 의 줄이 둘이면 파일에서 뒤의 줄이 이긴다** — 보이는 쪽의 id 지도
    /// (`view::status` 의 `by_id`)가 뒷줄의 칸·제목을 내므로, 앞줄 나이를 실으면 한 줄에
    /// 두 줄의 값이 섞인다. 겹친 id 자체는 `duplicate_id` 가 따로 말한다.
    fn ages<'x>(mut self, of: impl Fn(&'x Issue) -> &'x str, rows: &[&'x Issue], now: &str) -> Warning {
        for i in rows {
            if let Some(d) = days_since(of(i), now) {
                self.ages.insert(i.id.clone(), d);
            }
        }
        self
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

    /// AGENTS.md 의 관리 블록이 이 바이너리가 쓸 글에서 낡았다는 **알림**(moai-mj45).
    ///
    /// 이슈에서 오는 말이 아니라 [`status`] 가 만들지 않는다 — 여기는 `&[Issue]` 만 받는 순수
    /// 함수라 파일을 안 읽는다. 읽는 쪽(`cmd::init::agents_notice`)이 재고, `status` 와 훅의 보드가
    /// 이것을 `notices` 에 얹는다. 모양을 여기 두는 것은 알림의 낱말(`kind`·`hint`)이 한 곳에 서게
    /// 해서다. **경고가 아니다**: 낡은 안내는 고칠 일이 아니라 다시 심을 일이고, Stop 훅이 세는
    /// `warnings` 에 들면 도구를 새로 빌드할 때마다 세션이 붙들린다.
    ///
    /// `root` 는 부른 사람의 셸이 뿌리에 있지 않을 때의 뿌리(셸에 붙여 넣을 모양)다 — `init` 은
    /// 부른 자리에 심으므로 고칠 명령이 `-C` 로 거기를 댄다.
    /// `edited` 면 블록 안을 사람이 고친 것이고, 아니면 어떤 바이너리가 쓴 그대로다 — **낱말이
    /// 갈린다**(2026-09-15 사용자 결정). 뭉뚱그려 `moai init` 만 대면, main 을 받고 아직 다시
    /// 빌드 안 한 세션이 그 말을 따라 **새 안내를 옛 글로 되돌리고** 그 되돌림이 머지로 실린다.
    /// 기계도 가르라고 `kind` 를 따로 둔다.
    /// 사용자 설정에서 못 읽은 것(리뷰 moai-80qw). **셈만 든다** — 어느 파일의 어느 값인지는
    /// 부르는 쪽이 이미 stderr 로 한 줄씩 냈다. 보드에 그 긴 줄을 또 실으면 표를 밀어낸다.
    pub fn user_config(n: usize) -> Warning {
        Warning::new("user_config", Vec::new()).count(n).notice()
    }

    pub fn agents_stale(root: Option<&str>, edited: bool) -> Warning {
        let kind = if edited { "agents_hand_edited" } else { "agents_stale" };
        Warning::new(kind, Vec::new()).count(1).notice().hint(&Warning::init_hint(root))
    }

    /// 딸린 파일(`.gitignore`·`.gitattributes`)에 moai 가 쓰는 규칙이 빠졌다는 **알림**
    /// (moai-2f99). 재는 쪽은 `cmd::init::dotfile_gaps` 고, `status` 와 훅의 보드가 이것을
    /// `notices` 에 얹는다 — [`status`] 는 `&[Issue]` 만 받는 순수 함수라 파일을 안 읽는다.
    ///
    /// **파일마다 `kind` 가 다르다**(부르는 쪽이 고른다). 빠졌을 때의 결과가 아주 달라
    /// (`.gitignore` 는 옆 워크트리가 `git add -A` 에 딸려가는 일, `.gitattributes` 는 저널이
    /// 머지에서 충돌하는 일) 한 낱말로 뭉치면 그 중 한쪽이 반드시 거짓말이 된다 — 바로 위
    /// [`Warning::agents_stale`] 이 이미 그 자리다.
    ///
    /// `ids` 에는 **빠진 규칙 줄**이 그대로 든다. 이 알림은 이슈를 안 가리키므로 id 지도에 없고,
    /// 화면은 못 찾은 id 를 그대로 한 줄씩 내는 길을 이미 갖고 있다(`view::preview`) — 이 알림만을
    /// 위한 갈래를 거기 두지 않는 까닭이고, 그래야 고칠 명령(`hint`)도 같은 길로 따라 나온다.
    pub fn dotfile_rules(kind: &'static str, missing: &[&str], root: Option<&str>) -> Warning {
        Warning::new(kind, missing.iter().map(|l| (*l).to_string()).collect()).notice().hint(&Warning::init_hint(root))
    }

    /// 심은 머지 드라이버가 **못 도는** 상태의 알림(moai-2ewr). 재는 쪽은
    /// `cmd::merge_driver::notice` 고, `status` 와 훅의 보드가 이것을 `notices` 에 얹는다 —
    /// [`status`] 는 `&[Issue]` 만 받는 순수 함수라 설정도 파일도 안 읽는다.
    ///
    /// **안 심은 것은 여기서 말하지 않는다** — 그것은 [`Warning::merge_driver_absent`] 다. 여기는
    /// 심어 놓고 그 명령이 못 도는 자리다: 사람은 이슈마다 푸는 것이 돈다고 믿는데 실제로는 안
    /// 돌고, 그 사실이 어느 화면에도 안 선다. 둘을 가르는 까닭은 칠 줄이 같아도 무엇이 어긋났는지가
    /// 다르기 때문이다 — 한쪽은 "한 번도 안 쳤다" 고 다른 한쪽은 "쳐 뒀는데 그 자리가 비었다" 다.
    ///
    /// **경고가 아니라 알림이다.** 계획이 어긋난 것이 아니라 설치가 어긋난 것이고, 종료 코드는
    /// 안 바뀐다 — `agents_stale` 과 같은 자리다.
    ///
    /// `ids` 에 **적힌 명령**을 그대로 담는다. 어느 경로가 썩었는지가 고치는 데 필요한 전부다.
    ///
    /// **힌트는 그대로 칠 수 있는 줄이다**(리뷰 moai-h6aq.cx8). 앞 판은 `--as <늘 있는 자리>` 로
    /// 끝나 자리표시자를 남겼는데, 이 저장소의 힌트는 에이전트가 **그대로 친다** — 껍데기는
    /// `<늘` 을 넣기 자리로 읽어 깨지고, 따옴표로 싸면 그 글자가 명령으로 심긴다. 맨
    /// `--install` 은 지금 도는 바이너리의 절대 경로로 다시 심으므로, 흔한 판(옛 자리가 사라졌다,
    /// 다시 빌드해 자리가 옮겨졌다)에서 그대로 답이다. 워크트리의 `target/` 을 피하라는 말은
    /// `merge-driver --install --help` 와 관리 블록에 이미 있다.
    pub fn merge_driver_rotten(cmd: &str, root: Option<&str>) -> Warning {
        Warning::merge_driver_unusable("merge_driver_rotten", cmd, root)
    }

    /// 심어 둔 줄이 **쓸 수 없다**는 알림 둘이 함께 쓰는 꼴 — `rotten` 과 `alien`.
    ///
    /// **낱말은 둘이고 고칠 줄은 하나다.** 사람이 봐야 할 것이 달라 `kind` 를 가르지만(`dotfile_rules`
    /// 와 같은 자리), 치라는 줄은 맨 `--install` 로 같다 — 그 줄을 두 군데 적어 두면 한쪽만 고치는
    /// 날이 오고, 그때 같은 고침을 두 말로 대게 된다(리뷰 moai-vbmn.spv).
    fn merge_driver_unusable(kind: &'static str, cmd: &str, root: Option<&str>) -> Warning {
        let hint = Warning::cli_hint(root, "merge-driver --install");
        Warning::new(kind, vec![cmd.to_string()]).notice().hint(&hint)
    }

    /// 심어 둔 명령이 **이 드라이버를 모른다**는 알림(moai-zdw4). 자리도 있고 돌기도 도는데
    /// `merge-driver` 를 모르는 자리다 — 그 이름의 **다른 도구**가 거기 있다.
    ///
    /// **못 도는 것과 가른다.** 고칠 명령이 같아도 사람이 봐야 할 것이 다르다: 한쪽은 "적은
    /// 자리가 비었다" 고 이쪽은 "그 이름에 딴 것이 선다" 다. 이름을 `moai` 그대로 두고 배포하기로
    /// 한 뒤(2026-09-20) `--as moai` 로 심은 클론에서 실제로 가까워진 자리라, 뭉뚱그려 "안 돈다"
    /// 고 말하면 사람이 없는 파일을 찾으러 간다.
    ///
    /// 고칠 명령은 맨 `--install` 이다 — 지금 도는 이 바이너리의 절대 경로로 다시 심으면 이름
    /// 충돌 자체가 사라진다.
    pub fn merge_driver_alien(cmd: &str, root: Option<&str>) -> Warning {
        Warning::merge_driver_unusable("merge_driver_alien", cmd, root)
    }

    /// 저장소는 `merge=moai` 를 걸어 뒀는데 이 클론에 **안 심었다**는 알림(moai-9khu,
    /// 2026-09-20 사용자 결정).
    ///
    /// **한때 말하지 않던 자리다.** `moai-w8so` 의 실측은 그대로 서 있다 — 안 심은 클론에서는 그
    /// 낱말이 무시되고 git 의 기본 머지가 돌며 표식도 선다. 무해하다는 그 말과 말할 값이 없다는
    /// 말은 다르다: 설정은 커밋되지 않아 **클론마다 한 번** 쳐야 하고, 안 친 쪽은 이슈마다 푸는
    /// 값을 잃는 줄 모르고 잃는다. 새 사용자가 정확히 밟는 자리다.
    ///
    /// **경고가 아니라 알림이고 종료 코드는 안 바뀐다.** 막는 것이 아니라 비추는 것이다.
    ///
    /// **`ids` 가 없다.** 셀 이슈도 댈 경로도 없다 — 그 자리가 비었다는 것이 전부이고, 칠 줄은
    /// `hint` 가 낸다(`agents_stale` 과 같은 자리).
    pub fn merge_driver_absent(root: Option<&str>) -> Warning {
        let hint = Warning::cli_hint(root, "merge-driver --install");
        Warning::new("merge_driver_absent", Vec::new()).count(1).notice().hint(&hint)
    }

    /// 심은 줄이 **옛 판**이라는 알림(moai-h54i). 적힌 명령은 도는데 그 줄에 지금 판의 마디가
    /// 없는 자리다 — 리뷰 `moai-h6aq.cx8` 의 8번이 짚었다.
    ///
    /// **못 도는 것과 낱말을 가른다.** 고칠 명령이 같아도 뜻이 다르다: 한쪽은 "적은 자리가
    /// 비었다" 고 다른 한쪽은 "그 줄이 낡았다" 다. 뭉치면 그 중 한쪽이 반드시 거짓말이 된다
    /// (`agents_stale` 을 둘로 가른 것과 같은 까닭).
    ///
    /// 고칠 명령은 **같은 명령으로 다시 심는다** — 맨 `--install` 은 지금 도는 바이너리로 바꿔
    /// 적어, 그것이 워크트리의 `target/` 이면 고치라는 말이 썩을 자리를 심는다.
    pub fn merge_driver_stale(cmd: &str, root: Option<&str>) -> Warning {
        let tail = format!("merge-driver --install --as {}", crate::text::shell_word(cmd));
        Warning::new("merge_driver_stale", vec![cmd.to_string()]).notice().hint(&Warning::cli_hint(root, &tail))
    }

    /// 고칠 명령. 뿌리가 부른 자리와 다르면 `-C` 로 거기를 댄다 — `init` 은 부른 자리에 심으므로
    /// 그 `-C` 가 없으면 따라 친 쪽에 트래커가 하나 더 선다.
    fn init_hint(root: Option<&str>) -> String {
        Warning::cli_hint(root, "init")
    }

    /// 힌트 한 줄. **`-C` 를 붙이는 규칙은 한 자리다** — 알림마다 제 손으로 지으면 규칙이 바뀔 때
    /// 한쪽만 안 고쳐진다(리뷰 moai-h6aq.cx8).
    ///
    /// **알림 밖에서도 이 문으로 든다**(리뷰) — `cmd::init` 의 워크트리 거절과 `--check` 가 같은
    /// `moai -C <뿌리> init` 을 손으로 짓고 있었다. 뿌리는 이미 감싼 글자로 받는다
    /// (`init::away_root`·`text::shell_word`).
    pub(crate) fn cli_hint(root: Option<&str>, tail: &str) -> String {
        match root {
            None => format!("moai {tail}"),
            Some(r) => format!("moai -C {r} {tail}"),
        }
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
    /// 아직 판정 안 한 기한([`Dues`]) — `--json` 에는 안 실린다. [`StatusReport::judged`] 를 지난
    /// 보고서에서는 비어 있다: 그 줄들은 위 `warnings` 에 경고 둘로 서 있다.
    #[serde(skip)]
    pub dues: Dues,
}

impl StatusReport {
    /// 깨진 데이터가 있는가. 종료 코드를 가르는 유일한 것.
    pub fn broken(&self) -> bool {
        self.warnings.iter().any(|w| w.fatal)
    }

    /// 기한을 **읽는 사람의 달로 재어** 경고 목록에 제자리로 끼운다(moai-fgjj). 한 번 지나면
    /// [`StatusReport::dues`] 가 비어, 두 번 접히는 일이 없다.
    ///
    /// 부르는 쪽은 **낼 것을 그 자리에서 다 내는** 표면이다 — `moai status` 와 `--json`, 훅의 셈.
    /// 셈을 들고 있다가 나중에 그리는 쪽(탐색기의 배너·프로젝트 층)은 이것을 안 부르고
    /// [`Dues`] 를 그대로 들었다가 그릴 때 [`Dues::judge`] 로 잰다.
    #[must_use]
    pub fn judged(mut self, now: &str, zone: &crate::tz::Zone) -> StatusReport {
        // **자리를 먼저 든다**(리뷰) — `take` 뒤의 `self.dues` 는 `Dues::default()` 라 `at` 이 0 이고,
        // 그러면 기한 줄이 적어 둔 자리가 아니라 목록 **머리**에 끼어 `no_epic` 보다 앞에 선다.
        let dues = std::mem::take(&mut self.dues);
        let at = dues.at.min(self.warnings.len());
        self.warnings.splice(at..at, dues.judge(now, zone));
        self
    }
}

/// 기한을 재야 하는 마일스톤 줄 — **판정은 안 접어 둔다**(moai-fgjj, 2026-09-23 사용자 결정).
///
/// 시간대가 닿는 셈은 이 하나다. 기한은 시각이 아니라 **달력의 날**(`2026-09-20`)이라 그것과
/// 견주는 "오늘" 이 읽는 사람의 시간대로 정해지고(moai-h2th), 나머지 경고는 흐른 시간으로 재므로
/// 시간대가 없다(`days_since`).
///
/// **그 판정을 `warnings` 에 미리 접어 넣지 않는다.** 그 수를 캐시로 드는 쪽이 있다 — 탐색기의
/// 배너(`tui::Site`)와 프로젝트 층의 `+N`(`tui::layer::Summary`)은 이슈 수에 비례한 훑기를
/// 프레임마다 못 하므로 읽을 때 한 번 세어 들고 있고, 그 셈은 스레드에서 돈다. 접어 넣으면 그
/// 캐시가 시간대에 매여, `SPC o t` 로 시간대를 바꾼 뒤 옛 달로 판정한 수가 화면에 남는다. 그것을
/// 바로잡는 길은 셋이었고(캐시를 버리기·줄마다 시간대를 찍기·안 접기) 사람이 안 접는 쪽을
/// 골랐다 — **저장은 UTC 로 두고 보여줄 때만 시간대를 입힌다**는 이 저장소의 결이 셈에도 그대로
/// 선다(`tz` 의 머리글). 그래서 캐시는 시간대에 안 닿고, 무효화도 도장도 필요 없다.
///
/// **값이 없다.** 기한을 드는 줄은 마일스톤뿐이고(`moai edit --due` 가 마일스톤 줄에만 쓴다) 이
/// 저장소는 넷이다 — 그릴 때마다 재도 훑는 것이 이슈 전체가 아니라 그 넷이다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dues {
    /// (마일스톤 id, `due_on`) — 계획 안에 있고 안 닫힌 줄 가운데 기한을 든 것. 차례는 파일
    /// 차례다: 급한 차례는 날수를 알아야 정해지므로 [`Dues::judge`] 가 세운다.
    rows: Vec<(String, String)>,
    /// `status_due_days` — 문턱을 함께 든다. [`Warning::days`] 에 그대로 실려, 센 것과 화면이
    /// 말하는 폭이 안 갈린다(`stale_review` 와 같은 까닭).
    days: i64,
    /// 시간대를 안 접은 경고 목록에서 이 둘이 들어갈 자리([`StatusReport::judged`]).
    at: usize,
}

impl Dues {
    /// 읽는 사람의 달로 재어 경고 둘을 짓는다(moai-tfcp). **막지 않는다** — 경고는 종료 코드를
    /// 안 바꾼다(CLAUDE.md).
    ///
    /// 지난 것과 다가온 것을 **두 갈래로** 낸다: 사람이 할 일이 다르다(늦은 까닭을 대는 것과 남은
    /// 것을 추리는 것). 한 낱말로 뭉치면 그중 한쪽이 반드시 거짓말이 된다 — `agents_stale` 을
    /// 가른 것과 같은 자다.
    ///
    /// **나이를 여기서 싣는다**([`Warning::ages`] 가 아니라 직접) — 저쪽은 RFC3339 시각을 받는
    /// 자라 날짜(`YYYY-MM-DD`)를 못 읽는다. 재는 자리가 싣고 보이는 쪽은 읽기만 한다.
    /// **부호째 싣는다** — 지난 것은 음수다([`crate::model::days_until`] 과 같은 부호). 크기만
    /// 싣고 갈래로 뜻을 가르면 `ages` 한 자리에 정반대의 두 뜻이 앉아, `kind` 를 같이 안 보는
    /// `--json` 쪽이 "2일 지남" 과 "2일 남음" 을 같은 값으로 읽는다.
    #[must_use]
    pub fn judge(&self, now: &str, zone: &crate::tz::Zone) -> Vec<Warning> {
        let (mut overdue, mut soon) = self.split(now, zone);
        // **급한 것이 앞이다.** `view::preview` 는 앞의 셋만 그리고 나머지를 "N건 더" 로 접으므로,
        // 파일 차례(곧 id 차례)로 두면 90일 지난 줄이 접히고 2일 지난 줄이 화면에 선다. 둘 다
        // 오름차순이면 된다 — 지난 쪽은 음수라 가장 많이 지난 것이 앞에 온다.
        //
        // **세우는 자리는 여기다**(리뷰) — 가르는 자리([`Dues::split`])에 두면 갈래가 비었는지만 보는
        // [`Dues::count`] 도 같이 정렬하는데, 그쪽은 그리는 걸음마다 돌아 버릴 차례를 프레임마다 짓는다.
        for rows in [&mut overdue, &mut soon] {
            // id 로 동점을 푸는 것은 같은 날수가 여럿일 때 차례가 판마다 안 흔들리게 하려는
            // 것이다. `sort_by_key` 로 적으면 그 id 를 견줄 때마다 베낀다 — 빌려서 견준다.
            rows.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));
        }
        let mut out = Vec::new();
        for (kind, rows, days) in [
            // **문턱을 그대로 넘긴다** — `stale_review` 와 같은 까닭이다. 여기 수를 박아 두면
            // `status_due_days` 를 고친 저장소에서 경고가 센 것과 화면이 말하는 폭이 갈린다.
            ("milestone_overdue", &overdue, None),
            ("milestone_due_soon", &soon, Some(self.days)),
        ] {
            if rows.is_empty() {
                continue;
            }
            let mut w = Warning::new(kind, rows.iter().map(|(id, _)| (*id).to_string()).collect());
            for (id, d) in rows {
                w.ages.insert((*id).to_string(), *d);
            }
            out.push(match days {
                Some(d) => w.days(d),
                None => w,
            });
        }
        out
    }

    /// 판정이 낼 **경고의 수**(0·1·2) — 짓지 않고 센다. 캐시가 든 수에 이것을 더한 것이 화면이
    /// 대는 "드러난 것 N건" 이다(`tui::Surfaced`). 그리는 걸음마다 부르므로 `Warning` 도 차례도
    /// 안 짓는다 — 차례를 세우는 자는 [`Dues::judge`] 다.
    pub fn count(&self, now: &str, zone: &crate::tz::Zone) -> usize {
        let (overdue, soon) = self.split(now, zone);
        usize::from(!overdue.is_empty()) + usize::from(!soon.is_empty())
    }

    /// 지난 것과 다가온 것으로 가른다 — 차례는 **파일 차례 그대로**다(곧 id 차례). 급한 것을 앞으로
    /// 올리는 것은 짓는 자리([`Dues::judge`])가 하고, 세는 자리([`Dues::count`])는 갈래가 비었는지만
    /// 보므로 정렬을 지나지 않는다.
    fn split(&self, now: &str, zone: &crate::tz::Zone) -> (Vec<(&str, i64)>, Vec<(&str, i64)>) {
        // **읽는 사람의 달로 잰다**(moai-h2th). `zone.shift` 는 꼴을 안 바꾸므로 그대로
        // [`crate::model::days_until`] 에 든다. 고리 밖에서 한 번만 옮긴다 — 줄마다 옮기면
        // 같은 값을 줄 수만큼 다시 짓는다.
        let today = zone.shift(now);
        let (mut overdue, mut soon): (Vec<(&str, i64)>, Vec<(&str, i64)>) = (Vec::new(), Vec::new());
        for (id, due) in &self.rows {
            // 꼴이 틀린 값은 말없이 넘긴다 — 담는 쪽이 까닭을 적었다.
            let Some(left) = crate::model::days_until(due, &today) else {
                continue;
            };
            match left {
                d if d < 0 => overdue.push((id.as_str(), d)),
                d if d <= self.days => soon.push((id.as_str(), d)),
                _ => {}
            }
        }
        (overdue, soon)
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
///
/// `zone` 은 **읽는 사람의 시간대**다 — 기한 판정 하나에만 든다([`Dues`]). 이 문은 **낼 것을 그
/// 자리에서 다 내는** 표면의 것이라 판정까지 접어서 낸다([`StatusReport::judged`]): `moai status`·
/// `--json`·훅의 셈이 그렇다. 셈을 들고 있다가 나중에 그리는 탐색기는 [`status_in`] 으로 시간대
/// 없이 세고 [`Dues`] 를 그대로 들었다가 그릴 때 잰다(moai-fgjj).
pub fn status(
    issues: &[Issue],
    unreadable: &[Unreadable],
    cfg: &Config,
    now: &str,
    zone: &crate::tz::Zone,
) -> StatusReport {
    status_unjudged(issues, unreadable, cfg, now).judged(now, zone)
}

/// [`status`] 에서 **기한 판정만 뺀 것.** 시간대를 안 받으므로 tzdb 를 안 만진다.
///
/// 시간대를 안 드는 표면이 셋이다 — 셈만 쓰고 시각을 한 줄도 안 그리는 `moai project ls`,
/// [`Dues`] 를 들었다가 그릴 때 재는 탐색기의 층, 그리고 위의 [`status`] 자신. 그 셋이 저마다
/// `status_in(.., &Soil::of(..))` 를 손으로 이어 적던 것을 여기 한 이름에 모은다(리뷰 moai-j4ie).
/// 손으로 이으면 [`Soil`] 이 아닌 조각에서 지은 지도를 넘겨도 컴파일되고, 그때 수가 조용히
/// 어긋난다 — 이 문은 `issues` 하나만 받아 그 짝을 못 어긋나게 한다.
///
/// **이미 [`Soil`] 을 들고 있는 쪽은 [`status_in`] 이다.** 탐색기의 적재가 그쪽이다(moai-u5o9).
pub fn status_unjudged(issues: &[Issue], unreadable: &[Unreadable], cfg: &Config, now: &str) -> StatusReport {
    // **파일 전체를 훑어야 아는 것은 한 걸음으로 잰다**(moai-oxup, [`Soil`]). 손으로 이을 때는 `groups`
    // 가 `milestones`·`misplaced`·두 롤업 안에서 저마다 다시 지어 `status` 한 번에 예닐곱 번 돌았다.
    status_in(issues, unreadable, cfg, now, &Soil::of(issues))
}

/// [`status`] 와 같은 것. **이미 잰 [`Soil`] 을 받는다** — 탐색기는 적재 때 색인·묶음 칸을 지으려고
/// 이미 쟀으므로, 경고 셈에서 다시 재면 같은 걸음을 두 벌 걷는다(moai-u5o9).
///
/// **시간대를 안 받는다**(moai-fgjj, 2026-09-23 사용자 결정). 시간대에 닿는 셈은 마일스톤 기한
/// 판정 하나뿐이라(moai-h2th), 그 판정에 드는 줄만 [`StatusReport::dues`] 에 담아 내고 날 판정은
/// 그리는 쪽에 맡긴다 — 그 까닭은 [`Dues`] 에 적었다. 판정까지 접어서 받으려면 [`status`] 나
/// [`StatusReport::judged`] 다.
///
/// **나이는 안 옮긴다.** `days_since` 가 재는 것은 흐른 시간이고 흐른 시간에는 시간대가 없다 —
/// `now` 를 통째로 옮기면 모든 나이가 시간대 폭만큼 부풀어 하루씩 늘어난다. 그래서 이 문이 내는
/// 나머지 경고는 시간대와 무관하고, 캐시로 들어도 시간대가 바뀌어 낡지 않는다.
pub fn status_in<'a>(
    issues: &'a [Issue],
    unreadable: &[Unreadable],
    cfg: &Config,
    now: &str,
    soil: &Soil<'a>,
) -> StatusReport {
    let group = &soil.epic;
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // **여기가 "지금 계획" 의 정의다.** 보드 수·모든 경고·흐름이 이 하나를
    // 지나므로, 미뤄 둔 것을 여기서 빼면 아래 전부에서 저절로 빠진다.
    // 묶음의 읽은 칸도 같은 자리에서 한 번만 받는다 — 둘 다 조상과 소속을 타는
    // 셈이라 따로 부르면 `moai status` 한 번에 같은 걸음을 두 벌 걷는다.
    let roots = &soil.roots;
    let stands = soil.stands(issues, cfg);
    // 묶음이 막을 때 그 막음이 선 때(`blocked_since`). 칸과 한 번의 셈에서 받는다.
    let group_since: BTreeMap<&str, &str> = stands.iter().map(|(id, s)| (*id, s.since)).collect();
    // 막대 곁에 댈 미룬 수도 같은 셈에서 받는다([`Stand::deferred`], moai-zxwj).
    let put_aside: BTreeMap<&str, usize> = stands.iter().map(|(id, s)| (*id, s.deferred)).collect();
    let out_of_plan: BTreeSet<&str> = roots.keys().copied().collect();
    // 도는 마일스톤 — `ready` 가 밖의 일을 빼는 자와 **같은 자다**(moai-493a).
    let running = running_in(cfg, &by_id, &stands, &out_of_plan);
    let (states, waits) = split_stands(stands);
    // **계획 밖인가는 줄에 묻는다**(moai-u3ta) — 위의 `out_of_plan` 은 id 로 접은 것이라 같은
    // id 의 앞줄이 뒷줄의 답을 입는다. 보드 수·경고·흐름이 이 하나를 지나므로 여기서 갈리면
    // 아래가 다 갈린다. **id 로 불리는 자리**(막는 줄, 빈 에픽, 마감)는 그대로 접은 것을 쓴다.
    let off = &soil.shelved;
    let work: Vec<&Issue> =
        issues.iter().enumerate().filter(|(k, i)| is_work(i) && off[*k].is_none()).map(|(_, i)| i).collect();
    // **한 번만 잰다.** 둘 다 이슈 전부의 물림을 타고 올라가므로, 경고마다
    // 다시 부르면 같은 걸음을 `moai status` 한 번에 여러 벌 걷는다.
    // `placed` 는 자리를 못 정하는 줄(`nav` 의 `(길 잃음)` 과 같은 집합),
    // `held` 는 못 쓸 참조를 든 줄이다 — 아래 6번이 둘을 합쳐 드러낸다.
    let placed = &soil.lost;
    let held = broken_in(issues, &soil.kinds);
    let counts: BTreeMap<String, usize> =
        cfg.statuses.iter().map(|s| (s.clone(), work.iter().filter(|i| i.status.as_str() == s).count())).collect();

    let eclipsed = soil.eclipsed();
    // **꾸짖는 자와 세는 자가 한 자를 쓴다**(리뷰) — 소속 지도를 곧바로 짚던 아래 두 경고는
    // 롤업이 `placed.at(i)` 로 옮겨 간 날부터 같은 화면에서 롤업과 어긋났다. 에픽이 `0/1` 로
    // 센 그 줄을 `에픽 없는 이슈` 가 함께 이름 댔다.
    let in_epic = soil.placed(Kind::Epic);
    let in_stone = soil.placed(Kind::Milestone);
    let rolls = rollup_of_in(issues, cfg, &in_epic, &eclipsed);
    // **묶음 줄에는 읽은 칸을 곁들인다.** 막대(`3/5`)는 계획 중 얼마나 했나이고 칸은
    // 지금 할 것이 남았나라, 남은 멤버를 미뤄 접은 묶음은 `1/2` 인 채로 닫혀 있다 —
    // 세션이 여기서 시작하는데 그것을 안 말하면 접은 묶음과 굴러가는 묶음이 같아 보인다.
    // **미룬 수도 같은 자리에서 곁들인다**(moai-zxwj) — 막대의 분모는 미룬 멤버를 그대로 세므로
    // (2026-09-21 사용자 결정), 그 수가 안 줄어드는 까닭이 화면에 없으면 읽는 쪽은 `102/110` 에서
    // 여덟이 남은 줄 안다. 칸과 **한 자리에서** 곁들이는 까닭은 둘 다 [`Stand`] 가 세기 때문이다.
    let stood = |r: Roll| {
        let column = r.id.as_deref().and_then(|id| states.get(id)).map(|c| c.to_string());
        let deferred = r.id.as_deref().and_then(|id| put_aside.get(id)).copied();
        Roll { column, deferred, ..r }
    };
    let epics: Vec<Roll> = rolls.iter().filter(|r| r.id.is_some()).cloned().map(stood).collect();
    // 마일스톤을 하나도 안 쓰는 저장소에는 줄도 경고도 내지 않는다.
    let stones: Vec<Roll> =
        rollup_of_in(issues, cfg, &in_stone, &eclipsed).into_iter().filter(|r| r.id.is_some()).map(stood).collect();
    let mut warnings = Vec::new();
    let mut notices = Vec::new();

    // **가려진 줄은 소속으로 꾸짖지 않는다** (1·1-2). 소속 지도의 값은 쌍둥이의
    // 종류로 셈한 것이라 그 줄의 것이 아니고, 그 줄은 `(길 잃음)` 에 서며 힌트
    // (`moai show -e none`·`--milestone none`)의 거름망도 안 고른다 — 세면 경고가
    // 가리킨 명령이 침묵한다(moai-b5lu). `duplicate_id` 가 그 id 를 따로 드러낸다. 판정은 위에서
    // 롤업에 넘긴 그것(`soil.eclipsed`)이다.

    // 1. 에픽에 안 붙은 것. 마일스톤이 아직 없으므로 **제일 중요한 신호**다
    //    — "물어보지 않고 만든 이슈" 의 지문이다.
    //    **길 잃은 줄 밑에 접힌 줄도 안 센다**(moai-uni2) — 트리가 그 줄을 `(길 잃음)` 안에
    //    그리고, 고칠 곳은 부모의 끊긴 참조라 6번의 `dangling_*` 가 댄다.
    let folded = &soil.folded;
    let loose: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            // **소속은 [`Placed`] 에 묻는다** — 위의 롤업이 `어느 묶음에도 안 딸린 것` 을 고르는
            // 그 자다(`rollup_of_in` 의 `loose`). 지도를 곧바로 짚으면 같은 id 의 앞줄이 뒷줄의
            // 빈 값을 입어, 에픽이 멤버로 센 줄을 같은 화면이 `에픽 없음` 으로 꾸짖는다(리뷰).
            !i.status.is_done() && !eclipsed(i) && in_epic.at(i).is_none() && !folded.contains(i.id.as_str())
        })
        .collect();
    // **분모는 미룬 일까지 센다.** 에픽을 통째로 미루면 그 멤버만 `work` 에서 빠져,
    // 원래 있던 소속 없는 일 하나가 "열린 것의 100%" 로 선다 — 미루기 하나로 경고가
    // 늘어 `Stop` 이 세션을 붙들었다(moai-c8lb 와 같은 덫). 분자는 그대로 지금 계획만
    // 센다: 미룬 소속 없는 일로는 꾸짖지 않는다. 그래서 미루기는 비율을 못 올린다.
    //
    // **문턱도 id 로 잰다** — 경고가 내는 셈(`Warning::new` 가 거른 `count`)과 같은 자다.
    // 줄로 재면 같은 종류 쌍둥이 한 쌍이 `status_no_epic_min` 을 넘겨 놓고 `4건` 을 말한다.
    let open = issues
        .iter()
        .filter(|i| is_work(i) && !i.status.is_done())
        .map(|i| i.id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let no_epic = Warning::new("no_epic", ids_of(&loose));
    let ratio = if open == 0 { 0.0 } else { no_epic.count as f64 / open as f64 };
    // **빈 집합으로는 말하지 않는다.** 문턱이 0 이면 `>=` 가 늘 참이라, 이슈가 하나도 없는
    // 저장소에서 `에픽 없는 이슈 0건` 이 영영 선다 — 0 은 낮춘 것이지 "없는 것도 세라" 가 아니다.
    if no_epic.count > 0 && (no_epic.count >= cfg.status.no_epic_min || ratio >= cfg.status.no_epic_ratio) {
        warnings.push(no_epic.ratio(ratio).hint("moai show -e none"));
    }

    // 1-2. 마일스톤을 쓰기 시작했는데 거기 안 붙은 일. 마일스톤이 없는
    //      저장소에는 말하지 않는다 — 안 쓰는 기능으로 잔소리하지 않는다.
    if !stones.is_empty() {
        // 종류가 틀린 참조는 **마일스톤이 있는 것이 아니다.** 그대로 세면
        // 그 줄이 "마일스톤 있음" 으로 빠져, 정작 드러내야 할 것이 숨는다.
        //
        // **소속도 판정도 줄마다 묻는다**(리뷰, `in_stone`·[`Soil::adrift`]) — 위의 마일스톤 표를
        // 세는 자와 여기가 지도를 저마다 짚던 때는, 같은 화면이 `argos-m001 0/1` 을 그려 놓고
        // 그 줄을 `마일스톤에 안 붙은 일` 로 이름 댔다.
        let outside: Vec<&Issue> = work
            .iter()
            .copied()
            .filter(|i| {
                // 길 잃은 줄 밑에 접힌 줄은 1번과 같은 까닭으로 안 센다(moai-uni2) —
                // 트리가 그 줄을 `(길 잃음)` 안에 그리고, 고칠 곳은 부모의 끊긴 참조다.
                !i.status.is_done()
                    && !eclipsed(i)
                    && !folded.contains(i.id.as_str())
                    && (in_stone.at(i).is_none() || soil.adrift(i) == Some(Misplace::Milestone))
            })
            .collect();
        if !outside.is_empty() {
            warnings.push(Warning::new("no_milestone", ids_of(&outside)).hint("moai show --milestone none"));
        }
    }

    // 2. review 에서 썩는 것. 게이트를 없앤 대가라 여기가 제일 먼저 곪는다.
    let rotting: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            i.status.as_str() == "review"
                && days_since(&i.status_since, now).is_some_and(|d| d > cfg.status.review_days)
        })
        .collect();
    if !rotting.is_empty() {
        warnings.push(
            Warning::new("stale_review", ids_of(&rotting))
                .days(cfg.status.review_days)
                .ages(|i| i.status_since.as_str(), &rotting, now)
                // **문턱을 그대로 넘긴다.** 여기 수를 박아 두면 `status_review_days` 를 고친
                // 저장소에서 경고가 센 것과 안내가 내는 것이 다른 집합이 된다 — 낮춘 쪽에서는
                // 안내가 빈 목록을 내서 읽는 쪽이 경고를 틀린 것으로 읽는다.
                .hint(&format!("moai show -s review --stale {}", cfg.status.review_days)),
        );
    }

    // 2-2. 오래 막혀 있는 것. 막힌 채로 방치되는 것이 계획이 멈춘 자리다.
    // 미뤄 둔 것에 막힌 것은 **아래 2-3 이 제 이름으로** 말한다. 여기서도
    // 세면 같은 줄이 두 번 나오고, 이쪽 말로는 막는 줄을 어디서 찾는지 모른다.
    let by_deferred = |i: &Issue| !holding(i, &by_id, &out_of_plan, &states, &waits).0.is_empty();
    let stuck: Vec<&Issue> = work
        .iter()
        .copied()
        .filter(|i| {
            !i.status.is_done()
                && is_blocked(i, &by_id, &states, &waits)
                && !by_deferred(i)
                && days_since(blocked_since(i, &by_id, &states, &waits, &group_since), now)
                    .is_some_and(|d| d > cfg.status.blocked_days)
        })
        .collect();
    if !stuck.is_empty() {
        warnings.push(Warning::new("blocked_stale", ids_of(&stuck)).days(cfg.status.blocked_days).ages(
            |i| blocked_since(i, &by_id, &states, &waits, &group_since),
            &stuck,
            now,
        ));
    }

    // 2-3. 미뤄 둔 것에 막힌 것. **날짜를 안 기다린다** — 계획이 스스로
    //      모순된 자리라(지금 할 일이 지금 안 할 일을 기다린다) 사흘 둔다고
    //      풀리지 않는다. 막지는 않는다.
    //      **나이는 모순이 선 때부터다**(moai-hcx3) — 막는 쪽을 미룬 날과 이 막음이 선 날
    //      ([`blocked_since`]) 가운데 늦은 것. 칸 나이를 대면 "미룬 것에 N일 막힘" 으로 읽히는데
    //      그 줄이 칸에 머문 날수일 뿐이다.
    //      - 미룬 날: 막는 줄마다 그것을 계획에서 빼는 미룸 전부(`deferred_sources_in` —
    //        물려받은 조상의 미룸까지) 가운데 가장 이른 `deferred_at`, 막는 줄이 여럿이면 그중
    //        가장 이른 것.
    //      - 막음이 선 날로 누른다: 40일 전에 미룬 줄에 오늘 막힌 줄은 모순이 오늘 섰다. 막는
    //        줄이 어제 done 에서 되돌아 나왔어도 그렇다. `blocked_stale` 과 같은 자다.
    let waiting: Vec<&Issue> = work.iter().copied().filter(|i| !i.status.is_done() && by_deferred(i)).collect();
    if !waiting.is_empty() {
        let sources = deferred_sources_in(issues, group, &soil.milestone, roots);
        // 시각은 줄과 같은 수명이라, 받는 자리(`Warning::ages`)의 서명으로 추론되게 그 자리에 둔다.
        warnings.push(
            Warning::new("blocked_by_deferred", ids_of(&waiting))
                .ages(
                    |i| {
                        let shelved = holding(i, &by_id, &out_of_plan, &states, &waits)
                            .0
                            .iter()
                            .filter_map(|b| sources.get(b))
                            .flatten()
                            .filter_map(|r| by_id.get(r).and_then(|x| x.deferred_at.as_deref()))
                            .min();
                        // 계획 밖인 줄은 언제나 미룬 곳이 있다(`deferred_roots_in` ⊆
                        // `deferred_sources_in`). 못 찾으면 나이를 안 싣는다 — 빈 시각은
                        // `days_since` 가 거른다.
                        let Some(shelved) = shelved else { return "" };
                        shelved.max(blocked_since(i, &by_id, &states, &waits, &group_since))
                    },
                    &waiting,
                    now,
                )
                .hint("moai show --deferred"),
        );
    }

    // 3. 한 번에 여러 개 벌인 것. AI 가 가장 잘 하는 실수다.
    //    **가려진 줄은 벌인 일이 아니다** — [`wip`] 과 같은 자다(moai-es40, 사용자 결정). 여기만 세면
    //    한눈 보기가 집은 것 셋을 대는 그 보드에 `4건` 이 서고, `ready` 아래 줄에 없는 id 를 잊은 것으로
    //    꾸짖는다(4번) — 그 제목은 쌍둥이 에픽의 것이다. 그 id 는 `duplicate_id` 가 따로 드러낸다.
    let wip: Vec<&Issue> = work.iter().copied().filter(|i| cfg.is_started(i.status.as_str()) && !eclipsed(i)).collect();
    // 문턱은 id 로 잰다 — 위 `no_epic` 과 같은 까닭이다.
    let overload = Warning::new("wip_overload", ids_of(&wip));
    if overload.count > cfg.status.wip_limit {
        warnings.push(overload.limit(cfg.status.wip_limit));
    }

    // 4. 집어 놓고 잊은 것.
    let forgotten: Vec<&Issue> = wip
        .iter()
        .copied()
        .filter(|i| {
            i.status.as_str() != "review" && days_since(&i.status_since, now).is_some_and(|d| d > cfg.status.wip_days)
        })
        .collect();
    if !forgotten.is_empty() {
        warnings.push(Warning::new("stale_progress", ids_of(&forgotten)).days(cfg.status.wip_days).ages(
            |i| i.status_since.as_str(),
            &forgotten,
            now,
        ));
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

    let orphans: Vec<&Issue> =
        issues.iter().filter(|i| crate::id::parent_of(&i.id).is_some_and(|p| !known.contains(p))).collect();
    if !orphans.is_empty() {
        warnings.push(Warning::new("orphan_child", ids_of(&orphans)));
    }
    let dangling_blockers: Vec<&Issue> =
        issues.iter().filter(|i| i.blocked_by.iter().any(|b| !known.contains(b.as_str()))).collect();
    if !dangling_blockers.is_empty() {
        warnings.push(Warning::new("dangling_blocked_by", ids_of(&dangling_blockers)));
    }
    // 먼 미래 시각(moai-ugjp). **경고지 깨진 데이터가 아니다** — 줄은 읽히고 고칠 것일 뿐이라
    // 종료 코드를 안 바꾼다(사람이 정했다). 종류를 안 가린다: 시각은 모든 줄이 든다.
    let ahead: Vec<&Issue> = issues.iter().filter(|i| far_ahead(i, now)).collect();
    if !ahead.is_empty() {
        warnings.push(Warning::new("future_timestamp", ids_of(&ahead)));
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
    let piled = |(k, i): &(usize, &Issue)| is_idea(i) && !i.status.is_done() && off[*k].is_none();
    let count = issues.iter().enumerate().filter(piled).count();
    // 문턱 0 으로 `쌓인 idea 0건` 이 서지 않게 한다 — 위 `no_epic` 과 같은 까닭이다.
    if count > 0 && count >= cfg.status.idea_pile {
        let oldest = issues
            .iter()
            .enumerate()
            .filter(piled)
            .filter_map(|(_, i)| days_since(&i.created_at, now))
            .max()
            .unwrap_or(0);
        notices.push(Warning::new("idea_pile", Vec::new()).count(count).oldest(oldest).notice().hint("moai idea ls"));
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
    //
    //      **줄마다 센다**(moai-u3ta) — 이 수가 가리키는 `moai show --deferred` 도 줄마다
    //      고르므로, id 로 세면 미룬 쌍둥이 하나가 제 짝까지 이 수에 얹는다.
    let shelved = |(k, _): &(usize, &Issue)| off[*k].is_some();
    let count = issues.iter().enumerate().filter(shelved).count();
    if count > 0 {
        let oldest = issues
            .iter()
            .enumerate()
            .filter(shelved)
            .filter_map(|(_, i)| i.deferred_at.as_deref().and_then(|at| days_since(at, now)))
            .max()
            .unwrap_or(0);
        notices.push(
            Warning::new("deferred", Vec::new()).count(count).oldest(oldest).notice().hint("moai show --deferred"),
        );
    }

    // 6-4. 도는 마일스톤(moai-tvvb). **알림이다** — 고칠 것이 아니라 지금 무엇이 먼저인지를
    //      대는 줄이고, `ready` 가 밖의 일을 안 내는 까닭이 여기 말고는 설 데가 없다.
    //
    //      **도는지는 멤버에서 읽는다**([`running_in`]) — 위에서 이미 잰 `stands` 를 그대로
    //      쓴다. 여기서 다시 재면 `ready` 가 빼는 자와 이 줄이 비추는 자가 갈린다.
    //
    //      **세는 것은 밖에 남은 일이다**, `ready` 가 낼 수 있는 것이 아니다 — 막혔거나 자식이
    //      열린 줄도 "이번 마일스톤 밖에 남았다" 는 말에는 든다. 두 수가 다를 수 있어 낱말도
    //      다르게 적는다(`ready` 는 "안 냈다", 여기는 "밖에 남았다").
    //      **`p0` 은 안 센다** — 밖에 있어도 집는 것이라 이 줄이 미루라고 말하는 대상이 아니다.
    //      **이미 집은 것도 안 센다**(첫 칸인 것만 센다) — 밖에서 벌여 놓은 일은 그대로 끝내는
    //      것이지 나중으로 미루는 것이 아니다(AGENTS 블록의 "이미 집은 것은 그대로 끝내면
    //      된다"). 막혔거나 자식이 열린 줄은 첫 칸이라 그대로 든다.
    if !running.is_empty() {
        let outside = work
            .iter()
            // **가려진 줄은 안 센다**(리뷰) — 위의 1·1-2 와 같은 자고, `ready` 도 그 줄은 못 낸다
            // (`unblocked_pick`). 세면 아무도 못 집는 줄을 두고 `나중으로 미루라` 고 말한다.
            .filter(|i| i.status.as_str() == cfg.first_status() && i.priority() != 0 && !eclipsed(i))
            // **소속은 [`Placed`] 에 묻는다**(리뷰) — 위의 마일스톤 표와 `ready` 의 문이 같은 자를
            // 쓰는데 여기만 지도를 짚으면, `argos-m001 0/2` 를 그린 화면이 그 둘을 `밖에 남았다`
            // 고 센다.
            .filter(|i| !in_stone.at(i).is_some_and(|m| running.iter().any(|r| r.id == m)))
            .count();
        // **0 이면 말하지 않는다**(리뷰 6) — "밖에 남은 일 0건은 나중이다" 는 아무것도 안
        // 말하면서 자리만 차지한다. 위의 `idea_pile`·`deferred` 가 같은 까닭으로 0 을 거른다.
        // 어느 마일스톤이 도는지는 보드의 마일스톤 표가 이미 낸다.
        if outside > 0 {
            notices.push(
                Warning::new("milestone_focus", running.iter().map(|m| m.id.clone()).collect())
                    .count(outside)
                    .notice()
                    .hint("moai ready"),
            );
        }
    }

    // 6-5. **마일스톤이 겹쳐 돈다**(moai-nnal, 2026-09-22 사용자 결정). 알림이지 경고가
    //      아니다 — 겹치는 것은 `ready` 가 이미 견디는 판이고(다 안으로 센다), 고칠 일이
    //      있다는 말이 아니라 지금 겹쳐 돈다는 사실이다. 경고로 두면 겹친 동안 내내 Stop 훅이
    //      세는 수가 하나 늘어 세션이 붙들린다.
    //
    //      **셋 이상도 이 한 줄이고, 그 줄이 수를 말한다**(리뷰) — 글에 "둘" 을 박아 두면 셋이
    //      돌 때 보드가 제 밑에 세 줄을 늘어놓으면서 둘이라고 말한다. 어느 마일스톤인지는
    //      `preview` 가 id 와 제목으로 낸다.
    //
    //      차례를 다시 매기지 않는다 — `running` 은 `stands`(id 로 키를 잡은 `BTreeMap`)에서
    //      오므로 이미 id 차례다. 위 `milestone_focus` 가 같은 자리에서 같은 꼴로 담는다.
    if running.len() >= 2 {
        notices.push(
            Warning::new("milestones_running", running.iter().map(|m| m.id.clone()).collect())
                .notice()
                .hint("moai ready"),
        );
    }

    // 6-6. 마일스톤의 기한(moai-tfcp, 2026-09-22 사용자 결정). **판정은 여기서 안 한다** —
    //      기한을 든 줄과 문턱만 [`Dues`] 에 담고, 읽는 사람의 달로 재는 것은 그리는 쪽이 한다
    //      (moai-fgjj, 2026-09-23 사용자 결정). 까닭은 [`Dues`] 에 적었다.
    //
    //      **끝난 마일스톤은 안 담는다.** 다 닫힌 뒤의 지난 기한은 고칠 일이 아니라 지난 일이고,
    //      세면 닫힌 마일스톤이 영영 경고로 선다. **미뤄 둔 것도 안 담는다** — 계획에서 뺀 것이
    //      잔소리를 늘리면 미루기가 경고를 낳는 손잡이가 된다(`running_in` 과 같은 까닭). 이 거름은
    //      시간대와 무관하므로 여기 남는다 — 옮겨 두면 그리는 쪽마다 같은 거름을 다시 적는다.
    //
    //      꼴이 틀린 값은 그대로 담는다 — 판정하는 자가 말없이 넘긴다([`Dues::judge`]). 쓰기가 이미
    //      거절하므로 여기 오는 것은 손으로 푼 줄뿐이고, 그것은 `unknown_field` 가 아니라 그 줄을
    //      고칠 때 드러난다.
    let dues = Dues {
        rows: issues
            .iter()
            .filter(|i| i.kind == Kind::Milestone)
            .filter(|m| {
                let id = m.id.as_str();
                !out_of_plan.contains(id) && !eclipsed(m) && !states.get(id).is_some_and(|c| *c == crate::config::DONE)
            })
            .filter_map(|m| m.due_on.as_deref().map(|d| (m.id.clone(), d.to_string())))
            .collect(),
        days: cfg.status.due_days,
        // **기한 둘이 들어갈 자리를 적어 둔다** — 밑의 7 이 깨진 데이터를 뒤에 얹으므로, 판정을
        // 그냥 뒤에 붙이면 보드에서 기한 줄이 치명적인 줄 **밑으로** 내려간다.
        at: warnings.len(),
    };

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
        warnings.push(Warning::new("unreadable_line", Vec::new()).count(unreadable.len()).hint("moai show").fatal());
    }

    // 흐름. 만드는 속도가 끝내는 속도를 넘으면 쌓인다.
    //
    // **여기만 `is_work` 로 센다.** 위의 `work` 는 "지금 계획" 이고 이 줄은
    // "지난 이레에 있었던 일" 이라 묻는 것이 다르다 — 미뤄 둔 것을 여기서
    // 빼면 오늘 셋을 미루는 것만으로 `생성 5 · 쌓이는 중 +5` 가
    // `생성 2 · +2` 가 되어, 미루기가 쌓임 경고를 지우는 손잡이가 된다.
    // 나이는 0 아래로 안 내려간다(`days_since`) — 조금 미래로 찍힌 줄도 오늘 것으로 센다.
    // **먼 미래 시각을 든 줄은 통째로 뺀다**(moai-ugjp) — 2099 는 "최근" 이 아니고, 셈에 넣으면
    // 위 `future_timestamp` 가 드러낸 오타가 흐름 숫자로도 새어 나온다.
    let within = |at: &str| days_since(at, now).is_some_and(|d| d < cfg.status.flow_days);
    let happened: Vec<&Issue> = issues.iter().filter(|i| is_work(i) && !far_ahead(i, now)).collect();
    let created = happened.iter().filter(|i| within(&i.created_at)).count();
    let closed = happened.iter().filter(|i| i.status.is_done() && within(&i.status_since)).count();

    StatusReport {
        counts,
        total: work.len(),
        milestones: stones,
        epics,
        warnings,
        notices,
        flow: Flow { days: cfg.status.flow_days, created, done: closed, net: created as i64 - closed as i64 },
        dues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    /// 시험의 바탕 시간대 — 저장된 그대로다. 여기 쓰는 `now` 가 모두 UTC 로 적혀 있어, 옮기는
    /// 시간대를 들면 기한 말고도 이 파일의 모든 시각이 함께 움직인다. 옮겨서 갈리는 것은
    /// [`a_deadline_is_read_in_the_reader_s_day`] 가 따로 잰다.
    fn utc() -> &'static crate::tz::Zone {
        crate::tz::Zone::stored()
    }

    /// 아무것도 안 적은 저장소가 받는 문턱. **기본값을 여기 다시 적지 않는다** —
    /// 적으면 기본값을 고칠 때 시험만 옛 수를 든 채 통과한다.
    const IDEA_PILE: usize = crate::config::Thresholds::DEFAULT.idea_pile;

    fn make(id: &str, kind: Kind, status: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new(status), "2026-09-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str, status: &str) -> Issue {
        let mut i = make(id, Kind::Issue, status);
        i.epic = Some(epic.into());
        i
    }

    /// [`kin_of`] 는 **줄로 가른다, id 로가 아니라**. 같은 id 를 든 줄이 둘이면 [`group_members`]
    /// 는 이긴 줄 하나만 멤버로 내는데, id 로 거르면 그 판정이 진 줄에도 옮아붙어 멤버도 자식도
    /// 아닌 채 상세에서 통째로 사라진다 — 깨진 자료는 숨기지 않고 `duplicate_id` 가 따로 드러낸다는
    /// 약속을 그 자리에서 어긴다(리뷰).
    #[test]
    fn a_twin_that_is_not_the_member_still_stands_as_a_child() {
        let mut twin = make("argos-aaaa.b1x", Kind::Idea, "todo");
        twin.title = "가려진 앞줄".into();
        let all = vec![make("argos-aaaa", Kind::Epic, "todo"), twin, make("argos-aaaa.b1x", Kind::Issue, "todo")];
        let kids = children_of(&all, "argos-aaaa");
        assert_eq!(kids.len(), 2, "같은 id 를 든 두 줄이 다 자식이다");
        let mine = group_members(&all, &all[0]);
        assert_eq!(mine.len(), 1, "멤버는 이긴 줄 하나다");
        let drawn = kin_of(&kids, &mine);
        assert_eq!(drawn.len(), 1, "진 줄까지 걷혔다");
        assert_eq!(drawn[0].title, "가려진 앞줄", "걷힌 것이 이긴 줄이 아니라 진 줄이다");
    }

    /// 멤버가 하나도 없으면 자식은 그대로다 — 묶음이 아닌 줄을 펼치는 흔한 길이 여기다.
    #[test]
    fn kin_of_keeps_every_child_when_nothing_is_a_member() {
        let all = vec![make("argos-aaaa", Kind::Issue, "todo"), make("argos-aaaa.b1x", Kind::Issue, "todo")];
        let kids = children_of(&all, "argos-aaaa");
        assert_eq!(kin_of(&kids, &[]).len(), kids.len());
    }

    /// 이름 후보는 **`worktree::names` 에게 묻는다** — 여기 베껴 두면 그쪽에 후보가 하나 늘어도
    /// 아래 시험들이 옛 규칙에 대고 푸르게 지나간다.
    fn tree(path: &str, branch: &str, holds: &[&str]) -> Workplace {
        let t = crate::worktree::Tree { path: path.into(), label: branch.into(), head: String::new() };
        Workplace {
            names: crate::worktree::names([&t]),
            path: t.path,
            branch: t.label,
            holds: holds.iter().map(|s| s.to_string()).collect(),
            touched: BTreeSet::new(),
            marked: BTreeSet::new(),
            born: None,
            unknown: false,
            broken: false,
        }
    }

    /// 시험의 시계 — 여기 줄들은 `make` 가 9월 1일에 만들므로 어느 것도 "방금 집은" 것이 아니다.
    const LATER: &str = "2026-09-15T12:00:00Z";

    /// **집은 줄의 자리는 워크트리에서 파생한다**(moai-ir8q) — 이름이 그 줄·조상·에픽을 가리키거나,
    /// 옆 스냅샷이 그 줄을 쥐었으면 그 워크트리다. 안 집은 줄은 자리를 안 묻는다.
    #[test]
    fn a_picked_row_is_placed_by_worktree_name_ancestor_epic_or_side_snapshot() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0002.a", Kind::Issue, "review"),
            make("argos-0003", Kind::Issue, "in_progress"),
            make("argos-0004", Kind::Issue, "in_progress"),
            make("argos-0005", Kind::Issue, "todo"),
        ];
        let trees = vec![
            tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[]),
            tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &["argos-0003"]),
            tree("/r/.claude/worktrees/argos-0005", "worktree-argos-0005", &[]),
        ];
        let at = places(&issues, &cfg(), &trees, LATER);
        let branches = |id: &str| at.get(id).map(|p| p.at().iter().map(|w| w.branch.as_str()).collect::<Vec<_>>());
        assert_eq!(branches("argos-0002"), Some(vec!["worktree-argos-0001"]), "에픽 이름으로 못 찾았다");
        assert_eq!(branches("argos-0002.a"), Some(vec!["worktree-argos-0001"]), "부모의 에픽으로 못 찾았다");
        assert_eq!(branches("argos-0003"), Some(vec!["worktree-agent-x"]), "옆 스냅샷으로 못 찾았다");
        assert_eq!(branches("argos-0004"), Some(vec![]), "자리 없는 집은 줄은 빈 목록으로 선다");
        assert!(matches!(at["argos-0004"], Place::Lost), "{:?}", at["argos-0004"]);
        assert_eq!(branches("argos-0005"), None, "안 집은 줄의 자리를 셌다");
        // **묶음의 자리는 멤버에서 굴려 올린다**(moai-0h8m) — 워크트리 이름이 에픽 id 라 이어받는
        // 세션이 에픽부터 읽는다.
        assert_eq!(branches("argos-0001"), Some(vec!["worktree-argos-0001"]), "에픽이 멤버의 자리를 못 냈다");
    }

    /// **굴려 올린 자리에 같은 워크트리가 두 번 서지 않는다.** 마일스톤은 멤버를 두 길로 받는다
    /// (이슈의 마일스톤, 그리고 그 이슈가 든 에픽의 마일스톤) — 같은 것이라 한 번만 걸어야 하고,
    /// 여러 자리에 선 멤버를 합칠 때 잇닿지 않은 중복까지 걷어내야 한다. 안 그러면 `moai show
    /// <마일스톤>` 이 같은 `자리` 줄을 두 번 낸다.
    ///
    /// 한 멤버가 두 자리에 서려면 **같은 거리의 이름** 둘이어야 한다 — 에픽 이름과 제 이름이면 제
    /// 이름만 선다([`the_closest_name_wins_the_place`]).
    #[test]
    fn a_rolled_up_place_names_each_worktree_once() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-0009".into());
        let issues = vec![
            make("argos-0009", Kind::Milestone, "todo"),
            epic,
            member("argos-0002", "argos-0001", "in_progress"),
            member("argos-0003", "argos-0001", "in_progress"),
        ];
        // 둘 다 디렉터리 이름이 0002 를 가리키고, 뒤의 것은 가지 이름으로 0003 도 가리킨다 — 0002 는
        // 둘에, 0003 은 뒤의 것에 서서 에픽이 뒤의 것을 두 번 받는다.
        let trees = vec![
            tree("/r/a/argos-0002", "worktree-argos-0002", &[]),
            tree("/r/b/argos-0002", "worktree-argos-0003", &[]),
        ];
        let at = places(&issues, &cfg(), &trees, LATER);
        let branches = |id: &str| at.get(id).map(|p| p.at().iter().map(|w| w.branch.as_str()).collect::<Vec<_>>());
        let both = Some(vec!["worktree-argos-0002", "worktree-argos-0003"]);
        assert_eq!(branches("argos-0002"), both);
        assert_eq!(branches("argos-0003"), Some(vec!["worktree-argos-0003"]));
        assert_eq!(branches("argos-0001"), both, "에픽이 자리를 겹쳐 냈다");
        assert_eq!(branches("argos-0009"), both, "마일스톤이 같은 자리를 두 번 냈다");
    }

    /// **묶음의 자리에는 그 묶음 이름의 워크트리도 선다**(사용자 결정 2026-09-19, moai-t3yj). 멤버가
    /// 저마다 제 이름 워크트리로 가면 굴림은 그것만 내는데(moai-m62u), 가지와 커밋 안 한 일이 사는
    /// 곳은 에픽 워크트리고 이어받는 세션이 읽는 것이 `moai show <에픽>` 의 자리다. 멤버가 아무 데도
    /// 안 섰어도(자리를 잃었어도) 그 이름의 워크트리가 있으면 거기다.
    #[test]
    fn a_group_keeps_the_worktree_named_after_it() {
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "in_progress")];
        let trees = vec![
            tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[]),
            tree("/r/.claude/worktrees/argos-0002", "worktree-argos-0002", &[]),
        ];
        let at = places(&issues, &cfg(), &trees, LATER);
        fn branches<'a>(p: &Place<'a>) -> Vec<&'a str> {
            p.at().iter().map(|w| w.branch.as_str()).collect()
        }
        assert_eq!(branches(&at["argos-0002"]), ["worktree-argos-0002"], "멤버가 에픽 워크트리까지 낸다");
        assert_eq!(
            branches(&at["argos-0001"]),
            ["worktree-argos-0001", "worktree-argos-0002"],
            "에픽의 자리에서 제 이름 워크트리가 빠졌다"
        );

        // 멤버가 자리를 잃어도 에픽 이름의 워크트리는 선다.
        let alone = vec![trees[0].clone()];
        let at = places(&issues, &cfg(), &alone, LATER);
        assert!(matches!(at["argos-0002"], Place::At(_)), "에픽 이름이 멤버를 가리키는 길이 닫혔다");
        let gone = vec![tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[]), tree("/r/x", "x", &[])];
        let mut lost = issues.clone();
        lost[1].status_since = "2000-01-01T00:00:00Z".into();
        let at = places(&lost, &cfg(), &gone, LATER);
        assert_eq!(branches(&at["argos-0001"]), ["worktree-argos-0001"]);
    }

    /// **가장 가까운 이름이 자리를 쥔다**(moai-m62u, 사용자 결정) — 머지하고 안 치운 에픽 워크트리가
    /// 그 에픽에 뒤이어 집은 멤버를, 그 멤버를 제 이름으로 띄운 워크트리보다 먼저 쥐지 않는다. 조상도
    /// 같다 — 제 이름으로 뜬 자식의 자리는 부모 이름의 워크트리가 아니다. 가리키는 이름이 에픽 하나뿐이면
    /// 여전히 거기다(에픽 이름으로 뜬 워크트리에서 멤버를 하는 것이 규약이다).
    #[test]
    fn the_closest_name_wins_the_place() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            member("argos-0003", "argos-0001", "in_progress"),
            member("argos-0003.aaa", "argos-0001", "in_progress"),
        ];
        let trees = vec![
            tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[]),
            tree("/r/.claude/worktrees/argos-0002", "worktree-argos-0002", &[]),
            tree("/r/.claude/worktrees/argos-0003", "worktree-argos-0003", &[]),
            tree("/r/.claude/worktrees/argos-0003.aaa", "worktree-argos-0003.aaa", &[]),
        ];
        let at = places(&issues, &cfg(), &trees, LATER);
        let branches = |id: &str| at.get(id).map(|p| p.at().iter().map(|w| w.branch.as_str()).collect::<Vec<_>>());
        assert_eq!(branches("argos-0002"), Some(vec!["worktree-argos-0002"]), "에픽 이름이 제 이름과 겨뤄 이겼다");
        assert_eq!(
            branches("argos-0003.aaa"),
            Some(vec!["worktree-argos-0003.aaa"]),
            "부모 이름이 제 이름과 겨뤄 이겼다"
        );
        let only_epic = &trees[..1];
        let at = places(&issues, &cfg(), only_epic, LATER);
        assert_eq!(at["argos-0002"].at().len(), 1, "에픽 이름만 있을 때 자리를 잃었다");

        // 훅의 자도 같다 — 옆 이름이 더 가까울 때만 옆의 것이다. 같은 거리면 제 것이다.
        let set = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
        let (epic, own, none) = (set(&["argos-0001"]), set(&["argos-0002"]), set(&[]));
        let (epics, stones) = ties(&issues);
        let over =
            |away: &BTreeSet<String>, own: &BTreeSet<String>, i: &Issue| claims_over(&epics, &stones, away, own, i);
        assert!(!over(&epic, &own, &issues[1]), "제 이름 워크트리의 줄을 에픽 워크트리에 넘겼다");
        assert!(over(&epic, &own, &issues[2]), "제 이름이 안 가리키는 멤버를 제 것으로 셌다");
        assert!(over(&epic, &none, &issues[1]));
        assert!(!over(&own, &own, &issues[1]), "같은 거리를 옆에 넘겼다");
        assert!(over(&set(&["argos-0003.aaa"]), &set(&["argos-0003"]), &issues[3]), "자식 이름이 부모 이름에 졌다");
    }

    /// **묶음이 아닌 id 로는 굴려 올리지 않는다.** 소속 지도(`groups`)는 `epic` 에 적힌 글자를
    /// 그대로 내므로 끊긴 참조도 이슈 id 도 값이 된다 — 그것에 키를 주면 `stranded` 가 그 id 를
    /// `by_id` 로 찾아 **집지도 않은 줄**을 "자리 없는 집은 줄" 로 세고, `show <그 id>` 는
    /// `placeable` 이 막아 아무 말도 안 해 두 표면이 갈린다.
    #[test]
    fn a_broken_epic_reference_never_becomes_a_place() {
        let issues = vec![
            // 이슈인데 남의 `epic` 이 가리킨다 — 묶음이 아니다.
            make("argos-0007", Kind::Issue, "todo"),
            member("argos-0002", "argos-0007", "in_progress"),
            // 아예 없는 id 를 가리키는 줄.
            member("argos-0003", "argos-0404", "in_progress"),
        ];
        let trees = vec![tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[])];
        let at = places(&issues, &cfg(), &trees, LATER);
        assert_eq!(at.keys().map(String::as_str).collect::<Vec<_>>(), ["argos-0002", "argos-0003"], "{at:?}");
        let w = stranded(&issues, &cfg(), &trees, LATER).expect("자리 없는 줄을 안 비췄다");
        assert_eq!(w.ids, ["argos-0002", "argos-0003"], "집지도 않은 줄을 자리 없음으로 셌다");
        assert_eq!(w.count, 2);
    }

    /// **이름이 id 인 워크트리는 물려받은 벌여 놓인 줄로 자리를 안 댄다.** main 에서 뜬 워크트리의
    /// 스냅샷에는 갈라질 때 main 에 집혀 있던 줄이 다 있다 — 그것을 세면 세션이 죽은 줄도 그 뒤에
    /// 뜬 워크트리 아무 데나 자리가 잡혀 `stranded` 가 영영 안 선다. 그 워크트리에서 늦게 만진 줄은
    /// 이름과 상관없이 자리로 센다.
    #[test]
    fn a_named_worktree_does_not_place_rows_it_merely_inherited() {
        let issues = vec![
            make("argos-0001", Kind::Issue, "in_progress"),
            make("argos-0002", Kind::Issue, "in_progress"),
            make("argos-0003", Kind::Issue, "in_progress"),
        ];
        let mut named =
            tree("/r/.claude/worktrees/argos-0002", "worktree-argos-0002", &["argos-0001", "argos-0002", "argos-0003"]);
        named.touched.insert("argos-0003".into());
        let trees = vec![named];
        let at = places(&issues, &cfg(), &trees, LATER);
        assert_eq!(at["argos-0001"].at().len(), 0, "물려받은 줄을 이름 있는 워크트리의 자리로 셌다");
        assert_eq!(at["argos-0002"].at().len(), 1);
        assert_eq!(at["argos-0003"].at().len(), 1, "늦게 만진 줄을 자리로 안 셌다");
    }

    /// **제 손으로 적어 둔 집기는 이름에게 안 밀린다**(리뷰 moai-71ht 셋째 판의 훑기). 규약은 에픽
    /// 이름으로 워크트리를 띄우고 머지 뒤에도 한동안 두는데, 그 멤버를 실제로 집은 것은 이름이 id 가
    /// 아닌 에이전트 격리 워크트리다 — 이름으로만 좁히던 판은 그 자리를 화면에서 통째로 지워,
    /// 이어받는 세션을 아무도 없는 워크트리로 보냈다. 이름이 답한 자리도 함께 선다.
    #[test]
    fn a_marked_worktree_stands_beside_the_named_one() {
        let mut issues =
            vec![make("argos-0009", Kind::Epic, "in_progress"), make("argos-0001", Kind::Issue, "in_progress")];
        issues[1].epic = Some("argos-0009".into());
        let epic = tree("/r/.claude/worktrees/argos-0009", "worktree-argos-0009", &[]);
        let mut agent = tree("/r/.claude/worktrees/agent-7f", "worktree-agent-7f", &[]);
        agent.marked.insert("argos-0001".into());
        let trees = vec![epic, agent];
        let at = places(&issues, &cfg(), &trees, LATER);
        let paths: Vec<&str> = at["argos-0001"].at().iter().filter_map(|w| w.path.to_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("agent-7f")), "집었다고 적어 둔 자리를 이름이 지웠다 — {paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("argos-0009")), "이름이 답한 자리가 빠졌다 — {paths:?}");
    }

    /// **같은 id 의 묶음 쌍둥이가 자리 잃은 줄을 가리지 않는다**(리뷰 moai-ya06). 머지가 한 id 에
    /// 두 줄을 남기고 뒷줄이 에픽이면, 줄 전체로 되짚는 지도(`by_id`)는 그 에픽을 내준다 — 그것을
    /// 종류로 거르던 판이 앞줄의 **집힌 이슈**를 통째로 버려, 세션이 죽은 줄이 `duplicate_id`
    /// 하나 때문에 영영 안 보였다. 되짚는 자는 `started` 라 묶음 id 는 애초에 안 든다 — 가려진
    /// 줄을 빼는 `wip` 이 아니다(moai-es40).
    #[test]
    fn a_group_twin_of_the_same_id_does_not_hide_stranded_work() {
        let issues = vec![
            make("argos-0002", Kind::Issue, "in_progress"),
            // 머지가 남긴 뒷줄 — 같은 id 인데 종류가 다르다.
            make("argos-0002", Kind::Epic, "todo"),
        ];
        let trees = vec![tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[])];
        let w = stranded(&issues, &cfg(), &trees, LATER).expect("쌍둥이가 자리 잃은 줄을 가렸다");
        assert_eq!(w.ids, ["argos-0002"]);
    }

    /// **자리가 안 보이는 까닭 넷이 다 서는지 본다**(리뷰 moai-ya06). 한때 시험이 `At` 과 `Lost`
    /// 만 짚어, `blind` 를 통째로 지워도 — 그러면 `Place::Unknown` 이 영영 안 선다 — 단위 시험이
    /// 하나도 안 깨졌다. 굴림도 같은 차례를 쓰므로(`Place::rank`) 에픽에서 한 번 더 본다.
    #[test]
    fn a_place_says_why_it_is_not_seen() {
        let fresh_at = |id: &str, at: &str| {
            let mut i = member(id, "argos-0001", "in_progress");
            i.status_since = at.into();
            i
        };
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            fresh_at("argos-0002", "2026-09-15T11:30:00Z"), // `LATER` 의 삼십 분 전 — 방금 집었다
            member("argos-0003", "argos-0001", "in_progress"), // 9월 1일에 집혔다 — 오래됐다
        ];
        let lost = vec![tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[])];
        let at = places(&issues, &cfg(), &lost, LATER);
        assert!(matches!(at["argos-0002"], Place::Fresh), "{:?}", at["argos-0002"]);
        assert!(matches!(at["argos-0003"], Place::Lost), "{:?}", at["argos-0003"]);
        // 방금 집은 멤버가 있으면 에픽도 그렇게 선다 — 굴림의 차례가 줄 하나의 차례와 같다.
        assert!(matches!(at["argos-0001"], Place::Fresh), "{:?}", at["argos-0001"]);

        // 못 읽은 워크트리가 하나 끼면 `없다` 도 `방금` 도 아니라 `모른다` 다(사용자 결정) —
        // 이름이 아무 줄도 안 가리키는 워크트리라야 그렇게 가린다.
        let mut blind = lost.clone();
        blind[0].unknown = true;
        let at = places(&issues, &cfg(), &blind, LATER);
        assert!(matches!(at["argos-0003"], Place::Unknown), "{:?}", at["argos-0003"]);
        assert!(matches!(at["argos-0002"], Place::Unknown), "방금 집었다고 단정했다 — {:?}", at["argos-0002"]);
        assert!(matches!(at["argos-0001"], Place::Unknown), "굴림이 모름을 버렸다");

        // **이름이 집은 줄을 가리키는 워크트리는 못 읽어도 남을 안 가린다** — 그 줄은 이미 제
        // 자리가 있고, 나머지 줄까지 덮으면 치우지 않은 깨진 워크트리 하나가 경고를 잠재운다.
        let mut named_blind = vec![tree("/r/.claude/worktrees/argos-0002", "worktree-argos-0002", &[])];
        named_blind[0].unknown = true;
        let at = places(&issues, &cfg(), &named_blind, LATER);
        assert!(matches!(at["argos-0002"], Place::At(_)), "{:?}", at["argos-0002"]);
        assert!(matches!(at["argos-0003"], Place::Lost), "이름이 가리키는 워크트리가 남의 줄을 덮었다");
        assert!(stranded(&issues, &cfg(), &blind, LATER).is_none(), "모르는 것을 자리 없음으로 셌다");
        assert!(stranded(&issues, &cfg(), &lost, LATER).is_some(), "자리 잃은 줄을 안 비췄다");
    }

    /// **에픽 이름 워크트리도 이름이 집은 줄을 가리킨다**(moai-1i9d). 규약의 워크트리 이름은 에픽
    /// id 인데(`worktree-<에픽>`), 가리는 자가 이름을 집은 id 와 바로 견주기만 하면 에픽은 집히지
    /// 않으니 그 워크트리는 "아무 줄도 안 가리키는" 것으로 읽힌다 — 그것 하나를 못 읽는 것만으로
    /// 딴 에픽의 자리 잃은 줄이 `Unknown` 으로 덮여 `stranded` 에서 빠진다. 가리는 워크트리를
    /// 세는 자([`blinding`])도 같은 답을 내야 한다 — 갈리면 화면이 "다 못 셌다" 라고 하는데 판정은
    /// 다 셌다.
    #[test]
    fn an_unreadable_epic_worktree_does_not_blind_the_rest() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0005", Kind::Epic, "todo"),
            member("argos-0003", "argos-0005", "in_progress"), // 9월 1일에 집혔다 — 자리를 잃었다
        ];
        let mut trees = vec![tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[])];
        trees[0].unknown = true;
        let at = places(&issues, &cfg(), &trees, LATER);
        assert!(matches!(at["argos-0002"], Place::At(_)), "{:?}", at["argos-0002"]);
        assert!(
            matches!(at["argos-0003"], Place::Lost),
            "에픽 이름 워크트리가 남의 줄을 덮었다 — {:?}",
            at["argos-0003"]
        );
        let w = stranded(&issues, &cfg(), &trees, LATER).expect("에픽 이름 워크트리 하나가 stranded 를 재웠다");
        assert_eq!(w.ids, ["argos-0003"]);
        assert!(blinding(&issues, &cfg(), &trees).is_empty(), "판정을 안 가리는 워크트리를 가린다고 셌다");

        // 이름이 아무 집은 줄도 안 가리키면 그때는 가린다 — 두 자가 같은 답이다.
        let mut stray = vec![tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[])];
        stray[0].unknown = true;
        assert!(matches!(places(&issues, &cfg(), &stray, LATER)["argos-0003"], Place::Unknown));
        assert_eq!(blinding(&issues, &cfg(), &stray).len(), 1);

        // **둘이 섞여 있으면 가린 것만 든다.** 화면은 깨진 스냅샷 둘을 다 대지만
        // (`worktree::Unread::all`) `unreadable_worktrees` 와 층의 셈은 뒤엣것 하나다 — 두 목록이
        // 다시 하나로 합쳐지면 여기가 먼저 깨진다.
        let mut mixed = vec![
            tree("/r/.claude/worktrees/argos-0001", "worktree-argos-0001", &[]),
            tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[]),
        ];
        mixed[0].unknown = true;
        mixed[1].unknown = true;
        let only: Vec<&str> = blinding(&issues, &cfg(), &mixed).iter().map(|t| t.branch.as_str()).collect();
        assert_eq!(only, ["worktree-agent-x"], "판정을 안 가리는 에픽 워크트리까지 셌다");
        assert!(matches!(places(&issues, &cfg(), &mixed, LATER)["argos-0003"], Place::Unknown));

        // 집은 줄이 없으면 가릴 판정도 없다 — `places` 는 빈 지도를 내고, 세는 자도 비어야 한다.
        let idle = vec![make("argos-0001", Kind::Epic, "todo")];
        assert!(places(&idle, &cfg(), &stray, LATER).is_empty());
        assert!(blinding(&idle, &cfg(), &stray).is_empty(), "집은 줄이 없는데 가린다고 셌다");
    }

    /// **워크트리가 없으면 아무 키도 없다**(moai-tbin) — "없다" 는 찾아보고 못 찾았을 때의 말이다.
    /// 그리고 **집은 멤버를 둔 묶음은 언제나 키를 받는다**(moai-oepz) — 문서의 계약과 코드가 한때
    /// 갈려, 워크트리가 있을 때만 굴림이 섰다.
    #[test]
    fn with_no_worktrees_nothing_is_placed_and_groups_always_roll_up() {
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "in_progress")];
        assert!(places(&issues, &cfg(), &[], LATER).is_empty(), "볼 워크트리가 없는데 자리를 단정했다");
        assert!(stranded(&issues, &cfg(), &[], LATER).is_none());

        // 워크트리가 있으면 멤버도 묶음도 키를 받는다 — 자리를 못 찾아도 그렇다.
        let trees = vec![tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &[])];
        let at = places(&issues, &cfg(), &trees, LATER);
        assert!(matches!(at["argos-0002"], Place::Lost), "{:?}", at["argos-0002"]);
        assert!(matches!(at["argos-0001"], Place::Lost), "묶음이 키를 못 받았다");
    }

    /// **굴려 올린 자리의 차례는 `trees` 의 것이다.** 멤버의 답을 이어 붙이면 차례가 멤버 id 를
    /// 따라가, `show <일>` 과 `show <에픽>` 이 같은 워크트리 짝을 서로 다른 차례로 낸다.
    #[test]
    fn a_rolled_up_place_keeps_the_worktree_order() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            member("argos-0003", "argos-0001", "in_progress"),
        ];
        // 차례는 `zeta` 가 뒤다. 그런데 거기 선 멤버(`argos-0002`)의 id 는 앞선다.
        let trees = vec![
            tree("/r/.claude/worktrees/alpha", "worktree-argos-0003", &[]),
            tree("/r/.claude/worktrees/zeta", "worktree-argos-0002", &[]),
        ];
        let at = places(&issues, &cfg(), &trees, LATER);
        let branches = |id: &str| at[id].at().iter().map(|w| w.branch.as_str()).collect::<Vec<_>>();
        assert_eq!(branches("argos-0001"), ["worktree-argos-0003", "worktree-argos-0002"], "멤버 id 차례로 냈다");
    }

    /// **이름 없는 워크트리는 뜨기 한 시간 전부터 그 뒤로 움직인 줄을 쥔다**(사용자 결정). 그보다
    /// 먼저 집혀 물려 들어온 줄은 그 워크트리의 자리가 아니다 — 안 그러면 에이전트 격리 워크트리
    /// 하나가 자리 잃은 줄을 전부 가린다. 윗금은 없다: `status_since` 는 칸이 선 때라 뜬 뒤에
    /// 칸을 옮긴 산 일이 제 자리를 잃으면 안 된다. 뜬 시각을 못 읽으면 전처럼 다 믿는다.
    #[test]
    fn a_nameless_worktree_holds_what_moved_around_the_time_it_was_made() {
        let picked_at = |id: &str, at: &str| {
            let mut i = make(id, Kind::Issue, "in_progress");
            i.status_since = at.into();
            i
        };
        let issues = vec![
            picked_at("argos-0001", "2026-09-15T08:00:00Z"), // 세 시간 전 — 물려받았다
            picked_at("argos-0002", "2026-09-15T10:40:00Z"), // 이십 분 전 — 이 워크트리의 일
            picked_at("argos-0003", "2026-09-15T11:30:00Z"), // 뜬 뒤에 칸이 움직였다 — 여전히 이 워크트리의 일
        ];
        let mut agent =
            tree("/r/.claude/worktrees/agent-x", "worktree-agent-x", &["argos-0001", "argos-0002", "argos-0003"]);
        agent.born = Some("2026-09-15T11:00:00Z".into());
        let trees = vec![agent.clone()];
        let at = places(&issues, &cfg(), &trees, LATER);
        let counts: Vec<usize> =
            ["argos-0001", "argos-0002", "argos-0003"].iter().map(|id| at[*id].at().len()).collect();
        assert_eq!(counts, [0, 1, 1]);

        // **되돌렸다 다시 집은 줄은 지금 집은 줄이다**(moai-hav1) — `started_at` 은 처음 뗀 때라
        // 옛날인데, 이 워크트리가 뜰 무렵 움직인 것은 `status_since` 가 안다.
        //
        // **양쪽을 다 잰다.** 빠지는 쪽만 재던 판은 `started_at.or(status_since)` 로 바꾸는
        // 되돌림을 초록으로 지났다(리뷰 moai-71ht.rv0) — 물려받은 줄이 최근 `started_at` 하나로
        // 이 워크트리의 것이 되어, 격리 워크트리 하나가 자리 잃은 줄을 도로 다 가린다.
        let mut repicked = issues.clone();
        repicked[0].started_at = Some("2026-09-15T10:50:00Z".into());
        repicked[1].started_at = Some("2026-08-01T00:00:00Z".into());
        let at = places(&repicked, &cfg(), &trees, LATER);
        assert_eq!(at["argos-0002"].at().len(), 1, "다시 집은 줄을 처음 뗀 때로 재 자리를 잃었다");
        assert_eq!(at["argos-0001"].at().len(), 0, "물려받은 줄을 처음 뗀 때로 재 이 워크트리에 붙였다");

        agent.born = None;
        let trees = vec![agent];
        let at = places(&issues, &cfg(), &trees, LATER);
        assert!(at.values().all(|p| p.at().len() == 1), "뜬 시각을 모르는데 쥔 줄을 버렸다");
    }

    /// **자리 없는 집은 줄을 경고로 비춘다**(moai-4370). 워크트리를 하나도 안 쓰는 저장소는 조용하고,
    /// 방금 집은 줄은 워크트리가 뜰 틈(한 시간)을 준다. 미룬 것은 안 센다. 막지 않는다 — 치명이 아니다.
    #[test]
    fn stranded_names_picked_rows_no_live_worktree_holds() {
        let now = "2026-09-15T12:00:00Z";
        let old = |id: &str| {
            let mut i = make(id, Kind::Issue, "in_progress");
            i.status_since = "2026-09-15T08:00:00Z".into();
            i
        };
        let mut fresh = make("argos-0003", Kind::Issue, "in_progress");
        fresh.status_since = "2026-09-15T11:30:00Z".into();
        let mut shelved = old("argos-0004");
        shelved.deferred_at = Some("2026-09-15T09:00:00Z".into());
        let issues = vec![old("argos-0001"), old("argos-0002"), fresh, shelved];
        let trees = vec![tree("/r/.claude/worktrees/argos-0002", "worktree-argos-0002", &[])];

        let w = stranded(&issues, &cfg(), &trees, now).expect("자리 없는 줄을 못 봤다");
        assert_eq!(w.kind, "stranded");
        assert_eq!(w.ids, ["argos-0001"], "{w:?}");
        assert!(!w.fatal && !w.notice);
        let hint = w.hint.as_deref().expect("고칠 손을 안 댄다");
        assert!(hint.contains("moai mv <id> todo"), "{w:?}");
        // **내미는 줄의 자유 글도 작은따옴표다**(moai-vj4e). 큰따옴표로 가르친 `-m` 은 채운 글에
        // 백틱이나 `$(…)` 가 들면 셸이 명령으로 풀어 글이 잘린 채 0 으로 끝난다 — 가르치는 글을
        // 훑는 `guide` 의 시험은 `guide` 의 글만 보아 이 줄을 못 본다.
        assert!(!hint.contains('"'), "보드가 자유 글을 큰따옴표로 가르친다 — {hint}");

        assert!(stranded(&issues, &cfg(), &[], now).is_none(), "워크트리를 안 쓰는 저장소에서 떠들었다");
        let all_placed = vec![tree("/r/w", "worktree-argos-0001", &[]), trees[0].clone()];
        assert!(stranded(&issues, &cfg(), &all_placed, now).is_none());

        // **`status()` 는 이것을 안 싣는다.** 훅의 기준선과 `Stop` 이 `status()` 의 경고를 세므로,
        // 여기 들어가는 순간 남의 세션이 워크트리를 치우는 것만으로 제 세션이 붙들린다 — 경고가
        // 게이트가 되는 자리다(CLAUDE.md). 옮겨 놓아도 모든 시험이 푸른 채로 지나가던 자리라
        // 여기서 못박는다.
        let st = status(&issues, &[], &cfg(), now, utc());
        assert!(!st.warnings.iter().chain(&st.notices).any(|w| w.kind == "stranded"), "{:?}", st.warnings);
    }

    /// **길 잃은 생각 밑에 접힌 일은 에픽 없는 이슈로 안 센다**(moai-uni2). 멀쩡한 생각 밑의
    /// 일은 그대로 센다 — 생각은 소속을 안 넘긴다. 에픽 없는 이슈를 사이에 둔 손자도 접힌다.
    #[test]
    fn a_row_folded_under_a_lost_thought_is_not_loose() {
        let now = "2026-09-11T00:00:00Z";
        let loose = |issues: &[Issue]| -> Vec<String> {
            status(issues, &[], &cfg(), now, utc())
                .warnings
                .iter()
                .find(|w| w.kind == "no_epic")
                .map(|w| w.ids.clone())
                .unwrap_or_default()
        };
        let thought = |id: &str, epic: Option<&str>| {
            let mut t = make(id, Kind::Idea, "todo");
            t.epic = epic.map(str::to_string);
            t
        };

        // 끊긴 에픽을 든 생각 밑 — 자식도 손자도 길 잃음 안에 접힌다.
        let lost = [
            thought("argos-0001", Some("argos-zzzz")),
            make("argos-0001.aaa", Kind::Issue, "todo"),
            make("argos-0001.aaa.bbb", Kind::Issue, "todo"),
        ];
        assert!(loose(&lost).is_empty(), "길 잃은 생각 밑에 접힌 줄을 에픽 없음으로 센다: {:?}", loose(&lost));
        let adrift = |all: &[Issue]| {
            let seen = misplaced(all);
            all.iter().map(|i| seen.contains_key(i.id.as_str())).collect::<Vec<bool>>()
        };
        let folded = under_lost(&lost, &Lines::of(&lost), &groups(&lost), &adrift(&lost));
        assert_eq!(folded.into_iter().collect::<Vec<_>>(), ["argos-0001.aaa", "argos-0001.aaa.bbb"]);

        // 멀쩡한 생각 밑 — 그대로 에픽 없는 이슈다.
        let healthy = [thought("argos-0002", None), make("argos-0002.aaa", Kind::Issue, "todo")];
        assert_eq!(loose(&healthy), ["argos-0002.aaa"]);

        // 제 에픽을 적은 자식은 접히지 않는다 — 제 에픽으로 간다.
        let own = [
            thought("argos-0003", Some("argos-zzzz")),
            make("argos-0004", Kind::Epic, "todo"),
            member("argos-0003.aaa", "argos-0004", "todo"),
        ];
        assert!(under_lost(&own, &Lines::of(&own), &groups(&own), &adrift(&own)).is_empty());
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
        assert_eq!(
            r.counts,
            BTreeMap::from([
                ("todo".to_string(), 1),
                ("in_progress".to_string(), 0),
                ("review".to_string(), 0),
                ("done".to_string(), 2),
            ])
        );
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

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let ids: Vec<&String> =
            st.warnings.iter().filter(|w| w.kind.starts_with("dangling")).flat_map(|w| w.ids.iter()).collect();
        assert!(ids.iter().any(|id| *id == "argos-e001"), "에픽의 망가진 epic 을 안 말한다 — {ids:?}");
        assert!(ids.iter().any(|id| *id == "argos-0010"), "이슈의 망가진 milestone 을 안 말한다 — {ids:?}");
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
        let st = status(&rows, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "dangling_epic").expect("경고가 없다");
        assert_eq!(w.ids.first().map(String::as_str), Some("argos-zzz9"), "{:?}", w.ids);
    }

    /// 마일스톤 아닌 것을 가리킨 줄을 "마일스톤 있음" 으로 세지 않는다 —
    /// 그러면 `no_milestone` 이 그 줄을 오히려 빼 버린다.
    #[test]
    fn a_wrong_kind_milestone_does_not_count_as_having_one() {
        let mut bad = make("argos-0005", Kind::Issue, "todo");
        bad.milestone = Some("argos-0002".into()); // 에픽이다
        let issues = vec![make("argos-0001", Kind::Milestone, "todo"), make("argos-0002", Kind::Epic, "todo"), bad];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let issues = vec![make("argos-0001", Kind::Epic, "in_progress"), member("argos-0002", "argos-0001", "done")];
        let rolls = rollup(&issues, &cfg());
        assert_eq!(roll_of(&rolls, Some("argos-0001")).percent, Some(100));
    }

    #[test]
    fn ready_excludes_what_is_not_pickable() {
        let issues = vec![
            make("argos-00aa", Kind::Epic, "todo"),         // 에픽 자체
            make("argos-0003", Kind::Issue, "in_progress"), // 이미 집은 것
            make("argos-0004", Kind::Issue, "todo"),        // 자식이 남은 부모
            make("argos-0004.aaa", Kind::Issue, "todo"),    // 그 자식 — 이건 집는다
            make("argos-0005", Kind::Issue, "todo"),        // 평범한 것
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

    /// 막음 하나의 판정. 없는 것은 막지 않고, 미룬 것은 **여전히 막는다** —
    /// `ready` 가 빼는 것과 `held` 가 대는 것이 이 표 하나에서 나온다.
    #[test]
    fn a_blocker_is_judged_in_one_place() {
        assert_eq!(blocker(None, false, Waiting::Live), Blocker::Missing);
        assert_eq!(blocker(None, true, Waiting::Live), Blocker::Missing);
        assert_eq!(blocker(Some("done"), true, Waiting::Live), Blocker::Done);
        assert_eq!(blocker(Some("todo"), true, Waiting::Live), Blocker::Deferred);
        assert_eq!(blocker(Some("review"), false, Waiting::Live), Blocker::Open);
        // 미뤄 뺀 멤버 덕에 done 으로 선 묶음은 끝난 것이 아니다(moai-0gxf).
        assert_eq!(blocker(Some("done"), false, Waiting::Shelved), Blocker::Deferred);
        let blocks: Vec<bool> =
            [Blocker::Open, Blocker::Deferred, Blocker::Done, Blocker::Missing].map(Blocker::blocks).to_vec();
        assert_eq!(blocks, [true, true, false, false]);
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
        let issues = vec![make("argos-0001", Kind::Issue, "todo"), {
            let mut i = make("argos-0002", Kind::Issue, "todo");
            i.blocked_by = vec!["argos-0001".into()]; // 0001 이 0002 를 막는다
            i
        }];
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
        let st = status(&issues, &[], &cfg(), now, utc());
        let w = st.warnings.iter().find(|w| w.kind == "blocked_stale").expect("경고가 없다");
        assert_eq!(w.ids, ["argos-0002"]);

        // 막 막힌 것은 아직 말하지 않는다.
        let mut fresh = make("argos-0002", Kind::Issue, "todo");
        fresh.blocked_by = vec!["argos-0001".into()];
        fresh.status_since = now.to_string();
        let issues = vec![make("argos-0001", Kind::Issue, "todo"), fresh];
        let st = status(&issues, &[], &cfg(), now, utc());
        assert!(st.warnings.iter().all(|w| w.kind != "blocked_stale"));
    }

    /// **막힌 기간은 막음이 다시 선 때부터 잰다**(moai-xib6). 막힌 줄의 칸 나이로만 재면,
    /// 끝난 에픽에 멤버를 더하거나 접은 멤버를 도로 집어 에픽이 done 에서 되돌아 나오는 순간
    /// 그 에픽에 막힌 오래된 줄이 곧장 "N일째 막혔다" 로 선다.
    #[test]
    fn blocked_stale_counts_from_when_the_block_came_back() {
        // `make` 은 09-01 에 만든다 — 기준 시각에서 열흘 전이다.
        let (now, today) = ("2026-09-11T00:00:00Z", "2026-09-10T12:00:00Z");
        let blocked = |by: &str| {
            let mut x = make("argos-0009", Kind::Issue, "todo");
            x.blocked_by = vec![by.into()];
            x
        };
        let stale = |issues: &[Issue]| {
            status(issues, &[], &cfg(), now, utc()).warnings.iter().any(|w| w.kind == "blocked_stale")
        };
        let epic = || make("argos-0001", Kind::Epic, "todo");
        let finished = || member("argos-0002", "argos-0001", "done");

        // 오래 막힌 것은 그대로 꾸짖는다 — 기준선.
        assert!(stale(&[epic(), finished(), member("argos-0003", "argos-0001", "todo"), blocked("argos-0001")]));

        // 끝난 에픽에 오늘 멤버를 더했다 — 에픽이 막 다시 막았다.
        let mut added = member("argos-0003", "argos-0001", "todo");
        (added.created_at, added.status_since) = (today.into(), today.into());
        assert!(!stale(&[epic(), finished(), added, blocked("argos-0001")]), "막 다시 막은 줄을 N일째로 꾸짖는다");

        // 막는 이슈가 오늘 done 에서 되돌아 나왔다.
        let mut reopened = make("argos-0001", Kind::Issue, "todo");
        reopened.status_since = today.into();
        assert!(!stale(&[reopened, blocked("argos-0001")]));

        // **미루기·도로 집기는 막힘 시계를 새로 세우지 않는다**(moai-cxk8). 새로 세우면
        // `defer`→`--undo` 두 번이 열흘 막힘 경고를 지우는 손잡이가 된다. 막는 줄을 미뤄 둔
        // 동안은 미룬 막음도 막아(moai-2sea) `blocked_by_deferred` 로 드러나 있었고, 막힌 줄
        // 제가 미뤄 둔 동안은 경고에 안 들었지만 막음은 안 풀렸다.
        // 접어 둔 멤버를 오늘 도로 집었다.
        let mut back = member("argos-0003", "argos-0001", "todo");
        back.planned_at = Some(today.into());
        assert!(stale(&[epic(), finished(), back, blocked("argos-0001")]), "멤버를 도로 집은 것이 열흘 막힘을 지웠다");

        // 막힌 줄 제가 오늘 도로 집혔다.
        let mut mine = blocked("argos-0005");
        mine.planned_at = Some(today.into());
        assert!(
            stale(&[make("argos-0005", Kind::Issue, "todo"), mine]),
            "제 줄을 미뤘다 도로 집은 것이 열흘 막힘을 지웠다"
        );

        // 막는 이슈를 오늘 미뤘다가 도로 집었다.
        let mut shuffled = make("argos-0005", Kind::Issue, "todo");
        shuffled.planned_at = Some(today.into());
        assert!(stale(&[shuffled, blocked("argos-0005")]), "막는 줄을 미뤘다 도로 집은 것이 열흘 막힘을 지웠다");

        // 미뤄 둔 에픽을 오늘 도로 집었다.
        let mut undone = epic();
        undone.planned_at = Some(today.into());
        assert!(
            stale(&[undone, finished(), member("argos-0003", "argos-0001", "todo"), blocked("argos-0001")]),
            "묶음을 도로 집은 것이 열흘 막힘을 지웠다"
        );

        // 오래 막아 온 줄이 남아 있으면, 다른 막는 줄이 오늘 움직여도 줄곧 막혀 있었다.
        let mut two = blocked("argos-0005");
        two.blocked_by.push("argos-0006".into());
        let mut moved = make("argos-0006", Kind::Issue, "in_progress");
        moved.status_since = today.into();
        assert!(
            stale(&[make("argos-0005", Kind::Issue, "todo"), moved, two]),
            "오늘 움직인 막음 하나가 열흘 막힘을 가린다"
        );
    }

    /// **경고가 판정한 나이를 싣는다**(moai-7azq). 목록이 줄마다 제 칸 나이를 새로 재면, 칸에
    /// 30일 선 줄이 막음이 5일 전 다시 선 것으로 `blocked_stale` 에 걸려도 "30일" 로 선다.
    #[test]
    fn a_warning_carries_the_age_it_was_judged_by() {
        let now = "2026-10-01T00:00:00Z";
        // 막는 줄은 5일 전 done 에서 되돌아 나왔다. 막힌 줄은 `make` 이 09-01 에 만들어 칸에 30일.
        let mut blocker = make("argos-0001", Kind::Issue, "todo");
        blocker.status_since = "2026-09-26T00:00:00Z".into();
        let mut stuck = make("argos-0002", Kind::Issue, "todo");
        stuck.blocked_by = vec!["argos-0001".into()];
        let st = status(&[blocker, stuck], &[], &cfg(), now, utc());
        let w = st.warnings.iter().find(|w| w.kind == "blocked_stale").expect("막힘 경고가 없다");
        assert_eq!(w.ages.get("argos-0002"), Some(&5), "칸 나이를 댔다 — {w:?}");
        assert_eq!(w.ages.len(), w.ids.len());

        // 칸 나이로 거는 경고는 칸 나이를 싣는다.
        let mut picked = make("argos-0003", Kind::Issue, "in_progress");
        picked.status_since = "2026-09-11T00:00:00Z".into();
        let st = status(&[picked], &[], &cfg(), now, utc());
        let w = st.warnings.iter().find(|w| w.kind == "stale_progress").expect("잊은 것 경고가 없다");
        assert_eq!(w.ages.get("argos-0003"), Some(&20));

        // 같은 id 의 줄이 둘 다 걸리면 뒷줄의 나이다 — 보이는 쪽이 뒷줄의 칸·제목을 낸다.
        let mut front = make("argos-0005", Kind::Issue, "review");
        front.status_since = "2026-09-21T00:00:00Z".into(); // 10일
        let mut back = make("argos-0005", Kind::Issue, "review");
        back.status_since = "2026-09-27T00:00:00Z".into(); // 4일
        let st = status(&[front, back], &[], &cfg(), now, utc());
        let w = st.warnings.iter().find(|w| w.kind == "stale_review").expect("썩는 review 경고가 없다");
        assert_eq!(w.ages.get("argos-0005"), Some(&4), "앞줄 나이를 뒷줄 옆에 댔다");

        // 날짜로 안 거는 경고는 안 싣는다 — JSON 에서 키가 사라진다.
        let mut blocked = make("argos-0004", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-9999".into()];
        let st = status(&[blocked], &[], &cfg(), now, utc());
        let w = st.warnings.iter().find(|w| w.kind == "dangling_blocked_by").expect("끊긴 막음 경고가 없다");
        assert!(w.ages.is_empty());
        assert!(!serde_json::to_string(w).unwrap().contains("\"ages\""));
    }

    /// **미룬 것에 막힌 줄의 나이는 막는 쪽을 미룬 지 며칠이다**(moai-hcx3). 칸 나이를 대면
    /// "미룬 것에 N일 막힘" 으로 읽히는데 뜻이 다르다. 물려받은 미룸은 조상이 미뤄진 날,
    /// 막는 줄이 여럿이면 가장 이른 미룸이다.
    #[test]
    fn a_row_held_by_a_deferral_is_aged_from_the_deferral() {
        let now = "2026-10-01T00:00:00Z";
        let held = |id: &str, by: &[&str]| {
            let mut x = make(id, Kind::Issue, "todo"); // `make` 은 09-01 — 칸에 30일
            x.blocked_by = by.iter().map(|b| b.to_string()).collect();
            x
        };
        let aged = |issues: &[Issue], id: &str| {
            let st = status(issues, &[], &cfg(), now, utc());
            let w = st.warnings.iter().find(|w| w.kind == "blocked_by_deferred").expect("미룬 것에 막힘 경고가 없다");
            w.ages.get(id).copied()
        };

        // 제 줄을 12일 전에 미뤘다.
        let mut shelved = make("argos-0001", Kind::Issue, "todo");
        shelved.deferred_at = Some("2026-09-19T00:00:00Z".into());
        assert_eq!(
            aged(&[shelved.clone(), held("argos-0009", &["argos-0001"])], "argos-0009"),
            Some(12),
            "칸 나이를 댔다"
        );

        // 막는 줄은 20일 전에 미룬 에픽 밑이다 — 그 에픽을 미룬 날로 센다.
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-11T00:00:00Z".into());
        let under = member("argos-0003", "argos-0002", "todo");
        assert_eq!(aged(&[epic.clone(), under.clone(), held("argos-0009", &["argos-0003"])], "argos-0009"), Some(20));

        // 미룬 막음이 둘이면 가장 이른 것 — 모순이 선 때다. 칸 나이(30일)와 안 겹치게 26일.
        let mut older = make("argos-0004", Kind::Issue, "todo");
        older.deferred_at = Some("2026-09-05T00:00:00Z".into());
        assert_eq!(
            aged(&[shelved.clone(), older, held("argos-0009", &["argos-0001", "argos-0004"])], "argos-0009"),
            Some(26)
        );

        // 한 막는 줄의 미룸이 둘이면(제 미룸 40일, 에픽 20일) 가장 이른 것. 막음은 그보다 먼저 섰다.
        let mut both = member("argos-0003", "argos-0002", "todo");
        both.deferred_at = Some("2026-08-22T00:00:00Z".into());
        both.status_since = "2026-08-01T00:00:00Z".into();
        let mut long_held = held("argos-0009", &["argos-0003"]);
        long_held.status_since = "2026-08-01T00:00:00Z".into();
        assert_eq!(aged(&[epic.clone(), both, long_held], "argos-0009"), Some(40));

        // **막음이 선 날로 누른다.** 40일 전에 미룬 줄에 이틀 전 막힌 줄은 모순이 이틀째다.
        let mut fresh = held("argos-0009", &["argos-0003"]);
        fresh.status_since = "2026-09-29T00:00:00Z".into();
        assert_eq!(aged(&[epic.clone(), under.clone(), fresh], "argos-0009"), Some(2), "막음보다 이른 미룸으로 셌다");
        // 막는 줄이 어제 done 에서 되돌아 나와 미룬 에픽 밑에 다시 섰다.
        let mut reopened = member("argos-0003", "argos-0002", "todo");
        reopened.status_since = "2026-09-30T00:00:00Z".into();
        assert_eq!(aged(&[epic.clone(), reopened, held("argos-0009", &["argos-0003"])], "argos-0009"), Some(1));

        // 묶음을 제가 미뤘다 — 그 묶음을 미룬 날.
        let mut shelved_epic = make("argos-0002", Kind::Epic, "todo");
        shelved_epic.deferred_at = Some("2026-09-16T00:00:00Z".into());
        assert_eq!(aged(&[shelved_epic, under.clone(), held("argos-0009", &["argos-0002"])], "argos-0009"), Some(15));

        // 끝난 것으로 읽는 묶음이 미룬 멤버를 기다린다 — 묶음이 아니라 그 멤버를 미룬 날.
        let open_epic = make("argos-0002", Kind::Epic, "todo");
        let mut aside = member("argos-0005", "argos-0002", "todo");
        aside.deferred_at = Some("2026-09-21T00:00:00Z".into());
        let rows = [open_epic, member("argos-0006", "argos-0002", "done"), aside, held("argos-0009", &["argos-0002"])];
        assert_eq!(aged(&rows, "argos-0009"), Some(10));
    }

    /// 막는 쪽이 사라지면 `ready` 는 조용히 넘어가지만 `status` 는 드러낸다.
    #[test]
    fn status_warns_about_a_dangling_blocker() {
        let mut i = make("argos-0001", Kind::Issue, "todo");
        i.blocked_by = vec!["argos-9999".into()];
        let st = status(&[i], &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "dangling_blocked_by").expect("경고가 없다");
        assert_eq!(w.ids, ["argos-0001"]);
    }

    /// 자식이 다 끝나면 부모를 집을 수 있다.
    #[test]
    fn a_parent_becomes_ready_once_children_close() {
        let issues = vec![make("argos-0004", Kind::Issue, "todo"), make("argos-0004.aaa", Kind::Issue, "done")];
        let got: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0004"]);
    }

    /// **이로써 풀린 일만 댄다**(moai-942k). 막는 줄과 마지막 자식을 닫으면 막혔던 줄과 부모가
    /// 새로 ready 가 된다. 닫은 줄, 원래 ready 이던 줄, 여전히 막힌 줄은 안 댄다.
    #[test]
    fn unblocked_names_only_what_the_write_just_freed() {
        let mut blocked = make("argos-0002", Kind::Issue, "todo");
        blocked.blocked_by = vec!["argos-0001".into()];
        let mut still = make("argos-0006", Kind::Issue, "todo");
        still.blocked_by = vec!["argos-0001".into(), "argos-0005".into()];
        let before = vec![
            make("argos-0001", Kind::Issue, "in_progress"), // 막는 줄
            blocked,
            make("argos-0003", Kind::Issue, "todo"),     // 부모
            make("argos-0003.aaa", Kind::Issue, "todo"), // 그 마지막 자식
            make("argos-0004", Kind::Issue, "todo"),     // 원래 ready
            make("argos-0005", Kind::Issue, "todo"),     // 아직 안 닫힌 막는 줄
            still,
        ];
        let mut after = before.clone();
        for i in after.iter_mut().filter(|i| i.id == "argos-0001" || i.id == "argos-0003.aaa") {
            i.status = Status::new("done");
        }
        let got: BTreeSet<&str> = unblocked(&before, &after, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, BTreeSet::from(["argos-0002", "argos-0003"]));
        // 아무것도 안 바꾼 쓰기는 풀린 것이 없다.
        assert!(unblocked(&before, &before, &cfg()).is_empty());
    }

    /// **이미 시작한 부모는 `unblocked` 에 안 든다**(moai-j4xs). ready 는 첫 칸의 줄만 세므로,
    /// 마지막 자식을 닫아도 `in_progress` 에 선 부모는 두 판을 견주는 자에 안 걸린다 — 그
    /// 부모야말로 "이제 닫으면 된다" 는 말을 받을 줄이다.
    ///
    /// **첫 칸의 부모는 여기 안 든다** — 그쪽은 `unblocked` 가 이미 댄다. 둘이 겹치면 한 줄이
    /// 두 번 불려 읽는 쪽이 두 건으로 센다.
    #[test]
    fn closable_names_the_started_parent_whose_last_child_just_closed() {
        let before = vec![
            make("argos-0001", Kind::Issue, "in_progress"), // 시작한 부모
            make("argos-0001.aaa", Kind::Issue, "todo"),    // 그 마지막 자식
            make("argos-0002", Kind::Issue, "todo"),        // 첫 칸의 부모
            make("argos-0002.aaa", Kind::Issue, "todo"),
            make("argos-0003", Kind::Issue, "in_progress"), // 자식이 남은 부모
            make("argos-0003.aaa", Kind::Issue, "todo"),
            make("argos-0003.bbb", Kind::Issue, "todo"),
        ];
        let mut after = before.clone();
        for i in after.iter_mut().filter(|i| i.id.ends_with(".aaa")) {
            i.status = Status::new("done");
        }
        let got: Vec<&str> = closable(&before, &after, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-0001"], "시작한 부모 하나만 든다");
        let freed: BTreeSet<&str> = unblocked(&before, &after, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert!(freed.contains("argos-0002"), "첫 칸의 부모는 전처럼 `unblocked` 가 댄다");
        assert!(!freed.contains("argos-0001"), "같은 줄이 두 자리에서 불렸다");
        // 아무것도 안 바꾼 쓰기는 닫을 수 있게 된 것도 없다.
        assert!(closable(&before, &before, &cfg()).is_empty());
    }

    /// **같은 에픽의 다음 일은 에픽마다 하나**(moai-j4xs). 이미 ready 이던 줄이라 두 판을
    /// 견주는 [`unblocked`] 에는 안 들고, 멤버 하나를 닫은 자리에서 가장 자주 묻는 것이 이것이다.
    #[test]
    fn next_of_names_one_pick_per_epic_of_what_just_closed() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-000a", "argos-0001", "done"), // 방금 닫은 멤버
            member("argos-000b", "argos-0001", "todo"), // 그 에픽의 다음
            member("argos-000c", "argos-0001", "todo"), // 목록은 `moai ready` 의 일이다
            make("argos-0002", Kind::Epic, "todo"),
            member("argos-000d", "argos-0002", "todo"), // 남의 에픽 — 안 댄다
        ];
        let got: Vec<&str> = next_of(&issues, &cfg(), &["argos-000a"], &[]).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000b"], "그 에픽의 다음 하나만 든다");
        // **`unblocked` 에 이미 든 줄은 빼고 고른다** — 같은 줄을 두 줄에 적으면 두 건으로 읽힌다.
        let said = [&issues[2]];
        let got: Vec<&str> = next_of(&issues, &cfg(), &["argos-000a"], &said).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000c"], "이미 댄 줄을 또 댔다");
        // **에픽 없는 줄을 닫은 것은 여기서 할 말이 없다** — "같은 에픽" 이 없다. 이것을
        // 안 가르면 에픽 없는 일 하나를 닫을 때마다 아무 에픽의 줄이나 "다음" 으로 선다.
        let mut loose = issues.clone();
        loose.push(make("argos-000e", Kind::Issue, "done"));
        assert!(next_of(&loose, &cfg(), &["argos-000e"], &[]).is_empty(), "에픽 없는 줄에 남의 다음을 댔다");
        assert!(next_of(&issues, &cfg(), &[], &[]).is_empty(), "닫은 것이 없으면 다음도 없다");
    }

    /// 급한 것 먼저, 그다음 끝나가는 에픽 먼저.
    #[test]
    fn ready_sorts_urgent_then_nearly_finished() {
        let mut issues = vec![
            make("argos-0001", Kind::Epic, "todo"), // 0% 에픽
            make("argos-0002", Kind::Epic, "todo"), // 50% 에픽
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

    /// 마일스톤 하나와 그 안의 에픽·멤버, 그리고 밖에 선 일 둘(`p2`·`p0`).
    /// `start` 면 안의 멤버 하나를 집은 채로 둔다 — 그것이 "도는 중" 의 뜻이다.
    fn with_milestone(start: bool) -> Vec<Issue> {
        let mut stone = make("argos-m001", Kind::Milestone, "todo");
        stone.title = "v0.1".into();
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-m001".into());
        let mut hot = make("argos-000z", Kind::Issue, "todo");
        hot.priority = Some(0);
        vec![
            stone,
            epic,
            member("argos-001a", "argos-0001", if start { "in_progress" } else { "todo" }),
            member("argos-001b", "argos-0001", "todo"),
            make("argos-000y", Kind::Issue, "todo"), // 밖의 p2
            hot,                                     // 밖의 p0
        ]
    }

    /// 마일스톤에 **든 시간**은 닫힌 멤버에서 지금 센다(moai-wfup). 저장하지 않는다.
    ///
    /// 재는 자리가 지키는 것 넷이다 — 묶음 멤버(에픽)는 안 센다(그 밑의 이슈가 이미 들어
    /// 있으므로 함께 세면 두 번 더해진다), **멤버의 자식도 같은 까닭으로 안 센다**(리뷰: 이
    /// 저장소는 리뷰를 자식으로 세우고 그 구간은 부모 안에 든다), 시작이 안 적힌 줄은
    /// **모르는 것**이라 `closed` 에는 들고 `measured` 에는 안 든다, 끝이 시작보다 앞선 줄은
    /// 0 으로 안 누른다.
    #[test]
    fn a_milestone_counts_only_the_members_it_could_actually_measure() {
        let mut stone = make("argos-m001", Kind::Milestone, "todo");
        stone.title = "v0.1".into();
        let mut epic = make("argos-0001", Kind::Epic, "done");
        epic.milestone = Some("argos-m001".into());
        // 에픽 줄에도 두 값이 선다 — 손으로 친 `mv` 의 것이라 세면 안 된다.
        epic.started_at = Some("2026-09-01T00:00:00Z".into());
        epic.done_at = Some("2026-09-02T00:00:00Z".into());
        let span = |id: &str, from: &str, to: &str| {
            let mut i = member(id, "argos-0001", "done");
            i.started_at = Some(from.into());
            i.done_at = Some(to.into());
            i
        };
        let mut unmeasured = member("argos-001c", "argos-0001", "done");
        unmeasured.done_at = Some("2026-09-01T06:00:00Z".into()); // 옛 바이너리가 옮긴 줄
        let mut backwards = member("argos-001d", "argos-0001", "done");
        backwards.started_at = Some("2026-09-01T06:00:00Z".into());
        backwards.done_at = Some("2026-09-01T05:00:00Z".into()); // 손으로 푼 줄
        let all = vec![
            stone.clone(),
            epic,
            span("argos-001a", "2026-09-01T00:00:00Z", "2026-09-01T01:00:00Z"), // 60분
            span("argos-001b", "2026-09-01T00:00:00Z", "2026-09-01T03:00:00Z"), // 180분
            unmeasured,
            backwards,
            member("argos-001e", "argos-0001", "todo"), // 아직 안 닫혔다
        ];

        let sp = spent_on(&all, &stone);
        assert_eq!(sp.closed, 4, "닫힌 멤버를 잘못 셌다 — {sp:?}");
        assert_eq!(sp.measured, 2, "모르는 것을 잰 것으로 셌다 — {sp:?}");
        assert_eq!(sp.minutes, 240, "{sp:?}");
        // 짝수 개의 중앙값은 가운데 둘의 평균이다.
        assert_eq!(sp.median, Some(120), "{sp:?}");

        // 홀수 개면 가운데 하나다.
        let mut odd = all.clone();
        odd.push(span("argos-001f", "2026-09-01T00:00:00Z", "2026-09-01T02:00:00Z"));
        assert_eq!(spent_on(&odd, &odd[0]).median, Some(120), "{:?}", spent_on(&odd, &odd[0]));

        // **잰 부모의 자식은 그 구간 안이다**(리뷰) — 이 저장소의 규약이 리뷰를 자식으로
        // 세우므로(`AGENTS.md` 훅 규칙 3) 흔한 판이고, 같이 더하면 한 시간이 두 시간으로 선다.
        // 시간만 접는다: 닫힌 수는 막대와 같아야 하므로 자식도 그대로 센다.
        let mut kid = all.clone();
        kid.push(span("argos-001a.r3q", "2026-09-01T00:10:00Z", "2026-09-01T00:40:00Z")); // 부모(001a) 구간 안
        kid.push(span("argos-001a.r3q.9zz", "2026-09-01T00:15:00Z", "2026-09-01T00:20:00Z")); // 손자도 같다
        let with_kid = spent_on(&kid, &stone);
        assert_eq!((with_kid.measured, with_kid.minutes, with_kid.median), (2, 240, Some(120)), "{with_kid:?}");
        assert_eq!(with_kid.closed, sp.closed + 2, "자식을 닫힌 수에서 뺐다 — 막대와 갈라진다 {with_kid:?}");

        // **차례가 뒤집혀도 접힌다.** 멤버는 `display_order` 로 오는데 그것은 급한 것을 앞에
        // 세우므로, `p0` 자식이 제 부모보다 앞에 설 수 있다 — 그때 들어온 차례대로 세면 부모를
        // 보기 전에 자식을 세어 이중 셈이 그대로 돌아온다(리뷰).
        let mut hasty = all.clone();
        let mut first = span("argos-001a.aaa", "2026-09-01T00:10:00Z", "2026-09-01T00:40:00Z");
        first.priority = Some(0);
        hasty.push(first);
        let jumped = spent_on(&hasty, &stone);
        assert_eq!((jumped.measured, jumped.minutes), (2, 240), "급한 자식이 제 부모를 앞질러 따로 세졌다 {jumped:?}");

        // **부모가 아무 구간도 못 냈으면 자식이 유일한 증거다.** `argos-001c` 는 닫혔지만 시작이
        // 안 적혔고(옛 바이너리), `argos-001e` 는 아직 안 닫혔다 — 둘의 자식은 제 몫으로 선다.
        let mut only = all.clone();
        only.push(span("argos-001c.r3q", "2026-09-01T00:00:00Z", "2026-09-01T00:30:00Z"));
        only.push(span("argos-001e.r3q", "2026-09-01T00:00:00Z", "2026-09-01T00:45:00Z"));
        let evidence = spent_on(&only, &stone);
        assert_eq!((evidence.closed, evidence.measured, evidence.minutes), (6, 4, 315), "{evidence:?}");

        // 부모가 이 묶음에 아예 없어도 자식이 제 몫으로 선다.
        let mut orphan = all.clone();
        orphan.push(span("argos-009x.r3q", "2026-09-01T00:00:00Z", "2026-09-01T00:30:00Z"));
        let alone = spent_on(&orphan, &stone);
        assert_eq!((alone.closed, alone.measured, alone.minutes), (5, 3, 270), "{alone:?}");

        // 묶음이 아닌 줄에는 빈 값이다.
        assert_eq!(spent_on(&all, &all[2]), Spent::default());
    }

    /// **마일스톤이 도는 동안 그 안이 먼저다**(moai-q04l, 2026-09-20 사용자 결정).
    ///
    /// 밖의 일은 `p0` 만 남고 나머지는 [`Focus::outside`] 로 빠진다 — 지우는 것이 아니라
    /// 대는 것이다. 막지 않기로 한 결정이 여기서 서므로, 뺀 줄이 목록에 남아야 부르는 쪽이
    /// 왜 짧은지 말할 수 있다.
    #[test]
    fn a_running_milestone_pulls_its_members_up_and_leaves_only_p0_outside() {
        let issues = with_milestone(true);
        let (picks, focus) = ready_in(&issues, &cfg());
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        // 밖의 `p0` 이 먼저다 — 핫픽스는 마일스톤보다 급하다. 그다음이 도는 마일스톤의 남은 멤버.
        assert_eq!(got, ["argos-000z", "argos-001b"], "{got:?}");
        assert_eq!(focus.running.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["argos-m001"]);
        let held: Vec<&str> = focus.outside.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(held, ["argos-000y"], "밖의 p2 를 안 뺐거나 어디에도 안 담았다 — {held:?}");
    }

    /// **세워 두기만 한 마일스톤은 안 돈다.** 시작을 멤버에서 읽으므로(`Stand::busy`),
    /// 아무도 집지 않은 동안은 규칙이 서지 않고 밖의 일이 그대로 선다 — 사용자가 고른 대가다.
    #[test]
    fn a_milestone_nobody_has_started_changes_nothing() {
        let issues = with_milestone(false);
        let (picks, focus) = ready_in(&issues, &cfg());
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        assert!(focus.running.is_empty() && focus.outside.is_empty(), "{focus:?}");
        assert!(got.contains(&"argos-000y"), "안 도는데 밖의 일을 뺐다 — {got:?}");
        assert_eq!(got.len(), 4, "{got:?}");
    }

    /// **미뤄 둔 마일스톤은 안 돈다.** 계획에서 뺀 것이 밖의 일을 밀어내면, 미루기가 도리어
    /// 규칙을 세우는 손잡이가 된다 — 미룬 묶음의 멤버는 애초에 `ready` 에도 없다.
    #[test]
    fn a_deferred_milestone_does_not_run() {
        let mut issues = with_milestone(true);
        issues[0].deferred_at = Some("2026-09-02T00:00:00Z".into());
        let (picks, focus) = ready_in(&issues, &cfg());
        assert!(focus.running.is_empty(), "{focus:?}");
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000z", "argos-000y"], "미룬 마일스톤이 밖의 일을 밀어냈다 — {got:?}");
    }

    /// **도는 마일스톤이 둘이면 둘 다 안이다**(사용자 결정 3). 그 안의 차례는 지금까지와 같다.
    #[test]
    fn two_running_milestones_are_both_inside() {
        let mut issues = with_milestone(true);
        let mut other = make("argos-m002", Kind::Milestone, "todo");
        other.title = "v0.2".into();
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-m002".into());
        issues.push(other);
        issues.push(epic);
        issues.push(member("argos-002a", "argos-0002", "in_progress"));
        issues.push(member("argos-002b", "argos-0002", "todo"));
        let (picks, focus) = ready_in(&issues, &cfg());
        let running: Vec<&str> = focus.running.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(running, ["argos-m001", "argos-m002"], "{running:?}");
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000z", "argos-001b", "argos-002b"], "{got:?}");
    }

    /// **멤버와 멤버 사이에서 꺼지지 않는다**(리뷰 moai-493a.2om 1).
    ///
    /// 집은 멤버를 닫고 다음 멤버를 집기 전의 틈 — 하필 다음에 무엇을 집을지 고르는 그
    /// 순간에 규칙이 사라지면, `ready` 가 밖의 일을 통째로 도로 낸다. 읽은 칸으로 재면 첫
    /// 멤버를 집는 순간 서서 마지막 멤버가 닫힐 때까지 그대로다.
    #[test]
    fn a_running_milestone_does_not_flicker_between_members() {
        let mut issues = with_milestone(true);
        // 집고 있던 멤버를 닫는다. 남은 멤버는 아직 첫 칸이다.
        issues[2].status = Status::new("done");
        let (picks, focus) = ready_in(&issues, &cfg());
        let running: Vec<&str> = focus.running.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(running, ["argos-m001"], "멤버 사이의 틈에서 규칙이 꺼졌다");
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000z", "argos-001b"], "{got:?}");
    }

    /// **다 끝난 마일스톤은 안 돈다.** 읽은 칸이 `done` 이면 그 마일스톤은 끝난 것이고,
    /// 그때부터 밖의 일이 다시 선다 — 규칙이 영영 안 풀리는 자리를 만들지 않는다.
    #[test]
    fn a_finished_milestone_stops_running() {
        let mut issues = with_milestone(true);
        issues[2].status = Status::new("done");
        issues[3].status = Status::new("done");
        let (picks, focus) = ready_in(&issues, &cfg());
        assert!(focus.running.is_empty(), "{focus:?}");
        let got: Vec<&str> = picks.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(got, ["argos-000z", "argos-000y"], "끝난 마일스톤이 밖의 일을 붙들었다 — {got:?}");
    }

    /// **마일스톤 우선은 막음이 아니다**(리뷰 moai-493a.2om 2).
    ///
    /// 마일스톤이 끝나는 쓰기를 거른 목록으로 견주면, 아무도 안 막던 밖의 일이 통째로
    /// "이로써 풀림" 으로 선다 — `moai mv <멤버> done` 한 줄이 거짓말을 한다.
    #[test]
    fn finishing_a_milestone_does_not_call_outside_work_unblocked() {
        let before = {
            let mut i = with_milestone(true);
            i[3].status = Status::new("done");
            i
        };
        let after = {
            let mut i = before.clone();
            i[2].status = Status::new("done"); // 마지막 멤버를 닫는다 — 마일스톤이 끝난다
            i
        };
        let freed: Vec<&str> = unblocked(&before, &after, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert!(freed.is_empty(), "막지도 않던 밖의 일을 풀렸다고 말한다 — {freed:?}");
    }

    /// **보드가 그것을 알림 한 줄로 댄다**(moai-tvvb). 경고가 아니라 알림이고, 세는 것은
    /// 밖에 남은 일이다 — `p0` 은 밖에 있어도 집으므로 안 센다.
    #[test]
    fn the_board_says_a_milestone_is_running_and_counts_what_waits_outside() {
        let issues = with_milestone(true);
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let w = st.notices.iter().find(|w| w.kind == "milestone_focus").expect("알림이 없다");
        assert!(w.notice && !w.fatal, "{w:?}");
        assert_eq!(w.ids, ["argos-m001"], "{w:?}");
        assert_eq!(w.count, 1, "밖에 남은 일을 잘못 셌다(p0 을 셌나) — {w:?}");
        assert!(
            !st.warnings.iter().any(|w| w.kind == "milestone_focus"),
            "알림이 경고로 섰다 — 세션을 붙드는 자리가 된다"
        );
        // 안 도는 저장소에는 줄이 없다.
        let quiet = status(&with_milestone(false), &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(!quiet.notices.iter().any(|w| w.kind == "milestone_focus"), "{:?}", quiet.notices);
        // **밖에 남은 것이 없으면 도는 중이어도 말하지 않는다**(리뷰 6) — `밖에 남은 일 0건`
        // 은 아무것도 안 말하면서 자리만 차지한다. 도는 것은 마일스톤 표가 이미 낸다.
        let only = {
            let mut i = with_milestone(true);
            i.retain(|x| !x.id.starts_with("argos-000")); // 밖의 줄을 치운다
            i
        };
        let bare = status(&only, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(!bare.notices.iter().any(|w| w.kind == "milestone_focus"), "0 건을 말했다 — {:?}", bare.notices);
    }

    /// **끝나가는지는 칸 셈과 같은 자로 잰다**(moai-ha03). 미룬 멤버까지 센 막대로 재면, 한 번만
    /// 집으면 닫히는 에픽(끝난 1 · 미룬 3 · 남은 1 = 20%)이 두 번 집어야 하는 에픽(끝난 1 · 남은 2
    /// = 33%)보다 뒤에 선다 — 미룬 멤버는 칸 셈에서 빠지는데 차례만 그것을 셌다.
    #[test]
    fn nearly_finished_is_measured_without_deferred_members() {
        let put_off = |id: &str| {
            let mut m = member(id, "argos-0002", "todo");
            m.deferred_at = Some("2026-09-01T00:00:00Z".into());
            m
        };
        let mut issues = vec![
            make("argos-0001", Kind::Epic, "todo"), // 끝난 1 · 남은 2
            member("argos-001a", "argos-0001", "done"),
            member("argos-001b", "argos-0001", "todo"),
            member("argos-001c", "argos-0001", "todo"),
            make("argos-0002", Kind::Epic, "todo"), // 끝난 1 · 미룬 3 · 남은 1
            member("argos-002a", "argos-0002", "done"),
            put_off("argos-002b"),
            put_off("argos-002c"),
            put_off("argos-002d"),
            member("argos-002e", "argos-0002", "todo"),
        ];
        // 한 번에 닫히는 에픽의 남은 일을 가장 늦게 만든다 — 차례가 나이로 갈리면 이 시험이 못 본다.
        issues[9].created_at = "2026-09-09T00:00:00Z".into();
        let got = picks(&issues);
        assert_eq!(got, ["argos-002e", "argos-001b", "argos-001c"], "{got:?}");
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
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        assert!(st.warnings.is_empty(), "{:?}", kinds(&st));
        assert!(!st.broken());
    }

    /// 에픽에 안 붙은 것이 제일 중요한 신호다 — 마일스톤이 아직 없으므로.
    #[test]
    fn loose_issues_are_the_headline() {
        let issues: Vec<Issue> = (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "todo")).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        assert_eq!(kinds(&st), ["no_epic"]);
        assert_eq!(st.warnings[0].count, 5);
        assert_eq!(st.warnings[0].hint.as_deref(), Some("moai show -e none"));

        // 끝난 것은 세지 않는다 — 이미 지나간 일이다
        let done: Vec<Issue> = (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "done")).collect();
        assert!(status(&done, &[], &cfg(), "2026-09-01T00:00:00Z", utc()).warnings.is_empty());
    }

    /// 게이트를 없앤 대가는 review 가 썩는 것이다. 날짜로 잰다.
    #[test]
    fn review_rot_is_measured_in_days() {
        let i = at(make("argos-0001", Kind::Issue, "review"), "2026-09-01T00:00:00Z", "2026-09-01T00:00:00Z");
        let issues = vec![i];
        let early = status(&issues, &[], &cfg(), "2026-09-03T00:00:00Z", utc());
        assert!(!kinds(&early).contains(&"stale_review"));
        let st = status(&issues, &[], &cfg(), "2026-09-06T00:00:00Z", utc());
        assert!(kinds(&st).contains(&"stale_review"), "{:?}", kinds(&st));
        assert_eq!(st.warnings.iter().find(|w| w.kind == "stale_review").unwrap().days, Some(3));
    }

    #[test]
    fn too_much_at_once_is_named() {
        let issues: Vec<Issue> = (0..4).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "in_progress")).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "wip_overload").unwrap();
        assert_eq!((w.count, w.limit), (4, Some(3)));
        // 셋까지는 말하지 않는다
        let three = status(&issues[..3], &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        assert!(!kinds(&three).contains(&"wip_overload"));
    }

    #[test]
    fn an_unfilled_plan_shows_and_finished_work_is_not_scolded() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),        // 빈 에픽
            make("argos-0002", Kind::Epic, "in_progress"), // 다 끝났고 적힌 칸은 안 닫힘
            member("argos-0003", "argos-0002", "done"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        let k = kinds(&st);
        assert!(k.contains(&"dangling_epic") && k.contains(&"orphan_child"), "{k:?}");
        assert!(!st.broken(), "드러낼 것을 깨진 것으로 셌다");

        let dup = vec![make("argos-0001", Kind::Issue, "todo"), make("argos-0001", Kind::Issue, "todo")];
        assert!(status(&dup, &[], &cfg(), "2026-09-01T00:00:00Z", utc()).broken());
        assert!(status(&[], &[Unreadable { id: None }], &cfg(), "2026-09-01T00:00:00Z", utc()).broken());
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
        let st = status(&issues, &[], &cfg(), "2026-09-10T00:00:00Z", utc());
        assert_eq!(st.flow, Flow { days: 7, created: 3, done: 1, net: 2 });
    }

    /// 에픽은 칸 집계에 세지 않는다 — 에픽은 하는 일이 아니다.
    #[test]
    fn the_board_counts_work_not_epics() {
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "todo")];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
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

    /// **에픽 줄은 제 `milestone` 에만 서고, 멤버는 에픽 줄이 선 곳을 받는다.**
    /// 에픽 줄이 든 `epic` 과 id 부모는 그 줄의 자리가 아니다 — `nav` 는 에픽을
    /// 마일스톤 밑에만 둔다. 한때 끊긴 `epic` 을 든 에픽 줄은 마일스톤을 못 받는데
    /// 멤버는 그 줄의 필드를 다시 읽어 세어졌다(moai-0prl).
    #[test]
    fn an_epic_line_stands_in_its_own_milestone() {
        let with = |mut i: Issue, epic: Option<&str>, stone: Option<&str>| {
            i.epic = epic.map(Into::into);
            i.milestone = stone.map(Into::into);
            i
        };
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            make("argos-m002", Kind::Milestone, "todo"),
            with(make("argos-0001", Kind::Epic, "todo"), Some("argos-nope"), Some("argos-m001")), // 끊긴 epic
            member("argos-0002", "argos-0001", "todo"),
            with(make("argos-0003", Kind::Epic, "todo"), None, Some("argos-m002")),
            with(make("argos-0004", Kind::Epic, "todo"), Some("argos-0003"), Some("argos-m001")), // 에픽 안 에픽
            member("argos-0005", "argos-0004", "todo"),
            with(make("argos-0006", Kind::Epic, "todo"), Some("argos-0003"), None), // 제 것이 없다
            member("argos-0007", "argos-0006", "todo"),
            with(make("argos-0008", Kind::Issue, "todo"), None, Some("argos-m002")),
            with(make("argos-0008.aa1", Kind::Epic, "todo"), None, Some("argos-m001")), // 이슈 밑 id
            member("argos-0009", "argos-0008.aa1", "todo"),
            make("argos-0008.bb2", Kind::Epic, "todo"), // 이슈 밑 id, 제 것이 없다
            // 에픽 줄 밑에 id 로 선 이슈 — 에픽이 없어 부모를 타다 에픽 줄에서
            // 멈춘다. 넘어가면 그 에픽 줄이 안 받은 이슈의 마일스톤을 받는다.
            make("argos-0008.bb2.cc3", Kind::Issue, "todo"),
        ];
        let m = milestones(&issues);
        for (id, want) in [
            ("argos-0001", Some("argos-m001")),
            ("argos-0002", Some("argos-m001")),
            ("argos-0004", Some("argos-m001")),
            ("argos-0005", Some("argos-m001")),
            ("argos-0006", None),
            ("argos-0007", None),
            ("argos-0008.aa1", Some("argos-m001")),
            ("argos-0009", Some("argos-m001")),
            ("argos-0008.bb2", None),
            ("argos-0008.bb2.cc3", None),
        ] {
            assert_eq!(m.get(id).copied(), want, "{id}");
        }
        // 자리는 안 바꿔도 못 쓸 참조는 드러난다.
        assert_eq!(broken(&issues).get("argos-0001"), Some(&Misplace::Epic));
        assert!(!misplaced(&issues).contains_key("argos-0001"), "에픽 줄을 제 epic 으로 길 잃게 했다");
    }

    /// **마일스톤 줄은 어느 마일스톤에도 안 든다**(moai-8tav). 제 `milestone` 필드로도, 마일스톤을
    /// 든 이슈 밑에 id 로 서도 — `nav` 는 마일스톤을 언제나 뿌리에 둔다. 마일스톤 줄 밑의 일은
    /// 여전히 그 마일스톤에 든다.
    #[test]
    fn a_milestone_line_belongs_to_no_milestone() {
        let stone = |mut i: Issue, m: &str| {
            i.milestone = Some(m.into());
            i
        };
        let issues = vec![
            make("argos-m002", Kind::Milestone, "todo"),
            stone(make("argos-m001", Kind::Milestone, "todo"), "argos-m002"), // 제 필드
            stone(make("argos-0001", Kind::Issue, "todo"), "argos-m002"),
            make("argos-0001.aa1", Kind::Milestone, "todo"), // 마일스톤을 든 이슈 밑 id
            make("argos-m001.bb2", Kind::Issue, "todo"),     // 마일스톤 줄 밑의 일
        ];
        let m = milestones(&issues);
        assert_eq!(m.get("argos-m001"), None, "마일스톤 줄이 제 milestone 필드로 다른 마일스톤에 들었다");
        assert_eq!(m.get("argos-0001.aa1"), None, "이슈 밑 마일스톤 줄이 그 이슈의 마일스톤에 들었다");
        assert_eq!(m.get("argos-m001.bb2").copied(), Some("argos-m001"), "마일스톤 줄 밑의 일이 제 마일스톤을 잃었다");
        // 머리글이 세는 멤버와 같은 것을 말한다 — 마일스톤 줄은 어느 쪽에도 안 선다.
        let m2 = issues.iter().find(|i| i.id == "argos-m002").unwrap();
        assert!(group_members(&issues, m2).iter().all(|i| i.kind != Kind::Milestone));
        assert!(!misplaced(&issues).contains_key("argos-m001"));
    }

    /// 묶음은 일이 아니다. 세면 보드의 숫자가 할 일과 묶음을 합친 것이 된다.
    #[test]
    fn groupings_are_not_work() {
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "todo"),
        ];
        let st = status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        assert_eq!(st.total, 1);
        assert_eq!(st.counts.get("todo"), Some(&1));
        assert!(ready(&issues, &cfg()).iter().all(|i| i.id == "argos-0002"));
    }

    /// 마일스톤을 안 쓰는 저장소에는 마일스톤 이야기를 꺼내지 않는다.
    #[test]
    fn milestones_stay_quiet_until_used() {
        let plain = vec![make("argos-0001", Kind::Issue, "todo")];
        let st = status(&plain, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
        assert!(st.milestones.is_empty());
        assert!(!kinds(&st).contains(&"no_milestone"), "{:?}", kinds(&st));

        let mut used = plain.clone();
        used.push(make("argos-m001", Kind::Milestone, "todo"));
        let st = status(&used, &[], &cfg(), "2026-09-01T00:00:00Z", utc());
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

    /// **부모가 묶음이면 그 묶음이 소속이다** (moai-9t3l). 가장자리마다 한 줄 —
    /// 제 필드가 이기고, 사슬을 타고, 묶음 줄은 안 받고, 없는 부모는 안 넘는다.
    #[test]
    fn a_child_of_a_group_takes_that_group() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.milestone = Some("argos-m001".into());
        epic.epic = Some("argos-0002".into()); // 에픽 줄이 든 `epic` 은 안 내려온다
        let mut own_epic = make("argos-0001.aa2", Kind::Issue, "todo");
        own_epic.epic = Some("argos-0002".into());
        let mut own_stone = make("argos-m001.cc2", Kind::Issue, "todo");
        own_stone.milestone = Some("argos-m002".into());
        let issues = vec![
            make("argos-m001", Kind::Milestone, "todo"),
            make("argos-m002", Kind::Milestone, "todo"),
            epic,
            make("argos-0002", Kind::Epic, "todo"),
            make("argos-0001.aa1", Kind::Issue, "todo"),
            make("argos-0001.aa1.bb1", Kind::Issue, "todo"),
            own_epic,
            make("argos-0001.aa3", Kind::Idea, "todo"),
            make("argos-0001.ee1", Kind::Epic, "todo"),
            make("argos-0001.aa1.ee2", Kind::Epic, "todo"),
            make("argos-0001.aa2.ee3", Kind::Epic, "todo"),
            make("argos-m001.cc1", Kind::Issue, "todo"),
            own_stone,
            make("argos-m001.dd1", Kind::Milestone, "todo"),
            make("argos-zzzz.ff1", Kind::Issue, "todo"),
        ];
        let (e, m) = (groups(&issues), milestones(&issues));
        for (id, epic, stone) in [
            ("argos-0001.aa1", Some("argos-0001"), Some("argos-m001")),
            ("argos-0001.aa1.bb1", Some("argos-0001"), Some("argos-m001")), // 사슬을 탄다
            ("argos-0001.aa2", Some("argos-0002"), None),                   // 제 에픽이 이긴다
            ("argos-0001.aa3", Some("argos-0001"), Some("argos-m001")),     // 생각도 받는다
            ("argos-0001", None, Some("argos-m001")), // 에픽 줄은 제 `epic` 으로도 안 든다(moai-fg0t)
            ("argos-0001.ee1", None, None),           // 에픽 줄은 부모 에픽도, 그 에픽이 든 `epic` 도 안 받는다
            ("argos-0001.aa1.ee2", None, None),       // 부모 이슈의 에픽도 안 받는다(moai-k9yb)
            ("argos-0001.aa2.ee3", None, None),       // 부모 이슈가 적은 `epic` 도
            ("argos-m001.cc1", None, Some("argos-m001")),
            ("argos-m001.cc2", None, Some("argos-m002")), // 제 마일스톤이 이긴다
            ("argos-m001.dd1", None, None),               // 마일스톤 줄은 안 받는다
            ("argos-zzzz.ff1", None, None),               // 없는 부모는 넘지 않는다
        ] {
            assert_eq!((e.get(id).copied(), m.get(id).copied()), (epic, stone), "{id}");
        }
        // 못 쓸 참조로 드러난다 — 가리키는 에픽은 멀쩡해도 그 필드는 아무 자리도 안 정한다.
        assert_eq!(broken(&issues).get("argos-0001"), Some(&Misplace::Epic));
        assert!(
            !status(&issues, &[], &cfg(), "2026-09-01T00:00:00Z", utc())
                .warnings
                .iter()
                .any(|w| w.kind == "no_epic" && w.ids.iter().any(|i| i.starts_with("argos-0001.")))
        );
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
        // 에픽 줄이 든 `epic` — `nav` 는 에픽을 에픽 밑에 두지 않는다. 그 에픽의
        // 마일스톤도 물려받지 않는다 — 에픽 줄은 제 `milestone` 에만 선다(moai-0prl).
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
            ["argos-0002", "argos-0003", "argos-0003.aaa"],
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
            // 시작한 멤버 중 가장 앞 칸에 선다(moai-p415) — 전에는 "하는 중" 한 칸으로 접었다.
            ("todo", &["review", "review"][..], "review"),
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
            vec![mile, epic, member("argos-0002", "argos-0001", "done"), member("argos-0003", "argos-0001", "todo")]
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
            make("argos-0001", Kind::Epic, "done"), // 적힌 칸만 닫힘
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0003", Kind::Epic, "todo"), // 멤버가 다 끝남
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
        let closed =
            vec![put_off_line(make("argos-0001", Kind::Epic, "todo")), member("argos-0002", "argos-0001", "done")];
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
    ///
    /// **가려진 묶음 줄도 안 입는다**(moai-7iyc.5fz) — 칸 지도는 id 로 짠 것이라 그 id 의 뜻을
    /// 정하는 뒷줄의 칸 하나만 든다. 앞줄에도 입히면 `show -s` 가 그 줄을 뒷줄의 칸으로 고르는데
    /// 트리는 `(길 잃음)` 에 제 칸으로 그려, 한 화면이 같은 줄을 두 칸으로 말한다.
    #[test]
    fn a_work_row_never_wears_a_groupings_column() {
        let cfg = cfg();
        let read = |issues: &[Issue], at: usize| {
            let states = group_states(issues, &cfg);
            column(&Kinds::of(issues), &issues[at], &states).to_string()
        };

        // 뒷줄이 일이다 — 에픽 줄이 가려진다.
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0001", Kind::Issue, "todo"), // 같은 id 를 든 일 (머지 흔적)
        ];
        assert_eq!(read(&issues, 2), "todo", "일이 묶음의 칸을 입었다");
        assert_eq!(read(&issues, 0), "todo", "가려진 묶음 줄이 쌍둥이의 읽은 칸을 입었다");

        // 뒷줄이 에픽이다 — 에픽은 제 읽은 칸을 그대로 입고, 가려진 일 줄은 제 칸이다.
        let issues = vec![
            make("argos-0001", Kind::Issue, "todo"),
            member("argos-0002", "argos-0001", "in_progress"),
            make("argos-0001", Kind::Epic, "todo"),
        ];
        assert_eq!(read(&issues, 2), "in_progress", "묶음이 제 읽은 칸을 잃었다");
        assert_eq!(read(&issues, 0), "todo", "일이 묶음의 칸을 입었다");
    }

    /// **한 쌍둥이의 미룸이 다른 줄로 가지 않는다**(moai-u3ta).
    ///
    /// 담는 꼴이 `filter_map().collect()` 이던 동안은 "이 줄은 안 미뤄졌다" 를 적을 길이 없어,
    /// 미룬 묶음에 든 앞줄의 답이 산 묶음에 든 뒷줄에 그대로 남았다. 새는 방향이 한쪽이라
    /// 계획 밖인 줄이 계획 안으로 읽힌 적은 없고, 늘 산 줄이 사라졌다.
    #[test]
    fn a_twin_does_not_wear_the_others_deferral() {
        let off = |mut i: Issue| {
            i.deferred_at = Some("2026-09-01T00:00:00Z".into());
            i
        };
        // 축마다 한 판 — 마일스톤, 에픽, 그리고 제 줄에 적은 미룸.
        let stone = |id: &str, to: &str| {
            let mut i = make(id, Kind::Issue, "todo");
            i.milestone = Some(to.into());
            i
        };
        let mut piles = vec![
            (
                "마일스톤",
                vec![
                    stone("argos-0002", "argos-m001"),
                    stone("argos-0002", "argos-m002"),
                    off(make("argos-m001", Kind::Milestone, "todo")),
                    make("argos-m002", Kind::Milestone, "todo"),
                ],
            ),
            (
                "에픽",
                vec![
                    member("argos-0002", "argos-e001", "todo"),
                    member("argos-0002", "argos-e002", "todo"),
                    off(make("argos-e001", Kind::Epic, "todo")),
                    make("argos-e002", Kind::Epic, "todo"),
                ],
            ),
            ("제 줄", vec![off(make("argos-0002", Kind::Issue, "todo")), make("argos-0002", Kind::Issue, "todo")]),
        ];
        // 거울 판도 본다 — 뒷줄이 미룬 쪽이면 답이 뒤집힐 뿐 갈래는 같다.
        for (what, pile) in piles.clone() {
            let mut flipped = pile.clone();
            flipped.swap(0, 1);
            piles.push((what, flipped));
        }

        for (what, issues) in piles {
            let want = issues[0].is_deferred()
                || issues[0].epic.as_deref() == Some("argos-e001")
                || issues[0].milestone.as_deref() == Some("argos-m001");
            let out = shelved(&issues);
            assert_eq!(out[0].is_some(), want, "{what}: 앞줄의 답이 틀렸다 — {out:?}");
            assert_eq!(out[1].is_some(), !want, "{what}: 한 줄의 미룸이 다른 줄에 갔다 — {out:?}");

            // 내주는 문도 줄에 묻는다 — 산 줄은 그대로 `ready` 에 선다.
            let picked: Vec<&str> = ready(&issues, &cfg()).iter().map(|i| i.title.as_str()).collect();
            assert_eq!(picked.len(), 1, "{what}: 계획 안인 줄이 `ready` 에서 사라졌다 — {picked:?}");

            // 목록도 줄에 묻는다.
            let cfg = cfg();
            let wh = crate::query::Where::of(&issues, &cfg);
            assert_eq!(wh.deferred(&issues[0]), want, "{what}: 목록이 앞줄을 달리 봤다");
            assert_eq!(wh.deferred(&issues[1]), !want, "{what}: 목록이 뒷줄을 달리 봤다");

            // id 밖에 없는 표면은 **뒷줄이 답한다**(2026-09-23 사용자 결정).
            let roots = deferred_roots(&issues);
            assert_eq!(roots.contains_key("argos-0002"), !want, "{what}: 접은 지도가 앞줄의 답을 들었다");
        }
    }

    /// **무작위 더미로 같은 대조를 돌린다.** 표로 적은 모양은 누가 떠올린 것뿐이다 —
    /// `nav` 의 무작위 대조(`a_milestone_counts_what_it_draws_on_random_piles`)가 지난 회차에
    /// 세고·그리는 쪽만 옮긴 것을 잡았고, 이번에 거르고·내주고·세는 쪽이 갈린 것도 표가 아니라
    /// 이 대조가 지킬 몫이다. 씨앗을 고정해 늘 같은 더미를 만든다.
    ///
    /// 재는 것은 하나다 — **줄마다의 답([`shelved`])과 그 줄을 내주거나 숨기거나 세는 자가
    /// 같은 말을 하는가.** 묶음이 선 id 는 뺀다: 묶음이 계획 밖인가에는 읽은 칸이 걸리고
    /// ([`settled_groups`]) 그 칸은 멤버를 id 로 세어 나온 값이라 줄로 되물을 수 없다
    /// ([`torn_ids`], 2026-09-23 사용자 결정).
    #[test]
    fn every_surface_reads_the_same_line_on_random_piles() {
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut roll = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let cfg = cfg();
        for _ in 0..3000 {
            let mut issues: Vec<Issue> = Vec::new();
            for n in 0..(2 + roll(10)) {
                let kind = [Kind::Milestone, Kind::Epic, Kind::Epic, Kind::Issue, Kind::Issue, Kind::Idea][roll(6)];
                // 중복 id 를 섞는다 — 이 갈래가 갈리는 것은 그 저장소에서뿐이다.
                let id = match roll(4) {
                    0 if !issues.is_empty() => format!("{}.a{n:02}", issues[roll(issues.len())].id),
                    1 if !issues.is_empty() => issues[roll(issues.len())].id.clone(),
                    _ => format!("argos-{n:04}"),
                };
                let status = ["todo", "in_progress", "done"][roll(3)];
                let mut i = make(&id, kind, status);
                let pick = |roll: &mut dyn FnMut(usize) -> usize| match roll(4) {
                    0 | 1 => None,
                    2 if !issues.is_empty() => Some(issues[roll(issues.len())].id.clone()),
                    _ => Some("argos-zzzz".to_string()),
                };
                i.epic = pick(&mut roll);
                i.milestone = pick(&mut roll);
                // 미룸도 섞는다 — 물려받는 걸음이 서려면 미룬 줄이 하나는 있어야 한다.
                if roll(3) == 0 {
                    i.deferred_at = Some("2026-09-01T00:00:00Z".into());
                }
                issues.push(i);
            }
            let out = shelved(&issues);
            let wh = crate::query::Where::of(&issues, &cfg);

            // 1. 내주는 문 — 계획 밖인 줄은 내주지 않는다.
            for r in ready(&issues, &cfg) {
                let at = issues.iter().position(|i| std::ptr::eq(i, r)).expect("`ready` 가 남의 줄을 냈다");
                assert!(out[at].is_none(), "계획 밖인 줄을 내줬다 — {} / {issues:#?}", r.id);
            }
            // 2. 목록·보드·탐색기가 숨기는 자 — 같은 줄에 같은 답이다. **묶음 줄만 건너뛴다**
            //    (리뷰 moai-jk2u.hr4) — 한때 묶음이 선 id 를 통째로 건너뛰었는데, 그러면 그 id
            //    를 묶음과 나눠 쓴 일 줄이 접은 지도로 답하는 것을 이 대조가 못 봤다.
            for (at, i) in issues.iter().enumerate() {
                if is_group(i) {
                    continue;
                }
                assert_eq!(
                    wh.deferred(i),
                    i.is_deferred() || out[at].is_some(),
                    "목록과 줄마다의 답이 갈렸다 — {} / {issues:#?}",
                    i.id
                );
            }
            // 3. `moai status` 가 비추는 수를 내는 자가 **줄마다의 답을 짚는가.** 접은 지도를
            //    짚으면 여기서 갈린다. 이 수가 가리키는 `moai show --deferred` 와 글자째
            //    견주지는 못한다 — 그쪽은 끝난 줄의 제 미룸도 내고(`Where::deferred` 의 첫 줄)
            //    묶음 줄은 접은 지도로 답한다. 일 줄끼리의 대조는 위 2 가 한다.
            let said = status(&issues, &[], &cfg, "2026-10-01T00:00:00Z", utc());
            let count = said.notices.iter().find(|w| w.kind == "deferred").map_or(0, |w| w.count);
            assert_eq!(count, out.iter().filter(|r| r.is_some()).count(), "비추는 수가 줄마다의 답과 갈렸다");
        }
    }

    /// **도로 집는 말은 풀어야 할 미룸을 다 댄다**(moai-g2a1). 제 줄도 미뤘고 미룬 에픽에도
    /// 든 줄에 가까운 하나만 대면, 그것을 풀고도 여전히 빠진 채 그제야 다음을 댄다.
    #[test]
    fn every_deferral_that_keeps_a_row_out_is_named() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut both = member("argos-0002", "argos-0001", "todo");
        both.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let only = member("argos-0003", "argos-0001", "todo");
        let issues = vec![epic, both, only];
        let sources = deferred_sources(&issues);
        assert_eq!(sources["argos-0002"], ["argos-0002", "argos-0001"], "가까운 것부터 다 댄다");
        assert_eq!(sources["argos-0003"], ["argos-0001"]);
        assert_eq!(sources["argos-0001"], ["argos-0001"]);
        assert_eq!(
            deferred_roots(&issues).keys().collect::<Vec<_>>(),
            sources.keys().collect::<Vec<_>>(),
            "키가 어긋났다"
        );
        assert_eq!(deferred_roots(&issues)["argos-0002"], "argos-0002", "어디 밑인지는 그대로 가까운 하나다");

        // 자식과 부모가 같은 에픽에 들면 걸음이 그 에픽을 두 번 짚는다 — 한 번만 댄다.
        let mut nested = issues.clone();
        nested.push(make("argos-0003.1", Kind::Issue, "todo"));
        assert_eq!(deferred_sources(&nested)["argos-0003.1"], ["argos-0001"], "같은 미룸을 두 번 댔다");

        // 미룬 마일스톤이 done 으로 읽혀도 멤버 없는 에픽은 그 밑에서 계획 밖이다 — 비우지 않는다.
        let mut mile = make("argos-000m", Kind::Milestone, "todo");
        mile.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut finished = make("argos-000d", Kind::Issue, "done");
        finished.milestone = Some("argos-000m".into());
        let mut empty = make("argos-000e", Kind::Epic, "todo");
        empty.milestone = Some("argos-000m".into());
        let under_done = vec![mile, finished, empty];
        assert_eq!(deferred_roots(&under_done)["argos-000e"], "argos-000m");
        assert_eq!(deferred_sources(&under_done)["argos-000e"], ["argos-000m"], "도로 집을 곳이 비었다");

        // done 으로 읽은 미룬 마일스톤도 댄다 — 미룬 에픽 E 를 풀면 그 밑 X 는 M 을 뿌리로 받아
        // 여전히 빠진다. 거르면 E 를 풀고서야 M 을 댄다.
        let mut m = make("argos-00mm", Kind::Milestone, "todo");
        m.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut e = make("argos-00ee", Kind::Epic, "todo");
        e.deferred_at = Some("2026-09-01T00:00:00Z".into());
        e.milestone = Some("argos-00mm".into());
        let mut d = make("argos-00dd", Kind::Issue, "done");
        d.milestone = Some("argos-00mm".into());
        let two = vec![m, e, member("argos-00xx", "argos-00ee", "todo"), d];
        assert_eq!(
            deferred_sources(&two)["argos-00xx"],
            ["argos-00ee", "argos-00mm"],
            "done 으로 읽은 묶음의 미룸을 뺐다"
        );
    }

    /// **막대의 분모가 안 줄어드는 까닭을 셀 수 있다**(moai-zxwj, 2026-09-21 사용자 결정).
    ///
    /// 막대는 미룬 멤버를 분모에 그대로 두고(`Roll::total`), 칸은 그것을 뺀다(`counted`). 그
    /// 차이가 [`Stand::deferred`] 고, 화면이 `4/5  미룬 1` 로 대는 수다 — 이 수가 없으면 읽는
    /// 쪽은 남은 하나를 아무도 안 집는 까닭을 보드에서 알 길이 없다.
    ///
    /// **묶음 제 미룸으로는 안 센다.** [`counted`] 가 그런 멤버를 안 빼므로 0 이고, 그 줄은
    /// `put_off` 가 따로 말한다 — 세면 미뤄 둔 에픽마다 `0/2  미룬 2` 가 서서 제 미룸을
    /// 멤버의 미룸인 양 두 번 말한다.
    #[test]
    fn a_groups_deferred_members_are_counted_beside_the_bar() {
        let cfg = cfg();
        let mut shelved = member("argos-0003", "argos-0001", "todo");
        shelved.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "done"), shelved];
        let stands = group_stands(&issues, &cfg);
        assert_eq!(stands["argos-0001"].deferred, 1, "미뤄 뺀 멤버를 안 셌다");
        // 막대는 그 멤버를 분모에 그대로 둔다 — 둘이 갈리는 것이 이 수가 서는 까닭이다.
        let rolls = rollup(&issues, &cfg);
        let roll = roll_of(&rolls, Some("argos-0001"));
        assert_eq!((roll.done, roll.total), (1, 2), "막대가 미룬 멤버를 분모에서 뺐다");
        assert_eq!(stands["argos-0001"].column, crate::config::DONE, "칸은 미룬 멤버를 안 센다");

        // 미룬 멤버가 없으면 0 이다 — 꼬리가 모든 줄에 붙으면 뜻이 사라진다.
        let plain = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "done")];
        assert_eq!(group_stands(&plain, &cfg)["argos-0001"].deferred, 0);

        // 묶음 제가 미뤄 멤버가 물려받은 것은 안 센다 — 그 줄은 `put_off` 가 말한다.
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let inherited = vec![epic.clone(), member("argos-0002", "argos-0001", "todo")];
        assert_eq!(group_stands(&inherited, &cfg)["argos-0001"].deferred, 0, "제 미룸을 멤버의 것인 양 셌다");

        // **제 줄도 미뤘는데 멤버 하나가 따로 또 미뤄진 판**(리뷰). `counted` 는 묶음 제 미룸으로
        // 빠진 멤버만 도로 세므로 그 차가 1 인데, 둘 다 계획 밖이다 — 그대로 내면 화면이
        // `0/2  미룬 1  미룸` 으로 "하나는 아직 집을 것" 이라고 거짓을 말한다.
        let mut twice = member("argos-0003", "argos-0001", "todo");
        twice.deferred_at = Some("2026-09-02T00:00:00Z".into());
        let both = vec![epic, member("argos-0002", "argos-0001", "todo"), twice];
        assert_eq!(
            group_stands(&both, &cfg)["argos-0001"].deferred,
            0,
            "통째로 계획 밖인 묶음이 멤버 하나만 미뤘다고 말한다"
        );
    }

    /// `ready` 가 미룬 막음에 대는 도로 집는 말도 풀어야 할 미룸을 다 댄다(moai-g2a1).
    #[test]
    fn held_names_every_deferral_to_undo() {
        let deferred_epic = || {
            let mut e = make("argos-0001", Kind::Epic, "todo");
            e.deferred_at = Some("2026-09-01T00:00:00Z".into());
            e
        };
        let mut inner = member("argos-0002", "argos-0001", "todo");
        inner.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let waiting_on = |id: &str| {
            let mut x = make("argos-0009", Kind::Issue, "todo");
            x.blocked_by = vec![id.into()];
            x
        };

        // 막는 줄이 제 줄도 미뤘고 미룬 에픽에도 든다(moai-phzi).
        let row = vec![deferred_epic(), inner.clone(), waiting_on("argos-0002")];
        let h = held(&row, &cfg());
        assert_eq!(h[0].by, ["argos-0002"]);
        assert_eq!(h[0].undo, ["argos-0001", "argos-0002"], "가까운 하나만 댄다");

        // 제가 미뤄졌고 멤버도 다 미룬 묶음이 막는다 — 묶음만 풀면 멤버에 막힌다(moai-1c2l.8o1).
        let group = vec![deferred_epic(), inner, waiting_on("argos-0001")];
        let h = held(&group, &cfg());
        assert_eq!(h[0].by, ["argos-0001"]);
        assert_eq!(h[0].undo, ["argos-0001", "argos-0002"], "묶음만 대면 풀고도 막힌다");
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
        let shelved = shelved_in(&issues, &e, &m);
        let cfg = cfg();
        let stands = group_stands_in(&issues, &cfg, &Lines::of(&issues), &e, &m, &super::kinds(&issues), &shelved);
        assert_eq!(stands["argos-0001"].since, "2026-09-11T00:00:00Z");
        // 셀 멤버가 없으면 묶음이 생긴 때다 — 안 읽히는 칸의 시각은 안 쓴다.
        assert_eq!(stands["argos-0007"].since, "2026-09-01T00:00:00Z");
    }

    /// **묶음은 시작한 멤버 중 가장 앞 칸에 선다**(moai-p415). 칸 자리로 "시작한 칸" 을 고르면
    /// `todo,blocked,in_progress,review,done` 에서 멤버가 in_progress 인 에픽이 `blocked` 로
    /// 읽혔다. 시작한 멤버가 없으면(끝난 것과 첫 칸뿐) 설정의 첫 시작 칸이다.
    #[test]
    fn a_group_stands_in_its_least_advanced_started_members_column() {
        let cfg =
            Config::parse("prefix = \"argos\"\nstatuses = \"todo, blocked, in_progress, review, done\"\n").unwrap();
        let rows = |cols: &[&str]| {
            let mut issues = vec![make("argos-0001", Kind::Epic, "todo")];
            for (n, c) in cols.iter().enumerate() {
                issues.push(member(&format!("argos-000{}", n + 2), "argos-0001", c));
            }
            issues
        };
        let read = |cols: &[&str]| group_states(&rows(cols), &cfg)["argos-0001"].to_string();
        assert_eq!(read(&["in_progress", "todo"]), "in_progress", "시작한 칸을 자리로 골랐다");
        assert_eq!(read(&["review", "done"]), "review");
        assert_eq!(read(&["blocked", "in_progress"]), "blocked");
        assert_eq!(read(&["done", "todo"]), "blocked", "시작한 멤버가 없으면 첫 시작 칸이다");
        assert_eq!(read(&["todo", "todo"]), "todo");
        assert_eq!(read(&[]), "todo");
        assert_eq!(read(&["done", "done"]), "done");

        let working = rows(&["in_progress", "todo"]);
        assert!(group_stands(&working, &cfg)["argos-0001"].busy, "in_progress 멤버가 있는데 안 바쁘다");
        let resting = rows(&["done", "todo"]);
        assert!(!group_stands(&resting, &cfg)["argos-0001"].busy, "아무도 손대지 않았는데 바쁘다");
        // 설정에 없는 칸의 멤버는 칸 셈이 못 고르니 바쁨으로도 안 센다 — 세면 묶음이 대신 선
        // 시작 칸에서 도는데 그 밑에 도는 줄이 없다.
        let unknown = rows(&["qa", "todo"]);
        let stand = &group_stands(&unknown, &cfg)["argos-0001"];
        assert_eq!((stand.column, stand.busy), ("blocked", false), "모르는 칸 멤버로 바쁘다고 했다");
    }

    /// **읽은 칸과 "집은 멤버가 있다" 는 다른 말이다.** 끝난 멤버 하나와 첫 칸 하나로도
    /// 묶음은 시작한 칸으로 읽히지만 아무도 손대지 않는다(moai-x5eg). 두 칸짜리 설정에는
    /// 시작했다는 말이 없어 언제나 거짓이다.
    ///
    /// `review` 멤버도 시작한 것으로 센다(moai-p415) — "시작했다" 의 뜻이 `Config::is_started`
    /// 하나다. 도는 글리프도 같은 뜻을 쓰므로(moai-q59j) review 에 선 바쁜 묶음은 돈다.
    #[test]
    fn a_group_is_busy_only_with_a_member_in_the_started_column() {
        let issues = vec![
            make("argos-0001", Kind::Epic, "todo"),
            member("argos-0002", "argos-0001", "done"),
            member("argos-0003", "argos-0001", "todo"),
            make("argos-0004", Kind::Epic, "todo"),
            member("argos-0005", "argos-0004", "review"),
            make("argos-0006", Kind::Epic, "todo"),
            member("argos-0007", "argos-0006", "in_progress"),
        ];
        let cfg = cfg();
        let stands = group_stands(&issues, &cfg);
        let read = |id: &str| (stands[id].column, stands[id].busy);
        assert_eq!(read("argos-0001"), ("in_progress", false), "반쯤 끝난 에픽을 바쁘다고 한다");
        assert_eq!(read("argos-0004"), ("review", true), "review 멤버의 칸·시작을 잘못 읽었다");
        assert!(cfg.is_started(read("argos-0004").0), "review 에 선 바쁜 묶음이 시작한 칸이 아니다");
        assert_eq!(read("argos-0006"), ("in_progress", true));

        let two = Config::parse("prefix = \"argos\"\nstatuses = \"todo, done\"\n").unwrap();
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "todo")];
        assert!(!group_stands(&issues, &two)["argos-0001"].busy, "첫 칸 멤버를 집은 것으로 셌다");
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
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert_eq!(st.total, 1, "idea 를 이슈로 셌다");
        assert_eq!(st.counts.get("todo"), Some(&1), "{:?}", st.counts);
    }

    /// 에픽에 든 idea 도 진행률을 움직이지 않는다. 움직이면 생각을 담을수록
    /// 그 에픽이 덜 끝난 것으로 보인다.
    #[test]
    fn an_idea_does_not_move_an_epics_rollup() {
        let mut inside = idea("argos-0003");
        inside.epic = Some("argos-0001".into());
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "done"), inside];
        let rolls = rollup(&issues, &cfg());
        let r = roll_of(&rolls, Some("argos-0001"));
        assert_eq!((r.total, r.done, r.percent), (1, 1, Some(100)), "{r:?}");
    }

    /// **에픽 없이 사는 것이 정상이다.** 여기 걸리면 담는 족족 잔소리가 는다.
    #[test]
    fn an_idea_without_an_epic_is_not_a_warning() {
        let issues: Vec<Issue> = (0..9).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-09-02T00:00:00Z", utc());
        assert_eq!(st.flow.created, 1, "{:?}", st.flow);
    }
    /// **쌓이는 것 자체는 문제가 아니고, 쌓인 줄 모르는 것이 문제다.** 담는
    /// 비용을 0 으로 만들었으니 쌓인다 — 그래서 `status` 가 한 줄로 비춘다.
    #[test]
    fn a_pile_of_ideas_shows_up_in_status() {
        let quiet: Vec<Issue> = (0..IDEA_PILE - 1).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&quiet, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(
            !st.notices.iter().any(|w| w.kind == "idea_pile"),
            "몇 개 안 되는데 벌써 말한다 — 담을 때마다 잔소리가 는다"
        );

        let mut piled = quiet;
        piled.push(idea("argos-0009"));
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let w = st.notices.iter().find(|w| w.kind == "idea_pile").expect("쌓였는데 아무 말도 안 한다");
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
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(!st.notices.iter().any(|w| w.kind == "idea_pile"), "{:?}", st.notices);
    }
    /// **알림은 경고 수에 안 든다.** 배너가 "드러난 것 N건" 이라 말하는데
    /// 담아 둔 생각이 거기 들면, 담을수록 고칠 것이 늘었다고 말하게 된다.
    #[test]
    fn a_notice_is_not_counted_among_the_warnings() {
        let piled: Vec<Issue> = (0..IDEA_PILE).map(|n| idea(&format!("argos-000{n}"))).collect();
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
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
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), member("argos-0002", "argos-0001", "done"), inside];
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
        let st = status(&piled, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let kinds: Vec<&str> = st.notices.iter().map(|w| w.kind).collect();
        assert!(!kinds.contains(&"idea_pile"), "숨길 것을 세었다 — {kinds:?}");
        assert!(kinds.contains(&"deferred"), "제 이름으로도 안 말한다 — {kinds:?}");
    }

    /// **기한은 읽는 사람의 달로 판정한다**(moai-h2th). 같은 화면의 `생성`·`시작` 이 이미
    /// 그 사람의 시간대로 그려지므로(`view::stamp`), 판정만 UTC 로 두면 한 줄 안에서 시계 둘이
    /// 돈다 — 서울의 00시~09시 세션이 매일 하루씩 어긋난다.
    ///
    /// **나이는 안 옮긴다**(이 시험이 함께 잰다). `days_since` 가 재는 것은 흐른 시간이고
    /// 흐른 시간에는 시간대가 없다 — `now` 를 통째로 옮기면 여기 모든 나이가 같이 부푼다.
    #[test]
    fn a_deadline_is_read_in_the_reader_s_day() {
        // 서울에서는 09-12 05:00 이고 UTC 로는 아직 09-11 이다.
        let now = "2026-09-11T20:00:00Z";
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.due_on = Some("2026-09-11".into());
        // 멤버가 있어야 마일스톤이 끝난 것으로 안 읽힌다.
        // 멤버를 벌여 둔 채로 둔다 — 아래에서 칸 나이(`stale_progress`)가 안 옮겨졌는지를 이 줄로 잰다.
        let mut inside = make("argos-0002", Kind::Issue, "in_progress");
        inside.milestone = Some("argos-0001".into());
        let issues = vec![stone, inside];

        let ages = |zone: &crate::tz::Zone, kind: &str| -> Option<i64> {
            let st = status(&issues, &[], &cfg(), now, zone);
            st.warnings.iter().find(|w| w.kind == kind).and_then(|w| w.ages.get("argos-0001").copied())
        };
        assert_eq!(ages(utc(), "milestone_due_soon"), Some(0), "UTC 에서는 오늘이 기한이다");
        assert_eq!(ages(utc(), "milestone_overdue"), None);
        assert_eq!(ages(&seoul, "milestone_overdue"), Some(-1), "서울에서는 어제 지났다");
        assert_eq!(ages(&seoul, "milestone_due_soon"), None);

        // 같은 셈의 나이는 시간대를 안 탄다 — 옮긴 것은 기한 판정뿐이다.
        let age_of = |zone: &crate::tz::Zone| -> Option<i64> {
            let st = status(&issues, &[], &cfg(), now, zone);
            st.warnings.iter().find(|w| w.kind == "stale_progress").and_then(|w| w.ages.get("argos-0002").copied())
        };
        assert_eq!(age_of(utc()), Some(10), "잴 나이가 없으면 아래 줄이 헛돈다");
        assert_eq!(age_of(&seoul), age_of(utc()), "시간대가 칸 나이까지 옮겼다");
    }

    /// **셈은 시간대에 안 닿고, 판정은 나중에 접는다**(moai-fgjj, 2026-09-23 사용자 결정).
    ///
    /// [`status_in`] 은 기한을 든 줄만 [`StatusReport::dues`] 에 담고 경고로는 안 세운다 — 그 수를
    /// 캐시로 드는 쪽(탐색기의 배너·프로젝트 층)이 시간대에 매이지 않게 하려는 것이다. 접는 자는
    /// [`StatusReport::judged`] 고, **깨진 데이터보다 앞에 끼운다**: 뒤에 붙이면 보드에서 기한 줄이
    /// 치명적인 줄 밑으로 내려간다.
    #[test]
    fn a_deadline_is_counted_when_it_is_drawn_not_when_it_is_measured() {
        // 서울에서는 09-12 05:00 이고 UTC 로는 아직 09-11 이다.
        let now = "2026-09-11T20:00:00Z";
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.due_on = Some("2026-09-11".into());
        let mut inside = make("argos-0002", Kind::Issue, "todo");
        inside.milestone = Some("argos-0001".into());
        // **문턱을 걸치는 줄을 함께 둔다**(moai-ynd6). 기한이 09-15 면 UTC 의 오늘(09-11)에서는
        // 나흘 뒤라 `status_due_days`(3)를 넘어 안 세고, 서울의 오늘(09-12)에서는 사흘 뒤라 센다 —
        // **`count` 가 시간대를 안 보면 여기서 붉어진다.** 이 줄이 없으면 두 견주기가 다 1 을
        // 기다려, `Dues::split` 에서 `zone.shift(now)` 가 빠져도 `count` 가 1 을 박아도 초록이다.
        let mut edge = make("argos-0003", Kind::Milestone, "todo");
        edge.due_on = Some("2026-09-15".into());
        // 같은 id 를 두 줄로 둬 깨진 데이터 경고(`duplicate_id`)를 세운다 — 끼우는 자리를 이 줄로 잰다.
        let issues = vec![stone, inside, edge, make("argos-0002", Kind::Issue, "todo")];

        let bare = status_in(&issues, &[], &cfg(), now, &Soil::of(&issues));
        let kinds: Vec<&str> = bare.warnings.iter().map(|w| w.kind).collect();
        assert!(
            !kinds.iter().any(|k| k.starts_with("milestone_")),
            "시간대 없이 센 셈에 기한 판정이 들었다 — {kinds:?}"
        );
        assert!(kinds.contains(&"duplicate_id"), "전제가 안 섰다 — {kinds:?}");

        // 같은 줄이 시간대에 따라 다른 갈래로 선다. 다시 세지 않는다 — 판정만 다시 한다.
        //
        // **세는 자도 시간대를 본다**(moai-ynd6) — 모든 탐색기 표면이 지나는 것은 [`Dues::count`]
        // 고, 갈래를 가르는 위아래 견주기는 [`Dues::judge`] 를 지난다. 문턱을 걸치는 `argos-0003`
        // 이 서울에서만 `다가온 것` 에 들어 수가 갈린다.
        assert_eq!(bare.dues.count(now, utc()), 1, "UTC 에서는 오늘 기한 하나뿐이다");
        assert_eq!(bare.dues.count(now, &seoul), 2, "서울에서는 지난 것과 다가온 것 둘이다");
        let kind_of = |zone: &crate::tz::Zone| -> Vec<&'static str> {
            bare.dues.judge(now, zone).into_iter().map(|w| w.kind).collect()
        };
        assert_eq!(kind_of(utc()), ["milestone_due_soon"]);
        assert_eq!(kind_of(&seoul), ["milestone_overdue", "milestone_due_soon"]);
        assert_eq!(
            bare.dues.judge(now, &seoul)[0].ages.get("argos-0001").copied(),
            Some(-1),
            "판정이 나이를 안 실었다"
        );

        // 접은 뒤에는 `status` 와 한 글자도 안 다르고, 기한 줄이 깨진 데이터 **앞**에 선다.
        let folded = status_in(&issues, &[], &cfg(), now, &Soil::of(&issues)).judged(now, &seoul);
        let whole = status(&issues, &[], &cfg(), now, &seoul);
        let kinds: Vec<&str> = folded.warnings.iter().map(|w| w.kind).collect();
        assert_eq!(kinds, whole.warnings.iter().map(|w| w.kind).collect::<Vec<_>>());
        // **없는 것은 통과가 아니라 실패다**(리뷰) — `position` 을 그대로 견주면 `Option` 끼리 견주게
        // 되고 `None < Some(_)` 이라, 기한 줄이 통째로 빠진 판이 조용히 초록으로 선다.
        let at =
            |k: &str| kinds.iter().position(|x| *x == k).unwrap_or_else(|| panic!("{k} 가 목록에 없다 — {kinds:?}"));
        assert!(at("milestone_overdue") < at("duplicate_id"), "기한 줄이 치명적인 줄 밑으로 내려갔다 — {kinds:?}");
        // **앞의 경고보다 밑이다** — 끼우는 자리는 `Dues::at` 이 적어 둔 그 자리이지 목록의 머리가
        // 아니다. 머리에 끼우면 기한이 `no_epic`·`wip_overload` 를 제치고 보드의 첫 줄로 선다.
        assert!(at("no_epic") < at("milestone_overdue"), "기한 줄이 앞의 경고를 제치고 머리에 섰다 — {kinds:?}");
        assert!(folded.dues.count(now, &seoul) == 0, "한 번 접은 것을 두 번 접을 수 있다");
    }

    /// 미루는 길은 무엇이든 받는다. 드러내는 자리가 `is_work` 로 좁히면
    /// 미뤄 둔 에픽이 목록에서만 사라지고 아무 데서도 안 보인다 — 보이는
    /// 유일한 자리가 그 줄만 안 비추는 꼴이다.
    #[test]
    fn a_deferred_grouping_is_still_named() {
        let mut epic = make("argos-0001", Kind::Epic, "todo");
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let st = status(&[epic], &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let issues = vec![make("argos-0001", Kind::Epic, "todo"), shelved, member("argos-0003", "argos-0001", "done")];
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let issues: Vec<Issue> = (0..5).map(|n| make(&format!("argos-000{n}"), Kind::Issue, "todo")).collect();
        let before = status(&issues, &[], &cfg(), "2026-09-02T00:00:00Z", utc()).flow;
        let mut after = issues;
        for i in after.iter_mut().take(3) {
            i.deferred_at = Some("2026-09-02T00:00:00Z".into());
        }
        let after = status(&after, &[], &cfg(), "2026-09-02T00:00:00Z", utc()).flow;
        assert_eq!(before.created, 5, "{before:?}");
        assert_eq!(after.created, before.created, "미루자 만든 사실이 사라졌다 — {after:?}");
        assert_eq!(after.net, before.net, "{after:?}");
    }

    // ── 미룸은 물려받는다 ─────────────────────────────────────────────

    fn picks(issues: &[Issue]) -> Vec<&str> {
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

        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
        let st = status(&issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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

    /// **쪽지가 옆줄의 답을 물려주지 않는다**(moai-1pra). [`Handed`] 는 조상이 넘기는
    /// 소속을 한 번 도는 동안 한 번만 풀고 앞자리를 따로 쥐는데, 그 둘이 id 를 안 보면 에픽
    /// A 의 멤버가 에픽 B 의 답을 입는다 — 화면에도 `-e` 거름망에도 그대로 샌다.
    /// **줄을 일부러 섞어 넣는다** — id 순으로 오면 앞자리가 늘 맞아 빗나간 길을 안 지난다.
    #[test]
    fn the_map_hands_each_plan_member_its_own_epic() {
        let a = make("argos-0002", Kind::Epic, "todo");
        let b = make("argos-0003", Kind::Epic, "todo");
        let under_a = make("argos-0002.aaa", Kind::Issue, "todo");
        let under_b = make("argos-0003.bbb", Kind::Issue, "todo");
        // 두 단 밑 — 쪽지가 지나온 줄에 같은 답을 적는 자리다.
        let deep = make("argos-0002.aaa.ccc", Kind::Issue, "todo");
        // 제 `epic` 을 적은 줄은 id 부모를 이긴다. 쪽지가 그것을 조상에게 흘리면 안 된다.
        let mut moved = make("argos-0002.ddd", Kind::Issue, "todo");
        moved.epic = Some("argos-0003".into());
        let under_moved = make("argos-0002.ddd.eee", Kind::Issue, "todo");
        let issues = vec![deep, under_b, moved, under_a, b, under_moved, a];

        let g = groups(&issues);
        assert_eq!(g.get("argos-0002.aaa"), Some(&"argos-0002"), "{g:?}");
        assert_eq!(g.get("argos-0003.bbb"), Some(&"argos-0003"), "{g:?}");
        assert_eq!(g.get("argos-0002.aaa.ccc"), Some(&"argos-0002"), "{g:?}");
        assert_eq!(g.get("argos-0002.ddd"), Some(&"argos-0003"), "제가 적은 에픽을 잃었다 — {g:?}");
        assert_eq!(g.get("argos-0002.ddd.eee"), Some(&"argos-0003"), "부모가 적은 에픽이 안 내려왔다 — {g:?}");
    }

    /// **차는 중인 `rooted` 를 문 쪽지가 거짓을 안 남긴다**(리뷰). [`rooted_thoughts`] 는
    /// [`Handed`] 하나를 고리 내내 나눠 쓰는데, 그 답이 서는 것은 오름이 `t` 의 조상만 짚고
    /// 조상의 id 가 반드시 더 짧아 **길이 순 고리에서 앞서 정해졌기** 때문이다. 생각이 한
    /// 단뿐이면 그 자리가 안 열린다 — 두 단으로 겹쳐야 안쪽 생각이 바깥 생각의 답을 묻는다.
    ///
    /// 정렬이 무너지면 안쪽 생각이 먼저 돌아, 아직 안 끊긴 바깥 생각에서 에픽을 받아 쪽지에
    /// 적는다. 그러면 안쪽이 `folds` 로 읽혀 `rooted` 에 안 들고, 그 밑이 통째로 에픽을
    /// 물려받는다 — 마지막 줄이 그것을 잡는다.
    #[test]
    fn a_thought_folded_under_a_rooted_thought_hands_nothing_down() {
        let epic = make("argos-0002", Kind::Epic, "todo");
        // 에픽 바로 밑의 생각은 늘 뿌리로 올라간다 — 부모가 묶음이라 접힐 자리가 없다.
        let outer = make("argos-0002.aaa", Kind::Idea, "todo");
        // 그 밑의 생각이 제 `epic` 으로 같은 에픽을 적으면, 바깥 생각을 아직 안 끊긴 줄로 읽는
        // 때에만 "부모가 넘긴 것과 같다" 가 되어 접힌다.
        let mut inner = make("argos-0002.aaa.bbb", Kind::Idea, "todo");
        inner.epic = Some("argos-0002".into());
        let under = make("argos-0002.aaa.bbb.ccc", Kind::Issue, "todo");
        let issues = vec![under, inner, epic, outer];

        let g = groups(&issues);
        // 생각 제 줄의 소속은 남는다 — 끊기는 것은 그 밑이다.
        assert_eq!(g.get("argos-0002.aaa"), Some(&"argos-0002"), "{g:?}");
        assert_eq!(g.get("argos-0002.aaa.bbb"), Some(&"argos-0002"), "제가 적은 에픽을 잃었다 — {g:?}");
        assert_eq!(g.get("argos-0002.aaa.bbb.ccc"), None, "끊긴 생각 밑이 에픽을 물려받았다 — {g:?}");
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
        let st = status(&issues, &clash, &cfg(), "2026-09-01T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "duplicate_id").expect("중복을 못 봤다");
        assert_eq!(w.ids, ["argos-0001"]);
        assert!(w.fatal);

        let apart = [Unreadable { id: Some("argos-0002") }, Unreadable { id: None }];
        let st = status(&issues, &apart, &cfg(), "2026-09-01T00:00:00Z", utc());
        assert!(!st.warnings.iter().any(|w| w.kind == "duplicate_id"), "{:?}", st.warnings);
    }

    /// **먼 미래 시각을 든 줄을 드러낸다**(moai-ugjp). 나이는 0 아래로 안 내려가서(moai-fix6)
    /// 2099 같은 오타가 목록에서 "오늘" 로 숨고 흐름 셈에도 들었다. 도구는 제 시계로만 적으니
    /// 그런 시각은 손으로 고친 줄이나 크게 틀린 시계에서 온다. 사람이 정한 대로 — 경고지
    /// 깨진 데이터가 아니고(종료 코드 0), 문턱은 하루, 시각을 다 보고, 흐름에서 뺀다.
    /// **시작·끝 시각도 든다**(moai-38mh) — 시작은 안 덮여, 틀린 시계의 값이 다음 이동 뒤에도 남는다.
    #[test]
    fn a_row_stamped_far_in_the_future_is_named_and_left_out_of_the_flow() {
        let now = "2026-09-11T00:00:00Z";
        let at = |id: &str, t: &str| {
            let mut i = make(id, Kind::Issue, "todo");
            (i.created_at, i.updated_at, i.status_since) = (now.into(), now.into(), now.into());
            if !t.is_empty() {
                i.created_at = t.into();
            }
            i
        };
        let typo = at("argos-0001", "2099-09-11T00:00:00Z");
        let skewed = at("argos-0002", "2026-09-11T12:00:00Z"); // 옆 기계 시계가 반나절 빠르다 — 문턱 안
        let plain = at("argos-0003", "");
        let mut late_deferral = at("argos-0004", "");
        late_deferral.deferred_at = Some("2026-09-13T00:00:00Z".into()); // 이틀 뒤 — 시각을 다 본다
        let mut late_plan = at("argos-0005", "");
        late_plan.planned_at = Some("2027-01-01T00:00:00Z".into());
        let mut late_start = at("argos-0006", "");
        late_start.started_at = Some("2099-01-01T00:00:00Z".into());
        let mut late_finish = at("argos-0007", "");
        late_finish.done_at = Some("2099-01-01T00:00:00Z".into());
        let st =
            status(&[typo, skewed, plain, late_deferral, late_plan, late_start, late_finish], &[], &cfg(), now, utc());

        let w = st.warnings.iter().find(|w| w.kind == "future_timestamp").expect("먼 미래 시각을 안 말한다");
        assert_eq!(w.ids, ["argos-0001", "argos-0004", "argos-0005", "argos-0006", "argos-0007"], "{w:?}");
        assert!(!w.fatal && !w.notice && !st.broken(), "경고로 비영 종료한다 — {w:?}");
        // 흐름은 먼 미래 시각을 "최근" 으로 세지 않는다 — 걸린 줄은 통째로 빠진다.
        assert_eq!(st.flow.created, 2, "{:?}", st.flow);

        // 문턱 안이면 말하지 않는다.
        let ok = status(&[at("argos-0002", "2026-09-11T23:59:00Z")], &[], &cfg(), now, utc());
        assert!(!ok.warnings.iter().any(|w| w.kind == "future_timestamp"), "{:?}", ok.warnings);
    }

    /// **경고는 id 를 한 번씩만 댄다** (moai-ddtg). 같은 id 의 줄이 둘이면 줄마다 id
    /// 지도를 물어 같은 id 가 두 번 담겼다 — 사람이 고칠 손잡이는 id 하나고, 셈(`N건`)이
    /// 줄 수로 부풀면 `moai show -e none` 같은 힌트가 내는 것과도 어긋난다. 세 줄 중복도
    /// `duplicate_id` 에 그 id 를 두 번 대지 않는다.
    #[test]
    fn a_warning_names_a_duplicated_id_once() {
        let mut thought = make("argos-0000", Kind::Idea, "todo");
        thought.milestone = Some("argos-zzzz".into());
        let mut stone = make("argos-0000", Kind::Milestone, "todo");
        stone.milestone = Some("argos-zzzz".into());
        let st = status(&[thought, stone], &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "dangling_milestone").expect("경고가 없다");
        assert_eq!((w.ids.as_slice(), w.count), (&["argos-0000".to_string()][..], 1), "{w:?}");

        let mut orphan = make("argos-0009.aaa", Kind::Issue, "todo");
        orphan.blocked_by = vec!["argos-gone".into()];
        let thrice = vec![orphan.clone(), orphan.clone(), orphan];
        let st = status(&thrice, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        for kind in ["orphan_child", "dangling_blocked_by", "duplicate_id"] {
            let w = st.warnings.iter().find(|w| w.kind == kind).unwrap_or_else(|| panic!("{kind} 가 없다"));
            assert_eq!((w.ids.as_slice(), w.count), (&["argos-0009.aaa".to_string()][..], 1), "{w:?}");
        }

        // **문턱도 id 로 잰다.** 벌인 일 셋에 그중 하나의 쌍둥이 줄 — 줄로 재면 `WIP_LIMIT`
        // 를 넘겨 `3건` 을 말하는 `wip_overload` 가 선다. 소속 없는 일도 같은 자로 잰다.
        let held: Vec<Issue> = ["argos-0101", "argos-0102", "argos-0103", "argos-0103"]
            .iter()
            .map(|id| make(id, Kind::Issue, "in_progress"))
            .collect();
        let st = status(&held, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(!st.warnings.iter().any(|w| w.kind == "wip_overload"), "{:?}", st.warnings);
        let loose: Vec<Issue> = ["argos-0201", "argos-0202", "argos-0203", "argos-0204", "argos-0204"]
            .iter()
            .map(|id| make(id, Kind::Issue, "todo"))
            .chain((0..30).map(|n| member(&format!("argos-1{n:03}"), "argos-e001", "todo")))
            .chain([make("argos-e001", Kind::Epic, "todo")])
            .collect();
        let st = status(&loose, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        assert!(!st.warnings.iter().any(|w| w.kind == "no_epic"), "{:?}", st.warnings);
    }

    /// **가려진 줄은 쌍둥이의 소속을 달지도, 쌍둥이의 소속으로 세지도 않는다** (moai-b5lu).
    /// 종류가 다른 쌍둥이에게 id 가 가려진 줄([`eclipsed`])은 트리에서 `(길 잃음)`,
    /// 롤업·`-e`/`--milestone` 거름망에서 어느 묶음에도 안 든다. 그런데 에픽 칸은 id 로
    /// 찾아 그 줄에 쌍둥이 에픽의 제목을 냈고, `no_epic`·`no_milestone` 은 쌍둥이의 지도
    /// 값으로 세어 경고에 선 줄을 힌트(`moai show -e none`)가 안 냈다.
    #[test]
    fn an_eclipsed_row_borrows_no_membership_from_its_twin() {
        let epic = make("argos-e001", Kind::Epic, "todo");
        // 앞줄 이슈는 에픽이 없고, 뒷줄 생각이 그 에픽에 든다 — 에픽 칸이 흐르는 쪽.
        let bare = make("argos-0001", Kind::Issue, "todo");
        let mut held = make("argos-0001", Kind::Idea, "todo");
        held.epic = Some("argos-e001".into());
        let rows = [epic, bare, held];
        let labels = epic_labels(&rows);
        assert_eq!(labels.at(&rows[2]), Some(EpicLabel::Named("argos-e001 제목")));
        assert_eq!(labels.at(&rows[1]), None, "가려진 줄이 쌍둥이 에픽을 달았다");

        // 앞줄 이슈는 에픽·마일스톤이 있고 뒷줄 생각은 없다 — 경고가 쌍둥이 값으로 세는 쪽.
        let stone = make("argos-m001", Kind::Milestone, "todo");
        let mut placed = make("argos-e002", Kind::Epic, "todo");
        placed.milestone = Some("argos-m001".into());
        let mut member = make("argos-0002", Kind::Issue, "todo");
        member.epic = Some("argos-e002".into());
        let mut shadowed = make("argos-0003", Kind::Issue, "todo");
        shadowed.epic = Some("argos-e002".into());
        let loose_thought = make("argos-0003", Kind::Idea, "todo");
        let rows = vec![stone, placed, member, shadowed, loose_thought];
        let st = status(&rows, &[], &cfg(), "2026-09-11T00:00:00Z", utc());
        for kind in ["no_epic", "no_milestone"] {
            let named =
                st.warnings.iter().filter(|w| w.kind == kind).flat_map(|w| w.ids.iter()).any(|id| id == "argos-0003");
            assert!(!named, "{kind} 가 가려진 줄을 쌍둥이 값으로 셌다 — {:?}", st.warnings);
        }
    }

    /// **가려진 줄은 `derived_epic` 도 안 단다** (moai-53s2, 2026-09-23 사용자 결정).
    /// 소속 지도는 id 로 짠 것이라 가려진 줄에 쌍둥이의 값을 주는데, 트리는 그 줄을
    /// `(길 잃음)` 에 두고 `-e` 도 `-e none` 도 안 고르며 롤업은 어느 묶음에도 안 센다 —
    /// `show --json` 목록만 그 줄에 쌍둥이의 에픽을 달아, 한 바이너리가 한 물음에 표면마다
    /// 다른 답을 했다. 문은 [`stands_in`] 하나다: 표면을 하나씩 고치면 가려진 줄을 싣는
    /// 다음 표면이 같은 구멍을 다시 연다.
    #[test]
    fn an_eclipsed_row_is_given_no_derived_epic() {
        let epic = make("argos-e001", Kind::Epic, "todo");
        let bare = make("argos-0001", Kind::Issue, "todo");
        let mut held = make("argos-0001", Kind::Idea, "todo");
        held.epic = Some("argos-e001".into());
        // 제 `epic` 을 적은 가려진 줄 — `query` 가 그 값으로도 안 고르므로 여기서도 안 낸다.
        let mut wrote = make("argos-0002", Kind::Issue, "todo");
        wrote.epic = Some("argos-e001".into());
        let plain = make("argos-0002", Kind::Idea, "todo");
        let rows = [epic, bare, held, wrote, plain];
        let kind_of = Kinds::of(&rows);
        let placed = groups(&rows);
        let of = |i: &Issue| stands_in(&kind_of, i, placed.get(i.id.as_str()).copied()).map(str::to_string);

        assert_eq!(of(&rows[1]), None, "가려진 줄이 쌍둥이의 에픽을 달았다");
        assert_eq!(of(&rows[2]), Some("argos-e001".into()), "가리는 줄의 소속까지 지웠다");
        assert_eq!(of(&rows[3]), None, "제 epic 을 적은 가려진 줄이 값을 냈다");
        assert_eq!(of(&rows[4]), None, "생각이 뿌리로 올라간 자리가 달라졌다");

        // **좁힌 지도도 같은 답이다**([`Kinds::of_ids`]) — `ready --json` 과 `status --json` 이 쓴다.
        let narrow = Kinds::of_ids(&rows, &["argos-0001"]);
        assert!(narrow.eclipses(&rows[1]), "좁힌 지도가 가려짐을 놓쳤다");
        assert_eq!(stands_in(&narrow, &rows[1], placed.get("argos-0001").copied()), None);

        // **빌린 지도도 같은 답이다**([`Kinds::kept`]) — 탐색기가 든 꼴이다.
        let owned: BTreeMap<String, Kind> = rows.iter().map(|i| (i.id.clone(), i.kind)).collect();
        assert!(Kinds::kept(&owned).eclipses(&rows[1]), "빌린 지도가 가려짐을 놓쳤다");

        // [`Kinds::no_twins`] 는 "쌍둥이가 못 선다" 는 **주장**이다 — 쓰기 경로와 `show <id>`
        // 가 그 까닭을 제자리에 적는다. 주장이 틀린 자리에서는 이렇게 문이 열린 채로 선다.
        assert_eq!(stands_in(&Kinds::no_twins(), &rows[1], Some("argos-e001")), Some("argos-e001"));
    }

    /// **가려진 줄은 집을 일이 아니다** (moai-lg2t). 트리에서 `(길 잃음)` 에 서는 줄을
    /// `ready` 가 내밀면 에픽 칸에 `에픽 없음` 을 달고, 집으려 하면 파일째 쓰기가 막힌다.
    /// 막혀 있어도 `held` 가 그 줄을 도로 집을 일로 대지 않는다 — 둘은 한 자로 고른다.
    #[test]
    fn an_eclipsed_row_is_not_offered_as_work() {
        let mut shadowed = make("argos-0001", Kind::Issue, "todo");
        shadowed.blocked_by = vec!["argos-0009".into()];
        let rows = [
            deferred("argos-0009", "todo"),
            make("argos-0002", Kind::Issue, "todo"),
            shadowed,
            make("argos-0001", Kind::Idea, "todo"),
            make("argos-0002", Kind::Epic, "todo"),
        ];
        let rows = &rows[..];
        let ids: Vec<&str> = ready(rows, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert!(!ids.contains(&"argos-0001") && !ids.contains(&"argos-0002"), "가려진 줄을 냈다 — {ids:?}");
        assert!(held(rows, &cfg()).is_empty(), "가려진 줄을 막힌 일로 댔다");

        // 같은 종류의 쌍둥이는 가려지지 않는다 — 같은 값을 같은 자로 읽는다.
        let twins = vec![make("argos-0003", Kind::Issue, "todo"), make("argos-0003", Kind::Issue, "todo")];
        assert_eq!(ready(&twins, &cfg()).len(), 2);
    }

    /// **가려진 줄은 집은 일도 아니다**(moai-es40, 사용자 결정). `ready`·`held` 가 거르는 줄을 `wip` 만
    /// 세면 보드의 벌여 놓은 셈·`ready` 아래 줄·훅 초점이 `(길 잃음)` 에 선 줄을 집은 일로 댄다.
    #[test]
    fn an_eclipsed_row_is_not_counted_as_picked() {
        let rows = vec![
            make("argos-0001", Kind::Issue, "in_progress"),
            make("argos-0001", Kind::Epic, "todo"),
            make("argos-0002", Kind::Issue, "in_progress"),
        ];
        let ids: Vec<&str> = wip(&rows, &cfg()).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["argos-0002"], "가려진 줄을 집은 일로 셌다");

        // 같은 종류의 쌍둥이는 가려지지 않는다 — 둘 다 집은 일이다.
        let twins =
            vec![make("argos-0003", Kind::Issue, "in_progress"), make("argos-0003", Kind::Issue, "in_progress")];
        assert_eq!(wip(&twins, &cfg()).len(), 2);
    }

    /// **`status` 의 벌여 놓은 셈도 같은 자다**(에픽 끝 리뷰 moai-r8gw.b4s) — `wip_overload`·`stale_progress`
    /// 가 제 손으로 세면, 한눈 보기가 `wip` 으로 집은 것 셋을 대는 보드에 `4건` 이 서고 `ready` 아래 줄에
    /// 없는 id 를 잊은 것으로 꾸짖는다.
    #[test]
    fn an_eclipsed_row_is_neither_overload_nor_forgotten() {
        let mut rows: Vec<Issue> = ["argos-0001", "argos-0002", "argos-0003", "argos-0004"]
            .iter()
            .map(|id| make(id, Kind::Issue, "in_progress"))
            .collect();
        rows.push(make("argos-0004", Kind::Epic, "todo"));
        let st = status(&rows, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
        assert!(
            !st.warnings.iter().any(|w| w.kind == "wip_overload"),
            "가려진 줄을 벌인 일로 셌다 — {:?}",
            st.warnings
        );
        let forgotten = st.warnings.iter().find(|w| w.kind == "stale_progress").expect("잊은 것 경고가 없다");
        let picked: Vec<String> = wip(&rows, &cfg()).iter().map(|i| i.id.clone()).collect();
        assert_eq!(forgotten.ids, picked, "잊은 것과 집은 것이 다른 자로 셌다");
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
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
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

    /// **남은 멤버를 미뤄 접은 묶음도 미룬 일에 막혀 있다**(moai-0gxf). 묶음의 칸은
    /// 미룬 멤버를 빼고 읽어 `done` 으로 서지만, 그것에 막힌 일을 풀면 같은 미룬 멤버에
    /// 곧장 막힌 줄은 held 로 서는데 묶음 너머로 막힌 줄만 `ready` 에 선다. 댈 곳은
    /// 묶음이 아니라 뺀 멤버다 — 묶음에 `--undo` 를 쳐 봐야 미룬 것이 없다.
    #[test]
    fn a_group_folded_by_deferring_its_rest_still_holds() {
        let epic = make("argos-0001", Kind::Epic, "todo");
        let mut rest = member("argos-0003", "argos-0001", "todo");
        rest.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut through = make("argos-0004", Kind::Issue, "todo");
        through.blocked_by = vec!["argos-0001".into()];
        let mut direct = make("argos-0005", Kind::Issue, "todo");
        direct.blocked_by = vec!["argos-0003".into()];
        let issues = vec![epic, member("argos-0002", "argos-0001", "done"), rest, through, direct];
        assert_eq!(group_states(&issues, &cfg()).get("argos-0001"), Some(&"done"), "칸 셈은 그대로다");
        assert!(picks(&issues).is_empty(), "미룬 멤버 너머로 막힌 줄을 집으라고 내민다 — {:?}", picks(&issues));

        let held = held(&issues, &cfg());
        let named: Vec<(&str, &[&str], &[&str])> =
            held.iter().map(|h| (h.issue.id.as_str(), h.by.as_slice(), h.undo.as_slice())).collect();
        assert_eq!(
            named,
            [
                ("argos-0004", &["argos-0003"][..], &["argos-0003"][..]),
                ("argos-0005", &["argos-0003"][..], &["argos-0003"][..])
            ]
        );
        let st = status(&issues, &[], &cfg(), "2026-10-01T00:00:00Z", utc());
        let w = st.warnings.iter().find(|w| w.kind == "blocked_by_deferred").expect("미룬 것에 막혔다고 안 한다");
        assert_eq!(w.ids, ["argos-0004", "argos-0005"]);

        // 뺀 멤버를 도로 집으면 묶음은 제 칸으로 막는다 — 미룸 말은 사라진다.
        let mut back = issues.clone();
        back[2].deferred_at = None;
        assert!(held_of(&back).is_empty());
        assert_eq!(picks(&back), ["argos-0003"], "막힌 둘은 안 풀리고, 도로 집은 멤버만 선다");
        // 뺀 멤버가 끝나면 풀린다.
        back[2].status = Status::new("done");
        assert_eq!(picks(&back), ["argos-0004", "argos-0005"]);
    }

    /// **기다릴 일이 없는 묶음이 막아도 까닭을 댄다**(moai-1c2l). 끝난 멤버 없이 전부
    /// 미룬 묶음은 첫 칸이라 막는데, 그 칸은 미룬 멤버만 기다린다 — 접은 묶음(moai-0gxf)과
    /// 같은 말을 한다. 멤버가 하나도 없는 묶음은 영영 안 풀리니 비었다고 댄다. 둘 다
    /// 막음은 풀지 않는다 — 미룬 일은 끝난 일이 아니고, 빈 에픽은 채울 자리다.
    #[test]
    fn a_group_with_nothing_live_to_wait_on_says_why_it_blocks() {
        let blocked_by = |id: &str| {
            let mut x = make("argos-0009", Kind::Issue, "todo");
            x.blocked_by = vec![id.into()];
            x
        };
        type Said = (String, Vec<String>, Vec<String>, Vec<String>);
        let said = |issues: &[Issue]| -> Vec<Said> {
            let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
            held(issues, &cfg()).iter().map(|h| (h.issue.id.clone(), s(&h.by), s(&h.undo), s(&h.empty))).collect()
        };
        let row = |by: &[&str], undo: &[&str], empty: &[&str]| -> Said {
            let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
            ("argos-0009".into(), s(by), s(undo), s(empty))
        };

        // 끝난 멤버 없이 전부 미뤘다 — 칸은 첫 칸이지만 기다리는 것은 미룬 멤버뿐이다.
        let mut rest = member("argos-0002", "argos-0001", "todo");
        rest.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let shelved = vec![make("argos-0001", Kind::Epic, "todo"), rest, blocked_by("argos-0001")];
        assert!(picks(&shelved).is_empty());
        assert_eq!(said(&shelved), [row(&["argos-0002"], &["argos-0002"], &[])]);
        // 멤버를 다 미룬 묶음을 제가 또 미뤘으면 묶음을 댄다 — 멤버를 도로 집어도 안 풀린다.
        // 도로 집을 곳은 둘 다다: 묶음만 풀면 이번엔 미룬 멤버에 막힌다(moai-g2a1).
        let mut both = shelved.clone();
        both[0].deferred_at = Some("2026-09-02T00:00:00Z".into());
        assert_eq!(said(&both), [row(&["argos-0001"], &["argos-0001", "argos-0002"], &[])]);
        assert_eq!(blocker(Some("todo"), true, Waiting::Shelved), Blocker::Deferred);

        // 멤버가 하나도 없다 — 도로 집을 것이 없으니 비었다고 댄다.
        let empty = vec![make("argos-0001", Kind::Epic, "todo"), blocked_by("argos-0001")];
        assert!(picks(&empty).is_empty(), "빈 에픽이 막음을 풀었다");
        assert_eq!(said(&empty), [row(&[], &[], &["argos-0001"])]);
        // 빈 마일스톤도 같다.
        let stone = vec![make("argos-0001", Kind::Milestone, "todo"), blocked_by("argos-0001")];
        assert_eq!(said(&stone), [row(&[], &[], &["argos-0001"])]);

        // 빈 묶음을 제가 미뤘으면 그 미룸이 먼저다 — 도로 집는 말이 풀 길이다.
        let mut put_off = make("argos-0001", Kind::Epic, "todo");
        put_off.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let own = vec![put_off, blocked_by("argos-0001")];
        assert_eq!(said(&own), [row(&["argos-0001"], &["argos-0001"], &[])]);

        // 멤버를 채우면 보통 막음이다 — 까닭을 따로 대지 않는다.
        let mut filled = empty.clone();
        filled.push(member("argos-0003", "argos-0001", "todo"));
        assert!(said(&filled).is_empty(), "{:?}", said(&filled));
        assert_eq!(picks(&filled), ["argos-0003"]);

        // 막음 셈은 탐색기와 한 자리다.
        assert_eq!(blocker(Some("todo"), false, Waiting::Nothing), Blocker::Empty);
        assert_eq!(blocker(Some("todo"), true, Waiting::Nothing), Blocker::Deferred);
        assert!(Blocker::Empty.blocks());
    }

    fn held_of(issues: &[Issue]) -> Vec<&str> {
        held(issues, &cfg()).iter().map(|h| h.issue.id.as_str()).collect()
    }

    /// 미룬 마일스톤 밑의 빈 에픽은 `미뤄 둔 것` 으로 세면서 `속이 빈 에픽` 으로
    /// 꾸짖지 않는다. 미룰수록 잔소리가 느는 실패가 물려받은 자리에서 돌아온다.
    #[test]
    fn an_epic_under_a_deferred_milestone_is_not_scolded() {
        let mut stone = make("argos-0001", Kind::Milestone, "todo");
        stone.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let st = status(&[stone, epic], &[], &cfg(), "2026-09-11T00:00:00Z", utc());
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
            status(issues, &[], &cfg(), "2026-09-11T00:00:00Z", utc()).warnings.iter().any(|w| w.kind == "no_epic")
        };
        assert!(!no_epic(&issues), "미루기 전부터 경고가 섰다 — 시험이 헛돈다");
        issues[0].deferred_at = Some("2026-09-01T00:00:00Z".into());
        assert!(!no_epic(&issues), "미루기 하나로 `에픽 없음` 경고가 섰다");
    }
}
