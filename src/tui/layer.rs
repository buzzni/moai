//! 프로젝트 층 — 등록한 프로젝트를 디렉터리처럼 드나든다(moai-ujpu).
//!
//! **층은 `nav::Seg` 의 한 마디가 아니라 따로 선 자리다**([`At`]). `nav` 는 한 프로젝트의
//! `&[Issue]` 만 아는 순수 모듈로 남는다 — 프로젝트를 마디로 넣으면 색인이 남의 줄을 제
//! 트리로 읽고, 같은 id 를 쓰는 두 프로젝트가 한 트리에서 섞인다(`projects` 가 줄을
//! 합치지 않는 까닭과 같다).
//!
//! `App` 은 여전히 **한 프로젝트의** 줄을 든다. 들어가면 그 프로젝트를 읽어 들이고,
//! 올라오면 그 줄을 비운다 — 층에 선 동안 `App::repo` 는 `None` 이라 어느 프로젝트에도
//! 쓸 수 없고, 옛 프로젝트의 id 가 커서 정체로 새지 않는다.
//!
//! **정체는 경로다.** 이름은 등록 목록 전체에서 정해지는 파생값(`user_config::names`)이라
//! 목록이 바뀌면 달라진다.
//!
//! 조각이 아니다 — 저장소와 사용자 설정을 연다(`input::NOT_COMPONENTS`).

use super::form::{Form, Target};
use super::keys::{BROWSE, Browse, JOT, Jot, label};
use super::{App, Row, Stamp};
use crate::nav::Index;
use crate::projects::{self, State};
use crate::store::Repo;
use crate::user_config;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

/// 지금 서 있는 곳.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum At {
    /// 프로젝트 층.
    Layer,
    /// 그 경로의 프로젝트 안. **그동안 `App::repo` 는 늘 `Some` 이고 이 프로젝트의 것이다.**
    Project(PathBuf),
}

/// 프로젝트 층. `App::layer` 가 `None` 이면 등록한 것이 없고, 탐색기는 오늘과 같다.
pub struct Layer {
    /// 층의 말(moai-ra67). **층도 고른 말로 선다** — 한때 [`shut`] 이 `Lang::Ko` 를 손으로
    /// 줬고, 그 한 줄이 "탐색기는 아직 한국어로 선다" 는 뜻이었다. 말은 [`super::Site::lang`]
    /// 과 한 자리에서 오고(`Ctx::lang`), 읽기가 스레드로 흩어지므로 층이 제 몫을 들고 간다.
    pub lang: crate::i18n::Lang,
    pub at: At,
    /// 층의 줄. 차례는 띄운 자리(등록 안 됨)가 맨 앞, 나머지는 등록 차례 그대로다.
    pub places: Vec<Place>,
    /// 사용자 설정을 읽다 만난 것. 층에 선 동안 배너가 비춘다.
    pub problems: Vec<String>,
    /// 그 탈의 갈래([`user_config::Trouble`], moai-9p7v). **[`App::relayer`] 는 이제 갈래를 안 가린다**
    /// (moai-po6v) — 어느 탈이든 들고 있던 층을 두므로, 이 값을 보고 갈라서는 자리는 없다. 다시 읽을
    /// 때를 정하는 자는 층이 아니라 [`App::config_tried`] 다. 여기 새 갈래를 매달지 않는다.
    pub trouble: Option<user_config::Trouble>,
    /// 읽은 사용자 설정 파일. 그 파일이 바뀌면 걸음이 여기를 다시 읽는다(`App::follow_config`) — **시험은 제 임시 파일을 준다.**
    /// 환경을 다시 보면 돌리는 사람의 설정을 읽는다.
    pub config: Option<PathBuf>,
    /// `.moai` 안에서 띄웠으면 그 뿌리. 등록돼 있지 않아도 층에 선다.
    launch: Option<PathBuf>,
    /// 스레드에서 읽고 있는 프로젝트들. 끝나면 [`App::follow`] 가 받는다.
    pending: Option<(Receiver<Looked>, std::thread::JoinHandle<()>)>,
    /// **펼친 프로젝트의 줄**을 읽고 있는 스레드(moai-12yx) — 어느 프로젝트인지와 받을 곳. 요약을
    /// 읽는 [`Layer::pending`] 과 따로 두는 것은 값이 다르기 때문이다: 요약은 줄마다 작은 셈이고
    /// 이것은 그 프로젝트를 통째로 읽어 색인까지 짓는 일이라, 한 번에 **하나만** 돈다.
    pub(super) reading: Option<(PathBuf, Receiver<crate::fail::R<super::Fresh>>, std::thread::JoinHandle<()>)>,
    /// 펼쳐 놓고 아직 못 읽은 프로젝트 — 차례로 하나씩 읽는다. 사람이 여럿을 잇따라 펼쳐도
    /// 스레드가 그 수만큼 서지 않는다.
    pub(super) wanted: Vec<PathBuf>,
}

/// 층의 한 줄 — 프로젝트 하나.
pub struct Place {
    /// 정체. 등록한 철자 그대로(띄운 자리면 그 뿌리).
    pub path: PathBuf,
    /// 화면에 댈 이름. 파생값이라 정체로 쓰지 않는다.
    pub name: String,
    /// 사용자 설정에 정한 색 — 없으면 경로로 고른다(`draw::project_style`).
    pub hue: Option<crate::style::Hue>,
    /// 사용자 설정에 있는가. 아니면 띄운 자리라 층에 섰을 뿐이다.
    pub registered: bool,
    /// 이 탐색기를 띄운 자리인가.
    pub launched: bool,
    pub look: Look,
    /// 읽어 든 그 프로젝트의 줄(moai-eyre). **한눈 보기가 머리줄 밑에 세울 것이고**, 없으면 머리줄만
    /// 선다 — 읽는 것은 펼칠 때다(moai-12yx). 요약([`Look::Open`])과 따로 드는 까닭은 값이 다르기
    /// 때문이다: 요약은 걸음마다 다시 재는 작은 셈이고, 이것은 그 프로젝트를 통째로 읽어 색인까지 지은
    /// 것이다.
    pub site: Option<super::Site>,
    /// 읽기 **전에** 잰 표식 — `.moai/issues.jsonl` 과 `.moai/config.toml`, 옆 워크트리.
    marks: Marks,
    /// 읽은 것을 **들인** 때([`Layer::adopt`]) — 시작한 때가 아니다(까닭은 거기). [`due`] 가 이것으로
    /// 잰다. `None` 이면 곧바로 다시 읽을 줄이다 — 아직 안 읽었거나, 방금 열어 본 줄(`App::open_place`)
    /// 이나 올라오며 떠난 줄([`App::climb`]).
    read_at: Option<std::time::Instant>,
}

/// 한 줄을 다시 읽을 까닭이 되는 표식.
///
/// 설정 표식까지 재는 까닭: `moai init` 은 설정이 먼저 생기고, 깨진 설정을 고친 것은
/// 스냅샷 표식으로는 안 보인다. **디렉터리가 있는지도 잰다** — `.moai` 없는 디렉터리가
/// 지워지거나(init 전 → 없다) 빈 디렉터리로 다시 생기면(없다 → init 전) 두 파일의 표식은
/// 둘 다 `None` 그대로라, 층이 옛 까닭과 옛 고칠 길을 영영 댄다.
///
/// **옆 워크트리도 잰다**(moai-al0x, `worktree::place_marks`). 요약에 자리 판정이 실리는데,
/// 그 답은 워크트리를 띄우거나 치우는 것만으로 바뀐다 — `.moai` 두 파일은 그대로다. 안 재면
/// 워크트리를 치운 뒤에도 층이 영영 "자리 없는 것 0건" 을 댄다. 모두 `stat` 과 작은
/// 파일 읽기라 걸음마다 재도 싸다(git 을 안 띄운다).
#[derive(Debug, Clone, PartialEq, Default)]
struct Marks {
    dir: bool,
    issues: Stamp,
    config: Stamp,
    trees: Vec<(PathBuf, Stamp)>,
}

/// 이만큼 지난 읽기는 표식이 그대로여도 다시 읽는다(moai-al0x·moai-z4r4, 사용자 결정 2026-09-18).
/// 셈에는 시계로 재는 것이 든다 — 방금 집은 줄에 워크트리가 뜰 틈(한 시간, `report::stranded`)과
/// 날로 재는 경고. 파일은 그대로라 표식으로는 영영 안 보이고, 옛 수가 선 채 남는다. 층과
/// 프로젝트 안([`App::follow`])이 **같은 자**를 쓴다 — 따로 두면 한쪽만 틈을 넘겨 두 화면이 또
/// 갈린다. 읽기는 둘 다 스레드로 가서, 1분에 한 번이면 그 값이 화면을 안 멈춘다.
pub(super) const REREAD_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// 들인 지 [`REREAD_EVERY`] 가 지났는가 — **이 자 하나를** 층의 줄과 프로젝트 안이 함께 쓴다. 상수만
/// 나눠 갖고 견주는 법을 저마다 적으면(`>=` 냐 `>` 냐, 안 들인 것을 어떻게 치느냐) 한쪽만 고쳐져 두
/// 화면이 또 갈린다. 들인 적이 없으면 시계로는 안 낡는다 — 그런 줄은 부르는 쪽이 따로 고른다.
pub(super) fn due(read_at: Option<std::time::Instant>) -> bool {
    read_at.is_some_and(|t| t.elapsed() >= REREAD_EVERY)
}

/// 진 읽기를 **지금 갚을 때인가**(moai-po6v) — 표식이 그대로여도 다시 읽을 까닭이 되는 자다.
/// 갈래가 정하고([`user_config::Again`]) 시계는 [`due`] 하나로 잰다.
///
/// **설정과 읽음이 이 자 하나를 함께 쓴다**([`App::follow_config`]·[`App::follow_read`]) — [`due`] 와
/// 같은 까닭이다. 저마다 적으면 갈래를 하나 더할 때 한쪽만 고쳐져, 설정은 다시 읽는데 읽음은 영영
/// 안 읽는 화면이 선다. 갈래를 더하는 사람이 고칠 자리는 [`user_config::Trouble::again`] 하나다.
pub(super) fn owed(tried: &Tried) -> bool {
    match tried.trouble.map(user_config::Trouble::again) {
        // **잠깐이라던 것이 잠깐이 아니면 시계로 내린다**(moai-4qbv.i0g 리뷰) — 이 갈래는 모르는 실패가
        // 다 모이는 자리다(`user_config::unreadable` 이 안 집은 것은 다 잠깐으로 본다). 끊긴 sshfs·NFS 는
        // `ENOTCONN`·`EIO` 로 지는데 그것이 여기 들어, 끝을 안 두면 **막히는 읽기**를 걸음마다 다시 연다.
        // 이슈가 "다시 해 보는 데 끝을 두라" 고 한 자리다.
        Some(user_config::Again::Step) => tried.since.is_none_or(|t| t.elapsed() < BLIP) || due(tried.at),
        Some(user_config::Again::Clock) => due(tried.at),
        Some(user_config::Again::Never) | None => false,
    }
}

/// 잠깐이라고 봐 주는 동안(moai-4qbv.i0g 리뷰). 이만큼 이어서 지면 그것은 잠깐이 아니라 탈이라,
/// 다시 읽는 때가 [`REREAD_EVERY`] 로 내려간다. 걸음은 30ms~700ms 마다 오므로 이 사이에 몇 번은 된다 —
/// 걸음 수로 세지 않는 것은 걸음 사이가 화면에 따라 스무 배까지 벌어지기 때문이다.
pub(super) const BLIP: std::time::Duration = std::time::Duration::from_secs(3);

/// 마지막 읽기의 자취 — 갈래와 **언제 해 봤는가·언제부터 지고 있는가**(moai-po6v). 설정과 읽음이
/// 저마다 같은 세 값을 들던 판은 셋을 함께 올리는 규칙이 두 곳에 적혀, 한쪽만 고쳐질 자리였다.
/// 올리는 자([`Tried::saw`])와 재는 자([`owed`])가 각각 하나다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tried {
    /// 멀쩡했으면 `None`. 다시 읽을 때를 정하는 자다([`user_config::Trouble::again`]).
    pub trouble: Option<user_config::Trouble>,
    /// 마지막으로 **읽어 본** 때 — 되든 안 되든 찍는다. 시계로 다시 보는 갈래가 이것을 잰다.
    pub at: Option<std::time::Instant>,
    /// 지금 이어지는 실패가 **시작한** 때. 읽히면 지운다 — 다음 실패는 다시 잠깐부터 센다.
    pub since: Option<std::time::Instant>,
}

impl Tried {
    /// 읽어 본 것을 적는다. **읽었으면 늘 부른다** — 안 부르면 자취가 낡은 채로 남아 다시 읽을 때를
    /// 옛 갈래가 정한다.
    pub fn saw(&mut self, trouble: Option<user_config::Trouble>) {
        let now = std::time::Instant::now();
        self.since = match (self.trouble.is_some(), trouble.is_some()) {
            (_, false) => None,
            (true, true) => self.since.or(Some(now)),
            (false, true) => Some(now),
        };
        self.trouble = trouble;
        self.at = Some(now);
    }
}

/// 프로젝트 하나를 본 것.
pub enum Look {
    /// 아직 안 읽었다. `.moai` 안에서 띄우면 남의 프로젝트는 처음 올라갈 때 읽는다.
    Unread,
    Open { sum: Summary },
    /// 못 연다. `said` 는 CLI 한눈 보기와 **같은 말**이다(`view::unopened`, 색은 걷었다).
    Shut { state: Shut, said: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shut {
    Uninit,
    Missing,
    Unreadable,
}

/// 연 프로젝트의 셈 — `moai status` 의 한눈 보기와 같은 자(`report::status`·`report::wip`).
pub struct Summary {
    /// config 차례로 칸마다 일의 수. 미룬 것은 뺀다 — 한눈 보기의 보드와 같다.
    pub counts: Vec<(String, usize)>,
    pub picked: Vec<Picked>,
    /// 드러난 것의 수. 알림은 안 센다. **자리 없는 줄도 여기 든다** — 층의 `!` 와 "드러난 것
    /// N건" 이 `moai status` 와 같은 수를 말해야, 층에서 보고 들어간 사람이 다른 수를 안 본다.
    pub warnings: usize,
    /// 그중 집었는데 일하는 워크트리가 없는 줄(moai-p3bs) — 층은 이것을 낱말로 따로 댄다.
    /// 죽은 세션을 찾으러 돌아온 사람이 보는 첫 화면이 여기다.
    pub stranded: usize,
    /// 스냅샷을 **못 읽은** 워크트리의 수(`worktree::Unread::all`) — 층의 `!` 와 "스냅샷을 못 읽은
    /// 워크트리 N곳" 이 이것으로 선다(사용자 결정 2026-09-18, 리뷰 moai-rgz9.7vt). **판 것 가운데
    /// 센다** — 이름만으로 자리가 다 잡히면 옆 스냅샷을 아예 안 연다(`worktree::Unread`). 판정을 안 가려도
    /// 깨진 파일은 고칠 사람이 알아야 하고, 한눈 보기가 같은 것을 `옆 워크트리 문제 N건` 으로 센다 —
    /// 여기만 조용하면 두 화면이 같은 저장소를 달리 말한다.
    pub unread: usize,
    /// 그중 자리 판정을 **가린** 것(`report::blinding`). 그런 워크트리가 있으면 위의 `stranded` 가
    /// "센 결과 0" 이 아니라 "못 셌다" 인데, 이것이 없으면 층이 그 둘을 같은 화면으로 낸다.
    /// 못 읽어도 이름이 집은 줄을 가리키는 워크트리는 판정을 안 가리므로 여기 안 든다(moai-rgz9) —
    /// 들면 다 세고도 "자리를 다 못 셌다" 가 선다.
    pub blind: usize,
    pub unreadable: usize,
}

/// 집은 일 한 줄.
pub struct Picked {
    pub id: String,
    pub title: String,
    pub column: String,
}

/// 스레드가 돌려주는 것 — 어느 경로를 어떤 표식으로 읽었나.
struct Looked {
    path: PathBuf,
    marks: Marks,
    look: Look,
}

fn marks_of(dir: &Path) -> Marks {
    // **읽는 트래커를 잰다**(moai-y7go, 리뷰 moai-71ht 셋째 판) — 등록한 자리가 딸린 워크트리면
    // `Repo::open` 이 루트의 트래커를 연다(`Repo::opened_root`). 그 자리의 `.moai` 를 재던 판은 루트에
    // 떨어진 쓰기를 하나도 못 보고 1분 시계가 돌 때까지 낡은 셈을 세웠다 — 표식이 있는 까닭이 그
    // 시계를 안 기다리는 것이다.
    let moai = crate::store::Repo::opened_root(dir).join(".moai");
    Marks {
        dir: dir.is_dir(),
        issues: crate::store::stamp(&moai.join("issues.jsonl")),
        config: crate::store::stamp(&moai.join("config.toml")),
        trees: crate::worktree::place_marks(dir),
    }
}

/// 같은 디렉터리인가 — 등록 목록이 쓰는 그 자다. 여기에 따로 두면 층이 "같은 프로젝트"
/// 라고 본 줄을 등록(`projects::add`)이 다른 것으로 세어 같은 저장소가 둘로 선다.
fn same_dir(a: &Path, b: &Path) -> bool {
    user_config::same_dir(a, b)
}

/// 연 프로젝트 하나를 센다. **`&[Issue]` 에 대한 셈은 전부 `report` 가 한다.**
pub fn summarize(repo: &Repo, load: &crate::store::Load, now: &str) -> Summary {
    let cfg = &repo.config;
    let unreadable = load.unreadable();
    let st = crate::report::status(&load.issues, &unreadable, cfg, now);
    // **자리도 여기서 잰다**(moai-p3bs) — `moai status` 와 같은 자(`worktree::stranded_at`). 한때
    // 그 한 명령에만 있어, 층에서 "드러난 문제 없다" 를 보고 들어가면 경고가 서 있었다.
    // 층은 겹쳐 보지 않는다(`projects::open`) — 그 자리의 스냅샷 그대로 잰다.
    // 자리를 재는 뿌리는 **등록한 그 체크아웃**이다(`repo.here()`) — `moai status` 와 같은 자.
    let (lost, unread) = crate::worktree::stranded_at(repo.here(), cfg, &load.issues, false, now);
    let stranded = lost.as_ref().map_or(0, |w| w.count);
    Summary {
        counts: cfg.statuses.iter().map(|s| (s.clone(), st.counts.get(s).copied().unwrap_or(0))).collect(),
        picked: crate::report::wip(&load.issues, cfg)
            .into_iter()
            .map(|i| Picked { id: i.id.clone(), title: i.title.clone(), column: i.status.as_str().to_string() })
            .collect(),
        warnings: st.warnings.len() + usize::from(lost.is_some()),
        stranded,
        unread: unread.all.len(),
        blind: unread.blinding.len(),
        unreadable: load.errors.len(),
    }
}

/// 열지 못한 상태를 층의 말로. 말은 CLI 한눈 보기의 것을 그대로 쓴다 — 같은 상태를 두
/// 화면이 달리 부르면 둘을 오가는 사람이 두 낱말을 다 배워야 한다.
fn shut(path: &Path, name: &str, state: State, lang: crate::i18n::Lang) -> Look {
    let kind = match &state {
        State::Uninit => Shut::Uninit,
        State::Missing => Shut::Missing,
        State::Unreadable(_) | State::Open { .. } => Shut::Unreadable,
    };
    let p = projects::Project {
        path: path.to_path_buf(),
        name: name.to_string(),
        hue: None,
        state,
        origin: Default::default(),
        trouble: Vec::new(),
        swept: false,
    };
    // 말은 층이 들고 온다(moai-ra67) — 한눈 보기(`view::unopened`)가 내는 그 글을 그대로 쓴다.
    let said = crate::style::plain(&crate::view::unopened(&p, &p.seen(|_, _| ()), lang))
        .trim()
        .to_string();
    Look::Shut { state: kind, said }
}

/// 경로들을 연다. **프로젝트마다 제 스레드에서** 읽고, **닿는 대로 하나씩 보낸다**(moai-ezwu).
///
/// 한 줄의 값은 거의 자리 판정(`worktree::stranded_at`)이 옆 워크트리의 스냅샷을 파는 데 들고
/// (이슈 745·워크트리 일곱에 ~170ms, 판정을 빼면 ~40ms), 줄마다 디스크를 따로 만지므로 서로
/// 기다릴 까닭이 없다. 차례대로 읽으면 층이 멈추는 값이 프로젝트 수만큼 더해진다.
///
/// **한 벌로 묶어 보내지 않는다**(리뷰 moai-3lul.kt0) — 묶으면 빠른 줄까지 가장 느린 줄을
/// 기다려 첫 화면이 통째로 `읽는 중` 으로 서고, 한 줄이 멈춘 마운트에 걸리면 나머지가 영영 안
/// 찬다. 받는 쪽([`App::follow_layer`])은 걸음마다 닿은 만큼 들인다. **다만 읽기는 여전히 한 벌씩
/// 띄운다**([`Layer::launch`]) — 벌의 가장 느린 줄이 끝나야 다음 벌이 서므로, 느린 줄 하나가 다른
/// 줄의 다음 읽기(표식·시계)와 새로 등록한 줄의 첫 읽기를 그만큼 붙든다(리뷰 moai-3lul.kt0 다시 본 판,
/// 줄마다 따로 띄우는 것은 넘겼다).
///
/// 보내기가 실패하면(받는 쪽이 이 읽기를 버렸다) 그 줄은 버려진다 — 버린 읽기는 받을 것이 아니다.
///
/// 나란히 부르는 것은 한눈 보기와 같은 [`projects::each`] 다 — 한 줄의 패닉이 **그 까닭 그대로**
/// 되던져지고(`scope` 에 맡기면 std 가 까닭을 지운다), 스레드를 못 띄우면 그 자리에서 읽는다.
/// 보내기는 줄마다 제 스레드 안에서 하므로 닿는 대로 흐른다.
fn look_into(paths: &[PathBuf], now: &str, lang: crate::i18n::Lang, tx: &std::sync::mpsc::Sender<Looked>) {
    projects::each(paths, |path| {
        let _ = tx.send(look_one(path, now, lang));
    });
}

/// 한 경로를 연다. **표식을 먼저 잰다** — 읽고 나서 재면 그 사이의 쓰기가 "이미 본 것"
/// 으로 적혀 영영 안 보인다(`App::open` 과 같은 까닭).
fn look_one(path: &Path, now: &str, lang: crate::i18n::Lang) -> Looked {
    let marks = marks_of(path);
    // 여는 길은 한눈 보기와 같다(`projects::open_one`) — 상태를 가르는 셈을 두 벌 두지 않는다.
    // 이름은 여기서 안 쓴다(층이 목록 전체로 이미 정했다). 말에 이름은 안 든다.
    let p = projects::open_one(path, String::new(), None, false);
    let look = match p.state {
        State::Open { repo, load } => Look::Open { sum: summarize(&repo, &load, now) },
        state => shut(&p.path, &p.name, state, lang),
    };
    Looked { path: p.path, marks, look }
}

impl Layer {
    /// 사용자 설정을 읽어 층을 세운다. 줄은 아직 안 읽었다([`Look::Unread`]).
    ///
    /// `launch` 는 `.moai` 안에서 띄웠을 때의 뿌리다. **등록돼 있지 않아도 맨 앞에 선다** —
    /// 빼면 `0` 으로 층에 올라간 뒤 내려올 길이 없어, 디렉터리처럼 드나든다는 약속이 한
    /// 방향으로만 선다. 등록돼 있으면 그 줄에 표시만 붙는다. 이름은 띄운 자리까지 넣고
    /// 가른다 — 같은 화면에 같은 이름이 둘 서면 안 된다.
    pub fn read(config: Option<&Path>, launch: Option<&Path>) -> Layer {
        Layer::of(&user_config::read(config), launch)
    }

    /// 이미 읽은 설정으로 층을 세운다(moai-u8cs) — 띄울 때 보기와 같은 한 번의 읽기를 나눠 쓴다.
    ///
    /// **설정 자리는 읽은 것(`Registry::path`)에서 든다**(moai-y61p 단계 리뷰). 자리를 따로 받으면 줄은 한 파일에서
    /// 세우고 다시 읽기·등록·해제는 다른 파일에 하는 층이 설 수 있다.
    pub fn of(reg: &user_config::Registry, launch: Option<&Path>) -> Layer {
        let found = launch.and_then(|l| reg.projects.iter().position(|p| same_dir(&p.path, l)));
        let mut entries = reg.projects.clone();
        let extra = match (launch, found) {
            (Some(l), None) => {
                entries.insert(0, user_config::Project { path: l.to_path_buf(), hue: None });
                true
            }
            _ => false,
        };
        let names = user_config::names(&entries);
        let places: Vec<Place> = entries
            .into_iter()
            .zip(names)
            .enumerate()
            .map(|(k, (p, name))| Place {
                registered: !(extra && k == 0),
                launched: (extra && k == 0) || found == Some(k),
                path: p.path,
                name,
                hue: p.hue,
                look: Look::Unread,
                marks: Marks::default(),
                read_at: None,
                site: None,
            })
            .collect();
        let at = match places.iter().find(|p| p.launched) {
            Some(p) => At::Project(p.path.clone()),
            None => At::Layer,
        };
        Layer {
            // 기본값은 [`super::Site::lang`] 과 같은 까닭으로 한국어다 — 부른 쪽이 갈아 끼운다.
            lang: crate::i18n::Lang::Ko,
            at,
            places,
            problems: reg.problems.clone(),
            trouble: reg.trouble,
            config: reg.path.clone(),
            launch: launch.map(Path::to_path_buf),
            pending: None,
            reading: None,
            wanted: Vec::new(),
        }
    }

    /// 등록한 프로젝트가 하나라도 있는가. **없으면 층을 세우지 않는다** — 띄운 자리 하나뿐인
    /// 층은 오늘 화면에 `..` 하나를 더할 뿐이다(결정 3).
    pub fn registered(&self) -> bool {
        self.places.iter().any(|p| p.registered)
    }

    /// 다시 읽어야 할 줄 — 아직 안 읽었거나(읽은 때가 없거나), 표식이 바뀌었거나, 시계로 낡은 것([`due`]).
    ///
    /// **시계로 낡는 것은 연 줄과 못 읽는 줄이다.** 연 줄의 셈에는 때가 들고(한 시간 틈·날로 재는 경고),
    /// 못 읽는 줄은 표식이 그대로여도 풀린다 — 권한을 고친 것(`chmod` 은 고친 때를 안 바꾼다)이나 한 번
    /// 끊겼던 원격 디스크는 (고친 때, 길이)로 안 보여, 안 재면 "못 읽는다" 가 영영 선다(리뷰
    /// moai-3lul.kt0 다시 본 판). init 전과 사라진 디렉터리는 표식이 다 본다(디렉터리·설정 표식) — 그
    /// 줄의 말(`view::unopened`)에는 때도 안 들어 1분마다 다시 읽어도 같은 글이다.
    fn stale(&self) -> Vec<PathBuf> {
        self.places
            .iter()
            .filter(|p| {
                let clocked = matches!(p.look, Look::Open { .. } | Look::Shut { state: Shut::Unreadable, .. });
                matches!(p.look, Look::Unread) || p.read_at.is_none() || (clocked && due(p.read_at)) || marks_of(&p.path) != p.marks
            })
            .map(|p| p.path.clone())
            .collect()
    }

    /// 그 줄을 **곧바로 다시 읽을 줄로** 둔다 — 셈은 새것이 닿을 때까지 그대로 선다([`App::climb`]).
    fn forget(&mut self, path: &Path) {
        if let Some(p) = self.places.iter_mut().find(|p| p.path == path) {
            p.read_at = None;
        }
    }

    /// 읽어 온 줄 하나를 경로로 맞춰 들인다. 그새 목록에서 빠진 경로는 버린다.
    ///
    /// **읽은 때는 여기서 찍는다** — 읽기가 시작한 때로 찍으면 한 줄 읽는 데 [`REREAD_EVERY`] 가
    /// 넘게 걸리는 자리(느린 마운트)에서 들이는 순간 이미 낡아, 층이 같은 줄을 쉬지 않고 다시
    /// 읽는다(리뷰 moai-3lul.kt0). 표식은 반대로 읽기 **전**의 것이다 — 그 사이의 쓰기를 놓치면
    /// 영영 안 보인다.
    fn adopt(&mut self, looked: impl IntoIterator<Item = Looked>) {
        // **줄을 든 프로젝트는 표식이 움직이면 줄도 다시 읽는다**(리뷰). 요약만 새것으로 갈던
        // 때는 머리줄의 셈과 그 밑의 줄이 한 화면에서 다른 말을 했다 — 옆 세션이 닫은 일이 셈
        // 에서는 `✓` 로 올라가는데 줄은 세션 내내 `todo` 였다. 읽으러 가는 자는 [`App::want_site`]
        // 뿐인데 그쪽은 펼칠 때만 불리고, 든 줄이 있으면 아무것도 안 한다.
        let mut again: Vec<PathBuf> = Vec::new();
        for l in looked {
            if let Some(p) = self.places.iter_mut().find(|p| p.path == l.path) {
                if p.marks != l.marks && p.site.is_some() {
                    again.push(p.path.clone());
                }
                p.marks = l.marks;
                p.read_at = Some(std::time::Instant::now());
                p.look = l.look;
            }
        }
        for path in again {
            if !self.wanted.contains(&path) && self.reading.as_ref().is_none_or(|(p, ..)| *p != path) {
                self.wanted.push(path);
            }
        }
    }

    pub(super) fn position(&self, path: &Path) -> Option<usize> {
        self.places.iter().position(|p| p.path == path)
    }

    /// 낡은 줄을 **스레드로** 읽으러 간다 — 층에 섰고 도는 읽기가 없을 때만. 기다리는 동안 층은
    /// 옛 셈(처음이면 `읽는 중`)을 낸다. 받는 것은 [`App::follow_layer`] 다.
    ///
    /// 올라올 때와 밖에서 띄울 때도 이 길이다(moai-ezwu, 사용자 결정 2026-09-18). 한때 둘은 그
    /// 자리에서 읽었다 — 첫 화면에 수가 서야 한다는 것이었는데, 그 값이 프로젝트마다 더해져
    /// 워크트리가 많은 저장소 몇 개면 키 한 번에 화면이 수백 ms 멈췄다. 커서는 경로로 서므로
    /// 수가 늦게 와도 설 자리는 안 바뀐다.
    fn launch(&mut self) {
        if self.at != At::Layer || self.pending.is_some() {
            return;
        }
        let stale = self.stale();
        if stale.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let now = crate::model::now();
        let lang = self.lang;
        let handle = std::thread::spawn(move || look_into(&stale, &now, lang, &tx));
        self.pending = Some((rx, handle));
    }

    /// 지금 선 자리의 **헤더 번호** — `0` 이 층(`<0> 전체`)이고 그다음이 등록 차례다. 목록에서
    /// 빠진 경로에 서 있으면 번호가 없다.
    ///
    /// **한 곳에서 읽는다.** 헤더가 빛을 세울 자리를 고르는 것(`draw::with_projects`)과 맨
    /// 숫자가 "이미 그 자리인가" 를 가르는 것(`App::key` 의 [`super::keys::Browse::Project`])이
    /// 같은 답을 봐야 한다 — 따로 세면 한쪽의 첨자 기준만 옮겨도 헤더는 `<2>` 를 빛내는데 `2` 가
    /// 그 프로젝트를 다시 열어 커서와 굴린 자리를 첫 줄로 튕긴다.
    pub(super) fn number(&self) -> Option<usize> {
        match &self.at {
            At::Layer => Some(0),
            At::Project(p) => self.position(p).map(|at| at + 1),
        }
    }
}

impl App {
    /// 층에 서 있는가.
    pub fn on_layer(&self) -> bool {
        self.layer.as_ref().is_some_and(|l| l.at == At::Layer)
    }

    /// 지금 서 있는 프로젝트의 줄. 층에 섰거나 층이 없으면 `None` — 층이 없을 때의
    /// 프로젝트는 `App::repo` 하나뿐이다.
    pub fn project(&self) -> Option<&Place> {
        let l = self.layer.as_ref()?;
        match &l.at {
            At::Project(p) => l.places.get(l.position(p)?),
            At::Layer => None,
        }
    }

    /// 층 줄의 정체.
    pub(super) fn place_path(&self, at: usize) -> Option<&Path> {
        self.layer.as_ref()?.places.get(at).map(|p| p.path.as_path())
    }

    /// `.moai` 밖에서 띄운 탐색기 — 층에서 시작하고, **읽기는 스레드에 맡긴다**([`Layer::launch`]).
    /// 첫 화면은 줄마다 `읽는 중` 으로 서고 읽는 대로 수가 찬다 — 그 자리에서 다 읽으면 등록한
    /// 프로젝트의 값을 다 더한 만큼 첫 화면이 안 선다(moai-ezwu).
    pub fn on_projects(layer: Layer) -> App {
        let mut app = App::build(Vec::new(), Index::of(&[]), Default::default(), blank_config(), Vec::new(), Vec::new());
        let mut layer = Layer { at: At::Layer, ..layer };
        layer.launch();
        app.layer = Some(layer);
        app
    }

    /// 안에서 띄운 탐색기에 층을 얹는다 — **등록한 것이 읽혔을 때만.** 설정이 깨져 하나도
    /// 안 읽혔으면 층은 안 서지만 까닭을 [`App::unlayered`] 에 붙여 배너가 댄다. 층이
    /// 조용히 사라지면 여러 프로젝트 보기가 고장 난 줄만 알고, 같은 자리의 `moai status`
    /// 는 파싱 오류를 댄다. 층이 서면 층이 제 `problems` 로 댄다.
    pub fn attach_layer(self, layer: Layer) -> App {
        if layer.registered() {
            return self.with_layer(layer);
        }
        let mut app = self;
        app.unlayered = unlayered_of(&layer);
        app
    }

    /// 층을 얹기만 한다 — **커서는 안 건드린다.** 한때 여기서 `..` 너머 첫 줄로 밀었는데,
    /// 층이 서면서 뿌리에 `..` 이 새로 생기던 시절의 일이다. 뿌리의 `..` 을 걷은 뒤(moai-i784)
    /// 층이 서도 줄은 하나도 안 밀리므로 밀 것이 없고, 미는 척하는 한 줄이 남아 있으면 그것을
    /// 위해 뿌리의 목록을 통째로 세고 정렬하는 값(`first_row` → `rows`)을 띄울 때마다 치른다.
    pub fn with_layer(mut self, mut layer: Layer) -> App {
        // **말은 화면에서 온다**(moai-ra67) — 층은 설정에서 나므로 제 말을 모른다. 얹는 문이
        // 여기 하나라, 부르는 쪽마다 손으로 이어 주던 판이 한 자리만 빠뜨려도 못 연 프로젝트의
        // 한 줄만 딴 말로 섰다.
        layer.lang = self.site.lang;
        self.layer = Some(layer);
        self
    }

    /// 층의 줄을 **지금의 디렉터리로** 연다. 못 열면 그 줄을 고쳐 세우고 CLI 한눈 보기와 같은
    /// 말(`view::unopened`)을 알림으로 댄 뒤 `None` — 들어가기(Enter)와 담기(`n`)가 같은 길이라
    /// 같은 상태를 두 키가 달리 부르지 않는다.
    ///
    /// **한 상태만 갈린다**(moai-lmot) — 열리지만 스냅샷을 못 읽는 저장소다. `n` 은 [`Depth::Whole`]
    /// 로 물어 여기서 `view::unopened` 의 말을 받고, Enter 는 [`Depth::Lean`] 이라 곧 뒤따르는
    /// 읽기가 제 까닭을 댄다("들어가지 못했다 — …"). 둘 다 안 들어가고 줄도 다시 읽히지만,
    /// **말은 다르다** — 여기서 두 벌로 적은 것이 아니라 읽는 자리가 둘이라 그렇다.
    ///
    /// **도는 층 읽기는 버린다**(moai-800o). 그것은 이 줄을 재기 **전에** 띄운 것이라, 늦게 닿으면
    /// 여기서 고쳐 세운 줄(`Look::Shut`)을 옛 디렉터리의 값으로 덮는다. 버린 줄은 층에 선 다음
    /// 걸음에 [`Layer::stale`] 이 다시 고른다 — 못 들어갔거나 `n` 으로 층에 남았으면 곧바로,
    /// 들어갔으면 올라올 때다.
    pub(super) fn open_place(&mut self, at: usize, how: Depth) -> Option<Repo> {
        if let Some((_, handle)) = self.layer.as_mut()?.pending.take() {
            self.discard(handle);
        }
        let place = self.layer.as_mut()?.places.get_mut(at)?;
        let marks = marks_of(&place.path);
        // **여는 길은 층의 줄과 같다**([`look_one`] 의 `projects::open_one`) — 스냅샷까지 읽어 본다.
        // `Repo::open` 만으로는 `.moai/config.toml` 까지만 보여, 층의 줄은 "못 읽는다" 로 서는데 `n` 은
        // 폼을 열고 사람은 다 적고 Ctrl-S 를 눌러서야 못 담는다고 듣는다(적은 것이 갈 데가 없다). 여는
        // 법을 두 벌로 적으면 한쪽만 고쳐져 Enter 와 층의 줄이 같은 디렉터리를 달리 가른다. 한 번 더
        // 읽는 값은 사람이 키를 누른 한 번뿐이라 싸다.
        //
        // **줄을 곧 스레드가 읽을 자리는 그 한 번도 안 읽는다**([`Depth::Lean`], moai-m59y).
        let opened = match how {
            Depth::Whole => match projects::open_one(&place.path, String::new(), None, false).state {
                State::Open { repo, .. } => Ok(repo),
                state => Err(state),
            },
            Depth::Lean => projects::open_shallow(&place.path),
        };
        let state = match opened {
            // **열린 줄은 곧바로 다시 읽을 줄로 둔다**(리뷰 moai-3lul.kt0 다시 본 판) — 층의 셈이
            // "못 읽는다" 나 옛 수로 서 있어도 방금 연 것이 지금이다. 층에 남았으면(`n`) 다음 걸음에,
            // 들어갔으면 어느 길로 떠나든(올라오기·옆 번호) 올라온 뒤에 다시 읽는다. 셈은 새것이 닿을
            // 때까지 그대로 선다.
            Ok(repo) => {
                place.read_at = None;
                return Some(repo);
            }
            Err(state) => state,
        };
        place.look = shut(&place.path, &place.name, state, self.site.lang);
        place.marks = marks;
        // 들인 때로 찍는다 — [`Layer::adopt`] 와 같은 자다.
        place.read_at = Some(std::time::Instant::now());
        if let Look::Shut { said, .. } = &place.look {
            self.notice = Some(said.clone());
        }
        None
    }

    /// 지금 선 프로젝트의 정체 — **쓰기가 닿는 곳.** 층이 있으면 `At::Project` 의 경로(그동안
    /// `repo` 는 그 프로젝트의 것이다), 층이 없으면 저장소 뿌리. 층에 섰으면 `None` 이다.
    pub fn here(&self) -> Option<PathBuf> {
        match &self.layer {
            Some(l) => match &l.at {
                At::Project(p) => Some(p.clone()),
                At::Layer => None,
            },
            // 층이 없으면 선 체크아웃이다(`Repo::here`) — 층의 줄과 같은 자로 적힌다(`cmd::tui`).
            None => self.site.repo.as_ref().map(|r| r.here().to_path_buf()),
        }
    }

    /// `n` — **담을 곳을 박아** 생각 담기 폼을 연다(moai-fccv).
    ///
    /// 프로젝트 안이면 지금 선 곳이다. 층에 섰으면 커서의 프로젝트를 담을 곳으로 제안한다 —
    /// 층에 선 채 폼을 열고, 머리가 그 프로젝트를 댄다. 여는 순간 [`App::open_place`] 로 열어
    /// 보고, 못 열면(init 전·없음·못 읽음) **폼을 안 연다** — 적고 나서야 못 담는다고 들으면
    /// 적은 것이 갈 데가 없다. 담는 순간 그 프로젝트로 들어간다([`App::stand_at`]).
    pub(super) fn open_form(&mut self) {
        let into = if self.on_layer() {
            // **커서가 선 줄의 프로젝트에 담는다**(moai-5v3q) — 머리줄이면 그 프로젝트고, 그 밑의
            // 이슈 줄이면 그 줄이 사는 프로젝트다. 한눈 보기는 프로젝트를 가로지르므로 "지금 선
            // 프로젝트" 라는 것이 없다 — 커서가 그것을 댄다.
            let at = match self.current() {
                Some(Row::Project(at)) => at,
                Some(Row::Item(super::Seat::Place(at), ..)) => at,
                _ => {
                    self.notice = Some("담을 프로젝트가 없다 — `moai project add <dir>` 로 등록하면 여기 선다".into());
                    return;
                }
            };
            if self.open_place(at, Depth::Whole).is_none() {
                return;
            }
            self.layer.as_ref().and_then(|l| l.places.get(at)).map(|p| Target { path: p.path.clone(), name: p.name.clone() })
        } else {
            self.here().map(|path| {
                let name = match self.project() {
                    Some(p) => p.name.clone(),
                    None => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string()),
                };
                Target { path, name }
            })
        };
        // 편집기가 있으면 **루프에 맡긴다**(moai-08af) — 터미널을 내리는 것은 App 이 못 한다.
        // 담을 곳은 여기서 박힌 그대로 요청에 실려 가고, 돌아온 글이 [`App::edited`] 로 담긴다.
        match &self.editor {
            Some(editor) => {
                let text = super::jotfile::template(into.as_ref());
                self.edit = Some(super::Edit { into, text, editor: editor.clone() });
            }
            None => self.mode = super::Mode::Idea(Form::new(into)),
        }
    }

    /// 폼이 박은 담을 곳에 **선다** — 쓰기 바로 앞에서 부른다. 서면 참이고, 못 서면 쓰기의
    /// 실패로 까닭을 달고(`trouble`, 폼은 적던 그대로) 거짓이다.
    ///
    /// 지금 선 곳이 그 경로면 그대로다. 층에 섰으면 그 경로의 줄을 **경로로** 다시 찾아
    /// 들어간다 — 첨자로 들면 층이 다시 읽혀 차례가 바뀐 뒤 옆 프로젝트에 들어간다. 그 밖에
    /// 선 곳이 다르면(다른 프로젝트 안) **쓰지 않는다.** 폼이 열린 동안 선 곳을 옮기는 키는
    /// 없지만, 그 불변식에 기대지 않고 쓰는 자리에서 거른다 — 머리에 보인 곳 말고 다른 곳에
    /// 쓰는 길을 구조로 막는다.
    pub(super) fn stand_at(&mut self, into: Option<&Target>) -> bool {
        let want = into.map(|t| t.path.clone());
        if self.here() == want {
            return true;
        }
        let why = match into {
            // 문구 속 키 이름은 표에서 읽는다 — 키를 옮기면 이 말도 따라온다.
            None => format!("담을 곳 없이 연 폼이다 — {} 로 닫고 프로젝트 안에서 다시 {}", label(JOT, Jot::Close), label(BROWSE, Browse::Jot)),
            Some(t) if self.on_layer() => {
                let name = crate::text::one_line(&t.name);
                match self.layer.as_ref().and_then(|l| l.position(&t.path)) {
                    Some(at) => {
                        self.enter_project(at);
                        if self.here() == want {
                            return true;
                        }
                        // 들어가기가 댄 까닭(못 연 말·못 읽은 말)을 옮긴다. 앞의 글리프는 배너의 `!` 와 겹친다.
                        let said = self.notice.take().unwrap_or_default();
                        format!("{name} 에 못 들어갔다 · {}", said.trim_start_matches(['·', '!', ' ']).trim_start_matches("들어가지 못했다 — "))
                    }
                    None => format!("{name} 이 프로젝트 층에서 빠졌다 — {} 로 닫고 다시 고른다", label(JOT, Jot::Close)),
                }
            }
            Some(t) => format!(
                "폼을 연 곳({})과 지금 선 곳이 다르다 — {} 로 닫고 다시 {}",
                crate::text::one_line(&t.name),
                label(JOT, Jot::Close),
                label(BROWSE, Browse::Jot)
            ),
        };
        self.notice = None;
        self.trouble = Some(format!("쓰지 못했다 — {}", why.trim()));
        self.write_failed = true;
        false
    }

    /// 층의 줄로 들어간다. **늘 `Repo::open` 부터 다시 한다** — 층의 셈이 낡았어도 들어가는
    /// 것은 지금의 디렉터리다. 못 열면 층에 선 채 그 까닭을 알림으로 댄다(그 줄도 고쳐 선다).
    ///
    /// 읽기는 그 자리에서 한다. 누른 사람은 결과를 기다리고 있다(`App::reload` 와 같다).
    ///
    /// **한 프로젝트에 매인 것은 여기서도 푼다**([`App::leave_project`]). 한때 이 길은 층을
    /// 거쳐서만 닿았고, 푸는 일은 [`App::climb`] 하나가 맡았다 — 헤더의 번호(moai-o133)가 옆
    /// 프로젝트로 바로 건너뛰는 길을 내면서 그 길이 안 도는 드나들기가 생겼다. **겹쳐 보기는
    /// 읽기 전에 정한다** — 읽는 값이 그 깃발을 탄다.
    pub(super) fn enter_project(&mut self, at: usize) {
        // **여는 데까지만 본다**(moai-lmot) — 줄은 바로 아래에서 읽는다. 스냅샷까지 열어 보던
        // 때는 그 판의 `load` 를 버리고 두 줄 뒤에서 같은 파일을 또 팠다. 갈리는 것은 하나다:
        // 열리지만 스냅샷을 못 읽는 저장소를 여태는 여기서 `Look::Shut` 으로 고쳐 세웠고, 이제는
        // 아래의 읽기가 제 까닭을 알림으로 댄다("들어가지 못했다 — …"). 그 줄은 `read_at` 이
        // 비어 다음 걸음의 쓸기가 다시 읽어 제 상태로 선다 — 말은 남고 줄은 저절로 낫는다.
        let Some(repo) = self.open_place(at, Depth::Lean) else { return };
        let Some(path) = self.place_path(at).map(Path::to_path_buf) else { return };
        // **겹쳐 보기를 켠다는 말은 한 번만 적는다.** 읽는 값이 이 깃발을 타므로 읽기에 건네는
        // 것과 App 에 남기는 것이 갈리면 목록은 겹친 줄인데 뱃지는 꺼진 화면이 난다. 세우는
        // 것은 **읽은 뒤**다 — 못 읽으면 선 자리도 깃발도 그대로여야 한다.
        let overlay = true;
        match (self.read)(&repo, overlay) {
            Ok(fresh) => {
                // 떠난 프로젝트에 매인 것을 푼다 — 층에서 왔으면 이미 풀린 것을 한 번 더 풀 뿐이다.
                let leaving = match &self.layer.as_ref().map(|l| &l.at) {
                    Some(At::Project(p)) => Some(p.clone()),
                    _ => None,
                };
                self.leave_project(leaving);
                if let Some(layer) = &mut self.layer {
                    layer.at = At::Project(path);
                }
                self.worktree = overlay;
                // 누군지도 **그 프로젝트의 뿌리에서** 다시 푼다(moai-j038.vna) — 헤더(`told_user`)가 뿌리마다
                // 다시 푸는 것과 같은 까닭이다(moai-d3sy): 프로젝트마다 git 설정이 다를 수 있고, 안 풀면 [NEW]
                // 가 띄운 자리의 사람으로 서서 `moai -C <그 프로젝트> read --all` 과 다른 줄을 센다. 안 읽음은
                // 들이기(`apply_fresh`)가 세므로 그 **앞**이다.
                self.me = self.whoami(&repo.root);
                self.site.cfg = repo.config.clone();
                self.site.repo = Some(repo);
                // **그 줄이 이미 들고 있던 읽음을 베껴 든다**(moai-2gep) — 펼쳐 본 프로젝트는 제 표를
                // 들고 선다. 아래의 `load_read` 가 그 파일을 못 읽으면(옛 `sudo moai read` 가 남긴
                // root 의 파일) 들일 것이 없어 내게 온 줄이 모두 [NEW] 로 서고, 그 화면의 `SPC m a`
                // 한 번이 그것을 다 찍는다. 세워 둔 줄에서 드는 [`App::follow_site`] 와 같은 자다.
                //
                // **읽기가 되면 아래가 덮는다** — 여기 든 것은 못 읽을 때의 밑값이지 답이 아니다.
                //
                // **다만 베낀다**(리뷰) — `follow_site` 는 그 줄의 `Site` 를 통째로 갈아 끼우는 길이라
                // 옮겨 들어도 되지만("그 줄은 곧 버려진다"), 이 길은 줄을 그대로 두고 들어간다. 옮겨
                // 들면 그 줄이 **빈 표에 멀쩡한 갈래**로 남아, 그 줄을 다시 세는 자가 내게 온 것을 모두
                // [NEW] 로 센다([`App::recount_unread_in`] 의 문지기가 갈래를 보므로 그 꼴은 못 막는다)
                // — 이 이슈가 없애려던 바로 그 화면이다.
                //
                // **갈래와 읽어 본 때는 안 든다** — `follow_site` 와 같은 까닭이다. 바로 아래 `load_read`
                // 가 이 걸음의 읽기로 다시 적어 덮이고, 덮이기 전에 한 번 읽히는 데가 있다(그 `knew`).
                // 게다가 `Tried::since` 까지 따라와 방금 난 실패가 오래된 것으로 보여, [`owed`] 가 잠깐
                // 봐 주는 틈(`BLIP`)을 건너뛰고 시계로 내려앉는다.
                let carried = self.site_mut(super::Seat::Place(at)).map(|h| (h.seen.clone(), h.read_stamp));
                if let Some((seen, stamp)) = carried {
                    self.site.seen = seen;
                    self.site.read_stamp = stamp;
                }
                // **그 프로젝트의 읽음을 여기서 든다**(리뷰) — [`App::leave_project`] 가 `Site` 를 비워
                // `seen` 이 빈 표라, 안 들면 아래의 들이기가 그 빈 표로 세어 **내게 온 줄이 모두** [NEW]
                // 로 그려진다. 걸음이 그것을 고치지만 걸음은 최대 700ms 뒤고, 그사이 `SPC m a` 를 누르면
                // 안 든 표로 잰 "안 읽음" 전부에 도장을 찍는다. 자리는 `me` 를 다시 푸는 바로 여기다.
                self.load_read();
                self.cursor = 0;
                // 떠난 프로젝트의 줄은 `leave_project` 가 이미 비웠다 — 두 프로젝트가 같은 prefix 를
                // 쓰면(`argos-0001`) 남은 줄의 id 로 들이기가 커서를 붙들어 남의 줄 번호에 섰다.
                // 들이기가 커서를 **첫 줄에** 세우고 상세를 되감는다 — 줄을 비운 뒤라 붙들 정체가 없고
                // (`leave_project`), 뿌리에는 `..` 이 없어(moai-i784) 첫 줄이 곧 첫 이슈다. 디렉터리에
                // 들어갈 때와 같은 자리다(`App::first_row`, moai-cm13). 여기서 목록을 또 세지 않는다(moai-go4o).
                self.apply_fresh(fresh);
            }
            Err(e) => self.notice = Some(format!("들어가지 못했다 — {e}")),
        }
    }

    /// 프로젝트 뿌리에서 층으로 올라간다. **떠난 프로젝트에 선다.**
    ///
    /// 그 프로젝트의 줄을 비운다 — 층에서는 어느 프로젝트에도 쓸 수 없어야 하고(`App::write`
    /// 는 `repo` 가 없으면 멈춘다), 옛 id 가 다음 프로젝트의 커서 정체로 새면 안 된다. 도는
    /// 읽기도 버린다. 거름망은 푼다 — 한 프로젝트의 줄과 칸 이름에 매인 것이다.
    ///
    /// 안에 있는 동안 낡은 줄은 **스레드로** 다시 읽는다([`Layer::launch`], moai-ezwu) — 올라오는
    /// 키가 그 값을 기다리지 않는다. 그동안 그 줄은 옛 셈을 낸다.
    ///
    /// **떠난 프로젝트의 줄은 표식이 그대로여도 다시 읽는다**([`Layer::forget`]). 방금 안에서 본 배너와
    /// 올라와 보는 그 줄이 같은 수를 대야 한다 — 층의 줄은 1분 시계가 따로 돌아, 안에 있는 동안 한 시간
    /// 틈을 넘긴 줄이 안쪽 배너에는 서고 층에는 최대 1분 안 섰다. 못 읽던 줄에 방금 들어갔다 나왔는데
    /// "못 읽는다" 가 남는 것도 같은 자리다(리뷰 moai-3lul.kt0 다시 본 판). 한 줄 읽기라 싸다.
    pub(super) fn climb(&mut self) {
        let Some(layer) = &mut self.layer else { return };
        let At::Project(from) = std::mem::replace(&mut layer.at, At::Layer) else { return };
        self.leave_project(Some(from.clone()));
        // **시각은 안 올린다** — 머리의 `↻` 가 그것으로 "방금 갱신했다" 를 말하는데, 읽기는 이제
        // 스레드로 가서 아직 안 왔다(리뷰 moai-3lul.kt0). 들일 때 [`App::follow_layer`] 가 올린다.
        let Some(layer) = &mut self.layer else { return };
        layer.forget(&from);
        layer.launch();
        let at = layer.position(&from).unwrap_or(0);
        self.stand_on_place(at);
    }

    /// 한눈 보기에서 커서를 그 프로젝트의 **머리줄**에 세운다(moai-i0wd). **층 줄의 첨자는 이제 줄
    /// 번호가 아니다** — 펼친 프로젝트가 제 줄을 이고 서므로, 첨자를 커서에 그대로 넣으면 남의
    /// 프로젝트의 이슈 줄에 선다.
    pub(super) fn stand_on_place(&mut self, at: usize) {
        self.cursor = self.rows().iter().position(|r| matches!(r, Row::Project(n) if *n == at)).unwrap_or(0);
    }

    /// **한 프로젝트에 매인 것을 모두 푼다** — 떠나는 두 길(올라가기 [`App::climb`], 옆으로
    /// 건너가기 [`App::enter_project`])이 이것 하나를 부른다(moai-800o). 한때 두 길이 목록을
    /// 따로 들었다 — 한쪽은 여기서 비우고 다른 쪽은 `apply_fresh` 의 갈래에 기대어, 같은 답을
    /// 다른 길로 냈다. 그래서 들이기를 고치면 한쪽만 깨졌고, 건너가는 길은 짓던 커밋 표를 안
    /// 버려 떠난 뿌리의 표가 새 프로젝트의 표에 섞였다. **프로젝트에 매인 것을 새로 들이면
    /// 여기에 적는다.** 선 자리(`Layer::at`)와 커서는 부르는 쪽이 정한다 — 어디로 가느냐가 다르다.
    fn leave_project(&mut self, park_at: Option<PathBuf>) {
        if let Some((_, handle)) = self.pending.take() {
            self.discard(handle);
        }
        // 커밋 표도 프로젝트에 매인 것이다(moai-a4i0). 짓던 것을 놓지 않으면 떠난 프로젝트의
        // 이력을 마저 걷는 동안 층이 빠른 걸음으로 깨어 있고, 그 답이 다음 프로젝트의 표를
        // 세우는 자리를 막는다(`follow_commits` 는 도는 것이 있으면 새로 안 띄운다).
        if let Some((_, handle)) = self.commits_job.take() {
            self.discard(handle);
        }
        // **떠난 프로젝트의 줄은 통째로 그 층 줄에 둔다**(moai-i0wd) — 한눈 보기가 그 밑에 세울
        // 것이고, 방금 떠난 프로젝트가 접힌 채로 서면 `0` 을 누른 화면이 늘 비어 보인다(사용자
        // 결정 2026-09-19: "있던 프로젝트만 펼쳐 둔다"). 커밋 표·안 읽음·못 읽는 줄이 함께 가므로
        // 다음 프로젝트에 섞이지 않는다 — 한때 필드마다 비우던 것이 그 섞임을 막으려는 일이었다.
        //
        // **그 프로젝트 안에서 어디를 보고 있었나는 안 든다.** 경로·커서 기억·펼친 자리는 도로
        // 들어갈 때 처음부터다 — 마디가 그 프로젝트의 이슈 id 라 남겨 두면 지운 줄의 id 가 쌓인다.
        let blank = super::Site::of(Vec::new(), Index::of(&[]), Default::default(), self.site.cfg.clone(), Vec::new(), Vec::new());
        let mut parked = std::mem::replace(&mut self.site, blank);
        parked.path.clear();
        parked.remembered.clear();
        parked.expanded.clear();
        // **거름망 마스크도 푼다**(리뷰). 아래에서 `filter_text` 를 비우므로 걸린 글은 사라지는데,
        // 마스크를 두고 가면 그 프로젝트의 줄은 한눈 보기에서 걸러진 채 서고 — 경로 줄에 뱃지도
        // 없고 Esc 로 풀 것도 없어 — 왜 줄이 적은지 말할 자리가 도구 안에 안 남는다.
        parked.keep = vec![true; parked.issues.len()];
        if let (Some(at), Some(layer)) = (park_at, self.layer.as_mut())
            && let Some(place) = layer.places.iter_mut().find(|p| p.path == at)
        {
            place.site = Some(parked);
        }
        self.filter_text = None;
        // **보기는 돌리지 않는다**(moai-2bzp). 보기·정렬·열은 사람의 설정이라 사용자 설정에 적혀
        // 프로젝트를 옮겨도 이어진다 — 한때(moai-fmv5) 여기서 처음값으로 돌렸는데, 그러면 저장한
        // 보기가 층을 한 번 오갈 때마다 사라진다. 그때의 까닭(다른 프로젝트의 칸 이름이 뱃지에 남아
        // 걷을 길이 없다)은 뱃지가 이 프로젝트의 칸만 대게 해 풀었다(`View::badge`).
        // **겹쳐 보기는 기본값(켬)으로 돌린다.** 한 프로젝트에서 `w` 로 끈 것은 그
        // 프로젝트에 매인 뜻이다 — 층에서는 `w` 가 안 먹어 되켤 길이 없는 채로, 다음
        // 프로젝트가 시키지도 않은 끈 화면으로 읽힌다. 거름망과 같은 까닭이다.
        self.worktree = true;
        self.trouble = None;
        self.write_failed = false;
        self.site.path.clear();
        self.site.remembered.clear();
        // 펼쳐 둔 자리도 그 프로젝트에 매인 것이다 — 마디가 그 프로젝트의 이슈 id 다. 두고 오면
        // 같은 prefix 를 쓰는 다음 프로젝트에서 아무도 안 펼친 묶음이 펼쳐진 채 서고(위의 커서
        // 정체와 같은 자리), 바구니 마디(`Milestone(None)`·`Lost`)는 id 조차 없어 prefix 가 달라도
        // 그대로 샌다. 세션 내내 쌓이기도 한다.
        self.site.expanded.clear();
        // 펴 둔 본문도 그 프로젝트에 매인 것이다(리뷰) — 글과 id 가 떠난 줄의 것이다. 상세를
        // 연 채면 다음 프레임의 `draw::fill_body` 가 곧 갈아 끼우지만, `SPC v p` 로 상세를
        // 닫아 둔 채 떠나면 그리는 쪽이 안 돌아 큰 본문 한 벌이 세션 내내 남는다.
        self.body = None;
        self.detail.rewind();
    }

    /// **등록 목록이 바뀐 뒤** 층을 다시 세운다 — 이 탐색기가 바꾼 것(층의 `a`·`d`, moai-plvy)과 밖에서
    /// 바꾼 것(`moai project add|rm`, 설정 파일의 표식으로 안다 — [`App::follow_config`], moai-en4u)이
    /// 같은 길이다. 층을 다시 세우는 길은 이것 하나다 — 손으로 "전부 다시" 읽던 `SPC r` 은 걷었다.
    ///
    /// - **선 자리를 둔다.** 프로젝트 안에서 `a` 로 등록해도 층으로 끌어올리지 않는다
    /// - **이미 본 프로젝트를 일부러 다시 읽지 않는다.** 경로가 같은 줄의 셈·표식·읽은 때를 옮겨 들고,
    ///   층에 섰으면 [`Layer::launch`] 로 스레드에서 낡은 줄만 읽는다 — 새로 선 줄과, 원래 낡았던
    ///   줄(표식이 바뀌었거나 시계로 낡은 것)이다. 한 줄을 더하거나 뺀 것이라, 등록 수만큼 저장소를
    ///   다시 읽을 까닭이 없다. 그 자리에서 읽으면 시계로 낡은 줄까지 함께 걸려 등록 하나 바꾸는 키가
    ///   등록 수만큼 멈춘다(리뷰 moai-3lul.kt0)
    /// - **층이 없었으면 세운다.** `.moai` 안에서 띄웠고 등록이 0 이었던 경우다. 띄운 자리가
    ///   `At::Project` 로 서므로 지금 프로젝트는 그대로이고, 층이 새로 서도 뿌리의 줄은 그대로라
    ///   (`..` 은 디렉터리에만 선다, moai-i784) 커서는 보던 줄(정체)에 선다. 못 세우면 그 까닭
    ///   ([`App::unlayered`])을 **이번 읽기로** 다시 단다 — 설정을 고쳤으면 옛 까닭이 걷히고, 깨졌으면
    ///   새 까닭이 선다
    ///
    /// `land` 가 있으면 층에 섰을 때 커서를 그 프로젝트(경로)에 둔다 — 방금 등록한 것이 눈앞에
    /// 있어야 등록된 줄 안다. 없거나 못 찾으면 보던 줄, 그것도 사라졌으면(뺐으면) 그 번호를 자른 자리.
    ///
    /// **탈이 있으면 아무것도 안 갈아 끼운다**(moai-9p7v·moai-po6v, 사용자 결정 2026-09-19) — 들고 있던
    /// 층이 그대로 남고 까닭 한 줄만 새 읽기의 것으로 바뀐다([`holding`]). 깨진 설정도 여기 든다:
    /// 한때는 빈 층과 그 까닭이 고칠 것을 비춘다고 봤는데, 오타 하나에 등록한 프로젝트가 통째로
    /// 사라졌다. 비추는 데 필요한 것은 빈 층이 아니라 까닭 한 줄이다.
    pub(super) fn relayer(&mut self, land: Option<&Path>) {
        self.relayer_with(None, land);
    }

    /// [`App::relayer`] 와 같되 **이미 읽은 설정**을 쓴다(moai-7yil) — 같은 걸음에 읽음도 그 설정에서
    /// 드는 [`App::follow_config`] 가 한 번의 읽기를 나눠 쓴다.
    ///
    /// `None` 이면 제가 읽는다 — **층을 다시 세우는 길은 세우던 층의 파일(`Layer::config`)에서, 처음
    /// 세우는 길은 `App::user_config` 에서.** 둘은 같은 자리를 가리키도록 `cmd::tui` 가 한 번에 준다
    /// (`user_config::path`). `Some` 을 받으면 그 설정의 자리가 곧 새 층의 자리다(`Layer::of` 가
    /// `Registry::path` 를 든다, moai-y61p) — 부르는 쪽이 같은 파일을 읽어 넘길 때만 맞는 말이라,
    /// 다른 파일의 설정을 넘기면 층은 한 파일에서 서고 등록·해제는 다른 파일에 간다.
    pub(super) fn relayer_with(&mut self, reg: Option<&user_config::Registry>, land: Option<&Path>) {
        let held = self.current().map(|r| self.anchor_of(&r));
        match self.layer.take() {
            None => {
                let Some(repo) = &self.site.repo else { return };
                let here = Some(repo.here().to_path_buf());
                let mut fresh = match reg {
                    Some(reg) => Layer::of(reg, here.as_deref()),
                    None => Layer::read(self.user_config.as_deref(), here.as_deref()),
                };
                // **말은 화면에서 온다**(moai-ra67) — 다시 세운 층은 설정에서 나므로 제 말을 모른다.
                // 안 이어 주면 설정 파일이 한 번 바뀔 때마다 못 연 프로젝트의 한 줄만 기본 말로 돌아간다.
                fresh.lang = self.site.lang;
                // 못 읽었으면 세우지 않는다 — 다만 **까닭은 댄다**(리뷰). 층이 없는 화면에서는 이 배너가
                // 유일한 말이라, 조용히 돌아서면 쭉 못 읽는 설정이 아무 말 없이 빈 화면으로 선다. 다음
                // 읽기가 되면 그때 걷힌다([`unlayered_of`] 는 댈 까닭이 없으면 `None` 이다 — 없는 파일이
                // 그렇다: 아직 아무것도 등록 안 한 사람의 정상이라 잔소리할 자리가 아니다).
                //
                // **갈래를 안 가린다**(moai-po6v) — 여기서 세울 것은 어차피 없다. 못 읽었으면 목록이
                // 비고, 깨졌으면 파싱이 진 자리라 역시 비어, 빈 층을 세우는 일과 안 세우는 일이 같다.
                if fresh.trouble.is_some() || !fresh.registered() {
                    self.unlayered = unlayered_of(&fresh);
                    // 층이 없으면 들고 있는 것도 없다 — 이 화면의 말은 위의 한 줄이다.
                    self.held = None;
                    return;
                }
                self.unlayered = None;
                self.held = None;
                self.layer = Some(fresh);
            }
            Some(mut old) => {
                let mut fresh = match reg {
                    Some(reg) => Layer::of(reg, old.launch.as_deref()),
                    None => Layer::read(old.config.as_deref(), old.launch.as_deref()),
                };
                // 말은 화면에서 온다 — 위와 같은 자리다(moai-ra67).
                fresh.lang = self.site.lang;
                // **탈이 있으면 들고 있던 층을 두고 까닭을 단다**(moai-po6v, 사용자 결정 2026-09-19).
                // 빈 층으로 갈아 끼우면 줄이 통째로 사라지는데, 손으로 누르던 비상구(`SPC r`)는
                // 걷었다(moai-en4u) — 되돌릴 길이 도구 밖에만 남는다.
                //
                // **갈래를 안 가린다.** 한때는 잠깐의 실패(`Reading`)만 들고 서고 깨진 설정은 빈 층으로
                // 갈아 끼웠는데(moai-9p7v), 사람이 `[tui]` 에 오타 하나를 낸 순간 등록한 프로젝트가
                // 통째로 사라졌다. 고칠 것을 비추는 데 필요한 것은 빈 층이 아니라 **까닭 한 줄**이고,
                // 그것은 아래가 단다.
                //
                // **까닭은 새 읽기의 것으로 갈아 끼운다** — 들고 있던 층이 지난 읽기의 까닭을 이고 있으면
                // 지금 무엇이 어긋났는지를 덮는다.
                if fresh.trouble.is_some() {
                    // **까닭은 `App::held` 로 낸다**(moai-23pm) — 층의 `problems` 는 **읽힌** 설정의
                    // 틀린 줄을 대는 자리라 배너가 `on_layer()` 로 막는다(등록 목록의 일이다). 파일을
                    // 통째로 못 읽은 것은 그 화면과 상관없이 대야 한다.
                    //
                    // **들고 있던 `problems` 는 그대로 둔다** — 지난 읽기의 것이지만 지금 들고 선
                    // 바로 그 줄들을 설명한다. 갈아 끼우면 왜 저 줄이 빠졌는지가 사라진다.
                    //
                    // **든 줄이 없으면 걷는다**(리뷰) — 그때 그 글이 설명할 줄이 하나도 없다. 두면
                    // 지난 읽기의 까닭이 지금 까닭 옆에 표식도 없이 서서(`draw::banner` 가 `held`
                    // 다음에 `Layer::problems` 를 잇는다), 고친 줄을 다시 고치라고 하거나 같은 글이
                    // 두 번 선다. 한때 이 자리가 통째로 갈아 끼우던 까닭이 그것이다.
                    self.held = holding(&fresh, old.registered());
                    if !old.registered() {
                        old.problems.clear();
                    }
                    old.trouble = fresh.trouble;
                    self.layer = Some(old);
                    return;
                }
                for p in &mut fresh.places {
                    if let Some(o) = old.places.iter_mut().find(|o| o.path == p.path) {
                        p.look = std::mem::replace(&mut o.look, Look::Unread);
                        p.marks = std::mem::take(&mut o.marks);
                        p.read_at = o.read_at;
                        // **읽어 든 줄도 옮겨 든다**(리뷰). 두고 가면 한눈 보기가 펼쳐 둔 프로젝트가
                        // 통째로 접힌 머리줄만 남는다 — 이 길은 사용자 설정이 바뀔 때마다 도는데
                        // (`App::follow_config`) 보기 토글과 읽음이 **그 파일을 스스로 쓴다.**
                        // 곧 `SPC v` 한 번이 그 토글을 걸 줄을 다 없앤다. 다시 읽으러 가는 자도
                        // 없다(`App::want_site` 는 펼칠 때만 부른다).
                        p.site = o.site.take();
                    }
                }
                fresh.at = match old.at {
                    At::Layer => At::Layer,
                    // 띄운 자리를 등록하면 따로 섰던 줄이 등록 줄로 합쳐지고, 등록 철자가 띄운
                    // 뿌리와 다를 수 있다(링크) — 그때는 새 층이 정한 띄운 자리로 옮겨 선다.
                    At::Project(p) if fresh.position(&p).is_none() && old.launch.as_deref() == Some(p.as_path()) => {
                        fresh.at.clone()
                    }
                    At::Project(p) => At::Project(p),
                };
                // 도는 읽기는 경로로 맞춰 들이므로 넘겨도 섞이지 않는다. **줄을 읽는 쪽도 같이
                // 넘긴다**(리뷰) — 버리면 그 손잡이가 join 도 discard 도 없이 떨어져, 그 스레드가
                // 터져도 되던질 데가 없다(`App::discarded` 가 막으려는 바로 그 자리). 기다리던
                // 줄(`wanted`)도 넘긴다: 버리면 펼쳐 놓고 못 읽은 프로젝트가 영영 안 읽힌다.
                // 읽혔으니 들고 선 까닭을 걷는다 — 다음 읽기가 되면 배너에서 사라져야 한다.
                self.held = None;
                fresh.pending = old.pending.take();
                fresh.reading = old.reading.take();
                fresh.wanted = std::mem::take(&mut old.wanted);
                self.layer = Some(fresh);
            }
        }
        if let Some(layer) = &mut self.layer {
            layer.launch();
        }
        let rows = self.rows();
        let landed = land.filter(|_| self.on_layer()).and_then(|want| {
            let l = self.layer.as_ref()?;
            l.position(want).or_else(|| l.places.iter().position(|p| same_dir(&p.path, want)))
        });
        let at = landed.or_else(|| held.as_ref().and_then(|a| self.row_of(&rows, a))).unwrap_or(self.cursor);
        self.stand(&rows, at, held.as_ref());
    }

    /// 걸음마다 층을 본다. 스레드가 읽어 온 줄은 **어디 서 있든** 받는다 — 경로로 맞춰
    /// 들이므로 안에 들어간 뒤에 닿아도 섞일 데가 없다. **닿은 만큼 들인다**(리뷰 moai-3lul.kt0):
    /// 읽기는 줄마다 따로 오므로(`look_into`) 빠른 줄이 느린 줄을 안 기다린다. 새로 읽으러 가는
    /// 것은 **층에 선 동안만**이다([`Layer::launch`]): 안에 있는 동안 남의 프로젝트를 걸음마다 재고
    /// 읽을 까닭이 없고, 올라갈 때 낡은 줄을 읽으러 띄운다(`climb`).
    pub(super) fn follow_layer(&mut self) {
        let Some(layer) = &mut self.layer else { return };
        while let Some((rx, _)) = &layer.pending {
            match rx.try_recv() {
                Ok(looked) => {
                    layer.adopt([looked]);
                    if layer.at == At::Layer {
                        self.site.now = crate::model::now();
                    }
                }
                // 아직 읽는 중이다 — 다음 걸음에 마저 받는다.
                Err(TryRecvError::Empty) => break,
                // 다 보냈거나 읽던 스레드가 죽었다 — 죽었으면 받은 읽기와 같게 되던진다(`App::follow`).
                Err(TryRecvError::Disconnected) => {
                    if let Some((_, handle)) = layer.pending.take()
                        && let Err(payload) = handle.join()
                    {
                        std::panic::resume_unwind(payload);
                    }
                }
            }
        }
        layer.launch();
    }

    /// 층을 읽는 스레드가 도는가.
    /// 이 프로젝트의 줄을 지금 읽고 있거나 줄 서 있는가 — 머리줄이 그동안 도는 글리프를 세운다.
    pub(super) fn reading_place(&self, path: &Path) -> bool {
        self.layer.as_ref().is_some_and(|l| {
            l.reading.as_ref().is_some_and(|(p, ..)| p == path) || l.wanted.iter().any(|p| p == path)
        })
    }

    pub(super) fn layer_loading(&self) -> bool {
        // 펼친 프로젝트의 줄을 읽는 것도 읽는 중이다(moai-12yx) — 루프가 그동안 빠른 걸음으로
        // 깨어 머리줄의 도는 글리프를 돌리고, 시험의 `settle_reads` 도 이것으로 기다린다.
        self.layer.as_ref().is_some_and(|l| l.pending.is_some() || l.reading.is_some() || !l.wanted.is_empty())
    }
}

/// 층의 줄을 얼마나 깊이 열 것인가([`App::open_place`], moai-m59y).
///
/// 가르는 것은 **줄을 누가 읽는가** 다. 사람이 누른 한 번(Enter·`n`)은 그 자리에서 스냅샷까지
/// 읽어 보고, 펼치기는 줄을 스레드가 읽으므로 여는 데까지만 본다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Depth {
    /// 스냅샷까지 읽어 본다 — 못 읽는 줄을 그 자리에서 [`Look::Shut`] 으로 세운다.
    ///
    /// **읽는 사람이 한참 뒤에 있는 자리만 이것을 쓴다**(moai-lmot) — 지금은 `n`(담기) 하나다.
    /// 거기서는 못 읽는 저장소가 폼을 열고, 사람이 다 적고 Ctrl-S 를 눌러서야 못 담는다고
    /// 듣는다(적은 것이 갈 데가 없다). 뒤이어 곧 읽는 자리는 그 읽기가 제 까닭을 대므로
    /// [`Depth::Lean`] 이다.
    Whole,
    /// 여는 데까지만([`projects::open_shallow`]) — 줄은 곧 **누군가 읽는다**. 스레드가 읽든
    /// (`read_wanted`) 그 자리에서 읽든(`enter_project`) 여기서 스냅샷을 또 파면 같은 파일을
    /// 두 번 파는 것이고, 첫 판의 `load` 는 그대로 버려진다.
    ///
    /// **moai-uxrn 의 138→458ms 를 여기에 대지 않는다**(리뷰) — 그것은 옆 워크트리를 겹쳐 읽는
    /// 값인데, 여기서 걷어낸 `open_one` 은 `worktree: false` 로 불러 `worktree::gather` 가
    /// `repo.read()` 뒤에 바로 돌아섰다. 옮긴 것은 파싱 한 판이고, 그 값은 안 쟀다.
    Lean,
}

/// 탈이 난 설정을 **들고 설 때** 대는 까닭(moai-po6v) — 배너가 이것을 한 줄로 줄여 낸다
/// (`draw::banner`). **서는 자리는 [`App::held`] 다**(moai-23pm) — `Layer::problems` 에 실으면
/// 배너가 `on_layer()` 로 막아 프로젝트 안에서는 아무 말이 없다.
///
/// **들고 있는 것이 없으면 "들고 있다" 고 하지 않는다**(리뷰) — 까닭만 그대로 낸다. 등록한 줄 하나
/// 없는 층에 대고 그렇게 말하면, 아무것도 안 든 화면에 든 것이 있다고 말하는 꼴이다. 층은 등록한
/// 줄이 없어도 선다(`.moai` 밖에서 띄운 화면이 그렇다, `App::on_projects`) — `held` 가 그것을 가른다.
///
/// 사라진 파일은 `problems` 가 빈다 — 읽기에게는 댈 까닭이 아니라서다(아직 아무것도 등록 안 한
/// 사람의 정상). 여기서는 다르다: 들고 있는 것이 있으면 **있던 파일이 사라졌다**는 뜻이라 말해야 한다.
/// **안 들었으면 그대로 `None` 이다**(리뷰) — 없던 것을 잃었다고 말하는 꼴이라, [`unlayered_of`] 가
/// 빈 `problems` 에 `None` 을 내는 것과 한 답이다.
fn holding(fresh: &Layer, held: bool) -> Option<String> {
    let said = fresh.problems.iter().map(|p| crate::text::one_line(p)).collect::<Vec<_>>();
    if !held && said.is_empty() {
        return None;
    }
    let why = match (said.as_slice(), &fresh.config) {
        ([], Some(p)) => format!("{}: 파일이 사라졌다", p.display()),
        ([], None) => "파일이 사라졌다".to_string(),
        _ => said.join(" · "),
    };
    Some(match held {
        true => format!("사용자 설정을 못 읽어 지난 것을 들고 있다 — {why}"),
        false => why,
    })
}

/// 등록한 것이 하나도 안 읽혀 **층을 안 세운** 까닭([`App::unlayered`]) — 설정을 읽다 만난 것이
/// 있을 때만. 멀쩡한 빈 설정이면 `None` 이다. 띄울 때([`App::attach_layer`])와 설정이 바뀐 뒤
/// ([`App::relayer`])가 같은 말을 쓴다.
fn unlayered_of(layer: &Layer) -> Option<String> {
    (!layer.problems.is_empty()).then(|| {
        let why = layer.problems.iter().map(|p| crate::text::one_line(p)).collect::<Vec<_>>().join(" · ");
        format!("사용자 설정을 못 읽어 프로젝트 층을 안 세웠다 — {why}")
    })
}

/// 층에 선 동안 `App::cfg` 자리를 채우는 설정. **층에서는 아무도 읽지 않는다** — 칸 이름을
/// 묻는 것은 프로젝트 안의 줄뿐이고, 들어가면 그 프로젝트의 설정으로 갈아 끼운다.
fn blank_config() -> crate::config::Config {
    crate::config::Config::parse("prefix = \"moai\"\n").expect("고정된 설정 글이다")
}

/// 그림 시험이 디스크 없이 층을 세운다. 읽을 것이 없게 모든 줄을 방금 본 것으로 둔다 —
/// 없는 경로의 표식은 [`Marks::default`] 와 같고 읽은 때가 방금이라 [`Layer::stale`] 이 다시
/// 읽으러 가지 않는다.
#[cfg(test)]
pub(super) fn fake(places: Vec<(&str, &str, Look)>, at: At) -> Layer {
    Layer {
        // 기본값은 [`Layer::of`] 와 같다(moai-ra67).
        lang: crate::i18n::Lang::Ko,
        at,
        places: places
            .into_iter()
            .map(|(name, path, look)| Place {
                path: PathBuf::from(path),
                name: name.into(),
                hue: None,
                registered: true,
                launched: false,
                look,
                marks: Marks::default(),
                read_at: Some(std::time::Instant::now()),
                site: None,
            })
            .collect(),
        problems: Vec::new(),
        trouble: None,
        config: None,
        launch: None,
        pending: None,
        reading: None,
        wanted: Vec::new(),
    }
}

/// 가짜 층의 한 줄에 **그 프로젝트의 줄까지** 올린다(moai-m59y). [`fake`] 만으로는 머리줄뿐이라
/// (`site: None`) 한눈 보기의 목록에 남의 프로젝트의 줄이 한 번도 안 서고, 그 줄을 그리는 길
/// (`draw::row_line` 의 `Seat::Place`)을 그림 시험이 통째로 안 지난다 — 남의 줄이 제 프로젝트의
/// 칸과 색으로 서는지를 아무도 안 보는 자리가 거기 있었다.
#[cfg(test)]
pub(super) fn fill(layer: &mut Layer, at: usize, issues: Vec<crate::model::Issue>, cfg: crate::config::Config) {
    let (index, ground) = crate::tui::measure(&issues, &cfg);
    layer.places[at].site = Some(super::Site::of(issues, index, ground, cfg, crate::nav::Path::new(), Vec::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use crate::store::Opened;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path as NavPath;
    use crate::tui::{Mode, stamp_of};

    /// 진짜 디렉터리 여럿과 사용자 설정 한 벌. **돌리는 사람의 설정은 안 읽는다** — 층에
    /// 제 설정 파일을 준다(`Layer::config`).
    ///
    /// 이 묶음만 쓰는 손놀림. 자리를 만들고 지우는 일은 [`Scratch`] 가 한다.
    trait Places {
        fn project(&self, name: &str, lines: &[(&str, &str, &str)]) -> PathBuf;
        fn dir(&self, name: &str) -> PathBuf;
        fn register(&self, dirs: &[&Path]) -> PathBuf;
    }

    impl Places for Scratch {
        /// `.moai` 를 가진 프로젝트 하나. 줄은 `(id, 제목, 칸)`.
        fn project(&self, name: &str, lines: &[(&str, &str, &str)]) -> PathBuf {
            let dir = self.join(name);
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
            write_lines(&dir, lines);
            dir
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// 사용자 설정에 이 차례로 등록한다.
        fn register(&self, dirs: &[&Path]) -> PathBuf {
            let path = self.join("user/config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let body: String = dirs.iter().map(|d| format!("[[project]]\npath = {:?}\n", d.to_str().unwrap())).collect();
            std::fs::write(&path, body).unwrap();
            path
        }
    }

    fn write_lines(dir: &Path, lines: &[(&str, &str, &str)]) {
        let body: String = lines
            .iter()
            .map(|(id, title, st)| {
                let i = Issue::new((*id).into(), (*title).into(), Kind::Issue, Status::new(*st), "2026-09-01T00:00:00Z");
                format!("{}\n", serde_json::to_string(&i).unwrap())
            })
            .collect();
        std::fs::write(dir.join(".moai/issues.jsonl"), body).unwrap();
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    use super::super::settle_reads as settle;

    /// 밖에서 띄운 탐색기 — 첫 읽기가 **끝난 뒤**의 것. 읽기는 스레드로 간다([`Layer::launch`]).
    fn layered(cfg: &Path) -> App {
        let mut a = App::on_projects(Layer::read(Some(cfg), None));
        settle(&mut a);
        a
    }

    fn names(a: &App) -> Vec<String> {
        a.layer.as_ref().unwrap().places.iter().map(|p| p.name.clone()).collect()
    }

    fn look<'a>(a: &'a App, name: &str) -> &'a Look {
        &a.layer.as_ref().unwrap().places.iter().find(|p| p.name == name).unwrap().look
    }

    /// 보이는 줄의 제목 (`..` 은 뺀다). **줄마다 제 프로젝트에서 읽는다**([`App::issue_at`]) —
    /// 한눈 보기의 줄은 남의 목록의 첨자라, 지금 선 것으로 읽으면 엉뚱한 제목이 나온다.
    fn titles(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(seat, e, _) => e.at().and_then(|at| a.issue_at(*seat, at)).map(|i| i.title.clone()),
                _ => None,
            })
            .collect()
    }

    /// 같은 id 를 쓰는 두 프로젝트 — 둘의 제목이 달라 섞이면 곧바로 보인다.
    fn twins(s: &Scratch) -> (PathBuf, PathBuf) {
        let one = s.project("one", &[("argos-0001", "one 의 첫 줄", "todo"), ("argos-0002", "one 의 둘째 줄", "in_progress")]);
        let two = s.project("two", &[("argos-0002", "two 의 줄", "todo"), ("argos-0001", "two 의 집은 줄", "in_progress")]);
        (one, two)
    }

    /// **`.moai` 밖에서 띄우면 층에서 시작하고, 등록 차례대로 프로젝트마다 제 상태로 선다.**
    /// init 전·사라진 디렉터리·깨진 설정은 그 줄에서만 말하고 CLI 한눈 보기와 같은 말을 쓴다.
    #[test]
    fn outside_the_layer_lists_each_registered_project_in_its_own_state() {
        let s = Scratch::fenced("layer-list");
        let (one, two) = twins(&s);
        let bare = s.dir("bare");
        let gone = s.join("gone");
        let broken = s.dir("broken");
        std::fs::create_dir_all(broken.join(".moai")).unwrap();
        std::fs::write(broken.join(".moai/config.toml"), "statuses = \n").unwrap();
        let cfg = s.register(&[&one, &two, &bare, &gone, &broken]);

        // **첫 화면은 기다리지 않는다**(moai-ezwu) — 줄은 곧바로 서고 셈은 스레드가 채운다.
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        assert!(a.loading(), "밖에서 띄운 첫 화면이 그 자리에서 다 읽었다 — 등록한 수만큼 멈춘다");
        assert!(a.layer.as_ref().unwrap().places.iter().all(|p| matches!(p.look, Look::Unread)));
        assert_eq!(a.rows(), (0..5).map(Row::Project).collect::<Vec<_>>(), "읽기를 기다리느라 줄이 안 섰다");
        settle(&mut a);
        assert!(a.on_layer() && a.site.repo.is_none() && a.site.issues.is_empty());
        assert_eq!(names(&a), ["one", "two", "bare", "gone", "broken"]);

        let Look::Open { sum } = look(&a, "one") else { panic!("one 이 안 열렸다") };
        assert_eq!(sum.counts.iter().find(|(c, _)| c == "todo").unwrap().1, 1);
        assert_eq!(sum.picked.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["one 의 둘째 줄"]);
        let Look::Open { sum } = look(&a, "two") else { panic!("two 가 안 열렸다") };
        assert_eq!(sum.picked.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["two 의 집은 줄"], "같은 id 의 줄이 섞였다");

        for (name, state, word) in
            [("bare", Shut::Uninit, "init 전"), ("gone", Shut::Missing, "디렉터리가 없다"), ("broken", Shut::Unreadable, "못 읽는다")]
        {
            match look(&a, name) {
                Look::Shut { state: got, said } => {
                    assert_eq!(*got, state, "{name}");
                    assert!(said.contains(word), "{name}: {said}");
                    assert!(!said.contains('\u{1b}'), "색 이스케이프가 화면 글에 남았다 — {said:?}");
                }
                _ => panic!("{name} 이 열린 것으로 섰다"),
            }
        }
    }

    /// **들어가면 그 프로젝트의 줄만, 나오면 떠난 프로젝트에 선다.** 같은 id 가 두 프로젝트에
    /// 있어도 커서는 옛 프로젝트의 id 를 붙들고 넘어가지 않는다. 거름망은 나올 때 풀린다.
    #[test]
    fn entering_and_leaving_keeps_each_projects_lines_apart() {
        let s = Scratch::fenced("layer-enter");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);

        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer());
        assert_eq!(a.project().map(|p| p.path.clone()), Some(one.clone()));
        assert_eq!(a.site.repo.as_ref().map(|r| r.root.clone()), Some(one.clone()), "선 프로젝트와 쓸 저장소가 어긋났다");
        // 뿌리에 `..` 은 없다 — 층으로는 `0` 이 간다(moai-i784).
        assert!(!a.rows().contains(&Row::Up), "프로젝트 뿌리에 `..` 이 섰다");
        assert_eq!(titles(&a), ["one 의 첫 줄", "one 의 둘째 줄"]);

        // one 의 argos-0002 에 서고 거름망을 건다 — two 에서 argos-0002 는 다른 자리의 다른 줄이다.
        a.key(key(KeyCode::End));
        assert!(matches!(a.current(), Some(Row::Item(_, crate::nav::Entry::Leaf { at: 1 }, _))), "{:?}", a.current());
        a.hit("SPC f");
        for c in "status=in_progress".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filter_text.as_deref(), Some("status=in_progress"));

        a.key(key(KeyCode::Home));
        a.hit("0");
        assert!(a.on_layer() && a.site.repo.is_none() && a.site.issues.is_empty(), "층에 올라왔는데 프로젝트의 줄이 남았다");
        assert_eq!(a.current(), Some(Row::Project(0)), "떠난 프로젝트에 안 섰다");
        assert_eq!(a.filter_text, None, "한 프로젝트에 건 거름망이 층까지 따라왔다");

        // **떠난 프로젝트는 펼쳐진 채로 선다**(moai-i0wd) — 그 밑에 그 줄이 서므로 다음 머리줄은
        // 한 칸 아래가 아니다. 아직 안 읽은 둘째 프로젝트는 머리줄만이라 맨 아랫줄이 그것이다.
        a.key(key(KeyCode::End));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.repo.as_ref().map(|r| r.root.clone()), Some(two.clone()));
        assert_eq!(titles(&a), ["two 의 집은 줄", "two 의 줄"], "옛 프로젝트의 줄이 섞였다");
        // **들어가면 첫 줄에 선다.** 뿌리에 `..` 이 없어진 뒤로는(moai-i784) 그것이 곧 첫 이슈다 —
        // 한때 `..` 에 세웠다가 들어가자마자 누른 Enter 가 층으로 되올라가 Enter 두 번이 제자리인
        // 것을 고쳤는데(moai-cm13), 이제 그 줄 자체가 없어 같은 일이 생길 자리가 아예 없다.
        // 옛 커서의 id(one 의 argos-0002, 둘째 줄)를 따라가지 않는 것은 그대로 잰다.
        assert_eq!(a.cursor, 0, "들어가서 첫 줄에 안 섰거나 옛 커서의 id 를 따라갔다");

        // 뿌리의 ← 는 이제 아무 일도 안 한다 — 층으로는 `0` 이 간다.
        a.key(key(KeyCode::Left));
        assert!(!a.on_layer(), "뿌리의 ← 가 층으로 올라갔다");
        a.hit("0");
        assert_eq!(a.current(), Some(Row::Project(1)));
    }

    /// **겹쳐 보기를 끈 것은 한 프로젝트에 매인다** — 올라오면 거름망처럼 풀려 기본값(켬)으로
    /// 돌아간다(moai-zcuh).
    ///
    /// 끈 깃발을 들고 올라가면 층에서는 `w` 가 안 먹어 되켤 길이 없는 채로, 다음 프로젝트가
    /// 시키지도 않은 끈 화면으로 읽힌다.
    #[test]
    fn climbing_restores_the_worktree_overlay_like_it_drops_the_filter() {
        let s = Scratch::fenced("layer-climb-w");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);

        a.key(key(KeyCode::Enter));
        assert!(a.worktree, "프로젝트에 들어갔는데 겹쳐 보기가 꺼져 있다");
        a.hit("SPC v w Esc");
        assert!(!a.worktree, "프로젝트 안에서 w 가 안 껐다");
        // 보기는 사람의 설정이라 **따라간다**(moai-2bzp) — 겹쳐 보기와 반대다.
        a.hit("SPC v d Esc");
        a.hit("SPC s t Esc");
        let (view, order) = (a.view.clone(), a.order);
        assert!(!view.hides(crate::config::DONE), "프로젝트 안에서 SPC v d 가 done 을 안 보였다");

        a.key(key(KeyCode::Home));
        a.hit("0");
        assert!(a.on_layer());
        assert!(a.worktree, "층에 올라왔는데 끈 것이 따라왔다 — 되켤 키가 여기 없다");

        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert!(a.worktree, "다음 프로젝트가 시키지 않은 끈 화면으로 읽혔다");
        assert_eq!((a.view.clone(), a.order), (view, order), "보기·정렬이 층을 오가며 처음으로 돌아갔다");
    }

    /// **층에서 띄운 읽기는 방금 잰 줄을 덮지 않는다**(moai-800o). 들어가려다 못 열면 그 줄을
    /// 지금의 디렉터리로 고쳐 세우는데(`Look::Shut`), 그 앞에 띄운 층 읽기는 옛 디렉터리를 잰
    /// 것이다 — 늦게 닿으면 열린 줄로 되돌려, 사람이 방금 들은 "디렉터리가 없다" 와 화면이 어긋난다.
    #[test]
    fn a_layer_read_started_before_entering_does_not_undo_the_shut_it_wrote() {
        let s = Scratch::fenced("layer-late-shut");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        assert!(matches!(look(&a, "two"), Look::Open { .. }));

        // 층이 two 를 읽어 둔 채 아직 들이지 않았다. 그새 two 가 사라진다.
        let (tx, rx) = std::sync::mpsc::channel();
        look_into(std::slice::from_ref(&two), &crate::model::now(), crate::i18n::Lang::Ko, &tx);
        a.layer.as_mut().unwrap().pending = Some((rx, std::thread::spawn(|| {})));
        std::fs::remove_dir_all(&two).unwrap();

        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert!(a.on_layer(), "사라진 프로젝트에 들어갔다");
        assert!(matches!(look(&a, "two"), Look::Shut { state: Shut::Missing, .. }));
        a.follow();
        assert!(
            matches!(look(&a, "two"), Look::Shut { state: Shut::Missing, .. }),
            "들어가기 전에 띄운 층 읽기가 방금 쓴 '없다' 를 열린 줄로 덮었다"
        );
    }

    /// **들어가면 층의 읽기를 놓는다**(moai-800o). 안 놓으면 프로젝트 안에서도 `App::loading`
    /// 이 참이라 루프가 빠른 걸음으로 깨어, 이 프로젝트와 상관없는 층 읽기를 기다린다.
    #[test]
    fn entering_a_project_lets_go_of_the_layer_read() {
        let s = Scratch::fenced("layer-enter-pending");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);

        let (hold, wait) = std::sync::mpsc::channel::<()>();
        let (_tx, rx) = std::sync::mpsc::channel();
        a.layer.as_mut().unwrap().pending = Some((rx, std::thread::spawn(move || {
            let _ = wait.recv();
        })));

        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer());
        assert!(!a.layer_loading(), "층에서 띄운 읽기가 프로젝트 안까지 따라왔다");
        drop(hold);
    }

    /// **헤더 번호로 옆 프로젝트에 건너가면 떠난 프로젝트의 커밋 표를 버린다**(moai-800o) — 층을
    /// 거쳐 가는 길(`App::climb`)과 같은 것을 되돌린다. 짓던 표가 늦게 닿으면 떠난 뿌리의 표가
    /// 새 프로젝트의 표에 섞인다.
    #[test]
    fn jumping_to_a_sibling_project_drops_the_commit_table_of_the_one_it_left() {
        let s = Scratch::fenced("layer-jump-commits");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.key(key(KeyCode::Enter));
        assert_eq!(a.here(), Some(one.clone()));

        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(super::super::Commits::from([(one.clone(), Default::default())])).unwrap();
        a.commits_job = Some((rx, std::thread::spawn(|| {})));

        a.hit("2");
        assert_eq!(a.here(), Some(two.clone()));
        a.follow();
        assert!(!a.site.commits.contains_key(&one), "떠난 프로젝트의 커밋 표가 새 프로젝트로 넘어왔다");
    }

    /// **설정이 깨져 층이 안 서도 까닭은 댄다.** 등록한 것이 하나도 안 읽히면 층은 없고
    /// (프로젝트 안 화면은 예전 그대로), 배너가 설정의 문제를 한 줄로 댄다. 멀쩡한 설정이면
    /// 그 줄이 없다.
    #[test]
    fn a_broken_config_leaves_no_layer_but_says_why() {
        let s = Scratch::fenced("layer-broken-cfg");
        let here = s.project("here", &[("argos-0009", "여기 줄", "todo")]);
        let cfg = s.join("broken.toml");
        std::fs::write(&cfg, "[[project]]\npath = \"/x\"\n[[project\n").unwrap();
        let open = || {
            let repo = Repo::at(here.clone(), crate::config::Config::parse("prefix = \"argos\"\n").unwrap());
            let stamp = stamp_of(&repo);
            let load = repo.read().unwrap();
            let (index, ground) = crate::tui::measure(&load.issues, &repo.config);
            App::open(repo, load, index, ground, NavPath::new(), stamp)
        };
        let mut a = open().attach_layer(Layer::read(Some(&cfg), Some(&here)));
        assert!(a.layer.is_none(), "깨진 설정으로 층을 세웠다");
        let why = a.unlayered.clone().expect("층이 없는 까닭을 안 들었다");
        assert!(why.contains("TOML") && !why.contains('\n'), "{why:?}");
        let banner = super::super::draw::tests_banner(&mut a);
        assert!(banner.contains("프로젝트 층을 안 세웠다"), "배너가 까닭을 안 댔다 — {banner:?}");

        // **떠 있는 동안 설정을 고치면 까닭이 걷힌다**(moai-en4u) — 걸음이 설정을 다시 읽는다. 층이 안
        // 서도(등록 0) 옛 "못 읽어" 가 붙박이로 남지 않는다. 다시 깨지면 다시 선다.
        a.user_config = Some(cfg.clone());
        a.follow();
        std::fs::write(&cfg, "").unwrap();
        a.follow();
        assert!(a.layer.is_none() && a.unlayered.is_none(), "고친 설정에 옛 까닭이 남았다 — {:?}", a.unlayered);
        std::fs::write(&cfg, "[[project\n").unwrap();
        a.follow();
        assert!(a.unlayered.as_deref().is_some_and(|u| u.contains("TOML")), "다시 깨진 설정의 까닭을 안 달았다 — {:?}", a.unlayered);

        std::fs::write(&cfg, "").unwrap();
        let a = open().attach_layer(Layer::read(Some(&cfg), Some(&here)));
        assert!(a.layer.is_none() && a.unlayered.is_none(), "멀쩡한 빈 설정에 까닭을 달았다");
    }

    /// **`.moai` 안에서 띄우면 그 안에서 시작하고, `0` 으로 층에 올라가 띄운 자리에 선다**
    /// (결정 3, 올라가는 키는 moai-i784 에서 Bksp 에서 `0` 으로 옮겼다). 등록 안 된 자리는 층
    /// 맨 앞에 서서 도로 내려갈 수 있다. 남의 프로젝트는 올라갈 때 처음 읽는다.
    #[test]
    fn launched_inside_it_starts_inside_and_climbs_to_where_it_was_launched() {
        let s = Scratch::fenced("layer-inside");
        let (one, two) = twins(&s);
        let here = s.project("here", &[("argos-0009", "여기 줄", "todo")]);
        let cfg = s.register(&[&one, &two]);

        let repo = Repo::at(here.clone(), crate::config::Config::parse("prefix = \"argos\"\n").unwrap());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = crate::tui::measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, NavPath::new(), stamp).with_layer(Layer::read(Some(&cfg), Some(&here)));
        assert!(!a.on_layer());
        assert_eq!(titles(&a), ["여기 줄"]);
        assert_eq!(a.cursor, 0, "뿌리에 `..` 이 없는데 커서가 한 칸 내려가 섰다");
        assert!(matches!(look(&a, "one"), Look::Unread), "안에서 띄웠는데 남의 프로젝트를 먼저 읽었다");

        a.hit("0");
        assert!(a.on_layer());
        assert_eq!(names(&a), ["here", "one", "two"]);
        let at = a.layer.as_ref().unwrap();
        assert!(at.places[0].launched && !at.places[0].registered);
        assert_eq!(a.current(), Some(Row::Project(0)), "띄운 자리에 안 섰다");
        assert!(a.loading(), "올라갔는데 남의 프로젝트를 읽으러 안 갔다");
        settle(&mut a);
        assert!(matches!(look(&a, "two"), Look::Open { .. }), "올라갔는데 남의 프로젝트를 안 읽었다");

        a.key(key(KeyCode::Enter));
        assert_eq!(titles(&a), ["여기 줄"], "띄운 자리로 도로 못 내려간다");

        // 등록된 자리에서 띄우면 따로 서지 않고 그 줄에 표시만 붙는다.
        let layer = Layer::read(Some(&cfg), Some(&two));
        assert_eq!(layer.places.iter().map(|p| (p.launched, p.registered)).collect::<Vec<_>>(), [(false, true), (true, true)]);
        assert_eq!(layer.at, At::Project(two.clone()));
    }

    /// **등록한 것이 없으면 층을 세우지 않는다** — 띄운 자리 하나뿐인 층은 오늘 화면에 `..`
    /// 하나를 더할 뿐이다. 사라진 디렉터리라도 등록돼 있으면 선다.
    #[test]
    fn without_a_registration_there_is_no_layer() {
        let s = Scratch::fenced("layer-none");
        let here = s.project("here", &[]);
        let cfg = s.register(&[]);
        assert!(!Layer::read(Some(&cfg), Some(&here)).registered());
        assert!(!Layer::read(None, Some(&here)).registered(), "설정 자리를 몰라도 층이 섰다");
        let cfg = s.register(&[&s.join("gone")]);
        assert!(Layer::read(Some(&cfg), Some(&here)).registered());
    }

    /// 사용자 설정에 정한 색이 층의 줄까지 실려 온다(moai-o04b) — `draw::project_style` 이 그것을
    /// 입힌다. 띄운 자리로만 선 줄은 설정에 없으니 정한 색도 없다.
    #[test]
    fn a_colour_chosen_in_the_user_config_rides_on_the_place() {
        let s = Scratch::fenced("layer-hue");
        let (one, here) = (s.dir("one"), s.dir("here"));
        let cfg = s.register(&[&one]);
        std::fs::write(&cfg, format!("{}color = \"blue\"\n", std::fs::read_to_string(&cfg).unwrap())).unwrap();
        let layer = Layer::read(Some(&cfg), Some(&here));
        let hues: Vec<_> = layer.places.iter().map(|p| (p.name.as_str(), p.hue.map(crate::style::Hue::name))).collect();
        assert_eq!(hues, [("here", None), ("one", Some("blue"))]);
    }

    /// 열 수 없는 프로젝트에 들어가려 하면 **층에 선 채 까닭만 말한다.** 넘어지지도, 빈
    /// 화면에 들어가지도 않는다. 그새 init 했으면 들어간다 — 층의 셈이 아니라 지금의
    /// 디렉터리를 연다.
    #[test]
    fn entering_a_project_that_cannot_open_only_says_why() {
        let s = Scratch::fenced("layer-shut");
        let bare = s.dir("bare");
        let gone = s.join("gone");
        let cfg = s.register(&[&bare, &gone]);
        let mut a = layered(&cfg);

        a.key(key(KeyCode::Enter));
        assert!(a.on_layer() && a.site.repo.is_none());
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("init 전")), "{:?}", a.notice);
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert!(a.on_layer());
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("디렉터리가 없다")), "{:?}", a.notice);

        std::fs::create_dir_all(bare.join(".moai")).unwrap();
        std::fs::write(bare.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        a.key(key(KeyCode::Up));
        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer(), "init 한 뒤에도 층의 옛 셈을 보고 안 들어갔다");
        assert_eq!(a.site.repo.as_ref().map(|r| r.root.clone()), Some(bare));
    }

    /// **층에 선 동안 바뀐 프로젝트만 스레드에서 다시 읽는다.** 안에 들어가 있는 동안에는
    /// 남의 프로젝트를 재지도 읽지도 않는다.
    #[test]
    fn only_the_project_that_changed_is_reread_and_only_on_the_layer() {
        let s = Scratch::fenced("layer-reread");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        // **읽은 때로 잰다** — 표식은 다시 읽어도 같은 값이라 "다시 읽었나" 를 못 가른다(리뷰 moai-3lul.kt0).
        let before = a.layer.as_ref().unwrap().places[0].read_at;

        a.follow();
        assert!(!a.loading(), "아무것도 안 바뀌었는데 읽으러 갔다");

        write_lines(&two, &[("argos-0001", "two 의 집은 줄", "done")]);
        assert_eq!(a.layer.as_ref().unwrap().stale(), std::slice::from_ref(&two));
        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 안 읽었다");
        settle(&mut a);
        let Look::Open { sum } = look(&a, "two") else { panic!() };
        assert!(sum.picked.is_empty(), "다시 읽은 셈이 안 들어왔다");
        assert_eq!(a.layer.as_ref().unwrap().places[0].read_at, before, "안 바뀐 프로젝트까지 다시 읽었다");

        // 안에 들어가면 층의 표식은 안 본다.
        a.key(key(KeyCode::Enter));
        write_lines(&two, &[("argos-0001", "two 의 집은 줄", "in_progress")]);
        a.follow();
        assert!(!a.loading(), "안에 있는 동안 남의 프로젝트를 읽으러 갔다");
        a.hit("0");
        // 올라오는 키는 읽기를 기다리지 않는다(moai-ezwu) — 그동안 옛 셈이 선다.
        assert!(a.loading(), "올라갈 때 바뀐 것을 읽으러 안 갔다");
        let Look::Open { sum } = look(&a, "two") else { panic!() };
        assert!(sum.picked.is_empty(), "올라오는 키가 읽기를 기다렸다");
        settle(&mut a);
        let Look::Open { sum } = look(&a, "two") else { panic!() };
        assert_eq!(sum.picked.len(), 1, "올라갈 때 바뀐 것을 안 읽었다");
    }

    /// **워크트리를 치우면 층이 따라간다**(moai-al0x). 자리 판정은 `.moai` 가 그대로여도 워크트리
    /// 하나로 답이 바뀐다 — 한때 층은 두 파일만 재어 손으로 다시 읽기 전까지 "자리 없는 것" 을 안 댔다.
    #[test]
    fn removing_the_worktree_that_held_a_picked_line_shows_on_the_layer() {
        let s = Scratch::fenced("layer-worktree-gone");
        let main = s.project("main", &[("argos-0002", "집은 줄", "in_progress")]);
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../argos-0002", "-b", "worktree-argos-0002"]);
        // 딸린 워크트리가 하나도 없으면 판정은 조용하다(워크트리 규약을 안 쓰는 저장소) — 하나는 남긴다.
        run(&main, &["worktree", "add", "-q", "../other", "-b", "other"]);
        let cfg = s.register(&[&main]);
        let mut a = layered(&cfg);
        let stranded = |a: &App| match look(a, "main") {
            Look::Open { sum } => sum.stranded,
            _ => panic!("main 이 안 열렸다"),
        };
        assert_eq!(stranded(&a), 0, "이름이 쥔 워크트리가 있는데 자리 없다고 댄다");

        // **들어가면 층과 같은 수다** — 안쪽 배너도 자리 없는 줄을 센다(사용자 결정 2026-09-18).
        let Look::Open { sum } = look(&a, "main") else { panic!() };
        let on_layer = sum.warnings;
        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer());
        assert_eq!(a.site.warnings, on_layer, "층과 안쪽 배너가 같은 저장소를 달리 센다");

        // **겹쳐 보기를 꺼도 워크트리가 사라지는 것을 본다**(리뷰 moai-3lul.kt0). 끄면 옆 스냅샷을
        // 아예 안 열어, 한때는 자리 판정이 보는 것이 지켜보는 표식에 하나도 안 들었다 — 치운 뒤
        // 배너가 옛 수로 굳었다.
        a.hit("SPC v w Esc");
        assert!(!a.worktree, "w 가 겹쳐 보기를 안 껐다");
        settle(&mut a);
        let was = a.site.warnings;
        std::fs::remove_dir_all(s.join("argos-0002")).unwrap();
        settle(&mut a);
        assert_eq!(a.site.warnings, was + 1, "겹쳐 보기를 끈 채로는 치운 워크트리를 못 본다");

        // 올라오면 층도 같은 것을 센다.
        a.hit("0");
        settle(&mut a);
        assert_eq!(stranded(&a), 1, "워크트리를 치웠는데 층이 옛 수를 낸다");
        let Look::Open { sum } = look(&a, "main") else { panic!() };
        assert_eq!(sum.warnings, was + 1, "층과 안쪽 배너가 갈렸다");
    }

    /// **파일이 그대로여도 시계가 가면 다시 읽는다**(moai-al0x). 요약에는 시계로 재는 것(워크트리가
    /// 뜰 한 시간 틈, 날로 재는 경고)이 들어, 표식만 보면 옛 수가 선 채 남는다. 층에 선 동안만이다.
    #[test]
    fn a_layer_line_read_long_ago_is_reread_even_if_nothing_changed() {
        let s = Scratch::fenced("layer-clock");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.follow();
        assert!(!a.loading(), "방금 읽은 것을 또 읽으러 갔다");

        let long_ago = std::time::Instant::now().checked_sub(REREAD_EVERY).expect("시계가 1분도 안 돌았다");
        a.layer.as_mut().unwrap().places[1].read_at = Some(long_ago);
        assert_eq!(a.layer.as_ref().unwrap().stale(), std::slice::from_ref(&two), "오래된 줄만 골라야 한다");
        settle(&mut a);
        let read_at = a.layer.as_ref().unwrap().places[1].read_at.unwrap();
        assert!(read_at.elapsed() < REREAD_EVERY, "다시 읽고도 읽은 때를 안 올렸다");

        // 안에 들어가 있는 동안은 시계가 가도 남의 줄을 안 읽는다.
        a.key(key(KeyCode::Enter));
        a.layer.as_mut().unwrap().places[1].read_at = Some(long_ago);
        a.follow();
        assert!(!a.loading(), "안에 있는 동안 시계를 보고 남의 프로젝트를 읽으러 갔다");

        // **안쪽은 제 프로젝트를 같은 자로 다시 읽는다**(moai-z4r4) — 배너의 수에도 시계로 재는 것이
        // 들어, 안 읽으면 틈을 넘긴 순간 층의 `!` 와 갈린다.
        a.site.read_at = Some(long_ago);
        a.follow();
        assert!(a.loading(), "읽은 지 1분이 넘었는데 제 프로젝트를 다시 안 읽었다");
        settle(&mut a);
        assert!(a.site.read_at.is_some_and(|t| t.elapsed() < REREAD_EVERY), "다시 읽고도 읽은 때를 안 올렸다");
        a.follow();
        assert!(!a.loading(), "방금 읽은 프로젝트를 또 읽으러 갔다");
    }

    /// **`.moai` 없는 디렉터리가 사라지거나 다시 생기는 것도 본다.** 두 파일의 표식은 그
    /// 동안 둘 다 없음 그대로라, 디렉터리를 안 재면 층이 "init 전" 을 영영 댄다.
    #[test]
    fn a_bare_directory_that_disappears_is_reread() {
        let s = Scratch::fenced("layer-vanish");
        let bare = s.dir("bare");
        let cfg = s.register(&[&bare]);
        let mut a = layered(&cfg);
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Uninit, .. }));

        std::fs::remove_dir_all(&bare).unwrap();
        settle(&mut a);
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Missing, .. }), "사라진 디렉터리를 init 전으로 둔다");

        std::fs::create_dir_all(&bare).unwrap();
        settle(&mut a);
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Uninit, .. }), "다시 생긴 디렉터리를 없다로 둔다");
    }

    /// **층에서 프로젝트 안의 줄에 매인 키는 아무것도 안 한다** — 폼·칸을 안 열고 왜 안 되는지를
    /// 한 줄로 말한다. `n` 은 여기 없다(커서의 프로젝트에 담는다 — 아래 시험들).
    #[test]
    fn keys_that_need_a_project_say_so_on_the_layer_and_touch_nothing() {
        let s = Scratch::fenced("layer-refuse");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let files = || [&one, &two].map(|d| std::fs::read(d.join(".moai/issues.jsonl")).unwrap());
        let was = files();
        let mut a = layered(&cfg);
        a.user = Some("레이븐 (raven@example.com)".into());

        // 바로 누르는 `/` 는 까닭을 댄다.
        a.key(key(KeyCode::Char('/')));
        assert_eq!(a.mode, Mode::Browse, "/ 가 층에서 칸을 열었다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("거름망")), "{:?}", a.notice);
        // 메뉴의 거름망·워크트리는 층의 메뉴에 안 선다 — 눌러도 모르는 키라 메뉴만 떠 있다(moai-7sjm).
        for path in ["SPC f", "SPC v w"] {
            a.hit(path);
            assert_eq!(a.mode, Mode::Browse, "{path} 가 층에서 칸을 열었다");
            assert!(super::super::menu::open(&a.chord), "{path}: 안 선 키가 메뉴를 닫았다");
            a.key(key(KeyCode::Esc));
            assert!(!super::super::menu::open(&a.chord));
        }
        assert!(a.worktree, "층에서 SPC v w 가 겹쳐 보기를 건드렸다");

        // **거들쇠가 붙어도 새지 않는다.** `refused` 는 Ctrl·Alt 를 그냥 넘기므로, 키를
        // 나누는 쪽이 안 거르면 Ctrl-A 가 등록 창을, Ctrl-D 가 "목록에서 뺄까" 를 띄운다 —
        // 터미널에서 줄 맨 앞·EOF 로 손에 익은 키라 누를 일이 실제로 있다.
        for c in ['a', 'd', 'f', 'w', 'n'] {
            for m in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
                a.key(KeyEvent::new(KeyCode::Char(c), m));
                assert_eq!(a.mode, Mode::Browse, "{m:?}-{c} 가 칸을 열었다");
            }
        }
        assert!(a.worktree, "Ctrl-w 가 겹쳐 보기를 건드렸다");
        // **걷은 F7 은 수식키가 붙든 말든 아무 일도 없다**(moai-7sjm) — 옛 `refused` 는 수식키 붙은
        // 키를 통째로 넘겨 `F(7)` 이 층에서 거름망 칸을 열었다(moai-nc7w).
        for m in [KeyModifiers::NONE, KeyModifiers::CONTROL, KeyModifiers::ALT] {
            a.key(KeyEvent::new(KeyCode::F(7), m));
            assert_eq!((&a.mode, &a.notice), (&Mode::Browse, &None), "{m:?}-F7 가 뜻을 했다");
        }
        assert_eq!(files(), was, "층에서 누른 키가 파일을 바꿨다");
        assert!(!one.join(".moai/journal.jsonl").exists() && !two.join(".moai/journal.jsonl").exists());

        // 들어가면 `n` 은 오늘처럼 폼을 연다.
        a.key(key(KeyCode::Enter));
        a.hit("SPC n");
        assert!(matches!(a.mode, Mode::Idea(_)));
    }

    /// 두 프로젝트의 `issues.jsonl` 바이트.
    fn snapshots(dirs: &[&Path]) -> Vec<Vec<u8>> {
        dirs.iter().map(|d| std::fs::read(d.join(".moai/issues.jsonl")).unwrap()).collect()
    }

    /// 그 프로젝트 파일에 선 생각들의 제목 — 화면이 아니라 **파일을** 읽는다.
    fn ideas_at(dir: &Path) -> Vec<String> {
        let repo = match Repo::open(dir).unwrap() {
            Opened::Repo(r) => r,
            _ => panic!("{} 가 안 열린다", dir.display()),
        };
        repo.read().unwrap().issues.into_iter().filter(|i| i.kind == Kind::Idea).map(|i| i.title).collect()
    }

    fn type_in(a: &mut App, text: &str) {
        for c in text.chars() {
            a.key(key(KeyCode::Char(c)));
        }
    }

    fn target(a: &App) -> Option<PathBuf> {
        match &a.mode {
            Mode::Idea(f) => f.into.as_ref().map(|t| t.path.clone()),
            Mode::Ask(_) | Mode::Browse | Mode::Grep(..) | Mode::Filter(_) | Mode::Pick(_) | Mode::Unregister(_) => None,
        }
    }

    fn on_layer_with_twins(s: &Scratch) -> (PathBuf, PathBuf, App) {
        let (one, two) = twins(s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user = Some("레이븐 (raven@example.com)".into());
        (one, two, a)
    }

    /// **들어갈 때 그 줄이 들고 있던 읽음을 옮겨 든다**(moai-2gep). [`App::leave_project`] 가 `Site` 를
    /// 비우고 [`App::load_read`] 가 파일에서 다시 드는데, 그 파일을 못 읽으면(옛 `sudo moai read` 가
    /// 남긴 root 의 파일) 들일 것이 없어 **내게 온 줄이 모두** [NEW] 로 선다 — 그 화면의 `SPC m a`
    /// 한 번이 그것을 다 찍는다. 층의 줄은 펼칠 때 읽어 둔 표를 들고 있으니, 버리지 않고 옮겨 든다.
    /// 세워 둔 줄에서 옮겨 드는 [`App::follow_site`] 와 같은 자다.
    #[test]
    #[cfg(unix)]
    fn entering_a_project_carries_the_read_marks_the_row_already_held() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::fenced("layer-enter-read");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        a.user = Some("레이븐 (raven@example.com)".into());
        let seen: std::collections::BTreeMap<String, String> =
            [("argos-0001".to_string(), "2026-09-14T00:00:00Z".to_string())].into_iter().collect();
        crate::read_marks::update(&cfg, &one, |sh| sh.mark(&seen)).unwrap();

        a.want_site(0);
        settle(&mut a);
        let held = a.layer.as_ref().unwrap().places[0].site.as_ref().expect("펼친 줄이 제 Site 를 든다");
        assert_eq!(held.seen.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "시험의 전제 — 줄이 읽음을 들었다");

        // 들어가는 길에서 다시 읽으면 빈 표다. 들고 있던 것을 버리면 그 프로젝트가 통째로 [NEW] 다.
        let at = crate::read_marks::path_for(&cfg, &one);
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&at).is_ok() {
            // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다.
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
            return;
        }
        a.enter_project(0);
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!a.on_layer(), "시험의 전제 — 들어갔다");
        assert_eq!(
            a.site.seen.get("argos-0001").map(String::as_str),
            Some("2026-09-14T00:00:00Z"),
            "들어가며 들고 있던 읽음을 버렸다 — 이 프로젝트가 통째로 [NEW] 로 선다"
        );
    }

    /// **한눈 보기는 프로젝트마다 머리줄 하나와 그 밑의 줄을 한 목록으로 세운다**(moai-eyre, 사용자
    /// 결정 2026-09-19). 줄은 제 프로젝트를 [`super::Seat`] 으로 들고 다녀, 같은 id 를 쓰는 두
    /// 프로젝트가 한 트리에서 안 섞인다. 아직 안 읽은 프로젝트는 머리줄만 서고(읽는 것은 펼칠
    /// 때다 — moai-12yx), 접은 프로젝트는 그 밑이 빠진다 — 다 접으면 옛 프로젝트 층과 같은 화면이다.
    #[test]
    fn the_one_list_stacks_each_project_under_its_head() {
        let s = Scratch::fenced("layer-one-list");
        let (one, _two, mut a) = on_layer_with_twins(&s);
        assert!(a.on_layer(), "시험의 전제 — 층에서 시작한다");
        assert_eq!(a.rows().len(), 2, "안 읽은 프로젝트가 줄을 세웠다");

        // 읽기는 스레드로 간다(moai-12yx) — `settle` 이 다 받을 때까지 걸음을 돌린다.
        a.want_site(0);
        settle(&mut a);
        assert!(a.rows().len() > 2, "첫 프로젝트를 못 읽었다");
        let rows = a.rows();
        assert!(matches!(rows[0], Row::Project(0)), "{:?}", rows[0]);
        assert!(matches!(rows[1], Row::Item(crate::tui::Seat::Place(0), ..)), "{:?}", rows[1]);
        assert!(matches!(rows.last(), Some(Row::Project(1))), "둘째 머리줄이 안 섰다: {rows:?}");
        let mut titles: Vec<String> = titles(&a);
        titles.sort();
        assert_eq!(titles, ["one 의 둘째 줄", "one 의 첫 줄"], "남의 프로젝트의 줄이 섞였다");

        // 접으면 그 밑이 빠진다 — 머리줄은 남는다.
        a.folded.insert(one);
        assert_eq!(a.rows().len(), 2, "접었는데 줄이 남았다");
    }

    /// **한눈 보기의 상세도 그 줄의 프로젝트에 묻는다**(리뷰). 막는 것은 그 프로젝트의 색인에서
    /// 푸는데 기다림(`Site::waits`)만 지금 선 프로젝트에 묻던 때는, 한눈 보기의 `App::site` 가
    /// 줄이 빈 자리 채우개라([`App::leave_project`]) 그 첨자가 넘쳐 **커서를 옮기는 것만으로**
    /// 탐색기가 죽었다. 제목(`title_of`)과 커밋(`commits_of`)도 같은 자리다.
    #[test]
    fn the_detail_of_a_foreign_row_reads_its_own_project() {
        let s = Scratch::fenced("layer-foreign-detail");
        let one = s.project("one", &[("argos-0001", "막는 줄", "todo")]);
        // 막음은 `write_lines` 가 안 적는다 — 이 프로젝트만 손으로 적는다.
        let open = Issue::new("argos-0001".into(), "막는 줄".into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z");
        let mut blocked =
            Issue::new("argos-0002".into(), "막힌 줄".into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z");
        blocked.blocked_by = vec!["argos-0001".into()];
        let body = format!("{}\n{}\n", serde_json::to_string(&open).unwrap(), serde_json::to_string(&blocked).unwrap());
        std::fs::write(one.join(".moai/issues.jsonl"), body).unwrap();
        let cfg = s.register(&[&one]);
        let mut a = layered(&cfg);
        a.want_site(0);
        settle(&mut a);

        let rows = a.rows();
        let at = rows
            .iter()
            .position(|r| {
                matches!(r, Row::Item(seat, e, _) if e.at().and_then(|at| a.issue_at(*seat, at)).is_some_and(|i| i.id == "argos-0002"))
            })
            .expect("막힌 줄이 한눈 보기에 안 섰다");
        a.cursor = at;
        // 그리는 것이 곧 시험이다 — 넘치면 여기서 죽는다.
        let shot = crate::tui::draw::tests::render(&mut a, 110, 20).join("\n");
        assert!(shot.contains("막는 줄"), "남의 줄의 막음을 제 프로젝트에서 못 읽었다:\n{shot}");
    }

    /// **층을 다시 세워도 읽어 둔 줄은 남는다**(리뷰). 이 길은 사용자 설정이 바뀔 때마다 도는데
    /// ([`App::follow_config`]) 보기 토글과 읽음이 그 파일을 **스스로 쓴다** — 두고 가면 `SPC v`
    /// 한 번이 그 토글을 걸 줄을 통째로 없애고, 다시 읽으러 가는 자도 없다([`App::want_site`] 는
    /// 펼칠 때만 부른다).
    #[test]
    fn rebuilding_the_layer_keeps_the_rows_it_already_read() {
        let s = Scratch::fenced("layer-relayer-rows");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.want_site(0);
        settle(&mut a);
        let was = a.rows().len();
        assert!(was > 2, "시험의 전제 — 첫 프로젝트를 펼쳐 읽었다");
        a.relayer(None);
        assert_eq!(a.rows().len(), was, "층을 다시 세우자 펼쳐 둔 프로젝트의 줄이 사라졌다");
    }

    /// **펼치며 읽은 줄에도 화면의 보기를 곧바로 건다**(리뷰, moai-1xo5 의 약속). `Site::of` 는
    /// `shown` 을 비워 두고 그것을 채우는 자는 보기 토글뿐이라([`App::see`]), 안 걸면 방금 펼친
    /// 프로젝트만 보기를 안 따라 같은 화면의 두 프로젝트가 한 토글에 다르게 선다.
    #[test]
    fn a_freshly_read_project_follows_the_view_that_is_already_on() {
        let s = Scratch::fenced("layer-view-on-read");
        let one = s.project("one", &[("argos-0001", "열린 줄", "todo"), ("argos-0002", "끝난 줄", "done")]);
        let cfg = s.register(&[&one]);
        let mut a = layered(&cfg);
        assert!(a.view.hides(crate::config::DONE), "시험의 전제 — 탐색기는 done 을 숨긴 채로 뜬다");
        a.want_site(0);
        settle(&mut a);
        assert_eq!(titles(&a), ["열린 줄"], "펼치며 읽은 프로젝트가 걸려 있던 보기를 안 따랐다");
    }

    /// **펼쳐 둔 자리는 프로젝트를 건너지 않는다**(리뷰) — 마디가 그 프로젝트의 이슈 id 이고,
    /// 바구니 마디(`(마일스톤 없음)`·`(길 잃음)`)는 id 조차 없어 prefix 가 달라도 그대로 샌다.
    /// 두고 오면 다음 프로젝트가 아무도 안 펼친 묶음을 펼친 채 세운다. `leave_project` 가 커서
    /// 정체·거름망·안 읽음을 푸는 것과 같은 자리다.
    #[test]
    fn folding_does_not_cross_projects() {
        let s = Scratch::fenced("layer-fold-leak");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.hit("1");
        // 이 fixture 에는 묶음이 없어 키로 펼칠 것이 없다 — 펼쳐 둔 자리를 손으로 심는다.
        // 바구니 마디는 id 를 안 들어, 두 프로젝트의 prefix 가 달라도 그대로 겹치는 자리다.
        a.site.expanded.insert(vec![crate::nav::Seg::Milestone(None)]);
        a.hit("2");
        assert!(a.site.expanded.is_empty(), "옆 프로젝트로 건너갔는데 펼침이 남았다: {:?}", a.site.expanded);

        a.site.expanded.insert(vec![crate::nav::Seg::Lost]);
        a.hit("0");
        assert!(a.site.expanded.is_empty(), "층으로 올라왔는데 펼침이 남았다: {:?}", a.site.expanded);
    }

    /// **뿌리의 Bksp 는 조용히 먹히지 않고 갈 키를 댄다**(리뷰 moai-lur8.met) — 여태 그 키가
    /// 층으로 올라갔으므로, 아무 말 없이 안 듣는 것은 고장으로 읽힌다.
    #[test]
    fn backspace_at_a_layered_root_names_the_key_that_replaced_it() {
        let s = Scratch::fenced("layer-bksp-says");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.hit("1");
        a.key(key(KeyCode::Backspace));
        let said = a.notice.clone().unwrap_or_default();
        assert!(said.contains('0') && said.contains("프로젝트 층"), "갈 키를 안 댄다 — {said:?}");
        assert!(!a.on_layer(), "말만 하고 올라가 버렸다");
    }

    /// **머리줄에서 `l`·`→` 는 그 프로젝트를 펼치고, `h`·`←` 가 접고, Enter 가 들어간다**
    /// (moai-i0wd, 사용자 결정 2026-09-19). 묶음 줄과 같은 손가락이다 — 한때 층에서 `l` 은
    /// 들어가기였는데(moai-9m2d), 그때 층은 트리가 아니었다.
    #[test]
    fn a_head_opens_with_l_and_enters_with_enter() {
        for k in ["l", "Right"] {
            let s = Scratch::fenced("layer-l-opens");
            let (_one, _two, mut a) = on_layer_with_twins(&s);
            assert!(a.on_layer(), "시험의 전제 — 층에 섰다");
            let heads = a.rows().len();
            a.hit(k);
            // 읽기는 스레드로 간다(moai-12yx) — 다 받은 뒤에 줄이 선다.
            settle(&mut a);
            assert!(a.on_layer(), "`{k}` 가 프로젝트로 들어가 버렸다");
            assert!(a.rows().len() > heads, "`{k}` 가 머리줄을 안 폈다");
            // 접으면 도로 머리줄만 — 읽은 것은 버리지 않는다.
            a.hit("h");
            assert_eq!(a.rows().len(), heads, "`h` 가 머리줄을 안 접었다");
        }
        let s = Scratch::fenced("layer-enter-enters");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.hit("Enter");
        assert!(!a.on_layer(), "머리줄의 Enter 가 프로젝트로 안 들어갔다");
    }

    /// **보기는 펼친 프로젝트 전부에 걸리고, 검색과 거름망은 프로젝트 안에서만 건다**(moai-1xo5,
    /// 사용자 결정 2026-09-19). 보기·정렬·열은 보는 사람의 것이라 화면에 하나뿐이다 — 안 걸면 같은
    /// 화면의 두 프로젝트가 한 토글에 다르게 선다. 찾는 일은 반대로 한 프로젝트의 물음이다.
    #[test]
    fn the_view_crosses_projects_but_the_search_does_not() {
        let s = Scratch::fenced("layer-view-cross");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.hit("l");
        settle(&mut a);
        a.key(key(KeyCode::End));
        a.hit("l");
        settle(&mut a);
        // **프로젝트마다 줄 수를 센다**(리뷰). 남은 프로젝트 *수* 로만 재던 때는 한쪽에만 걸려도
        // 둘 다 줄이 남아 통과했다 — 그때 이 시험은 제 이름의 것을 못 잡는다.
        let per_place = |a: &App| -> std::collections::BTreeMap<usize, usize> {
            let mut out = std::collections::BTreeMap::new();
            for r in a.rows() {
                if let Row::Item(crate::tui::Seat::Place(n), ..) = r {
                    *out.entry(n).or_insert(0usize) += 1;
                }
            }
            out
        };
        let before = per_place(&a);
        assert_eq!(before.len(), 2, "두 프로젝트를 다 못 폈다: {before:?}");

        // 미룬 것도 done 도 없는 fixture 라, 칸 하나를 숨겨 둘 다 한 줄씩 주는지 본다.
        a.hit("SPC v 1 Esc");
        let after = per_place(&a);
        for (n, was) in &before {
            assert_eq!(after.get(n), Some(&(was - 1)), "프로젝트 {n} 에 보기가 안 걸렸다 — {before:?} → {after:?}");
        }
        a.hit("SPC v 1 Esc");
        assert_eq!(per_place(&a), before, "도로 켜니 줄이 안 돌아왔다");

        // 검색·거름망은 그대로 프로젝트 안의 일이다. 바로 누르는 `/` 는 까닭과 갈 키를 대고,
        // 메뉴의 `SPC f` 는 아예 안 선다 — 메뉴는 켜진 것만 세운다(`menu::entries`).
        a.hit("/");
        assert!(matches!(a.mode, Mode::Browse), "`/` 가 한눈 보기에서 칸을 열었다");
        let said = a.notice.clone().unwrap_or_default();
        assert!(said.contains("프로젝트 안"), "`/` 가 까닭을 안 댄다 — {said}");
        a.hit("SPC f");
        assert!(matches!(a.mode, Mode::Browse), "`SPC f` 가 한눈 보기에서 칸을 열었다");
    }

    /// **한눈 보기의 줄에 누른 키는 그 줄의 프로젝트로 간다**(moai-5v3q, 사용자 결정 2026-09-19) —
    /// 담기(`n`)는 그 프로젝트에 담고, 읽음(`r`)은 그 프로젝트의 줄에 도장을 찍는다. 지금 선
    /// 프로젝트로 읽으면 남의 목록의 첨자를 이 프로젝트의 줄로 읽어 엉뚱한 데 적힌다.
    #[test]
    fn a_key_on_a_foreign_row_writes_to_that_project() {
        let s = Scratch::fenced("layer-row-writes");
        let (one, _two, mut a) = on_layer_with_twins(&s);
        a.hit("l");
        settle(&mut a);
        // 머리줄 바로 밑 — one 의 줄이다.
        a.key(key(KeyCode::Down));
        let Some(Row::Item(crate::tui::Seat::Place(n), ..)) = a.current() else { panic!("{:?}", a.current()) };
        assert_eq!(a.place_path(n).map(std::path::Path::to_path_buf), Some(one.clone()), "커서가 one 의 줄에 안 섰다");

        // 담기 — 폼의 담을 곳이 그 줄의 프로젝트다.
        a.hit("SPC n");
        assert_eq!(target(&a), Some(one.clone()), "담을 곳이 그 줄의 프로젝트가 아니다");
        a.key(key(KeyCode::Esc));

        // 읽음 — 그 프로젝트의 줄에 적힌다. 적을 것이 없으면 그렇다고만 하고 조용하다.
        a.hit("r");
        let said = a.notice.clone().unwrap_or_default();
        assert!(!said.contains("못"), "읽음이 남의 프로젝트로 갔다 — {said}");
    }

    /// **펼치는 키는 그 자리에서 안 읽는다**(moai-12yx, 사용자 결정 2026-09-19) — 스레드에 맡기고
    /// 머리줄이 도는 동안 화면은 그대로 돈다. 워크트리 일곱에 138→458ms 를 잰 값(moai-uxrn)이
    /// 펼치는 키 하나에 통째로 실리지 않게 하는 것이 까닭이다.
    #[test]
    fn opening_a_head_reads_in_a_thread() {
        let s = Scratch::fenced("layer-read-thread");
        let (one, _two, mut a) = on_layer_with_twins(&s);
        let heads = a.rows().len();
        a.hit("l");
        assert!(a.loading(), "펼쳤는데 읽으러 안 갔다");
        assert!(a.reading_place(&one), "읽는 중인 프로젝트를 머리줄이 모른다");
        assert_eq!(a.rows().len(), heads, "읽기를 기다리지 않고 그 자리에서 읽었다");
        settle(&mut a);
        assert!(!a.reading_place(&one), "다 읽고도 도는 중이라 한다");
        assert!(a.rows().len() > heads, "읽어 온 줄이 안 섰다");
        // 두 번 펴도 다시 안 읽는다 — 든 줄이 그대로 선다.
        a.hit("h");
        a.hit("l");
        assert!(!a.loading(), "이미 읽은 프로젝트를 다시 읽으러 갔다");
        assert!(a.rows().len() > heads);
    }

    /// **옆 프로젝트의 보기는 줄도 보기도 안 바뀌면 다시 안 센다**(moai-m59y) — 지금 선 프로젝트
    /// 하나를 다시 읽을 때마다 든 프로젝트 **전부**의 `shown`·`lit` 을 다시 세던 자리다. 보기가
    /// 바뀌면 그때는 든 것이 다 따라 선다 — 보기는 보는 사람의 것이라 화면에 하나뿐이다(moai-1xo5).
    #[test]
    fn a_neighbour_is_not_measured_again_when_this_project_is_read() {
        let s = Scratch::fenced("layer-see-again");
        let (_one, _two, mut a) = on_layer_with_twins(&s);
        a.want_site(0);
        settle(&mut a);
        assert!(a.site_of_place(0).is_some_and(|s| s.shown == [true, true]), "시험의 전제 — 옆 줄이 다 보인다");

        // 옆 프로젝트의 값을 손으로 흐트러뜨린다 — 다시 셌는지가 이 값으로 드러난다. 한 줄만
        // 건드린다: 둘 다 숨기면 층에 이슈 줄이 하나도 안 서 보기 토글 자체가 안 듣는다.
        if let Some(site) = a.site_mut(super::super::Seat::Place(0)) {
            site.shown[0] = false;
        }
        a.adopt(Vec::new());
        assert!(
            a.site_of_place(0).is_some_and(|s| s.shown == [false, true]),
            "지금 선 프로젝트를 다시 읽었을 뿐인데 옆 프로젝트를 다시 셌다"
        );

        // 보기가 바뀌면 든 것이 다 따라 선다.
        a.hit("SPC v d");
        assert!(a.site_of_place(0).is_some_and(|s| s.shown == [true, true]), "보기 토글이 옆 프로젝트를 안 다시 셌다 — {:?}", a.view);
    }

    /// **펼치기는 여는 데까지만 그 자리에서 한다**(moai-m59y) — 줄은 스레드가 읽는다. 열리지만
    /// 스냅샷을 못 읽는 프로젝트가 그 둘을 가른다: 스냅샷까지 그 자리에서 읽던 때(`Depth::Whole`)는
    /// 스레드가 아예 안 뜨고 머리줄이 `Look::Shut` 으로 고쳐 섰고, 지금은 일꾼이 그 파일을 읽다
    /// 만나 까닭을 제 길로 댄다 — UI 실은 같은 파일을 두 번 파지 않는다.
    #[test]
    fn expanding_opens_without_reading_the_snapshot_here() {
        let s = Scratch::fenced("layer-lean-open");
        let bad = s.project("bad", &[("argos-0001", "못 읽을 줄", "todo")]);
        // 파일 자리에 디렉터리를 둔다 — `Repo::open` 은 설정까지만 보므로 열리고, 읽기가 터진다.
        let file = bad.join(".moai/issues.jsonl");
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        let cfg = s.register(&[&bad]);
        let mut a = layered(&cfg);
        a.notice = None;

        a.hit("l");
        assert!(a.loading(), "펼쳤는데 읽으러 안 갔다 — 그 자리에서 읽고 말았다");
        settle(&mut a);
        let said = a.notice.clone().unwrap_or_default();
        assert!(said.contains("줄을 못 읽었다"), "일꾼이 만난 까닭을 안 댔다 — {said:?}");
    }

    /// **머리줄의 `Tab` 은 그 프로젝트를 묶음까지 다 편다**(moai-i0wd) — 펼쳐져 있으면 통째로
    /// 접는다. 묶음 줄의 `Tab` 과 같은 자다([`App::expand_all`]).
    ///
    /// **fixture 에 묶음을 둔다**(moai-m59y) — 줄만 있는 프로젝트로 재던 때는 `l` 과 `Tab` 이
    /// 같은 줄 수를 내, [`App::open_all`] 을 통째로 지워도 이 시험이 지나갔다. 에픽 하나와 그
    /// 멤버가 있어야 "묶음까지" 가 실제로 걸린다.
    ///
    /// **`Tab` 은 안 읽은 머리줄에서 먼저 누른다**(리뷰) — 그래야 [`App::deep`] 에 뜻을 담고
    /// 읽어 온 뒤에 펴는 길([`App::follow_site`])을 지난다. 읽어 둔 프로젝트에서 누르면 `open_all`
    /// 이 그 자리에서 돌아, 그 길을 통째로 지워도 이 시험이 지나갔다.
    #[test]
    fn tab_opens_a_whole_project_and_folds_it_again() {
        let s = Scratch::fenced("layer-tab-all");
        let deep = s.project("deep", &[("argos-0001", "묶음 밖의 줄", "todo")]);
        write_group(&deep);
        let cfg = s.register(&[&deep]);
        let mut a = layered(&cfg);
        let heads = a.rows().len();
        assert_eq!(heads, 1, "시험의 전제 — 안 읽은 프로젝트는 머리줄만 선다");

        // 안 읽은 채로 누른 `Tab` — 읽으러 가고, 줄이 닿은 뒤에 묶음까지 편다.
        a.hit("Tab");
        assert!(a.loading(), "Tab 이 읽으러 안 갔다 — 그 자리에서 읽고 말았다");
        settle(&mut a);
        let all = a.rows().len();
        assert!(titles(&a).contains(&"에픽의 멤버".to_string()), "읽어 온 뒤에 묶음을 안 폈다 — {:?}", titles(&a));
        a.hit("Tab");
        assert_eq!(a.rows().len(), heads, "다시 누른 Tab 이 안 접었다");

        // `l` 은 머리줄만 편다 — 에픽은 접힌 채라 멤버가 안 선다. 담아 둔 `Tab` 의 뜻이 남아
        // 있으면 여기서 통째로 펼쳐진다.
        a.hit("l");
        settle(&mut a);
        let shallow = a.rows().len();
        assert!(shallow > heads && shallow < all, "l 이 한 층만 안 폈다 — {:?}", titles(&a));
        assert!(!titles(&a).contains(&"에픽의 멤버".to_string()), "l 이 묶음까지 폈다 — {:?}", titles(&a));
    }

    /// **못 읽은 읽기도 `Tab` 의 뜻을 걷는다**(리뷰) — 안 걷으면 그 뜻이 남아, 다음에 `l` 로 한
    /// 층만 펴려던 사람이 통째로 펼쳐진 프로젝트를 본다. [`Depth::Lean`] 이 "열리지만 스냅샷을 못
    /// 읽는" 저장소를 일꾼에게 보내면서 그 갈래가 실제로 닿는다.
    #[test]
    fn a_failed_read_does_not_leave_the_tab_intent_behind() {
        let s = Scratch::fenced("layer-tab-failed");
        let deep = s.project("deep", &[("argos-0001", "묶음 밖의 줄", "todo")]);
        write_group(&deep);
        let file = deep.join(".moai/issues.jsonl");
        let body = std::fs::read_to_string(&file).unwrap();
        // 파일 자리에 디렉터리를 둔다 — `Repo::open` 은 설정까지만 보므로 열리고, 읽기가 터진다.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        let cfg = s.register(&[&deep]);
        let mut a = layered(&cfg);
        let heads = a.rows().len();

        a.hit("Tab");
        settle(&mut a);
        assert_eq!(a.rows().len(), heads, "시험의 전제 — 못 읽었으니 머리줄만 선다");

        // 파일을 고치고 이번에는 `l` 로 한 층만 편다.
        std::fs::remove_dir(&file).unwrap();
        std::fs::write(&file, body).unwrap();
        a.hit("l");
        settle(&mut a);
        assert!(a.rows().len() > heads, "고친 뒤의 l 이 프로젝트를 안 폈다");
        assert!(!titles(&a).contains(&"에픽의 멤버".to_string()), "걷지 않은 Tab 의 뜻이 l 을 통째로 폈다 — {:?}", titles(&a));
    }

    /// 에픽 하나와 그 멤버를 그 프로젝트에 더한다 — 이미 쓴 줄 뒤에 잇는다.
    fn write_group(dir: &Path) {
        let mut epic = Issue::new("argos-0100".into(), "에픽".into(), Kind::Epic, Status::new("todo"), "2026-09-01T00:00:00Z");
        epic.priority = Some(2);
        let mut member =
            Issue::new("argos-0101".into(), "에픽의 멤버".into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z");
        member.epic = Some(epic.id.clone());
        let file = dir.join(".moai/issues.jsonl");
        let mut body = std::fs::read_to_string(&file).unwrap();
        for i in [epic, member] {
            body.push_str(&format!("{}\n", serde_json::to_string(&i).unwrap()));
        }
        std::fs::write(&file, body).unwrap();
    }

    /// **건너뛰면 포커스가 목록으로 돌아온다**(리뷰 moai-i784.pzh) — 층의 상세에는 듣는 키가
    /// 없어, 상세에 포커스를 둔 채 `0` 을 누르면 무엇을 눌러야 할지 없는 화면이 선다.
    #[test]
    fn a_digit_jump_puts_the_focus_back_on_the_list() {
        let s = Scratch::fenced("layer-digit-focus");
        let (_one, two, mut a) = on_layer_with_twins(&s);
        a.hit("2");
        a.hit("Ctrl-w w");
        assert_eq!(a.focus, super::super::Pane::Detail, "시험의 전제 — 상세에 포커스가 갔다");
        a.hit("0");
        assert!(a.on_layer());
        assert_eq!(a.focus, super::super::Pane::Explorer, "층에 섰는데 포커스가 상세에 남았다");

        a.hit("Ctrl-w w");
        a.hit("1");
        assert_eq!(a.focus, super::super::Pane::Explorer, "프로젝트로 건너뛰었는데 포커스가 상세에 남았다");
        assert_ne!(a.here(), Some(two), "1 이 첫 프로젝트로 안 갔다");

        // **이미 그 자리여도 돌아온다**(리뷰). 안 옮기는 갈래로 떨어지면 포커스를 안 건드려,
        // 고치려던 그 막힌 화면(층의 상세)에 그대로 남는 길이 남는다.
        a.hit("Ctrl-w w");
        a.hit("1");
        assert_eq!(a.focus, super::super::Pane::Explorer, "이미 선 프로젝트의 번호를 누르니 포커스가 상세에 남았다");
        a.hit("0");
        a.hit("Ctrl-w w");
        assert_eq!(a.focus, super::super::Pane::Detail, "시험의 전제 — 층에서도 상세로 간다");
        a.hit("0");
        assert!(a.on_layer());
        assert_eq!(a.focus, super::super::Pane::Explorer, "층에서 누른 `0` 이 포커스를 안 돌렸다");
    }

    /// **들어가기도 여는 데까지만 그 자리에서 한다**(moai-lmot) — 줄은 바로 아래에서 읽으므로,
    /// 여기서 스냅샷을 또 파면 같은 파일을 두 번 판다. 열리지만 스냅샷을 못 읽는 프로젝트가 그
    /// 둘을 가른다: 스냅샷까지 그 자리에서 읽던 때(`Depth::Whole`)는 여는 쪽이 CLI 한눈 보기의
    /// 말로 줄을 고쳐 세웠고, 지금은 읽기가 제 까닭을 댄다. 어느 쪽이든 **안 들어간다.**
    #[test]
    fn entering_opens_without_reading_the_snapshot_twice() {
        let s = Scratch::fenced("layer-enter-lean");
        let here = s.project("here", &[("argos-0001", "여기 줄", "todo")]);
        let bad = s.project("bad", &[("argos-0002", "못 읽을 줄", "todo")]);
        // 파일 자리에 디렉터리를 둔다 — `Repo::open` 은 설정까지만 보므로 열리고, 읽기가 터진다.
        let file = bad.join(".moai/issues.jsonl");
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        let cfg = s.register(&[&here, &bad]);
        let mut a = layered(&cfg);
        a.hit("1");
        assert_eq!(a.here(), Some(here.clone()), "시험의 전제 — 첫 프로젝트에 들어갔다");

        a.hit("2");
        assert_eq!(a.here(), Some(here), "못 읽는 프로젝트로 들어가 버렸다");
        let said = a.notice.clone().unwrap_or_default();
        assert!(said.contains("들어가지 못했다"), "읽기가 만난 까닭을 안 댔다 — {said:?}");
        // 그 줄은 다음 쓸기가 다시 읽어 제 상태로 선다 — 여는 쪽이 고쳐 세우지 않는다.
        assert!(a.layer.as_ref().unwrap().places[1].read_at.is_none(), "다시 읽을 줄로 안 뒀다");
    }

    /// **못 들어가면 포커스도 그대로다**(리뷰). `enter_project` 는 실패하면 선 자리를 안 바꾸고
    /// 까닭만 알림으로 다는데, 부르는 쪽이 포커스만 옮기면 읽던 상세가 키를 잃는다.
    #[test]
    fn a_failed_digit_jump_leaves_the_focus_where_it_was() {
        let s = Scratch::fenced("layer-digit-jump-fail");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.hit("1");
        assert_eq!(a.here(), Some(one.clone()), "1 이 첫 프로젝트로 안 갔다");
        a.hit("Ctrl-w w");
        assert_eq!(a.focus, super::super::Pane::Detail, "시험의 전제 — 상세에 포커스가 갔다");

        // 둘째 프로젝트를 통째로 치운다 — `open_place` 가 못 열고 `enter_project` 는 알림만 단다.
        std::fs::remove_dir_all(&two).unwrap();
        a.hit("2");
        assert_eq!(a.here(), Some(one), "못 여는 프로젝트로 들어가 버렸다");
        assert!(a.notice.is_some(), "못 들어갔는데 아무 말도 없다");
        assert_eq!(a.focus, super::super::Pane::Detail, "못 들어갔는데 포커스만 옮겼다");
    }

    /// **맨 숫자가 프로젝트를 고른다**(moai-o133) — `1`·`2` 는 등록 차례의 프로젝트로 바로 들어가고,
    /// `0` 은 층으로 돌아온다. 헤더가 그 번호를 대므로 어디서 눌러도 같은 자리로 간다.
    #[test]
    fn a_bare_digit_jumps_to_that_project_and_zero_comes_back() {
        let s = Scratch::fenced("layer-digit-jump");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.hit("2");
        assert_eq!(a.here(), Some(two.clone()), "2 가 둘째 프로젝트로 안 갔다");
        a.hit("1");
        assert_eq!(a.here(), Some(one.clone()), "프로젝트 안에서 누른 1 이 첫째로 안 갔다");
        a.hit("0");
        assert!(a.on_layer(), "0 이 층으로 안 돌아왔다");
        // 등록한 수를 넘는 번호는 아무 일도 안 한다 — 없는 자리로 보내면 무엇이 일어났는지 모른다.
        a.hit("2");
        a.hit("7");
        assert_eq!(a.here(), Some(two), "없는 번호가 선 자리를 흔들었다");
    }

    /// **건너뛰어도 떠난 프로젝트의 것은 안 따라온다**(리뷰). 거름망은 한 프로젝트의 줄과 칸
    /// 이름에 매이고 끈 겹쳐 보기는 그 프로젝트에 매인 뜻이라 올라올 때 풀리는데(`climb`),
    /// 맨 숫자는 층을 안 거쳐 그 길이 안 돈다 — 안 풀면 옆 프로젝트가 시키지도 않은 거른
    /// 화면으로, 끈 화면으로 열린다.
    #[test]
    fn a_digit_jump_drops_what_is_bound_to_the_project_it_leaves() {
        let s = Scratch::fenced("layer-digit-jump-clean");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.hit("1");
        assert_eq!(a.here(), Some(one), "1 이 첫째 프로젝트로 안 갔다");
        a.hit("SPC f");
        for c in "status=in_progress".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filter_text.as_deref(), Some("status=in_progress"));
        a.hit("SPC v w Esc");
        assert!(!a.worktree, "프로젝트 안에서 w 가 안 껐다");

        a.hit("2");
        assert_eq!(a.here(), Some(two), "2 가 둘째 프로젝트로 안 갔다");
        assert_eq!(a.filter_text, None, "거름망이 옆 프로젝트로 따라왔다");
        assert!(a.worktree, "끈 겹쳐 보기가 옆 프로젝트로 따라왔다");
    }

    /// **프로젝트 안에서 `n` 은 그 프로젝트에만 담는다.** 같은 id 를 쓰는 두 프로젝트 중 선
    /// 프로젝트의 파일만 바뀌고, 알림이 어느 프로젝트인지 댄다.
    #[test]
    fn n_inside_a_project_writes_only_that_projects_file() {
        let s = Scratch::fenced("layer-jot-inside");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.here(), Some(two.clone()));
        let before = snapshots(&[&one]);

        a.hit("SPC n");
        assert_eq!(target(&a), Some(two.clone()), "폼이 선 프로젝트를 담을 곳으로 안 박았다");
        type_in(&mut a, "two 에 담을 것");
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);

        assert_eq!(ideas_at(&two), ["two 에 담을 것"]);
        assert_eq!(snapshots(&[&one]), before, "선 프로젝트 말고 다른 프로젝트 파일이 바뀌었다");
        assert!(!one.join(".moai/journal.jsonl").exists());
        let n = a.notice.clone().unwrap_or_default();
        assert!(n.starts_with("✓ 담김 · two · argos-"), "알림이 프로젝트를 안 댄다 — {n:?}");
    }

    /// **층에서 `n` 은 커서의 프로젝트를 담을 곳으로 박아 층에 선 채 폼을 연다.** 담으면 그
    /// 프로젝트로 들어가 만든 줄에 서고, 다른 프로젝트 파일은 그대로다. Esc 로 닫으면 아무
    /// 데도 안 들어간다.
    #[test]
    fn n_on_the_layer_saves_into_the_project_under_the_cursor() {
        let s = Scratch::fenced("layer-jot-layer");
        let (one, two, mut a) = on_layer_with_twins(&s);
        let before = snapshots(&[&one, &two]);

        a.hit("SPC n");
        assert!(a.on_layer(), "폼을 열며 프로젝트로 들어갔다");
        assert_eq!(target(&a), Some(one.clone()));
        a.key(key(KeyCode::Esc));
        assert!(a.on_layer() && a.mode == Mode::Browse, "빈 폼을 닫았는데 들어갔다");
        assert_eq!(snapshots(&[&one, &two]), before);

        a.hit("SPC n");
        type_in(&mut a, "one 에 담을 것");
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert_eq!(a.here(), Some(one.clone()), "담은 프로젝트로 안 들어갔다");
        assert_eq!(ideas_at(&one), ["one 에 담을 것"]);
        assert_eq!(snapshots(&[&two])[0], before[1], "커서의 프로젝트 말고 다른 파일이 바뀌었다");
        assert_eq!(titles(&a).iter().filter(|t| *t == "one 에 담을 것").count(), 1);
        let on = a.current().and_then(|r| match r {
            Row::Item(_, e, _) => e.at().map(|at| a.site.issues[at].title.clone()),
            _ => None,
        });
        assert_eq!(on.as_deref(), Some("one 에 담을 것"), "만든 줄에 안 섰다");
        assert!(a.notice.as_deref().is_some_and(|n| n.starts_with("✓ 담김 · one · ")), "{:?}", a.notice);
    }

    /// **담을 곳은 여는 순간 경로로 박힌다.** 폼이 열린 동안 층이 다시 읽혀 차례가 바뀌고
    /// 커서가 옆 프로젝트로 가도, 담기는 곳은 머리에 보인 그 프로젝트다.
    #[test]
    fn the_target_is_fixed_when_the_form_opens() {
        let s = Scratch::fenced("layer-jot-fixed");
        let (one, two, mut a) = on_layer_with_twins(&s);
        let before = snapshots(&[&two]);
        a.hit("SPC n");
        type_in(&mut a, "one 의 생각");

        // 폼이 열린 동안에는 키로 못 옮기므로 속을 직접 흔든다 — 등록 차례가 뒤집히고, 층이
        // 다시 읽히고, 커서가 0(이제 two)에 선다.
        s.register(&[&two, &one]);
        a.relayer(None);
        a.cursor = 0;
        a.follow();
        assert_eq!(a.current(), Some(Row::Project(0)));
        assert_eq!(a.place_path(0), Some(two.as_path()), "판이 다르다 — 차례가 안 뒤집혔다");
        assert_eq!(target(&a), Some(one.clone()), "층이 다시 읽히자 담을 곳이 바뀌었다");

        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert_eq!(ideas_at(&one), ["one 의 생각"]);
        assert_eq!(snapshots(&[&two]), before, "커서가 옮겨 간 프로젝트에 담겼다");

        // 프로젝트 안에서 연 폼인데 선 곳이 그새 다른 프로젝트면 **쓰지 않는다.**
        a.hit("SPC n");
        type_in(&mut a, "one 에만");
        a.climb();
        let two_at = a.layer.as_ref().unwrap().position(&two).unwrap();
        a.enter_project(two_at);
        assert_eq!(a.here(), Some(two.clone()));
        let (one_before, two_before) = (snapshots(&[&one]), snapshots(&[&two]));
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(&a.mode, Mode::Idea(f) if f.title.text() == "one 에만"), "선 곳이 다른데 폼이 닫혔다 — {:?}", a.mode);
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다") && t.contains("one")), "{:?}", a.trouble);
        assert_eq!((snapshots(&[&one]), snapshots(&[&two])), (one_before, two_before), "머리에 보인 곳 말고 다른 곳에 썼다");
    }

    /// **편집기로 적는 동안에도 담을 곳은 연 순간의 프로젝트다**(moai-08af). 편집기가 도는 사이
    /// 층이 다시 읽혀 차례가 뒤집히고 커서가 옆 프로젝트에 서도, 돌아온 글은 박힌 곳에 담긴다.
    #[test]
    fn an_edit_lands_in_the_project_fixed_when_the_editor_opened() {
        let s = Scratch::fenced("layer-edit-fixed");
        let (one, two, mut a) = on_layer_with_twins(&s);
        let before = snapshots(&[&two]);
        a.editor = Some("vi".into());
        a.hit("SPC n");
        let edit = a.edit.take().expect("층에서 편집기를 안 청했다");
        assert_eq!(edit.into.as_ref().map(|t| t.path.clone()), Some(one.clone()));
        assert!(a.on_layer(), "편집기를 청하며 층을 떠났다");

        s.register(&[&two, &one]);
        a.relayer(None);
        a.cursor = 0;
        a.follow();
        assert_eq!(a.place_path(0), Some(two.as_path()), "판이 다르다 — 차례가 안 뒤집혔다");

        a.edited(edit.into, Ok("one 의 생각\n".into()));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert_eq!(ideas_at(&one), ["one 의 생각"]);
        assert_eq!(snapshots(&[&two]), before, "커서가 옮겨 간 프로젝트에 담겼다");
    }

    /// 층에서 연 폼의 프로젝트가 그새 등록에서 빠지면 **쓰지 않고 말한다.** 폼은 적던 그대로다.
    #[test]
    fn a_target_dropped_from_the_layer_is_not_written_elsewhere() {
        let s = Scratch::fenced("layer-jot-dropped");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.hit("SPC n");
        type_in(&mut a, "갈 데 없는 것");
        s.register(&[&two]);
        a.relayer(None);
        let before = snapshots(&[&one, &two]);
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(a.mode, Mode::Idea(_)), "{:?}", a.mode);
        assert!(a.on_layer());
        assert!(a.trouble.as_deref().is_some_and(|t| t.contains("빠졌다")), "{:?}", a.trouble);
        assert_eq!(snapshots(&[&one, &two]), before);
        // 버리고 닫으면 까닭도 걷힌다(쓰기의 실패와 같다)
        a.key(key(KeyCode::Esc));
        a.key(key(KeyCode::Char('y')));
        assert_eq!((&a.mode, &a.trouble), (&Mode::Browse, &None));
    }

    /// **열 수 없는 프로젝트에는 폼을 안 연다** — init 전·사라진 디렉터리. Enter 와 같은
    /// 말(`view::unopened`)을 한 줄로 댄다. 그새 init 했으면 연다.
    #[test]
    fn n_on_a_project_that_cannot_open_opens_no_form() {
        let s = Scratch::fenced("layer-jot-shut");
        let bare = s.dir("bare");
        let gone = s.join("gone");
        let cfg = s.register(&[&bare, &gone]);
        let mut a = layered(&cfg);

        a.hit("SPC n");
        assert_eq!(a.mode, Mode::Browse, "init 전 프로젝트에 폼을 열었다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("init 전")), "{:?}", a.notice);
        a.key(key(KeyCode::Down));
        a.hit("SPC n");
        assert_eq!(a.mode, Mode::Browse);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("디렉터리가 없다")), "{:?}", a.notice);
        assert!(!bare.join(".moai").exists() && !gone.exists(), "못 여는 프로젝트에 무언가 만들었다");

        std::fs::create_dir_all(bare.join(".moai")).unwrap();
        std::fs::write(bare.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        a.key(key(KeyCode::Up));
        a.hit("SPC n");
        assert_eq!(target(&a), Some(bare), "init 한 뒤에도 층의 옛 셈을 보고 안 열었다");
    }

    /// **누군지 묻고 이어진 쓰기도 박은 프로젝트에 담는다.** 층에서 연 폼이 묻는 칸을 지나
    /// 담기면 그 프로젝트 파일에만 선다.
    #[test]
    fn the_question_and_its_retry_stay_on_the_fixed_project() {
        fn nobody(user: Option<&str>, root: &std::path::Path) -> crate::fail::R<crate::model::Actor> {
            match user {
                Some(raw) => crate::model::actor(Some(raw), root),
                None => Err(crate::fail::Fail::coded("누가 하는지 모른다 — 시험", crate::fail::code::NO_ACTOR)),
            }
        }
        let s = Scratch::fenced("layer-jot-ask");
        let (one, two, mut a) = on_layer_with_twins(&s);
        a.user = None;
        a.identify = nobody;
        let before = snapshots(&[&one]);
        a.key(key(KeyCode::Down));
        a.hit("SPC n");
        assert_eq!(target(&a), Some(two.clone()));
        type_in(&mut a, "two 의 생각");
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert!(matches!(a.mode, Mode::Ask(_)), "{:?}", a.mode);
        assert!(ideas_at(&two).is_empty(), "묻기 전에 썼다");

        // 묻는 칸을 Esc 로 물리면 폼이 돌아오고 담을 곳은 그대로다.
        a.key(key(KeyCode::Esc));
        assert_eq!(target(&a), Some(two.clone()));
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        type_in(&mut a, "레이븐 (raven@example.com)");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert_eq!(ideas_at(&two), ["two 의 생각"]);
        assert_eq!(snapshots(&[&one]), before, "묻고 이어진 쓰기가 다른 프로젝트에 닿았다");
        assert_eq!(a.here(), Some(two));
    }

    /// **떠난 프로젝트에서 짓던 읽기는 다음 프로젝트에 안 닿는다.** 같은 id 를 쓰는 두
    /// 프로젝트에서 늦게 닿은 읽기가 들어오면 화면이 남의 줄이 된다.
    #[test]
    fn a_read_in_flight_from_the_project_left_behind_never_lands() {
        let s = Scratch::fenced("layer-inflight");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);

        a.key(key(KeyCode::Enter));
        write_lines(&one, &[("argos-0001", "one 의 새 줄", "todo")]);
        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 안 읽었다");
        a.hit("0");
        // 떠난 프로젝트가 제 줄을 이고 서므로 둘째 머리줄은 맨 아래다(moai-i0wd).
        a.key(key(KeyCode::End));
        a.key(key(KeyCode::Enter));
        settle(&mut a);
        assert_eq!(titles(&a), ["two 의 집은 줄", "two 의 줄"], "떠난 프로젝트의 읽기가 들어왔다");
        assert_eq!(a.site.repo.as_ref().map(|r| r.root.clone()), Some(two));
    }

    /// **밖에서 `moai project add` 한 것이 누르지 않아도 층에 선다**(moai-en4u) — 걸음이 사용자 설정의
    /// 표식을 잰다. 한때 `SPC r` 을 눌러야 섰다. 커서는 보던 프로젝트(경로)에 선다.
    #[test]
    fn a_registration_made_outside_shows_on_the_layer_by_itself() {
        let s = Scratch::fenced("layer-follow-config");
        let (one, two) = twins(&s);
        let three = s.project("three", &[]);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        // 첫 걸음은 재기만 한다 — 띄울 때 이미 읽었다.
        a.follow();
        a.key(key(KeyCode::Down));
        assert_eq!(a.current(), Some(Row::Project(1)));

        // 상세를 굴려 둔다 — 보던 프로젝트에 그대로 서면 굴린 자리도 둔다(차례는 밀려도).
        a.detail.fit(5, 50);
        a.detail.by(3);
        s.register(&[&three, &one, &two]);
        settle(&mut a);
        assert_eq!(names(&a), ["three", "one", "two"]);
        assert_eq!(a.current(), Some(Row::Project(2)), "보던 프로젝트를 놓쳤다");
        assert_eq!(a.detail.offset(), 3, "같은 프로젝트에 섰는데 상세를 되감았다");
        assert!(matches!(look(&a, "three"), Look::Open { .. }));

        // 보던 것을 빼면 그 번호를 자른 자리의 **다른** 프로젝트에 서고, 상세는 첫 줄부터다.
        s.register(&[&three, &one]);
        settle(&mut a);
        assert_eq!(a.current(), Some(Row::Project(1)), "뺀 줄의 번호를 목록 안으로 안 잘랐다");
        assert_eq!(a.detail.offset(), 0, "다른 프로젝트에 섰는데 굴린 자리가 남았다");
    }

    /// **설정을 못 읽어도 층은 그대로 남고, 언젠가 다시 읽는다**(moai-9p7v, moai-po6v). 표식은 걸음마다
    /// 재지만 읽기는 한 번 지면 그만이라, 한 번의 실패가 층을 빈 채로 두고 설정이 **다시 바뀔 때까지**
    /// 아무도 다시 읽지 않았다. 손으로 누르던 비상구(`SPC r`)는 걷었다(moai-en4u).
    ///
    /// 권한으로 읽기만 지게 한다 — `chmod` 는 고친 때도 길이도 안 바꿔([`crate::store::stamp`]) 표식이
    /// 그대로다. 표식만 보면 되돌린 뒤에도 "이미 본 설정" 이라 세 번째 프로젝트가 영영 안 선다.
    ///
    /// **다시 읽는 때가 시계로 옮겨 갔다**(moai-po6v) — 이 갈래는 다시 해도 같은 쪽이라 걸음마다 읽는
    /// 것은 헛돈다. 한때는 못 읽으면 표식을 물려 다음 걸음이 다시 읽게 했는데, 그 길은 띄울 때 진
    /// 읽기를 못 갚는다(`a_read_the_launch_lost_is_owed_though_the_stamp_never_moves`).
    #[test]
    #[cfg(unix)]
    fn a_config_that_could_not_be_read_leaves_the_layer_alone_and_is_retried() {
        use std::os::unix::fs::PermissionsExt;
        let chmod = |p: &Path, mode: u32| std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
        let s = Scratch::fenced("layer-config-unreadable");
        let (one, two) = twins(&s);
        let three = s.project("three", &[]);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        // 첫 걸음은 재기만 한다 — 띄울 때 이미 읽었다.
        a.follow();
        assert_eq!(names(&a), ["one", "two"], "시험의 전제 — 층이 섰다");
        // 옆 터미널이 읽음을 적었다 — 걸음이 등록 목록과 **같은 읽기로** 그것을 든다(moai-7yil).
        let marks = format!("{}\n[read]\nargos-0001 = \"2026-09-14T00:00:00Z\"\n", std::fs::read_to_string(&cfg).unwrap());
        std::fs::write(&cfg, &marks).unwrap();
        settle(&mut a);
        assert_eq!(a.legacy_read.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "시험의 전제 — 옛 [read] 를 들었다");

        // 설정이 바뀌었는데 그 읽기가 진다. 층도 읽어 둔 줄도 그대로여야 한다.
        s.register(&[&one, &two, &three]);
        chmod(&cfg, 0o000);
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two"], "못 읽은 것으로 층이 사라졌다");
        assert_eq!(a.config_tried.trouble, Some(user_config::Trouble::Unreadable), "다시 읽을 때를 정할 갈래를 안 들었다");
        assert_eq!(a.legacy_read.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "못 읽은 설정의 빈 표를 들여 옛 [read] 가 사라졌다");

        // 권한만 되돌린다 — 파일은 그대로라 표식도 그대로다. 시계가 돌아야 다시 읽는다.
        chmod(&cfg, 0o644);
        a.config_tried.at = Some(std::time::Instant::now().checked_sub(REREAD_EVERY).expect("시계가 1분도 안 돌았다"));
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two", "three"], "읽을 수 있게 됐는데 다시 안 읽었다");
        assert_eq!(a.config_tried.trouble, None, "읽혔는데 탈이 남았다");
    }

    /// **잠깐이라던 것이 잠깐이 아니면 시계로 내린다**(moai-4qbv.i0g 리뷰). [`user_config::Again::Step`]
    /// 은 모르는 실패가 다 모이는 자리라(`user_config::unreadable` 이 안 집은 것), 끊긴 sshfs·NFS 의
    /// `ENOTCONN`·`EIO` 가 여기 든다 — 끝을 안 두면 **막히는 읽기**를 걸음마다 다시 연다.
    #[test]
    fn a_blip_that_lasts_is_no_blip_and_drops_to_the_clock() {
        let long_ago = |d| std::time::Instant::now().checked_sub(d).expect("시계가 1분도 안 돌았다");
        let mut t = Tried::default();
        assert!(!owed(&t), "멀쩡한데 빚이 섰다");

        t.saw(Some(user_config::Trouble::Reading));
        let first = t.since;
        assert!(owed(&t), "잠깐의 실패를 다음 걸음에 안 갚는다");
        t.saw(Some(user_config::Trouble::Reading));
        assert_eq!(t.since, first, "이어서 지는데 처음 진 때를 다시 찍었다 — 끝이 영영 안 온다");

        t.since = Some(long_ago(BLIP));
        assert!(!owed(&t), "잠깐이 아닌 것을 걸음마다 다시 읽는다");
        t.at = Some(long_ago(REREAD_EVERY));
        assert!(owed(&t), "시계가 돌았는데 다시 안 본다");

        t.saw(None);
        assert!(!owed(&t) && t.since.is_none(), "읽혔는데 자취가 남았다 — 다음 실패가 잠깐부터 못 센다");
    }

    /// **띄울 때 진 읽기는 빚으로 남아 다음 걸음이 갚는다**(moai-po6v). 표식은 읽기 **전에** 재므로
    /// 진 뒤에도 파일의 것과 같다 — 표식만 보면 그 한 번의 실패가 세션 내내 남아, 등록한 프로젝트가
    /// 영영 안 선다. `config_stamp` 를 `None` 으로 두는 것으로는 안 된다: 다음 걸음이 같은 표식을
    /// 다시 재고 그것을 밑값으로 삼아 돌아설 뿐이다.
    ///
    /// 권한으로 지게 한다 — `chmod` 은 고친 때도 길이도 안 바꿔([`crate::store::stamp`]) 표식이 내내
    /// 그대로다. 그 갈래는 시계로 다시 보는 자리라([`user_config::Again::Clock`]) 시계를 돌려 갚는다.
    #[test]
    #[cfg(unix)]
    fn a_read_the_launch_lost_is_owed_though_the_stamp_never_moves() {
        use std::os::unix::fs::PermissionsExt;
        let chmod = |p: &Path, mode: u32| std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
        let s = Scratch::fenced("layer-config-owed");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);

        // 띄울 때 못 읽었다 — `cmd::tui` 와 같은 차례로 표식을 먼저 재고 읽는다.
        chmod(&cfg, 0o000);
        // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다 — 이웃 시험과 같은 문지기다.
        if std::fs::read(&cfg).is_ok() {
            chmod(&cfg, 0o644);
            return;
        }
        let stamp = crate::store::stamp(&cfg);
        let reg = user_config::read(Some(&cfg));
        assert_eq!(reg.trouble, Some(user_config::Trouble::Unreadable), "시험의 전제 — 읽기가 졌다");
        let mut a = App::on_projects(Layer::of(&reg, None));
        a.user_config = Some(cfg.clone());
        a.config_stamp = Some(stamp);
        a.config_tried.saw(reg.trouble);
        settle(&mut a);
        assert!(names(&a).is_empty(), "시험의 전제 — 못 읽어 줄이 없다");

        // 권한을 되돌린다. 파일은 한 바이트도 안 바뀌어 표식이 그대로다.
        chmod(&cfg, 0o644);
        assert_eq!(crate::store::stamp(&cfg), stamp, "시험의 전제 — chmod 은 표식을 안 바꾼다");
        settle(&mut a);
        assert!(names(&a).is_empty(), "다시 해도 같은 갈래를 걸음마다 다시 읽었다");

        a.config_tried.at = Some(std::time::Instant::now().checked_sub(REREAD_EVERY).expect("시계가 1분도 안 돌았다"));
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two"], "시계가 돌았는데 빚진 읽기를 안 갚았다");
        assert_eq!(a.config_tried.trouble, None, "읽혔는데 탈이 남았다");
    }

    /// **잠깐의 실패는 다음 걸음에 갚는다**(moai-po6v) — 시계를 안 기다린다. 갈래마다 벗어나는 길이
    /// 다르고([`user_config::Again`]), 잠깐인 것을 시계에 걸면 1분 동안 층이 빈 채로 선다.
    #[test]
    fn a_blip_is_read_again_on_the_very_next_step() {
        let s = Scratch::fenced("layer-config-blip");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::of(&user_config::Registry::default(), None));
        a.user_config = Some(cfg.clone());
        a.config_stamp = Some(crate::store::stamp(&cfg));
        a.config_tried.saw(Some(user_config::Trouble::Reading));
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two"], "잠깐의 실패를 시계에 걸어 다음 걸음이 안 읽었다");
    }

    /// **쭉 못 읽어도 조용하지 않다**(moai-po6v, 사용자 결정 2026-09-19) — 지난 것을 들고 서되 까닭을
    /// 배너에 댄다. 한때 들고 있던 층을 두고 `return` 만 해, 옆 터미널에서 등록한 프로젝트가 영영 안
    /// 서는데 까닭을 아무 데서도 못 읽었다. 그 말이 서는 자리는 [`App::held`] 다(moai-23pm) —
    /// `Layer::problems` 에 실으면 배너가 `on_layer()` 로 막아 프로젝트 안에서는 아무 말이 없다.
    #[test]
    #[cfg(unix)]
    fn a_config_we_cannot_read_keeps_the_layer_and_says_why() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::fenced("layer-config-said");
        let (one, two) = twins(&s);
        let three = s.project("three", &[]);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        a.follow();
        assert_eq!(names(&a), ["one", "two"], "시험의 전제 — 층이 섰다");

        // 설정이 바뀌었는데 그 읽기가 진다.
        s.register(&[&one, &two, &three]);
        std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o000)).unwrap();
        // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다 — 이웃 시험과 같은 문지기다.
        if std::fs::read(&cfg).is_ok() {
            std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o644)).unwrap();
            return;
        }
        settle(&mut a);
        std::fs::set_permissions(&cfg, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(names(&a), ["one", "two"], "못 읽은 것으로 층이 사라졌다");
        // **서는 자리는 `App::held` 다**(moai-23pm) — 층의 `problems` 는 읽힌 설정의 틀린 줄을 대는
        // 자리라 배너가 프로젝트 안에서 막는다.
        let said = a.held.clone().expect("들고 선 까닭을 안 댔다");
        assert!(said.contains("들고 있다"), "지난 것을 들고 있다는 말이 없다 — {said:?}");
        assert!(said.contains(&cfg.display().to_string()), "어느 파일인지 안 댔다 — {said:?}");
    }

    /// **설정 파일이 사라져도 지난 것을 들고 선다**(moai-po6v) — autofs·sshfs 홈이 끊기면 설정이 빈
    /// 디렉터리째 사라진다. "없다" 를 "읽었더니 비었다" 로 들면 층도 읽음도 빈 채로 갈리는데,
    /// 그것이 moai-9p7v 가 막으려던 바로 그 해다(걷어 낸 `read_marks_at` 에도 있던 구멍이다).
    #[test]
    fn a_config_that_vanished_keeps_the_layer_and_the_read_marks() {
        let s = Scratch::fenced("layer-config-gone");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        a.follow();
        let marks = format!("{}\n[read]\nargos-0001 = \"2026-09-14T00:00:00Z\"\n", std::fs::read_to_string(&cfg).unwrap());
        std::fs::write(&cfg, &marks).unwrap();
        settle(&mut a);
        assert_eq!(a.legacy_read.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "시험의 전제 — 옛 [read] 를 들었다");

        std::fs::remove_file(&cfg).unwrap();
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two"], "사라진 설정으로 층이 비었다");
        assert_eq!(
            a.legacy_read.get("argos-0001").map(String::as_str),
            Some("2026-09-14T00:00:00Z"),
            "사라진 설정의 빈 표를 들여 옛 [read] 가 사라졌다"
        );

        // 돌아오면 표식이 그것을 낸다 — 없는 파일의 표식은 `None` 이라 늘 갈린다.
        std::fs::write(&cfg, &marks).unwrap();
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two"], "돌아온 설정을 안 읽었다");
        assert_eq!(a.config_tried.trouble, None, "돌아왔는데 탈이 남았다");
    }

    /// **설정을 한 번도 안 만든 사람에게 잃었다고 하지 않는다**(moai-po6v). 없는 파일은 갈래로는
    /// 탈이지만(`Trouble::Gone`) 사람에게 댈 까닭은 아니다 — 들고 있는 줄이 하나도 없으면 잃은 것도
    /// 없다. 가르는 자는 등록한 줄이 있는가 하나다.
    #[test]
    fn a_config_that_was_never_made_is_not_told_as_lost() {
        let s = Scratch::fenced("layer-config-never");
        let cfg = s.dir("설정").join("config.toml");
        let mut a = App::on_projects(Layer::of(&user_config::read(Some(&cfg)), None));
        a.user_config = Some(cfg.clone());
        assert!(names(&a).is_empty(), "시험의 전제 — 등록한 줄이 없다");
        a.relayer(None);
        // **보는 자리는 `App::held` 다**(리뷰) — `Layer::problems` 를 보던 판은 이 자리가 거기 안
        // 적게 된 뒤로 무엇을 해도 지나갔다(옆의 두 시험은 옮겼는데 이것만 남았다).
        assert_eq!(a.held, None, "없던 파일을 잃었다고 말했다 — {:?}", a.held);
        let said = &a.layer.as_ref().expect("빈 층이 섰다").problems;
        assert!(said.is_empty(), "없던 파일을 잃었다고 말했다 — {said:?}");
    }

    /// **빈 층에 대고 "지난 것을 들고 있다" 고 하지 않는다**(리뷰) — 까닭은 대되 안 든 것을 들었다고
    /// 말하지 않는다. `.moai` 밖에서 띄운 화면은 등록한 줄이 하나도 없어도 층이 서므로(`App::on_projects`),
    /// 그 사람이 `[tui]` 에 오타를 내면 이 길로 온다. 사라진 파일만 가리던 `held` 를 모든 갈래로 넓힌
    /// 자리다 — 깨진 글은 `problems` 가 비지 않아 그 문지기를 그냥 지나갔다.
    #[test]
    fn a_layer_that_holds_nothing_says_the_reason_without_claiming_to_hold() {
        let s = Scratch::fenced("layer-config-empty-broken");
        let cfg = s.dir("설정").join("config.toml");
        std::fs::write(&cfg, "[tui]\n").unwrap();
        let mut a = App::on_projects(Layer::of(&user_config::read(Some(&cfg)), None));
        a.user_config = Some(cfg.clone());
        assert!(names(&a).is_empty(), "시험의 전제 — 등록한 줄이 없다");

        std::fs::write(&cfg, "[tui\n").unwrap();
        a.relayer(None);
        let said = a.held.clone().expect("깨진 글의 까닭을 안 댔다");
        assert!(!said.contains("들고 있다"), "안 든 것을 들었다고 말했다 — {said:?}");
        assert!(said.contains(&cfg.display().to_string()), "어느 파일인지 안 댔다 — {said:?}");
    }

    /// **한 걸음이 등록과 읽음을 한 번의 읽기로 든다**(moai-7yil). 둘은 한 파일에 산다 — 저마다 읽던
    /// 판은 설정이 바뀐 걸음마다 같은 글을 두 번 파싱하고 층을 다시 세웠다. 보기 토글과 읽음이 그
    /// 파일을 **스스로 써서** 이 길은 자주 돈다(`r` 을 누르고 있으면 누를 때마다).
    #[test]
    fn one_step_takes_the_registration_and_the_read_marks_together() {
        let s = Scratch::fenced("layer-config-one-read");
        let (one, two) = twins(&s);
        let three = s.project("three", &[]);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        a.follow();
        assert_eq!(names(&a), ["one", "two"]);
        assert!(a.legacy_read.is_empty(), "시험의 전제 — 아직 옛 [read] 가 없다");

        // 한 번의 쓰기가 둘을 함께 바꾼다 — 한 걸음 뒤 둘 다 들어 있어야 한다.
        s.register(&[&one, &two, &three]);
        let both = format!("{}\n[read]\nargos-0001 = \"2026-09-14T00:00:00Z\"\n", std::fs::read_to_string(&cfg).unwrap());
        std::fs::write(&cfg, both).unwrap();
        settle(&mut a);
        assert_eq!(names(&a), ["one", "two", "three"], "등록을 안 들었다");
        assert_eq!(a.legacy_read.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "옛 [read] 를 안 들었다");
    }

    /// **깨진 설정은 스스로 다시 읽지 않는다**(moai-9p7v) — 다시 읽어도 같고, 사람이 고치면 파일이
    /// 바뀌어 표식이 그것을 낸다([`user_config::Again::Never`]).
    ///
    /// **층과 읽음은 둘 다 지킨다**(moai-po6v, 사용자 결정 2026-09-19). 깨진 파일의 `read` 는 파싱이
    /// 진 자리라 빈 표이고, 그것을 들이면 내 줄이 통째로 [NEW] 로 선다 — 층도 같다. 한때는 층만
    /// 비웠는데(빈 층과 까닭이 고칠 것을 비춘다는 말이었다), 사람이 `[tui]` 에 오타 하나를 낸 순간
    /// 등록한 프로젝트가 통째로 사라졌다. 고칠 것을 비추는 것은 빈 층이 아니라 **까닭 한 줄**이다.
    #[test]
    fn a_broken_config_is_not_retried_every_step() {
        let s = Scratch::fenced("layer-config-broken");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = layered(&cfg);
        a.user_config = Some(cfg.clone());
        a.follow();
        let marks = format!("{}\n[read]\nargos-0001 = \"2026-09-14T00:00:00Z\"\n", std::fs::read_to_string(&cfg).unwrap());
        std::fs::write(&cfg, &marks).unwrap();
        settle(&mut a);
        assert_eq!(a.legacy_read.get("argos-0001").map(String::as_str), Some("2026-09-14T00:00:00Z"), "시험의 전제 — 옛 [read] 를 들었다");

        std::fs::write(&cfg, "[[project]\npath = ").unwrap();
        settle(&mut a);
        assert_eq!(a.config_stamp, Some(crate::store::stamp(&cfg)), "깨진 설정의 표식은 올라간다");
        assert_eq!(a.config_tried.trouble.map(user_config::Trouble::again), Some(user_config::Again::Never), "깨진 글을 스스로 다시 읽는다");
        assert_eq!(names(&a), ["one", "two"], "오타 하나로 층이 통째로 사라졌다");
        let said = a.held.clone().expect("들고 선 까닭을 안 댔다");
        assert!(said.contains("들고 있다"), "지난 것을 들고 있다는 말이 없다 — {said:?}");
        assert_eq!(
            a.legacy_read.get("argos-0001").map(String::as_str),
            Some("2026-09-14T00:00:00Z"),
            "깨진 설정의 빈 표를 들여 옛 [read] 가 사라졌다"
        );
    }

    /// **못 읽는 줄도 시계로 다시 본다**(리뷰 moai-3lul.kt0 다시 본 판). 권한을 고치는 `chmod` 는 고친
    /// 때도 길이도 안 바꿔, 표식만 보면 층이 "못 읽는다" 를 영영 댄다. init 전 줄은 표식이 다
    /// 보므로 시계에 안 건다.
    #[test]
    fn a_row_that_could_not_be_read_is_retried_by_the_clock() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::fenced("layer-unreadable-clock");
        let (one, two) = twins(&s);
        let bare = s.dir("bare");
        let cfg = s.register(&[&one, &two, &bare]);
        let mut a = layered(&cfg);
        let file = one.join(".moai/issues.jsonl");
        let mode = |m: u32| std::fs::set_permissions(&file, std::fs::Permissions::from_mode(m)).unwrap();
        mode(0o000);
        if std::fs::read(&file).is_ok() {
            // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다.
            mode(0o644);
            return;
        }
        let long_ago = std::time::Instant::now().checked_sub(REREAD_EVERY).expect("시계가 1분도 안 돌았다");
        a.layer.as_mut().unwrap().places[0].read_at = Some(long_ago);
        settle(&mut a);
        assert!(matches!(look(&a, "one"), Look::Shut { state: Shut::Unreadable, .. }), "못 읽게 된 것을 못 봤다");

        mode(0o644);
        let layer = a.layer.as_mut().unwrap();
        for p in &mut layer.places {
            p.read_at = Some(long_ago);
        }
        assert_eq!(layer.stale(), [one.clone(), two.clone()], "못 읽는 줄은 시계로 낡고, init 전 줄은 안 낡는다");
        settle(&mut a);
        assert!(matches!(look(&a, "one"), Look::Open { .. }), "권한을 고쳤는데 층이 옛 까닭을 댄다");
    }

    /// **올라오면 떠난 프로젝트의 줄은 표식이 그대로여도 다시 읽는다**(리뷰 moai-3lul.kt0 다시 본 판).
    /// 안의 배너와 그 줄은 시계가 따로 돌아, 안에 있는 동안 한 시간 틈을 넘긴 줄이 안쪽에는 서고 층에는
    /// 최대 1분 안 섰다. 남의 줄은 표식과 시계대로다. 옆 번호로 건너가며 떠난 줄도 같다 — 올라오기만
    /// 떠나는 길이 아니다(연 줄을 잊는 자리는 `App::open_place` 다).
    #[test]
    fn climbing_rereads_the_project_it_left() {
        let s = Scratch::fenced("layer-climb-forget");
        let (_one, two, mut a) = on_layer_with_twins(&s);
        let read = |a: &App| a.layer.as_ref().unwrap().places.iter().map(|p| p.read_at).collect::<Vec<_>>();
        let before = read(&a);
        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer());
        a.hit("0");
        assert!(a.loading(), "떠난 프로젝트를 다시 읽으러 안 갔다");
        settle(&mut a);
        let after = read(&a);
        assert!(after[0] > before[0], "떠난 프로젝트의 줄을 다시 안 읽었다");
        assert_eq!(after[1], before[1], "안 바뀐 남의 줄까지 다시 읽었다");

        a.hit("1");
        a.hit("2");
        assert_eq!(a.here(), Some(two), "2 가 둘째 프로젝트로 안 갔다");
        a.hit("0");
        settle(&mut a);
        let last = read(&a);
        assert!(last[0] > after[0], "옆 번호로 건너가며 떠난 프로젝트의 줄을 다시 안 읽었다");
        assert!(last[1] > after[1], "올라오며 떠난 프로젝트의 줄을 다시 안 읽었다");
    }

    /// **겹쳐 보기를 꺼도 스냅샷을 못 읽은 옆 워크트리를 댄다**(리뷰 moai-3lul.kt0 다시 본 판, 사용자 결정
    /// moai-rgz9.7vt). 자리 판정은 끈 채로도 옆 스냅샷을 파는데, 못 읽은 것을 안 대면 판정이 가려진 0 이
    /// "없다" 로 읽힌다 — 층은 같은 저장소에 `!` 를 세운다. 켰을 때는 겹치는 읽기가 이미 대므로 두 번 안 댄다.
    #[test]
    fn turning_the_overlay_off_still_names_a_sibling_snapshot_it_could_not_read() {
        let s = Scratch::fenced("layer-unread-sibling");
        let main = s.project("main", &[("argos-0002", "집은 줄", "in_progress")]);
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../wt-x", "-b", "wt-x"]);
        // 파일 자리에 디렉터리가 섰다 — 누가 돌려도(root 여도) 못 읽는다.
        std::fs::create_dir_all(s.join("wt-x/.moai/issues.jsonl")).unwrap();
        let cfg = s.register(&[&main]);
        let mut a = layered(&cfg);
        let Look::Open { sum } = look(&a, "main") else { panic!("main 이 안 열렸다") };
        assert_eq!((sum.unread, sum.blind), (1, 1), "층이 못 읽은 옆 스냅샷을 안 댄다");

        let named = |a: &App| a.site.elsewhere.iter().filter(|l| l.contains("wt-x")).count();
        a.key(key(KeyCode::Enter));
        assert_eq!(named(&a), 1, "겹쳐 볼 때 못 읽은 옆 스냅샷을 안 대거나 두 번 댄다 — {:?}", a.site.elsewhere);
        a.hit("SPC v w Esc");
        assert!(!a.worktree, "w 가 겹쳐 보기를 안 껐다");
        settle(&mut a);
        assert_eq!(named(&a), 1, "겹쳐 보기를 끄자 못 읽은 옆 스냅샷이 배너에서 사라졌다 — {:?}", a.site.elsewhere);
    }

    /// **자리 판정은 옆을 실제로 겹쳤는가로 잰다**(리뷰 moai-3lul.kt0 다시 본 판). 딸린 워크트리에서 git 이
    /// 저장소를 거절하면(깨진 설정, `safe.directory`, git 이 없다) 겹쳐 보기를 켜도 줄은 그 워크트리의
    /// 스냅샷뿐이다 — 갈라질 때의 main 이라, 그 뒤 main 에서 끝낸 일이 아직 집혀 있다. 그것을 겹친 것으로
    /// 재면 끝난 일을 "자리 없다" 로 댄다. 켠 깃발이 아니라 `Gathered::swept` 를 넘기는 두 길 — 다시
    /// 읽기(`prepare`)와 여는 읽기(`App::overlaid`) — 을 모두 지난다.
    #[test]
    fn a_linked_worktree_that_git_refuses_is_judged_like_plain_status() {
        let s = Scratch::fenced("layer-linked-unswept");
        let main = s.project("main", &[("argos-0002", "집은 줄", "in_progress")]);
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["add", ".moai"]);
        run(&main, &["commit", "-q", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../wt-self", "-b", "wt-self"]);
        // main 에서 그 일이 끝났다 — 딸린 워크트리의 스냅샷에는 아직 집힌 채다.
        write_lines(&main, &[("argos-0002", "집은 줄", "done")]);
        let repo = Repo::at(s.join("wt-self"), crate::config::Config::parse("prefix = \"argos\"\n").unwrap());
        let own = repo.read().unwrap().issues;
        // 전제: 겹친 것으로 재면 끝난 일이 자리 없다로 선다. 시계는 줄의 때(2026-09-01)에서 한참 지난
        // 것으로 준다 — 방금 집은 줄의 틈에 걸리면 전제가 안 선다.
        assert_eq!(super::super::placed(&repo, &own, true, "2026-09-10T00:00:00Z", &Default::default()).0, 1, "전제가 안 섰다");

        // git 이 저장소를 거절한다 — 파일로 읽는 자리 판정(`worktree::on_disk`)은 그래도 옆을 찾는다.
        std::fs::write(main.join(".git/config"), "[core\n").unwrap();
        let plain = super::super::warnings_of(&own, &[], &repo.config, &crate::model::now());
        let f = super::super::prepare(&repo, true).unwrap();
        assert!(f.unfound.is_some(), "전제: git 이 저장소를 거절하지 않았다");
        assert_eq!(f.warnings, plain, "못 겹친 딸린 워크트리가 main 에서 끝낸 일을 자리 없다로 댄다 (다시 읽기)");

        let g = crate::worktree::gather(&repo, true).unwrap();
        let stamp = stamp_of(&repo);
        let (index, ground) = super::super::measure(&g.load.issues, &repo.config);
        let a = App::open(repo, g.load, index, ground, NavPath::new(), stamp).overlaid(g.origin, g.trouble, g.watched, g.swept, &g.sides);
        assert_eq!(a.site.warnings, plain, "못 겹친 딸린 워크트리가 main 에서 끝낸 일을 자리 없다로 댄다 (여는 읽기)");
    }

    /// **띄울 때와 다시 읽을 때 지켜보는 목록이 같다**(리뷰 moai-3lul.kt0 다시 본 판). 다르면 조용한
    /// 저장소에서도 첫 다시 읽기(늦어도 1분 시계)가 그것을 "커밋이 섰다" 로 읽어 커밋 표를 통째로 다시
    /// 짓는다. 여는 길(`cmd::tui`)의 차례 그대로 세운다 — 자리 판정의 표식을 먼저 재고 겹쳐 읽는다.
    #[test]
    fn the_first_quiet_reread_after_launch_keeps_the_commit_table() {
        let s = Scratch::fenced("layer-launch-watched");
        let main = s.project("main", &[("argos-0002", "집은 줄", "in_progress")]);
        let run = |dir: &Path, args: &[&str]| crate::git::tests::run_git(dir, None, args);
        run(&main, &["init", "-q"]);
        run(&main, &["commit", "-q", "--allow-empty", "-m", "a"]);
        run(&main, &["worktree", "add", "-q", "../w1", "-b", "w1"]);
        // 옆에 스냅샷이 있으면 겹치는 읽기는 그 `.git` 을 안 재고, 자리 판정의 표식만 잰다.
        std::fs::create_dir_all(s.join("w1/.moai")).unwrap();
        std::fs::write(s.join("w1/.moai/issues.jsonl"), "").unwrap();
        let repo = Repo::at(main.clone(), crate::config::Config::parse("prefix = \"argos\"\n").unwrap());
        let stamp = stamp_of(&repo);
        let places = crate::worktree::place_marks(&repo.root);
        let mut g = crate::worktree::gather(&repo, true).unwrap();
        super::super::watch(&mut g.watched, places);
        let (index, ground) = super::super::measure(&g.load.issues, &repo.config);
        let mut a = App::open(repo, g.load, index, ground, NavPath::new(), stamp).overlaid(g.origin, g.trouble, g.watched, g.swept, &g.sides);
        let paths: std::collections::BTreeSet<PathBuf> = a.site.watched.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(paths.len(), a.site.watched.len(), "같은 파일을 두 번 지켜본다");
        // 여는 커밋 표는 다 짓게 둔다.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while a.gathering_commits() {
            assert!(std::time::Instant::now() < until, "커밋 표를 다 못 지었다");
            a.follow();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let launched = a.site.watched.clone();

        a.site.read_at = Some(std::time::Instant::now().checked_sub(REREAD_EVERY).expect("시계가 1분도 안 돌았다"));
        a.follow();
        assert!(a.loading(), "1분이 지났는데 다시 안 읽었다");
        settle(&mut a);
        assert_eq!(a.site.watched, launched, "띄울 때와 다시 읽을 때 지켜보는 목록이 갈렸다");
        assert!(!a.gathering_commits(), "아무것도 안 바뀌었는데 커밋 표를 다시 짓는다");
    }
}
