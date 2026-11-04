//! 요청 하나의 옆길 — 명령이 내는 줄(stdout) 말고 **곁에 남기는 것**을 요청 단위로 모은다(moai-5n6m.m0b).
//!
//! 곁에 남기는 것은 넷이다 — stderr 로 갈 알림 줄(`cmd::tell`·`cmd::tell_after`), 덜 받은 답이라는 깃발
//! (`cmd::note_partial`), 찾기가 체크아웃 밖으로 올라가 잡은 트래커(`store` 의 `CLIMBED`), 못 읽은 저널 자리
//! (`store` 의 `UNREAD`). 명령 하나를 돌리고 끝나는 CLI 는 이것을 프로세스 전역에 담아 `main` 이 끝에 한 번
//! 읽는다. 오래 사는 프로세스(데몬)에서 그 전역은 늘기만 하고, 스레드 둘이 함께 돌면 서로의 줄이 섞인다.
//!
//! **쓰기가 남기는 것도 여기 든다**(moai-3hgm.25e) — 고른 종료 코드(`cmd::exit_with`), 쓰기가 못 읽는 줄을 두고 본
//! 셈(`TALLY`), 못 적은 저널(`MISSED`)과 덧붙일 말(`NOTED`), 워크트리에서 루트로 옮겨 쓴 자리(`MOVED`). `main` 의
//! 꼬리(`crate::run`)가 그 다섯을 끝에 한 번 읽어 stderr 에 내는 것을, 요청은 제 답의 말로 돌려받는다.
//!
//! **그래서 요청마다 제 통([`Side`])을 세운다.** 통은 요청을 부른 쪽([`crate::cmd::request`])이 짓고, 그 부름이
//! 도는 동안 이 스레드에 걸어 둔다([`enter`]). 쓰는 자리는 [`with`] 로 걸린 통을 먼저 찾고, 없으면 옛 길(전역·
//! stderr)로 간다. **CLI 는 통을 안 건다** — 그래서 CLI 의 출력은 이 모듈이 서기 전과 바이트째 같다.
//!
//! **스레드 로컬은 통을 건네는 길일 뿐이다.** 값은 요청의 것이고(`Arc`), 요청 안에서 띄운 실은 통을 물려받는다 —
//! 읽기 길에서 실을 띄우는 자리는 `projects::each` 하나고, 거기서 [`current`] 를 잡아 실마다 [`enter`] 한다.
//! 그 밖에서 띄운 실은 통을 모르고 옛 길로 간다. 걸어 둔 통은 [`Entered`] 가 놓일 때 걷힌다 — 패닉으로
//! 풀려도 걷혀, 같은 스레드의 다음 요청이 앞 요청의 통을 물려받지 않는다.
//!
//! **지금 자리도 통이 든다**([`Side::here`]). 프로세스의 현재 디렉터리는 모든 스레드가 나눠 쓰므로, 여러
//! 프로젝트를 맡는 데몬은 그것을 옮길 수 없다. 통이 걸린 동안 [`cwd`] 는 그 자리를 낸다 — 트래커 찾기
//! (`store::Repo::find`)와 "명령을 친 자리"(`store::invoked_checkout`)가 그것을 읽는다.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 요청 하나가 곁에 남긴 것.
#[derive(Debug, Default)]
pub(crate) struct Side {
    /// 이 요청이 선 자리 — CLI 에서 `moai -C <여기>` 로 부른 것과 같다.
    pub here: PathBuf,
    /// stderr 로 갈 줄, 낸 차례대로(`cmd::tell`).
    pub told: Vec<String>,
    /// 명령이 선 뒤에만 낼 줄(`cmd::tell_after`) — 넘어진 판에서는 버린다.
    pub after: Vec<String>,
    /// 덜 받은 답이다 — CLI 라면 결과를 다 낸 뒤 1 로 끝났다(`cmd::note_partial`).
    pub partial: bool,
    /// 체크아웃 밖으로 올라가 잡은 트래커 — `(선 자리, 잡은 뿌리)`(`store::told_climb`).
    pub climbed: Vec<(PathBuf, PathBuf)>,
    /// 못 읽어 건너뛴 저널 자리, 만난 차례대로(`store::note_missed`).
    pub unread: Vec<crate::store::Unread>,
    /// 명령이 고른 종료 코드 — 0 이면 안 고른 것이다(`cmd::exit_with`, moai-3hgm.25e).
    pub exit: u8,
    /// 쓰기가 못 읽는 줄을 두고 본 것, 저장소마다(`store` 의 `TALLY`, moai-3hgm.25e).
    pub tallies: Vec<(PathBuf, crate::store::Tally)>,
    /// 스냅샷은 담겼는데 저널에 못 적은 까닭 — `(뿌리, 말)`(`store` 의 `MISSED`).
    pub missed: Vec<(PathBuf, String)>,
    /// 쓰기는 담겼고 덧붙일 말 — `(뿌리, 말)`(`store` 의 `NOTED`).
    pub noted: Vec<(PathBuf, String)>,
    /// 딸린 워크트리에서 루트의 트래커로 옮겨 쓴 자리 — `(선 자리, 쓴 트래커)`(`store` 의 `MOVED`).
    pub moved: Vec<(PathBuf, PathBuf)>,
}

/// 걸어 두는 값 — 요청 안에서 띄운 실이 함께 쓰므로 `Arc<Mutex<…>>` 다.
pub(crate) type Shared = Arc<Mutex<Side>>;

thread_local! {
    static HUNG: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

/// 이 스레드에 걸린 통 — 없으면 `None`(CLI).
pub(crate) fn current() -> Option<Shared> {
    HUNG.with_borrow(|h| h.clone())
}

/// **요청 안인가** — 이 스레드에 통이 걸렸는가(리뷰 moai-3hgm.puw 10번). 통은 요청을 부른 쪽만 걸므로 CLI 에서는 늘
/// 거짓이고, 그래서 아래 자리들이 이것으로 가른 갈래는 CLI 에서 서지 않는다.
///
/// 이 물음을 맨 `current().is_some()` 으로 자리마다 다시 적던 판은, 환경이나 프로세스에 기대는 새 자리가 그 검사를
/// 빠뜨리기 쉬웠다. 요청 안에서 다르게 하는 일은 모두 이것으로 묻고, 여기 적는다 — 무엇을 가르는지가 한 자리에 선다.
///
/// - `model::actor` — 데몬의 `MOAI_ACTOR` 를 안 읽는다. 요청을 보낸 사람이 아니다(moai-3hgm.k7s)
/// - `hook::record_own_picks` — 세션의 집기를 안 적는다. 데몬의 `CLAUDE_CODE_SESSION_ID` 는 요청의 것이 아니다
///   (moai-3hgm.25e)
/// - `cmd::refuse_if_flag_like` — 깃발 같은 값과 `-` 한 글자를 재지 않는다. 요청의 필드는 형이 있는 글이고, 그 검사가
///   훑는 argv 는 데몬의 것이다(moai-3hgm.25e)
/// - `cmd::add` 의 `read_source` — 계획 파일을 안 연다. `stdin_text` — stdin 을 안 읽고 거절한다. `dash_reads_stdin` —
///   글 자리의 `-` 를 글로 받는다. 요청에는 stdin 이 없다(moai-3hgm.25e)
pub(crate) fn hung() -> bool {
    HUNG.with_borrow(Option::is_some)
}

/// 걸린 통에 `f` 를 부른다. **통이 없으면 부르지 않고 `None` 이다** — 부르는 쪽이 그때 옛 길로 간다.
///
/// **`f` 안에서 이 모듈을 다시 부르지 않는다** — 통의 자물쇠를 쥔 채라, 같은 스레드에서 [`with`] 를 또 부르면
/// 영영 멈춘다(`cmd::tell`·`cmd::note_partial` 도 이것을 지난다).
pub(crate) fn with<T>(f: impl FnOnce(&mut Side) -> T) -> Option<T> {
    let side = current()?;
    let mut side = side.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut side))
}

/// [`enter`] 가 건 통을 놓을 때 걷는다 — 걸기 전의 것으로 되돌린다.
pub(crate) struct Entered(Option<Shared>);

impl Drop for Entered {
    fn drop(&mut self) {
        let before = self.0.take();
        HUNG.with_borrow_mut(|h| *h = before);
    }
}

/// `side` 를 이 스레드에 건다. 돌려받은 값이 놓일 때 걷힌다. `None` 이면 걸린 것을 걷은 채로 둔다.
pub(crate) fn enter(side: Option<Shared>) -> Entered {
    Entered(HUNG.with_borrow_mut(|h| std::mem::replace(h, side)))
}

/// 지금 자리 — 통이 걸렸으면 그 자리([`Side::here`]), 아니면 프로세스의 현재 디렉터리다.
pub(crate) fn cwd() -> std::io::Result<PathBuf> {
    match with(|s| s.here.clone()) {
        Some(here) => Ok(here),
        None => std::env::current_dir(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **걸린 통은 그 스레드의 것이고, 놓이면 걷힌다** — 패닉으로 풀려도. 걷히지 않으면 같은 스레드의 다음
    /// 요청이 앞 요청의 줄을 물려받는다.
    #[test]
    fn a_hung_side_is_the_threads_own_and_goes_away_when_dropped() {
        assert!(current().is_none());
        let side: Shared = Arc::new(Mutex::new(Side { here: PathBuf::from("/a"), ..Side::default() }));
        {
            let _in = enter(Some(side.clone()));
            assert_eq!(cwd().unwrap(), PathBuf::from("/a"));
            with(|s| s.told.push("one".into()));
            // 옆 스레드는 이 통을 모른다.
            std::thread::spawn(|| assert!(current().is_none())).join().unwrap();
        }
        assert!(current().is_none());
        assert_eq!(side.lock().unwrap().told, vec!["one".to_string()]);

        let caught = std::panic::catch_unwind(|| {
            let _in = enter(Some(Arc::new(Mutex::new(Side::default()))));
            panic!("요청 도중에 죽었다");
        });
        assert!(caught.is_err());
        assert!(current().is_none(), "패닉한 요청의 통이 스레드에 남았다");
    }

    /// **`projects::each` 가 띄운 실도 요청의 통에 담는다** — 읽기 길에서 실을 띄우는 자리가 거기 하나다. 물려주지
    /// 않으면 실이 낸 줄이 요청을 떠나 데몬의 stderr 로 간다(`project.ls` 는 걸어 둔 프로젝트가 둘이면 실을 띄운다).
    #[test]
    fn the_threads_projects_each_starts_inherit_the_side() {
        let side: Shared = Arc::new(Mutex::new(Side::default()));
        {
            let _in = enter(Some(side.clone()));
            crate::projects::each(&[1, 2, 3], |n| crate::cmd::tell(&format!("line {n}")));
        }
        let mut told = side.lock().unwrap().told.clone();
        told.sort();
        assert_eq!(told, ["line 1", "line 2", "line 3"], "실의 줄이 통을 떠났다");
    }
}
