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

pub(crate) mod cache;
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
    /// `--new=./<뿌리 밑 경로>@BASE` — 저장소 대 작업 사본 BASE diff 의 새 쪽(moai-0d0s.x27 의 `branch`). 노드마다의 BASE
    /// 를 읽는다: `svn commit` 은 뿌리의 리비전을 안 올려, `<URL>@<뿌리 리비전>` 은 update 전까지 방금 커밋한 리비전을
    /// 빠뜨린다(에픽 끝 리뷰 실측, svn 1.14.3). 손댄 것·더한 것·디스크에서 없어진 파일은 안 읽고 `.svn` 도 그대로다.
    NewBase(&'a str),
    /// 작업 사본 `diff` 의 대상 경로 — `./<경로>`, **끝에 `@` 를 안 붙인다**(moai-0d0s.s6g 실측, svn 1.14.3).
    /// 작업 사본 대상의 `svn diff` 는 peg 를 안 읽는다: `./a@b` 는 그대로 `a@b` 고, `./m.txt@` 는 `@` 까지 이름으로
    /// 읽어 `E155010: The node '…/m.txt@' was not found` 로 죽는다. `-` 로 시작하는 이름은 `./` 가 막는다.
    /// `info`·`status` 의 경로는 [`Arg::Path`] 다.
    Local(&'a str),
    /// `--summarize --xml` — `diff` 가 글 대신 파일마다 `item`(added·deleted·modified)과 `kind` 를 낸다. 바이너리·빈 파일의
    /// 더함·지움은 글 diff 에 `---`/`+++` 줄이 없어 거기서는 못 가린다(에픽 끝 리뷰 2번, svn 1.14.3 실측). 이것이 선
    /// `diff` 에는 `--internal-diff` 를 안 단다 — svn 이 둘을 함께 받지 않고(`E205000`), 요약은 diff 프로그램을 부르지
    /// 않는다(사람의 `diff-cmd` 를 걸어 재 봤다). `--show-copies-as-adds` 도 안 받지만, 복사된 디렉터리의 파일은 그것
    /// 없이도 하나하나 `added` 로 선다. 경로는 대상 URL(URL 대 작업 사본이면 `--old` 의 URL) 밑의 인코딩된 URL 이다 — [`below`].
    Summarize,
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
/// 않게. 하나뿐인 예외는 [`Arg::Summarize`] 다 — svn 이 그것과 함께는 안 받고, 요약은 diff 프로그램을 안 부른다. `--ignore-properties` 도 함께다: 속성 변경은 diff 에 내지 않는다(에픽이 정한 것).
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
        Sub::Diff if args.iter().any(|a| matches!(a, Arg::Summarize)) => {
            cmd.arg("--ignore-properties");
        }
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
            Arg::NewBase(p) => {
                cmd.arg(format!("--new=./{p}@BASE"));
            }
            Arg::Local(p) => {
                cmd.arg(format!("./{p}"));
            }
            Arg::Summarize => {
                cmd.args(["--summarize", "--xml"]);
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
    Some(tests::config_override().unwrap_or_else(tests::config_dir))
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
    /// id → 리비전 표([`cache`])를 아직 다 못 채웠다 — 시한이 먼저 다 됐다(moai-0d0s.4o8). 채운 조각은 캐시에
    /// 남아 다음 부름이 이어 채운다. 덜 찬 표로는 답하지 않는다. 기계의 말은 `timeout` 이다.
    Filling,
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
            Error::Filling => "timeout",
        }
    }

    /// 실패가 딸고 나온 운영체제·svn 의 말. 옮기지 않는다.
    pub fn said(&self) -> String {
        match self {
            Error::Spawn(e) => e.to_string(),
            Error::Timeout => String::new(),
            Error::Offline(why) => why.clone().unwrap_or_default(),
            Error::Failed(why) => why.clone(),
            Error::Filling => "the revision table is still being filled".into(),
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

/// Where a working copy stands by the trunk·branches·tags convention (moai-0d0s.x27), read from its `relative-url` by
/// [`layout`] — the one place that decides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Layout {
    /// `…/trunk[/…]` — the main line, as a Git main checkout: no fork point.
    Trunk,
    /// `…/branches/<x>[/…]`.
    Branch(Branch),
    /// Anything else: `tags/<x>`, a path with none of the three words, `branches` with no name after it.
    Unknown,
}

/// A working copy on `…/branches/<name>[/<sub>]`. Paths are repository paths split into **decoded** components, so
/// they are encoded exactly once, by [`url`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Branch {
    pub name: String,
    /// The branch root, keyword and any prefix included — `["proj", "branches", "x"]`.
    pub root: Vec<String>,
    /// Where the working copy stands below the branch root — `["sub"]`, or empty at the root.
    pub sub: Vec<String>,
}

impl Layout {
    /// The branch's name, when the working copy stands on one.
    pub(crate) fn branch_name(&self) -> Option<&str> {
        match self {
            Layout::Branch(b) => Some(&b.name),
            Layout::Trunk | Layout::Unknown => None,
        }
    }
}

/// **The convention, decided here and nowhere else** (moai-0d0s.x27). The first of `trunk`·`branches`·`tags` met from
/// the repository root decides: `…/trunk[/…]` is the trunk, `…/branches/<x>[/…]` the branch `<x>` — its root is
/// everything up to `<x>`, the words before the keyword kept as a prefix — and the rest is [`Layout::Unknown`]. A URL
/// whose components do not decode to plain names (empty, `.`, `..`, bad `%`) is unknown too. Pure.
pub(crate) fn layout(relative_url: &str) -> Layout {
    let Some(parts) = components(relative_url) else { return Layout::Unknown };
    let Some(at) = parts.iter().position(|p| matches!(p.as_str(), "trunk" | "branches" | "tags")) else {
        return Layout::Unknown;
    };
    match (parts[at].as_str(), parts.get(at + 1)) {
        ("trunk", _) => Layout::Trunk,
        ("branches", Some(name)) => Layout::Branch(Branch {
            name: name.clone(),
            root: parts[..=at + 1].to_vec(),
            sub: parts[at + 2..].to_vec(),
        }),
        _ => Layout::Unknown,
    }
}

/// The working copy at `root`, read with `info` (local, no server) and placed by [`layout`]. **The one seam moai-0d0s.qg0
/// asks "is this working copy a branch?" through** — the `branch`·`whole` scopes decide by the same [`layout`].
pub(crate) fn layout_at(root: &Path, budget: Option<Duration>) -> Result<Layout, Error> {
    Ok(layout(&info(root, "", budget)?.relative_url))
}

/// `^/a/b%20c` → `["a", "b c"]`; `None` when a component is not a plain name once decoded.
fn components(relative_url: &str) -> Option<Vec<String>> {
    let rest = relative_url.strip_prefix("^/")?;
    if rest.is_empty() {
        return Some(Vec::new());
    }
    rest.split('/').map(|c| decoded(c).filter(|d| plain(d))).collect()
}

/// A path component svn may hold and a URL may carry: not empty, not `.` or `..`, no `/` or NUL.
fn plain(c: &str) -> bool {
    !c.is_empty() && c != "." && c != ".." && !c.contains(['/', '\0'])
}

/// A URL component with its `%XX` undone; `None` when an escape is broken or the bytes are no UTF-8.
fn decoded(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// **The one URL builder** (moai-0d0s.x27): the repository root as `svn info` printed it (already encoded — left as it
/// is) and each decoded component percent-encoded once, everything but `A–Z a–z 0–9 - . _ ~`. The peg `@<rev>` is put
/// on by the [`Arg`] that carries it, so a `@` in a name never reads as one. Takes only components that are
/// [`plain`]; anything else is no URL.
pub(crate) fn url(repos_root: &str, path: &[String]) -> Option<String> {
    let mut out = repos_root.trim_end_matches('/').to_owned();
    for c in path {
        if !plain(c) {
            return None;
        }
        out.push('/');
        for b in c.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    Some(out)
}

/// The project path of `url` below `base` — both as svn prints them, so each is decoded component by component before
/// they are compared and an encoding svn chose differently from [`url`] still matches. Every component below must be
/// [`plain`] once decoded; `None` when `url` is not strictly below `base` or a component is not. What `diff
/// --summarize` prints is **untrusted** like any svn path: the caller still checks the result as it checks the rest.
pub(crate) fn below(base: &str, url: &str) -> Option<String> {
    let split = |u: &str| u.trim_end_matches('/').split('/').map(decoded).collect::<Option<Vec<_>>>();
    let (base, url) = (split(base)?, split(url)?);
    let rest = url.strip_prefix(base.as_slice())?;
    (!rest.is_empty() && rest.iter().all(|c| plain(c))).then(|| rest.join("/"))
}

/// `/a/b` as `log` prints a repository path, split into plain components; `None` otherwise.
pub(crate) fn repo_path(text: &str) -> Option<Vec<String>> {
    let rest = text.strip_prefix('/')?;
    rest.split('/').map(|c| plain(c).then(|| c.to_owned())).collect()
}

/// Where a branch was copied from — the repository path and revision of the copy's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fork {
    pub path: Vec<String>,
    pub rev: u64,
}

/// How a branch root came to be: the revision that made it, and the copy it was made from — `None` for a root made
/// without one (`svn mkdir`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Created {
    pub rev: u64,
    pub from: Option<Fork>,
}

/// The first entry of `log --xml -v --stop-on-copy -r 1:<BASE> --limit 1 <branch root>` read as [`Created`]: only a path
/// that is the branch root **or above it**, added or replaced with `copyfrom-path`·`copyfrom-rev`, is the copy — the
/// branch root continues below the copied path, so its source is the copy's source with the rest joined on (the
/// longest such path wins). Other paths of the same revision are ignored. `None` when there is no entry, or a copy's
/// attributes do not read as a plain path and a revision older than the entry's.
pub(crate) fn created_of(text: &str, branch_root: &[String]) -> Option<Created> {
    let doc = xml::parse(text);
    let entry = doc.child("log")?.child("logentry")?;
    let rev: u64 = entry.attr("revision")?.trim().parse().ok().filter(|&r| r > 0)?;
    let mut best: Option<(usize, Fork)> = None;
    for p in entry.child("paths").map(|ps| ps.children("path").collect::<Vec<_>>()).unwrap_or_default() {
        let (Some(from), Some(from_rev)) = (p.attr("copyfrom-path"), p.attr("copyfrom-rev")) else { continue };
        if !matches!(p.attr("action"), Some("A" | "R")) {
            continue;
        }
        let Some(at) = repo_path(p.text().trim()) else { continue };
        if !branch_root.starts_with(&at) || best.as_ref().is_some_and(|(n, _)| *n >= at.len()) {
            continue;
        }
        let mut path = repo_path(from)?;
        path.extend_from_slice(&branch_root[at.len()..]);
        let from_rev: u64 = from_rev.trim().parse().ok().filter(|&r| r > 0 && r < rev)?;
        best = Some((at.len(), Fork { path, rev: from_rev }));
    }
    Some(Created { rev, from: best.map(|(_, f)| f) })
}

/// Where the branch root of `here` was copied from — `None` when it was made without a copy. Asked of the server once
/// per branch root and kept in the cache file's `forks` ([`cache`]); asked again only when the working copy's BASE is
/// older than the revision that made the root (the root was made anew under the same name). The cache's other keys
/// (`revs`·`through`) are written back as they were read.
pub(crate) fn fork_of(
    root: &Path,
    here: &Info,
    branch: &Branch,
    budget: Option<Duration>,
) -> Result<Option<Fork>, Error> {
    let ask = || {
        let url = url(&here.repos_root, &branch.root).ok_or_else(|| Error::Failed("no branch URL".into()))?;
        let args = [
            Arg::Flag("--xml"),
            Arg::Flag("-v"),
            Arg::Flag("--stop-on-copy"),
            Arg::Value("-r", format!("1:{}", here.revision)),
            Arg::Value("--limit", "1".into()),
            Arg::Url(&url, Some(here.revision)),
        ];
        let (out, cut) = run(root, Sub::Log, &args, LOG_LIMIT, budget)?;
        if cut {
            return Err(Error::Failed("svn log output exceeds its limit".into()));
        }
        created_of(&String::from_utf8_lossy(&out), &branch.root)
            .ok_or_else(|| Error::Failed("svn log gave no creation of the branch".into()))
    };
    Ok(created_within(cache::dir().as_deref(), here, &branch.root, ask)?.from)
}

/// [`fork_of`] with the cache directory and the server's answer given.
fn created_within(
    dir: Option<&Path>,
    here: &Info,
    branch_root: &[String],
    ask: impl FnOnce() -> Result<Created, Error>,
) -> Result<Created, Error> {
    let key = cache::fork_key(branch_root);
    let path = dir.and_then(|d| cache::path_in(d, &here.uuid));
    if let Some(p) = &path
        && let Some(known) = cache::read(p, &here.uuid).fork(&key)
        && known.rev <= here.revision
    {
        return Ok(known);
    }
    let created = ask()?;
    if let Some(p) = &path {
        // Read again just before writing, so a table filled meanwhile is kept. A failed write is no failure.
        let mut table = cache::read(p, &here.uuid);
        table.set_fork(&key, &created);
        let _ = cache::write(p, &table);
    }
    Ok(created)
}

/// A chunk of `log --xml` is cut here and asked again at half the range (moai-0d0s.4o8).
const LOG_LIMIT: usize = 32 * 1024 * 1024;

/// `(revision, message)` of each `<logentry>` — a revision the reader may not see has no `<msg>` and reads as empty.
fn log_entries(text: &str) -> Vec<(u64, String)> {
    let doc = xml::parse(text);
    let Some(log) = doc.child("log") else { return Vec::new() };
    log.children("logentry")
        .filter_map(|e| {
            let rev = e.attr("revision")?.trim().parse().ok()?;
            Some((rev, e.child("msg").map(xml::Element::text).unwrap_or_default()))
        })
        .collect()
}

/// The commits whose subject or `Refs:`·`Closes:`·`Fixes:` lines name `id`, newest first — what
/// [`crate::git::commits_within`] gives for Git, with `r<number>` in place of a hash (moai-0d0s.4o8).
///
/// The answer comes from the id → revision table ([`cache`]): read from the person's cache directory, brought up to
/// HEAD from the repository root's log at most once a minute. The working copy is read for its repository root and
/// UUID only; nothing in it or in `.moai` is written. `budget` bounds the whole read; without one the read stops at
/// [`SERVER_TIMEOUT`] all told (the person's "30 seconds for `moai diff`, keeping what was filled"), and every server
/// call keeps its own [`SERVER_TIMEOUT`] too. A table never completed answers nothing — the read is
/// [`Error::Filling`] or the reason it stopped — and a table completed before answers when the server cannot.
pub(crate) fn commits_within(
    root: &Path,
    id: &str,
    budget: Option<Duration>,
) -> Result<Vec<crate::git::Commit>, Error> {
    let text = crate::model::now();
    let now = match crate::model::parse_rfc3339(&text) {
        Some(_) => text,
        None => crate::model::format_rfc3339(
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        ),
    };
    let table = table_within(root, budget, cache::dir().as_deref(), &now, cache::CHUNK, SERVER_TIMEOUT)?;
    Ok(table.commits(id))
}

/// [`commits_within`]'s table, with its cache directory, clock, chunk size and server time given.
fn table_within(
    root: &Path,
    budget: Option<Duration>,
    dir: Option<&Path>,
    now: &str,
    chunk: u64,
    server: Duration,
) -> Result<cache::Table, Error> {
    let end = std::time::Instant::now() + budget.unwrap_or(server);
    if budget == Some(Duration::ZERO) {
        return Err(Error::Timeout);
    }
    let left = || end.saturating_duration_since(std::time::Instant::now());
    // With the caller's budget, a call past it is `timeout` and one past its server time `offline`. Without one, the
    // read's cap is each call's server time, so a stop is `offline` and [`cache::settle`] tells the cap apart.
    let call = |sub: Sub, args: &[Arg], limit: usize| match budget {
        Some(_) => bounded(command(root, sub, args), reaches_server(sub, args), limit, Some(left()), server),
        None => bounded(command(root, sub, args), reaches_server(sub, args), limit, None, server.min(left())),
    };
    let here = info(root, "", budget.map(|_| left()))?;
    let path = dir.and_then(|d| cache::path_in(d, &here.uuid));
    let mut table = path.as_deref().map_or_else(|| cache::Table::new(&here.uuid), |p| cache::read(p, &here.uuid));
    let seconds = crate::model::parse_rfc3339(now).unwrap_or(0);
    if table.fresh(seconds) {
        return Ok(table);
    }
    let save = |t: &cache::Table| {
        if let Some(p) = &path {
            // A table that cannot be kept is still the answer; the next read asks the server again.
            let _ = cache::write(p, t);
        }
    };
    let root_url = here.repos_root.as_str();
    let filled = (|| {
        let (out, cut) = call(Sub::Info, &[Arg::Flag("--xml"), Arg::Url(root_url, None)], 1024 * 1024)?;
        let head = (!cut)
            .then(|| info_of(&String::from_utf8_lossy(&out)))
            .flatten()
            .ok_or_else(|| Error::Failed("svn info gave no repository head".into()))?
            .revision;
        let fetch = |from: u64, to: u64| {
            let range = Arg::Value("-r", format!("{from}:{to}"));
            let (out, cut) = call(Sub::Log, &[Arg::Flag("--xml"), range, Arg::Url(root_url, None)], LOG_LIMIT)?;
            if cut {
                return Ok(cache::Fetched::Cut);
            }
            let entries = log_entries(&String::from_utf8_lossy(&out));
            Ok(cache::Fetched::Revs(entries.into_iter().filter_map(|(rev, msg)| cache::rev_of(rev, &msg)).collect()))
        };
        cache::fill(&mut table, head, chunk, now, fetch, &save)
    })();
    match filled {
        Ok(()) => Ok(table),
        Err(stop) => {
            // A complete table whose refresh failed waits its minute too — else every read waits the server out again.
            if table.complete() {
                table.tried_at = Some(now.to_owned());
                save(&table);
            }
            cache::settle(&table, stop, left().is_zero()).map(|()| table)
        }
    }
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

    thread_local! {
        static CONFIG: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    }

    pub(crate) fn config_override() -> Option<PathBuf> {
        CONFIG.with(|c| c.borrow().clone())
    }

    /// `f` 안에서 이 층이 `dir` 을 `--config-dir` 로 준다 — **이 스레드에서만**(moai-0d0s.s6g). 한 시험이 심은
    /// `diff-cmd` 가 같은 판의 다른 시험의 svn 에 닿지 않게. 시한을 주면 읽기가 다른 스레드에서 돌아 이것이 안 닿으니,
    /// 안에서는 시한 없이 부른다.
    pub(crate) fn with_config<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                CONFIG.with(|c| *c.borrow_mut() = None);
            }
        }
        CONFIG.with(|c| *c.borrow_mut() = Some(dir.to_path_buf()));
        let _reset = Reset;
        f()
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
        // 요약만 `--internal-diff` 없이 선다 — svn 이 함께 안 받는다. 속성은 그대로 안 본다.
        let summary = argv(&command(root, Sub::Diff, &[Arg::Summarize, Arg::Url("file:///r", Some(2))]));
        assert!(!summary.iter().any(|a| a == "--internal-diff"), "{summary:?}");
        for flag in ["--ignore-properties", "--summarize", "--xml", "--non-interactive"] {
            assert!(summary.iter().any(|a| a == flag), "{flag}: {summary:?}");
        }
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

    const T0: &str = "2026-11-18T00:00:00Z";
    fn at(secs: i64) -> String {
        crate::model::format_rfc3339(crate::model::parse_rfc3339(T0).expect("T0") + secs)
    }
    fn hashes(table: &cache::Table, id: &str) -> Vec<(String, bool)> {
        table.commits(id).into_iter().map(|c| (c.hash, c.tracker)).collect()
    }
    /// Every name under the working copy but `.svn` — a read must add none.
    fn tree(wc: &Path) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).expect("dir").flatten() {
                let p = e.path();
                if p.file_name().is_some_and(|n| n == ".svn") {
                    continue;
                }
                out.push(p.strip_prefix(base).unwrap_or(&p).to_string_lossy().into_owned());
                if p.is_dir() {
                    walk(&p, base, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(wc, wc, &mut out);
        out.sort();
        out
    }

    /// trunk with the six ways a message can name `moai-ab12`, r2..r7.
    fn named_repo(name: &str) -> (Repo, PathBuf) {
        let repo = Repo::new(name);
        let wc = repo.checkout("trunk", "wc");
        let messages = [
            "feat: one (moai-ab12)",
            "fix: two\n\nRefs: moai-ab12",
            "docs: prose\n\nthis sentence mentions moai-ab12 in passing",
            "child moai-ab12.xyz",
            "branch worktree-moai-ab12",
            "chore(tracker): moai-ab12 를 집는다",
        ];
        for (n, msg) in messages.iter().enumerate() {
            commit_files(&wc, &[("a.txt", &format!("{n}\n"))], msg);
        }
        (repo, wc)
    }

    #[test]
    fn the_revision_table_reads_subjects_and_trailers_newest_first_and_writes_only_the_cache() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = named_repo("svn-table");
        let cache_dir = Scratch::new("svn-table-cache");
        let before = (admin_snapshot(&wc), tree(&wc));
        let table = table_within(&wc, None, Some(&cache_dir), T0, 2, SERVER_TIMEOUT).expect("table");
        assert_eq!(
            hashes(&table, "moai-ab12"),
            [("r7".to_owned(), true), ("r3".to_owned(), false), ("r2".to_owned(), false)],
            "the subject, the Refs line and the tracker commit (marked); never prose, a child or a branch name"
        );
        assert_eq!(table.commits("moai-ab12")[2].subject, "feat: one (moai-ab12)");
        assert_eq!(hashes(&table, "moai-ab12.xyz"), [("r5".to_owned(), false)]);
        assert_eq!((table.through, table.complete()), (7, true));
        assert_eq!((admin_snapshot(&wc), tree(&wc)), before, "the working copy is only read");
        let here = info(&wc, "", None).expect("info");
        let file = cache_dir.join(format!("{}.json", here.uuid));
        assert_eq!(crate::scratch::names(&cache_dir), [format!("{}.json", here.uuid)]);
        assert_eq!(cache::read(&file, &here.uuid), table);
        // No cache directory known: the same answer, nothing written anywhere.
        let none = table_within(&wc, None, None, T0, 2, SERVER_TIMEOUT).expect("table");
        assert_eq!(none.commits("moai-ab12"), table.commits("moai-ab12"));
        assert_eq!(tree(&wc), before.1);
        // The crate's entry point reads the same, through the cache directory this thread is given.
        let other = Scratch::new("svn-table-entry");
        let got = cache::tests::with_dir(Some(&other), || commits_within(&wc, "moai-ab12", None)).expect("commits");
        assert_eq!(got, table.commits("moai-ab12"));
        assert!(other.join(format!("{}.json", here.uuid)).is_file());
        // A budget of nothing is a timeout, not a guess.
        assert!(matches!(table_within(&wc, Some(Duration::ZERO), None, T0, 2, SERVER_TIMEOUT), Err(Error::Timeout)));
    }

    #[test]
    fn the_table_asks_the_server_once_a_minute_and_reads_only_what_is_new() {
        if !svn_ready() {
            return;
        }
        let (_repo, wc) = named_repo("svn-poll");
        let cache_dir = Scratch::new("svn-poll-cache");
        let read = |now: &str| table_within(&wc, None, Some(&cache_dir), now, 2, SERVER_TIMEOUT).expect("table");
        assert_eq!(read(T0).through, 7);
        commit_files(&wc, &[("b.txt", "b\n")], "late (moai-ab12)");
        let cached = read(&at(30));
        assert_eq!((cached.through, cached.checked_at.as_deref()), (7, Some(T0)), "inside the minute: the file alone");
        let later = read(&at(61));
        assert_eq!(later.through, 8);
        assert_eq!(hashes(&later, "moai-ab12")[0], ("r8".to_owned(), false));
        assert_eq!(later.checked_at.as_deref(), Some(at(61).as_str()));
        // A check time ahead of the clock asks at once.
        commit_files(&wc, &[("c.txt", "c\n")], "later still (moai-ab12)");
        assert_eq!(read(&at(-3600)).through, 9);
    }

    #[test]
    fn a_server_out_of_reach_answers_from_a_complete_table_or_says_offline() {
        if !svn_ready() {
            return;
        }
        let (repo, wc) = named_repo("svn-offline");
        let cache_dir = Scratch::new("svn-offline-cache");
        let full = table_within(&wc, None, Some(&cache_dir), T0, 2, SERVER_TIMEOUT).expect("table");
        std::fs::remove_dir_all(repo.scratch.join("repo")).expect("drop the repository");
        let before = admin_snapshot(&wc);
        // Long past the minute, the server is asked and is gone: the complete table answers.
        let stale = table_within(&wc, None, Some(&cache_dir), &at(3600), 2, SERVER_TIMEOUT).expect("served");
        assert_eq!(stale.commits("moai-ab12"), full.commits("moai-ab12"));
        // The failed refresh waits its minute too: the file says when it was tried, and inside that minute nothing asks.
        let uuid = info(&wc, "", None).expect("info").uuid;
        let kept = cache::read(&cache_dir.join(format!("{uuid}.json")), &uuid);
        assert_eq!((kept.tried_at.as_deref(), kept.checked_at.as_deref()), (Some(at(3600).as_str()), Some(T0)));
        let seconds = |s: &str| crate::model::parse_rfc3339(s).unwrap();
        assert!(kept.fresh(seconds(&at(3630))) && !kept.fresh(seconds(&at(3660))));
        // No table to fall back on: offline, and no panic, no hang.
        let empty = Scratch::new("svn-offline-empty");
        let got = table_within(&wc, None, Some(&empty), T0, 2, SERVER_TIMEOUT);
        assert!(matches!(got, Err(Error::Offline(Some(_)))), "{got:?}");
        let got = table_within(&wc, None, None, T0, 2, SERVER_TIMEOUT);
        assert_eq!(got.map(|_| ()).map_err(|e| e.kind()), Err("offline"));
        assert_eq!(admin_snapshot(&wc), before);
    }

    fn branch(name: &str, root: &[&str], sub: &[&str]) -> Layout {
        let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        Layout::Branch(Branch { name: name.into(), root: owned(root), sub: owned(sub) })
    }

    #[test]
    fn the_convention_is_read_from_the_first_keyword() {
        for trunk in ["^/trunk", "^/trunk/sub", "^/proj/trunk", "^/proj/trunk/a/b", "^/a/trunk/branches/y"] {
            assert_eq!(layout(trunk), Layout::Trunk, "{trunk}");
        }
        assert_eq!(layout("^/branches/x"), branch("x", &["branches", "x"], &[]));
        assert_eq!(layout("^/branches/x/sub"), branch("x", &["branches", "x"], &["sub"]));
        let korean = branch("한글 x", &["proj", "branches", "한글 x"], &[]);
        assert_eq!(layout("^/proj/branches/한글 x"), korean);
        assert_eq!(layout("^/proj/branches/%ED%95%9C%EA%B8%80%20x"), korean, "svn prints it encoded");
        assert_eq!(layout("^/branches/x@%23%3F%25/s%20p"), branch("x@#?%", &["branches", "x@#?%"], &["s p"]));
        // The first keyword met from the root decides: a later one is a plain name.
        assert_eq!(layout("^/branches/trunk/tags"), branch("trunk", &["branches", "trunk"], &["tags"]));
        for unknown in [
            "^/tags/1.0",
            "^/tags/1.0/branches/x",
            "^/",
            "^/src",
            "^/branches",
            "^/proj/branches",
            "^/Trunk",
            "^/branches//x",
            "^/branches/%2E%2E/x",
            "^/branches/x%2Fy",
            "^/branches/%E",
            "^/branches/%FF",
            "/branches/x",
            "",
        ] {
            assert_eq!(layout(unknown), Layout::Unknown, "{unknown:?}");
        }
        assert_eq!(layout("^/branches/x").branch_name(), Some("x"));
        assert_eq!(layout("^/trunk").branch_name(), None);
    }

    #[test]
    fn a_url_encodes_each_component_once() {
        let parts = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            url("file:///r%20oot", &parts(&["branches", "한 x@#?%", "a.b-c_d~e"])).as_deref(),
            Some("file:///r%20oot/branches/%ED%95%9C%20x%40%23%3F%25/a.b-c_d~e"),
            "the root as svn printed it, each name once"
        );
        assert_eq!(url("file:///r/", &parts(&["trunk"])).as_deref(), Some("file:///r/trunk"));
        assert_eq!(url("file:///r", &[]).as_deref(), Some("file:///r"));
        for bad in [&["a", ".."][..], &["."], &[""], &["a/b"], &["a\0"]] {
            assert_eq!(url("file:///r", &parts(bad)), None, "{bad:?}");
        }
        // The peg goes on after the whole URL, so a name's `@` is never one.
        let encoded = url("file:///r", &parts(&["x@1"])).unwrap();
        let args = argv(&command(Path::new("."), Sub::Diff, &[Arg::Old(&encoded, 3), Arg::NewBase("")]));
        assert!(args.contains(&"--old=file:///r/x%401@3".to_owned()), "{args:?}");
        assert!(args.contains(&"--new=./@BASE".to_owned()), "{args:?}");
        assert!(!reaches_server(Sub::Status, &[Arg::NewBase("")]), "BASE is read from the working copy");
        assert_eq!(repo_path("/branches/한 x"), Some(parts(&["branches", "한 x"])));
        for bad in ["", "trunk", "/", "/a//b", "/a/../b", "/./a", "/a/"] {
            assert_eq!(repo_path(bad), None, "{bad:?}");
        }
    }

    fn log_of(rev: u64, paths: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<log>\n<logentry\n   revision=\"{rev}\">\n<author>a</author>\n<paths>\n{paths}</paths>\n<msg>m</msg>\n</logentry>\n</log>\n"
        )
    }

    #[test]
    fn the_creation_is_the_copy_of_the_branch_root_or_above() {
        let root = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let x = root(&["branches", "x"]);
        // Attributes in any order; another path copied in the same revision is not the branch's.
        let text = log_of(
            3,
            "<path kind=\"file\" copyfrom-rev=\"1\" copyfrom-path=\"/elsewhere\" action=\"A\">/tags/t</path>\n<path\n   text-mods=\"false\"\n   kind=\"dir\"\n   copyfrom-path=\"/trunk\"\n   copyfrom-rev=\"2\"\n   action=\"A\"\n   prop-mods=\"false\">/branches/x</path>\n",
        );
        let fork = Some(Fork { path: root(&["trunk"]), rev: 2 });
        assert_eq!(created_of(&text, &x), Some(Created { rev: 3, from: fork }));
        // Copied from another branch: that copy is the fork point, as it is.
        let text = log_of(5, "<path action=\"A\" copyfrom-path=\"/branches/y\" copyfrom-rev=\"4\">/branches/x</path>");
        assert_eq!(created_of(&text, &x).unwrap().from, Some(Fork { path: root(&["branches", "y"]), rev: 4 }));
        // A copy of a directory above: the rest of the branch root's path is joined on.
        let text = log_of(9, "<path action=\"A\" copyfrom-path=\"/old\" copyfrom-rev=\"8\">/proj</path>");
        let deep = root(&["proj", "branches", "x"]);
        assert_eq!(
            created_of(&text, &deep).unwrap().from,
            Some(Fork { path: root(&["old", "branches", "x"]), rev: 8 })
        );
        // `svn mkdir`: made, not copied.
        let text = log_of(8, "<path kind=\"dir\" action=\"A\">/branches/x</path>");
        assert_eq!(created_of(&text, &x), Some(Created { rev: 8, from: None }));
        // A copy below the root, a sibling's copy or a modification is not the root's creation.
        let text = log_of(
            6,
            "<path action=\"A\" copyfrom-path=\"/trunk/a\" copyfrom-rev=\"2\">/branches/x/a</path><path action=\"A\" copyfrom-path=\"/trunk\" copyfrom-rev=\"2\">/branches/xy</path><path action=\"M\" copyfrom-path=\"/trunk\" copyfrom-rev=\"2\">/branches/x</path>",
        );
        assert_eq!(created_of(&text, &x), Some(Created { rev: 6, from: None }));
        // What svn never prints is no answer: a copy source that is no plain path, a revision not older than the copy.
        for bad in [
            log_of(3, "<path action=\"A\" copyfrom-path=\"/a/../b\" copyfrom-rev=\"2\">/branches/x</path>"),
            log_of(3, "<path action=\"A\" copyfrom-path=\"trunk\" copyfrom-rev=\"2\">/branches/x</path>"),
            log_of(3, "<path action=\"A\" copyfrom-path=\"/trunk\" copyfrom-rev=\"3\">/branches/x</path>"),
            log_of(3, "<path action=\"A\" copyfrom-path=\"/trunk\" copyfrom-rev=\"-c\">/branches/x</path>"),
            log_of(0, ""),
            "<log></log>".to_owned(),
            String::new(),
        ] {
            assert_eq!(created_of(&bad, &x), None, "{bad}");
        }
    }

    fn here_at(uuid: &str, revision: u64) -> Info {
        Info {
            url: "file:///r/branches/x".into(),
            relative_url: "^/branches/x".into(),
            repos_root: "file:///r".into(),
            uuid: uuid.into(),
            revision,
        }
    }

    #[test]
    fn a_branch_root_is_asked_once_and_again_only_when_made_after_base() {
        const UUID: &str = "0f6c1c1e-2a5d-4b8e-9a43-1f0c3d2e5b6a";
        let dir = Scratch::new("svn-fork-cache");
        let x: Vec<String> = vec!["branches".into(), "x".into()];
        let asked = std::cell::Cell::new(0);
        let answer = |rev: u64, from: u64| {
            let asked = &asked;
            move || {
                asked.set(asked.get() + 1);
                Ok(Created { rev, from: Some(Fork { path: vec!["trunk".into()], rev: from }) })
            }
        };
        let first = created_within(Some(&dir), &here_at(UUID, 7), &x, answer(3, 2)).unwrap();
        assert_eq!((first.rev, asked.get()), (3, 1));
        let again = created_within(Some(&dir), &here_at(UUID, 9), &x, answer(3, 2)).unwrap();
        assert_eq!((again, asked.get()), (first.clone(), 1), "the second read is the cache's");
        // The root made anew after this working copy's BASE: asked again, and kept.
        let old = || {
            asked.set(asked.get() + 1);
            Ok(Created { rev: 1, from: None })
        };
        assert_eq!(created_within(Some(&dir), &here_at(UUID, 2), &x, old).unwrap(), Created { rev: 1, from: None });
        assert_eq!(asked.get(), 2);
        assert_eq!(created_within(Some(&dir), &here_at(UUID, 2), &x, answer(9, 8)).unwrap().rev, 1);
        assert_eq!(asked.get(), 2, "the root made anew is kept too");
        // A server that cannot answer leaves the cache as it was and says why.
        let y: Vec<String> = vec!["branches".into(), "y".into()];
        let gone = || Err(Error::Offline(None));
        assert!(matches!(created_within(Some(&dir), &here_at(UUID, 9), &y, gone), Err(Error::Offline(None))));
        let file = cache::path_in(&dir, UUID).unwrap();
        assert!(cache::read(&file, UUID).fork("/branches/y").is_none());
        // No cache directory: asked every time, nothing written.
        let none = Scratch::new("svn-fork-nocache");
        let before = asked.get();
        created_within(None, &here_at(UUID, 9), &x, answer(3, 2)).unwrap();
        created_within(None, &here_at(UUID, 9), &x, answer(3, 2)).unwrap();
        assert_eq!(asked.get(), before + 2);
        assert!(crate::scratch::names(&none).is_empty());
    }
}
