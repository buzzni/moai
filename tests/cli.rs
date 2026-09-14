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

/// **시험이 띄우는 프로세스는 시험을 돌리는 사람의 기계를 읽지 않는다.** 모든
/// 실행이 여기서 시작한다 — 따로 막기로 한 시험만 막혀 있으면 새 시험이 그것을
/// 잊는 날 결과가 기계를 따라 달라진다. json 훑기의 `skill status` 가 진짜
/// `~/.claude/plugins` 를 읽던 것이 그렇게 났다(moai-kebc).
///
/// 걷는 것은 moai 가 실제로 기계에서 읽는 자리뿐이다.
/// - `HOME`·`CLAUDE_CONFIG_DIR`·`CLAUDE_CODE_PLUGIN_CACHE_DIR` — `claude` 의 장부
/// - git 전역·시스템 설정 — 사람 이름이 `MOAI_ACTOR` 다음에 여기서 온다.
///   `GIT_CONFIG_GLOBAL` 을 주면 git 은 `~/.gitconfig` 도 `XDG_CONFIG_HOME` 도 안 본다.
///   환경으로 넣는 설정(`GIT_CONFIG_COUNT`·`GIT_CONFIG_PARAMETERS`)과 저장소를
///   가리키는 변수(`GIT_DIR`·`GIT_WORK_TREE`·…)도 걷는다 — git 훅이나
///   `git -c … rebase -x 'cargo test'` 안에서 돌면 git 이 이것들을 내보내고,
///   그러면 moai 의 `git config user.name` 이 바깥 사람의 이름을 읽는다
/// - `MOAI_CONFIG`·`XDG_CONFIG_HOME` — 사용자 설정(등록한 프로젝트 목록)의 자리
/// - `MOAI_ACTOR`·`MOAI_NOW` — 셸에 내보내 둔 값이 새면 "사람을 못 찾는다" 와
///   "시계를 고정하지 않았다" 를 보려던 시험이 조용히 딴것을 본다
///
/// 시험이 제 값을 주려면 이 뒤에 `.env` 로 덮는다. `PATH` 는 두고 간다 — moai 가
/// git 을 부르고, `claude` 가 없는 자리가 필요한 시험은 `Claude` 가 따로 만든다.
fn isolated(program: impl AsRef<std::ffi::OsStr>) -> Command {
    // 빈 집 하나를 모든 시험이 함께 쓴다. moai 도 git 도 `HOME` 에 쓰지 않으니
    // 비어 있는 채로 남고, `target/` 밑이라 기계에 부스러기를 흘리지 않는다.
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("moai-cli-home");
    std::fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(program);
    cmd.env("HOME", &home)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_CODE_PLUGIN_CACHE_DIR")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("MOAI_ACTOR")
        .env_remove("MOAI_NOW")
        // 사용자 설정은 **없는 파일**을 가리킨다. 등록한 프로젝트가 새면 `.moai`
        // 밖에서 부르는 시험이 돌리는 사람의 프로젝트를 본다. `XDG_CONFIG_HOME` 도
        // 걷는다 — `MOAI_CONFIG` 를 덮어쓴 시험이 그것을 지우면 그다음 자리다.
        .env("MOAI_CONFIG", home.join("moai-config-unset/config.toml"))
        .env_remove("XDG_CONFIG_HOME");
    for var in GIT_LEAKS {
        cmd.env_remove(var);
    }
    cmd
}

/// 물려받으면 git 이 바깥 저장소나 바깥 설정을 보게 되는 변수들.
const GIT_LEAKS: &[&str] = &[
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

fn moai(dir: &Path, args: &[&str]) -> Output {
    isolated(BIN)
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
    let out = isolated("bash")
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
        isolated(BIN)
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
            isolated(BIN)
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

    // 그리고 그 줄을 **들고 간다.** 쓰기는 되고 줄은 남는다 — 막으면
    // 되돌릴 방법이 도구 밖에만 남고, 버리면 조용한 손실이다.
    let write = moai(s.path(), &["add", "새 것"]);
    assert!(write.status.success(), "깨진 줄 하나가 쓰기를 막았다");
    assert!(
        String::from_utf8_lossy(&write.stderr).contains("그대로 두고 썼다"),
        "조용히 지나갔다 — {}",
        String::from_utf8_lossy(&write.stderr)
    );
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("{\"id\": 깨짐"), "모르는 줄을 잃었다 — {after}");
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

// ── `.moai` 밖의 한눈 보기 — 등록한 프로젝트마다 (moai-6au6) ──────────

/// 등록 목록을 **제 임시 파일로** 쓴다. 돌리는 사람의 설정을 읽으면 결과가 기계를 따른다.
fn registry(s: &Scratch, dirs: &[&Path]) -> PathBuf {
    let path = s.path().join("user/config.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let body: String = dirs.iter().map(|d| format!("[[project]]\npath = {:?}\n", d.to_str().unwrap())).collect();
    std::fs::write(&path, body).unwrap();
    path
}

fn moai_with(dir: &Path, config: &Path, args: &[&str]) -> Output {
    isolated(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_CONFIG", config)
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .expect("moai 를 실행하지 못했다")
}

fn ok_with(dir: &Path, config: &Path, args: &[&str]) -> String {
    let out = moai_with(dir, config, args);
    assert!(
        out.status.success(),
        "moai {args:?} 가 실패했다\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// 한눈 보기에서 이름이 `name` 인 프로젝트의 덩어리. 프로젝트는 빈 줄로 갈린다.
fn block<'a>(out: &'a str, name: &str) -> &'a str {
    out.split("\n\n")
        .find(|b| b.starts_with(&format!("{name}  ")))
        .unwrap_or_else(|| panic!("{name} 덩어리가 없다 — {out}"))
}

/// 디렉터리 하나를 만들어 돌려준다.
fn dir_in(s: &Scratch, rel: &str) -> PathBuf {
    let d = s.path().join(rel);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// **같은 id 가 두 프로젝트에 있어도 섞이지 않는다.** 접두어가 같은 두 저장소는 흔하고,
/// 줄을 한데 모아 세면 한쪽에서 집은 일이 다른 쪽 보드에 서거나 `ready` 에서 빠진다.
/// 디렉터리 이름이 겹치면 위 디렉터리를 붙여 가른다.
#[test]
fn outside_a_repo_each_registered_project_stands_apart_even_with_the_same_ids() {
    let s = Scratch::new("ovsame");
    let (one, two, out) = (dir_in(&s, "one/api"), dir_in(&s, "two/api"), dir_in(&s, "out"));
    ok(&one, &["init", "argos"]);
    let id = add(&one, &["같은 줄"]);
    std::fs::create_dir_all(two.join(".moai")).unwrap();
    for f in ["config.toml", "issues.jsonl", "journal.jsonl"] {
        std::fs::copy(one.join(".moai").join(f), two.join(".moai").join(f)).unwrap();
    }
    ok(&two, &["mv", &id, "in_progress"]);
    let cfg = registry(&s, &[&one, &two]);

    let st = ok_with(&out, &cfg, &["status"]);
    let (a, b) = (block(&st, "one/api"), block(&st, "two/api"));
    assert!(a.contains("todo 1") && a.contains("in_progress 0"), "{st}");
    assert!(!a.contains(&id), "집지 않은 쪽에 집은 줄이 섰다 — {st}");
    assert!(b.contains("todo 0") && b.contains("in_progress 1"), "{st}");
    assert!(b.contains(&format!("two/api  {id}")), "집은 줄 곁에 프로젝트 이름이 없다 — {st}");

    let rd = ok_with(&out, &cfg, &["ready"]);
    assert!(block(&rd, "one/api").contains(&format!("one/api  {id}")), "{rd}");
    assert!(!block(&rd, "two/api").contains(&id), "집은 일이 ready 에 섰다 — {rd}");
}

/// 글에서 SGR 을 걷는다 — 색을 얹은 화면과 끈 화면이 글자로 같은지 견줄 때.
fn strip_sgr(t: &str) -> String {
    let mut o = String::new();
    let mut rest = t;
    while let Some(at) = rest.find("\u{1b}[") {
        o.push_str(&rest[..at]);
        let tail = &rest[at..];
        rest = &tail[tail.find('m').unwrap() + 1..];
    }
    o.push_str(rest);
    o
}

/// 낱말 바로 앞에 이어 붙은 SGR 덩어리 — 칠하지 않았으면 빈 글자다.
fn sgr_before(line: &str, word: &str) -> String {
    let at = line.find(&format!("{word}\u{1b}[0m")).unwrap_or_else(|| panic!("{word} 가 칠해지지 않았다 — {line:?}"));
    let mut head = &line[..at];
    let mut sgr = String::new();
    while let Some(esc) = head.rfind('\u{1b}') {
        let seq = &head[esc..];
        let body = seq.strip_prefix("\u{1b}[").and_then(|b| b.strip_suffix('m'));
        if !body.is_some_and(|b| b.chars().all(|c| c.is_ascii_digit() || c == ';')) {
            break;
        }
        sgr.insert_str(0, seq);
        head = &head[..esc];
    }
    sgr
}

/// **사용자 설정에 정한 색이 경로 해시를 이긴다** (moai-o04b) — 한눈 보기의 이름·id·머리와
/// `project ls` 의 이름이 다 따라온다. 틀린 값은 한 줄로 비추고 해시 색으로 서며 0 으로 끝나고,
/// 명령은 틀린 값을 거절해 파일을 안 건드린다. `auto` 로 되돌리면 설정 바이트와 색이 처음으로
/// 돌아온다. 색을 끈 화면은 어느 때나 한 바이트도 안 바뀐다 — 칠하는 것만 바뀐다.
#[test]
fn a_colour_chosen_in_the_user_config_beats_the_hash_and_only_colour_changes() {
    let s = Scratch::new("ovchosen");
    let out = dir_in(&s, "out");
    let one = dir_in(&s, "one");
    ok(&one, &["init", "argos"]);
    let picked = add(&one, &["집은 일"]);
    ok(&one, &["mv", &picked, "in_progress"]);
    let todo = add(&one, &["집을 일"]);
    let cfg = registry(&s, &[&one]);
    let original = std::fs::read_to_string(&cfg).unwrap();
    let one_arg = one.to_str().unwrap();

    let run = |args: &[&str], colour: bool| {
        let mut cmd = isolated(BIN);
        cmd.args(args).current_dir(&out).env("MOAI_CONFIG", &cfg).env("MOAI_NOW", NOW);
        if colour {
            cmd.arg("--color").arg("always").env_remove("NO_COLOR");
        } else {
            cmd.env("NO_COLOR", "1");
        }
        cmd.output().unwrap()
    };
    let stdout = |args: &[&str], colour: bool| {
        let o = run(args, colour);
        assert!(o.status.success(), "{args:?} → {}", text(&o));
        String::from_utf8(o.stdout).unwrap()
    };
    // 한 화면에서 프로젝트 `one` 이 입은 색 — 이름 칸·id 칸·머리가 한 색인지도 여기서 본다.
    let worn = |cmd: &str, id: &str| -> String {
        let t = stdout(&[cmd], true);
        let row = t.lines().find(|l| l.starts_with("  ") && l.contains("one\u{1b}[0m") && l.contains(id)).unwrap_or_else(|| panic!("{t}"));
        let name = sgr_before(row, "one");
        assert_eq!(name, sgr_before(row, id), "이름 칸과 id 칸 색이 다르다 — {row:?}");
        let head = t.lines().find(|l| !l.starts_with(' ') && l.contains("one\u{1b}[0m  ")).unwrap();
        assert!(sgr_before(head, "one").contains(&name), "머리가 줄과 다른 색이다 — {head:?}");
        assert_eq!(strip_sgr(&t), stdout(&[cmd], false), "색을 얹으며 글자가 바뀌었다");
        name
    };
    let listed = || {
        let t = stdout(&["project", "ls"], true);
        sgr_before(t.lines().find(|l| l.starts_with('\u{1b}') && l.contains("one\u{1b}[0m")).unwrap(), "one")
    };
    let plain_ready = stdout(&["ready"], false);
    let plain_status = stdout(&["status"], false);
    let plain_ls = stdout(&["project", "ls"], false);
    let unchanged = |why: &str| {
        assert_eq!(stdout(&["ready"], false), plain_ready, "{why}: 색을 끈 ready 가 바뀌었다");
        assert_eq!(stdout(&["status"], false), plain_status, "{why}: 색을 끈 status 가 바뀌었다");
        assert_eq!(stdout(&["project", "ls"], false), plain_ls, "{why}: 색을 끈 project ls 가 바뀌었다");
    };

    let hashed = worn("ready", &todo);
    assert_eq!(worn("status", &picked), hashed);
    assert_eq!(listed(), hashed, "project ls 의 이름이 한눈 보기와 다른 색이다");
    // 해시가 고른 것과 **다른** 색을 고른다 — 같은 색을 고르면 이겼는지 못 가린다.
    let (target, code) = [("cyan", "[36m"), ("green", "[32m"), ("blue", "[34m")]
        .into_iter()
        .find(|(_, c)| !hashed.contains(c))
        .unwrap();

    let set = stdout(&["project", "color", one_arg, target, "--json"], false);
    assert!(set.contains(&format!("\"color\":\"{target}\"")) && set.contains("\"was\":null") && set.contains("\"changed\":true"), "{set}");
    assert!(std::fs::read_to_string(&cfg).unwrap().contains(&format!("color = \"{target}\"")));
    for (cmd, id) in [("ready", &todo), ("status", &picked)] {
        let now = worn(cmd, id);
        assert!(now.contains(code) && now != hashed, "{cmd}: 정한 색({target})이 해시를 못 이겼다 — {now:?}");
    }
    assert!(listed().contains(code), "project ls 가 정한 색을 안 입었다");
    let json = stdout(&["project", "ls", "--json"], false);
    assert!(json.contains(&format!("\"color\":\"{target}\"")), "{json}");
    unchanged("색을 정한 뒤");

    // 명령은 팔레트 밖 색을 거절하고, 받는 이름을 대며, 파일을 안 건드린다.
    let before = std::fs::read_to_string(&cfg).unwrap();
    for bad in ["red", "magenta", "Green"] {
        let o = run(&["project", "color", one_arg, bad, "--json"], false);
        assert!(!o.status.success(), "{bad} 를 받았다");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("\"code\":\"bad_input\"") && err.contains("cyan·green·blue") && err.contains("auto"), "{bad} → {err}");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), before, "{bad} 로 설정을 고쳤다");
    }
    // 등록 안 된 디렉터리에는 색을 못 정한다 — 저절로 등록하지도 않는다.
    let o = run(&["project", "color", out.to_str().unwrap(), target, "--json"], false);
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("\"code\":\"not_found\""), "{}", text(&o));
    assert_eq!(std::fs::read_to_string(&cfg).unwrap(), before);

    // `auto` 는 정한 것을 지운다 — 설정 바이트도 색도 처음으로 돌아온다.
    let back = stdout(&["project", "color", one_arg, "auto", "--json"], false);
    assert!(back.contains("\"color\":null") && back.contains(&format!("\"was\":\"{target}\"")), "{back}");
    assert_eq!(std::fs::read_to_string(&cfg).unwrap(), original, "auto 로 되돌렸는데 설정 바이트가 다르다");
    assert_eq!(worn("ready", &todo), hashed, "auto 로 되돌렸는데 해시 색이 아니다");
    assert_eq!(listed(), hashed);
    let again = stdout(&["project", "color", one_arg, "auto", "--json"], false);
    assert!(again.contains("\"changed\":false"), "{again}");

    // 손으로 적은 틀린 값은 한 줄로 비추고 해시 색으로 서며 0 으로 끝난다.
    std::fs::write(&cfg, format!("{original}color = \"red\"\n")).unwrap();
    let o = run(&["ready"], false);
    assert!(o.status.success(), "{}", text(&o));
    let said = String::from_utf8(o.stdout).unwrap();
    assert!(said.contains("\"red\" 는 프로젝트 색이 아니다") && said.contains("경로로 고른 색을 쓴다"), "{said}");
    assert_eq!(worn("ready", &todo), hashed, "틀린 값이 해시 색을 흔들었다");
    let ls = run(&["project", "ls"], false);
    assert!(ls.status.success() && String::from_utf8_lossy(&ls.stderr).contains("\"red\""), "{}", text(&ls));
}

/// **프로젝트마다 색이 다르고, 한 프로젝트의 줄은 한 색이다** (moai-xs9x). 색은 경로로
/// 고른다 — 다른 프로젝트를 더하고 빼도 제 색이 그대로다. 색을 끄면 글자는 칠하기 전과
/// 바이트까지 같다: 이름이 곁에 서서 **색이 혼자 뜻을 지지 않는다.**
///
/// 시험 디렉터리는 부를 때마다 경로가 달라 어느 둘이 다른 색일지 못 박을 수 없다. 그래서
/// 열셋을 올린다 — 셋 중 하나를 고르는 해시가 열셋을 전부 한 색에 몰 확률은 3^-12 다.
#[test]
fn outside_a_repo_each_project_wears_its_own_colour_and_only_colour_changes() {
    let s = Scratch::new("ovhue");
    let out = dir_in(&s, "out");
    let names: Vec<String> = (0..13).map(|i| format!("p{i}")).collect();
    let dirs: Vec<PathBuf> = names.iter().map(|n| dir_in(&s, n)).collect();
    ok(&dirs[0], &["init", "argos"]);
    let picked = add(&dirs[0], &["집은 일"]);
    ok(&dirs[0], &["mv", &picked, "in_progress"]);
    let todo = add(&dirs[0], &["집을 일"]);
    for d in &dirs[1..] {
        std::fs::create_dir_all(d.join(".moai")).unwrap();
        for f in ["config.toml", "issues.jsonl", "journal.jsonl"] {
            std::fs::copy(dirs[0].join(".moai").join(f), d.join(".moai").join(f)).unwrap();
        }
    }
    let all: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
    let run = |cfg: &Path, args: &[&str], colour: bool| {
        let mut cmd = isolated(BIN);
        cmd.args(args).current_dir(&out).env("MOAI_CONFIG", cfg).env("MOAI_NOW", NOW);
        if colour {
            cmd.arg("--color").arg("always").env_remove("NO_COLOR");
        } else {
            cmd.env("NO_COLOR", "1");
        }
        let o = cmd.output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    let strip = strip_sgr;

    let cfg = registry(&s, &all);
    for (cmd, id) in [("ready", &todo), ("status", &picked)] {
        let painted = run(&cfg, &[cmd], true);
        let plain = run(&cfg, &[cmd], false);
        assert!(!plain.contains('\u{1b}'), "색을 껐는데 이스케이프가 섰다 — {plain}");
        assert_eq!(strip(&painted), plain, "색을 얹으며 글자가 바뀌었다");

        let mut hues = std::collections::BTreeSet::new();
        for n in &names {
            let row = painted
                .lines()
                .find(|l| l.starts_with("  ") && l.contains(&format!("{n}\u{1b}[0m")) && l.contains(id.as_str()))
                .unwrap_or_else(|| panic!("{n} 의 줄이 없다 — {painted}"));
            let (name_sgr, id_sgr) = (sgr_before(row, n), sgr_before(row, id));
            assert!(!name_sgr.is_empty(), "{n} 의 이름 칸이 안 칠해졌다 — {row:?}");
            assert_eq!(name_sgr, id_sgr, "{n} 의 이름 칸과 id 칸 색이 다르다 — {row:?}");
            let head = painted.lines().find(|l| !l.starts_with(' ') && l.contains(&format!("{n}\u{1b}[0m  "))).unwrap();
            assert!(sgr_before(head, n).contains(&name_sgr), "{n} 의 머리가 줄과 다른 색이다 — {head:?}");
            hues.insert(name_sgr);
        }
        assert!(hues.len() > 1, "열세 프로젝트가 한 색이다 — {painted}");
    }

    // 다른 프로젝트를 빼고 차례를 바꿔도 제 색이 그대로다 — 등록 순서가 아니라 경로로 고른다.
    let colour_of = |cfg: &Path, n: &str| {
        let t = run(cfg, &["ready"], true);
        let row = t.lines().find(|l| l.starts_with("  ") && l.contains(&format!("{n}\u{1b}[0m"))).unwrap().to_string();
        sgr_before(&row, n)
    };
    let full = colour_of(&cfg, "p7");
    let alone = s.path().join("alone.toml");
    std::fs::write(&alone, format!("[[project]]\npath = {:?}\n", dirs[7].to_str().unwrap())).unwrap();
    assert_eq!(colour_of(&alone, "p7"), full, "혼자 올리니 색이 바뀌었다");
    let shuffled = s.path().join("shuffled.toml");
    let body: String =
        [9, 7, 2].iter().map(|i| format!("[[project]]\npath = {:?}\n", dirs[*i].to_str().unwrap())).collect();
    std::fs::write(&shuffled, body).unwrap();
    assert_eq!(colour_of(&shuffled, "p7"), full, "차례를 바꾸니 색이 바뀌었다");
}

/// **읽기는 관대하다.** init 전·사라진 디렉터리·깨진 스냅샷·깨진 설정·설정 파일의 못 읽는
/// 항목은 제 줄에서만 말하고, 멀쩡한 프로젝트는 그대로 보이며, 종료 코드는 0 이다 — 남의
/// 저장소 하나로 한눈 보기 전체가 실패로 읽히면 나머지를 못 믿는다.
#[test]
fn outside_a_repo_the_overview_names_each_trouble_and_still_exits_zero() {
    let s = Scratch::new("ovtrouble");
    let (good, bare, broken, badcfg, out) =
        (dir_in(&s, "good"), dir_in(&s, "bare"), dir_in(&s, "broken"), dir_in(&s, "badcfg"), dir_in(&s, "out"));
    let gone = s.path().join("gone");
    ok(&good, &["init", "argos"]);
    let id = add(&good, &["멀쩡한 일"]);
    ok(&broken, &["init", "argos"]);
    let mut lines = issues(&broken);
    lines.push_str("{이건 JSON 이 아니다\n");
    std::fs::write(broken.join(".moai/issues.jsonl"), lines).unwrap();
    ok(&badcfg, &["init", "argos"]);
    std::fs::write(badcfg.join(".moai/config.toml"), "prefix = \"\"\n").unwrap();
    let cfg = registry(&s, &[&good, &bare, &gone, &broken, &badcfg]);
    let mut text = std::fs::read_to_string(&cfg).unwrap();
    text.push_str("[[project]]\npath = \"relative/dir\"\n");
    std::fs::write(&cfg, text).unwrap();

    let st = ok_with(&out, &cfg, &["status"]);
    assert!(block(&st, "good").contains("todo 1"), "{st}");
    assert!(block(&st, "bare").contains("init 전"), "{st}");
    assert!(block(&st, "gone").contains("디렉터리가 없다"), "{st}");
    assert!(block(&st, "broken").contains("데이터가 깨졌다"), "{st}");
    assert!(block(&st, "badcfg").contains("못 읽는다") && block(&st, "badcfg").contains("prefix"), "{st}");
    assert!(st.contains("절대경로"), "사용자 설정의 문제를 말하지 않았다 — {st}");

    let rd = ok_with(&out, &cfg, &["ready"]);
    assert!(block(&rd, "good").contains(&id), "{rd}");
    assert!(block(&rd, "broken").contains("읽을 수 없는 줄 1개"), "{rd}");
    assert!(block(&rd, "bare").contains("init 전"), "{rd}");

    // 인자 없이 부른 것도 같은 한눈 보기다 — 세션의 시작점이다.
    let bare_call = ok_with(&out, &cfg, &[]);
    assert!(block(&bare_call, "good").contains("todo 1"), "{bare_call}");
}

/// **등록한 경로의 제어문자는 화면을 다시 칠하지 못한다.** 설정 파일은 손으로 고칠 수
/// 있고, ESC 가 든 경로를 그대로 그리면 그 줄이 커서를 옮기고 화면을 지운다
/// (`moai project ls` 와 같은 자).
#[test]
fn outside_a_repo_a_path_cannot_repaint_the_screen() {
    let s = Scratch::new("ovescape");
    let out = dir_in(&s, "out");
    let cfg = s.path().join("user/config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    std::fs::write(&cfg, format!("[[project]]\npath = \"{}/gone\\u001b[2Jx\"\n", s.path().display())).unwrap();
    for verb in ["status", "ready"] {
        let shown = ok_with(&out, &cfg, &[verb]);
        assert!(!shown.contains('\u{1b}'), "{verb}: {shown:?}");
        assert!(shown.contains("gone[2Jx") && shown.contains("디렉터리가 없다"), "{verb}: {shown}");
    }
}

/// **남의 저장소의 글자도 화면을 다시 칠하지 못한다** — 칸 이름(`statuses`)·id·제목.
///
/// 위 시험이 `NO_COLOR` 로 도는 바람에 이것을 못 잡았다: 색을 끄면 anstream 이 나가는
/// 길에서 ESC 를 걷어내, 걸러지지 않은 글자가 시험에서만 안전해 보인다. 켜고 재야 진짜
/// 터미널과 같은 길이다. 등록한 것은 남의 저장소일 수 있고 `.moai/config.toml` 도
/// `issues.jsonl` 도 손으로 고칠 수 있다 — 읽기는 관대하되 그리기는 엄해야 하는 자리다.
#[test]
fn outside_a_repo_another_repos_own_text_cannot_repaint_the_screen() {
    let s = Scratch::new("ovescape2");
    let (odd, out) = (dir_in(&s, "odd"), dir_in(&s, "out"));
    std::fs::create_dir_all(odd.join(".moai")).unwrap();
    std::fs::write(odd.join(".moai/config.toml"), "prefix = \"argos\"\nstatuses = \"todo,\u{1b}[2Jwip,done\"\n").unwrap();
    std::fs::write(
        odd.join(".moai/issues.jsonl"),
        // JSON 문자열 안의 제어문자는 `\\u001b` 로 적는다 — 날 바이트로 두면 줄이 통째로
        // 못 읽는 줄이 되어 시험이 아무것도 안 잰다.
        "{\"id\":\"argos-\\u001b[2J01\",\"title\":\"\\u001b[2J집은 것\",\"status\":\"\\u001b[2Jwip\",\"created_at\":\"2026-09-01T00:00:00Z\",\"updated_at\":\"2026-09-01T00:00:00Z\",\"status_since\":\"2026-09-01T00:00:00Z\"}\n\
         {\"id\":\"argos-0002\",\"title\":\"\\u001b[2J집을 것\",\"status\":\"todo\",\"created_at\":\"2026-09-01T00:00:00Z\",\"updated_at\":\"2026-09-01T00:00:00Z\",\"status_since\":\"2026-09-01T00:00:00Z\"}\n",
    )
    .unwrap();
    let cfg = registry(&s, &[&odd]);

    // **색을 켜고 잰다.** `--color always` 는 `NO_COLOR` 를 이기므로 anstream 이 걷어내지
    // 않고, 그래서 진짜 터미널과 같은 바이트가 나온다.
    for args in [&["status", "--color", "always"][..], &["ready", "--color", "always"], &["project", "ls", "--color", "always"]] {
        let shown = ok_with(&out, &cfg, args);
        assert!(!shown.contains("\u{1b}[2J"), "{args:?} 가 남의 ESC 를 흘렸다: {shown:?}");
        // 거르되 버리지는 않는다 — 글자는 남아야 어느 줄인지 안다.
        assert!(shown.contains("[2J"), "{args:?}: 글자를 통째로 버렸다 — {shown}");
        assert!(shown.contains("\u{1b}["), "{args:?}: 색이 안 켜졌다 — 시험이 아무것도 안 재고 있다");
    }
}

/// 한눈 보기의 기계 출력. 프로젝트마다 `name`·`path`·`state` 가 서고, 연 것만 제 셈을
/// 곁에 든다. **`projects` 키가 곧 여러 프로젝트를 봤다는 뜻이다.**
#[test]
fn outside_a_repo_the_overview_speaks_json() {
    let s = Scratch::new("ovjson");
    let (good, bare, out) = (dir_in(&s, "good"), dir_in(&s, "bare"), dir_in(&s, "out"));
    let gone = s.path().join("gone");
    ok(&good, &["init", "argos"]);
    let id = add(&good, &["집을 일"]);
    let picked = add(&good, &["집은 일"]);
    ok(&good, &["mv", &picked, "in_progress"]);
    let cfg = registry(&s, &[&good, &bare, &gone]);

    let st = ok_with(&out, &cfg, &["status", "--json"]);
    one_json_value(&st);
    let good_at = format!("{{\"name\":\"good\",\"path\":{:?},\"state\":\"ok\",\"status\":{{\"counts\":", good.to_str().unwrap());
    assert!(st.starts_with(&format!("{{\"projects\":[{good_at}")), "{st}");
    assert!(st.contains(&format!("\"picked\":[{{\"id\":\"{picked}\"")), "{st}");
    let bare_at = st.find(&format!("{{\"name\":\"bare\",\"path\":{:?},\"state\":\"uninitialized\"}}", bare.to_str().unwrap()));
    let gone_at = st.find(&format!("{{\"name\":\"gone\",\"path\":{:?},\"state\":\"missing\"}}", gone.to_str().unwrap()));
    assert!(bare_at.is_some() && gone_at.is_some() && bare_at < gone_at, "등록 차례가 아니다 — {st}");
    assert!(st.trim_end().ends_with(&format!("\"problems\":[],\"config\":{:?}}}", cfg.to_str().unwrap())), "{st}");

    let rd = ok_with(&out, &cfg, &["ready", "--json"]);
    one_json_value(&rd);
    assert!(rd.contains(&format!("\"state\":\"ok\",\"ready\":[{{\"id\":\"{id}\"")), "{rd}");
    assert!(!rd.contains(&picked), "집은 일이 ready 에 섰다 — {rd}");
    assert!(rd.contains("\"unreadable\":0"), "{rd}");
}

/// **`.moai` 밖의 `tui --json` 은 프로젝트 층의 줄을 낸다**(moai-ujpu) — 탐색기 줄과 같은
/// 키(`title`·`kind`·`dir`·`path`)에 한눈 보기와 같은 상태 낱말. 프로젝트 줄의 `path` 는
/// 디렉터리라 `-C` 로 들어간다. 등록 차례 그대로다. `--path` 는 어느 프로젝트의 id 인지
/// 몰라 거절하고, 등록한 것이 없으면 `status` 와 같은 말로 멈춘다.
#[test]
fn outside_a_repo_tui_json_lists_the_project_layer() {
    let s = Scratch::new("ovtui");
    let (good, bare, out) = (dir_in(&s, "good"), dir_in(&s, "bare"), dir_in(&s, "out"));
    ok(&good, &["init", "argos"]);
    let picked = add(&good, &["집은 일"]);
    ok(&good, &["mv", &picked, "in_progress"]);
    let cfg = registry(&s, &[&good, &bare]);

    let rows = ok_with(&out, &cfg, &["tui", "--json"]);
    one_json_value(&rows);
    let good_at = format!(
        "[{{\"title\":\"good\",\"kind\":\"project\",\"dir\":true,\"path\":{:?},\"state\":\"ok\",\"counts\":{{",
        good.to_str().unwrap()
    );
    assert!(rows.starts_with(&good_at), "{rows}");
    assert!(rows.contains(&format!("\"picked\":[\"{picked}\"]")) && rows.contains("\"in_progress\":1"), "{rows}");
    let bare_at = format!(
        "{{\"title\":\"bare\",\"kind\":\"project\",\"dir\":false,\"path\":{:?},\"state\":\"uninitialized\"}}]",
        bare.to_str().unwrap()
    );
    assert!(rows.trim_end().ends_with(&bare_at), "{rows}");

    // 들어가는 손잡이는 디렉터리다 — 거기서 부르면 오늘의 `tui --json` 이다.
    let inside = ok_with(&out, &cfg, &["-C", good.to_str().unwrap(), "tui", "--json"]);
    assert!(inside.contains(&format!("\"id\":\"{picked}\"")), "{inside}");

    let o = moai_with(&out, &cfg, &["tui", "--json", "--path", &picked]);
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("-C <dir> tui --path"), "{}", text(&o));

    let empty = registry(&s, &[]);
    let o = moai_with(&out, &empty, &["tui", "--json"]);
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("moai project add"), "{}", text(&o));
}

/// 등록한 것이 없으면 **전처럼 실패하되** 등록하는 길을 댄다. 보여줄 것이 없는데 0 으로
/// 끝나면 `.moai` 밖에서 부른 실수가 성공으로 읽힌다. 설정 파일이 깨져 목록이 빈 것이면
/// 그 까닭도 함께 말한다.
#[test]
fn outside_a_repo_with_nothing_registered_it_fails_and_says_how_to_register() {
    let s = Scratch::new("ovempty");
    let out = dir_in(&s, "out");
    let cfg = registry(&s, &[]);
    for verb in ["status", "ready"] {
        let o = moai_with(&out, &cfg, &[verb]);
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(!o.status.success(), "{verb}");
        assert!(err.contains("moai init") && err.contains("moai project add"), "{verb}: {err}");
    }
    let help = ok_with(&out, &cfg, &[]);
    assert!(help.contains("moai init") && help.contains("moai project add"), "{help}");

    std::fs::write(&cfg, "project = 3\n").unwrap();
    let o = moai_with(&out, &cfg, &["status"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("표 배열"), "{}", String::from_utf8_lossy(&o.stderr));
}

/// **쓰는 명령은 `.moai` 밖에서 여전히 멈춘다** — 등록한 프로젝트가 있어도 어느 것에
/// 쓸지 모른다. 대신 다른 곳의 저장소를 부르는 길(`-C`)을 댄다.
#[test]
fn a_write_outside_a_repo_still_stops_and_points_at_dash_c() {
    let s = Scratch::new("ovwrite");
    let (good, out) = (dir_in(&s, "good"), dir_in(&s, "out"));
    ok(&good, &["init", "argos"]);
    let before = issues(&good);
    let cfg = registry(&s, &[&good]);
    let o = moai_with(&out, &cfg, &["add", "어디에 쓸까"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("moai -C <dir>"), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(issues(&good), before, "등록한 프로젝트에 썼다");
}

/// **`.moai` 안에서는 등록 목록이 있어도 바이트 하나 안 바뀐다** (결정 3).
#[test]
fn inside_a_repo_a_registry_changes_nothing() {
    let s = init("ovinside");
    add(s.path(), &["제 일"]);
    let other = dir_in(&s, "elsewhere");
    ok(&other, &["init", "other"]);
    let cfg = registry(&s, &[&other, s.path()]);
    for args in [&[][..], &["status"], &["ready"], &["status", "--json"], &["ready", "--json"]] {
        let plain = moai(s.path(), args);
        let with = moai_with(s.path(), &cfg, args);
        assert_eq!(plain.stdout, with.stdout, "{args:?}");
        assert_eq!(plain.stderr, with.stderr, "{args:?}");
        assert_eq!(plain.status.code(), with.status.code(), "{args:?}");
    }
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

/// **`-e none` 이 못 끊는 소속은 끊기지 않았다고 말한다** (moai-w5gz). id 부모에서
/// 오는 소속 — 이슈 밑 자식(부모의 에픽을 물려받는다)과 에픽 밑 자식(moai-9t3l) — 은
/// 필드가 아니라 자리에서 오므로, 필드를 비워도 그대로 남는다. 조용하면 사람은 뺀
/// 줄 안다. 사람에게는 한 줄로, `--json` 에는 더한 키로 댄다.
#[test]
fn edit_says_when_epic_none_cannot_cut_a_parents_membership() {
    let s = init("editkept");
    let epic = add(s.path(), &["에픽", "--type", "epic"]);
    let other = add(s.path(), &["딴 에픽", "--type", "epic"]);
    let parent = add(s.path(), &["부모", "-e", &epic]);
    let child = add(s.path(), &["자식", "--parent", &parent, "-e", &other]);
    let review = add(s.path(), &["리뷰", "--parent", &epic]);
    let top = add(s.path(), &["홀로 선 이슈", "-e", &epic]);
    let inner = ok(s.path(), &["epic", "add", "에픽 밑 에픽", "--parent", &epic, "-q"]).trim().to_string();

    let says = |id: &str, from: &str| {
        let out = moai(s.path(), &["edit", id, "-e", "none"]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let lines: Vec<&str> = err.lines().filter(|l| l.contains(&format!("부모 {from}"))).collect();
        assert_eq!(lines.len(), 1, "{id}: 부모 {from} 에서 온 소속을 한 줄로 안 댔다 — {err:?}");
        assert!(lines[0].contains(&epic) && lines[0].contains("-e <"), "{id}: 에픽과 옮기는 길이 없다 — {err:?}");
    };
    // 이슈 밑 자식 — 제 필드는 비워지고, 소속은 부모의 에픽으로 돌아간다.
    says(&child, &parent);
    assert!(!line_of(s.path(), &child).contains("\"epic\""), "필드는 비워져야 한다");
    assert!(ok(s.path(), &["show", "-e", &epic]).contains(&child));
    // 에픽 밑 자식 — 비울 필드가 없어 바뀐 것이 없어도 말한다.
    says(&review, &epic);

    let json = ok(s.path(), &["edit", &review, "-e", "none", "--json"]);
    one_json_value(&json);
    assert!(json.contains(&format!(r#""inherited_epic":{{"epic":"{epic}","parent":"{epic}"}}"#)), "{json}");
    let json = ok(s.path(), &["edit", &child, "-e", "none", "--json"]);
    assert!(json.contains(&format!(r#""inherited_epic":{{"epic":"{epic}","parent":"{parent}"}}"#)), "{json}");

    // 끊긴 것, 끊을 뜻이 없던 것, 소속을 안 받는 줄은 조용하다.
    for args in [
        vec!["edit", top.as_str(), "-e", "none"],
        vec!["edit", review.as_str(), "--tag", "x"],
        vec!["edit", inner.as_str(), "-e", "none"],
    ] {
        let out = moai(s.path(), &args);
        assert!(out.status.success());
        assert!(out.stderr.is_empty(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let mut j = args.clone();
        j.push("--json");
        let json = ok(s.path(), &j);
        assert!(!json.contains("inherited_epic"), "{args:?}: {json}");
    }

    // **이 키도 우리 것이다** — `derived_status` 와 같은 까닭. `--json` 을 되써 넣어
    // 모르는 필드로 든 줄이면 한 객체에 같은 키가 둘 서거나, 끊긴 줄이 안 끊긴 것처럼 읽힌다.
    let doctored: String = issues(s.path())
        .lines()
        .map(|l| format!("{}{}\n", &l[..l.len() - 1], r#","inherited_epic":"거짓"}"#))
        .collect();
    std::fs::write(s.path().join(".moai/issues.jsonl"), doctored).unwrap();
    let json = ok(s.path(), &["edit", &review, "-e", "none", "--json"]);
    assert_eq!(json.matches(r#""inherited_epic""#).count(), 1, "같은 키가 두 번 났다 — {json}");
    assert!(!json.contains("거짓"), "{json}");
    let json = ok(s.path(), &["edit", &top, "-e", "none", "--json"]);
    assert!(!json.contains("inherited_epic"), "파일의 값이 끊긴 줄에 샜다 — {json}");
    assert!(line_of(s.path(), &top).contains(r#""inherited_epic":"거짓""#), "모르는 필드를 잃었다");
}

/// **`--milestone none` 도 못 끊는 소속은 끊기지 않았다고 말한다** (moai-0lmn). 에픽이
/// 마일스톤을 이기고 부모도 이기므로, 제 필드를 비워도 에픽이나 부모가 선 마일스톤에
/// 그대로 든다 — `-e none` 과 같은 모양이라 같은 말투로 댄다.
#[test]
fn edit_says_when_milestone_none_cannot_cut_an_inherited_milestone() {
    let s = init("editkeptms");
    let ms = ok(s.path(), &["milestone", "add", "v1", "-q"]).trim().to_string();
    let epic = add(s.path(), &["에픽", "--type", "epic", "--milestone", &ms]);
    let member = add(s.path(), &["멤버", "-e", &epic, "--milestone", &ms]);
    let parent = add(s.path(), &["부모", "--milestone", &ms]);
    let child = add(s.path(), &["자식", "--parent", &parent]);
    let top = add(s.path(), &["홀로 선 이슈", "--milestone", &ms]);

    let says = |id: &str, from: &str| {
        let out = moai(s.path(), &["edit", id, "--milestone", "none"]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let lines: Vec<&str> = err.lines().filter(|l| l.contains(from)).collect();
        assert_eq!(lines.len(), 1, "{id}: {from} 에서 온 마일스톤을 한 줄로 안 댔다 — {err:?}");
        assert!(lines[0].contains(&ms) && lines[0].contains("--milestone none"), "{id}: {err:?}");
    };
    // 에픽 멤버 — 제 필드는 비워지고, 소속은 에픽이 선 마일스톤으로 남는다.
    says(&member, &format!("에픽 {epic}"));
    assert!(!line_of(s.path(), &member).contains("\"milestone\""), "필드는 비워져야 한다");
    assert!(ok(s.path(), &["show", "--milestone", &ms]).contains(&member));
    // 부모 밑 자식 — 비울 필드가 없어 바뀐 것이 없어도 말한다.
    says(&child, &format!("조상 {parent}"));
    // 손자는 **값을 든 조상**을 댄다 — 바로 위 자식을 고치라면 아무것도 안 바뀐다.
    let grand = add(s.path(), &["손자", "--parent", &child]);
    says(&grand, &format!("조상 {parent}"));
    // 마일스톤 밑에 id 로 선 자식 — 그 마일스톤의 필드를 고치라고 대지 않는다.
    let under = add(s.path(), &["마일스톤 밑", "--parent", &ms]);
    let out = moai(s.path(), &["edit", &under, "--milestone", "none"]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains(&ms) && err.contains("--milestone none"), "{err:?}");
    assert!(!err.contains(&format!("moai edit {ms}")), "고쳐도 안 바뀌는 길을 댔다 — {err:?}");

    let json = ok(s.path(), &["edit", &member, "--milestone", "none", "--json"]);
    one_json_value(&json);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":"{ms}","epic":"{epic}"}}"#)), "{json}");
    let json = ok(s.path(), &["edit", &child, "--milestone", "none", "--json"]);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":"{ms}","parent":"{parent}"}}"#)), "{json}");
    let json = ok(s.path(), &["edit", &grand, "--milestone", "none", "--json"]);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":"{ms}","parent":"{parent}"}}"#)), "{json}");

    // 모르는 필드로 든 같은 이름은 끊긴 줄에 새지 않는다.
    let doctored: String = issues(s.path())
        .lines()
        .map(|l| format!("{}{}\n", &l[..l.len() - 1], r#","inherited_milestone":"거짓"}"#))
        .collect();
    std::fs::write(s.path().join(".moai/issues.jsonl"), doctored).unwrap();
    let json = ok(s.path(), &["edit", &member, "--milestone", "none", "--json"]);
    assert_eq!(json.matches(r#""inherited_milestone""#).count(), 1, "같은 키가 두 번 났다 — {json}");

    // 끊긴 것, 끊을 뜻이 없던 것, 제 필드에만 서는 에픽 줄은 조용하다.
    for args in [
        vec!["edit", top.as_str(), "--milestone", "none"],
        vec!["edit", child.as_str(), "--tag", "x"],
        vec!["edit", epic.as_str(), "--milestone", "none"],
    ] {
        let out = moai(s.path(), &args);
        assert!(out.status.success());
        assert!(out.stderr.is_empty(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let mut j = args.clone();
        j.push("--json");
        let json = ok(s.path(), &j);
        assert!(!json.contains("inherited_milestone"), "{args:?}: {json}");
    }
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

/// **CLI 상세도 막음을 그린다**(moai-rvcb) — `ready` 가 고르는 그 자로, 탐색기와 같은 낱말로.
/// 막는 줄이 끝나면 풀림, 미루면 미룬 까닭과 함께 막힘, 지우면 끊김이다. `--json` 도 같은 답을 낸다.
#[test]
fn show_draws_each_blocker_with_the_words_ready_uses() {
    let s = init("showblocks");
    let a = add(s.path(), &["먼저 할 것"]);
    let c = add(s.path(), &["미뤄 둔 것"]);
    let b = add(s.path(), &["막히는 것"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    ok(s.path(), &["link", &c, "--blocks", &b]);
    let line = |out: &str, id: &str| out.lines().find(|l| l.contains(id) && !l.starts_with(id)).unwrap_or_default().to_string();

    let shown = ok(s.path(), &["show", &b]);
    assert!(line(&shown, &a).contains("막힘") && line(&shown, &a).contains("먼저 할 것"), "{shown}");

    ok(s.path(), &["mv", &a, "done"]);
    ok(s.path(), &["defer", &c]);
    let shown = ok(s.path(), &["show", &b]);
    assert!(line(&shown, &a).contains("풀림"), "끝난 막음이 풀림으로 안 섰다\n{shown}");
    let deferred = line(&shown, &c);
    assert!(deferred.contains("막힘") && deferred.contains("미룸"), "미룬 막음이 미뤘다고 안 한다\n{shown}");
    // `ready` 와 같은 답이다 — 미룬 막음도 막으므로 b 는 집을 일로 안 선다. b 는 `held` 에
    // "미룬 것에 막혀 못 집는 것" 으로 서므로, `ready` 목록만 떼어 본다.
    let rd = ok(s.path(), &["ready", "--json"]);
    let picks = rd.split("\"held\"").next().unwrap_or_default();
    assert!(!picks.contains(&b), "미룬 막음에 막힌 줄을 집으라고 낸다\n{rd}");
    assert!(rd.contains(&format!("\"id\":\"{b}\"")), "held 에 막힌 줄이 없다\n{rd}");

    ok(s.path(), &["rm", &c]);
    let shown = ok(s.path(), &["show", &b]);
    assert!(line(&shown, &c).contains("끊김"), "끊긴 막음이 끊김으로 안 섰다\n{shown}");
    let json = ok(s.path(), &["show", &b, "--json"]);
    // 차례는 적힌 `blocked_by` 차례다 — 쓸 때 id 로 정렬되므로 여기서는 차례를 안 본다.
    assert!(json.contains("\"blockers\":[") && json.contains(&format!("{{\"id\":\"{a}\",\"state\":\"done\"}}")), "{json}");
    assert!(json.contains(&format!("{{\"id\":\"{c}\",\"state\":\"missing\"}}")), "{json}");
    assert!(!ok(s.path(), &["show", &a, "--json"]).contains("\"blockers\""), "막음이 없는데 blockers 키가 섰다");
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

    let later = isolated(BIN)
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

/// **묶음의 칸은 멤버에서 읽는다** (moai-j3b3). 멤버 하나가 집히고 하나가 닫혔는데
/// 에픽이 `· todo` 로 서면 바로 밑의 `멤버 1/2` 와 한 화면에서 모순된다.
/// 손으로 둔 칸은 안 읽되, 그렇다고 말은 한다.
#[test]
fn a_group_stands_in_the_column_its_members_read() {
    let s = init("groupcol");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let held = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let closed = add(s.path(), &["락", "-e", &epic]);
    ok(s.path(), &["mv", &held, "in_progress"]);
    ok(s.path(), &["mv", &closed, "done"]);

    let out = ok(s.path(), &["show", &epic]);
    let head = out.lines().nth(1).unwrap();
    assert!(head.contains("▸ in_progress"), "{out}");
    assert!(!head.contains("적힌 칸"), "옮긴 적 없는 칸을 따로 말했다\n{out}");

    let list = ok(s.path(), &["show", "--all"]);
    let row = list.lines().find(|l| l.starts_with(&epic)).unwrap();
    assert!(row.contains("▸"), "{list}");
    assert!(list.contains("3건 (in_progress 2 · done 1)"), "꼬리가 줄과 다른 칸으로 셌다\n{list}");
    assert!(!ok(s.path(), &["show", "-s", "todo"]).contains(&epic), "진행 중인 에픽을 할 일로 골랐다");

    // 기계 출력은 `status` 를 적힌 값 그대로 두고 읽은 칸을 곁들인다 — 표면 넷이 같은 키다.
    for args in [vec!["show", &epic, "--json"], vec!["show", "--json"], vec!["tui", "--json"]] {
        let json = ok(s.path(), &args);
        let row = json.split("},{").find(|r| r.contains(&format!(r#""id":"{epic}""#))).unwrap();
        assert!(row.contains(r#""status":"todo""#) && row.contains(r#""derived_status":"in_progress""#), "{args:?}: {json}");
    }
    let member_row = ok(s.path(), &["show", &held, "--json"]);
    assert!(!member_row.contains("derived_status"), "묶음 아닌 줄이 읽은 칸을 냈다 — {member_row}");

    // 손으로 둔 칸은 읽은 칸을 못 이긴다 — 그러나 상세가 그것을 말한다.
    ok(s.path(), &["mv", &epic, "done"]);
    let out = ok(s.path(), &["show", &epic]);
    assert!(out.lines().nth(1).unwrap().contains("▸ in_progress"), "{out}");
    assert!(out.contains("적힌 칸 `done` 은 안 읽는다"), "{out}");
    let edited = ok(s.path(), &["edit", &epic, "--tag", "storage"]);
    assert!(edited.lines().nth(1).unwrap().contains("▸ in_progress"), "{edited}");
    let edited = ok(s.path(), &["edit", &epic, "--tag", "cache", "--json"]);
    assert!(edited.contains(r#""status":"done""#) && edited.contains(r#""derived_status":"in_progress""#), "{edited}");
    // **되풀이해 불러도 같은 모양이다.** 바뀐 것이 없을 때만 키가 사라지면 재시도한
    // 쪽은 그 줄이 묶음이 아닌 줄 알고 적힌 칸을 읽는다.
    let again = ok(s.path(), &["edit", &epic, "--tag", "cache", "--json"]);
    assert!(again.contains(r#""derived_status":"in_progress""#), "바뀐 것이 없다고 키를 뺐다 — {again}");
}

/// **줄을 내는 표면은 모두 서 있는 칸을 말한다.** `show` 만 곁들이면 `add`·`defer`·
/// `link` 를 읽는 쪽은 같은 에픽을 적힌 칸으로 읽고, 만든 사람은 `add` 가 낸 글리프와
/// 바로 다음 `show` 가 다른 칸을 말하는 것을 본다.
#[test]
fn every_surface_that_prints_a_group_reads_its_column() {
    let s = init("groupsurface");
    // 만든 칸은 안 읽힌다 — 멤버가 없으니 첫 칸에 선다.
    let made = ok(s.path(), &["add", "저장 계층", "--type", "epic", "-s", "done"]);
    assert!(made.contains('·') && !made.contains('✓'), "만든 줄이 적힌 칸을 그린다 — {made}");
    let epic = made.split_whitespace().next().unwrap().to_string();
    let held = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    ok(s.path(), &["mv", &held, "in_progress"]);

    let json = ok(s.path(), &["add", "다른 에픽", "--type", "epic", "-s", "review", "--json"]);
    assert!(json.contains(r#""derived_status":"todo""#), "add --json 이 읽은 칸을 안 낸다 — {json}");
    for args in [
        vec!["defer", &epic, "-m", "잠깐"],
        vec!["defer", &epic, "--undo"],
        vec!["link", &held, "--blocks", &epic],
    ] {
        let mut args = args.clone();
        args.push("--json");
        let json = ok(s.path(), &args);
        assert!(
            json.contains(r#""derived_status":"in_progress""#),
            "{args:?} 가 읽은 칸을 안 낸다 — {json}"
        );
    }
    // 펼친 계획의 에픽도 같다.
    let idea = ok(s.path(), &["idea", "add", "캐시 층", "-q"]).trim().to_string();
    let grown =
        from_stdin(s.path(), &["idea", "promote", &idea, "--from", "-", "--json"], "# 캐시 층\n- [p2] 첫 이슈\n");
    let json = String::from_utf8_lossy(&grown.stdout).to_string();
    assert!(json.contains(r#""derived_status":"todo""#), "펼친 에픽이 읽은 칸을 안 낸다 — {json}");
}

/// **`--stale` 도 서 있는 칸의 나이를 잰다.** `-s` 는 읽은 칸으로 고르는데 나이만
/// 적힌 칸의 시각으로 재면, 오늘 진행 중이 된 에픽이 "열흘째 멈춰 있다" 로 걸린다.
#[test]
fn stale_ages_a_group_by_its_members() {
    let s = init("groupstale");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let one = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    // 열흘 뒤에 집었다 — 에픽 줄의 시각은 만들어진 그날 그대로다.
    let late = at(s.path(), "2026-09-21T04:12:03Z", &["mv", &one, "in_progress"]);
    assert!(late.status.success());
    let shown = |now: &str| -> String {
        let out = at(s.path(), now, &["show", "-s", "in_progress", "--stale", "3"]);
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert!(!shown("2026-09-21T04:12:03Z").contains(&epic), "오늘 진행 중이 된 에픽을 멈춘 것으로 셌다");
    assert!(shown("2026-10-01T04:12:03Z").contains(&epic), "멤버가 열흘째 안 움직이는데 안 셌다");
}

/// **묶음이 제 멤버를 막는 것은 고리다.** 묶음은 멤버가 다 끝나야 닫히므로 둘 다 영영
/// 안 끝나고, 적힌 칸을 `done` 으로 옮겨 푸는 길은 이제 없다.
#[test]
fn a_grouping_cannot_block_its_own_member() {
    let s = init("groupcycle");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let one = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let outside = add(s.path(), &["남의 일"]);

    let out = moai(s.path(), &["link", &epic, "--blocks", &one]);
    assert!(!out.status.success(), "제 멤버를 막게 뒀다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("고리"), "{:?}", out.stderr);
    // 남을 막는 것은 그대로 된다.
    assert!(moai(s.path(), &["link", &epic, "--blocks", &outside]).status.success());
}

/// **묶음을 옮기는 것은 막지 않되, 서 있는 칸을 말한다.** 말하지 않으면 옮긴
/// 사람은 에픽이 닫힌 줄 알고 화면은 계속 `in_progress` 를 그린다.
#[test]
fn moving_a_group_writes_and_says_where_it_stands() {
    let s = init("groupmv");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let held = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    ok(s.path(), &["mv", &held, "in_progress"]);

    let out = ok(s.path(), &["mv", &epic, "done"]);
    assert!(line_of(s.path(), &epic).contains(r#""status":"done""#), "쓰기를 막았다");
    assert!(out.contains("서 있는 칸은 in_progress"), "{out}");
    // **끝난 멤버가 없으면 멤버를 미뤄도 안 닫힌다** — 셀 멤버가 비면 첫 칸이다.
    // 시킨 대로 했는데 같은 말이 돌아오는 자리라, 접는 길을 제대로 댄다.
    assert!(out.contains(&format!("moai defer {epic}")), "접을 수 없는데 멤버를 미루라고 한다 — {out}");
    let again = ok(s.path(), &["mv", &epic, "done"]);
    assert!(again.contains("서 있는 칸은 in_progress"), "이미 그 칸이면 입을 다물었다 — {again}");
    // **기계 출력도 같은 것을 말한다.** 이미 그 칸이던 묶음은 `already` 에 id 뿐이라,
    // 여기 없으면 되풀이해 부른 쪽만 그 칸을 모른다.
    let json = ok(s.path(), &["mv", &epic, "done", "--json"]);
    assert!(json.contains(r#""already":["#) && json.contains(r#""derived_status":"in_progress""#), "{json}");

    // 끝난 멤버가 하나 생기면 남은 멤버를 미뤄 접을 수 있다 — 그때는 그렇게 댄다.
    let closed = add(s.path(), &["락", "-e", &epic]);
    ok(s.path(), &["mv", &closed, "done"]);
    let out = ok(s.path(), &["mv", &epic, "done"]);
    assert!(out.contains("접으려면 남은 멤버를"), "접을 수 있는데 안 알려 준다 — {out}");
    ok(s.path(), &["defer", &held, "-m", "다음에"]);
    let out = ok(s.path(), &["mv", &epic, "done"]);
    assert!(!out.contains("서 있는 칸"), "접었는데 아직 말한다 — {out}");
    ok(s.path(), &["defer", &held, "--undo"]);

    let json = ok(s.path(), &["mv", &epic, "review", "--json"]);
    assert!(json.contains(r#""derived_status":"in_progress""#), "{json}");

    // 읽은 칸과 같은 칸으로 옮기면 말할 것이 없다. 일을 옮길 때도 없다.
    let out = ok(s.path(), &["mv", &epic, "in_progress"]);
    assert!(!out.contains("서 있는 칸"), "{out}");
    assert!(!ok(s.path(), &["mv", &held, "done"]).contains("서 있는 칸"));
}

/// **빈 묶음이 막으면 `ready` 가 까닭을 댄다**(moai-1c2l). 멤버 없는 에픽은 영영 안
/// 풀리는데 끝난 것도 미룬 것도 아니라, 안 대면 목록이 까닭 없이 빈다.
#[test]
fn ready_names_an_empty_group_that_blocks() {
    let s = init("emptyblock");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let work = add(s.path(), &["원자적 쓰기"]);
    ok(s.path(), &["link", &epic, "--blocks", &work]);
    let r = ok(s.path(), &["ready"]);
    assert!(r.contains("멤버가 없는 묶음에 막혀"), "왜 비었는지 안 말한다 — {r}");
    assert!(r.contains(&format!("moai link {epic} --unblocks {work}")), "푸는 말을 안 댄다 — {r}");
    assert!(!r.contains("미뤄 둔 것에 막혀"), "미룬 것이 없는데 미뤘다고 한다 — {r}");
    // 기계 출력도 빈 묶음을 댄다(moai-w6n2). 도로 집을 것이 없으니 `by`·`undo` 는 안 선다.
    let json = ok(s.path(), &["ready", "--json"]);
    assert_eq!(json.trim(), format!(r#"{{"ready":[],"held":[{{"id":"{work}","empty":["{epic}"]}}]}}"#), "{json}");

    // 채우면 보통 막음이다 — 까닭을 따로 대지 않는다.
    let lock = add(s.path(), &["락", "-e", &epic]);
    assert!(!ok(s.path(), &["ready"]).contains("멤버가 없는"));
    // 막힌 것이 없으면 held 는 빈 배열로 선다 — 키가 있다 없다로 모양이 흔들리지 않는다.
    let json = ok(s.path(), &["ready", "--json"]);
    assert!(json.starts_with(&format!(r#"{{"ready":[{{"id":"{lock}""#)) && json.trim_end().ends_with(r#""held":[]}"#), "{json}");
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
    isolated(BIN)
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
    let mut child = isolated(BIN)
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
    // **stdin 을 안 읽고 돌아서는 길이 있다.** `idea add --from -` 처럼 먼저
    // 거절하는 명령은 우리가 쓰기를 마치기 전에 끝나 있을 수 있고, 그때 쓰기는
    // EPIPE 로 끝난다 — 그것은 시험이 보려던 거절 그 자체다. 여기서 패닉하면
    // 같은 시험이 기계 부하에 따라 붙었다 떨어졌다 한다.
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
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

/// **연습도 `--json` 을 지킨다.** 계획을 미리 검사하는 쪽은 `--dry-run` 과
/// `--json` 을 같이 쓴다. 여기서 사람 글이 나오면 그쪽은 파싱에 실패하고,
/// 그러면 진짜로 만들어 보고서야 계획을 읽는다 — 연습이 막으려던 그 일이다.
#[test]
fn a_rehearsal_still_speaks_json() {
    let s = init("dryrunjson");
    let out = from_stdin(s.path(), &["add", "--from", "-", "--dry-run", "--json"], PLAN);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    one_json_value(&text);
    // 연습임이 출력에 적혀 있어야 한다 — id 가 없는 까닭이 거기서 나온다.
    assert!(text.contains(r#""dry_run":true"#), "{text}");
    assert!(text.contains(r#""title":"저장 계층""#), "{text}");
    assert!(text.contains(r#""priority":1"#), "{text}");
    assert_eq!(issues(s.path()), "", "연습인데 썼다");
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
    assert!(md.contains("moai status") && md.contains("승인 게이트가 없다"), "{md}");
    // 훅이 서는 규칙도 같은 출처에서 온다 — 스킬에만 적혀 있던 자리다.
    assert!(md.contains("리뷰도 이슈다"), "규칙 셋이 빠졌다\n{md}");
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
        // `hook` 도 그렇다 — 출력이 언제나 기계용 계약 JSON 이라 `--json` 이
        // 켤 것이 없고, stdin 없이 부르면 아무 말도 안 하는 것이 맞다.
        // 대신 `the_hook_*` 시험들이 그 계약을 더 깐깐하게 본다.
        .filter(|c| c != "help" && c != "hook")
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
    "init", "add", "status", "ready", "show", "note", "link", "defer", "tui", "edit", "mv", "rm",
    // `skill` 은 `--dry-run` 으로만 부른다. 진짜 설치는 `claude` 를 부르고
    // 사람의 설정을 건드리므로 훑기가 할 일이 아니다.
    "skill",
    // 종류 네임스페이스는 `moai <종류> show --json` 으로 같은 길을 지난다.
    "issue", "epic", "milestone", "idea",
    // 사용자 설정을 고친다. `add`·`rm` 은 제 설정 파일로 따로 부른다 — 공용 집에 쓰면 안 된다.
    "project",
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
        vec!["defer", &id, "--json"],
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
        vec!["skill", "install", "--dry-run", "--json"],
        // 장부를 읽기만 한다. `uninstall` 은 `claude` 를 부르므로 훑지 않는다 —
        // 가짜 `claude` 로 따로 본다 (`skill_*` 시험).
        vec!["skill", "status", "--json"],
        // 공용 설정(없는 파일)을 읽기만 한다.
        vec!["project", "ls", "--json"],
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
    // 쓰는 `project` 동사는 제 설정 파일로 — 공용 집을 비워 둔다.
    let config = s.path().join("user-config.toml");
    for args in [
        &["project", "add", ".", "--json"][..],
        &["project", "color", ".", "green", "--json"],
        &["project", "rm", ".", "--json"],
    ] {
        one_json_value(&project_ok(s.path(), &config, args));
    }
    // 한 번에 만들기도 배열 하나를 낸다
    let bulk = from_stdin(s.path(), &["add", "--from", "-", "--json"], "# 가\n- 나\n");
    assert!(bulk.status.success());
    one_json_value(&String::from_utf8(bulk.stdout).unwrap());
}

/// **시험을 돌리는 사람의 `~/.claude` 와 git 설정이 시험에 새지 않는다.**
///
/// 새는 것은 시험 프로세스가 물려받은 환경이라, 그 환경을 바꿔 보려면 시험을
/// 한 겹 더 띄워야 한다 — 병렬로 도는 시험 안에서 `set_var` 는 남의 시험까지
/// 바꾼다. 그래서 이 시험은 제 바이너리를 **자기 자신만** 돌리게 다시 부른다.
/// 바깥은 가짜 "진짜 집" 에 moai 가 심긴 장부와 사람 이름을 심고, 그것이
/// 샐 때는 실제로 보인다는 것부터 확인한다 — 안 보이는 표지로는 아무것도 못 증명한다.
#[test]
fn tests_do_not_read_the_runners_home() {
    const PROBE: &str = "MOAI_CLI_LEAK_PROBE";
    let skill_status = |dir: &Path| String::from_utf8(moai(dir, &["skill", "status", "--json"]).stdout).unwrap();

    // 안쪽: 물려받은 환경이 새는 집을 가리킨다. 공용 도우미로 부르면 안 보여야 한다.
    if let Some(repo) = std::env::var_os(PROBE) {
        let repo = PathBuf::from(repo);
        let json = skill_status(&repo);
        assert!(json.contains("\"installs\":[]"), "시험이 사람의 장부를 읽었다\n{json}");
        // 제 사람을 안 주면 git 에서 찾는다. 새는 집의 git 설정이나 물려받은
        // `MOAI_ACTOR` 가 보이면 여기서 이름이 붙어 쓰기가 지나간다.
        let bare = isolated(BIN).args(["add", "누구냐"]).current_dir(&repo).output().unwrap();
        // 다른 까닭으로 넘어져도 초록이 되지 않게, 사람을 못 찾아 멈춘 것인지까지 본다.
        assert!(!bare.status.success(), "시험이 사람의 이름을 읽었다\n{}", text(&bare));
        assert!(text(&bare).contains("git 사용자 정보가 없다"), "사람을 못 찾아 멈춘 것이 아니다\n{}", text(&bare));
        return;
    }

    let s = init("leakprobe");
    let root = s.path().canonicalize().unwrap();
    let market = field(&ok(s.path(), &["skill", "install", "--dry-run", "--json"]), "market");
    let leak = Scratch::new("leakprobe-home");
    let ledger = |plugins: &Path| {
        std::fs::create_dir_all(plugins).unwrap();
        std::fs::write(
            plugins.join("installed_plugins.json"),
            format!(
                "{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"/없는/자리\",\"version\":\"0.0.1\"}}]}}}}",
                root.display()
            ),
        )
        .unwrap();
    };
    let home = leak.path().join("home");
    let config = leak.path().join("config");
    let cache = leak.path().join("cache");
    ledger(&home.join(".claude/plugins"));
    ledger(&config.join("plugins"));
    ledger(&cache);
    let gitconfig = "[user]\n\tname = 새는 사람\n\temail = leak@example.com\n";
    std::fs::write(home.join(".gitconfig"), gitconfig).unwrap();
    std::fs::create_dir_all(leak.path().join("xdg/git")).unwrap();
    std::fs::write(leak.path().join("xdg/git/config"), gitconfig).unwrap();
    let leaky: [(&str, &Path); 4] = [
        ("HOME", &home),
        ("CLAUDE_CONFIG_DIR", &config),
        ("CLAUDE_CODE_PLUGIN_CACHE_DIR", &cache),
        ("XDG_CONFIG_HOME", &leak.path().join("xdg")),
    ];

    // 표지가 샐 때 실제로 보이는가. 장부는 자리마다 따로 본다.
    for (var, at) in &leaky[..3] {
        let out = Command::new(BIN)
            .args(["skill", "status", "--json"])
            .current_dir(s.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env(var, at)
            .output()
            .unwrap();
        let json = String::from_utf8(out.stdout).unwrap();
        assert!(!json.contains("\"installs\":[]"), "{var} 에 심은 장부가 새도 안 보인다 — 표지가 헛것이다\n{json}");
    }
    for (var, at) in [leaky[0], leaky[3]] {
        let out = Command::new(BIN)
            .args(["add", "누구냐", "-q"])
            .current_dir(s.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            // 기계의 `/etc/gitconfig` 에 이름이 있으면 심은 것 없이도 지나가 표지를 못 잰다.
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(var, at)
            .output()
            .unwrap();
        assert!(out.status.success(), "{var} 에 심은 git 설정이 새도 안 보인다 — 표지가 헛것이다\n{}", text(&out));
    }

    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--exact", "tests_do_not_read_the_runners_home", "--nocapture"]).env(PROBE, s.path());
    for (var, at) in &leaky {
        child.env(var, at);
    }
    child.env("MOAI_ACTOR", "새는 사람 (leak@example.com)").env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"));
    let out = child.output().unwrap();
    let said = text(&out);
    assert!(out.status.success(), "안쪽 시험이 실패했다\n{said}");
    // 안쪽이 이름을 못 찾아 돌지 않고 초록으로 끝나면 이 시험은 아무것도 안 봤다.
    assert!(said.contains("1 passed"), "안쪽 시험이 돌지 않았다\n{said}");
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

/// 도움말에서 들여쓰기를 잃은 줄.
///
/// clap 이 그리는 몫에서 `Usage:` 뒤에 들여쓰지 않고 서는 줄은 `Options:` 같은
/// 머리(`:` 로 끝남)뿐이다 — 설명은 모두 들여쓴다. 그러니 `Usage:` 뒤의 들여쓰지
/// 않은 머리 아닌 줄은 `after_help` 의 글이고, **그 뒤에 머리 없이 들여쓴 줄이
/// 다시 서면** 앞 줄이 제 들여쓰기를 잃은 것이다. `after_help = "\` 의 줄 잇기가
/// 개행과 함께 다음 줄의 앞 공백까지 먹어 첫 줄만 왼쪽 끝에 붙던 모양이 정확히
/// 이것이다 (moai-p63y). 바로 밑 줄만 보면 첫 문단이 한 줄인 것(`hook`)을 놓친다.
fn lost_indent(help: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let mut prose: Option<&str> = None;
    for l in help.lines().skip_while(|l| !l.starts_with("Usage:")).skip(1) {
        if l.trim().is_empty() {
            continue;
        }
        if l.starts_with(char::is_whitespace) {
            if let Some(p) = prose.take() {
                bad.push(format!("{p}\n{l}"));
            }
        } else if l.trim_end().ends_with(':') {
            prose = None;
        } else {
            prose = prose.or(Some(l));
        }
    }
    bad
}

/// 도움말의 `Commands:` 밑에 선 이름들. `help` 는 clap 이 만드는 것이라 뺀다.
fn subcommands(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|l| !l.starts_with("Commands:"))
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .filter(|c| c != "help")
        .collect()
}

/// 모든 명령(하위 명령까지)의 `--help` 에서 예시·설명 줄이 들여쓰기를 지킨다.
/// **명령 목록은 바이너리의 도움말에서 읽는다** — 새 명령이 `after_help = "\` 로
/// 같은 덫을 밟아도 이름을 적어 넣지 않고 걸린다.
#[test]
fn every_help_keeps_its_indent() {
    let s = init("helpindent");
    // 판정이 헛돌지 않는지 먼저 본다 — 고치기 전 `status --help` 의 모양 그대로.
    let head = "Usage: moai status [OPTIONS]\n\nOptions:\n  -h, --help  Print help\n\n";
    for broken in [
        "아무것도 막지 않는다. 승인도 통과도 없다.\n  대신 에픽에 안 붙은 이슈를 드러낸다.\n",
        "사람이 손으로 부를 일은 없다.\n\n  **아무것도 막지 않고**\n",
    ] {
        let found = lost_indent(&format!("{head}{broken}"));
        assert_eq!(found.len(), 1, "판정이 잃은 들여쓰기를 못 알아본다 — {broken:?}");
    }
    for fine in [
        "예시:\n  moai add \"제목\"\n\n한 번에 여럿:\n  moai add --from -\n\n제목은 `--` 로 시작해도 된다.\n",
        "  아무것도 막지 않는다.\n  대신 드러낸다.\n",
    ] {
        let found = lost_indent(&format!("{head}{fine}"));
        assert!(found.is_empty(), "멀쩡한 글을 잡는다 — {found:?}");
    }

    let mut queue: Vec<Vec<String>> = vec![vec![]];
    let mut seen = Vec::new();
    let mut with_examples = 0;
    let mut bad = Vec::new();
    while let Some(path) = queue.pop() {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let help = ok(s.path(), &args);
        if help.lines().any(|l| l.starts_with("  moai ")) {
            with_examples += 1;
        }
        for b in lost_indent(&help) {
            bad.push(format!("moai {} --help:\n{b}", path.join(" ")));
        }
        for c in subcommands(&help) {
            let mut next = path.clone();
            next.push(c);
            queue.push(next);
        }
        seen.push(path.join(" "));
    }
    // 훑기가 실제로 돌았는지 — 하위 명령까지 내려갔고 예시 줄을 가진 도움말을 봤다.
    assert!(seen.len() > 20, "명령 목록을 못 읽었다 — {seen:?}");
    assert!(seen.iter().any(|c| c == "skill install"), "하위 명령으로 안 내려갔다 — {seen:?}");
    assert!(with_examples >= 8, "예시 줄이 있는 도움말이 {with_examples} 개뿐이다 — {seen:?}");
    assert!(bad.is_empty(), "들여쓰기를 잃은 줄:\n\n{}", bad.join("\n\n"));
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

/// **설정이 깨진 저장소 안의 인자 없는 `moai` 는 그 설정을 댄다** — `status` 와 같은 말,
/// 같은 종료 코드로. 도움말과 "아직 moai 저장소가 아니다" 를 내면 사람은 제 저장소를
/// 믿지 못하고 `moai init` 을 다시 친다 (moai-byih).
#[test]
fn bare_moai_inside_names_a_broken_repo_config() {
    let s = init("bare-badcfg");
    std::fs::write(s.path().join(".moai/config.toml"), "prefix = \"\"\n").unwrap();
    for json in [false, true] {
        let flag: &[&str] = if json { &["--json"] } else { &[] };
        let bare = moai(s.path(), flag);
        let status = moai(s.path(), &[flag, &["status"]].concat());
        let said = text(&bare);
        assert!(!said.contains("아직 moai 저장소가 아니다"), "json={json}\n{said}");
        assert!(said.contains("config.toml") && said.contains("prefix"), "json={json}: 깨진 설정을 안 댔다\n{said}");
        assert_eq!(bare.status.code(), status.status.code(), "json={json}\n{said}");
        assert_eq!(bare.stderr, status.stderr, "json={json}: status 와 다른 말을 한다\n{said}");
    }
}

// ── 누가 하는가 ───────────────────────────────────────────────────────

/// `MOAI_ACTOR` 를 걷고 git 이 읽을 설정을 통째로 지정해 돌린다. moai 는 git
/// 저장소를 요구하지 않으므로 전역·시스템 설정까지 막아야 사람을 못 찾는
/// 상황을 실제로 만들 수 있다.
fn with_git_config(dir: &Path, cfg: &str, args: &[&str]) -> Output {
    isolated(BIN)
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
    let mut child = isolated(BIN)
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

/// 연습과 `--json` 을 같이 줘도 기계 출력이다. `add --from` 쪽과 **같은
/// 모양**이라야 계획을 미리 검사하는 코드가 두 동사에 두 벌 필요하지 않다.
#[test]
fn a_rehearsed_promote_still_speaks_json() {
    let s = init("ideadryjson");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let before = issues(s.path());
    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-", "--dry-run", "--json"],
        "# 새 에픽\n- [p1] 첫 일\n",
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    one_json_value(&text);
    assert!(text.contains(r#""dry_run":true"#), "{text}");
    assert!(text.contains(r#""title":"새 에픽""#), "{text}");
    // 무엇이 닫힐 것인지는 진짜 출력과 같은 낱말로 말한다.
    assert_eq!(field(&text, "promoted"), id, "{text}");
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
    for want in ["moai idea add", "moai idea ls", "moai idea promote", "moai defer"] {
        assert!(block.contains(want), "{want} 가 없다 — {block}");
    }
}

/// **마크다운은 `idea add` 로 들어오지 않는다.** `#` 이 에픽이고 `-` 가
/// 이슈라는 뜻이 형식에 박혀 있어 종류 고정 장치가 거기까지 못 가고, 그래서
/// 담은 줄 알았던 것이 그대로 보드에 선다 — 일로 세지 않으려고 담은 것이
/// 조용히 일이 되는 것이 이 기능이 막으려던 바로 그것이다.
#[test]
fn a_plan_cannot_be_poured_in_through_idea_add() {
    let s = init("ideafrom");
    let before = issues(s.path());
    let out = from_stdin(s.path(), &["idea", "add", "--from", "-"], "# 에픽\n- 이슈\n");
    assert!(!out.status.success(), "생각 담는 자리로 계획이 들어왔다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("promote"), "어디로 가야 하는지 안 말한다 — {err}");
    assert_eq!(issues(s.path()), before, "거절했는데 썼다");
    // 다른 네임스페이스는 그대로다 — 거기서는 마크다운이 뜻이 통한다.
    let bulk = from_stdin(s.path(), &["epic", "add", "--from", "-"], "# 에픽\n- 이슈\n");
    assert!(bulk.status.success(), "{}", String::from_utf8_lossy(&bulk.stderr));
}

/// 이미 닫힌 생각을 또 펼쳐도 **일어나지 않은 전이를 적지 않는다.**
/// `done → done` 을 적으면 저널이 거짓말을 하고 `status_since` 가 밀려
/// "언제 닫혔나" 를 잃는다. 적어 온 말은 그래도 남는다 — `mv` 와 같은 규칙이다.
#[test]
fn promoting_a_closed_thought_forges_no_transition() {
    let s = init("ideatwice");
    let id = ok(s.path(), &["idea", "add", "두 번 펼칠 것", "-q"]).trim().to_string();
    for plan in ["# 첫 계획\n- 가\n", "# 둘째 계획\n- 나\n"] {
        let out = from_stdin(s.path(), &["idea", "promote", &id, "--from", "-"], plan);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let closed = line_of(s.path(), &id);
    let mine: Vec<&str> =
        journal(s.path()).lines().filter(|l| l.contains(&format!(r#""id":"{id}""#))).map(|l| {
            // 줄 자체를 들고 가면 소유권이 걸린다 — 뜻만 본다.
            if l.contains(r#""kind":"status""#) {
                "status"
            } else if l.contains(r#""kind":"note""#) {
                "note"
            } else {
                "create"
            }
        }).collect();
    // 첫 번은 진짜 전이고, 둘째 번은 전이가 아니라 적어 온 말이다.
    assert_eq!(mine, ["create", "status", "note"], "{mine:?}");
    assert!(closed.contains(r#""status":"done""#), "{closed}");
}

/// `--json` 이 **닫힌 생각까지** 말한다. 사람 출력에는 `→ done` 이 있는데
/// 기계 출력에만 없으면 받는 쪽이 두 표면 중 하나를 못 믿게 된다.
#[test]
fn promote_json_names_the_thought_it_closed() {
    let s = init("ideapromoted");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-", "--json"],
        "# 새 에픽\n- 첫 일\n",
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&id), "무엇을 닫았는지 안 말한다 — {text}");
    assert!(text.contains(r#""status":"done""#), "{text}");
    assert!(text.contains("새 에픽") && text.contains("첫 일"), "{text}");
}

/// idea 밑에 만든 자식은 **그 밑에 접힌다.** 잎으로 서는 것은 부모가 될 수
/// 있고, 여기서만 idea 를 빼면 자식이 부모를 잃고 뿌리로 떠올라 — 그러면
/// 그 idea 는 열리지도 않는다.
#[test]
fn a_child_of_a_thought_stays_under_it() {
    let s = init("ideachild");
    let parent = ok(s.path(), &["idea", "add", "부모 생각", "-q"]).trim().to_string();
    let child = add(s.path(), &["그 자식", "--parent", &parent]);
    assert!(child.starts_with(&format!("{parent}.")), "{child}");

    let root = ok(s.path(), &["tui", "--json"]);
    assert!(!root.contains(&child), "자식이 뿌리로 떠올랐다 — {root}");
    assert!(root.contains(r#""dir":true"#), "부모가 열리지 않는다 — {root}");

    let inside = ok(s.path(), &["tui", "--json", "--path", &parent]);
    assert!(inside.contains(&child), "열었는데 자식이 없다 — {inside}");
}

/// **에픽에 든 생각 밑의 자식도 그 에픽에 안 세어진다.** 생각은 뿌리로 올라
/// 그 자식도 같이 올라가는데, 셈만 생각의 에픽을 물려주면 에픽 상세가
/// `멤버 0/1` 을 내고 그 밑에 줄이 없다(moai-14dm).
#[test]
fn a_child_of_a_thought_in_an_epic_is_counted_where_it_is_drawn() {
    let s = init("ideachildepic");
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "-q"]).trim().to_string();
    let thought = ok(s.path(), &["idea", "add", "샤딩", "-e", &epic, "-q"]).trim().to_string();
    let child = add(s.path(), &["생각의 자식", "--parent", &thought]);

    let detail = ok(s.path(), &["show", &epic]);
    assert!(!detail.contains("0/1"), "그리지 않는 줄을 셌다 — {detail}");
    let tree = ok(s.path(), &["show", "--tree"]);
    assert!(tree.contains(&child), "{tree}");
    assert!(tree.contains("에픽 없음"), "{tree}");
    assert!(!ok(s.path(), &["show", "-e", &epic]).contains(&child), "-e 가 그리는 자리와 다른 것을 고른다");
}

/// **담아 둔 생각이 일을 가로막지 않는다.** idea 를 이슈 밑에 달아 두면
/// 그 이슈가 `ready` 에서 사라졌다 — 그런데 idea 는 어느 목록에도 안 나오니
/// 왜 사라졌는지 볼 방법이 없었다. 조용히 멈추는 것이 제일 나쁘다.
#[test]
fn a_thought_parked_under_work_does_not_stall_it() {
    let s = init("ideachild");
    let work = add(s.path(), &["진짜 일"]);
    ok(s.path(), &["idea", "add", "나중에 볼 것", "--parent", &work, "-q"]);
    let r = ok(s.path(), &["ready"]);
    assert!(r.contains(&work), "담아 둔 생각이 일을 멈춰 세웠다 — {r}");
}

/// 생각은 막는 것이 아니다. idea 는 보통 `done` 에 닿지 않으므로, 막게 두면
/// 막힌 이슈가 영영 안 풀리고 `status` 는 그것을 "계획이 멈춘 자리" 로 센다.
#[test]
fn a_thought_cannot_block_work() {
    let s = init("ideablock");
    let thought = ok(s.path(), &["idea", "add", "먼저 생각", "-q"]).trim().to_string();
    let work = add(s.path(), &["막힐 일"]);
    let out = moai(s.path(), &["link", &thought, "--blocks", &work]);
    assert!(!out.status.success(), "생각이 일을 막게 뒀다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("promote"), "어디로 가야 하는지 안 말한다 — {err}");
    assert!(ok(s.path(), &["ready"]).contains(&work), "거부해 놓고 막았다");
}

/// 연습이 승인의 자리다. 그 자리가 못 할 일을 하겠다고 말하면, 사람이
/// "좋다" 한 뒤에야 도구가 거절한다.
#[test]
fn the_rehearsal_checks_what_the_real_run_checks() {
    let s = init("ideadrycheck");
    let work = add(s.path(), &["진짜 일"]);
    for target in [work.as_str(), "argos-zzzz"] {
        let out = from_stdin(
            s.path(),
            &["idea", "promote", target, "--from", "-", "--dry-run"],
            "# 가\n- 나\n",
        );
        assert!(!out.status.success(), "{target} 를 펼치겠다고 했다");
    }
}

/// `-a none` 으로 담은 생각은 임자 없이 펼쳐진다. 연습 삼아 담아 둔 것이
/// 펼치는 순간 누군가의 일이 되면, 그 사람은 시키지도 않은 일을 떠안는다.
#[test]
fn promoting_keeps_the_thought_unowned() {
    let s = init("ideaowner");
    let id = ok(s.path(), &["idea", "add", "임자 없는 생각", "-a", "none", "-q"]).trim().to_string();
    let out = from_stdin(s.path(), &["idea", "promote", &id, "--from", "-", "--json"], "# 가\n- 나\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let made = String::from_utf8(out.stdout).unwrap();
    assert!(!made.contains("assignee"), "임자 없이 담은 것에 임자가 붙었다 — {made}");
}

/// 종류를 `--type` 으로 적어도 같은 자리에서 막힌다. 한쪽 철자만 막으면
/// 다른 철자가 그대로 보드에 이슈를 만든다.
#[test]
fn a_plan_cannot_be_poured_in_as_a_type_flag_either() {
    let s = init("ideatypefrom");
    let out = from_stdin(s.path(), &["add", "--from", "-", "--type", "idea"], "# 묶음\n- 하나\n");
    assert!(!out.status.success(), "idea 라 적었는데 에픽과 이슈를 만들었다");
    assert_eq!(issues(s.path()), "", "거부해 놓고 썼다");
}

/// 담아 둔 것이 있는데 목록이 그냥 "없다." 라고 하면, 방금 담은 사람은
/// 파일이 비었다고 믿는다. done 을 숨길 때 그 수를 말하는 것과 같은 규칙이다.
#[test]
fn an_empty_list_says_the_thoughts_are_hidden() {
    let s = init("ideahiddencount");
    for n in 0..3 {
        ok(s.path(), &["idea", "add", &format!("생각 {n}"), "-q"]);
    }
    let out = ok(s.path(), &["show"]);
    assert!(out.contains('3') && out.contains("idea"), "숨긴 것을 안 센다 — {out}");
}

/// **한 화면이 두 말을 하지 않는다.** 에픽에 든 생각을 상세가 줄로 내면서
/// 머리글은 `멤버 0/0` 이라 하면, 보는 쪽은 어느 쪽도 못 믿는다. 세는 쪽은
/// 못 바꾸므로(진행률이 생각을 세면 담을수록 덜 끝난 것으로 보인다) 자리를
/// 맞춘다 — 소속은 필드로 남아 `--type idea -e` 가 찾아낸다.
#[test]
fn a_thought_does_not_hang_under_an_epic() {
    let s = init("ideaepic");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let thought =
        ok(s.path(), &["idea", "add", "샤딩을 해 볼까", "-e", &epic, "-q"]).trim().to_string();

    let detail = ok(s.path(), &["show", &epic]);
    assert!(!detail.contains(&thought), "멤버 0/0 밑에 생각을 그렸다 — {detail}");
    assert!(!ok(s.path(), &["show", "--tree"]).contains(&thought));

    // 소속은 잃지 않았다.
    let found = ok(s.path(), &["show", "--type", "idea", "-e", &epic]);
    assert!(found.contains(&thought), "에픽으로 못 찾는다 — {found}");
}

// ── 못 읽는 줄 ──────────────────────────────────────────────────────

/// 읽을 수 없는 줄 하나가 파일 전체의 쓰기를 막지 않는다.
///
/// **CLAUDE.md 의 규칙 그대로다** — 엄함은 *지금 쓰는 줄*에 대한 것이지 파일
/// 전체에 대한 것이 아니다. 막으면 되돌릴 방법이 도구 밖에만 남는다.
#[test]
fn one_unreadable_line_does_not_block_every_write() {
    let s = init("opaquewrite");
    let good = add(s.path(), &["멀쩡한 일"]);
    let bad = r#"{"id":"argos-9999","title":"뒷 단계가 쓴 줄","kind":"몰라","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#;
    std::fs::write(
        s.path().join(".moai/issues.jsonl"),
        format!("{bad}\n{}", issues(s.path())),
    )
    .unwrap();

    // 쓰기가 된다. 그리고 조용히 되지 않는다.
    let noisy = moai(s.path(), &["add", "그래도 만들어진다"]);
    assert!(noisy.status.success(), "{}", String::from_utf8_lossy(&noisy.stderr));
    assert!(
        String::from_utf8_lossy(&noisy.stderr).contains("그대로 두고 썼다"),
        "막지 않는 대신 시끄러워야 한다 — {}",
        String::from_utf8_lossy(&noisy.stderr)
    );
    let made = add(s.path(), &["그래도 만들어진다 둘"]);
    assert!(issues(s.path()).contains(&made), "못 읽는 줄 하나가 쓰기를 막았다");
    ok(s.path(), &["mv", &good, "done"]);

    // 그 줄은 **글자 하나 안 바뀌고** 남는다.
    let after = issues(s.path());
    assert!(after.contains(bad), "모르는 줄을 잃었다 — {after}");
}

/// 잃지 않는 것으로 끝이 아니다. **시끄러워야 한다** — 조용히 지나가면
/// 그 줄이 무엇인지 아무도 안 본다.
#[test]
fn an_unreadable_line_still_shouts() {
    let s = init("opaqueloud");
    add(s.path(), &["멀쩡한 일"]);
    std::fs::write(
        s.path().join(".moai/issues.jsonl"),
        format!("이건 JSON 도 아니다\n{}", issues(s.path())),
    )
    .unwrap();

    let out = moai(s.path(), &["status"]);
    assert!(!out.status.success(), "깨진 데이터인데 0 으로 끝났다");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("읽을 수 없는 줄"), "{text}");
}

/// 읽고 그대로 쓰면 바이트가 같다 — 모르는 줄이 섞여 있어도. 깨지면 매
/// 명령이 헛 diff 를 만든다.
#[test]
fn a_file_with_an_unreadable_line_is_idempotent() {
    let s = init("opaquestable");
    add(s.path(), &["멀쩡한 일"]);
    let bad = r#"{"id":"argos-9999","title":"모르는 종류","kind":"몰라","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#;
    std::fs::write(
        s.path().join(".moai/issues.jsonl"),
        format!("{}{bad}\n", issues(s.path())),
    )
    .unwrap();

    let once = { add(s.path(), &["한 번 쓴다"]); issues(s.path()) };
    add(s.path(), &["두 번 쓴다"]);
    let twice = issues(s.path());
    let settled: Vec<&str> = twice.lines().filter(|l| !l.contains("두 번 쓴다")).collect();
    assert_eq!(settled.join("\n") + "\n", once, "쓸 때마다 줄이 움직인다");
}

// ── 미루기 ──────────────────────────────────────────────────────────

/// 미루면 `deferred_at` 이 붙고, 도로 집으면 사라진다. **같은 줄이 그대로
/// 돌아온다** — 종류도 칸도 안 건드린다.
#[test]
fn deferring_and_taking_it_back_leaves_the_row_itself_alone() {
    let s = init("defer");
    let id = add(s.path(), &["나중에 할 일"]);
    let before = line_of(s.path(), &id);

    ok(s.path(), &["defer", &id]);
    let after = line_of(s.path(), &id);
    assert!(after.contains(r#""deferred_at""#), "{after}");
    assert!(after.contains(r#""kind""#) == before.contains(r#""kind""#), "종류를 건드렸다");
    assert!(after.contains(r#""status":"todo""#), "칸을 옮겼다 — {after}");

    ok(s.path(), &["defer", &id, "--undo"]);
    assert!(!line_of(s.path(), &id).contains("deferred"), "도로 집었는데 자국이 남았다");
}

/// 여럿을 한 번에 미룬다. `mv` 가 그러듯 하나가 없다고 나머지를 안 미루지
/// 않는다 — 되풀이해 부르는 것이 흔하다.
#[test]
fn deferring_takes_many_and_reports_what_it_could_not() {
    let s = init("defermany");
    let a = add(s.path(), &["하나"]);
    let b = add(s.path(), &["둘"]);
    let out = moai(s.path(), &["defer", &a, "argos-0000", &b]);
    assert!(!out.status.success(), "못 찾은 것이 있는데 0 으로 끝났다");
    assert!(line_of(s.path(), &a).contains("deferred_at"), "나머지를 안 미뤘다");
    assert!(line_of(s.path(), &b).contains("deferred_at"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("argos-0000"));
}

/// 이미 미룬 것을 또 미뤄도 **시각이 안 밀린다.** 밀리면 "언제부터 미뤄
/// 뒀나" 가 마지막으로 명령을 친 때가 된다 — `mv` 가 같은 자리에서 같은
/// 규칙을 쓴다.
#[test]
fn deferring_twice_does_not_move_the_stamp() {
    let s = init("deferagain");
    let id = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &id]);
    let once = line_of(s.path(), &id);
    ok(s.path(), &["defer", &id]);
    assert_eq!(line_of(s.path(), &id), once, "두 번째 미루기가 시각을 밀었다");
}

/// **필드 변경은 저널에 안 적는다** (CLAUDE.md). 적어 온 말은 그래도
/// 버리지 않는다 — `mv` 와 같은 규칙이다.
#[test]
fn deferring_writes_the_reason_but_not_the_field_change() {
    let s = init("defernote");
    let id = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &id]);
    let quiet = ok(s.path(), &["show", &id]);
    assert_eq!(quiet.matches("note:").count(), 0, "필드 변경을 저널에 적었다 — {quiet}");

    ok(s.path(), &["defer", &id, "--undo"]);
    ok(s.path(), &["defer", &id, "-m", "다음 분기에 다시 본다"]);
    let told = ok(s.path(), &["show", &id]);
    assert!(told.contains("다음 분기에"), "적어 온 말을 버렸다 — {told}");
}

/// 미룬 것은 목록에서 done 과 같은 자리에서 빠지고, `--deferred` 로 본다.
/// **켜는 말과 좁히는 말이 하나다** — "미룬 것 보기" 가 한 낱말로 끝난다.
#[test]
fn deferred_rows_leave_the_list_and_come_back_by_name() {
    let s = init("deferlist");
    let put_off = add(s.path(), &["나중에"]);
    let now = add(s.path(), &["지금"]);
    ok(s.path(), &["defer", &put_off]);

    let plain = ok(s.path(), &["show"]);
    assert!(plain.contains(&now) && !plain.contains(&put_off), "{plain}");
    assert!(plain.contains("미룸") || plain.contains("1건"), "숨긴 것을 안 센다 — {plain}");

    let only = ok(s.path(), &["show", "--deferred"]);
    assert!(only.contains(&put_off) && !only.contains(&now), "{only}");
    assert!(ok(s.path(), &["show", "--all"]).contains(&put_off), "--all 이 안 보여준다");
}

/// 미룬 것은 `ready` 에도 보드에도 없고, `status` 가 한 줄로 그것을 비춘다.
/// **막지 않는다** — 알림이지 경고가 아니다.
#[test]
fn status_shows_what_was_put_off_without_blocking() {
    let s = init("deferstatus");
    let put_off = add(s.path(), &["나중에"]);
    let now = add(s.path(), &["지금"]);
    ok(s.path(), &["defer", &put_off]);

    let r = ok(s.path(), &["ready"]);
    assert!(r.contains(&now) && !r.contains(&put_off), "{r}");

    let out = moai(s.path(), &["status"]);
    assert!(out.status.success(), "알림으로 비영 종료했다 — 그러면 이건 게이트다");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("이슈 1"), "미룬 것을 보드가 셌다 — {text}");
    assert!(text.contains("미뤄 둔 것 1건"), "{text}");
    assert!(text.contains("+ 미뤄 둔 것"), "알림이 경고 글리프를 달았다 — {text}");
    assert!(text.contains("moai show --deferred"), "다음에 무엇을 칠지 안 말한다 — {text}");
}

/// **알림만 있는 것은 "아무 문제 없다" 이다.** 여기가 무너지면 생각을 담거나
/// 무언가를 미룬 순간부터 그 줄이 사라져, 세션을 닫기 전에 "경고가 늘지
/// 않았는지" 보는 사람이 알림을 경고로 읽는다.
#[test]
fn a_repo_with_only_notices_still_reads_clean() {
    let s = init("noticeclean");
    let id = add(s.path(), &["지금 할 일"]);
    ok(s.path(), &["idea", "add", "반짝", "-q"]);
    let later = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &later]);
    ok(s.path(), &["epic", "add", "묶음", "-q"]);
    ok(s.path(), &["edit", &id, "-e", &field(&ok(s.path(), &["epic", "show", "--json"]), "id")]);

    let out = ok(s.path(), &["status"]);
    assert!(out.contains("미뤄 둔 것"), "알림이 안 나온다 — {out}");
    assert!(out.contains("드러난 문제 없다"), "알림 하나로 깨끗함을 잃었다 — {out}");

    // **기계 출력도 같은 말을 한다.** 알림이 `warnings` 에 섞이면 그 배열의
    // 길이를 세는 쪽(에이전트·Stop 훅)이 담고 미룰 때마다 경고가 늘었다고
    // 읽는다(moai-c8lb).
    let json = ok(s.path(), &["status", "--json"]);
    assert!(json.contains(r#""warnings":[]"#), "알림이 고칠 것 자리에 섰다 — {json}");
    assert!(json.contains(r#""notices":["#) && json.contains(r#""kind":"deferred""#), "{json}");
}

/// 미뤄 둔 줄을 옮기면 **말한다.** 칸은 옮겨졌는데 보드에도 `ready` 에도 안
/// 나오므로, 말하지 않으면 집어 든 일이 통째로 안 보인다. 막지는 않는다.
#[test]
fn moving_a_deferred_row_says_it_is_still_out_of_the_plan() {
    let s = init("defermv");
    let id = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &id]);
    let out = moai(s.path(), &["mv", &id, "in_progress"]);
    assert!(out.status.success(), "막았다 — 이 도구에 게이트는 없다");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("in_progress"), "안 옮겼다 — {text}");
    // **도로 집는 말은 그대로 쳐서 되는 명령이다** — id 가 빠진 `moai defer --undo`
    // 는 clap 이 인자가 없다며 거절한다.
    assert!(text.contains(&format!("moai defer {id} --undo")), "보드에서 빠져 있다는 말을 안 한다 — {text}");
    assert!(!ok(s.path(), &["ready"]).contains(&id));

    // **기계 출력도 같은 것을 말한다** — `--json` 을 읽는 에이전트에게만 까닭이
    // 없으면, 방금 집은 일이 훅의 초점에서 빠진 것을 알 길이 없다.
    let json = ok(s.path(), &["mv", &id, "review", "--json"]);
    assert!(
        json.contains(&format!(r#""shelved":[{{"id":"{id}","root":"{id}"}}]"#)),
        "기계 출력만 안 말한다 — {json}"
    );
}

/// **묶음을 미루면 멤버도 계획에서 빠진다.** 에픽 줄 하나만 사라지고 멤버가
/// `ready` 에 그 에픽 제목을 달고 서면, 미루기는 머리글 하나 지운 일이다.
/// 목록도 같은 자로 숨기고, `--deferred` 가 그것을 연다.
#[test]
fn deferring_an_epic_takes_its_members_out_of_the_plan() {
    let s = init("deferepicmembers");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    let member = add(s.path(), &["파서", "-e", &epic]);
    let other = add(s.path(), &["지금 할 것"]);
    ok(s.path(), &["defer", &epic]);

    let r = ok(s.path(), &["ready"]);
    assert!(r.contains(&other) && !r.contains(&member), "미룬 에픽의 멤버가 올라왔다 — {r}");

    let plain = ok(s.path(), &["show"]);
    assert!(!plain.contains(&member), "목록만 멤버를 계획으로 낸다 — {plain}");
    assert!(plain.contains("미룸 2건 숨김"), "{plain}");
    let only = ok(s.path(), &["show", "--deferred"]);
    assert!(only.contains(&member), "세어 놓고 가리킨 곳이 비었다 — {only}");
    let all = ok(s.path(), &["show", "--all"]);
    let row = all.lines().find(|l| l.contains(&member)).expect("--all 이 멤버를 안 낸다");
    assert!(row.contains("미룸"), "물려받은 미룸이 표를 잃었다 — {row}");
    assert!(ok(s.path(), &["status"]).contains("미뤄 둔 것 2건"));

    // **펼친 상세도 까닭을 말한다.** 목록에서 빠진 줄을 id 로 콕 집어 펼친 자리가
    // "왜 ready 에 안 나오나" 에 답하는 곳인데, 제 `deferred_at` 만 보면 표가 없다.
    let detail = ok(s.path(), &["show", &member]);
    assert!(detail.contains(&format!("미룸 — {epic} 밑")), "상세가 물려받은 미룸을 안 말한다 — {detail}");
    let json = ok(s.path(), &["show", &member, "--json"]);
    assert!(json.contains(&format!(r#""shelved_by":"{epic}""#)), "기계 출력만 안 말한다 — {json}");
}

/// **물려받은 미룸 앞에서 `--undo` 는 "이미 계획에 있다" 고 하지 않는다.** 제 줄은
/// 미룬 적이 없어 그렇게 말했는데, 실제로는 미룬 에픽 때문에 여전히 빠져
/// 있었다(moai-kluk). 도로 집을 줄을 사람에게도 기계에도 댄다.
#[test]
fn undoing_an_inherited_deferral_names_the_row_that_holds_it() {
    let s = init("deferundoinherited");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    let member = add(s.path(), &["파서", "-e", &epic]);
    ok(s.path(), &["defer", &epic]);

    let out = ok(s.path(), &["defer", &member, "--undo"]);
    assert!(!out.contains("이미 계획에 있다"), "빠져 있는 줄을 계획에 있다고 한다 — {out}");
    assert!(out.contains(&format!("moai defer {epic} --undo")), "도로 집을 줄을 안 댄다 — {out}");

    let json = ok(s.path(), &["defer", &member, "--undo", "--json"]);
    assert!(json.contains(r#""already":[]"#), "{json}");
    assert!(json.contains(&format!(r#""shelved":[{{"id":"{member}","root":"{epic}"}}]"#)), "{json}");

    // 에픽을 도로 집으면 같은 명령이 조용하다 — 헛되이 세지 않는다.
    ok(s.path(), &["defer", &epic, "--undo"]);
    let calm = ok(s.path(), &["defer", &member, "--undo"]);
    assert!(calm.contains("이미 계획에 있다") && !calm.contains("--undo`"), "{calm}");
}

/// **도로 집는 말은 풀어야 할 미룸을 다 댄다**(moai-g2a1). 제 줄도 미뤘고 미룬 에픽에도
/// 든 줄에 가까운 하나만 대면, 그것을 풀고도 여전히 빠진 채 그제야 다음을 댄다.
#[test]
fn moving_a_row_shelved_twice_names_every_deferral_to_undo() {
    let s = init("defertwice");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    let member = add(s.path(), &["파서", "-e", &epic]);
    ok(s.path(), &["defer", &epic]);
    ok(s.path(), &["defer", &member]);

    let out = ok(s.path(), &["mv", &member, "in_progress"]);
    assert!(out.contains(&format!("moai defer {member} {epic} --undo")), "하나만 댄다 — {out}");
    let json = ok(s.path(), &["mv", &member, "review", "--json"]);
    assert!(json.contains(&format!(r#""root":"{member}","roots":["{member}","{epic}"]"#)), "{json}");

    // 하나뿐이면 기계 출력은 전과 같다.
    ok(s.path(), &["defer", &member, "--undo"]);
    let one = ok(s.path(), &["mv", &member, "todo", "--json"]);
    assert!(one.contains(&format!(r#""shelved":[{{"id":"{member}","root":"{epic}"}}]"#)), "{one}");
}

/// **미룬 것이 막고 있으면 `ready` 가 까닭 없이 비지 않는다.** 막는 줄은
/// 어느 목록에도 없으므로 그 id 와 푸는 길을 같이 댄다.
#[test]
fn ready_names_the_deferred_blocker_it_is_waiting_on() {
    let s = init("deferblocker");
    let blocker = add(s.path(), &["막는 것"]);
    let blocked = add(s.path(), &["막힌 것"]);
    ok(s.path(), &["link", &blocker, "--blocks", &blocked]);
    ok(s.path(), &["defer", &blocker]);

    let r = ok(s.path(), &["ready"]);
    assert!(r.contains("0건"), "미룬 막음을 끝난 것으로 봤다 — {r}");
    assert!(r.contains("미뤄 둔 것에 막혀"), "왜 비었는지 안 말한다 — {r}");
    assert!(r.contains(&blocked) && r.contains(&blocker), "누가 누구를 막는지 안 댄다 — {r}");

    // **기계 출력도 같은 것을 말한다**(moai-w6n2). 맨 배열이던 때는 `[]` 뿐이라, 에이전트는
    // 왜 비었는지 모른 채 할 일이 없다고 읽었다. 이제 객체로 감싸 held 를 싣는다.
    let json = ok(s.path(), &["ready", "--json"]);
    one_json_value(&json);
    assert_eq!(
        json.trim(),
        format!(r#"{{"ready":[],"held":[{{"id":"{blocked}","by":["{blocker}"],"undo":["{blocker}"]}}]}}"#),
        "{json}"
    );

    let st = moai(s.path(), &["status", "--json"]);
    assert!(st.status.success(), "경고로 비영 종료했다");
    assert!(String::from_utf8(st.stdout).unwrap().contains("blocked_by_deferred"));
}

// ── 리뷰가 잡은 것들 ────────────────────────────────────────────────
//
// 아래는 전부 **조용히 되돌아갈 자리**다. 어느 것도 컴파일러가 못 잡고,
// 하나같이 사람이 도구를 믿는 근거를 갉아먹는다.

/// **연습이라 적힌 명령이 쓰면 안 된다.** `--from` 없이 부른 `--dry-run` 은
/// 한 줄을 찍은 다음 그것을 실제로 만들었다 — 막는 줄 알고 부른 명령이 쓰는
/// 것보다 나쁜 것은 없다. `clap` 의 `requires` 로는 못 막는다: `--from` 이
/// 제목과 conflicts 라, 제목이 있으면 못 채울 요구로 보고 건너뛴다.
#[test]
fn a_rehearsal_without_a_plan_is_refused_not_written() {
    let s = init("dryrunalone");
    let out = moai(s.path(), &["add", "연습", "--dry-run"]);
    assert!(!out.status.success(), "연습이라 해 놓고 받아들였다");
    assert_eq!(issues(s.path()), "", "연습인데 썼다 — {}", issues(s.path()));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--from"),
        "어디로 가야 하는지 안 말한다 — {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **마크다운이 못 내는 종류는 전부 막는다.** `#` 은 에픽이고 `-` 는
/// 이슈라서 마일스톤은 나올 수가 없는데, 한때 `moai milestone add --from -`
/// 은 조용히 에픽과 이슈를 만들었다 — 시킨 것과 다른 것을 만드는 쪽이
/// 거절보다 나쁘다.
#[test]
fn a_plan_refuses_a_kind_the_markdown_cannot_make() {
    let s = init("stonefrom");
    let out = from_stdin(s.path(), &["milestone", "add", "--from", "-"], "# 에픽\n- 일\n");
    assert!(!out.status.success(), "마일스톤을 시켰는데 에픽을 만들었다");
    assert_eq!(issues(s.path()), "", "거절해 놓고 썼다");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("milestone add"),
        "어디로 가야 하는지 안 말한다 — {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// 못 읽는 줄 하나가 **성공한 쓰기를 실패로 보이게 하지 않는다.** `promote`
/// 만 락 밖에서 읽고 부분 실패 깃발을 세웠다 — 그것을 실패로 읽은 쪽이 다시
/// 부르면 같은 계획이 두 벌 생긴다.
#[test]
fn an_unreadable_line_does_not_fail_a_promote() {
    let s = init("promotebroken");
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let path = s.path().join(".moai/issues.jsonl");
    let mut text = issues(s.path());
    text.push_str("{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n");
    std::fs::write(&path, text).unwrap();

    let out = from_stdin(
        s.path(),
        &["idea", "promote", &id, "--from", "-"],
        "# 새 에픽\n- [p1] 첫 일\n",
    );
    assert!(
        out.status.success(),
        "성공한 promote 가 실패로 끝났다 — 다시 부르면 계획이 두 벌이다\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(issues(s.path()).contains("새 에픽"), "안 만들었다");
    assert!(issues(s.path()).contains("몰라"), "모르는 줄을 잃었다");
}

/// **일어나지 않은 쓰기를 주장하지 않는다.** `moai note` 는 스냅샷을 안
/// 건드리는데, 못 읽는 줄이 하나라도 있으면 "그대로 두고 썼다" 고 말했다.
#[test]
fn a_journal_only_write_claims_no_rewrite() {
    let s = init("carriednote");
    let id = add(s.path(), &["제목"]);
    let path = s.path().join(".moai/issues.jsonl");
    let mut text = issues(s.path());
    text.push_str("{깨짐\n");
    std::fs::write(&path, &text).unwrap();

    let out = moai(s.path(), &["note", &id, "메모"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(issues(s.path()), text, "스냅샷을 건드렸다");
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("그대로 두고 썼다"),
        "안 쓴 쓰기를 주장했다 — {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **못 읽는 줄이 산 줄의 id 를 들고 있으면 `status` 가 중복이라 말한다.**
/// 줄 번호만 보던 때는 `읽을 수 없는 줄` 만 서고 중복은 안 서서, 그 줄이
/// 읽히게 되는 날에야 모든 쓰기가 막혔다(moai-4dk4). 쓰기는 여전히 안 막는다.
#[test]
fn status_names_an_unreadable_line_that_reuses_a_live_id() {
    let s = init("unreadabledup");
    let id = add(s.path(), &["산 줄"]);
    let path = s.path().join(".moai/issues.jsonl");
    let mut text = issues(s.path());
    text.push_str(&format!("{{\"id\":\"{id}\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}}\n"));
    std::fs::write(&path, &text).unwrap();

    let out = moai(s.path(), &["status", "--json"]);
    assert!(!out.status.success(), "깨진 데이터인데 0 으로 끝났다");
    let json = String::from_utf8(out.stdout).unwrap();
    assert!(json.contains(r#""kind":"duplicate_id""#), "중복을 못 봤다 — {json}");
    assert!(json.contains(&format!(r#""ids":["{id}"]"#)), "{json}");

    // 드러내기만 한다 — 다른 줄을 쓰는 것은 막지 않는다.
    let other = moai(s.path(), &["add", "새 줄"]);
    assert!(other.status.success(), "{}", String::from_utf8_lossy(&other.stderr));
}

/// **같은 id 의 줄이 둘이면 `show <id>` 도 뒷줄을 연다** — 트리·탐색기(`nav::Index::find`)와
/// id 지도(`report::groups`·`milestones`)가 모두 뒷줄을 고르는데 상세만 앞줄을 열면, 머리
/// 제목·필드는 앞줄 것이고 멤버 셈은 뒷줄 것인 한 화면이 선다(moai-e0ro). 종류가 다른
/// 쌍둥이면 앞줄은 가려진 줄이라 마일스톤 상세를 CLI 로 아예 못 열었다(moai-2m9p).
/// 중복은 숨기지 않는다 — 상세가 한 줄로 말하고, `--json` 도 같은 수를 낸다.
#[test]
fn show_opens_the_later_line_of_a_duplicate_id_and_says_so() {
    let s = init("dupline");
    std::fs::write(
        s.path().join(".moai/issues.jsonl"),
        concat!(
            "{\"id\":\"argos-0001\",\"title\":\"앞 줄 생각\",\"kind\":\"idea\",\"status\":\"todo\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n",
            "{\"id\":\"argos-0001\",\"title\":\"뒷줄 마일스톤\",\"kind\":\"milestone\",\"status\":\"todo\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n",
            "{\"id\":\"argos-0002\",\"title\":\"딸린 에픽\",\"kind\":\"epic\",\"status\":\"todo\",\"milestone\":\"argos-0001\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}\n",
        ),
    )
    .unwrap();

    let out = ok(s.path(), &["show", "argos-0001"]);
    assert!(out.contains("뒷줄 마일스톤") && !out.contains("앞 줄 생각"), "앞줄을 열었다 — {out}");
    assert!(out.contains("argos-0002"), "뒷줄의 멤버가 안 나왔다 — {out}");
    assert!(out.contains("이 id 의 줄이 2개"), "중복을 말하지 않았다 — {out}");

    let json = ok(s.path(), &["show", "argos-0001", "--json"]);
    assert_eq!(field(&json, "title"), "뒷줄 마일스톤", "{json}");
    assert!(json.contains(r#""members":["argos-0002"]"#), "{json}");
    assert!(json.contains(r#""duplicate_lines":2"#), "{json}");

    // 중복이 아닌 줄에는 말을 붙이지 않는다.
    let lone = ok(s.path(), &["show", "argos-0002"]);
    assert!(!lone.contains("이 id 의 줄이"), "{lone}");
    assert!(!ok(s.path(), &["show", "argos-0002", "--json"]).contains("duplicate_lines"));
}

/// **안 썼어도 파일이 상했다는 것은 말한다.** 저널만 쓰는 명령과 할 일이 없던
/// 쓰기는 `report_load_errors` 도 안 지나, 그 동사만 쓰는 쪽은 상한 줄을 영영
/// 몰랐다(moai-relb). 종료 코드는 그대로다 — 쓰기는 성공했다.
#[test]
fn a_write_that_touched_nothing_still_names_the_unreadable_line() {
    let s = init("heldnote");
    let id = add(s.path(), &["제목"]);
    let path = s.path().join(".moai/issues.jsonl");
    let mut text = issues(s.path());
    text.push_str("{깨짐\n");
    std::fs::write(&path, &text).unwrap();

    for args in [
        vec!["note", id.as_str(), "메모"],
        vec!["defer", id.as_str(), "--undo"],
    ] {
        let out = moai(s.path(), &args);
        assert!(out.status.success(), "{args:?} — {}", String::from_utf8_lossy(&out.stderr));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("읽을 수 없는 줄 1개"), "{args:?} 가 상한 줄을 말 안 했다 — {err}");
        assert!(err.contains("moai show"), "{args:?} 가 어디서 보는지 안 댄다 — {err}");
        assert!(!err.contains("그대로 두고 썼다"), "{args:?} 가 안 쓴 쓰기를 주장했다 — {err}");
    }
    assert_eq!(issues(s.path()), text, "스냅샷을 건드렸다");
}

/// **세어 놓고 못 보여 주는 수를 만들지 않는다.** `미뤄 둔 것 N건` 은 종류를
/// 안 가리고 세므로(미뤄 둔 에픽까지 비추려고), 그 줄이 가리키는 명령이
/// 생각을 숨기면 가리킨 곳이 비어 있다.
#[test]
fn the_deferred_line_points_at_a_command_that_shows_them() {
    let s = init("deferidea");
    let id = ok(s.path(), &["idea", "add", "미룰 생각", "-q"]).trim().to_string();
    ok(s.path(), &["defer", &id]);
    assert!(ok(s.path(), &["status"]).contains("미뤄 둔 것 1건"), "안 센다");
    let out = ok(s.path(), &["show", "--deferred"]);
    assert!(out.contains(&id), "세어 놓고 가리킨 곳이 비었다 — {out}");
}

/// **닫은 줄에는 안 붙인다.** 끝난 일은 원래 보드에도 `ready` 에도 안
/// 나오므로 미뤘다는 것이 더는 안 보이는 까닭이 아니고, `--undo` 는 끝난
/// 일을 계획에 도로 넣으라는 엉뚱한 말이 된다.
#[test]
fn closing_a_deferred_row_drops_the_advisory() {
    let s = init("defermvdone");
    let id = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &id]);
    let text = ok(s.path(), &["mv", &id, "done"]);
    assert!(text.contains("done"), "안 옮겼다 — {text}");
    assert!(!text.contains("--undo"), "끝난 일에게 도로 집으라 한다 — {text}");
}

/// **미뤄 둔 묶음은 꾸짖지 않는다.** "다음 분기에" 하고 통째로 미룬 에픽이
/// 그 순간 `속이 빈 에픽` 으로 서면, 미룰수록 잔소리가 늘어 안 미루고 그냥
/// 쌓아 둔다.
#[test]
fn a_deferred_grouping_is_not_also_scolded() {
    let s = init("deferepicwarn");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    ok(s.path(), &["defer", &epic]);
    let out = ok(s.path(), &["status"]);
    assert!(!out.contains("속이 빈 에픽"), "미룬 에픽을 고발했다 — {out}");
    assert!(out.contains("미뤄 둔 것 1건"), "제 이름으로도 안 말한다 — {out}");
    assert!(out.contains("드러난 문제 없다"), "알림 하나로 깨끗함을 잃었다 — {out}");
}

/// **트리도 안 낸 것을 말한다.** 롤업 머리글은 `is_work` 로 세므로 미뤄 둔
/// 멤버까지 세는데 그 줄은 트리에서 빠진다 — 말하지 않으면 `0/2` 밑에 줄
/// 하나만 서고 왜 하나가 없는지 아무도 모른다.
#[test]
fn the_tree_says_what_it_did_not_draw() {
    let s = init("treehidden");
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "-q"]).trim().to_string();
    add(s.path(), &["멤버", "-e", &epic]);
    let put_off = add(s.path(), &["미룬 멤버", "-e", &epic]);
    ok(s.path(), &["defer", &put_off]);

    let out = ok(s.path(), &["show", "--tree"]);
    assert!(out.contains("0/2"), "머리글이 계획 전부를 안 센다 — {out}");
    assert!(!out.contains(&put_off), "미룬 줄을 그렸다 — {out}");
    assert!(out.contains("미룸 1건 숨김"), "왜 하나가 없는지 안 말한다 — {out}");
}

/// **사람 화면과 기계 출력이 같은 멤버를 낸다** — 마일스톤도, 물려받은 자식도.
/// 한때 `--json` 은 에픽에만, 제 `epic` 을 적은 줄만 내 에이전트와 사람이 같은
/// 묶음을 다르게 셌다(moai-qizs).
#[test]
fn a_grouping_lists_the_same_members_on_both_surfaces() {
    let s = init("groupjson");
    let stone = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "--milestone", &stone, "-q"])
        .trim()
        .to_string();
    let work = add(s.path(), &["파서", "-e", &epic]);
    let child = add(s.path(), &["파서 자식", "--parent", &work]);

    let stone_json = ok(s.path(), &["show", &stone, "--json"]);
    assert!(stone_json.contains(r#""members""#), "마일스톤이 멤버 키를 안 낸다 — {stone_json}");
    // 따옴표까지 찾는다 — 자식 id 는 부모 id 로 시작해, 맨 id 로 찾으면 부모가
    // 빠져도 자식이 대신 걸린다.
    for id in [&epic, &work, &child] {
        assert!(stone_json.contains(&format!("\"{id}\"")), "{id} 가 빠졌다 — {stone_json}");
    }
    let epic_json = ok(s.path(), &["show", &epic, "--json"]);
    assert!(epic_json.contains(&format!("\"{child}\"")), "물려받은 자식을 기계 출력만 뺐다 — {epic_json}");
    assert!(ok(s.path(), &["show", &epic]).contains(&child), "사람 화면이 자식을 안 그린다");
}

/// **꼬리는 그린 줄을 숨겼다고 말하지 않는다.** 거름망은 잎에 걸리는데 트리는
/// 걸린 자손의 조상도 그리므로, 평평하게 센 수를 그대로 대면 방금 그린 생각을
/// `idea 1건 숨김` 이라 부른다(moai-wi67).
#[test]
fn the_tree_tail_does_not_count_what_it_drew() {
    let s = init("treetailancestor");
    let thought = ok(s.path(), &["idea", "add", "반짝", "-q"]).trim().to_string();
    let child = add(s.path(), &["자식", "--parent", &thought]);

    let out = ok(s.path(), &["show", "--tree"]);
    assert!(out.contains(&thought) && out.contains(&child), "{out}");
    assert!(!out.contains("idea 1건 숨김"), "그린 줄을 숨겼다고 말한다 — {out}");

    // 목록은 조상을 안 그리므로 여전히 센다.
    assert!(ok(s.path(), &["show"]).contains("idea 1건 숨김"), "목록이 숨긴 것을 안 센다");
}

/// **트리가 조상으로 그린 줄도 계획 밖이면 그렇다고 단다.** 에픽의 미룸을 받은
/// 생각은 제 자식에게 미룸을 안 넘기므로, 자식이 걸리면 그 생각이 조상으로 그려진다 —
/// 표가 없으면 계획 밖의 줄이 일과 똑같이 보이고, 꼬리는 그 줄을 숨긴 수에서 뺀다.
#[test]
fn the_tree_marks_a_deferred_row_it_draws_as_an_ancestor() {
    let s = init("treedeferredancestor");
    let epic = ok(s.path(), &["epic", "add", "나중", "-q"]).trim().to_string();
    let thought = ok(s.path(), &["idea", "add", "생각", "-e", &epic, "-q"]).trim().to_string();
    let child = add(s.path(), &["자식", "--parent", &thought]);
    ok(s.path(), &["defer", &epic]);

    let tree = ok(s.path(), &["show", "--tree"]);
    let drawn = tree
        .lines()
        .find(|l| l.contains(&thought) && !l.contains(&child))
        .unwrap_or_else(|| panic!("생각이 조상으로 안 그려졌다 — {tree}"));
    assert!(drawn.contains("미룸"), "계획 밖의 조상이 일과 똑같이 보인다 — {tree}");
    let kid = tree.lines().find(|l| l.contains(&child)).unwrap_or_else(|| panic!("{tree}"));
    assert!(!kid.contains("미룸"), "계획 안의 자식에 미룸을 달았다 — {tree}");
}

/// 담아 둔 생각은 **기계 출력에서도** 멤버가 아니다. 화면도(`nav` 가 에픽
/// 밑에 안 걸어서) 머리글도(`rollup` 이 `is_work` 로 세서) 세지 않는 줄을
/// `--json` 만 세면, 받는 쪽이 계획에 없는 것을 계획으로 읽는다.
#[test]
fn a_thought_is_not_a_member_on_either_surface() {
    let s = init("epicjsonidea");
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "-q"]).trim().to_string();
    let member = add(s.path(), &["진짜 일", "-e", &epic]);
    let thought = ok(s.path(), &["idea", "add", "샤딩", "-e", &epic, "-q"]).trim().to_string();

    let json = ok(s.path(), &["show", &epic, "--json"]);
    assert!(json.contains(&member), "멤버를 잃었다 — {json}");
    assert!(!json.contains(&thought), "생각을 멤버로 셌다 — {json}");
    assert!(ok(s.path(), &["show", &epic]).contains("멤버   0/1"), "머리글이 달라졌다");
}

/// **에픽만 떼면 반만 뗀 것이다.** 마일스톤은 에픽을 타고 물려받으므로,
/// 에픽에서만 빼면 그 줄이 마일스톤 바구니로 떨어져 `moai show <마일스톤>` 이
/// 똑같이 머리글과 어긋난다 — 세는 자와 그리는 자가 갈라진 자리(moai-lhbh)다.
#[test]
fn a_thought_does_not_hang_under_a_milestone_either() {
    let s = init("ideastone");
    let stone = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "--milestone", &stone, "-q"])
        .trim()
        .to_string();
    add(s.path(), &["진짜 일", "-e", &epic]);
    let thought = ok(s.path(), &["idea", "add", "샤딩", "-e", &epic, "-q"]).trim().to_string();

    let out = ok(s.path(), &["show", &stone]);
    assert!(out.contains("멤버   0/1"), "{out}");
    assert!(!out.contains(&thought), "머리글이 안 세는 줄을 그 밑에 그렸다 — {out}");
}

/// **부모가 묶음이면 그 묶음이 소속이다** (moai-9t3l). `--parent <에픽>` 으로 만든
/// 자식은 id 가 에픽 밑에 붙는데, "자식은 부모의 에픽을" 이 부모가 에픽 자신일 때
/// 물려줄 것이 없어 트리의 `에픽 없음`·status 의 `에픽 없는 이슈` 로 빠졌다 — 에픽
/// 전체를 보는 리뷰를 세울 때마다 났다. 적힌 필드는 그대로고 읽을 때 정한다.
///
/// 제 `epic` 을 따로 적은 자식은 그것이 이긴다(물려받은 소속과 같은 차례). 부모가
/// 마일스톤이면 같은 규칙으로 그 마일스톤에 든다.
#[test]
fn a_child_of_a_group_belongs_to_that_group() {
    let s = init("groupparent");
    let stone = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "--milestone", &stone, "-q"]).trim().to_string();
    let other = ok(s.path(), &["epic", "add", "딴 에픽", "-q"]).trim().to_string();
    let member = add(s.path(), &["멤버", "-e", &epic]);
    let review = add(s.path(), &["리뷰 — 에픽 전체", "-t", "review", "--parent", &epic]);
    let grand = add(s.path(), &["리뷰의 자식", "--parent", &review]);
    let moved = add(s.path(), &["딴 데 적은 자식", "--parent", &epic, "-e", &other]);
    let under_stone = add(s.path(), &["마일스톤의 자식", "--parent", &stone]);
    assert!(review.starts_with(&format!("{epic}.")), "에픽 밑에 id 로 안 섰다 — {review}");
    assert!(!line_of(s.path(), &review).contains(r#""epic""#), "필드를 적었다 — 읽을 때 정해야 한다");

    let tree = ok(s.path(), &["show", "--tree"]);
    let at = |id: &str| tree.find(&format!("{id}  ")).unwrap_or_else(|| panic!("트리에 {id} 가 없다 — {tree}"));
    assert!(at(&epic) < at(&review) && at(&review) < at(&grand) && at(&grand) < at(&other), "{tree}");
    assert!(!tree[at(&epic)..at(&grand)].contains("에픽 없음"), "에픽의 자식이 `에픽 없음` 으로 빠졌다 — {tree}");
    assert!(at(&other) < at(&moved), "제 에픽을 적은 자식이 부모 에픽 밑에 그려졌다 — {tree}");

    let st = ok(s.path(), &["status", "--json"]);
    // status 의 `--json` 은 경고에서만 id 를 댄다. 마일스톤의 자식은 에픽이 없어 거기 선다.
    for id in [&review, &grand] {
        assert!(!st.contains(&format!("\"{id}\"")), "에픽의 자식 {id} 를 경고에 댔다 — {st}");
    }
    let loose = ok(s.path(), &["show", "-e", "none"]);
    for id in [&review, &grand, &moved] {
        assert!(!loose.contains(id.as_str()), "`-e none` 이 {id} 를 낸다 — {loose}");
    }

    let detail = ok(s.path(), &["show", &epic]);
    assert!(detail.contains("멤버   0/3"), "{detail}");
    // `children` 은 id 로 매인 자식이라 제 에픽을 따로 적은 줄도 든다. 멤버만 본다.
    let members = |group: &str| {
        let json = ok(s.path(), &["show", group, "--json"]);
        let from = json.find(r#""members":["#).unwrap_or_else(|| panic!("멤버 키가 없다 — {json}"));
        let rest = &json[from..];
        rest[..rest.find(']').unwrap()].to_string()
    };
    let json = members(&epic);
    let filtered = ok(s.path(), &["show", "-e", &epic]);
    let inside = ok(s.path(), &["tui", "--json", "--path", &epic]);
    for id in [&member, &review] {
        assert!(detail.contains(id.as_str()), "상세가 {id} 를 안 그린다 — {detail}");
        assert!(json.contains(&format!("\"{id}\"")), "--json 이 {id} 를 안 낸다 — {json}");
        assert!(filtered.contains(id.as_str()), "`-e` 가 {id} 를 안 고른다 — {filtered}");
        assert!(inside.contains(&format!("\"{id}\"")), "탐색기가 에픽 안에 {id} 를 안 둔다 — {inside}");
    }
    assert!(json.contains(&format!("\"{grand}\"")) && filtered.contains(&grand), "손자가 사슬을 못 탔다");
    assert!(!json.contains(&format!("\"{moved}\"")) && !filtered.contains(&moved), "제 에픽이 안 이겼다");
    assert!(ok(s.path(), &["show", "-e", &other]).contains(&moved));

    // 마일스톤은 에픽을 거쳐 온다 — 에픽의 자식도 그 에픽이 선 마일스톤에 든다.
    let home = ok(s.path(), &["show", "--milestone", &stone]);
    assert!(home.contains(&review) && home.contains(&grand), "{home}");
    // 부모가 마일스톤이면 그 마일스톤이다.
    let stone_json = members(&stone);
    assert!(home.contains(&under_stone), "마일스톤의 자식이 필터에서 빠졌다 — {home}");
    assert!(stone_json.contains(&format!("\"{under_stone}\"")), "{stone_json}");
    let bare = ok(s.path(), &["show", "--milestone", "none"]);
    assert!(!bare.contains(&under_stone), "마일스톤의 자식이 `마일스톤 없음` 이다 — {bare}");
}

/// **부모 밑에 접힌 자식은 부모의 마일스톤으로 센다.** 자식이 제 마일스톤을
/// 따로 적어도 트리는 그 줄을 부모 밑에 그리므로, 제 것으로 세면 `show <그
/// 마일스톤>` 이 `멤버 0/1` 이라 말하면서 줄을 못 내고 `--json` 만 그 자식을
/// 낸다 — 에픽이 이기는 자리(moai-lhbh)와 같은 어긋남이 부모에서 났다(moai-uqoe).
#[test]
fn a_folded_child_counts_toward_its_parents_milestone() {
    let s = init("foldstone");
    let m1 = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let m2 = ok(s.path(), &["milestone", "add", "v0.2", "-q"]).trim().to_string();
    let parent = add(s.path(), &["부모", "--milestone", &m1]);
    let child = add(s.path(), &["자식", "--parent", &parent, "--milestone", &m2]);

    let tree = ok(s.path(), &["show", "--tree"]);
    // 자식 바로 위에 선 마일스톤이 v0.1 이어야 한다. 마일스톤의 차례는 id 가 정하므로
    // 어느 쪽이 먼저 나올지는 모른다.
    let at = |id: &str| tree.find(id).unwrap_or_else(|| panic!("트리에 {id} 가 없다 — {tree}"));
    let (a1, a2, ac) = (at(&m1), at(&m2), at(&child));
    assert!(a1 < ac && !(a1 < a2 && a2 < ac), "트리가 그 자식을 v0.1 밑에 안 그린다 — {tree}");

    let elsewhere = ok(s.path(), &["show", &m2]);
    assert!(elsewhere.contains("멤버   0/0"), "그리지 않는 줄을 셌다 — {elsewhere}");
    let json = ok(s.path(), &["show", &m2, "--json"]);
    assert!(!json.contains(&child), "--json 만 그 자식을 낸다 — {json}");
    assert!(!ok(s.path(), &["show", "--milestone", &m2]).contains(&child), "필터가 트리와 갈린다");

    let home = ok(s.path(), &["show", &m1]);
    assert!(home.contains("멤버   0/2") && home.contains(&child), "{home}");
    assert!(ok(s.path(), &["show", &m1, "--json"]).contains(&child));
    assert!(ok(s.path(), &["show", "--milestone", &m1]).contains(&child));
}

/// **에픽 줄은 제 마일스톤에 서고, 그 멤버는 에픽 줄이 선 마일스톤으로 센다.**
/// 트리는 에픽을 제 마일스톤 밑에만 두는데, 세는 쪽이 에픽 줄의 `epic` 이나
/// id 부모를 타고 올라가 에픽 줄을 딴 곳에 세우면서 멤버는 에픽의 `milestone`
/// 필드를 다시 읽어, `show <마일스톤>` 이 `멤버 0/1` 이라 말하면서 줄을 못 냈다
/// (moai-0prl). 에픽 줄이 딴 데를 가리키는 세 길이 전부 CLI 로 만들어진다 —
/// 없는 에픽, 다른 에픽, 이슈 밑의 id.
#[test]
fn an_epic_line_stands_in_its_own_milestone_with_its_members() {
    let s = init("epicstone");
    let m1 = ok(s.path(), &["milestone", "add", "v0.1", "-q"]).trim().to_string();
    let m2 = ok(s.path(), &["milestone", "add", "v0.2", "-q"]).trim().to_string();
    let epic = |args: &[&str]| {
        let mut v = vec!["epic", "add"];
        v.extend_from_slice(args);
        v.push("-q");
        ok(s.path(), &v).trim().to_string()
    };
    // 없는 에픽을 가리키는 에픽 줄. `edit` 은 경고하고 적는다.
    let stray = epic(&["끊긴", "--milestone", &m1]);
    ok(s.path(), &["edit", &stray, "--epic", "argos-zzzz"]);
    // 다른 마일스톤에 선 에픽을 가리키는 에픽 줄.
    let outer = epic(&["바깥", "--milestone", &m2]);
    let inner = epic(&["안쪽", "--milestone", &m1]);
    ok(s.path(), &["edit", &inner, "--epic", &outer]);
    // 다른 마일스톤에 선 이슈 밑에 id 로 선 에픽 줄.
    let host = add(s.path(), &["집", "--milestone", &m2]);
    let nested = epic(&["이슈 밑", "--parent", &host, "--milestone", &m1]);
    assert!(nested.starts_with(&format!("{host}.")), "이슈 밑에 id 로 안 섰다 — {nested}");

    let work: Vec<String> =
        [&stray, &inner, &nested].iter().map(|e| add(s.path(), &["일", "-e", e])).collect();

    let tree = ok(s.path(), &["show", "--tree"]);
    let at = |needle: &str| tree.find(needle);
    let sections = [(at(&format!("{m1}  ")), "v0.1"), (at(&format!("{m2}  ")), "v0.2"), (at("(마일스톤 없음)"), "없음")];
    for id in work.iter().chain([&stray, &inner, &nested]) {
        let pos = at(id).unwrap_or_else(|| panic!("트리에 {id} 가 없다 — {tree}"));
        let under = sections.iter().filter(|(p, _)| p.is_some_and(|p| p < pos)).max_by_key(|(p, _)| *p);
        assert_eq!(under.map(|(_, n)| *n), Some("v0.1"), "트리가 {id} 를 v0.1 밑에 안 그린다 — {tree}");
    }

    let home = ok(s.path(), &["show", &m1]);
    assert!(home.contains("멤버   0/3"), "{home}");
    let json = ok(s.path(), &["show", &m1, "--json"]);
    let filtered = ok(s.path(), &["show", "--milestone", &m1]);
    for id in &work {
        assert!(home.contains(id.as_str()), "머리글이 센 줄을 목록이 못 낸다 — {home}");
        assert!(json.contains(id.as_str()), "--json 이 {id} 를 안 낸다 — {json}");
        assert!(filtered.contains(id.as_str()), "필터가 트리와 갈린다 — {filtered}");
    }

    let elsewhere = ok(s.path(), &["show", &m2]);
    assert!(elsewhere.contains("멤버   0/1"), "그리지 않는 줄을 셌다 — {elsewhere}");
    let json = ok(s.path(), &["show", &m2, "--json"]);
    for id in work.iter().chain([&inner, &nested]) {
        assert!(!json.contains(id.as_str()), "--json 이 딴 마일스톤에 {id} 를 낸다 — {json}");
    }

    // 에픽 줄이 든 끊긴 `epic` 은 자리를 안 바꿔도 드러난다.
    let st = ok(s.path(), &["status"]);
    assert!(st.contains("에픽으로 쓸 수 없는 것을 가리키는 줄") && st.contains(&stray), "{st}");
}


// ── 리뷰가 잡은 것 ───────────────────────────────────────────────────

/// **미뤄 둔 묶음은 표가 낱말로 말한다.** 한때 다 끝난 묶음을 노란 글씨로 재촉하던
/// 표가 미룬 묶음까지 재촉했다. 그 재촉은 걷었지만(moai-j3b3, 묶음의 칸은 멤버에서
/// 읽는다) 무엇이 미뤄졌는지는 여전히 색이 아니라 낱말이 진다.
#[test]
fn a_deferred_epic_is_not_nagged_in_the_table_either() {
    let s = init("defertable");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    add(s.path(), &["일", "-e", &epic]); // 안 끝난 채로 남는다
    ok(s.path(), &["defer", &epic]);
    let out = ok(s.path(), &["status"]);
    assert!(!out.contains("닫을 때가 됐다"), "미룬 묶음을 표가 재촉한다 — {out}");
    assert!(out.contains("미룸"), "무엇이 미뤄졌는지 낱말로 안 말한다 — {out}");
}

/// **끝난 묶음은 미뤄 둔 것으로 안 센다.** 세면 `moai status` 가 센 줄을 그 줄이
/// 가리키는 `moai show --deferred` 가 done 으로 숨겨, 세어 놓고 못 보여 주는 수가
/// 된다 — `idea_pile` 이 피한 그 덫이다. 남은 일이 생기면 다시 센다.
#[test]
fn a_finished_grouping_is_not_counted_as_shelved() {
    let s = init("shelvedone");
    let epic = ok(s.path(), &["epic", "add", "다음 분기", "-q"]).trim().to_string();
    let one = add(s.path(), &["일", "-e", &epic]);
    ok(s.path(), &["mv", &one, "done"]);
    ok(s.path(), &["defer", &epic, "-m", "다음 분기에"]);

    let st = ok(s.path(), &["status"]);
    assert!(!st.contains("미뤄 둔 것"), "끝난 묶음을 미뤄 둔 것으로 셌다 — {st}");
    let list = ok(s.path(), &["show", "--deferred"]);
    assert!(!list.contains(&epic), "센 것과 내는 것이 어긋난다 — {list}");
    // 끝난 묶음을 옮길 때 도로 집으라고 하지 않는다 — 그 줄은 계획 밖이 아니다.
    let moved = ok(s.path(), &["mv", &epic, "done"]);
    assert!(!moved.contains("--undo"), "끝난 묶음에 도로 집으라고 한다 — {moved}");

    // 남은 일이 생기면 그 묶음은 다시 계획 밖이다 — 세는 곳과 내는 곳이 같이 움직인다.
    add(s.path(), &["남은 일", "-e", &epic]);
    let st = ok(s.path(), &["status"]);
    assert!(st.contains("미뤄 둔 것 2건"), "{st}");
    assert!(ok(s.path(), &["show", "--deferred"]).contains(&epic), "센 것을 안 낸다");
}

/// **읽은 칸 키는 우리 것이다.** `--json` 을 파일에 되써 넣어 `derived_status` 를
/// 모르는 필드로 든 줄이 생기면, 그대로 내보낼 때 한 객체에 같은 키가 둘 서서 깐깐한
/// 파서가 거절하고 묶음 아닌 줄에는 읽은 칸이 있는 것처럼 보인다. 파일의 값은
/// 그대로 둔다 — `moai status` 가 "모르는 필드" 로 비춘다.
#[test]
fn a_stored_derived_status_never_doubles_in_the_output() {
    let s = init("derivedtwice");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let one = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    ok(s.path(), &["mv", &one, "in_progress"]);
    // 손으로 푼 머지가 남긴 모양 — 두 줄 다 그 키를 들고 있다.
    let doctored: String = issues(s.path())
        .lines()
        .map(|l| format!("{}{}\n", &l[..l.len() - 1], r#","derived_status":"done"}"#))
        .collect();
    std::fs::write(s.path().join(".moai/issues.jsonl"), doctored).unwrap();

    let json = ok(s.path(), &["show", &epic, "--json"]);
    assert_eq!(json.matches(r#""derived_status""#).count(), 1, "같은 키가 두 번 났다 — {json}");
    assert!(json.contains(r#""derived_status":"in_progress""#), "{json}");
    let work = ok(s.path(), &["show", &one, "--json"]);
    assert!(!work.contains("derived_status"), "묶음 아닌 줄이 읽은 칸을 냈다 — {work}");
    // 파일은 그대로다 — 모르는 필드는 잃지 않는다.
    assert!(line_of(s.path(), &one).contains(r#""derived_status":"done""#));
    assert!(ok(s.path(), &["status"]).contains("모르는 필드"));
}

/// **접은 묶음은 표와 기계 출력이 그렇다고 말한다.** 남은 멤버를 미뤄 닫은 묶음은
/// 막대가 `1/2` 인 채로 done 에 서는데, 세션이 시작하는 화면이 그것을 안 말하면
/// 접은 것과 굴러가는 것이 똑같아 보인다 — `moai show` 는 이미 하나를 숨긴다.
#[test]
fn status_says_which_groupings_stand_closed() {
    let s = init("statusfold");
    let epic = add(s.path(), &["저장 계층", "--type", "epic"]);
    let one = add(s.path(), &["원자적 쓰기", "-e", &epic]);
    let two = add(s.path(), &["락", "-e", &epic]);
    ok(s.path(), &["mv", &one, "done"]);

    let out = ok(s.path(), &["status"]);
    assert!(!out.contains("닫힘"), "굴러가는 묶음을 닫혔다고 한다 — {out}");
    let json = ok(s.path(), &["status", "--json"]);
    assert!(json.contains(r#""derived_status":"in_progress""#), "{json}");

    ok(s.path(), &["defer", &two, "-m", "다음에"]);
    let out = ok(s.path(), &["status"]);
    assert!(out.contains("1/2") && out.contains("닫힘"), "접은 묶음을 안 비춘다 — {out}");
    let json = ok(s.path(), &["status", "--json"]);
    assert!(json.contains(r#""derived_status":"done""#), "{json}");
}

/// **꼬리가 대는 낱말이 실제로 그 줄을 내야 한다.** 첫 까닭으로 갈랐을 때
/// 닫아 둔 생각이 `idea N건 숨김 — --type idea` 로 섰는데 그 명령은 done 을
/// 여전히 숨겨 아무것도 안 냈다 — `idea_pile` 이 피한 "세어 놓고 못 보여 주는
/// 수" 가 목록 꼬리에 그대로 있었다.
#[test]
fn every_hidden_count_names_a_flag_that_opens_it() {
    let s = init("hiddenhonest");
    let shelved = add(s.path(), &["닫고 미룬 일"]);
    ok(s.path(), &["mv", &shelved, "done"]);
    ok(s.path(), &["defer", &shelved]);
    let closed = ok(s.path(), &["idea", "add", "닫은 생각", "-q"]).trim().to_string();
    ok(s.path(), &["mv", &closed, "done"]);
    let alive = ok(s.path(), &["idea", "add", "산 생각", "-q"]).trim().to_string();

    let out = ok(s.path(), &["show"]);
    // 닫고 미룬 줄은 `--all` 이 연다 — `--deferred` 는 done 을 그대로 숨긴다.
    assert!(out.contains("done 1건 숨김 — `--all`"), "{out}");
    assert!(ok(s.path(), &["show", "--all"]).contains(&shelved), "댄 낱말이 그 줄을 안 낸다");
    // 산 생각 하나만 `--type idea` 가 연다. 닫은 생각은 어느 한 낱말로도
    // 안 열리므로 아예 안 센다 — 못 보여 줄 수를 대느니 말을 안 한다.
    assert!(out.contains("idea 1건 숨김 — `--type idea`"), "{out}");
    let ideas = ok(s.path(), &["show", "--type", "idea"]);
    assert!(ideas.contains(&alive), "{ideas}");
    assert!(!ideas.contains(&closed), "{ideas}");
}

/// **사람 화면과 기계 출력이 같은 것을 멤버라 부른다.** 머리글(`rollup`)도
/// `--json` 도 담아 둔 생각을 안 세는데 사람 화면만 그리면, `멤버 0/1` 밑에
/// 줄 둘이 서서 어느 숫자를 믿어야 할지 알 수 없다.
#[test]
fn the_detail_draws_exactly_what_it_counts_as_a_member() {
    let s = init("memberdraw");
    let epic = ok(s.path(), &["epic", "add", "저장 계층", "-q"]).trim().to_string();
    let work = add(s.path(), &["파서", "-e", &epic]);
    let thought =
        ok(s.path(), &["idea", "add", "이렇게 하면", "--parent", &work, "-q"]).trim().to_string();

    let human = ok(s.path(), &["show", &epic]);
    assert!(human.contains("멤버   0/1"), "{human}");
    assert!(human.contains(&work), "{human}");
    assert!(!human.contains(&thought), "머리글이 안 세는 줄을 그 밑에 그렸다 — {human}");
    let machine = ok(s.path(), &["show", &epic, "--json"]);
    assert!(!machine.contains(&thought), "{machine}");
}

/// **자식 줄도 제 종류와 미룸을 말한다.** 이 목록은 걸러지지 않으므로 담아 둔
/// 생각과 미뤄 둔 것이 그대로 서는데, 표가 없으면 `ready` 도 보드도 안 세는
/// 줄이 일과 똑같이 보인다.
#[test]
fn a_child_that_is_not_in_the_plan_says_so() {
    let s = init("childmark");
    let parent = add(s.path(), &["부모"]);
    let shelved = add(s.path(), &["미룰 자식", "--parent", &parent]);
    ok(s.path(), &["defer", &shelved]);
    ok(s.path(), &["idea", "add", "자식 생각", "--parent", &parent, "-q"]);

    let out = ok(s.path(), &["show", &parent]);
    assert!(out.contains("미룸"), "미뤄 둔 자식이 일과 똑같이 보인다 — {out}");
    assert!(out.contains("idea"), "담아 둔 자식이 일과 똑같이 보인다 — {out}");
}
/// **연습은 진짜와 같은 값을 말한다.** 마크다운은 `# [p1] 제목` 을 받고
/// `create_drafts` 는 그것을 에픽에도 그대로 쓰는데, 연습만 종류로 잘라 내면
/// 미리 검사하는 쪽이 안 적힌 값을 기본값으로 읽는다.
#[test]
fn a_rehearsal_reports_the_priority_it_will_write() {
    let s = init("dryrunprio");
    let plan = "# [p1] 급한 에픽\n- [p0] 첫 일\n";
    let out = from_stdin(s.path(), &["add", "--from", "-", "--dry-run", "--json"], plan);
    let seen = String::from_utf8(out.stdout).unwrap();
    assert!(seen.contains(r#""kind":"epic","title":"급한 에픽","priority":1"#), "{seen}");

    let real = from_stdin(s.path(), &["add", "--from", "-"], plan);
    assert!(real.status.success(), "{}", String::from_utf8_lossy(&real.stderr));
    let epic = issues(s.path()).lines().find(|l| l.contains("급한 에픽")).unwrap().to_string();
    assert!(epic.contains(r#""priority":1"#), "연습이 진짜보다 적게 말했다 — {epic}");
}

/// **연습이 진짜보다 엄하면 안 된다.** 진짜 `promote` 는 못 읽는 줄을 그대로
/// 들고 넘어가 0 으로 끝나는데 연습만 1 로 끝나면, 그것을 거절로 읽은 쪽이
/// 도구가 기꺼이 해 줄 계획을 버린다.
#[test]
fn a_rehearsal_is_no_stricter_than_the_real_run() {
    let s = init("dryruncarry");
    let thought = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let path = s.path().join(".moai/issues.jsonl");
    let mut raw = std::fs::read_to_string(&path).unwrap();
    raw.push_str("{\"id\":\"argos-zzzz\",\"title\":\"x\",\"kind\":\"몰라\",\"status\":\"todo\",\"created_at\":\"2026-01-01T00:00:00Z\",\"updated_at\":\"2026-01-01T00:00:00Z\",\"status_since\":\"2026-01-01T00:00:00Z\"}\n");
    std::fs::write(&path, raw).unwrap();

    let plan = "# 새 에픽\n- 첫 일\n";
    let rehearsal = from_stdin(s.path(), &["idea", "promote", &thought, "--from", "-", "--dry-run"], plan);
    assert!(rehearsal.status.success(), "연습만 실패로 끝난다 — {}", String::from_utf8_lossy(&rehearsal.stderr));
    // 어느 줄인지는 그래도 말한다.
    assert!(String::from_utf8_lossy(&rehearsal.stderr).contains("읽을 수 없는 줄"), "조용히 지나갔다");
    let real = from_stdin(s.path(), &["idea", "promote", &thought, "--from", "-"], plan);
    assert!(real.status.success(), "{}", String::from_utf8_lossy(&real.stderr));
}

/// **빈 까닭은 안 적는다.** `moai note` 가 같은 자리에서 거절하는데 여기만
/// 받으면, 이력에 내용 없는 `note:` 줄이 부를 때마다 하나씩 쌓인다.
#[test]
fn an_empty_reason_is_refused_like_an_empty_note() {
    let s = init("deferblank");
    let id = add(s.path(), &["일"]);
    let out = moai(s.path(), &["defer", &id, "-m", "   "]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("까닭이 비었다"), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!journal(s.path()).contains(r#""kind":"note""#), "거절해 놓고 적었다 — {}", journal(s.path()));
}

// ── 훅 — 규칙을 읽히는 자리에 놓는다 ────────────────────────────────

/// 훅은 stdin 으로 이벤트를 받고 stdout 으로 계약 JSON 을 낸다.
///
/// **세션 표를 시험마다 갈라 둔다.** 표가 섞이면 "세션당 한 번" 시험이 앞
/// 시험이 남긴 표를 보고 조용해져, 고장 난 채로 초록이 된다.
fn hook(s: &Scratch, event: &str, input: &str) -> Output {
    hook_in(s, s.path(), event, input)
}

/// 훅 프로세스를 `run_in` 에서 띄운다. 이벤트가 가리키는 저장소와 다른 자리여도
/// 판정이 같아야 한다 — 훅 프로세스의 자리는 아무도 약속하지 않았다.
fn hook_in(s: &Scratch, run_in: &Path, event: &str, input: &str) -> Output {
    use std::io::Write as _;
    let tmp = s.path().join("hooktmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = isolated(BIN)
        .args(["hook", event])
        .current_dir(run_in)
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
        .env("MOAI_NOW", NOW)
        .env("TMPDIR", &tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "훅이 비영으로 끝났다 — 훅은 무엇이 어긋나도 0 이어야 한다\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn hook_out(s: &Scratch, event: &str, input: &str) -> String {
    String::from_utf8(hook(s, event, input).stdout).unwrap()
}

/// 실린 글. 계약은 `hookSpecificOutput.additionalContext` 하나뿐이다.
///
/// **이스케이프를 푼 글이 아니라 파일에 적힌 그대로를 돌려준다.** 색이 샜는지
/// 보려면 그래야 한다 — `\u001b` 로 인코딩된 이스케이프는 글자로 풀어 놓으면
/// 평범한 제어문자가 되어 `one_json_value` 의 검사도 지나간다.
fn carried_text(out: &str) -> String {
    one_json_value(out);
    assert!(out.contains("\"hookSpecificOutput\""), "계약 JSON 이 아니다 — {out}");
    assert!(out.contains("\"hookEventName\""), "이벤트 이름이 없다 — {out}");
    let key = "\"additionalContext\":\"";
    let at = out.find(key).unwrap_or_else(|| panic!("실은 글이 없다 — {out}"));
    let rest = &out[at + key.len()..];
    // 싣는 글에 큰따옴표를 넣지 않는 시험만 이 헬퍼를 쓴다.
    rest[..rest.find('"').unwrap_or(rest.len())].to_string()
}

fn event(s: &Scratch, session: &str) -> String {
    let cwd = s.path().display().to_string();
    assert!(!cwd.contains(['"', '\\']), "시험 경로에 따옴표가 있다 — {cwd}");
    format!("{{\"session_id\":\"{session}\",\"cwd\":\"{cwd}\"}}")
}

/// 컨텍스트가 접힌 뒤 다시 열린 세션의 이벤트.
fn compacted(s: &Scratch, session: &str) -> String {
    event(s, session).replacen('{', "{\"source\":\"compact\",", 1)
}

/// 그 세션이 적어 둔 기준선. **이름을 박지 않는다** — 표의 키에는 저장소도
/// 들어서, 파일 이름을 시험이 다시 지어내면 구현과 조용히 어긋난다.
fn baseline(s: &Scratch, session: &str) -> Option<usize> {
    let dir = s.path().join("hooktmp");
    let head = format!("moai-hook-{session}-");
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with(&head) && name.ends_with(".warn")
        })
        .and_then(|e| std::fs::read_to_string(e.path()).ok())
        .and_then(|t| t.trim().parse().ok())
}

/// 보드는 세션의 **첫 프롬프트에 한 번만** 실린다.
///
/// 한 번이 아니면 매 프롬프트에 같은 1KB 가 붙고, 그러면 사람이 훅을 꺼 버린다.
/// 아예 안 실리면 이어받는 세션이 무엇이 열려 있는지 모른 채 규칙만 만난다.
#[test]
fn the_board_rides_the_first_prompt_only() {
    let s = init("hookboard");
    ok(s.path(), &["add", "락을 잡는다"]);

    let first = carried_text(&hook_out(&s, "user-prompt-submit", &event(&s, "s1")));
    assert!(first.contains("락을 잡는다"), "보드가 안 실렸다\n{first}");
    assert!(first.contains("TodoWrite"), "무엇을 쓰라는 말이 없다\n{first}");

    let again = hook_out(&s, "user-prompt-submit", &event(&s, "s1"));
    assert!(again.trim().is_empty(), "같은 세션에 두 번 실었다\n{again}");

    // 다른 세션은 다시 받는다 — 표는 세션마다 따로다.
    let other = hook_out(&s, "user-prompt-submit", &event(&s, "s2"));
    assert!(!other.trim().is_empty(), "새 세션이 보드를 못 받았다");
}

/// 실리는 글에 색이 섞이면 안 된다. 계약 JSON 안의 이스케이프는 아무도
/// 걷어내지 않아 받는 쪽 화면에 그 글자가 그대로 뜬다.
#[test]
fn no_escape_codes_ride_the_contract() {
    let s = init("hookcolor");
    ok(s.path(), &["add", "락을 잡는다"]);
    // `NO_COLOR` 를 주지 않는다 — 끄고 시험하면 새는 것을 못 본다.
    let out = hook_out(&s, "user-prompt-submit", &event(&s, "s1"));
    // JSON 안에서 색은 `\u001b` 로 인코딩되어 실려 나간다. 날 이스케이프만
    // 찾으면 바로 이 새는 길을 못 본다.
    assert!(!out.contains("\\u001b"), "색이 샜다\n{out}");
    assert!(!out.contains('\u{1b}'), "날 이스케이프가 샜다\n{out}");
}

/// 접힌 뒤(`SessionStart` 의 `source=compact`)에 집고 있던 것이 실린다.
/// 집은 것이 없으면 조용하다 — 막 비운 자리에 "없다" 를 적을 일이 아니다.
///
/// **이 시험은 바이너리의 stdout 만 본다.** 그 글이 대화에 실리는지는 말하지
/// 못한다 — `PreCompact` 에 싣던 시절에도 여기는 초록이었고 `claude` 는 그
/// 출력을 거절했다. 붙는지는 사람이 `/compact` 를 쳐서 봤다 (moai-91jk).
#[test]
fn what_is_held_rides_the_fold() {
    let s = init("hookfold");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "--json"]), "id");

    let quiet = hook_out(&s, "session-start", &compacted(&s, "s1"));
    assert!(quiet.trim().is_empty(), "집은 것이 없는데 실었다\n{quiet}");

    ok(s.path(), &["mv", &id, "in_progress"]);
    let out = hook_out(&s, "session-start", &compacted(&s, "s1"));
    assert!(out.contains("\"hookEventName\":\"SessionStart\""), "이벤트 이름이 틀렸다\n{out}");
    let held = carried_text(&out);
    assert!(held.contains(&id), "집은 것의 id 가 없다\n{held}");
    assert!(held.contains("락을 잡는다"), "집은 것의 제목이 없다\n{held}");

    // 처음 열린 세션과 재개는 여전히 아무것도 안 싣는다.
    for source in ["startup", "resume", "clear"] {
        let input = event(&s, "s2").replacen('{', &format!("{{\"source\":\"{source}\","), 1);
        let out = hook_out(&s, "session-start", &input);
        assert!(out.trim().is_empty(), "{source} 에서 무언가 실었다\n{out}");
    }
}

/// **접힌 뒤는 같은 세션이다.** 기준선은 그대로 두고, 보드는 다음 프롬프트에
/// 다시 싣는다 — 접힐 때 보드도 같이 떨어졌다.
///
/// 기준선을 다시 적으면 접기 전에 늘린 경고가 물려받은 빚에 묻혀 `Stop` 이
/// 그것을 영영 못 본다.
#[test]
fn the_fold_keeps_the_baseline_and_reloads_the_board() {
    let s = init("hookrefold");
    ok(s.path(), &["add", "락을 잡는다"]);
    hook_out(&s, "session-start", &event(&s, "s1"));
    let before = baseline(&s, "s1").expect("기준선을 안 적었다");

    let first = hook_out(&s, "user-prompt-submit", &event(&s, "s1"));
    assert!(!first.trim().is_empty(), "보드가 안 실렸다");
    assert!(hook_out(&s, "user-prompt-submit", &event(&s, "s1")).trim().is_empty());

    // 접기 전에 경고를 하나 늘린다.
    ok(s.path(), &["add", "또 하나"]);
    hook_out(&s, "session-start", &compacted(&s, "s1"));
    assert_eq!(baseline(&s, "s1"), Some(before), "접힌 뒤에 기준선을 다시 적었다");

    let again = carried_text(&hook_out(&s, "user-prompt-submit", &event(&s, "s1")));
    assert!(again.contains("락을 잡는다"), "접힌 뒤 보드를 다시 안 실었다\n{again}");

    // 여는 훅이 안 돌았던 세션은 접힌 뒤에라도 기준선을 얻는다 — 안 그러면
    // `Stop` 이 끝까지 견줄 것이 없다.
    assert_eq!(baseline(&s, "s3"), None);
    hook_out(&s, "session-start", &compacted(&s, "s3"));
    assert!(baseline(&s, "s3").is_some(), "기준선 없는 세션이 접힌 뒤에도 기준선을 못 얻었다");
}

/// **알림은 `Stop` 이 세는 경고가 아니다.** 생각을 담고 일을 미루는 것은
/// 도구가 권하는 일인데, 그것이 경고 수를 올리면 시킨 대로 한 세션이 "경고가
/// 늘었다" 로 붙들린다 — 실제로 그랬다(moai-c8lb).
#[test]
fn stop_does_not_count_notices_as_new_warnings() {
    let s = init("hookstopnotice");
    ok(s.path(), &["add", "락을 잡는다"]);
    hook_out(&s, "session-start", &event(&s, "s1"));

    for n in 0..5 {
        ok(s.path(), &["idea", "add", &format!("생각 {n}"), "-q"]);
    }
    let later = add(s.path(), &["나중에"]);
    ok(s.path(), &["defer", &later]);

    let out = hook_out(&s, "stop", &event(&s, "s1"));
    assert!(!out.contains("경고가"), "알림을 경고로 셌다 — {out}");
}

/// `SessionStart` 는 아무것도 싣지 않고 기준선만 적는다.
///
/// 재개에서 그 출력이 대화에 안 붙는 것을 확인하고 옮긴 자리다. 안 붙는 자리에
/// 대고 실으면 훅이 사는지 죽었는지 모른 채 규칙만 남는다.
#[test]
fn the_session_start_only_writes_the_baseline() {
    let s = init("hookbase");
    // 에픽 없는 이슈 하나 = 경고 하나.
    ok(s.path(), &["add", "락을 잡는다"]);

    let out = hook_out(&s, "session-start", &event(&s, "s1"));
    assert!(out.trim().is_empty(), "SessionStart 가 무언가 실었다\n{out}");

    let n: usize = baseline(&s, "s1").expect("기준선을 안 적었다");
    assert!(n > 0, "경고가 있는데 기준선이 0 이다");
}

/// **훅은 실패하지 않는다.** 무엇이 어긋나도 빈 출력과 종료 코드 0 이다.
///
/// 훅이 에러를 뱉으면 매 세션 시작이 시끄럽고, 그러면 사람이 훅을 꺼 버린다 —
/// 꺼진 규칙은 없는 규칙이다.
#[test]
fn the_hook_never_fails() {
    let s = init("hooksafe");
    let outside = Scratch::new("hooksafe-outside");
    for (what, input) in [
        ("저장소가 아닌 곳", event(&outside, "s1")),
        ("없는 자리", "{\"cwd\":\"/does/not/exist\",\"session_id\":\"s1\"}".into()),
        ("JSON 이 아닌 입력", "not json at all".into()),
        ("빈 입력", String::new()),
        ("모르는 필드만 잔뜩", "{\"context_tokens\":9,\"transcript_path\":\"/x\"}".into()),
    ] {
        for ev in ["session-start", "user-prompt-submit", "pre-tool-use", "stop"] {
            let out = hook_out(&s, ev, &input);
            assert!(out.trim().is_empty(), "{what} 에서 {ev} 가 무언가 냈다\n{out}");
        }
    }
}

/// 자리는 stdin 이 정한다. 훅 프로세스가 어디서 도는지는 아무도 약속하지 않았다.
#[test]
fn the_hook_works_where_stdin_says() {
    let s = init("hookcwd");
    ok(s.path(), &["add", "락을 잡는다"]);
    let elsewhere = Scratch::new("hookcwd-elsewhere");

    // 프로세스는 남의 디렉터리에서 돌지만, 이벤트가 가리키는 저장소를 읽는다.
    use std::io::Write as _;
    let tmp = s.path().join("hooktmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut child = isolated(BIN)
        .args(["hook", "user-prompt-submit"])
        .current_dir(elsewhere.path())
        .env("MOAI_NOW", NOW)
        .env("TMPDIR", &tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(event(&s, "s9").as_bytes());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let text = carried_text(&String::from_utf8(out.stdout).unwrap());
    assert!(text.contains("락을 잡는다"), "stdin 이 가리킨 저장소를 안 읽었다\n{text}");
}

/// 못 읽는 줄이 있으면 그 사실이 **실리는 보드에** 들어야 한다.
///
/// 빈 슬라이스를 넘기던 자리다. 깨진 줄 경고는 실린 보드 말고는 에이전트가
/// 알아낼 길이 없고, 기준선까지 같은 만큼 낮게 잡혀 `Stop` 이 "늘었다" 를
/// 영영 못 보게 된다.
#[test]
fn a_broken_line_rides_the_board_too() {
    let s = init("hookbroken");
    ok(s.path(), &["add", "락을 잡는다"]);
    let path = s.path().join(".moai").join("issues.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{ 이건 JSON 이 아니다\n");
    std::fs::write(&path, text).unwrap();

    let board = carried_text(&hook_out(&s, "user-prompt-submit", &event(&s, "s1")));
    assert!(board.contains("읽을 수 없는"), "깨진 줄이 보드에 없다\n{board}");

    hook(&s, "session-start", &event(&s, "s2"));
    let with_broken = baseline(&s, "s2").expect("기준선을 안 적었다");
    assert!(with_broken >= 2, "깨진 줄이 기준선에서 빠졌다 — {with_broken}");
}

/// 한 세션이 저장소 둘을 오가도 각자 제 보드를 받는다.
///
/// 표의 키가 세션 id 뿐이면 둘째 저장소는 첫째의 표를 보고 영영 조용해진다.
#[test]
fn two_repos_in_one_session_each_get_a_board() {
    let one = init("hooktwo-a");
    let two = init("hooktwo-b");
    ok(one.path(), &["add", "저장 계층"]);
    ok(two.path(), &["add", "표면 다듬기"]);
    // 표를 한 자리에 모아 둔다 — 갈라 두면 시험이 저 스스로 답을 만든다.
    let tmp = one.path().join("hooktmp");
    std::fs::create_dir_all(&tmp).unwrap();
    std::os::unix::fs::symlink(&tmp, two.path().join("hooktmp")).unwrap();

    let first = carried_text(&hook_out(&one, "user-prompt-submit", &event(&one, "same")));
    assert!(first.contains("저장 계층"), "{first}");
    let second = hook_out(&two, "user-prompt-submit", &event(&two, "same"));
    assert!(!second.trim().is_empty(), "둘째 저장소가 보드를 못 받았다");
    assert!(carried_text(&second).contains("표면 다듬기"), "{second}");
}

/// 막을 때의 계약 — `permissionDecision: "deny"` 와 읽을 수 있는 까닭.
fn refusal(out: &str) -> String {
    one_json_value(out);
    assert!(out.contains("\"permissionDecision\":\"deny\""), "막지 않았다 — {out}");
    let key = "\"permissionDecisionReason\":\"";
    let at = out.find(key).unwrap_or_else(|| panic!("까닭이 없다 — {out}"));
    let rest = &out[at + key.len()..];
    rest[..rest.find("\"}").unwrap_or(rest.len())].to_string()
}

fn call(s: &Scratch, tool: &str, body: &str, session: &str) -> String {
    let cwd = s.path().display().to_string();
    hook_out(
        s,
        "pre-tool-use",
        &format!(
            "{{\"session_id\":\"{session}\",\"cwd\":\"{cwd}\",\"tool_name\":\"{tool}\",\"tool_input\":{body}}}"
        ),
    )
}

/// 규칙이 실제로 계약 JSON 으로 나온다. **막힌 쪽이 읽고 그대로 고칠 수 있는
/// 글이어야 한다** — 고칠 명령 없는 거절은 사람을 부르는 게이트다.
#[test]
fn a_refused_call_carries_the_way_out() {
    let s = init("hookrule");
    let epic = field(&ok(s.path(), &["add", "저장 계층", "--type", "epic", "--json"]), "id");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "-e", &epic, "--json"]), "id");

    // 집은 것이 없으면 저장소 파일을 못 고친다.
    let why = refusal(&call(&s, "Edit", "{\"file_path\":\"src/store.rs\"}", "s1"));
    assert!(why.contains(&format!("moai mv {id} in_progress")), "집을 것을 안 낸다 — {why}");

    ok(s.path(), &["mv", &id, "in_progress"]);
    assert!(
        call(&s, "Edit", "{\"file_path\":\"src/store.rs\"}", "s1").trim().is_empty(),
        "집었는데도 막는다"
    );

    // 집은 뒤에는 그 단위 밖에 세우는 것이 막힌다.
    let why = refusal(&call(&s, "Bash", "{\"command\":\"moai add \\\"딴 일\\\"\"}", "s1"));
    assert!(why.contains(&format!("-e {epic}")), "에픽을 안 가리킨다 — {why}");
    assert!(
        call(&s, "Bash", &format!("{{\"command\":\"moai add 안의일 -e {epic}\"}}"), "s1")
            .trim()
            .is_empty(),
        "단위 안인데 막는다"
    );
}

/// `Stop` 은 **세션당 한 번만** 붙든다. 규칙 2 가 초점을 요구하므로, 그것
/// 없이는 일하는 내내 매 턴이 붙들린다 — 같은 잔소리를 매번 들으면 아무도
/// 안 읽는다. 이미 붙든 뒤라고 알려 오면(`stop_hook_active`) 바로 보낸다.
#[test]
fn the_close_holds_once_and_then_lets_go() {
    let s = init("hookstop");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "--json"]), "id");
    ok(s.path(), &["mv", &id, "in_progress"]);
    let cwd = s.path().display().to_string();
    let ev = format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\"}}");

    let held = hook_out(&s, "stop", &ev);
    assert!(held.contains("\"decision\":\"block\""), "안 붙들었다 — {held}");
    assert!(held.contains(&format!("moai mv {id} done")), "옮길 길이 없다 — {held}");

    assert!(hook_out(&s, "stop", &ev).trim().is_empty(), "두 번 붙들었다");

    // 이미 붙든 턴이라고 알려 오면 그 자리에서 보낸다.
    let s2 = init("hookstop2");
    let id2 = field(&ok(s2.path(), &["add", "락을 잡는다", "--json"]), "id");
    ok(s2.path(), &["mv", &id2, "in_progress"]);
    let cwd2 = s2.path().display().to_string();
    let out = hook_out(
        &s2,
        "stop",
        &format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd2}\",\"stop_hook_active\":true}}"),
    );
    assert!(out.trim().is_empty(), "이미 붙든 턴을 또 붙들었다 — {out}");
}

/// 규칙은 **사람을 부르지 않는다.** 막힌 쪽이 거절문이 낸 명령을 그대로
/// 부르면 지나간다 — 값은 왕복 한 번이고, 그것이 옛 게이트와 갈리는 지점이다.
#[test]
fn every_refusal_is_undone_by_its_own_advice() {
    let s = init("hookundo");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "--json"]), "id");

    let why = refusal(&call(&s, "Edit", "{\"file_path\":\"src/store.rs\"}", "s1"));
    // 거절문이 낸 첫 명령을 그대로 친다.
    // 까닭은 계약 JSON 에서 꺼낸 **원시** 글이라 줄바꿈이 아직 `\n` 두 글자다.
    let line = why
        .split("\\n")
        .find(|l| l.trim_start().starts_with("moai mv"))
        .expect("집을 명령이 없다");
    let args: Vec<&str> = line.split_whitespace().skip(1).take(3).collect();
    ok(s.path(), &args);
    assert!(
        call(&s, "Edit", "{\"file_path\":\"src/store.rs\"}", "s1").trim().is_empty(),
        "시킨 대로 했는데 또 막는다"
    );
    assert!(args.contains(&id.as_str()), "낸 명령이 실제 이슈를 안 가리킨다 — {line}");
}

// ── 훅 판정 — 시험판이 실제로 넘어진 자리를 계약째로 다시 밟는다 ──────
//
// 판정 자체는 `hook.rs` 의 단위 시험이 본다. 여기는 **stdin JSON 을 넣고
// stdout JSON 을 견준다** — 도구 이름을 가르는 자리, `cwd` 로 경로를 푸는
// 자리, 계약 JSON 을 쓰는 자리는 단위 시험이 못 밟는다. 시험판에서 잡힌
// 버그 셋 중 둘이 바로 그 자리에 있었다.

/// JSON 문자열 하나. **제어문자는 전부 이스케이프한다** — 하나라도 날것으로 들어가면
/// 훅이 stdin 을 못 풀어 조용히 지나가고, "막지 않는다" 는 시험이 헛으로 초록이 된다.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn shell_call(s: &Scratch, cmd: &str) -> String {
    call(s, "Bash", &format!("{{\"command\":{}}}", json_str(cmd)), "s1")
}

/// **훅 프로세스는 남의 자리에서 띄운다.** 저장소 안에서 띄우면 `cwd` 를
/// 무시하는 판도 상대 경로를 우연히 옳게 풀어, 시험이 아무것도 안 지킨다.
fn edit_call(s: &Scratch, elsewhere: &Scratch, path: &str) -> String {
    let input = format!(
        "{{\"session_id\":\"s1\",\"cwd\":{},\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":{}}}}}",
        json_str(&s.path().display().to_string()),
        json_str(path)
    );
    String::from_utf8(hook_in(s, elsewhere.path(), "pre-tool-use", &input).stdout).unwrap()
}

fn review_call(s: &Scratch) -> String {
    call(s, "Skill", "{\"skill\":\"code-review\",\"args\":\"high\"}", "s1")
}

/// 규칙 1 — 초점 밖에 세우지 않는다. **담는 것과 계획을 세우는 것은 자유다.**
#[test]
fn creation_is_judged_through_the_contract() {
    let s = init("hookcreate");
    let epic = field(&ok(s.path(), &["add", "저장 계층", "--type", "epic", "--json"]), "id");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "-e", &epic, "--json"]), "id");

    // 집은 것이 없으면 무엇을 세워도 지나간다.
    assert!(shell_call(&s, "moai add \"딴 일\"").trim().is_empty(), "초점 없이 막았다");

    ok(s.path(), &["mv", &id, "in_progress"]);
    refusal(&shell_call(&s, "moai add \"딴 일\""));
    for free in [
        format!("moai add \"안의 일\" -e {epic}"),
        format!("moai add \"자식\" --parent {id}"),
        "moai idea add \"떠오른 것\"".to_string(),
        "moai add --from - <<'MD'\n# 딴 에픽\n- 첫 이슈\nMD".to_string(),
        // 리뷰 이야기를 적는 메모는 글자로 가르지 않는다.
        format!("moai note {id} \"code-review 가 moai add 를 짚었다\""),
    ] {
        let out = shell_call(&s, &free);
        assert!(out.trim().is_empty(), "막혔다 — {free}\n{out}");
    }
}

/// 규칙 2 — 저장소를 고치기 전에 하나를 집는다. **세는 것은 저장소 안의
/// 일감뿐이다.** 상대 경로는 stdin 의 `cwd` 로 푼다 — 훅 프로세스의 자리로
/// 풀던 시험판은 `src/main.rs` 를 저장소 밖으로 보아 규칙이 통째로 샜다.
#[test]
fn edits_are_judged_through_the_contract() {
    let s = init("hookedit");
    ok(s.path(), &["add", "락을 잡는다"]);
    let root = s.path().display().to_string();
    let other = Scratch::new("hookedit-other");

    for counted in [format!("{root}/src/store.rs"), format!("{root}/CLAUDE.md"), "src/main.rs".into()] {
        refusal(&edit_call(&s, &other, &counted));
    }
    for free in [
        format!("{root}/.moai/config.toml"),
        format!("{root}/.claude/settings.json"),
        format!("{root}/target/release/moai"),
        std::env::temp_dir().join("scratchpad/memo.md").display().to_string(),
        format!("{}/src/main.rs", other.path().display()),
    ] {
        let out = edit_call(&s, &other, &free);
        assert!(out.trim().is_empty(), "막혔다 — {free}\n{out}");
    }

    // 껍데기로 쓰는 것도 같은 규칙이다. 상대 경로는 stdin 의 `cwd` 로 푼다.
    let why = refusal(&shell_call(&s, "sed -i 's/a/b/' src/store.rs"));
    assert!(why.contains("src/store.rs"), "무엇을 고치려 했는지가 없다 — {why}");
    for free in ["grep -rn x src > /dev/null", "cargo test 2>&1 | tail -5"] {
        let out = shell_call(&s, free);
        assert!(out.trim().is_empty(), "막혔다 — {free}\n{out}");
    }
}

/// **옆 워크트리가 쥔 일은 이 세션의 일이 아니다.** 집기 커밋이 main 에 들어오면 main
/// 스냅샷에는 옆에서 집은 줄이 벌여 놓은 칸에 선다 — 그것을 제 일로 세던 훅은 main
/// 세션에 남의 일을 옮기라고 붙들고 main 의 `moai add` 를 막았다(moai-0yrv). 워크트리
/// 목록을 읽는 배선은 단위 시험이 못 밟아 여기서 본다.
#[test]
fn the_hook_leaves_what_a_named_worktree_holds_to_that_worktree() {
    let s = Scratch::new("hookaway");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let there = field(&ok(&main, &["add", "옆에서 할 일", "--json"]), "id");
    ok(&main, &["mv", &there, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);

    let stop = |session: &str| {
        let input = format!("{{\"session_id\":\"{session}\",\"cwd\":{}}}", json_str(&main.display().to_string()));
        String::from_utf8(hook_in(&s, &main, "stop", &input).stdout).unwrap()
    };
    let add = |session: &str| {
        let input = format!(
            "{{\"session_id\":\"{session}\",\"cwd\":{},\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{}}}}}",
            json_str(&main.display().to_string()),
            json_str("moai add \"딴 일\"")
        );
        String::from_utf8(hook_in(&s, &main, "pre-tool-use", &input).stdout).unwrap()
    };

    // 옆 워크트리가 없으면 여전히 이 자리의 일이다.
    assert!(stop("s1").contains(&format!("moai mv {there}")), "워크트리 없이도 안 붙든다");
    assert!(refusal(&add("s1")).contains(&there), "워크트리 없이도 안 막는다");

    let dir = format!(".claude/worktrees/{there}");
    let branch = format!("worktree-{there}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &branch]);
    let held = stop("s2");
    assert!(held.trim().is_empty(), "옆 워크트리가 쥔 일로 붙든다\n{held}");
    let made = add("s2");
    assert!(made.trim().is_empty(), "옆 워크트리가 쥔 일로 생성을 막는다\n{made}");

    // 그 워크트리 안에서는 제 일이다.
    let inside = main.join(&dir);
    let input = format!("{{\"session_id\":\"s3\",\"cwd\":{}}}", json_str(&inside.display().to_string()));
    let own = String::from_utf8(hook_in(&s, &inside, "stop", &input).stdout).unwrap();
    assert!(own.contains(&format!("moai mv {there}")), "제 워크트리에서 제 일을 놓친다\n{own}");
}

/// 규약대로 일을 main 에서 집고 그 이름의 워크트리를 띄운 저장소. (main, 워크트리, 집은 id)
fn picked_in_a_worktree(s: &Scratch) -> (PathBuf, PathBuf, String) {
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let id = field(&ok(&main, &["add", "워크트리에서 할 일", "--json"]), "id");
    ok(&main, &["mv", &id, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    let dir = format!(".claude/worktrees/{id}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &format!("worktree-{id}")]);
    let inside = main.join(&dir);
    (main, inside, id)
}

/// 도구 호출 하나를 `cwd` 자리의 세션으로 부른다.
fn tool_at(s: &Scratch, cwd: &Path, tool: &str, body: &str) -> String {
    let input = format!(
        "{{\"session_id\":\"s1\",\"cwd\":{},\"tool_name\":\"{tool}\",\"tool_input\":{body}}}",
        json_str(&cwd.display().to_string())
    );
    String::from_utf8(hook_in(s, cwd, "pre-tool-use", &input).stdout).unwrap()
}

/// **워크트리 안의 리뷰 규칙은 main 에서 집은 리뷰를 본다** (moai-w2iy). 트래커는 main 에서
/// 만지는 것이 규약이라, 리뷰 이슈는 main 스냅샷에만 있고 워크트리의 스냅샷(HEAD)에는
/// 없다 — 제 스냅샷만 읽던 훅은 시킨 대로 세우고 집은 리뷰를 "없다" 로 막았다.
#[test]
fn a_review_picked_in_main_opens_the_review_inside_the_worktree() {
    let s = Scratch::new("hookreviewwt");
    let (main, inside, id) = picked_in_a_worktree(&s);
    let review = "{\"skill\":\"code-review\",\"args\":\"high\"}";

    // 리뷰 이슈가 어디에도 없으면 여전히 막는다 — 겹쳐 봐도 풀리지 않는다.
    let why = refusal(&tool_at(&s, &inside, "Skill", review));
    assert!(why.contains(&format!("--parent {id}")), "{why}");

    let r = field(
        &ok(&main, &["add", "리뷰 — 워크트리 일", "-t", "review", "--parent", &id, "-b", "무엇을 왜 보는가", "--json"]),
        "id",
    );
    ok(&main, &["mv", &r, "in_progress"]);
    let out = tool_at(&s, &inside, "Skill", review);
    assert!(out.trim().is_empty(), "main 에서 집은 리뷰를 못 보고 막는다\n{out}");
}

/// **`moai` 는 그 명령이 가리키는 저장소의 트래커로 판정한다** (moai-23ky) — `-C`·`--dir`
/// 나 앞의 `cd`. 세션 자리의 트래커로 판정하던 훅은 남의 프로젝트에 세우는 줄을 제
/// 초점으로 막았고, 남의 프로젝트가 쥔 초점은 못 봤다.
#[test]
fn a_moai_call_is_judged_by_the_tracker_it_points_at() {
    let a = init("hookaimA");
    let b = init("hookaimB");
    let held = field(&ok(a.path(), &["add", "여기서 할 일", "--json"]), "id");
    ok(a.path(), &["mv", &held, "in_progress"]);
    let bash = |s: &Scratch, cmd: &str| tool_at(s, s.path(), "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));
    let bp = b.path().display().to_string();

    // A 가 쥔 것으로 B 에 세우는 줄을 막지 않는다.
    for cmd in [
        format!("moai -C {bp} add \"딴 일\""),
        format!("moai --dir={bp} add \"딴 일\""),
        format!("cd {bp} && moai add \"딴 일\""),
    ] {
        let out = bash(&a, &cmd);
        assert!(out.trim().is_empty(), "남의 트래커에 세우는 줄을 제 초점으로 막았다 — {cmd}\n{out}");
    }
    // 같은 줄의 제 자리 토막은 여전히 제 트래커로 본다.
    let why = refusal(&bash(&a, &format!("moai -C {bp} add \"딴 일\" && moai add \"또 딴 일\"")));
    assert!(why.contains(&held), "{why}");

    // 거꾸로 — B 가 쥔 것이 있으면 B 에 세우는 줄은 B 의 초점으로 막힌다.
    let theirs = field(&ok(b.path(), &["add", "저기서 할 일", "--json"]), "id");
    ok(b.path(), &["mv", &theirs, "in_progress"]);
    ok(a.path(), &["mv", &held, "done"]);
    let why = refusal(&bash(&a, &format!("moai -C {bp} add \"딴 일\"")));
    assert!(why.contains(&theirs), "가리킨 트래커의 초점을 못 봤다 — {why}");
}

/// 워크트리 세션이 `-C <main>` 으로 트래커를 만진다 — **같은 저장소의 워크트리는 이 세션의
/// 자리다.** main 의 눈으로만 보면 이 워크트리가 쥔 일은 "옆의 것" 이라 초점에서 빠져,
/// `-C <main>` 한 번으로 규칙 1 을 넘는다. main 에서 방금 집은 줄은 겹쳐 보고 안다.
#[test]
fn a_worktree_session_touching_main_is_still_that_session() {
    let s = Scratch::new("hookaimwt");
    let (main, inside, id) = picked_in_a_worktree(&s);
    let mp = main.display().to_string();
    let bash = |cmd: &str| tool_at(&s, &inside, "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));

    let why = refusal(&bash(&format!("moai -C {mp} add \"딴 일\"")));
    assert!(why.contains(&id), "main 을 가리키면 제 초점을 잃는다 — {why}");
    let out = bash(&format!("moai -C {mp} add \"자식\" --parent {id}"));
    assert!(out.trim().is_empty(), "제 일의 자식을 막았다\n{out}");

    // main 에서 둘째 일을 집었다 — 워크트리 스냅샷은 모르지만 막지 않는다(moai-iaa4 의 자리).
    let next = field(&ok(&main, &["add", "둘째 일", "--json"]), "id");
    ok(&main, &["mv", &next, "in_progress"]);
    let out = bash(&format!("moai -C {mp} add \"둘째의 자식\" --parent {next}"));
    assert!(out.trim().is_empty(), "main 에서 방금 집은 일의 자식을 막았다\n{out}");

    // 거꾸로 — 세션은 main 에 서 있고 명령이 워크트리로 들어간다(에이전트 스레드는 자리가 main
    // 으로 돌아온다). **딸린 워크트리를 가리키면 그 워크트리의 일로 본다** — main 의 눈으로는
    // 그 워크트리의 일이 "옆의 것" 이라 제 단위 안의 줄이 막히고, 단위 밖의 줄은 샌다.
    ok(&main, &["mv", &next, "done"]);
    let ip = inside.display().to_string();
    let from_main = |cmd: &str| tool_at(&s, &main, "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));
    let out = from_main(&format!("cd {ip} && moai add \"자식\" --parent {id}"));
    assert!(out.trim().is_empty(), "워크트리로 들어가 제 일의 자식을 세우는 것을 막았다\n{out}");
    let why = refusal(&from_main(&format!("moai -C {ip} add \"딴 일\"")));
    assert!(why.contains(&id), "워크트리를 가리킨 단위 밖 줄을 그 워크트리의 초점으로 못 막는다 — {why}");
}

/// 규칙 2 의 껍데기 쪽은 **stdin 의 `cwd` 로** 상대 경로를 푼다. 훅 프로세스를
/// 저장소 뿌리에서 띄우고 `cwd` 만 하위 디렉터리로 준다 — 뿌리로 푸는 판은 여기서
/// `a.md` 를 대고, 트래커 안에 서서 친 `../src` 쓰기는 놓친다. 단위 시험은 순수
/// 함수만 보므로 이 배선은 여기서만 밟힌다.
#[test]
fn a_shell_write_is_resolved_where_the_shell_stands() {
    let s = init("hookcwd");
    ok(s.path(), &["add", "락을 잡는다"]);
    let at = |cwd: &Path, cmd: &str| {
        let input = format!(
            "{{\"session_id\":\"s1\",\"cwd\":{},\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{}}}}}",
            json_str(&cwd.display().to_string()),
            json_str(cmd)
        );
        String::from_utf8(hook_in(&s, s.path(), "pre-tool-use", &input).stdout).unwrap()
    };
    let docs = s.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    let why = refusal(&at(&docs, "echo x > a.md"));
    assert!(why.contains("docs/a.md"), "껍데기의 자리로 안 풀었다 — {why}");

    let out = at(&s.path().join(".moai"), "echo x > ../src/a.rs");
    let why = refusal(&out);
    assert!(why.contains("src/a.rs"), "{why}");
    let target = s.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    let out = at(&target, "echo x > out.log");
    assert!(out.trim().is_empty(), "빌드 산출물 자리의 쓰기를 막았다\n{out}");
}

/// 규칙 3 — 리뷰도 이슈다. 그 리뷰는 **지금 보는 것에 매여야 한다.**
#[test]
fn reviews_are_judged_through_the_contract() {
    let s = init("hookreview");
    let epic = field(&ok(s.path(), &["add", "저장 계층", "--type", "epic", "--json"]), "id");
    let far = field(&ok(s.path(), &["add", "표면", "--type", "epic", "--json"]), "id");
    let id = field(&ok(s.path(), &["add", "락을 잡는다", "-e", &epic, "--json"]), "id");
    let review = field(
        &ok(s.path(), &["add", "리뷰 — 락", "-t", "review", "-e", &far, "-b", "락을 본다", "--json"]),
        "id",
    );

    // 집은 것이 없고 리뷰 줄이 놀고 있으면 그것을 집으라고 한다.
    let why = refusal(&review_call(&s));
    assert!(why.contains(&format!("moai mv {review} in_progress")), "집을 명령이 없다 — {why}");

    // 집은 것이 있는데 리뷰가 딴 에픽에 있으면 막는다.
    ok(s.path(), &["mv", &id, "in_progress"]);
    let why = refusal(&review_call(&s));
    assert!(why.contains(&review), "안 매인 리뷰를 안 짚는다 — {why}");

    // 같은 에픽으로 옮기면 지나간다.
    ok(s.path(), &["edit", &review, "-e", &epic]);
    let out = review_call(&s);
    assert!(out.trim().is_empty(), "매였는데 막는다\n{out}");
}

// ── skill status · uninstall — 심은 것을 보이고 걷어낸다 ──────────────
//
// **진짜 `claude` 에 닿지 않는다.** `HOME` 을 시험 디렉터리로, `PATH` 를 가짜
// `claude` 와 `/usr/bin:/bin` 으로만 둔다. 가짜는 받은 인자를 적어 두고 0 을
// 낸다. 이 시험이 사람의 등록을 걷는 날이 오면 그것이 제일 나쁜 버그다.

/// **실행할 파일은 이 프로세스에서 쓰지 않는다 — `cp` 에게 쓰게 한다.** 이 프로세스가
/// 쓰기 fd 를 여는 순간 옆 스레드의 시험이 fork 하면 그 자식이 fd 를 물려받고,
/// 자식이 exec 할 때까지 그 inode 에 쓰는 이가 남아 Linux 가 exec 을 ETXTBSY 로
/// 거절한다. 임시 이름에 쓰고 `rename` 해도 **inode 가 같아** 소용없다 — 재어 보니
/// 복사 2,400 번에 `fs::copy` 345 번, 복사+rename 294 번, `cp` 0 번 터졌다.
/// 쓰기 fd 가 `cp` 안에만 살면 이 프로세스의 fork 가 그것을 물려받을 길이 없다.
fn place_exe(src: &Path, dst: &Path) {
    let out = Command::new("cp").arg(src).arg(dst).output().unwrap();
    assert!(out.status.success(), "{}", text(&out));
}

struct Claude {
    home: Scratch,
    bin: PathBuf,
    log: PathBuf,
}

impl Claude {
    fn new(name: &str) -> Claude {
        Claude::failing_on(name, None)
    }

    /// 인자에 `word` 가 들면 비영으로 끝나는 가짜.
    fn failing_on(name: &str, word: Option<&str>) -> Claude {
        let home = Scratch::new(name);
        let bin = home.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let log = home.path().join("claude.log");
        // 원본은 `bin` 밖에 쓰고 `place_exe` 로 내놓는다 — 이 프로세스가 쓴 inode 를
        // moai 가 exec 하면 옆 시험의 fork 와 겹쳐 ETXTBSY 가 날 수 있다.
        let source = home.path().join("claude.sh");
        // 패턴을 따옴표로 싼다 — 안 싸면 `plugin uninstall` 의 빈칸이 패턴을 둘로
        // 갈라 문법 오류가 나고, 가짜가 **모든** 부름에 비영으로 끝난다.
        let fail = word.map(|w| format!("case \"$*\" in *\"{w}\"*) exit 1;; esac\n")).unwrap_or_default();
        std::fs::write(&source, format!("#!/bin/sh\necho \"$@\" >> \"{}\"\n{fail}exit 0\n", log.display()))
            .unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();
        place_exe(&source, &bin.join("claude"));
        std::fs::create_dir_all(home.path().join(".claude/plugins")).unwrap();
        // moai 가 PATH 에서 찾는 것은 `sh` 뿐이다 (`command -v` 로 이름을 찾는다).
        let sys = home.path().join("sysbin");
        std::fs::create_dir_all(&sys).unwrap();
        std::os::unix::fs::symlink("/bin/sh", sys.join("sh")).unwrap();
        Claude { home, bin, log }
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// `claude` 가 남긴 장부를 흉내 낸다.
    fn ledger(&self, name: &str, body: &str) {
        std::fs::write(self.home.path().join(".claude/plugins").join(name), body).unwrap();
    }

    fn run(&self, dir: &Path, args: &[&str], with_claude: bool) -> Output {
        self.command(Path::new(BIN), dir, args, with_claude).output().unwrap()
    }

    /// 부를 moai 와 환경을 더 줄 수 있는 모양. **[`isolated`] 에서 출발해 집과 PATH 만
    /// 제 것으로 덮는다** — 따로 막으면 걷을 목록이 두 벌이 되고, 실제로 이쪽은
    /// `MOAI_ACTOR`·git 환경 변수를 물려받은 채 남아 있었다(moai-0auu).
    fn command(&self, bin: &Path, dir: &Path, args: &[&str], with_claude: bool) -> Command {
        // PATH 에는 가짜 `claude` 와 `sh` 하나만 둔다. `/usr/bin:/bin` 을 통째로 두면
        // 그 자리에 진짜 `claude` 가 깔린 기계에서 시험이 사람의 등록을 부른다.
        let sys = self.home.path().join("sysbin");
        let path = if with_claude {
            format!("{}:{}", self.bin.display(), sys.display())
        } else {
            sys.display().to_string()
        };
        let mut cmd = isolated(bin);
        cmd.args(args).current_dir(dir).env("HOME", self.home.path()).env("PATH", path).env("NO_COLOR", "1");
        cmd
    }
}

/// **가짜 `claude` 를 쓰는 시험도 같은 격리 위에 선다.** 집과 PATH 만 제 것이고,
/// 셸에서 새는 사람·시계·git 변수는 [`isolated`] 가 걷은 그대로 걷혀 있어야 한다.
/// 환경을 실제로 심어 돌리는 대신 짓는 명령의 환경을 읽는다 — 부모 프로세스의 환경을
/// 바꾸면 병렬로 도는 옆 시험이 그것을 본다.
#[test]
fn the_fake_claude_command_starts_from_the_isolated_one() {
    let c = Claude::new("claude-isolated");
    let s = Scratch::new("claude-isolated-dir");
    let cmd = c.command(Path::new(BIN), s.path(), &["skill", "status"], true);
    let envs: std::collections::HashMap<_, _> = cmd.get_envs().collect();
    let removed = |var: &str| matches!(envs.get(std::ffi::OsStr::new(var)), Some(None));
    let set = |var: &str| envs.get(std::ffi::OsStr::new(var)).copied().flatten();

    for var in ["MOAI_ACTOR", "MOAI_NOW", "CLAUDE_CONFIG_DIR", "CLAUDE_CODE_PLUGIN_CACHE_DIR", "XDG_CONFIG_HOME"]
        .iter()
        .chain(GIT_LEAKS)
    {
        assert!(removed(var), "{var} 를 안 걷었다");
    }
    assert_eq!(set("GIT_CONFIG_GLOBAL"), Some(std::ffi::OsStr::new("/dev/null")));
    // 사용자 설정은 공용 빈 집 밑의 **없는** 파일이다. 집을 덮어도 이 자리는 따라가지 않는다.
    let config = Path::new(set("MOAI_CONFIG").expect("MOAI_CONFIG 를 안 줬다"));
    assert!(config.starts_with(env!("CARGO_TARGET_TMPDIR")) && !config.exists(), "사용자 설정이 격리되지 않았다 — {}", config.display());
    assert_eq!(set("HOME"), Some(c.home.path().as_os_str()), "집은 가짜 claude 의 것이어야 한다");
    let path = set("PATH").expect("PATH 를 안 줬다").to_string_lossy().into_owned();
    assert!(path.starts_with(&c.bin.display().to_string()), "가짜 claude 가 PATH 앞에 없다 — {path}");
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// 저장소 하나에 트리를 심고, `claude` 가 적었을 장부를 그 판 `version` 으로 세운다.
/// 설치본 디렉터리에는 트리의 매니페스트를 그대로 복사한다 — `claude` 가 하는 일이다.
fn installed(s: &Scratch, c: &Claude, version: &str) -> (String, PathBuf) {
    let plan = String::from_utf8(c.run(s.path(), &["skill", "install", "--dry-run", "--json"], true).stdout).unwrap();
    let market = field(&plan, "market");
    let dir = PathBuf::from(field(&plan, "dir"));
    let root = dir.parent().unwrap().parent().unwrap().to_path_buf();
    assert!(c.run(s.path(), &["skill", "install"], true).status.success());

    let copy = c.home.path().join(format!(".claude/plugins/cache/{market}/moai/{version}"));
    std::fs::create_dir_all(copy.join(".claude-plugin")).unwrap();
    std::fs::copy(dir.join(".claude-plugin/plugin.json"), copy.join(".claude-plugin/plugin.json")).unwrap();
    // 옛 판 하나가 캐시에 남아 있다.
    std::fs::create_dir_all(copy.parent().unwrap().join("0.0.0")).unwrap();

    c.ledger(
        "known_marketplaces.json",
        &format!("{{\"{market}\":{{\"installLocation\":\"{}\"}}}}", dir.display()),
    );
    c.ledger(
        "installed_plugins.json",
        &format!(
            "{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"{}\",\"version\":\"{version}\"}}]}}}}",
            root.display(),
            copy.display()
        ),
    );
    (market, dir)
}

/// `status` 는 장부를 읽어 **어긋난 판**을 짚는다. 무엇이 어긋나도 0 이다.
#[test]
fn skill_status_names_a_stale_install() {
    let s = init("skillstatus");
    let c = Claude::new("skillstatus-home");
    let (market, _) = installed(&s, &c, "0.0.1");

    let out = c.run(s.path(), &["skill", "status"], true);
    assert!(out.status.success(), "어긋났다고 비영으로 끝났다\n{}", text(&out));
    let said = text(&out);
    assert!(said.contains(&format!("`{market}` 등록됨")), "{said}");
    assert!(said.contains("판 0.0.1") && said.contains("다시 심는다"), "낡은 판을 안 짚는다\n{said}");
    assert!(said.contains("옛 판 1개"), "남은 캐시를 안 센다\n{said}");

    let json = String::from_utf8(c.run(s.path(), &["skill", "status", "--json"], true).stdout).unwrap();
    one_json_value(&json);
    assert!(json.contains("\"current\":false"), "{json}");
    assert!(json.contains("\"hook_exe_found\":true"), "심은 바이너리를 못 찾는다\n{json}");
    assert!(json.contains("\"claude\":true"), "{json}");

    // 판을 맞추면 조용해진다.
    let want = field(&json, "want_version");
    installed(&s, &c, &want);
    let said = text(&c.run(s.path(), &["skill", "status"], true));
    assert!(!said.contains("다시 심는다"), "맞는 판인데 다시 심으라 한다\n{said}");
}

/// 아무것도 안 심긴 기계에서도 `status` 는 멀쩡히 끝난다 — 설정 없는 기계에서
/// 도구가 고장 난 것으로 보이면 안 된다.
#[test]
fn skill_status_on_a_bare_machine_is_quiet_and_fine() {
    let s = init("skillbare");
    let c = Claude::new("skillbare-home");
    let out = c.run(s.path(), &["skill", "status"], false);
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(said.contains("등록 안 됨") && said.contains("PATH 에 없다"), "{said}");
}

/// 설치본이 부르는 바이너리가 사라지면 그것을 짚는다. 훅은 그때 조용히
/// 아무것도 안 하므로, 여기 말고는 알 길이 없다.
#[test]
fn skill_status_notices_a_vanished_hook_binary() {
    let s = init("skillgone");
    let c = Claude::new("skillgone-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    let manifest = c.home.path().join(format!(".claude/plugins/cache/{market}/moai/0.0.1/.claude-plugin/plugin.json"));
    let body = std::fs::read_to_string(&manifest).unwrap().replace(BIN, "/nowhere/moai");
    std::fs::write(&manifest, body).unwrap();

    let said = text(&c.run(s.path(), &["skill", "status"], true));
    assert!(said.contains("/nowhere/moai") && said.contains("없다"), "{said}");

    // **파일은 있어도 실행할 수 없으면** 훅은 권한 오류를 삼키고 아무것도 안 한다.
    let noexec = c.home.path().join("noexec-moai");
    std::fs::write(&noexec, "#!/bin/sh\n").unwrap();
    let body = std::fs::read_to_string(&manifest).unwrap().replace("/nowhere/moai", &noexec.display().to_string());
    std::fs::write(&manifest, body).unwrap();
    let json = String::from_utf8(c.run(s.path(), &["skill", "status", "--json"], true).stdout).unwrap();
    assert!(json.contains("\"hook_exe_found\":false"), "실행할 수 없는 훅을 있다고 한다\n{json}");
}

/// **판은 그 설치의 훅이 부르는 것으로 견준다.** 같은 moai 를 다른 자리에서 부른
/// 것만으로 "다시 심는다" 가 뜨면, 시킨 대로 한 사람의 훅이 그 자리를 부르게 바뀐다.
#[test]
fn skill_status_from_another_binary_keeps_a_current_install_current() {
    let s = init("skillotherbin");
    let c = Claude::new("skillotherbin-home");
    installed(&s, &c, "0.0.1");
    let json = String::from_utf8(c.run(s.path(), &["skill", "status", "--json"], true).stdout).unwrap();
    installed(&s, &c, &field(&json, "want_version"));

    let copy = c.home.path().join("otherbin/moai");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    place_exe(Path::new(BIN), &copy);
    let out = c.command(&copy, s.path(), &["skill", "status"], true).output().unwrap();
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(!said.contains("다시 심는다"), "같은 내용인데 다시 심으라 한다\n{said}");
    assert!(said.contains("훅이 부르는 것과 다르다"), "다른 moai 로 불렀다는 말이 없다\n{said}");
}

/// **등록이 남의 자리를 가리키면 다시 심으라고 하지 않는다.** `install` 은 그때
/// 등록을 건너뛰어, 시킨 대로 해도 아무것도 안 바뀐다 — 어느 줄에서든 같은 덫이다.
#[test]
fn skill_status_under_a_clash_offers_no_reinstall() {
    let s = init("skillclashstatus");
    let c = Claude::new("skillclashstatus-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    c.ledger("known_marketplaces.json", &format!("{{\"{market}\":{{\"installLocation\":\"/elsewhere\"}}}}"));
    std::fs::remove_dir_all(c.home.path().join(format!(".claude/plugins/cache/{market}"))).unwrap();

    let said = text(&c.run(s.path(), &["skill", "status"], true));
    assert!(!said.contains("moai skill install --scope"), "헛도는 명령을 일러 준다\n{said}");
    assert!(said.contains(&format!("claude plugin marketplace remove {market}")), "빠져나갈 길이 없다\n{said}");

    c.ledger("installed_plugins.json", "{\"version\":2,\"plugins\":{}}");
    let said = text(&c.run(s.path(), &["skill", "status"], true));
    assert!(!said.contains("`moai skill install` 로 심는다"), "헛도는 명령을 일러 준다\n{said}");
}

/// `claude` 는 `CLAUDE_CONFIG_DIR` 이 있으면 장부를 거기 둔다. `HOME` 만 보던 판은
/// 그 사람에게 "걷어낼 것이 없다" 고 말하고 0 으로 끝났다 — 훅은 그대로 살아 있는데.
#[test]
fn skill_reads_the_ledger_where_claude_keeps_it() {
    let s = init("skillcfgdir");
    let c = Claude::new("skillcfgdir-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    let cfg = c.home.path().join("elsewhere-config");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::rename(c.home.path().join(".claude/plugins"), cfg.join("plugins")).unwrap();
    std::fs::create_dir_all(c.home.path().join(".claude/plugins")).unwrap();

    let status = c.command(Path::new(BIN), s.path(), &["skill", "status", "--json"], true)
        .env("CLAUDE_CONFIG_DIR", &cfg)
        .output()
        .unwrap();
    let json = String::from_utf8(status.stdout).unwrap();
    assert!(!json.contains("\"installs\":[]"), "옮긴 장부를 못 읽는다\n{json}");

    let before = c.calls();
    let out = c.command(Path::new(BIN), s.path(), &["skill", "uninstall"], true)
        .env("CLAUDE_CONFIG_DIR", &cfg)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before.len()..].to_string();
    assert!(calls.contains(&format!("plugin uninstall moai@{market} --scope local")), "걷을 것을 못 찾았다\n{calls}");
}

/// `claude` 의 캐시에서 설치본이 사라지면 그것을 짚는다. 장부의 판이 맞아도
/// 훅은 아예 안 실린다 — 매니페스트를 못 읽었다고 조용히 넘기면 `·` 만 남는다.
#[test]
fn skill_status_notices_a_vanished_install_copy() {
    let s = init("skillnocopy");
    let c = Claude::new("skillnocopy-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    let json = String::from_utf8(c.run(s.path(), &["skill", "status", "--json"], true).stdout).unwrap();
    installed(&s, &c, &field(&json, "want_version"));
    let copy = c.home.path().join(format!(".claude/plugins/cache/{market}/moai"));
    std::fs::remove_dir_all(&copy).unwrap();

    let out = c.run(s.path(), &["skill", "status"], true);
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(said.contains("! 설치") && said.contains("설치본"), "사라진 설치본을 안 짚는다\n{said}");
    let json = String::from_utf8(c.run(s.path(), &["skill", "status", "--json"], true).stdout).unwrap();
    assert!(json.contains("\"copy_found\":false"), "{json}");
}

/// `uninstall` 은 범위마다 걷고 마켓플레이스를 지운다. **파일은 남긴다** —
/// 돌고 있는 세션이 물고 있을 수 있다.
#[test]
fn skill_uninstall_asks_claude_and_keeps_the_files() {
    let s = init("skillrm");
    let c = Claude::new("skillrm-home");
    let (market, dir) = installed(&s, &c, "0.0.1");
    let before = c.calls();

    let rehearsal = c.run(s.path(), &["skill", "uninstall", "--dry-run"], true);
    assert!(rehearsal.status.success(), "{}", text(&rehearsal));
    assert_eq!(c.calls(), before, "연습인데 claude 를 불렀다");
    assert!(text(&rehearsal).contains(&format!("claude plugin uninstall moai@{market} --scope local")));

    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before.len()..].to_string();
    assert!(calls.contains(&format!("plugin uninstall moai@{market} --scope local")), "{calls}");
    assert!(calls.contains(&format!("plugin marketplace remove {market}")), "{calls}");
    assert!(dir.join("skills/moai/SKILL.md").is_file(), "심은 파일을 지웠다");
    assert!(text(&out).contains("다시 열어야"), "열린 세션에 대해 말하지 않는다\n{}", text(&out));
}

/// **플러그인을 못 걷으면 마켓플레이스도 안 지운다.** 지우면 걷을 이름이 사라진
/// 설치가 남아 도구로 되돌릴 길이 없다. 첫 줄도 "걷었다" 고 말하지 않는다.
#[test]
fn skill_uninstall_stops_at_the_first_failure() {
    let s = init("skillrmfail");
    let c = Claude::failing_on("skillrmfail-home", Some("plugin uninstall"));
    let (market, _) = installed(&s, &c, "0.0.1");
    let before = c.calls().len();

    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(!out.status.success(), "실패했는데 0 으로 끝났다");
    let calls = c.calls()[before..].to_string();
    assert!(calls.contains("plugin uninstall"), "{calls}");
    assert!(!calls.contains("marketplace remove"), "못 걷었는데 마켓플레이스를 지웠다\n{calls}");
    let said = text(&out);
    assert!(said.starts_with(&format!("! `{market}` 을 다 걷지 못했다")), "{said}");
    assert!(said.contains("안 불렀다"), "안 부른 걸음을 안 밝힌다\n{said}");
}

/// **범위마다 한 번만 걷는다.** 장부에 같은 범위 줄이 겹치면 같은 걷기를 두 번
/// 부르고, 둘째가 이미 걷힌 것에 실패해 첫 실패에서 멈춘다 — 마켓플레이스가 안
/// 지워진 채 남는다.
#[test]
fn skill_uninstall_calls_each_scope_once() {
    let s = init("skillrmdup");
    let c = Claude::new("skillrmdup-home");
    let (market, dir) = installed(&s, &c, "0.0.1");
    let root = dir.parent().unwrap().parent().unwrap();
    let copy = c.home.path().join(format!(".claude/plugins/cache/{market}/moai/0.0.1"));
    let row = format!(
        "{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"{}\",\"version\":\"0.0.1\"}}",
        root.display(),
        copy.display()
    );
    c.ledger("installed_plugins.json", &format!("{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{row},{row}]}}}}"));
    let before = c.calls().len();

    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before..].to_string();
    assert_eq!(calls.matches("plugin uninstall").count(), 1, "같은 범위를 두 번 걷는다\n{calls}");
    assert!(calls.contains(&format!("plugin marketplace remove {market}")), "{calls}");
}

/// **같은 이름이 남의 자리를 가리키면 아무것도 부르지 않는다.** 걷으면 그
/// 저장소의 규칙이 말없이 사라진다.
#[test]
fn skill_uninstall_leaves_another_repos_registration_alone() {
    let s = init("skillclash");
    let c = Claude::new("skillclash-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    c.ledger("known_marketplaces.json", &format!("{{\"{market}\":{{\"installLocation\":\"/elsewhere\"}}}}"));
    let before = c.calls();

    let out = c.run(s.path(), &["skill", "uninstall", "--json"], true);
    assert!(!out.status.success(), "못 걷었는데 성공으로 끝났다");
    let json = String::from_utf8(out.stdout).unwrap();
    assert!(json.contains("\"blocked_by\":\"/elsewhere\""), "{json}");
    assert_eq!(c.calls(), before, "남의 등록인데 claude 를 불렀다");
}

/// **등록을 못 했으면 심기도 비영으로 끝난다.** 파일은 심었어도 훅은 안 선다 —
/// `uninstall` 이 같은 반쪽 상태를 실패로 끝내는 것과 같은 셈이다. 연습은 같은
/// 자리에서 등록을 건너뛸 것을 미리 말한다.
#[test]
fn skill_install_without_registration_is_not_a_success() {
    let s = init("skillinstallpartial");
    let c = Claude::new("skillinstallpartial-home");
    let dir = field(&String::from_utf8(c.run(s.path(), &["skill", "install", "--dry-run", "--json"], false).stdout).unwrap(), "dir");
    // **하위 디렉터리에서 불러도 옳은 자리를 댄다.** 뿌리 기준의 상대 경로를 내면
    // 그 자리에서 친 줄이 없는 디렉터리를 가리킨다.
    let sub = s.path().join("src");
    std::fs::create_dir_all(&sub).unwrap();
    let out = c.run(&sub, &["skill", "install"], false);
    assert!(!out.status.success(), "등록을 못 했는데 성공으로 끝났다\n{}", text(&out));
    assert!(text(&out).contains("손으로 마친다"), "마칠 길을 안 낸다\n{}", text(&out));
    assert!(
        text(&out).contains(&format!("claude plugin marketplace add {dir} --scope local")),
        "마칠 길이 부른 자리에 따라 틀린다\n{}",
        text(&out)
    );

    let (market, _) = installed(&s, &c, "0.0.1");
    c.ledger("known_marketplaces.json", &format!("{{\"{market}\":{{\"installLocation\":\"/elsewhere\"}}}}"));
    let plan = String::from_utf8(c.run(s.path(), &["skill", "install", "--dry-run", "--json"], true).stdout).unwrap();
    assert!(plan.contains("\"blocked_by\":\"/elsewhere\""), "연습이 건너뛸 등록을 약속한다\n{plan}");
    let out = c.run(s.path(), &["skill", "install", "--scope", "user"], true);
    assert!(!out.status.success(), "등록을 건너뛰었는데 성공으로 끝났다");
    assert!(!text(&out).contains("--scope user"), "헛도는 범위 바꾸기를 일러 준다\n{}", text(&out));
}

/// `claude` 가 없으면 부를 명령을 내고 비영으로 끝난다. **절반을 해 놓고
/// 아무 말 없이 성공하는 것이 제일 나쁘다.**
#[test]
fn skill_uninstall_without_claude_says_what_to_run() {
    let s = init("skillnoclaude");
    let c = Claude::new("skillnoclaude-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    let out = c.run(s.path(), &["skill", "uninstall"], false);
    assert!(!out.status.success(), "아무것도 못 했는데 성공으로 끝났다");
    assert!(text(&out).contains(&format!("claude plugin marketplace remove {market}")), "{}", text(&out));
}

// ── 메모 — 긴 글을 남기는 길 ────────────────────────────────────────

/// 리뷰 전문처럼 **긴 글**은 stdin 으로 들어간다.
///
/// 이 길이 없어서 리뷰가 낸 글이 매번 요약만 남고 버려졌다. 명령줄에 5천 자를
/// 욱여넣을 수는 없고, 넣더라도 그 명령줄은 규칙이 읽는 바로 그 글이다.
#[test]
fn a_long_note_comes_from_stdin() {
    let s = init("notebody");
    let id = field(&ok(s.path(), &["add", "이슈", "--json"]), "id");
    let long: String =
        (0..200).map(|n| format!("리뷰가 낸 {n}번째 줄\n")).collect::<Vec<_>>().concat();

    let out = from_stdin(s.path(), &["note", &id, "-b", "-"], &long);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // 저널에 **통째로** 들어간다 — 잘리면 원문을 남기는 뜻이 없다.
    let shown = ok(s.path(), &["show", &id]);
    assert!(shown.contains("리뷰가 낸 0번째 줄"), "첫 줄이 없다");
    assert!(shown.contains("리뷰가 낸 199번째 줄"), "마지막 줄이 없다\n{shown}");
}

/// 짧은 발견은 그대로 자리 인자로 남는다. 있던 길을 막지 않는다.
#[test]
fn a_short_note_still_takes_a_bare_word() {
    let s = init("noteshort");
    let id = field(&ok(s.path(), &["add", "이슈", "--json"]), "id");
    let out = ok(s.path(), &["note", &id, "파서가 BOM 에서 죽는다"]);
    assert!(out.contains("파서가 BOM 에서 죽는다"), "{out}");
}

/// **둘은 서로 밀어낸다.** 둘 다 받으면 어느 쪽이 이기는지 아무도 못 외우고,
/// 외우지 못하는 규칙은 언젠가 남의 글을 지운다.
#[test]
fn the_two_ways_to_write_a_note_push_each_other_out() {
    let s = init("notepush");
    let id = field(&ok(s.path(), &["add", "이슈", "--json"]), "id");
    let out = moai(s.path(), &["note", &id, "가", "-b", "나"]);
    assert!(!out.status.success(), "둘 다 받았다");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("cannot be used with"), "무엇이 부딪혔는지 안 말한다 — {said}");
}

/// **"안 줬다" 와 "준 자리가 비었다" 는 다른 말이다.** 파일을 잘못 짚어 빈
/// stdin 이 들어온 사람에게 "안 줬다" 고 하면, 제가 준 것을 도구가 못 본 줄
/// 알고 같은 명령을 다시 친다.
#[test]
fn an_empty_note_is_told_apart_from_a_missing_one() {
    let s = init("noteempty");
    let id = field(&ok(s.path(), &["add", "이슈", "--json"]), "id");

    let missing = moai(s.path(), &["note", &id]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("안 줬다"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let blank = from_stdin(s.path(), &["note", &id, "-b", "-"], "   \n \n");
    assert!(!blank.status.success(), "빈 메모가 들어갔다");
    assert!(
        String::from_utf8_lossy(&blank.stderr).contains("비었다"),
        "{}",
        String::from_utf8_lossy(&blank.stderr)
    );

    // 둘 다 저널에 아무것도 안 남겼다.
    let shown = ok(s.path(), &["show", &id]);
    assert_eq!(shown.matches("note:").count(), 0, "{shown}");
}

// ── 워크트리 함께 보기 ────────────────────────────────────────────────

/// 사람의 git 설정 없이 git 을 돌린다. 커밋에 이름이 필요하니 여기서 준다.
fn git(dir: &Path, args: &[&str]) -> String {
    // `isolated` 로 띄운다 — git 훅 안에서 시험이 돌 때 물려받은 `GIT_DIR` 이 남으면
    // 여기서의 `git commit` 이 바깥 저장소에 떨어진다.
    let out = isolated("git")
        .args(["-c", "user.name=테스터", "-c", "user.email=tester@example.com", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git 을 실행하지 못했다");
    assert!(out.status.success(), "git {args:?} 가 실패했다\n{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// 시계를 달리 두고 돌린다 — 옆 워크트리의 쓰기가 **더 늦게** 떨어진 것을 흉내 낸다.
fn ok_at(dir: &Path, now: &str, args: &[&str]) -> String {
    let out = isolated(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_ACTOR", "테스터 (tester@example.com)")
        .env("MOAI_NOW", now)
        .env("NO_COLOR", "1")
        .output()
        .expect("moai 를 실행하지 못했다");
    assert!(out.status.success(), "moai {args:?} 가 실패했다\n{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

const LATER: &str = "2026-09-12T00:00:00Z";

struct Trees {
    s: Scratch,
    epic: String,
    /// 옆에서 집은 일.
    picked: String,
    /// 양쪽이 **같은 시각에** 고친 일 — 동률.
    tied: String,
    /// 옆에서만 만든 일.
    made: String,
}

impl Trees {
    fn main(&self) -> PathBuf {
        self.s.path().join("main")
    }
    fn feat(&self) -> PathBuf {
        self.s.path().join("feat")
    }
}

/// `main` 과 `feat/x` 워크트리. 둘은 같은 스냅샷에서 갈라졌고, 옆에서는
/// 하나를 집고(늦게), 하나를 같은 시각에 고치고, 하나를 새로 만들었다.
/// moai 를 들이기 **전** 커밋에서 갈라진 `old` 워크트리도 하나 둔다.
fn trees(name: &str) -> Trees {
    let s = Scratch::new(name);
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "moai 이전"]);
    ok(&main, &["init", "argos"]);
    let epic = field(&ok(&main, &["epic", "add", "에픽", "--json"]), "id");
    let picked = field(&ok(&main, &["add", "집을 일", "-e", &epic, "--json"]), "id");
    let tied = field(&ok(&main, &["add", "여기 제목", "-e", &epic, "--json"]), "id");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "init"]);
    git(&main, &["worktree", "add", "-q", "../feat", "-b", "feat/x"]);
    git(&main, &["worktree", "add", "-q", "../old", "-b", "old", "HEAD~1"]);

    let feat = s.path().join("feat");
    ok_at(&feat, LATER, &["mv", &picked, "in_progress"]);
    ok(&feat, &["edit", &tied, "--title", "옆 제목"]);
    let made = field(&ok_at(&feat, LATER, &["add", "옆에서 만든 일", "-e", &epic, "--json"]), "id");
    Trees { s, epic, picked, tied, made }
}

/// 옆에서 늦게 옮긴 줄은 **그 줄로** 서고 `⎇ <브랜치>` 를 단다. 같은 시각이면 지금
/// 브랜치의 줄이다. 옆에만 있는 줄도 선다. 플래그 없이는 전과 똑같다.
#[test]
fn worktree_overlays_the_latest_line_and_names_its_branch() {
    let t = trees("wtlist");
    let main = t.main();

    let plain = ok(&main, &["show"]);
    assert!(!plain.contains('⎇') && !plain.contains(&t.made), "플래그 없이 겹쳤다\n{plain}");

    let shown = ok(&main, &["show", "--worktree"]);
    let line = |id: &str| shown.lines().find(|l| l.starts_with(id)).unwrap_or_else(|| panic!("{id} 가 없다\n{shown}")).to_string();
    assert!(line(&t.picked).contains("▸  ⎇ feat/x 집을 일"), "늦은 옆 줄이 안 섰다\n{shown}");
    assert!(line(&t.made).contains("⎇ feat/x 옆에서 만든 일"), "옆에만 있는 줄이 없다\n{shown}");
    let tied = line(&t.tied);
    assert!(tied.contains("여기 제목") && !tied.contains('⎇'), "동률에 지금 브랜치 줄이 안 섰다\n{shown}");

    let rows = ok(&main, &["show", "--worktree", "--json"]);
    let row = |id: &str| rows.split("},{").find(|r| r.contains(&format!("\"id\":\"{id}\""))).unwrap().to_string();
    assert!(row(&t.picked).contains("\"branch\":\"feat/x\""), "{rows}");
    assert!(!row(&t.tied).contains("\"branch\""), "지금 브랜치 줄에 branch 키가 붙었다\n{rows}");
    // 트리도 같은 머리표를 단다.
    let tree = ok(&main, &["show", "--tree", "--worktree"]);
    assert!(tree.contains("⎇ feat/x 옆에서 만든 일"), "{tree}");
}

/// `ready` 는 옆에서 이미 집은 일을 **집을 수 있다고 내지 않고**, 잡고 있는 것에
/// 어느 워크트리에서 잡았는지 댄다. `status` 는 겹쳐 봤다고 머리에서 말한다.
#[test]
fn worktree_keeps_ready_from_offering_what_another_worktree_picked() {
    let t = trees("wtready");
    let main = t.main();

    let before = ok(&main, &["ready"]);
    assert!(before.contains(&t.picked), "{before}");

    let ready = ok(&main, &["ready", "--worktree"]);
    let picks: String = ready.lines().take_while(|l| !l.starts_with('!')).collect::<Vec<_>>().join("\n");
    assert!(!picks.contains(&t.picked), "옆에서 집은 일을 집으라고 낸다\n{ready}");
    assert!(ready.contains(&format!("{} ⎇ feat/x", t.picked)), "누가 어디서 잡았는지 안 댄다\n{ready}");
    assert!(picks.contains(&t.made), "{ready}");

    let json = ok(&main, &["ready", "--worktree", "--json"]);
    assert!(!json.contains(&t.picked), "{json}");
    assert!(json.contains("\"branch\":\"feat/x\""), "{json}");

    let status = ok(&main, &["status", "--worktree"]);
    // 스냅샷 없는 `old` 는 겹친 것이 아니라 이름에도 안 선다.
    let head = status.lines().next().unwrap();
    assert!(head.contains("⎇ feat/x 겹쳐 봄") && !head.contains("old"), "{status}");
    let st = ok(&main, &["status", "--worktree", "--json"]);
    assert!(st.contains(&format!("\"branches\":{{\"{}\":\"feat/x\",\"{}\":\"feat/x\"}}", t.made, t.picked))
        || st.contains(&format!("\"branches\":{{\"{}\":\"feat/x\",\"{}\":\"feat/x\"}}", t.picked, t.made)), "{st}");
    assert!(!ok(&main, &["status", "--json"]).contains("branches"), "플래그 없이 branches 키가 붙었다");

    // 옆에서 온 줄의 이력은 **그 워크트리의 저널**에서 읽는다.
    let one = ok(&main, &["show", &t.picked, "--worktree"]);
    assert!(one.contains("⎇ feat/x") && one.contains("todo → in_progress"), "{one}");
    let _ = &t.epic;
}

/// **보여줄 때만 겹친다.** 어느 워크트리의 파일도 한 바이트 안 바뀐다. 스냅샷 없는
/// 워크트리는 말하지 않는다.
#[test]
fn worktree_writes_nothing_and_skips_trees_without_a_snapshot() {
    let t = trees("wtquiet");
    let files = |d: &Path| (issues(d), std::fs::read_to_string(d.join(".moai/journal.jsonl")).unwrap());
    let (m0, f0) = (files(&t.main()), files(&t.feat()));
    for args in [
        vec!["status", "--worktree"],
        vec!["ready", "--worktree"],
        vec!["show", "--worktree"],
        vec!["show", "--tree", "--worktree"],
        vec!["show", &t.picked, "--worktree"],
    ] {
        let out = moai(&t.main(), &args);
        assert!(out.status.success(), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.is_empty(), "스냅샷 없는 워크트리를 두고 말했다 — {args:?}\n{err}");
    }
    assert_eq!(files(&t.main()), m0, "지금 워크트리 파일이 바뀌었다");
    assert_eq!(files(&t.feat()), f0, "옆 워크트리 파일이 바뀌었다");
}

/// 옆 파일이 깨졌어도, git 저장소가 아니어도 **막지 않는다** — 말만 하고 0 으로 끝난다.
#[test]
fn worktree_trouble_is_told_but_never_fails_the_command() {
    let t = trees("wtbroken");
    let feat = t.feat().join(".moai/issues.jsonl");
    let mut src = std::fs::read_to_string(&feat).unwrap();
    src.push_str("{깨진 줄\n");
    std::fs::write(&feat, src).unwrap();

    let out = moai(&t.main(), &["status", "--worktree"]);
    assert!(out.status.success(), "옆 파일 때문에 status 가 실패했다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("⎇ feat/x") && err.contains("읽을 수 없는 줄 1개"), "{err}");
    // 보드는 stderr 의 말을 "문제 없다" 로 뒤집지 않는다(moai-cuw2).
    let board = String::from_utf8_lossy(&out.stdout);
    assert!(!board.contains("드러난 문제 없다") && board.contains("옆 워크트리 문제 1건"), "{board}");
    let shown = ok(&t.main(), &["show", "--worktree"]);
    assert!(shown.contains("⎇ feat/x 옆에서 만든 일"), "깨진 줄 말고 나머지도 버렸다\n{shown}");

    let bare = init("wtnogit");
    let out = moai(bare.path(), &["status", "--worktree"]);
    assert!(out.status.success(), "git 저장소가 아니라고 실패했다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("워크트리를 못 찾았다"));
    let board = String::from_utf8_lossy(&out.stdout);
    assert!(!board.contains("드러난 문제 없다") && board.contains("옆 워크트리 문제 1건"), "{board}");
    // 겹쳐 보라고 안 시켰으면 옆을 찾지도 않으니 문제도 없다.
    assert!(ok(bare.path(), &["status"]).contains("드러난 문제 없다"));
}

/// **여기서 지운 줄은 옆 줄로 되살아나지 않는다**(moai-0a0u). 갈라진 뒤 옆에서 안
/// 만진 줄은 사라지고, 옆에서 집은 줄은 지웠어도 선다 — 부딪힌 것을 감추면 옆에서
/// 하던 일이 안 보인다. 옆에서 새로 만든 줄은 전처럼 선다.
#[test]
fn worktree_does_not_revive_a_line_removed_here() {
    let t = trees("wtrm");
    let main = t.main();
    ok(&main, &["rm", &t.tied, &t.picked]);

    let shown = ok(&main, &["show", "--worktree"]);
    assert!(!shown.contains(&t.tied), "여기서 지운 줄이 옆 줄로 되살았다\n{shown}");
    assert!(shown.contains(&t.made), "{shown}");
    let picked = shown.lines().find(|l| l.starts_with(t.picked.as_str()));
    assert!(picked.is_some_and(|l| l.contains("⎇ feat/x")), "옆에서 집은 일이 사라졌다\n{shown}");

    let ready = ok(&main, &["ready", "--worktree", "--json"]);
    assert!(!ready.contains(&t.tied), "지운 일을 집으라고 낸다\n{ready}");
    let status = ok(&main, &["status", "--worktree", "--json"]);
    assert!(!status.contains(&t.tied), "{status}");
}

/// 옆에서 **늦게 미루거나 도로 집은 것**은 여기서 먼저 옮긴 칸에 가려지지 않는다(moai-l11z).
/// 미루기는 칸을 안 옮기지만 `planned_at` 이 그 때를 적는다 — 도로 집어 `deferred_at` 이
/// 지워져도 남는다. (moai-4i82 에서 e9 세션이 짠 시험을 받았다.)
#[test]
fn worktree_a_later_defer_or_undo_there_is_not_hidden_by_an_earlier_move_here() {
    let t = trees("wtdefer");
    let (main, feat) = (t.main(), t.feat());
    ok_at(&main, LATER, &["mv", &t.tied, "in_progress"]);
    ok_at(&feat, "2026-09-13T00:00:00Z", &["defer", &t.tied]);
    assert!(issues(&feat).contains("\"planned_at\":\"2026-09-13T00:00:00Z\""), "{}", issues(&feat));
    let deferred = ok(&main, &["show", "--deferred", "--worktree"]);
    let line = deferred.lines().find(|l| l.starts_with(t.tied.as_str())).unwrap_or_default();
    assert!(line.contains("⎇ feat/x"), "옆의 늦은 defer 가 가려졌다\n{deferred}");

    ok_at(&main, "2026-09-14T00:00:00Z", &["defer", &t.tied]);
    ok_at(&feat, "2026-09-15T00:00:00Z", &["defer", &t.tied, "--undo"]);
    let deferred = ok(&main, &["show", "--deferred", "--worktree"]);
    assert!(!deferred.lines().any(|l| l.starts_with(t.tied.as_str())), "옆의 늦은 --undo 가 가려졌다\n{deferred}");
    // 도로 집은 줄도 그 때를 든다 — 이것이 없으면 위의 셈이 저널을 접어야 한다.
    assert!(issues(&feat).contains("\"planned_at\":\"2026-09-15T00:00:00Z\""), "{}", issues(&feat));
}

/// 옆에서 집은 뒤 **여기서 필드만 늦게 고쳐도** 옆에서 집은 것이 풀리지 않는다
/// (moai-2f5g). 겹치는 규칙이 칸을 옮긴 시각을 먼저 본다.
#[test]
fn worktree_a_later_field_edit_here_does_not_unpick_what_another_worktree_picked() {
    let t = trees("wtedit");
    let main = t.main();
    ok_at(&main, "2026-09-13T00:00:00Z", &["edit", &t.picked, "-p", "0"]);

    let ready = ok(&main, &["ready", "--worktree", "--json"]);
    assert!(ready.contains("\"branch\":\"feat/x\""), "{ready}");
    let picks = ok(&main, &["ready", "--worktree"]);
    let offered: String = picks.lines().take_while(|l| !l.starts_with('!')).collect::<Vec<_>>().join("\n");
    assert!(!offered.contains(&t.picked), "여기서 우선순위만 고쳤는데 옆에서 집은 일을 집으라고 낸다\n{picks}");
    let shown = ok(&main, &["show", "--worktree"]);
    let line = shown.lines().find(|l| l.starts_with(t.picked.as_str())).unwrap_or_default();
    assert!(line.contains("▸  ⎇ feat/x"), "옆에서 집은 줄이 안 섰다\n{shown}");
}

/// **`.moai` 밖 한눈 보기도 `--worktree` 로 프로젝트마다 겹친다**(moai-x0gb). 옆에서 집은
/// 일은 `ready` 에서 빠지고 집은 것에 `⎇` 와 함께 서며, 머리가 겹쳐 봤다고 말한다. 옆
/// 파일이 깨졌으면 stderr 가 아니라 그 프로젝트의 줄이 말하고 0 으로 끝난다.
#[test]
fn outside_a_repo_the_overview_overlays_each_projects_worktrees() {
    let t = trees("ovwt");
    let out = dir_in(&t.s, "out");
    let cfg = registry(&t.s, &[&t.main()]);

    let plain = ok_with(&out, &cfg, &["status"]);
    assert!(!plain.contains('⎇'), "플래그 없이 겹쳤다\n{plain}");

    let run = moai_with(&out, &cfg, &["status", "--worktree"]);
    assert!(run.status.success());
    assert!(run.stderr.is_empty(), "stderr 로 말했다 — {}", String::from_utf8_lossy(&run.stderr));
    let st = String::from_utf8(run.stdout).unwrap();
    let b = block(&st, "main");
    assert!(b.lines().next().unwrap().contains("⎇ feat/x 겹쳐 봄"), "머리가 겹쳐 봤다고 안 한다\n{st}");
    assert!(b.contains("in_progress 1") && b.contains("⎇ feat/x 집을 일"), "옆에서 집은 것이 안 섰다\n{st}");

    let rd = ok_with(&out, &cfg, &["ready", "--worktree"]);
    let b = block(&rd, "main");
    assert!(!b.contains(&t.picked), "옆에서 집은 일을 집으라고 낸다\n{rd}");
    assert!(b.contains("⎇ feat/x 옆에서 만든 일"), "{rd}");
    let json = ok_with(&out, &cfg, &["ready", "--worktree", "--json"]);
    assert!(json.contains(&format!("\"id\":\"{}\"", t.made)) && json.contains("\"branch\":\"feat/x\""), "{json}");
    assert!(!json.contains("\"trouble\""), "문제가 없는데 trouble 키가 섰다\n{json}");

    let feat = t.feat().join(".moai/issues.jsonl");
    let mut src = std::fs::read_to_string(&feat).unwrap();
    src.push_str("{깨진 줄\n");
    std::fs::write(&feat, src).unwrap();
    let run = moai_with(&out, &cfg, &["status", "--worktree"]);
    assert!(run.status.success(), "옆 워크트리 때문에 한눈 보기가 실패했다");
    assert!(run.stderr.is_empty(), "{}", String::from_utf8_lossy(&run.stderr));
    let st = String::from_utf8(run.stdout).unwrap();
    assert!(block(&st, "main").contains("! ⎇ feat/x") && st.contains("읽을 수 없는 줄 1개"), "그 프로젝트 줄에서 말하지 않는다\n{st}");
    let b = block(&st, "main");
    assert!(!b.contains("드러난 문제 없다") && b.contains("옆 워크트리 문제 1건"), "위에서 문제를 말하고 밑에서 문제 없다고 한다\n{st}");
    let json = ok_with(&out, &cfg, &["status", "--worktree", "--json"]);
    assert!(json.contains("\"trouble\":[\"⎇ feat/x"), "{json}");
}

// ── moai project ────────────────────────────────────────────────────────────

/// **등록 시험은 저마다 제 설정 파일을 쓴다.** [`isolated`] 의 `MOAI_CONFIG` 는
/// 모든 시험이 함께 쓰는 빈 집 밑이라, 거기에 등록하면 병렬로 도는 시험끼리
/// 목록이 섞이고 공용 집이 더는 비어 있지 않다(moai-88l8.5ik). 사람도 안 준다 —
/// 이력이 남는 파일이 아니라 누군지 묻지 않아야 한다.
fn project(dir: &Path, config: &Path, args: &[&str]) -> Output {
    isolated(BIN)
        .args(args)
        .current_dir(dir)
        .env("MOAI_CONFIG", config)
        .env("NO_COLOR", "1")
        .output()
        .expect("moai 를 실행하지 못했다")
}

fn project_ok(dir: &Path, config: &Path, args: &[&str]) -> String {
    let out = project(dir, config, args);
    assert!(out.status.success(), "moai {args:?} 가 실패했다\n{}", text(&out));
    String::from_utf8(out.stdout).unwrap()
}

/// `.moai` 밖에서 더하고 보고 뺀다. 뺀 뒤에도 디렉터리와 그 `.moai` 는 그대로다.
#[test]
fn project_add_ls_rm_round_trip_outside_any_moai() {
    let home = Scratch::new("project-roundtrip");
    let config = home.path().join("cfg/moai/config.toml");
    let argos = init("project-argos");
    let bare = home.path().join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let argos_real = argos.path().canonicalize().unwrap();
    let bare_real = bare.canonicalize().unwrap();

    let added = project_ok(home.path(), &config, &["project", "add", argos.path().to_str().unwrap()]);
    assert!(added.contains("등록함") && !added.contains("init 전"), "{added}");
    // 상대경로는 지금 자리에 붙고, `.moai` 가 없어도 받되 그렇다고 말한다.
    let added = project_ok(home.path(), &config, &["project", "add", "bare"]);
    assert!(added.contains("등록함") && added.contains("init 전"), "{added}");
    let written = std::fs::read_to_string(&config).unwrap();
    assert!(written.contains(&format!("path = \"{}\"", bare_real.display())), "절대경로로 적지 않았다\n{written}");

    let ls = project_ok(home.path(), &config, &["project", "ls"]);
    let lines: Vec<&str> = ls.lines().collect();
    assert!(lines[0].contains(&argos_real.display().to_string()) && lines[0].ends_with("✓ done 0"), "{ls}");
    assert!(lines[1].starts_with("bare ") && lines[1].ends_with("init 전"), "{ls}");

    let json = project_ok(home.path(), &config, &["project", "ls", "--json"]);
    one_json_value(&json);
    assert!(json.contains(&format!("\"config\":\"{}\"", config.display())), "{json}");
    assert!(json.contains("\"state\":\"initialized\",\"counts\":{"), "{json}");
    assert!(
        json.contains(&format!(
            "{{\"name\":\"bare\",\"path\":\"{}\",\"state\":\"uninitialized\"}}",
            bare_real.display()
        )),
        "{json}"
    );
    assert!(json.ends_with("\"problems\":[]}\n"), "{json}");

    let gone = project_ok(home.path(), &config, &["project", "rm", "bare"]);
    assert!(gone.contains("뺌") && gone.contains(&bare_real.display().to_string()), "{gone}");
    let rm_json = project_ok(home.path(), &config, &["project", "rm", argos.path().to_str().unwrap(), "--json"]);
    one_json_value(&rm_json);
    assert!(rm_json.contains(&format!("\"removed\":[\"{}\"]", argos_real.display())), "{rm_json}");
    assert!(bare.is_dir() && argos.path().join(".moai/issues.jsonl").is_file(), "목록 밖의 것을 건드렸다");

    let empty = project_ok(home.path(), &config, &["project", "ls", "--json"]);
    assert!(empty.contains("\"projects\":[]"), "{empty}");
    assert!(project_ok(home.path(), &config, &["project", "ls"]).contains("등록한 프로젝트가 없다"));
}

/// 두 번 더해도 한 줄이고 파일은 한 글자도 안 바뀐다. 이미 있는 것은 실패가 아니다.
#[test]
fn project_add_is_idempotent() {
    let home = Scratch::new("project-idem");
    let config = home.path().join("config.toml");
    std::fs::create_dir_all(home.path().join("a")).unwrap();
    project_ok(home.path(), &config, &["project", "add", "a"]);
    let before = std::fs::read_to_string(&config).unwrap();

    let again = project_ok(home.path(), &config, &["project", "add", "./a/"]);
    assert!(again.contains("이미 등록돼 있다"), "{again}");
    let json = project_ok(home.path(), &config, &["project", "add", "a", "--json"]);
    assert!(json.contains("\"added\":false"), "{json}");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), before);
}

/// **설정이 깨진 `.moai` 도 등록은 하되 못 읽는다고 한 줄 댄다** (moai-9omq). 등록은
/// 사람의 설정에 쓰는 것이지 그 저장소에 쓰는 것이 아니라 막지 않는다 — 읽기는 관대하다.
/// 조용히 받으면 "등록함" 을 믿은 사람이 층과 `status` 에서 "못 읽는다" 를 처음 만난다.
/// `--json` 은 더하기만 한다: `initialized` 는 전처럼 `.moai` 가 있다는 뜻이고, 곁에 `error`.
#[test]
fn project_add_names_a_repo_it_cannot_read() {
    let home = Scratch::new("project-badcfg");
    let config = home.path().join("config.toml");
    let bad = init("project-badcfg-repo");
    std::fs::write(bad.path().join(".moai/config.toml"), "prefix = \"\"\n").unwrap();
    let good = init("project-goodcfg-repo");

    let said = project_ok(home.path(), &config, &["project", "add", bad.path().to_str().unwrap()]);
    assert!(said.contains("등록함"), "{said}");
    assert!(said.contains("못 읽는다") && said.contains("prefix"), "깨진 설정을 안 댔다\n{said}");
    assert!(std::fs::read_to_string(&config).unwrap().contains("path = "), "등록을 안 했다");

    let json = project_ok(home.path(), &config, &["project", "add", bad.path().to_str().unwrap(), "--json"]);
    one_json_value(&json);
    assert!(json.contains("\"initialized\":true") && json.contains("\"error\":\"") && json.contains("prefix"), "{json}");

    let fine = project_ok(home.path(), &config, &["project", "add", good.path().to_str().unwrap(), "--json"]);
    assert!(!fine.contains("\"error\""), "멀쩡한 저장소에 error 를 달았다\n{fine}");
    let fine = project_ok(home.path(), &config, &["project", "add", good.path().to_str().unwrap()]);
    assert!(!fine.contains("못 읽는다"), "{fine}");
}

/// **링크 철자로 적힌 줄이 있으면 푼 경로를 또 넣지 않는다.** 등록은 링크를 풀어 적지만
/// 손으로 적은 줄이나 옛 바이너리가 적은 줄은 링크 철자일 수 있다 — 글자로만 견주면 같은
/// 저장소가 두 줄로 서고, 층에도 `project ls` 에도 둘이 보이며 하나를 빼도 다른 하나가
/// 남는다. TUI 의 고르기 창은 이미 링크를 풀어 그 줄에 `✓ 등록됨` 을 달고 있다.
#[cfg(unix)]
#[test]
fn project_add_is_idempotent_across_a_symlink_spelling() {
    let home = Scratch::new("project-link");
    let config = home.path().join("config.toml");
    let real = home.path().join("real");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, home.path().join("link")).unwrap();
    // 손으로 적은 것처럼 링크 철자로 넣어 둔다.
    std::fs::write(&config, format!("[[project]]\npath = {:?}\n", home.path().join("link").to_str().unwrap())).unwrap();
    let before = std::fs::read_to_string(&config).unwrap();

    let again = project_ok(home.path(), &config, &["project", "add", "real"]);
    assert!(again.contains("이미 등록돼 있다"), "{again}");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), before, "같은 디렉터리가 두 줄로 섰다");
    let shown = project_ok(home.path(), &config, &["project", "ls"]);
    assert_eq!(shown.lines().filter(|l| l.contains("init 전")).count(), 1, "{shown}");
}

/// **`color` 가 찾는 철자는 `rm` 과 같다.** 손으로 `/w/a/../b` 라 적힌 줄은 글자 정리로도
/// 링크 풀기로도 그 철자가 안 나와, 한쪽에만 철자를 더하면 `rm` 이 빼는 줄을 `color` 가
/// "등록돼 있지 않다" 고 거절한다 — 그 거절문이 시키는 `add` 는 같은 디렉터리의 둘째 줄을 만든다.
#[test]
fn project_color_finds_every_spelling_that_rm_can_remove() {
    let home = Scratch::new("project-spell");
    let config = home.path().join("config.toml");
    std::fs::create_dir_all(home.path().join("b")).unwrap();
    std::fs::create_dir_all(home.path().join("a")).unwrap();
    let odd = home.path().join("a/../b");
    std::fs::write(&config, format!("[[project]]\npath = {:?}\n", odd.to_str().unwrap())).unwrap();

    let said = project_ok(home.path(), &config, &["project", "color", odd.to_str().unwrap(), "green"]);
    assert!(said.contains("green"), "{said}");
    assert!(std::fs::read_to_string(&config).unwrap().contains("color = \"green\""));
    let json = project_ok(home.path(), &config, &["project", "rm", odd.to_str().unwrap(), "--json"]);
    assert!(json.contains("\"removed\":[\""), "{json}");
}

/// **안내가 대는 명령은 붙여 넣으면 그 디렉터리로 풀린다.** 경로를 화면용 `one_line` 에
/// 지나게 한 뒤 감쌌을 때는 이름 속 탭이 빈칸이 된 채 인용돼, 시키는 대로 치면 없는
/// 디렉터리를 가리켰다(moai-0cl3). 제어문자는 `$'…'` 로 한 줄에 원문 그대로 선다.
#[test]
fn a_hint_spells_a_path_with_a_tab_so_the_shell_gets_that_directory() {
    let home = Scratch::new("project-tab");
    let config = home.path().join("config.toml");
    let odd = home.path().join("a\tb");
    std::fs::create_dir_all(&odd).unwrap();

    let said = project_ok(home.path(), &config, &["project", "add", odd.to_str().unwrap()]);
    assert!(said.contains(r"a\tb'") && said.contains("$'"), "init 안내가 탭을 원문으로 안 적었다 — {said}");
    // 머리 줄(`등록함 …`)은 화면용이라 빈칸으로 접는 것이 맞다 — 명령 안의 철자만 본다.
    assert!(!said.contains("a b'"), "명령 안에서 탭을 빈칸으로 바꿔 적었다 — {said}");

    let other = home.path().join("c\td");
    std::fs::create_dir_all(&other).unwrap();
    let out = project(home.path(), &config, &["project", "color", other.to_str().unwrap(), "green"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(err.contains(r"c\td'") && err.contains("moai project add $'"), "거절문의 add 가 탭을 원문으로 안 적었다 — {err}");
}

/// 없는 디렉터리와 파일은 거절하고, 설정은 만들지도 않는다.
#[test]
fn project_add_refuses_what_is_not_a_directory() {
    let home = Scratch::new("project-missing");
    let config = home.path().join("cfg/config.toml");
    let out = project(home.path(), &config, &["project", "add", "없음", "--json"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(r#""code":"not_found""#), "{}", text(&out));

    std::fs::write(home.path().join("file"), "").unwrap();
    let out = project(home.path(), &config, &["project", "add", "file", "--json"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(r#""code":"bad_input""#), "{}", text(&out));
    assert!(!config.exists(), "거절한 등록이 설정 파일을 만들었다");
}

/// 모노레포: `.git`·`.moai` 를 찾아 올라가지 않는다. 준 하위 디렉터리가 따로 서고,
/// 루트와 그 안이 함께 등록돼도 막지 않는다. 이름이 겹치면 위 조각이 붙는다.
///
/// `-C` 를 주면 상대경로는 그 디렉터리에 붙는다 — `git -C` 처럼. 자리 인자의
/// 필드가 전역 `-C` 와 같은 clap id(`dir`)였을 때 준 경로가 `-C` 로 새어
/// `add a` 가 `a/a` 를 찾았다. 그 되돌림도 여기서 잡힌다.
#[test]
fn project_registers_monorepo_subdirs_separately() {
    let repo = init("project-mono");
    for sub in ["apps/a", "libs/a", "apps/b"] {
        std::fs::create_dir_all(repo.path().join(sub)).unwrap();
    }
    let config = repo.path().join("user-config.toml");
    for sub in ["a", "b"] {
        project_ok(repo.path(), &config, &["-C", "apps", "project", "add", sub]);
    }
    project_ok(repo.path(), &config, &["project", "add", "libs/a"]);
    project_ok(repo.path(), &config, &["project", "add", "."]);

    let json = project_ok(repo.path(), &config, &["project", "ls", "--json"]);
    let root = repo.path().canonicalize().unwrap();
    for (name, path) in [("apps/a", root.join("apps/a")), ("b", root.join("apps/b")), ("libs/a", root.join("libs/a"))] {
        let row = format!("{{\"name\":\"{name}\",\"path\":\"{}\",\"state\":\"uninitialized\"", path.display());
        assert!(json.contains(&row), "{row} 가 없다 — 루트의 .moai 를 제 것으로 읽었거나 이름을 못 갈랐다\n{json}");
    }
    assert!(json.contains(&format!("\"path\":\"{}\",\"state\":\"initialized\"", root.display())), "{json}");
}

/// 사라진 디렉터리는 `ls` 가 그 줄만 말하고 0 으로 끝나며, 적힌 경로로 뺄 수 있다.
#[test]
fn project_rm_takes_a_vanished_dir() {
    let home = Scratch::new("project-vanish");
    let config = home.path().join("config.toml");
    std::fs::create_dir_all(home.path().join("gone")).unwrap();
    std::fs::create_dir_all(home.path().join("kept")).unwrap();
    project_ok(home.path(), &config, &["project", "add", "gone"]);
    project_ok(home.path(), &config, &["project", "add", "kept"]);
    std::fs::remove_dir(home.path().join("gone")).unwrap();

    let ls = project_ok(home.path(), &config, &["project", "ls"]);
    assert!(ls.lines().any(|l| l.starts_with("gone ") && l.ends_with("디렉터리가 없다")), "{ls}");
    assert!(project_ok(home.path(), &config, &["project", "ls", "--json"]).contains("\"state\":\"missing\""));

    assert!(project_ok(home.path(), &config, &["project", "rm", "gone"]).contains("뺌"));
    // 등록돼 있지 않은 것을 빼는 것은 실패가 아니다 — 바라던 모양이 이미 그렇다.
    let again = project_ok(home.path(), &config, &["project", "rm", "gone", "--json"]);
    assert!(again.contains("\"removed\":[]"), "{again}");
    assert!(project_ok(home.path(), &config, &["project", "ls"]).contains("kept"));
}

/// `ls` 는 프로젝트를 **한눈 보기와 같은 길로 연다.** 연 것은 칸별 수를 한눈 보기와 같은
/// 자로 내고(에픽은 안 센다), 못 읽는 줄은 그 줄 곁에 센다. 설정이나 스냅샷이 깨진
/// 것은 "있음" 이 아니라 `못 읽는다` 한 줄로 서고, 나머지는 그대로 보이며 0 으로 끝난다.
///
/// `state` 낱말은 옛 것 그대로다 — 연 것은 `initialized`. 더한 것은 키뿐이다.
#[test]
fn project_ls_counts_each_column_and_names_a_broken_moai() {
    let s = Scratch::new("project-ls-counts");
    let (good, badcfg, badsnap, bare, out) =
        (dir_in(&s, "good"), dir_in(&s, "badcfg"), dir_in(&s, "badsnap"), dir_in(&s, "bare"), dir_in(&s, "out"));
    ok(&good, &["init", "argos"]);
    add(&good, &["남은 일"]);
    let picked = add(&good, &["집은 일"]);
    ok(&good, &["mv", &picked, "in_progress"]);
    ok(&good, &["epic", "add", "묶음"]);
    let mut snap = issues(&good);
    snap.push_str("못 읽는 줄\n");
    std::fs::write(good.join(".moai/issues.jsonl"), snap).unwrap();
    ok(&badcfg, &["init", "argos"]);
    std::fs::write(badcfg.join(".moai/config.toml"), "prefix = \"BAD!\"\n").unwrap();
    ok(&badsnap, &["init", "argos"]);
    std::fs::write(badsnap.join(".moai/issues.jsonl"), b"\xff\xfe\n").unwrap();
    // 칸 이름도 남의 설정에서 온다 — 제어문자가 든 칸이 목록 화면을 다시 칠하면 안 된다.
    let odd = dir_in(&s, "odd");
    ok(&odd, &["init", "argos"]);
    std::fs::write(odd.join(".moai/config.toml"), "prefix = \"argos\"\nstatuses = \"todo,\u{1b}[2Jwip,done\"\n").unwrap();
    let cfg = registry(&s, &[&good, &badcfg, &badsnap, &bare, &odd]);

    let ls = project_ok(&out, &cfg, &["project", "ls"]);
    let row = |name: &str| ls.lines().find(|l| l.starts_with(&format!("{name} "))).unwrap_or_else(|| panic!("{name} 줄이 없다\n{ls}"));
    assert!(row("good").contains("· todo 1  ▸ in_progress 1  ? review 0  ✓ done 0"), "{ls}");
    assert!(row("good").ends_with("! 읽을 수 없는 줄 1개"), "{ls}");
    assert!(row("badcfg").contains("! 못 읽는다 — ") && row("badcfg").contains("prefix"), "{ls}");
    assert!(row("badsnap").contains("! 못 읽는다 — ") && row("badsnap").contains("issues.jsonl"), "{ls}");
    assert!(!ls.contains(".moai 있음"), "깨진 .moai 를 있음으로 접었다\n{ls}");
    assert!(row("bare").ends_with("init 전"), "{ls}");
    assert!(row("odd").contains("[2Jwip 0"), "{ls}");
    assert!(!ls.contains('\u{1b}'), "남의 설정이 화면을 다시 칠했다\n{ls:?}");
    // 한눈 보기와 같은 자로 센다 — 두 화면의 수가 어긋나면 어느 쪽을 믿을지 모른다.
    assert!(ok_with(&out, &cfg, &["status"]).contains("todo 1    ▸ in_progress 1"));

    let json = project_ok(&out, &cfg, &["project", "ls", "--json"]);
    one_json_value(&json);
    let good_row = format!(
        "{{\"name\":\"good\",\"path\":{:?},\"state\":\"initialized\",\"counts\":{{\"done\":0,\"in_progress\":1,\"review\":0,\"todo\":1}},\"unreadable\":1}}",
        good.to_str().unwrap()
    );
    assert!(json.contains(&good_row), "{good_row} 가 없다\n{json}");
    let broken = format!("{{\"name\":\"badcfg\",\"path\":{:?},\"state\":\"unreadable\",\"error\":\"", badcfg.to_str().unwrap());
    assert!(json.contains(&broken), "{json}");
    assert!(json.contains("\"name\":\"badsnap\"") && json.matches("\"state\":\"unreadable\"").count() == 2, "{json}");
    assert!(json.contains(&format!("{{\"name\":\"bare\",\"path\":{:?},\"state\":\"uninitialized\"}}", bare.to_str().unwrap())), "{json}");
}

/// 깨진 설정: `ls` 는 까닭을 말하고 0 으로 끝나고, `add`·`rm` 은 멈추고 파일을 안 건드린다.
#[test]
fn a_broken_user_config_refuses_writes_but_ls_is_lenient() {
    let home = Scratch::new("project-broken");
    let config = home.path().join("config.toml");
    std::fs::create_dir_all(home.path().join("a")).unwrap();
    let src = "project = \"/a\"\n";
    std::fs::write(&config, src).unwrap();

    let out = project(home.path(), &config, &["project", "ls"]);
    assert!(out.status.success(), "깨진 설정으로 ls 가 실패했다\n{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("[[project]]"), "{}", text(&out));
    let json = project_ok(home.path(), &config, &["project", "ls", "--json"]);
    assert!(json.contains("\"projects\":[]") && json.contains("\"problems\":[\""), "{json}");

    for args in [["project", "add", "a", "--json"], ["project", "rm", "a", "--json"]] {
        let out = project(home.path(), &config, &args);
        assert!(!out.status.success(), "{args:?} 가 깨진 설정에 썼다");
        assert!(String::from_utf8_lossy(&out.stderr).contains(r#""code":"broken""#), "{args:?}\n{}", text(&out));
    }
    assert_eq!(std::fs::read_to_string(&config).unwrap(), src);
}

/// **세션 시작점(`moai`)도 설정의 문제를 댄다.** 설정이 깨져 등록한 것이 하나도 안
/// 읽히면 `status`·`ready` 는 파싱 오류를 대는데, 인자 없는 `moai` 만 "등록한 것이 없다,
/// 더하라" 로 끝나 사람을 깨진 파일에 `project add` 치게 했다. 도움말 자리라 종료 코드는 0.
#[test]
fn bare_moai_outside_names_a_broken_user_config() {
    let home = Scratch::new("bare-broken");
    let config = home.path().join("config.toml");
    std::fs::write(&config, "[[project]]\npath = \"/x\"\n[[project\n").unwrap();
    let out = project(home.path(), &config, &[]);
    assert!(out.status.success(), "{}", text(&out));
    let said = text(&out);
    assert!(said.contains("project add"), "{said}");
    assert!(said.contains("TOML") && said.contains("config.toml"), "깨진 설정을 안 댔다\n{said}");

    std::fs::write(&config, "").unwrap();
    assert!(!text(&project(home.path(), &config, &[])).contains("TOML"), "멀쩡한 설정에 문제를 댔다");
}

/// **등록 한 번에 설정 파일의 권한이 풀리지 않는다.** 임시 파일은 umask 권한으로 새로
/// 서므로 그대로 바꿔 끼우면 `chmod 600` 한 설정이 0644 가 된다.
#[cfg(unix)]
#[test]
fn project_writes_keep_the_config_files_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = Scratch::new("project-perms");
    let config = home.path().join("config.toml");
    std::fs::create_dir_all(home.path().join("a")).unwrap();
    std::fs::create_dir_all(home.path().join("b")).unwrap();
    std::fs::write(&config, "# 내 설정\n").unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    for args in [["project", "add", "a"].as_slice(), &["project", "color", "a", "green"], &["project", "add", "b"], &["project", "rm", "a"]] {
        project_ok(home.path(), &config, args);
        let mode = std::fs::metadata(&config).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{args:?} 뒤에 권한이 {mode:o} 로 바뀌었다");
    }
}

/// **스냅샷 쓰기도 사람이 정한 권한을 지킨다** (moai-c1s3). `with_write` 는 매번 임시 파일을
/// 새로 세워 바꿔 끼우므로, 그대로 두면 `chmod 600` 한 `issues.jsonl` 이 `add` 한 번에 umask
/// 권한으로 풀린다. 저널은 제자리에 덧붙이므로 원래 안 풀린다 — 함께 재 둔다.
#[cfg(unix)]
#[test]
fn repo_writes_keep_the_snapshot_files_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let s = init("repo-perms");
    let files = [s.path().join(".moai/issues.jsonl"), s.path().join(".moai/journal.jsonl")];
    for f in &files {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let id = add(s.path(), &["권한"]);
    ok(s.path(), &["mv", &id, "in_progress"]);
    ok(s.path(), &["note", &id, "메모"]);
    for f in &files {
        let mode = std::fs::metadata(f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{} 의 권한이 {mode:o} 로 바뀌었다", f.display());
    }
}

/// 설정 파일이 없을 때 빼면 아무 일도 안 한다 — 설정 디렉터리조차 만들지 않는다.
#[test]
fn project_rm_without_a_config_leaves_no_trace() {
    let home = Scratch::new("project-rm-none");
    let config = home.path().join("cfg/moai/config.toml");
    let out = project_ok(home.path(), &config, &["project", "rm", "어디든"]);
    assert!(out.contains("등록돼 있지 않다"), "{out}");
    assert!(!home.path().join("cfg").exists(), "아무것도 안 뺀 명령이 설정 디렉터리를 만들었다");
}
