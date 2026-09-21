//! 시간대 — **화면에만** 건다(moai-v6vo).
//!
//! 저널의 `ts` 와 스냅샷의 `created_at`·`updated_at` 은 RFC3339 UTC 고정폭이고 그대로 남는다.
//! `--json` 도 그대로다. 바꾸는 것은 사람이 읽는 글자뿐이다 — `i18n` 이 `kind` 를 안 건드리고
//! `said` 만 옮긴 것과 같은 줄이고, "설정은 화면만 바꾼다. 설정이 이미 쓴 줄을 바꾸면 그건
//! 설정이 아니라 마이그레이션이다" 와도 같다.
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
    /// 그 이름의 자료가 없다 — 오타이거나, 이 기계의 tzdb 가 그 이름을 모른다.
    Unknown { name: String },
    /// 자료는 있는데 못 읽거나 TZif 가 아니다.
    Unreadable { name: String, said: String },
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
    /// 첫 전환보다 앞선 때에 더할 초.
    before: i32,
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
        Zone { name: "UTC".into(), shifts: Vec::new(), before: 0 }
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

    /// UTC 인가 — 화면이 이것으로 "그대로 둔다" 를 가른다.
    pub fn is_utc(&self) -> bool {
        self.shifts.is_empty() && self.before == 0
    }

    /// 이름으로 연다. `UTC` 는 자료를 안 본다 — tzdb 가 없는 기계에서도 서야 한다.
    pub fn load(name: &str) -> Result<Zone, Trouble> {
        if name == "UTC" {
            return Ok(Zone::utc());
        }
        let dir = zoneinfo();
        if !dir.is_dir() {
            return Err(Trouble::NoTzdb { at: dir });
        }
        let at = safe_join(&dir, name).ok_or_else(|| Trouble::Unknown { name: name.to_string() })?;
        let raw = std::fs::read(&at).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Trouble::Unknown { name: name.to_string() },
            _ => Trouble::Unreadable { name: name.to_string(), said: e.to_string() },
        })?;
        let (shifts, before) = parse(&raw)
            .ok_or_else(|| Trouble::Unreadable { name: name.to_string(), said: "TZif 가 아니다".into() })?;
        Ok(Zone { name: name.to_string(), shifts, before })
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
    fn offset_at(&self, secs: i64) -> i32 {
        match self.shifts.partition_point(|(at, _)| *at <= secs) {
            0 => self.before,
            n => self.shifts[n - 1].1,
        }
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
            Some(secs) => crate::model::format_rfc3339(secs + i64::from(self.offset_at(secs))),
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

/// `TZ` 가 대는 이름. 없거나 이름 꼴이 아니면 `None` 이고, 그때는 `/etc/localtime` 이 답한다.
fn env_name() -> Option<String> {
    // `TZ=:Asia/Seoul` 처럼 콜론을 다는 꼴이 있다(POSIX).
    let raw = std::env::var_os("TZ")?.to_string_lossy().trim_start_matches(':').to_string();
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
    // **이름 꼴만 받는다.** `TZ=<+09>-9` 같은 POSIX 규칙 글은 이름이 아니다. `EST5EDT`·`Etc/GMT+9`
    // 처럼 숫자와 부호를 쓰는 **진짜 이름**이 있어 숫자로는 못 가르므로, 가르는 것은 글자뿐이다 —
    // 그물을 빠져나온 규칙 글은 tzdb 에서 못 찾고 UTC 로 떨어지며 한 줄로 알린다.
    raw.chars().all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c)).then_some(raw)
}

/// `at` 이 `dir` 밑을 가리키면 그 아래 경로가 곧 시간대 이름이다.
///
/// **상대 링크를 푼다**(리뷰). systemd 의 `timedatectl set-timezone` 은 `/etc/localtime` 을
/// `../usr/share/zoneinfo/<이름>` 으로 건다 — 절대로 거는 배포판도 있어 둘 다 산다. 상대인 것을
/// 그대로 잘라 내려 하면 어느 접두어와도 안 맞아 이름을 못 얻고, 그 기계에서는 화면이 UTC 로
/// 서면서 매 명령에 "시스템이 제 시간대를 안 댄다" 가 붙는다 — 이 기능이 고치려던 바로 그 자리다.
///
/// **접는 것은 글자로만 한다** — 파일 시스템에 안 묻는다. 물어야 하는 자리(링크의 링크, 링크인
/// tzdb 디렉터리)는 부르는 쪽이 `canonicalize` 로 한 번 더 댄다.
fn name_under(at: &Path, dir: &Path) -> Option<String> {
    let full = match at.is_absolute() {
        true => at.to_path_buf(),
        // `/etc/localtime` 의 링크라 기준은 `/etc` 다.
        false => flatten(&Path::new("/etc").join(at)),
    };
    let name = full.strip_prefix(dir).ok()?.to_str()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// 경로의 `.`·`..` 를 **글자로만** 접는다. `canonicalize` 와 달리 파일 시스템을 안 본다 —
/// 없는 자리도 접을 수 있어야 하고, 접는 값이 이름 하나를 얻는 값보다 크면 안 된다.
fn flatten(at: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for part in at.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
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

/// TZif 를 푼다 — (전환, 첫 전환 앞의 오프셋). 모양이 아니면 `None`.
///
/// **판 2 이상이면 뒤 자료를 읽는다.** 앞의 판 1 자료는 32비트 시각이라 2038년에 끊긴다. 판 1
/// 짜리 파일(요즘은 거의 없다)은 앞 자료를 그대로 쓴다.
///
/// 꼬리의 POSIX 규칙 글(`KST-9`)은 **안 읽는다** — 마지막 전환보다 뒤를 그 규칙으로 계산하는
/// 자린데, tzdb 는 앞으로 수십 년어치 전환을 이미 담고 있어 이 도구가 그리는 때에는 안 걸린다.
/// 걸리면 마지막 전환의 오프셋이 그대로 이어진다.
fn parse(raw: &[u8]) -> Option<(Vec<(i64, i32)>, i32)> {
    let (v1, version) = head(raw, 0)?;
    if version < b'2' {
        return block(raw, v1.after, &v1, 4);
    }
    // 판 1 자료를 건너뛴 자리에 판 2 머리가 다시 선다.
    let skip = v1.size(4);
    let (v2, _) = head(raw, v1.after + skip)?;
    block(raw, v2.after, &v2, 8)
}

/// TZif 머리글의 셈들.
struct Head {
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
    /// 머리글 바로 뒤 — 자료가 시작하는 자리.
    after: usize,
}

impl Head {
    /// 이 셈이 낸 자료 덩어리의 바이트 수. `time` 은 전환 시각 하나의 크기(4 또는 8)다.
    fn size(&self, time: usize) -> usize {
        self.timecnt * (time + 1)
            + self.typecnt * 6
            + self.charcnt
            + self.leapcnt * (time + 4)
            + self.isstdcnt
            + self.isutcnt
    }
}

fn head(raw: &[u8], at: usize) -> Option<(Head, u8)> {
    let b = raw.get(at..at + 44)?;
    if &b[..4] != b"TZif" {
        return None;
    }
    let n = |i: usize| -> usize { u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize };
    Some((
        Head {
            isutcnt: n(20),
            isstdcnt: n(24),
            leapcnt: n(28),
            timecnt: n(32),
            typecnt: n(36),
            charcnt: n(40),
            after: at + 44,
        },
        b[4],
    ))
}

fn block(raw: &[u8], at: usize, h: &Head, time: usize) -> Option<(Vec<(i64, i32)>, i32)> {
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
        let z = Zone { name: "T".into(), shifts: Vec::new(), before: 9 * 3600 };
        for odd in ["", "언제", "2026-09-21", "2026-09-21T08:24:58+09:00"] {
            assert_eq!(z.shift(odd), odd, "못 읽은 글을 건드렸다");
        }
    }

    /// 첫 전환 앞과 뒤, 그리고 전환 바로 그 순간.
    #[test]
    fn the_offset_follows_the_transitions() {
        let z = Zone { name: "T".into(), shifts: vec![(100, 3600), (200, 7200)], before: 0 };
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
        // `..` 은 글자로만 접는다 — 없는 자리도 접힌다.
        assert_eq!(flatten(Path::new("/etc/../usr/share/zoneinfo/UTC")), PathBuf::from("/usr/share/zoneinfo/UTC"));
        assert_eq!(flatten(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
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
        let (shifts, before) = parse(&raw).expect("판 2 파일을 못 읽었다");
        assert_eq!(before, 0);
        assert_eq!(shifts, vec![(100, 3600), (1 << 40, 7200)], "판 1 자료를 읽었다");

        // 판 1 짜리 파일은 앞 자료를 그대로 쓴다.
        let only = tzif(b'\0', 4, &[(100i64, 1)], &[0, 3600, 7200]);
        assert_eq!(parse(&only), Some((vec![(100, 3600)], 0)));

        // 모양이 아니면 `None` — 그 이름은 못 읽은 것이고, 부르는 쪽이 UTC 로 떨어진다.
        assert_eq!(parse(b"nope"), None);
        assert_eq!(parse(&raw[..40]), None, "머리글이 잘린 파일을 읽었다");
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
