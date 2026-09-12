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
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
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

/// 도구가 자라면 AGENTS.md 블록은 반드시 낡는다. 다시 쓸 길이 없으면 새
/// 세션의 에이전트가 없는 명령을 쓰고 있는 명령을 모른다.
#[test]
fn init_can_be_run_again_to_refresh() {
    let s = init("twice");
    let id = add(s.path(), &["지워지면 안 되는 것"]);

    // 블록을 일부러 낡게 만들어 둔다
    let md = s.path().join("AGENTS.md");
    let old = std::fs::read_to_string(&md).unwrap();
    std::fs::write(&md, old.replace("moai status", "moai 낡은명령")).unwrap();

    let out = ok(s.path(), &["init"]);
    assert!(out.contains("이미 심겨 있다"), "{out}");
    assert!(std::fs::read_to_string(&md).unwrap().contains("moai status"), "블록이 안 맞춰졌다");
    // 이슈와 저널은 그대로다
    assert!(issues(s.path()).contains(&id), "이슈를 지웠다");
    assert!(!journal(s.path()).is_empty());
}

/// 접두어는 처음 한 번만. 바꾸면 이미 발급된 id 가 제 접두어를 잃는다.
#[test]
fn init_refuses_to_change_the_prefix() {
    let s = init("reprefix");
    let out = moai(s.path(), &["init", "다른것"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("나중에 못 바꾼다"));
    // 같은 접두어면 그냥 맞춘다
    assert!(moai(s.path(), &["init", "argos"]).status.success());
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
                .env("MOAI_ACTOR", "테스터 (tester@example.com)")
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
        (vec!["add", "   "], "제목이 없다"),
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
    assert!(j.contains(r#""kind":"status","by":"테스터","by_email":"tester@example.com","from":"todo","to":"in_progress""#), "{j}");
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

    // 비우는 낱말은 `none` **하나다.** 빈 값도 비우기로 치면 `-e ""` 한 번에
    // 소속이 조용히 날아가고, 그것은 오타와 구분되지 않는다.
    ok(s.path(), &["edit", &id, "-e", &epic]);
    let out = moai(s.path(), &["edit", &id, "-e", ""]);
    assert!(!out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(line_of(s.path(), &id).contains(&format!(r#""epic":"{epic}""#)));

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

/// 막던 이슈를 지우면 그 사실도 알린다. 에픽·부모만 보고 `blocked_by` 를
/// 빠뜨리면, 방금 제 손으로 만든 끊긴 참조를 조용히 넘긴 것이 된다.
#[test]
fn rm_names_the_blocks_it_broke() {
    let s = init("rmblock");
    let a = add(s.path(), &["막는 것"]);
    let b = add(s.path(), &["막히는 것"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);

    let out = moai(s.path(), &["rm", &a]);
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("끊긴 참조") && err.contains(&b), "{err}");
    // 막지는 않는다 — 남은 참조는 그대로 두고 `status` 가 드러낸다
    assert!(line_of(s.path(), &b).contains("blocked_by"));
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

    // 같은 뜻을 한 문자열로도 쓸 수 있다. **둘 다 비지 않았음을 먼저 본다** —
    // 안 그러면 양쪽이 다 `없다.` 인 실패를 이 단언이 통과시킨다.
    let flags = ok(s.path(), &["show", "-s", "review"]);
    let string = ok(s.path(), &["show", "--filter", "status=review"]);
    assert!(flags.contains(&a), "{flags}");
    assert_eq!(flags, string);

    // 그리고 두 표현은 **쌓인다** — 덮어쓰면 `-t bug` 가 조용히 사라진다
    let both = ok(s.path(), &["show", "-t", "bug", "--filter", "tag=parser"]);
    assert!(both.contains(&b) && !both.contains(&a), "{both}");

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

// ── S3 — 에픽 · 부모-자식 · ready ─────────────────────────────────────

/// 에픽 밑에 이슈, 그 밑에 자식. 한 번에 세운다.
fn a_small_tree(s: &Scratch) -> (String, String, String, String) {
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let member = add(s.path(), &["원자적 쓰기", "-e", &epic, "-t", "bug"]);
    let child = add(s.path(), &["회귀 테스트", "--parent", &member]);
    let loose = add(s.path(), &["떠 있는 것"]);
    (epic, member, child, loose)
}

#[test]
fn tree_nests_and_does_not_repeat() {
    let s = init("tree");
    let (epic, member, child, loose) = a_small_tree(&s);
    let out = ok(s.path(), &["show", "--tree"]);

    assert_eq!(out.matches(&child).count(), 1, "자식이 두 번 나왔다\n{out}");
    let at = |t: &str| out.lines().position(|l| l.contains(t)).unwrap();
    assert!(at(&epic) < at(&member) && at(&member) < at(&child), "{out}");
    assert!(at(&loose) > at(&child), "{out}");
    assert!(out.contains("에픽 없음  1건"), "{out}");
}

/// 자식은 소속을 조상에게서 물려받는다 — 진행률도 그 수를 센다.
#[test]
fn a_child_counts_toward_its_ancestors_epic() {
    let s = init("inherit");
    let (epic, member, _child, _) = a_small_tree(&s);
    assert!(ok(s.path(), &["show", "--tree"]).contains("0/2"), "손자를 안 셌다");
    ok(s.path(), &["mv", &member, "done"]);
    let out = ok(s.path(), &["show", &epic]);
    assert!(out.contains("멤버   1/2"), "{out}");
    // 이력은 언제나 맨 끝이다
    let lines: Vec<&str> = out.lines().collect();
    let at_hist = lines.iter().position(|l| l.contains("이력")).unwrap();
    assert!(lines[at_hist..].iter().all(|l| !l.contains("멤버   ")), "{out}");
}

/// 멤버 없는 에픽은 0% 가 아니다 — "아직 안 한 것" 과 "속을 안 채운 것" 은 다르다.
#[test]
fn an_empty_epic_reads_as_empty_not_zero() {
    let s = init("emptyepic");
    add(s.path(), &["아직 빈 에픽", "--type", "epic"]);
    let out = ok(s.path(), &["show", "--tree"]);
    assert!(out.contains("0/0") && out.contains("자식 없음"), "{out}");
    assert!(!out.contains("0%"), "{out}");
}

#[test]
fn the_list_names_the_epic_not_its_id() {
    let s = init("epiccol");
    let (epic, _m, _c, _l) = a_small_tree(&s);
    let out = ok(s.path(), &["show"]);
    assert!(out.contains("저장 계층"), "{out}");
    // 멤버 줄에 에픽 id 가 그대로 실리지 않는다
    let member_line = out.lines().find(|l| l.contains("원자적 쓰기")).unwrap();
    assert!(!member_line.contains(&epic), "{member_line}");
}

#[test]
fn ready_picks_what_can_be_started() {
    let s = init("ready");
    let (epic, member, child, loose) = a_small_tree(&s);
    let out = ok(s.path(), &["ready"]);

    // 에픽 자체는 집는 것이 아니고, 자식이 남은 부모도 아니다
    assert!(!out.contains(&epic), "에픽을 집으라고 했다\n{out}");
    assert!(!out.contains(&format!("{member} ")), "자식 남은 부모를 집으라고 했다\n{out}");
    assert!(out.contains(&child) && out.contains(&loose), "{out}");
    assert!(out.contains("2건"), "{out}");
    // 어디에 속한 일인지 말한다
    assert!(out.contains("저장 계층") && out.contains("에픽 없음"), "{out}");

    // 집으면 목록에서 빠지고, 대신 "이미 잡고 있는 것" 으로 뜬다
    ok(s.path(), &["mv", &child, "in_progress"]);
    let out = ok(s.path(), &["ready"]);
    assert!(!out.contains(&format!("{child} ")), "{out}");
    assert!(out.contains("이미 잡고 있는 것 1건"), "{out}");

    // 자식이 끝나면 부모를 집을 수 있다
    ok(s.path(), &["mv", &child, "done"]);
    assert!(ok(s.path(), &["ready"]).contains(&member));
}

#[test]
fn ready_and_tree_speak_json() {
    let s = init("s3json");
    let (epic, _m, _c, _l) = a_small_tree(&s);
    one_json_value(&ok(s.path(), &["ready", "--json"]));
    one_json_value(&ok(s.path(), &["show", &epic, "--json"]));
    assert!(ok(s.path(), &["show", &epic, "--json"]).contains("\"members\""));
    // --tree 는 보기 방식일 뿐이라 --json 은 같은 배열을 낸다
    assert_eq!(
        ok(s.path(), &["show", "--json"]),
        ok(s.path(), &["show", "--tree", "--json"])
    );
}

/// 하나를 콕 집은 자리에 목록용 플래그를 주면 조용히 버리지 않는다.
#[test]
fn tree_is_refused_on_a_single_issue() {
    let s = init("treeone");
    let (_e, member, _c, _l) = a_small_tree(&s);
    let out = moai(s.path(), &["show", &member, "--tree"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--tree"));
}

// ── S4 — moai status ─────────────────────────────────────────────────

fn at(dir: &Path, now: &str, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
        .env("MOAI_NOW", now)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

/// **경고가 있어도 종료 코드는 0 이다.**
///
/// 여기서 비영으로 끝내면 에이전트가 이것을 "실패" 로 읽고, 그러면 이건
/// 린트고, 린트는 곧 게이트다. 이전 시도가 정확히 그것으로 죽었다.
/// 이 테스트를 고치려는 사람은 그 사실부터 다시 읽어야 한다.
#[test]
fn status_warns_without_blocking() {
    let s = init("status");
    for n in 0..6 {
        add(s.path(), &[&format!("떠 있는 것 {n}")]);
    }
    let out = at(s.path(), NOW, &["status"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("에픽 없는 이슈 6건"), "{text}");
    assert!(out.status.success(), "경고를 보고 비영으로 끝냈다");
}

#[test]
fn status_says_so_when_nothing_is_wrong() {
    let s = init("clean");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let out = ok(s.path(), &["status"]);
    assert!(out.contains("드러난 문제 없다"), "{out}");
}

/// 깨진 데이터만 종료 코드를 바꾼다.
#[test]
fn only_broken_data_makes_status_fail() {
    let s = init("broken_status");
    let id = add(s.path(), &["제목"]);
    assert!(at(s.path(), NOW, &["status"]).status.success());

    // 같은 줄을 한 번 더 — 머지를 잘못 풀었을 때 나오는 모양
    let path = s.path().join(".moai/issues.jsonl");
    let line = line_of(s.path(), &id);
    std::fs::write(&path, format!("{line}\n{line}\n")).unwrap();

    let out = at(s.path(), NOW, &["status"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("id 가 두 번 있다"));
    assert!(!out.status.success(), "깨진 데이터를 보고 0 으로 끝냈다");
}

/// `warnings[].kind` 가 타입 붙은 열거값이라 받는 쪽이 산문을 안 읽는다.
#[test]
fn status_json_is_machine_shaped() {
    let s = init("statusjson");
    let epic = add(s.path(), &["빈 에픽", "--type", "epic"]);
    for n in 0..6 {
        add(s.path(), &[&format!("떠 있는 것 {n}")]);
    }
    let out = ok(s.path(), &["status", "--json"]);
    one_json_value(&out);
    for want in [
        "\"kind\":\"no_epic\"",
        "\"kind\":\"empty_epic\"",
        "\"hint\":\"moai show -e none\"",
        "\"flow\"",
        "\"counts\"",
        "\"fatal\":false",
    ] {
        assert!(out.contains(want), "{want} 가 없다\n{out}");
    }
    assert!(out.contains(&epic), "{out}");
}

/// 시계가 가는 것만으로 드러나는 것들 — 고정 시계 없이는 시험할 수 없다.
#[test]
fn time_alone_surfaces_rot() {
    let s = init("rot");
    let id = add(s.path(), &["리뷰에 둔 것"]);
    at(s.path(), "2026-09-01T00:00:00Z", &["mv", &id, "review"]);

    let soon = String::from_utf8(at(s.path(), "2026-09-02T00:00:00Z", &["status"]).stdout).unwrap();
    assert!(!soon.contains("review 에"), "{soon}");

    let later = String::from_utf8(at(s.path(), "2026-09-10T00:00:00Z", &["status"]).stdout).unwrap();
    assert!(later.contains("review 에 3일 넘게 멈춘 것 1건"), "{later}");
    assert!(later.contains("moai show -s review --stale 3"), "{later}");
}

/// `moai status` 가 이 도구의 prime 이다 — 세션 시작에 치는 명령 하나.
/// 보드·경고·흐름·다음 행동이 한 화면에 다 있어야 그 노릇을 한다.
#[test]
fn status_is_one_screen_with_everything() {
    let s = init("prime");
    add(s.path(), &["제목"]);
    let out = ok(s.path(), &["status"]);
    assert!(out.contains("todo") && out.contains("done"), "보드가 없다\n{out}");
    assert!(out.contains("최근 7일"), "흐름이 없다\n{out}");
    assert!(out.contains("moai ready"), "다음 행동이 없다\n{out}");
    assert!(out.contains(".moai/issues.jsonl"), "어느 저장소인지 안 말한다\n{out}");
}

// ── S5 — 한 번에 만들기 · AGENTS.md ──────────────────────────────────

fn from_stdin(dir: &Path, args: &[&str], input: &str) -> Output {
    use std::io::Write as _;
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

const PLAN: &str = "\
# 저장 계층
- [p1] 원자적으로 쓴다 #enhancement
- 잘린 줄을 복구한다 #bug
# CLI 표면
- [p3] --json 이 tags 를 빠뜨린다 #bug
";

/// 사람이 \"좋다\" 한 순간 에픽과 이슈가 **한 번의 호출로** 선다.
#[test]
fn a_whole_plan_lands_in_one_call() {
    let s = init("bulk");
    let out = from_stdin(s.path(), &["add", "--from", "-"], PLAN);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("에픽 2 · 이슈 3"), "{text}");

    assert_eq!(issues(s.path()).lines().count(), 5);
    // 이슈가 제 에픽에 붙었고, 태그와 우선순위가 살아 있다
    let tree = ok(s.path(), &["show", "--tree"]);
    assert!(tree.contains("저장 계층") && tree.contains("CLI 표면"), "{tree}");
    assert!(!tree.contains("에픽 없음"), "떠 있는 것이 생겼다\n{tree}");
    assert!(issues(s.path()).contains(r#""priority":1"#));
    assert!(issues(s.path()).contains(r#""tags":["enhancement"]"#));
    // 만든 수만큼 저널에 남는다
    assert_eq!(journal(s.path()).lines().count(), 5);
}

/// heredoc 오타로 여섯 개를 잘못 만드는 것을 막는 것이 `--dry-run` 이다.
#[test]
fn dry_run_writes_nothing() {
    let s = init("dryrun");
    let out = from_stdin(s.path(), &["add", "--from", "-", "--dry-run"], PLAN);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("만들 것"));
    assert_eq!(issues(s.path()), "", "만들지 말라는데 만들었다");
    assert_eq!(journal(s.path()), "", "만들지 말라는데 저널에 적었다");
}

/// 못 읽은 줄은 **전부** 모아 한 번에 말한다. 하나씩 고치게 하면 여섯 줄짜리
/// heredoc 을 여섯 번 다시 보낸다. 그리고 하나라도 틀리면 아무것도 안 만든다.
#[test]
fn a_bad_plan_names_every_line_and_writes_nothing() {
    let s = init("badplan");
    let out = from_stdin(
        s.path(),
        &["add", "--from", "-"],
        "# 가\n이건 뭔가\n- [p9] 너무 큼\n- \n",
    );
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    for want in ["2줄", "3줄", "4줄"] {
        assert!(err.contains(want), "{want} 가 없다\n{err}");
    }
    assert_eq!(issues(s.path()), "");
}

/// 계획 없이 쌓이는 것을 막는 도구라, 대량 생성이 바로 그 자리다.
#[test]
fn bulk_refuses_issues_with_no_epic() {
    let s = init("noepicbulk");
    let out = from_stdin(s.path(), &["add", "--from", "-"], "- 그냥 하나\n");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("어느 에픽의"));
}

#[test]
fn init_writes_an_agents_block() {
    let s = init("agents");
    let md = std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap();
    assert!(md.contains("<!-- moai:begin -->") && md.contains("<!-- moai:end -->"));
    assert!(md.contains("moai status") && md.contains("승인 게이트는 없다"), "{md}");
    assert!(md.contains("--from"), "한 번에 만드는 법이 빠졌다\n{md}");
    // 치트시트가 낡으면 새 세션의 에이전트가 있는 명령을 모른다.
    assert!(md.contains("moai tui"), "치트시트가 낡았다\n{md}");

    let off = Scratch::new("noagents");
    ok(off.path(), &["init", "argos", "--no-agents"]);
    assert!(!off.path().join("AGENTS.md").exists());
}

/// 남의 산문은 한 글자도 건드리지 않는다.
#[test]
fn init_keeps_what_someone_else_wrote() {
    let s = Scratch::new("keepprose");
    let mine = "# 우리 규약\n\n손으로 쓴 것.\n";
    std::fs::write(s.path().join("AGENTS.md"), mine).unwrap();
    ok(s.path(), &["init", "argos"]);
    let md = std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap();
    assert!(md.starts_with(mine), "{md}");
    assert_eq!(md.matches("<!-- moai:begin -->").count(), 1);
}

/// **훑기 목록이 명령을 빠뜨리면 여기서 걸린다.**
///
/// 아래 `every_command_still_speaks_json` 은 손으로 적은 목록을 돈다. 그것만으로는
/// 새 명령을 더해도 아무것도 안 잡히는데, CLAUDE.md 는 "저절로 잡는다" 고 적고
/// 있었다 — 틀린 문장을 믿고 안전망이 있다고 여기는 것이 제일 나쁘다.
///
/// 그래서 **명령 목록을 바이너리에게 묻는다.** 도움말이 곧 그 목록이라 이 시험은
/// 새 명령이 생기는 순간 이름을 대며 실패한다. clap 을 dev-dependency 로 끌어올
/// 필요도 없다.
#[test]
fn the_json_sweep_covers_every_command() {
    let s = init("sweepcover");
    let help = ok(s.path(), &["--help"]);
    let listed: Vec<String> = help
        .lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        // `help` 는 clap 이 제 손으로 만드는 것이라 `--json` 이 뜻이 없다.
        .filter(|c| c != "help")
        .collect();
    assert!(listed.len() > 5, "명령 목록을 못 읽었다 — {listed:?}");

    for cmd in &listed {
        assert!(
            JSON_SWEEP.contains(&cmd.as_str()),
            "`{cmd}` 가 --json 훑기에서 빠졌다. \
             every_command_still_speaks_json 의 JSON_SWEEP 에 넣는다"
        );
    }
}

/// `--json` 훑기가 실제로 부르는 명령들. **위 시험이 이 목록을 도움말과 견준다** —
/// 목록을 여기 한 자리에 두어야 그 견줌이 뜻을 갖는다.
const JSON_SWEEP: &[&str] = &[
    "init", "add", "status", "ready", "show", "note", "link", "tui", "edit", "mv", "rm",
    // 종류 네임스페이스는 `moai <종류> show --json` 으로 같은 길을 지난다.
    "issue", "epic", "milestone", "idea",
];

/// 새 명령에 `--json` 을 빠뜨리면 여기서 걸린다.
#[test]
fn every_command_still_speaks_json() {
    let s = Scratch::new("jsonsweep");
    one_json_value(&ok(s.path(), &["init", "argos", "--json"]));
    let made = ok(s.path(), &["add", "제목", "--json"]);
    let id = field(&made, "id");
    let epic = field(&ok(s.path(), &["add", "에픽", "--type", "epic", "--json"]), "id");

    let cases = [
        vec!["status", "--json"],
        vec!["ready", "--json"],
        vec!["show", "--json"],
        vec!["show", "--tree", "--json"],
        vec!["show", &id, "--json"],
        vec!["show", &epic, "--json"],
        vec!["note", &id, "메모", "--json"],
        vec!["link", &id, "--blocks", &epic, "--json"],
        // tui 는 화면을 켜지 않고 목록만 낸다.
        vec!["tui", "--json"],
        // 종류 네임스페이스도 같은 길을 지난다.
        vec!["issue", "show", "--json"],
        vec!["epic", "show", "--json"],
        vec!["milestone", "show", "--json"],
        vec!["idea", "show", "--json"],
        vec!["edit", &id, "--tag", "bug", "--json"],
        vec!["mv", &id, "review", "--json"],
        vec!["rm", &id, "--json"],
    ];
    // **적어 둔 목록과 실제로 부르는 목록을 여기서 잇는다.** 잇지 않으면
    // `JSON_SWEEP` 에 이름만 적고 한 번도 안 부르는 명령이 생기고, 그러면
    // 위 시험은 초록인데 그 명령의 `--json` 은 아무도 안 본 것이 된다 —
    // 없는 안전망을 있다고 믿는 것이 제일 나쁘다.
    // `init` 과 `add` 는 위에서 바탕을 세우며 이미 `--json` 으로 부른다.
    for cmd in JSON_SWEEP.iter().filter(|c| !["init", "add"].contains(c)) {
        assert!(
            cases.iter().any(|a| a[0] == *cmd),
            "`{cmd}` 가 JSON_SWEEP 에는 있는데 실제로 부르지 않는다"
        );
    }
    for args in &cases {
        one_json_value(&ok(s.path(), args));
    }
    // 한 번에 만들기도 배열 하나를 낸다
    let bulk = from_stdin(s.path(), &["add", "--from", "-", "--json"], "# 가\n- 나\n");
    assert!(bulk.status.success());
    one_json_value(&String::from_utf8(bulk.stdout).unwrap());
}

// ── 리뷰 미처리 건 정리 ──────────────────────────────────────────────

/// `allow_hyphen_values` 의 대가를 막는다. 오타 난 플래그가 조용히 이슈가
/// 되면, 계획 없이 쌓이는 것을 막겠다는 도구가 제 손으로 쓰레기를 만든다.
#[test]
fn a_mistyped_flag_does_not_become_an_issue() {
    let s = init("flaglike");
    let out = moai(s.path(), &["add", "--dryrun"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("플래그로 보인다") && err.contains("moai add -- --dryrun"), "{err}");
    assert_eq!(issues(s.path()), "", "거부해 놓고 썼다");

    // 낱말이 여럿인 제목은 그대로 통과한다 — 이걸 받으려고 켠 기능이다
    add(s.path(), &["--json 이 tags 를 빠뜨린다"]);
    // `--` 를 쓴 사람은 이미 "이 뒤는 플래그가 아니다" 라고 말한 것이다
    let out = moai(s.path(), &["add", "-q", "--", "--dryrun"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(issues(s.path()).contains("--dryrun"));

    // `edit --title` 도 같은 규칙을 쓴다
    let id = add(s.path(), &["평범한 제목"]);
    assert!(!moai(s.path(), &["edit", &id, "--title", "--dryrun"]).status.success());
}

/// clap 의 `2 values required by '<id> <id>...'` 는 무엇을 빠뜨렸는지
/// 말해 주지 않는다.
#[test]
fn mv_says_what_is_missing() {
    let s = init("mvargs");
    let id = add(s.path(), &["제목"]);

    let out = moai(s.path(), &["mv", &id]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("옮길 칸을 안 적었다") && err.contains("todo, in_progress"), "{err}");

    // 그리고 `-m` 이 가운데 있어도 읽는다
    assert!(ok(s.path(), &["mv", &id, "-m", "메모", "review"]).contains("todo → review"));
}

/// `--json` 의 `code` 는 받는 쪽이 분기하는 값이다. 명령마다 다르면 계약이
/// 아니다 — 예전에는 `show` 만 `not_found` 를 냈다.
#[test]
fn the_same_failure_gets_the_same_code() {
    let s = init("codes");
    for args in [
        vec!["show", "argos-0000", "--json"],
        vec!["note", "argos-0000", "메모", "--json"],
        vec!["edit", "argos-0000", "--title", "x", "--json"],
        vec!["add", "x", "--parent", "argos-0000", "--json"],
    ] {
        let out = moai(s.path(), &args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(r#""code":"not_found""#), "{args:?} → {err}");
    }
}

/// 되풀이해 불러도 같은 자리면 그만이다. `mv` 가 0 으로 끝나는데 `edit` 만
/// 1 로 끝나면 받는 쪽이 재시도를 못 짠다.
#[test]
fn doing_nothing_is_not_a_failure() {
    let s = init("noop");
    let id = add(s.path(), &["제목"]);
    ok(s.path(), &["edit", &id, "--tag", "bug"]);
    let out = moai(s.path(), &["edit", &id, "--tag", "bug"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("바뀐 것이 없다"));

    ok(s.path(), &["mv", &id, "done"]);
    assert!(moai(s.path(), &["mv", &id, "done"]).status.success());
}

/// 사람 출력에는 있는데 기계 출력에만 없으면, 받는 쪽이 두 표면 중 하나를
/// 못 믿게 된다. 그 뒤처리를 할 쪽이 바로 그 기계다.
#[test]
fn json_reports_the_whole_outcome() {
    let s = init("jsonout");
    let epic = add(s.path(), &["에픽", "--type", "epic"]);
    let member = add(s.path(), &["멤버", "-e", &epic]);

    let out = moai(s.path(), &["mv", &member, "argos-0000", "review", "--json"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"moved\"") && text.contains("\"already\""), "{text}");
    assert!(text.contains(r#""missing":["argos-0000"]"#), "{text}");
    assert!(!out.status.success(), "못 찾은 것이 있는데 0 으로 끝났다");

    let out = ok(s.path(), &["rm", &epic, "--json"]);
    assert!(out.contains("\"removed\""), "{out}");
    assert!(out.contains(&format!(r#""dangling":["{member}"]"#)), "{out}");
}

/// `--filter` 를 `;` 로 쪼개면 grep 에 세미콜론을 못 쓴다. 여러 개는 플래그를
/// 되풀이한다.
#[test]
fn a_semicolon_is_a_letter_not_a_separator() {
    let s = init("semicolon");
    add(s.path(), &["a;b 가 깨진다"]);
    add(s.path(), &["멀쩡한 것"]);
    let out = ok(s.path(), &["show", "--filter", "grep=a;b"]);
    assert!(out.contains("a;b 가 깨진다") && !out.contains("멀쩡한 것"), "{out}");
}

// ── 2단계 — 마일스톤 ─────────────────────────────────────────────────

/// 마일스톤은 에픽과 이슈를 담는다. 사용자가 처음부터 물었던 그림이다.
#[test]
fn a_milestone_holds_epics_and_issues() {
    let s = init("milestone");
    let m = add(s.path(), &["v0.1", "--type", "milestone"]);
    let epic = add(s.path(), &["저장 계층", "--type", "epic", "--milestone", &m]);
    let member = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let child = add(s.path(), &["회귀 테스트", "--parent", &member]);
    let direct = add(s.path(), &["에픽 없이 바로", "--milestone", &m]);
    let outside = add(s.path(), &["아무 데도"]);

    let out = ok(s.path(), &["show", &m]);
    for want in [&epic, &member, &child, &direct] {
        assert!(out.contains(want.as_str()), "{want} 가 없다\n{out}");
    }
    assert!(!out.contains(&outside), "{out}");
    // 진척은 **일** 만 센다 — 에픽은 묶음이라 안 센다
    assert!(out.contains("멤버   0/3"), "{out}");

    // 종류만 따로 볼 수 있다
    assert!(ok(s.path(), &["show", "milestone"]).contains(&m));
    assert!(!ok(s.path(), &["show", "epic"]).contains(&m));
}

/// `--milestone` 은 물려받은 것까지 고른다. 트리가 보여 주는 것과 목록이
/// 고르는 것이 달라지면 둘 중 하나를 못 믿게 된다.
#[test]
fn the_milestone_filter_sees_what_was_inherited() {
    let s = init("mfilter");
    let m = add(s.path(), &["v0.1", "--type", "milestone"]);
    let epic = add(s.path(), &["저장 계층", "--type", "epic", "--milestone", &m]);
    let member = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let outside = add(s.path(), &["아무 데도"]);

    let out = ok(s.path(), &["show", "--milestone", &m]);
    assert!(out.contains(&member) && !out.contains(&outside), "{out}");
    assert_eq!(out, ok(s.path(), &["show", "--filter", &format!("milestone={m}")]));

    let none = ok(s.path(), &["show", "--milestone", "none"]);
    assert!(none.contains(&outside) && !none.contains(&member), "{none}");
}

/// 마일스톤을 안 쓰면 그 이야기를 꺼내지 않는다. 안 쓰는 기능으로 잔소리하지 않는다.
#[test]
fn status_mentions_milestones_only_when_used() {
    let s = init("mstatus");
    add(s.path(), &["제목"]);
    assert!(!ok(s.path(), &["status"]).contains("마일스톤"), "안 쓰는데 꺼냈다");

    let m = add(s.path(), &["v0.1", "--type", "milestone"]);
    let out = ok(s.path(), &["status"]);
    assert!(out.contains("마일스톤") && out.contains(&m), "{out}");
    assert!(out.contains("마일스톤에 안 붙은 이슈 1건"), "{out}");
    // 묶음은 보드에 세지 않는다
    assert!(out.contains("이슈 1"), "{out}");
}

/// `moai <종류> <동사>` 규칙이 디스패치 코드 0줄로 따라온다.
#[test]
fn the_milestone_namespace_costs_nothing() {
    let s = init("mns");
    let m = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    assert!(line_of(s.path(), &m).contains(r#""kind":"milestone""#));
    assert!(ok(s.path(), &["milestone", "show"]).contains(&m));
}

/// 본문은 그려서 내고, `--raw` 는 파일에 있는 그대로 낸다.
/// **`--json` 의 body 는 언제나 원문이다** — 기계가 읽는 것을 그려서 주면 안 된다.
#[test]
fn the_body_is_drawn_but_the_raw_text_stays_reachable() {
    let s = init("mdraw");
    let id = add(s.path(), &["제목", "-b", "**굵게** 와 `코드`\n\n- 하나\n- 둘"]);

    let drawn = ok(s.path(), &["show", &id]);
    assert!(!drawn.contains("**굵게**"), "기호가 그대로 남았다\n{drawn}");
    assert!(drawn.contains('•'), "목록 글머리가 없다\n{drawn}");
    // 색을 꺼도 코드는 코드로 남는다
    assert!(drawn.contains("`코드`"), "색 없이 코드를 못 가린다\n{drawn}");

    let raw = ok(s.path(), &["show", &id, "--raw"]);
    assert!(raw.contains("**굵게**") && raw.contains("- 하나"), "원문이 아니다\n{raw}");

    let json = ok(s.path(), &["show", &id, "--json"]);
    assert!(json.contains("**"), "--json 의 body 가 그려져 나왔다\n{json}");

    // **두 길 다 제어문자를 걸러 낸다.** 파일에 심긴 ESC 는 `--raw` 를 타고도
    // 화면에 닿지 못한다 — 닿으면 그 줄이 커서를 옮기고 화면을 지운다.
    let esc = add(s.path(), &["제어문자", "-b", "앞\u{1b}[2J\u{7}뒤"]);
    for args in [vec!["show", &esc], vec!["show", &esc, "--raw"]] {
        let out = ok(s.path(), &args);
        assert!(!out.contains('\u{1b}'), "ESC 가 나갔다 — {args:?}\n{out:?}");
        assert!(!out.contains('\u{7}'), "벨이 나갔다 — {args:?}\n{out:?}");
    }

    // 목록 자리의 `--raw` 는 아무 일도 못 한다. 말없이 먹지 않고 거절한다 —
    // 필터를 조용히 버리지 않는 것과 같은 까닭이다.
    let out = moai(s.path(), &["show", "--raw"]);
    assert!(!out.status.success(), "목록 자리의 --raw 를 말없이 먹었다");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--raw"),
        "무엇이 문제인지 말하지 않았다\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **하나를 집은 자리에서 버려지는 필터가 없다.** 걸러지지 않은 그 이슈를
/// 그대로 내면 부르는 쪽은 걸러진 결과라고 믿는다 — `--milestone` 이 그렇게
/// 목록에서만 뜻이 있는데 거절 목록에 빠져 있었다.
#[test]
fn every_list_only_filter_is_refused_on_a_single_issue() {
    let s = init("onefilter");
    let id = add(s.path(), &["제목"]);
    // **`FilterArgs` 의 필드마다 하나씩 있다.** 빠뜨린 필드는 `first_given`
    // 에서 빠져도 아무도 못 보고, 그것이 `--milestone` 에게 실제로 일어났다.
    for flag in [
        vec!["-s", "todo"],
        vec!["-t", "bug"],
        vec!["--no-tag", "bug"],
        vec!["-e", "none"],
        vec!["--parent", "argos-0001"],
        vec!["-p", "1"],
        vec!["--type", "issue"],
        vec!["-g", "제"],
        vec!["--stale", "3"],
        vec!["--all"],
        vec!["--filter", "status=todo"],
        vec!["--milestone", "없는것"],
        vec!["--tree"],
    ] {
        let mut args = vec!["show", id.as_str()];
        args.extend(flag.iter().copied());
        let out = moai(s.path(), &args);
        assert!(!out.status.success(), "{flag:?} 를 말없이 버렸다");
        // **무엇을 버렸는지까지 말한다.** 거절만 보면 엉뚱한 까닭(잘못된 값,
        // 없는 id)으로 실패해도 통과하고, 그러면 이 시험은 자기가 지키려던
        // 것을 안 지킨다.
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(said.contains(flag[0]), "{flag:?} 를 거절하며 그 이름을 안 짚었다 — {said}");
    }
}

/// **트리는 모든 줄을 정확히 한 번 낸다.** 자리를 정하는 코드가 둘이면
/// 어긋나고, 실제로 어긋났다 — 제 에픽이 부모와 다른 자식은 두 번 나왔고
/// 끊긴 참조를 가진 줄은 아예 사라졌다.
#[test]
fn the_tree_shows_every_issue_exactly_once() {
    let s = init("treeonce");
    let ga = add(s.path(), &["에픽 가", "--type", "epic"]);
    let na = add(s.path(), &["에픽 나", "--type", "epic"]);
    let member = add(s.path(), &["가의 멤버", "-e", &ga]);
    let child = ok(s.path(), &["add", "자식인데 에픽이 다르다", "--parent", &member, "-e", &na, "-q"])
        .trim()
        .to_string();
    let inherits = ok(s.path(), &["add", "물려받는 자식", "--parent", &member, "-q"]).trim().to_string();
    let loose = add(s.path(), &["소속 없는 일"]);
    let dangling = add(s.path(), &["없는 에픽을 가리킨다"]);
    ok(s.path(), &["edit", &dangling, "-e", "argos-zzzz"]);
    let wrong = add(s.path(), &["에픽 아닌 것을 에픽이라 한다"]);
    ok(s.path(), &["edit", &wrong, "-e", &member]);

    let tree = ok(s.path(), &["show", "--tree", "--all"]);
    // **id 가 자식 id 의 앞부분이기도 하다** — `argos-x` 는 `argos-x.aa1` 안에도
    // 들어 있다. 뒤에 점이 붙지 않은 것만 그 줄로 센다.
    let times = |id: &str| {
        tree.match_indices(id)
            .filter(|(at, _)| !tree[at + id.len()..].starts_with('.'))
            .count()
    };
    for id in [&ga, &na, &member, &child, &inherits, &loose, &dangling, &wrong] {
        let n = times(id);
        assert_eq!(n, 1, "{id} 가 트리에 {n}번 나온다\n{tree}");
    }
}

/// **머리글은 그 밑의 줄과 맞는다.** 소속 없는 줄이 남의 에픽 바로 밑에
/// 같은 들여쓰기로 끼면 그 에픽의 멤버로 읽히고, 자식을 거느린 줄만 잎에서
/// 빠져 셈이 모자라면 머리글이 제 밑을 거짓으로 센다.
#[test]
fn the_tree_groups_every_loose_row_under_one_honest_header() {
    let s = init("treehead");
    let m = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let inside = ok(s.path(), &["add", "에픽 안", "--type", "epic", "--milestone", &m, "-q"])
        .trim()
        .to_string();
    ok(s.path(), &["add", "안의 일", "-e", &inside, "-q"]);
    let outside =
        ok(s.path(), &["add", "에픽 밖", "--type", "epic", "-q"]).trim().to_string();
    ok(s.path(), &["add", "밖의 일", "-e", &outside, "-q"]);
    let parent = add(s.path(), &["소속 없는 부모"]);
    ok(s.path(), &["add", "그 자식", "--parent", &parent, "-q"]);
    let leaf = add(s.path(), &["소속 없는 잎"]);

    let tree = ok(s.path(), &["show", "--tree", "--all"]);
    // 바구니는 제 이름으로 불린다 — "에픽 없음" 집계를 빌려 쓰면 안 된다.
    assert!(tree.contains("(마일스톤 없음)"), "바구니가 제 이름을 잃었다\n{tree}");
    // 소속 없는 것은 셋(부모·자식·잎)이고, 머리글이 그렇게 말한다.
    assert!(tree.contains("에픽 없음  3건"), "소속 없는 것을 덜 셌다\n{tree}");
    // 소속 없는 부모는 머리글 **뒤에** 온다. 앞에 오면 위 에픽의 멤버로 읽힌다.
    let at = |needle: &str| tree.find(needle).unwrap_or_else(|| panic!("{needle} 이 없다\n{tree}"));
    assert!(at("에픽 없음") < at(&parent), "소속 없는 부모가 머리글 위로 샜다\n{tree}");
    assert!(at("에픽 없음") < at(&leaf), "{tree}");
    assert!(at(&outside) < at("에픽 없음"), "에픽이 소속 없는 것 뒤로 밀렸다\n{tree}");
}

/// **묶음 상세는 제 멤버를 "에픽 없음" 이라 부르지 않는다.** 트리의 뿌리에서만
/// 뜻이 있는 머리글이 에픽 밑까지 따라 내려가던 자리다.
#[test]
fn a_grouping_detail_does_not_label_its_own_members_as_loose() {
    let s = init("memberhead");
    let m = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let e = ok(s.path(), &["add", "에픽", "--type", "epic", "--milestone", &m, "-q"])
        .trim()
        .to_string();
    let a = add(s.path(), &["멤버 하나", "-e", &e]);
    ok(s.path(), &["add", "멤버 둘", "-e", &e, "-q"]);

    let detail = ok(s.path(), &["show", &e]);
    assert!(!detail.contains("에픽 없음"), "제 멤버를 에픽 없음이라 불렀다\n{detail}");
    assert!(detail.contains(&a), "멤버를 안 냈다\n{detail}");

    // 마일스톤 상세의 에픽 줄은 **집계를 낸다.** 빈 집계를 넘기면 `에픽 1건`
    // 처럼 나와 같은 에픽이 `--tree` 와 다르게 읽힌다.
    let mile = ok(s.path(), &["show", &m]);
    assert!(mile.contains("0/2"), "에픽 줄이 집계를 잃었다\n{mile}");
}

// ── TUI ────────────────────────────────────────────────────────────

/// TTY 가 아니면 화면을 켜지 않고 분명히 거절한다. 이게 없으면 파이프로 부른
/// `moai tui` 가 대체 화면을 켠 채 멈춰 서고, 테스트가 거기서 죽는다.
#[test]
fn tui_refuses_when_there_is_no_terminal() {
    let s = init("tuinotty");
    add(s.path(), &["제목"]);
    let out = moai(s.path(), &["tui"]);
    assert!(!out.status.success(), "TTY 없이도 켜려 들었다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("터미널"), "{err}");
}

/// `--json` 은 화면을 켜지 않고 그 디렉터리 목록을 낸다. 이것이 실제 바이너리로
/// 트리를 훑는 손잡이다.
#[test]
fn tui_json_lists_a_directory_without_a_terminal() {
    let s = init("tuijson");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let member = add(s.path(), &["원자적으로 쓴다", "-e", &epic]);
    let loose = add(s.path(), &["아무 데도 안 딸린 것"]);

    // 뿌리: 마일스톤이 없으므로 에픽과 소속 없는 이슈가 나란히 선다
    let root = ok(s.path(), &["tui", "--json"]);
    one_json_value(&root);
    assert!(root.contains(&epic) && root.contains(&loose), "{root}");
    assert!(!root.contains(&member), "멤버가 뿌리에 새어 나왔다 — {root}");

    // 에픽 안으로 들어가면 그 멤버가 나온다
    let inside = ok(s.path(), &["tui", "--json", "--path", &epic]);
    one_json_value(&inside);
    assert!(inside.contains(&member), "{inside}");
}

/// **멤버 없는 에픽도 디렉터리다.** 비었다고 부모를 대신 열면 그 에픽을 물은
/// 답으로 형제들이 나오고, `dir` 을 보고 파고드는 쪽은 제자리를 돈다.
#[test]
fn tui_json_opens_an_empty_epic_not_its_parent() {
    let s = init("tuiempty");
    let epic = add(s.path(), &["아직 안 채운 에픽", "--type", "epic"]);
    let sibling = add(s.path(), &["남"]);

    let root = ok(s.path(), &["tui", "--json"]);
    let inside = ok(s.path(), &["tui", "--json", "--path", &epic]);
    one_json_value(&inside);
    assert_ne!(inside.trim(), root.trim(), "빈 에픽을 물었더니 뿌리가 나왔다");
    assert!(!inside.contains(&sibling), "형제가 그 에픽 안에 있다 — {inside}");
    assert_eq!(inside.trim(), "[]", "{inside}");
}

/// 바구니에는 id 가 없다. **그래도 도로 넣을 손잡이를 준다** — `dir: true` 만
/// 주고 들어갈 길을 안 주면 훑는 쪽은 있는 줄 알면서 못 본다.
#[test]
fn tui_json_hands_back_a_path_for_every_row() {
    let s = init("tuipath");
    let one = add(s.path(), &["에픽 없는 것"]);
    // 없는 에픽을 가리키게 손으로 고친다 — 길 잃음 바구니가 생긴다
    let line = line_of(s.path(), &one);
    let broken = line.replace(r#""status""#, r#""epic":"argos-zzzz","status""#);
    let file = s.path().join(".moai/issues.jsonl");
    std::fs::write(&file, format!("{broken}\n")).unwrap();

    let root = ok(s.path(), &["tui", "--json"]);
    assert!(root.contains(r#""kind":"bucket""#), "{root}");
    assert!(root.contains(r#""path":"길잃음""#), "바구니에 손잡이가 없다 — {root}");

    let inside = ok(s.path(), &["tui", "--json", "--path", "길잃음"]);
    assert!(inside.contains(&one), "{inside}");

    // 없는 바구니는 조용한 빈 목록이 아니라 거절이다
    let out = moai(s.path(), &["tui", "--json", "--path", "없음"]);
    assert!(!out.status.success(), "없는 바구니를 열어 주었다");
}

/// **못 읽은 줄을 삼키지 않는다.** 다른 읽기 명령과 같이 stderr 로 알리고
/// 0 이 아닌 값으로 끝난다 — 조용히 짧아진 목록이 이 도구의 유일한 금기다.
#[test]
fn tui_json_tells_about_unreadable_lines() {
    let s = init("tuibad");
    add(s.path(), &["멀쩡한 것"]);
    let file = s.path().join(".moai/issues.jsonl");
    let mut src = std::fs::read_to_string(&file).unwrap();
    src.push_str("{ 이건 JSON 이 아니다\n");
    std::fs::write(&file, src).unwrap();

    let out = moai(s.path(), &["tui", "--json"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("읽을 수 없는 줄"), "조용히 넘어갔다 — {err}");
    assert!(!out.status.success(), "일부만 읽고도 성공이라고 했다");
}

// ── 막음 (`moai link`) ─────────────────────────────────────────────

/// A 가 B 를 막으면 B 의 `blocked_by` 에 A 가 적히고, B 는 `ready` 에서 빠진다.
#[test]
fn link_blocks_writes_to_the_blocked_side_and_ready_excludes_it() {
    let s = init("link");
    let a = add(s.path(), &["막는 것"]);
    let b = add(s.path(), &["막히는 것"]);

    ok(s.path(), &["link", &a, "--blocks", &b]);
    assert!(line_of(s.path(), &b).contains(&format!(r#""blocked_by":["{a}"]"#)), "{}", line_of(s.path(), &b));
    assert!(!line_of(s.path(), &a).contains("blocked_by"), "막는 쪽에는 안 적힌다");

    let ready = ok(s.path(), &["ready"]);
    assert!(ready.contains(&a) && !ready.contains(&b), "{ready}");

    ok(s.path(), &["mv", &a, "done"]);
    let ready = ok(s.path(), &["ready"]);
    assert!(ready.contains(&b), "막은 것이 끝났는데도 여전히 막혀 있다 — {ready}");
}

/// `--unblocks` 는 그 막음만 없앤다.
#[test]
fn link_unblocks_removes_just_that_edge() {
    let s = init("unlink");
    let a = add(s.path(), &["a"]);
    let b = add(s.path(), &["b"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    ok(s.path(), &["link", &a, "--unblocks", &b]);
    assert!(!line_of(s.path(), &b).contains("blocked_by"));
}

/// 고리는 쓰기 전에 막는다.
#[test]
fn link_refuses_a_cycle() {
    let s = init("cycle");
    let a = add(s.path(), &["a"]);
    let b = add(s.path(), &["b"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    let out = moai(s.path(), &["link", &b, "--blocks", &a]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("고리"));
    assert!(!line_of(s.path(), &a).contains("blocked_by"), "거부했는데 썼다");
}

/// 스스로를 막는 것도 고리로 거부한다.
#[test]
fn link_refuses_self_block() {
    let s = init("selfblock");
    let a = add(s.path(), &["a"]);
    let out = moai(s.path(), &["link", &a, "--blocks", &a]);
    assert!(!out.status.success());
}

/// 같은 id 를 `--blocks` 와 `--unblocks` 에 동시에 적으면 어느 쪽도 조용히
/// 이기게 두지 않는다 — 거부한다.
#[test]
fn link_refuses_the_same_id_in_both_lists() {
    let s = init("linkconflict");
    let a = add(s.path(), &["a"]);
    let b = add(s.path(), &["b"]);
    let out = moai(s.path(), &["link", &a, "--blocks", &b, "--unblocks", &b]);
    assert!(!out.status.success());
    assert!(!line_of(s.path(), &b).contains("blocked_by"), "거부했는데 썼다");
}

/// 막던 쪽이 지워져도 남은 참조는 풀 수 있어야 한다. 못 풀면 `status` 가
/// 드러낸 것을 도구로 고칠 길이 파일 직접 편집밖에 안 남는다.
#[test]
fn link_can_unblock_after_the_blocker_was_removed() {
    let s = init("unblockgone");
    let a = add(s.path(), &["막는 것"]);
    let b = add(s.path(), &["막히는 것"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    ok(s.path(), &["rm", &a]);
    assert!(line_of(s.path(), &b).contains("blocked_by"), "사전 조건이 안 섰다");

    ok(s.path(), &["link", &a, "--unblocks", &b]);
    assert!(!line_of(s.path(), &b).contains("blocked_by"), "지워진 막음을 못 풀었다");
}

/// 없는 이슈를 막거나 막힐 수는 없다.
#[test]
fn link_refuses_unknown_ids() {
    let s = init("linkmissing");
    let a = add(s.path(), &["a"]);
    let out = moai(s.path(), &["link", &a, "--blocks", "argos-zzzz"]);
    assert!(!out.status.success());
    let out = moai(s.path(), &["link", "argos-zzzz", "--blocks", &a]);
    assert!(!out.status.success());
}

// ── 첫인상 ───────────────────────────────────────────────────────────

/// 맨몸으로 부른 것을 실패로 끝내면 처음 만난 쪽이 도구가 고장 난 줄 안다.
/// 그리고 에이전트는 exit 2 를 보고 다시 안 부른다.
#[test]
fn calling_it_bare_is_not_a_failure() {
    let s = init("bare");
    add(s.path(), &["제목"]);

    let out = moai(s.path(), &[]);
    assert!(out.status.success(), "맨몸 호출이 {:?} 로 끝났다", out.status.code());
    let text = String::from_utf8_lossy(&out.stdout);
    // 지금 무슨 상태인가
    assert!(text.contains("todo") && text.contains("최근 7일"), "{text}");
    // 다음에 무엇을 치는가
    assert!(text.contains("moai ready") && text.contains("moai --help"), "{text}");
    assert!(text.contains("AGENTS.md"), "일하는 법으로 가는 길이 없다\n{text}");
    assert!(String::from_utf8_lossy(&out.stderr).is_empty(), "{:?}", out.stderr);
}

/// 저장소 밖에서는 무엇을 할 수 있는지와 어떻게 시작하는지를 같이 낸다.
#[test]
fn outside_a_repo_it_teaches_instead_of_erroring() {
    let s = Scratch::new("bareout");
    let out = moai(s.path(), &[]);
    assert!(out.status.success(), "{:?}", out.status.code());
    let text = String::from_utf8_lossy(&out.stdout);
    for want in ["moai status", "moai ready", "add --from", "승인 게이트가 없다", "moai init"] {
        assert!(text.contains(want), "{want} 가 없다\n{text}");
    }
}

/// 처음 만난 쪽이 알아야 할 것은 둘이다 — 지금 무슨 상태인가, 다음에 무엇을
/// 치는가. `--help` 만 봐도 둘째가 나와야 한다.
#[test]
fn help_says_what_to_type_next() {
    let s = init("help");
    let out = ok(s.path(), &["--help"]);
    for want in ["moai status", "moai ready", "moai mv", "add --from", "AGENTS.md"] {
        assert!(out.contains(want), "{want} 가 없다\n{out}");
    }
    // 명령 목록도 그대로 있다
    assert!(out.contains("milestone") && out.contains("note"), "{out}");
}

/// 인자 없이 부른 것도 `--json` 이 돈다 — 에이전트가 첫 호출부터 기계로 읽는다.
#[test]
fn the_bare_call_speaks_json_too() {
    let s = init("barejson");
    add(s.path(), &["제목"]);
    let out = ok(s.path(), &["--json"]);
    one_json_value(&out);
    assert!(out.contains("\"warnings\"") && out.contains("\"flow\""), "{out}");
}

// ── 누가 하는가 ───────────────────────────────────────────────────────

/// `MOAI_ACTOR` 를 걷고 git 이 읽을 설정을 통째로 지정해 돌린다. moai 는 git
/// 저장소를 요구하지 않으므로 전역·시스템 설정까지 막아야 사람을 못 찾는
/// 상황을 실제로 만들 수 있다.
fn with_git_config(dir: &Path, cfg: &str, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env_remove("MOAI_ACTOR")
        .env("GIT_CONFIG_GLOBAL", cfg)
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

/// 아무 데서도 사람을 못 찾는 자리.
fn without_user(dir: &Path, args: &[&str]) -> Output {
    with_git_config(dir, "/dev/null", args)
}

/// 이름만으로는 같은 이름이 둘일 때 갈라지지 않는다. 저널에 메일까지 남는다.
#[test]
fn the_journal_records_a_name_and_an_email() {
    let s = init("who");
    add(s.path(), &["제목"]);
    let j = journal(s.path());
    assert!(j.contains(r#""by":"테스터","by_email":"tester@example.com""#), "{j}");
}

/// `--user` 가 설정보다 앞선다 — 사람을 부르지 않고도 이름을 댈 길이 있어야
/// 이 멈춤이 게이트가 되지 않는다.
#[test]
fn the_user_flag_wins_over_the_environment() {
    let s = init("userflag");
    ok(s.path(), &["add", "제목", "--user", "레이븐 (raven@buzzni.com)"]);
    let j = journal(s.path());
    assert!(j.contains(r#""by":"레이븐","by_email":"raven@buzzni.com""#), "{j}");
}

/// 모양이 어긋나면 조용히 이름으로 삼지 않는다 — 메일 없는 줄이 그렇게 샌다.
#[test]
fn a_malformed_user_is_refused() {
    let s = init("baduser");
    // 빈 값도 준 것이다 — `--user "$NAME"` 의 변수가 비었을 때 조용히 git 설정으로
    // 넘어가면 엉뚱한 사람 이름으로 저널이 쌓인다.
    for bad in ["레이븐", "", "  "] {
        let out = moai(s.path(), &["add", "제목", "--user", bad]);
        assert!(!out.status.success(), "{bad:?} 를 받아 버렸다");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("이름 (메일)"), "{bad:?}: {err}");
    }
    assert_eq!(issues(s.path()).lines().count(), 0, "거절했는데 줄이 남았다");
}

/// 아무 데서도 사람을 못 찾으면 멈추고 **무엇을 하라고** 말한다. 이름 없는
/// 줄을 쌓아 두면 나중에 누구도 되짚지 못한다.
#[test]
fn with_no_user_anywhere_it_says_what_to_set() {
    let s = init("nouser");
    let out = without_user(s.path(), &["add", "제목"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("git config user.name"), "{err}");
    assert!(err.contains("git config user.email"), "{err}");
    assert!(err.contains("--user"), "{err}");
    assert_eq!(issues(s.path()).lines().count(), 0);
}

/// git 이 준 값도 `--user` 와 **같은 자로 잰다.** 한쪽만 통과시키면 이 도구가
/// 스스로 모양이 아니라고 부르는 값이 되돌릴 수 없는 저널에 영구히 쌓인다.
#[test]
fn a_malformed_git_identity_is_refused_too() {
    let s = init("badgit");
    let cfg = s.path().join("gitconfig");
    std::fs::write(&cfg, "[user]\n\tname = 레이븐\n\temail = raven\n").unwrap();
    let out = with_git_config(s.path(), cfg.to_str().unwrap(), &["add", "제목"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("git config user.email"), "{err}");
    assert_eq!(issues(s.path()).lines().count(), 0, "거절했는데 줄이 남았다");
}

/// 읽기는 사람을 묻지 않는다. 물으면 설정 없는 기계에서 `moai show` 가 죽고,
/// 그건 보러 온 사람에게 도구가 고장 난 것으로 보인다.
#[test]
fn reading_never_asks_who_you_are() {
    let s = init("readonly");
    add(s.path(), &["제목"]);
    for args in [["status"], ["ready"], ["show"]] {
        let out = without_user(s.path(), &args);
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// 저널을 적는 넷이 **모두** 사람을 묻고, 넷이 모두 `--user` 로 답을 받는다.
/// 한 명령이 `ctx.user` 를 안 넘기면 컴파일은 그대로 되고 `--user` 만 조용히
/// 무시된다 — 그 구멍은 이렇게 넷을 같이 돌려야 드러난다.
#[test]
fn every_journalling_command_asks_who_and_takes_an_answer() {
    let s = init("whoall");
    let id = add(s.path(), &["시험"]);
    let each: [&[&str]; 4] =
        [&["add", "또"], &["mv", &id, "review"], &["note", &id, "메모"], &["rm", &id]];

    for args in each {
        let out = without_user(s.path(), args);
        assert!(!out.status.success(), "{args:?} 가 사람을 안 묻고 지나갔다");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("--user"), "{args:?}: {err}");
    }
    for args in each {
        let mut v = args.to_vec();
        v.extend_from_slice(&["--user", "레이븐 (raven@buzzni.com)"]);
        let out = without_user(s.path(), &v);
        assert!(out.status.success(), "{v:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let j = journal(s.path());
    for kind in ["create", "status", "note", "rm"] {
        assert!(
            j.contains(&format!(r#""kind":"{kind}","by":"레이븐","by_email":"raven@buzzni.com""#)),
            "{kind} 이 메일을 안 남겼다\n{j}"
        );
    }
}

/// `--dry-run` 은 아무것도 쓰지 않으므로 사람을 묻지 않는다. 물으면 계획을
/// 확인해 보려던 사람이 설정부터 하게 된다.
#[test]
fn a_dry_run_does_not_ask_who_you_are() {
    let s = init("dryrunwho");
    use std::io::Write as _;
    let mut child = Command::new(BIN)
        .args(["add", "--from", "-", "--dry-run"])
        .current_dir(s.path())
        .env_remove("MOAI_ACTOR")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all("# 에픽\n- 하나\n".as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(issues(s.path()).lines().count(), 0);
}

/// `-a` 를 안 주면 만든 사람이 담당이다. `none` 이면 비운다. 이름만 준 것은
/// 이름만 넣는다 — 남의 메일을 모르는 채로 맡기는 일이 실제로 있다.
#[test]
fn add_assigns_to_whoever_made_it() {
    let s = init("assign");
    let mine = add(s.path(), &["내 것"]);
    let line = line_of(s.path(), &mine);
    assert!(line.contains(r#""assignee":"테스터","assignee_email":"tester@example.com""#), "{line}");

    let nobody = add(s.path(), &["임자 없음", "-a", "none"]);
    let line = line_of(s.path(), &nobody);
    assert!(!line.contains("assignee"), "{line}");

    let hers = add(s.path(), &["남의 것", "-a", "철수"]);
    let line = line_of(s.path(), &hers);
    assert!(line.contains(r#""assignee":"철수""#) && !line.contains("assignee_email"), "{line}");
}

/// `me` 는 **어느 길로 와도** 지금 사람이다. 플래그를 넘기기 전에 풀면
/// `--filter assignee=me` 는 손이 닿지 않아 `me` 라는 이름을 찾고, 쉼표로 이은
/// 항도 같이 샌다 — 둘 다 조용히 0건이라 오타와 구별되지 않는다.
#[test]
fn me_means_me_however_it_arrives() {
    let s = init("me");
    let mine = add(s.path(), &["내 것"]);
    add(s.path(), &["남의 것", "-a", "철수"]);

    for args in [
        vec!["show", "-a", "me", "--json"],
        vec!["show", "--filter", "assignee=me", "--json"],
        vec!["show", "-a", "me,아무도아님", "--json"],
    ] {
        let out = ok(s.path(), &args);
        assert!(out.contains(&mine), "{args:?} 가 내 것을 못 찾았다: {out}");
        assert!(!out.contains("남의 것"), "{args:?} 가 남의 것까지 냈다: {out}");
    }
}

/// 대량 생성도 담당을 받는다. `--from` 이 `-a` 를 통째로 흘리던 자리다 —
/// 단건에만 붙고 계획 한 장에는 안 붙었다.
#[test]
fn a_whole_plan_gets_an_assignee_too() {
    let s = init("bulkassign");
    let out = from_stdin(s.path(), &["add", "--from", "-"], "# 에픽\n- 하나\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let all = issues(s.path());
    assert_eq!(all.matches(r#""assignee":"테스터""#).count(), 2, "{all}");
    assert_eq!(all.matches(r#""assignee_email":"tester@example.com""#).count(), 2, "{all}");
}

/// 옛 저널 줄에는 `by_email` 이 없다. `by` 를 객체로 바꿨다면 그 줄이 파싱에
/// 실패하고, 실패한 줄은 조용히 버려져 이력이 통째로 사라진다.
#[test]
fn an_old_journal_line_without_an_email_still_shows() {
    let s = init("oldjournal");
    let id = add(s.path(), &["제목"]);
    let path = s.path().join(".moai/journal.jsonl");
    let mut j = std::fs::read_to_string(&path).unwrap();
    j.push_str(&format!(
        "{{\"ts\":\"2026-09-01T00:00:00Z\",\"id\":\"{id}\",\"kind\":\"note\",\"by\":\"옛사람\",\"text\":\"옛 메모\"}}\n"
    ));
    std::fs::write(&path, j).unwrap();
    let out = ok(s.path(), &["show", &id]);
    assert!(out.contains("옛 메모") && out.contains("옛사람"), "{out}");
}

/// 표기는 **화면만** 바꾼다. 파일은 언제나 이름과 메일을 갈라서 든다 —
/// 설정 하나가 이미 쓴 줄을 바꾸면 그건 설정이 아니라 마이그레이션이다.
#[test]
fn naming_changes_the_screen_not_the_file() {
    let s = init("naming");
    let id = add(s.path(), &["표기"]);
    let before = issues(s.path());
    let path = s.path().join(".moai/config.toml");
    let set = |how: &str| {
        let src = std::fs::read_to_string(&path).unwrap();
        let kept: Vec<&str> =
            src.lines().filter(|l| !l.trim_start().starts_with("naming")).collect();
        std::fs::write(&path, format!("{}\nnaming = \"{how}\"\n", kept.join("\n"))).unwrap();
    };

    set("name");
    let out = ok(s.path(), &["show", &id]);
    assert!(out.contains("테스터") && !out.contains("tester@example.com"), "{out}");

    set("email");
    let out = ok(s.path(), &["show", &id]);
    assert!(out.contains("tester@example.com"), "{out}");

    // 오타는 조용히 통과하지 않는다 — 통과하면 왜 표기가 안 바뀌는지 못 찾는다.
    set("Full");
    let err = String::from_utf8_lossy(&moai(s.path(), &["show", &id]).stderr).into_owned();
    assert!(err.contains("full·name·email"), "{err}");

    set("full");
    assert_eq!(issues(s.path()), before, "표기를 바꿨는데 파일이 달라졌다");
}

// ── idea — 반짝 생각을 담는 칸 ──────────────────────────────────────

/// **담는 비용이 0 에 가까워야 담는다.** 제목 하나로 끝나야 하고, 우선순위도
/// 에픽도 안 물어야 한다. 하나라도 더 물으면 그 자리에서 던져 놓는 대신
/// 사람이 생각을 접는다.
#[test]
fn an_idea_costs_one_title() {
    let s = init("ideaadd");
    let id = ok(s.path(), &["idea", "add", "반짝 떠오른 것", "-q"]).trim().to_string();
    let line = line_of(s.path(), &id);
    assert!(line.contains(r#""kind":"idea""#), "{line}");
    assert!(!line.contains(r#""epic""#), "안 물은 에픽이 붙었다 — {line}");
    assert!(!line.contains(r#""priority""#), "안 물은 우선순위가 붙었다 — {line}");
}

/// 긴 생각도 한 번에 들어간다 — 본문은 `-b -` 로 stdin 에서 받는다.
#[test]
fn an_idea_takes_a_body_from_stdin() {
    let s = init("ideabody");
    let out = from_stdin(s.path(), &["idea", "add", "긴 생각", "-b", "-", "--json"], "여러\n줄\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let made = String::from_utf8(out.stdout).unwrap();
    assert!(made.contains(r#""body":"여러\n줄""#), "{made}");
}

/// `ls` 는 `show` 의 다른 이름이다. 어휘를 둘로 만들지 않으려고 별명으로 둔다 —
/// 목록을 내는 동사가 둘이면 도움말이 둘 다 가르쳐야 한다.
#[test]
fn idea_ls_is_idea_show() {
    let s = init("idealist");
    let id = ok(s.path(), &["idea", "add", "담아 둔 것", "-q"]).trim().to_string();
    let listed = ok(s.path(), &["idea", "ls"]);
    assert!(listed.contains(&id), "{listed}");
    assert_eq!(listed, ok(s.path(), &["idea", "show"]), "ls 와 show 가 다른 것을 낸다");
}

/// 고치고 버리는 것은 이미 있는 동사가 한다. `id` 가 대상을 정확히 가리키므로
/// 종류를 덧붙일 자리가 없다 — `epic`·`milestone` 이 `mv`·`edit`·`rm` 을
/// 갖지 않는 것과 같은 규칙이다.
#[test]
fn editing_and_removing_an_idea_uses_the_plain_verbs() {
    let s = init("ideaedit");
    let id = ok(s.path(), &["idea", "add", "고칠 것", "-q"]).trim().to_string();
    ok(s.path(), &["edit", &id, "--tag", "parser"]);
    assert!(line_of(s.path(), &id).contains(r#""tags":["parser"]"#));
    ok(s.path(), &["rm", &id]);
    assert!(!issues(s.path()).contains(&id), "안 지워졌다");
}

/// **idea 는 묶음이 아니다.** 상세가 `멤버 0/0` 을 내면, 아무것도 안 담을
/// 자리에 담을 것이 있다고 말하는 것이고 사람은 그 0 을 채우려 든다.
#[test]
fn an_idea_is_not_a_grouping() {
    let s = init("ideadetail");
    let id = ok(s.path(), &["idea", "add", "반짝", "-q"]).trim().to_string();
    let out = ok(s.path(), &["show", &id]);
    assert!(!out.contains("멤버"), "idea 를 묶음으로 펼쳤다 — {out}");
    assert!(out.contains("idea"), "무슨 종류인지 안 말한다 — {out}");
}

/// idea 는 일도 아니다 — 보드에도 `ready` 에도 안 든다. 여기가 조용히
/// 틀어지면 사람이 생각을 담을수록 화면이 시끄러워지고, 그러면 안 담게 된다.
#[test]
fn an_idea_stays_out_of_the_board_and_ready() {
    let s = init("ideaquiet");
    ok(s.path(), &["idea", "add", "반짝", "-q"]);
    let work = add(s.path(), &["진짜 일"]);
    let st = ok(s.path(), &["status"]);
    assert!(st.contains("이슈 1"), "idea 를 이슈로 셌다 — {st}");
    let r = ok(s.path(), &["ready"]);
    assert!(r.contains(&work), "{r}");
    assert_eq!(r.matches("argos-").count(), 1, "담아 둔 생각이 집을 일로 올라왔다 — {r}");
}

/// **기본 목록에는 안 나온다.** `moai show` 는 일을 보는 자리인데 거기에
/// 생각 조각이 섞이면 목록이 흐려지고, 흐려지면 담기가 꺼려진다.
#[test]
fn ideas_stay_out_of_the_plain_list() {
    let s = init("ideafilter");
    let thought = ok(s.path(), &["idea", "add", "반짝", "-q"]).trim().to_string();
    let work = add(s.path(), &["진짜 일"]);

    let plain = ok(s.path(), &["show"]);
    assert!(plain.contains(&work), "{plain}");
    assert!(!plain.contains(&thought), "기본 목록에 idea 가 섞였다 — {plain}");

    for args in [vec!["show", "--type", "idea"], vec!["idea", "ls"]] {
        let out = ok(s.path(), &args);
        assert!(out.contains(&thought), "{args:?} — {out}");
        assert!(!out.contains(&work), "{args:?} 가 일까지 냈다 — {out}");
    }
}

/// **글로는 찾아진다.** 이미 적어 둔 생각을 다시 안 적으려면 찾아져야 한다 —
/// 겹칠 것 같을 때 `-g` 로 훑는 것이 이 도구가 가르치는 첫 동작이다.
#[test]
fn grep_reaches_into_ideas() {
    let s = init("ideagrep");
    let thought = ok(s.path(), &["idea", "add", "파서를 다시 쓴다", "-q"]).trim().to_string();
    let out = ok(s.path(), &["show", "-g", "파서"]);
    assert!(out.contains(&thought), "적어 둔 생각을 못 찾는다 — {out}");
}

/// 모르는 종류는 조용히 0건이 되지 않는다. 오타가 "그 종류는 비었다" 와
/// 구별되지 않으면 사람은 없는 것을 찾고 있다고 믿는다.
#[test]
fn the_kind_vocabulary_names_idea() {
    let s = init("ideavocab");
    let err = String::from_utf8_lossy(&moai(s.path(), &["show", "아이디어"]).stderr).into_owned();
    assert!(err.contains("idea"), "종류 목록이 idea 를 안 댄다 — {err}");
}

/// 쌓인 생각을 `status` 가 한 줄로 비춘다. **막지 않는다** — 종료 코드가
/// 0 이 아니게 되는 순간 부르는 쪽이 이것을 실패로 읽고, 그러면 이건 린트고
/// 린트는 곧 게이트다.
#[test]
fn status_shows_a_pile_of_ideas_without_blocking() {
    let s = init("ideapile");
    for n in 0..5 {
        ok(s.path(), &["idea", "add", &format!("생각 {n}"), "-q"]);
    }
    let out = moai(s.path(), &["status"]);
    assert!(out.status.success(), "알림으로 비영 종료했다 — 그러면 이건 게이트다");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("쌓인 idea 5건"), "{text}");
    assert!(text.contains("moai idea ls"), "다음에 무엇을 칠지 안 말한다 — {text}");
    // `?` 는 보드에서 review 칸의 글리프다. 한 글자가 두 뜻을 지면 안 된다.
    assert!(text.contains("+ 쌓인 idea"), "알림이 경고 글리프를 달았다 — {text}");
}

/// **펼치면 닫힌다.** 에픽과 이슈가 생기고 그 idea 는 `done` 으로 간다 —
/// 펼쳐졌으므로 더 볼 것이 없다.
#[test]
fn promoting_an_idea_opens_a_plan_and_closes_the_thought() {
    let s = init("ideapromote");
    let id = ok(s.path(), &["idea", "add", "저장 계층을 다시", "-q"]).trim().to_string();
    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-"],
        "# 저장 계층\n- [p1] 원자적으로 쓴다 #bug\n- 잘린 줄을 복구한다\n",
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let made = ok(s.path(), &["show", "--json"]);
    assert_eq!(made.matches(r#""kind":"epic""#).count(), 1, "{made}");
    assert!(made.contains("원자적으로 쓴다") && made.contains("잘린 줄을 복구한다"), "{made}");

    let line = line_of(s.path(), &id);
    assert!(line.contains(r#""status":"done""#), "펼쳤는데 안 닫혔다 — {line}");
}

/// **무엇이 무엇에서 나왔는지는 저널에 적는다.** idea 에 `spawned` 같은
/// 필드를 들면 그건 파생값이고, 그 순간 에픽을 지울 때 idea 도 손봐야 한다.
#[test]
fn what_came_from_what_lives_in_the_journal() {
    let s = init("ideatrace");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let out = from_stdin(s.path(), &["idea", "promote", &id, "--from", "-"], "# 새 에픽\n- 첫 일\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let epic = {
        let listed = ok(s.path(), &["epic", "show", "--json"]);
        field(&listed, "id")
    };
    let shown = ok(s.path(), &["show", &id]);
    assert!(shown.contains(&epic), "이력이 무엇이 나왔는지 안 말한다 — {shown}");
    assert!(!line_of(s.path(), &id).contains("spawned"), "파생값을 줄에 적었다");
}

/// `--dry-run` 은 아무것도 만들지 않는다. AI 가 펼친 안을 사람이 한 번 보고
/// "좋다" 하는 자리다.
#[test]
fn promote_can_be_rehearsed() {
    let s = init("ideadry");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let before = issues(s.path());
    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-", "--dry-run"],
        "# 새 에픽\n- 첫 일\n",
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("새 에픽") && text.contains("첫 일"), "{text}");
    assert_eq!(issues(s.path()), before, "연습인데 썼다");
}

/// idea 가 아닌 것은 펼치지 않는다. 조용히 받아 주면 이슈 하나가 까닭 없이
/// 닫히고, 그 까닭은 저널에만 남는다.
#[test]
fn only_an_idea_can_be_promoted() {
    let s = init("ideaonly");
    let work = add(s.path(), &["진짜 일"]);
    let out = from_stdin(s.path(), &["idea", "promote", &work, "--from", "-"], "# 가\n- 나\n");
    assert!(!out.status.success(), "일을 펼쳐 버렸다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("idea"), "{err}");
}

/// `promote` 는 훑기 목록의 `idea` 줄이 닿지 않는 길이다 — stdin 을 먹으므로
/// `every_command_still_speaks_json` 의 손이 안 간다. 여기서 따로 부른다.
#[test]
fn promote_speaks_json_too() {
    let s = init("ideapromotejson");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-", "--json"],
        "# 새 에픽\n- 첫 일\n",
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    one_json_value(&String::from_utf8(out.stdout).unwrap());
}

/// `moai init` 이 쓰는 블록이 idea 를 가르친다. 안 가르치면 에이전트는 지금
/// 범위 밖의 것을 만나도 `moai add` 로 이슈를 만들고, 보드가 그만큼 흐려진다.
#[test]
fn the_agents_block_teaches_idea() {
    let s = init("agentsidea");
    let block = std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap();
    for want in ["moai idea add", "moai idea ls", "moai idea promote"] {
        assert!(block.contains(want), "{want} 가 없다 — {block}");
    }
}
