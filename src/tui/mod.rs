//! 탐색기 화면. **쓰기는 [`App::write`] 한 구멍으로만 난다.**
//!
//! 상태([`App`])와 그림([`draw`])을 나눈다. 상태 전이는 터미널 없이 시험되고,
//! 그림은 `TestBackend` 로 시험된다 — 둘 다 TTY 를 켜지 않는다.

pub mod draw;
pub mod edit;
pub mod form;
pub mod input;
pub mod keys;
pub mod layer;
pub mod picker;
pub mod register;
pub mod scroll;

use crate::config::Config;
use crate::model::{Issue, Kind, Status};
use crate::nav::{Entry, Index, Path, Seg};
use crate::query::{Filter, Raw, Where};
use crate::store::{Load, Repo};
use form::{Act, Form};
use input::Input;
use keys::Lookup;
use ratatui::crossterm::event::KeyEvent;
use scroll::{PAGE, Scroll};

/// 목록의 한 줄. `..` 은 이슈가 아니므로 [`Entry`] 로는 못 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 한 층 위로. 뿌리가 아닐 때만 맨 앞에 선다 — MC 와 같다. 프로젝트 층이 있으면
    /// 프로젝트 뿌리에도 서서 층으로 올라간다.
    Up,
    Item(Entry),
    /// 프로젝트 층의 한 줄 — `layer.places` 의 첨자다. 정체는 경로다([`Anchor::Project`]).
    Project(usize),
}

/// 커서가 선 줄의 **정체**. 첨자(`Entry::at`)와 줄 번호는 다시 읽으면 바뀐다 —
/// 커서 위에 줄 하나가 생기거나 칸이 바뀌어 차례가 달라지면 같은 번호가 다른
/// 이슈를 가리키고, 그러면 가만히 보던 커서가 옆 줄로 튄다.
///
/// 이슈는 id 로 붙든다. **중복 id 면 첫 줄로 간다** — 둘 중 어느 쪽인지는 id 로는
/// 못 가르고, `duplicate_id` 는 이미 배너가 말한다. 바구니는 제 줄이 없어 `Seg` 로.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Up,
    Issue(String),
    Bucket(Seg),
    /// 이름이 아니라 경로 — 이름은 등록 목록이 바뀌면 달라진다.
    Project(std::path::PathBuf),
}

/// 무엇을 받고 있는가. 글을 받는 동안에는 이동키가 글자가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Browse,
    /// `/` 로 연 빠른 검색.
    Grep(Input),
    /// `f` 로 연 거름망. **CLI 와 같은 `항목=값` 문법이다.**
    Filter(Input),
    /// 쓰기 앞에서 누군지 묻는 칸. [`App::write`] 만 연다.
    Ask(Ask),
    /// `n` 으로 연 생각 담기 폼(moai-11s4). **어디를 보고 있든 같은 폼이다** — 커서가
    /// 선 에픽에 넣지 않는다. idea 가 소속을 가지면 그것이 이 단계가 없애려던 무게다.
    Idea(Form),
    /// 층의 `a` 가 연 디렉터리 고르기 창(moai-plvy). 프로젝트 안에서도 연다 — 등록이 0 인
    /// 채 `.moai` 안에서 띄우면 층이 없어, 층에서만 열면 첫 등록을 할 길이 없다.
    Pick(picker::Picker),
    /// 층의 `d` 가 묻는 "목록에서 뺄까". `y` 만 뺀다.
    Unregister(register::Unregister),
}

/// 받고 나서 다시 부를 쓰기. **붙잡는 것이 없는 함수다** — 적던 것은 [`Ask`] 가
/// 들고 있다가 되돌려 놓는 모드 안에 있고, 이 함수는 거기서 다시 읽어 쓴다.
/// 닫는 함수(`FnOnce`)를 통째로 들고 있으면 그 결과를 받을 자리가 없어
/// 성공한 뒤의 일(폼 닫기)을 부른 쪽이 두 벌 짓게 된다. 같은 함수를
/// 한 번 더 부르면 그 일이 한 벌로 남는다. 커서 옮기기와 알림은 [`App::write`] 가
/// 스스로 하므로 어느 길로 이어진 쓰기든 같다.
pub type Retry = fn(&mut App);

/// 쓰기가 **어느 줄을** 만들었거나 건드렸나. [`App::write`] 에 넘기는 닫는 함수가
/// 저널과 함께 낸다 — `with_write` 의 `(저널, T)` 에서 `T` 자리다.
///
/// **id 는 닫는 함수만 안다.** 새 id 는 락 안에서 다시 읽은 목록을 보고 고르므로
/// 밖에서 짐작할 수 없고, 쓰고 난 목록을 견줘 "새로 생긴 줄" 을 찾으면 같은 틈에
/// 남이 쓴 줄과 갈리지 않는다. 그래서 쓰는 쪽이 댄다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touched {
    /// 쓰고 나면 커서가 설 줄.
    pub id: String,
    /// 알림의 앞말 — `담김`. **부르는 쪽이 정한다**: 쓰기의 뜻(담기·고치기·옮기기)을
    /// 여기서 저널을 보고 가르면 필드 고치기는 저널을 안 남겨(CLAUDE.md) 이름이 없다.
    pub done: &'static str,
}

/// 쓴 줄이 목록 어디에 섰나. 알림이 무엇을 덧붙일지를 가른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// 그 줄에 커서가 섰다.
    Shown,
    /// 줄은 있는데 거름망이 가렸다. 커서는 옮기지 않는다.
    Hidden,
    /// 다시 읽은 목록에 그 id 가 없다 — 다시 읽기가 실패했거나 지운 쓰기다.
    Missing,
}

/// 누군지 묻는 칸의 상태.
///
/// **받은 것은 그 세션 동안만 든다**(`App::user`). `.moai/config.toml` 에도
/// `git config` 에도 적지 않는다 — 도구가 남의 설정을 쓰는 순간 그건 설정이 아니라
/// 마이그레이션이다(moai-nmv2 사용자 결정). 대신 칸 위에 `git config` 로 적어 두면
/// 다시 안 묻는다고 댄다.
#[derive(Debug, Clone)]
pub struct Ask {
    pub input: Input,
    /// 마지막 Enter 가 거절된 까닭. **치는 동안에는 나무라지 않는다** — `이름 (메일)`
    /// 은 닫는 괄호를 치기 전까지 늘 모양이 아니라, 거름망처럼 그 자리에서 판정하면
    /// 적는 내내 빨간 줄이 선다. 칸이 키를 먹으면 걷힌다.
    pub error: Option<String>,
    /// 왜 묻는가 — 쓰기를 멈춘 거절문의 첫 줄 그대로. **없는 것과 틀린 것이 같은 코드다**
    /// (`NO_ACTOR`): git 설정이 있는데 메일에 `@` 가 없는 사람에게 "모른다" 고만 하면
    /// 이미 적은 설정의 무엇이 틀렸는지 영영 모른다. 코드를 가르지 않는 것은 그쪽이
    /// 일부러 하나로 묶은 것이라서다(`model::bad_git_identity`) — 여기서는 말을 옮긴다.
    why: String,
    /// 묻기 전에 받던 것 — 적던 폼. **Esc 든 받든 여기로 돌아간다.** 한 키에 적던
    /// 것이 날아가면 다음부터 안 쓴다.
    back: Box<Mode>,
    then: Retry,
}

/// **다시 부를 함수는 견주지 않는다.** 함수 주소는 같은 함수라도 코드 조각마다
/// 갈라질 수 있어 `==` 이 뜻을 못 진다(`unpredictable_function_pointer_comparisons`).
/// 견주는 쪽은 시험의 `assert_eq!(a.mode, ..)` 뿐이고, 거기서 보는 것은 칸의 글이다.
impl PartialEq for Ask {
    fn eq(&self, other: &Ask) -> bool {
        self.input == other.input && self.error == other.error && self.back == other.back
    }
}

impl Eq for Ask {}

/// 어느 칸이 이동키를 먹는가. **화면에 칸이 늘면 여기가 는다** — 순환은
/// [`Pane::ALL`] 의 차례를 따르므로 새 칸은 거기 한 자리를 얻는 것으로 끝난다.
///
/// 포커스가 없던 때는 왼쪽 목록이 늘 `↑↓` 를 먹고 오른쪽은 `j`/`k` 로만 굴렀다.
/// 한 화면에 이동키가 두 벌이면 칸이 하나 더 생길 때마다 세 벌째를 지어야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// 왼쪽 목록. **여기서 시작한다** — 여는 순간 `↓` 가 목록을 내려가던 손을
    /// 배신하지 않는다.
    #[default]
    Explorer,
    /// 오른쪽 상세.
    Detail,
}

impl Pane {
    /// `Tab` 이 도는 차례.
    pub const ALL: [Pane; 2] = [Pane::Explorer, Pane::Detail];

    fn at(self) -> usize {
        Pane::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// 다음 칸. 끝에서 처음으로 돈다.
    pub fn next(self) -> Pane {
        Pane::ALL[(self.at() + 1) % Pane::ALL.len()]
    }

    /// 앞 칸. 처음에서 끝으로 돈다.
    pub fn prev(self) -> Pane {
        Pane::ALL[(self.at() + Pane::ALL.len() - 1) % Pane::ALL.len()]
    }
}

/// 묶음 id → 멤버에서 읽은 것(`report::group_stands`). 이슈를 빌리지 않게 소유한다.
type States = std::collections::BTreeMap<String, Stood>;

/// 묶음 하나를 읽은 것 — 서 있는 칸과, 그 밑에 집은 일이 있는가(`report::Stand::busy`).
struct Stood {
    column: String,
    busy: bool,
}

fn states_of(issues: &[Issue], cfg: &Config) -> States {
    crate::report::group_stands(issues, cfg)
        .into_iter()
        .map(|(id, s)| (id.to_string(), Stood { column: s.column.to_string(), busy: s.busy }))
        .collect()
}

/// 다시 읽은 것 — **무거운 셈을 다 마친 모양.** 읽기·색인·칸 지도·경고 셈은
/// 이슈 수에 비례해, 1만 개에서 한 번에 수백 ms 다. 그것을 그리는 루프에서 하면
/// 에이전트가 몰아 쓰는 동안 탐색기가 걸음마다 멈칫한다. 그래서 루프 밖 스레드에서
/// 이 모양까지 짓고, 루프는 커서·거름망만 맞춘다([`App::apply_fresh`]).
///
/// 여기 드는 셈은 전부 `&[Issue]` 에 대한 순수 함수라 스레드로 옮길 수 있다 —
/// `report`·`query` 를 순수하게 둔 계약이 여기서 값을 한다.
pub struct Fresh {
    /// 어느 프로젝트를 읽었나 — 받는 쪽이 지금 프로젝트와 견준다([`App::receive`]).
    root: std::path::PathBuf,
    stamp: Stamp,
    issues: Vec<Issue>,
    index: Index,
    states: States,
    warnings: usize,
    unreadable: Vec<Option<String>>,
    origin: crate::worktree::Origin,
    elsewhere: Vec<String>,
    watched: Vec<(std::path::PathBuf, Stamp)>,
    now: String,
}

/// 버린 다시 읽기 손잡이를 이만큼까지 든다(`App::discarded`). 버리는 것은 사람의
/// 손(F5·`w`·쓰기)이 읽기가 도는 동안 닿을 때뿐이고, 한 읽기는 1만 개에서도 수백 ms
/// 라 보통은 하나도 안 쌓인다. 이것이 차는 것은 읽기가 멈춘 때뿐이다.
const DISCARDED_KEPT: usize = 8;

/// 저장소를 읽어 [`Fresh`] 를 짓는다. **어느 스레드에서 불러도 같다.**
fn prepare(repo: &Repo, worktree: bool) -> crate::fail::R<Fresh> {
    // 읽기 **전에** 잰다. 뒤에 재면 읽고 재는 사이의 쓰기를 놓치고, 놓친
    // 것은 영영 안 돌아온다. 먼저 재면 최악이 헛 갱신 하나다.
    let stamp = stamp_of(repo);
    let g = crate::worktree::gather(repo, worktree)?;
    // 옆에서만 온 줄과 겹친 id 는 중복으로 세지 않는다 (`Origin::unreadable`).
    let unreadable: Vec<Option<String>> = g
        .origin
        .unreadable(g.load.errors.iter().map(|e| e.id.as_deref()))
        .into_iter()
        .map(|id| id.map(str::to_string))
        .collect();
    let issues = g.load.issues;
    let now = crate::model::now();
    Ok(Fresh {
        root: repo.root.clone(),
        stamp,
        index: Index::of(&issues),
        states: states_of(&issues, &repo.config),
        warnings: warnings_of(&issues, &unreadable, &repo.config, &now),
        issues,
        unreadable,
        origin: g.origin,
        elsewhere: g.trouble,
        watched: g.watched,
        now,
    })
}

/// **알림은 안 센다.** 배너는 "드러난 것 N건" 이라고 말하는데, 담아 둔
/// 생각이 쌓였다는 알림을 거기 더하면 생각을 담을수록 화면이 고쳐야 할
/// 것이 늘었다고 말한다 — 그러면 안 담게 된다. 무엇이 알림인지는
/// `report` 가 `notices` 로 따로 내므로 여기서 다시 판단하지 않는다.
fn warnings_of(issues: &[Issue], unreadable: &[Option<String>], cfg: &Config, now: &str) -> usize {
    // 못 읽는 줄의 id 까지 넘긴다 — 산 줄과의 중복을 `moai status` 와 같은
    // 자로 센다.
    let lines: Vec<crate::report::Unreadable> =
        unreadable.iter().map(|id| crate::report::Unreadable { id: id.as_deref() }).collect();
    // 알림은 `notices` 에 따로 있다 — `warnings` 가 곧 고칠 것이다.
    crate::report::status(issues, &lines, cfg, now).warnings.len()
}

pub struct App {
    pub issues: Vec<Issue>,
    pub index: Index,
    /// 묶음 id → 멤버에서 읽은 칸과 집은 멤버가 있는가 (`report::group_stands`). **적재 때 한 번 센다**
    /// — 프레임마다 세면 줄 하나 그리는 데 저장소를 걷는다.
    states: States,
    pub cfg: Config,
    pub path: Path,
    pub cursor: usize,
    pub mode: Mode,
    /// 이동키(`↑↓`·PageUp/Down·Home/End)를 먹는 칸. `Tab`·`Shift-Tab` 이 돌린다.
    /// **다시 읽어도 그대로다** — 굴리던 사람의 손이 갱신 한 번에 목록으로 튀면 안 된다.
    pub focus: Pane,
    /// 지금 걸린 거름망. 사람이 적은 글 그대로도 들고 있어야 화면에 되비친다.
    pub filter_text: Option<String>,
    /// 어디서 읽어 왔나. 시험은 저장소 없이 App 을 세우므로 없을 수 있다.
    pub repo: Option<Repo>,
    /// 읽은 그 순간의 시각. **프레임마다가 아니라 적재마다 잡는다** — 매번
    /// 다시 잡으면 "3일 넘게" 같은 판정이 초 단위로 깜빡인다.
    pub now: String,
    /// 읽다 만난 못 읽는 줄 — 줄마다 **그 줄이 쓰는 id** (읽어 낼 수 있었던
    /// 것만). 대체 화면 안에서는 stderr 로 못 알린다. 수를 따로 들지 않는다 —
    /// 둘로 들면 어긋날 수 있고, 산 줄과의 중복을 `moai status` 와 같은 자로
    /// 세려면 수만으로는 모자란다(moai-4dk4).
    pub unreadable: Vec<Option<String>>,
    /// 마지막 갱신이나 쓰기가 **실패한** 까닭 — 무엇을 못 했는지까지 단 쪽이 적는다.
    /// 조용히 삼키면 F5 가 아무 일도 안 하는데 "바뀌었다" 배너는 붙어 있어, 사람은
    /// 누르고 또 누르며 까닭을 못 얻는다.
    pub trouble: Option<String>,
    /// `trouble` 이 **쓰기의 실패**인가. 그렇다면 다시 읽기가 성공해도 걷지 않는다 —
    /// 락을 못 잡은 때가 곧 남이 쓰고 있던 때라 바로 다음 걸음이 다시 읽고, 그 읽기가
    /// 까닭을 지우면 폼은 열린 채인데 왜 안 닫혔는지가 화면 어디에도 없다. 걷는 것은
    /// 다음에 성공한 쓰기나, 그 자리를 덮는 읽기의 실패다.
    write_failed: bool,
    /// 방금 성공한 쓰기의 한 줄 알림 — `✓ 담김 · moai-xxxx`. **다음 키 하나에 걷힌다**
    /// ([`App::key`]). 지나가는 말이라 붙박이로 두면 오래전 쓰기를 방금 한 것처럼 말하고,
    /// 다시 읽기로 걷으면 남이 쓴 걸음 하나에 읽기도 전에 사라진다.
    ///
    /// `trouble` 과 **따로 든다.** 쓰기 뒤 다시 읽기가 실패하면 둘이 함께 참이다 —
    /// 파일에는 담겼고 화면은 못 읽었다. 한 칸에 담으면 어느 한쪽이 거짓말을 한다.
    pub notice: Option<String>,
    /// 사용자 설정을 못 읽어 **층을 안 세운** 까닭. 층이 서면 층이 제 `problems` 를 대므로
    /// 층이 없을 때만 든다. 붙박이다 — 다시 읽기가 걷는 `trouble` 에 두면 700ms 뒤에
    /// 사라져 사람은 층이 왜 없는지 끝내 모른다. F5 로 설정을 다시 읽어 층이 서면 걷힌다.
    pub unlayered: Option<String>,
    /// `--user` 로 **준 값 그대로**(`Ctx::user` 와 같다), 또는 누군지 묻는 칸에서
    /// 받은 것([`Mode::Ask`]). 쓸 때마다 `model::actor` 로 푼다 — 미리 풀어 두면
    /// 설정 없는 기계에서 읽기만 하려던 탐색기가 여는 순간 사람을 묻는다. 읽기는
    /// 묻지 않는다. **받은 것은 이 세션이 끝나면 사라진다** — 어디에도 적지 않는다.
    pub user: Option<String>,
    /// 누가 쓰는가를 푸는 길. 진짜 길은 `model::actor` 다. **시험이 갈아 끼운다** —
    /// 그쪽은 `MOAI_ACTOR` 와 이 기계의 git 설정을 읽어, 갈아 끼우지 않으면
    /// "누군지 모를 때" 를 시험한 결과가 돌리는 사람의 설정에 달린다.
    identify: fn(Option<&str>) -> crate::fail::R<crate::model::Actor>,
    /// `moai status` 가 드러낼 것의 수. 자세한 화면은 나중에 얹는다.
    pub warnings: usize,
    /// 상세의 굴린 자리. **왼쪽 커서를 옮기면 첫 줄로 돌아간다** — 다른
    /// 이슈를 보는데 굴린 자리가 남아 있으면 첫 줄부터 못 본다.
    pub detail: Scroll,
    /// 본문을 그리지 않고 원문 그대로 보는가. 그린 글은 기호가 지워져
    /// 되돌릴 수 없다 — 긁어 붙이거나 마크다운을 고칠 때 이 길이 필요하다.
    pub raw: bool,
    /// 에픽·마일스톤 줄에 진행 바탕을 까는가(moai-94uf). **세션만 든다, 켜진 채로 시작한다** —
    /// 포커스 없이 여러 묶음의 진척을 한눈에 보는 것이 이 탐색기의 쓸모라서다.
    pub shade: bool,
    /// 마지막으로 읽은 파일의 (고친 때, 길이).
    stamp: Stamp,
    /// 겹쳐 보는 동안 함께 지켜보는 옆 워크트리 스냅샷과 그 표식(`worktree::gather`
    /// 가 읽기 **전에** 잰 것). 꺼져 있으면 비었다.
    watched: Vec<(std::path::PathBuf, Stamp)>,
    /// 스레드에서 짓고 있는 다시 읽기. 끝나면 [`App::follow`] 가 받아 들인다.
    /// 손잡이는 스레드가 죽었을 때 그 패닉을 루프로 되던지려고 든다.
    pending: Option<(std::sync::mpsc::Receiver<crate::fail::R<Fresh>>, std::thread::JoinHandle<()>)>,
    /// 버린 다시 읽기(F5·`w`·쓰기가 `pending` 을 버렸을 때)의 손잡이. 결과는 안 받지만
    /// **패닉은 받는다** — ratatui 의 패닉 훅은 어느 스레드에서 나든 터미널을 걷으므로,
    /// 손잡이를 같이 버리면 루프가 걷힌 화면에 모른 채 그린다. [`App::follow`] 가
    /// 걸음마다 끝난 것을 join 해 패닉이면 되던진다. [`DISCARDED_KEPT`] 개까지 든다.
    discarded: Vec<std::thread::JoinHandle<()>>,
    /// [`DISCARDED_KEPT`] 를 넘겨 **아직 도는 채로 놓은** 손잡이 수. 그 스레드가 터지면
    /// 터미널이 걷히는데 되던질 길이 없다 — 화면이 그것을 말한다(`draw::banner`).
    /// **붙박이다.** `trouble` 은 다음에 성공한 다시 읽기가 걷는데, 놓는 때가 곧 F5·`w`·
    /// 쓰기가 새 읽기를 띄운 때라 몇백 ms 뒤에 사라진다. 놓은 스레드는 다시 볼 길이
    /// 없으므로 세션 내내 남긴다.
    let_go: usize,
    /// 다시 읽는 길. 진짜 길은 [`prepare`] 다. **시험이 갈아 끼운다** — 스레드에서
    /// 짓는 읽기가 패닉하는 때는 진짜 파일로는 못 만든다.
    read: fn(&Repo, bool) -> crate::fail::R<Fresh>,
    /// 이슈 첨자 → 걸렸는가. **거름망이 바뀔 때만 다시 센다** — 매 프레임
    /// `Filter::matches` 를 돌리면 `Where::of` 가 프레임마다 지도를 다시 만든다.
    keep: Vec<bool>,
    /// 층마다 커서를 기억한다. 들어갔다 나오면 **있던 자리로 돌아온다** —
    /// 매번 맨 위로 튕기면 형제 여럿을 훑는 일이 못 할 짓이 된다.
    remembered: Vec<usize>,
    /// 목록이 훑고 있는 자리. **프레임을 넘어 산다** — 매 프레임 새로 만들면
    /// 0 번 줄부터 다시 세어 커서를 늘 맨 아랫줄에 붙이고, 그러면 커서 아래를
    /// 한 줄도 못 본다. 상세와 **같은 조각**이다([`Scroll`]).
    pub list: Scroll,
    pub quit: bool,
    /// 도는 글리프의 걸음. **그린 횟수가 아니라 시계가 올린다** — 그릴 때마다
    /// 올리면 키를 누르는 동안에는 타이핑 속도로 돌고 가만히 두면 파일을 보러
    /// 깨는 걸음(700ms)으로 느려진다. 둘 다 "도는 것" 으로 읽히지 않는다.
    /// 올리는 것은 `cmd::tui` 의 루프 하나뿐이고, 그래서 한 프레임 안의
    /// 목록·상세·롤업이 같은 걸음을 본다.
    pub spin: usize,
    /// 다른 워크트리를 겹쳐 보는가. `w` 가 켜고 끈다 — CLI 의 `--worktree` 와 같은
    /// 길(`worktree::gather`)로 읽는다. **꺼진 채로 시작한다**: 켜면 `git` 을 부르고
    /// 옆 파일을 읽는데, 시키지 않은 일을 여는 순간마다 하면 탐색기가 무거워진다.
    pub worktree: bool,
    /// 겹쳐 본 줄의 출처. 꺼져 있으면 비었다.
    pub origin: crate::worktree::Origin,
    /// 옆 워크트리에서 만난 문제. **배너로 말만 한다** — CLI 가 stderr 로 흘리는
    /// 말인데, 대체 화면 안에서는 그 길을 못 쓴다.
    pub elsewhere: Vec<String>,
    /// 프로젝트 층([`layer`]). **`None` 이면 등록한 것이 없고 오늘 탐색기 그대로다.** 층이
    /// 있으면 지금 선 곳(`layer.at`)이 층이거나 한 프로젝트 안이고, 층에 선 동안에는 위의
    /// 한 프로젝트 자리(`repo`·`issues`·`index`…)가 비었다.
    pub layer: Option<layer::Layer>,
    /// 사용자 설정 파일의 자리 — **층이 없을 때** 등록(`a`)이 쓰는 곳이다. 층이 있으면 층이 읽은
    /// 파일(`Layer::config`)을 쓴다. `cmd::tui` 가 `user_config::path()` 로 넣고, 시험은 임시
    /// 파일을 준다 — 여기서 환경을 읽으면 시험이 돌리는 사람의 설정을 고친다.
    pub user_config: Option<std::path::PathBuf>,
    /// **띄운 자리**(cwd). 고르기 창이 처음 여기서 연다. 시험은 임시 디렉터리를 준다.
    ///
    /// 이름이 [`App::here`] 와 겹치지 않게 둔다 — 그쪽은 *지금 선 프로젝트*, 곧 **쓰기가
    /// 닿는 곳**이다. 둘 다 `Option<PathBuf>` 라 괄호 하나를 빠뜨려도 컴파일되고, 그러면
    /// 생각 담기가 머리에 보인 곳 말고 띄운 자리에 쓰려 든다.
    pub launched_at: Option<std::path::PathBuf>,
    /// 고르기 창을 마지막으로 닫은 디렉터리 — 다시 열면 여기서 연다.
    pick_from: Option<std::path::PathBuf>,
}

impl App {
    /// 저장소 없이 세운다 — 시험과 눈으로 보는 길이 이것을 쓴다. 진짜 길은
    /// [`App::open`] 이고, 그쪽은 색인을 부른 쪽에서 받는다.
    #[cfg(test)]
    pub fn new(issues: Vec<Issue>, cfg: Config, path: Path) -> App {
        let index = Index::of(&issues);
        App::build(issues, index, cfg, path, Vec::new())
    }

    /// 저장소에서 읽어 세운다.
    ///
    /// **색인은 부른 쪽이 이미 만든 것을 받는다** — 여는 데서 다시 만들면
    /// 같은 훑기를 두 번 하고, 그 훑기는 이슈 수에 비례한다.
    /// **표식도 부른 쪽이 읽기 전에 잰 것을 받는다** — 읽고 나서 재면 그
    /// 사이에 떨어진 쓰기가 "이미 본 것" 으로 적혀 영영 안 보인다.
    pub fn open(repo: Repo, load: Load, index: Index, path: Path, stamp: Stamp) -> App {
        let cfg = repo.config.clone();
        let ids = load.errors.iter().map(|e| e.id.clone()).collect();
        let mut app = App::build(load.issues, index, cfg, path, ids);
        app.stamp = stamp;
        app.repo = Some(repo);
        app
    }

    fn build(
        issues: Vec<Issue>,
        index: Index,
        cfg: Config,
        path: Path,
        unreadable_ids: Vec<Option<String>>,
    ) -> App {
        // 들어간 채로 시작하면(`--path`) 나올 층마다 기억 자리를 만들어 둔다.
        let remembered = vec![0; path.len()];
        let keep = vec![true; issues.len()];
        let states = states_of(&issues, &cfg);
        let mut app = App {
            issues,
            index,
            states,
            cfg,
            path,
            cursor: 0,
            mode: Mode::Browse,
            focus: Pane::default(),
            detail: Scroll::default(),
            raw: false,
            shade: true,
            filter_text: None,
            repo: None,
            now: crate::model::now(),
            unreadable: unreadable_ids,
            trouble: None,
            unlayered: None,
            write_failed: false,
            notice: None,
            user: None,
            identify: crate::model::actor,
            warnings: 0,
            stamp: None,
            watched: Vec::new(),
            pending: None,
            discarded: Vec::new(),
            let_go: 0,
            read: prepare,
            keep,
            remembered,
            list: Scroll::default(),
            quit: false,
            spin: 0,
            worktree: false,
            origin: crate::worktree::Origin::default(),
            elsewhere: Vec::new(),
            layer: None,
            user_config: None,
            launched_at: None,
            pick_from: None,
        };
        // 한 번만 센다. `report::status` 는 이슈 수에 비례한 훑기라, 못 읽는 줄
        // 수를 나중에 넣겠다고 두 번 부르면 그 절반이 버려진다.
        app.warnings = warnings_of(&app.issues, &app.unreadable, &app.cfg, &app.now);
        app
    }

    /// 다시 읽는다. **거름망과 있던 자리는 지키려 애쓴다** — 갱신 한 번에
    /// 하던 일이 흩어지면 F5 를 안 누르게 되고, 그러면 낡은 화면을 본다.
    ///
    /// **사람이 누른 갱신(F5·`w`)은 그 자리에서 읽는다.** 누른 사람은 결과를 기다리고
    /// 있고, `w` 는 켠 뜻대로 읽힌 화면이 곧바로 서야 한다. 스레드에서 짓던 것이
    /// 있으면 버린다 — 누르기 **전에** 시작한 읽기라 늦게 도착하면 방금 읽은 것을
    /// 옛 것으로 덮는다(`w` 를 끄기 전 설정으로 읽은 것이면 더더욱).
    pub fn reload(&mut self) {
        // 층에서 누른 F5 는 사용자 설정부터 다시 읽고 프로젝트를 다시 연다.
        if self.on_layer() {
            self.reread_layer();
            return;
        }
        if self.repo.is_none() {
            return;
        }
        if let Some((_, handle)) = self.pending.take() {
            self.discard(handle);
        }
        let Some(repo) = &self.repo else { return };
        let fresh = (self.read)(repo, self.worktree);
        self.receive(fresh);
    }

    /// 다시 읽은 결과를 받는다. **소리 없이 넘기지 않는다** — 실패를 삼키면 갱신이
    /// 아무 일도 안 하는데 사람은 까닭을 못 얻는다. 실패하면 표식을 안 올리므로
    /// 다음 걸음에 다시 해 본다.
    ///
    /// **다른 프로젝트에서 지은 것은 버린다.** 프로젝트를 옮길 때 도는 읽기를 이미
    /// 버리지만([`App::discard`]), 받는 자리에서 한 번 더 뿌리를 견준다 — 떠난
    /// 프로젝트의 줄이 지금 프로젝트의 화면으로 들어오면 같은 id 가 엉뚱한 줄을
    /// 가리키고, 그 위에서 쓰면 쓰는 곳은 맞는데 보고 쓴 것이 틀린다.
    fn receive(&mut self, fresh: crate::fail::R<Fresh>) {
        match fresh {
            Ok(f) if self.repo.as_ref().is_none_or(|r| r.root != f.root) => {}
            Ok(f) => self.apply_fresh(f),
            Err(e) => {
                self.trouble = Some(format!("다시 읽지 못했다 — {e}"));
                self.write_failed = false;
            }
        }
    }

    /// 탐색기가 `issues.jsonl` 을 바꾸는 **유일한 길.** 모든 쓰기가 여기를 지난다.
    ///
    /// CLI 와 같은 [`Repo::with_write`] 를 부른다 — 쓰기 경로가 화면 쪽에 따로
    /// 서면 락·재읽기·검증·원자적 교체를 또 반쯤 구현하게 되고, 옛 `write.rs` 가
    /// 그렇게 부풀었다. 닫는 함수가 받는 목록은 **락 안에서 다시 읽은 것**이다.
    /// 화면이 들고 있는 `self.issues` 는 낡았을 수 있으니 그것을 보고 판단하지 않는다.
    ///
    /// **쓰고 나면 [`App::reload`] 로 다시 읽는다.** 만든 줄을 `self.issues` 에 손으로
    /// 넣으면 그 순간 화면과 파일이 갈라진다 — 정규화·정렬·묶음의 칸·경고 셈이
    /// 파일 쪽에만 걸린다. 다시 읽기가 표식도 함께 잡으므로(`prepare` 는 읽기 전에
    /// 잰다) 제가 쓴 것을 "밖에서 바뀌었다" 로 읽어 한 번 더 읽는 일이 없다. 스레드에서
    /// 짓던 읽기는 버린다 — 쓰기 **전에** 띄운 것이라 늦게 닿으면 방금 쓴 것을
    /// 옛 화면으로 덮는다(`reload` 가 이미 그렇게 한다). **다시 읽기는 이 한 번이다** —
    /// 한 키에 목록을 여러 번 다시 세던 것을 걷어낸 판단(moai-cf1t)을 되돌리지 않는다.
    ///
    /// **다시 읽고 나면 쓴 줄에 선다**([`Touched`], [`App::land`]). 담긴 것이 눈앞에
    /// 보여야 담긴 줄 안다 — 사람이 방금 담은 것을 확인하러 헤매면 다음부터 안 담는다.
    /// 그 줄이 다른 디렉터리에 서면 그리로 간다(idea 는 에픽에 안 들어가므로 에픽 안에서
    /// 담으면 뿌리에 선다). 한 줄 알림(`notice`)이 만든 id 를 댄다. **거름망이 그 줄을
    /// 가리면 커서는 두고 그렇다고 말한다** — 조용히 안 보이면 저장이 실패한 것으로
    /// 읽힌다. 거름망을 대신 풀지는 않는다: 사람이 건 것이고, 푸는 키는 알림이 댄다.
    /// 돌려주는 것은 그 id 다.
    ///
    /// **실패를 삼키지 않는다.** 대체 화면 안에서는 stderr 가 안 보이므로 `trouble`
    /// 로 말하고 `None` 을 낸다 — 부른 쪽(폼)은 그것을 보고 적던 것을 닫지 않는다.
    /// 실패하면 **다시 읽지 않는다**: 파일은 그대로다. 그 뒤 저절로 다시 읽어도 까닭은
    /// 남는다(`write_failed`) — 다음 쓰기가 성공할 때 걷힌다.
    ///
    /// **누군지 모르면 락을 잡기 전에 멈추고 묻는다**([`Mode::Ask`]). 이름 없는 줄을
    /// 적느니 한 번 묻는다는 규약이다. 받으면 `user` 에 들고, **적던 모드를 되돌려
    /// 놓은 뒤** `retry` 를 부른다 — 그래서 `retry` 는 이 쓰기를 부른 그 함수다. 묻는
    /// 동안은 `None` 이다(아직 안 썼다). 묻는 것은 `NO_ACTOR` 일 때 — `--user`·
    /// `MOAI_ACTOR`·git 설정이 모두 없거나, **git 설정이 있어도 모양이 틀렸을 때**다.
    /// 뒤의 것도 묻는 까닭은 고칠 곳이 이 화면 밖의 설정이라서다: 이번 세션의 이름을
    /// 받으면 나가지 않고 쓸 수 있고, 무엇이 틀렸는지는 칸 위에 거절문 그대로 선다
    /// ([`Ask`] 의 `why`). `--user`·`MOAI_ACTOR` 로 **준 값**의 모양이 틀린 것은
    /// 사람이 준 것을 조용히 갈아 치울 일이 아니라 배너로 말한다(`BAD_INPUT`).
    ///
    /// **동기다.** 로컬 파일 하나라 짧고, 그동안 화면은 멈춘다. 비동기 런타임을
    /// 들이면 CLI 전체가 async 로 물든다. 락을 못 잡으면 `with_write` 가 5초 뒤
    /// 아무것도 안 쓰고 물러나고, 그 말이 그대로 화면에 선다.
    #[must_use = "None 이면 쓰지 못했다 — 적던 것을 닫으면 사람이 적은 것을 잃는다"]
    pub fn write(
        &mut self,
        retry: Retry,
        f: impl FnOnce(
            &mut Vec<Issue>,
            &Config,
            &std::collections::BTreeSet<String>,
            &crate::model::Actor,
        ) -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)>,
    ) -> Option<String> {
        // 앞 쓰기의 알림은 이 쓰기가 무엇이 되든 낡았다 — 실패한 뒤에 옛 `✓` 가 남으면
        // 이번 것이 담긴 것으로 읽힌다.
        self.notice = None;
        let Some(repo) = &self.repo else {
            self.trouble = Some("쓰지 못했다 — 저장소 없이 연 화면이다".into());
            self.write_failed = true;
            return None;
        };
        let by = (self.identify)(self.user.as_deref());
        if let Err(e) = &by
            && e.code == crate::fail::code::NO_ACTOR
        {
            let back = std::mem::replace(&mut self.mode, Mode::Browse);
            let why = e.message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
            self.mode = Mode::Ask(Ask { input: Input::default(), error: None, why, back: Box::new(back), then: retry });
            return None;
        }
        let written = by.and_then(|by| repo.with_write(|issues, cfg, reserved| f(issues, cfg, reserved, &by)));
        match written {
            Ok(touched) => {
                self.write_failed = false;
                self.reload();
                let Touched { id, done } = touched;
                // 층이 있으면 **어느 프로젝트에** 담겼는지도 댄다 — 같은 id 가 두 프로젝트에 있을 수
                // 있어 id 만으로는 어디인지 모른다(moai-fccv). 층이 없으면 프로젝트는 하나뿐이다.
                let what = match self.project() {
                    Some(p) => format!("{} · {id}", crate::text::one_line(&p.name)),
                    None => id.clone(),
                };
                let told = match self.land(&id) {
                    Landing::Shown => format!("✓ {done} · {what}"),
                    Landing::Hidden => format!(
                        "✓ {done} · {what} — 거름망에 가려 안 보인다 · {} 로 푼다",
                        keys::label(keys::BROWSE, keys::Browse::ClearFilter)
                    ),
                    // 다시 읽기가 실패했으면 그 까닭은 `trouble` 이 따로 댄다. 담긴 것은 참이다.
                    Landing::Missing => format!("✓ {done} · {what} — 다시 읽은 목록에 없다"),
                };
                self.notice = Some(told);
                Some(id)
            }
            // 거절문은 여러 줄일 수 있다(누군지 모를 때는 고칠 명령까지 낸다). 배너는
            // 한 줄이라 줄바꿈이 그대로 가면 그림이 찢어진다 — 한 줄로 잇는다.
            Err(e) => {
                let why: Vec<&str> = e.message.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                self.trouble = Some(format!("쓰지 못했다 — {}", why.join("  ")));
                self.write_failed = true;
                None
            }
        }
    }

    /// 스레드에서 짓고 있는 다시 읽기가 있는가. 루프가 이 동안은 더 자주 깨어 받는다.
    pub fn loading(&self) -> bool {
        self.pending.is_some() || self.layer_loading()
    }

    /// 버린 다시 읽기 스레드가 아직 남았는가. 루프는 이 동안에도 빠른 걸음으로 깬다 —
    /// 그 스레드가 패닉하면 터미널은 이미 걷혔고, 느린 걸음(700ms)으로 깨면 그동안
    /// 걷힌 화면에 그린다. [`App::loading`] 과 가르는 것은 **새 읽기를 막지 않기**
    /// 때문이다 — 버린 것은 받을 것이 아니다.
    pub fn reaping(&self) -> bool {
        !self.discarded.is_empty()
    }

    /// 결과는 버리되 손잡이는 든다 — 그 스레드의 패닉을 다음 걸음이 되던진다.
    /// 다시 읽기(`reload`)와 프로젝트 옮기기가 도는 읽기를 버리는 길이다.
    fn discard(&mut self, handle: std::thread::JoinHandle<()>) {
        if self.discarded.len() >= DISCARDED_KEPT {
            // 놓기 **전에** 끝난 것부터 거둔다. 지난 걸음에 살아 있던 것도 그새 끝났을
            // 수 있고, 그것이 가장 오래된 자리에 있으면 패닉째 놓게 된다.
            self.reap();
        }
        if self.discarded.len() >= DISCARDED_KEPT {
            // 이만큼 안 끝났으면 읽기가 멈춘 것이다(느린 원격 디스크 따위). 기다리면
            // 루프가 같이 멈추므로 가장 오래된 것을 놓는다 — 그 하나만 1b63abe 이전
            // 처지로 돌아간다. **놓기 직전에 한 번 더 본다** — 거둔 뒤 그새 끝났으면
            // 놓을 까닭이 없고, 패닉이면 여기서 되던진다. 그래도 돌면 놓고 센다.
            let oldest = self.discarded.remove(0);
            if oldest.is_finished() {
                if let Err(payload) = oldest.join() {
                    std::panic::resume_unwind(payload);
                }
            } else {
                self.let_go += 1;
            }
        }
        self.discarded.push(handle);
    }

    /// 버린 스레드 중 끝난 것을 join 한다. **패닉이면 되던진다** — [`App::follow`] 가
    /// 받은 스레드에 하는 것과 같은 끝이다. 끝나지 않은 것은 기다리지 않는다.
    fn reap(&mut self) {
        if self.discarded.is_empty() {
            return;
        }
        let (done, alive): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.discarded).into_iter().partition(|h| h.is_finished());
        self.discarded = alive;
        for handle in done {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
        }
    }

    /// 지어 온 것을 들인다. 여기서 하는 셈은 커서·경로·거름망뿐이다.
    fn apply_fresh(&mut self, f: Fresh) {
        if !self.write_failed {
            self.trouble = None;
        }
        self.stamp = f.stamp;
        self.unreadable = f.unreadable;
        self.origin = f.origin;
        self.elsewhere = f.elsewhere;
        self.watched = f.watched;
        self.warnings = f.warnings;
        self.take(f.issues, f.index, f.states, f.now);
    }

    /// 돌 것이 한 줄이라도 있는가. **화면에 보이는지까지는 따지지 않는다** —
    /// 보이는 줄만 가리려면 루프가 그림의 결과를 알아야 하고 그 값은 그린 뒤에야
    /// 나온다. 틀리는 쪽은 "있는데 안 보인다" 하나뿐이고, 그때 손해는 안 보이는
    /// 것을 위해 걸음을 재는 것이다. 반대쪽은 안 틀린다 — 화면에 도는 글리프가
    /// 있으면 그 이슈는 `issues` 에 있으므로 여기가 참이다. 스피너를 그려 놓고
    /// 아무도 안 깨우는 조합은 그래서 못 생긴다.
    ///
    /// **줄마다 묻는 자는 [`App::spins`] 하나다** — 그리는 쪽(`draw::glyph_of`)과 깨우는
    /// 쪽이 따로 판단하면 한쪽만 고쳐져, 도는데 안 깨우거나 안 도는데 깨운다.
    pub fn spinning(&self) -> bool {
        (0..self.issues.len()).any(|at| self.spins(at))
    }

    /// 그 줄이 도는가 — **지금 누가 손대고 있는 줄**이다.
    ///
    /// 일은 제 칸이 도는 칸이면 돈다. **묶음은 읽은 칸이 도는 칸이고, 그 밑에 집은 일이
    /// 실제로 있을 때만** 돈다(`report::Stand::busy`). 읽은 칸만 보면 멤버 하나가 끝나고
    /// 나머지가 `todo` 이기만 해도 `in_progress` 로 읽혀, 아무도 손대지 않는데 돌고
    /// 반쯤 끝난 에픽 하나가 탐색기를 120ms 마다 영영 깨운다(ce73eda). 그렇다고 묶음을
    /// 아예 안 돌리면 멤버를 집은 에픽이 `▸` 로 멈춰 서서, 목록 뿌리에서 무엇이 움직이는지
    /// 안 보인다(moai-x5eg). `--worktree` 로 겹친 줄도 겹친 칸으로 센다 — 옆에서 집은
    /// 멤버가 이 탐색기의 에픽을 돌린다.
    ///
    /// **계획에서 뺀 줄은 안 돈다**(moai-tawj) — 제가 미뤘든 부모·에픽·마일스톤에서
    /// 물려받았든(`nav::Index::deferred_root`). 묶음이 미룬 멤버로 안 도는 것과 같은
    /// 자다: 미룬 일은 칸이 `in_progress` 여도 지금 누가 손대는 줄이 아니다. 묶음 제
    /// 줄에도 건다 — 칸 셈은 묶음 제 미룸으로 멤버를 안 빼(`report::counted`) 미룬
    /// 에픽이 `busy` 로 남는데, 그 멤버는 물려받은 미룸으로 멈추니 에픽만 혼자 돈다.
    /// 그래서 이 자로는 **줄이 돌면 그 위 묶음도 돌고, 묶음이 돌면 그 밑에 도는 줄이
    /// 있다.**
    pub fn spins(&self, at: usize) -> bool {
        let i = &self.issues[at];
        if self.index.deferred_root(&i.id).is_some() {
            return false;
        }
        let busy = !crate::report::is_group(i) || self.states.get(&i.id).is_some_and(|s| s.busy);
        busy && crate::style::spins(self.column(at))
    }

    /// 그 줄이 **서 있는** 칸 — 묶음이면 멤버에서 읽은 칸, 아니면 제 칸. CLI 가
    /// 묻는 `report::column` 과 같은 답이다 — 묶음만 읽은 칸을 받는 것까지 같다.
    pub fn column(&self, at: usize) -> &str {
        let i = &self.issues[at];
        crate::report::is_group(i)
            .then(|| self.states.get(&i.id).map(|s| s.column.as_str()))
            .flatten()
            .unwrap_or(i.status.as_str())
    }

    /// 새 자료를 받아들이고 어긋난 것을 손본다. **시험이 저장소 없이 부른다** — 진짜
    /// 길은 스레드에서 셈을 마친 [`Fresh`] 를 [`App::apply_fresh`] 로 들인다.
    ///
    /// **커서는 번호가 아니라 정체로 따라간다**([`Anchor`]). 보던 줄이 아직 보이면
    /// 그 줄에 서고, 사라졌으면(지워졌거나 거름망에 빠졌으면) 전처럼 그 번호를 목록
    /// 안으로 자른 자리에 선다.
    #[cfg(test)]
    pub fn adopt(&mut self, issues: Vec<Issue>) {
        let index = Index::of(&issues);
        let states = states_of(&issues, &self.cfg);
        let now = crate::model::now();
        self.warnings = warnings_of(&issues, &self.unreadable, &self.cfg, &now);
        self.take(issues, index, states, now);
    }

    /// 이미 센 자료를 들이고 커서·경로·거름망을 맞춘다 — [`App::adopt`] 와 스레드에서
    /// 지어 온 것([`Fresh`])이 함께 지나는 길이다.
    fn take(
        &mut self,
        issues: Vec<Issue>,
        index: Index,
        states: States,
        now: String,
    ) {
        // **옛 자료로 잰다** — 줄의 첨자는 옛 `issues` 를 가리킨다.
        let held = self.current().map(|r| self.anchor_of(&r));
        self.issues = issues;
        self.index = index;
        self.states = states;
        self.now = now;
        self.repair_path();
        // 거름망은 이슈 첨자에 매인 것이라 반드시 다시 센다.
        match self.filter_text.clone() {
            Some(t) => {
                let mode = if let Some(q) = t.strip_prefix('/') {
                    Mode::Grep(Input::new(q))
                } else {
                    Mode::Filter(Input::new(&t))
                };
                if self.apply(&mode).is_err() {
                    self.clear_filter();
                }
            }
            None => self.keep = vec![true; self.issues.len()],
        }
        let rows = self.rows();
        let found = held.and_then(|a| rows.iter().position(|r| self.anchor_of(r) == a));
        // **굴린 자리는 같은 줄일 때만 둔다.** 다른 이슈로 옮겨 섰는데 굴린 수가 남으면
        // 그 이슈를 첫 줄부터 못 본다 — 커서를 옮길 때 0 으로 되돌리는 것(`move_to`)과
        // 같은 까닭이다. 같은 줄이면 본문이 바뀌었어도 두고, 넘치면 그림이 자른다.
        if found.is_none() {
            self.detail.rewind();
        }
        self.cursor = found.unwrap_or(self.cursor.min(rows.len().saturating_sub(1)));
    }

    /// 방금 쓴 줄에 선다. **다시 읽은 뒤에 부른다** — 첨자도 자리도 새 `issues` 에 대해 잰다.
    ///
    /// 자리는 `nav` 가 정한 그 줄의 집(`home_of`)이다. 묶음이어도 **안으로 들어가지
    /// 않는다** — 들어가면 방금 만든 것은 목록에 없고 `..` 만 보인다. 그 줄 위에
    /// 서야 오른쪽이 그 줄을 보여 준다.
    ///
    /// 거름망에 가리면 **길도 커서도 그대로 둔다.** 가려진 디렉터리로 옮겨 놓으면 사람은
    /// 왜 여기로 왔는지도 모르는 목록을 본다. 중복 id 면 그 디렉터리의 첫 줄이다([`Anchor`]).
    fn land(&mut self, id: &str) -> Landing {
        let Some(at) = self.index.find(id) else { return Landing::Missing };
        let home = self.index.home_of(at).clone();
        let was = std::mem::replace(&mut self.path, home);
        let want = Anchor::Issue(id.to_string());
        let rows = self.rows();
        let Some(row) = rows.iter().position(|r| self.anchor_of(r) == want) else {
            self.path = was;
            return Landing::Hidden;
        };
        if self.path != was || row != self.cursor {
            self.detail.rewind();
        }
        // 기억 자리는 **갈라지기 전까지만** 참이다. 그 밑은 들어간 적이 없는 층이라 0 —
        // `leave` 는 나온 디렉터리를 먼저 찾으므로 이 수는 못 찾을 때만 쓰인다.
        let same = was.iter().zip(&self.path).take_while(|(a, b)| a == b).count();
        self.remembered.truncate(same);
        self.remembered.resize(self.path.len(), 0);
        self.cursor = row;
        Landing::Shown
    }

    /// 그 줄의 정체. 지금 `issues` 에 대해 잰다.
    fn anchor_of(&self, row: &Row) -> Anchor {
        match row {
            Row::Up => Anchor::Up,
            Row::Item(Entry::Dir { seg, at: None }) => Anchor::Bucket(seg.clone()),
            Row::Item(Entry::Dir { at: Some(at), .. } | Entry::Leaf { at }) => {
                Anchor::Issue(self.issues[*at].id.clone())
            }
            Row::Project(at) => Anchor::Project(self.place_path(*at).map(Into::into).unwrap_or_default()),
        }
    }

    /// 들고 있던 경로가 아직 갈 수 있는 길인가.
    ///
    /// 마일스톤이 **처음 생기는 순간** 트리가 한 층 깊어져 경로가 통째로
    /// 낡는다. 지운 에픽도 마찬가지다. 갈 수 있는 데까지만 남기고 자른다 —
    /// 없는 자리에 서 있으면 빈 목록이 나오고, 사람은 자료가 사라진 줄 안다.
    fn repair_path(&mut self) {
        let mut good = Path::new();
        for seg in self.path.clone() {
            let here = self.index.entries(&self.issues, &good);
            let ok = here.iter().any(|e| matches!(e, Entry::Dir { seg: s, .. } if *s == seg));
            if !ok {
                break;
            }
            good.push(seg);
        }
        if good.len() == self.path.len() {
            self.path = good;
            return;
        }
        // **옮겨진 것과 지워진 것은 다르다.** 서 있던 마디가 아직 살아 있으면
        // 자리만 바뀐 것이니 그 새 자리로 따라간다 — 마일스톤이 처음 생기면
        // 에픽이 한 층 깊어지는데, 거기서 뿌리로 내려놓으면 가장 흔한 갱신이
        // 하필 자리를 가장 크게 잃는 갱신이 된다. `home_of` 가 그 새 자리를
        // 이미 알고, `cmd/tui.rs::resolve` 도 같은 셈을 쓴다.
        if let Some(at) = self.path.last().and_then(|s| self.index.find(seg_id(s)?))
            && self.index.is_dir(&self.issues, at)
        {
            let mut moved = self.index.home_of(at).clone();
            moved.push(self.index.seg_of(&self.issues, at));
            if moved != self.path {
                self.remembered = vec![0; moved.len()];
                self.cursor = 0;
                self.path = moved;
                return;
            }
        }
        self.remembered.truncate(good.len());
        self.cursor = 0;
        self.path = good;
    }

    /// 파일이 우리가 읽은 뒤로 바뀌었으면 **저절로 다시 읽는다.**
    ///
    /// 한때 말만 하고 F5 를 기다렸다(moai-6qdx) — 읽으면 커서가 튀었기 때문이다.
    /// 커서·기억 자리·굴린 자리가 줄의 정체를 따라가게 된 뒤로(moai-cera) 그 까닭이
    /// 없어졌고, 남은 것은 사람이 배너를 보고 키를 눌러야 하는 수고뿐이었다.
    ///
    /// **고친 때만 보면 놓친다** — rename 으로 갈아끼우는 쓰기는 같은 초에 떨어질 수
    /// 있어 길이도 함께 본다. **`stamp` 이 없다고 멈추지 않는다.** 아직 파일이 없는
    /// 저장소는 `stamp_of` 가 `None` 을 내는데, 거기서 걸러 버리면 파일이 생긴 뒤에도
    /// 영영 바뀐 줄 모른다. `None != Some(..)` 이 이미 바르게 답한다.
    ///
    /// **읽기가 실패하면 표식을 안 올린다**(`reload`) — 다음 걸음에 다시 해 본다.
    ///
    /// **겹쳐 보는 동안에는 옆 워크트리 스냅샷도 본다**(`watched`). 옆에서 `mv` 가
    /// 떨어진 것을 모르면 겹쳐 보기를 켠 뜻이 절반만 선다. 스냅샷이 아직 없던 옆
    /// 워크트리도 지켜보므로 거기서 `moai init` 하면 알아챈다. 새로 생긴 워크트리는
    /// `git worktree list` 를 다시 불러야 알아서 여기서는 못 보고, F5 가 그 길이다 —
    /// 걸음마다 git 을 띄우는 것은 700ms 마다 프로세스 하나를 만드는 일이다.
    ///
    /// **읽기는 스레드에서 한다**([`Fresh`]). 짓는 동안은 표식을 다시 재지 않는다 —
    /// 하나가 끝나기 전에 또 띄우면 몰아 쓰는 동안 스레드가 쌓인다. 끝난 것을 들인
    /// 뒤에도 파일이 또 바뀌었으면(표식은 읽기 전에 쟀다) 다음 걸음이 다시 띄운다.
    pub fn follow(&mut self) {
        self.reap();
        // 층은 제 표식을 따로 본다 — 층에 선 동안에는 아래(한 프로젝트)가 비어 할 일이 없다.
        self.follow_layer();
        if let Some((rx, _)) = &self.pending {
            match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Ok(fresh) => {
                    self.pending = None;
                    self.receive(fresh);
                }
                // 짓던 스레드가 죽었다. **이어 돌지 않는다** — ratatui 가 `try_init` 에서
                // 건 패닉 훅은 어느 스레드의 패닉에도 돌아 raw mode 와 대체 화면을 이미
                // 걷었다. 여기서 배너만 달고 이어 돌면 걷힌 터미널에 그리고, 키는 Enter
                // 를 쳐야 들어온다. 루프에서 난 패닉과 같게 되던져 끝낸다.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some((_, handle)) = self.pending.take()
                        && let Err(payload) = handle.join()
                    {
                        std::panic::resume_unwind(payload);
                    }
                    self.trouble = Some("다시 읽던 중에 멈췄다 — 다음 걸음에 다시 읽는다".into());
                }
            }
            return;
        }
        let Some(repo) = &self.repo else { return };
        let moved = stamp_of(repo) != self.stamp
            || self.watched.iter().any(|(path, was)| crate::store::stamp(path) != *was);
        if moved {
            let (tx, rx) = std::sync::mpsc::channel();
            let repo = repo.clone();
            let worktree = self.worktree;
            let read = self.read;
            // 받는 쪽이 사라졌으면(F5 로 버렸으면) 보내기가 실패한다 — 버린 것이라 그대로 둔다.
            let handle = std::thread::spawn(move || {
                let _ = tx.send(read(&repo, worktree));
            });
            self.pending = Some((rx, handle));
        }
    }

    /// 거름망을 건다. 빈 글은 "거름망 없음" 이다.
    ///
    /// **`all` 을 켠다.** `Filter` 의 기본값은 done 을 숨기는데, 탐색기가
    /// 시키지도 않은 줄을 숨기면 파일이 사라진 것처럼 보인다. 열린 것만 보려면
    /// `status=todo` 라고 적으면 된다.
    pub fn apply(&mut self, mode: &Mode) -> Result<(), String> {
        let text = match mode {
            Mode::Grep(q) | Mode::Filter(q) => q.text().to_string(),
            Mode::Browse | Mode::Ask(_) | Mode::Idea(_) | Mode::Pick(_) | Mode::Unregister(_) => String::new(),
        };
        if text.trim().is_empty() {
            self.filter_text = None;
            self.keep = vec![true; self.issues.len()];
            return Ok(());
        }
        let filter = self.build_filter(mode)?;
        // 칸 이름은 `Filter::build` 가 모른다 — 저장소가 정하는 것이라
        // `config` 에 있다. `cmd/show.rs` 와 같은 자로 잰다: 조용히 0건을 내면
        // `status=in-progress` 같은 오타가 "그 칸은 비었다" 와 구별되지 않는다.
        for s in &filter.status {
            self.cfg.require_known(s)?;
        }
        // 시계는 **적재마다** 고정한 것을 쓴다. 여기서 다시 잡으면 `stale=`
        // 같은 물음이 화면의 나머지와 다른 시각으로 판정된다.
        let now = self.now.clone();
        let wh = Where::of(&self.issues, &self.cfg);
        self.keep = self.issues.iter().map(|i| filter.matches(i, &now, &wh)).collect();
        self.filter_text = Some(match mode {
            Mode::Grep(_) => format!("/{text}"),
            _ => text,
        });
        Ok(())
    }

    /// 적은 글을 거름망으로. **적는 곳과 물어보는 곳이 같은 것을 쓴다** —
    /// 갈라지면 프롬프트 밑의 오류가 Enter 가 판정할 글과 다른 글을 판정한다.
    fn build_filter(&self, mode: &Mode) -> Result<Filter, String> {
        let raw = match mode {
            Mode::Grep(q) => Raw { grep: Some(q.text().to_string()), all: true, ..Raw::default() },
            Mode::Filter(q) => Raw { filter: split_filter(q.text()), all: true, ideas: true, ..Raw::default() },
            Mode::Browse | Mode::Ask(_) | Mode::Idea(_) | Mode::Pick(_) | Mode::Unregister(_) => Raw::default(),
        };
        // **`Filter::build` 를 지난다.** 소문자 접기·태그 정규화·`항목=값` 해석이
        // 전부 거기 있고, 건너뛰면 CLI 와 TUI 가 같은 글을 다르게 읽는다.
        Filter::build(raw)
    }

    pub fn clear_filter(&mut self) {
        self.filter_text = None;
        self.keep = vec![true; self.issues.len()];
    }

    /// 지금 디렉터리의 줄들.
    pub fn rows(&self) -> Vec<Row> {
        if let Some(l) = self.layer.as_ref().filter(|_| self.on_layer()) {
            return (0..l.places.len()).map(Row::Project).collect();
        }
        let mut rows: Vec<Row> = Vec::new();
        if !self.path.is_empty() || self.layer.is_some() {
            rows.push(Row::Up);
        }
        let keep = &self.keep;
        rows.extend(
            self.index
                .entries_where(&self.issues, &self.path, &|at| keep[at])
                .into_iter()
                .map(Row::Item),
        );
        rows
    }

    /// 커서가 가리키는 줄.
    pub fn current(&self) -> Option<Row> {
        self.rows().get(self.cursor).cloned()
    }

    /// 이미 센 목록에서 커서가 가리키는 줄. **그림은 한 프레임에 목록을 한 번만
    /// 센다** — 목록 패널과 상세 패널이 각자 세면 이슈 전부를 훑고 정렬하는 일이
    /// 한 키 누름에 두 번 더 돈다.
    pub fn current_of(&self, rows: &[Row]) -> Option<Row> {
        rows.get(self.cursor).cloned()
    }

    pub fn key(&mut self, k: KeyEvent) {
        // 쓰기의 알림은 **다음 키 하나에 걷힌다.** 읽었으면 할 일을 다 했고, 이 키가 또
        // 쓰기라면 그 쓰기가 제 알림을 새로 단다.
        self.notice = None;
        // raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. **어느 모드에서든 먼저 받는다** — 글을
        // 받는 중에 글자로 먹으면 검색칸에 `c` 가 찍히고, 폼·창·물음에서 막히면 멈춘 화면에서
        // 나갈 길이 없다. 적던 것은 그 길로 날아간다.
        if let Lookup::Run(keys::Anywhere::Quit) = keys::lookup(keys::ANYWHERE, &[k]) {
            self.quit = true;
            return;
        }
        // 글을 받는 동안에는 이동키가 글자다. 먼저 가로챈다. **`Tab` 도 여기서
        // 멈춘다** — 적다 말고 포커스가 튀면 적던 것을 잃는다.
        if !matches!(self.mode, Mode::Browse) {
            self.typing(k);
            return;
        }
        // **키의 뜻은 표에서 읽는다**([`keys::BROWSE`]). 표에 없는 키 — Ctrl·Alt 붙은 글자키도
        // — 는 여기 뜻이 없다: 안 거르면 Ctrl-A 가 등록 창을, Ctrl-D 가 "목록에서 뺄까" 를
        // 띄운다.
        let Lookup::Run(act) = keys::lookup(keys::BROWSE, &[k]) else { return };
        // **되는지는 한 판정이 가른다**([`keys::Browse::enabled`]) — 키 바가 같은 판정으로
        // 적을 키를 고르므로 둘이 안 갈린다. 층에서 뜻이 없는 키는 왜 안 되는지를 한 줄로
        // 말한다. `n` 은 층에서도 듣는다 — 커서의 프로젝트를 담을 곳으로 박는다([`App::open_form`]).
        match act.enabled(&self.key_ctx()) {
            Ok(()) => {}
            Err(keys::Off::Quiet) => return,
            Err(keys::Off::Why(say)) => {
                self.notice = Some(say);
                return;
            }
        }
        use keys::Browse as B;
        match act {
            B::Quit => self.quit = true,
            B::FocusPrev => self.focus = self.focus.prev(),
            B::FocusNext => self.focus = self.focus.next(),
            B::Step => self.step(k),
            // **드나드는 키도 포커스를 탄다**(`enabled`). 상세를 읽다가 누른 Enter·←가 목록을
            // 옮기면 보던 이슈가 바뀌고 굴린 자리도 첫 줄로 돌아간다 — ↑↓ 를 포커스에
            // 태운 까닭과 같다. 상세에서는 아직 뜻이 없어 아무 일도 안 한다.
            B::Enter => self.enter(),
            B::Leave => self.leave(),
            B::Grep => self.mode = Mode::Grep(Input::default()),
            B::Filter => self.mode = Mode::Filter(Input::default()),
            // **포커스와 상관없이 연다.** 무엇을 보다가 떠올랐든 담는 칸은 하나다. 담을 곳은
            // 여는 순간 박힌다 — 층에서는 커서의 프로젝트다(moai-fccv).
            B::Jot => self.open_form(),
            // 등록은 사람의 설정이지 이 프로젝트의 것이 아니라 **어디서든 연다**(moai-plvy).
            B::Pick => self.open_picker(),
            // 해제는 층의 줄에서만, 목록 포커스로(`enabled`) — 커서가 선 줄을 뺀다.
            B::Unregister => self.ask_unregister(),
            // 거름망이 걸려 있으면 Esc 가 그것을 푼다. 아니면 아무 일도 없다 —
            // Esc 로 화면이 꺼지면 실수 한 번에 하던 것이 날아간다.
            B::ClearFilter => self.clear_filter(),
            B::Reload => self.reload(),
            // 켜고 끄는 것은 **다시 읽는 것**이다. 겹친 줄은 적재 때 한 번 세는 것이라
            // (`states`·거름망·경고), 들고 있는 것에 덧칠하면 셈이 옛 줄로 남는다.
            B::Worktree => {
                self.worktree = !self.worktree;
                self.reload();
            }
            B::Raw => {
                self.raw = !self.raw;
                // 그린 것과 원문은 줄 수가 다르다. 굴린 자리를 들고 가면
                // 엉뚱한 데가 나온다.
                self.detail.rewind();
            }
            // 진행 바탕을 켜고 끈다(moai-94uf). 세는 것은 그대로고 그리는 것만 바뀌므로 다시
            // 읽지 않는다 — `w` 와 다른 까닭이다.
            B::Shade => self.shade = !self.shade,
            // 상세를 굴린다. **왼쪽은 그대로 둔다** — 오른쪽만 길어서 못 보는
            // 것이므로, 굴리려고 커서를 옮기게 하면 보던 이슈를 잃는다.
            // **포커스와 상관없이 듣는다** — 포커스가 생기기 전부터 손에 익은
            // 사람이 있고, 목록에 선 채로 상세를 한 칸 굴리는 길이 여전히 쓸모 있다.
            B::DetailDown => self.detail.by(1),
            B::DetailUp => self.detail.by(-1),
            B::DetailPageDown => self.detail.by(PAGE as isize),
            B::DetailPageUp => self.detail.by(-(PAGE as isize)),
        }
    }

    /// 키 표가 켜짐과 낱말을 가를 값. **여기서 잰다** — 표([`keys`])는 조각이라 `App` 을 모른다.
    pub fn key_ctx(&self) -> keys::Ctx {
        keys::Ctx {
            layer: self.on_layer(),
            list_focus: self.focus == Pane::Explorer,
            worktree: self.worktree,
            raw: self.raw,
            shade: self.shade,
            next_pane: draw::pane_name(self.focus.next()),
            prev_pane: draw::pane_name(self.focus.prev()),
        }
    }

    /// 이동키 하나를 **포커스 있는 칸에** 준다. 두 칸이 같은 조각([`scroll`])으로
    /// 굴러 걸음(`PAGE`)도 끝의 뜻도 같다.
    ///
    /// 상세의 끝(`End`)은 마지막으로 그린 줄 수로 잰다 — 줄 수는 폭에 달렸고 폭은
    /// 그려야 나온다. 루프는 키 하나마다 한 번 그리므로 그 수는 한 걸음 넘게 낡지 않는다.
    fn step(&mut self, k: KeyEvent) {
        match self.focus {
            Pane::Explorer => {
                if let Some(at) = scroll::cursor(k, self.cursor, || self.rows().len()) {
                    self.move_to(at);
                }
            }
            Pane::Detail => {
                self.detail.key(k);
            }
        }
    }

    /// 커서를 옮기고 **상세를 첫 줄로 되돌린다.** 다른 것을 보는데 굴린 자리가
    /// 남아 있으면 그 이슈의 첫 줄부터 못 본다.
    fn move_to(&mut self, at: usize) {
        if at != self.cursor {
            self.detail.rewind();
        }
        self.cursor = at;
    }

    /// 글을 받는 중.
    ///
    /// **글자를 넣고 지우는 것은 칸([`Input`])이 한다.** 여기는 칸이 돌려준 키만
    /// 정한다 — Enter·Esc·Ctrl-C. 칸이 먹은 키는 여기까지 오지 않으므로 빈 칸의
    /// Backspace 가 "한 층 위로" 로 새지 않는다.
    fn typing(&mut self, k: KeyEvent) {
        match self.mode {
            Mode::Idea(_) => return self.jot(k),
            Mode::Pick(_) => return self.pick(k),
            Mode::Unregister(_) => return self.settle_unregister(k),
            _ => {}
        }
        let eaten = match &mut self.mode {
            Mode::Grep(input) | Mode::Filter(input) => input.key(k),
            Mode::Ask(ask) => {
                let eaten = ask.input.key(k);
                if eaten {
                    ask.error = None;
                }
                eaten
            }
            Mode::Browse | Mode::Idea(_) | Mode::Pick(_) | Mode::Unregister(_) => return,
        };
        if eaten {
            return;
        }
        // 칸이 안 먹은 키는 표([`keys::PROMPT`])가 가른다 — Enter·Esc. **Ctrl 은 글자가
        // 아니다**: 칸이 안 먹은 Ctrl 조합(Ctrl-Enter 같은 것)은 표에 없어 아무 일도 하지
        // 않는다. Ctrl-C 는 [`App::key`] 가 이미 받았다.
        //
        // **Alt 는 옮기면서 바뀌었다.** 옛 `typing()` 은 Alt 를 안 보고 `Alt-b` 를
        // `b` 로, `Alt-Backspace` 를 한 글자 지우기로 먹었다. 칸은 Alt 조합을
        // 글자로 치지 않으므로(Alt 를 Meta 로 보내는 터미널에서 `b` 가 찍히는 것은
        // 사람이 친 것이 아니다) 이제 둘 다 아무 일도 하지 않는다.
        let Lookup::Run(act) = keys::lookup(keys::PROMPT, &[k]) else { return };
        if matches!(self.mode, Mode::Ask(_)) {
            self.answer(act);
            return;
        }
        match act {
            keys::Prompt::Apply => {
                let mode = self.mode.clone();
                // 잘못 적은 것은 버리지 않고 그 자리에 둔다 — 지우고 다시 치게
                // 하면 긴 거름망일수록 고치기가 벌이 된다.
                if self.apply(&mode).is_ok() {
                    self.mode = Mode::Browse;
                    self.cursor = self.cursor.min(self.rows().len().saturating_sub(1));
                }
            }
            keys::Prompt::Cancel => self.mode = Mode::Browse,
        }
    }

    /// 붙여 넣은 글(moai-od9q) — 루프가 bracketed paste 로 받은 `Event::Paste`. **키로
    /// 풀지 않는다.** 풀면 붙인 글의 탭이 Tab 으로 폼의 칸을 옮기고, 줄바꿈이 Enter 로
    /// 검색을 걸거나 제목을 떠나며, 탐색 중에 붙인 `q` 는 탐색기를 끝낸다.
    ///
    /// 글은 **지금 열린 글칸 하나에만** 들어간다 — 검색·거름망·누군지 묻는 칸·폼의 포커스 칸·
    /// 고르기 창의 경로 칸. 무엇으로 받을지(한 줄로 잇기·줄로 가르기·걸러낼 글자)는 칸이 정한다.
    /// 받을 칸이 없으면 삼키고 그렇다고 말한다 — 말없이 삼키면 붙여넣기가 고장 난 줄 안다.
    /// 해제 물음에서는 **다른 키처럼** 물음을 거둔다: 붙인 글 속 `y` 는 답이 아니다.
    pub fn paste(&mut self, s: &str) {
        self.notice = None;
        // 층에서는 `/`·`f` 가 안 열린다 — **키 처리와 같은 판정**([`keys::Browse::enabled`])으로
        // 열리는 칸만 대고, 키 이름은 표에서 읽는다.
        let ctx = self.key_ctx();
        let open: Vec<String> = [keys::Browse::Grep, keys::Browse::Filter, keys::Browse::Jot]
            .into_iter()
            .filter(|a| a.enabled(&ctx).is_ok())
            .map(|a| format!("`{}`", keys::label(keys::BROWSE, a)))
            .collect();
        let open = open.join("·");
        match &mut self.mode {
            Mode::Browse => self.notice = Some(format!("붙여 넣을 칸이 없다 — {open} 으로 칸을 열고 붙인다")),
            Mode::Grep(input) | Mode::Filter(input) => input.paste(s),
            Mode::Ask(ask) => {
                ask.input.paste(s);
                ask.error = None;
            }
            Mode::Idea(form) => form.paste(s),
            Mode::Pick(picker) => picker.paste(s),
            Mode::Unregister(_) => self.mode = Mode::Browse,
        }
    }

    /// 누군지 묻는 칸이 먹지 않은 키 — Enter·Esc.
    ///
    /// **받은 것은 `model::Actor::parse` 로 잰다** — `--user`·`MOAI_ACTOR` 와 같은
    /// 자다. 여기서 따로 재면 CLI 가 거절하는 모양이 화면에서는 통과해 되돌릴 수
    /// 없는 저널에 쌓인다. 거절하면 칸은 열린 채 까닭을 달고, 적은 것은 그대로 둔다.
    ///
    /// 받거나 그만두면 **적던 모드로 먼저 돌아간다.** 받았으면 그다음에 쓰기를 다시
    /// 부른다 — 다시 부른 쓰기는 되돌려 놓은 폼에서 적던 것을 읽는다.
    fn answer(&mut self, act: keys::Prompt) {
        let Mode::Ask(ask) = &mut self.mode else { return };
        let who = match act {
            keys::Prompt::Apply => match crate::model::Actor::parse(ask.input.text()) {
                Some(who) => Some(who),
                None => {
                    ask.error = Some(format!("`이름 (메일)` 모양이 아니다 — 예: {ASK_EXAMPLE}"));
                    return;
                }
            },
            keys::Prompt::Cancel => None,
        };
        let Mode::Ask(ask) = std::mem::replace(&mut self.mode, Mode::Browse) else { return };
        self.mode = *ask.back;
        if let Some(who) = who {
            self.user = Some(crate::model::label(&who.name, Some(&who.email), crate::config::Naming::Full));
            (ask.then)(self);
        }
    }

    /// 생각 담기 폼의 키. **무엇을 할지는 폼이 정하고**([`Form::key`]) 여기는 그대로 한다.
    ///
    /// Ctrl-C 는 폼보다 먼저 [`App::key`] 가 받는다 — 어느 모드에서든 나가는 길이다. 적던 것은
    /// 그 길로 날아가지만, raw mode 에서 Ctrl-C 를 막으면 멈춘 화면에서 나갈 길이 없어진다.
    fn jot(&mut self, k: KeyEvent) {
        let Mode::Idea(form) = &mut self.mode else { return };
        match form.key(k) {
            Act::Stay => {}
            Act::Save => save_idea(self),
            Act::Close => {
                self.mode = Mode::Browse;
                self.forget_write_failure();
            }
        }
    }

    /// 폼을 닫으면 **그 폼이 못 쓴 까닭도 걷는다.** 쓰기의 실패는 저절로 다시 읽기에
    /// 안 지워지게 붙박아 두었는데(`write_failed`), 그것은 폼이 열린 채 왜 안 닫혔는지를
    /// 말하려는 것이었다. 적던 것을 버리고 닫았으면 그 말이 가리키는 것이 화면에 없다 —
    /// 남겨 두면 다음에 쓰기가 성공할 때까지 빨간 배너가 아무것도 못 고치는 채로 선다.
    /// 읽기의 실패는 건드리지 않는다: 그것은 폼과 상관없이 지금 화면이 낡았다는 말이다.
    fn forget_write_failure(&mut self) {
        if self.write_failed {
            self.trouble = None;
            self.write_failed = false;
        }
    }

    /// 지금 적고 있는 글에 대한 오류. 없으면 `None`.
    ///
    /// **Enter 가 밟을 길을 그대로 밟는다.** 칸 이름 검사까지 여기서 해야
    /// `status=in-progress` 같은 오타가 "그 칸은 비었다" 로 보이지 않는다.
    pub fn input_error(&self) -> Option<String> {
        match &self.mode {
            Mode::Filter(q) if !q.text().trim().is_empty() => match self.build_filter(&self.mode) {
                Err(e) => Some(e),
                Ok(f) => f.status.iter().find_map(|s| self.cfg.require_known(s).err()),
            },
            _ => None,
        }
    }

    fn enter(&mut self) {
        match self.current() {
            Some(Row::Up) => self.leave(),
            Some(Row::Item(Entry::Dir { seg, .. })) => {
                self.remembered.push(self.cursor);
                self.path.push(seg);
                self.cursor = 0;
                self.detail.rewind();
            }
            Some(Row::Project(at)) => self.enter_project(at),
            // 잎은 들어갈 데가 없다. 상세는 오른쪽이 이미 보여 주고 있다.
            _ => {}
        }
    }

    /// 한 층 위로. **방금 나온 디렉터리에 선다** — 그것이 곧 돌아갈 자리다.
    ///
    /// 기억한 번호(`remembered`)는 들어가 있는 동안 파일이 바뀌면 낡는다: 위에 에픽
    /// 하나가 생기면 같은 번호가 옆 에픽을 가리킨다. 그래서 번호는 나온 디렉터리를
    /// 못 찾을 때만(거름망에 빠졌거나 `--path` 로 시작했거나) 쓴다.
    fn leave(&mut self) {
        // 프로젝트 뿌리에서 한 층 더 — 층이 있으면 그리로 간다(결정 3). 층에 섰으면 위가 없다.
        if self.path.is_empty() {
            self.climb();
            return;
        }
        if let Some(from) = self.path.pop() {
            self.detail.rewind();
            let fallback = self.remembered.pop().unwrap_or(0);
            let rows = self.rows();
            self.cursor = rows
                .iter()
                .position(|r| matches!(r, Row::Item(Entry::Dir { seg, .. }) if *seg == from))
                .unwrap_or(fallback.min(rows.len().saturating_sub(1)));
        }
    }

    /// 지금 어디인가. 뿌리는 `/`.
    pub fn crumbs(&self) -> String {
        if self.on_layer() {
            return "프로젝트 층".into();
        }
        if self.path.is_empty() {
            return "/".into();
        }
        let mut out = String::new();
        for seg in &self.path {
            out.push('/');
            out.push_str(&self.seg_label(seg));
        }
        out
    }

    /// id 를 제목으로 푼다. 없으면 **끊겼다고 적는다** — id 만 내면 그것이
    /// 그저 제목 없는 줄인지 없는 것을 가리키는 참조인지 알 길이 없다.
    /// 훑지 않는다. 이 함수는 막는 것마다·소속마다·프레임마다 불린다.
    pub fn title_of(&self, id: &str) -> String {
        match self.index.find(id) {
            Some(at) => self.issues[at].title.clone(),
            None => format!("{id}  {MISSING}"),
        }
    }

    fn seg_label(&self, seg: &Seg) -> String {
        let id = match seg {
            Seg::Milestone(None) => return "(마일스톤 없음)".into(),
            Seg::Lost => return "(길 잃음)".into(),
            Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => id,
        };
        match self.index.find(id) {
            Some(at) => self.issues[at].title.clone(),
            None => format!("{id}  {MISSING}"),
        }
    }
}

/// 폼에 적힌 생각 하나를 담는다. **[`Retry`] 로도 넘긴다** — 누군지 묻고 받으면
/// [`App::answer`] 가 폼을 되돌려 놓고 이 함수를 다시 부르고, 이 함수는 되돌려 놓은
/// 폼에서 제목과 본문을 다시 읽는다.
///
/// **만드는 길은 CLI `idea add` 와 같다**(`store::new_id`·`store::admit`) — id 모양,
/// 정규화, 검증, 저널 `create`, 만든 사람이 담당인 것까지. 에픽은 없다: 커서가 선 자리가
/// 무엇이든 넣지 않는다. 칸은 config 의 첫 칸이다.
///
/// **담는 곳은 폼이 연 순간 박은 프로젝트다**([`form::Target`]) — 커서도, 층이 그새 다시
/// 읽힌 것도 상관없다([`App::stand_at`]).
///
/// 되면 폼을 **닫기만 한다.** 다시 읽기·만든 줄에 커서 두기·`✓ 담김 · id` 알림은
/// `write` 가 한다(moai-064q) — 여기서 또 하면 한 쓰기에 목록을 두 번 세거나 알림이 둘이
/// 된다. 안 되면 **아무것도 건드리지 않는다** — 누군지 묻는 중이면 `write` 가 이미 모드를
/// `Ask` 로 바꿨고, 실패면 폼이 열린 채 까닭이 배너에 선다.
fn save_idea(app: &mut App) {
    let Mode::Idea(form) = &app.mode else { return };
    let (title, body, into) = (form.title(), form.body(), form.into.clone());
    // 폼이 이미 거절한다. 되돌아온 길(`answer`)도 같은 폼이라 여기 걸릴 일은 없지만,
    // 빈 제목을 `write` 까지 보내면 누군지부터 묻는다 — 거절이 물음 뒤로 밀린다.
    if title.is_empty() {
        return;
    }
    // **폼이 박은 곳에 서야 쓴다**(moai-fccv). 층에서 연 폼이면 여기서 그 프로젝트로 들어가고,
    // 선 곳이 다르면 쓰지 않는다. `write` 는 `self.repo` 에 쓰므로 여기를 지나면 머리에 보인
    // 곳이 곧 쓰는 곳이다 — 묻고 이어진 쓰기도 이 함수를 다시 지난다.
    if !app.stand_at(into.as_ref()) {
        return;
    }
    let at = crate::model::now();
    let wrote = app.write(save_idea, move |issues, cfg, reserved, by| {
        let id = crate::store::new_id(issues, cfg, reserved, None, &title);
        let mut idea = Issue::new(id, title, Kind::Idea, Status::new(cfg.first_status()), &at);
        (idea.assignee, idea.assignee_email) = by.as_assignee();
        idea.body = body;
        let (entry, made) = crate::store::admit(issues, cfg, idea, by)?;
        Ok((vec![entry], Touched { id: made.id, done: "담김" }))
    });
    if wrote.is_some() {
        app.mode = Mode::Browse;
    }
}

/// 누군지 묻는 칸이 보이는 본보기. 거절문이 댄다.
pub const ASK_EXAMPLE: &str = "레이븐 (raven@example.com)";

/// 없는 것을 가리키는 참조에 붙이는 말. **한 낱말로 통일한다** — 자리마다
/// 다른 말을 쓰면 같은 깨짐을 서로 다른 일로 읽는다.
const MISSING: &str = "(없다)";

/// 그 마디가 가리키는 줄의 id. 바구니는 제 줄이 없으므로 `None`.
fn seg_id(seg: &Seg) -> Option<&str> {
    match seg {
        Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => Some(id),
        Seg::Milestone(None) | Seg::Lost => None,
    }
}

pub use crate::store::Stamp;

pub fn stamp_of(repo: &Repo) -> Stamp {
    crate::store::stamp(&repo.issues_path())
}

/// 한 줄을 `--filter` 토큰들로 쪼갠다.
///
/// **`항목=` 이 시작하는 데서만 쪼갠다.** 그냥 띄어쓰기로 쪼개면 값에 빈칸이
/// 든 것(`grep=원자적 쓰기`, `status=to do`)을 이 칸에서는 아예 적을 수 없다 —
/// CLI 는 그것을 인자 하나로 받으므로, "CLI 와 같은 문법" 이라던 약속이 거기서
/// 깨진다. 항목 이름이 없는 조각은 앞 토큰의 값에 마저 붙는다.
fn split_filter(q: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in q.split_whitespace() {
        match out.last_mut() {
            Some(prev) if !w.contains('=') => {
                prev.push(' ');
                prev.push_str(w);
            }
            _ => out.push(w.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use crate::model::{Kind, Status};

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn make(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    fn member(id: &str, epic: &str) -> Issue {
        let mut i = make(id, Kind::Issue);
        i.epic = Some(epic.into());
        i
    }

    fn app() -> App {
        let issues = vec![
            make("argos-0001", Kind::Epic),
            make("argos-0002", Kind::Epic),
            member("argos-0003", "argos-0001"),
            member("argos-0004", "argos-0001"),
            make("argos-0009", Kind::Issue),
        ];
        App::new(issues, cfg(), Path::new())
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 루프가 빠른 걸음으로 깰지를 이 답으로 정한다. **거짓을 내면 도는
    /// 글리프가 첫 칸에 멈춘 채 7초에 한 번만 움직인다** — 스피너가 있는데
    /// 아무도 깨우지 않는 그 조합이 눈에는 버그로 보이고 코드로는 안 보인다.
    #[test]
    fn the_loop_only_wakes_fast_when_something_spins() {
        let mut a = app();
        assert!(!a.spinning(), "todo 뿐인데 돈다고 한다");
        let mut issues = a.issues.clone();
        // 에픽의 적힌 칸으로는 안 돈다 — 묶음의 칸은 멤버에서 읽는다.
        issues[0].status = Status::new("in_progress");
        a.adopt(issues.clone());
        assert!(!a.spinning(), "멤버가 안 움직인 에픽이 적힌 칸으로 돈다");
        // 읽은 칸으로도 안 돈다 — 멤버 하나가 끝나기만 해도 묶음은 `in_progress` 로
        // 읽히는데, 그것으로 깨우면 아무도 손대지 않은 저장소에서 화면이 계속 깬다.
        issues[2].status = Status::new("done");
        a.adopt(issues.clone());
        assert!(!a.spinning(), "일은 멈췄는데 묶음의 읽은 칸으로 돈다");
        issues[3].status = Status::new("in_progress");
        a.adopt(issues);
        assert!(a.spinning(), "in_progress 가 있는데 안 돈다고 한다");
    }

    /// 줄마다 도는지(`App::spins`) — **묶음은 그 밑에 집은 일이 있을 때만 돈다**
    /// (moai-x5eg). 깨우는 쪽(`spinning`)은 이 답을 모은 것이어야 한다.
    #[test]
    fn a_group_spins_only_while_a_member_is_held() {
        fn spun(a: &App) -> Vec<&str> {
            (0..a.issues.len()).filter(|&at| a.spins(at)).map(|at| a.issues[at].id.as_str()).collect()
        }
        let mut mile = make("argos-0005", Kind::Milestone);
        mile.status = Status::new("todo");
        let mut epic = make("argos-0001", Kind::Epic);
        epic.milestone = Some("argos-0005".into());
        let mut closed = member("argos-0002", "argos-0001");
        closed.status = Status::new("done");
        let mut issues = vec![mile, epic, closed, member("argos-0003", "argos-0001")];
        let mut a = App::new(issues.clone(), cfg(), Path::new());

        // 끝난 것 하나와 첫 칸 하나 — 둘 다 `in_progress` 로 읽히지만 아무도 손대지 않는다.
        assert_eq!((a.column(0), a.column(1)), ("in_progress", "in_progress"));
        assert!(spun(&a).is_empty(), "반쯤 끝난 묶음이 돈다 — {:?}", spun(&a));
        assert!(!a.spinning());

        // 하나를 집으면 그 일과 에픽, 그리고 **에픽을 거쳐 물려받은 마일스톤**까지 돈다.
        issues[3].status = Status::new("in_progress");
        a.adopt(issues.clone());
        assert_eq!(spun(&a), ["argos-0005", "argos-0001", "argos-0003"]);
        assert!(a.spinning());

        // **미룬 멤버는 묶음을 안 돌린다.** 칸 셈이 그 멤버를 빼므로(`report::counted`)
        // 곁들이도 뺀다 — 에픽은 끝난 멤버만 남아 `done` 으로 읽힌다. **미룬 일 제 줄도
        // 안 돈다**(moai-tawj) — 계획에서 뺀 일이 "지금 손대는 중" 으로 보이면 안 되고,
        // 돌면 탐색기를 스피너 걸음으로 깨운다. 글리프는 칸을 여전히 말한다(`▸` 가 아닌
        // 멈춘 `in_progress` 글리프).
        issues[3].deferred_at = Some("2026-09-02T00:00:00Z".into());
        a.adopt(issues.clone());
        assert_eq!(a.column(1), "done");
        assert_eq!(a.column(3), "in_progress");
        assert!(spun(&a).is_empty(), "미룬 일이 돈다 — {:?}", spun(&a));
        assert!(!a.spinning(), "미룬 일 하나가 탐색기를 빠른 걸음으로 깨운다");

        // **묶음을 미루면 그 밑이 다 멈춘다.** 칸 셈은 묶음 제 미룸으로 멤버를 안 빼
        // 에픽은 여전히 `in_progress` 로 읽히지만, 에픽도 멤버도 계획에서 빠졌다 — 멤버는
        // 물려받은 미룸으로 안 돌고, 묶음이 혼자 돌면 도는 멤버 하나 없이 도는 줄이 선다.
        issues[3].deferred_at = None;
        issues[1].deferred_at = Some("2026-09-02T00:00:00Z".into());
        a.adopt(issues.clone());
        assert_eq!(a.column(1), "in_progress", "제 미룸으로 집은 멤버를 뺐다");
        assert!(spun(&a).is_empty(), "미룬 에픽 밑이 돈다 — {:?}", spun(&a));

        // **부모를 미뤄도 같다** — 물려받은 미룸으로 빠진 자식은 안 돌고, 에픽도 그 자식으로
        // 안 바쁘다. 줄이 돌면 그 위의 묶음도 돌고, 묶음이 돌면 그 밑에 도는 줄이 있다.
        issues[1].deferred_at = None;
        issues[3].status = Status::new("todo");
        issues[3].deferred_at = Some("2026-09-02T00:00:00Z".into());
        let mut child = member("argos-0003.a1b", "argos-0001");
        child.status = Status::new("in_progress");
        issues.push(child);
        a.adopt(issues);
        assert!(spun(&a).is_empty(), "미룬 부모 밑의 자식이 돈다 — {:?}", spun(&a));

        // 깨우는 쪽은 줄마다의 답을 모은 것이다.
        assert_eq!(a.spinning(), !spun(&a).is_empty());
    }

    /// 스레드에서 짓는 다시 읽기를 **끝날 때까지** 받는다. 루프가 하는 것을 흉내 낸다.
    fn settle(a: &mut App) {
        a.follow();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while a.loading() {
            assert!(std::time::Instant::now() < until, "다시 읽기가 끝나지 않는다");
            std::thread::sleep(std::time::Duration::from_millis(2));
            a.follow();
        }
    }

    /// 진짜 파일을 쓰는 시험이 쓰는 임시 자리. **터져도 치운다** — 바로
    /// `remove_dir_all` 을 부르면 assert 하나가 터질 때마다 찌꺼기가 남고,
    /// 이름이 pid 라 다음 실행이 그것을 치우지도 못한다.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "moai-tui-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 지금 보이는 줄의 id 들 (`..` 은 뺀다).
    fn shown(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(e) => e.at().map(|at| a.issues[at].id.clone()),
                Row::Up | Row::Project(_) => None,
            })
            .collect()
    }

    /// 뿌리에는 `..` 이 없다. 들어가면 생긴다.
    #[test]
    fn up_appears_only_below_the_root() {
        let mut a = app();
        assert!(!a.rows().contains(&Row::Up));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.rows().first(), Some(&Row::Up));
    }

    /// 들어갔다 나오면 **있던 자리로 돌아온다**. 매번 맨 위로 튕기면 못 쓴다.
    #[test]
    fn leaving_puts_the_cursor_back_where_it_was() {
        let mut a = app();
        a.key(key(KeyCode::Down)); // 두 번째 에픽
        assert_eq!(a.cursor, 1);
        a.key(key(KeyCode::Enter));
        assert_eq!((a.cursor, a.path.len()), (0, 1));
        a.key(key(KeyCode::Backspace));
        assert_eq!((a.cursor, a.path.len()), (1, 0), "있던 자리로 안 돌아왔다");
    }

    /// 들어가 있는 동안 **위에 줄이 생겨도** 나오면 방금 나온 디렉터리에 선다.
    /// 기억한 번호로 돌아가면 옆 에픽에 선다.
    #[test]
    fn leaving_finds_the_directory_it_came_from_even_after_a_reload() {
        let mut a = app();
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, [Seg::Epic("argos-0002".into())]);

        let mut more = a.issues.clone();
        more.push(make("argos-0000", Kind::Epic));
        a.adopt(more);
        a.key(key(KeyCode::Backspace));
        assert_eq!(
            a.current(),
            Some(Row::Item(Entry::Dir { seg: Seg::Epic("argos-0002".into()), at: a.index.find("argos-0002") })),
            "나온 디렉터리가 아니라 기억한 번호에 섰다"
        );
    }

    /// 커서는 목록 밖으로 못 나간다.
    #[test]
    fn the_cursor_stays_inside_the_list() {
        let mut a = app();
        for _ in 0..50 {
            a.key(key(KeyCode::Up));
        }
        assert_eq!(a.cursor, 0);
        for _ in 0..50 {
            a.key(key(KeyCode::Down));
        }
        assert_eq!(a.cursor, a.rows().len() - 1);
        a.key(key(KeyCode::Home));
        assert_eq!(a.cursor, 0);
        a.key(key(KeyCode::End));
        assert_eq!(a.cursor, a.rows().len() - 1);
    }

    /// `Tab` 은 앞으로, `Shift-Tab` 은 뒤로 돈다. 끝에서 처음으로 넘어간다.
    /// `Shift-Tab` 은 터미널에 따라 `BackTab` 으로도 Shift 붙은 `Tab` 으로도 온다.
    #[test]
    fn tab_cycles_the_focus_both_ways() {
        let mut a = app();
        assert_eq!(a.focus, Pane::Explorer, "목록에서 시작하지 않는다");
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Pane::Detail);
        a.key(key(KeyCode::Tab));
        assert_eq!(a.focus, Pane::Explorer, "끝에서 처음으로 안 돌았다");
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(a.focus, Pane::Detail, "BackTab 이 뒤로 안 돌았다");
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(a.focus, Pane::Explorer, "Shift 붙은 Tab 이 뒤로 안 돌았다");
        // 순환은 칸이 몇이든 제자리로 돌아온다
        for p in Pane::ALL {
            assert_eq!(p.next().prev(), p);
            let mut q = p;
            for _ in Pane::ALL {
                q = q.next();
            }
            assert_eq!(q, p);
        }
    }

    /// 그림이 잴 것을 손으로 넣는다 — 상세가 `height` 줄 칸에 `len` 줄을 그렸다.
    /// 시험은 터미널 없이 돌므로 그림 대신 이것을 부른다.
    fn drawn(a: &mut App, height: usize, len: usize) {
        a.detail.fit(height, len);
    }

    /// 이동키는 **포커스 있는 칸이** 먹는다. 상세에 포커스가 있으면 왼쪽 커서는
    /// 그대로다 — 굴리려다 보던 이슈를 잃으면 안 된다.
    #[test]
    fn movement_keys_go_to_the_focused_pane() {
        let mut a = app();
        a.key(key(KeyCode::Down));
        assert_eq!((a.cursor, a.detail.offset()), (1, 0));
        drawn(&mut a, 10, 40);

        a.key(key(KeyCode::Tab));
        a.key(key(KeyCode::Down));
        assert_eq!((a.cursor, a.detail.offset()), (1, 1), "상세에 포커스가 있는데 목록이 움직였다");
        a.key(key(KeyCode::PageDown));
        assert_eq!((a.cursor, a.detail.offset()), (1, 11));
        a.key(key(KeyCode::Up));
        a.key(key(KeyCode::PageUp));
        assert_eq!((a.cursor, a.detail.offset()), (1, 0));
        a.key(key(KeyCode::PageUp));
        assert_eq!(a.detail.offset(), 0, "첫 줄 위로 굴렀다");
        a.key(key(KeyCode::End));
        assert_eq!((a.cursor, a.detail.offset()), (1, 30), "End 가 끝에 안 닿았다");
        a.key(key(KeyCode::Up));
        assert_eq!(a.detail.offset(), 29, "끝에서 ↑ 가 곧바로 안 듣는다");
        a.key(key(KeyCode::Home));
        assert_eq!((a.cursor, a.detail.offset()), (1, 0));

        // 목록으로 돌아오면 이동키가 다시 커서를 옮긴다
        a.key(key(KeyCode::Tab));
        a.key(key(KeyCode::End));
        assert_eq!(a.cursor, a.rows().len() - 1);
        a.key(key(KeyCode::Home));
        assert_eq!(a.cursor, 0);
        a.key(key(KeyCode::PageDown));
        assert_eq!(a.cursor, a.rows().len() - 1, "PageDown 이 목록 밖으로 나갔다");
    }

    /// **드나드는 키(Enter·→·Backspace·←)도 포커스를 탄다.** 상세를 읽다 누른 키가
    /// 목록을 옮기면 보던 이슈가 바뀌고 굴린 자리도 잃는다.
    #[test]
    fn entering_and_leaving_keys_go_to_the_focused_pane() {
        for k in [KeyCode::Enter, KeyCode::Right] {
            let mut a = app();
            a.key(key(KeyCode::Tab));
            a.key(key(k));
            assert!(a.path.is_empty(), "상세에 포커스가 있는데 {k:?} 가 목록을 들어갔다");
            a.key(key(KeyCode::Tab));
            a.key(key(k));
            assert_eq!(a.path.len(), 1, "목록에 포커스가 있는데 {k:?} 가 안 들어갔다");
        }
        for k in [KeyCode::Backspace, KeyCode::Left] {
            let mut a = app();
            a.key(key(KeyCode::Enter));
            drawn(&mut a, 10, 40);
            a.key(key(KeyCode::Tab));
            a.key(key(KeyCode::Down));
            a.key(key(k));
            assert_eq!((a.path.len(), a.detail.offset()), (1, 1), "상세에 포커스가 있는데 {k:?} 가 목록을 나갔다");
            a.key(key(KeyCode::Tab));
            a.key(key(k));
            assert!(a.path.is_empty(), "목록에 포커스가 있는데 {k:?} 가 안 나갔다");
        }
    }

    /// 글을 받는 중에는 `Tab` 이 포커스를 안 옮기고 글자로도 안 들어간다 — 적다
    /// 말고 튀면 적던 것을 잃는다.
    #[test]
    fn tab_does_not_move_the_focus_while_typing() {
        for (opener, start) in [
            (KeyCode::Char('/'), Pane::Explorer),
            (KeyCode::Char('f'), Pane::Explorer),
            (KeyCode::Char('/'), Pane::Detail),
        ] {
            let mut a = app();
            a.focus = start;
            a.key(key(opener));
            a.key(key(KeyCode::Char('a')));
            a.key(key(KeyCode::Tab));
            a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
            assert_eq!(a.focus, start, "{opener:?} 중에 Tab 이 포커스를 옮겼다");
            assert!(matches!(&a.mode, Mode::Grep(b) | Mode::Filter(b) if b.text() == "a"), "{:?}", a.mode);
        }
    }

    /// `j`/`k` 는 **포커스와 상관없이** 상세를 굴린다 — 손에 익은 사람이 이미 있다.
    #[test]
    fn j_and_k_still_scroll_the_detail_from_either_pane() {
        for start in Pane::ALL {
            let mut a = app();
            a.focus = start;
            drawn(&mut a, 10, 40);
            a.key(key(KeyCode::Char('j')));
            a.key(key(KeyCode::Char('j')));
            a.key(key(KeyCode::Char('k')));
            assert_eq!((a.cursor, a.detail.offset(), a.focus), (0, 1, start), "{start:?}");
        }
    }

    /// 잎에서 Enter 는 아무 일도 하지 않는다 — 들어갈 데가 없다.
    #[test]
    fn entering_a_leaf_does_nothing() {
        let mut a = app();
        a.key(key(KeyCode::End)); // 소속 없는 이슈 (잎)
        let before = a.path.clone();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, before);
    }

    /// `..` 에서 Enter 는 나가기다.
    #[test]
    fn entering_the_up_row_leaves() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path.len(), 1);
        a.cursor = 0; // `..`
        a.key(key(KeyCode::Enter));
        assert!(a.path.is_empty());
    }

    #[test]
    fn quitting_works_every_documented_way() {
        for k in [
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            key(KeyCode::Char('q')),
            key(KeyCode::F(10)),
        ] {
            let mut a = app();
            a.key(k);
            assert!(a.quit, "{k:?} 로 못 나갔다");
        }
    }

    /// 뿌리는 `/`, 들어가면 **제목**으로 적는다 — id 를 적으면 사람이 그걸
    /// 다시 찾아봐야 한다.
    #[test]
    fn crumbs_read_as_titles() {
        let mut a = app();
        assert_eq!(a.crumbs(), "/");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.crumbs(), "/argos-0001 제목");
    }

    fn typed(a: &mut App, text: &str) {
        for c in text.chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
    }

    /// `/` 로 검색하면 안 걸린 잎은 사라지고, 걸린 것을 품은 디렉터리는 남는다.
    #[test]
    fn slash_searches_titles() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        assert!(matches!(a.mode, Mode::Grep(_)));
        typed(&mut a, "0004");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(a.filter_text.as_deref(), Some("/0004"));

        // 뿌리에는 그것을 품은 에픽만 남는다
        let ids = shown(&a);
        assert_eq!(ids, ["argos-0001"], "{ids:?}");
        // 그 안에 걸린 것이 있다
        a.key(key(KeyCode::Enter));
        assert_eq!(shown(&a), ["argos-0004"]);
    }

    /// 거름망은 **CLI 와 같은 문법**이다. 없는 항목은 그 자리에서 나무란다.
    #[test]
    fn f_takes_the_same_grammar_as_the_cli() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);

        // 잘못 적으면 걸리지 않고 그 자리에 남는다 — 지우고 다시 치게 하지 않는다
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "statu=todo");
        assert!(matches!(a.mode, Mode::Filter(_)), "잘못 적었는데 넘어갔다");
        assert!(a.input_error().is_some());
        assert_eq!(a.filter_text, None);
    }

    /// **값에 빈칸이 들어간다.** 띄어쓰기로 죄다 쪼개면 `grep=원자적 쓰기` 를
    /// 이 칸에서는 아예 못 적는다 — CLI 는 인자 하나로 받으니 "같은 문법" 이
    /// 아니게 된다. 항목 이름이 시작하는 데서만 쪼갠다.
    #[test]
    fn a_filter_value_may_contain_spaces() {
        let mut issues = vec![make("argos-0001", Kind::Epic), make("argos-0009", Kind::Issue)];
        issues[1].title = "원자적 쓰기를 고친다".into();
        let mut a = App::new(issues, cfg(), Path::new());
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "grep=원자적 쓰기");
        assert!(a.input_error().is_none(), "{:?}", a.input_error());
        assert_eq!(shown(&a), ["argos-0009"]);

        // 여러 조건은 여전히 띄어쓰기로 잇는다
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic status=todo");
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);
    }

    /// **모르는 칸은 거절한다.** `cmd/show.rs` 가 쓰는 것과 같은 자다 — 조용히
    /// 0건을 내면 `status=in-progress` 같은 오타가 "그 칸은 비었다" 와
    /// 구별되지 않는다.
    #[test]
    fn an_unknown_column_is_refused_not_silently_empty() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "status=in-progress");
        assert!(matches!(a.mode, Mode::Filter(_)), "오타인데 걸렸다");
        assert!(a.input_error().is_some_and(|e| e.contains("칸")), "{:?}", a.input_error());
        assert_eq!(a.filter_text, None);
    }

    /// raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. **글을 받는 중에도** 받아야
    /// 한다 — 글자로 먹으면 검색칸에 `c` 가 찍히고 나갈 길이 하나로 줄어든다.
    #[test]
    fn ctrl_c_quits_even_while_typing() {
        for opener in [KeyCode::Char('/'), KeyCode::Char('f')] {
            let mut a = app();
            a.key(key(opener));
            a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(a.quit, "{opener:?} 중에 Ctrl-C 를 글자로 먹었다");
            assert!(matches!(&a.mode, Mode::Grep(b) | Mode::Filter(b) if b.text().is_empty()));
        }
    }

    /// 파일이 **아직 없는** 저장소에서도 생긴 것을 알아챈다. 표식이 없다고
    /// 한 번 걸러 버리면 그 뒤로 영영 못 알아챈다.
    #[test]
    fn a_file_that_appears_later_is_still_noticed() {
        let scratch = Scratch::new("appear");
        let dir = scratch.0.clone();
        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        assert!(stamp.is_none() && load.issues.is_empty(), "판이 다르다");
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);

        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        settle(&mut a);
        assert_eq!(a.issues.len(), 1, "없던 파일이 생긴 것을 못 알아챘다");
        assert!(a.trouble.is_none());
    }

    /// **거름망이 찾으려던 것을 숨기지 않는다.** 저 자신은 안 걸리는 에픽도 걸린
    /// 멤버가 있으면 남는다. `Filter` 의 기본값이 done 을 숨기는 것에 걸려들지 않아야 한다.
    ///
    /// **에픽 자신은 정말로 안 걸려야 한다.** 묶음의 칸은 멤버에서 읽으므로(moai-j3b3)
    /// 적힌 칸을 `done` 에 둬도 멤버가 `todo` 뿐이면 에픽이 `todo` 로 서서 제 손으로
    /// 걸린다 — 그러면 이 시험은 자손으로 남는 길을 한 번도 안 지난다. 끝난 멤버를
    /// 하나 둬 에픽을 `in_progress` 에 세운다.
    #[test]
    fn an_unmatched_epic_does_not_swallow_its_matching_members() {
        let mut closed = member("argos-0002", "argos-0001");
        closed.status = Status::new("done");
        let issues = vec![
            make("argos-0001", Kind::Epic),
            closed,
            member("argos-0003", "argos-0001"),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        assert_eq!(a.column(0), "in_progress", "에픽이 제 손으로 걸린다 — 시험이 자손 길을 안 지난다");
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "status=todo");
        assert_eq!(shown(&a), ["argos-0001"], "안 걸린 에픽이 걸린 멤버를 데리고 사라졌다");
    }

    /// Esc 는 거름망을 푼다. 화면을 끄지는 않는다 — 실수 한 번에 하던 것이
    /// 날아가면 안 된다.
    #[test]
    fn esc_clears_the_filter_but_never_quits() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        typed(&mut a, "0004");
        assert!(a.filter_text.is_some());
        a.key(key(KeyCode::Esc));
        assert_eq!(a.filter_text, None);
        assert!(!a.quit);
        assert_eq!(shown(&a).len(), 3);
    }

    /// 칸이 먹는 키는 탐색기로 새지 않는다. 빈 칸의 Backspace·`←` 가 "한 층
    /// 위로" 로 읽히면 적다 말고 디렉터리를 잃는다. 커서가 글 안으로 들어가
    /// 친 글자가 그 자리에 선다 — 칸이 들고 온 것이다.
    #[test]
    fn keys_the_box_eats_never_reach_the_browser() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        let inside = a.path.clone();
        a.key(key(KeyCode::Char('/')));
        for code in [KeyCode::Backspace, KeyCode::Left, KeyCode::Char('a'), KeyCode::Char('c'), KeyCode::Left] {
            a.key(key(code));
        }
        a.key(key(KeyCode::Char('b')));
        assert_eq!(a.path, inside, "칸의 키가 탐색기로 샜다");
        assert!(matches!(&a.mode, Mode::Grep(b) if b.text() == "abc"), "{:?}", a.mode);
    }

    /// 글을 받는 동안에는 이동키가 글자다 — `q` 를 쳤다고 꺼지면 못 쓴다.
    #[test]
    fn typing_does_not_trigger_browse_keys() {
        let mut a = app();
        a.key(key(KeyCode::Char('/')));
        for c in "quit".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert!(!a.quit);
        assert_eq!(a.mode, Mode::Grep(Input::new("quit")));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.mode, Mode::Browse);
    }

    /// **붙여넣기는 열린 글칸에 글로만 들어간다**(moai-od9q). 키로 읽지 않는다 — 탐색 중에
    /// 붙인 `q` 가 끝내지 않고, 검색칸에 붙인 줄바꿈이 Enter 로 걸리지 않는다. 받을 칸이
    /// 없으면 말없이 삼키지 않고 그렇다고 한다. 해제 물음에 붙인 `y` 는 답이 아니다.
    #[test]
    fn a_paste_lands_in_the_open_field_and_never_acts_as_keys() {
        let mut a = app();
        a.paste("q\n");
        assert!(!a.quit, "탐색 중에 붙인 q 가 끝냈다");
        assert_eq!(a.mode, Mode::Browse);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("붙여")), "받을 칸이 없는데 말이 없다 — {:?}", a.notice);
        a.key(key(KeyCode::Down));
        assert_eq!(a.notice, None, "붙여넣기의 말이 다음 키에 안 걷혔다");

        a.key(key(KeyCode::Char('/')));
        a.paste("qu\tit\n");
        assert_eq!(a.mode, Mode::Grep(Input::new("qu it")), "줄바꿈이 Enter 로 걸렸다");
        a.key(key(KeyCode::Esc));

        a.key(key(KeyCode::Char('f')));
        a.paste("status=todo");
        assert_eq!(a.mode, Mode::Filter(Input::new("status=todo")));
        a.key(key(KeyCode::Esc));

        a.key(key(KeyCode::Char('n')));
        a.paste("제목\t이어\n본문");
        let Mode::Idea(form) = &a.mode else { panic!("폼이 닫혔다 — {:?}", a.mode) };
        assert_eq!((form.title.text(), form.field), ("제목 이어 본문", super::form::Field::Title));

        a.mode = Mode::Unregister(register::Unregister { path: "/w/one".into(), name: "one".into() });
        a.paste("y");
        assert_eq!(a.mode, Mode::Browse, "해제 물음이 안 거둬졌다");
    }

    /// 들고 있던 길이 사라지면 **갈 수 있는 데까지만** 남긴다. 없는 자리에 서
    /// 있으면 빈 목록이 나오고, 사람은 자료가 사라진 줄 안다.
    #[test]
    fn reload_repairs_a_path_that_no_longer_exists() {
        let mut a = app();
        a.key(key(KeyCode::Enter)); // 에픽 안으로
        assert_eq!(a.path.len(), 1);

        // 그 에픽이 사라진 자료로 갈아탄다
        a.adopt(vec![make("argos-0002", Kind::Epic), make("argos-0009", Kind::Issue)]);
        assert!(a.path.is_empty(), "없는 자리에 그대로 서 있다");
        assert_eq!(a.cursor, 0);
        assert!(!a.rows().is_empty());
    }

    /// 마일스톤이 **처음 생기면** 트리가 한 층 깊어져 경로가 통째로 낡는다.
    /// 그래도 **서 있던 것이 아직 있으면 따라간다** — 자리만 바뀐 것을 지워진
    /// 것과 같이 다루면, 가장 흔한 갱신이 하필 자리를 가장 크게 잃는다.
    #[test]
    fn a_first_milestone_deepens_the_tree_and_the_cursor_follows_the_epic() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        let was = a.path.clone();

        let mut with = a.issues.clone();
        with.push(make("argos-9999", Kind::Milestone));
        a.adopt(with);
        // 길은 낡았지만 뿌리로 내려놓지는 않는다 — 에픽이 간 자리로 따라간다
        assert_ne!(a.path, was, "트리가 깊어졌는데 길이 그대로다");
        assert_eq!(a.path.last(), was.last(), "서 있던 에픽을 놓쳤다");
        assert_eq!(a.path.len(), 2, "{:?}", a.path);
        assert_eq!(a.remembered.len(), a.path.len());
    }

    /// 갱신해도 걸어 둔 거름망은 살아 있다. 갱신 한 번에 하던 일이 흩어지면
    /// F5 를 안 누르게 되고, 그러면 낡은 화면을 본다.
    #[test]
    fn reloading_keeps_the_filter() {
        let mut a = app();
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        assert_eq!(shown(&a).len(), 2);

        let mut more = a.issues.clone();
        more.push(make("argos-0007", Kind::Epic));
        a.adopt(more);
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a).len(), 3, "거름망이 새 자료에 다시 걸리지 않았다");
    }

    /// 진짜 파일을 두고 **바뀐 것을 알아채고 저절로 다시 읽는지** 본다. 고친 때만
    /// 보면 rename 으로 갈아끼우는 쓰기를 같은 초에 놓치므로 길이도 함께 본다.
    /// 다시 읽어도 커서는 보던 줄에 남는다.
    #[test]
    fn it_notices_a_changed_file_and_rereads_it() {
        let scratch = Scratch::new("reload");
        let dir = scratch.0.clone();
        let line = |i: &Issue| format!("{}\n", serde_json::to_string(i).unwrap());
        std::fs::write(dir.join(".moai/issues.jsonl"), line(&make("argos-0001", Kind::Epic))).unwrap();

        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);
        assert_eq!(a.issues.len(), 1);

        // 아직 아무도 안 건드렸다 — 읽지 않는다. 읽으면 표식이 같아도 매 걸음
        // 저장소를 통째로 다시 세는 것이다.
        let stamp_before = a.stamp;
        a.follow();
        assert!(!a.loading(), "안 바뀌었는데 다시 읽으러 갔다");
        assert_eq!(a.stamp, stamp_before);

        // 에픽 안에 들어가 있는 동안 밖에서 한 줄 더한다
        a.key(key(KeyCode::Enter));
        let mut src = std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap();
        src.push_str(&line(&member("argos-0004", "argos-0001")));
        std::fs::write(dir.join(".moai/issues.jsonl"), src).unwrap();

        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 읽으러 가지 않았다");
        assert_eq!(a.issues.len(), 1, "스레드가 지을 것을 루프에서 읽었다");
        settle(&mut a);
        assert_eq!(a.issues.len(), 2, "바뀐 것을 저절로 안 읽었다");
        assert_eq!(a.path, [Seg::Epic("argos-0001".into())], "읽고 나서 자리를 잃었다");
        assert!(a.trouble.is_none());
    }

    /// **다시 읽어도 커서는 보던 줄에 선다.** 위에 줄이 생기거나 사라져도, 칸이
    /// 바뀌어 차례가 달라져도 번호가 아니라 그 줄을 따라간다. 보던 줄이 사라지면
    /// 목록 밖으로 나가지 않는 이웃 자리에 선다.
    #[test]
    fn reloading_keeps_the_cursor_on_the_same_line() {
        let mut a = app();
        a.key(key(KeyCode::Enter)); // argos-0001 안 — `..`, 0003, 0004
        a.key(key(KeyCode::End));
        let at = |a: &App| match a.current() {
            Some(Row::Item(e)) => a.issues[e.at().unwrap()].id.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(at(&a), "argos-0004");

        // 위에 줄이 하나 생긴다 — 같은 번호는 이제 0003 이다.
        let mut more = a.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(at(&a), "argos-0004", "위에 줄이 생기자 커서가 옆 줄로 튀었다");

        // 위의 줄이 사라진다.
        let fewer: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0000" && i.id != "argos-0003").cloned().collect();
        a.adopt(fewer);
        assert_eq!(at(&a), "argos-0004", "위의 줄이 사라지자 커서가 튀었다");

        // 보던 줄이 사라지면 목록 안의 이웃 자리로.
        let gone: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert!(a.cursor < a.rows().len(), "목록 밖에 섰다");

        // `..` 에 서 있으면 `..` 에 남는다.
        let mut b = app();
        b.key(key(KeyCode::Enter));
        let mut more = b.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        b.adopt(more);
        assert_eq!(b.current(), Some(Row::Up));
    }

    /// 다시 읽어도 **같은 줄을 보고 있으면 굴린 자리를 둔다.** 보던 줄이 사라져
    /// 다른 줄에 서면 첫 줄부터 보인다.
    #[test]
    fn reloading_keeps_the_scroll_only_on_the_same_line() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        a.key(key(KeyCode::End)); // argos-0004
        drawn(&mut a, 10, 40);
        a.detail.by(7);
        let mut more = a.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(a.detail.offset(), 7, "같은 줄인데 굴린 자리를 잃었다");

        let gone: Vec<Issue> = a.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert_eq!(a.detail.offset(), 0, "다른 줄에 섰는데 굴린 자리가 남았다");
    }

    /// 겹쳐 보는 동안에는 **옆 워크트리 스냅샷이 바뀐 것도** 다시 읽을 까닭이다.
    /// 안 바뀌었으면 읽지 않는다.
    #[test]
    fn a_change_in_a_watched_worktree_snapshot_rereads_too() {
        let scratch = Scratch::new("watched");
        let dir = scratch.0.clone();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);

        let other = dir.join("other.jsonl");
        std::fs::write(&other, "").unwrap();
        a.watched = vec![(other.clone(), crate::store::stamp(&other))];
        a.now = "읽기 전".into();
        settle(&mut a);
        assert_eq!(a.now, "읽기 전", "아무것도 안 바뀌었는데 다시 읽었다");

        std::fs::write(&other, "{}\n").unwrap();
        settle(&mut a);
        assert_ne!(a.now, "읽기 전", "옆 스냅샷이 바뀐 것을 못 알아챘다");
    }

    /// **사람이 누른 갱신이 스레드의 늦은 결과에 덮이지 않는다.** F5 를 누르기 전에
    /// 띄운 읽기는 누른 뒤의 파일보다 옛것일 수 있다.
    #[test]
    fn a_manual_reload_drops_the_read_in_flight() {
        let scratch = Scratch::new("inflight");
        let dir = scratch.0.clone();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        let repo = Repo { root: dir.clone(), config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);

        std::fs::write(dir.join(".moai/issues.jsonl"), format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap())).unwrap();
        a.follow();
        assert!(a.loading());
        a.key(key(KeyCode::F(5)));
        assert!(!a.loading(), "F5 가 짓던 것을 안 버렸다");
        assert_eq!(a.issues.len(), 1);
    }

    /// **다른 프로젝트에서 지은 읽기는 안 들인다.** 같은 id 를 쓰는 두 프로젝트에서
    /// 떠난 쪽의 읽기가 늦게 닿으면, 들이는 순간 지금 프로젝트의 화면이 남의 줄이 된다.
    #[test]
    fn a_read_built_for_another_project_is_not_taken() {
        let (mine_dir, mut a) = writable("receive-mine");
        let (theirs, _) = writable("receive-theirs");
        touch_outside(&theirs);
        let other = Repo { root: theirs.0.clone(), config: cfg() };
        a.receive(prepare(&other, false));
        assert_eq!(shown(&a), ["argos-0001"], "남의 프로젝트에서 지은 줄을 들였다");
        assert!(a.trouble.is_none());

        // 제 것은 들인다 — 막은 것이 뿌리 견주기이지 받기 자체가 아니다.
        let mine = a.repo.clone().unwrap();
        touch_outside(&mine_dir);
        a.receive(prepare(&mine, false));
        assert_eq!(a.issues.len(), 2);
    }

    /// 판 밖에서 한 줄을 더해 다음 `follow` 가 스레드 읽기를 띄우게 한다.
    fn touch_outside(scratch: &Scratch) {
        let file = scratch.0.join(".moai/issues.jsonl");
        let mut src = std::fs::read_to_string(&file).unwrap();
        src.push_str(&format!("{}\n", serde_json::to_string(&make("argos-0003", Kind::Issue)).unwrap()));
        std::fs::write(&file, src).unwrap();
    }

    /// 버린 스레드가 **다 끝날 때까지** 기다린다. join 은 하지 않는다 — 그것은 `follow` 의 일이다.
    fn discarded_settle(a: &App) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !a.discarded.iter().all(|h| h.is_finished()) {
            assert!(std::time::Instant::now() < until, "버린 읽기가 끝나지 않는다");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    fn boom(_: &Repo, _: bool) -> crate::fail::R<Fresh> {
        panic!("버린 읽기가 터졌다")
    }

    /// **F5 가 버린 읽기가 패닉하면 다음 걸음이 되던진다.** 패닉 훅은 이미 터미널을
    /// 걷었다 — 손잡이를 같이 버리면 루프는 걷힌 화면에 모른 채 그린다(1b63abe 가
    /// 받은 스레드에만 막은 구멍).
    #[test]
    fn a_discarded_read_that_panics_is_rethrown_at_the_next_step() {
        let (scratch, mut a) = writable("discard-panic");
        touch_outside(&scratch);
        a.read = boom;
        a.follow();
        assert!(a.loading());
        // 사람이 누른 갱신은 진짜 길로 읽는다 — 터지는 것은 버린 스레드뿐이다.
        a.read = prepare;
        a.key(key(KeyCode::F(5)));
        assert!(!a.loading(), "F5 가 짓던 것을 안 버렸다");
        assert!(a.reaping(), "버린 손잡이를 안 들었다 — 루프가 빠른 걸음으로 안 깬다");
        assert_eq!(a.issues.len(), 2, "F5 가 제 자리에서 안 읽었다");

        discarded_settle(&a);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.follow()));
        let payload = caught.expect_err("버린 스레드의 패닉을 삼켰다");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"버린 읽기가 터졌다"));
    }

    /// **버린 읽기가 제대로 끝나면 join 만 하고 결과는 안 들인다.** 다시 읽으러 가지도
    /// 않는다 — F5 가 표식을 이미 올렸다.
    #[test]
    fn a_discarded_read_that_finishes_is_joined_and_ignored() {
        let (scratch, mut a) = writable("discard-ok");
        touch_outside(&scratch);
        a.follow();
        assert!(a.loading());
        a.key(key(KeyCode::F(5)));
        assert!(a.reaping());
        let stamp = a.stamp;

        discarded_settle(&a);
        a.follow();
        assert!(!a.reaping(), "끝난 스레드를 join 안 했다");
        assert!(!a.loading(), "버린 읽기가 끝난 것을 보고 또 읽으러 갔다");
        assert_eq!(a.stamp, stamp);
        assert_eq!(a.issues.len(), 2);
        assert!(a.trouble.is_none());
    }

    /// **든 손잡이는 [`DISCARDED_KEPT`] 개를 안 넘는다.** 안 끝나는 스레드를 기다리면
    /// 루프가 같이 멈추므로 가장 오래된 것을 놓는다. 끝나지 않은 것은 `follow` 가
    /// 기다리지 않는다.
    #[test]
    fn kept_discarded_reads_are_bounded() {
        let (scratch, mut a) = writable("discard-bound");
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
        for _ in 0..DISCARDED_KEPT {
            let rx = rx.clone();
            a.discarded.push(std::thread::spawn(move || {
                let _ = rx.lock().map(|r| r.recv());
            }));
        }
        a.follow();
        assert_eq!(a.discarded.len(), DISCARDED_KEPT, "안 끝난 것을 기다리거나 버렸다");

        touch_outside(&scratch);
        a.follow();
        assert!(a.loading());
        a.key(key(KeyCode::F(5)));
        assert_eq!(a.discarded.len(), DISCARDED_KEPT, "든 손잡이가 상한을 넘었다");
        // **도는 것을 놓았으면 화면이 말한다**(moai-j9on) — 그 스레드가 터지면 터미널이
        // 걷히는데 되던질 손잡이가 없다. 다시 읽기가 걷는 `trouble` 이 아니라 붙박이다:
        // F5 가 짓는 읽기가 끝나는 순간 걷히면 몇백 ms 뒤에 사라진다.
        assert_eq!(a.let_go, 1);
        let said = super::draw::tests_banner(&mut a);
        assert!(said.contains("다시 읽기 1개를 놓았다"), "도는 스레드를 말없이 놓았다 — {said:?}");

        drop(tx);
        discarded_settle(&a);
        a.follow();
        assert!(!a.reaping());
        settle(&mut a);
        let said = super::draw::tests_banner(&mut a);
        assert!(said.contains("다시 읽기 1개를 놓았다"), "다시 읽기가 놓은 것의 말을 걷었다 — {said:?}");
        // 붙박이라 **쓰기의 알림 뒤에** 선다 — 앞에 서면 세션 내내 80칸에서 알림을 밀어낸다.
        a.notice = Some("✓ 담음".into());
        let said = super::draw::tests_banner(&mut a);
        assert!(said.find("✓ 담음") < said.find("다시 읽기 1개"), "놓은 것의 말이 알림을 앞질렀다 — {said:?}");
    }

    /// **꽉 찬 채로 버릴 때 가장 오래된 것이 그새 패닉으로 끝났으면 놓지 않고 되던진다.**
    /// 지난 걸음의 거두기는 그것이 살아 있을 때 지나갔다 — 거두지 않고 놓으면 그 패닉만
    /// 걷힌 화면 뒤로 사라진다.
    #[test]
    fn a_full_discard_list_reaps_before_it_lets_go_of_the_oldest() {
        let (_scratch, mut a) = writable("discard-full-panic");
        a.discarded.push(std::thread::spawn(|| panic!("가장 오래된 것이 터졌다")));
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
        for _ in 1..DISCARDED_KEPT {
            let rx = rx.clone();
            a.discarded.push(std::thread::spawn(move || {
                let _ = rx.lock().map(|r| r.recv());
            }));
        }
        while !a.discarded[0].is_finished() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        // `follow` 를 거치지 않고 짓던 읽기를 세운다 — 거기서 거두면 이 틈을 못 본다.
        let (ptx, prx) = std::sync::mpsc::channel();
        a.pending = Some((prx, std::thread::spawn(move || drop(ptx))));

        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.key(key(KeyCode::F(5)))));
        drop(tx);
        let payload = caught.expect_err("꽉 찼을 때 끝난 패닉을 거두지 않고 놓았다");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"가장 오래된 것이 터졌다"));
    }

    /// 쓰기 시험이 쓰는 판 — 진짜 파일에 줄 하나를 두고 연 탐색기. **사람은
    /// `--user` 로 준다** — `MOAI_ACTOR` 를 시험에서 바꾸면 같은 프로세스의 다른
    /// 시험이 그 값을 본다.
    fn writable(name: &str) -> (Scratch, App) {
        let scratch = Scratch::new(name);
        let dir = scratch.0.clone();
        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        let repo = Repo { root: dir, config: cfg() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, Path::new(), stamp);
        a.user = Some("레이븐 (raven@example.com)".into());
        (scratch, a)
    }

    /// 생각 하나를 담는 쓰기 — 폼이 부를 모양 그대로다.
    fn add_idea(a: &mut App, id: &'static str) -> Option<String> {
        a.write(|_| {}, move |issues, _, _, by| {
            issues.push(Issue::new(id.into(), "떠오른 것".into(), Kind::Idea, Status::new("todo"), "2026-09-13T00:00:00Z"));
            Ok((vec![crate::model::JournalEntry::create(id, "떠오른 것", "2026-09-13T00:00:00Z", by)], Touched { id: id.into(), done: "담김" }))
        })
    }

    /// **쓰면 파일이 바뀌고, 화면은 그 파일을 다시 읽은 것이다.** 손으로 넣은 것이
    /// 아니라는 증거가 저널과 표식이다 — 표식이 그대로면 다음 걸음이 제가 쓴 것을
    /// "밖에서 바뀌었다" 로 읽고 한 번 더 읽는다.
    #[test]
    fn a_write_lands_in_the_file_and_the_screen_rereads_it() {
        let (scratch, mut a) = writable("write");
        let file = scratch.0.join(".moai/issues.jsonl");
        a.trouble = Some("다시 읽지 못했다 — 옛 까닭".into());

        assert_eq!(add_idea(&mut a, "argos-0002").as_deref(), Some("argos-0002"));
        assert!(std::fs::read_to_string(&file).unwrap().contains("argos-0002"), "파일에 안 닿았다");
        assert_eq!(a.issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["argos-0001", "argos-0002"]);
        assert_eq!(a.index.find("argos-0002"), Some(1), "색인이 다시 안 섰다 — 손으로 넣은 것이다");
        let repo = a.repo.clone().unwrap();
        assert_eq!(repo.journal_of("argos-0002").unwrap()[0].by, "레이븐");
        assert!(a.trouble.is_none(), "다시 읽었는데 옛 까닭이 남았다");

        assert_eq!(a.stamp, stamp_of(&repo), "표식을 다시 안 잡았다");
        a.follow();
        assert!(!a.loading(), "제가 쓴 것을 밖에서 바뀐 것으로 읽었다");
    }

    /// 커서가 선 줄의 id. `..` 이나 바구니면 없다.
    fn on(a: &App) -> Option<String> {
        a.current().and_then(|r| match r {
            Row::Item(e) => e.at().map(|at| a.issues[at].id.clone()),
            Row::Up | Row::Project(_) => None,
        })
    }

    /// **쓰고 나면 만든 줄에 커서가 서고, 한 줄 알림이 그 id 를 댄다.** 담긴 것이
    /// 눈앞에 보여야 담긴 줄 안다. 알림은 다음 키 하나에 걷힌다.
    #[test]
    fn a_write_puts_the_cursor_on_the_line_it_made_and_names_it() {
        let (_scratch, mut a) = writable("land");
        assert_eq!(on(&a).as_deref(), Some("argos-0001"));
        drawn(&mut a, 10, 40);
        a.detail.by(5);

        assert_eq!(add_idea(&mut a, "argos-0002").as_deref(), Some("argos-0002"));
        assert_eq!(on(&a).as_deref(), Some("argos-0002"), "만든 줄에 안 섰다 — {:?}", a.rows());
        assert_eq!(a.notice.as_deref(), Some("✓ 담김 · argos-0002"));
        assert_eq!(a.detail.offset(), 0, "다른 줄에 섰는데 굴린 자리가 남았다");
        assert!(a.trouble.is_none());

        a.key(key(KeyCode::Down));
        assert_eq!(a.notice, None, "다음 키에 알림이 안 걷혔다");
    }

    /// **쓴 줄이 다른 디렉터리에 서면 그리로 간다.** 에픽 안에서 담은 생각은 뿌리에
    /// 서고, 뿌리에서 만든 멤버는 에픽 안에 선다. 들어간 층에서 나오면 그 에픽에 선다.
    #[test]
    fn a_line_written_into_another_directory_takes_the_cursor_there() {
        let (_scratch, mut a) = writable("land-elsewhere");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.path, [Seg::Epic("argos-0001".into())]);

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert!(a.path.is_empty(), "생각은 에픽 밖에 서는데 에픽 안에 남았다 — {:?}", a.path);
        assert_eq!(on(&a).as_deref(), Some("argos-0002"));
        assert!(a.remembered.is_empty());

        let wrote = a.write(|_| {}, |issues, _, _, by| {
            let at = "2026-09-13T00:00:00Z";
            issues.push(member("argos-0003", "argos-0001"));
            Ok((vec![crate::model::JournalEntry::create("argos-0003", "멤버", at, by)], Touched { id: "argos-0003".into(), done: "만듦" }))
        });
        assert_eq!(wrote.as_deref(), Some("argos-0003"));
        assert_eq!(a.path, [Seg::Epic("argos-0001".into())], "에픽 안의 줄인데 그리로 안 갔다");
        assert_eq!(on(&a).as_deref(), Some("argos-0003"));
        assert_eq!(a.remembered.len(), a.path.len());
        assert_eq!(a.notice.as_deref(), Some("✓ 만듦 · argos-0003"));

        a.key(key(KeyCode::Backspace));
        assert_eq!(on(&a).as_deref(), Some("argos-0001"), "나오니 들어간 에픽에 안 섰다");
    }

    /// **거름망이 새 줄을 가리면 말한다.** 조용히 안 보이면 저장이 실패한 것으로
    /// 읽힌다. 커서도 길도 거름망도 그대로다 — 사람이 건 것을 대신 풀지 않는다.
    #[test]
    fn a_filter_hiding_the_new_line_is_said_not_silent() {
        let (_scratch, mut a) = writable("land-hidden");
        a.key(key(KeyCode::Char('f')));
        typed(&mut a, "type=epic");
        let (path, cursor) = (a.path.clone(), a.cursor);

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert_eq!(a.issues.len(), 2, "쓰기가 안 닿았다");
        assert_eq!((a.path.clone(), a.cursor), (path, cursor), "가려진 줄을 찾아 자리를 옮겼다");
        assert_eq!(on(&a).as_deref(), Some("argos-0001"));
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"), "거름망을 대신 풀었다");
        assert_eq!(a.notice.as_deref(), Some("✓ 담김 · argos-0002 — 거름망에 가려 안 보인다 · Esc 로 푼다"));
    }

    /// 닫는 함수가 댄 id 가 다시 읽은 목록에 없으면 **커서는 두고 그렇다고 말한다.**
    #[test]
    fn a_touched_id_missing_after_the_reread_moves_nothing() {
        let (_scratch, mut a) = writable("land-missing");
        let wrote = a.write(|_| {}, |_, _, _, _| Ok((vec![], Touched { id: "argos-9999".into(), done: "담김" })));
        assert_eq!(wrote.as_deref(), Some("argos-9999"));
        assert_eq!((a.cursor, on(&a).as_deref()), (0, Some("argos-0001")));
        assert_eq!(a.notice.as_deref(), Some("✓ 담김 · argos-9999 — 다시 읽은 목록에 없다"));
    }

    /// **쓰기 전에 띄운 다시 읽기는 버린다.** 늦게 닿으면 방금 쓴 것을 옛 화면으로
    /// 덮는다. 쓰기는 락 안에서 파일을 다시 읽으므로 밖에서 떨어진 줄도 잃지 않는다.
    #[test]
    fn a_write_drops_the_read_in_flight_and_keeps_the_outside_change() {
        let (scratch, mut a) = writable("write-inflight");
        let file = scratch.0.join(".moai/issues.jsonl");
        let mut src = std::fs::read_to_string(&file).unwrap();
        src.push_str(&format!("{}\n", serde_json::to_string(&make("argos-0003", Kind::Issue)).unwrap()));
        std::fs::write(&file, src).unwrap();
        a.follow();
        assert!(a.loading(), "판이 다르다 — 밖의 쓰기를 못 봤다");

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert!(!a.loading(), "쓰기 전에 띄운 읽기가 남았다");
        assert_eq!(a.issues.len(), 3, "밖에서 떨어진 줄이나 제가 쓴 줄을 잃었다");
        settle(&mut a);
        assert_eq!(a.issues.len(), 3);
    }

    /// **실패는 화면에 선다.** 검증에 걸리면 파일도 화면도 그대로고, 까닭이 남는다 —
    /// 다시 읽으면 그 까닭이 지워지므로 실패한 뒤에는 읽지 않는다.
    #[test]
    fn a_refused_write_says_why_and_touches_nothing() {
        let (scratch, mut a) = writable("write-refused");
        let file = scratch.0.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        let stamp = a.stamp;
        // 앞 쓰기의 알림이 남아 있으면 실패한 이번 쓰기가 담긴 것으로 읽힌다.
        a.notice = Some("✓ 담김 · argos-0000".into());

        let out = a.write(|_| {}, |issues, _, _, _| {
            issues.push(Issue::new("argos-0002".into(), "t".into(), Kind::Idea, Status::new("없는칸"), "2026-09-13T00:00:00Z"));
            Ok((vec![], Touched { id: "argos-0002".into(), done: "담김" }))
        });
        assert!(out.is_none(), "거절됐는데 썼다고 한다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        assert_eq!(a.issues.len(), 1);
        assert_eq!(a.stamp, stamp);
        let t = a.trouble.clone().unwrap_or_default();
        assert!(t.starts_with("쓰지 못했다") && t.contains("칸"), "{t}");
        assert_eq!(a.notice, None, "실패했는데 앞 쓰기의 알림이 남았다");
        assert_eq!(a.cursor, 0);

        // 여러 줄 거절문(고칠 명령까지 내는 것)은 배너 한 줄로 이어진다.
        let out = a.write(|_| {}, |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> {
            Err("첫 줄\n      고칠 명령".into())
        });
        assert!(out.is_none());
        assert_eq!(a.trouble.as_deref(), Some("쓰지 못했다 — 첫 줄  고칠 명령"));
    }

    /// **쓰기의 실패는 저절로 다시 읽기에 지워지지 않는다.** 락을 못 잡은 때가 곧
    /// 남이 쓰고 있던 때라, 실패 바로 다음 걸음이 그 쓰기를 보고 다시 읽는다 — 그
    /// 읽기가 까닭을 지우면 폼은 열린 채인데 왜 안 닫혔는지 아무 데도 없다. 다음
    /// 쓰기가 성공하면 그때 걷힌다.
    #[test]
    fn a_failed_write_survives_the_background_reread() {
        let (scratch, mut a) = writable("write-sticky");
        let file = scratch.0.join(".moai/issues.jsonl");
        let out = a.write(|_| {}, |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> { Err("락".into()) });
        assert!(out.is_none());

        let mut src = std::fs::read_to_string(&file).unwrap();
        src.push_str(&format!("{}\n", serde_json::to_string(&make("argos-0003", Kind::Issue)).unwrap()));
        std::fs::write(&file, src).unwrap();
        settle(&mut a);
        assert_eq!(a.issues.len(), 2, "밖의 쓰기를 못 읽었다");
        assert_eq!(a.trouble.as_deref(), Some("쓰지 못했다 — 락"), "저절로 다시 읽기가 쓰기의 까닭을 지웠다");

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert!(a.trouble.is_none(), "쓰기가 성공했는데 옛 까닭이 남았다");
    }

    /// **준 사람의 모양이 틀리면 묻지 않고 말한다.** `--user "이름만"` 은 사람이 준
    /// 것이라 조용히 갈아 치울 일이 아니다 — 묻는 것은 아무것도 없을 때뿐이다. 락도
    /// 잡기 전이라 닫는 함수는 불리지도 않는다.
    #[test]
    fn a_malformed_user_stops_the_write_on_screen_without_asking() {
        let (scratch, mut a) = writable("write-actor");
        let file = scratch.0.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.user = Some("이름만".into());

        let out = a.write(|_| {}, |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> {
            panic!("누군지 모르는데 닫는 함수를 불렀다")
        });
        assert!(out.is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        let t = a.trouble.clone().unwrap_or_default();
        assert!(t.starts_with("쓰지 못했다") && !t.contains('\n'), "{t:?}");
        assert_eq!(a.mode, Mode::Browse, "사람이 준 것을 두고 또 물었다");
    }

    /// 누군지 모르는 기계. **이 기계의 git 설정도 `MOAI_ACTOR` 도 안 본다** — 준 것만
    /// 푼다. 진짜 길(`model::actor`)을 쓰면 이 시험들이 돌리는 사람의 설정에 달린다.
    fn nobody(user: Option<&str>) -> crate::fail::R<crate::model::Actor> {
        match user {
            Some(raw) => crate::model::actor(Some(raw)),
            None => Err(crate::fail::Fail::coded("누가 하는지 모른다 — 시험\n\n  고칠 명령", crate::fail::code::NO_ACTOR)),
        }
    }

    fn type_in(a: &mut App, text: &str) {
        for c in text.chars() {
            a.key(key(KeyCode::Char(c)));
        }
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// 파일에 선 생각들 — 화면이 아니라 **파일을** 읽는다.
    fn ideas_in(repo: &Repo) -> Vec<Issue> {
        repo.read().unwrap().issues.into_iter().filter(|i| i.kind == Kind::Idea).collect()
    }

    /// `n` 으로 폼을 열어 제목을 적는다.
    fn jotting(a: &mut App, title: &str) {
        a.key(key(KeyCode::Char('n')));
        type_in(a, title);
    }

    /// **`n` 은 어디를 보고 있든 같은 빈 폼을 연다** — 목록에서도 상세에서도 에픽 안에서도.
    /// 글을 받는 중이면 `n` 은 글자다. 폼 안의 포커스는 탐색기의 포커스와 따로라, 닫으면
    /// 보던 칸이 그대로 있다.
    #[test]
    fn n_opens_the_form_from_anywhere_but_is_a_letter_while_typing() {
        for (inside, focus) in [(false, Pane::Explorer), (true, Pane::Explorer), (true, Pane::Detail)] {
            let mut a = app();
            if inside {
                a.key(key(KeyCode::Enter));
            }
            a.focus = focus;
            a.key(key(KeyCode::Char('n')));
            assert_eq!(a.mode, Mode::Idea(Form::default()), "{inside} {focus:?}");
            a.key(key(KeyCode::Tab));
            assert_eq!(a.focus, focus, "폼의 Tab 이 탐색기 포커스를 옮겼다");
            a.key(key(KeyCode::Esc));
            assert_eq!((&a.mode, a.focus), (&Mode::Browse, focus));
        }
        for opener in [KeyCode::Char('/'), KeyCode::Char('f')] {
            let mut a = app();
            a.key(key(opener));
            a.key(key(KeyCode::Char('n')));
            assert!(matches!(&a.mode, Mode::Grep(q) | Mode::Filter(q) if q.text() == "n"), "{:?}", a.mode);
        }
    }

    /// **담으면 파일에 idea 로 선다 — 에픽 없이.** 커서가 에픽 안에 있어도 거기 넣지 않는다.
    /// 만드는 길은 CLI 와 같다: 첫 칸, 만든 사람이 담당, 저널 `create` 한 줄. 본문은 여러
    /// 줄 그대로, 끝의 빈 줄은 뗀다. 담으면 폼이 닫힌다.
    #[test]
    fn ctrl_s_saves_an_idea_without_an_epic_wherever_the_cursor_is() {
        let (_scratch, mut a) = writable("jot");
        a.key(key(KeyCode::Enter)); // 에픽 안
        assert_eq!(a.path.len(), 1, "판이 다르다 — 에픽 안에 못 들어갔다");
        jotting(&mut a, "  반짝 떠오른 것 ");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "첫 줄");
        a.key(key(KeyCode::Enter));
        type_in(&mut a, "둘째 줄");
        a.key(key(KeyCode::Enter));
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "담았는데 폼이 안 닫혔다 — {:?}", a.trouble);

        let repo = a.repo.clone().unwrap();
        let made = ideas_in(&repo);
        assert_eq!(made.len(), 1, "{made:?}");
        let idea = &made[0];
        assert_eq!(idea.title, "반짝 떠오른 것");
        assert_eq!(idea.body.as_deref(), Some("첫 줄\n둘째 줄"));
        assert_eq!((idea.epic.as_deref(), idea.milestone.as_deref()), (None, None), "커서가 선 에픽에 넣었다");
        assert_eq!(idea.status.as_str(), "todo");
        assert_eq!((idea.assignee.as_deref(), idea.assignee_email.as_deref()), (Some("레이븐"), Some("raven@example.com")));
        assert!(idea.id.starts_with("argos-"), "{}", idea.id);
        let journal = repo.journal_of(&idea.id).unwrap();
        assert_eq!(journal.len(), 1);
        assert_eq!((journal[0].kind.as_str(), journal[0].title.as_deref(), journal[0].by.as_str()), ("create", Some("반짝 떠오른 것"), "레이븐"));
        // 화면은 파일을 다시 읽은 것이고, 커서는 만든 줄에 서며 알림은 하나다 — 폼은 닫기만
        // 하고 뒤처리는 `write` 가 한다(moai-064q). idea 는 에픽에 안 드니 뿌리로 나온다.
        assert!(a.index.find(&idea.id).is_some(), "쓰고 다시 안 읽었다");
        assert_eq!((on(&a), a.path.len()), (Some(idea.id.clone()), 0), "만든 줄에 안 섰다");
        assert_eq!(a.notice, Some(format!("✓ 담김 · {}", idea.id)));

        // F2 도 담는다. 제목만으로 된다.
        jotting(&mut a, "하나 더");
        a.key(key(KeyCode::F(2)));
        assert_eq!(a.mode, Mode::Browse);
        let made = ideas_in(&repo);
        assert!(made.iter().any(|i| i.title == "하나 더" && i.body.is_none()), "{made:?}");
    }

    /// **제목이 비면 담지 않는다. 그 밖에는 아무것도 안 묻는다** — 누군지도 안 푼다.
    /// 파일은 그대로고 폼은 까닭을 달고 제목 칸에 선다.
    #[test]
    fn an_empty_title_is_refused_in_place_and_nothing_is_asked() {
        fn refuse(_: Option<&str>) -> crate::fail::R<crate::model::Actor> {
            panic!("빈 제목인데 누군지 물었다")
        }
        let (scratch, mut a) = writable("jot-empty");
        let file = scratch.0.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.identify = refuse;
        jotting(&mut a, "   ");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "본문만 있다");
        a.key(ctrl('s'));
        let Mode::Idea(form) = &a.mode else { panic!("빈 제목에 폼이 닫혔다 — {:?}", a.mode) };
        assert_eq!((form.error.as_deref(), form.field), (Some(form::EMPTY_TITLE), form::Field::Title));
        assert_eq!(form.body.text(), "본문만 있다", "거절하며 적은 것을 지웠다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        assert!(a.trouble.is_none(), "빈 제목은 쓰기의 실패가 아니다 — {:?}", a.trouble);
    }

    /// **담을 곳 없이 연 폼은 저장소가 있어도 안 쓴다** — 쓰는 곳은 폼이 박은 곳이지 지금
    /// `repo` 가 아니다(moai-fccv). `n` 은 늘 담을 곳을 박으므로 이것은 속을 직접 세운 폼이다.
    #[test]
    fn a_form_without_a_target_never_writes() {
        let (scratch, mut a) = writable("jot-untargeted");
        let file = scratch.0.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        jotting(&mut a, "여기");
        assert!(matches!(&a.mode, Mode::Idea(f) if f.into.as_ref().is_some_and(|t| t.path == scratch.0)), "{:?}", a.mode);
        a.mode = Mode::Idea(Form { title: Input::new("어디에도"), ..Form::default() });
        a.key(ctrl('s'));
        assert!(matches!(a.mode, Mode::Idea(_)), "{:?}", a.mode);
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다")), "{:?}", a.trouble);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    }

    /// **Esc 는 빈 폼을 곧바로 닫고, 적던 것이 있으면 한 번 묻는다.** `y` 만 버린다 —
    /// 다른 키는 폼으로 돌아가고 적던 것은 그대로다.
    #[test]
    fn esc_closes_a_blank_form_at_once_and_asks_before_dropping_what_was_typed() {
        let mut a = app();
        a.key(key(KeyCode::Char('n')));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.mode, Mode::Browse, "빈 폼인데 물었다");

        jotting(&mut a, "적던 것");
        a.key(key(KeyCode::Esc));
        assert!(matches!(&a.mode, Mode::Idea(f) if f.leaving), "적던 것이 있는데 한 키에 닫혔다 — {:?}", a.mode);
        a.key(key(KeyCode::Char('q')));
        assert!(!a.quit, "묻는 중의 q 로 꺼졌다");
        assert!(matches!(&a.mode, Mode::Idea(f) if !f.leaving && f.title.text() == "적던 것"), "{:?}", a.mode);
        a.key(key(KeyCode::Esc));
        a.key(key(KeyCode::Char('y')));
        assert_eq!(a.mode, Mode::Browse);
        assert!(!a.quit);
    }

    /// **쓰기가 실패하면 폼은 적던 그대로 열려 있고 까닭이 배너에 선다.** 고치고 다시
    /// 누르면 담긴다. 실패한 채로 버리고 닫으면 그 까닭도 걷힌다 — 가리키던 폼이 없다.
    /// 읽기의 실패는 폼을 닫아도 남는다.
    #[test]
    fn a_failed_save_keeps_the_form_and_closing_it_clears_the_reason() {
        let (scratch, mut a) = writable("jot-fail");
        let lock = scratch.0.join(".moai/lock");
        // 락 파일 자리에 디렉터리를 두면 `with_write` 가 락을 못 열고 곧바로 물러난다.
        std::fs::create_dir_all(&lock).unwrap();
        jotting(&mut a, "못 담길 것");
        a.key(ctrl('s'));
        assert!(matches!(&a.mode, Mode::Idea(f) if f.title.text() == "못 담길 것"), "실패했는데 폼이 닫혔다 — {:?}", a.mode);
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다")), "{:?}", a.trouble);
        assert!(ideas_in(a.repo.as_ref().unwrap()).is_empty());

        // 고치고 다시 누르면 담기고 까닭이 걷힌다
        std::fs::remove_dir(&lock).unwrap();
        a.key(key(KeyCode::F(2)));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert!(a.trouble.is_none());
        assert_eq!(ideas_in(a.repo.as_ref().unwrap()).len(), 1);

        // 실패한 채로 버리고 닫으면 까닭도 걷힌다. 성공한 쓰기가 락 파일을 남겼다.
        std::fs::remove_file(&lock).unwrap();
        std::fs::create_dir_all(&lock).unwrap();
        jotting(&mut a, "버릴 것");
        a.key(ctrl('s'));
        assert!(a.trouble.is_some());
        a.key(key(KeyCode::Esc));
        a.key(key(KeyCode::Char('y')));
        assert_eq!((&a.mode, &a.trouble), (&Mode::Browse, &None), "버리고 닫았는데 까닭이 남았다");

        // 읽기의 실패는 폼과 상관없다 — 닫아도 남는다
        a.trouble = Some("다시 읽지 못했다 — 시험".into());
        a.key(key(KeyCode::Char('n')));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.trouble.as_deref(), Some("다시 읽지 못했다 — 시험"));
    }

    /// **누군지 모르면 쓰기 앞에서 묻고, 받으면 멈췄던 쓰기를 잇는다.** 묻는 동안 파일은
    /// 그대로다. 모양이 틀리면 칸이 열린 채 까닭을 달고, 받은 것은 세션 동안만 들어 다음
    /// 쓰기는 안 묻는다 — `.moai/config.toml` 에는 아무것도 안 적는다. 받고 나면 폼이
    /// 되돌아와 그 폼의 제목과 본문으로 담긴다.
    #[test]
    fn an_unknown_actor_is_asked_once_and_the_write_goes_on() {
        let (scratch, mut a) = writable("ask");
        let file = scratch.0.join(".moai/issues.jsonl");
        let config = scratch.0.join(".moai/config.toml");
        let (before, config_before) = (std::fs::read_to_string(&file).unwrap(), std::fs::read_to_string(&config).unwrap());
        a.user = None;
        a.identify = nobody;
        jotting(&mut a, "떠오른 것");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "본문");

        a.key(ctrl('s'));
        assert!(matches!(&a.mode, Mode::Ask(ask) if ask.why == "누가 하는지 모른다 — 시험"), "모르는데 안 물었거나 까닭을 옮기지 않았다 — {:?}", a.mode);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "묻기 전에 썼다");
        assert!(a.trouble.is_none(), "묻는 것은 실패가 아니다 — {:?}", a.trouble);

        type_in(&mut a, "이름만");
        a.key(key(KeyCode::Enter));
        let Mode::Ask(ask) = &a.mode else { panic!("모양이 틀렸는데 칸이 닫혔다 — {:?}", a.mode) };
        assert!(ask.error.as_deref().is_some_and(|e| e.contains("이름 (메일)")), "{:?}", ask.error);
        assert_eq!(ask.input.text(), "이름만", "거절하며 적은 것을 지웠다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        assert_eq!(a.user, None);
        a.key(key(KeyCode::Char(' ')));
        assert!(matches!(&a.mode, Mode::Ask(ask) if ask.error.is_none()), "고치기 시작했는데 까닭이 남았다");

        a.key(ctrl('u'));
        type_in(&mut a, "레이븐 (raven@example.com)");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.mode, Mode::Browse, "받았는데 멈췄던 쓰기가 안 이어졌다");
        let repo = a.repo.clone().unwrap();
        let made = ideas_in(&repo);
        assert_eq!(made.len(), 1, "받은 뒤에도 파일에 안 닿았다");
        assert_eq!((made[0].title.as_str(), made[0].body.as_deref()), ("떠오른 것", Some("본문")), "되돌린 폼에서 안 읽었다");
        // 이어진 쓰기도 같은 뒤처리를 받는다 — Enter 가 알림을 걷은 뒤에 쓰기가 제 알림을 단다.
        assert_eq!(on(&a), Some(made[0].id.clone()), "묻고 이어진 쓰기가 만든 줄에 안 섰다");
        assert_eq!(a.notice, Some(format!("✓ 담김 · {}", made[0].id)));
        let journal = repo.journal_of(&made[0].id).unwrap();
        assert_eq!((journal[0].by.as_str(), journal[0].by_email.as_deref()), ("레이븐", Some("raven@example.com")));
        assert_eq!(a.user.as_deref(), Some("레이븐 (raven@example.com)"));
        assert_eq!(std::fs::read_to_string(&config).unwrap(), config_before, "받은 것을 설정에 적었다");

        // 두 번째 쓰기는 묻지 않는다.
        jotting(&mut a, "또 하나");
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "한 번 받았는데 또 물었다");
        assert_eq!(ideas_in(&repo).len(), 2);
    }

    /// **Esc 는 아무것도 안 쓰고 적던 폼으로 돌아간다.** 한 키에 적던 것이 날아가면
    /// 다음부터 안 쓴다. 받다 만 이름도 들지 않는다.
    #[test]
    fn esc_on_the_question_writes_nothing_and_gives_the_form_back() {
        let (scratch, mut a) = writable("ask-esc");
        let file = scratch.0.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.user = None;
        a.identify = nobody;
        jotting(&mut a, "적던 것");
        let form = a.mode.clone();

        a.key(key(KeyCode::F(2)));
        assert!(matches!(a.mode, Mode::Ask(_)), "{:?}", a.mode);
        type_in(&mut a, "레이븐 (raven@example.com)");
        a.key(key(KeyCode::Esc));
        assert_eq!(a.mode, form, "적던 폼으로 안 돌아왔다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "그만뒀는데 썼다");
        assert_eq!(a.user, None, "그만뒀는데 받다 만 이름을 들었다");
        assert!(!a.quit);
    }

    /// **읽기는 묻지 않는다.** 누가 하는지는 쓸 때만 푼다 — 여는 순간이나 돌아다니는
    /// 동안 물으면 설정 없는 기계에서 도구가 고장 난 것으로 보인다.
    #[test]
    fn reading_never_asks_who() {
        fn refuse(_: Option<&str>) -> crate::fail::R<crate::model::Actor> {
            panic!("읽기가 누군지 물었다")
        }
        let (_scratch, mut a) = writable("ask-read");
        a.user = None;
        a.identify = refuse;
        for k in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace, KeyCode::F(5), KeyCode::Tab, KeyCode::Char('m')] {
            a.key(key(k));
        }
        a.key(key(KeyCode::Char('/')));
        type_in(&mut a, "제목");
        a.key(key(KeyCode::Enter));
        settle(&mut a);
        assert_eq!(a.mode, Mode::Browse);
    }

    /// 빈 디렉터리에서도 무너지지 않는다.
    #[test]
    fn an_empty_repo_is_safe() {
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        assert!(a.rows().is_empty());
        assert_eq!(a.current(), None);
        for k in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace, KeyCode::End] {
            a.key(key(k));
        }
        assert_eq!(a.cursor, 0);
    }
}
