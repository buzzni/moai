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

// ── S2 — 칸 옮기기·고치기·지우기·메모, 그리고 필터 ────────────────────

fn add(dir: &Path, args: &[&str]) -> String {
    let mut v = vec!["add"];
    v.extend_from_slice(args);
    v.push("-q");
    ok(dir, &v).trim().to_string()
}

/// 그 이슈의 줄 하나. 파일 전체를 보면 남의 줄에 걸린다 — 에픽의
/// `"kind":"epic"` 이 `"epic"` 검사에 잡히는 식으로.
fn line_of(dir: &Path, id: &str) -> String {
    issues(dir)
        .lines()
        .find(|l| l.starts_with(&format!("{{\"id\":\"{id}\"")))
        .unwrap_or_else(|| panic!("{id} 줄이 없다"))
        .to_string()
}

fn journal(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".moai/journal.jsonl")).unwrap()
}

#[test]
fn mv_moves_and_journals() {
    let s = init("mv");
    let id = add(s.path(), &["제목"]);
    ok(s.path(), &["mv", &id, "in_progress"]);

    let line = line_of(s.path(), &id);
    assert!(line.contains(r#""status":"in_progress""#), "{line}");
    let j = journal(s.path());
    assert!(j.contains(r#""kind":"status","by":"테스터","from":"todo","to":"in_progress""#), "{j}");
}

/// **되감기를 막지 않는다.** 막으면 그게 게이트고, 이전 시도가 그것으로 죽었다.
#[test]
fn mv_backwards_is_allowed() {
    let s = init("mvback");
    let id = add(s.path(), &["제목"]);
    ok(s.path(), &["mv", &id, "done"]);
    let out = ok(s.path(), &["mv", &id, "todo"]);
    assert!(out.contains("done → todo"), "{out}");
    // 두 번째는 이미 그 칸이라 아무 일도 없다
    assert!(ok(s.path(), &["mv", &id, "todo"]).contains("이미 todo"), "");
}

/// #a-partial — 하나가 없다고 나머지를 안 옮기지 않는다. 대신 비영으로 끝난다.
#[test]
fn mv_does_everything_it_can_then_fails() {
    let s = init("mvpart");
    let a = add(s.path(), &["가"]);
    let b = add(s.path(), &["나"]);
    let out = moai(s.path(), &["mv", &a, "argos-0000", &b, "review"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&a) && text.contains(&b), "{text}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("argos-0000"));
    assert!(!out.status.success(), "못 찾은 것이 있는데 0 으로 끝났다");
    assert_eq!(issues(s.path()).matches(r#""status":"review""#).count(), 2);
}

#[test]
fn mv_refuses_a_column_that_does_not_exist() {
    let s = init("mvbad");
    let id = add(s.path(), &["제목"]);
    let out = moai(s.path(), &["mv", &id, "blocked"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("라는 칸이 없다"));
    assert!(journal(s.path()).lines().count() == 1, "거부가 저널을 건드렸다");
}

#[test]
fn edit_changes_fields_and_none_clears() {
    let s = init("edit");
    let epic = add(s.path(), &["에픽", "--type", "epic"]);
    let id = add(s.path(), &["제목", "-t", "bug", "-e", &epic]);

    ok(s.path(), &["edit", &id, "--tag", "parser", "--untag", "bug", "-p", "0", "-a", "claude"]);
    let line = line_of(s.path(), &id);
    assert!(line.contains(r#""tags":["parser"]"#), "{line}");
    assert!(line.contains(r#""priority":0"#), "{line}");
    assert!(line.contains(r#""assignee":"claude""#), "{line}");
    assert!(line.contains(&format!(r#""epic":"{epic}""#)), "{line}");

    ok(s.path(), &["edit", &id, "-e", "none", "-a", "none"]);
    let line = line_of(s.path(), &id);
    assert!(!line.contains("\"epic\"") && !line.contains("\"assignee\""), "{line}");

    // 필드 변경은 저널에 적지 않는다 — 적기 시작하면 이벤트 로그가 된다
    assert_eq!(journal(s.path()).lines().count(), 2, "생성 둘 말고 더 쌓였다");
}

#[test]
fn edit_says_so_when_nothing_was_asked_for() {
    let s = init("editnop");
    let id = add(s.path(), &["제목"]);
    let out = moai(s.path(), &["edit", &id]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("무엇을 고칠지"));
}

#[test]
fn rm_removes_and_names_what_it_broke() {
    let s = init("rm");
    let epic = add(s.path(), &["에픽", "--type", "epic"]);
    let member = add(s.path(), &["멤버", "-e", &epic]);
    let out = moai(s.path(), &["rm", &epic]);
    assert!(out.status.success());
    assert!(!issues(s.path()).contains(&format!("\"id\":\"{epic}\"")));
    assert!(issues(s.path()).contains(&member));
    // 막지 않고 알린다 — 끊긴 참조는 `moai status` 가 드러낸다
    assert!(String::from_utf8_lossy(&out.stderr).contains("끊긴 참조"));
    assert!(journal(s.path()).contains(r#""kind":"rm""#));
}

/// 메모는 스냅샷을 건드리지 않고 저널에만 쌓인다.
#[test]
fn note_only_touches_the_journal() {
    let s = init("note");
    let id = add(s.path(), &["제목"]);
    let before = issues(s.path());
    ok(s.path(), &["note", &id, "Trino 0.9 에서만 재현된다"]);
    assert_eq!(issues(s.path()), before, "메모가 스냅샷을 바꿨다");
    assert!(journal(s.path()).contains(r#""kind":"note""#));
    // 그리고 상세의 이력에 보인다 — 아무도 안 읽는 로그는 곧 깨진다
    assert!(ok(s.path(), &["show", &id]).contains("Trino 0.9"));
}

/// CLI 를 관리할 트래커라 `--json 이 tags 를 빠뜨린다` 같은 제목이 흔하다.
#[test]
fn a_title_may_start_with_hyphens() {
    let s = init("hyphen");
    let id = add(s.path(), &["--json 이 tags 를 빠뜨린다", "-t", "bug"]);
    assert!(issues(s.path()).contains("--json 이 tags 를 빠뜨린다"));
    // 그래도 아는 플래그는 여전히 플래그다
    assert!(ok(s.path(), &["show", &id, "--json"]).starts_with('{'));
    assert!(!moai(s.path(), &["add", "--json"]).status.success());
}

#[test]
fn filters_reach_the_command_line() {
    let s = init("filter");
    let a = add(s.path(), &["가", "-t", "bug"]);
    let b = add(s.path(), &["나", "-t", "bug,parser"]);
    add(s.path(), &["다", "-t", "chore"]);
    ok(s.path(), &["mv", &a, "review"]);

    let and = ok(s.path(), &["show", "-t", "bug", "-t", "parser"]);
    assert!(and.contains(&b) && !and.contains(&a), "{and}");

    let or = ok(s.path(), &["show", "-t", "bug,chore"]);
    assert!(or.contains("3건") || or.contains(&a), "{or}");

    // 같은 뜻을 한 문자열로도 쓸 수 있다
    let flags = ok(s.path(), &["show", "-s", "review"]);
    let string = ok(s.path(), &["show", "--filter", "status=review"]);
    assert_eq!(flags, string);

    let e = moai(s.path(), &["show", "-s", "todo", "-s", "review"]);
    assert!(String::from_utf8_lossy(&e.stderr).contains("-s todo,review"));
    let e = moai(s.path(), &["show", "--filter", "statu=todo"]);
    assert!(String::from_utf8_lossy(&e.stderr).contains("status, tag"));
}

/// done 은 기본으로 숨기되 몇 건인지는 말한다. 칸을 콕 집으면 그 말을 따른다.
#[test]
fn done_is_hidden_but_counted() {
    let s = init("done");
    let a = add(s.path(), &["가"]);
    add(s.path(), &["나"]);
    ok(s.path(), &["mv", &a, "done"]);

    let out = ok(s.path(), &["show"]);
    assert!(!out.contains(&a), "{out}");
    assert!(out.contains("done 1건 숨김"), "{out}");
    assert!(ok(s.path(), &["show", "--all"]).contains(&a));
    assert!(ok(s.path(), &["show", "-s", "done"]).contains(&a));
}

/// `--stale` 은 지금 칸에 머문 기간이다. 시계를 고정해야 시험할 수 있다.
#[test]
fn stale_finds_what_rots_in_a_column() {
    let s = init("stale");
    let id = add(s.path(), &["오래된 리뷰"]);
    ok(s.path(), &["mv", &id, "review"]);

    let later = Command::new(BIN)
        .args(["show", "-s", "review", "--stale", "3"])
        .current_dir(s.path())
        .env("MOAI_NOW", "2026-09-20T04:12:03Z")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&later.stdout).contains(&id));

    let soon = ok(s.path(), &["show", "-s", "review", "--stale", "3"]);
    assert!(!soon.contains(&id), "{soon}");
}
