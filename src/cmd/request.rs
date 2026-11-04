//! 읽기 명령 하나를 **요청으로** 돌리는 입구(moai-5n6m.m0b) — 오래 사는 프로세스(데몬)가 부른다.
//!
//! CLI 는 argv 를 풀고, `-C` 면 프로세스의 현재 디렉터리를 옮기고, 명령을 돌린 뒤 곁에 남은 것(알림·부분 실패)을
//! 프로세스 전역에서 한 번 읽고 끝난다. 데몬은 한 프로세스에서 여러 프로젝트·여러 요청을, 스레드로 함께 돌리기도
//! 하므로 그 셋을 다 못 쓴다. 여기는 그 셋을 요청의 것으로 바꿔 **같은 명령 함수**를 부른다 —
//!
//! - 자리는 프로세스의 현재 디렉터리가 아니라 요청의 `root` 다([`crate::side::Side::here`]). CLI 로 치면
//!   `moai -C <root> <명령> --json` 을 그 `root` 에서 친 것이다
//! - 곁에 남는 것은 요청의 통([`crate::side`])에 모이고, 끝에 [`Answer`] 로 돌아온다. 프로세스 전역은 안 건드린다
//! - 화면 말·시간대·사람은 요청이 준다. 안 준 것은 CLI 와 같이 푼다(`MOAI_LANG`·설정, `TZ`, `MOAI_ACTOR`·git 설정)
//!
//! **명령을 다시 짜지 않는다.** 여섯 명령(`status`·`ready`·`prime`·`show`·`stats`·`project ls`)의 `cmd::*::run` 을
//! 그대로 부르고, 명령 뒤의 꼬리도 CLI 와 한 함수([`super::finish`])다. 그래서 아카이브 문맥·거르개·`--json` 의 꼴이
//! CLI 와 갈릴 자리가 없다.
//!
//! **부르는 쪽이 지킬 것** — 요청 하나는 이 입구를 한 번, 끝날 때까지 한 스레드에서 부른다(스레드는 아무것이나,
//! 여럿이 함께 불러도 된다). 프로세스의 현재 디렉터리를 옮기지 않는다. 요청 안에서 제 손으로 실을 띄워 moai 를
//! 부르면 그 실은 통을 모른다 — 그런 실도 이 입구를 지난다. 이 입구 밖에서 부른 `store`·`cmd` 는 옛 길(프로세스
//! 전역·stderr)로 간다.
#![cfg_attr(not(test), expect(dead_code, reason = "데몬의 읽기 메서드(moai-5n6m.q16)가 부를 입구다"))]

use super::{Ctx, R};
use crate::cli::{Cmd, ProjectCmd, ShowArgs, StatsArgs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 돌릴 읽기 명령 — 값은 CLI 의 그 명령이 받는 것과 같다.
pub(crate) enum Read {
    Status {
        worktree: bool,
    },
    Ready {
        worktree: bool,
        limit: Option<usize>,
    },
    Prime {
        worktree: bool,
    },
    /// 목록·id 하나·거르개 — `moai show` 와 같다(`moai epic show` 처럼 종류를 박은 것이 아니다).
    Show(Box<ShowArgs>),
    Stats(Box<StatsArgs>),
    /// 등록한 프로젝트 — 사용자 설정을 읽는다. [`Request::root`] 는 안 쓴다.
    ProjectLs,
}

impl Read {
    /// CLI 가 푼 명령에서 — 여섯 밖의 것은 그대로 돌려준다. 시험이 CLI 와 같은 인자로 두 길을 견줄 때 쓴다.
    pub(crate) fn of(cmd: Cmd) -> Result<Read, Box<Cmd>> {
        Ok(match cmd {
            Cmd::Status(w) => Read::Status { worktree: w.worktree },
            Cmd::Ready(a) => Read::Ready { worktree: a.worktree.worktree, limit: a.limit },
            Cmd::Prime(w) => Read::Prime { worktree: w.worktree },
            Cmd::Show(a) => Read::Show(Box::new(a)),
            Cmd::Stats(a) => Read::Stats(Box::new(a)),
            Cmd::Project(ProjectCmd::Ls) => Read::ProjectLs,
            other => return Err(Box::new(other)),
        })
    }
}

/// 요청 하나.
pub(crate) struct Request {
    /// 이 요청이 선 프로젝트 — `moai -C <root>` 의 그 자리다. 거기서 위로 `.moai` 를 찾는 것도 CLI 와 같다.
    pub root: PathBuf,
    pub read: Read,
    /// `--user` 와 같은 뜻의 `이름 (메일)`. 없으면 CLI 와 같이 `MOAI_ACTOR`, 그다음 그 프로젝트의 git 설정이다.
    pub actor: Option<String>,
    /// 화면 말. 없으면 CLI 와 같이 `MOAI_LANG`, 그다음 사용자 설정이다.
    pub lang: Option<crate::i18n::Lang>,
    /// 시간대. 없으면 CLI 와 같이 `TZ`, 그다음 `/etc/localtime` 이다.
    pub zone: Option<crate::tz::Zone>,
}

/// 요청 하나의 답.
pub(crate) struct Answer {
    /// CLI 가 `--json` 으로 stdout 에 낼 줄 — 넘어졌으면 CLI 가 `{"error", "code"}` 로 낼 그 실패다.
    pub out: R<Vec<String>>,
    /// CLI 가 stderr 에 낼 줄, 낸 차례대로. 색은 걷었다 — `--json` 의 CLI 도 색을 끈다.
    pub messages: Vec<String>,
    /// 덜 받은 답이다 — CLI 라면 `out` 을 다 낸 뒤 1 로 끝났다.
    pub partial: bool,
}

/// 읽기 명령 하나를 요청으로 돌린다. **프로세스의 현재 디렉터리도, 프로세스 전역의 알림 통도 안 건드린다.**
pub(crate) fn read(req: Request) -> Answer {
    let Request { root, read, actor, lang, zone } = req;
    let side: crate::side::Shared =
        Arc::new(Mutex::new(crate::side::Side { here: root, ..crate::side::Side::default() }));
    let entered = crate::side::enter(Some(side.clone()));
    // `chdir` 은 참이다 — 요청한 쪽은 그 자리에 서 있지 않으니, 그 자리에서 칠 명령은 `-C <뿌리>` 를 붙여 댄다.
    let ctx = Ctx::asked(true, actor, true, lang, zone);
    let out = match read {
        Read::Status { worktree } => super::status::run(&ctx, worktree),
        Read::Ready { worktree, limit } => super::ready::run(&ctx, worktree, limit),
        Read::Prime { worktree } => super::prime::run(&ctx, worktree),
        Read::Show(a) => super::show::run(&ctx, *a, None),
        Read::Stats(a) => super::stats::run(&ctx, *a),
        Read::ProjectLs => super::project::ls(&ctx),
    };
    super::finish(&ctx, &out);
    // `main` 의 꼬리(`crate::run`) 가운데 읽기가 닿는 둘 — 못 읽은 저널, 올라가 잡은 트래커. **`--json` 으로 넘어진
    // 판에서는 안 낸다** — 거기 stderr 는 오류 객체 하나다(`crate::run` 의 `Err` 갈래).
    let mut tail = Vec::new();
    let mut mine = false;
    if out.is_ok() {
        let unread = crate::side::with(|s| s.unread.clone()).unwrap_or_default();
        let said = crate::unread_lines(ctx.lang(), &unread);
        if !said.is_empty() {
            let here = crate::store::Repo::find(|| ctx.lang()).ok().flatten().map(|r| r.root);
            mine = crate::any_mine(here.as_deref(), &unread);
            tail.extend(said);
        }
        let climbed = crate::side::with(|s| s.climbed.clone()).unwrap_or_default();
        tail.extend(climbed.iter().map(|(from, to)| crate::climb_line(ctx.lang(), from, to)));
    }
    drop(entered);
    let side = std::mem::take(&mut *side.lock().unwrap_or_else(|e| e.into_inner()));
    let messages = side.told.into_iter().chain(tail).map(|l| crate::style::plain(&l)).collect();
    Answer { out, messages, partial: side.partial || mine }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use clap::Parser as _;
    use std::path::Path;
    use std::process::Command;

    const ACTOR: &str = "테스터 (tester@example.com)";
    /// 안쪽 시험이 받는 자리 — 바깥 시험이 다시 띄운 프로세스에서만 선다.
    const PLACE: &str = "MOAI_REQUEST_TEST_DIR";

    /// CLI 의 진짜 바이너리 — `cargo test` 는 통합 시험이 쓰려고 이것을 함께 짓는다(`target/<판>/moai`). 단위 시험에는
    /// `CARGO_BIN_EXE_moai` 가 안 서서 이 시험 바이너리(`target/<판>/deps/moai-…`)의 자리에서 찾는다. **`--lib` 만
    /// 돌리면 낡은 바이너리와 견줄 수 있다** — 그때는 `cargo test` 로 다시 돌린다.
    fn bin() -> std::path::PathBuf {
        let exe = std::env::current_exe().unwrap();
        let bin = exe.parent().and_then(Path::parent).unwrap().join(format!("moai{}", std::env::consts::EXE_SUFFIX));
        assert!(
            bin.is_file(),
            "CLI 바이너리가 없다: {} — `cargo test` 로 돌린다(통합 시험이 그것을 짓는다)",
            bin.display()
        );
        bin
    }

    /// 이 시험 바이너리를 **안쪽 시험 하나만** 도는 새 프로세스로 다시 띄운다 — 사람의 설정·시계·말을 걷은 환경이다.
    ///
    /// 따로 띄우는 까닭 둘. 요청이 안 준 것은 CLI 와 같이 환경과 사용자 설정에서 오는데(`MOAI_NOW`·`MOAI_CONFIG`),
    /// 단위 시험은 한 프로세스의 스레드라 `set_var` 로 그것을 못 박는다. 그리고 "프로세스 전역이 안 늘었다" 를 재려면
    /// 옆 시험이 그 전역을 안 건드린 프로세스가 든다(`lib` 의 시험이 [`super::super::note_partial`] 을 세운다).
    fn rerun(inner: &str, dir: &Path) {
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        for var in crate::git_leaks::swept(true) {
            cmd.env_remove(var);
        }
        let out = cmd
            .args(["--exact", inner, "--ignored", "--test-threads=1", "--nocapture"])
            .env(PLACE, dir)
            .env("HOME", &home)
            .env("MOAI_CONFIG", dir.join("config.toml"))
            .env("MOAI_CTL_DIR", dir.join("ctl"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("MOAI_NOW", "2026-09-11T04:12:03Z")
            .env("TZ", "UTC")
            .env("NO_COLOR", "1")
            .env_remove("MOAI_LANG")
            .env_remove("MOAI_ACTOR")
            .env_remove("MOAI_HERE")
            .env_remove("MOAI_KEYS")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("USERPROFILE")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CLAUDE_CODE_PLUGIN_CACHE_DIR")
            .output()
            .unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "안쪽 시험 {inner} 이 붉다\n{said}");
        // 안쪽이 아무것도 안 돌고 초록으로 끝나면 이 시험은 아무것도 안 본 것이다.
        assert!(said.contains("1 passed"), "안쪽 시험 {inner} 이 안 돌았다\n{said}");
    }

    /// CLI 한 판 — `moai -C <root> --json --user <사람> <args>` 를 그 자리에서, 고른 말로.
    struct Ran {
        out: String,
        messages: Vec<String>,
        partial: bool,
    }

    fn cli(root: &Path, lang: &str, args: &[&str]) -> Ran {
        let out = Command::new(bin())
            .arg("-C")
            .arg(root)
            .args(["--json", "--user", ACTOR])
            .args(args)
            .current_dir(root)
            .env("MOAI_LANG", lang)
            .output()
            .unwrap();
        let code = out.status.code();
        assert!(matches!(code, Some(0 | 1)), "moai {args:?} 가 {code:?} 로 끝났다");
        let stdout = String::from_utf8(out.stdout).unwrap();
        let messages = String::from_utf8_lossy(&out.stderr).lines().map(crate::style::plain).collect();
        Ran { out: stdout.strip_suffix('\n').unwrap_or(&stdout).to_string(), messages, partial: code == Some(1) }
    }

    /// 같은 인자를 요청으로.
    fn asked(root: &Path, lang: Lang, args: &[&str]) -> Answer {
        let cli = crate::cli::Cli::try_parse_from(std::iter::once("moai").chain(args.iter().copied())).unwrap();
        let read = Read::of(cli.cmd.unwrap()).unwrap_or_else(|_| panic!("{args:?} 는 읽기 명령이 아니다"));
        read_at(root, lang, read)
    }

    fn read_at(root: &Path, lang: Lang, read: Read) -> Answer {
        super::read(Request {
            root: root.to_path_buf(),
            read,
            actor: Some(ACTOR.to_string()),
            lang: Some(lang),
            zone: None,
        })
    }

    /// 시험이 쓰는 저장소 하나 — CLI 로 짓는다(쓰기는 이 입구의 일이 아니다).
    fn project(dir: &Path, prefix: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let moai = |args: &[&str]| {
            let out = Command::new(bin()).args(["--user", ACTOR]).args(args).current_dir(dir).output().unwrap();
            assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        };
        moai(&["init", prefix]);
        let plan = format!("# {prefix} 묶음\n- [p1] 첫 일 #core\n- [p2] 둘째 일\n");
        let mut child = Command::new(bin())
            .args(["--user", ACTOR, "add", "--from", "-"])
            .current_dir(dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(plan.as_bytes()).unwrap();
        assert!(child.wait().unwrap().success());
        moai(&["add", &format!("{prefix} 밖의 일"), "-p", "0", "-t", "bug"]);
        let bugs = moai(&["show", "--json", "-t", "bug"]);
        let id = serde_json::from_str::<serde_json::Value>(&bugs).unwrap()[0]["id"].as_str().unwrap().to_string();
        moai(&["mv", &id, "in_progress"]);
        moai(&["note", &id, "model: anthropic/opus-5.5 (high — 시험)"]);
        moai(&["project", "add", "."]);
    }

    /// 못 읽는 줄 하나 — `ready`·`stats` 가 stderr 에 그 파일의 자리를 대고 1 로 끝난다.
    fn garble(dir: &Path) {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(".moai/issues.jsonl")).unwrap();
        writeln!(f, "이 줄은 JSON 이 아니다").unwrap();
    }

    /// **한 프로세스에서 두 프로젝트·두 말을 번갈아 불러도 답이 저마다 CLI 와 같다**(moai-5n6m.m0b).
    ///
    /// 옛 길이었다면 두 곳이 붉어진다 — 앞 요청의 말이 `OnceLock` 에 남고, 앞 요청의 못 읽은 줄·부분 실패가 프로세스
    /// 전역에 남아 뒤의 모든 요청에 다시 선다. 견주는 것은 stdout 줄·stderr 줄·부분 실패 셋이다.
    #[test]
    fn the_read_entry_answers_as_the_cli_does() {
        let dir = crate::scratch::Scratch::new("request-cli");
        rerun("cmd::request::tests::inner_alternates_projects_and_languages", dir.path());
    }

    #[test]
    #[ignore = "the_read_entry_answers_as_the_cli_does 가 걷은 환경에서 띄운다"]
    fn inner_alternates_projects_and_languages() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let (a, b) = (dir.join("alpha"), dir.join("beta"));
        project(&a, "alpha");
        project(&b, "beta");
        garble(&b);
        let id = |root: &Path| {
            let ran = cli(root, "en", &["show", "-t", "bug"]);
            serde_json::from_str::<serde_json::Value>(&ran.out).unwrap()[0]["id"].as_str().unwrap().to_string()
        };
        let (ida, idb) = (id(&a), id(&b));
        let turns = [
            (&a, "en", Lang::En, &ida),
            (&b, "ko", Lang::Ko, &idb),
            (&a, "ko", Lang::Ko, &ida),
            (&b, "en", Lang::En, &idb),
        ];
        for round in 0..2 {
            for (root, code, lang, id) in turns {
                let shapes: [Vec<&str>; 10] = [
                    vec!["status"],
                    vec!["ready"],
                    vec!["prime"],
                    vec!["show"],
                    vec!["show", id],
                    vec!["show", "-s", "todo", "--sort", "priority"],
                    vec!["show", "--tree"],
                    vec!["stats"],
                    vec!["stats", "--by", "kind"],
                    vec!["project", "ls"],
                ];
                for args in shapes {
                    let want = cli(root, code, &args);
                    let got = asked(root, lang, &args);
                    let at = format!("{} {code} {args:?} (판 {round})", root.display());
                    let out = got.out.unwrap_or_else(|e| panic!("{at}: 넘어졌다 — {}", e.message));
                    assert_eq!(out.join("\n"), want.out, "{at}: stdout 이 CLI 와 다르다");
                    assert_eq!(got.messages, want.messages, "{at}: stderr 가 CLI 와 다르다");
                    assert_eq!(got.partial, want.partial, "{at}: 부분 실패가 CLI 와 다르다");
                }
            }
        }
        // 잰 것이 비지 않았다 — 못 읽는 줄을 든 쪽은 알림과 부분 실패를 정말 냈다.
        let ready = asked(&b, Lang::En, &["ready"]);
        assert!(ready.partial && !ready.messages.is_empty(), "못 읽는 줄이 아무 말도 안 냈다");
        // **프로세스 전역은 그대로다** — 요청이 남긴 것은 다 제 답으로 돌아갔다.
        assert!(!super::super::had_partial(), "요청의 부분 실패가 프로세스 깃발에 남았다");
        assert!(crate::store::journal_unread().is_empty() && crate::store::climbs().is_empty());
    }

    /// **스레드 둘이 함께 불러도 곁의 말이 안 섞인다**(moai-5n6m.m0b) — 저마다 제 저장소의 못 읽는 줄만 대고, 부분
    /// 실패도 저마다 선다. 옛 길이었다면 두 요청이 stderr 와 전역 깃발 하나를 나눠 썼다.
    #[test]
    fn two_threads_keep_their_messages_apart() {
        let dir = crate::scratch::Scratch::new("request-threads");
        rerun("cmd::request::tests::inner_two_threads", dir.path());
    }

    #[test]
    #[ignore = "two_threads_keep_their_messages_apart 가 걷은 환경에서 띄운다"]
    fn inner_two_threads() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let (a, b) = (dir.join("alpha"), dir.join("beta"));
        project(&a, "alpha");
        project(&b, "beta");
        garble(&a);
        garble(&b);
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|s| {
            for (mine, theirs, lang) in [(&a, &b, Lang::En), (&b, &a, Lang::Ko)] {
                let barrier = &barrier;
                s.spawn(move || {
                    let first = asked(mine, lang, &["ready"]);
                    barrier.wait();
                    for _ in 0..20 {
                        for args in [&["ready"][..], &["stats"], &["show"]] {
                            let got = asked(mine, lang, args);
                            assert!(got.partial, "{args:?}: 제 못 읽는 줄로 부분 실패가 안 섰다");
                            let said = got.messages.join("\n");
                            assert!(said.contains(&mine.display().to_string()), "{args:?}: 제 자리를 안 댄다\n{said}");
                            assert!(
                                !said.contains(&theirs.display().to_string()),
                                "{args:?}: 옆 요청의 말이 섞였다\n{said}"
                            );
                        }
                        let again = asked(mine, lang, &["ready"]);
                        assert_eq!(again.messages, first.messages, "같은 요청의 말이 판마다 다르다");
                        assert_eq!(again.out.unwrap(), first.out.as_ref().unwrap().clone());
                    }
                });
            }
        });
        assert!(!super::super::had_partial(), "요청의 부분 실패가 프로세스 깃발에 남았다");
    }
}
