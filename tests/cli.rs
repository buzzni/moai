//! 바이너리를 실제로 돌린다. dev-dependency 없이 — 테스트 하네스가
//! 의존성을 끌고 오기 시작하면 "남의 저장소에 설치되는 도구" 라는 전제가 흐려진다.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_moai");
const NOW: &str = "2026-09-11T04:12:03Z";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "moai-cli-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
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

fn moai(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_ACTOR", "테스터")
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .expect("moai 를 실행하지 못했다")
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = moai(dir, args);
    assert!(
        out.status.success(),
        "moai {args:?} 가 실패했다\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn init(name: &str) -> Scratch {
    let s = Scratch::new(name);
    ok(s.path(), &["init", "argos"]);
    s
}

fn issues(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap()
}

#[test]
fn init_creates_exactly_three_files() {
    let s = init("init");
    let mut names: Vec<String> = std::fs::read_dir(s.path().join(".moai"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["config.toml", "issues.jsonl", "journal.jsonl"]);

    let attrs = std::fs::read_to_string(s.path().join(".gitattributes")).unwrap();
    assert!(attrs.contains("journal.jsonl  text eol=lf merge=union"), "{attrs}");
    // 스냅샷에 union 을 걸면 같은 id 를 가진 줄이 둘 생긴다 — 데이터 손상이다.
    assert!(!attrs.contains("issues.jsonl   text eol=lf merge"), "{attrs}");
}

/// 두 번째 init 은 거부한다. 있는 이슈를 지우는 길을 만들지 않는다.
#[test]
fn init_refuses_to_run_twice() {
    let s = init("twice");
    let out = moai(s.path(), &["init"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("이미 있다"));
}

/// 남의 .gitignore 를 지우지 않고 빠진 줄만 덧붙인다.
#[test]
fn init_appends_to_an_existing_gitignore() {
    let s = Scratch::new("ignore");
    std::fs::write(s.path().join(".gitignore"), "target/\n").unwrap();
    ok(s.path(), &["init", "argos"]);
    let got = std::fs::read_to_string(s.path().join(".gitignore")).unwrap();
    assert!(got.starts_with("target/\n"), "{got}");
    assert!(got.contains(".moai/lock"), "{got}");
}

#[test]
fn add_then_show() {
    let s = init("add");
    ok(s.path(), &["add", "첫 이슈", "-t", "bug", "-p", "1"]);
    let out = ok(s.path(), &["show"]);
    assert!(out.contains("첫 이슈") && out.contains("#bug") && out.contains("p1"), "{out}");
    assert!(out.contains("1건 (todo 1)"), "{out}");
}

/// `-q` 는 id 만 낸다 — 셸에서 `ID=$(moai add ...)` 로 쓰기 위한 것이다.
#[test]
fn quiet_prints_only_the_id() {
    let s = init("quiet");
    let id = ok(s.path(), &["add", "제목", "-q"]);
    let id = id.trim();
    assert!(id.starts_with("argos-") && !id.contains(' '), "{id:?}");
    assert!(issues(s.path()).contains(id));
}

#[test]
fn children_hang_under_their_parent_in_the_file() {
    let s = init("child");
    let parent = ok(s.path(), &["add", "부모", "-q"]).trim().to_string();
    ok(s.path(), &["add", "자식", "--parent", &parent, "-q"]);
    ok(s.path(), &["add", "남", "-q"]);
    let ids: Vec<String> = issues(s.path())
        .lines()
        .map(|l| field(l, "id"))
        .collect();
    let at = ids.iter().position(|i| i == &parent).unwrap();
    assert!(ids[at + 1].starts_with(&format!("{parent}.")), "{ids:?}");
}

/// 모든 명령에 `--json` 이 돌고, 사람 출력은 한 글자도 섞이지 않는다.
#[test]
fn every_command_speaks_json() {
    let s = Scratch::new("json");
    one_json_value(&ok(s.path(), &["init", "argos", "--json"]));

    let made = ok(s.path(), &["add", "제목", "--json"]);
    one_json_value(&made);
    let id = field(&made, "id");

    for args in [
        vec!["show", "--json"],
        vec!["show", "issue", "--json"],
        vec!["show", "epic", "--json"],
        vec!["show", &id, "--json"],
    ] {
        one_json_value(&ok(s.path(), &args));
    }
}

/// `{"id":"argos-4aex",...}` 에서 값 하나를 꺼낸다.
fn field(json: &str, key: &str) -> String {
    let at = json.find(&format!("\"{key}\":\"")).unwrap_or_else(|| panic!("{key} 가 없다 — {json}"));
    let rest = &json[at + key.len() + 4..];
    rest[..rest.find('"').unwrap()].to_string()
}

/// 의존성 없이 하는 최소 검사 — 값 하나고, 한 줄이고, 이스케이프가 없다.
fn one_json_value(s: &str) {
    let t = s.trim();
    assert!(!t.is_empty(), "빈 출력");
    assert_eq!(t.lines().count(), 1, "JSON 은 한 줄이다 — {t}");
    assert!(!t.contains('\u{1b}'), "JSON 에 이스케이프가 섞였다 — {t}");
    let (first, last) = (t.chars().next().unwrap(), t.chars().last().unwrap());
    assert!(
        (first == '{' && last == '}') || (first == '[' && last == ']'),
        "JSON 값 하나가 아니다 — {t}"
    );
}

/// `moai show | head` 는 크래시가 아니라 조용한 종료여야 한다.
#[test]
fn survives_a_closed_pipe() {
    let s = init("pipe");
    for n in 0..50 {
        ok(s.path(), &["add", &format!("이슈 {n}"), "-q"]);
    }
    let out = Command::new("bash")
        .arg("-c")
        .arg(format!("'{BIN}' show | head -1 >/dev/null; exit ${{PIPESTATUS[0]}}"))
        .current_dir(s.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "닫힌 파이프에 {:?} 로 끝났다\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// 파이프로 넘기면 색이 저절로 꺼진다. 리다이렉트된 출력에 이스케이프가 남으면
/// `grep`·`diff`·골든 테스트가 전부 어긋난다.
#[test]
fn colour_turns_itself_off_when_not_a_terminal() {
    let s = Scratch::new("colour");
    ok(s.path(), &["init", "argos"]);
    ok(s.path(), &["add", "제목", "-q"]);
    let run = |args: &[&str]| {
        Command::new(BIN)
            .args(args)
            .current_dir(s.path())
            .env_remove("NO_COLOR")
            .output()
            .unwrap()
            .stdout
    };
    assert!(!String::from_utf8(run(&["show"])).unwrap().contains('\u{1b}'));
    // 강제하면 나온다 — `less -R` 로 넘길 때 필요하다.
    assert!(String::from_utf8(run(&["show", "--color", "always"])).unwrap().contains('\u{1b}'));
}

/// **조용한 손실이 이 도구가 못 견디는 유일한 실패 모드다.**
/// 락이 없으면 마지막에 rename 한 프로세스가 앞의 이슈를 지운다.
#[test]
fn concurrent_adds_all_survive() {
    let s = init("race");
    let n = 8;
    let kids: Vec<_> = (0..n)
        .map(|i| {
            Command::new(BIN)
                .args(["add", &format!("동시 {i}"), "-q"])
                .current_dir(s.path())
                .env("MOAI_ACTOR", "테스터")
                .env("NO_COLOR", "1")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for k in kids {
        let out = k.wait_with_output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let lines = issues(s.path()).lines().count();
    assert_eq!(lines, n, "{n}개를 동시에 넣었는데 {lines}줄만 남았다");
}

/// 깨진 줄이 있어도 나머지를 보여주고, 어느 줄인지 말하고, 비영으로 끝난다.
#[test]
fn a_broken_line_is_reported_but_the_rest_still_shows() {
    let s = init("broken");
    ok(s.path(), &["add", "멀쩡한 것", "-q"]);
    let path = s.path().join(".moai/issues.jsonl");
    let mut src = std::fs::read_to_string(&path).unwrap();
    src.push_str("{\"id\": 깨짐\n");
    std::fs::write(&path, src).unwrap();

    let out = moai(s.path(), &["show"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("멀쩡한 것"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("2줄"));
    assert!(!out.status.success(), "깨진 줄을 보고 0 으로 끝났다");

    // 그리고 그 위에 덮어쓰지 않는다 — 쓰면 그 줄이 영원히 사라진다.
    let write = moai(s.path(), &["add", "새 것"]);
    assert!(!write.status.success());
    assert!(String::from_utf8_lossy(&write.stderr).contains("2줄"));
}

#[test]
fn refuses_what_it_should() {
    let s = init("refuse");
    for (args, want) in [
        (vec!["show", "isue"], "id 도 종류도 아니다"),
        (vec!["show", "argos-0000"], "못 찾았다"),
        (vec!["add", "x", "-s", "blocked"], "라는 칸이 없다"),
        (vec!["add", "x", "--parent", "argos-0000"], "부모가 없는 자식"),
        (vec!["add", "   "], "제목이 비었다"),
    ] {
        let out = moai(s.path(), &args);
        assert!(!out.status.success(), "{args:?} 가 통과했다");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(want), "{args:?} → {err}");
        // 거부는 아무것도 쓰지 않는다
        assert_eq!(issues(s.path()).lines().count(), 0, "{args:?} 가 파일을 건드렸다");
    }
}

/// 저장소 밖에서는 무엇을 해야 하는지 말한다.
#[test]
fn outside_a_repo_it_says_what_to_do() {
    let s = Scratch::new("outside");
    let out = moai(s.path(), &["show"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("moai init"));
}
