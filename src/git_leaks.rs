//! 물려받으면 git 이 **엉뚱한 저장소·바깥 설정·바깥 사람**을 보게 되는 변수들 — **목록은 이 파일 하나다.**
//!
//! 두 무리다. [`REPO`] 는 **`-C` 로 댄 저장소의 답을 바꾸는 것** — 어느 저장소를 여는가, 어느 이력을
//! 걷는가, `git config` 가 어떤 값을 내는가 — 이라 **릴리스에서도 걷는다**(moai-ztdf, 2026-09-15 사용자
//! 결정). [`TEST`] 는 사람·시계·해시라 시험 빌드에서만 걷는다 — 시험이 제 값으로 커밋하기 위한 것이지
//! 릴리스가 지울 까닭이 없다. `git::command` 가 무리를 고르고, 통합 시험의 `isolated`(tests/cli.rs)는
//! 둘 다 걷는다. 그쪽은 따로 된 크레이트고 이 패키지는 바이너리뿐이라 `use` 로는 못 가져가서 `#[path]`
//! 로 이 파일을 읽는다 — 두 벌로 두면 한쪽에만 더한 변수가 말없이 갈라진다.
//!
//! **어디서 오는가**(git 2.43 에서 잰 것). *딸린* 워크트리의 훅과 `rebase -x` 는 `GIT_DIR`·`GIT_INDEX_FILE`
//! 을 준다 — 주 워크트리의 `rebase -x` 는 저장소를 가리키는 것을 하나도 안 준다. **주 워크트리의 커밋
//! 훅도 `GIT_DIR` 을 안 준다**(상대 경로 `GIT_INDEX_FILE` 만 준다). 그 모양에서 [`REPO`] 의 걷기는
//! 걷을 것이 없고, 거기서 실제로 오는 것은 `GIT_AUTHOR_*` 와 `GIT_CONFIG_PARAMETERS` 다. pre-receive 훅은
//! `GIT_DIR=.`·`GIT_OBJECT_DIRECTORY`·`GIT_ALTERNATE_OBJECT_DIRECTORIES`·`GIT_QUARANTINE_PATH` 를, 모든
//! 커밋 훅은 `GIT_AUTHOR_*` 를, 바깥의 `git -c` 는 `GIT_CONFIG_PARAMETERS` 를 준다. 나머지
//! (`GIT_WORK_TREE`·`GIT_COMMON_DIR`·`GIT_CONFIG_COUNT` 등)는 훅이 주지는 않지만 사람이 내보낼 수 있는
//! 같은 식구다(`git rev-parse --local-env-vars`). `GIT_NAMESPACE`·`GIT_QUARANTINE_PATH` 는 그 목록에는
//! 없지만 같은 값을 하므로 손으로 얹은 것이다 — 목록에 없다고 지우면 moai-g1a3 이 돌아온다.
//!
//! **남으면 무엇이 깨지는가.** [`REPO`] 는 두 길로 `-C` 를 이긴다. `GIT_DIR` 무리는 **어느 저장소를
//! 여는지**를 바꿔 커밋 칸과 워크트리 겹쳐 보기가 훅 저장소의 이력을 내게 하고, `GIT_CONFIG_PARAMETERS`
//! 무리는 저장소는 그대로 둔 채 **`git config` 가 내는 값**을 바꾼다. 둘 다 끝은 같다 — `model::git_config`
//! 가 남의 이름을 읽어 그 프로젝트의 저널에 **영구히** 적는다. [`TEST`] 는 그 답을 안 바꾼다 —
//! `GIT_AUTHOR_*` 는 이 도구가 한 번도 안 읽고 커밋의 사람만 바꾼다. 시험에서는
//! `git -C <임시 저장소>` 가 바깥 저장소에 쓰고, `GIT_QUARANTINE_PATH` 하나만 남아도 임시 저장소의
//! 커밋이 `ref updates forbidden inside quarantine environment` 로 막히며, `GIT_AUTHOR_NAME` 이 남으면
//! `-c user.name=t` 로 박은 사람이 바깥 사람으로 바뀐다(moai-g1a3). **둘을 한 문장으로 묶지 않는다** —
//! 묶는 순간 이 파일이 목록을 둘로 가른 까닭이 지워진다.

/// git 이 **저장소 지역 환경**으로 세는 것 — 어느 저장소를, 어느 이력을, 어느 설정 파일을 보는가를
/// 바꾼다. **릴리스도 걷는다.** 어느 저장소를 볼지는 `-C` 나 `.moai` 찾기가 정하고, 물려받은 값이
/// 그것을 이기지 못한다.
///
/// **git 이 제 `--local-env-vars` 로 대는 이름은 하나도 안 빠뜨린다** — `git::tests` 의
/// `the_leak_list_covers_what_git_calls_local` 이 git 에게 직접 물어 잰다. 손으로 적은 목록이라 git 이
/// 이름을 하나 더하면 조용히 낡는데, 낡은 것이 드러나는 자리가 남의 저널뿐이면 그때는 이미 늦다.
/// 거꾸로 git 이 안 대는데 여기 든 것도 있다 — `GIT_QUARANTINE_PATH`·`GIT_NAMESPACE`·
/// `GIT_CEILING_DIRECTORIES`·`GIT_DISCOVERY_ACROSS_FILESYSTEM`·`GIT_CONFIG_GLOBAL`·`GIT_CONFIG_SYSTEM`
/// 은 같은 값을 하므로 손으로 얹었다. 그 시험은 그것들을 못 보므로 **지운다고 붉어지지 않는다.**
///
/// **그 시험은 경계도 못 붙잡는다** — 이름이 `REPO` 와 [`TEST`] 중 어디 들었는지는 안 본다. 릴리스가
/// 실제로 걷느냐는 tests/cli.rs 의 `an_inherited_git_dir_does_not_beat_the_project_we_were_given` 이
/// 이 목록을 통째로 심어서 붙잡는다.
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
    "GIT_IMPLICIT_WORK_TREE",
    // **설정 파일을 통째로 딴 데로 돌린다.** `model::git_config` 는 말 그대로 `git config <키>` 라,
    // 이것 하나로 사람이 남의 파일에서 온다 — `-C` 는 못 이긴다(재 봤다).
    "GIT_CONFIG",
    // **이력을 조용히 끊거나 고쳐 쓴다.** 커밋 칸은 `git log` 하나로 서므로, 남으면 이슈에 커밋이
    // 없는 것처럼 보인다 — `GIT_SHALLOW_FILE` 하나로 세 줄이 두 줄이 되는 것을 쟀다.
    "GIT_SHALLOW_FILE",
    "GIT_GRAFT_FILE",
    "GIT_REPLACE_REF_BASE",
    "GIT_NO_REPLACE_OBJECTS",
    // **찾기를 막는다.** git 이 안 대는 이름이지만 여기가 제 자리다 — `GIT_DIR` 을 늘 걷게 된 뒤로는
    // 어느 저장소인지를 **찾기**가 혼자 정하므로, 뿌리 위에 천장이 서면 `git -C <뿌리> config` 가
    // 저장소를 못 찾아 조용히 전역 사람으로 떨어지고 커밋 칸은 까닭 없이 빈다(moai-ztdf 리뷰).
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    // **설정을 환경으로 넣는 것** (2026-09-15 사용자 결정, moai-ztdf 리뷰 발견 1). `git config` 가 내는
    // **값**을 이겨 `-C` 로 댄 저장소의 사람을 갈아 치운다 — 재 봤다: 바깥이 `git -c user.name=남 commit`
    // 으로 띄운 훅 안에서 `moai -C B add` 가 B 의 저널에 `by: 남` 을 영구히 적었다. **주 워크트리의
    // 커밋 훅은 `GIT_DIR` 을 안 주므로**(위 "어디서 오는가") 가장 흔한 훅 모양에서는 이것이 유일하게
    // 새는 길이고, 그 길을 막는 것이 이 에픽의 목적이다. 일부러 준 값이라는 말은 **제 저장소**에
    // 적을 때 서는 것이지 `-C` 로 남의 프로젝트에 적을 때 따라가지 않는다.
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    // 설정 **파일**을 통째로 돌리는 것. `GIT_CONFIG` 와 한 식구이고 git 의 `--local-env-vars` 에는
    // 둘 다 없다 — 그래서 `the_leak_list_covers_what_git_calls_local` 은 이 셋을 못 본다.
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
];

/// 시험 빌드에서만 걷는 것 — **사람과 시계와 해시**.
///
/// **릴리스는 안 걷는다.** 커밋 훅 안에서 moai 를 부른 사람이 `GIT_AUTHOR_NAME` 으로 준 값은 그 사람이
/// 일부러 준 것이다. 지금 moai 는 사람을 `git config user.name` 으로만 읽지만, 여기서 걷어 버리면
/// 앞으로도 그 값을 못 읽는다.
///
/// **설정을 환경으로 넣는 것은 여기 없다** — `GIT_CONFIG_PARAMETERS`·`GIT_CONFIG_COUNT` 는 [`REPO`] 로
/// 갔다(2026-09-15 사용자 결정). 그 둘은 "사람이 준 값" 이 아니라 `git config` 가 내는 값을 **이기는**
/// 것이라, `-C` 로 댄 남의 프로젝트 저널에 남의 이름을 적는 길이었다. 가르는 자는 "사람이 줬는가" 가
/// 아니라 **"`-C` 가 가리킨 저장소의 답을 바꾸는가"** 다.
pub const TEST: &[&str] = &[
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

/// 걷을 이름들 — `with_test` 면 [`TEST`] 까지 얹는다.
///
/// **두 무리를 잇는 자리는 여기 하나다.** `git::command` 도 통합 시험의 `isolated`(tests/cli.rs)도
/// 이것을 부른다. 잇는 식을 부르는 쪽마다 두면, 무리가 하나 늘거나 `TEST` 가 `REPO` 처럼 늘 걷히게
/// 되는 날 한 곳을 빠뜨려도 아무도 모른다 — 목록을 한 파일에 둔 것과 같은 까닭이다(moai-g1a3).
pub fn swept(with_test: bool) -> impl Iterator<Item = &'static &'static str> {
    REPO.iter().chain(if with_test { TEST } else { &[] as &[&str] })
}
