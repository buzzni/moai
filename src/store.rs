//! 저장소를 찾고 읽고 쓴다. **파일을 만지는 곳은 여기뿐이다** (`cmd::init` 제외).
//!
//! `issues.jsonl` 을 바꾸는 길은 [`Repo::with_write`] 하나다. 명령마다 쓰기
//! 경로가 갈라지면 락·정렬·검증·원자적 쓰기를 저마다 반쯤 구현하게 된다.

use crate::config::Config;
use crate::fail::{Fail, R, code};
use crate::i18n::Lang;
use crate::model::{Actor, Issue, JournalEntry};
use crate::path::dir_of;
use fs2::FileExt;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 락을 못 잡으면 **아무것도 쓰지 않고** 물러난다.
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// 쓰기 경로가 **말 없이** 들고 나오는 까닭(moai-iq7j, 2026-09-21).
///
/// 저장 계층은 화면 말을 모른 채 둔다(2026-09-20 사용자 결정) — `store::with_write` 는 락을 쥔 채
/// 돌고, 머지 드라이버는 사용자 설정을 아예 안 연다. 그래서 이 자리의 거절은 **자료**이고, 글을
/// 짓는 자리는 [`crate::view::store_trouble`] 하나다. `user_config::WriteTrouble` 과
/// `read_marks::SheetRefusal` 이 이미 그 꼴이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// 연 자리가 디렉터리가 아니다 — 그 자리([`Repo::open`]).
    NotADirectory { at: String },
    /// 같은 id 가 두 번 있다 — 그 id. 짝지을 수가 없으니 아무것도 안 쓴다.
    DuplicateId { id: String },
    /// 다른 moai 가 쓰고 있어 물러났다 — 기다린 초.
    LockBusy { secs: u64 },
    /// 저장소 락을 그 자리에 못 잡는다([`crate::held::lock`], moai-sn57) — 락 자리와 까닭. 링크이거나 보통
    /// 파일이 아니거나, 그 디렉터리가 체크아웃 밖·`.git/` 으로 풀린다. 기다려도 안 풀리니 `locked` 가 아니다.
    LockUnheld { at: PathBuf, why: crate::held::Unheld },
    /// 스냅샷이 쓰는 동안 쥐는 락과 한 파일로 풀린다([`Repo::far_lock`]) — 스냅샷 자리와 풀린 자리. 거기 쓰면
    /// `rename` 이 락을 갈아끼워 쓰는 쪽들이 서로를 못 막는다.
    SnapshotOnLock { at: PathBuf, to: PathBuf },
    /// 스냅샷은 담겼는데 저널을 못 적었다 — io 가 낸 말과, 말이 함께 사라진 이슈들.
    JournalLost { said: String, ids: Vec<String> },
    /// 쓰려는 줄이 검사에 걸렸다([`crate::model::Invalid`], moai-yve0) — 가리키는 자리와 그 까닭.
    Invalid { at: At, why: crate::model::Invalid },
    /// 적을 저널 줄에 메일이 없다([`file_entries`], moai-nzlo) — 그 줄의 id.
    NoJournalEmail { id: String },
}

impl Trouble {
    /// 이 거절의 `--json` 코드 — **자료가 되기 전에 들던 값을 그대로 든다**(리뷰).
    ///
    /// 글을 자료로 바꾸면서 갈래마다 달랐던 코드를 [`Repo::with_write`] 가 `broken` 하나로 뭉치던
    /// 판이 있었다. `fail` 의 머리 글이 적어 둔 그대로 **`code` 는 받는 쪽이 분기하는 값**이라,
    /// 태그에 쉼표를 하나 넣은 `moai add` 가 "파일이 깨졌다" 로 나갔고 같은 검사를 지나는
    /// `moai edit`·`moai link` 는 제 손으로 `Fail::new` 를 지어 `error` 를 냈다 — 한 거절이
    /// 명령마다 다른 코드로 나가는 것이 그 글이 이름 붙여 둔 실패다.
    ///
    /// **낱말을 빠짐없이 적는다** — `_` 로 받으면 갈래가 느는 날 새 거절이 말없이 `error` 가 된다.
    fn code(&self) -> &'static str {
        match self {
            // 파일이 상했다 — 사람이 손으로 푼다. 락 자리의 거절은 스냅샷의 거절(`Repo::rooted`)과 같은 코드다 —
            // `locked` 로 내면 그 코드에 다시 해 보는 고리가 끝없이 돈다(2026-10-02 사용자 결정).
            Trouble::DuplicateId { .. } | Trouble::LockUnheld { .. } | Trouble::SnapshotOnLock { .. } => code::BROKEN,
            Trouble::LockBusy { .. } => code::LOCKED,
            // 고칠 곳이 argv 가 아니라 사용자 정보다 — `model::NoActor` 와 같은 코드로 나간다.
            Trouble::NoJournalEmail { .. } => code::NO_ACTOR,
            // 나머지는 부르는 쪽이 준 값이나 자리가 틀린 것이다.
            Trouble::NotADirectory { .. } | Trouble::JournalLost { .. } | Trouble::Invalid { .. } => code::ERROR,
        }
    }
}

/// 거절이 **가리키는 줄**(moai-1rkl, moai-yve0). 이미 선 줄은 id 로, 이번 쓰기가 짓는 줄은
/// 제목으로 가리킨다 — 거절은 쓰기를 통째로 물리므로 방금 뽑은 id 는 어디에도 안 남는다.
///
/// **낱말이 아니라 갈래로 든다** — "새 줄" 은 화면 말이고, 고르는 자리(락 안)는 그 말을 모른다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum At {
    /// 이미 선 줄 — 그 id.
    Id(String),
    /// 아직 안 지은 줄 — 그 제목(한 줄로 접어 60바이트에 맞춘 것).
    Unwritten(String),
}

/// [`Repo::write_locked`] 가 멈춘 까닭 — 락을 쥔 자리는 말을 모르므로 거절은 자료로 들고 나온다
/// (`read_marks::Stop` 과 한 꼴이다). 락을 잡다 물러난 것([`Lock::inside`]·[`Lock::wait`])도 이 꼴이다.
enum Stop {
    /// io·락이 낸 것 — 이미 글이다.
    Failed(Fail),
    /// 말 없이 든 거절 — 글은 [`Repo::with_write_lines`] 가 락을 놓은 뒤에 편다([`Stop::said`]).
    Refused(Trouble),
}

impl Stop {
    /// 고른 말로 편 실패 — **코드는 갈래가 쥔다**([`Trouble::code`]). 여기서 하나로 뭉치면 태그 오타가
    /// "파일이 깨졌다" 로 나간다. 말은 이 자리에서야 묻는다.
    fn said(self, lang: impl FnOnce() -> crate::i18n::Lang) -> Fail {
        match self {
            Stop::Failed(e) => e,
            Stop::Refused(t) => Fail::coded(crate::view::store_trouble(lang(), &t), t.code()),
        }
    }
}

impl From<Fail> for Stop {
    fn from(e: Fail) -> Stop {
        Stop::Failed(e)
    }
}

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

/// [`remember_invoked`] 가 적어 둔 자리 — `-C` 가 옮기기 **전의** 현재 디렉터리다(리뷰 moai-gu5m.ke0).
///
/// 상대 철자로 부른 실행 파일(`argv[0]`)은 커널이 이 자리에서 찾았다. 그 철자를 되짚는 쪽(`cmd::skill` 의
/// `invoked`)이 `-C` 가 옮긴 지금 자리에 붙이면 없는 자리를 짚는다. `main` 을 안 지난 부름(단위 시험)에서는
/// 지금 자리다.
pub(crate) fn invoked_dir() -> Option<PathBuf> {
    match INVOKED.get() {
        Some(at) => at.clone(),
        None => std::env::current_dir().ok(),
    }
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
    /// 대개 나오고, JSON 조차 아닌 줄도 **머리가 성하면 나온다**(moai-ijfy).
    /// 읽는 자는 [`crate::id::id_of`] 하나고, 그 둘째 단이 머지 드라이버가
    /// 짝짓는 자와 같은 자라 둘이 갈릴 자리가 없다. 둘 다 못 읽는 줄에서만
    /// `None` 이고, 그때는 지어내지 않는다.
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

// **`.moai` 를 못 찾았다는 말은 이 층에 없다**(moai-5j49, 2026-09-21 사용자 결정). `.moai` 밖에서
// 아무 명령이나 치면 서는 첫 화면이라 영어로 골라 온 사람이 가장 먼저 읽는 줄인데, 저장 계층은
// 화면 말을 모른 채 둔다는 2026-09-20 결정이 그 줄을 여기 묶어 두고 있었다. 이제 찾기만 여기서
// 하고(`Repo::find`) 못 찾은 것을 말로 옮기는 자리는 `crate::cmd::open_repo` 하나다.
//
// **나머지 다섯은 여기 남는다**(같은 결정) — `디렉터리가 아니다`·`id 가 두 번 있다`·
// `적어 온 말도 안 남았다`·`경로에 디렉터리가 없다`·락 시간초과. 넷이 `Repo::with_write` 와
// `Lock` 안이라, 거기까지 영어로 세우려면 `crate::fail::Fail` 이 자료를 들고 나중에 펴지는
// 꼴이 되어야 한다 — `Fail::message` 를 읽는 자리가 열일곱이고, 쓰기 경로의 오류 꼴이 통째로
// 바뀐다. 0.1.1 로 미뤘다(idea `moai-uqxn`). `config::Config` 의 검증 글 열넷도 `open_repo` 를
// 지나 사람에게 닿지만 그쪽은 이 층이 아니다(idea `moai-gbk3`).
//
// **`///` 로 적지 않는다**(리뷰) — 이 글이 딸릴 항목은 지워진 `NOT_A_REPO` 였다. 문서 주석으로
// 두면 바로 아래 [`CLIMBED`] 에 들러붙어, 상관없는 static 의 요약 줄이 이 글이 된다.

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
///   천장에 걸리면 [`Repo::find`] 가 못 찾고 `decide` 가 `None` 을 내어, 규칙 셋이
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
/// 설정이 깨졌거나, 스냅샷을 안 읽기로 했거나(체크아웃 밖·`.git/` 으로 가는 링크, 보통 파일이 아닌 것),
/// 디렉터리를 못 읽는 것은 여기가 아니라 `Err` 다 — 고칠 것이지 상태가 아니다.
#[derive(Clone)]
pub enum Opened {
    Repo(Repo),
    /// 디렉터리는 있는데 `.moai/` 가 없다.
    Uninit,
    /// 디렉터리가 없다.
    Missing,
}

impl Repo {
    /// `.moai/` 를 가진 디렉터리를 위로 찾는다 — **못 찾은 것을 실패로 접지 않는다**(`None`).
    /// 깊이를 코드에 박지 않는다([`look`]). 못 찾은 것을 말로 옮기는 자리는 [`crate::cmd::open_repo`] 다.
    ///
    /// 못 찾은 것과 찾았는데 설정이 깨진 것은 다르다. `.moai` 밖에서 부른
    /// `status` 는 앞의 것일 때만 등록한 프로젝트를 보여 줘야 한다 — 뒤의 것까지
    /// 한눈 보기로 넘기면 제 저장소의 깨진 설정이 남의 프로젝트 목록 뒤에 숨는다.
    /// **말은 거절할 때만 묻는다**(moai-iq7j·moai-ivt9) — 설정이 깨졌거나 스냅샷을 안 읽기로 한 때만
    /// [`Lang`] 을 푼다.
    /// 값으로 받으면 멀쩡한 판마다 사용자 설정을 열고, 훅은 도구 호출마다 이 길을 지난다.
    pub fn find(lang: impl FnOnce() -> Lang) -> R<Option<Repo>> {
        let dir = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
        Repo::find_from(&dir, lang)
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
    pub fn find_from(dir: &Path, lang: impl FnOnce() -> Lang) -> R<Option<Repo>> {
        Repo::found_root(dir).map(|found| Repo::from_found(found, lang)).transpose()
    }

    /// 찾은 자리로 [`Repo`] 를 짓는다 — **옮겨 가는 길은 여기 하나다**([`Repo::find_from`] 이 쓴다).
    /// 옮기지 않는 [`Repo::find_here`] 와 갈라 두지 않으면 한쪽만 `MOAI_HERE` 를 보거나 한쪽만
    /// 워크트리를 안 옮긴다.
    ///
    /// **`MOAI_HERE` 는 이 체크아웃에 쓴다.** 일부러 갈라 놓는 자리다 — 옆 스냅샷이 갈라진 상태를
    /// 짓는 시험과, 그 워크트리에서만 쓰는 트래커를 든 사람이다. 옮기는 것은 막는 것이 아니라
    /// 옮기는 것이므로, 되돌릴 손잡이 하나를 두는 값이 싸다. **끄는 값도 받는다** — 글이 `=1` 로
    /// 적혀 있어 `MOAI_HERE=0` 을 "아니오" 로 읽고 쓰는 쪽이 생기는데, 있기만 하면 켜던 판은
    /// 그 사람에게 말없이 갈라진 스냅샷을 줬다(리뷰 moai-71ht.jlh).
    fn from_found(found: PathBuf, lang: impl FnOnce() -> Lang) -> R<Repo> {
        let Some(root) = Repo::redirect(&found) else {
            return Repo::rooted(found, lang);
        };
        // **설정은 한 번만 읽는다** — 찾은 자리로 [`Repo`] 를 지어 놓고 버리던 판은 워크트리의
        // `config.toml` 을 읽고 안 쓴 채 버렸다. 훅이 도구 호출마다 지나는 길이다.
        Ok(Repo { moved_from: Some(found), ..Repo::rooted(root, lang)? })
    }

    /// [`Repo::find_from`] 과 같되 **안 옮긴다** — 찾은 자리의 트래커 그대로다.
    ///
    /// 옮겨 갈 루트를 못 읽을 때(거기 `config.toml` 이 깨졌거나 스냅샷을 안 읽기로 했다, moai-itsu) 물러설
    /// 자리다. 훅이 그 자리로 선다 — `moai` 는 크게 실패하는 것이 맞지만, 훅까지 조용해지면 그 한 파일
    /// 때문에 저장소의 모든 워크트리에서 규칙이 통째로 꺼진다(리뷰 moai-71ht.i1u).
    pub fn find_here(dir: &Path, lang: impl FnOnce() -> Lang) -> R<Option<Repo>> {
        Repo::found_root(dir).map(|root| Repo::rooted(root, lang)).transpose()
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

    /// **설정의 거절도 자료로 받는다**(moai-ivt9) — `config::Config::load` 는 화면 말을 모르고,
    /// 펴는 자는 여기 하나다([`crate::view::config_refused`]).
    ///
    /// **스냅샷도 여기서 먼저 잰다**(moai-itsu) — 체크아웃 밖·`.git/` 으로 가는 링크거나(끝이 없는 링크도
    /// 끝의 자리로 잰다) 보통 파일이 아니면 이 저장소를 여는 길이 고른 말로 멈춘다(2026-10-02 사용자 결정,
    /// `--json` 코드는 `broken`). 그 저장소에서 부른 명령은 거기서 서고, 여럿을 한눈에 그리는 자리(밖의
    /// 한눈 보기·`project ls`·탐색기의 레이어)는 그 줄을 "못 읽는다" 로 대고 나머지를 그린다. 받은 저장소가
    /// 커밋한 `issues.jsonl -> /proc/self/pagemap` 하나로 모든 명령이 메모리를 다 쓰던 자리다. 빈 스냅샷으로
    /// 읽고 알리는 길도 있었지만 그러면 보드가 "이슈 0개" 라는 거짓 그림을 그린다 — 쓰기는 어차피
    /// [`target_of`] 가 같은 링크를 거절한다.
    ///
    /// **재는 것은 여기고 읽는 것은 [`Repo::read`] 다.** 읽기는 화면 말을 모르는 자리라(락 안에서도
    /// 읽는다) 거절을 고른 말로 못 편다. 여기서 먼저 물어 흔한 경우는 고른 말로 서고, 그 뒤에 링크가 갈린
    /// 경우(탐색기가 떠 있는 동안 받은 `git pull`)는 읽기가 같은 자로 다시 재어 말 없는 꼴로 멈춘다. 쓰기는
    /// 그 읽기보다 먼저 [`Repo::far_lock`] 의 [`target_of`] 가 같은 링크를 제 말(`error`)로 거절한다.
    ///
    /// **뿌리는 한 번 푼다**([`crate::held::Home`]) — 설정과 스냅샷을 같은 뿌리로 잰다.
    fn rooted(root: PathBuf, lang: impl FnOnce() -> Lang) -> R<Repo> {
        let home = crate::held::Home::of(&root);
        let config = match Config::load_in(&root, &home) {
            Ok(c) => c,
            Err(why) => return Err(Fail::config(&why, lang())),
        };
        let issues = root.join(".moai").join("issues.jsonl");
        if let Err(why) = crate::held::check(&issues, &home) {
            return Err(Fail::coded(crate::held::refused(lang(), &issues, &why), code::BROKEN));
        }
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
    ///
    /// **"없다" 를 가르는 자는 [`gone`] 하나다**(moai-blvx) — 읽음 쪽([`crate::read_marks::settle`])도
    /// 같은 자를 쓴다. 닫은 글로 여기 두던 판은 두 표면이 같은 자리를 달리 불렀다.
    ///
    /// **말은 거절할 때만 묻는다**(moai-iq7j) — 여는 것은 한눈 보기가 줄마다 부르는 길이라
    /// (`projects::open_shallow`), 값으로 받으면 멀쩡한 줄마다 사용자 설정을 연다.
    pub fn open(dir: &Path, lang: impl FnOnce() -> crate::i18n::Lang) -> R<Opened> {
        match std::fs::metadata(dir) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => {
                let why = Trouble::NotADirectory { at: dir.display().to_string() };
                return Err(Fail::new(crate::view::store_trouble(lang(), &why)));
            }
            Err(e) if gone(&e) => return Ok(Opened::Missing),
            Err(e) => return Err(Fail::new(format!("{}: {e}", dir.display()))),
        }
        match std::fs::metadata(dir.join(".moai")) {
            // **여기도 루트로 옮겨 간다**(moai-y7go, 리뷰 moai-71ht.jlh 사용자 결정) — 등록한 자리가
            // 딸린 워크트리면 탐색기의 쓰기가 그 워크트리의 스냅샷에 조용히 들어가, 같은 자리를 CLI 로
            // 칠 때와 다른 파일이 바뀐다. **위로 찾지 않는다는 계약은 그대로다** — 옮기는 곳은 위가
            // 아니라 같은 나무의 주 체크아웃이고, 거기에 트래커가 없으면 옮기지 않는다.
            Ok(m) if m.is_dir() => match Repo::redirect(dir) {
                Some(root) => {
                    Ok(Opened::Repo(Repo { moved_from: Some(dir.to_path_buf()), ..Repo::rooted(root, lang)? }))
                }
                None => Repo::rooted(dir.to_path_buf(), lang).map(Opened::Repo),
            },
            // `.moai` 가 파일이면 저장소가 아니다 — 위로 찾는 [`Repo::find`] 의 `is_dir` 과 같은 자다.
            Ok(_) => Ok(Opened::Uninit),
            // **`.moai` 가 없는 딸린 워크트리도 옮겨 간다**(리뷰 moai-71ht 셋째 판) — moai 를 들이기 전에
            // 갈라진 가지다. "init 전" 으로 대던 판은 `moai -C <워크트리> init` 을 시켰는데, 그 뒤로는
            // 어느 길도 그 트래커를 안 읽고(CLI 는 위로 찾아 루트로 간다) 커밋하면 병합에서 `config.toml`
            // 이 add/add 로 부딪힌다 — 아무도 안 읽는 파일을 만들라고 시킨 셈이었다.
            Err(e) if gone(&e) => match Repo::redirect(dir) {
                Some(root) => {
                    Ok(Opened::Repo(Repo { moved_from: Some(dir.to_path_buf()), ..Repo::rooted(root, lang)? }))
                }
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
    /// **옛 한 파일.** 새 줄은 여기 안 간다 — 읽을 때 [`Repo::journal_files`] 가 드는 N 개 중
    /// 하나일 뿐이다(moai-b7cq). 옮기는 마이그레이션은 없다: 두 꼴이 계속 나란히 산다.
    pub fn journal_path(&self) -> PathBuf {
        self.dir().join("journal.jsonl")
    }

    /// 사람마다 갈린 저널이 사는 자리 — `.moai/journal/<메일>.jsonl`(moai-b7cq).
    ///
    /// **파일이 여럿인 것이 정상 꼴이다**(2026-09-21 사용자 결정). 사람이 여럿이면 메일도
    /// 여럿이라, 읽는 쪽은 처음부터 N 개를 합치게 짜여 있다 — 메일이 바뀌어 하나 더 서는 것은
    /// 값이 0 이다.
    pub fn journal_dir(&self) -> PathBuf {
        self.dir().join("journal")
    }

    /// 저널 자리의 이름 전부 — 옛 한 파일이 먼저, 그다음 [`Repo::journal_dir`] 의 것이 이름 차례로. 링크로
    /// 이은 이름을 아직 안 접은 목록이라, 읽는 쪽은 [`Repo::journal_files`] 를 부르고 이것을 바로 쓰는 것은
    /// 표식([`Repo::journal_marks`])뿐이다.
    ///
    /// **차례가 계약이다.** 같은 `ts` 를 든 줄은 [`Repo::journal_by_id`] 의 안정 정렬이 읽은
    /// 차례 그대로 두므로, 파일 차례가 흔들리면 같은 저장소에서 부를 때마다 이력의 차례가
    /// 바뀐다. `read_dir` 의 차례는 파일시스템의 것이라 정해져 있지 않아 이름으로 세운다.
    ///
    /// 디렉터리가 없으면 옛 한 파일뿐이다 — 저널이 없는 저장소는 고장이 아니다.
    ///
    /// **못 여는 자리는 넘어가되 조용히는 아니다**(2026-09-21 사용자 결정, moai-6ney). 여기서
    /// 멈추면 자리 하나를 못 여는 것만으로 **모든 사람의** 이력이 통째로 안 보인다 — 읽기는
    /// 관대하고 쓰기는 엄하다는 규약의 자리다. 대신 [`note_unread`] 로 세어 두고, `cmd::run` 이
    /// 나오면서 stderr 로 대며 비영 종료로 끝낸다. 목록의 줄 하나를 못 읽는 것도 같다: 그 줄이
    /// 누구의 파일이었는지는 아무도 모르니 그 사실 그대로 센다.
    ///
    /// **옛 한 파일도 보통 파일일 때만 든다**(리뷰 moai-wcy8.rbj) — 아래 `journal/` 의 것과 같은 자다. 그
    /// 자리의 FIFO 를 여는 `fs::read` 는 쓰는 쪽이 올 때까지 영영 멈추고(`/dev/zero` 로 가는 링크면 메모리가
    /// 찰 때까지 읽는다), 탐색기는 이 읽기를 루프에서도 부른다. 없는 자리와 못 잰 자리는 그대로 든다 — 없으면
    /// 읽는 쪽이 넘기고 표식([`Repo::journal_marks`])이 나중에 생긴 것을 알아채며, 못 잰 까닭은 읽는 쪽이
    /// 센다([`note_unread`]).
    ///
    /// **이름마다 그 파일의 자리([`FileId`])를 곁에 든다** — 판정하려고 이미 잰 `metadata` 에서 나온다.
    /// [`Repo::journal_files`] 가 하드 링크까지 접는 데 쓰고, 그것을 거기서 다시 재면 저널을 읽을 때마다 이름
    /// 수만큼 `stat` 이 는다.
    ///
    /// **보통 파일로 안 서는 이름과 못 잰 이름은 링크의 끝을 잰다**(리뷰 moai-itsu.n8z) — 체크아웃 밖·`.git/`
    /// 으로 가는 링크면 `outside` 로 센다([`crate::held::place`]). 그 이름은 저널이 아니라 목록에서 빠지는데,
    /// 재기 전에 빠지던 판은 `x.jsonl -> /dev/zero`·`-> /밖/디렉터리` 를 말없이 넘겨, 같은 커밋된 링크가 끝이
    /// 보통 파일일 때만 `outside` 로 섰다. 못 잰 이름(끝이 없거나 못 닿는 링크)도 그 링크를 대야 고칠 곳이
    /// 보인다 — `failed`·`permission` 으로 대면 `chmod` 를 하러 간다. 뿌리는 그런 이름을 처음 만날 때 푼다 —
    /// 보통 파일뿐인 흔한 경우에는 안 푼다.
    fn journal_names(&self) -> Vec<(PathBuf, FileId)> {
        let home = std::cell::OnceCell::new();
        let outside = |p: &Path| match crate::held::place(p, home.get_or_init(|| crate::held::Home::of(&self.root))) {
            Err(why) => {
                note_missed(&self.root, p, Missed::Held(why));
                true
            }
            Ok(_) => false,
        };
        let legacy = self.journal_path();
        let mut out = match std::fs::metadata(&legacy) {
            Ok(m) if !m.is_file() => {
                outside(&legacy);
                Vec::new()
            }
            Ok(m) => vec![(legacy, file_id(&m))],
            // 못 잰 옛 한 파일은 그대로 든다 — 링크의 끝은 [`Repo::journal_files`] 가 접기 전에 잰다.
            Err(_) => vec![(legacy, None)],
        };
        let at = self.journal_dir();
        let dir = match std::fs::read_dir(&at) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
            Err(e) => {
                note_unread(&self.root, &at, &e);
                return out;
            }
        };
        let mut split = Vec::new();
        for e in dir {
            let p = match e {
                Ok(e) => e.path(),
                Err(e) => {
                    note_unread(&self.root, &at, &e);
                    continue;
                }
            };
            if !p.extension().is_some_and(|x| x == "jsonl") {
                continue;
            }
            // **`.` 로 시작하는 이름은 저널이 아니다**(moai-o4jt). Emacs 는 저장하지 않은 채 연 파일 곁에
            // 잠금 링크 `.#<이름>.jsonl -> user@host.PID:BOOT` 를 세운다. 가리키는 곳이 없는 링크라 아래
            // `metadata` 가 지고, 그것을 못 읽은 저널로 세어 — 빠진 줄은 하나도 없는데 — 모든 세션의
            // `show`·`-g`·`--removed` 가 1 로 끝났다. `--removed` 의 알림은 손으로 줄을 지우라고까지 권했다.
            //
            // **`.#` 하나가 아니라 `.` 전부를 거른다.** 이 도구가 짓는 이름은 [`journal_file`] 이 메일의
            // `.` 를 모두 `_` 로 접어, `.jsonl` 앞에 `.` 가 서는 일이 처음부터 없다 — 메일의 앞부분도 `.` 로
            // 시작할 수 없다. 그러니 넓혀도 잃는 저널이 없다. 반대로 이 디렉터리에 실제로 서는 숨은 이름은
            // 다 다른 도구의 것이다. Emacs 의 잠금 링크 말고도, macOS 가 SMB·exFAT 같은 파일시스템이나
            // tar·zip 아카이브에 까는 AppleDouble(`._<이름>.jsonl`)이 있다. `.#` 만 막으면 다음 철자가 같은
            // 구멍을 다시 연다. 숨지 않은 이름(옛 `<메일>.jsonl` 을 이은 링크 같은 것)은 그대로 읽는다.
            if p.file_name().is_some_and(|n| n.as_encoded_bytes().starts_with(b".")) {
                continue;
            }
            // **`metadata` 로 잰다** — `read_dir` 의 `file_type` 은 심볼릭 링크를 따라가지 않아,
            // 디렉터리를 가리키는 링크가 `!is_dir` 을 지나 아래 `fs::read` 에서 EISDIR 로 터진다.
            //
            // **못 잰 자리는 조용히 안 접는다**(리뷰). `Path::is_file` 은 실패를 `false` 로
            // 접으므로, 이름은 읽히는데 잴 수 없는 자리(`chmod 400 .moai/journal` — 읽기는
            // 되고 훑기는 안 되는 자리다)가 통째로 **말없이** 빠졌다: 이력이 하나도 없는
            // 화면이 stderr 한 줄 없이 0 으로 끝났다. 이 판을 시끄럽게 만들자는 것이
            // moai-6ney 인데, 정작 그 금이 여기 하나 남아 있었다.
            match p.metadata() {
                Ok(m) if m.is_file() => split.push((p, file_id(&m))),
                // 디렉터리(나 그것을 가리키는 링크)는 저널이 아니다 — 못 읽은 것이 아니므로 안 센다. 다만 그
                // 이름이 체크아웃 밖·`.git/` 으로 가는 링크면 그 링크를 댄다.
                Ok(_) => {
                    outside(&p);
                }
                Err(e) => {
                    if !outside(&p) {
                        note_unread(&self.root, &p, &e);
                    }
                }
            }
        }
        split.sort();
        out.extend(split);
        out
    }

    /// 읽을 저널 파일 — [`Repo::journal_names`] 에서 **푼 자리가 같은 파일을 한 번만** 남긴다(moai-p9mq).
    ///
    /// 메일을 바꾼 사람이 옛 `<메일>.jsonl` 을 새 파일로 이어 두면 `journal_names` 의 `metadata` 가 링크를
    /// 따라가 둘 다 보통 파일로 보여, 같은 줄이 이력·`-g`·`show --removed` 에 두 번 서고 `work` 는 토큰을 두 번
    /// 더했다. 쓰기([`append_inside`])는 링크를 따라 한 파일에만 드니 읽기도 그 한 파일로 센다. 차례는 이름
    /// 목록의 것이고, 한 파일은 **처음 만난 이름의 자리**에 선다.
    ///
    /// **링크 이름은 푼 자리로 바꿔 둔다**(리뷰 moai-7dmq.j2a). 링크 철자를 남기면 둘이 샌다. 목록을 짓고 읽기
    /// 전에 그 링크가 사라지면(체크아웃·머지가 갈아끼운다) 읽는 쪽이 없는 자리로 넘겨, 멀쩡히 선 끝 파일의 이력이
    /// 말없이 빠졌다. 또 못 푼 조각의 자리(`Garbled.at`)와 못 읽은 자리가 링크를 대, 그것을 보고 `sed -i` 로
    /// 고치면 링크가 보통 파일로 갈려 겹침이 영영 돌아왔다. 보통 파일의 이름은 받은 철자 그대로다 — 조상에 링크가
    /// 있는 뿌리에서 모든 자리가 다른 철자로 나가지 않게 한다.
    ///
    /// 푸는 자는 [`crate::held::place`] 라 끝내 못 푸는 자리(없는 옛 한 파일, 제 자리를 가리키는 링크)는 받은
    /// 철자 그대로 들어 다른 파일과 접히지 않는다 — 읽는 쪽이 넘기거나 센다. 빼면 못 읽은 까닭이 말없이 사라진다.
    ///
    /// **하드 링크도 접는다**(moai-itsu.aod) — 풀어도 이름이 둘이라 푼 자리로는 못 접던 것을, 그 파일의 자리
    /// ([`FileId`], 장치와 inode)로 접는다. [`Lock::holds`] 가 같은 파일을 가르는 자와 같다([`file_id`]). 잴 수
    /// 없던 이름(위의 못 푼 자리)은 전처럼 푼 자리로 접는다. **받은 저장소에서는 여전히 못 접는다** — git 은 하드
    /// 링크를 싣지 않아 같은 줄을 든 보통 파일 둘로 선다. 이 접기가 값을 내는 것은 링크를 건 그 체크아웃에서다.
    ///
    /// **열쇠가 겹치면 앞 이름을 다시 잰다**(리뷰 moai-itsu.n8z) — 자리는 [`Repo::journal_names`] 가 이름을 훑을
    /// 때 잰 것이라, 그 사이에 체크아웃·머지가 지운 파일의 inode 를 새로 지은 파일이 물려받으면(ext4 는 비운
    /// 번호를 곧바로 다시 준다) 다른 두 파일이 한 열쇠를 든다. 그대로 접으면 산 파일의 이력이 그 읽기에서 말없이
    /// 빠진다. 앞 이름이 더는 그 자리가 아니면 뒤 이름을 읽는다 — 겹치는 것은 하드 링크와 이 틈뿐이라, 흔한
    /// 경우에는 `stat` 이 안 는다.
    ///
    /// **체크아웃 밖으로 풀리는 자리는 안 읽는다**(moai-karj). 쓰기([`append_inside`])가 [`target_of`] 로 이미
    /// 거절하던 자리를 읽기도 같은 자로 잰다 — 저장소가 든 파일은 제 체크아웃 안에서만 링크를 따른다. 받은
    /// 저장소가 커밋한 `.moai/journal/x.jsonl -> /proc/self/pagemap` 은 `stat` 이 크기 0 인 보통 파일이라 답해
    /// [`Repo::journal_names`] 의 판정을 지나고, [`Repo::journal_bytes`] 의 `fs::read` 가 256GiB 가량을 담으려다
    /// OOM 으로 죽었다. 자는 스냅샷·설정과 한 벌이다([`crate::held::place`], moai-itsu) — 안을 가리키는 링크
    /// (moai-p9mq 의 `<옛 메일>.jsonl -> <새 메일>.jsonl`)는 따르고, 밖·`.git/` 으로 가는 링크는 끝이 없어도 안
    /// 따른다. 그 까닭은 거기 적혀 있다.
    ///
    /// - 견주는 것은 [`crate::held::place`] 가 끝까지 푼 자리다. 끝 조각의 링크도, 옛 한 파일의 링크도, 가운데
    ///   디렉터리의 링크(`.moai/journal -> /밖`)도 거기서 다 드러난다
    /// - 딸린 워크트리에서 부른 `moai` 는 루트의 트래커를 읽으므로 `self.root` 가 그 루트다 — 쓰기가 견주는
    ///   뿌리와 같다
    /// - **접기 전에 잰다** — 밖을 가리키는 이름이 둘이면 둘 다 센다. 접고 나서 재면 둘째가 말없이 빠진다
    /// - **넘기되 조용히는 아니다**(moai-6ney) — 링크 이름을 [`note_missed`] 에 `outside` 로 센다(moai-itsu).
    ///   고칠 것이 그 링크라 잠깐의 io 실패(`failed`)와 갈라 둔다
    fn journal_files(&self) -> Vec<PathBuf> {
        let home = crate::held::Home::of(&self.root);
        // 접는 열쇠 → 그 열쇠를 처음 든 이름. 잰 이름의 열쇠는 그 파일의 자리(`Ok`), 못 잰 이름은 푼 자리(`Err`)다
        // — 둘은 서로 안 겹친다.
        let mut seen: BTreeMap<Result<(u64, u64), PathBuf>, PathBuf> = BTreeMap::new();
        let mut out = Vec::new();
        for (p, id) in self.journal_names() {
            let real = match crate::held::place(&p, &home) {
                Ok(real) => real,
                Err(why) => {
                    note_missed(&self.root, &p, Missed::Held(why));
                    continue;
                }
            };
            let key = id.ok_or_else(|| real.clone());
            if let Some(first) = seen.get(&key) {
                let still = match &key {
                    Ok(id) => std::fs::metadata(first).ok().and_then(|m| file_id(&m)).as_ref() == Some(id),
                    Err(_) => true,
                };
                if still {
                    continue;
                }
            }
            seen.insert(key, p.clone());
            let link = std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink());
            out.push(if link { real } else { p });
        }
        out
    }

    /// 저널의 표식 — [`Repo::journal_dir`] 디렉터리 자체와, [`Repo::journal_names`] 의 이름마다
    /// (moai-wcy8.403). 탐색기가 노트를 읽고(`tui::Ground::read_notes`) 이것으로 다시 읽을 때를 안다 —
    /// `moai note` 는 스냅샷을 안 바꾸므로 스냅샷 표식만 보면 새 노트가 영영 안 실린다.
    ///
    /// **디렉터리도 잰다** — 처음 쓰는 사람의 `<메일>.jsonl` 은 목록에 없던 파일이라, 파일만 재면 그 사람의
    /// 첫 노트를 못 알아챈다. 파일이 생기면 디렉터리의 고친 때가 바뀐다. 없는 자리는 `None` 으로 서므로
    /// 나중에 생긴 것도 알아챈다([`stamp`]).
    ///
    /// **디렉터리는 훑기 전에 잰다**(리뷰 moai-wcy8.rbj) — `worktree::heads` 가 제 디렉터리를 재는 차례와
    /// 같다. 훑고 나서 재면 그 사이에 생긴 `<메일>.jsonl` 이 목록에는 없는데 디렉터리의 표식은 이미 그것을 센
    /// 뒤라, 그 사람이 이어 적는 노트를 걸음이 다음 시계까지 못 알아챈다.
    ///
    /// **접기 전의 이름을 잰다**(리뷰 moai-7dmq.j2a) — [`Repo::journal_files`] 가 접어 뺀 링크가 다른 파일로
    /// 옮겨 가도 그 이름의 표식은 바뀐다. 접은 목록만 재면 그 옮김을 걸음이 다음 강제 다시 읽기까지 못
    /// 알아챈다. [`stamp`] 는 링크를 따라가니 같은 파일을 두 번 재는 셈이지만, 표식은 견주기만 하니 겹쳐도 된다.
    pub fn journal_marks(&self) -> Vec<(PathBuf, Stamp)> {
        let dir = self.journal_dir();
        let dir_stamp = stamp(&dir);
        let mut out = vec![(dir, dir_stamp)];
        out.extend(self.journal_names().into_iter().map(|(p, _)| {
            let s = stamp(&p);
            (p, s)
        }));
        out
    }

    /// 전부 메모리로 읽는다. 디스크 인덱스는 두지 않는다 — 이전 시도가
    /// SQLite 인덱스를 만들어 재 보고 **순수 손해**임을 확인했다.
    ///
    /// **읽기는 락을 잡지 않는다.** 쓰기가 `rename` 으로 갈아끼우므로 독자는
    /// 옛 파일 아니면 새 파일을 보지, 찢어진 파일을 볼 수 없다.
    ///
    /// **체크아웃 안에서만 읽는다**([`read_snapshot`], moai-itsu) — 고른 말로 멈추는 것은 이 저장소를 지은
    /// [`Repo::rooted`] 가 먼저 했고, 여기서 다시 재는 것은 그 뒤에 링크가 갈린 경우를 위해서다. 여기는 화면
    /// 말을 모르는 자리라 그 거절은 말 없는 꼴([`crate::held::spelled`])로 선다.
    pub fn read(&self) -> R<Load> {
        Ok(read_snapshot(&self.root).map_err(Unsnapped::into_fail)?.unwrap_or_default())
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
    ///
    /// **말은 락을 놓은 뒤에 편다**(moai-iq7j, moai-rtji 가 읽음 쪽에 세운 꼴). 이 자리는 화면
    /// 말을 모른 채 둔다는 2026-09-20 결정 아래 있고, 그래서 `lang` 은 값이 아니라 **묻는 길**이다:
    /// 아무것도 거절하지 않는 판(= 거의 모든 판)은 사용자 설정을 아예 안 연다. 락 안에서 물으면
    /// 그 설정이 FIFO 일 때 락을 쥔 채 영영 멈춘다 — 몸통([`Repo::write_locked`])이 멈춘 까닭을
    /// 자료로 들고 나오고 [`Repo::with_write_lines`] 가 편다. 이 함수는 그 문에서 못 읽는 줄 인자만
    /// 뺀 것이다.
    pub fn with_write<T, F>(&self, lang: impl Fn() -> crate::i18n::Lang, f: F) -> R<T>
    where
        F: FnOnce(&mut Vec<Issue>, &Config, &BTreeSet<String>) -> R<(Vec<JournalEntry>, T)>,
    {
        self.with_write_lines(lang, |issues, _, cfg, reserved| f(issues, cfg, reserved))
    }

    /// [`Repo::with_write`] 와 같은 길인데 **못 읽는 줄도 닫는 함수에 넘긴다**(moai-mo9v.3yp).
    ///
    /// 못 읽는 줄을 지우는 길이 `moai rm --line` 하나라 이 문을 따로 연다. 둘째 쓰기 경로를 두지
    /// 않으려고 몸통은 하나다 — 락 안에서 다시 읽은 그 줄들이 넘어가고, 닫는 함수가 남긴 것만
    /// 파일 끝에 원문 그대로 되쓴다. 여느 명령이 그 줄들을 만질 까닭은 없으니 `with_write` 는
    /// 그것을 안 보인다: 보이면 못 읽는 줄을 들고 다시 쓴다는 약속을 명령마다 지켜야 한다.
    pub fn with_write_lines<T, F>(&self, lang: impl Fn() -> crate::i18n::Lang, f: F) -> R<T>
    where
        F: FnOnce(&mut Vec<Issue>, &mut Vec<LoadError>, &Config, &BTreeSet<String>) -> R<(Vec<JournalEntry>, T)>,
    {
        // 펴는 자리는 락을 다 놓은 여기다 — 코드는 갈래가 쥔다([`Stop::said`]).
        let (out, note) = self.write_locked(f).map_err(|stop| stop.said(&lang))?;
        // **못 적은 일기는 여기서 말이 된다** — 스냅샷은 담겼으니 실패가 아니고, 찍는 자는 `main` 이다.
        if let Some(t) = note {
            let said = crate::view::store_trouble(lang(), &t);
            MISSED.lock().unwrap_or_else(|e| e.into_inner()).push((self.root.clone(), said));
        }
        Ok(out)
    }

    /// [`Repo::with_write_lines`] 의 몸통([`Repo::with_write`] 도 이것을 지난다) — 락을 잡고, 읽고,
    /// 고치고, 쓴다. **돌아올 때 락을 놓는다.**
    /// 멈춘 까닭과 못 적은 일기는 [`Trouble`] 로 들고 나온다: 이 안은 화면 말을 모른다.
    fn write_locked<T, F>(&self, f: F) -> Result<(T, Option<Trouble>), Stop>
    where
        F: FnOnce(&mut Vec<Issue>, &mut Vec<LoadError>, &Config, &BTreeSet<String>) -> R<(Vec<JournalEntry>, T)>,
    {
        // **저장소 락은 받은 저장소가 커밋할 수 있는 자리다**([`Lock::inside`], moai-sn57) — 링크를 안 따르고
        // 체크아웃 안에만 짓는다. 두 락을 한 뿌리로 잰다. 못 잡으면 그 까닭도 자료로 들고 나간다.
        let home = crate::held::Home::of(&self.root);
        let lock = Lock::inside(&self.dir().join("lock"), &home)?;
        // 링크 너머의 트래커 자리에도 그 자리의 락을 잡는다 — 까닭은 [`Repo::far_lock`] 에 있다. 차례는
        // 늘 제 락 → 너머의 락이다.
        let _far = self.far_lock(&lock, &home)?;

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
        // `unreadable_line` 을 치명으로 내고 거기서만 비영 종료한다. 사람이 그 줄을 치우는 길은
        // `moai rm --line` 하나고, 그 명령만 [`Repo::with_write_lines`] 로 이 줄들을 받는다.
        let before = {
            let opaque: Vec<&str> = load.errors.iter().map(|e| e.text.as_str()).collect();
            render_issues(&load.issues, &opaque)
        };

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
        let mut unread = load.errors;
        let (entries, out) = f(&mut issues, &mut unread, &self.config, &reserved)?;
        let opaque: Vec<&str> = unread.iter().map(|e| e.text.as_str()).collect();

        // **글의 크기는 한 자리에서 잰다**(moai-m9a8). 노트·`mv -m`·`defer -m`·제목·본문이 모두
        // 여기를 지나므로 명령마다 따로 걸면 한 곳은 반드시 잊는다. 저널에 적힐 글은 여기서, 제목과
        // 본문은 아래 바뀐 줄에서 — 둘 다 스냅샷을 쓰기 전이라 거절하면 아무것도 안 남는다.
        //
        // **이 쓰기가 짓는 줄은 id 로 안 부른다**(moai-1rkl) — 거절하면 그 id 는 어디에도 안 남아,
        // 받는 쪽이 없는 것을 찾으러 간다. 그 줄의 제목 한 토막으로 가리킨다.
        for e in &entries {
            for (what, t) in [("note", &e.text), ("memo", &e.note)] {
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
                    false => {
                        match issues.iter().find(|i| i.id == e.id).map(|i| i.title.as_str()).or(e.title.as_deref()) {
                            Some(t) => crate::model::unwritten(t),
                            None => e.id.clone(),
                        }
                    }
                };
                crate::model::check_text_size(at, what, t)?;
            }
        }

        // **저널이 갈 파일은 스냅샷을 쓰기 전에 정한다**(moai-nzlo) — 바로 위 크기 검사와 같은
        // 까닭이다. 쓴 뒤에 알면 그 거절은 "썼지만 이력은 못 남겼다" 로 떨어져, 주인 없는 줄을
        // 막자는 결정이 알림 한 줄로 주저앉는다.
        let filed = file_entries(&entries).map_err(Stop::Refused)?;

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
                    ("title", Some(&i.title), was.map(|o| &o.title)),
                    ("body", i.body.as_ref(), was.and_then(|o| o.body.as_ref())),
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
                // 두 번 돌리면 그때마다 다른 id 가 나왔다.
                // **가리키는 말은 여기서 고른다**(moai-yve0) — 이번 쓰기가 짓는 줄은 거절 뒤에
                // 그 id 가 어디에도 안 남으므로(moai-1rkl) 제목 한 토막으로 가리킨다. 글은 락을
                // 놓은 뒤 [`Repo::with_write_lines`] 가 편다.
                i.validate_keeping(&self.config, kept).map_err(|why| {
                    let at = match was {
                        Some(_) => At::Id(i.id.clone()),
                        // 제목을 한 줄로 접어 자르는 자는 [`crate::model::fit_title`] 하나다 —
                        // 크기 거절문이 쓰는 자와 같은 토막이라야 두 말이 같은 줄을 가리킨다.
                        None => At::Unwritten(crate::model::fit_title(&i.title)),
                    };
                    Stop::Refused(Trouble::Invalid { at, why })
                })?;
            }
        }
        issues.sort_by(|a, b| a.id.cmp(&b.id));
        if let Some(dup) = first_duplicate(&issues) {
            // 락을 쥔 자리라 **글이 아니라 자료로** 물러난다 — 펴는 자는 [`Repo::with_write_lines`] 다.
            return Err(Stop::Refused(Trouble::DuplicateId { id: dup.to_string() }));
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
            write_atomic_inside(&self.issues_path(), after.as_bytes(), &self.root)?;
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
        let mut note = None;
        if !filed.is_empty()
            && let Err(e) = self.append_journal(&filed)
        {
            if !wrote {
                return Err(e.into());
            }
            // **적어 온 말은 저널에만 산다**(`mv -m`·`defer -m`·`promote` 의 메모). 스냅샷이
            // 담겼다고 "다시 부르지 않는다" 만 말하면 그 말은 영영 사라진다 — 어느 이슈의
            // 말이었는지 대어 `moai note` 로 다시 적게 한다.
            //
            // **여기서도 글을 안 짓는다**(moai-iq7j) — 락을 쥔 자리라 자료로 들고 나가고,
            // [`Repo::with_write_lines`] 가 락을 놓은 뒤에 펴서 [`MISSED`] 에 민다.
            //
            // **`rm` 줄의 메모는 적어 온 말이 아니다**(리뷰 moai-mo9v.1ln). `rm --line` 이 싣는 것은 지운
            // 줄의 원문이라 `moai note` 로 다시 적을 말이 아니고, 부른 쪽이 이미 화면과 `--json` 으로
            // 받았다. 셈에 넣으면 id 를 못 읽은 줄에서 빈 id 를 대며 `moai note` 를 권했다.
            let mut worded: Vec<String> = entries
                .iter()
                .filter(|j| j.kind != "rm" && (j.text.is_some() || j.note.is_some()))
                .map(|j| j.id.clone())
                .collect();
            worded.dedup();
            note = Some(Trouble::JournalLost { said: e.message, ids: worded });
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
        Ok((out, note))
    }

    /// `issues.jsonl` 이 **다른 디렉터리의 파일을 가리키는 링크면** 그 디렉터리의 락(`<그 자리>/lock`)도
    /// 잡는다(moai-4oab 리뷰). 링크가 아니거나 이미 쥔 락이면 `None` 이다 — 흔한 경우(링크가 아닌 스냅샷)는
    /// [`target_of`] 가 재고 끝난다.
    ///
    /// 쓰기는 링크를 따라가 가리키는 파일에 든다([`write_atomic_inside`]). 링크는 체크아웃 안에서만
    /// 따라가지만, 한 트래커가 다른 트래커를 품으면(하위 디렉터리의 트래커) 둘이 한 파일을 가리킬 수 있다.
    /// 그 둘은 저마다 제 `.moai/lock` 을 잡으므로, 이쪽 락만
    /// 쥐면 둘이 서로를 안 막아 나중에 `rename` 한 쪽이 앞의 줄을 조용히 지운다 — 돌려 보니 동시 `add`
    /// 스물넷에 일곱에서 열이 사라졌고, `mv --from` 의 집기 경합은 양쪽이 다 이겼다. 사용자 설정이 같은
    /// 까닭으로 둘째 락을 잡는다([`crate::user_config::update`]).
    ///
    /// **이름은 그 자리의 `lock` 이다**([`lock_beside`] 가 아니다) — 가리켜진 파일이 진짜 트래커의
    /// `issues.jsonl` 이면 그쪽 사람이 잡는 락이 바로 그것이라, 곁 이름(`issues.jsonl.lock`)을 잡으면 그쪽과
    /// 서로를 안 막는다. 대가는 그 자리가 `.moai` 가 아닐 때(`shared/`) 락 파일 하나가 서는 것인데, 잃는
    /// 것보다 싸다 — 사용자 설정이 dotfiles 저장소에 락을 세우는 것과 같은 비용이다.
    ///
    /// **이미 쥔 락인지는 철자가 아니라 파일로 견준다**([`Lock::holds`]) — 같은 `.moai` 안의 링크나 위
    /// 디렉터리가 링크라 철자만 다른 자리를 다시 잡으면, 제가 쥔 락을 제가 기다리다 `locked` 로 물러난다.
    /// 쓸 자리가 못 쓰는 자리면([`target_of`] 의 거절 — 고리, 없는 디렉터리) 여기서 그 말로 멈춘다.
    ///
    /// **다만 그 자리가 링크면 쥔 락으로 치지 않는다**(리뷰 moai-sn57.kq4) — [`Lock::holds`] 는 링크를 따라
    /// 견주므로, 커밋된 `lock -> /proc/self/fd/3` 은 프로세스마다 제가 쥔 `.moai/lock` 으로 풀려 아무도 너머의
    /// 락을 안 잡았다. 한 파일을 함께 쓰는 두 트래커가 다시 서로를 안 막아, 동시 `add` 스물넷이 다 0 으로
    /// 끝나고 열셋만 남았다. 링크인 그 자리는 [`Lock::inside`] 가 거절한다.
    ///
    /// **스냅샷이 쥔 락과 한 파일이면 안 쓴다**([`Trouble::SnapshotOnLock`], 리뷰 moai-sn57.kq4) —
    /// `issues.jsonl -> lock` 이면 쓰기마다 `rename` 이 락의 아이노드를 갈아끼워, 다음 쓰는 쪽은 새 파일을
    /// 잠그고 앞의 쪽은 지워진 파일을 쥔 채 서로를 안 막는다. 너머의 락은 잡기 전에는 없을 수 있어, 견주는
    /// 것은 두 락을 다 잡은 뒤다.
    fn far_lock(&self, held: &Lock, home: &crate::held::Home) -> Result<Option<Lock>, Stop> {
        let here = self.issues_path();
        let real = target_of(&here, Some(&self.root))?;
        if real == here {
            return Ok(None);
        }
        let dir = crate::path::dir_of(&real);
        let at = dir.join("lock");
        let linked = std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink());
        let far = match !linked && held.holds(&at).unwrap_or_else(|| crate::user_config::same_dir(dir, &self.dir())) {
            true => None,
            false => Some(Lock::inside(&at, home)?),
        };
        if std::iter::once(held).chain(&far).any(|l| l.holds(&real) == Some(true)) {
            return Err(Stop::Refused(Trouble::SnapshotOnLock { at: here, to: real }));
        }
        Ok(far)
    }

    /// 적을 줄을 **파일마다 나눠 담는다**(moai-nzlo). 한 판의 줄이 한 사람의 것이 아닐 수 있어
    /// (`--user` 를 섞어 부르는 고리) 갈래는 줄마다 본다 — 그래야 이름이 늘 그 줄의 임자에서 온다.
    fn append_journal(&self, filed: &[(String, Vec<&JournalEntry>)]) -> R<()> {
        let dir = self.journal_dir();
        std::fs::create_dir_all(&dir).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
        for (name, entries) in filed {
            let path = dir.join(name);
            let mut buf = String::new();
            for e in entries {
                buf.push_str(&serde_json::to_string(e).map_err(|e| Fail::new(e.to_string()))?);
                buf.push('\n');
            }
            // 링크는 체크아웃 안에서만 따라간다(moai-wd44) — 커밋된 링크 하나가 저널 줄을 `~/.bashrc` 에 싣던 자리다.
            append_inside(&path, buf.as_bytes(), &self.root)?;
        }
        // **자리도 적는다**(리뷰, [`write_atomic_in`] 과 같은 자). 첫 쓰기가 디렉터리와 파일을
        // 함께 새로 짓는데, `sync_all` 은 그 파일의 내용만 적고 **자리의 이름은 안 적는다** —
        // 전원이 나가면 스냅샷은 남고 저널 파일이 통째로 사라진다. 옛 한 파일은 `init` 이 지어
        // 커밋까지 된 이름이라 이 틈이 없었다.
        #[cfg(unix)]
        if let Ok(d) = std::fs::File::open(&dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    /// `moai show <id>` 의 이력 전용. **접지 않는다.**
    ///
    /// 이 함수가 `Vec<Issue>` 를 돌려주게 되는 날이 저널을 상태의 원천으로
    /// 삼기 시작한 날이고, 이전 시도가 거기서 복잡해졌다.
    pub fn journal_of(&self, id: &str) -> Vec<JournalEntry> {
        self.journal_by_id(&BTreeSet::from([id]), |_| true).remove(id).unwrap_or_default()
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
    /// **끊긴 꼬리에 붙어 한 줄이 된 줄은 줄 머리(`{"ts":"`)마다 갈라 온전한 쪽을 푼다**(리뷰). 새 바이너리는
    /// 꼬리를 채운 뒤에 덧붙이지만(moai-a65c) 옛 바이너리는 끊긴 줄 끝에 그대로 붙인다 — 붙은 쪽은 온전한
    /// 줄이라, 통째로 버리면 그 노트와 칸 옮김이 이력·`-g`·`work` 에서 말없이 사라진다([`pieces`]).
    ///
    /// **여전히 접지 않는다** — 돌려주는 것은 줄 그대로지 상태가 아니다.
    ///
    /// **못 읽는 *파일*은 넘어가되 [`journal_unread`] 에 선다**(2026-09-21 사용자 결정, moai-6ney).
    ///
    /// **그래서 이 함수는 지지 않는다**(moai-f2lc). 한때 꼴이 [`R`] 이었던 것은 못 읽는 파일
    /// 하나에 통째로 지던 때의 자취고, 관대해진 뒤로는 지는 길이 하나도 안 남았다 — 부르는
    /// 자리마다 `?` 가 영영 안 터질 자리에 서 있었고, 그러면 **나중에 정말 질 길이 생겨도
    /// 오늘과 구별되지 않는다.** 못 읽은 것은 값이 아니라 [`journal_unread`] 로 나간다.
    pub fn journal_by_id(
        &self,
        want: &BTreeSet<&str>,
        line: impl Fn(&str) -> bool,
    ) -> BTreeMap<String, Vec<JournalEntry>> {
        let mut out: BTreeMap<String, Vec<JournalEntry>> = BTreeMap::new();
        // **못 푼 조각은 안 묻는다**(`None`) — 이력에서 빠진 줄은 스냅샷이 여전히 그 이슈를 말한다. 저널이 답의
        // 전부인 [`Repo::journal_of_kind`] 만 묻는다(moai-g8ho).
        self.each_entry(
            line,
            |e| {
                if want.contains(e.id.as_str()) {
                    out.entry(e.id.clone()).or_default().push(e);
                }
            },
            None,
        );
        for v in out.values_mut() {
            in_ts_order(v);
        }
        out
    }

    /// 그 `kind` 의 줄 전부를 `ts` 차례로(moai-7dmq). 지운 id 는 부르는 쪽이 모르므로
    /// [`Repo::journal_by_id`] 처럼 id 로는 못 고른다 — `moai show --removed` 가 부른다.
    ///
    /// **읽는 규칙은 `journal_by_id` 와 한 벌이다**([`Repo::each_entry`]) — 관대함도, 파일 차례도,
    /// 못 읽은 파일을 [`journal_unread`] 에 세는 것도 같다. 따로 적으면 한쪽만 깨진 줄을 받아들이는
    /// 날이 온다.
    ///
    /// **여전히 접지 않는다.** 돌려주는 것은 적힌 줄 그대로다 — 그 id 가 지금 스냅샷에 되살아나
    /// 있는지는 묻지 않는다(2026-09-30 사용자 결정).
    ///
    /// 풀기 전에 `"<kind>"` 글자로 먼저 거른다. 저널의 거의 모든 줄이 `note`·`status` 라, 이 거르개 없이는
    /// 저널을 통째로 푼다(`journal_by_id` 의 `line` 과 같은 까닭이다). **`\u` 가 든 줄은 다 푼다**(리뷰) —
    /// 손으로 쓴 줄은 `kind` 의 값 글자를 `\u` 이스케이프(뒤에 16진 넷)로 적을 수 있고, 그 줄도 풀면 `rm` 이다
    /// (`model::may_hold_note` 와 같은 까닭). 그것을 거르면 `moai show <id>` 의 이력은 그 줄을 "삭제" 로
    /// 그리는데 `--removed` 에는 안 선다 — `JournalEntry::removes_issue` 가 막으려던 어긋남이다.
    ///
    /// **못 푼 조각은 자리째 곁에 낸다**(moai-g8ho). 저널을 관대하게 읽어도 되는 까닭은 "저널은 상태를 안
    /// 만든다" 였는데, `--removed` 에서는 저널이 답의 전부다 — 끊긴 `rm` 줄을 말없이 빼면 그 지움이 조용히
    /// 사라진다. 그 줄이 정말 `rm` 이었는지는 못 풀었으니 모른다: 낼 수 있는 것은 "지움을 들었을 수 있는
    /// 줄" 이고, 알리는 것은 부르는 쪽이 한다.
    ///
    /// **들었을 수 있는지는 조각이 제 입으로 댄 `kind` 로 가른다**([`may_be`], 리뷰). 앞 거르개는 풀 줄을
    /// 고르는 빠른 길일 뿐이다 — 그것으로 가르던 판은 `"rm"` 을 다 적기 전에 끊긴 줄을 말없이 뺐고, `\u` 가 든
    /// 끊긴 노트를 지움이라 불렀다. 그래서 앞 거르개를 못 지난 줄도 끊긴 꼴이면([`cut`]) 조각으로 본다.
    /// 조각의 도장도 읽어 둔다([`Garbled::ts`]) — `--since` 는 그 폭에 들었을 수 있는 조각만 댄다.
    pub fn journal_of_kind(&self, kind: &str) -> (Vec<JournalEntry>, Vec<Garbled>) {
        let quoted = format!("\"{kind}\"");
        let mut out = Vec::new();
        let mut garbled: Vec<Garbled> = Vec::new();
        self.each_entry(
            |l| l.contains(&quoted) || l.contains("\\u"),
            |e| {
                if e.kind == kind {
                    out.push(e);
                }
            },
            Some(&mut |piece: &str, at: &Path, line: usize| {
                if !may_be(kind, piece) {
                    return;
                }
                let ts = stamp_of(piece);
                match garbled.last_mut() {
                    // 한 줄의 두 조각은 한 자리로 댄다. 도장은 폭에 들 수 있는 쪽을 남긴다 — 못 읽었거나 늦은 쪽.
                    Some(g) if g.at == at && g.line == line => g.ts = g.ts.take().zip(ts).map(|(a, b)| a.max(b)),
                    _ => garbled.push(Garbled { at: at.to_path_buf(), line, ts }),
                }
            }),
        );
        in_ts_order(&mut out);
        (out, garbled)
    }

    /// 저널 파일을 다 걸으며 `line` 이 고른 줄을 풀어 `take` 에 건넨다 — [`Repo::journal_by_id`] 와
    /// [`Repo::journal_of_kind`] 가 지나는 한 걸음. 읽는 법의 까닭은 `journal_by_id` 에 적혀 있다.
    /// 차례는 읽은 차례(파일 차례, 파일 안의 줄 차례)고, `ts` 로 세우는 것은 부르는 쪽이 [`in_ts_order`] 로 한다.
    ///
    /// **못 푼 조각은 `torn` 에 건넨다**(moai-g8ho) — 조각의 글, 파일, 1부터 센 줄 번호다. 이력은 `None` 을
    /// 넘겨 묻지 않고(그러면 깨진 글자가 든 줄은 풀기 전에 건너뛴다), 저널이 답의 전부인
    /// [`Repo::journal_of_kind`] 가 묻는다. 깨진 글자는 `�` 로 바꾼 글로 고르고 가르기만 하지 풀지는 않는다.
    ///
    /// **`\n` 없이 끝난 줄이 조각을 내면 그 파일을 다시 읽는다**(리뷰). 읽기는 락을 안 잡으므로, 옆 세션이
    /// 덧붙이는 도중의 줄이 끝에 반쯤 보일 수 있다 — ext4 는 한 번의 덧붙이기도 쪽(4KiB)마다 드러내, 여러 쪽에
    /// 걸친 `rm` 의 끝이 조각으로 잡혀 멀쩡한 저널로 0 아닌 코드를 냈다. 다시 읽어 파일이 뒤로 자랐으면 그쪽을
    /// 믿고, 자라기를 멈췄는데도 반쯤이면(정말 끊겼다) 조각으로 댄다. 몇 번까지만 다시 읽는다 — 쉬지 않고
    /// 덧붙이는 쪽 곁에서도 읽기가 끝나야 한다.
    fn each_entry(
        &self,
        line: impl Fn(&str) -> bool,
        mut take: impl FnMut(JournalEntry),
        mut torn: Option<&mut dyn FnMut(&str, &Path, usize)>,
    ) {
        const REREADS: usize = 3;
        let asks = torn.is_some();
        for path in self.journal_files() {
            let Some(mut bytes) = self.journal_bytes(&path) else { continue };
            let mut read = scan(&bytes, &line, asks);
            for _ in 0..REREADS {
                if !read.open_tail {
                    break;
                }
                match self.journal_bytes(&path) {
                    Some(again) if again.len() > bytes.len() && again.starts_with(&bytes) => {
                        read = scan(&again, &line, asks);
                        bytes = again;
                    }
                    _ => break,
                }
            }
            read.entries.into_iter().for_each(&mut take);
            if let Some(t) = torn.as_deref_mut() {
                for (piece, n) in &read.torn {
                    t(piece, &path, *n);
                }
            }
        }
    }

    /// 저널 파일 하나를 통째로 읽는다. 없는 파일은 `None` 이다 — 옛 한 파일이 없는 저장소가 흔하다.
    ///
    /// **못 읽는 파일은 넘어가되 조용히는 아니다**(2026-09-21 사용자 결정, moai-6ney). 여기서 멈추던 때는
    /// 남의 파일 하나가 0600 으로 서는 것만으로 **제 파일에만** 이력이 있는 이슈까지 아무것도 안 보였다.
    /// 조용한 손실을 막는 일은 이제 [`note_unread`] 와 그것을 대는 `cmd::run` 이 한다 — 종료 코드도 0 이 아니다.
    ///
    /// **연 파일이 댄 크기까지만 읽는다**(moai-karj 리뷰). [`Repo::journal_files`] 는 이름으로 재므로, 잰 뒤
    /// 읽기 전에 체크아웃·머지가 그 이름을 procfs 로 가는 링크로 갈아끼우면 `fs::read` 가 크기 0 이라 답한
    /// 파일을 끝없이 읽는다. 연 손잡이의 `fstat` 은 그 틈이 없다 — 디스크의 보통 파일은 크기를 바로 대고,
    /// 덧붙는 중의 꼬리는 [`Repo::each_entry`] 가 다시 읽어 받는다. 읽는 자는 스냅샷·설정과 한 벌이다
    /// ([`crate::held::read`], moai-itsu) — 막히지 않게 열어, 그 틈에 FIFO 로 갈린 이름 앞에서도 안 멈춘다.
    fn journal_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        match crate::held::read(path) {
            Ok(b) => Some(b),
            Err(crate::held::Fell::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(crate::held::Fell::Io(e)) => {
                note_unread(&self.root, path, &e);
                None
            }
            Err(crate::held::Fell::Unheld(why)) => {
                note_missed(&self.root, path, Missed::Held(why));
                None
            }
        }
    }
}

/// 저널에서 못 푼 조각의 자리 — 파일과 1부터 센 줄 번호, 그리고 읽혔으면 그 조각의 도장
/// ([`Repo::journal_of_kind`], moai-g8ho).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Garbled {
    pub at: PathBuf,
    pub line: usize,
    /// 조각이 댄 `ts` — 값이 닫는 따옴표까지 온전히 읽힐 때만 선다([`stamp_of`]). `--removed --since` 가 이것으로
    /// 댈지를 가른다(`query::may_fall_in`): 도장이 폭 밖이면 온전했어도 그 목록에 안 섰을 줄이다.
    pub ts: Option<String>,
}

/// 파일 하나를 가른 것 — [`Repo::each_entry`] 가 파일마다 한 번(덜 적힌 꼬리면 다시 읽을 때마다 한 번 더) 짓는다.
#[derive(Default)]
struct Scan {
    entries: Vec<JournalEntry>,
    /// 못 푼 조각과 그 줄 번호(1부터). 묻는 쪽이 있을 때만 모은다.
    torn: Vec<(String, usize)>,
    /// `\n` 없이 끝난 마지막 줄이 조각을 냈다 — 옆 세션이 아직 적는 줄일 수 있다.
    open_tail: bool,
}

/// 파일 하나의 바이트를 줄로 가르고 푼다 — 읽는 법은 [`Repo::journal_by_id`] 에, 조각을 가르는 법은
/// [`Repo::journal_of_kind`] 에 적혀 있다. `asks` 가 거짓이면 조각을 안 모으고, 깨진 글자가 든 줄은 바꾼 글을
/// 짓기 전에 건너뛴다(리뷰) — 이력은 그 줄을 안 푸는데, 지어서 버리면 인코딩이 통째로 틀린 저널에서 읽기가
/// 몇 배로 는다.
fn scan(bytes: &[u8], line: &impl Fn(&str) -> bool, asks: bool) -> Scan {
    let bytes = bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes);
    let mut out = Scan::default();
    let mut last = 0;
    for (n, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        last = n;
        // `str::lines` 와 같은 줄이다 — `\n` 에서 가르고 끝의 `\r` 을 뗀다.
        let text = match std::str::from_utf8(raw) {
            Ok(l) => std::borrow::Cow::Borrowed(l),
            Err(_) if !asks => continue,
            Err(_) => String::from_utf8_lossy(raw),
        };
        let whole = matches!(text, std::borrow::Cow::Borrowed(_));
        let l = text.strip_suffix('\r').unwrap_or(&text);
        if l.trim().is_empty() {
            continue;
        }
        let picked = line(l);
        if picked
            && whole
            && let Ok(e) = serde_json::from_str::<JournalEntry>(l)
        {
            out.entries.push(e);
            continue;
        }
        if !picked && !(asks && cut(l)) {
            continue;
        }
        for piece in pieces(l) {
            match whole.then(|| serde_json::from_str::<JournalEntry>(piece).ok()).flatten() {
                // 앞 거르개가 고른 줄에서 푼 것만 건넨다 — 안 고른 줄의 조각은 부르는 쪽이 찾는 줄일 수 없다.
                Some(e) if picked => out.entries.push(e),
                Some(_) => {}
                None if asks => out.torn.push((piece.to_string(), n + 1)),
                None => {}
            }
        }
    }
    out.open_tail = !bytes.ends_with(b"\n") && out.torn.last().is_some_and(|(_, n)| *n == last + 1);
    out
}

/// 줄 머리 — 저널 줄은 `serde_json` 이 쓰고 `ts` 가 첫 필드라 모든 줄이 이 글자로 연다. 따옴표는 JSON 글
/// 안에서 늘 `\"` 로 적히므로 이 글자는 값 안에 못 선다 — 줄 가운데에 서면 거기서 다른 줄이 붙은 것이다.
const HEAD: &str = "{\"ts\":\"";

/// 끊긴 줄의 꼴인가 — 앞 거르개를 못 지난 줄도 조각인지 여기서 가른다([`Repo::journal_of_kind`]). `{` 로 열었는데
/// `}` 로 안 닫혔거나, 제 `kind` 를 다 적기 전에 다른 줄이 붙었다: 첫 `kind` 열쇠 앞에 줄 머리([`HEAD`])가 또
/// 섰거나, 그 값에 `{` 가 들었다(`"kind":"r{"ts":"…`).
///
/// **첫 `kind` 에서 멈춘다**(리뷰) — `kind` 를 다 적은 뒤에 끊긴 앞 토막은 그 값으로 이미 갈리고(다른 갈래면
/// 지움이 아니다), 붙은 뒤 토막이 찾는 갈래면 줄째 앞 거르개를 지났다. 줄 끝까지 훑으면 거르개를 못 지난 모든
/// 줄을 한 번 더 읽어 `--removed` 가 눈에 띄게 느려졌다. `}` 로 닫혔는데 `kind` 열쇠가 아예 없는 줄은 붙은 저널
/// 줄이 아니다 — 붙은 뒤 토막은 온전한 줄이라 늘 그 열쇠를 든다.
fn cut(l: &str) -> bool {
    const KIND: &str = "\"kind\":\"";
    let t = l.trim();
    if !t.starts_with('{') {
        return false;
    }
    if !t.ends_with('}') {
        return true;
    }
    t.find(KIND).is_some_and(|k| {
        t[1..k].contains(HEAD) || t[k + KIND.len()..].split('"').next().is_some_and(|v| v.contains('{'))
    })
}

/// 한 줄을 줄 머리([`HEAD`])마다 가른다 — 끊긴 꼬리에 옛 바이너리가 붙여 쓴 줄은 한 줄에 둘이 든다(moai-a65c
/// 전의 바이너리). 머리가 맨 앞에만 서면 줄 그대로 하나다.
fn pieces(l: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = l;
    loop {
        let skip = rest.chars().next().map_or(0, char::len_utf8);
        match rest[skip..].find(HEAD) {
            Some(i) => {
                out.push(&rest[..skip + i]);
                rest = &rest[skip + i..];
            }
            None => {
                out.push(rest);
                return out;
            }
        }
    }
}

/// 못 푼 조각이 `kind` 의 줄이었을 수 있는가 — 조각이 **제 입으로 댄 `kind`** 로 가른다(리뷰). 값이 온전히
/// 읽히고 다른 갈래면 아니다. 값 가운데서 잘렸으면 그 토막으로 시작하는 갈래만, 열쇠에 못 닿았으면 무엇이든
/// 될 수 있다. 값에 역슬래시가 들면 풀지 않고 "그럴 수 있다" 로 친다 — 손으로 쓴 줄은 값 글자를 `\u` 로 적을 수
/// 있다(`model::may_hold_note` 와 같은 까닭).
fn may_be(kind: &str, piece: &str) -> bool {
    const KEY: &str = "\"kind\":\"";
    let Some(at) = piece.find(KEY) else { return true };
    let value = &piece[at + KEY.len()..];
    match value.find('"') {
        Some(end) => value[..end].contains('\\') || &value[..end] == kind,
        None => value.contains('\\') || kind.starts_with(value),
    }
}

/// 조각의 도장 — `"ts":"…"` 의 값이 닫는 따옴표까지 읽힐 때만. 잘렸거나 이스케이프가 들면 `None` 이다.
fn stamp_of(piece: &str) -> Option<String> {
    const KEY: &str = "\"ts\":\"";
    let value = &piece[piece.find(KEY)? + KEY.len()..];
    let stamp = &value[..value.find('"')?];
    (!stamp.contains('\\')).then(|| stamp.to_string())
}

/// 저널 줄을 `ts` 차례로 세운다 — **안정 정렬이다**: 같은 `ts` 를 든 줄은 읽은 차례 그대로 남고, 그 차례를
/// 세우는 자가 [`Repo::journal_files`] 다. [`Repo::journal_by_id`] 와 [`Repo::journal_of_kind`] 가 이 한 자로
/// 세운다(리뷰) — 둘로 적으면 한쪽 차례만 바뀌어도 한 이슈의 이력과 `show --removed` 가 같은 줄들을 다른
/// 차례로 낸다.
fn in_ts_order(v: &mut [JournalEntry]) {
    v.sort_by(|a, b| a.ts.cmp(&b.ts));
}

/// 메일에서 저널 파일 이름을 짓는다 — `raven@buzzni.com` 이면 `raven_buzzni_com.jsonl`(moai-nzlo).
///
/// **이름이 아니라 메일로 짓는다**(2026-09-21 사용자 결정). 이름에는 한글과 빈칸이 들고, 메일은
/// ASCII 로만 서면서 사람이 읽을 수 있으며, 줄마다 `by_email` 이 이미 있다.
///
/// `@` 와 `.` 를 `_` 로, ASCII 낱말·숫자·`-`·`_` 밖의 글자도 `_` 로 접는다. **큰 글자는 내린다** —
/// 접지 않으면 글자 크기를 안 가리는 파일시스템(macOS·Windows)에서만 두 메일이 한 파일로 합쳐져,
/// 같은 저장소가 기계마다 다른 수의 파일을 갖는다. 접히더라도 줄마다 `by_email` 이 있어 읽는
/// 쪽은 여전히 가른다.
///
/// 글자 하나가 글자 하나로 가므로 결과는 빈 이름도, `.`·`..` 도, 경로 조각도 될 수 없다 —
/// 이름을 디렉터리에 이어 붙이는 자리가 여기 하나라 그 보장이 여기서 선다.
///
/// **길이도 여기서 자른다**(리뷰). 접은 글자는 다 ASCII 한 바이트라 글자 수가 곧 바이트 수고,
/// 파일 이름의 상한(`NAME_MAX`, 흔히 255)을 넘기면 `open` 이 ENAMETOOLONG 으로 진다 — 그 실패는
/// **스냅샷을 쓴 뒤에** 나서 "썼지만 이력은 못 남겼다" 로 떨어지고, 그 사람의 이력은 그 뒤로도
/// 영영 안 남는다. 잘린 이름이 겹쳐도 줄마다 `by_email` 이 있어 읽는 쪽은 가른다.
pub fn journal_file(email: &str) -> Option<String> {
    /// `.jsonl` 여섯 자를 붙일 자리를 남긴 상한. 실제 메일이 닿을 수 있는 길이가 아니다.
    const CAP: usize = 200;
    let email = email.trim();
    if email.is_empty() {
        return None;
    }
    let name: String = email
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '_',
        })
        .take(CAP)
        .collect();
    Some(format!("{name}.jsonl"))
}

/// 적을 줄을 갈 파일마다 모은다 — **스냅샷을 쓰기 전에** 부른다(moai-nzlo).
///
/// 메일이 없는 줄이 하나라도 있으면 아무것도 안 쓰고 멈춘다. `unknown.jsonl` 도, 이름으로 지은
/// 파일도 두지 않는다(2026-09-21 사용자 결정) — 이력이 남는 것이 목적인 파일에 주인 없는 줄을
/// 채우느니 한 번 물어보는 편이 싸다. 게이트가 아닌 까닭은 `--user` 와 `MOAI_ACTOR` 둘 다 사람
/// 없이 채워지기 때문이고, 쓰지 않는 `status`·`ready`·`show` 는 여기를 안 지난다.
///
/// **[`crate::model::Actor`] 가 이미 막는다** — 메일 없는 사람으로는 만들어지지도 않으므로
/// (`Actor::is_sane`) 이 거절은 손으로 지은 줄에만 선다. 그래도 두는 것은 파일 이름을 정하는
/// 자리가 여기 하나여서다: 여기서 안 막으면 그 줄이 갈 곳이 없다.
fn file_entries(entries: &[JournalEntry]) -> Result<Vec<(String, Vec<&JournalEntry>)>, Trouble> {
    // **메일로 모으고 이름은 갈래마다 한 번 짓는다** — 한 판의 줄은 거의 다 한 사람의 것이라,
    // 줄마다 접으면 같은 주소를 줄 수만큼 다시 접어 버린다. 모으는 꼴은 바로 위
    // [`Repo::journal_by_id`] 와 같은 `BTreeMap` 이다(리뷰).
    let mut by_email: BTreeMap<&str, Vec<&JournalEntry>> = BTreeMap::new();
    for e in entries {
        let who = e.by_email.as_deref().map(str::trim).filter(|m| !m.is_empty());
        let Some(who) = who else { return Err(Trouble::NoJournalEmail { id: e.id.clone() }) };
        by_email.entry(who).or_default().push(e);
    }
    by_email
        .into_iter()
        .map(|(who, v)| match journal_file(who) {
            Some(name) => Ok((name, v)),
            // 다듬고도 이름이 안 서는 주소 — 위에서 빈 것은 이미 걸렀으니 여기 오지 않는다.
            None => Err(Trouble::NoJournalEmail { id: v[0].id.clone() }),
        })
        .collect()
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

/// 못 읽어 건너뛴 저널 자리 하나 — 어느 저장소의 어느 파일을 왜 못 읽었나.
///
/// **[`Unread::kind`] 는 기계의 것이고 [`Unread::said`] 는 사람의 것이다**([`crate::git::Told`] 와 같은 가름,
/// moai-f2lc). 받는 쪽이 글을 부분 문자열로 맞추면 그 줄은 기계마다·말마다 다르게 읽힌다. 가르는 자는 `kind` 다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    /// 그 저널이 사는 저장소의 뿌리. `main` 이 제 것과 옆 워크트리의 것을 이것으로 가른다.
    pub root: PathBuf,
    /// 못 읽은 자리 — 파일이거나, 훑지 못한 `journal/` 디렉터리다. **고치는 법이 곧 이 자리에
    /// 대는 `chmod`** 나 이 링크를 고치는 것이라, 줄이지 않고 통째로 든다.
    pub at: PathBuf,
    /// 왜 못 읽었나 — 갈래와 글은 이것에서 읽는다([`Unread::kind`]·[`Unread::said`]).
    pub why: Missed,
}

/// 저널 자리를 못 읽은 까닭 — **io 가 진 것과 안 읽기로 한 것을 가른다**(moai-itsu).
///
/// 한때는 둘을 한 꼴(`kind`·`said`)에 담아, 체크아웃 밖을 가리키는 링크가 `failed` 로 서고 `said` 에 moai 가
/// 지은 영어 문장이 들었다. 기계는 "그 링크를 걷어라" 를 잠깐의 io 실패와 못 갈랐고, `MOAI_LANG=ko` 에서는
/// 한국어 머리 뒤에 영어 설명이 붙었다(리뷰 moai-karj.8zm 4·5번).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missed {
    /// io 가 진 것 — `permission`·`failed` 와 운영체제가 낸 말. **말은 안 옮긴다**(`LANG` 과 libc 의 것이다).
    Io { kind: &'static str, said: String },
    /// 안 읽기로 한 자리([`crate::held::Unheld`]) — 글은 받는 쪽이 고른 말로 짓는다.
    Held(crate::held::Unheld),
}

impl Unread {
    /// `permission`·`failed`·`outside`. 늘어날 수 있으므로 받는 쪽은 모르는 값을 `failed` 처럼 다룬다 —
    /// 고칠 길이 하나로 정해진 갈래를 가르는 것이 이 값의 일이다. `permission` 은 `chmod` 한 줄이고,
    /// `outside` 는 체크아웃 밖·`.git/` 으로 가는 링크라 그 링크를 고친다. 보통 파일이 아닌 자리는 그 둘
    /// 어느 쪽으로도 안 풀려 `failed` 다.
    pub fn kind(&self) -> &'static str {
        match &self.why {
            Missed::Io { kind, .. } => kind,
            Missed::Held(crate::held::Unheld::NotAFile) => "failed",
            // 링크를 고칠 일이다. `Link` 는 락만 내고 저널은 안 낸다([`crate::held::lock`]).
            Missed::Held(
                crate::held::Unheld::Outside { .. } | crate::held::Unheld::IntoGit { .. } | crate::held::Unheld::Link,
            ) => "outside",
        }
    }

    /// 사람이 읽을 까닭 한 토막 — io 가 낸 말은 그대로, 안 읽기로 한 것은 고른 말로. 무엇을 못 했는지는
    /// 말묶음의 `warn.unread_journal` 이 앞에 붙인다.
    pub fn said(&self, lang: crate::i18n::Lang) -> String {
        match &self.why {
            Missed::Io { said, .. } => said.clone(),
            Missed::Held(why) => crate::held::said(lang, why),
        }
    }
}

/// 못 읽어 건너뛴 저널 자리, 만난 차례대로([`Unread`]).
///
/// **관대하되 시끄럽게**(2026-09-21 사용자 결정, moai-6ney). 저널이 사람마다 갈린 뒤로 파일은
/// N 개고, 한 체크아웃을 두 계정이 쓰면 남의 `<메일>.jsonl` 이 0600 으로 서는 일이 흔하다.
/// 그 하나에 [`Repo::journal_by_id`] 가 통째로 지면 **제 파일에만** 이력이 있는 이슈까지
/// 아무것도 안 보이고, 되돌릴 길은 도구 밖의 `chmod` 뿐이다 — 읽기는 관대하고 쓰기는 엄하다는
/// 규약이 막는 자리다.
///
/// 관대해진 읽기가 **조용해지지 않도록** 세는 자가 이것이다. [`MISSED`]·[`TALLY`] 와 같은 꼴이고
/// 같은 까닭이다: `store` 는 터미널을 모르므로 세어 두기만 하고, 말하는 자리는 `cmd::run` 하나다.
/// 그 자리가 stderr 로 대고 `cmd::note_partial` 로 종료 코드까지 0 이 아니게 한다.
///
/// **명령 하나가 아니라 프로세스 하나의 것이다.** `moai show` 는 목록과 상세에서 저널을 두 번
/// 읽으므로 같은 자리를 두 번 만난다 — [`note_unread`] 가 같은 짝을 두 번 안 담는다.
///
/// **뿌리를 곁에 든다**([`MISSED`] 와 같은 꼴, 리뷰). `show --worktree` 는 겹쳐 온 줄의 이력을
/// **그 워크트리의** 저널에서 읽으므로(`cmd::show::home`), 이 목록에는 남의 체크아웃의 자리가
/// 섞인다. 뿌리가 없으면 `main` 이 그것을 못 갈라, 옆 워크트리의 0600 파일 하나로 제 파일은
/// 멀쩡한 판이 비영 종료했다 — `cmd::gather` 가 옆 워크트리의 깨진 줄에 대해 못박은 것과
/// 같은 금이다("옆 워크트리의 깨진 줄로 `moai status` 가 비영 종료하면 제 파일은 멀쩡한데
/// 도구가 실패로 읽힌다"). 말하는 것은 둘 다 하고, 종료 코드를 바꾸는 것은 제 뿌리뿐이다.
///
/// **걷는 자가 없다** — 프로세스 하나 안에서는 늘기만 한다. 탐색기 머리의 줄은 고치면 걷히는 [`UNREAD_NOW`] 를
/// 읽는다(moai-mkyg.ncj).
static UNREAD: std::sync::Mutex<Vec<Unread>> = std::sync::Mutex::new(Vec::new());

pub fn journal_unread() -> Vec<Unread> {
    UNREAD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// **지금** 못 읽는 저널 자리 — [`note_missed`] 가 [`UNREAD`] 와 함께 담되, 탐색기가 다시 재어 이제 읽히는 자리를
/// 걷는다([`reopen_unread`], moai-mkyg.ncj). 걷혔다가 다시 진 자리는 여기서만 끝에 다시 붙는다. 탐색기 머리의 줄과
/// 낮은 창의 알림 띠가 읽는다.
///
/// **둘로 가른 까닭**(리뷰 moai-mkyg.n60) — 나갈 때의 말(`main` 의 `unread_journals`)은 걷히면 안 된다. 탐색기는 다시
/// 그리기 전에 모자란 답(통계 창의 토큰 합, `/` 의 노트 찾기)을 이미 보였고, 낮은 창의 알림 띠는 다음 키에 걷힌다 —
/// [`MISSED`] 를 비우지 않는 것과 같은 까닭이다. 한 목록을 걷던 판은 고친 뒤에 나간 `moai tui` 가 그 자리를 안 대고
/// 0 으로 끝났다. CLI 의 명령은 걷지 않으므로 둘이 늘 같다.
static UNREAD_NOW: std::sync::Mutex<Vec<Unread>> = std::sync::Mutex::new(Vec::new());

/// [`UNREAD_NOW`] — 탐색기 머리의 줄이 읽는다.
pub fn journal_unread_now() -> Vec<Unread> {
    UNREAD_NOW.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 못 읽은 자리를 센다. **같은 짝은 한 번만 선다** — 한 명령이 저널을 여러 번 읽어도 사람은
/// 같은 줄을 두 번 볼 까닭이 없다.
fn note_unread(root: &Path, at: &Path, err: &std::io::Error) {
    note_missed(root, at, Missed::Io { kind: unread_kind(err), said: err.to_string() });
}

/// [`note_unread`] 의 몸통 — 안 읽기로 한 자리([`crate::held::Unheld`])도 여기로 든다.
fn note_missed(root: &Path, at: &Path, why: Missed) {
    let one = Unread { root: root.to_path_buf(), at: at.to_path_buf(), why };
    CAUGHT.with_borrow_mut(|c| {
        if let Some(c) = c {
            c.push(one.clone());
        }
    });
    for list in [&UNREAD_NOW, &UNREAD] {
        let mut v = list.lock().unwrap_or_else(|e| e.into_inner());
        if !v.contains(&one) {
            v.push(one.clone());
        }
    }
}

thread_local! {
    /// [`reopen_unread`] 가 다시 읽는 동안 **이 스레드가** 센 자리. [`UNREAD_NOW`] 는 같은 짝을 두 번 안 담아, 거기서는
    /// 아직 못 읽는 자리와 이제 읽히는 자리가 안 갈린다. 스레드의 것이라 옆 스레드의 읽기(탐색기의 다시 읽기)가
    /// 센 것은 안 섞인다.
    static CAUGHT: std::cell::RefCell<Option<Vec<Unread>>> = const { std::cell::RefCell::new(None) };
}

/// **못 읽었던 저널 자리를 다시 재어, 이제 읽히는 자리를 [`UNREAD_NOW`] 에서 걷는다**(moai-mkyg.ncj). 걷은 수를 낸다.
///
/// 머리의 줄이 늘기만 하는 [`UNREAD`] 를 읽던 때는 탐색기 머리의 `Journal` 줄이 `chmod 644` 뒤에도 나갈 때까지
/// 섰다(리뷰 moai-pvpb.rrr 7번). 부르는 자리는 탐색기 하나다 — **CLI 는 안 부른다.** 명령 하나는 앞에서 못 읽은 채
/// 이미 내보냈으니, 뒤에 읽힌다고 걷으면 모자란 답이 0 으로 끝난다(조용한 손실). 탐색기는 화면을 다시 그리므로
/// 머리의 줄은 "지금 못 읽는 것" 을 대는 것이 맞는 말이다. 나갈 때의 말은 이것이 안 걷는 [`UNREAD`] 가 댄다 — 다시
/// 그리기 전에 모자란 답을 이미 보였다.
///
/// **읽는 자는 저널을 읽는 그 걸음이다** — [`Repo::journal_files`] 가 디렉터리·이름·링크를 다시 재고, 파일은
/// [`Repo::journal_bytes`] 가 연다. 판정을 여기 따로 적으면 두 자가 갈려, 읽는 쪽은 넘기는 자리를 여기는 걷거나
/// 그 반대가 된다. **여는 파일은 목록에 선 자리로 풀리는 것뿐이다** — 그 걸음이 열려다 졌거나 이름을 못 재어 못 연
/// 자리다. 못 재었던 이름은 고치면 여기서 처음 열고, 끊겼던 링크가 이미 읽힌 파일로 이어지면 그 파일을 한 번 더
/// 연다. 목록에 디렉터리만 섰으면 그 안의 파일은 다음 읽기가 잰다.
///
/// - **견주는 것은 푼 자리다**(리뷰 moai-mkyg.n60) — [`Repo::journal_files`] 는 링크 이름을 푼 자리로, 보통 이름은
///   받은 철자로 내고 같은 파일을 처음 만난 이름으로 접는다. 철자로 견주던 판은 뿌리가 링크 너머에서 불릴 때 접던
///   링크가 지워지거나, 끊겼던 링크가 못 읽는 파일로 이어지면 그 파일을 안 열고 고친 것으로 걷었다
/// - 걷는 것은 **부를 때 섰던 줄 가운데** 이번에 안 센 것뿐이다. 그 사이 옆 스레드가 새로 센 줄은 이 걸음이
///   못 본 것이라 둔다
/// - 아직 못 읽는 줄은 제자리에 남는다 — 차례가 그대로라 머리의 줄이 대는 첫 자리가 잴 때마다 바뀌지 않는다.
///   이미 말한 줄인지는 받는 쪽이 줄째 견준다(`tui::App::tell_journals`). 이번에 새로 진 자리는 [`note_missed`]
///   가 늘 하듯 끝에 붙는다 — 철자만 바뀐 자리도 새 철자로 끝에 붙고, 옛 철자는 걷은 수에 든다
///
/// **루프에서 돈다** — 하는 일은 못 읽는 자리마다 1분에 한 번 이름을 재고 여는 것이고, 고친 파일은 한 번 통째로
/// 읽는다(풀지는 않는다). 여는 길이 [`crate::held::read`] 라 남이 리스를 쥔 파일 앞에서는 그것이 걷힐 때까지
/// 기다린다 — 노트를 거는 걸음(`tui::Ground::read_notes`)과 통계 창이 루프에서 저널을 읽는 것과 같은 몫이다.
///
/// **설정은 부르는 쪽 것을 빌린다** — 저널 읽기는 설정을 안 쓰지만 [`Repo`] 를 짓는 문이 받는다. 뿌리마다 새로
/// 읽으면 이 프로세스가 연 적 없는 옆 워크트리의 `config.toml` 을 열고, 그것을 못 읽는 뿌리는 잴 수도 없다.
/// 노트를 옆 뿌리의 저널에서 읽는 길(`cmd::show::at_home`)이 이미 그렇게 빌린다.
pub fn reopen_unread(config: &Config) -> usize {
    reopen_picked(config, |_| true)
}

/// [`reopen_unread`] 의 몸통 — `pick` 이 고른 줄만 다시 잰다. 고르는 것은 시험이다: [`UNREAD_NOW`] 는 프로세스 하나의
/// 것이라, 다 재면 나란히 도는 시험이 심은 자리까지 걷는다.
fn reopen_picked(config: &Config, pick: impl Fn(&Unread) -> bool) -> usize {
    let had: Vec<Unread> = journal_unread_now().into_iter().filter(|u| pick(u)).collect();
    if had.is_empty() {
        return 0;
    }
    let roots: BTreeSet<&Path> = had.iter().map(|u| u.root.as_path()).collect();
    let tried: BTreeSet<PathBuf> = had.iter().map(|u| crate::path::real(&u.at)).collect();
    CAUGHT.set(Some(Vec::new()));
    for root in &roots {
        let repo = Repo::at(root.to_path_buf(), config.clone());
        for p in repo.journal_files() {
            if tried.contains(&crate::path::real(&p)) {
                let _ = repo.journal_bytes(&p);
            }
        }
    }
    let caught = CAUGHT.take().unwrap_or_default();
    let mut v = UNREAD_NOW.lock().unwrap_or_else(|e| e.into_inner());
    let before = v.len();
    v.retain(|u| !had.contains(u) || caught.contains(u));
    before - v.len()
}

/// io 의 실패를 [`Unread::kind`] 로 접는다.
///
/// **가르는 것은 고칠 수 있는가다.** `permission` 하나만 따로 세우는 까닭은 그것이 `chmod` 한
/// 줄로 풀리는 유일한 갈래여서고, 나머지는 받는 쪽이 할 일이 같다. 없는 저널은 고장이 아니라
/// [`Repo::journal_files`] 와 [`Repo::journal_by_id`] 가 그냥 넘기므로 `NotFound` 는 거의 안
/// 온다 — `journal/` 안의 끊긴 심볼릭 링크가 [`Repo::journal_names`] 의 `metadata` 에서 하나
/// 들어오는데, 그것도 `chmod` 로는 안 풀리니 `failed` 가 제자리다.
fn unread_kind(err: &std::io::Error) -> &'static str {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => "permission",
        _ => "failed",
    }
}

/// 파일이 그때 그것인지 가늠하는 표식. 고친 때만 보면 놓친다 — rename 으로
/// 갈아끼우는 쓰기는 같은 초에 떨어질 수 있어 길이도 함께 본다. 파일이 없으면 `None`.
pub type Stamp = Option<(std::time::SystemTime, u64)>;

/// 파일 하나의 자리 — 장치와 inode(moai-itsu.aod). 이름이 둘이어도(하드 링크, 같은 파일로 가는 링크) 같은
/// 파일이면 같은 값이다. 유닉스 밖에서는 안 선다 — 그때는 푼 자리로 접고([`Repo::journal_files`]), 락은 견줄
/// 길이 없다고 답한다([`Lock::holds`]).
type FileId = Option<(u64, u64)>;

fn file_id(m: &std::fs::Metadata) -> FileId {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        None
    }
}

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
///
/// **받는 것은 그 트래커의 뿌리(`.moai` 를 든 디렉터리)다** — 읽을 자리(`<뿌리>/.moai/issues.jsonl`)와 견줄
/// 체크아웃이 한 값에서 나온다. 둘을 따로 받던 판은 부르는 자리마다 짝을 손으로 맞췄다(리뷰 moai-itsu.n8z).
///
/// **그 체크아웃 안에서만, 연 손잡이가 댄 크기까지만 읽는다**([`crate::held`], moai-itsu). 받은 저장소가
/// `issues.jsonl -> /proc/self/pagemap` 을 커밋해 두면 `fs::read_to_string` 이 끝없이 읽어 모든 명령이 메모리를
/// 다 썼고, FIFO 면 모든 명령이 멈췄다. 안 읽는 자리는 **자료로 낸다**([`Unsnapped::Held`]) — 여기는 화면
/// 말을 모른다. 옆 워크트리의 것은 `worktree::Trouble::Unheld` 로 실려 고른 말로 펴지고, 제 트래커를 지은
/// 뒤에 갈린 것은 [`Repo::read`] 가 말 없는 꼴로 멈춘다([`Unsnapped::into_fail`]).
pub fn read_snapshot(root: &Path) -> Result<Option<Load>, Unsnapped> {
    let path = root.join(".moai").join("issues.jsonl");
    match crate::held::read_inside(&path, &crate::held::Home::of(root)) {
        Ok(s) => Ok(Some(parse_issues(&s))),
        Err(crate::held::Fell::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(crate::held::Fell::Io(e)) => Err(Unsnapped::Failed(Fail::new(format!("{}: {e}", path.display())))),
        Err(crate::held::Fell::Unheld(why)) => Err(Unsnapped::Held { at: path, why }),
    }
}

/// [`read_snapshot`] 이 못 읽은 까닭 — **안 읽기로 한 것은 자료로 든다**(리뷰 moai-itsu.n8z). 글로 지어 내던
/// 판은 옆 워크트리의 줄이 한국어 머리 뒤에 moai 가 지은 영어(`not a regular file`)로 끝났고, 밖 링크는 까닭
/// 없이 `<자리> -> <끝>` 만 댔다.
#[derive(Debug)]
pub enum Unsnapped {
    /// 안 읽기로 한 자리([`crate::held::Unheld`]) — 그 스냅샷의 자리와 까닭. 글은 받는 쪽이 고른 말로 편다.
    Held { at: PathBuf, why: crate::held::Unheld },
    /// io 가 진 것 — 이미 글이다(자리와 운영체제의 말).
    Failed(Fail),
}

impl Unsnapped {
    /// 말을 모르는 자리의 꼴 — 안 읽기로 한 것은 [`crate::held::spelled`] 와 `broken` 이다([`Repo::read`]).
    pub fn into_fail(self) -> Fail {
        match self {
            Unsnapped::Held { at, why } => Fail::coded(crate::held::spelled(&at, &why), code::BROKEN),
            Unsnapped::Failed(e) => e,
        }
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
                // **id 를 읽는 자는 하나다**(moai-ijfy). `crate::id::id_of` 가
                // `serde_json` 을 한 겹 아래로 부르고, 그것도 진 줄은 머리에서
                // 긁는다. 여기서 따로 읽던 동안 머지 드라이버만 그 머리를 보아,
                // 산 줄의 깨진 쌍둥이가 든 id 를 [`Load::reserved_ids`] 가 안
                // 잡아 두고 `report` 의 `duplicate_id` 도 못 댔다 — `moai status`
                // 는 `Unreadable rows` 만 말했다.
                id: crate::id::id_of(line),
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
    // **검사는 여기서 다시 안 건다**(moai-yve0). 한때 이 자리에도 걸었다 — 거절이 쓰기를 통째로
    // 물리므로 방금 뽑은 id 는 어디에도 안 남는데 검사의 말이 `<id>: ` 로 시작해, `add --from` 에
    // `#bug,perf` 한 줄을 준 부름이 없는 id 를 댔다(moai-1rkl). 이제 가리키는 말을 고르는 자가
    // [`Repo::write_locked`] 하나고, 그쪽도 **아직 없던 줄이면 제목으로** 가리킨다 — 같은 말이
    // 한 자리에서 나온다. 여기서 또 걸면 그 자리는 화면 말을 모르는 채 글을 지어야 한다.
    let entry = JournalEntry::create(&issue.id, &issue.title, &issue.created_at, by);
    issues.push(issue.clone());
    Ok((entry, issue))
}

/// `<파일>` 을 쓸 때 쓸 임시 파일의 이름 — `<파일>.tmp.<pid>.<스레드 번호>` (moai-mpf4).
///
/// **pid 하나로는 모자란다.** [`crate::latest::spawn`] 이 이 저장소의 첫 "본 스레드 밖 쓰기" 라,
/// 한 프로세스의 스레드 둘이 같은 파일을 쓰면 임시 경로가 **같은 이름 하나**로 겹쳤다 — 한쪽의
/// `rename` 이 다른 쪽이 아직 쓰는 중인 파일을 들고 가거나, 먼저 간 쪽의 파일을 뒤엣것이 덮어
/// 반쪽짜리 글이 대상에 실릴 수 있었다.
///
/// **번호는 스레드마다 한 번 매기고 그 스레드가 사는 동안 안 바뀐다.** 부를 때마다 세는 쪽이 더
/// 쉽지만, 그러면 같은 스레드의 두 번째 쓰기가 다른 이름을 써 **이름을 미리 아는 길이 없어진다** — 임시
/// 자리를 막아 두고 그리로 갔는지 재는 시험(`cmd::init` 의
/// `root_files_are_swapped_through_a_temp_file_in_dot_moai`)이 그 길로 선다. 쓰다 죽어 남은
/// 찌꺼기는 다음 쓰기가 이어 쓰지 않는다([`open_tmp`]) — 죽은 쓰기마다 하나씩 남는다.
///
/// 번호를 매기는 자는 프로세스 안에서만 선다 — 프로세스가 다르면 pid 가 가른다. **pid 가 못 가르는
/// 자리는 pid 네임스페이스가 다른 두 프로세스다**(같은 마운트를 쓰는 컨테이너 둘). 그래도 `flock` 을
/// 쥐고 쓰는 파일(`issues.jsonl`·사용자 설정·읽음 표시)은 한 커널 안에서 이미 차례를 서므로, 겹침이
/// 실제로 서는 곳은 락 없이 쓰는 파일(`latest.toml`, `init` 이 심는 `AGENTS.md`)과 `flock` 이 건너가지
/// 못하는 공유 파일시스템이다. 그 겹침은 이름이 아니라 여는 자가 막는다([`open_tmp`] 의 `create_new`).
/// **돌아온 pid 는 겹침이 아니다** — pid 는 앞 주인이 거둬진 뒤에야 돌아오므로, 그 이름에 남은 파일은
/// 죽은 쓰기의 찌꺼기다.
pub(crate) fn tmp_name(file: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    thread_local! {
        static MARK: u64 = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    MARK.with(|mark| format!("{file}.tmp.{}.{mark}", std::process::id()))
}

/// [`open_tmp`] 가 차례로 여는 임시 이름 — 첫째는 [`tmp_name`] 그대로고, 그 이름에 **남의
/// 파일이 이미 있으면** 뒤에 `.<이 프로세스의 수>.1`·`.2`… 를 붙여 다음으로 간다(moai-ydm7.976).
///
/// **첫째를 [`tmp_name`] 으로 둔다** — 겹침이 없는 보통 경우에 이름이 예전과 같아야, 쓰다 죽어
/// 남는 찌꺼기의 꼴도 이름을 미리 아는 시험도 그대로 선다. 수를 [`TMP_TRIES`] 로 묶는 것은
/// 막힌 디렉터리에서 끝없이 돌지 않기 위해서다.
///
/// **둘째부터는 이 프로세스만의 수([`process_nonce`])를 넣는다.** 첫 이름에 남은 찌꺼기는 아무도
/// 안 지우는데([`open_tmp`]), pid 가 늘 같은 자리(컨테이너의 PID 1)에서는 쓰다 죽을 때마다 같은
/// 이름이 하나씩 막힌다. 뒤의 이름까지 pid·번호만으로 지으면 그런 찌꺼기 여덟 개가 그 pid 의
/// 쓰기를 영영 막고, 되돌릴 길은 사람이 손으로 지우는 것뿐이다 — [`Lock`] 이 `O_EXCL` 락 파일을
/// 안 쓰는 그 까닭이다.
pub(crate) fn tmp_names(file: &str) -> impl Iterator<Item = String> + use<> {
    let first = tmp_name(file);
    let nonce = process_nonce();
    (0..TMP_TRIES).map(move |n| match n {
        0 => first.clone(),
        n => format!("{first}.{nonce:016x}.{n}"),
    })
}

/// [`tmp_names`] 가 내는 이름 수. 다 막혔으면 쓰기는 실패하고 대상은 안 바뀐다 — 첫 이름 말고는
/// 이 프로세스만의 이름이라, 다 막혔다면 겹침도 찌꺼기도 아닌 다른 무엇이 어긋난 것이다.
pub(crate) const TMP_TRIES: usize = 8;

/// 이 프로세스만의 수 — 처음 부를 때 한 번 뽑고 프로세스가 사는 동안 그대로다([`tmp_names`]).
///
/// 뽑는 자는 표준 라이브러리의 `RandomState` 다 — 프로세스마다 운영체제의 난수로 시작하므로,
/// pid 가 같은 두 프로세스도 다른 수를 받는다. 한 프로세스 안에서는 늘 같아야 임시 이름을 미리
/// 아는 시험이 선다.
fn process_nonce() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    static NONCE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *NONCE.get_or_init(|| std::collections::hash_map::RandomState::new().build_hasher().finish())
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
///
/// **뿌리 없는 경로 하나는 옮기지 않는다**(moai-iq7j). `path.parent()` 가 없는 것은 사람이 밟는
/// 자리가 아니라 부르는 쪽의 실수이고(뿌리 `/` 나 빈 경로), 이 함수는 머지 드라이버도 부른다 —
/// 화면 말을 물려주면 git 이 부르는 길이 사용자 설정을 연다. io 가 내는 줄과 같은 결로 영어 한
/// 줄을 둔다.
///
/// **링크면 링크가 가리키는 파일을 갈아끼우고 링크는 그대로 둔다**(moai-4oab, 2026-09-29 사용자
/// 결정). 쓸 자리를 푸는 것은 [`target_of`] 가 한다.
///
/// **임시 파일은 받은 철자 곁에 먼저 짓고, 거기서 못 갈아끼우면 가리키는 파일 곁으로 한 번 더
/// 간다**(moai-4oab 리뷰). 가리키는 파일 곁에 곧장 지으면 링크인 `.moai/issues.jsonl` 의 쓰다 죽은
/// 찌꺼기가 `../shared/` 처럼 `init` 이 무시하지 않는 자리에 남아 `git add -A` 에 딸려 온다 —
/// moai-3akx 가 뿌리 파일에서 막은 바로 그 찌꺼기다. 물러서는 까닭은 `rename` 이 파일시스템을 못
/// 건너서다 — 대상이 다른 파일시스템이면 받은 철자 곁에서는 `EXDEV` 로 멈춘다. 실패한 쪽은 임시
/// 파일을 치우고 대상을 안 건드리므로 다시 써도 잃을 것이 없다(`cmd::init::plant` 와 같은 물러섬이다).
/// 링크가 아니면 두 자리가 같아 한 번뿐이다.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> R<()> {
    atomic(path, bytes, None)
}

/// [`write_atomic`] 이되 **링크는 `checkout` 안을 가리킬 때만 따라간다**(moai-4oab, 2026-09-29 사용자
/// 결정 둘째 판). 저장소가 든 파일(`.moai/issues.jsonl`, `AGENTS.md`)은 남이 커밋한 링크일 수 있다 —
/// 받은 저장소의 `AGENTS.md -> ~/.bashrc` 를 그대로 따라가면 흔한 `moai init`·`moai add` 가 체크아웃
/// 밖의 파일을 고쳐 쓴다. 사용자 설정·읽음 표시·`latest.toml` 은 사람이 제 손으로 건 링크(dotfiles)라
/// [`write_atomic`] 으로 어디든 따라간다.
pub(crate) fn write_atomic_inside(path: &Path, bytes: &[u8], checkout: &Path) -> R<()> {
    atomic(path, bytes, Some(checkout))
}

/// [`write_atomic`]·[`write_atomic_inside`] 의 몸통 — 임시 자리를 고르고 물러서는 것이 여기 한 벌이다.
fn atomic(path: &Path, bytes: &[u8], within: Option<&Path>) -> R<()> {
    let real = target_of(path, within)?;
    let (near, far) = (parent_of(path, path)?, parent_of(&real, path)?);
    match replace(&real, bytes, near) {
        Err(_) if near != far => replace(&real, bytes, far),
        done => done,
    }
}

/// 파일이 든 디렉터리 — 없으면(뿌리 `/` 나 빈 경로) `named` 를 대는 거절이다(moai-iq7j).
fn parent_of<'a>(path: &'a Path, named: &Path) -> R<&'a Path> {
    path.parent().ok_or_else(|| Fail::new(format!("{}: no parent directory", named.display())))
}

/// 쓸 자리 — 링크를 끝까지 푼 자리([`crate::path::follow_links`])이고, **갈아끼워도 되는 자리일 때만**
/// 낸다. 못 풀거나(고리) 안 되는 자리는 받은 철자를 대는 거절이고, 대상은 한 글자도 안 바뀐다.
///
/// - **가리키는 자리의 디렉터리가 없으면 그 말로 멈춘다**(moai-4oab 리뷰). 그냥 두면 거절문이 지은 적도
///   없는 임시 파일(`<대상>.tmp.<pid>.0`)을 대고 링크는 말하지 않아, `init` 을 부른 사람은 권한을 고치러
///   가고 `init --check` 는 블록이 없다고 해 같은 자리를 돈다. 디렉터리를 짓지는 않는다 — 사용자 설정의
///   `LinkDangling` 과 같은 까닭이다(아직 안 받은 저장소 자리일 수 있다)
/// - **보통 파일이 아니면**(장치·소켓·FIFO·디렉터리) 갈아끼우지 않는다(moai-4oab 리뷰). 링크를 따라가면서
///   `latest.toml -> /dev/null` 같은 링크의 끝도 쓸 자리가 됐는데, `rename` 은 그 자리를 보통 파일로
///   바꾼다 — root 로 도는 컨테이너에서는 `/dev/null` 이 그렇게 사라진다. 없는 파일은 새로 짓는다
///
/// **`within` 을 받으면 링크의 끝이 그 안에 있어야 한다**([`write_atomic_inside`]). 견주는 것은 끝의
/// 디렉터리를 푼 자리다 — 링크 글의 `..` 이나 가운데 디렉터리 링크로 밖에 닿는 것도 거기서 드러난다.
/// 그 디렉터리가 없으면 여기서 못 재고, 아래의 "디렉터리가 없다" 거절이 멈춘다.
///
/// **끝 조각이 링크가 아니어도 잰다**(moai-wd44 리뷰). 끝만 재던 판은 커밋된 `.moai/journal -> /밖` 을
/// 지나 `moai add` 가 체크아웃 밖에 `<사람>.jsonl` 을 지었다 — 막으려던 것과 같은 길이 디렉터리
/// 링크 하나로 열려 있었다. 그래서 `.moai` 자체가 체크아웃 밖을 가리키는 저장소도 여기서 거절된다.
/// 저장소 파일은 체크아웃 안에서만 링크를 따른다는 2026-09-29 사용자 결정을 디렉터리에도 그대로 편다.
///
/// **거절문에 싣는 링크의 끝은 제어 문자를 걷는다**(리뷰). 그 글은 받은 저장소가 커밋한 링크 글이고,
/// 거절문은 `init` 의 "못 썼다" 줄과 저널을 못 적었다는 줄을 거쳐 그대로 터미널로 나간다 — ESC 가
/// 든 링크 하나가 그 화면을 다시 칠한다(`text::sanitize` 가 선 까닭과 같다).
///
/// **체크아웃 안이라도 git 의 자리(`.git/`)는 아니다**(리뷰). 받은 저장소가
/// `.moai/journal/<사람>.jsonl -> ../../.git/config` 를 커밋해 두면 흔한 `moai add` 가 git 설정에 JSON 한
/// 줄을 덧붙여, 그 클론의 git 이 `bad config line` 으로 통째로 섰다. 저장소가 든 파일이 git 의 자리를
/// 가리킬 까닭은 없다.
fn target_of(path: &Path, within: Option<&Path>) -> R<PathBuf> {
    let real = crate::path::follow_links(path).map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
    let linked = real != path;
    let end = || crate::text::one_line(&real.display().to_string());
    if let Some(root) = within {
        let root = crate::path::real(root);
        if let Ok(dir) = std::fs::canonicalize(crate::path::dir_of(&real)) {
            if !dir.starts_with(&root) {
                // 끝 조각이 링크가 아니면 가운데 디렉터리가 링크다 — 그때 댈 것은 푼 디렉터리다.
                let landed = real.file_name().map_or_else(|| dir.clone(), |name| dir.join(name));
                return Err(Fail::new(format!(
                    "{} points at {}, outside {} — nothing is written, so the link is not replaced. A file the \
                     repository holds follows a link only inside its own checkout",
                    path.display(),
                    crate::text::one_line(&landed.display().to_string()),
                    root.display()
                )));
            }
            // 끝 이름도 센다 — 딸린 워크트리의 `.git` 은 디렉터리가 아니라 `gitdir:` 한 줄짜리 파일이다.
            let git = |p: &std::ffi::OsStr| p == ".git";
            if dir
                .strip_prefix(&root)
                .is_ok_and(|rest| rest.components().any(|c| git(c.as_os_str())) || real.file_name().is_some_and(git))
            {
                return Err(Fail::new(format!(
                    "{} points at {}, inside git's own directory — nothing is written, so the link is not \
                     replaced. A file the repository holds never follows a link into .git",
                    path.display(),
                    end()
                )));
            }
        }
    }
    match std::fs::metadata(&real) {
        Ok(m) if !m.is_file() => Err(Fail::new(match linked {
            true => format!(
                "{} points at {}, which is not a regular file — nothing is written, so the link is not replaced",
                path.display(),
                end()
            ),
            false => format!("{}: not a regular file — nothing is written", path.display()),
        })),
        Err(e) if linked && gone(&e) && std::fs::metadata(crate::path::dir_of(&real)).is_err_and(|e| gone(&e)) => {
            Err(Fail::new(format!(
                "{} points at {}, and that directory is not there — nothing is written, so the link is not \
                 replaced. Make that place, or fix the link",
                path.display(),
                end()
            )))
        }
        _ => Ok(real),
    }
}

/// [`write_atomic`] 이되 **임시 파일을 `tmp_dir` 에 둔다**(moai-3akx). 쓰다 죽으면 임시 파일이
/// 그 자리에 남으므로, 저장소 뿌리의 파일을 쓰는 `init` 은 이미 무시되는 `.moai/*.tmp.*` 자리를
/// 준다 — 옆자리에 두면 `AGENTS.md.tmp.…` 가 뿌리에 남아 `git add -A` 에 딸려 온다.
/// `rename` 은 파일시스템을 못 건너므로 `tmp_dir` 이 다른 파일시스템이면 `Err` 다 — 그때 옆자리로
/// 물러서는 것은 고르는 쪽이 한다(`cmd::init::plant`). 실패하면 **임시 파일을 남기지 않고** 대상은
/// 한 글자도 안 바뀐다.
///
/// **대상이 심볼릭 링크면 링크를 따라가 가리키는 파일을 바꾼다**(moai-4oab). `rename` 은 링크를
/// 안 따라가고 링크 자체를 갈아끼운다 — `AGENTS.md -> CLAUDE.md` 인 저장소의 `init` 이
/// `AGENTS.md` 를 따로 선 파일로 만들고 `CLAUDE.md` 에는 블록을 안 넣었고, 링크인
/// `issues.jsonl` 은 다음 쓰기에 보통 파일이 되어 가리키던 쪽이 옛 글에 멈췄다. 사용자 설정
/// ([`crate::user_config::update`])은 그 전부터 이렇게 했다. 여기서 푸는 까닭은 부르는 자리가
/// 여럿이라서다 — 부르는 쪽마다 풀게 두면 푼 곳과 잊은 곳이 갈린다. **락은 여기서 못 고른다** —
/// 락은 읽기보다 먼저 잡아야 하는데, 이 함수는 읽은 뒤에 불린다. 링크 너머의 파일을 함께 쓰는 쪽과
/// 서로를 막는 것은 부르는 쪽의 일이다([`Repo::write_locked`] 의 `far_lock`, 사용자 설정의 둘째 락).
///
/// **부르는 쪽은 저장소 뿌리의 파일(`init` 의 `AGENTS.md`)뿐이라 링크는 `checkout` 안에서만
/// 따른다**([`write_atomic_inside`] 와 같은 까닭이다).
pub(crate) fn write_atomic_in(path: &Path, bytes: &[u8], tmp_dir: &Path, checkout: &Path) -> R<()> {
    replace(&target_of(path, Some(checkout))?, bytes, tmp_dir)
}

/// 저장소가 든 파일에 **제자리에서 덧붙인다**(`O_APPEND`) — 링크는 [`write_atomic_inside`] 와 같은 자로
/// `checkout` 안을 가리킬 때만 따라간다(moai-wd44). 저널(`.moai/journal/*.jsonl`)과 `init` 이 줄을 더하는
/// `.gitignore`·`.gitattributes` 가 여기로 온다.
///
/// 갈아끼우는 쓰기만 막던 때는 받은 저장소의 `.moai/journal/<사람>.jsonl -> ~/.bashrc` 에 다음
/// `moai add` 가 JSON 한 줄을 덧붙였다 — `O_APPEND` 도 링크를 따라간다. 거절은 [`target_of`] 의 말
/// 그대로다(링크와 그 끝을 대고, 보통 파일이 아니거나 디렉터리가 없는 자리도 같다). 쓸 자리가 없으면
/// 새로 짓는 것은 전과 같다.
///
/// **푼 자리를 연다** — 받은 철자를 다시 열면 재고 난 뒤 바뀐 링크를 따라간다. 이미 푼 자리의 끝이 그
/// 사이에 링크로 바뀌는 것까지는 못 막는다 — 갈아끼우는 쪽과 같은 틈이다.
///
/// **끊긴 꼬리를 먼저 채운다**(moai-a65c). 파일이 `\n` 없이 끝나면(디스크가 찼거나 덧붙이다 죽었다 —
/// [`Repo::with_write`] 가 흔한 일로 치는 것) 새 줄이 그 끝에 그대로 붙어, 끊긴 줄과 새 줄이 한 줄이
/// 되고 둘 다 못 읽힌다. 저널이면 이번에 적은 `rm` 이 `show --removed` 에서 통째로 빠진다. 그래서
/// 끝 바이트가 `\n` 이 아니면 `\n` 하나를 앞에 붙여 **한 번에** 쓴다 — 끊긴 줄은 끊긴 채 남고
/// (이력은 건너뛰고, `show --removed` 는 지움을 들었을 수 있으면 댄다 — `Repo::journal_of_kind`) 새 줄은
/// 제 줄에 선다. 채우는 곳을 여기 하나로 둔 까닭은 부르는 쪽마다 채우면 둘이 겹쳐 빈 줄이 하나 더 서기
/// 때문이다 — `init` 의 `ensure_lines` 도 제 몫을 걷었다.
/// 락 없이 둘이 함께 채우면 빈 줄이 하나 더 설 수 있지만, 빈 줄은 저널에서도 딸린 파일에서도 뜻이 없다.
///
/// **꼬리를 못 보면 전처럼 덧붙이기만 한다**(리뷰). 끝 바이트를 보려고 읽기까지 열었는데, 쓰기만 되는
/// 파일(`0200`)은 그 열기에서 넘어져 덧붙이기 자체를 잃었다 — `moai note` 가 실패하고 `mv` 는 이력을 못
/// 남겼다. 채우기 하나 때문에 줄을 잃으면 막으려던 손실보다 크다. 그런 자리에서 붙어 버린 줄은 읽는 쪽이
/// 줄 머리로 갈라 되찾는다(`Repo::journal_by_id`).
pub(crate) fn append_inside(path: &Path, bytes: &[u8], checkout: &Path) -> R<()> {
    let real = target_of(path, Some(checkout))?;
    let fail = |e: std::io::Error| Fail::new(format!("{}: {e}", path.display()));
    let open = |read: bool| std::fs::OpenOptions::new().read(read).create(true).append(true).open(&real);
    let (mut f, torn) = match open(true) {
        Ok(mut f) => {
            let torn = torn_tail(&mut f).unwrap_or(false);
            (f, torn)
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => (open(false).map_err(fail)?, false),
        Err(e) => return Err(fail(e)),
    };
    let lead: &[u8] = if torn { b"\n" } else { b"" };
    f.write_all(&[lead, bytes].concat()).map_err(fail)?;
    f.sync_all().map_err(fail)
}

/// 비지 않은 파일이 `\n` 없이 끝나는가 — [`append_inside`] 가 채울지를 가른다. 읽는 것은 끝 한
/// 바이트뿐이고, `O_APPEND` 라 읽으려고 옮긴 자리가 다음 쓰기의 자리를 바꾸지 않는다.
fn torn_tail(f: &mut std::fs::File) -> std::io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    if f.metadata()?.len() == 0 {
        return Ok(false);
    }
    f.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    f.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// [`write_atomic`]·[`write_atomic_in`] 의 몸통 — **이미 푼 자리**([`target_of`])를 `tmp_dir` 의 임시
/// 파일로 갈아끼운다. 푸는 것을 여기 두지 않는 까닭은 한 번만 풀기 위해서다(리뷰) — 두 번 풀면 그 사이에
/// 링크가 바뀐 경우에 임시 자리와 갈아끼울 자리가 서로 다른 풀이에서 온다.
fn replace(path: &Path, bytes: &[u8], tmp_dir: &Path) -> R<()> {
    let perms = std::fs::metadata(path).ok().map(|m| m.permissions());
    let dir = parent_of(path, path)?;
    let (tmp, mut f) = open_tmp(path, tmp_dir)?;
    // **어디서 실패하든 임시 파일을 치운다.** `rename` 에서만 치우던 때는 디스크가 찬(ENOSPC)
    // 쓰기가 죽지 않고도 `<파일>.tmp.…` 를 남겼다 — moai-3akx 가 막으려던 찌꺼기다. 치우는 것은
    // **이 쓰기가 만든 파일뿐이다** — [`open_tmp`] 가 연 뒤에만 이 자리에 온다.
    let fail = |at: &Path, e: std::io::Error| {
        let _ = std::fs::remove_file(&tmp);
        Fail::new(format!("{}: {e}", at.display()))
    };
    let filled = (|| {
        if let Some(p) = perms {
            f.set_permissions(p)?;
        }
        f.write_all(bytes)?;
        f.sync_all()
    })();
    drop(f);
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

/// 임시 파일을 **새로 만들어서만** 연다(`O_CREAT|O_EXCL`, moai-ydm7.976) — 그 이름에 이미 무엇이
/// 있으면 [`tmp_names`] 의 다음 이름으로 간다.
///
/// **`File::create` 는 있는 파일을 말없이 비운다.** pid 가 같은 두 프로세스(pid 네임스페이스가
/// 다른 컨테이너 둘이 같은 마운트를 쓸 때)가 락 없이 한 파일을 쓰면, 늦은 쪽이 앞쪽이 아직 쓰는
/// 파일을 잘라 제 글을 쓰고 앞쪽의 `rename` 이 그것을 들고 간다 — 어느 쪽도 아닌 글이 대상에
/// 실린다. 조용한 손실이 이 도구가 못 견디는 하나뿐인 실패 모드다(CLAUDE.md). 새로 만들어서만
/// 열면 남의 파일을 안 건드리고, 마지막 이름 자리에 걸린 링크도 따라가지 않는다.
///
/// **남의 파일은 지우지 않는다** — 살아 있는 쓰기의 것인지 죽은 쓰기의 찌꺼기인지 여기서는 못
/// 가른다. 찌꺼기는 그 자리에 남는다: `.moai/` 의 것은 `.moai/*.tmp.*` 로 무시되지만, 사용자 설정
/// 디렉터리나 `init` 이 물러선 뿌리 등 다른 자리의 것은 무시되지 않는다. 그 찌꺼기가 쓰기를 막지는
/// 못한다 — 둘째 이름부터는 이 프로세스만의 것이다([`tmp_names`]).
fn open_tmp(path: &Path, tmp_dir: &Path) -> R<(PathBuf, std::fs::File)> {
    let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("out");
    let mut last = None;
    for name in tmp_names(file) {
        let tmp = tmp_dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => return Ok((tmp, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some((tmp, e)),
            Err(e) => return Err(Fail::new(format!("{}: {e}", tmp.display()))),
        }
    }
    let (tmp, e) = last.expect("TMP_TRIES 는 0 이 아니다");
    Err(Fail::new(format!("{}: {e}", tmp.display())))
}

/// 이 자리에 트래커를 세우면 무엇이 어긋나는가(moai-pjrr·moai-mz0e) — 없으면 `None`.
///
/// 묻는 것은 **명령이 어느 트래커로 가는가** 하나인데, 자리마다 답이 달라 [`Elsewhere`] 로 가른다.
///
/// - **딸린 워크트리**([`Elsewhere::Worktree`]): 워크트리 안에서 친 `moai` 는 주 체크아웃의 트래커를
///   읽고 쓴다(moai-y7go). 여기 심은 `.moai` 는 **아무도 안 읽고**, 커밋되면 병합에서 겨룬다 —
///   그래서 거절한다
/// - **위에 트래커가 있는 하위 디렉터리**([`Elsewhere::Above`]): 여기 세우면 이 밑의 명령은 여기
///   것을 쓰고 옆 디렉터리는 위의 것을 쓴다. 심은 것이 **읽히기는 한다** — 그래서 알리기만 한다
///
/// **둘을 가른 것은 값이 다르기 때문이다**(2026-09-20 사용자 결정 둘째 판). 위로 찾기에는 경계를
/// 그을 자가 없다 — 천장을 두었다가 걷은 것이 같은 날의 moai-a2kn 이고, 그 결정은 "`여기서
/// moai init` 은 트래커를 하나 더 세운다" 를 **살아 있는 길**로 적었다. `~/.moai` 를 둔 사람의 새
/// 프로젝트마다 거절을 세우면 그 길이 막힌다. 워크트리는 다르다: 거기 심은 것은 어느 명령도 안 읽어,
/// 알림 한 줄로 두면 사람이 그것을 모른 채 커밋한다.
///
/// `MOAI_HERE` 는 둘 다 끈다 — 그 워크트리에서만 쓰는 트래커를 일부러 두는 길이다.
///
/// **위로 찾는 자는 `.moai` 가 디렉터리인가로 가른다** — [`look`] 의 위로 찾기와 같은 자다.
/// `config.toml` 까지 봐야 트래커라고 세는 자리도 있지만([`crate::worktree::tracker_root`]), 여기서
/// 물어야 하는 것은 "명령이 어디로 가는가" 라 그쪽 자를 쓰면 설정이 빠진 `.moai` 위에서 둘이 갈린다.
///
/// **찾은 자리에서 한 번 더 옮김을 묻는다**(리뷰) — `.moai` 를 가진 조상을 찾았다고 거기가 끝이
/// 아니다. [`Repo::find_from`] 은 그 자리에서 [`crate::worktree::tracker_root`] 로 한
/// 번 더 옮겨 가므로(moai-y7go), 묻지 않으면 도구가 **제가 안 읽는 트래커**를 댄다.
pub(crate) fn planted_elsewhere(root: &Path) -> Option<Elsewhere> {
    // **`MOAI_HERE` 가 이 물음을 통째로 끈다.** [`Repo::opened_root`] 도 같은 손잡이를 거치지만
    // (`Repo::redirect`), 여기 한 줄로 세워야 두 갈래가 한 자로 꺼진다 — 그쪽에 맡기던 판은 켠
    // 것이 어느 갈래를 끄는지가 두 모듈을 오가야 보였다.
    //
    // **끄는 자리는 여기 하나다**(moai-ko4y, 2026-09-21 사용자 결정). 알맹이([`elsewhere`])는 손잡이를
    // 안 묻는다 — "지금 이 부름이 거절되는가" 와 "나중에 누가 어디서 `init` 을 쳐야 하나" 는 다른
    // 물음이고, 뒤엣것은 그 사람의 셸이 무엇을 켰는지 여기서 알 수 없다([`init_belongs_at`]).
    (!here_wanted()).then(|| elsewhere(root)).flatten()
}

/// [`planted_elsewhere`] 의 알맹이 — **`MOAI_HERE` 를 안 묻는다.**
///
/// **조상 훑기는 [`climb`] 하나다**(moai-c9ty). 글은 "위로 찾는 자와 같은 자" 라 적혀 있는데 걸음을
/// 다시 적어 두었던 자리고, 두 벌이면 [`look`] 이 "못 들여다보는 조상은 건너뛴다" 를 고치는 날
/// 이쪽만 옛 걸음으로 남는다. **제 자리는 안 묻는다** — 여기 이미 심겨 있으면 [`crate::cmd::init::run`]
/// 이 딸린 파일만 다시 맞추므로, `root.parent()` 에서 올라간다.
///
/// **딸린 워크트리 안은 어디든 워크트리다**(moai-pk4x, 2026-09-21 사용자 결정). 워크트리 꼭대기만
/// 묻던 판은 **밑자리**(`<wt>/src/deep`)에서 조상 훑기가 그 꼭대기를 지나 주 체크아웃까지 올라가
/// [`Elsewhere::Above`] 를 냈고, `init` 은 알림 한 줄만 내고 워크트리 안에 `.moai` 를 심었다 —
/// 아무도 안 읽고 커밋되면 병합에서 겨루는 파일이다. 가르는 자를 [`crate::worktree::main_root`]
/// 하나로 둔다: 그것이 답하면 밑자리든 꼭대기든 워크트리고, **찾는 자리도 그것이 비추는 자리**다.
///
/// **대는 자리는 나를 다스리는 트래커다.** [`crate::worktree::main_root`] 가 비추는 자리
/// (`<main>/<밑길>`)를 그대로 대면 그 디렉터리가 없을 때 대는 명령도 안 돌아, **비친 자리에서
/// 위로 찾은** 트래커를 댄다 — 밑자리든 꼭대기든, 모노레포의 한 칸이든 같은 자 하나로 선다.
///
/// **찾기는 주 체크아웃을 안 떠난다**(리뷰). 조상 훑기를 제 자리에서 시작해 찾은 것을 그대로
/// [`Elsewhere::Worktree`] 에 넣던 판은 그 갈래가 약속한 것(*나를 다스리는 주 체크아웃의 트래커*)을
/// 셋으로 깼다 — 집에 `.moai` 를 둔 사람은 **모든 저장소의 모든 워크트리**에서 `init` 이 거절당하고
/// 집을 다시 `init` 하라는 줄을 받았고, 트래커가 이 가지에서 처음 선 저장소는 제 워크트리 안의
/// 자리를 "주 체크아웃" 으로 들었으며, 워크트리 안에 딴 `.moai` 가 있으면 진짜 주 체크아웃의
/// 트래커를 아예 안 물었다. 가르는 자는 [`climb`] 의 둘째 값이다 — 제 체크아웃을 두고 올라갔으면
/// 그것은 "위에 있다" 이지 주 체크아웃이 아니다.
///
/// **주 체크아웃 밖에 뜬 워크트리도 같다.** 비친 자리에서 찾으므로 `git worktree add ../side` 처럼
/// 주 체크아웃 **밖**에 선 워크트리의 밑자리도 잡힌다 — 제 자리에서만 올라가던 판은 거기서 아무것도
/// 못 찾아 `None` 을 냈고, `init` 이 워크트리 안에 트래커를 심었다(moai-pk4x 가 막는 바로 그것).
///
/// **찾은 조상이 다시 옮겨 가면 그 자리를 댄다** — 딸린 워크트리 안에 겹쳐 둔 저장소(`<wt>/vendor`)가
/// 그 자리다. 안 묻던 판은 도구가 **제가 안 읽는 트래커**를 알림에 댔다.
///
/// **옮길 곳이 없으면 그 자리다**(moai-71ht.jlh). 주 체크아웃에도 위에도 트래커가 없는 워크트리 —
/// 그 가지에서 처음 `init` 하는 자리다 — 는 `None` 이고, 거기 세우는 것이 유일한 길이다. 워크트리라는
/// 사실만으로 거절하면 트래커를 처음 들이는 길이 통째로 막힌다.
fn elsewhere(root: &Path) -> Option<Elsewhere> {
    // 위에서 찾은 자리 — **찾고 나서 한 번 더 옮김을 묻는다.** 딸린 워크트리의 트래커를 잡았으면
    // 대야 할 말은 "위에 있다" 가 아니라 그것이 옮겨 가는 자리다.
    let above = || {
        let at = root.parent().and_then(climb).map(|(at, _)| at)?;
        Some(crate::worktree::tracker_root(&at).map_or(Elsewhere::Above(at), Elsewhere::Worktree))
    };
    // 딸린 워크트리가 아니면 위에서 찾은 것이 답이다 — 조상 훑기는 여기서 처음 돈다.
    let Some(mirror) = crate::worktree::main_root(root) else { return above() };
    climb(&mirror)
        .filter(|(_, left_a_checkout)| !*left_a_checkout)
        .map(|(at, _)| Elsewhere::Worktree(at))
        .or_else(above)
}

/// **"여기서 `moai init` 하라" 를 대도 되는가** — 안 되면 대신 댈 주 체크아웃이다(moai-nppo).
///
/// [`planted_elsewhere`] 의 두 갈래 가운데 [`Elsewhere::Worktree`] 하나만 든다. `init` 이 거절하는
/// 자리가 그것뿐이라, 표면들이 묻는 것도 그 하나다 — [`Elsewhere::Above`] 는 세워지고 `init` 이
/// 세우고 나서 알리므로 대는 말을 바꿀 까닭이 없다.
///
/// **가르는 자를 여기 하나로 둔다.** 한눈 보기(`view::unopened`)·`moai project add|ls`·
/// `moai init --check` 가 저마다 갈래를 풀면 한 곳을 고친 날 나머지가 옛 말을 한다 — 등록한
/// 워크트리 한 줄이 영영 `init 전` 으로 서고 그 줄이 대는 명령이 1 로 끝나던 자리가 그것이다.
///
/// **이미 여기 심겨 있으면 안 묻는다**(리뷰) — [`crate::cmd::init::run`] 의 `dir.exists()` 와 같은
/// 자다. 거절은 **새 트래커가 설 때만** 서고, 심겨 있는 자리에서 `init` 이 하는 일은 딸린 파일을
/// 다시 맞추는 것뿐이라 0 으로 끝난다. 그 물음을 부르는 쪽 하나에만 두던 판은 `.moai` 를 커밋하는
/// 저장소(여기가 그렇다)의 **모든 워크트리**에서 `moai init --check` 가 "`moai init` 은 여기 안
/// 선다" 를 냈다 — 그 말은 거짓인 데다, 따라 친 `moai -C <주 체크아웃> init` 은 방금 잰 것과 **다른
/// 체크아웃**의 `AGENTS.md` 와 딸린 파일을 고쳐, 이 워크트리의 낡은 블록은 영영 낡은 채로 남았다.
/// 묻는 자리를 여기 두어야 표면이 다섯이 되어도 같은 답이 선다.
///
/// **`MOAI_HERE` 를 안 물려받는다**(moai-ko4y, 2026-09-21 사용자 결정) — [`planted_elsewhere`] 가
/// 아니라 [`elsewhere`] 로 든다. 이 자가 답하는 것은 **나중의 다른 부름**이 어디서 서느냐라, 그
/// 부름의 셸이 손잡이를 켤지는 여기서 알 수 없다. 물려받던 판은 `MOAI_HERE=1` 인 셸의 `moai status`
/// 가 워크트리 줄에 `moai -C <워크트리> init` 을 댔는데, 그 줄을 접두어 없이 딴 셸에 붙여 넣으면 1 로
/// 끝났다 — `MOAI_HERE=1 moai project ls --json` 은 `tracker_at` 을 통째로 뺐다. 지금 프로세스의
/// 거절은 그대로 꺼진다([`planted_elsewhere`]): 그 손잡이를 켠 사람은 여기 심는 것이 뜻이다.
pub(crate) fn init_belongs_at(dir: &Path) -> Option<PathBuf> {
    if dir.join(".moai").exists() {
        return None;
    }
    match elsewhere(dir) {
        Some(Elsewhere::Worktree(main)) => Some(main),
        Some(Elsewhere::Above(_)) | None => None,
    }
}

/// **이 부름이 여기서 실제로 읽는 트래커의 뿌리** — 없으면 `None`.
///
/// [`init_belongs_at`] 과 **묻는 것이 다르다.** 그쪽은 "나중의 다른 부름이 어디서 `init` 을 쳐야
/// 하나" 라 손잡이를 안 보고(moai-ko4y), 이쪽은 "지금 이 셸이 무엇을 읽고 있나" 라 손잡이를 본다.
/// 가르는 자는 [`Repo::redirect`] 하나고, 찾는 걸음은 [`Repo::find_from`] 과 같은 자([`look`])다 —
/// 갈라 적으면 한쪽만 옮겨 가는 날 이 답이 읽는 파일과 갈린다.
///
/// 쓰는 자리는 `moai init --check` 의 끝줄이다(moai-ha0f, 리뷰 moai-uocc.45o 의 2번) — 손잡이를 켠
/// 셸에 "여기 심는다" 를 대기 전에, **그 심는 것이 이 셸이 읽던 트래커를 가리는지**를 이 자에게
/// 묻는다. 딸린 워크트리의 밑자리(`<wt>/src/deep`)가 그 자리다: 손잡이를 켠 셸은 `<wt>/.moai` 를
/// 읽는데, 그 줄을 따라 치면 `src/deep` 에 아무도 안 읽는 `.moai` 가 서고 원래 줄들은 사라진 것처럼
/// 보인다.
pub(crate) fn tracker_in_use(dir: &Path) -> Option<PathBuf> {
    look(dir).map(|found| Repo::opened_root(&found))
}

/// [`planted_elsewhere`] 가 찾은 자리 — **자리마다 값이 다르다.**
pub(crate) enum Elsewhere {
    /// 이 워크트리를 **다스리는 주 체크아웃의 트래커 자리**. 여기 심은 트래커는 아무도 안 읽어
    /// `init` 이 **거절하고**, "여기서 `init` 하라" 를 대는 표면들은 **그 자리를 대신 댄다**(moai-nppo).
    /// 꼭대기만이 아니라 워크트리 **밑자리**에서도 선다(moai-pk4x) — 고르는 차례는 [`elsewhere`] 에 있다.
    Worktree(PathBuf),
    /// 위에서 찾은 트래커의 뿌리. 여기 세운 것도 읽히므로 **알리기만 한다.**
    Above(PathBuf),
}

/// **그 자리가 없다는 뜻인가** — 있고 없고를 가르는 잣대는 도구에 하나다(moai-blvx).
///
/// 경로 가운데가 파일이면(`file/sub`) `NotADirectory` 다. 그 자리는 **없는 것**이지 잠깐 못 보는
/// 것이 아니라, `NotFound` 와 한 낱말로 읽는다.
///
/// [`Repo::open`] 안에 닫힌 글로 있던 것을 꺼냈다. 숨어 있던 동안 [`crate::read_marks::settle`] 은
/// `NotFound` 만 조용히 지나가, 등록한 줄의 경로 가운데가 파일로 바뀌면 저장소 쪽은 조용히 "없다"
/// 로 지나가는데 읽음 쪽은 적재마다 "자리를 못 풀어 적힌 철자로 든다 — Not a directory" 를 냈다
/// (리뷰 moai-f31d.lhe 12번). 같은 조건을 두 표면이 달리 부르던 자리다.
///
/// **여기 안 든 갈래는 없는 것이 아니다.** `EACCES`·`ELOOP`·`ESTALE` 는 자리가 서 있는데 못 닿은
/// 것이라 읽는 쪽이 **까닭을 댄다** — 없는 자리는 저쪽이 이미 제 낱말로 대므로 조용히 지나간다
/// ([`crate::read_marks::settle`]). 가르는 잣대가 [`crate::user_config::unreadable`] 과 따로 서는 까닭도
/// 그것이다: 그쪽은 **다시 해 볼 값**을 가르고 이쪽은 **있는가**를 가른다.
///
/// **쓰는 쪽은 둘 다 대기 자리로 간다**(moai-jfgn, 2026-09-21 사용자 결정). 한때는 없는 자리만 받은
/// 철자의 읽음 파일에 적었는데, 링크가 잠깐 바뀌었다 돌아오는 창의 도장을 그 뒤에 아무도 다시 안 봤다
/// — 그래서 이 갈래는 이제 **떨어지는가**가 아니라 **까닭을 대는가**만 가른다
/// ([`crate::read_marks::Settled`]).
pub(crate) fn gone(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory)
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
    /// 받은 저장소가 커밋할 수 없는 자리의 락을 잡는다.
    ///
    /// **말은 물러날 때만 묻는다**(moai-iq7j) — `lang` 이 값이 아니라 묻는 길인 까닭이고,
    /// 락을 아직 안 쥔 자리라 여기서 물어도 제 락에 걸리지 않는다.
    ///
    /// **링크를 따른다** — 사용자 자신의 파일(설정·읽음 표시) 곁의 락과, git 의 공용 디렉터리에 선 워크트리 표식의
    /// 락(`worktree::note_held`)이 이 문을 지난다. stow·rcm 은 dotfiles 의 파일을 하나씩 걸어 락까지
    /// 걸리고(`user_config::a_linked_lock_file_is_locked_once`), 그 락을 막으면 그런 사람의 설정 쓰기가 다 선다.
    /// 받은 저장소가 커밋할 수 있는 락은 [`Lock::inside`] 를 지난다.
    pub(crate) fn acquire(path: &Path, lang: impl FnOnce() -> crate::i18n::Lang) -> R<Lock> {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| Fail::new(format!("{}: {e}", path.display())))?;
        Lock::wait(f, path).map_err(|stop| stop.said(lang))
    }

    /// **저장소 락** — `.moai/lock` 과 링크 너머 스냅샷 곁의 락([`Repo::far_lock`]). 연 자리를
    /// [`crate::held::lock`] 이 잰다: 디렉터리는 `home` 안이고 `.git/` 밖이어야 하며, 끝 조각은 어디를
    /// 가리키든 링크를 안 따르고, 보통 파일이어야 한다(moai-sn57, 2026-10-02 사용자 결정).
    ///
    /// 따르던 판은 커밋된 `.moai/lock -> /proc/self/fd/2` 하나로 프로세스마다 제 stderr 를 잠가, 동시
    /// `moai add` 스물넷이 다 0 으로 끝나고 이슈는 셋에서 다섯만 남았다 — 이 도구가 못 견디는 조용한 손실이다.
    /// 거절은 아무것도 안 쓰고 `broken` 으로 멈춘다([`Trouble::LockUnheld`]).
    ///
    /// **말은 안 묻는다** — 물러난 까닭은 자료([`Stop::Refused`])로 들고 나가 [`Repo::with_write_lines`] 가 락을
    /// 다 놓은 뒤에 편다. 너머의 락은 제 락을 쥔 채 잡으므로, 여기서 물으면 사용자 설정을 읽는 동안(FIFO 면 영영)
    /// `.moai/lock` 을 쥐고 있어 옆의 쓰기가 다 `locked` 로 물러났다(리뷰 moai-sn57.kq4).
    fn inside(path: &Path, home: &crate::held::Home) -> Result<Lock, Stop> {
        let f = match crate::held::lock(path, home) {
            Ok(f) => f,
            Err(crate::held::Fell::Unheld(why)) => {
                return Err(Stop::Refused(Trouble::LockUnheld { at: path.to_path_buf(), why }));
            }
            Err(crate::held::Fell::Io(e)) => return Err(Lock::failed(path, &e)),
        };
        Lock::wait(f, path)
    }

    /// io 가 진 것에 락 자리를 단 글 — **자리는 제어 문자를 걷는다**. 너머의 락은 커밋된 링크 글을 이어 붙인
    /// 철자라 받은 저장소가 지은 디렉터리 이름이 그대로 든다([`crate::held::refused`] 와 같은 까닭).
    fn failed(path: &Path, e: &std::io::Error) -> Stop {
        Fail::new(format!("{}: {e}", crate::text::one_line(&path.display().to_string()))).into()
    }

    /// 연 락 파일에 `flock` 을 잡을 때까지 기다린다 — [`Lock::acquire`]·[`Lock::inside`] 의 몸통. 기다리다
    /// 물러나면 그 까닭도 자료([`Trouble::LockBusy`])로 낸다 — 말로 펴는 것은 부르는 쪽이다.
    fn wait(f: std::fs::File, path: &Path) -> Result<Lock, Stop> {
        let start = Instant::now();
        loop {
            match f.try_lock_exclusive() {
                Ok(()) => return Ok(Lock(f)),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
                {
                    if start.elapsed() >= LOCK_TIMEOUT {
                        return Err(Stop::Refused(Trouble::LockBusy { secs: LOCK_TIMEOUT.as_secs() }));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(Lock::failed(path, &e)),
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
    ///
    /// **같은 파일을 가르는 자는 [`file_id`] 하나다** — 저널을 접는 자리([`Repo::journal_files`])와 한 벌이다(리뷰
    /// moai-itsu.n8z). unix 에서는 늘 값이 서므로 그 견줌이 곧 장치·아이노드의 견줌이다.
    pub(crate) fn holds(&self, path: &Path) -> Option<bool> {
        #[cfg(unix)]
        {
            Some(match (self.0.metadata(), std::fs::metadata(path)) {
                (Ok(held), Ok(other)) => file_id(&held) == file_id(&other),
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
    use crate::model::{Kind, Status};
    use crate::scratch::Scratch;

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

        let Opened::Repo(repo) = Repo::open(&side, || crate::i18n::Lang::Ko).unwrap() else { panic!("안 열렸다") };
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
        let Opened::Repo(repo) = Repo::open(&feat, || crate::i18n::Lang::Ko).unwrap() else { panic!("안 열렸다") };
        assert_eq!(repo.root, feat, "옮길 곳이 없는데 옮겼다");
        assert_eq!(repo.here(), feat, "안 옮겼는데 옮겼다고 적었다");
    }

    /// **위에 트래커가 있으면 그 자리를 찾아 낸다**(moai-pjrr). 막지는 않는다 — 여기 심은 것은 이
    /// 밑에서 읽히고, 위로 찾기에는 경계를 그을 자가 없다(같은 날 moai-a2kn 이 천장을 걷었다).
    /// 부르는 쪽이 이 값으로 알림 한 줄을 세운다.
    ///
    /// **`.moai` 가 디렉터리인가로 가른다** — 위로 찾는 [`look`] 과 같은 자다. 그 자가 갈리면
    /// 여기서 지나간 자리를 명령이 잡는다.
    ///
    /// 한때 `cmd/init.rs` 에 섰다 — [`planted_elsewhere`] 가 이리로 오면서 그 파일의 것은 하나도
    /// 안 재게 되었고, 재는 자와 시험이 갈리면 여기를 고치는 사람이 시험을 못 찾는다(리뷰).
    #[test]
    fn a_subdir_under_a_tracker_finds_the_one_above() {
        let s = Scratch::new("init-above");
        let deep = s.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        assert!(planted_elsewhere(&deep).is_none(), "트래커가 없는데 자리를 댔다");

        moai_at(s.path());
        match planted_elsewhere(&deep) {
            Some(Elsewhere::Above(at)) => assert_eq!(at, s.path(), "댄 자리가 트래커의 자리가 아니다"),
            Some(Elsewhere::Worktree(main)) => panic!("워크트리가 아닌데 워크트리라 했다 — {}", main.display()),
            None => panic!("위의 트래커를 못 봤다"),
        }

        // 그 자리 자신은 안 묻는다 — 여기 이미 심겨 있으면 `run` 이 딸린 파일만 다시 맞춘다.
        assert!(planted_elsewhere(s.path()).is_none(), "제 트래커를 남의 것으로 댔다");
    }

    /// **이미 심겨 있는 자리는 "여기 안 선다" 가 아니다**(리뷰) — [`init_belongs_at`] 은
    /// [`crate::cmd::init::run`] 과 **같은 물음**에 답해야 한다. `run` 은 `.moai` 가 이미 있으면 안
    /// 묻고 딸린 파일만 다시 맞춰 0 으로 끝나므로, 그 자리에서 "주 체크아웃에서 친다" 를 대면 대는
    /// 말이 거짓이고 따라 친 명령은 **다른 체크아웃**의 `AGENTS.md` 를 고친다.
    ///
    /// `.moai` 를 커밋하는 저장소(이 저장소가 그렇다)에서는 워크트리마다 `.moai` 가 함께 와,
    /// 그 갈래가 **모든 워크트리 세션**에 섰다. 밑자리는 그대로 주 체크아웃을 댄다 — 거기는 `run` 도
    /// 거절하는 자리다.
    #[test]
    fn a_worktree_that_already_carries_a_tracker_is_not_sent_away() {
        let dir = Scratch::real("init-belongs");
        let main = dir.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &std::path::Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        moai_at(&main);
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        git(&main, &["worktree", "add", "-q", "side", "-b", "side"]);
        let side = main.join("side");

        // 가지가 `.moai` 를 함께 들고 왔다 — `moai init` 은 여기서 0 으로 끝난다.
        assert!(side.join(".moai").is_dir(), "워크트리가 트래커를 안 들고 왔다");
        assert_eq!(init_belongs_at(&side), None, "심겨 있는 워크트리를 딴 데로 보냈다");

        // 밑자리에는 안 심겼다 — 거기는 `run` 도 거절하니 주 체크아웃을 댄다.
        let deep = side.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(init_belongs_at(&deep), Some(main.clone()), "밑자리가 주 체크아웃을 못 댔다");

        // 트래커를 걷으면 그 자리도 주 체크아웃을 댄다 — 가르는 것은 `.moai` 가 여기 있는가다.
        std::fs::remove_dir_all(side.join(".moai")).unwrap();
        assert_eq!(init_belongs_at(&side), Some(main), "안 심긴 워크트리를 제자리라 했다");
    }

    /// **moai 를 들이기 전 커밋에서 갈라진 워크트리**(moai-pk4x) — 그 안에는 `.moai` 가 한 자리도
    /// 없어, 조상 훑기가 워크트리 꼭대기를 지나 주 체크아웃까지 올라갔다. 거기 트래커가 있으니
    /// [`Elsewhere::Above`] 가 서고, `init` 은 알림 한 줄만 내고 **워크트리 안에** 트래커를 심었다 —
    /// 아무도 안 읽고 커밋되면 병합에서 겨루는 파일이다(moai-mz0e 가 꼭대기에서 막는 바로 그것).
    ///
    /// 가르는 자는 [`crate::worktree::main_root`] 다 — 그것이 답하면 위에 무엇이 있든 워크트리다.
    /// 대는 자리는 **실제로 읽히는 트래커**이지 비친 자리(`<main>/src/deep`, 없는 디렉터리다)가 아니다.
    #[test]
    fn a_worktree_that_carries_no_tracker_is_still_a_worktree() {
        let dir = Scratch::real("init-belongs-bare-wt");
        let main = dir.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &std::path::Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        // moai 를 들이기 **전** 커밋 — 워크트리는 여기서 갈라진다.
        std::fs::write(main.join("README"), "before moai\n").unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "before"]);
        // 갈라질 자리를 가지로 박아 둔다 — `run_git` 은 낸 글을 안 돌려줘 sha 를 못 읽는다.
        git(&main, &["branch", "pre"]);
        moai_at(&main);
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        git(&main, &["add", "-A", "-f"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        git(&main, &["worktree", "add", "-q", "--detach", "side", "pre"]);

        let side = main.join("side");
        assert!(!side.join(".moai").exists(), "시험의 전제 — 워크트리가 트래커를 들고 왔다");
        let deep = side.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();

        match planted_elsewhere(&deep) {
            Some(Elsewhere::Worktree(at)) => {
                assert!(crate::user_config::same_dir(&at, &main), "주 체크아웃이 아닌 자리를 댔다 — {}", at.display());
            }
            Some(Elsewhere::Above(at)) => panic!("워크트리 밑자리를 '위에 있다' 로 댔다 — {}", at.display()),
            None => panic!("워크트리 밑자리에 트래커를 심어도 된다고 했다"),
        }
        assert!(
            init_belongs_at(&deep).as_deref().is_some_and(|at| crate::user_config::same_dir(at, &main)),
            "init 을 댈 자리가 주 체크아웃이 아니다"
        );
    }

    /// **주 체크아웃 밖에 뜬 워크트리의 밑자리도 워크트리다**(moai-pk4x, 리뷰). `git worktree add ../side`
    /// 는 규약의 자리가 아니지만 git 이 막지 않는다 — 거기서는 조상 훑기가 제 자리에서 올라가 봐야
    /// `.moai` 를 한 자리도 못 만나, `init` 이 **워크트리 안에** 트래커를 심었다. 꼭대기만 답을 냈던
    /// 것은 비친 자리(`<main>`)가 마침 트래커의 자리여서다.
    ///
    /// 찾는 자리를 **비친 자리**로 옮기면 밑자리도 같은 답을 낸다([`elsewhere`]).
    #[test]
    fn a_worktree_outside_the_main_checkout_is_a_worktree_all_the_way_down() {
        let dir = Scratch::real("init-belongs-away-wt");
        let main = dir.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &std::path::Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        std::fs::write(main.join("README"), "before moai\n").unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "before"]);
        git(&main, &["branch", "pre"]);
        moai_at(&main);
        std::fs::write(main.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        git(&main, &["add", "-A", "-f"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        // **주 체크아웃 밖**에 뜬다 — `<scratch>/side` 는 `<scratch>/main` 의 밑이 아니다.
        git(&main, &["worktree", "add", "-q", "--detach", "../side", "pre"]);

        let side = dir.join("side");
        assert!(!side.join(".moai").exists(), "시험의 전제 — 워크트리가 트래커를 들고 왔다");
        let deep = side.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();

        for at in [side.as_path(), deep.as_path()] {
            match planted_elsewhere(at) {
                Some(Elsewhere::Worktree(told)) => assert!(
                    crate::user_config::same_dir(&told, &main),
                    "주 체크아웃이 아닌 자리를 댔다 — {} ({})",
                    told.display(),
                    at.display()
                ),
                Some(Elsewhere::Above(told)) => {
                    panic!("{} 를 '위에 있다' 로 댔다 — {}", at.display(), told.display())
                }
                None => panic!("{} 에 트래커를 심어도 된다고 했다", at.display()),
            }
        }
    }

    /// **워크트리 안의 자리를 "주 체크아웃" 이라 대지 않는다**(리뷰). [`Elsewhere::Worktree`] 는
    /// *나를 다스리는 주 체크아웃의 트래커 자리* 라는 약속인데, 찾은 조상을 그대로 그 갈래에 넣던
    /// 판은 **그 조상이 이 워크트리 안**일 때도 그렇게 댔다 — 트래커가 이 가지에서 처음 선 저장소다.
    /// `init` 은 거절하고 `moai -C <워크트리> init` 을 댔는데, 그 자리는 제가 방금 거절한 그 자리다.
    ///
    /// 같은 구멍이 `~/.moai` 를 가진 사람에게는 더 크게 섰다 — 조상 훑기가 저장소를 통째로 지나
    /// 집을 잡아, **모든 저장소의 모든 워크트리**에서 `init` 이 거절하고 집을 다시 `init` 하라고 댔다.
    /// 가르는 자는 [`crate::worktree::main_root`] 가 비추는 자리에서 **제 체크아웃을 안 떠나고**
    /// 찾았는가다([`climb`] 의 둘째 값).
    #[test]
    fn a_tracker_inside_the_worktree_is_not_called_the_main_checkout() {
        let dir = Scratch::real("init-belongs-only-wt");
        let main = dir.join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &std::path::Path, args: &[&str]| crate::git::tests::run_git(at, None, args);
        git(&main, &["init", "-q"]);
        std::fs::write(main.join("README"), "no moai here\n").unwrap();
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "before"]);
        git(&main, &["worktree", "add", "-q", "side", "-b", "side"]);
        // 트래커는 이 가지에서 처음 섰다 — 주 체크아웃에는 없다.
        let side = main.join("side");
        moai_at(&side);
        std::fs::write(side.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        assert!(!main.join(".moai").exists(), "시험의 전제 — 주 체크아웃에 트래커가 섰다");

        let deep = side.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        match planted_elsewhere(&deep) {
            Some(Elsewhere::Above(at)) => {
                assert!(crate::user_config::same_dir(&at, &side), "위의 트래커를 딴 자리로 댔다 — {}", at.display());
            }
            Some(Elsewhere::Worktree(at)) => {
                panic!("워크트리 안의 자리를 주 체크아웃이라 댔다 — {}", at.display())
            }
            None => panic!("위에 선 트래커를 한 줄도 안 알렸다"),
        }
        // 여기 세운 것은 이 밑에서 실제로 읽힌다([`Repo::find_from`] 이 워크트리의 트래커를 그대로
        // 든다 — 주 체크아웃에 옮겨 갈 자리가 없다). 그러니 "딴 데서 쳐라" 를 대면 안 된다.
        assert_eq!(init_belongs_at(&deep), None, "여기 세우면 읽히는데 딴 자리를 댔다");
    }

    // **`MOAI_HERE` 를 안 물려받는 것은 여기서 안 잰다**(moai-ko4y, 리뷰). 이 층에서 재려면
    // `set_var` 로 프로세스 환경을 만져야 하는데, 단위 시험은 한 프로세스의 **스레드**로 나란히
    // 돌아 그 값을 옆 시험이 본다 — 실제로 `a_worktree_that_carries_no_tracker_is_still_a_worktree`
    // 가 마흔 판에 두 번 `MOAI_HERE=1` 을 물려받아 붉어졌다. `user_config::path_from` 이 환경을
    // 인자로 받는 것과 `tests_do_not_read_the_runners_home` 이 제 바이너리를 다시 띄우는 것이 같은
    // 까닭이다. 재는 자리는 CLI 층 하나다 — `tests/cli.rs` 의
    // `saying_where_init_goes_does_not_inherit_moai_here` 가 손잡이를 켠 **딴 프로세스**로
    // `--check` 의 `tracker_at` 과 거절이 꺼지는 것을 함께 잰다.

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
        r.with_write(
            || crate::i18n::Lang::Ko,
            |issues, _, _| {
                issues.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::create("argos-4aex", "t", T, &crate::model::someone("raven"))], ()))
            },
        )
        .unwrap();
        let load = r.read().unwrap();
        assert_eq!(load.issues.len(), 1);
        assert!(load.errors.is_empty());
        assert_eq!(r.journal_of("argos-4aex").len(), 1);
    }

    /// 임의 순서로 넣어도 파일은 언제나 id 순이고, 자식이 부모 밑에 붙는다.
    #[test]
    fn output_is_sorted_and_deterministic() {
        let (r, d) = repo("sorted");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |issues, _, _| {
                for id in ["argos-4aey", "argos-4aex.ae3", "argos-0001", "argos-4aex"] {
                    issues.push(issue(id));
                }
                Ok((vec![], ()))
            },
        )
        .unwrap();
        let src = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let ids: Vec<&str> = src.lines().map(|l| l.split('"').nth(3).unwrap()).collect();
        assert_eq!(ids, ["argos-0001", "argos-4aex", "argos-4aex.ae3", "argos-4aey"]);
    }

    /// 바뀐 게 없으면 파일을 건드리지 않는다 — 헛 diff 를 만들지 않는다.
    #[test]
    fn unchanged_write_leaves_the_file_alone() {
        let (r, d) = repo("idem");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![], ()))
            },
        )
        .unwrap();
        let path = d.join(".moai/issues.jsonl");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        r.with_write(|| crate::i18n::Lang::Ko, |_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    /// **파일 이름은 메일에서 온다**(moai-nzlo). 접는 자리가 여기 하나라, 이름이 경로 조각이
    /// 되거나 기계마다 달라지는 것을 여기서 막는다.
    #[test]
    fn a_journal_file_is_named_by_the_email() {
        for (email, want) in [
            ("raven@buzzni.com", "raven_buzzni_com.jsonl"),
            // 큰 글자를 내린다 — 안 내리면 글자 크기를 안 가리는 파일시스템에서만 둘이 하나가 된다.
            ("Raven@Buzzni.COM", "raven_buzzni_com.jsonl"),
            // ASCII 낱말·숫자·`-`·`_` 밖은 다 `_` 다.
            // 글자 하나가 글자 하나다 — 바이트로 세면 한글 한 자가 `_` 셋이 된다.
            ("레이븐+메일@a.b", "_______a_b.jsonl"),
            ("a-b_c1@x.io", "a-b_c1_x_io.jsonl"),
            // 경로가 될 수 있는 글자는 남지 않는다 — 디렉터리에 이어 붙이는 자리가 여기 하나다.
            ("../../etc/passwd@x", "______etc_passwd_x.jsonl"),
        ] {
            let got = journal_file(email).unwrap_or_else(|| panic!("{email:?} 를 안 받았다"));
            assert_eq!(got, want, "{email:?}");
            assert!(!got.contains(['/', '\\']) && got != ".jsonl", "{email:?} → {got}");
        }
        assert_eq!(journal_file(""), None);
        assert_eq!(journal_file("   "), None);
    }

    /// 이 저장소의 자리만 골라 센 것 — [`UNREAD`] 는 프로세스 하나의 것이라 병렬 시험끼리
    /// 섞인다. 수를 세지 말고 제 뿌리 밑의 짝만 본다.
    fn unread_under(root: &Path) -> Vec<Unread> {
        journal_unread().into_iter().filter(|u| u.at.starts_with(root)).collect()
    }

    /// [`unread_under`] 와 같되 탐색기 머리의 줄이 읽는 [`UNREAD_NOW`] 에서 고른다 — 다시 재는 걸음이 걷는 쪽이다.
    fn unread_now_under(root: &Path) -> Vec<Unread> {
        journal_unread_now().into_iter().filter(|u| u.at.starts_with(root)).collect()
    }

    /// **못 여는 저널 자리는 넘어가되 조용히는 아니다**(2026-09-21 사용자 결정, moai-6ney).
    /// 멈추던 때는 자리 하나를 못 여는 것만으로 모든 사람의 이력이 통째로 안 보였다. 지금은
    /// 읽던 것을 내고, 못 연 자리를 [`journal_unread`] 에 세운다 — 그것을 대는 자가 `cmd::run`
    /// 이고, 거기서 종료 코드도 0 이 아니게 된다.
    #[test]
    fn an_unreadable_journal_dir_is_carried_and_told() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("nojournaldir");
        let by = crate::model::someone("raven");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::create("argos-4aex", "t", T, &by)], ()))
            },
        )
        .unwrap();
        assert_eq!(r.journal_of("argos-4aex").len(), 1);
        assert!(unread_under(d.path()).is_empty(), "멀쩡한 판에서 무언가를 셌다");

        let dir = d.join(".moai/journal");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read_dir(&dir).is_ok() {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }
        // **재기부터 하고 권한을 되돌린 뒤에 견준다**(리뷰) — 견주다 넘어지면 0000 인 자리가
        // 그대로 남아 [`crate::scratch::Scratch`] 의 `Drop` 이 못 치우고, 죽은 pid 의 임시
        // 디렉터리가 기계에 쌓인다(`scratch` 모듈이 이름 대어 걷어낸 바로 그 새는 자리다).
        let got = r.journal_of("argos-4aex");
        let told = unread_under(d.path());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(got.is_empty(), "못 연 자리에서 이력을 지어냈다");
        assert_eq!(told.len(), 1, "못 연 자리를 안 셌거나 여러 번 셌다 — {told:?}");
        assert_eq!(told[0].root, r.root, "어느 저장소인지 안 댄다 — {told:?}");
        assert_eq!(told[0].at, dir, "어느 자리인지 안 댄다 — {told:?}");
        assert!(!told[0].said(crate::i18n::Lang::En).is_empty(), "까닭을 안 댄다");
        // **`chmod` 로 풀리는 갈래를 기계가 가른다**(moai-f2lc) — `said` 는 운영체제가 지은
        // 글이라 `LANG` 과 libc 에 따라 바뀌니, 받는 쪽이 그것을 부분 문자열로 맞추면 안 된다.
        assert_eq!(told[0].kind(), "permission", "권한으로 막힌 자리를 그렇게 안 댄다 — {told:?}");
    }

    /// **이름은 읽히는데 잴 수 없는 자리도 센다**(리뷰, moai-6ney). `chmod 400 .moai/journal` 은
    /// `read_dir` 은 되고 `stat` 은 안 되는 자리인데, `Path::is_file` 이 그 실패를 `false` 로
    /// 접어 **모든** 저널 파일이 말없이 빠졌다 — 이력 없는 화면이 stderr 한 줄 없이 0 으로
    /// 끝났다. 조용한 손실이 이 도구가 못 견디는 하나다.
    #[test]
    fn a_journal_dir_that_cannot_be_stat_ed_is_told_not_swallowed() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("nostatjournal");
        let by = crate::model::someone("raven");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::create("argos-4aex", "t", T, &by)], ()))
            },
        )
        .unwrap();

        let dir = d.join(".moai/journal");
        let mine = dir.join(journal_file(&by.email).unwrap());
        // 읽기만 되고 훑기(`x`)는 안 되는 자리 — 이름은 나오지만 `metadata` 가 EACCES 로 진다.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o400)).unwrap();
        if std::fs::metadata(&mine).is_ok() {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }
        let got = r.journal_of("argos-4aex");
        let told = unread_under(d.path());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(got.is_empty(), "못 잰 자리에서 이력을 지어냈다");
        assert_eq!(told.len(), 1, "못 잰 파일을 안 셌거나 여러 번 셌다 — {told:?}");
        assert_eq!(told[0].at, mine, "어느 파일인지 안 댄다 — {told:?}");
        assert_eq!(told[0].kind(), "permission", "{told:?}");
    }

    /// **남의 파일 하나가 제 이력까지 막지 않는다**(2026-09-21 사용자 결정, moai-6ney). 한
    /// 체크아웃을 두 계정이 쓰면 남의 `<메일>.jsonl` 이 0600 으로 서는 일이 흔한데, 그때
    /// 통째로 지던 자리다 — **제 파일에만** 이력이 있는 이슈까지 아무것도 안 보였다.
    ///
    /// 관대해진 읽기가 조용해지지 않는 것까지 한 자리에서 잰다: 읽은 것은 나오고, 못 읽은
    /// 파일은 [`journal_unread`] 에 선다.
    #[test]
    fn an_unreadable_journal_file_is_skipped_and_told() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("nojournalfile");
        let mine = crate::model::someone("raven");
        let theirs = crate::model::someone("other");
        for by in [&mine, &theirs] {
            r.with_write(
                || crate::i18n::Lang::Ko,
                |i, _, _| {
                    let id = format!("argos-{}", &by.name[..4]);
                    i.push(issue(&id));
                    Ok((vec![JournalEntry::create(&id, "t", T, by)], ()))
                },
            )
            .unwrap();
        }
        let theirs_file = d.join(".moai/journal").join(journal_file(&theirs.email).unwrap());
        assert!(theirs_file.is_file(), "남의 파일이 안 섰다 — {}", theirs_file.display());

        std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&theirs_file).is_ok() {
            std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o644)).unwrap();
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }
        // **두 번 읽어도 한 줄이다** — `moai show` 는 목록과 상세에서 저널을 두 번 읽는다.
        // 재기부터 하고 권한을 되돌린 뒤에 견준다(위와 같은 까닭).
        let mine_hist = r.journal_of("argos-rave");
        let once = unread_under(d.path());
        let theirs_hist = r.journal_of("argos-othe");
        let twice = unread_under(d.path());
        std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert_eq!(mine_hist.len(), 1, "제 파일의 이력까지 잃었다");
        assert_eq!(once.len(), 1, "못 읽은 파일을 안 셌거나 여러 번 셌다 — {once:?}");
        assert_eq!(once[0].at, theirs_file, "어느 파일인지 안 댄다 — {once:?}");
        assert_eq!(once[0].kind(), "permission", "{once:?}");
        assert!(theirs_hist.is_empty());
        assert_eq!(twice.len(), 1, "같은 파일을 두 번 셌다");
    }

    /// **고친 저널 자리는 다시 재면 걷힌다 — 아직 못 읽는 자리는 남는다**(moai-mkyg.ncj). 머리의 줄이 늘기만 하는
    /// [`UNREAD`] 를 읽던 때는 `chmod 644` 뒤에도 탐색기 머리의 `Journal` 줄이 나갈 때까지 섰다. 파일 하나와
    /// `journal/` 디렉터리 하나를 다 본다 — 파일은 [`Repo::journal_bytes`] 가, 디렉터리는 [`Repo::journal_names`] 가
    /// 센 자리다.
    ///
    /// 리뷰 moai-mkyg.n60 이 둘을 더 잰다. **걷는 것은 머리의 줄이 읽는 [`UNREAD_NOW`] 뿐이다** — 나갈 때의 말이 읽는
    /// [`UNREAD`] 는 고쳐도 그대로다. **목록에 없던 파일은 안 연다** — 내 파일은 읽혔던 자리라, 그 뒤에 못 읽게 되어도
    /// 다시 재는 걸음은 그것을 안 열어 새로 세지 않는다.
    #[test]
    fn a_fixed_journal_place_is_dropped_on_reopen_and_a_broken_one_stays() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("reopenjournal");
        let mine = crate::model::someone("raven");
        let theirs = crate::model::someone("other");
        for by in [&mine, &theirs] {
            r.with_write(
                || crate::i18n::Lang::Ko,
                |i, _, _| {
                    let id = format!("argos-{}", &by.name[..4]);
                    i.push(issue(&id));
                    Ok((vec![JournalEntry::create(&id, "t", T, by)], ()))
                },
            )
            .unwrap();
        }
        let dir = d.join(".moai/journal");
        let theirs_file = dir.join(journal_file(&theirs.email).unwrap());
        let mine_file = dir.join(journal_file(&mine.email).unwrap());
        let ours = |u: &Unread| u.at.starts_with(d.path());
        std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&theirs_file).is_ok() {
            std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o644)).unwrap();
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }
        // 재기부터 하고 권한을 되돌린 뒤에 견준다 — 위의 시험들과 같은 까닭이다.
        r.journal_of("argos-othe");
        let seen = unread_now_under(d.path());
        std::fs::set_permissions(&mine_file, std::fs::Permissions::from_mode(0o000)).unwrap();
        let kept = reopen_picked(&r.config, ours);
        let still = unread_now_under(d.path());
        std::fs::set_permissions(&mine_file, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&theirs_file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let fixed = reopen_picked(&r.config, ours);
        let after_file = unread_now_under(d.path());

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        r.journal_of("argos-othe");
        let dir_seen = unread_now_under(d.path());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dir_fixed = reopen_picked(&r.config, ours);
        let after_dir = unread_now_under(d.path());

        assert_eq!(seen.len(), 1, "시험의 전제 — 남의 파일을 못 읽은 자리로 셌다: {seen:?}");
        assert_eq!((kept, &still), (0, &seen), "아직 못 읽는 자리를 걷었거나 목록에 없던 내 파일을 열었다");
        assert_eq!(fixed, 1, "고친 파일을 안 걷었다 — {after_file:?}");
        assert!(after_file.is_empty(), "고친 파일이 남았다 — {after_file:?}");
        assert_eq!(dir_seen.len(), 1, "시험의 전제 — 디렉터리를 못 훑은 자리로 셌다: {dir_seen:?}");
        assert_eq!((dir_fixed, after_dir.len()), (1, 0), "고친 디렉터리를 안 걷었다 — {after_dir:?}");
        assert_eq!(r.journal_of("argos-othe").len(), 1, "다 고쳤는데 남의 이력을 못 읽는다");
        assert!(unread_now_under(d.path()).is_empty(), "다 고친 뒤의 읽기가 무언가를 셌다");
        assert_eq!(unread_under(d.path()), [seen, dir_seen].concat(), "나갈 때 댈 자리까지 걷었다");
    }

    /// **다시 재는 걸음은 철자가 아니라 푼 자리로 견준다**(리뷰 moai-mkyg.n60). 뿌리를 링크 너머에서 부르고 저널에
    /// `a.jsonl -> c.jsonl` 이 서 있으면, 못 읽는 `c.jsonl` 은 링크가 푼 자리의 철자로 선다. 그 링크를 지우면
    /// [`Repo::journal_files`] 는 같은 파일을 뿌리의 철자로 내는데, 철자로 견주던 판은 그것을 안 열고 고친 것으로 걷어
    /// 머리의 줄이 아직 못 읽는 파일을 두고 사라졌다.
    #[cfg(unix)]
    #[test]
    fn a_reopen_finds_a_journal_whose_spelling_changed() {
        use std::os::unix::fs::PermissionsExt;
        let real = scratch("reopenrespelled");
        let outer = Scratch::new("store-reopenrespelled-link");
        let root = outer.join("via");
        std::os::unix::fs::symlink(real.path(), &root).unwrap();
        let r = Repo::at(root.clone(), Config::load(&root).unwrap());
        let dir = real.join(".moai/journal");
        std::fs::create_dir_all(&dir).unwrap();
        let c = dir.join("c.jsonl");
        std::fs::write(&c, "").unwrap();
        std::os::unix::fs::symlink("c.jsonl", dir.join("a.jsonl")).unwrap();
        std::fs::set_permissions(&c, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&c).is_ok() {
            std::fs::set_permissions(&c, std::fs::Permissions::from_mode(0o644)).unwrap();
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }
        let mine = |u: &Unread| u.root == root;
        let now = || journal_unread_now().into_iter().filter(|u| u.root == root).collect::<Vec<_>>();
        r.journal_of("argos-0001");
        let seen = now();
        std::fs::remove_file(dir.join("a.jsonl")).unwrap();
        reopen_picked(&r.config, mine);
        let after = now();
        std::fs::set_permissions(&c, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert_eq!(seen.len(), 1, "시험의 전제 — 못 읽는 파일을 셌다: {seen:?}");
        assert!(
            after.iter().any(|u| u.at.ends_with("c.jsonl")),
            "철자만 바뀐 못 읽는 파일을 고친 것으로 걷었다 — {after:?}"
        );
    }

    /// **링크가 가리키는 저널은 한 번만 읽는다**(moai-p9mq). 메일을 바꾼 사람이 옛 `<메일>.jsonl` 을 새
    /// 파일로 이어 두면 `metadata` 가 링크를 따라가 둘 다 보통 파일로 보여, 같은 줄이 이력·`-g`·`work`·
    /// `show --removed` 에 두 번 섰다 — `work` 는 토큰을 두 번 더했다. 옛 한 파일이 링크인 경우도 같다.
    ///
    /// 남의 파일은 제 줄대로 선다 — 푼 자리가 다른 파일까지 하나로 접으면 이력을 잃는다.
    ///
    /// **두 단계로 나눠 본다**(리뷰) — 옛 한 파일은 늘 먼저 만나므로, 그 링크를 처음부터 세우면 `journal/` 안의
    /// 링크끼리 접히는 경우(moai-p9mq 가 겨눈 경우)와 거기서 한 파일이 어느 자리에 서는지가 통째로 가려진다.
    /// 그때는 두 링크를 지워도, 남는 철자를 바꿔도 이 시험이 푸르렀다.
    #[cfg(unix)]
    #[test]
    fn a_journal_reached_through_a_link_is_read_once() {
        let (r, _d) = repo("journal-link");
        let mine = crate::model::someone("raven");
        let theirs = crate::model::someone("other");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "model: anthropic/opus-5 tokens=100", T, &mine)], ()))
            },
        )
        .unwrap();
        r.with_write(
            || crate::i18n::Lang::Ko,
            |_, _, _| {
                Ok((
                    vec![
                        JournalEntry::removed("argos-0002", "둘", T, &mine),
                        JournalEntry::note("argos-4aex", "남", T, &theirs),
                    ],
                    (),
                ))
            },
        )
        .unwrap();
        let name = journal_file(&mine.email).unwrap();
        let real = crate::path::real(&r.journal_dir().join(&name));
        let old = r.journal_dir().join("0-old.jsonl");
        let late = r.journal_dir().join("zzz-old.jsonl");
        let other = r.journal_dir().join(journal_file(&theirs.email).unwrap());
        // 줄은 두 단계 모두 한 번씩이다 — 달라지는 것은 한 파일이 서는 자리뿐이다.
        let read_once = |step: &str| {
            let hist: Vec<_> = r.journal_of("argos-4aex").into_iter().filter_map(|e| e.text).collect();
            assert_eq!(
                hist,
                ["model: anthropic/opus-5 tokens=100", "남"],
                "{step}: 링크가 가리키는 저널을 다시 읽었거나 남의 줄을 잃었다"
            );
            let removed: Vec<String> = r.journal_of_kind("rm").0.into_iter().map(|e| e.id).collect();
            assert_eq!(removed, ["argos-0002"], "{step}: 지운 줄이 두 번 섰다");
            r.journal_files()
        };

        // 첫 단계는 `journal/` 안의 링크뿐이다 — 이름 차례로 제 끝의 앞과 뒤에 하나씩.
        std::os::unix::fs::symlink(&name, &old).unwrap();
        std::os::unix::fs::symlink(&name, &late).unwrap();
        // **처음 만난 이름의 자리에 푼 자리가 선다** — 링크 철자로 두면 읽기 전에 그 링크가 사라질 때 끝 파일의
        // 이력이 말없이 빠진다. 아직 없는 옛 한 파일은 못 풀어도 접지 않고 남는다.
        assert_eq!(read_once("journal/ 안의 링크"), [r.journal_path(), real.clone(), other.clone()]);
        // 표식은 접기 전의 이름을 다 잰다 — 접어 뺀 링크가 다른 파일로 옮겨 가도 알아챈다.
        let marked: Vec<PathBuf> = r.journal_marks().into_iter().map(|(p, _)| p).collect();
        let names = [r.journal_dir(), r.journal_path(), old, other.clone(), r.journal_dir().join(&name), late];
        assert_eq!(marked, names);

        // 옛 한 파일도 같은 파일로 이으면 맨 앞의 그 자리에 선다.
        std::os::unix::fs::symlink(format!("journal/{name}"), r.journal_path()).unwrap();
        assert_eq!(read_once("옛 한 파일의 링크"), [real.clone(), other.clone()]);

        // **하드 링크도 한 번이다**(moai-itsu.aod) — 풀어도 이름이 둘이라 푼 자리로 접던 판은 같은 줄을 두 번
        // 읽었다. 같은 파일의 자리(장치와 inode)로 접는다.
        std::fs::hard_link(r.journal_dir().join(&name), r.journal_dir().join("zzzz-hard.jsonl")).unwrap();
        assert_eq!(read_once("하드 링크"), [real, other]);
    }

    /// **체크아웃 밖으로 풀리는 저널 자리는 안 읽는다**(moai-karj). 받은 저장소가
    /// `.moai/journal/x.jsonl -> /proc/self/pagemap` 을 커밋해 두면 `stat -L` 은 크기 0 인 보통 파일이라 답해
    /// [`Repo::journal_names`] 를 지나고, `fs::read` 가 끝없이 읽어 이력·`-g`·`--removed`·탐색기의 노트 찾기가
    /// OOM 으로 죽었다. 쓰기([`append_inside`])가 이미 거절하던 자리라, 읽기도 같은 자로 잰다.
    ///
    /// **끝없이 읽는 파일로 재지 않는다** — 고침이 없으면 시험이 메모리를 다 쓴다. 밖에 멀쩡한 저널을 두고
    /// 그 줄이 이력에 서는지를 본다. 끝 조각의 링크, 옛 한 파일의 링크, 가운데 디렉터리의 링크를 다 본다 —
    /// 쓰기 쪽 시험(`a_repository_file_never_follows_a_link_out_of_its_checkout`)이 재는 꼴과 같다. 안을
    /// 가리키는 링크(moai-p9mq)는 `a_journal_reached_through_a_link_is_read_once` 가 지킨다.
    ///
    /// **옛 한 파일의 링크를 먼저 건다** — 둘째 링크는 같은 파일을 가리키므로, 접고 나서 재면 그 이름이 말없이
    /// 빠진다. 차례를 거꾸로 두면 첫 판이 이미 센 이름이라 그 빠짐이 가려진다.
    #[cfg(unix)]
    #[test]
    fn a_journal_link_out_of_the_checkout_is_not_read_but_told() {
        let away = Scratch::new("store-journal-away");
        let theirs = away.join("x.jsonl");
        let line = serde_json::to_string(&JournalEntry::note("argos-4aex", "밖", T, &crate::model::someone("other")));
        std::fs::write(&theirs, format!("{}\n", line.unwrap())).unwrap();
        // **`kind` 로 가른다**(moai-itsu) — `said` 의 낱말로 가르던 판은 `Unread` 의 글이 "가르는 자가 아니다" 라고
        // 적은 일을 시험이 했다. 체크아웃 밖과 `.git/` 은 같은 `outside` 고, 둘은 자료의 갈래로 가른다.
        let outside = |u: &Unread, at: &Path| {
            u.at == at && u.kind() == "outside" && matches!(u.why, Missed::Held(crate::held::Unheld::Outside { .. }))
        };

        let (r, d) = repo("journal-outside");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "안", T, &crate::model::someone("raven"))], ()))
            },
        )
        .unwrap();
        let texts = || r.journal_of("argos-4aex").into_iter().filter_map(|e| e.text).collect::<Vec<_>>();
        for link in [r.journal_path(), r.journal_dir().join("zz-away.jsonl")] {
            std::os::unix::fs::symlink(&theirs, &link).unwrap();
            assert_eq!(texts(), ["안"], "{}: 체크아웃 밖의 저널을 읽었거나 제 이력을 잃었다", link.display());
            let told = unread_under(d.path());
            assert!(told.iter().any(|u| outside(u, &link)), "밖을 가리키는 링크를 말없이 뺐다 — {told:?}");
        }

        // 가운데 디렉터리가 링크여도 같다 — 끝 조각은 링크가 아니라 보통 파일로 보인다.
        let (r, d) = repo("journal-outside-dir");
        std::os::unix::fs::symlink(away.path(), r.journal_dir()).unwrap();
        assert!(r.journal_of("argos-4aex").is_empty(), "디렉터리 링크를 지나 체크아웃 밖의 저널을 읽었다");
        let told = unread_under(d.path());
        assert!(told.iter().any(|u| outside(u, &r.journal_dir().join("x.jsonl"))), "{told:?}");

        // **못 푼 철자는 재지 않는다** — 뿌리는 받은 철자 그대로라(`moai -C <링크>`) 없는 옛 한 파일의 철자는 푼
        // 뿌리와 안 맞는다. 그것을 밖이라 세면 링크로 연 저장소의 명령이 하나같이 0 아닌 코드로 끝난다.
        let (r, d) = repo("journal-outside-spelled");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "안", T, &crate::model::someone("raven"))], ()))
            },
        )
        .unwrap();
        let via = away.join("door");
        std::os::unix::fs::symlink(d.path(), &via).unwrap();
        let spelled = Repo::at(via.clone(), r.config.clone());
        assert!(!spelled.journal_path().exists(), "옛 한 파일이 없는 판이어야 한다");
        assert_eq!(spelled.journal_of("argos-4aex").len(), 1, "링크로 연 뿌리에서 제 이력을 잃었다");
        let told = unread_under(&via);
        assert!(told.is_empty(), "링크로 연 뿌리의 저널을 밖이라고 했다 — {told:?}");

        // **체크아웃 안이라도 `.git/` 으로 가는 링크는 안 읽는다**(리뷰) — 쓰기([`target_of`])와 같은 자다.
        let (r, d) = repo("journal-into-git");
        std::fs::create_dir_all(d.join(".git")).unwrap();
        std::fs::copy(&theirs, d.join(".git/x.jsonl")).unwrap();
        std::fs::create_dir_all(r.journal_dir()).unwrap();
        let link = r.journal_dir().join("g.jsonl");
        std::os::unix::fs::symlink("../../.git/x.jsonl", &link).unwrap();
        assert!(r.journal_of("argos-4aex").is_empty(), "링크를 지나 .git 안의 파일을 저널로 읽었다");
        let told = unread_under(d.path());
        let into_git = |u: &Unread| matches!(u.why, Missed::Held(crate::held::Unheld::IntoGit { .. }));
        assert!(told.iter().any(|u| u.at == link && u.kind() == "outside" && into_git(u)), "{told:?}");
    }

    /// **연 파일이 댄 크기까지만 읽는다**(moai-karj 리뷰) — 이름으로 잰 뒤 읽기 전에 그 자리가 procfs 로 갈리는
    /// 틈을 손잡이의 크기가 닫는다. `/proc/self/status` 는 `stat` 이 크기 0 이라 답하면서 글을 내는 파일이라,
    /// 고침이 없으면 글이 읽히고 고치면 빈 것이다. 끝없이 읽는 파일로 재지 않는다 — 고침이 없으면 시험이
    /// 메모리를 다 쓴다. 그 자는 [`crate::held::read`] 에 살고 거기 곁의 시험이 재므로, 여기서 지키는 것은
    /// **저널이 그 자를 지나는가** 다 — 저널만 `fs::read` 로 돌아가면 이 시험 하나가 붉어진다.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_journal_is_read_no_further_than_the_size_its_handle_gives() {
        let (r, _d) = repo("journal-bounded");
        let status = Path::new("/proc/self/status");
        assert_eq!(std::fs::metadata(status).unwrap().len(), 0, "procfs 가 크기를 대는 판이다 — 시험이 못 잰다");
        assert_eq!(r.journal_bytes(status), Some(Vec::new()), "손잡이가 댄 크기 너머를 읽었다");
    }

    /// **스냅샷·설정이 체크아웃 밖·`.git/` 으로 가거나 보통 파일이 아니면 모든 명령이 고른 말로 멈춘다**
    /// (moai-itsu, 2026-10-02 사용자 결정). 받은 저장소가 커밋한 `issues.jsonl -> /proc/self/pagemap` 하나로 모든
    /// 명령이 메모리를 다 쓰던 자리다. 끝없이 읽는 파일로 재지 않는다 — 고침이 없으면 시험이 메모리를 다 쓴다.
    /// 밖과 `.git/` 에 멀쩡한 파일을 두고 그것이 읽히지 않는지를 본다. 디렉터리는 고침 전에도 `error` 로
    /// 멈췄으니 코드(`broken`)로 가른다. **끝이 없는 링크도 같다**(리뷰 moai-itsu.n8z) — 재지 않던 판은 밖을
    /// 가리키는 끝 없는 스냅샷을 "없음" 으로 읽어 빈 보드를 그렸다.
    #[cfg(unix)]
    #[test]
    fn a_snapshot_or_config_out_of_the_checkout_stops_every_command() {
        let away = Scratch::new("store-held-away");
        let row = serde_json::to_string(&issue("argos-4aex")).unwrap();
        std::fs::write(away.join("issues.jsonl"), format!("{row}\n")).unwrap();
        std::fs::write(away.join("config.toml"), "prefix = \"argos\"\n").unwrap();
        let find = |d: &Scratch, lang| Repo::find_from(d.path(), move || lang).map(|r| r.map(|r| r.root));
        for file in ["issues.jsonl", "config.toml"] {
            for how in ["outside", "git", "directory", "dangling", "dangling-git"] {
                let d = scratch(&format!("held-{how}"));
                let at = d.join(".moai").join(file);
                std::fs::remove_file(&at).unwrap();
                match how {
                    "outside" => std::os::unix::fs::symlink(away.join(file), &at).unwrap(),
                    "git" => {
                        std::fs::create_dir_all(d.join(".git")).unwrap();
                        std::fs::copy(away.join(file), d.join(".git").join(file)).unwrap();
                        std::os::unix::fs::symlink(format!("../.git/{file}"), &at).unwrap();
                    }
                    "dangling" => std::os::unix::fs::symlink(away.join(format!("missing-{file}")), &at).unwrap(),
                    "dangling-git" => {
                        std::fs::create_dir_all(d.join(".git")).unwrap();
                        std::os::unix::fs::symlink(format!("../.git/missing-{file}"), &at).unwrap();
                    }
                    _ => std::fs::create_dir(&at).unwrap(),
                }
                let e = find(&d, crate::i18n::Lang::Ko).expect_err(&format!("{file} ({how}): 읽고 지나갔다"));
                assert_eq!(e.code, code::BROKEN, "{file} ({how}): {}", e.message);
                assert!(
                    e.message.contains(&at.display().to_string()),
                    "{file} ({how}): 자리를 안 댄다 — {}",
                    e.message
                );
                let en = find(&d, crate::i18n::Lang::En).unwrap_err().message;
                assert_ne!(en, e.message, "{file} ({how}): 말이 안 갈린다 — 말묶음을 안 지났다");
            }
        }

        // **안을 가리키는 링크는 그대로 읽는다** — 쓰기([`target_of`])도 따르는 자리다.
        let d = scratch("held-inside");
        std::fs::create_dir_all(d.join("data")).unwrap();
        std::fs::write(d.join("data/issues.jsonl"), format!("{row}\n")).unwrap();
        std::fs::remove_file(d.join(".moai/issues.jsonl")).unwrap();
        std::os::unix::fs::symlink("../data/issues.jsonl", d.join(".moai/issues.jsonl")).unwrap();
        let r = Repo::find_from(d.path(), || crate::i18n::Lang::Ko).unwrap().expect("저장소를 못 찾았다");
        assert_eq!(r.read().unwrap().issues.len(), 1, "안을 가리키는 링크를 안 읽었다");

        // **지은 뒤에 링크가 갈려도 읽기가 다시 잰다** — 탐색기가 떠 있는 동안 받은 `git pull` 이 그런 경우다.
        // 여기는 화면 말을 모르는 자리라 말 없는 꼴(`<자리> -> <끝>`)로 멈춘다.
        std::fs::remove_file(d.join(".moai/issues.jsonl")).unwrap();
        std::os::unix::fs::symlink(away.join("issues.jsonl"), d.join(".moai/issues.jsonl")).unwrap();
        let e = r.read().expect_err("지은 뒤에 갈린 링크를 따라 밖을 읽었다");
        assert_eq!(e.code, code::BROKEN, "{}", e.message);
        assert!(e.message.contains(" -> "), "{}", e.message);
    }

    /// **링크를 지나 연 뿌리도 제 스냅샷을 읽는다**(리뷰 moai-itsu.n8z) — 견줄 뿌리를 안 푼 채 재면(`held::Home`
    /// 이 막는 꼴) 링크 철자의 뿌리 아래 멀쩡한 스냅샷이 밖으로 읽혀 모든 명령이 섰다. 손으로 적은 등록 경로
    /// (`Repo::open`)와 훅이 `cd`·`-C` 로 찾은 자리(`Repo::find_from`)가 그 철자를 그대로 든다.
    #[cfg(unix)]
    #[test]
    fn a_root_reached_through_a_link_reads_its_own_snapshot() {
        let d = scratch("held-via-link");
        let row = serde_json::to_string(&issue("argos-4aex")).unwrap();
        std::fs::write(d.join(".moai/issues.jsonl"), format!("{row}\n")).unwrap();
        let away = Scratch::new("store-held-via");
        let via = away.join("door");
        std::os::unix::fs::symlink(d.path(), &via).unwrap();
        let Ok(Opened::Repo(r)) = Repo::open(&via, || crate::i18n::Lang::Ko) else {
            panic!("링크로 연 저장소가 안 열렸다")
        };
        assert_eq!(r.read().unwrap().issues.len(), 1, "링크로 연 뿌리에서 제 스냅샷을 못 읽었다");
        let r = Repo::find_from(&via, || crate::i18n::Lang::Ko).unwrap().expect("링크로 찾은 저장소가 없다");
        assert_eq!(r.read().unwrap().issues.len(), 1, "링크로 찾은 뿌리에서 제 스냅샷을 못 읽었다");
    }

    /// **끝이 보통 파일이 아니거나 없는 밖 링크도 `outside` 로 선다**(리뷰 moai-itsu.n8z). 목록이 보통 파일만
    /// 들던 판은 `-> /dev/null` 을 재기 전에 말없이 뺐고, 끝이 없는 밖 링크는 `failed`(ENOENT)로 섰다 — 같은
    /// 커밋된 링크가 끝의 꼴에 따라 조용하거나 다른 갈래로 섰다. 안에 선 디렉터리는 저널이 아니라 여전히 말하지
    /// 않는다.
    #[cfg(unix)]
    #[test]
    fn a_journal_link_out_to_no_file_or_no_end_is_told_as_outside() {
        let away = Scratch::new("store-journal-noend-away");
        let (r, d) = repo("journal-noend");
        std::fs::create_dir_all(r.journal_dir()).unwrap();
        let null = r.journal_dir().join("dev.jsonl");
        std::os::unix::fs::symlink("/dev/null", &null).unwrap();
        let gone = r.journal_dir().join("gone.jsonl");
        std::os::unix::fs::symlink(away.join("missing.jsonl"), &gone).unwrap();
        std::fs::create_dir(r.journal_dir().join("sub.jsonl")).unwrap();
        assert!(r.journal_of("argos-4aex").is_empty());
        let told = unread_under(d.path());
        for link in [&null, &gone] {
            assert!(
                told.iter().any(|u| u.at == *link && u.kind() == "outside"),
                "{}: 밖 링크를 outside 로 안 댔다 — {told:?}",
                link.display()
            );
        }
        assert_eq!(told.len(), 2, "안의 디렉터리까지 댔거나 한 링크를 두 번 댔다 — {told:?}");
    }

    /// **읽을 때 보통 파일이 아니면 넘기되 센다** — 목록을 짓고 읽기 전에 그 이름이 디렉터리·FIFO 로 갈린
    /// 경우다([`Repo::journal_bytes`] 의 안 읽기로 한 갈래). `failed` 로 서고 막히지 않는다. 이 갈래를 세는 줄을
    /// 걷어도 다른 시험이 안 붉어지던 자리다(리뷰 moai-itsu.n8z).
    #[cfg(unix)]
    #[test]
    fn a_journal_that_is_no_file_at_the_read_is_told() {
        let (r, d) = repo("journal-no-file");
        let dir = d.join("turned.jsonl");
        std::fs::create_dir(&dir).unwrap();
        assert_eq!(r.journal_bytes(&dir), None, "디렉터리를 저널로 읽었다");
        let told = unread_under(d.path());
        let not_a_file = |u: &Unread| matches!(u.why, Missed::Held(crate::held::Unheld::NotAFile));
        assert!(told.iter().any(|u| u.at == dir && u.kind() == "failed" && not_a_file(u)), "{told:?}");
    }

    /// **못 푼 옛 한 파일은 접지 않고 읽는 쪽에 넘긴다**(리뷰) — 목록에서 빼면 읽는 쪽이 그 자리를 못 만나
    /// 못 읽은 까닭([`journal_unread`])도 안 선다. 제 자리를 가리키는 링크는 `metadata` 도 푸는 것도 ELOOP 로
    /// 지는데, 그 이력이 빠진 화면이 stderr 한 줄 없이 0 으로 끝나면 moai-6ney 가 막은 조용한 손실이다.
    ///
    /// `journal/` 이 있어야 접는 걸음까지 간다 — 없으면 [`Repo::journal_files`] 가 그 앞에서 돌아온다.
    #[cfg(unix)]
    #[test]
    fn an_old_journal_that_cannot_be_resolved_is_still_told() {
        let (r, d) = repo("journal-loop");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &crate::model::someone("raven"))], ()))
            },
        )
        .unwrap();
        std::os::unix::fs::symlink("journal.jsonl", r.journal_path()).unwrap();

        let got = r.journal_of("argos-4aex");
        assert_eq!(got.len(), 1, "제 파일의 이력을 잃었다: {got:?}");
        let told = unread_under(d.path());
        assert!(told.iter().any(|u| u.at == r.journal_path()), "못 푼 옛 한 파일을 말없이 뺐다 — {told:?}");
    }

    /// **`.` 로 시작하는 이름은 저널이 아니다**(moai-o4jt). Emacs 는 저장하지 않은 채 연 파일 곁에 잠금
    /// 링크 `.#<이름>.jsonl -> user@host.PID:BOOT` 를 세운다. `.jsonl` 거르개를 지나는데 가리키는 곳이
    /// 없는 링크라 `metadata` 가 졌고, 그것을 못 읽은 저널로 세어 빠진 줄이 하나도 없는데 1 로 끝났다.
    ///
    /// macOS 가 까는 AppleDouble(`._<이름>.jsonl`)도 함께 잰다 — 이쪽은 보통 파일이라 못 읽은 것으로는
    /// 안 서지만, 그 안을 저널 줄로 풀 까닭도 없다. `.#` 만 거르면 `files` 를 견주는 줄이 붉어진다.
    #[cfg(unix)]
    #[test]
    fn a_hidden_name_beside_the_journal_is_not_a_journal() {
        let (r, d) = repo("journal-hidden");
        let by = crate::model::someone("raven");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &by)], ()))
            },
        )
        .unwrap();
        let name = journal_file(&by.email).unwrap();
        let mine = r.journal_dir().join(&name);
        let lock = r.journal_dir().join(format!(".#{name}"));
        std::os::unix::fs::symlink("raven@host.4242:1727740800", &lock).unwrap();
        std::fs::write(r.journal_dir().join(format!("._{name}")), b"\x00\x05\x16\x07\x00\x02\x00\x00Mac OS X").unwrap();

        let files = r.journal_files();
        let hist = r.journal_of("argos-4aex");
        let told = unread_under(d.path());
        assert!(told.is_empty(), "편집기의 잠금 링크를 못 읽은 저널로 셌다 — {told:?}");
        assert_eq!(files, [r.journal_path(), mine], "숨은 이름을 저널로 들였다");
        assert_eq!(hist.len(), 1, "제 이력을 잃었다");
    }

    /// **메일이 없으면 아무것도 안 쓴다**(moai-nzlo, 2026-09-21 사용자 결정). `unknown.jsonl` 도,
    /// 이름으로 지은 파일도 두지 않는다 — 이력이 남는 것이 목적인 파일이라 주인 없는 줄을
    /// 채우느니 한 번 물어보는 편이 싸다.
    ///
    /// **스냅샷도 안 나간다.** 쓴 뒤에 알면 그 거절이 "썼지만 이력은 못 남겼다" 로 떨어져,
    /// 막자는 결정이 알림 한 줄로 주저앉는다.
    #[test]
    fn a_journal_line_without_an_email_writes_nothing() {
        let (r, d) = repo("nomail");
        let by = crate::model::someone("raven");
        let e = r
            .with_write(
                || crate::i18n::Lang::Ko,
                |i, _, _| {
                    i.push(issue("argos-4aex"));
                    let mut j = JournalEntry::create("argos-4aex", "t", T, &by);
                    j.by_email = None;
                    Ok((vec![j], ()))
                },
            )
            .expect_err("메일 없는 줄을 받아 적었다");
        assert_eq!(e.code, code::NO_ACTOR, "{}", e.message);
        assert!(e.message.contains("argos-4aex"), "어느 줄인지 안 댄다 — {}", e.message);
        assert!(r.read().unwrap().issues.is_empty(), "스냅샷이 나갔다");
        assert!(!d.join(".moai/journal").exists(), "빈 자리를 지었다");
    }

    /// 스냅샷을 안 바꾸는 기록(`note`)도 저널에는 남아야 한다.
    #[test]
    fn journal_only_writes_still_land() {
        let (r, _d) = repo("note");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![], ()))
            },
        )
        .unwrap();
        r.with_write(
            || crate::i18n::Lang::Ko,
            |_, _, _| Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &crate::model::someone("raven"))], ())),
        )
        .unwrap();
        let j = r.journal_of("argos-4aex");
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].text.as_deref(), Some("발견"));
    }

    /// **`kind` 로 고른 줄은 파일을 가로질러 `ts` 차례로 선다**(moai-7dmq). 지운 id 는 부르는 쪽이
    /// 모르므로 `journal_by_id` 로는 못 물어, `show --removed` 가 이것을 부른다.
    ///
    /// 한 번에 넷을 잰다 — 글이 `rm` 인 `note` 와 제목이 `rm` 인 `create` 는 앞 거르개를 지나도 `kind` 가
    /// 달라 빠지고, `"rm"` 을 든 채 깨진 줄은 파서에서 넘어져도 파일 하나를 넘어뜨리지 않되 그 자리를 곁에 내고
    /// (리뷰 — 앞 거르개에서 걸러지는 깨진 줄로는 파서의 관대함을 못 잰다, moai-g8ho), 같은 `ts` 는 파일 차례(옛 한
    /// 파일이 먼저)를 지키고,
    /// `kind` 를 `\u` 로 적은 손 줄도 고른다(리뷰).
    #[test]
    fn journal_of_kind_picks_one_kind_across_files_in_ts_order() {
        let (r, d) = repo("ofkind");
        let by = crate::model::someone("raven");
        let line = |e: &JournalEntry| serde_json::to_string(e).unwrap();
        let old = [
            line(&JournalEntry::removed("argos-0003", "셋", "2026-09-12T00:00:00Z", &by)),
            line(&JournalEntry::note("argos-0009", "rm", "2026-09-10T00:00:00Z", &by)),
            "{\"ts\":\"2026-09-1\",\"kind\":\"rm\"".to_string(),
            line(&JournalEntry::removed("argos-0002", "둘", "2026-09-11T00:00:00Z", &by)),
        ];
        std::fs::write(r.journal_path(), old.join("\n") + "\n").unwrap();
        let split = [
            line(&JournalEntry::removed("argos-0001", "하나", "2026-09-10T00:00:00Z", &by)),
            line(&JournalEntry::create("argos-0004", "rm", "2026-09-09T00:00:00Z", &by)),
            line(&JournalEntry::removed_line(Some("argos-0005"), "{깨진", "2026-09-11T00:00:00Z", &by)),
            // `kind` 의 `m` 을 `\u` 이스케이프로 적은 손 줄이다. 역슬래시는 글자로 끼운다 — 소스에 이스케이프를
            // 그대로 적으면 편집기에 따라 풀려, 시험이 아무것도 안 잰다.
            format!(
                r#"{{"ts":"2026-09-13T00:00:00Z","id":"argos-0006","kind":"r{bs}u006d","by":"손","title":"이스케이프"}}"#,
                bs = '\\'
            ),
        ];
        std::fs::create_dir_all(d.join(".moai/journal")).unwrap();
        std::fs::write(d.join(".moai/journal").join(journal_file(&by.email).unwrap()), split.join("\n") + "\n")
            .unwrap();

        let (rows, garbled) = r.journal_of_kind("rm");
        let got: Vec<(String, String)> = rows.into_iter().map(|e| (e.ts, e.id)).collect();
        assert_eq!(
            got,
            [
                ("2026-09-10T00:00:00Z".to_string(), "argos-0001".to_string()),
                ("2026-09-11T00:00:00Z".to_string(), "argos-0002".to_string()),
                ("2026-09-11T00:00:00Z".to_string(), "argos-0005".to_string()),
                ("2026-09-12T00:00:00Z".to_string(), "argos-0003".to_string()),
                ("2026-09-13T00:00:00Z".to_string(), "argos-0006".to_string()),
            ],
            "kind·차례·관대함·이스케이프 가운데 하나가 어긋났다"
        );
        assert_eq!(
            garbled,
            [Garbled { at: r.journal_path(), line: 3, ts: Some("2026-09-1".into()) }],
            "못 푼 rm 줄의 자리를 안 댔다"
        );
        let (rows, garbled) = r.journal_of_kind("status");
        assert!(rows.is_empty(), "없는 kind 에서 줄을 지어냈다");
        assert!(garbled.is_empty(), "다른 갈래를 댄 조각을 댔다: {garbled:?}");
    }

    /// **못 푼 조각은 제 입으로 댄 `kind` 로 가른다**(moai-g8ho, 리뷰). 앞 거르개(`"rm"`·`\u`)로 가르던 판은 두
    /// 쪽으로 틀렸다 — `"rm"` 을 다 적기 전에 끊긴 줄은 말없이 빠졌고, `\u` 가 든 끊긴 노트는 지움으로 불렸다.
    /// 붙은 줄은 줄 머리로 갈라 온전한 쪽을 되찾는다. 깨진 글자는 `�` 로 바꾼 글로 **가르기만** 하고 풀지는
    /// 않는다 — 꼴이 온전한 줄이라도 그 글로 풀면 망가진 제목이 목록과 이력에 선다.
    #[test]
    fn a_garbled_line_that_may_hold_the_kind_is_named() {
        let (r, _d) = repo("ofkind-garbled");
        let by = crate::model::someone("raven");
        let line = |e: &JournalEntry| serde_json::to_string(e).unwrap();
        let whole = line(&JournalEntry::removed("argos-0001", "하나", T, &by));
        let glued = [
            &b"{\"ts\":\"2026-09-16T00:00:00Z\",\"id\":\"argos-0008\",\"kind\":\"no"[..],
            line(&JournalEntry::removed("argos-0009", "아홉", "2026-09-16T00:00:00Z", &by)).as_bytes(),
        ]
        .concat();
        // 앞 거르개를 못 지나는 붙은 줄 둘 — `kind` 값 가운데서, 또는 `kind` 에 닿기 전에 끊긴 `rm` 에 노트가 붙었다.
        let note = line(&JournalEntry::note("argos-0013", "x", "2026-09-20T00:00:00Z", &by));
        let onto_value =
            [&b"{\"ts\":\"2026-09-18T00:00:00Z\",\"id\":\"argos-0011\",\"kind\":\"r"[..], note.as_bytes()].concat();
        let onto_key = [&b"{\"ts\":\"2026-09-19T00:00:00Z\",\"id\":\"argos-0012\",\""[..], note.as_bytes()].concat();
        let mut bytes = Vec::new();
        for l in [
            // 1: 끊긴 노트 — 제 `kind` 가 노트라 안 댄다.
            &b"{\"ts\":\"2026-09-10\",\"kind\":\"note\",\"text\":\"x\""[..],
            // 2: 글자가 깨진 채 끊긴 `rm` — 댄다.
            b"{\"ts\":\"2026-09-10\",\"kind\":\"rm\",\"title\":\"\xed\x95",
            // 3: 온전한 줄.
            whole.as_bytes(),
            // 4: 끊긴 노트에 붙은, 필드가 모자란 `rm` — 앞 토막은 노트, 뒤 토막은 `rm` 이라 댄다.
            b"{\"ts\":\"2026-09-11\",\"kind\":\"no{\"ts\":\"2026-09-12\",\"kind\":\"rm\"}",
            // 5: `"rm"` 을 다 적기 전에 끊긴 줄 — 앞 거르개를 못 지나도 댄다.
            b"{\"ts\":\"2026-09-13T00:00:00Z\",\"id\":\"argos-0005\",\"kind\":\"r",
            // 6: `kind` 에 닿기 전에 끊긴 줄 — 무엇이었는지 모르니 댄다.
            b"{\"ts\":\"2026-09-14T00:00:00Z\",\"id\":\"argos-0006\",\"",
            // 7: `\u` 가 든 끊긴 노트 — 앞 거르개는 지나도 제 `kind` 가 노트라 안 댄다.
            b"{\"ts\":\"2026-09-15T00:00:00Z\",\"id\":\"argos-0007\",\"kind\":\"note\",\"by\":\"x\",\"text\":\"\\u001b[31mre",
            // 8: 옛 바이너리가 끊긴 노트에 붙여 쓴 온전한 `rm` — 되찾아 목록에 세운다.
            &glued,
            // 9: 꼴은 온전한데 제목의 한 바이트가 깨진 `rm` — 풀지 않고 댄다.
            b"{\"ts\":\"2026-09-17T00:00:00Z\",\"id\":\"argos-0010\",\"kind\":\"rm\",\"by\":\"x\",\"title\":\"\xff\"}",
            // 10·11: 끊긴 `rm` 에 붙은 노트 — 줄은 `}` 로 닫혀도 앞 토막을 댄다.
            &onto_value,
            &onto_key,
        ] {
            bytes.extend_from_slice(l);
            bytes.push(b'\n');
        }
        std::fs::write(r.journal_path(), bytes).unwrap();
        let (rows, garbled) = r.journal_of_kind("rm");
        let ids: Vec<&str> = rows.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["argos-0001", "argos-0009"], "붙은 줄의 온전한 쪽을 못 되찾았거나 깨진 글자를 풀었다");
        let named: Vec<(usize, Option<&str>)> = garbled.iter().map(|g| (g.line, g.ts.as_deref())).collect();
        assert_eq!(
            named,
            [
                (2, Some("2026-09-10")),
                (4, Some("2026-09-12")),
                (5, Some("2026-09-13T00:00:00Z")),
                (6, Some("2026-09-14T00:00:00Z")),
                (9, Some("2026-09-17T00:00:00Z")),
                (10, Some("2026-09-18T00:00:00Z")),
                (11, Some("2026-09-19T00:00:00Z")),
            ],
            "지움을 들었을 수 있는 조각만, 그 도장과 함께 대야 한다"
        );
        assert_eq!(r.journal_of("argos-0001").len(), 1, "이력 쪽 읽기가 달라졌다");
        assert_eq!(r.journal_of("argos-0009").len(), 1, "붙은 줄을 이력이 되찾지 못했다");
    }

    /// **`\n` 없이 끝난 줄이 조각을 낼 때만 다시 읽을 까닭이 선다**(리뷰) — 읽기는 락을 안 잡아, 옆 세션이
    /// 덧붙이는 도중의 줄이 끝에 반쯤 보일 수 있다. 채운 꼬리 뒤의 조각, 끝의 온전한 줄, 조각을 안 묻는 읽기는
    /// 까닭이 아니다.
    #[test]
    fn only_a_torn_last_line_asks_for_a_second_read() {
        let full = r#"{"ts":"2026-09-01T00:00:00Z","id":"argos-0001","kind":"rm","by":"x","title":"t"}"#;
        let half = r#"{"ts":"2026-09-02T00:00:00Z","id":"argos-0002","kind":"rm","by":"Te"#;
        let pick = |l: &str| l.contains("\"rm\"");
        let open = |text: String, asks: bool| scan(text.as_bytes(), &pick, asks).open_tail;
        assert!(open(format!("{full}\n{half}"), true), "반쯤 적힌 끝줄을 다시 안 읽는다");
        assert!(!open(format!("{half}\n{full}"), true), "채운 꼬리 뒤의 조각으로 다시 읽는다");
        assert!(!open(format!("{full}\n{half}\n"), true), "`\\n` 으로 끝난 조각으로 다시 읽는다");
        assert!(!open(full.to_string(), true), "`\\n` 없이 끝난 온전한 줄로 다시 읽는다");
        assert!(!open(format!("{full}\n{half}"), false), "조각을 안 묻는 읽기가 다시 읽는다");
    }

    /// **끊긴 꼬리 뒤에 적은 줄은 제 줄에 선다**(moai-a65c). 저널이 `\n` 없이 끝난 뒤(디스크가 찼거나 덧붙이다
    /// 죽었다) 다음 `rm` 이 그 끝에 붙어, 끊긴 줄과 새 줄이 한 줄이 되고 `show --removed` 에서 그 지움이 빠지던
    /// 자리다. 끊긴 줄은 끊긴 채 남고, 새 줄은 이력에도 `kind` 읽기에도 선다.
    #[test]
    fn a_journal_line_after_a_torn_tail_stands_on_its_own_line() {
        let (r, d) = repo("torn-tail");
        let by = crate::model::someone("raven");
        let file = d.join(".moai/journal").join(journal_file(&by.email).unwrap());
        std::fs::create_dir_all(crate::path::dir_of(&file)).unwrap();
        let whole = serde_json::to_string(&JournalEntry::note("argos-0001", "앞", T, &by)).unwrap();
        let torn = r#"{"ts":"2026-09-10T00:00:00Z","id":"argos-0002","kind":"no"#;
        std::fs::write(&file, format!("{whole}\n{torn}")).unwrap();
        r.with_write(
            || crate::i18n::Lang::Ko,
            |_, _, _| Ok((vec![JournalEntry::removed("argos-0003", "셋", T, &by)], ())),
        )
        .unwrap();
        let removed: Vec<String> = r.journal_of_kind("rm").0.into_iter().map(|e| e.id).collect();
        assert_eq!(removed, ["argos-0003"], "끊긴 꼬리 뒤의 rm 줄이 그 꼬리에 붙었다");
        assert_eq!(r.journal_of("argos-0001").len(), 1, "앞의 온전한 줄을 잃었다");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with(&format!("{whole}\n{torn}\n{{")), "끊긴 줄을 고쳐 썼거나 빈 줄을 더 세웠다\n{text}");
    }

    /// [`append_inside`] 는 **끝이 `\n` 이 아닐 때만** 채운다 — 빈 파일과 새 파일, 온전히 끝난 파일에는 한
    /// 바이트도 더하지 않는다. 채우는 자리가 둘이던 판(`init` 도 채웠다)은 빈 줄이 하나 더 섰다.
    #[test]
    fn append_inside_heals_a_torn_tail_and_nothing_else() {
        let s = Scratch::new("store-append-torn");
        let dir = s.join("repo");
        std::fs::create_dir_all(&dir).unwrap();
        let at = |name: &str, before: Option<&str>| {
            let p = dir.join(name);
            if let Some(b) = before {
                std::fs::write(&p, b).unwrap();
            }
            append_inside(&p, b"new\n", &dir).unwrap();
            std::fs::read_to_string(&p).unwrap()
        };
        assert_eq!(at("fresh", None), "new\n");
        assert_eq!(at("empty", Some("")), "new\n");
        assert_eq!(at("whole", Some("old\n")), "old\nnew\n");
        assert_eq!(at("torn", Some("old")), "old\nnew\n");
        assert_eq!(at("crlf", Some("old\r")), "old\r\nnew\n");
    }

    /// **읽기가 안 되는 파일에도 전처럼 덧붙인다**(리뷰) — 꼬리를 보려고 읽기까지 열던 판은 쓰기만 되는
    /// 파일(`0200`)에서 덧붙이기 자체를 잃었다. 꼬리를 못 보니 채우지는 않는다. 읽기 권한을 안 따지는 사용자
    /// (root)로 돌면 막힌 자리를 못 지으니 건너뛴다.
    #[cfg(unix)]
    #[test]
    fn append_inside_still_appends_to_a_file_it_cannot_read() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("store-append-writeonly");
        let dir = s.join("repo");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("w.jsonl");
        std::fs::write(&p, "old").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o200)).unwrap();
        if std::fs::read(&p).is_ok() {
            return;
        }
        let got = append_inside(&p, b"new\n", &dir);
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        got.expect("읽기가 안 되는 파일에 덧붙이기를 잃었다");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "oldnew\n");
    }

    /// **옛 한 파일 자리가 보통 파일이 아니면 안 연다**(리뷰 moai-wcy8.rbj) — `journal/` 의 것과 같은 자다.
    /// FIFO 를 여는 `fs::read` 는 쓰는 쪽이 올 때까지 영영 멈추고, 탐색기는 `/` 가 노트를 처음 볼 때 이 읽기를
    /// 루프에서 부른다. 제 파일의 이력은 그대로 온다. `mkfifo` 가 없는 기계면 건너뛴다(`write_atomic` 의 시험과
    /// 같은 길이다).
    #[cfg(unix)]
    #[test]
    fn a_fifo_where_the_old_journal_was_is_not_opened() {
        let (r, _d) = repo("journal-fifo");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &crate::model::someone("raven"))], ()))
            },
        )
        .unwrap();
        if !std::process::Command::new("mkfifo").arg(r.journal_path()).status().is_ok_and(|st| st.success()) {
            return;
        }
        let reader = Repo::at(r.root.clone(), r.config.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(reader.journal_of("argos-4aex"));
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(5));
        if got.is_err() {
            // 멈춘 읽기를 풀어 준다 — 쓰는 쪽이 열었다 닫으면 읽는 쪽은 끝을 본다.
            let _ = std::fs::OpenOptions::new().write(true).open(r.journal_path());
        }
        let j = got.expect("옛 저널 자리의 FIFO 에서 읽기가 멈췄다");
        assert!(j.iter().any(|e| e.text.as_deref() == Some("발견")), "제 파일의 이력을 잃었다: {j:?}");
        assert!(!r.journal_marks().iter().any(|(p, _)| *p == r.journal_path()), "FIFO 를 저널로 쟀다");
    }

    /// **저널만 못 적은 쓰기는 담긴 것으로 끝나고, 저널이 전부인 쓰기는 실패다**(moai-52z9).
    /// 앞의 것을 `Err` 로 내면 다시 부른 `add` 가 같은 이슈를 하나 더 세운다.
    #[cfg(unix)]
    #[test]
    fn a_write_whose_journal_fails_still_lands_but_a_journal_only_one_does_not() {
        use std::os::unix::fs::PermissionsExt;
        let (r, d) = repo("journalfail");
        // **막는 것은 자리다**(moai-nzlo) — 저널은 이제 `.moai/journal/<메일>.jsonl` 이라 파일
        // 하나를 잠가도 첫 쓰기가 옆에 새 이름으로 연다. 디렉터리를 잠그면 그 안에 못 짓는다.
        let journal = d.join(".moai/journal");
        std::fs::create_dir_all(&journal).unwrap();
        std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::File::create(journal.join("probe")).is_ok() {
            return; // root 는 권한을 안 본다 — 재현이 안 되는 자리다
        }

        let by = crate::model::someone("raven");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![JournalEntry::create("argos-4aex", "t", T, &by)], ()))
            },
        )
        .expect("스냅샷을 썼는데 실패로 냈다 — 다시 부르면 둘 선다");
        assert_eq!(r.read().unwrap().issues.len(), 1);
        assert!(journal_misses().iter().any(|(root, _)| root == d.path()), "못 남긴 것을 안 셌다");

        let e = r.with_write(
            || crate::i18n::Lang::Ko,
            |_, _, _| Ok((vec![JournalEntry::note("argos-4aex", "발견", T, &by)], ())),
        );
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
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![], ()))
            },
        )
        .expect("깨진 줄 하나가 쓰기를 막았다");

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("{깨짐"), "모르는 줄을 잃었다 — {after}");
        assert!(after.contains("argos-4aex"), "쓰겠다고 해 놓고 안 썼다 — {after}");

        // 두 번째 쓰기에서 줄이 또 움직이지 않는다 (멱등).
        let once = std::fs::read_to_string(&path).unwrap();
        r.with_write(|| crate::i18n::Lang::Ko, |_, _, _| Ok((vec![], ()))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    }

    #[test]
    fn refuses_duplicate_ids() {
        let (r, _d) = repo("dup");
        let e = r
            .with_write(
                || crate::i18n::Lang::Ko,
                |i, _, _| {
                    i.push(issue("argos-4aex"));
                    i.push(issue("argos-4aex"));
                    Ok((vec![], ()))
                },
            )
            .unwrap_err()
            .message;
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
        std::fs::write(d.join(".moai/issues.jsonl"), format!("{}\n", serde_json::to_string(&old).unwrap())).unwrap();

        r.with_write(
            || crate::i18n::Lang::Ko,
            |issues, _, _| {
                issues.push(issue("argos-0002"));
                Ok((vec![], ()))
            },
        )
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
        std::fs::write(d.join(".moai/issues.jsonl"), format!("{}\n", serde_json::to_string(&old).unwrap())).unwrap();

        r.with_write(
            || crate::i18n::Lang::Ko,
            |issues, _, _| {
                issues[0].title = "고친 제목".into();
                Ok((vec![], ()))
            },
        )
        .expect("옛 칸에 선 줄의 제목을 못 고쳤다");
        assert_eq!(r.read().unwrap().get("argos-0001").unwrap().title, "고친 제목");
        assert_eq!(r.read().unwrap().get("argos-0001").unwrap().status.as_str(), "옛날칸");

        let e = r
            .with_write(
                || crate::i18n::Lang::Ko,
                |issues, _, _| {
                    issues[0].status = Status::new("또 없는 칸");
                    Ok((vec![], ()))
                },
            )
            .unwrap_err()
            .message;
        assert!(e.contains("라는 칸이 없다"), "{e}");
    }

    /// 쓰기 도중 실패하면 원본이 그대로다 — 반쯤 쓰인 파일이 남지 않는다.
    #[test]
    fn a_rejected_write_leaves_the_file_untouched() {
        let (r, d) = repo("rollback");
        r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aex"));
                Ok((vec![], ()))
            },
        )
        .unwrap();
        let before = std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap();
        let _ = r.with_write(
            || crate::i18n::Lang::Ko,
            |i, _, _| {
                i.push(issue("argos-4aey"));
                i[0].status = Status::new("없는칸");
                Ok((vec![], ()))
            },
        );
        assert_eq!(std::fs::read_to_string(d.join(".moai/issues.jsonl")).unwrap(), before);
    }
    /// 못 읽는 줄도 **id 는 내놓는다.** 줄을 `Issue` 로 못 읽는 것과 그 안의
    /// `id` 를 못 읽는 것은 다른 일이다 — 한 단 낮게 읽으면 나온다.
    ///
    /// **id 를 읽는 자는 하나다**(moai-ijfy). `crate::id::id_of` 가 `serde_json` 한 겹
    /// 아래로 내려가고, 그것도 진 줄은 머리에서 긁는다 — 머지 드라이버가 짝짓는 자와
    /// 같은 자라 둘이 갈릴 자리가 없다.
    #[test]
    fn an_unreadable_line_still_yields_its_id() {
        let load = parse_issues("{\"id\":\"argos-9999\",\"title\":\"몰라\",\"kind\":\"몰라\",\"status\":\"todo\"}\n");
        assert_eq!(load.errors.len(), 1);
        assert_eq!(load.errors[0].id.as_deref(), Some("argos-9999"));
        assert_eq!(load.reserved_ids().iter().next().map(String::as_str), Some("argos-9999"));
    }

    /// **꼬리가 잘려 JSON 이 진 줄도 id 는 내놓는다**(moai-ijfy). 머지 드라이버는 그 머리를
    /// 긁어 짝짓는데 여기가 못 보던 동안, 산 줄과 그 줄의 깨진 쌍둥이가 함께 선 파일에서
    /// [`Load::reserved_ids`] 가 그 id 를 안 잡아 두고(`id::generate` 가 다시 지을 수 있었다)
    /// `report` 의 `duplicate_id` 도 그 id 를 못 댔다 — `moai status` 는 `Unreadable rows` 만
    /// 말했고, 사람은 어느 줄을 지워야 하는지 들을 데가 없었다.
    #[test]
    fn a_broken_twin_of_a_live_row_is_reserved_and_shows_up_as_a_duplicate() {
        let live = serde_json::to_string(&issue("argos-0001")).unwrap();
        let twin = live.strip_suffix('}').expect("쓴 줄은 `}` 로 끝난다");
        let load = parse_issues(&format!("{live}\n{twin}\n"));
        assert_eq!(load.issues.len(), 1);
        assert_eq!(load.errors[0].id.as_deref(), Some("argos-0001"), "머지 드라이버가 짝지은 id 를 못 본다");
        assert!(load.reserved_ids().contains("argos-0001"), "새 id 가 이 id 를 다시 지을 수 있다");
        // `report` 의 `duplicate_id` 가 읽는 자리도 같은 id 를 든다 — 그쪽에서 산 줄과 견주는
        // 것은 `an_unreadable_line_reusing_a_live_id_is_a_duplicate` 가 잰다.
        assert_eq!(load.unreadable()[0].id, Some("argos-0001"), "겹쳤다고 말해 주는 자가 이 줄을 못 본다");
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
            .with_write(
                || crate::i18n::Lang::Ko,
                |issues, cfg, reserved| {
                    assert!(reserved.contains("argos-9999"), "못 읽는 줄의 id 를 안 줬다 — {reserved:?}");
                    // 그 줄의 id 를 그대로 노리는 씨앗이라도 다른 것이 나와야 한다.
                    let taken = taken_ids(issues, reserved);
                    let id = crate::id::generate(&cfg.prefix, &taken, "argos-9999");
                    issues.push(issue(&id));
                    Ok((vec![], id))
                },
            )
            .unwrap();
        assert_ne!(minted, "argos-9999", "못 읽는 줄과 같은 id 를 뽑았다");
    }

    /// 등록한 디렉터리 하나를 열 때 넷을 가른다 — 연 것, init 전, 사라짐, 깨짐.
    #[test]
    fn open_tells_uninit_missing_and_broken_apart() {
        let root = scratch("open");
        assert!(matches!(Repo::open(&root, || crate::i18n::Lang::Ko), Ok(Opened::Repo(r)) if r.root == *root.path()));

        let bare = root.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        assert!(matches!(Repo::open(&bare, || crate::i18n::Lang::Ko), Ok(Opened::Uninit)));
        // 위로 찾지 않는다 — 바깥 저장소의 `.moai` 를 제 것으로 내면 안 된다.
        assert!(matches!(Repo::open(&bare.join("gone"), || crate::i18n::Lang::Ko), Ok(Opened::Missing)));
        // 경로 중간이 파일이어도 없는 것이다.
        std::fs::write(root.join("file"), "").unwrap();
        assert!(matches!(Repo::open(&root.join("file/sub"), || crate::i18n::Lang::Ko), Ok(Opened::Missing)));
        assert!(Repo::open(&root.join("file"), || crate::i18n::Lang::Ko).is_err(), "파일을 디렉터리로 열었다");

        let broken = root.join("broken");
        std::fs::create_dir_all(broken.join(".moai")).unwrap();
        std::fs::write(broken.join(".moai/config.toml"), "prefix = \"\"\n").unwrap();
        assert!(Repo::open(&broken, || crate::i18n::Lang::Ko).is_err(), "깨진 설정을 init 전으로 접었다");
    }

    /// **임시 이름은 스레드마다 다르고 한 스레드 안에서는 늘 같다**(moai-mpf4). pid 하나였을 때는
    /// 스레드 둘이 같은 파일을 쓰면 임시 경로가 겹쳐, 한쪽이 아직 쓰는 중인 파일을 다른 쪽이 들고
    /// 갔다. 이름이 스레드마다 갈리는 것과 한 스레드 안에서 안 바뀌는 것을 함께 잰다 — 뒤엣것이
    /// 깨지면 임시 자리를 미리 막아 두고 재는 시험
    /// (`cmd::init` 의 `root_files_are_swapped_through_a_temp_file_in_dot_moai`)이 선 바닥이
    /// 무너진다.
    #[test]
    fn the_temp_name_is_one_per_thread_and_never_shared() {
        let mine = tmp_name("issues.jsonl");
        assert_eq!(mine, tmp_name("issues.jsonl"), "같은 스레드인데 이름이 바뀐다");
        assert_ne!(mine, tmp_name("다른파일"), "파일이 다른데 이름이 같다");
        let theirs = std::thread::spawn(|| tmp_name("issues.jsonl")).join().unwrap();
        assert_ne!(mine, theirs, "스레드가 다른데 임시 이름이 같다");
    }

    /// **임시 이름에 남의 파일이 있으면 그것을 안 건드리고 다음 이름으로 간다**(moai-ydm7.976).
    /// pid 가 같은 두 프로세스(같은 마운트의 컨테이너 둘)가 한 이름을 쓰는 경우를 그 이름에 파일을
    /// 먼저 두어 세운다. `File::create` 로 열던 때는 그 파일을 비우고 제 글을 써 들고 갔다 —
    /// 앞쪽이 아직 쓰는 중이었다면 어느 쪽도 아닌 글이 대상에 실린다.
    #[test]
    fn a_temp_name_already_taken_is_left_alone() {
        let s = crate::scratch::Scratch::new("tmp-excl");
        let target = s.join("issues.jsonl");
        std::fs::write(&target, "옛\n").unwrap();
        let theirs = s.join(tmp_name("issues.jsonl"));
        std::fs::write(&theirs, "남이 쓰는 중\n").unwrap();

        write_atomic(&target, "새\n".as_bytes()).expect("첫 이름이 막혔다고 쓰기가 실패했다");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "새\n");
        assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "남이 쓰는 중\n", "남의 임시 파일을 건드렸다");
        let mut left: Vec<_> = std::fs::read_dir(s.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        left.sort();
        assert_eq!(left.len(), 2, "제 임시 파일을 남겼다: {left:?}");
    }

    /// **이름이 다 막히면 쓰기는 실패하고 아무것도 안 바뀐다** — 대상도, 남의 파일도. 막힌 이름을
    /// 지우고 쓰면 살아 있는 남의 쓰기를 부순다.
    #[test]
    fn every_temp_name_taken_fails_without_touching_anything() {
        let s = crate::scratch::Scratch::new("tmp-excl-full");
        let target = s.join("issues.jsonl");
        std::fs::write(&target, "옛\n").unwrap();
        let names: Vec<String> = tmp_names("issues.jsonl").collect();
        assert_eq!(names.len(), TMP_TRIES);
        assert_eq!(names[0], tmp_name("issues.jsonl"), "첫 이름이 예전 이름과 갈렸다");
        for n in &names {
            std::fs::write(s.join(n), "남\n").unwrap();
        }

        assert!(write_atomic(&target, "새\n".as_bytes()).is_err(), "막힌 이름을 덮고 썼다");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "옛\n");
        for n in &names {
            assert_eq!(std::fs::read_to_string(s.join(n)).unwrap(), "남\n", "{n} 을 건드렸다");
        }
    }

    /// **pid 가 같은 죽은 쓰기의 찌꺼기는 쓰기를 막지 못한다.** 컨테이너의 PID 1 처럼 pid 가 늘
    /// 같은 자리에서는 쓰다 죽을 때마다 pid·번호로 지은 이름이 하나씩 남고, 아무도 안 지운다
    /// ([`open_tmp`]). 둘째 이름부터 pid·번호만으로 짓던 때는 그런 찌꺼기 여덟 개(`<첫 이름>`,
    /// `<첫 이름>.1`…`.7`)가 그 pid 의 쓰기를 `File exists` 로 영영 막았다 — 사람이 손으로
    /// 지워야만 풀렸다. 둘째 이름부터는 이 프로세스만의 것이라 그 찌꺼기와 겹치지 않는다.
    #[test]
    fn leftovers_of_a_dead_writer_with_the_same_pid_never_block_a_write() {
        let s = crate::scratch::Scratch::new("tmp-excl-stale");
        let target = s.join("issues.jsonl");
        std::fs::write(&target, "옛\n").unwrap();
        let first = tmp_name("issues.jsonl");
        let stale: Vec<String> =
            std::iter::once(first.clone()).chain((1..TMP_TRIES).map(|n| format!("{first}.{n}"))).collect();
        for n in &stale {
            std::fs::write(s.join(n), "죽은 쓰기\n").unwrap();
        }

        write_atomic(&target, "새\n".as_bytes()).expect("죽은 쓰기의 찌꺼기에 막혔다");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "새\n");
        for n in &stale {
            assert_eq!(std::fs::read_to_string(s.join(n)).unwrap(), "죽은 쓰기\n", "{n} 을 건드렸다");
        }
        // 둘째 이름부터는 이 프로세스만의 수가 든다 — 같은 프로세스 안에서는 늘 같다.
        let names: Vec<String> = tmp_names("issues.jsonl").collect();
        assert_eq!(names, tmp_names("issues.jsonl").collect::<Vec<_>>(), "한 프로세스 안에서 이름이 바뀐다");
        assert!(names[1..].iter().all(|n| !stale.contains(n)), "둘째 이름이 pid·번호만으로 지어졌다: {names:?}");
    }

    /// **스레드 둘이 같은 파일을 함께 써도 반쪽 글이 남지 않는다**(moai-mpf4). 조용한 손실이 이
    /// 도구가 못 견디는 하나뿐인 실패 모드라(CLAUDE.md), 늦은 쪽이 이기는 것만 약속하고
    /// **둘 중 하나가 통째로** 남는 것을 잰다. 겹치던 때는 한쪽의 `rename` 이 다른 쪽이 아직
    /// 쓰는 중인 임시 파일을 들고 가, 어느 쪽도 아닌 글이 대상에 실릴 수 있었다.
    #[test]
    fn two_threads_writing_one_file_never_leave_half_a_line() {
        let s = Scratch::new("store-atomic-threads");
        let path = s.join("held.toml");
        // 길게 적는다 — 한 번의 `write_all` 로 안 끝나야 찢어지는 자리가 실제로 생긴다.
        let (a, b) = ("가".repeat(200_000), "나".repeat(200_000));
        std::thread::scope(|scope| {
            for text in [&a, &b] {
                scope.spawn(|| {
                    for _ in 0..8 {
                        write_atomic(&path, text.as_bytes()).expect("못 썼다");
                    }
                });
            }
        });
        let got = std::fs::read_to_string(&path).unwrap();
        assert!(got == a || got == b, "어느 쪽도 아닌 글이 남았다: {}바이트", got.len());
        // 찌꺼기도 안 남는다 — 스레드마다 이름이 갈려도 쓰고 나면 치운다.
        let left: Vec<_> = std::fs::read_dir(s.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from("held.toml")], "임시 파일이 남았다: {left:?}");
    }

    /// 그 자리가 심볼릭 링크인가 — 없으면 거짓이다. 링크를 시험하는 것들이 함께 쓴다(리뷰: 시험마다 같은
    /// 클로저(closure)를 따로 적고 있었다).
    #[cfg(unix)]
    fn is_link(p: &Path) -> bool {
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
    }

    /// 디렉터리에 선 이름들, 이름 차례로 — 찌꺼기를 세는 시험들이 함께 쓴다.
    #[cfg(unix)]
    fn names(d: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        v.sort();
        v
    }

    /// **링크를 따라가 가리키는 파일을 바꾸고 링크는 그대로 둔다**(moai-4oab). 사슬(링크의 링크)도,
    /// 임시 자리를 따로 주는 [`write_atomic_in`] 도 같다. 가리키는 파일의 권한도 지킨다 — 링크 곁에서
    /// 권한을 읽으면 링크가 아니라 대상의 것을 읽어야 한다.
    ///
    /// **임시 파일은 받은 철자 곁에서 먼저 난다**(리뷰). 찌꺼기만 세면 어디서 났는지 못 가른다 — 같은
    /// 파일시스템에서는 어느 쪽에서 나도 `rename` 이 치워 간다. 그래서 한쪽의 임시 이름을 다 막아 두고
    /// 쓴다: 가리키는 파일 곁을 막아도 써져야 받은 철자 곁에서 난 것이고, 받은 철자 곁을 막아도 써져야
    /// 가리키는 파일 곁으로 물러선 것이다 — `EXDEV` 의 물러섬을 한 파일시스템 안에서 시험하는 길이다.
    #[cfg(unix)]
    #[test]
    fn a_write_through_a_symlink_changes_the_target_and_keeps_the_link() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("store-atomic-link");
        std::fs::create_dir(s.join("dots")).unwrap();
        let real = s.join("dots/real");
        std::fs::write(&real, "옛\n").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mid = s.join("mid");
        std::os::unix::fs::symlink("dots/real", &mid).unwrap();
        let link = s.join("link");
        std::os::unix::fs::symlink(&mid, &link).unwrap();

        // 막은 것은 디렉터리라 그 이름으로는 파일을 못 짓는다. 이름은 가리키는 파일의 것이다.
        let blocked = |dir: &Path, text: &str| {
            let taken: Vec<_> = tmp_names("real").map(|n| dir.join(n)).collect();
            for t in &taken {
                std::fs::create_dir(t).unwrap();
            }
            let wrote = write_atomic(&link, text.as_bytes());
            for t in &taken {
                std::fs::remove_dir(t).unwrap();
            }
            wrote
        };
        blocked(&s.join("dots"), "새\n").expect("받은 철자 곁이 아니라 가리키는 파일 곁에서 임시 파일을 지으려 했다");
        assert!(is_link(&link) && is_link(&mid), "링크를 보통 파일로 갈아끼웠다");
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "새\n");
        assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600, "대상의 권한이 풀렸다");
        blocked(s.path(), "둘째\n").expect("받은 철자 곁에서 못 쓰자 가리키는 파일 곁으로 물러서지 않았다");
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "둘째\n");
        assert!(is_link(&link) && is_link(&mid), "물러선 쓰기가 링크를 갈아끼웠다");
        assert_eq!(names(&s.join("dots")), ["real"], "가리키는 파일 곁에 찌꺼기가 남았다");
        assert_eq!(names(s.path()), ["dots", "link", "mid"], "링크 곁에 찌꺼기가 남았다");

        std::fs::create_dir(s.join("tmp")).unwrap();
        write_atomic_in(&link, "셋째\n".as_bytes(), &s.join("tmp"), s.path()).unwrap();
        assert!(is_link(&link), "임시 자리를 따로 준 쓰기가 링크를 갈아끼웠다");
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "셋째\n");
    }

    /// **가리키는 파일이 아직 없으면 거기에 만든다** — dotfiles 는 링크를 먼저 걸기도 한다. 가리키는
    /// 디렉터리마저 없으면 쓰기는 **링크와 그 자리를 대며** 실패하고 링크는 그대로다 — 지은 적도 없는
    /// 임시 파일을 대던 거절문은 사람을 권한 고치기로 보냈다(리뷰). **고리인 링크는 거절한다** — 따라가다
    /// 멈춘 링크에 `rename` 하면 그것을 보통 파일로 갈아끼운다.
    #[cfg(unix)]
    #[test]
    fn a_dangling_or_looping_symlink_never_becomes_a_plain_file() {
        let s = Scratch::new("store-atomic-dangling");
        let link = s.join("link");
        std::os::unix::fs::symlink("later", &link).unwrap();
        write_atomic(&link, "처음\n".as_bytes()).unwrap();
        assert!(is_link(&link));
        assert_eq!(std::fs::read_to_string(s.join("later")).unwrap(), "처음\n");

        let gone = s.join("gone");
        std::os::unix::fs::symlink("nowhere/file", &gone).unwrap();
        let e = write_atomic(&gone, "x\n".as_bytes()).expect_err("없는 디렉터리를 가리키는 링크에 썼다");
        assert!(is_link(&gone), "실패한 쓰기가 링크를 갈아끼웠다");
        assert!(
            e.message.contains("nowhere/file") && e.message.contains("not there") && !e.message.contains(".tmp."),
            "링크가 가리키는 자리 대신 다른 것을 댔다: {}",
            e.message
        );

        let (a, b) = (s.join("a"), s.join("b"));
        std::os::unix::fs::symlink("b", &a).unwrap();
        std::os::unix::fs::symlink("a", &b).unwrap();
        assert!(write_atomic(&a, "x\n".as_bytes()).is_err(), "고리인 링크에 썼다");
        assert!(is_link(&a) && is_link(&b), "고리인 링크를 갈아끼웠다");
        assert_eq!(names(s.path()), ["a", "b", "gone", "later", "link"], "찌꺼기가 남았다");
    }

    /// **보통 파일이 아닌 자리는 갈아끼우지 않는다**(moai-4oab 리뷰). 링크를 따라가면서 링크의 끝이 장치·
    /// 소켓·FIFO 여도 쓸 자리가 됐는데, `rename` 은 그 자리를 보통 파일로 바꾼다 — `latest.toml ->
    /// /dev/null` 이 root 로 도는 컨테이너에서 `/dev/null` 을 없앤다. root 없이 지을 수 있는 FIFO 로
    /// 시험한다 — `mkfifo` 가 없는 기계면 그 갈래는 건너뛴다(`skill` 의 시험과 같은 길이다).
    #[cfg(unix)]
    #[test]
    fn a_write_never_replaces_what_is_not_a_regular_file() {
        use std::os::unix::fs::FileTypeExt;
        let s = Scratch::new("store-atomic-fifo");
        std::fs::create_dir(s.join("dir")).unwrap();
        let e = write_atomic(&s.join("dir"), b"x\n").expect_err("디렉터리를 갈아끼우려 했다");
        assert!(e.message.contains("not a regular file"), "{}", e.message);
        let fifo = s.join("fifo");
        if std::process::Command::new("mkfifo").arg(&fifo).status().is_ok_and(|st| st.success()) {
            let link = s.join("latest.toml");
            std::os::unix::fs::symlink("fifo", &link).unwrap();
            let e = write_atomic(&link, b"x = 1\n").expect_err("FIFO 를 보통 파일로 갈아끼웠다");
            assert!(e.message.contains("not a regular file"), "{}", e.message);
            assert!(std::fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo(), "FIFO 가 사라졌다");
            assert!(is_link(&link), "링크를 갈아끼웠다");
        }
        assert!(names(s.path()).iter().all(|n| !n.contains(".tmp.")), "찌꺼기가 남았다: {:?}", names(s.path()));
    }

    /// **링크로 한 `issues.jsonl` 을 함께 쓰는 두 트래커도 서로를 막는다**(moai-4oab 리뷰). 쓰기가 링크를
    /// 따라가면서 둘째 체크아웃이 첫째의 파일을 가리키는 꼴이 오래 서게 됐는데, 각자 제 `.moai/lock` 만
    /// 잡으면 나중에 `rename` 한 쪽이 앞의 줄을 조용히 지운다 — 사용자 설정의
    /// `two_spellings_of_one_config_still_lock_each_other` 가 측정한 것과 같은 손실이다. `flock` 은 열린 파일마다라
    /// 한 프로세스의 스레드끼리도 서로 막는다.
    #[cfg(unix)]
    #[test]
    fn two_trackers_sharing_one_issue_file_through_a_link_still_lock_each_other() {
        // 링크는 체크아웃 안에서만 따라가므로(moai-4oab) 둘이 한 파일을 나누는 꼴은 한 트래커가 다른
        // 트래커를 품을 때 선다 — 바깥 `a` 와 그 안의 `b` 가 `b` 안의 `data/issues.jsonl` 을 함께 가리킨다.
        let (a, da) = repo("far-lock-a");
        let inner = da.join("inner");
        std::fs::create_dir_all(inner.join(".moai")).unwrap();
        std::fs::create_dir(inner.join("data")).unwrap();
        std::fs::write(inner.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        std::fs::write(inner.join("data/issues.jsonl"), "").unwrap();
        std::fs::remove_file(a.issues_path()).unwrap();
        std::os::unix::fs::symlink("../inner/data/issues.jsonl", a.issues_path()).unwrap();
        std::os::unix::fs::symlink("../data/issues.jsonl", inner.join(".moai/issues.jsonl")).unwrap();
        let b = Repo::at(inner.clone(), Config::load(&inner).unwrap());
        let (threads, each) = (8, 5);
        std::thread::scope(|scope| {
            for t in 0..threads {
                // 짝수 갈래는 가리켜진 트래커로, 홀수 갈래는 링크를 든 트래커로 — 같은 파일이다.
                let r = if t % 2 == 0 { &a } else { &b };
                scope.spawn(move || {
                    for n in 0..each {
                        r.with_write(
                            || crate::i18n::Lang::Ko,
                            |issues, cfg, reserved| {
                                let title = format!("갈래 {t} 의 {n}");
                                let id = new_id(issues, cfg, reserved, None, &title);
                                issues.push(Issue::new(id, title, Kind::Issue, Status::new("todo"), T));
                                Ok((vec![], ()))
                            },
                        )
                        .unwrap();
                    }
                });
            }
        });
        assert_eq!(a.read().unwrap().issues.len(), threads * each, "링크로 함께 쓰는 두 트래커가 서로를 지웠다");
        assert!(is_link(&a.issues_path()) && is_link(&b.issues_path()), "링크를 보통 파일로 갈아끼웠다");
    }

    /// **저장소 락은 링크를 안 따르고 체크아웃 밖에 안 선다**(moai-sn57, 2026-10-02 사용자 결정). 받은 저장소가
    /// `.moai/lock` 을 링크로 커밋하면 따르던 판은 그 끝에 락을 지었다 — `-> ../.git/index.lock` 이면 그 뒤의 git
    /// 커밋이 다 졌고, 밖을 가리키면 체크아웃 밖에 파일이 섰다. 안을 가리키는 `-> issues.jsonl` 도 거절한다 —
    /// 쓰기마다 `rename` 으로 갈리는 아이노드라 두 쓰는 쪽이 서로 다른 파일을 쥔다. 보통 파일이 아닌 자리도
    /// 거절하고, FIFO 앞에서는 안 멈춘다. 어느 것이든 아무것도 안 쓰고 `broken` 으로 멈추며 심어 둔 것은 남는다.
    /// 링크 너머 스냅샷 곁의 락([`Repo::far_lock`])도 같다 — 그 락만 링크를 따르던 대로 되돌려도 이 시험이 붉어진다.
    /// 거꾸로 스냅샷이 락과 한 파일로 풀리는 링크(`issues.jsonl -> lock`)도 거절한다(리뷰 moai-sn57.kq4).
    /// 동시 쓰기로 재는 것은 `tests/cli.rs` 의 `a_lock_that_is_a_link_stops_every_write_instead_of_losing_them` 이다.
    #[cfg(unix)]
    #[test]
    fn a_repository_lock_never_follows_a_link_nor_stands_outside_its_checkout() {
        let away = Scratch::new("store-lock-away");
        let refused = |r: &Repo, what: &str| {
            let wrote = r.with_write(
                || crate::i18n::Lang::En,
                |issues, _, _| {
                    issues.push(issue("argos-4aex"));
                    Ok((vec![], ()))
                },
            );
            let e = wrote.expect_err(what);
            assert_eq!(e.code, code::BROKEN, "{what}: {}", e.message);
            assert!(r.read().unwrap().issues.is_empty(), "{what}: 거절하고도 썼다");
            e.message
        };

        // 링크 셋 — 밖으로, `.git/` 안으로(끝이 없다), 안의 보통 파일로.
        for (name, to) in [
            ("out", away.join("lock")),
            ("git", PathBuf::from("../.git/index.lock")),
            ("in", PathBuf::from("issues.jsonl")),
        ] {
            let (r, d) = repo(&format!("lock-link-{name}"));
            std::fs::create_dir(d.join(".git")).unwrap();
            let lock = r.dir().join("lock");
            std::os::unix::fs::symlink(&to, &lock).unwrap();
            let said = refused(&r, name);
            assert!(said.contains("a link"), "{name}: 링크라고 안 댔다 — {said}");
            assert!(is_link(&lock), "{name}: 링크를 갈아끼웠다");
            assert!(!d.join(".git/index.lock").exists(), "{name}: .git 안에 락을 지었다");
        }
        assert!(!away.join("lock").exists(), "체크아웃 밖에 락을 지었다");

        // **링크 너머 스냅샷 곁의 락도 같은 문을 지난다**([`Repo::far_lock`]). `issues.jsonl -> ../<디렉터리>/issues.jsonl`
        // 이면 그 디렉터리의 `lock` 을 잡는데, 그 `lock` 이 밖을 가리키는 링크면 따르던 판은 체크아웃 밖에 락을
        // 지었다. 그 디렉터리 이름은 받은 저장소가 지은 것이라, 거절문에 실린 제어 문자도 걷혀야 한다.
        let (r, d) = repo("lock-link-far");
        let far = d.join("da\u{1b}[2Jta");
        std::fs::create_dir(&far).unwrap();
        std::fs::rename(r.issues_path(), far.join("issues.jsonl")).unwrap();
        std::os::unix::fs::symlink("../da\u{1b}[2Jta/issues.jsonl", r.issues_path()).unwrap();
        std::os::unix::fs::symlink(away.join("far"), far.join("lock")).unwrap();
        let said = refused(&r, "너머의 락");
        assert!(said.contains("a link"), "너머의 락: 링크라고 안 댔다 — {said}");
        assert!(!said.contains('\u{1b}'), "너머의 락: 디렉터리 이름의 제어 문자가 거절문에 그대로 든다 — {said:?}");
        assert!(is_link(&far.join("lock")), "너머의 락: 링크를 갈아끼웠다");
        assert!(!away.join("far").exists(), "너머의 락 링크를 따라 체크아웃 밖에 락을 지었다");

        // **이미 쥔 락으로 풀리는 링크도 링크다**(리뷰 moai-sn57.kq4). 쥔 락인지를 링크를 따라 재던 판은 그 자리를
        // 건너뛰었다 — 커밋된 `lock -> /proc/self/fd/3` 은 프로세스마다 제 `.moai/lock` 으로 풀려, 한 파일을
        // 함께 쓰는 두 트래커가 아무도 너머의 락을 안 잡고 줄을 잃었다. 여기서는 그 풀이를 제 락으로 가리켜 잰다.
        let (r, d) = repo("lock-link-far-held");
        std::fs::create_dir(d.join("data")).unwrap();
        std::fs::rename(r.issues_path(), d.join("data/issues.jsonl")).unwrap();
        std::os::unix::fs::symlink("../data/issues.jsonl", r.issues_path()).unwrap();
        std::os::unix::fs::symlink("../.moai/lock", d.join("data/lock")).unwrap();
        let said = refused(&r, "쥔 락으로 풀리는 너머의 락");
        assert!(said.contains("a link"), "쥔 락으로 풀리는 너머의 락을 링크라고 안 댔다 — {said}");

        // **스냅샷이 락과 한 파일이면 안 쓴다**(리뷰 moai-sn57.kq4) — 제 락이든 너머의 락이든 쓰기마다 `rename` 이
        // 그 아이노드를 갈아끼워, 쓰는 쪽들이 서로 다른 파일을 잠그고 줄을 잃는다.
        for (name, to) in [("own", "lock"), ("far", "../data/lock")] {
            let (r, d) = repo(&format!("lock-snapshot-on-{name}"));
            std::fs::create_dir(d.join("data")).unwrap();
            std::fs::write(d.join("data/lock"), "").unwrap();
            std::fs::remove_file(r.issues_path()).unwrap();
            std::os::unix::fs::symlink(to, r.issues_path()).unwrap();
            let said = refused(&r, name);
            assert!(said.contains("the lock moai holds"), "{name}: 락에 닿는 스냅샷이라고 안 댔다 — {said}");
            assert!(is_link(&r.issues_path()), "{name}: 스냅샷 링크를 갈아끼웠다");
        }

        // 보통 파일이 아닌 락 자리 — 디렉터리와 FIFO. FIFO 를 쓰기로 그냥 열면 읽는 쪽을 영영 기다린다.
        let (r, _d) = repo("lock-dir");
        std::fs::create_dir(r.dir().join("lock")).unwrap();
        let said = refused(&r, "디렉터리");
        assert!(said.contains("not a regular file"), "{said}");
        let (r, d) = repo("lock-fifo");
        let at = r.dir().join("lock");
        let c = std::ffi::CString::new(at.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: 널로 끝나는 경로와 권한 비트만 넘긴다.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "FIFO 를 못 지었다");
        let home = crate::held::Home::of(d.path());
        let got = crate::held::tests::within("FIFO 인 락", move || match Lock::inside(&at, &home) {
            Ok(_) => None,
            Err(Stop::Refused(t)) => Some(t.code()),
            Err(Stop::Failed(e)) => Some(e.code),
        });
        assert_eq!(got, Some(code::BROKEN), "FIFO 에 락을 잡았거나 다른 까닭으로 졌다");

        // `.moai` 자체가 밖을 가리키는 자리 — 여는 길(`Repo::rooted`)은 대개 설정에서 먼저 멈추지만, 연 뒤에 `.moai`
        // 가 갈린 [`Repo`](떠 있는 탐색기의 것)는 그 길을 다시 안 지난다. 따르던 판은 거기서 락 파일 하나를 밖에 지었다.
        let s = Scratch::new("store-lock-moai-out");
        let elsewhere = away.join("moai");
        std::fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, s.join(".moai")).unwrap();
        let r = Repo::at(s.to_path_buf(), r.config.clone());
        let said = refused(&r, ".moai 가 밖");
        assert!(said.contains("outside"), "{said}");
        assert!(!elsewhere.join("lock").exists(), "`.moai` 링크를 따라 체크아웃 밖에 락을 지었다");
    }

    /// **저장소가 든 파일은 체크아웃 밖을 가리키는 링크를 안 따라간다**(moai-4oab, 2026-09-29 사용자 결정
    /// 둘째 판). 받은 저장소가 `.moai/issues.jsonl -> ~/.bashrc` 를 커밋해 두면 흔한 `moai add` 가 그 파일을
    /// 고쳐 쓰던 자리다. 링크 글의 `..` 으로 나가는 것도, 안의 디렉터리 링크를 거쳐 나가는 것도 같다.
    /// 거절은 링크와 그 끝을 대고 아무것도 안 바꾼다. 사람이 건 링크를 따르는 [`write_atomic`] 은 그대로다.
    #[cfg(unix)]
    #[test]
    fn a_repository_file_never_follows_a_link_out_of_its_checkout() {
        let s = Scratch::new("store-atomic-outside");
        let (repo_dir, away) = (s.join("repo"), s.join("away"));
        std::fs::create_dir_all(repo_dir.join("inside")).unwrap();
        std::fs::create_dir(&away).unwrap();
        std::fs::write(away.join("rc"), "옛\n").unwrap();
        std::os::unix::fs::symlink("../away/rc", repo_dir.join("dotdot")).unwrap();
        std::os::unix::fs::symlink(&away, repo_dir.join("inside/door")).unwrap();
        std::os::unix::fs::symlink("inside/door/rc", repo_dir.join("through")).unwrap();
        for name in ["dotdot", "through"] {
            let link = repo_dir.join(name);
            let e = write_atomic_inside(&link, b"x\n", &repo_dir).expect_err("체크아웃 밖을 고쳐 썼다");
            assert!(e.message.contains("outside"), "{}", e.message);
            let e = write_atomic_in(&link, b"x\n", &repo_dir, &repo_dir).expect_err("체크아웃 밖을 고쳐 썼다");
            assert!(e.message.contains("outside"), "{}", e.message);
            let e = append_inside(&link, b"x\n", &repo_dir).expect_err("체크아웃 밖에 덧붙였다");
            assert!(e.message.contains("outside"), "{}", e.message);
            assert!(is_link(&link), "{name}: 링크를 갈아끼웠다");
        }
        assert_eq!(std::fs::read_to_string(away.join("rc")).unwrap(), "옛\n");
        assert_eq!(names(&repo_dir), ["dotdot", "inside", "through"], "찌꺼기가 남았다");

        // 안을 가리키는 링크는 따라가고, 사람의 파일을 쓰는 쪽은 밖이라도 따라간다.
        std::fs::write(repo_dir.join("inside/real"), "").unwrap();
        std::os::unix::fs::symlink("inside/real", repo_dir.join("near")).unwrap();
        write_atomic_inside(&repo_dir.join("near"), b"in\n", &repo_dir).unwrap();
        assert_eq!(std::fs::read_to_string(repo_dir.join("inside/real")).unwrap(), "in\n");
        append_inside(&repo_dir.join("near"), b"more\n", &repo_dir).unwrap();
        assert_eq!(std::fs::read_to_string(repo_dir.join("inside/real")).unwrap(), "in\nmore\n");
        assert!(is_link(&repo_dir.join("near")), "덧붙이다 링크를 갈아끼웠다");
        write_atomic(&repo_dir.join("dotdot"), b"dotfiles\n").unwrap();
        assert_eq!(std::fs::read_to_string(away.join("rc")).unwrap(), "dotfiles\n");

        // **끝 조각이 아니라 가운데 디렉터리가 링크여도 같다**(리뷰) — 커밋된 `.moai/journal -> /밖` 에
        // `moai add` 가 새 저널 파일을 짓던 자리다.
        std::os::unix::fs::symlink(&away, repo_dir.join("journal")).unwrap();
        let e = append_inside(&repo_dir.join("journal/who.jsonl"), b"{}\n", &repo_dir).expect_err("밖에 지었다");
        assert!(e.message.contains("outside"), "{}", e.message);
        write_atomic_inside(&repo_dir.join("journal/who.jsonl"), b"{}\n", &repo_dir).expect_err("밖에 지었다");
        assert!(!away.join("who.jsonl").exists(), "디렉터리 링크를 지나 체크아웃 밖에 파일을 지었다");

        // **체크아웃 안이라도 `.git/` 은 아니다**(리뷰) — 저널 링크 하나로 git 설정에 JSON 이 붙어 그 클론의
        // git 이 섰다. 덧붙이는 쪽도 갈아끼우는 쪽도 같다.
        std::fs::create_dir(repo_dir.join(".git")).unwrap();
        std::fs::write(repo_dir.join(".git/config"), "[core]\n").unwrap();
        std::os::unix::fs::symlink(".git/config", repo_dir.join("gitdir")).unwrap();
        let e = append_inside(&repo_dir.join("gitdir"), b"{}\n", &repo_dir).expect_err(".git 에 덧붙였다");
        assert!(e.message.contains(".git"), "{}", e.message);
        write_atomic_inside(&repo_dir.join("gitdir"), b"{}\n", &repo_dir).expect_err(".git 을 고쳐 썼다");
        assert_eq!(std::fs::read_to_string(repo_dir.join(".git/config")).unwrap(), "[core]\n");
        assert!(is_link(&repo_dir.join("gitdir")), "링크를 갈아끼웠다");
        // 딸린 워크트리의 `.git` 은 파일이다 — 끝 이름이 `.git` 인 것도 같다.
        let tree = s.join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join(".git"), "gitdir: /x\n").unwrap();
        std::os::unix::fs::symlink(".git", tree.join("gitfile")).unwrap();
        append_inside(&tree.join("gitfile"), b"{}\n", &tree).expect_err("워크트리의 .git 파일에 덧붙였다");
        assert_eq!(std::fs::read_to_string(tree.join(".git")).unwrap(), "gitdir: /x\n");
    }
}
