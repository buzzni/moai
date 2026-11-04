//! `changes` — 커서 뒤로 바뀐 줄과 사라진 줄을 고른다(moai-5n6m.2ki). **순수 함수다.**
//!
//! 받는 것은 읽어 건넨 활성 스냅샷 `&[Issue]` 와 저널 줄(`rm`·`note`), 커서 글, 지금 시각뿐이다. 파일을 열지도 찍지도
//! 않는다. 답은 **줄 본문이 아니라 id** 다 — 스냅샷이 진실이고, 클라이언트가 바뀐 줄을 다시 읽는다.
//!
//! 이것은 **저널을 접어 상태를 만드는 일이 아니다.** 저널에서 얻는 것은 "이 id 를 다시 읽어라" 는 가리킴 둘뿐이다 —
//! `rm` 줄은 스냅샷에서 사라진 줄을 찾고(`show --removed` 와 같은 자리), `note` 줄은 스냅샷의 `updated_at` 을 안
//! 건드리는 메모가 붙은 줄을 찾는다(`show -g` 와 같은 자리). 어느 종류를 읽을지는 부르는 쪽이 고른다(이 함수는
//! 종류를 모른다 — `rm` 이 아닌 줄은 모두 "바뀜" 이다). 저널에 새 필드도, 스냅샷에 새 필드도 없다.
//!
//! ## 커서
//!
//! 클라이언트가 해석하지 않는 불투명 글(base64url 의 JSON)이다. **서버에 아무것도 안 둔다** — 데몬이 다시 떠도
//! 커서는 산다(`cmd::supervise` 의 `watch` 커서와 같은 꼴이다). 담는 것은 다섯이다.
//!
//! - `t`: 창의 시작(초). 커서를 낸 때에서 [`LAG`] 초 앞이다. 그 뒤에 `updated_at`·`ts` 가 찍힌 줄만 후보다
//! - `seen`: 커서를 낼 때 창 안에 있던 줄의 **지문**. 후보 가운데 지문이 같은 줄은 안 바뀐 것이다
//! - `n`: 그때의 활성 줄 수. 줄이 아무 말 없이 사라지거나 불어난 것(아카이브·병합·손 고침)을 잰다
//! - `a`: 아카이브 디렉터리의 표식. 바뀌었으면 줄이 옮겨 갔을 수 있다
//! - `s`: 프로젝트(뿌리 경로)의 지문
//!
//! **동률.** `updated_at` 의 해상도는 초다. 같은 초에 줄 둘이 바뀌어도, 같은 줄이 같은 초에 두 번 바뀌어도 놓치지
//! 않는다 — 둘째 고침은 줄의 직렬화를 바꿔 지문이 다르다. 창을 [`LAG`] 초 앞에서 열어 두는 것은, 시각을 먼저 뜬 쓰기가
//! 락을 기다리다 우리 읽기 뒤에 확정되는 틈 때문이다. 창 안의 줄은 지문으로 걸러서 같은 줄을 두 번 알리지 않는다.
//!
//! **`reset`.** 아래 어느 하나라도 서면 목록을 비우고 `reset: true` 를 낸다 — 클라이언트는 전체를 다시 읽고 이
//! 답의 새 커서로 이어 간다. 커서가 없거나 못 읽거나 다른 프로젝트의 것, 시계가 커서보다 뒤로 갔다, 저널을 못
//! 읽었거나 창에 들었을 수 있는 못 푼 조각이 있다, 아카이브 표식이 바뀌었다, 줄 수가 저널로 설명되지 않는다.
//! **새 커서는 `reset` 일 때도 늘 담긴다.**
//!
//! 알려진 틈: 줄 수가 저널과 비긴 아카이브(같은 창에 `rm` 이 있었다)는 표식이 막고, 표식이 안 바뀌는 병합은 줄 수가
//! 막는다. 둘이 한꺼번에 비기는 병합은 못 잡는다 — 그것을 잡으려면 줄의 id 를 커서에 모두 실어야 한다.

use crate::model::{Issue, JournalEntry, parse_rfc3339};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 커서의 판. 못 알아보는 판은 `reset` 이다.
const VERSION: u32 = 1;

/// 창을 커서를 낸 때보다 앞에서 연다(초). 시각을 뜬 쓰기가 락을 기다려 우리 읽기 뒤에 확정되는 틈을 덮는다.
pub const LAG: i64 = 2;

#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    v: u32,
    s: String,
    t: i64,
    a: String,
    n: usize,
    /// 창 안에 있던 활성 줄 — id → 지문.
    rows: BTreeMap<String, String>,
    /// 창 안에 있던 저널 줄의 지문.
    log: BTreeSet<String>,
}

/// 답. 줄 본문은 싣지 않는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub cursor: String,
    /// 커서 뒤로 바뀐(새로 생기거나 고친 줄, 메모가 붙은 줄) 산 줄의 id. 차례는 id 순이다.
    pub changed: Vec<String>,
    /// 커서 뒤로 `rm` 이 지웠고 지금 스냅샷에 없는 id.
    pub removed: Vec<String>,
    /// 참이면 두 목록은 비었다 — 전체를 다시 읽는다.
    pub reset: bool,
}

/// 부르는 쪽이 읽어 건네는 것.
pub struct Input<'a> {
    /// 활성 스냅샷. 아카이브는 넣지 않는다.
    pub issues: &'a [Issue],
    /// 저널 줄 — `rm` 과 `note`. 다른 종류를 건네면 그것도 "바뀜" 이다.
    pub journal: &'a [JournalEntry],
    /// 저널에서 못 푼 조각이 댄 `ts`. `None` 은 도장을 못 읽은 것이다.
    pub torn: &'a [Option<&'a str>],
    /// 읽지 못한 저널 파일이 있었다.
    pub unread: bool,
    /// 프로젝트를 가르는 글(뿌리 경로).
    pub scope: &'a str,
    /// 아카이브 디렉터리의 표식 — 바뀌면 줄이 옮겨 갔을 수 있다.
    pub archive: &'a str,
    /// 지금(초).
    pub now: i64,
}

/// 커서 뒤로 바뀐 것. 커서가 없으면 `reset` 이다 — 클라이언트는 전체를 읽고 이 답의 커서로 이어 간다.
pub fn changes(input: &Input, cursor: Option<&str>) -> Changes {
    let fresh = fresh(input);
    let reset = || Changes { cursor: fresh.clone(), changed: Vec::new(), removed: Vec::new(), reset: true };
    let Some(old) = cursor.and_then(decode) else { return reset() };
    if old.v != VERSION || old.s != digest(input.scope.as_bytes()) {
        return reset();
    }
    if old.t > input.now {
        return reset();
    }
    if old.a != input.archive {
        return reset();
    }
    if input.unread || input.torn.iter().any(|ts| ts.and_then(parse_rfc3339).is_none_or(|t| t >= old.t)) {
        return reset();
    }

    let mut changed: BTreeSet<&str> = BTreeSet::new();
    for issue in input.issues {
        let in_window = parse_rfc3339(&issue.updated_at).is_none_or(|t| t >= old.t);
        if in_window && old.rows.get(&issue.id) != Some(&row_digest(issue)) {
            changed.insert(&issue.id);
        }
    }
    let mut gone: BTreeSet<&str> = BTreeSet::new();
    let mut deleted = 0usize;
    for e in input.journal {
        // 도장을 못 읽는 줄은 어느 창에도 안 든다 — `show --removed --since` 와 같은 약속이다.
        if parse_rfc3339(&e.ts).is_none_or(|t| t < old.t) || old.log.contains(&entry_digest(e)) {
            continue;
        }
        if e.removes_issue() {
            deleted += 1;
            gone.insert(&e.id);
        } else {
            changed.insert(&e.id);
        }
    }
    // 지금 있는 줄만 "바뀜" 이다 — 사라진 줄의 메모는 알릴 것이 없다. 지웠다가 다시 지은 id 는 있는 쪽이 이긴다.
    let present: BTreeSet<&str> = input.issues.iter().map(|i| i.id.as_str()).collect();
    changed.retain(|id| present.contains(id));
    changed.extend(gone.iter().filter(|id| present.contains(*id)));
    gone.retain(|id| !present.contains(id));

    // 줄 수가 저널로 설명되는가 — 새 줄(창 안에서 지어졌고 커서가 몰랐던 줄)만큼 늘고, 지움 줄 수 이하로 준다.
    let added = input
        .issues
        .iter()
        .filter(|i| parse_rfc3339(&i.created_at).is_some_and(|t| t >= old.t) && !old.rows.contains_key(&i.id))
        .count();
    let upper = old.n + added;
    let lower = upper.saturating_sub(deleted);
    if input.issues.len() > upper || input.issues.len() < lower {
        return reset();
    }

    Changes {
        cursor: fresh,
        changed: changed.into_iter().map(str::to_string).collect(),
        removed: gone.into_iter().map(str::to_string).collect(),
        reset: false,
    }
}

/// 지금 상태의 커서 글.
fn fresh(input: &Input) -> String {
    let t = input.now - LAG;
    let rows = input
        .issues
        .iter()
        .filter(|i| parse_rfc3339(&i.updated_at).is_none_or(|u| u >= t))
        .map(|i| (i.id.clone(), row_digest(i)))
        .collect();
    let log = input.journal.iter().filter(|e| parse_rfc3339(&e.ts).is_some_and(|u| u >= t)).map(entry_digest).collect();
    let cursor = Cursor {
        v: VERSION,
        s: digest(input.scope.as_bytes()),
        t,
        a: input.archive.to_string(),
        n: input.issues.len(),
        rows,
        log,
    };
    // `Cursor` 는 문자열 키의 맵과 수뿐이라 직렬화가 넘어질 길이 없다.
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).unwrap_or_default())
}

fn decode(text: &str) -> Option<Cursor> {
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(text).ok()?).ok()
}

fn digest(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(&ring::digest::digest(&ring::digest::SHA256, bytes).as_ref()[..12])
}

/// 줄의 지문 — 줄이 파일에 쓰이는 그 직렬화다. 같은 초의 둘째 고침은 이것이 달라진다.
fn row_digest(issue: &Issue) -> String {
    digest(&serde_json::to_vec(issue).unwrap_or_default())
}

/// 저널 줄의 지문 — 같은 초에 같은 줄이 둘이면 하나로 센다(`watch` 와 같은 약속이다).
fn entry_digest(entry: &JournalEntry) -> String {
    digest(&serde_json::to_vec(entry).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Actor, Kind, Status};

    const T0: &str = "2026-11-04T10:00:00Z";
    const SCOPE: &str = "/p/a";

    fn secs(s: &str) -> i64 {
        parse_rfc3339(s).unwrap()
    }

    fn row(id: &str, at: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new("todo"), at)
    }

    fn edited(mut i: Issue, title: &str, at: &str) -> Issue {
        i.title = title.into();
        i.updated_at = at.into();
        i
    }

    fn by() -> Actor {
        Actor { name: "나".into(), email: "me@example.com".into() }
    }

    fn rm(id: &str, at: &str) -> JournalEntry {
        JournalEntry::removed(id, "제목", at, &by())
    }

    fn note(id: &str, at: &str) -> JournalEntry {
        JournalEntry::note(id, "메모", at, &by())
    }

    fn input<'a>(issues: &'a [Issue], journal: &'a [JournalEntry], now: &str) -> Input<'a> {
        Input { issues, journal, torn: &[], unread: false, scope: SCOPE, archive: "", now: secs(now) }
    }

    /// 한 번 읽어 커서를 받는다.
    fn first(issues: &[Issue], journal: &[JournalEntry], now: &str) -> String {
        changes(&input(issues, journal, now), None).cursor
    }

    #[test]
    fn no_cursor_resets_and_hands_out_a_cursor() {
        let rows = [row("a-1", T0)];
        let got = changes(&input(&rows, &[], T0), None);
        assert!(got.reset && got.changed.is_empty() && got.removed.is_empty());
        // 그 커서로 바로 이으면 바뀐 것이 없다.
        let next = changes(&input(&rows, &[], T0), Some(&got.cursor));
        assert_eq!(next, Changes { cursor: next.cursor.clone(), changed: vec![], removed: vec![], reset: false });
    }

    #[test]
    fn rows_changed_after_the_cursor_are_named_and_unchanged_ones_are_not() {
        let t1 = "2026-11-04T10:00:10Z";
        let old = [row("a-1", "2026-11-04T09:00:00Z"), row("a-2", T0)];
        let cursor = first(&old, &[], T0);
        let now = [old[0].clone(), edited(old[1].clone(), "고침", t1), row("a-3", t1)];
        let got = changes(&input(&now, &[], t1), Some(&cursor));
        assert!(!got.reset, "{got:?}");
        assert_eq!(got.changed, ["a-2", "a-3"]);
        assert!(got.removed.is_empty());
    }

    #[test]
    fn two_rows_changed_in_the_cursors_own_second_are_both_caught() {
        let old = [row("a-1", T0), row("a-2", "2026-11-04T09:00:00Z")];
        let cursor = first(&old, &[], T0);
        // 커서를 낸 바로 그 초에 둘이 바뀌고 하나가 생긴다.
        let now = [edited(old[0].clone(), "고침", T0), edited(old[1].clone(), "고침", T0), row("a-3", T0)];
        let got = changes(&input(&now, &[], T0), Some(&cursor));
        assert!(!got.reset, "{got:?}");
        assert_eq!(got.changed, ["a-1", "a-2", "a-3"]);
    }

    #[test]
    fn a_row_changed_twice_in_one_second_is_not_lost() {
        let base = row("a-1", "2026-11-04T09:00:00Z");
        let once = edited(base.clone(), "첫 고침", T0);
        let cursor = first(std::slice::from_ref(&once), &[], T0);
        // 같은 초에 한 번 더 — `updated_at` 은 그대로고 줄만 달라졌다.
        let twice = edited(once.clone(), "둘째 고침", T0);
        let got = changes(&input(std::slice::from_ref(&twice), &[], T0), Some(&cursor));
        assert_eq!(got.changed, ["a-1"]);
        // 이미 알린 모습 그대로면 다시 알리지 않는다.
        let again = changes(&input(std::slice::from_ref(&once), &[], T0), Some(&cursor));
        assert!(again.changed.is_empty() && !again.reset);
    }

    #[test]
    fn a_change_inside_the_lag_window_is_named_once() {
        // 시각은 LAG 안에서 떴지만 우리 읽기 뒤에 확정된 쓰기 — 커서 시각보다 1초 앞의 도장이다.
        let cursor = first(&[], &[], "2026-11-04T10:00:05Z");
        let late = [row("a-1", "2026-11-04T10:00:04Z")];
        let got = changes(&input(&late, &[], "2026-11-04T10:00:06Z"), Some(&cursor));
        assert_eq!(got.changed, ["a-1"]);
        let after = changes(&input(&late, &[], "2026-11-04T10:00:06Z"), Some(&got.cursor));
        assert!(after.changed.is_empty());
    }

    #[test]
    fn removed_rows_come_from_rm_lines_after_the_cursor_and_only_if_gone() {
        let t1 = "2026-11-04T10:00:10Z";
        let rows = [row("a-1", "2026-11-04T09:00:00Z"), row("a-2", "2026-11-04T09:00:00Z")];
        let old_rm = [rm("a-0", "2026-11-04T08:00:00Z")];
        let cursor = first(&rows, &old_rm, T0);
        // a-2 를 지웠다. 옛 지움(a-0)은 커서 앞이라 다시 안 나온다.
        let now = [rows[0].clone()];
        let log = [old_rm[0].clone(), rm("a-2", t1)];
        let got = changes(&input(&now, &log, t1), Some(&cursor));
        assert!(!got.reset, "{got:?}");
        assert_eq!(got.removed, ["a-2"]);
        assert!(got.changed.is_empty());
        // 지웠다가 같은 id 로 다시 지었으면 지운 것이 아니라 바뀐 것이다.
        let back = [rows[0].clone(), row("a-2", "2026-11-04T10:00:11Z")];
        let got = changes(&input(&back, &log, "2026-11-04T10:00:11Z"), Some(&cursor));
        assert_eq!(got.changed, ["a-2"]);
        assert!(got.removed.is_empty() && !got.reset, "{got:?}");
    }

    #[test]
    fn a_note_names_its_row_but_a_note_on_a_missing_row_is_dropped() {
        let t1 = "2026-11-04T10:00:10Z";
        let rows = [row("a-1", "2026-11-04T09:00:00Z")];
        let cursor = first(&rows, &[], T0);
        let log = [note("a-1", t1), note("gone", t1)];
        let got = changes(&input(&rows, &log, t1), Some(&cursor));
        assert_eq!(got.changed, ["a-1"]);
        // 같은 초에 같은 메모를 또 알리지 않는다.
        let next = changes(&input(&rows, &log, t1), Some(&got.cursor));
        assert!(next.changed.is_empty());
    }

    #[test]
    fn a_garbled_or_foreign_or_unknown_cursor_resets() {
        let rows = [row("a-1", T0)];
        let good = first(&rows, &[], T0);
        let foreign = changes(&Input { scope: "/p/b", ..input(&rows, &[], T0) }, Some(&good));
        assert!(foreign.reset);
        for bad in ["", "not base64!", "e30", &URL_SAFE_NO_PAD.encode(br#"{"v":9}"#)] {
            assert!(changes(&input(&rows, &[], T0), Some(bad)).reset, "{bad}");
        }
        let other_version = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(
                &serde_json::json!({"v":2,"s":digest(SCOPE.as_bytes()),"t":0,"a":"","n":1,"rows":{},"log":[]}),
            )
            .unwrap(),
        );
        assert!(changes(&input(&rows, &[], T0), Some(&other_version)).reset);
    }

    #[test]
    fn a_clock_that_went_back_resets() {
        let rows = [row("a-1", T0)];
        let cursor = first(&rows, &[], "2026-11-04T10:00:30Z");
        let got = changes(&input(&rows, &[], "2026-11-04T10:00:00Z"), Some(&cursor));
        assert!(got.reset);
    }

    #[test]
    fn rows_that_vanish_without_an_rm_line_reset() {
        let rows = [row("a-1", "2026-11-04T09:00:00Z"), row("a-2", "2026-11-04T09:00:00Z")];
        let cursor = first(&rows, &[], T0);
        // 아카이브·손 고침·병합이 줄을 빼 갔다 — 저널이 설명하지 못한다.
        let got = changes(&input(&rows[..1], &[], "2026-11-04T10:00:10Z"), Some(&cursor));
        assert!(got.reset && got.changed.is_empty());
        // 늘어난 것도 마찬가지다 — 병합이 커서보다 오래된 도장의 줄을 들였다.
        let more = [rows[0].clone(), rows[1].clone(), row("a-3", "2026-11-04T08:00:00Z")];
        assert!(changes(&input(&more, &[], "2026-11-04T10:00:10Z"), Some(&cursor)).reset);
    }

    #[test]
    fn an_archive_mark_that_moved_resets() {
        let rows = [row("a-1", "2026-11-04T09:00:00Z")];
        let cursor = first(&rows, &[], T0);
        let moved = Input { archive: "2026.jsonl:1:2", ..input(&rows, &[], T0) };
        assert!(changes(&moved, Some(&cursor)).reset);
    }

    #[test]
    fn a_journal_that_could_hide_a_change_resets() {
        let rows = [row("a-1", "2026-11-04T09:00:00Z")];
        let cursor = first(&rows, &[], T0);
        let t1 = "2026-11-04T10:00:10Z";
        // 못 읽은 저널 파일, 도장을 못 읽은 조각, 창에 들었을 수 있는 조각은 reset 이다.
        assert!(changes(&Input { unread: true, ..input(&rows, &[], t1) }, Some(&cursor)).reset);
        assert!(changes(&Input { torn: &[None], ..input(&rows, &[], t1) }, Some(&cursor)).reset);
        assert!(changes(&Input { torn: &[Some(t1)], ..input(&rows, &[], t1) }, Some(&cursor)).reset);
        // 창 앞의 조각은 이 커서가 놓칠 것이 없다.
        let old = [Some("2026-11-04T08:00:00Z")];
        assert!(!changes(&Input { torn: &old, ..input(&rows, &[], t1) }, Some(&cursor)).reset);
    }
}
