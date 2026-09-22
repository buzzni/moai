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
//! - **하루 한 번 묻는다**([`WINDOW`]). 답과 물은 때와 **물은 자리**(moai-dael), 못 들었으면
//!   그 **까닭**(2026-09-22 사용자 결정)을 [`FILE`] 에 적고, 같은 자리의 답이 그 안이면 안
//!   묻는다.
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
//! 갈래마다 다르다**: 시간당 부름 수를 태운 것은 기다리면 풀리고, 프록시가 인증서를 갈아 끼운
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
/// 물은 자리(moai-dael). **옛 파일에는 없다** — 없으면 어디에 물은 답인지 모르는 것이고,
/// 모르는 답은 다시 묻는다.
pub const URL: &str = "url";
/// 못 들었으면 그 까닭([`Trouble`], 2026-09-22 사용자 결정). 태그를 든 줄에는 **없다** —
/// 파일이 드는 것은 답 하나고, 태그가 곧 그 답이다.
pub const TROUBLE: &str = "trouble";

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

/// 설정에서 이것을 끄는 자리 — `[update] check = false`. **읽는 자는 여기가 아니다**
/// ([`crate::user_config::Doc::update_check`], moai-d74q): 설정 파일은 한 번만 판다. 이름만
/// 여기 두는 것은 이 기능의 것이기 때문이고, 두 벌로 적으면 이름을 고치는 날 한쪽이 낡는다.
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
    /// 때문이다. 시간당 부름 수를 다 태운 것은 기다리면 풀리고, 프록시가 인증서를 갈아 끼운
    /// 네트워크는 이 기능을 못 쓴다는 뜻이며, 네트워크가 없는 것은 둘 중 어느 것도 아니다.
    Unasked(Why),
}

/// 못 물은 까닭 — **자료지 글이 아니다**. 꼴은 `show --json` 의 `commits_error`
/// ([`crate::git::Told`])와 같다: 가르는 것은 `kind` 고, `said` 는 사람이 까닭을 볼 한 줄이다.
///
/// **`said` 는 라이브러리와 운영체제가 지은 글이라 안 옮긴다** — [`crate::git::Error::said`]
/// 와 같은 자리다. 사람에게 댈 한 낱말은 화면이 [`Trouble`] 로 고른다.
///
/// **직렬화를 안 얹는다.** `--json` 에 실릴 날 그 자리가 [`Trouble::name`] 을 부르면 된다 —
/// 낱말을 짓는 자가 둘이 되면 파일에 적는 낱말과 기계에 내는 낱말이 갈린다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Why {
    pub kind: Trouble,
    pub said: String,
}

/// 못 물은 까닭의 갈래. **낱말이 아니라 타입이다** — 화면이 이것으로 갈라 그리므로, 갈래를
/// 더하는 날 컴파일러가 안 고친 자리를 이름으로 댄다.
///
/// **기계에 낼 낱말은 [`Trouble::name`] 하나가 짓고 [`Trouble::from_name`] 이 되읽는다** —
/// `not_asked`·`rate_limited` 처럼 `commits_error` 의 `kind` 와 같은 꼴이다. 왕복이 필요한 것은
/// 이 낱말이 [`FILE`] 에 적히기 때문이고([`TROUBLE`]), 그래서 **짓는 자를 한자리에 둔다** —
/// 직렬화와 손으로 적은 표를 함께 두면 한쪽만 고치는 날이 온다. 그 둘이 실제로 맞물리는지는
/// `the_kind_words_are_the_contract` 가 잰다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// 아직 안 물었다 — 적어 둔 답이 없거나, 물었지만 태그를 못 들은 채 하루가 안 지났다.
    /// 끈 사람도 여기로 온다([`Off`]): 제가 끈 것은 제가 안다.
    NotAsked,
    /// 네트워크가 없다 — 이름을 못 풀거나, 붙지 못하거나, 소켓이 끊겼다.
    Offline,
    /// [`TIMEOUT`] 안에 못 끝냈다. 느린 네트워크와 안 붙는 네트워크는 다른 일이다.
    Timeout,
    /// 시간당 부름 수를 다 태웠다 — GitHub 은 이것을 403 으로도 429 로도 낸다. **기다리면
    /// 풀리고 사람이 할 것이 없는 갈래라** 따로 둔다: 에이전트가 도는 고리가 이 자리에 자주 선다.
    RateLimited,
    /// 서버가 그 밖의 상태 코드를 냈다 — 4xx 도 5xx 도 여기 든다. 404(저장소를 옮겼다)가 가장
    /// 흔하고, 그때는 사람이 고칠 자리가 있다.
    ///
    /// **5xx 는 고칠 자리가 없다**(리뷰). 그쪽은 [`RateLimited`](Trouble::RateLimited) 와
    /// 마찬가지로 기다리는 것이 할 일의 전부라, 화면 글을 "거절" 이라 적던 판은 GitHub 한 번
    /// 흔들린 것을 두고 저장소가 옮겨 갔는지 토큰이 틀렸는지 찾아다니게 했다. 그래도 갈래를 더
    /// 쪼개지 않는 것은 그 둘을 가를 자가 상태 코드 하나뿐이고, 이 값을 내는 표면이 아직
    /// 탐색기 한 줄이기 때문이다 — 글은 상태 코드를 안 고르는 쪽으로 적는다.
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

impl Trouble {
    /// 파일과 기계가 가를 낱말.
    pub fn name(self) -> &'static str {
        match self {
            Trouble::NotAsked => "not_asked",
            Trouble::Offline => "offline",
            Trouble::Timeout => "timeout",
            Trouble::RateLimited => "rate_limited",
            Trouble::Http => "http",
            Trouble::Tls => "tls",
            Trouble::Garbled => "garbled",
            Trouble::OddTag => "odd_tag",
            Trouble::Failed => "failed",
        }
    }

    /// [`name`](Trouble::name) 이 지은 낱말을 되읽는다. **모르는 낱말은 `None` 이다** — 새
    /// 바이너리가 적은 갈래를 옛 바이너리가 읽는 자리라, 모르면 까닭을 모르는 것이지 줄이
    /// 깨진 것이 아니다([`read`] 는 관대하다).
    pub fn from_name(word: &str) -> Option<Trouble> {
        [
            Trouble::NotAsked,
            Trouble::Offline,
            Trouble::Timeout,
            Trouble::RateLimited,
            Trouble::Http,
            Trouble::Tls,
            Trouble::Garbled,
            Trouble::OddTag,
            Trouble::Failed,
        ]
        .into_iter()
        .find(|k| k.name() == word)
    }
}

impl Why {
    /// 아직 안 물었다 — `said` 는 빈 글이다. 적어 둘 까닭이 없다.
    pub fn not_asked() -> Self {
        Why { kind: Trouble::NotAsked, said: String::new() }
    }

    /// `ureq` 가 낸 실패를 갈래로. **`said` 는 그 크레이트가 지은 글 그대로다.**
    ///
    /// **403 과 429 를 한 갈래로 접는다.** GitHub 은 시간당 부름 수를 다 태운 부름에 둘 중
    /// 아무 쪽이나 내고(문서가 그렇게 적는다), 사람이 할 일은 둘 다 "기다린다" 다.
    /// `User-Agent` 를 안 단 부름도 403 인데, 그쪽은 [`ask_within`] 이 늘 달고 나가므로
    /// 이 자리에 안 선다.
    fn from_ureq(e: &ureq::Error) -> Self {
        let kind = match e {
            ureq::Error::StatusCode(403 | 429) => Trouble::RateLimited,
            ureq::Error::StatusCode(_) => Trouble::Http,
            ureq::Error::Timeout(_) => Trouble::Timeout,
            // **TLS 의 실패는 `Io` 로 온다**(moai-t906 이 재어 알아낸 자리). 핸드셰이크가 깨지면
            // rustls 의 오류가 `io::Error` 에 싸여 오고, `ureq::Error::Rustls` 는 그 길에 안
            // 선다 — 갈래를 그 변형으로만 재던 판은 프록시가 인증서를 갈아 끼운 기계에
            // "네트워크가 없다" 고 말했다. rustls 는 그 오류를 `InvalidData` 로 싼다.
            ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::InvalidData => Trouble::Tls,
            ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => Trouble::Offline,
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) | ureq::Error::Pem(_) => Trouble::Tls,
            // **우리가 건 상한에 걸린 것은 못 읽는 답이다**(리뷰가 적어 둔 자리). 답이
            // 256KB 를 넘는 것은 그 자리가 릴리스 API 가 아니라는 뜻이라, 사람이 할 일은
            // 깨진 JSON 을 받았을 때와 같다 — `MOAI_API_URL` 이 가리키는 자리를 본다.
            ureq::Error::BodyExceedsLimit(_) => Trouble::Garbled,
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
    /// **어디에 물었나**(moai-dael). 옛 바이너리가 적은 줄에는 없어 `None` 이다.
    ///
    /// 이것이 없던 때는 `MOAI_API_URL` 을 한 번 바꿔 부른 답이 하루 동안 **진짜 부름에 서고**,
    /// 거울을 쓰는 사람에게는 그 반대가 됐다 — 창이 자리를 안 봤기 때문이다.
    ///
    /// **적어 둔 까닭과 함께 서지 않는다** — [`Held::trouble`] 은 태그가 없을 때만 선다.
    ///
    /// **사용자 정보는 떼고 든다**([`place_of`], 리뷰). 이 값은 파일에 적히는데, 그 파일의 첫
    /// 쓰기는 umask 를 따라 남이 읽을 수 있다([`crate::store::write_atomic`] 은 **이미 있는**
    /// 파일의 권한만 지킨다). `MOAI_API_URL` 에 토큰을 끼워 둔 사람의 그 토큰이 설정 디렉터리에
    /// 평문으로 하루를 사는 길이었다 — 묻는 자리인가만 가리면 되므로 그 토막은 애초에 필요 없다.
    pub url: Option<String>,
    /// 못 들었으면 그 까닭(2026-09-22 사용자 결정, 리뷰가 연 자리). **태그를 든 줄에는 없다** —
    /// 파일이 드는 것은 답 하나고, 태그가 섰으면 그것이 답이다.
    ///
    /// 안 적던 판은 **까닭이 그 부름 한 판에만** 섰다. 창이 하루라 두 번째 부름부터는
    /// [`Trouble::NotAsked`] 한 글로 접혔고, 그러면 인증서를 갈아 끼우는 프록시 뒤의 사람은
    /// "TLS 실패" 를 하루에 한 번만 보고 나머지는 "최신 확인 안 함" 만 봤다 — 갈래를 나눈
    /// 까닭이 그 자리에서 사라졌다.
    ///
    /// **`said` 는 안 적는다.** 그것은 `ureq` 가 이 부름에 대고 지은 글이라 다음 부름의 것이
    /// 아니고, 파일에 남길 만큼 오래 참인 값도 아니다 — 되읽은 까닭의 `said` 는 빈 글이다.
    pub trouble: Option<Trouble>,
}

/// 자리를 가릴 때 쓸 꼴 — **사용자 정보(`user:pass@`)를 뗀 `url`**(리뷰).
///
/// [`Held::url`] 이 적어 두는 값이자 [`Held::asked_here`] 가 견주는 값이다. 양쪽이 같은 자를
/// 지나야 토큰만 다른 두 부름이 같은 자리로 선다 — 토큰은 자리가 아니다.
///
/// **꼴이 아닌 글은 그대로 둔다.** 이 값은 `MOAI_API_URL` 이 준 남의 글이고, 여기가 하는 일은
/// 자리를 가리는 것 하나다 — 못 알아보는 글을 고쳐 쓰기 시작하면 그 글이 가리키던 자리가
/// 바뀐다. 자르는 자는 [`is_loopback`] 이 권한을 떼는 자와 같다.
pub fn place_of(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let cut = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(cut);
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    format!("{scheme}://{host}{tail}")
}

impl Held {
    /// 이 답이 `url` 에 물은 것인가(moai-dael). **자리를 모르는 옛 줄은 아니라고 본다** —
    /// 읽기는 관대하되(줄을 안 버린다) 이 물음에는 엄하다. 모르는 자리의 답을 이 자리의 답으로
    /// 세우는 것이 이 이슈가 없앤 그 일이다.
    ///
    /// **자리는 한 칸이다**(2026-09-22 사용자 결정). 자리마다 한 줄을 드는 표로 갈 수도 있었고
    /// 그러면 아래 둘이 함께 닫히지만, 둘 다 좁은 경우라 꼴을 한 번 더 바꾸지 않기로 했다.
    ///
    /// - `MOAI_API_URL` 을 **오가는** 사람은 부를 때마다 묻게 된다(창이 안 닫힌다). 그 값은
    ///   거울이나 시험을 위한 손잡이라 한 기계에서 오가는 일이 드물다
    /// - 옛 바이너리가 이 파일을 쓰면 `tag` 만 갈리고 [`Held::url`] 은 그대로 남아, 남의
    ///   자리에서 들은 태그가 이 자리의 답으로 읽힐 수 있다. 옛 바이너리와 `MOAI_API_URL` 이
    ///   **함께** 서야 하는 자리다
    ///
    /// **치르는 값은 판올림 뒤 한 번 더 묻는 것에서 그치지 않는다**(리뷰). 그 한 번마저
    /// 실패하면 [`refresh`] 가 태그 없는 줄을 적어, 자리를 모르던 옛 태그가 파일에서
    /// **지워진다** — `refresh` 의 "못 들었다고 알던 것을 지우지는 않는다" 는 자리를 아는 줄에만
    /// 선다. 모르는 자리의 태그를 이 자리의 답으로 안 세우기로 한 이상 그 태그는 들고 갈 데가
    /// 없고, 남겨 두면 다음 판이 그것을 이 자리의 답으로 읽는다.
    pub fn asked_here(&self, url: &str) -> bool {
        self.url.as_deref() == Some(place_of(url).as_str())
    }

    /// 이 답이 아직 창 안인가. 때가 꼴이 아니면 낡은 것으로 본다 — 못 읽는 도장 하나가 묻기를
    /// 영영 막으면, 그 파일을 지우는 것 말고 되돌릴 길이 없다.
    ///
    /// **조금 앞선 도장은 봐준다**([`crate::model::FUTURE_SLACK_SECS`], moai-21un). 설정
    /// 디렉터리를 나눠 쓰는 두 기계의 시계가 1초만 어긋나도 한쪽이 찍은 도장이 다른 쪽에는
    /// 미래로 보여, 그 기계는 부를 때마다 바깥을 두드렸다 — 에이전트가 도는 고리에서는
    /// GitHub 의 시간당 60번을 태우는 길이다. 봐주는 자는 이 저장소가 이미 쓰던 것과 같다
    /// (`report` 의 `far_ahead`, `model::days_since` 의 0 클램프).
    ///
    /// **크게 앞선 도장은 여전히 낡은 것이다.** 시계를 되돌린 기계나 손으로 적은 2099 가 창을
    /// 영영 열어 두지 못하게 한다.
    ///
    /// **봐주는 폭이 창과 같다는 것은 재 볼 자리다**(리뷰). [`crate::model::FUTURE_SLACK_SECS`]
    /// 도 [`WINDOW`] 도 하루라, 하루 앞선 도장을 든 기계는 이틀까지 안 묻는다 — 모듈 머리의
    /// "하루 한 번 묻는다" 는 시계가 맞는 기계의 이야기다. 한자리에 둔 값을 그대로 쓰는 쪽을
    /// 골랐으나, `report` 의 `far_ahead` 를 늘리는 날 이쪽 주기가 함께 늘어난다.
    /// 이 줄이 드는 답. 태그가 섰으면 그것이 답이고, 없으면 **적어 둔 까닭**이다
    /// (2026-09-22 사용자 결정) — 되읽은 까닭의 `said` 는 빈 글이다([`Held::trouble`]).
    ///
    /// **[`refresh`] 와 [`held`] 가 한 자를 쓴다.** 둘이 따로 풀던 판은 창 안에서 낸 답과 첫
    /// 프레임의 답이 갈릴 자리였다.
    pub fn answer(&self) -> Seen {
        match &self.tag {
            Some(tag) => seen(mine(), Some(tag)),
            None => Seen::Unasked(Why { kind: self.trouble.unwrap_or(Trouble::NotAsked), said: String::new() }),
        }
    }

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
    let url = doc.get(URL).and_then(|i| i.as_str()).map(String::from);
    let trouble = doc.get(TROUBLE).and_then(|i| i.as_str()).and_then(Trouble::from_name);
    Some(Held { asked_at, tag, url, trouble })
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
/// 진 쪽도 잃는 것이 없다. 같은 프로세스의 스레드 둘도 같다 — [`crate::store::tmp_name`] 이 임시
/// 이름을 스레드마다 가르므로(moai-mpf4) 둘 중 한쪽의 글이 통째로 남는다. [`spawn`] 을 한 번에
/// 하나만 띄우는 것은 여전히 부르는 쪽의 몫이지만, 그것은 스레드와 소켓을 아끼자는 것이지 파일이
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
    let mut put = |key: &str, value: &Option<String>| match value {
        Some(v) => doc[key] = toml_edit::value(v.as_str()),
        None => {
            doc.remove(key);
        }
    };
    put(TAG, &held.tag);
    put(URL, &held.url);
    put(TROUBLE, &held.trouble.map(|k| k.name().to_string()));
    write_atomic(&file_at(dir), doc.to_string().as_bytes())
}

/// 왜 안 묻는지 — **글이 아니라 자료다**. 지금은 아무도 이것을 펴지 않는다: 끈 것은 제가 끈
/// 사람이 알기 때문이다.
///
/// **못 물은 까닭이 갈린 뒤에도 이 셋은 안 갈렸다**(moai-580l, 리뷰). [`Seen::Unasked`] 는
/// 이제 아홉 글로 서지만 이 셋은 모두 [`Trouble::NotAsked`] 한 글로 접힌다 — 갈래를 나눈
/// 까닭이 "사람이 할 일이 갈래마다 다르다" 인데, 제가 끈 것에는 할 일이 없다.
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

/// 한 번 묻는다. 태그를 못 얻으면 **까닭을 든다**([`Why`], moai-580l).
///
/// `User-Agent` 를 다는 것은 GitHub 가 없는 부름에 403 을 주기 때문이다.
pub fn ask(url: &str) -> Result<String, Why> {
    ask_within(url, TIMEOUT)
}

/// 이 자리를 `https` 로 부르는가 — 되돌림이 평문으로 내려가는 것을 막을지 가른다([`agent_for`]).
///
/// **스킴은 대소문자를 안 가린다**(RFC 3986). `HTTPS://` 를 평문으로 읽으면 그 부름만 문이 안
/// 서고, 그것은 조용히 약해지는 쪽이다.
fn secure_scheme(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("https"))
}

/// 물을 때 쓰는 `ureq::Agent` 하나 — **[`ask_within`] 에서 갈라 두었다**(moai-t906).
///
/// 시험의 부름은 모두 `http://127.0.0.1` 인데 생산의 부름은 모두 `https` 라, 붙여 두면 TLS 를
/// 어떻게 세웠는지가 **한 번도 안 재인 채로** 산다 — 기능 조합이 바뀌어 인증서 검증이 꺼져도
/// 온 시험이 푸르고, 생산에서는 "못 물었다" 로 접혀 네트워크 없음과 구별이 안 간다. 갈라 두면
/// 시험이 이 자리의 설정을 그대로 읽는다(`the_tls_it_builds_is_rustls_over_mozillas_roots`).
fn agent_for(url: &str, timeout: Duration) -> ureq::Agent {
    let built = ureq::Agent::config_builder()
        // **머리부터 몸까지 통째로 잰다.** 붙기만 재면 붙여 놓고 한 글자씩 흘리는 자리에
        // 영영 붙들린다.
        .timeout_global(Some(timeout))
        // 되돌림은 **둘까지만** 따라간다(기본은 열이다). 저장소 이름이 바뀌면 릴리스 API 가 301
        // 로 보내므로 아예 안 따라가면 그날 이 기능이 죽는다. 그래도 열까지 갈 까닭은 없고,
        // 셋째부터는 `max_redirects_will_error` 가 오류로 세 "못 물었다" 가 된다.
        .max_redirects(2)
        // **되돌림이 평문으로 내려가지 못하게 막는다**(리뷰). ureq 의 기본은 `https_only: false`
        // 고, 그 문은 되돌림 **한 걸음마다** 다시 선다(`run::call_run`) — 안 세우면 `https` 로
        // 나간 부름이 `302 Location: http://…` 한 줄에 평문으로 내려가고, 거기서 받은 태그가
        // 탐색기 판 줄에 서서 [`FILE`] 에 하루를 산다. SECURITY.md 가 "TLS 는 rustls 에
        // webpki-roots, 검증 켬" 이라 적은 것은 그 첫 걸음만의 이야기였다.
        //
        // **처음부터 평문으로 부른 자리는 그대로 둔다.** `https_only` 는 전부 아니면 전무라,
        // 늘 켜면 `MOAI_API_URL=http://거울…` 을 손으로 적은 사람이 그날로 못 쓴다 — 그것은
        // 그 사람이 고른 것이고, 여기서 막을 것은 **고르지 않은 내려감**이다.
        .https_only(secure_scheme(url));
    // **되돌이 자리는 프록시를 안 탄다.** ureq 의 기본은 `ALL_PROXY`·`HTTPS_PROXY`·`HTTP_PROXY`
    // 를 그대로 따르고 `NO_PROXY` 에 손으로 적은 이름만 비껴간다 — 되돌이를 기본으로 비껴가지
    // 않는다. 바깥으로 나가는 부름에는 그 편이 맞지만(회사 그물은 프록시로만 나간다), 제 서버를
    // 띄워 붙는 이 모듈의 시험은 프록시가 선 기계에서 그리로 나가 버린다. 그러면 서버는 손님을
    // 못 만나 `accept` 에서 멈추고, `cargo test` 는 붉어지는 대신 **영영 매달린다**. curl 도
    // 7.86 부터 되돌이를 비껴간다.
    if is_loopback(url) { built.proxy(None) } else { built }.build().into()
}

/// [`ask`] 되 기다리는 상한을 받는다. **시험이 그 상한을 짧게 줘서 실제로 끊기는지 잰다** —
/// 상한을 상수로만 두면 그것이 서는지를 5초씩 기다려야만 볼 수 있고, 그러면 아무도 안 잰다.
pub fn ask_within(url: &str, timeout: Duration) -> Result<String, Why> {
    let agent = agent_for(url, timeout);
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
        // **바이트로 받아 우리가 씻는다**(리뷰, moai-580l 이 연 자리). `read_to_string` 에
        // `lossy_utf8(true)` 를 주던 판은 이 자리에서 아무 일도 안 했다 — ureq 는 그 설정을
        // `Content-Type` 이 `text/` 로 시작할 때만 입히는데(`body::ResponseInfo::is_text`),
        // GitHub 은 `application/json` 으로 답한다. 그러면 씻는 자가 안 서서 `read_to_string`
        // 이 깨진 바이트에 `io::ErrorKind::InvalidData` 로 죽고, 그 오류가 [`Why::from_ureq`]
        // 의 **TLS 갈래**로 떨어져 핸드셰이크가 멀쩡한 기계에 "TLS 실패" 가 섰다 — 그 글은 인증서를
        // 갈아 끼우는 프록시 뒤라 영영 못 쓴다는 뜻이라(moai-uwuw), 고칠 것이 없는 사람을
        // 고치러 보낸다.
        .read_to_vec()
        .map_err(|e| Why::from_ureq(&e))?;
    // 글자가 깨져도 읽는다 — 어차피 `tag_name` 하나만 집고, 못 집으면 "못 물었다" 다.
    let body = String::from_utf8_lossy(&body);
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
    // **다른 자리에 물은 답은 이 자리의 답이 아니다**(moai-dael) — 창도 안 닫고, 못 들었을 때
    // 들고 갈 지난 답도 안 된다. 거울을 한 번 보고 온 사람에게 그 태그가 진짜 릴리스로 서던
    // 자리다.
    let held = read(dir).filter(|h| h.asked_here(url));
    if let Some(h) = &held
        && h.fresh(now, window)
    {
        return h.answer();
    }
    let asked = ask(url);
    let why = asked.as_ref().err().cloned();
    let tag = asked.ok().or_else(|| held.and_then(|h| h.tag));
    // **까닭은 태그가 없을 때만 적는다** — 태그가 섰으면 그것이 답이고, 곁에 까닭을 두면 파일이
    // 답을 둘 드는 셈이 된다. 들은 판은 까닭을 지운다(`put` 이 키를 뺀다).
    let trouble = tag.is_none().then(|| why.as_ref().map_or(Trouble::NotAsked, |w| w.kind));
    let now = Held { asked_at: now.to_string(), tag, url: Some(place_of(url)), trouble };
    let answer = now.answer();
    let _ = write(dir, &now);
    // **적어 둔 까닭에는 `said` 가 없다**(위). 이 판의 까닭은 그 글을 들고 있으므로 그대로 낸다.
    match (answer, why) {
        (Seen::Unasked(_), Some(why)) => Seen::Unasked(why),
        (answer, _) => answer,
    }
}

/// 적어 둔 답만 읽는다 — **묻지 않는다.** 창이 지났어도 지난 답을 그대로 낸다: 그리는 쪽이
/// 첫 프레임에 쓸 값이고, 새 답은 [`spawn`] 이 받아 뒤에 얹는다.
pub fn held(dir: &Path, url: &str) -> Seen {
    // **자리를 함께 묻는다**(moai-dael) — 첫 프레임이 다른 자리의 답을 이 자리의 답으로 그리면,
    // [`refresh`] 가 그것을 버리기 전까지 거짓이 서 있다.
    read(dir).filter(|h| h.asked_here(url)).map_or_else(|| Seen::Unasked(Why::not_asked()), |h| h.answer())
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

    /// **서버가 낸 상태 코드도 갈린다**(moai-580l). 시간당 부름 수를 태운 것은 기다리면 풀리고,
    /// 옮겨 간 저장소는 사람이 고칠 것이 다르다 — 한 글로 접으면 어느 쪽인지 모른다.
    ///
    /// **403 과 429 는 한 갈래다.** GitHub 은 부름 수를 태운 부름에 둘 중 아무 쪽이나 낸다.
    #[test]
    fn a_refusal_says_whether_waiting_will_fix_it() {
        for (status, kind) in [
            ("403 Forbidden", Trouble::RateLimited),
            ("429 Too Many Requests", Trouble::RateLimited),
            ("404 Not Found", Trouble::Http),
            ("500 Internal Server Error", Trouble::Http),
        ] {
            let (url, handle) = saying_once(status, "{}");
            let got = ask_within(&url, TIMEOUT).map_err(|w| w.kind);
            assert_eq!(got, Err(kind), "{status}");
            let _ = handle.join();
        }
    }

    /// **성치 않은 바이트가 든 답은 "TLS 실패" 가 아니다**(리뷰). `read_to_string` 에
    /// `lossy_utf8(true)` 를 주던 판은 이 자리에서 아무 일도 안 했다 — ureq 는 그 설정을
    /// `Content-Type` 이 `text/` 로 시작할 때만 입히는데 GitHub 은 `application/json` 으로
    /// 답한다. 그래서 깨진 바이트 하나가 `io::ErrorKind::InvalidData` 로 죽고, 그 오류가
    /// [`Why::from_ureq`] 의 TLS 갈래로 떨어져 핸드셰이크가 멀쩡한 기계에 "인증서를 갈아 끼우는
    /// 프록시 뒤라 영영 못 쓴다" 는 뜻의 글이 섰다.
    ///
    /// **씻어 읽으므로 태그는 그대로 집힌다** — 깨진 바이트가 태그 밖에 있으면 성한 답이다.
    #[test]
    fn a_broken_byte_in_the_answer_is_not_a_tls_failure() {
        // 태그는 성하고 딴 자리가 깨졌다 — 씻어 읽으니 그대로 집는다.
        let (url, handle) = bytes_once("200 OK", b"{\"name\":\"\xff\xfe\",\"tag_name\":\"v9.9.9\"}");
        assert_eq!(ask_within(&url, TIMEOUT).as_deref(), Ok("v9.9.9"), "깨진 바이트가 성한 태그를 막았다");
        let _ = handle.join();
        // 깨진 바이트가 태그 안에 들면 씻긴 자리가 남아 판으로 안 읽힌다 — **태그 꼴이 아닌
        // 것**이지 TLS 가 안 선 것이 아니다.
        let s = Scratch::new("latest-broken-byte");
        let (url, handle) = bytes_once("200 OK", b"{\"tag_name\":\"v9.9.9\xff\"}");
        let got = refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW);
        assert_eq!(why_of(&got), Some(Trouble::OddTag), "성치 않은 바이트를 TLS 실패로 읽었다 — {got:?}");
        let _ = handle.join();
    }

    /// **TLS 를 어떻게 세웠는지를 잰다**(moai-t906). 시험의 부름은 모두 `http://127.0.0.1` 이라
    /// rustls·webpki-roots 가 한 번도 안 돈다 — 기능 조합이 바뀌어 인증서 검증이 꺼져도 온
    /// 시험이 푸르고, 생산에서는 "못 물었다" 로 접혀 네트워크 없음과 구별이 안 간다.
    ///
    /// **살아 있는 네트워크를 안 탄다.** 바깥에 붙는 시험은 느리고 흔들린다 — 대신
    /// `ureq::Agent` 가 들고 있는 설정을 글자로 읽고, TLS 가 실제로 도는지는 아래 시험이 따로
    /// 잰다.
    ///
    /// 재는 다섯은 저마다 다른 되돌림을 잡는다.
    ///
    /// - `provider` — `native-tls` 로 갈아타면 그 기계에 깔린 뿌리를 믿게 된다
    /// - `root_certs` — `PlatformVerifier` 로 바뀌면 프록시가 끼운 뿌리를 믿는다. 루트 저장소는
    ///   `webpki-roots` 하나로 간다는 것이 moai-uwuw 가 닫은 결정이다
    /// - `disable_verification` — 켜지면 아무 인증서나 지난다
    /// - `use_sni` — 꺼지면 이름을 안 대고 붙어, 한 주소에 여러 이름이 선 자리에서 엉뚱한
    ///   인증서를 받는다
    /// - `https_only` — 꺼지면 되돌림 한 줄이 이 부름을 평문으로 내린다(리뷰). ureq 의 기본이
    ///   꺼짐이라 **안 적으면 꺼진다**
    #[test]
    fn the_tls_it_builds_is_rustls_over_mozillas_roots() {
        let agent = agent_for(API, TIMEOUT);
        let tls = agent.config().tls_config();
        assert_eq!(tls.provider(), ureq::tls::TlsProvider::Rustls, "TLS 제공자가 rustls 가 아니다");
        assert!(
            matches!(tls.root_certs(), ureq::tls::RootCerts::WebPki),
            "뿌리 인증서가 webpki-roots 가 아니다 — {:?}",
            tls.root_certs()
        );
        assert!(!tls.disable_verification(), "인증서 검증이 꺼져 있다");
        assert!(tls.use_sni(), "SNI 가 꺼져 있다");
        assert!(agent.config().https_only(), "되돌림이 평문으로 내려갈 수 있다");
    }

    /// **되돌림은 평문으로 못 내려간다**(리뷰). `https` 로 나간 부름이 `302 Location: http://…`
    /// 한 줄에 평문이 되면, 거기서 받은 태그가 판 줄에 서고 [`FILE`] 에 하루를 산다 — 그 문은
    /// 되돌림 걸음마다 다시 서야 하고(`run::call_run`), ureq 의 기본은 꺼짐이다.
    ///
    /// **처음부터 평문인 자리는 그대로 둔다.** `MOAI_API_URL=http://거울…` 은 그 사람이 고른
    /// 것이고, 여기서 막을 것은 고르지 않은 내려감이다 — 시험 서버가 모두 그 자리에 선다.
    #[test]
    fn a_plain_call_stays_plain_and_a_secure_one_cannot_be_downgraded() {
        assert!(agent_for("https://api.github.com/x", TIMEOUT).config().https_only());
        assert!(agent_for("HTTPS://api.github.com/x", TIMEOUT).config().https_only(), "스킴을 대소문자로 갈랐다");
        assert!(!agent_for("http://127.0.0.1:8080/x", TIMEOUT).config().https_only(), "평문 거울을 막았다");
        assert!(!secure_scheme("설명 없는 글"), "스킴이 없는 글을 https 로 읽었다");
    }

    /// **TLS 가 실제로 돈다**(moai-t906). 위의 시험은 설정을 읽을 뿐이라, 기능이 빠져 그
    /// 레이어가 아예 안 서는 판은 못 잡는다 — 그때는 `https` 부름이 TLS 가 아니라 다른 까닭으로
    /// 죽는다.
    ///
    /// **평문으로 답하는 제 서버에 `https` 로 붙는다.** 클라이언트는 ClientHello 를 보내고
    /// 서버는 HTTP 로 답하므로 핸드셰이크(handshake)가 깨진다 — 그 깨짐이 [`Trouble::Tls`] 로
    /// 오면 TLS 레이어가 선 것이다. 바깥에 안 붙으니 느리지도 흔들리지도 않는다.
    /// **인증서 검증 자체를 재지는 못한다**: 핸드셰이크가 그 앞에서 깨지므로 루트 저장소는
    /// 안 열린다 — 그 자리는 위의 시험이 설정으로 잰다.
    #[test]
    fn a_plain_answer_to_an_https_call_is_a_tls_failure() {
        let (url, handle) = server_once("{}");
        let https = url.replacen("http://", "https://", 1);
        let got = ask_within(&https, TIMEOUT).map_err(|w| w.kind);
        assert_eq!(got, Err(Trouble::Tls), "TLS 레이어가 안 섰거나 그 실패를 다른 갈래로 읽었다");
        let _ = handle.join();
    }

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
        saying_once("200 OK", body)
    }

    /// [`server_once`] 되 **상태 줄을 고른다**(moai-580l). 시간당 부름 수를 태운 403 과 저장소를
    /// 옮긴 404 는 사람이 할 일이 다르고, 그 갈림을 재려면 서버가 그 상태 코드를 내야 한다.
    fn saying_once(status: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<usize>) {
        bytes_once(status, body.as_bytes())
    }

    /// [`saying_once`] 되 **몸을 바이트로 받는다**(리뷰). 성치 않은 UTF-8 을 보내려면 `&str` 로는
    /// 못 짓는데, 그것이 `application/json` 답에서 실제로 오는 꼴이다.
    fn bytes_once(status: &'static str, body: &'static [u8]) -> (String, std::thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("못 띄웠다");
        let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut served = 0;
            if let Ok((stream, _)) = listener.accept() {
                answer(stream, status, body);
                served += 1;
            }
            served
        });
        (url, handle)
    }

    fn answer(mut stream: TcpStream, status: &str, body: &[u8]) {
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
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(body);
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
        let held = Held {
            asked_at: "2026-09-21T00:00:00Z".into(),
            tag: Some("v0.1.0".into()),
            url: Some(API.into()),
            trouble: None,
        };
        write(s.path(), &held).unwrap();
        let src = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(src.starts_with(HEADER), "머리 줄이 머리에 없다\n{src}");
        // 두 번째 쓰기도 머리를 흔들지 않고, 읽는 쪽은 그대로 읽는다.
        write(s.path(), &Held { asked_at: "2026-09-22T00:00:00Z".into(), tag: None, url: None, trouble: None })
            .unwrap();
        let again = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(again.starts_with(HEADER), "두 번째 쓰기가 머리를 흔들었다\n{again}");
        let back = read(s.path()).unwrap();
        assert_eq!(back.tag, None);
        // 빈 값은 키를 지운다 — 옛 자리가 남아 다음 부름을 헷갈리게 하지 않는다.
        assert_eq!(back.url, None, "지운 자리가 파일에 남았다");
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
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let old = Held {
            asked_at: "2026-09-20T00:00:00Z".into(),
            tag: Some("v0.0.1".into()),
            url: Some(url.clone()),
            trouble: None,
        };
        write(s.path(), &old).unwrap();
        let now = "2026-09-21T00:00:01Z";
        assert_eq!(refresh(s.path(), &url, now, WINDOW), Seen::Newer { tag: "v9.9.9".into() });
        assert_eq!(handle.join().unwrap(), 1, "창이 지났는데 안 물었다");
        let held = read(s.path()).unwrap();
        assert_eq!(held.asked_at, now);
        assert_eq!(held.tag.as_deref(), Some("v9.9.9"));
    }

    /// **다른 자리에 물은 답은 이 자리의 답이 아니다**(moai-dael). `MOAI_API_URL` 을 한 번 바꿔
    /// 부르면 그 답이 하루 동안 진짜 부름에 섰고, 거울을 쓰는 사람에게는 그 반대가 됐다.
    ///
    /// 세 자리를 함께 잰다 — 창이 안 닫히는 것, 첫 프레임([`held`])이 그 답을 안 그리는 것,
    /// 그리고 못 들었을 때 그 답을 **지난 답으로 들고 가지 않는** 것.
    #[test]
    fn an_answer_from_another_place_is_not_this_places_answer() {
        let s = Scratch::new("latest-place");
        let mirror = Held {
            asked_at: "2026-09-21T00:00:00Z".into(),
            tag: Some("v9.9.9".into()),
            url: Some("http://mirror.example/releases/latest".into()),
            trouble: None,
        };
        write(s.path(), &mirror).unwrap();
        let here = nobody_there();
        // 첫 프레임은 남의 답을 안 그린다.
        assert_eq!(why_of(&held(s.path(), &here)), Some(Trouble::NotAsked));
        // 창 안의 도장인데도 묻는다 — 그리고 못 들었으니 남의 태그를 들고 가지 않는다.
        let got = refresh(s.path(), &here, "2026-09-21T00:00:01Z", WINDOW);
        assert_eq!(why_of(&got), Some(Trouble::Offline));
        let now = read(s.path()).expect("도장이 없다");
        assert_eq!(now.url.as_deref(), Some(here.as_str()), "물은 자리를 안 적었다");
        assert_eq!(now.tag, None, "남의 자리에서 들은 태그를 들고 갔다");
        // 같은 자리의 답이면 창이 그대로 닫힌다.
        assert!(now.asked_here(&here));
    }

    /// **토큰은 자리가 아니다**(리뷰). `MOAI_API_URL` 에 끼운 사용자 정보가 [`FILE`] 에 평문으로
    /// 남던 자리다 — 그 파일의 첫 쓰기는 umask 를 따라 남이 읽는다. 떼고 적으니 토큰만 다른 두
    /// 부름은 같은 자리로 서고, 창도 그대로 닫힌다.
    #[test]
    fn the_token_in_the_url_is_not_written_down() {
        let s = Scratch::new("latest-userinfo");
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let with_token = url.replacen("http://", "http://ci-bot:ghp_secret@", 1);
        assert_eq!(
            refresh(s.path(), &with_token, "2026-09-21T00:00:00Z", WINDOW),
            Seen::Newer { tag: "v9.9.9".into() }
        );
        let src = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(!src.contains("ghp_secret"), "토큰이 파일에 남았다\n{src}");
        // 토막만 뗐지 자리는 그대로다 — 토큰을 낀 부름도, 안 낀 부름도 같은 자리로 선다.
        let now = read(s.path()).expect("도장이 없다");
        assert_eq!(now.url.as_deref(), Some(url.as_str()));
        assert!(now.asked_here(&with_token) && now.asked_here(&url), "자리가 안 맞는다");
        let _ = handle.join();
        // 꼴이 아닌 글은 그대로 둔다 — 자리를 가리는 것이 이 자의 일 전부다.
        assert_eq!(place_of("설명 없는 글"), "설명 없는 글");
        assert_eq!(place_of("https://api.github.com/x"), "https://api.github.com/x");
    }

    /// **못 물은 까닭은 창 내내 선다**(2026-09-22 사용자 결정). 안 적던 판은 까닭이 그 부름 한
    /// 판에만 서고 두 번째 부름부터 "안 물었다" 한 글로 접혔다 — 인증서를 갈아 끼우는 프록시
    /// 뒤의 사람은 "TLS 실패" 를 하루에 한 번만 보고 나머지는 갈래 없는 글만 봤다.
    ///
    /// **들은 판은 그 까닭을 지운다** — 파일이 드는 것은 답 하나고, 태그가 섰으면 그것이 답이다.
    #[test]
    fn the_reason_stands_for_the_whole_window() {
        let s = Scratch::new("latest-why-held");
        let dead = nobody_there();
        assert_eq!(why_of(&refresh(s.path(), &dead, "2026-09-21T00:00:00Z", WINDOW)), Some(Trouble::Offline));
        // 창 안이라 다시 안 묻는다 — 그래도 같은 글이 선다.
        assert_eq!(why_of(&refresh(s.path(), &dead, "2026-09-21T12:00:00Z", WINDOW)), Some(Trouble::Offline));
        // 첫 프레임이 읽는 자리도 같다.
        assert_eq!(why_of(&held(s.path(), &dead)), Some(Trouble::Offline));
        // 파일에 낱말로 적힌다 — 판이 다른 바이너리가 되읽을 자리다.
        let src = std::fs::read_to_string(file_at(s.path())).unwrap();
        assert!(src.contains("trouble = \"offline\""), "까닭을 안 적었다\n{src}");

        // **들은 판은 지운다.** 같은 자리에 태그가 서면 까닭은 답이 아니다.
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let stale = Held {
            asked_at: "2026-09-20T00:00:00Z".into(),
            tag: None,
            url: Some(url.clone()),
            trouble: Some(Trouble::Tls),
        };
        write(s.path(), &stale).unwrap();
        assert_eq!(refresh(s.path(), &url, "2026-09-21T12:00:00Z", WINDOW), Seen::Newer { tag: "v9.9.9".into() });
        assert_eq!(read(s.path()).unwrap().trouble, None, "태그를 들었는데 까닭이 남았다");
        let _ = handle.join();
    }

    /// **되읽은 까닭에는 `said` 가 없고, 이 판의 까닭에는 있다**(2026-09-22 사용자 결정).
    /// `said` 는 `ureq` 가 이 부름에 대고 지은 글이라 파일에 남길 값이 아니다 — 그래서 물은
    /// 판만 그 글을 들고, 창 안에서 되읽은 판은 갈래만 든다.
    #[test]
    fn only_the_run_that_asked_carries_the_librarys_words() {
        let s = Scratch::new("latest-why-said");
        let dead = nobody_there();
        let Seen::Unasked(asked) = refresh(s.path(), &dead, "2026-09-21T00:00:00Z", WINDOW) else {
            panic!("못 물었다가 아니다");
        };
        assert!(!asked.said.is_empty(), "물은 판이 까닭의 글을 안 들었다");
        let Seen::Unasked(reread) = held(s.path(), &dead) else { panic!("못 물었다가 아니다") };
        assert_eq!(reread, Why { kind: Trouble::Offline, said: String::new() });
    }

    /// **자리를 모르는 옛 줄은 다시 묻는다**(moai-dael). 이 필드 전에 적힌 파일이고, 어디에
    /// 물은 답인지 알 길이 없다 — 치르는 값은 판올림 뒤 한 번 더 묻는 것뿐이다.
    #[test]
    fn a_stamp_from_before_this_field_is_asked_again() {
        let s = Scratch::new("latest-oldstamp");
        std::fs::write(file_at(s.path()), "asked_at = \"2026-09-21T00:00:00Z\"\ntag = \"v9.9.9\"\n").unwrap();
        let old = read(s.path()).expect("못 읽었다");
        assert_eq!(old.tag.as_deref(), Some("v9.9.9"), "옛 줄을 버렸다");
        assert!(!old.asked_here(API), "자리를 모르는 줄을 이 자리의 답으로 세웠다");
        let (url, handle) = server_once(r#"{"tag_name":"v0.0.1"}"#);
        assert_eq!(refresh(s.path(), &url, "2026-09-21T00:00:01Z", WINDOW), Seen::Ahead);
        assert_eq!(handle.join().unwrap(), 1, "창 안이라며 안 물었다");
    }

    #[test]
    fn a_failed_ask_does_not_forget_what_it_already_heard() {
        // 어제 "새 판 v9.9.9 가 있다" 를 본 사람이, 오늘 그물이 한 번 끊긴 것만으로 "못 물었다"
        // 를 보게 하지 않는다. 도장은 새로 찍혀 하루 동안 다시 안 묻고, 태그는 지난 것이 선다.
        let s = Scratch::new("latest-keeps");
        let url = nobody_there();
        let old = Held {
            asked_at: "2026-09-20T00:00:00Z".into(),
            tag: Some("v9.9.9".into()),
            url: Some(url.clone()),
            trouble: None,
        };
        write(s.path(), &old).unwrap();
        let now = "2026-09-21T00:00:01Z";
        assert_eq!(refresh(s.path(), &url, now, WINDOW), Seen::Newer { tag: "v9.9.9".into() });
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
        let far =
            Held { asked_at: "2099-01-01T00:00:00Z".into(), tag: Some("v0.1.0".into()), url: None, trouble: None };
        assert!(!far.fresh("2026-09-21T00:00:00Z", WINDOW), "시계를 되돌린 기계가 영영 안 묻는다");
        // 1초 어긋난 시계 — 이것까지 낡았다고 하면 그 기계는 부를 때마다 묻는다.
        let ticking =
            Held { asked_at: "2026-09-21T00:00:01Z".into(), tag: Some("v0.1.0".into()), url: None, trouble: None };
        assert!(ticking.fresh("2026-09-21T00:00:00Z", WINDOW), "1초 앞선 도장에 다시 물었다");
        // 경계를 재는 자리. `slack` 만큼 앞선 것은 봐주고, 1초 더 앞선 것은 안 봐준다.
        let at = |ahead: i64| Held { asked_at: stamp(ahead), tag: None, url: None, trouble: None };
        assert!(at(slack).fresh("2026-09-21T00:00:00Z", WINDOW), "딱 슬랙만큼 앞선 도장을 낡았다고 했다");
        assert!(!at(slack + 1).fresh("2026-09-21T00:00:00Z", WINDOW), "슬랙을 넘은 도장이 창을 열어 뒀다");
        let broken = Held { asked_at: "어제".into(), tag: None, url: None, trouble: None };
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
        // "네트워크가 없다" 다**(moai-580l) — 태그가 이상한 것도, 부름 수를 태운 것도 아니다.
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

    /// **갈래의 낱말은 저마다 다르고 안 바뀐다**(moai-580l). 이 낱말은 [`FILE`] 에 적혀
    /// 다음 부름이 되읽으므로(2026-09-22 사용자 결정), 고치면 그 판의 파일이 옛 판에 안 읽힌다.
    ///
    /// **짓는 자와 되읽는 자가 맞물리는지 함께 잰다** — 둘이 갈리면 적어 둔 까닭이 말없이
    /// 사라지고, 화면은 "안 물었다" 로 떨어진다.
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
            assert_eq!(kind.name(), word, "{kind:?}");
            assert_eq!(Trouble::from_name(word), Some(kind), "되읽지 못했다 — {word}");
        }
        // 모르는 낱말은 까닭을 모르는 것이지 줄이 깨진 것이 아니다.
        assert_eq!(Trouble::from_name("새 갈래"), None);
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
        assert_eq!(why_of(&held(s.path(), API)), Some(Trouble::NotAsked));
    }

    #[test]
    fn what_it_wrote_it_reads_back() {
        let s = Scratch::new("latest-roundtrip");
        for (tag, trouble) in [(Some("v0.1.0"), None), (None, Some("tls")), (None, None)] {
            for url in [Some(API), None] {
                let held = Held {
                    asked_at: "2026-09-21T00:00:00Z".into(),
                    tag: tag.map(String::from),
                    url: url.map(String::from),
                    trouble: trouble.map(|w| Trouble::from_name(w).expect("모르는 낱말")),
                };
                write(s.path(), &held).unwrap();
                assert_eq!(read(s.path()).as_ref(), Some(&held));
            }
        }
    }

    #[test]
    fn a_key_it_does_not_know_survives_a_write() {
        // 판이 다른 바이너리들이 한 파일을 쓴다 — 새 판이 적은 키를 옛 판이 한 번 만져
        // 지우면, 이 모듈이 선 자리(판이 섞이는 기계)가 곧 그 손실이 나는 자리다.
        let s = Scratch::new("latest-unknown");
        std::fs::write(file_at(s.path()), "# 사람이 적은 줄\nasked_at = \"2026-09-20T00:00:00Z\"\netag = \"W/abc\"\n")
            .unwrap();
        let now = Held {
            asked_at: "2026-09-21T00:00:00Z".into(),
            tag: Some("v0.2.0".into()),
            url: Some(API.into()),
            trouble: None,
        };
        write(s.path(), &now).unwrap();
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
