//! lib 크레이트를 **다른 크레이트처럼** 부른다(moai-50p5.gol). `moai::` 로만 닿는다.
//!
//! 이 파일이 쓰는 이름이 곧 지키는 공개 표면이다. 연 모듈 열셋 가운데 하나라도 실수로 private 으로
//! 되돌리면 여기가 컴파일에서 진다 — 시험이 직접 부르지 않는 일곱도 아래 `use` 가 이름으로 든다.
//! 견줄 답은 `CARGO_BIN_EXE_moai` 바이너리의 `--json` 에서 받는다 — 같은 저장소를 두 길(lib·CLI)로
//! 읽어 id 집합과 칸이 같아야 한다.
//!
//! `store` 의 전역 알림 값(`MOVED`·`TALLY` …)은 단언하지 않는다. 시험이 한 프로세스의
//! 여러 스레드라 서로 섞인다.

use moai::config::Config;
use moai::i18n::Lang;
use moai::model::{Issue, Kind};
use moai::query::{Filter, Raw, Where};
use moai::report;
use moai::store::{Opened, Repo};
// 이 시험이 이름으로 쓰지 않는 일곱이다 — `fail` 은 CONTRIBUTING.md "The library" 의 첫 여섯에 들고(위의 `i18n` 은
// 따라 연 쪽이다), 나머지 여섯은 공개 시그니처가 타입 이름을 대서 따라 연 것이다. 이 줄이 없으면 일곱 가운데 하나를
// 닫아도 컴파일이 선다.
#[allow(unused_imports)]
use moai::{fail, held, nav, read_marks, style, tz, user_config};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

// 임시 자리의 뿌리 — `tests/cli.rs`·`tests/images.rs` 와 **한 파일**을 읽는다. `TMPDIR` 이 체크아웃 안이면
// 밖으로 옮기는 자가 거기 있다(moai-izeo). 맨 `temp_dir()` 위에 세우면 그 체크아웃의 `.git/config` 에 이
// 시험의 바이너리로 머지 드라이버가 심기고(`init`), 딸린 워크트리 안이면 `init` 이 거절돼 넷이 다 붉어진다.
// 그 파일의 `#[cfg(test)]` 시험도 여기서 함께 돈다.
#[allow(dead_code)]
#[path = "../src/scratch.rs"]
mod scratch;
// 물려받으면 git 이 바깥 저장소·바깥 설정을 보게 되는 이름들 — `tests/cli.rs` 의 `isolated` 와 한 목록이다.
#[path = "../src/git_leaks.rs"]
#[allow(dead_code)]
mod git_leaks;

use scratch::Scratch;

const BIN: &str = env!("CARGO_BIN_EXE_moai");
const NOW: &str = "2026-09-11T04:12:03Z";
/// lib 쪽 쓰기가 찍는 시각. 시험 프로세스에는 `MOAI_NOW` 가 없어(`set_var` 는 병렬로 도는 시험과 겨룬다)
/// 시각을 손으로 건넨다 — `NOW` 와 달라야 `updated_at` 이 정말 움직였는지 보인다.
const LATER: &str = "2026-09-12T00:00:00Z";
const ACTOR: &str = "테스터 (tester@example.com)";

/// `tests/cli.rs` 의 `isolated` 가 걷는 이름 가운데 git 목록 밖의 것 — 사람의 집·설정·세션·창.
/// 이 시험의 명령이 지금 읽는 것은 세션 id(집기 기록) 하나뿐이지만, 따로 막은 시험만 막혀 있으면
/// 명령을 더하는 날 결과가 기계를 따라 달라진다(`isolated` 의 주석).
const SWEPT: &[&str] = &[
    "USERPROFILE",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_PLUGIN_CACHE_DIR",
    "MOAI_HERE",
    "COLUMNS",
    "XDG_RUNTIME_DIR",
    "XDG_CONFIG_HOME",
    "MOAI_KEYS",
    "BASH_ENV",
    "MOAI_API_URL",
    "MOAI_REPO",
    "MOAI_INSTALL_URL",
    "TMUX",
    "TMUX_PANE",
    "SAYCODE_AGENT_ENV",
    "CMUX_SURFACE_ID",
    "CMUX_CLAUDE_TEAMS_TMUX_SHIM",
    "CLAUDE_CODE_SESSION_ID",
    "CODEX_THREAD_ID",
    "CODEX_SESSION_ID",
    "ANTIGRAVITY_CONVERSATION_ID",
    "MOAI_AGENT",
];

/// 바이너리를 격리해서 돌린다. 사람의 설정·시계·창에 닿지 않게 환경을 못박는다.
fn moai(dir: &Path, args: &[&str], stdin: Option<&str>) -> String {
    use std::io::Write;
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(BIN);
    // **걷기가 먼저다** — `Command` 의 환경은 이름마다 마지막에 부른 것이 이긴다(`isolated` 와 같은 차례).
    for var in git_leaks::swept(true).chain(SWEPT) {
        cmd.env_remove(var);
    }
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
        .env("TZ", "UTC")
        .stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("moai 를 실행하지 못했다");
    if let Some(text) = stdin {
        // 쓰기 실패(EPIPE)는 버린다 — moai 가 stdin 을 읽기 전에 거절하고 나가면 여기서 패닉해 아래 단언이
        // 낼 moai 의 stderr 를 가린다(`tests/cli.rs` 의 `from_stdin` 과 같은 까닭).
        let _ = child.stdin.take().unwrap().write_all(text.as_bytes());
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

/// 에픽 하나와 멤버 다섯: 집은 일, 미룬 일, 막힌 일, 집을 수 있는 일 둘. CLI 로만 짓는다.
fn stocked(name: &str) -> Scratch {
    let s = Scratch::new(&format!("lib-api-{name}"));
    moai(s.path(), &["init", "argos"], None);
    let plan = "# Epic\n- [p1] started\n- [p2] shelved\n- [p2] blocked\n- [p3] free-a\n- [p2] free-b\n";
    // `add --json` 이 만든 줄을 그대로 내므로 id 를 거기서 짚는다.
    let made: serde_json::Value = serde_json::from_str(&moai(s.path(), &["add", "--from", "-", "--json"], Some(plan)))
        .unwrap_or_else(|e| panic!("add --json 이 JSON 이 아니다: {e}"));
    let by = |t: &str| -> String {
        let row = made.as_array().and_then(|rows| rows.iter().find(|r| r["title"] == t));
        row.and_then(|r| r["id"].as_str()).unwrap_or_else(|| panic!("add 가 {t} 줄을 안 냈다")).to_string()
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
    assert_eq!(load.issues.iter().filter(|i| i.kind == Kind::Epic).count(), 1);
    assert_eq!(load.issues.len(), 6, "에픽 하나와 멤버 다섯");

    // 같은 줄을 CLI 로 받는다. `--all` 은 미룬 줄과 done 을 숨기지 않으므로(이 저장소에는 아카이브가 없다)
    // 여섯이 다 와야 한다. `--deferred` 를 얹으면 미룬 줄**만** 남아 한 줄만 견주게 된다.
    let cli = json(s.path(), &["show", "--all", "--json"]);
    assert_eq!(json_ids(&cli), ids(&load.issues), "lib 와 CLI 의 id 집합이 갈렸다");

    // 줄마다 칸과 제목이 CLI 의 것과 같다.
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

    // `-s todo` — 숨는 줄(미룬 것)은 기본으로 빠진다. 빼는 것도 `matches` 의 몫이라 그것 하나로 거른다 —
    // `hidden_by` 를 따로 얹으면 `matches` 가 숨기기를 잊어도 이 시험이 대신 빼 주어 못 잡는다.
    let filter = Filter::build(Raw { status: vec!["todo".into()], ..Raw::default() }).expect("필터");
    let wh = Where::from_soil(&load.issues, cfg, report::Soil::of(&load.issues));
    let mine = ids(load.issues.iter().filter(|i| filter.matches(i, NOW, &wh)));
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

    // status: 총수·칸별 수·칸 차례가 CLI 의 것과 같다. 칸별 수는 맵째 견준다 — 한쪽에만 선 칸도 잡는다.
    let board = report::status_unjudged(&load.issues, &[], cfg, NOW);
    let cli = json(s.path(), &["status", "--json"]);
    assert_eq!(board.total as u64, cli["total"].as_u64().unwrap());
    let lib_counts: BTreeMap<String, u64> = board.counts.iter().map(|(k, n)| (k.clone(), *n as u64)).collect();
    let cli_counts: BTreeMap<String, u64> = cli["counts"]
        .as_object()
        .expect("`counts` 가 없다")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or_else(|| panic!("{k} 의 수가 수가 아니다"))))
        .collect();
    assert_eq!(lib_counts, cli_counts, "칸별 수가 갈렸다");
    let cli_statuses: Vec<&str> =
        cli["statuses"].as_array().expect("`statuses` 가 없다").iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(board.statuses, cli_statuses, "칸 차례가 갈렸다");
}

#[test]
fn a_write_through_with_write_shows_in_the_cli() {
    let s = stocked("write");
    let repo = open(s.path());
    let id = id_of(&repo.read().unwrap_or_else(|e| panic!("{e}")).issues, "free-a");

    // `moai edit` 이 하는 그대로: 닫는 함수가 락 안에서 다시 읽은 줄을 고치고 `Issue::settle` 로 맞춘다
    // (정규화 → 검사 → 바뀌었으면 `updated_at`). **`with_write` 는 `updated_at` 을 안 찍는다** — `settle` 을
    // 빼면 줄은 바뀌는데 `updated_at` 이 옛 값이라 `--since`·`[NEW]` 가 그 고침을 못 본다.
    // 필드 변경(제목)은 저널에 적지 않는다 — 빈 목록이다.
    let before = repo.with_write(
        || Lang::En,
        |issues, cfg, _reserved| {
            let row = issues.iter_mut().find(|i| i.id == id).expect("락 안에서 줄이 사라졌다");
            let was = row.clone();
            row.title = "renamed through the library".to_string();
            assert!(row.settle(&was, cfg, LATER).expect("settle 이 거절했다"), "제목을 고쳤는데 안 바뀌었다고 한다");
            Ok((Vec::new(), was.title))
        },
    );
    assert_eq!(before.unwrap_or_else(|e| panic!("{e}")), "free-a");

    let shown = json(s.path(), &["show", &id, "--json"]);
    assert_eq!(shown["title"], "renamed through the library");
    assert_eq!(shown["status"], "todo", "제목만 고쳤는데 칸이 움직였다");
    assert_eq!(shown["updated_at"], LATER, "고친 시각이 안 남았다");

    // 쓴 뒤에 lib 로 다시 읽어도 같다 — 두 길이 한 파일을 본다.
    let again = repo.read().unwrap_or_else(|e| panic!("{e}"));
    let row = again.issues.iter().find(|i| i.id == id).unwrap();
    assert_eq!(row.title, "renamed through the library");
    assert_eq!(row.updated_at, LATER);
}
