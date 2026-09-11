//! 이슈와 저널 레코드. **여기는 파일시스템도 터미널도 모른다.**

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 우선순위를 안 적었을 때의 값. 기본값은 파일에 쓰지 않는다 (§정규화).
pub const DEFAULT_PRIORITY: u8 = 2;
pub const MAX_PRIORITY: u8 = 3;

/// 이슈의 구조적 종류. `tags` 와 축이 다르다 —
/// `kind` 는 무엇인가, `tags` 는 어떤 성격인가.
///
/// `Milestone` 은 2단계에 CLI 가 붙지만 **지금도 읽을 줄은 안다.**
/// 모르는 값으로 거절하면 새 바이너리가 쓴 파일을 옛 바이너리가 통째로 못 읽는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Issue,
    Epic,
    Milestone,
}

impl Kind {
    fn is_default(&self) -> bool {
        *self == Kind::Issue
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Issue => "issue",
            Kind::Epic => "epic",
            Kind::Milestone => "milestone",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;
    fn from_str(s: &str) -> Result<Kind, String> {
        match s {
            "issue" => Ok(Kind::Issue),
            "epic" => Ok(Kind::Epic),
            "milestone" => Ok(Kind::Milestone),
            _ => Err(format!("`{s}` 는 종류가 아니다. 종류: issue, epic, milestone")),
        }
    }
}

/// 칸반 컬럼. **enum 이 아니다** — 설정으로 칸을 더할 수 있어야 하기 때문이다.
///
/// 검증은 **쓰기에만** 한다. 읽기에서 거절하면 설정에서 칸 하나를 지운 순간
/// 파일 전체가 안 읽히고, 무엇이 문제인지 볼 방법까지 같이 사라진다.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Status(pub String);

impl Status {
    pub fn new(s: impl Into<String>) -> Status {
        Status(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// 코드가 상태에 묻는 사실상 유일한 질문.
    pub fn is_done(&self) -> bool {
        self.0 == crate::config::DONE
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `.moai/issues.jsonl` 의 한 줄.
///
/// **키 순서가 곧 직렬화 순서다** — `serde_json::Value` 로 쓰면 알파벳으로
/// 정렬돼 `id`·`title` 이 줄 가운데로 밀린다. struct 로 써야 `cut -c1-100`
/// 만으로 파일이 읽힌다. 그리고 `body` 는 유일하게 길어질 수 있어 맨 뒤다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Kind::is_default")]
    pub kind: Kind,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// 소속. **파생이 아니라 필드다** — 부모-자식(id 의 점)과 직교한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    /// 더 큰 소속. 에픽과도 직교한다 — 에픽에 안 붙은 이슈가 마일스톤에는
    /// 붙을 수 있고, 그 반대도 된다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// `status` 가 마지막으로 바뀐 때. 방치 검사와 "review 에 6일" 이 여기서 나온다.
    pub status_since: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// 모르는 필드를 **잃지 않고 되쓴다.**
    ///
    /// 스냅샷은 매 쓰기마다 전체 파일이 다시 써진다. 이것이 없으면 2단계
    /// 필드(`milestone` 등)가 든 파일을 1단계 바이너리가 한 번 건드리는 것만으로
    /// 1만 줄에서 그 필드가 조용히 사라진다. 조용한 손실이 이 설계가 못 견디는
    /// 유일한 실패 모드다. 대신 `moai status` 가 "모르는 필드를 들고 있다" 를 비춘다.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rest: BTreeMap<String, serde_json::Value>,
}

/// 태그 표기를 하나로 맞춘다. 앞의 `#` 은 있어도 없어도 같은 태그고,
/// **대소문자도 가리지 않는다.**
///
/// 소문자로 접는 이유 — 태그를 만드는 것이 주로 에이전트고, 같은 개념을
/// 매번 `Bug`·`bug` 로 달리 적는다. 접지 않으면 한 개념이 둘로 갈라져
/// 어느 `-t` 로도 한 번에 못 찾는다.
///
/// **쓰는 쪽(`add`·`edit`)과 찾는 쪽(`query`)이 같은 함수를 쓴다.** 둘이
/// 갈라지면 방금 붙인 태그를 같은 낱말로 찾지 못한다.
pub fn normalize_tag(raw: &str) -> String {
    raw.trim().trim_start_matches('#').trim().to_lowercase()
}

impl Issue {
    pub fn new(id: String, title: String, kind: Kind, status: Status, at: &str) -> Issue {
        Issue {
            id,
            title,
            kind,
            status,
            priority: None,
            tags: Vec::new(),
            assignee: None,
            epic: None,
            milestone: None,
            created_at: at.to_string(),
            updated_at: at.to_string(),
            status_since: at.to_string(),
            body: None,
            rest: BTreeMap::new(),
        }
    }

    pub fn priority(&self) -> u8 {
        self.priority.unwrap_or(DEFAULT_PRIORITY)
    }

    /// 쓰기 직전에 한 번. 결정적 출력과 기본값 생략을 여기서 보장한다.
    pub fn normalize(&mut self) {
        for t in self.tags.iter_mut() {
            *t = normalize_tag(t);
        }
        self.tags.retain(|t| !t.is_empty());
        self.tags.sort();
        self.tags.dedup();
        if self.priority == Some(DEFAULT_PRIORITY) {
            self.priority = None; // 기본값을 쓰면 1만 줄이 통째로 diff 에 뜬다
        }
        if self.body.as_deref().is_some_and(str::is_empty) {
            self.body = None;
        }
    }

    /// 쓰기는 읽기보다 엄하다. 읽기는 아는 만큼 보여주고, 쓰기는 거부한다.
    pub fn validate(&self, cfg: &crate::config::Config) -> Result<(), String> {
        if !crate::id::is_valid(&self.id) {
            return Err(format!("id 형식이 아니다 — {:?}", self.id));
        }
        let t = self.title.trim();
        if t.is_empty() {
            return Err(format!("{}: 제목이 비었다", self.id));
        }
        if self.title.contains('\n') {
            return Err(format!("{}: 제목은 한 줄이다", self.id));
        }
        cfg.require_known(self.status.as_str()).map_err(|e| format!("{}: {e}", self.id))?;
        if self.priority.is_some_and(|p| p > MAX_PRIORITY) {
            return Err(format!(
                "{}: 우선순위는 0~{MAX_PRIORITY} 다 — {:?}",
                self.id, self.priority
            ));
        }
        for tag in &self.tags {
            if tag.is_empty() || tag.contains(|c: char| c.is_whitespace() || c == ',') {
                return Err(format!("{}: 태그에 공백이나 쉼표를 넣지 않는다 — {tag:?}", self.id));
            }
        }
        for (what, v) in [("에픽", &self.epic), ("마일스톤", &self.milestone)] {
            if let Some(v) = v
                && !crate::id::is_valid(v)
            {
                return Err(format!("{}: {what} id 형식이 아니다 — {v:?}", self.id));
            }
        }
        Ok(())
    }
}

/// `.moai/journal.jsonl` 의 한 줄. **추가만 한다.**
///
/// 적는 것은 `create`·`status`·`note`·`rm` 넷뿐이다. 필드 변경을 적기
/// 시작하면 이 파일은 이벤트 로그가 되고, 그러면 "스냅샷 대신 이걸 접으면
/// 되지 않나" 가 반드시 돌아온다. **저널은 상태 계산에 읽히지 않는다.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// 맨 앞이라 `sort` 가 그대로 먹는다.
    pub ts: String,
    pub id: String,
    /// enum 이 아니다 — 모르는 kind 는 읽는 쪽이 건너뛴다. 안 읽혀도 상태가
    /// 안 틀리는 유일한 파일이라 여기만 관대해도 된다.
    pub kind: String,
    pub by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl JournalEntry {
    fn base(kind: &str, id: &str, at: &str, by: &str) -> JournalEntry {
        JournalEntry {
            ts: at.to_string(),
            id: id.to_string(),
            kind: kind.to_string(),
            by: by.to_string(),
            from: None,
            to: None,
            title: None,
            text: None,
            note: None,
        }
    }
    pub fn create(id: &str, title: &str, at: &str, by: &str) -> JournalEntry {
        JournalEntry { title: Some(title.to_string()), ..Self::base("create", id, at, by) }
    }
    pub fn status(id: &str, from: &Status, to: &Status, note: Option<String>, at: &str, by: &str) -> JournalEntry {
        JournalEntry {
            from: Some(from.0.clone()),
            to: Some(to.0.clone()),
            note,
            ..Self::base("status", id, at, by)
        }
    }
    pub fn note(id: &str, text: &str, at: &str, by: &str) -> JournalEntry {
        JournalEntry { text: Some(text.to_string()), ..Self::base("note", id, at, by) }
    }
    pub fn removed(id: &str, title: &str, at: &str, by: &str) -> JournalEntry {
        JournalEntry { title: Some(title.to_string()), ..Self::base("rm", id, at, by) }
    }
}

/// 누가 했는가. `MOAI_ACTOR` → `git config user.name` → `unknown`.
pub fn actor() -> String {
    if let Ok(a) = std::env::var("MOAI_ACTOR")
        && !a.trim().is_empty()
    {
        return a.trim().to_string();
    }
    let out = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    match out {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "unknown".into(),
    }
}

// ── 시각 ──────────────────────────────────────────────────────────────
//
// 타임스탬프는 `String` 이다. 구조체로 들고 있으면 라운드트립 바이트 동일성을
// 보장하기 어렵고(`Z` vs `+00:00`, 나노초) 멱등성 테스트가 그것부터 잡는다.
// RFC3339 UTC 고정폭이라 문자열 비교로 정렬이 맞는다.

/// 지금. `MOAI_NOW` 가 있으면 그것을 쓴다 — 시계를 고정해야 "3일 전" 을 시험한다.
pub fn now() -> String {
    if let Ok(t) = std::env::var("MOAI_NOW")
        && !t.trim().is_empty()
    {
        return t.trim().to_string();
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_rfc3339(secs)
}

/// 1970-01-01 부터의 날 수를 (년, 월, 일) 로. Howard Hinnant 의 `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn format_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-09-11T04:12:03Z` → epoch 초. 형식이 아니면 `None`.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0, 4)?, n(5, 7)? as u32, n(8, 10)? as u32);
    let (h, mi, se) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

/// `at` 이 `now` 로부터 며칠 전인가. 파싱이 안 되면 `None`.
pub fn days_since(at: &str, now: &str) -> Option<i64> {
    Some((parse_rfc3339(now)? - parse_rfc3339(at)?).div_euclid(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn issue() -> Issue {
        Issue::new(
            "argos-4aex".into(),
            "제목".into(),
            Kind::Issue,
            Status::new("todo"),
            "2026-09-11T04:12:03Z",
        )
    }

    /// 최소 형태는 필수 6개뿐이고 키 순서가 고정이다.
    #[test]
    fn minimal_line_omits_everything_optional() {
        let line = serde_json::to_string(&issue()).unwrap();
        assert_eq!(
            line,
            r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#
        );
    }

    #[test]
    fn full_line_keeps_declared_key_order() {
        let mut i = issue();
        i.kind = Kind::Epic;
        i.priority = Some(1);
        i.tags = vec!["bug".into()];
        i.assignee = Some("claude".into());
        i.epic = Some("argos-9k2p".into());
        i.milestone = Some("argos-m001".into());
        i.body = Some("본문".into());
        let line = serde_json::to_string(&i).unwrap();
        let want = [
            "id", "title", "kind", "status", "priority", "tags", "assignee", "epic",
            "milestone", "created_at", "updated_at", "status_since", "body",
        ];
        let at: Vec<usize> = want
            .iter()
            .map(|k| line.find(&format!("\"{k}\":")).unwrap_or_else(|| panic!("{k} 가 없다 — {line}")))
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{line}");
    }

    /// 읽고 그대로 쓰면 바이트가 같다. 이게 깨지면 매 명령이 헛 diff 를 만든다.
    #[test]
    fn round_trips_byte_identical() {
        for line in [
            r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
            r#"{"id":"argos-9k2p","title":"에픽","kind":"epic","status":"in_progress","priority":1,"tags":["bug"],"assignee":"claude","epic":"argos-0000","milestone":"argos-m001","created_at":"2026-09-10T09:00:00Z","updated_at":"2026-09-11T05:02:44Z","status_since":"2026-09-10T09:30:00Z","body":"여러\n줄"}"#,
        ] {
            let i: Issue = serde_json::from_str(line).unwrap();
            assert_eq!(serde_json::to_string(&i).unwrap(), line);
        }
    }

    /// 뒷 단계가 쓴 필드를 앞 단계 바이너리가 읽고 써도 잃지 않는다.
    ///
    /// 매 쓰기가 전체 재작성이라, 이게 없으면 새 바이너리가 쓴 필드를 옛
    /// 바이너리가 한 번 만지는 것만으로 1만 줄에서 지운다.
    #[test]
    fn unknown_fields_survive() {
        let line = r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z","due":"2026-10-01","estimate":90}"#;
        let i: Issue = serde_json::from_str(line).unwrap();
        assert_eq!(i.rest.len(), 2, "{:?}", i.rest);
        let out = serde_json::to_string(&i).unwrap();
        assert!(out.contains(r#""due":"2026-10-01""#), "{out}");
        assert!(out.contains(r#""estimate":90"#), "{out}");
    }

    /// 옛 바이너리가 2단계 종류를 만나도 줄을 버리지 않는다.
    #[test]
    fn reads_milestone_kind() {
        let i: Issue = serde_json::from_str(
            r#"{"id":"argos-4aex","title":"M1","kind":"milestone","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
        )
        .unwrap();
        assert_eq!(i.kind, Kind::Milestone);
    }

    /// 같은 개념이 대소문자로 갈라지지 않는다. 태그를 만드는 것이 주로
    /// 에이전트라 `Bug`·`bug` 를 섞어 쓴다.
    #[test]
    fn tags_fold_to_one_spelling() {
        assert_eq!(normalize_tag("  #Bug "), "bug");
        assert_eq!(normalize_tag("PARSER"), "parser");
        assert_eq!(normalize_tag("한글"), "한글");

        let mut i = issue();
        i.tags = vec!["Bug".into(), "bug".into(), "#BUG".into()];
        i.normalize();
        assert_eq!(i.tags, ["bug"], "한 개념이 둘로 갈라졌다");
    }

    #[test]
    fn kinds_round_trip() {
        assert_eq!("milestone".parse::<Kind>().unwrap(), Kind::Milestone);
        assert_eq!("epic".parse::<Kind>().unwrap(), Kind::Epic);
        assert!("없는것".parse::<Kind>().is_err());
        let i: Issue = serde_json::from_str(
            r#"{"id":"argos-4aex","title":"M1","kind":"milestone","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
        )
        .unwrap();
        assert_eq!(i.kind, Kind::Milestone);
    }

    #[test]
    fn normalize_sorts_tags_and_drops_defaults() {
        let mut i = issue();
        i.tags = vec!["z".into(), "a".into(), "z".into()];
        i.priority = Some(DEFAULT_PRIORITY);
        i.body = Some(String::new());
        i.normalize();
        assert_eq!(i.tags, ["a", "z"]);
        assert_eq!(i.priority, None);
        assert_eq!(i.body, None);
        assert_eq!(i.priority(), DEFAULT_PRIORITY);
    }

    #[test]
    fn validate_refuses_what_writing_must_not_accept() {
        let c = cfg();
        for (mutate, want) in [
            ((|i: &mut Issue| i.title = "  ".into()) as fn(&mut Issue), "제목이 비었다"),
            (|i| i.title = "두\n줄".into(), "한 줄"),
            (|i| i.id = "argos-4ae".into(), "id 형식"),
            (|i| i.status = Status::new("없는칸"), "라는 칸이 없다"),
            (|i| i.priority = Some(9), "우선순위는"),
            (|i| i.tags = vec!["두 낱말".into()], "공백이나 쉼표"),
            (|i| i.epic = Some("이상한".into()), "에픽 id 형식"),
        ] {
            let mut i = issue();
            mutate(&mut i);
            let e = i.validate(&c).unwrap_err();
            assert!(e.contains(want), "{want} 를 기대했는데 {e:?}");
        }
        assert!(issue().validate(&c).is_ok());
    }

    #[test]
    fn formats_and_parses_time() {
        for (secs, text) in [
            (0, "1970-01-01T00:00:00Z"),
            (1_789_084_800, "2026-09-11T00:00:00Z"),
            (1_789_099_923, "2026-09-11T04:12:03Z"),
        ] {
            assert_eq!(format_rfc3339(secs), text);
            assert_eq!(parse_rfc3339(text), Some(secs));
        }
        for bad in ["2026-09-11", "2026-09-11T04:12:03+09:00", "", "20260911T041203Z", "2026-13-11T04:12:03Z"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn counts_days() {
        assert_eq!(days_since("2026-09-08T00:00:00Z", "2026-09-11T04:12:03Z"), Some(3));
        assert_eq!(days_since("2026-09-11T04:12:03Z", "2026-09-11T23:59:59Z"), Some(0));
        assert_eq!(days_since("어제", "2026-09-11T04:12:03Z"), None);
    }

    /// 저널은 넷만 적는다. 필드 변경을 적기 시작하면 이벤트 로그가 된다.
    #[test]
    fn journal_entries_are_shaped() {
        let e = JournalEntry::status(
            "argos-4aex",
            &Status::new("todo"),
            &Status::new("in_progress"),
            None,
            "2026-09-11T05:02:44Z",
            "claude",
        );
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"ts":"2026-09-11T05:02:44Z","id":"argos-4aex","kind":"status","by":"claude","from":"todo","to":"in_progress"}"#
        );
    }
}
