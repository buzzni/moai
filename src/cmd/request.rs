//! 명령 하나를 **요청으로** 돌리는 입구 — 오래 사는 프로세스(데몬)가 부른다. 읽기 여섯은 [`read`](moai-5n6m.m0b),
//! 쓰기 일곱은 [`write`](moai-3hgm.25e)다.
//!
//! CLI 는 argv 를 풀고, `-C` 면 프로세스의 현재 디렉터리를 옮기고, 명령을 돌린 뒤 곁에 남은 것(알림·부분 실패)을
//! 프로세스 전역에서 한 번 읽고 끝난다. 데몬은 한 프로세스에서 여러 프로젝트·여러 요청을, 스레드로 함께 돌리기도
//! 하므로 그 셋을 다 못 쓴다. 여기는 그 셋을 요청의 것으로 바꿔 **같은 명령 함수**를 부른다 —
//!
//! - 자리는 프로세스의 현재 디렉터리가 아니라 요청의 `root` 다([`crate::side::Side::here`]). CLI 로 치면
//!   `moai -C <root> <명령> --json` 을 그 `root` 에서 친 것이다
//! - 곁에 남는 것은 요청의 통([`crate::side`])에 모이고, 끝에 [`Answer`] 로 돌아온다. 프로세스 전역은 안 건드린다
//! - 화면 말·시간대·사람은 요청이 준다. 안 준 것은 CLI 와 같이 푼다(`MOAI_LANG`·설정, `TZ`, git 설정) — **사람만은
//!   이 프로세스의 `MOAI_ACTOR` 를 안 읽는다**(moai-3hgm.k7s, `model::actor`): 그 값은 데몬을 띄운 셸의 것이다
//!
//! **명령을 다시 짜지 않는다.** 읽기 여섯(`status`·`ready`·`prime`·`show`·`stats`·`project ls`)과 쓰기 일곱(`add`·
//! `mv`·`edit`·`note`·`defer`·`rm`·`link`)의 `cmd::*::run` 을 그대로 부르고, 명령 뒤의 꼬리도 CLI 와 한 함수다
//! ([`super::finish`] 와 `lib` 의 줄 짓기). 그래서 아카이브 문맥·거르개·`--json` 의 꼴·쓰기 경로(`store::with_write`)가
//! CLI 와 갈릴 자리가 없다.
//!
//! **쓰기에는 stdin 도 argv 도 없다**(moai-3hgm.25e) — 통이 걸렸으면 글 자리의 `-` 는 stdin 이 아니라 그 글이고
//! (`add::read_body`), stdin 을 읽으려는 길(`add --from -`)은 기다리지 않고 거절된다. 깃발 같은 제목을 막는 검사
//! (`refuse_if_flag_like`)도 안 선다 — 요청의 필드는 형이 있는 글이다.
//!
//! **부르는 쪽이 지킬 것** — 요청 하나는 이 입구를 한 번, 끝날 때까지 한 스레드에서 부른다(스레드는 아무것이나,
//! 여럿이 함께 불러도 된다). 프로세스의 현재 디렉터리를 옮기지 않는다. 요청 안에서 제 손으로 실을 띄워 moai 를
//! 부르면 그 실은 통을 모른다 — 그런 실도 이 입구를 지난다. 이 입구 밖에서 부른 `store`·`cmd` 는 옛 길(프로세스
//! 전역·stderr)로 간다.
#![cfg_attr(not(any(test, unix)), allow(dead_code, reason = "부르는 데몬 메서드는 유닉스에만 선다"))]

use super::{Ctx, R};
use crate::cli::{
    AddArgs, BacklogCmd, Cmd, DeferArgs, EditArgs, LinkArgs, MvArgs, NoteArgs, ProjectCmd, RmArgs, ShowArgs, StatsArgs,
    Typed,
};
use crate::model::Kind;
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
    #[cfg_attr(not(test), expect(dead_code, reason = "시험만 부른다"))]
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

/// 돌릴 쓰기 명령 — **값은 CLI 의 인자 구조체 그대로다**(moai-3hgm.25e). 명령 함수가 받는 꼴을 따로 짜면 두 길이
/// 갈린다. 글 자리(본문·까닭·노트)는 받은 그대로 글이다 — `-` 한 글자도(모듈 머리).
pub(crate) enum Write {
    /// `moai add`. `kind` 가 서면 `moai <kind> add` 다 — `cmd::typed` 가 부르는 `add::run(ctx, args, Some(kind))` 그대로.
    /// **계획(`--from`)은 안 낸다**: 그 길은 stdin 이나 파일을 읽어, 통이 걸리면 거절된다(`add::read_source`).
    Add {
        args: Box<AddArgs>,
        kind: Option<Kind>,
    },
    Mv(Box<MvArgs>),
    Edit(Box<EditArgs>),
    Note(Box<NoteArgs>),
    Defer(Box<DeferArgs>),
    Rm(Box<RmArgs>),
    Link(Box<LinkArgs>),
}

impl Write {
    /// CLI 가 푼 명령에서 — 일곱 밖의 것은 그대로 돌려준다. 종류를 박은 `add`(`moai epic add` 등)는 [`Write::Add`] 의
    /// `kind` 로 온다 — `cmd::dispatch` 가 가르는 것과 같다. 시험이 CLI 와 같은 인자로 두 길을 견줄 때 쓴다.
    #[cfg_attr(not(test), expect(dead_code, reason = "시험만 부른다"))]
    pub(crate) fn of(cmd: Cmd) -> Result<Write, Box<Cmd>> {
        let add = |args: AddArgs, kind| Write::Add { args: Box::new(args), kind };
        Ok(match cmd {
            Cmd::Add(a) => add(a, None),
            Cmd::Issue(Typed::Add(a)) => add(a, Some(Kind::Issue)),
            Cmd::Epic(Typed::Add(a)) => add(a, Some(Kind::Epic)),
            Cmd::Milestone(Typed::Add(a)) => add(a, Some(Kind::Milestone)),
            Cmd::Backlog(BacklogCmd::Common(t)) => match *t {
                Typed::Add(a) => add(a, Some(Kind::Backlog)),
                other => return Err(Box::new(Cmd::Backlog(BacklogCmd::Common(Box::new(other))))),
            },
            Cmd::Mv(a) => Write::Mv(Box::new(a)),
            Cmd::Edit(a) => Write::Edit(Box::new(a)),
            Cmd::Note(a) => Write::Note(Box::new(a)),
            Cmd::Defer(a) => Write::Defer(Box::new(a)),
            Cmd::Rm(a) => Write::Rm(Box::new(a)),
            Cmd::Link(a) => Write::Link(Box::new(a)),
            other => return Err(Box::new(other)),
        })
    }
}

/// 요청 하나.
pub(crate) struct Request {
    /// 이 요청이 선 프로젝트 — `moai -C <root>` 의 그 자리다. 거기서 위로 `.moai` 를 찾는 것도 CLI 와 같다.
    pub root: PathBuf,
    pub read: Read,
    /// `--user` 와 같은 뜻의 `이름 (메일)`. 없으면 그 프로젝트의 git 설정이고, 그것도 없으면 모르는 사람이다(쓰기는
    /// `no_actor` 로 멈춘다). **이 프로세스의 `MOAI_ACTOR` 는 안 읽는다**(moai-3hgm.k7s, `model::actor`).
    pub actor: Option<String>,
    /// 화면 말. 없으면 CLI 와 같이 `MOAI_LANG`, 그다음 사용자 설정이다.
    pub lang: Option<crate::i18n::Lang>,
    /// 시간대. 없으면 CLI 와 같이 `TZ`, 그다음 `/etc/localtime` 이다.
    pub zone: Option<crate::tz::Zone>,
    /// 부르는 쪽이 이 요청을 풀려고 이미 읽은 사용자 설정. 없으면 명령이 처음 물을 때 읽는다 — 주면 한 요청이 같은
    /// 파일을 두 번 안 판다.
    pub reg: Option<crate::user_config::Registry>,
}

/// 쓰기 요청 하나 — 명령 밖의 다섯 칸은 [`Request`] 와 같은 뜻이다(moai-3hgm.25e).
pub(crate) struct WriteRequest {
    pub root: PathBuf,
    pub write: Write,
    pub actor: Option<String>,
    pub lang: Option<crate::i18n::Lang>,
    pub zone: Option<crate::tz::Zone>,
    pub reg: Option<crate::user_config::Registry>,
}

/// 요청 하나의 답.
pub(crate) struct Answer {
    /// CLI 가 `--json` 으로 stdout 에 낼 줄 — 넘어졌으면 CLI 가 `{"error", "code"}` 로 낼 그 실패다.
    pub out: R<Vec<String>>,
    /// CLI 가 stderr 에 낼 줄, 낸 차례대로. 색은 걷었다 — `--json` 의 CLI 도 색을 끈다. 넘어진 판의 오류 객체는
    /// 안 든다 — 그것은 `out` 이다.
    pub messages: Vec<String>,
    /// 덜 받은 답이다 — CLI 라면 `out` 을 다 낸 뒤 1 로 끝났다.
    pub partial: bool,
    /// CLI 가 고를 종료 코드 — `crate::run` 과 같은 자다: 넘어졌으면 1, 명령이 고른 코드가 있으면 그것, 덜 받은
    /// 답이면 1, 아니면 0(moai-3hgm.25e). 데몬의 봉투가 `exit` 로 싣는다(moai-3hgm.k7s).
    pub exit: u8,
}

/// 읽기 명령 하나를 요청으로 돌린다. **프로세스의 현재 디렉터리도, 프로세스 전역의 알림 통도 안 건드린다.**
pub(crate) fn read(req: Request) -> Answer {
    let Request { root, read, actor, lang, zone, reg } = req;
    // `chdir` 은 참이다 — 요청한 쪽은 그 자리에 서 있지 않으니, 그 자리에서 칠 명령은 `-C <뿌리>` 를 붙여 댄다.
    let ctx = Ctx::asked(true, actor, true, lang, zone, reg);
    answer(root, &ctx, |ctx| match read {
        Read::Status { worktree } => super::status::run(ctx, worktree),
        Read::Ready { worktree, limit } => super::ready::run(ctx, worktree, limit),
        Read::Prime { worktree } => super::prime::run(ctx, worktree),
        Read::Show(a) => super::show::run(ctx, *a, None),
        Read::Stats(a) => super::stats::run(ctx, *a),
        Read::ProjectLs => super::project::ls(ctx),
    })
}

/// 쓰기 명령 하나를 요청으로 돌린다(moai-3hgm.25e) — [`read`] 와 같은 약속이다. 쓰기는 CLI 와 같은
/// `store::with_write` 를 지나고, 쓰기 뒤에 CLI 가 stderr 에 내는 꼬리(못 읽는 줄을 들고 쓴 것·못 적은 저널·덧붙일
/// 말·못 읽은 저널·올라가 잡은 트래커·루트로 옮겨 쓴 자리)는 [`Answer::messages`] 로 돌아온다.
pub(crate) fn write(req: WriteRequest) -> Answer {
    let WriteRequest { root, write, actor, lang, zone, reg } = req;
    let ctx = Ctx::asked(true, actor, true, lang, zone, reg);
    answer(root, &ctx, |ctx| match write {
        Write::Add { args, kind } => super::add::run(ctx, *args, kind),
        Write::Mv(a) => super::mv::run(ctx, *a),
        Write::Edit(a) => super::edit::run(ctx, *a),
        Write::Note(a) => super::note::run(ctx, *a),
        Write::Defer(a) => super::defer::run(ctx, *a),
        Write::Rm(a) => super::rm::run(ctx, *a),
        Write::Link(a) => super::link::run(ctx, *a),
    })
}

/// 요청 하나를 제 통 안에서 돌리고 답으로 접는다 — [`read`] 와 [`write`] 의 몸이다.
///
/// 차례는 `crate::run` 그대로다: 명령이 낸 알림(`tell`) → 명령 뒤의 꼬리([`super::finish`]) → `main` 의 꼬리
/// ([`tail`]). **`--json` 으로 넘어진 판에서는 `main` 의 꼬리를 안 낸다** — 거기 stderr 는 오류 객체 하나다
/// (`crate::run` 의 `Err` 갈래).
fn answer(root: PathBuf, ctx: &Ctx, run: impl FnOnce(&Ctx) -> R<Vec<String>>) -> Answer {
    let side: crate::side::Shared =
        Arc::new(Mutex::new(crate::side::Side { here: root, ..crate::side::Side::default() }));
    let entered = crate::side::enter(Some(side.clone()));
    let out = run(ctx);
    super::finish(ctx, &out);
    let (tail, mine) = match out.is_ok() {
        true => tail(ctx),
        false => (Vec::new(), false),
    };
    drop(entered);
    let side = std::mem::take(&mut *side.lock().unwrap_or_else(|e| e.into_inner()));
    let messages = side.told.into_iter().chain(tail).map(|l| crate::style::plain(&l)).collect();
    let partial = side.partial || mine;
    // `crate::run` 이 고르는 자 그대로 — 고른 코드가 먼저고, 덜 받은 답은 그다음이다.
    let exit = match (&out, side.exit) {
        (Err(_), _) => 1,
        (Ok(_), 0) => u8::from(partial),
        (Ok(_), code) => code,
    };
    Answer { out, messages, partial, exit }
}

/// `main` 의 꼬리(`crate::run` 의 `Ok` 갈래)가 stderr 에 낼 줄 — 같은 차례, 같은 글이다. 든 것은 이 요청의 통에서
/// 읽는다(통이 걸린 채 부른다). 뒤의 값은 못 읽은 저널 가운데 제 저장소의 것이 있는가 — CLI 라면 1 로 끝났다.
///
/// 차례: 못 읽는 줄을 들고 쓴 것(`carried`) → 못 적은 저널(`unjournaled`) → 덧붙일 말(`noted`) → 못 읽은 저널
/// (`unread_journals`) → 올라가 잡은 트래커 → 루트로 옮겨 쓴 자리(`redirected`). **빈 것은 자리도 말도 안 고른다** —
/// `crate::run` 의 넷과 같은 까닭이다. 자리(`Repo::find`)는 앞의 넷이 찾고, 그 찾기가 올라가 잡은 것까지 뒤의 줄에 선다.
fn tail(ctx: &Ctx) -> (Vec<String>, bool) {
    let here = || crate::store::Repo::find(|| ctx.lang()).ok().flatten().map(|r| r.root);
    let mut lines = Vec::new();
    let tallies = crate::store::unreadable_tallies();
    if crate::carried_any(&tallies) {
        lines.extend(crate::carried_lines(ctx.lang(), here().as_deref(), &tallies));
    }
    let missed = crate::store::journal_misses();
    if !missed.is_empty() {
        lines.extend(crate::unjournaled_lines(ctx.lang(), here().as_deref(), &missed));
    }
    let notes = crate::store::write_notes();
    if !notes.is_empty() {
        lines.extend(crate::noted_lines(here().as_deref(), &notes));
    }
    let mut mine = false;
    let unread = crate::store::journal_unread();
    if !unread.is_empty() {
        // 자리를 먼저 찾는다 — `crate::unread_journals` 가 `tell_unread` 앞에서 찾는 차례 그대로다.
        let here = here();
        let said = crate::unread_lines(ctx.lang(), &unread);
        if !said.is_empty() {
            mine = crate::any_mine(here.as_deref(), &unread);
            lines.extend(said.iter().map(|l| crate::warned(l)));
        }
    }
    let climbed = crate::side::with(|s| s.climbed.clone()).unwrap_or_default();
    lines.extend(climbed.iter().map(|(from, to)| crate::climb_line(ctx.lang(), from, to)));
    lines.extend(crate::store::redirects().iter().map(|(from, to)| crate::moved_line(ctx.lang(), from, to)));
    (lines, mine)
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
        rerun_with(inner, dir, None, &[]);
    }

    /// [`rerun`] 에 stdin 과 환경을 더 먹여 띄운다. `fed` 가 서면 그 바이트를 쓰고 닫는다 — 통이 걸린 요청이 그것을 안
    /// 읽었는지를 안쪽 시험이 끝에 제 손으로 읽어 잰다. 닫아 두므로 잘못 읽은 길도 멈추지 않고 그 글을 쥐어, 단언이
    /// 붉어진다. `env` 는 걷은 환경 위에 마지막으로 세운다.
    fn rerun_with(inner: &str, dir: &Path, fed: Option<&[u8]>, env: &[(&str, &str)]) {
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        for var in crate::git_leaks::swept(true) {
            cmd.env_remove(var);
        }
        cmd.args(["--exact", inner, "--ignored", "--test-threads=1", "--nocapture"])
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
            // 집기 기록(`hook::record_own_picks`)은 이 세션의 것이 아니다 — 시험을 돌리는 Claude 세션의 값이 새면 `mv` 가
            // 그 세션의 집기로 적는다.
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .envs(env.iter().copied())
            .stdin(match fed {
                Some(_) => std::process::Stdio::piped(),
                None => std::process::Stdio::null(),
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        if let Some(fed) = fed {
            use std::io::Write as _;
            // 쓰고 놓는다 — 놓는 것이 닫는 것이다(EOF).
            child.stdin.take().unwrap().write_all(fed).unwrap();
        }
        let out = child.wait_with_output().unwrap();
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
            reg: None,
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

    /// 저널을 못 읽게 막는다 — 저널을 읽는 `show`·`stats` 가 그 자리를 대고 1 로 끝난다(`store::note_missed` 의 길).
    /// 막아도 읽히는 자리(root)면 빈 목록이다. 시험 끝에 [`unlock`] 으로 풀어야 임시 자리가 치워진다.
    #[cfg(unix)]
    fn lock_journals(dir: &Path) -> Vec<std::path::PathBuf> {
        use std::os::unix::fs::PermissionsExt as _;
        let files: Vec<_> = std::fs::read_dir(dir.join(".moai/journal")).unwrap().map(|e| e.unwrap().path()).collect();
        for f in &files {
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        if files.iter().any(|f| std::fs::read(f).is_ok()) {
            unlock(&files);
            return Vec::new();
        }
        files
    }

    #[cfg(unix)]
    fn unlock(files: &[std::path::PathBuf]) {
        use std::os::unix::fs::PermissionsExt as _;
        for f in files {
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
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
        // 저널 자리는 `a` 에서 막는다 — `b` 는 못 읽는 스냅샷 줄을 든다. 셋째 통(못 읽은 저널)도 CLI 와 견준다.
        #[cfg(unix)]
        let locked = lock_journals(&a);
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
        #[cfg(unix)]
        if !locked.is_empty() {
            let shown = asked(&a, Lang::En, &["show", &ida]);
            let at = locked[0].display().to_string();
            assert!(shown.partial, "못 읽은 저널로 부분 실패가 안 섰다");
            assert!(shown.messages.iter().any(|m| m.starts_with("moai: ") && m.contains(&at)), "{:?}", shown.messages);
            unlock(&locked);
        }
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

    /// 쓰기 한 판 — stdout 줄·stderr 줄·종료 코드와, 그 뒤 트래커(`.moai`)의 파일들. 파일은 읽기 좋게 글로 든다 —
    /// 붉어질 때 어긋난 줄이 그대로 보인다.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Wrote {
        out: String,
        messages: Vec<String>,
        exit: u8,
        files: std::collections::BTreeMap<String, String>,
    }

    /// `.moai` 밑의 파일 전부 — 자리(`.moai` 에서 본) → 바이트. 락 파일은 뺀다: 쓰기가 잡았다 놓은 자리일 뿐 쓴 것이 아니다.
    fn files_of(moai: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        fn walk(at: &Path, base: &Path, into: &mut std::collections::BTreeMap<String, Vec<u8>>) {
            for e in std::fs::read_dir(at).unwrap() {
                let path = e.unwrap().path();
                if path.is_dir() {
                    walk(&path, base, into);
                } else if path.file_name().is_some_and(|n| n != "lock") {
                    let rel = path.strip_prefix(base).unwrap().display().to_string();
                    into.insert(rel, std::fs::read(&path).unwrap());
                }
            }
        }
        let mut into = std::collections::BTreeMap::new();
        walk(moai, moai, &mut into);
        into
    }

    /// `.moai` 를 찍어 둔 그대로 되돌린다 — CLI 가 쓴 뒤, 요청이 같은 자리에서 다시 쓰게.
    fn put_back(moai: &Path, snap: &std::collections::BTreeMap<String, Vec<u8>>) {
        std::fs::remove_dir_all(moai).unwrap();
        for (rel, bytes) in snap {
            let at = moai.join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, bytes).unwrap();
        }
    }

    fn texts(files: std::collections::BTreeMap<String, Vec<u8>>) -> std::collections::BTreeMap<String, String> {
        files.into_iter().map(|(k, v)| (k, String::from_utf8_lossy(&v).into_owned())).collect()
    }

    /// CLI 의 쓰기 한 판 — `moai -C <root> --json --user <사람> <args>` 를 그 자리에서. 트래커는 `tracker` 의 것을 잰다
    /// (워크트리에서 친 쓰기는 루트의 것을 고친다).
    fn cli_wrote(root: &Path, tracker: &Path, lang: &str, args: &[&str]) -> Wrote {
        let out = Command::new(bin())
            .arg("-C")
            .arg(root)
            .args(["--json", "--user", ACTOR])
            .args(args)
            .current_dir(root)
            .env("MOAI_LANG", lang)
            .output()
            .unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        Wrote {
            out: stdout.strip_suffix('\n').unwrap_or(&stdout).to_string(),
            messages: String::from_utf8_lossy(&out.stderr).lines().map(crate::style::plain).collect(),
            exit: u8::try_from(out.status.code().expect("시그널로 끝났다")).unwrap(),
            files: texts(files_of(&tracker.join(".moai"))),
        }
    }

    /// 같은 인자를 쓰기 요청으로. 넘어졌으면 CLI 가 stderr 끝에 낼 오류 객체(`crate::fail`)를 말 끝에 붙여 견준다.
    fn asked_wrote(root: &Path, tracker: &Path, lang: Lang, args: &[&str]) -> Wrote {
        let got = write_at(root, lang, args);
        let (out, mut messages) = (got.out, got.messages);
        let out = match out {
            Ok(lines) => lines.join("\n"),
            Err(e) => {
                messages.push(serde_json::json!({"error": e.message, "code": e.code}).to_string());
                String::new()
            }
        };
        Wrote { out, messages, exit: got.exit, files: texts(files_of(&tracker.join(".moai"))) }
    }

    fn write_at(root: &Path, lang: Lang, args: &[&str]) -> Answer {
        let cli = crate::cli::Cli::try_parse_from(std::iter::once("moai").chain(args.iter().copied())).unwrap();
        let write = Write::of(cli.cmd.unwrap()).unwrap_or_else(|_| panic!("{args:?} 는 쓰기 명령이 아니다"));
        super::write(WriteRequest {
            root: root.to_path_buf(),
            write,
            actor: Some(ACTOR.to_string()),
            lang: Some(lang),
            zone: None,
            reg: None,
        })
    }

    /// 새로 선 줄의 id — `add --json` 의 답에서.
    fn made_id(out: &str) -> String {
        serde_json::from_str::<serde_json::Value>(out).unwrap()["id"]
            .as_str()
            .expect("만든 줄의 id 가 없다")
            .to_string()
    }

    /// 같은 쓰기를 **같은 자리에서** CLI 로 한 번, 되돌린 뒤 요청으로 한 번 — stdout·stderr·종료 코드·남은 파일이 같다.
    /// 남는 것은 요청이 쓴 것이다(CLI 의 것과 같다). `made` 면 새 줄을 세운다: id 는 시각과 프로세스로 뽑으므로
    /// 요청 쪽의 id 를 CLI 의 것으로 바꿔 놓고, id 차례로 선 스냅샷은 줄을 골라 견준다.
    fn same(root: &Path, tracker: &Path, code: &str, lang: Lang, args: &[&str], made: bool) -> Wrote {
        let moai = tracker.join(".moai");
        let before = files_of(&moai);
        let want = cli_wrote(root, tracker, code, args);
        put_back(&moai, &before);
        let mut got = asked_wrote(root, tracker, lang, args);
        let at = format!("{} {code} {args:?}", root.display());
        let mut want_cmp = want.clone();
        if made {
            let (theirs, mine) = (made_id(&want.out), made_id(&got.out));
            assert_ne!(theirs, mine, "{at}: 두 판이 같은 id 를 뽑았다 — 바꿔 견줄 것이 없다");
            got.out = got.out.replace(&mine, &theirs);
            got.messages = got.messages.iter().map(|m| m.replace(&mine, &theirs)).collect();
            for text in got.files.values_mut() {
                *text = text.replace(&mine, &theirs);
            }
            for files in [&mut got.files, &mut want_cmp.files] {
                let snap = files.get_mut("issues.jsonl").unwrap();
                let mut lines: Vec<&str> = snap.lines().collect();
                lines.sort_unstable();
                *snap = lines.join("\n");
            }
        }
        assert_eq!(got, want_cmp, "{at}: 요청의 쓰기가 CLI 와 다르다");
        want
    }

    /// 제목으로 줄 하나의 id.
    fn id_titled(root: &Path, title: &str) -> String {
        let ran = cli(root, "en", &["show", "--all"]);
        let rows: Vec<serde_json::Value> = serde_json::from_str(&ran.out).unwrap();
        let row = rows.iter().find(|r| r["title"] == title).unwrap_or_else(|| panic!("`{title}` 이 없다"));
        row["id"].as_str().unwrap().to_string()
    }

    /// **쓰기 일곱이 요청으로도 CLI 와 같다**(moai-3hgm.25e) — 같은 자리에서 같은 인자로 CLI 와 [`super::write`] 를 돌려
    /// stdout 줄·stderr 줄·종료 코드와 그 뒤의 스냅샷·저널 바이트를 견준다.
    ///
    /// 판마다 CLI 가 stderr 에 내는 갈래를 하나씩 든다 — 없는 에픽·남의 줄·`--from` 에 진 집기·없는 id(부분 실패와
    /// 넘어진 쓰기)·못 읽는 줄을 들고 쓴 것·못 적은 저널·워크트리에서 루트로 옮겨 쓴 것. 옛 길이었다면 그 말들은
    /// `eprintln!` 과 프로세스 전역으로 가 답에 안 섰다.
    #[test]
    fn the_write_entry_answers_as_the_cli_does() {
        let dir = crate::scratch::Scratch::real("request-write");
        rerun("cmd::request::tests::inner_writes_as_the_cli_does", dir.path());
    }

    #[test]
    #[ignore = "the_write_entry_answers_as_the_cli_does 가 걷은 환경에서 띄운다"]
    fn inner_writes_as_the_cli_does() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let a = dir.join("alpha");
        project(&a, "alpha");
        let setup = Command::new(bin())
            .args(["--user", ACTOR, "add", "남의 일", "-a", "Other (other@example.com)"])
            .current_dir(&a)
            .output()
            .unwrap();
        assert!(setup.status.success(), "{}", String::from_utf8_lossy(&setup.stderr));
        let (first, second) = (id_titled(&a, "첫 일"), id_titled(&a, "둘째 일"));
        let (bug, other) = (id_titled(&a, "alpha 밖의 일"), id_titled(&a, "남의 일"));
        let w = |code: &str, lang: Lang, args: &[&str]| same(&a, &a, code, lang, args, false);
        let made = |code: &str, lang: Lang, args: &[&str]| same(&a, &a, code, lang, args, true);

        made("en", Lang::En, &["add", "새 일", "-b", "본문 한 줄\n둘째 줄", "-p", "1", "-t", "bug"]);
        made("ko", Lang::Ko, &["epic", "add", "새 묶음"]);
        made("en", Lang::En, &["backlog", "add", "생각 하나"]);
        let held = made("en", Lang::En, &["add", "없는 에픽 밑", "-e", "alpha-zzzz"]);
        assert!(!held.messages.is_empty(), "없는 에픽이 아무 말도 안 냈다");
        let stale = w("en", Lang::En, &["mv", &bug, "review", "--from", "todo"]);
        assert_eq!(stale.exit, 1, "`--from` 에 진 집기가 0 으로 끝났다: {stale:?}");
        let theirs = w("ko", Lang::Ko, &["mv", &other, "in_progress"]);
        assert!(theirs.messages.iter().any(|m| m.contains(&other)), "남의 줄을 집은 말이 없다: {theirs:?}");
        w("en", Lang::En, &["edit", &first, "--title", "고친 첫 일", "-b", "새 본문"]);
        w("ko", Lang::Ko, &["note", &first, "남길 말"]);
        let gone = w("en", Lang::En, &["note", "alpha-zzzz", "없는 줄"]);
        assert!(gone.exit == 1 && gone.out.is_empty(), "없는 줄의 노트가 넘어지지 않았다: {gone:?}");
        let missing = w("en", Lang::En, &["mv", "alpha-zzzz", &first, "done"]);
        assert_eq!(missing.exit, 1, "없는 id 가 부분 실패로 안 섰다: {missing:?}");
        w("en", Lang::En, &["defer", &second, "-m", "다음 분기"]);
        w("ko", Lang::Ko, &["defer", &second, "--undo"]);
        w("en", Lang::En, &["link", &first, "--blocks", &second]);
        w("en", Lang::En, &["rm", &other]);

        // 못 읽는 줄을 들고 쓴다 — `main` 의 꼬리가 "그대로 두고 썼다" 를 낸다(`lib::carried`).
        garble(&a);
        let carried = w("en", Lang::En, &["note", &first, "깨진 줄 곁에서"]);
        let carried_too = w("ko", Lang::Ko, &["mv", &first, "review"]);
        for c in [&carried, &carried_too] {
            assert!(c.messages.iter().any(|m| m.contains("moai show")), "들고 간 줄을 안 말했다: {c:?}");
        }

        // 스냅샷은 쓰고 저널은 못 적는다 — "썼지만 이력은 못 남겼다"(`lib::unjournaled`). 뿌리(root)는 권한을 안 따르므로
        // 막아도 적히면 그 판은 견주기만 한다.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let journal = a.join(".moai/journal");
            let shut = |mode: u32| {
                for e in std::fs::read_dir(&journal).unwrap() {
                    std::fs::set_permissions(e.unwrap().path(), std::fs::Permissions::from_mode(mode)).unwrap();
                }
                let dir_mode = if mode == 0o444 { 0o555 } else { 0o755 };
                std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(dir_mode)).unwrap();
            };
            let moai = a.join(".moai");
            let before = files_of(&moai);
            shut(0o444);
            let want = cli_wrote(&a, &a, "en", &["mv", &first, "done"]);
            shut(0o644);
            put_back(&moai, &before);
            shut(0o444);
            let got = asked_wrote(&a, &a, Lang::En, &["mv", &first, "done"]);
            shut(0o644);
            assert_eq!(got, want, "못 적은 저널의 쓰기가 CLI 와 다르다");
        }

        // **딸린 워크트리에서 친 쓰기는 루트의 트래커를 고치고 그 자리를 댄다**(`lib::redirected`).
        let main = dir.join("wt");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        let moai = |args: &[&str]| {
            let out = Command::new(bin()).args(["--user", ACTOR]).args(args).current_dir(&main).output().unwrap();
            assert!(out.status.success(), "moai {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        moai(&["init", "wt"]);
        moai(&["add", "워크트리 일"]);
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        git(&main, &["worktree", "add", "-q", "side", "-b", "side"]);
        let side = main.join("side");
        let id = id_titled(&main, "워크트리 일");
        let moved = same(&side, &main, "en", Lang::En, &["note", &id, "워크트리에서 남긴 말"], false);
        assert!(
            moved.messages.iter().any(|m| m.contains(&main.join(".moai").display().to_string())),
            "루트로 옮겨 쓴 자리를 안 댔다: {moved:?}"
        );

        // **프로세스 전역은 그대로다** — 요청이 남긴 것은 다 제 답으로 돌아갔다.
        assert!(
            !super::super::had_partial() && super::super::chosen_exit() == 0,
            "요청의 종료 코드가 프로세스에 남았다"
        );
        assert!(crate::store::redirects().is_empty(), "옮겨 쓴 자리가 프로세스 통에 남았다");
        assert!(crate::store::unreadable_tallies().is_empty(), "들고 간 줄의 셈이 프로세스 통에 남았다");
        assert!(crate::store::journal_misses().is_empty() && crate::store::write_notes().is_empty());
        assert!(crate::store::journal_unread().is_empty() && crate::store::climbs().is_empty());
    }

    /// **통이 걸린 쓰기는 stdin 을 안 읽는다**(moai-3hgm.25e) — 글 자리의 `-` 는 그 글이고, stdin 을 읽으려는 길
    /// (`add --from -`·`--from /dev/stdin`)은 기다리지 않고 거절된다. 깃발 같은 제목도 글로 받는다. 끝에 이 프로세스의
    /// stdin 이 먹인 그대로 남았는지 제 손으로 읽어 잰다 — 하나라도 읽었으면 거기서 붉어진다.
    #[test]
    fn a_hung_side_never_reads_the_real_stdin() {
        let dir = crate::scratch::Scratch::new("request-stdin");
        rerun_with("cmd::request::tests::inner_never_reads_stdin", dir.path(), Some(b"SENTINEL\n"), &[]);
    }

    #[test]
    #[ignore = "a_hung_side_never_reads_the_real_stdin 이 stdin 을 먹여 띄운다"]
    fn inner_never_reads_stdin() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let a = dir.join("alpha");
        project(&a, "alpha");
        let first = id_titled(&a, "첫 일");
        let ok = |args: &[&str]| -> serde_json::Value {
            let got = write_at(&a, Lang::En, args);
            let out = got.out.unwrap_or_else(|e| panic!("{args:?}: 넘어졌다 — {}", e.message));
            serde_json::from_str(&out.join("\n")).unwrap()
        };
        assert_eq!(ok(&["note", &first, "-b", "-"])["text"], "-", "`-b -` 의 `-` 를 글로 안 받았다");
        assert_eq!(ok(&["note", &first, "-"])["text"], "-", "자리 인자 `-` 를 글로 안 받았다");
        let made = ok(&["add", "-b", "-", "--", "-x"]);
        assert_eq!((&made["title"], &made["body"]), (&serde_json::json!("-x"), &serde_json::json!("-")));
        assert_eq!(ok(&["edit", &first, "-b", "-"])["body"], "-");
        ok(&["mv", &first, "in_progress", "-m", "-"]);
        ok(&["defer", &first, "-m", "-"]);
        for from in ["-", "/dev/stdin"] {
            let got = write_at(&a, Lang::En, &["add", "--from", from]);
            let e = got.out.err().unwrap_or_else(|| panic!("`--from {from}` 가 섰다"));
            assert_eq!(e.code, super::super::code::BAD_INPUT, "`--from {from}`: {}", e.message);
        }
        let mut left = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut left).unwrap();
        assert_eq!(left, "SENTINEL\n", "통이 걸린 요청이 프로세스의 stdin 을 읽었다");
    }

    /// **요청은 세션의 집기를 안 적는다**(moai-3hgm.25e) — 데몬의 `CLAUDE_CODE_SESSION_ID` 는 그것을 띄운 셸의 것이다.
    /// 적으면 Claude 세션에서 띄운 데몬의 모든 `mv` 가 그 세션의 집기로 서서, 그 세션의 훅이 남의 줄을 제 것으로 읽는다.
    /// 같은 환경의 CLI 는 적는다 — 시험이 기록 자리를 옳게 보는지를 그쪽이 잰다.
    #[test]
    fn a_request_records_no_pickup_for_the_daemons_session() {
        let dir = crate::scratch::Scratch::new("request-picks");
        rerun_with(
            "cmd::request::tests::inner_records_no_pickup",
            dir.path(),
            None,
            &[("CLAUDE_CODE_SESSION_ID", "daemon-shell")],
        );
    }

    #[test]
    #[ignore = "a_request_records_no_pickup_for_the_daemons_session 이 세션 값을 세워 띄운다"]
    fn inner_records_no_pickup() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let a = dir.join("alpha");
        project(&a, "alpha");
        // 집기 기록은 사용자 설정 곁(`<MOAI_CONFIG 의 자리>/picks`)에 선다. 짓는 `project` 의 CLI `mv` 가 이미 하나 적었다.
        let picks = dir.join("picks");
        let recorded = || if picks.is_dir() { texts(files_of(&picks)) } else { Default::default() };
        let (first, second) = (id_titled(&a, "첫 일"), id_titled(&a, "둘째 일"));
        let before = recorded();
        let got = write_at(&a, Lang::En, &["mv", &first, "in_progress"]);
        assert!(got.out.is_ok() && got.exit == 0, "요청의 집기가 넘어졌다: {:?}", got.messages);
        assert_eq!(recorded(), before, "요청이 데몬의 세션으로 집기를 적었다");
        let ran = cli_wrote(&a, &a, "en", &["mv", &second, "in_progress"]);
        assert_eq!(ran.exit, 0, "{ran:?}");
        let after = recorded();
        assert!(
            after != before && after.values().any(|t| t.contains(&second)),
            "같은 환경의 CLI 가 집기를 안 적었다 — 이 시험이 기록 자리를 잘못 본다"
        );
    }

    /// 데몬을 띄운 셸의 사람 — 요청이 안 읽어야 할 `MOAI_ACTOR` 다.
    const DAEMON: &str = "Daemon (daemon@example.com)";

    /// **요청은 데몬의 `MOAI_ACTOR` 를 안 읽는다**(moai-3hgm.k7s) — 그 값은 데몬을 띄운 셸의 것이라 요청을 보낸 쪽을
    /// 말해 주지 않는다. `actor` 도 git 설정도 없으면 쓰기는 `no_actor` 로 멈추고, 읽기는 모르는 사람이라 아무것도
    /// 안 가른다. 같은 환경의 CLI 는 그 값으로 쓰고 가른다 — 시험이 환경을 옳게 세웠는지를 그쪽이 잰다.
    #[test]
    fn a_request_never_reads_the_daemons_moai_actor() {
        let dir = crate::scratch::Scratch::new("request-actor");
        rerun_with("cmd::request::tests::inner_never_reads_moai_actor", dir.path(), None, &[("MOAI_ACTOR", DAEMON)]);
    }

    #[test]
    #[ignore = "a_request_never_reads_the_daemons_moai_actor 가 MOAI_ACTOR 를 세워 띄운다"]
    fn inner_never_reads_moai_actor() {
        let dir = std::path::PathBuf::from(std::env::var_os(PLACE).expect("바깥 시험이 띄운다"));
        let a = dir.join("alpha");
        project(&a, "alpha");
        let first = id_titled(&a, "첫 일");
        let parsed = |args: &[&str]| {
            let cli = crate::cli::Cli::try_parse_from(std::iter::once("moai").chain(args.iter().copied())).unwrap();
            cli.cmd.unwrap()
        };
        let unasked = |args: &[&str]| {
            let write = Write::of(parsed(args)).unwrap_or_else(|_| panic!("{args:?} 는 쓰기 명령이 아니다"));
            super::write(WriteRequest {
                root: a.clone(),
                write,
                actor: None,
                lang: Some(Lang::En),
                zone: None,
                reg: None,
            })
        };
        let cli_bare = |args: &[&str]| {
            Command::new(bin()).arg("-C").arg(&a).arg("--json").args(args).current_dir(&a).output().unwrap()
        };

        // 쓰기 — 요청은 멈추고, 같은 환경의 CLI 는 `MOAI_ACTOR` 로 쓴다.
        let got = unasked(&["note", &first, "누가 썼나"]);
        let e = got.out.expect_err("사람을 모르는 요청의 쓰기가 섰다 — 데몬의 MOAI_ACTOR 를 읽었다");
        assert_eq!(e.code, super::super::code::NO_ACTOR, "{}", e.message);
        let ran = cli_bare(&["note", &first, "누가 썼나"]);
        assert!(
            ran.status.success(),
            "같은 환경의 CLI 가 MOAI_ACTOR 로 못 썼다: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let note: serde_json::Value = serde_json::from_slice(&ran.stdout).unwrap();
        assert_eq!(
            note["by_email"], "daemon@example.com",
            "CLI 가 MOAI_ACTOR 를 안 읽었다 — 이 시험이 환경을 잘못 세웠다"
        );

        // 읽기 — 모르는 사람이라 테스터의 줄이 `others` 로 안 간다. CLI 는 데몬의 사람으로 가른다.
        let others = |out: &str| -> usize {
            let v: serde_json::Value = serde_json::from_str(out).unwrap();
            v["others"].as_array().unwrap().len()
        };
        let read = Read::of(parsed(&["ready"])).unwrap_or_else(|_| panic!("ready 는 읽기 명령이다"));
        let asked =
            super::read(Request { root: a.clone(), read, actor: None, lang: Some(Lang::En), zone: None, reg: None });
        let out = asked.out.unwrap_or_else(|e| panic!("ready 가 넘어졌다 — {}", e.message)).join("\n");
        assert_eq!(others(&out), 0, "요청의 ready 가 데몬의 사람으로 남의 줄을 갈랐다: {out}");
        let ran = cli_bare(&["ready"]);
        assert!(
            others(&String::from_utf8_lossy(&ran.stdout)) > 0,
            "같은 환경의 CLI 가 가르지 않았다 — 견줄 거리가 없다"
        );
    }
}
