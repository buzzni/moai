//! 바이너리를 실제로 돌린다. dev-dependency 없이 — 테스트 하네스가
//! 의존성을 끌고 오기 시작하면 "남의 저장소에 설치되는 도구" 라는 전제가 흐려진다.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_moai");
const NOW: &str = "2026-09-11T04:12:03Z";
/// 시험이 대는 사람. **한 자리에 둔다** — 글자를 베껴 적으면 한쪽만 고쳐도 아무도 모른다.
const ACTOR: &str = "테스터 (tester@example.com)";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = scratch::base().join(format!(
            "moai-cli-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        if scratch::fenced_base() {
            scratch::fence(&dir);
        }
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
/// 걷는 것은 moai 와 **시험이 띄우는 셸**이 실제로 기계에서 읽는 자리뿐이다.
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
/// - `BASH_ENV` — bash 를 띄우는 시험(`bash -c`, 예제 루프)이 **켜자마자 읽는 파일**.
///   비대화형 bash 는 스크립트 첫 줄보다 먼저 그 파일을 돌리고, 그것이 돌리는 사람의
///   `~/.bashrc` 인 셸이 실제로 있다(이 저장소를 돌리는 자리가 그렇다). 거기서 `MOAI`·
///   `MOAI_ACTOR` 를 내보내거나 `cd` 하면 아래 `.env` 로 준 값과 자리를 bash 안에서 덮는다.
///   `ENV`·`SHELLOPTS`·`POSIXLY_CORRECT` 는 걷지 않는다 — 비대화형 bash 는 그것들로
///   시작 파일을 읽지 않는다(재 봤다). 걷는 목록은 **재 본 것만** 늘린다
/// - `COLUMNS` — clap 이 도움말을 접을 때 읽는 폭. 지금은 접기를 꺼 안 읽지만(moai-opjn),
///   그것이 돌아오는 날을 지키는 `narrow_terminals_keep_heredoc_openers_whole` 는 폭 하나만
///   바꿔 기준과 견준다. 기준이 돌리는 사람의 창 폭을 물려받으면 좁은 셸에서는 훑기가 접힌
///   `Commands:` 줄을 명령 이름으로 읽어, 접힘을 대는 대신 엉뚱한 이름으로 넘어진다
///
/// 시험이 제 값을 주려면 이 뒤에 `.env` 로 덮는다. `PATH` 는 두고 간다 — moai 가
/// git 을 부르고, `claude` 가 없는 자리가 필요한 시험은 `Claude` 가 따로 만든다.
fn isolated(program: impl AsRef<std::ffi::OsStr>) -> Command {
    // 빈 집 하나를 모든 시험이 함께 쓴다. moai 도 git 도 `HOME` 에 쓰지 않으니
    // 비어 있는 채로 남고, `target/` 밑이라 기계에 부스러기를 흘리지 않는다.
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("moai-cli-home");
    std::fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(program);
    // **걷기가 먼저다.** `Command` 의 환경은 이름마다 마지막에 부른 것이 이긴다 —
    // 아래 `.env` 보다 뒤에 두면, 목록에 `GIT_CONFIG_GLOBAL` 같은 이름이 느는 날
    // 이 루프가 방금 세운 격리를 도로 지운다. `git::isolated` 도 같은 차례다.
    // 그날 `the_fake_claude_command_starts_from_the_isolated_one` 도 같이 고친다 —
    // 그쪽은 목록의 **모든** 이름이 걷혔는지 보므로, 여기서 덮어 준 이름에 걸린다.
    // 차례를 되돌려 달래지 않는다. 그게 이 차례가 막는 바로 그 덫이다.
    for var in git_leaks::swept(true) {
        cmd.env_remove(var);
    }
    cmd.env("HOME", &home)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_CODE_PLUGIN_CACHE_DIR")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("MOAI_ACTOR")
        .env_remove("MOAI_NOW")
        .env_remove("COLUMNS")
        // 사용자 설정은 **없는 파일**을 가리킨다. 등록한 프로젝트가 새면 `.moai`
        // 밖에서 부르는 시험이 돌리는 사람의 프로젝트를 본다. `XDG_CONFIG_HOME` 도
        // 걷는다 — `MOAI_CONFIG` 를 덮어쓴 시험이 그것을 지우면 그다음 자리다.
        .env("MOAI_CONFIG", home.join("moai-config-unset/config.toml"))
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("BASH_ENV")
        // **시험은 한국어 화면을 본다**(moai-zeyv). 기본은 영어지만(사용자 결정) 이 저장소의
        // 시험은 글자를 그대로 견주는 것이 수백 줄이라, 여기서 언어를 못 박는다 — 안 박으면
        // 글자를 말묶음으로 옮길 때마다 시험이 "말이 바뀐 것" 인지 "동작이 바뀐 것" 인지를
        // 못 가른다. 영어 화면은 `english_is_the_default_when_nothing_picks_a_language` 가
        // 이 변수를 걷고 따로 잰다.
        .env("MOAI_LANG", "ko");
    cmd
}

// 물려받으면 git 이 바깥 저장소나 바깥 설정을 보게 되는 변수들 — 단위 시험(`git::command`)과 **한 파일**을
// 읽는다. 따로 된 크레이트라 `use` 로는 못 가져가고, 두 벌로 두면 한쪽에만 더한 변수가 말없이 갈라진다.
#[path = "../src/git_leaks.rs"]
mod git_leaks;

// 임시 자리의 뿌리와 울타리 — 단위 시험의 `Scratch` 와 **한 파일**을 읽는다(moai-boc6). 임시 자리가 체크아웃
// 안이면 자리마다 울타리를 치는데, 두 벌로 두면 한쪽만 그 울타리를 친다. 쓰는 것은 `base`·`fenced_base`·`fence` 다.
// 그 파일의 `#[cfg(test)]` 시험도 여기서 함께 돈다 — 통합 시험도 `cfg(test)` 로 컴파일된다.
#[path = "../src/scratch.rs"]
#[allow(dead_code)]
mod scratch;

/// 시험이 부르는 moai 한 벌. **사람·시계·색을 한 자리에서 준다**(moai-uu47).
///
/// 이 세 줄이 여덟 군데에 베껴져 있었다. 베낀 자리는 조용히 갈라진다 — `hook_in` 은 사람과
/// 시계만 주고 색을 안 줘, 다른 자리들과 다른 환경에서 돌고 있었다.
///
/// 더 줄 것이 있으면 돌려받아 이어 붙인다(`env`·`current_dir`). 걷어야 할 것은
/// [`isolated`] 가 이미 걷었다.
fn staged(args: &[&str]) -> Command {
    let mut cmd = isolated(BIN);
    cmd.args(args)
        .env("MOAI_ACTOR", ACTOR)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1");
    cmd
}

/// 시계를 **안 고정한** 한 벌. 겨루기 시험이 쓴다 — 여덟이 같은 시각을 들고 달리면
/// 겨루는 것이 실제 시계가 아니라 그 값이 된다.
fn staged_live(args: &[&str]) -> Command {
    let mut cmd = isolated(BIN);
    cmd.args(args).env("MOAI_ACTOR", ACTOR).env("NO_COLOR", "1");
    cmd
}

fn moai(dir: &Path, args: &[&str]) -> Output {
    staged(args).current_dir(dir).output().expect("moai 를 실행하지 못했다")
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

/// **말이 바뀌어도 표는 안 깨진다**(moai-i44x) — 한글·일본어·중국어는 한 글자가 두 칸이라,
/// 글자 수로 재는 자리가 하나라도 있으면 그 말에서만 열이 어긋난다.
///
/// 다섯 말로 `ready` 를 돌려 **열이 같은 칸에서 시작하는지**를 표시 폭으로 잰다. 글자 수로
/// 재면 영어에서는 맞고 한국어·일본어에서만 틀리므로, 한 말로만 재는 시험은 이것을 못 본다.
#[test]
fn every_language_keeps_the_ready_table_in_line() {
    let s = init("wide");
    // **글자 수와 칸 수가 크게 엇갈리는 제목을 섞는다** — 한글 제목은 글자 수보다 칸 수가
    // 곱절이라, 글자 수로 채우는 코드는 여기서만 어긋난다.
    ok(s.path(), &["add", "짧은 일"]);
    ok(s.path(), &["add", "아주 긴 한글 제목이 여기에 들어간다"]);
    ok(s.path(), &["add", "plain ascii"]);
    for lang in ["en", "ko", "ja", "zh", "es"] {
        let mut cmd = isolated(BIN);
        cmd.args(["ready"]).current_dir(s.path()).env("MOAI_NOW", NOW).env("NO_COLOR", "1").env("MOAI_LANG", lang);
        let out = cmd.output().expect("moai 를 실행하지 못했다");
        assert!(out.status.success(), "{lang}: {}", String::from_utf8_lossy(&out.stderr));
        let screen = String::from_utf8(out.stdout).unwrap();
        // **제목 뒤의 열이 서는 자리를 잰다** — 제목이 두 칸 글자라, 글자 수로 채우면 여기서
        // 어긋난다. 우선순위(`p2`)는 id 뒤라 ASCII 만 앞서므로 그것만 재면 못 잡는다.
        let starts: Vec<(usize, usize)> = screen
            .lines()
            .filter(|l| l.contains("argos-"))
            .map(|l| {
                let p = l.find("p2").expect("우선순위 칸이 없다");
                (cells(&l[..p]), last_column(l))
            })
            .collect();
        assert!(starts.len() >= 3, "{lang}: 표에 줄이 모자라다\n{screen}");
        // **잰 자리가 0 이면 아무것도 안 잰 것이다** — 아래 두 줄은 다 0 이어도 통과한다.
        assert!(starts.iter().all(|(p, e)| *p > 0 && *e > *p), "{lang}: 열을 못 찾았다 — {starts:?}\n{screen}");
        assert!(starts.windows(2).all(|w| w[0].0 == w[1].0), "{lang}: 우선순위 열이 줄마다 다른 칸에서 선다 — {starts:?}\n{screen}");
        assert!(starts.windows(2).all(|w| w[0].1 == w[1].1), "{lang}: 제목 뒤 열이 줄마다 다른 칸에서 선다 — {starts:?}\n{screen}");
    }
}

/// 화면에서 그 글이 차지하는 **칸 수**. 한글·일본어·중국어는 한 글자가 두 칸이다.
fn cells(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// 그 줄의 **마지막 열이 서는 칸.** 열 사이는 두 칸 이상 벌어지므로 마지막 틈 뒤가 그 자리다.
///
/// **글자로 찾지 않는다**(리뷰 moai-80qw). 전에는 `에픽 없음` 을 `rfind` 로 짚었는데, 그 낱말은
/// 말묶음으로 옮겨 갈 차례에 있는 것이라 옮기는 날 en·ja·zh·es 에서 `expect` 가 터진다 —
/// 번역이 처음으로 잰 칸에 들어오는 바로 그때, 어긋남을 재는 대신 시험이 죽는다.
fn last_column(l: &str) -> usize {
    let line = l.trim_end();
    let Some(gap) = line.rfind("  ") else { return 0 };
    let tail = &line[gap..];
    cells(&line[..gap + (tail.len() - tail.trim_start().len())])
}

/// **아무도 고르지 않으면 지금은 한국어고, 영어는 낱말 하나로 온다**(사용자 결정, 리뷰
/// moai-80qw.cb8). 옮긴 글이 열 줄뿐이라 기본을 영어로 두면 화면이 두 말로 섞인다 — 옮김이
/// 화면을 덮을 때 `Lang` 의 `#[default]` 한 줄과 함께 이 기대값이 영어로 바뀐다.
///
/// **`MOAI_LANG` 으로 한국어·일본어도 함께 잰다** — 고르는 길이 도는지, 그리고 두 칸 글자가
/// 섞여도 줄이 서는지. 어느 말이든 같은 수를 대는 것으로 그 화면이 같은 화면임을 잰다.
#[test]
fn nothing_picked_means_korean_for_now_and_english_is_one_word_away() {
    let s = init("lang");
    ok(s.path(), &["add", "첫 일"]);
    let say = |lang: Option<&str>| {
        let mut cmd = isolated(BIN);
        cmd.args(["status"]).current_dir(s.path()).env("MOAI_NOW", NOW).env("NO_COLOR", "1");
        match lang {
            Some(l) => cmd.env("MOAI_LANG", l),
            None => cmd.env_remove("MOAI_LANG"),
        };
        let out = cmd.output().expect("moai 를 실행하지 못했다");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    // **아무것도 안 고르면 지금은 한국어다**(사용자 결정) — 옮긴 글이 열 줄뿐이라, 기본을
    // 영어로 두면 화면이 두 말로 섞인다. 옮김이 화면을 덮으면 이 기대값이 영어로 바뀐다.
    let fallback = say(None);
    assert!(fallback.contains("이슈 1"), "아무도 안 골랐는데 기본 화면이 아니다\n{fallback}");

    // **영어는 낱말 하나로 온다** — 기준 말이라 늘 고를 수 있다.
    let english = say(Some("en"));
    assert!(english.contains("Issues 1"), "MOAI_LANG=en 이 안 들었다\n{english}");
    let head = |screen: &str| screen.lines().next().unwrap_or_default().to_string();
    assert!(!head(&english).contains("이슈"), "영어 화면의 머리에 한국어가 남았다\n{english}");

    let korean = say(Some("ko"));
    assert!(korean.contains("이슈 1"), "MOAI_LANG=ko 가 안 들었다\n{korean}");
    assert_eq!(korean, fallback, "고른 한국어와 기본 화면이 다르다");

    // 없는 키는 영어로 떨어진다 — 화면이 비지 않는다.
    let japanese = say(Some("ja_JP.UTF-8"));
    assert!(japanese.contains("課題 1"), "로캘 모양의 MOAI_LANG 이 안 들었다\n{japanese}");

    // 말이 달라도 같은 화면이다 — 줄 수가 같다.
    assert_eq!(english.lines().count(), korean.lines().count(), "영어와 한국어의 줄 수가 다르다");
    assert_eq!(english.lines().count(), japanese.lines().count(), "영어와 일본어의 줄 수가 다르다");
}

/// **설정에 적은 말도 든다**(moai-slfv), 그리고 **틀리면 저장소 안에서도 댄다**(리뷰 moai-80qw).
///
/// 여기까지 오는 길(`Doc::lang` → `Registry::lang` → `i18n::pick`)은 `MOAI_LANG` 만 재는
/// 시험들이 한 줄도 안 밟던 자리다 — 그 길이 끊어져도 다 푸르고, `[i18n] lang` 을 적어 둔
/// 사람만 조용히 영어를 본다. 틀린 값은 **stderr 로** 대고 종료 코드는 안 바꾼다: `status` 는
/// 아무것도 막지 않는다.
#[test]
fn the_config_picks_the_language_and_a_bad_one_is_named() {
    let s = init("lang-config");
    ok(s.path(), &["add", "첫 일"]);
    let config = s.path().join("user/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let run = |body: &str| {
        std::fs::write(&config, body).unwrap();
        let mut cmd = isolated(BIN);
        cmd.args(["status"])
            .current_dir(s.path())
            .env("MOAI_CONFIG", &config)
            .env("MOAI_NOW", NOW)
            .env("NO_COLOR", "1")
            .env_remove("MOAI_LANG");
        let out = cmd.output().expect("moai 를 실행하지 못했다");
        assert!(out.status.success(), "설정 하나로 멈췄다\n{}", String::from_utf8_lossy(&out.stderr));
        (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
    };

    let (screen, said) = run("[i18n]\nlang = \"ja\"\n");
    assert!(screen.contains("課題 1"), "설정에 적은 말이 안 들었다\n{screen}");
    assert!(said.is_empty(), "멀쩡한 설정에 할 말이 생겼다 — {said}");

    // **환경이 설정을 이긴다** — 같은 파일을 두고 `MOAI_LANG` 을 주면 그쪽이다.
    let mut cmd = isolated(BIN);
    cmd.args(["status"]).current_dir(s.path()).env("MOAI_CONFIG", &config).env("MOAI_NOW", NOW).env("NO_COLOR", "1");
    let out = cmd.output().expect("moai 를 실행하지 못했다");
    assert!(String::from_utf8_lossy(&out.stdout).contains("이슈 1"), "MOAI_LANG 이 설정에 졌다");

    // **오타는 조용히 기본값이 되지 않는다.** 이 줄이 없으면 고친 설정이 왜 안 듣는지 알 길이 없다.
    let (screen, said) = run("[i18n]\nlang = \"kr\"\n");
    assert!(screen.contains("이슈 1"), "모르는 값에서 기본값(지금은 한국어)으로 안 떨어졌다\n{screen}");
    assert!(said.contains("`i18n.lang`") && said.contains("\"kr\""), "틀린 설정을 아무도 안 댔다 — {said:?}");
    // 보드도 그것을 안다 — 없으면 "드러난 문제 없다" 가 방금 stderr 에 한 말을 뒤집는다.
    assert!(screen.contains("사용자 설정에서 못 읽은 것 1건"), "보드가 stderr 의 말을 모른다\n{screen}");
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
    // **까닭도 같이 심는다.** 이 주석이 왜 `issues.jsonl` 에 union 을 걸면 안 되는지 적은 유일한
    // 자리다 — 빼면 새 저장소가 그 까닭 없이 서고, 다음 사람이 union 을 다시 건다.
    assert!(attrs.contains("# 스냅샷에는 merge=union 을 쓰지 않는다"), "규칙만 심고 까닭을 뺐다\n{attrs}");
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

/// **새로 심는 접두어는 8자까지다**(moai-f7xs). 사람이 준 긴 것은 거절하고 짧은 후보를 대며
/// 아무것도 안 만든다. 디렉터리 이름에서 만든 긴 것은 머리글자로 줄이고 그렇다고 말한다.
/// 이미 긴 접두어로 심긴 저장소는 막지 않는다 — 읽기는 관대하게, 접두어는 못 바꾸는 값이다.
#[test]
fn init_keeps_a_new_prefix_short() {
    let s = Scratch::new("shortprefix");
    let out = moai(s.path(), &["init", "my-company-backend"]);
    assert!(!out.status.success(), "긴 접두어를 받았다");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("8자까지") && err.contains("`mcb`"), "{err}");
    assert!(!s.path().join(".moai").exists(), "거절하고도 .moai 를 만들었다");
    // 모양이 틀린 긴 접두어는 모양을 먼저 말한다 — 길이 오류가 틀린 후보(`MyCompan`)를 대면 안 된다.
    let out = moai(s.path(), &["init", "MyCompanyBackend"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("소문자") && !err.contains("8자까지"), "{err}");

    // 디렉터리 이름에서 만든 긴 것은 줄인다.
    let long = s.path().join("my-company-backend");
    std::fs::create_dir_all(&long).unwrap();
    let out = ok(&long, &["init"]);
    assert!(out.contains("접두어는 `mcb`") && out.contains("줄였다"), "{out}");
    assert!(add(&long, &["첫 이슈"]).starts_with("mcb-"), "줄인 접두어로 id 를 안 냈다");

    // 기계 출력은 줄였을 때만 원래 이름을 싣는다.
    let other = s.path().join("another-long-name-here");
    std::fs::create_dir_all(&other).unwrap();
    let json = ok(&other, &["init", "--json"]);
    assert!(json.contains("\"prefix\":\"alnh\"") && json.contains("\"shortened_from\":\"another-long-name-here\""), "{json}");
    let short = s.path().join("argos");
    std::fs::create_dir_all(&short).unwrap();
    assert!(!ok(&short, &["init", "--json"]).contains("shortened_from"), "안 줄였는데 원래 이름을 실었다");

    // 이미 긴 접두어로 심긴 저장소는 다시 불러도, 이슈를 만들어도 된다.
    let old = s.path().join("old");
    std::fs::create_dir_all(old.join(".moai")).unwrap();
    std::fs::write(old.join(".moai/config.toml"), "prefix = \"my-company-backend\"\n").unwrap();
    std::fs::write(old.join(".moai/issues.jsonl"), "").unwrap();
    std::fs::write(old.join(".moai/journal.jsonl"), "").unwrap();
    assert!(ok(&old, &["init"]).contains("이미 심겨 있다"));
    assert!(ok(&old, &["init", "my-company-backend"]).contains("이미 심겨 있다"), "같은 긴 접두어로 다시 부른 것을 막았다");
    let out = moai(&old, &["init", "another-long-one"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("나중에 못 바꾼다") && !err.contains("8자까지"), "{err}");
    assert!(add(&old, &["옛 저장소의 이슈"]).starts_with("my-company-backend-"), "옛 긴 접두어를 막았다");
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

/// **워크트리 자리도 gitignore 한다**(moai-mxtb) — 감독 일꾼 절차가 `.claude/worktrees/` 에
/// 워크트리를 뜨는데, 그 자리가 안 막히면 `git add -A` 에 남의 가지 전체가 딸려 온다.
/// 넣는 것은 그 자리 하나다 — `.claude/` 통째는 저장소가 커밋하는 설정·스킬을 가린다.
/// 같은 뜻의 철자나 `.claude/` 를 이미 막았으면 더하지 않고, 다시 불러도 한 줄이다.
#[test]
fn init_ignores_the_worktree_dir_once_and_respects_equivalent_spellings() {
    let s = Scratch::new("ignorewt");
    let claude_lines = |got: &str| got.lines().filter(|l| l.contains(".claude")).count();

    let fresh = s.path().join("fresh");
    std::fs::create_dir_all(&fresh).unwrap();
    ok(&fresh, &["init", "argos"]);
    let got = std::fs::read_to_string(fresh.join(".gitignore")).unwrap();
    assert!(got.lines().any(|l| l == "/.claude/worktrees/"), "새 저장소에 워크트리 자리를 안 막았다\n{got}");
    ok(&fresh, &["init"]);
    assert_eq!(std::fs::read_to_string(fresh.join(".gitignore")).unwrap(), got, "다시 init 하자 .gitignore 가 바뀌었다");

    for (name, already) in [("anchored", "/.claude/worktrees\n"), ("bare", ".claude/worktrees/\n"), ("whole", ".claude/\n")] {
        let dir = s.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let original = format!("target/\n# 우리 것\n{already}");
        std::fs::write(dir.join(".gitignore"), &original).unwrap();
        ok(&dir, &["init", "argos"]);
        let got = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(got.starts_with(&original), "`{name}`: 원래 .gitignore 를 바꿨다\n{got}");
        assert_eq!(claude_lines(&got), 1, "`{name}`: 같은 뜻의 줄이 있는데 또 더했다\n{got}");
    }

    // 끝 `/` 는 "디렉터리만" 이다(리뷰 moai-mxtb.az6). `.moai/lock/` 은 락 파일을 못 막으니
    // 같은 이름이라도 덮은 것으로 치지 않고 `.moai/lock` 을 더한다.
    let dir = s.path().join("dironly");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".gitignore"), ".moai/lock/\n").unwrap();
    ok(&dir, &["init", "argos"]);
    let got = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(got.lines().any(|l| l == ".moai/lock"), "디렉터리 전용 줄을 락 파일을 막은 것으로 쳤다\n{got}");
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
            staged_live(&["add", &format!("동시 {i}"), "-q"])
                .current_dir(s.path())
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
    staged(args)
        .current_dir(dir)
        .env("MOAI_CONFIG", config)
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

/// **`.moai` 밖 한눈 보기도 자리 없는 줄을 비춘다**(moai-p3bs). 죽은 세션을 찾으러 돌아온 사람이
/// 프로젝트 밖에서 보는 화면이 여기라, 안쪽 `moai status` 에만 그 말이 있으면 못 본다. 경고 수와
/// `--json` 의 `warnings` 가 안쪽과 같은 수를 말한다.
#[test]
fn the_overview_counts_work_with_no_live_worktree() {
    let s = Scratch::new("ovstranded");
    let (one, out) = (dir_in(&s, "one"), dir_in(&s, "out"));
    git(&one, &["init", "-q"]);
    ok(&one, &["init", "argos"]);
    let lost = add(&one, &["세션이 죽은 일"]);
    ok(&one, &["mv", &lost, "in_progress"]);
    git(&one, &["add", "-A"]);
    git(&one, &["commit", "-q", "-m", "집는다"]);
    // 이름이 그 줄을 안 가리키는 워크트리 — 워크트리를 쓰는 저장소라는 표시다.
    git(&one, &["worktree", "add", "-q", ".claude/worktrees/agent-x", "-b", "worktree-agent-x"]);
    let cfg = registry(&s, &[&one]);

    let inside = ok_at(&one, LATER, &["status"]);
    assert!(inside.contains("일하는 워크트리가 없는 것 1건"), "안쪽이 안 비췄다\n{inside}");
    // 안쪽이 세는 경고 수 — 밖에서도 같은 수를 말해야 한다. **`warnings` 안만 센다**:
    // `"kind":` 는 `notices` 에도 붙어, 통째로 세면 알림 하나가 서는 날 이 시험이 자기가
    // 재겠다고 한 것과 아무 상관 없는 까닭으로 깨진다.
    let json = ok_at(&one, LATER, &["status", "--json"]);
    let want = json
        .split("\"warnings\":[")
        .nth(1)
        .and_then(|r| r.split("],\"notices\":").next())
        .unwrap_or_else(|| panic!("경고 배열을 못 찾았다\n{json}"))
        .matches("\"kind\":")
        .count()
        .to_string();
    let n = |t: &str| t.split("경고 ").nth(1).and_then(|r| r.split('건').next()).map(str::to_string);

    let out_text = isolated(BIN)
        .args(["status"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(out_text.status.success());
    let text = String::from_utf8(out_text.stdout).unwrap();
    let mine = block(&text, "one");
    assert!(!mine.contains("드러난 문제 없다"), "자리 없는 줄을 안 세웠다\n{text}");
    assert_eq!(n(mine), Some(want), "안쪽과 다른 수를 말한다\n{text}");

    let machine = isolated(BIN)
        .args(["status", "--json"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let json = String::from_utf8(machine.stdout).unwrap();
    assert!(json.contains("\"kind\":\"stranded\"") && json.contains(&lost), "{json}");

    // **탐색기의 프로젝트 층도 같은 셈을 쓴다**(`layer::summarize`) — 층은 경고를 수로만 내므로
    // 그 수에 들었는지와, 무엇인지 대는 `stranded` 키로 본다.
    let layer = isolated(BIN)
        .args(["tui", "--json"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let layer = String::from_utf8(layer.stdout).unwrap();
    assert!(layer.contains("\"stranded\":1"), "층이 자리 없는 줄을 안 셌다\n{layer}");
    assert!(layer.contains("\"warnings\":2"), "층의 경고 수에 안 들었다\n{layer}");

    // **못 읽은 워크트리가 있으면 "센 결과 0" 이 아니라 "못 셌다" 다**(리뷰 moai-p3bs.op2) —
    // 밖에서는 옆 스냅샷을 아예 안 여므로, 세지 못했다는 사실이 여기서 사라지면 죽은 세션이
    // 통째로 조용해진다.
    let snap = one.join(".claude/worktrees/agent-x/.moai/issues.jsonl");
    std::fs::remove_file(&snap).unwrap();
    std::fs::create_dir(&snap).unwrap();
    let blind = isolated(BIN)
        .args(["status"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let blind = String::from_utf8(blind.stdout).unwrap();
    let mine = block(&blind, "one");
    assert!(!mine.contains("드러난 문제 없다"), "못 셌는데 문제 없다고 했다\n{blind}");
    assert!(mine.contains("옆 워크트리 문제 1건"), "못 읽은 워크트리를 안 셌다\n{blind}");
    // **센 것은 줄로도 댄다** — 수만 서면 `— 위 줄` 이 없는 줄을 가리키고 어느 워크트리인지 모른다.
    assert!(
        mine.contains("스냅샷을 못 읽었다 — ⎇ worktree-agent-x: .claude/worktrees/agent-x"),
        "못 읽은 워크트리를 세기만 하고 대지 않았다\n{blind}"
    );

    // **겹쳐 보면 `gather` 가 같은 워크트리를 이미 냈다 — 두 번 세지 않는다**(`status` 의
    // `said_already` 와 같은 자). 겹쳐 세면 깨진 워크트리 하나가 `옆 워크트리 문제 2건` 으로 서서
    // 보는 쪽이 두 곳이 깨진 줄로 읽는다. **기계도 같은 사실을 안쪽과 같은 키로 받는다** — 없으면
    // 밖에서 읽는 쪽은 "자리 잃은 일이 없다" 와 "못 셌다" 를 못 가른다.
    let both = isolated(BIN)
        .args(["status", "--worktree"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let both = String::from_utf8(both.stdout).unwrap();
    assert!(block(&both, "one").contains("옆 워크트리 문제 1건"), "한 워크트리를 두 번 셌다\n{both}");
    assert!(!block(&both, "one").contains("스냅샷을 못 읽었다 — ⎇"), "`gather` 가 낸 워크트리를 한 번 더 댔다\n{both}");
    let machine = isolated(BIN)
        .args(["status", "--worktree", "--json"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let machine = String::from_utf8(machine.stdout).unwrap();
    assert!(
        machine.contains("\"unreadable_worktrees\":[{\"path\":\".claude/worktrees/agent-x\""),
        "밖 한눈 보기의 기계 출력이 못 읽은 워크트리를 안 댔다\n{machine}"
    );

    let layer = isolated(BIN)
        .args(["tui", "--json"])
        .current_dir(&out)
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let layer = String::from_utf8(layer.stdout).unwrap();
    assert!(layer.contains("\"unreadable_worktrees\":1"), "층이 못 읽은 워크트리를 안 댔다\n{layer}");
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

    // 손으로 적은 표 모양 color 는 색으로도 auto 로도 덮지 않는다(moai-r9qa) — 0 아닌 코드로 멈추고,
    // 어느 파일의 어느 줄인지 대고, 한 바이트도 안 바꾼다.
    let table = format!("{original}color.x = 1\n");
    std::fs::write(&cfg, &table).unwrap();
    for word in [target, "auto"] {
        let o = run(&["project", "color", one_arg, word, "--json"], false);
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(!o.status.success() && err.contains("손으로") && err.contains("config.toml") && err.contains(one_arg), "{word} → {}", text(&o));
        // 깨진 설정과 같은 코드다(moai-3owm) — 기계가 I/O 실패와 갈라 사람에게 넘긴다.
        assert!(err.contains(r#""code":"broken""#), "{word} → {err}");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), table, "{word} 가 표 모양 color 를 덮었다");
    }

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

/// 줄 **맨 앞의** 칠한 칸 — (SGR, 글). `indent` 를 떼고 곧바로 SGR 이 이어져야 칸이다.
///
/// 한눈 보기의 줄은 `  <SGR>이름<리셋>  <SGR>id<리셋> …`, 머리는 `<SGR>이름<리셋>  경로` 다.
/// **이름은 이 자리로 찾는다** — 줄 어디서든 `이름<리셋>` 을 찾으면 무작위 id 의 끝이 이름과
/// 같을 때(`argos-i3p1` 과 `p1`) 남의 줄을 제 줄로 읽어 시험이 흔들렸다(moai-7ccd).
fn name_cell<'a>(line: &'a str, indent: &str) -> Option<(String, &'a str)> {
    let mut rest = line.strip_prefix(indent)?;
    let mut sgr = String::new();
    while let Some(tail) = rest.strip_prefix("\u{1b}[") {
        let end = tail.find('m')?;
        if !tail[..end].chars().all(|c| c.is_ascii_digit() || c == ';') {
            return None;
        }
        sgr.push_str(&rest[..end + 3]);
        rest = &tail[end + 1..];
    }
    let (text, _) = rest.split_once("\u{1b}[0m")?;
    (!sgr.is_empty()).then_some((sgr, text))
}

/// 프로젝트 `name` 의 일 줄 중 `id` 가 든 것.
fn project_row<'a>(painted: &'a str, name: &str, id: &str) -> Option<&'a str> {
    painted.lines().find(|l| name_cell(l, "  ").is_some_and(|(_, n)| n == name) && l.contains(id))
}

/// 프로젝트 `name` 의 머리 줄.
fn project_head<'a>(painted: &'a str, name: &str) -> Option<&'a str> {
    painted.lines().find(|l| !l.starts_with(' ') && name_cell(l, "").is_some_and(|(_, n)| n == name))
}

/// **줄 찾기는 id 의 끝에 속지 않는다**(moai-7ccd). 옛 찾기(`이름<리셋>` 이 줄 어디든 있다)는
/// `p0` 의 줄에 선 `argos-i3p1` 을 `p1` 의 줄로 읽었다 — 그 무작위 id 를 손으로 박아 본다.
#[test]
fn a_project_row_is_found_by_its_name_cell_not_by_an_id_that_ends_like_it() {
    let painted = "\u{1b}[1m\u{1b}[32mp0\u{1b}[0m  /w/p0   1건
  \u{1b}[32mp0\u{1b}[0m   \u{1b}[32margos-i3p1\u{1b}[0m  p2  집을 일

\u{1b}[1m\u{1b}[35mp1\u{1b}[0m  /w/p1   1건
  \u{1b}[35mp1\u{1b}[0m   \u{1b}[35margos-i3p1\u{1b}[0m  p2  집을 일
";
    let old = painted.lines().find(|l| l.starts_with("  ") && l.contains("p1\u{1b}[0m") && l.contains("argos-i3p1")).unwrap();
    assert!(old.contains("\u{1b}[32mp0"), "옛 찾기가 틀리는 자리를 못 만들었다 — {old:?}");
    let row = project_row(painted, "p1", "argos-i3p1").expect("p1 의 줄이 없다");
    assert_eq!(name_cell(row, "  "), Some(("\u{1b}[35m".to_string(), "p1")), "{row:?}");
    let head = project_head(painted, "p1").expect("p1 의 머리가 없다");
    assert_eq!(name_cell(head, ""), Some(("\u{1b}[1m\u{1b}[35m".to_string(), "p1")), "{head:?}");
    assert_eq!(project_row(painted, "p", "argos-i3p1"), None, "이름의 앞부분으로 줄을 찾았다");
    assert_eq!(project_row(painted, "i3p1", ""), None, "id 로 줄을 찾았다");
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
            let row = project_row(&painted, n, id).unwrap_or_else(|| panic!("{n} 의 줄이 없다 — {painted}"));
            // 칠하지 않은 이름 칸은 `name_cell` 이 칸으로 안 읽어 위의 찾기에서 이미 멈춘다.
            let (name_sgr, id_sgr) = (name_cell(row, "  ").unwrap().0, sgr_before(row, id));
            assert_eq!(name_sgr, id_sgr, "{n} 의 이름 칸과 id 칸 색이 다르다 — {row:?}");
            let head = project_head(&painted, n).unwrap_or_else(|| panic!("{n} 의 머리가 없다 — {painted}"));
            assert!(name_cell(head, "").unwrap().0.contains(&name_sgr), "{n} 의 머리가 줄과 다른 색이다 — {head:?}");
            hues.insert(name_sgr);
        }
        assert!(hues.len() > 1, "열세 프로젝트가 한 색이다 — {painted}");
    }

    // 다른 프로젝트를 빼고 차례를 바꿔도 제 색이 그대로다 — 등록 순서가 아니라 경로로 고른다.
    let colour_of = |cfg: &Path, n: &str| {
        let t = run(cfg, &["ready"], true);
        let row = project_row(&t, n, &todo).unwrap_or_else(|| panic!("{n} 의 줄이 없다 — {t}"));
        name_cell(row, "  ").unwrap().0
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
/// 몰라 거절한다. 전체는 한눈 보기와 같은 객체(`projects`·`problems`·`config`)라, 등록한 것이
/// 없거나 설정이 깨져도 JSON 으로 0 이다(moai-yxae).
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
        "{{\"projects\":[{{\"title\":\"good\",\"kind\":\"project\",\"dir\":true,\"path\":{:?},\"state\":\"ok\",\"counts\":{{",
        good.to_str().unwrap()
    );
    assert!(rows.starts_with(&good_at), "{rows}");
    assert!(rows.contains(&format!("\"picked\":[\"{picked}\"]")) && rows.contains("\"in_progress\":1"), "{rows}");
    let bare_at = format!(
        "{{\"title\":\"bare\",\"kind\":\"project\",\"dir\":false,\"path\":{:?},\"state\":\"uninitialized\"}}],\"problems\":[],\"config\":{:?}}}",
        bare.to_str().unwrap(),
        cfg.to_str().unwrap()
    );
    assert!(rows.trim_end().ends_with(&bare_at), "{rows}");

    // 들어가는 손잡이는 디렉터리다 — 거기서 부르면 오늘의 `tui --json` 이다.
    let inside = ok_with(&out, &cfg, &["-C", good.to_str().unwrap(), "tui", "--json"]);
    assert!(inside.contains(&format!("\"id\":\"{picked}\"")), "{inside}");

    let o = moai_with(&out, &cfg, &["tui", "--json", "--path", &picked]);
    assert!(!o.status.success() && String::from_utf8_lossy(&o.stderr).contains("-C <dir> tui --path"), "{}", text(&o));

    // 등록한 것이 없어도 같은 객체로 0 이다(moai-yxae) — stdout 에 JSON 이 없던 자리다.
    let empty = registry(&s, &[]);
    let none = ok_with(&out, &empty, &["tui", "--json"]);
    one_json_value(&none);
    assert!(none.starts_with("{\"projects\":[],\"problems\":[]"), "{none}");

    // 사용자 설정의 문제는 stderr 가 아니라 `problems` 에 선다.
    std::fs::write(&empty, "not [toml\n").unwrap();
    let o = moai_with(&out, &empty, &["tui", "--json"]);
    let said = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{}", text(&o));
    one_json_value(&said);
    assert!(said.starts_with("{\"projects\":[],\"problems\":[\""), "{said}");
    assert!(o.stderr.is_empty(), "설정 문제를 stderr 로도 냈다 — {}", String::from_utf8_lossy(&o.stderr));
}

/// 등록한 것이 없으면 등록하는 길을 댄다. 설정 파일이 깨져 목록이 빈 것이면 그 까닭도 함께
/// 말한다. **`status` 는 0 으로 끝난다**(moai-ynsb) — 세션의 시작점이 제 파일 아닌 것으로
/// 실패해 보이면 안 된다. `ready` 는 여전히 멈춘다.
#[test]
fn outside_a_repo_with_nothing_registered_it_says_how_to_register() {
    let s = Scratch::new("ovempty");
    let out = dir_in(&s, "out");
    let cfg = registry(&s, &[]);
    let st = ok_with(&out, &cfg, &["status"]);
    assert!(st.contains("moai init") && st.contains("moai project add"), "{st}");
    let js = ok_with(&out, &cfg, &["status", "--json"]);
    one_json_value(&js);
    assert!(js.starts_with("{\"projects\":[],\"problems\":[]"), "{js}");

    let o = moai_with(&out, &cfg, &["ready"]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!o.status.success(), "ready");
    assert!(err.contains("moai init") && err.contains("moai project add"), "ready: {err}");
    let help = ok_with(&out, &cfg, &[]);
    assert!(help.contains("moai init") && help.contains("moai project add"), "{help}");

    // **탐색기만 다르다**(moai-r8kl) — 사람이 보는 화면이라 빈 층을 열고 `SPC p a` 를 댄다. 여기는
    // 터미널이 아니라 그 까닭으로 멈추고, 등록이 없다는 말로는 안 멈춘다. `--json` 은 `status --json`
    // 과 같은 빈 객체로 0 이다(moai-yxae).
    let o = moai_with(&out, &cfg, &["tui"]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!o.status.success() && err.contains("터미널이 아니라") && !err.contains("moai project add"), "{err}");
    let js = ok_with(&out, &cfg, &["tui", "--json"]);
    assert!(js.starts_with("{\"projects\":[]"), "{js}");

    // 깨진 설정도 0 이고, 목록이 빈 까닭을 댄다 — 모양이 틀린 것도, TOML 이 아닌 것도.
    std::fs::write(&cfg, "project = 3\n").unwrap();
    let st = ok_with(&out, &cfg, &["status"]);
    assert!(st.contains("표 배열") && st.contains("moai project add"), "{st}");
    std::fs::write(&cfg, "not [toml\n").unwrap();
    let st = ok_with(&out, &cfg, &["status"]);
    assert!(st.contains("moai project add") && st.lines().count() > 2, "까닭을 안 댔다 — {st}");
    let js = ok_with(&out, &cfg, &["status", "--json"]);
    one_json_value(&js);
    assert!(js.starts_with("{\"projects\":[],\"problems\":[\""), "{js}");
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

/// **시작·끝 시각은 칸을 옮기는 그 쓰기에 실린다**(moai-38mh).
///
/// 시작은 **처음** 첫 칸을 떠난 때 하나고 되집어도 안 덮는다 — `status_since` 는 칸을
/// 옮길 때마다 새로 서므로 집은 때가 review·done 에서 사라진다. 끝은 done 에 들 때마다
/// 덮고 done 을 떠나도 **안 지운다**: 소요가 되돌린 판까지 품는다.
#[test]
fn mv_stamps_the_first_start_and_the_last_finish() {
    let s = init("mvtimes");
    let id = add(s.path(), &["제목"]);
    assert!(!line_of(s.path(), &id).contains("started_at"), "첫 칸에 선 줄에 시작이 섰다");

    let t1 = "2026-09-11T05:00:00Z";
    assert!(at(s.path(), t1, &["mv", &id, "in_progress"]).status.success());
    let line = line_of(s.path(), &id);
    assert!(line.contains(&format!(r#""started_at":"{t1}""#)), "{line}");
    assert!(!line.contains("done_at"), "안 끝난 줄에 끝난 때가 섰다 — {line}");

    // 다음 칸으로 가도 시작은 그대로다. 새로 서는 것은 `status_since` 뿐이다.
    let t2 = "2026-09-11T06:00:00Z";
    assert!(at(s.path(), t2, &["mv", &id, "review"]).status.success());
    let line = line_of(s.path(), &id);
    assert!(line.contains(&format!(r#""started_at":"{t1}""#)), "{line}");
    assert!(line.contains(&format!(r#""status_since":"{t2}""#)), "{line}");

    let t3 = "2026-09-11T07:00:00Z";
    assert!(at(s.path(), t3, &["mv", &id, "done"]).status.success());
    assert!(line_of(s.path(), &id).contains(&format!(r#""done_at":"{t3}""#)));

    // 되돌려도 끝난 때는 안 지워진다 — "아직 안 끝났다" 는 `status` 가 말한다.
    let t4 = "2026-09-11T08:00:00Z";
    assert!(at(s.path(), t4, &["mv", &id, "todo"]).status.success());
    let line = line_of(s.path(), &id);
    assert!(line.contains(&format!(r#""done_at":"{t3}""#)), "되집었다고 끝난 때를 지웠다 — {line}");

    // 다시 집어도 시작은 처음 것이고, 다시 닫으면 끝은 마지막 것이다.
    let t5 = "2026-09-11T09:00:00Z";
    assert!(at(s.path(), t5, &["mv", &id, "in_progress"]).status.success());
    assert!(line_of(s.path(), &id).contains(&format!(r#""started_at":"{t1}""#)));
    let t6 = "2026-09-11T10:00:00Z";
    assert!(at(s.path(), t6, &["mv", &id, "done"]).status.success());
    let line = line_of(s.path(), &id);
    assert!(line.contains(&format!(r#""started_at":"{t1}""#)), "{line}");
    assert!(line.contains(&format!(r#""done_at":"{t6}""#)), "{line}");

    // 첫 칸에서 곧바로 닫은 줄도 시작한 줄이다 — 떠난 때가 시작이다.
    let quick = add(s.path(), &["곧바로 닫는다"]);
    assert!(at(s.path(), t2, &["mv", &quick, "done"]).status.success());
    let line = line_of(s.path(), &quick);
    assert!(line.contains(&format!(r#""started_at":"{t2}""#)), "{line}");
    assert!(line.contains(&format!(r#""done_at":"{t2}""#)), "{line}");
}

/// 적히기만 하고 어느 화면에도 안 서는 필드는 틀려도 아무도 모른다 — 상세와 `--json` 둘 다
/// 같은 값을 낸다. 아직 첫 칸인 줄에는 그 줄을 안 세운다.
#[test]
fn the_start_and_finish_stamps_reach_both_surfaces() {
    let s = init("mvtimeshow");
    let id = add(s.path(), &["제목"]);
    assert!(!ok(s.path(), &["show", &id]).contains("시작"), "안 집은 줄이 시작을 말한다");

    assert!(at(s.path(), "2026-09-11T05:00:00Z", &["mv", &id, "in_progress"]).status.success());
    let text = ok(s.path(), &["show", &id]);
    assert!(text.contains("시작   2026-09-11 05:00"), "{text}");
    // 아직 안 끝났다 — 자리는 서되 값이 없다.
    assert!(text.contains("끝    —"), "{text}");
    let json = ok(s.path(), &["show", &id, "--json"]);
    assert!(json.contains(r#""started_at":"2026-09-11T05:00:00Z""#), "{json}");
    assert!(!json.contains("done_at"), "{json}");
}

/// **이미 첫 칸을 떠난 줄에는 거짓 시작을 안 적는다**(리뷰 moai-u5bk.3wq). 이 필드 전에 집은 줄 —
/// 옛 바이너리가 옮긴 줄 — 을 다음에 옮긴 때로 적으면, review 에서 닫는 순간 시작과 끝이 같아져
/// 통계가 "0 분에 했다" 로 읽는다. 빈 칸과 0 과 거짓 값은 셋 다 다르다 — 모르면 비우고 끝만 적는다.
#[test]
fn a_line_already_under_way_gets_no_false_start() {
    let s = init("mvlegacy");
    let id = add(s.path(), &["옛 바이너리가 집은 줄"]);
    // 옛 바이너리가 09-05 에 review 까지 옮긴 모습 — 칸만 옮기고 시작 시각은 모른다.
    let old = line_of(s.path(), &id);
    let moved = old
        .replace(r#""status":"todo""#, r#""status":"review""#)
        .replace(&format!(r#""status_since":"{NOW}""#), r#""status_since":"2026-09-05T00:00:00Z""#);
    assert_ne!(old, moved, "시험이 줄을 못 고쳤다");
    std::fs::write(s.path().join(".moai/issues.jsonl"), issues(s.path()).replace(&old, &moved)).unwrap();

    let t = "2026-09-12T09:00:00Z";
    assert!(at(s.path(), t, &["mv", &id, "done"]).status.success());
    let line = line_of(s.path(), &id);
    assert!(!line.contains("started_at"), "모르는 시작을 닫은 때로 적었다 — {line}");
    assert!(line.contains(&format!(r#""done_at":"{t}""#)), "{line}");

    // 상세는 모르는 시작을 `—` 로 대고, `끝` 은 윗줄의 `수정` 과 같은 자리에 선다.
    let text = ok(s.path(), &["show", &id]);
    let row = |head: &str| {
        text.lines()
            .find(|l| l.trim_start().starts_with(head))
            .unwrap_or_else(|| panic!("{head} 줄이 없다\n{text}"))
            .to_string()
    };
    let col = |l: &str, w: &str| l.find(w).map(|b| l[..b].chars().count());
    assert!(row("시작").contains('—'), "{text}");
    assert_eq!(col(&row("시작"), "끝"), col(&row("생성"), "수정"), "끝이 수정 밑에 안 섰다\n{text}");
}

/// **칸에 드는 길이 어느 것이든 같은 줄이 선다**(리뷰 moai-u5bk.3wq). 첫 칸 밖에서 만든 줄
/// (`add -s`)은 만든 때가 곧 첫 칸을 떠난 때고, `done` 에서 만든 줄은 그때 끝났다. 펼쳐 닫은
/// 생각도 `mv` 로 닫은 생각과 같은 시각을 든다 — 한때 `mv` 만 시각을 적었다.
#[test]
fn every_way_into_a_column_stamps_the_same() {
    let s = init("stampways");
    let made = |args: &[&str]| {
        let out = at(s.path(), NOW, args);
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };

    let going = made(&["add", "만들 때 집은 일", "-s", "in_progress", "-q"]);
    let line = line_of(s.path(), &going);
    assert!(line.contains(&format!(r#""started_at":"{NOW}""#)) && !line.contains("done_at"), "{line}");
    assert!(at(s.path(), "2026-09-12T05:00:00Z", &["mv", &going, "review"]).status.success());
    assert!(line_of(s.path(), &going).contains(&format!(r#""started_at":"{NOW}""#)), "만든 때의 시작을 덮었다");

    let finished = made(&["add", "이미 끝낸 일", "-s", "done", "-q"]);
    let line = line_of(s.path(), &finished);
    assert!(line.contains(&format!(r#""done_at":"{NOW}","started_at":"{NOW}""#)), "{line}");

    let untouched = line_of(s.path(), &made(&["add", "안 집은 일", "-q"]));
    assert!(!untouched.contains("started_at") && !untouched.contains("done_at"), "첫 칸에서 난 줄에 시각을 적었다 — {untouched}");

    let thought = made(&["idea", "add", "펼칠 생각", "-q"]);
    let out = from_stdin(s.path(), &["idea", "promote", &thought, "--from", "-"], "# 펼친 에픽\n- 첫 일\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let line = line_of(s.path(), &thought);
    assert!(line.contains(&format!(r#""done_at":"{NOW}","started_at":"{NOW}""#)), "펼쳐 닫은 생각에 시각이 없다 — {line}");
}

/// **묶음은 제 줄의 시작·끝을 상세에 안 그린다**(리뷰 moai-u5bk.3wq) — 묶음의 칸은 멤버에서 읽으므로,
/// 손으로 친 `mv <에픽> done` 이 남긴 시각이 서면 도는 에픽 밑에 `끝` 이 선다. 기계 출력은 적힌 값을
/// 그대로 낸다 — 적힌 칸(`status`)을 그대로 내는 것과 같은 약속이다.
#[test]
fn a_group_does_not_show_its_own_stamps() {
    let s = init("groupstamps");
    let epic = ok(s.path(), &["epic", "add", "에픽", "-q"]).trim().to_string();
    let member = add(s.path(), &["멤버", "-e", &epic]);
    ok(s.path(), &["mv", &member, "in_progress"]);
    ok(s.path(), &["mv", &epic, "done"]);
    let text = ok(s.path(), &["show", &epic]);
    assert!(!text.lines().any(|l| l.trim_start().starts_with("시작")), "도는 에픽 밑에 제 줄의 시작·끝을 그렸다\n{text}");
    assert!(ok(s.path(), &["show", &member]).lines().any(|l| l.trim_start().starts_with("시작")), "멤버의 시작까지 걷었다");
    assert!(ok(s.path(), &["show", &epic, "--json"]).contains("\"done_at\""), "기계 출력이 적힌 값을 걷었다");
}

/// **깨진 저널 한 줄이 목록도 이력도 넘어뜨리지 않는다**(리뷰 moai-u5bk.3wq). 목록 `--json` 이 `work` 를
/// 읽으려 저널을 읽는데, 파일을 통째로 UTF-8 로 읽으면 글자 가운데서 끊긴 덧붙이기 한 줄(디스크가
/// 찼다·죽었다)이 그 뒤로 모든 목록을 실패로 만든다. 깨진 줄만 건너뛴다 — 저널이 늘 해 온 약속이다.
#[test]
fn a_torn_journal_line_breaks_neither_the_list_nor_the_history() {
    let s = init("tornjournal");
    let id = add(s.path(), &["일"]);
    ok(s.path(), &["note", &id, "model: anthropic/opus-5 tokens=7 (low — 멀쩡한 줄)"]);
    let path = s.path().join(".moai/journal.jsonl");
    let mut torn = std::fs::read(&path).unwrap();
    // 한글 한 글자(`한` = ED 95 9C)의 앞 두 바이트에서 끊긴 덧붙이기.
    torn.extend_from_slice(format!(r#"{{"ts":"{NOW}","id":"{id}","kind":"note","by":"t","text":"model: x "#).as_bytes());
    torn.extend_from_slice(b"\xed\x95");
    std::fs::write(&path, &torn).unwrap();

    let list = ok(s.path(), &["show", "--json"]);
    assert!(list.contains(r#""tokens":7"#), "깨진 줄 하나 때문에 멀쩡한 줄까지 잃었다\n{list}");
    assert!(ok(s.path(), &["show", &id, "--json"]).contains(r#""tokens":7"#));
    assert!(ok(s.path(), &["show", &id]).contains("멀쩡한 줄"), "이력이 넘어졌다");
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

/// **마일스톤 줄에는 `--milestone <id>` 를 못 적는다**(moai-bg55, 사용자와 정함). 그 필드는
/// 소속으로 안 세므로(moai-jwnr) 조용히 받으면 적은 사람은 걸린 줄 안다. 만들 때도 고칠
/// 때도 같은 자로 거절하고 파일은 한 바이트도 안 바뀐다. **엄함은 지금 쓰는 줄에만** —
/// 이미 그 필드를 든 옛 줄은 다른 쓰기를 막지 않고, 그 줄을 손댈 때 비우는 길을 댄다.
#[test]
fn a_milestone_line_refuses_a_milestone_but_old_lines_do_not_block() {
    let s = init("stonestone");
    let m2 = ok(s.path(), &["milestone", "add", "M2", "-q"]).trim().to_string();
    let m1 = ok(s.path(), &["milestone", "add", "M1", "-q"]).trim().to_string();
    let refused = |args: &[&str], id: &str| {
        let before = issues(s.path());
        let out = moai(s.path(), args);
        assert!(!out.status.success(), "{args:?} 를 받았다");
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(err.contains("다른 마일스톤에 들지 않는다") && err.contains(&format!("moai edit {id} --milestone none")), "{args:?}: {err:?}");
        assert_eq!(issues(s.path()), before, "{args:?}: 거절했는데 파일이 바뀌었다");
    };
    refused(&["edit", &m1, "--milestone", &m2], &m1);
    let before = issues(s.path());
    let out = moai(s.path(), &["milestone", "add", "M3", "--milestone", &m2]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success() && err.contains("다른 마일스톤에 들지 않는다"), "만들 때 받았다 — {err:?}");
    // 만들 때는 **빼라고** 댄다. 거절된 새 줄의 id 는 저장되지 않으므로, 그 id 로 `moai edit` 를
    // 치라고 대면 시킨 대로 친 명령이 "못 찾았다" 로 끝난다(리뷰 moai-bg55.oya).
    assert!(err.contains("`--milestone` 을 빼고"), "만들 때 뺄 길을 안 댔다 — {err:?}");
    assert_eq!(issues(s.path()), before, "거절한 만들기가 파일을 바꿨다");
    // 이슈·에픽은 여전히 마일스톤에 든다.
    add(s.path(), &["일", "--milestone", &m2]);
    add(s.path(), &["에픽", "--type", "epic", "--milestone", &m2]);

    // 옛 바이너리가 쓴 줄 — 필드를 든 마일스톤 줄을 파일에 직접 둔다.
    let file = s.path().join(".moai/issues.jsonl");
    let src = std::fs::read_to_string(&file).unwrap();
    let old = src
        .lines()
        .map(|l| match l.contains(&format!("\"id\":\"{m1}\"")) {
            true => l.replacen("\"created_at\"", &format!("\"milestone\":\"{m2}\",\"created_at\""), 1),
            false => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&file, &old).unwrap();
    // 그 줄을 안 건드리는 쓰기는 지나간다.
    add(s.path(), &["딴 일"]);
    assert!(line_of(s.path(), &m1).contains(&format!("\"milestone\":\"{m2}\"")), "안 건드린 옛 줄의 필드를 지웠다");
    // 그 줄을 손대면 거절하고 비우는 길을 댄다 — 그 길을 치면 지나간다.
    refused(&["edit", &m1, "--title", "M1 새 제목"], &m1);
    ok(s.path(), &["edit", &m1, "--milestone", "none"]);
    // 키로 찾는다 — 마일스톤 줄은 `"kind":"milestone"` 을 들어 낱말만 찾으면 늘 걸린다.
    assert!(!line_of(s.path(), &m1).contains("\"milestone\":"), "--milestone none 이 옛 줄을 못 비웠다");
}

/// **다른 마일스톤을 적어도 에픽·조상이 이기면 그렇다고 말한다** (moai-mhxf). 필드는 X 가
/// 되는데 줄은 에픽·조상이 선 곳에 그대로 서 `show --milestone X` 가 조용히 그 줄을 못 낸다 —
/// `none` 이 못 끊는 것(moai-0lmn)과 같은 어긋남이라 같은 자리에서 한 줄로 댄다.
#[test]
fn edit_says_when_another_milestone_loses_to_the_epic_or_an_ancestor() {
    let s = init("editlostms");
    let m1 = ok(s.path(), &["milestone", "add", "v1", "-q"]).trim().to_string();
    let m2 = ok(s.path(), &["milestone", "add", "v2", "-q"]).trim().to_string();
    let epic = add(s.path(), &["에픽", "--type", "epic", "--milestone", &m1]);
    let bare = add(s.path(), &["마일스톤 없는 에픽", "--type", "epic"]);
    let member = add(s.path(), &["멤버", "-e", &epic]);
    let loose = add(s.path(), &["빈 에픽의 멤버", "-e", &bare]);
    let parent = add(s.path(), &["부모", "--milestone", &m1]);
    let child = add(s.path(), &["자식", "--parent", &parent]);
    let top = add(s.path(), &["홀로 선 이슈", "--milestone", &m1]);

    let says = |id: &str, from: &str, stood: &str| -> String {
        let out = moai(s.path(), &["edit", id, "--milestone", &m2]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let lines: Vec<&str> = err.lines().filter(|l| l.contains(from)).collect();
        assert_eq!(lines.len(), 1, "{id}: {from} 에게 진 것을 한 줄로 안 댔다 — {err:?}");
        assert!(lines[0].contains(stood) && lines[0].contains(&format!("--milestone {m2}")), "{id}: {err:?}");
        assert!(!ok(s.path(), &["show", "--milestone", &m2]).contains(id), "{id} 가 {m2} 에 섰다");
        err
    };
    says(&member, &format!("에픽 {epic}"), &m1);
    says(&loose, &format!("에픽 {bare}"), "어느 마일스톤에도 안 든다");
    says(&child, &format!("조상 {parent}"), &m1);

    // 마일스톤 밑에 id 로 선 부모의 자식 — 비워서는 못 끊지만 다른 마일스톤은 접힌 맨 위 줄
    // (그 부모)에 적으면 옮겨진다. "id 가 마일스톤 밑이라 못 옮긴다" 고 하면 헛말이다(리뷰 moai-z3lo.dxz).
    let under = add(s.path(), &["마일스톤 밑", "--parent", &m1]);
    let grand = add(s.path(), &["그 밑", "--parent", &under]);
    let err = says(&grand, &format!("조상 {under}"), &m1);
    assert!(err.contains(&format!("`moai edit {under} --milestone {m2}`")), "옮길 길을 안 댔다 — {err:?}");
    let json = ok(s.path(), &["edit", &grand, "--milestone", &m2, "--json"]);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":"{m1}","parent":"{under}"}}"#)), "{json}");
    // 뿌리로 올라간 생각 밑 — 어느 필드로도 못 옮기니 길은 안 대고, 조용하지도 않다.
    let thought = ok(s.path(), &["idea", "add", "생각", "-q"]).trim().to_string();
    let pinned = add(s.path(), &["생각 밑", "--parent", &thought]);
    let err = says(&pinned, &format!("id 가 {thought} 밑에"), "어느 마일스톤에도 안 든다");
    assert!(!err.contains("moai edit"), "고쳐도 안 바뀌는 길을 댔다 — {err:?}");
    // 못 쓸 에픽의 멤버는 `(길 잃음)` 에 선다 — 없는 에픽의 마일스톤을 고치라고 대지 않는다.
    let gone = add(s.path(), &["지울 에픽", "--type", "epic", "--milestone", &m1]);
    let stray = add(s.path(), &["길 잃을 멤버", "-e", &gone]);
    ok(s.path(), &["rm", &gone]);
    let err = says(&stray, &format!("에픽 {gone}"), "어느 마일스톤에도 안 든다");
    assert!(!err.contains(&format!("moai edit {gone}")), "없는 줄을 고치라고 댔다 — {err:?}");

    let json = ok(s.path(), &["edit", &member, "--milestone", &m2, "--json"]);
    one_json_value(&json);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":"{m1}","epic":"{epic}"}}"#)), "{json}");
    let json = ok(s.path(), &["edit", &loose, "--milestone", &m2, "--json"]);
    assert!(json.contains(&format!(r#""inherited_milestone":{{"milestone":null,"epic":"{bare}"}}"#)), "{json}");

    // 적은 것이 선 것과 같으면, 제 필드가 답이면, 안 적었으면 조용하다.
    for args in [
        vec!["edit", member.as_str(), "--milestone", m1.as_str()],
        vec!["edit", top.as_str(), "--milestone", m2.as_str()],
        vec!["edit", child.as_str(), "--tag", "x"],
    ] {
        let out = moai(s.path(), &args);
        assert!(out.status.success());
        assert!(out.stderr.is_empty(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let mut j = args.clone();
        j.push("--json");
        assert!(!ok(s.path(), &j).contains("inherited_milestone"), "{args:?}");
    }
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

/// **상세가 이 이슈에 닿은 커밋을 그린다**(moai-emcv) — 커밋 제목에 id 를 적은 것을 git 에서
/// 읽는다. 트래커 커밋은 사람 화면에서 빼고 `--json` 에는 표시와 함께 낸다. 자식의 커밋은
/// 부모의 것이 아니다. git 저장소가 아니면 칸도 키도 없이 상세가 그대로 열린다.
///
/// **이슈보다 먼저 찍힌 커밋도 낸다**(moai-hws2) — 그 id 를 적었으면 그 이슈의 커밋이다. 한때는
/// 생성일에서 걷기를 끊어 그런 커밋을 뺐는데, `--since` 는 거르기가 아니라 끊기라 날짜가 거꾸로 선
/// 커밋 하나가 그 밑을 통째로 가렸다(`show_sees_commits_under_a_backdated_one`).
///
/// **커밋 시각은 그래도 고정한다** — 차례는 커밋 시각이 정하고, 기계 시계로 찍으면 `LATER` 로 찍은
/// 커밋들과의 앞뒤가 돌리는 기계마다 달라진다.
#[test]
fn show_draws_the_commits_that_name_the_issue() {
    let s = init("showcommits");
    let a = add(s.path(), &["고칠 것"]);
    git(s.path(), &["init", "-q"]);
    // 이슈보다 이틀 먼저 찍힌 커밋 — rebase 가 옛 시각을 옮긴 꼴이다. 그 id 를 적었으니 센다.
    git_at(s.path(), "2026-09-09T00:00:00Z", &["commit", "-q", "--allow-empty", "-m", &format!("feat: 옛것 ({a})")]);
    git_at(s.path(), LATER, &["commit", "-q", "--allow-empty", "-m", &format!("chore(tracker): {a} 를 워크트리에서 집는다")]);
    git_at(s.path(), LATER, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 고친다 ({a})")]);
    git_at(s.path(), LATER, &["commit", "-q", "--allow-empty", "-m", &format!("fix: 리뷰 ({a}.x1y)")]);
    let hash = git(s.path(), &["rev-parse", "HEAD~1"]).trim().to_string();

    let shown = ok(s.path(), &["show", &a]);
    assert!(shown.contains(&format!("{}   feat: 고친다 ({a})", &hash[..7])), "고친 커밋이 없다\n{shown}");
    assert!(!shown.contains("chore(tracker)"), "트래커 커밋을 사람 화면에 그렸다\n{shown}");
    assert!(!shown.contains("fix: 리뷰"), "자식의 커밋을 부모에 그렸다\n{shown}");
    assert!(shown.contains("옛것"), "이슈보다 먼저 찍힌 커밋을 뺐다 — 그 id 를 적은 커밋이다\n{shown}");
    let (commits_at, history_at) = (shown.find("\n커밋\n"), shown.find("\n이력\n"));
    assert!(commits_at.is_some() && commits_at < history_at, "커밋이 이력 앞에 안 섰다\n{shown}");

    let json = ok(s.path(), &["show", &a, "--json"]);
    assert!(
        json.contains(&format!("\"commits\":[{{\"hash\":\"{hash}\",\"subject\":\"feat: 고친다 ({a})\",\"tracker\":false}}")),
        "새 커밋이 먼저, 해시는 줄이지 않고 낸다\n{json}"
    );
    assert!(json.contains("\"tracker\":true"), "트래커 커밋을 --json 에서도 뺐다\n{json}");

    let bare = init("showcommitsbare");
    let b = add(bare.path(), &["git 밖"]);
    let shown = ok(bare.path(), &["show", &b]);
    assert!(!shown.contains("\n커밋\n"), "{shown}");
    // `--json` 은 빈 배열과 까닭을 낸다(moai-rzsv) — 그 가름은 `json_tells_no_commits_apart_from_no_git` 이 본다.
    assert!(ok(bare.path(), &["show", &b, "--json"]).contains(r#""commits":[]"#), "빈 배열을 안 냈다");
}

/// **git 을 못 쓰는 자리에서 상세는 말없이 열린다**(moai-mauw) — git 이 PATH 에 없을 때도, 커밋이
/// 하나도 없는 저장소에서도. 커밋 칸만 비고, 종료 코드도 표준 오류도 그대로다.
#[test]
fn show_opens_quietly_where_git_cannot_answer() {
    let s = init("showcommitsnogit");
    let a = add(s.path(), &["고칠 것"]);
    let quiet = |out: Output, why: &str| {
        let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success() && stdout.contains("고칠 것"), "{why}: 상세가 안 열렸다\n{stdout}\n{stderr}");
        assert!(stderr.is_empty() && !stdout.contains("\n커밋\n"), "{why}: 말없이 비우지 않았다\n{stdout}\n{stderr}");
    };

    let empty = s.path().join("no-git-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let no_git = isolated(BIN)
        .args(["show", &a])
        .current_dir(s.path())
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .env("PATH", &empty)
        .output()
        .expect("moai 를 실행하지 못했다");
    quiet(no_git, "git 이 없다");

    git(s.path(), &["init", "-q"]);
    quiet(moai(s.path(), &["show", &a]), "커밋이 없는 저장소");
}

/// **`edit` 뒤의 상세도 막음을 그린다**(moai-xe74) — `show <id>` 와 글자까지 같은 줄이다.
/// 막는 줄이 끝나면 풀림으로 바뀐 것도 쓴 그 자리에서 보인다.
#[test]
fn edit_detail_draws_the_same_blocker_lines_as_show() {
    let s = init("editblocks");
    let a = add(s.path(), &["먼저 할 것"]);
    let b = add(s.path(), &["막히는 것"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    let line = |out: &str| out.lines().find(|l| l.contains(a.as_str()) && !l.starts_with(b.as_str())).unwrap_or_default().to_string();

    let edited = ok(s.path(), &["edit", &b, "-p", "1"]);
    assert!(line(&edited).contains("막힘") && line(&edited).contains("먼저 할 것"), "edit 상세에 막음 줄이 없다\n{edited}");
    assert_eq!(line(&edited), line(&ok(s.path(), &["show", &b])), "edit 과 show 가 같은 막음을 다르게 그린다");

    ok(s.path(), &["mv", &a, "done"]);
    let edited = ok(s.path(), &["edit", &b, "-p", "2"]);
    assert!(line(&edited).contains("풀림"), "끝난 막음이 풀림으로 안 섰다\n{edited}");
    assert_eq!(line(&edited), line(&ok(s.path(), &["show", &b])));
}

/// **덧붙인 키가 이긴다**(moai-kgu2) — 줄이 모르는 필드로 `children`·`blockers` 를 들고 있어도
/// `show --json` 은 그 키를 한 번씩만, 우리 값으로 낸다. 파일은 한 바이트도 안 바뀐다.
#[test]
fn show_json_keys_win_over_unknown_fields_of_the_same_name() {
    let s = init("showkeys");
    let a = add(s.path(), &["막는 것"]);
    let b = add(s.path(), &["막히는 것"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    let doctored = issues(s.path()).replace(
        &format!("\"id\":\"{b}\","),
        &format!("\"id\":\"{b}\",\"children\":[\"가짜\"],\"blockers\":\"가짜\","),
    );
    std::fs::write(s.path().join(".moai/issues.jsonl"), &doctored).unwrap();

    let json = ok(s.path(), &["show", &b, "--json"]);
    assert_eq!(json.matches("\"children\":").count(), 1, "children 키가 둘 섰다\n{json}");
    assert_eq!(json.matches("\"blockers\":").count(), 1, "blockers 키가 둘 섰다\n{json}");
    assert!(json.contains("\"children\":[]") && !json.contains("가짜"), "모르는 필드가 덧붙인 키를 이겼다\n{json}");
    assert!(json.contains(&format!("\"blockers\":[{{\"id\":\"{a}\"")), "{json}");
    assert_eq!(issues(s.path()), doctored, "출력에서 걷으려다 파일을 바꿨다");
}

/// **조건부 키는 이번에 안 실었으면 되쓴 줄에서도 안 나간다**(moai-2l8n). `--json` 을 파일에
/// 되써 넣은 줄은 그때의 조건부 키(`commits_error`·`blockers`·…)를 모르는 필드로 들고 있다.
/// 덧붙인 키만 걷으면, 그 조건이 아닌 지금 옛 값이 그대로 딸려 나가 거짓을 말한다 — git 이
/// 멀쩡한데 `commits` 옆에 "못 읽었다" 가 선다. 파일은 한 바이트도 안 바뀐다.
///
/// 되쓴 줄은 **다른 명령의 출력**에서도 온다 — `edit --json` 의 `inherited_*` 도 여기서 걷는다.
#[test]
fn show_json_drops_stale_conditional_keys_carried_by_a_rewritten_line() {
    let s = init("stalekeys");
    let id = add(s.path(), &["되쓴 줄"]);
    git(s.path(), &["init", "-q"]);
    git_at(s.path(), NOW, &["commit", "-q", "--allow-empty", "-m", "chore: 첫 커밋"]);
    let stale = [
        ("commits_error", r#""가짜""#),
        ("blockers", r#"[{"id":"가짜"}]"#),
        ("shelved_by", r#""가짜""#),
        ("workplaces", r#"["가짜"]"#),
        ("place", r#""가짜""#),
        ("duplicate_lines", "2"),
        ("members", r#"["가짜"]"#),
        ("inherited_epic", r#"{"epic":"가짜","parent":"가짜"}"#),
        ("inherited_milestone", r#"{"milestone":"가짜"}"#),
    ];
    let fields: Vec<String> = stale.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    let doctored = issues(s.path()).replace(
        &format!("\"id\":\"{id}\","),
        &format!("\"id\":\"{id}\",{},\"due\":\"2026-10-01\",", fields.join(",")),
    );
    std::fs::write(s.path().join(".moai/issues.jsonl"), &doctored).unwrap();

    let json = ok(s.path(), &["show", &id, "--json"]);
    for (k, _) in &stale {
        assert!(!json.contains(&format!("\"{k}\"")), "이번에 안 실은 {k} 가 되쓴 줄에서 딸려 나왔다\n{json}");
    }
    assert!(json.contains(r#""commits":[]"#), "{json}");
    assert!(json.contains(r#""due":"2026-10-01""#), "조건부 키가 아닌 모르는 필드까지 걷었다\n{json}");
    assert_eq!(issues(s.path()), doctored, "출력에서 걷으려다 파일을 바꿨다");
}

/// **줄을 내는 명령 전부가 같은 목록으로 걷는다**(moai-qn5d). 걷기가 `show <id> --json` 에만 있으면
/// 되쓴 줄을 `ready`·`show` 목록·`edit`·`mv` 가 그대로 펴서, 막음 없는 일에 옛 `blockers` 가,
/// 멀쩡한 git 옆에 옛 `commits_error` 가 선다. 걷는 자리는 `Row::of` 하나다.
///
/// 사용자가 같은 이름으로 둔 제 필드도 줄 출력에서 숨는다 — 받은 값이다(2026-09-18 사용자 결정 A).
/// 파일은 그대로다.
#[test]
fn every_line_printing_command_drops_keys_moai_appends() {
    let s = init("stalerows");
    let id = add(s.path(), &["되쓴 줄"]);
    let stale = [
        ("derived_status", r#""가짜""#),
        ("branch", r#""가짜""#),
        ("children", r#"["가짜"]"#),
        ("journal", r#"["가짜"]"#),
        ("members", r#"["가짜"]"#),
        ("shelved_by", r#""가짜""#),
        ("duplicate_lines", "2"),
        ("blockers", r#"[{"id":"가짜"}]"#),
        ("workplaces", r#"["가짜"]"#),
        ("place", r#""lost""#),
        ("commits", r#"["가짜"]"#),
        ("commits_error", r#""가짜""#),
        ("work", r#"["가짜"]"#),
        ("inherited_epic", r#"{"epic":"가짜","parent":"가짜"}"#),
        ("inherited_milestone", r#"{"milestone":"가짜"}"#),
    ];
    let fields: Vec<String> = stale.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    let doctored = issues(s.path()).replace(
        &format!("\"id\":\"{id}\","),
        &format!("\"id\":\"{id}\",{},\"due\":\"2026-10-01\",", fields.join(",")),
    );
    std::fs::write(s.path().join(".moai/issues.jsonl"), &doctored).unwrap();

    // `stands` 는 **그 표면이 언제나 세우는** 키다 — 걷은 뒤 제 값으로 다시 서므로 키가 있는
    // 것이 옳고, 거짓을 싣지 않았는지는 되쓴 값(`가짜`)이 사라졌는지로 잰다(moai-p8qj 의 `work`).
    let seen = |args: &[&str], stands: &[&str]| {
        let json = ok(s.path(), args);
        assert!(json.contains(&id), "{args:?} 가 그 줄을 안 냈다\n{json}");
        assert!(!json.contains("가짜"), "{args:?} 가 되쓴 줄의 옛 값을 냈다\n{json}");
        for (k, _) in &stale {
            if stands.contains(k) {
                continue;
            }
            assert!(!json.contains(&format!("\"{k}\"")), "{args:?} 가 되쓴 줄의 {k} 를 냈다\n{json}");
        }
        assert!(json.contains(r#""due":"2026-10-01""#), "{args:?} 가 겹치지 않는 모르는 필드까지 걷었다\n{json}");
    };
    seen(&["ready", "--json"], &[]);
    seen(&["show", "--json"], &["work"]);
    seen(&["show", "-s", "todo", "--json"], &["work"]);
    assert_eq!(issues(s.path()), doctored, "출력에서 걷으려다 파일을 바꿨다");
    // 쓰는 명령도 같다 — 파일에는 모르는 필드가 그대로 남는다.
    seen(&["edit", &id, "-p", "1", "--json"], &[]);
    seen(&["mv", &id, "in_progress", "--json"], &[]);
    assert!(line_of(s.path(), &id).contains(r#""place":"lost""#), "모르는 필드를 잃었다");
}

/// **끊긴 에픽을 든 생각 밑에 접힌 줄은 트리와 status 가 같게 읽는다**(moai-uni2) — 길 잃은
/// 부모의 묶음이다. 트리는 그 줄을 `(길 잃음)` 안의 생각 밑에 그리므로, status 가 그 줄을
/// "에픽 없는 이슈" 로 세면 `moai show -e none` 을 가리키며 고칠 수 없는 줄을 고치라 한다.
/// 고칠 곳은 생각의 끊긴 에픽 하나고, `dangling_epic` 이 그것을 댄다.
#[test]
fn a_child_under_a_lost_thought_is_not_counted_as_having_no_epic() {
    let s = init("lostthought");
    // 마일스톤을 쓰는 저장소여야 `no_milestone` 도 같은 자로 읽는지 본다.
    let stone = add(s.path(), &["v1", "--type", "milestone"]);
    let epic = add(s.path(), &["지울 에픽", "--type", "epic", "--milestone", &stone]);
    let thought = add(s.path(), &["생각", "--type", "idea", "-e", &epic]);
    let child = add(s.path(), &["생각 밑의 일", "--parent", &thought]);
    assert!(moai(s.path(), &["rm", &epic]).status.success());

    let tree = ok(s.path(), &["show", "--tree"]);
    let lost_at = tree.find("(길 잃음)").unwrap_or_else(|| panic!("길 잃음 바구니가 없다\n{tree}"));
    assert!(tree[lost_at..].contains(&child), "트리가 자식을 길 잃음 밖에 그렸다\n{tree}");

    let st = ok(s.path(), &["status", "--json"]);
    let warning = |kind: &str| st.split("{\"kind\":").find(|w| w.starts_with(&format!("\"{kind}\""))).map(str::to_string);
    assert!(warning("dangling_epic").is_some_and(|w| w.contains(&thought)), "고칠 곳(생각의 끊긴 에픽)을 안 댄다\n{st}");
    assert!(
        !warning("no_epic").is_some_and(|w| w.contains(&child)),
        "트리가 길 잃음에 그린 줄을 status 는 에픽 없는 이슈로 센다\n{st}"
    );
    assert!(
        !warning("no_milestone").is_some_and(|w| w.contains(&child)),
        "트리가 길 잃음에 그린 줄을 status 는 마일스톤 없는 일로 센다\n{st}"
    );
}

/// **`-e none`·`--milestone none` 도 status 와 같은 자로 고른다**(moai-phw9) — 끊긴 에픽을 든
/// 생각 밑에 접힌 줄은 트리가 `(길 잃음)` 안에 그리고 status 가 두 경고 어디에도 안 센다.
/// 거름망만 그 줄을 "없는 것" 으로 고르면 한 저장소가 같은 줄을 세 가지로 말한다. 진짜 소속
/// 없는 일은 그대로 고른다.
#[test]
fn none_filters_skip_a_child_under_a_lost_thought_like_status() {
    let s = init("nonelost");
    let mile = add(s.path(), &["마일스톤", "--type", "milestone"]);
    let epic = add(s.path(), &["지울 에픽", "--type", "epic", "--milestone", &mile]);
    let thought = add(s.path(), &["생각", "--type", "idea", "-e", &epic]);
    let child = add(s.path(), &["생각 밑의 일", "--parent", &thought]);
    let loose = add(s.path(), &["그냥 소속 없는 일"]);
    assert!(moai(s.path(), &["rm", &epic]).status.success());

    for flag in ["-e", "--milestone"] {
        let listed = ok(s.path(), &["show", flag, "none"]);
        let ids: Vec<&str> = listed.lines().filter_map(|l| l.split_whitespace().next()).collect();
        assert!(!ids.contains(&child.as_str()), "`{flag} none` 이 길 잃은 생각 밑에 접힌 줄을 고른다\n{listed}");
        assert!(ids.contains(&loose.as_str()), "`{flag} none` 이 진짜 소속 없는 일까지 뺐다\n{listed}");
    }
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

/// **한 번에 적는 글은 64KB 까지다**(moai-m9a8). 리뷰 원문 자리에 대화록 JSONL 이 두 번
/// 들어가 노트 한 줄이 36만 자가 됐다 — 저널은 덧붙이기만 해 영영 남는다. 넘으면 잘라 적지
/// 않고 거절하며, 거절문이 무엇을 넣어야 했는지 댄다. 노트·`-m`·제목·본문이 한 자리를 지난다 —
/// 제목은 `create` 가 저널에 옮겨 적으므로 한 줄짜리 대화록이 그 문으로 들어온다.
/// **재는 것은 지금 쓰는 글뿐이다** — 이미 큰 본문·제목을 든 줄도 옮기고 지울 수 있다.
#[test]
fn a_text_over_the_limit_is_refused_whole_and_says_what_to_write_instead() {
    let s = init("big-note");
    let id = add(s.path(), &["제목", "-b", "작다"]);
    let limit = 64 * 1024;
    let big = "가".repeat(limit / 3 + 1);
    assert!(big.len() > limit);
    let (before, notes) = (issues(s.path()), journal(s.path()));

    let refused = from_stdin(s.path(), &["note", &id, "-b", "-"], &big);
    assert!(!refused.status.success());
    let err = String::from_utf8_lossy(&refused.stderr);
    assert!(err.contains("64KB") && err.contains("대화록"), "무엇을 넣어야 했는지 안 댄다\n{err}");
    for (args, what) in [
        (vec!["mv", id.as_str(), "in_progress", "-m", big.as_str()], "mv -m"),
        (vec!["defer", id.as_str(), "-m", big.as_str()], "defer -m"),
        (vec!["edit", id.as_str(), "-b", big.as_str()], "edit -b"),
        (vec!["add", "새것", "-b", big.as_str()], "add -b"),
        (vec!["add", big.as_str()], "add 제목"),
        (vec!["edit", id.as_str(), "--title", big.as_str()], "edit --title"),
    ] {
        let out = moai(s.path(), &args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success() && err.contains("64KB"), "{what} 가 상한을 넘는 글을 받았다\n{err}");
    }
    // 계획은 stdin 으로 오므로 argv 의 길이 상한도 없다 — 이 상한이 유일한 문이다.
    let plan = from_stdin(s.path(), &["add", "--from", "-"], &format!("# 에픽\n- {big}\n"));
    assert!(!plan.status.success() && String::from_utf8_lossy(&plan.stderr).contains("64KB"), "add --from 이 큰 제목을 받았다");
    assert_eq!(issues(s.path()), before, "거절한 쓰기가 스냅샷을 바꿨다");
    assert_eq!(journal(s.path()), notes, "거절한 쓰기가 저널에 남았다");

    // 딱 상한은 받는다.
    ok(s.path(), &["note", &id, &"a".repeat(limit)]);

    // 손으로 넣은 큰 본문은 막지 않는다 — 그 줄을 옮기고 제목을 고칠 수 있어야 한다.
    let path = s.path().join(".moai/issues.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""body":"작다""#), "{text}");
    std::fs::write(&path, text.replace(r#""body":"작다""#, &format!(r#""body":"{big}""#))).unwrap();
    ok(s.path(), &["mv", &id, "in_progress"]);
    ok(s.path(), &["edit", &id, "--title", "새 제목"]);

    // 손으로 넣은 큰 제목도 — 옮기고 **지울 수** 있어야 한다. 저널의 `title` 을 따로 재지 않는 까닭이다:
    // `rm` 은 그 줄의 제목을 저널에 옮겨 적는데, 거기서 재면 이 줄은 도구 안에서 영영 못 치운다.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""title":"새 제목""#), "{text}");
    std::fs::write(&path, text.replace(r#""title":"새 제목""#, &format!(r#""title":"{big}""#))).unwrap();
    ok(s.path(), &["mv", &id, "review"]);
    ok(s.path(), &["rm", &id]);
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
    staged(args).current_dir(dir).env("MOAI_NOW", now).output().unwrap()
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
    let mut child = staged(args)
        .current_dir(dir)
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

/// **템플릿 파일은 `--var` 로 채워 편다**(moai-cypw). 연습도 채운 뒤의 계획을 보여 준다.
#[test]
fn a_template_file_is_filled_with_vars() {
    let s = init("template");
    let tpl = s.path().join("release.md");
    std::fs::write(&tpl, "# 릴리스 {{version}}\n- [p1] {{version}} 태그를 단다 #release\n- {{channel}} 에 올린다\n").unwrap();
    let path = tpl.to_str().unwrap();

    let rehearsal = ok(s.path(), &["add", "--from", path, "--var", "version=1.2", "--var", "channel=stable", "--dry-run"]);
    assert!(rehearsal.contains("릴리스 1.2") && rehearsal.contains("stable 에 올린다"), "{rehearsal}");
    assert_eq!(issues(s.path()), "", "연습인데 썼다");

    ok(s.path(), &["add", "--from", path, "--var", "version=1.2", "--var", "channel=stable"]);
    let made = issues(s.path());
    assert!(made.contains("\"title\":\"릴리스 1.2\"") && made.contains("\"title\":\"1.2 태그를 단다\""), "{made}");
    assert!(!made.contains("{{"), "채우지 않은 자리가 남았다\n{made}");
}

/// **못 채운 변수·계획에 없는 변수·같은 이름 두 번·모양이 틀린 `--var`·`--from` 없는 `--var` 는
/// 거절하고 아무것도 안 만든다**(사람이 정했다). 반만 채운 계획이 조용히 서지 않게.
#[test]
fn template_vars_that_do_not_fit_are_refused_and_write_nothing() {
    let s = init("templatebad");
    let tpl = s.path().join("t.md");
    std::fs::write(&tpl, "# {{version}}\n- {{channel}} 에 올린다\n").unwrap();
    let path = tpl.to_str().unwrap();
    for (args, says) in [
        (vec!["--var", "version=1"], "channel"),
        (vec!["--var", "version=1", "--var", "channel=c", "--var", "verison=2"], "verison"),
        (vec!["--var", "version=1", "--var", "version=2", "--var", "channel=c"], "version"),
        (vec!["--var", "version", "--var", "channel=c"], "이름=값"),
        (vec!["--var", "=1", "--var", "version=1", "--var", "channel=c"], "=1"),
        (vec!["--var", "version=1\n- [p0] 몰래", "--var", "channel=c"], "줄바꿈"),
        // 빈 값은 반쯤 채운 제목을 조용히 세운다 — 셸 변수가 비었을 때 흔하다(사람이 정했다).
        (vec!["--var", "version=", "--var", "channel=c"], "비었다"),
        (vec!["--var", "version=  ", "--var", "channel=c"], "비었다"),
        // 이름은 계획이 변수로 읽는 모양이다 — `{{버전}}` 은 글자라 "계획에 없는 변수" 로 둘러대지 않는다.
        (vec!["--var", "version=1", "--var", "channel=c", "--var", "버전=1"], "영문"),
    ] {
        let mut argv = vec!["add", "--from", path];
        argv.extend(args.iter().copied());
        let out = moai(s.path(), &argv);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?} 를 받았다");
        assert!(err.contains(says), "{args:?} 의 거절이 {says} 를 안 댄다 — {err}");
        assert_eq!(issues(s.path()), "", "{args:?} 인데 썼다");
    }
    let out = moai(s.path(), &["add", "제목", "--var", "a=1"]);
    assert!(!out.status.success(), "--from 없는 --var 를 받았다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("--from"), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(issues(s.path()), "", "--from 없는 --var 인데 썼다");

    // 값은 늘 제목 글자다 — 태그 자리의 변수는 값이 태그를 정하게 되므로 거절하고 아무것도 안 만든다.
    std::fs::write(&tpl, "# 릴리스\n- 올린다 #{{kind}}\n").unwrap();
    let out = moai(s.path(), &["add", "--from", path, "--var", "kind=bug"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("#{{kind}}"), "태그 자리의 변수를 받았다 — {err}");
    assert_eq!(issues(s.path()), "", "태그 자리의 변수인데 썼다");
}

/// `idea promote --from` 도 같은 길로 템플릿을 채운다 — 두 명령이 형식을 따로 알지 않는다.
#[test]
fn promote_fills_a_template_too() {
    let s = init("templatepromote");
    let idea = ok(s.path(), &["idea", "add", "릴리스를 돌린다", "-q"]).trim().to_string();
    let tpl = s.path().join("t.md");
    std::fs::write(&tpl, "# 릴리스 {{version}}\n- 올린다\n").unwrap();
    ok(s.path(), &["idea", "promote", &idea, "--from", tpl.to_str().unwrap(), "--var", "version=2.0"]);
    assert!(issues(s.path()).contains("\"title\":\"릴리스 2.0\""), "{}", issues(s.path()));
    // 채우지 않은 템플릿은 펼치지 않는다 — 아직 열린 생각에 대고 불러, 아무것도 안 쓰고 닫지도 않는지 본다.
    let open = ok(s.path(), &["idea", "add", "다음 릴리스", "-q"]).trim().to_string();
    let before = issues(s.path());
    let out = moai(s.path(), &["idea", "promote", &open, "--from", tpl.to_str().unwrap()]);
    assert!(!out.status.success(), "채우지 않은 템플릿을 펼쳤다");
    assert_eq!(issues(s.path()), before, "거절한 펼치기가 썼다");
}

#[test]
fn init_writes_an_agents_block() {
    let s = init("agents");
    let md = std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap();
    assert!(md.contains("<!-- moai:begin v:") && md.contains("<!-- moai:end -->"));
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

/// **`init --check` 은 AGENTS.md 블록을 `current`·`stale`·`missing` 으로 답하고 아무것도 안
/// 쓴다**(moai-mstm). 낡음으로는 0 이 아닌 적이 없다 — 낡음은 알릴 것이지 실패가 아니다
/// (2026-09-14 사용자 결정). 비영은 파일을 못 읽을 때뿐이다.
/// 옛 맨 마커도, 손으로 고친 블록도 `stale` 이다. 다시 심은 뒤에는 `current` 이고, 한 번 더
/// 심어도 바이트가 같다.
#[test]
fn init_check_says_current_stale_or_missing_and_writes_nothing() {
    let s = Scratch::new("initcheck");
    let md = s.path().join("AGENTS.md");
    let check = |want: &str| {
        let before = std::fs::read_to_string(&md).ok();
        let said = ok(s.path(), &["init", "--check"]);
        assert!(said.contains(want), "{want}: {said}");
        let js = ok(s.path(), &["init", "--check", "--json"]);
        one_json_value(&js);
        assert_eq!(field(&js, "agents"), want, "{js}");
        assert_eq!(std::fs::read_to_string(&md).ok(), before, "{want}: --check 가 AGENTS.md 를 썼다");
    };

    check("missing");
    assert!(!s.path().join(".moai").exists(), "--check 가 .moai 를 심었다");

    std::fs::write(&md, "# 산문\n\n<!-- moai:begin -->\n옛 내용\n<!-- moai:end -->\n").unwrap();
    check("stale");

    ok(s.path(), &["init", "argos"]);
    check("current");
    let once = std::fs::read_to_string(&md).unwrap();
    assert!(once.starts_with("# 산문\n\n<!-- moai:begin v:"), "{once}");
    ok(s.path(), &["init"]);
    assert_eq!(std::fs::read_to_string(&md).unwrap(), once, "다시 심었더니 바뀌었다");

    // 블록 안을 손으로 고치면 낡은 것이다 — 다음 `init` 이 덮어쓴다.
    std::fs::write(&md, once.replace("승인 게이트가 없다", "승인 게이트가 있다")).unwrap();
    check("stale");
}

/// **못 읽는 AGENTS.md 는 덮어쓰지 않는다**(리뷰 moai-epb0.27f). `init` 이 그것을 빈 글로 읽던
/// 때는 UTF-8 이 아닌 파일(CP949 로 저장한 한국어 산문 따위)이 블록 하나로 통째로 바뀌었고,
/// 같은 파일에 `--check` 는 못 읽는다고 답했다. 둘이 한 길로 읽고, `init` 은 아무것도 심기 전에 멈춘다.
#[test]
fn init_never_overwrites_an_agents_md_it_cannot_read() {
    let s = Scratch::new("agentsunreadable");
    let md = s.path().join("AGENTS.md");
    let mine = b"# \xb1\xd4\xbe\xe0 (CP949)\n\nhuman text\n";
    std::fs::write(&md, mine).unwrap();

    let out = moai(s.path(), &["init", "argos"]);
    assert!(!out.status.success(), "못 읽는 파일로 init 이 성공했다 — {}", text(&out));
    assert!(text(&out).contains("--no-agents"), "비켜 갈 길을 안 댄다 — {}", text(&out));
    assert_eq!(std::fs::read(&md).unwrap(), mine, "못 읽는 AGENTS.md 를 덮어썼다");
    assert!(!s.path().join(".moai").exists(), "멈추기 전에 .moai 를 심었다");
    assert!(!moai(s.path(), &["init", "--check"]).status.success());

    ok(s.path(), &["init", "argos", "--no-agents"]);
    assert_eq!(std::fs::read(&md).unwrap(), mine);
}

/// **못 쓰는 AGENTS.md 도 건너뛰고 나머지를 심는다**(moai-780n, 2026-09-15 사용자 결정). 읽기
/// 전용 파일 하나로 `?` 에 끊기던 때는 `.moai/` 와 `.gitattributes` 는 이미 선 채 접두어도 딸린
/// 파일 안내도 못 찍고 exit 1 이었다 — 같은 실행이 만든 안내가 통째로 삼켜졌다. **못 읽는 것과
/// 다르다**: 그쪽은 아무것도 심기 전에 멈춰 남의 산문을 지키지만, 여기는 못 쓰는 것뿐이라 잃을
/// 산문이 없다. 쓸 수 있게 고치고 다시 부르면 그때 블록이 선다.
#[cfg(unix)]
#[test]
fn init_finishes_even_when_agents_md_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("agentsro");
    let md = s.path().join("AGENTS.md");
    let mine = "# 우리 규약\n\n손으로 쓴 것.\n";
    std::fs::write(&md, mine).unwrap();
    std::fs::set_permissions(&md, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().append(true).open(&md).is_ok() {
        return; // root 는 권한을 안 본다
    }

    let out = moai(s.path(), &["init", "argos"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(std::fs::read_to_string(&md).unwrap(), mine, "못 쓰는 AGENTS.md 가 바뀌었다");
    assert!(said.contains("AGENTS.md") && said.contains("못 썼다"), "{said}");
    // **같은 실행이 만든 나머지 말이 삼켜지지 않는다** — 접두어도, 딸린 파일도 그대로 선다.
    assert!(said.contains("argos"), "접두어를 안 찍었다 — {said}");
    assert!(s.path().join(".moai/issues.jsonl").exists() && s.path().join(".gitattributes").exists(), "{said}");
    let js = ok(s.path(), &["init", "--json"]);
    assert!(js.contains("\"AGENTS.md\":{\"kind\":\"unwritable\"") && js.contains("\"agents\":false"), "{js}");

    // 쓸 수 있게 고치면 그때 블록이 선다 — 산문은 그대로 위에 남는다.
    std::fs::set_permissions(&md, std::fs::Permissions::from_mode(0o644)).unwrap();
    ok(s.path(), &["init"]);
    let now = std::fs::read_to_string(&md).unwrap();
    assert!(now.starts_with(mine) && now.contains("moai:begin"), "{now}");
}

/// **빠진 딸린 파일 규칙을 `status` 알림과 `init --check` 가 비춘다**(moai-2f99, 2026-09-15 사용자
/// 결정). `init` 이 한 번 말하고 마는 자리라, 못 써서 건너뛴 저장소는 `/.claude/worktrees/` 없이
/// 얼마든지 오래 간다 — 그러면 `git add -A` 가 옆 워크트리를 통째로 담는다(moai-mxtb 가 그 줄을
/// 넣은 까닭). **경고가 아니라 알림이다**: 종료 코드도 "드러난 문제 없다" 도 그대로다.
#[test]
fn a_missing_dotfile_rule_shows_in_status_and_check() {
    let s = init("dotgap");
    let ignore = s.path().join(".gitignore");
    // moai 가 넣은 줄 중 하나만 지운다 — 사람이 손으로 지웠거나, 못 써서 건너뛴 자리다.
    let kept: String = std::fs::read_to_string(&ignore).unwrap().lines().filter(|l| !l.contains("worktrees")).map(|l| format!("{l}\n")).collect();
    std::fs::write(&ignore, &kept).unwrap();

    let out = moai(s.path(), &["status"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", text(&out));
    assert!(said.contains("+ .gitignore") && said.contains("worktrees"), "{said}");
    // **고칠 명령까지 낸다** — 다른 알림과 같은 자리에 같은 모양으로. 없으면 무엇이 잘못됐다는
    // 말만 남고 어떻게 고치는지는 아무 데도 없다.
    assert!(said.contains("→ `moai init`"), "고칠 명령을 안 냈다 — {said}");
    assert!(said.contains("드러난 문제 없다"), "알림이 문제로 섰다 — {said}");
    let js = ok(s.path(), &["status", "--json"]);
    let notices = &js[js.find("\"notices\"").expect("notices 가 없다")..];
    assert!(notices.contains("\"kind\":\"gitignore_rules\""), "알림 자리에 없다 — {js}");

    let check = ok(s.path(), &["init", "--check"]);
    assert!(check.contains(".gitignore") && check.contains("worktrees"), "{check}");
    let cjs = ok(s.path(), &["init", "--check", "--json"]);
    assert!(cjs.contains("\"missing\":{\".gitignore\":[\"/.claude/worktrees/\"]}"), "{cjs}");

    // **없는 파일은 통째로 빠진 것이다** — `git add -A` 가 옆 워크트리를 담는 위험이 가장 큰
    // 자리라 입을 다물면 안 된다. 못 읽는 파일과 갈린다: 그쪽은 무엇이 들었는지 모른다.
    std::fs::remove_file(&ignore).unwrap();
    let gone = ok(s.path(), &["status"]);
    assert!(gone.contains(".gitignore") && gone.contains("worktrees"), "없는 .gitignore 를 안 비췄다 — {gone}");
    // 못 읽는 파일은 말하지 않는다 — `init` 이 안 건드리니 여기서 말하면 그 알림이 영영 안 걷힌다.
    std::fs::write(&ignore, [0xb0, 0xa1, b'\n']).unwrap();
    let bad = ok(s.path(), &["status"]);
    assert!(!bad.contains(".gitignore"), "못 읽는 파일을 빠졌다고 했다 — {bad}");
    assert!(!ok(s.path(), &["init", "--check", "--json"]).contains("\"missing\":"), "못 읽는 파일을 빠졌다고 했다");

    // 다시 심으면 사라진다.
    std::fs::write(&ignore, &kept).unwrap();
    ok(s.path(), &["init"]);
    let after = ok(s.path(), &["status"]);
    assert!(!after.contains(".gitignore"), "{after}");
    // `"missing"` 만 찾으면 `{"agents":"missing"}` 이 걸린다 — 키로 찾는다.
    assert!(!ok(s.path(), &["init", "--check", "--json"]).contains("\"missing\":"), "채웠는데 아직 빠졌다고 한다");
}

/// **`status` 는 낡은 AGENTS.md 블록을 알림(`notices`)으로 비춘다**(moai-mj45). 경고가 아니다 —
/// 종료 코드도 "드러난 문제 없다" 도 그대로다. **없는 블록은 말하지 않는다**: `--no-agents` 로
/// 안 쓰기로 한 저장소를 영영 조른다(2026-09-14 사용자 결정). 다시 심으면 사라진다.
///
/// 훅이 싣는 보드도 같은 알림을 든다 — 훅의 보드는 화면과 같은 말을 해야 한다. 하위 디렉터리에서
/// 부르면 고칠 명령이 `-C <뿌리>` 를 댄다: 맨 `moai init` 은 부른 자리에 둘째 트래커를 심는다.
#[test]
fn status_notices_a_stale_agents_block_but_not_a_missing_one() {
    let s = init("agentsnotice");
    let md = s.path().join("AGENTS.md");
    let quiet = |why: &str| {
        let st = ok(s.path(), &["status"]);
        assert!(!st.contains("AGENTS.md"), "{why}: {st}");
        assert!(!ok(s.path(), &["status", "--json"]).contains("agents_"), "{why}");
    };
    quiet("갓 심은 블록");

    let fresh = std::fs::read_to_string(&md).unwrap();
    std::fs::write(&md, fresh.replace("승인 게이트가 없다", "승인 게이트가 있다")).unwrap();
    let st = ok(s.path(), &["status"]);
    assert!(st.contains("+ AGENTS.md 블록을 손으로 고쳤다") && st.contains("`moai init`"), "{st}");
    let board = carried_text(&hook_out(&s, "user-prompt-submit", &event(&s, "agents")));
    assert!(board.contains("AGENTS.md 블록을 손으로 고쳤다"), "훅의 보드에 알림이 없다 — {board}");

    let sub = s.path().join("src").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    let root = std::fs::canonicalize(s.path()).unwrap();
    let below = ok(&sub, &["status"]);
    assert!(below.contains(&format!("`moai -C {} init`", root.display())), "하위에서 뿌리를 안 댄다 — {below}");
    assert!(!sub.join(".moai").exists());
    // `-C` 로 본 셸도 뿌리에 있지 않다 — 등록한 프로젝트를 한눈에 보다 `moai -C <프로젝트> status` 로 들어온 자리.
    let outside = Scratch::new("agentsnotice-outside");
    let there = ok(outside.path(), &["-C", &s.path().display().to_string(), "status"]);
    assert!(there.contains(&format!("`moai -C {} init`", root.display())), "-C 로 본 셸에 뿌리를 안 댄다 — {there}");
    assert!(st.contains("드러난 문제 없다"), "알림이 문제로 섰다 — {st}");
    let js = ok(s.path(), &["status", "--json"]);
    let notices = &js[js.find("\"notices\"").expect("notices 가 없다")..];
    assert!(notices.contains("\"kind\":\"agents_hand_edited\""), "알림 자리에 없다 — {js}");
    assert!(!js[..js.find("\"notices\"").unwrap()].contains("agents_"), "경고로 셌다 — {js}");

    ok(s.path(), &["init"]);
    quiet("다시 심은 뒤");

    std::fs::remove_file(&md).unwrap();
    quiet("블록이 없는 저장소");
}

/// **못 쓰는 `.gitignore` 에 걸려도 저장소를 반쯤 남기지 않는다**(moai-0dwc). 읽기 실패는
/// 넘어가게 고쳤는데 쓰기 실패는 `?` 로 끊어, `.moai/` 는 이미 심긴 채 `.gitattributes` 도
/// AGENTS 블록도 없는 저장소가 남았다. 못 읽는 자리와 같은 자다(2026-09-15 사용자 결정):
/// 건너뛰고 나머지를 심고, 손으로 더할 줄을 대고 0 으로 끝난다. 읽히고 쓰이게 고친 뒤 다시
/// 부르면 그때 채운다 — 미리 재고 심기 전에 멈추면 읽기 전용 파일 하나가 이미 심긴 저장소의
/// AGENTS 블록 갱신까지 막는다.
#[cfg(unix)]
#[test]
fn init_finishes_even_when_a_dotfile_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("rodotfile");
    let ignore = s.path().join(".gitignore");
    // 한 줄은 이미 들어 있다 — 못 쓴 자리가 **빠진 줄만** 대는지 여기서 본다.
    let theirs = "build/\n.moai/lock\n";
    std::fs::write(&ignore, theirs).unwrap();
    std::fs::set_permissions(&ignore, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().append(true).open(&ignore).is_ok() {
        return; // root 는 권한을 안 본다
    }

    let out = moai(s.path(), &["init", "argos"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(std::fs::read_to_string(&ignore).unwrap(), theirs, "못 쓰는 파일이 바뀌었다");
    // **낱말로 가른다** — 못 읽은 자리와 같은 말을 하면 사람이 권한 대신 인코딩을 고치러 간다.
    assert!(said.contains(".gitignore 에 못 썼다"), "{said}");
    assert!(!said.contains("못 읽어"), "쓰기 실패를 읽기 실패라 한다 — {said}");
    // **빠진 줄만 댄다**(리뷰 moai-humk) — 읽기는 됐으니 무엇이 이미 있는지 안다. 통째로 대면
    // 따라 붙여 넣은 사람의 파일에 같은 줄이 둘 선다.
    assert!(said.lines().any(|l| l.trim() == "/.claude/worktrees/"), "빠진 줄을 안 댔다 — {said}");
    assert!(!said.lines().any(|l| l.trim() == ".moai/lock"), "이미 있는 줄을 더하라고 한다 — {said}");
    // 반쯤 심긴 저장소를 남기지 않는다 — 나머지는 다 선다.
    assert!(s.path().join(".moai/issues.jsonl").exists(), ".moai 를 안 심었다");
    assert!(std::fs::read_to_string(s.path().join(".gitattributes")).unwrap().contains("merge=union"));
    assert!(std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap().contains("moai:begin"));
    // 못 쓴 자리도 이름→(갈래, 까닭)으로 선다(moai-ejgp). **갈래가 실려야** 기계가 가른다 —
    // `Permission denied` 는 못 읽을 때도 못 쓸 때도 똑같이 떠서 까닭 글만으로는 안 갈린다.
    let js = ok(s.path(), &["init", "--json"]);
    one_json_value(&js);
    assert!(js.contains("\"untouched\":{\".gitignore\":{\"kind\":\"unwritable\","), "{js}");
    assert!(js.contains("Permission denied"), "{js}");

    // 쓸 수 있게 고치고 다시 부르면 그때 채운다 — 남의 줄은 그대로다.
    std::fs::set_permissions(&ignore, std::fs::Permissions::from_mode(0o644)).unwrap();
    ok(s.path(), &["init"]);
    let now = std::fs::read_to_string(&ignore).unwrap();
    assert!(now.starts_with(theirs) && now.contains("/.claude/worktrees/"), "{now}");
    assert_eq!(now.matches(".moai/lock").count(), 1, "이미 있던 줄을 또 넣었다 — {now}");
}

/// **못 읽는 `.gitignore`·`.gitattributes` 는 안 건드린다**(moai-gq1c). 빈 글로 치고 쓰던 때는
/// CP949 로 적은 남의 파일이 moai 줄만 남기고 통째로 사라졌다 — 되돌릴 길은 git 뿐이다.
/// **멈추지는 않는다**(2026-09-15 사용자 결정): 나머지는 다 심고, 그 자리에 무엇을 손으로
/// 더할지 대며 0 으로 끝난다. AGENTS.md 가 멈추는 자리인 까닭은 그쪽이 도구가 쓴 블록을
/// 통째로 갈아 끼우는 자리라서다.
#[test]
fn init_never_overwrites_a_dotfile_it_cannot_read() {
    let s = Scratch::new("badignore");
    // CP949 로 적힌 남의 줄 — UTF-8 로는 못 읽는다.
    let theirs: &[u8] = b"\xc7\xd1\xb1\xdb\nbuild/\n";
    std::fs::write(s.path().join(".gitignore"), theirs).unwrap();

    let out = moai(s.path(), &["init", "argos"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(std::fs::read(s.path().join(".gitignore")).unwrap(), theirs, "못 읽는 파일을 덮었다");
    assert!(said.contains(".gitignore") && said.contains("못 읽어"), "{said}");
    // **붙여 넣을 수 있어야 한다** — 한 줄에 하나씩이라야 규칙이 선다. 쉼표로 잇던 때는
    // `.gitattributes` 의 두 줄이 한 줄이 되어 `journal.jsonl` 이 `merge=union` 을 못 받았다.
    assert!(
        said.lines().any(|l| l.trim() == ".moai/lock"),
        "손으로 더할 줄을 한 줄에 하나씩 안 댔다 — {said}"
    );
    // 나머지는 다 심는다.
    assert!(s.path().join(".moai/issues.jsonl").exists(), "나머지를 안 심었다");
    assert!(std::fs::read_to_string(s.path().join(".gitattributes")).unwrap().contains(".moai/issues.jsonl"));
    assert!(std::fs::read_to_string(s.path().join("AGENTS.md")).unwrap().contains("moai:begin"));

    let js = ok(s.path(), &["init", "--json"]);
    one_json_value(&js);
    assert!(js.contains("\"gitignore\":false"), "{js}");
    // **이름만이 아니라 갈래와 까닭까지**(moai-ejgp) — 기계가 '다시 인코딩하라' 와 'chmod 하라'
    // 를 가른다. 까닭 글만으로는 안 갈린다: 권한으로 못 읽는 파일도 `Permission denied` 다.
    assert!(js.contains("\"untouched\":{\".gitignore\":{\"kind\":\"unreadable\","), "갈래를 안 실었다 — {js}");
    assert!(js.contains("UTF-8"), "까닭을 안 실었다 — {js}");

    // `.gitattributes` 도 같은 자리다 — 둘 다 못 읽으면 둘 다 든다.
    std::fs::write(s.path().join(".gitattributes"), theirs).unwrap();
    let both = ok(s.path(), &["init", "--json"]);
    assert!(both.contains("\"untouched\":{\".gitattributes\":{") && both.contains("},\".gitignore\":{"), "{both}");
    assert_eq!(std::fs::read(s.path().join(".gitattributes")).unwrap(), theirs, "못 읽는 파일을 덮었다");

    // 읽히게 고치면 그때 넣는다 — 남의 줄은 그대로 두고 뒤에 붙는다.
    std::fs::write(s.path().join(".gitattributes"), "").unwrap();
    std::fs::write(s.path().join(".gitignore"), "build/\n").unwrap();
    ok(s.path(), &["init"]);
    let now = std::fs::read_to_string(s.path().join(".gitignore")).unwrap();
    assert!(now.starts_with("build/\n") && now.contains(".moai/lock"), "{now}");
    // 고친 뒤에는 기계에게도 남은 것이 없다고 말한다.
    let clean = ok(s.path(), &["init", "--json"]);
    assert!(!clean.contains("untouched"), "다 읽히는데 못 읽었다고 한다 — {clean}");
}

/// **`init` 은 제가 쓴 것만 말한다**(moai-knn0). 블록이 이미 맞으면 "맞췄다" 도 `"agents":true`
/// 도 거짓말이다 — 낡았다는 알림을 보고 부른 사람이 그 줄만 보고는 무엇이 바뀌었는지 모른다.
/// `gitattributes`·`gitignore` 가 이미 쓴 때만 말하는 자리라 셋을 한 자로 맞춘다.
#[test]
fn init_says_only_what_it_wrote() {
    let s = init("saidwrote");
    // 갓 심은 자리를 다시 부르면 블록이 이미 맞는다 — 그러니 안 썼다고 말한다.
    let first = ok(s.path(), &["init", "--json"]);
    assert!(first.contains("\"agents\":false"), "안 썼는데 썼다고 한다 — {first}");

    let md = s.path().join("AGENTS.md");
    let fresh = std::fs::read_to_string(&md).unwrap();
    let edited = fresh.replace("승인 게이트가 없다", "승인 게이트가 있다");
    // 안내 글이 바뀌어 이 낱말이 사라지면 아래 단언이 엉뚱한 것을 탓한다 — 여기서 먼저 잡는다.
    assert_ne!(edited, fresh, "시험이 블록을 못 고쳤다 — 안내 글에서 찾는 낱말이 사라졌다");
    std::fs::write(&md, edited).unwrap();
    let wrote = ok(s.path(), &["init", "--json"]);
    assert!(wrote.contains("\"agents\":true"), "고친 블록을 다시 썼는데 안 썼다고 한다 — {wrote}");

    let said = ok(s.path(), &["init"]);
    assert!(!said.contains("블록을 맞췄다"), "안 쓰고 맞췄다고 한다 — {said}");
    assert!(said.contains("이미 다 맞아 있다"), "그대로인데 아무 말도 안 한다 — {said}");
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
    assert_eq!(md.matches("<!-- moai:begin").count(), 1);
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
    // 읽음도 사용자 설정에 적는다. 시험의 `MOAI_CONFIG` 는 그 시험만의 임시 자리다.
    "read",
];

/// **읽음은 내 설정에만 적힌다**(moai-u8oh, 사용자 결정) — 트래커 파일은 한 바이트도 안 바뀐다.
/// `--all` 은 내게 온 것 가운데 안 읽은 것을, `-e` 는 그 묶음(에픽·마일스톤)의 멤버와 **그 밑까지**
/// 적는다 — 물려받은 소속도 센다(moai-u8oh.x85). 없는 줄은 #a-partial 대로 말하고 비영으로 끝나고,
/// id 를 준 길은 누군지 몰라도 적는다.
#[test]
fn read_marks_the_line_in_my_config_and_leaves_the_tracker_alone() {
    let s = init("readmark");
    let cfg = s.path().join("user.toml");
    let mine = |args: &[&str]| ok_with(s.path(), &cfg, args);
    let epic = field(&mine(&["add", "에픽", "--type", "epic", "--json"]), "id");
    let member = field(&mine(&["add", "멤버", "-e", &epic, "--json"]), "id");
    // 멤버의 자식 — **제 `epic` 을 안 적고 부모에게서 받는다**(`moai add --parent`). `-e` 가
    // 줄의 `epic` 필드만 보던 때 이 줄이 통째로 빠졌다(moai-u8oh.x85).
    let child = field(&mine(&["add", "자식", "--parent", &member, "--json"]), "id");
    let before = issues(s.path());

    let out = mine(&["read", &member, "--json"]);
    assert!(out.contains(&member), "{out}");
    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(saved.contains("[read]") && saved.contains(&member), "{saved}");
    assert_eq!(issues(s.path()), before, "읽었다고 트래커가 바뀌었다");

    // 두 번째는 적을 것이 없다 — 그 뒤로 줄이 안 바뀌어 같은 도장이 이미 적혀 있다(moai-lyc1).
    assert!(mine(&["read", &member]).contains("읽음으로 적을 것이 없다"));

    // `-e` 는 에픽 자신과 멤버와 **그 밑까지** 함께 적는다. 자식이 빠지면 "그 밑까지" 가 거짓이다.
    let out = mine(&["read", "-e", &epic, "--json"]);
    assert!(out.contains(&epic) && out.contains(&child), "에픽 자신이나 멤버의 자식이 빠졌다 — {out}");

    // 마일스톤도 묶음이다 — 묶음 줄 하나만 적고 멤버를 흘리지 않는다.
    let stone = field(&mine(&["milestone", "add", "v1", "--json"]), "id");
    let dated = field(&mine(&["add", "마일 이슈", "--milestone", &stone, "--json"]), "id");
    let out = mine(&["read", "-e", &stone, "--json"]);
    assert!(out.contains(&dated), "마일스톤의 멤버가 빠졌다 — {out}");

    // #a-partial — 없는 줄은 말하고 나머지는 적되, **비영으로 끝난다**. 0 으로 끝나면 고리를
    // 짜는 쪽이 오타 친 id 를 적힌 것으로 세고 넘어간다.
    let another = field(&mine(&["add", "또 하나", "--json"]), "id");
    let out = moai_with(s.path(), &cfg, &["read", "없는-줄", &another, "--json"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains(&another), "하나가 없다고 나머지를 안 적었다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("없는-줄"));
    assert!(!out.status.success(), "못 찾은 것이 있는데 0 으로 끝났다");

    // 없는 묶음도 같은 자리다 — 조용히 빈 손으로 끝나면 오타를 친 줄 모른다.
    let out = moai_with(s.path(), &cfg, &["read", "-e", "없는-에픽", "--json"]);
    assert!(!out.status.success(), "없는 묶음인데 0 으로 끝났다");
    assert!(String::from_utf8_lossy(&out.stdout).contains(r#""missing":["없는-에픽"]"#), "{out:?}");

    // **누군지 몰라도 id 를 준 길은 적는다** — 저널에 안 쓰니 이름 없는 줄이 남을 자리가 없다.
    // 담당을 재는 `--all` 만 사람을 묻는다.
    let bare = isolated(BIN)
        .args(["read", &member])
        .current_dir(s.path())
        .env("MOAI_CONFIG", &cfg)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .expect("moai 를 실행하지 못했다");
    assert!(bare.status.success(), "{}", String::from_utf8_lossy(&bare.stderr));

    // `--all` 은 내게 온 것 가운데 **안 읽은 것만** — 위에서 다 읽었으니 새로 만든 줄 하나뿐이다.
    let late = field(&mine(&["add", "늦게 온 것", "--json"]), "id");
    let out = mine(&["read", "--all", "--json"]);
    assert!(out.contains(&late), "안 읽은 줄이 안 들었다 — {out}");
    assert!(!out.contains(&member), "이미 읽은 줄을 다시 적었다 — {out}");
}

/// **읽음은 본 때가 아니라 본 줄의 `updated_at` 을 적는다**(moai-lyc1, 사용자 결정 2026-09-19). 본 때를
/// 적던 때는 워크트리 가지에서 01:30 에 고치고 03:00 에 머지한 줄을 02:00 에 읽었으면 [NEW] 가 영영 안
/// 섰다 — 이 저장소의 평소 흐름이다. 견주는 식은 `>` 그대로라, 옛 바이너리가 적은 본 때는 옛 뜻대로
/// 읽혀 업그레이드가 [NEW] 를 한꺼번에 세우지 않는다.
#[test]
fn reading_writes_the_stamp_of_the_line_i_saw() {
    let s = init("readstamp");
    let cfg = s.path().join("user.toml");
    let at = |when: &str, args: &[&str]| {
        let out = staged(args).current_dir(s.path()).env("MOAI_CONFIG", &cfg).env("MOAI_NOW", when).output().unwrap();
        assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let id = field(&at("2026-09-18T00:00:00Z", &["add", "워크트리에서 고칠 줄", "--json"]), "id");

    // 02:00 에 읽는다 — 적히는 것은 그 줄의 도장(00:00)이다.
    at("2026-09-18T02:00:00Z", &["read", &id]);
    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(saved.contains(&format!("\"{id}\" = \"2026-09-18T00:00:00Z\"")) || saved.contains(&format!("{id} = \"2026-09-18T00:00:00Z\"")), "{saved}");

    // 가지에서 01:30 에 찍힌 고침이 머지로 들어온다 — 읽은 때(02:00)보다 이르지만 본 줄과 다르다.
    at("2026-09-18T01:30:00Z", &["edit", &id, "--tag", "merged"]);
    let out = at("2026-09-18T03:00:00Z", &["read", "--all", "--json"]);
    assert!(out.contains(&id), "머지로 들어온 고침에 [NEW] 가 안 섰다 — {out}");

    // 옛 값(본 때, 도장보다 늦다)은 옛 뜻대로 읽는다 — 그 뒤로 안 바뀐 줄은 안 읽음이 아니다.
    let old = field(&at("2026-09-18T04:00:00Z", &["add", "옛 바이너리로 읽은 줄", "--json"]), "id");
    let text = std::fs::read_to_string(&cfg).unwrap();
    std::fs::write(&cfg, format!("{text}\"{old}\" = \"2026-09-18T05:00:00Z\"\n")).unwrap();
    let out = at("2026-09-18T06:00:00Z", &["read", "--all", "--json"]);
    assert!(!out.contains(&old), "옛 값으로 읽은 줄이 다시 [NEW] 로 섰다 — {out}");
}

/// **같은 id 의 줄이 둘이면 늦은 도장을 적는다**(moai-7c50.exy). 안 읽음은 쌍둥이 가운데 하나만 바뀌어도
/// 그 id 를 세우는데, 뒷줄 하나의 도장만 적던 때는 앞줄이 늘 더 늦어 `read --all` 이 그 id 를 세우고도
/// "적을 것이 없다" 만 되뇌었다 — [NEW] 가 영영 안 내린다. 본 때를 적던 때는 그 때가 둘 다를 덮었다.
#[test]
fn reading_a_duplicate_id_takes_the_later_stamp_of_the_twins() {
    let s = init("readtwins");
    let cfg = s.path().join("user.toml");
    // 담당은 시험의 사람(`ACTOR`)이다 — `--all` 은 내게 온 것만 센다.
    let line = |at: &str| {
        format!(
            "{{\"id\":\"argos-0001\",\"title\":\"쌍둥이\",\"status\":\"todo\",\"assignee\":\"테스터\",\"assignee_email\":\"tester@example.com\",\"created_at\":\"2026-09-18T00:00:00Z\",\"updated_at\":\"{at}\",\"status_since\":\"2026-09-18T00:00:00Z\"}}\n"
        )
    };
    // 앞줄이 늦다 — 뒷줄(`Load::get`·탐색기가 여는 줄)만 적으면 앞줄이 남는다.
    std::fs::write(s.path().join(".moai/issues.jsonl"), format!("{}{}", line("2026-09-18T05:00:00Z"), line("2026-09-18T01:00:00Z"))).unwrap();

    let out = ok_with(s.path(), &cfg, &["read", "--all", "--json"]);
    assert!(out.contains("argos-0001"), "안 읽은 쌍둥이를 안 적었다 — {out}");
    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(saved.contains("argos-0001 = \"2026-09-18T05:00:00Z\""), "늦은 도장을 안 적었다 — {saved}");
    let out = ok_with(s.path(), &cfg, &["read", "--all", "--json"]);
    assert!(!out.contains("argos-0001"), "읽은 쌍둥이가 여전히 안 읽음이다 — {out}");

    // id 를 준 길도 같다 — 뒷줄이 늦으면 뒷줄 것이다.
    std::fs::write(s.path().join(".moai/issues.jsonl"), format!("{}{}", line("2026-09-18T06:00:00Z"), line("2026-09-18T07:00:00Z"))).unwrap();
    ok_with(s.path(), &cfg, &["read", "argos-0001"]);
    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(saved.contains("argos-0001 = \"2026-09-18T07:00:00Z\""), "늦은 도장을 안 적었다 — {saved}");
}

/// **`-e` 는 그 묶음 밑에 그려진 것만 적는다**(moai-j038.vna) — 트리·`show -e`·탐색기의 `SPC m r` 과 같은
/// 자(`nav::Index::under_group`)다. id 조상을 따로 훑던 때는 `moai epic add --parent <바깥>` 으로 선 안쪽
/// 에픽과 **그 멤버 절반**(id 로 선 것만)을, `--parent <바깥> -e <남>` 으로 남의 에픽에 든 자식까지 적었다 —
/// 어느 표면도 그 줄들을 바깥 에픽 밑에 그리지 않는다. 묶음이 아닌 줄은 적기 전에 거절한다.
#[test]
fn reading_a_group_takes_what_is_drawn_under_it() {
    let s = init("readgroup");
    let cfg = s.path().join("user.toml");
    let mine = |args: &[&str]| ok_with(s.path(), &cfg, args);
    let outer = field(&mine(&["epic", "add", "바깥", "--json"]), "id");
    let other = field(&mine(&["epic", "add", "남의 에픽", "--json"]), "id");
    let member = field(&mine(&["add", "바깥 멤버", "-e", &outer, "--json"]), "id");
    let inner = field(&mine(&["epic", "add", "안쪽", "--parent", &outer, "--json"]), "id");
    let by_field = field(&mine(&["add", "안쪽 멤버", "-e", &inner, "--json"]), "id");
    let by_id = field(&mine(&["add", "안쪽 자식", "--parent", &inner, "--json"]), "id");
    let elsewhere = field(&mine(&["add", "남의 자식", "--parent", &outer, "-e", &other, "--json"]), "id");
    // id 가 서로의 앞머리라(`바깥.xxx`) 따옴표째 찾는다.
    let has = |out: &str, id: &str| out.contains(&format!("\"{id}\""));

    let out = mine(&["read", "-e", &outer, "--json"]);
    assert!(has(&out, &outer) && has(&out, &member), "묶음 줄이나 멤버가 빠졌다 — {out}");
    for stray in [&inner, &by_field, &by_id, &elsewhere] {
        assert!(!has(&out, stray), "바깥 에픽 밑에 안 그려진 {stray} 를 적었다 — {out}");
    }

    // 묶음이 아닌 줄은 적기 전에 거절한다 — 받으면 그 줄 하나만 적고 0 으로 끝나 "그 밑까지" 가 거짓이 된다.
    let out = moai_with(s.path(), &cfg, &["read", "-e", &by_field]);
    assert!(!out.status.success(), "묶음이 아닌 줄을 받았다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("에픽·마일스톤"), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!std::fs::read_to_string(&cfg).unwrap().contains(&format!("{by_field} =")), "거절해 놓고 적었다");
}

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
    // `read` 는 **제 설정 파일로** 여기서 따로 부른다 — 공용 집의 없는 파일에 쓰면 그 격리가 깨진다
    // (`project add`·`rm` 과 같은 까닭). 무엇을 적는지는 위의 `read_marks_…` 시험이 본다.
    let own = s.path().join("sweep-read.toml");
    one_json_value(&ok_with(s.path(), &own, &["read", &id, "--json"]));
    for cmd in JSON_SWEEP.iter().filter(|c| !["init", "add", "read"].contains(c)) {
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

/// **본문 속 명령은 `moai show` 를 지나도 한 줄로 남는다**(moai-syp8·moai-xtu8).
///
/// 본문은 폭 76에 맞춰 접히는데, 인라인 코드 안의 공백까지 접는 자리로 보면 화면에 찍힌
/// 명령이 두 줄로 갈린다. 그 줄을 복사해 돌린 셸은 잘린 명령을 본다 — 화면이 아니라
/// **복사한 뒤**가 무너지는 자리라, 여기서 보는 것은 그려진 출력의 한 줄이다.
///
/// 폭보다 긴 코드도 끊지 않는다(사용자 결정, moai-krh7). 줄이 오른쪽으로 삐져나가는 것은
/// 터미널이 화면에서만 접고, 복사하면 온전하다.
#[test]
fn a_command_in_a_body_survives_being_drawn() {
    let s = init("mdcode");
    // 76칸 안에 드는 명령과, 혼자서 76칸을 넘는 명령을 함께 싣는다.
    let short = "moai note moai-4aex -b - <<'NOTE'";
    let long = "moai add \"아주 긴 제목을 가진 이슈를 한 번에 만든다\" -t bug -e moai-4aex --milestone v0.1 -b -";
    let body = format!("보기 하나는 `{short}` 이고, 폭을 넘기는 것은 `{long}` 이다. 둘 다 복사해 돈다.");
    let id = add(s.path(), &["제목", "-b", &body]);

    let drawn = ok(s.path(), &["show", &id]);
    for cmd in [short, long] {
        assert!(
            drawn.lines().any(|l| l.contains(cmd)),
            "그려진 본문에서 명령이 갈렸다 — {cmd}\n{drawn}"
        );
    }
}

/// **되뽑은 계획은 도로 들어간다.** `show <에픽> --as-plan` 의 출력을 그대로
/// `add --from` 에 넣으면 같은 모양의 에픽이 선다 — 이 짝이 틀로 쓰는 계약이다.
#[test]
fn an_epic_comes_back_out_as_a_plan_that_goes_back_in() {
    let s = init("asplan");
    let made = from_stdin(
        s.path(),
        &["add", "--from", "-"],
        "# 릴리스 #release\n- 바이너리를 올린다\n- [p1] 태그를 단다 #git\n",
    );
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let epic = field(&ok(s.path(), &["show", "epic", "--json"]), "id");

    let plan = ok(s.path(), &["show", &epic, "--as-plan"]);
    assert_eq!(plan, "# 릴리스 #release\n- [p1] 태그를 단다 #git\n- 바이너리를 올린다\n");

    let again = init("asplanagain");
    let back = from_stdin(again.path(), &["add", "--from", "-"], &plan);
    assert!(back.status.success(), "되뽑은 계획이 도로 안 들어간다\n{}", String::from_utf8_lossy(&back.stderr));
    let epic2 = field(&ok(again.path(), &["show", "epic", "--json"]), "id");
    assert_eq!(ok(again.path(), &["show", &epic2, "--as-plan"]), plan);

    let json = ok(s.path(), &["show", &epic, "--as-plan", "--json"]);
    one_json_value(&json);
    assert!(json.contains(r##""plan":"# 릴리스 #release\n"##), "{json}");
    assert!(json.contains(r#""lossy":[]"#), "{json}");

    // 앞머리 `[` 와 끝의 `#낱말` 은 이스케이프로 도로 들어간다(moai-a5pz).
    let bracket = add(s.path(), &["[WIP] 반쯤 #12", "-e", &epic]);
    let plan = ok(s.path(), &["show", &epic, "--as-plan"]);
    assert!(plan.contains("- \\[WIP] 반쯤 \\#12\n"), "{plan}");
    assert!(ok(s.path(), &["show", &epic, "--as-plan", "--json"]).contains(r#""lossy":[]"#), "이스케이프한 제목을 짚었다");
    let round = init("asplanescape");
    assert!(from_stdin(round.path(), &["add", "--from", "-"], &plan).status.success(), "{plan}");
    assert!(ok(round.path(), &["show", "-g", "WIP"]).contains("[WIP] 반쯤 #12"), "제목이 도로 안 섰다");
    ok(s.path(), &["rm", &bracket]);

    // 이스케이프로도 못 담는 제목은 조용히 틀리지 않고 이름을 댄다. 실패로는 안 끝난다.
    let wip = add(s.path(), &["\\[이미 역슬래시]", "-e", &epic]);
    let out = moai(s.path(), &["show", &epic, "--as-plan"]);
    assert!(out.status.success(), "경고로 실패했다\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains(&wip), "{}", String::from_utf8_lossy(&out.stderr));
    let json = ok(s.path(), &["show", &epic, "--as-plan", "--json"]);
    assert!(json.contains(&format!(r#""lossy":["{wip}"]"#)), "{json}");

    // 에픽이 아닌 것과 목록 자리는 거절한다 — 조용히 엉뚱한 계획을 내지 않는다.
    let lone = add(s.path(), &["그냥 이슈"]);
    for args in [vec!["show", lone.as_str(), "--as-plan"], vec!["show", "--as-plan"], vec!["show", &epic, "--as-plan", "--raw"]] {
        let out = moai(s.path(), &args);
        assert!(!out.status.success(), "{args:?} 를 말없이 먹었다");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("--as-plan"), "무엇이 문제인지 말하지 않았다 — {args:?}\n{err}");
    }
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
    // 화면의 말을 바꾸는 길도 선다 — AGENTS.md 에만 있으면 처음 만난 쪽이 못 찾는다(moai-kbky)
    assert!(out.contains("MOAI_LANG=en") && out.contains("[i18n]"), "{out}");
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
    let mut closer: Option<String> = None;
    for l in help.lines().skip_while(|l| !l.starts_with("Usage:")).skip(1) {
        // **heredoc 블록은 왼쪽 끝이 옳다** — 복사해 돌려면 여는 줄도 닫는 줄도 들여쓰지
        // 않는다(moai-foc3). 블록 안은 산문도 설명도 아니라 건너뛰고, 닫힌 뒤 새로 센다.
        if let Some(tag) = &closer {
            if l == tag {
                closer = None;
                prose = None;
            }
            continue;
        }
        if let Some(tag) = heredoc_tag(l) {
            closer = Some(tag);
            prose = None;
            continue;
        }
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

/// 줄에 heredoc 이 열리면 그 닫는 표시. `<<<` 는 heredoc 이 아니다.
fn heredoc_tag(line: &str) -> Option<String> {
    let at = line.match_indices("<<").map(|(i, _)| i).find(|&i| {
        !line[..i].ends_with('<') && !line[i + 2..].starts_with('<')
    })?;
    let rest = line[at + 2..].trim_start_matches('-').trim_start();
    let rest = rest.trim_start_matches(['\'', '"']);
    let tag: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!tag.is_empty()).then_some(tag)
}

/// 모든 명령(하위 명령까지)의 `--help`. **명령 목록은 바이너리의 도움말에서 읽는다.**
fn every_help(s: &Scratch) -> Vec<(String, String)> {
    let mut queue: Vec<Vec<String>> = vec![vec![]];
    let mut all = Vec::new();
    while let Some(path) = queue.pop() {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let help = ok(s.path(), &args);
        for c in subcommands(&help) {
            let mut next = path.clone();
            next.push(c);
            queue.push(next);
        }
        all.push((path.join(" "), help));
    }
    all
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

    let mut seen = Vec::new();
    let mut with_examples = 0;
    let mut bad = Vec::new();
    for (path, help) in every_help(&s) {
        if help.lines().any(|l| l.starts_with("  moai ")) {
            with_examples += 1;
        }
        for b in lost_indent(&help) {
            bad.push(format!("moai {path} --help:\n{b}"));
        }
        seen.push(path);
    }
    // 훑기가 실제로 돌았는지 — 하위 명령까지 내려갔고 예시 줄을 가진 도움말을 봤다.
    assert!(seen.len() > 20, "명령 목록을 못 읽었다 — {seen:?}");
    assert!(seen.iter().any(|c| c == "skill install"), "하위 명령으로 안 내려갔다 — {seen:?}");
    assert!(with_examples >= 8, "예시 줄이 있는 도움말이 {with_examples} 개뿐이다 — {seen:?}");
    assert!(bad.is_empty(), "들여쓰기를 잃은 줄:\n\n{}", bad.join("\n\n"));
}

/// **도움말의 heredoc 은 복사해서 그대로 돈다** (moai-foc3).
///
/// 들여쓴 heredoc 을 그대로 치면 닫는 표시가 들여써져 셸이 끝을 못 찾는다 — `<<-` 는
/// 탭만 벗긴다. 한 줄에 `<<'MD' ... MD` 로 줄인 것은 닫히지 않는다. 표시가 `EOF`·`MD`
/// 면 커밋 메시지나 `moai note -b - <<'MD'` 에 인용할 때 바깥 heredoc 을 일찍 닫는다.
#[test]
fn every_help_heredoc_is_copyable() {
    // 판정이 헛돌지 않는지 먼저 본다.
    for (broken, why) in [
        ("  moai add --from - <<'PLAN'\n  # 에픽\n  PLAN\n", "들여쓴 여는 줄"),
        ("moai add --from - <<'PLAN'\n# 에픽\n  PLAN\n", "들여쓴 닫는 줄"),
        ("moai note t-1 -b - <<'PLAN' ... PLAN\n", "한 줄로 줄인 것"),
        ("moai add --from - <<'EOF'\n# 에픽\nEOF\n", "겹치는 표시"),
    ] {
        assert!(!copyable_heredocs(broken).is_empty(), "판정이 {why} 을 못 알아본다");
    }
    assert!(copyable_heredocs("moai add --from - <<'PLAN'\n# 에픽\nPLAN\n  설명\n").is_empty());
    assert!(copyable_heredocs("grep x <<< \"hi\"\n").is_empty(), "here-string 을 heredoc 으로 읽는다");

    let s = init("helpheredoc");
    let mut bad = Vec::new();
    let mut seen = 0;
    for (path, help) in every_help(&s) {
        seen += help.lines().filter(|l| heredoc_tag(l).is_some()).count();
        for b in copyable_heredocs(&help) {
            bad.push(format!("moai {path} --help: {b}"));
        }
    }
    assert!(seen >= 4, "도움말에서 heredoc 을 {seen}개밖에 못 찾았다");
    assert!(bad.is_empty(), "복사해 못 도는 heredoc:\n{}", bad.join("\n"));
}

/// **좁은 터미널에서도 heredoc 여는 줄이 접히지 않는다** (moai-opjn).
///
/// clap 의 `wrap_help` 는 도움말을 터미널 폭에 맞춰 낱말 사이에 **실제 개행**을 넣어
/// 접는다. 여는 줄이 두 줄로 갈리면 복사한 명령은 heredoc 없이 돈다 — `-b -` 는 본문 대신
/// 터미널을 기다리고, 다음 줄로 밀린 `<<'NOTE'` 는 명령 없는 heredoc 이 되어 본문을
/// 삼킨다. 그래서 `wrap_help` 를 뺐다 — 뺀 채로는 clap 이 `COLUMNS` 를 안 읽어 이 시험은
/// 폭과 무관하게 초록이다. **지키는 것은 누가 그 기능을 다시 켜는 날이다.** 켜지면 시험에
/// 터미널이 없어 clap 이 `COLUMNS` 로 폭을 정하고, 좁힌 폭에서 도움말이 달라져 붉어진다.
///
/// **여는 줄만 보지 않고 도움말을 통째로 견준다.** 예시 명령도 같은 까닭으로 복사해 돌아야
/// 하고, 여는 줄만 보면 잡는 힘이 예시 줄이 우연히 몇 칸인지에 매인다 — clap 은 폭에 드는
/// 줄을 가르지 않아서, `30`·`45` 로 보던 때는 26칸짜리 `moai add --from - <<'PLAN'` 을 어느
/// 폭에서도 못 갈랐다(moai-opjn.l0i). 폭은 가장 짧은 여는 줄보다 좁게 둔다.
#[test]
fn narrow_terminals_keep_heredoc_openers_whole() {
    const NARROW: usize = 20;
    let s = init("helpnarrow");
    let mut openers = 0;
    let mut bad = Vec::new();
    for (path, full) in every_help(&s) {
        openers += full.lines().filter(|l| heredoc_tag(l).is_some()).count();
        let narrow = help_at(&s, &path, NARROW);
        if narrow != full {
            let split: Vec<&str> = full.lines().filter(|l| !narrow.lines().any(|n| n == *l)).collect();
            // 도움말마다 한 줄 — 여는 줄이 갈렸으면 그것을, 아니면 처음 갈린 줄을 댄다.
            let shown = split.iter().find(|l| heredoc_tag(l).is_some()).or(split.first());
            let shown = shown.copied().unwrap_or("(갈린 줄은 없고 무언가 더 붙었다)");
            bad.push(format!("COLUMNS={NARROW} moai {path} --help: {}줄 갈림 — {shown}", split.len()));
        }
    }
    assert!(openers >= 4, "heredoc 여는 줄을 {openers}개밖에 못 봤다");
    assert!(bad.is_empty(), "좁은 터미널에서 접힌 도움말:\n{}", bad.join("\n"));
}

/// `COLUMNS` 를 준 채 부른 `moai <path> --help`. 견줄 기준이 `every_help`(`ok`)라서 폭 말고는
/// `moai` 와 같은 환경으로 부른다 — 다른 것이 섞이면 달라진 까닭이 폭이 아닐 수 있다.
fn help_at(s: &Scratch, path: &str, columns: usize) -> String {
    let mut args: Vec<&str> = path.split_whitespace().collect();
    args.push("--help");
    let out = staged(&args)
        .current_dir(s.path())
        .env("COLUMNS", columns.to_string())
        .output()
        .expect("moai 를 실행하지 못했다");
    assert!(out.status.success(), "COLUMNS={columns} moai {args:?} 가 실패했다\n{}", text(&out));
    String::from_utf8(out.stdout).unwrap()
}

/// **모든 명령의 `--help` 와 `-h` 는 80칸 안이다**(moai-c57v). 도움말은 접지 않으므로(moai-opjn)
/// 넘는 줄은 좁은 터미널에서 그대로 꺾인다. `tui` 의 글만 보던 시험(moai-lz2t)을 넓혔다 —
/// after_help 글줄, clap 이 옵션 열 옆에 붙이는 짧은 help, `--help` 가 옵션 밑에 펴는
/// long_help 문단까지 **그려진 모양 그대로** 잰다. 옵션 열의 폭은 그 명령에서 가장 긴 옵션이
/// 정하므로, 한 명령에 옵션을 더하는 것만으로 남의 줄이 넘칠 수 있다 — 그래서 글이 아니라
/// 그린 것을 잰다. `-h` 는 long_help 가 있는 명령에서 모양이 달라(설명이 옵션 열 옆 한 줄)
/// 따로 불러 잰다(moai-x18p).
///
/// **모호폭(`·`·`—`·`→`)은 두 칸으로 센다**(사용자 결정, moai-ygki). 한국어 설정의 터미널은
/// 흔히 그것을 두 칸으로 그려, 한 칸으로 재어 80칸인 줄이 84칸이 되어 꺾였다. 넓게 그리는
/// 쪽에 맞추면 두 터미널 모두에서 선다. 화면 코드의 `text::width` 는 그대로 한 칸이다 —
/// 그쪽은 칸을 채워 그리는 셈이라 넓게 세면 한 칸 터미널에서 줄 끝이 빈다.
#[test]
fn every_help_fits_in_eighty_columns() {
    let s = init("helpwidth");
    let mut seen = 0;
    let mut wide = Vec::new();
    for (path, help) in every_help(&s) {
        seen += 1;
        let mut args: Vec<&str> = path.split_whitespace().collect();
        args.push("-h");
        let short = ok(s.path(), &args);
        for (flag, text) in [("--help", help.as_str()), ("-h", short.as_str())] {
            for l in text.lines().filter(|l| cjk_cells(l) > 80) {
                wide.push(format!("moai {path} {flag}  {}: {l}", cjk_cells(l)));
            }
        }
    }
    assert!(seen > 20, "명령 목록을 못 읽었다 — {seen}개");
    assert!(wide.is_empty(), "80칸을 넘는 도움말 줄:\n{}", wide.join("\n"));
}

/// **`-h` 의 설명 열은 한 칸에 선다**(moai-o46r). clap 은 `unicode` 기능 없이는 value_name 을
/// 글자 수로 세어, `<이름 (메일)>`·`<어떻게>` 가 든 옵션만 설명이 두어 칸 왼쪽에 섰다. 폭 시험은
/// 넘친 줄만 잡으니 이 어긋남은 못 본다 — 그래서 열이 서는 칸을 따로 잰다. 칸은 clap 과 같은
/// `cells` 로 센다(모호폭 한 칸) — 넓게 세면 `·` 가 든 옵션 이름이 어긋난 것으로 읽힌다.
#[test]
fn every_short_help_aligns_its_descriptions() {
    let s = init("helpalign");
    let mut seen = 0;
    let mut bad = Vec::new();
    for (path, _) in every_help(&s) {
        let mut args: Vec<&str> = path.split_whitespace().collect();
        args.push("-h");
        let short = ok(s.path(), &args);
        // clap 은 절(`Arguments:`·`Options:`)마다 따로 맞춘다 — 들이지 않은 줄에서 끊는다.
        let mut sections: Vec<Vec<(usize, &str)>> = vec![vec![]];
        for l in short.lines() {
            if !l.is_empty() && !l.starts_with(' ') {
                sections.push(vec![]);
            } else if let Some(c) = description_column(l) {
                sections.last_mut().unwrap().push((c, l));
            }
        }
        for cols in sections {
            seen += cols.len();
            if cols.windows(2).any(|w| w[0].0 != w[1].0) {
                let rows: Vec<String> = cols.iter().map(|(c, l)| format!("  {c:>3}: {l}")).collect();
                bad.push(format!("moai {path} -h\n{}", rows.join("\n")));
            }
        }
    }
    assert!(seen > 100, "설명 열을 못 읽었다 — {seen}줄");
    assert!(bad.is_empty(), "설명 열이 줄마다 다른 칸에 선 도움말:\n{}", bad.join("\n"));
}

/// 옵션·인자 줄에서 **설명이 서는 칸.** 두 칸 넘게 들여 `-`·`<`·`[` 로 시작하고, 이름 뒤 두 칸
/// 넘는 틈 다음이 설명이다. 설명이 다음 줄로 내려간 옵션과 옵션이 아닌 줄은 `None`.
///
/// **들여쓰기를 먼저 걷는다** — 짧은 이름 없는 옵션(`      --user <이름 (메일)>`)은 clap 이 여섯
/// 칸을 들이므로, 두 칸만 걷고 `-` 를 보면 그 줄이 통째로 빠진다. 이 시험이 잡으려던 바로 그
/// 줄이 거기 있다.
fn description_column(line: &str) -> Option<usize> {
    let body = line.strip_prefix("  ")?.trim_start();
    if !body.starts_with(['-', '<', '[']) {
        return None;
    }
    let gap = body.find("  ")?;
    let desc = body[gap..].trim_start();
    (!desc.is_empty()).then(|| cells(&line[..line.len() - desc.len()]))
}

/// 모호폭을 두 칸으로 센 폭 — 도움말 폭 시험만 쓴다(moai-ygki).
fn cjk_cells(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width_cjk(s)
}

/// 복사해 못 도는 heredoc 마다 한 줄.
fn copyable_heredocs(help: &str) -> Vec<String> {
    let lines: Vec<&str> = help.lines().collect();
    let mut bad = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(tag) = heredoc_tag(line) else { continue };
        if line.starts_with(char::is_whitespace) {
            bad.push(format!("여는 줄이 들여써졌다 — {line}"));
        }
        if matches!(tag.as_str(), "EOF" | "MD") {
            bad.push(format!("표시 {tag} 는 바깥 heredoc 과 겹친다 — {line}"));
        }
        match lines[i + 1..].iter().find(|l| l.trim() == tag) {
            Some(close) if *close == tag => {}
            Some(close) => bad.push(format!("닫는 줄이 들여써졌다 — {close:?}")),
            None => bad.push(format!("닫히지 않는다 — {line}")),
        }
    }
    bad
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

/// **훅이 내보낸 저장소 변수가 `-C` 를 이기지 못한다**(moai-ztdf). git 훅과 딸린 워크트리의
/// `rebase -x` 는 `GIT_DIR` 무리를 내보내고, 그것은 `git -C <경로>` 를 이긴다. 그대로 두면
/// 저장소 R 의 훅에서 부른 `moai -C P` 가 R 의 `user.name` 을 읽어 **P 의 저널에 남의 이름을
/// 영구히** 적는다. 이력이 목적인 파일이라 되돌리기 어렵다 — 커밋 칸도 남의 이력을 읽는다.
///
/// 쓰는 사람과 읽는 화면 **둘 다** 본다: 담당·저널의 이름과 메일, 그리고 `show` 의 커밋 칸.
///
/// **커밋 칸은 없는 것과 있는 것을 같이 본다.** 없는 것만 보면 커밋 칸이 통째로 비어도 초록이라,
/// 이 시험이 "P 의 이력을 읽는다" 와 "아무 이력도 안 읽는다" 를 못 가른다.
///
/// **시각은 고정한다**(`git_at`). 걷기가 `created_at`(= `MOAI_NOW`) 에서 끊기므로, 커밋을 기계 시계로
/// 찍으면 시계가 그보다 이른 기계에서 양쪽 다 안 보여 이 시험이 조용히 초록이 된다.
///
/// **심는 것은 `REPO` 무리뿐이다** — 그 무리에는 `GIT_CONFIG_PARAMETERS`·`GIT_CONFIG_COUNT` 도 든다
/// (바깥 `git -c` 가 내보낸다. 2026-09-15 사용자 결정으로 `TEST` 에서 옮겨 왔고, 릴리스도 걷는다).
/// **주 워크트리의 커밋 훅은 `GIT_DIR` 을 안 주므로 가장 흔한 훅 모양에서는 그것이 유일하게 새는
/// 길이다** — 그래서 아래 `planted` 는 그 이름에 `.git` 경로가 아니라 **R 의 사람을 실제로 담은**
/// 설정을 준다. 경로를 주면 git 이 `bogus format` 으로 죽어, 걷기가 무너진 날 이 시험이 "남의 사람을
/// 적었다" 대신 생 stderr 를 낸다.
///
/// **고치기 전에 빨갰던 것은 쓰는 쪽이다.** 읽는 쪽(커밋 칸)은 옛 `git::output` 이 이미 `GIT_DIR` 무리
/// 셋을 릴리스에서도 걷고 있어 그때도 초록이었다 — 여기 남긴 까닭은 앞으로의 방벽이다.
#[test]
fn an_inherited_git_dir_does_not_beat_the_project_we_were_given() {
    let s = init("hookenv");
    let theirs = s.path().join("theirs");
    std::fs::create_dir_all(&theirs).unwrap();
    // R — 훅이 도는 남의 저장소. 이름도 커밋도 이쪽에만 있다.
    git(&theirs, &["init", "-q"]);
    git(&theirs, &["config", "user.name", "남의 이름"]);
    git(&theirs, &["config", "user.email", "theirs@example.com"]);
    // P — moai 프로젝트. 이 저장소의 사람이 적혀 있다.
    git(s.path(), &["init", "-q"]);
    git(s.path(), &["config", "user.name", "내 이름"]);
    git(s.path(), &["config", "user.email", "mine@example.com"]);

    // 훅이 실제로 내보내는 모양 그대로 — 저장소를 가리키는 변수만, 다만 **`REPO` 를 통째로** 심는다
    // (목록은 `#[path]` 로 이미 들어와 있다). 몇 개만 골라 심으면 안 심은 이름이 `TEST` 로 옮겨져도
    // 모든 시험이 초록이라, 이 변경이 실제로 가른 경계를 아무것도 붙잡지 못한다. **여기가 그 경계를
    // 붙잡는 유일한 자리다** — 단위 시험은 `cfg!(test)` 라 두 무리를 구별하지 못한다.
    //
    // **값은 git 이 그 이름에 기대하는 꼴로 준다.** 불리언 자리에 경로를 주면 git 이 `bad boolean
    // environment value` 로 죽어, 걷기가 무너진 날 시험이 내는 말이 "남의 사람을 적었다" 가 아니라
    // 생 stderr 가 된다 — 회귀는 잡히는데 무엇이 깨졌는지를 못 가리킨다.
    let planted: Vec<(&'static str, String)> = git_leaks::REPO
        .iter()
        .map(|&var| {
            let path = |rel: &str| theirs.join(rel).to_str().unwrap().to_string();
            let at = match var {
                "GIT_INDEX_FILE" => path(".git/index"),
                // 사람을 읽는 `git config` 를 곧장 R 의 설정으로 돌린다 — 가장 날카로운 탐침이다.
                "GIT_CONFIG" => path(".git/config"),
                "GIT_OBJECT_DIRECTORY" | "GIT_ALTERNATE_OBJECT_DIRECTORIES" => path(".git/objects"),
                "GIT_SHALLOW_FILE" => path(".git/shallow"),
                "GIT_GRAFT_FILE" => path(".git/info/grafts"),
                "GIT_WORK_TREE" | "GIT_PREFIX" | "GIT_CEILING_DIRECTORIES" => path(""),
                // 설정을 **값으로** 넣는 것. 바깥 `git -c user.name=… commit` 이 훅에 내보내는 꼴
                // 그대로다 — `-C <P>` 를 대도 이것은 이기므로, 걷기가 무너지면 P 의 저널에 R 의
                // 이름이 적힌다. 경로를 주면 git 이 형식 오류로 죽어 그 경계를 못 잰다.
                "GIT_CONFIG_PARAMETERS" => "'user.name=남의 이름' 'user.email=theirs@example.com'".into(),
                // 짝이 되는 `GIT_CONFIG_KEY_0`·`GIT_CONFIG_VALUE_0` 은 번호가 붙어 목록에 못 적는다 —
                // 이 세는 값 하나를 걷는 것이 그것들을 통째로 무르는 길이라, 여기서는 그 꼴만 맞춘다.
                "GIT_CONFIG_COUNT" => "0".into(),
                // 불리언으로 읽는 것들.
                "GIT_IMPLICIT_WORK_TREE" | "GIT_NO_REPLACE_OBJECTS" | "GIT_DISCOVERY_ACROSS_FILESYSTEM" => "1".into(),
                "GIT_NAMESPACE" => "theirs".into(),
                "GIT_REPLACE_REF_BASE" => "refs/replace/".into(),
                _ => path(".git"),
            };
            (var, at)
        })
        .collect();
    let in_hook = |args: &[&str]| {
        let mut cmd = isolated(BIN);
        cmd.args(args).current_dir(&theirs).env("MOAI_NOW", NOW).env("NO_COLOR", "1");
        for (var, at) in &planted {
            cmd.env(var, at);
        }
        cmd.output().unwrap()
    };
    let project = s.path().to_str().unwrap();
    let made = in_hook(&["-C", project, "add", "훅 안에서 만든 것", "--json"]);
    let said = String::from_utf8_lossy(&made.stdout).to_string();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    // **메일도 본다.** 파일에는 이름과 메일이 갈라져 있어, 이름만 보면 절반이 새도 초록이다.
    assert!(said.contains(r#""assignee":"내 이름""#), "훅 저장소의 사람을 담당으로 적었다\n{said}");
    assert!(said.contains(r#""assignee_email":"mine@example.com""#), "훅 저장소의 메일을 담당으로 적었다\n{said}");
    let j = journal(s.path());
    assert!(!j.contains("남의 이름"), "훅 저장소 주인의 이름이 이 프로젝트의 저널에 남았다\n{j}");
    assert!(!j.contains("theirs@example.com"), "훅 저장소 주인의 메일이 이 프로젝트의 저널에 남았다\n{j}");

    // 읽는 쪽도 같다 — 커밋 칸은 이 프로젝트의 이력에서 읽는다. R 의 커밋이 걸리면 안 된다.
    let id = field(&said, "id");
    git_at(s.path(), NOW, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 이 저장소의 커밋 ({id})")]);
    git_at(&theirs, NOW, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 남의 이력이 샜다 ({id})")]);
    let shown = in_hook(&["-C", project, "show", &id]);
    assert!(shown.status.success(), "{}", String::from_utf8_lossy(&shown.stderr));
    let shown = String::from_utf8_lossy(&shown.stdout);
    assert!(shown.contains("이 저장소의 커밋"), "이 프로젝트의 커밋이 커밋 칸에 안 섰다 — 빈 칸은 아무것도 못 잰다\n{shown}");
    assert!(!shown.contains("남의 이력이 샜다"), "훅 저장소의 커밋을 이 이슈에 붙였다\n{shown}");
}

/// **`--json` 은 "커밋 없음" 의 까닭을 가른다**(moai-rzsv, 2026-09-15 사용자 결정). 사람 화면은
/// 그대로 말없이 빈 칸이지만(moai-mauw), 기계는 셋을 갈라야 한다 — 진짜로 아무도 안 고친 이슈,
/// git 이 없는 기계, git 저장소가 아닌 자리. 못 가르면 에이전트가 끝난 일을 다시 하거나 손댄
/// 이슈를 안 손댄 것으로 보고한다.
///
/// `commits` 는 **늘 선다**(빈 배열도 사실이다). git 을 못 읽었을 때만 `commits_error` 가 붙는다.
#[test]
fn json_tells_no_commits_apart_from_no_git() {
    let s = init("jsonnocommits");
    let id = add(s.path(), &["고칠 것"]);

    // git 저장소가 아닌 자리 — 빈 배열에 까닭이 붙는다.
    let outside = ok(s.path(), &["show", &id, "--json"]);
    assert!(outside.contains(r#""commits":[]"#), "빈 배열을 안 냈다\n{outside}");
    // **까닭은 가를 수 있는 값이다**(moai-6p1n) — 산문을 부분 문자열로 맞추지 않는다.
    // 울타리 밑(임시 자리가 체크아웃 안인 기계)에는 "저장소가 아닌 자리" 가 없다 — `git.rs` 의
    // `a_repo_without_commits_is_empty_not_broken` 와 같은 자리다(moai-boc6).
    if !scratch::fenced_base() {
        assert!(outside.contains(r#""commits_error":{"kind":"not_a_repo","said":"#), "git 을 못 읽은 까닭이 없다\n{outside}");
    }
    let root = s.path().to_str().unwrap();
    assert!(!outside.contains(root), "기계의 절대 경로가 --json 으로 나갔다\n{outside}");

    // 갓 만든 저장소 — **커밋이 하나도 없는 것은 실패가 아니다.** `git log HEAD` 가 죽는 자리라
    // 그대로 두면 `commits_error` 가 "여기서는 못 물어봤다" 로 서서 받는 쪽이 정반대로 읽는다.
    git(s.path(), &["init", "-q"]);
    let unborn = ok(s.path(), &["show", &id, "--json"]);
    assert!(unborn.contains(r#""commits":[]"#), "빈 배열을 안 냈다\n{unborn}");
    assert!(!unborn.contains("commits_error"), "커밋 없는 저장소를 못 읽은 것으로 냈다\n{unborn}");

    // 저장소이고 이력도 있지만 이 이슈를 댄 커밋은 없다 — 빈 배열만, 까닭은 없다.
    git_at(s.path(), NOW, &["commit", "-q", "--allow-empty", "-m", "chore: 아무 id 도 안 대는 커밋"]);
    let none = ok(s.path(), &["show", &id, "--json"]);
    assert!(none.contains(r#""commits":[]"#), "빈 배열을 안 냈다\n{none}");
    assert!(!none.contains("commits_error"), "멀쩡히 읽었는데 까닭을 달았다\n{none}");

    // 커밋이 있으면 그대로 선다.
    git_at(s.path(), LATER, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 고친다 ({id})")]);
    let some = ok(s.path(), &["show", &id, "--json"]);
    assert!(some.contains(r#""subject":"feat: 고친다"#), "커밋을 안 냈다\n{some}");
    assert!(!some.contains("commits_error"), "멀쩡히 읽었는데 까닭을 달았다\n{some}");

    // 사람 화면은 그대로다 — 까닭을 화면에 늘어놓지 않는다(moai-mauw).
    let bare = init("jsonnocommits-bare");
    let b = add(bare.path(), &["git 밖"]);
    let shown = ok(bare.path(), &["show", &b]);
    assert!(!shown.contains("커밋") && !shown.contains("commits_error"), "사람 화면이 시끄러워졌다\n{shown}");
}

/// **일한 AI 는 노트의 `model:` 줄에서 읽어 `work` 로 낸다**(moai-8f2g, 2026-09-18 사용자 결정).
/// 저장하지 않는다 — 적는 것은 `moai note` 그대로다. 키는 **늘 선다**(빈 배열도 사실이다, moai-2l8n).
/// 토큰을 모르면 `null` 이고 0 은 0 이다. 꼴에 안 맞는 줄은 값이 안 될 뿐 이력에 그대로 남는다.
#[test]
fn show_json_reads_who_did_the_work_from_model_notes() {
    let s = init("worknotes");
    let id = add(s.path(), &["고칠 것"]);
    let none = ok(s.path(), &["show", &id, "--json"]);
    assert!(none.contains(r#""work":[]"#), "줄이 없는데 빈 배열을 안 냈다\n{none}");

    ok(s.path(), &["note", &id, "고쳤다\nmodel: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)"]);
    ok(s.path(), &["note", &id, "model: opus-5 (medium — 옛 줄)"]);
    ok(s.path(), &["note", &id, "model: opus-5 tokens 12 (오타)"]);
    ok(s.path(), &["mv", &id, "done", "-m", "model: openai/gpt-6 tokens=0"]);
    let some = ok(s.path(), &["show", &id, "--json"]);
    assert!(
        some.contains(r#"{"provider":"anthropic","model":"opus-5","tokens":182000,"grade":"high","why":"쓰기 경로","#),
        "꼴대로 적은 줄을 못 읽었다\n{some}"
    );
    assert!(
        some.contains(r#"{"provider":null,"model":"opus-5","tokens":null,"grade":"medium","why":"옛 줄","#),
        "옛 줄을 회사·토큰 없이 읽지 못했다\n{some}"
    );
    assert!(some.contains(r#""model":"gpt-6","tokens":0,"#), "0 을 모름으로 읽었다\n{some}");
    assert_eq!(some.matches(r#""provider":"#).count(), 3, "오타 줄을 값으로 읽었다\n{some}");
    // 값이 안 된 줄도 이력에서 빠지지 않는다.
    assert!(ok(s.path(), &["show", &id]).contains("tokens 12"), "꼴에 안 맞는 노트가 이력에서 사라졌다");
}

/// **목록도 `work` 를 낸다**(moai-p8qj). 닫힌 500건의 토큰을 더하려고 `show <id> --json` 을
/// 500번 부르면 저널 전체를 500번 읽는다 — 목록이 한 번 읽어 id 로 가른다.
///
/// 두 표면의 답은 **같아야 한다**. 키는 목록에서도 늘 선다(moai-2l8n).
#[test]
fn the_list_json_carries_the_same_work_as_one_expanded_issue() {
    let s = init("worklist");
    let mine = add(s.path(), &["내가 한 것"]);
    let bare = add(s.path(), &["아무도 안 적은 것"]);
    ok(s.path(), &["note", &mine, "model: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)"]);
    // 꼴을 옮겨 적은 예는 값이 아니다 — 목록도 하나를 펼칠 때와 같은 자로 읽는다.
    ok(s.path(), &["note", &bare, "이렇게 적는다\n\n    model: anthropic/opus-5 (low — 예)"]);

    let one = ok(s.path(), &["show", &mine, "--json"]);
    let list = ok(s.path(), &["show", "--json"]);
    let row = |list: &str, id: &str| {
        list.split("{\"id\":")
            .find(|r| r.starts_with(&format!("\"{id}\"")))
            .unwrap_or_else(|| panic!("{id} 줄이 목록에 없다\n{list}"))
            .to_string()
    };
    let did = r#""work":[{"provider":"anthropic","model":"opus-5","tokens":182000,"grade":"high","why":"쓰기 경로","#;
    assert!(one.contains(did), "하나를 펼친 쪽이 안 냈다\n{one}");
    assert!(row(&list, &mine).contains(did), "목록이 하나를 펼친 쪽과 다른 답을 냈다\n{list}");
    assert!(row(&list, &bare).contains(r#""work":[]"#), "적은 줄이 없는데 키가 안 섰거나 예를 값으로 읽었다\n{list}");
    // 걸러진 목록도 같다.
    let picked = ok(s.path(), &["show", "-s", "todo", "--json"]);
    assert!(picked.contains(did), "필터를 준 목록이 work 를 잃었다\n{picked}");

    // **차례도 같다**(리뷰 moai-u5bk.3wq) — 파일에 늦게 적혔어도 이른 시각의 줄이 앞이다. 두 표면이
    // 차례를 따로 세우던 때는 한쪽만 바꿔도 줄 하나짜리 이 시험이 못 잡았다.
    let early = "model: anthropic/sonnet-5 tokens=5 (low — 먼저 한 판)";
    assert!(at(s.path(), "2026-09-10T00:00:00Z", &["note", &mine, early]).status.success());
    let one = ok(s.path(), &["show", &mine, "--json"]);
    let list = ok(s.path(), &["show", "--json"]);
    let work = |json: &str| {
        let rest = &json[json.find("\"work\":").unwrap_or_else(|| panic!("work 가 없다\n{json}"))..];
        rest[..rest.find("}]").map_or(rest.len(), |e| e + 2)].to_string()
    };
    let (in_one, in_list) = (work(&one), work(&row(&list, &mine)));
    assert_eq!(in_list, in_one, "목록과 하나를 펼친 쪽의 work 가 갈렸다");
    assert!(in_one.find("sonnet-5") < in_one.find("opus-5"), "이른 시각의 줄이 앞에 안 섰다 — {in_one}");
}

/// **날짜가 거꾸로 선 커밋 밑도 본다**(moai-hws2). `show` 는 이슈가 생긴 때에서 걷기를 끊었는데,
/// `git log --since` 는 거르기가 아니라 **끊기**라 그보다 이른 커밋을 하나 만나면 그 아래를 통째로
/// 안 본다 — `rebase`·`am --committer-date-is-author-date`·하루 넘게 늦은 시계가 그런 커밋을 만든다.
/// 탐색기는 이력을 다 걸어 이미 보이던 것이라, 같은 물음에 두 표면이 다른 답을 냈다.
#[test]
fn show_sees_commits_under_a_backdated_one() {
    let s = init("backdated");
    let id = add(s.path(), &["고칠 것"]);
    git(s.path(), &["init", "-q"]);
    git_at(s.path(), NOW, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 고친다 ({id})")]);
    // rebase 가 옛 작성 시각을 커밋 시각으로 옮긴 커밋 — 이 이슈가 생기기 한참 전이다.
    git_at(s.path(), "2020-01-01T00:00:00Z", &["commit", "-q", "--allow-empty", "-m", "chore: 옛 날짜로 얹힌 커밋"]);

    let shown = ok(s.path(), &["show", &id]);
    assert!(shown.contains("feat: 고친다"), "날짜가 거꾸로 선 커밋 밑을 못 봤다\n{shown}");
    let json = ok(s.path(), &["show", &id, "--json"]);
    assert!(json.contains("feat: 고친다"), "--json 도 같은 답이어야 한다\n{json}");
    assert!(!json.contains("옛 날짜로 얹힌"), "id 를 안 적은 커밋이 붙었다\n{json}");
}

/// **사람은 부른 자리가 아니라 그 프로젝트에서 온다**(moai-d3sy). 환경을 다 걷어도(moai-ztdf) 어느
/// 저장소의 사람인지는 여전히 `git config` 를 **어디서** 부르는가가 정했다 — 프로젝트 P 안에 딴
/// 저장소가 겹쳐 있으면(`P/vendor`, 흔한 모양이다) 거기서 부른 `moai add` 가 P 의 저널에 그 저장소의
/// 이름을 영구히 적는다. 커밋 칸은 같은 명령에서 P 를 읽으므로, 한 명령이 두 저장소를 보고 있었다.
///
/// 환경 변수는 하나도 안 심는다 — 이 축은 그것 없이도 샌다.
#[test]
fn the_person_comes_from_the_project_not_the_directory_we_stand_in() {
    let s = init("nestedrepo");
    git(s.path(), &["init", "-q"]);
    git(s.path(), &["config", "user.name", "프로젝트 주인"]);
    git(s.path(), &["config", "user.email", "project@example.com"]);
    // P 안에 겹친 저장소 — 제 사람이 따로 적혀 있다.
    let vendor = s.path().join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();
    git(&vendor, &["init", "-q"]);
    git(&vendor, &["config", "user.name", "벤더 봇"]);
    git(&vendor, &["config", "user.email", "bot@vendor.example"]);

    // `-C` 도 환경도 없다. 그 밑에서 그냥 부른다 — `.moai` 는 위로 찾아 P 가 나온다.
    let made = isolated(BIN)
        .args(["add", "겹친 저장소 안에서 만든 것", "--json"])
        .current_dir(&vendor)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&made.stdout).to_string();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert!(said.contains(r#""assignee":"프로젝트 주인""#), "겹친 저장소의 사람을 담당으로 적었다\n{said}");
    assert!(said.contains(r#""assignee_email":"project@example.com""#), "겹친 저장소의 메일을 적었다\n{said}");
    let j = journal(s.path());
    assert!(!j.contains("벤더 봇") && !j.contains("bot@vendor.example"), "겹친 저장소의 사람이 저널에 남았다\n{j}");

    // 사람을 아무 데서도 못 찾으면 **그대로 멈춘다** — 이 고침이 그 규약을 건드리지 않는다.
    let nameless = Scratch::new("nestedrepo-nameless");
    ok(nameless.path(), &["init", "argos"]);
    let inner = nameless.path().join("vendor");
    std::fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "-q"]);
    git(&inner, &["config", "user.name", "벤더 봇"]);
    git(&inner, &["config", "user.email", "bot@vendor.example"]);
    let out = isolated(BIN)
        .args(["add", "이름 없는 프로젝트"])
        .current_dir(&inner)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "겹친 저장소의 사람으로 적고 지나갔다");
    assert!(String::from_utf8_lossy(&out.stderr).contains("git config user.name"), "{}", String::from_utf8_lossy(&out.stderr));
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

/// **선 에픽에 멤버로 펼친다**(`-e`, moai-f3ml). 에픽이 내건 것이 idea 로 밖에 나가
/// 있던 것을 되찾는 자리라, 새 에픽이 서면 그 에픽이 목적을 못 이룬 채 닫힌다.
/// 길이 `promote` 하나로 남아야 idea 가 저절로 닫히고 출처가 저널에 선다.
#[test]
fn promote_can_pour_into_a_standing_epic() {
    let s = init("ideainto");
    let epic = ok(s.path(), &["epic", "add", "선 에픽", "-q"]).trim().to_string();
    let id = ok(s.path(), &["idea", "add", "밖에 나간 것", "-q"]).trim().to_string();
    let before = issues(s.path()).lines().filter(|l| l.contains(r#""kind":"epic""#)).count();

    let out = from_stdin(s.path(), &["idea", "promote", &id, "-e", &epic, "--from", "-"], "- [p1] 되찾은 일\n");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let after = issues(s.path());
    assert_eq!(after.lines().filter(|l| l.contains(r#""kind":"epic""#)).count(), before, "새 에픽이 섰다");
    let member = after.lines().find(|l| l.contains("되찾은 일")).expect("멤버가 안 섰다");
    assert!(member.contains(&format!(r#""epic":"{epic}""#)), "에픽에 안 들었다 — {member}");
    assert!(line_of(s.path(), &id).contains(r#""status":"done""#), "idea 가 안 닫혔다");
    // 출처는 멤버에 적힌다 — 에픽은 이 idea 에서 나온 것이 아니다.
    let from_here = format!("{id} 에서 펼쳤다");
    let log = journal(s.path());
    let noted: Vec<&str> = log.lines().filter(|l| l.contains(&from_here)).collect();
    assert_eq!(noted.len(), 1, "{noted:?}");
    assert!(!noted[0].contains(&format!(r#""id":"{epic}""#)), "선 에픽에 출처를 적었다 — {}", noted[0]);
}

/// 선 에픽에 펼치는 계획에 `#` 줄은 설 자리가 없다. 받아 주면 에픽이 하나 더 서거나
/// 조용히 버려지는데, 어느 쪽이든 사람이 적은 것과 다르다. 없는 것·에픽 아닌 것도
/// 거절한다 — idea 가 닫히므로 틀린 자리에 펼친 것을 되돌릴 길이 도구 밖에만 남는다.
#[test]
fn promote_into_refuses_what_it_cannot_honour() {
    let s = init("ideaintobad");
    let epic = ok(s.path(), &["epic", "add", "선 에픽", "-q"]).trim().to_string();
    let work = add(s.path(), &["그냥 일"]);
    let id = ok(s.path(), &["idea", "add", "펼칠 것", "-q"]).trim().to_string();
    let before = issues(s.path());
    for (target, plan, why) in [
        (epic.as_str(), "# 또 에픽\n- 가\n", "`#` 줄"),
        ("t-none", "- 가\n", "없는 에픽"),
        (work.as_str(), "- 가\n", "에픽 아닌 것"),
    ] {
        for dry in [false, true] {
            let mut args = vec!["idea", "promote", &id, "-e", target, "--from", "-"];
            if dry {
                args.push("--dry-run");
            }
            let out = from_stdin(s.path(), &args, plan);
            assert!(!out.status.success(), "{why} 을 받았다 (dry={dry})");
        }
    }
    assert_eq!(issues(s.path()), before, "거절했는데 썼다");
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

/// **`done` 으로 옮기면 이로써 풀린 일을 한 줄로 댄다**(moai-942k, 사용자와 정함). `ready` 를
/// 다시 안 불러도 다음 일을 안다. 풀린 것이 없으면 말하지 않고, `--json` 은 `unblocked` 를
/// 늘 싣는다.
#[test]
fn mv_done_names_the_work_it_just_unblocked() {
    let s = init("mvfreed");
    let first = add(s.path(), &["막는 일"]);
    let waiting = add(s.path(), &["기다리는 일"]);
    ok(s.path(), &["link", &first, "--blocks", &waiting]);
    assert!(!ok(s.path(), &["ready"]).contains(&waiting), "막았는데 ready 에 섰다");

    let text = ok(s.path(), &["mv", &first, "done"]);
    let line = text.lines().find(|l| l.contains("풀림")).unwrap_or_else(|| panic!("풀린 일을 안 댄다 — {text}"));
    assert!(line.contains(&waiting) && line.contains("기다리는 일"), "{line:?}");
    assert!(ok(s.path(), &["ready"]).contains(&waiting), "댄 일이 ready 에 없다");

    // 풀린 것이 없으면 조용하다 — 기계 출력은 빈 배열을 싣는다.
    let lone = add(s.path(), &["홀로 선 일"]);
    let json = ok(s.path(), &["mv", &lone, "done", "--json"]);
    one_json_value(&json);
    assert!(json.contains(r#""unblocked":[]"#), "{json}");
    assert!(!ok(s.path(), &["mv", &waiting, "done"]).contains("풀림"), "풀린 것 없이 말했다");

    // 기계 출력은 풀린 줄을 싣는다.
    let a = add(s.path(), &["둘째 막는 일"]);
    let b = add(s.path(), &["둘째 기다리는 일"]);
    ok(s.path(), &["link", &a, "--blocks", &b]);
    let json = ok(s.path(), &["mv", &a, "done", "--json"]);
    assert!(json.contains(&format!(r#""unblocked":[{{"id":"{b}""#)), "{json}");
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

/// **저널만 못 적은 `add` 는 0 으로 끝나고 stderr 로 말한다**(moai-52z9). 실패로 끝나면
/// 사람이 다시 부르고, id 가 다른 같은 이슈가 둘 선다. 저널이 전부인 `note` 는 그대로 실패다.
#[cfg(unix)]
#[test]
fn an_add_whose_journal_fails_succeeds_and_says_so() {
    use std::os::unix::fs::PermissionsExt;
    let s = init("journalfail");
    let journal = s.path().join(".moai/journal.jsonl");
    std::fs::write(&journal, "").unwrap();
    std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().append(true).open(&journal).is_ok() {
        return; // root 는 권한을 안 본다
    }

    let out = moai(s.path(), &["add", "한 번만", "-q"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "담겼는데 실패로 끝났다 — {err}");
    assert!(err.contains("이력(journal.jsonl)은 못 남겼다"), "{err}");
    assert_eq!(issues(s.path()).matches("한 번만").count(), 1);

    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let note = moai(s.path(), &["note", &id, "메모"]);
    assert!(!note.status.success(), "아무것도 안 담긴 note 가 성공으로 끝났다");

    // 칸은 옮겨졌지만 `-m` 의 말은 저널에만 산다 — 다시 적을 길을 대야 한다.
    let mv = moai(s.path(), &["mv", &id, "in_progress", "-m", "까닭"]);
    let err = String::from_utf8_lossy(&mv.stderr);
    assert!(mv.status.success(), "{err}");
    assert!(err.contains("moai note") && err.contains(&id), "잃은 말을 안 댔다 — {err}");
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

/// **에픽 줄이 든 `epic` 은 소속이 아니다** (moai-fg0t, 사용자 결정 A). 트리는 에픽을 에픽
/// 밑에 안 그리는데 `-e` 거름망만 그 줄을 골랐다. 필드는 그대로 두고 status 가 못 쓸 참조로 댄다.
#[test]
fn an_epic_line_does_not_join_the_epic_it_names() {
    let s = init("epicinepic");
    let outer = ok(s.path(), &["epic", "add", "바깥", "-q"]).trim().to_string();
    let inner = add(s.path(), &["안쪽", "--type", "epic", "-e", &outer]);
    assert!(line_of(s.path(), &inner).contains(&format!(r#""epic":"{outer}""#)), "필드를 지웠다");
    assert!(!ok(s.path(), &["show", "-e", &outer]).contains(&inner), "`-e` 가 트리에 없는 에픽 줄을 골랐다");
    let shown = ok(s.path(), &["show", &inner]);
    assert!(shown.contains("에픽에 안 든다"), "상세가 그 필드를 소속처럼 그렸다 — {shown}");
    let st = ok(s.path(), &["status", "--json"]);
    let warning = |kind: &str| st.split("{\"kind\":").find(|w| w.starts_with(&format!("\"{kind}\""))).map(str::to_string);
    assert!(warning("dangling_epic").is_some_and(|w| w.contains(&inner)), "못 쓸 참조로 안 댔다 — {st}");
    assert!(moai(s.path(), &["status"]).status.success(), "경고로 비영 종료한다");
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
    let mut out = hook_in_raw(s, run_in, event, input);
    out.stdout = without_korean_notice(&String::from_utf8(out.stdout).unwrap()).into_bytes();
    out
}

/// 한국어 글 알림(moai-6rrb)의 첫 낱말. 훅의 글과 같아야 한다 — 어긋나면 알림을 못 걷어 위의 시험들이
/// 한꺼번에 붉어지니 저절로 드러난다.
const KOREAN_NOTICE: &str = "방금 moai 에 넣은 한국어 글을 다듬었는가";

/// **한국어 글 알림을 걷은 출력.** 훅 시험들은 한국어 제목을 표본으로 쓰면서 "막지 않았다·다른 비춤이
/// 없다" 를 빈 출력으로 잰다 — 한국어 글에 늘 붙는 알림이 그 자리를 다 붉게 만든다. 알림은 다른 비춤 뒤에
/// 이어 붙으므로(`Decision::then`) 그 뒤를 잘라 낸다. 알림 자체는 [`hook_in_raw`] 로 따로 잰다.
///
/// **알림이 설 자리가 아니면 걷지 않고 붉힌다**(리뷰 moai-5wk4.76z). 막는 답 안에, 두 번, 또는 다른 문단
/// 앞에 선 알림을 말없이 잘라 내면 모든 훅 시험이 그 어긋남을 가린 채 초록이 된다 — 알림의 글에는 문단
/// 가름(`\n\n`)이 없으니, 그 뒤에 가름이 있으면 다른 문단이 뒤에 선 것이다.
fn without_korean_notice(out: &str) -> String {
    let Some(at) = out.find(KOREAN_NOTICE) else { return out.to_string() };
    let key = "\"additionalContext\":\"";
    assert!(!out.contains("\"permissionDecision\"") && !out.contains("\"decision\""), "막는 답에 알림이 붙었다\n{out}");
    assert_eq!(out.matches(KOREAN_NOTICE).count(), 1, "알림이 두 번 섰다\n{out}");
    assert!(out[at..].ends_with("\"}}\n") && !out[at..].contains("\\n\\n"), "알림이 마지막 문단이 아니다\n{out}");
    if out[..at].ends_with(key) {
        return String::new();
    }
    let cut = out[..at].strip_suffix("\\n\\n").unwrap_or_else(|| panic!("알림이 문단 가름 없이 붙었다\n{out}"));
    assert!(cut.contains(key), "알림이 비추는 글 밖에 섰다\n{out}");
    format!("{cut}\"}}}}\n")
}

/// 훅을 띄워 **걷지 않은** 출력을 받는다.
fn hook_in_raw(s: &Scratch, run_in: &Path, event: &str, input: &str) -> Output {
    hook_at_home(s, run_in, None, event, input)
}

/// [`hook_in_raw`] 를 제 집(`claude` 의 장부가 놓이는 자리)에서 — 플러그인 장부를 흉내 내는 시험이 쓴다.
fn hook_at_home(s: &Scratch, run_in: &Path, home: Option<&Path>, event: &str, input: &str) -> Output {
    use std::io::Write as _;
    let tmp = s.path().join("hooktmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut cmd = staged(&["hook", event]);
    if let Some(home) = home {
        cmd.env("HOME", home);
    }
    let mut child = cmd
        .current_dir(run_in)
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

/// **한국어 글을 넣는 셸 호출에 다듬기 알림이 계약째로 붙는다**(moai-6rrb). 막지 않고, 이 저장소에
/// 플러그인이 없으면 사람에게 `moai skill install` 을 청하라고 한다. 영어 글과 읽기에는 아무 말도 없다.
/// 다른 훅 시험은 이 알림을 걷고 본다([`without_korean_notice`]) — 여기가 걷지 않고 보는 자리다.
#[test]
fn korean_text_going_into_moai_gets_a_polishing_notice() {
    let s = init("hookkorean");
    let raw = |cmd: &str| {
        let cwd = s.path().display().to_string();
        let input = format!(
            "{{\"session_id\":\"s1\",\"cwd\":{},\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{}}}}}",
            json_str(&cwd),
            json_str(cmd)
        );
        String::from_utf8(hook_in_raw(&s, s.path(), "pre-tool-use", &input).stdout).unwrap()
    };
    let out = raw("moai idea add '떠오른 것'");
    let said = carried_text(&out);
    assert!(said.starts_with(KOREAN_NOTICE), "{out}");
    assert!(!out.contains("permissionDecision"), "알림이 막는다\n{out}");
    assert!(said.contains("moai skill install"), "없는 플러그인을 안 비춘다\n{out}");
    for quiet in ["moai idea add 'an idea'", "moai show -g 한국어", "moai note x 'model: anthropic/opus-5 (low — 글)'"] {
        assert!(raw(quiet).trim().is_empty(), "헛 비춘다 — {quiet}\n{}", raw(quiet));
    }
    // 트래커가 없는 자리를 가리킨 `moai` 는 스스로 실패해 아무것도 안 넣는다(리뷰 moai-5wk4.76z).
    let nowhere = Scratch::new("hookkorean-nowhere");
    let failing = format!("moai -C {} note x '한글 노트'", nowhere.path().display());
    assert!(raw(&failing).trim().is_empty(), "안 넣은 글을 비춘다\n{}", raw(&failing));
}

/// **딸린 워크트리는 주 체크아웃에 깐 플러그인을 깔린 것으로 읽는다**(리뷰 moai-5wk4.76z) — `claude` 가 설치를
/// "여기" 로 치는 자와 같다. 주 체크아웃에 `local` 로 깐 뒤에도 워크트리에서 한국어 글을 적을 때마다 "깔려
/// 있지 않다" 며 사람을 부르게 하던 자리다 — 시킨 대로 다시 깔아도 그 줄은 안 꺼졌다.
#[test]
fn a_linked_worktree_sees_the_korean_plugins_of_its_main_checkout() {
    let s = Scratch::new("hookkoreanwt");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "init"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/argos-wt", "-b", "worktree-argos-wt"]);
    let inside = main.join(".claude/worktrees/argos-wt");
    let home = s.path().join("home");
    std::fs::create_dir_all(home.join(".claude/plugins")).unwrap();
    let ledger = |at: &Path| {
        let row = format!(
            "[{{\"scope\":\"local\",\"projectPath\":{},\"installPath\":\"/x\",\"version\":\"1\"}}]",
            json_str(&at.display().to_string())
        );
        let body = format!("{{\"version\":2,\"plugins\":{{\"korean-skills@korean-skills\":{row},\"humanize-korean@im-not-ai\":{row}}}}}");
        std::fs::write(home.join(".claude/plugins/installed_plugins.json"), body).unwrap();
    };
    let said = |cwd: &Path| {
        let input = format!(
            "{{\"session_id\":\"s1\",\"cwd\":{},\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{}}}}}",
            json_str(&cwd.display().to_string()),
            json_str(&format!("moai -C {} idea add '떠오른 것'", main.display()))
        );
        carried_text(&String::from_utf8(hook_at_home(&s, cwd, Some(&home), "pre-tool-use", &input).stdout).unwrap())
    };
    ledger(&main);
    for cwd in [&main, &inside] {
        let out = said(cwd);
        assert!(out.starts_with(KOREAN_NOTICE), "{out}");
        assert!(!out.contains("moai skill install"), "주 체크아웃에 깐 것을 없다고 한다 — {}\n{out}", cwd.display());
    }
    // 이 저장소 밖에 깐 것은 여전히 없는 것이다.
    ledger(&s.path().join("elsewhere"));
    assert!(said(&inside).contains("moai skill install"), "남의 자리의 설치를 제 것으로 읽는다");
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
        "moai add --from - <<'MD'\n# 딴 에픽\n- 첫 이슈\nMD".to_string(),
        // 리뷰 이야기를 적는 메모는 글자로 가르지 않는다.
        format!("moai note {id} \"code-review 가 moai add 를 짚었다\""),
    ] {
        let out = shell_call(&s, &free);
        assert!(out.trim().is_empty(), "막혔다 — {free}\n{out}");
    }

    // 담는 것은 막지 않고 **갈림길 1 의 둘째 물음을 싣는다**(moai-d4e0) — 계약의 `additionalContext` 로.
    let out = shell_call(&s, "moai idea add \"떠오른 것\"");
    one_json_value(&out);
    assert!(!out.contains("permissionDecision"), "담는 것을 막았다\n{out}");
    assert!(out.contains("\"additionalContext\":\"") && out.contains(&format!("{epic} 가 내건 것")), "{out}");
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
    // 내미는 줄은 main 의 트래커를 겨눈다(moai-gyqh).
    let root = std::fs::canonicalize(&main).unwrap().display().to_string();
    assert!(own.contains(&format!("moai -C {root} mv {there}")), "제 워크트리에서 제 일을 놓친다\n{own}");
}

/// **이름이 id 가 아닌 워크트리가 갈라질 때 이미 집혀 있던 일은 그 워크트리의 것일 수 있다**
/// (moai-ntl6, 사용자 결정 B). 에이전트 격리 워크트리(`worktree-agent-<해시>`)와 옛 id 로 뜬
/// 워크트리가 실제로 그랬다(moai-apsa·nt0h). 누구의 것인지 모르는 줄로는 막지도 붙들지도
/// 않는다. 갈라진 **뒤에** main 에서 집은 일은 여전히 main 의 초점이다.
#[test]
fn work_picked_before_an_unnamed_worktree_branched_is_left_to_it() {
    let s = Scratch::new("hookbranchpoint");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let agent = field(&ok(&main, &["add", "에이전트가 할 일", "--json"]), "id");
    let renamed = field(&ok(&main, &["add", "옛 id 에서 옮긴 일", "--json"]), "id");
    ok(&main, &["mv", &agent, "in_progress"]);
    ok(&main, &["mv", &renamed, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/agent-a04acfb3", "-b", "worktree-agent-a04acfb3"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/argos-old1", "-b", "worktree-argos-old1"]);

    let at_main = |event: &str, session: &str, tool: Option<&str>| {
        let body = tool.map_or(String::new(), |cmd| {
            format!(",\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":{}}}", json_str(cmd))
        });
        let input = format!("{{\"session_id\":\"{session}\",\"cwd\":{}{body}}}", json_str(&main.display().to_string()));
        String::from_utf8(hook_in(&s, &main, event, &input).stdout).unwrap()
    };

    // 갈라질 때 집혀 있던 일로는 막지도 붙들지도 않는다.
    let out = at_main("pre-tool-use", "s1", Some("moai add \"딴 일\""));
    assert!(out.trim().is_empty(), "옆 워크트리가 쥐었을 일로 main 의 생성을 막는다\n{out}");
    let out = at_main("stop", "s1", None);
    assert!(out.trim().is_empty(), "옆 워크트리가 쥐었을 일로 main 세션을 붙든다\n{out}");

    // 갈라진 뒤 main 에서 집은 일은 main 의 초점이다 — 막고, 붙들고, 그것만 댄다.
    let mine = field(&ok(&main, &["add", "main 에서 집은 일", "--json"]), "id");
    ok(&main, &["mv", &mine, "in_progress"]);
    let why = refusal(&at_main("pre-tool-use", "s2", Some("moai add \"딴 일\"")));
    assert!(why.contains(&mine) && !why.contains(&agent) && !why.contains(&renamed), "{why}");
    let out = at_main("pre-tool-use", "s2", Some(&format!("moai add \"자식\" --parent {mine}")));
    assert!(out.trim().is_empty(), "main 의 일의 자식을 막았다\n{out}");
    let held = at_main("stop", "s2", None);
    assert!(held.contains(&format!("moai mv {mine}")), "main 에서 집은 일을 안 붙든다\n{held}");
    assert!(!held.contains(&agent) && !held.contains(&renamed), "옆이 쥐었을 일을 옮기라고 한다\n{held}");
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

/// **자리 경로는 늘 main 에서 잰 상대 경로다**(moai-xpd7·moai-3aec, 2026-09-18 사용자 결정) — main
/// 밖에 만든 워크트리는 `../` 로 올라가서 잰다. 절대 경로로 두던 판은 한 배열에 두 모양이 섞였고
/// 기계의 홈 경로가 `--json` 으로 나갔다. main 이 없는 맨몸 저장소는 그 저장소 디렉터리에서 재어,
/// 어느 워크트리에서 불러도 같은 글자가 나온다.
#[test]
fn a_worktree_outside_main_is_named_relative_to_main() {
    let s = Scratch::new("outsideplace");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let id = field(&ok(&main, &["add", "밖에서 할 일", "--json"]), "id");
    ok(&main, &["mv", &id, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", &format!("../{id}"), "-b", &format!("worktree-{id}")]);

    let json = ok(&main, &["show", &id, "--json"]);
    assert!(json.contains(&format!(r#""workplaces":[{{"path":"../{id}","#)), "main 밖 워크트리를 ../ 로 안 쟀다\n{json}");
    let root = s.path().to_str().unwrap();
    assert!(!json.contains(root), "기계의 절대 경로가 --json 으로 나갔다\n{json}");
    let line = ok(&main, &["show", &id]);
    assert!(line.contains(&format!("../{id}")) && !line.contains(root), "사람 화면의 자리가 다른 자로 쟀다\n{line}");

    // 맨몸 저장소 — main 이 없다. 딸린 워크트리 둘 가운데 어디서 불러도 같은 글자다.
    let bare = s.path().join("repo.git");
    git(s.path(), &["clone", "-q", "--bare", main.to_str().unwrap(), bare.to_str().unwrap()]);
    git(&bare, &["worktree", "add", "-q", "../w1", "-b", "w1"]);
    git(&bare, &["worktree", "add", "-q", &format!("../b-{id}"), &format!("worktree-{id}")]);
    for at in ["w1".to_string(), format!("b-{id}")] {
        let json = ok(&s.path().join(&at), &["show", &id, "--json", "--worktree"]);
        assert!(json.contains(&format!(r#""path":"../b-{id}""#)), "{at} 에서 부르니 다른 자로 쟀다\n{json}");
    }
}

/// **집었는데 일하는 워크트리가 없는 줄은 `status` 가 비춘다**(moai-4370) — 세션이 죽어도 칸은
/// `in_progress` 로 남는다. 막지 않는다: 종료 코드는 0 이고 `--json` 의 `warnings` 에 선다. 워크트리가
/// 뜬 일은 안 세고, 방금 집은 일은 워크트리가 뜰 틈을 준다.
#[test]
fn status_names_picked_work_with_no_live_worktree_and_still_exits_zero() {
    let s = Scratch::new("stranded");
    let (main, inside, id) = picked_in_a_worktree(&s);
    let lost = field(&ok(&main, &["add", "세션이 죽은 일", "--json"]), "id");
    ok(&main, &["mv", &lost, "in_progress"]);

    // 방금 집은 일은 아직 안 센다.
    let out = ok(&main, &["status"]);
    assert!(!out.contains("일하는 워크트리가 없는"), "워크트리가 뜰 틈을 안 줬다\n{out}");

    let out = isolated(BIN).arg("status").current_dir(&main).env("MOAI_NOW", LATER).env("NO_COLOR", "1").output().unwrap();
    assert!(out.status.success(), "경고로 비영 종료했다");
    let text = String::from_utf8(out.stdout).unwrap();
    let block: String = text.split("집었는데 일하는 워크트리가 없는 것 1건").nth(1).expect(&text).split("\n\n").next().unwrap().into();
    assert!(block.contains(&lost) && !block.contains(&id), "워크트리가 뜬 일까지 셌다\n{text}");
    assert!(block.contains("moai mv <id> todo"), "고칠 손을 안 댔다\n{text}");
    let json = ok_at(&main, LATER, &["status", "--json"]);
    assert!(json.contains("\"kind\":\"stranded\"") && json.contains(&lost), "{json}");

    // **물려받은 줄은 자리가 아니다**(moai-ir8q.beq). 자리 잃은 줄이 커밋된 뒤 다른 일의 워크트리가
    // 뜨면 그 스냅샷에도 벌여 놓여 있다 — 그것을 자리로 세면 워크트리가 하나 뜨는 순간 사라진다.
    // 그 워크트리 안에서 집은 줄은 거기서 만진 흔적이라 자리다 — `--worktree` 로 겹쳐 봐도 그렇다.
    let there = field(&ok(&main, &["add", "옆에서 할 일", "--json"]), "id");
    let moved = field(&ok(&main, &["add", "옆에서 집을 일", "--json"]), "id");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "자리 잃은 줄을 커밋한다"]);
    let dir = format!(".claude/worktrees/{there}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &format!("worktree-{there}")]);
    let side = main.join(&dir);
    ok_at(&side, "2026-09-11T06:00:00Z", &["mv", &moved, "in_progress"]);
    let stranded_ids = |json: &str| -> String {
        json.split("\"kind\":\"stranded\"").nth(1).map(|t| t.split('}').next().unwrap().to_string()).unwrap_or_default()
    };
    for args in [&["status", "--json"][..], &["status", "--worktree", "--json"][..]] {
        let json = ok_at(&main, LATER, args);
        let ids = stranded_ids(&json);
        assert!(ids.contains(&lost), "{args:?}: 물려받은 줄로 자리를 댔다\n{json}");
        assert!(!ids.contains(&moved), "{args:?}: 워크트리 안에서 집은 줄을 자리 없다고 했다\n{json}");
    }

    // **딸린 워크트리의 스냅샷은 갈라질 때의 main 이다** — 그 뒤 main 에서 놓은 줄이 거기서는 아직
    // 집혀 있다. 겹쳐 보지 않고 그것으로 재면 끝난 일을 남에게 다시 준다.
    ok(&main, &["mv", &lost, "todo"]);
    let json = ok_at(&side, LATER, &["status", "--json"]);
    assert!(!json.contains("\"stranded\""), "낡은 스냅샷으로 자리 없는 줄을 댔다\n{json}");

    // 워크트리를 치우면 그 일도 자리를 잃는다. 워크트리가 하나도 없으면 조용하다 — 그때는 main 에서 일한다.
    git(&main, &["worktree", "remove", "--force", &inside.display().to_string()]);
    git(&main, &["worktree", "remove", "--force", &side.display().to_string()]);
    let json = ok_at(&main, LATER, &["status", "--json"]);
    assert!(!json.contains("\"stranded\""), "워크트리를 안 쓰는 저장소에서 떠들었다\n{json}");
}

/// **자리를 셀 일이 없으면 옆 스냅샷을 안 판다**(moai-7igy, 사용자 결정). `moai status` 는 세션마다·
/// 훅마다 도는데, 워크트리 일곱이면 스냅샷 여덟 벌을 다시 파 40→95ms(따뜻)·138→458ms(참)이었다.
///
/// **판 것을 어떻게 아는가** — 깨진 스냅샷을 둔 워크트리를 판 순간 `status` 가 "못 읽었다" 를 낸다
/// (moai-lt7h). 안 판 경우에는 그 말이 없다. 집은 줄이 없을 때와, 이름으로 자리가 다 잡힐 때가 그렇다.
#[test]
fn status_reads_a_side_snapshot_only_when_a_place_is_still_missing() {
    let s = Scratch::new("placecost");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let named = field(&ok(&main, &["add", "이름이 붙은 일", "--json"]), "id");
    ok(&main, &["mv", &named, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", &format!(".claude/worktrees/{named}"), "-b", &format!("worktree-{named}")]);
    // 이름이 id 가 아닌 워크트리 하나 — 판정이 스냅샷으로 넘어가는 유일한 길이다.
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/agent-x", "-b", "worktree-agent-x"]);
    // 못 읽는 스냅샷 — 깨진 줄은 `store` 가 견디므로(그것이 규약이다) 아예 못 여는 것으로 만든다.
    let broken = main.join(".claude/worktrees/agent-x/.moai/issues.jsonl");
    std::fs::remove_file(&broken).unwrap();
    std::fs::create_dir(&broken).unwrap();

    // 집은 줄이 이름으로 다 자리를 잡았다 — 깨진 스냅샷을 안 판다.
    let out = isolated(BIN).arg("status").current_dir(&main).env("MOAI_NOW", LATER).env("NO_COLOR", "1").output().unwrap();
    let said = String::from_utf8(out.stderr).unwrap();
    assert!(said.is_empty(), "이름으로 답이 나왔는데 옆 스냅샷을 팠다\n{said}");

    // 자리를 못 찾은 줄이 생기면 그때 판다 — 그리고 못 읽었다고 말한다.
    let lost = field(&ok(&main, &["add", "자리 없는 일", "--json"]), "id");
    ok(&main, &["mv", &lost, "in_progress"]);
    let out = isolated(BIN).arg("status").current_dir(&main).env("MOAI_NOW", LATER).env("NO_COLOR", "1").output().unwrap();
    let said = String::from_utf8(out.stderr).unwrap();
    assert!(said.contains("못 읽었다") && said.contains("agent-x"), "못 읽은 워크트리를 말하지 않았다\n{said}");
    // **떠들어도 0 으로 끝난다** — 이것은 남의 워크트리의 문제고, `moai status` 는 아무것도 막지
    // 않는다(CLAUDE.md). 비영으로 끝나는 순간 에이전트가 이것을 고장으로 읽는다.
    assert!(out.status.success(), "못 읽은 워크트리로 비영 종료했다");
    // **못 읽은 워크트리가 있으면 자리 없다고 단정하지 않는다**(moai-lt7h) — 거기일 수 있다.
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("일하는 워크트리가 없는"), "모르는 것을 자리 없음으로 셌다\n{text}");
    let out = ok_at(&main, LATER, &["show", &lost]);
    assert!(out.contains("자리   모른다"), "{out}");
    assert!(ok_at(&main, LATER, &["show", &lost, "--json"]).contains("\"place\":\"unknown\""));
    // **기계도 같은 것을 가른다**(리뷰 moai-ya06) — `stranded` 가 조용한 것이 "자리 잃은 일이
    // 없다" 인지 "못 셌다" 인지를 `--json` 만 읽는 쪽도 알아야 한다. 감독 스킬이 이 목록으로 죽은
    // 세션의 일을 거두므로, 그 침묵이 곧 일을 영영 안 거두는 것이 된다.
    let json = ok_at(&main, LATER, &["status", "--json"]);
    assert!(!json.contains("\"stranded\""), "모르는 것을 자리 없음으로 셌다\n{json}");
    // 가지와 **경로를 함께** 낸다 — 떼어 낸 HEAD 는 이름이 커밋 앞 일곱 자라 가지만으로는 두
    // 워크트리가 글자까지 같아진다. 경로는 `show` 와 같이 꼭대기에서 줄여 절대 경로를 안 낸다.
    assert!(
        json.contains(r#""unreadable_worktrees":[{"path":".claude/worktrees/agent-x","branch":"worktree-agent-x"}]"#),
        "기계에게는 안 댔다\n{json}"
    );
    assert!(!json.contains(&main.display().to_string()), "기계의 절대 경로가 그대로 나갔다\n{json}");

    // **겹쳐 볼 때는 같은 워크트리를 두 번 말하지 않는다** — `--worktree` 면 `gather` 가 옆 스냅샷을
    // 빠짐없이 열어 `⎇ <가지>: …` 로 이미 냈다. 두 줄로 내면 보드의 `옆 워크트리 문제 N건` 이
    // 깨진 워크트리 하나를 둘로 세어, 보는 쪽이 두 곳이 깨진 줄로 읽는다.
    let out =
        isolated(BIN).args(["status", "--worktree"]).current_dir(&main).env("MOAI_NOW", LATER).env("NO_COLOR", "1").output().unwrap();
    let said = String::from_utf8(out.stderr).unwrap();
    assert_eq!(said.lines().count(), 1, "깨진 워크트리 하나를 두 줄로 말했다\n{said}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("옆 워크트리 문제 1건"), "깨진 워크트리 하나를 둘로 셌다\n{text}");

    // 스냅샷이 멀쩡해지면 자리 없음이 제대로 선다.
    std::fs::remove_dir(&broken).unwrap();
    std::fs::copy(main.join(".moai/issues.jsonl"), &broken).unwrap();
    let text = ok_at(&main, LATER, &["status"]);
    assert!(text.contains("일하는 워크트리가 없는 것 1건") && text.contains(&lost), "{text}");
    let json = ok_at(&main, LATER, &["status", "--json"]);
    assert!(!json.contains("unreadable_worktrees"), "다 읽었는데 못 읽었다고 했다\n{json}");
}

/// **가려진 줄도 옆 스냅샷을 판다**(moai-es40, 에픽 끝 리뷰 moai-r8gw.b4s). `report::wip` 은 종류가 다른
/// 쌍둥이에게 가려진 줄을 빼지만 자리 셈은 그 줄까지 본다(`report::started`, 사용자 결정) — 팔지 가르는
/// 문(`worktree::workplaces`)이 `wip` 으로 재면 옆에서 도는 그 줄이 스냅샷을 안 판 채 `stranded` 와
/// `show` 의 `자리 없다` 로 서고, 감독이 그 말대로 산 일에 둘째 워크트리를 띄운다.
#[test]
fn an_eclipsed_picked_row_is_not_stranded_while_a_side_works_it() {
    let s = Scratch::new("eclipsedplace");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let id = field(&ok(&main, &["add", "옆에서 할 일", "--json"]), "id");
    ok(&main, &["mv", &id, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    // 이름이 id 가 아닌 워크트리(에이전트 격리)가 그 줄을 main 보다 늦게 고쳤다(`touched`).
    let dir = ".claude/worktrees/agent-x";
    git(&main, &["worktree", "add", "-q", dir, "-b", "worktree-agent-x"]);
    ok_at(&main.join(dir), "2026-09-11T06:00:00Z", &["edit", &id, "--tag", "side"]);
    // 머지가 main 에 같은 id 의 에픽 뒷줄을 남겼다 — 앞줄의 집힌 이슈가 가려진다.
    let path = main.join(".moai/issues.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!(
        "{{\"id\":\"{id}\",\"kind\":\"epic\",\"title\":\"쌍둥이 에픽\",\"status\":\"todo\",\"created_at\":\"2026-09-11T00:00:00Z\",\"updated_at\":\"2026-09-11T00:00:00Z\",\"status_since\":\"2026-09-11T00:00:00Z\"}}\n"
    ));
    std::fs::write(&path, text).unwrap();

    // `duplicate_id` 는 치명이라 비영으로 끝난다 — 종료 코드가 아니라 경고를 본다.
    let out = isolated(BIN).args(["status", "--json"]).current_dir(&main).env("MOAI_NOW", LATER).output().unwrap();
    let json = String::from_utf8(out.stdout).unwrap();
    assert!(json.contains("\"duplicate_id\""), "쌍둥이를 못 세웠다 — 이 시험이 견줄 것이 없다\n{json}");
    assert!(!json.contains("\"stranded\""), "옆에서 도는 가려진 줄을 자리 없다고 했다\n{json}");
    let shown = ok_at(&main, LATER, &["show", &id, "--json"]);
    assert!(shown.contains("\"place\":\"at\"") && shown.contains(dir), "{shown}");
}

/// **판정을 안 가리는 못 읽은 워크트리는 말은 하되 "다 못 셌다" 로 세지 않는다**(moai-rgz9·moai-1i9d,
/// 사용자 결정 2026-09-18).
/// 규약의 워크트리 이름은 에픽 id 다(`worktree-<에픽>`). 그 워크트리의 스냅샷을 못 읽어도 이름이
/// 집은 멤버를 가리키므로 그 멤버는 이미 제 자리에 섰고, 딴 줄의 자리도 가리지 않는다 — 그러면
/// 자리 잃은 딴 줄이 그대로 `stranded` 에 서야 하고, 어느 표면도 "못 읽었다" 를 대지 않아야 한다.
/// 한때 판정은 에픽 이름을 못 알아봐 딴 줄까지 "모른다" 로 덮었고, 표면은 판정과 따로 못 읽은
/// 워크트리를 다 세어 화면마다 "자리를 다 못 셌다" 가 섰다.
#[test]
fn an_unreadable_epic_worktree_is_told_of_but_does_not_blind() {
    let s = Scratch::new("epicblind");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let epic = ok(&main, &["epic", "add", "에픽", "-q"]).trim().to_string();
    let member = field(&ok(&main, &["add", "에픽의 일", "-e", &epic, "--json"]), "id");
    ok(&main, &["mv", &member, "in_progress"]);
    // 자리를 잃은 딴 줄 — 이것이 있어야 옆 스냅샷을 판다(`worktree::workplaces` 의 문).
    let lost = field(&ok(&main, &["add", "자리 없는 일", "--json"]), "id");
    ok(&main, &["mv", &lost, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", &format!(".claude/worktrees/{epic}"), "-b", &format!("worktree-{epic}")]);
    let broken = main.join(format!(".claude/worktrees/{epic}/.moai/issues.jsonl"));
    std::fs::remove_file(&broken).unwrap();
    std::fs::create_dir(&broken).unwrap();

    // **깨졌다는 말은 산다**(사용자 결정 2026-09-18) — 판정을 가렸는지와 별개로, 그 스냅샷을 고칠
    // 사람이 있어야 고쳐진다. 가린 것만 세는 자리는 `--json` 의 `unreadable_worktrees` 와 층이다.
    let out = isolated(BIN).arg("status").current_dir(&main).env("MOAI_NOW", LATER).env("NO_COLOR", "1").output().unwrap();
    let said = String::from_utf8(out.stderr).unwrap();
    assert!(said.contains("못 읽었다") && said.contains(&epic), "깨진 스냅샷을 아무 데서도 안 말했다\n{said}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("일하는 워크트리가 없는 것 1건") && text.contains(&lost), "에픽 워크트리가 딴 줄을 가렸다\n{text}");
    assert!(text.contains("옆 워크트리 문제 1건"), "깨진 스냅샷을 보드가 안 셌다\n{text}");
    let json = ok_at(&main, LATER, &["status", "--json"]);
    assert!(!json.contains("unreadable_worktrees"), "판정을 안 가리는 워크트리를 '못 셌다' 로 댔다\n{json}");
    // **기계도 깨진 스냅샷을 듣는다**(moai-zah3) — 판정을 안 가려도 곁의 키가 댄다.
    assert!(
        json.contains(&format!(r#""broken_worktrees":[{{"path":".claude/worktrees/{epic}","branch":"worktree-{epic}"}}]"#)),
        "깨진 스냅샷이 기계가 읽는 자리에 없다\n{json}"
    );
    assert!(json.contains("\"stranded\""), "에픽 워크트리가 딴 줄을 가렸다\n{json}");
    assert!(ok_at(&main, LATER, &["show", &member, "--json"]).contains("\"place\":\"at\""));

    // 밖 한눈 보기와 탐색기의 층도 같은 자다(`worktree::stranded_at`).
    let home = Scratch::new("epicblind-home");
    let config = registry(&home, &[main.as_path()]);
    let outside = dir_in(&home, "밖");
    let seen = isolated(BIN)
        .arg("status")
        .current_dir(&outside)
        .env("MOAI_CONFIG", &config)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let seen = String::from_utf8(seen.stdout).unwrap();
    assert!(seen.contains("못 읽었다") && seen.contains("옆 워크트리 문제 1건"), "밖에서 깨진 스냅샷을 안 댔다\n{seen}");
    assert!(seen.contains(&lost), "밖에서 자리 잃은 줄을 안 댔다\n{seen}");
    let layer = isolated(BIN)
        .args(["tui", "--json"])
        .current_dir(&outside)
        .env("MOAI_CONFIG", &config)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let layer = String::from_utf8(layer.stdout).unwrap();
    // 층은 0 이면 키를 안 단다 — 서 있으면 셌다는 뜻이다.
    assert!(!layer.contains("\"unreadable_worktrees\"") && layer.contains("\"stranded\":1"), "{layer}");
    assert!(layer.contains("\"broken_worktrees\":1"), "층의 기계 출력이 깨진 스냅샷을 안 셌다\n{layer}");
    let machine = isolated(BIN)
        .args(["status", "--json"])
        .current_dir(&outside)
        .env("MOAI_CONFIG", &config)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let machine = String::from_utf8(machine.stdout).unwrap();
    assert!(
        machine.contains("\"broken_worktrees\":[") && !machine.contains("\"unreadable_worktrees\""),
        "밖 한눈 보기의 기계 출력이 안쪽과 다르게 댔다\n{machine}"
    );
}

/// **`gather` 가 실제로 셌을 때만 입을 다문다**(리뷰 moai-ya06). `--worktree` 면 그쪽이 같은
/// 워크트리를 `⎇ <가지>: …` 로 이미 내므로 여기서는 안 내는데, 그 전제는 **`gather` 가 옆을
/// 셀 수 있었을 때만** 선다 — 저쪽은 git 을 불러 세고(`others_of`) 이쪽은 git 이 적어 둔 파일만
/// 읽으므로(`on_disk`), git 이 없으면 저쪽은 "못 찾았다" 한 줄만 내고 워크트리를 한 곳도 안
/// 대는데 이쪽은 그대로 찾아 낸다. 그때까지 입을 다물면 깨진 워크트리를 아무도 말하지 않고
/// `stranded` 까지 조용해진다.
#[test]
fn overlaying_without_git_still_names_an_unreadable_worktree() {
    let s = Scratch::new("placenogit");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "처음"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/agent-x", "-b", "worktree-agent-x"]);
    // 워크트리가 갈라진 **뒤에** 집는다 — 옆 스냅샷에는 없으니 이름으로도 스냅샷으로도 안 잡힌다.
    let lost = field(&ok(&main, &["add", "자리 없는 일", "--json"]), "id");
    ok(&main, &["mv", &lost, "in_progress"]);
    let broken = main.join(".claude/worktrees/agent-x/.moai/issues.jsonl");
    std::fs::remove_file(&broken).unwrap();
    std::fs::create_dir(&broken).unwrap();

    // git 을 못 부르게 한다 — `gather` 는 옆을 못 세고, `workplaces` 는 파일로 그대로 센다.
    let empty = s.path().join("nogit");
    std::fs::create_dir_all(&empty).unwrap();
    let out = isolated(BIN)
        .args(["status", "--worktree"])
        .current_dir(&main)
        .env("PATH", &empty)
        .env("MOAI_NOW", LATER)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let said = String::from_utf8(out.stderr).unwrap();
    assert!(out.status.success(), "git 이 없다고 비영 종료했다\n{said}");
    assert!(said.contains("못 읽었다") && said.contains("agent-x"), "아무도 깨진 워크트리를 안 댔다\n{said}");
}

/// **팔지 말지는 부르는 쪽이 거는 줄로 잰다**(moai-7igy). 겹쳐 보는 쪽(`--worktree`)은 옆에서
/// 만들고 집은 줄까지 `stranded` 와 `자리` 에 거는데, 팔 까닭을 main 의 스냅샷으로만 재면 그 줄은
/// 거기 없어 "이름으로 다 잡혔다" 에 조용히 들어간다 — 그러면 `holds` 가 빈 채로 나가 살아 있는
/// 세션의 일이 통째로 자리를 잃는다.
#[test]
fn overlaying_places_work_that_only_a_side_worktree_knows_about() {
    let s = Scratch::new("placeside");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    // main 의 집은 줄은 이름으로 잡힌다 — 이것만 보면 더 팔 까닭이 없어 보인다.
    let named = field(&ok(&main, &["add", "이름이 붙은 일", "--json"]), "id");
    ok(&main, &["mv", &named, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", &format!(".claude/worktrees/{named}"), "-b", &format!("worktree-{named}")]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/agent-x", "-b", "worktree-agent-x"]);

    // 이름이 id 가 아닌 워크트리가 제 줄을 만들고 집는다 — main 의 스냅샷에는 없다.
    let side = main.join(".claude/worktrees/agent-x");
    let mine = field(&ok(&side, &["add", "옆에서 만든 일", "--json"]), "id");
    ok(&side, &["mv", &mine, "in_progress"]);

    let text = ok_at(&main, LATER, &["status", "--worktree"]);
    assert!(!text.contains("일하는 워크트리가 없는"), "옆에서 집은 산 일을 자리 없음으로 셌다\n{text}");
    let out = ok_at(&main, LATER, &["show", &mine, "--worktree"]);
    assert!(out.contains("자리   .claude/worktrees/agent-x"), "{out}");
}

/// **`show <에픽>` 도 자리를 낸다**(moai-0h8m) — 워크트리 이름은 규약상 에픽 id 라, 이어받는 세션이
/// 에픽부터 읽는다. 묶음은 집히지 않으므로 멤버의 자리를 굴려 올린다.
#[test]
fn show_rolls_a_groups_place_up_from_its_members() {
    let s = Scratch::new("placeepic");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let epic = field(&ok(&main, &["epic", "add", "저장 계층", "--json"]), "id");
    let one = field(&ok(&main, &["add", "첫 일", "-e", &epic, "--json"]), "id");
    ok(&main, &["mv", &one, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", &format!(".claude/worktrees/{epic}"), "-b", &format!("worktree-{epic}")]);

    let out = ok_at(&main, LATER, &["show", &epic]);
    let line = out.lines().find(|l| l.trim_start().starts_with("자리")).unwrap_or_else(|| panic!("에픽에 자리가 없다\n{out}"));
    assert!(line.contains(&format!(".claude/worktrees/{epic}")), "{line}");
    assert!(ok_at(&main, LATER, &["show", &epic, "--json"]).contains(&format!("\"branch\":\"worktree-{epic}\"")));
}

/// **`show <id>` 는 집은 줄이 어느 워크트리에서 돌고 있는지 댄다**(moai-6opu) — 이어받는 세션이
/// 그 자리로 들어가면 된다. 자리 없는 집은 줄은 그렇다고 말한다. 안 집은 줄과 워크트리를 안 쓰는
/// 저장소에는 줄을 안 세운다. `--json` 도 같은 것을 `workplaces` 로 낸다.
#[test]
fn show_names_the_worktree_a_picked_row_lives_in() {
    let s = Scratch::new("showplace");
    let (main, inside, id) = picked_in_a_worktree(&s);
    let lost = field(&ok(&main, &["add", "세션이 죽은 일", "--json"]), "id");
    ok(&main, &["mv", &lost, "in_progress"]);
    let idle = field(&ok(&main, &["add", "안 집은 일", "--json"]), "id");

    let place = |out: &str| -> String {
        out.lines()
            .find(|l| l.trim_start().starts_with("자리"))
            .unwrap_or_else(|| panic!("자리 줄이 없다\n{out}"))
            .to_string()
    };
    let out = ok(&main, &["show", &id]);
    let line = place(&out);
    assert!(line.contains(&format!(".claude/worktrees/{id}")) && line.contains(&format!("worktree-{id}")), "{line}");
    // **경로는 뿌리에서 잰 것이다** — 절대 경로가 그대로 나가면 위의 `contains` 도 지나가므로 여기서 못박는다.
    assert!(!line.contains(&main.display().to_string()), "뿌리에서 안 잘랐다\n{line}");
    let json = ok(&main, &["show", &id, "--json"]);
    assert!(json.contains("\"workplaces\":[{") && json.contains(&format!("\"branch\":\"worktree-{id}\"")), "{json}");

    // **방금 집은 줄에는 `status` 와 같은 한 시간 틈을 준다**(moai-xn9n) — 규약은 집고 커밋한 뒤
    // 워크트리를 띄우므로, 그 사이를 "없다" 로 대면 멀쩡한 줄이 버려진 것처럼 읽힌다.
    let out = ok(&main, &["show", &lost]);
    assert!(out.contains("자리   아직"), "방금 집은 줄을 버려진 것처럼 냈다\n{out}");
    assert!(ok(&main, &["show", &lost, "--json"]).contains("\"place\":\"fresh\""));
    let out = ok_at(&main, LATER, &["show", &lost]);
    assert!(out.contains("자리   없다"), "자리 없는 줄을 말하지 않았다\n{out}");
    let json = ok_at(&main, LATER, &["show", &lost, "--json"]);
    assert!(json.contains("\"workplaces\":[]") && json.contains("\"place\":\"lost\""), "{json}");

    let out = ok(&main, &["show", &idle]);
    assert!(!out.contains("자리"), "안 집은 줄에 자리를 세웠다\n{out}");
    assert!(!ok(&main, &["show", &idle, "--json"]).contains("workplaces"));

    // **딸린 워크트리 안에서 펼쳐도 경로는 main 에서 잰 상대 경로다**(moai-fygk) — 제 꼭대기로
    // 재면 옆 워크트리가 하나도 안 잘려 기계의 절대 경로가 그대로 나간다. 규약상 세션은 대개
    // 워크트리 안에서 도므로 그쪽이 흔한 자리다.
    let other = field(&ok(&main, &["add", "옆에서 할 일 하나 더", "--json"]), "id");
    ok(&main, &["mv", &other, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "하나 더 집는다"]);
    let dir2 = format!(".claude/worktrees/{other}");
    git(&main, &["worktree", "add", "-q", &dir2, "-b", &format!("worktree-{other}")]);
    let line = place(&ok(&inside, &["show", &other, "--worktree"]));
    assert!(line.contains(&dir2), "main 에서 잰 상대 경로가 아니다\n{line}");
    assert!(!line.contains(&main.display().to_string()), "옆 워크트리가 절대 경로로 샜다\n{line}");

    // **제 워크트리 안에서 펼쳐도 자리는 빈 칸이 아니다** — 뿌리와 같은 자리라 잘라 내면 아무것도
    // 안 남는다. 딸린 워크트리는 `--worktree` 로 겹쳐 봐야 자리를 잰다.
    let line = place(&ok(&inside, &["show", &id, "--worktree"]));
    assert!(line.split_whitespace().nth(1).is_some_and(|p| !p.starts_with('(')), "자리 칸이 비었다\n{line}");
    assert!(!ok(&inside, &["show", &id, "--worktree", "--json"]).contains("\"path\":\"\""));

    // **겹쳐 보지 않은 딸린 워크트리는 자리를 말하지 않는다**(moai-4370 과 같은 까닭) — 그 스냅샷은
    // 갈라질 때의 main 이라, main 이 놓은 줄을 거기서는 아직 집힌 것으로 보고 "자리 없다" 로 댄다.
    let there = field(&ok(&main, &["add", "옆에서 할 일", "--json"]), "id");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "자리 잃은 줄을 커밋한다"]);
    let dir = format!(".claude/worktrees/{there}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &format!("worktree-{there}")]);
    let side = main.join(&dir);
    ok_at(&main, LATER, &["mv", &lost, "todo"]);
    let out = ok(&side, &["show", &lost]);
    assert!(!out.contains("자리"), "낡은 스냅샷으로 자리를 댔다\n{out}");
    assert!(!ok(&side, &["show", &lost, "--json"]).contains("workplaces"));
    // 겹쳐 보면 main 의 칸이 들어와 아예 집은 줄이 아니다 — 그때도 "자리 없다" 는 안 선다.
    let out = ok(&side, &["show", &lost, "--worktree"]);
    assert!(!out.contains("자리"), "겹쳐 보고도 자리를 댔다\n{out}");
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
    // 하위 셸의 `cd` 는 뒤로 안 이어진다 — 뒷토막은 제 자리에 선다.
    let why = refusal(&bash(&a, &format!("(cd {bp} && moai status); moai add \"딴 일\"")));
    assert!(why.contains(&held), "묶음 밖 토막을 남의 트래커로 보냈다 — {why}");
    // 아직 없는 디렉터리는 실행할 때 생겨 `moai` 가 위로 찾아 이 트래커에 세운다 — 아무도 안 보면 샌다.
    for cmd in ["mkdir fresh && moai -C fresh add \"딴 일\"", "mkdir fresh && cd fresh && moai add \"딴 일\""] {
        let why = refusal(&bash(&a, cmd));
        assert!(why.contains(&held), "없는 디렉터리를 거쳐 규칙 1 을 넘었다 — {cmd}\n{why}");
    }

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

/// 규약대로 에픽 멤버를 main 에서 집고 그 이름의 워크트리를 띄운 저장소. (main, 워크트리, 에픽, 집은 id)
fn epic_member_in_a_worktree(s: &Scratch) -> (PathBuf, PathBuf, String, String) {
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let epic = field(&ok(&main, &["add", "저장 계층", "--type", "epic", "--json"]), "id");
    let id = field(&ok(&main, &["add", "워크트리에서 할 일", "-e", &epic, "--json"]), "id");
    ok(&main, &["mv", &id, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    let dir = format!(".claude/worktrees/{id}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &format!("worktree-{id}")]);
    let inside = main.join(&dir);
    (main, inside, epic, id)
}

/// **겹쳐 보고 풀린 거절은 비추는 줄이 곁들어도 풀린 것이다**(moai-dw63.e31). `Pass` 만 풀린
/// 것으로 치던 `settle` 은 `idea add` 하나를 곁들인 명령줄을 워크트리의 낡은 스냅샷으로 도로
/// 막았다 — 그 거절은 이미 main 에서 집은 일을 집으라고 시켰다.
#[test]
fn a_note_does_not_bring_back_a_deny_the_fresh_view_lifted() {
    let s = Scratch::new("hooknotewt");
    let (main, inside, epic, _) = epic_member_in_a_worktree(&s);
    let mp = main.display().to_string();
    let bash = |cmd: &str| tool_at(&s, &inside, "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));

    // main 에서 둘째 일을 세우고 집었다 — 워크트리 스냅샷은 모르지만 겹쳐 보고 푼다(moai-iaa4).
    let next = field(&ok(&main, &["add", "둘째 일", "-e", &epic, "--json"]), "id");
    ok(&main, &["mv", &next, "in_progress"]);
    let lifted = format!("moai -C {mp} add \"둘째의 자식\" --parent {next}");
    let out = bash(&lifted);
    assert!(out.trim().is_empty(), "main 에서 방금 집은 일의 자식을 막았다\n{out}");

    for cmd in [format!("{lifted} && moai -C {mp} idea add \"떠오른 것\""), format!("moai -C {mp} idea add \"떠오른 것\"; {lifted}")] {
        let out = bash(&cmd);
        one_json_value(&out);
        assert!(!out.contains("permissionDecision"), "비추는 줄이 곁들자 풀린 거절이 돌아왔다 — {cmd}\n{out}");
        assert!(out.contains(&format!("{epic} 가 내건 것")), "{out}");
    }
}

/// **옆이 쥐었을 일로는 비추지도 않는다**(moai-ntl6 의 자, moai-dw63.e31). 이름이 id 가 아닌
/// 워크트리가 갈라질 때 집혀 있던 일은 그 워크트리의 것일 수 있다 — 막지도 붙들지도 않기로 한 그
/// 줄의 에픽을 제 물음으로 비추면, main 세션을 남의 에픽에 세우는 길로 보낸다.
#[test]
fn work_an_unnamed_worktree_may_hold_is_not_offered_as_this_sessions_aim() {
    let s = Scratch::new("hooknoteunsure");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let theirs = field(&ok(&main, &["add", "옆의 에픽", "--type", "epic", "--json"]), "id");
    let agent = field(&ok(&main, &["add", "에이전트가 할 일", "-e", &theirs, "--json"]), "id");
    ok(&main, &["mv", &agent, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/agent-a04acfb3", "-b", "worktree-agent-a04acfb3"]);
    let bash = |cmd: &str| tool_at(&s, &main, "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));

    let out = bash("moai idea add \"관찰\"");
    assert!(out.trim().is_empty(), "옆 워크트리가 쥐었을 일의 에픽을 제 물음으로 비춘다\n{out}");

    // 갈라진 뒤 main 에서 집은 제 일은 비춘다 — 그것만 댄다.
    let mine = field(&ok(&main, &["add", "제 에픽", "--type", "epic", "--json"]), "id");
    let work = field(&ok(&main, &["add", "main 에서 집은 일", "-e", &mine, "--json"]), "id");
    ok(&main, &["mv", &work, "in_progress"]);
    let out = bash("moai idea add \"관찰\"");
    assert!(out.contains(&format!("{mine} 가 내건 것")), "제 일의 물음을 안 비춘다\n{out}");
    assert!(!out.contains(&theirs), "옆이 쥐었을 일의 에픽을 댄다\n{out}");
}

/// **`Stop` 은 에픽이 닫히는지를 main 까지 겹친 줄로 잰다**(moai-dw63.e31). 트래커는 main 에서
/// 쓰므로 워크트리의 스냅샷은 갈라진 때에 멈춰 있다 — 그 사이 main 에서 끝낸 멤버를 아직 벌여
/// 놓은 것으로 읽던 판은, 미루는 순간 에픽이 닫히는 마지막 멤버에 "지금 안 할 것이면" 을 그냥 댔다.
#[test]
fn the_stop_hook_measures_the_epic_on_what_main_has_closed() {
    let s = Scratch::new("hookstopfresh");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let epic = field(&ok(&main, &["add", "저장 계층", "--type", "epic", "--json"]), "id");
    let id = field(&ok(&main, &["add", "워크트리에서 할 일", "-e", &epic, "--json"]), "id");
    let other = field(&ok(&main, &["add", "옆에서 끝낼 일", "-e", &epic, "--json"]), "id");
    ok(&main, &["mv", &id, "in_progress"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "집는다"]);
    let dir = format!(".claude/worktrees/{id}");
    git(&main, &["worktree", "add", "-q", &dir, "-b", &format!("worktree-{id}")]);
    let inside = main.join(&dir);
    let stop = |session: &str| {
        let input = format!("{{\"session_id\":\"{session}\",\"cwd\":{}}}", json_str(&inside.display().to_string()));
        String::from_utf8(hook_in(&s, &inside, "stop", &input).stdout).unwrap()
    };

    // 남은 멤버가 첫 칸에 있으면 미뤄도 안 닫힌다 — 보통 줄이다.
    let held = stop("s1");
    assert!(held.contains("지금 안 할 것이면") && !held.contains("목적을 접을 때만"), "{held}");

    // 옆 멤버를 main 에서 끝냈다. 워크트리 스냅샷은 모르지만 미루면 에픽이 닫힌다.
    for to in ["in_progress", "done"] {
        let out = staged(&["mv", other.as_str(), to]).current_dir(&main).env("MOAI_NOW", LATER).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let held = stop("s2");
    assert!(
        held.contains(&format!("{epic} 의 목적을 접을 때만")),
        "main 에서 끝낸 멤버를 못 보고 마지막 멤버를 그냥 미루라고 한다\n{held}"
    );

    // **내미는 명령은 main 의 트래커를 겨눈다**(moai-gyqh) — 워크트리에서 맨 `moai` 로 치면 워크트리의
    // 스냅샷만 바뀌어 main 은 집은 채로 남고, 이 훅은 제 스냅샷을 보고 조용해진다.
    let root = std::fs::canonicalize(&main).unwrap().display().to_string();
    for line in [format!("moai -C {root} mv {id} todo -m"), format!("moai -C {root} defer {id}"), format!("moai -C {root} note {id}")] {
        assert!(held.contains(&line), "main 을 안 겨눈다 — {line}\n{held}");
    }
    assert!(!held.contains("  moai mv") && !held.contains("  moai defer"), "맨 moai 가 남았다\n{held}");
    // 시킨 대로 치면 main 이 놓는다.
    let out = staged(&["-C", &root, "mv", id.as_str(), "todo", "-m", "결정을 기다린다"]).current_dir(&inside).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(ok(&main, &["show", &id]).contains("todo"), "main 이 안 놓았다");
}

/// **main 이 모르는 줄은 main 으로 겨누지 않고, 낡은 칸으로 집으라는 줄은 main 에서 멈춘다**(리뷰
/// moai-ju21.70g). 워크트리에서 맨 `moai` 로 세운 줄을 겨누면 시킨 대로 친 줄이 "못 찾았다" 로 끝나고,
/// main 이 이미 닫은 줄을 낡은 스냅샷의 "집으라" 대로 겨누면 main 에서 도로 연다 — `--from` 이 막는다.
#[test]
fn the_hook_aims_at_main_only_what_main_knows_alike() {
    let s = Scratch::new("hookaimknown");
    let main = s.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    ok(&main, &["init", "argos"]);
    let shared = field(&ok(&main, &["add", "함께 아는 일", "--json"]), "id");
    let closed = field(&ok(&main, &["add", "main 이 닫을 일", "--json"]), "id");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "세운다"]);
    git(&main, &["worktree", "add", "-q", ".claude/worktrees/feat", "-b", "feat"]);
    let inside = main.join(".claude/worktrees/feat");
    for to in ["in_progress", "done"] {
        ok(&main, &["mv", &closed, to]);
    }
    let local = field(&ok(&inside, &["add", "워크트리에서 세운 일", "--json"]), "id");

    let body = format!("{{\"file_path\":{}}}", json_str(&inside.join("src/x.rs").display().to_string()));
    let why = refusal(&tool_at(&s, &inside, "Edit", &body));
    let root = std::fs::canonicalize(&main).unwrap().display().to_string();
    for id in [&shared, &closed] {
        assert!(why.contains(&format!("moai -C {root} mv {id} in_progress --from todo")), "main 이 아는 {id} 를 안 겨눈다\n{why}");
    }
    assert!(why.contains(&format!("\\n  moai mv {local} in_progress --from todo")), "main 이 모르는 줄을 겨눴다\n{why}");
    // 그 id 를 모르는 줄은 여전히 겨눈다 — 한 줄에 명령 하나다.
    assert!(why.contains(&format!("moai -C {root} add '제목'")), "{why}");
    // 낡은 칸으로 집으라는 줄은 main 에서 멈춘다 — main 이 닫은 일을 도로 열지 않는다.
    let out = staged(&["-C", &root, "mv", closed.as_str(), "in_progress", "--from", "todo"]).current_dir(&inside).output().unwrap();
    assert!(!out.status.success(), "낡은 칸으로 main 의 닫힌 일을 옮겼다");
    assert!(ok(&main, &["show", &closed]).contains("done"), "main 이 닫은 일을 도로 열었다");

    // main 에 트래커가 없으면 아무것도 안 겨눈다 — 그 가지에서 처음 `init` 한 트래커다.
    let bare = s.path().join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q"]);
    git(&bare, &["commit", "-q", "--allow-empty", "-m", "처음"]);
    git(&bare, &["worktree", "add", "-q", "../bare-feat", "-b", "feat"]);
    let branch = s.path().join("bare-feat");
    ok(&branch, &["init", "argos"]);
    let body = format!("{{\"file_path\":{}}}", json_str(&branch.join("src/x.rs").display().to_string()));
    let why = refusal(&tool_at(&s, &branch, "Edit", &body));
    assert!(!why.contains("moai -C"), "트래커 없는 main 을 겨눈다\n{why}");
}

/// **규칙 4 는 트래커 밖에서도 선다**(리뷰 moai-ju21.70g) — 사람의 tmux 서버는 트래커와 무관하다.
/// 트래커를 찾은 뒤에만 보던 판은 스크래치패드로 `cd` 해 둔 세션의 `tmux kill-server` 를 보냈다.
#[test]
fn the_tmux_rule_stands_outside_a_tracker() {
    let s = Scratch::new("hooktmuxbare");
    let bash = |cmd: &str| tool_at(&s, s.path(), "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));
    assert!(refusal(&bash("tmux kill-server")).starts_with("규칙 4"), "트래커 밖에서 사람의 서버를 겨눈 줄이 지나간다");
    assert_eq!(bash("env -u TMUX tmux -L t kill-server"), "", "제 서버를 가리킨 줄을 막는다");
    assert_eq!(bash("echo x > f"), "", "트래커 밖에서 트래커의 규칙을 세운다");
}

/// 다른 트래커를 가리키는 토막의 판정도 **같은 차례로 잇는다**(`Decision::then`, moai-dw63.e31) —
/// 남이 막으면 제 비춤이 그것을 가리지 않고, 둘 다 비추면 둘 다 싣는다.
#[test]
fn notes_and_refusals_from_two_trackers_are_joined_in_one_order() {
    let a = init("hooknoteA");
    let b = init("hooknoteB");
    let ea = field(&ok(a.path(), &["add", "A 의 에픽", "--type", "epic", "--json"]), "id");
    let wa = field(&ok(a.path(), &["add", "A 의 일", "-e", &ea, "--json"]), "id");
    ok(a.path(), &["mv", &wa, "in_progress"]);
    let eb = field(&ok(b.path(), &["add", "B 의 에픽", "--type", "epic", "--json"]), "id");
    let wb = field(&ok(b.path(), &["add", "B 의 일", "-e", &eb, "--json"]), "id");
    ok(b.path(), &["mv", &wb, "in_progress"]);
    let bash = |cmd: &str| tool_at(&a, a.path(), "Bash", &format!("{{\"command\":{}}}", json_str(cmd)));
    let bp = b.path().display().to_string();

    // 제 자리의 물음이 남의 트래커의 규칙 1 을 가리지 않는다.
    let why = refusal(&bash(&format!("moai idea add \"a\"; moai -C {bp} add \"딴 일\"")));
    assert!(why.contains(&wb), "{why}");

    // 둘 다 비추면 둘 다 싣는다 — 뒤의 물음을 말없이 버리지 않는다.
    let out = bash(&format!("moai idea add \"a\"; moai -C {bp} idea add \"b\""));
    one_json_value(&out);
    assert!(!out.contains("permissionDecision"), "{out}");
    assert!(out.contains(&format!("{ea} 가 내건 것")) && out.contains(&format!("{eb} 가 내건 것")), "{out}");
    assert!(out.contains(&format!("moai -C {bp} idea promote <그 id> -e {eb}")), "남의 트래커에 되찾을 자리를 안 댄다\n{out}");
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

    // 설정 파일을 가리키는 둘은 **걷은 뒤 제 값으로 박는다**(`isolated`) — 돌리는 사람의 `~/.gitconfig`
    // 를 안 읽게 하려면 비우는 것만으로는 모자라 `/dev/null` 을 대야 한다. 그것은 아래에서 따로 본다.
    let pinned = ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"];
    for var in
        ["MOAI_ACTOR", "MOAI_NOW", "CLAUDE_CONFIG_DIR", "CLAUDE_CODE_PLUGIN_CACHE_DIR", "XDG_CONFIG_HOME", "BASH_ENV"]
            .iter()
            .chain(git_leaks::swept(true))
            .filter(|var| !pinned.contains(&&***var))
    {
        assert!(removed(var), "{var} 를 안 걷었다");
    }
    assert_eq!(set("GIT_CONFIG_GLOBAL"), Some(std::ffi::OsStr::new("/dev/null")));
    assert_eq!(set("GIT_CONFIG_SYSTEM"), Some(std::ffi::OsStr::new("/dev/null")));
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

/// 함께 까는 두 플러그인 — `guide::KOREAN_PLUGINS` 와 같아야 한다.
const COMPANIONS: [(&str, &str); 2] =
    [("korean-skills@korean-skills", "DaleSeo/korean-skills"), ("humanize-korean@im-not-ai", "epoko77-ai/im-not-ai")];

/// **한국어 글쓰기 플러그인 둘을 moai 와 같은 범위로 함께 깐다**(moai-lr1s). 사용자 전역에 깔지 않는다는
/// 결정(moai-5wk4)이 서는 자리다 — `--scope` 를 안 따르면 `local` 로 부른 사람의 전역 설정이 바뀐다.
/// 연습은 아무것도 부르지 않고 부를 것을 댄다.
#[test]
fn skill_install_brings_the_korean_plugins_at_the_same_scope() {
    let s = init("skillkorean");
    let c = Claude::new("skillkorean-home");
    let before = c.calls();
    let plan = text(&c.run(s.path(), &["skill", "install", "--scope", "project", "--dry-run"], true));
    assert_eq!(c.calls(), before, "연습인데 claude 를 불렀다");
    for (id, repo) in COMPANIONS {
        assert!(plan.contains(&format!("claude plugin marketplace add {repo} --scope project")), "{plan}");
        assert!(plan.contains(&format!("claude plugin install {id} --scope project -y")), "{plan}");
    }

    let out = c.run(s.path(), &["skill", "install", "--scope", "project"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before.len()..].to_string();
    for (id, repo) in COMPANIONS {
        assert!(calls.contains(&format!("plugin marketplace add {repo} --scope project")), "{calls}");
        assert!(calls.contains(&format!("plugin install {id} --scope project -y")), "{calls}");
    }
    assert!(!calls.contains("--scope user"), "사용자 전역을 건드렸다\n{calls}");
    assert!(text(&out).contains("korean-skills@korean-skills 을 함께 깔았다"), "{}", text(&out));
}

/// **같은 이름의 마켓플레이스가 다른 저장소를 가리키면 건너뛴다.** 덮으면 남의 등록이 이쪽으로 돌아선다.
/// **같은 출처를 같은 철자로 알면 다시 더한다**(리뷰 moai-5wk4.76z) — `claude` 는 그것을 받아 `--scope` 의
/// 설정에 적는다. 안 더하던 판은 `--scope project` 의 커밋되는 설정에 마켓플레이스를 안 적었다.
#[test]
fn skill_install_skips_a_korean_marketplace_that_points_elsewhere() {
    let s = init("skillkoreanclash");
    let c = Claude::new("skillkoreanclash-home");
    c.ledger(
        "known_marketplaces.json",
        r#"{"korean-skills":{"source":{"source":"github","repo":"someone/else"}},"im-not-ai":{"source":{"source":"github","repo":"epoko77-ai/im-not-ai"}}}"#,
    );
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "install", "--json"], true);
    assert!(out.status.success(), "{}", text(&out));
    let json = String::from_utf8(out.stdout).unwrap();
    assert!(json.contains("이미 someone/else 를 가리킨다"), "{json}");
    let calls = c.calls()[before..].to_string();
    assert!(!calls.contains("korean-skills"), "남의 이름을 건드렸다\n{calls}");
    assert!(calls.contains("plugin marketplace add epoko77-ai/im-not-ai --scope local"), "아는 출처를 이 범위에 안 적는다\n{calls}");
    assert!(calls.contains("plugin install humanize-korean@im-not-ai --scope local -y"), "{calls}");
}

/// **같은 저장소를 다른 철자로 알면 남의 것으로 안 본다**(리뷰 moai-5wk4.76z). upstream 안내대로
/// `daleseo/korean-skills` 로 더했거나 주소(`https://github.com/…`)로 더한 사람에게 "이미 다른 곳" 이라며 영영
/// 건너뛰던 자리다. 철자가 다르면 `claude` 가 다시 받아 덮으니 더하지는 않고 설치만 부른다.
#[test]
fn skill_install_reads_another_spelling_of_the_same_korean_marketplace() {
    let s = init("skillkoreanspell");
    let c = Claude::new("skillkoreanspell-home");
    c.ledger(
        "known_marketplaces.json",
        r#"{"korean-skills":{"source":{"source":"github","repo":"daleseo/korean-skills"}},"im-not-ai":{"source":{"source":"git","url":"https://github.com/epoko77-ai/im-not-ai.git"}}}"#,
    );
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "install"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before..].to_string();
    for (id, _) in COMPANIONS {
        assert!(calls.contains(&format!("plugin install {id} --scope local -y")), "{calls}");
    }
    assert!(!calls.contains("marketplace add DaleSeo") && !calls.contains("marketplace add epoko77-ai"), "다른 철자를 덮는다\n{calls}");
    assert!(!text(&out).contains("건너뛰었다"), "{}", text(&out));
}

/// **moai 를 등록하지 못했으면 곁의 것도 안 깐다**(리뷰 moai-5wk4.76z). 여기서만 깔면 moai 없이 곁의 것만
/// 남고, `uninstall` 은 moai 의 설치로 범위를 재니 그것을 걷을 길도 없다. 칠 줄은 손으로 친다.
#[test]
fn skill_install_keeps_the_korean_plugins_out_when_moai_is_not_registered() {
    let s = init("skillkoreannomoai");
    let c = Claude::failing_on("skillkoreannomoai-home", Some("moai@"));
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "install"], true);
    assert!(!out.status.success(), "등록을 못 했는데 성공으로 끝났다\n{}", text(&out));
    let calls = c.calls()[before..].to_string();
    assert!(!calls.contains("korean-skills") && !calls.contains("im-not-ai"), "moai 없이 곁의 것을 깔았다\n{calls}");
    assert!(
        text(&out).contains("korean-skills@korean-skills 을 못 깔았다 — 손으로: claude plugin marketplace add DaleSeo/korean-skills"),
        "{}",
        text(&out)
    );
}

/// **걷을 때는 moai 를 걷는 범위에서만 함께 걷고, 마켓플레이스는 둔다** — 이름이 기계 하나에서 전역이라
/// 다른 저장소의 설치가 그것을 쓰고 있을 수 있다. 다른 저장소에 깔린 줄은 안 건드린다.
#[test]
fn skill_uninstall_takes_the_korean_plugins_along() {
    let s = init("skillkoreanrm");
    let c = Claude::new("skillkoreanrm-home");
    let (market, dir) = installed(&s, &c, "0.0.1");
    let root = dir.parent().unwrap().parent().unwrap();
    let moai = format!(
        "{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"/x\",\"version\":\"0.0.1\"}}",
        root.display()
    );
    let here = format!("{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"/y\",\"version\":\"1\"}}", root.display());
    let there = r#"{"scope":"local","projectPath":"/elsewhere","installPath":"/z","version":"1"}"#;
    c.ledger(
        "installed_plugins.json",
        &format!(
            "{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{moai}],\"korean-skills@korean-skills\":[{here}],\"humanize-korean@im-not-ai\":[{there}]}}}}"
        ),
    );
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before..].to_string();
    assert!(calls.contains("plugin uninstall korean-skills@korean-skills --scope local"), "{calls}");
    assert!(!calls.contains("humanize-korean@im-not-ai"), "다른 저장소의 설치를 걷었다\n{calls}");
    assert!(!calls.contains("marketplace remove korean-skills"), "함께 쓰는 마켓플레이스를 지웠다\n{calls}");
}

/// **함께 깐 것을 못 걷어도 moai 의 걷기는 걷힌 것이다**(리뷰 moai-5wk4.76z) — `install` 과 같은 셈이다.
/// 하나가 실패해도 다음 것을 부르고, 못 걷은 것은 손으로 칠 줄로 낸다. moai 의 걸음에 섞여 있던 판은 moai 를
/// 다 걷고도 비영으로 끝났고, 다시 부르면 moai 의 설치가 없어 "걷어낼 것이 없다" 며 곁의 것을 남겼다.
#[test]
fn skill_uninstall_counts_the_korean_plugins_apart() {
    let s = init("skillkoreanrmfail");
    let c = Claude::failing_on("skillkoreanrmfail-home", Some("plugin uninstall korean-skills"));
    let (market, dir) = installed(&s, &c, "0.0.1");
    let root = dir.parent().unwrap().parent().unwrap();
    let row = |v: &str| format!("{{\"scope\":\"local\",\"projectPath\":\"{}\",\"installPath\":\"/x\",\"version\":\"{v}\"}}", root.display());
    c.ledger(
        "installed_plugins.json",
        &format!(
            "{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{}],\"korean-skills@korean-skills\":[{}],\"humanize-korean@im-not-ai\":[{}]}}}}",
            row("0.0.1"),
            row("1"),
            row("1")
        ),
    );
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(out.status.success(), "moai 를 다 걷었는데 비영이다\n{}", text(&out));
    let calls = c.calls()[before..].to_string();
    assert!(calls.contains("plugin uninstall humanize-korean@im-not-ai --scope local"), "실패 뒤의 것을 안 불렀다\n{calls}");
    let said = text(&out);
    assert!(said.starts_with(&format!("`{market}` 을 걷었다")), "{said}");
    assert!(said.contains("! claude plugin uninstall korean-skills@korean-skills --scope local  — 실패"), "{said}");
}

/// **사용자 범위의 곁의 것은 다른 저장소의 moai 도 쓰면 두고 간다**(리뷰 moai-5wk4.76z). 그 줄은 기계에
/// 하나라, 한 저장소의 걷기가 다른 저장소의 두 플러그인까지 지웠다. 걷는 줄은 손으로 칠 수 있게 댄다.
#[test]
fn skill_uninstall_leaves_user_scope_korean_plugins_another_moai_uses() {
    let s = init("skillkoreanrmuser");
    let c = Claude::new("skillkoreanrmuser-home");
    let (market, _) = installed(&s, &c, "0.0.1");
    let user = |v: &str| format!("{{\"scope\":\"user\",\"installPath\":\"/x\",\"version\":\"{v}\"}}");
    let ledger = |other: bool| {
        format!(
            "{{\"version\":2,\"plugins\":{{\"moai@{market}\":[{}]{},\"korean-skills@korean-skills\":[{}],\"humanize-korean@im-not-ai\":[{}]}}}}",
            user("0.0.1"),
            if other { format!(",\"moai@moai-other-1234\":[{}]", user("9")) } else { String::new() },
            user("1"),
            user("1")
        )
    };
    c.ledger("installed_plugins.json", &ledger(true));
    let before = c.calls().len();
    let out = c.run(s.path(), &["skill", "uninstall"], true);
    assert!(out.status.success(), "{}", text(&out));
    let calls = c.calls()[before..].to_string();
    assert!(calls.contains(&format!("plugin uninstall moai@{market} --scope user")), "{calls}");
    assert!(!calls.contains("korean-skills@korean-skills") && !calls.contains("humanize-korean@im-not-ai"), "다른 저장소가 쓰는 것을 걷었다\n{calls}");
    assert!(text(&out).contains("claude plugin uninstall korean-skills@korean-skills --scope user"), "{}", text(&out));

    // 이 저장소의 moai 만 사용자 범위에 서 있으면 함께 걷는다(사용자 결정 moai-5wk4 — 같은 범위로 깔고 걷는다).
    let (_, _) = installed(&s, &c, "0.0.1");
    c.ledger("installed_plugins.json", &ledger(false));
    let before = c.calls().len();
    assert!(c.run(s.path(), &["skill", "uninstall"], true).status.success());
    let calls = c.calls()[before..].to_string();
    assert!(calls.contains("plugin uninstall korean-skills@korean-skills --scope user"), "{calls}");
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
    git_run(dir, None, args)
}

/// 커밋 시각까지 고정해 돌린다. **차례가 답을 가르는 시험은 기계 시계에 매이면 안 된다** —
/// 커밋 칸은 `git log` 의 차례(커밋 시각) 그대로 서므로(`git::table`), 일부만 기계 시계로
/// 찍으면 고정한 커밋들과의 앞뒤가 돌리는 기계마다 달라진다.
fn git_at(dir: &Path, at: &str, args: &[&str]) -> String {
    git_run(dir, Some(at), args)
}

fn git_run(dir: &Path, at: Option<&str>, args: &[&str]) -> String {
    // `isolated` 로 띄운다 — git 훅 안에서 시험이 돌 때 물려받은 `GIT_DIR` 이 남으면
    // 여기서의 `git commit` 이 바깥 저장소에 떨어진다.
    let mut cmd = isolated("git");
    cmd.args(["-c", "user.name=테스터", "-c", "user.email=tester@example.com", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir);
    if let Some(at) = at {
        cmd.env("GIT_AUTHOR_DATE", at).env("GIT_COMMITTER_DATE", at);
    }
    let out = cmd.output().expect("git 을 실행하지 못했다");
    assert!(out.status.success(), "git {args:?} 가 실패했다\n{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// 시계를 달리 두고 돌린다 — 옆 워크트리의 쓰기가 **더 늦게** 떨어진 것을 흉내 낸다.
fn ok_at(dir: &Path, now: &str, args: &[&str]) -> String {
    let out = staged(args)
        .current_dir(dir)
        .env("MOAI_NOW", now)
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
    // 커밋도 **그 워크트리의 가지**에서 읽는다 — 일을 고친 커밋은 저쪽에만 있다(moai-emcv).
    git_at(&t.s.path().join("feat"), LATER, &["commit", "-q", "--allow-empty", "-m", &format!("feat: 옆에서 고친다 ({})", t.picked)]);
    let one = ok(&main, &["show", &t.picked, "--worktree"]);
    assert!(one.contains("feat: 옆에서 고친다"), "옆 가지의 커밋을 이쪽 HEAD 에서 찾았다\n{one}");
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
    // 울타리 밑에는 "저장소가 아닌 자리" 가 없다(moai-boc6) — 못 찾았다는 말은 그 밖에서만 잰다.
    if !scratch::fenced_base() {
        assert!(String::from_utf8_lossy(&out.stderr).contains("워크트리를 못 찾았다"));
        let board = String::from_utf8_lossy(&out.stdout);
        assert!(!board.contains("드러난 문제 없다") && board.contains("옆 워크트리 문제 1건"), "{board}");
    }
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

    for args in [
        &["project", "add", "a", "--json"][..],
        &["project", "rm", "a", "--json"],
        // 목록의 모양이 틀렸는데 "등록돼 있지 않다"(not_found) 로 새면 그 말이 시키는 `add` 가 거절된다
        // (moai-gmdu 에픽 리뷰) — 색도 목록을 고치는 쓰기라 같은 거절이다.
        &["project", "color", "a", "green", "--json"],
    ] {
        let out = project(home.path(), &config, args);
        assert!(!out.status.success(), "{args:?} 가 깨진 설정에 썼다");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(r#""code":"broken""#), "{args:?}\n{}", text(&out));
        // 손으로 고치라는 말에는 **어느 파일인지** 붙는다 — 설정의 자리는 환경이 골라 사람이 모를 수 있다.
        assert!(err.contains("config.toml"), "{args:?}\n{}", text(&out));
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

/// `examples/bash-agent/agent.sh` 를 **실제로 돌린다** (moai-0j1y). 예제는 계약 시험을
/// 겸한다 — `ready --json` 의 모양(`{"ready":[…],"held":[…]}`)이나 `mv`·`note` 의 인자가
/// 바뀌면 예제가 조용히 썩는 대신 여기서 떨어진다. bash·jq 가 없는 기계에서는 건너뛰지
/// 않고 실패한다 — 건너뛰는 시험은 아무도 안 보는 사이 예제를 썩힌다(사람이 정했다).
#[cfg(unix)]
fn agent(dir: &Path, work: &Path) -> Output {
    agent_cmd(dir, work, Path::new(BIN))
        .output()
        .expect("bash 를 실행하지 못했다 — 예제 시험에는 bash 와 jq 가 있어야 한다")
}

/// 예제를 띄울 명령. `moai` 자리에 대역을 끼우거나 환경을 더 줄 시험이 이것을 쓴다
/// (`Claude::command` 와 같은 모양이다).
#[cfg(unix)]
fn agent_cmd(dir: &Path, work: &Path, moai: &Path) -> Command {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/bash-agent/agent.sh");
    let mut cmd = isolated("bash");
    cmd.arg(script)
        .current_dir(dir)
        .env("MOAI", moai)
        .env("AGENT_WORK", work)
        .env("MOAI_ACTOR", ACTOR)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1");
    cmd
}

/// 실행할 파일을 `dst` 에 내놓는다. 몸은 곁의 `.src` 에 쓰고 **[`place_exe`] 가 옮긴다** —
/// 이 프로세스가 쓴 inode 를 예제의 bash 가 exec 하면 옆 시험의 fork 와 겹쳐 ETXTBSY 다.
#[cfg(unix)]
fn write_exe(dst: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let source = dst.with_extension("src");
    std::fs::write(&source, body).unwrap();
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();
    place_exe(&source, dst);
}

/// 일 명령 대역. 받은 id 를 기록에 적고 `body` 를 돈다 — 그 출력이 노트가 된다.
#[cfg(unix)]
fn work_script(s: &Scratch, body: &str) -> PathBuf {
    let path = s.path().join("work.sh");
    let log = s.path().join("worked");
    write_exe(&path, &format!("#!/bin/sh\necho \"$1\" >> '{}'\n{body}\n", log.display()));
    path
}

/// 일 명령이 받은 id 들. **한 번도 안 불렸으면 빈 목록이다** — 없는 파일로 터지면 예제가
/// 왜 일찍 멈췄는지(jq 가 없다, 일 명령을 못 돌린다)를 가린 채 파일 이름만 남는다.
#[cfg(unix)]
fn worked(s: &Scratch) -> Vec<String> {
    std::fs::read_to_string(s.path().join("worked")).unwrap_or_default().lines().map(str::to_string).collect()
}

#[cfg(unix)]
#[test]
fn the_bash_agent_example_works_the_ready_queue_until_it_is_empty() {
    let s = init("agent-loop");
    let later = add(s.path(), &["나중 일", "-p", "2"]);
    let first = add(s.path(), &["급한 일", "-p", "1"]);
    let parked = add(s.path(), &["미룬 일", "-p", "0"]);
    ok(s.path(), &["defer", &parked, "-m", "지금 아님"]);
    // 미뤄 둔 것에 막힌 줄은 `held` 로 선다 — 큐가 비면 그 수를 댄다. 이것이 없으면 `held`
    // 가 늘 비어, 키 이름이 바뀌어도(`jq '.held | length'` 는 없는 키에 0 을 낸다) 아무도 안 잡는다.
    let waiting = add(s.path(), &["막힌 일", "-p", "0"]);
    ok(s.path(), &["link", &parked, "--blocks", &waiting]);
    let work = work_script(&s, "echo \"$1 을 끝냈다: $2\"");

    let out = agent(s.path(), &work);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "예제가 실패했다\n{}", text(&out));

    assert_eq!(worked(&s), [first.as_str(), later.as_str()], "ready 차례대로 집지 않았다\n{}", text(&out));
    for (id, title) in [(&first, "급한 일"), (&later, "나중 일")] {
        assert!(line_of(s.path(), id).contains("\"status\":\"done\""), "{id} 가 done 이 아니다");
        let shown = ok(s.path(), &["show", id]);
        assert!(shown.contains(&format!("{id} 을 끝냈다: {title}")), "일 명령의 출력이 노트로 안 남았다\n{shown}");
    }
    for id in [&parked, &waiting] {
        assert!(line_of(s.path(), id).contains("\"status\":\"todo\""), "{id} 를 집었다");
    }
    assert!(stdout.contains("집을 일이 없다"), "{stdout}");
    assert!(stdout.contains("막힌 일 1건"), "막혀 못 집는 일의 수를 안 댔다\n{stdout}");
}

#[cfg(unix)]
#[test]
fn the_bash_agent_example_stops_on_a_failed_job_and_leaves_it_picked() {
    let s = init("agent-fail");
    let id = add(s.path(), &["깨지는 일", "-p", "1"]);
    let other = add(s.path(), &["다음 일", "-p", "3"]);
    let work = work_script(&s, "echo 컴파일이 깨졌다; exit 3");

    let out = agent(s.path(), &work);
    // **1 인지까지 본다.** 아무 비영 종료나 받으면 일 명령을 한 번도 못 돌린 채 멈춘 것
    // (jq 가 없다·집기 전에 거절했다)도 "일이 실패했다" 로 읽힌다.
    assert_eq!(out.status.code(), Some(1), "일이 실패하면 1 로 멈춘다\n{}", text(&out));
    assert_eq!(worked(&s), [id.as_str()], "실패한 뒤에도 돌았다\n{}", text(&out));
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""), "실패한 일을 내려놓았다");
    let shown = ok(s.path(), &["show", &id]);
    assert!(shown.contains("실패(3): 컴파일이 깨졌다"), "실패 코드와 출력이 노트로 안 남았다\n{shown}");
    assert!(line_of(s.path(), &other).contains("\"status\":\"todo\""));
}

/// 공백뿐인 출력도, UTF-8 이 아닌 출력도 끝낸 일이다. `note` 는 빈 메모를 거절하고
/// (`trim` 은 유니코드 공백을 턴다) UTF-8 이 아닌 stdin 도 통째로 거절하므로, 그대로 넘기면
/// 끝낸 일이 in_progress 로 남은 채 루프가 실패로 멈춘다.
///
/// **bash 의 `[:space:]` 로 가르지 않는다** — NBSP 는 로캘에 따라 글자로 세어, `note` 가
/// 빈 메모로 거절하는 것을 스크립트는 글이라고 읽는다. 가르는 자를 `note` 와 맞춘다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_closes_a_job_whose_output_is_blank_or_not_utf8() {
    for (name, body, noted) in [
        ("agent-blank", "printf '  \\n'", "(출력 없음)"),
        ("agent-nbsp", "printf '\\302\\240\\302\\240\\n'", "(출력 없음)"),
        ("agent-latin1", "printf 'caf\\351 끝\\n'", "caf\u{fffd} 끝"),
        // `note` 의 64KB 상한(moai-m9a8)을 넘는 출력 — 끝만 적고 그렇다고 밝힌 채 닫는다.
        ("agent-long", "head -c 100000 /dev/zero | tr '\\0' 'x'; printf '\\n마지막 줄\\n'", "끝 16000 자만 적는다"),
    ] {
        let s = init(name);
        let id = add(s.path(), &["말없는 일", "-p", "1"]);
        let work = work_script(&s, body);

        let out = agent(s.path(), &work);
        assert!(out.status.success(), "{name}: 끝낸 일에 멈췄다\n{}", text(&out));
        assert!(line_of(s.path(), &id).contains("\"status\":\"done\""), "{name}: 닫지 않았다");
        let shown = ok(s.path(), &["show", &id]);
        assert!(shown.contains(noted), "{name}: 노트가 {noted:?} 가 아니다\n{shown}");
    }
}

/// `ready` 와 `mv` 사이에 옆 에이전트가 같은 줄을 먼저 집으면 `mv` 는 `already` 로 0 을
/// 낸다. 종료 코드만 믿으면 둘이 같은 일을 한다 — 예제는 `moved` 를 보고 넘어가야 한다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_skips_a_job_a_neighbour_claimed_first() {
    let s = init("agent-race");
    let contested = add(s.path(), &["다툰 일", "-p", "1"]);
    let other = add(s.path(), &["남은 일", "-p", "2"]);
    // 이 에이전트의 `mv <contested> in_progress` 바로 앞에 옆 에이전트가 끼어든다.
    let wrapper = s.path().join("moai-race.sh");
    write_exe(
        &wrapper,
        &format!(
            "#!/bin/sh\nif [ \"$1\" = mv ] && [ \"$2\" = {contested} ] && [ \"$3\" = in_progress ]; then '{BIN}' mv {contested} in_progress >/dev/null; fi\nexec '{BIN}' \"$@\"\n"
        ),
    );
    let work = work_script(&s, "echo 했다");

    let out = agent_cmd(s.path(), &work, &wrapper).output().unwrap();
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(worked(&s), [other.as_str()], "남이 집은 일을 또 했다\n{}", text(&out));
    assert!(line_of(s.path(), &contested).contains("\"status\":\"in_progress\""), "남이 집은 일을 닫았다");
}

/// **못 돌린 것을 "남이 집었다" 로 읽지 않는다.** 0 아닌 코드는 겨루다 진 것에도,
/// 신원 없음·락 걸림·칸 오타·깨진 줄에도 똑같이 붙는다. 실패를 진 것으로 읽으면 같은
/// 줄이 다시 나와 `seen` 이 "또 집을 일로 나왔다" 며 엉뚱한 까닭을 댄다 — 가르는 것은
/// stdout 이다: 진 것은 줄을 내고, 실패는 stderr 만 내며 stdout 이 빈다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_stops_when_a_claim_fails_instead_of_losing() {
    let s = init("agent-claimfail");
    let id = add(s.path(), &["집히면 안 되는 일"]);
    // 집기만 실패하게 한다 — moai 가 락을 못 잡거나 신원을 모를 때의 모양이다.
    let wrapper = s.path().join("moai-locked.sh");
    write_exe(
        &wrapper,
        &format!("#!/bin/sh\nif [ \"$1\" = mv ]; then echo '{{\"error\":\"잠겨 있다\",\"code\":\"locked\"}}' >&2; exit 1; fi\nexec '{BIN}' \"$@\"\n"),
    );
    let work = work_script(&s, "echo 했다");

    let out = agent_cmd(s.path(), &work, &wrapper).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(worked(&s).is_empty(), "못 집은 일을 했다\n{}", text(&out));
    assert!(!text(&out).contains("또 집을 일로 나왔다"), "실패를 겨루기로 읽어 엉뚱한 까닭을 댔다\n{}", text(&out));
    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""), "안 집힌 줄이 움직였다");
}

/// **`--from` 이 맞았다고 집은 것은 아니다.** 첫 칸이 곧 집는 칸인 설정에서는 칸이 맞고도
/// 아무것도 안 옮기고 `already` 로 0 을 낸다 — `moved` 를 안 보면 여럿이 같은 일을 한다.
/// 예제의 옛 판이 `.moved | length` 로 지키던 자리고, 그 말이 그 주석에 적혀 있었다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_does_not_mistake_already_for_a_claim() {
    let s = init("agent-already");
    std::fs::write(s.path().join(".moai/config.toml"), "prefix = \"agent-already\"\nstatuses = \"in_progress,review,done\"\n").unwrap();
    add(s.path(), &["첫 칸이 곧 집는 칸인 일"]);
    let work = work_script(&s, "echo 했다");

    let out = agent(s.path(), &work);
    assert!(worked(&s).is_empty(), "집지도 않은 일을 했다\n{}", text(&out));
    assert_eq!(out.status.code(), Some(1), "집은 것이 없는데 조용히 끝났다\n{}", text(&out));
}

/// **채비가 안 됐으면 아무것도 집지 않는다.** 집은 뒤에야 알면 맨 위 일이 in_progress 에
/// 박히고, 다시 부를 때마다 그다음 일이 또 박힌다 — 되돌리는 말은 사람 몫이다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_claims_nothing_when_it_cannot_set_itself_up() {
    let s = init("agent-nowork");
    let id = add(s.path(), &["건드리면 안 되는 일", "-p", "1"]);
    let work = work_script(&s, "echo 했다");

    // 일 명령을 못 돌린다.
    let out = agent(s.path(), &s.path().join("없는-일.sh"));
    assert_eq!(out.status.code(), Some(2), "못 도는 일 명령을 집기 전에 안 걸렀다\n{}", text(&out));
    assert!(text(&out).contains("실행할 수 없다"), "{}", text(&out));

    // 일의 출력을 받아 둘 자리가 없다.
    let out = agent_cmd(s.path(), &work, Path::new(BIN)).env("TMPDIR", s.path().join("없는-자리")).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "쓸 수 없는 TMPDIR 을 집기 전에 안 걸렀다\n{}", text(&out));

    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""), "채비가 안 됐는데 일을 집었다");
    assert!(worked(&s).is_empty(), "일이 돌았다");
}

/// **일이 스스로 정한 것을 덮지 않는다.** 일이 `defer` 하거나 `review` 로 옮기고 0 으로 끝나면
/// 그 판단이 남아야 한다 — 안 하기로 한 것을 `done` 으로 옮기지 않는다(AGENTS.md). 첫 칸으로
/// 되돌린 줄이 `ready` 에 다시 서면 같은 일을 끝없이 도는 대신 멈춘다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_keeps_what_the_work_command_decided() {
    let s = init("agent-decides");
    let epic = ok(s.path(), &["epic", "add", "접을 에픽", "-q"]).trim().to_string();
    // **묶음을 미룬 것도 센다.** 멤버의 `deferred_at` 은 비어 있고 `shelved_by` 만 그 에픽을 댄다.
    let member = add(s.path(), &["에픽 멤버", "-p", "0", "-e", &epic]);
    let shelved = add(s.path(), &["미룰 일", "-p", "1"]);
    let reviewed = add(s.path(), &["리뷰로 갈 일", "-p", "2"]);
    let returned = add(s.path(), &["되돌릴 일", "-p", "3"]);
    let work = work_script(
        &s,
        &format!(
            "case \"$2\" in\n  에픽*) \"$MOAI\" defer {epic} -m 접자 >/dev/null ;;\n  \
             미룰*) \"$MOAI\" defer \"$1\" -m 지금-아님 >/dev/null ;;\n  \
             리뷰*) \"$MOAI\" mv \"$1\" review >/dev/null ;;\n  \
             되돌릴*) \"$MOAI\" mv \"$1\" todo >/dev/null ;;\nesac\necho 했다"
        ),
    );

    let out = agent(s.path(), &work);
    assert_eq!(out.status.code(), Some(1), "되돌린 줄이 또 나왔는데 안 멈췄다\n{}", text(&out));
    assert_eq!(
        worked(&s),
        [member.as_str(), shelved.as_str(), reviewed.as_str(), returned.as_str()],
        "차례대로 한 번씩 돌지 않았다\n{}",
        text(&out)
    );
    let line = line_of(s.path(), &shelved);
    assert!(line.contains("\"status\":\"in_progress\"") && line.contains("\"deferred_at\""), "미룬 일을 닫았다 — {line}");
    assert!(line_of(s.path(), &member).contains("\"status\":\"in_progress\""), "미룬 에픽의 멤버를 닫았다");
    assert!(line_of(s.path(), &reviewed).contains("\"status\":\"review\""), "리뷰로 보낸 일을 닫았다");
    assert!(line_of(s.path(), &returned).contains("\"status\":\"todo\""), "되돌린 일을 닫았다");
}

/// **`.moai` 밖에서는 멈춘다.** 거기서 `ready --json` 은 등록한 프로젝트마다의 한눈 보기
/// (`{"projects":…}`)를 내는데, 그걸 빈 큐로 읽으면 할 일이 쌓여 있는데 "집을 일이 없다" 로
/// 조용히 0 을 낸다. 이 시험이 없으면 그 검사를 지워도 나머지 넷이 다 푸르다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_refuses_to_run_outside_a_repo() {
    let s = init("agent-outside");
    let id = add(s.path(), &["등록한 곳의 일", "-p", "1"]);
    let home = Scratch::new("agent-outside-home");
    let config = registry(&home, &[s.path()]);
    let outside = dir_in(&home, "밖");
    let work = work_script(&s, "echo 했다");
    // **정말 밖인지부터 본다.** 임시 디렉터리가 어느 `.moai` 밑이면(TMPDIR 을 저장소 안에 둔
    // 기계) 루프가 그 저장소의 진짜 일을 집어 닫는다 — 그때는 집기 전에 여기서 떨어져야 한다.
    let probe = moai(&outside, &["ready", "--json"]);
    assert!(!probe.status.success(), "여기는 어느 저장소 안이다 — 예제를 돌리면 그 저장소를 건드린다\n{}", text(&probe));

    let out = agent_cmd(&outside, &work, Path::new(BIN)).env("MOAI_CONFIG", &config).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "밖에서 2 로 안 멈췄다\n{}", text(&out));
    assert!(text(&out).contains("안에서 돌린다"), "{}", text(&out));

    // 등록한 프로젝트가 없으면 `ready` 가 한눈 보기 대신 아예 거절한다(비영 종료). 흔한 쪽은
    // 이쪽이다 — 그때도 같은 말과 같은 코드로 멈춰야 "일이 실패했다"(1)로 읽히지 않는다.
    let bare = agent_cmd(&outside, &work, Path::new(BIN)).output().unwrap();
    assert_eq!(bare.status.code(), Some(2), "등록한 것이 없는 밖에서 2 로 안 멈췄다\n{}", text(&bare));
    assert!(text(&bare).contains("안에서 돌린다"), "{}", text(&bare));

    assert!(worked(&s).is_empty(), "밖에서 일을 했다");
    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""), "밖에서 등록한 프로젝트의 일을 집었다");
}

/// **일이 띄워 두고 간 자식이 루프를 잡지 않는다.** 출력을 파이프로 받으면 `$(…)` 는 그 파이프를
/// 물려받은 자식이 죽을 때까지 기다린다 — dev 서버를 띄우고 0 으로 끝난 일이 노트도 없이 멈춘다.
#[cfg(unix)]
#[test]
fn the_bash_agent_example_does_not_wait_for_a_child_the_job_left_running() {
    let s = init("agent-daemon");
    let id = add(s.path(), &["서버 띄우는 일", "-p", "1"]);
    let work = work_script(&s, "sleep 30 &\necho \"띄웠다: $1\"");

    let started = std::time::Instant::now();
    let out = agent(s.path(), &work);
    assert!(out.status.success(), "{}", text(&out));
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "띄워 둔 자식을 기다렸다");
    assert!(line_of(s.path(), &id).contains("\"status\":\"done\""), "닫지 않았다\n{}", text(&out));
    assert!(ok(s.path(), &["show", &id]).contains("띄웠다"), "노트가 안 남았다");
}

/// **집기는 본 칸이 그대로일 때만 먹는다** (moai-f8q1). `ready` 와 `mv` 사이에 옆
/// 에이전트가 같은 줄을 집고 닫기까지 하면, 뒤늦은 `mv` 가 `done` 을 `in_progress` 로
/// 되연다. `--from <칸>` 을 적으면 락 안에서 다시 보고 칸이 달라졌으면 안 옮긴다.
#[test]
fn a_move_from_a_column_only_lands_while_the_row_is_still_there() {
    let s = init("mv-from");
    let id = add(s.path(), &["겨루는 일"]);
    ok(s.path(), &["mv", &id, "in_progress", "--from", "todo"]);
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""));

    // 뒤늦은 집기. 칸이 이미 달라졌으니 아무것도 안 옮기고 **부분 실패**로 끝난다.
    let out = moai(s.path(), &["mv", &id, "review", "--from", "todo"]);
    assert!(!out.status.success(), "진 집기가 성공으로 끝났다\n{}", text(&out));
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(said.contains(&id) && said.contains("in_progress"), "지금 칸을 말하지 않는다\n{said}");
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""), "진 집기가 칸을 덮었다");
    assert!(!journal(s.path()).contains("\"to\":\"review\""), "안 옮긴 것을 저널에 적었다");
}

/// 진 줄은 `--json` 에도 선다 — 사람 출력에만 있으면 받는 쪽이 두 표면 중 하나를
/// 못 믿는다. **지금 칸을 함께 준다**: 그것이 없으면 진 쪽이 한 번 더 물어야 한다.
#[test]
fn a_lost_claim_stands_in_the_json_with_the_column_it_found() {
    let s = init("mv-from-json");
    let id = add(s.path(), &["겨루는 일"]);
    ok(s.path(), &["mv", &id, "done"]);
    let out = moai(s.path(), &["mv", &id, "in_progress", "--from", "todo", "--json"]);
    assert!(!out.status.success(), "진 집기가 성공으로 끝났다");
    let json = String::from_utf8(out.stdout).unwrap();
    one_json_value(&json);
    assert!(json.contains("\"stale\":[{"), "진 줄 목록이 없다\n{json}");
    assert!(json.contains(&format!("\"id\":\"{id}\"")) && json.contains("\"status\":\"done\""), "{json}");
    assert!(json.contains("\"moved\":[]"), "{json}");
}

/// 안 적으면 지금과 똑같다 — 막지 않는다. 승인 게이트를 만들지 않는 것이 이 도구의
/// 밑동이라, 칸 검사는 **부르는 쪽이 골라 켜는 것**이다.
#[test]
fn a_move_without_from_still_never_refuses() {
    let s = init("mv-from-off");
    let id = add(s.path(), &["되감는 일"]);
    ok(s.path(), &["mv", &id, "done"]);
    ok(s.path(), &["mv", &id, "todo"]);
    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""));
}

/// 모르는 칸은 옮길 칸과 같은 자로 거절한다 — 오타가 "아무것도 안 옮겼다" 로
/// 조용히 넘어가면 에이전트는 영영 아무 일도 못 집는다. **`defer` 도 같은 자다** —
/// 한쪽만 시험하면 두 벌로 적힌 검사가 갈릴 때 한쪽만 잡힌다.
#[test]
fn a_from_column_that_does_not_exist_is_refused() {
    let s = init("mv-from-bad");
    let id = add(s.path(), &["일"]);
    for args in [vec!["mv", &id, "done", "--from", "없는칸"], vec!["defer", &id, "--from", "없는칸"]] {
        let out = moai(s.path(), &args);
        assert!(!out.status.success(), "{args:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("없는칸"), "{args:?}\n{}", text(&out));
    }
    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""), "거절하면서 옮겼다");
    assert!(!line_of(s.path(), &id).contains("\"deferred_at\""), "거절하면서 미뤘다");
}

/// **config 가 더는 모르는 칸도 줄이 거기 있으면 받는다** (moai-hym7). 칸 이름을 바꾸면
/// 옛 이름에 선 줄이 남는데, 그 이름을 오타로 보고 거절하면 그 줄은 **영영** `--from`
/// 으로 못 집는다 — 남의 낡은 줄 하나가 쓰기를 막는 자리다(CLAUDE.md). 검사가 노리는
/// 것은 오타지 낡음이 아니므로, 아무 줄도 서 있지 않은 이름만 거절한다.
#[test]
fn a_from_column_the_config_forgot_still_works_while_a_row_sits_there() {
    let s = init("mv-from-renamed");
    let id = add(s.path(), &["옛 칸에 선 일"]);
    let other = add(s.path(), &["옛 칸에 선 둘째"]);
    ok(s.path(), &["mv", &id, &other, "in_progress"]);
    let cfg = s.path().join(".moai/config.toml");
    let renamed = std::fs::read_to_string(&cfg).unwrap().replace("in_progress", "doing");
    std::fs::write(&cfg, renamed).unwrap();

    // 줄은 아직 `in_progress` 에 서 있다 — 설정만 그 이름을 잊었다.
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""));
    ok(s.path(), &["mv", &id, "done", "--from", "in_progress"]);
    assert!(line_of(s.path(), &id).contains("\"status\":\"done\""), "옛 칸에 선 줄을 못 집었다");

    // **미루기도 같은 자다.** 미룬 줄은 옛 칸에 그대로 서 있으므로, 쓰기 검사가 칸
    // 이름을 다시 물으면 `--from` 이 통과시킨 줄을 쓰기가 거절한다 — 검사 둘이 서로
    // 반대를 말하면 부르는 쪽은 어디를 고칠지 못 고른다(moai-hym7.xvc 가 짚었다).
    ok(s.path(), &["defer", &other, "-m", "다음에", "--from", "in_progress"]);
    assert!(line_of(s.path(), &other).contains("\"deferred_at\""), "옛 칸에 선 줄을 못 미뤘다");
    ok(s.path(), &["defer", &other, "--undo", "--from", "in_progress"]);
    assert!(!line_of(s.path(), &other).contains("\"deferred_at\""), "옛 칸에 선 줄을 못 도로 집었다");

    // **제목 고치기와 막기도 같은 자다.** `store::with_write` 만 풀고 `edit`·`link` 가
    // 제 손으로 칸 이름을 다시 물으면 옛 칸에 선 줄은 제목 하나 못 고치고 막음도 못
    // 푼다 — 도구 안에서 영영 못 만지는 줄이 되어, 푼 것이 헛일이 된다. 탐색기는
    // `with_write` 만 지나므로 여기서 갈리면 두 표면이 서로 다른 말을 한다.
    ok(s.path(), &["edit", &other, "--title", "고친 제목"]);
    assert!(line_of(s.path(), &other).contains("고친 제목"), "옛 칸에 선 줄의 제목을 못 고쳤다");
    assert!(line_of(s.path(), &other).contains("\"status\":\"in_progress\""), "제목만 고쳤는데 칸이 움직였다");
    ok(s.path(), &["link", &id, "--blocks", &other]);
    assert!(line_of(s.path(), &other).contains("\"blocked_by\""), "옛 칸에 선 줄을 못 막았다");
    ok(s.path(), &["link", &id, "--unblocks", &other]);

    // **읽기가 쓰기보다 엄하면 안 된다** (moai-lvf9.t10). 옮길 수는 있는데 못 찾는 줄이
    // 생기면, 칸 이름을 바꾼 뒤 정리하려는 사람이 그 줄에 닿을 길이 없다.
    let listed = ok(s.path(), &["show", "-s", "in_progress"]);
    assert!(listed.contains(&other), "옛 칸에 선 줄을 못 찾는다\n{listed}");

    // 칸을 **옮기는** 쓰기는 그대로 엄하다 — 갈 칸이 아는 칸이어야 한다.
    let out = moai(s.path(), &["mv", &other, "doing"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(!moai(s.path(), &["mv", &other, "in_progress"]).status.success(), "모르는 칸으로 옮겼다");

    // 아무 줄도 안 선 이름은 그대로 거절한다 — 오타 검사는 살아 있다.
    let out = moai(s.path(), &["mv", &id, "todo", "--from", "in_progress"]);
    assert!(!out.status.success(), "이제 아무도 안 선 옛 칸이 통과했다\n{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("in_progress"), "{}", text(&out));
    assert!(line_of(s.path(), &id).contains("\"status\":\"done\""), "거절하면서 옮겼다");
}

/// **칸 오타는 사람을 못 찾는 것보다 먼저 선다** — `mv` 와 `defer` 가 한 자다.
/// 뒤로 밀리면 신원 없는 기계(CI·훅)에서 오타가 "누가 하는지 모른다" 로 덮여, 부르는
/// 쪽이 받는 `code` 가 `bad_status` 가 아니라 `no_actor` 가 된다 — 고칠 곳이 설정인지
/// 명령인지가 갈리는 자리다. 한쪽만 시험하면 두 벌로 적힌 차례가 갈릴 때 한쪽만 잡힌다.
///
/// **`bad_status` 를 내는 검사는 하나도 빠짐없이 먼저 선다.** 오타만 앞세우고 묶음
/// 가드를 사람 뒤에 두면, 같은 자의 잘못이 `--from` 의 값에 따라 두 `code` 로 갈린다 —
/// 오타는 `bad_status`, 묶음은 `no_actor`. 부르는 쪽은 그것을 가를 방법이 없다.
#[test]
fn a_bad_from_column_is_named_before_the_missing_person() {
    let s = init("from-before-who");
    let id = add(s.path(), &["일"]);
    let epic = add(s.path(), &["묶음", "--type", "epic"]);
    add(s.path(), &["멤버", "-e", &epic]);
    let cases = [
        vec!["mv", &id, "done", "--from", "없는칸"],
        vec!["defer", &id, "--from", "없는칸"],
        vec!["mv", &epic, "done", "--from", "todo"],
        vec!["defer", &epic, "--from", "todo"],
    ];
    for args in cases {
        let out = without_user(s.path(), &args);
        assert!(!out.status.success(), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("칸"), "{args:?} 가 칸 대신 사람을 말했다\n{err}");
        assert!(!err.contains("--user"), "{args:?} 가 사람을 먼저 물었다\n{err}");
    }
}

/// **같은 id 를 두 번 적어도 제가 방금 쓴 값과 겨루지 않는다.** `--from` 이 재는 것은
/// 부르는 쪽이 본 칸이지 이 명령이 만든 칸이 아니다 — 돌면서 그때그때 보던 판은 한 줄을
/// `moved` 와 `stale` 에 함께 세우고 0 아닌 코드를 냈다. 그 코드를 "못 집었다" 로 읽는
/// 것이 `--from` 의 계약이라, 집어 놓고 버리는 일이 된다.
#[test]
fn a_repeated_id_does_not_race_the_write_this_command_just_made() {
    let s = init("mv-from-twice");
    let id = add(s.path(), &["되풀이"]);
    let out = moai(s.path(), &["mv", &id, &id, "in_progress", "--from", "todo", "--json"]);
    assert!(out.status.success(), "제가 쓴 값과 겨뤘다\n{}", text(&out));
    let json = String::from_utf8(out.stdout).unwrap();
    one_json_value(&json);
    assert!(json.contains("\"stale\":[]"), "{json}");
    assert!(json.contains("\"already\":[\"") && json.contains("\"moved\":[{"), "{json}");
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""));
}

/// **둘이 같은 줄을 집으면 하나만 이긴다.** 조용한 손실이 이 도구가 못 견디는 유일한
/// 실패 모드고, 집기가 그것을 두 에이전트에게 열어 주면 같은 일을 둘이 한다.
#[test]
fn only_one_racer_claims_a_row() {
    let s = init("mv-race");
    let id = add(s.path(), &["하나뿐인 일"]);
    let kids: Vec<_> = (0..8)
        .map(|_| {
            staged_live(&["mv", &id, "in_progress", "--from", "todo"])
                .current_dir(s.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let won = kids.into_iter().map(|k| k.wait_with_output().unwrap()).filter(|o| o.status.success()).count();
    assert_eq!(won, 1, "집기를 이긴 것이 하나가 아니다");
    assert_eq!(journal(s.path()).matches("\"to\":\"in_progress\"").count(), 1, "옮긴 줄이 저널에 여럿이다");
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""));
}

/// **미루기도 본 칸이 그대로일 때만 먹는다** (moai-o5ss). 옆에서 집어 일하기
/// 시작한 줄을 뒤늦은 `defer` 가 계획 밖으로 빼면, 일하던 쪽은 제 일이 보드에서
/// 사라진 까닭을 어디서도 못 읽는다. `--undo` 도 같은 자로 잰다.
#[test]
fn a_defer_from_a_column_only_lands_while_the_row_is_still_there() {
    let s = init("defer-from");
    let id = add(s.path(), &["겨루는 일"]);
    ok(s.path(), &["mv", &id, "in_progress"]);

    let out = moai(s.path(), &["defer", &id, "-m", "나중에", "--from", "todo"]);
    assert!(!out.status.success(), "진 미루기가 성공으로 끝났다\n{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("in_progress"), "지금 칸을 말하지 않는다\n{}", text(&out));
    assert!(!line_of(s.path(), &id).contains("\"deferred_at\""), "진 미루기가 계획에서 뺐다");
    assert!(!journal(s.path()).contains("나중에"), "안 미룬 줄에 까닭을 적었다");

    // 본 칸이 맞으면 그대로 먹는다. 되돌리기도 같은 자를 받는다.
    ok(s.path(), &["defer", &id, "--from", "in_progress", "-m", "나중에"]);
    assert!(line_of(s.path(), &id).contains("\"deferred_at\""));
    let lost = moai(s.path(), &["defer", &id, "--undo", "--from", "todo", "--json"]);
    assert!(!lost.status.success(), "진 되돌리기가 성공으로 끝났다");
    let json = String::from_utf8(lost.stdout).unwrap();
    one_json_value(&json);
    assert!(json.contains("\"stale\":[{") && json.contains("\"status\":\"in_progress\""), "{json}");
    assert!(line_of(s.path(), &id).contains("\"deferred_at\""), "진 되돌리기가 도로 집었다");
    ok(s.path(), &["defer", &id, "--undo", "--from", "in_progress"]);
    assert!(!line_of(s.path(), &id).contains("\"deferred_at\""));
}

/// **묶음에는 `--from` 이 없다** (사람이 정했다, moai-8xwi.rzg). 묶음의 칸은 멤버에서
/// 읽고 쓰기는 줄에 적힌 칸(미루기면 `deferred_at`)에 한다 — 재는 축과 쓰는 축이 갈려
/// 있어, 겨루는 둘이 같은 `--from` 을 걸면 **둘 다 이긴다.** 먹는 척하는 가드는 안
/// 쓰느니만 못하므로 아예 거절하고, 무엇을 대신 하면 되는지 함께 낸다.
#[test]
fn a_group_cannot_be_claimed_with_from() {
    let s = init("from-group");
    let epic = ok(s.path(), &["epic", "add", "묶음", "-q"]).trim().to_string();
    let member = add(s.path(), &["멤버", "-e", &epic]);
    ok(s.path(), &["mv", &member, "in_progress"]);
    // 줄에 적힌 칸은 아직 첫 칸이고 읽은 칸만 움직였다 — 어느 칸을 줘도 마찬가지다.
    assert!(line_of(s.path(), &epic).contains("\"status\":\"todo\""), "{}", line_of(s.path(), &epic));

    for args in [
        vec!["mv", &epic, "done", "--from", "in_progress"],
        vec!["mv", &epic, "done", "--from", "todo"],
        vec!["defer", &epic, "-m", "접는다", "--from", "in_progress"],
    ] {
        let out = moai(s.path(), &args);
        assert!(!out.status.success(), "묶음에 --from 이 먹었다: {args:?}\n{}", text(&out));
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(said.contains("멤버에서 읽는다") && said.contains(&epic), "까닭을 안 댄다\n{said}");
    }
    // **말만 보고 끝내지 않는다.** 거절하면서 쓰는 것이 여기서 제일 나쁜 일이다.
    assert!(line_of(s.path(), &epic).contains("\"status\":\"todo\""), "거절하면서 옮겼다");
    assert!(!line_of(s.path(), &epic).contains("\"deferred_at\""), "거절하면서 미뤘다");
    assert!(!journal(s.path()).contains("\"to\":\"done\""), "안 옮긴 것을 저널에 적었다");

    // `--from` 없이는 그대로 된다 — 묶음을 접는 길(AGENTS.md)이 막히지 않는다.
    ok(s.path(), &["defer", &epic, "-m", "접는다"]);
    assert!(line_of(s.path(), &epic).contains("\"deferred_at\""));
}

/// `examples/python-agents/agents.py` 를 **실제로 돌린다** (moai-pz8m). bash 판과 같이 예제가
/// 계약 시험을 겸한다 — `ready --json --worktree`·`mv --from`·`note -b -`·`show --json` 의 모양이
/// 바뀌면 예제가 조용히 썩는 대신 여기서 떨어진다. python3·git 이 없으면 건너뛰지 않고 실패한다.
#[cfg(unix)]
fn agents_cmd(dir: &Path, work: &Path, n: usize) -> Command {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/python-agents/agents.py");
    let mut cmd = isolated("python3");
    // **예제가 읽는 나머지 둘도 걷는다.** 돌리는 사람의 셸에 예제를 쓰다 남은 `AGENT_BASE`·
    // `AGENT_WORKTREES` 가 새면 가지가 없다며 2 로 끝나거나 워크트리가 `.worktrees/` 밖에 선다.
    // 제 값을 줄 시험은 이 뒤에 `.env` 로 덮는다.
    cmd.arg(script)
        .args(["--agents", &n.to_string()])
        .current_dir(dir)
        .env_remove("AGENT_BASE")
        .env_remove("AGENT_WORKTREES")
        .env("MOAI", BIN)
        .env("AGENT_WORK", work)
        .env("MOAI_ACTOR", ACTOR)
        .env("MOAI_NOW", NOW)
        .env("NO_COLOR", "1");
    cmd
}

/// 예제를 돌릴 저장소 — git 저장소여야 워크트리를 띄운다.
///
/// **`.moai` 를 첫 커밋에 싣는다.** 실제 저장소가 그렇고, 그래야 워크트리에도 트래커가 선다 —
/// 안 실으면 워크트리에 `.moai` 가 아예 없어 "워크트리의 트래커는 안 바뀐다" 는 단언이 헛돈다
/// (리뷰 moai-pz8m.4zl 이 짚고, 단언을 조이자 실제로 빨개졌다).
#[cfg(unix)]
fn agents_repo(name: &str) -> Scratch {
    let s = Scratch::new(name);
    git(s.path(), &["init", "-q"]);
    ok(s.path(), &["init", "argos"]);
    git(s.path(), &["add", "-A"]);
    git(s.path(), &["commit", "-q", "-m", "처음"]);
    s
}

/// **일꾼 셋이 겨뤄도 한 일은 한 번만 한다** — 모든 일이 done 으로 끝나고, 일 명령은 id 마다
/// 한 번씩 불리며, 저마다 제 워크트리(`agent/<id>` 가지) 안에서 돈다. 트래커는 뿌리에만 쓰인다.
#[cfg(unix)]
#[test]
fn the_python_agents_share_the_queue_and_each_job_runs_once_in_its_own_worktree() {
    let s = agents_repo("agents-crew");
    let ids: Vec<String> = (1..=5).map(|n| add(s.path(), &[&format!("일 {n}"), "-p", "2"])).collect();
    // **일감을 main 에 싣는다** — 워크트리의 트래커에도 그 줄이 있어야, 예제가 거기에 잘못
    // 쓰는 날 그 쓰기가 실제로 먹고 아래 단언이 그것을 본다. 안 실으면 그 줄이 없어 쓰기가
    // 먼저 거절당해, 단언은 늘 지나간다.
    git(s.path(), &["add", "-A"]);
    git(s.path(), &["commit", "-q", "-m", "일감"]);
    // 일 명령 대역: 받은 id 와 **돈 자리**를 적는다 — 워크트리 안에서 돌았는지를 거기서 잰다.
    let log = s.path().join("worked");
    let work = s.path().join("work.sh");
    write_exe(&work, &format!("#!/bin/sh\necho \"$1 $(pwd)\" >> '{}'\necho \"$1 을 끝냈다\"\n", log.display()));

    // **일 명령은 상대 경로로 준다** — 문서의 모양(`AGENT_WORK=./do-one.sh`)이다. 그 스크립트는 뿌리에만
    // 있고(커밋 안 됐다) 일은 워크트리를 cwd 로 돌므로, 예제가 뿌리 기준으로 박지 않으면 못 띄운다.
    let out = agents_cmd(s.path(), Path::new("./work.sh"), 3)
        .output()
        .expect("python3 를 실행하지 못했다 — 예제 시험에는 python3 가 있어야 한다");
    assert!(out.status.success(), "예제가 실패했다\n{}", text(&out));

    let worked: Vec<(String, String)> = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(|l| {
            let (id, at) = l.split_once(' ').unwrap();
            (id.to_string(), at.to_string())
        })
        .collect();
    let mut seen: Vec<&str> = worked.iter().map(|(id, _)| id.as_str()).collect();
    seen.sort_unstable();
    let mut want: Vec<&str> = ids.iter().map(String::as_str).collect();
    want.sort_unstable();
    assert_eq!(seen, want, "일마다 한 번씩 돌지 않았다 — 둘이 같은 일을 했거나 빠뜨렸다\n{}", text(&out));

    for (id, at) in &worked {
        assert!(at.ends_with(&format!(".worktrees/{id}")), "{id} 가 제 워크트리 밖에서 돌았다 — {at}");
        assert!(line_of(s.path(), id).contains("\"status\":\"done\""), "{id} 가 done 이 아니다");
        assert!(ok(s.path(), &["show", id]).contains(&format!("{id} 을 끝냈다")), "{id} 의 출력이 노트로 안 남았다");
    }
    let branches = git(s.path(), &["branch", "--list", "agent/*"]);
    for id in &ids {
        assert!(branches.contains(&format!("agent/{id}")), "{id} 의 가지가 없다\n{branches}");
        // **트래커는 뿌리에만 쓴다** — 워크트리의 `.moai` 는 main 에서 뜬 그대로다.
        let theirs = std::fs::read_to_string(s.path().join(".worktrees").join(id).join(".moai/issues.jsonl")).unwrap_or_else(|e| panic!("{id} 의 워크트리 트래커가 없다 — 없으면 이 단언은 아무것도 안 잰다: {e}"));
        assert!(!theirs.contains("in_progress") && !theirs.contains("\"done\""), "{id} 의 워크트리 트래커가 바뀌었다");
    }
    // 병합은 안 한다 — main 은 처음 그대로다.
    assert_eq!(git(s.path(), &["rev-list", "--count", "main"]).trim(), "2", "예제가 main 에 무언가를 합쳤다");
}

/// **일이 실패하면 그 일꾼만 멈추고 끝에 1 이다.** 실패한 일은 in_progress 로 남고 노트에 까닭이
/// 선다. 옆 일꾼은 남은 일을 마저 한다 — 한 일의 실패가 큐 전체를 세우지 않는다.
#[cfg(unix)]
#[test]
fn a_failed_python_agent_job_stays_picked_and_the_others_carry_on() {
    let s = agents_repo("agents-fail");
    let broken = add(s.path(), &["깨지는 일", "-p", "0"]);
    let rest: Vec<String> = (1..=3).map(|n| add(s.path(), &[&format!("멀쩡한 일 {n}"), "-p", "2"])).collect();
    let work = s.path().join("work.sh");
    write_exe(&work, &format!("#!/bin/sh\nif [ \"$1\" = '{broken}' ]; then echo 컴파일이 깨졌다; exit 3; fi\necho 됐다\n"));

    let out = agents_cmd(s.path(), &work, 2).output().expect("python3 를 실행하지 못했다");
    assert_eq!(out.status.code(), Some(1), "일이 실패하면 1 로 끝난다\n{}", text(&out));
    assert!(line_of(s.path(), &broken).contains("\"status\":\"in_progress\""), "실패한 일을 내려놓았다");
    assert!(ok(s.path(), &["show", &broken]).contains("실패(3): 컴파일이 깨졌다"), "실패 코드와 출력이 노트로 안 남았다");
    for id in &rest {
        assert!(line_of(s.path(), id).contains("\"status\":\"done\""), "{id} 를 옆 일꾼이 마저 안 했다\n{}", text(&out));
    }
}

/// **채비가 안 되면 아무것도 안 집는다**(종료 코드 2) — 일 명령이 없거나, 워크트리를 띄울 가지가
/// 없으면 저장소를 건드리기 전에 멈춘다. 집은 뒤에 드러나면 맨 위 일이 in_progress 에 박힌다.
#[cfg(unix)]
#[test]
fn the_python_agents_check_their_kit_before_picking_anything() {
    let s = agents_repo("agents-kit");
    let id = add(s.path(), &["일", "-p", "1"]);
    let missing = s.path().join("없는-일.sh");
    let out = agents_cmd(s.path(), &missing, 2).output().expect("python3 를 실행하지 못했다");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    // 까닭까지 본다 — 다른 채비(가지·moai)에 걸려 2 가 나도 이 단언은 지나가면 안 된다.
    assert!(String::from_utf8_lossy(&out.stderr).contains("없는-일.sh"), "{}", text(&out));
    let work = s.path().join("work.sh");
    write_exe(&work, "#!/bin/sh\necho 됐다\n");
    let out = agents_cmd(s.path(), &work, 2).env("AGENT_BASE", "없는-가지").output().expect("python3 를 실행하지 못했다");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("없는-가지"), "{}", text(&out));
    assert!(line_of(s.path(), &id).contains("\"status\":\"todo\""), "채비가 안 됐는데 집었다");
}

/// **일이 정한 것을 덮지 않고, 되돌린 일은 한 번만 한다.** 일은 워크트리를 cwd 로 도므로 트래커는
/// `"$MOAI" -C "$AGENT_ROOT"` 로 만진다 — 예제가 그 둘을 넘겨야 일의 `review`·`defer` 가 뿌리에
/// 떨어지고 닫기가 그것을 본다. 일이 첫 칸으로 되돌린 줄은 **옆 일꾼도** 다시 집지 않는다 —
/// 일꾼마다의 `seen` 만으로는 그 줄을 처음 보는 옆 일꾼이 같은 일을 또 했다.
#[cfg(unix)]
#[test]
fn the_python_agents_keep_what_the_work_decided_and_do_a_returned_job_only_once() {
    let s = agents_repo("agents-decide");
    // 맨 위 일이 오래 걸려야, 그것을 쥔 일꾼이 옆이 되돌린 줄을 **처음 보는 채로** 다음 판에 온다.
    let slow = add(s.path(), &["오래 걸리는 일", "-p", "0"]);
    let returned = add(s.path(), &["되돌릴 일", "-p", "1"]);
    let reviewed = add(s.path(), &["리뷰로 갈 일", "-p", "2"]);
    let shelved = add(s.path(), &["미룰 일", "-p", "3"]);
    // 워크트리의 트래커에도 그 줄이 있어야, 일이 맨 `moai` 로 거기에 쓰는 날 그 쓰기가 먹는다 —
    // 그러면 뿌리는 in_progress 로 남아 닫기가 done 으로 덮고, 아래 단언이 그것을 잡는다.
    git(s.path(), &["add", "-A"]);
    git(s.path(), &["commit", "-q", "-m", "일감"]);
    let log = s.path().join("worked");
    let work = s.path().join("work.sh");
    write_exe(
        &work,
        &format!(
            "#!/bin/sh\necho \"$1\" >> '{}'\ncase \"$2\" in\n  \
             오래*) sleep 1 ;;\n  \
             되돌릴*) \"$MOAI\" -C \"$AGENT_ROOT\" mv \"$1\" todo >/dev/null ;;\n  \
             리뷰*) \"$MOAI\" -C \"$AGENT_ROOT\" mv \"$1\" review >/dev/null ;;\n  \
             미룰*) \"$MOAI\" -C \"$AGENT_ROOT\" defer \"$1\" -m 지금-아님 >/dev/null ;;\nesac\necho 했다\n",
            log.display()
        ),
    );

    let out = agents_cmd(s.path(), &work, 2).output().expect("python3 를 실행하지 못했다");
    assert_eq!(out.status.code(), Some(1), "되돌린 줄이 또 나왔는데 1 로 안 끝났다\n{}", text(&out));
    let mut worked: Vec<String> = std::fs::read_to_string(&log).unwrap_or_default().lines().map(str::to_string).collect();
    worked.sort_unstable();
    let mut want = vec![slow.clone(), returned.clone(), reviewed.clone(), shelved.clone()];
    want.sort_unstable();
    assert_eq!(worked, want, "일마다 한 번씩 돌지 않았다 — 되돌린 일을 옆 일꾼이 또 했다\n{}", text(&out));
    assert!(line_of(s.path(), &slow).contains("\"status\":\"done\""), "{}", text(&out));
    assert!(line_of(s.path(), &returned).contains("\"status\":\"todo\""), "되돌린 일을 닫았다\n{}", text(&out));
    assert!(line_of(s.path(), &reviewed).contains("\"status\":\"review\""), "리뷰로 보낸 일을 닫았다\n{}", text(&out));
    let line = line_of(s.path(), &shelved);
    assert!(line.contains("\"status\":\"in_progress\"") && line.contains("\"deferred_at\""), "미룬 일을 닫았다 — {line}");
}

/// **옆 워크트리에만 있는 줄은 건너뛴다.** `ready --worktree` 는 그 줄도 내지만 뿌리에는 없어 집기가
/// `missing` 으로 돌아온다 — 그것을 "졌다" 로 읽으면 다음 판에 또 나와 "사람이 볼 차례다" 로 1 을
/// 낸다. 뿌리의 일은 다 하고 0 으로 끝나야 한다.
#[cfg(unix)]
#[test]
fn the_python_agents_skip_a_row_only_a_side_worktree_has() {
    let s = agents_repo("agents-side");
    let mine = add(s.path(), &["뿌리의 일", "-p", "1"]);
    git(s.path(), &["add", "-A"]);
    git(s.path(), &["commit", "-q", "-m", "일감"]);
    let side = s.path().join("side");
    git(s.path(), &["worktree", "add", "-q", "-b", "side", side.to_str().unwrap(), "main"]);
    // 맨 위에 선다 — 뿌리의 일보다 먼저 겨루고 먼저 `missing` 을 받는다.
    let theirs = add(&side, &["옆에만 있는 일", "-p", "0"]);
    let work = s.path().join("work.sh");
    write_exe(&work, "#!/bin/sh\necho 됐다\n");

    let out = agents_cmd(s.path(), &work, 2).output().expect("python3 를 실행하지 못했다");
    assert!(out.status.success(), "옆에만 있는 줄 때문에 실패로 끝났다\n{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("{theirs}  건너뛴다")), "{}", text(&out));
    assert!(line_of(s.path(), &mine).contains("\"status\":\"done\""), "{}", text(&out));
    assert!(line_of(&side, &theirs).contains("\"status\":\"todo\""), "옆의 줄을 건드렸다");
}

/// **`--from` 이 맞았다고 집은 것은 아니다** — bash 판의 같은 시험을 옮긴 것이다(moai-8ksq). 첫 칸이
/// 곧 집는 칸인 설정에서는 `mv` 가 아무것도 안 옮기고 `already` 로 0 을 낸다. 종료 코드만 믿거나
/// `already` 를 못 가르면 일꾼 둘이 같은 일을 하거나 엉뚱한 까닭으로 멈춘다 — 그래서 까닭까지 본다.
#[cfg(unix)]
#[test]
fn the_python_agents_do_not_mistake_already_for_a_claim() {
    let s = agents_repo("agents-already");
    std::fs::write(s.path().join(".moai/config.toml"), "prefix = \"argos\"\nstatuses = \"in_progress,review,done\"\n").unwrap();
    let id = add(s.path(), &["첫 칸이 곧 집는 칸인 일"]);
    let work = work_script(&s, "echo 했다");

    let out = agents_cmd(s.path(), &work, 2).output().expect("python3 를 실행하지 못했다");
    assert!(worked(&s).is_empty(), "집지도 않은 일을 했다\n{}", text(&out));
    assert_eq!(out.status.code(), Some(1), "집은 것이 없는데 조용히 끝났다\n{}", text(&out));
    assert!(text(&out).contains(&format!("{id}  이미 in_progress 다")), "already 를 가르지 못했다\n{}", text(&out));
}

/// **노트를 못 남긴 일은 닫지 않는다.** 닫으면 일의 출력이 아무 데도 없이 done 이 된다 — 일꾼은
/// 그 일을 in_progress 로 두고 1 로 멈춘다. 노트만 실패하게 moai 를 감싼다(락·신원의 모양이다).
#[cfg(unix)]
#[test]
fn a_python_agent_job_whose_note_fails_stays_picked() {
    let s = agents_repo("agents-nonote");
    let id = add(s.path(), &["노트 못 남길 일", "-p", "1"]);
    // 예제는 늘 `moai -C <뿌리> …` 로 부르므로 명령 이름은 셋째 자리다.
    let wrapper = s.path().join("moai-nonote.sh");
    write_exe(
        &wrapper,
        &format!("#!/bin/sh\nif [ \"$3\" = note ]; then echo '잠겨 있다' >&2; exit 1; fi\nexec '{BIN}' \"$@\"\n"),
    );
    let work = work_script(&s, "echo 했다");

    let out = agents_cmd(s.path(), &work, 2).env("MOAI", &wrapper).output().expect("python3 를 실행하지 못했다");
    assert_eq!(out.status.code(), Some(1), "노트를 못 남겼는데 1 로 안 끝났다\n{}", text(&out));
    assert_eq!(worked(&s), [id.as_str()], "일이 한 번 돌지 않았다\n{}", text(&out));
    assert!(line_of(s.path(), &id).contains("\"status\":\"in_progress\""), "노트 없이 닫았다\n{}", text(&out));
    assert!(String::from_utf8_lossy(&out.stderr).contains("노트를 못 남겼다: 잠겨 있다"), "까닭을 안 댔다\n{}", text(&out));
}

/// **일이 띄워 두고 간 자식이 일꾼을 잡지 않는다** — 출력을 파이프가 아니라 파일로 받는 까닭이다.
/// 파이프로 받으면 그것을 물려받은 자식이 죽을 때까지 기다려, dev 서버를 띄우고 0 으로 끝난 일이
/// 노트도 없이 멈춘다. bash 판의 같은 시험을 옮긴 것이다.
#[cfg(unix)]
#[test]
fn the_python_agents_do_not_wait_for_a_child_the_job_left_running() {
    let s = agents_repo("agents-daemon");
    let id = add(s.path(), &["서버 띄우는 일", "-p", "1"]);
    let work = work_script(&s, "sleep 30 &\necho \"띄웠다: $1\"");

    let started = std::time::Instant::now();
    let out = agents_cmd(s.path(), &work, 2).output().expect("python3 를 실행하지 못했다");
    assert!(out.status.success(), "{}", text(&out));
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "띄워 둔 자식을 기다렸다");
    assert!(line_of(s.path(), &id).contains("\"status\":\"done\""), "닫지 않았다\n{}", text(&out));
    assert!(ok(s.path(), &["show", &id]).contains(&format!("띄웠다: {id}")), "노트가 안 남았다");
}
