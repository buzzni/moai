//! **저장소가 든 파일을 읽는 자**(moai-itsu). 스냅샷(`.moai/issues.jsonl`)·설정(`.moai/config.toml`)·저널
//! (`.moai/journal/*.jsonl`)은 받은 저장소가 커밋한 링크일 수 있다 — 0.1.6(moai-karj)이 저널에만 건 자를
//! 셋에 같은 자로 편다. `issues.jsonl -> /proc/self/pagemap` 하나로 모든 명령이 메모리를 다 쓰던 자리다.
//!
//! 자는 둘이고, 둘 다 여기 하나씩이다.
//!
//! - [`place`] — 링크를 푼 자리가 **체크아웃 안이고 `.git/` 밖**인가. 쓰기(`store::target_of`)가 이미
//!   거절하던 자리를 읽기도 같은 자로 잰다. procfs·sysfs·장치 파일은 늘 체크아웃 밖이고, git 이 실을 수
//!   있는 것은 보통 파일과 링크뿐이라 안에 선 보통 파일은 디스크에 있는 만큼만 읽힌다
//! - [`read`] — 연 손잡이가 **보통 파일이라 답할 때만, 그 손잡이가 댄 크기까지만** 읽는다. 이름으로 잰 뒤
//!   읽기 전에 그 이름이 갈리는 틈을 손잡이가 닫는다. `O_NONBLOCK` 으로 열어 FIFO 앞에서 쓰는 쪽을
//!   기다리며 멈추지 않는다
//!
//! **거절은 자료다**([`Unheld`]) — 글은 부르는 쪽이 고른 말로 [`said`] 가 짓는다. 말을 못 고르는 자리
//! (옆 워크트리의 스냅샷)는 말 없는 꼴 [`spelled`] 를 쓴다.

use std::path::{Path, PathBuf};

/// 저장소가 든 파일을 **안 읽는 까닭** — 말이 아니라 자료다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unheld {
    /// 링크가 체크아웃 밖으로 풀린다 — 풀린 자리와 견준 체크아웃(푼 철자).
    Outside { to: PathBuf, home: PathBuf },
    /// 링크가 체크아웃 안의 git 자리(`.git/`)로 풀린다 — 풀린 자리. 팩 파일 하나가 몇 GiB 라도 통째로 담긴다.
    IntoGit { to: PathBuf },
    /// 보통 파일이 아니다 — FIFO·장치·디렉터리. 링크가 아니라 그 자리에 바로 선 것도 든다.
    NotAFile,
}

/// [`read_inside`] 가 진 까닭 — **안 읽기로 한 것과 io 가 진 것을 가른다.** 앞의 것은 손으로 링크를 고칠
/// 일이고(`--json` 의 `broken`), 뒤의 것은 운영체제가 낸 말 그대로다.
#[derive(Debug)]
pub enum Fell {
    Unheld(Unheld),
    Io(std::io::Error),
}

/// 받은 자리를 푼 자리 — **체크아웃 안이고 `.git/` 밖일 때만** 낸다. `home` 은 이미 푼 뿌리
/// ([`crate::path::real`])다. 부르는 쪽이 파일 여럿을 잴 때 뿌리를 한 번만 풀게 받는다.
///
/// - **못 푼 자리는 재지 않는다**(`real == p`) — 받은 철자 그대로라 어디로 가는지 모른다. 뿌리의 조상에
///   링크가 있으면 그 철자는 푼 뿌리와 안 맞아 밖으로 잘못 읽힌다. 못 푼 자리는 읽는 쪽이 넘기거나 센다
/// - **안을 가리키는 링크는 그대로 따른다** — 메일을 바꾼 사람의 `<옛 메일>.jsonl -> <새 메일>.jsonl`
///   (moai-p9mq), `issues.jsonl -> data/issues.jsonl` 이 그렇다
/// - **끝 이름도 센다** — 딸린 워크트리의 `.git` 은 디렉터리가 아니라 `gitdir:` 한 줄짜리 파일이다
///   (`store::target_of` 와 같은 자)
pub(crate) fn place(p: &Path, home: &Path) -> Result<PathBuf, Unheld> {
    let real = crate::path::real(p);
    if real == p {
        return Ok(real);
    }
    match real.strip_prefix(home) {
        Err(_) => Err(Unheld::Outside { to: real, home: home.to_path_buf() }),
        Ok(rest) if rest.components().any(|c| c.as_os_str() == ".git") => Err(Unheld::IntoGit { to: real }),
        Ok(_) => Ok(real),
    }
}

/// **읽기 전에 재기만 한다** — [`place`] 와, 그 자리가 보통 파일인가(`stat`, FIFO 앞에서도 안 멈춘다). 없는
/// 자리와 못 잰 자리는 지나간다 — 읽는 쪽이 그 말로 넘기거나 멈춘다. 말을 고를 수 있는 자리(`store::Repo` 를
/// 짓는 길)가 읽기보다 먼저 물어, 거절을 고른 말로 낸다.
pub(crate) fn check(p: &Path, home: &Path) -> Result<(), Unheld> {
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
/// `stat` 이 크기 0 이라 답하면서 글을 끝없이 내는 procfs 파일은 빈 것으로 읽힌다.
///
/// **링크의 끝이 어디인지는 안 잰다** — 그것은 [`place`] 의 일이고, 둘을 함께 거는 길이 [`read_inside`] 다.
/// 이것만 따로 부르는 자리는 그 앞에서 이미 잰 쪽이다(`store::Repo::journal_files`, `store::Repo::read`).
pub(crate) fn read(p: &Path) -> Result<Vec<u8>, Fell> {
    use std::io::Read;
    let f = open(p).map_err(Fell::Io)?;
    let m = f.metadata().map_err(Fell::Io)?;
    if !m.is_file() {
        return Err(Fell::Unheld(Unheld::NotAFile));
    }
    let mut out = Vec::new();
    f.take(m.len()).read_to_end(&mut out).map_err(Fell::Io)?;
    Ok(out)
}

/// [`place`] 로 재고 [`read`] 로 읽는다 — 저장소가 든 파일 하나를 그 체크아웃(`home`, 받은 철자) 안에서만.
/// **푼 자리를 연다** — 받은 철자를 다시 열면 잰 뒤 바뀐 링크를 따라간다(`store::append_inside` 와 같은 까닭).
pub(crate) fn read_inside(p: &Path, home: &Path) -> Result<Vec<u8>, Fell> {
    let real = place(p, &crate::path::real(home)).map_err(Fell::Unheld)?;
    read(&real)
}

/// 글로 읽는다 — [`read`] 와 같되 UTF-8 이 아니면 `fs::read_to_string` 과 같은 io 실패다. 그 자리를 이것으로
/// 바꿔도 사람이 보는 말이 안 바뀐다.
pub(crate) fn text(bytes: Vec<u8>) -> std::io::Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// 읽기 전용으로, **막히지 않게** 연다. FIFO 를 그냥 열면 쓰는 쪽이 올 때까지 `open` 에서 영영 멈춘다 —
/// `O_NONBLOCK` 은 보통 파일에서는 아무것도 안 바꾸고, FIFO 는 바로 열려 [`read`] 의 `fstat` 에서 걸린다.
pub(crate) fn open(p: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NONBLOCK);
    }
    o.open(p)
}

/// 안 읽은 까닭 한 토막 — 고른 말로. 앞에 자리를 다는 것은 부르는 쪽이다(스냅샷·설정은 `held.refused`,
/// 저널은 `warn.unread_journal`).
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
    }
}

/// 안 읽어 멈춘 한 줄 — `<자리>: 읽지 않아 이 명령은 여기서 멈춘다 — <까닭>`. 스냅샷처럼 못 읽으면 아무것도
/// 못 하는 파일이 쓴다(`store::Repo` 를 짓는 길).
pub fn refused(lang: crate::i18n::Lang, at: &Path, why: &Unheld) -> String {
    crate::i18n::fill(
        crate::i18n::say(lang, "held.refused"),
        &[("at", &at.display().to_string()), ("why", &said(lang, why))],
    )
}

/// 말 없는 꼴 — `<자리> -> <풀린 자리>`. **말을 못 고르는 자리가 쓴다** — 옆 워크트리의 스냅샷을 읽는
/// 길(`worktree::gather`)은 화면 말을 모르고, 그 줄은 말묶음의 `trouble.unread` 가 감싼다. 보통 파일이
/// 아닌 것은 io 가 내는 말과 같은 결로 댄다.
pub fn spelled(at: &Path, why: &Unheld) -> String {
    match why {
        Unheld::Outside { to, .. } | Unheld::IntoGit { to } => {
            format!("{} -> {}", at.display(), crate::text::one_line(&to.display().to_string()))
        }
        Unheld::NotAFile => format!("{}: not a regular file", at.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// 안을 가리키는 링크는 따르고, 밖·`.git/` 으로 가는 링크는 안 따른다. 못 푼 철자는 재지 않는다.
    #[cfg(unix)]
    #[test]
    fn a_link_is_followed_only_inside_its_checkout_and_never_into_git() {
        let s = Scratch::new("held-place");
        let away = Scratch::new("held-place-away");
        let home = crate::path::real(s.path());
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

        assert_eq!(place(&link("in", Path::new("data/x")), &home), Ok(home.join("data/x")));
        let out = link("out", &away.join("y"));
        assert!(matches!(place(&out, &home), Err(Unheld::Outside { .. })), "밖으로 가는 링크를 따랐다");
        let git = link("git", Path::new(".git/config"));
        assert!(matches!(place(&git, &home), Err(Unheld::IntoGit { .. })), ".git 으로 가는 링크를 따랐다");
        // 없는 자리는 못 푼다 — 받은 철자 그대로 나가 읽는 쪽이 `NotFound` 로 넘긴다.
        let gone = s.join("gone");
        assert_eq!(place(&gone, &home), Ok(gone));
    }

    /// **보통 파일이 아니면 안 읽는다** — FIFO 를 열어도 멈추지 않는다. 고침이 없으면 이 시험이 `open` 에서
    /// 영영 멈춘다(쓰는 쪽이 없다).
    #[cfg(unix)]
    #[test]
    fn a_fifo_or_a_directory_is_not_read_and_does_not_hang() {
        let s = Scratch::new("held-fifo");
        let fifo = s.join("f");
        let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: 널로 끝나는 경로와 권한 비트만 넘긴다.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "FIFO 를 못 지었다");
        assert!(matches!(read(&fifo), Err(Fell::Unheld(Unheld::NotAFile))));
        assert!(matches!(read(s.path()), Err(Fell::Unheld(Unheld::NotAFile))));
        assert!(matches!(read(&s.join("none")), Err(Fell::Io(e)) if e.kind() == std::io::ErrorKind::NotFound));
    }

    /// **손잡이가 댄 크기까지만 읽는다.** `/proc/self/status` 는 `stat` 이 크기 0 이라 답하면서 글을 내는
    /// 파일이라, 고침이 없으면 글이 읽히고 고치면 빈 것이다. 끝없이 읽는 파일로 재지 않는다 — 고침이 없으면
    /// 시험이 메모리를 다 쓴다.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_is_read_no_further_than_the_size_its_handle_gives() {
        let status = Path::new("/proc/self/status");
        assert_eq!(std::fs::metadata(status).unwrap().len(), 0, "procfs 가 크기를 대는 판이다 — 시험이 못 잰다");
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
