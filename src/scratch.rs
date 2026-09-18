//! 시험이 쓰는 임시 디렉터리 한 벌(moai-shgx). **시험에서만 선다.**
//!
//! ## 왜 한 벌인가
//!
//! 같은 것이 다섯 벌 있었다 — `tui/register.rs`·`tui/layer.rs`·`tui/mod.rs`·
//! `user_config.rs` 와 통합 시험의 것. 앞의 넷과 `cmd/tui.rs` 의 편집기 시험 자리는
//! 이것으로 모았다. **통합 시험(`tests/cli.rs`)의 것은 아직 따로다** — 따로 된
//! 크레이트라 `use` 로는 못 가져간다. 그리고 **모으지 않은 자리**(`git.rs`·
//! `store.rs`·`worktree.rs`)는 끝에서 `remove_dir_all` 을 부르는 식이라, 시험이
//! 패닉하면 그 줄에 닿지 못해 `/tmp/moai-git-<pid>` 가 그대로 남는다. 이름에 pid 가
//! 들어 다음 판이 그것을 치우지도 않는다 — 이 기계에 죽은 pid 의 것 셋이 남아 있었다.
//!
//! `Drop` 은 패닉으로 풀릴 때도 돈다. 그래서 치우는 일은 **놓을 때** 한다.
//!
//! ## 자리
//!
//! `std::env::temp_dir()` 다. **`CARGO_TARGET_TMPDIR` 은 여기서 못 쓴다** — cargo 는
//! 그것을 통합 시험·벤치 타깃에만 주고, 이 모듈은 바이너리 타깃의 단위 시험으로
//! 컴파일되므로 `option_env!` 가 늘 `None` 이었다(`tests/cli.rs` 는 통합 시험이라
//! `env!` 로 그것을 곧바로 읽는다). 있는 척하는 갈래는 걷어냈다.
//!
//! 저장소 안(`target/` 밑)으로 옮기지도 않는다 — 울타리 없는 자리마다 위의 `.git` 이
//! 꼭대기로 잡혀 moai-46xz 가 통째로 돌아온다([`Scratch::fenced`]). 치우는 일은
//! `Drop` 이 맡으므로 `/tmp` 에 쌓이지 않는다.

use std::path::{Path, PathBuf};

/// 시험 하나가 쓰는 디렉터리. 놓을 때 지운다.
///
/// 이름에 pid·스레드·셈을 함께 넣는다 — pid 만으로는 한 판 안의 스레드 둘이 같은 자리를
/// 잡고, 한쪽의 `Drop` 이 다른 쪽이 쓰는 자리를 지운다.
pub struct Scratch(PathBuf);

impl Scratch {
    /// `<자리>/moai-<이름>-<pid>-<스레드>-<셈>` 을 만든다. 이미 있으면 지우고 새로 만든다.
    pub fn new(name: &str) -> Scratch {
        Scratch::in_place(&base(), name)
    }

    /// 자리를 받아 세운다. 이름 짓는 법은 [`Scratch::new`] 와 같다. 울타리는 없다 —
    /// [`Scratch::fenced_in`] 이 이것 위에 세우고, 울타리가 실제로 일하는지 견주는
    /// 시험이 대조군으로 쓴다.
    pub fn in_place(base: &Path, name: &str) -> Scratch {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NTH: AtomicU32 = AtomicU32::new(0);
        let dir = base.join(format!(
            "moai-{name}-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            NTH.fetch_add(1, Ordering::Relaxed),
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        Scratch(dir)
    }

    /// 링크를 푼 자리. macOS 의 `/tmp` 는 `/private/tmp` 로 가는 링크라, 도구가 내는
    /// 철자와 시험이 든 철자가 다르면 견주는 시험이 헛돈다.
    pub fn real(name: &str) -> Scratch {
        let mut s = Scratch::new(name);
        s.0 = std::fs::canonicalize(&s.0).unwrap_or_else(|e| panic!("{}: {e}", s.0.display()));
        s
    }

    /// 아무것도 없는 저장소를 울타리로 세운 자리(moai-46xz). **위로 새지 않는다.**
    ///
    /// `worktree` 의 파일로 읽는 반쪽은 `Path::ancestors()` 를 타고 올라가 `.git` 을 처음
    /// 만나는 곳을 저장소 꼭대기로 읽는다. 임시 자리가 어느 체크아웃 **안**이면
    /// (컨테이너·CI 에서 `TMPDIR` 이 그렇게 잡힌다) 그 체크아웃이 꼭대기로 잡혀, 시험이
    /// 만들지도 않은 워크트리 이름이 답에 섞인다. 울타리가 그 훑기를 여기서 멈춘다.
    ///
    /// **빈 `.git` 디렉터리로는 반쪽만 막는다.** 파일로 읽는 훑기는 거기서 서지만, git 은
    /// `HEAD`·`objects`·`refs` 가 없는 `.git` 을 저장소로 안 보고 **지나쳐 위로 올라간다**(재
    /// 봤다) — `heads`·`gather`·`git::table` 처럼 git 으로 읽는 길은 그대로 위의 체크아웃을
    /// 읽는다. 그래서 그 셋을 둔다. 커밋도 딸린 워크트리도 없으니 이름은 제 것뿐이고(`away` 는
    /// 제 것을 뺀다), 밑에 진짜 저장소를 만드는 시험은 영향이 없다 — 두 훑기 모두 그 저장소를
    /// 먼저 만난다.
    pub fn fenced(name: &str) -> Scratch {
        Scratch::fenced_in(&base(), name)
    }

    /// 자리를 골라 세우는 울타리. 임시 자리가 체크아웃 안일 때를 **흉내 내는** 시험이 쓴다 —
    /// 그런 시험은 `TMPDIR` 을 못 바꾼다(한 판의 모든 스레드가 그것을 함께 본다).
    pub fn fenced_in(base: &Path, name: &str) -> Scratch {
        let s = Scratch::in_place(base, name);
        let git = s.0.join(".git");
        let fail = |e: std::io::Error| panic!("{}: {e}", git.display());
        for dir in ["objects", "refs"] {
            std::fs::create_dir_all(git.join(dir)).unwrap_or_else(fail);
        }
        std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap_or_else(fail);
        s
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 임시 자리의 뿌리. 모듈 머리의 "자리" 가 왜 이것뿐인지를 적어 둔다.
fn base() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **패닉으로 끝나도 치운다.** 끝에서 `remove_dir_all` 을 부르던 자리들이 남긴
    /// 찌꺼기가 이 시험이 막는 것이다.
    #[test]
    fn a_panicking_test_still_leaves_nothing() {
        let seen = std::panic::catch_unwind(|| {
            let s = Scratch::new("panic");
            let dir = s.path().to_path_buf();
            std::fs::write(dir.join("있다"), "x").unwrap();
            assert!(dir.is_dir());
            panic!("{}", dir.display());
        });
        let dir = PathBuf::from(*seen.unwrap_err().downcast::<String>().unwrap());
        assert!(!dir.exists(), "패닉 뒤에 {} 가 남았다", dir.display());
    }

    /// 같은 이름을 같은 판에서 두 번 잡아도 서로의 자리를 안 지운다.
    #[test]
    fn two_scratches_of_one_name_do_not_share_a_place() {
        let (a, b) = (Scratch::new("twice"), Scratch::new("twice"));
        assert_ne!(a.path(), b.path());
        std::fs::write(a.join("a"), "a").unwrap();
        drop(b);
        assert!(a.join("a").exists(), "옆의 Drop 이 이쪽 자리를 지웠다");
    }
}
