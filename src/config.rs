//! `.moai/config.toml` 의 아주 작은 부분집합 파서와 설정 값.
//!
//! 중첩도 배열도 쓰지 않으므로 TOML 라이브러리를 넣지 않는다.
//! **엄격하게 읽는다** — 설정 오타가 조용히 통과하면 id 접두어가 틀어지거나
//! 상태 목록이 비고, 둘 다 되돌리기 어렵다.

use std::path::{Path, PathBuf};

/// 상태 목록을 안 적었을 때의 칸반 컬럼.
pub const DEFAULT_STATUSES: &str = "todo,in_progress,review,done";

/// 종결 상태. **설정으로 바꿀 수 없다.**
///
/// 코드가 상태에 묻는 질문은 사실상 이것 하나뿐이라(진행률·열린 이슈 수·방치
/// 검사) beads 식 상태 카테고리 표를 두지 않는다. 어휘가 둘이 되면 둘을 계속
/// 맞춰야 한다. 둘째 종결 상태가 실제로 필요해지면 그때 목록 하나를 더한다.
pub const DONE: &str = "done";

/// 따옴표 **밖의** `#` 부터가 주석이다. 값 안의 `#` 을 자르면
/// `title = "규칙 #1"` 이 조용히 깨진다.
///
/// 따옴표가 짝이 안 맞으면 어디까지가 값인지 알 수 없다. 세어서 넘기지 말고 거절한다.
fn strip_comment(line: &str, n: usize) -> Result<&str, Trouble> {
    let mut quoted = false;
    let mut cut = line.len();
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => {
                cut = i;
                break;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(Trouble::Unbalanced { line: n });
    }
    Ok(&line[..cut])
}

/// 값은 반드시 큰따옴표로 감싼다. `trim_matches` 로 벗기면
/// `prefix = "argos" 오타` 가 `argos" 오타` 로 조용히 통과한다.
fn value(raw: &str) -> Option<&str> {
    let inner = raw.trim().strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then_some(inner)
}

/// `키 = 값` 줄 하나의 **쓰인 그대로**. 따옴표는 안 벗긴다 — 글([`scalar`])과
/// 수([`count`]·[`days`]·[`ratio`])가 따옴표를 서로 반대로 요구하므로, 벗기는 일은 읽는 쪽이 한다.
///
/// `table` 은 그 줄 위의 마지막 `[…]` 머리다. 이 파서는 테이블을 안 읽으므로 그런 줄은 값으로
/// 안 쓰이는데, **안 쓰인다는 것을 아는 자리가 있어야** [`Thresholds::check_keys`] 가 그것을
/// 댈 수 있다 — 조용히 넘기면 `[status]` 밑에 적은 문턱이 영영 안 먹고 까닭이 어디에도 없다.
struct Entry<'s> {
    key: &'s str,
    value: &'s str,
    line: usize,
    table: Option<&'s str>,
}

/// 파일을 **한 번만** 훑어 줄을 다 모은다.
///
/// 키마다 다시 훑던 때는 `Config::parse` 한 번이 파일을 열두 번 지났다 — 문턱 여덟에
/// `scalar` 셋, 거기에 `check_keys` 가 한 번 더였다. 훑는 자가 하나라 주석·따옴표·테이블
/// 규칙도 한 곳에만 산다.
fn entries(src: &str) -> Result<Vec<Entry<'_>>, Trouble> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src); // BOM
    let mut out = Vec::new();
    let mut table = None;
    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let l = strip_comment(line, n)?.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(t) = l.strip_prefix('[') {
            // 모양이 어긋난 머리로 파일 전체를 거절하지 않는다 — 전에도 `[` 줄은 그냥 넘겼다.
            table = Some(t.strip_suffix(']').unwrap_or(t).trim());
            continue;
        }
        let (k, v) = l.split_once('=').ok_or(Trouble::NotAPair { line: n })?;
        out.push(Entry { key: k.trim(), value: v.trim(), line: n, table });
    }
    Ok(out)
}

/// 최상위 줄 하나를 찾는다. **테이블 안의 줄은 안 쓴다** — 이 파서는 테이블을 안 읽으므로
/// 거기 적힌 `prefix` 를 맨 위의 것으로 읽으면 안 적은 값을 적은 것으로 센다.
fn raw<'s>(es: &[Entry<'s>], key: &str) -> Option<(&'s str, usize)> {
    es.iter().find(|e| e.table.is_none() && e.key == key).map(|e| (e.value, e.line))
}

/// 최상위 `키 = "값"` 하나를 읽는다.
fn text(es: &[Entry<'_>], key: &str) -> Result<Option<String>, Trouble> {
    let Some((v, n)) = raw(es, key) else { return Ok(None) };
    let v = value(v).ok_or_else(|| Trouble::NotQuoted { line: n, key: key.into(), raw: v.into() })?;
    Ok(Some(v.to_string()))
}

/// 수는 **따옴표 없이** 적는다 — TOML 이 수를 적는 꼴이다. 언젠가 `toml` 크레이트로
/// 갈아 끼울 때 이미 쓴 설정 파일이 그대로 읽혀야 하는데, 따옴표를 두르면 그때 글이
/// 되어 그 파일만 조용히 안 읽힌다.
///
/// **못 읽은 수를 기본값으로 덮지 않는다.** 덮으면 고쳐 적은 값이 안 먹는 까닭을
/// 설정 파일만 보고는 못 찾는다 — 임계값을 파일로 뺀 뜻이 거기서 사라진다.
fn number<T: std::str::FromStr>(es: &[Entry<'_>], key: &str, want: Want) -> Result<Option<T>, Trouble> {
    let Some((v, n)) = raw(es, key) else { return Ok(None) };
    if v.starts_with('"') {
        return Err(Trouble::NumberQuoted { line: n, key: key.into(), raw: v.into() });
    }
    v.parse::<T>().map(Some).map_err(|_| Trouble::NotANumber { line: n, key: key.into(), want, raw: v.into() })
}

/// 날수. 음수를 막으려고 `u32` 로 읽고 넓힌다 — `-1` 이 통과하면 그 경고가 모든 줄에 선다.
fn days(es: &[Entry<'_>], key: &str, default: i64) -> Result<i64, Trouble> {
    Ok(number::<u32>(es, key, Want::Whole)?.map_or(default, i64::from))
}

/// 건수.
fn count(es: &[Entry<'_>], key: &str, default: usize) -> Result<usize, Trouble> {
    Ok(number::<usize>(es, key, Want::Whole)?.unwrap_or(default))
}

/// 비율. `0.15` 가 15% 다 — 백분율로 적지 않는다. 1 을 넘기면 그 경고가 영영 안 서는데,
/// 끄려는 뜻이었다면 그것은 임계값이 아니라 없는 손잡이다. 조용히 끄느니 거절한다.
fn ratio(es: &[Entry<'_>], key: &str, default: f64) -> Result<f64, Trouble> {
    let Some(v) = number::<f64>(es, key, Want::Fraction)? else { return Ok(default) };
    if !(0.0..=1.0).contains(&v) {
        return Err(Trouble::RatioRange { key: key.into(), value: v });
    }
    Ok(v)
}

/// 사람을 어떻게 낼까. `레이븐 (raven@buzzni.com)` 은 22칸이라 좁은 화면에서
/// 줄을 다 먹는다. 무엇이 읽기 좋은지는 저장소마다 다르고, 그 판단은 도구가
/// 아니라 그 저장소가 한다.
///
/// **저장은 이것과 무관하다.** 파일에는 언제나 이름과 메일이 갈라져 들어간다 —
/// 표기를 바꿨다고 이미 쓴 줄이 달라지면 그건 설정이 아니라 마이그레이션이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Naming {
    /// `이름 (메일)`
    #[default]
    Full,
    /// `이름`
    Name,
    /// `메일`. 메일을 모르는 사람은 이름으로 낸다 — 빈칸을 내면 그 줄이
    /// 누구의 것인지 화면에서 사라진다.
    Email,
}

impl Naming {
    pub const ALL: [&'static str; 3] = ["full", "name", "email"];

    fn parse(raw: &str) -> Option<Naming> {
        match raw {
            "full" => Some(Naming::Full),
            "name" => Some(Naming::Name),
            "email" => Some(Naming::Email),
            _ => None,
        }
    }
}

/// 접두어의 **모양** — 소문자·숫자·`-`, `-` 로 시작하거나 끝나지 않는다. 길이는 안 본다:
/// 그것은 새로 심을 때만 거는 규칙이다(`cmd::init::PREFIX_MAX`). 읽는 자리와 심는 자리가 같은
/// 규칙을 쓰도록 한 곳에 둔다 — 심는 자리가 길이를 먼저 보면 모양이 틀린 긴 접두어에 그 자체로
/// 틀린 짧은 후보를 댔다(리뷰 moai-f7xs.z1x).
pub fn check_prefix(prefix: &str) -> Result<(), Trouble> {
    if !prefix.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') {
        return Err(Trouble::PrefixCharset { raw: prefix.into() });
    }
    if prefix.starts_with('-') || prefix.ends_with('-') {
        return Err(Trouble::PrefixDash { raw: prefix.into() });
    }
    Ok(())
}

/// `moai status` 가 무엇부터 잔소리할지 정하는 수들.
///
/// **막는 것이 아니라 비추는 것이라 설정으로 둔다.** 종료 코드는 여기 어느 값으로도
/// 안 바뀐다 — 값을 낮춰 잔소리를 늘려도 게이트는 안 생긴다(CLAUDE.md, `moai status`).
///
/// 한때 `report.rs` 의 이름 붙인 상수였다. 그때 안 뺀 까닭은 "지금 설정 시스템을
/// 만들면 아무도 안 고치는 파일이 하나 늘 뿐" 이었고, 실제로 상수 일곱이 도입된 커밋
/// 뒤로 한 번도 안 바뀌었다(moai-pz7h 의 2026-09-11·09-12 실측). 저장소마다 벌여 놓는
/// 폭이 다르다는 것이 드러나 사람이 빼기로 정했다(2026-09-19).
///
/// **평평한 키로 둔다** — `[status]` 테이블로 적으면 [`raw`] 가 `[` 줄을 건너뛰어
/// `review_days` 가 최상위 키와 한 이름이 되고, 그것을 가르려면 이 파서가 테이블을
/// 알아야 한다. 테이블이 정말 필요해지는 날이 `toml` 크레이트를 넣는 날이다.
///
/// ```toml
/// status_review_days   = 3
/// status_wip_days      = 2
/// status_blocked_days  = 3
/// status_wip_limit     = 3
/// status_no_epic_ratio = 0.15
/// status_no_epic_min   = 5
/// status_flow_days     = 7
/// status_idea_pile     = 5
/// status_due_days      = 3
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// review 에 **이 날수를 넘겨** 머물면 썩는 것으로 본다(`d > review_days`).
    pub review_days: i64,
    /// 집어 놓고 이 날수를 넘겨 안 건드리면 잊은 것으로 본다.
    pub wip_days: i64,
    /// 막힌 채로 이 날수를 넘겨 서 있으면 "계획이 멈춘 자리" 로 본다.
    pub blocked_days: i64,
    /// 한 번에 이보다 많이 벌이면 알린다(`count > wip_limit`).
    pub wip_limit: usize,
    /// 에픽 없는 이슈가 **이 비율부터** 알린다(`ratio >= no_epic_ratio`).
    pub no_epic_ratio: f64,
    /// 비율이 낮아도 이 수부터 알린다(`count >= no_epic_min`).
    pub no_epic_min: usize,
    /// 흐름을 재는 창(일).
    pub flow_days: i64,
    /// 담아 둔 생각이 이만큼 쌓이면 알린다.
    pub idea_pile: usize,
    /// 마일스톤의 종료 기한이 **이 날수 안으로** 다가오면 알린다(`0 <= 남은 날수 <= due_days`,
    /// moai-tfcp). 지난 기한은 이 값과 무관하게 언제나 말한다 — 문턱은 "미리 한 번 비춘다" 의
    /// 폭이지 "늦은 것을 봐준다" 의 폭이 아니다.
    pub due_days: i64,
}

impl Default for Thresholds {
    fn default() -> Thresholds {
        Thresholds::DEFAULT
    }
}

impl Thresholds {
    /// 한 줄도 안 적은 저장소가 받는 값. **옛 상수 그대로다** — 설정으로 뺐다고
    /// 이미 도는 저장소의 경고가 달라지면 그건 설정이 아니라 마이그레이션이다.
    ///
    /// `const` 로 두어 시험이 상수 자리에서 그대로 쓴다 — 기본값을 시험마다 다시
    /// 적으면 기본값을 고칠 때 시험이 안 따라온다.
    pub const DEFAULT: Thresholds = Thresholds {
        review_days: 3,
        wip_days: 2,
        blocked_days: 3,
        wip_limit: 3,
        no_epic_ratio: 0.15,
        no_epic_min: 5,
        flow_days: 7,
        idea_pile: 5,
        due_days: 3,
    };

    /// 아는 키. **오타를 조용히 넘기지 않으려고 목록으로 든다** — `status_` 로 시작하는
    /// 모르는 키는 거절한다. 여느 모르는 키와 달리 여기서 엄한 까닭은, 이 값들이 고치고
    /// 나서 화면이 안 바뀌는 것으로만 확인되는 자리라서다: `status_reveiw_days = 1` 은
    /// 조용히 통과하면 영영 안 먹고, 왜 안 먹는지 설정 파일에는 아무 자취가 없다.
    pub const KEYS: [&'static str; 9] = [
        "status_review_days",
        "status_wip_days",
        "status_blocked_days",
        "status_wip_limit",
        "status_no_epic_ratio",
        "status_no_epic_min",
        "status_flow_days",
        "status_idea_pile",
        "status_due_days",
    ];

    fn parse(es: &[Entry<'_>]) -> Result<Thresholds, Trouble> {
        let d = Thresholds::DEFAULT;
        let t = Thresholds {
            review_days: days(es, "status_review_days", d.review_days)?,
            wip_days: days(es, "status_wip_days", d.wip_days)?,
            blocked_days: days(es, "status_blocked_days", d.blocked_days)?,
            wip_limit: count(es, "status_wip_limit", d.wip_limit)?,
            no_epic_ratio: ratio(es, "status_no_epic_ratio", d.no_epic_ratio)?,
            no_epic_min: count(es, "status_no_epic_min", d.no_epic_min)?,
            flow_days: days(es, "status_flow_days", d.flow_days)?,
            idea_pile: count(es, "status_idea_pile", d.idea_pile)?,
            due_days: days(es, "status_due_days", d.due_days)?,
        };
        // 흐름 창이 0 이면 `생성 0 · 완료 0` 이 서서 "아무 일도 없었다" 로 읽힌다.
        // 그것은 비추는 수를 끈 것이지 낮춘 것이 아니다.
        if t.flow_days == 0 {
            return Err(Trouble::FlowDaysZero);
        }
        Ok(t)
    }

    /// `status_` 로 시작하는데 아는 키가 아닌 줄과, **문턱을 맨 위가 아닌 자리에 적은 줄**을 댄다.
    ///
    /// 문턱은 평평한 키다. 그런데 TOML 로는 `[status]` 밑에 `review_days` 를 적는 것이 더
    /// 자연스러워서 실제로 그렇게 적히고, 이 파서는 테이블을 안 읽으므로 그 줄이 **조용히 안
    /// 먹는다** — 오타를 소리내는 것과 똑같은 까닭으로 이것도 소리내야 한다. 고치고 나서 화면이
    /// 안 바뀌는 것으로만 확인되는 자리라, 넘기면 왜 안 먹는지 설정 파일에 아무 자취가 없다.
    fn check_keys(es: &[Entry<'_>]) -> Result<(), Trouble> {
        for e in es {
            let flat = format!("status_{}", e.key);
            let known = Thresholds::KEYS.contains(&e.key);
            let bare = Thresholds::KEYS.contains(&flat.as_str());
            let named = match (e.table.is_some(), known, bare) {
                // 테이블 안에 적은 문턱 — 이 파서는 그 줄을 안 읽는다.
                (true, true, _) => e.key.to_string(),
                (true, _, true) | (false, _, true) => flat,
                (false, false, _) if e.key.starts_with("status_") => {
                    return Err(Trouble::NoSuchThreshold { line: e.line, key: e.key.to_string() });
                }
                _ => continue,
            };
            return Err(Trouble::ThresholdInTable { line: e.line, named });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// id 접두어. 그 저장소의 프로젝트명이다.
    pub prefix: String,
    /// 칸반 컬럼. 적은 순서가 곧 보드의 순서다.
    pub statuses: Vec<String>,
    /// 화면이 사람을 내는 모양. 파일에 쓰는 모양이 아니다.
    pub naming: Naming,
    /// `moai status` 의 잔소리 문턱. 아무것도 막지 않는다.
    pub status: Thresholds,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config, Refused> {
        let path = root.join(".moai/config.toml");
        let at = |why| Refused { at: path.clone(), why };
        let src = std::fs::read_to_string(&path).map_err(|e| at(Trouble::Unreadable { said: e.to_string() }))?;
        Config::parse(&src).map_err(at)
    }

    pub fn parse(src: &str) -> Result<Config, Trouble> {
        // **파일은 한 번만 훑는다**([`entries`]) — 키마다 다시 훑으면 키 수 × 줄 수다.
        let es = &entries(src)?;
        let prefix = text(es, "prefix")?.filter(|p| !p.is_empty()).ok_or(Trouble::NoPrefix)?;
        check_prefix(&prefix)?;

        let raw = text(es, "statuses")?.unwrap_or_else(|| DEFAULT_STATUSES.into());
        let statuses: Vec<String> = raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if statuses.is_empty() {
            return Err(Trouble::NoStatuses);
        }
        if !statuses.iter().any(|s| s == DONE) {
            return Err(Trouble::NoDone { raw });
        }
        if let Some(dup) = statuses.iter().enumerate().find(|(i, s)| statuses[..*i].contains(s)) {
            return Err(Trouble::StatusTwice { status: dup.1.clone() });
        }

        let naming = match text(es, "naming")? {
            None => Naming::default(),
            Some(raw) => Naming::parse(&raw).ok_or(Trouble::NamingUnknown { raw })?,
        };

        Thresholds::check_keys(es)?;
        let status = Thresholds::parse(es)?;

        Ok(Config { prefix, statuses, naming, status })
    }

    /// 새 이슈가 놓이는 칸. 목록의 첫 칸이다.
    pub fn first_status(&self) -> &str {
        &self.statuses[0]
    }

    /// 시작했으나 안 끝난 칸인가 — 첫 칸도 `done` 도 아니다. **"시작했다" 의 뜻은 여기
    /// 하나다**(moai-p415). `wip`·`wip_overload`·훅의 집기 판단·묶음이 저마다 "첫 칸도 done
    /// 도 아니다" 를 적고 있었고, 묶음만 [`Config::started_status`] 로 **자리를** 골라 칸이 더
    /// 있는 설정에서 멤버가 `in_progress` 인 에픽이 `blocked` 로 읽혔다.
    ///
    /// 설정에 없는 칸도 시작한 것으로 센다 — 읽기는 관대하다. 칸을 알아야 하는 쪽(훅)은
    /// [`Config::knows`] 를 따로 묻는다.
    pub fn is_started(&self, status: &str) -> bool {
        status != self.first_status() && status != DONE
    }

    /// 시작한 것이 놓이는 칸 — 첫 칸도 끝난 칸도 아닌 첫 칸. 그런 칸이 없는
    /// 두 칸짜리 설정이면 첫 칸이다: 거기서는 "시작했지만 안 끝났다" 를 말할
    /// 낱말이 없고, `done` 으로 말하면 안 끝난 것을 끝났다고 한다.
    ///
    /// **"시작했는가" 를 묻는 데 쓰지 않는다** — 그것은 [`Config::is_started`] 다. 이 칸은
    /// 묶음이 반쯤 끝났는데 시작한 멤버가 없을 때 설 자리를 댈 때만 쓴다.
    pub fn started_status(&self) -> &str {
        self.statuses
            .iter()
            .find(|s| *s != self.first_status() && *s != DONE)
            .map_or(self.first_status(), String::as_str)
    }

    pub fn knows(&self, status: &str) -> bool {
        self.statuses.iter().any(|s| s == status)
    }

    /// 모르는 칸을 거부한다. **문장이 여기 하나다** — `add`·`mv`·`show` 와
    /// `Issue::validate` 가 저마다 같은 말을 짓고 있으면 반드시 갈라진다.
    pub fn require_known(&self, status: &str) -> Result<(), NoSuchColumn> {
        if self.knows(status) {
            return Ok(());
        }
        Err(NoSuchColumn { name: status.to_string(), nor_rows: false, known: self.statuses.clone() })
    }
}

/// 설정을 읽다 멈춘 까닭 — **말이 아니라 자료다**(moai-ivt9, [`NoSuchColumn`] 과 같은 까닭).
/// 글은 [`crate::view::config_trouble`] 이 짓는다.
///
/// `.moai/config.toml` 은 **쓰기 경로가 지나는 자리다** — `store::Repo::rooted` 가 읽고, 훅이
/// 도구 호출마다 그 길로 든다. 거기서 글을 지으면 찾기의 서명에 화면 말이 번지고, 말을 모른 채
/// 도는 자리(머지 드라이버·한눈 보기의 줄마다 열기)까지 사용자 설정을 열게 된다 — moai-iq7j 가
/// `store::Trouble` 로 그은 금이고 여기가 그 금의 이쪽이다.
#[derive(Debug, Clone, PartialEq)]
pub enum Trouble {
    /// 파일을 못 읽었다 — io 가 낸 말. **이미 글이다**(운영체제의 것이라 안 옮긴다).
    Unreadable { said: String },
    /// 따옴표가 짝이 안 맞는다 — 그 줄.
    Unbalanced { line: usize },
    /// `키 = "값"` 꼴이 아니다 — 그 줄.
    NotAPair { line: usize },
    /// 글인데 큰따옴표가 없다 — 그 줄·키·쓰인 그대로.
    NotQuoted { line: usize, key: String, raw: String },
    /// 수인데 따옴표를 둘렀다 — 그 줄·키·쓰인 그대로.
    NumberQuoted { line: usize, key: String, raw: String },
    /// 수를 못 읽었다 — 그 줄·키·바라는 꼴·쓰인 그대로.
    NotANumber { line: usize, key: String, want: Want, raw: String },
    /// 비율이 0 과 1 밖이다 — 키와 읽힌 값.
    RatioRange { key: String, value: f64 },
    /// `prefix` 에 못 쓰는 글자가 들었다 — 쓰인 그대로.
    PrefixCharset { raw: String },
    /// `prefix` 가 `-` 로 시작하거나 끝난다 — 쓰인 그대로.
    PrefixDash { raw: String },
    /// 흐름 창이 0 이다. 비추는 수를 끈 것이지 낮춘 것이 아니다.
    FlowDaysZero,
    /// `status_` 로 시작하는데 없는 설정이다 — 그 줄·키. **아는 키는 [`Thresholds::KEYS`] 가 댄다.**
    NoSuchThreshold { line: usize, key: String },
    /// 문턱을 테이블 안에 적었다 — 그 줄과, 맨 위에 적을 평평한 이름.
    ThresholdInTable { line: usize, named: String },
    /// `prefix` 가 없다.
    NoPrefix,
    /// `statuses` 가 비었다.
    NoStatuses,
    /// `statuses` 에 `done` 이 없다 — 쓰인 그대로.
    NoDone { raw: String },
    /// 한 칸이 두 번 적혔다 — 그 칸.
    StatusTwice { status: String },
    /// `naming` 이 모르는 값이다 — 쓰인 그대로.
    NamingUnknown { raw: String },
}

/// 수가 어떤 꼴이어야 하는가([`Trouble::NotANumber`]). **낱말이 아니라 갈래로 든다** —
/// 고르는 자리([`number`])는 화면 말을 모른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    /// `0` 이상의 정수.
    Whole,
    /// `0` 과 `1` 사이의 소수.
    Fraction,
}

/// 어느 파일을 읽다 멈췄는가([`Config::load`]). 파싱만 하는 [`Config::parse`] 는 자리를 모르므로
/// [`Trouble`] 만 낸다 — 자리를 붙이는 자가 하나여야 같은 까닭이 두 모양으로 서지 않는다.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub at: PathBuf,
    pub why: Trouble,
}

/// 모르는 칸([`Config::require_known`]·[`crate::cmd::unknown_column`]) — **말이 아니라
/// 자료다**(moai-fdk7, `user_config::ConfigTrouble` 과 같은 까닭). 글은
/// [`crate::view::no_such_column`] 이 짓는다.
///
/// **아는 칸 목록을 함께 든다.** 펴는 쪽이 `Config` 를 다시 들 까닭이 없어지고, 탐색기의
/// 거름망 줄처럼 설정에서 먼 자리도 같은 글을 짓는다.
///
/// **저장 계층은 화면 말을 모른 채 둔다**(사람이 정했다, 2026-09-20). `Issue::validate` 도
/// 이 자료를 받지만 거기서는 이웃 검사 열셋과 같은 말로 편다 — 파일을 재는 글과 사람이
/// 보는 글은 다른 것이고, `store::with_write` 에 화면 말을 물려주지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoSuchColumn {
    /// 사람이 적은 칸 이름.
    pub name: String,
    /// **그 칸에 선 줄도 없다.** 줄까지 보고 거절한 자리(`--from`·거름망·`show -s`)에서 서고,
    /// 설정만 보고 거절한 자리(`add -s`·`mv <칸>`)에서는 안 선다 — 두 거절은 잰 것이 달라
    /// 같은 말을 하면 안 된다(moai-hym7).
    pub nor_rows: bool,
    /// 그 저장소가 아는 칸, 적힌 차례 그대로.
    pub known: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 안 적으면 `full` 이다. 옛 저장소의 설정 파일에는 이 키가 없다.
    #[test]
    fn naming_defaults_to_full() {
        assert_eq!(Config::parse("prefix = \"argos\"\n").unwrap().naming, Naming::Full);
    }

    /// 오타를 조용히 통과시키면 왜 표기가 안 바뀌는지 아무도 못 찾는다.
    ///
    /// **낱말이 아니라 갈래로 잰다**(moai-ivt9) — 글은 말묶음으로 갔고, 고른 말에 따라
    /// 달라진다. 무엇을 댈지는 `view::tests::config_trouble_names_what_it_needs` 가 잰다.
    #[test]
    fn a_misspelled_naming_is_refused() {
        for raw in ["Full", "이름", "", "name,email"] {
            let src = format!("prefix = \"argos\"\nnaming = \"{raw}\"\n");
            let e = Config::parse(&src).unwrap_err();
            assert_eq!(e, Trouble::NamingUnknown { raw: raw.to_string() }, "{raw:?}");
        }
        assert_eq!(Config::parse("prefix = \"argos\"\nnaming = \"email\"\n").unwrap().naming, Naming::Email);
    }

    /// 글 하나를 원본에서 바로 읽는다 — 줄 모으기([`entries`])와 글 읽기([`text`])를 한 번에
    /// 지나는 시험용 길이다. 도는 코드는 `Config::parse` 가 줄을 한 번 모아 나눠 쓴다.
    fn scalar(src: &str, key: &str) -> Result<Option<String>, Trouble> {
        text(&entries(src)?, key)
    }

    #[test]
    fn reads_a_quoted_scalar() {
        assert_eq!(scalar("prefix = \"argos\"\n", "prefix").unwrap().as_deref(), Some("argos"));
        assert_eq!(scalar("\u{feff}prefix = \"argos\"\n", "prefix").unwrap().as_deref(), Some("argos"));
        assert_eq!(
            scalar("# 주석\nprefix = \"ai-argos\"  # 뒤 주석\n", "prefix").unwrap().as_deref(),
            Some("ai-argos")
        );
        assert_eq!(scalar("other = \"x\"\n", "prefix").unwrap(), None);
    }

    /// 값 안의 `#` 은 주석이 아니다.
    #[test]
    fn hash_inside_quotes_survives() {
        assert_eq!(scalar("t = \"규칙 #1\"\n", "t").unwrap().as_deref(), Some("규칙 #1"));
    }

    #[test]
    fn refuses_sloppy_values() {
        let quoted = |raw: &str| Trouble::NotQuoted { line: 1, key: "prefix".into(), raw: raw.into() };
        for (src, want) in [
            ("prefix = argos\n", quoted("argos")),
            ("prefix = \"argos\" 오타\n", quoted("\"argos\" 오타")),
            ("prefix = \"argos\n", Trouble::Unbalanced { line: 1 }),
            ("prefix\n", Trouble::NotAPair { line: 1 }),
        ] {
            assert_eq!(scalar(src, "prefix").unwrap_err(), want, "{src:?}");
        }
    }

    #[test]
    fn defaults_the_kanban_columns() {
        let c = Config::parse("prefix = \"argos\"\n").unwrap();
        assert_eq!(c.statuses, ["todo", "in_progress", "review", "done"]);
        assert_eq!(c.first_status(), "todo");
        assert!(c.knows("done") && !c.knows("blocked"));
    }

    /// 시작한 칸은 첫 칸도 끝도 아닌 첫 칸이다. 그런 칸이 없으면 첫 칸 —
    /// 두 칸짜리 설정에서 `done` 으로 말하면 안 끝난 것을 끝났다고 한다.
    #[test]
    fn the_started_column_is_neither_first_nor_done() {
        let started = |cols: &str| {
            let c = Config::parse(&format!("prefix = \"a\"\nstatuses = \"{cols}\"\n")).unwrap();
            c.started_status().to_string()
        };
        assert_eq!(started("todo,in_progress,review,done"), "in_progress");
        assert_eq!(started("todo,done,doing"), "doing");
        assert_eq!(started("todo,done"), "todo");
    }

    #[test]
    fn extra_columns_are_allowed() {
        let c = Config::parse("prefix = \"a\"\nstatuses = \"todo, blocked, done\"\n").unwrap();
        assert_eq!(c.statuses, ["todo", "blocked", "done"]);
    }

    /// **시작했다는 판단은 자리가 아니라 뜻으로 한다**(moai-p415) — 첫 칸도 `done` 도 아니면
    /// 시작했다. 두 칸짜리 설정에는 시작한 칸이 없다.
    #[test]
    fn started_is_neither_first_nor_done() {
        let c = Config::parse("prefix = \"a\"\nstatuses = \"todo, blocked, in_progress, review, done\"\n").unwrap();
        let started: Vec<&str> = c.statuses.iter().map(String::as_str).filter(|s| c.is_started(s)).collect();
        assert_eq!(started, ["blocked", "in_progress", "review"]);
        let two = Config::parse("prefix = \"a\"\nstatuses = \"todo, done\"\n").unwrap();
        assert!(!two.is_started("todo") && !two.is_started("done"));
    }

    /// 한 줄도 안 적은 저장소는 옛 상수를 그대로 받는다. 설정으로 뺀 것이
    /// 이미 도는 저장소의 경고를 바꾸면 그건 설정이 아니라 마이그레이션이다.
    #[test]
    fn thresholds_default_to_the_old_constants() {
        let c = Config::parse("prefix = \"argos\"\n").unwrap();
        assert_eq!(c.status, Thresholds::DEFAULT);
        assert_eq!((c.status.review_days, c.status.wip_days, c.status.blocked_days), (3, 2, 3));
        assert_eq!((c.status.wip_limit, c.status.no_epic_min, c.status.idea_pile), (3, 5, 5));
        assert_eq!((c.status.no_epic_ratio, c.status.flow_days, c.status.due_days), (0.15, 7, 3));
    }

    /// 적은 값이 그대로 선다 — 아홉 키를 한 번에 본다. 하나를 빼먹고 기본값으로
    /// 두면 그 키만 고쳐도 안 먹는데, 화면에는 아무 자취가 없다.
    ///
    /// **목록이 [`Thresholds::KEYS`] 와 같은 길이인지도 여기서 잰다**(리뷰) — 키를 더하면서
    /// 이 시험을 안 고치면 그 키는 아무 데서도 안 읽히는 채로 초록이다. `status_due_days` 가
    /// 실제로 그렇게 들어왔다.
    #[test]
    fn every_threshold_can_be_set() {
        let src = "prefix = \"argos\"
status_review_days   = 10
status_wip_days      = 11
status_blocked_days  = 12
status_wip_limit     = 13
status_no_epic_ratio = 0.5
status_no_epic_min   = 14
status_flow_days     = 15
status_idea_pile     = 16
status_due_days      = 17
";
        assert_eq!(src.lines().skip(1).count(), Thresholds::KEYS.len(), "아는 키 하나가 이 시험에 안 섰다");
        let t = Config::parse(src).unwrap().status;
        assert_eq!((t.review_days, t.wip_days, t.blocked_days, t.flow_days), (10, 11, 12, 15));
        assert_eq!((t.wip_limit, t.no_epic_min, t.idea_pile), (13, 14, 16));
        assert_eq!(t.no_epic_ratio, 0.5);
        assert_eq!(t.due_days, 17, "status_due_days 가 안 먹는다");
    }

    /// 수는 따옴표 없이 적는다. 두르면 `toml` 크레이트로 갈아 끼우는 날 글이 되므로,
    /// 그때 조용히 안 읽히느니 지금 거절한다.
    #[test]
    fn a_quoted_number_is_refused() {
        let e = Config::parse("prefix = \"a\"\nstatus_wip_limit = \"3\"\n").unwrap_err();
        assert_eq!(e, Trouble::NumberQuoted { line: 2, key: "status_wip_limit".into(), raw: "\"3\"".into() });
    }

    /// **오타가 조용히 통과하면 안 먹는 까닭을 설정 파일만 보고는 못 찾는다.**
    /// 임계값은 고친 뒤에 화면이 안 바뀌는 것으로만 확인되는 자리다.
    #[test]
    fn a_misspelled_threshold_key_is_refused() {
        let e = Config::parse("prefix = \"a\"\nstatus_reveiw_days = 1\n").unwrap_err();
        // 아는 키 목록은 [`Thresholds::KEYS`] 에서 펴는 쪽이 달고, `status_review_days` 가 그 안에 있다.
        assert_eq!(e, Trouble::NoSuchThreshold { line: 2, key: "status_reveiw_days".into() });
        assert!(Thresholds::KEYS.contains(&"status_review_days"));
        // 주석 안의 오타는 오타가 아니다.
        Config::parse("prefix = \"a\"\n# status_reveiw_days = 1\n").unwrap();
        // `statuses` 는 `status_` 로 시작하지 않는다 — 칸 목록을 오타로 읽으면 안 된다.
        Config::parse("prefix = \"a\"\nstatuses = \"todo,done\"\n").unwrap();
    }

    /// 못 읽는 수를 기본값으로 덮지 않는다 — 덮으면 고친 값이 안 먹는다.
    #[test]
    fn refuses_a_threshold_that_is_not_a_number() {
        let whole =
            |key: &str, raw: &str| Trouble::NotANumber { line: 2, key: key.into(), want: Want::Whole, raw: raw.into() };
        for (src, want) in [
            ("status_wip_limit = 셋\n", whole("status_wip_limit", "셋")),
            ("status_review_days = -1\n", whole("status_review_days", "-1")),
            ("status_review_days = 1.5\n", whole("status_review_days", "1.5")),
            ("status_no_epic_ratio = 15\n", Trouble::RatioRange { key: "status_no_epic_ratio".into(), value: 15.0 }),
            ("status_no_epic_ratio = -0.1\n", Trouble::RatioRange { key: "status_no_epic_ratio".into(), value: -0.1 }),
            // 흐름 창이 0 이면 `생성 0 · 완료 0` 이 서서 "아무 일도 없었다" 로 읽힌다.
            ("status_flow_days = 0\n", Trouble::FlowDaysZero),
        ] {
            assert_eq!(Config::parse(&format!("prefix = \"a\"\n{src}")).unwrap_err(), want, "{src:?}");
        }
    }

    /// 0 은 끄는 것이 아니라 낮추는 것이다 — 문턱이 0 이면 한 건부터 선다.
    #[test]
    fn zero_is_a_lower_threshold_not_a_switch() {
        let c = Config::parse("prefix = \"a\"\nstatus_review_days = 0\nstatus_wip_limit = 0\n").unwrap();
        assert_eq!((c.status.review_days, c.status.wip_limit), (0, 0));
    }

    /// **`[status]` 테이블은 조용히 안 먹는 자리다.** 이 파서는 테이블을 안 읽는데 TOML 로는
    /// 그 꼴이 더 자연스러워 실제로 그렇게 적힌다 — 오타를 소리내는 것과 똑같은 까닭으로
    /// 이것도 소리내야 한다. 고친 뒤 화면이 안 바뀌는 것으로만 확인되는 자리라서다.
    #[test]
    fn a_threshold_written_under_a_table_is_refused() {
        for (src, line) in [
            ("prefix = \"a\"\n[status]\nreview_days = 1\n", 3),
            ("prefix = \"a\"\n[status]\nstatus_review_days = 1\n", 3),
            // 맨 위에 접두어 없이 적은 것도 같다 — 그 이름의 설정은 없다.
            ("prefix = \"a\"\nreview_days = 1\n", 2),
        ] {
            let e = Config::parse(src).unwrap_err();
            assert_eq!(e, Trouble::ThresholdInTable { line, named: "status_review_days".into() }, "{src:?}");
        }
        // 문턱이 아닌 키는 테이블 안에 있어도 그대로 넘긴다 — 모르는 키는 여전히 자유다.
        Config::parse("prefix = \"a\"\n[아무거나]\nfoo = \"x\"\n").unwrap();
    }

    /// **테이블 안의 줄을 맨 위의 것으로 읽지 않는다.** 읽으면 안 적은 접두어가 적힌 것이 된다.
    #[test]
    fn a_key_inside_a_table_is_not_a_top_level_key() {
        let e = Config::parse("[아무거나]\nprefix = \"argos\"\n").unwrap_err();
        assert_eq!(e, Trouble::NoPrefix);
    }

    #[test]
    fn refuses_broken_config() {
        for (src, want) in [
            ("statuses = \"todo,done\"\n", Trouble::NoPrefix),
            ("prefix = \"Argos\"\n", Trouble::PrefixCharset { raw: "Argos".into() }),
            ("prefix = \"-a\"\n", Trouble::PrefixDash { raw: "-a".into() }),
            ("prefix = \"a\"\nstatuses = \"todo,review\"\n", Trouble::NoDone { raw: "todo,review".into() }),
            ("prefix = \"a\"\nstatuses = \" , \"\n", Trouble::NoStatuses),
            ("prefix = \"a\"\nstatuses = \"todo,todo,done\"\n", Trouble::StatusTwice { status: "todo".into() }),
        ] {
            assert_eq!(Config::parse(src).unwrap_err(), want, "{src:?}");
        }
    }
}
