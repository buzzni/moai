//! 시간대 — **화면에만** 건다(moai-v6vo).
//!
//! 저널의 `ts` 와 스냅샷의 `created_at`·`updated_at` 은 RFC3339 UTC 고정폭이고 그대로 남는다.
//! `--json` 도 그대로다. 바꾸는 것은 사람이 읽는 글자뿐이다 — `i18n` 이 `kind` 를 안 건드리고
//! `said` 만 옮긴 것과 같은 줄이고, "설정은 화면만 바꾼다. 설정이 이미 쓴 줄을 바꾸면 그건
//! 설정이 아니라 마이그레이션이다" 와도 같다.
//!
//! **사람이 치는 날도 이 자로 읽는다** — 마일스톤 기한(moai-h2th)과 날로 친 때 거르개(`query::End::Wall`,
//! moai-efoc)는 읽는 사람의 날로 잰다. 적힌 값은 그대로지만, `show --created <날> --json` 이 **어느 줄을**
//! 내는지는 시간대를 따른다.
//!
//! **크레이트를 안 들인다**(2026-09-21 사용자 결정). 길이 둘이었다 — 시스템의
//! `/usr/share/zoneinfo` 를 읽거나, 크레이트가 tzdb 를 박아 넣거나. 바이너리 15MB 예산과
//! `ratatui` 의 달력 위젯이 `time` 을 끌고 와 일부러 기본 기능을 껐던 판단을 그대로 두는 쪽을
//! 골랐다. 대가는 받은 기계에 zoneinfo 가 없을 수 있다는 것이고, 그때는 UTC 로 떨어지고 한 줄로
//! 알린다([`Trouble`], moai-77ap) — **읽기는 관대하다**.
//!
//! **조각이다.** 설정도 화면도 모른다. 어느 이름을 쓸지는 부르는 쪽이 정하고, 못 풀었다는 말은
//! [`crate::view`] 가 짓는다.

use std::path::{Path, PathBuf};

/// tzdb 가 사는 곳. **환경 변수로 갈아 끼운다** — 시험이 제 자료를 대고 돌 수 있어야 한다.
/// (`TZDIR` 은 glibc 가 이미 쓰는 이름이다.)
fn zoneinfo() -> PathBuf {
    std::env::var_os("TZDIR").map_or_else(|| PathBuf::from("/usr/share/zoneinfo"), PathBuf::from)
}

/// 시간대를 못 풀었을 때의 까닭. **말이 아니라 자료다** — 화면 말을 짓는 것은 `view` 고, 여기는
/// 무엇이 없었는지만 든다(`store::Trouble` 과 같은 결).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// tzdb 가 아예 없다 — 정적 musl 판을 알파인·scratch 에 받은 자리다.
    NoTzdb { at: PathBuf },
    /// 그 이름의 자료가 없고 POSIX 규칙 글로도 안 읽힌다 — 오타이거나, 이 기계의 tzdb 가 그 이름을 모른다.
    Unknown { name: String },
    /// 자료는 있는데 못 읽는다. `said` 는 운영체제가 낸 글이다.
    Unreadable { name: String, said: String },
    /// 읽었는데 TZif 가 아니다 — tzdb 디렉터리에 함께 사는 `leapseconds`·`zone.tab` 같은 파일이거나 잘린 파일이다.
    /// **까닭을 글로 싣지 않는다**(리뷰 moai-efoc.3e1) — `Unreadable` 의 `said` 에 한국어 글을 싣던 판은 영어 화면에도
    /// 그 글을 냈다. 말은 [`crate::view::zone_trouble`] 이 짓는다.
    NotTzif { name: String },
    /// 시스템이 어느 시간대인지를 말해 주지 않는다 — `/etc/localtime` 이 이름을 안 댄다.
    NoSystemZone,
}

/// 시간대 하나 — 이름과, 그 이름이 언제부터 얼마를 더하는가.
///
/// **UTC 는 자료 없이 선다**([`Zone::utc`]) — tzdb 가 없는 기계에서도 늘 답이 있어야 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    name: String,
    /// (그때부터, UTC 에 더할 초). 차례대로 오름차순이다.
    shifts: Vec<(i64, i32)>,
    /// 첫 전환보다 앞선 때에 더할 초. 전환이 하나도 없고 규칙이 있으면 안 쓴다 — 그때는 규칙이 모든 때를 잰다.
    before: i32,
    /// 마지막 전환보다 뒤를 재는 규칙 — 파일 꼬리(footer)의 POSIX TZ 글([`Rule`], moai-r621). 규칙이 마지막 전환
    /// 뒤에 처음 바꾸는 때부터 잰다([`Zone::offset_at`]). 없거나 못 읽으면 마지막 전환의 값이 뒤로 이어진다.
    rule: Option<Rule>,
}

impl Default for Zone {
    fn default() -> Self {
        Zone::utc()
    }
}

impl Zone {
    /// 저장된 그대로 — 아무것도 안 더한다. **이름이 `UTC` 다**: 빈 이름으로 두면 화면이 "시간대를
    /// 못 골랐다" 와 "UTC 를 골랐다" 를 못 가린다.
    pub fn utc() -> Zone {
        Zone { name: "UTC".into(), shifts: Vec::new(), before: 0, rule: None }
    }

    /// 저장된 그대로 — 빌려 쓰는 [`Zone::utc`]. **시간대를 안 얹은 화면이 이것을 든다**
    /// (`view::Screen::zone`): 안 얹었다는 것은 지어낸 답이 아니라 *파일에 적힌 값 그대로*라는
    /// 뜻이고, 그 값은 어느 판에서나 같아 한 벌만 있으면 된다.
    pub fn stored() -> &'static Zone {
        static UTC: std::sync::OnceLock<Zone> = std::sync::OnceLock::new();
        UTC.get_or_init(Zone::utc)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// 내내 같은 만큼 더하는 시간대 — **시험만 든다**(moai-h2th).
    ///
    /// [`Zone::load`] 는 기계의 tzdb 를 보므로, 시간대를 재는 시험이 그것으로 서면 zoneinfo 없는
    /// 기계에서 조용히 아무것도 안 재게 된다(`if let Ok(seoul)` 로 통째로 건너뛴다). 재려는 것이
    /// "옮긴 뒤의 답" 이지 "tzdb 를 읽는 법" 이 아닌 자리에서는 자료 없이 옮기는 자가 낫다.
    /// 여름 시간이 없는 자리(`Asia/Seoul` 이 그렇다)는 이 꼴이 실제 자료와 같은 답을 낸다.
    /// **0 은 안 받는다**(리뷰) — 이 꼴은 전환도 꼬리 규칙도 없어 [`Zone::is_utc`] 가 `before` 하나로 가르므로
    /// `fixed(.., 0)` 은 이름이 무엇이든 UTC 고, [`Zone::shift`] 가 그 자리에서 되돌아간다. 시간대를 재려고 쓴
    /// 시험이 그런 시간대를 들면 아무것도 안 재면서 푸르게 지나간다.
    #[cfg(test)]
    pub fn fixed(name: &str, secs: i32) -> Zone {
        assert_ne!(secs, 0, "옮기지 않는 시간대로는 옮긴 답을 못 잰다 — UTC 는 Zone::utc 다");
        Zone { name: name.to_string(), shifts: Vec::new(), before: secs, rule: None }
    }

    /// UTC 인가 — 화면이 이것으로 "그대로 둔다" 를 가른다.
    ///
    /// **꼬리의 규칙도 본다**(moai-r621). 전환이 하나도 없는 파일은 규칙이 있으면 그것이, 없으면 `before` 가 모든
    /// 때를 잰다 — [`Zone::offset_at`] 과 같은 갈래다. `before` 만 보면 규칙만 든 파일이 UTC 로 읽혀
    /// [`Zone::shift`] 가 그 자리에서 되돌아간다.
    pub fn is_utc(&self) -> bool {
        self.shifts.is_empty() && self.rule.as_ref().map_or(self.before == 0, Rule::is_utc)
    }

    /// 이름으로 연다. `UTC` 는 자료를 안 본다 — tzdb 가 없는 기계에서도 서야 한다.
    ///
    /// **tzdb 로 못 연 이름은 POSIX 규칙 글로 읽는다**(moai-btxt.1gk) — `JST-9`·`<+09>-9`·`CST6CDT,M3.2.0,M11.1.0`.
    /// glibc 의 차례(파일 먼저, 규칙 다음)와 같아서 `EST5EDT` 처럼 둘 다인 이름은 tzdb 의 것이 선다. 그 시간대의
    /// 이름은 **받은 글 그대로**다(2026-10-01 사용자 결정) — 설정에 적어도 같은 자로 다시 열린다. 규칙 글은 자료가
    /// 필요 없어 tzdb 가 없는 기계(정적 musl 판)에서도 선다. 규칙으로도 못 읽으면 tzdb 의 까닭을 그대로 낸다.
    ///
    /// **tzdb 가 낸 까닭을 가리지 않는다**(리뷰 moai-btxt.0ln) — glibc 는 파일을 못 열면 까닭이 무엇이든 규칙으로
    /// 읽는다. 없는 이름(`Unknown`)만 넘기던 판은 tzdb 디렉터리에 권한이 없는 샌드박스에서 `TZ=JST-9` 를
    /// `Unreadable` 로 떨궜다. 규칙 글이 아닌 이름은 규칙으로도 안 풀려 tzdb 의 까닭이 그대로 선다.
    pub fn load(name: &str) -> Result<Zone, Trouble> {
        if name == "UTC" {
            return Ok(Zone::utc());
        }
        Zone::from_tzdb(name).or_else(|why| Zone::from_rule(name).ok_or(why))
    }

    /// POSIX 규칙 글 하나로 선 시간대 — 전환이 없고 모든 때를 규칙이 잰다.
    fn from_rule(text: &str) -> Option<Zone> {
        let rule = Rule::parse(text)?;
        Some(Zone { name: text.to_string(), shifts: Vec::new(), before: rule.std, rule: Some(rule) })
    }

    fn from_tzdb(name: &str) -> Result<Zone, Trouble> {
        let dir = zoneinfo();
        if !dir.is_dir() {
            return Err(Trouble::NoTzdb { at: dir });
        }
        let at = safe_join(&dir, name).ok_or_else(|| Trouble::Unknown { name: name.to_string() })?;
        let raw = std::fs::read(&at).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Trouble::Unknown { name: name.to_string() },
            _ => Trouble::Unreadable { name: name.to_string(), said: e.to_string() },
        })?;
        let (shifts, before, rule) = parse(&raw).ok_or_else(|| Trouble::NotTzif { name: name.to_string() })?;
        Ok(Zone { name: name.to_string(), shifts, before, rule })
    }

    /// 이 기계가 선 시간대. **`TZ` 가 먼저다** — 그것으로 한 번만 다르게 보는 길이 사람에게
    /// 이미 익은 자다. 다음이 `/etc/localtime` 의 심볼릭 링크가 대는 이름이다.
    ///
    /// 이름을 못 얻으면 UTC 와 까닭을 함께 낸다 — **막지 않는다**.
    pub fn system() -> (Zone, Option<Trouble>) {
        match system_name() {
            None => (Zone::utc(), Some(Trouble::NoSystemZone)),
            Some(name) => match Zone::load(&name) {
                Ok(z) => (z, None),
                Err(why) => (Zone::utc(), Some(why)),
            },
        }
    }

    /// 그때 UTC 에 더할 초.
    ///
    /// **마지막 전환 뒤는 꼬리의 규칙이 잰다**(RFC 8536 3.3, moai-r621) — 다만 규칙이 그 전환 **뒤에** 처음
    /// 바꾸는 때부터다. 그 사이는 마지막 전환의 값이 선다. tzcode 의 `localtime.c` 가 꼬리로 지은 전환 가운데
    /// 마지막 전환보다 뒤의 것만 잇는 것과 같은 자다. 규약은 꼬리가 마지막 전환과 같은 답을 내야 한다고 하지만
    /// 2023c 까지의 zic(glibc 2.39 가 싣는 것도)이 지은 slim 파일이 그것을 어긴다: `America/Ojinaga` 는
    /// 2022-10-30 에 CST 로 옮긴 것이 마지막 전환인데 꼬리(`CST6CDT,M3.2.0,M11.1.0`)는 11-06 까지 CDT 다.
    /// 규칙을 그 순간부터 대던 판은 그 한 주를 한 시간 앞선 시각으로 그렸다(리뷰 moai-efoc.3e1). 규약대로 지은
    /// 파일에서는 두 자의 답이 같다.
    ///
    /// 전환이 하나도 없으면 모든 때를 규칙이 잰다.
    fn offset_at(&self, secs: i64) -> i32 {
        let n = self.shifts.partition_point(|(at, _)| *at <= secs);
        if n == self.shifts.len()
            && let Some(rule) = &self.rule
        {
            let Some(&(last, _)) = self.shifts.last() else { return rule.offset_at(secs) };
            if let Some((at, offset)) = rule.last_change(secs)
                && at > i128::from(last)
            {
                return offset;
            }
        }
        match n {
            0 => self.before,
            n => self.shifts[n - 1].1,
        }
    }

    /// 그 순간(epoch 초)의 **이 시간대의 벽시계** — epoch 초 꼴로(moai-efoc). [`Zone::shift`] 가 글자로 짓는
    /// 값을 수로 낸다: 날로 거르는 자(`query::spans_hold`)가 줄마다 부르므로 글을 짓고 다시 풀 까닭이 없다.
    /// **셈은 여기 하나다** — `shift` 도 이것을 부른다. 화면이 대는 날과 거르개가 잡는 날이 한 식에서 나와야
    /// `--created <날>` 이 상세가 댄 날의 줄을 잡는다(리뷰 moai-efoc.ln9).
    pub fn local(&self, secs: i64) -> i64 {
        secs + i64::from(self.offset_at(secs))
    }

    /// `2026-09-21T08:24:58Z` → 이 시간대의 같은 순간(`2026-09-21T17:24:58Z` 꼴).
    ///
    /// **꼴을 안 바꾼다** — 받은 그대로의 RFC3339 를 내므로 [`crate::view::stamp`] 가 하던
    /// 글자 자르기가 그대로 선다. 못 읽은 글은 **그대로 돌려준다**: 읽기는 관대하고, 못 읽은
    /// 것을 지어내면 그 줄이 언제인지를 잃는다.
    pub fn shift(&self, at: &str) -> String {
        if self.is_utc() {
            return at.to_string();
        }
        match crate::model::parse_rfc3339(at) {
            None => at.to_string(),
            Some(secs) => crate::model::format_rfc3339(self.local(secs)),
        }
    }
}

/// `/usr/share/zoneinfo` 밑의 이름인가 — `..` 과 절대 경로를 막는다. 이름은 설정 파일과 고르는
/// 창에서 오므로, 그대로 이어 붙이면 아무 파일이나 TZif 로 읽으려 든다.
fn safe_join(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.starts_with('/') {
        return None;
    }
    let mut at = dir.to_path_buf();
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        at.push(part);
    }
    Some(at)
}

/// 시스템이 대는 이름. `TZ` 가 먼저고, 그다음이 `/etc/localtime` 이 가리키는 자리다.
fn system_name() -> Option<String> {
    if let Some(name) = env_name() {
        return Some(name);
    }
    let link = std::fs::read_link("/etc/localtime").ok()?;
    let dir = zoneinfo();
    // 링크가 tzdb 안을 가리키면 그 아래 경로가 곧 이름이다. 밖을 가리키면 이름을 모른다 —
    // 자료는 읽을 수 있어도 **무엇이라 불러야 할지**를 모르므로 설정에 적을 수 없다.
    name_under(&link, &dir)
        .or_else(|| name_under(&link, Path::new("/usr/share/zoneinfo")))
        // **글자로 못 맞추면 그때만 파일 시스템에 묻는다**(리뷰) — 링크가 또 링크이거나
        // (`/etc/localtime` → `/etc/zoneinfo/…`), tzdb 디렉터리 자체가 링크인 기계가 있다
        // (macOS 의 `/usr/share/zoneinfo` → `/var/db/timezone/zoneinfo`). 거기서 포기하면 화면이
        // UTC 로 서면서 **매 명령**에 "시스템이 제 시간대를 안 댄다" 가 붙는다.
        .or_else(|| {
            let real = std::fs::canonicalize("/etc/localtime").ok()?;
            [std::fs::canonicalize(&dir).ok(), std::fs::canonicalize("/usr/share/zoneinfo").ok()]
                .into_iter()
                .flatten()
                .find_map(|root| name_under(&real, &root))
        })
}

/// `TZ` 가 대는 이름. 없거나 tzdb 밖을 가리키는 경로면 `None` 이고, 그때는 `/etc/localtime` 이 답한다.
fn env_name() -> Option<String> {
    tz_name(&std::env::var_os("TZ")?)
}

/// `TZ` 의 값을 이름으로 — [`env_name`] 의 셈. 환경을 안 읽어야 시험이 꼴마다 잰다.
fn tz_name(value: &std::ffi::OsStr) -> Option<String> {
    // `TZ=:Asia/Seoul` 처럼 콜론을 다는 꼴이 있다(POSIX).
    let raw = value.to_string_lossy().trim_start_matches(':').to_string();
    // **빈 `TZ` 는 UTC 다**(POSIX) — 설정해 두고 비운 것은 "여기는 UTC 로 보겠다" 는 말이지
    // "안 정했다" 가 아니다. 안 받고 `/etc/localtime` 으로 내려가면 같은 기계의 다른 도구와
    // 시각이 갈린다.
    if raw.is_empty() {
        return Some("UTC".into());
    }
    // 경로로 준 꼴(`TZ=:/etc/localtime`·`TZ=/usr/share/zoneinfo/Asia/Seoul`)은 링크와 **같은 자**로
    // 푼다(리뷰). 이름으로 넘기면 `safe_join` 이 절대 경로를 막아 `Unknown` 이 되고, 그렇게 둔
    // 기계에서는 매 명령에 "이 시간대를 모른다" 가 붙는다. tzdb 밖을 가리키면 이름을 모르는
    // 것이라 다음 자리(`/etc/localtime`)로 내려간다.
    if raw.starts_with('/') {
        let at = PathBuf::from(&raw);
        return name_under(&at, &zoneinfo()).or_else(|| name_under(&at, Path::new("/usr/share/zoneinfo")));
    }
    // **그 밖은 다 이름으로 넘긴다** — `TZ=<+09>-9` 같은 POSIX 규칙 글도 [`Zone::load`] 가 tzdb 다음에
    // 읽는다(moai-btxt.1gk). 한때 이름 꼴의 글자만 받아, 꺾쇠가 든 규칙 글은 `/etc/localtime` 으로 조용히
    // 내려갔고 `JST-9` 는 tzdb 에서 못 찾아 UTC 로 떨어졌다. 둘 다 아닌 글은 `load` 가 못 풀어 UTC 로
    // 떨어지며 한 줄로 알린다 — glibc 도 못 읽는 `TZ` 를 UTC 로 읽는다.
    Some(raw)
}

/// `at` 이 `dir` 밑을 가리키면 그 아래 경로가 곧 시간대 이름이다.
///
/// **상대 링크를 푼다**(리뷰). systemd 의 `timedatectl set-timezone` 은 `/etc/localtime` 을
/// `../usr/share/zoneinfo/<이름>` 으로 건다 — 절대로 거는 배포판도 있어 둘 다 산다. 상대인 것을
/// 그대로 잘라 내려 하면 어느 접두어와도 안 맞아 이름을 못 얻고, 그 기계에서는 화면이 UTC 로
/// 서면서 매 명령에 "시스템이 제 시간대를 안 댄다" 가 붙는다 — 이 기능이 고치려던 바로 그 자리다.
///
/// **접는 것은 글자로만 한다** — 파일 시스템에 안 묻는다([`crate::path::lexical`]). 물어야 하는
/// 자리(링크의 링크, 링크인 tzdb 디렉터리)는 부르는 쪽이 `canonicalize` 로 한 번 더 댄다.
fn name_under(at: &Path, dir: &Path) -> Option<String> {
    // **절대도 접는다**(리뷰). 한쪽만 접던 판은 `TZ=/usr/share/zoneinfo/../zoneinfo/Asia/Seoul`
    // 이나 `..` 이 든 절대 링크에서 `../zoneinfo/Asia/Seoul` 을 **이름이라고** 냈다 —
    // `safe_join` 이 `..` 을 막아 그 이름은 못 읽히고, 그 기계는 UTC 로 서면서 매 명령에 "이
    // 시간대를 모른다" 를 단다. 이 함수가 고치려던 바로 그 자리다.
    let full = match at.is_absolute() {
        true => crate::path::lexical(at),
        // `/etc/localtime` 의 링크라 기준은 `/etc` 다.
        false => crate::path::lexical(&Path::new("/etc").join(at)),
    };
    let name = full.strip_prefix(dir).ok()?.to_str()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// 화면이 설 시간대와, **그때 할 말 하나**. 고른 이름이 있으면 그것이 답이고, 없으면 시스템이다
/// ([`Zone::system`]).
///
/// **까닭은 화면이 실제로 선 시계의 것 하나다**(리뷰). 시스템을 못 풀었어도 고른 이름이 서면
/// 시스템 쪽 까닭은 할 말이 아니다 — 화면은 그 이름으로 서는데 "시각은 UTC 로 선다" 가 나란히
/// 붙으면 둘 중 어느 쪽을 믿을지 사람이 정해야 한다. `/etc/localtime` 을 상대 링크로 거는 기계
/// (systemd 의 `timedatectl`)가 흔해서 실제로 겹치는 자리다.
///
/// 못 푼 이름은 UTC 로 떨어지고 **그 이름의** 까닭을 댄다 — 막지 않는다(moai-77ap).
pub fn chosen(picked: Option<&str>) -> (Zone, Option<Trouble>) {
    match picked {
        None => Zone::system(),
        Some(name) => match Zone::load(name) {
            Ok(z) => (z, None),
            Err(why) => (Zone::utc(), Some(why)),
        },
    }
}

/// 이 기계가 아는 이름 전부 — 고르는 창(`SPC o t`)이 읽는다. 차례는 이름순이다.
///
/// **TZif 인 파일만 든다.** tzdb 디렉터리에는 `zone.tab`·`leapseconds` 처럼 시간대가 아닌 것이
/// 함께 산다. 확장자로 거르면 그 목록이 판마다 달라지므로 파일 머리 넉 자를 본다.
///
/// **`posix/`·`right/` 는 뺀다** — 뿌리의 이름과 같은 자료를 한 벌씩 더 담은 것이라, 들이면
/// 목록이 세 배가 되면서 고를 것은 하나도 안 는다.
///
/// tzdb 가 없으면 **빈 목록**이다. 까닭을 함께 낸다 — 빈 목록만 내면 고르는 창이 "시간대가
/// 없다" 로 서서 무엇이 잘못됐는지 아무 데도 안 적힌다.
pub fn names() -> (Vec<String>, Option<Trouble>) {
    let dir = zoneinfo();
    if !dir.is_dir() {
        return (Vec::new(), Some(Trouble::NoTzdb { at: dir }));
    }
    let mut out = Vec::new();
    walk(&dir, &dir, &mut out, DEEP);
    out.sort();
    out.dedup();
    (out, None)
}

/// 몇 층까지 내려가나. tzdb 가 가장 깊은 자리는 두 층이고(`America/Argentina/Buenos_Aires`),
/// 나머지는 여유다. **바닥이 있어야 하는 까닭은 링크다**(리뷰) — `at.is_dir()` 은 심볼릭 링크를
/// 따라가므로, tzdb 안에 제 위를 가리키는 링크가 하나 있으면 끝없이 내려가다 스택이 터진다.
/// 이 자리는 `TZDIR` 로 갈아 끼울 수 있어 남이 지은 디렉터리일 수도 있다.
const DEEP: usize = 8;

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>, left: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let at = e.path();
        let Ok(rel) = at.strip_prefix(root) else { continue };
        let Some(name) = rel.to_str() else { continue };
        if name == "posix" || name == "right" {
            continue;
        }
        // `file_type` 은 심볼릭 링크를 안 따라간다 — 링크도 자료라 열어 본다.
        match at.is_dir() {
            true if left > 0 => walk(&at, root, out, left - 1),
            true => {}
            false if is_tzif(&at) => out.push(name.to_string()),
            false => {}
        }
    }
}

fn is_tzif(at: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 4];
    std::fs::File::open(at).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"TZif"
}

/// TZif 를 푼다 — (전환, 첫 전환 앞의 오프셋, 꼬리의 규칙). 모양이 아니면 `None`.
///
/// **판 2 이상이면 뒤 자료를 읽는다.** 앞의 판 1 자료는 32비트 시각이라 2038년에 끊긴다. 판 1
/// 짜리 파일(요즘은 거의 없다)은 앞 자료를 그대로 쓰고, 꼬리가 없다.
///
/// **꼬리(footer)의 POSIX 규칙 글(`EST5EDT,M3.2.0,M11.1.0`)을 읽는다**(moai-r621). 마지막 전환보다 뒤를 그
/// 규칙이 잰다. 한때 안 읽었다 — tzdb 가 앞으로 수십 년어치 전환을 담는다고 봤는데, `zic -b fat` 으로 지은
/// 파일도 2037년까지만 적어 그 뒤는 꼬리만 잰다. 2020b 부터 zic 의 기본인 `slim` 은 규칙으로 잴 수 있는 전환을
/// 아예 안 적어, New York 의 마지막 전환이 2007년이다. 안 읽던 판은 2026년 1월의 줄을 서머타임으로 그렸고,
/// 날로 친 거르개(`--created <날>`)가 어느 줄을 내는지도 그 어긋남을 따랐다.
///
/// **꼬리를 못 읽어도 파일은 선다** — 읽기는 관대하다. 그때는 옛 판처럼 마지막 전환의 값이 뒤로 이어진다.
fn parse(raw: &[u8]) -> Option<(Vec<(i64, i32)>, i32, Option<Rule>)> {
    let (v1, version) = head(raw, 0, 4)?;
    if version < b'2' {
        let (shifts, before) = block(raw, &v1)?;
        return Some((shifts, before, None));
    }
    // 판 1 자료를 건너뛴 자리에 판 2 머리가 다시 선다.
    let (v2, _) = head(raw, v1.after + v1.size, 8)?;
    let (shifts, before) = block(raw, &v2)?;
    Some((shifts, before, footer(raw, v2.after + v2.size)))
}

/// 판 2 자료 뒤의 꼬리 — 줄바꿈 둘 사이의 POSIX TZ 글. 비었으면(규약이 허락한다) 규칙이 없다.
fn footer(raw: &[u8], at: usize) -> Option<Rule> {
    let tail = raw.get(at..)?.strip_prefix(b"\n")?;
    let end = tail.iter().position(|b| *b == b'\n')?;
    Rule::parse(std::str::from_utf8(&tail[..end]).ok()?)
}

/// TZif 머리글의 셈들. [`block`] 이 읽는 것만 든다.
struct Head {
    timecnt: usize,
    typecnt: usize,
    /// 전환 시각 하나의 크기 — 판 1 자료는 4, 판 2 자료는 8. **[`head`] 가 `size` 를 셀 때 쓴 값 그대로 든다**
    /// (리뷰 moai-btxt.0ln): [`block`] 이 따로 받으면 그 값과 어긋날 수 있고, 그때는 파일 안이라고 확인해 둔
    /// `size` 가 [`block`] 이 읽는 범위를 묶지 못한다.
    time: usize,
    /// 머리글 바로 뒤 — 자료가 시작하는 자리.
    after: usize,
    /// 이 머리글이 낸 자료 덩어리의 바이트 수. **파일 안에 다 든다** — [`head`] 가 재 두었다.
    size: usize,
}

/// `at` 의 머리글을 읽는다. `time` 은 그 자료의 전환 시각 하나의 크기(판 1 은 4, 판 2 는 8)다.
///
/// **자료 덩어리가 파일 안에 다 들어야 머리글로 받는다**(moai-btxt.xia). 셈 여섯은 남이 지은 파일(`TZDIR`)이
/// 대는 32비트 수라, 곱해 더한 크기가 `usize` 가 32비트인 기계에서 넘칠 수 있다 — 그 크기는 `u64` 로 재고,
/// 파일 길이를 넘으면 여기서 돌려보낸다. 그러면 [`block`] 과 [`parse`] 가 이 셈으로 짓는 자리는 모두 파일
/// 길이 안이라 어느 기계에서도 안 넘친다. 뒤쪽(윤초·표준/UT 표시)은 안 읽지만 그 자리가 잘렸으면 못 받는다 —
/// 판 1 파일도, 판 2 자료도 그렇다(판 2 는 한때 꼬리만 없는 것으로 읽었다). tzcode 와 glibc 도 그런 파일을 안 받는다.
fn head(raw: &[u8], at: usize, time: usize) -> Option<(Head, u8)> {
    let b = raw.get(at..at + 44)?;
    if &b[..4] != b"TZif" {
        return None;
    }
    let n = |i: usize| u64::from(u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]));
    let (isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt) = (n(20), n(24), n(28), n(32), n(36), n(40));
    // 셈 하나가 2³² 아래이고 곱하는 수가 열둘 아래라, 여섯을 더해도 `u64` 에 넉넉히 든다.
    let t = time as u64;
    let size = timecnt * (t + 1) + typecnt * 6 + charcnt + leapcnt * (t + 4) + isstdcnt + isutcnt;
    let after = at + 44;
    let size = usize::try_from(size).ok().filter(|size| *size <= raw.len() - after)?;
    // 파일 안에 드는 크기보다 작은 셈이라 `usize` 로 옮겨도 잃는 것이 없다.
    Some((Head { timecnt: timecnt as usize, typecnt: typecnt as usize, time, after, size }, b[4]))
}

/// 머리글 `h` 가 낸 자료 덩어리를 읽는다 — 자리와 시각의 크기는 그 머리글이 든 값이다.
fn block(raw: &[u8], h: &Head) -> Option<(Vec<(i64, i32)>, i32)> {
    let (at, time) = (h.after, h.time);
    if h.typecnt == 0 {
        return None;
    }
    let times = raw.get(at..at + h.timecnt * time)?;
    let kinds = raw.get(at + h.timecnt * time..at + h.timecnt * (time + 1))?;
    let infos_at = at + h.timecnt * (time + 1);
    let infos = raw.get(infos_at..infos_at + h.typecnt * 6)?;
    // ttinfo 하나는 여섯 바이트 — 오프셋 넷, 서머타임 하나, 이름 첫 자 하나.
    let offset = |k: usize| -> Option<i32> {
        let i = infos.get(k * 6..k * 6 + 4)?;
        Some(i32::from_be_bytes([i[0], i[1], i[2], i[3]]))
    };
    let is_dst = |k: usize| infos.get(k * 6 + 4).is_some_and(|d| *d != 0);
    // **첫 전환 앞은 서머타임 아닌 첫 종류다**(TZif 규약). 없으면 0번이다.
    let before = offset((0..h.typecnt).find(|k| !is_dst(*k)).unwrap_or(0))?;
    let mut shifts = Vec::with_capacity(h.timecnt);
    for i in 0..h.timecnt {
        let t = &times[i * time..(i + 1) * time];
        let secs = match time {
            4 => i64::from(i32::from_be_bytes([t[0], t[1], t[2], t[3]])),
            _ => i64::from_be_bytes([t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7]]),
        };
        let k = usize::from(*kinds.get(i)?);
        shifts.push((secs, offset(k)?));
    }
    // 차례가 어긋난 파일은 이분 탐색이 엉뚱한 답을 낸다 — 못 믿을 자료로 읽는다.
    if shifts.windows(2).any(|w| w[0].0 > w[1].0) {
        return None;
    }
    Some((shifts, before))
}

/// 마지막 전환 뒤를 재는 규칙 — TZif 꼬리의 POSIX TZ 글을 푼 것(moai-r621).
///
/// `EST5EDT,M3.2.0,M11.1.0` 은 "표준시는 UTC-5, 서머타임은 UTC-4, 3월 둘째 일요일 02:00 에 들어가 11월 첫
/// 일요일 02:00 에 나온다" 다. **푼 값은 UTC 에 더할 초다** — POSIX 글은 부호가 반대라(`EST5` 가 -5h) 푸는
/// 자리에서 한 번 뒤집는다.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    /// 표준시에 UTC 에 더할 초.
    std: i32,
    /// 서머타임. 없으면 표준시가 내내 이어진다.
    dst: Option<Dst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dst {
    /// 서머타임에 UTC 에 더할 초.
    offset: i32,
    /// 들어가는 날과 그날의 벽시계 초. **표준시로 잰다** — 바뀌기 직전의 벽시계다(POSIX).
    start: (Day, i32),
    /// 나오는 날과 그날의 벽시계 초. **서머타임으로 잰다.**
    end: (Day, i32),
}

/// 규칙의 날 하나. 셋 다 그해의 날을 가리킨다 — 평년의 `365` 만은 이듬해 1월 1일이다(glibc 와 같다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Day {
    /// `Jn` — 1..=365. 2월 29일을 안 센다(윤년에도 `J60` 은 3월 1일이다).
    Julian(u16),
    /// `n` — 0..=365. 2월 29일도 센다.
    Ordinal(u16),
    /// `Mm.w.d` — m 월의 w 째 d 요일. w 가 5 면 그달의 마지막 d 요일이고, d 는 0 이 일요일이다.
    Month { m: u8, w: u8, d: u8 },
}

impl Rule {
    /// 꼴이 아니면 `None` — 부르는 쪽은 규칙이 없는 것으로 읽는다.
    ///
    /// **서머타임 이름만 있고 날이 없는 글(`EST5EDT`)은 안 받는다.** 그때의 날은 POSIX 가 구현에 맡긴
    /// 자리라, 지어서 채우면 기계마다 다른 답을 낸다. zic 는 그런 꼬리를 안 쓴다.
    fn parse(s: &str) -> Option<Rule> {
        let mut p = Posix { s: s.as_bytes(), at: 0 };
        p.name()?;
        let std = -p.offset()?;
        if p.done() {
            return Some(Rule { std, dst: None });
        }
        p.name()?;
        // 서머타임의 오프셋을 빼면 표준시보다 한 시간 앞이다(POSIX).
        let offset = match p.peek() {
            Some(b',') => std + 3600,
            _ => -p.offset()?,
        };
        p.eat(b',')?;
        let start = p.when()?;
        p.eat(b',')?;
        let end = p.when()?;
        p.done().then_some(Rule { std, dst: Some(Dst { offset, start, end }) })
    }

    fn is_utc(&self) -> bool {
        self.std == 0 && self.dst.is_none()
    }

    /// 그때 UTC 에 더할 초.
    fn offset_at(&self, secs: i64) -> i32 {
        self.last_change(secs).map_or(self.std, |(_, offset)| offset)
    }

    /// 그 순간까지 온 마지막 전환 — (그 순간, 그때부터 UTC 에 더할 초). 서머타임이 없으면 전환도 없다.
    ///
    /// **두 해 앞부터 한 해 뒤까지의 전환을 늘어놓고, 그 순간까지 온 마지막 것을 본다.** 남반구처럼 서머타임이
    /// 해를 넘겨 걸치는 자리도, 전환 시각이 해 끝을 넘는 자리(`J365/25`, RFC 8536 의 늘 서머타임)도 한 식으로
    /// 선다. 두 해 앞까지 보는 까닭은 시각이 167시까지 가서다(RFC 8536 3.3.1) — 앞 해의 두 전환이 다 이 해로
    /// 넘어오면 그때 선 것은 두 해 앞의 전환이다(`J365/167,J365/100`, 리뷰 moai-efoc.3e1). 한 해의 전환은 그해
    /// 앞뒤 여드레 남짓 안에 서므로 세 해 앞이나 두 해 뒤는 답이 될 수 없다. 두 전환이 한 순간이면 들어가는
    /// 쪽을 뒤로 친다. 셈은 `i128` 로 한다: 받는 초가 `i64` 끝이어도 날 수에 86,400 을 곱하다 넘치지 않는다.
    ///
    /// **이듬해의 들어감에 닿은 나옴은 없는 전환이다**(moai-btxt.49f). `J1/0,J365/26` 처럼 서머타임이 한 해 넘게
    /// 걸리면 두 해의 서머타임이 겹친다 — 그 나옴을 전환으로 치던 판은 한 해 대부분을 표준시로 그렸다. tzcode 는
    /// 나옴이 한 해 넘게 뒤인 해에 전환을 안 지어 늘 서머타임으로 읽고, 이 셈도 그쪽을 따른다. glibc 는 해마다
    /// UTC 의 그해 안에서만 들어감과 나옴을 견줘, 1월 1일 0시(UTC)부터 그해의 들어감까지(`J1/0` 이면 세 시간)를
    /// 표준시로 그린다(리뷰 moai-btxt.0ln, Python `time.localtime` 으로 확인했다). 앞 해의 서머타임이 아직 안
    /// 끝난 때라 그 세 시간도 tzcode 를 따른다. 겹침은 **해마다** 가린다: 서수 날(`n`)은 2월 29일을 세어 같은
    /// 규칙이 평년에만 겹칠 수 있고, 그때 윤년의 나옴은 그대로 선다(glibc 와 같다). 한 순간에 맞물린 나옴
    /// (`J365/25`, RFC 8536 의 늘 서머타임)도 이 갈래다. 해를 넘겨 걸친 남반구는 나옴이 그해의 들어감보다 앞이라
    /// 안 걸린다.
    fn last_change(&self, secs: i64) -> Option<(i128, i32)> {
        let dst = self.dst.as_ref()?;
        let year = crate::model::civil_from_days(secs.div_euclid(86_400)).0;
        let moment = |y: i64, (day, time): (Day, i32), before: i32| {
            i128::from(day.in_year(y)) * 86_400 + i128::from(time) - i128::from(before)
        };
        let start = |y: i64| moment(y, dst.start, self.std);
        // `(때, 들어가는가)` 의 최댓값이 마지막 전환이다 — `true` 가 `false` 보다 커서 한 순간이면 들어가는 쪽이 이긴다.
        let (at, on) = (year - 2..=year + 1)
            .flat_map(|y| {
                let end = Some(moment(y, dst.end, dst.offset)).filter(|end| *end < start(y + 1));
                [Some((start(y), true)), end.map(|end| (end, false))]
            })
            .flatten()
            .filter(|&(at, _)| at <= i128::from(secs))
            .max()?;
        Some((at, if on { dst.offset } else { self.std }))
    }
}

impl Day {
    /// 그해의 이 날 — epoch 일로.
    fn in_year(self, y: i64) -> i64 {
        use crate::model::{days_from_civil, days_in_month};
        let jan1 = days_from_civil(y, 1, 1);
        match self {
            Day::Julian(n) => jan1 + i64::from(n) - 1 + i64::from(n >= 60 && days_in_month(y, 2) == 29),
            Day::Ordinal(n) => jan1 + i64::from(n),
            Day::Month { m, w, d } => {
                let first = days_from_civil(y, u32::from(m), 1);
                // 1970-01-01 은 목요일이다.
                let weekday = (first + 4).rem_euclid(7);
                let mut day = (i64::from(d) - weekday).rem_euclid(7) + 7 * (i64::from(w) - 1);
                // 다섯째가 없는 달은 넷째가 마지막이다.
                if day >= i64::from(days_in_month(y, u32::from(m))) {
                    day -= 7;
                }
                first + day
            }
        }
    }
}

/// POSIX TZ 글을 앞에서부터 읽는 자.
struct Posix<'a> {
    s: &'a [u8],
    at: usize,
}

impl Posix<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn done(&self) -> bool {
        self.at == self.s.len()
    }

    fn eat(&mut self, c: u8) -> Option<()> {
        (self.peek()? == c).then(|| self.at += 1)
    }

    /// 이름 — `EST` 처럼 글자 셋 이상이거나, `<+0330>` 처럼 꺾쇠에 싼 것. 이름은 쓰지 않고 건너뛴다.
    fn name(&mut self) -> Option<()> {
        if self.eat(b'<').is_some() {
            let len = self.s[self.at..].iter().position(|c| *c == b'>')?;
            self.at += len + 1;
            return (len > 0).then_some(());
        }
        let len = self.s[self.at..].iter().take_while(|c| c.is_ascii_alphabetic()).count();
        self.at += len;
        (len >= 3).then_some(())
    }

    /// `[+-]hh[:mm[:ss]]` — 부호를 붙인 초. 시는 `hours` 까지 받는다.
    fn hms(&mut self, hours: i64) -> Option<i32> {
        let sign = if self.eat(b'-').is_some() {
            -1
        } else {
            self.eat(b'+');
            1
        };
        let mut secs = self.num(0..=hours)? * 3600;
        for unit in [60, 1] {
            if self.eat(b':').is_none() {
                break;
            }
            secs += self.num(0..=59)? * unit;
        }
        i32::try_from(sign * secs).ok()
    }

    /// 오프셋. POSIX 는 시를 24 까지 둔다.
    fn offset(&mut self) -> Option<i32> {
        self.hms(24)
    }

    /// 날과, `/` 뒤의 시각(없으면 02:00). **시각은 부호가 붙고 167 시까지 간다** — RFC 8536 3.3.1 이
    /// POSIX 를 넓힌 자리다(`M3.5.0/-1`, 늘 서머타임의 `J365/25`).
    fn when(&mut self) -> Option<(Day, i32)> {
        // 자리마다 받는 폭을 [`Posix::num`] 에 건넨다 — 폭 안의 값이라 좁히는 `as` 는 잃는 것이 없다.
        let day = if self.eat(b'J').is_some() {
            Day::Julian(self.num(1..=365)? as u16)
        } else if self.eat(b'M').is_some() {
            let m = self.num(1..=12)? as u8;
            self.eat(b'.')?;
            let w = self.num(1..=5)? as u8;
            self.eat(b'.')?;
            Day::Month { m, w, d: self.num(0..=6)? as u8 }
        } else {
            Day::Ordinal(self.num(0..=365)? as u16)
        };
        let time = match self.eat(b'/') {
            Some(()) => self.hms(167)?,
            None => 2 * 3600,
        };
        Some((day, time))
    }

    /// 숫자 하나. 없거나 `range` 밖이면 `None`.
    fn num(&mut self, range: std::ops::RangeInclusive<i64>) -> Option<i64> {
        let len = self.s[self.at..].iter().take_while(|c| c.is_ascii_digit()).count();
        let n = std::str::from_utf8(&self.s[self.at..self.at + len]).ok()?.parse::<i64>().ok()?;
        self.at += len;
        range.contains(&n).then_some(n)
    }
}

/// 이 기계의 시간대를 **처음 읽을 때 푼다**(moai-s3i7). 명령 층이 [`crate::view::Screen::at`] 에
/// 이것을 얹고, 화면이 시각을 실제로 그릴 때에만 [`System::zone`] 이 tzdb 를 만진다.
///
/// 옛 자리는 화면을 지을 때 시간대를 풀어 얹었다. 그래서 `ready`·`prime`·`show`(목록)·
/// `backlog ls` 처럼 시각을 한 줄도 안 그리는 명령도 zoneinfo 없는 기계(정적 musl 판, moai-77ap)
/// 에서 [`System::trouble`] 한 줄을 stderr 에 냈다. 알림은 **드는 자리가 아니라 그리는 자리**
/// 에서 선다 — 푼 적 없는 판은 할 말이 없다.
#[derive(Default)]
pub struct System(std::sync::OnceLock<(Zone, Option<Trouble>)>);

impl System {
    /// 이 기계의 시간대. 처음 부를 때 [`Zone::system`] 으로 한 번 푼다.
    pub fn zone(&self) -> &Zone {
        &self.0.get_or_init(Zone::system).0
    }

    /// 풀다 만난 것. **[`System::zone`] 을 부른 판에서만 선다** — 안 읽은 판은 `None` 이다.
    pub fn trouble(&self) -> Option<&Trouble> {
        self.0.get().and_then(|(_, why)| why.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UTC 는 **자료 없이** 선다 — tzdb 가 없는 기계(정적 musl 판을 알파인에 받은 자리)에서도
    /// 늘 답이 있어야 한다. 그때 글자는 저장된 그대로다.
    #[test]
    fn utc_needs_no_data_and_changes_nothing() {
        let z = Zone::utc();
        assert!(z.is_utc());
        assert_eq!(z.name(), "UTC");
        assert_eq!(z.shift("2026-09-21T08:24:58Z"), "2026-09-21T08:24:58Z");
        // 이름으로 열어도 자료를 안 본다 — `TZDIR` 이 없는 자리를 가리켜도 선다.
        assert_eq!(Zone::load("UTC"), Ok(Zone::utc()));
    }

    /// **못 읽은 글은 그대로 돌려준다.** 읽기는 관대하고, 지어낸 시각은 그 줄이 언제인지를 잃는다.
    #[test]
    fn a_stamp_it_cannot_read_comes_back_untouched() {
        let z = Zone { name: "T".into(), shifts: Vec::new(), before: 9 * 3600, rule: None };
        for odd in ["", "언제", "2026-09-21", "2026-09-21T08:24:58+09:00"] {
            assert_eq!(z.shift(odd), odd, "못 읽은 글을 건드렸다");
        }
    }

    /// 첫 전환 앞과 뒤, 그리고 전환 바로 그 순간.
    #[test]
    fn the_offset_follows_the_transitions() {
        let z = Zone { name: "T".into(), shifts: vec![(100, 3600), (200, 7200)], before: 0, rule: None };
        assert_eq!(z.offset_at(0), 0, "첫 전환 앞이 첫 전환의 값을 썼다");
        assert_eq!(z.offset_at(99), 0);
        assert_eq!(z.offset_at(100), 3600, "전환 그 순간부터 새 값이다");
        assert_eq!(z.offset_at(150), 3600);
        assert_eq!(z.offset_at(200), 7200);
        assert_eq!(z.offset_at(i64::MAX), 7200, "마지막 전환의 값이 뒤로 이어진다");
        assert_eq!(z.shift("1970-01-01T00:02:00Z"), "1970-01-01T01:02:00Z");
    }

    /// **`..` 과 절대 경로는 이름이 아니다.** 이름은 설정 파일과 고르는 창에서 오므로, 그대로
    /// 이어 붙이면 tzdb 밖의 아무 파일이나 TZif 로 읽으려 든다.
    #[test]
    fn a_name_cannot_climb_out_of_the_tzdb() {
        let dir = Path::new("/usr/share/zoneinfo");
        for bad in ["", "/etc/passwd", "../../etc/passwd", "Asia/../../etc/passwd", "./x", "Asia//Seoul"] {
            assert_eq!(safe_join(dir, bad), None, "{bad:?} 가 이름으로 섰다");
        }
        assert_eq!(safe_join(dir, "Asia/Seoul"), Some(dir.join("Asia/Seoul")));
        assert_eq!(safe_join(dir, "UTC"), Some(dir.join("UTC")));
    }

    /// **`/etc/localtime` 은 상대로도 걸린다**(리뷰). systemd 의 `timedatectl set-timezone` 은
    /// `../usr/share/zoneinfo/<이름>` 으로 걸고, 절대로 거는 배포판도 있어 둘 다 산다. 상대인 것을
    /// 그대로 잘라 내려 하면 어느 접두어와도 안 맞아, 그 기계에서는 화면이 UTC 로 서면서 매 명령에
    /// "시스템이 제 시간대를 안 댄다" 가 붙는다 — 이 기능이 고치려던 바로 그 자리다.
    #[test]
    fn a_relative_localtime_link_still_names_its_zone() {
        let dir = Path::new("/usr/share/zoneinfo");
        assert_eq!(name_under(Path::new("/usr/share/zoneinfo/Asia/Seoul"), dir), Some("Asia/Seoul".into()));
        assert_eq!(name_under(Path::new("../usr/share/zoneinfo/Asia/Seoul"), dir), Some("Asia/Seoul".into()));
        assert_eq!(name_under(Path::new("../usr/share/zoneinfo/UTC"), dir), Some("UTC".into()));
        // tzdb 밖을 가리키면 이름을 모른다 — 자료는 읽혀도 무엇이라 부를지를 모른다.
        assert_eq!(name_under(Path::new("/etc/localtime"), dir), None);
        assert_eq!(name_under(Path::new("../var/db/timezone/Asia/Seoul"), dir), None);
        // 제 자리를 가리키는 링크는 이름이 아니다 — 빈 글자가 이름으로 서지 않는다.
        assert_eq!(name_under(dir, dir), None);
    }

    /// **`TZ` 의 꼴 셋**(리뷰) — 빈 글은 UTC(POSIX), 경로는 링크와 같은 자로 풀고, 그 밖은 이름이다.
    #[test]
    fn the_tz_variable_takes_a_name_a_path_or_nothing() {
        let zone = Path::new("/usr/share/zoneinfo");
        assert_eq!(name_under(Path::new("/usr/share/zoneinfo/Asia/Tokyo"), zone), Some("Asia/Tokyo".into()));
        // `TZ=:/etc/localtime` 은 tzdb 밖이라 이름이 아니다 — 다음 자리로 내려간다.
        assert_eq!(name_under(Path::new("/etc/localtime"), zone), None);
        // **절대 경로도 글자로 접는다**(리뷰) — 안 접던 판은 `../zoneinfo/Asia/Tokyo` 를 이름으로
        // 냈고, `safe_join` 이 그 `..` 을 막아 그 기계가 UTC 로 떨어졌다. 없는 자리도 접힌다.
        assert_eq!(
            name_under(Path::new("/usr/share/zoneinfo/../zoneinfo/Asia/Tokyo"), zone),
            Some("Asia/Tokyo".into())
        );
        assert_eq!(name_under(Path::new("/usr/share/./zoneinfo/UTC"), zone), Some("UTC".into()));

        // **규칙 글도 이름으로 넘긴다**(moai-btxt.1gk) — 꺾쇠가 든 글을 거르던 판은 `/etc/localtime` 으로 내려갔다.
        let name = |v: &str| tz_name(std::ffi::OsStr::new(v));
        assert_eq!(name(""), Some("UTC".into()));
        assert_eq!(name(":Asia/Seoul"), Some("Asia/Seoul".into()));
        for rule in ["JST-9", "<+09>-9", "<-03>3", "CST6CDT,M3.2.0,M11.1.0", "AEST-10AEDT,M10.1.0,M4.1.0/3"] {
            assert_eq!(name(rule), Some(rule.into()), "{rule:?} 를 이름으로 안 넘겼다");
            assert_eq!(name(&format!(":{rule}")), Some(rule.into()), ":{rule:?} 를 이름으로 안 넘겼다");
        }
    }

    /// **tzdb 에 없는 이름은 POSIX 규칙 글로 연다**(moai-btxt.1gk) — 이름은 받은 글 그대로다(2026-10-01 사용자
    /// 결정). 규칙 글은 자료가 필요 없어, 이 기계에 tzdb 가 있든 없든 같은 답이다.
    #[test]
    fn a_posix_rule_opens_as_a_zone_under_its_own_text() {
        let at = |s: &str| crate::model::parse_rfc3339(s).unwrap();
        for text in ["JST-9", "<+09>-9"] {
            let z = Zone::load(text).unwrap_or_else(|why| panic!("{text:?} 를 못 열었다 — {why:?}"));
            assert_eq!(z.name(), text, "받은 글이 이름으로 안 섰다");
            assert!(!z.is_utc());
            assert_eq!(z.shift("2026-10-01T00:00:00Z"), "2026-10-01T09:00:00Z");
        }
        let ny = Zone::load("EST5EDT,M3.2.0,M11.1.0").expect("서머타임이 든 규칙 글을 못 열었다");
        assert_eq!(ny.local(at("2026-01-15T12:00:00Z")), at("2026-01-15T07:00:00Z"));
        assert_eq!(ny.local(at("2026-07-01T12:00:00Z")), at("2026-07-01T08:00:00Z"));
        // `/` 가 든 규칙 글은 tzdb 쪽에서 여러 마디의 경로로 찾다가 못 찾고 규칙으로 선다(리뷰 moai-btxt.0ln).
        let sydney = Zone::load("AEST-10AEDT,M10.1.0,M4.1.0/3").expect("`/` 가 든 규칙 글을 못 열었다");
        assert_eq!(sydney.local(at("2026-01-15T00:00:00Z")), at("2026-01-15T11:00:00Z"));
        assert_eq!(sydney.local(at("2026-07-01T00:00:00Z")), at("2026-07-01T10:00:00Z"));
        // UTC 와 같은 규칙은 UTC 다 — 화면이 그 자리에서 그대로 둔다.
        assert!(Zone::load("UTC0").expect("UTC0 를 못 열었다").is_utc());
        // 이름도 규칙도 아니면 tzdb 의 까닭이 그대로 선다 — 규칙으로 못 읽었다는 말을 따로 짓지 않는다.
        for bad in ["JST", "JST-9x", "<+09"] {
            match Zone::load(bad) {
                Err(Trouble::Unknown { name }) => assert_eq!(name, bad),
                Err(Trouble::NoTzdb { .. }) => {}
                other => panic!("{bad:?} 가 시간대로 섰다 — {other:?}"),
            }
        }
    }

    /// 손으로 지은 TZif 를 판 1·판 2 두 꼴로 읽는다. **판 2 면 뒤 자료를 읽는다** — 앞의 32비트
    /// 시각은 2038년에 끊기고, 그 자료를 읽으면 그 뒤의 전환이 통째로 사라진다.
    #[test]
    fn a_v2_file_is_read_from_the_second_block() {
        // 판 1 자료에는 전환 하나(+1h), 판 2 자료에는 둘(+1h, +2h)을 담는다 — 어느 쪽을 읽었는지가
        // 전환 수로 드러난다.
        let v1 = tzif(b'2', 4, &[(100i64, 1)], &[0, 3600, 7200]);
        let v2 = tzif(b'2', 8, &[(100i64, 1), (1 << 40, 2)], &[0, 3600, 7200]);
        let mut raw = v1.clone();
        raw.extend_from_slice(&v2);
        raw.extend_from_slice(b"\nSTD0\n");
        let (shifts, before, rule) = parse(&raw).expect("판 2 파일을 못 읽었다");
        assert_eq!(before, 0);
        assert_eq!(shifts, vec![(100, 3600), (1 << 40, 7200)], "판 1 자료를 읽었다");
        assert_eq!(rule, Some(Rule { std: 0, dst: None }), "꼬리를 못 읽었다");

        // 판 1 짜리 파일은 앞 자료를 그대로 쓰고, 꼬리가 없다.
        let only = tzif(b'\0', 4, &[(100i64, 1)], &[0, 3600, 7200]);
        assert_eq!(parse(&only), Some((vec![(100, 3600)], 0, None)));

        // 모양이 아니면 `None` — 그 이름은 못 읽은 것이고, 부르는 쪽이 UTC 로 떨어진다.
        assert_eq!(parse(b"nope"), None);
        assert_eq!(parse(&raw[..40]), None, "머리글이 잘린 파일을 읽었다");

        // **꼬리를 못 읽어도 파일은 선다**(moai-r621) — 규칙만 없고, 마지막 전환의 값이 뒤로 이어진다.
        let mut odd = v1.clone();
        odd.extend_from_slice(&v2);
        odd.extend_from_slice(b"\nEST5EDT\n");
        assert_eq!(parse(&odd), Some((vec![(100, 3600), (1 << 40, 7200)], 0, None)));
    }

    /// **머리글의 셈이 파일 밖을 대면 TZif 가 아니다**(moai-btxt.xia). 셈은 남이 지은 파일(`TZDIR`)이 대는 32비트
    /// 수라, 곱해 더하면 `usize` 가 32비트인 기계에서 넘친다 — 거기서 넘친 크기로 자리를 지으면 엉뚱한 바이트를
    /// 읽거나 첨자가 터진다. 64비트 기계에서는 셈 하나를 끝까지 올려도 안 넘치니, 여기서 붉어지는 것은 파일 길이로
    /// 묶는 자다. 판 1 파일의 뒤쪽이 잘린 것도 같은 자에 걸린다.
    #[test]
    fn a_header_that_counts_past_the_file_is_not_a_tzif() {
        let fine = tzif(b'\0', 4, &[(100i64, 1)], &[0, 3600]);
        assert_eq!(parse(&fine), Some((vec![(100, 3600)], 0, None)));
        // isutcnt·isstdcnt·leapcnt·timecnt·typecnt·charcnt 차례로 하나씩 끝까지 올린다.
        for at in [20, 24, 28, 32, 36, 40] {
            let mut huge = fine.clone();
            huge[at..at + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            assert_eq!(parse(&huge), None, "{at} 자리의 셈이 파일 밖을 대는데 읽었다");
        }
        // 윤초 하나를 댔는데 그 자리가 없다 — 판 1 은 그 자리를 안 읽어도, 잘린 파일이라 못 받는다.
        let mut cut = fine.clone();
        cut[28..32].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(parse(&cut), None, "잘린 판 1 파일을 읽었다");
        // 판 2 의 앞 자료가 파일 밖을 대면 뒤 머리글을 찾으러 가지 않는다.
        let mut v2 = tzif(b'2', 4, &[(100i64, 1)], &[0, 3600]);
        v2.extend_from_slice(&tzif(b'2', 8, &[(100i64, 1)], &[0, 3600]));
        v2.extend_from_slice(b"\nSTD0\n");
        assert!(parse(&v2).is_some());
        v2[32..36].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(parse(&v2), None, "판 1 자료의 셈이 파일 밖을 대는데 읽었다");
    }

    /// **slim 파일은 마지막 전환 뒤를 꼬리가 잰다**(moai-r621). `zic -b slim`(2020b 부터 기본)으로 지은
    /// New York 은 2007년 3월의 EDT 가 마지막 전환이다 — 꼬리를 안 읽던 판은 2026년 1월을 서머타임으로
    /// 그렸다. 리뷰 moai-efoc.ln9 가 되풀이한 그 순간을 그대로 잰다.
    #[test]
    fn a_slim_file_reads_past_its_last_transition_from_the_footer() {
        let est = |s: &str| crate::model::parse_rfc3339(s).unwrap();
        let shifts = [(est("2006-10-29T06:00:00Z"), 0u8), (est("2007-03-11T07:00:00Z"), 1)];
        let v1 = tzif(b'2', 4, &shifts, &[-5 * 3600, -4 * 3600]);
        let v2 = tzif(b'2', 8, &shifts, &[-5 * 3600, -4 * 3600]);
        let mut raw = v1;
        raw.extend_from_slice(&v2);
        raw.extend_from_slice(b"\nEST5EDT,M3.2.0,M11.1.0\n");
        let (shifts, before, rule) = parse(&raw).expect("slim 파일을 못 읽었다");
        let z = Zone { name: "America/New_York".into(), shifts, before, rule };
        assert_eq!(z.shift("2026-01-15T04:30:00Z"), "2026-01-14T23:30:00Z", "겨울을 서머타임으로 그렸다");
        assert_eq!(z.shift("2026-07-01T12:00:00Z"), "2026-07-01T08:00:00Z");
        assert_eq!(z.shift("2007-07-01T12:00:00Z"), "2007-07-01T08:00:00Z");
        // **적힌 전환 안쪽은 전환이 잰다**(리뷰 moai-efoc.3e1) — 2006-11-01 은 10-29 에 EST 로 옮긴 뒤지만 2007년부터의
        // 규칙으로 재면 11-05 까지 EDT 다. 규칙을 전환보다 앞세우는 판이 여기서 붉어진다.
        assert_eq!(z.shift("2006-11-01T12:00:00Z"), "2006-11-01T07:00:00Z", "적힌 전환 안쪽을 규칙이 쟀다");
        assert_eq!(z.shift("2007-01-15T04:30:00Z"), "2007-01-14T23:30:00Z");
    }

    /// **마지막 전환 뒤는 규칙이 그 뒤에 처음 바꾸는 때부터 잰다**(리뷰 moai-efoc.3e1). slim 으로 지은
    /// `America/Ojinaga` 는 2022-10-30 에 CST 로 옮긴 것이 마지막 전환인데, 꼬리(`CST6CDT,M3.2.0,M11.1.0`)는 11-06
    /// 까지 CDT 다 — 규칙을 그 순간부터 대던 판은 그 한 주를 한 시간 앞선 시각으로 그렸다. tzcode 는 꼬리로 지은 전환 가운데
    /// 마지막 전환보다 뒤의 것만 잇고, fat 판과 tzdata 원본(`-6 - CST 2022 N 30`)도 그 주는 CST 다.
    #[test]
    fn the_footer_takes_over_at_its_first_change_after_the_last_transition() {
        let at = |s: &str| crate::model::parse_rfc3339(s).unwrap();
        let z = Zone {
            name: "America/Ojinaga".into(),
            shifts: vec![(at("2022-03-13T09:00:00Z"), -6 * 3600), (at("2022-10-30T08:00:00Z"), -6 * 3600)],
            before: -7 * 3600,
            rule: Rule::parse("CST6CDT,M3.2.0,M11.1.0"),
        };
        for (when, off) in [
            ("2022-10-30T08:00:00Z", -6),
            ("2022-11-01T12:00:00Z", -6),
            ("2022-11-06T07:00:00Z", -6),
            ("2023-03-12T07:59:59Z", -6),
            ("2023-03-12T08:00:00Z", -5),
            ("2023-11-05T07:00:00Z", -6),
        ] {
            assert_eq!(z.offset_at(at(when)), off * 3600, "Ojinaga {when}");
        }
    }

    /// 꼬리 글의 꼴 — 실제 tzdb 가 쓰는 것들이다. 전환 앞뒤 한 초씩을 잰다.
    #[test]
    fn the_footer_rule_reads_every_posix_form() {
        let at = |s: &str| crate::model::parse_rfc3339(s).unwrap();
        let rule = |s: &str| Rule::parse(s).unwrap_or_else(|| panic!("{s:?} 를 못 읽었다"));
        let h = |x: f64| (x * 3600.0) as i32;

        // 북반구 — 3월 둘째 일요일 02:00 EST 에 들어가 11월 첫 일요일 02:00 EDT 에 나온다.
        let ny = rule("EST5EDT,M3.2.0,M11.1.0");
        assert_eq!(ny, rule("EST5EDT4,M3.2.0/2,M11.1.0/02:00:00"), "빼도 되는 자리를 채운 글과 달리 읽었다");
        for (when, off) in [
            ("2026-03-08T06:59:59Z", -5.0),
            ("2026-03-08T07:00:00Z", -4.0),
            ("2026-11-01T05:59:59Z", -4.0),
            ("2026-11-01T06:00:00Z", -5.0),
        ] {
            assert_eq!(ny.offset_at(at(when)), h(off), "New York {when}");
        }

        // 남반구 — 서머타임이 해를 넘겨 걸친다. 나오는 시각은 서머타임의 03:00 이다.
        let sydney = rule("AEST-10AEDT,M10.1.0,M4.1.0/3");
        for (when, off) in [
            ("2026-01-01T00:00:00Z", 11.0),
            ("2026-04-04T15:59:59Z", 11.0),
            ("2026-04-04T16:00:00Z", 10.0),
            ("2026-10-03T15:59:59Z", 10.0),
            ("2026-10-03T16:00:00Z", 11.0),
            ("2026-12-31T23:59:59Z", 11.0),
        ] {
            assert_eq!(sydney.offset_at(at(when)), h(off), "Sydney {when}");
        }

        // 꺾쇠 이름과 음수 시각(RFC 8536 3.3.1) — Nuuk 는 3월 마지막 일요일 -01:00 에 들어간다.
        let nuuk = rule("<-02>2<-01>,M3.5.0/-1,M10.5.0/0");
        for (when, off) in [
            ("2026-03-29T00:59:59Z", -2.0),
            ("2026-03-29T01:00:00Z", -1.0),
            ("2026-10-25T00:59:59Z", -1.0),
            ("2026-10-25T01:00:00Z", -2.0),
        ] {
            assert_eq!(nuuk.offset_at(at(when)), h(off), "Nuuk {when}");
        }

        // 분이 든 오프셋과 시각 — Chatham 은 9월 마지막 일요일 02:45 에 들어간다.
        let chatham = rule("<+1245>-12:45<+1345>,M9.5.0/2:45,M4.1.0/3:45");
        assert_eq!(chatham.offset_at(at("2026-09-26T13:59:59Z")), h(12.75));
        assert_eq!(chatham.offset_at(at("2026-09-26T14:00:00Z")), h(13.75));

        // 24시 — Santiago 는 토요일 24:00, 곧 일요일 00:00 에 바뀐다.
        let santiago = rule("<-04>4<-03>,M9.1.6/24,M4.1.6/24");
        assert_eq!(santiago.offset_at(at("2026-09-06T03:59:59Z")), h(-4.0));
        assert_eq!(santiago.offset_at(at("2026-09-06T04:00:00Z")), h(-3.0));

        // 늘 서머타임 — 나오자마자 다시 들어간다. Casablanca 가 이 꼴이다. 해 바뀌는 순간도 +01 이다.
        let casablanca = rule("<+00>0<+01>,0/0,J365/25");
        for when in ["2026-01-01T00:00:00Z", "2025-12-31T23:00:00Z", "2026-06-30T12:00:00Z", "2028-12-31T23:30:00Z"] {
            assert_eq!(casablanca.offset_at(at(when)), h(1.0), "Casablanca {when}");
        }

        // 시각이 167시까지 가면 앞 해의 두 전환이 다 이 해로 넘어온다 — 그때 선 것은 두 해 앞의 전환이다(리뷰
        // moai-efoc.3e1). 2024년 몫의 들어감(2025-01-06 23:00)이 2026-01-04 03:00 의 나옴까지 이어진다.
        let late = rule("STD0DST,J365/167,J365/100");
        for (when, off) in [("2026-01-02T00:00:00Z", 1.0), ("2026-01-04T03:00:00Z", 0.0), ("2026-01-06T23:00:00Z", 1.0)]
        {
            assert_eq!(late.offset_at(at(when)), h(off), "J365/167 {when}");
        }

        // **한 해 넘게 걸친 서머타임은 늘 서머타임이다**(moai-btxt.49f) — 그해의 나옴(이듬해 1월 1일 04:00 UTC)이
        // 이듬해의 들어감(03:00 UTC)보다 늦어 두 해의 서머타임이 겹친다. 나옴을 마지막 전환으로 치던 판은 1월 1일
        // 04:00 부터 한 해 내내 표준시(-3)로 그렸다. tzcode 는 늘 서머타임(-2)으로 읽는다. 윤년도 같다. 첫 줄에서는
        // glibc 와 갈린다 — glibc 는 그해의 들어감(03:00 UTC) 앞의 세 시간을 표준시로 그리지만, 앞 해의 서머타임이
        // 아직 안 끝난 때라 tzcode 를 따른다(리뷰 moai-btxt.0ln).
        let spanning = rule("AAA3BBB,J1/0,J365/26");
        for when in [
            "2026-01-01T02:59:59Z",
            "2026-01-01T03:00:00Z",
            "2026-01-01T04:00:00Z",
            "2026-07-01T12:00:00Z",
            "2028-12-31T12:00:00Z",
            "2029-01-01T04:00:00Z",
        ] {
            assert_eq!(spanning.offset_at(at(when)), h(-2.0), "J1/0,J365/26 {when}");
        }

        // 겹침은 **해마다** 가린다 — 서수 날(`n`)은 2월 29일을 세어, 같은 `365/1` 이 평년에는 이듬해의 들어감과
        // 맞물리고 윤년에는 12월 31일에 끝난다. 그날 하루만 표준시다(glibc 와 같다).
        let ordinal = rule("STD0DST,0/0,365/1");
        for (when, off) in [
            ("2026-12-31T12:00:00Z", 1.0),
            ("2027-06-01T00:00:00Z", 1.0),
            ("2028-12-30T23:59:59Z", 1.0),
            ("2028-12-31T00:00:00Z", 0.0),
            ("2028-12-31T23:59:59Z", 0.0),
            ("2029-01-01T00:00:00Z", 1.0),
        ] {
            assert_eq!(ordinal.offset_at(at(when)), h(off), "0/0,365/1 {when}");
        }

        // 서머타임 없는 규칙 — 분이 든 오프셋은 부호를 뒤집어 읽는다.
        assert_eq!(rule("<+0330>-3:30"), Rule { std: h(3.5), dst: None });
        assert_eq!(rule("KST-9").offset_at(i64::MAX), h(9.0));

        // `i64` 끝에서도 넘치지 않는다.
        let _ = ny.offset_at(i64::MAX);
        let _ = ny.offset_at(i64::MIN);

        // 꼴이 아니면 `None` — 부르는 쪽은 규칙이 없는 것으로 읽는다.
        for bad in [
            "",
            "EST",
            "E5",
            "EST5x",
            "EST5EDT",
            "EST5EDT4",
            "EST5EDT,M3.2.0",
            "EST5EDT,M13.2.0,M11.1.0",
            "EST5EDT,M3.6.0,M11.1.0",
            "EST5EDT,M3.2.7,M11.1.0",
            "EST5EDT,J0,J300",
            "EST5EDT,366,300",
            "EST25",
            "<>5",
            "<EST5",
        ] {
            assert_eq!(Rule::parse(bad), None, "{bad:?} 를 규칙으로 읽었다");
        }
    }

    /// 규칙의 날 셋 — `Jn` 은 2월 29일을 안 세고, `n` 은 센다. `Mm.5.d` 는 그달의 마지막 그 요일이다.
    #[test]
    fn a_rule_day_lands_on_its_calendar_day() {
        use crate::model::days_from_civil;
        assert_eq!(Day::Julian(59).in_year(2028), days_from_civil(2028, 2, 28));
        assert_eq!(Day::Julian(60).in_year(2028), days_from_civil(2028, 3, 1), "윤년의 J60 이 2월 29일에 섰다");
        assert_eq!(Day::Julian(60).in_year(2026), days_from_civil(2026, 3, 1));
        assert_eq!(Day::Julian(365).in_year(2028), days_from_civil(2028, 12, 31));
        assert_eq!(Day::Ordinal(0).in_year(2026), days_from_civil(2026, 1, 1));
        assert_eq!(Day::Ordinal(59).in_year(2028), days_from_civil(2028, 2, 29));
        assert_eq!(Day::Ordinal(59).in_year(2026), days_from_civil(2026, 3, 1));
        // 2026년 3월 1일은 일요일이다 — 첫째 일요일이 1일, 다섯째(마지막)는 29일이다.
        assert_eq!(Day::Month { m: 3, w: 1, d: 0 }.in_year(2026), days_from_civil(2026, 3, 1));
        assert_eq!(Day::Month { m: 3, w: 5, d: 0 }.in_year(2026), days_from_civil(2026, 3, 29));
        // 2026년 2월에는 다섯째 토요일이 없다 — 넷째(28일)가 마지막이다.
        assert_eq!(Day::Month { m: 2, w: 5, d: 6 }.in_year(2026), days_from_civil(2026, 2, 28));
        assert_eq!(Day::Month { m: 11, w: 1, d: 0 }.in_year(2026), days_from_civil(2026, 11, 1));
    }

    /// **전환이 없는 파일은 꼬리가 모든 때를 잰다**(RFC 8536 3.3). 그 꼬리가 UTC 가 아니면 UTC 가 아니다 —
    /// `before` 만 보던 [`Zone::is_utc`] 는 그런 시간대를 UTC 로 읽어 [`Zone::shift`] 가 그대로 돌려줬다.
    #[test]
    fn a_zone_with_only_a_footer_is_not_utc() {
        let only = |s: &str| Zone { name: "T".into(), shifts: Vec::new(), before: 0, rule: Rule::parse(s) };
        let ny = only("EST5EDT,M3.2.0,M11.1.0");
        assert!(!ny.is_utc());
        assert_eq!(ny.shift("2026-01-15T04:30:00Z"), "2026-01-14T23:30:00Z");
        // `Etc/UTC` 의 slim 파일은 전환 없이 `UTC0` 만 든다 — 그것은 UTC 다.
        assert!(only("UTC0").is_utc());
    }

    /// 시험용 TZif 한 벌. `kinds` 는 ttinfo 의 오프셋들이고, 첫 것을 표준(서머타임 아님)으로 둔다.
    fn tzif(version: u8, time: usize, shifts: &[(i64, u8)], kinds: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"TZif");
        out.push(version);
        out.extend_from_slice(&[0u8; 15]);
        for n in [0u32, 0, 0, shifts.len() as u32, kinds.len() as u32, 0] {
            out.extend_from_slice(&n.to_be_bytes());
        }
        for (at, _) in shifts {
            match time {
                4 => out.extend_from_slice(&(*at as i32).to_be_bytes()),
                _ => out.extend_from_slice(&at.to_be_bytes()),
            }
        }
        for (_, k) in shifts {
            out.push(*k);
        }
        for (n, off) in kinds.iter().enumerate() {
            out.extend_from_slice(&off.to_be_bytes());
            out.push(u8::from(n > 0)); // 첫 것만 표준
            out.push(0);
        }
        out
    }

    /// **까닭은 화면이 실제로 선 시계의 것 하나다**(리뷰). 고른 이름이 서면 시스템을 못 푼 까닭은
    /// 할 말이 아니다 — 화면은 그 이름으로 서는데 "시각은 UTC 로 선다" 가 나란히 붙으면 둘 중
    /// 어느 쪽을 믿을지 사람이 정해야 한다. `/etc/localtime` 을 상대 링크로 거는 기계가 흔해
    /// 실제로 겹치는 자리다.
    #[test]
    fn the_chosen_zone_answers_for_itself() {
        // `UTC` 는 자료를 안 보므로 tzdb 없는 기계에서도 선다 — 그때도 할 말은 없다.
        assert_eq!(chosen(Some("UTC")), (Zone::utc(), None));
        // 못 푼 이름은 UTC 로 떨어지고 **그 이름의** 까닭을 댄다(자료가 아예 없으면 그 까닭이다).
        let (z, why) = chosen(Some("Mars/Olympus"));
        assert!(z.is_utc(), "못 푼 이름으로 시계가 섰다");
        match why {
            Some(Trouble::Unknown { name }) => assert_eq!(name, "Mars/Olympus"),
            Some(Trouble::NoTzdb { .. }) => {}
            other => panic!("못 푼 이름의 까닭이 아니다 — {other:?}"),
        }
        // 고른 적이 없으면 시스템이 답한다 — 까닭도 시스템의 것이다.
        assert_eq!(chosen(None), Zone::system());
    }

    /// **이 기계가 무엇을 쓰든 답이 있다.** 시스템이 이름을 안 대면 UTC 와 까닭을 함께 낸다 —
    /// 막지 않는다.
    #[test]
    fn the_system_zone_always_answers() {
        let (z, why) = Zone::system();
        // 못 풀었으면 UTC 고, 풀었으면 그 이름이 선다. 둘 다 아닌 답은 없다.
        assert!(why.is_none() || z == Zone::utc(), "못 풀었는데 UTC 가 아니다 — {z:?} {why:?}");
        assert!(!z.name().is_empty());
    }
}
