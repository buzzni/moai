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
//! ## 락이 없는 까닭
//!
//! 같은 파일을 두 프로세스가 고치지 않는다. 보내기는 새 이름의 파일을 들이고, 읽기는 그 파일을 `read/`
//! 로 옮긴다. 둘 다 디렉터리 항목 하나를 바꾸는 시스템 호출 하나라 차례는 커널이 정한다.
//!
//! - **보내기**는 temp 에 다 쓴 뒤 `hard_link` 로 제 이름에 들인다([`send`]). `rename` 은 같은 이름이
//!   있으면 **말없이 덮는다** — 이름이 겹친 두 편지 중 하나가 사라지는 조용한 손실이다. `link` 는 그
//!   자리에서 `EEXIST` 로 지고, 진 쪽은 다음 이름으로 다시 든다. 반쯤 쓴 편지는 읽는 쪽에 안 보인다
//! - **읽기**는 `rename(<id>.json, read/<id>@<읽은 이>.json)` 이다([`take`]). 둘이 같은 편지를 겨루면
//!   한쪽만 이기고 다른 쪽은 `NotFound` 를 받는다 — `any-idle-worker` 편지를 먼저 가진 쪽이 그것이다
//!
//! `store::with_write` 는 안 지난다 — 그 락은 스냅샷의 것이다.
//!
//! ## 되돌릴 수 없는 것은 편지 파일의 꼴이다
//!
//! 처음부터 `v: 1` 을 적고 모르는 키는 그대로 든다([`Letter::rest`]). 편지는 한 번 쓰고 안 고치니 파일의
//! 모르는 키는 저절로 남고, `--json` 도 그것을 그대로 낸다. **편지의 id 는 파일 이름이다** — 파일 안에는
//! 안 적는다. 파생값을 저장하면 둘이 갈리는 날이 온다(CLAUDE.md "파생값은 저장하지 않는다").

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

/// 에이전트가 가질 수 있는 이름 — [`is_name`] 이고 받는 이 낱말([`ANY_IDLE_WORKER`])이 아니다.
pub fn is_agent_name(name: &str) -> bool {
    is_name(name) && name != ANY_IDLE_WORKER
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

/// 우편함에 선 편지 — 파일 이름에서 읽은 id 와, 읽었으면 읽은 이.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub id: String,
    /// 읽은 이 — 안 읽었으면 `None`. `read/<id>@<읽은 이>.json` 의 뒤쪽이다.
    pub reader: Option<String>,
    pub letter: Letter,
}

/// 못 읽은 파일 — 자리와 까닭. 막지 않고 댄다.
#[derive(Debug, Clone, PartialEq)]
pub struct Garbled {
    pub path: PathBuf,
    pub why: String,
    /// 편지로는 못 읽었지만 JSON 으로는 읽혀 알아낸 받는 이 — 모르면 `None` 이다. 남의 편지가 깨진 것을 제
    /// 답이 덜 난 것으로 세지 않으려고 든다(`inbox`).
    pub to: Option<String>,
}

/// 편지로 못 읽은 파일의 받는 이를 너그럽게 읽는다 — 키 하나의 꼴이 틀렸을 뿐 JSON 이면 `to` 는 읽힌다.
fn addressee(path: &Path) -> Option<String> {
    let v: serde_json::Value = read_json(path).ok()??;
    Some(v.get("to")?.as_str()?.to_string())
}

/// 편지를 들인다 — 새 id 를 낸다.
///
/// temp 에 다 쓰고 `sync` 한 뒤 `hard_link` 로 `<id>.json` 에 들인다. 이름이 이미 서 있으면(`EEXIST`) 다음
/// 이름으로 다시 든다 — `rename` 이었으면 그 자리의 편지를 말없이 덮었다(모듈 머리글). temp 는 들인 뒤에
/// 지운다. 반쯤 쓴 편지는 숨은 이름이라 읽는 쪽([`list`])이 안 본다.
///
/// **하드 링크를 못 거는 파일 시스템**(`EEXIST` 가 아닌 실패)에서는 그 자리가 비었을 때만 `rename` 으로
/// 물러선다 — 보는 것과 옮기는 것 사이의 틈이 남지만, 그 자리에서 보내기를 통째로 막는 것보다 낫다.
pub fn send(dir: &Path, letter: &Letter) -> std::io::Result<String> {
    send_from(dir, letter, &mut clock_micros)
}

/// [`send`] 의 몸통 — 이름을 지을 시계(`clock`)를 받는다. 시험이 이미 선 이름 위로 보내 보려고 가른다.
///
/// **이름은 들이기 바로 앞에 짓는다**(리뷰 moai-h8tn.x4l). `EEXIST` 가 지켜 주는 것은 아직 `mail/` 에 선 편지뿐이다
/// — 읽음으로 옮겨 간 편지의 이름은 비어 보인다. temp 를 쓰고 `sync` 하기 전(수~수십 ms 앞)에 지은 이름은, 같은
/// 마이크로초에 시작한 이웃이 먼저 들인 편지가 그사이 읽혀 나가면 그대로 다시 들어가 한 id 가 두 편지에 선다.
/// 둘째 편지를 같은 이가 읽으면 [`take`] 의 `rename` 이 첫 편지의 `read/` 파일을 말없이 덮는다. 들이기 바로 앞에
/// 지으면 그 틈이 시스템 호출 하나 너비로 준다. 다시 들 때는 시계가 안 갔어도 하나 올린다.
fn send_from(dir: &Path, letter: &Letter, clock: &mut dyn FnMut() -> u64) -> std::io::Result<String> {
    ensure_dir(dir)?;
    sweep_temps(dir);
    let mut text = serde_json::to_string(letter).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = temp_in(dir);
    write_new(&tmp, text.as_bytes(), true)?;
    let mut last = None;
    let mut tried: Option<u64> = None;
    for _ in 0..10_000u64 {
        let now = clock();
        let micros = tried.map_or(now, |before| now.max(before.wrapping_add(1)));
        tried = Some(micros);
        let id = mint(&letter.sent_at, micros);
        let at = dir.join(format!("{id}.json"));
        match std::fs::hard_link(&tmp, &at) {
            Ok(()) => {
                let _ = std::fs::remove_file(&tmp);
                sync_dir(dir);
                return Ok(id);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) if !at.exists() && std::fs::rename(&tmp, &at).is_ok() => {
                sync_dir(dir);
                return Ok(id);
            }
            Err(e) => {
                last = Some(e);
                break;
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(last.unwrap_or_else(|| std::io::Error::other("no free letter id")))
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

/// 우편함을 읽는다 — 안 읽은 편지와, `read_by` 를 주면 그 이가 읽은 편지까지. id 차례(보낸 차례)다.
///
/// **보통 파일만 읽는다** — `DirEntry::file_type` 은 링크를 안 따라간다. 파이프가 서 있으면 여는 자리에서
/// 멈추고, 링크는 남의 파일을 읽힌다. 숨은 이름(temp·`.gitignore`)은 안 본다. 디렉터리가 없으면 빈 우편함이다.
///
/// **읽은 편지는 파일 이름으로 먼저 거른다**(리뷰 moai-h8tn.x4l) — 읽은 이가 이름에 서 있으니, 남이 읽은 편지를
/// 열어 가를 까닭이 없다. `read/` 는 지우지 않아 쌓이기만 하고, `inbox --all --wait` 는 반 초마다 이 자리를 훑는다.
/// 읽은 이 자리가 이름의 꼴이 아니면(손으로 놓은 파일) 편지로 안 센다.
pub fn list(dir: &Path, read_by: Option<&str>) -> (Vec<Stored>, Vec<Garbled>) {
    let mut letters = Vec::new();
    let mut garbled = Vec::new();
    for (path, stem) in json_files(dir) {
        if !is_id(&stem) {
            continue;
        }
        match read_json::<Letter>(&path) {
            Ok(Some(letter)) => letters.push(Stored { id: stem, reader: None, letter }),
            Ok(None) => {}
            Err(why) => {
                let to = addressee(&path);
                garbled.push(Garbled { path, why, to })
            }
        }
    }
    if let Some(me) = read_by {
        for (path, stem) in json_files(&dir.join("read")) {
            let Some((id, reader)) = stem.split_once('@') else { continue };
            if !is_id(id) || reader != me || !is_name(reader) {
                continue;
            }
            match read_json::<Letter>(&path) {
                Ok(Some(letter)) => {
                    letters.push(Stored { id: id.to_string(), reader: Some(reader.to_string()), letter })
                }
                Ok(None) => {}
                // 읽은 이가 나인 것만 여기 온다 — 받는 이를 따로 잴 것이 없다.
                Err(why) => garbled.push(Garbled { path, why, to: None }),
            }
        }
    }
    letters.sort_by(|a, b| a.id.cmp(&b.id).then(a.reader.is_some().cmp(&b.reader.is_some())));
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

/// 이 편지가 `me` 에게 가는가 — 이름으로 받은 것, 그리고 `any-idle-worker` 편지 가운데 제가 보내지 않은
/// 것(감독은 빼고, 2026-10-04 사용자 결정). 읽은 편지는 읽은 이가 `me` 인 것만이다.
pub fn for_me(stored: &Stored, me: &str, role: &str) -> bool {
    if let Some(reader) = &stored.reader {
        return reader == me;
    }
    let letter = &stored.letter;
    letter.to == me || (letter.to == ANY_IDLE_WORKER && may_take_open(me, role, &letter.from))
}

/// `any-idle-worker` 편지를 가질 수 있는가 — 보낸 이가 아니고 감독이 아니다(2026-10-04 사용자 결정). [`for_me`] 와
/// [`idle_worker`] 가 이 하나로 잰다 — 따로 적으면 `send --wake` 가 깨운 일꾼이 정작 그 편지를 못 가지는 날이 온다.
fn may_take_open(me: &str, role: &str, from: &str) -> bool {
    from != me && role != SUPERVISOR
}

/// 편지 하나를 가졌는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// 이 프로세스가 옮겼다 — 이제 `read/<id>@<읽은 이>.json` 이다.
    Mine,
    /// 그 자리에 없었다 — 남이 먼저 가졌다(`any-idle-worker`)거나 이미 읽었다.
    Lost,
}

/// 편지를 읽음으로 옮긴다 — `rename` 하나다(모듈 머리글). 먼저 옮긴 쪽만 [`Took::Mine`] 을 받는다.
///
/// **읽고 지우는 두 걸음으로 바꾸지 않는다** — 둘이 같이 읽고 같이 지우면 한 편지가 두 세션에 실린다.
/// 겨루기 시험 둘(`tests/cli.rs` 의 `concurrent_acks_deliver_each_letter_once`·`an_open_letter_goes_to_exactly_one_worker`)이
/// 그 자리를 잡는다 — 복사하고 지우는 꼴로 바꾸어 재 보니 둘 다 붉어졌다.
///
/// `rename` 은 `read/<id>@<읽은 이>.json` 이 이미 서 있으면 **말없이 덮는다** — 그 자리가 비어 있다는 것은 id 가
/// 두 번 안 서는 것([`send_from`] 이 이름을 들이기 바로 앞에 짓는다)에 기댄다.
pub fn take(dir: &Path, id: &str, reader: &str) -> std::io::Result<Took> {
    let read = dir.join("read");
    std::fs::create_dir_all(&read)?;
    match std::fs::rename(dir.join(format!("{id}.json")), read.join(format!("{id}@{reader}.json"))) {
        Ok(()) => Ok(Took::Mine),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Took::Lost),
        Err(e) => Err(e),
    }
}

// ── 출석 ──────────────────────────────────────────────────────────────

/// 출석 한 장 — `.moai/agents/<name>.json`. 에이전트가 `moai hello` 로, 훅이 `SessionStart` 에서 쓴다.
///
/// `pid` 는 **에이전트 프로세스**다(훅의 셸이 아니다). `pid_start` 는 그 프로세스가 선 때(리눅스
/// `/proc/<pid>/stat` 의 22째 칸)라, pid 가 재사용돼도 죽은 출석이 산 것으로 안 보인다([`alive`]).
/// 없을 수 있는 키(`pid_start`·`session`·`tmux_*`)는 없으면 안 적는다 — 없음이 곧 "모른다" 다.
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
    /// 벤더의 세션 id — 훅이 이 세션의 출석을 찾는 열쇠고, Codex 를 깨울 때의 `--thread` 다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_pane: Option<String>,
    /// 그 칸이 선 tmux 서버의 소켓 — `$TMUX` 의 첫 토막. 깨울 때 `-S` 로 그 서버만 겨눈다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_socket: Option<String>,
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

impl Presence {
    /// 지금 이 프로세스가 선 tmux 칸 — `$TMUX_PANE` 과 `$TMUX` 의 소켓. 에이전트가 띄운 셸과 훅은 그
    /// 에이전트의 환경을 물려받으므로 그 칸이 곧 에이전트의 칸이다.
    pub fn tmux_here() -> (Option<String>, Option<String>) {
        let pane = std::env::var("TMUX_PANE").ok().filter(|p| !p.is_empty());
        let socket =
            std::env::var("TMUX").ok().and_then(|t| t.split(',').next().map(str::to_string)).filter(|s| !s.is_empty());
        (pane, socket)
    }

    /// 이 장이 그 프로세스의 것인가 — pid 가 같고, 선 때를 둘 다 알면 그것도 같다(pid 는 재사용된다).
    /// **"같은 에이전트인가" 는 이 하나로 잰다** — `hello`·훅·[`me_among`] 이 저마다 적던 자리다.
    pub fn runs_as(&self, p: &Proc) -> bool {
        self.pid == p.pid && (self.pid_start.is_none() || p.start.is_none() || self.pid_start == p.start)
    }

    /// 그 에이전트가 떠난 것이 확실한가 — **모르면 아니다**([`alive`] 가 `None`). 걷기·깨우기·이름 겨루기가 이
    /// 하나로 잰다.
    pub fn gone(&self) -> bool {
        alive(self.pid, self.pid_start) == Some(false)
    }
}

/// 출석 한 장의 자리 — `<출석부>/<이름>.json`. 이름이 곧 파일 이름이다.
fn card_at(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// 출석을 적는다 — temp 를 쓰고 `<name>.json` 으로 `rename` 한다. **덮는 것이 뜻이다** — 같은 이름은 같은
/// 에이전트고, 마지막에 적은 쪽이 지금이다. 반쯤 쓴 장은 안 보인다.
///
/// **`sync` 하지 않는다**(리뷰 moai-h8tn.x4l) — 훅이 프롬프트마다·턴 끝마다 이 장을 쓰는데, 저장소가 Ceph RBD 에
/// 서는 이 기계에서 `sync` 한 번이 수십 ms 다. 출석은 기록이 아니라 지금의 표라 잃어도 다음 훅이 다시 쓰고, 기계가
/// 죽은 뒤에는 장의 pid 가 모두 죽어 어차피 걷힌다. 다른 프로세스가 반쯤 쓴 장을 못 보는 것은 `rename` 이 지킨다.
pub fn write_presence(dir: &Path, presence: &Presence) -> std::io::Result<()> {
    if !is_agent_name(&presence.name) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, presence.name.clone()));
    }
    ensure_dir(dir)?;
    let mut text = serde_json::to_string(presence).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = temp_in(dir);
    write_new(&tmp, text.as_bytes(), false)?;
    std::fs::rename(&tmp, card_at(dir, &presence.name)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// 출석 한 장을 걷는다 — 이름을 바꾼 에이전트의 옛 장이나 죽은 장이다.
pub fn forget(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(card_at(dir, name))
}

/// 두 이름의 장이 **한 파일**인가 — 대소문자를 안 가리는 파일 시스템(macOS 기본)에서는 `Worker` 와 `worker` 가 한
/// 파일이다. 이름을 바꾼 뒤 옛 장을 걷을 때 묻는다: 한 파일이면 걷는 것이 방금 쓴 장을 지운다.
pub fn same_card(dir: &Path, a: &str, b: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let id = |name: &str| std::fs::metadata(card_at(dir, name)).ok().map(|m| (m.dev(), m.ino()));
        matches!((id(a), id(b)), (Some(x), Some(y)) if x == y)
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        a.eq_ignore_ascii_case(b)
    }
}

/// 출석부를 읽는다 — 이름 차례다. 산 것만 고르지 않는다: 그것은 [`alive`] 를 묻는 쪽이 한다.
pub fn presences(dir: &Path) -> (Vec<Presence>, Vec<Garbled>) {
    let mut out = Vec::new();
    let mut garbled = Vec::new();
    for (path, stem) in json_files(dir) {
        if !is_agent_name(&stem) {
            continue;
        }
        match read_json::<Presence>(&path) {
            // **파일 이름이 이름이다** — 안에 적힌 이름이 다르면 파일 쪽을 믿는다. 쓰는 쪽이 늘 같게 쓰니,
            // 다른 것은 손으로 옮긴 파일이다.
            Ok(Some(p)) => out.push(Presence { name: stem, ..p }),
            Ok(None) => {}
            Err(why) => garbled.push(Garbled { path, why, to: None }),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    (out, garbled)
}

/// 죽은 출석을 걷는다 — 걷은 이름을 낸다. **산지 모르는 것은 안 걷는다**([`alive`] 가 `None`).
///
/// 보는 것과 지우는 것 사이에 같은 이름의 새 에이전트가 장을 쓰면 그것을 지울 수 있다. 그 에이전트의 다음
/// 훅(`UserPromptSubmit`·`Stop`)이 장을 다시 쓰므로 스스로 낫는다 — 그 틈을 막으려고 락을 두지 않는다.
pub fn sweep(dir: &Path) -> Vec<String> {
    let (all, _) = presences(dir);
    all.into_iter().filter(Presence::gone).filter(|p| forget(dir, &p.name).is_ok()).map(|p| p.name).collect()
}

/// 이 프로세스의 조상 가운데 출석부에 선 에이전트 — **"나는 누구인가" 의 답이다.** 에이전트가 띄운 셸에서
/// 도는 `moai` 의 조상에 그 에이전트가 있다. 선 때까지 맞아야 한다(둘 다 알 때) — pid 는 재사용된다.
pub fn me_among<'a>(presences: &'a [Presence], ancestors: &[Proc]) -> Option<&'a Presence> {
    ancestors.iter().find_map(|a| presences.iter().find(|p| p.runs_as(a)))
}

/// 출석 하나를 깨울 수 있는 일꾼 가운데 고른다 — `any-idle-worker` 편지를 보낼 때. 산 것, 놀고 있는 것,
/// 그 편지를 가질 수 있는 것([`for_me`] 와 같은 자) 가운데 가장 오래 논 것이다.
pub fn idle_worker<'a>(presences: &'a [Presence], from: &str) -> Option<&'a Presence> {
    presences
        .iter()
        .filter(|p| p.status == IDLE && may_take_open(&p.name, &p.role, from))
        .filter(|p| !p.gone())
        .min_by(|a, b| a.since.cmp(&b.since).then(a.name.cmp(&b.name)))
}

// ── 프로세스 ──────────────────────────────────────────────────────────

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

/// Claude Code 가 그 세션에 붙인 이름 — `~/.claude/sessions/<pid>.json` 의 `name` 이다. `ListAgents` 와
/// `SendMessage` 가 쓰는 그 이름이라, 출석의 이름을 그것과 맞추면 감독이 같은 이름으로 깨운다
/// (2026-10-04 사용자 결정). **비문서 파일이다** — 못 읽으면 `None` 이고 부르는 쪽이 세션 id 로 짓는다.
///
/// **절대 경로만 본다**(리뷰 moai-h8tn.x4l) — 빈 `HOME` 은 빈 경로라 `.claude/sessions/…` 가 훅이 옮겨 간 자리, 곧
/// 세션의 저장소에서 풀린다. 그 저장소가 심어 둔 파일 하나가 에이전트의 이름을 고르게 된다.
pub fn claude_session_name(pid: u32) -> Option<String> {
    let set = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home =
        set("CLAUDE_CONFIG_DIR").or_else(|| set("HOME").map(|h| h.join(".claude"))).filter(|d| d.is_absolute())?;
    let v: serde_json::Value = read_json(&home.join("sessions").join(format!("{pid}.json"))).ok()??;
    name_from(v.get("name")?.as_str()?)
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
}

/// 벤더의 길로 깨운다(2026-10-04 사용자 결정 "벤더의 길") — Claude 는 `SendMessage`(이 CLI 가 부를 수 없어
/// 보낸 쪽이 부른다), Codex 는 `codex queue --thread`, Antigravity 는 그 tmux 칸에 [`WAKE_WORDS`] 를 친다.
/// 그 밖은 깨우지 않는다 — 다음 턴의 훅이 싣는다. **일하는 중이면 안 깨운다** — 그 턴이 끝날 때 `Stop`
/// 훅이 편지를 싣는다.
///
/// 띄우는 프로세스는 입출력을 모두 닫는다 — 물려주면 훅처럼 stdout 을 받아 두는 쪽이 EOF 를 못 받는다
/// (`skill::command` 의 "그 성질에 기대고 있다"). 10초 안에 안 끝나면 죽인다.
pub fn wake(p: &Presence) -> Woke {
    let woke = |via, done, why| Woke { to: p.name.clone(), via, done, why };
    if p.status == BUSY {
        return woke("none", false, Some("busy"));
    }
    match p.vendor.as_str() {
        "claude" => woke("send_message", false, Some("ask_sender")),
        "codex" => {
            let Some(thread) = p.session.as_deref().filter(|s| !s.is_empty()) else {
                return woke("codex", false, Some("no_session"));
            };
            let ran =
                run(std::process::Command::new("codex").args(["queue", "--thread", thread, "--message", WAKE_WORDS]));
            woke("codex", ran.is_ok(), ran.err())
        }
        "antigravity" => {
            // **소켓을 모르는 칸은 안 친다**(리뷰 moai-h8tn.x4l) — `-S` 없는 `tmux` 는 보내는 쪽의 `$TMUX`(사람의
            // 서버)나 기본 서버에 붙고, 칸 id(`%N`)는 서버마다 따로 세어 그 서버에서는 남의 칸이다. 사람의 창에
            // `moai inbox` 와 Enter 가 쳐진다.
            let (Some(pane), Some(socket)) =
                (p.tmux_pane.as_deref().filter(|s| !s.is_empty()), p.tmux_socket.as_deref().filter(|s| !s.is_empty()))
            else {
                return woke("tmux", false, Some("no_pane"));
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
        _ => woke("none", false, Some("no_way")),
    }
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

// ── 파일 ──────────────────────────────────────────────────────────────

/// 디렉터리를 짓고 **제 `.gitignore`(`*`)를 둔다.** `init` 이 `.gitignore` 에 이 자리를 더하지만, 그
/// `init` 을 아직 다시 안 친 저장소에서도 훅이 출석을 쓴다 — 거기서 `git add -A` 한 번이 pid 와 경로를
/// 커밋한다. 디렉터리가 제 무시를 들면 저장소 설정과 상관없이 git 이 안 본다(`*` 이 그 `.gitignore` 도
/// 가린다).
///
/// **빈 `.gitignore` 는 없는 것으로 친다**(리뷰 moai-h8tn.x4l) — 쓰다 기계가 죽으면 이름만 선 빈 파일이 남는데, 이름만
/// 보고 넘어가던 판은 그 디렉터리의 무시를 영영 껐다. 한 번만 쓰는 파일이라 내려 쓴다.
fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
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
    Ok(())
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

/// 죽은 보내기가 남긴 temp 를 걷는다 — 10분 넘은 것만. 보내는 쪽은 temp 를 몇 밀리초만 든다. **이름부터 본다** —
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
        let text = std::fs::read_to_string(s.path().join(format!("{id}.json"))).unwrap();
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
        std::fs::write(
            s.path().join("20261004-061203-00000001.json"),
            r#"{"v":2,"to":"b","from":"a","subject":"s","body":"","sent_at":"2026-10-04T06:12:03Z","reply_to":null,"priority":"high"}"#,
        )
        .unwrap();
        let (got, bad) = list(s.path(), None);
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
        std::fs::write(s.path().join(".tmp.1.2"), "{").unwrap();
        std::fs::write(s.path().join("notes.json"), "{}").unwrap();
        let id = send(s.path(), &letter("b", "a", "real")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(s.path().join(format!("{id}.json")), s.path().join("20261004-061203-zzzzzzzz.json"))
            .unwrap();
        let (got, bad) = list(s.path(), None);
        assert_eq!(got.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), [id.as_str()]);
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// **id 의 글자 차례가 보낸 차례다** — 같은 초 안에서도. 훅이 싣는 차례가 이것이다.
    #[test]
    fn ids_sort_in_the_order_sent() {
        let s = Scratch::new("mail-order");
        let ids: Vec<String> = (0..5).map(|n| send(s.path(), &letter("b", "a", &n.to_string())).unwrap()).collect();
        let (got, _) = list(s.path(), None);
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
            std::fs::write(s.path().join(format!("{}.json", mint(at, base + bump))), text).unwrap();
        }
        // 시계가 멈춰 있어도 다시 들 때마다 하나 올린다.
        let id = send_from(s.path(), &letter("b", "a", "mine"), &mut || base).unwrap();
        assert_eq!(id, mint(at, base + 3), "비어 있는 다음 이름으로 안 들었다");
        let (got, _) = list(s.path(), None);
        let bodies: Vec<&str> =
            got.iter().filter(|g| g.letter.subject == "squatter").map(|g| g.letter.body.as_str()).collect();
        assert_eq!(bodies, ["0", "1", "2"], "먼저 선 편지를 덮었다");
        assert!(got.iter().any(|g| g.id == id && g.letter.subject == "mine"), "새 편지가 안 섰다");
        assert!(
            !std::fs::read_dir(s.path()).unwrap().any(|e| e
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
            written =
                std::fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".tmp."));
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
        let id = send(s.path(), &letter(ANY_IDLE_WORKER, "boss", "job")).unwrap();
        assert_eq!(take(s.path(), &id, "w1").unwrap(), Took::Mine);
        assert_eq!(take(s.path(), &id, "w2").unwrap(), Took::Lost);
        let (got, _) = list(s.path(), Some("w1"));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].reader.as_deref(), Some("w1"));
        assert!(for_me(&got[0], "w1", "") && !for_me(&got[0], "w2", ""), "읽은 편지가 남의 것으로도 섰다");
    }

    /// `any-idle-worker` 는 보낸 이와 감독을 빼고 누구에게나 간다(2026-10-04 사용자 결정).
    #[test]
    fn any_idle_worker_skips_the_sender_and_supervisors() {
        let open = Stored { id: "x".into(), reader: None, letter: letter(ANY_IDLE_WORKER, "boss", "job") };
        assert!(for_me(&open, "w1", ""));
        assert!(for_me(&open, "w1", "worker"));
        assert!(!for_me(&open, "boss", ""), "보낸 이가 제 편지를 받았다");
        assert!(!for_me(&open, "other", SUPERVISOR), "감독이 일감을 가졌다");
        let named = Stored { id: "y".into(), reader: None, letter: letter("w1", "boss", "job") };
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

    /// 출석은 덮어 적고, 죽은 것만 걷는다 — 산지 모르는 것(pid 0)은 남는다.
    #[test]
    #[cfg(target_os = "linux")]
    fn the_sweep_takes_only_the_dead() {
        let s = Scratch::new("agents-sweep");
        let me = proc_of(std::process::id()).unwrap();
        let card = |name: &str, pid: u32, start: Option<u64>| Presence {
            v: VERSION,
            name: name.into(),
            vendor: "claude".into(),
            model: String::new(),
            role: String::new(),
            status: IDLE.into(),
            since: "2026-10-04T06:12:03Z".into(),
            pid,
            pid_start: start,
            session: None,
            cwd: String::new(),
            tmux_pane: None,
            tmux_socket: None,
            rest: BTreeMap::new(),
        };
        write_presence(s.path(), &card("live", me.pid, me.start)).unwrap();
        write_presence(s.path(), &card("reused", me.pid, me.start.map(|t| t + 1))).unwrap();
        write_presence(s.path(), &card("unknown", 0, None)).unwrap();
        assert_eq!(sweep(s.path()), ["reused"]);
        let (left, _) = presences(s.path());
        assert_eq!(left.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["live", "unknown"]);
        assert!(
            me_among(&left, std::slice::from_ref(&me)).is_some_and(|p| p.name == "live"),
            "조상의 pid 로 제 출석을 못 찾았다"
        );
    }

    /// 디렉터리는 제 무시를 든다 — `init` 을 다시 안 친 저장소에서도 `git add -A` 가 안 담는다.
    #[test]
    fn the_mailbox_ignores_itself() {
        let s = Scratch::new("mail-ignore");
        let dir = s.path().join("mail");
        send(&dir, &letter("b", "a", "x")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(".gitignore")).unwrap(), "*\n");
    }
}
