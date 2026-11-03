//! lib 크레이트를 **다른 크레이트처럼** 부른다(moai-50p5.gol). `moai::` 로만 닿는다.
//!
//! 이 파일이 쓰는 이름이 곧 지키는 공개 표면이다. 모듈 하나를 실수로 private 으로 되돌리면
//! 여기가 컴파일에서 진다. 견줄 답은 `CARGO_BIN_EXE_moai` 바이너리의 `--json` 에서 받는다 —
//! 같은 저장소를 두 길(lib·CLI)로 읽어 id 집합과 칸이 같아야 한다.
//!
//! `store` 의 전역 알림 값(`MOVED`·`TALLY` …)은 단언하지 않는다. 시험이 한 프로세스의
//! 여러 스레드라 서로 섞인다.

use moai::config::Config;
use moai::i18n::Lang;
use moai::model::{Issue, Kind};
use moai::query::{Filter, Raw, Where};
use moai::report;
use moai::store::{Opened, Repo};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_moai");
const NOW: &str = "2026-09-11T04:12:03Z";
const ACTOR: &str = "테스터 (tester@example.com)";

/// 임시 저장소 자리. 놓을 때 치운다(패닉으로 풀릴 때도).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "moai-lib-api-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        // 겹치면 크게 실패한다 — 이미 있는 자리를 말없이 나눠 쓰지 않는다.
        std::fs::create_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 바이너리를 격리해서 돌린다. 사람의 설정·시계·창에 닿지 않게 환경을 못박는다.
fn moai(dir: &Path, args: &[&str], stdin: Option<&str>) -> String {
    use std::io::Write;
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .current_dir(dir)
        .env("HOME", &home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("MOAI_ACTOR", ACTOR)
        .env("MOAI_NOW", NOW)
        .env("MOAI_CONFIG", dir.join("no-user-config.toml"))
        .env("MOAI_CTL_DIR", dir.join("ctl"))
        .env("MOAI_NO_UPDATE_CHECK", "1")
        .env("MOAI_LANG", "en")
        .env("NO_COLOR", "1")
        .env_remove("MOAI_HERE")
        .stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("moai 를 실행하지 못했다");
    if let Some(text) = stdin {
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "moai {args:?} 가 실패했다\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn json(dir: &Path, args: &[&str]) -> serde_json::Value {
    serde_json::from_str(&moai(dir, args, None)).unwrap_or_else(|e| panic!("moai {args:?} 의 --json 이 아니다: {e}"))
}

/// id 를 제목으로 찾는다 — `add` 가 뽑는 id 는 무작위라 제목으로 짚는다.
fn id_of(issues: &[Issue], title: &str) -> String {
    issues.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("{title} 줄이 없다")).id.clone()
}

/// 에픽 하나와 멤버 다섯: 집은 일, 미룬 일, 막힌 일, 집을 수 있는 일 둘.
///
/// 반환은 (저장소 자리, 제목 → id 를 찾을 때 쓸 스냅샷은 부르는 쪽이 읽는다).
fn stocked(name: &str) -> Scratch {
    let s = Scratch::new(name);
    moai(s.path(), &["init", "argos"], None);
    let plan = "# Epic\n- [p1] started\n- [p2] shelved\n- [p2] blocked\n- [p3] free-a\n- [p2] free-b\n";
    moai(s.path(), &["add", "--from", "-"], Some(plan));
    let all = json(s.path(), &["show", "--json"]);
    let by = |t: &str| -> String {
        all.as_array().unwrap().iter().find(|r| r["title"] == t).unwrap()["id"].as_str().unwrap().to_string()
    };
    let (started, shelved, blocked) = (by("started"), by("shelved"), by("blocked"));
    moai(s.path(), &["mv", &started, "in_progress", "--from", "todo"], None);
    moai(s.path(), &["defer", &shelved, "-m", "later"], None);
    moai(s.path(), &["link", &started, "--blocks", &blocked], None);
    s
}

/// 공개 표면으로 저장소를 연다. `Repo::open` 은 위로 찾지 않고 준 자리의 `.moai` 를 연다.
fn open(dir: &Path) -> Repo {
    match Repo::open(dir, || Lang::En).unwrap_or_else(|e| panic!("{e}")) {
        Opened::Repo(repo) => repo,
        Opened::Uninit => panic!("init 전이다"),
        Opened::Missing => panic!("자리가 없다"),
    }
}

fn ids<'a>(rows: impl IntoIterator<Item = &'a Issue>) -> BTreeSet<String> {
    rows.into_iter().map(|i| i.id.clone()).collect()
}

fn json_ids(v: &serde_json::Value) -> BTreeSet<String> {
    v.as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect()
}

#[test]
fn open_reads_the_same_rows_the_cli_lists() {
    let s = stocked("open");
    let repo = open(s.path());
    let load = repo.read().unwrap_or_else(|e| panic!("{e}"));
    assert!(load.errors.is_empty(), "못 읽는 줄이 섰다: {} 줄", load.errors.len());

    // 같은 줄을 CLI 로 받는다. 미룬 줄·done 은 목록에서 숨으므로 `--all --deferred` 는 쓰지 않고
    // 숨는 것을 lib 쪽 필터로도 같게 거른다 — 아래 `query_and_the_cli_filter_agree` 가 그것을 본다.
    let cli = json(s.path(), &["show", "--deferred", "--all", "--json"]);
    let cli_all = json_ids(&cli);
    let lib_all = ids(&load.issues);
    assert!(cli_all.is_subset(&lib_all), "CLI 가 lib 에 없는 줄을 냈다: {cli_all:?} vs {lib_all:?}");
    assert_eq!(load.issues.iter().filter(|i| i.kind == Kind::Epic).count(), 1);
    assert_eq!(load.issues.len(), 6, "에픽 하나와 멤버 다섯");

    // 줄 하나의 칸이 CLI 의 것과 같다.
    for row in cli.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let mine = load.issues.iter().find(|i| i.id == id).unwrap();
        assert_eq!(mine.status.as_str(), row["status"].as_str().unwrap(), "{id} 의 칸이 갈렸다");
        assert_eq!(mine.title, row["title"].as_str().unwrap());
    }
}

#[test]
fn query_and_the_cli_filter_agree() {
    let s = stocked("query");
    let repo = open(s.path());
    let load = repo.read().unwrap_or_else(|e| panic!("{e}"));
    let cfg: &Config = &repo.config;

    // `-s todo` — 숨는 줄(미룬 것)은 기본으로 빠진다.
    let filter =
        Filter::build(Raw { status: vec!["todo".into()], ..Raw::default() }).unwrap_or_else(|_| panic!("필터"));
    let wh = Where::from_soil(&load.issues, cfg, report::Soil::of(&load.issues));
    let mine =
        ids(load.issues.iter().filter(|i| filter.hidden_by(i, NOW, &wh).is_none() && filter.matches(i, NOW, &wh)));
    let cli = json_ids(&json(s.path(), &["show", "-s", "todo", "--json"]));
    assert_eq!(mine, cli, "-s todo 의 id 집합이 갈렸다");
    assert!(!mine.is_empty());
}

#[test]
fn report_ready_and_status_match_the_cli() {
    let s = stocked("report");
    let repo = open(s.path());
    let load = repo.read().unwrap_or_else(|e| panic!("{e}"));
    let cfg = &repo.config;

    // ready: 집을 수 있는 일은 둘(free-a, free-b)이다. 집은 일·미룬 일·막힌 일은 안 든다.
    let ready = report::ready(&load.issues, cfg);
    let mine = ids(ready.iter().copied());
    let cli = json(s.path(), &["ready", "--json"]);
    assert_eq!(mine, json_ids(&cli["ready"]), "ready 의 id 집합이 갈렸다");
    let mut titles: Vec<&str> = ready.iter().map(|i| i.title.as_str()).collect();
    titles.sort();
    assert_eq!(titles, ["free-a", "free-b"]);

    // status: 칸별 수와 총수가 CLI 의 것과 같다.
    let board = report::status_unjudged(&load.issues, &[], cfg, NOW);
    let cli = json(s.path(), &["status", "--json"]);
    assert_eq!(board.total as u64, cli["total"].as_u64().unwrap());
    for (column, n) in &board.counts {
        assert_eq!(*n as u64, cli["counts"][column.as_str()].as_u64().unwrap_or(0), "{column} 의 수가 갈렸다");
    }
    assert_eq!(board.statuses, cfg.statuses.iter().map(|s| s.as_str().to_string()).collect::<Vec<_>>());
}

#[test]
fn a_write_through_with_write_shows_in_the_cli() {
    let s = stocked("write");
    let repo = open(s.path());
    let id = id_of(&repo.read().unwrap_or_else(|e| panic!("{e}")).issues, "free-a");

    // CLI 의 명령이 하는 그대로: 닫는 함수가 락 안에서 다시 읽은 줄을 고치고, 남길 저널 줄을 돌려준다.
    // 필드 변경(제목)은 저널에 적지 않는다 — 빈 목록이다.
    let before = repo.with_write(
        || Lang::En,
        |issues, _cfg, _reserved| {
            let row = issues.iter_mut().find(|i| i.id == id).expect("락 안에서 줄이 사라졌다");
            let old = std::mem::replace(&mut row.title, "renamed through the library".to_string());
            Ok((Vec::new(), old))
        },
    );
    assert_eq!(before.unwrap_or_else(|e| panic!("{e}")), "free-a");

    let shown = json(s.path(), &["show", &id, "--json"]);
    assert_eq!(shown["title"], "renamed through the library");
    assert_eq!(shown["status"], "todo", "제목만 고쳤는데 칸이 움직였다");

    // 쓴 뒤에 lib 로 다시 읽어도 같다 — 두 길이 한 파일을 본다.
    let again = repo.read().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(again.issues.iter().find(|i| i.id == id).unwrap().title, "renamed through the library");
}
