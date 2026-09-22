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
//!   대고 물었다 — 빈 크레이트 308,752바이트 기준 `ureq` 3 은 +1,880,088바이트, `minreq` 는
//!   +1,137,240바이트, `curl` 프로세스는 0바이트였다. 그런데도 크레이트를 고른 것은 **없는
//!   기계가 없기 때문**이다. `curl` 도 `wget` 도 없는 기계에서는 이 기능이 영영 "못 물었다" 로
//!   서는데, 그 기계가 어느 기계인지 도구는 모른다
//! - **하루 한 번 묻는다**([`WINDOW`]). 답과 물은 때를 [`FILE`] 에 적고 그 안이면 안 묻는다.
//!   자리는 읽음 파일([`crate::read_marks`])의 관례를 따라 설정 파일 곁이고, 판은 프로젝트마다
//!   다르지 않으니 사람마다 한 파일이다
//! - **기본은 켬이되 사람이 보는 화면에서만이다**([`gate`]). `--json`·비대화형·파이프는 안
//!   묻는다 — 에이전트가 도는 기계가 매번 바깥을 두드리면 안 된다
//! - **태그를 semver 로 견준다**([`compare`]). 넷이 다른 글이다
//!
//! **그리는 걸음에 실리지 않는다.** [`spawn`] 이 딴 실에서 묻고, 그리는 쪽은 `App` 이 받아 둔
//! 답을 읽기만 한다 — `tui::draw::version_said` 는 프레임마다 도는 자리라 거기서 `git config`
//! 조차 안 부른다(그 주석이 적어 둔 까닭이 이것과 같다).
//!
//! **실패는 모두 같은 자리로 내려앉는다.** 네트워크가 없든, 느려서 [`TIMEOUT`] 을 넘든, 답이
//! 깨진 JSON 이든, 태그가 `v1.2.3` 꼴이 아니든 [`Seen::Unasked`] 다 — 사람에게 보일 글은 "못
//! 물었다" 하나고, 그 까닭을 넷으로 갈라 봤자 고칠 수 있는 것이 없다.
// **이 파일의 값을 아직 아무도 안 그린다** — 판 줄에 얹는 일이 moai-3gia 고, 그 일이 쥔 파일
// (`src/tui/draw.rs`)을 지금 옆 세션이 들고 있다. 그때 이 줄을 걷는다.
//
// 그래서 **지금 바이너리에는 `ureq` 가 안 들어 있다** — LTO 가 닿지 않는 이 모듈을 통째로
// 걷는다(6,186,192바이트, 들이기 전과 같다). 판 줄이 이것을 부르는 날 드는 값을 재 두었다:
// `main` 에서 한 번 부르게 하고 릴리스로 빌드하면 7,934,720바이트로, +1,748,528바이트다.
// 15MB 예산의 53%다.
#![allow(dead_code)]

use crate::fail::R;
use crate::store::write_atomic;
use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 답이 사는 파일 — `<설정 디렉터리>/latest.toml`. **도구가 짓고 도구가 고쳐 쓴다.**
pub const FILE: &str = "latest.toml";

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
    /// 못 물었다. 안 물은 것(끈 것)과 물었는데 못 들은 것이 **같은 글**인 것은 뜻이 같아서다 —
    /// 둘 다 "지금 최신판을 모른다" 고, 사람이 할 일도 같다.
    Unasked,
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
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        Some(_) => return None,
        None => (v, None),
    };
    // 빌드 메타데이터(`+…`)는 판을 가르지 않는다 — semver 가 그렇게 정했다.
    let core = core.split('+').next().unwrap_or(core);
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
    Some((n, pre))
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
            (Some(x), Some(y)) => x.cmp(y),
        },
        other => other,
    })
}

/// 받은 태그를 내 판과 견줘 [`Seen`] 으로. 태그가 없거나 꼴이 아니면 못 물은 것이다.
pub fn seen(mine: &str, tag: Option<&str>) -> Seen {
    let Some(tag) = tag else { return Seen::Unasked };
    match compare(mine, version_of(tag)) {
        Some(Ordering::Less) => Seen::Newer { tag: tag.to_string() },
        Some(Ordering::Equal) => Seen::Same,
        Some(Ordering::Greater) => Seen::Ahead,
        None => Seen::Unasked,
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
    pub fn fresh(&self, now: &str, window: i64) -> bool {
        let (Some(then), Some(now)) = (crate::model::parse_rfc3339(&self.asked_at), crate::model::parse_rfc3339(now))
        else {
            return false;
        };
        // **앞선 도장도 낡은 것이다.** 시계를 되돌린 기계나 손으로 적은 미래의 때가 창을 영영
        // 열어 두지 못하게 한다.
        (0..window).contains(&(now - then))
    }
}

/// 답이 사는 자리 — 설정 파일의 **곁**이다.
pub fn file_at(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// 적어 둔 답을 읽는다. **관대하게 읽는다** — 파일이 없거나 깨졌으면 없는 것이고, 없으면 묻는다.
pub fn read(dir: &Path) -> Option<Held> {
    let src = std::fs::read_to_string(file_at(dir)).ok()?;
    let doc: toml_edit::DocumentMut = src.parse().ok()?;
    let asked_at = doc.get("asked_at")?.as_str()?.to_string();
    let tag = doc.get("tag").and_then(|i| i.as_str()).map(String::from);
    Some(Held { asked_at, tag })
}

/// 답을 적는다. **스냅샷 먼저, 저널 나중** 과 같은 자리에 선다 — 이 파일은 잃어도 한 번 더 묻는
/// 것이 전부라, 락을 잡지 않고 temp+rename 만 한다. 둘이 같이 쓰면 늦은 쪽이 남고, 둘 다 같은
/// 것을 적으므로 진 쪽도 잃는 것이 없다.
pub fn write(dir: &Path, held: &Held) -> R<()> {
    std::fs::create_dir_all(dir).map_err(|e| crate::fail::Fail::new(format!("{}: {e}", dir.display())))?;
    let mut out = String::from("# moai 가 짓는 파일이다. 지워도 되고, 다음에 다시 묻는다.\n");
    out.push_str(&format!("asked_at = {}\n", toml_edit::Value::from(held.asked_at.as_str())));
    if let Some(tag) = &held.tag {
        out.push_str(&format!("tag = {}\n", toml_edit::Value::from(tag.as_str())));
    }
    write_atomic(&file_at(dir), out.as_bytes())
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
pub fn gate(env: impl Fn(&str) -> Option<OsString>, config_says: Option<bool>, on_screen: bool) -> Option<Off> {
    if env(OFF_VAR).is_some_and(|v| !v.is_empty()) {
        return Some(Off::Env);
    }
    if config_says == Some(false) {
        return Some(Off::Config);
    }
    if !on_screen {
        return Some(Off::NotAScreen);
    }
    None
}

/// 이 부름이 사람이 보는 화면인가 — 표준 출력이 터미널인가로 묻는다.
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
pub fn config_says(path: Option<&Path>) -> Option<bool> {
    let doc: toml_edit::DocumentMut = std::fs::read_to_string(path?).ok()?.parse().ok()?;
    doc.get(UPDATE)?.as_table_like()?.get(CHECK)?.as_bool()
}

/// 한 번 묻는다. 태그를 못 얻으면 `None` 이고, **까닭은 안 든다**(모두 "못 물었다" 로 내려앉는다).
///
/// `User-Agent` 를 다는 것은 GitHub 가 없는 부름에 403 을 주기 때문이다.
pub fn ask(url: &str) -> Option<String> {
    ask_within(url, TIMEOUT)
}

/// [`ask`] 되 기다리는 상한을 받는다. **시험이 그 상한을 짧게 줘서 실제로 끊기는지 잰다** —
/// 상한을 상수로만 두면 그것이 서는지를 5초씩 기다려야만 볼 수 있고, 그러면 아무도 안 잰다.
pub fn ask_within(url: &str, timeout: Duration) -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        // **머리부터 몸까지 통째로 잰다.** 붙기만 재면 붙여 놓고 한 글자씩 흘리는 자리에
        // 영영 붙들린다.
        .timeout_global(Some(timeout))
        // 되돌림을 따라가지 않는다 — 릴리스 API 는 곧바로 답하고, 따라가는 만큼 시간이 는다.
        .max_redirects(2)
        .build()
        .into();
    let body = agent
        .get(url)
        .header("User-Agent", concat!("moai/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        // **받는 만큼에 상한을 둔다.** 기본은 10MB 고, 이 답은 몇 KB 다 — `MOAI_API_URL` 이
        // 가리키는 자리가 끝없이 뱉을 때 그 10MB 를 다 받아 줄 까닭이 없다.
        .with_config()
        .limit(256 * 1024)
        // 글자가 깨져도 읽는다 — 어차피 `tag_name` 하나만 집고, 못 집으면 "못 물었다" 다.
        .lossy_utf8(true)
        .read_to_string()
        .ok()?;
    tag_in(&body)
}

/// 답에서 태그를 집는다. 깨진 JSON 도, `tag_name` 이 없는 답도 `None` 이다.
fn tag_in(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let tag = v.get("tag_name")?.as_str()?.trim();
    (!tag.is_empty()).then(|| tag.to_string())
}

/// 물을 자리 — `MOAI_API_URL` 이 있으면 그것, 없으면 [`API`].
pub fn url_from(env: impl Fn(&str) -> Option<OsString>) -> String {
    env("MOAI_API_URL")
        .filter(|v| !v.is_empty())
        .and_then(|v| v.into_string().ok())
        .unwrap_or_else(|| API.to_string())
}

/// 창이 열렸으면 묻고 적는다. 창 안이면 적어 둔 것을 그대로 쓴다.
///
/// **못 들어도 적는다** — 물은 때를 적어 두어야 그물 없는 기계가 부를 때마다 [`TIMEOUT`] 을
/// 버리지 않는다. 적기에 실패하는 것은 답을 바꾸지 않는다(다음에 한 번 더 묻는 것이 전부다).
pub fn refresh(dir: &Path, url: &str, now: &str, window: i64) -> Seen {
    if let Some(held) = read(dir)
        && held.fresh(now, window)
    {
        return seen(mine(), held.tag.as_deref());
    }
    let tag = ask(url);
    let _ = write(dir, &Held { asked_at: now.to_string(), tag: tag.clone() });
    seen(mine(), tag.as_deref())
}

/// 적어 둔 답만 읽는다 — **묻지 않는다.** 창이 지났어도 지난 답을 그대로 낸다: 그리는 쪽이
/// 첫 프레임에 쓸 값이고, 새 답은 [`spawn`] 이 받아 뒤에 얹는다.
pub fn held(dir: &Path) -> Seen {
    read(dir).map_or(Seen::Unasked, |h| seen(mine(), h.tag.as_deref()))
}

/// 딴 실에서 [`refresh`] 를 돌리고 답을 흘린다. **부르는 쪽은 기다리지 않는다** — 받는 쪽이
/// `try_recv` 로 집고, 안 왔으면 [`held`] 가 준 값을 그대로 그린다.
///
/// 실이 먼저 끝나 받는 쪽이 사라져도 보내기가 실패할 뿐이라 아무 일도 안 난다.
pub fn spawn(dir: PathBuf, url: String, now: String, window: i64) -> std::sync::mpsc::Receiver<Seen> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(refresh(&dir, &url, &now, window));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
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
    fn nobody_there() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{addr}/releases/latest")
    }

    #[test]
    fn the_four_answers_are_four_different_things() {
        assert_eq!(seen("0.1.0", Some("v0.2.0")), Seen::Newer { tag: "v0.2.0".into() });
        assert_eq!(seen("0.1.0", Some("v0.1.0")), Seen::Same);
        assert_eq!(seen("0.2.0", Some("v0.1.0")), Seen::Ahead);
        assert_eq!(seen("0.1.0", None), Seen::Unasked);
        // 꼴이 아닌 태그는 "새 판" 이 아니라 "못 물었다" 다 — 모르는 것을 아는 척하지 않는다.
        assert_eq!(seen("0.1.0", Some("latest")), Seen::Unasked);
        assert_eq!(seen("0.1.0", Some("v0.1")), Seen::Unasked);
        assert_eq!(seen("0.1.0", Some("v0.1.0.1")), Seen::Unasked);
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
    fn a_stamp_from_the_future_does_not_hold_the_window_open() {
        let held = Held { asked_at: "2026-09-22T00:00:00Z".into(), tag: Some("v0.1.0".into()) };
        assert!(!held.fresh("2026-09-21T00:00:00Z", WINDOW), "시계를 되돌린 기계가 영영 안 묻는다");
        let broken = Held { asked_at: "어제".into(), tag: None };
        assert!(!broken.fresh("2026-09-21T00:00:00Z", WINDOW), "못 읽는 도장이 묻기를 막는다");
    }

    #[test]
    fn a_grid_that_is_not_there_is_not_asked_and_not_a_failure() {
        let s = Scratch::new("latest-nogrid");
        let url = nobody_there();
        // 거절당해도 답은 "못 물었다" 고, 종료 코드를 바꿀 `Err` 가 아니다.
        assert_eq!(refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW), Seen::Unasked);
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
        let held = std::thread::spawn(move || {
            let held = listener.accept().map(|(s, _)| s);
            // 끊지 않고 들고 있는다. 시험이 끝나면 함께 떨어진다.
            std::thread::sleep(Duration::from_secs(2));
            drop(held);
        });
        let began = std::time::Instant::now();
        assert_eq!(ask_within(&url, Duration::from_millis(300)), None);
        assert!(began.elapsed() < Duration::from_secs(2), "상한을 넘겨 기다렸다 — {:?}", began.elapsed());
        let _ = held.join();
    }

    #[test]
    fn an_answer_that_lies_falls_back_to_unasked() {
        for body in [r#"{"tag_name":"#, r#"{"tag_name":42}"#, r#"{"name":"v9.9.9"}"#, r#"{"tag_name":"세판"}"#, ""] {
            let s = Scratch::new("latest-lies");
            let (url, handle) = server_once(Box::leak(body.to_string().into_boxed_str()));
            assert_eq!(refresh(s.path(), &url, "2026-09-21T00:00:00Z", WINDOW), Seen::Unasked, "{body}");
            let _ = handle.join();
        }
    }

    #[test]
    fn a_cache_file_a_person_broke_is_read_as_absent() {
        let s = Scratch::new("latest-broken");
        std::fs::write(file_at(s.path()), "asked_at = [\n").unwrap();
        assert!(read(s.path()).is_none());
        std::fs::write(file_at(s.path()), "asked_at = 3\n").unwrap();
        assert!(read(s.path()).is_none(), "때가 낱말이 아닌데 읽었다");
        assert_eq!(held(s.path()), Seen::Unasked);
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
    fn the_two_switches_and_the_screen_each_stop_it() {
        let none = |_: &str| None;
        assert_eq!(gate(none, None, true), None);
        assert_eq!(gate(none, Some(true), true), None);
        assert_eq!(gate(|k| (k == OFF_VAR).then(|| OsString::from("1")), None, true), Some(Off::Env));
        // 빈 값은 없는 것이다 — 환경변수의 관례.
        assert_eq!(gate(|k| (k == OFF_VAR).then(OsString::new), None, true), None);
        assert_eq!(gate(none, Some(false), true), Some(Off::Config));
        assert_eq!(gate(none, None, false), Some(Off::NotAScreen));
        // 껐는데 화면도 아니면 먼저 만난 까닭을 든다 — 어느 쪽이든 안 묻는 것은 같다.
        assert_eq!(gate(none, Some(false), false), Some(Off::Config));
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
    fn the_place_it_asks_can_be_turned() {
        assert_eq!(url_from(|_| None), API);
        assert_eq!(url_from(|k| (k == "MOAI_API_URL").then(|| OsString::from("http://x/y"))), "http://x/y");
        assert_eq!(url_from(|k| (k == "MOAI_API_URL").then(OsString::new)), API, "빈 값은 없는 것이다");
    }

    #[test]
    fn the_thread_hands_the_answer_over() {
        let s = Scratch::new("latest-thread");
        let (url, handle) = server_once(r#"{"tag_name":"v9.9.9"}"#);
        let rx = spawn(s.path().to_path_buf(), url, "2026-09-21T00:00:00Z".into(), WINDOW);
        assert_eq!(rx.recv_timeout(Duration::from_secs(20)).expect("답이 안 왔다"), Seen::Newer { tag: "v9.9.9".into() });
        let _ = handle.join();
    }

    #[test]
    fn the_version_it_compares_against_is_the_one_it_was_built_with() {
        // 판 줄에 서는 값과 견주는 값이 갈리면, 화면은 "새 판" 인데 받아 보면 같은 판이다.
        assert_eq!(mine(), env!("CARGO_PKG_VERSION"));
        assert_eq!(seen(mine(), Some(&format!("v{}", mine()))), Seen::Same);
    }
}
