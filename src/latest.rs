//! 최신판을 묻고 답을 하루 들고 있는다 (moai-omww, 사용자 결정 2026-09-21).
//!
//! 저장소에 네트워크를 타는 길이 하나도 없었다 — `install.sh` 만 깔 때 최신 릴리스를 받고, 깔린
//! 뒤에 새 판이 났는지는 아무도 안 물었다. 탐색기 판 줄이 `latest not checked` 라 적은 것은 그
//! 사실을 낱말로 적은 것이지 실패가 아니었다.
//!
//! **막지 않는다.** 새 판이 있어도 알림 한 줄이고 종료 코드는 안 건드린다 — `moai status` 가
//! 아무것도 막지 않는다는 결정과 같은 줄이다. 못 물은 것과 새 판이 없는 것은 **다른 글**이고
//! ([`Seen`]), 어느 쪽도 사람을 세우지 않는다.
//!
//! 사용자 결정 넷이 이 파일의 뼈대다.
//!
//! - **묻는 길은 `ureq` 다.** 새 축의 의존성이라 CLAUDE.md 가 먼저 상의하라고 한 자리고, 재서
//!   대고 물었다. 크레이트를 고른 것은 **없는 기계가 없기 때문**이다 — `curl` 도 `wget` 도 없는
//!   기계에서는 이 기능이 영영 "못 물었다" 로 서는데, 그 기계가 어느 기계인지 도구는 모른다.
//!   잰 값은 `Cargo.toml` 의 그 줄에 있다(이 저장소에서 +1,775,096바이트, 15MB 예산의 53%) —
//!   **한자리에만, 잰 때와 함께 적는다**: 두 벌로 적으면 다시 잴 때 한쪽이 낡고, 때가 없으면
//!   다음 사람이 그 수를 지금 크기로 읽는다
//! - **하루 한 번 묻는다**([`WINDOW`]). 답과 물은 때를 [`FILE`] 에 적고 그 안이면 안 묻는다.
//!   자리는 읽음 파일([`crate::read_marks`])의 관례를 따라 설정 파일 곁이고, 판은 프로젝트마다
//!   다르지 않으니 사람마다 한 파일이다
//! - **기본은 켬이되 사람이 보는 화면에서만이다**([`gate`]). `--json` 과 파이프는 안 묻는다 —
//!   에이전트가 도는 기계가 매번 바깥을 두드리면 안 된다. **둘은 따로 묻는다**: 에이전트는
//!   tmux 칸 안에서 치므로 표준 출력이 터미널이고, 화면만 재면 `--json` 이 그 문을 지난다
//! - **태그를 semver 로 견준다**([`compare`]). 넷이 다른 글이다
//!
//! **그리는 걸음에 실리지 않는다.** [`spawn`] 이 딴 실에서 묻고, 그리는 쪽은 `App` 이 받아 둔
//! 답을 읽기만 한다 — `tui::draw::version_said` 는 프레임마다 도는 자리라 거기서 `git config`
//! 조차 안 부른다(그 주석이 적어 둔 까닭이 이것과 같다).
//!
//! **실패는 모두 [`Seen::Unasked`] 로 내려앉되, 까닭을 들고 온다**([`Why`], 사용자 결정
//! 2026-09-22, moai-580l). 처음에는 한 변형으로 접었는데 — 네트워크가 없든, 느려서
//! [`TIMEOUT`] 을 넘든, 답이 깨진 JSON 이든, 태그가 `v1.2.3` 꼴이 아니든 — **사람이 할 일이
//! 갈래마다 다르다**: 시간당 판 수를 태운 것은 기다리면 풀리고, 프록시가 인증서를 갈아 끼운
//! 네트워크는 이 기능을 영영 못 쓰며(moai-uwuw), 네트워크가 없는 것은 그 둘 중 어느 것도
//! 아니다. 가르는 것은 `kind` 고 꼴은 `commits_error` 와 같다([`crate::git::Told`]).
//!
//! **그래도 넷은 그대로 넷이다.** 위의 사용자 결정 4 가 가른 넷은 *화면 글*이고, 이것은 그
//! 넷째 **안**을 가르는 일이다 — 못 물은 것은 여전히 못 물은 것이라 최신과 안 섞이고,
//! 갈랐어도 아무것도 막지 않는다.

use crate::fail::R;
use crate::store::write_atomic;
use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 답이 사는 파일 — `<설정 디렉터리>/latest.toml`. **도구가 짓고 도구가 고쳐 쓴다.**
pub const FILE: &str = "latest.toml";

/// [`FILE`] 안의 두 키. **이름은 한자리에 둔다** — [`UPDATE`]·[`CHECK`] 를 상수로 둔 까닭과
/// 같다. 글 속에 박아 두면 이름을 고치는 날 읽는 쪽만 따라가고 쓰는 쪽이 낡는다.
pub const ASKED_AT: &str = "asked_at";
pub const TAG: &str = "tag";

/// 받아서 들 태그의 길이 상한(바이트). git 의 ref 이름은 낱말 하나라 이보다 길 수 없고,
/// [`ask_within`] 이 받는 256KB 를 그대로 파일에 적어 둘 까닭도 없다.
const MAX_TAG: usize = 128;

/// 묻는 자리. `MOAI_API_URL` 이 있으면 그것을 쓴다 — `install.sh` 가 이미 같은 이름으로 같은
/// 자리를 돌린다. 시험이 제 서버를 띄워 붙는 자리이기도 하다.
pub const API: &str = "https://api.github.com/repos/buzzni/moai/releases/latest";

/// 한 번 물으면 이만큼은 안 묻는다.
pub const WINDOW: i64 = 24 * 60 * 60;

/// 한 번 묻는 데 드는 시간의 상한. **짧다** — 이 답은 화면 한 줄이라, 느린 그물에서 오래
/// 붙들고 있느니 "못 물었다" 가 낫다.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// 설정에서 이것을 끄는 자리 — `[update] check = false`.
pub const UPDATE: &str = "update";
pub const CHECK: &str = "check";

/// 환경에서 이것을 끄는 자리. 값은 안 본다 — 비어 있지 않으면 끈다(환경변수의 관례).
pub const OFF_VAR: &str = "MOAI_NO_UPDATE_CHECK";

/// 물어서 안 것. **넷이 다른 글이다**(사용자 결정 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// 새 판이 있다. 태그는 받은 그대로다 — 글에 그대로 서므로 `v` 를 떼지 않는다.
    Newer { tag: String },
    /// 같은 판이다.
    Same,
    /// 내 판이 더 앞섰다. 소스로 빌드해 쓰는 사람이 늘 보는 자리라 "새 판" 과 갈라 둔다 —
    /// 합치면 개발 중인 사람에게 평생 "낡았다" 고 말한다.
    Ahead,
    /// 못 물었다. **까닭을 함께 든다**([`Why`], moai-580l) — 사람이 할 일이 까닭마다 다르기
    /// 때문이다. 시간당 판 수를 다 태운 것은 기다리면 풀리고, 프록시가 인증서를 갈아 끼운
    /// 네트워크는 이 기능을 못 쓴다는 뜻이며, 네트워크가 없는 것은 둘 중 어느 것도 아니다.
    Unasked(Why),
}

/// 못 물은 까닭 — **자료지 글이 아니다**. 꼴은 `show --json` 의 `commits_error`
/// ([`crate::git::Told`])와 같다: 가르는 것은 `kind` 고, `said` 는 사람이 까닭을 볼 한 줄이다.
///
/// **`said` 는 라이브러리와 운영체제가 지은 글이라 안 옮긴다** — [`crate::git::Error::said`]
/// 와 같은 자리다. 사람에게 댈 한 낱말은 화면이 [`Trouble`] 로 고른다.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Why {
    pub kind: Trouble,
    pub said: String,
}

/// 못 물은 까닭의 갈래. **낱말이 아니라 타입이다** — 화면이 이것으로 갈라 그리므로, 갈래를
/// 더하는 날 컴파일러가 안 고친 자리를 이름으로 댄다.
///
/// **기계에 낼 낱말은 직렬화가 짓는다** — `not_asked`·`rate_limited` 처럼 `commits_error` 의
/// `kind` 와 같은 꼴이다. 낱말을 손으로 적는 함수를 곁에 두지 않는 것은 그것이 갈래와 두 벌이
/// 되기 때문이고, 그 낱말이 실제로 그렇게 서는지는 `the_kind_words_are_the_contract` 가 잰다.
/// **아직 아무 표면도 이 값을 안 낸다** — 판 소식은 탐색기 한 줄뿐이다. `--json` 에 실릴 날
/// 그 자리가 쓰는 낱말이 여기 미리 서 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trouble {
    /// 아직 안 물었다 — 적어 둔 답이 없거나, 물었지만 태그를 못 들은 채 하루가 안 지났다.
    /// 끈 사람도 여기로 온다([`Off`]): 제가 끈 것은 제가 안다.
    NotAsked,
    /// 네트워크가 없다 — 이름을 못 풀거나, 붙지 못하거나, 소켓이 끊겼다.
    Offline,
    /// [`TIMEOUT`] 안에 못 끝냈다. 느린 네트워크와 안 붙는 네트워크는 다른 일이다.
    Timeout,
    /// 시간당 판 수를 다 태웠다 — GitHub 은 이것을 403 으로도 429 로도 낸다. **기다리면
    /// 풀리는 하나뿐인 갈래라** 따로 둔다: 에이전트가 도는 고리가 이 자리에 자주 선다.
    RateLimited,
    /// 서버가 그 밖의 상태 코드로 거절했다 — 404(저장소를 옮겼다)가 가장 흔하다.
    Http,
    /// TLS 가 안 섰다. 사내 네트워크의 프록시가 인증서를 갈아 끼우는 기계가 여기 선다 —
    /// 그 기계는 이 기능을 **못 쓴다**(moai-uwuw 가 닫은 결정, 루트 저장소는 `webpki-roots`
    /// 하나로 간다). 다른 갈래와 섞으면 그 사람이 고칠 수 없는 것을 고치러 다닌다.
    Tls,
    /// 답이 왔는데 못 읽었다 — JSON 이 아니거나, `tag_name` 이 없거나, 태그가 성치 않다.
    Garbled,
    /// 태그는 받았는데 판으로 못 읽었다 — `v0.1` 처럼 수가 셋이 아니거나 semver 가 아니다.
    /// [`Garbled`](Trouble::Garbled) 와 가르는 까닭은 고칠 자리가 다르기 때문이다: 이쪽은
    /// **릴리스를 지은 쪽**이 태그를 그렇게 달았다.
    OddTag,
    /// 위의 어느 것도 아닌 실패. [`crate::git::Told`] 의 `failed` 와 같은 자리다.
    Failed,
}

impl Why {
    /// 아직 안 물었다 — `said` 는 빈 글이다. 적어 둘 까닭이 없다.
    pub fn not_asked() -> Self {
        Why { kind: Trouble::NotAsked, said: String::new() }
    }

    /// `ureq` 가 낸 실패를 갈래로. **`said` 는 그 크레이트가 지은 글 그대로다.**
    ///
    /// **403 과 429 를 한 갈래로 접는다.** GitHub 은 시간당 판 수를 다 태운 부름에 둘 중
    /// 아무 쪽이나 내고(문서가 그렇게 적는다), 사람이 할 일은 둘 다 "기다린다" 다.
    /// `User-Agent` 를 안 단 부름도 403 인데, 그쪽은 [`ask_within`] 이 늘 달고 나가므로
    /// 이 자리에 안 선다.
    fn from_ureq(e: &ureq::Error) -> Self {
        let kind = match e {
            ureq::Error::StatusCode(403 | 429) => Trouble::RateLimited,
            ureq::Error::StatusCode(_) => Trouble::Http,
            ureq::Error::Timeout(_) => Trouble::Timeout,
            ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => Trouble::Offline,
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::Pem(_) => Trouble::Tls,
            _ => Trouble::Failed,
        };
        Why { kind, said: e.to_string() }
    }
}

/// 이 바이너리의 판.
pub fn mine() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 태그에서 판을 뗀다 — `v0.1.0` → `0.1.0`. `v` 없는 태그도 받는다.
fn version_of(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// `1.2.3` 또는 `1.2.3-rc1` → `([1,2,3], Some("rc1"))`. 꼴이 아니면 `None`.
///
/// **수는 셋이어야 한다.** `0.1` 을 `0.1.0` 으로 채워 읽으면 태그 하나가 두 판을 뜻하게 되고,
/// 그때 "같은 판" 이 참인지 거짓인지 아무도 모른다. 릴리스가 짓는 태그는 `scripts/check-version.sh`
/// 가 `Cargo.toml` 과 맞춰 둔 셋짜리다.
fn parts(v: &str) -> Option<([u64; 3], Option<&str>)> {
    // 빌드 메타데이터(`+…`)는 판을 가르지 않는다 — semver 가 그렇게 정했다. **`-` 보다 먼저
    // 뗀다**: semver 의 꼴이 `<수>[-<앞판>][+<메타>]` 라 `-` 를 먼저 가르면 `0.2.0+ci-1234` 의
    // `1234` 가 앞판으로 읽혀, 같은 판이 "앞선 판" 이 된다. 뒤에 붙은 `+` 도 함께 떨어져
    // `0.2.0-rc1+b.7` 의 앞판이 `rc1+b.7` 이 되는 일도 없다 — 그 둘은 semver 에서 같은 판이다.
    let (v, build) = match v.split_once('+') {
        Some((v, build)) => (v, Some(build)),
        None => (v, None),
    };
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        Some(_) => return None,
        None => (v, None),
    };
    let mut it = core.split('.');
    let mut n = [0u64; 3];
    for slot in &mut n {
        let word = it.next()?;
        if word.is_empty() || !word.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = word.parse().ok()?;
    }
    if it.next().is_some() {
        return None;
    }
    // **앞판도 꼴을 잰다.** semver 가 앞판에 허락하는 글자는 `[0-9A-Za-z-]` 뿐이고 마디는 비지
    // 않는다. 안 재면 `v9.9.9-<ESC>[2J` 가 성한 판으로 읽혀 [`Seen::Newer`] 의 태그로 화면에
    // 그대로 서고([`Seen::Newer`] 의 글이 그렇게 약속한다) 하루 동안 [`FILE`] 에 남는다 —
    // 그물에서 온 글을 들어오는 자리에서 씻는 것은 `text::sanitize` 가 선 까닭과 같다.
    //
    // **메타데이터도 같은 자로 잰다**(리뷰). 판을 가르지 않는다고 안 재던 자리인데, 안 재면
    // `v9.9.9+` 나 `v9.9.9+아무 글` 이 성한 판으로 읽혀 위와 **똑같이** 화면에 서고 파일에
    // 남는다 — 태그가 화면에 그대로 서는 것은 앞판이 붙었는지와 무관하기 때문이다. 앞판의
    // 빈 마디(`0.2.0-`)를 거절하면서 메타데이터의 빈 마디(`0.2.0+`)를 받는 것도 한쪽만 선
    // 자다.
    if [pre, build].into_iter().flatten().any(|s| !dotted_ok(s)) {
        return None;
    }
    Some((n, pre))
}

/// semver 의 마디 열인가 — 점으로 가르고, 마디는 비지 않으며 `[0-9A-Za-z-]` 뿐이다.
/// 앞판과 빌드 메타데이터가 같은 자를 쓴다(semver 9·10항).
fn dotted_ok(s: &str) -> bool {
    s.split('.').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

/// 두 앞판을 semver 로 견준다 — 점으로 가른 마디마다, 수는 수로 수 아닌 것은 글자로, 수가
/// 글자보다 앞이고 마디가 적은 쪽이 앞이다.
///
/// **글자로만 견주면 `rc.2` 가 `rc.10` 보다 뒤에 선다.** 핵심 세 수에서 같은 함정을 막은 것이
/// `a_number_is_read_as_a_number_not_as_a_word` 인데, 한 겹 아래 이 자리에도 그 함정이 있었다.
fn pre_cmp(a: &str, b: &str) -> Ordering {
    let num = |w: &str| w.bytes().all(|b| b.is_ascii_digit()).then(|| w.parse::<u64>().ok()).flatten();
    let (mut x, mut y) = (a.split('.'), b.split('.'));
    loop {
        let ord = match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            // 마디가 적은 쪽이 앞이다.
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) => match (num(p), num(q)) {
                (Some(m), Some(n)) => m.cmp(&n),
                // 수는 글자보다 앞이다.
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => p.cmp(q),
            },
        };
        // 이 마디가 같으면 다음 마디로 간다.
        if ord != Ordering::Equal {
            return ord;
        }
    }
}

/// 두 판을 견준다. 한쪽이라도 꼴이 아니면 `None` 이고, 그때 답은 [`Seen::Unasked`] 다.
///
/// **prerelease 는 그 판보다 앞이다**(semver). `0.2.0-rc1` 은 `0.2.0` 보다 앞이고 `0.1.0` 보다
/// 뒤다. `releases/latest` 는 prerelease 를 빼고 주므로 지금은 드문 자리지만, 뒤에 태그를 직접
/// 읽게 되면 이 줄이 그때 선다.
pub fn compare(mine: &str, theirs: &str) -> Option<Ordering> {
    let (a, apre) = parts(mine)?;
    let (b, bpre) = parts(theirs)?;
    Some(match a.cmp(&b) {
        Ordering::Equal => match (apre, bpre) {
            (None, None) => Ordering::Equal,
            // prerelease 가 붙은 쪽이 앞이다.
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(x), Some(y)) => pre_cmp(x, y),
        },
        other => other,
    })
}

/// 받은 태그를 내 판과 견줘 [`Seen`] 으로. 태그가 없거나 꼴이 아니면 못 물은 것이고, 그
/// **둘은 다른 갈래다**([`Trouble::NotAsked`]·[`Trouble::OddTag`], moai-580l).
pub fn seen(mine: &str, tag: Option<&str>) -> Seen {
    let Some(tag) = tag else { return Seen::Unasked(Why::not_asked()) };
    match compare(mine, version_of(tag)) {
        Some(Ordering::Less) => Seen::Newer { tag: tag.to_string() },
        Some(Ordering::Equal) => Seen::Same,
        Some(Ordering::Greater) => Seen::Ahead,
        // **받은 태그를 `said` 에 그대로 싣는다** — 무엇이 판으로 안 읽혔는지가 고칠 자리다.
        // 들어오는 자리([`tag_in`])가 제어문자와 길이를 이미 물렸으므로 화면에 실어도 된다.
        None => Seen::Unasked(Why { kind: Trouble::OddTag, said: tag.to_string() }),
    }
}

/// 적어 둔 답 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// 물은 때(RFC3339 UTC). **물은 때지 답을 받은 때가 아니다** — 못 들은 판도 이 자리를
    /// 적으므로, 그물이 없는 기계가 부를 때마다 5초를 버리지 않는다.
    pub asked_at: String,
    /// 받은 태그. 못 들었으면 `None` 이다.
    pub tag: Option<String>,
}

impl Held {
    /// 이 답이 아직 창 안인가. 때가 꼴이 아니면 낡은 것으로 본다 — 못 읽는 도장 하나가 묻기를
    /// 영영 막으면, 그 파일을 지우는 것 말고 되돌릴 길이 없다.
    ///
    /// **조금 앞선 도장은 봐준다**([`crate::model::FUTURE_SLACK_SECS`], moai-21un). 설정
    /// 디렉터리를 나눠 쓰는 두 기계의 시계가 1초만 어긋나도 한쪽이 찍은 도장이 다른 쪽에는
    /// 미래로 보여, 그 기계는 부를 때마다 바깥을 두드렸다 — 에이전트가 도는 고리에서는
    /// GitHub 의 시간당 60판을 태우는 길이다. 봐주는 자는 이 저장소가 이미 쓰던 것과 같다
    /// (`report` 의 `far_ahead`, `model::days_since` 의 0 클램프).
    ///
    /// **크게 앞선 도장은 여전히 낡은 것이다.** 시계를 되돌린 기계나 손으로 적은 2099 가 창을
    /// 영영 열어 두지 못하게 한다.
    pub fn fresh(&self, now: &str, window: i64) -> bool {
        let (Some(then), Some(now)) = (crate::model::parse_rfc3339(&self.asked_at), crate::model::parse_rfc3339(now))
        else {
            return false;
        };
        (-crate::model::FUTURE_SLACK_SECS..window).contains(&(now - then))
    }
}

/// 답이 사는 자리 — 설정 파일의 **곁**이다.
pub fn file_at(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// 적어 둔 답을 읽는다. **관대하게 읽는다** — 파일이 없거나 깨졌으면 없는 것이고, 없으면 묻는다.
pub fn read(dir: &Path) -> Option<Held> {
    let doc = doc_at(dir)?;
    let asked_at = doc.get(ASKED_AT)?.as_str()?.to_string();
    let tag = doc.get(TAG).and_then(|i| i.as_str()).map(String::from);
    Some(Held { asked_at, tag })
}

/// 파일을 문서로 읽는다. 없거나 깨졌으면 `None` — [`read`] 와 [`write`] 가 한 자를 쓴다.
fn doc_at(dir: &Path) -> Option<toml_edit::DocumentMut> {
    std::fs::read_to_string(file_at(dir)).ok()?.parse().ok()
}

/// 도구가 이 파일을 처음 지을 때 머리에 다는 줄. **영어다** — 화면 말과 달리 이 글은 파일에
/// 남고 `write` 는 말을 모른다. 말묶음을 안 타는 글은 영어로 둔다는 2026-09-20 결정
/// (`moai-54k2`, `moai init` 이 심는 블록이 영어가 된 그 결정)과 같은 자리다.
const HEADER: &str = "# moai writes this file. Delete it and it asks again.\n";

/// 아직 없던 파일의 첫 문서 — [`HEADER`] 를 **문서의 꾸밈으로** 단다.
///
/// **`HEADER.parse()` 로 짓지 않는다**(리뷰). 키도 표도 없는 글에서 라이브러리는 그 주석을
/// 통째로 *끝 글*(`trailing`)로 들고 맨 끝에 그린다 — 뿌리의 키는 그보다 먼저 서므로, 머리에
/// 달려던 줄이 `asked_at`·`tag` **밑으로** 밀린다. 사용자 설정이 같은 함정을 만나 푼 자리가
/// [`crate::user_config::Doc`] 의 `lift_head_comment` 다(moai-bx7g).
fn new_doc() -> toml_edit::DocumentMut {
    let mut doc = toml_edit::DocumentMut::new();
    doc.decor_mut().set_prefix(HEADER);
    doc
}

/// 답을 적는다. **스냅샷 먼저, 저널 나중** 과 같은 자리에 선다 — 이 파일은 잃어도 한 번 더 묻는
/// 것이 전부라 **락을 안 잡는다.** 프로세스 둘이 같이 쓰면 늦은 쪽이 남고, 둘 다 같은 것을 적으므로
/// 진 쪽도 잃는 것이 없다. 같은 프로세스의 실 둘도 같다 — [`crate::store::tmp_name`] 이 임시
/// 이름을 실마다 가르므로(moai-mpf4) 둘 중 한쪽의 글이 통째로 남는다. [`spawn`] 을 한 판에
/// 하나만 띄우는 것은 여전히 부르는 쪽의 몫이지만, 그것은 실과 소켓을 아끼자는 것이지 파일이
/// 깨지기 때문이 아니다.
///
/// **`write_atomic` 은 fsync 를 두 번 한다** — 잃어도 그만인 이 파일에는 과한 durability 지만,
/// 쓰기 길을 하나로 두는 것(`write_atomic_as` 를 되살리지 않는다는 moai-c1s3 결정)이 더 값지다.
/// 하루 한 번 드는 값이다.
///
/// **모르는 키와 곁의 주석은 그대로 들고 간다**([`crate::read_marks`] 의 관례이고, CLAUDE.md 의
/// "모르는 필드는 보존돼야 한다" 다). 이 파일은 판이 다른 바이너리들이 함께 쓰는 자리라 — 새 판이
/// 났는지 묻는 것이 이 모듈의 일이니 판이 섞이는 것은 예외가 아니라 전제다 — 새 바이너리가 적은
/// 키를 옛 바이너리가 한 번 만져 지우면 안 된다.
pub fn write(dir: &Path, held: &Held) -> R<()> {
    std::fs::create_dir_all(dir).map_err(|e| crate::fail::Fail::new(format!("{}: {e}", dir.display())))?;
    let mut doc = doc_at(dir).unwrap_or_else(new_doc);
    doc[ASKED_AT] = toml_edit::value(held.asked_at.as_str());
    match &held.tag {
        Some(tag) => doc[TAG] = toml_edit::value(tag.as_str()),
        None => {
            doc.remove(TAG);
        }
    }
    write_atomic(&file_at(dir), doc.to_string().as_bytes())
}

/// 왜 안 묻는지 — **글이 아니라 자료다**. 지금은 아무도 이것을 펴지 않는다: 안 물은 것과 못 물은
/// 것이 사람에게는 한 글이고([`Seen::Unasked`]), 까닭이 궁금한 사람은 제가 끈 것을 안다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Off {
    /// [`OFF_VAR`] 가 섰다.
    Env,
    /// 설정이 껐다 — `[update] check = false`.
    Config,
    /// 사람이 보는 화면이 아니다 — `--json`·파이프·비대화형.
    NotAScreen,
}

/// 물어도 되는가. `None` 이면 물어도 된다.
///
/// **끄는 길이 둘인 것은 자리가 둘이기 때문이다.** 설정은 이 사람의 기계에서 늘, 환경변수는 이
/// 부름에서만 — CI 한 판이나 에이전트 한 판을 위해 사람의 설정을 고치게 하지 않는다.
///
/// **`json` 을 따로 받는다.** [`on_screen`] 은 표준 출력이 터미널인가만 묻는데, 에이전트는
/// tmux 칸 안에서 `moai show --json` 을 친다 — 그때 출력은 터미널이라 화면만 재면 이 문이
/// 열린다. 부르는 쪽이 `!json && on_screen()` 을 잊지 않게 갈라 받는다. 둘 다 같은 까닭
/// ([`Off::NotAScreen`])으로 접히는 것은, 사람에게 댈 글이 하나여서지 물음이 하나여서가 아니다.
pub fn gate(
    env: impl Fn(&str) -> Option<OsString>,
    config_says: Option<bool>,
    json: bool,
    on_screen: bool,
) -> Option<Off> {
    if env(OFF_VAR).is_some_and(|v| !v.is_empty()) {
        return Some(Off::Env);
    }
    if config_says == Some(false) {
        return Some(Off::Config);
    }
    if json || !on_screen {
        return Some(Off::NotAScreen);
    }
    None
}

/// 이 부름이 사람이 보는 화면인가 — 표준 출력이 터미널인가로 묻는다. **`--json` 은 안 본다**:
/// 그것은 [`gate`] 가 따로 받는다.
pub fn on_screen() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// 사람의 설정이 이것을 껐는가 — `[update] check = false`.
///
/// **[`crate::user_config::Doc`] 를 안 지난다.** 그쪽은 고쳐 쓰는 길이라 주석과 모르는 키를
/// 지키는 것이 일인데, 이 키는 **도구가 한 번도 안 쓴다** — 사람이 손으로 적고 도구는 읽기만
/// 한다. 키 이름을 두 벌로 두지 않으려고 이름은 [`UPDATE`]·[`CHECK`] 한 자리에 두었다.
///
/// **끄는 값만 읽는다.** `false` 가 아닌 것은 모두 켠 것이다. 색이나 화면 말과 달리 틀린 값을
/// 알리지 않는 것은 이 설정이 **끄는 스위치 하나**라, 안 먹은 것이 그 자리에서 보이기 때문이다 —
/// 끄려고 적었는데 판 줄이 여전히 서면 그때 안다. 틀린 색은 그럴듯한 색이 서서 모르지만 여기는
/// 다르다.
///
/// **BOM 은 떼고 읽는다.** toml_edit 의 파서는 머리의 `\u{feff}` 에 걸려 통째로 실패하고,
/// [`crate::user_config::Doc::parse`] 가 그것을 떼는 것도 같은 까닭이다 — 그쪽만 떼면 윈도에서
/// 적은 설정이 `[i18n]` 은 먹는데 이 키만 말없이 안 먹는다. 틀린 값과 달리 이쪽은 **바로 적었는데**
/// 안 먹는 것이라, "안 먹으면 그 자리에서 보인다" 는 위의 까닭이 안 선다.
pub fn config_says(path: Option<&Path>) -> Option<bool> {
    let src = std::fs::read_to_string(path?).ok()?;
    let doc: toml_edit::DocumentMut = src.strip_prefix('\u{feff}').unwrap_or(&src).parse().ok()?;
    doc.get(UPDATE)?.as_table_like()?.get(CHECK)?.as_bool()
}

/// 한 번 묻는다. 태그를 못 얻으면 **까닭을 든다**([`Why`], moai-580l).
///
/// `User-Agent` 를 다는 것은 GitHub 가 없는 부름에 403 을 주기 때문이다.
pub fn ask(url: &str) -> Result<String, Why> {
    ask_within(url, TIMEOUT)
}

/// [`ask`] 되 기다리는 상한을 받는다. **시험이 그 상한을 짧게 줘서 실제로 끊기는지 잰다** —
/// 상한을 상수로만 두면 그것이 서는지를 5초씩 기다려야만 볼 수 있고, 그러면 아무도 안 잰다.
pub fn ask_within(url: &str, timeout: Duration) -> Result<String, Why> {
    let built = ureq::Agent::config_builder()
        // **머리부터 몸까지 통째로 잰다.** 붙기만 재면 붙여 놓고 한 글자씩 흘리는 자리에
        // 영영 붙들린다.
        .timeout_global(Some(timeout))
        // 되돌림은 **둘까지만** 따라간다(기본은 열이다). 저장소 이름이 바뀌면 릴리스 API 가 301
        // 로 보내므로 아예 안 따라가면 그날 이 기능이 죽는다. 그래도 열까지 갈 까닭은 없고,
        // 셋째부터는 `max_redirects_will_error` 가 오류로 세 "못 물었다" 가 된다.
        .max_redirects(2);
    // **되돌이 자리는 프록시를 안 탄다.** ureq 의 기본은 `ALL_PROXY`·`HTTPS_PROXY`·`HTTP_PROXY`
    // 를 그대로 따르고 `NO_PROXY` 에 손으로 적은 이름만 비껴간다 — 되돌이를 기본으로 비껴가지
    // 않는다. 바깥으로 나가는 부름에는 그 편이 맞지만(회사 그물은 프록시로만 나간다), 제 서버를
    // 띄워 붙는 이 모듈의 시험은 프록시가 선 기계에서 그리로 나가 버린다. 그러면 서버는 손님을
    // 못 만나 `accept` 에서 멈추고, `cargo test` 는 붉어지는 대신 **영영 매달린다**. curl 도
    // 7.86 부터 되돌이를 비껴간다.
    let agent: ureq::Agent = if is_loopback(url) { built.proxy(None) } else { built }.build().into();
    let body = agent
        .get(url)
        .header("User-Agent", concat!("moai/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| Why::from_ureq(&e))?
        .body_mut()
        // **받는 만큼에 상한을 둔다.** `read_to_string` 하나만 부르면 10MB 에서 끊기지만
        // `with_config()` 로 들어가는 순간 상한이 `u64::MAX` 로 풀리므로, 이 줄은 조이는 것이
        // 아니라 **다시 거는 것**이다. 이 답은 몇 KB 고, `MOAI_API_URL` 이 가리키는 자리가
        // 끝없이 뱉을 때 그것을 다 받아 줄 까닭이 없다.
        .with_config()
        .limit(256 * 1024)
        // 글자가 깨져도 읽는다 — 어차피 `tag_name` 하나만 집고, 못 집으면 "못 물었다" 다.
        .lossy_utf8(true)
        .read_to_string()
        .map_err(|e| Why::from_ureq(&e))?;
    // **읽다 끊긴 것과 못 읽는 답은 다른 갈래다** — 위의 `map_err` 가 앞엣것을, 여기가 뒤엣것을
    // 든다. `said` 에 답을 싣지 않는 것은 그것이 256KB 까지 가는 남의 글이기 때문이다.
    tag_in(&body).ok_or_else(|| Why { kind: Trouble::Garbled, said: String::new() })
}

/// 이 자리가 이 기계 자신인가 — `http://127.0.0.1:…`·`localhost`·`[::1]`.
///
/// **이름을 풀지 않는다.** 여기서 DNS 를 물으면 프록시를 고르는 데 그물이 드는데, 이 물음의
/// 임자는 "시험이 제 서버에 붙는가" 하나다. 남의 이름이 되돌이를 가리키는 판은 프록시를 타도
/// 그만이다(그 판의 답은 어차피 "못 물었다" 다).
fn is_loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    // 사용자 정보(`user@`)를 떼고 포트를 뗀다. `[::1]:8080` 은 대괄호가 포트와 주소를 가른다.
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split_once(']').map_or(v6, |(h, _)| h),
        None => host.split_once(':').map_or(host, |(h, _)| h),
    };
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host.parse::<std::net::Ipv4Addr>().is_ok_and(|a| a.is_loopback())
}

/// 답에서 태그를 집는다. 깨진 JSON 도, `tag_name` 이 없는 답도 `None` 이다.
///
/// **들어오는 자리에서 잰다**(`git::Error::told` 가 git 의 글을 그렇게 다루는 자리와 같다).
/// 이 글은 그물에서 오고, 화면에 그대로 서며([`Seen::Newer`]), 하루 동안 [`FILE`] 에 남는다 —
/// 제어문자 하나가 화면을 다시 칠하고 그 바이트가 다시 물을 때까지 남는다. **씻지 않고
/// 물린다**: 낱말 하나여야 할 자리에 제어문자가 들었으면 그것은 태그가 아니라 딴 것이고, 반만
/// 씻어 들이면 "받은 그대로다" 라는 [`Seen::Newer`] 의 약속이 거짓이 된다.
fn tag_in(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let tag = v.get("tag_name")?.as_str()?.trim();
    let sane = !tag.is_empty() && tag.len() <= MAX_TAG && !tag.chars().any(char::is_control);
    sane.then(|| tag.to_string())
}

/// 물을 자리 — `MOAI_API_URL` 이 있으면 그것, 없으면 [`API`].
pub fn url_from(env: impl Fn(&str) -> Option<OsString>) -> String {
    env("MOAI_API_URL").filter(|v| !v.is_empty()).and_then(|v| v.into_string().ok()).unwrap_or_else(|| API.to_string())
}

/// 창이 열렸으면 묻고 적는다. 창 안이면 적어 둔 것을 그대로 쓴다.
///
/// **못 들어도 적는다** — 물은 때를 적어 두어야 그물 없는 기계가 부를 때마다 [`TIMEOUT`] 을
/// 버리지 않는다. 적기에 실패해도 이 판의 답은 안 바뀐다.
///
/// **다만 적기가 늘 실패하면 창은 영영 안 닫힌다** — 창을 닫는 것이 그 도장 하나뿐이라, 읽기
/// 전용 `$HOME` 이나 남의 uid 가 쥔 설정 디렉터리에서는 "한 번 더" 가 아니라 **부를 때마다**
/// 묻는다. 그 기계에서 에이전트가 도는 고리는 GitHub 의 시간당 60판을 금세 태운다. 지금은 아무도
/// 안 부르므로 고치지 않고 적어 둔다 — 부르는 쪽이 서면 그 판에서 한 번 알릴지를 정한다.
///
/// **못 들었다고 알던 것을 지우지는 않는다.** 도장만 새로 찍고 태그는 지난 것을 들고 간다 —
/// 지우던 판은 어제 "새 판 v0.2.0 이 있다" 를 본 사람이 오늘 그물이 한 번 끊긴 것만으로 "못
/// 물었다" 를 보게 했고, 그것은 [`held`] 가 "창이 지났어도 지난 답을 그대로 낸다" 고 적어 둔
/// 약속과도 어긋난다. 릴리스는 사라지지 않으니 지난 답은 틀려도 낡은 쪽으로만 틀린다.
pub fn refresh(dir: &Path, url: &str, now: &str, window: i64) -> Seen {
    let held = read(dir);
    if let Some(h) = &held
        && h.fresh(now, window)
    {
        return seen(mine(), h.tag.as_deref());
    }
    let asked = ask(url);
    let why = asked.as_ref().err().cloned();
    let tag = asked.ok().or_else(|| held.and_then(|h| h.tag));
    let _ = write(dir, &Held { asked_at: now.to_string(), tag: tag.clone() });
    match tag {
        Some(tag) => seen(mine(), Some(&tag)),
        // 못 들었고 알던 것도 없다 — 이 판의 까닭을 그대로 낸다(moai-580l).
        None => Seen::Unasked(why.unwrap_or_else(Why::not_asked)),
    }
}

/// 적어 둔 답만 읽는다 — **묻지 않는다.** 창이 지났어도 지난 답을 그대로 낸다: 그리는 쪽이
/// 첫 프레임에 쓸 값이고, 새 답은 [`spawn`] 이 받아 뒤에 얹는다.
pub fn held(dir: &Path) -> Seen {
    read(dir).map_or_else(|| Seen::Unasked(Why::not_asked()), |h| seen(mine(), h.tag.as_deref()))
}

/// 딴 실에서 [`refresh`] 를 돌리고 답을 흘린다. **부르는 쪽은 기다리지 않는다** — 받는 쪽이
/// `try_recv` 로 집고, 안 왔으면 [`held`] 가 준 값을 그대로 그린다.
///
/// 실이 먼저 끝나 받는 쪽이 사라져도 보내기가 실패할 뿐이라 아무 일도 안 난다.
///
/// **실을 못 띄워도 부르는 쪽은 안 죽는다.** `std::thread::spawn` 은 OS 가 실을 못 내면
/// **부른 실에서** 패닉한다 — 그러면 이 기능 때문에 `moai tui` 가 뜨다 만다. 이 모듈이 내건
/// 것은 "아무것도 막지 않는다" 이므로 [`std::thread::Builder`] 로 띄우고, 못 띄우면 보내는
/// 쪽을 놓는다: 받는 쪽은 끊긴 것으로 읽고 [`held`] 가 준 값을 그대로 그린다.
///
/// **손잡이를 함께 돌려준다**(리뷰 7번). 탐색기의 다른 딴 실들이 `Option<(Receiver<…>,
/// JoinHandle<()>)>` 를 함께 드는 것과 같은 꼴이다(`tui::mod` 의 `commits_job`·`pending`,
/// `tui::layer` 의 `reading`) — 손잡이를 드는 쪽이 **한 판에 하나만** 띄우는 자리이기도 하다.
/// 안 들면 프레임마다 띄워 실과 소켓이 초당 서른씩 난다.
///
/// **다만 끝에서 기다리지는 않는다.** 사람이 `moai tui` 를 열고 몇 초 만에 닫으면 그 판의
/// 도장이 안 찍혀 창이 안 닫히는데([`refresh`] 는 도장을 묻고 난 뒤에 찍는다), 닫는 걸음을
/// [`TIMEOUT`] 만큼 붙잡는 것이 더 나쁘다 — 잃는 것은 "한 번 더 묻는다" 뿐이다.
///
/// **못 띄우면 `None` 이다.** `std::thread::spawn` 은 OS 가 실을 못 내면 **부른 실에서**
/// 패닉한다 — 그러면 이 기능 때문에 `moai tui` 가 뜨다 만다. 이 모듈이 내건 것은 "아무것도
/// 막지 않는다" 이므로 [`std::thread::Builder`] 로 띄우고, 못 띄우면 부르는 쪽이 [`held`] 가
/// 준 값을 그대로 그린다.
pub fn spawn(dir: PathBuf, url: String, now: String, window: i64) -> Option<Job> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("moai-latest".into())
        .spawn(move || {
            let _ = tx.send(refresh(&dir, &url, &now, window));
        })
        .ok()?;
    Some((rx, handle))
}

/// 도는 물음 하나 — 받는 쪽과 손잡이. 탐색기가 이 꼴 그대로 든다.
pub type Job = (std::sync::mpsc::Receiver<Seen>, std::thread::JoinHandle<()>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// 못 물었으면 그 갈래, 아니면 `None`. **갈래까지 잰다**(moai-580l) — `Seen::Unasked` 인
    /// 것만 재면 403 이 "네트워크 없음" 으로 서도 시험이 파랗다. `said` 는 라이브러리가
    /// 지은 글이라 글자로 안 잰다.
    fn why_of(seen: &Seen) -> Option<Trouble> {
        match seen {
            Seen::Unasked(why) => Some(why.kind),
            _ => None,
        }
    }
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    /// 한 판만 답하는 서버. 답한 뒤 닫히므로, **두 번째 부름은 붙지 못한다** — 창이 정말로
    /// 묻기를 막았는지를 이 성질로 잰다.
    fn server_once(body: &'static str) -> (String, std::thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("못 띄웠다");
        let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut served = 0;
            if let Ok((stream, _)) = listener.accept() {
                answer(stream, body);
                served += 1;
            }
            served
        });
        (url, handle)
    }

    fn answer(mut stream: TcpStream, body: &str) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        // 머리를 다 읽어야 클라이언트가 답을 받는다.
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if line == "\r\n" || line == "\n" {
                break;
            }
            line.clear();
        }
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.flush();
    }

    /// 아무도 안 듣는 자리. 붙는 즉시 거절당한다 — 느린 그물이 아니라 **없는 그물**을 재는 자다.
    ///
    /// **빈 포트를 잡았다 놓는 식으로 고르지 않는다.** 그렇게 고르면 놓은 포트를 커널이 같은
    /// 판 안에서 `127.0.0.1:0` 로 여는 옆 시험([`server_once`])에 곧바로 내줄 수 있다 —
    /// 그러면 "없는 그물" 시험이 진짜 답을 받아 붉어지고, 그 서버는 제 손님을 잃어 `accept`
    /// 에서 영영 멈춘다. 1 번 포트는 1024 아래라 권한 없는 판이 못 잡고, 그래서 이 시험이 스스로
    /// 겨루지 않는다.
    fn nobody_there() -> String {
        "http://127.0.0.1:1/releases/latest".to_string()
    }

    #[test]
    fn the_four_answers_are_four_different_things() {
        assert_eq!(seen("0.1.0", Some("v0.2.0")), Seen::Newer { tag: "v0.2.0".into() });
        assert_eq!(seen("0.1.0", Some("v0.1.0")), Seen::Same);
        assert_eq!(seen("0.2.0", Some("v0.1.0")), Seen::Ahead);
        assert_eq!(why_of(&seen("0.1.0", None)), Some(Trouble::NotAsked));
        // 꼴이 아닌 태그는 "새 판" 이 아니라 "못 물었다" 다 — 모르는 것을 아는 척하지 않는다.
        // **그 안에서 또 갈린다**(moai-580l): 아예 안 물은 것과 태그를 못 읽은 것은 고칠 자리가
        // 다르다.
        for tag in ["latest", "v0.1", "v0.1.0.1"] {
            assert_eq!(why_of(&seen("0.1.0", Some(tag))), Some(Trouble::OddTag), "{tag}");
        }
    }

    #[test]
    fn a_number_is_read_as_a_number_not_as_a_word() {
        // 글자로 견주면 `0.10.0` 이 `0.9.0` 보다 앞선다.
        assert_eq!(compare("0.9.0", "0.10.0"), Some(Ordering::Less));
        assert_eq!(compare("0.10.0", "0.9.0"), Some(Ordering::Greater));
    }

    #[test]
    fn a_prerelease_stands_before_the_version_it_leads() {
        assert_eq!(compare("0.2.0-rc1", "0.2.0"), Some(Ordering::Less));
        assert_eq!(compare("0.2.0", "0.2.0-rc1"), Some(Ordering::Greater));
        assert_eq!(compare("0.1.0", "0.2.0-rc1"), Some(Ordering::Less));
        assert_eq!(compare("0.2.0+build.5", "0.2.0"), Some(Ordering::Equal));
        assert_eq!(compare("0.2.0-", "0.2.0"), None);
    }

    #[test]
    fn a_number_inside_a_prerelease_is_read_as_a_number_too() {
        // 핵심 세 수에서 막은 함정이 한 겹 아래에도 있었다 — 앞판을 글자 한 덩이로 견주면
        // `rc.10` 이 `rc.2` 보다 앞서고, 그때 rc.2 를 든 사람은 "내 판이 더 앞섰다" 를 보며
        // rc.10 을 영영 못 듣는다. 점으로 갈라 수인 마디를 수로 견주면 풀린다.
        assert_eq!(compare("0.2.0-rc.2", "0.2.0-rc.10"), Some(Ordering::Less));
        assert_eq!(compare("0.2.0-alpha.9", "0.2.0-alpha.10"), Some(Ordering::Less));
        // **`rc9` 와 `rc10` 은 그대로 글자다.** 점이 없으면 마디 하나이고, semver 는 글자가 든
        // 마디를 ASCII 차례로 견주라고 못박았다 — `rc9` 가 뒤다. 수로 읽는 것이 친절해 보이지만
        // 그건 semver 가 아니고, 규격과 어긋난 자를 들면 다른 도구와 답이 갈린다.
        assert_eq!(compare("0.2.0-rc9", "0.2.0-rc10"), Some(Ordering::Greater));
        assert_eq!(compare("0.2.0-rc.10", "0.2.0-rc.2"), Some(Ordering::Greater));
        // 마디가 적은 쪽이 앞이고, 수가 글자보다 앞이다(semver).
        assert_eq!(compare("0.2.0-alpha", "0.2.0-alpha.1"), Some(Ordering::Less));
        assert_eq!(compare("0.2.0-1", "0.2.0-alpha"), Some(Ordering::Less));
        assert_eq!(compare("0.2.0-rc.1", "0.2.0-rc.1"), Some(Ordering::Equal));
    }

    #[test]
    fn build_metadata_does_not_make_a_new_version() {
        // `+` 를 `-` 보다 먼저 떼지 않으면 `ci-1234` 의 `1234` 가 앞판으로 읽혀, 같은 판을 든
        // 사람이 "새 판이 있다" 를 보고 받아 보면 제가 든 그 판이다.
        assert_eq!(compare("0.2.0+ci-1234", "0.2.0"), Some(Ordering::Equal));
        assert_eq!(compare("0.2.0", "0.2.0+ci-1234"), Some(Ordering::Equal));
        assert_eq!(compare("0.2.0-rc1+b.7", "0.2.0-rc1"), Some(Ordering::Equal));
        assert_eq!(seen("0.2.0", Some("v0.2.0+ci-1234")), Seen::Same);
    }

    #[test]
    fn a_tag_that_is_not_a_word_is_not_a_version() {
        // 앞판 자리는 semver 가 `[0-9A-Za-z-]` 로 좁혀 둔 자리다. 안 재면 그물에서 온
        // 제어문자가 성한 판으로 읽혀 [`Seen::Newer`] 의 태그로 화면에 그대로 선다.
        for tag in ["v9.9.9-\u{1b}[2J", "v9.9.9-rc 1", "v9.9.9-rc."] {
            assert_eq!(why_of(&seen("0.1.0", Some(tag))), Some(Trouble::OddTag), "{tag:?}");
        }
        assert_eq!(seen("0.1.0", Some("v9.9.9-rc.1")), Seen::Newer { tag: "v9.9.9-rc.1".into() });
    }

    #[test]
    fn build_metadata_is_measured_by_the_same_rule_as_the_prerelease() {
        // 판을 안 가른다고 안 재던 자리다 — 그런데 태그는 **받은 그대로** 화면에 서고 하루 동안
        // [`FILE`] 에 남으므로, 앞판 뒤에 붙든 `+` 뒤에 붙든 들어오는 글은 같은 자로 재야 한다.
        for (tag, why) in
            [("v9.9.9+", "빈 메타데이터"), ("v9.9.9+b..7", "빈 마디"), ("v9.9.9+새 판", "낱말이 아닌 메타데이터")]
        {
            assert_eq!(why_of(&seen("0.1.0", Some(tag))), Some(Trouble::OddTag), "{why}를 받았다");
        }
        // 성한 메타데이터는 그대로 지난다 — 판을 안 가르는 것은 전과 같다.
        assert_eq!(compare("0.2.0+ci-1234", "0.2.0"), Some(Ordering::Equal));
        assert_eq!(compare("0.2.0-rc1+b.7", "0.2.0-rc1"), Some(Ordering::Equal));
    }

    #[test]
    fn a_file_it_makes_itself_starts_with_the_header() {
        // 머리 줄이 `asked_at` 밑으로 밀리면 "이 파일은 지워도 된다" 는 말을 파일을 연 사람이
        // 맨 끝에서야 본다 — 사용자 설정이 `lift_head_comment` 로 푼 그 자리와 같다.
        let s = Scratch::new("latest-header");
        write(s.path(), &Held { asked_at: "2026-09-21T00:00:00Z".into(), tag: Some("v0.1.0".into()) }).unwrap();
        let src = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(src.starts_with(HEADER), "머리 줄이 머리에 없다\n{src}");
        // 두 번째 쓰기도 머리를 흔들지 않고, 읽는 쪽은 그대로 읽는다.
        write(s.path(), &Held { asked_at: "2026-09-22T00:00:00Z".into(), tag: None }).unwrap();
        let again = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(again.starts_with(HEADER), "두 번째 쓰기가 머리를 흔들었다\n{again}");
        assert_eq!(read(s.path()).unwrap().tag, None);
    }

    #[test]
    fn a_tag_it_hears_is_held_for_a_day() {
        let s = Scratch::new("latest-held");
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let first = refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW);
        assert_eq!(first, Seen::Newer { tag: "v9.9.9".into() });
        assert_eq!(handle.join().unwrap(), 1, "한 판도 안 물었다");

        // 서버는 닫혔다. 창 안이라면 붙을 일이 없으므로 같은 답이 그대로 선다.
        let again = refresh(s.path(), &url, "2026-09-21T23:59:59Z", WINDOW);
        assert_eq!(again, first);
        assert_eq!(read(s.path()).unwrap().asked_at, "2026-09-21T00:00:00Z", "창 안인데 도장을 다시 찍었다");
    }

    #[test]
    fn past_the_window_it_asks_again() {
        let s = Scratch::new("latest-window");
        write(s.path(), &Held { asked_at: "2026-09-20T00:00:00Z".into(), tag: Some("v0.0.1".into()) }).unwrap();
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let now = "2026-09-21T00:00:01Z";
        assert_eq!(refresh(s.path(), &url, now, WINDOW), Seen::Newer { tag: "v9.9.9".into() });
        assert_eq!(handle.join().unwrap(), 1, "창이 지났는데 안 물었다");
        let held = read(s.path()).unwrap();
        assert_eq!(held.asked_at, now);
        assert_eq!(held.tag.as_deref(), Some("v9.9.9"));
    }

    #[test]
    fn a_failed_ask_does_not_forget_what_it_already_heard() {
        // 어제 "새 판 v9.9.9 가 있다" 를 본 사람이, 오늘 그물이 한 번 끊긴 것만으로 "못 물었다"
        // 를 보게 하지 않는다. 도장은 새로 찍혀 하루 동안 다시 안 묻고, 태그는 지난 것이 선다.
        let s = Scratch::new("latest-keeps");
        write(s.path(), &Held { asked_at: "2026-09-20T00:00:00Z".into(), tag: Some("v9.9.9".into()) }).unwrap();
        let now = "2026-09-21T00:00:01Z";
        assert_eq!(refresh(s.path(), &nobody_there(), now, WINDOW), Seen::Newer { tag: "v9.9.9".into() });
        let held = read(s.path()).expect("도장이 없다");
        assert_eq!(held.asked_at, now, "못 들었는데 도장을 안 찍었다");
        assert_eq!(held.tag.as_deref(), Some("v9.9.9"), "알던 것을 지웠다");
    }

    /// **앞선 도장은 슬랙만큼만 봐준다**(moai-21un). 몇 초 어긋난 시계가 부를 때마다 바깥을
    /// 두드리게 하지 않으면서, 손으로 적은 먼 미래는 그대로 낡은 것으로 잡는다. 경계는
    /// [`crate::model::FUTURE_SLACK_SECS`] 고, `report::far_ahead` 와 **같은 쪽이 열린다** —
    /// 딱 그만큼 앞선 것은 둘 다 봐준다.
    #[test]
    fn a_stamp_from_the_future_does_not_hold_the_window_open() {
        let slack = crate::model::FUTURE_SLACK_SECS;
        let far = Held { asked_at: "2099-01-01T00:00:00Z".into(), tag: Some("v0.1.0".into()) };
        assert!(!far.fresh("2026-09-21T00:00:00Z", WINDOW), "시계를 되돌린 기계가 영영 안 묻는다");
        // 1초 어긋난 시계 — 이것까지 낡았다고 하면 그 기계는 부를 때마다 묻는다.
        let ticking = Held { asked_at: "2026-09-21T00:00:01Z".into(), tag: Some("v0.1.0".into()) };
        assert!(ticking.fresh("2026-09-21T00:00:00Z", WINDOW), "1초 앞선 도장에 다시 물었다");
        // 경계를 재는 자리. `slack` 만큼 앞선 것은 봐주고, 1초 더 앞선 것은 안 봐준다.
        let at = |ahead: i64| Held { asked_at: stamp(ahead), tag: None };
        assert!(at(slack).fresh("2026-09-21T00:00:00Z", WINDOW), "딱 슬랙만큼 앞선 도장을 낡았다고 했다");
        assert!(!at(slack + 1).fresh("2026-09-21T00:00:00Z", WINDOW), "슬랙을 넘은 도장이 창을 열어 뒀다");
        let broken = Held { asked_at: "어제".into(), tag: None };
        assert!(!broken.fresh("2026-09-21T00:00:00Z", WINDOW), "못 읽는 도장이 묻기를 막는다");
    }

    /// `2026-09-21T00:00:00Z` 에서 `ahead` 초 뒤인 때를 RFC3339 로. 시험이 경계를 초 단위로
    /// 짚으려면 날짜를 손으로 세지 않는 자가 있어야 한다.
    fn stamp(ahead: i64) -> String {
        crate::model::format_rfc3339(crate::model::parse_rfc3339("2026-09-21T00:00:00Z").unwrap() + ahead)
    }

    #[test]
    fn a_grid_that_is_not_there_is_not_asked_and_not_a_failure() {
        let s = Scratch::new("latest-nogrid");
        let url = nobody_there();
        // 거절당해도 답은 "못 물었다" 고, 종료 코드를 바꿀 `Err` 가 아니다. **까닭은
        // "네트워크가 없다" 다**(moai-580l) — 태그가 이상한 것도, 판 수를 태운 것도 아니다.
        let got = refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW);
        assert_eq!(why_of(&got), Some(Trouble::Offline), "{got:?}");
        // **못 들어도 도장은 찍힌다** — 그물 없는 기계가 부를 때마다 기다리지 않는다.
        let held = read(s.path()).expect("도장이 없다");
        assert_eq!(held.asked_at, "2026-09-21T00:00:00Z");
        assert_eq!(held.tag, None);
        assert!(held.fresh("2026-09-21T00:00:01Z", WINDOW));
    }

    #[test]
    fn a_place_that_never_answers_is_cut_off() {
        // 붙기는 하되 한 글자도 안 주는 자리 — 느린 그물의 최악이다.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
        // **손님을 놓지 않는다.** 서버가 먼저 끊으면 `ask_within` 이 상한이 아니라 끊김으로
        // 돌아와, 상한이 사라진 판에서도 이 시험이 푸르게 선다. 시험이 끝나 `listener` 가
        // 떨어질 때까지 든다 — 상한의 몇 곱절을 자는 대신 채널 하나로 기다린다.
        let (done, wait) = std::sync::mpsc::channel::<()>();
        let held = std::thread::spawn(move || {
            let held = listener.accept().map(|(s, _)| s);
            let _ = wait.recv();
            drop(held);
        });
        let cut = Duration::from_millis(300);
        let began = std::time::Instant::now();
        assert_eq!(ask_within(&url, cut).map_err(|w| w.kind), Err(Trouble::Timeout));
        let took = began.elapsed();
        // **양쪽을 잰다.** 위만 재면 끊김으로 즉시 돌아온 판이 통과하고, 아래만 재면 영영
        // 기다리는 판이 통과한다. 위는 넉넉히 둔다 — 부하 20~27 에서도 붉어지지 않게.
        assert!(took >= cut, "상한 앞에서 돌아왔다 — 끊긴 것이지 끊은 것이 아니다 ({took:?})");
        assert!(took < Duration::from_secs(10), "상한을 넘겨 기다렸다 — {took:?}");
        drop(done);
        let _ = held.join();
    }

    /// **기계가 가를 낱말은 갈래마다 다르고 안 바뀐다**(moai-580l). 아직 아무 표면도 이 값을
    /// 안 내므로, 낱말이 어긋나는 것을 잡을 자리가 여기 말고 없다 — `commits_error` 의
    /// `kind` 와 같은 꼴(`{kind, said}`)로 서는지를 글자로 잰다.
    ///
    /// **낱말을 손으로 적는 함수를 곁에 두지 않는다** — 그러면 갈래와 두 벌이 되어 한쪽만
    /// 고치는 날이 온다. 짓는 자는 직렬화 하나고, 이 시험이 그 값을 못 박는다.
    #[test]
    fn the_kind_words_are_the_contract() {
        let words = [
            (Trouble::NotAsked, "not_asked"),
            (Trouble::Offline, "offline"),
            (Trouble::Timeout, "timeout"),
            (Trouble::RateLimited, "rate_limited"),
            (Trouble::Http, "http"),
            (Trouble::Tls, "tls"),
            (Trouble::Garbled, "garbled"),
            (Trouble::OddTag, "odd_tag"),
            (Trouble::Failed, "failed"),
        ];
        for (kind, word) in words {
            let why = Why { kind, said: "그 까닭".into() };
            let got = serde_json::to_string(&why).expect("못 적었다");
            assert_eq!(got, format!(r#"{{"kind":"{word}","said":"그 까닭"}}"#), "{kind:?}");
        }
        let said: std::collections::BTreeSet<&str> = words.iter().map(|(_, w)| *w).collect();
        assert_eq!(said.len(), words.len(), "같은 낱말을 두 갈래가 쓴다");
        // **빠진 갈래가 없다.** 목록은 손으로 적는 것이라 갈래를 더하고 잊을 수 있어, 아래
        // `match` 가 그 자리를 **컴파일에서** 댄다 — 시험이 붉어지는 것보다 이르다.
        let _every = |kind: Trouble| match kind {
            Trouble::NotAsked
            | Trouble::Offline
            | Trouble::Timeout
            | Trouble::RateLimited
            | Trouble::Http
            | Trouble::Tls
            | Trouble::Garbled
            | Trouble::OddTag
            | Trouble::Failed => (),
        };
    }

    /// **거짓말은 갈래가 둘이다**(moai-580l). 답을 못 읽은 것([`Trouble::Garbled`])과 태그는
    /// 받았는데 판으로 못 읽은 것([`Trouble::OddTag`])은 고칠 자리가 다르다 — 앞은 묻는 자리가
    /// 이상한 것이고, 뒤는 릴리스를 지은 쪽이 태그를 그렇게 달았다는 뜻이다.
    #[test]
    fn an_answer_that_lies_falls_back_to_unasked() {
        // 제어문자가 든 태그도 `Garbled` 다 — 판 줄에 그대로 서는 글이라 들이는 자리에서 물린다.
        for (body, kind) in [
            (r#"{"tag_name":"#, Trouble::Garbled),
            (r#"{"tag_name":42}"#, Trouble::Garbled),
            (r#"{"name":"v9.9.9"}"#, Trouble::Garbled),
            ("{\"tag_name\":\"v9.9.9-\\u001b[2J\"}", Trouble::Garbled),
            ("", Trouble::Garbled),
            (r#"{"tag_name":"세판"}"#, Trouble::OddTag),
        ] {
            let s = Scratch::new("latest-lies");
            let (url, handle) = server_once(body);
            let got = refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW);
            assert_eq!(why_of(&got), Some(kind), "{body}");
            let _ = handle.join();
        }
    }

    #[test]
    fn a_tag_with_a_control_character_is_not_even_written_down() {
        // 판 줄에 그대로 서는 글이라 들이지 않고, 들이지 않았으니 파일에도 안 남는다 —
        // 한 번 적히면 다시 물을 때까지 하루를 그 바이트가 산다.
        let s = Scratch::new("latest-escape");
        let (url, handle) = server_once("{\"tag_name\":\"v9.9.9-\\u001b[2J\"}");
        // 들이는 자리가 물렸으니 **읽지 못한 답**이지, 태그가 이상한 것이 아니다.
        let got = refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW);
        assert_eq!(why_of(&got), Some(Trouble::Garbled));
        assert_eq!(read(s.path()).and_then(|h| h.tag), None, "제어문자가 든 태그를 적어 두었다");
        let _ = handle.join();
    }

    #[test]
    fn a_cache_file_a_person_broke_is_read_as_absent() {
        let s = Scratch::new("latest-broken");
        std::fs::write(file_at(s.path()), "asked_at = [\n").unwrap();
        assert!(read(s.path()).is_none());
        std::fs::write(file_at(s.path()), "asked_at = 3\n").unwrap();
        assert!(read(s.path()).is_none(), "때가 낱말이 아닌데 읽었다");
        assert_eq!(why_of(&held(s.path())), Some(Trouble::NotAsked));
    }

    #[test]
    fn what_it_wrote_it_reads_back() {
        let s = Scratch::new("latest-roundtrip");
        for tag in [Some("v0.1.0"), None] {
            let held = Held { asked_at: "2026-09-21T00:00:00Z".into(), tag: tag.map(String::from) };
            write(s.path(), &held).unwrap();
            assert_eq!(read(s.path()).as_ref(), Some(&held));
        }
    }

    #[test]
    fn a_key_it_does_not_know_survives_a_write() {
        // 판이 다른 바이너리들이 한 파일을 쓴다 — 새 판이 적은 키를 옛 판이 한 번 만져
        // 지우면, 이 모듈이 선 자리(판이 섞이는 기계)가 곧 그 손실이 나는 자리다.
        let s = Scratch::new("latest-unknown");
        std::fs::write(file_at(s.path()), "# 사람이 적은 줄\nasked_at = \"2026-09-20T00:00:00Z\"\netag = \"W/abc\"\n")
            .unwrap();
        write(s.path(), &Held { asked_at: "2026-09-21T00:00:00Z".into(), tag: Some("v0.2.0".into()) }).unwrap();
        let src = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(src.contains("etag = \"W/abc\""), "모르는 키가 사라졌다 — {src}");
        assert!(src.contains("# 사람이 적은 줄"), "곁의 주석이 사라졌다 — {src}");
        assert_eq!(read(s.path()).unwrap().tag.as_deref(), Some("v0.2.0"));
    }

    #[test]
    fn the_two_switches_and_the_screen_each_stop_it() {
        let none = |_: &str| None;
        assert_eq!(gate(none, None, false, true), None);
        assert_eq!(gate(none, Some(true), false, true), None);
        assert_eq!(gate(|k| (k == OFF_VAR).then(|| OsString::from("1")), None, false, true), Some(Off::Env));
        // 빈 값은 없는 것이다 — 환경변수의 관례.
        assert_eq!(gate(|k| (k == OFF_VAR).then(OsString::new), None, false, true), None);
        assert_eq!(gate(none, Some(false), false, true), Some(Off::Config));
        assert_eq!(gate(none, None, false, false), Some(Off::NotAScreen));
        // **터미널에서 친 `--json` 도 화면이 아니다.** 에이전트는 tmux 칸 안에서 친다.
        assert_eq!(gate(none, None, true, true), Some(Off::NotAScreen));
        // 껐는데 화면도 아니면 먼저 만난 까닭을 든다 — 어느 쪽이든 안 묻는 것은 같다.
        assert_eq!(gate(none, Some(false), false, false), Some(Off::Config));
    }

    #[test]
    fn the_setting_reads_only_the_switch_that_turns_it_off() {
        let s = Scratch::new("latest-config");
        let at = s.path().join("config.toml");
        let read = |src: &str| {
            std::fs::write(&at, src).unwrap();
            config_says(Some(&at))
        };
        assert_eq!(read("[update]\ncheck = false\n"), Some(false));
        assert_eq!(read("[update]\ncheck = true\n"), Some(true));
        // 꼴이 아닌 값도, 없는 표도, 없는 파일도 모두 "안 적었다" 다 — 기본은 켬이다.
        assert_eq!(read("[update]\ncheck = \"no\"\n"), None);
        assert_eq!(read("[update]\n"), None);
        assert_eq!(read("[i18n]\nlang = \"ko\"\n"), None);
        assert_eq!(read("check = false\n"), None, "표 밖의 같은 이름을 읽었다");
        assert_eq!(read("[update]\ncheck = [\n"), None, "깨진 설정이 막지 않는다");
        assert_eq!(config_says(Some(&s.path().join("없다.toml"))), None);
        assert_eq!(config_says(None), None, "설정 파일이 어디인지 모르는 기계");
    }

    #[test]
    fn its_own_machine_is_told_apart_from_the_grid() {
        // 프록시가 선 기계에서 이 갈래가 틀리면 시험이 붉어지는 대신 매달린다.
        assert!(is_loopback("http://127.0.0.1:41234/releases/latest"));
        assert!(is_loopback("http://127.0.0.1:1/releases/latest"));
        assert!(is_loopback("http://LocalHost/x"));
        assert!(is_loopback("http://[::1]:8080/x"));
        assert!(is_loopback("http://user@127.9.9.9/x"), "127/8 은 통째로 되돌이다");
        assert!(!is_loopback(API));
        assert!(!is_loopback("http://127.0.0.1.example.com/x"), "이름 속의 숫자는 되돌이가 아니다");
        assert!(!is_loopback("http://10.0.0.1/x"));
    }

    #[test]
    fn the_place_it_asks_can_be_turned() {
        assert_eq!(url_from(|_| None), API);
        assert_eq!(url_from(|k| (k == "MOAI_API_URL").then(|| OsString::from("http://x/y"))), "http://x/y");
        assert_eq!(url_from(|k| (k == "MOAI_API_URL").then(OsString::new)), API, "빈 값은 없는 것이다");
    }

    #[test]
    fn the_thread_hands_the_answer_over() {
        let s = Scratch::new("latest-thread");
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let (rx, job) =
            spawn(s.path().to_path_buf(), url, "2026-09-21T00:00:00Z".into(), WINDOW).expect("실을 못 띄웠다");
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(20)).expect("답이 안 왔다"),
            Seen::Newer { tag: "v9.9.9".into() }
        );
        let _ = job.join();
        let _ = handle.join();
    }

    #[test]
    fn the_version_it_compares_against_is_the_one_it_was_built_with() {
        // 판 줄에 서는 값과 견주는 값이 갈리면, 화면은 "새 판" 인데 받아 보면 같은 판이다.
        // **`mine()` 을 `env!` 와 견주지 않는다** — 그 함수의 몸통이 바로 그 매크로라 제 자신과
        // 견주는 꼴이고, 어떤 고침도 그 줄을 못 붉힌다. 재는 것은 견주는 길 전체다.
        assert_eq!(seen(mine(), Some(&format!("v{}", mine()))), Seen::Same);
        assert_eq!(seen(mine(), Some(mine())), Seen::Same, "`v` 없는 태그");
    }
}
