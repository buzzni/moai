//! 요청 하나의 옆길 — 명령이 내는 줄(stdout) 말고 **곁에 남기는 것**을 요청 단위로 모은다(moai-5n6m.m0b).
//!
//! 곁에 남기는 것은 넷이다 — stderr 로 갈 알림 줄(`cmd::tell`·`cmd::tell_after`), 덜 받은 답이라는 깃발
//! (`cmd::note_partial`), 찾기가 체크아웃 밖으로 올라가 잡은 트래커(`store` 의 `CLIMBED`), 못 읽은 저널 자리
//! (`store` 의 `UNREAD`). 명령 하나를 돌리고 끝나는 CLI 는 이것을 프로세스 전역에 담아 `main` 이 끝에 한 번
//! 읽는다. 오래 사는 프로세스(데몬)에서 그 전역은 늘기만 하고, 스레드 둘이 함께 돌면 서로의 줄이 섞인다.
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

/// 걸린 통에 `f` 를 부른다. **통이 없으면 부르지 않고 `None` 이다** — 부르는 쪽이 그때 옛 길로 간다.
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
}
