//! 우편함과 출석부(moai-h8tn) — **편지 하나가 파일 하나다.**
//!
//! 에이전트끼리 주고받는 글은 `.moai/mail/` 에, 지금 누가 와 있는지는 `.moai/agents/` 에 산다. 자리는
//! [`crate::store::Repo::mail_dir`]·[`crate::store::Repo::agents_dir`] 가 짓고, 트래커와 같이 딸린
//! 워크트리에서 루트로 옮겨 가 모든 세션이 한 우편함을 본다.
//!
//! **트래커가 아니다** — `issues.jsonl` 과 저널에는 한 글자도 안 든다(CLAUDE.md "저널에 적는 것은
//! create·status·note·rm 넷뿐"). 우편함은 전달이지 기록이 아니다. 결정·`Next:`·`model:` 은 지금처럼
//! 노트로 남는다. 둘 다 `.gitignore` 에 든다 — `init` 이 줄을 더하고, 디렉터리도 제 `.gitignore`(`*`)를
//! 든다(아래 [`ensure_dir`]).
//!
//! ## 받는 이마다 함이 하나다
//!
//! 편지는 `mail/<받는 이>/<id>.json` 에, `any-idle-worker` 편지는 `mail/any-idle-worker/<id>.json` 에 든다
//! (2026-10-04 사용자 결정, moai-ew4o.c92). **에이전트 하나가 여는 자리는 그 둘뿐이다** — 훅은 프롬프트·턴 끝마다
//! 편지를 찾는데, 한 디렉터리에 모두 두던 판은 남에게 간 편지까지 열어 받는 이를 가렸다(남의 편지 300통에 훅 한 번이
//! +300ms). 받는 이를 못 읽는 깨진 편지 하나가 모든 에이전트의 `inbox` 에 서던 것도 같은 뿌리다 — 이제 깨진 편지는
//! 그 함의 임자에게만 선다. **함이 곧 받는 이다** — 이름을 바꾼 에이전트에게 따라간 편지([`carry`])와 되돌아온
//! 편지([`retire`])는 적힌 `to` 와 함이 다르다. 그 편지를 가르는 것은 [`Stored::returned`] 다.
//!
//! ## 락이 없는 까닭
//!
//! 같은 파일을 두 프로세스가 고치지 않는다. 보내기는 새 이름의 파일을 들이고, 읽기는 그 파일을 `read/`
//! 로 옮긴다. 둘 다 디렉터리 항목 하나를 바꾸는 시스템 호출 하나라 차례는 커널이 정한다.
//!
//! - **보내기**는 temp 에 다 쓴 뒤 `hard_link` 로 제 이름에 들인다([`send`]). `rename` 은 같은 이름이
//!   있으면 **말없이 덮는다** — 이름이 겹친 두 편지 중 하나가 사라지는 조용한 손실이다. `link` 는 그
//!   자리에서 `EEXIST` 로 지고, 진 쪽은 다음 이름으로 다시 든다. 반쯤 쓴 편지는 읽는 쪽에 안 보인다
//! - **읽기**는 `rename(<함>/<id>.json, <함>/read/<id>@<읽은 이>.json)` 이다([`take`]). 둘이 같은 편지를 겨루면
//!   한쪽만 이기고 다른 쪽은 `NotFound` 를 받는다 — `any-idle-worker` 편지를 먼저 가진 쪽이 그것이다
//! - **함을 옮기기**(되돌리기·따라가기)도 덮지 않는 `rename` 하나다([`relocate`]) — 옮기는 동안 읽는 쪽이 가지면
//!   옮기기가 지고, 옮긴 자리의 같은 id 는 다음 id 로 피한다
//!
//! `store::with_write` 는 안 지난다 — 그 락은 스냅샷의 것이다.
//!
//! ## 되돌릴 수 없는 것은 편지 파일의 꼴이다
//!
//! 처음부터 `v: 1` 을 적고 모르는 키는 그대로 든다([`Letter::rest`]). 편지는 한 번 쓰고 안 고치니 파일의
//! 모르는 키는 저절로 남고, `--json` 도 그것을 그대로 낸다. **편지의 id 는 파일 이름이고, 받는 이는 함의
//! 이름이다** — 둘 다 파일 안의 값으로 정하지 않는다. 파생값을 저장하면 둘이 갈리는 날이 온다(CLAUDE.md
//! "파생값은 저장하지 않는다"). id 는 **한 함 안에서** 겹치지 않는다 — 주소는 함과 id 둘이다.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// 편지와 출석 파일의 판. 꼴이 바뀌면 올린다 — 읽는 쪽은 모르는 판도 읽는다(읽기는 관대하다).
pub const VERSION: u64 = 1;

/// 받는 이 자리의 낱말 — 놀고 있는 일꾼 누구나. 먼저 `rename` 한 쪽이 가진다([`take`]).
pub const ANY_IDLE_WORKER: &str = "any-idle-worker";

/// 감독의 역할 낱말 — `any-idle-worker` 편지를 안 가진다(2026-10-04 사용자 결정, [`for_me`]).
pub const SUPERVISOR: &str = "supervisor";

/// 출석의 두 상태. 훅이 `UserPromptSubmit` 에서 `busy`, `Stop` 에서 `idle` 을 적는다.
pub const BUSY: &str = "busy";
pub const IDLE: &str = "idle";

/// 깨울 때 그 에이전트의 입에 넣는 말 — 받은 쪽이 그대로 치면 편지가 나온다.
pub const WAKE_WORDS: &str = "moai inbox";

/// 본문의 상한 — 넘으면 보내기가 거절한다(쓰기는 엄하다). 훅이 이 글을 그대로 맥락에 싣는다: 상한이
/// 없으면 편지 하나가 받는 세션의 맥락을 통째로 먹는다. 리뷰 원문을 노트에 적을 때의 64KB 와 같은 자다.
pub const BODY_MAX: usize = 64 * 1024;

/// 읽을 파일의 상한 — 넘으면 안 읽고 못 읽은 것으로 댄다. 우리가 쓰는 편지는 [`BODY_MAX`] 남짓이라 이
/// 자리에 닿는 것은 손으로 놓은 파일이다.
const FILE_MAX: u64 = 1024 * 1024;

/// 이름의 상한 — 이름이 곧 파일 이름이다.
const NAME_MAX: usize = 64;

// ── 이름 ──────────────────────────────────────────────────────────────

/// 에이전트 이름이 될 수 있는 글인가. **이름이 파일 이름이 되므로** 글자를 좁힌다 — 영숫자와 `.`·`_`·`-`,
/// 64자까지, `.` 으로 시작하지 않는다(숨은 파일은 temp 의 자리다). `@` 가 안 들어 읽은 편지의 파일 이름
/// (`<id>@<읽은 이>.json`)이 갈라지고, `/` 가 안 들어 디렉터리를 벗어나지 못한다.
///
/// [`ANY_IDLE_WORKER`] 는 받는 이 자리의 낱말이라 에이전트가 못 갖는다 — [`is_agent_name`] 이 가른다.
pub fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_MAX
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 에이전트가 가질 수 있는 이름 — [`is_name`] 이고 받는 이 낱말([`ANY_IDLE_WORKER`])이 아니다. **대소문자만 다른 것도
/// 아니다**(리뷰 moai-ew4o.q9f) — 이름이 곧 함의 디렉터리라, 대소문자를 안 가리는 파일 시스템(macOS 기본)에서는
/// `Any-Idle-Worker` 의 함이 열린 편지의 함과 한 디렉터리다. 그 이름의 에이전트는 열린 편지를 제 이름 앞의 편지로 읽어
/// 감독이어도 한 번에 다 가졌고, 떠나면 남은 일감을 모두 보낸 이에게 되돌렸다.
pub fn is_agent_name(name: &str) -> bool {
    is_name(name) && !name.eq_ignore_ascii_case(ANY_IDLE_WORKER)
}

/// 편지를 받을 수 있는 이름 — 에이전트 이름이거나 [`ANY_IDLE_WORKER`] 그대로다. 대소문자만 다른 그 낱말은 아무의 이름도
/// 아니면서(위) 대소문자를 안 가리는 파일 시스템에서는 열린 편지의 함에 들어, 한 사람에게 쓴 편지를 누구나 가진다.
pub fn is_recipient(name: &str) -> bool {
    name == ANY_IDLE_WORKER || is_agent_name(name)
}

/// 남이 지은 글을 이름으로 접는다 — 못 쓰는 글자는 `-` 로, 길면 자른다. Claude 의 세션 이름처럼 우리가
/// 고르지 않은 글을 이름으로 쓸 때 지난다. 접어도 이름이 못 되면 `None` 이다.
///
/// **영숫자가 하나도 없으면 이름이 아니다**(리뷰 moai-h8tn.x4l) — 한글 세션 이름(`감독`)은 접으면 `--` 가 되는데,
/// 그것은 꼴만 이름이고 명령줄에서는 깃발 끝으로 읽혀(`moai send -- …`) 아무도 그 이름으로 못 보낸다. 길이가 같은
/// 다른 한글 이름과도 겹친다. 같은 까닭으로 **앞의 `-` 도 걷는다** — `-` 로 여는 이름은 받는 이 자리에서 깃발이
/// 된다. 못 되면 부르는 쪽이 세션 id·pid 로 짓는다.
pub fn name_from(raw: &str) -> Option<String> {
    let folded: String = raw
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .take(NAME_MAX)
        .collect();
    let folded = folded.trim_start_matches(['.', '-']).to_string();
    (folded.chars().any(|c| c.is_ascii_alphanumeric()) && is_agent_name(&folded)).then_some(folded)
}

/// 이름에 토막을 잇는다 — **잇는 토막이 상한에 안 잘리게** 앞을 줄인다(리뷰 moai-h8tn.x4l). 겹친 이름을 가르려고
/// 이은 토막이 [`name_from`] 의 64자에 잘리면 가른 이름이 도로 같은 이름이 되어, 산 남의 출석을 덮는다.
pub fn name_with(base: &str, tail: &str) -> Option<String> {
    let head: String = base.chars().take(NAME_MAX.saturating_sub(tail.chars().count() + 1)).collect();
    name_from(&format!("{head}-{tail}"))
}

// ── 편지 ──────────────────────────────────────────────────────────────

/// 편지 한 통 — **파일 꼴이 곧 이것이다.** 일곱 키는 늘 적고(`reply_to` 는 없으면 `null`), 모르는 키는
/// [`Letter::rest`] 가 든다.
///
/// 읽을 때는 키마다 `default` 다 — 손으로 놓은 편지나 다른 판이 쓴 편지에서 한 키가 빠졌다고 통째로
/// 못 읽으면 그 편지를 볼 길이 사라진다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Letter {
    #[serde(default)]
    pub v: u64,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub sent_at: String,
    #[serde(default)]
    pub reply_to: Option<String>,
    /// 모르는 키 — 다른 판이 더한 것. 그대로 들고 그대로 낸다.
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

/// 우편함에 선 편지 — 파일 이름에서 읽은 id 와 되돌아온 표, 든 함, 읽었으면 읽은 이.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub id: String,
    /// 든 함의 이름 — 받는 이의 이름이거나 [`ANY_IDLE_WORKER`]. **받는 이는 이것이다**(모듈 머리글).
    pub mailbox: String,
    /// 읽은 이 — 안 읽었으면 `None`. `<함>/read/<id>@<읽은 이>.json` 의 뒤쪽이다.
    pub reader: Option<String>,
    /// 되돌아온 편지인가 — 받는 이가 읽기 전에 떠나 보낸 이의 함으로 돌아왔다([`retire`]). **파일 이름의 표다**
    /// (`<id>.returned.json`, 2026-10-05 사용자 결정) — 편지에 적힌 이름으로 알아내던 판은 이름을 바꾸면 표가 틀리거나
    /// 빠졌다(리뷰 moai-ew4o.q9f 10·11번).
    pub returned: bool,
    pub letter: Letter,
}

impl Stored {
    /// 열린 편지(`any-idle-worker`)인가 — 놀고 있는 일꾼 누구나 가질 수 있다.
    pub fn open(&self) -> bool {
        self.mailbox == ANY_IDLE_WORKER
    }

    /// 이 편지가 함에 선 파일 이름.
    fn file(&self) -> String {
        format!("{}{}.json", self.id, mark(self.returned))
    }
}

/// 되돌아온 편지의 표 — 파일 이름에서 id 뒤, `.json` 앞에 선다. id 에는 `.` 가 없어 갈린다.
const RETURNED: &str = ".returned";

fn mark(returned: bool) -> &'static str {
    if returned { RETURNED } else { "" }
}

/// 함에 선 파일 이름(`.json` 을 뗀 것)을 id 와 되돌아온 표로 가른다 — 편지 꼴이 아니면 `None` 이다.
fn letter_stem(stem: &str) -> Option<(&str, bool)> {
    match stem.strip_suffix(RETURNED) {
        Some(id) if is_id(id) => Some((id, true)),
        _ => is_id(stem).then_some((stem, false)),
    }
}

/// 읽은 편지의 파일 이름(`.json` 을 뗀 것) `<id>[.returned]@<읽은 이>` 를 id·되돌아온 표·읽은 이로 가른다 — [`take`] 가
/// 짓는 이름의 거꾸로다. 읽은 이 자리가 이름의 꼴이 아니면(손으로 놓은 파일) `None` 이다. 읽는 쪽([`list`])과 걷는 쪽
/// ([`sweep_read`])이 이 하나로 가른다 — 저마다 가르면 꼴이 바뀌는 날 한쪽은 새 이름을 영영 안 걷거나 안 보이는 파일을 지운다.
fn read_stem(stem: &str) -> Option<(&str, bool, &str)> {
    let (head, reader) = stem.split_once('@')?;
    let (id, returned) = letter_stem(head)?;
    is_name(reader).then_some((id, returned, reader))
}

/// 못 읽은 파일 — 자리와 까닭. 막지 않고 댄다.
#[derive(Debug, Clone, PartialEq)]
pub struct Garbled {
    pub path: PathBuf,
    pub why: Why,
    /// 든 함 — 편지면 그 함의 이름이고, 출석 파일이면 `None` 이다. 남의 함이 깨진 것을 제 답이 덜 난 것으로 세지
    /// 않으려고 든다(`inbox`). 우편함 자체를 못 열었으면([`Fenced`]) 그것도 `None` 이다 — 제 함도 못 연 것이다.
    pub mailbox: Option<String>,
}

/// 못 읽은 까닭 — 읽다 진 파일이거나, 체크아웃 밖(또는 `.git/` 안)으로 풀려 **안 연 디렉터리**다([`Fenced`]). 둘을 한 글로
/// 접지 않는다 — 안 연 디렉터리는 "못 읽는 편지" 가 아니라 쓰는 길의 거절([`refusal`])과 같은 `<자리>: <까닭>` 으로 댄다.
#[derive(Debug, Clone, PartialEq)]
pub enum Why {
    /// 읽다 진 까닭 — 사람이 읽을 한 줄(크기·JSON·io 의 말).
    Bad(String),
    /// 안 연 디렉터리의 까닭.
    Fenced(crate::held::Unheld),
}

impl Garbled {
    /// 안 연 디렉터리면 그 거절 — 쓰는 길의 [`refusal`] 과 **같은 말·같은 코드**(`broken`)다. 읽다 진 파일이면 `None`.
    pub fn refusal(&self, lang: crate::i18n::Lang) -> Option<crate::fail::Fail> {
        match &self.why {
            Why::Fenced(why) => Some(fenced_fail(lang, &self.path, why)),
            Why::Bad(_) => None,
        }
    }
}

/// 받는 이의 함 — `<우편함>/<이름>/`. 이름은 파일 이름이 될 수 있는 글이다([`is_name`]) — 부르는 쪽이 걸러 둔다.
fn mailbox(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

/// 편지로 못 읽은 파일의 받는 이를 너그럽게 읽는다 — 키 하나의 꼴이 틀렸을 뿐 JSON 이면 `to` 는 읽힌다.
fn addressee(path: &Path) -> Option<String> {
    let v: serde_json::Value = read_json(path).ok()??;
    Some(v.get("to")?.as_str()?.to_string())
}

/// 편지를 받는 이의 함에 들인다 — 새 id 를 낸다. 받는 이가 이름의 꼴이 아니면 안 들인다(그 이름이 디렉터리가 된다).
///
/// temp 에 다 쓰고 `sync` 한 뒤 `hard_link` 로 `<함>/<id>.json` 에 들인다. 이름이 이미 서 있으면(`EEXIST`) 다음
/// 이름으로 다시 든다 — `rename` 이었으면 그 자리의 편지를 말없이 덮었다(모듈 머리글). temp 는 들인 뒤에
/// 지운다. 반쯤 쓴 편지는 숨은 이름이라 읽는 쪽([`list`])이 안 본다.
///
/// **하드 링크를 못 거는 파일 시스템**(`EEXIST` 가 아닌 실패)에서는 그 자리가 비었을 때만 `rename` 으로
/// 물러선다 — 보는 것과 옮기는 것 사이의 틈이 남지만, 그 자리에서 보내기를 통째로 막는 것보다 낫다.
pub fn send(dir: &Path, letter: &Letter) -> std::io::Result<String> {
    send_from(dir, letter, &mut clock_micros)
}

/// [`send`] 의 몸통 — 이름을 지을 시계(`clock`)를 받는다. 시험이 이미 선 이름 위로 보내 보려고 가른다.
fn send_from(dir: &Path, letter: &Letter, clock: &mut dyn FnMut() -> u64) -> std::io::Result<String> {
    if !is_recipient(&letter.to) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, letter.to.clone()));
    }
    let home = home_of(dir);
    let dir = ensure_dir(dir, &home)?;
    // 함도 잰다 — 우편함이 안에 서도 그 밑의 `<받는 이> -> <밖>` 하나로 편지가 밖에 든다.
    let held = make_dir(&mailbox(&dir, &letter.to), &home)?;
    sweep_temps(&held);
    let mut text = serde_json::to_string(letter).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = temp_in(&held);
    write_new(&tmp, text.as_bytes(), true)?;
    let placed = place(&held, &letter.sent_at, None, false, clock, &mut |at| match std::fs::hard_link(&tmp, at) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
            if at.exists() {
                Err(e)
            } else {
                std::fs::rename(&tmp, at).map_err(|_| e)
            }
        }
        done => done,
    });
    let _ = std::fs::remove_file(&tmp);
    if placed.is_ok() {
        sync_dir(&held);
    }
    placed
}

/// 그 함에 비어 있는 id 로 들인다 — `put` 이 `AlreadyExists` 로 지면 다음 id 로 다시 든다. `first` 를 주면 그 id 부터
/// 해 본다(옮기는 편지가 제 id 를 지킨다, [`relocate`]).
///
/// **함의 이름표는 부르는 쪽이 내려 쓴다**([`sync_dir`], 리뷰 moai-ew4o.q9f) — 한 통을 들이는 [`send`] 는 그 자리에서,
/// 여럿을 옮기는 [`retire`]·[`carry`]·[`migrate`] 는 다 옮긴 뒤 함마다 한 번이다. 옮길 때마다 내려 쓰던 판은 편지 수만큼
/// 디렉터리 fsync 를 치러(이 저장소가 선 Ceph RBD 에서 하나에 수십 ms), 짧은 상한이 걸린 훅(Codex `SessionEnd`)이 되돌리는
/// 도중에 끊길 수 있었다 — 장을 걷은 뒤라 남은 편지는 아무도 안 되돌린다. 옮기기는 `rename` 하나라 내려 쓰기 전에 기계가
/// 죽어도 편지는 옛 자리나 새 자리 하나에 선다.
///
/// **이름은 들이기 바로 앞에 짓는다**(리뷰 moai-h8tn.x4l). `EEXIST` 가 지켜 주는 것은 아직 함에 선 편지뿐이다
/// — 읽음으로 옮겨 간 편지의 이름은 비어 보인다. temp 를 쓰고 `sync` 하기 전(수~수십 ms 앞)에 지은 이름은, 같은
/// 마이크로초에 시작한 이웃이 먼저 들인 편지가 그사이 읽혀 나가면 그대로 다시 들어가 한 id 가 두 편지에 선다.
/// 둘째 편지를 같은 이가 읽으면 [`take`] 의 `rename` 이 첫 편지의 `read/` 파일을 말없이 덮는다. 들이기 바로 앞에
/// 지으면 그 틈이 시스템 호출 하나 너비로 준다. 다시 들 때는 시계가 안 갔어도 하나 올린다.
///
/// `returned` 면 되돌아온 표를 단 이름(`<id>.returned.json`)으로 든다. **표가 다른 같은 id 도 선 것으로 친다** — 한 함 안의
/// id 는 하나다(모듈 머리글).
fn place(
    held: &Path,
    sent_at: &str,
    first: Option<&str>,
    returned: bool,
    clock: &mut dyn FnMut() -> u64,
    put: &mut dyn FnMut(&Path) -> std::io::Result<()>,
) -> std::io::Result<String> {
    let mut first = first.map(str::to_string);
    let mut tried: Option<u64> = None;
    for _ in 0..10_000u64 {
        let id = match first.take() {
            Some(id) => id,
            None => {
                let now = clock();
                let micros = tried.map_or(now, |before| now.max(before.wrapping_add(1)));
                tried = Some(micros);
                mint(sent_at, micros)
            }
        };
        if std::fs::symlink_metadata(held.join(format!("{id}{}.json", mark(!returned)))).is_ok() {
            continue;
        }
        match put(&held.join(format!("{id}{}.json", mark(returned)))) {
            Ok(()) => return Ok(id),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free letter id"))
}

/// 편지 id — `<날>-<때>-<8자>`. 앞의 둘은 `sent_at` 의 날과 때(사람이 읽고 고른다), 뒤의 8자는 벽시계의
/// 마이크로초를 36진으로 적은 것이라 **id 의 글자 차례가 보낸 차례다**(같은 초 안에서도). 겹치면
/// [`send`] 가 하나 올려 다시 든다. 8자는 36⁸ 마이크로초(32일 남짓)에서 한 바퀴 돌지만, 앞의 날·때가
/// 같은 초를 다시 만들 일은 없다.
fn mint(sent_at: &str, micros: u64) -> String {
    let digits: String = sent_at.chars().filter(char::is_ascii_digit).collect();
    let (day, time) = if digits.len() >= 14 { (&digits[..8], &digits[8..14]) } else { ("00000000", "000000") };
    format!("{day}-{time}-{}", base36(micros % 36u64.pow(8), 8))
}

fn base36(mut n: u64, width: usize) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![b'0'; width];
    for slot in out.iter_mut().rev() {
        *slot = DIGITS[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// 벽시계의 마이크로초 — **`MOAI_NOW` 를 안 탄다.** 이 값은 id 의 차례를 가르는 것이라, 시계를 멈춘
/// 시험에서도 보낸 차례가 서야 한다. `sent_at` 은 [`crate::model::now`] 로 짓는다.
fn clock_micros() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}

/// `me` 에게 갈 수 있는 편지 — **제 함과 `any-idle-worker` 함, 둘만 연다**(모듈 머리글). `read_too` 면 두 함에서 그 이가
/// 읽은 편지까지 든다. id 차례(보낸 차례)다. 열린 편지를 누가 가질 수 있는지는 [`for_me`] 가 가른다.
///
/// **보통 파일만 읽는다** — `DirEntry::file_type` 은 링크를 안 따라간다. 파이프가 서 있으면 여는 자리에서
/// 멈추고, 링크는 남의 파일을 읽힌다. 숨은 이름(temp)은 안 본다. 함이 없으면 빈 함이다.
///
/// **읽은 편지는 파일 이름으로 먼저 거른다**(리뷰 moai-h8tn.x4l) — 읽은 이가 이름에 서 있으니, 남이 읽은 편지를
/// 열어 가를 까닭이 없다. `read/` 는 읽은 지 며칠([`sweep_read`])이 지나야 걷혀 그동안 쌓이고, `inbox --all --wait` 는
/// 반 초마다 이 자리를 훑는다. 읽은 이 자리가 이름의 꼴이 아니면(손으로 놓은 파일) 편지로 안 센다.
///
/// **체크아웃 밖으로 풀리는 디렉터리는 안 연다**([`reach`], moai-kxkw.7ky) — 우편함이면 하나, 함이나 그 `read/` 면 그
/// 함마다 하나씩 못 읽은 것으로 댄다. 빈 함으로 넘기면 받은 저장소의 링크 하나가 편지를 말없이 감춘다.
///
/// **함과 그 `read/` 를 함께 잰다 — `read_too` 가 아니어도**(리뷰 moai-kxkw.k2f) — `read/` 가 밖으로 풀리는 함의 편지는
/// [`take`] 가 못 옮긴다. 그 편지를 내놓던 판은 훅이 프롬프트마다 옮기려다 말없이 졌고, `inbox --ack` 는 부를 때마다 같은
/// 편지를 새로 받은 것으로 내어 `--wait` 로 일감을 기다리는 일꾼이 같은 일을 되풀이했다. 그 함은 하나로 대고 내놓지 않는다
/// — 편지는 함에 그대로 남는다.
pub fn list(dir: &Path, me: &str, read_too: bool) -> (Vec<Stored>, Vec<Garbled>) {
    list_of(dir, me, read_too, None)
}

/// 편지 하나 — `me` 에게 갈 수 있는 편지 가운데 id 가 `id` 인 것, 읽은 것까지다(moai-54yc.v70, `moai inbox <id>`). **그 id 의
/// 파일만 연다** — id 는 파일 이름에 선다. 두 함과 그 `read/` 를 통째로 열던 판은 읽은 편지가 쌓인 만큼 부를 때마다 다
/// 열었고(5천 통에 0.6초), 그 id 와 무관한 깨진 편지 하나로 그 부름을 덜 낸 것(비영)으로 끝냈다 — 훅이 내미는
/// `inbox <id> --ack` 가 편지를 옮겨 놓고 실패로 읽혔다(리뷰 moai-54yc.vqe). 못 연 자리(체크아웃 밖으로 풀린 우편함·함·
/// `read/`)는 [`list`] 와 같이 댄다. id 는 한 함 안에서만 겹치지 않으니(모듈 머리글) 두 함에 하나씩 설 수 있다.
pub fn list_one(dir: &Path, me: &str, id: &str) -> (Vec<Stored>, Vec<Garbled>) {
    list_of(dir, me, true, Some(id))
}

/// [`list`]·[`list_one`] 의 몸통 — `only` 면 그 id 의 파일만 연다(파일 이름으로 먼저 거른다).
fn list_of(dir: &Path, me: &str, read_too: bool, only: Option<&str>) -> (Vec<Stored>, Vec<Garbled>) {
    let mut letters = Vec::new();
    let mut garbled = Vec::new();
    let home = home_of(dir);
    let real = match reach(dir, &home) {
        Ok(real) => real,
        Err(f) => return (letters, vec![f.garbled(None)]),
    };
    let boxes: &[&str] = if me == ANY_IDLE_WORKER { &[ANY_IDLE_WORKER] } else { &[me, ANY_IDLE_WORKER] };
    for &held in boxes {
        let reached = reach(&mailbox(&real, held), &home).and_then(|at| Ok((reach(&at.join("read"), &home)?, at)));
        let Ok((read, at)) = reached.map_err(|f| garbled.push(f.garbled(Some(held)))) else { continue };
        let (got, bad) = unread_in(&at, held, only);
        letters.extend(got);
        garbled.extend(bad);
        if !read_too {
            continue;
        }
        for (path, stem) in json_files(&read) {
            let Some((id, returned, reader)) = read_stem(&stem) else { continue };
            if reader != me || only.is_some_and(|o| o != id) {
                continue;
            }
            let stored = |letter| Stored {
                id: id.to_string(),
                mailbox: held.to_string(),
                reader: Some(me.into()),
                returned,
                letter,
            };
            match read_json::<Letter>(&path) {
                Ok(Some(letter)) => letters.push(stored(letter)),
                Ok(None) => {}
                Err(why) => garbled.push(Garbled { path, why: Why::Bad(why), mailbox: Some(held.to_string()) }),
            }
        }
    }
    letters.sort_by(|a, b| a.id.cmp(&b.id).then(a.reader.is_some().cmp(&b.reader.is_some())));
    (letters, garbled)
}

/// 한 함의 안 읽은 편지 — 이름 차례는 부르는 쪽이 맞춘다. `at` 은 [`reach`] 를 지난 자리다 — 부르는 쪽이 잰다. `only` 면
/// 그 id 의 파일만 연다([`list_one`]).
fn unread_in(at: &Path, held: &str, only: Option<&str>) -> (Vec<Stored>, Vec<Garbled>) {
    let mut letters = Vec::new();
    let mut garbled = Vec::new();
    for (path, stem) in json_files(at) {
        let Some((id, returned)) = letter_stem(&stem) else { continue };
        if only.is_some_and(|o| o != id) {
            continue;
        }
        match read_json::<Letter>(&path) {
            Ok(Some(letter)) => {
                letters.push(Stored { id: id.to_string(), mailbox: held.to_string(), reader: None, returned, letter })
            }
            Ok(None) => {}
            Err(why) => garbled.push(Garbled { path, why: Why::Bad(why), mailbox: Some(held.to_string()) }),
        }
    }
    (letters, garbled)
}

/// 편지 id 의 꼴인가 — [`mint`] 가 짓는 `<8>-<6>-<8>` 이다. 손으로 놓은 다른 이름의 파일은 편지로 안 센다.
pub fn is_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    matches!(parts.as_slice(), [d, t, n]
        if d.len() == 8 && t.len() == 6 && n.len() == 8
            && d.chars().chain(t.chars()).all(|c| c.is_ascii_digit())
            && n.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()))
}

/// 편지 id 의 보낸 때(초) — 앞의 `<날>-<때>` 다([`mint`] 가 `sent_at` 에서 딴다, UTC). 보낸 때를 못 읽어 지은 id
/// (`00000000-000000-…`)와 꼴이 아닌 글은 `None` 이다.
fn sent_at_of(id: &str) -> Option<i64> {
    let at = |a: usize, z: usize| id.get(a..z);
    crate::model::parse_rfc3339(&format!(
        "{}-{}-{}T{}:{}:{}Z",
        at(0, 4)?,
        at(4, 6)?,
        at(6, 8)?,
        at(9, 11)?,
        at(11, 13)?,
        at(13, 15)?
    ))
}

/// 이 편지가 `me` 에게 가는가 — 제 함에 든 것, 그리고 `any-idle-worker` 편지 가운데 제가 보내지 않은 것(감독은
/// 빼고, 2026-10-04 사용자 결정). 읽은 편지는 읽은 이가 `me` 인 것만이다.
pub fn for_me(stored: &Stored, me: &str, role: &str) -> bool {
    if let Some(reader) = &stored.reader {
        return reader == me;
    }
    if stored.open() { may_take_open(me, role, &stored.letter.from) } else { stored.mailbox == me }
}

/// `any-idle-worker` 편지를 가질 수 있는가 — 보낸 이가 아니고 감독이 아니다(2026-10-04 사용자 결정). [`for_me`] 와
/// [`idle_worker`] 가 이 하나로 잰다 — 따로 적으면 `send --wake` 가 깨운 일꾼이 정작 그 편지를 못 가지는 날이 온다.
fn may_take_open(me: &str, role: &str, from: &str) -> bool {
    from != me && role != SUPERVISOR
}

/// 편지 하나를 가졌는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// 이 프로세스가 옮겼다 — 이제 `<함>/read/<id>@<읽은 이>.json` 이다.
    Mine,
    /// 그 자리에 없었다 — 남이 먼저 가졌다(`any-idle-worker`)거나 이미 읽었다.
    Lost,
}

/// 편지를 읽음으로 옮긴다 — `rename` 하나다(모듈 머리글). 먼저 옮긴 쪽만 [`Took::Mine`] 을 받는다. 되돌아온 편지는 그
/// 표를 읽은 이 앞에 들고 간다(`read/<id>.returned@<읽은 이>.json`) — 읽은 이의 이름에는 `.` 가 들 수 있어 뒤에 못 단다.
///
/// **읽고 지우는 두 걸음으로 바꾸지 않는다** — 둘이 같이 읽고 같이 지우면 한 편지가 두 세션에 실린다.
/// 겨루기 시험 둘(`tests/cli.rs` 의 `concurrent_acks_deliver_each_letter_once`·`an_open_letter_goes_to_exactly_one_worker`)이
/// 그 자리를 잡는다 — 복사하고 지우는 꼴로 바꾸어 재 보니 둘 다 붉어졌다.
///
/// `rename` 은 `read/<id>@<읽은 이>.json` 이 이미 서 있으면 **말없이 덮는다** — 그 자리가 비어 있다는 것은 id 가
/// 두 번 안 서는 것([`place`] 가 이름을 들이기 바로 앞에 짓는다)에 기댄다.
///
/// **읽은 때는 옮기기 앞에 적는다**([`stamp_read`]) — [`sweep_read`] 가 그것으로 읽은 지 며칠인지 잰다. 옮긴 뒤에 적던 판은
/// 두 틈을 남겼다(리뷰 moai-kxkw.k2f). 옮긴 편지가 보낸 때를 든 채 `read/` 에 선 사이에 걷기가 그것을 지울 수 있었고, 훅이
/// 옮긴 뒤 글을 내기 전에 쓰기(메타데이터) 하나를 더 치러 저장소가 멈춘 날 그 사이에 상한에 끊기면 읽음이 된 편지가
/// 아무에게도 안 실렸다 — moai-jzym.flj 가 출석 쓰기를 옮기기 앞으로 당긴 그 까닭이다. 이제 거기서 멈추면 편지는 안 읽은 채
/// 남는다. 겨루기에 진 쪽(남이 먼저 가졌다)이 적은 시각도 지금이라 해가 없고, 안 읽은 편지의 수정 시각은 아무도 안 읽는다.
pub fn take(dir: &Path, stored: &Stored, reader: &str) -> std::io::Result<Took> {
    let home = home_of(dir);
    let held = reach(&mailbox(dir, &stored.mailbox), &home)?;
    let read = make_dir(&held.join("read"), &home)?;
    let from = held.join(stored.file());
    stamp_read(&from);
    match std::fs::rename(&from, read.join(format!("{}{}@{reader}.json", stored.id, mark(stored.returned)))) {
        Ok(()) => Ok(Took::Mine),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Took::Lost),
        Err(e) => Err(e),
    }
}

/// 이 편지를 `reader` 가 읽음으로 옮겼는가 — 그 함의 `read/<id>[.returned]@<reader>.json` 이 섰다([`take`] 가 짓는 이름).
/// [`Took::Lost`] 를 받은 쪽이 **누가** 가졌는지 묻는다(`inbox` 의 기다림, 리뷰 moai-54yc.vqe) — 같은 이름의 다른 부름(그
/// 세션의 훅, 같은 이름의 옆 기다림)이 가졌으면 그 이름은 편지를 얻은 것이다. 함이 체크아웃 밖으로 풀리면 모른다(`false`).
pub fn read_by(dir: &Path, stored: &Stored, reader: &str) -> bool {
    let Ok(held) = reach(&mailbox(dir, &stored.mailbox), &home_of(dir)) else { return false };
    let name = format!("{}{}@{reader}.json", stored.id, mark(stored.returned));
    std::fs::symlink_metadata(held.join("read").join(name)).is_ok()
}

/// 읽은 때를 그 편지의 수정 시각에 적는다 — 지금([`crate::model::now`], 시험은 `MOAI_NOW` 로 못박는다). 편지는 한 번
/// 쓰고 안 고치니 그 칸이 비어 있고, `rename` 은 그 시각을 안 바꾼다. 적는 때는 옮기기 앞이다([`take`]).
///
/// **열지 않고 경로로 적는다**(리눅스 `utimensat`, 링크를 안 따른다) — 링크면 그 링크 제 시각을 고치고 끝은 안 건드리며,
/// FIFO 앞에서도 안 멈춘다. **시각을 못박아 적는 것은 그 파일의 임자만 한다** — 쓰기 권한으로는 안 된다. 다른 uid 가 보낸
/// 편지(같은 저장소를 쓰는 다른 컨테이너)는 `EPERM` 으로 지니, 그때는 "지금" 으로 적는다 — 그것은 쓰기 권한이면 되고
/// `MOAI_NOW` 는 못 탄다. 둘 다 못 적으면 그 편지는 보낸 때로 재여, 그보다 오래 기다린 편지면 **다음 걷기에 걷힌다** — 이미
/// 실린 편지가 `inbox --all` 에서 사라질 뿐, 안 읽은 편지는 안 잃는다. 리눅스 밖은 막히지 않게(`O_NONBLOCK`), 링크를 안
/// 따라(`O_NOFOLLOW`) 읽기로 열어 적는다 — 임자는 쓰기 권한 없이도 적는다.
fn stamp_read(path: &Path) {
    let Some(secs) = crate::model::parse_rfc3339(&crate::model::now()) else { return };
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return };
        // SAFETY: `timespec` 는 맨 자료라 0 으로 지어도 된다 — 32비트 판은 숨은 칸을 들어 글자 그대로는 못 짓는다.
        let mut times: [libc::timespec; 2] = unsafe { std::mem::zeroed() };
        times[0].tv_nsec = libc::UTIME_OMIT;
        times[1].tv_sec = secs as libc::time_t;
        let set = |times: &[libc::timespec; 2]| {
            // SAFETY: `c` 는 NUL 로 끝나는 산 C 글이고 `times` 는 두 칸짜리 배열이다 — 둘 다 부름이 끝날 때까지 산다.
            unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), libc::AT_SYMLINK_NOFOLLOW) }
        };
        if set(&times) != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
            times[0].tv_nsec = libc::UTIME_NOW;
            times[1] = times[0];
            let _ = set(&times);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let Ok(secs) = u64::try_from(secs) else { return };
        let mut o = std::fs::OpenOptions::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.read(true).custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        }
        // 유닉스 밖은 시각을 고치는 손잡이가 쓰기로 열려야 한다.
        #[cfg(not(unix))]
        o.write(true);
        if let Ok(f) = o.open(path) {
            let _ = f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs));
        }
    }
}

/// 읽은 지 `days` 날이 **지난** 편지를 걷는다 — 걷은 수를 낸다. `0` 이면 안 걷는다(2026-10-05 사용자 결정,
/// moai-kxkw.my1).
///
/// 읽음으로 옮긴 편지([`take`])는 지우는 길이 없어 `read/` 에 끝없이 쌓였다 — 감독을 오래 돌릴수록 `.moai/mail` 이
/// 컸다. 날수는 저장소의 설정(`mail_read_days`, 안 적으면 7일)이고, 읽은 때는 [`take`] 가 그 파일의 수정 시각에 적어
/// 둔 것이다([`stamp_read`]). 그것을 안 적던 판에 읽은 편지는 그 시각이 보낸 때라 그것으로 잰다 — 읽은 때보다 이르다.
///
/// - **안 읽은 편지는 안 걷는다** — 떠난 이 앞의 편지는 [`retire`] 가 보낸 이에게 되돌린다
/// - **읽은 편지의 꼴인 이름만 걷는다**(`<id>@<읽은 이>.json`) — 손으로 놓은 파일은 그대로다
/// - 함마다 [`reach`] 로 잰다 — 체크아웃 밖으로 풀리는 함의 `read/` 는 안 연다. 밖에 선 남의 파일을 지우는 것이 이
///   에픽이 막은 바로 그 꼴이다(moai-kxkw.7ky)
/// - **걷는 자리는 `moai agents` 하나다** — 죽은 장을 걷는 그 자리다([`sweep`]). 훅과 `send`·`inbox` 는 안 걷는다 —
///   도구 호출마다 도는 자리가 함을 다 훑지 않는다
/// - **보낸 때가 금보다 늦은 편지는 이름만 보고 넘긴다** — 읽은 때는 보낸 때보다 늦으니 그 편지는 아직 걷을 때가 아니다.
///   감독의 깨우기 글이 이 명령을 반 초마다 부르는 자리라(`moai-supervise` 의 5-1) 편지마다 `stat` 을 치르지 않는다. 앞날로
///   하루([`crate::model::FUTURE_SLACK_SECS`])를 넘는 이름은 손으로 놓은 것이라 수정 시각으로 잰다
pub fn sweep_read(dir: &Path, days: i64) -> usize {
    if days <= 0 {
        return 0;
    }
    let Some(now) = crate::model::parse_rfc3339(&crate::model::now()) else { return 0 };
    let cutoff = now.saturating_sub(days.saturating_mul(24 * 60 * 60));
    let home = home_of(dir);
    let Ok(real) = reach(dir, &home) else { return 0 };
    let Ok(entries) = std::fs::read_dir(&real) else { return 0 };
    let mut swept = 0;
    for name in entries.filter_map(Result::ok).filter_map(|e| e.file_name().into_string().ok()) {
        if !is_recipient(&name) {
            continue;
        }
        let Ok(read) = reach(&mailbox(&real, &name).join("read"), &home) else { continue };
        for (path, stem) in json_files(&read) {
            let Some((id, _, _)) = read_stem(&stem) else { continue };
            if sent_at_of(id).is_some_and(|sent| sent >= cutoff && sent <= now + crate::model::FUTURE_SLACK_SECS) {
                continue;
            }
            let read_at = std::fs::symlink_metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_secs()).ok());
            if read_at.is_some_and(|at| at < cutoff) && std::fs::remove_file(&path).is_ok() {
                swept += 1;
            }
        }
    }
    swept
}

// ── 함 옮기기 ────────────────────────────────────────────────────────

/// 떠난 이름의 안 읽은 편지를 보낸 이의 함으로 되돌린다 — 되돌린 수를 낸다(2026-10-04 사용자 결정, moai-ew4o.l3n).
///
/// 이름은 Claude 가 디렉터리마다 256개 안에서 짓고 세션은 그보다 훨씬 많이 뜬다 — 떠난 세션 앞으로 남은 편지가 그
/// 이름을 이어받은 새 세션의 첫 프롬프트에 실려 읽음이 되던 자리다. 그래서 이름을 쥔 장이 떠났다고 **확실할 때** 그
/// 함을 비운다: 산 것이 아닌 장의 이름을 새 장이 이어받을 때(훅·`hello`), `moai agents` 가 죽은 장을 걷을 때, Codex
/// 세션이 끝날 때다. 보낸 이에게는 "읽기 전에 떠났다" 로 실린다([`Stored::returned`]).
///
/// - **아무도 쥐지 않은 이름 앞의 편지는 그대로다** — 아직 인사하지 않은 에이전트에게 먼저 보낸 편지는 다음에 그
///   이름을 받는 세션의 것이다. 되돌리는 때는 장이 떠난 때지 편지가 온 때가 아니다
/// - **제가 쓴 편지와 되돌아온 편지는 떠난 이가 읽은 것으로 둔다** — 보낸 이가 곧 떠난 이라 돌려보낼 곳이 없다
/// - 깨진 편지는 그대로 둔다 — 보낸 이를 모른다
/// - 돌려보낸 편지는 보낸 이의 함에 **되돌아온 표**를 달고 든다([`Stored::returned`])
pub fn retire(dir: &Path, name: &str) -> usize {
    let home = home_of(dir);
    let Ok(at) = reach(&mailbox(dir, name), &home) else { return 0 };
    let (letters, _) = unread_in(&at, name, None);
    let mut touched: Vec<PathBuf> = Vec::new();
    let mut returned = 0;
    for s in letters {
        let from = s.letter.from.as_str();
        if s.returned || from == name || !is_agent_name(from) {
            let _ = take(dir, &s, name);
            continue;
        }
        let (path, into) = (at.join(s.file()), mailbox(dir, from));
        if let Ok(Some((_, into))) = relocate(&path, &into, &home, &s.id, true, &s.letter.sent_at) {
            returned += 1;
            if !touched.contains(&into) {
                touched.push(into);
            }
        }
    }
    // 이름표는 다 옮긴 뒤 함마다 한 번 내려 쓴다([`place`]).
    touched.iter().for_each(|d| sync_dir(d));
    returned
}

/// 이름을 바꾼 에이전트의 안 읽은 편지를 새 이름의 함으로 옮긴다 — 받는 이는 같은 에이전트다(moai-ew4o.l3n).
/// 옛 이름 앞에 남은 편지는 아무도 못 읽고 남던 자리다.
pub fn carry(dir: &Path, old: &str, new: &str) {
    let home = home_of(dir);
    let Ok(at) = reach(&mailbox(dir, old), &home) else { return };
    let into = mailbox(dir, new);
    let (letters, _) = unread_in(&at, old, None);
    let mut moved = None;
    for s in letters {
        if let Ok(Some((_, real))) = relocate(&at.join(s.file()), &into, &home, &s.id, s.returned, &s.letter.sent_at) {
            moved = Some(real);
        }
    }
    if let Some(into) = moved {
        sync_dir(&into);
    }
}

/// 한 함에 모두 두던 판의 편지(`<우편함>/<id>.json`·`<우편함>/read/<id>@<읽은 이>.json`)를 받는 이의 함으로 옮긴다
/// (moai-ew4o.c92). 그 꼴은 릴리스 전에만 섰다 — 그 판으로 보낸 편지가 판을 갈아 끼우는 순간 길을 잃지 않게
/// `send`·`inbox` 가 부를 때마다 옮긴다. **훅은 안 부른다** — 훅이 여는 자리는 두 함뿐이다. 받는 이를 못 읽는 파일은
/// 그대로 둔다(그것은 이제 아무의 함에도 안 선다).
///
/// 이름이 `read` 인 에이전트의 함도 `<우편함>/read/` 다 — 그 함의 편지에는 `@` 가 없어 옛 읽은 편지와 갈린다.
pub fn migrate(dir: &Path) {
    // 푼 자리는 `real` 로 따로 든다 — [`home_of`] 는 받은 철자(`<뿌리>/.moai/mail`)에서만 맞아, 같은 이름으로 가리면 푼 자리를
    // 넘겨 뿌리를 어긋나게 재는 부름이 지어진다(리뷰 moai-kxkw.k2f).
    let home = home_of(dir);
    let Ok(real) = reach(dir, &home) else { return };
    let mut touched: Vec<PathBuf> = Vec::new();
    let mut moved = |into: PathBuf| {
        if !touched.contains(&into) {
            touched.push(into);
        }
    };
    for (path, stem) in json_files(&real) {
        if !is_id(&stem) {
            continue;
        }
        let Some(to) = addressee(&path).filter(|to| is_name(to)) else { continue };
        let sent_at = read_json::<Letter>(&path).ok().flatten().map(|l| l.sent_at).unwrap_or_default();
        if let Ok(Some((_, into))) = relocate(&path, &mailbox(&real, &to), &home, &stem, false, &sent_at) {
            moved(into);
        }
    }
    // 옛 꼴의 읽은 편지 — 그 자리가 체크아웃 밖으로 풀리면 안 연다.
    let read = reach(&real.join("read"), &home).map(|read| json_files(&read)).unwrap_or_default();
    for (path, stem) in read {
        let Some((id, reader)) = stem.split_once('@') else { continue };
        if !is_id(id) || !is_name(reader) {
            continue;
        }
        let Some(to) = addressee(&path).filter(|to| is_name(to)) else { continue };
        let Ok(into) = make_dir(&mailbox(&real, &to).join("read"), &home) else { continue };
        if move_new(&path, &into.join(format!("{stem}.json"))).is_ok() {
            moved(into);
        }
    }
    // 이름표는 다 옮긴 뒤 함마다 한 번 내려 쓴다([`place`]).
    touched.iter().for_each(|d| sync_dir(d));
}

/// 편지 하나를 다른 함으로 옮긴다 — **덮지 않는다**([`move_new`]). 그 함에 같은 id 가 서 있으면 다음 id 로 든다 —
/// id 는 한 함 안에서만 겹치지 않는다(모듈 머리글). 옮긴 id 와 그 함의 푼 자리를 내고, 옮기기 전에 남이 가졌으면
/// (읽었거나 먼저 옮겼으면) `None` 이다. **그 함의 이름표는 안 내려 쓴다** — 여럿을 옮기는 쪽이 다 옮긴 뒤 한 번 쓴다
/// ([`place`]). 옮길 함이 체크아웃 밖으로 풀리면 안 옮긴다([`make_dir`]) — 편지는 제자리에 남는다.
fn relocate(
    from: &Path,
    into: &Path,
    home: &crate::held::Home,
    id: &str,
    returned: bool,
    sent_at: &str,
) -> std::io::Result<Option<(String, PathBuf)>> {
    let into = make_dir(into, home)?;
    match place(&into, sent_at, Some(id), returned, &mut clock_micros, &mut |at| move_new(from, at)) {
        Ok(id) => Ok(Some((id, into))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// 덮지 않는 `rename` — 그 자리에 무엇이 있으면 `AlreadyExists` 다. `rename` 은 같은 이름을 말없이 덮어, 옮긴 자리의
/// 편지 하나가 사라진다.
///
/// 리눅스는 `renameat2(RENAME_NOREPLACE)`, macOS 는 `renamex_np(RENAME_EXCL)` 로 **한 번에** 한다. 그 길이 없는
/// 자리(다른 유닉스, 그 깃발을 모르는 파일 시스템)는 `link` 뒤 `unlink` 로 물러선다 — 그 사이에 남이 옛 자리의 편지를
/// 가지면 들인 것을 걷고 진다. 걷기 전에 옮긴 자리에서 누가 읽으면 한 편지가 둘에게 실린다 — 그 틈은 시스템 호출
/// 둘 사이다.
///
/// **링크도 못 거는 파일 시스템**(일부 FUSE, FAT)에서는 그 자리가 비었을 때만 `rename` 으로 물러선다 — [`send_from`] 과
/// 같은 자다(리뷰 moai-ew4o.q9f). 안 물러서던 판은 거기서 되돌리기·따라가기·옮기기가 말없이 다 졌다: 장은 걷혔는데 편지는
/// 떠난 이의 함에 남아 그 이름을 받는 다음 세션에 실렸고, 옛 꼴의 편지는 아무의 함에도 안 섰다. 보는 것과 옮기는 것
/// 사이의 틈은 남는다.
fn move_new(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes());
        let (Ok(a), Ok(b)) = (c(from), c(to)) else {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        };
        // SAFETY: 두 글은 NUL 로 끝나는 산 C 글이고 부름이 끝날 때까지 산다. 상대 경로는 이 프로세스의 자리에서 푼다.
        #[cfg(target_os = "linux")]
        let r = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                a.as_ptr(),
                libc::AT_FDCWD,
                b.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        // SAFETY: 위와 같다.
        #[cfg(target_os = "macos")]
        let r = unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_EXCL) } as libc::c_long;
        if r == 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if !matches!(e.raw_os_error(), Some(libc::EINVAL | libc::ENOSYS | libc::ENOTSUP)) {
            return Err(e);
        }
    }
    match std::fs::hard_link(from, to) {
        Ok(()) => {}
        Err(e) if matches!(e.kind(), std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::NotFound) => {
            return Err(e);
        }
        Err(_) if std::fs::symlink_metadata(to).is_ok() => {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        }
        Err(_) => return std::fs::rename(from, to),
    }
    std::fs::remove_file(from).inspect_err(|_| {
        let _ = std::fs::remove_file(to);
    })
}

// ── 출석 ──────────────────────────────────────────────────────────────

/// 출석 한 장 — `.moai/agents/<name>.json`. 에이전트가 `moai hello` 로, 훅이 `SessionStart` 에서 쓴다.
///
/// `pid` 는 **에이전트 프로세스**다(훅의 셸이 아니다). `pid_start` 는 그 프로세스가 선 때(리눅스
/// `/proc/<pid>/stat` 의 22째 칸)라, pid 가 재사용돼도 죽은 출석이 산 것으로 안 보인다([`alive`]).
/// `machine` 은 그 pid 가 뜻을 갖는 자리다 — 다른 기계의 장은 pid 로 안 잰다([`Presence::here`]).
/// 없을 수 있는 키(`pid_start`·`machine`·`host`·`session`·`tmux_*`)는 없으면 안 적는다 — 없음이 곧 "모른다" 다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    #[serde(default)]
    pub v: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub since: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_start: Option<u64>,
    /// 그 pid 가 선 기계 — [`machine`] 의 값이다(moai-dhxm). 같은 저장소를 다른 기계(컨테이너)에서 쓰면 그 pid 는 이
    /// 기계의 `/proc` 에 없거나 남의 프로세스라, 이 값이 이 기계의 것과 다른 장은 프로세스로 안 재고 닻으로 잰다. **pid 와
    /// 함께 간다**([`Presence::at`]) — 따로 적으면 이 기계의 pid 가 남의 기계 이름표를 단다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// 그 기계의 호스트 이름 — 사람 화면에 "어디의 pid 인가" 를 대는 데만 쓴다. **견주지 않는다** — 컨테이너끼리 같은
    /// 이름을 쓸 수 있고, 같은 기계의 이름이 바뀔 수도 있다. 같은 기계인가는 `machine` 이 말한다. `machine` 을 적을 때만
    /// 적는다 — 기계를 모르면 댈 곳이 없다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// 벤더의 세션 id — 훅이 이 세션의 출석을 찾는 열쇠다. 깨우기에는 안 쓴다(moai 는 에이전트를 안 띄운다).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_pane: Option<String>,
    /// 그 칸이 선 tmux 서버의 소켓 — `$TMUX` 의 첫 토막. 깨울 때 `-S` 로 그 서버만 겨눈다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_socket: Option<String>,
    /// 살아 있다는 닻 — 그 에이전트의 훅이나 기다림이 마지막으로 이 장을 적은 때다(moai-j3n5). **모든 장에 적는다**
    /// (moai-dhxm) — 읽는 쪽이 프로세스로 못 재는 장(`pid` 0 인 Codex, 다른 기계의 장)은 오래 안 적힌 것을 떠난 것으로
    /// 본다([`Presence::stale`]). 프로세스를 아는 장도 적는 것은, 그 장을 프로세스로 재는 것이 이 기계뿐이라서다 — 같은
    /// 저장소를 쓰는 다른 기계는 이것으로만 잰다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<String>,
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

/// 프로세스로 못 재는 장(pid 를 모르거나 다른 기계의 것)이 이만큼(초) 넘게 안 적혔으면 떠난 것으로 본다(2026-10-04 사용자
/// 결정, moai-j3n5) — 20분이다.
/// Codex 의 장은 pid 를 몰라(훅이 세션 여럿이 함께 쓰는 데몬 밑에서 돈다, moai-sile) 세션이 죽거나 `SessionEnd` 가 짧은
/// 상한을 넘기면 `idle` 로 영영 남았고, `send any-idle-worker --wake` 가 그 죽은 장을 골라 산 일꾼을 안 깨웠다.
///
/// **떠난 것으로 볼 뿐 지우지 않는다**(2026-10-05 사용자 결정, 리뷰 moai-ew4o.q9f 1·3번) — 깨우기·`--status idle`·이름
/// 겨루기가 건너뛰고(다른 기계의 장이 쥔 지은 이름만은 하루를 기다린다 — [`Presence::holds_made_name`]) `moai agents` 가
/// [`GONE`] 으로 보이지만, 장(역할·`hello --name` 의 이름)과 함은 남아 그 세션의 다음 훅에 살아난다. 지우던 판은 돌아온
/// 감독이 역할을 잃고 `any-idle-worker` 일감을 가졌고, 같은 65초 안에 연 두 세션이 지워진 뒤 돌아오면 먼저 온 쪽이 남의
/// 편지를 받았다. 지우는 것은 [`EXPIRE_AFTER`] 넘게 안 적힌 장이다.
pub const STALE_AFTER: i64 = 20 * 60;

/// 프로세스로 못 재는 장이 이만큼(초) 넘게 안 적혔으면 `moai agents` 가 지운다 — 하루다(같은 결정). 그 세션은 끝났는데
/// `SessionEnd` 가 안 왔거나(죽었다·짧은 상한을 넘겼다) 하루 넘게 아무것도 안 한 것이다. Codex 의 장(pid 0)은 그 함을
/// 그대로 둔다 — 그 세션이 돌아오면 Codex 의 이름은 세션 id 에서 지어 같은 이름을 다시 받는다. 다른 기계의 장도 하루를
/// 기다린다 — 그 기계에서는 아직 살아 있을 수 있다. 그 함은 걷을 때 비운다([`sweep`]). 그 장이 지은 이름을 놓는 때도
/// 이것이다([`Presence::holds_made_name`]).
pub const EXPIRE_AFTER: i64 = 24 * 60 * 60;

/// [`STALE_AFTER`] 넘게 안 적힌 장을 `moai agents` 가 보이는 상태 — 파일에는 안 적는다(보는 쪽이 그때 잰다).
pub const GONE: &str = "gone";

/// 닻을 이만큼(초)마다 다시 적는다 — 도구 부름마다 장을 다시 쓰지 않으려고 둔다. [`STALE_AFTER`] 보다 한참 짧다.
pub const SEEN_EVERY: i64 = 60;

impl Presence {
    /// 지금 이 프로세스가 선 tmux 칸 — `$TMUX_PANE` 과 `$TMUX` 의 소켓. 에이전트가 띄운 셸과 훅은 그
    /// 에이전트의 환경을 물려받으므로 그 칸이 곧 에이전트의 칸이다.
    pub fn tmux_here() -> (Option<String>, Option<String>) {
        let pane = std::env::var("TMUX_PANE").ok().filter(|p| !p.is_empty());
        let socket =
            std::env::var("TMUX").ok().and_then(|t| t.split(',').next().map(str::to_string)).filter(|s| !s.is_empty());
        (pane, socket)
    }

    /// 이 장이 그 프로세스의 것인가 — 이 기계의 장이고([`Presence::here`]), pid 가 같고, 선 때를 둘 다 알면 그것도
    /// 같다(pid 는 재사용된다). **"같은 에이전트인가" 는 이 하나로 잰다** — `hello`·훅·[`me_among`] 이 저마다 적던 자리다.
    /// 다른 기계의 장은 pid 가 겹쳐도 이 프로세스가 아니다 — 겹친 것으로 읽던 판은 훅이 그 장을 제 것으로 이어 쓰거나
    /// "한 프로세스는 장 하나" 로 걷고 그 함을 되돌렸다(moai-dhxm).
    pub fn runs_as(&self, p: &Proc) -> bool {
        self.here() && self.pid == p.pid && (self.pid_start.is_none() || p.start.is_none() || self.pid_start == p.start)
    }

    /// 이 장의 pid 가 이 기계의 것인가(moai-dhxm) — 장에 적힌 기계가 이 프로세스의 기계([`machine`])와 같다. **기계를 안
    /// 적은 장은 이 기계의 것으로 읽는다** — 이 필드 전의 판과 기계를 못 읽은 판이 적은 장이고, 그 판이 하던 그대로다.
    /// 이 프로세스가 제 기계를 모르면 기계를 적은 장은 남의 것이다 — 같다고 말할 수 없다.
    pub fn here(&self) -> bool {
        match self.machine.as_deref() {
            None => true,
            Some(m) => machine().is_some_and(|h| h == m),
        }
    }

    /// 이 장을 프로세스로 재는가 — pid 를 알고 그 pid 가 이 기계의 것이다. 아니면 닻으로 잰다([`Presence::stale`]).
    fn by_process(&self) -> bool {
        self.pid != 0 && self.here()
    }

    /// pid 를 이 기계의 프로세스로 적는다 — 기계와 호스트 이름이 pid 와 함께 간다(moai-dhxm). 장에 이 기계의 pid 를 적는
    /// 자리(훅·`hello`)가 이 하나로 적는다. pid 0(모른다)이면 둘 다 비운다 — 가리킬 프로세스가 없다. 이 기계를 못 읽으면
    /// 호스트 이름도 안 적는다([`Presence::host`]).
    pub fn at(self, pid: u32, pid_start: Option<u64>) -> Presence {
        let machine = if pid == 0 { None } else { machine() };
        let host = machine.as_ref().and_then(|_| hostname());
        Presence { pid, pid_start, machine, host, ..self }
    }

    /// 기계를 안 적은 옛 장의 프로세스가 이 기계에 살아 있으면 기계를 단다(moai-dhxm) — 이 필드 전의 판이 적은 장은 같은
    /// 저장소를 쓰는 다른 기계가 pid 로 재어 걷고 그 함의 편지를 되돌린다. 산 세션의 장을 그대로 이어 쓰기만 하면 판을
    /// 올린 날 돌던 세션은 끝날 때까지 기계를 안 단다. 그 장을 다시 쓰는 자리(훅의 [`crate::cmd::hook`] `attendee`·
    /// [`keep_alive`])가 이것을 지난다.
    ///
    /// **산 것을 이 기계의 `/proc` 로 확인했을 때만 단다**([`alive`] 가 `Some(true)`) — 그 pid 가 이 기계의 것이라는 뜻이라
    /// pid 와 기계가 함께 간다([`Presence::at`]). 모르면(`None`) 안 단다. pid 를 모르는 장(Codex)과 이미 기계를 적은 장은
    /// 그대로다 — 남의 기계의 장을 이 기계의 것으로 고쳐 적지 않는다.
    pub fn claimed(self) -> Presence {
        if self.machine.is_some() || self.pid == 0 || machine().is_none() {
            return self;
        }
        match alive(self.pid, self.pid_start) {
            Some(true) => {
                let (pid, start) = (self.pid, self.pid_start);
                self.at(pid, start)
            }
            _ => self,
        }
    }

    /// 그 에이전트의 프로세스가 없다고 확실한가 — **모르면 아니다**([`alive`] 가 `None`). 걷을 때 그 이름 앞의 편지를
    /// 보낸 이에게 되돌리는 것은 이것뿐이다([`sweep`]). **다른 기계의 장은 모른다**(moai-dhxm) — 그 pid 는 이 기계의
    /// `/proc` 에 없을 뿐이다. 없다고 읽던 판은 컨테이너 여럿이 한 저장소를 쓰면 `moai agents` 가 남의 산 장을 걷고 그
    /// 함의 편지를 보낸 이에게 되돌렸다.
    pub fn dead(&self) -> bool {
        self.by_process() && alive(self.pid, self.pid_start) == Some(false)
    }

    /// 닻이 낡았는가 — 프로세스로 못 재는 장이 [`STALE_AFTER`] 넘게 안 적혔다(moai-j3n5). 둘 다 못 읽으면 모른다(낡지
    /// 않았다).
    ///
    /// **닻과 상태를 바꾼 때(`since`) 가운데 늦은 쪽으로 잰다**(리뷰 moai-ew4o.q9f) — 닻을 모르는 바이너리(이 필드 전의 판,
    /// 훅이 그 판을 부르는 동안)는 장을 고쳐 적어도 `seen` 을 모르는 키로 그대로 옮기고 `since` 만 새로 댄다. 닻만 보던
    /// 판은 그 훅이 도는 산 장을 새 판의 `hello` 가 닻을 적은 지 20분 뒤에 걷었다. 닻이 없는 장도 이 자로 `since` 다.
    pub fn stale(&self, now: &str) -> bool {
        self.quiet(now).is_some_and(|secs| secs > STALE_AFTER)
    }

    /// 지울 때가 되었는가 — 프로세스로 못 재는 장이 [`EXPIRE_AFTER`] 넘게 안 적혔다([`sweep`]).
    pub fn expired(&self, now: &str) -> bool {
        self.quiet(now).is_some_and(|secs| secs > EXPIRE_AFTER)
    }

    /// 프로세스로 못 재는 장이 몇 초째 안 적혔나 — 닻과 `since` 가운데 늦은 쪽부터 잰다. 프로세스로 재는 장이거나 못
    /// 읽으면 `None` 이다(모른다).
    fn quiet(&self, now: &str) -> Option<i64> {
        if self.by_process() {
            return None;
        }
        let at = [self.seen.as_deref(), Some(self.since.as_str())]
            .into_iter()
            .flatten()
            .filter_map(crate::model::parse_rfc3339)
            .max()?;
        Some(crate::model::parse_rfc3339(now)? - at)
    }

    /// 그 에이전트가 떠났다고 보는가 — 프로세스가 죽었거나 닻이 낡았다. **모르면 아니다.** 걷기·깨우기와 창이 대는
    /// 이름(`MOAI_AGENT`·`hello --name`)의 겨루기가 이 하나로 잰다. 지은 이름의 겨루기는 [`Presence::holds_made_name`] 이다.
    pub fn gone(&self) -> bool {
        self.dead() || self.stale(&crate::model::now())
    }

    /// 지은 이름을 아직 쥐었는가 — 이름 겨루기에서 **지은 이름**(Claude 의 세션 이름, `<벤더>-<세션 토막>`,
    /// `codex-<토막>`, `<벤더>-<pid>`)이 빈 것인지를 이 하나로 잰다(2026-10-05 사용자 결정, moai-nas5). **다른 기계의
    /// 장은 하루([`EXPIRE_AFTER`]) 넘게 안 적힐 때까지 쥔다** — 보이는 상태([`Presence::gone`])와 이름을 놓는 때를 가른다.
    /// 그 밖의 장은 떠나지 않은 동안 쥔다. **다른 기계의 장은 기계를 적은 장이다**([`Presence::here`]) — 기계를 안 적은 장은
    /// 이 기계의 장처럼 잰다: Codex 의 장(pid 0, [`Presence::at`])은 20분 조용하면 그 이름을 놓고, 이 필드 전의 판이나 리눅스
    /// 밖에서 적은 장은 그 pid 를 이 기계에서 잰다.
    ///
    /// Claude 의 장은 프롬프트 앞에서 쉬는 동안 훅이 안 돌아 닻을 안 적는다. 그래서 다른 기계의 산 Claude 장도 20분이면
    /// 떠난 것으로 읽히는데, Claude 의 세션 이름은 컨테이너마다 따로 세어 겹친다. 떠난 것으로 읽힌 이름을 내주던 판은 다른
    /// 컨테이너에서 같은 이름을 받은 새 세션이 그 장을 덮고 그 함의 편지를 보낸 이에게 되돌렸다 — 아직 산 세션의 편지다.
    /// 이제 새 세션은 토막을 붙여 가른다. 하루가 지나면 걷기([`sweep`])가 그 장을 걷고 함을 비우는 때와 같다.
    ///
    /// **창이 대는 이름은 이것으로 안 잰다** — [`Presence::gone`] 이다. 그 이름은 우연히 겹친 것이 아니라 창이 "나는 이
    /// 이름이다" 라고 댄 것이라, 컨테이너를 다시 띄운 `MOAI_AGENT=w1` 창은 앞 컨테이너의 낡은 w1 장이 20분 뒤 떠난
    /// 것으로 읽히면 제 이름을 되찾는다(2026-10-05 사용자 결정, moai-dhxm). 그래서 다른 세션이 지은 이름을 창이 그대로
    /// 대면, 떠난 것으로 읽히는 그 장을 넘겨받아 안 읽은 편지를 되돌린다 — 대는 쪽이 고른 것이다.
    pub fn holds_made_name(&self) -> bool {
        if self.here() { !self.gone() } else { !self.expired(&crate::model::now()) }
    }

    /// 닻을 적는다(moai-j3n5). 장을 쓰는 자리(훅·`hello`·기다림)가 이 하나로 적는다. **모든 장에 적는다**(moai-dhxm) —
    /// 프로세스를 아는 장도 다른 기계에서는 이것으로만 잰다. 노는 장이 닫힐 때(`SessionEnd`)는 다시 안 쓴다 — 닻은 "아직
    /// 산다" 다([`crate::cmd::hook`] 의 `rest`).
    pub fn stamp(&mut self, now: &str) {
        self.seen = Some(now.to_string());
    }

    /// 닻을 다시 적을 때가 되었는가 — 닻이 [`SEEN_EVERY`] 넘게 묵었다.
    pub fn due(&self, now: &str) -> bool {
        let at = self.seen.as_deref().and_then(crate::model::parse_rfc3339);
        match (crate::model::parse_rfc3339(now), at) {
            (Some(now), Some(at)) => now - at >= SEEN_EVERY,
            _ => true,
        }
    }
}

/// 출석 한 장의 자리 — `<출석부>/<이름>.json`. 이름이 곧 파일 이름이다.
fn card_at(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// 출석을 적는다 — temp 를 쓰고 `<name>.json` 으로 `rename` 한다. **덮는 것이 뜻이다** — 같은 이름은 같은
/// 에이전트고, 마지막에 적은 쪽이 지금이다. 반쯤 쓴 장은 안 보이고, 쓰다 죽어 남은 temp 는 [`sweep`] 이 걷는다. 출석부가
/// 체크아웃 밖으로 풀리면 안 쓴다([`ensure_dir`]).
///
/// **`sync` 하지 않는다**(리뷰 moai-h8tn.x4l) — 훅이 프롬프트마다·턴 끝마다 이 장을 쓰는데, 저장소가 Ceph RBD 에
/// 서는 이 기계에서 `sync` 한 번이 수십 ms 다. 출석은 기록이 아니라 지금의 표라 잃어도 다음 훅이 다시 쓰고, 기계가
/// 죽은 뒤에는 장의 pid 가 모두 죽어 어차피 걷힌다. 다른 프로세스가 반쯤 쓴 장을 못 보는 것은 `rename` 이 지킨다.
pub fn write_presence(dir: &Path, presence: &Presence) -> std::io::Result<()> {
    if !is_agent_name(&presence.name) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, presence.name.clone()));
    }
    let dir = ensure_dir(dir, &home_of(dir))?;
    let mut text = serde_json::to_string(presence).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = temp_in(&dir);
    write_new(&tmp, text.as_bytes(), false)?;
    std::fs::rename(&tmp, card_at(&dir, &presence.name)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// 출석 한 장을 걷는다 — 이름을 바꾼 에이전트의 옛 장이나 죽은 장이다. 출석부가 체크아웃 밖으로 풀리면 안 걷는다
/// ([`reach`], moai-kxkw.7ky) — 밖에 심어 둔 죽은 pid 의 장을 `moai agents` 가 지우던 자리다.
pub fn forget(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(card_at(&reach(dir, &home_of(dir))?, name))
}

/// 두 이름의 장이 **한 파일**인가 — 대소문자를 안 가리는 파일 시스템(macOS 기본)에서는 `Worker` 와 `worker` 가 한
/// 파일이다. 이름을 바꾼 뒤 옛 장을 걷을 때 묻는다: 한 파일이면 걷는 것이 방금 쓴 장을 지운다.
pub fn same_card(dir: &Path, a: &str, b: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Ok(dir) = reach(dir, &home_of(dir)) else { return false };
        let id = |name: &str| std::fs::metadata(card_at(&dir, name)).ok().map(|m| (m.dev(), m.ino()));
        matches!((id(a), id(b)), (Some(x), Some(y)) if x == y)
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        a.eq_ignore_ascii_case(b)
    }
}

/// 출석부를 읽는다 — 이름 차례다. 산 것만 고르지 않는다: 그것은 [`alive`] 를 묻는 쪽이 한다.
///
/// 출석부가 체크아웃 밖으로 풀리면 안 읽고 그 자리 하나를 못 읽은 것으로 댄다([`reach`], moai-kxkw.7ky) — 밖에 심어 둔
/// 장이 이 저장소의 에이전트로 서면 감독이 그 이름에 일감을 보낸다.
pub fn presences(dir: &Path) -> (Vec<Presence>, Vec<Garbled>) {
    let mut out = Vec::new();
    let mut garbled = Vec::new();
    let dir = match reach(dir, &home_of(dir)) {
        Ok(dir) => dir,
        Err(f) => return (out, vec![f.garbled(None)]),
    };
    for (path, stem) in json_files(&dir) {
        if !is_agent_name(&stem) {
            continue;
        }
        match read_json::<Presence>(&path) {
            // **파일 이름이 이름이다** — 안에 적힌 이름이 다르면 파일 쪽을 믿는다. 쓰는 쪽이 늘 같게 쓰니,
            // 다른 것은 손으로 옮긴 파일이다.
            Ok(Some(p)) => out.push(Presence { name: stem, ..p }),
            Ok(None) => {}
            Err(why) => garbled.push(Garbled { path, why: Why::Bad(why), mailbox: None }),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    (out, garbled)
}

/// 출석부를 체크아웃 밖으로 풀려 못 연 것인가 — [`presences`] 가 낸 못 읽은 것 가운데 그 거절이다. 그때 빈 출석부는 "아무도
/// 없다" 가 아니라 **누가 있는지 모른다** 는 뜻이다 — 이름 겨루기·넘겨받기·역할을 그것으로 가리면 안 된다(리뷰
/// moai-kxkw.k2f: 감독의 역할이 비어 `any-idle-worker` 일감을 가졌고, 훅은 떠난 이의 것으로 친 되돌아온 편지를 읽음으로 치웠다).
pub fn roster_fenced(garbled: &[Garbled]) -> Option<&Garbled> {
    garbled.iter().find(|g| matches!(g.why, Why::Fenced(_)))
}

/// 걷은 장 하나 — 이름과 걷은 까닭(moai-dhxm). `moai agents` 가 사람에게 셋을 갈라 말한다.
///
/// - `dead` — 그 프로세스가 이 기계에서 죽어 걷었고 함을 비웠다(편지가 보낸 이에게 돌아갔다)
/// - `elsewhere` — 다른 기계의 장이 하루([`EXPIRE_AFTER`]) 넘게 안 적혀 걷었고, 함도 비웠다
/// - 둘 다 아니면 pid 를 모르는 장(Codex)이 하루 넘게 안 적혀 걷었고 함은 남았다
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swept {
    pub name: String,
    pub dead: bool,
    pub elsewhere: bool,
}

/// 떠난 출석을 걷는다 — 걷은 장을 낸다([`Swept`]). **산지 모르는 것은 안 걷는다**([`alive`] 가 `None`, 닻이 아직 산 장).
///
/// **프로세스가 죽은 장은 그 함도 비운다**([`retire`]) — 안 읽은 편지가 보낸 이에게 되돌아간다. **프로세스로 못 재는 장
/// (pid 를 모르거나 다른 기계의 것)은 하루([`EXPIRE_AFTER`]) 넘게 안 적혔을 때만 걷는다**(moai-dhxm) — 그 세션이 살아
/// 돌아올 수 있고, 다른 기계의 장은 그 기계에서 아직 살 수 있다. 20분 넘게 조용한 장은 떠난 것으로 읽을 뿐 안 걷는다
/// ([`STALE_AFTER`], 2026-10-05 사용자 결정) — 역할과 이름이 남아야 돌아온 세션이 그대로 선다.
///
/// **걷은 장의 함은 pid 를 모르는 장(Codex)만 남긴다**(같은 결정) — 그 이름은 세션 id 에서 지어 돌아온 그 세션이 같은
/// 이름을 다시 받는다. **다른 기계의 장은 함을 비운다**(2026-10-05 사용자 결정, moai-dhxm) — 컨테이너를 다시 띄우면 앞
/// 컨테이너의 장이 모두 다른 기계의 것으로 읽혀 이 길을 타는데, Claude 의 세션 이름은 디렉터리마다 256개 남짓이라
/// 남겨 둔 편지를 나중에 같은 이름을 받은 무관한 새 세션이 받았다(리뷰 moai-dhxm.3sc). 하루 넘게 쉰 세션이 돌아오면
/// 그 편지는 보낸 이에게 되돌아간 표를 달고 있다.
///
/// **걷기 바로 앞에 장을 다시 읽는다**(리뷰 moai-ew4o.q9f) — 출석부를 한 번 읽고 이름으로 걷던 판은, 그 사이(앞선 장의
/// 편지를 되돌리는 동안 길어진다) 같은 이름을 넘겨받은 새 세션의 장을 지우고 그 세션 앞으로 온 편지까지 보낸 이에게
/// 되돌렸다. 장은 그 세션의 다음 훅이 다시 쓰지만 되돌린 편지는 안 돌아온다. 다시 읽은 것과 지우는 것 사이의 틈은
/// 남는다 — 시스템 호출 몇 개 너비고, 그것을 막으려고 락을 두지 않는다.
///
/// **쓰다 죽은 출석의 temp 도 여기서 걷는다**(moai-kxkw.68i) — [`write_presence`] 가 temp 를 쓰고 `rename` 하기 전에 죽으면
/// (훅이 I/O 정체 중에 끊기면) 점 파일이 남는데, temp 를 걷는 자리가 보내기([`send`])의 함뿐이라 출석부의 것은 아무도
/// 안 걷었다. 훅이 아니라 여기서 걷는 까닭은 죽은 장과 같다 — 도구 호출마다 도는 자리는 남의 파일을 지우지 않는다(훅은
/// 출석부를 읽기는 한다 — 지우는 것은 `moai agents` 하나다).
///
/// 출석부가 체크아웃 밖으로 풀리면 아무것도 안 걷는다([`reach`]).
pub fn sweep(dir: &Path, mail: &Path) -> Vec<Swept> {
    let Ok(real) = reach(dir, &home_of(dir)) else { return Vec::new() };
    sweep_temps(&real);
    sweep_from(dir, mail, presences(dir).0, &crate::model::now())
}

/// [`sweep`] 의 몸통 — 읽어 둔 출석부와 지금을 받는다. 시험이 읽은 뒤에 장을 고쳐 보려고 가른다.
fn sweep_from(dir: &Path, mail: &Path, all: Vec<Presence>, now: &str) -> Vec<Swept> {
    let mut swept = Vec::new();
    for p in all {
        let dead = p.dead();
        let expired = !dead && p.expired(now);
        if !(dead || expired) || !unchanged(dir, &p) || forget(dir, &p.name).is_err() {
            continue;
        }
        // pid 를 알지만 프로세스로 못 재는 장 — 다른 기계의 장이다([`Presence::here`]).
        let elsewhere = expired && p.pid != 0;
        if dead || elsewhere {
            retire(mail, &p.name);
        }
        swept.push(Swept { name: p.name, dead, elsewhere });
    }
    swept
}

/// 그 장이 읽은 뒤로 그대로인가 — 디스크의 장을 다시 읽어 견준다([`sweep`]). 이름은 파일 이름으로 맞춘다([`presences`]).
/// 없거나 못 읽으면 그대로가 아니다 — 걷지 않는다.
fn unchanged(dir: &Path, p: &Presence) -> bool {
    let Ok(dir) = reach(dir, &home_of(dir)) else { return false };
    match read_json::<Presence>(&card_at(&dir, &p.name)) {
        Ok(Some(now)) => Presence { name: p.name.clone(), ..now } == *p,
        _ => false,
    }
}

/// 장을 새 이름으로 옮긴다 — 새 장을 쓰고, 옛 장이 다른 파일이면 걷고, 안 읽은 편지를 새 이름으로 옮긴다([`carry`]).
/// **새 장을 쓴 뒤에, 그것이 다른 파일일 때만** 걷는다(리뷰 moai-h8tn.x4l) — 먼저 걷으면 쓰기가 진 판에 장이 하나도 안
/// 남고, 대소문자만 바꾼 이름은 대소문자를 안 가리는 파일 시스템에서 방금 쓴 그 파일이다(그때는 함도 한 디렉터리다).
pub fn rename_card(dir: &Path, mail: &Path, presence: &Presence, old: &str) -> std::io::Result<()> {
    write_presence(dir, presence)?;
    if presence.name != old && !same_card(dir, old, &presence.name) {
        let _ = forget(dir, old);
        carry(mail, old, &presence.name);
    }
    Ok(())
}

/// 떠난 장의 이름을 넘겨받는다 — 그 이름(대소문자를 안 가린다)의 떠난 장마다 그 함을 비운다([`retire`]). 떠난 세션
/// 앞으로 남은 편지가 새 세션에 실리지 않게(moai-ew4o.l3n).
///
/// **이어 쓰는 장(`keep`)은 넘겨받는 것이 아니다**(리뷰 moai-ew4o.q9f) — 제 장이 떠난 것으로 읽혀도(닻이 20분 넘게 묵은
/// Codex 장, `hello --as` 로 이은 떠난 장) 그 함은 제 편지다. 안 빼던 판은 같은 세션이 다시 인사하는 것만으로 제 편지를
/// 보낸 이에게 "읽기 전에 떠났다" 로 되돌렸다.
///
/// **새로 쥐는 이름의 함에 기다리던 되돌아온 편지는 앞사람의 것이다**(2026-10-05 사용자 결정, 리뷰 moai-ew4o.q9f 10번) —
/// 둘 다 떠난 뒤 걷기가 보낸 이의 이름 앞으로 되돌린 편지는 아무 장도 안 쥔 함에 남는다. 그 이름을 받은 새 세션에
/// "제가 보낸 편지가 되돌아왔다" 로 실리지 않게, 읽음으로 치운다. 이어 쓰는 장이 그 이름을 이미 쥐었으면 제 편지라 둔다.
pub fn take_over(mail: &Path, all: &[Presence], name: &str, keep: Option<&str>) {
    for old in all.iter().filter(|p| p.name.eq_ignore_ascii_case(name) && keep != Some(p.name.as_str()) && p.gone()) {
        retire(mail, &old.name);
    }
    if keep.is_some_and(|k| k.eq_ignore_ascii_case(name)) {
        return;
    }
    let Ok(at) = reach(&mailbox(mail, name), &home_of(mail)) else { return };
    let (letters, _) = unread_in(&at, name, None);
    for s in letters.iter().filter(|s| s.returned) {
        let _ = take(mail, s, name);
    }
}

/// 이 장의 닻을 다시 적는다 — 때가 되었을 때만([`Presence::due`]). 디스크의 장을 다시 읽어
/// 적는다 — 앞에서 읽은 장으로 덮으면 그 사이 훅이 고친 칸을 되돌린다. 못 적으면 조용히 지나간다.
///
/// 다른 기계의 장은 안 적는다 — 그 장의 닻은 그 기계의 몫이다.
///
/// **쓰기 바로 앞에 한 번 더 읽어 견준다**(moai-dhxm) — 도구 부름의 훅이 모든 말씨에서 이것을 부르게 되자, 부모 세션의
/// 장을 함께 쓰는 서브에이전트의 도구 부름이 그 사이 `Stop` 이 적은 상태나 `MOAI_AGENT` 로 옮긴 이름을 앞서 읽은 장으로
/// 되돌릴 수 있었다(옮기기 전 이름의 장이 되살아난다). 그 사이 바뀌었으면 안 쓴다 — 바꾼 쪽이 닻도 적었다. 다시 읽은 것과
/// 쓰는 것 사이의 틈은 [`sweep`] 과 같이 남긴다. 기계를 안 적은 옛 장은 여기서 기계를 단다([`Presence::claimed`]).
pub fn keep_alive(dir: &Path, which: impl Fn(&Presence) -> bool) {
    let (all, _) = presences(dir);
    let Some(read) = all.into_iter().find(|p| which(p)) else { return };
    let now = crate::model::now();
    // **다른 기계의 장에는 닻을 안 적는다**(2026-10-05 사용자 결정, moai-dhxm) — 닻은 "그 프로세스가 산다" 인데, 여기서
    // 도는 것은 그 프로세스가 아니다. 적던 판은 컨테이너를 다시 띄운 뒤 `MOAI_AGENT=w1` 창의 `moai inbox` 가 앞 컨테이너의
    // 낡은 w1 장을 1분마다 살려, 그 장이 영영 안 낡고 창은 그 이름을 못 되찾았다. 다른 기계에서 `--as` 로 읽어도 그 장을
    // 살려 두지 않는다.
    if !read.here() || !read.due(&now) {
        return;
    }
    let mut p = read.clone().claimed();
    p.stamp(&now);
    if unchanged(dir, &read) {
        let _ = write_presence(dir, &p);
    }
}

/// Codex 세션의 장 이름 — `codex-<세션 id 앞 8자>` 를 [`made_name`] 으로 가른다. 훅([`crate::cmd::hook`] 의 `attendee`)과
/// `hello` 가 이 하나로 짓는다 — 따로 짓던 판은 잇는 토막의 차례가 갈려, 훅이 아직 장을 안 지은 창의 `hello` 가 훅과 다른
/// 이름으로 섰다(리뷰 moai-ew4o.q9f).
pub fn codex_name(all: &[Presence], session: &str) -> Option<String> {
    let short: String = session.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    made_name(all, name_with("codex", &short)?, session)
}

/// 지은 이름을 가른다 — `base` 를 남이 쥐었으면 세션 id 앞 8자를, 그래도 쥐었으면 세션 id 를 통째로 잇는다. **쥐었는가는
/// [`Presence::holds_made_name`] 으로 잰다**(moai-nas5) — 다른 기계의 조용한 장도 하루 동안은 그 이름을 쥔다. 훅이 짓는
/// 이름(Claude 의 세션 이름, `<벤더>-<세션 토막>`)과 [`codex_name`] 이 이 하나로 가른다 — 두 자리에 따로 적던 판은 같은 자를
/// 두 번 고쳐야 했다.
///
/// **가른 이름도 다시 본다**(리뷰 moai-u5wr.e74) — Codex 의 세션 id 는 UUIDv7 이라 앞 8자가 밀리초 시각의 윗자리고 65초
/// 남짓마다만 바뀐다. 그 사이에 연 세션 셋은 토막까지 같아, 한 번만 가르던 판은 셋째가 둘째의 장을 덮었다. 토막을 이어도
/// 남이 쥐었으면 세션 id 를 통째로 잇는다 — 세션마다 하나다. 토막은 상한에 안 잘리게 잇는다([`name_with`]).
///
/// **셋 다 쥐였어도 이름을 낸다**(리뷰 moai-nas5.cn7 15번, moai-keka.q2w) — 앞 8자 토막 뒤에 `-2`, `-3`… 을 이어 빈
/// 이름을 찾는다. `None` 을 내던 판은 훅이 그 세션에 장을 안 세워, 그 세션의 `moai inbox`·`send` 가 누구인지 몰라 섰다.
/// 다른 기계의 조용한 장이 하루 쥐게 된 뒤로(moai-nas5) 그 판이 넓어졌다. 이어 보는 수는 출석부의 장 수보다 하나 많다 —
/// 끝의 수가 다르면 이름이 다르니, 그 가운데 하나는 반드시 빈다. 이름이 못 되는 `base` 만 `None` 이다.
pub fn made_name(all: &[Presence], base: String, session: &str) -> Option<String> {
    let held = |name: &str| all.iter().any(|p| p.name.eq_ignore_ascii_case(name) && p.holds_made_name());
    if !held(&base) {
        return Some(base);
    }
    let alnum = |n: usize| session.chars().filter(char::is_ascii_alphanumeric).take(n).collect::<String>();
    let (short, whole) = (alnum(8), alnum(usize::MAX));
    let counted = (2..=all.len() + 2).map(|n| format!("{short}-{n}"));
    [short.clone(), whole].into_iter().chain(counted).filter_map(|tail| name_with(&base, &tail)).find(|n| !held(n))
}

/// 이 프로세스의 조상 가운데 출석부에 선 에이전트 — **"나는 누구인가" 의 답이다.** 에이전트가 띄운 셸에서
/// 도는 `moai` 의 조상에 그 에이전트가 있다. 선 때까지 맞아야 한다(둘 다 알 때) — pid 는 재사용된다.
pub fn me_among<'a>(presences: &'a [Presence], ancestors: &[Proc]) -> Option<&'a Presence> {
    ancestors.iter().find_map(|a| presences.iter().find(|p| p.runs_as(a)))
}

/// 출석 하나를 깨울 수 있는 일꾼 가운데 고른다 — `any-idle-worker` 편지를 보낼 때. 산 것, 놀고 있는 것,
/// 그 편지를 가질 수 있는 것([`for_me`] 와 같은 자) 가운데 가장 오래 논 것이다.
///
/// **이 기계의 일꾼이 먼저다**(moai-dhxm) — 다른 기계의 장은 못 깨운다([`wake`] 의 `no_way`). 가장 오래 논 일꾼이 다른
/// 기계에 있으면 그를 골라 아무도 안 두드리던 판은, 이 기계에서 노는 일꾼을 두고 열린 편지를 세워 두었다. 이 기계에 노는
/// 일꾼이 없을 때만 다른 기계의 일꾼을 낸다 — 못 깨워도 누가 노는지는 댄다.
pub fn idle_worker<'a>(presences: &'a [Presence], from: &str) -> Option<&'a Presence> {
    presences
        .iter()
        .filter(|p| p.status == IDLE && may_take_open(&p.name, &p.role, from))
        .filter(|p| !p.gone())
        .min_by(|a, b| b.here().cmp(&a.here()).then(a.since.cmp(&b.since)).then(a.name.cmp(&b.name)))
}

// ── 프로세스 ──────────────────────────────────────────────────────────

/// 이 프로세스 아래에서 `moai inbox --wait` 가 도는가 — 그 에이전트가 턴 안에서 편지를 기다리는 중인가.
///
/// 리눅스만 잰다 — `/proc` 의 `stat`(부모)과 `cmdline`(인자)을 읽어 그 pid 의 자손을 훑는다. 못 재는 판(다른
/// 유닉스·`/proc` 을 못 읽음)은 `false` 다: 모르는 것을 "기다린다" 로 읽으면 깨워야 할 에이전트를 안 깨운다. 깨우기는
/// 덤이고, 그쪽으로 틀리는 값은 한 번 덜 두드리는 것보다 크다.
pub fn waits(pid: u32) -> bool {
    if pid <= 1 || !cfg!(target_os = "linux") {
        return false;
    }
    let Ok(entries) = std::fs::read_dir("/proc") else { return false };
    let mut kids: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for e in entries.filter_map(Result::ok) {
        let Some(n) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        if let Some(p) =
            std::fs::read(format!("/proc/{n}/stat")).ok().and_then(|b| parse_stat(n, &String::from_utf8_lossy(&b)))
        {
            kids.entry(p.ppid).or_default().push(n);
        }
    }
    let mut todo = vec![pid];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(at) = todo.pop() {
        if !seen.insert(at) {
            continue;
        }
        for &kid in kids.get(&at).into_iter().flatten() {
            let args = std::fs::read(format!("/proc/{kid}/cmdline")).unwrap_or_default();
            let args: Vec<&[u8]> = args.split(|b| *b == 0).collect();
            if args.contains(&b"inbox".as_slice()) && args.iter().any(|a| a.starts_with(b"--wait")) {
                return true;
            }
            todo.push(kid);
        }
    }
    false
}

/// 프로세스 하나 — pid·부모·선 때·이름.
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// 선 때 — 리눅스만 안다. pid 재사용을 가른다.
    pub start: Option<u64>,
    pub comm: String,
}

/// 프로세스를 읽는다. 리눅스는 `/proc/<pid>/stat` 하나를, 다른 유닉스는 `ps` 를 띄운다. **이름이 UTF-8 이
/// 아니어도 읽는다**(리뷰 moai-h8tn.x4l) — 커널은 이름을 15바이트에서 자르므로 한글 이름이 글자 가운데서 잘린다.
/// 글로 읽던 판은 그 프로세스를 "없다" 로 읽어 산 에이전트의 장을 걷었다.
pub fn proc_of(pid: u32) -> Option<Proc> {
    if pid == 0 {
        return None;
    }
    if cfg!(target_os = "linux") {
        let stat = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
        return parse_stat(pid, &String::from_utf8_lossy(&stat));
    }
    let out = std::process::Command::new("ps")
        .args(["-o", "ppid=", "-o", "comm=", "-p", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let line = text.lines().next()?.trim();
    let (ppid, comm) = line.split_once(char::is_whitespace)?;
    let comm = comm.trim();
    let comm = comm.rsplit('/').next().unwrap_or(comm);
    Some(Proc { pid, ppid: ppid.trim().parse().ok()?, start: None, comm: comm.to_string() })
}

/// `/proc/<pid>/stat` 을 가른다. **이름은 괄호 안이고 괄호와 빈칸을 품을 수 있다** — 마지막 `)` 에서
/// 가른다. 그 뒤가 3째 칸(상태)부터라 부모는 4째, 선 때는 22째다.
fn parse_stat(pid: u32, stat: &str) -> Option<Proc> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = stat.get(open + 1..close)?.to_string();
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    Some(Proc { pid, ppid: rest.get(1)?.parse().ok()?, start: rest.get(19).and_then(|s| s.parse().ok()), comm })
}

/// 이 프로세스의 조상 — 부모부터 위로. 32단에서 멈춘다(고리를 못 막는 `ps` 를 위한 천장이다).
pub fn ancestors() -> Vec<Proc> {
    let mut out = Vec::new();
    let mut pid = parent_pid();
    while pid > 1 && out.len() < 32 {
        let Some(p) = proc_of(pid) else { break };
        pid = p.ppid;
        out.push(p);
    }
    out
}

fn parent_pid() -> u32 {
    #[cfg(unix)]
    {
        std::os::unix::process::parent_id()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// 프로세스 이름이 어느 벤더의 에이전트인가 — 앞머리로 본다(`codex-x86_64-…` 같은 이름이 있다).
pub fn vendor_of(comm: &str) -> Option<&'static str> {
    let c = comm.to_ascii_lowercase();
    if c.starts_with("claude") {
        Some("claude")
    } else if c.starts_with("codex") {
        Some("codex")
    } else if c.starts_with("agy") || c.starts_with("antigravity") {
        Some("antigravity")
    } else {
        None
    }
}

/// 조상 가운데 처음으로 이름이 벤더인 것 — 이 `moai` 를 띄운 에이전트다.
pub fn agent_among(ancestors: &[Proc]) -> Option<(&Proc, &'static str)> {
    ancestors.iter().find_map(|p| vendor_of(&p.comm).map(|v| (p, v)))
}

/// Codex 가 제 도구 셸에 세우는 그 세션의 id — `CODEX_THREAD_ID`, 없으면 `CODEX_SESSION_ID`(2026-10-05 사람의 codex 0.160
/// 창에서 쟀다, moai-u5wr.7xr). 훅 stdin 의 `session_id` 와 같은 값이라 그 세션의 장을 이것으로 찾는다. Codex 의 셸은 세션
/// 모두가 함께 쓰는 데몬 밑에서 돌아 조상으로는 세션을 못 가른다(moai-sile).
///
/// **조상이 codex 인 부름에서만 읽는다** — 부르는 쪽이 가린다. Codex 셸에서 띄운 다른 프로그램도 이 값을 물려받는다.
pub fn codex_session() -> Option<String> {
    ["CODEX_THREAD_ID", "CODEX_SESSION_ID"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

/// 이 프로세스가 선 기계 — pid 가 이 프로세스들을 가리키는 자리다(moai-dhxm). 같은 저장소를 여러 컨테이너가 쓰면 장의
/// pid 는 그것을 적은 컨테이너에서만 뜻을 갖는다. 장에 이 값을 적고, 이 값이 다른 장은 pid 로 안 잰다([`Presence::here`]).
///
/// - **리눅스**는 `boot_id` 와 pid 이름공간의 번호를 잇는다(`<boot_id>/<번호>`). 한 커널 위의 컨테이너는 `boot_id` 가
///   같고 이름공간이 갈린다. 이름공간 번호는 커널마다 따로 세어(처음 것은 어느 기계나 `4026531836` 이다) `boot_id` 가
///   다른 기계를 가른다. 컨테이너를 다시 띄우거나 기계가 다시 서면 값이 바뀌어 앞의 장은 남의 것으로 읽힌다 — 걷기는
///   닻이 하루 묵을 때로 미뤄지지만, 산 장을 죽은 것으로 걷는 쪽보다 싸다(모르면 아니다). **못 가르는 판이 하나 있다** —
///   한 메모리 스냅숏에서 되살린 VM 들은 `boot_id` 를 함께 들고 처음 이름공간의 번호도 같아 한 기계로 읽힌다. 그 판은 이
///   필드 전처럼 pid 로 잰다
/// - **다른 유닉스는 `None` 이다** — 거기서 pid 가 뜻을 갖는 자리는 기계의 부팅 하나인데, 호스트 이름은 그것을 못 댄다.
///   macOS 는 네트워크·VPN 을 옮기면 이름을 바꿔(DHCP·Bonjour) 그 순간 이 기계의 산 장이 모두 남의 것으로 읽히고(`moai
///   inbox` 가 나를 못 찾는다), 기계끼리 같은 이름도 흔하다 — [`Presence::host`] 를 안 견주는 까닭과 같다. 부팅 하나를 대는
///   값(macOS 의 `kern.bootsessionuuid`)은 이 저장소의 CI 가 짓지 않는 판에서만 읽을 수 있어 아직 들이지 않았다. 그 장은
///   이 필드 전의 판처럼 pid 로 잰다
/// - 못 읽으면 `None` 이다 — 장에 안 적고, 그 장은 이 필드 전의 판처럼 pid 로 잰다
///
/// 한 번만 읽는다 — 프로세스가 사는 동안 바뀌지 않는다.
pub fn machine() -> Option<String> {
    static HERE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    HERE.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        let boot = boot.trim();
        let ns = std::fs::read_link("/proc/self/ns/pid").ok()?;
        let ns = ns.to_str()?.strip_prefix("pid:[")?.strip_suffix(']')?;
        let word = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        (word(boot) && word(ns)).then(|| format!("{boot}/{ns}"))
    })
    .clone()
}

/// 이 기계의 호스트 이름 — 사람 화면에 "어디의 장인가" 를 대는 이름이다([`Presence::host`]). 못 읽으면 `None`.
pub fn hostname() -> Option<String> {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        // SAFETY: 버퍼와 그 길이를 함께 넘긴다. 잘린 이름은 NUL 이 없을 수 있어 아래에서 첫 NUL 이나 끝까지만 읽는다.
        if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
            return None;
        }
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        let name = String::from_utf8_lossy(&buf[..end]).trim().to_string();
        (!name.is_empty()).then_some(name)
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// 그 프로세스가 아직 사는가 — **모르면 `None`** 이다(걷지 않는다).
///
/// 리눅스는 `/proc` 로 재고, 선 때가 다르면 죽은 것이다(pid 재사용). 다른 유닉스는 `kill(pid, 0)` 이고
/// 선 때를 모른다 — 거기서는 재사용된 pid 가 산 것으로 읽힌다. 그 밖은 모른다.
///
/// **없다고 답한 것만 죽은 것이다**(리뷰 moai-h8tn.x4l) — `/proc/<pid>/stat` 을 못 읽은 까닭이 `NotFound` 가 아니면
/// (권한, `hidepid`) 모른다. 그것까지 죽은 것으로 세던 판은 `moai agents` 가 산 남의 장을 걷었다.
pub fn alive(pid: u32, start: Option<u64>) -> Option<bool> {
    if pid == 0 {
        return None;
    }
    if cfg!(target_os = "linux") {
        if !Path::new("/proc/self/stat").exists() {
            return None;
        }
        return match std::fs::read(format!("/proc/{pid}/stat")) {
            Ok(stat) => {
                let p = parse_stat(pid, &String::from_utf8_lossy(&stat))?;
                Some(start.is_none() || p.start.is_none() || p.start == start)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(_) => None,
        };
    }
    #[cfg(unix)]
    {
        let Ok(pid) = i32::try_from(pid) else { return None };
        // SAFETY: 시그널 0 은 아무것도 안 보내고 그 pid 가 있는지만 묻는다.
        if unsafe { libc::kill(pid, 0) } == 0 {
            return Some(true);
        }
        return match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => Some(false),
            Some(libc::EPERM) => Some(true),
            _ => None,
        };
    }
    #[allow(unreachable_code)]
    None
}

/// Claude Code 가 그 세션에 붙인 이름을 출석의 이름으로 접은 것 — [`claude_session_title`] 을 [`name_from`] 으로 접는다.
/// 출석의 이름을 그것과 맞추면 감독이 같은 이름으로 깨운다(2026-10-04 사용자 결정). 못 읽거나 이름으로 못 접으면 `None`
/// 이고 부르는 쪽이 세션 id 로 짓는다.
pub fn claude_session_name(pid: u32) -> Option<String> {
    name_from(&claude_session_title(pid)?)
}

/// Claude Code 가 그 세션에 붙인 이름 그대로 — `~/.claude/sessions/<pid>.json` 의 `name` 이다. `ListAgents` 와
/// `SendMessage` 가 쓰는 그 이름이다. **비문서 파일이다** — 못 읽으면 `None` 이다.
///
/// **절대 경로만 본다**(리뷰 moai-h8tn.x4l) — 빈 `HOME` 은 빈 경로라 `.claude/sessions/…` 가 훅이 옮겨 간 자리, 곧
/// 세션의 저장소에서 풀린다. 그 저장소가 심어 둔 파일 하나가 에이전트의 이름을 고르게 된다.
pub fn claude_session_title(pid: u32) -> Option<String> {
    let set = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home =
        set("CLAUDE_CONFIG_DIR").or_else(|| set("HOME").map(|h| h.join(".claude"))).filter(|d| d.is_absolute())?;
    let v: serde_json::Value = read_json(&home.join("sessions").join(format!("{pid}.json"))).ok()??;
    Some(v.get("name")?.as_str()?.to_string())
}

/// `SendMessage` 로 그 세션에 닿는 이름 — Claude Code 가 붙인 이름 그대로다(2026-10-06 사용자 결정, moai-keka.id8).
/// **장에 적지 않고 깨울 때 읽는다** — 깨우기는 이 기계의 장에만 돌아([`wake`]) 그 pid 의 파일이 여기 있고, 사람이 세션
/// 이름을 바꿔도 지금 이름이 나온다. 장의 이름과 같으면 `None` 이다 — 댈 것이 없다.
///
/// 출석의 이름은 그 이름과 갈릴 수 있다 — 남이 쥐면 토막이 붙고(`moa-issue-3-1a2b3c4d`), 이름에 못 쓰는 글자는 접힌다.
/// 그 이름을 대던 판은 보낸 쪽이 Claude 가 모르는 이름으로 `SendMessage` 를 했다. 한 줄이 아니거나 길면 안 댄다 — 남의
/// 파일의 글을 보낸 쪽 화면에 그대로 싣는다.
fn send_message_name(p: &Presence) -> Option<String> {
    if p.pid == 0 {
        return None;
    }
    let raw = claude_session_title(p.pid)?;
    let raw = raw.trim();
    let fits = !raw.is_empty() && raw.chars().count() <= NAME_MAX && !raw.chars().any(char::is_control);
    (fits && raw != p.name).then(|| raw.to_string())
}

// ── 깨우기 ────────────────────────────────────────────────────────────

/// 깨운 결과 — `send --wake` 가 낸다. `via` 는 쓴 길, `done` 은 그 길을 실제로 탔는지, `why` 는 못 탄 까닭의
/// 낱말(기계가 가른다)이다. 사람 말은 부르는 쪽이 이 값으로 짓는다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Woke {
    pub to: String,
    pub via: &'static str,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<&'static str>,
    /// 일하는 중(`why: busy`)이면 언제부터인가 — 장의 `since` 다. **그 턴이 정말 도는지는 모른다**(moai-u5wr.f29):
    /// Claude 와 Antigravity 는 사람이 Esc 로 끊은 턴에 훅을 하나도 안 내, 그 장은 다음 프롬프트까지 `busy` 로 남는다.
    /// 그래서 "턴이 끝나면 싣는다" 를 약속하지 않고 이 값을 댄다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// 보낸 쪽이 `SendMessage` 에 댈 이름(`why: ask_sender`) — Claude Code 가 그 세션을 부르는 이름이 `to` 와 다를 때만
    /// 선다(moai-keka.id8, [`send_message_name`]). 없으면 `to` 다 — 같거나, 이 기계에서 Claude 의 장부를 못 읽었다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_message_to: Option<String>,
}

/// 깨운다 — **덤이다**(2026-10-04 사용자 결정 "깨우기를 덤으로 낮춘다"). 편지를 받는 기본 길은 일꾼의 기다림
/// (`moai inbox --ack --wait`)과 훅이고, 깨우기는 그 둘 밖에서 노는 에이전트를 한 번 두드리는 것뿐이다.
///
/// - **Claude** 는 이 CLI 가 못 깨운다 — 보낸 쪽이 `SendMessage` 로 깨우라고 댄다(`ask_sender`). Claude 가 그 세션을
///   부르는 이름이 장의 이름과 다르면 그 이름을 함께 댄다(`send_message_to`, moai-keka.id8)
/// - **tmux 칸이 적힌 에이전트**는 그 칸에 [`WAKE_WORDS`] 를 친다 — 벤더를 안 가린다
/// - **둘 다 아니면 아무것도 안 한다**(`no_way`) — tmux 를 안 쓰는 사람도 있다. 사람 화면은 이때 입을 다문다
///
/// **moai 는 에이전트의 실행 파일을 안 띄운다**(같은 결정) — `codex queue --thread` 로 깨우던 길을 걷었다. 띄우는 것은
/// `tmux` 하나다. **일하는 중이면 안 깨운다** — 그 턴의 끝(훅이 걸린 에이전트의 `Stop`)이나 다음 프롬프트, 다음
/// `moai inbox` 가 편지를 싣는다. 끊긴 턴은 끝이 안 오니 언제부터 일하는 중인지(`since`)를 함께 낸다. Claude 는 tmux 칸이
/// 적혀 있어도 그 칸에 안 친다 — 사람이 앉아 있을 법한 창에 글자를 끼우지 않고, 결정이 댄 길(SendMessage)로 간다.
///
/// 띄우는 프로세스는 입출력을 모두 닫는다 — 물려주면 훅처럼 stdout 을 받아 두는 쪽이 EOF 를 못 받는다
/// (`skill::command` 의 "그 성질에 기대고 있다"). 10초 안에 안 끝나면 죽인다.
pub fn wake(p: &Presence) -> Woke {
    let woke = |via, done, why| Woke { to: p.name.clone(), via, done, why, since: None, send_message_to: None };
    if p.status == BUSY {
        return Woke { since: Some(p.since.clone()).filter(|s| !s.is_empty()), ..woke("none", false, Some("busy")) };
    }
    // **다른 기계의 에이전트는 못 깨운다**(moai-dhxm) — 그 tmux 서버도 `SendMessage` 의 소켓도 그 기계에 있다. 소켓 자리가
    // 이 기계에도 있으면(`/tmp/tmux-1000/default` 는 어디나 같다) 이 기계의 서버에서 같은 칸 id 를 가진 남의 칸에 글자가
    // 쳐진다.
    if !p.here() {
        return woke("none", false, Some("no_way"));
    }
    if p.vendor == "claude" {
        return Woke { send_message_to: send_message_name(p), ..woke("send_message", false, Some("ask_sender")) };
    }
    // **기다리는 에이전트는 안 두드린다**(리뷰 moai-snyk.nic 9번). `moai inbox --wait` 는 기다리는 동안 장을 `idle` 로
    // 적는다 — 감독이 일꾼을 찾는 표다. 그런데 그 기다림은 턴 **안의** 셸 명령이라, 그 칸에 `moai inbox`+Enter 를 치면
    // 도는 턴에 글자가 끼어든다. 기다림이 편지를 스스로 가지니 두드릴 까닭도 없다.
    if waits(p.pid) {
        return woke("none", false, Some("waiting"));
    }
    // **소켓을 모르는 칸은 안 친다**(리뷰 moai-h8tn.x4l) — `-S` 없는 `tmux` 는 보내는 쪽의 `$TMUX`(사람의 서버)나
    // 기본 서버에 붙고, 칸 id(`%N`)는 서버마다 따로 세어 그 서버에서는 남의 칸이다. 사람의 창에 `moai inbox` 와
    // Enter 가 쳐진다.
    let (Some(pane), Some(socket)) =
        (p.tmux_pane.as_deref().filter(|s| !s.is_empty()), p.tmux_socket.as_deref().filter(|s| !s.is_empty()))
    else {
        return woke("none", false, Some("no_way"));
    };
    let tmux = || {
        let mut cmd = std::process::Command::new("tmux");
        cmd.args(["-S", socket]);
        cmd
    };
    let ran = run(tmux().args(["send-keys", "-t", pane, "-l", WAKE_WORDS]))
        .and_then(|()| run(tmux().args(["send-keys", "-t", pane, "Enter"])));
    woke("tmux", ran.is_ok(), ran.err())
}

/// 명령 하나를 입출력 없이 10초 안에 돌린다. 못 띄우면 `missing`, 비영이면 `failed`, 넘으면 `timeout` 이다.
/// 기다림은 [`crate::git::waited`] 하나로 한다 — 몇 ms 에 끝나는 `tmux send-keys` 를 고정 낮잠으로 늦추지 않는다.
fn run(cmd: &mut std::process::Command) -> Result<(), &'static str> {
    use std::process::Stdio;
    let mut child =
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_| "missing")?;
    match crate::git::waited(&mut child, std::time::Duration::from_secs(10)) {
        Some(status) if status.success() => Ok(()),
        Some(_) => Err("failed"),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err("timeout")
        }
    }
}

// ── 체크아웃 안에서만 ──────────────────────────────────────────────────

/// 디렉터리가 체크아웃 밖으로(또는 `.git/` 안으로) 풀려 **안 열고 안 쓴** 까닭 — 그 디렉터리와 [`crate::held::Unheld`]
/// (moai-kxkw.7ky).
///
/// io 실패에 실어 나른다(`io::Error::other`) — 쓰는 길은 모두 `io::Result` 라 `?` 하나로 멈춘다. 사람에게 대는 쪽은
/// [`refusal`] 이 꺼내 고른 말로 펴고, 읽는 길은 [`Garbled`] 로 댄다. 말을 못 고르는 자리(훅)는 아무것도 안 댄다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fenced {
    pub at: PathBuf,
    pub why: crate::held::Unheld,
}

/// 말 없는 꼴 — `<자리> -> <풀린 자리>`([`crate::held::spelled`]). 고른 말은 [`refusal`] 이 편다.
impl std::fmt::Display for Fenced {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::held::spelled(&self.at, &self.why))
    }
}

impl std::error::Error for Fenced {}

impl From<Fenced> for std::io::Error {
    fn from(f: Fenced) -> std::io::Error {
        std::io::Error::other(f)
    }
}

impl Fenced {
    /// 못 연 디렉터리 하나 — 읽는 길([`list`]·[`presences`])은 그 자리를 빈 것으로 넘기지 않고 댄다.
    fn garbled(self, mailbox: Option<&str>) -> Garbled {
        Garbled { path: self.at, why: Why::Fenced(self.why), mailbox: mailbox.map(str::to_string) }
    }

    /// io 실패에 실려 온 이것 — 못 꺼내면(다른 io 실패) `None` 이다.
    pub fn of(e: &std::io::Error) -> Option<&Fenced> {
        e.get_ref().and_then(|inner| inner.downcast_ref::<Fenced>())
    }
}

/// 쓰다 진 것을 멈추는 말 — `<자리>: <까닭>`. 체크아웃 밖으로 풀린 디렉터리면 그 자리와 고른 말에 **코드는 `broken`**
/// 이다([`crate::held::refused`] — 스냅샷·설정·락의 거절과 같은 꼴, 같은 코드: 고칠 것은 그 링크다). 아니면 `dir` 과
/// 운영체제의 말이다. `send`·`hello`·`inbox --ack` 가 이것으로 댄다.
pub fn refusal(lang: crate::i18n::Lang, dir: &Path, e: &std::io::Error) -> crate::fail::Fail {
    match Fenced::of(e) {
        Some(f) => fenced_fail(lang, &f.at, &f.why),
        None => crate::fail::Fail::new(format!("{}: {e}", dir.display())),
    }
}

/// 안 연 디렉터리의 거절 — 쓰는 길([`refusal`])과 읽는 길([`Garbled::refusal`])이 이 하나로 짓는다.
fn fenced_fail(lang: crate::i18n::Lang, at: &Path, why: &crate::held::Unheld) -> crate::fail::Fail {
    crate::fail::Fail::coded(crate::held::refused(lang, at, why), crate::fail::code::BROKEN)
}

/// 우편함·출석부가 선 트래커 뿌리 — **그 디렉터리의 두 칸 위다.** 이 모듈이 받는 `dir` 은 늘
/// [`crate::store::mail_at`]·[`crate::store::agents_at`] 이 지은 `<뿌리>/.moai/<mail|agents>` 다. 견주는 자리가 뿌리라서
/// `.moai` 자체가 밖을 가리키는 링크도 걸린다.
///
/// 뿌리를 따로 받지 않는 까닭은 부르는 쪽(`send`·`inbox`·`hello`·`agents` 와 훅)이 모두 그 디렉터리 하나만 들고
/// 오기 때문이다 — 뿌리를 둘째 인자로 받으면 두 값이 갈리는 꼴이 지어진다.
fn home_of(dir: &Path) -> crate::held::Home {
    crate::held::Home::of(dir.parent().and_then(Path::parent).unwrap_or(dir))
}

/// 체크아웃 안에서 푼 디렉터리 — 밖이나 `.git/` 으로 풀리면([`Fenced`]) 안 연다. 재는 자는 [`crate::held::place_dir`]
/// 이다 — 저장소가 든 파일을 읽는 자([`crate::held::place`])와 쓰는 자(`store::write_atomic_inside`)가 선 그 금이다.
///
/// **푼 자리를 낸다** — 뒤따르는 열기·짓기·옮기기는 그 자리에 한다. 받은 철자를 다시 열면 잰 뒤 바뀐 링크를
/// 따라간다(`held::read_inside` 와 같은 까닭). 링크가 없으면 두 자리는 같다.
fn reach(d: &Path, home: &crate::held::Home) -> Result<PathBuf, Fenced> {
    crate::held::place_dir(d, home).map_err(|why| Fenced { at: d.to_path_buf(), why })
}

/// [`reach`] 로 재고 짓는다 — 푼 자리를 낸다.
fn make_dir(d: &Path, home: &crate::held::Home) -> std::io::Result<PathBuf> {
    let real = reach(d, home)?;
    std::fs::create_dir_all(&real)?;
    Ok(real)
}

// ── 파일 ──────────────────────────────────────────────────────────────

/// 디렉터리를 짓고 **제 `.gitignore`(`*`)를 둔다.** `init` 이 `.gitignore` 에 이 자리를 더하지만, 그
/// `init` 을 아직 다시 안 친 저장소에서도 훅이 출석을 쓴다 — 거기서 `git add -A` 한 번이 pid 와 경로를
/// 커밋한다. 디렉터리가 제 무시를 들면 저장소 설정과 상관없이 git 이 안 본다(`*` 이 그 `.gitignore` 도
/// 가린다).
///
/// **빈 `.gitignore` 는 없는 것으로 친다**(리뷰 moai-h8tn.x4l) — 쓰다 기계가 죽으면 이름만 선 빈 파일이 남는데, 이름만
/// 보고 넘어가던 판은 그 디렉터리의 무시를 영영 껐다. 한 번만 쓰는 파일이라 내려 쓴다.
///
/// **짓기 전에 잰다**([`make_dir`], moai-kxkw.7ky) — 받은 저장소가 커밋한 `.moai/agents -> <밖>` 을 맨 `create_dir_all` 이
/// 따라가, `moai hello` 가 밖에 `.gitignore`(`*`)와 장을 썼다. 푼 자리를 내니 그 디렉터리의 쓰기는 거기 한다.
fn ensure_dir(dir: &Path, home: &crate::held::Home) -> std::io::Result<PathBuf> {
    let dir = make_dir(dir, home)?;
    let ignore = dir.join(".gitignore");
    match std::fs::symlink_metadata(&ignore) {
        Err(_) => {
            let _ = write_new(&ignore, b"*\n", true);
        }
        Ok(m) if m.is_file() && m.len() == 0 => {
            let _ = std::fs::remove_file(&ignore);
            let _ = write_new(&ignore, b"*\n", true);
        }
        Ok(_) => {}
    }
    Ok(dir)
}

/// 숨은 temp 이름 — pid 와 나노초라 겹치지 않는다. 숨은 이름이라 읽는 쪽이 안 본다.
fn temp_in(dir: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    dir.join(format!(".tmp.{}.{nanos}", std::process::id()))
}

/// 새 파일에만 쓴다(`O_EXCL`) — 그 자리에 무엇이 서 있으면(링크 포함) 안 쓴다. `sync` 면 내려 쓴다.
///
/// **쓰다 지면 만든 파일을 걷는다**(리뷰 moai-h8tn.x4l) — 디스크가 차서 쓰기가 지면 빈 파일이 남는다. temp 면
/// 출석부에서는 아무도 안 걷고, 디렉터리의 `.gitignore` 면 빈 채로 영영 서 — [`ensure_dir`] 은 이름만 보고 다시 안
/// 쓴다 — 그 디렉터리를 git 이 보게 된다.
fn write_new(path: &Path, bytes: &[u8], sync: bool) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes).and_then(|()| if sync { file.sync_all() } else { Ok(()) }).inspect_err(|_| {
        let _ = std::fs::remove_file(path);
    })
}

/// 디렉터리의 이름표를 내려 쓴다 — **편지의 `sync` 는 그 이름을 안 적는다**(리뷰 moai-h8tn.x4l, `store::write_atomic_in`
/// 과 같은 자). 들인 이름이 디스크에 안 선 채 기계가 죽으면 "보냈다" 고 한 편지가 temp 로만 남고, 그 temp 는 10분 뒤
/// [`sweep_temps`] 가 걷는다.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// 죽은 보내기([`send`])와 죽은 출석 쓰기([`write_presence`], 걷는 자리는 [`sweep`])가 남긴 temp 를 걷는다 — 10분 넘은
/// 것만. 쓰는 쪽은 temp 를 몇 밀리초만 든다. `dir` 은 [`reach`] 를 지난 자리다. **이름부터 본다** —
/// 편지마다 `stat` 을 치르지 않는다.
fn sweep_temps(dir: &Path) {
    const KEEP: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_name().to_str().is_some_and(|n| n.starts_with(".tmp.")) {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .filter(std::fs::Metadata::is_file)
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 그 디렉터리의 `*.json` 보통 파일 — `(자리, 이름에서 .json 을 뗀 것)`. 숨은 이름은 뺀다.
fn json_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".json")?.to_string();
            (!stem.starts_with('.')).then(|| (e.path(), stem))
        })
        .collect()
}

/// JSON 파일 하나를 읽는다 — 크기 상한을 넘으면 안 읽는다. 까닭은 사람이 읽을 한 줄이다.
///
/// **목록을 본 뒤에 사라진 파일은 못 읽은 것이 아니다**(`Ok(None)`) — 옆 세션이 그 사이에 읽음으로 옮긴
/// 편지다. 못 읽은 것으로 세던 판은 여럿이 함께 `inbox --ack` 하는 자리에서 멀쩡한 편지를 "못 읽는 편지" 로
/// 대고 비영으로 끝났다(겨루기 시험이 부하 아래에서 잡았다).
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let gone = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if gone(&e) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if meta.len() > FILE_MAX {
        return Err(format!("{} bytes", meta.len()));
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if gone(&e) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_str(&text).map(Some).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn letter(to: &str, from: &str, subject: &str) -> Letter {
        Letter {
            v: VERSION,
            to: to.into(),
            from: from.into(),
            subject: subject.into(),
            body: String::new(),
            sent_at: "2026-10-04T06:12:03Z".into(),
            reply_to: None,
            rest: BTreeMap::new(),
        }
    }

    /// **편지 파일의 꼴은 되돌릴 수 없다** — 일곱 키가 이 차례로 서고 `v` 는 1 이다. 여기가 바뀌면 이미 쓴
    /// 편지를 옛 바이너리가 못 읽거나 새 바이너리가 다르게 읽는다.
    #[test]
    fn the_letter_file_has_seven_keys_in_order() {
        let s = Scratch::new("mail-shape");
        let id = send(s.path(), &letter("b", "a", "hi")).unwrap();
        let text = std::fs::read_to_string(s.path().join("b").join(format!("{id}.json"))).unwrap();
        assert_eq!(
            text,
            "{\"v\":1,\"to\":\"b\",\"from\":\"a\",\"subject\":\"hi\",\"body\":\"\",\"sent_at\":\"2026-10-04T06:12:03Z\",\"reply_to\":null}\n"
        );
        assert!(is_id(&id), "{id}");
        assert!(id.starts_with("20261004-061203-"), "{id}");
    }

    /// **모르는 키는 남는다** — 다른 판이 더한 키가 읽고 다시 내는 사이에 사라지면, 새 판이 쓴 편지를 옛 판이
    /// 한 번 읽는 것만으로 그 뜻이 없어진다.
    #[test]
    fn unknown_keys_survive_reading() {
        let s = Scratch::new("mail-unknown");
        std::fs::create_dir_all(s.path().join("b")).unwrap();
        std::fs::write(
            s.path().join("b/20261004-061203-00000001.json"),
            r#"{"v":2,"to":"b","from":"a","subject":"s","body":"","sent_at":"2026-10-04T06:12:03Z","reply_to":null,"priority":"high"}"#,
        )
        .unwrap();
        let (got, bad) = list(s.path(), "b", false);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(got[0].letter.v, 2);
        assert_eq!(got[0].letter.rest.get("priority"), Some(&serde_json::json!("high")));
        let again = serde_json::to_string(&got[0].letter).unwrap();
        assert!(again.contains(r#""priority":"high""#), "{again}");
    }

    /// 반쯤 쓴 편지(숨은 temp)와 편지 꼴이 아닌 이름, 링크는 안 읽는다.
    #[test]
    fn temps_and_strangers_are_not_letters() {
        let s = Scratch::new("mail-strangers");
        let held = s.path().join("b");
        std::fs::create_dir_all(&held).unwrap();
        std::fs::write(held.join(".tmp.1.2"), "{").unwrap();
        std::fs::write(held.join("notes.json"), "{}").unwrap();
        let id = send(s.path(), &letter("b", "a", "real")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(held.join(format!("{id}.json")), held.join("20261004-061203-zzzzzzzz.json"))
            .unwrap();
        let (got, bad) = list(s.path(), "b", false);
        assert_eq!(got.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), [id.as_str()]);
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// **id 의 글자 차례가 보낸 차례다** — 같은 초 안에서도. 훅이 싣는 차례가 이것이다.
    #[test]
    fn ids_sort_in_the_order_sent() {
        let s = Scratch::new("mail-order");
        let ids: Vec<String> = (0..5).map(|n| send(s.path(), &letter("b", "a", &n.to_string())).unwrap()).collect();
        let (got, _) = list(s.path(), "b", false);
        assert_eq!(got.iter().map(|g| g.letter.subject.clone()).collect::<Vec<_>>(), ["0", "1", "2", "3", "4"]);
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    /// **같은 이름이 서 있으면 다음 이름으로 든다** — 덮지 않는다. `rename` 으로 들이던 판이면 여기서 앞
    /// 편지가 사라진다.
    #[test]
    fn a_taken_id_is_never_overwritten() {
        let s = Scratch::new("mail-taken");
        let at = "2026-10-04T06:12:03Z";
        // 첫 이름 셋을 남이 먼저 쥐었다 — 같은 마이크로초에 든 이웃이 그 자리다.
        let base = 1_000;
        for bump in 0..3 {
            let mut squatter = letter("b", "a", "squatter");
            squatter.body = bump.to_string();
            let text = serde_json::to_string(&squatter).unwrap();
            std::fs::create_dir_all(s.path().join("b")).unwrap();
            std::fs::write(s.path().join("b").join(format!("{}.json", mint(at, base + bump))), text).unwrap();
        }
        // 시계가 멈춰 있어도 다시 들 때마다 하나 올린다.
        let id = send_from(s.path(), &letter("b", "a", "mine"), &mut || base).unwrap();
        assert_eq!(id, mint(at, base + 3), "비어 있는 다음 이름으로 안 들었다");
        let (got, _) = list(s.path(), "b", false);
        let bodies: Vec<&str> =
            got.iter().filter(|g| g.letter.subject == "squatter").map(|g| g.letter.body.as_str()).collect();
        assert_eq!(bodies, ["0", "1", "2"], "먼저 선 편지를 덮었다");
        assert!(got.iter().any(|g| g.id == id && g.letter.subject == "mine"), "새 편지가 안 섰다");
        assert!(
            !std::fs::read_dir(s.path().join("b")).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp.")),
            "temp 가 남았다"
        );
    }

    /// **이름은 temp 를 다 쓴 뒤에 짓는다**(리뷰 moai-h8tn.x4l) — 그 앞에 지은 이름은 `sync` 하는 사이에 읽혀 나간
    /// 이웃의 이름과 겹칠 수 있고, 그러면 [`take`] 의 `rename` 이 읽은 편지를 덮는다. 시계를 읽는 순간 temp 가
    /// 이미 서 있어야 한다.
    #[test]
    fn the_id_is_minted_once_the_letter_is_written() {
        let s = Scratch::new("mail-mint-late");
        let dir = s.path().to_path_buf();
        let mut written = false;
        send_from(&dir, &letter("b", "a", "x"), &mut || {
            written = std::fs::read_dir(dir.join("b"))
                .unwrap()
                .any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".tmp."));
            7
        })
        .unwrap();
        assert!(written, "temp 를 쓰기 전에 이름을 지었다");
    }

    /// **먼저 옮긴 쪽만 가지고, 뒤의 쪽은 `Lost` 를 받는다** — 읽은 이는 파일 이름에 선다. 차례대로 부르는
    /// 시험이라 겨룸은 못 잰다 — 그것은 `tests/cli.rs` 의 겨루기 시험 둘이 잰다([`take`]).
    #[test]
    fn two_takers_never_share_a_letter() {
        let s = Scratch::new("mail-take");
        send(s.path(), &letter(ANY_IDLE_WORKER, "boss", "job")).unwrap();
        let (open, _) = list(s.path(), "w1", false);
        assert_eq!(take(s.path(), &open[0], "w1").unwrap(), Took::Mine);
        assert_eq!(take(s.path(), &open[0], "w2").unwrap(), Took::Lost);
        let (got, _) = list(s.path(), "w1", true);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].reader.as_deref(), Some("w1"));
        assert!(for_me(&got[0], "w1", "") && !for_me(&got[0], "w2", ""), "읽은 편지가 남의 것으로도 섰다");
        // 진 쪽은 누가 가졌는지 이름으로 묻는다 — 같은 이름이 가졌으면 진 것이 아니다(`inbox` 의 기다림).
        assert!(read_by(s.path(), &open[0], "w1") && !read_by(s.path(), &open[0], "w2"));
    }

    /// **편지 하나는 그 id 의 파일만 연다**(리뷰 moai-54yc.vqe) — 그 id 와 무관한 깨진 편지는 그 부름에 안 선다(`moai inbox
    /// <id>` 가 그것 때문에 덜 낸 것으로 끝나지 않는다). 안 읽은 것도 읽은 것도 그 id 면 든다.
    #[test]
    fn one_letter_opens_only_its_own_files() {
        let s = Scratch::new("mail-one");
        let a = send(s.path(), &letter("w1", "boss", "a")).unwrap();
        send(s.path(), &letter("w1", "boss", "b")).unwrap();
        std::fs::write(s.path().join("w1/20261004-061203-zzzzzzzz.json"), "{").unwrap();
        let (_, bad) = list(s.path(), "w1", false);
        assert_eq!(bad.len(), 1, "깨진 편지를 안 댔다 — 시험이 헛돈다");
        let (got, bad) = list_one(s.path(), "w1", &a);
        assert!(bad.is_empty(), "그 id 와 무관한 깨진 편지를 댔다 — {bad:?}");
        assert_eq!(got.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), [a.as_str()]);
        assert_eq!(take(s.path(), &got[0], "w1").unwrap(), Took::Mine);
        std::fs::write(s.path().join("w1/read/20261004-061203-zzzzzzzy@w1.json"), "{").unwrap();
        let (got, bad) = list_one(s.path(), "w1", &a);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(got.len(), 1, "읽은 편지를 안 찾았다");
        assert_eq!(got[0].reader.as_deref(), Some("w1"));
        // 그 id 의 파일이 깨졌으면 댄다 — 못 찾은 것과 못 읽은 것을 가른다.
        let (got, bad) = list_one(s.path(), "w1", "20261004-061203-zzzzzzzz");
        assert!(got.is_empty() && bad.len() == 1, "{got:?} {bad:?}");
    }

    /// `any-idle-worker` 는 보낸 이와 감독을 빼고 누구에게나 간다(2026-10-04 사용자 결정).
    #[test]
    fn any_idle_worker_skips_the_sender_and_supervisors() {
        let open = Stored {
            id: "x".into(),
            mailbox: ANY_IDLE_WORKER.into(),
            reader: None,
            returned: false,
            letter: letter(ANY_IDLE_WORKER, "boss", "job"),
        };
        assert!(for_me(&open, "w1", ""));
        assert!(for_me(&open, "w1", "worker"));
        assert!(!for_me(&open, "boss", ""), "보낸 이가 제 편지를 받았다");
        assert!(!for_me(&open, "other", SUPERVISOR), "감독이 일감을 가졌다");
        let named = Stored {
            id: "y".into(),
            mailbox: "w1".into(),
            reader: None,
            returned: false,
            letter: letter("w1", "boss", "job"),
        };
        assert!(for_me(&named, "w1", SUPERVISOR) && !for_me(&named, "w2", ""));
    }

    #[test]
    fn names_are_file_safe() {
        for good in ["moa-issue-b6", "claude-6d160dd7", "a.b_c"] {
            assert!(is_agent_name(good), "{good}");
        }
        for bad in ["", ".hidden", "a/b", "a@b", "../x", ANY_IDLE_WORKER, &"x".repeat(65), "한글"] {
            assert!(!is_agent_name(bad), "{bad}");
        }
        assert_eq!(name_from(" moa issue/b6 ").as_deref(), Some("moa-issue-b6"));
        assert_eq!(name_from("..x").as_deref(), Some("x"));
        assert_eq!(name_from(""), None);
        // **한글·기호만 든 세션 이름은 이름이 못 된다**(리뷰 moai-h8tn.x4l) — 접으면 `--` 라 명령줄이 깃발로 읽고,
        // 길이가 같은 다른 이름과 겹친다. `-` 로 여는 이름도 깃발이라 앞을 걷는다.
        assert_eq!(name_from("감독"), None);
        assert_eq!(name_from("(wip) fix").as_deref(), Some("wip--fix"));
        assert_eq!(name_from("moai 감독").as_deref(), Some("moai---"));
        // **잇는 토막은 상한에 안 잘린다** — 잘리면 가른 이름이 도로 산 남의 이름이 된다.
        let long = name_from(&"a".repeat(70)).unwrap();
        let split = name_with(&long, "sess0001").unwrap();
        assert!(split.len() == 64 && split.ends_with("-sess0001") && split != long, "{split}");
    }

    /// `/proc/<pid>/stat` 의 이름에 괄호와 빈칸이 들어도 부모와 선 때를 바로 읽는다.
    #[test]
    fn a_stat_line_with_parens_in_the_name_parses() {
        let line = "42 (a (b) c) S 7 42 42 0 -1 4194560 1 0 0 0 0 0 0 0 20 0 1 0 98765 0 0";
        let p = parse_stat(42, line).unwrap();
        assert_eq!((p.ppid, p.start, p.comm.as_str()), (7, Some(98765), "a (b) c"));
    }

    /// 이 시험 프로세스는 살아 있고, 선 때가 다르면 죽은 것이다(pid 재사용). 출석부의 걷기가 이 자로 선다.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_reused_pid_reads_as_dead() {
        let me = proc_of(std::process::id()).unwrap();
        assert_eq!(alive(me.pid, me.start), Some(true));
        assert_eq!(alive(me.pid, me.start.map(|s| s + 1)), Some(false));
        assert_eq!(alive(me.pid, None), Some(true));
    }

    #[test]
    fn vendors_are_read_off_the_process_name() {
        assert_eq!(vendor_of("claude"), Some("claude"));
        assert_eq!(vendor_of("codex-x86_64-unknown-linux-musl"), Some("codex"));
        assert_eq!(vendor_of("agy"), Some("antigravity"));
        assert_eq!(vendor_of("bash"), None);
    }

    /// 출석은 덮어 적고, 걷는 것은 프로세스가 죽은 장과 프로세스를 모르는데 하루 넘게 안 적힌 장이다(moai-j3n5).
    /// **20분 넘게 조용한 장은 떠난 것으로 읽을 뿐 남는다**(2026-10-05 사용자 결정) — 역할과 이름을 들고 그 세션을
    /// 기다린다. **죽은 장의 편지는 보낸 이에게 되돌아온 표를 달고 돌아가고, 프로세스를 모르는 장의 편지는 남는다**
    /// (moai-ew4o.l3n) — 그 세션은 살아 돌아와 같은 이름을 다시 받을 수 있다.
    #[test]
    #[cfg(target_os = "linux")]
    fn the_sweep_takes_the_dead_and_the_expired() {
        let s = Scratch::new("agents-sweep");
        let (agents, mail) = (s.path().join("agents"), s.path().join("mail"));
        let me = proc_of(std::process::id()).unwrap();
        let now = crate::model::parse_rfc3339(&crate::model::now()).unwrap();
        let ago = |secs: i64| Some(crate::model::format_rfc3339(now - secs));
        let card = |name: &str, pid: u32, start: Option<u64>, seen: Option<String>| Presence {
            v: VERSION,
            name: name.into(),
            vendor: "claude".into(),
            model: String::new(),
            role: String::new(),
            status: IDLE.into(),
            since: seen.clone().unwrap_or_else(|| "2026-10-04T06:12:03Z".into()),
            pid,
            pid_start: start,
            machine: None,
            host: None,
            session: None,
            cwd: String::new(),
            tmux_pane: None,
            tmux_socket: None,
            seen,
            rest: BTreeMap::new(),
        };
        write_presence(&agents, &card("live", me.pid, me.start, None)).unwrap();
        write_presence(&agents, &card("reused", me.pid, me.start.map(|t| t + 1), None)).unwrap();
        write_presence(&agents, &card("unknown", 0, None, ago(60))).unwrap();
        write_presence(&agents, &card("stale", 0, None, ago(STALE_AFTER + 1))).unwrap();
        write_presence(&agents, &card("expired", 0, None, ago(EXPIRE_AFTER + 1))).unwrap();
        send(&mail, &letter("reused", "boss", "to the dead")).unwrap();
        send(&mail, &letter("expired", "boss", "to the expired")).unwrap();
        let mut swept: Vec<(String, bool)> = sweep(&agents, &mail).into_iter().map(|s| (s.name, s.dead)).collect();
        swept.sort();
        assert_eq!(swept, [("expired".to_string(), false), ("reused".to_string(), true)]);
        let (left, _) = presences(&agents);
        assert_eq!(left.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["live", "stale", "unknown"]);
        assert!(left.iter().any(|p| p.name == "stale" && p.gone()), "조용한 장을 떠난 것으로 안 읽었다");
        assert!(
            me_among(&left, std::slice::from_ref(&me)).is_some_and(|p| p.name == "live"),
            "조상의 pid 로 제 출석을 못 찾았다"
        );
        let (back, _) = list(&mail, "boss", false);
        assert_eq!(back.len(), 1, "죽은 이의 편지가 안 돌아왔다 — {back:?}");
        assert!(back[0].returned && back[0].letter.subject == "to the dead", "{back:?}");
        assert_eq!(list(&mail, "expired", false).0.len(), 1, "프로세스를 모르는 이의 편지를 되돌렸다");
        assert!(list(&mail, "reused", false).0.is_empty(), "되돌린 편지가 떠난 이의 함에도 남았다");
    }

    /// **떠난 이름을 넘겨받으면 그 함부터 비우고, 이름을 바꾸면 편지가 따라간다**(moai-ew4o.l3n). 아무도 안 쥔 이름 앞의
    /// 편지(인사 전에 보낸 것)는 그대로 남아 다음에 그 이름을 받는 이에게 간다. 제가 쓴 편지는 떠난 이가 읽은 것으로
    /// 둔다 — 돌려보낼 곳이 없다.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_name_taken_over_returns_its_letters_and_a_rename_carries_them() {
        let s = Scratch::new("mail-retire");
        let (agents, mail) = (s.path().join("agents"), s.path().join("mail"));
        let me = proc_of(std::process::id()).unwrap();
        let mut gone = Presence {
            v: VERSION,
            name: "w1".into(),
            vendor: "claude".into(),
            model: String::new(),
            role: String::new(),
            status: IDLE.into(),
            since: String::new(),
            pid: me.pid,
            pid_start: me.start.map(|t| t + 1),
            machine: None,
            host: None,
            session: None,
            cwd: String::new(),
            tmux_pane: None,
            tmux_socket: None,
            seen: None,
            rest: BTreeMap::new(),
        };
        write_presence(&agents, &gone).unwrap();
        send(&mail, &letter("w1", "boss", "job")).unwrap();
        send(&mail, &letter("w1", "w1", "note to self")).unwrap();
        send(&mail, &letter("early", "boss", "before hello")).unwrap();
        let (all, _) = presences(&agents);
        take_over(&mail, &all, "W1", None);
        let (back, _) = list(&mail, "boss", false);
        assert_eq!(back.iter().map(|b| b.letter.subject.as_str()).collect::<Vec<_>>(), ["job"]);
        assert!(back[0].returned, "되돌아온 편지로 안 읽힌다");
        let (left, _) = list(&mail, "w1", true);
        assert!(
            left.iter().all(|l| l.reader.as_deref() == Some("w1")),
            "떠난 이의 함에 안 읽은 편지가 남았다 — {left:?}"
        );
        // 산 장의 이름은 넘겨받지 않는다 — 함도 그대로다.
        gone.pid_start = me.start;
        write_presence(&agents, &gone).unwrap();
        send(&mail, &letter("w1", "boss", "second")).unwrap();
        let (all, _) = presences(&agents);
        take_over(&mail, &all, "w1", None);
        assert_eq!(list(&mail, "w1", false).0.len(), 1, "산 이의 편지를 되돌렸다");
        // 이름을 바꾸면 편지가 새 함으로 따라간다 — 되돌아온 편지가 아니다.
        let moved = Presence { name: "w2".into(), ..gone };
        rename_card(&agents, &mail, &moved, "w1").unwrap();
        let (carried, _) = list(&mail, "w2", false);
        assert_eq!(carried.iter().map(|c| c.letter.subject.as_str()).collect::<Vec<_>>(), ["second"]);
        assert!(!carried[0].returned);
        assert_eq!(presences(&agents).0.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["w2"]);
        // 인사 전에 보낸 편지는 아무도 안 쥔 이름 앞에 남는다.
        assert_eq!(list(&mail, "early", false).0.len(), 1);
    }

    /// **되돌아온 표는 파일 이름에 산다**(2026-10-05 사용자 결정, 리뷰 moai-ew4o.q9f 10·11번) — 이름으로 알아내던 판은
    /// 받는 이가 보낸 이의 이름을 받으면 그 이에게 온 편지가 "되돌아왔다" 로 서고, 되돌아온 편지는 보낸 이가 이름을 바꾸면
    /// 표를 잃었다. 둘 다 떠난 뒤 아무도 안 쥔 이름 앞으로 되돌린 편지는, 그 이름을 새로 받는 장에게 앞사람의 것이라
    /// 읽음으로 치운다.
    #[test]
    fn the_returned_mark_lives_in_the_file_name() {
        let s = Scratch::new("mail-returned-mark");
        let mail = s.path();
        // 받는 이(w2)가 보낸 이의 이름(boss)으로 바꿔도 그 편지는 되돌아온 것이 아니다.
        send(mail, &letter("w2", "boss", "job")).unwrap();
        carry(mail, "w2", "boss");
        let (got, _) = list(mail, "boss", false);
        assert_eq!(got.len(), 1);
        assert!(!got[0].returned, "이름을 바꾼 받는 이의 편지를 되돌아온 것으로 읽었다");
        // 되돌아온 편지는 보낸 이가 이름을 바꿔도 표를 들고 간다.
        send(mail, &letter("gone1", "sup", "task")).unwrap();
        retire(mail, "gone1");
        let (back, _) = list(mail, "sup", false);
        assert!(back.len() == 1 && back[0].returned, "{back:?}");
        assert!(mail.join("sup").join(format!("{}.returned.json", back[0].id)).is_file(), "표가 파일 이름에 없다");
        carry(mail, "sup", "sup2");
        let (moved, _) = list(mail, "sup2", false);
        assert!(moved.len() == 1 && moved[0].returned, "이름을 바꾸니 표를 잃었다 — {moved:?}");
        // 읽어도 표가 남는다 — `inbox --all` 이 다시 보일 때.
        assert_eq!(take(mail, &moved[0], "sup2").unwrap(), Took::Mine);
        let (read, _) = list(mail, "sup2", true);
        assert!(read.len() == 1 && read[0].returned && read[0].reader.as_deref() == Some("sup2"), "{read:?}");
        // 둘 다 떠난 뒤 아무도 안 쥔 이름 앞으로 되돌린 편지는, 그 이름을 새로 받는 이에게 안 간다.
        send(mail, &letter("zen-owl", "amber-fox", "for zen")).unwrap();
        retire(mail, "zen-owl");
        assert!(list(mail, "amber-fox", false).0.iter().all(|l| l.returned), "되돌린 편지에 표가 없다");
        take_over(mail, &[], "amber-fox", None);
        assert!(list(mail, "amber-fox", false).0.is_empty(), "앞사람에게 되돌아온 편지가 새 장에 남았다");
        // 같은 이름을 이어 쓰는 장은 제 되돌아온 편지를 그대로 받는다.
        send(mail, &letter("zen-owl", "amber-fox", "again")).unwrap();
        retire(mail, "zen-owl");
        take_over(mail, &[], "amber-fox", Some("amber-fox"));
        assert_eq!(list(mail, "amber-fox", false).0.len(), 1, "이어 쓰는 장의 되돌아온 편지를 치웠다");
    }

    /// **한 함에 모두 두던 판의 편지는 받는 이의 함으로 옮겨진다**(moai-ew4o.c92) — 판을 갈아 끼우는 순간 길을 잃지 않게.
    /// 읽은 편지도 옮긴다. 이름이 `read` 인 에이전트의 함과 옛 읽음 자리가 한 디렉터리여도 갈린다.
    #[test]
    fn flat_letters_move_into_their_mailboxes() {
        let s = Scratch::new("mail-migrate");
        let dir = s.path();
        let text = |to: &str| serde_json::to_string(&letter(to, "boss", "old")).unwrap();
        std::fs::write(dir.join("20261004-061203-00000001.json"), text("w1")).unwrap();
        std::fs::create_dir_all(dir.join("read")).unwrap();
        std::fs::write(dir.join("read/20261004-061203-00000002@w1.json"), text("w1")).unwrap();
        std::fs::write(dir.join("read/20261004-061203-00000003.json"), text("read")).unwrap();
        std::fs::write(dir.join("20261004-061203-00000004.json"), "not json").unwrap();
        migrate(dir);
        let (got, _) = list(dir, "w1", true);
        let mut seen: Vec<(&str, bool)> = got.iter().map(|g| (g.id.as_str(), g.reader.is_some())).collect();
        seen.sort();
        assert_eq!(seen, [("20261004-061203-00000001", false), ("20261004-061203-00000002", true)]);
        assert!(!dir.join("20261004-061203-00000001.json").exists(), "옛 자리에 남았다");
        assert_eq!(list(dir, "read", false).0.len(), 1, "`read` 의 함을 옛 읽음 자리로 읽었다");
        assert!(dir.join("20261004-061203-00000004.json").exists(), "받는 이를 못 읽는 파일을 옮겼다");
    }

    /// **함을 옮길 때 같은 id 가 서 있으면 덮지 않고 다음 id 로 든다**([`move_new`]) — 옮긴 자리의 편지가 사라지면 조용한
    /// 손실이다.
    #[test]
    fn a_relocated_letter_never_overwrites() {
        let s = Scratch::new("mail-relocate");
        let dir = s.path();
        let id = send(dir, &letter("a", "boss", "first")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::copy(dir.join("a").join(format!("{id}.json")), dir.join("b").join(format!("{id}.json"))).unwrap();
        let home = home_of(dir);
        let from = dir.join("a").join(format!("{id}.json"));
        let (moved, _) = relocate(&from, &dir.join("b"), &home, &id, false, "2026-10-04T06:12:03Z").unwrap().unwrap();
        assert_ne!(moved, id, "같은 id 위로 옮겼다");
        assert_eq!(list(dir, "b", false).0.len(), 2, "옮긴 자리의 편지를 덮었다");
        assert!(list(dir, "a", false).0.is_empty());
        // 남이 먼저 가진 편지는 못 옮긴다 — 옮긴 것으로 세지 않는다.
        assert_eq!(relocate(&dir.join("a").join("gone.json"), &dir.join("b"), &home, &id, false, "").unwrap(), None);
    }

    /// 시험용 장 하나 — 프로세스를 모르는(pid 0) Codex 장이다. 시험마다 고칠 칸만 고친다.
    fn codex_card(name: &str) -> Presence {
        Presence {
            v: VERSION,
            name: name.into(),
            vendor: "codex".into(),
            model: String::new(),
            role: String::new(),
            status: IDLE.into(),
            since: String::new(),
            pid: 0,
            pid_start: None,
            machine: None,
            host: None,
            session: None,
            cwd: String::new(),
            tmux_pane: None,
            tmux_socket: None,
            seen: None,
            rest: BTreeMap::new(),
        }
    }

    /// **이어 쓰는 장은 넘겨받는 것이 아니다**(리뷰 moai-ew4o.q9f) — 제 장이 떠난 것으로 읽혀도(닻이 낡은 Codex 장) 그 함은
    /// 그대로다. 같은 이름의 떠난 장을 남이 넘겨받을 때는 여전히 비운다.
    #[test]
    fn the_card_being_continued_is_not_taken_over() {
        let s = Scratch::new("mail-keep");
        let (agents, mail) = (s.path().join("agents"), s.path().join("mail"));
        let card = Presence { seen: Some("2026-10-04T06:12:03Z".into()), ..codex_card("codex-01a107b4") };
        write_presence(&agents, &card).unwrap();
        send(&mail, &letter("codex-01a107b4", "boss", "job")).unwrap();
        let (all, _) = presences(&agents);
        assert!(all[0].gone(), "시험의 장이 낡지 않았다");
        take_over(&mail, &all, "codex-01a107b4", Some("codex-01a107b4"));
        assert_eq!(list(&mail, "codex-01a107b4", false).0.len(), 1, "이어 쓰는 장의 편지를 되돌렸다");
        assert!(list(&mail, "boss", false).0.is_empty());
        take_over(&mail, &all, "codex-01a107b4", None);
        assert_eq!(list(&mail, "boss", false).0.len(), 1, "남이 넘겨받는 떠난 장의 함을 안 비웠다");
    }

    /// **걷기 바로 앞에 장을 다시 읽는다**(리뷰 moai-ew4o.q9f) — 출석부를 읽은 뒤에 같은 이름을 새 세션이 넘겨받았으면 그
    /// 장도 그 함도 안 건드린다. 읽은 그대로 걷던 판은 새 장을 지우고 새 세션 앞의 편지를 보낸 이에게 되돌렸다.
    #[test]
    #[cfg(target_os = "linux")]
    fn the_sweep_leaves_a_card_rewritten_after_it_looked() {
        let s = Scratch::new("agents-sweep-race");
        let (agents, mail) = (s.path().join("agents"), s.path().join("mail"));
        let me = proc_of(std::process::id()).unwrap();
        let old = Presence {
            vendor: "claude".into(),
            pid: me.pid,
            pid_start: me.start.map(|t| t + 1),
            session: Some("old".into()),
            ..codex_card("w7")
        };
        write_presence(&agents, &old).unwrap();
        let (looked, _) = presences(&agents);
        assert!(looked[0].dead(), "시험의 장이 죽은 것이 아니다");
        // 그 사이 새 세션이 그 이름을 넘겨받아 산 프로세스로 장을 쓰고, 그 세션 앞으로 편지가 온다.
        write_presence(&agents, &Presence { pid_start: me.start, session: Some("new".into()), ..old }).unwrap();
        send(&mail, &letter("w7", "boss", "for the newcomer")).unwrap();
        assert!(sweep_from(&agents, &mail, looked, "2026-10-05T00:00:00Z").is_empty(), "새 장을 걷었다");
        assert_eq!(presences(&agents).0.len(), 1, "새 장이 사라졌다");
        assert_eq!(list(&mail, "w7", false).0.len(), 1, "새 세션 앞의 편지를 되돌렸다");
        assert!(list(&mail, "boss", false).0.is_empty());
    }

    /// **대소문자만 다른 `any-idle-worker` 도 이름이 아니다**(리뷰 moai-ew4o.q9f) — 대소문자를 안 가리는 파일 시스템에서는 그
    /// 이름의 함이 열린 편지의 함이다. 받는 이 자리에는 그 낱말 그대로만 선다.
    #[test]
    fn any_idle_worker_in_another_case_is_no_ones_name() {
        for name in ["Any-Idle-Worker", "ANY-IDLE-WORKER", ANY_IDLE_WORKER] {
            assert!(!is_agent_name(name), "{name} 을 이름으로 받았다");
        }
        assert!(is_recipient(ANY_IDLE_WORKER) && is_recipient("w1"));
        assert!(!is_recipient("Any-Idle-Worker"));
        assert_eq!(name_from("Any Idle Worker"), None, "세션 이름을 열린 편지의 함으로 접었다");
        let s = Scratch::new("mail-any-case");
        assert!(send(s.path(), &letter("Any-Idle-Worker", "boss", "x")).is_err(), "대소문자만 다른 낱말 앞으로 보냈다");
    }

    /// **닻과 `since` 가운데 늦은 쪽으로 잰다**(리뷰 moai-ew4o.q9f) — 닻을 모르는 옛 판의 훅이 `since` 만 새로 대도 그 장은
    /// 산 것이다. 둘 다 못 읽으면 모른다.
    #[test]
    fn staleness_reads_the_later_of_seen_and_since() {
        let card = |seen: Option<&str>, since: &str| Presence {
            seen: seen.map(str::to_string),
            since: since.into(),
            ..codex_card("cx")
        };
        let now = "2026-10-04T07:00:00Z";
        assert!(!card(Some("2026-10-04T06:00:00Z"), "2026-10-04T06:55:00Z").stale(now), "새로 선 since 를 안 봤다");
        assert!(card(Some("2026-10-04T06:00:00Z"), "2026-10-04T06:10:00Z").stale(now));
        assert!(!card(Some("2026-10-04T06:59:00Z"), "2026-10-04T06:10:00Z").stale(now));
        assert!(!card(None, "2026-10-04T06:50:00Z").stale(now));
        assert!(card(None, "2026-10-04T06:30:00Z").stale(now));
        assert!(!card(None, "").stale(now), "모르는 것을 낡은 것으로 읽었다");
    }

    /// **지은 이름 셋이 다 쥐였어도 이름을 낸다**(리뷰 moai-nas5.cn7 15번, moai-keka.q2w) — 앞 8자 토막 뒤에 수를 이어
    /// 빈 이름을 찾는다. `None` 을 내던 판은 훅이 그 세션에 장을 안 세워, 그 세션의 `moai inbox`·`send` 가 누구인지 몰라
    /// 섰다. 이어 보는 수는 출석부의 장 수보다 하나 많아 하나는 반드시 빈다.
    #[test]
    fn a_made_name_is_found_even_when_every_suffix_is_held() {
        let now = crate::model::now();
        let live = |name: &str| Presence { since: now.clone(), seen: Some(now.clone()), ..codex_card(name) };
        let session = "01a107b4-6b9e-7c3d-8a21-5f0e9d4c3b2a";
        let whole = "codex-01a107b46b9e7c3d8a215f0e9d4c3b2a";
        let mut all = vec![live("codex"), live("codex-01a107b4"), live(whole)];
        assert!(all.iter().all(Presence::holds_made_name), "시험의 장이 이름을 안 쥐었다");
        assert_eq!(made_name(&all, "codex".into(), session).as_deref(), Some("codex-01a107b4-2"));
        all.push(live("CODEX-01A107B4-2"));
        assert_eq!(made_name(&all, "codex".into(), session).as_deref(), Some("codex-01a107b4-3"), "대소문자만 다른 장");
        // 출석부가 그 이름들로 다 차도 하나는 빈다.
        let full: Vec<Presence> = std::iter::once(live("codex"))
            .chain([live("codex-01a107b4"), live(whole)])
            .chain((2..=8).map(|n| live(&format!("codex-01a107b4-{n}"))))
            .collect();
        let got = made_name(&full, "codex".into(), session).expect("이름을 못 냈다");
        assert!(!full.iter().any(|p| p.name.eq_ignore_ascii_case(&got)), "쥔 이름을 냈다 — {got}");
    }

    /// **다른 기계의 장은 pid 로 안 잰다**(moai-dhxm) — 같은 저장소를 컨테이너 여럿이 쓰면 장의 pid 는 그것을 적은
    /// 컨테이너에서만 뜻을 갖는다. 이 기계에 없는 pid 를 죽은 것으로 읽던 판은 `moai agents` 가 남의 산 장을 걷고 그 함의
    /// 편지를 보낸 이에게 되돌렸고, 이 기계의 프로세스와 pid 가 겹친 장은 제 것으로 읽었다. 그 장은 닻으로 잰다 — 20분
    /// 조용하면 떠난 것으로 읽고, 하루 조용하면 걷되 함은 남긴다. 기계를 안 적은 옛 장은 옛 판처럼 pid 로 잰다.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_card_from_another_machine_is_measured_by_its_anchor() {
        let here = machine().expect("이 기계를 못 읽었다");
        let (boot, ns) = here.split_once('/').expect("기계의 꼴이 `<boot_id>/<번호>` 가 아니다");
        assert!(boot.len() == 36 && ns.bytes().all(|b| b.is_ascii_digit()), "{here}");
        let s = Scratch::new("agents-elsewhere");
        let (agents, mail) = (s.path().join("agents"), s.path().join("mail"));
        let me = proc_of(std::process::id()).unwrap();
        let now = crate::model::parse_rfc3339(&crate::model::now()).unwrap();
        let ago = |secs: i64| crate::model::format_rfc3339(now - secs);
        // 이 기계에는 없는 pid — 커널의 pid 상한(4194304) 위다.
        let nobody = 4_194_400;
        let there = "00000000-0000-0000-0000-000000000000/4026532999";
        let card = |name: &str, pid: u32, start: Option<u64>, machine: Option<&str>, seen: String| Presence {
            vendor: "claude".into(),
            pid,
            pid_start: start,
            machine: machine.map(str::to_string),
            host: machine.map(|_| "box-b".to_string()),
            since: seen.clone(),
            seen: Some(seen),
            ..codex_card(name)
        };
        write_presence(&agents, &card("away", nobody, Some(1), Some(there), ago(60))).unwrap();
        write_presence(&agents, &card("twin", me.pid, me.start, Some(there), ago(60))).unwrap();
        write_presence(&agents, &card("quiet", nobody, Some(1), Some(there), ago(STALE_AFTER + 1))).unwrap();
        write_presence(&agents, &card("long-gone", nobody, Some(1), Some(there), ago(EXPIRE_AFTER + 1))).unwrap();
        write_presence(&agents, &card("old", nobody, Some(1), None, ago(60))).unwrap();
        write_presence(&agents, &card("mine", nobody, Some(1), Some(&here), ago(60))).unwrap();
        for name in ["away", "quiet", "long-gone", "old", "mine"] {
            send(&mail, &letter(name, "boss", name)).unwrap();
        }
        let (all, _) = presences(&agents);
        let by = |name: &str| all.iter().find(|p| p.name == name).unwrap().clone();
        assert!(!by("away").dead() && !by("away").gone(), "다른 기계의 산 장을 죽은 것으로 읽었다");
        assert!(by("quiet").gone() && !by("quiet").dead(), "조용한 다른 기계의 장을 떠난 것으로 안 읽었다");
        assert!(me_among(&all, std::slice::from_ref(&me)).is_none(), "pid 가 겹친 다른 기계의 장을 제 것으로 읽었다");
        assert!(by("old").dead() && by("mine").dead(), "이 기계의 죽은 장을 안 읽었다");
        let mut swept: Vec<(String, bool, bool)> =
            sweep(&agents, &mail).into_iter().map(|s| (s.name, s.dead, s.elsewhere)).collect();
        swept.sort();
        assert_eq!(
            swept,
            [
                ("long-gone".to_string(), false, true),
                ("mine".to_string(), true, false),
                ("old".to_string(), true, false)
            ]
        );
        // 이 기계의 죽은 장은 편지가 돌아간다. 다른 기계의 장은 20분 조용해도 함이 남고, 하루 넘게 조용해 걷으면 함을
        // 비운다(2026-10-05 사용자 결정) — 같은 이름을 나중에 받은 무관한 세션이 그 편지를 받지 않게.
        let mut back: Vec<String> = list(&mail, "boss", false).0.into_iter().map(|l| l.letter.subject).collect();
        back.sort();
        assert_eq!(back, ["long-gone", "mine", "old"]);
        for name in ["away", "quiet"] {
            assert_eq!(list(&mail, name, false).0.len(), 1, "{name} 의 편지를 되돌렸다");
        }
        assert!(list(&mail, "long-gone", false).0.is_empty(), "걷은 다른 기계의 장의 함에 편지가 남았다");
        // 다른 기계의 장에는 닻을 안 적는다(2026-10-05 사용자 결정) — 여기서 도는 것은 그 프로세스가 아니다. 적으면 낡은
        // 장이 이 기계의 `inbox --as` 나 `MOAI_AGENT` 창 덕에 영영 산 것으로 읽힌다.
        keep_alive(&agents, |p| p.name == "quiet");
        let (left, _) = presences(&agents);
        let quiet = left.iter().find(|p| p.name == "quiet").unwrap();
        assert!(quiet.gone(), "다른 기계의 낡은 장에 닻을 적어 살렸다 — {quiet:?}");
        // 다른 기계의 에이전트는 못 깨운다 — 그 tmux 칸 id 는 이 기계의 서버에서 남의 칸이다.
        let far = Presence {
            vendor: "codex".into(),
            tmux_pane: Some("%1".into()),
            tmux_socket: Some(s.path().join("no-such-socket").display().to_string()),
            ..by("away")
        };
        let woke = wake(&far);
        assert_eq!((woke.via, woke.done, woke.why), ("none", false, Some("no_way")), "다른 기계의 칸에 글자를 쳤다");
        // pid 를 적는 자리는 기계와 호스트 이름을 함께 적는다. pid 를 모르면 둘 다 비운다.
        let placed = codex_card("p").at(me.pid, me.start);
        assert_eq!(placed.machine.as_deref(), Some(here.as_str()));
        assert!(placed.host.is_some() && placed.here() && placed.runs_as(&me));
        let unknown = placed.at(0, None);
        assert_eq!((unknown.machine, unknown.host), (None, None));
        // 기계를 안 적은 옛 장은 그 pid 가 이 기계에 살아 있을 때만 기계를 단다 — 죽은 장·pid 를 모르는 장·남의 기계의 장은
        // 그대로다.
        let legacy = Presence { pid: me.pid, pid_start: me.start, ..codex_card("legacy") };
        assert_eq!(legacy.claimed().machine.as_deref(), Some(here.as_str()), "산 옛 장에 기계를 안 달았다");
        assert_eq!(by("old").claimed().machine, None, "죽은 옛 장에 기계를 달았다");
        assert_eq!(codex_card("cx").claimed().machine, None, "pid 를 모르는 장에 기계를 달았다");
        assert_eq!(by("away").claimed().machine.as_deref(), Some(there), "남의 기계의 장을 이 기계로 고쳐 적었다");
    }

    /// **지은 이름은 다른 기계의 조용한 장이 하루 쥔다**(moai-nas5, 2026-10-05 사용자 결정) — 보이는 상태([`Presence::gone`])
    /// 와 이름을 놓는 때를 가른다. 20분 조용한 다른 기계의 장은 떠난 것으로 보이지만 그 이름은 아직 그 장의 것이고, 하루가
    /// 지나 걷힐 때 놓는다. 이 기계의 장과 Codex 의 장(pid 0)은 떠나면 놓는다.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_made_name_is_let_go_by_another_machine_only_after_a_day() {
        let now = crate::model::parse_rfc3339(&crate::model::now()).unwrap();
        let ago = |secs: i64| crate::model::format_rfc3339(now - secs);
        let session = "01a107b4-6b9e-7c3d-8a21-5f0e9d4c3b2a";
        let far = |secs: i64| Presence {
            vendor: "claude".into(),
            pid: 4_194_400,
            pid_start: Some(1),
            machine: Some("00000000-0000-0000-0000-000000000000/4026532999".into()),
            since: ago(secs),
            seen: Some(ago(secs)),
            ..codex_card("codex-01a107b4")
        };
        let quiet = far(STALE_AFTER + 1);
        assert!(!quiet.here() && quiet.gone(), "시험의 장이 떠난 다른 기계의 장이 아니다");
        assert!(quiet.holds_made_name(), "20분 조용한 다른 기계의 장이 지은 이름을 놓았다");
        assert!(far(60).holds_made_name());
        assert!(!far(EXPIRE_AFTER + 1).holds_made_name(), "하루 넘게 조용한 다른 기계의 장이 이름을 쥐었다");
        let codex = Presence { since: ago(STALE_AFTER + 1), seen: Some(ago(STALE_AFTER + 1)), ..codex_card("cx") };
        assert!(codex.here() && codex.gone() && !codex.holds_made_name(), "떠난 Codex 장이 이름을 쥐었다");
        // 이 기계의 장은 프로세스로 잰다 — 산 동안은 닻이 아무리 묵어도 쥐고, 죽었으면 곧 놓는다. pid 를 아는 장을 다른
        // 기계의 장처럼 닻으로 재면 죽은 장이 이름을 영영 쥐어(닻을 안 보니 하루도 안 찬다) 그 이름을 받는 새 세션마다 토막이
        // 붙는다(리뷰 moai-nas5.cn7).
        let me = proc_of(std::process::id()).unwrap();
        let near = |pid: u32, start: Option<u64>| far(EXPIRE_AFTER + 1).at(pid, start);
        assert!(
            near(me.pid, me.start).here() && near(me.pid, me.start).holds_made_name(),
            "이 기계의 산 장이 이름을 놓았다"
        );
        assert!(!near(4_194_400, Some(1)).holds_made_name(), "이 기계의 죽은 장이 지은 이름을 쥐었다");
        // `codex_name` 도 [`made_name`] 으로 가른다 — 기계를 적은 다른 기계의 장(창이 `codex-…` 를 댄 Claude 장 같은)이 쥔
        // 이름은 하루 동안 비켜 간다. Codex 의 장은 기계를 안 적어 위의 `codex` 처럼 20분이면 놓는다.
        assert_eq!(codex_name(std::slice::from_ref(&quiet), session).as_deref(), Some("codex-01a107b4-01a107b4"));
        assert_eq!(codex_name(&[far(EXPIRE_AFTER + 1)], session).as_deref(), Some("codex-01a107b4"));
    }

    /// **깨울 일꾼은 이 기계의 것부터 고른다**(moai-dhxm) — 다른 기계의 장은 못 깨운다([`wake`] 의 `no_way`). 가장 오래 논
    /// 일꾼이 다른 기계에 있으면 그를 골라 아무도 안 두드리던 판은, 이 기계에서 노는 일꾼을 두고 열린 편지를 세워 두었다.
    /// 이 기계에 노는 일꾼이 없으면 다른 기계의 일꾼을 낸다 — 못 깨워도 누가 노는지는 댄다.
    #[test]
    fn the_worker_to_wake_is_one_on_this_machine_first() {
        let now = crate::model::parse_rfc3339(&crate::model::now()).unwrap();
        let ago = |secs: i64| crate::model::format_rfc3339(now - secs);
        let far = Presence {
            pid: 4_194_400,
            pid_start: Some(1),
            machine: Some("00000000-0000-0000-0000-000000000000/4026532999".into()),
            since: ago(600),
            seen: Some(ago(30)),
            ..codex_card("far")
        };
        let near = Presence { since: ago(60), seen: Some(ago(30)), ..codex_card("near") };
        assert!(!far.here() && !far.gone() && near.here() && !near.gone(), "시험의 장이 뜻한 대로가 아니다");
        let picked = |all: &[Presence]| idle_worker(all, "boss").map(|p| p.name.clone());
        assert_eq!(picked(&[far.clone(), near]).as_deref(), Some("near"), "못 깨우는 다른 기계의 일꾼을 골랐다");
        assert_eq!(picked(std::slice::from_ref(&far)).as_deref(), Some("far"));
        assert_eq!(wake(&far).why, Some("no_way"));
    }

    /// **닻은 모든 장에 적는다**(moai-dhxm) — 프로세스를 아는 장도 다른 기계에서는 닻으로만 잰다. 때는 [`SEEN_EVERY`] 다.
    #[test]
    fn every_card_takes_an_anchor() {
        let mut card = Presence { pid: 4_194_400, pid_start: Some(1), ..codex_card("w") };
        assert!(card.due("2026-10-04T06:12:03Z"), "닻이 없는 장을 적을 때로 안 읽었다");
        card.stamp("2026-10-04T06:12:03Z");
        assert_eq!(card.seen.as_deref(), Some("2026-10-04T06:12:03Z"), "프로세스를 아는 장에 닻을 안 적었다");
        assert!(!card.due("2026-10-04T06:13:02Z") && card.due("2026-10-04T06:13:03Z"));
    }

    /// 디렉터리는 제 무시를 든다 — `init` 을 다시 안 친 저장소에서도 `git add -A` 가 안 담는다.
    #[test]
    fn the_mailbox_ignores_itself() {
        let s = Scratch::new("mail-ignore");
        let dir = s.path().join("mail");
        send(&dir, &letter("b", "a", "x")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(".gitignore")).unwrap(), "*\n");
    }

    /// 그 디렉터리의 모든 이름 — 숨은 것(`.gitignore`·temp)까지. 없으면 비었다.
    fn everything_in(dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut out: Vec<String> = entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        out.sort();
        out
    }

    /// 쓰다 진 까닭이 체크아웃 밖으로 풀린 디렉터리인가 — `send`·`hello` 가 고른 말로 펴는 그 자료다([`refusal`]).
    fn fenced(e: &std::io::Error) -> bool {
        Fenced::of(e).is_some()
    }

    /// 못 읽은 것이 꼭 하나, 그 함(`held`, 우편함 자체면 `None`)을 못 연 것인가.
    fn fenced_box(bad: &[Garbled], held: Option<&str>) -> bool {
        matches!(bad, [g] if matches!(g.why, Why::Fenced(_)) && g.mailbox.as_deref() == held)
    }

    /// 그 시각(초)에 손을 댄 것으로 적는다 — 걷기가 재는 수정 시각이다.
    fn touched_at(path: &Path, secs: u64) {
        let at = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    /// **우편함과 출석부는 체크아웃 밖으로 풀리는 링크를 안 따른다**(moai-kxkw.7ky) — 받은 저장소가 커밋한 링크 하나로
    /// `moai hello` 가 밖에 `.gitignore`(`*`)와 장을 쓰고, `moai agents` 는 밖에 심어 둔 장을 지웠다(리뷰 moai-ml0d.que).
    /// 링크가 서는 자리마다 잰다 — `.moai` 자체(훅은 `Repo` 없이 이 자리를 바로 짓는다), 우편함·출석부, 받는 이의 함,
    /// 그 함의 `read/`, 옛 꼴의 `read/`. 쓰기는 [`Fenced`] 로 멈추고, 읽기는 그 자리를 못 읽은 것으로 대며, 걷기와 옮기기는
    /// 아무것도 안 한다 — 밖의 것을 안으로도, 안의 것을 밖으로도 안 옮긴다. 자리마다 그 울타리 하나를 걷으면 붉어지게
    /// 짰다(리뷰 moai-kxkw.k2f).
    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_checkout_is_followed_nowhere() {
        let s = Scratch::new("mail-fence");
        let away = Scratch::new("mail-fence-away");
        let link = |to: &Path, at: &Path| std::os::unix::fs::symlink(to, at).unwrap();
        let (agents, mail) = (crate::store::agents_at(s.path()), crate::store::mail_at(s.path()));

        // `.moai` 자체가 밖을 가리킨다 — 아직 없는 `.moai/agents` 를 지으며 그 링크를 따라가던 자리다.
        link(away.path(), &s.join(".moai"));
        assert!(fenced(&write_presence(&agents, &codex_card("w1")).unwrap_err()), "출석을 밖에 쓰려 했다");
        assert!(fenced(&send(&mail, &letter("w1", "boss", "x")).unwrap_err()), "편지를 밖에 쓰려 했다");
        assert_eq!(everything_in(away.path()), Vec::<String>::new(), "체크아웃 밖에 지었다");
        std::fs::remove_file(s.join(".moai")).unwrap();

        // 출석부가 밖을 가리킨다 — 밖에 심어 둔 죽은 pid 의 장은 이 저장소의 에이전트가 아니고, 걷을 것도 아니다. 밖의 오랜
        // temp 도 걷지 않는다.
        std::fs::create_dir_all(s.join(".moai")).unwrap();
        let planted = away.join("planted.json");
        std::fs::write(
            &planted,
            format!("{{\"v\":1,\"name\":\"planted\",\"pid\":{},\"pid_start\":1}}\n", std::process::id()),
        )
        .unwrap();
        let temp = away.join(".tmp.1.2");
        std::fs::write(&temp, "{").unwrap();
        touched_at(&temp, 1);
        link(away.path(), &agents);
        assert!(fenced(&write_presence(&agents, &codex_card("w1")).unwrap_err()));
        let (seen, bad) = presences(&agents);
        assert!(seen.is_empty(), "밖에 선 장을 이 저장소의 에이전트로 읽었다");
        assert!(fenced_box(&bad, None) && roster_fenced(&bad).is_some(), "못 연 출석부를 안 댔다 — {bad:?}");
        assert!(sweep(&agents, &mail).is_empty() && planted.exists() && temp.exists(), "밖의 것을 걷었다");
        assert!(fenced(&forget(&agents, "planted").unwrap_err()) && planted.exists(), "밖의 장을 지웠다");
        assert!(!same_card(&agents, "planted", "planted"), "밖의 장을 열어 견줬다");
        assert_eq!(everything_in(away.path()), [".tmp.1.2", "planted.json"], "체크아웃 밖에 썼다");
        std::fs::remove_file(&agents).unwrap();

        // 우편함이 밖을 가리킨다 — 읽기는 우편함 하나를 못 읽은 것으로 대고, 옮기기는 아무것도 안 한다.
        std::fs::create_dir_all(away.join("w1")).unwrap();
        let outside_letter = away.join("w1/20261004-061203-00000001.json");
        std::fs::write(&outside_letter, serde_json::to_string(&letter("w1", "boss", "밖")).unwrap()).unwrap();
        link(away.path(), &mail);
        assert!(fenced(&send(&mail, &letter("w1", "boss", "x")).unwrap_err()));
        let (got, bad) = list(&mail, "w1", true);
        assert!(got.is_empty(), "밖에 선 편지를 읽었다");
        assert!(fenced_box(&bad, None), "못 연 우편함을 안 댔다 — {bad:?}");
        assert_eq!(retire(&mail, "w1"), 0);
        carry(&mail, "w1", "w2");
        migrate(&mail);
        assert_eq!(sweep_read(&mail, 1), 0);
        assert!(outside_letter.exists() && !away.join("w2").exists(), "밖의 편지를 옮겼다");
        std::fs::remove_file(&mail).unwrap();

        // 받는 이의 함 하나가 밖을 가리킨다 — 우편함은 안에 서도 그 함마다 잰다. 되돌리기·따라가기는 밖의 편지를 안으로
        // 들이지 않고, 옛 꼴의 읽은 편지는 밖의 함으로 내보내지 않는다.
        std::fs::create_dir_all(&mail).unwrap();
        link(&away.join("w1"), &mail.join("w1"));
        assert!(fenced(&send(&mail, &letter("w1", "boss", "x")).unwrap_err()));
        let (got, bad) = list(&mail, "w1", false);
        assert!(got.is_empty() && fenced_box(&bad, Some("w1")), "{bad:?}");
        let stored = Stored {
            id: "20261004-061203-00000001".into(),
            mailbox: "w1".into(),
            reader: None,
            returned: false,
            letter: letter("w1", "boss", "밖"),
        };
        assert!(
            fenced(&take(&mail, &stored, "w1").unwrap_err()) && outside_letter.exists(),
            "밖의 편지를 읽음으로 옮겼다"
        );
        take_over(&mail, &[], "w1", None);
        assert_eq!(retire(&mail, "w1"), 0, "밖의 함의 편지를 안의 함으로 되돌렸다");
        carry(&mail, "w1", "w3");
        let legacy = mail.join("read/20261004-061203-00000002@w1.json");
        std::fs::create_dir_all(mail.join("read")).unwrap();
        std::fs::write(&legacy, serde_json::to_string(&letter("w1", "boss", "옛")).unwrap()).unwrap();
        migrate(&mail);
        assert!(legacy.exists(), "옛 꼴의 읽은 편지를 밖의 함으로 내보냈다");
        std::fs::remove_dir_all(mail.join("read")).unwrap();
        assert_eq!(everything_in(&away.join("w1")), ["20261004-061203-00000001.json"], "밖의 함을 건드렸다");
        assert!(!mail.join("w3").exists() && !mail.join("boss").exists(), "밖의 편지를 안으로 들였다");

        // 함은 안에 서고 그 `read/` 가 밖을 가리킨다 — [`take`] 가 못 옮기니 그 함은 하나로 대고 편지를 내놓지 않는다(내놓던
        // 판은 `inbox --ack` 가 부를 때마다 같은 편지를 새로 받은 것으로 냈다). 편지는 제 함에 남고, 밖의 함으로 따라가지
        // 않으며, 밖에 선 편지 꼴의 파일은 읽은 편지로 읽지도 걷지도 않는다.
        let id = send(&mail, &letter("w2", "boss", "안")).unwrap();
        let (got, _) = list(&mail, "w2", false);
        let old_read = away.join(format!("{id}@w2.json"));
        std::fs::write(&old_read, serde_json::to_string(&letter("w2", "boss", "밖")).unwrap()).unwrap();
        touched_at(&old_read, 1);
        link(away.path(), &mail.join("w2/read"));
        assert!(fenced(&take(&mail, &got[0], "w2").unwrap_err()));
        for read_too in [false, true] {
            let (got, bad) = list(&mail, "w2", read_too);
            assert!(got.is_empty() && fenced_box(&bad, Some("w2")), "read_too={read_too}: {got:?} {bad:?}");
        }
        carry(&mail, "w2", "w1");
        assert!(mail.join(format!("w2/{id}.json")).exists(), "읽음으로 옮기다 편지를 잃었다");
        assert_eq!(
            everything_in(&away.join("w1")),
            ["20261004-061203-00000001.json"],
            "안의 편지를 밖의 함으로 옮겼다"
        );
        assert_eq!(sweep_read(&mail, 1), 0);
        assert!(old_read.exists(), "밖에 선 파일을 읽은 편지로 걷었다");
        std::fs::remove_file(mail.join("w2/read")).unwrap();

        // 옛 꼴의 `read/`(한 함에 모두 두던 판)가 밖을 가리킨다 — 밖의 파일을 안의 함으로 들이지 않는다.
        let legacy_out = away.join("20261004-061203-00000003@w3.json");
        std::fs::write(&legacy_out, serde_json::to_string(&letter("w3", "boss", "밖")).unwrap()).unwrap();
        link(away.path(), &mail.join("read"));
        migrate(&mail);
        assert!(legacy_out.exists() && !mail.join("w3").exists(), "밖의 옛 읽은 편지를 안으로 옮겼다");
    }

    /// **읽은 때는 옮기기 앞에 적고, 임자는 쓰기 권한 없이도 적는다**(리뷰 moai-kxkw.k2f) — 쓰기로 열어 적던 판은 `0444`
    /// 편지에서 조용히 져, 읽은 편지가 보낸 때로 재여 다음 걷기에 걷혔다.
    #[cfg(unix)]
    #[test]
    fn a_letter_the_reader_cannot_write_is_still_stamped_read() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("mail-stamp-ro");
        let mail = crate::store::mail_at(s.path());
        let id = send(&mail, &letter("w1", "boss", "x")).unwrap();
        let at = mail.join(format!("w1/{id}.json"));
        touched_at(&at, 1);
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o444)).unwrap();
        let (got, _) = list(&mail, "w1", false);
        assert_eq!(take(&mail, &got[0], "w1").unwrap(), Took::Mine);
        let read = std::fs::metadata(mail.join(format!("w1/read/{id}@w1.json"))).unwrap().modified().unwrap();
        let day = std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400);
        assert!(read > day, "읽은 때를 못 적어 보낸 때가 남았다 — {read:?}");
    }

    /// **보낸 때가 금보다 늦은 편지는 이름만 보고 남긴다** — 읽은 때는 보낸 때보다 늦다. 앞날로 하루를 넘는 이름은 믿지
    /// 않고 수정 시각으로 잰다(손으로 놓은 파일).
    #[test]
    fn a_letter_sent_after_the_cutoff_is_kept_by_its_name_alone() {
        let s = Scratch::new("mail-sweep-name");
        let mail = crate::store::mail_at(s.path());
        let read = mail.join("w1/read");
        std::fs::create_dir_all(&read).unwrap();
        let now = crate::model::parse_rfc3339(&crate::model::now()).unwrap();
        let named = |secs: i64| read.join(format!("{}@w1.json", mint(&crate::model::format_rfc3339(secs), 1)));
        let (recent, far) = (named(now - 3600), named(now + 400 * 86_400));
        for path in [&recent, &far] {
            std::fs::write(path, "{}").unwrap();
            touched_at(path, 1);
        }
        assert_eq!(sweep_read(&mail, 7), 1);
        assert!(recent.exists(), "보낸 지 한 시간 된 편지를 걷었다");
        assert!(!far.exists(), "먼 앞날의 이름을 믿고 남겼다");
    }

    /// **안을 가리키는 링크는 따른다** — 저장소가 든 다른 파일과 같은 자다(`store::write_atomic_inside`). 끝이 아직 없는
    /// 링크면 그 끝에 디렉터리를 짓는다.
    #[cfg(unix)]
    #[test]
    fn a_link_inside_the_checkout_is_followed() {
        let s = Scratch::new("mail-fence-in");
        let mail = crate::store::mail_at(s.path());
        std::fs::create_dir_all(&mail).unwrap();
        std::fs::create_dir_all(s.join("data")).unwrap();
        std::os::unix::fs::symlink("../../data/w1", mail.join("w1")).unwrap();
        let id = send(&mail, &letter("w1", "boss", "안")).unwrap();
        assert!(s.join(format!("data/w1/{id}.json")).exists(), "안을 가리키는 함에 안 들였다");
        let (got, bad) = list(&mail, "w1", false);
        assert!(bad.is_empty() && got.len() == 1, "{bad:?}");
        assert_eq!(take(&mail, &got[0], "w1").unwrap(), Took::Mine);
        assert!(s.join(format!("data/w1/read/{id}@w1.json")).exists());
    }

    /// **출석부에 남은 temp 는 `moai agents` 의 걷기가 걷는다**(moai-kxkw.68i) — 쓰다 죽은 [`write_presence`] 가 남긴
    /// 점 파일을 아무도 안 걷었다. 10분 넘은 것만이다 — 지금 쓰는 중인 temp 는 남는다.
    #[test]
    fn the_sweep_takes_the_temps_a_dead_attendance_write_left() {
        let s = Scratch::new("mail-agent-temps");
        let (agents, mail) = (crate::store::agents_at(s.path()), crate::store::mail_at(s.path()));
        write_presence(&agents, &codex_card("w1")).unwrap();
        let (stale, fresh) = (agents.join(".tmp.1.2"), agents.join(".tmp.3.4"));
        std::fs::write(&stale, "{").unwrap();
        std::fs::write(&fresh, "{").unwrap();
        let ago = std::time::SystemTime::now() - std::time::Duration::from_secs(11 * 60);
        std::fs::File::options().write(true).open(&stale).unwrap().set_modified(ago).unwrap();
        sweep(&agents, &mail);
        assert!(!stale.exists(), "쓰다 죽은 출석의 temp 를 안 걷었다");
        assert!(fresh.exists(), "지금 쓰는 중일 수 있는 temp 를 걷었다");
        assert!(agents.join("w1.json").exists());
    }
}
