//! `changes` 메서드의 몸통(moai-5n6m.2ki) — 클라이언트가 건넨 커서 뒤로 바뀐 줄·사라진 줄을 id 로 낸다.
//!
//! 계산은 순수 함수 [`crate::query::changes::changes`] 에 있다(커서의 꼴·동률·`reset` 의 까닭도 거기 적혀 있다).
//! 여기는 **읽어 건네는** 얇은 층이다 — 요청의 통([`crate::side`])을 세우고 그 안에서 스냅샷과 저널을 읽는다.
//! 프로세스의 현재 디렉터리는 옮기지 않는다(`cmd::request::read` 와 같은 약속).
//!
//! **메서드 표에는 아직 안 단다** — `project` 를 푸는 자리(q16)와 함께 `methods()` 에 묶는 것은 다음 멤버의 일이다.
//!
//! ## 메서드 `changes`
//!
//!     params: { "project": "<q16 이 푼다>", "cursor": "<이전 답의 cursor>"? }
//!     result: { "cursor": "...", "changed": ["<id>", …], "removed": ["<id>", …], "reset": false }
//!
//! - `cursor` 는 불투명 글이다. 못 읽거나 낡았거나 다른 프로젝트의 것이면 오류가 아니라 `reset: true` 다
//! - `reset: true` 면 두 목록은 비었다 — 전체를 다시 읽고 이 답의 `cursor` 로 이어 간다. **전체를 읽기 전에 커서를
//!   받아 두면** 그사이의 쓰기는 다음 답에 선다(겹쳐 알릴 뿐 놓치지 않는다)
//! - `cursor` 를 안 주면 `reset: true` 와 첫 커서를 낸다
//! - `changed` 는 새로 생겼거나 고쳤거나 **메모가 붙은** 산 줄이다. 메모는 스냅샷의 줄을 안 바꾸므로 저널의 `note`
//!   줄로 찾는다 — 그 줄의 내용을 읽지 않고 id 만 쓴다. `removed` 는 `rm` 이 지웠고 지금 없는 id 다
#![cfg_attr(not(test), expect(dead_code, reason = "읽기 메서드와 project 푸는 자리가 묶는다(moai-5n6m.cb9)"))]

use super::rpc::RpcError;
use crate::query::changes::{Changes, Input};
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// 저널에서 "다시 읽어라" 를 찾을 종류. **한 줄을 고치면 바뀜의 종류가 바뀐다.**
/// `rm` 은 사라진 줄을, `note` 는 스냅샷이 모르는 메모를 가리킨다.
const KINDS: [&str; 2] = ["rm", "note"];

/// 메서드 몸통 — `root` 는 부르는 쪽이 푼 프로젝트의 자리다. `params.cursor` 만 읽는다.
pub(crate) fn handle(root: &Path, params: &Value) -> Result<Value, RpcError> {
    let cursor = match params.get("cursor") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        Some(_) => return Err(RpcError::invalid_params("cursor must be a string")),
    };
    let answer = poll(root, cursor, crate::model::parse_rfc3339(&crate::model::now()).unwrap_or(0))
        .map_err(|f| RpcError::moai(f.code, f.message))?;
    serde_json::to_value(answer).map_err(|e| RpcError::new(super::rpc::code::INTERNAL, e.to_string()))
}

/// `root` 의 트래커를 읽어 [`crate::query::changes::changes`] 를 부른다. `now` 는 초다(시험이 건넨다).
pub(crate) fn poll(root: &Path, cursor: Option<&str>, now: i64) -> crate::fail::R<Changes> {
    let side: crate::side::Shared =
        Arc::new(Mutex::new(crate::side::Side { here: root.to_path_buf(), ..crate::side::Side::default() }));
    let entered = crate::side::enter(Some(side.clone()));
    let read = read();
    drop(entered);
    let (repo_root, issues, journal, torn, archive) = read?;
    let unread = !side.lock().unwrap_or_else(|e| e.into_inner()).unread.is_empty();
    let scope = std::fs::canonicalize(&repo_root).unwrap_or(repo_root).display().to_string();
    let torn: Vec<Option<&str>> = torn.iter().map(|t| t.as_deref()).collect();
    let input =
        Input { issues: &issues, journal: &journal, torn: &torn, unread, scope: &scope, archive: &archive, now };
    Ok(crate::query::changes::changes(&input, cursor))
}

type Read =
    (std::path::PathBuf, Vec<crate::model::Issue>, Vec<crate::model::JournalEntry>, Vec<Option<String>>, String);

/// 통이 걸린 동안 읽는다. 트래커가 없으면 CLI 와 같은 거절이다.
fn read() -> crate::fail::R<Read> {
    let lang = crate::i18n::Lang::En;
    let repo = crate::store::Repo::find(|| lang)?
        .ok_or_else(|| crate::fail::Fail::new(crate::i18n::say(lang, "refuse.not_a_repo")))?;
    let issues = repo.read()?.issues;
    let (mut journal, mut torn) = (Vec::new(), Vec::new());
    for kind in KINDS {
        let (entries, garbled) = repo.journal_of_kind(kind);
        journal.extend(entries);
        torn.extend(garbled.into_iter().map(|g| g.ts));
    }
    let archive = archive_mark(&repo.root);
    Ok((repo.root.clone(), issues, journal, torn, archive))
}

/// 아카이브 디렉터리와 그 파일의 표식 — 이름·고친 때·길이. 바뀌면 줄이 옮겨 갔을 수 있다.
fn archive_mark(root: &Path) -> String {
    let marks = crate::archive::marks(root).unwrap_or_default();
    marks
        .iter()
        .map(|(path, stamp)| {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match stamp {
                Some((at, len)) => {
                    let ns = at.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
                    format!("{name}:{ns}:{len}")
                }
                None => format!("{name}:-"),
            }
        })
        .collect::<Vec<_>>()
        .join("|")
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

    fn secs(at: &str) -> i64 {
        crate::model::parse_rfc3339(at).unwrap()
    }

    /// 이 시각에 부른 `changes` — `(커서, 바뀜, 사라짐, reset)`.
    fn ask(dir: &Path, cursor: Option<&str>, at: &str) -> (String, Vec<String>, Vec<String>, bool) {
        let got = poll(dir, cursor, secs(at)).unwrap_or_else(|e| panic!("{}", e.message));
        (got.cursor, got.changed, got.removed, got.reset)
    }

    fn id_of(out: &str) -> String {
        out.split_whitespace().find(|w| w.starts_with("t-")).unwrap_or_else(|| panic!("id 가 없다: {out}")).to_string()
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
        serde_json::from_str::<Value>(&out).unwrap()["id"].as_str().map(str::to_string).unwrap_or_else(|| id_of(&out))
    }

    #[test]
    fn each_write_names_exactly_its_row() {
        let (_keep, root) = project("changes-writes");
        let (a, b, c) = (
            add(&root, "2026-11-04T09:01:00Z", "하나"),
            add(&root, "2026-11-04T09:01:00Z", "둘"),
            add(&root, "2026-11-04T09:01:00Z", "셋"),
        );
        let (cursor, changed, removed, reset) = ask(&root, None, "2026-11-04T09:10:00Z");
        assert!(reset && changed.is_empty() && removed.is_empty());

        let step = |at: &str, args: &[&str], cursor: &str| {
            moai(&root, at, args);
            ask(&root, Some(cursor), at)
        };
        // add
        let d = add(&root, "2026-11-04T09:11:00Z", "넷");
        let (cursor, changed, removed, reset) = ask(&root, Some(&cursor), "2026-11-04T09:11:00Z");
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
        let (_, changed, removed, reset) = ask(&root, Some(&cursor), "2026-11-04T09:16:00Z");
        assert_eq!((changed, removed, reset), (vec![], vec![], false));
    }

    #[test]
    fn two_writes_in_one_second_and_a_double_write_are_both_seen() {
        let (_keep, root) = project("changes-ties");
        let (a, b) = (add(&root, "2026-11-04T09:01:00Z", "하나"), add(&root, "2026-11-04T09:01:00Z", "둘"));
        let at = "2026-11-04T09:30:00Z";
        moai(&root, at, &["edit", &a, "--title", "첫 고침"]);
        // 커서를 같은 초에 받고, 그 초가 끝나기 전에 둘이 더 바뀐다 — 하나는 두 번째 고침이다.
        let (cursor, ..) = ask(&root, None, at);
        moai(&root, at, &["edit", &a, "--title", "둘째 고침"]);
        moai(&root, at, &["edit", &b, "--title", "고침"]);
        let (cursor, changed, removed, reset) = ask(&root, Some(&cursor), at);
        let mut want = vec![a.clone(), b.clone()];
        want.sort();
        assert_eq!((changed, removed, reset), (want, vec![], false));
        // 알린 것을 같은 초에 다시 묻으면 비었다.
        let (_, changed, _, reset) = ask(&root, Some(&cursor), at);
        assert!(changed.is_empty() && !reset);
    }

    #[test]
    fn a_row_moved_to_the_archive_resets() {
        let (_keep, root) = project("changes-archive");
        let a = add(&root, "2026-11-04T09:01:00Z", "하나");
        add(&root, "2026-11-04T09:01:00Z", "둘");
        moai(&root, "2026-11-04T09:02:00Z", &["mv", &a, "done"]);
        let (cursor, ..) = ask(&root, None, "2026-12-30T00:00:00Z");
        let out = moai(&root, "2026-12-30T00:00:01Z", &["archive"]);
        assert!(!root.join(".moai/archive").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true), "{out}");
        let (_, changed, removed, reset) = ask(&root, Some(&cursor), "2026-12-30T00:00:02Z");
        assert!(reset && changed.is_empty() && removed.is_empty());
    }

    #[test]
    fn a_bad_cursor_or_project_is_a_reset_or_a_moai_error() {
        let (_keep, root) = project("changes-bad");
        let got = handle(&root, &serde_json::json!({ "cursor": "쓰레기" })).unwrap();
        assert_eq!(got["reset"], true);
        assert!(got["cursor"].is_string() && got["changed"] == serde_json::json!([]));
        let fresh = handle(&root, &serde_json::json!({})).unwrap();
        assert_eq!(fresh["reset"], true);
        assert!(handle(&root, &serde_json::json!({ "cursor": 3 })).is_err());
        // 트래커가 없는 자리는 오류다 — reset 이 아니다.
        let empty = Scratch::new("changes-none");
        assert!(handle(empty.path(), &serde_json::json!({})).is_err());
    }
}
