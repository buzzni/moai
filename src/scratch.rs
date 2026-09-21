//! 시험이 쓰는 임시 디렉터리 한 벌(moai-shgx). **시험에서만 선다.**
//!
//! ## 왜 한 벌인가
//!
//! 같은 것이 다섯 벌 있었다 — `tui/register.rs`·`tui/layer.rs`·`tui/mod.rs`·
//! `user_config.rs` 와 통합 시험의 것. 앞의 넷과 `cmd/tui.rs` 의 편집기 시험 자리는
//! 이것으로 모았다. **통합 시험(`tests/cli.rs`)의 `Scratch` 는 아직 따로다** — 따로 된
//! 크레이트라 `use` 로는 못 가져가고, 뿌리([`base`])만 `#[path]` 로 이 파일을
//! 읽어 함께 쓴다. 울타리는 안 쓴다(moai-izeo 뒤로 그쪽에서 [`fence`] 를 부르지 않는다).
//! 그래서 **이 파일의 단위 시험은 그쪽에서도 컴파일돼 돈다** — 여기 더하는 시험은 `crate::` 의
//! 다른 모듈을 쓰지 못한다(그쪽 크레이트에는 없다). 그리고 **모으지 않은 자리**(`git.rs`·
//! `store.rs`·`worktree.rs`)는 끝에서 `remove_dir_all` 을 부르는 식이라, 시험이
//! 패닉하면 그 줄에 닿지 못해 `/tmp/moai-git-<pid>` 가 그대로 남는다. 이름에 pid 가
//! 들어 다음 판이 그것을 치우지도 않는다 — 이 기계에 죽은 pid 의 것 셋이 남아 있었다.
//!
//! `Drop` 은 패닉으로 풀릴 때도 돈다. 그래서 치우는 일은 **놓을 때** 한다.
//!
//! ## 자리
//!
//! `std::env::temp_dir()` 다 — **그것이 어느 체크아웃 안이면 밖으로 옮긴다**([`base`], moai-izeo).
//! **`CARGO_TARGET_TMPDIR` 은 여기서 못 쓴다** — cargo 는
//! 그것을 통합 시험·벤치 타깃에만 주고, 이 모듈은 바이너리 타깃의 단위 시험으로
//! 컴파일되므로 `option_env!` 가 늘 `None` 이었다(`tests/cli.rs` 는 통합 시험이라
//! `env!` 로 그것을 곧바로 읽는다). 있는 척하는 갈래는 걷어냈다.
//!
//! 일부러 저장소 안(`target/` 밑)으로 옮기지는 않는다 — 위의 `.git` 이 꼭대기로 잡혀 moai-46xz 가
//! 통째로 돌아오고, 위의 `.moai` 는 그보다 나쁘다(moai-izeo: 시험이 바깥 트래커에 실제로 썼다).
//! [`Scratch::fenced`] 는 그래서 남는다 — **자리를 고르는 시험**(`layer`·`worktree`·`git`)이 제
//! 자리를 저장소 꼭대기로 세울 때 쓴다. 울타리는 자리째 `Drop` 이 치운다 — 체크아웃 안에 커밋
//! 없는 저장소가 남으면 그 체크아웃의 `git add -A` 가 거기서 멈춘다([`fence`]).

use std::path::{Path, PathBuf};

/// 시험 하나가 쓰는 디렉터리. 놓을 때 지운다.
///
/// 이름에 pid·스레드·셈을 함께 넣는다 — pid 만으로는 한 판 안의 스레드 둘이 같은 자리를
/// 잡고, 한쪽의 `Drop` 이 다른 쪽이 쓰는 자리를 지운다.
pub struct Scratch(PathBuf);

impl Scratch {
    /// `<자리>/moai-<이름>-<pid>-<스레드>-<셈>` 을 만든다. 이미 있으면 지우고 새로 만든다.
    ///
    /// **울타리는 없다.** [`base`] 가 체크아웃 밖임을 보장하므로 위에 잡힐 `.git` 도 `.moai` 도
    /// 없다(moai-izeo). 울타리를 골라 치던 자리다 — 그것은 `.git` 훑기만 세웠고, `.moai` 를
    /// 찾아 오르는 쪽은 그대로 바깥 트래커를 잡았다.
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
        // **여기만 터진다** — 방금 지은 디렉터리라 못 푸는 것은 시험 자리가 성치 않다는 뜻이고,
        // 받은 철자로 떨어지면([`crate::store::real`]) 견주는 시험이 까닭 없이 붉어진다.
        s.0 = std::fs::canonicalize(&s.0).unwrap_or_else(|e| panic!("{}: {e}", s.0.display()));
        s
    }

    /// 아무것도 없는 저장소를 울타리로 세운 자리(moai-46xz). **위로 새지 않는다.**
    ///
    /// `worktree` 의 파일로 읽는 반쪽은 `Path::ancestors()` 를 타고 올라가 `.git` 을 처음
    /// 만나는 곳을 저장소 꼭대기로 읽는다. 자리가 어느 체크아웃 **안**이면 그 체크아웃이
    /// 꼭대기로 잡혀, 시험이 만들지도 않은 워크트리 이름이 답에 섞인다. 울타리가 그 훑기를
    /// 여기서 멈춘다.
    ///
    /// **[`base`] 밑에서는 그럴 일이 없다**(moai-izeo) — 그러니 이것을 부르는 것은 제 자리를
    /// 일부러 저장소 꼭대기로 세우려는 시험이지, 바깥이 샐까 봐 막는 시험이 아니다.
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
        fence(&s.0);
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

/// 임시 자리의 뿌리. **어느 체크아웃 안도 아님을 보장한다**(moai-izeo, 2026-09-20 사용자 결정).
///
/// `TMPDIR` 이 moai 체크아웃 안이면(컨테이너·CI 에서 흔하다) 시험이 만든 자리 위에 그
/// 체크아웃이 서고, moai 는 `.git` 이 아니라 `.moai` 를 찾아 위로 오르므로 **바깥 저장소의
/// 트래커**가 잡힌다. 통합 시험 스무 개가 붉었고, 그중 하나는 그 트래커에 이슈를 실제로 **썼다**
/// (moai-595o). 울타리([`fence`])로는 못 막는다 — 그것은 `.git` 훑기를 세울 뿐이다.
///
/// **환경변수로 천장을 두는 길은 안 갔다.** 시험 하나 때문에 생긴 변수가 찾기 경로의 갈래로
/// 영영 남는다. 자리를 옮기는 것은 시험 바닥 안에서 끝나고 제품 표면을 안 늘린다.
///
/// 옮겼으면 **한 줄로 알린다** — `TMPDIR` 을 준 사람은 거기에 쓸 까닭이 있었고, 말없이 딴 데
/// 쓰면 디스크가 어디서 차는지 못 찾는다(libtest 가 지나간 시험의 글을 삼키므로 붉은 판이나
/// `--nocapture` 에서 보인다). 갈 곳이 아예 없으면 크게 실패한다: 그대로 돌면 스무 개가 알 수
/// 없는 까닭으로 붉고 남의 트래커가 더러워진다.
pub fn base() -> PathBuf {
    static BASE: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    // **셈은 한 번이고 그 뒤로는 그 답을 되뇐다.** `OnceLock` 은 패닉한 초기화를 독으로 안 봐,
    // 안에서 패닉하면 부를 때마다 훑기를 처음부터 다시 돌고 같은 말을 새로 짓는다 — 그러면
    // "크게 실패한다" 가 진단 한 줄이 아니라 시험 수만큼 되풀이되는 소음이 된다.
    match BASE.get_or_init(pick_base) {
        Ok(dir) => dir.clone(),
        Err(why) => panic!("{why}"),
    }
}

/// [`base`] 가 한 번 하는 셈. 갈 곳이 없으면 그 까닭을 글로 낸다.
fn pick_base() -> Result<PathBuf, String> {
    pick(std::env::temp_dir(), &[Path::new("/tmp"), Path::new("/var/tmp")])
}

/// [`pick_base`] 의 셈을 **자리를 받아** 푼다. 갈래가 갈리는 곳이 여기라, 받지 않으면 옮기는
/// 쪽도 실패하는 쪽도 어느 기계에서도 안 돌고 되돌려 놔도 판이 푸르다 — `std::env::temp_dir()`
/// 은 한 판의 모든 스레드가 함께 봐 시험이 못 바꾼다.
fn pick(wanted: PathBuf, aways: &[&Path]) -> Result<PathBuf, String> {
    if !inside_checkout(&wanted) {
        return Ok(wanted);
    }
    for away in aways {
        let away = away.to_path_buf();
        // **있는 자리가 아니라 쓸 수 있는 자리를 고른다.** `TMPDIR` 을 딴 데로 돌린 까닭이 대개
        // `/tmp` 가 못 쓰는 자리(읽기 전용·꽉 참·샌드박스 밖)여서라, 있기만 한 것을 보고 옮기면
        // 시험 천여 개가 저마다 `Permission denied` 로 넘어지고 아래의 한 줄은 그 까닭을 못 댄다.
        if inside_checkout(&away) || !usable(&away) {
            continue;
        }
        // **링크를 풀어 넘긴다** — macOS 의 `/tmp` 는 `/private/tmp` 로 가는 링크라, 안 풀면
        // 도구가 내는 철자와 시험이 든 철자가 갈려 경로를 견주는 시험이 통째로 헛돈다
        // ([`Scratch::real`] 이 자리마다 풀던 것을 뿌리에서 한 번에 푼다).
        let away = real(&away);
        eprintln!(
            "moai 시험: 임시 자리({})가 체크아웃 안이라 {} 로 옮긴다 — \
             그대로 두면 시험이 바깥 저장소의 .moai 를 잡는다",
            wanted.display(),
            away.display()
        );
        return Ok(away);
    }
    Err(format!(
        "임시 자리({})가 체크아웃 안이고 밖에 쓸 자리도 없다 — \
         TMPDIR 을 체크아웃 밖에 두고 다시 돌린다",
        wanted.display()
    ))
}

/// 그 자리에 **정말 만들 수 있는가.** `is_dir` 은 있는지만 답한다 — 있는데 못 쓰는 자리가
/// [`pick_base`] 가 고르는 바로 그 자리들이다.
fn usable(dir: &Path) -> bool {
    let probe = dir.join(format!("moai-probe-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir(&probe);
    let made = std::fs::create_dir(&probe).is_ok();
    if made {
        let _ = std::fs::remove_dir(&probe);
    }
    made
}

/// `dir` 이 어느 체크아웃이나 트래커 안인가. **표식은 둘이다** — `.git` 과 `.moai`(moai-izeo).
///
/// `.git` 만 보던 자는 울타리([`fence`])가 못 막던 바로 그 자리를 놓친다: moai 는 `.git` 이 아니라
/// `.moai` 를 찾아 위로 오르므로(`store::Repo::found_root`), git 이 아닌 moai 프로젝트 밑에 선
/// 임시 자리를 "밖" 이라고 답하고, 그러면 시험이 그 사람의 트래커를 읽고 쓴다. 두 표식을 함께
/// 봐야 [`base`] 의 말이 참이 된다.
///
/// **링크를 푼 자리로 잰다** — `TMPDIR` 이 체크아웃 안을 가리키는 링크면 글자로 된 조상에는
/// `.git` 이 없는데 git 은 실제 자리(`getcwd`)로 위를 찾는다.
pub fn inside_checkout(dir: &Path) -> bool {
    real(dir).ancestors().any(|d| d.join(".git").exists() || d.join(".moai").is_dir())
}

/// [`crate::store::real`] 과 같은 자 — **여기만 제 몸으로 든다.**
///
/// 이 파일은 `tests/cli.rs` 가 `#[path]` 로 함께 들어(dev-dependency 0개, CLAUDE.md "테스트"),
/// 그쪽에서는 `crate` 가 시험 크레이트라 `crate::store` 가 없다. 한 자로 모으는 결정(moai-8csx)이
/// 닿지 못하는 한 자리고, 갈리면 시험 자리가 링크를 달리 푼다 — 그래서 글로 맨다.
fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `dir` 에 아무것도 없는 저장소를 세운다([`Scratch::fenced`] 가 왜 세 개인지 적는다).
///
/// **울타리는 시험 자리마다 치고 그 자리와 함께 치운다.** 한때 판들이 나눠 쓰는 뿌리 하나
/// (`moai-fence`)를 두었는데 주인이 없어 아무도 안 치웠고, 그 체크아웃에 커밋 없는 저장소가 남아
/// `git add -A` 가 거기서 통째로 멈췄다(`does not have a commit checked out`, 에픽 리뷰
/// moai-8mq9.1bw). 자리마다 치면 `Drop` 이 치우고, 남는 것은 판이 죽었을 때뿐이다.
///
/// 부르는 곳은 [`Scratch::fenced_in`] 하나다 — 통합 시험(`tests/cli.rs`)의 `Scratch` 는 moai-izeo
/// 뒤로 [`base`] 만 읽고 울타리를 안 친다.
///
/// **막는 것은 `.git` 훑기뿐이다.** `.moai` 를 찾아 오르는 쪽([`inside_checkout`] 이 두 번째
/// 표식으로 세는 그것)은 여기서 안 선다. 그러니 체크아웃 **안**에 세운 울타리 자리
/// ([`Scratch::fenced_in`] 에 뿌리를 골라 주는 쓰임)에서 `Repo` 를 여는 시험을 쓰면, 그 훑기가
/// 울타리를 지나쳐 돌리는 사람의 진짜 트래커를 잡는다 — moai-595o 가 났던 바로 그 길이다.
/// 여기에 `.moai` 를 같이 심지 않는 것은 그것이 곧 설정을 읽히는 저장소가 되기 때문이고,
/// 그래서 그런 시험은 뿌리를 [`base`] 로 두거나 제 `.moai` 를 손수 세워야 한다.
pub fn fence(dir: &Path) {
    let git = dir.join(".git");
    let fail = |e: std::io::Error| panic!("{}: {e}", git.display());
    for sub in ["objects", "refs"] {
        std::fs::create_dir_all(git.join(sub)).unwrap_or_else(fail);
    }
    std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap_or_else(fail);
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
        // **제 패닉을 잡았는지 본다.** [`base`] 도 글로 패닉해서, 안 보면 자리를 만들지도 못한
        // 판이 "아무것도 안 남았다" 로 지나간다 — 치우는 일을 한 번도 안 재고서.
        assert!(dir.starts_with(base()), "자리를 만들기도 전에 넘어졌다 — {}", dir.display());
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

    /// **체크아웃 안인지는 링크를 푼 자리로 잰다**(moai-boc6). git 이 울타리에서 서는지는
    /// `worktree::tests` 가 본다 — git 은 `git::command` 로만 띄우고, 이 파일은 통합 시험도 읽어
    /// 거기에는 그 모듈이 없다.
    #[test]
    fn a_link_into_a_checkout_is_inside_it() {
        let outer = Scratch::fenced_in(&base(), "outer");
        let tmp = outer.join("tmp");
        std::fs::create_dir(&tmp).unwrap();
        assert!(inside_checkout(&tmp));
        #[cfg(unix)]
        {
            let away = Scratch::in_place(&base(), "link");
            let link = away.join("tmp");
            std::os::unix::fs::symlink(&tmp, &link).unwrap();
            assert!(inside_checkout(&link), "체크아웃 안을 가리키는 링크를 밖으로 읽었다");
        }
    }

    /// **울타리는 자리째 치운다**(moai-8mq9.1bw). 판들이 나눠 쓰던 뿌리는 아무도 안 치워 그
    /// 체크아웃의 `git add -A` 를 멈췄다.
    #[test]
    fn a_fenced_scratch_takes_its_fence_away() {
        let s = Scratch::fenced("fence-drop");
        let dir = s.path().to_path_buf();
        assert!(dir.join(".git/HEAD").is_file(), "울타리를 안 쳤다");
        assert_eq!(dir.parent(), Some(base().as_path()), "자리는 임시 자리 바로 밑이다");
        drop(s);
        assert!(!dir.exists());
    }

    /// **뿌리는 어느 체크아웃 안도 아니다**(moai-izeo). 여기가 무너지면 시험이 바깥 저장소의
    /// `.moai` 를 잡고, 쓰는 시험 하나가 그 트래커를 실제로 더럽힌다.
    ///
    /// 이 줄은 [`base`] 가 이미 본 것을 다시 보는 것이라 혼자서는 못 무너진다 — 무엇을 표식으로
    /// 세는지는 [`a_tracker_above_counts_as_inside_too`] 가, 옮기는 갈래는
    /// [`a_base_inside_a_checkout_moves_out_or_says_why`] 가 잰다. 여기서 재는 것은 그 셋이 실제
    /// 기계에서 한 답으로 모이는가다.
    #[test]
    fn the_base_is_never_inside_a_checkout() {
        assert!(!inside_checkout(&base()), "임시 자리가 체크아웃 안이다 — {}", base().display());
        assert!(!inside_checkout(Scratch::new("outside").path()));
    }

    /// **위의 `.moai` 도 "안" 이다**(moai-izeo). 울타리는 `.git` 만 세웠는데 moai 는 `.moai` 를
    /// 찾아 오르므로, `.git` 만 보는 자는 git 아닌 moai 프로젝트 밑에 선 임시 자리를 "밖" 이라
    /// 답한다 — 그 기계에서 시험이 그 사람의 트래커를 읽고 **쓴다**(moai-595o).
    #[test]
    fn a_tracker_above_counts_as_inside_too() {
        let outer = Scratch::new("tracker-outer");
        let under = outer.join("tmp");
        std::fs::create_dir(&under).unwrap();
        assert!(!inside_checkout(&under), "표식이 하나도 없는데 안이라 했다");
        std::fs::create_dir(outer.join(".moai")).unwrap();
        assert!(inside_checkout(&under), "위의 .moai 를 못 봤다 — 시험이 바깥 트래커를 잡는다");
    }

    /// **옮기는 갈래와 실패하는 갈래를 실제로 돌린다**(moai-izeo). [`base`] 는 `TMPDIR` 을 한
    /// 판에 한 번만 보므로 뿌리로는 이 둘을 못 재고, 그래서 [`pick`] 이 자리를 받는다.
    #[test]
    fn a_base_inside_a_checkout_moves_out_or_says_why() {
        let outer = Scratch::fenced_in(&base(), "pick-outer");
        let inside = outer.join("tmp");
        std::fs::create_dir(&inside).unwrap();
        let away = Scratch::new("pick-away");

        // 밖이면 준 자리를 그대로 쓴다 — 갈 곳을 안 줘도 그렇다.
        assert_eq!(pick(away.path().to_path_buf(), &[]).unwrap(), away.path());
        // 안이면 밖으로 옮긴다. **링크를 푼 철자로** 넘긴다.
        let moved = pick(inside.clone(), &[away.path()]).unwrap();
        assert_eq!(moved, std::fs::canonicalize(away.path()).unwrap(), "링크를 안 풀고 넘겼다");
        // 갈 곳이 못 쓰는 자리뿐이면 그 자리를 건너뛴다 — 있기만 한 것을 보고 옮기지 않는다.
        let gone = outer.join("없다");
        assert!(pick(inside.clone(), &[gone.as_path()]).is_err(), "못 쓰는 자리로 옮겼다");
        // 갈 곳이 다 체크아웃 안이면 크게 실패한다 — 조용히 눌러앉지 않는다.
        let why = pick(inside.clone(), &[inside.as_path()]).unwrap_err();
        assert!(why.contains("TMPDIR"), "고칠 길을 안 댄다 — {why}");
    }
}
