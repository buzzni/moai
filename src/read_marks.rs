//! 적어 둔 읽음 — **프로젝트마다 제 파일**(moai-f31d, 사용자 결정 2026-09-19).
//!
//! 이슈 id → 마지막으로 본 줄의 `updated_at`(moai-50mn·moai-lyc1). 한때 사용자 설정 한 파일의 `[read]`
//! 표에 모두 모았는데, 그 자리가 둘을 깨뜨렸다.
//!
//! - **키가 이슈 id 뿐이라 저장소가 섞였다**(moai-omx7). 접두어는 디렉터리 이름에서 오고 뿌리 몸통은
//!   base36 넉 자라, 이름이 같은 디렉터리 둘은 같은 공간에서 id 를 뽑는다 — 각 300줄이면 5% 남짓
//!   겹쳐, 한쪽에서 읽은 것이 다른 저장소의 같은 id 를 말없이 읽음으로 만들었다
//! - **표가 안 줄어 설정 쓰기마다 느려졌다**(moai-dt5q). 보기 토글·`project add` 처럼 읽음과 아무
//!   상관 없는 쓰기도 그 표를 통째로 읽고 되썼다 — 1만 항목에 35~100ms
//!
//! 프로젝트마다 파일을 가르면 둘이 함께 풀린다. 파일이 갈리니 키는 지금처럼 이슈 id 로 두고, 설정
//! 쓰기는 이 파일을 아예 안 만진다.
//!
//! **트래커에 안 쓴다는 옛 결정은 그대로다**(사용자 결정 2026-09-15) — 읽음은 사람마다 다른 값이라
//! `.moai/issues.jsonl` 에 적으면 읽기만 해도 남과 부딪히고, 남의 읽음이 내 diff 에 섞인다. 자리가
//! 설정 한 파일에서 그 곁의 디렉터리로 바뀔 뿐이다.
//!
//! **옛 `[read]` 는 겹쳐 보기만 한다**(사용자 결정 3) — 옮기지 않는다. 그 표는 이슈 id 로만 키를 잡으니
//! 위의 섞임이 **옛 줄에 한해** 그대로 남는다. 새로 적는 것은 모두 제 파일로 가므로 섞임은 자라지 않지만,
//! 사라지지도 않는다 — 걷으려면 그것은 설정이 아니라 마이그레이션이라, 그 값을 따로 물어야 한다.
//!
//! **락과 쓰기는 [`crate::user_config::update`] 보다 단순하다.** 그쪽은 링크 자리에 `rename` 하지 않으려고
//! 설정 **파일**을 풀어 그 자리에 락까지 하나 더 잡는데, 이 파일은 도구가 짓는 것이라 링크일 수 없어
//! 락이 하나다. 다만 **자리를 고를 때는 그 파일을 푼다**([`place_of`], moai-f5e3) — 같은 설정이 XDG 기본과
//! dotfiles 링크 두 철자로 불리면 읽음 파일이 둘로 갈리기 때문이다. 문서 자체는
//! [`crate::user_config::Doc`] 을 그대로 쓴다([`Sheet`]) — 바이트를 지키는 자를 둘로 두지 않는다.

use crate::fail::{Fail, R};
use crate::store::{Lock, dir_of, lock_beside, write_atomic};
use crate::user_config::{Doc, refuse, write_value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{Item, Table};

/// 읽음이 사는 표의 이름. 옛 `[read]` 와 같은 낱말이다 — 자리가 바뀌었을 뿐 뜻은 그대로다.
const READ: &str = "read";
/// 이 파일이 어느 프로젝트의 것인지 적는 키([`path_for`] 의 해시가 부딪혔는지 여기서 본다).
const PATH: &str = "path";

/// 같은 자리를 가리키는 철자를 **하나로 모은다** — 링크와 `..`·끝 `/` 를 걷는다(`canonicalize`).
/// 못 풀면 받은 철자로 떨어진다(moai-f5e3, 사용자 결정 2026-09-20): 자리를 못 고르는 것보다 낫다 —
/// 지워진 뿌리를 가리키는 등록 줄 하나가 `moai read` 를 통째로 멈추면 안 된다.
///
/// **떨어진 판의 도장은 버려지지 않는다.** 도구가 짓는 대기 자리에 남고([`spool_at`]), 다음 성한 판이
/// 그것을 합치고 지운다(moai-bdej).
///
/// **없는 것은 탈이 아니다.** 설정 파일도 읽음 파일도 처음에는 없다 — `NotFound` 는 조용히 지나간다.
fn settle(path: &Path) -> (PathBuf, Option<String>) {
    match std::fs::canonicalize(path) {
        Ok(real) => (real, None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (path.to_path_buf(), None),
        Err(e) => (path.to_path_buf(), Some(format!("{}: 자리를 못 풀어 적힌 철자로 든다 — {e}", path.display()))),
    }
}

/// 읽음 파일의 이름 — `<설정 디렉터리>/read/<뿌리 해시>.toml`.
///
/// **이름을 세는 자는 하나다**(리뷰). 시험이 이 식을 저마다 옮겨 적던 판은, 이름을 바꾸는 순간 모든
/// 사람의 읽음이 한 번에 사라지는데 시험은 옛 이름을 그대로 재며 푸르게 섰다.
fn sheet_at(dir: &Path, root: &Path) -> PathBuf {
    dir.join("read").join(format!("{:016x}.toml", crate::text::fnv1a64(root.as_os_str().as_encoded_bytes())))
}

/// **못 푼 판이 적는 대기 자리** — `<설정 디렉터리>/read/<받은 철자 해시>.pending.toml`.
///
/// [`settle`] 이 자리를 못 풀면 그 판의 도장은 갈 곳이 없다. 받은 철자의 **읽음 파일**에 적던 판이
/// moai-bdej 였고, 그 파일은 옛 바이너리가 쓴 파일과 이름이 같아 "아직 안 합쳤다" 를 그 존재로 물을
/// 수 없었다 — 파일의 때로 물었더니 지금 자리에 아무 쓰기나 들면 창이 영영 닫혔다(리뷰 3·4·6·7).
///
/// **그래서 대기 자리는 도구가 짓고 도구가 지운다**(사용자 결정 2026-09-20, 리뷰 뒤 고친 판).
/// 있는가가 곧 "아직 안 합쳤다" 이고, 다음 성한 쓰기가 합치고 지워 닫는다 — 닫는 자가 합치기 자신이라
/// 때도 필드도 필요 없다. 옛 바이너리가 쓴 파일([`Place::past`])은 이름이 달라 섞이지 않고, 그것을
/// 지우지도 옮기지도 않는다는 결정은 그대로 산다.
fn spool_at(dir: &Path, root: &Path) -> PathBuf {
    dir.join("read").join(format!("{:016x}.pending.toml", crate::text::fnv1a64(root.as_os_str().as_encoded_bytes())))
}

/// 이 부름이 **읽고 쓸 자리**와 그 자리를 고른 뿌리, 그리고 고르다 만난 까닭.
///
/// **고르는 자는 하나다**([`place_of`]·[`at_of`] 가 함께 쓴다). 갈라 두던 판은 못 푼 자리에서 둘이
/// 다른 파일을 댔다 — 쓰기는 대기 자리로 가는데 표식만 재는 쪽은 읽음 파일을 재, 탐색기가 제가 적은
/// 것을 못 보았다.
fn chosen(dir: &Path, root: &Path) -> (PathBuf, PathBuf, Option<String>) {
    match settle(root) {
        // 못 풀었다 — 이 판의 도장은 대기 자리로 간다. 뿌리는 받은 철자다(문지기와 `claim` 이 그것으로 견준다).
        (_, Some(why)) => (spool_at(dir, root), root.to_path_buf(), Some(why)),
        (real, None) => (sheet_at(dir, &real), real, None),
    }
}

/// 한 프로젝트의 읽음이 사는 자리들.
pub struct Place {
    /// 읽고 **쓰는** 자리. 하나다.
    pub at: PathBuf,
    /// [`Place::at`] 을 고른 **푼 뿌리**. 문지기([`Sheet::owns`])와 [`Sheet::claim`] 이 이것으로 견주고
    /// 적는다 — **이름을 고른 값과 파일에 적는 값을 가르지 않는다**(리뷰). 갈라 두던 판은 받은 철자를
    /// `path` 에 적어, 그 철자가 가리키던 링크가 사라지는 날 도구가 제가 지은 파일을 남의 것으로 읽었다.
    pub root: PathBuf,
    /// **옛 철자로 선 파일들 — 읽기만 한다.** 쓰는 자리를 둘로 두면 이 에픽이 고친 섞임이 그대로
    /// 돌아온다(moai-omx7).
    ///
    /// **이 부름이 받은 철자의 것만 찾는다.** 표면마다 제 철자로 부르므로(한눈 보기는 등록 줄, CLI 는
    /// `current_dir`) 저마다 제 옛 파일을 만난다. 아무 철자나 찾아 주지는 않는다 — 그러려면 `read/` 를
    /// 통째로 훑어 파일마다 `path` 를 견줘야 하고, 그 값을 읽기마다 치른다. **저장소 이름을 바꾼 것은
    /// 여기서도 못 잇는다**: 옛 파일의 `path` 가 옛 이름을 가리켜 어느 철자로도 안 풀린다. 그것은
    /// 지우지 않기로 한 결정의 대가다(사용자 결정 2026-09-20).
    ///
    /// **처음 짓는 파일일 때만 합친다**([`overlay_place`]·[`update`]). 그 한 번에 이 표가 지금 자리로
    /// 들어오고, 그 뒤로는 읽기도 쓰기도 여기를 안 연다 — 읽기마다 겹쳐 보던 판이 낳은 둘(리뷰 7·13)은
    /// 그대로 닫혀 있다: 걷기가 지금 자리만 줄여 걷은 id 가 다음 읽기에 되살아났고(moai-dt5q 가 내건
    /// 것이 옛 자리를 가진 사람에게만 꺼졌다), 그 읽기 값을 내내 치렀다.
    ///
    /// **못 푼 판의 도장은 여기로 안 온다**(moai-bdej, 리뷰 뒤 고친 판) — 그것은 [`spool_at`] 의 대기
    /// 자리로 가고 [`Place::pending`] 이 그것을 든다. 한때 이 자리로 보내고 파일의 때로 "아직 안
    /// 합쳤다" 를 물었는데, 그 물음은 닫히지 않았다: 지금 자리에 아무 쓰기나 들면 때가 올라가 그
    /// 도장이 영영 남았고(리뷰 3), 걷은 id 가 되살아나는 자리도 함께 열렸다(리뷰 6).
    pub past: Vec<PathBuf>,
    /// **아직 안 합친 대기 자리**([`spool_at`]) — 있으면 [`update`] 가 합치고 **지운다.**
    ///
    /// 못 푼 판에서는 `None` 이다: 그때는 [`Place::at`] 이 곧 그 자리라 제 자신에 겹칠 일이 없다.
    ///
    /// **가르는 자는 있는가 하나다.** 필드도 때도 아니라서 되풀이해 떨어져도 같은 답을 내고, 지운
    /// 뒤에는 어느 읽기도 다시 안 연다 — 걷은 id 가 되살아나던 자리(리뷰 7·13, moai-dt5q)는 그대로
    /// 닫혀 있다.
    pub pending: Option<PathBuf>,
    /// 자리를 고르다 만난 까닭.
    pub problems: Vec<String>,
}

/// 그 프로젝트의 읽음 파일 — `<설정 디렉터리>/read/<뿌리 해시>.toml`.
///
/// **설정 곁에 둔다.** `MOAI_CONFIG` 가 가리키는 자리를 따라가므로 시험과 격리가 설정 하나만 옮기면
/// 읽음도 함께 옮겨 간다. 자리를 따로 정하면 그 둘이 갈려, 시험이 제 임시 설정을 주고도 돌리는 사람의
/// 읽음을 읽는다.
///
/// **이름은 뿌리 경로의 해시다.** 경로를 그대로 파일 이름에 넣으면 깊은 경로가 이름 길이 상한을 넘고,
/// 글자를 걷어 짧게 만들면 서로 다른 뿌리가 한 이름으로 모여 지금 고치는 바로 그 섞임이 돌아온다.
/// 해시가 부딪혔는지는 파일 안의 `path` 로 본다([`Sheet::owns`]) — 글자로 적어 두고 견준다.
///
/// 셈은 [`crate::text::fnv1a64`] 다. **표준 해셔를 안 쓴다** — `DefaultHasher` 는 러스트 버전마다 값이
/// 달라질 수 있다고 문서가 밝혀, 올리기 한 번에 모든 사람의 읽음 파일 이름이 바뀐다. 상수와 그 까닭은
/// 이미 `text` 에 한 벌 있어 여기 다시 적지 않는다(moai-2vrw 가 못박은 그 값이다, 리뷰).
///
/// **뿌리는 푼 경로로 세고, 설정 자리는 받은 철자 그대로 둔다**(moai-f5e3).
///
/// 뿌리를 푸는 까닭: `Repo::open` 은 받은 철자를 그대로 뿌리로 들어 같은 저장소가 `/w/b` 와 `/w/b/`
/// 로, `cd` 한 자리와 등록한 줄로 갈린다 — 철자를 세던 판은 그때마다 읽음 파일이 둘로 갈려 서로를
/// 못 본 채 저마다 걷었다. 이 멤버가 고치는 해가 그것이다.
///
/// **설정 자리를 안 푸는 까닭은 더 무겁다**(리뷰가 잰 것, 2026-09-20). 한때 설정 **파일**을 풀어 두
/// 철자를 한자리로 모았는데, `~/.config/moai/config.toml` 이 dotfiles 로 걸린 사람(흔한 꼴이다)은
/// `read/` 가 통째로 그 저장소 안에 섰다 — 지은 바이너리로 재 보니 `dotfiles/moai/read/….toml` 과 그
/// 락이 거기 생겼다. `~/.config` 자체가 링크인 사람도 같다. 무엇을 읽었는지는 사람마다 다른 사적인
/// 값이고, **남의 읽음이 내 diff 에 섞이지 않게 하려고** 트래커 밖에 둔 것이 이 모듈의 첫
/// 결정이다(2026-09-15) — 그것을 버전 관리되는 저장소에 넣으면 그 결정이 통째로 뒤집힌다.
///
/// 그 대가로 XDG 기본과 `MOAI_CONFIG` 를 번갈아 쓰는 사람의 읽음은 두 자리로 갈린 채다. 잃는 것은
/// 없고(둘 다 남는다) 같은 철자로 부르면 제 것을 본다. 갈라짐이 흔하고 해가 큰 축은 뿌리고, 설정 축은
/// 그 반대다 — 그래서 한쪽만 푼다.
///
/// 옛 철자로 이미 선 파일은 [`Place::past`] 로 **읽기만** 한다 — 지우지도 옮기지도 않으니
/// 마이그레이션이 아니고 읽음을 잃는 사람도 없다(옛 `[read]` 를 겹쳐 보는 것과 같은 꼴이다).
pub fn place_of(config: &Path, root: &Path) -> Place {
    let dir = dir_of(config);
    let (at, root_of, why) = chosen(dir, root);
    // 못 푼 판에서는 `at` 이 곧 대기 자리다 — 옛 자리도 대기 자리도 안 든다(제 자신에 겹칠 일이 없고,
    // 옛 철자 파일은 그 판에서 이름이 `at` 과 같아 이미 열려 있다).
    if why.is_some() {
        return Place { at, root: root_of, past: Vec::new(), pending: None, problems: why.into_iter().collect() };
    }
    // 옛 자리는 **뿌리 축 하나**다. 등록한 줄은 이미 푼 경로라(`user_config::resolve_dir`) 흔한 판은
    // 두 철자가 같고, 그때 이 줄은 통째로 `at` 으로 접힌다.
    //
    // **뿌리는 바이트로 가른다** — 이름이 바이트 해시라 끝 `/` 하나가 딴 파일을 짓는데, `Path` 의
    // 견줌은 그 둘을 같다고 읽어 정작 찾아야 할 옛 파일을 건너뛴다.
    let past = (root_of.as_os_str() != root.as_os_str()).then(|| sheet_at(dir, root)).into_iter().collect();
    // 대기 자리는 **받은 철자**의 것이다 — 떨어진 판도 같은 철자로 불려 같은 이름을 지었다. 표면마다
    // 제 철자로 부르므로(한눈 보기는 등록 줄, CLI 는 `current_dir`) 저마다 제 대기 자리를 만난다.
    Place { at, root: root_of, past, pending: Some(spool_at(dir, root)), problems: Vec::new() }
}

/// 지금 자리의 표에 **대기 자리와 옛 자리와 옛 `[read]`** 를 차례로 얹는다 — [`read`] 와 탐색기의
/// `r`([`overlay_older`])이 한 줄로 쓴다.
///
/// **고르는 것과 얹는 것을 한자리에 둔다**(리뷰). 얹는 차례를 [`older`] 하나로 모은 까닭이 두 표면이
/// 저마다 세다 갈라진 것이었는데(moai-f5e3, 리뷰 7·13), 고르는 쪽을 부르는 쪽마다 적으면 그 갈라짐이
/// 반만 닫힌다.
///
/// **가르는 자는 둘이고 서로 다르다.**
///
/// - **대기 자리는 있으면 든다**([`Place::pending`]). 그것이 곧 "아직 안 합쳤다" 이고, 합친 쓰기가
///   지워 닫는다. 읽기는 아무것도 안 지우니 그 사이의 읽기마다 얹는데, 그 값은 합칠 것이 생긴
///   동안만 든다
/// - **옛 자리는 지금 자리가 아직 없을 때만 든다**(리뷰 7·13). 첫 쓰기가 그 표를 여기로 합치므로
///   파일이 선 뒤에는 옛 자리에 새로 든 것이 없다 — 그때도 겹쳐 보던 판은 걷은 id 를 다음 읽기에
///   되살렸고(moai-dt5q 가 내건 것이 옛 자리를 가진 사람에게만 꺼졌다) 그 읽기 값을 내내 치렀다
fn overlay_place(
    place: &Place,
    seen: &mut BTreeMap<String, String>,
    legacy: &BTreeMap<String, String>,
    problems: &mut Vec<String>,
) {
    let mut places: Vec<&Path> = Vec::new();
    // **대기 자리가 먼저다** — 떨어진 판이 옛 자리보다 나중에 적힌 것이다. 겹치는 차례는 나중에 적힌
    // 것이 이기는 쪽으로 세운다(`overlay` 는 먼저 든 것을 안 덮는다).
    if let Some(spool) = place.pending.as_deref().filter(|p| p.exists()) {
        places.push(spool);
    }
    if !place.at.exists() {
        places.extend(place.past.iter().map(PathBuf::as_path));
    }
    older(&places, &place.root, seen, legacy, problems);
}

/// 읽고 쓰는 자리와 그 자리를 고른 푼 뿌리 — **옛 자리는 안 센다.** 쓰는 쪽([`update`])과 표식만 재는
/// 쪽(`tui::App::follow_read` 은 걸음마다 부른다)은 `past` 를 안 쓰는데, 짓고 버리면 그만큼이 헛일이다(리뷰).
fn at_of(config: &Path, root: &Path) -> (PathBuf, PathBuf) {
    let (at, root_of, _) = chosen(dir_of(config), root);
    (at, root_of)
}

/// 읽고 쓰는 자리 하나([`Place::at`]).
pub fn path_for(config: &Path, root: &Path) -> PathBuf {
    at_of(config, root).0
}

/// 읽어 낸 읽음과 그때 생긴 말.
///
/// **못 든 것과 한 줄 건너뛴 것을 가른다**(리뷰). 둘을 `problems` 하나로 내던 판은 부르는 쪽이 셋 다
/// 틀리게 읽었다 — 탐색기는 낱말이 아닌 줄 하나에 성한 표를 통째로 버려 내 줄이 모두 [NEW] 로 섰고,
/// CLI 는 못 읽은 파일을 조용히 빈 표로 지나갔다. 가르는 낱말은 [`crate::user_config::Trouble`] 과
/// **같은 것**이다 — 설정이 이미 그것들을 그 이름으로 가르고(`moai-9p7v`), 둘을 두면 한쪽만 고쳐진다.
pub struct Marks {
    /// 이슈 id → 마지막으로 본 줄의 도장. 옛 `[read]` 를 겹쳐 본 값이다([`overlay`]).
    pub seen: BTreeMap<String, String>,
    /// 사람에게 댈 까닭. `trouble` 이 서 있으면 **표를 못 든 까닭**이고, 없으면 건너뛴 줄의 까닭이다.
    pub problems: Vec<String>,
    /// **표를 못 들었는가.** 갈래마다 다시 읽을 때가 다르고, 그것을 [`crate::user_config::Trouble::again`]
    /// 하나가 답한다(moai-po6v) — `Reading` 은 잠깐(`ESTALE`·`EIO`)이라 다음 걸음에, `Unreadable`
    /// (`EACCES`·디렉터리·UTF-8 아닌 바이트)은 다시 해도 같지만 고친 것이 표식을 안 바꿔 시계로,
    /// `Broken` 은 사람이 고치면 파일이 바뀌어 표식에 맡긴다. 이때 `seen` 에는 **지금 자리의 것이 없다** —
    /// 옛 `[read]` 와, 있으면 대기 자리([`Place::pending`])에서 든 것뿐이다. 그래서 부르는 쪽은 들고 있던
    /// 것을 둔다 — 빈 표든 반쪽 표든 들이면 내 줄이 [NEW] 로 선다.
    ///
    /// **"옛 `[read]` 밖에 없다" 로 적던 판은 낡았다**(리뷰). 무엇이 함께 서는지는 [`overlay_place`] 가
    /// 가르니 그 수를 글로 못박지 않는다 — 못박아 두면 그 글을 믿는 다음 사람이 반쪽 표를 성한 것으로
    /// 들인다.
    ///
    /// **`Gone` 은 이 읽기가 혼자 세우지 않는다** — 없는 읽음 파일은 "아직 이 프로젝트를 안 읽었다"
    /// 는 정상이고, 그것만 보고는 홈이 끊긴 것과 갈릴 수 없다. 가르는 자는 **곁의 설정도 사라졌는가**
    /// 이고, 그것을 아는 쪽은 부르는 탐색기다 — 세우는 자리는 `App::read_marks_of` 다
    /// (moai-4qbv.i0g 리뷰). 읽음 파일은 설정 파일 곁의 디렉터리에 살아 둘이 함께 사라진다.
    pub trouble: Option<crate::user_config::Trouble>,
}

/// 그 프로젝트의 읽음. **실패하지 않는다** — 못 읽거나 깨졌으면 [`Marks::trouble`] 과 까닭 한 줄이다.
/// 읽음 하나 때문에 제 저장소를 보던 명령이 넘어지면 도구가 고장 난 것으로 보인다
/// ([`crate::user_config::read`] 와 같은 자다).
///
/// **낱말이 아닌 줄 하나는 못 든 것이 아니다.** 그 줄만 건너뛰고 나머지는 그대로 낸다([`read_table`]) —
/// `trouble` 은 안 선다. 옛 `[read]` 도 같은 자로 읽힌다(`look_problems` 가 `trouble` 을 안 세우는 그것).
///
/// **옛 `[read]` 를 겹쳐 본다**(사용자 결정 3, 2026-09-19). 옮기지 않고 읽기만 한다 — 옛 줄을 건드리면
/// 그것은 설정이 아니라 마이그레이션이고, 그 사이 도는 옛 바이너리나 옆 세션이 읽음을 잃는다. 겹칠
/// 때는 **이 파일이 이긴다**: 옛 표는 이 바이너리가 다시 안 적는 지나간 값이고, 새 자리의 값이 그 뒤에
/// 적힌 것이다.
pub fn read(config: &Path, root: &Path, legacy: &BTreeMap<String, String>) -> Marks {
    let mut place = place_of(config, root);
    let mut marks = read_one(&place.at, &place.root);
    // **자리를 고르다 만난 까닭은 뒤에 붙인다**(리뷰). 앞에 두던 판은 `problems.first()` 하나만 보는
    // 탐색기가 표를 못 든 진짜 까닭 대신 이것을 대고, 낱말까지 "이상한 줄" 로 어긋났다.
    marks.problems.append(&mut place.problems);
    // 대기 자리와 옛 자리와 옛 `[read]` — 가르는 자는 [`overlay_place`] 에 있다. **읽기는 아무것도
    // 안 지운다**: 대기 자리를 닫는 것은 쓰기이고, 그때까지는 읽기마다 얹어 화면이 그 도장을 든다.
    overlay_place(&place, &mut marks.seen, legacy, &mut marks.problems);
    marks
}

/// 지금 자리의 표 위에 **옛 자리들을 차례대로** 얹는다 — 옛 철자로 선 파일, 그다음 설정의 옛 `[read]`.
///
/// **겹치는 차례는 한 함수에 있다**(moai-f5e3, 리뷰). [`read`] 와 탐색기의 `r`([`overlay_older`])이 저마다
/// 세던 판은 이미 갈려 있었다 — `r` 한 번이 옛 철자 파일의 읽음을 화면에서 지워 그 줄이 [NEW] 로 돌아왔고,
/// 그 판에 적을 것이 없으면(이미 읽은 줄) 파일이 안 바뀌어 다음 걸음도 다시 안 읽어 그대로 남았다.
///
/// **옛 자리의 탈은 `trouble` 로 안 올린다.** 그 파일은 있을 수도 없을 수도 있는 것이라, 그것 하나로
/// 지금 자리의 성한 표를 버리면 안 된다 — 까닭만 곁들인다.
fn older(
    past_places: &[&Path],
    root: &Path,
    seen: &mut BTreeMap<String, String>,
    legacy: &BTreeMap<String, String>,
    problems: &mut Vec<String>,
) {
    for old in past_places {
        let mut past = read_one(old, root);
        overlay(seen, &past.seen);
        problems.append(&mut past.problems);
    }
    overlay(seen, legacy);
}

/// 락 안에서 든 표에 [`read`] 와 **같은 차례로** 옛 자리와 옛 `[read]` 를 얹는다 — 탐색기의 `r` 이 제가
/// 방금 쓴 표를 화면에 들일 때 이것으로 든다. 까닭은 안 낸다: 그 자리는 쓴 결과를 말하지 읽기를 말하지
/// 않고, 같은 까닭은 다음 걸음의 [`read`] 가 댄다.
pub fn overlay_older(
    config: &Path,
    root: &Path,
    seen: &mut BTreeMap<String, String>,
    legacy: &BTreeMap<String, String>,
) {
    overlay_place(&place_of(config, root), seen, legacy, &mut Vec::new());
}

/// 읽음 파일 **하나**를 읽는다. 겹치는 일은 [`read`] 가 한다. `root` 는 [`Place::root`] — 자리를 고른
/// 그 푼 뿌리다. 여기서 다시 풀지 않는다(리뷰): 파일마다 같은 값을 다시 세면 읽기 한 번이 `canonicalize`
/// 를 열 번 부르고, 그 사이 자리가 바뀌면 이름을 고른 자와 문지기가 서로 다른 뿌리를 본다.
fn read_one(path: &Path, root: &Path) -> Marks {
    use crate::user_config::Trouble;
    // **까닭에는 어느 파일인지를 붙인다** — 이름이 뿌리의 해시라 사람이 짐작할 수 없어, 안 붙이면
    // "손으로 고친다" 가 갈 곳 없는 말이 된다([`update`] 가 거절문에 붙이는 것과 같은 자다, 리뷰).
    let at = |why: String| format!("{}: {why}", path.display());
    let (seen, problems, trouble) = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (BTreeMap::new(), Vec::new(), None),
        // **가르는 잣대는 설정과 한 자다**(moai-po6v, `user_config::unreadable`) — 갈라 두면 같은
        // `Trouble` 을 두 곳이 달리 읽어, 한쪽을 고친 날 다른 쪽이 조용히 옛 뜻으로 남는다.
        // 권한으로 못 읽는 것은 다시 해도 같다 — 다시 읽을 때는 `App::take_read` 가 갈래로 정한다.
        Err(e) if crate::user_config::unreadable(&e) => {
            (BTreeMap::new(), vec![at(e.to_string())], Some(Trouble::Unreadable))
        }
        Err(e) => (BTreeMap::new(), vec![at(e.to_string())], Some(Trouble::Reading)),
        Ok(src) => match Sheet::parse(&src) {
            Err(e) => (BTreeMap::new(), vec![at(e)], Some(Trouble::Broken)),
            Ok(sheet) if !sheet.owns(root) => (
                BTreeMap::new(),
                vec![at(format!("{} 의 읽음이 아니라 안 읽는다 — 손으로 지운다", root.display()))],
                Some(Trouble::Broken),
            ),
            Ok(sheet) => {
                let (seen, problems) = sheet.marks();
                (seen, problems.into_iter().map(at).collect(), None)
            }
        },
    };
    Marks { seen, problems, trouble }
}

/// 옛 `[read]` 를 겹친다 — **새 자리가 이긴다**(사용자 결정 3). 겹치는 자가 둘이 되면 그 규칙도 둘이
/// 되고, 옛 표를 걷는 날 한쪽만 걷힌다(리뷰).
///
/// 이미 있는 id 는 **키를 안 짓는다** — `entry` 로 넘기면 1만 항목짜리 옛 표가 읽기마다 버릴 문자열
/// 1만 개를 짓는다.
pub fn overlay(marks: &mut BTreeMap<String, String>, legacy: &BTreeMap<String, String>) {
    for (id, when) in legacy {
        if !marks.contains_key(id) {
            marks.insert(id.clone(), when.clone());
        }
    }
}

/// `[read]` 표를 **관대하게** 읽는다 — 낱말이 아닌 값은 까닭 한 줄로 대고 건너뛴다. 표가 없으면 빈 표다.
///
/// **읽는 자를 하나로 둔다**(리뷰) — 옛 `[read]`([`crate::user_config::Doc::read_marks`])와 이 파일이 같은
/// 모양이라, 둘이 저마다 읽으면 모양이 자라는 날 한쪽만 따라가고 그 한쪽은 남은 읽음을 조용히 버린다.
pub(crate) fn read_table(root: &Table) -> (BTreeMap<String, String>, Vec<String>) {
    let mut problems = Vec::new();
    let Some(item) = root.get(READ) else {
        return (BTreeMap::new(), problems);
    };
    let Some(t) = item.as_table_like() else {
        problems.push(format!("`{READ}` 는 `[{READ}]` 표여야 한다 — 지금은 {}", item.type_name()));
        return (BTreeMap::new(), problems);
    };
    let mut marks = BTreeMap::new();
    for (id, at) in t.iter() {
        match at.as_str() {
            Some(when) => {
                marks.insert(id.to_string(), when.to_string());
            }
            None => problems.push(format!("`{READ}.{id}` 는 때를 적은 낱말이어야 한다 — 지금은 {}", at.type_name())),
        }
    }
    (marks, problems)
}

/// 읽음을 고치는 **유일한 길**. 락 → 락 안에서 읽기 → 고치기 → 바뀌었으면 temp+rename.
/// [`crate::user_config::update`] 와 같은 모양이고 같은 까닭이다 — 둘이 동시에 적으면 나중에 `rename`
/// 한 쪽이 앞의 읽음을 조용히 지운다.
///
/// **깨진 파일에는 쓰지 않는다**(`broken`). 도구가 짓는 파일이라 깨질 일이 드물지만, 깨졌으면 사람이
/// 볼 수 있게 두고 멈춘다 — 덮으면 그 안의 읽음이 통째로 사라진다.
///
/// **자리를 고르다 만난 까닭은 값에 실어 돌려준다**([`Wrote::problems`], moai-ajh2). 읽는 길은 그것을
/// 이미 대는데([`Marks::problems`]) 쓰는 길만 버리던 판은, 뿌리 윗자리에 잠깐 `EACCES`·`ESTALE`·`ELOOP`
/// 가 난 그 한 번이 옛 철자 자리에 적고(락도 딴 자리다) 아무 말 없이 0 으로 끝났다.
///
/// **건너뛴 줄도 같은 자리에 싣는다**(moai-upna). 자리 고르기만 싣던 판은 [`Sheet::marks`] 가 건너뛴
/// 줄을 그대로 버렸다 — 읽는 길([`read_one`])은 그것을 대므로 같은 파일을 두 길이 달리 읽었고, 그
/// 줄은 견줌에서도 빠져 쓰는 길이 "이미 읽었는가" 를 그 줄 없이 쟀다.
///
/// **그 도장은 다음 성한 쓰기가 합치고 지운다**(moai-bdej, 사용자 결정 2026-09-20). 떨어진 판은 도구가
/// 짓는 대기 자리([`spool_at`])에 적고, 자리가 다시 풀리는 판이 그것을 [`Place::pending`] 으로 들어
/// 여기서 합친 뒤 그 파일을 지운다 — **닫는 자가 합치기 자신이다.** 그러니 이 줄은 "철자가 밀렸다" 가
/// 아니라 "이 판의 도장이 아직 대기 자리에 있다" 로 읽는다.
///
/// 첫 판은 받은 철자의 **읽음 파일**에 적고 파일의 때로 "아직 안 합쳤다" 를 물었는데, 그 물음은 닫히지
/// 않았다(리뷰 3·4·6·7) — 지금 자리에 아무 쓰기나 들면 때가 올라가 그 도장이 영영 남았고, 옛 파일과
/// 이름이 같아 지우거나 옮겨 닫을 수도 없었다.
///
/// 떨어지는 길 자체는 2026-09-20 사용자 결정이라 여기서 안 뒤집는다. 쓰기만 거절하는 길
/// (규약의 "읽기는 관대하고 쓰기는 엄하다")은 `ESTALE` 이 잠깐 나는 기계에서 `moai read` 가 가끔
/// 지는 값을 치르는데, 합치는 길은 그 값을 안 치르고도 도장을 지킨다.
///
/// **어디로 내는지는 부르는 쪽이 정한다**: 이 모듈은 아무것도 안 찍는 자라([`crate::report`]·
/// [`crate::query`] 와 같은 약속) 여기서 찍으면 탐색기의 화면에 stderr 한 줄이 끼어든다.
///
/// **아직 없는 파일은 짓기 전에 묻는다**(moai-dyb7). 디렉터리와 락을 먼저 짓고 `f` 를 부르던 판은,
/// 적을 것이 없는 `moai read <id>` 하나가 아직 아무것도 안 읽은 사람의 집에 `read/` 와 0바이트
/// `<해시>.toml.lock` 을 남겼다 — 프로젝트마다 하나씩 쌓이고 아무도 안 치운다. 그래서 파일이 없는
/// 판에서만 **락 밖에서 한 번 재 보고**, 쓸 것이 없으면 아무것도 안 짓고 돌아선다.
///
/// **치우는 길로 안 간다.** 쥔 락 파일을 지우면 기다리던 갈래가 지워진 그 아이노드를 잡고, 그다음에 온
/// 갈래는 새로 지은 파일을 잡아 둘이 함께 쓴다 — 조용한 손실이라 못 견딘다(CLAUDE.md). 남는 값은
/// 이쪽이 싸다: 없는 파일을 읽는 것은 파싱할 것이 없고, 파일이 이미 있으면 디렉터리도 락도 그 곁에
/// 이미 선 세간이라 새로 남는 것이 없다.
///
/// 그래서 `f` 는 **두 번 돌 수 있다** — 재 보기 한 번, 락 안의 진짜 쓰기 한 번. 준 [`Sheet`] 밖에
/// 자국을 남기면 안 된다.
pub fn update<T>(config: &Path, root: &Path, f: impl Fn(&mut Sheet) -> R<T>) -> R<Wrote<T>> {
    // **자리는 락 밖에서 고른다.** `canonicalize` 는 락이 필요 없는데, 안에서 하면 쓰는 이마다 그만큼
    // 더 기다린다(리뷰). 푼 뿌리를 함께 받아 문지기와 [`Sheet::claim`] 에 그대로 넘긴다 — 여기서 다시
    // 풀면 이름을 고른 값과 견주는 값이 갈린다.
    let place = place_of(config, root);
    let (path, root, past, mut problems) = (place.at, place.root, place.past, place.problems);
    // **대기 자리는 있을 때만 든다** — 있는가가 곧 "아직 안 합쳤다" 다([`Place::pending`]).
    let pending = place.pending.filter(|p| p.exists());
    let dir = dir_of(&path);
    let err = |e: std::io::Error| Fail::new(format!("{}: {e}", path.display()));
    let called = |sheet: &mut Sheet| {
        f(sheet).map_err(|e| match e.code {
            crate::fail::code::BROKEN => Fail::coded(format!("{}: {}", path.display(), e.message), e.code),
            _ => e,
        })
    };
    // **짓기 전에 재 본다**(moai-dyb7). 없는 파일은 빈 것으로 들고, 옛 자리도 그대로 겹쳐 본다 — 락 안의
    // 차례와 **같은 것을 재야** 한다. 빼먹으면 옛 자리에만 있는 읽음이 합쳐질 판을 "쓸 것이 없다" 로 읽어,
    // 합치기가 다음 쓰기까지 미뤄진다.
    //
    // 그 뒤에 옆에서 파일이 서도 잃는 것은 없다 — 여기서 "쓸 것이 없다" 가 나오려면 빈 표에 대고도 적을
    // 것이 없었다는 뜻이고, 그것은 어느 표에 대고도 적을 것이 없다.
    if !path.exists() {
        let mut trial =
            Sheet::parse("").map_err(|e| refuse(format!("{}: {e} — 고치기 전까지 쓰지 않는다", path.display())))?;
        // 재 보기의 까닭은 **돌아설 때만** 싣는다 — 안 돌아서면 락 안의 합치기가 같은 줄을 다시 내므로,
        // 둘 다 실으면 한 판의 한 탈이 두 줄로 선다.
        let mut why = Vec::new();
        // 락 안의 차례와 **같은 것을 잰다** — 파일이 아직 없으니 옛 자리도 다 든다.
        let mut trying: Vec<&Path> = pending.as_deref().into_iter().collect();
        trying.extend(past.iter().map(PathBuf::as_path));
        merge_past(&mut trial, &trying, &root, &mut why)?;
        let out = called(&mut trial)?;
        if !trial.changed() {
            problems.append(&mut why);
            return Ok(Wrote { value: out, problems });
        }
    }
    // **남이 못 들여다보는 자리에 짓는다**(리뷰). 무엇을 읽었는지는 설정과 같은 갈래의 사적인 값인데,
    // `write_atomic` 이 지키는 것은 **있던 파일**의 권한이라 처음 쓰기는 umask 를 따른다 — `chmod 600
    // config.toml` 해 둔 사람의 읽음이 자리를 옮기는 것만으로 0644 로 풀린다. 파일마다 권한을 입히는
    // 길은 안 낸다(`write_atomic_as` 를 되살리지 않는다는 moai-c1s3 결정) — 디렉터리 하나를 닫는다.
    //
    // **처음 지을 때만** 닫는다. 사람이 나중에 연 권한을 도구가 쓰기마다 되돌리면 그건 설정이 아니다.
    let fresh_dir = !dir.exists();
    std::fs::create_dir_all(dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
    #[cfg(unix)]
    if fresh_dir {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let _ = fresh_dir;
    // 락 자리는 [`crate::store::lock_beside`] 가 센다 — 같은 디렉터리의 두 파일이 저마다 세면 자리
    // 규칙이 바뀌는 날 한쪽만 따라가 둘이 서로를 안 막는다(리뷰).
    let _lock = Lock::acquire(&lock_beside(&path))?;

    // 락을 잡은 **뒤에** 읽는다. 밖에서 읽으면 두 프로세스가 같은 옛 표를 고친다.
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(err(e)),
    };
    let fresh_sheet = src.is_empty();
    let mut sheet =
        Sheet::parse(&src).map_err(|e| refuse(format!("{}: {e} — 고치기 전까지 쓰지 않는다", path.display())))?;
    // **남의 읽음 위에 쓰지 않는다** — 해시가 부딪혔다. 재어 본 일이 없는 만큼 드문 자리지만, 조용히
    // 섞는 것이 이 에픽이 고치는 바로 그 해라 멈춘다.
    if !sheet.owns(&root) {
        return Err(refuse(format!(
            "{}: {} 의 읽음이 아니라 쓰지 않는다 — 손으로 지운다",
            path.display(),
            root.display()
        )));
    }
    // **손으로 고칠 거절에는 어느 파일인지 붙인다** — [`crate::user_config::update`] 와 같은 자리이고 같은
    // 까닭이다(리뷰). 이 파일의 이름은 뿌리의 해시라 사람이 짐작할 수 없어, 붙이지 않으면 "손으로
    // 고친다" 가 갈 곳 없는 말이 된다. 붙이는 곳을 부르는 쪽마다 두면 붙인 곳과 잊은 곳이 갈린다.

    // **이 파일에서 건너뛴 줄도 댄다**(moai-upna). [`Sheet::marks`] 의 둘째 값은 읽는 길만 물고
    // 갔고([`read_one`]) 쓰는 길은 버렸다 — 손으로 적은 `"argos-0002" = 3` 한 줄이 있는 사람에게
    // `moai read --all` 은 그것을 대는데 `moai read <id>` 는 조용했고, 그 줄은 견줌에서도 빠져
    // 쓰는 길이 "이미 읽었는가" 를 그 줄 없이 쟀다. 옛 자리와 대기 자리의 같은 줄은 [`merge_past`]
    // 가 이미 대므로, 여기서 대는 것은 **지금 자리의 것**이다.
    //
    // **락 안에서 한 번만 훑는다.** 닫은 글은 두 번 돌 수 있어(재 보기·락 안) 그 안에서 세면 같은
    // 줄이 두 번 서고, 부르는 쪽마다 세면 [`crate::cmd::read`] 와 탐색기가 갈린다. 재 보기 쪽은 빈
    // 표라 셀 것이 없다. 겹치기 전에 센다 — [`merge_past`] 가 얹은 줄은 도구가 지은 값이라 건너뛸
    // 것이 없고, 그 뒤에 세면 같은 줄을 옛 자리의 것과 섞어 두 번 댈 길이 열린다.
    problems.extend(sheet.marks().1.into_iter().map(|why| format!("{}: {why}", path.display())));

    // **대기 자리는 그 락 안에서 들고, 쓴 뒤에 지운다**(moai-bdej, 리뷰 4). 락 없이 읽고 지우던 길은
    // 읽기와 지우기 사이에 떨어진 판이 적은 도장을 함께 지웠다 — 조용한 손실이라 못 견딘다. 락의
    // 차례는 늘 `at` → 대기 자리다: 떨어진 판은 대기 자리가 곧 `at` 이라 하나만 잡으므로 고리가 없다.
    let held = match &pending {
        Some(spool) => Some((Lock::acquire(&lock_beside(spool))?, spool)),
        None => None,
    };
    if let Some((_, spool)) = &held {
        merge_past(&mut sheet, &[spool.as_path()], &root, &mut problems)?;
    }
    // **옛 자리는 처음 짓는 파일일 때만 합친다**(리뷰 13). 읽기도 그때만 보므로([`overlay_place`])
    // 합치는 자리는 여기 하나다 — 옛 파일은 그대로 두니 지우는 것도 옮기는 것도 아니다. 겹치는
    // 차례는 그대로다: 여기 이미 있는 id 는 안 건드린다.
    if fresh_sheet {
        merge_past(&mut sheet, &past.iter().map(PathBuf::as_path).collect::<Vec<_>>(), &root, &mut problems)?;
    }
    let out = called(&mut sheet)?;
    // **바뀐 것이 없으면 파일을 안 짓는다.** 어느 프로젝트의 것인지 적는 줄([`Sheet::claim`])도 그때
    // 함께 적는다 — 먼저 적던 판은 그 한 줄이 쓰기를 세워, 적을 것이 없는 `moai read` 하나가 아직
    // 아무것도 안 한 사람의 집에 빈 읽음 파일을 지었다.
    if sheet.changed() {
        sheet.claim(&root);
        write_atomic(&path, sheet.render().as_bytes())?;
    }
    // **지우는 것은 쓴 뒤다.** 먼저 지우면 아래 쓰기가 넘어지는 판에 그 도장이 어디에도 없다. 바뀐 것이
    // 없어도 지운다 — 그때는 그 도장이 이미 이 표에 서 있다는 뜻이다(합쳐도 새로 적을 것이 없었다).
    //
    // **못 지워도 넘어지지 않는다.** 쓰기는 이미 끝났고 도장은 여기 있다 — 남은 대기 자리는 다음 판이
    // 한 번 더 합치는 값(같은 것을 다시 얹으니 잃는 것이 없다)뿐이라, 까닭만 싣고 지나간다.
    if let Some((_lock, spool)) = &held
        && let Err(e) = std::fs::remove_file(spool)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        problems.push(format!("{}: {e} — 합쳤지만 대기 자리를 못 치웠다", spool.display()));
    }
    Ok(Wrote { value: out, problems })
}

/// 다른 자리의 표를 이 [`Sheet`] 에 겹친다 — 대기 자리([`Place::pending`])와, 처음 짓는 파일이면
/// 옛 철자로 선 파일들([`Place::past`]).
///
/// 재 보기와 락 안의 쓰기가 **한 함수로 겹친다**(moai-dyb7) — 둘로 두면 한쪽만 고치는 날 재 보기가
/// 쓰기와 다른 답을 내고, 그 어긋남은 "쓸 것이 없다" 로 조용히 돌아선다.
///
/// **못 든 까닭은 물고 나온다**(리뷰) — 읽는 길의 [`older`] 와 같은 자다. 버리던 판은 그 한 번이 아무 말
/// 없이 지나갔다. **못 든 것은 그대로 남는다**: 대기 자리는 합쳤을 때만 지워지므로(같은 쓰기가 그것을
/// 읽고 지운다) 못 읽은 판에서는 파일이 그대로 서 있고, 사람이 권한을 고치면 다음 판이 다시 든다.
/// 옛 철자 파일은 처음 짓는 판에서만 들어 그 한 번을 놓치면 다음 판이 안 여는데, 그 자리는 이 결정이
/// 안 하기로 한 일(옛 파일을 안 건드린다)의 대가다.
///
/// **여기 이미 선 id 는 안 건드린다** — 읽는 길의 [`overlay`] 와 같은 차례고(이 파일이 이긴다), 그것을
/// [`update`] 가 약속한 글이다. 걸러야 하는 까닭이 둘이다(리뷰).
///
/// - [`Sheet::mark`] 는 [`crate::user_config::write_value`] 로 **덮어쓴다.** 처음 짓는 파일에만 겹치던
///   동안은 빈 표에만 닿아 부딪칠 일이 없었는데, 대기 자리는 선 표에도 겹친다 — 안 거르면 그 자리의
///   낡은 도장이 지금 자리의 새 도장을 되돌려 이미 읽은 줄이 [NEW] 로 돌아오고, 읽기는 `overlay` 로
///   새것을 지키니 화면과 파일이 한 id 를 달리 든다
/// - `mark` 는 **하나라도 때가 아니면 모두 거절한다.** 사람이 적어 둔 점 키(`a-0002.rv = …`) 자리에
///   다른 자리의 같은 id 가 닿으면 그 한 줄이 **읽음을 적는 모든 명령**을 세운다 — "엄함은 지금 쓰는
///   줄에 대한 것이지 파일 전체에 대한 것이 아니다"(CLAUDE.md). 그래서 **때가 아닌 값이 앉은 자리와
///   `[read]` 가 표가 아닌 판도** 못 앉히는 것으로 센다([`Sheet::taken`]): 그 줄은 사람이 고칠
///   때까지 그대로 남고, 부른 쪽의 제 id 만 거절당한다
///
/// **거꾸로 놓친 자리는 아직 열려 있다**(moai-bdej 리뷰 8). 떨어진 판이 적는 것도 그 줄의 `updated_at`
/// 이라([`crate::query::read_marks_of`]), 여기 이미 선 id 에 **더 늦은** 도장이 떨어져 있으면 그것이
/// 조용히 버려진다 — 그 줄은 사람이 다시 읽을 때까지 [NEW] 로 선다. 고치는 길은 `늦은 것이 이긴다`
/// 로, 그 규칙은 이 저장소에 이미 있다(`read_marks_of` 가 같은 id 의 줄에서 가장 늦은 도장을 고른다).
/// 여기서 안 바꾼 까닭은 [`overlay`] 의 차례가 **사람의 결정**(2026-09-19 결정 3, `이 파일이 이긴다`)
/// 이고 `the_three_places_overlay_in_one_order` 가 때가 아닌 글자로 그것을 못박아서다 — 늦은 것을
/// 고르려면 읽는 길까지 함께 옮겨야 하고, 그것은 이 자리의 고침이 아니라 그 결정을 다시 여는 일이다.
fn merge_past(sheet: &mut Sheet, past: &[&Path], root: &Path, problems: &mut Vec<String>) -> R<()> {
    let mut older_marks = BTreeMap::new();
    for old in past {
        let mut got = read_one(old, root);
        overlay(&mut older_marks, &got.seen);
        problems.append(&mut got.problems);
    }
    older_marks.retain(|id, _| !sheet.taken(id));
    if !older_marks.is_empty() {
        sheet.mark(&older_marks)?;
    }
    Ok(())
}

/// 읽음을 고치고 나온 것 — 부른 쪽이 시킨 값과 **그 자리를 고르다 만난 까닭**.
///
/// [`Marks`] 와 같은 모양이다(moai-ajh2). 읽는 쪽과 쓰는 쪽이 한 자를 쓰지 않던 판은 쓰는 쪽만 조용했다.
#[derive(Debug)]
pub struct Wrote<T> {
    /// [`update`] 에 준 함수가 돌려준 것.
    pub value: T,
    /// 자리를 고르다([`Place::problems`]) 또 옛 자리를 합치다([`merge_past`]) 만나고, **이 파일에서
    /// 건너뛴 줄**([`Sheet::marks`] 의 둘째 값, moai-upna) 때문에 생긴 까닭. 빈 것이 정상이다.
    pub problems: Vec<String>,
}

/// 읽음 파일 하나. 모르는 키와 주석은 그대로 들고 간다 — 도구가 짓는 파일이어도 사람이 곁에 한 줄
/// 적어 둘 수 있고, 새 바이너리가 적은 키를 옛 바이너리가 한 번 만져 지우면 안 된다.
///
/// **[`Doc`] 을 그대로 두른다**(리뷰). 그쪽이 이미 바이트를 지킨다 — BOM, 줄바꿈 없이 끝난 파일,
/// CRLF(moai-r9qa·moai-lb0u), 값 뒤의 주석([`write_value`]), 주석만 있던 파일의 머리 주석
/// ([`Doc::new_table`]). 그 넷은 저마다 값을 치르고 고친 자리라, 문서를 새로 두르면 넷이 한꺼번에
/// 돌아온다. 갈라 두던 판이 실제로 그랬다.
pub struct Sheet {
    doc: Doc,
}

impl Sheet {
    fn parse(src: &str) -> Result<Sheet, String> {
        Ok(Sheet { doc: Doc::parse(src)? })
    }

    fn changed(&self) -> bool {
        self.doc.changed()
    }

    /// 이 파일이 그 뿌리의 것인가. `path` 가 없으면(처음 짓는 파일) 참이다 — 그때 [`Sheet::claim`] 이 적는다.
    ///
    /// **낱말이 아닌 `path` 는 남의 것으로 읽는다**(리뷰). 참으로 읽던 판은 [`Sheet::claim`] 이 그것을
    /// 안 덮는 것과 어긋나, `path = 3` 하나가 붙은 파일을 해시가 닿는 **모든** 뿌리가 제 것으로 여겨
    /// 함께 적었다 — 막으려던 섞임이 도리어 조용해졌다.
    fn owns(&self, root: &Path) -> bool {
        match self.doc.root().get(PATH) {
            None => true,
            // **푼 경로로 견준다**(moai-f5e3) — 파일 이름은 그것으로 고르는데 이 문지기만 철자를 보면,
            // 옛 철자로 적힌 `path` 를 남의 것으로 읽어 제 읽음을 안 읽는다. 짝이 어긋난 문지기는
            // 지키는 시늉만 한다. 받는 `root` 는 그 이름을 고른 [`Place::root`] 다.
            //
            // **견주는 자는 [`crate::user_config::same_dir`] 하나다**(리뷰). 손으로 풀어 견주던 판은
            // 이 모듈을 그 견줌의 다섯째 사본으로 만들면서, 철자가 같을 때 거저 끝나는 길까지 버려
            // 이 바이너리가 적은 파일에도 `canonicalize` 를 두 번씩 불렀다 — 락 안에서도 그랬다.
            Some(item) => item.as_str().is_some_and(|at| crate::user_config::same_dir(Path::new(at), root)),
        }
    }

    /// 어느 프로젝트의 것인지 적는다 — 없을 때만. 이미 적혀 있으면 [`Sheet::owns`] 가 같은 것을 보았다.
    ///
    /// **적는 것은 이름을 고른 푼 뿌리다**([`Place::root`], moai-f5e3 리뷰). 받은 철자를 적던 판은 이름과
    /// 짝이 어긋나, 그 철자가 링크였다가 사라지는 날 [`Sheet::owns`] 가 제가 지은 파일을 남의 것으로
    /// 읽었다 — 읽기는 빈 표와 `Broken` 을 내고 쓰기는 "손으로 지운다" 로 거절해, 시키는 대로 하면
    /// 그 프로젝트의 읽음이 통째로 사라졌다. 옛 바이너리에도 이쪽이 안전하다: 그것이 푼 철자로 불리면
    /// 같은 파일을 열어 글자로 견주는데, 그때 맞는 값이 이것이다.
    ///
    /// **글자로 못 적는 뿌리는 안 적는다**(리뷰). `display()` 는 UTF-8 이 아닌 바이트를 U+FFFD 로 바꿔
    /// 적는데, [`Sheet::owns`] 는 그것을 바이트째 견줘 남의 것으로 읽는다 — 제가 방금 지은 파일을 다음
    /// 명령이 거절하고, 거절문은 그 파일을 손으로 지우라 하고, 지우면 같은 줄이 다시 적히는 고리였다.
    /// 이름의 해시는 바이트를 그대로 세므로 안 적어도 파일은 제 것이다.
    fn claim(&mut self, root: &Path) {
        if self.doc.root().get(PATH).is_some() || root.to_str().is_none() {
            return;
        }
        if write_value(self.doc.root_mut(), PATH, root.display().to_string().into()) {
            self.doc.touched();
        }
    }

    /// 옛 자리의 그 id 를 여기에 **못 앉히는가** — 자리가 이미 섰거나, `[read]` 가 표가 아니어서
    /// 어느 id 도 앉힐 데가 없다.
    ///
    /// [`Sheet::marks`] 로 묻지 않는 까닭이 그것이다(리뷰). 그쪽은 관대하게 읽어 때가 아닌 값을
    /// 건너뛰는데, 여기서 건너뛰면 [`merge_past`] 가 사람이 적어 둔 점 키(`a-0002.rv = …`) 자리를
    /// 건드리려 하고 [`Sheet::mark`] 가 쓰기 전체를 거절한다. 모르는 값은 **선 것**으로 센다.
    ///
    /// **`[read]` 가 표가 아닌 판도 같은 자다**(리뷰). 그때 `mark` 의 첫 문지기는 쓰기 전체를
    /// 거절하는데, 그 거절은 *지금 쓰는 줄*이 아니라 사람이 손으로 적은 `read = 3` 한 줄에서 온다
    /// (CLAUDE.md "남의 낡은 줄 하나가 모든 쓰기를 막으면"). 합칠 것을 다 걷어 [`merge_past`] 를
    /// 조용히 지나가게 한다 — 그 파일이 깨졌다는 말은 읽는 길([`read_table`])이 이미 댄다. 부른
    /// 쪽의 제 id 는 그대로 거절당한다: 그것이 지금 쓰는 줄이다.
    fn taken(&self, id: &str) -> bool {
        match self.doc.root().get(READ) {
            None => false,
            Some(item) => item.as_table_like().is_none_or(|t| t.contains_key(id)),
        }
    }

    /// 적어 둔 읽음. **관대하게 읽는다** — 낱말이 아닌 값은 까닭 한 줄로 대고 건너뛴다
    /// ([`read_table`], 옛 `[read]` 와 같은 자).
    pub fn marks(&self) -> (BTreeMap<String, String>, Vec<String>) {
        read_table(self.doc.root())
    }

    /// 읽음을 적는다 — **준 id 만 손댄다**. 같은 때가 이미 적혀 있으면 아무것도 안 한다. 돌려주는 것은
    /// 실제로 바뀐 id 다(부르는 쪽이 "무엇을 적었나" 를 락 안에서 잰 그대로 댄다, moai-j038.vna).
    ///
    /// 준 id 의 자리에 때가 아닌 것이 있으면 **하나도 안 적는다** — 손으로 적은 맨 점 키(`a-0002.rv = …`)는
    /// `a-0002` 표 밑의 `rv` 로 읽히는데, 그 위에 때를 덮으면 자식의 읽음과 그 위 주석이 말없이 사라진다.
    ///
    /// **값만 바꾼다**([`write_value`], 리뷰) — 키 위의 주석·값 뒤의 주석·키 모양은 그대로다.
    /// `Table::insert` 로 갈아 끼우면 키를 새로 지어 셋이 다 사라진다(`Doc::set_hue` 가 적어 둔 그대로다).
    pub fn mark(&mut self, marks: &BTreeMap<String, String>) -> R<Vec<String>> {
        if marks.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(item) = self.doc.root().get(READ) {
            let Some(t) = item.as_table_like() else {
                return Err(refuse(format!(
                    "`{READ}` 가 `[{READ}]` 표가 아니라({}) 읽음을 적지 않는다 — 손으로 고친다",
                    item.type_name()
                )));
            };
            if let Some((id, odd)) =
                marks.keys().find_map(|id| t.get(id).filter(|v| v.as_str().is_none()).map(|v| (id, v)))
            {
                return Err(refuse(format!(
                    "`{READ}` 의 `{id}` 가 때가 아니라({}) 읽음을 적지 않는다 — 손으로 고친다",
                    odd.type_name()
                )));
            }
        } else {
            // 주석만 있던 파일이면 머리 주석을 머리에 둔다 — 안 두면 사람이 적어 둔 줄이 `[read]` 밑으로
            // 밀려 마지막 읽음에 붙은 말로 읽힌다(moai-gmdu 에픽 리뷰가 설정에서 고친 그 자리다, 리뷰).
            let t = self.doc.new_table();
            self.doc.root_mut().insert(READ, Item::Table(t));
        }
        let t = self.doc.root_mut().get_mut(READ).and_then(Item::as_table_like_mut).expect("방금 표로 섰다");
        let mut written = Vec::new();
        for (id, when) in marks {
            if write_value(t, id, when.as_str().into()) {
                written.push(id.clone());
            }
        }
        if !written.is_empty() {
            self.doc.touched();
        }
        Ok(written)
    }

    /// **트래커에 없는 id 의 줄을 걷는다**(moai-dt5q, 사용자 결정 2 — 2026-09-19). 지운 이슈의 읽음은
    /// 아무도 다시 안 본다.
    ///
    /// **닫힌 줄은 안 걷는다.** 걷으면 그 줄이 다시 목록에 설 때 [NEW] 가 되살아나, 읽음의 뜻이
    /// "본 적 있다" 에서 "최근에 본 적 있다" 로 바뀐다.
    ///
    /// **때가 적힌 줄만 걷는다**(리뷰) — 손으로 적은 맨 점 키(`a-0002.rv = …`)는 `a-0002` 표로 읽히는데,
    /// 그 표를 지우면 자식의 읽음과 그 위 주석이 말없이 사라진다. [`Sheet::mark`] 가 그 자리를 안 덮는
    /// 것과 같은 자다 — 무엇인지 모르는 값은 읽기가 까닭을 대고 사람이 푼다.
    ///
    /// **주석은 키와 함께 안 지운다**(리뷰) — 빼는 자는 [`crate::user_config::Doc::drop_keys`] 하나다.
    /// 맨 `remove` 로 빼던 판은 빈 줄 너머의 주석까지 가져가, 이 모듈이 약속한 "모르는 키와 주석은
    /// 그대로 들고 간다" 가 걷기 한 번에 깨졌다.
    ///
    /// 읽음을 적는 그 자리에서만 부른다 — 거기는 트래커를 이미 들고 있다. 설정 쓰기가 트래커를 읽어야
    /// 하는 일을 안 만들자는 것이 이 자리의 까닭이다.
    pub fn prune(&mut self, known: &BTreeSet<&str>) -> usize {
        let Some(t) = self.doc.root().get(READ).and_then(Item::as_table_like) else {
            return 0;
        };
        // 걷을 것만 짓는다 — 먼저 모두 베끼던 판은 걷을 것이 없는 흔한 판에서도 키 수만큼 문자열을 지었다.
        let gone: Vec<String> = t
            .iter()
            .filter(|(id, at)| at.as_str().is_some() && !known.contains(id))
            .map(|(id, _)| id.to_string())
            .collect();
        self.doc.drop_keys(READ, &gone)
    }

    fn render(&self) -> String {
        self.doc.render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn marks(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(a, b)| ((*a).to_string(), (*b).to_string())).collect()
    }

    /// **프로젝트마다 제 파일이다**(moai-omx7) — 이름이 같은 디렉터리 둘이 같은 id 를 써도 서로의
    /// 읽음을 안 민다. 그 섞임이 이 에픽을 연 까닭이다.
    #[test]
    fn two_projects_with_the_same_id_do_not_push_each_other() {
        let s = Scratch::new("read-marks-split");
        let cfg = s.join("config.toml");
        let (one, two) = (s.join("a/api"), s.join("b/api"));

        update(&cfg, &one, |sheet| sheet.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        update(&cfg, &two, |sheet| sheet.mark(&marks(&[("argos-0001", "B")]))).unwrap();

        assert_eq!(read(&cfg, &one, &BTreeMap::new()).seen, marks(&[("argos-0001", "A")]));
        assert_eq!(read(&cfg, &two, &BTreeMap::new()).seen, marks(&[("argos-0001", "B")]));
        assert_ne!(path_for(&cfg, &one), path_for(&cfg, &two));
    }

    /// **한 저장소를 가리키는 철자가 여럿이어도 한 파일에 쓴다**(moai-f5e3). `Repo::open` 은 받은 철자를
    /// 그대로 뿌리로 들어, 끝 `/`·`..`·링크가 저마다 딴 파일을 짓던 자리다 — 둘은 서로를 못 본 채
    /// 저마다 걷었다.
    #[test]
    fn every_spelling_of_one_root_writes_to_one_file() {
        let s = Scratch::new("read-marks-spelling");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let real = std::fs::canonicalize(&root).unwrap();

        let spellings = [root.clone(), s.join("proj/"), s.join("proj/../proj"), real.clone()];
        for (n, spelling) in spellings.iter().enumerate() {
            let id = format!("argos-{n:04}");
            update(&cfg, spelling, |sh| sh.mark(&marks(&[(&id, "A")]))).unwrap();
            assert_eq!(path_for(&cfg, spelling), path_for(&cfg, &real), "{} 가 딴 파일로 갔다", spelling.display());
        }
        // 넷이 한 파일에 쌓였고, 어느 철자로 읽어도 넷이 다 보인다.
        let sheets = std::fs::read_dir(dir_of(&cfg).join("read")).unwrap().count();
        assert_eq!(sheets, 2, "읽음 파일이 하나(와 그 락)가 아니다");
        for spelling in &spellings {
            assert_eq!(
                read(&cfg, spelling, &BTreeMap::new()).seen.len(),
                4,
                "{} 로 읽으니 덜 보인다",
                spelling.display()
            );
        }
    }

    /// **옛 철자로 선 파일은 읽기만 한다**(moai-f5e3, 사용자 결정 2026-09-20). 지우지도 옮기지도 않으니
    /// 마이그레이션이 아니고 읽음을 잃는 사람도 없다 — 옛 `[read]` 를 겹쳐 보는 것과 같은 꼴이다.
    ///
    /// **쓰는 자리는 하나다.** 둘에 쓰면 이 에픽이 고친 섞임이 그대로 돌아오므로 그것을 못박는다.
    #[test]
    fn the_file_an_old_spelling_left_is_read_but_never_written() {
        let s = Scratch::new("read-marks-past");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();

        // 옛 바이너리가 안 푼 철자로 적어 둔 파일 — 끝 `/` 하나가 딴 이름을 냈다.
        let slashed = s.join("proj/");
        let old = sheet_at(dir_of(&cfg), &slashed);
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        let before = format!("path = {:?}\n\n[read]\n\"argos-0001\" = \"옛것\"\n", slashed.display().to_string());
        std::fs::write(&old, &before).unwrap();
        assert_ne!(old, path_for(&cfg, &slashed), "시험의 전제 — 옛 이름과 새 이름이 다르다");

        // 그 철자로 부르면 읽을 때는 보인다 — 찾는 옛 자리는 **이 부름이 받은 철자**의 것이다.
        assert_eq!(read(&cfg, &slashed, &BTreeMap::new()).seen.get("argos-0001").map(String::as_str), Some("옛것"));

        // 적을 때는 새 자리에만 간다 — 옛 파일은 한 바이트도 안 바뀐다. **첫 쓰기가 옛 표를 여기로
        // 합친다**(리뷰 13) — 그래야 걷기가 지금 자리만 줄여도 걷은 id 가 다음 읽기에 안 되살아난다.
        update(&cfg, &slashed, |sh| sh.mark(&marks(&[("argos-0002", "새것")]))).unwrap();
        assert_eq!(std::fs::read_to_string(&old).unwrap(), before, "옛 철자 파일에 썼다");
        let now = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        assert!(now.contains("argos-0002") && now.contains("argos-0001"), "옛 표를 안 합쳤다 — {now}");

        // 합쳤으니 옛 파일이 사라져도 그 읽음은 남는다 — 이제 읽기는 옛 자리를 안 연다.
        std::fs::remove_file(&old).unwrap();
        let seen = read(&cfg, &slashed, &BTreeMap::new()).seen;
        assert_eq!(seen.get("argos-0001").map(String::as_str), Some("옛것"));
        assert_eq!(seen.len(), 2);

        // 같은 id 를 다시 적으면 **지금 자리가 이긴다**.
        update(&cfg, &slashed, |sh| sh.mark(&marks(&[("argos-0001", "새것이 이긴다")]))).unwrap();
        assert_eq!(
            read(&cfg, &slashed, &BTreeMap::new()).seen.get("argos-0001").map(String::as_str),
            Some("새것이 이긴다")
        );
    }

    /// **겹치는 차례는 셋이다** — 지금 자리 > 옛 철자 > 설정의 옛 `[read]`. 한 자리에서 갈라지게 두면
    /// 넷째가 붙는 날 한쪽만 따라간다.
    #[test]
    fn the_three_places_overlay_in_one_order() {
        let s = Scratch::new("read-marks-three");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();

        let slashed = s.join("proj/");
        let old = sheet_at(dir_of(&cfg), &slashed);
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(
            &old,
            format!("path = {:?}\n\n[read]\n\"a\" = \"옛 철자\"\n\"b\" = \"옛 철자\"\n", slashed.display().to_string()),
        )
        .unwrap();
        update(&cfg, &slashed, |sh| sh.mark(&marks(&[("a", "지금 자리")]))).unwrap();
        let legacy = marks(&[("a", "옛 표"), ("b", "옛 표"), ("c", "옛 표")]);

        let seen = read(&cfg, &slashed, &legacy).seen;
        assert_eq!(seen.get("a").map(String::as_str), Some("지금 자리"));
        assert_eq!(seen.get("b").map(String::as_str), Some("옛 철자"));
        assert_eq!(seen.get("c").map(String::as_str), Some("옛 표"));
    }

    /// **못 푸는 자리는 받은 철자로 떨어지고 까닭을 댄다**(사용자 결정 2026-09-20). 지워진 뿌리를
    /// 가리키는 등록 줄 하나가 `moai read` 를 통째로 멈추면 안 된다.
    ///
    /// **없는 것은 탈이 아니다** — 설정도 읽음 파일도 처음에는 없다.
    #[test]
    fn a_root_i_cannot_resolve_falls_back_and_says_so() {
        let s = Scratch::new("read-marks-unresolved");
        let cfg = s.join("config.toml");
        let gone = s.join("사라진 것");
        let place = place_of(&cfg, &gone);
        assert_eq!(place.at, sheet_at(dir_of(&cfg), &gone));
        assert_eq!(place.root, gone, "못 푼 자리를 뿌리로 안 들었다");
        assert!(place.problems.is_empty(), "없는 자리를 탈로 댔다 — {:?}", place.problems);

        // 경로 가운데가 파일이면 `NotADirectory` 다 — 그것은 댄다. 윈도에서는 `NotFound` 라 안 잰다.
        #[cfg(unix)]
        {
            std::fs::write(s.join("파일"), "x").unwrap();
            let through = s.join("파일/밑");
            let place = place_of(&cfg, &through);
            assert_eq!(place.problems.len(), 1, "{:?}", place.problems);
            assert!(place.problems[0].contains("자리를 못 풀어"), "{}", place.problems[0]);
            assert_eq!(read(&cfg, &through, &BTreeMap::new()).problems.len(), 1, "읽기가 그 까닭을 안 물고 왔다");
        }
    }

    /// **쓰는 길도 그 까닭을 댄다**(moai-ajh2). 읽는 길만 대던 판은 정작 값을 잃는 쪽이 조용했다 —
    /// 뿌리 윗자리가 잠깐 막히면 옛 철자 자리에 적고(락도 딴 자리다) 아무 말 없이 0 으로 끝났다.
    ///
    /// 막지는 않는다: 적기는 그대로 서고, 까닭은 값에 실려 나간다([`Wrote::problems`]).
    #[test]
    #[cfg(unix)]
    fn writing_says_why_it_could_not_settle_the_root() {
        let s = Scratch::new("read-marks-write-why");
        let cfg = s.join("config.toml");
        std::fs::write(s.join("파일"), "x").unwrap();
        let through = s.join("파일/밑");

        let wrote = update(&cfg, &through, |sh| sh.mark(&marks(&[("a", "A")]))).unwrap();
        assert_eq!(wrote.problems.len(), 1, "쓰는 길이 까닭을 버렸다 — {:?}", wrote.problems);
        assert!(wrote.problems[0].contains("자리를 못 풀어"), "{}", wrote.problems[0]);
        assert_eq!(wrote.value, vec!["a".to_string()], "말만 하고 안 적었다 — {:?}", wrote.value);

        // 성한 자리는 조용하다 — 빈 `problems` 가 정상이다.
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let wrote = update(&cfg, &root, |sh| sh.mark(&marks(&[("b", "B")]))).unwrap();
        assert!(wrote.problems.is_empty(), "성한 자리를 탈로 댔다 — {:?}", wrote.problems);
    }

    /// **쓰는 길도 건너뛴 줄을 댄다**(moai-upna). 읽는 길만 대던 판은 같은 파일을 두 길이 달리 읽었다 —
    /// 손으로 적은 `"a-0002" = 3` 한 줄이 있으면 `moai read --all` 은 그것을 대는데 `moai read <id>`
    /// 는 조용했고, 그 줄은 견줌에서도 빠져 쓰는 길이 "이미 읽었는가" 를 그 줄 없이 쟀다.
    ///
    /// **재는 자는 한 자리다** — 두 길의 글을 글자째 견준다. 한쪽만 고치면 여기가 붉어진다.
    #[test]
    fn writing_says_which_line_it_skipped() {
        let s = Scratch::new("read-marks-skipped");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(dir_of(&at)).unwrap();
        std::fs::write(&at, "[read]\n\"a-0002\" = 3\n\"a-0003\" = \"본 때\"\n").unwrap();

        let wrote = update(&cfg, &root, |sh| sh.mark(&marks(&[("a-0001", "A")]))).unwrap();
        assert_eq!(wrote.value, vec!["a-0001".to_string()], "말만 하고 안 적었다 — {:?}", wrote.value);
        assert_eq!(wrote.problems.len(), 1, "쓰는 길이 건너뛴 줄을 버렸다 — {:?}", wrote.problems);
        assert!(wrote.problems[0].contains("a-0002"), "{}", wrote.problems[0]);
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).problems, wrote.problems, "두 길이 같은 줄을 달리 부른다");

        // 건너뛴 줄은 **그대로 남는다** — 대는 것이지 걷는 것이 아니다.
        let (kept, skipped) = Sheet::parse(&std::fs::read_to_string(&at).unwrap()).unwrap().marks();
        assert_eq!(skipped.len(), 1, "건너뛴 줄을 지웠다 — {skipped:?}");
        assert_eq!(kept.get("a-0001").map(String::as_str), Some("A"));

        // 성한 표는 조용하다 — 빈 `problems` 가 정상이다.
        let clean = s.join("proj2");
        std::fs::create_dir_all(&clean).unwrap();
        let wrote = update(&cfg, &clean, |sh| sh.mark(&marks(&[("b", "B")]))).unwrap();
        assert!(wrote.problems.is_empty(), "성한 표를 탈로 댔다 — {:?}", wrote.problems);
    }

    /// **못 푼 판의 도장은 대기 자리로 가고, 다음 성한 쓰기가 합치고 지운다**(moai-bdej,
    /// 사용자 결정 2026-09-20 — 리뷰 뒤 고친 판).
    ///
    /// 첫 판은 받은 철자의 **읽음 파일**에 적고 합칠 것이 남았는지를 파일의 때로 물었는데, 그 물음은
    /// 닫히지 않았다 — 지금 자리에 아무 쓰기나 들면 때가 올라가 그 도장이 영영 남고(리뷰 3), 걷은 id
    /// 가 되살아나는 자리도 함께 열렸다(리뷰 6). 이제 가르는 자는 **대기 자리가 있는가** 하나고,
    /// 닫는 자는 합치기 자신이다.
    ///
    /// **되돌림을 재는 자는 한 자리다.** 앞 절은 떨어진 도장이 돌아오는 것을, 뒤 절은 합친 뒤에 다시
    /// 안 열리는 것을 잰다(리뷰 7·13 이 고친 자리다) — 한쪽만 고치면 다른 쪽이 무른다.
    #[test]
    #[cfg(unix)]
    fn a_stamp_that_fell_to_an_unresolved_spelling_joins_the_next_healthy_write() {
        let s = Scratch::new("read-marks-fallen");
        let cfg = s.join("config.toml");
        std::fs::create_dir_all(s.join("real/proj")).unwrap();
        std::fs::write(s.join("막힌 것"), "x").unwrap();
        let gate = s.join("문");
        std::os::unix::fs::symlink("real", &gate).unwrap();
        // **받은 철자가 푼 경로와 다르다** — 그래야 떨어진 자리가 다음 판에 옛 자리로 선다.
        let spelling = s.join("문/proj/../proj");
        let at = path_for(&cfg, &spelling);
        let fell = spool_at(dir_of(&cfg), &spelling);
        assert_ne!(at, fell, "시험의 전제 — 푼 자리와 대기 자리가 딴 파일이다");
        // **대기 자리는 옛 철자 파일과 이름이 다르다** — 같으면 옛 바이너리가 쓴 파일을 지운다.
        assert_ne!(fell, sheet_at(dir_of(&cfg), &spelling), "대기 자리가 옛 철자 파일과 한 이름이다");

        // 여러 달치 도장이 이미 선 사람.
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("a", "먼저")]))).unwrap();
        assert!(at.exists(), "성한 판이 제 자리에 안 적었다");

        // 뿌리 윗자리가 잠깐 막힌 한 판 — 자리를 못 풀어 도장이 받은 철자 파일로 간다.
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("막힌 것", &gate).unwrap();
        let wrote = update(&cfg, &spelling, |sh| sh.mark(&marks(&[("b", "떨어진 판")]))).unwrap();
        assert_eq!(wrote.problems.len(), 1, "떨어진 것을 안 댔다 — {:?}", wrote.problems);
        assert!(fell.exists(), "떨어진 판이 대기 자리에 안 적었다");
        // **읽기는 그 사이에도 그 도장을 든다** — 닫는 것은 쓰기라, 화면이 [NEW] 로 서 있지 않는다.
        assert_eq!(read(&cfg, &spelling, &BTreeMap::new()).seen.get("b").map(String::as_str), Some("떨어진 판"));

        // 자리가 다시 풀린 판이 그것을 합치고 **지운다** — 지금 자리 파일이 이미 있어도.
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("real", &gate).unwrap();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("c", "나중")]))).unwrap();
        assert!(!fell.exists(), "합쳤는데 대기 자리가 남았다");
        let seen = read(&cfg, &spelling, &BTreeMap::new()).seen;
        assert_eq!(seen.get("b").map(String::as_str), Some("떨어진 판"), "떨어진 도장을 잃었다 — {seen:?}");
        assert_eq!(seen.len(), 3, "{seen:?}");

        // **합친 뒤에는 안 연다** — 걷은 id 가 되살아나던 자리(리뷰 7·13)는 그대로 닫혀 있다.
        let keep = BTreeSet::from(["c"]);
        assert_eq!(update(&cfg, &spelling, |sh| Ok(sh.prune(&keep))).unwrap().value, 2, "안 걷었다");
        assert_eq!(read(&cfg, &spelling, &BTreeMap::new()).seen.len(), 1, "걷은 도장이 대기 자리에서 되살아났다");

        // **푼 철자로 부른 쓰기는 대기 자리를 닫지 않는다**(리뷰 3). 한때 가르는 자가 파일의 때라,
        // 그 한 번이 창을 영영 닫아 떨어진 도장이 그대로 남았다.
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("막힌 것", &gate).unwrap();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("d", "또 떨어진 판")]))).unwrap();
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("real", &gate).unwrap();
        let real = std::fs::canonicalize(&spelling).unwrap();
        assert_ne!(real.as_os_str(), spelling.as_os_str(), "시험의 전제 — 푼 철자가 받은 철자와 다르다");
        update(&cfg, &real, |sh| sh.mark(&marks(&[("e", "푼 철자로")]))).unwrap();
        assert!(fell.exists(), "푼 철자로 부른 쓰기가 대기 자리를 닫았다");
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("f", "그 뒤")]))).unwrap();
        let seen = read(&cfg, &spelling, &BTreeMap::new()).seen;
        assert_eq!(seen.get("d").map(String::as_str), Some("또 떨어진 판"), "되풀이해 떨어진 도장을 잃었다 — {seen:?}");
        assert!(!fell.exists(), "합쳤는데 대기 자리가 남았다");
    }

    /// **대기 자리의 낡은 도장이 지금 자리의 새 도장을 되돌리지 않는다**(리뷰) — 겹치는 차례는 읽는
    /// 길과 같다(이 파일이 이긴다).
    ///
    /// 합치기가 처음 짓는 파일에만 겹치던 동안은 빈 표에만 닿아 부딪칠 일이 없었다. 대기 자리는 선
    /// 표에도 겹치는데 [`Sheet::mark`] 는 [`crate::user_config::write_value`] 로 **덮어쓰므로**, 안 거르면
    /// 이미 읽은 줄이 [NEW] 로 돌아온다. 읽기([`older`])는 [`overlay`] 로 새 도장을 지키니 거르지 않으면
    /// 두 길이 한 id 를 달리 세기도 한다 — 화면은 새것을, 파일은 낡은 것을 든다.
    #[test]
    #[cfg(unix)]
    fn a_pending_place_does_not_rewind_a_stamp_that_stands_here() {
        let s = Scratch::new("read-marks-rewind");
        let cfg = s.join("config.toml");
        std::fs::create_dir_all(s.join("real/proj")).unwrap();
        std::fs::write(s.join("막힌 것"), "x").unwrap();
        let gate = s.join("문");
        std::os::unix::fs::symlink("real", &gate).unwrap();
        let spelling = s.join("문/proj/../proj");
        let (at, fell) = (path_for(&cfg, &spelling), spool_at(dir_of(&cfg), &spelling));
        let block = || {
            std::fs::remove_file(&gate).unwrap();
            std::os::unix::fs::symlink("막힌 것", &gate).unwrap();
        };
        let clear = || {
            std::fs::remove_file(&gate).unwrap();
            std::os::unix::fs::symlink("real", &gate).unwrap();
        };

        // 지금 자리에 `a` 의 **새 도장**이 선다.
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("a", "새 때")]))).unwrap();
        assert_eq!(read(&cfg, &spelling, &BTreeMap::new()).seen.get("a").map(String::as_str), Some("새 때"));

        // 떨어진 판이 같은 `a` 를 **낡은 도장**으로, 그리고 새 `b` 를 대기 자리에 적는다.
        block();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("a", "낡은 때"), ("b", "떨어진 판")]))).unwrap();
        assert!(fell.exists(), "떨어진 판이 대기 자리에 안 적었다");
        clear();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("c", "또")]))).unwrap();

        let seen = read(&cfg, &spelling, &BTreeMap::new()).seen;
        assert_eq!(seen.get("a").map(String::as_str), Some("새 때"), "대기 자리가 새 도장을 되돌렸다 — {seen:?}");
        assert_eq!(seen.get("b").map(String::as_str), Some("떨어진 판"), "떨어진 도장을 잃었다 — {seen:?}");
        // **파일과 화면이 같은 것을 든다** — 읽기만 새 때를 지키면 다음 쓰기가 낡은 때를 굳힌다.
        let (kept, _) = Sheet::parse(&std::fs::read_to_string(&at).unwrap()).unwrap().marks();
        assert_eq!(kept.get("a").map(String::as_str), Some("새 때"), "파일에는 낡은 도장이 굳었다 — {kept:?}");
    }

    /// **남의 낡은 줄 하나가 모든 쓰기를 막지 않는다**(CLAUDE.md). 손으로 적은 점 키(`a-0002.rv = …`)는
    /// [`Sheet::mark`] 가 거절하는 자리인데, 옛 자리에 같은 id 가 있으면 [`merge_past`] 가 그것을
    /// 건드리려 해 **읽음을 적는 모든 명령**이 거절로 끝났다 — 처음 짓는 파일에만 겹치던 동안은 빈
    /// 표에만 닿아 못 보던 자리다. 이미 선 id 를 거르면 그 줄은 사람이 고칠 때까지 그대로 남는다.
    #[test]
    #[cfg(unix)]
    fn a_hand_written_dotted_key_here_does_not_block_the_merge() {
        let s = Scratch::new("read-marks-dotted");
        let cfg = s.join("config.toml");
        std::fs::create_dir_all(s.join("real/proj")).unwrap();
        std::fs::write(s.join("막힌 것"), "x").unwrap();
        let gate = s.join("문");
        std::os::unix::fs::symlink("real", &gate).unwrap();
        let spelling = s.join("문/proj/../proj");
        let at = path_for(&cfg, &spelling);

        // 지금 자리가 먼저 선다 — 안 그러면 첫 성한 쓰기가 대기 자리를 여기로 합쳐 점 키 자리에 때가 앉는다.
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("z", "내 것")]))).unwrap();
        // 떨어진 판이 `a-0002` 를 때로 적는다.
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("막힌 것", &gate).unwrap();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("a-0002", "떨어진 판")]))).unwrap();
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("real", &gate).unwrap();

        // 지금 자리에는 사람이 그 id 밑에 점 키를 적어 두었다.
        let src = std::fs::read_to_string(&at).unwrap().replace("\nz = ", "\na-0002.rv = \"손으로\"\nz = ");
        std::fs::write(&at, &src).unwrap();
        assert!(std::fs::read_to_string(&at).unwrap().contains("a-0002.rv"), "시험의 전제 — 점 키를 못 심었다");

        let wrote = update(&cfg, &spelling, |sh| sh.mark(&marks(&[("y", "다음")])));
        assert!(wrote.is_ok(), "점 키 하나가 쓰기를 막았다 — {:?}", wrote.err());
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("a-0002.rv = \"손으로\""), "사람이 적은 줄을 덮었다 — {now}");
        assert!(now.contains("y = \"다음\""), "시킨 것을 안 적었다 — {now}");
    }

    /// **`[read]` 가 표가 아닌 판도 합치기를 막지 않는다**(리뷰). 점 키와 같은 자다 — `Sheet::mark` 의
    /// 첫 문지기가 쓰기 전체를 거절하는데 그 거절은 사람이 손으로 적은 `read = 3` 한 줄에서 온다.
    /// 처음 짓는 파일에만 겹치던 동안은 빈 표에만 닿아 못 보던 자리고, 그때 적을 것이 없는
    /// `moai read` 는 0 으로 지나갔다. 부른 쪽의 제 id 는 그대로 거절당한다 — 그것이 *지금 쓰는 줄*이다.
    #[test]
    #[cfg(unix)]
    fn a_read_key_that_is_not_a_table_does_not_block_the_merge() {
        let s = Scratch::new("read-marks-scalar");
        let cfg = s.join("config.toml");
        std::fs::create_dir_all(s.join("real/proj")).unwrap();
        std::fs::write(s.join("막힌 것"), "x").unwrap();
        let gate = s.join("문");
        std::os::unix::fs::symlink("real", &gate).unwrap();
        let spelling = s.join("문/proj/../proj");
        let at = path_for(&cfg, &spelling);

        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("z", "내 것")]))).unwrap();
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("막힌 것", &gate).unwrap();
        update(&cfg, &spelling, |sh| sh.mark(&marks(&[("a", "떨어진 판")]))).unwrap();
        std::fs::remove_file(&gate).unwrap();
        std::os::unix::fs::symlink("real", &gate).unwrap();

        // 사람이 `[read]` 자리에 낱값을 적어 두었다.
        let path = std::fs::read_to_string(&at).unwrap().lines().next().unwrap().to_string();
        std::fs::write(&at, format!("{path}\nread = 3\n")).unwrap();

        // 적을 것이 없는 판은 그대로 지나간다 — 옛 자리를 합치려다 거절로 끝나지 않는다.
        let quiet = update(&cfg, &spelling, |sh| sh.mark(&BTreeMap::new()));
        assert!(quiet.is_ok(), "적을 것 없는 쓰기가 남의 줄에 막혔다 — {:?}", quiet.err());
        assert!(std::fs::read_to_string(&at).unwrap().contains("read = 3"), "사람이 적은 줄을 덮었다");
        // 부른 쪽의 제 id 는 그대로 거절당한다 — 그것이 지금 쓰는 줄이다.
        let mine = update(&cfg, &spelling, |sh| sh.mark(&marks(&[("y", "내가 쓰는 줄")])));
        assert!(mine.is_err(), "깨진 `[read]` 에 제 줄을 적었다");
    }

    /// **문지기도 푼 경로로 견준다**(moai-f5e3) — 파일 이름을 푼 경로로 고르면서 `path` 만 철자로 보면,
    /// 옛 철자로 적힌 줄을 남의 것으로 읽어 제 읽음을 안 읽는다.
    #[test]
    fn the_guard_compares_the_settled_path_too() {
        let s = Scratch::new("read-marks-owns");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        // 같은 자리를 딴 철자로 적어 둔 파일.
        let spelled = s.join("proj/../proj");
        std::fs::write(&at, format!("path = {:?}\n\n[read]\n\"a\" = \"A\"\n", spelled.display().to_string())).unwrap();

        let got = read(&cfg, &root, &BTreeMap::new());
        assert_eq!(got.trouble, None, "{:?}", got.problems);
        assert_eq!(got.seen.get("a").map(String::as_str), Some("A"), "제 파일을 남의 것으로 읽었다");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("b", "B")]))).unwrap();
        assert_eq!(
            read(&cfg, &root, &BTreeMap::new()).seen.get("b").map(String::as_str),
            Some("B"),
            "제 파일에 쓰기를 거절했다"
        );
    }

    /// **파일이 대는 자리는 그 이름을 고른 자리다**(moai-f5e3 리뷰). 이름은 푼 뿌리로 고르면서 `path` 에는
    /// 받은 철자를 적던 판은, 그 철자가 가리키던 링크가 사라지는 순간 제가 지은 파일을 남의 것으로 읽었다 —
    /// 읽기는 빈 표와 `Broken` 을, 쓰기는 "손으로 지운다" 를 냈고, 시키는 대로 지우면 그 프로젝트의 읽음이
    /// 통째로 사라졌다.
    #[test]
    #[cfg(unix)]
    fn the_place_a_sheet_claims_is_the_one_that_named_it() {
        let s = Scratch::new("read-marks-claim");
        let cfg = s.join("config.toml");
        let real = s.join("proj");
        std::fs::create_dir_all(&real).unwrap();
        let link = s.join("바로 가기");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let settled = std::fs::canonicalize(&link).unwrap();

        // 링크 철자로 적어도 파일은 푼 자리의 것 하나다.
        update(&cfg, &link, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        let at = path_for(&cfg, &link);
        assert_eq!(at, path_for(&cfg, &real), "링크와 실제 자리가 딴 파일로 갔다");
        let text = std::fs::read_to_string(&at).unwrap();
        assert!(text.contains(&settled.display().to_string()), "받은 철자를 적었다 — 이름과 짝이 어긋난다\n{text}");

        // 링크가 사라져도 제 파일이다 — 읽기도 쓰기도 그대로 선다.
        std::fs::remove_file(&link).unwrap();
        let got = read(&cfg, &real, &BTreeMap::new());
        assert_eq!(got.trouble, None, "제가 지은 파일을 남의 것으로 읽었다 — {:?}", got.problems);
        assert_eq!(got.seen.get("argos-0001").map(String::as_str), Some("A"));
        update(&cfg, &real, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap();
    }

    /// **읽음은 설정 철자가 가리키는 자리에 선다 — 링크를 따라가지 않는다**(moai-f5e3, 리뷰가 잰 것).
    ///
    /// `~/.config/moai/config.toml` 을 dotfiles 로 거는 것은 흔한 꼴인데, 한때 설정 **파일**을 풀어
    /// 자리를 고르던 판은 `read/` 를 통째로 그 저장소 안에 세웠다 — 지은 바이너리로 재 보니 그 밑에
    /// 파일과 락이 생겼다. 무엇을 읽었는지는 사적인 값이고, 남의 읽음이 내 diff 에 섞이지 않게 하려고
    /// 트래커 밖에 둔 것이 이 모듈의 첫 결정이다(2026-09-15). 버전 관리되는 자리에 넣으면 그 결정이
    /// 통째로 뒤집힌다.
    #[test]
    #[cfg(unix)]
    fn the_sheet_stays_where_the_config_spelling_says() {
        let s = Scratch::new("read-marks-dotfiles");
        let dots = s.join("dots/moai");
        std::fs::create_dir_all(&dots).unwrap();
        std::fs::write(dots.join("config.toml"), "").unwrap();
        // 사람이 흔히 거는 꼴 — XDG 자리의 설정 파일이 dotfiles 의 파일을 가리킨다.
        let xdg = s.join("xdg/moai");
        std::fs::create_dir_all(&xdg).unwrap();
        let cfg = xdg.join("config.toml");
        std::os::unix::fs::symlink(dots.join("config.toml"), &cfg).unwrap();
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();

        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        assert!(
            path_for(&cfg, &root).starts_with(&xdg),
            "읽음이 링크를 따라 나갔다 — {}",
            path_for(&cfg, &root).display()
        );
        assert!(!dots.join("read").exists(), "읽음이 dotfiles 저장소 안에 섰다");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen.get("argos-0001").map(String::as_str), Some("A"));
    }

    /// **설정 곁에 선다** — `MOAI_CONFIG` 를 옮기면 읽음도 따라간다. 자리를 따로 정하면 제 임시 설정을
    /// 준 시험이 돌리는 사람의 읽음을 읽는다.
    #[test]
    fn the_file_stands_beside_the_config_it_was_given() {
        let s = Scratch::new("read-marks-beside");
        let root = s.join("proj");
        for dir in ["one", "two"] {
            let cfg = s.join(dir).join("config.toml");
            assert!(path_for(&cfg, &root).starts_with(s.join(dir)), "{}", path_for(&cfg, &root).display());
        }
    }

    /// **옛 `[read]` 는 겹쳐 보고 안 지운다**(사용자 결정 3). 겹치면 새 자리가 이긴다 — 옛 표는 이
    /// 바이너리가 다시 안 적는 지나간 값이다.
    #[test]
    fn the_old_table_is_read_alongside_and_the_new_place_wins() {
        let s = Scratch::new("read-marks-legacy");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sheet| sheet.mark(&marks(&[("argos-0001", "새것")]))).unwrap();

        let legacy = marks(&[("argos-0001", "옛것"), ("argos-0009", "옛것뿐")]);
        let Marks { seen, problems, trouble } = read(&cfg, &root, &legacy);
        assert_eq!(seen, marks(&[("argos-0001", "새것"), ("argos-0009", "옛것뿐")]));
        assert!(problems.is_empty(), "{problems:?}");
        assert!(trouble.is_none(), "성한 읽기에 탈이 섰다 — {trouble:?}");
    }

    /// **트래커에 없는 id 만 걷는다**(moai-dt5q, 사용자 결정 2). 닫힌 줄은 남는다 — 걷으면 [NEW] 가
    /// 되살아나 읽음의 뜻이 바뀐다.
    #[test]
    fn pruning_drops_only_what_the_tracker_no_longer_has() {
        let s = Scratch::new("read-marks-prune");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let all = marks(&[("argos-0001", "A"), ("argos-0002", "B"), ("argos-0003", "C")]);
        update(&cfg, &root, |sheet| sheet.mark(&all)).unwrap();

        let known: BTreeSet<&str> = ["argos-0001", "argos-0003"].into_iter().collect();
        let gone = update(&cfg, &root, |sheet| Ok(sheet.prune(&known))).unwrap();
        assert_eq!(gone.value, 1);
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0001", "A"), ("argos-0003", "C")]));
    }

    /// **같은 때를 다시 안 적는다** — 헛 쓰기가 없고, 적은 id 만 돌려준다.
    #[test]
    fn writing_the_same_stamp_again_changes_nothing() {
        let s = Scratch::new("read-marks-idempotent");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        assert_eq!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap().value, ["argos-0001"]);
        let was = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        assert!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap().value.is_empty());
        assert_eq!(std::fs::read_to_string(path_for(&cfg, &root)).unwrap(), was, "같은 때를 다시 적어 파일이 바뀌었다");
        assert_eq!(update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "B")]))).unwrap().value, ["argos-0001"]);
    }

    /// **`.` 이 든 자식 id 는 따옴표에 싼 낱말 키로 적는다**(moai-j038.vna) — 맨 키로 적히면 다음 읽기가
    /// 점 찍은 키로 보아 `argos-0003` 표 밑의 `rv` 로 읽고, 그 줄의 읽음이 통째로 사라진다. 리뷰 이슈의
    /// id 가 늘 이 꼴이라 흔한 자리다.
    #[test]
    fn a_child_id_with_a_dot_is_written_as_one_quoted_key() {
        let s = Scratch::new("read-marks-dotted");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0003.rv", "A")]))).unwrap();
        let text = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        assert!(text.contains("\"argos-0003.rv\""), "맨 키로 적었다 — 다음 읽기가 못 찾는다\n{text}");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0003.rv", "A")]));
    }

    /// **다시 적는 줄의 주석과 키 모양도 그대로다**(리뷰) — `Table::insert` 로 갈아 끼우던 판은 키를 새로
    /// 지어 키 위의 주석·값 뒤의 주석·따옴표가 한꺼번에 사라졌다. 도장은 줄이 바뀔 때마다 다시 적히므로
    /// 드문 자리가 아니라 **늘 지나는 자리**다.
    #[test]
    fn re_marking_a_line_keeps_its_comments_and_key_shape() {
        let s = Scratch::new("read-marks-decor");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!(
            "path = {:?}\n\n[read]\n# 손으로 적은 까닭\n\"argos-0001\" = \"A\"  # 뒤 주석\n",
            root.display().to_string()
        );
        std::fs::write(&at, &src).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("# 손으로 적은 까닭"), "키 위의 주석이 사라졌다\n{now}");
        assert!(now.contains("\"argos-0001\" = \"B\"  # 뒤 주석"), "뒤 주석이나 따옴표가 사라졌다\n{now}");
    }

    /// **주석만 있던 파일의 머리 주석은 머리에 남는다**(moai-gmdu 에픽 리뷰가 설정에서 고친 자리, 리뷰) —
    /// 맨 `Table::new()` 로 표를 세우면 그 주석이 `[read]` 밑으로 밀려 마지막 읽음에 붙은 말로 읽힌다.
    #[test]
    fn a_head_comment_stays_above_the_first_read_table() {
        let s = Scratch::new("read-marks-head");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "# 이 파일에 적어 둔 까닭\n").unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.starts_with("# 이 파일에 적어 둔 까닭\n"), "머리 주석이 표 밑으로 밀렸다\n{now}");
    }

    /// **줄 끝과 끝 줄바꿈은 원문대로 돌려준다**(moai-lb0u·moai-r9qa, 리뷰) — 라이브러리는 줄 끝을 `\n`
    /// 으로 접어, 안 되돌리면 CRLF 로 든 dotfiles 저장소에서 `moai read` 한 번이 모든 줄을 바꾼다.
    #[test]
    fn crlf_and_a_missing_final_newline_survive_a_write() {
        let s = Scratch::new("read-marks-crlf");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, format!("path = {:?}\r\n\r\n[read]\r\n\"argos-0001\" = \"A\"", root.display().to_string()))
            .unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(!now.replace("\r\n", "").contains('\n'), "줄 끝이 LF 로 접혔다 — {now:?}");
        assert!(!now.ends_with('\n'), "없던 끝 줄바꿈을 더했다 — {now:?}");
    }

    /// **낱말이 아닌 `path` 는 남의 것으로 읽는다**(리뷰) — 참으로 읽던 판은 `claim` 이 그 자리를 안 덮는
    /// 것과 어긋나, `path = 3` 하나가 붙은 파일을 해시가 닿는 모든 뿌리가 제 것으로 여겨 함께 적었다.
    #[test]
    fn a_path_that_is_not_a_word_is_not_owned() {
        let s = Scratch::new("read-marks-odd-path");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "path = 3\n\n[read]\n\"argos-0001\" = \"A\"\n").unwrap();
        assert!(read(&cfg, &root, &BTreeMap::new()).seen.is_empty(), "남의 읽음을 들었다");
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
    }

    /// **글자로 못 적는 뿌리도 제 읽음을 든다**(리뷰) — `display()` 가 U+FFFD 로 바꿔 적은 줄을 `owns` 가
    /// 바이트째 견주면, 제가 방금 지은 파일을 다음 명령이 남의 것으로 읽고 거절한다. 그 거절은 손으로
    /// 지우라 하는데, 지우면 같은 줄이 다시 적혀 고리가 안 끊긴다.
    #[cfg(unix)]
    #[test]
    fn a_root_that_is_not_utf8_can_still_write_twice() {
        use std::os::unix::ffi::OsStrExt;
        let s = Scratch::new("read-marks-nonutf8");
        let cfg = s.join("config.toml");
        let root = s.path().join(std::ffi::OsStr::from_bytes(b"re\xffpo"));
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).expect("제가 지은 파일을 남의 것으로 읽었다");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0001", "A"), ("argos-0002", "B")]));
    }

    /// **걷기도 때가 적힌 줄만 손댄다**(리뷰) — 손으로 적은 맨 점 키는 부모 id 의 표로 읽히는데, 부모가
    /// 트래커에 없다고 그 표를 지우면 자식의 읽음과 주석이 함께 사라진다. `mark` 가 그 자리를 안 덮는
    /// 것과 같은 자여야 한다.
    #[test]
    fn pruning_leaves_alone_what_marking_refuses_to_touch() {
        let s = Scratch::new("read-marks-prune-odd");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!("path = {:?}\n\n[read]\n\"argos-0002\".rv = \"A\"\n", root.display().to_string());
        std::fs::write(&at, &src).unwrap();
        let known: BTreeSet<&str> = BTreeSet::new();
        assert_eq!(update(&cfg, &root, |sh| Ok(sh.prune(&known))).unwrap().value, 0, "때가 아닌 자리를 걷었다");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), src, "걷을 것이 없는데 파일을 고쳤다");
    }

    /// **걷기가 주석을 데려가지 않는다**(리뷰) — 맨 `TableLike::remove` 는 키 위의 주석을 **빈 줄 너머까지**
    /// 함께 지워, 읽음을 한 번 걷는 것이 사람이 적어 둔 글과 앞 줄의 꼬리를 말없이 가져갔다. 빼는 자는
    /// 보기를 뺄 때와 같은 [`crate::user_config::Doc::drop_keys`] 하나다(moai-liij).
    #[test]
    fn pruning_keeps_the_comments_around_what_it_drops() {
        let s = Scratch::new("read-marks-prune-comments");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!(
            "path = {:?}\n\n[read]\n# 이 파일에 대해 적어 둔 말\n\n\"argos-0001\" = \"A\"\n\"argos-0002\" = \"B\"\n",
            root.display().to_string()
        );
        std::fs::write(&at, &src).unwrap();
        let known: BTreeSet<&str> = ["argos-0002"].into_iter().collect();
        assert_eq!(update(&cfg, &root, |sh| Ok(sh.prune(&known))).unwrap().value, 1);
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("# 이 파일에 대해 적어 둔 말"), "걷기가 빈 줄 너머의 주석을 데려갔다\n{now}");
        assert!(!now.contains("argos-0001"), "걷을 것을 안 걷었다\n{now}");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0002", "B")]));
    }

    /// **모르는 키와 주석은 그대로 간다** — 새 바이너리가 적은 것을 옛 바이너리가 한 번 만져 지우면 안 된다.
    #[test]
    fn unknown_keys_and_comments_survive_a_write() {
        let s = Scratch::new("read-marks-keep");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!(
            "# 손으로 적은 줄\npath = {:?}\nnote = \"나중 바이너리의 키\"\n\n[read]\n\"argos-0001\" = \"A\"\n",
            root.display().to_string()
        );
        std::fs::write(&at, &src).unwrap();
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap();
        let now = std::fs::read_to_string(&at).unwrap();
        assert!(now.contains("# 손으로 적은 줄"), "{now}");
        assert!(now.contains("note = \"나중 바이너리의 키\""), "{now}");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0001", "A"), ("argos-0002", "B")]));
    }

    /// **남의 읽음 위에 쓰지 않는다** — 해시가 부딪히면 읽기는 빈 표와 까닭 한 줄, 쓰기는 멈춘다.
    /// 조용히 섞는 것이 이 에픽이 고치는 바로 그 해다.
    #[test]
    fn a_sheet_that_belongs_to_another_root_is_never_touched() {
        let s = Scratch::new("read-marks-collision");
        let cfg = s.join("config.toml");
        let (mine, other) = (s.join("proj"), s.join("남의 것"));
        let at = path_for(&cfg, &mine);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, format!("path = {:?}\n\n[read]\n\"argos-0001\" = \"A\"\n", other.display().to_string()))
            .unwrap();

        let Marks { seen, problems, trouble } = read(&cfg, &mine, &BTreeMap::new());
        assert!(seen.is_empty(), "남의 읽음을 들었다 — {seen:?}");
        assert_eq!(problems.len(), 1, "{problems:?}");
        // 사람이 고쳐야 같아지는 탈이다 — 다시 읽어도 같으니 `Broken` 이다(`Reading` 이면 걸음마다 다시 읽는다).
        assert_eq!(trouble, Some(crate::user_config::Trouble::Broken), "{trouble:?}");
        let e = update(&cfg, &mine, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
        assert!(std::fs::read_to_string(&at).unwrap().contains("argos-0001"), "남의 파일을 덮었다");
    }

    /// **낱말이 아닌 줄 하나는 못 든 것이 아니다**(리뷰) — 그 줄만 건너뛰고 나머지는 그대로 낸다.
    /// 둘을 한 `problems` 로 내던 판은 탐색기가 성한 표를 통째로 버려, 손으로 적은 맨 점 키 하나가
    /// 그 프로젝트의 줄을 모두 [NEW] 로 세웠다. **까닭에는 어느 파일인지가 붙는다** — 이름이 해시라
    /// 안 붙이면 사람이 그 파일을 못 찾는다.
    #[test]
    fn one_odd_row_is_skipped_and_the_rest_still_load() {
        let s = Scratch::new("read-marks-lenient");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            &at,
            format!("path = {:?}\n\n[read]\n\"argos-0001\" = \"A\"\nargos-0002 = 3\n", root.display().to_string()),
        )
        .unwrap();
        let got = read(&cfg, &root, &BTreeMap::new());
        assert_eq!(got.seen, marks(&[("argos-0001", "A")]), "성한 줄까지 버렸다");
        assert!(got.trouble.is_none(), "건너뛴 줄 하나를 못 든 것으로 셌다 — {:?}", got.trouble);
        assert_eq!(got.problems.len(), 1, "{:?}", got.problems);
        assert!(got.problems[0].contains(&at.display().to_string()), "어느 파일인지를 안 댔다 — {:?}", got.problems);
    }

    /// **못 읽은 것과 깨진 것을 가른다**(리뷰) — 탐색기가 표식을 올릴지를 이것으로 가르므로, 권한
    /// 하나가 세션 내내 [NEW] 를 세워 두던 자리가 여기다.
    ///
    /// **권한은 잠깐이 아니다**(moai-po6v) — 다시 해도 같고, 고치는 `chmod` 은 표식을 안 바꿔 표식으로는
    /// 영영 못 벗어난다. 그래서 갈래는 `Unreadable` 이고 벗어나는 길은 시계다
    /// (`App::a_sheet_we_cannot_read_again_is_retried_by_the_clock`). 가르는 자는 설정과 한 자다
    /// (`user_config::unreadable`).
    #[cfg(unix)]
    #[test]
    fn a_sheet_i_cannot_open_is_transient_not_broken() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("read-marks-eacces");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        let at = path_for(&cfg, &root);
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000)).unwrap();
        let got = read(&cfg, &root, &BTreeMap::new());
        // root 로 돌리면 권한이 안 걸린다 — 그때는 이 시험이 잴 것이 없다.
        if got.trouble.is_some() {
            assert_eq!(got.trouble, Some(crate::user_config::Trouble::Unreadable), "{:?}", got.problems);
            assert!(got.seen.is_empty(), "못 읽고도 표를 냈다");
        }
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen, marks(&[("argos-0001", "A")]));
    }

    /// **깨진 파일에는 안 쓴다** — 읽기는 까닭을 대고 빈 표로 지나간다.
    #[test]
    fn a_broken_sheet_is_told_on_read_and_refused_on_write() {
        let s = Scratch::new("read-marks-broken");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "[read\n\"argos-0001\" = ").unwrap();
        assert!(read(&cfg, &root, &BTreeMap::new()).seen.is_empty());
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).problems.len(), 1);
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
    }

    /// **때가 아닌 값이 있으면 하나도 안 적는다** — 맨 점 키(`a-0002.rv = …`)는 표로 읽히는데, 그 위에
    /// 때를 덮으면 자식의 읽음과 주석이 말없이 사라진다.
    #[test]
    fn a_value_that_is_not_a_stamp_stops_the_whole_write() {
        let s = Scratch::new("read-marks-odd");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let at = path_for(&cfg, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let src = format!("path = {:?}\n\n[read]\n\"argos-0002\".rv = \"A\"\n", root.display().to_string());
        std::fs::write(&at, &src).unwrap();
        let e = update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0002", "B")]))).unwrap_err();
        assert_eq!(e.code, crate::fail::code::BROKEN, "{e}");
        assert_eq!(std::fs::read_to_string(&at).unwrap(), src, "거절하고도 파일을 고쳤다");
    }

    /// **적을 것이 없으면 파일을 안 만든다** — 빈 쓰기 하나 때문에 아직 아무것도 안 한 사람의 집에
    /// 디렉터리와 락 파일이 생긴다.
    #[test]
    fn an_empty_write_leaves_no_file_behind() {
        let s = Scratch::new("read-marks-empty");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&BTreeMap::new())).unwrap();
        let at = path_for(&cfg, &root);
        assert!(!at.exists(), "빈 쓰기가 파일을 지었다");
        // **락 파일과 디렉터리까지 센다**(moai-dyb7). `.toml` 만 보던 판은 프로젝트마다 쌓이는 0바이트
        // `<해시>.toml.lock` 과 `read/` 를 그대로 지나갔다 — 남의 설정 디렉터리에 남는 쓰레기다.
        assert!(!crate::store::lock_beside(&at).exists(), "빈 쓰기가 락 파일을 남겼다");
        assert!(!dir_of(&at).exists(), "빈 쓰기가 `read/` 를 지었다");

        // 쓸 것이 있으면 셋 다 선다 — 재 보기가 진짜 쓰기를 삼키지 않는다.
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A")]))).unwrap();
        assert!(at.exists(), "쓸 것이 있는데 안 적었다");
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen.get("argos-0001").map(String::as_str), Some("A"));
    }

    /// **옛 자리에만 있는 읽음은 재 보기가 삼키지 않는다**(moai-dyb7). 락 밖의 재 보기가 옛 자리를
    /// 안 겹쳐 보면 "쓸 것이 없다" 로 돌아서, 첫 쓰기 한 번에 합치기로 한 것(리뷰 13)이 안 일어난다.
    #[test]
    fn the_trial_run_merges_the_older_places_too() {
        let s = Scratch::new("read-marks-trial-past");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let slashed = s.join("proj/");
        let old = sheet_at(dir_of(&cfg), &slashed);
        std::fs::create_dir_all(dir_of(&old)).unwrap();
        std::fs::write(&old, format!("path = {:?}\n\n[read]\n\"a\" = \"옛 철자\"\n", slashed.display().to_string()))
            .unwrap();

        // 적을 것은 없다 — 옛 자리의 한 줄만이 쓸 까닭이다.
        update(&cfg, &slashed, |sh| sh.mark(&BTreeMap::new())).unwrap();
        assert_eq!(
            read(&cfg, &slashed, &BTreeMap::new()).seen.get("a").map(String::as_str),
            Some("옛 철자"),
            "재 보기가 옛 자리를 안 보고 돌아섰다"
        );
    }

    /// **동시에 적어도 서로를 안 지운다**(CLAUDE.md — 조용한 손실이 이 도구가 못 견디는 유일한 실패다).
    /// 락 없이 읽고 되쓰면 나중에 `rename` 한 갈래가 앞의 읽음을 통째로 덮는다.
    #[test]
    fn concurrent_marks_all_survive() {
        let s = Scratch::new("read-marks-concurrent");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        let (threads, each) = (8, 5);
        std::thread::scope(|scope| {
            for t in 0..threads {
                let (cfg, root) = (cfg.clone(), root.clone());
                scope.spawn(move || {
                    for i in 0..each {
                        let id = format!("argos-{t}{i:02}");
                        update(&cfg, &root, |sh| sh.mark(&marks(&[(&id, "A")]))).unwrap();
                    }
                });
            }
        });
        assert_eq!(read(&cfg, &root, &BTreeMap::new()).seen.len(), threads * each, "동시에 적은 읽음이 서로를 지웠다");
    }

    /// **읽고 그대로 쓰면 바이트가 같다** — 헛 diff 가 없다. 도구가 짓는 파일이어도 dotfiles 저장소에
    /// 들어가면 사람이 그 diff 를 본다.
    #[test]
    fn reading_and_writing_back_changes_no_bytes() {
        let s = Scratch::new("read-marks-idem-bytes");
        let cfg = s.join("config.toml");
        let root = s.join("proj");
        update(&cfg, &root, |sh| sh.mark(&marks(&[("argos-0001", "A"), ("argos-0002", "B")]))).unwrap();
        let was = std::fs::read_to_string(path_for(&cfg, &root)).unwrap();
        let known: BTreeSet<&str> = ["argos-0001", "argos-0002"].into_iter().collect();
        update(&cfg, &root, |sh| {
            sh.mark(&marks(&[("argos-0001", "A")]))?;
            Ok(sh.prune(&known))
        })
        .unwrap();
        assert_eq!(std::fs::read_to_string(path_for(&cfg, &root)).unwrap(), was);
    }

    /// **이름은 안 바뀐다** — 바뀌면 모든 사람의 읽음이 한 번에 사라진다. 셈은 [`crate::text::fnv1a64`]
    /// 가 제 시험값으로 못박으므로, 여기서는 **그 셈에 매였다는 것**을 이름으로 못박는다.
    #[test]
    fn the_hash_is_pinned_so_the_names_never_move() {
        let name = |root: &str| path_for(Path::new("/c/config.toml"), Path::new(root)).file_name().unwrap().to_owned();
        assert_eq!(name("/a/api"), "c812cb6e42a00af2.toml");
        assert_ne!(name("/a/api"), name("/b/api"));
        assert_eq!(path_for(Path::new("/c/config.toml"), Path::new("/a/api")).parent().unwrap(), Path::new("/c/read"));
    }
}
