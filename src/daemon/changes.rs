//! `changes` 메서드의 몸통(moai-5n6m.2ki, moai-8ts9) — 데몬이 기억한 옛 스냅샷과 지금을 줄마다 견주어, 클라이언트의
//! 커서 뒤로 바뀐 id 와 사라진 id 를 낸다.
//!
//! 견주는 계산은 순수 함수 [`crate::daemon::diff`] 에 있다(무엇을 견주는지, 파생값이 왜 잡히는지도 거기 적혀 있다).
//! 여기는 둘이다 — **읽어 건네는** 층(목록은 읽기 입구 `cmd::request::read` 로, 노트는 요청의 통 안에서 저널로)과,
//! 연결 스레드가 나눠 쓰는 **기억**([`Memory`]).
//!
//! 메서드 표에는 `read::register` 가 단다 — 거기서 `project` 를 풀고 [`Memory`] 를 잡아 이 몸통을 부른다.
//!
//! ## 메서드 `changes`
//!
//!     params: { "project": "<q16 이 푼다>", "cursor": "<이전 답의 cursor>"? }
//!     result: { "value": { "cursor": "...", "changed": ["<id>", …], "removed": ["<id>", …], "reset": false },
//!               "messages": [...], "partial": false }
//!
//! - **"전부 읽기" 는 `show` 에 `all: true` 하나만 준 것이다**(같은 `project`, 다른 거르개·쪽 없이). `changes` 가 견주는
//!   목록이 바로 그것이다 — `reset: true` 를 받으면 클라이언트는 그 목록을 다시 읽고 이 답의 `cursor` 로 이어 간다.
//!   **전부 읽기 전에 커서를 받아 두면** 그사이의 쓰기는 다음 답에 선다(겹쳐 알릴 뿐 놓치지 않는다)
//! - `changed` 는 그 목록에 새로 섰거나 목록의 줄이 달라진 id 다 — 제 줄의 고침만이 아니라 파생값(묶음의 칸,
//!   멤버의 에픽·물려받은 마일스톤, 미룬 조상)의 바뀜도, `updated_at` 을 옛 값으로 둔 채 `git pull`·병합·손 고침으로
//!   들어온 줄도 든다. **노트가 달라진 줄도 든다** — 메모는 줄을 안 바꾸므로 저널의 `note` 줄을 id 마다 견준다
//! - `removed` 는 옛 목록에 있었고 지금 목록에 없는 id 다 — `rm` 뿐 아니라 아카이브로 옮긴 줄, done 에
//!   `archive_days` 넘게 서서 목록이 숨기게 된 줄도 든다. 전부 읽기에 안 서는 줄은 다 여기다
//! - `cursor` 는 불투명 글이다 — 안에 데몬 인스턴스의 표식과 견준 스냅샷의 지문이 든다. 못 읽거나, 다른 데몬(다시
//!   띄운 데몬 포함)의 것이거나, 기억에서 밀려난 지문이면 오류가 아니라 `reset: true` 다. `cursor` 를 안 주면 `reset:
//!   true` 와 첫 커서를 낸다. `reset` 이면 두 목록은 비었다
//! - 봉투의 `messages`·`partial` 은 읽기 메서드와 같다 — 목록을 읽다 만난 못 읽는 줄·못 읽은 저널 파일을 CLI 의 말로
//!   대고, CLI 라면 1 로 끝났을 판이면 `partial` 이다. **못 읽은 저널 파일은 `reset` 을 세우지 않는다** — 다시 읽어도
//!   그 파일은 못 읽으니 `reset` 은 아무것도 되찾지 못한다. 그 파일의 노트는 읽을 수 있게 될 때까지 안 서고, 읽히는
//!   판에 그 id 가 `changed` 에 선다. 덜 읽은 목록도 기억한다 — 클라이언트의 전부 읽기도 같은 목록을 받는다
//!
//! ## 기억
//!
//! 프로젝트(푼 뿌리)마다 최근 스냅샷 [`KEEP`] 벌을, 모든 프로젝트를 합쳐 어림 [`BOUND`] 바이트까지 든다. 넘치면 가장
//! 오래 안 쓴 것부터 밀어낸다(LRU — 커서로 찾은 것과 새로 넣은 것이 "썼다" 다). 방금 넣은 것은 밀어내지 않는다 —
//! 그것을 가리키는 커서를 막 내준 참이다. 기억은 프로세스의 것이라 데몬을 다시 띄우면 비고, 옛 커서는 인스턴스
//! 표식이 달라 `reset` 이다(2026-11-04 사람이 받아들인 대가). 스냅샷에도 저널에도 아무것도 안 쓴다.

use super::rpc::{Reply, RpcError};
use crate::cmd::request::Request;
use crate::daemon::diff::{Changes, Seen};
use crate::i18n::Lang;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 프로젝트 하나에 기억하는 스냅샷 수. 커서를 들고 띄엄띄엄 묻는 클라이언트가 그사이 이만큼 넘게 바뀌면 `reset`
/// 이다 — 전부 한 번 다시 읽는 값이라 크게 둘 까닭이 없다.
pub const KEEP: usize = 8;

/// 모든 프로젝트를 합친 기억의 어림 상한(바이트, [`Seen::weight`]). 줄 1만 개짜리 트래커의 스냅샷 하나가 어림 2MB 다.
pub const BOUND: usize = 32 << 20;

/// 커서의 판. 못 알아보는 판은 `reset` 이다.
const VERSION: &str = "2";

/// 연결 스레드가 나눠 쓰는 기억 — 데몬 하나에 하나다(`read::register` 가 짓는다). 복제는 같은 기억을 가리킨다.
#[derive(Clone)]
pub(crate) struct Memory {
    /// 이 데몬 인스턴스의 표식 — 띄울 때마다 새로 뽑는다. 다른 데몬이 낸 커서를 가른다.
    instance: Arc<str>,
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    keep: usize,
    bound: usize,
    /// 쓴 차례를 세는 수 — 작을수록 오래 안 썼다.
    tick: u64,
    bytes: usize,
    projects: HashMap<PathBuf, Vec<Kept>>,
}

struct Kept {
    fingerprint: String,
    seen: Arc<Seen>,
    weight: usize,
    used: u64,
}

impl Memory {
    pub(crate) fn new() -> Memory {
        Memory::with(KEEP, BOUND)
    }

    fn with(keep: usize, bound: usize) -> Memory {
        let inner = Inner { keep, bound, tick: 0, bytes: 0, projects: HashMap::new() };
        Memory { instance: instance_mark().into(), inner: Arc::new(Mutex::new(inner)) }
    }

    /// 커서가 가리키던 옛 스냅샷을 찾고, 지금 것을 넣는다. 락 안에서는 맵만 만진다 — 읽기와 견줌은 밖에서 한다.
    fn swap(&self, project: &Path, asked: Option<&str>, fingerprint: &str, seen: &Arc<Seen>) -> Option<Arc<Seen>> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let inner = &mut *g;
        inner.tick += 1;
        let tick = inner.tick;
        let list = inner.projects.entry(project.to_path_buf()).or_default();
        let old = asked.and_then(|f| list.iter_mut().find(|k| k.fingerprint == f)).map(|k| {
            k.used = tick;
            k.seen.clone()
        });
        inner.tick += 1;
        let now = inner.tick;
        match list.iter_mut().find(|k| k.fingerprint == fingerprint) {
            Some(k) => k.used = now,
            None => {
                let weight = seen.weight();
                list.push(Kept { fingerprint: fingerprint.to_string(), seen: seen.clone(), weight, used: now });
                inner.bytes += weight;
            }
        }
        inner.evict(project, now);
        old
    }

    #[cfg(test)]
    fn held(&self, project: &Path) -> usize {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.projects.get(project).map_or(0, Vec::len)
    }
}

impl Inner {
    /// 이 프로젝트가 [`KEEP`] 벌을 넘으면 그 안에서, 모두가 상한을 넘으면 모두에서 가장 오래 안 쓴 것을 밀어낸다.
    /// `fresh`(방금 넣거나 쓴 것)는 남긴다.
    fn evict(&mut self, project: &Path, fresh: u64) {
        while let Some(list) = self.projects.get_mut(project)
            && list.len() > self.keep
        {
            let Some(at) = oldest(list, fresh) else { break };
            self.bytes -= list.remove(at).weight;
        }
        while self.bytes > self.bound {
            let victim = self
                .projects
                .iter()
                .filter_map(|(p, list)| oldest(list, fresh).map(|at| (list[at].used, p.clone(), at)))
                .min_by_key(|(used, ..)| *used);
            let Some((_, p, at)) = victim else { break };
            let list = self.projects.get_mut(&p).expect("방금 찾은 프로젝트다");
            self.bytes -= list.remove(at).weight;
            if list.is_empty() {
                self.projects.remove(&p);
            }
        }
    }
}

fn oldest(list: &[Kept], fresh: u64) -> Option<usize> {
    list.iter().enumerate().filter(|(_, k)| k.used != fresh).min_by_key(|(_, k)| k.used).map(|(at, _)| at)
}

/// 인스턴스 표식 — 운영체제의 난수 12바이트. 난수를 못 뽑으면 pid 와 띄운 때(나노초)로 갈음한다.
fn instance_mark() -> String {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use ring::rand::SecureRandom as _;
    let mut bytes = [0u8; 12];
    if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
        let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        bytes[..4].copy_from_slice(&std::process::id().to_le_bytes());
        bytes[4..].copy_from_slice(&(ns as u64).to_le_bytes());
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

/// 커서 글 — `<판>.<인스턴스>.<지문>`. 둘 다 base64url 이라 `.` 가 안 든다.
fn encode(instance: &str, fingerprint: &str) -> String {
    format!("{VERSION}.{instance}.{fingerprint}")
}

/// 이 데몬이 낸 커서면 그 지문. 못 읽거나, 다른 판이거나, 다른 인스턴스의 것이면 `None` — `reset` 이다.
fn decode<'c>(cursor: &'c str, instance: &str) -> Option<&'c str> {
    let mut parts = cursor.split('.');
    let (v, i, f) = (parts.next()?, parts.next()?, parts.next()?);
    (parts.next().is_none() && v == VERSION && i == instance && !f.is_empty()).then_some(f)
}

/// 메서드 몸통 — `whole` 은 부르는 쪽이 지은 "전부 읽기"(`show` 에 `all: true`)의 요청, `lang` 은 요청의 화면 말이다.
/// 읽기 메서드와 같은 봉투(`{value, messages, partial}`)로 낸다.
pub(crate) fn handle(memory: &Memory, whole: Request, lang: Lang, cursor: Option<&str>) -> Result<Reply, RpcError> {
    let polled = poll(memory, whole, lang, cursor)?;
    super::read::envelope(&polled.changes, &polled.messages, polled.partial)
}

/// 한 번의 폴링 — 순수 함수의 답과, 읽다가 곁에 남은 것(CLI 라면 stderr 줄과 종료 코드 1).
pub(crate) struct Polled {
    pub changes: Changes,
    pub messages: Vec<String>,
    pub partial: bool,
}

/// 목록과 노트를 읽어 [`Seen`] 을 짓고(락 밖), 기억과 바꿔 끼운 뒤(락 안), 견준다(락 밖).
pub(crate) fn poll(memory: &Memory, whole: Request, lang: Lang, cursor: Option<&str>) -> Result<Polled, RpcError> {
    let project = std::fs::canonicalize(&whole.root).unwrap_or_else(|_| whole.root.clone());
    let root = whole.root.clone();
    let answer = crate::cmd::request::read(whole);
    let lines = match answer.out {
        Ok(lines) => lines,
        Err(fail) => {
            let mut error = RpcError::moai(fail.code, fail.message);
            if !answer.messages.is_empty() {
                error.data = Some(serde_json::json!({ "code": fail.code, "messages": answer.messages }));
            }
            return Err(error);
        }
    };
    let mut rows = Vec::new();
    for line in &lines {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(items)) => rows.extend(items),
            Ok(other) => rows.push(other),
            Err(e) => return Err(RpcError::new(super::rpc::code::INTERNAL, format!("show printed no JSON: {e}"))),
        }
    }
    let read = notes(&root, lang)?;
    let mut messages = answer.messages;
    for line in read.messages {
        if !messages.contains(&line) {
            messages.push(line);
        }
    }

    let seen = Arc::new(Seen::of(&rows, &read.notes));
    let fingerprint = seen.fingerprint();
    let asked = cursor.and_then(|c| decode(c, &memory.instance));
    let old = memory.swap(&project, asked, &fingerprint, &seen);
    let changes = crate::daemon::diff::changes(old.as_deref(), &seen, encode(&memory.instance, &fingerprint));
    Ok(Polled { changes, messages, partial: answer.partial || read.partial })
}

/// 저널의 `note` 줄과, 읽다가 곁에 남은 것.
struct Notes {
    notes: Vec<crate::model::JournalEntry>,
    messages: Vec<String>,
    partial: bool,
}

/// 요청의 통([`crate::side`])을 걸고 `note` 줄을 읽는다 — 못 읽은 저널 파일이 프로세스 전역으로 새지 않게.
/// 프로세스의 현재 디렉터리는 옮기지 않는다(`cmd::request::read` 와 같은 약속).
///
/// **못 푼 조각은 버린다** — id 를 못 읽은 조각은 가리킬 줄이 없다. 덧붙이는 도중에 반쯤 보인 줄이었다면 다음
/// 폴링에 온전한 줄로 읽혀 그때 선다.
fn notes(root: &Path, lang: Lang) -> Result<Notes, RpcError> {
    let side: crate::side::Shared =
        Arc::new(Mutex::new(crate::side::Side { here: root.to_path_buf(), ..crate::side::Side::default() }));
    let entered = crate::side::enter(Some(side.clone()));
    let read =
        crate::store::Repo::find(|| lang).map(|repo| repo.map(|r| (r.root.clone(), r.journal_of_kind("note").0)));
    drop(entered);
    let (repo_root, notes) = match read {
        Ok(Some(found)) => found,
        Ok(None) => {
            return Err(RpcError::moai(crate::fail::code::NOT_A_PROJECT, crate::i18n::say(lang, "refuse.not_a_repo")));
        }
        Err(f) => return Err(RpcError::moai(f.code, f.message)),
    };
    let side = std::mem::take(&mut *side.lock().unwrap_or_else(|e| e.into_inner()));
    let said = crate::unread_lines(lang, &side.unread);
    let mine = !said.is_empty() && crate::any_mine(Some(&repo_root), &side.unread);
    let messages = side
        .told
        .iter()
        .cloned()
        .chain(said.iter().map(|l| crate::warned(l)))
        .map(|l| crate::style::plain(&l))
        .collect();
    Ok(Notes { notes, messages, partial: side.partial || mine })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::process::Command;

    const ACTOR: &str = "테스터 (tester@example.com)";

    /// CLI 의 진짜 바이너리 — `cmd::request` 의 시험과 같은 길이다(`cargo test` 가 통합 시험용으로 함께 짓는다).
    fn bin() -> std::path::PathBuf {
        let exe = std::env::current_exe().unwrap();
        let bin = exe.parent().and_then(Path::parent).unwrap().join(format!("moai{}", std::env::consts::EXE_SUFFIX));
        assert!(bin.is_file(), "CLI 바이너리가 없다: {} — `cargo test` 로 돌린다", bin.display());
        bin
    }

    /// 시각 `at` 에 CLI 를 한 번 친다. 쓰기의 도장이 이 시각이 된다(`MOAI_NOW`).
    fn moai(dir: &Path, at: &str, args: &[&str]) -> String {
        let out = Command::new(bin())
            .args(["--user", ACTOR])
            .args(args)
            .current_dir(dir)
            .env("MOAI_NOW", at)
            .env("MOAI_CONFIG", dir.join("none.toml"))
            .env("HOME", dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("TZ", "UTC")
            .env("NO_COLOR", "1")
            .env_remove("MOAI_ACTOR")
            .env_remove("MOAI_HERE")
            .output()
            .unwrap();
        assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    /// "전부 읽기" 의 요청 — 메서드가 짓는 것과 같다(`read::whole`). 사람의 설정은 안 읽는다(빈 설정을 건넨다).
    fn whole(root: &Path) -> Request {
        Request {
            root: root.to_path_buf(),
            read: super::super::read::whole().unwrap(),
            actor: Some(ACTOR.to_string()),
            lang: Some(Lang::En),
            zone: None,
            reg: Some(crate::user_config::Registry::default()),
        }
    }

    /// `changes` 한 번 — `(커서, 바뀜, 사라짐, reset)`.
    fn ask(memory: &Memory, root: &Path, cursor: Option<&str>) -> (String, Vec<String>, Vec<String>, bool) {
        let got = poll(memory, whole(root), Lang::En, cursor).unwrap_or_else(|e| panic!("{}", e.message)).changes;
        (got.cursor, got.changed, got.removed, got.reset)
    }

    fn project(name: &str) -> (Scratch, std::path::PathBuf) {
        let dir = Scratch::new(name);
        let root = dir.path().to_path_buf();
        moai(&root, "2026-11-04T09:00:00Z", &["init", "t"]);
        (dir, root)
    }

    /// 새 줄의 id — `add --json` 의 `id`.
    fn add(root: &Path, at: &str, title: &str) -> String {
        let out = moai(root, at, &["add", title, "--json"]);
        serde_json::from_str::<Value>(&out).unwrap()["id"].as_str().unwrap().to_string()
    }

    #[test]
    fn each_write_names_exactly_its_row() {
        let (_keep, root) = project("changes-writes");
        let memory = Memory::new();
        let (a, b, c) = (
            add(&root, "2026-11-04T09:01:00Z", "하나"),
            add(&root, "2026-11-04T09:01:00Z", "둘"),
            add(&root, "2026-11-04T09:01:00Z", "셋"),
        );
        let (cursor, changed, removed, reset) = ask(&memory, &root, None);
        assert!(reset && changed.is_empty() && removed.is_empty());

        let step = |at: &str, args: &[&str], cursor: &str| {
            moai(&root, at, args);
            ask(&memory, &root, Some(cursor))
        };
        // add
        let d = add(&root, "2026-11-04T09:11:00Z", "넷");
        let (cursor, changed, removed, reset) = ask(&memory, &root, Some(&cursor));
        assert_eq!((changed, removed, reset), (vec![d.clone()], vec![], false));
        // edit
        let (cursor, changed, removed, reset) = step("2026-11-04T09:12:00Z", &["edit", &a, "--title", "고침"], &cursor);
        assert_eq!((changed, removed, reset), (vec![a.clone()], vec![], false));
        // mv
        let (cursor, changed, removed, reset) = step("2026-11-04T09:13:00Z", &["mv", &b, "in_progress"], &cursor);
        assert_eq!((changed, removed, reset), (vec![b.clone()], vec![], false));
        // note — 스냅샷의 줄은 그대로고 저널만 늘지만 그 줄을 다시 읽으라고 알린다
        let (cursor, changed, removed, reset) = step("2026-11-04T09:14:00Z", &["note", &c, "메모"], &cursor);
        assert_eq!((changed, removed, reset), (vec![c.clone()], vec![], false));
        // rm
        let (cursor, changed, removed, reset) = step("2026-11-04T09:15:00Z", &["rm", &d], &cursor);
        assert_eq!((changed, removed, reset), (vec![], vec![d.clone()], false));
        // 아무 일도 없으면 비었다
        let (_, changed, removed, reset) = ask(&memory, &root, Some(&cursor));
        assert_eq!((changed, removed, reset), (vec![], vec![], false));
    }

    #[test]
    fn two_writes_in_one_second_and_a_double_write_are_both_seen() {
        let (_keep, root) = project("changes-ties");
        let memory = Memory::new();
        let (a, b) = (add(&root, "2026-11-04T09:01:00Z", "하나"), add(&root, "2026-11-04T09:01:00Z", "둘"));
        let at = "2026-11-04T09:30:00Z";
        moai(&root, at, &["edit", &a, "--title", "첫 고침"]);
        // 커서를 같은 초에 받고, 그 초가 끝나기 전에 둘이 더 바뀐다 — 하나는 두 번째 고침이다.
        let (cursor, ..) = ask(&memory, &root, None);
        moai(&root, at, &["edit", &a, "--title", "둘째 고침"]);
        moai(&root, at, &["edit", &b, "--title", "고침"]);
        let (cursor, changed, removed, reset) = ask(&memory, &root, Some(&cursor));
        let mut want = vec![a.clone(), b.clone()];
        want.sort();
        assert_eq!((changed, removed, reset), (want, vec![], false));
        // 알린 것을 다시 묻으면 비었다.
        let (_, changed, _, reset) = ask(&memory, &root, Some(&cursor));
        assert!(changed.is_empty() && !reset);
    }

    /// 아카이브로 옮긴 줄은 `removed` 다 — 전부 읽기의 목록에서 빠진 줄이다. 기억이 없던 판은 줄 수·표식으로 이것을
    /// 재어 `reset` 했다.
    #[test]
    fn a_row_moved_to_the_archive_is_removed() {
        let (_keep, root) = project("changes-archive");
        let memory = Memory::new();
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        add(&root, "2026-11-04T09:01:00Z", "둘");
        moai(&root, "2026-11-04T09:02:00Z", &["mv", &a, "done"]);
        let (cursor, ..) = ask(&memory, &root, None);
        let out = moai(&root, "2026-12-30T00:00:01Z", &["archive"]);
        assert!(!root.join(".moai/archive").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true), "{out}");
        let (_, changed, removed, reset) = ask(&memory, &root, Some(&cursor));
        assert_eq!((changed, removed, reset), (vec![], vec![a], false));
    }

    /// **못 읽는 저널 파일은 `reset` 을 세우지 않는다** — 다시 읽어도 못 읽으니 `reset` 은 되찾는 것이 없다. 까닭은
    /// `messages` 에 선다. 늘 `reset` 이던 판은 폴링마다 클라이언트에게 전부 다시 읽게 했다(리뷰의 5).
    #[cfg(unix)]
    #[test]
    fn an_unreadable_journal_is_named_but_does_not_reset() {
        use std::os::unix::fs::PermissionsExt as _;
        let (_keep, root) = project("changes-unread");
        let memory = Memory::new();
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        let theirs = root.join(".moai/journal/other_example_com.jsonl");
        std::fs::write(&theirs, "").unwrap();
        std::fs::set_permissions(&theirs, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&theirs).is_ok() {
            return; // root 로 돌면 막을 수 없다
        }
        let (cursor, ..) = ask(&memory, &root, None);
        moai(&root, "2026-11-04T09:02:00Z", &["note", &a, "메모"]);
        let got = poll(&memory, whole(&root), Lang::En, Some(&cursor)).unwrap();
        std::fs::set_permissions(&theirs, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!((got.changes.changed, got.changes.reset), (vec![a], false));
        assert!(got.messages.iter().any(|m| m.contains("other_example_com")), "{:?}", got.messages);
    }

    /// **도장을 옛 값으로 둔 채 고친 줄도 선다** — 다른 clone 에서 한 시간 전에 고친 줄이 `git pull` 로 들어오는 것과
    /// 같다. 창(`updated_at`)으로 거르던 판은 이것을 바뀜으로도 `reset` 으로도 안 냈다(리뷰 `moai-5n6m.e9f` 의 4).
    #[test]
    fn a_row_rewritten_under_its_old_stamp_is_changed() {
        let (_keep, root) = project("changes-pull");
        let memory = Memory::new();
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        add(&root, "2026-11-04T09:01:00Z", "둘");
        let (cursor, ..) = ask(&memory, &root, None);
        let path = root.join(".moai/issues.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"하나\""));
        std::fs::write(&path, text.replace("\"하나\"", "\"남이 고침\"")).unwrap();
        let (_, changed, removed, reset) = ask(&memory, &root, Some(&cursor));
        assert_eq!((changed, removed, reset), (vec![a], vec![], false));
    }

    /// 옛 도장의 노트도 선다 — 시각의 창이 없다. 락을 오래 기다린 `note`(리뷰의 1)와 병합으로 들어온 노트가 이 꼴이다.
    #[test]
    fn a_note_with_an_old_stamp_is_seen() {
        let (_keep, root) = project("changes-old-note");
        let memory = Memory::new();
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        let (cursor, ..) = ask(&memory, &root, None);
        moai(&root, "2026-01-01T00:00:00Z", &["note", &a, "옛 도장의 메모"]);
        let (_, changed, _, reset) = ask(&memory, &root, Some(&cursor));
        assert_eq!((changed, reset), (vec![a], false));
    }

    /// 밀려난 지문, 다른 인스턴스(다시 띄운 데몬)의 커서, 못 읽는 커서는 `reset` 이다.
    #[test]
    fn an_evicted_or_foreign_or_garbled_cursor_resets() {
        let (_keep, root) = project("changes-evict");
        let memory = Memory::with(2, BOUND);
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        let (first, ..) = ask(&memory, &root, None);
        let (second, ..) = ask(&memory, &root, Some(&first));
        assert_eq!(first, second, "안 바뀐 스냅샷은 같은 지문이다");
        // 두 번 더 바뀌면 기억 둘 가운데 첫 것이 밀려난다.
        let mut cursor = first.clone();
        for (n, at) in ["2026-11-04T09:02:00Z", "2026-11-04T09:03:00Z"].iter().enumerate() {
            moai(&root, at, &["edit", &a, "--title", &format!("고침 {n}")]);
            let (next, _, _, reset) = ask(&memory, &root, Some(&cursor));
            assert!(!reset);
            cursor = next;
        }
        let canon = std::fs::canonicalize(&root).unwrap();
        assert_eq!(memory.held(&canon), 2);
        let (_, changed, removed, reset) = ask(&memory, &root, Some(&first));
        assert!(reset && changed.is_empty() && removed.is_empty(), "밀려난 지문이 reset 이 아니다");
        // 다른 데몬의 기억은 이 커서를 모른다 — 같은 스냅샷이어도.
        let other = Memory::new();
        let (_, _, _, reset) = ask(&other, &root, Some(&cursor));
        assert!(reset, "다른 인스턴스의 커서가 reset 이 아니다");
        for bad in ["", "쓰레기", "2..", "1.x.y", &format!("{cursor}.z")] {
            assert!(ask(&memory, &root, Some(bad)).3, "{bad}");
        }
        // 지금 커서는 여전히 산다.
        assert!(!ask(&memory, &root, Some(&cursor)).3);
    }

    /// 크기 상한은 모든 프로젝트를 합쳐 센다. 방금 넣은 것은 상한을 넘어도 남는다 — 그것을 가리키는 커서를 막 냈다.
    #[test]
    fn the_size_bound_evicts_the_least_recently_used() {
        let memory = Memory::with(8, 1);
        let one = Arc::new(Seen::of(&[serde_json::json!({ "id": "a-1" })], &[]));
        let two = Arc::new(Seen::of(&[serde_json::json!({ "id": "a-2" })], &[]));
        memory.swap(Path::new("/p/a"), None, &one.fingerprint(), &one);
        memory.swap(Path::new("/p/b"), None, &two.fingerprint(), &two);
        assert_eq!((memory.held(Path::new("/p/a")), memory.held(Path::new("/p/b"))), (0, 1));
    }

    #[test]
    fn a_missing_tracker_is_an_error_not_a_reset() {
        let memory = Memory::new();
        let empty = Scratch::new("changes-none");
        assert!(poll(&memory, whole(empty.path()), Lang::En, None).is_err());
    }

    /// **못 읽는 스냅샷 줄은 `show` 처럼 댄다** — 줄을 대는 말과 덜 받은 답(`partial`).
    #[test]
    fn a_torn_snapshot_line_is_named_and_partial() {
        let (_keep, root) = project("changes-torn");
        let memory = Memory::new();
        add(&root, "2026-11-04T09:01:00Z", "하나");
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().append(true).open(root.join(".moai/issues.jsonl")).unwrap();
        writeln!(f, "{{this line is torn").unwrap();
        let got = poll(&memory, whole(&root), Lang::En, None).unwrap();
        assert!(got.partial, "못 읽는 줄로 partial 이 안 섰다");
        assert!(
            got.messages.iter().any(|m| m.contains("issues.jsonl")),
            "못 읽는 줄의 자리를 안 댄다 — {:?}",
            got.messages
        );
    }
}
