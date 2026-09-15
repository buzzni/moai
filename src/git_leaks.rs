//! 물려받으면 시험의 git 이 바깥 저장소·바깥 설정·바깥 사람을 보게 되는 변수들 — **목록은 이 파일 하나다.**
//!
//! 단위 시험은 `git::command` 가, 통합 시험은 tests/cli.rs 의 `isolated` 가 이것을 걷는다. 그쪽은 따로 된
//! 크레이트고 이 패키지는 바이너리뿐이라 `use` 로는 못 가져가서 `#[path]` 로 이 파일을 읽는다 — 두 벌로
//! 두면 한쪽에만 더한 변수가 말없이 갈라진다.
//!
//! **어디서 오는가**(git 2.43 에서 잰 것). *딸린* 워크트리의 훅과 `rebase -x` 는 `GIT_DIR`·`GIT_INDEX_FILE`
//! 을 준다 — 주 워크트리의 `rebase -x` 는 저장소를 가리키는 것을 하나도 안 준다. pre-receive 훅은
//! `GIT_DIR=.`·`GIT_OBJECT_DIRECTORY`·`GIT_ALTERNATE_OBJECT_DIRECTORIES`·`GIT_QUARANTINE_PATH` 를, 모든
//! 커밋 훅은 `GIT_AUTHOR_*` 를, 바깥의 `git -c` 는 `GIT_CONFIG_PARAMETERS` 를 준다. 나머지
//! (`GIT_WORK_TREE`·`GIT_COMMON_DIR`·`GIT_CONFIG_COUNT`·`GIT_NAMESPACE`)는 훅이 주지는 않지만 사람이
//! 내보낼 수 있는 같은 식구다(`git rev-parse --local-env-vars`).
//!
//! **남으면 무엇이 깨지는가.** `git -C <임시 저장소>` 가 바깥 저장소를 읽거나 바깥 객체 저장소에 쓰고,
//! `GIT_QUARANTINE_PATH` 하나만 남아도 임시 저장소의 커밋이 `ref updates forbidden inside quarantine
//! environment` 로 막히며, `GIT_AUTHOR_NAME` 이 남으면 `-c user.name=t` 로 박은 사람이 바깥 사람으로 바뀐다
//! (moai-g1a3).

pub const LEAKS: &[&str] = &[
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_QUARANTINE_PATH",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
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
