//! svn 을 부르는 층(moai-0d0s.gst).
//!
//! **읽기만 한다.** 부를 수 있는 하위 명령은 [`Sub`] 의 넷(`info`·`status`·`diff`·`log`)뿐이고,
//! 명령을 짓는 자리는 [`command`] 하나다 — 하위 명령은 글이 아니라 타입으로 받으므로 `update`·
//! `cleanup`·`commit` 같은 쓰는 명령은 이 층으로 지을 수 없다. 시험이 임시 저장소를 지을 때 쓰는
//! `svnadmin create`·`checkout`·`commit` 은 `#[cfg(test)]` 밑 [`tests`] 에만 있다.
//!
//! 모든 부름이 지키는 것:
//! - `--non-interactive` 에 표준 입력을 끊고, 유닉스에서는 **세션을 갈라 제어 터미널을 놓는다**
//!   (`setsid`). `--non-interactive` 는 svn 제 물음만 막는다 — `svn+ssh` 의 ssh 가 `/dev/tty` 로
//!   비밀번호나 호스트 키를 물으면 탐색기의 화면이 깨지고 키를 빼앗긴다. 실측은 이 멤버의 노트에
//!   있다: 세션을 가른 자식은 `/dev/tty` 를 못 연다. 대가로 Ctrl-C 가 자식에 안 닿아, 연결을 기다리던
//!   svn 은 moai 가 죽은 뒤에도 연결 시도가 끝날 때까지(재 보니 135초) 남는다. 터미널도 입력도 안
//!   쥐고 쓰는 것도 없어 받아들인다
//! - 메시지 말은 `LC_MESSAGES=C` 로 고정하되 **문자셋은 UTF-8 로 둔다**([`ctype`]). `LC_ALL=C` 면
//!   한글 파일명 앞에서 svn 이 죽는다(`E000022`)
//! - 사람의 svn 설정(`~/.subversion` 의 인증 캐시·`servers`)과 `SVN_SSH` 는 그대로 쓴다. 시험 빌드만
//!   제 `--config-dir` 을 준다 — 사람의 홈에 `~/.subversion` 을 짓지 않는다
//! - 자리(`current_dir`)는 프로젝트 뿌리, 경로는 뿌리에서의 상대 경로를 [`literal`] 로 싸서 넘긴다
//! - 시한과 출력 한도는 [`crate::git::bounded_command`] 를 그대로 탄다. **서버에 닿는 부름**(URL 을
//!   대상으로 한 `info`·`diff`, 모든 `log`)은 따로 [`SERVER_TIMEOUT`] 안에 끝나야 한다 — 사람이 정한
//!   "svn 읽기에만 30초" 다. 로컬 작업 사본만 읽는 부름은 git 과 같이 부르는 쪽 시한만 따른다

pub(crate) mod xml;

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

/// 서버에 닿는 부름 하나가 기다리는 상한. 부르는 쪽 시한이 더 짧으면 그쪽이 이긴다.
pub(crate) const SERVER_TIMEOUT: Duration = Duration::from_secs(30);

/// 부를 수 있는 하위 명령 — 넷뿐이다. 여기 더하는 것이 이 층의 계약을 바꾸는 일이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sub {
    Info,
    Status,
    Diff,
    Log,
}

impl Sub {
    fn word(self) -> &'static str {
        match self {
            Sub::Info => "info",
            Sub::Status => "status",
            Sub::Diff => "diff",
            Sub::Log => "log",
        }
    }
}

/// 하위 명령 뒤에 붙는 낱말 하나.
#[derive(Debug, Clone)]
pub(crate) enum Arg<'a> {
    /// 이 층이 아는 꼬리표 — `--xml`·`-v`·`--stop-on-copy` 같은 것.
    Flag(&'static str),
    /// 꼬리표와 그 값을 두 낱말로 — `-c 12`·`-r 1:HEAD`·`--limit 2000`.
    Value(&'static str, String),
    /// 뿌리 밑의 경로. 옵션으로도 peg 리비전으로도 안 읽히게 [`literal`] 로 싼다.
    Path(&'a str),
    /// 저장소 URL. 리비전을 주면 `<URL>@<rev>`, 안 주면 빈 peg(`<URL>@`) 로 싼다 — URL 에 든 `@` 를
    /// svn 이 peg 로 읽지 않게.
    Url(&'a str, Option<u64>),
    /// `--old=<URL>@<rev>` — URL 대 작업 사본 diff 의 옛 쪽.
    Old(&'a str, u64),
    /// `--new=<뿌리 밑 경로>` — URL 대 작업 사본 diff 의 새 쪽.
    New(&'a str),
}

impl Arg<'_> {
    fn reaches_server(&self) -> bool {
        matches!(self, Arg::Url(..) | Arg::Old(..))
    }
}

/// 뿌리 밑의 경로를 svn 이 글자 그대로 읽게 싼다 — `./<경로>@`.
///
/// - `-` 로 시작하는 이름은 옵션으로 읽힌다. svn 에는 git 의 `--literal-pathspecs` 가 없어 `./` 를
///   앞에 붙인다
/// - 이름에 든 `@` 는 peg 리비전으로 읽힌다(`icon@2x.png` 의 `2x.png`). 끝에 빈 peg `@` 를 붙이면
///   svn 은 마지막 `@` 만 peg 로 읽는다
/// - 빈 경로는 뿌리 자신이다(`./@`)
pub(crate) fn literal(path: &str) -> String {
    format!("./{path}@")
}

/// URL 에 peg 리비전을 붙인다. `None` 은 빈 peg — HEAD 다.
pub(crate) fn pegged(url: &str, rev: Option<u64>) -> String {
    match rev {
        Some(r) => format!("{url}@{r}"),
        None => format!("{url}@"),
    }
}

/// 이 부름이 서버에 닿는가 — `log` 는 언제나, `info`·`diff` 는 URL 을 받았을 때.
pub(crate) fn reaches_server(sub: Sub, args: &[Arg]) -> bool {
    sub == Sub::Log || args.iter().any(Arg::reaches_server)
}

/// svn 을 띄울 명령 한 벌 — **짓는 자리는 여기 하나다.**
///
/// `diff` 는 언제나 `--internal-diff` 를 단다 — 사람의 `diff-cmd` 설정이 다른 프로그램을 부르지
/// 않게. `--ignore-properties` 도 함께다: 속성 변경은 diff 에 내지 않는다(에픽이 정한 것).
/// `status` 는 `--ignore-externals` 를 단다 — `svn:externals` 너머를 훑지 않는다.
pub(crate) fn command(root: &Path, sub: Sub, args: &[Arg]) -> std::process::Command {
    let mut cmd = std::process::Command::new("svn");
    cmd.current_dir(root).stdin(std::process::Stdio::null());
    environment(&mut cmd);
    cmd.arg(sub.word()).arg("--non-interactive");
    if let Some(dir) = config_dir() {
        cmd.arg("--config-dir").arg(dir);
    }
    match sub {
        Sub::Diff => {
            cmd.args(["--internal-diff", "--ignore-properties"]);
        }
        Sub::Status => {
            cmd.arg("--ignore-externals");
        }
        Sub::Info | Sub::Log => {}
    }
    for arg in args {
        match arg {
            Arg::Flag(f) => {
                cmd.arg(f);
            }
            Arg::Value(f, v) => {
                cmd.arg(f).arg(v);
            }
            Arg::Path(p) => {
                cmd.arg(literal(p));
            }
            Arg::Url(u, rev) => {
                cmd.arg(pegged(u, *rev));
            }
            Arg::Old(u, rev) => {
                cmd.arg(format!("--old={}", pegged(u, Some(*rev))));
            }
            Arg::New(p) => {
                cmd.arg(format!("--new={}", literal(p)));
            }
        }
    }
    detach(&mut cmd);
    cmd
}

/// 말은 C 로, 문자셋은 UTF-8 로. `LC_ALL`·`LANGUAGE` 는 걷는다 — 둘 다 `LC_MESSAGES` 를 이긴다.
fn environment(cmd: &mut std::process::Command) {
    let get = |k: &str| std::env::var_os(k);
    cmd.env_remove("LC_ALL").env_remove("LANGUAGE").env("LC_MESSAGES", "C").env("LC_CTYPE", ctype(get));
}

/// 자식에 줄 `LC_CTYPE`. 걷기 전에 실제로 쓰이던 문자셋(`LC_ALL` > `LC_CTYPE` > `LANG`)이 UTF-8 이면
/// 그것을 그대로, 아니면 UTF-8 기본값을 준다. 사람의 셸이 `LC_ALL=ko_KR.UTF-8` 이면 `LC_ALL` 을 걷는
/// 순간 문자셋이 `LANG` 으로 떨어지는데, 그것이 `C` 면 한글 이름에서 svn 이 죽는다.
fn ctype(get: impl Fn(&str) -> Option<OsString>) -> OsString {
    let effective = ["LC_ALL", "LC_CTYPE", "LANG"].into_iter().find_map(|k| get(k).filter(|v| !v.is_empty()));
    match effective {
        Some(v) if utf8_locale(&v) => v,
        _ => OsString::from(if cfg!(target_os = "macos") { "UTF-8" } else { "C.UTF-8" }),
    }
}

fn utf8_locale(v: &std::ffi::OsStr) -> bool {
    let v = v.to_string_lossy().to_ascii_lowercase();
    v.contains("utf-8") || v.contains("utf8")
}

/// 시험 빌드만 쓰는 svn 설정 자리. 사람의 홈에 `~/.subversion` 을 짓지 않는다.
#[cfg(test)]
fn config_dir() -> Option<std::path::PathBuf> {
    Some(tests::config_dir())
}

#[cfg(not(test))]
fn config_dir() -> Option<std::path::PathBuf> {
    None
}

/// 자식을 제 세션에 세운다 — 제어 터미널이 없으니 svn 도 ssh 도 `/dev/tty` 를 못 연다.
#[cfg(unix)]
fn detach(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt as _;
    // SAFETY: fork 와 exec 사이에서 부르는 것은 async-signal-safe 인 setsid(2) 하나뿐이고, 메모리를
    // 잡거나 락을 쥐지 않는다. 실패(이미 무리의 머리인 경우)는 그대로 두고 띄운다 — 그때도 터미널을
    // 쥐는 일은 부모가 정한 그대로다.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn detach(_cmd: &mut std::process::Command) {}

/// svn 을 부르다 난 실패. `kind` 가 기계의 말이다.
#[derive(Debug)]
pub(crate) enum Error {
    /// svn 을 못 띄웠다 — 안 깔린 기계가 가장 흔하다.
    Spawn(std::io::Error),
    /// 부르는 쪽 시한이 다 됐다.
    Timeout,
    /// 서버에 못 닿았다 — svn 의 오류 코드로 가른다([`OFFLINE`]), 또는 [`SERVER_TIMEOUT`] 이 다 됐다.
    /// 시한으로 끊긴 것은 svn 의 말이 없어 `None` 이다.
    Offline(Option<String>),
    /// 그 밖의 실패 — svn 의 stderr 를 싣는다. 낡은 작업 사본의 "upgrade 하라" 도 여기다.
    Failed(String),
}

/// 서버(저장소)에 못 닿았다는 svn 오류 코드. `E170013` 은 `file://` 의 없는 저장소에도 서고(실측),
/// 그것도 "저장소에 못 닿았다" 다.
const OFFLINE: &[&str] = &[
    "E170013", // Unable to connect to a repository at URL
    "E000111", // Connection refused
    "E000110", // Connection timed out
    "E000113", // No route to host
    "E000101", // Network is unreachable
    "E670002", // Unknown hostname
    "E670008", // Address not available / name resolution
    "E175002", // ra_serf connection failure
    "E175012", // ra_serf connection timed out
    "E210002", // svn:// connection closed unexpectedly
    "E210003", // svn:// network unreachable
];

impl Error {
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Spawn(e) if e.kind() == std::io::ErrorKind::NotFound => "no_svn",
            Error::Spawn(_) => "failed",
            Error::Timeout => "timeout",
            Error::Offline(_) => "offline",
            Error::Failed(_) => "failed",
        }
    }

    /// 실패가 딸고 나온 운영체제·svn 의 말. 옮기지 않는다.
    pub fn said(&self) -> String {
        match self {
            Error::Spawn(e) => e.to_string(),
            Error::Timeout => String::new(),
            Error::Offline(why) => why.clone().unwrap_or_default(),
            Error::Failed(why) => why.clone(),
        }
    }

    fn of_git(e: crate::git::Error) -> Self {
        match e {
            crate::git::Error::Spawn(e) => Error::Spawn(e),
            crate::git::Error::Failed(why) if codes(&why).any(|c| OFFLINE.contains(&c)) => Error::Offline(Some(why)),
            other => Error::Failed(other.said()),
        }
    }
}

/// `svn: E170013: …` 줄의 코드들.
pub(crate) fn codes(stderr: &str) -> impl Iterator<Item = &str> {
    stderr.lines().filter_map(|l| {
        let code = l.trim_start().strip_prefix("svn: ")?.split(':').next()?;
        (code.len() == 7 && code.starts_with('E') && code[1..].bytes().all(|b| b.is_ascii_digit())).then_some(code)
    })
}

/// svn 을 한 번 부른다 — 한도 `limit` 바이트까지 받고, 넘으면 자르고 자식을 끝낸다(`true`).
pub(crate) fn run(
    root: &Path,
    sub: Sub,
    args: &[Arg],
    limit: usize,
    budget: Option<Duration>,
) -> Result<(Vec<u8>, bool), Error> {
    bounded(command(root, sub, args), reaches_server(sub, args), limit, budget, SERVER_TIMEOUT)
}

/// 시한을 고르고 부른다. 서버에 닿는 부름은 `min(부르는 쪽 시한, server)` 를, 아니면 부르는 쪽
/// 시한만 쓴다. 끊겼을 때 묶은 것이 `server` 였으면 [`Error::Offline`], 부르는 쪽이었으면
/// [`Error::Timeout`] 이다.
fn bounded(
    cmd: std::process::Command,
    remote: bool,
    limit: usize,
    budget: Option<Duration>,
    server: Duration,
) -> Result<(Vec<u8>, bool), Error> {
    let (wait, server_bound) = match (remote, budget) {
        (false, b) => (b, false),
        (true, Some(b)) if b < server => (Some(b), false),
        (true, _) => (Some(server), true),
    };
    match crate::git::bounded_command(cmd, limit, wait) {
        Some(Ok(out)) => Ok(out),
        Some(Err(e)) => Err(Error::of_git(e)),
        None if server_bound => Err(Error::Offline(None)),
        None => Err(Error::Timeout),
    }
}

/// `svn info --xml` 이 낸 작업 사본 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Info {
    /// 그 자리의 URL.
    pub url: String,
    /// `^/trunk/sub` 꼴.
    pub relative_url: String,
    /// 저장소 뿌리 URL.
    pub repos_root: String,
    pub uuid: String,
    /// 그 자리의 리비전(BASE).
    pub revision: u64,
}

/// 뿌리 밑 `path`(빈 글이면 뿌리 자신)의 [`Info`]. 버전 관리 밖이면 실패다.
pub(crate) fn info(root: &Path, path: &str, budget: Option<Duration>) -> Result<Info, Error> {
    let (out, cut) = run(root, Sub::Info, &[Arg::Flag("--xml"), Arg::Path(path)], 1024 * 1024, budget)?;
    if cut {
        return Err(Error::Failed("svn info output exceeds its limit".into()));
    }
    info_of(&String::from_utf8_lossy(&out)).ok_or_else(|| Error::Failed("svn info gave no entry".into()))
}

fn info_of(text: &str) -> Option<Info> {
    let doc = xml::parse(text);
    let entry = doc.child("info")?.child("entry")?;
    let repo = entry.child("repository")?;
    let field = |e: &xml::Element, n: &str| e.child(n).map(|c| c.text().trim().to_owned()).filter(|t| !t.is_empty());
    Some(Info {
        url: field(entry, "url")?,
        relative_url: field(entry, "relative-url")?,
        repos_root: field(repo, "root")?,
        uuid: field(repo, "uuid")?,
        revision: entry.attr("revision")?.trim().parse().ok()?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::path::PathBuf;

    /// 시험 빌드의 `--config-dir`. 한 번만 짓는다 — `config`·`servers`·`README.txt` 를 미리 두면 svn 이
    /// 그 자리에 아무것도 새로 쓰지 않아, 같은 판의 스레드끼리 지으며 다투지 않는다.
    pub(crate) fn config_dir() -> PathBuf {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let dir = crate::scratch::base().join(format!("moai-svn-config-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            for f in ["config", "servers", "README.txt"] {
                let _ = std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(f));
            }
            dir
        })
        .clone()
    }

    /// 시험이 svn 을 쓸 수 있는가에 대한 판정.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum Gate {
        Run,
        Skip,
        Fail,
    }

    /// `found` 는 `svn`·`svnadmin` 이 둘 다 도는가, `required` 는 `MOAI_TEST_SVN` 의 값이다.
    /// `required` 면 없을 때 건너뛰지 않고 붉어진다 — CI 리눅스가 그렇게 돈다.
    pub(crate) fn gate(found: bool, required: Option<&str>) -> Gate {
        match (found, required) {
            (true, _) => Gate::Run,
            (false, Some("required")) => Gate::Fail,
            (false, _) => Gate::Skip,
        }
    }

    fn runs(program: &str) -> bool {
        std::process::Command::new(program)
            .arg("--version")
            .arg("--quiet")
            .stdin(std::process::Stdio::null())
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// svn 시험의 첫 줄. `false` 면 한 줄 알리고 그 시험을 그대로 통과시킨다.
    pub(crate) fn svn_ready() -> bool {
        static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let found = *FOUND.get_or_init(|| runs("svn") && runs("svnadmin"));
        match gate(found, std::env::var("MOAI_TEST_SVN").ok().as_deref()) {
            Gate::Run => true,
            Gate::Skip => {
                eprintln!("svn 시험을 건너뛴다: svn 이 없다");
                false
            }
            Gate::Fail => panic!("MOAI_TEST_SVN=required 인데 svn 이나 svnadmin 을 못 띄운다"),
        }
    }

    /// 시험이 쓰는 svn — 쓰는 명령도 부른다. **시험에서만 선다.**
    pub(crate) fn svn_in(dir: &Path, args: &[&str]) -> String {
        let mut cmd = std::process::Command::new("svn");
        cmd.current_dir(dir).stdin(std::process::Stdio::null());
        environment(&mut cmd);
        let out = cmd
            .args(args)
            .arg("--non-interactive")
            .arg("--config-dir")
            .arg(config_dir())
            .output()
            .unwrap_or_else(|e| panic!("svn {args:?}: {e}"));
        assert!(out.status.success(), "svn {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// `file://` URL — 윈도는 `file:///C:/…` 꼴이다.
    pub(crate) fn file_url(path: &Path) -> String {
        let p = path.to_string_lossy().replace('\\', "/");
        if p.starts_with('/') { format!("file://{p}") } else { format!("file:///{p}") }
    }

    /// `svnadmin create` 로 지은 임시 저장소와 `trunk`·`branches`·`tags`(r1).
    pub(crate) struct Repo {
        pub scratch: Scratch,
        pub url: String,
    }

    impl Repo {
        pub(crate) fn new(name: &str) -> Repo {
            let scratch = Scratch::real(name);
            let repo = scratch.join("repo");
            let out = std::process::Command::new("svnadmin")
                .arg("create")
                .arg(&repo)
                .stdin(std::process::Stdio::null())
                .output()
                .expect("svnadmin");
            assert!(out.status.success(), "svnadmin create: {}", String::from_utf8_lossy(&out.stderr));
            let url = file_url(&repo);
            svn_in(
                &scratch,
                &[
                    "mkdir",
                    "-q",
                    "-m",
                    "layout",
                    &format!("{url}/trunk"),
                    &format!("{url}/branches"),
                    &format!("{url}/tags"),
                ],
            );
            Repo { scratch, url }
        }

        /// `<저장소>/<sub>` 를 `<스크래치>/<dir>` 에 받는다.
        pub(crate) fn checkout(&self, sub: &str, dir: &str) -> PathBuf {
            let wc = self.scratch.join(dir);
            svn_in(&self.scratch, &["checkout", "-q", &format!("{}/{sub}", self.url), &wc.to_string_lossy()]);
            wc
        }

        /// trunk 를 `branches/<name>` 으로 복사한다.
        pub(crate) fn branch(&self, name: &str) {
            let (from, to) = (format!("{}/trunk", self.url), format!("{}/branches/{name}", self.url));
            svn_in(&self.scratch, &["copy", "-q", "-m", &format!("branch {name}"), &from, &to]);
        }
    }

    /// 작업 사본에 파일을 쓰고 더해 커밋한다. 경로는 [`literal`] 로 싸서 넘긴다.
    pub(crate) fn commit_files(wc: &Path, files: &[(&str, &str)], message: &str) {
        for (path, body) in files {
            let full = wc.join(path);
            if let Some(dir) = full.parent() {
                std::fs::create_dir_all(dir).expect("dir");
            }
            let fresh = !full.exists();
            std::fs::write(&full, body).expect("write");
            if fresh {
                svn_in(wc, &["add", "-q", "--parents", &literal(path)]);
            }
        }
        svn_in(wc, &["commit", "-q", "-m", message]);
    }

    /// `.svn` 밑의 이름·크기·mtime 과 `wc.db` 의 바이트 — 읽기 전후로 견준다.
    pub(crate) fn admin_snapshot(wc: &Path) -> Vec<(String, u64, Option<std::time::SystemTime>, Vec<u8>)> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, u64, Option<std::time::SystemTime>, Vec<u8>)>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
                let name = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().into_owned();
                if meta.is_dir() {
                    walk(&p, base, out);
                } else {
                    let bytes = if name == "wc.db" { std::fs::read(&p).unwrap_or_default() } else { Vec::new() };
                    out.push((name, meta.len(), meta.modified().ok(), bytes));
                }
            }
        }
        let mut out = Vec::new();
        walk(&wc.join(".svn"), &wc.join(".svn"), &mut out);
        out.sort();
        out
    }

    fn argv(cmd: &std::process::Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn every_subcommand_is_non_interactive_with_stdin_closed() {
        let root = Path::new(".");
        for sub in [Sub::Info, Sub::Status, Sub::Diff, Sub::Log] {
            let cmd = command(root, sub, &[]);
            let args = argv(&cmd);
            assert_eq!(args.first().map(String::as_str), Some(sub.word()));
            assert!(args.iter().any(|a| a == "--non-interactive"), "{sub:?}: {args:?}");
            let at = args.iter().position(|a| a == "--config-dir").expect("시험 빌드는 --config-dir 을 준다");
            assert_eq!(args.get(at + 1).map(PathBuf::from), Some(config_dir()));
            let envs: Vec<_> = cmd.get_envs().collect();
            assert!(envs.contains(&(std::ffi::OsStr::new("LC_MESSAGES"), Some(std::ffi::OsStr::new("C")))));
            assert!(envs.contains(&(std::ffi::OsStr::new("LC_ALL"), None)), "LC_ALL 을 걷는다");
        }
        let diff = argv(&command(root, Sub::Diff, &[]));
        assert!(diff.iter().any(|a| a == "--internal-diff"), "diff 는 사람의 diff-cmd 를 안 부른다");
        assert!(argv(&command(root, Sub::Status, &[])).iter().any(|a| a == "--ignore-externals"));
    }

    /// 지은 명령을 **가짜 `svn`** 으로 띄워 자식이 받은 표준 입력과 터미널을 본다. `spawn` 은 따로
    /// 안 주면 부모의 표준 입력을 물려주므로, [`command`] 가 끊지 않았으면 `/dev/null` 이 아니다.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_child_gets_no_stdin_and_no_terminal() {
        use std::os::unix::fs::PermissionsExt as _;
        let bin = Scratch::new("svn-fake");
        let fake = bin.join("svn");
        std::fs::write(
            &fake,
            "#!/bin/sh\nreadlink /proc/$$/fd/0\nif ( : < /dev/tty ) 2>/dev/null; then echo tty-open; else echo tty-closed; fi\n",
        )
        .expect("fake");
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        for sub in [Sub::Info, Sub::Status, Sub::Diff, Sub::Log] {
            let mut cmd = command(&bin, sub, &[]);
            // PATH 를 자식 환경에 주면 그것으로 프로그램을 찾는다 — 가짜가 먼저 잡힌다.
            let mut path = std::ffi::OsString::from(bin.path());
            path.push(":");
            path.push(std::env::var_os("PATH").unwrap_or_default());
            cmd.env("PATH", path);
            let child = cmd.stdout(std::process::Stdio::piped()).spawn().expect("spawn fake svn");
            let out = String::from_utf8_lossy(&child.wait_with_output().expect("wait").stdout).into_owned();
            let mut lines = out.lines();
            assert_eq!(lines.next(), Some("/dev/null"), "{sub:?}: 표준 입력이 열려 있다");
            assert_eq!(lines.next(), Some("tty-closed"), "{sub:?}: 제어 터미널을 놓지 않았다");
        }
    }

    #[test]
    fn literal_paths_and_pegs() {
        assert_eq!(literal("-r"), "./-r@");
        assert_eq!(literal("a@b"), "./a@b@");
        assert_eq!(literal(""), "./@");
        assert_eq!(pegged("file:///r/a@b", None), "file:///r/a@b@");
        assert_eq!(pegged("file:///r/trunk", Some(12)), "file:///r/trunk@12");
        let args = argv(&command(Path::new("."), Sub::Diff, &[Arg::Old("file:///r/trunk", 3), Arg::New("")]));
        assert!(args.contains(&"--old=file:///r/trunk@3".to_owned()) && args.contains(&"--new=./@".to_owned()));
        assert!(reaches_server(Sub::Log, &[]));
        assert!(reaches_server(Sub::Diff, &[Arg::Old("file:///r", 1), Arg::New("")]));
        assert!(reaches_server(Sub::Info, &[Arg::Url("file:///r", None)]));
        assert!(!reaches_server(Sub::Info, &[Arg::Path("x")]));
        assert!(!reaches_server(Sub::Status, &[Arg::Path("")]));
        assert!(!reaches_server(Sub::Diff, &[Arg::Path("")]));
    }

    #[test]
    fn the_charset_stays_utf8_when_lc_all_is_lifted() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
        };
        assert_eq!(ctype(env(&[("LC_ALL", "ko_KR.UTF-8"), ("LANG", "C")])), "ko_KR.UTF-8");
        assert_eq!(ctype(env(&[("LC_CTYPE", "en_US.utf8")])), "en_US.utf8");
        let fallback = if cfg!(target_os = "macos") { "UTF-8" } else { "C.UTF-8" };
        assert_eq!(ctype(env(&[("LC_ALL", "C"), ("LANG", "ko_KR.UTF-8")])), fallback);
        assert_eq!(ctype(env(&[])), fallback);
        assert_eq!(ctype(env(&[("LC_ALL", ""), ("LANG", "ko_KR.UTF-8")])), "ko_KR.UTF-8");
    }

    #[test]
    fn the_skip_gate_reads_detection_and_the_variable() {
        assert_eq!(gate(true, None), Gate::Run);
        assert_eq!(gate(true, Some("required")), Gate::Run);
        assert_eq!(gate(false, None), Gate::Skip);
        assert_eq!(gate(false, Some("")), Gate::Skip);
        assert_eq!(gate(false, Some("required")), Gate::Fail);
    }

    #[test]
    fn error_codes_split_offline_from_failed() {
        let offline = "svn: E170013: Unable to connect to a repository at URL 'svn://h/x'\nsvn: E000111: Can't connect to host 'h': Connection refused";
        assert_eq!(codes(offline).collect::<Vec<_>>(), ["E170013", "E000111"]);
        assert_eq!(Error::of_git(crate::git::Error::Failed(offline.into())).kind(), "offline");
        let upgrade =
            "svn: E155036: Please see the 'svn upgrade' command\nsvn: E155036: The working copy at '/x' is too old";
        let e = Error::of_git(crate::git::Error::Failed(upgrade.into()));
        assert_eq!(e.kind(), "failed");
        assert!(e.said().contains("svn upgrade"), "낡은 작업 사본의 말을 그대로 올린다");
        let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        assert_eq!(Error::of_git(crate::git::Error::Spawn(missing)).kind(), "no_svn");
        assert_eq!(codes("svn: warning: W155010: x\nsvn: E2: short\n한글 svn: E170013:").count(), 0);
    }

    #[test]
    fn info_xml_is_read_whatever_the_attribute_order() {
        let text = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<info>\n<entry\n   kind=\"dir\"\n   revision=\"4\"\n   path=\"한글 &amp; 디렉터리\">\n<url>file:///r/trunk/%ED%95%9C</url>\n<relative-url>^/trunk/%ED%95%9C</relative-url>\n<repository>\n<root>file:///r</root>\n<uuid>0f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a</uuid>\n</repository>\n</entry>\n</info>\n";
        let info = info_of(text).expect("info");
        assert_eq!(info.revision, 4);
        assert_eq!(info.relative_url, "^/trunk/%ED%95%9C");
        assert_eq!(info.uuid, "0f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a");
        assert!(info_of("<info><entry revision=\"4\"><url>x</url></entry></info>").is_none(), "빈 필드는 답이 아니다");
        assert!(info_of("<info><entry revision=\"x\"").is_none());
    }

    /// 받아들이고 끝없이 기다리는 서버 — svn:// 의 인사를 안 보낸다. 받은 연결을 돌려준다.
    fn silent_server() -> (String, std::sync::mpsc::Receiver<std::net::TcpStream>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("svn://127.0.0.1:{}/x", listener.local_addr().expect("addr").port());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                if tx.send(s).is_err() {
                    break;
                }
            }
        });
        (url, rx)
    }

    /// 끊긴 자식이 정말 죽었는가 — 그 연결이 EOF 로 닫힌다.
    fn closed(rx: &std::sync::mpsc::Receiver<std::net::TcpStream>) {
        use std::io::Read as _;
        let mut s = rx.recv_timeout(Duration::from_secs(10)).expect("svn 이 서버에 닿았다");
        s.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        let mut buf = [0u8; 64];
        loop {
            match s.read(&mut buf) {
                Ok(0) => return,
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => return,
                Err(e) => panic!("끊은 svn 이 아직 연결을 쥐고 있다: {e}"),
            }
        }
    }

    #[test]
    fn a_silent_server_is_offline_after_the_server_timeout_and_the_child_goes() {
        if !svn_ready() {
            return;
        }
        let (url, rx) = silent_server();
        let root = Scratch::new("svn-silent");
        let args = [Arg::Flag("--xml"), Arg::Url(&url, None)];
        let started = std::time::Instant::now();
        let got = bounded(command(&root, Sub::Info, &args), true, 1 << 20, None, Duration::from_millis(800));
        assert!(matches!(got, Err(Error::Offline(None))), "{got:?}");
        assert!(started.elapsed() < Duration::from_secs(10), "서버 시한에 끊는다: {:?}", started.elapsed());
        closed(&rx);

        // 부르는 쪽 시한이 더 짧으면 그것은 서버 탓이 아니다 — timeout 이다.
        let got = bounded(
            command(&root, Sub::Info, &args),
            true,
            1 << 20,
            Some(Duration::from_millis(300)),
            Duration::from_secs(30),
        );
        assert!(matches!(got, Err(Error::Timeout)), "{got:?}");
        closed(&rx);

        // 시한 0 도 timeout 이다.
        let got =
            bounded(command(&root, Sub::Info, &args), true, 1 << 20, Some(Duration::ZERO), Duration::from_secs(30));
        assert!(matches!(got, Err(Error::Timeout)), "{got:?}");
    }

    #[test]
    fn the_server_timeout_does_not_touch_local_calls() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("svn-local");
        let wc = repo.checkout("trunk", "wc");
        commit_files(&wc, &[("a.txt", "a\n")], "first");
        // 서버 시한을 0 으로 줘도 작업 사본만 읽는 부름은 끝까지 간다.
        let got = bounded(command(&wc, Sub::Status, &[Arg::Flag("--xml")]), false, 1 << 20, None, Duration::ZERO);
        assert!(got.is_ok(), "{got:?}");
        let got = bounded(
            command(&wc, Sub::Info, &[Arg::Flag("--xml"), Arg::Path("")]),
            false,
            1 << 20,
            None,
            Duration::ZERO,
        );
        assert!(got.is_ok(), "{got:?}");
        // 로컬 부름도 부르는 쪽 시한 0 이면 timeout 이다.
        let got = run(&wc, Sub::Status, &[Arg::Flag("--xml")], 1 << 20, Some(Duration::ZERO));
        assert!(matches!(got, Err(Error::Timeout)), "{got:?}");
    }

    #[test]
    fn output_past_the_limit_is_cut() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("svn-limit");
        let wc = repo.checkout("trunk", "wc");
        for i in 0..5 {
            commit_files(&wc, &[("a.txt", &"x\n".repeat(i + 1))], &format!("rev {i}"));
        }
        let url = format!("{}/trunk", repo.url);
        let (bytes, cut) = run(&wc, Sub::Log, &[Arg::Flag("--xml"), Arg::Url(&url, None)], 100, None).expect("log");
        assert!(cut);
        assert_eq!(bytes.len(), 100);
        let (bytes, cut) = run(&wc, Sub::Log, &[Arg::Flag("--xml"), Arg::Url(&url, None)], 1 << 20, None).expect("log");
        assert!(!cut && bytes.len() > 100);
    }

    #[test]
    fn info_reads_the_working_copy_and_its_subdirectory_without_writing() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("svn-info");
        let wc = repo.checkout("trunk", "wc");
        commit_files(&wc, &[("sub/s.txt", "s\n")], "first");
        svn_in(&wc, &["update", "-q"]);
        let before = admin_snapshot(&wc);
        let top = info(&wc, "", None).expect("info");
        assert_eq!(top.url, format!("{}/trunk", repo.url));
        assert_eq!(top.relative_url, "^/trunk");
        assert_eq!(top.repos_root, repo.url);
        assert_eq!(top.revision, 2);
        assert!(!top.uuid.is_empty());
        let sub = info(&wc.join("sub"), "", None).expect("info sub");
        assert_eq!(sub.url, format!("{}/trunk/sub", repo.url));
        assert_eq!(sub.relative_url, "^/trunk/sub");
        assert_eq!(sub.uuid, top.uuid);
        assert_eq!(info(&wc, "sub", None).expect("info path").relative_url, "^/trunk/sub");
        assert_eq!(admin_snapshot(&wc), before, "info 가 .svn 을 바꿨다");
        // 버전 관리 밖은 실패다.
        std::fs::write(wc.join("loose.txt"), "x").expect("write");
        assert_eq!(info(&wc, "loose.txt", None).map_err(|e| e.kind()), Err("failed"));
        let outside = Scratch::new("svn-outside");
        assert_eq!(info(&outside, "", None).map_err(|e| e.kind()), Err("failed"));
        // 없는 저장소는 offline 이다(E170013).
        let gone = file_url(&repo.scratch.join("nope"));
        let got = run(&wc, Sub::Info, &[Arg::Flag("--xml"), Arg::Url(&gone, None)], 1 << 20, None);
        assert_eq!(got.map(|_| ()).map_err(|e| e.kind()), Err("offline"));
    }

    #[test]
    fn hostile_names_reach_info_literally() {
        if !svn_ready() {
            return;
        }
        let repo = Repo::new("svn-names");
        let wc = repo.checkout("trunk", "wc");
        let mut names = vec!["--output=evil", "a@b", "@", "-r", "sp ace.txt", "한글 이름.txt", "it's.txt"];
        if cfg!(unix) {
            names.push("q\"uote.txt");
        }
        let files: Vec<(&str, &str)> = names.iter().map(|n| (*n, "x\n")).collect();
        commit_files(&wc, &files, "names");
        svn_in(&wc, &["update", "-q"]);
        for name in &names {
            let got = info(&wc, name, None).unwrap_or_else(|e| panic!("{name}: {e:?} {}", e.said()));
            let tail = got.url.rsplit_once("/trunk/").map(|(_, t)| decoded(t));
            assert_eq!(tail.as_deref(), Some(*name), "{}", got.url);
        }
        assert!(!wc.join("evil").exists(), "--output=evil 이 옵션으로 읽혔다");
    }

    /// URL 의 퍼센트 꼴을 푼다 — 시험이 받은 이름과 견주려고.
    fn decoded(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%'
                && i + 2 < b.len()
                && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
            out.push(b[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}
