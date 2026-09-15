//! 물려받으면 git 이 **엉뚱한 저장소·바깥 설정·바깥 사람**을 보게 되는 변수들 — **목록은 이 파일 하나다.**
//!
//! 두 무리다. [`REPO`] 는 어느 저장소를 보는가를 바꾸므로 **릴리스에서도 걷는다**(moai-ztdf, 사용자
//! 결정). [`TEST`] 는 사람·시계·해시라 시험 빌드에서만 걷는다 — 시험이 제 값으로 커밋하기 위한 것이지
//! 릴리스가 지울 까닭이 없다. `git::command` 가 무리를 고르고, 통합 시험의 `isolated`(tests/cli.rs)는
//! 둘 다 걷는다. 그쪽은 따로 된 크레이트고 이 패키지는 바이너리뿐이라 `use` 로는 못 가져가서 `#[path]`
//! 로 이 파일을 읽는다 — 두 벌로 두면 한쪽에만 더한 변수가 말없이 갈라진다.
//!
//! **어디서 오는가**(git 2.43 에서 잰 것). *딸린* 워크트리의 훅과 `rebase -x` 는 `GIT_DIR`·`GIT_INDEX_FILE`
//! 을 준다 — 주 워크트리의 `rebase -x` 는 저장소를 가리키는 것을 하나도 안 준다. pre-receive 훅은
//! `GIT_DIR=.`·`GIT_OBJECT_DIRECTORY`·`GIT_ALTERNATE_OBJECT_DIRECTORIES`·`GIT_QUARANTINE_PATH` 를, 모든
//! 커밋 훅은 `GIT_AUTHOR_*` 를, 바깥의 `git -c` 는 `GIT_CONFIG_PARAMETERS` 를 준다. 나머지
//! (`GIT_WORK_TREE`·`GIT_COMMON_DIR`·`GIT_CONFIG_COUNT`·`GIT_NAMESPACE`)는 훅이 주지는 않지만 사람이
//! 내보낼 수 있는 같은 식구다(`git rev-parse --local-env-vars`).
//!
//! **남으면 무엇이 깨지는가.** 이 변수들은 `git -C <경로>` 를 **이긴다**. 그래서 훅 안에서 부른
//! `moai -C <다른 프로젝트>` 가 훅 저장소의 `user.name` 을 읽어 그 프로젝트의 저널에 **남의 이름을
//! 영구히** 적고(`model::git_config`), 커밋 칸과 워크트리 겹쳐 보기가 훅 저장소의 이력을 읽는다.
//! 시험에서는 `git -C <임시 저장소>` 가 바깥 저장소에 쓰고, `GIT_QUARANTINE_PATH` 하나만 남아도 임시
//! 저장소의 커밋이 `ref updates forbidden inside quarantine environment` 로 막히며, `GIT_AUTHOR_NAME` 이
//! 남으면 `-c user.name=t` 로 박은 사람이 바깥 사람으로 바뀐다(moai-g1a3).

/// 어느 저장소를 보는가를 바꾸는 것 — **릴리스도 걷는다.** 어느 저장소를 볼지는 언제나 `-C` 나
/// `.moai` 찾기가 정하고, 물려받은 값이 그것을 이기지 못한다.
pub const REPO: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_QUARANTINE_PATH",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

/// 시험 빌드에서만 걷는 것 — 사람·시계·해시와 환경으로 넣는 설정.
///
/// **릴리스는 안 걷는다.** 커밋 훅 안에서 moai 를 부른 사람이 `GIT_AUTHOR_NAME` 이나 `git -c` 로 준
/// 값은 그 사람이 일부러 준 것이다. 지금 moai 는 사람을 `git config user.name` 으로만 읽지만, 여기서
/// 걷어 버리면 앞으로도 그 값을 못 읽는다.
pub const TEST: &[&str] = &[
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    // 시험이 제 시계·제 사람으로 커밋한다. 걷기가 먼저라, 제 값을 주는 시험은 그 뒤에 덮어 살아남는다.
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
    // 기계의 기본 해시. 남으면 sha256 저장소가 서서 해시가 40자인지 보는 시험이 깨진다.
    "GIT_DEFAULT_HASH",
];
