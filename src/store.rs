//! 저장소를 찾고 읽고 쓴다. **파일을 만지는 곳은 여기뿐이다** (`cmd::init` 제외).
//!
//! `issues.jsonl` 을 바꾸는 길은 [`Repo::with_write`] 하나다. 명령마다 쓰기
//! 경로가 갈라지면 락·정렬·검증·원자적 쓰기를 저마다 반쯤 구현하게 된다.

use crate::config::Config;
use crate::fail::{Fail, R, code};
use crate::model::{Actor, Issue, JournalEntry};
use fs2::FileExt;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 락을 못 잡으면 **아무것도 쓰지 않고** 물러난다.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct Repo {
    pub root: PathBuf,
    pub config: Config,
    /// 찾은 자리가 딸린 워크트리라 **루트로 옮겨 온 것인가** — 그 워크트리의 자리다(moai-y7go).
    /// [`Repo::here`] 가 이것을 낸다. 조용히 딴 파일을 고치면 시킨 쪽은 제가 친 자리에 썼다고 믿어,
    /// `main` 이 프로세스 끝에 [`redirects`] 로 한 줄을 남긴다.
    ///
    /// **읽는 문은 [`Repo::here`] 하나다** — 밖에서 직접 풀면(`moved_from.unwrap_or(root)`) 이 다발이
    /// 명령마다 다시 서고, 한 곳만 안 고쳐져도 그 표면만 조용히 루트를 가리킨다.
    moved_from: Option<PathBuf>,
}

impl Repo {
    /// 시험과 딴 자리를 여는 쪽이 쓰는 생성자 — 옮겨 온 것이 아니다.
    pub fn at(root: PathBuf, config: Config) -> Repo {
        Repo { root, config, moved_from: None }
    }

    /// 시험용 — **딸린 워크트리에서 루트의 트래커로 옮겨 온** [`Repo`]. [`Repo::here`] 와
    /// [`Repo::root`] 가 갈린다. [`Repo::at`] 만으로 세운 시험은 둘이 늘 같아, 둘을 헷갈린 자리를
    /// 하나도 못 잡는다(탐색기가 읽음 파일을 `here()` 로 고르던 자리, 리뷰).
    #[cfg(test)]
    pub fn moved(root: PathBuf, config: Config, from: PathBuf) -> Repo {
        Repo { root, config, moved_from: Some(from) }
    }

    /// 같은 트래커를 **딴 체크아웃의 눈으로** — [`Repo::here`] 만 갈아 끼운다.
    ///
    /// 훅이 쓴다(moai-acf7): `MOAI_HERE=1` 로 제 `.moai` 를 든 워크트리가 `moai -C <루트> …` 를 치면
    /// 판정할 스냅샷은 루트의 것이지만 **누구인가는 그 워크트리**다. 이름을 루트에서 읽으면 그
    /// 워크트리가 쥔 일이 통째로 "옆의 것" 이 되어(`worktree::away`) 규칙이 제 일을 못 본다.
    ///
    /// **읽기 전용 [`Repo`] 에만 쓴다.** `moved_from` 을 읽는 자리는 [`Repo::here`] 말고 둘 더
    /// 있고(`note_held` 의 물러설 자리, [`with_write`] 가 [`MOVED`] 에 미는 줄), 그 둘에게 이
    /// 값은 거짓말이다 — 아무것도 옮겨 오지 않았다. 지금 부르는 자리(`cmd/hook.rs` 의 `route_one`)
    /// 는 그 [`Repo`] 로 `read()` 만 한다. 여기로 쓰는 길이 생기면 `moved_from` 을 쪼갠다.
    pub fn seen_from(self, here: PathBuf) -> Repo {
        Repo { moved_from: Some(here), ..self }
    }

    /// 이 세션이 **선 체크아웃** — 트래커를 루트로 옮겨 왔으면 옮겨 오기 전의 자리다(moai-y7go).
    ///
    /// [`Repo::root`] 는 **트래커가 사는 곳**이다. 둘은 딸린 워크트리에서만 갈리는데, 이 저장소는
    /// 일을 모두 워크트리에서 하므로 그때가 늘이다. "지금 어느 체크아웃인가" 를 묻는 자리
    /// (훅이 세는 파일·워크트리 이름·`git log` 의 `HEAD`·`.claude/` 에 심는 것)는 이것을 쓴다 —
    /// `root` 로 물으면 워크트리의 파일이 죄다 루트의 `.claude/worktrees/…` 밑으로 보인다.
    pub fn here(&self) -> &Path {
        self.moved_from.as_deref().unwrap_or(&self.root)
    }
}

/// 이 프로세스가 **딸린 워크트리에서 루트의 트래커로 옮겨 쓴 것** — `(선 자리, 쓴 트래커)`.
///
/// [`MISSED`] 와 같은 까닭으로 `store` 는 담아 두기만 하고 찍지 않는다. 찍는 자리는 `main` 이다:
/// 대체 화면을 쥔 탐색기 안에서 `eprintln!` 은 그림을 망가뜨리고(`tui::draw` 의 배너가 그래서
/// 있다), 쓰기 **전에** 찍으면 락이나 검증에 걸려 아무것도 안 쓴 명령이 "썼다" 고 말한다.
///
/// **자리마다 한 줄이다**([`TALLY`]·[`MISSED`] 와 같은 자, 리뷰 moai-71ht 셋째 판) — 탐색기는 층에서
/// 여러 프로젝트에 쓰고, `Repo::open` 도 옮겨 가므로 옮겨 쓴 자리가 둘일 수 있다. 한 칸만 두던 판은
/// 마지막 하나만 알렸다. 같은 자리에 열 번 써도 줄은 하나다.
static MOVED: std::sync::Mutex<Vec<(PathBuf, PathBuf)>> = std::sync::Mutex::new(Vec::new());

/// 옮겨 쓴 자리들 — 없으면 비어 있다. `main` 이 끝에 한 번 읽는다.
pub fn redirects() -> Vec<(PathBuf, PathBuf)> {
    MOVED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 이 프로세스가 **명령을 친 자리** — `-C` 가 옮기기 전의 현재 디렉터리다([`remember_invoked`]).
///
/// 집은 표식([`Repo::note_held`])이 "어느 체크아웃이 집었나" 를 이것으로 가른다. 트래커를 **찾은**
/// 길(`-C`·위로 찾기·등록한 자리)로 가르던 판은 규약이 권하는 `moai -C <루트> mv <id> in_progress`
/// 를 워크트리에서 친 집기를 통째로 빠뜨려 산 일을 `stranded` 로 댔고, 루트에서 `-C <워크트리>` 로
/// 친 것과 탐색기가 등록한 워크트리를 연 것은 남의 자리에 집기를 적었다(리뷰 moai-71ht 셋째 판).
static INVOKED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// `-C` 를 따르기 **전에** `main` 이 한 번 부른다. 두 번째부터는 아무것도 안 한다.
pub fn remember_invoked() {
    let _ = INVOKED.set(std::env::current_dir().ok());
}

/// `MOAI_HERE` 가 **켜져 있는가** — 빈 값과 흔한 "아니오" 낱말은 끈 것으로 읽는다.
///
/// 글은 `MOAI_HERE=1` 로 적혀 있어 사람은 이것을 참·거짓으로 읽는다. 있기만 하면 켜던 판은
/// `MOAI_HERE=0` 을 켬으로 읽어, 끄려던 사람에게 말없이 갈라진 스냅샷을 줬다.
/// 켜는 낱말만 켠다 — `crate::skill` 의 `MOAI_BLESS` 와 같은 자다(거기도 `MOAI_BLESS=0` 이
/// 커밋된 트리를 다시 쓰던 자리였다).
pub(crate) fn here_wanted() -> bool {
    let v = std::env::var_os("MOAI_HERE");
    let v = v.as_ref().map(|v| v.to_string_lossy().trim().to_ascii_lowercase());
    matches!(v.as_deref(), Some("1" | "true" | "yes" | "on"))
}

/// 읽다가 만난 잘못된 줄. **한 줄이 깨졌다고 파일을 통째로 거부하지 않는다** —
/// 거부하면 무엇이 잘못됐는지 볼 방법까지 같이 사라진다.
#[derive(Debug)]
pub struct LoadError {
    pub line: usize,
    pub message: String,
    /// 못 읽은 줄의 **원문 그대로**. 이것이 있어야 되쓸 때 그 줄을 잃지 않는다.
    pub text: String,
    /// 그 줄이 쓰고 있는 id. **줄을 `Issue` 로 못 읽는 것과 그 안의 `id` 를
    /// 못 읽는 것은 다른 일이다** — 한 단 낮게(`serde_json::Value`) 읽으면
    /// 대개 나온다. JSON 도 아닌 줄에서는 `None` 이고, 그때는 지어내지 않는다.
    pub id: Option<String>,
}

#[derive(Debug, Default)]
pub struct Load {
    pub issues: Vec<Issue>,
    pub errors: Vec<LoadError>,
}

impl Load {
    /// id 로 줄을 찾는다. **같은 id 의 줄이 둘이면 뒷줄이다.**
    ///
    /// 트리·탐색기(`nav::Index::find`)와 id 로 짠 지도(`report::groups`·`milestones`·
    /// `misplaced`)가 모두 뒷줄을 고른다. 여기만 앞줄이면 `moai show <id>` 의 머리 제목·
    /// 필드는 앞줄 것이고 멤버 셈은 뒷줄 것인 한 화면이 서고, 종류가 다른 쌍둥이면
    /// 가려진 줄(`report::eclipsed`)을 열어 멤버가 통째로 빈다(moai-e0ro). 중복은
    /// 쓰기가 거부하고 `duplicate_id` 가 드러내는 깨진 상태라, 여기는 읽는 자만 맞춘다.
    pub fn get(&self, id: &str) -> Option<&Issue> {
        self.issues.iter().rfind(|i| i.id == id)
    }

    /// 못 읽는 줄이 이미 쓰고 있는 id. **새 id 를 여기서 피해 뽑는다.**
    ///
    /// 안 피하면 못 읽는 동안은 아무 데도 안 보이는 중복이 생기고, 그 줄이
    /// 읽히게 되는 날(새 바이너리로 갈아타면) `duplicate_id` 가 서서 모든
    /// 쓰기가 막힌다 — 그때는 어느 줄을 고쳐야 하는지도 사람이 알아내야 한다.
    pub fn reserved_ids(&self) -> BTreeSet<String> {
        self.errors.iter().filter_map(|e| e.id.clone()).collect()
    }

    /// 못 읽는 줄을 `report` 가 받는 모양으로. **그 줄이 쓰는 id 를 함께 넘긴다** —
    /// id 가 있어야 산 줄과의 중복이 드러난다(moai-4dk4).
    ///
    /// 한 줄짜리지만 자리마다 손으로 적으면 언젠가 `None` 으로 적는 곳이 생기고, 그러면
    /// 그 화면만 중복을 못 본다. 옆의 [`Load::reserved_ids`] 가 같은 `errors` 에서 뽑는
    /// 다른 파생값이라 짝으로 둔다.
    pub fn unreadable(&self) -> Vec<crate::report::Unreadable<'_>> {
        self.errors.iter().map(|e| crate::report::Unreadable { id: e.id.as_deref() }).collect()
    }
}

/// `.moai` 를 못 찾았을 때의 말. **다른 곳의 저장소를 부르는 길(`-C`)을 함께 댄다** —
/// 등록한 프로젝트를 한눈에 보는 `status` 에 익은 사람은 `.moai` 밖에서 `add`·`mv` 도
/// 될 줄 알고, 쓰는 명령은 어느 프로젝트인지 모르니 멈추는 것이 맞다(moai-6au6).
pub const NOT_A_REPO: &str =
    "moai 저장소가 아니다 (.moai/ 를 못 찾았다). `moai init` 으로 시작하거나, 다른 곳의 저장소면 `moai -C <dir> <명령>` 으로 부른다";

/// 이 프로세스가 **제 체크아웃 밖으로 올라가 잡은 트래커** — `(선 자리, 잡은 뿌리)`.
///
/// [`MOVED`] 와 **같은 꼴로** 담아 두기만 하고 `main` 이 찍는다. 담는 자리는 다르다 — 아래를 본다.
///
/// **제 체크아웃 안에서 올라간 것은 한 줄도 안 낸다.** 제 저장소의 하위 디렉터리에 선 사람에게
/// 매번 알리면 그건 소음이고, 소음은 곧 아무도 안 읽는 줄이다. 남는 것은 잡은 트래커가 내가 선
/// 체크아웃 **밖**일 때뿐이고(`~/.moai` 하나가 홈 아래 전부를 잡는 꼴, 그리고 `.moai` 없는 클론
/// 안에서 그 위의 트래커를 잡는 꼴), 그게 바로 알릴 값이 있는 자리다.
///
/// **적는 말이 "썼다" 가 아니다.** 여기 담기는 것은 *찾기*의 결과라 `status`·`ready`·`show` 도
/// 지난다 — `MOVED` 처럼 "썼다" 로 적으면 읽기만 한 명령이 일어나지 않은 쓰기를 주장한다.
static CLIMBED: std::sync::Mutex<Vec<(PathBuf, PathBuf)>> = std::sync::Mutex::new(Vec::new());

/// 올라가 잡은 자리들 — 없으면 비어 있다. `main` 이 끝에 한 번 읽는다.
pub fn climbs() -> Vec<(PathBuf, PathBuf)> {
    CLIMBED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `.moai` 를 가진 조상을 찾는다 — **위로 끝까지 간다.**
///
/// **천장을 두었다가 걷었다**(moai-a2kn, 2026-09-20 사용자 결정 둘째 판). 한때 "내 것이 아닌
/// 디렉터리로는 안 올라간다" 를 세웠는데, 둘째 판 리뷰가 재 보니 값이 무거웠다.
///
/// - **훅이 조용히 꺼졌다.** 훅은 트래커를 읽기만 하는데(`cmd::hook` 에 `with_write` 가 없다)
///   천장에 걸리면 [`Repo::discover`] 가 실패하고 `decide` 가 `None` 을 내어, 규칙 셋이
///   stderr 한 줄 없이 통째로 꺼졌다. 막아서 얻는 것이 없는 자리에서 잃는 것만 컸다
/// - **임자(uid)로 재는 것이 양쪽으로 틀렸다.** 조에 공유한 체크아웃·CI 가 받아 둔 체크아웃·
///   `sudo` 는 제 트래커를 잃고, 모두 uid 0 인 컨테이너에서는 아무것도 안 막았다
/// - **거절문이 준 길이 도리어 가르쳤다.** `moai -C <남의 자리>` 는 천장을 그냥 지나고,
///   `여기서 moai init` 은 트래커를 하나 더 세운다
///
/// 남은 것은 **읽기는 관대하고 쓰기는 엄하다** 는 규약 그대로다 — 찾기는 안 막고, 못 쓰는
/// 트래커는 쓰기가 제 자리에서 거절한다(파일 권한이 이미 그 문이다). 대신 어느 트래커를
/// 잡았는지를 [`CLIMBED`] 가 한 줄로 비춘다.
fn look(from: &Path) -> Option<PathBuf> {
    let (root, climbed) = climb(from)?;
    // **적는 것은 이 명령이 선 자리에서 올라간 때뿐이다.** 훅은 셸 명령에서 읽어 낸 남의
    // 디렉터리로도 트래커를 찾아 보므로(`cmd::hook::route_one`), 그것까지 적으면 손도 안 댄
    // 프로젝트를 잡았다고 말한다 — 아직 만들지도 않은 디렉터리를 대기도 한다.
    if climbed && std::env::current_dir().is_ok_and(|cwd| cwd == from) {
        let mut told = CLIMBED.lock().unwrap_or_else(|e| e.into_inner());
        let pair = (from.to_path_buf(), root.clone());
        if !told.contains(&pair) {
            told.push(pair);
        }
    }
    Some(root)
}

/// 찾은 뿌리와 **체크아웃을 두고 올라왔는가**. 뒤의 값이 알림을 가른다.
///
/// 가르는 자는 오는 길에 지난 `.git` 이다 — 트래커보다 **아래**에 있는 `.git` 은 "내가 선
/// 체크아웃을 두고 그 밖의 트래커를 잡았다" 는 뜻이고, 그것이 알릴 값이 있는 모양이다.
///
/// - `.moai` 없는 클론 안에서 친 `add` 가 그 위의 `~/.moai` 에 드는 것 — 알린다
/// - `P/vendor`(겹쳐 둔 저장소)에서 P 의 트래커를 쓰는 것 — 알린다. 일부러 살려 둔 길이지만
///   어느 트래커에 드는지는 보여야 한다
/// - 모노레포의 `top/projA/sub` 가 `top/projA/.moai` 를 잡는 것 — 조용하다(같은 체크아웃 안)
/// - git 을 안 쓰는 프로젝트의 하위 디렉터리 — 조용하다. 거기서는 "내 프로젝트 위" 와 "남의
///   트래커 위" 를 가를 값이 아예 없어, 매번 한 줄을 내면 그건 소음이고 소음은 곧 아무도 안
///   읽는 줄이다. 이 한 자리는 못 본 채로 둔다
fn climb(from: &Path) -> Option<(PathBuf, bool)> {
    let mut dir = from.to_path_buf();
    let mut left_a_checkout = false;
    loop {
        // **못 들여다보는 조상은 건너뛴다** (`is_dir` 이 `false` 로 접는다). 위로 찾는
        // 길에서는 권한 없는 남의 디렉터리를 지나는 것이 흔한 일이라, [`Repo::open`]
        // 처럼 그것을 실패로 세면 제 저장소 밖 어디서나 넘어진다.
        if dir.join(".moai").is_dir() {
            return Some((dir, left_a_checkout));
        }
        if !left_a_checkout {
            left_a_checkout = dir.join(".git").exists();
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// 디렉터리 하나를 [`Repo::open`] 으로 연 결과.
///
/// **셋을 가른다.** 등록한 프로젝트를 한눈에 볼 때 "아직 `init` 안 했다" 와
/// "디렉터리가 사라졌다" 는 사람이 할 일이 다르다 — 앞은 `moai init`, 뒤는
/// 등록을 뺀다. 둘을 한 `None` 으로 접으면 받는 쪽이 파일 시스템을 다시 뒤져야
/// 하고, 그 뒤짐은 부르는 곳마다 조금씩 달라진다.
///
/// 설정이 깨졌거나 디렉터리를 못 읽는 것은 여기가 아니라 `Err` 다 — 고칠 것이지
/// 상태가 아니다.
#[derive(Clone)]
pub enum Opened {
    Repo(Repo),
    /// 디렉터리는 있는데 `.moai/` 가 없다.
    Uninit,
    /// 디렉터리가 없다.
    Missing,
}

impl Repo {
    /// `.moai/` 를 가진 디렉터리를 위로 찾는다. 깊이를 코드에 박지 않는다([`look`]).
    pub fn discover() -> R<Repo> {
        Repo::find()?.ok_or_else(|| NOT_A_REPO.into())
    }

    /// [`Repo::discover`] 와 같되 **못 찾은 것을 실패로 접지 않는다** — `None`.
    ///
    /// 못 찾은 것과 찾았는데 설정이 깨진 것은 다르다. `.moai` 밖에서 부른
    /// `status` 는 앞의 것일 때만 등록한 프로젝트를 보여 줘야 한다 — 뒤의 것까지
    /// 한눈 보기로 넘기면 제 저장소의 깨진 설정이 남의 프로젝트 목록 뒤에 숨는다.
    pub fn find() -> R<Option<Repo>> {
        let dir = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
        Repo::find_from(&dir)
    }

    /// [`Repo::find`] 를 준 디렉터리에서 — 훅이 명령이 가리키는 트래커(`-C`·`cd`)를 찾을 때 쓴다.
    ///
    /// **딸린 워크트리에서 찾으면 루트의 트래커를 낸다**(moai-y7go, 2026-09-19 사용자 결정).
    /// 워크트리의 `.moai` 를 고치면 병합에서 스냅샷이 충돌하고, 푸는 길이 도구 밖에만 남는다 —
    /// 규약이 "트래커는 워크트리 안에서 쓰지 않는다" 인 까닭이다. 그 글을 훅으로 지키게 하던 판은
    /// **명령 이름**을 보고 막아, `Edit`·`sed -i`·`echo >>`·사람의 터미널·예제 스크립트가 다
    /// 비켜갔다(리뷰 moai-71ht.rv0). 고칠 자리는 트래커를 찾는 이 한 곳이다.
    ///
    /// **막지 않는다** — 옮겨 갈 뿐이라 게이트가 아니다. 읽기도 함께 옮겨 간다: 쓰기만 옮기면
    /// 명령이 **갈라질 때의 낡은 줄**로 id 를 풀고 지금 줄에 쓴다.
    ///
    /// 옮겨 가지 않는 자리 둘 — 루트에 `.moai` 가 없거나([`Repo::find_from`] 이 위로 찾다 만난
    /// 워크트리가 그 저장소의 것이 아니다) 주 체크아웃을 못 찾는 것(서브모듈·맨 저장소)이다.
    /// 그때는 찾은 그대로다.
    ///
    /// **루트의 설정이 깨진 것은 조용히 되돌아가지 않는다**(리뷰 moai-71ht.jlh). 삼키고 워크트리에
    /// 쓰던 판은 루트의 `config.toml` 에 충돌 표시 하나가 박히는 순간 저장소의 모든 워크트리가
    /// 말없이 제 스냅샷에 쓰기 시작해, 이 기능이 막으려던 갈라짐을 아무 말 없이 지었다. 쓸 트래커를
    /// 못 여는 것은 고칠 것이지 갈래가 아니다 — 루트에서 치면 나는 그 오류를 여기서도 그대로 낸다.
    pub fn find_from(dir: &Path) -> R<Option<Repo>> {
        Repo::found_root(dir).map(Repo::from_found).transpose()
    }

    /// 찾은 자리로 [`Repo`] 를 짓는다 — [`Repo::find_from`] 과 [`Repo::discover`] 가 같이 쓴다.
    /// 둘이 따로 적으면 한쪽만 `MOAI_HERE` 를 보거나 한쪽만 워크트리를 안 옮긴다.
    ///
    /// **`MOAI_HERE` 는 이 체크아웃에 쓴다.** 일부러 갈라 놓는 자리다 — 옆 스냅샷이 갈라진 상태를
    /// 짓는 시험과, 그 워크트리에서만 쓰는 트래커를 든 사람이다. 옮기는 것은 막는 것이 아니라
    /// 옮기는 것이므로, 되돌릴 손잡이 하나를 두는 값이 싸다. **끄는 값도 받는다** — 글이 `=1` 로
    /// 적혀 있어 `MOAI_HERE=0` 을 "아니오" 로 읽고 쓰는 쪽이 생기는데, 있기만 하면 켜던 판은
    /// 그 사람에게 말없이 갈라진 스냅샷을 줬다(리뷰 moai-71ht.jlh).
    fn from_found(found: PathBuf) -> R<Repo> {
        let Some(root) = Repo::redirect(&found) else {
            return Repo::rooted(found);
        };
        // **설정은 한 번만 읽는다** — 찾은 자리로 [`Repo`] 를 지어 놓고 버리던 판은 워크트리의
        // `config.toml` 을 읽고 안 쓴 채 버렸다. 훅이 도구 호출마다 지나는 길이다.
        Ok(Repo { moved_from: Some(found), ..Repo::rooted(root)? })
    }

    /// [`Repo::find_from`] 과 같되 **안 옮긴다** — 찾은 자리의 트래커 그대로다.
    ///
    /// 옮겨 갈 루트를 못 읽을 때(거기 `config.toml` 이 깨졌다) 물러설 자리다. 훅이 그 자리로
    /// 선다 — `moai` 는 크게 실패하는 것이 맞지만, 훅까지 조용해지면 그 한 파일 때문에 저장소의
    /// 모든 워크트리에서 규칙이 통째로 꺼진다(리뷰 moai-71ht.i1u).
    pub fn find_here(dir: &Path) -> R<Option<Repo>> {
        Repo::found_root(dir).map(Repo::rooted).transpose()
    }

    /// 이 자리의 트래커가 **옮겨 갈 루트** — 옮기지 않을 자리면 `None`.
    ///
    /// [`Repo::find_from`]·[`Repo::open`] 과 탐색기 층의 표식(`tui::layer::marks_of`)이 **이 한 자로**
    /// 가른다. `open` 만 손잡이를 안 보던 판은 `MOAI_HERE=1` 을 켠 사람에게 CLI 와 탐색기가 같은
    /// 자리에서 다른 파일을 읽게 했다(리뷰 moai-71ht 셋째 판).
    fn redirect(found: &Path) -> Option<PathBuf> {
        (!here_wanted()).then(|| crate::worktree::tracker_root(found)).flatten()
    }

    /// 이 자리를 [`Repo::open`] 으로 열면 **읽을 트래커의 뿌리** — 옮길 곳이 없으면 그 자리 그대로다.
    /// 탐색기 층이 다시 읽을 까닭을 잴 때 이것으로 표식을 뜬다 — 읽는 파일과 재는 파일이 갈리면
    /// 그 줄은 시계가 돌 때까지 낡은 셈을 낸다.
    pub fn opened_root(dir: &Path) -> PathBuf {
        Repo::redirect(dir).unwrap_or_else(|| dir.to_path_buf())
    }

    /// [`Repo::find_from`] 의 **찾기만** — `.moai` 를 가진 조상의 자리다. 설정은 안 읽는다:
    /// 읽을 자리를 [`crate::worktree::tracker_root`] 가 아직 옮길 수 있다.
    fn found_root(dir: &Path) -> Option<PathBuf> {
        look(dir)
    }

    fn rooted(root: PathBuf) -> R<Repo> {
        let config = Config::load(&root)?;
        Ok(Repo::at(root, config))
    }

    /// **준 디렉터리 그 자리의** `.moai/` 를 연다. 위로 찾지 않는다 — 다만 딸린 워크트리면 같은
    /// 나무의 주 체크아웃으로 옮겨 간다([`Repo::redirect`]).
    ///
    /// 등록한 경로는 사람이 "이것이 프로젝트다" 라고 이름 댄 뿌리다. 위로 찾으면
    /// 모노레포에서 `.moai` 없는 하위 디렉터리가 바깥 저장소의 이슈를 제 이름으로
    /// 내 같은 이슈가 두 프로젝트에 두 번 선다 — "init 전" 이 정직한 답이다. 옮기는 곳은 위가
    /// 아니라 **옆**이라(같은 나무의 같은 자리) 그 갈림이 안 산다.
    ///
    /// 경로는 **받은 철자 그대로** 뿌리가 된다. 링크를 풀지 않는다 — 풀지 말지는
    /// 경로를 가진 쪽(`user_config::resolve_dir`)이 이미 정했다.
    pub fn open(dir: &Path) -> R<Opened> {
        let gone = |e: &std::io::Error| {
            // 경로 중간이 파일이면(`file/sub`) `NotADirectory` 다 — 없는 것과 같다.
            matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory)
        };
        match std::fs::metadata(dir) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(Fail::new(format!("디렉터리가 아니다 — {}", dir.display()))),
            Err(e) if gone(&e) => return Ok(Opened::Missing),
            Err(e) => return Err(Fail::new(format!("{}: {e}", dir.display()))),
        }
        match std::fs::metadata(dir.join(".moai")) {
            // **여기도 루트로 옮겨 간다**(moai-y7go, 리뷰 moai-71ht.jlh 사용자 결정) — 등록한 자리가
            // 딸린 워크트리면 탐색기의 쓰기가 그 워크트리의 스냅샷에 조용히 들어가, 같은 자리를 CLI 로
            // 칠 때와 다른 파일이 바뀐다. **위로 찾지 않는다는 계약은 그대로다** — 옮기는 곳은 위가
            // 아니라 같은 나무의 주 체크아웃이고, 거기에 트래커가 없으면 옮기지 않는다.
            Ok(m) if m.is_dir() => match Repo::redirect(dir) {
                Some(root) => Ok(Opened::Repo(Repo { moved_from: Some(dir.to_path_buf()), ..Repo::rooted(root)? })),
                None => Repo::rooted(dir.to_path_buf()).map(Opened::Repo),
            },
            // `.moai` 가 파일이면 저장소가 아니다 — 위로 찾는 [`Repo::find`] 의 `is_dir` 과 같은 자다.
            Ok(_) => Ok(Opened::Uninit),
            // **`.moai` 가 없는 딸린 워크트리도 옮겨 간다**(리뷰 moai-71ht 셋째 판) — moai 를 들이기 전에
            // 갈라진 가지다. "init 전" 으로 대던 판은 `moai -C <워크트리> init` 을 시켰는데, 그 뒤로는
            // 어느 길도 그 트래커를 안 읽고(CLI 는 위로 찾아 루트로 간다) 커밋하면 병합에서 `config.toml`
            // 이 add/add 로 부딪힌다 — 아무도 안 읽는 파일을 만들라고 시킨 셈이었다.
            Err(e) if gone(&e) => match Repo::redirect(dir) {
                Some(root) => Ok(Opened::Repo(Repo { moved_from: Some(dir.to_path_buf()), ..Repo::rooted(root)? })),
                None => Ok(Opened::Uninit),
            },
            // 권한 없음 따위는 init 전이 아니다. 접으면 "init 하라" 는 틀린 말을 한다.
            Err(e) => Err(Fail::new(format!("{}: {e}", dir.join(".moai").display()))),
        }
    }

    /// **이 체크아웃에서 집은 줄을 git 관리 디렉터리에 적어 둔다**(moai-y7go, 2026-09-19 사용자 결정).
    ///
    /// 쓰기가 루트로 옮겨 가면서([`Repo::find_from`]) 워크트리의 스냅샷이 더는 안 움직인다. 이름이
    /// id 가 아닌 워크트리(에이전트 격리)는 그래서 "여기서 집었다" 를 말할 길을 잃었고, 산 일이
    /// `report::places` 에서 자리를 잃어 `stranded` 로 섰다(리뷰 moai-71ht.jlh). 스냅샷에는 안 적는다 —
    /// 그것이 병합에서 겨루는 파일이다.
    ///
    /// **집은 자리는 명령을 친 체크아웃이다**([`INVOKED`], 리뷰 moai-71ht 셋째 판) — 트래커를 어느
    /// 길로 찾았는지가 아니다. 규약이 권하는 `moai -C <루트> mv <id> in_progress` 를 워크트리에서
    /// 치면 옮긴 것이 없어(`moved_from` 이 빈다) 집기가 통째로 빠졌고, 그 반대(루트에서 `-C <워크트리>`,
    /// 탐색기가 등록한 워크트리를 연 것)는 아무도 일하지 않는 자리에 집기를 적었다.
    ///
    /// **놓기는 어디서 쳤든 적는다** — 칸을 떠난 줄과 지운 줄은 [`crate::worktree::note_held`] 가
    /// 이 저장소의 모든 표식에서 뺀다. 규약대로 루트에서 닫은 줄이 옛 워크트리의 표식에 남으면, 그
    /// 줄을 다시 집는 순간 아무도 없는 자리가 "여기서 돈다" 로 서서 `stranded` 가 영영 안 선다.
    ///
    /// **칸이 안 바뀐 쓰기는 지나간다** — 제목·본문·태그만 고친 쓰기가 워크트리마다 표식을 훑을
    /// 까닭이 없다. 새로 선 첫 칸 줄도 마찬가지다: 어느 표식에도 없다.
    fn note_held(&self, before: &[Issue], after: &[Issue]) {
        let was: std::collections::BTreeMap<&str, &str> =
            before.iter().map(|i| (i.id.as_str(), i.status.as_str())).collect();
        let (mut claimed, mut released) = (Vec::new(), Vec::new());
        for i in after {
            let started = self.config.is_started(i.status.as_str());
            match was.get(i.id.as_str()) {
                Some(w) if *w == i.status.as_str() => continue,
                None if !started => continue,
                _ => {}
            }
            if started { claimed.push(i.id.clone()) } else { released.push(i.id.clone()) }
        }
        // 지운 줄도 뺀다 — 없는 id 가 표식에 남으면 `places` 가 그 워크트리를 영영 댄다.
        let here: std::collections::BTreeSet<&str> = after.iter().map(|i| i.id.as_str()).collect();
        released.extend(before.iter().filter(|i| !here.contains(i.id.as_str())).map(|i| i.id.clone()));
        if claimed.is_empty() && released.is_empty() {
            return;
        }
        // 친 자리를 모르는 길(`main` 을 안 지나는 시험)은 트래커를 찾은 자리로 가늠한다.
        let at = INVOKED.get().cloned().flatten().or_else(|| self.moved_from.clone());
        crate::worktree::note_held(&self.root, at.as_deref(), &claimed, &released);
    }

    pub fn dir(&self) -> PathBuf {
        self.root.join(".moai")
    }
    pub fn issues_path(&self) -> PathBuf {
        self.dir().join("issues.jsonl")
    }
    pub fn journal_path(&self) -> PathBuf {
        self.dir().join("journal.jsonl")
    }

    /// 전부 메모리로 읽는다. 디스크 인덱스는 두지 않는다 — 이전 시도가
    /// SQLite 인덱스를 만들어 재 보고 **순수 손해**임을 확인했다.
    ///
    /// **읽기는 락을 잡지 않는다.** 쓰기가 `rename` 으로 갈아끼우므로 독자는
    /// 옛 파일 아니면 새 파일을 보지, 찢어진 파일을 볼 수 없다.
    pub fn read(&self) -> R<Load> {
        Ok(read_snapshot(&self.issues_path())?.unwrap_or_default())
    }

    /// `issues.jsonl` 을 바꾸는 **유일한 경로**.
    ///
    /// 락 → (락 안에서) 읽기 → 고치기 → 정규화·검증·정렬 → 원자적 교체 →
    /// 저널 추가. 순서가 중요하다: **스냅샷 먼저, 저널 나중.** 중간에 죽으면
    /// 저널에 줄이 하나 비는데(이력 공백), 반대 순서면 저널이 일어나지 않은
    /// 일을 주장한다. 빠진 일기가 거짓말하는 일기보다 싸다.
    /// 닫는 함수는 `(이슈들, 설정, 못 읽는 줄이 이미 쓰는 id)` 를 받는다.
    /// 셋째 것을 **인자로 주는 까닭**은 안 쓰는 쪽이 잊을 수 없게 하려는
    /// 것이다 — 락 안에서 읽은 것이라 밖에서 다시 구하면 그 사이에 달라진다.
    pub fn with_write<T, F>(&self, f: F) -> R<T>
    where
        F: FnOnce(&mut Vec<Issue>, &Config, &BTreeSet<String>) -> R<(Vec<JournalEntry>, T)>,
    {
        let _lock = Lock::acquire(&self.dir().join("lock"))?;

        // 락을 잡은 **뒤에** 읽는다. 밖에서 읽으면 두 프로세스가 같은 옛 상태를
        // 고쳐 쓰고, 나중에 rename 한 쪽이 앞의 이슈를 조용히 지운다.
        let load = self.read()?;
        // **못 읽은 줄 때문에 쓰기를 막지 않는다.** 들고 있다가 그대로 되쓴다.
        //
        // 한때 여기서 통째로 거절했다. 그러면 뒷 단계 바이너리가 쓴 줄 하나가
        // 앞 단계 사람의 `add`·`mv`·`edit` 을 전부 막아, 되돌릴 방법이 도구
        // 밖에만 남는다 — CLAUDE.md 가 이름 붙여 둔 실패다. 엄함은 *지금 쓰는
        // 줄*에 대한 것이지 파일 전체에 대한 것이 아니다.
        //
        // 잃지도 않고 막지도 않는 대신 **시끄럽다**: `moai status` 가
        // `unreadable_line` 을 치명으로 내고 거기서만 비영 종료한다.
        let opaque: Vec<&str> = load.errors.iter().map(|e| e.text.as_str()).collect();
        let before = render_issues(&load.issues, &opaque);

        // 정규화한 원본을 들고 있다가 **바뀐 줄만** 검사한다.
        //
        // 전부 검사하면 남의 낡은 줄 하나가 모든 쓰기를 막는다 — config 에서
        // 칸 이름을 하나 고치는 순간 그 칸에 있던 이슈 때문에 `moai add` 조차
        // 안 된다. 읽기는 관대하고 쓰기는 엄하다는 규칙은 **지금 쓰는 줄**에
        // 대한 것이지, 파일 전체에 대한 것이 아니다.
        let mut original = load.issues.clone();
        for o in original.iter_mut() {
            o.normalize();
        }

        let reserved = load.reserved_ids();
        let mut issues = load.issues;
        let (entries, out) = f(&mut issues, &self.config, &reserved)?;

        // **글의 크기는 한 자리에서 잰다**(moai-m9a8). 노트·`mv -m`·`defer -m`·제목·본문이 모두
        // 여기를 지나므로 명령마다 따로 걸면 한 곳은 반드시 잊는다. 저널에 적힐 글은 여기서, 제목과
        // 본문은 아래 바뀐 줄에서 — 둘 다 스냅샷을 쓰기 전이라 거절하면 아무것도 안 남는다.
        //
        // **이 쓰기가 짓는 줄은 id 로 안 부른다**(moai-1rkl) — 거절하면 그 id 는 어디에도 안 남아,
        // 받는 쪽이 없는 것을 찾으러 간다. 그 줄의 제목 한 토막으로 가리킨다.
        for e in &entries {
            for (what, t) in [("노트", &e.text), ("메모", &e.note)] {
                let Some(t) = t else { continue };
                // **가리키는 말은 거절할 때만 짓는다**([`crate::model::check_text_size`] 가 늦게
                // 부른다). 미리 지으면 `create` 처럼 글이 없는 저널 줄까지 `original` 과 `issues` 를
                // 통째로 훑고 제목을 두 벌 베낀다 — 8,000줄 계획이면 락을 쥔 채 그 헛일이 8,000번이다.
                //
                // **제목은 저널 줄에도 있다.** 스냅샷에서 못 찾는 줄(같은 쓰기가 세우고 다시
                // 지운 줄)에 빈 글로 떨어지던 판은 `새 줄 ''` 을 냈다 — id 도 제목도 아니라,
                // 받는 쪽에 손잡이가 하나도 안 남는다. `create`·`rm` 이 옮겨 적은 `title` 을
                // 다음 자리로 두고, 그것마저 없으면 id 라도 댄다.
                let at = || match original.iter().any(|o| o.id == e.id) {
                    true => e.id.clone(),
                    false => match issues
                        .iter()
                        .find(|i| i.id == e.id)
                        .map(|i| i.title.as_str())
                        .or(e.title.as_deref())
                    {
                        Some(t) => crate::model::unwritten(t),
                        None => e.id.clone(),
                    },
                };
                crate::model::check_text_size(at, what, t)?;
            }
        }

        for i in issues.iter_mut() {
            i.normalize();
            let was = original.iter().find(|o| o.id == i.id);
            if was != Some(&*i) {
                // 제목과 본문은 **이번에 바뀌었을 때만** 잰다 — 이미 큰 것을 든 줄도 옮기고 고칠 수 있다.
                //
                // **제목도 여기서 막아야 저널이 막힌다.** `create`·`rm` 이 제목을 저널의 `title` 로
                // 옮겨 적는데, 제목을 안 재면 `moai add "<대화록 한 줄>"` 이 노트와 똑같이 저널에 영영
                // 남는다. 저널의 `title` 을 위에서 재지 않는 것은 그것이 스냅샷 제목의 사본이라서다 —
                // 거기서 재면 옛 큰 제목을 든 줄을 `rm` 으로도 못 치운다.
                for (what, now, before) in [
                    ("제목", Some(&i.title), was.map(|o| &o.title)),
                    ("본문", i.body.as_ref(), was.and_then(|o| o.body.as_ref())),
                ] {
                    if let Some(text) = now
                        && now != before
                    {
                        // 이번에 지은 줄이면 id 가 아니라 제목으로 가리킨다(moai-1rkl).
                        // **거절할 때만 짓는다** — 넘치는 글은 드물고 `unwritten` 은 제목을 두 벌
                        // 베낀다. 500줄 계획이면 그 헛일을 500번 하고 전부 버리던 자리다.
                        let at = || match was {
                            Some(_) => i.id.clone(),
                            None => crate::model::unwritten(&i.title),
                        };
                        crate::model::check_text_size(at, what, text)?;
                    }
                }
                // **칸을 안 건드린 쓰기는 칸 이름을 다시 안 묻는다**(moai-hym7, 사람이
                // 정했다). 바뀐 줄만 재는 것과 같은 까닭이 한 겹 더 든 것이다 — `config`
                // 에서 칸 이름을 고치면 옛 이름에 선 줄이 남는데, 그 줄을 미루거나 제목만
                // 고치는 것까지 막으면 그 줄은 도구 안에서 영영 못 만진다. 옮기는 쓰기는
                // 그대로 엄하다: 갈 칸은 이번에 쓰는 값이라 여기가 서지 않는다.
                let kept = was.is_some_and(|o| o.status == i.status);
                // **여기도 id 로 안 부른다**(moai-1rkl) — 위의 크기 검사만 고치고 두면 같은
                // 쓰기가 한 축에서는 제목을, 다른 축에서는 없는 id 를 댄다. 실제로 `add --from`
                // 에 `#bug,perf` 한 줄을 준 부름이 `<안 남을 id>: 태그에 …` 를 냈고, 같은 계획을
                // 두 번 돌리면 그때마다 다른 id 가 나왔다. **갈아 끼우는 자는 한 자리다**
                // ([`crate::model::point_at_unwritten`]) — 검사마다 따로 적으면 두 벌로 갈린다.
                i.validate_keeping(&self.config, kept).map_err(|said| match was {
                    Some(_) => said,
                    None => crate::model::point_at_unwritten(&i.id, &i.title, said),
                })?;
            }
        }
        issues.sort_by(|a, b| a.id.cmp(&b.id));
        if let Some(dup) = first_duplicate(&issues) {
            return Err(Fail::coded(format!("id 가 두 번 있다 — {dup}"), code::BROKEN));
        }

        // 내용이 그대로면 스냅샷은 건드리지 않는다 (헛 diff 방지).
        // 저널은 따로다 — `moai note` 처럼 스냅샷을 안 바꾸는 기록이 있다.
        let after = render_issues(&issues, &opaque);
        // **정말 쓴 자리에서만 센다.** 안 쓴 자리에서 세면 `moai note` 처럼
        // 스냅샷을 안 건드리는 명령까지 "그대로 두고 썼다" 고 말해, 일어나지
        // 않은 쓰기를 주장한다 — 저널이 거짓말하면 안 되는 것과 같은 까닭이다.
        //
        // **안 쓴 자리에서도 들고 있다는 것은 따로 센다.** `moai note` 처럼
        // 저널만 쓰는 명령은 `report_load_errors` 도 안 지나므로, 여기서 입을
        // 다물면 그 동사만 쓰는 쪽은 파일이 상했다는 것을 영영 모른다(moai-relb).
        // 낱말이 달라야 해서 수를 갈라 둔다 — "그대로 두고 썼다" 는 쓴 자리의 말이다.
        //
        // **호출마다 덮지 않고 쌓는다**([`Tally::after`]). 한 프로세스가 여러 번 쓰는
        // 길(탐색기)에서 마지막 호출만 말하면 앞에서 쓴 것을 "안 건드렸다" 로 지운다.
        let wrote = after != before;
        if wrote {
            // 사람이 정한 권한은 `write_atomic` 이 지킨다. 저널은 제자리에 덧붙이므로 원래 안 풀린다.
            write_atomic(&self.issues_path(), after.as_bytes())?;
            self.note_held(&original, &issues);
        }
        let mut tallies = TALLY.lock().unwrap_or_else(|e| e.into_inner());
        match tallies.iter_mut().find(|(root, _)| *root == self.root) {
            Some((_, t)) => *t = t.after(wrote, opaque.len()),
            None => tallies.push((self.root.clone(), Tally::default().after(wrote, opaque.len()))),
        }
        drop(tallies);
        // **스냅샷을 썼으면 저널 실패는 실패가 아니다**(moai-52z9). 줄은 이미 들어갔는데
        // `Err` 를 내면 사람은 다시 부르고, `add` 는 id 가 다른 같은 이슈를 하나 더 세운다.
        // 순서는 그대로다 — 빠진 일기가 거짓말하는 일기보다 싸다. 고칠 것은 말이다:
        // 세어 두고 `main`·탐색기가 "썼지만 이력은 못 남겼다" 고 말한다.
        //
        // **안 썼으면 그대로 `Err` 다.** `note` 처럼 저널만 적는 쓰기는 저널이 전부라,
        // 거기서 실패하면 아무것도 안 담겼고 다시 부르는 것이 맞다.
        if !entries.is_empty()
            && let Err(e) = self.append_journal(&entries)
        {
            if !wrote {
                return Err(e);
            }
            // **적어 온 말은 저널에만 산다**(`mv -m`·`defer -m`·`promote` 의 메모). 스냅샷이
            // 담겼다고 "다시 부르지 않는다" 만 말하면 그 말은 영영 사라진다 — 어느 이슈의
            // 말이었는지 대어 `moai note` 로 다시 적게 한다.
            let mut worded: Vec<&str> = entries
                .iter()
                .filter(|j| j.text.is_some() || j.note.is_some())
                .map(|j| j.id.as_str())
                .collect();
            worded.dedup();
            let why = if worded.is_empty() {
                e.message
            } else {
                format!("{} (적어 온 말도 안 남았다 — `moai note` 로 다시 적는다: {})", e.message, worded.join(" "))
            };
            MISSED.lock().unwrap_or_else(|e| e.into_inner()).push((self.root.clone(), why));
        }
        // **어디에 썼는지 담아 둔다**(moai-y7go) — 딸린 워크트리에서 친 `moai` 는 루트의 트래커를
        // 고친다([`Repo::find_from`]). 조용히 옮기면 시킨 쪽은 제가 선 자리에 썼다고 믿고, 그
        // 워크트리의 `.moai` 가 왜 안 바뀌는지를 딴 데서 찾는다. **정말 담긴 뒤에** 담는다 — 락·검증에
        // 걸린 쓰기는 위에서 물러났고, 저널만 적는 `note` 는 그 적기가 실패하면 바로 위에서 `Err` 다
        // (저널 앞에서 담던 판은 탐색기가 그 실패를 배너로 삼키고 끝에 "썼다" 를 냈다).
        if (wrote || !entries.is_empty())
            && let Some(from) = &self.moved_from
        {
            let mut moved = MOVED.lock().unwrap_or_else(|e| e.into_inner());
            let pair = (from.clone(), self.dir());
            if !moved.contains(&pair) {
                moved.push(pair);
            }
        }
        Ok(out)
    }

    fn append_journal(&self, entries: &[JournalEntry]) -> R<()> {
        let path = self.journal_path();
        let mut buf = String::new();
        for e in entries {
            buf.push_str(&serde_json::to_string(e).map_err(|e| Fail::new(e.to_string()))?);
            buf.push('\n');
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        f.write_all(buf.as_bytes()).map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        f.sync_all().map_err(|e| Fail::new(format!("{}: {e}", path.display())))
    }

    /// `moai show <id>` 의 이력 전용. **접지 않는다.**
    ///
    /// 이 함수가 `Vec<Issue>` 를 돌려주게 되는 날이 저널을 상태의 원천으로
    /// 삼기 시작한 날이고, 이전 시도가 거기서 복잡해졌다.
    pub fn journal_of(&self, id: &str) -> R<Vec<JournalEntry>> {
        Ok(self.journal_by_id(&BTreeSet::from([id]), |_| true)?.remove(id).unwrap_or_default())
    }

    /// 여러 id 의 이력을 **파일 한 번 읽기로** 가른다(moai-p8qj). 없는 id 는 키가 안 선다.
    ///
    /// [`Repo::journal_of`] 를 id 마다 부르면 파일 전체를 id 수만큼 읽는다 — 목록이 500줄이면
    /// 3MB 를 500번이다. 그 함수는 이것의 한 id 짜리라, 읽는 관대함도 차례(`ts`, 같으면 파일
    /// 차례)도 **한 벌이다** — 둘로 두었던 때는 한쪽 차례만 바꿔도 아무 시험도 안 붉어졌다.
    ///
    /// `line` 은 **풀기 전의 줄**을 고른다 — 고른 줄만 JSON 으로 푼다. `journal_of` 는 다 고른다.
    /// 목록의 `work` 는 `model:` 줄을 들 수 있는 줄만 푼다(`model::may_hold_work`) — 줄 하나 없는
    /// 목록도 3MB 저널을 통째로 풀던 자리다(리뷰 moai-u5bk.3wq).
    ///
    /// **줄마다 따로 읽는다.** 파일을 통째로 UTF-8 로 읽으면 글자 가운데서 끊긴 덧붙이기 한 줄
    /// (디스크가 찼거나 죽었다 — `with_write` 가 흔한 일로 치는 것)이 파일 전체를 못 읽게 해, 이
    /// 저널을 읽는 표면이 다 넘어진다. 모르는/깨진 줄은 건너뛴다 — 저널은 상태를 만들지 않으므로
    /// 여기서 관대해도 답이 틀리지 않는다. 깨진 글자를 `�` 로 바꿔 읽지는 않는다: 그 줄의 `model:`
    /// 이 망가진 값으로 통계에 선다. `\n` 바이트는 UTF-8 글자 안에 안 나오므로 바이트로 갈라도
    /// 글자를 자르지 않는다. 파일이 없으면 빈 손이다 — 저널만 없는 저장소는 고장이 아니다.
    ///
    /// **여전히 접지 않는다** — 돌려주는 것은 줄 그대로지 상태가 아니다.
    pub fn journal_by_id(
        &self,
        want: &BTreeSet<&str>,
        line: impl Fn(&str) -> bool,
    ) -> R<BTreeMap<String, Vec<JournalEntry>>> {
        let path = self.journal_path();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(Fail::new(format!("{}: {e}", path.display()))),
        };
        let bytes = bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(&bytes);
        let mut out: BTreeMap<String, Vec<JournalEntry>> = BTreeMap::new();
        for raw in bytes.split(|b| *b == b'\n') {
            // `str::lines` 와 같은 줄이다 — `\n` 에서 가르고 끝의 `\r` 을 뗀다.
            let Ok(l) = std::str::from_utf8(raw) else { continue };
            let l = l.strip_suffix('\r').unwrap_or(l);
            if l.trim().is_empty() || !line(l) {
                continue;
            }
            let Ok(e) = serde_json::from_str::<JournalEntry>(l) else { continue };
            if want.contains(e.id.as_str()) {
                out.entry(e.id.clone()).or_default().push(e);
            }
        }
        for v in out.values_mut() {
            v.sort_by(|a, b| a.ts.cmp(&b.ts));
        }
        Ok(out)
    }
}

/// 이 프로세스의 `with_write` 들이 못 읽는 줄에 대해 본 것.
///
/// `store` 는 터미널을 모르므로 찍지 않는다 — 세어 두기만 하고 `main` 이
/// 말한다. `cmd::had_partial` 과 같은 모양이고, 같은 까닭이다: 쓰기 명령마다
/// 파일을 한 번 더 읽어 보고하게 하면 그 읽기가 락 밖이라 락 안에서 본 것과
/// 다를 수 있다.
///
/// **명령 하나가 아니라 프로세스 하나의 것이다.** `moai tui` 는 여러 번 쓰고 나서
/// `main` 을 지난다. 마지막 호출만 남기면 앞에서 쓴 뒤 끝에 `note` 하나를 적은
/// 세션이 "이번 명령은 그 파일을 안 건드렸다" 고 나오고, 반대 순서면 안 쓴
/// 쓰기의 수가 쓴 자리의 말로 선다(moai-2v3w).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// 한 번이라도 스냅샷을 썼는가.
    pub wrote: bool,
    /// 쓴 호출이 **그대로 들고 넘어간** 줄의 수 — 쓴 호출 가운데 가장 많던 것.
    /// 더하지 않는다: 두 번 쓰면 같은 줄을 두 번 들고 간 것이다.
    pub carried: usize,
    /// 마지막 호출이 파일에서 본 줄의 수. 쓴 것과 무관한 파일의 지금 모습이다.
    pub seen: usize,
}

impl Tally {
    /// 호출 하나를 더 접는다. **순수하다** — 전역을 건드리는 것은 `with_write` 뿐이다.
    pub fn after(self, wrote: bool, unreadable: usize) -> Tally {
        Tally {
            wrote: self.wrote || wrote,
            carried: if wrote { self.carried.max(unreadable) } else { self.carried },
            seen: unreadable,
        }
    }
}

/// **저장소마다 따로 센다.** 탐색기는 층에서 여러 프로젝트에 쓴다 — 하나로 접으면
/// A 에서 들고 간 줄을 B 의 말로 내거나, A 가 상한 것을 B 의 깨끗한 쓰기가 덮어
/// 입을 다문다. 처음 쓴 차례대로 둔다.
static TALLY: std::sync::Mutex<Vec<(PathBuf, Tally)>> = std::sync::Mutex::new(Vec::new());

pub fn unreadable_tallies() -> Vec<(PathBuf, Tally)> {
    TALLY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 스냅샷은 썼는데 저널에 못 적은 쓰기 — `(저장소 뿌리, 까닭)`, 일어난 차례대로.
///
/// [`TALLY`] 와 같은 까닭으로 `store` 는 세어 두기만 한다. **비우지 않는다** — 탐색기가
/// 제 쓰기의 것을 알림으로 말하고 나서도, 나올 때 `main` 이 한 번 더 stderr 로 남긴다.
/// 대체 화면 안의 알림은 닫으면 사라지기 때문이다.
static MISSED: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

pub fn journal_misses() -> Vec<(PathBuf, String)> {
    MISSED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 파일이 그때 그것인지 가늠하는 표식. 고친 때만 보면 놓친다 — rename 으로
/// 갈아끼우는 쓰기는 같은 초에 떨어질 수 있어 길이도 함께 본다. 파일이 없으면 `None`.
pub type Stamp = Option<(std::time::SystemTime, u64)>;

pub fn stamp(path: &Path) -> Stamp {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// 스냅샷 하나를 읽는다. 파일이 없으면 `None` — **없는 것과 빈 것을 가른다.**
///
/// 제 저장소에서는 둘이 같지만(`init` 직후), 다른 워크트리에서는 다르다: 파일이
/// 없는 워크트리는 moai 를 들이기 전에 갈라진 브랜치라 겹칠 것이 없고, 그것을
/// 빈 스냅샷으로 세면 "읽었는데 비었다" 로 보인다(`worktree::gather`).
///
/// **락을 잡지 않는다** — [`Repo::read`] 와 같은 까닭이다. 남의 워크트리를 읽는
/// 길도 여기를 지나므로, 거기서도 락을 안 잡는다: 남의 쓰기를 기다리게 할 까닭이
/// 없고, `rename` 이 찢어진 파일을 못 보게 한다.
pub fn read_snapshot(path: &Path) -> R<Option<Load>> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(parse_issues(&s))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Fail::new(format!("{}: {e}", path.display()))),
    }
}

pub fn parse_issues(src: &str) -> Load {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut load = Load::default();
    for (i, line) in src.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Issue>(line) {
            Ok(issue) => load.issues.push(issue),
            Err(e) => load.errors.push(LoadError {
                line: i + 1,
                message: e.to_string(),
                text: line.to_string(),
                // **둘째 파서를 만들지 않는다.** 같은 `serde_json` 을 한 겹
                // 아래로 부를 뿐이라 본체와 어긋날 자리가 없다 — 정규식으로
                // 긁었으면 그 순간 파서가 둘이 되고, 둘은 언젠가 갈라진다.
                id: serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| v.get("id")?.as_str().map(str::to_string)),
            }),
        }
    }
    load.issues.sort_by(|a, b| a.id.cmp(&b.id));
    load
}

/// 정렬은 `id` 바이트 오름차순이다. `-`(0x2D) < `.`(0x2E) < 숫자 < 소문자 라서
/// 자식이 부모 바로 밑에 붙고, 랜덤 id 가 삽입 위치를 파일 전체에 흩뿌려
/// git 충돌 확률을 떨어뜨린다 (파일 끝 append 는 두 브랜치가 **항상** 부딪친다).
///
/// **못 읽은 줄은 뒤에 그대로 붙는다.** 제자리에 둘 수가 없다 — 차례를 정하는
/// 것이 `id` 인데 그 줄은 `id` 를 못 읽어서 못 읽은 줄이다. 첫 쓰기 한 번만
/// 자리가 밀리고 그 뒤로는 움직이지 않는다.
fn render_issues(issues: &[Issue], opaque: &[&str]) -> String {
    let mut out = String::new();
    for i in issues {
        out.push_str(&serde_json::to_string(i).expect("Issue 는 언제나 직렬화된다"));
        out.push('\n');
    }
    for line in opaque {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn first_duplicate(sorted: &[Issue]) -> Option<&str> {
    sorted.windows(2).find(|w| w[0].id == w[1].id).map(|w| w[0].id.as_str())
}

/// 이미 쓰이고 있는 id 전부 — 읽은 줄의 것과 **못 읽는 줄의 것까지.**
///
/// 둘을 한 함수로 합쳐 두는 까닭은 한쪽만 넘기는 것이 불가능해야 하기
/// 때문이다. 갈라 두면 부르는 쪽이 언젠가 하나를 잊고, 잊은 그날은 아무
/// 증상도 없다.
pub fn taken_ids(issues: &[Issue], reserved: &BTreeSet<String>) -> BTreeSet<String> {
    issues.iter().map(|i| i.id.clone()).chain(reserved.iter().cloned()).collect()
}

/// 하나짜리를 만들 때의 새 id. **[`Repo::with_write`] 안에서 부른다** — 락 안에서
/// 읽은 `issues`·`reserved` 로 재야 두 프로세스가 같은 id 를 뽑지 않는다.
///
/// `parent` 가 있으면 그 밑의 자식 id 다. 부모가 **있는지는 보지 않는다** — 없는
/// 부모를 어떻게 말할지는 부르는 쪽의 말투라(CLI 는 `NOT_FOUND`) 여기서 정하지 않는다.
///
/// `moai add` 와 탐색기의 `n` 이 이 한 길을 쓴다. 갈라지면 같은 제목이 어디서
/// 만들었느냐에 따라 다른 모양의 id 를 얻는다. `add --from` 은 초안 여럿을 한
/// 번에 뽑느라 제 `taken` 을 들고 늘려 가므로 이 길이 아니다.
pub fn new_id(
    issues: &[Issue],
    cfg: &Config,
    reserved: &BTreeSet<String>,
    parent: Option<&str>,
    title: &str,
) -> String {
    let taken = taken_ids(issues, reserved);
    let seed = crate::id::seed(title);
    match parent {
        Some(p) => crate::id::generate_child(p, &taken, &seed),
        None => crate::id::generate(&cfg.prefix, &taken, &seed),
    }
}

/// 새 줄 하나를 들인다 — 정규화하고, 재고, 밀어 넣고, 저널의 `create` 한 줄을 낸다.
/// **만드는 쓰기는 다 이 길을 지난다**(`add`·`add --from`·`idea promote`·탐색기의 `n`).
/// 갈라지면 한쪽만 태그를 접거나 한쪽만 저널을 빼먹고, 그날은 아무 증상도 없다.
///
/// **밀어 넣기 전에 잰다.** `with_write` 가 뒤에서 바뀐 줄을 한 번 더 재지만, 여러
/// 줄을 만드는 쪽은 첫 거절에서 멈춰야 뒤의 초안이 헛 id 를 안 뽑는다.
///
/// 저널의 시각은 그 줄의 `created_at` 이다 — 둘을 따로 받으면 어긋날 수 있다.
/// 돌려주는 줄은 밀어 넣은 것의 사본이다(출력을 짓는 쪽이 쓴다).
pub fn admit(issues: &mut Vec<Issue>, cfg: &Config, mut issue: Issue, by: &Actor) -> R<(JournalEntry, Issue)> {
    // 첫 칸 밖에서 나는 줄(`add -s`)은 만든 때가 곧 시작이다 — 칸을 옮기는 쓰기와 같은 뜻으로
    // 적는다(`Issue::arrive`, moai-38mh).
    issue.arrive(cfg);
    issue.normalize();
    // **여기서 거절하면 이 id 를 안 댄다**(moai-1rkl). 거절은 쓰기를 통째로 물리므로 방금 뽑은
    // id 는 어디에도 안 남는데, [`Issue::validate_fields`] 의 말은 일곱 자리가 모두 `<id>: ` 로
    // 시작한다 — `add --from` 에 `#bug,perf` 한 줄을 준 부름이 없는 id 를 대고, 같은 계획을 두
    // 번 돌리면 그때마다 다른 id 가 나왔다. 크기 검사만 고치고 두면 같은 쓰기가 축마다 다른
    // 말을 하므로, **만드는 쓰기가 다 지나는 이 자리**에서 함께 건다.
    if let Err(said) = issue.validate(cfg) {
        return Err(crate::model::point_at_unwritten(&issue.id, &issue.title, said).into());
    }
    let entry = JournalEntry::create(&issue.id, &issue.title, &issue.created_at, by);
    issues.push(issue.clone());
    Ok((entry, issue))
}

/// temp 에 쓰고 `rename` 으로 갈아끼운다. 독자는 옛 파일 아니면 새 파일만 본다.
///
/// **옛 파일의 권한을 바꿔 끼우기 전에** 임시 파일에 입힌다. 새로 만든 임시 파일은 umask
/// 권한이라, 그대로 `rename` 하면 사람이 `chmod 600` 해 둔 파일이 쓰기 한 번에 남도 읽는
/// 파일로 바뀐다. 바꾼 뒤에 입히면 그 사이 잠깐 열려 있으므로 앞에서 한다. 파일이 없던
/// 처음 쓰기만 umask 를 따른다.
///
/// **권한을 고르는 인자는 두지 않는다.** 한때 권한을 넘기는 `write_atomic_as` 가 곁에 따로 있어
/// 사용자 설정만 그것을 불렀고, `issues.jsonl` 은 권한 없는 쪽을 불러 풀렸다 (moai-c1s3).
/// 지키지 않아야 할 쓰기가 없으니 잊을 자리도 없앤다. [`write_atomic_in`] 이 고르는 것은 **임시
/// 자리뿐**이고 권한은 둘이 한 몸통에서 지킨다 — 임시 자리를 잘못 고르면 찌꺼기가 남지만, 권한을
/// 잘못 고르면 남이 읽는다. 둘을 같은 무게로 읽고 `write_atomic_as` 를 되살리지 않는다.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> R<()> {
    let dir = path.parent().ok_or_else(|| Fail::new("경로에 디렉터리가 없다"))?;
    write_atomic_in(path, bytes, dir)
}

/// [`write_atomic`] 이되 **임시 파일을 `tmp_dir` 에 둔다**(moai-3akx). 쓰다 죽으면 임시 파일이
/// 그 자리에 남으므로, 저장소 뿌리의 파일을 쓰는 `init` 은 이미 무시되는 `.moai/*.tmp.*` 자리를
/// 준다 — 옆자리에 두면 `AGENTS.md.tmp.<pid>` 가 뿌리에 남아 `git add -A` 에 딸려 온다.
/// `rename` 은 파일시스템을 못 건너므로 `tmp_dir` 이 다른 파일시스템이면 `Err` 다 — 그때 옆자리로
/// 물러서는 것은 고르는 쪽이 한다(`cmd::init::plant`). 실패하면 **임시 파일을 남기지 않고** 대상은
/// 한 글자도 안 바뀐다.
pub(crate) fn write_atomic_in(path: &Path, bytes: &[u8], tmp_dir: &Path) -> R<()> {
    let perms = std::fs::metadata(path).ok().map(|m| m.permissions());
    let dir = path.parent().ok_or_else(|| Fail::new("경로에 디렉터리가 없다"))?;
    let tmp = tmp_dir.join(format!(
        "{}.tmp.{}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("out"),
        std::process::id()
    ));
    // **어디서 실패하든 임시 파일을 치운다.** `rename` 에서만 치우던 때는 디스크가 찬(ENOSPC)
    // 쓰기가 죽지 않고도 `<파일>.tmp.<pid>` 를 남겼다 — moai-3akx 가 막으려던 찌꺼기다.
    let fail = |at: &Path, e: std::io::Error| {
        let _ = std::fs::remove_file(&tmp);
        Fail::new(format!("{}: {e}", at.display()))
    };
    let filled = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        if let Some(p) = perms {
            f.set_permissions(p)?;
        }
        f.write_all(bytes)?;
        f.sync_all()
    })();
    filled.map_err(|e| fail(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| fail(path, e))?;
    // rename 자체는 원자적이지만 디렉터리 엔트리는 아직 디스크에 없을 수 있다. 임시 자리가 다른
    // 디렉터리면 그쪽에서 빠진 엔트리도 적는다 — 안 적으면 전원이 나간 뒤 임시 파일이 되살아난다.
    #[cfg(unix)]
    for d in std::iter::once(dir).chain((tmp_dir != dir).then_some(tmp_dir)) {
        if let Ok(d) = std::fs::File::open(d) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// 파일이 든 디렉터리. 디렉터리 조각이 없는 상대 철자(`config.toml`)면 `.` 이다.
pub(crate) fn dir_of(path: &Path) -> &Path {
    path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

/// 그 파일의 락 자리 — 곁의 `<이름>.lock`.
///
/// **락 자리를 세는 자를 하나로 둔다**(리뷰) — 같은 디렉터리에 사는 두 파일이 저마다 락 이름을 세면,
/// 자리 규칙이 바뀌는 날 한쪽만 따라가 둘이 서로를 안 막는다. [`crate::user_config::update`] 가 두 철자의
/// 락이 갈렸을 때를 재 뒀다(스무 개 중 열 개가 사라졌다) — 이 도구가 못 견딘다는 그 조용한 손실이다.
pub(crate) fn lock_beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(std::ffi::OsString::from).unwrap_or_else(|| "config".into());
    name.push(".lock");
    dir_of(path).join(name)
}

/// `flock(2)`. 프로세스가 죽으면 커널이 놓아 준다.
///
/// 직접 만든 락 파일(`O_EXCL`)을 쓰지 않는 이유가 이것이다 — 죽으면 찌꺼기가
/// 남아 **사람이 손으로 지워야 한다.** 사람 손이 덜 가게 하려고 만드는 도구에
/// "락 파일 좀 지워주세요" 를 넣을 수는 없다.
pub(crate) struct Lock(std::fs::File);

impl Lock {
    pub(crate) fn acquire(path: &Path) -> R<Lock> {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        let start = Instant::now();
        loop {
            match f.try_lock_exclusive() {
                Ok(()) => return Ok(Lock(f)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                    if start.elapsed() >= LOCK_TIMEOUT {
                        return Err(Fail::coded(
                            format!(
                                "{} 초 동안 다른 moai 가 쓰고 있어 물러난다. 아무것도 바뀌지 않았다",
                                LOCK_TIMEOUT.as_secs()
                            ),
                            code::LOCKED,
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(Fail::new(format!("{}: {e}", path.display()))),
            }
        }
    }

    /// 이 락이 쥔 파일이 `path` 와 **한 파일**인가 — 철자가 아니라 파일로 견준다(unix 의 장치·아이노드). `path` 가
    /// 없으면 거짓이다. unix 밖에서는 견줄 길이 없어 `None` 이다.
    ///
    /// 같은 프로세스가 한 파일에 `flock` 을 두 번 잡으면 둘째가 첫째를 기다려 `locked` 로 물러난다. 위 디렉터리나 락
    /// 파일 자체가 링크이거나, 하드 링크이거나, 대소문자를 안 가르는 볼륨이면 철자가 달라도 한 파일이다 — 그것을
    /// 가르는 자리다(`user_config::update`). 쥔 쪽은 **연 파일을 그대로** 재므로, 그 사이 그 이름이 다른 파일로
    /// 갈아끼워져도 쥔 것을 헛짚지 않는다.
    pub(crate) fn holds(&self, path: &Path) -> Option<bool> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some(match (self.0.metadata(), std::fs::metadata(path)) {
                (Ok(held), Ok(other)) => (held.dev(), held.ino()) == (other.dev(), other.ino()),
                _ => false,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            None
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use crate::model::{Kind, Status};

    /// `<자리>/a/b/c` 를 만든다. 위로 찾기를 재는 시험들이 함께 쓴다.
    fn tree(name: &str) -> Scratch {
        let s = Scratch::new(name);
        std::fs::create_dir_all(s.join("a/b/c")).unwrap();
        s
    }

    fn moai_at(dir: &Path) {
        std::fs::create_dir_all(dir.join(".moai")).unwrap();
    }

    /// 찾은 뿌리만 — 못 찾았으면 `None`. 시험 대부분은 [`climb`] 의 둘째 값을 안 본다.
    fn root_of(found: &Option<(PathBuf, bool)>) -> Option<&Path> {
        found.as_ref().map(|(root, _)| root.as_path())
    }

    /// **체크아웃을 두고 올라왔는가** — 알릴지를 가르는 값이다.
    fn climbed_out(found: &Option<(PathBuf, bool)>) -> bool {
        matches!(found, Some((_, true)))
    }

    /// **처음 만난 `.moai` 가 이기고, 위로 끝까지 간다**(2026-09-20 사용자가 짚었다). 모노레포는
    /// `.git` 하나에 하위 프로젝트마다 `.moai` 가 설 수 있다 — "저장소마다 트래커 하나" 로 읽는
    /// 자를 두면 그 사람들이 제 트래커를 잃는다.
    #[test]
    fn each_project_in_a_monorepo_keeps_its_own_tracker() {
        let s = tree("look-monorepo");
        std::fs::create_dir_all(s.join(".git")).unwrap();
        moai_at(s.path());
        std::fs::create_dir_all(s.join("projA/sub")).unwrap();
        std::fs::create_dir_all(s.join("projB/sub")).unwrap();
        moai_at(&s.join("projA"));

        let a = climb(&s.join("projA/sub"));
        assert_eq!(root_of(&a), Some(s.join("projA").as_path()), "제 `.moai` 를 두고 모노레포 꼭대기로 갔다");
        // 꼭대기의 `.git` 이 `projA` 를 품으니 그 안에서 올라간 것이다 — 알릴 일이 아니다.
        assert!(!climbed_out(&a), "한 체크아웃 안에서 올라간 것을 밖이라고 했다");
        let b = climb(&s.join("projB/sub"));
        assert_eq!(root_of(&b), Some(s.path()), "`.moai` 없는 하위가 꼭대기의 것을 못 잡았다");
        assert!(!climbed_out(&b), "한 체크아웃 안에서 올라간 것을 밖이라고 했다");
    }

    /// **겹쳐 둔 저장소에서도 바깥 프로젝트의 트래커를 쓴다.** git 꼭대기를 천장으로 두었다가
    /// 걷어낸 까닭이다 — `P/vendor` 는 vendoring·서브모듈의 흔한 모양이고, 거기서 끊으면
    /// moai-d3sy 가 고친 자리(사람을 P 에서 읽는다)가 도로 무너진다.
    #[test]
    fn a_repo_nested_in_a_project_still_writes_to_that_project() {
        let s = tree("look-vendor");
        moai_at(s.path());
        let vendor = s.join("vendor");
        std::fs::create_dir_all(vendor.join(".git")).unwrap();
        let found = climb(&vendor);
        assert_eq!(root_of(&found), Some(s.path()));
        // 겹쳐 둔 저장소를 **두고** 올라갔다 — 어느 트래커에 드는지는 보여야 한다.
        assert!(climbed_out(&found), "체크아웃을 두고 올라간 것을 안 알린다");
    }

    /// **알리는 자는 오는 길에 지난 `.git` 하나다.** 세 자리를 가른다.
    ///
    /// - `.moai` 없는 클론 **안**에서 그 위의 트래커를 잡는 자리 — 알린다. 알릴 값이 있는 꼴이
    ///   바로 이것이고, "위로 오는 길 어딘가에 `.git` 이 있었나" 로 재던 판은 여기를 조용히 넘겼다
    /// - 트래커가 선 자리가 곧 체크아웃 꼭대기인 자리 — 안 알린다. 제 저장소의 하위 디렉터리에 선
    ///   사람에게 매번 한 줄을 내면 그건 소음이고, 소음은 아무도 안 읽는다
    /// - **git 이 아예 없는 자리 — 안 알린다.** 거기서는 "내 프로젝트 위" 와 "남의 트래커 위" 를
    ///   가를 값이 없다. 한때 알리던 자리인데, 그러면 git 을 안 쓰는 프로젝트의 하위 디렉터리에서
    ///   친 **모든** 명령에 한 줄이 서서 도리어 못 읽는 줄이 됐다(리뷰 moai-0ftu.h80 3번)
    #[test]
    fn climbing_out_of_a_checkout_is_told_of_but_climbing_inside_one_is_not() {
        let s = tree("look-climb-leak");
        moai_at(s.path());
        std::fs::create_dir_all(s.join("a/.git")).unwrap();
        let leak = climb(&s.join("a/b"));
        assert_eq!(root_of(&leak), Some(s.path()));
        assert!(climbed_out(&leak), "`.moai` 없는 클론을 두고 올라간 것을 조용히 넘겼다");

        let t = tree("look-climb-git");
        std::fs::create_dir_all(t.join(".git")).unwrap();
        moai_at(t.path());
        let inside = climb(&t.join("a/b"));
        assert_eq!(root_of(&inside), Some(t.path()));
        assert!(!climbed_out(&inside), "체크아웃 안에서 올라간 것을 알렸다 — 소음이다");

        let u = tree("look-climb-nogit");
        moai_at(u.path());
        let bare = climb(&u.join("a/b"));
        assert_eq!(root_of(&bare), Some(u.path()));
        assert!(!climbed_out(&bare), "git 이 없는 자리에서 매번 알린다 — 소음이다");
    }

    /// **등록한 자리가 딸린 워크트리면 루트의 트래커를 연다**(moai-y7go, 리뷰 moai-71ht.jlh
    /// 사용자 결정) — 탐색기의 프로젝트 층이 이 문으로 열므로, 안 옮기면 같은 자리를 CLI 로 칠 때와
    /// 다른 파일이 조용히 바뀐다. 옮길 곳에 트래커가 없으면 그 자리 그대로다.
    ///
    /// **푼 자리에 세운다**([`Scratch::real`], 리뷰 moai-71ht 셋째 판) — `repo.root` 는 git 이 적어 둔
    /// 경로를 푼 것이라(`worktree::main_root`), 임시 자리가 링크 뒤에 있는 기계(macOS 의 `/var`)에서는
    /// 안 푼 철자와 견주는 이 시험이 그 기계에서만 붉어진다.
    #[test]
    fn a_registered_worktree_opens_the_root_tracker() {
        let dir = Scratch::real("open-wt");
        let main = dir.join("main");
        std::fs::create_dir_all(&main).unwrap();
        // git 은 `git::isolated` 로만 띄운다 — 돌리는 사람의 설정이 새면 임시 저장소가 멈춘다.
        // 걷기와 격리, 실패의 말은 `git::tests::run_git` 하나가 안다(worktree.rs 도 그것을 부른다).
        let git = |at: &std::path::Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        std::fs::create_dir_all(main.join(".moai")).unwrap();
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        std::fs::write(main.join(".moai/issues.jsonl"), "").unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        git(&main, &["worktree", "add", "-q", "side", "-b", "side"]);
        let side = main.join("side");

        let Opened::Repo(repo) = Repo::open(&side).unwrap() else { panic!("안 열렸다") };
        assert_eq!(repo.root, main, "등록한 워크트리를 그 자리에서 열었다");
        assert_eq!(repo.here(), side, "어디서 열었는지를 잃었다");

        // 옮길 곳에 트래커가 없으면 그대로다 — 이 가지에서 처음 `init` 한 워크트리.
        let alone = dir.join("alone");
        std::fs::create_dir_all(&alone).unwrap();
        git(&alone, &["init", "-q"]);
        git(&alone, &["commit", "-q", "--allow-empty", "-m", "처음"]);
        git(&alone, &["worktree", "add", "-q", "feat", "-b", "feat"]);
        let feat = alone.join("feat");
        std::fs::create_dir_all(feat.join(".moai")).unwrap();
        std::fs::write(feat.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        let Opened::Repo(repo) = Repo::open(&feat).unwrap() else { panic!("안 열렸다") };
        assert_eq!(repo.root, feat, "옮길 곳이 없는데 옮겼다");
        assert_eq!(repo.here(), feat, "안 옮겼는데 옮겼다고 적었다");
    }

    const T: &str = "2026-09-11T04:12:03Z";

    fn issue(id: &str) -> Issue {
        Issue::new(id.into(), format!("{id} 의 제목"), Kind::Issue, Status::new("todo"), T)
    }

    /// `.moai` 한 벌이 든 임시 저장소. **돌려받은 것을 묶어 둔다** — 흘리면 그 줄
    /// 끝에서 디렉터리가 지워진다.
    fn scratch(name: &str) -> Scratch {
        let dir = Scratch::new(&format!("store-{name}"));
        std::fs::create_dir_all(dir.join(".moai")).unwrap();
        std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        dir
    }

    /// 임시 저장소와 **그 자리를 쥔 것**. 둘째를 놓으면 디렉터리가 사라지므로
    /// 부르는 쪽이 시험이 끝날 때까지 들고 있어야 한다.
    fn repo(name: &str) -> (Repo, Scratch) {
        let root = scratch(name);
        let config = Config::load(&root).unwrap();
        (Repo::at(root.to_path_buf(), config), root)
    }

    #[test]
    fn writes_and_reads_back() {
        let (r, _d) = repo("rw");
        r.with_write(|issues, _, _| {
            issues.push(issue("argos-4aex"));
            Ok((vec![JournalEntry::create("argos-4aex", "t", T, &crate::model::someone("raven"))], ()))
        })
        .unwrap();
        let load = r.read().unwrap();
        assert_eq!(load.issues.len(), 1);
        assert!(load.errors.is_empty());
        assert_eq!(r.journal_of("argos-4aex").unwrap().len(), 1);
    }

    /// 임의 순서로 넣어도 파일은 언제나 id 순이고, 자식이 부모 밑에 붙는다.
    #[test]
    fn output_is_sorted_and_deterministic() {
        let (r, d) = repo("sorted");
        r.with_write(|issues, _, _| {
            for id in ["argos-4aey", "argos-4aex.ae3", "argos-0001", "argos-4aex"] {
                issues.push(issue(id));
            }
            Ok((vec![], ()))
        })
        .unwrap();
        let src = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let ids: Vec<&str> = src
            .lines()
            .map(|l| l.split('"').nth(3).unwrap())
            .collect();
        assert_eq!(ids, ["argos-0001", "argos-4aex", "argos-4aex.ae3", "argos-4aey"]);
    }

    /// 바뀐 게 없으면 파일을 건드리지 않는다 — 헛 diff 를 만들지 않는다.
    #[test]
    fn unchanged_write_leaves_the_file_alone() {
        let (r, d) = repo("idem");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        let path = d.join(".moai/issues.jsonl");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        r.with_write(|_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    /// 스냅샷을 안 바꾸는 기록(`note`)도 저널에는 남아야 한다.
    #[test]
    fn journal_only_writes_still_land() {
        let (r, _d) = repo("note");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        r.with_write(|_, _, _| Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &crate::model::someone("raven"))], ())))
            .unwrap();
        let j = r.journal_of("argos-4aex").unwrap();
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].text.as_deref(), Some("발견"));
    }

    /// **저널만 못 적은 쓰기는 담긴 것으로 끝나고, 저널이 전부인 쓰기는 실패다**(moai-52z9).
    /// 앞의 것을 `Err` 로 내면 다시 부른 `add` 가 같은 이슈를 하나 더 세운다.
    #[cfg(unix)]
    #[test]
    fn a_write_whose_journal_fails_still_lands_but_a_journal_only_one_does_not() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("journalfail");
        let journal = d.join(".moai/journal.jsonl");
        std::fs::write(&journal, "").unwrap();
        std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new().append(true).open(&journal).is_ok() {
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }

        let by = crate::model::someone("raven");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![JournalEntry::create("argos-4aex", "t", T, &by)], ()))
        })
        .expect("스냅샷을 썼는데 실패로 냈다 — 다시 부르면 둘 선다");
        assert_eq!(r.read().unwrap().issues.len(), 1);
        assert!(journal_misses().iter().any(|(root, _)| root == d.path()), "못 남긴 것을 안 셌다");

        let e = r.with_write(|_, _, _| Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &by)], ())));
        assert!(e.is_err(), "저널만 적는 쓰기가 아무것도 안 담았는데 성공으로 끝났다");
    }

    /// **여러 번 쓴 프로세스는 한 번이라도 쓴 것을 잊지 않는다**(moai-2v3w). 탐색기가
    /// 이슈를 옮긴 뒤 `note` 하나로 끝나면, 마지막 호출만 보던 때는 나오면서 "안 건드렸다"
    /// 고 했다. 전역은 병렬 시험끼리 섞이므로 접는 함수를 직접 본다.
    #[test]
    fn the_tally_keeps_a_write_that_came_before_a_quiet_call() {
        let none = Tally::default();

        let wrote_then_noted = none.after(true, 1).after(false, 1);
        assert!(wrote_then_noted.wrote);
        assert_eq!(wrote_then_noted.carried, 1, "앞에서 들고 간 줄을 잊었다");

        // 반대 순서도 같은 답이다 — 안 쓴 호출의 수가 쓴 자리의 말로 서지 않는다.
        assert_eq!(none.after(false, 1).after(true, 1), wrote_then_noted);

        // 안 쓴 호출만이면 쓴 적이 없다.
        let quiet = none.after(false, 2).after(false, 2);
        assert_eq!(quiet, Tally { wrote: false, carried: 0, seen: 2 });

        // 깨끗할 때 쓰고 그 뒤에 줄이 상하면 — 쓰긴 했지만 들고 간 것은 없다.
        assert_eq!(none.after(true, 0).after(false, 1), Tally { wrote: true, carried: 0, seen: 1 });

        // 두 번 쓰면 같은 줄을 두 번 든 것이지 두 배가 아니다.
        assert_eq!(none.after(true, 3).after(true, 3).carried, 3);
    }

    /// 41번 줄이 깨져도 나머지가 읽히고, 어느 줄인지 보고된다.
    #[test]
    fn a_broken_line_does_not_sink_the_file() {
        let mut src = String::new();
        for n in 0..5 {
            src.push_str(&serde_json::to_string(&issue(&format!("argos-000{n}"))).unwrap());
            src.push('\n');
        }
        let mut lines: Vec<&str> = src.lines().collect();
        lines[2] = "{\"id\": 깨짐";
        let load = parse_issues(&lines.join("\n"));
        assert_eq!(load.issues.len(), 4);
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].line, 3);
    }

    /// 마지막 줄이 잘린 파일 — 프로세스가 죽었을 때 나올 수 있는 모양.
    #[test]
    fn a_truncated_tail_is_reported_not_fatal() {
        let good = serde_json::to_string(&issue("argos-0001")).unwrap();
        let load = parse_issues(&format!("{good}\n{{\"id\":\"argos-00"));
        assert_eq!(load.issues.len(), 1);
        assert_eq!(load.errors.len(), 1);
    }

    #[test]
    fn bom_and_blank_lines_are_tolerated() {
        let good = serde_json::to_string(&issue("argos-0001")).unwrap();
        let load = parse_issues(&format!("\u{feff}{good}\n\n"));
        assert_eq!(load.issues.len(), 1);
        assert!(load.errors.is_empty());
        assert!(parse_issues("").issues.is_empty());
    }

    /// 깨진 줄이 있어도 **쓴다.** 그 줄은 글자 하나 안 바뀌고 남는다.
    ///
    /// 한때 여기서 통째로 거절했다 — "쓰면 그 줄이 사라진다" 는 걱정이었다.
    /// 거절 대신 들고 있으면 걱정도 없고 막히지도 않는다. 막는 쪽이 비싼
    /// 이유는 뒷 단계 바이너리가 쓴 줄 하나가 앞 단계 사람의 모든 쓰기를
    /// 막아, 되돌릴 방법이 도구 밖에만 남기 때문이다.
    #[test]
    fn a_broken_line_is_carried_not_a_wall() {
        let (r, d) = repo("broken");
        let path = d.join(".moai/issues.jsonl");
        std::fs::write(&path, "{깨짐\n").unwrap();
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .expect("깨진 줄 하나가 쓰기를 막았다");

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("{깨짐"), "모르는 줄을 잃었다 — {after}");
        assert!(after.contains("argos-4aex"), "쓰겠다고 해 놓고 안 썼다 — {after}");

        // 두 번째 쓰기에서 줄이 또 움직이지 않는다 (멱등).
        let once = std::fs::read_to_string(&path).unwrap();
        r.with_write(|_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    }

    #[test]
    fn refuses_duplicate_ids() {
        let (r, _d) = repo("dup");
        let e = r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap_err().message;
        assert!(e.contains("두 번"), "{e}");
    }

    /// 남의 낡은 줄 하나가 모든 쓰기를 막지 않는다.
    ///
    /// config 에서 칸 이름을 고치면 그 칸에 있던 이슈는 더 이상 유효하지
    /// 않다. 그때도 새 이슈는 만들 수 있어야 한다 — 못 만들면 되돌릴 방법이
    /// 도구 밖에만 남는다.
    #[test]
    fn a_stale_row_does_not_block_unrelated_writes() {
        let (r, d) = repo("stale_row");
        let mut old = issue("argos-0001");
        old.status = Status::new("옛날칸");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&old).unwrap()),
        )
        .unwrap();

        r.with_write(|issues, _, _| {
            issues.push(issue("argos-0002"));
            Ok((vec![], ()))
        })
        .expect("낡은 줄 때문에 새 이슈를 못 넣었다");

        let load = r.read().unwrap();
        assert_eq!(load.issues.len(), 2);
        // 그리고 낡은 줄은 지워지지 않고 그대로 남는다
        assert_eq!(load.get("argos-0001").unwrap().status.as_str(), "옛날칸");
    }

    /// 그 줄을 직접 건드려도 **안 바꾼 칸은 다시 안 묻는다** (moai-hym7, 사람이 정했다).
    /// 한때 여기서 거절했는데, 그러면 `config` 에서 칸 이름을 고친 순간 옛 이름에 선 줄은
    /// 제목 하나 못 고치고 미루지도 못해 도구 안에서 영영 못 만진다. 엄함은 이번에 쓰는
    /// 값에 대한 것이다 — **칸을 옮기는** 쓰기는 그대로 거절한다(아래).
    #[test]
    fn touching_a_stale_row_keeps_its_column_but_cannot_move_it_to_a_missing_one() {
        let (r, d) = repo("stale_touch");
        let mut old = issue("argos-0001");
        old.status = Status::new("옛날칸");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&old).unwrap()),
        )
        .unwrap();

        r.with_write(|issues, _, _| {
            issues[0].title = "고친 제목".into();
            Ok((vec![], ()))
        })
        .expect("옛 칸에 선 줄의 제목을 못 고쳤다");
        assert_eq!(r.read().unwrap().get("argos-0001").unwrap().title, "고친 제목");
        assert_eq!(r.read().unwrap().get("argos-0001").unwrap().status.as_str(), "옛날칸");

        let e = r
            .with_write(|issues, _, _| {
                issues[0].status = Status::new("또 없는 칸");
                Ok((vec![], ()))
            })
            .unwrap_err()
            .message;
        assert!(e.contains("라는 칸이 없다"), "{e}");
    }

    /// 쓰기 도중 실패하면 원본이 그대로다 — 반쯤 쓰인 파일이 남지 않는다.
    #[test]
    fn a_rejected_write_leaves_the_file_untouched() {
        let (r, d) = repo("rollback");
        r.with_write(|i, _, _| {
            i.push(issue("argos-4aex"));
            Ok((vec![], ()))
        })
        .unwrap();
        let before = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let _ = r.with_write(|i, _, _| {
            i.push(issue("argos-4aey"));
            i[0].status = Status::new("없는칸");
            Ok((vec![], ()))
        });
        assert_eq!(std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap(), before);
    }
    /// 못 읽는 줄도 **id 는 내놓는다.** 줄을 `Issue` 로 못 읽는 것과 그 안의
    /// `id` 를 못 읽는 것은 다른 일이다 — 한 단 낮게 읽으면 나온다.
    ///
    /// **둘째 파서를 만들지 않는다.** `serde_json` 한 겹 아래로 내려갈 뿐이라
    /// 본체와 어긋날 자리가 없다. 정규식으로 긁었으면 그 순간 파서가 둘이 된다.
    #[test]
    fn an_unreadable_line_still_yields_its_id() {
        let load = parse_issues(
            "{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n",
        );
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].id.as_deref(), Some("argos-9999"));
        assert_eq!(load.reserved_ids().iter().next().map(String::as_str), Some("argos-9999"));
    }

    /// JSON 도 아닌 줄에는 내놓을 id 가 없다. 없는 것을 지어내지 않는다.
    #[test]
    fn a_line_that_is_not_even_json_yields_no_id() {
        let load = parse_issues("{깨짐\n");
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].id, None);
        assert!(load.reserved_ids().is_empty());
    }

    /// **쓰기는 그 id 를 피해서 뽑는다.** 안 피하면 못 읽는 동안은 아무 데도
    /// 안 보이는 중복이 생기고, 그 줄이 읽히게 되는 날 모든 쓰기가 막힌다.
    #[test]
    fn a_write_reserves_the_ids_it_cannot_read() {
        let (r, d) = repo("reserve");
        std::fs::write(
            d.join(".moai/issues.jsonl"),
            "{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n",
        )
        .unwrap();

        let minted = r
            .with_write(|issues, cfg, reserved| {
                assert!(reserved.contains("argos-9999"), "못 읽는 줄의 id 를 안 줬다 — {reserved:?}");
                // 그 줄의 id 를 그대로 노리는 씨앗이라도 다른 것이 나와야 한다.
                let taken = taken_ids(issues, reserved);
                let id = crate::id::generate(&cfg.prefix, &taken, "argos-9999");
                issues.push(issue(&id));
                Ok((vec![], id))
            })
            .unwrap();
        assert_ne!(minted, "argos-9999", "못 읽는 줄과 같은 id 를 뽑았다");
    }

    /// 등록한 디렉터리 하나를 열 때 넷을 가른다 — 연 것, init 전, 사라짐, 깨짐.
    #[test]
    fn open_tells_uninit_missing_and_broken_apart() {
        let root = scratch("open");
        assert!(matches!(Repo::open(&root), Ok(Opened::Repo(r)) if r.root == *root.path()));

        let bare = root.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        assert!(matches!(Repo::open(&bare), Ok(Opened::Uninit)));
        // 위로 찾지 않는다 — 바깥 저장소의 `.moai` 를 제 것으로 내면 안 된다.
        assert!(matches!(Repo::open(&bare.join("gone")), Ok(Opened::Missing)));
        // 경로 중간이 파일이어도 없는 것이다.
        std::fs::write(root.join("file"), "").unwrap();
        assert!(matches!(Repo::open(&root.join("file/sub")), Ok(Opened::Missing)));
        assert!(Repo::open(&root.join("file")).is_err(), "파일을 디렉터리로 열었다");

        let broken = root.join("broken");
        std::fs::create_dir_all(broken.join(".moai")).unwrap();
        std::fs::write(broken.join(".moai/config.toml"), "prefix = \"\"\n").unwrap();
        assert!(Repo::open(&broken).is_err(), "깨진 설정을 init 전으로 접었다");
    }
}
