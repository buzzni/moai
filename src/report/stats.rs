//! 통계 — 거르개가 고른 줄을 센다(moai-1hka.f4x). **순수 함수다** — 줄과, 부르는 쪽이 노트에서 읽어 건넨
//! `work`(`model::work_of`)만 본다. 아무것도 찍지 않고 파일도 열지 않는다.
//!
//! **CLI(`moai stats`)와 탐색기가 같은 값을 읽는다.** 둘이 따로 세면 같은 저장소가 두 수로 읽힌다. 그래서
//! 고르는 자([`select`])와 세는 자([`of`])를 여기 한 자리에 둔다 — 표면은 둘을 부르고 그리기만 한다.
//!
//! **저널의 칸 옮김은 접지 않는다**(CLAUDE.md "저널은 상태 계산에 읽히지 않는다"). 읽는 것은 스냅샷의 필드·
//! 시각과, 노트에서 읽은 `model:` 줄뿐이다. review 칸에 머문 시간이나 다시 연 횟수처럼 칸 옮김을 접어야 나오는
//! 값은 싣지 않는다 — 그런 값이 필요해지면 그 값은 스냅샷의 필드가 되어야 한다. 노트를 받는 것은 `-g` 가
//! 노트를 찾는 것과 같은 길이다: 글을 읽을 뿐, 상태를 계산하지 않는다.
//!
//! **저장하지 않는다.** 여기 나오는 수는 다 읽을 때 센 것이다.
//!
//! # 무엇을 세나
//!
//! - **고르는 것은 `show` 의 거르개다**([`select`]) — done·미룸·생각까지 연 채로 거른다. 지나간 일을 세는
//!   자리라 끝난 줄을 숨길 까닭이 없다. `--since`·`--created`·`--done` 이 숨김을 다 여는 것과 같은 결이다
//! - **세는 것은 한 종류다**([`counted_kind`]) — `--type` 이 없으면 일(`issue`, [`super::is_work`])이다. 보드의
//!   `todo 6` 에 묶음을 섞지 않는 것과 같은 까닭이고, 묶음의 기간은 멤버로 잰다(`-e`·`--milestone`). 생각을
//!   세려면 `--type idea` 다
//! - **`kind` 축만 고른 줄 전부를 센다** — 센 종류 하나로 재면 늘 한 칸짜리라, 그 축이 대는 것은 "거르개가 무엇을
//!   골랐고 무엇을 안 셌나" 다
//! - **칸·소속은 다른 표면이 읽는 그 자로 읽는다**([`Where::column`]·[`Where::epic_of`]·[`Where::milestone_of`]) —
//!   목록의 `derived_status`·`derived_epic` 과 같은 답이다
//! - **닫힌 때는 `--done` 이 읽는 그 값이다**([`closed_at`]) — 지금 done 에 선 줄이 그 칸에 든 때. 되돌린 줄의
//!   `done_at` 은 남아 있어도 지금 닫힌 줄이 아니다
//! - **모르는 것은 0 이 아니다** — `started_at` 이 없는 줄은 `unknown` 으로 따로 세고(`Spent` 와 같은 금),
//!   `tokens=` 가 없는 `model:` 줄은 토큰 합에 안 든다. 하나도 없으면 합은 `None` 이다

// 읽는 표면(`moai stats`, moai-1hka.k16)이 다음 커밋에 선다 — 그때 걷는다.
#![allow(dead_code)]

use crate::config::{Config, DONE};
use crate::model::{Issue, Kind, MAX_PRIORITY, Work, parse_rfc3339};
use crate::query::{Filter, Where};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// 분포를 내는 축. 이름은 `--by` 의 낱말이자 `--json` 의 `by` 아래 키다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Status,
    Kind,
    Priority,
    Tag,
    Assignee,
    Epic,
    Milestone,
}

impl Axis {
    /// `--by` 를 안 줬을 때 내는 차례.
    pub const ALL: [Axis; 7] =
        [Axis::Status, Axis::Kind, Axis::Priority, Axis::Tag, Axis::Assignee, Axis::Epic, Axis::Milestone];

    pub fn name(self) -> &'static str {
        match self {
            Axis::Status => "status",
            Axis::Kind => "kind",
            Axis::Priority => "priority",
            Axis::Tag => "tag",
            Axis::Assignee => "assignee",
            Axis::Epic => "epic",
            Axis::Milestone => "milestone",
        }
    }
}

/// 흐름의 칸 하나가 덮는 폭.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bucket {
    Day,
    /// ISO 주 — 월요일부터다.
    #[default]
    Week,
}

impl Bucket {
    /// 몇 칸을 거슬러 갈지 안 정했을 때의 수 — 두 주와 두 달 남짓이다.
    pub fn default_last(self) -> usize {
        match self {
            Bucket::Day => 14,
            Bucket::Week => 8,
        }
    }

    fn days(self) -> i64 {
        match self {
            Bucket::Day => 1,
            Bucket::Week => 7,
        }
    }

    /// 그날(벽시계의 epoch 일)이 든 칸의 첫날. 1970-01-01 이 목요일이라 월요일은 `+3` 을 7 로 나눈 나머지만큼
    /// 앞이다.
    fn start(self, day: i64) -> i64 {
        match self {
            Bucket::Day => day,
            Bucket::Week => day - (day + 3).rem_euclid(7),
        }
    }
}

/// 무엇을 셀지.
#[derive(Debug, Clone, Default)]
pub struct Ask {
    /// 셀 종류. 없으면 일이다([`counted_kind`]).
    pub kind: Option<Kind>,
    /// 낼 분포. **비면 일곱 다**([`Axis::ALL`]). 같은 축을 두 번 대도 한 번만 낸다.
    pub by: Vec<Axis>,
    pub bucket: Bucket,
    /// 흐름이 몇 칸을 거슬러 가나 — 지금 든 칸까지 친다. 0 은 한 칸으로 친다.
    pub last: usize,
}

/// 센 것 전부. **CLI 의 `--json` 이 이 모양 그대로다** — 키를 바꾸면 그 계약이 바뀐다(`moai stats --help`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    /// 아래 모든 수가 센 종류.
    pub kind: Kind,
    /// 거르개가 고른 줄 가운데 그 종류의 수 — 아래 수의 바탕이다.
    pub rows: usize,
    /// 축별 분포 — 물은 축만, 차례대로. `kind` 축만 고른 줄 전부를 센다(머리글).
    #[serde(serialize_with = "as_map")]
    pub by: Vec<(Axis, Vec<Count>)>,
    pub flow: Flow,
    /// 만든 때부터 닫힌 때까지.
    pub lead_time: Spans,
    /// 처음 첫 칸을 떠난 때(`started_at`)부터 닫힌 때까지.
    pub cycle_time: Spans,
    /// 노트의 `model:` 줄.
    pub work: Spend,
    /// `review` 태그를 단 줄 — 리뷰 이슈다(`guide::REVIEW_TAG`).
    pub reviews: Reviews,
}

/// `by` 를 **축 이름 → 분포** 의 객체로 낸다. 축 차례는 물은 차례다.
fn as_map<S: serde::Serializer>(by: &[(Axis, Vec<Count>)], s: S) -> Result<S::Ok, S::Error> {
    s.collect_map(by.iter().map(|(axis, counts)| (axis.name(), counts)))
}

/// 분포의 한 칸.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Count {
    /// 그 값 — 칸·종류·태그·담당 이름·에픽 id·마일스톤 id 는 글, 우선순위는 수다. **`None` 은 "없음"** 이다
    /// (태그 없는 줄, 에픽 없는 줄, 담당 없는 줄). 글로 `"none"` 을 적으면 이름이 `none` 인 태그와 안 갈린다.
    pub key: Option<Key>,
    /// 담당의 메일 — `assignee` 축에서 메일이 적힌 담당에만 선다. 이름과 메일을 한 글로 합치지 않는다
    /// (CLAUDE.md "파일에는 갈라서, 화면에는 합쳐서").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub rows: usize,
}

/// [`Count::key`] 의 값.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(untagged)]
pub enum Key {
    Text(String),
    Number(u8),
}

/// 칸마다 만든 수와 닫은 수.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flow {
    pub bucket: Bucket,
    /// 칸을 가른 시간대 — `--created <날>` 이 그날을 읽는 그 시간대다.
    pub zone: String,
    /// 오래된 칸부터. 지금 든 칸이 마지막이다.
    pub buckets: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Slot {
    /// 칸의 첫날(`YYYY-MM-DD`, 위 시간대의 날).
    pub start: String,
    /// 그 칸에 만든 줄.
    pub created: usize,
    /// 지금 done 에 선 줄 가운데 그 칸에 닫힌 것([`closed_at`]).
    pub done: usize,
}

/// 닫힌 줄의 소요(분).
///
/// **평균은 안 낸다** — 긴 꼬리 하나가 평균을 끌고 간다(`Spent::median` 의 moai-wfup 실측). 대신 몇을 쟀는지를
/// 늘 함께 낸다: `measured + unknown == done` 이다.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Spans {
    /// 지금 done 에 선 줄.
    pub done: usize,
    /// 그 가운데 시작과 끝을 다 읽은 것.
    pub measured: usize,
    /// 못 잰 것 — 시작이 안 적혔거나, 못 읽거나, 끝이 시작보다 앞선다. **0 분이 아니다.**
    pub unknown: usize,
    pub median: Option<i64>,
    /// 잰 것의 90번째 백분위수(nearest-rank).
    pub p90: Option<i64>,
}

/// `model:` 줄의 합. **줄과 토큰을 따로 센다** — 토큰을 안 적은 줄은 토큰 합에 안 들고, 그 수는
/// `lines - tokened` 다.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub lines: usize,
    /// 그 가운데 `tokens=` 를 든 줄.
    pub tokened: usize,
    /// 토큰을 든 줄만 더한 값. **든 줄이 없으면 `None`** — 0 은 "공짜로 했다" 로 읽힌다(CLAUDE.md "일한 AI 를
    /// 남긴다").
    pub tokens: Option<u64>,
}

impl Tally {
    fn add(&mut self, w: &Work) {
        self.lines += 1;
        if let Some(t) = w.tokens {
            self.tokened += 1;
            self.tokens = Some(self.tokens.unwrap_or(0).saturating_add(t));
        }
    }
}

/// 노트의 `model:` 줄을 센 것.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Spend {
    /// `model:` 줄을 하나라도 든 줄의 수 — 나머지는 아무도 적지 않았다.
    pub recorded: usize,
    #[serde(flatten)]
    pub all: Tally,
    /// 회사·모델별. 줄이 많은 것부터.
    pub by_model: Vec<ModelTally>,
    /// 등급별. 줄이 많은 것부터, 등급 없는 줄은 맨 뒤.
    pub by_grade: Vec<GradeTally>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelTally {
    /// 회사. 옛 줄(`model: opus-5 (…)`)은 없다 — 모델 이름으로 짐작해 채우지 않는다(`model::work_of`).
    pub provider: Option<String>,
    pub model: String,
    #[serde(flatten)]
    pub tally: Tally,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GradeTally {
    pub grade: Option<String>,
    #[serde(flatten)]
    pub tally: Tally,
}

/// 리뷰 이슈를 센 것.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Reviews {
    pub rows: usize,
    /// 리뷰 이슈에 적힌 `model:` 줄 — 등급별 분포가 곧 리뷰 등급의 분포다(그 줄의 등급은 리뷰 등급과 같은
    /// 낱말이다).
    pub work: Spend,
}

/// 세는 종류 — `--type` 이 없으면 **일**이다([`super::is_work`] 가 일로 치는 그 종류).
pub fn counted_kind(asked: Option<Kind>) -> Kind {
    asked.unwrap_or(Kind::Issue)
}

/// 셀 줄을 고른다 — `show` 의 거르개를 **done·미룸·생각까지 연 채로** 건다.
///
/// **목록의 숨김을 안 따른다.** 목록은 지금 볼 것을 내는 자리라 끝난 것을 숨기지만, 여기는 지나간 것을 세는
/// 자리다 — 숨기면 흐름의 "닫은 수" 가 늘 0 이다. 미룬 것도 센다: 미룬 일도 만들어진 일이고, 빼면 미루기가
/// 흐름의 수를 바꾸는 손잡이가 된다(`status` 의 흐름이 `is_work` 로만 세는 까닭과 같다). 좁히는 말은
/// 그대로 듣는다 — `--deferred` 는 미룬 것만, `-s` 는 그 칸만 고른다.
pub fn select<'a>(all: &'a [Issue], filter: &Filter, wh: &Where, now: &str) -> Vec<&'a Issue> {
    let open = Filter { all: true, ideas: true, ..filter.clone() };
    all.iter().filter(|i| open.matches(i, now, wh)).collect()
}

/// 그 줄이 **닫힌 때** — 지금 done 에 선 줄이 그 칸에 든 때다. `--done` 이 읽는 값과 같다(`Filter::matches`).
///
/// 일 줄이면 `status_since` 고, `mv` 가 쓴 줄이면 `done_at` 과 같다. `done_at` 이 생기기 전에 닫힌 줄에도 선다.
/// 묶음이면 멤버가 마지막으로 done 에 든 때다([`Where::since`]). 지금 done 이 아니면 없다 — 되돌린 줄의
/// `done_at` 은 "그때 끝났었다" 지 지금 닫힌 것이 아니다.
pub fn closed_at<'x>(i: &'x Issue, wh: &'x Where) -> Option<&'x str> {
    (wh.column(i) == DONE).then(|| wh.since(i))
}

/// 센다. `rows` 는 [`select`] 가 고른 줄이고, `work` 는 그 줄들의 노트에서 읽은 `model:` 줄이다(id → 줄).
/// 칸·소속·시간대는 `wh` 가 대고 — 흐름의 날은 `wh.zone` 으로 가른다(없으면 저장된 그대로, UTC) — `now` 는
/// 흐름의 마지막 칸을 정한다.
pub fn of(
    rows: &[&Issue],
    wh: &Where,
    cfg: &Config,
    work: &BTreeMap<String, Vec<Work>>,
    ask: &Ask,
    now: &str,
) -> Stats {
    let kind = counted_kind(ask.kind);
    let counted: Vec<&Issue> = rows.iter().copied().filter(|i| i.kind == kind).collect();
    let mut axes: Vec<Axis> = Vec::new();
    for a in if ask.by.is_empty() { &Axis::ALL[..] } else { &ask.by[..] } {
        if !axes.contains(a) {
            axes.push(*a);
        }
    }
    let by = axes.into_iter().map(|a| (a, spread(a, rows, &counted, wh, cfg))).collect();
    let reviewed: Vec<&Issue> =
        counted.iter().copied().filter(|i| i.tags.iter().any(|t| t == crate::guide::REVIEW_TAG)).collect();
    Stats {
        kind,
        rows: counted.len(),
        by,
        flow: flow(&counted, wh, ask, now),
        lead_time: spans(&counted, wh, |i| Some(i.created_at.as_str())),
        // **묶음의 시작은 안 읽는다** — 그 줄의 `started_at` 은 누가 그 줄에 손으로 `mv` 를 쳤나일 뿐이다(CLAUDE.md
        // "묶음의 기간은 멤버로 잰다"). 모르는 것으로 센다.
        cycle_time: spans(&counted, wh, |i| (!super::is_group(i)).then_some(i.started_at.as_deref()).flatten()),
        work: spend(&counted, work),
        reviews: Reviews { rows: reviewed.len(), work: spend(&reviewed, work) },
    }
}

/// 한 축의 분포.
fn spread(axis: Axis, rows: &[&Issue], counted: &[&Issue], wh: &Where, cfg: &Config) -> Vec<Count> {
    let text = |s: &str| Some(Key::Text(s.to_string()));
    match axis {
        // **설정의 칸은 0 이어도 선다** — 칸 차례가 곧 보드의 차례고, 빈 칸을 빼면 읽는 쪽이 "그 칸이 없다" 와
        // "비었다" 를 못 가른다. 설정에 없는 칸에 선 줄(이름을 고친 칸)은 그 뒤에 이름 차례로 선다.
        Axis::Status => {
            let mut n: BTreeMap<&str, usize> = BTreeMap::new();
            for i in counted {
                *n.entry(wh.column(i)).or_default() += 1;
            }
            let mut out: Vec<Count> =
                cfg.statuses.iter().map(|s| plain(text(s), n.remove(s.as_str()).unwrap_or(0))).collect();
            out.extend(n.into_iter().map(|(s, k)| plain(text(s), k)));
            out
        }
        Axis::Kind => [Kind::Issue, Kind::Epic, Kind::Milestone, Kind::Idea]
            .into_iter()
            .map(|k| plain(text(k.as_str()), rows.iter().filter(|i| i.kind == k).count()))
            .collect(),
        // **0~3 은 비어도 선다** — 칸과 같은 까닭이다. 손으로 고친 줄의 범위 밖 값(`9`)도 그 뒤에 선다: 읽기는
        // 관대하고, 빼면 이 축의 합이 줄 수보다 말없이 작아진다.
        Axis::Priority => {
            let mut n: BTreeMap<u8, usize> = (0..=MAX_PRIORITY).map(|p| (p, 0)).collect();
            for i in counted {
                *n.entry(i.priority()).or_default() += 1;
            }
            n.into_iter().map(|(p, k)| plain(Some(Key::Number(p)), k)).collect()
        }
        // 태그가 여럿인 줄은 태그마다 센다 — 이 축의 합은 줄 수보다 클 수 있다.
        Axis::Tag => ranked(counted.iter().flat_map(|i| {
            let tags: BTreeSet<&str> = i.tags.iter().map(String::as_str).collect();
            if tags.is_empty() { vec![None] } else { tags.into_iter().map(|t| Some((t, None))).collect() }
        })),
        // **이름과 메일의 짝으로 센다.** 담당이 없는 줄은 `-a none` 이 고르는 줄과 같다 — 이름이 없으면 없다.
        Axis::Assignee => {
            ranked(counted.iter().map(|i| i.assignee.as_deref().map(|name| (name, i.assignee_email.as_deref()))))
        }
        Axis::Epic => ranked(counted.iter().map(|i| wh.epic_of(i).map(|e| (e, None)))),
        Axis::Milestone => ranked(counted.iter().map(|i| wh.milestone_of(i).map(|m| (m, None)))),
    }
}

fn plain(key: Option<Key>, rows: usize) -> Count {
    Count { key, email: None, rows }
}

/// 값마다 센 것을 **많은 것부터** 세운다. 같으면 이름 차례고, "없음" 은 수와 상관없이 맨 뒤다 — 값이 아니라
/// 값이 빈 줄의 수라 순위에 섞이면 읽는 쪽이 그것을 한 값으로 읽는다.
fn ranked<'x>(keys: impl Iterator<Item = Option<(&'x str, Option<&'x str>)>>) -> Vec<Count> {
    let mut n: BTreeMap<Option<(&str, Option<&str>)>, usize> = BTreeMap::new();
    for k in keys {
        *n.entry(k).or_default() += 1;
    }
    let mut out: Vec<Count> = n
        .into_iter()
        .map(|(k, rows)| Count {
            key: k.map(|(v, _)| Key::Text(v.to_string())),
            email: k.and_then(|(_, e)| e).map(str::to_string),
            rows,
        })
        .collect();
    out.sort_by(|a, b| a.key.is_none().cmp(&b.key.is_none()).then(b.rows.cmp(&a.rows)).then_with(|| a.key.cmp(&b.key)));
    out
}

/// 칸마다 만든 수와 닫은 수.
///
/// **날은 읽는 사람의 날이다** — `wh.zone` 으로 옮긴 벽시계의 날로 가른다. `--created <날>`·`--done <날>` 이 그날을
/// 읽는 자(`query::spans_hold`)와 같은 시간대라, 칸 하나의 수가 그 칸의 날로 거른 목록의 수와 같다.
fn flow(counted: &[&Issue], wh: &Where, ask: &Ask, now: &str) -> Flow {
    let zone: &crate::tz::Zone = match wh.zone {
        Some(z) => z,
        None => crate::tz::Zone::stored(),
    };
    let mut out = Flow { bucket: ask.bucket, zone: zone.name().to_string(), buckets: Vec::new() };
    // 지금을 못 읽으면 칸을 세울 끝이 없다 — 지어낸 날로 세우지 않는다.
    let Some(now) = parse_rfc3339(now) else { return out };
    let step = ask.bucket.days();
    let day = |t: i64| zone.local(t).div_euclid(86_400);
    let last = ask.bucket.start(day(now));
    // **칸 수를 줄 수와 상관없이 막는다** — 이 수가 곧 만드는 칸의 수다.
    let n = ask.last.clamp(1, 100_000) as i64;
    let first = last - step * (n - 1);
    out.buckets = (0..n)
        .map(|k| {
            let (y, m, d) = crate::model::civil_from_days(first + k * step);
            Slot { start: format!("{y:04}-{m:02}-{d:02}"), created: 0, done: 0 }
        })
        .collect();
    let at = |stamp: &str| -> Option<usize> {
        let s = ask.bucket.start(day(parse_rfc3339(stamp)?));
        (first..=last).contains(&s).then(|| ((s - first) / step) as usize)
    };
    for i in counted {
        if let Some(k) = at(&i.created_at) {
            out.buckets[k].created += 1;
        }
        if let Some(k) = closed_at(i, wh).and_then(at) {
            out.buckets[k].done += 1;
        }
    }
    out
}

/// 지금 done 에 선 줄의 `from` → 닫힌 때([`closed_at`]) 소요.
///
/// **모르는 것은 따로 센다**([`super::spent_in`] 과 같은 금) — 시작이 없거나 못 읽거나, 끝이 시작보다 앞서면
/// `unknown` 이다. 그런 줄을 0 으로 눌러 담으면 "0 분에 했다" 가 잰 값으로 선다.
///
/// **자식을 부모 구간에 접지 않는다** — `spent_in` 은 묶음 하나의 벽시계를 **더하려고** 부모 구간 안의 자식을
/// 접지만, 여기는 줄 하나하나가 표본이다. 그래서 `moai show <에픽>` 의 중앙값과 `moai stats -e <에픽>` 의
/// cycle time 중앙값은 자식이 닫힌 묶음에서 다를 수 있다 — 묻는 것이 다르다.
fn spans<'x>(counted: &[&'x Issue], wh: &'x Where, from: impl Fn(&'x Issue) -> Option<&'x str>) -> Spans {
    let mut out = Spans::default();
    let mut minutes: Vec<i64> = Vec::new();
    for i in counted.iter().copied() {
        let Some(end) = closed_at(i, wh) else { continue };
        out.done += 1;
        let span = from(i).and_then(parse_rfc3339).zip(parse_rfc3339(end)).filter(|(s, e)| e >= s);
        match span {
            Some((s, e)) => minutes.push((e - s) / 60),
            None => out.unknown += 1,
        }
    }
    minutes.sort_unstable();
    out.measured = minutes.len();
    out.median = super::median(&minutes);
    // nearest-rank — `ceil(0.9 n)` 번째. 보간하지 않는다: 잰 값 가운데 하나를 댄다.
    out.p90 = (!minutes.is_empty()).then(|| minutes[(minutes.len() * 9).div_ceil(10) - 1]);
    out
}

/// 그 줄들의 `model:` 줄을 센다.
///
/// **id 마다 한 번이다** — `work` 는 id 로 든 것이라, 같은 id 를 든 줄이 둘이면(`duplicate_id`) 줄마다 더할 때
/// 같은 토큰이 두 번 선다.
fn spend(rows: &[&Issue], work: &BTreeMap<String, Vec<Work>>) -> Spend {
    let mut out = Spend::default();
    let mut models: BTreeMap<(Option<&str>, &str), Tally> = BTreeMap::new();
    let mut grades: BTreeMap<Option<&str>, Tally> = BTreeMap::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for i in rows {
        if !seen.insert(i.id.as_str()) {
            continue;
        }
        let Some(lines) = work.get(&i.id).filter(|l| !l.is_empty()) else { continue };
        out.recorded += 1;
        for w in lines {
            out.all.add(w);
            models.entry((w.provider.as_deref(), w.model.as_str())).or_default().add(w);
            grades.entry(w.grade.as_deref()).or_default().add(w);
        }
    }
    out.by_model = models
        .into_iter()
        .map(|((provider, model), tally)| ModelTally {
            provider: provider.map(str::to_string),
            model: model.to_string(),
            tally,
        })
        .collect();
    // 이름 차례로 모은 것을 **줄 수로** 다시 세운다 — 안정 정렬이라 같은 수는 이름 차례가 남는다.
    out.by_model.sort_by_key(|m| std::cmp::Reverse(m.tally.lines));
    out.by_grade =
        grades.into_iter().map(|(grade, tally)| GradeTally { grade: grade.map(str::to_string), tally }).collect();
    out.by_grade.sort_by(|a, b| a.grade.is_none().cmp(&b.grade.is_none()).then(b.tally.lines.cmp(&a.tally.lines)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn row(id: &str, status: &str, created: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new(status), created)
    }

    /// 닫힌 줄 — `mv` 가 쓴 그대로 칸 시각과 끝 시각이 같다.
    fn closed(id: &str, created: &str, started: Option<&str>, done: &str) -> Issue {
        let mut i = row(id, "done", created);
        i.started_at = started.map(str::to_string);
        i.status_since = done.into();
        i.done_at = Some(done.into());
        i
    }

    fn line(provider: Option<&str>, model: &str, tokens: Option<u64>, grade: Option<&str>) -> Work {
        Work {
            provider: provider.map(str::to_string),
            model: model.into(),
            tokens,
            grade: grade.map(str::to_string),
            why: None,
            at: String::new(),
            by: String::new(),
            by_email: None,
        }
    }

    const NOW: &str = "2026-09-30T12:00:00Z";

    /// 고르고 센다 — 표면이 부르는 그대로.
    fn count(all: &[Issue], ask: &Ask, work: &BTreeMap<String, Vec<Work>>) -> Stats {
        let c = cfg();
        let wh = Where::of(all, &c);
        let rows = select(all, &Filter::default(), &wh, NOW);
        of(&rows, &wh, &c, work, ask, NOW)
    }

    fn axis(st: &Stats, a: Axis) -> &[Count] {
        &st.by.iter().find(|(x, _)| *x == a).unwrap_or_else(|| panic!("{a:?} 축이 없다")).1
    }

    fn text(k: &str) -> Option<Key> {
        Some(Key::Text(k.into()))
    }

    /// **끝난 것·미룬 것·생각까지 고른다** — 목록이 숨기는 셋이다. 세는 것은 일뿐이고, `kind` 축만 고른 줄
    /// 전부를 센다.
    #[test]
    fn it_counts_done_and_deferred_work_and_tallies_every_kind() {
        let mut put_off = row("argos-0003", "todo", "2026-09-01T00:00:00Z");
        put_off.deferred_at = Some("2026-09-02T00:00:00Z".into());
        let mut epic = row("argos-0004", "todo", "2026-09-01T00:00:00Z");
        epic.kind = Kind::Epic;
        let mut idea = row("argos-0005", "todo", "2026-09-01T00:00:00Z");
        idea.kind = Kind::Idea;
        let all = vec![
            row("argos-0001", "todo", "2026-09-01T00:00:00Z"),
            closed("argos-0002", "2026-09-01T00:00:00Z", None, "2026-09-03T00:00:00Z"),
            put_off,
            epic,
            idea,
        ];
        let st = count(&all, &Ask::default(), &BTreeMap::new());
        assert_eq!(st.kind, Kind::Issue);
        assert_eq!(st.rows, 3, "끝난 일과 미룬 일도 센다");
        let kinds: Vec<usize> = axis(&st, Axis::Kind).iter().map(|c| c.rows).collect();
        assert_eq!(kinds, [3, 1, 0, 1], "kind 축은 고른 줄 전부다 — issue·epic·milestone·idea");
        let status: Vec<(Option<Key>, usize)> =
            axis(&st, Axis::Status).iter().map(|c| (c.key.clone(), c.rows)).collect();
        assert_eq!(
            status,
            [(text("todo"), 2), (text("in_progress"), 0), (text("review"), 0), (text("done"), 1)],
            "설정의 칸은 비어도 차례대로 선다"
        );

        let ideas = count(&all, &Ask { kind: Some(Kind::Idea), ..Ask::default() }, &BTreeMap::new());
        assert_eq!((ideas.kind, ideas.rows), (Kind::Idea, 1), "--type 이 센 종류를 바꾼다");
    }

    /// **거르개는 `show` 의 그것이다** — 좁히는 말은 그대로 듣는다.
    #[test]
    fn the_show_filters_narrow_what_is_counted() {
        let mut bug = row("argos-0001", "todo", "2026-09-01T00:00:00Z");
        bug.tags = vec!["bug".into()];
        let all = vec![bug, closed("argos-0002", "2026-09-01T00:00:00Z", None, "2026-09-03T00:00:00Z")];
        let c = cfg();
        let wh = Where::of(&all, &c);
        let only_bugs = Filter { tags: vec![vec!["bug".into()]], ..Filter::default() };
        let rows = select(&all, &only_bugs, &wh, NOW);
        assert_eq!(rows.len(), 1);
        let st = of(&rows, &wh, &c, &BTreeMap::new(), &Ask::default(), NOW);
        assert_eq!(st.rows, 1);
        assert_eq!(st.lead_time.done, 0, "걸러진 닫힌 줄은 소요에도 안 든다");
    }

    /// 많은 것부터, 같으면 이름 차례, **"없음" 은 맨 뒤**. 담당은 이름과 메일을 갈라 든다.
    #[test]
    fn spreads_rank_values_and_put_none_last() {
        let mut a = row("argos-0001", "todo", "2026-09-01T00:00:00Z");
        a.tags = vec!["parser".into(), "bug".into()];
        a.assignee = Some("Kim".into());
        a.assignee_email = Some("kim@example.com".into());
        let mut b = row("argos-0002", "todo", "2026-09-01T00:00:00Z");
        b.tags = vec!["bug".into()];
        b.assignee = Some("Kim".into());
        b.assignee_email = Some("kim@example.com".into());
        let c = row("argos-0003", "todo", "2026-09-01T00:00:00Z");
        let d = row("argos-0004", "todo", "2026-09-01T00:00:00Z");
        let mut e = row("argos-0005", "todo", "2026-09-01T00:00:00Z");
        e.priority = Some(0);
        let mut f = row("argos-0006", "todo", "2026-09-01T00:00:00Z");
        f.priority = Some(9);
        let st = count(&[a, b, c, d, e, f], &Ask::default(), &BTreeMap::new());
        let tags: Vec<(Option<Key>, usize)> = axis(&st, Axis::Tag).iter().map(|c| (c.key.clone(), c.rows)).collect();
        assert_eq!(tags, [(text("bug"), 2), (text("parser"), 1), (None, 4)], "없음이 가장 많아도 맨 뒤다");
        let who = axis(&st, Axis::Assignee);
        assert_eq!(who[0].key, text("Kim"));
        assert_eq!(who[0].email.as_deref(), Some("kim@example.com"));
        assert_eq!((who[1].key.clone(), who[1].rows), (None, 4));
        let p: Vec<usize> = axis(&st, Axis::Priority).iter().map(|c| c.rows).collect();
        assert_eq!(p, [1, 0, 4, 0, 1], "0~3 은 다 서고 안 적은 줄은 기본값 2 다. 범위 밖 값도 뒤에 선다");
        assert_eq!(axis(&st, Axis::Epic), [Count { key: None, email: None, rows: 6 }]);
    }

    /// **소속은 물려받은 것까지다** — 계획이 세운 멤버는 `epic` 을 안 적고 id 로 에픽을 진다(`derived_epic`).
    #[test]
    fn the_epic_axis_reads_the_derived_epic() {
        let mut epic = row("argos-0001", "todo", "2026-09-01T00:00:00Z");
        epic.kind = Kind::Epic;
        let mut stone = row("argos-0009", "todo", "2026-09-01T00:00:00Z");
        stone.kind = Kind::Milestone;
        epic.milestone = Some("argos-0009".into());
        let all = vec![epic, row("argos-0001.a1b", "todo", "2026-09-01T00:00:00Z"), stone];
        let st = count(&all, &Ask::default(), &BTreeMap::new());
        assert_eq!(axis(&st, Axis::Epic)[0].key, text("argos-0001"));
        assert_eq!(axis(&st, Axis::Milestone)[0].key, text("argos-0009"), "마일스톤은 에픽을 타고 온다");
    }

    /// `--by` 는 물은 축만, 물은 차례로 — 같은 축은 한 번이다.
    #[test]
    fn by_picks_the_axes_in_the_order_asked() {
        let all = vec![row("argos-0001", "todo", "2026-09-01T00:00:00Z")];
        let ask = Ask { by: vec![Axis::Tag, Axis::Status, Axis::Tag], ..Ask::default() };
        let st = count(&all, &ask, &BTreeMap::new());
        let names: Vec<Axis> = st.by.iter().map(|(a, _)| *a).collect();
        assert_eq!(names, [Axis::Tag, Axis::Status]);
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains(r#""by":{"tag":[{"key":null,"rows":1}],"status":["#), "{json}");
    }

    /// **소요는 지금 done 에 선 줄만, 모르는 것은 따로 센다** — 시작이 없거나 끝이 시작보다 앞선 줄은
    /// `unknown` 이고 중앙값에 안 든다. 되돌린 줄은 `done_at` 이 남아도 안 센다.
    #[test]
    fn spans_count_the_unknown_apart_and_skip_reopened_rows() {
        let mut reopened =
            closed("argos-0005", "2026-09-01T00:00:00Z", Some("2026-09-01T00:00:00Z"), "2026-09-02T00:00:00Z");
        reopened.status = Status::new("in_progress");
        let all = vec![
            // 시작 1시간 뒤 닫힘 — 만든 지 2시간.
            closed("argos-0001", "2026-09-01T00:00:00Z", Some("2026-09-01T01:00:00Z"), "2026-09-01T02:00:00Z"),
            // 시작 3시간 뒤 닫힘.
            closed("argos-0002", "2026-09-01T00:00:00Z", Some("2026-09-01T00:00:00Z"), "2026-09-01T03:00:00Z"),
            // 시작을 모른다 — 옛 바이너리가 집은 줄.
            closed("argos-0003", "2026-09-01T00:00:00Z", None, "2026-09-01T10:00:00Z"),
            // 끝이 시작보다 앞선다 — 손으로 푼 줄.
            closed("argos-0004", "2026-09-01T00:00:00Z", Some("2026-09-02T00:00:00Z"), "2026-09-01T05:00:00Z"),
            reopened,
        ];
        let st = count(&all, &Ask::default(), &BTreeMap::new());
        assert_eq!(
            st.cycle_time,
            Spans { done: 4, measured: 2, unknown: 2, median: Some(120), p90: Some(180) },
            "0 분으로 눌러 담지 않는다"
        );
        assert_eq!(st.lead_time.done, 4);
        assert_eq!(st.lead_time.measured, 4, "만든 때는 다 있다");
        assert_eq!(st.lead_time.median, Some(240), "120·180·300·600 의 가운데 둘의 평균");
        assert_eq!(st.lead_time.p90, Some(600));
    }

    /// 아무것도 안 닫혔으면 소요는 빈 값이다 — 0 이 아니다.
    #[test]
    fn nothing_closed_gives_no_median() {
        let st = count(&[row("argos-0001", "todo", "2026-09-01T00:00:00Z")], &Ask::default(), &BTreeMap::new());
        assert_eq!(st.lead_time, Spans::default());
        assert_eq!(
            serde_json::to_string(&st.cycle_time).unwrap(),
            r#"{"done":0,"measured":0,"unknown":0,"median":null,"p90":null}"#
        );
    }

    /// **묶음의 시작은 안 읽는다** — 그 줄의 `started_at` 은 손으로 친 `mv` 다.
    #[test]
    fn a_group_s_own_start_is_not_its_cycle() {
        let mut epic =
            closed("argos-0001", "2026-09-01T00:00:00Z", Some("2026-09-01T00:00:00Z"), "2026-09-02T00:00:00Z");
        epic.kind = Kind::Epic;
        epic.status = Status::new("todo");
        let all = vec![epic, closed("argos-0001.a1b", "2026-09-01T00:00:00Z", None, "2026-09-03T00:00:00Z")];
        let st = count(&all, &Ask { kind: Some(Kind::Epic), ..Ask::default() }, &BTreeMap::new());
        assert_eq!(st.rows, 1);
        assert_eq!(st.cycle_time.done, 1, "멤버가 다 닫힌 에픽은 done 에 선다");
        assert_eq!(st.cycle_time.unknown, 1, "적힌 시작을 일의 시작으로 읽었다");
        assert_eq!(st.lead_time.median, Some(2 * 24 * 60), "끝은 멤버가 마지막으로 닫힌 때다");
    }

    /// **흐름은 읽는 사람의 날로 가른다** — 서울의 월요일 오전 8시(UTC 일요일 23시)에 만든 줄은 서울에서는 그
    /// 주에, UTC 로는 지난주에 선다. 닫은 수는 지금 done 에 선 줄만 센다.
    #[test]
    fn flow_buckets_by_the_reader_s_week() {
        let mut late = row("argos-0001", "todo", "2026-09-27T23:00:00Z");
        late.status = Status::new("todo");
        let all = vec![
            late,
            closed("argos-0002", "2026-09-22T00:00:00Z", None, "2026-09-29T00:00:00Z"),
            // 창 밖 — 여덟 주보다 앞.
            row("argos-0003", "todo", "2026-01-05T00:00:00Z"),
        ];
        let c = cfg();
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let ask = Ask { last: 2, ..Ask::default() };
        let mut wh = Where::of(&all, &c);
        let rows = select(&all, &Filter::default(), &wh, NOW);
        let utc = of(&rows, &wh, &c, &BTreeMap::new(), &ask, NOW).flow;
        assert_eq!(utc.zone, "UTC");
        assert_eq!(
            utc.buckets,
            [
                Slot { start: "2026-09-21".into(), created: 2, done: 0 },
                Slot { start: "2026-09-28".into(), created: 0, done: 1 },
            ]
        );
        wh.zone = Some(&seoul);
        let local = of(&rows, &wh, &c, &BTreeMap::new(), &ask, NOW).flow;
        assert_eq!(local.zone, "Asia/Seoul");
        assert_eq!(local.buckets[1], Slot { start: "2026-09-28".into(), created: 1, done: 1 }, "서울의 월요일이다");

        let days =
            of(&rows, &wh, &c, &BTreeMap::new(), &Ask { bucket: Bucket::Day, last: 3, ..Ask::default() }, NOW).flow;
        let starts: Vec<&str> = days.buckets.iter().map(|s| s.start.as_str()).collect();
        assert_eq!(starts, ["2026-09-28", "2026-09-29", "2026-09-30"], "지금 든 날까지 친다");
        assert_eq!(days.buckets[0].created, 1);
        assert_eq!(days.buckets[1].done, 1);
    }

    /// 지금을 못 읽으면 칸을 안 세운다 — 1970 년의 칸을 지어내지 않는다.
    #[test]
    fn an_unreadable_now_gives_no_buckets() {
        let all = vec![row("argos-0001", "todo", "2026-09-01T00:00:00Z")];
        let c = cfg();
        let wh = Where::of(&all, &c);
        let rows = select(&all, &Filter::default(), &wh, NOW);
        let st = of(&rows, &wh, &c, &BTreeMap::new(), &Ask::default(), "어제");
        assert!(st.flow.buckets.is_empty());
    }

    /// **토큰은 든 줄만 더하고, 하나도 없으면 합이 없다** — 0 이 아니다. 같은 id 의 줄이 둘이어도 한 번만 센다.
    /// 리뷰는 `review` 태그를 단 줄이고, 그 줄의 등급이 리뷰 등급이다.
    #[test]
    fn work_sums_tokens_only_where_written_and_reviews_carry_their_grade() {
        let mut review = row("argos-0001.r1v", "done", "2026-09-01T00:00:00Z");
        review.tags = vec!["review".into()];
        let all = vec![
            row("argos-0001", "todo", "2026-09-01T00:00:00Z"),
            review,
            row("argos-0002", "todo", "2026-09-01T00:00:00Z"),
            // 같은 id 의 둘째 줄 — 머지를 잘못 푼 파일.
            row("argos-0002", "todo", "2026-09-01T00:00:00Z"),
            row("argos-0003", "todo", "2026-09-01T00:00:00Z"),
        ];
        let work: BTreeMap<String, Vec<Work>> = [
            (
                "argos-0001".to_string(),
                vec![
                    line(Some("anthropic"), "opus-5", Some(1000), Some("high")),
                    line(Some("anthropic"), "opus-5", None, Some("high")),
                ],
            ),
            ("argos-0001.r1v".to_string(), vec![line(Some("anthropic"), "opus-5", Some(500), Some("max"))]),
            ("argos-0002".to_string(), vec![line(None, "opus-5", None, None)]),
        ]
        .into_iter()
        .collect();
        let st = count(&all, &Ask::default(), &work);
        assert_eq!(st.work.recorded, 3, "같은 id 의 둘째 줄은 다시 안 센다");
        assert_eq!(st.work.all, Tally { lines: 4, tokened: 2, tokens: Some(1500) });
        let models: Vec<(Option<&str>, &str, &Tally)> =
            st.work.by_model.iter().map(|m| (m.provider.as_deref(), m.model.as_str(), &m.tally)).collect();
        assert_eq!(
            models,
            [
                (Some("anthropic"), "opus-5", &Tally { lines: 3, tokened: 2, tokens: Some(1500) }),
                (None, "opus-5", &Tally { lines: 1, tokened: 0, tokens: None }),
            ],
            "회사를 모르는 옛 줄은 짐작해 합치지 않는다"
        );
        let grades: Vec<Option<&str>> = st.work.by_grade.iter().map(|g| g.grade.as_deref()).collect();
        assert_eq!(grades, [Some("high"), Some("max"), None], "등급 없는 줄은 맨 뒤다");
        assert_eq!(st.reviews.rows, 1);
        assert_eq!(st.reviews.work.by_grade[0].grade.as_deref(), Some("max"));
        assert_eq!(st.reviews.work.all.tokens, Some(500));

        let json = serde_json::to_string(&st.work.by_model[1]).unwrap();
        assert_eq!(json, r#"{"provider":null,"model":"opus-5","lines":1,"tokened":0,"tokens":null}"#);
    }
}
