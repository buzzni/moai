//! **저장소가 든 파일을 읽는 자**(moai-itsu). 스냅샷(`.moai/issues.jsonl`)·설정(`.moai/config.toml`)·저널
//! (`.moai/journal/*.jsonl`)은 받은 저장소가 커밋한 링크일 수 있다 — 0.1.6(moai-karj)이 저널에만 건 자를
//! 셋에 같은 자로 편다. `issues.jsonl -> /proc/self/pagemap` 하나로 모든 명령이 메모리를 다 쓰던 자리다.
//!
//! 자는 둘이고, 둘 다 여기 하나씩이다. 견줄 뿌리는 [`Home`] 하나로만 든다.
//!
//! - [`place`] — 링크를 푼 자리가 **체크아웃 안이고 `.git/` 밖**인가. 쓰기(`store::target_of`)가 이미
//!   거절하던 자리를 읽기도 같은 자로 잰다. procfs·sysfs·장치 파일은 늘 체크아웃 밖이고, git 이 실을 수
//!   있는 것은 보통 파일과 링크뿐이라 안에 선 보통 파일은 디스크에 있는 만큼만 읽힌다
//! - [`read`] — 연 손잡이가 **보통 파일이라 답할 때만, 그 손잡이가 댄 크기까지만** 읽는다. 이름으로 잰 뒤
//!   읽기 전에 그 이름이 갈리는 틈을 손잡이가 닫는다. `O_NONBLOCK` 으로 열어 FIFO 앞에서 쓰는 쪽을
//!   기다리며 멈추지 않는다
//!
//! 저장소 락([`lock`], moai-sn57)도 이 둘 위에 선다 — 디렉터리는 [`place`] 로 재고, 끝 조각은 [`read`] 처럼
//! 연 손잡이로 잰다. 하나가 더 엄하다: 락은 안을 가리키는 링크도 안 따른다.
//!
//! **거절은 자료다**([`Unheld`]) — 글은 부르는 쪽이 고른 말로 [`said`] 가 짓는다. 말을 못 고르는 자리
//! (제 트래커를 지은 뒤에 링크가 갈린 경우의 [`crate::store::Repo::read`])는 말 없는 꼴 [`spelled`] 를 쓴다.

use std::path::{Path, PathBuf};

/// 견줄 뿌리 — **푼 자리로만 선다**([`Home::of`]). [`place`]·[`check`]·[`read_inside`]·[`open_inside`]·[`lock`] 이
/// 모두 이것을 받는다.
///
/// 한때는 맨 `&Path` 를 받아, [`place`]·[`check`] 는 "이미 푼 뿌리" 를 바라고 [`read_inside`] 는 받은 철자를
/// 제가 풀었다. 같은 꼴에 거꾸로인 전제가 글로만 갈려, 푼 적 없는 뿌리를 [`check`] 에 넘기면 컴파일은 되고
/// 링크를 지난 뿌리(손으로 적은 등록 경로, macOS 의 `/tmp`)에서는 멀쩡한 스냅샷이 "밖" 으로 읽혀 모든 명령이
/// 섰다(리뷰 moai-itsu.n8z). 푸는 자리를 여기 하나로 두면 그 꼴이 아예 안 지어진다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Home(PathBuf);

impl Home {
    /// 트래커의 뿌리(`.moai` 를 든 디렉터리)를 푼다 — 못 풀면 받은 철자다([`crate::path::real`]).
    pub(crate) fn of(root: &Path) -> Home {
        Home(crate::path::real(root))
    }
}

/// 저장소가 든 파일을 **안 읽는 까닭**, 락이면 **안 잡는 까닭**([`lock`]) — 말이 아니라 자료다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unheld {
    /// 링크가 체크아웃 밖으로 풀린다 — 풀린 자리와 견준 체크아웃(푼 철자).
    Outside { to: PathBuf, home: PathBuf },
    /// 링크가 체크아웃 안의 git 자리(`.git/`)로 풀린다 — 풀린 자리. 팩 파일 하나가 몇 GiB 라도 통째로 담긴다.
    IntoGit { to: PathBuf },
    /// 보통 파일이 아니다 — FIFO·장치·디렉터리·소켓. 링크가 아니라 그 자리에 바로 선 것도 든다.
    NotAFile,
    /// 링크다 — **저장소 락만 이 까닭을 낸다**([`lock`]). 읽는 자리는 안을 가리키는 링크를 따르지만 락은
    /// 어디를 가리키든 안 따른다(moai-sn57, 2026-10-02 사용자 결정).
    Link,
}

/// [`read`]·[`read_inside`]·[`open_inside`]·[`lock`] 이 진 까닭 — **안 읽기로(안 잡기로) 한 것과 io 가 진 것을
/// 가른다.** 앞의 것은 손으로 링크나 파일을 고칠 일이고(스냅샷·설정·락이면 `--json` 의 `broken`, 저널이면
/// `journal_error` 의 `outside`·`failed`), 뒤의 것은 운영체제가 낸 말 그대로다.
#[derive(Debug)]
pub enum Fell {
    Unheld(Unheld),
    Io(std::io::Error),
}

/// 받은 자리를 푼 자리 — **체크아웃 안이고 `.git/` 밖일 때만** 낸다.
///
/// - **못 푸는 링크도 잰다** — 끝이 없는 링크(`issues.jsonl -> <밖>/missing.jsonl`, 받은 사람의 기계에 없는
///   `/home/<남>/…`)와 끝에 못 닿는 링크(`-> /root/x`)는 통째로 풀리지 않는다. 그것을 재지 않던 판은 그
///   스냅샷을 "없음" 으로 읽어 보드가 "이슈 0개" 를 그렸고, 저널은 `outside` 대신 `failed`·`permission` 으로
///   섰다(리뷰 moai-itsu.n8z). 쓰기(`store::target_of`)와 같은 자로 끝 조각의 링크 사슬을 따라가 그 끝의
///   디렉터리를 푼다 — 그 디렉터리도 없으면 아직 있는 조상까지 푼다([`crate::path::real_prefix`])
/// - **링크가 아닌 없는 자리는 받은 철자 그대로다** — 부르는 쪽이 `home` 아래에 지은 철자라 잴 것이 없고,
///   읽는 쪽이 `NotFound` 로 넘긴다. 고리처럼 끝내 못 푸는 링크도 그대로 나가 읽는 쪽이 그 io 실패를 센다
/// - **안을 가리키는 링크는 그대로 따른다** — 메일을 바꾼 사람의 `<옛 메일>.jsonl -> <새 메일>.jsonl`
///   (moai-p9mq), `issues.jsonl -> data/issues.jsonl` 이 그렇다
/// - **끝 이름도 센다** — 딸린 워크트리의 `.git` 은 디렉터리가 아니라 `gitdir:` 한 줄짜리 파일이다
///   (`store::target_of` 와 같은 자)
pub(crate) fn place(p: &Path, home: &Home) -> Result<PathBuf, Unheld> {
    let Some(real) = landing(p) else { return Ok(p.to_path_buf()) };
    match real.strip_prefix(&home.0) {
        Err(_) => Err(Unheld::Outside { to: real, home: home.0.clone() }),
        Ok(rest) if into_git(rest) => Err(Unheld::IntoGit { to: real }),
        Ok(_) => Ok(real),
    }
}

/// **디렉터리**가 끝내 닿는 자리 — [`place`] 이되 **아직 없는 디렉터리는 있는 가장 깊은 조상으로 잰다**
/// (moai-kxkw.7ky). 거기서 짓고 쓸 자리라 푼 자리를 낸다 — 없는 조각은 그 뒤에 그대로 붙는다.
///
/// [`place`] 는 링크가 아닌 없는 자리를 받은 철자 그대로 내보낸다 — 읽는 쪽이 `NotFound` 로 넘기니 그것으로 됐다. 짓는
/// 쪽에서는 그것이 구멍이다: `.moai -> <밖>` 을 커밋한 저장소에서 `.moai/archive` 는 아직 없는 링크 아닌 자리라
/// 그대로 지나고, `create_dir_all` 은 그 링크를 따라 밖에 디렉터리를 짓는다. 그래서 있는 조상을 [`place`] 로 잰다.
/// 없는 나머지는 지을 때 보통 디렉터리로 선다.
///
/// **푼 자리가 아직 링크를 거치면 그 링크를 다시 잰다**(리뷰 moai-kxkw.k2f) — [`place`] 는 끝 없는 다른 링크를 거쳐 가는
/// 링크를 그 끝 없는 조각을 없는 디렉터리로 쳐서 "안" 으로 읽고, 끝이 없고 그 끝의 디렉터리도 없는 채 `..` 을 든 링크는 철자
/// 그대로 낸다. 둘 다 푼 자리의 있는 가장 깊은 조상이 링크로 남는다. 그 자리는 지금은 안 지어지지만(`mkdir` 은 끝 없는
/// 링크를 안 따른다) 잰 뒤에 그 끝이 생기면 밖에 지어진다 — 잰 사이를 노려 밖의 끝을 지었다 지우는 것만으로 `hello` 가 밖에
/// 장을 썼다. 그래서 남은 링크를 사슬의 끝까지 [`place`] 로 다시 재고, 끝내 못 푸는 링크(`..` 너머가 없다)는 어디에 닿을지
/// 모르니 거절한다. 지금 지어지는 꼴은 하나도 안 진다 — 끝 없는 안 링크는 끝의 디렉터리로 풀어 그 밑에 짓는다.
///
/// 아카이브([`crate::archive`])가 디렉터리를 짓고 열기 전에 이 자로 잰다.
pub(crate) fn place_dir(d: &Path, home: &Home) -> Result<PathBuf, Unheld> {
    let Some(head) = d.ancestors().find(|a| std::fs::symlink_metadata(a).is_ok()) else { return Ok(d.to_path_buf()) };
    let mut out = beneath(place(head, home)?, d, head);
    // 사슬의 깊이는 리눅스가 경로 하나를 풀며 따라가는 링크 수(`MAXSYMLINKS`)까지다 — `path::follow_links` 와 같은 끝.
    for _ in 0..=40 {
        let deepest = out.ancestors().find_map(|a| std::fs::symlink_metadata(a).ok().map(|m| (a.to_path_buf(), m)));
        let Some((link, _)) = deepest.filter(|(_, m)| m.file_type().is_symlink()) else { return Ok(out) };
        let real = place(&link, home)?;
        if real == link {
            // 고리처럼 끝내 못 따라가는 링크는 그대로 낸다 — 어디로도 안 풀려, 지을 때 운영체제가 그 말(`ELOOP`)로 진다.
            let Ok(end) = crate::path::follow_links(&link) else { return Ok(out) };
            return Err(Unheld::Outside { to: beneath(crate::path::lexical(&end), &out, &link), home: home.0.clone() });
        }
        out = beneath(real, &out, &link);
    }
    Ok(out)
}

/// `d` 의 조상 `head` 를 `real` 로 바꿔 단 자리 — 남은 조각이 비면 붙이지 않는다(`path::real_prefix` 와 같은 까닭).
fn beneath(real: PathBuf, d: &Path, head: &Path) -> PathBuf {
    // **떼어 내기는 실패하지 않는다** — `head` 는 `d` 의 조상이다(`path::real_prefix` 와 같은 자리).
    let rest = d.strip_prefix(head).expect("조상에서 떼어 낸다");
    if rest.as_os_str().is_empty() { real } else { real.join(rest) }
}

/// 체크아웃 아래의 자리 `rest` 가 **git 의 자리(`.git/`)에 드는가** — 어느 조각이든 `.git` 이면 든다. 끝 이름도
/// 센다 — 딸린 워크트리의 `.git` 은 디렉터리가 아니라 `gitdir:` 한 줄짜리 파일이다. 읽기([`place`])·쓰기
/// (`store::resolve`)·규칙 줄(`cmd::init::inside`)이 이 하나로 잰다(리뷰 moai-x0o7.52k) — 셋이 저마다 적던 판은
/// 한쪽을 고치는 날 나머지가 옛 답을 냈다.
///
/// **대소문자를 안 가린다** — 대소문자를 안 가르는 볼륨(macOS 의 기본 APFS)에서는 `.GIT` 도 그 자리다. git 도 트리
/// 안의 그런 조각을 대소문자 없이 거절하므로(`verify_dotfile`) 받은 저장소가 담아 올 자리를 잘못 막을 일은 없다.
pub(crate) fn into_git(rest: &Path) -> bool {
    rest.components().any(|c| c.as_os_str().eq_ignore_ascii_case(".git"))
}

/// 받은 자리가 끝내 닿는 자리 — 통째로 풀리면 그 자리고, 못 풀리는 링크면 끝 조각의 사슬을 따라가 그 끝의
/// 디렉터리를 푼 자리다(`store::target_of` 와 같은 풀이). 링크가 아니거나 끝내 못 푸는 자리는 `None` 이다.
///
/// **`..` 이 남은 끝은 조상으로 안 푼다** — 아직 없는 디렉터리 너머의 `..` 은 링크를 푼 뒤의 뜻이 달라
/// 진다([`crate::path::real_prefix`] 의 전제). 그런 끝은 재지 않고 넘긴다.
fn landing(p: &Path) -> Option<PathBuf> {
    if let Ok(real) = std::fs::canonicalize(p) {
        return Some(real);
    }
    if !std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
        return None;
    }
    let end = crate::path::follow_links(p).ok()?;
    match std::fs::canonicalize(crate::path::dir_of(&end)) {
        Ok(dir) => Some(end.file_name().map_or_else(|| dir.clone(), |name| dir.join(name))),
        Err(_) if end.is_absolute() && !end.components().any(|c| c == std::path::Component::ParentDir) => {
            Some(crate::path::real_prefix(&end))
        }
        Err(_) => None,
    }
}

/// **읽기 전에 재기만 한다** — [`place`] 와, 그 자리가 보통 파일인가(`stat`, FIFO 앞에서도 안 멈춘다). 없는
/// 자리와 못 잰 자리는 지나간다 — 읽는 쪽이 그 말로 넘기거나 멈춘다. 말을 고를 수 있는 자리(`store::Repo` 를
/// 짓는 길)가 읽기보다 먼저 물어, 거절을 고른 말로 낸다.
pub(crate) fn check(p: &Path, home: &Home) -> Result<(), Unheld> {
    let real = place(p, home)?;
    match std::fs::metadata(&real) {
        Ok(m) if !m.is_file() => Err(Unheld::NotAFile),
        _ => Ok(()),
    }
}

/// 파일 하나를 **연 손잡이가 댄 크기까지만** 읽는다. 없는 파일은 io 의 `NotFound` 그대로다 — 없는 것을 넘길지
/// 멈출지는 부르는 쪽의 일이고(스냅샷은 넘기고 설정은 멈춘다), 운영체제가 낸 말도 그대로 남는다.
///
/// **보통 파일이 아니면 안 읽는다**([`Unheld::NotAFile`]) — 재는 것은 연 손잡이의 `fstat` 이다. 이름으로
/// 잰 뒤 연 사이에 그 이름이 FIFO 나 장치로 갈려도 여기서 걸린다. 크기도 같은 손잡이에서 재므로,
/// `stat` 이 크기 0 이라 답하면서 글을 끝없이 내는 procfs 파일은 빈 것으로 읽힌다. 그 크기만큼 미리 잡아
/// 두고 읽는다 — `fs::read_to_string` 이 하던 대로다. 안 잡으면 큰 스냅샷이 서른두 바이트부터 곱절로 자라며
/// 열 번 넘게 나눠 읽혔다(리뷰 moai-itsu.n8z).
///
/// **링크의 끝이 어디인지는 안 잰다** — 그것은 [`place`] 의 일이고, 둘을 함께 거는 길이 [`read_inside`] 다.
/// 이것만 따로 부르는 자리는 그 앞에서 이미 잰 쪽이다(`store::Repo::journal_bytes` — `store::Repo::journal_files`
/// 가 이름마다 [`place`] 로 잰 뒤다).
pub(crate) fn read(p: &Path) -> Result<Vec<u8>, Fell> {
    use std::io::Read;
    let (f, len) = opened(p)?;
    let mut out = Vec::new();
    out.try_reserve_exact(usize::try_from(len).unwrap_or(usize::MAX))
        .map_err(|_| Fell::Io(std::io::ErrorKind::OutOfMemory.into()))?;
    f.take(len).read_to_end(&mut out).map_err(Fell::Io)?;
    Ok(out)
}

/// [`place`] 로 재고 [`read`] 로 읽어 글로 낸다 — 저장소가 든 파일 하나를 그 체크아웃(`home`) 안에서만.
/// **푼 자리를 연다** — 받은 철자를 다시 열면 잰 뒤 바뀐 링크를 따라간다(`store::append_inside` 와 같은 까닭).
///
/// UTF-8 이 아니면 `fs::read_to_string` 과 같은 io 실패다(`InvalidData`, 같은 말) — 그 자리를 이것으로 바꿔도
/// 사람이 보는 말이 안 바뀐다.
pub(crate) fn read_inside(p: &Path, home: &Home) -> Result<String, Fell> {
    utf8(read_bytes_inside(p, home)?).map_err(Fell::Io)
}

/// [`read_inside`] 의 바이트 판 — 같은 자리를 같은 자로 재고 읽되 글로 풀지 않는다. 아카이브의 id 를 예약하는 훑기가
/// UTF-8 이 아닌 파일에서도 그 안의 id(ASCII)를 건지려고 쓴다(moai-bth3 리뷰).
pub(crate) fn read_bytes_inside(p: &Path, home: &Home) -> Result<Vec<u8>, Fell> {
    let real = place(p, home).map_err(Fell::Unheld)?;
    read(&real)
}

/// 읽은 바이트를 글로 — UTF-8 이 아니면 `fs::read_to_string` 과 같은 io 실패다(`InvalidData`, 같은 말).
/// **그 말을 짓는 자리는 여기 하나다** — [`read_inside`] 와, 크기 상한을 제 손잡이로 재고 읽는 위키 페이지
/// (`wiki::read`)가 함께 쓴다. 저마다 적으면 같은 처지가 자리마다 다른 말로 선다.
pub(crate) fn utf8(bytes: Vec<u8>) -> std::io::Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// [`place`] 로 재고 **푼 자리를** 막히지 않게 열어, 손잡이가 보통 파일이라 답할 때만 낸다 — 읽지는 않는다.
/// 옆 워크트리의 스냅샷을 열어만 보는 값싼 문(`worktree::unreadable_snapshot`)이 쓴다. 이름으로 잰 뒤 받은
/// 철자를 다시 열면 그 사이에 바뀐 링크를 따라가므로, [`read_inside`] 와 같은 자리를 연다.
pub(crate) fn open_inside(p: &Path, home: &Home) -> Result<std::fs::File, Fell> {
    let real = place(p, home).map_err(Fell::Unheld)?;
    opened(&real).map(|(f, _)| f)
}

/// 막히지 않게 열어 **보통 파일일 때만** 손잡이와 그 크기를 낸다 — [`read`] 와 [`open_inside`] 의 몸통.
///
/// **소켓은 `open` 에서 진다**(`ENXIO`) — 손잡이의 `fstat` 까지 못 와, 연 것이 실패한 자리만 이름으로 다시
/// 잰다. 안 재면 소켓인 설정이 운영체제의 말과 `error` 로 서고, 같은 자리의 스냅샷은 보통 파일이 아니라는
/// 말과 `broken` 으로 서서 한 처지가 두 코드로 갈렸다(리뷰 moai-itsu.n8z). 없는 자리는 `NotFound` 그대로다.
fn opened(p: &Path) -> Result<(std::fs::File, u64), Fell> {
    let f = match open(p) {
        Ok(f) => f,
        Err(e) if e.kind() != std::io::ErrorKind::NotFound && std::fs::metadata(p).is_ok_and(|m| !m.is_file()) => {
            return Err(Fell::Unheld(Unheld::NotAFile));
        }
        Err(e) => return Err(Fell::Io(e)),
    };
    let m = f.metadata().map_err(Fell::Io)?;
    if !m.is_file() {
        return Err(Fell::Unheld(Unheld::NotAFile));
    }
    Ok((f, m.len()))
}

/// **저장소 락을 연다**(moai-sn57) — `.moai/lock` 과, 링크 너머 스냅샷 곁의 락(`store::Lock::inside`). 받은
/// 저장소가 커밋할 수 있는 자리라 읽는 자리와 같은 자로 재고, 하나를 더 건다.
///
/// - **디렉터리는 [`place`] 로 잰다** — 체크아웃 안이고 `.git/` 밖일 때만 그 푼 자리에 짓는다. `.moai` 가
///   밖을 가리키면 여는 길(`store::Repo::rooted`)이 대개 설정에서 먼저 멈추지만, 연 뒤에 `.moai` 가 갈리거나(떠
///   있는 탐색기가 받은 `git pull`) 스냅샷이 아직 없고 설정이 안으로 돌아오는 링크면 아무도 안 재어, 락 파일
///   하나가 체크아웃 밖에 섰다
/// - **끝 조각은 링크를 아예 안 따른다**(`O_NOFOLLOW`, [`Unheld::Link`]) — 안을 가리켜도 그렇다. 커밋된
///   `-> /proc/self/fd/2` 는 프로세스마다 제 stderr 를 잠가, 동시 `add` 스물넷이 다 0 으로 끝나고 셋에서
///   다섯만 남았다. `-> ../.git/index.lock` 이면 그 뒤의 git 커밋이 다 졌다. 안을 가리키는 `-> issues.jsonl`
///   도 쓰기마다 `rename` 으로 갈리는 아이노드를 잠가 두 쓰는 쪽이 서로 다른 파일을 쥔다
/// - **보통 파일일 때만** 손잡이를 낸다 — 연 손잡이의 `fstat` 으로 잰다. 막히지 않게 열어([`unblocked`])
///   FIFO 앞에서도 안 멈춘다
///
/// 흔한 길에 더해진 것은 디렉터리를 푸는 것과 `fstat` 이다 — 둘 다 아무것도 안 연다. 열기가 진 뒤에만 그
/// 자리를 한 번 더 잰다(`lstat`).
pub(crate) fn lock(p: &Path, home: &Home) -> Result<std::fs::File, Fell> {
    let name = p.file_name().ok_or(Fell::Unheld(Unheld::NotAFile))?;
    // 디렉터리의 거절은 그 안의 락 자리로 댄다 — 사람이 보는 것은 디렉터리가 아니라 락이 설 자리다.
    let beneath = |why| match why {
        Unheld::Outside { to, home } => Unheld::Outside { to: to.join(name), home },
        Unheld::IntoGit { to } => Unheld::IntoGit { to: to.join(name) },
        other => other,
    };
    let at = place(crate::path::dir_of(p), home).map_err(|why| Fell::Unheld(beneath(why)))?.join(name);
    let mut o = std::fs::OpenOptions::new();
    o.create(true).write(true).truncate(false);
    #[cfg(not(unix))]
    if std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Fell::Unheld(Unheld::Link));
    }
    let f = match unblocked(&mut o, NOFOLLOW, &at) {
        Ok(f) => f,
        // 링크면 `ELOOP`(FreeBSD 는 `EMLINK`), FIFO 는 `ENXIO`, 디렉터리는 `EISDIR` 다 — 낱말 대신 자리를 잰다.
        Err(e) => {
            return Err(match std::fs::symlink_metadata(&at) {
                Ok(m) if m.file_type().is_symlink() => Fell::Unheld(Unheld::Link),
                Ok(m) if !m.is_file() => Fell::Unheld(Unheld::NotAFile),
                _ => Fell::Io(e),
            });
        }
    };
    match f.metadata() {
        Ok(m) if m.is_file() => Ok(f),
        Ok(_) => Err(Fell::Unheld(Unheld::NotAFile)),
        Err(e) => Err(Fell::Io(e)),
    }
}

/// [`lock`] 이 [`unblocked`] 에 더 거는 것 — 끝 조각의 링크를 안 따른다.
#[cfg(unix)]
const NOFOLLOW: i32 = libc::O_NOFOLLOW;
#[cfg(not(unix))]
const NOFOLLOW: i32 = 0;

/// 남이 리스를 쥔 파일을 다시 열어 볼 때까지 기다리는 끝 — 리눅스가 리스를 걷는 기본 시간
/// (`/proc/sys/fs/lease-break-time`, 45초)보다 **한 초 길다**. 막히는 `open` 이 그만큼 기다리던 자리인데, 꼭
/// 같은 때에 그만두면 마지막 열기가 커널이 리스를 걷는 순간과 겨뤄, 막히는 `open` 이 늘 열던 파일을 가끔 못
/// 열었다 — 리스를 안 놓는 쪽 앞에서 서른두 번에 두 번이었다(리뷰 moai-sn57.kq4).
const LEASE_BREAK: std::time::Duration = std::time::Duration::from_secs(46);

/// 읽기 전용으로, **막히지 않게** 연다. FIFO 를 그냥 열면 쓰는 쪽이 올 때까지 `open` 에서 영영 멈춘다 —
/// `O_NONBLOCK` 이면 FIFO 는 바로 열려 [`opened`] 의 `fstat` 에서 걸린다. 리스 앞에서 쉬며 다시 여는 것은
/// [`unblocked`] 의 몫이다.
fn open(p: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    unblocked(&mut o, 0, p)
}

/// **막히지 않게 여는 자리는 여기 하나다** — [`open`] 의 읽기와 [`lock`] 의 쓰기가 함께 지난다. `flags` 는
/// `O_NONBLOCK` 위에 더 걸 것이다(unix 밖에서는 안 쓴다).
///
/// **보통 파일에는 아무것도 안 바꾸지만, 여는 것은 하나 바꾼다** — 남이 리스(`F_SETLEASE`)를 쥔 파일(커널
/// oplock 을 켠 Samba, 위임을 준 knfsd)은 리스가 풀리기를 기다리지 않고 `EWOULDBLOCK` 으로 바로 진다. 읽기
/// 열기는 쓰기 리스에, 쓰기로 여는 락은 읽기 리스에도 그렇게 진다. 막히는 `open` 은 리스가 걷힐 때까지
/// 기다렸다가 열었으므로, 그 실패만 잠깐씩 쉬며 [`LEASE_BREAK`] 까지 다시 연다(리뷰 moai-itsu.n8z). 진 열기가
/// 이미 리스를 거두라고 알렸으니 다시 열면 곧 열린다. FIFO 의 열기는 이 갈래로 안 온다.
fn unblocked(o: &mut std::fs::OpenOptions, flags: i32, p: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NONBLOCK | flags);
    }
    #[cfg(not(unix))]
    let _ = flags;
    let until = std::time::Instant::now() + LEASE_BREAK;
    loop {
        match o.open(p) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            got => return got,
        }
    }
}

/// 안 읽은 까닭 한 토막 — 고른 말로. 앞에 자리를 다는 것은 부르는 쪽이다(스냅샷·설정·락은 `<자리>: <까닭>` —
/// 스냅샷과 락은 [`refused`], 설정은 `view::config_refused` 가 같은 꼴로 단다. 저널은 `warn.unread_journal`).
pub fn said(lang: crate::i18n::Lang, why: &Unheld) -> String {
    use crate::i18n::{fill, say};
    // 링크 글은 받은 저장소가 커밋한 것이라 제어 문자를 걷는다 — ESC 가 든 링크 하나가 화면을 다시 칠한다.
    let shown = |p: &Path| crate::text::one_line(&p.display().to_string());
    match why {
        Unheld::Outside { to, home } => {
            fill(say(lang, "held.outside"), &[("to", &shown(to)), ("home", &home.display().to_string())])
        }
        Unheld::IntoGit { to } => fill(say(lang, "held.into_git"), &[("to", &shown(to))]),
        Unheld::NotAFile => say(lang, "held.not_a_file").to_string(),
        Unheld::Link => say(lang, "held.link").to_string(),
    }
}

/// 안 읽어 거절한 한 줄 — `<자리>: <까닭>`. 설정의 거절(`view::config_refused`)과 **같은 꼴이다** — 한
/// 거절을 두 모양으로 대면 같은 처지를 두 일로 읽는다. "이 명령은 여기서 멈춘다" 같은 말은 안 붙인다: 이
/// 줄은 그 저장소를 못 연 채 나머지를 계속 그리는 자리(밖의 한눈 보기, `project ls`, `prime`)에도 선다
/// (리뷰 moai-itsu.n8z). 락을 못 잡은 거절([`lock`])도 이 꼴에 "아무것도 안 바뀌었다" 만 덧붙인다.
///
/// **자리도 제어 문자를 걷는다** — 너머의 락(`store::Repo::far_lock`)은 커밋된 링크 글을 이어 붙인 철자라
/// 받은 저장소가 지은 디렉터리 이름이 그대로 든다(리뷰 moai-sn57.kq4).
pub fn refused(lang: crate::i18n::Lang, at: &Path, why: &Unheld) -> String {
    format!("{}: {}", crate::text::one_line(&at.display().to_string()), said(lang, why))
}

/// 말 없는 꼴 — `<자리> -> <풀린 자리>`. **말을 못 고르는 자리가 쓴다** — 제 트래커를 지은 뒤에 링크가 갈린
/// 경우의 [`crate::store::Repo::read`] 다(그 앞의 흔한 경우는 `store::Repo` 를 짓는 길이 고른 말로 먼저 섰고,
/// 옆 워크트리의 것은 `worktree::Trouble::Unheld` 로 자료째 나가 고른 말로 펴진다). 보통 파일이 아닌 것은
/// io 가 내는 말과 같은 결로 댄다.
pub fn spelled(at: &Path, why: &Unheld) -> String {
    match why {
        Unheld::Outside { to, .. } | Unheld::IntoGit { to } => {
            format!("{} -> {}", at.display(), crate::text::one_line(&to.display().to_string()))
        }
        Unheld::NotAFile => format!("{}: not a regular file", at.display()),
        Unheld::Link => format!("{}: a link, not followed", at.display()),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// 안을 가리키는 링크는 따르고, 밖·`.git/` 으로 가는 링크는 안 따른다. **끝이 없는 링크도 잰다** — 끝의
    /// 디렉터리로, 그 디렉터리도 없으면 아직 있는 조상으로. 링크가 아닌 없는 자리만 받은 철자 그대로다.
    #[cfg(unix)]
    #[test]
    fn a_link_is_followed_only_inside_its_checkout_and_never_into_git() {
        let s = Scratch::new("held-place");
        let away = Scratch::new("held-place-away");
        let home = Home::of(s.path());
        std::fs::create_dir_all(s.join("data")).unwrap();
        std::fs::create_dir_all(s.join(".git")).unwrap();
        std::fs::write(s.join("data/x"), "").unwrap();
        std::fs::write(s.join(".git/config"), "").unwrap();
        std::fs::write(away.join("y"), "").unwrap();
        let link = |name: &str, to: &Path| {
            let at = s.join(name);
            std::os::unix::fs::symlink(to, &at).unwrap();
            at
        };

        assert_eq!(place(&link("in", Path::new("data/x")), &home), Ok(home.0.join("data/x")));
        let out = link("out", &away.join("y"));
        assert!(matches!(place(&out, &home), Err(Unheld::Outside { .. })), "밖으로 가는 링크를 따랐다");
        let git = link("git", Path::new(".git/config"));
        assert!(matches!(place(&git, &home), Err(Unheld::IntoGit { .. })), ".git 으로 가는 링크를 따랐다");
        // 없는 자리는 못 푼다 — 받은 철자 그대로 나가 읽는 쪽이 `NotFound` 로 넘긴다.
        let gone = s.join("gone");
        assert_eq!(place(&gone, &home), Ok(gone));

        // **끝이 없는 링크도 잰다**(리뷰 moai-itsu.n8z) — 통째로는 못 풀어도 끝의 디렉터리는 풀린다. 재지 않던
        // 판은 이 셋을 "없음" 으로 넘겨, 밖을 가리키는 스냅샷이 빈 보드로 섰다.
        let dangling = link("dangling-out", &away.join("missing"));
        assert!(matches!(place(&dangling, &home), Err(Unheld::Outside { .. })), "끝이 없는 밖 링크를 안 쟀다");
        let dangling_git = link("dangling-git", Path::new(".git/missing"));
        assert!(matches!(place(&dangling_git, &home), Err(Unheld::IntoGit { .. })), "끝이 없는 .git 링크를 안 쟀다");
        // 끝의 디렉터리마저 없는 절대 링크(받은 사람의 기계에 없는 자리)는 아직 있는 조상으로 잰다.
        let elsewhere = link("elsewhere", Path::new("/nonexistent-moai-itsu/someone/.moai/issues.jsonl"));
        assert!(matches!(place(&elsewhere, &home), Err(Unheld::Outside { .. })), "없는 남의 자리를 안 쟀다");
        // 안을 가리키는 끝 없는 링크는 그대로 안이다 — 읽는 쪽이 `NotFound` 로 넘긴다.
        let inside = link("dangling-in", Path::new("data/missing"));
        assert_eq!(place(&inside, &home), Ok(home.0.join("data/missing")));
    }

    /// **아직 없는 디렉터리는 있는 가장 깊은 조상으로 잰다**(moai-kxkw.7ky) — [`place`] 는 링크 아닌 없는 자리를 그대로
    /// 내보내, 밖을 가리키는 `.moai` 밑의 `.moai/mail` 이 안으로 읽혔다. 푼 자리를 내고, 없는 조각은 그 뒤에 붙인다.
    #[cfg(unix)]
    #[test]
    fn a_directory_not_built_yet_is_measured_by_its_deepest_ancestor() {
        let s = Scratch::new("held-place-dir");
        let away = Scratch::new("held-place-dir-away");
        let home = Home::of(s.path());
        std::fs::create_dir_all(s.join("data")).unwrap();
        std::fs::create_dir_all(s.join(".git")).unwrap();
        let link = |name: &str, to: &Path| std::os::unix::fs::symlink(to, s.join(name)).unwrap();
        link("out", away.path());
        link("in", Path::new("data"));
        link("git", Path::new(".git"));
        link("dangling-in", Path::new("data/later"));

        assert_eq!(place(&s.join("out/mail"), &home), Ok(s.join("out/mail")), "전제가 갈렸다 — place 가 이미 잰다");
        assert!(matches!(place_dir(&s.join("out/mail/w1"), &home), Err(Unheld::Outside { .. })), "밖 밑을 안으로 쟀다");
        assert!(matches!(place_dir(&s.join("out"), &home), Err(Unheld::Outside { .. })));
        assert!(matches!(place_dir(&s.join("git/mail"), &home), Err(Unheld::IntoGit { .. })), ".git 밑을 안으로 쟀다");
        assert_eq!(
            place_dir(&s.join("in/mail/w1"), &home),
            Ok(home.0.join("data/mail/w1")),
            "안을 가리키는 링크를 안 풀었다"
        );
        assert_eq!(place_dir(&s.join("fresh/mail"), &home), Ok(home.0.join("fresh/mail")));
        assert_eq!(
            place_dir(&s.join("dangling-in/w1"), &home),
            Ok(home.0.join("data/later/w1")),
            "끝 없는 안 링크를 안 풀었다"
        );

        // **[`place`] 가 못 푸는 링크는 거절하고, 끝 없는 링크의 사슬은 끝까지 다시 잰다**(리뷰 moai-kxkw.k2f) — 둘 다 그대로
        // 지나던 판은 잰 뒤 밖의 끝이 생기는 틈에 밖에 지었다.
        let away_name = away.path().file_name().expect("자리에 이름이 있다");
        link("dangling-up", &Path::new("..").join(away_name).join("missing/deep"));
        link("not-yet", &away.join("not-yet"));
        link("via", Path::new("not-yet/x"));
        link("hop", Path::new("dangling-in/x"));
        assert!(
            matches!(place_dir(&s.join("dangling-up/w1"), &home), Err(Unheld::Outside { .. })),
            "끝의 디렉터리도 없는 `..` 링크를 안으로 쟀다"
        );
        assert!(
            matches!(place_dir(&s.join("via/w1"), &home), Err(Unheld::Outside { .. })),
            "끝 없는 밖 링크를 거쳐 가는 링크를 안으로 쟀다"
        );
        assert_eq!(
            place_dir(&s.join("hop/w1"), &home),
            Ok(home.0.join("data/later/x/w1")),
            "끝 없는 안 링크의 사슬을 끝까지 안 풀었다"
        );
    }

    /// **git 의 자리는 조각째, 대소문자 없이 잰다**(리뷰 moai-x0o7.52k) — 대소문자를 안 가르는 볼륨에서는 `.GIT` 도
    /// 그 자리다. 끝 이름도 세고(딸린 워크트리의 `.git` 파일), `.github`·`.gitignore` 처럼 이름만 닮은 것은 안 센다.
    #[test]
    fn git_s_place_is_matched_by_whole_component_without_case() {
        for p in [".git", ".git/config", "sub/.GIT/moai/issues.jsonl", "wt/.Git"] {
            assert!(into_git(Path::new(p)), "{p} 를 git 의 자리로 안 셌다");
        }
        for p in [".github/workflows", "a.git", "git", "", ".gitignore"] {
            assert!(!into_git(Path::new(p)), "{p} 를 git 의 자리로 셌다");
        }
    }

    /// 일을 딴 실에서 돌려 **멈추면 기다리지 않고 진다** — 고침이 없으면 FIFO 를 여는 `open` 이 쓰는 쪽을 영영
    /// 기다린다. 그 실은 남겨 두고 시험만 지게 해, 붉은 시험의 이름이 CI 의 시간 끝보다 먼저 선다(리뷰
    /// moai-itsu.n8z). 다른 모듈의 FIFO 시험(`worktree`)도 이것을 쓴다.
    #[cfg(unix)]
    pub(crate) fn within<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        let body = std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        match rx.recv_timeout(std::time::Duration::from_secs(60)) {
            Ok(got) => got,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!("{what}: 멈췄다 — 막히지 않게 열지 않았다"),
            // 실 안에서 진 단언은 그 말 그대로 다시 던진다.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => match body.join() {
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(()) => unreachable!("보내기 전에 끝났다"),
            },
        }
    }

    /// **보통 파일이 아니면 안 읽는다** — FIFO 를 열어도 멈추지 않는다. 고침이 없으면 `open` 이 쓰는 쪽을 영영
    /// 기다리는데, 시험은 [`within`] 이 정한 때에 진다. 소켓은 `open` 에서 지는데(`ENXIO`), 그것도 같은 갈래로 선다.
    #[cfg(unix)]
    #[test]
    fn a_fifo_a_socket_or_a_directory_is_not_read_and_does_not_hang() {
        let s = Scratch::new("held-fifo");
        let fifo = s.join("f");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: 널로 끝나는 경로와 권한 비트만 넘긴다.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "FIFO 를 못 지었다");
        let opened = fifo.clone();
        assert!(matches!(within("FIFO", move || read(&opened)), Err(Fell::Unheld(Unheld::NotAFile))));
        assert!(matches!(read(s.path()), Err(Fell::Unheld(Unheld::NotAFile))));
        assert!(matches!(read(&s.join("none")), Err(Fell::Io(e)) if e.kind() == std::io::ErrorKind::NotFound));
        let sock = s.join("s");
        let _listener = std::os::unix::net::UnixListener::bind(&sock).expect("소켓을 못 지었다");
        assert!(matches!(read(&sock), Err(Fell::Unheld(Unheld::NotAFile))), "소켓을 io 실패로 댔다");
    }

    /// **손잡이가 댄 크기까지만 읽는다.** `/proc/self/status` 는 `stat` 이 크기 0 이라 답하면서 글을 내는
    /// 파일이라, 고침이 없으면 글이 읽히고 고치면 빈 것이다. 끝없이 읽는 파일로 재지 않는다 — 고침이 없으면
    /// 시험이 메모리를 다 쓴다. 저널이 이 자를 지나는지는 `store` 의 시험이 따로 잰다.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_is_read_no_further_than_the_size_its_handle_gives() {
        let status = Path::new("/proc/self/status");
        assert_eq!(std::fs::metadata(status).unwrap().len(), 0, "procfs 가 크기를 대는 경우다 — 시험이 못 잰다");
        assert_eq!(read(status).unwrap(), Vec::<u8>::new(), "손잡이가 댄 크기 너머를 읽었다");
    }

    /// 말은 고른 말로 서고, 링크의 끝에 든 제어 문자는 걷힌다.
    #[test]
    fn the_reason_is_said_in_the_chosen_language_without_control_characters() {
        let why = Unheld::Outside { to: PathBuf::from("/proc/\u{1b}[2Jx"), home: PathBuf::from("/w") };
        let (en, ko) = (said(crate::i18n::Lang::En, &why), said(crate::i18n::Lang::Ko, &why));
        assert_ne!(en, ko, "말이 안 갈린다 — 말묶음을 안 지났다");
        for s in [&en, &ko, &spelled(Path::new("/w/.moai/issues.jsonl"), &why)] {
            assert!(!s.contains('\u{1b}'), "제어 문자가 그대로 나간다 — {s:?}");
        }
    }
}
