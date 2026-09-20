//! 탐색기 화면. **쓰기는 [`App::write`] 한 구멍으로만 난다.**
//!
//! 상태([`App`])와 그림([`draw`])을 나눈다. 상태 전이는 터미널 없이 시험되고,
//! 그림은 `TestBackend` 로 시험된다 — 둘 다 TTY 를 켜지 않는다.

pub mod draw;
pub mod edit;
pub mod form;
pub mod input;
pub mod jotfile;
pub mod keys;
pub mod layer;
pub mod menu;
pub mod picker;
pub mod register;
pub mod scroll;
pub mod view;

use crate::config::Config;
use crate::i18n::{fill, say};
use crate::model::{Issue, Kind, Status};
use crate::nav::{Entry, Index, Path, Seg, Twig};
use crate::query::{Filter, GrepIn, Raw};
use crate::store::{Load, Repo};
use form::{Act, Form};
use input::Input;
use keys::Lookup;
use ratatui::crossterm::event::KeyEvent;
use scroll::{Move, Scroll};

/// 옆 워크트리를 겹치다 만난 것을 **고른 말로**(moai-dpbi) — 여는 화면(`cmd::tui`)과 다시
/// 읽기([`prepare`])가 같은 자를 쓴다. 갈라 적으면 배너가 첫 화면과 다음 걸음에서 말을 바꾼다.
///
/// 한때 여기 `SAID`(한국어로 박은 상수)가 섰다 — 탐색기의 나머지 글이 소스에 박혀 있어 이
/// 줄만 영어로 펴면 한 화면이 두 말로 섰기 때문이다. moai-9it4 가 그 글을 다 옮기면서 걷었고,
/// 이제 다시 읽기도 제 말을 들고 간다([`prepare`] 의 `lang`).
pub fn said_trouble(trouble: &[crate::worktree::Trouble], lang: crate::i18n::Lang) -> Vec<String> {
    trouble.iter().map(|t| crate::view::trouble_line(lang, t)).collect()
}

/// 목록의 한 줄. `..` 은 이슈가 아니므로 [`Entry`] 로는 못 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// 한 층 위로. 뿌리가 아닐 때만 맨 앞에 선다 — MC 와 같다. **디렉터리에만 선다**
    /// (moai-i784): 층이 있어도 프로젝트 뿌리에는 안 서고, 층으로는 헤더의 `0`
    /// ([`keys::Browse::Project`])이 간다 — 같은 글자가 두 데로 가지 않게.
    Up,
    /// 목록의 한 줄과 **그 줄의 가지 모양**([`Twig`]) — 펼쳐 든 멤버는 깊이가 1 이상이다.
    /// [`Seat`] 이 그 줄이 **어느 프로젝트의 것인가**를 진다.
    Item(Seat, Entry, Twig),
    /// 프로젝트 머리줄 — `layer.places` 의 첨자다. 정체는 경로다([`Anchor::Project`]).
    /// 한눈 보기에서는 그 밑에 그 프로젝트의 줄이 선다.
    Project(usize),
}

/// 그 줄이 사는 프로젝트(moai-eyre). **한눈 보기는 프로젝트를 가로질러 줄을 세우므로**
/// ([`App::rows`]), 줄 하나만 보고는 그 `Entry` 의 첨자가 어느 이슈 목록의 것인지 알 수 없다.
///
/// 프로젝트 안에서는 늘 [`Seat::Here`] 다 — 한 프로젝트만 서므로 고를 것이 없다. 한눈 보기의
/// 줄은 [`Seat::Place`] 로 제 층 줄을 댄다. **첨자를 든다** — 경로를 들면 줄마다 문자열을
/// 베끼고, 첨자는 한 프레임 안에서만 사는 값이라 목록이 다시 지어지면 함께 새로 선다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seat {
    /// 지금 선 프로젝트 — `App::site`.
    Here,
    /// 한눈 보기의 `layer.places[n]` 이 든 프로젝트.
    Place(usize),
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
    /// 한눈 보기에서 **남의 프로젝트의** 줄(moai-1xo5) — 그 프로젝트의 경로와 id 다. id 만으로는
    /// 못 가른다: 두 프로젝트가 같은 prefix 를 쓰면 같은 id 가 둘 있고, 커서가 남의 줄로 튄다.
    Foreign(std::path::PathBuf, String),
    /// 바구니는 제 줄이 없어 `Seg` 로. **그 프로젝트도 함께 든다**(리뷰) — `Milestone(None)`·
    /// `Lost` 는 id 조차 없어, 경로 없이 들면 한눈 보기에서 두 프로젝트의 `(마일스톤 없음)` 이
    /// 글자 그대로 같은 정체가 된다: 다시 읽을 때마다 커서가 첫 프로젝트의 그 줄로 튄다.
    /// 프로젝트 안에서는 늘 `None` 이다 — 한 프로젝트만 서므로 가릴 것이 없다.
    Bucket(Option<std::path::PathBuf>, Seg),
    /// 이름이 아니라 경로 — 이름은 등록 목록이 바뀌면 달라진다.
    Project(std::path::PathBuf),
}

/// 무엇을 받고 있는가. 글을 받는 동안에는 이동키가 글자가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Browse,
    /// `/` 로 연 빠른 검색. 곁의 것은 찾을 자리 — 칸의 Tab·Shift-Tab 이 돌린다(moai-kojj).
    Grep(Input, GrepIn),
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

/// 줄을 가리는 것 — 거름망과 보기(`SPC v`) 각각([`App::veil`]). 둘 다 `false` 면 목록에 선다 — 검색이 걸린
/// 동안은 `viewed` 가 서도 선다([`App::visible`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Veil {
    /// 거름망(`/`·`f`)에 안 걸렸다.
    filtered: bool,
    /// 보기가 숨겼다.
    viewed: bool,
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

    /// 한 칸 왼쪽·오른쪽. **끝에서는 제자리다** — vi 의 `Ctrl-w h`·`Ctrl-w l` 이 그렇다.
    pub fn step(self, side: keys::Side) -> Pane {
        let at = self.at();
        let to = match side {
            keys::Side::Left => at.saturating_sub(1),
            keys::Side::Right => (at + 1).min(Pane::ALL.len() - 1),
        };
        Pane::ALL[to]
    }

    /// 칸의 이름. 키 바가 칸 옮기는 키가 **어디로 가는지** 댄다.
    pub fn word(self, lang: crate::i18n::Lang) -> &'static str {
        match self {
            Pane::Explorer => crate::i18n::say(lang, "tui.pane.list"),
            Pane::Detail => crate::i18n::say(lang, "tui.pane.detail"),
        }
    }
}

/// 묶음 id → 멤버에서 읽은 것(`report::group_stands`). 이슈를 빌리지 않게 소유한다.
type States = std::collections::BTreeMap<String, Stood>;

/// 묶음 하나를 읽은 것 — 서 있는 칸과 그 칸의 셈이 마지막으로 움직인 때(`report::Stand::since`),
/// 그 밑에 집은 일이 있는가(`report::Stand::busy`), 미뤄 뺀 멤버 덕에 `done` 으로 섰으면 그 멤버
/// (`report::Stand::aside`).
struct Stood {
    column: String,
    since: String,
    busy: bool,
    waiting: crate::report::Waiting,
    aside: Vec<String>,
}

/// **파일 전체를 훑어야 아는 것을 소유한 꼴**(moai-fbdg). `report::Soil` 은 이슈를 빌리므로 `App` 이
/// 들 수 없다 — 적재 때 한 걸음으로 재어 이것으로 소유해 두고, 색인(`nav::Index`)·묶음 칸·거름망이
/// 그 한 벌을 나눠 쓴다. 따로 셀 때는 `Index::of`·`states_of`·`Where::of` 가 저마다 소속 지도를
/// 지어, 거름망은 **키 하나마다** 이슈 1만 건에서 300ms 를 치렀다.
#[derive(Default)]
pub struct Ground {
    stands: States,
    epic: std::collections::BTreeMap<String, String>,
    milestone: std::collections::BTreeMap<String, String>,
    put_off: std::collections::BTreeSet<String>,
    kinds: std::collections::BTreeMap<String, crate::model::Kind>,
    folded: std::collections::BTreeSet<String>,
}

/// 적재 한 걸음(moai-fbdg) — 색인과 [`Ground`] 를 **한 번 잰 지도**(`report::Soil`)에서 짓는다. 여는 길
/// (`cmd::tui`)·다시 읽기([`prepare`])·시험의 들이기([`App::adopt`])가 모두 이 몸([`measure_in`])을
/// 지난다 — 한때 여는 길만 `Index::of` 와 `Ground::of` 를 따로 불러, 첫 화면 앞에서 소속 지도를 두 번
/// 쟀다(moai-xemz 리뷰).
pub fn measure(issues: &[Issue], cfg: &Config) -> (Index, Ground) {
    measure_in(issues, cfg, &crate::report::Soil::of(issues))
}

/// [`measure`] 와 같은 것. **이미 잰 지도를 받는다** — 다시 읽기([`prepare`])가 같은 지도를 경고 셈
/// (`warnings_in`, moai-u5o9)에도 넘긴다. 그 길이 이 몸을 제 자리에 한 벌 더 펴면, 색인·칸 지도에 무엇이
/// 들고 나는지가 여는 길과 다시 읽는 길에서 갈린다.
fn measure_in(issues: &[Issue], cfg: &Config, soil: &crate::report::Soil<'_>) -> (Index, Ground) {
    (Index::in_soil(issues, soil), Ground::in_soil(issues, cfg, soil))
}

impl Ground {
    fn in_soil(issues: &[Issue], cfg: &Config, soil: &crate::report::Soil<'_>) -> Ground {
        let stands = soil
            .stands(issues, cfg)
            .into_iter()
            .map(|(id, s)| {
                let aside = s.aside.iter().map(|m| m.to_string()).collect();
                let stood = Stood {
                    column: s.column.to_string(),
                    since: s.since.to_string(),
                    busy: s.busy,
                    waiting: s.waiting,
                    aside,
                };
                (id.to_string(), stood)
            })
            .collect();
        Ground {
            stands,
            epic: soil.epic.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            milestone: soil.milestone.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            put_off: soil.roots.keys().map(|k| k.to_string()).collect(),
            kinds: soil.kinds.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            folded: soil.folded.iter().map(|k| k.to_string()).collect(),
        }
    }

    /// 묶음 id → 멤버에서 읽은 칸(`report::group_states` 와 같은 답). 거름망과 `tui --json` 의 줄이 쓴다.
    pub fn columns(&self) -> std::collections::BTreeMap<&str, &str> {
        self.stands.iter().map(|(id, s)| (id.as_str(), s.column.as_str())).collect()
    }

    /// 거름망이 볼 꼴 — 든 지도를 빌리기만 하고 다시 재는 것은 없다. 빌린 지도를 짓는 값은 **이슈 수에
    /// 비례한다**: 소속 지도(`epic`·`milestone`)는 멤버 줄마다 한 칸이다. 그래도 재는 값(`Where::of`, 1만
    /// 건에서 수십 ms)보다 한참 싸서 거름망이 키마다 부른다. 필드는 **이름으로** 넘긴다 — 같은 타입의
    /// 지도가 넷이라 차례로 넘기면 서로 바뀌어도 컴파일된다.
    fn here(&self) -> crate::query::Where<'_> {
        fn borrow(m: &std::collections::BTreeMap<String, String>) -> std::collections::BTreeMap<&str, &str> {
            m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
        }
        let kinds = &self.kinds;
        crate::query::Where {
            epic: borrow(&self.epic),
            milestone: borrow(&self.milestone),
            put_off: self.put_off.iter().map(String::as_str).collect(),
            states: self.columns(),
            since: self.stands.iter().map(|(id, s)| (id.as_str(), s.since.as_str())).collect(),
            eclipsed: Some(Box::new(move |i: &Issue| crate::report::is_eclipsed(kinds, i))),
            folded: self.folded.iter().map(String::as_str).collect(),
        }
    }
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
    ground: Ground,
    warnings: usize,
    unreadable: Vec<Option<String>>,
    origin: crate::worktree::Origin,
    elsewhere: Vec<String>,
    /// 옆 워크트리를 못 찾은 까닭(`Gathered::unfound`). 사람이 SPC v w 로 켰을 때만 댄다.
    unfound: Option<String>,
    watched: Vec<(std::path::PathBuf, Stamp)>,
    now: String,
}

/// 뿌리(이 프로젝트, 줄을 보탠 옆 워크트리) → 그 가지의 id → 커밋 표(`git::table`, moai-a4i0).
/// **표만 짓는 스레드에서 짓는다**([`App::follow_commits`]) — 커서를 옮길 때마다 git 을 부르면
/// 걸음마다 루프가 멈칫한다. 다시 읽기([`prepare`])에도 태우지 않는다: 쓰기·SPC v w 는 그
/// 읽기를 **루프에서** 부르므로, 거기서 이력을 뿌리마다 끝까지 걸으면 쓸 때마다 화면이 멈춘다.
/// git 을 못 쓰는 뿌리는 빠진다 — 상세의 커밋 칸이 말없이 빈다(`show` 와 같은 자리, moai-mauw).
pub type Commits =
    std::collections::BTreeMap<std::path::PathBuf, std::collections::BTreeMap<String, Vec<crate::git::Commit>>>;

/// 커밋 표를 지을 뿌리 — 이 세션이 **선 체크아웃**과, 줄을 보태 온 옆 워크트리.
///
/// **트래커의 자리가 아니라 `Repo::here` 다**(moai-y7go, 리뷰 moai-71ht.jlh) — 트래커는 루트로
/// 옮겨 가지만 `git log` 가 묻는 `HEAD` 는 이 체크아웃의 것이다. 루트로 물으면 이 가지에서 방금
/// 한 일이 루트의 `HEAD` 에서 안 보여, 표가 "아무도 안 고쳤다" 로 빈다.
fn commit_roots(repo: &Repo, origin: &crate::worktree::Origin) -> Vec<std::path::PathBuf> {
    // **같은 뿌리를 두 번 담지 않는다** — 선 체크아웃이 옆에서 줄을 보태 온 자리이기도 하면(제
    // 스냅샷에만 있는 줄) `git log` 를 두 번 띄우고 표는 하나만 남는다. `repo.root` 로 담던 때는
    // `others_of` 의 가름이 그것을 막아 줬다.
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    for root in std::iter::once(repo.here()).chain(origin.roots()) {
        if !out.iter().any(|p| p == root) {
            out.push(root.to_path_buf());
        }
    }
    out
}

/// 뿌리마다 [`crate::git::table`] 을 짓는다. **어느 스레드에서 불러도 같다.**
///
/// `ids` 는 지금 들고 있는 줄의 id 다 — 표는 그것들과 낱말을 견줘 서므로, 형식이 어긋난 줄도
/// 제 커밋을 찾고 id 아닌 낱말은 표에 안 선다(moai-ynhj).
fn commit_tables(roots: &[std::path::PathBuf], ids: &std::collections::BTreeSet<String>) -> Commits {
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    roots.iter().filter_map(|root| crate::git::table(root, &ids).ok().map(|t| (root.clone(), t))).collect()
}

/// 버린 다시 읽기 손잡이를 이만큼까지 든다(`App::discarded`). 버리는 것은 사람의
/// 손(SPC v w·쓰기)이 읽기가 도는 동안 닿을 때뿐이고, 한 읽기는 1만 개에서도 수백 ms
/// 라 보통은 하나도 안 쌓인다. 이것이 차는 것은 읽기가 멈춘 때뿐이다.
const DISCARDED_KEPT: usize = 8;

/// 저장소를 읽어 [`Fresh`] 를 짓는다. **어느 스레드에서 불러도 같다.**
fn prepare(repo: &Repo, worktree: bool, lang: crate::i18n::Lang) -> crate::fail::R<Fresh> {
    // 읽기 **전에** 잰다. 뒤에 재면 읽고 재는 사이의 쓰기를 놓치고, 놓친
    // 것은 영영 안 돌아온다. 먼저 재면 최악이 헛 갱신 하나다.
    let stamp = stamp_of(repo);
    // **겹쳐 보지 않아도 HEAD 는 지켜본다**(moai-a4i0). 겹쳐 보면 `gather` 가 이미 잰다. 안 재면
    // `SPC v w` 로 끈 동안 커밋을 해도 스냅샷이 안 바뀌어 커밋 칸이 낡은 채 선다. `gather` 에
    // 두지 않는 것은 CLI 명령마다 git 을 한 번 더 띄우게 되어서다 — 지켜보는 것은 탐색기뿐이다.
    // **이것도 읽기 전에 잰다** — 읽는 동안 떨어진 커밋을 뒤에 재면 놓친다(위와 같은 까닭).
    let heads = if worktree { Vec::new() } else { crate::worktree::heads(&repo.root) };
    // **자리 판정이 보는 것도 지켜본다**(리뷰 moai-3lul.kt0, moai-al0x). [`placed`] 가 밑에서 옆
    // 워크트리의 있고 없음과 스냅샷을 읽는데, 겹쳐 보지 않을 때의 `watched` 에는 그것이 하나도
    // 안 들어 — 워크트리를 `rm -rf` 로 치워도 `App::follow` 가 다시 안 읽고 배너만 옛 수로 선다.
    // 층이 제 줄을 재는 자와 같다(`layer::marks_of`). **읽기 전에** 잰다(위와 같은 까닭).
    let places = crate::worktree::place_marks(repo.here());
    let g = crate::worktree::gather(repo, worktree)?;
    // 옆에서만 온 줄과 겹친 id 는 중복으로 세지 않는다 (`Origin::unreadable`).
    let unreadable: Vec<Option<String>> = g
        .origin
        .unreadable(g.load.errors.iter().map(|e| e.id.as_deref()))
        .into_iter()
        .map(|id| id.map(str::to_string))
        .collect();
    let issues = g.load.issues;
    // **한 걸음으로 잰다**(moai-fbdg) — 색인과 묶음 칸·거름망, 그리고 경고 셈(moai-u5o9)이 같은 지도를
    // 나눠 쓴다([`measure_in`]).
    let now = crate::model::now();
    let soil = crate::report::Soil::of(&issues);
    let (index, ground) = measure_in(&issues, &repo.config, &soil);
    let warnings = warnings_in(&issues, &unreadable, &repo.config, &now, &soil);
    let mut watched = g.watched;
    watch(&mut watched, heads);
    watch(&mut watched, places);
    // 옆을 **실제로 겹쳤는가**로 잰다 — 켠 깃발이 아니다([`placed`]).
    // 겹치며 이미 판 옆 스냅샷을 그대로 넘긴다(moai-kos1) — 자리 판정이 바로 앞에서 푼 같은
    // 파일을 다시 열어 파고 있었다. 걸음마다 치르던 값이라 쓰기·`SPC r`·프로젝트 들어가기가
    // 그것을 그대로 물었다(이 저장소에서 ~130ms).
    // **제 스냅샷도 그대로 넘긴다**(moai-mafv) — 걸음마다 다시 파던 마지막 한 벌이다.
    let (lost, said) = placed(repo, &issues, g.swept, &now, &crate::worktree::dug(&g.sides, &g.mine), lang);
    // 옆 워크트리의 문제는 **펴서** 싣는다(moai-dpbi) — 여는 화면(`cmd::tui`)과 같은 자다.
    let mut elsewhere = said_trouble(&g.trouble, lang);
    elsewhere.extend(said);
    Ok(Fresh {
        root: repo.root.clone(),
        stamp,
        index,
        ground,
        warnings: warnings + lost,
        issues,
        unreadable,
        origin: g.origin,
        elsewhere,
        // 못 찾은 까닭도 여기서 편다(moai-dpbi) — 배너와 알림은 글을 그대로 낸다.
        unfound: g.unfound.as_ref().map(|t| crate::view::trouble_line(lang, t)),
        watched,
        now,
    })
}

/// 지켜볼 것에 더한다 — **이미 든 자리는 안 더한다.** `worktree::heads`·`gather`·`worktree::place_marks`
/// 가 같은 파일(공용 디렉터리의 `worktrees`, 워크트리마다 `HEAD`, 옆 스냅샷)을 저마다 재므로, 그냥 이으면
/// 걸음마다 같은 파일을 두 번 재고 목록 모양이 읽는 길마다 갈린다 — 띄울 때(`cmd::tui`)와 다시 읽을 때의
/// 목록이 다르면 [`App::apply_fresh`] 가 그것을 "커밋이 섰다" 로 읽어 안 바뀐 커밋 표를 다시 짓는다.
/// 먼저 든 쪽의 표식을 둔다 — 둘 다 읽기 전에 잰 것이다.
pub fn watch(watched: &mut Vec<(std::path::PathBuf, Stamp)>, more: Vec<(std::path::PathBuf, Stamp)>) {
    for (path, stamp) in more {
        if !watched.iter().any(|(p, _)| *p == path) {
            watched.push((path, stamp));
        }
    }
}

/// 자리 판정이 배너에 싣는 것 — 자리 없는 집은 줄의 경고 수(0 이나 1)와, 판 것 가운데 스냅샷을 못
/// 읽은 옆 워크트리를 대는 말(moai-al0x). `moai status` 와 프로젝트 층이 싣는 그 셈이다
/// (`worktree::stranded_at`). 한때 여기만 안 세어, 층에서 `! 1` 을 보고 들어온 사람이 안쪽 배너에서
/// 0 을 봤다(사용자 결정 2026-09-18 — 안쪽이 `moai status` 에 맞춘다).
///
/// [`warnings_of`] 에 안 넣고 따로 둔 까닭: 그것은 `&[Issue]` 에 대한 순수한 셈이라 못 읽는 줄의
/// 자가 바뀔 때마다 다시 부르는데, 이것은 디스크의 워크트리를 읽는다(이름으로 안 잡히는 집은 줄이
/// 있으면 옆 스냅샷을 판다). 그래서 읽을 때마다 한 번 재어 더한다([`prepare`]·[`App::overlaid`]).
///
/// **`overlaid` 는 옆을 실제로 겹쳤는가다**(`Gathered::swept`), 켠 깃발이 아니다. 탐색기의 기본값(켬)
/// 에서 겹쳤으면 `moai status --worktree` 와 같은 수고, `w` 로 끄면 `moai status` 와 같은 수다. 켰는데
/// git 을 못 불러 못 겹쳤으면 줄은 제 스냅샷뿐이다 — 딸린 워크트리의 그 스냅샷은 갈라질 때의 main 이라,
/// 겹친 것으로 재면 main 에서 이미 끝낸 일이 "자리 없다" 로 선다(`workplaces` 가 딸린 워크트리를
/// 겹쳐 볼 때만 재는 까닭이 그것이다). 딸린 워크트리에서 겹쳐 보면 층·`moai status` 와 갈리는데
/// (리뷰 moai-3lul.kt0), 화면이 "지금 겹쳐 보는 것" 을 말하는 쪽이 맞다.
///
/// **못 읽은 옆 스냅샷은 겹치지 못했을 때만 댄다** — 겹쳤으면 `gather` 가 같은 워크트리를 `elsewhere`
/// 에 이미 댔다(`moai status` 의 `swept` 와 같은 자). 안 대면 판정이 가려진 0 이 "없다" 로 읽히고,
/// 층은 같은 저장소에 `!` 를 세운다(리뷰 moai-3lul.kt0 다시 본 판, 사용자 결정 moai-rgz9.7vt).
fn placed(
    repo: &Repo,
    issues: &[Issue],
    overlaid: bool,
    now: &str,
    dug: &crate::worktree::Dug<'_>,
    lang: crate::i18n::Lang,
) -> (usize, Vec<String>) {
    // **자리는 세션이 선 체크아웃에서 잰다**(`repo.here()`, 리뷰 moai-71ht 셋째 판) — 지켜볼 것을
    // 재는 자(`place_marks(repo.here())`)와 같은 뿌리여야 한다. 트래커의 자리로 재던 판은 딸린
    // 워크트리 안에서 자리를 파면서 그 자리들을 하나도 안 지켜봐, 옆 워크트리를 치워도 배너가
    // 옛 수로 섰다(moai-al0x 가 고친 자리다).
    let (lost, unread) = crate::worktree::stranded_at_in(repo.here(), &repo.config, issues, overlaid, now, dug);
    let said = match overlaid {
        true => Vec::new(),
        false => unread
            .all
            .iter()
            // 글은 [`crate::view::unread_worktree`] 한 자리에서 짓는다(리뷰) — `moai status` 의
            // stderr·밖 한눈 보기와 **같은 줄**이어야 한다. 갈라 적으면 말묶음을 고치는 날 여기만 남는다.
            .map(|t| crate::view::unread_worktree(lang, &t.branch, &t.path))
            .collect(),
    };
    (usize::from(lost.is_some()), said)
}

/// **알림은 안 센다.** 배너는 "드러난 것 N건" 이라고 말하는데, 담아 둔
/// 생각이 쌓였다는 알림을 거기 더하면 생각을 담을수록 화면이 고쳐야 할
/// 것이 늘었다고 말한다 — 그러면 안 담게 된다. 무엇이 알림인지는
/// `report` 가 `notices` 로 따로 내므로 여기서 다시 판단하지 않는다.
fn warnings_of(issues: &[Issue], unreadable: &[Option<String>], cfg: &Config, now: &str) -> usize {
    warnings_in(issues, unreadable, cfg, now, &crate::report::Soil::of(issues))
}

/// [`warnings_of`] 와 같은 것. 적재가 이미 잰 지도를 받는다(`report::status_in`, moai-u5o9).
fn warnings_in<'a>(
    issues: &'a [Issue],
    unreadable: &[Option<String>],
    cfg: &Config,
    now: &str,
    soil: &crate::report::Soil<'a>,
) -> usize {
    // 못 읽는 줄의 id 까지 넘긴다 — 산 줄과의 중복을 `moai status` 와 같은
    // 자로 센다.
    let lines: Vec<crate::report::Unreadable> =
        unreadable.iter().map(|id| crate::report::Unreadable { id: id.as_deref() }).collect();
    // 알림은 `notices` 에 따로 있다 — `warnings` 가 곧 고칠 것이다.
    crate::report::status_in(issues, &lines, cfg, now, soil).warnings.len()
}

/// 프로젝트 하나에 딸린 것 — 그 프로젝트의 줄과, 그 줄을 재고 그리는 데 드는 모든 것(moai-pqmg).
///
/// **한 덩이로 묶은 까닭은 여럿을 들려고다.** 한눈 보기가 프로젝트마다 커밋 칸·안 읽음 `[NEW]`·
/// `⎇` 워크트리 마크까지 제 것으로 세우기로 했고(사용자 결정 2026-09-19, moai-ucx8), 그러려면 이
/// 스물몇이 프로젝트마다 한 벌씩 있어야 한다. 흩어 두면 그 한 벌을 고르는 자리가 필드 수만큼 는다.
///
/// **보는 사람의 것은 안 든다** — 커서·굴린 자리·정렬·열·보기 토글·누구인가는 화면에 하나뿐이라
/// [`App`] 에 남는다. 여기 드는 것은 *그 프로젝트가 무엇인가* 와 *그 안에서 어디를 보나* 다.
pub struct Site {
    /// 이 화면의 말(moai-ra67). **탐색기도 고른 말로 선다** — 한때 이 층에 말이 안 닿아
    /// `layer::shut` 이 `Lang::Ko` 를 손으로 줬고, 그 한 줄이 "탐색기는 아직 한국어로 선다" 는
    /// 뜻이었다. 명령 층에서 한 번 풀어(`Ctx::lang`) 여기 놓는다 — `view::Screen` 이 CLI 에서
    /// 하는 일과 같은 자리다.
    ///
    /// **아직 여기서 오는 글이 탐색기의 전부는 아니다** — 넘겨받은 넷(미룸 한 마디·묶음 칸
    /// 한 줄·바구니 이름 둘)만 이 말을 따르고, 나머지 글자는 그대로 한국어다. 그 나머지는
    /// 에픽 moai-hom6 의 멤버로 서 있다.
    pub lang: crate::i18n::Lang,
    pub issues: Vec<Issue>,
    pub index: Index,
    /// 파일 전체를 훑어야 아는 것 — 묶음이 선 칸, 소속, 물려받은 미룸, 가려짐. **적재 때 한 번 센다**
    /// — 프레임마다 세면 줄 하나 그리는 데 저장소를 걷고, 거름망은 키 하나마다 그랬다(moai-fbdg).
    ground: Ground,
    pub cfg: Config,
    pub path: Path,
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
    /// `moai status` 가 드러낼 것의 수. 자세한 화면은 나중에 얹는다.
    pub warnings: usize,
    /// 마지막으로 읽은 파일의 (고친 때, 길이).
    stamp: Stamp,
    /// **이 프로젝트의** 적어 둔 읽음 — 이슈 id → 마지막으로 본 줄의 도장(moai-lyc1). 한때 `App` 에 맵
    /// 하나로 있었는데, 그것이 곧 저장 자리의 섞임이 화면에 비친 모습이었다(moai-bwce·moai-omx7) —
    /// 한눈 보기의 모든 프로젝트가 그 한 맵을 같이 봐, 이름이 같은 두 저장소의 같은 id 가 서로의
    /// [NEW] 를 내렸다. 읽음 파일이 프로젝트마다 갈렸으니([`crate::read_marks`]) 화면의 표도 여기 산다.
    pub seen: std::collections::BTreeMap<String, String>,
    /// 그 읽음 파일의 표식. 걸음이 이것을 재, 옆 터미널의 `moai read` 가 적은 줄이 이 화면에 닿는다
    /// ([`App::follow_config`], moai-j038.vna). 한때 설정 파일의 표식이 그 길이었는데, 읽음이 설정 밖으로
    /// 나가면서 설정은 더 안 바뀐다 — 그 길을 여기로 옮겼다.
    ///
    /// **없는 것과 못 찾은 것을 가른다**(`Option<…>`, [`App::config_stamp`] 와 같은 자) — 아직 안 든
    /// 프로젝트는 `None` 이고, 읽음 파일이 없는 것은 `Some(…)` 안의 `at` 이 `None` 이다. 하나로 들면
    /// 옛 `[read]` 만 있는 사람의 프로젝트가 영영 안 들린다(새 파일이 없어 표식이 늘 `None` 이라
    /// "그대로" 로 읽힌다).
    ///
    /// **재는 파일이 하나가 아니다**([`ReadStamp`], moai-65as) — 읽기가 여는 자리가 셋이라 그 전부를 든다.
    read_stamp: Option<ReadStamp>,
    /// 마지막 읽음 읽기의 자취(moai-po6v) — 갈래와 언제 해 봤는가. 다시 읽을 때를 정하는 자다
    /// ([`layer::owed`]). **설정과 한 자다**([`App::config_tried`]) — 갈라 두면 같은 갈래를 두 곳이 달리
    /// 읽어, 한쪽을 고친 날 다른 쪽이 조용히 옛 뜻으로 남는다.
    ///
    /// 아래 `read_at`(스냅샷을 **들인** 때)과 다른 값이다 — 이름이 닮았으나 재는 파일이 서로 다르다.
    read_tried: layer::Tried,
    /// 마지막 읽기를 **들인** 때. 표식이 그대로여도 [`layer::REREAD_EVERY`] 가 지나면 다시 읽는다
    /// ([`App::follow`], moai-z4r4). 들인 때로 찍는 까닭은 층과 같다(`Layer::adopt`) — 시작한 때로
    /// 찍으면 한 번 읽는 데 그만큼 걸리는 자리에서 쉬지 않고 다시 읽는다.
    read_at: Option<std::time::Instant>,
    /// 겹쳐 보는 동안 함께 지켜보는 옆 워크트리 스냅샷과, 어느 때든 지켜보는 HEAD·가지
    /// 파일·`packed-refs`, 자리 판정이 보는 옆 워크트리(`worktree::place_marks`)의 표식(읽기 **전에**
    /// 잰 것 — `worktree::gather`·[`prepare`]). 같은 파일은 한 번만 든다([`watch`]).
    watched: Vec<(std::path::PathBuf, Stamp)>,
    /// 이슈에 닿은 커밋 표([`Commits`]). **표식(`watched`)이 움직였을 때 새로 짓는다** —
    /// 거기 HEAD·가지 파일이 들어 있어 그것이 곧 "커밋이 섰는가" 다. 다시 읽을 때마다
    /// 지으면 스냅샷 쓰기 하나(`mv` 한 번, 옆 세션의 쓰기 하나)마다 뿌리마다 이력을 통째로
    /// 걷는다 — 커밋이 안 선 것을 알면서 걷는 일이다. 새 표가 올 때까지는 옛 표를 든다.
    commits: Commits,
    /// 지금 든 표를 지을 때 준 id 들. **표의 내용이 이 목록에 매인다**(moai-ynhj) — 표는
    /// 낱말을 이 id 들과 견줘 서므로, 여기 없던 id 가 줄에 서면 그 줄의 커밋 칸은 표식이
    /// 다시 움직일 때까지 영영 빈다(들여온 줄·되살린 파일처럼 커밋이 먼저 있고 줄이 나중에
    /// 오는 자리가 그렇다). 그래서 **새 id 가 들면 표식이 그대로여도 한 번 더 짓는다.**
    /// 사라진 id 는 안 센다 — 남은 칸은 아무도 찾지 않으므로 걷기를 새로 살 값이 없다.
    commit_ids: std::collections::BTreeSet<String>,
    /// 이슈 첨자 → 걸렸는가. **거름망이 바뀔 때만 다시 센다** — 매 프레임 `Filter::matches` 를 돌리면
    /// 거름망이 볼 꼴(`Ground::here`)을 프레임마다 다시 짓는다. 소속 지도 자체는 적재 때 한 번 잰다(moai-fbdg).
    keep: Vec<bool>,
    /// 이슈 첨자 → 보기에 보이는가. `keep` 과 같은 까닭으로 **보기나 자료가 바뀔 때만** 센다
    /// ([`App::see`]) — 묶음의 칸과 물려받은 미룸을 줄마다 프레임마다 다시 풀지 않는다.
    shown: Vec<bool>,
    /// `shown`·`lit` 이 **어느 보기로** 선 것인가(moai-m59y). 같은 보기를 같은 줄에 다시 걸면
    /// 건너뛴다 — 지금 선 프로젝트 하나를 다시 읽을 때마다(`App::take`) 든 프로젝트 **전부**의
    /// `shown`·`lit` 을 다시 세던 자리다. 그 둘은 보기와 줄의 함수라, 둘 다 그대로면 답이 같다.
    /// 줄이 바뀌는 길은 둘뿐이다 — 새로 지은 [`Site`](여기가 `None`)와 [`App::take`](거기서 비운다).
    seen_view: Option<view::View>,
    /// 보기에 보이는 줄이 사는 자리의 **모든 앞머리**([`App::see`]가 `shown` 과 함께 센다). 폴더가 검색 없이도
    /// 설지를 이 하나로 묻는다([`App::stands_in_view`]) — 폴더마다 이슈 전부를 훑으면 목록 한 번에 폴더 수 ×
    /// 이슈 수가 들고, 목록·머리 셈·열 셈이 프레임마다 그것을 묻는다(`Index::entries_sorted` 의 `lit` 과 같은 까닭).
    lit: std::collections::HashSet<Path>,
    /// **안 읽은 줄의 id**(moai-z9pc) — 내게 온 것 가운데 내가 마지막으로 본 뒤에 바뀐 것
    /// (`query::unread`). 줄·읽음·사람이 바뀔 때만 센다(`take`·`load_read`·`mark_read`) — 줄마다
    /// 프레임마다, 보기 토글마다 담당·조상을 다시 풀지 않는다.
    pub unread: std::collections::BTreeSet<String>,
    /// 층마다 커서를 기억한다. 들어갔다 나오면 **있던 자리로 돌아온다** —
    /// 매번 맨 위로 튕기면 형제 여럿을 훑는 일이 못 할 짓이 된다.
    remembered: Vec<usize>,
    /// 목록에서 펼쳐 둔 묶음의 자리(moai-7qot). **화면에만 산다**(사용자 결정) — 설정에 안
    /// 남는다. 보기(`Look`)는 *무엇을 숨기나* 고 펼침은 *지금 어디를 보나* 라 축이 다르고,
    /// 설정에 이슈 id 를 쌓으면 지운 줄의 id 가 설정에 남는다.
    ///
    /// 접어도 **밑의 펼침은 기억한다** — 접었다 다시 펼치면 안이 그대로 선다. `Tab`
    /// (다 펼침)의 두 번째 누름만 밑까지 걷는다([`App::expand_all`]).
    expanded: std::collections::HashSet<Path>,
    /// 겹쳐 본 줄의 출처. 꺼져 있으면 비었다.
    pub origin: crate::worktree::Origin,
    /// 옆 워크트리에서 만난 문제. **배너로 말만 한다** — CLI 가 stderr 로 흘리는
    /// 말인데, 대체 화면 안에서는 그 길을 못 쓴다.
    pub elsewhere: Vec<String>,
    /// 옆 워크트리를 **못 찾은** 까닭(`Gathered::unfound`) — git 밖 프로젝트다. 경로 줄에도 배너에도
    /// 안 세운다: 겹쳐 보기는 켜진 채로 시작해 git 밖 프로젝트를 볼 때마다 시키지 않은 말이 선다.
    /// 사람이 `SPC v w` 로 **켰을 때만** 알림으로 한 번 댄다(moai-d5vn).
    pub unfound: Option<String>,
}

pub struct App {
    /// 지금 선 프로젝트에 딸린 것 — 줄·색인·설정과 그 안에서 선 자리([`Site`]).
    pub site: Site,
    pub cursor: usize,
    pub mode: Mode,
    /// 이동키(`↑↓`·PageUp/Down·Home/End)를 먹는 칸. `Tab`·`Shift-Tab` 이 돌린다.
    /// **다시 읽어도 그대로다** — 굴리던 사람의 손이 갱신 한 번에 목록으로 튀면 안 된다.
    pub focus: Pane,
    /// 지금 걸린 거름망. 사람이 적은 글 그대로도 들고 있어야 화면에 되비친다.
    pub filter_text: Option<String>,
    /// 걸린 검색이 찾는 자리. `filter_text` 가 `/` 로 시작할 때만 뜻이 있다 — 다시 읽을 때
    /// 뱃지 글(`/id:0004`)을 되읽지 않고 이것으로 거름망을 다시 짓는다.
    pub grep_in: GrepIn,
    /// 검색 칸을 열기 전의 거름망·범위·커서. **칸은 치는 대로 거르므로**(moai-00le) Esc 가
    /// 그만두려면 되돌아갈 자리를 들고 있어야 한다. Enter 로 걸면 버린다.
    grep_was: Option<(Option<String>, GrepIn, usize)>,
    /// 마지막 갱신이나 쓰기가 **실패한** 까닭 — 무엇을 못 했는지까지 단 쪽이 적는다.
    /// 조용히 삼키면 갱신이 아무 일도 안 하는데 "바뀌었다" 배너는 붙어 있어, 사람은
    /// 기다리고 또 기다리며 까닭을 못 얻는다.
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
    /// 상세에 선 본문을 펼쳐 둔 한 벌(moai-fauw, [`draw::Body`]) — **그리기 전에** `draw::fill_body`
    /// 가 채운다. 그리는 쪽은 `&App` 만 빌려 제자리에서 못 넣고, 안 넣으면 프레임마다 같은 본문을
    /// 다시 판다. 값일 뿐이라 비어 있어도 그림은 같다 — 그때는 그리는 쪽이 그 자리에서 편다.
    body: Option<draw::Body>,
    /// 사용자 설정을 못 읽어 **층을 안 세운** 까닭. 층이 서면 아래의 [`App::held`] 가 대므로
    /// (moai-23pm) 층이 없을 때만 든다. 붙박이다 — 다시 읽기가 걷는 `trouble` 에 두면 700ms 뒤에
    /// 사라져 사람은 층이 왜 없는지 끝내 모른다. 설정 파일이 바뀌면 걸음이 다시 읽어([`App::follow_config`])
    /// 그 읽기로 다시 단다(`App::relayer`) — 층이 서거나 설정이 멀쩡해지면 걷히고, 깨지면 선다.
    pub unlayered: Option<String>,
    /// 사용자 설정을 못 읽어 **지난 층을 들고 선** 까닭(moai-23pm). 위의 [`App::unlayered`] 와 한
    /// 물음이고 서는 자리가 다를 뿐이다 — 저쪽은 층을 못 세운 화면, 이쪽은 낡은 층을 든 화면.
    /// 둘은 함께 서지 않는다(층이 있거나 없거나다).
    ///
    /// **`Layer::problems` 에 안 싣는다.** 거기 실으면 배너가 `on_layer()` 로 막아 프로젝트 안에서는
    /// 아무 말이 없는데, `moai tui` 를 저장소 안에서 띄우면 거기 선다 — 깨진 설정은 시계로도 안 풀려
    /// (`user_config::Again::Never`) 사람이 `0` 을 안 누르면 그 세션 내내 모른다. 그 칸은 **읽힌**
    /// 설정의 틀린 줄을 대는 자리로 두고(등록 목록의 일이라 프로젝트 안과 상관이 없다), 파일을 통째로
    /// 못 읽은 것은 여기로 낸다.
    pub held: Option<String>,
    /// `--user` 로 **준 값 그대로**(`Ctx::user` 와 같다), 또는 누군지 묻는 칸에서
    /// 받은 것([`Mode::Ask`]). 쓸 때마다 `model::actor` 로 푼다 — 미리 풀어 두면
    /// 설정 없는 기계에서 읽기만 하려던 탐색기가 여는 순간 사람을 묻는다. 읽기는
    /// 묻지 않는다. **받은 것은 이 세션이 끝나면 사라진다** — 어디에도 적지 않는다.
    pub user: Option<String>,
    /// 누가 쓰는가를 푸는 길. 진짜 길은 `model::actor` 다. **시험이 갈아 끼운다** —
    /// 그쪽은 `MOAI_ACTOR` 와 이 기계의 git 설정을 읽어, 갈아 끼우지 않으면
    /// "누군지 모를 때" 를 시험한 결과가 돌리는 사람의 설정에 달린다.
    identify: fn(Option<&str>, &std::path::Path) -> crate::fail::R<crate::model::Actor>,
    /// 헤더가 적을 사람을 푼 것 — 열쇠는 [`Self::user`] 와 **그 프로젝트의 `naming`** 이다.
    /// **프레임마다 풀지 않는다**: `model::actor` 는 `git config` 를 프로세스로 **두 번**
    /// 띄우는데, 그리는 쪽에서 부르면 묵혀 둔 화면도 초당 셋(`TICK`), 도는 것이 있으면 초당
    /// 열여섯(`SPIN_TICK`), 읽는 동안에는 초당 예순(`LOAD_POLL`) 번 그것을 띄운다 — 키를
    /// 누르고 있는 내내도 같다. 묻는 칸에서 사람을 받아 `user` 가 바뀌면 열쇠가 어긋나 저절로
    /// 다시 푼다.
    ///
    /// **`naming` 도 열쇠에 든다**(리뷰). 묵힌 것은 `model::label` 을 이미 지난 글이고 그 모양은
    /// 프로젝트 설정이 정한다 — 열쇠에서 빼면 `naming = "email"` 인 프로젝트로 건너뛴 뒤에도
    /// 헤더가 떠난 프로젝트의 `이름 (메일)` 을 그대로 이고 있다. 설정은 화면만 바꾸는 것이라,
    /// 화면이 안 바뀌면 그 설정은 없는 것과 같다.
    header_user: Option<(Option<String>, crate::config::Naming, std::path::PathBuf, String)>,
    /// 상세의 굴린 자리. **왼쪽 커서를 옮기면 첫 줄로 돌아간다** — 다른
    /// 이슈를 보는데 굴린 자리가 남아 있으면 첫 줄부터 못 본다.
    pub detail: Scroll,
    /// 본문을 그리지 않고 원문 그대로 보는가. 그린 글은 기호가 지워져
    /// 되돌릴 수 없다 — 긁어 붙이거나 마크다운을 고칠 때 이 길이 필요하다.
    pub raw: bool,
    /// 표만 짓는 스레드([`App::follow_commits`]). **한 번에 하나만 돈다** — 도는 동안 다시 읽기가
    /// 또 들어오면 `commits_due` 만 세우고, 이것이 끝나면 곧바로 하나를 더 띄운다.
    commits_job: Option<(std::sync::mpsc::Receiver<Commits>, std::thread::JoinHandle<()>)>,
    /// 지금 든 표(나 도는 스레드)보다 새 표가 필요한가. 연 순간과 다시 읽은 뒤마다 선다 —
    /// **여는 읽기와 다시 읽기는 표를 안 짓는다**, 첫 화면도 쓰기도 git 이 이력을 걷는 동안
    /// 붙잡을 까닭이 없다.
    commits_due: bool,
    /// 스레드에서 짓고 있는 다시 읽기. 끝나면 [`App::follow`] 가 받아 들인다.
    /// 손잡이는 스레드가 죽었을 때 그 패닉을 루프로 되던지려고 든다.
    pending: Option<(std::sync::mpsc::Receiver<crate::fail::R<Fresh>>, std::thread::JoinHandle<()>)>,
    /// 탐색에서 접두어(`gg` 의 첫 `g`) 뒤를 기다리는 키 열. **`Mode` 가 아니다** — 탐색이 아닌
    /// 모드는 전부 글칸으로 가므로(`App::key`), 모드로 두면 기다리는 `g` 뒤의 `g` 가 글자로 샌다.
    chord: keys::Chord,
    /// 버린 다시 읽기(SPC v w·쓰기가 `pending` 을 버렸을 때)의 손잡이. 결과는 안 받지만
    /// **패닉은 받는다** — ratatui 의 패닉 훅은 어느 스레드에서 나든 터미널을 걷으므로,
    /// 손잡이를 같이 버리면 루프가 걷힌 화면에 모른 채 그린다. [`App::follow`] 가
    /// 걸음마다 끝난 것을 join 해 패닉이면 되던진다. [`DISCARDED_KEPT`] 개까지 든다.
    discarded: Vec<std::thread::JoinHandle<()>>,
    /// [`DISCARDED_KEPT`] 를 넘겨 **아직 도는 채로 놓은** 손잡이 수. 그 스레드가 터지면
    /// 터미널이 걷히는데 되던질 길이 없다 — 화면이 그것을 말한다(`draw::banner`).
    /// **붙박이다.** `trouble` 은 다음에 성공한 다시 읽기가 걷는데, 놓는 때가 곧 SPC v w·
    /// 쓰기가 새 읽기를 띄운 때라 몇백 ms 뒤에 사라진다. 놓은 스레드는 다시 볼 길이
    /// 없으므로 세션 내내 남긴다.
    let_go: usize,
    /// 다시 읽는 길. 진짜 길은 [`prepare`] 다. **시험이 갈아 끼운다** — 스레드에서
    /// 짓는 읽기가 패닉하는 때는 진짜 파일로는 못 만든다.
    read: fn(&Repo, bool, crate::i18n::Lang) -> crate::fail::R<Fresh>,
    /// 무엇을 보일까(moai-fmv5) — 칸·미룸 토글. 거름망과 따로 들어 Esc 가 안 푼다.
    pub view: view::View,
    /// 목록 차례와 그 방향(moai-55cp). 기본은 우선순위 차례다.
    pub order: keys::Sorting,
    /// 목록 줄에 켜 둔 열(moai-g7p8). 처음에는 원래 줄 그대로(id·우선순위·셈)에 열 이름 줄이 얹힌다.
    pub fields: view::Fields,
    /// **옛 `[read]`** — 설정 한 파일에 모여 있던 읽음(moai-bwce, 사용자 결정 3). 이 바이너리는 여기
    /// 다시 안 적고 **읽기만 한다**: 옛 줄을 건드리면 그것은 설정이 아니라 마이그레이션이고, 그 사이 도는
    /// 옛 바이너리나 옆 세션이 읽음을 잃는다. 프로젝트의 읽음 파일에 없는 id 만 여기서 든다
    /// ([`crate::read_marks::read`]). 설정이 바뀌면 [`App::follow_config`] 가 다시 든다.
    pub legacy_read: std::collections::BTreeMap<String, String>,
    /// 내가 누구인가 — `이름 (메일)`. **띄울 때, 프로젝트를 옮길 때, 묻는 칸에서 사람을 받을 때만**
    /// 푼다(moai-z9pc, moai-j038.vna) — 헤더(`told_user`)가 뿌리와 `user` 가 바뀔 때 다시 푸는 것과 같은
    /// 자다. 다시 읽기마다 부르지는 않는다 — 그 길이 열리면 "읽기는 사람을 묻지 않는다" 가 무너진다
    /// (`reading_never_asks_who`). 모르면 `None` 이고 그러면 [NEW] 가 한 줄도 안 선다.
    pub me: Option<String>,
    /// 오른쪽 상세 칸이 보이나(moai-ymnu). 숨기면 목록이 폭을 다 쓴다. 설정에 남는다.
    pub detail_open: bool,
    /// 설정에 적혀 있다고 이 세션이 아는 보기 — 읽은 뒤와 적은 뒤의 [`App::look_now`](moai-2kyl 단계 리뷰).
    /// **적을 때 이것과 지금의 차이만 옮긴다**(`Doc::merge_look`). 화면이 든 보기를 통째로 적으면 그사이
    /// 옆 탐색기·손·새 바이너리가 적은 것을 토글 한 번이 되돌린다. 새 바이너리가 적은 모르는 낱말을 들고
    /// 있다가 도로 싣던 것(moai-2bzp 리뷰)도 이것으로 선다 — 이 세션이 안 바꾼 것은 안 적는다.
    saved: crate::user_config::Look,
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
    /// **지난 프레임이 도는 것을 화면에 그렸는가.** 루프는 이것으로 다음에 빠른 걸음으로
    /// 깰지를 정한다(`cmd::tui::loop_until_quit`) — 쓰는 곳은 `draw::screen` 하나뿐이다.
    pub spun: bool,
    /// 다른 워크트리를 겹쳐 보는가. `w` 가 켜고 끈다 — CLI 의 `--worktree` 와 같은
    /// 길(`worktree::gather`)로 읽는다. **켜진 채로 시작한다**(moai-zcuh): 탐색기는
    /// 사람이 둘러보는 자리라 옆 워크트리에서 집은 일이 안 보이면 보드가 거짓말을
    /// 한다. 값은 여는 순간 `git` 한 번과 옆 스냅샷 읽기다. CLI 의 `--worktree` 는
    /// 그대로 끈 채로 둔다 — 기계가 읽는 출력의 모양을 안 바꾼다.
    pub worktree: bool,
    /// 프로젝트 층([`layer`]). **`None` 이면 등록한 것이 없고 오늘 탐색기 그대로다.** 층이
    /// 있으면 지금 선 곳(`layer.at`)이 층이거나 한 프로젝트 안이고, 층에 선 동안에는 위의
    /// 한 프로젝트 자리(`repo`·`issues`·`index`…)가 비었다.
    pub layer: Option<layer::Layer>,
    /// 한눈 보기에서 **접어 둔** 프로젝트(moai-eyre). 비어 있으면 다 펼친 것이다 — 사람이 `0` 을
    /// 눌러 여는 화면은 줄이 보이는 화면이라, 접힘을 기본으로 두면 그 화면이 옛 층과 똑같아진다.
    ///
    /// **경로로 든다**(층 줄의 정체와 같다, [`Anchor::Project`]) — 첨자로 들면 설정이 바뀌어 층이
    /// 다시 서는 순간 접은 것이 옆 프로젝트로 옮아간다. **화면에만 산다**: 펼침(`Site::expanded`)이
    /// 설정에 안 남는 것과 같은 까닭이다(사용자 결정 moai-7qot).
    folded: std::collections::HashSet<std::path::PathBuf>,
    /// 지금 줄을 읽고 있는 프로젝트의 저장소(moai-12yx) — 읽어 온 것을 [`Site`] 에 얹을 때 쓴다.
    /// 여는 것은 그 자리에서(`open_place`), 읽는 것은 스레드에서 하므로 그사이 이것이 든다.
    reading_repo: Option<Repo>,
    /// `Tab` 으로 **다 펴 달라**고 한 프로젝트 가운데 아직 줄이 안 온 것(moai-12yx). 읽어 온 줄을
    /// 들일 때 이것을 보고 편다 — 누를 때는 펼 줄이 아직 없다.
    deep: std::collections::HashSet<std::path::PathBuf>,
    /// 사용자 설정 파일의 자리 — **층이 없을 때** 등록(`a`)이 쓰는 곳이다. 층이 있으면 층이 읽은
    /// 파일(`Layer::config`)을 쓴다. `cmd::tui` 가 `user_config::path()` 로 넣고, 시험은 임시
    /// 파일을 준다 — 여기서 환경을 읽으면 시험이 돌리는 사람의 설정을 고친다.
    pub user_config: Option<std::path::PathBuf>,
    /// 사용자 설정 파일의 표식 — **걸음마다 잰다**(moai-en4u). 손으로 누르던 다시 읽기(`SPC r`)를 걷으며
    /// 그 키만 보던 둘을 자동 갱신에 태웠다: 옆 터미널의 `moai read` 가 적은 읽음과, 밖에서
    /// `moai project add|rm` 한 층의 줄. 둘 다 이 한 파일에 산다.
    ///
    /// **띄우는 길(`cmd::tui`)은 설정을 읽기 전에 재어 넣는다** — 읽은 뒤 첫 걸음에 재면 읽고 첫 걸음
    /// 사이에 옆이 쓴 것을 본 것으로 삼아, 설정이 다시 바뀔 때까지 안 보인다(`prepare` 가 표식을
    /// 읽기 전에 재는 것과 같은 까닭). `None` 은 아직 안 쟀다 — 첫 걸음이 재기만 한다(시험의 길).
    pub config_stamp: Option<crate::store::Stamp>,
    /// 마지막 읽기가 만난 탈 — 멀쩡했으면 `None`(moai-po6v). **표식만으로는 못 재는 것을 잰다**:
    /// 표식은 읽기 *전에* 재므로 진 읽기 뒤에도 파일의 것과 같고, 권한을 고치는 `chmod` 은 표식을
    /// 아예 안 바꾼다. 그래서 다시 읽을 때를 정하는 자는 표식이 아니라 이 갈래다
    /// ([`layer::owed`]). 층의 못 읽는 줄이 시계에 걸린 그 자([`layer::due`])를 그대로 쓴다 —
    /// 상수를 두 벌로 두면 한쪽만 바뀐다.
    ///
    /// 띄우는 길(`cmd::tui`)이 제 읽기의 것을 넣는다 — 안 넣으면 띄울 때 진 한 번이 세션 내내 남는다.
    pub config_tried: layer::Tried,
    /// **띄운 자리**(cwd). 고르기 창이 처음 여기서 연다. 시험은 임시 디렉터리를 준다.
    ///
    /// 이름이 [`App::here`] 와 겹치지 않게 둔다 — 그쪽은 *지금 선 프로젝트*, 곧 **쓰기가
    /// 닿는 곳**이다. 둘 다 `Option<PathBuf>` 라 괄호 하나를 빠뜨려도 컴파일되고, 그러면
    /// 생각 담기가 머리에 보인 곳 말고 띄운 자리에 쓰려 든다.
    pub launched_at: Option<std::path::PathBuf>,
    /// 고르기 창을 마지막으로 닫은 디렉터리 — 다시 열면 여기서 연다.
    pick_from: Option<std::path::PathBuf>,
    /// 생각 담기를 적을 외부 편집기(moai-08af). **있으면 `n` 이 안 폼 대신 이것을 연다** —
    /// 둘을 고르는 키나 설정은 없다(둘째 어휘). `cmd::tui` 가 띄울 때 한 번 고른다
    /// ([`jotfile::pick`]). 시험이 세운 App 은 없어 안 폼을 연다 — 여기서 환경을 읽으면
    /// 시험이 돌리는 사람의 `$EDITOR` 에 달린다.
    pub editor: Option<String>,
    /// 루프에 맡긴 "편집기로 열어 달라". **App 은 터미널을 모른다** — 터미널을 내리고 편집기를
    /// 기다리고 다시 올리는 것은 루프가 하고, 받은 글은 [`App::edited`] 로 돌려준다.
    pub edit: Option<Edit>,
}

/// 편집기로 적어 달라는 요청. 담을 곳은 **여는 순간 박힌 것**이다(moai-fccv) — 편집기가
/// 도는 사이 층이 다시 읽혀도 돌아온 글은 이 곳으로 간다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub into: Option<form::Target>,
    /// 파일에 먼저 적을 글([`jotfile::template`]).
    pub text: String,
    /// 띄울 편집기([`App::editor`] 를 연 순간 옮긴 것).
    pub editor: String,
}

impl Site {
    /// 읽은 한 프로젝트를 세운다. **색인과 [`Ground`] 는 부른 쪽이 잰 것을 받는다**([`measure`]) —
    /// 여기서 다시 재면 같은 훑기를 두 번 하고, 그 훑기는 이슈 수에 비례한다.
    ///
    /// 여는 길([`App::open`])과 한눈 보기가 프로젝트를 읽는 길(moai-12yx)이 **이 몸 하나**를 지난다 —
    /// 두 벌로 적으면 한쪽만 고쳐져 같은 프로젝트가 화면 둘에서 달리 선다.
    fn of(
        issues: Vec<Issue>,
        index: Index,
        ground: Ground,
        cfg: Config,
        path: Path,
        unreadable: Vec<Option<String>>,
    ) -> Site {
        Site {
            // 기본값은 도구의 것이다(moai-9it4). 한때 여기가 `Lang::Ko` 였고 그 한 줄이 "탐색기는
            // 아직 한국어로 선다" 는 뜻이었다 — 말이 안 닿은 자리에서 영어를 집으면 한국어 화면
            // 한가운데 몇 줄만 영어로 섰기 때문이다. 이제 탐색기의 글이 다 말묶음에서 오므로 그
            // 버팀목이 필요 없다. 부른 쪽(`cmd/tui.rs`)이 고른 말로 갈아 끼운다.
            lang: crate::i18n::Lang::default(),
            // 들어간 채로 시작하면(`--path`) 나올 층마다 기억 자리를 만들어 둔다.
            remembered: vec![0; path.len()],
            keep: vec![true; issues.len()],
            issues,
            index,
            ground,
            cfg,
            path,
            repo: None,
            now: crate::model::now(),
            unreadable,
            warnings: 0,
            stamp: None,
            seen: Default::default(),
            read_stamp: None,
            read_tried: layer::Tried::default(),
            read_at: None,
            watched: Vec::new(),
            commits: Commits::new(),
            commit_ids: Default::default(),
            shown: Vec::new(),
            seen_view: None,
            lit: Default::default(),
            unread: Default::default(),
            expanded: Default::default(),
            origin: crate::worktree::Origin::default(),
            elsewhere: Vec::new(),
            unfound: None,
        }
    }

    /// 이 줄이 목록에 서는가 — 거름망이 안 가렸고, 보기가 안 가렸거나 검색이 보기를 걷었다([`Site::veil`]).
    /// **검색이 걸렸는가는 화면의 것이라 받아서 쓴다**(moai-eyre) — 한눈 보기는 프로젝트마다 이 판정을
    /// 부르는데, 검색은 그 화면 하나에 하나뿐이다.
    pub fn visible(&self, at: usize, searching: bool) -> bool {
        let v = self.veil(at);
        !v.filtered && (!v.viewed || searching)
    }

    /// 그 줄이 도는가 — **지금 누가 손대고 있는 줄**이다.
    ///
    /// **줄마다 묻는 자는 이것 하나다** — 도는 글리프(`draw::glyph_of`)·칸별 건수·빛줄기가
    /// 모두 이 답으로 그려지고, 루프는 따로 판단하지 않고 **그려진 화면**으로 깬다
    /// (`App::spun`, moai-5jh6). 그래서 도는데 안 깨우거나 안 도는데 깨우는 조합이 없다.
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
        let busy = !crate::report::is_group(i) || self.ground.stands.get(&i.id).is_some_and(|s| s.busy);
        // **도는 칸은 설정이 정한다**(moai-q59j) — 시작한 칸 모두(`Config::is_started`). 칸 이름
        // `"in_progress"` 를 박아 두면 칸 이름을 바꾼 설정에서 아무것도 안 돌았다. 설정이 모르는
        // 칸은 안 돈다 — 묶음의 `busy` 와 같은 자다(`report::Stand::busy`). 바쁜 묶음은 늘 시작한
        // 칸으로 읽히므로 묶음에는 이 검사가 답을 안 바꾼다 — 줄(일)을 위한 것이다.
        let col = self.column(at);
        busy && self.cfg.knows(col) && self.cfg.is_started(col)
    }

    /// 그 줄이 **서 있는** 칸 — 묶음이면 멤버에서 읽은 칸, 아니면 제 칸. CLI 가
    /// 묻는 `report::column` 과 같은 답이다 — 묶음만 읽은 칸을 받는 것까지 같다.
    pub fn column(&self, at: usize) -> &str {
        let i = &self.issues[at];
        crate::report::is_group(i)
            .then(|| self.ground.stands.get(&i.id).map(|s| s.column.as_str()))
            .flatten()
            .unwrap_or(i.status.as_str())
    }

    /// 그 줄이 묶음이면 막을 때 무엇을 기다리는가와 미뤄 뺀 멤버(`report::Stand::waiting`·
    /// `aside`). 묶음이 아니면 제 칸대로다. 막음을 가를 때 [`Site::column`] 과 함께
    /// `report::blocker` 에 댄다.
    ///
    /// **첨자를 받으므로 그 첨자가 사는 프로젝트에 묻는다**(리뷰) — `App` 에 두었던 때는 한눈
    /// 보기의 남의 줄이 제 색인에서 푼 첨자를 여기로 들고 와, 줄이 비어 있는 지금 선 프로젝트의
    /// 목록을 그 첨자로 짚어 그 자리에서 죽었다.
    pub fn waits(&self, at: usize) -> (crate::report::Waiting, &[String]) {
        let i = &self.issues[at];
        crate::report::is_group(i)
            .then(|| self.ground.stands.get(&i.id))
            .flatten()
            .map_or((crate::report::Waiting::Live, &[][..]), |s| (s.waiting, s.aside.as_slice()))
    }

    /// id 를 제목으로 푼다. 없으면 **끊겼다고 적는다** — id 만 내면 그것이 그저 제목 없는 줄인지
    /// 없는 것을 가리키는 참조인지 알 길이 없다. 훑지 않는다: 막는 것마다·소속마다·프레임마다 불린다.
    pub fn title_of(&self, id: &str) -> String {
        match self.index.find(id) {
            Some(at) => self.issues[at].title.clone(),
            // 없는 것을 가리키는 참조에 붙이는 말은 **키 하나로 통일한다** — 자리마다 다른 말을
            // 쓰면 같은 깨짐을 서로 다른 일로 읽는다.
            None => format!("{id}  {}", say(self.lang, "tui.missing")),
        }
    }

    /// 그 줄에 닿은 커밋. **줄이 온 워크트리의 가지에서 읽는다** — `show` 와 같은 까닭이다:
    /// `--worktree` 로 옆에서 집은 일을 고친 커밋은 저쪽 가지에만 있다. 표가 없으면 빈 것이다.
    pub fn commits_of(&self, id: &str) -> &[crate::git::Commit] {
        let root = self.origin.root(id).or(self.repo.as_ref().map(|r| r.here()));
        root.and_then(|r| self.commits.get(r)).and_then(|t| t.get(id)).map_or(&[], Vec::as_slice)
    }

    /// 보기(`SPC v`)가 이 줄을 보이는가 — 검색과 상관없이. `shown` 이 빈 때는 보인다.
    fn view_shows(&self, at: usize) -> bool {
        self.shown.get(at).copied().unwrap_or(true)
    }

    /// 이 줄을 **무엇이 가리는가** — 거름망(`keep`)과 보기(`shown`) 각각. **판정은 여기 하나다**
    /// (moai-1jay) — 목록·검색 셈·가린 셈·쓰기 알림·빈 목록의 까닭이 저마다 마스크를 읽으면, 숨기는
    /// 까닭이 하나 늘거나 빈 `shown` 의 기본이 바뀐 날 한쪽만 고쳐져 셈이 목록과, 알림이 누른 키와
    /// 어긋난다(moai-fmv5 가 그 알림을 만든 바로 그 실패). `shown` 이 빈 때(층에서 막 내려와 아직 안
    /// 센 때)는 보기가 안 가린다.
    ///
    /// **`viewed` 는 검색 중에도 보기가 숨기는가 그대로다** — 검색이 보기를 걷는 것은 목록에 서는가
    /// ([`Self::visible`])의 일이다. 여기서 검색을 보고 `viewed` 를 끄면, 검색 중에 쓴 줄이 검색에도 보기에도
    /// 가렸을 때 쓰기 알림이 "Esc 로 푼다" 만 대고, Esc 가 검색을 풀자마자 보기가 그 줄을 도로 가린다.
    fn veil(&self, at: usize) -> Veil {
        Veil { filtered: !self.keep.get(at).copied().unwrap_or(true), viewed: !self.view_shows(at) }
    }

    /// 보기만으로도 이 줄이 목록에 서는가 — 제 줄이 보이거나, 폴더면 밑에 보기가 보이는 줄이 있다. 목록이
    /// 폴더를 남기는 자(`Index::entries_sorted` 의 "밑에 걸린 것")와 같다. 밑은 [`App::see`] 가 센 `lit` 에서
    /// 읽는다 — 폴더마다 이슈 전부를 훑지 않는다.
    fn stands_in_view(&self, at: usize) -> bool {
        if self.view_shows(at) {
            return true;
        }
        if !self.index.is_dir(&self.issues, at) {
            return false;
        }
        let mut below = self.index.home_of(at).clone();
        below.push(self.index.seg_of(&self.issues, at));
        self.lit.contains(&below)
    }
}

/// 탐색기가 읽음으로 **지켜보는 표식** — [`crate::read_marks::read`] 가 실제로 여는 파일 전부를
/// 잰다(moai-65as).
///
/// 한때 이 값은 쓰는 자리 하나(`read_marks::path_for`)의 표식이었다. moai-bdej 가 못 푼 판의 도장을
/// 대기 자리로 보내고 다음 성한 쓰기가 합치게 하면서, 읽기가 여는 파일이 셋으로 늘었는데 재는 자는
/// 하나로 남았다 — 옆 터미널이 대기 자리에 적어도 이 표식이 안 움직여 [`App::load_read`] 가 안 돌고,
/// 합쳐질 줄이 화면에 내내 [NEW] 로 섰다.
///
/// **무엇을 재는가는 [`ReadStamp::of`] 한 자리가 정한다.** [`App::read_marks_of`] 와
/// [`App::follow_read`] 가 저마다 파일을 골라 재던 판이 이 버그다: 한쪽만 자리를 늘리면 표식이 든
/// 것과 읽기가 연 것이 갈려, 걸음이 제가 이미 읽은 것을 다시 읽거나 영영 안 읽는다.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadStamp {
    /// 읽고 쓰는 자리([`crate::read_marks::Place::at`]).
    at: Stamp,
    /// 대기 자리([`crate::read_marks::Place::pending`]). **있는가가 곧 "아직 안 합쳤다" 라** 생기는
    /// 것도 지워지는 것도 이 값이 바뀌는 일이다 — `exists` 를 따로 묻지 않는다. 못 푼 판에서는
    /// `at` 이 곧 그 자리라 잴 것이 없다(`Place::pending` 이 `None` 이다).
    pending: Stamp,
    /// 옛 철자 파일들([`crate::read_marks::Place::past`]). **지금 자리가 없을 때만 잰다** —
    /// `read_marks::overlay_place` 가 그때만 열기 때문이다. 흔한 판은 두 철자가 같아 비어 있다.
    past: Vec<Stamp>,
}

impl ReadStamp {
    /// 그 자리에서 읽기가 여는 파일을 전부 잰다.
    ///
    /// **`stat` 은 늘 하나, 흔하게 둘이다.** 지금 자리와 (못 푼 판이 아니면) 대기 자리다. 옛 자리는
    /// 지금 자리가 없을 때만 더 재는데, 그 판정을 방금 잰 `at` 에서 읽어 `exists` 를 한 번 더 묻지
    /// 않는다 — 못 읽은 파일(권한)도 없는 것으로 세지만, 그때 치르는 것은 안 열 파일을 잰 `stat`
    /// 하나이고 답은 안 틀린다.
    fn of(place: &crate::read_marks::Place) -> ReadStamp {
        let at = crate::store::stamp(&place.at);
        let pending = place.pending.as_deref().and_then(crate::store::stamp);
        let past = match at.is_none() {
            true => place.past.iter().map(|p| crate::store::stamp(p)).collect(),
            false => Vec::new(),
        };
        ReadStamp { at, pending, past }
    }
}

/// 한 걸음의 읽음 읽기([`App::read_marks_of`]).
struct Got {
    marks: crate::read_marks::Marks,
    stamp: ReadStamp,
    /// 읽음 **자리**를 고르다 만난 까닭 — 파일 안의 건너뛴 줄과 **다른 갈래**다(moai-hzfu).
    /// `Marks::problems` 는 둘을 한 자루에 담는데, 건너뛴 줄이 없으면 이것이 `first()` 가 되어
    /// "읽음에 이상한 줄이 있다" 로 이름 붙었다.
    where_why: Option<String>,
}

impl App {
    /// 저장소 없이 세운다 — 시험과 눈으로 보는 길이 이것을 쓴다. 진짜 길은
    /// [`App::open`] 이고, 그쪽은 색인과 [`Ground`] 를 부른 쪽에서 받는다.
    #[cfg(test)]
    pub fn new(issues: Vec<Issue>, cfg: Config, path: Path) -> App {
        let (index, ground) = measure(&issues, &cfg);
        App::build(issues, index, ground, cfg, path, Vec::new())
    }

    /// 저장소에서 읽어 세운다.
    ///
    /// **색인과 [`Ground`] 는 부른 쪽이 이미 잰 것을 받는다**([`measure`]) — 여는 데서 다시 만들면
    /// 같은 훑기를 두 번 하고, 그 훑기는 이슈 수에 비례한다.
    /// **표식도 부른 쪽이 읽기 전에 잰 것을 받는다** — 읽고 나서 재면 그
    /// 사이에 떨어진 쓰기가 "이미 본 것" 으로 적혀 영영 안 보인다.
    pub fn open(repo: Repo, load: Load, index: Index, ground: Ground, path: Path, stamp: Stamp) -> App {
        let cfg = repo.config.clone();
        let ids = load.errors.iter().map(|e| e.id.clone()).collect();
        let mut app = App::build(load.issues, index, ground, cfg, path, ids);
        app.site.stamp = stamp;
        // 띄울 때 읽은 것도 들인 읽기다 — 안 찍으면 조용한 저장소에서 시계로는 영영 다시 안 읽는다.
        app.site.read_at = Some(std::time::Instant::now());
        app.site.repo = Some(repo);
        app
    }

    /// 여는 읽기가 겹쳐 본 것을 들인다(`worktree::gather`). 못 읽는 줄은 겹친 뒤의 자로
    /// 다시 센다 — 옆에서 산 줄로 온 id 를 여기서도 못 읽는 줄로 세면 경고가 [`prepare`]
    /// 로 다시 읽은 화면과 갈린다.
    ///
    /// `swept` 은 옆을 실제로 겹쳤는가다(`Gathered::swept`) — 자리 판정이 그것으로 잰다([`placed`]).
    pub fn overlaid(
        mut self,
        origin: crate::worktree::Origin,
        mut elsewhere: Vec<String>,
        watched: Vec<(std::path::PathBuf, Stamp)>,
        swept: bool,
        // 겹치며 이미 판 옆 스냅샷([`crate::worktree::Gathered::sides`]) — 자리 판정이 같은
        // 파일을 다시 열어 파지 않게 넘긴다(moai-kos1). 첫 화면이 바로 이 값을 물었다.
        sides: &[crate::worktree::Side],
        // 겹치기 전에 잰 제 스냅샷([`crate::worktree::Gathered::mine`], moai-mafv) — 같은 까닭이다.
        mine: &crate::worktree::Floor,
    ) -> App {
        let unreadable: Vec<Option<String>> = origin
            .unreadable(self.site.unreadable.iter().map(Option::as_deref))
            .into_iter()
            .map(|id| id.map(str::to_string))
            .collect();
        // `build` 가 이미 한 번 셌다. 못 읽는 줄의 자가 안 바뀌었으면 같은 훑기를 다시 하지 않는다.
        if unreadable != self.site.unreadable {
            self.site.unreadable = unreadable;
            self.site.warnings = warnings_of(&self.site.issues, &self.site.unreadable, &self.site.cfg, &self.site.now);
        }
        // 자리 판정은 저장소가 있어야 잰다 — `build` 는 줄만 받아 못 쟀다. 다시 읽기는
        // `prepare` 가 같은 자로 세어 [`Fresh::warnings`] 에 실어 온다. 위의 셈에 **한 번만** 더한다 —
        // 이 길은 여는 읽기 하나가 한 번 지난다.
        if let Some(repo) = &self.site.repo {
            let dug = crate::worktree::dug(sides, mine);
            let (lost, said) =
                placed(repo, &self.site.issues, self.worktree && swept, &self.site.now, &dug, self.site.lang);
            self.site.warnings += lost;
            elsewhere.extend(said);
        }
        self.site.origin = origin;
        self.site.elsewhere = elsewhere;
        self.site.watched = watched;
        self
    }

    fn build(
        issues: Vec<Issue>,
        index: Index,
        ground: Ground,
        cfg: Config,
        path: Path,
        unreadable_ids: Vec<Option<String>>,
    ) -> App {
        let mut app = App {
            site: Site::of(issues, index, ground, cfg, path, unreadable_ids),
            cursor: 0,
            mode: Mode::Browse,
            focus: Pane::default(),
            detail: Scroll::default(),
            raw: false,
            filter_text: None,
            grep_in: GrepIn::All,
            grep_was: None,
            trouble: None,
            unlayered: None,
            held: None,
            write_failed: false,
            notice: None,
            body: None,
            user: None,
            identify: crate::model::actor,
            header_user: None,
            commits_job: None,
            commits_due: true,
            pending: None,
            chord: keys::Chord::default(),
            discarded: Vec::new(),
            let_go: 0,
            read: prepare,
            // **처음에는 done 을 숨긴다**(사람의 결정, 2026-09-14). 끝난 것이 목록을 채워 지금 볼
            // 것을 덮었고, 걷으려면 `status=todo,in_progress,review` 를 손으로 적어야 했다.
            legacy_read: Default::default(),
            view: view::View::hiding(crate::config::DONE),
            order: Default::default(),
            fields: Default::default(),
            detail_open: true,
            me: None,
            saved: Default::default(),
            list: Scroll::default(),
            quit: false,
            spin: 0,
            spun: false,
            worktree: true,
            layer: None,
            folded: Default::default(),
            reading_repo: None,
            deep: Default::default(),
            user_config: None,
            config_stamp: None,
            config_tried: layer::Tried::default(),
            launched_at: None,
            pick_from: None,
            editor: None,
            edit: None,
        };
        // **시험은 한국어로 잰다**(moai-9it4). 그림 시험의 글을 영어로 다시 적으면 `ko` 표를
        // 재는 자리가 통째로 없어진다 — 영어 표는 `english_has_every_key_the_source_asks_for`
        // 가 키마다 보지만, 한국어 표가 그 키를 실제로 드는지는 아무도 안 본다. 진짜 화면의
        // 말은 `cmd/tui.rs` 가 `Ctx::lang` 으로 넣고, 그 길은 `tests/cli.rs` 가 잰다.
        #[cfg(test)]
        {
            app.site.lang = crate::i18n::Lang::Ko;
        }
        // 한 번만 센다. `report::status` 는 이슈 수에 비례한 훑기라, 못 읽는 줄
        // 수를 나중에 넣겠다고 두 번 부르면 그 절반이 버려진다.
        app.site.warnings = warnings_of(&app.site.issues, &app.site.unreadable, &app.site.cfg, &app.site.now);
        app.see();
        app
    }

    /// 다시 읽는다. **거름망과 있던 자리는 지키려 애쓴다** — 갱신 한 번에
    /// 하던 일이 흩어지면 갱신을 꺼리게 되고, 그러면 낡은 화면을 본다.
    ///
    /// **사람이 누른 갱신(SPC v w·쓰기 뒤)은 그 자리에서 읽는다.** 누른 사람은 결과를 기다리고
    /// 있고, `w` 는 켠 뜻대로 읽힌 화면이 곧바로 서야 한다. 스레드에서 짓던 것이
    /// 있으면 버린다 — 누르기 **전에** 시작한 읽기라 늦게 도착하면 방금 읽은 것을
    /// 옛 것으로 덮는다(`w` 를 끄기 전 설정으로 읽은 것이면 더더욱).
    pub fn reload(&mut self) {
        // 층에는 다시 읽을 저장소가 없다 — 층에 서면 `repo` 가 빈다(`leave_project`). 층의 줄은 제 표식과
        // 시계([`App::follow_layer`])가, 등록 목록은 설정 파일의 표식([`App::follow_config`])이 따라간다.
        if self.site.repo.is_none() {
            return;
        }
        if let Some((_, handle)) = self.pending.take() {
            self.discard(handle);
        }
        let Some(repo) = &self.site.repo else { return };
        let fresh = (self.read)(repo, self.worktree, self.site.lang);
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
            Ok(f) if self.site.repo.as_ref().is_none_or(|r| r.root != f.root) => {}
            Ok(f) => self.apply_fresh(f),
            Err(e) => {
                self.trouble = Some(fill(say(self.site.lang, "tui.reload.failed"), &[("why", &e.to_string())]));
                self.write_failed = false;
            }
        }
    }

    /// 탐색기가 `issues.jsonl` 을 바꾸는 **유일한 길.** 모든 쓰기가 여기를 지난다.
    ///
    /// CLI 와 같은 [`Repo::with_write`] 를 부른다 — 쓰기 경로가 화면 쪽에 따로
    /// 서면 락·재읽기·검증·원자적 교체를 또 반쯤 구현하게 되고, 옛 `write.rs` 가
    /// 그렇게 부풀었다. 닫는 함수가 받는 목록은 **락 안에서 다시 읽은 것**이다.
    /// 화면이 들고 있는 `self.site.issues` 는 낡았을 수 있으니 그것을 보고 판단하지 않는다.
    ///
    /// **쓰고 나면 [`App::reload`] 로 다시 읽는다.** 만든 줄을 `self.site.issues` 에 손으로
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
        let Some(repo) = &self.site.repo else {
            self.trouble = Some(say(self.site.lang, "tui.write.no_repo").into());
            self.write_failed = true;
            return None;
        };
        let by = (self.identify)(self.user.as_deref(), &repo.root);
        if let Err(e) = &by
            && e.code == crate::fail::code::NO_ACTOR
        {
            let back = std::mem::replace(&mut self.mode, Mode::Browse);
            let why = e.message.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
            self.mode = Mode::Ask(Ask { input: Input::default(), error: None, why, back: Box::new(back), then: retry });
            return None;
        }
        // 저널 실패는 프로세스 전체에 쌓인다 — 이 쓰기 뒤에 이 저장소에 새로 선 것만 이 쓰기의 것이다.
        let missed_before = crate::store::journal_misses().len();
        let root = repo.root.clone();
        let written = by.and_then(|by| repo.with_write(|issues, cfg, reserved| f(issues, cfg, reserved, &by)));
        match written {
            Ok(touched) => {
                // 담긴 것은 참이라 성공으로 닫는다 — 실패로 내면 폼이 열린 채 남아 다시 누르면
                // 같은 것이 둘 선다(moai-52z9). 이력을 못 남긴 것은 알림 끝에 붙인다.
                let unjournaled = crate::store::journal_misses()
                    .into_iter()
                    .skip(missed_before)
                    .find(|(r, _)| *r == root)
                    .map(|(_, why)| {
                        fill(say(self.site.lang, "tui.write.unjournaled"), &[("why", &crate::text::one_line(&why))])
                    })
                    .unwrap_or_default();
                self.write_failed = false;
                self.reload();
                let Touched { id, done } = touched;
                // 층이 있으면 **어느 프로젝트에** 담겼는지도 댄다 — 같은 id 가 두 프로젝트에 있을 수
                // 있어 id 만으로는 어디인지 모른다(moai-fccv). 층이 없으면 프로젝트는 하나뿐이다.
                let what = match self.project() {
                    Some(p) => format!("{} · {id}", crate::text::one_line(&p.name)),
                    None => id.clone(),
                };
                let lang = self.site.lang;
                let told = match self.land(&id) {
                    Landing::Shown => fill(say(lang, "tui.write.landed"), &[("done", done), ("what", &what)]),
                    // **무엇이 가렸는지 가른다**(moai-fmv5) — 보기가 가린 줄에 "Esc 로 푼다" 를 대면
                    // Esc 는 거름망만 풀어 누른 키가 아무것도 안 한다. **둘 다 가렸으면 둘 다 댄다**
                    // (moai-2kyl 단계 리뷰) — 하나만 대면 그 키를 눌러도 다른 쪽에 여전히 가린다.
                    Landing::Hidden => {
                        let veil = self.site.index.find(&id).map(|at| self.site.veil(at)).unwrap_or_default();
                        let clear = keys::label(keys::BROWSE, keys::Browse::ClearFilter);
                        let show = keys::label(keys::BROWSE, keys::Browse::ShowAll);
                        match veil {
                            Veil { filtered: true, viewed: true } => fill(
                                say(lang, "tui.write.veiled_both"),
                                &[("done", done), ("what", &what), ("clear", &clear), ("show", &show)],
                            ),
                            Veil { filtered: false, viewed: true } => fill(
                                say(lang, "tui.write.veiled_view"),
                                &[("done", done), ("what", &what), ("show", &show)],
                            ),
                            Veil { filtered: true, viewed: false } => fill(
                                say(lang, "tui.write.veiled_filter"),
                                &[("done", done), ("what", &what), ("clear", &clear)],
                            ),
                            // 둘 다 안 가렸는데 줄이 안 섰다 — 오늘은 닿지 않는 갈래다. 숨기는 까닭이 셋째로
                            // 늘면 여기로 떨어지는데, 그때 거름망을 대면 누른 키가 아무것도 안 한다(moai-1jay).
                            Veil { filtered: false, viewed: false } => {
                                fill(say(lang, "tui.write.veiled"), &[("done", done), ("what", &what)])
                            }
                        }
                    }
                    // 다시 읽기가 실패했으면 그 까닭은 `trouble` 이 따로 댄다. 담긴 것은 참이다.
                    Landing::Missing => fill(say(lang, "tui.write.gone"), &[("done", done), ("what", &what)]),
                };
                self.notice = Some(told + &unjournaled);
                Some(id)
            }
            // 거절문은 여러 줄일 수 있다(누군지 모를 때는 고칠 명령까지 낸다). 배너는
            // 한 줄이라 줄바꿈이 그대로 가면 그림이 찢어진다 — 한 줄로 잇는다.
            Err(e) => {
                let why: Vec<&str> = e.message.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                self.trouble = Some(fill(say(self.site.lang, "tui.write.failed"), &[("why", &why.join("  "))]));
                self.write_failed = true;
                None
            }
        }
    }

    /// 헤더가 적을 사람. 처음 한 번만 풀고 [`Self::user`] 나 그 프로젝트의 `naming` 이
    /// 바뀌면 다시 푼다 — 까닭은 `header_user` 에 적었다.
    ///
    /// **누군지 몰라도 묻지 않는다.** 여는 화면은 읽기고, 읽기는 사람을 묻지 않는다 —
    /// 여기서 [`Mode::Ask`] 를 세우면 설정 없는 기계에서 탐색기가 묻는 칸으로 열린다.
    pub fn told_user(&mut self) -> &str {
        // 열쇠는 **견주기만** 한다 — 프레임마다 도는 자리라 짓지 않는다.
        let at = self.user_root();
        let fresh = self
            .header_user
            .as_ref()
            .is_none_or(|(u, n, r, _)| *u != self.user || *n != self.site.cfg.naming || r.as_path() != at);
        if fresh {
            let (at, naming) = (at.to_path_buf(), self.site.cfg.naming);
            let said = (self.identify)(self.user.as_deref(), &at)
                .map(|a| crate::model::label(&a.name, Some(&a.email), naming))
                .unwrap_or_else(|_| "—".into());
            self.header_user = Some((self.user.clone(), naming, at, said));
        }
        self.header_user.as_ref().map_or("—", |(_, _, _, said)| said.as_str())
    }

    /// 사람을 물을 자리 — 선 프로젝트의 뿌리다(moai-d3sy). 층에 서 있으면 아직 프로젝트가
    /// 없으니 띄운 자리에서 읽고, 그것도 없으면 지금 자리다. **뿌리가 바뀌면 사람도 다시
    /// 푼다** — 프로젝트마다 git 설정이 다를 수 있고, 헤더는 지금 선 프로젝트를 말해야 한다.
    fn user_root(&self) -> &std::path::Path {
        self.site
            .repo
            .as_ref()
            .map(|r| r.root.as_path())
            .or(self.launched_at.as_deref())
            .unwrap_or(std::path::Path::new("."))
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
        self.site.stamp = f.stamp;
        self.site.unreadable = f.unreadable;
        self.site.origin = f.origin;
        self.site.elsewhere = f.elsewhere;
        self.site.unfound = f.unfound;
        // 표는 **표식이 움직였을 때** 다음 걸음에 스레드가 짓는다(`App::commits`·
        // `follow_commits`). 도는 것이 있으면 그 답은 받되 이 읽기보다 낡았을 수 있어
        // 끝나는 대로 하나를 더 띄운다.
        // **표가 모르는 id 가 들어왔을 때도 짓는다**(`App::commit_ids`) — 표는 낱말을 준 id 와
        // 견줘 서므로, 커밋이 먼저 있고 줄이 나중에 온 자리는 이것 없이는 영영 빈 칸이다.
        // 칸 옮기기·메모처럼 id 가 그대로인 쓰기는 여기서 안 걸려 걷기를 새로 사지 않는다.
        self.commits_due |=
            self.site.watched != f.watched || f.issues.iter().any(|i| !self.site.commit_ids.contains(&i.id));
        self.site.watched = f.watched;
        self.site.read_at = Some(std::time::Instant::now());
        self.site.warnings = f.warnings;
        self.take(f.issues, f.index, f.ground, f.now);
    }

    /// 새 자료를 받아들이고 어긋난 것을 손본다. **시험이 저장소 없이 부른다** — 진짜
    /// 길은 스레드에서 셈을 마친 [`Fresh`] 를 [`App::apply_fresh`] 로 들인다.
    ///
    /// **커서는 번호가 아니라 정체로 따라간다**([`Anchor`]). 보던 줄이 아직 보이면
    /// 그 줄에 서고, 사라졌으면(지워졌거나 거름망에 빠졌으면) 전처럼 그 번호를 목록
    /// 안으로 자른 자리에 선다.
    #[cfg(test)]
    pub fn adopt(&mut self, issues: Vec<Issue>) {
        let (index, ground) = measure(&issues, &self.site.cfg);
        let now = crate::model::now();
        self.site.warnings = warnings_of(&issues, &self.site.unreadable, &self.site.cfg, &now);
        self.take(issues, index, ground, now);
    }

    /// 이미 센 자료를 들이고 커서·경로·거름망을 맞춘다 — [`App::adopt`] 와 스레드에서
    /// 지어 온 것([`Fresh`])이 함께 지나는 길이다.
    fn take(&mut self, issues: Vec<Issue>, index: Index, ground: Ground, now: String) {
        // **옛 자료로 잰다** — 줄의 첨자는 옛 `issues` 를 가리킨다.
        let held = self.current().map(|r| self.anchor_of(&r));
        self.site.issues = issues;
        self.site.index = index;
        self.site.ground = ground;
        self.site.now = now;
        // 줄이 바뀌었다 — 걸어 둔 보기는 옛 줄의 것이다(moai-m59y).
        self.site.seen_view = None;
        // 안 읽음은 **줄이 바뀔 때** 센다 — 보기 토글(`look`)도 지나는 `regrip` 에 두면 칸 하나 숨길
        // 때마다 저장소를 걷는다(moai-j038.vna).
        self.recount_unread();
        self.repair_path();
        // 거름망은 이슈 첨자에 매인 것이라 반드시 다시 센다.
        self.reapply();
        self.regrip(held);
    }

    /// 줄이 바뀐 뒤(다시 읽기·보기 토글) 보기를 다시 세고 **붙들어 둔 정체의 줄에 커서를 다시 세운다.**
    /// 그 줄이 사라졌으면(지워졌거나 가려졌으면) 전처럼 그 번호를 목록 안으로 자른 자리에 선다.
    /// `take`·`look` 이 같은 규칙을 저마다 다른 모양으로 적고 있었다(moai-y61p 단계 리뷰).
    ///
    /// **굴린 자리는 같은 줄일 때만 둔다.** 다른 이슈로 옮겨 섰는데 굴린 수가 남으면
    /// 그 이슈를 첫 줄부터 못 본다 — 커서를 옮길 때 0 으로 되돌리는 것(`move_to`)과
    /// 같은 까닭이다. 같은 줄이면 본문이 바뀌었어도 두고, 넘치면 그림이 자른다.
    fn regrip(&mut self, held: Option<Anchor>) {
        self.see();
        let rows = self.rows();
        let at = held.as_ref().and_then(|a| self.row_of(&rows, a)).unwrap_or(self.cursor);
        self.stand(&rows, at, held.as_ref());
    }

    /// 이미 센 목록의 `at` 에 **목록 안으로 잘라** 선다. 그 자리의 줄이 `held` 가 아니면 상세를
    /// 첫 줄로 되감는다(moai-go4o).
    ///
    /// **정체로 가른다, 번호로 가르지 않는다.** 뺀 줄의 번호에 다음 프로젝트가 올라서면 번호는
    /// 같아도 다른 것을 보고, 거꾸로 차례가 바뀌어 번호가 밀려도 정체가 같으면 같은 것을 본다 —
    /// 굴린 자리가 남으면 다른 줄을 첫 줄부터 못 본다(`move_to` 와 같은 까닭).
    ///
    /// 커서를 다시 세우는 자리 — 다시 읽기([`App::regrip`]), 거름망([`App::settle`]), 층 다시 세우기
    /// (`relayer`) — 가 **어디에 서느냐**만 저마다 고르고 서는 법은 이것 하나를 탄다.
    /// 한때 넷이 저마다 적어, 한쪽은 정체로 한쪽은 경로로 가르고 되감는 갈래 하나는 죽은 코드였고,
    /// 목록을 두세 번 셌다(`rows()` 뒤 `current()`). 목록은 부르는 쪽이 한 번 세어 넘긴다.
    fn stand(&mut self, rows: &[Row], at: usize, held: Option<&Anchor>) {
        self.cursor = at.min(rows.len().saturating_sub(1));
        if rows.get(self.cursor).map(|r| self.anchor_of(r)).as_ref() != held {
            self.detail.rewind();
        }
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
        let Some(at) = self.site.index.find(id) else { return Landing::Missing };
        let home = self.site.index.home_of(at).clone();
        let was = std::mem::replace(&mut self.site.path, home);
        let want = Anchor::Issue(id.to_string());
        let rows = self.rows();
        let Some(row) = self.row_of(&rows, &want) else {
            self.site.path = was;
            return Landing::Hidden;
        };
        if self.site.path != was || row != self.cursor {
            self.detail.rewind();
        }
        // 기억 자리는 **갈라지기 전까지만** 참이다. 그 밑은 들어간 적이 없는 층이라 0 —
        // `leave` 는 나온 디렉터리를 먼저 찾으므로 이 수는 못 찾을 때만 쓰인다.
        let same = was.iter().zip(&self.site.path).take_while(|(a, b)| a == b).count();
        self.site.remembered.truncate(same);
        self.site.remembered.resize(self.site.path.len(), 0);
        self.cursor = row;
        Landing::Shown
    }

    /// 그 줄의 정체. 지금 `issues` 에 대해 잰다.
    fn anchor_of(&self, row: &Row) -> Anchor {
        match row {
            Row::Up => Anchor::Up,
            Row::Item(seat, Entry::Dir { seg, at: None }, _) => {
                let at = match seat {
                    Seat::Here => None,
                    Seat::Place(n) => Some(self.place_path(*n).map(Into::into).unwrap_or_default()),
                };
                Anchor::Bucket(at, seg.clone())
            }
            // **그 줄의 프로젝트로 읽는다**(moai-1xo5) — 한눈 보기에서는 지금 선 프로젝트의 줄이
            // 비어 있어, 남의 첨자를 여기서 읽으면 그 자리에서 죽는다.
            Row::Item(seat, Entry::Dir { at: Some(at), .. } | Entry::Leaf { at }, _) => {
                let id = self.issue_at(*seat, *at).map(|i| i.id.clone()).unwrap_or_default();
                match seat {
                    Seat::Here => Anchor::Issue(id),
                    Seat::Place(n) => Anchor::Foreign(self.place_path(*n).map(Into::into).unwrap_or_default(), id),
                }
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
        for seg in self.site.path.clone() {
            let here = self.site.index.entries(&self.site.issues, &good);
            let ok = here.iter().any(|e| matches!(e, Entry::Dir { seg: s, .. } if *s == seg));
            if !ok {
                break;
            }
            good.push(seg);
        }
        if good.len() == self.site.path.len() {
            self.site.path = good;
            return;
        }
        // **옮겨진 것과 지워진 것은 다르다.** 서 있던 마디가 아직 살아 있으면
        // 자리만 바뀐 것이니 그 새 자리로 따라간다 — 마일스톤이 처음 생기면
        // 에픽이 한 층 깊어지는데, 거기서 뿌리로 내려놓으면 가장 흔한 갱신이
        // 하필 자리를 가장 크게 잃는 갱신이 된다. `home_of` 가 그 새 자리를
        // 이미 알고, `cmd/tui.rs::resolve` 도 같은 셈을 쓴다.
        if let Some(at) = self.site.path.last().and_then(|s| self.site.index.find(seg_id(s)?))
            && self.site.index.is_dir(&self.site.issues, at)
        {
            let mut moved = self.site.index.home_of(at).clone();
            moved.push(self.site.index.seg_of(&self.site.issues, at));
            if moved != self.site.path {
                self.site.remembered = vec![0; moved.len()];
                self.cursor = 0;
                self.site.path = moved;
                return;
            }
        }
        self.site.remembered.truncate(good.len());
        self.cursor = 0;
        self.site.path = good;
    }

    /// 파일이 우리가 읽은 뒤로 바뀌었으면 **저절로 다시 읽는다.**
    ///
    /// 한때 말만 하고 F5(뒤의 SPC r, 지금은 걷었다 — moai-en4u)를 기다렸다(moai-6qdx) — 읽으면 커서가 튀었기 때문이다.
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
    /// 공용 git 디렉터리의 `worktrees/` 표식으로 알아챈다(`worktree::heads`) — 걸음마다
    /// git 을 띄우는 것은 700ms 마다 프로세스 하나를 만드는 일이라, 파일만 잰다.
    ///
    /// **읽기는 스레드에서 한다**([`Fresh`]). 짓는 동안은 표식을 다시 재지 않는다 —
    /// 하나가 끝나기 전에 또 띄우면 몰아 쓰는 동안 스레드가 쌓인다. 끝난 것을 들인
    /// 뒤에도 파일이 또 바뀌었으면(표식은 읽기 전에 쟀다) 다음 걸음이 다시 띄운다.
    pub fn follow(&mut self) {
        self.reap();
        self.follow_config();
        self.follow_read();
        // 층은 제 표식을 따로 본다 — 층에 선 동안에는 아래(한 프로젝트)가 비어 할 일이 없다.
        self.follow_layer();
        // 펼친 프로젝트의 줄도 스레드에서 온다(moai-12yx) — 요약과 따로 돈다.
        self.follow_site();
        self.follow_commits();
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
                    self.trouble = Some(say(self.site.lang, "tui.reload.stopped").into());
                }
            }
            return;
        }
        let Some(repo) = &self.site.repo else { return };
        // **시계도 다시 읽을 까닭이다**(moai-z4r4) — 배너의 수에는 한 시간 틈과 날로 재는 경고가 들어
        // 파일이 그대로여도 답이 바뀐다. 층과 **같은 자**(`layer::due`)로 재므로, 안 읽으면 틈을 넘긴
        // 순간 층의 `!` 와 이 배너가 갈린다. 저장소가 서 있으면 읽은 때도 늘 서 있다 — 띄울 때의 첫
        // 읽기도 들인 읽기로 찍는다(`App::open`).
        let moved = layer::due(self.site.read_at)
            || stamp_of(repo) != self.site.stamp
            || self.site.watched.iter().any(|(path, was)| crate::store::stamp(path) != *was);
        if moved {
            let (tx, rx) = std::sync::mpsc::channel();
            let repo = repo.clone();
            let worktree = self.worktree;
            let read = self.read;
            let lang = self.site.lang;
            // 받는 쪽이 사라졌으면(사람이 누른 갱신이 버렸으면) 보내기가 실패한다 — 버린 것이라 그대로 둔다.
            let handle = std::thread::spawn(move || {
                let _ = tx.send(read(&repo, worktree, lang));
            });
            self.pending = Some((rx, handle));
        }
    }

    /// 새 커밋 표가 필요하면(`commits_due`) 짓는 스레드를 띄우고, 다 지었으면 받는다(moai-a4i0).
    ///
    /// **다시 읽기 손잡이(`pending`)와 따로 든다.** 그쪽에 태우면 연 직후 파일이 안 바뀌었는데도
    /// 저장소를 통째로 다시 세고, `loading` 이 참이 되어 안 바뀐 화면을 읽는 중이라 말한다.
    /// 다시 읽기([`prepare`]) 안에서 짓지도 않는다 — 그 읽기는 쓰기마다 루프에서 돈다.
    /// 스레드가 죽으면 다시 읽기와 같게 패닉을 되던진다 — 터미널은 이미 걷혔다.
    ///
    /// 도는 동안 다시 읽기가 또 들어와도 버리고 새로 띄우지 않는다 — 버린 손잡이가 쌓이고 git 이
    /// 겹쳐 돈다. 받은 답은 든 표보다는 새것이라 들이고, 필요하면 곧바로 하나를 더 띄운다.
    fn follow_commits(&mut self) {
        if let Some((rx, _)) = &self.commits_job {
            match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Ok(table) => {
                    self.commits_job = None;
                    // **뿌리마다 덮는다.** `commit_tables` 는 이번에 git 이 답을 안 준 뿌리를
                    // 통째로 빼고 오므로(`.ok()`), 받은 것을 그대로 넣으면 한 번 어긋난 걸음에
                    // 옛 표가 사라져 커밋 칸이 말없이 빈다 — 다음 표는 표식이 움직여야 온다.
                    self.site.commits.extend(table);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some((_, handle)) = self.commits_job.take()
                        && let Err(payload) = handle.join()
                    {
                        std::panic::resume_unwind(payload);
                    }
                    // 패닉 없이 사라졌다. 다시 읽기는 여기서 다음 걸음에 또 띄우므로
                    // (`follow` 의 `moved`) 표도 같게 다시 세운다 — 안 세우면 그 세션 내내 언다.
                    self.commits_due = true;
                }
            }
        }
        if !self.commits_due {
            return;
        }
        let Some(repo) = &self.site.repo else { return };
        self.commits_due = false;
        let roots = commit_roots(repo, &self.site.origin);
        // **스레드로 넘길 것은 값이다** — 빌린 `issues` 를 넘기면 그 스레드가 도는 동안 다시 읽기가
        // 목록을 갈아 끼울 수 없다. 넘긴 것을 그대로 들어 둔다(`commit_ids`): 다음 읽기가 그것과
        // 견줘 표가 모르는 id 를 붙잡는다.
        self.site.commit_ids = self.site.issues.iter().map(|i| i.id.clone()).collect();
        let ids = self.site.commit_ids.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(commit_tables(&roots, &ids));
        });
        self.commits_job = Some((rx, handle));
    }

    /// 커밋 표를 짓는 스레드가 돌거나 띄울 참인가. 루프가 이 동안은 빠른 걸음으로 깨어 받는다 —
    /// 느린 걸음이면 연 뒤·다시 읽은 뒤 한동안 커밋 칸이 빈다(낡는다). [`App::loading`] 과 가르는
    /// 까닭은 [`App::follow_commits`].
    pub fn gathering_commits(&self) -> bool {
        self.commits_job.is_some() || (self.commits_due && self.site.repo.is_some())
    }

    /// 들고 있는 `filter_text` 를 지금 `issues` 에 다시 건다. 못 걸면 푼다.
    fn reapply(&mut self) {
        let mode = match (self.grep_query(), &self.filter_text) {
            (Some((g, q)), _) => Mode::Grep(Input::new(q), g),
            (None, Some(t)) => Mode::Filter(Input::new(t)),
            (None, None) => return self.clear_filter(),
        };
        if self.apply(&mode).is_err() {
            self.clear_filter();
        }
    }

    /// 걸린 검색의 범위와 친 글. 거름망(`f`)이거나 걸린 것이 없으면 `None`.
    ///
    /// 뱃지 글은 `/<글>` 이거나 범위를 좁혔으면 `/<범위>:<글>` 이다([`App::apply`]). 범위는
    /// 글에서 되읽지 않고 [`App::grep_in`] 을 믿는다 — `/id:x` 를 전체 범위로 친 사람도 있다.
    pub fn grep_query(&self) -> Option<(GrepIn, &str)> {
        let q = self.filter_text.as_deref()?.strip_prefix('/')?;
        let q = match self.grep_in {
            GrepIn::All => q,
            g => q.strip_prefix(g.name()).and_then(|q| q.strip_prefix(':')).unwrap_or(q),
        };
        Some((self.grep_in, q))
    }

    /// 걸린 거름망에 **제 줄이 걸린** 이슈 수. 걸린 것을 품어 남은 디렉터리는 안 센다.
    /// 보기(`SPC v`)가 숨긴 줄도 안 센다 — 세어 놓고 목록에 없으면 셈이 거짓말이 된다. 검색이 걸린 동안은
    /// 보기가 안 가리므로 목록에 선 숨은 줄도 센다([`Self::unveiled_count`] 가 그 가운데 몇인지 댄다).
    pub fn hit_count(&self) -> usize {
        (0..self.site.keep.len()).filter(|&at| self.visible(at)).count()
    }

    /// 검색에 걸렸는데 **검색을 풀면 보기(`SPC v`)가 도로 가릴** 이슈 수(moai-qnkn). 검색 칸이 `N건` 곁에
    /// 댄다 — 흐린 줄이 왜 섰는지, 검색을 풀면 몇 줄이 도로 숨는지를 글로 말한다. **목록이 `숨김` 을 다는
    /// 자([`Self::unveiled`])와 같다** — 제 줄은 숨었어도 밑에 보기가 보이는 줄이 있는 폴더는 풀어도 서므로
    /// 안 센다. 제 줄로만 세면 셈은 `숨김 1건` 인데 목록 어디에도 `숨김` 이 없다.
    pub fn unveiled_count(&self) -> usize {
        if !self.searching() {
            return 0;
        }
        (0..self.site.keep.len()).filter(|&at| !self.site.veil(at).filtered && !self.site.stands_in_view(at)).count()
    }

    /// 이 줄이 목록에 서는가 — 거름망이 안 가렸고, 보기가 안 가렸거나 검색이 보기를 걷었다([`Self::veil`]).
    fn visible(&self, at: usize) -> bool {
        self.site.visible(at, self.searching())
    }

    /// 걸린 것이 검색(`/`)인가. **검색이 걸린 동안은 보기가 줄을 안 가린다**(moai-qnkn, 사용자 결정) —
    /// 끝난 일·미룬 일을 찾으려고 보기를 풀었다 되돌리는 것은 검색의 일이 아니다. 거름망(`f`)은 보기를
    /// 따른다: 그것은 "무엇을 볼지" 를 좁히는 물음이지 찾는 물음이 아니다. 칸을 Enter 로 닫아도 검색이
    /// 걸려 있는 한 그대로고, 풀면(Esc) 설정된 보기로 돌아간다 — **보기는 건드리지도 적지도 않는다.**
    pub fn searching(&self) -> bool {
        self.grep_query().is_some()
    }

    /// 이 줄이 **검색 덕에 선 숨은 줄**인가(moai-4x87) — 검색을 풀면 보기가 도로 가릴 줄. 목록이
    /// 흐리게 그리고 `숨김` 을 단다. 폴더는 목록이 남기는 자와 같다: 제 줄이 숨었어도 밑에 보기가
    /// 보이는 줄이 있으면 평소에도 서므로 숨은 줄이 아니다([`Self::stands_in_view`]). 바구니는 제 줄이 없어
    /// 안 단다.
    pub fn unveiled(&self, e: &Entry) -> bool {
        self.searching() && e.at().is_some_and(|at| !self.site.stands_in_view(at))
    }

    /// 목록에서 그 정체의 줄 자리. 커서를 붙드는 곳(다시 읽기·보기 토글·쓰기·층)이 같은 자로 찾는다.
    fn row_of(&self, rows: &[Row], want: &Anchor) -> Option<usize> {
        rows.iter().position(|r| self.anchor_of(r) == *want)
    }

    /// 지금 디렉터리에 **보기만 가린 줄**이 있는가 — 거름망은 지나는데 보기가 숨긴 것(moai-2kyl 단계 리뷰).
    /// 목록이 비었을 때 까닭을 대려고 묻는다. 이슈 수에 비례한 훑기라 줄이 있을 때는 안 부른다.
    pub fn view_hides_here(&self) -> bool {
        !self.on_layer()
            && !self
                .site
                .index
                .entries_where(&self.site.issues, &self.site.path, &|at| !self.site.veil(at).filtered)
                .is_empty()
    }

    /// 거름망을 건다. 빈 글은 "거름망 없음" 이다.
    ///
    /// **`all` 을 켠다.** `Filter` 의 기본값은 done 을 숨기는데, 탐색기가
    /// 시키지도 않은 줄을 숨기면 파일이 사라진 것처럼 보인다. 열린 것만 보려면
    /// `status=todo` 라고 적으면 된다.
    pub fn apply(&mut self, mode: &Mode) -> Result<(), String> {
        let text = match mode {
            Mode::Grep(q, _) | Mode::Filter(q) => q.text().to_string(),
            Mode::Browse | Mode::Ask(_) | Mode::Idea(_) | Mode::Pick(_) | Mode::Unregister(_) => String::new(),
        };
        if text.trim().is_empty() {
            self.filter_text = None;
            self.site.keep = vec![true; self.site.issues.len()];
            return Ok(());
        }
        let filter = self.build_filter(mode)?;
        // 칸 이름은 `Filter::build` 가 모른다 — 저장소가 정하는 것이라
        // `config` 에 있다. `cmd/show.rs` 와 같은 자로 잰다: 조용히 0건을 내면
        // `status=in-progress` 같은 오타가 "그 칸은 비었다" 와 구별되지 않는다.
        // 어느 줄이 선 칸이면 받는다 — `show -s`·`--from` 과 같은 술어다(moai-hym7).
        for s in &filter.status {
            if !crate::report::knows_column(&self.site.issues, &self.site.cfg, s) {
                return Err(crate::cmd::unknown_column(s, &self.site.cfg));
            }
        }
        // 시계는 **적재마다** 고정한 것을 쓴다. 여기서 다시 잡으면 `stale=`
        // 같은 물음이 화면의 나머지와 다른 시각으로 판정된다.
        let now = self.site.now.clone();
        // **적재 때 잰 것을 빌린다**(moai-fbdg) — 여기서 다시 재면 키 하나마다 소속 지도가 다시 선다.
        let wh = self.site.ground.here();
        self.site.keep = self.site.issues.iter().map(|i| filter.matches(i, &now, &wh)).collect();
        self.filter_text = Some(match mode {
            Mode::Grep(_, GrepIn::All) => format!("/{text}"),
            Mode::Grep(_, g) => format!("/{}:{text}", g.name()),
            _ => text,
        });
        if let Mode::Grep(_, g) = mode {
            self.grep_in = *g;
        }
        Ok(())
    }

    /// 적은 글을 거름망으로. **적는 곳과 물어보는 곳이 같은 것을 쓴다** —
    /// 갈라지면 프롬프트 밑의 오류가 Enter 가 판정할 글과 다른 글을 판정한다.
    fn build_filter(&self, mode: &Mode) -> Result<Filter, String> {
        let raw = match mode {
            Mode::Grep(q, g) => Raw { grep: Some(q.text().to_string()), grep_in: *g, all: true, ..Raw::default() },
            Mode::Filter(q) => Raw { filter: split_filter(q.text()), all: true, ideas: true, ..Raw::default() },
            Mode::Browse | Mode::Ask(_) | Mode::Idea(_) | Mode::Pick(_) | Mode::Unregister(_) => Raw::default(),
        };
        // **`Filter::build` 를 지난다.** 소문자 접기·태그 정규화·`항목=값` 해석이
        // 전부 거기 있고, 건너뛰면 CLI 와 TUI 가 같은 글을 다르게 읽는다.
        Filter::build(raw)
    }

    pub fn clear_filter(&mut self) {
        self.filter_text = None;
        self.site.keep = vec![true; self.site.issues.len()];
    }

    /// 키 표의 차례(조각)를 `query` 의 차례로 잇는다. 둘을 한 타입으로 두지 않는 까닭은 키 표가
    /// 조각이라 `crate::query` 를 못 부르기 때문이다(`input::tests::components_know_neither…`).
    fn sort_key(o: keys::Order) -> crate::query::SortKey {
        use crate::query::SortKey;
        match o {
            keys::Order::Priority => SortKey::Priority,
            keys::Order::Created => SortKey::Created,
            keys::Order::Updated => SortKey::Updated,
            keys::Order::Column => SortKey::Status,
            keys::Order::Assignee => SortKey::Assignee,
            keys::Order::Title => SortKey::Title,
        }
    }

    /// 줄마다 보기에 보이는지 다시 센다. 칸은 **목록의 글리프와 같은 자**([`App::column`])로,
    /// 미룸은 물려받은 것까지(`Index::deferred_root`) 읽는다 — 미룬 에픽 밑의 일도 같이 빠진다.
    /// 칸 숨김은 **이 프로젝트의 칸에만** 건다(`View::shows`).
    fn see(&mut self) {
        let view = self.view.clone();
        Self::see_in(&mut self.site, &view);
        // **한눈 보기의 남의 줄에도 같은 보기를 건다**(moai-1xo5, 사용자 결정 2026-09-19) — 보기는
        // 보는 사람의 것이라 화면에 하나뿐이다. 안 걸면 `SPC v d` 가 지금 선 프로젝트에만 들어,
        // 같은 화면의 두 프로젝트가 한 토글에 다르게 선다.
        let places = self.layer.as_mut().map_or(0, |l| l.places.len());
        for n in 0..places {
            if let Some(site) = self.layer.as_mut().and_then(|l| l.places.get_mut(n)).and_then(|p| p.site.as_mut()) {
                Self::see_in(site, &view);
            }
        }
    }

    /// 한 프로젝트에 보기를 건다 — `shown` 과 그 줄들이 사는 자리 전부(`lit`).
    ///
    /// **같은 보기를 같은 줄에 다시 걸지 않는다**(moai-m59y, `Site::seen_view`) — 지금 선 프로젝트를
    /// 다시 읽을 때마다 [`App::see`] 가 든 프로젝트 전부를 돌아, 옆 프로젝트들은 줄도 보기도 안
    /// 바뀐 채로 이슈 수 × 자리 깊이를 다시 셌다.
    fn see_in(site: &mut Site, view: &view::View) {
        if site.shown.len() == site.issues.len() && site.seen_view.as_ref() == Some(view) {
            return;
        }
        site.shown = (0..site.issues.len())
            .map(|at| {
                view.shows(site.column(at), site.index.deferred_root(&site.issues[at].id).is_some(), &site.cfg.statuses)
            })
            .collect();
        let mut lit = std::collections::HashSet::new();
        for (at, _) in site.shown.iter().enumerate().filter(|(_, on)| **on) {
            let home = site.index.home_of(at);
            for n in 1..=home.len() {
                if !lit.contains(&home[..n]) {
                    lit.insert(home[..n].to_vec());
                }
            }
        }
        site.lit = lit;
        site.seen_view = Some(view.clone());
    }

    /// 설정 자리(`App::user_config`)에서 보기를 읽어 [`App::adopt_look`] 에 넘긴다. **시험만 부른다**(moai-u8cs) —
    /// 띄우는 길(`cmd::tui`)은 층과 한 번 읽은 설정을 나눠 `adopt_look` 을 바로 부른다.
    #[cfg(test)]
    pub fn load_look(&mut self) {
        let (look, problems) = crate::user_config::read_look(self.user_config.as_deref());
        self.adopt_look(&look, problems);
    }

    /// 사용자 설정에 적어 둔 보기를 입힌다(moai-2bzp) — 칸 숨김·미룸·정렬·열. 없는 키는 처음값
    /// 그대로다. **읽기는 관대하다**: 모르는 낱말·틀린 키는 한 줄 알림으로 대고 나머지를 입힌다 —
    /// 틀린 키 하나로 탐색기가 안 뜨면 설정이 도구를 막는다. `problems` 는 설정을 읽다 만난 까닭이다.
    /// **파일은 안 읽는다**(moai-u8cs) — 띄우는 길(`cmd::tui`)이 층과 한 번 읽은 설정을 나눠 준다.
    ///
    /// 입힌 뒤의 보기를 `App::saved` 로 든다 — 모르는 낱말·틀린 값은 화면의 보기에 없으니, 이 세션이
    /// 그 키를 안 바꾸는 한 적을 때 파일의 것이 그대로 남는다(`Doc::merge_look`).
    pub fn adopt_look(&mut self, look: &crate::user_config::Look, mut problems: Vec<String>) {
        self.apply_look(look, &mut problems);
        self.saved = self.look_now();
        // **열만은 파일이 적은 것을 그대로 든다**(moai-6bc0 단계 리뷰). 다른 키는 입힌 결과가 곧 파일의
        // 값이지만 `fields` 는 아니다 — 파일이 몰랐던 열은 기본값으로 서므로 켜진 채인데 파일에는 없다.
        // 그것을 "이미 적혀 있다" 로 들면 그 열 이름은 영영 파일에 안 적히고, `fields_known` 이 적히는
        // 순간 다음 실행이 그 빈자리를 "사람이 껐다" 로 읽어 켜 둔 열이 꺼진다.
        //
        // **모르는 낱말은 걸러 둔다** — 새 바이너리가 적은 낱말까지 base 로 들면 이쪽 토글 한 번이
        // 그것을 지운다(`merge_words` 가 base 에 있고 new 에 없는 낱말을 뺀다).
        if let Some(words) = &look.fields {
            self.saved.fields = Some(words.iter().filter(|w| view::Field::named(w).is_some()).cloned().collect());
        }
        self.saved.fields_known = look.fields_known.clone();
        if !problems.is_empty() {
            self.notice = Some(fill(say(self.site.lang, "tui.look.problems"), &[("why", &problems.join(" · "))]));
        }
        self.see();
    }

    fn apply_look(&mut self, look: &crate::user_config::Look, problems: &mut Vec<String>) {
        if let Some(hidden) = &look.hidden {
            // **겹쳐 적힌 이름은 하나로 든다**(moai-2kyl 단계 리뷰) — 토글(`View::toggle`)은 한 번에 하나를
            // 빼, 겹친 채 들면 한 번 눌러서는 안 보인다.
            self.view.hidden = Vec::new();
            for h in hidden {
                if !self.view.hides(h) {
                    self.view.hidden.push(h.clone());
                }
            }
        }
        if let Some(d) = look.hide_deferred {
            self.view.hide_deferred = d;
        }
        // **차례와 방향은 한 벌이다**(moai-2kyl 단계 리뷰) — 모르는 차례의 방향을 처음 차례(우선순위)에
        // 입히면 아무도 안 고른 거꾸로가 선다. 모르는 차례면 방향도 두고, 파일의 둘은 그대로 남는다.
        let sort_known = match look.sort.as_deref() {
            None => true,
            Some(s) => match keys::Order::named(s) {
                Some(o) => {
                    self.order.by = o;
                    true
                }
                None => {
                    problems.push(fill(
                        say(self.site.lang, "tui.look.bad_sort"),
                        &[("word", s), ("known", &keys::Order::ALL.map(keys::Order::name).join("·"))],
                    ));
                    false
                }
            },
        };
        if sort_known && let Some(r) = look.sort_reversed {
            self.order.reversed = r;
        }
        if let Some(words) = &look.fields {
            // **적은 쪽이 알던 열만 그대로 따른다**(사용자 결정 2026-09-15) — `fields` 에 안 적힌 것이
            // "껐다" 인지 "그 열을 몰랐다" 인지 가르는 것이 `fields_known` 이다. 그 목록에 없는 열은
            // 여기 기본값으로 선다: 옛 설정을 가진 사람에게도 새 열이 뜨고, 끈 열은 끈 채로 남는다.
            let mut fields = view::Fields::default();
            for f in view::Field::ALL {
                let on = words.iter().any(|w| w == f.name());
                // **`fields` 에 적힌 열은 적은 쪽이 알던 열이다**(moai-svvk 에픽 리뷰). `fields_known` 은
                // *안 적힌* 열을 가르는 자이지 적힌 열을 거르는 자가 아니다 — 거르면 `fields = ["tags"]`,
                // `fields_known = ["id"]` 같은 손 설정에서 tags 가 기본값(꺼짐)으로 서 말없이 버려지고,
                // 손으로 적은 빈 목록을 옛 바이너리가 제 이름 목록으로 한 번 바꿔 적는 순간 새 바이너리의
                // 열이 켜진 채 적혀 있어도 꺼진다.
                let knew = on
                    || match &look.fields_known {
                        // **빈 목록은 "모든 열을 알았다" 다**(moai-4qkj, 사용자 결정 2026-09-18). 이 바이너리는
                        // 늘 열 이름을 다 적으니(`look_now`) 빈 목록은 손으로 적은 것이고, 그 사람이 적은
                        // `fields` 가 곧 켠 열이다. "아무 열도 몰랐다" 로 읽으면 안 적힌 열이 모두 기본값으로
                        // 서 적힌 `fields` 가 곧 켠 열이라는 뜻이 한마디 없이 버려진다.
                        //
                        // **그 "모든" 은 얼린 아홉 열이다**(moai-4gy5) — `Field::ALL` 이 아니다. 까닭은
                        // 그 상수의 글에 있다(`view::Field::EMPTY_KNOWN`).
                        Some(known) if known.is_empty() => view::Field::EMPTY_KNOWN.contains(&f),
                        Some(known) => known.iter().any(|w| w == f.name()),
                        // 이 키가 없던 때의 어휘 — 그때 있던 열이면 안 적힌 것이 곧 "껐다" 다.
                        None => view::Field::BEFORE_KNOWN.contains(&f),
                    };
                if knew {
                    fields.set(f, on);
                }
            }
            // **새 바이너리가 적은 이름은 오타가 아니다**(moai-zwam) — 같은 설정 파일을 옛 것과 새 것이
            // 번갈아 만지면, 옛 쪽은 제가 모른다는 이유로 그 이름을 오타로 읽어 띄울 때마다 잔소리를
            // 했다. 고칠 길도 없다: 쓰기 경로는 그 이름을 **일부러 지키고**(`App::adopt_look` 이
            // `saved.fields` 에서 걸러 `Doc::merge_look` 이 안 빼게 한다) 읽는 쪽만 오타로 봤다.
            //
            // 가르는 자는 `fields_known` 이다 — 거기 적혔다는 것은 적은 쪽이 그 열을 알았다는 뜻이다.
            // 목록에 없는 이름은 그대로 댄다: 손으로 낸 오타가 그것이고, 그때 이 한 줄이 유일한 말이다.
            //
            // **위의 `knew` 와 다른 물음이다** — 저기는 *안 적힌* 열을 가르느라 빈 목록과 없는 목록에
            // 저마다 어휘를 대는데(`EMPTY_KNOWN`·`BEFORE_KNOWN`), 여기는 *적힌* 이름이 그 목록에 들었나
            // 하나다. 둘 다 얼린 어휘가 이 바이너리의 `Field::ALL` 안에 있어, 그 밖의 이름은 어느 쪽으로
            // 읽어도 오타다 — 이름을 갈라 그 둘이 한 물음으로 안 보이게 둔다.
            let wrote = look.fields_known.as_deref().unwrap_or_default();
            for w in words {
                if view::Field::named(w).is_none() && !wrote.contains(w) {
                    problems.push(fill(
                        say(self.site.lang, "tui.look.bad_field"),
                        &[("word", w), ("known", &view::Field::ALL.map(view::Field::name).join("·"))],
                    ));
                }
            }
            self.fields = fields;
        }
        if let Some(open) = look.detail {
            self.detail_open = open;
            if !open {
                self.focus = Pane::Explorer;
            }
        }
    }

    /// 지금 보기를 설정에 적을 모양으로.
    fn look_now(&self) -> crate::user_config::Look {
        crate::user_config::Look {
            hidden: Some(self.view.hidden.clone()),
            hide_deferred: Some(self.view.hide_deferred),
            sort: Some(self.order.by.name().to_string()),
            sort_reversed: Some(self.order.reversed),
            fields: Some(
                view::Field::ALL.into_iter().filter(|f| self.fields.shows(*f)).map(|f| f.name().to_string()).collect(),
            ),
            // 이 바이너리가 아는 열 전부 — 다음에 읽는 쪽이 "안 적힌 것" 을 가를 자다.
            fields_known: Some(view::Field::ALL.into_iter().map(|f| f.name().to_string()).collect()),
            detail: Some(self.detail_open),
        }
    }

    /// 지금 보기를 사용자 설정에 적는다 — **토글마다**. 끝낼 때 한 번 적으면 Ctrl-C·터미널이 닫힌
    /// 때 잃는다. **이 세션이 바꾼 만큼만 옮긴다**(`Doc::merge_look`, moai-2kyl 단계 리뷰) — 옆 탐색기가
    /// 켠 열이나 손으로 고친 값을 내 토글 한 번이 되돌리지 않는다. 바뀐 것이 없으면 파일을 열지도 않는다.
    /// 설정 자리가 없으면(시험·설정 없는 기계) 적지 않는다. **못 적어도 화면은 바뀐 대로다** — 알림 한
    /// 줄로 대고, 다음 토글이 쌓인 차이를 다시 옮긴다.
    fn save_look(&mut self) {
        let Some(path) = self.user_config.clone() else { return };
        let look = self.look_now();
        if look == self.saved {
            return;
        }
        match crate::user_config::update(&path, |doc| doc.merge_look(&self.saved, &look)) {
            // 건너뛴 키도 든 것으로 옮긴다(moai-jr3z) — 안 옮기면 다음 저장마다 그 차이가 또 실려 같은 알림이
            // 토글마다 선다. 알림은 이번 한 번이고, 파일의 손으로 적은 모양은 그대로다.
            Ok(skipped) => {
                self.saved = look;
                if !skipped.is_empty() {
                    self.notice = Some(fill(say(self.site.lang, "tui.look.skipped"), &[("why", &skipped.join(" · "))]));
                }
            }
            Err(e) => {
                let why = crate::text::one_line(&e.to_string());
                self.notice = Some(fill(say(self.site.lang, "tui.look.unsaved"), &[("why", &why)]));
            }
        }
    }

    /// 읽음 파일을 고르는 뿌리 — **트래커가 사는 곳**(`Repo::root`)이다.
    ///
    /// **`App::here()` 가 아니다**(리뷰). 그쪽은 이 세션이 **선 체크아웃**이라, 딸린 워크트리에서는
    /// 트래커의 자리와 갈린다(`Repo::here`). 읽음의 키는 이슈 id 고 그 id 는 트래커에서 오므로, 자리는
    /// 트래커를 따라야 한다 — 쓰는 자([`App::mark_read`]·`cmd::read`·[`App::follow_site`])는 처음부터
    /// `root` 로 골랐는데 읽는 자만 `here()` 로 골라, 워크트리에서 띄운 탐색기가 제가 적은 파일을 다시
    /// 못 읽었다. `r` 을 누르면 [NEW] 가 내렸다가 다음 걸음에 도로 섰다.
    fn read_root(&self) -> Option<std::path::PathBuf> {
        self.site.repo.as_ref().map(|r| r.root.clone())
    }

    /// 그 프로젝트의 읽음을 **제 파일에서** 든다(moai-bwce) — 옛 `[read]` 를 겹쳐 보고, 표식을 읽기
    /// **전에** 잰다(뒤에 재면 읽고 다음 걸음 사이에 옆이 쓴 것을 놓친다). 설정 자리를 모르면(시험·설정
    /// 없는 기계) `None` 이고 부르는 쪽은 들고 있던 것을 둔다.
    /// [`App::read_marks_of`] 가 한 걸음에 들어 온 것 — 읽음과 그 파일의 표식, 그리고 **자리를 고르다
    /// 만난 까닭**(moai-hzfu). 셋을 튜플로 들던 판은 셋째가 붙으면서 부르는 쪽마다 자리를 세게 됐다.
    fn read_marks_of(&self, root: &std::path::Path) -> Option<Got> {
        let config = self.user_config.as_deref()?;
        // **자리를 한 번만 고른다** — 표식이 쓰는 파일과 아래에서 가려낼 자리 까닭이 한 값에서 온다.
        // `path_for` 로 파일만 받던 판은 같은 `settle` 을 두 번 돌면서도 그 까닭을 버렸다.
        let place = crate::read_marks::place_of(config, root);
        // **읽기가 여는 파일 전부를 잰다**([`ReadStamp`], moai-65as) — 아래 `read` 는 대기 자리와
        // (지금 자리가 없으면) 옛 자리까지 연다. 지금 자리 하나만 재던 판은 그 둘이 바뀌어도
        // [`App::follow_read`] 가 못 알아채, 합쳐질 줄이 내내 [NEW] 로 섰다.
        let stamp = ReadStamp::of(&place);
        let mut marks = crate::read_marks::read(config, root, &self.legacy_read);
        // **자리 탈을 줄 탈에서 가려낸다**(moai-hzfu). `Marks::problems` 는 두 갈래를 한 자루에
        // 담는다 — 자리를 고르다 만난 까닭(`place_of`)과 파일 안의 건너뛴 줄이다. `read` 는 자리
        // 까닭을 뒤에 붙여 `first()` 가 진짜 까닭을 먼저 보게 하지만, 건너뛴 줄이 하나도 없으면
        // 그 자리 까닭이 곧 `first()` 라 "읽음에 이상한 줄이 있다" 로 이름 붙었다. `r` 이 띄운
        // "자리를 못 풀어…" 한 줄이 다음 걸음에 그 이름으로 덮이던 자리다.
        //
        // **가르는 자리가 여기인 것은 임시다.** 제 집은 `Marks` 지만(그러면 CLI 도 함께 받는다)
        // 그 모듈은 지금 옆 워크트리가 쥐었다 — 탐색기 쪽에서만 갈라 두고, 옮길 때 이 블록이
        // 통째로 걷힌다. `cmd/read.rs` 는 둘 다 stderr 로 내므로 이름이 안 갈려도 틀리지 않는다.
        //
        // **값으로 뺀다.** 같은 `place_of` 가 낸 같은 글이라 맞는다. 그 사이에 자리가 바뀌어 글이
        // 달라지면 못 빼는데, 그때 최악이 지금까지의 이름이다 — 덜 맞는 이름이지 새 탈이 아니다.
        let mut where_why = None;
        for why in &place.problems {
            if let Some(at) = marks.problems.iter().position(|p| p == why) {
                where_why = Some(marks.problems.remove(at));
            }
        }
        // **설정이 사라졌으면 없는 읽음도 사라진 것으로 든다**(moai-4qbv.i0g 리뷰). 읽음 파일은 설정
        // 파일 곁의 디렉터리에 산다(`read_marks::path_for`) — autofs·sshfs 홈이 끊기면 둘이 함께
        // 사라진다. 그때 없는 읽음을 "아직 아무것도 안 읽은 프로젝트" 로 들면 빈 표를 들여 내 줄이
        // 통째로 [NEW] 로 서고, `SPC m a` 한 번이 그 프로젝트를 통째로 읽음으로 찍는다 — 층만 지키고
        // 읽음은 놓친 자리다(moai-po6v 가 설정에 대해 막은 그 해다).
        //
        // **읽음 쪽에서 혼자 가르지 않는다** — 없는 읽음은 그 프로젝트를 아직 안 읽은 사람의 정상이라,
        // 그것만 보고는 갈릴 수 없다. 가르는 자는 **곁의 설정도 사라졌는가** 다. 읽어 낸 줄이 있으면
        // 딴 자리에 살아 있다는 뜻이니 건드리지 않는다.
        if marks.trouble.is_none()
            && marks.seen.is_empty()
            && self.config_tried.trouble == Some(crate::user_config::Trouble::Gone)
        {
            marks.trouble = Some(crate::user_config::Trouble::Gone);
        }
        Some(Got { marks, stamp, where_why })
    }

    /// 들어 온 읽음을 한 [`Site`] 에 얹는다 — **들일지, 표식을 올릴지, 무엇을 말할지가 한 자리에 있다**
    /// (리뷰). [`App::load_read`] 와 [`App::follow_site`] 가 저마다 적던 판은 둘이 이미 갈렸다.
    ///
    /// 돌려주는 것은 화면에 댈 한 줄이다(없으면 `None`) — `notice` 는 `App` 의 것이라 여기서 못 적는다.
    fn take_read(site: &mut Site, got: Got) -> Option<String> {
        let Got { marks, stamp, where_why } = got;
        // **다시 읽을 때는 갈래가 정한다**(moai-po6v) — [`App::follow_config`] 와 **한 자다**
        // ([`crate::user_config::Again`]). 잠깐인 것만 표식을 안 올려 다음 걸음이 같은 차이를 다시 보게
        // 한다. 권한은 다시 해도 같아 걸음마다 읽으면 헛돌지만, 되돌리는 `chmod` 이 고친 때도 길이도
        // 안 바꿔 표식으로는 영영 못 벗어난다 — 그래서 표식은 올리고 시계로 다시 본다
        // ([`App::follow_read`]). 깨진 것·남의 것은 사람이 고쳐야 같아지고 고치면 파일이 바뀐다.
        site.read_tried.saw(marks.trouble);
        if marks.trouble.map(crate::user_config::Trouble::again) != Some(crate::user_config::Again::Step) {
            site.read_stamp = Some(stamp);
        }
        // **못 든 것과 한 줄 건너뛴 것을 가른다**(리뷰). 못 들었으면 들고 있던 것을 둔다 — 빈 표를
        // 들이면 내 줄이 통째로 [NEW] 로 선다. 건너뛴 줄 하나는 나머지를 버릴 까닭이 아니다: 버리던
        // 판은 손으로 적은 맨 점 키 하나가 그 프로젝트의 [NEW] 를 영영 세워 두었고, 같은 파일을 읽는
        // `moai read` 는 그 표를 그대로 썼다.
        //
        // **잠깐 못 읽은 것은 말하지 않는다** — 다음 걸음에 다시 읽으므로, 말하면 걸음마다 같은 줄이
        // 서서 사람이 방금 띄운 말을 덮는다(`follow_config` 도 그 갈래에서는 조용히 다시 읽는다).
        let told = match marks.trouble {
            // **못 읽은 것은 말하지 않는다** — 잠깐인 것은 다음 걸음에 지나가고, 다시 해도 같은 것은
            // 시계가 돌 때마다 같은 줄을 세워 사람이 방금 띄운 말을 덮는다. 설정은 배너가 늘 이고
            // 있지만(`App::held`·`Layer::problems`) 여기 낼 것은 스치는 알림 한 줄뿐이다.
            Some(t) if t.again() != crate::user_config::Again::Never => None,
            Some(_) => marks
                .problems
                .first()
                .map(|why| fill(say(site.lang, "tui.read.unheld"), &[("why", &crate::text::one_line(why))])),
            // **줄 탈이 자리 탈보다 앞선다**(moai-hzfu) — 건너뛴 줄은 이 파일을 정말 읽고 만난
            // 것이라 사람이 고칠 자리가 또렷하다. 자리 탈은 그것이 없을 때만 대고, **제 낱말로**
            // 댄다: 같은 까닭을 두 이름으로 부르면 `r` 이 띄운 줄과 다음 걸음의 줄이 갈린다.
            None => marks
                .problems
                .first()
                .map(|why| fill(say(site.lang, "tui.read.bad_line"), &[("why", &crate::text::one_line(why))]))
                .or_else(|| {
                    where_why
                        .as_ref()
                        .map(|why| fill(say(site.lang, "tui.read.bad_place"), &[("why", &crate::text::one_line(why))]))
                }),
        };
        if marks.trouble.is_none() {
            site.seen = marks.seen;
        }
        told
    }

    /// 지금 선 프로젝트의 읽음을 파일에서 들어 [NEW] 를 다시 센다. 띄울 때(`cmd::tui`), 프로젝트에 들어갈
    /// 때(`App::enter_project`), 그 파일이 바뀐 걸음([`App::follow_read`])이 부른다.
    pub fn load_read(&mut self) {
        let Some(root) = self.read_root() else { return };
        let Some(got) = self.read_marks_of(&root) else { return };
        // 못 들었으면 표가 그대로라 다시 셀 까닭이 없다 — 잠깐 못 읽는 동안은 이 길이 걸음마다 돈다.
        let took = got.marks.trouble.is_none();
        // **몰라진 그 한 번은 다시 센다**(moai-2gep) — 들고 있는 표가 없으면 [NEW] 를 안 세는데
        // (`App::recount_unread_in`), 그 답은 이미 세 놓은 줄을 지워야 선다. 몰랐다가 또 모르는
        // 걸음은 답이 같으니 안 센다 — 그 길은 시계가 돌 때마다 오고, 세는 값이 줄 수만큼이다.
        let knew = self.site.read_tried.trouble.is_none();
        // 까닭은 한 줄로 댄다 — 조용히 [NEW] 가 서면 왜 그런지 볼 데가 없다.
        if let Some(told) = Self::take_read(&mut self.site, got) {
            self.notice = Some(told);
        }
        if took || knew {
            self.recount_unread();
        }
    }

    /// **읽음 파일이 바뀌었으면 다시 든다** — 옆 터미널의 `moai read` 가 이 화면에 닿는 길이다
    /// (moai-j038.vna). 한때 그 길은 설정 파일의 표식이었는데, 읽음이 설정 밖으로 나가며 설정은 더 안
    /// 바뀐다(moai-bwce) — 그래서 그 파일의 표식을 따로 잰다.
    ///
    /// 처음 보는 프로젝트(표식이 없다)면 그때 든다 — 프로젝트를 옮기면 그 자리의 읽음이 그 걸음에 선다.
    ///
    /// **걸음마다 도는 자리다**(리뷰) — 목록을 굴리는 동안 빠른 걸음으로 돌므로, 빌려 쓰고 `stat` 은
    /// 한 번만 한다([`App::follow_config`] 와 같은 모양).
    fn follow_read(&mut self) {
        let (Some(config), Some(repo)) = (self.user_config.as_deref(), self.site.repo.as_ref()) else { return };
        // **재는 자리는 [`ReadStamp::of`] 가 정한다**(moai-65as) — 여기서 파일을 따로 고르면
        // [`App::read_marks_of`] 와 갈려, 걸음이 제가 이미 읽은 것을 다시 읽거나 영영 안 읽는다.
        // `path_for` 하나를 재던 판이 그렇게 갈렸다.
        let now = ReadStamp::of(&crate::read_marks::place_of(config, &repo.root));
        // **빚진 읽기는 표식이 그대로여도 한다**([`App::follow_config`] 와 한 자, moai-po6v) — 권한을
        // 되돌리는 `chmod` 은 표식을 안 바꿔, 표식만 보면 그 한 번이 세션 내내 [NEW] 를 세워 둔다.
        // 재는 자는 [`layer::owed`] 하나다 — 여기와 저기에 저마다 적으면 갈래를 더한 날 한쪽만 고쳐진다.
        let owed = layer::owed(&self.site.read_tried);
        if self.site.read_stamp == Some(now) && !owed {
            return;
        }
        self.load_read();
    }

    /// 사용자 설정 파일이 바뀌었으면 거기 사는 둘을 다시 든다 — 적어 둔 읽음과 층의 줄(moai-en4u).
    /// 한때 손으로 누르는 다시 읽기(`SPC r`)만 이 둘을 읽었다. 키를 걷으며 걸음마다 재는 표식에 태웠다
    /// — 누르는 것을 잊으면 옆 터미널의 `moai read` 가 적은 줄에 [NEW] 가 남고, 밖에서 등록한
    /// 프로젝트가 층에 안 선다.
    ///
    /// **이 탐색기가 쓴 것에도 한 번 돈다**(보기 토글·읽음). 막지 않는다 — 제 쓰기만 골라 건너뛰려면
    /// 쓰기마다 표식을 다시 재야 하고, 그 틈에 옆이 쓴 것을 제 것으로 삼킨다. 도는 값은 파일 하나
    /// 읽기와, 층이 있으면 낡은 줄만 스레드로 다시 읽는 것이다(`relayer` — 본 줄의 셈은 옮겨 든다).
    fn follow_config(&mut self) {
        let Some(path) = self.user_config.as_deref() else { return };
        let now = crate::store::stamp(path);
        let Some(was) = self.config_stamp.replace(now) else { return };
        // **빚진 읽기는 표식이 그대로여도 한다**(moai-po6v) — 띄울 때 진 읽기가 여기 든다. 표식은 읽기
        // **전에** 재므로 진 뒤에도 파일의 것과 같아, 표식만 보면 그 한 번의 실패가 세션 내내 남고
        // 등록한 프로젝트가 영영 안 선다. `config_stamp` 를 `None` 으로 두는 것으로는 안 된다 — 다음
        // 걸음이 같은 표식을 다시 재고 그것을 밑값 삼아 돌아설 뿐이다.
        //
        // **언제 갚는가는 갈래가 정한다**([`crate::user_config::Again`]) — 잠깐인 것은 다음 걸음에,
        // 고쳐도 표식이 안 바뀌는 것(권한)은 시계로, 고치면 파일이 바뀌는 것(깨진 글·사라진 파일)은
        // 표식에 맡기고 스스로는 안 한다. 한 가지로 재면 한쪽이 영영 안 풀리거나 걸음마다 헛돈다.
        // 재는 자는 [`layer::owed`] 하나다 — 읽음도 그것으로 잰다([`App::follow_read`]).
        let owed = layer::owed(&self.config_tried);
        if was == now && !owed {
            return;
        }
        // **한 번만 읽는다**(moai-7yil) — 적어 둔 읽음과 등록 목록이 한 파일에 살아, 둘이 저마다 읽던
        // 판은 이 걸음마다 같은 글을 두 번 파싱했다. 이 길은 자주 돈다: 보기 토글과 읽음이 그 파일을
        // **스스로 써서**, `r` 을 누르고 있으면 누를 때마다 두 번 파싱이 붙었다.
        let mut reg = crate::user_config::read(Some(path));
        // 표식은 늘 올린다 — 다시 읽을 때를 정하는 자는 이제 갈래다(위의 `owed`). 한때는 못 읽으면
        // 표식을 물려 다음 걸음이 같은 차이를 다시 보게 했는데(moai-9p7v), 그 길은 띄울 때 진 읽기를
        // 못 갚는다(밑값으로 삼을 `was` 가 없다). 재는 자가 둘이면 한쪽만 고쳐진다.
        self.config_tried.saw(reg.trouble);
        // **사라진 것을 본 걸음은 표식도 사라진 것으로 적는다**(리뷰) — 표식은 읽기 **전에** 재므로,
        // 그 틈에 없어진 파일은 있던 때의 표식을 인 채 남는다(NFS 의 attribute cache 는 그 틈을 초
        // 단위로 벌린다). 사라진 파일은 표식에 기대는 갈래라([`crate::user_config::Again::Never`])
        // 그대로 두면, 같은 (고친 때, 길이)로 돌아온 파일 — 깜빡인 마운트나 `cp -p` 로 되돌린 것 —
        // 이 표식으로 안 보여 "파일이 사라졌다" 를 인 층이 세션 내내 안 풀린다.
        if reg.trouble == Some(crate::user_config::Trouble::Gone) {
            self.config_stamp = Some(None);
        }
        // **옛 `[read]` 를 다시 든다**(moai-bwce, 사용자 결정 3) — 읽음은 이제 제 파일에 살고 여기는
        // 겹쳐 보는 옛 표다. 이 바이너리는 여기 안 적지만 옛 바이너리나 사람 손은 적을 수 있다.
        //
        // **탈이 있으면 들고 있던 것을 둔다**(리뷰) — 갈래를 안 가린다. 파싱이 지면 `user_config::read`
        // 는 `read` 를 빈 표로 둔 채 까닭만 대는데, 그 빈 표를 들이면 내 줄이 통째로 [NEW] 로 선다.
        // 걷어 낸 `read_marks_at` 이 **못 읽거나 깨졌으면** `None` 을 내 막던 자리다. 사라진 파일도
        // 여기 든다(moai-po6v) — autofs 홈이 끊긴 자리를 "읽었더니 비었다" 로 들면 같은 해가 난다.
        // 층도 그대로 들고 선다(`App::relayer_with`, 사용자 결정 2026-09-19).
        //
        // 같으면 안 든다 — 보기 토글이 `[tui]` 만 고쳐도 이 길은 돌고(스스로 쓴다), 다시 드는 일은
        // 읽음 파일 읽기와 `query::unread` 의 줄 수만큼 걷기다.
        if reg.trouble.is_none() && self.legacy_read != reg.read {
            self.legacy_read = std::mem::take(&mut reg.read);
            self.load_read();
        }
        self.relayer_with(Some(&reg), None);
    }

    /// 안 읽은 줄을 다시 센다. **누군지 모르면 아무것도 안 센다** — 읽기는 사람을 묻지 않는다
    /// (CLAUDE.md). 그러면 [NEW] 가 한 줄도 안 서고, 그것이 설정 없는 기계의 옳은 화면이다.
    fn recount_unread(&mut self) {
        self.recount_unread_in(Seat::Here);
    }

    /// [`App::recount_unread`] 와 같되 **그 프로젝트의** 안 읽은 줄을 다시 센다(moai-5v3q) —
    /// 한눈 보기에서 남의 줄에 읽음을 적으면 그 줄의 `[NEW]` 가 내려야 한다.
    fn recount_unread_in(&mut self, seat: Seat) {
        // **누구인가는 그 프로젝트의 뿌리에서 푼다** — 프로젝트마다 git 설정이 다를 수 있고,
        // 안 풀면 남의 프로젝트의 `[NEW]` 가 띄운 자리의 사람으로 서서 `moai -C <그 프로젝트>
        // read --all` 과 다른 줄을 센다(`App::enter_project` 가 들어갈 때 하는 것과 같은 자다).
        let me = match seat {
            Seat::Here => self.me.clone(),
            Seat::Place(n) => match self.layer.as_ref().and_then(|l| l.places.get(n)).and_then(|p| p.site.as_ref()) {
                Some(site) => site.repo.as_ref().map(|r| r.root.clone()).and_then(|root| self.whoami(&root)),
                None => return,
            },
        };
        let Some(site) = self.site_mut(seat) else { return };
        let Some(me) = me else {
            site.unread.clear();
            return;
        };
        // **읽음을 못 들었으면 아무것도 안 센다**(moai-2gep) — 위의 "누군지 모르면 안 센다" 와 같은
        // 자다. 빈 표로 세면 **내게 온 줄이 모두** [NEW] 로 서는데, 그것은 "다 안 읽었다" 가 아니라
        // "모른다" 다 — 그 화면의 `SPC m a` 한 번이 모르는 것을 다 읽음으로 찍는다. 들고 있는 표가
        // 있으면 그것으로 센다(`App::enter_project` 가 옮겨 든 것이 그것이다).
        //
        // 읽음 파일이 **없는** 것은 여기 안 든다 — 탈이 아니라 아직 아무것도 안 읽은 프로젝트의
        // 정상이고, 그때 모든 줄이 [NEW] 인 것이 맞는 답이다.
        //
        // **[`crate::user_config::Trouble::Gone`] 도 안 든다**(리뷰) — 읽기는 그 갈래를 혼자 안 세운다
        // ([`crate::read_marks::Marks::trouble`]). 세우는 자리는 [`App::read_marks_of`] 하나고 거기서
        // 서는 뜻은 "**곁의 설정이** 사라졌다" 다 — 읽음 파일 쪽은 멀쩡히 없는 것으로 읽혔다. **설정을
        // 아직 안 만든 기계가 그 자리에 그대로 든다**(`user_config::read` 가 없는 파일에 `Gone` 을
        // 남기고 `cmd::tui` 가 그것을 `App::config_tried` 에 적는다) — 그것까지 세면 처음 띄운 사람에게
        // **[NEW] 가 한 줄도 안 서고** 까닭도 없다(`Gone` 은 `problems` 가 빈다). 홈이 끊긴 쪽은 들고
        // 있던 표가 지킨다([`App::take_read`] 가 탈 앞에서 `seen` 을 안 덮는다) — 그 표가 비어 있으면
        // 애초에 잃을 것이 없어 모든 줄이 [NEW] 인 것이 맞다.
        if site.read_tried.trouble.is_some_and(|t| t != crate::user_config::Trouble::Gone) && site.seen.is_empty() {
            site.unread.clear();
            return;
        }
        // **그 프로젝트의 표로 센다**(moai-bwce) — `App` 의 맵 하나로 세던 판은 한눈 보기의 모든
        // 프로젝트를 남의 읽음으로 셌다.
        let Site { issues, seen, unread, .. } = site;
        *unread = crate::query::unread(issues, &me, seen).into_iter().map(str::to_string).collect();
    }

    /// 이 뿌리에서 나는 누구인가 — `이름 (메일)`(moai-j038.vna). 헤더([`App::told_user`])와 같은 자
    /// (`identify`·`user`)로 푼다. **묻지 않는다** — 못 풀면 `None` 이고 [NEW] 가 안 설 뿐이다.
    pub fn whoami(&self, root: &std::path::Path) -> Option<String> {
        (self.identify)(self.user.as_deref(), root)
            .ok()
            .map(|a| crate::model::label(&a.name, Some(&a.email), crate::config::Naming::Full))
    }

    /// 읽었다고 적는다(moai-z9pc) — `r`(이 줄)·`SPC m a`(안 읽은 것 전부)·`SPC m g`(이 묶음과 그 밑).
    ///
    /// **CLI 의 `moai read` 와 같은 자다**(moai-j038.vna) — `r` 은 `moai read <id>` 처럼 그 줄을 적고(내게
    /// 온 줄이 아니어도, 누군지 몰라도), `SPC m a` 는 `--all` 처럼 내게 온 안 읽은 것을, `SPC m g` 은
    /// `-e` 처럼 [`crate::nav::Index::under_group`] 을 적는다. 한때 `r` 은 안 읽은 줄만 적어, 같은 "이
    /// 줄을 읽었다" 가 두 표면에서 다른 상태를 남겼다.
    ///
    /// **이미 읽은 줄은 다시 안 적는다** — 가르는 것은 **락 안에서 읽은 파일의 표**다. 화면이 든 표는
    /// 옆 터미널의 `moai read` 를 모르므로 그것으로 가르면, 옆에서 방금 적은 새 도장을 이 화면이 든
    /// 낡은 줄의 도장으로 덮는다. 적는 값은 **이 화면이 그린 줄의 `updated_at`** 이다(moai-lyc1) — 본 것이
    /// 그 줄이다. 적고 나면 그 파일의 표를 든다.
    ///
    /// **내 설정에만 쓴다**(`user_config::update`) — 트래커 파일은 안 건드린다. 락을 잡는 쓰기라
    /// 실패할 수 있고, 그때는 화면도 안 바꾼다: 다음에 다시 누르면 된다. 설정 자리를 모르면
    /// (시험·설정 없는 기계) 화면에서만 걷는다 — 읽기는 사람을 묻지 않는다.
    fn mark_read(&mut self, act: keys::Browse, rows: &[Row]) {
        use keys::Browse as B;
        let cur = self.current_of(rows);
        // **읽음도 그 줄의 프로젝트에 적는다**(moai-5v3q) — 한눈 보기의 줄은 남의 목록의 첨자라,
        // 지금 선 프로젝트로 읽으면 엉뚱한 줄에 도장을 찍는다. **머리줄은 여기 안 온다**: 층의
        // 머리줄에서 읽음 셋은 아예 안 듣는다(`keys::Browse::enabled` 의 `on_row`) — 층의 줄은
        // 프로젝트고 안 읽은 줄은 들어간 프로젝트의 것이라는 결정(moai-j038.vna)이다. 그 결정을
        // 넓히려면 키 쪽을 먼저 연다.
        let seat = match &cur {
            Some(Row::Item(seat, ..)) => *seat,
            _ => Seat::Here,
        };
        // 빠진 프로젝트의 줄에는 안 적는다(moai-m59y) — 갈음한 프로젝트에 남의 첨자로 도장을
        // 찍으면 되돌리는 길이 도구 밖에만 남는다.
        let Some(site) = self.site_of_seat(seat) else { return };
        let targets: Vec<usize> = match act {
            B::Read => match &cur {
                Some(Row::Item(_, e, _)) => e.at().into_iter().collect(),
                _ => Vec::new(),
            },
            B::ReadAll => site.unread.iter().filter_map(|id| site.index.find(id)).collect(),
            B::ReadGroup => {
                let Some(Row::Item(_, e, _)) = &cur else {
                    self.notice = Some(say(self.site.lang, "tui.read.pick_in_group").into());
                    return;
                };
                let Some(group) = self.group_of(seat, e) else {
                    self.notice = Some(say(self.site.lang, "tui.read.not_in_group").into());
                    return;
                };
                let Some(site) = self.site_of_seat(seat) else { return };
                site.index.under_group(&site.issues, &group)
            }
            _ => return,
        };
        let Some(site) = self.site_of_seat(seat) else { return };
        // 넘친 첨자는 건너뛴다 — 그 줄은 이미 없다.
        let ids: std::collections::BTreeSet<&str> =
            targets.iter().filter_map(|&at| site.issues.get(at)).map(|i| i.id.as_str()).collect();
        if ids.is_empty() {
            self.notice = Some(say(self.site.lang, "tui.read.nothing").into());
            return;
        }
        let issues = &site.issues;
        let pick = |seen: &std::collections::BTreeMap<String, String>| {
            crate::query::read_marks_of(issues.iter().filter(|i| ids.contains(i.id.as_str())), seen)
        };
        // **그 줄이 사는 프로젝트의 읽음 파일에 적는다**(moai-bwce) — 한눈 보기의 줄이면 남의 프로젝트다.
        // 뿌리를 모르면(시험의 저장소 없는 App) 화면에서만 걷는다.
        let root = site.repo.as_ref().map(|r| r.root.clone());
        // **트래커에 없는 id 를 이 자리에서 걷는다**(moai-dt5q, 사용자 결정 2) — 여기는 그 프로젝트의
        // 줄을 이미 들고 있어, 설정 쓰기가 트래커를 읽어야 하는 일이 안 생긴다. 닫힌 줄은 안 걷는다.
        //
        // **못 읽은 줄이 쓰는 id 도 지킨다**(리뷰, `cmd::read` 와 같은 자) — 읽어 낸 줄은 트래커가 든
        // 줄이 아니다. 한 줄이 깨진 동안 누른 `r` 하나가 그 이슈의 읽음을 걷으면, 줄을 고친 뒤 [NEW]
        // 가 되살아난다.
        //
        // **이 화면이 트래커 전부를 봤을 때만 걷는다**(리뷰) — `cmd::read` 의 `keep_for_prune` 과 같은
        // 물음이다. 안 본 줄이 있으면 그 줄의 읽음이 조용히 사라지고, 여기 안 보이는 자리가 셋이다.
        //
        // - **겹쳐 보기가 꺼져 있다**(`w`). 그러면 `site.issues` 에 옆 워크트리의 줄이 없는데,
        //   켜고 찍은 도장은 이 파일에 있다 — `w` 를 껐다 켜는 것만으로 [NEW] 가 오갔다
        // - **옆 스냅샷 하나를 못 읽었다**(`site.elsewhere`). 켜 두고도 그 줄은 안 왔다
        // - **id 를 못 읽은 줄**(`site.unreadable` 의 `None`). 무엇을 지킬지 모른다
        // - **줄이 낡았다.** 옆 세션이 방금 세운 이슈를 옆 터미널이 읽음으로 적어도, 이 화면은 다음
        //   걸음(최대 700ms)까지 그 줄을 모른다 — 그 틈에 누른 `r` 이 그 도장을 걷는다
        let fresh_enough = self.worktree
            && site.elsewhere.is_empty()
            && site.unreadable.iter().all(Option::is_some)
            && site.repo.as_ref().is_some_and(|r| stamp_of(r) == site.stamp);
        let known: std::collections::BTreeSet<&str> = site
            .issues
            .iter()
            .map(|i| i.id.as_str())
            .chain(site.unreadable.iter().filter_map(Option::as_deref))
            .collect();
        // **자리를 못 푼 까닭도 함께 받는다**(moai-ajh2) — 쓰는 길이 그것을 버리던 판은 옛 철자 자리에
        // 적고도 "✓ 읽음" 만 세웠다.
        let (written, problems): (Vec<String>, Vec<String>) = match (self.user_config.clone(), root) {
            (Some(config), Some(root)) => {
                let wrote = crate::read_marks::update(&config, &root, |sheet| {
                    let marks = pick(&sheet.marks().0);
                    let written = sheet.mark(&marks)?;
                    if fresh_enough {
                        sheet.prune(&known);
                    }
                    Ok((written, sheet.marks().0))
                });
                match wrote {
                    Ok(crate::read_marks::Wrote { value: (written, mut seen), problems }) => {
                        // 파일의 표를 그대로 든다 — 옆 터미널이 그사이 적은 줄까지 함께 온다. 겹쳐만
                        // 보는 자리들은 여기 다시 얹는다 — 옛 철자로 선 파일(moai-f5e3)과 설정의 옛
                        // `[read]`(사용자 결정 3)다. 겹치는 자는 `read_marks::read` 와 **한 함수**다
                        // (`read_marks::overlay_older`, 리뷰). 둘로 두면 옛 표를 걷는 날 한쪽만 걷힌다 —
                        // 실제로 갈렸을 때 `r` 한 번이 옛 철자 파일의 읽음을 화면에서 지워, 적을 것이
                        // 없는 판(이미 읽은 줄)에서는 파일이 안 바뀌어 [NEW] 가 이 세션 내내 섰다.
                        crate::read_marks::overlay_older(&config, &root, &mut seen, &self.legacy_read);
                        if let Some(site) = self.site_mut(seat) {
                            site.seen = seen;
                        }
                        // **쓴 뒤에 표식을 박지 않는다**(리뷰). 여기서 재면 락을 놓은 **뒤**라, 그 틈에
                        // 옆이 쓴 파일의 표식을 제 것으로 박는다 — 그러면 [`App::follow_read`] 가 영영
                        // 같다고 보아 옆의 줄이 이 화면에 안 닿는다(이 표식이 열어 둔 바로 그 길이다).
                        // 다음 걸음이 한 번 더 읽는 값이 그것보다 싸다.
                        (written, problems)
                    }
                    Err(e) => {
                        let why = crate::text::one_line(&e.to_string());
                        self.notice = Some(fill(say(self.site.lang, "tui.read.unwritten"), &[("why", &why)]));
                        return;
                    }
                }
            }
            _ => {
                let marks = pick(&site.seen);
                let written = marks.keys().cloned().collect();
                if let Some(site) = self.site_mut(seat) {
                    site.seen.extend(marks);
                }
                (written, Vec::new())
            }
        };
        self.recount_unread_in(seat);
        let lang = self.site.lang;
        let said = match written.as_slice() {
            [] => say(lang, "tui.read.nothing").to_string(),
            [one] => fill(say(lang, "tui.read.marked_one"), &[("id", one)]),
            many => fill(say(lang, "tui.read.marked"), &[("n", &many.len().to_string())]),
        };
        // **적었다는 말과 어디에 적었는지를 함께 댄다**(moai-ajh2). 까닭만 세우면 `r` 이 먹었는지를 못
        // 보고, 적었다는 말만 세우면 옛 철자 자리에 적힌 것을 어디서도 못 본다. 여기 낼 것은 스치는 알림
        // 한 줄뿐이라([`App::take_read`] 와 같은 자리) 첫 까닭만 댄다.
        self.notice = Some(match problems.first() {
            Some(why) => format!("{said} — {}", crate::text::one_line(why)),
            None => said,
        });
    }

    /// `SPC m g` 이 읽을 묶음(에픽·마일스톤)의 id(moai-j038.vna) — 커서가 묶음 줄에 섰으면 그 묶음, 아니면
    /// 지금 경로에서 **가장 안쪽의 에픽·마일스톤**이다. 에픽 줄에서 누르면 그 에픽이 든 마일스톤이 아니라
    /// 그 에픽이다 — 누른 사람이 시키지 않은 줄을 읽지 않는다.
    ///
    /// **뿌리도 바구니도 이슈 폴더도 묶음이 아니다.** 경로를 그대로 묶음으로 쓰던 때는 빈 경로(뿌리)만
    /// 막아, 마일스톤이 하나라도 있는 저장소에서 `(마일스톤 없음)` 안의 줄에 서서 누르면 마일스톤 밖의 안
    /// 읽은 것이 통째로 적혔다(`(길 잃음)` 도 같다) — `SPC m g` 이 `SPC m a` 가 되고, 되돌리는 길은 도구 밖에만
    /// 있다. 자식 있는 이슈 폴더 안에서 누르면 그 폴더만 읽고 에픽의 나머지와 묶음 줄 자신을 빠뜨렸다.
    fn group_of(&self, seat: Seat, e: &Entry) -> Option<String> {
        // 빠진 프로젝트의 줄은 묶음이 없다(moai-m59y) — 갈음한 목록에서 읽으면 남의 에픽이 나온다.
        let site = self.site_of_seat(seat)?;
        if let Entry::Dir { at: Some(at), .. } = e
            && let Some(i) = site.issues.get(*at)
            && crate::report::is_group(i)
        {
            return Some(i.id.clone());
        }
        // **줄이 사는 자리에서 읽는다**(`home_of`), 지금 디렉터리에서 읽지 않는다. 지금 디렉터리로
        // 재던 때는, 펼쳐 든 멤버 줄에 서서 누르면 그 줄의 에픽이 아니라 **그 위 마일스톤**이 나와
        // `SPC m g` 이 마일스톤 전체를 읽음으로 적었다(리뷰) — 이 함수가 막으려던 바로 그것이고,
        // 되돌리는 길은 도구 밖에만 있다. 바구니는 첨자가 없어 지금 자리로 읽고, 그 자리에 묶음이
        // 없으면 그대로 `None` 이다.
        // 넘친 첨자에는 집이 없다 — `Index::home_of` 도 첨자를 그대로 짚는다(위의 `get` 과 한 자다).
        let home: &[Seg] = match e.at() {
            Some(at) if at >= site.issues.len() => return None,
            Some(at) => site.index.home_of(at),
            None => &site.path,
        };
        home.iter().rev().find_map(|seg| match seg {
            Seg::Epic(id) | Seg::Milestone(Some(id)) => Some(id.clone()),
            Seg::Issue(_) | Seg::Milestone(None) | Seg::Lost => None,
        })
    }

    /// 보기 토글 하나(`SPC v`). **커서는 줄의 정체로 붙든다** — 숨긴 줄에 서 있었으면 그 자리
    /// 가까이 남는다. 첨자로 두면 위에서 줄이 빠질 때마다 커서가 딴 이슈로 미끄러진다.
    ///
    /// `rows` 는 키 처리가 **이미 센 목록**이다(moai-zrzo) — 여기서 `current()` 로 다시 세면 토글 한 번에
    /// 목록을 세 번 센다(키 처리·붙들 줄·바뀐 뒤).
    fn look(&mut self, act: keys::Browse, rows: &[Row]) {
        use keys::Browse as B;
        let held = self.current_of(rows).map(|r| self.anchor_of(&r));
        match act {
            B::Column(n) => {
                // 번호는 **이 화면의 칸**을 센다([`App::screen_statuses`]) — 한눈 보기에서는 줄을
                // 낸 프로젝트들의 칸이고, 키 바·메뉴가 대는 번호와 같은 자여야 한다.
                if let Some(s) = self.screen_statuses().get(usize::from(n)).cloned() {
                    self.view.toggle(&s);
                }
            }
            B::Done => self.view.toggle(crate::config::DONE),
            B::Deferred => self.view.hide_deferred = !self.view.hide_deferred,
            // 이 프로젝트의 칸만 걷는다 — 다른 프로젝트에만 있는 칸 이름은 여기서 아무것도 안 숨겼으니
            // 들고 있는다(`View::show_all`).
            B::ShowAll => self.view.show_all(&self.screen_statuses()),
            B::Sort(o) => self.order = self.order.press(o),
            _ => return,
        }
        self.regrip(held);
        self.save_look();
    }

    /// 지금 디렉터리의 줄들. 차례는 고른 것(`SPC s`)이다.
    ///
    /// **한눈 보기에서는 프로젝트를 가로지른다**(moai-eyre, 사용자 결정 2026-09-19) — 프로젝트마다
    /// 머리줄 하나가 서고, 그 밑에 그 프로젝트의 줄이 선다. 머리줄을 다 접으면 옛 프로젝트 층과 같은
    /// 화면이다. **줄을 한 목록으로 합치지 줄 자체를 합치지는 않는다** — 프로젝트마다 제 `Site` 의
    /// 첨자를 쓰고([`Seat`]), 같은 id 를 쓰는 두 프로젝트가 한 트리에서 섞이지 않는다(moai-4ia9 의
    /// 되돌리지 않을 선).
    ///
    /// **아직 안 읽은 프로젝트는 머리줄만 선다** — 읽는 것은 펼칠 때다(moai-12yx).
    pub fn rows(&self) -> Vec<Row> {
        if let Some(l) = self.layer.as_ref().filter(|_| self.on_layer()) {
            let mut rows = Vec::new();
            for (n, p) in l.places.iter().enumerate() {
                rows.push(Row::Project(n));
                if self.folded.contains(&p.path) {
                    continue;
                }
                if let Some(site) = p.site.as_ref() {
                    rows.extend(self.rows_in(Seat::Place(n), site, &|at| site.visible(at, self.searching())));
                }
            }
            return rows;
        }
        self.rows_where(&|at| self.visible(at))
    }

    /// 펼친 프로젝트의 줄을 **스레드에 읽으러 보낸다**(moai-12yx). 이미 들었거나 이미 줄 서
    /// 있으면 아무 일도 안 한다. 읽는 동안 그 머리줄은 도는 글리프를 세운다(`draw::place_line`).
    ///
    /// **그 자리에서 안 읽는다**(사용자 결정 2026-09-19) — 워크트리 일곱에 138→458ms 를 잰 값이
    /// (moai-uxrn) 펼치는 키 하나에 통째로 실리면 큰 프로젝트를 펼칠 때마다 화면이 그만큼 멈춘다.
    /// 층의 요약이 스레드로 간 것과 같은 까닭이다(moai-ezwu).
    pub(super) fn want_site(&mut self, at: usize) {
        let Some(place) = self.layer.as_ref().and_then(|l| l.places.get(at)) else { return };
        if place.site.is_some() {
            return;
        }
        let path = place.path.clone();
        if let Some(layer) = self.layer.as_mut()
            && !layer.wanted.contains(&path)
            && layer.reading.as_ref().is_none_or(|(p, ..)| *p != path)
        {
            layer.wanted.push(path);
        }
    }

    /// 줄 선 프로젝트 하나를 읽으러 보낸다 — **한 번에 하나만 돈다**. 못 열면 그 줄은 제 요약이
    /// 대던 말을 그대로 대고([`App::open_place`]) 머리줄만 선다.
    pub(super) fn read_wanted(&mut self) {
        if self.layer.as_ref().is_none_or(|l| l.reading.is_some() || l.wanted.is_empty()) {
            return;
        }
        let Some(path) = self.layer.as_mut().map(|l| l.wanted.remove(0)) else { return };
        let Some(at) = self.layer.as_ref().and_then(|l| l.position(&path)) else { return };
        // 여는 것은 그 자리에서 한다 — 못 여는 까닭을 그 줄에 세우는 길이 이것 하나다(`open_place`).
        // **여는 데까지만 본다**(`Depth::Lean`, moai-m59y) — 줄은 바로 아래 스레드가 읽는다.
        // 여기서 스냅샷까지 읽으면 그 한 판을 UI 실이 치르고 일꾼이 같은 파일을 또 판다.
        let Some(repo) = self.open_place(at, layer::Depth::Lean) else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let read = self.read;
        let worktree = self.worktree;
        let lang = self.site.lang;
        let sent = repo.clone();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(read(&sent, worktree, lang));
        });
        if let Some(layer) = self.layer.as_mut() {
            layer.reading = Some((path, rx, handle));
        }
        // 연 저장소는 읽어 온 것을 들일 때 [`Site`] 에 얹는다.
        self.reading_repo = Some(repo);
    }

    /// 스레드가 읽어 온 줄을 그 층 줄에 들인다. 그새 목록에서 빠진 경로는 버린다 — 층의 요약을
    /// 들이는 자와 같다(`Layer::adopt`).
    pub(super) fn follow_site(&mut self) {
        let Some(layer) = self.layer.as_mut() else { return };
        let Some((path, rx, _)) = layer.reading.as_ref() else {
            self.read_wanted();
            return;
        };
        let (path, got) = match rx.try_recv() {
            Ok(got) => (path.clone(), Some(got)),
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => (path.clone(), None),
        };
        let (_, _, handle) = layer.reading.take().expect("바로 위에서 보았다");
        let repo = self.reading_repo.take();
        // **`Tab` 의 뜻은 이 읽기 하나로 끝난다** — 못 읽었을 때 그 뜻을 남겨 두면, 다음에 `l` 로
        // 한 층만 펴려던 사람이 통째로 펼쳐진 프로젝트를 본다. `Depth::Lean` 이 "열리지만 스냅샷을
        // 못 읽는" 저장소를 이 갈래로 보내면서 그 자리가 실제로 닿는다(moai-m59y).
        let deep = self.deep.remove(&path);
        match got {
            Some(Ok(fresh)) => {
                let cfg = repo.as_ref().map_or_else(|| self.site.cfg.clone(), |r| r.config.clone());
                let mut site = Site::of(fresh.issues, fresh.index, fresh.ground, cfg, Vec::new(), fresh.unreadable);
                // **펼친 프로젝트의 줄도 같은 말로 선다**(moai-ra67, 리뷰) — 한 화면의 말이지
                // 프로젝트의 것이 아니다. 안 이으면 한눈 보기에서 편 남의 프로젝트의 바구니
                // 이름만 [`Site::lang`] 의 처음값으로 서서, 같은 목록의 두 프로젝트가 같은
                // 바구니를 다른 말로 부른다.
                site.lang = self.site.lang;
                site.repo = repo;
                site.now = fresh.now;
                site.stamp = fresh.stamp;
                site.warnings = fresh.warnings;
                site.origin = fresh.origin;
                site.elsewhere = fresh.elsewhere;
                site.unfound = fresh.unfound;
                site.watched = fresh.watched;
                site.read_at = Some(std::time::Instant::now());
                // **그 프로젝트의 읽음을 제 파일에서 든다**(moai-bwce) — 한눈 보기의 줄도 제 [NEW] 를
                // 제 표로 센다. `App` 의 맵 하나를 같이 보던 판은 이름이 같은 두 저장소의 같은 id 가
                // 서로의 [NEW] 를 내렸다(moai-omx7 의 화면 쪽 모습이다).
                //
                // **들고 있던 것은 이 줄이 이미 들었던 표다**(리뷰) — 여기 `site` 는 방금 지은 것이라
                // `seen` 이 빈 표다. [`App::take_read`] 에 그대로 주던 판은 "탈이 있으면 들고 있던 것을
                // 둔다" 가 빈 말이 되어, 못 읽은 읽음 파일 하나가 그 프로젝트의 줄을 통째로 [NEW] 로
                // 세웠다. 갈음할 옛 줄에서 먼저 옮겨 든다 — 베끼지 않는다(그 줄은 곧 버려진다).
                if let Some(root) = site.repo.as_ref().map(|r| r.root.clone()) {
                    if let Some(held) = self
                        .layer
                        .as_mut()
                        .and_then(|l| l.places.iter_mut().find(|p| p.path == path))
                        .and_then(|p| p.site.as_mut())
                    {
                        site.seen = std::mem::take(&mut held.seen);
                        site.read_stamp = held.read_stamp.take();
                        // **갈래와 읽어 본 때는 안 옮긴다**(리뷰) — 바로 아래 [`App::take_read`] 가
                        // 이 걸음의 읽기로 둘 다 다시 적는다. 옮겨 봐야 그 자리에서 덮이고, 덮이는
                        // 줄은 "옮겼으니 이어진다" 는 거짓말을 남긴다. 표식만 옮기는 것은 다르다 —
                        // 잠깐의 실패([`crate::user_config::Again::Step`])에는 그쪽이 안 적는다.
                    }
                    if let Some(got) = self.read_marks_of(&root)
                        && let Some(told) = Self::take_read(&mut site, got)
                    {
                        self.notice = Some(told);
                    }
                }
                if let Some(place) = self.layer.as_mut().and_then(|l| l.places.iter_mut().find(|p| p.path == path)) {
                    place.site = Some(site);
                }
                let landed = self.layer.as_ref().and_then(|l| l.position(&path));
                if let Some(at) = landed {
                    // **든 줄에 화면의 것을 곧바로 건다**(리뷰). [`Site::of`] 는 `shown` 과 `unread` 를
                    // 비워 두고, 그 둘을 채우는 자([`App::see`]·[`App::recount_unread_in`])는 보기
                    // 토글과 쓰기에만 붙어 있다 — 안 걸면 방금 펼친 프로젝트만 보기를 안 따르고
                    // (`view_shows` 가 빈 `shown` 을 "다 보인다" 로 읽는다) `[NEW]` 도 한 줄도 안
                    // 선다. 같은 화면의 두 프로젝트가 한 토글에 다르게 서는 것이 moai-1xo5 가
                    // 없애려던 바로 그것이다.
                    let view = self.view.clone();
                    if let Some(site) = self.site_mut(Seat::Place(at)) {
                        Self::see_in(site, &view);
                    }
                    self.recount_unread_in(Seat::Place(at));
                }
                // `Tab` 으로 "다 펴 달라" 며 기다린 프로젝트면 이제 편다.
                if deep && let Some(at) = landed {
                    self.open_all(Seat::Place(at));
                }
            }
            // 못 읽었다 — 까닭을 한 줄로 대고 그 프로젝트는 머리줄만 선다. 다시 펴면 다시 간다.
            Some(Err(e)) => {
                self.notice = Some(fill(say(self.site.lang, "tui.layer.unread_rows"), &[("why", &e.to_string())]));
            }
            // 읽던 스레드가 죽었다. 루프에서 난 패닉과 같게 되던진다([`App::follow`]).
            None => {
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
            }
        }
        self.read_wanted();
    }

    /// [`App::site_of_seat`] 의 고칠 수 있는 판 — 펼침처럼 **그 프로젝트에 매인 것**을 고칠 때 쓴다.
    ///
    /// **지금 선 것으로 갈음하지 않는다.** 아직 줄을 안 읽은 프로젝트(읽는 중이거나 접힌 것)에
    /// 무언가 적으려다 갈음하면, 그 글이 **엉뚱한 프로젝트**에 조용히 적힌다 — 읽기는 한 프레임
    /// 어긋날 뿐이지만 쓰기는 남는다. 읽는 판도 이제 같은 자다(리뷰).
    pub(super) fn site_mut(&mut self, seat: Seat) -> Option<&mut Site> {
        match seat {
            Seat::Here => Some(&mut self.site),
            Seat::Place(n) => self.layer.as_mut().and_then(|l| l.places.get_mut(n)).and_then(|p| p.site.as_mut()),
        }
    }

    /// 그 줄이 사는 프로젝트 — 줄에 실린 [`Seat`] 을 푼다. `..` 과 머리줄은 제 줄이 없어 지금 선
    /// 것으로 답한다. **빠진 프로젝트의 줄은 `None`** 이다([`App::site_of_seat`]) — 갈음해 주면
    /// 받는 쪽이 남의 첨자를 이 목록에 대고, `Index::dir_path` 처럼 첨자를 그대로 짚는 자리에서
    /// 그것이 조용한 오답이 되거나 터진다(moai-m59y).
    pub fn site_of_row(&self, r: &Row) -> Option<&Site> {
        match r {
            Row::Item(seat, ..) => self.site_of_seat(*seat),
            Row::Up | Row::Project(_) => Some(&self.site),
        }
    }

    /// 이 화면에 **줄을 낸 프로젝트들** — 프로젝트 안에서는 선 것 하나, 한눈 보기에서는 펼쳐
    /// 줄을 낸 것 전부다(moai-1xo5 가 보기·정렬·열을 거는 바로 그 범위다).
    ///
    /// 한눈 보기의 `App::site` 는 **줄이 빈 자리 채우개**라(`App::leave_project`) 거기 섞지
    /// 않는다 — 그 설정은 마지막으로 떠난 프로젝트의 것이거나 `layer::blank_config` 다.
    pub(super) fn sites(&self) -> Vec<&Site> {
        let Some(l) = self.layer.as_ref().filter(|_| self.on_layer()) else { return vec![&self.site] };
        l.places.iter().filter(|p| !self.folded.contains(&p.path)).filter_map(|p| p.site.as_ref()).collect()
    }

    /// 이 화면이 번호를 매기고 뱃지에 대고 셈에 쓰는 **칸 이름** — 프로젝트 안에서는 그 설정
    /// 그대로, 한눈 보기에서는 줄을 낸 프로젝트의 칸을 차례대로 모은 것이다(리뷰).
    ///
    /// **한눈 보기에는 "지금 선 프로젝트" 가 없다.** 거기서 `App::site` 의 설정을 읽던 때는,
    /// 칸 이름을 따로 적은 프로젝트를 펼쳐 놓고 `SPC v 1` 을 누르면 그 프로젝트에 없는 이름이
    /// 숨겨져 아무것도 안 숨기고, 그 프로젝트에서 숨긴 칸은 뱃지에 못 서 `SPC v a` 로도 못 풀었다.
    pub fn screen_statuses(&self) -> Vec<String> {
        if !self.on_layer() {
            return self.site.cfg.statuses.clone();
        }
        let mut out: Vec<String> = Vec::new();
        for site in self.sites() {
            for s in &site.cfg.statuses {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
        }
        out
    }

    /// `seat` 이 든 프로젝트 — 한눈 보기의 줄은 제 층 줄의 것을, 프로젝트 안의 줄은 지금 선 것을
    /// 쓴다. **없으면 없다고 답한다.**
    ///
    /// 한때 여기 곁에 지금 선 것으로 갈음해 주는 판(`App::site_at`)이 있었다 — 층 줄이 그새 빠져도
    /// 그 한 프레임에 탐색기가 안 죽게 하려는 것이었다. **걷었다**(리뷰, moai-m59y): 줄에 실린
    /// 첨자는 제 프로젝트의 목록의 것이라(`Seat`) 남의 목록에 대면 엉뚱한 이슈가 그 줄로 서고,
    /// 넘치면 `Index::dir_path`·`home_of`·`Site::spins` 처럼 첨자를 그대로 짚는 자리에서 어차피
    /// 죽는다. 갈음이 지켜 주던 것을 이제는 **부르는 쪽이 그 줄을 건너뛰어** 지킨다 — 빠진 줄은
    /// 다음 프레임에 사라지고, 그때까지 남의 이슈를 그 줄로 세우지 않는다. 그 규칙을 쓰는 판이
    /// [`App::issue_at`]·[`App::site_of_row`]·[`App::site_mut`] 다.
    pub fn site_of_seat(&self, seat: Seat) -> Option<&Site> {
        match seat {
            Seat::Here => Some(&self.site),
            Seat::Place(n) => self.site_of_place(n),
        }
    }

    /// 그 줄의 이슈 — **없는 자리도 넘친 첨자도 `None`** 이다(moai-m59y). 줄에 실린 첨자는 제
    /// 프로젝트의 목록의 것이라([`Seat`]), 그 프로젝트가 없을 때 지금 선 것으로 갈음하면 첨자
    /// 넘침이 조용한 오답이 된다 — 남의 목록에서 그 첨자에 선 이슈가 이 줄로 선다.
    /// 부르는 쪽은 **그 줄을 건너뛴다**: 빠진 줄은 다음 프레임에 사라지고, 그때까지 남의 이슈를
    /// 그 줄로 세우지 않는다.
    pub fn issue_at(&self, seat: Seat, at: usize) -> Option<&Issue> {
        self.site_of_seat(seat)?.issues.get(at)
    }

    /// 지금 디렉터리의 목록 — `keep` 을 지나는 줄로. [`Self::rows`] 는 이것에 거름망과 보기를 건 것이다.
    /// 검색을 풀 때 커서가 설 이웃을 **보기를 안 건 차례**에서 찾으려고 따로 둔다([`Self::after_search`]).
    fn rows_where(&self, keep: &dyn Fn(usize) -> bool) -> Vec<Row> {
        self.rows_in(Seat::Here, &self.site, keep)
    }

    /// 한 프로젝트의 줄 — [`Self::rows_where`] 와 한눈 보기가 같은 몸을 지난다(moai-eyre). 차례·검색·
    /// 펼침은 화면의 것이고(그래서 `self`), 줄과 색인과 칸은 그 프로젝트의 것이다(그래서 `site`).
    fn rows_in(&self, seat: Seat, site: &Site, keep: &dyn Fn(usize) -> bool) -> Vec<Row> {
        let mut rows: Vec<Row> = Vec::new();
        let found_under = self.searched_open(site, keep);
        // **`..` 은 디렉터리에만 선다**(moai-i784). 프로젝트 뿌리에 한 줄 더 세워 층으로
        // 올려 보내던 길은 걷었다 — 층으로 가는 길은 헤더의 `0` 하나다(사용자 결정).
        // 길이 둘이면 뿌리의 `..` 이 디렉터리의 `..` 과 다른 데로 가, 같은 글자가 두 뜻을 진다.
        if !site.path.is_empty() {
            rows.push(Row::Up);
        }
        // **보기는 줄마다 건다** — 숨긴 칸의 묶음이라도 보이는 멤버가 있으면 디렉터리는 선다
        // (`Index::entries_where`). done 에픽 밑에 남은 todo 가 폴더째 사라지면 안 된다.
        rows.extend(
            site.index
                .entries_tree(
                    &site.issues,
                    &site.path,
                    keep,
                    &|a, b| {
                        // 칸은 목록의 글리프와 같은 자로 — 묶음은 멤버에서 읽은 칸이다. 담당은 화면에 선 이름으로.
                        crate::query::order_by(
                            Self::sort_key(self.order.by),
                            self.order.reversed,
                            (&site.issues[a], site.column(a)),
                            (&site.issues[b], site.column(b)),
                            &site.cfg.statuses,
                            site.cfg.naming,
                        )
                    },
                    &|under| site.expanded.contains(under) || found_under.contains(under),
                )
                .into_iter()
                .map(|(e, twig)| Row::Item(seat, e, twig)),
        );
        rows
    }

    /// **검색이 맞힌 줄을 품은 자리는 저절로 열린다**(moai-i5io, 사용자 결정) — 접힌 묶음
    /// 안에서 맞은 줄은 목록에 폴더 한 줄로만 서서, 무엇이 걸렸는지 보려면 사람이 들어가야 했다.
    /// 검색은 보기가 숨긴 줄까지 찾으므로(moai-qnkn) 그 줄이 어디 있는지를 화면이 말해야 한다.
    ///
    /// **거름망(`SPC f`)은 안 연다.** 거름망은 오래 걸어 두고 폴더를 돌아다니는 것이라(Esc 가
    /// 안 푸는 보기와 한 자리다) 저절로 열면 `status=todo` 한 줄에 저장소 전체가 펼쳐진다.
    /// 검색은 한 줄을 찾는 일이라 반대다.
    fn searched_open(&self, site: &Site, keep: &dyn Fn(usize) -> bool) -> std::collections::HashSet<Path> {
        let mut open = std::collections::HashSet::new();
        if !self.searching() {
            return open;
        }
        // 맞은 줄의 **조상 자리 전부**. 맞은 줄만 세면 두 층 밑의 줄은 가운데 폴더가 닫힌 채라
        // 여전히 안 보인다.
        for at in 0..site.issues.len() {
            if !keep(at) {
                continue;
            }
            let home = site.index.home_of(at);
            // **깊은 자리부터 넣는다.** 제 자리가 이미 들었으면 그 위도 들었으니 거기서 끊는다 —
            // 형제가 많은 에픽에서 첫 줄만 값을 치른다. 앞에서부터 넣던 때는 멤버마다 조상 전부를
            // 다시 만들어, 검색 한 글자에 (이슈 수 × 깊이)만큼 `Vec<Seg>` 를 지었다.
            for depth in (1..=home.len()).rev() {
                if !open.insert(home[..depth].to_vec()) {
                    break;
                }
            }
        }
        open
    }

    /// 커서가 선 묶음의 자리 — 그 줄을 펼치면 멤버가 이 자리에서 온다. 묶음이 아니면 없다.
    ///
    /// **자리는 색인에 묻는다**([`crate::nav::Index::dir_path`]) — 화면에 선 차례에서 조상을
    /// 되짚지 않는다. 되짚던 때는 자리를 정하는 자가 둘(`home_of` 와 목록의 차례)이 되어, 차례나
    /// 펼침 규칙이 바뀌는 날 한쪽만 바뀌어도 `l`·`h`·`Tab` 이 엉뚱한 자리를 열고 닫는다 — `nav`
    /// 머리글의 "자리를 정하는 법은 하나다" 가 그것을 막으려고 있는 규칙이다.
    fn dir_at(&self, rows: &[Row]) -> Option<Path> {
        self.dir_of(self.current_of(rows)).map(|(_, path)| path)
    }

    /// [`App::dir_at`] 과 같되 **그 줄이 사는 프로젝트까지** 낸다(moai-i0wd) — 한눈 보기에서는
    /// 펼침이 그 프로젝트의 것이라, 자리만으로는 어느 `Site` 의 `expanded` 를 고칠지 모른다.
    fn dir_seat_at(&self, rows: &[Row]) -> Option<(Seat, Path)> {
        self.dir_of(self.current_of(rows))
    }

    /// 그 줄이 묶음이면 그것이 여는 자리. [`App::dir_at`] 을 커서 밖의 줄(펼친 멤버의 부모)에도 쓴다.
    ///
    /// **빠진 프로젝트의 줄에는 자리가 없다**(moai-m59y) — 갈음한 목록에 남의 첨자를 대면
    /// `Index::dir_path` 가 첨자를 그대로 짚어 터진다. 여기는 `key_ctx` 가 키 하나마다 묻는
    /// 자리라, 그 죽음은 커서가 그 줄에 선 채 아무 키나 누르는 것으로 난다. 고치는 판
    /// ([`App::site_mut`])도 같은 자리에서 `None` 이니, 읽은 자리와 쓴 자리가 안 갈린다.
    fn dir_of(&self, row: Option<Row>) -> Option<(Seat, Path)> {
        match row {
            Some(Row::Item(seat, Entry::Dir { at: Some(at), .. }, _)) => {
                let site = self.site_of_seat(seat)?;
                (at < site.issues.len()).then(|| (seat, site.index.dir_path(&site.issues, at)))
            }
            // 바구니는 제 줄이 없어 첨자가 없다. 바구니 마디(`Milestone(None)`·`Lost`)는 집의 첫
            // 마디로만 서므로(`Index::home_of_work`) 늘 뿌리의 줄이고, 그때 지금 자리가 곧 제 부모다.
            Some(Row::Item(seat, Entry::Dir { seg, at: None }, _)) => {
                let mut path = self.site_of_seat(seat)?.path.clone();
                path.push(seg);
                Some((seat, path))
            }
            _ => None,
        }
    }

    /// 커서의 줄이 **화면에 펼쳐져 있는가** — 사람이 펼친 것(`expanded`)과 검색이 저절로 연 것
    /// ([`App::searched_open`]) 둘 다 센다. 하나만 보던 때는 검색이 연 줄에서 `h` 가 접을 것이
    /// 없는 줄로 읽혀 디렉터리를 통째로 나갔다(리뷰).
    fn open_at(&self, rows: &[Row]) -> Option<Path> {
        self.open_seat_at(rows).map(|(_, path)| path)
    }

    /// [`App::open_at`] 과 같되 그 줄의 프로젝트까지 낸다.
    fn open_seat_at(&self, rows: &[Row]) -> Option<(Seat, Path)> {
        let (seat, path) = self.dir_seat_at(rows)?;
        let site = self.site_of_seat(seat)?;
        (site.expanded.contains(&path) || self.searched_into(seat, &path)).then_some((seat, path))
    }

    /// 검색이 **이 자리를** 저절로 열었는가 — [`App::searched_open`] 의 한 자리 판이다. 집합을
    /// 짓지 않고 묻는다: 키 바와 메뉴가 키 하나마다 이것을 묻는데, 거기서 집합을 지으면 이슈
    /// 전부의 조상 자리를 프레임마다 다시 모은다(`lit` 을 미리 세는 까닭과 같다).
    fn searched_into(&self, seat: Seat, path: &Path) -> bool {
        // 그 집합에 든 자리는 맞은 줄의 조상 자리 전부다 — 곧 "맞은 줄의 집이 이 자리로 시작하는가" 다.
        // **빠진 프로젝트는 아무것도 안 열었다**(moai-m59y) — 갈음한 목록에서 찾으면 남의 줄이
        // 이 자리를 열어 놓은 것으로 읽혀, 접을 것이 없는 줄에서 `h` 가 디렉터리를 통째로 나간다.
        let Some(site) = self.site_of_seat(seat) else { return false };
        self.searching()
            && (0..site.issues.len())
                .any(|at| site.visible(at, self.searching()) && site.index.home_of(at).starts_with(path))
    }

    /// 커서가 **펼친 묶음의 멤버 줄**이면 그 부모 줄의 번호. 목록은 트리 차례라, 위로 올라가며
    /// 처음 만나는 한 층 얕은 줄이 곧 부모다.
    /// **부모는 같은 프로젝트에서 찾는다**(리뷰) — `Seat::Here` 로 못 박던 때는 한눈 보기의
    /// 멤버 줄에서 늘 `None` 이라, 거기서 `h` 가 부모를 접는 대신 "0 으로 층에 간다" 는 (이미
    /// 층인데) 엉뚱한 말을 냈다. 깊이만으로 가르면 머리줄을 넘어 앞 프로젝트의 줄이 부모가 된다.
    fn parent_row(&self, rows: &[Row]) -> Option<usize> {
        let Some(Row::Item(seat, _, twig)) = rows.get(self.cursor) else { return None };
        let (seat, depth) = (*seat, twig.depth().checked_sub(1)?);
        rows[..self.cursor].iter().rposition(|r| matches!(r, Row::Item(s, _, t) if *s == seat && t.depth() == depth))
    }

    /// 멤버 줄의 `h` — **부모 묶음을 접고 그 줄에 선다**(사용자 결정 2026-09-19). nvim-tree·ranger·
    /// netrw 가 그렇게 한다. 한 층 나가면 펼쳐 보던 트리가 통째로 사라지고 한 층 밖에 서서 보던
    /// 자리를 잃는다. 멤버 줄이 아니면 거짓 — 그때는 한 층 나간다.
    fn fold_parent(&mut self, rows: &[Row]) -> bool {
        let Some(up) = self.parent_row(rows) else { return false };
        if let Some((seat, path)) = self.dir_of(rows.get(up).cloned())
            && let Some(site) = self.site_mut(seat)
        {
            site.expanded.remove(&path);
        }
        // 부모 줄 위의 줄은 안 바뀐다 — 번호가 그대로 그 줄이다.
        let rows = self.rows();
        self.stand(&rows, up, None);
        true
    }

    /// 커서가 선 프로젝트 머리줄 — 한눈 보기가 아니거나 이슈 줄이면 없다(moai-i0wd).
    fn head_at(&self, rows: &[Row]) -> Option<usize> {
        match rows.get(self.cursor) {
            Some(Row::Project(at)) => Some(*at),
            _ => None,
        }
    }

    /// 커서의 머리줄이 **펼쳐져 있는가** — 접힘 목록에 없고 그 프로젝트의 줄을 이미 들었다.
    /// 머리줄이 아니면 거짓이다.
    fn head_open(&self, rows: &[Row]) -> bool {
        let Some(at) = self.head_at(rows) else { return false };
        let folded = self.place_path(at).is_some_and(|p| self.folded.contains(p));
        !folded && self.site_of_place(at).is_some()
    }

    /// 머리줄을 편다 — **그 프로젝트를 아직 안 읽었으면 여기서 읽는다**(moai-eyre 의 `fill_site`).
    /// `deep` 이면 그 밑의 묶음까지 다 편다(`Tab`). 이미 펼쳐져 있으면 `Tab` 은 통째로 접고
    /// `l` 은 아무 일도 안 한다 — 묶음 줄의 두 키와 같은 자다([`App::expand_all`]).
    fn unfold(&mut self, rows: &[Row], deep: bool) {
        let Some(at) = self.head_at(rows) else { return };
        let Some(path) = self.place_path(at).map(std::path::Path::to_path_buf) else { return };
        let was = !self.folded.contains(&path) && self.site_of_place(at).is_some();
        if was && deep {
            self.fold(rows);
            return;
        }
        self.folded.remove(&path);
        if deep {
            // **줄이 오면 다 편다**(moai-12yx) — 아직 안 읽은 프로젝트에서는 지금 펼 것이 없다.
            self.deep.insert(path.clone());
        }
        self.want_site(at);
        self.read_wanted();
        if deep {
            self.open_all(Seat::Place(at));
        }
    }

    /// 그 프로젝트의 묶음을 **다 편다**(`Tab`). 아직 줄이 없으면 아무 일도 안 한다 — 읽어 온
    /// 뒤에 [`App::follow_site`] 가 다시 부른다.
    fn open_all(&mut self, seat: Seat) {
        let Some(site) = self.site_mut(seat) else { return };
        let dirs: Vec<Path> = (0..site.issues.len())
            .filter(|&n| site.index.is_dir(&site.issues, n))
            .map(|n| site.index.dir_path(&site.issues, n))
            .collect();
        site.expanded.extend(dirs);
    }

    /// 머리줄을 접는다 — 그 프로젝트의 줄이 목록에서 빠진다. **읽은 것은 안 버린다**: 다시 펴면
    /// 그대로 서고, 낡았으면 표식과 시계가 다시 읽는다.
    fn fold(&mut self, rows: &[Row]) {
        let Some(at) = self.head_at(rows) else { return };
        if let Some(path) = self.place_path(at).map(std::path::Path::to_path_buf) {
            // 밑의 펼침까지 걷는다 — 다 접기(`Tab`)와 같은 자다: 접힌 것을 다시 펼 때 접기 전
            // 모양이 아니라 그 이전 모양이 서면, 같은 자리로 오는 데 두 번을 눌러야 한다.
            if let Some(site) = self.site_mut(Seat::Place(at)) {
                site.expanded.clear();
            }
            self.deep.remove(&path);
            self.folded.insert(path);
        }
    }

    /// 그 층 줄이 제 프로젝트의 줄을 이미 들었는가.
    fn site_of_place(&self, at: usize) -> Option<&Site> {
        self.layer.as_ref()?.places.get(at)?.site.as_ref()
    }

    /// 한 단계 펼친다(`l`·`→`). 이미 펼쳐져 있으면 아무 일도 없다 — 들어가는 것은 `Enter` 다.
    fn expand(&mut self, rows: &[Row]) {
        if let Some((seat, path)) = self.dir_seat_at(rows)
            && let Some(site) = self.site_mut(seat)
        {
            site.expanded.insert(path);
        }
    }

    /// 접는다(`h`·`←`). 접을 것이 없었으면 거짓 — 그때는 한 층 나간다([`App::leave`]).
    ///
    /// **검색이 저절로 연 줄에서도 참이다.** 그 펼침은 `expanded` 에 없어 걷을 것이 없지만, 여기서
    /// 거짓을 내면 눈에 열려 보이는 줄에서 `h` 가 디렉터리를 통째로 나간다 — 보던 자리를 잃는 것이
    /// 아무 일도 안 하는 것보다 나쁘다. 그 펼침은 검색을 풀 때 함께 걷힌다.
    fn collapse(&mut self, rows: &[Row]) -> bool {
        let Some((seat, path)) = self.open_seat_at(rows) else { return false };
        let Some(site) = self.site_mut(seat) else { return false };
        site.expanded.remove(&path);
        true
    }

    /// 재귀로 다 펼치고, 이미 펼쳐져 있으면 밑까지 접는다(`Tab`).
    ///
    /// **접을 때는 밑의 펼침까지 걷는다** — `h` 와 다른 자리다. `h` 는 한 단계라 안의 모양을
    /// 기억해 두는 것이 이롭지만, "다 접기" 가 안을 기억하면 다시 누를 때 접기 전 모양이 아니라
    /// 그 이전 모양이 서서 두 번 눌러야 같은 자리로 온다.
    ///
    /// **접을지는 이 줄이 열렸는가로 가른다**(리뷰). 밑에 남은 펼침으로 가르던 때는, `h` 로 한
    /// 단계 접어 밑의 펼침만 남은 뒤의 `Tab` 이 그 기억만 걷고 화면은 그대로여서 아무 일도 안 한
    /// 누름이 하나 생겼다 — 펼치려면 두 번을 눌러야 했고, 그것이 이 함수가 막으려던 바로 그것이다.
    fn expand_all(&mut self, rows: &[Row]) {
        let Some((seat, path)) = self.dir_seat_at(rows) else { return };
        if self.open_at(rows).is_some() {
            let Some(site) = self.site_mut(seat) else { return };
            let under: Vec<Path> = site.expanded.iter().filter(|p| p.starts_with(&path)).cloned().collect();
            for p in under {
                site.expanded.remove(&p);
            }
            return;
        }
        // 그 자리와 그 밑의 **묶음 줄 전부**. 자리는 `home_of` 가 정한 그대로라 목록이 세우는
        // 줄과 같은 것만 펼친다.
        // **접는 갈래와 같은 프로젝트에 적는다**(리뷰) — 여기만 지금 선 것에 적던 때는, 한눈
        // 보기의 남의 묶음 줄에서 `Tab` 이 아무 일도 안 하면서(빈 색인이라 밑이 안 나온다)
        // 남의 자리를 이 프로젝트의 펼침 목록에 흘렸다. `l`·`h` 는 이미 그 줄의 것에 적는다.
        let Some(site) = self.site_mut(seat) else { return };
        site.expanded.insert(path.clone());
        let under: Vec<Path> = site
            .index
            .descendants(&path)
            .into_iter()
            .filter(|&at| site.index.is_dir(&site.issues, at))
            .map(|at| site.index.dir_path(&site.issues, at))
            .collect();
        site.expanded.extend(under);
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
        //
        // **메뉴만 만지는 키는 그 하나로 안 센다**(moai-g56h) — 상태를 대는 항목은 메뉴를 열린
        // 채로 두므로([`menu::feed`]), `SPC v w` 가 단 알림을 읽고 메뉴를 닫는 Esc 가 곧 "다음 키"
        // 다. 거기서 걷으면 그 알림은 메뉴 창이 떠 있는 동안만 살고, 메뉴를 닫자마자 사라진다 —
        // 알림을 남긴 키와 그것을 지우는 키가 한 누름이다. 메뉴가 열린 채 아무 동작도 안 돈 키
        // (Esc·SPC 닫기, Bksp 한 층 위, 모르는 키, 한 층 내려가기)에는 도로 세운다.
        let carried = self.notice.take();
        let in_menu = menu::open(&self.chord);
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
            // 기다리던 열은 탐색의 것이다 — 글칸에서 돌아온 뒤의 `g` 가 옛 `g` 와 잇지 않게.
            self.chord.clear();
            self.typing(k);
            return;
        }
        // **키의 뜻은 표에서 읽는다**([`keys::BROWSE`]). 표에 없는 키 — Ctrl·Alt 붙은 글자키도
        // — 는 여기 뜻이 없다. 접두어(`g`)는 다음 키를 기다리고, 뜻 없는 다음 키는 그 `g` 와 함께
        // 버린다. SPC 는 메뉴를 열고, 열린 메뉴는 제 규칙(모르는 키 무시·Esc·Bksp)으로 받는다
        // ([`menu::feed`]) — 메뉴로 누른 동작도 바로 누른 키와 같은 아래 `match` 를 지난다.
        // 목록은 여기서 **한 번** 센다 — 커서의 사실(`Ctx::leaf`)과 이동의 끝(`step`)이 같은 줄을 읽는다.
        let rows = self.rows();
        let ctx = self.key_ctx(&rows);
        let Some(act) = menu::feed(&mut self.chord, &ctx, k) else {
            // **기다리는 열도 아무 동작을 안 돈 키다**(리뷰) — `gg` 의 첫 `g`·`Ctrl-w`·메뉴를 여는
            // SPC 가 그것이고, 거기서 걷으면 메뉴의 Esc 를 고친 까닭이 메뉴 밖에 그대로 남는다.
            // 가르는 것은 [`keys::Chord::feed`] 가 남긴 열이다 — 뜻 없는 키만 열을 버리므로,
            // **모르는 키는 여태처럼 걷는다.** 메뉴를 닫는 Esc·SPC 도 열을 버려 `in_menu` 가 쥔다.
            if in_menu || !self.chord.held().is_empty() {
                self.notice = carried;
            }
            return;
        };
        // **되는지는 한 판정이 가른다**([`keys::Browse::enabled`]) — 키 바가 같은 판정으로
        // 적을 키를 고르므로 둘이 안 갈린다. 층에서 뜻이 없는 키는 왜 안 되는지를 한 줄로
        // 말한다. `n` 은 층에서도 듣는다 — 커서의 프로젝트를 담을 곳으로 박는다([`App::open_form`]).
        match act.enabled(&ctx) {
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
            // **끝에서는 제자리다** — 목록에서 `Ctrl-w h` 를 눌러도 상세로 돌지 않는다.
            B::Focus(side) => self.focus = self.focus.step(side),
            B::Step(m) => self.step(m, rows.len()),
            // **드나드는 키도 포커스를 탄다**(`enabled`). 상세를 읽다가 누른 Enter·←가 목록을
            // 옮기면 보던 이슈가 바뀌고 굴린 자리도 첫 줄로 돌아간다 — ↑↓ 를 포커스에
            // 태운 까닭과 같다. 상세에서는 아직 뜻이 없어 아무 일도 안 한다.
            B::Enter => self.enter(),
            B::Leave => self.leave(),
            // **목록은 위에서 한 번 센 것을 받는다**(moai-zrzo 와 같은 자리) — 저마다 `rows()` 를
            // 다시 부르던 때는 `l`·`h`·`Tab` 한 번이 이슈 전부를 훑고 정렬하는 일을 두 번 했다.
            // **머리줄에서 `l`·`→` 는 그 프로젝트를 펼친다**(moai-i0wd, 사용자 결정 2026-09-19).
            // 들어가는 것은 Enter 다 — 한눈 보기는 고르는 화면이고 파고드는 곳은 프로젝트 안이다.
            // 한때 층에서 `l` 은 들어가기였다(moai-9m2d) — 그때 층은 트리가 아니었다.
            B::Expand if self.head_at(&rows).is_some() => self.unfold(&rows, false),
            B::Expand => self.expand(&rows),
            // **접을 것이 없으면 부모를 접고, 부모도 없으면 나간다** — 키 표도 그렇게 켠다
            // (`Browse::enabled`). 손에 익은 `h` 가 뿌리에서만 말하고 멤버 줄에서 입을 다물면
            // 어느 쪽이 고장인지 모른다.
            // 머리줄의 `h`·`←` 는 그 프로젝트를 접는다 — 접힌 머리줄에서는 아무 일도 없다(층은
            // 맨 위라 나갈 데가 없다).
            B::Collapse if self.head_at(&rows).is_some() => self.fold(&rows),
            B::Collapse => {
                if !self.collapse(&rows) && !self.fold_parent(&rows) {
                    self.leave();
                }
            }
            // 머리줄의 `Tab` 은 그 프로젝트를 묶음까지 다 펼친다 — 펼쳐져 있으면 통째로 접는다.
            B::ExpandAll if self.head_at(&rows).is_some() => self.unfold(&rows, true),
            B::ExpandAll => self.expand_all(&rows),
            // **헤더의 번호로 바로 간다**(moai-o133). `0` 은 전체 — 층이다. 이미 그 자리면
            // 아무 일도 안 한다: 같은 프로젝트를 다시 열면 커서와 굴린 자리가 첫 줄로 튄다.
            // **건너뛰면 포커스는 목록으로 돌아온다**(리뷰 moai-i784.pzh). 상세에 포커스를 둔 채
            // `0` 을 누르면 층에 서는데, 층의 상세에는 듣는 키가 없어 Enter 가 조용하고 키 바도
            // 그 키를 안 적는다 — 무엇을 눌러야 할지 없는 화면이 선다. 층으로든 프로젝트로든
            // 건너뛰는 것은 목록을 보러 가는 일이다.
            B::Project(n) => {
                // **선 자리의 번호는 한 곳에서 읽는다**([`layer::Layer::number`]) — 헤더가 빛을
                // 세울 자리를 고르는 자와 같아야 한다. 따로 세면 헤더는 `<2>` 를 빛내는데 `2` 는
                // 그 프로젝트를 다시 열어 커서와 굴린 자리를 첫 줄로 튕긴다.
                let here = self.layer.as_ref().and_then(layer::Layer::number);
                match usize::from(n) {
                    at if here == Some(at) => {}
                    0 => self.climb(),
                    at => self.enter_project(at - 1),
                }
                // **간 자리를 보고 포커스를 돌린다**(리뷰 moai-i784.pzh, 그리고 이 리뷰).
                // 이미 그 자리였어도 돌린다 — 누른 사람은 목록을 보러 온 것이고, 층의 상세에는
                // 듣는 키가 없어 거기 남으면 무엇을 눌러야 할지 없는 화면이 선다. 거꾸로 **못
                // 들어갔으면 건드리지 않는다**: `enter_project` 는 실패하면 아무것도 안 바꾸고
                // 까닭만 알림으로 다는데, 포커스만 옮기면 읽던 상세가 키를 잃는다.
                if self.layer.as_ref().and_then(layer::Layer::number) == Some(usize::from(n)) {
                    self.focus = Pane::Explorer;
                }
            }
            B::Grep => {
                self.grep_was = Some((self.filter_text.clone(), self.grep_in, self.cursor));
                self.mode = Mode::Grep(Input::default(), GrepIn::All);
            }
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
            B::ClearFilter => {
                let (was, held) = (self.searching(), self.current_of(&rows).map(|r| self.anchor_of(&r)));
                self.clear_filter();
                self.after_search(was, held);
            }
            // 켜고 끄는 것은 **다시 읽는 것**이다. 겹친 줄은 적재 때 한 번 세는 것이라
            // (`Ground`·색인·경고), 들고 있는 것에 덧칠하면 셈이 옛 줄로 남는다.
            // **켰는데 겹칠 것이 없으면 한 번 말한다**(moai-d5vn). 경로 줄은 옆이 없으면 비므로, 말이
            // 없으면 메뉴만 닫힌 똑같은 화면이 남아 누른 키가 고장 난 줄 안다. 못 찾았으면 그 까닭을,
            // 찾았는데 비었으면 없다고 댄다. 알림이라 다음 키에 걷힌다. 읽기가 실패했으면 `trouble` 이
            // 이미 그 까닭을 대므로 겹쳐 말하지 않는다 — 그래서 옛 `unfound` 를 먼저 비운다.
            B::Worktree => {
                self.worktree = !self.worktree;
                self.site.unfound = None;
                self.reload();
                if self.worktree && self.trouble.is_none() && self.site.origin.labels().is_empty() {
                    let g = crate::style::BRANCH_GLYPH;
                    // 스냅샷만 없는 옆은 "없음" 이 아니다 — 그 이름으로 줄에 `⎇` 가 선다(`Origin::named_only`).
                    let named = self.site.origin.named_only();
                    let lang = self.site.lang;
                    self.notice = Some(match &self.site.unfound {
                        // **까닭이 이미 "못 찾았다" 로 시작한다**(리뷰) — `worktree::Trouble::Unfound`
                        // 를 편 글이 그 문장이라, 앞에 한 번 더 달면 같은 말이 두 번 선다.
                        Some(why) => format!("{g} {}", crate::text::one_line(why)),
                        None if !named.is_empty() => {
                            fill(say(lang, "tui.worktree.no_snapshot"), &[("glyph", g), ("names", &named.join(", "))])
                        }
                        None => fill(say(lang, "tui.worktree.none"), &[("glyph", g)]),
                    });
                }
            }
            B::Column(_) | B::Done | B::Deferred | B::ShowAll | B::Sort(_) => self.look(act, &rows),
            // 열은 줄을 더하거나 빼지 않는다 — 커서를 붙들 까닭이 없다.
            B::Cell(f) => {
                self.fields.toggle(f);
                self.save_look();
            }
            // 상세를 숨기면 **포커스를 목록으로 되돌린다** — 안 보이는 칸에 포커스가 남으면
            // 이동키가 어디에도 안 닿아 화면이 굳은 것으로 보인다(moai-ymnu).
            // **읽음은 시킬 때만 선다**(사용자 결정) — 커서가 지나갔다고, 상세를 봤다고 서지 않는다.
            B::Read | B::ReadAll | B::ReadGroup => self.mark_read(act, &rows),
            B::Detail => {
                self.detail_open = !self.detail_open;
                if !self.detail_open {
                    self.focus = Pane::Explorer;
                }
                self.save_look();
            }
            B::Raw => {
                self.raw = !self.raw;
                // 그린 것과 원문은 줄 수가 다르다. 굴린 자리를 들고 가면
                // 엉뚱한 데가 나온다.
                self.detail.rewind();
            }
        }
    }

    /// 키 표가 켜짐과 낱말을 가를 값. **여기서 잰다** — 표([`keys`])는 조각이라 `App` 을 모른다.
    ///
    /// **목록(`rows`)은 든 쪽이 센 것을 받는다.** 커서가 선 줄의 사실(잎인가)은 목록을 세야
    /// 나오는데, 세는 데 이슈 전부를 훑고 정렬한다 — 그림은 프레임마다 이미 한 번 센 것을
    /// 넘기고, 키 처리는 키 하나에 한 번 센다. 여기서 따로 세면 바·메뉴·뱃지가 각자 센다.
    pub fn key_ctx(&self, rows: &[Row]) -> keys::Ctx {
        // 칸 이름은 **한 번만** 모은다 — 번호 수와 숨김 비트가 같은 목록을 봐야 하고, 한눈 보기의
        // 이것은 줄을 낸 프로젝트를 도는 셈이다([`App::screen_statuses`]).
        let statuses = self.screen_statuses();
        keys::Ctx {
            lang: self.site.lang,
            layer: self.on_layer(),
            on_row: matches!(self.current_of(rows), Some(Row::Item(..))),
            rows_here: rows.iter().any(|r| matches!(r, Row::Item(..))),
            list_focus: self.focus == Pane::Explorer,
            // [`App::enter`] 가 무언가 하는 줄 — `..`(나가기)·디렉터리·층의 프로젝트.
            leaf: !matches!(
                self.current_of(rows),
                Some(Row::Up | Row::Item(_, Entry::Dir { .. }, _) | Row::Project(_))
            ),
            // [`App::leave`] 가 무언가 하는 자리 — 디렉터리 안뿐이다. 층으로는 `0` 이 간다(moai-i784).
            root: self.site.path.is_empty(),
            // [`App::expand`] 가 무언가 하는 줄 — 펼칠 수 있는 폴더뿐이다. `leaf` 로 가르던 때는
            // `..` 과 층의 프로젝트 줄에서 `l`·`Tab` 이 켜진 채 아무 일도 안 했다(리뷰).
            // **머리줄도 펼칠 수 있는 줄이다**(moai-i0wd) — 그 밑에 그 프로젝트의 줄이 선다.
            group: self.dir_at(rows).is_some() || self.head_at(rows).is_some(),
            // 커서의 줄이 **화면에** 펼쳐져 있는가 — 접기가 접을 것과 나갈 것을 여기서 가른다.
            // 검색이 저절로 연 것까지 센다([`App::open_at`]).
            expanded: self.open_at(rows).is_some() || self.head_open(rows),
            nested: self.parent_row(rows).is_some(),
            worktree: self.worktree,
            raw: self.raw,
            columns: statuses.len().min(keys::NUMBERED),
            projects: self.layer.as_ref().map_or(0, |l| l.places.len().min(keys::NUMBERED)),
            hidden: statuses
                .iter()
                .take(keys::NUMBERED)
                .enumerate()
                .filter(|(_, s)| self.view.hides(s))
                .fold(0, |bits, (n, _)| bits | 1 << n),
            done_hidden: self.view.hides(crate::config::DONE),
            deferred_hidden: self.view.hide_deferred,
            sorting: self.order,
            fields: self.fields,
            detail: self.detail_open,
            next_pane: self.focus.next().word(self.site.lang),
            prev_pane: self.focus.prev().word(self.site.lang),
            left_pane: self.focus.step(keys::Side::Left).word(self.site.lang),
            right_pane: self.focus.step(keys::Side::Right).word(self.site.lang),
        }
    }

    /// 이동 하나를 **포커스 있는 칸에** 준다. 두 칸이 같은 조각([`scroll`])으로
    /// 굴러 걸음(`PAGE`·`HALF`)도 끝의 뜻도 같다.
    ///
    /// **`j`·`k` 도 여기로 온다**(moai-ob4c). 한때 `j`·`k` 는 포커스와 상관없이 상세를
    /// 굴렸다 — 목록에 선 채 상세를 한 줄 굴리는 길이었다. vi 대로 포커스 칸을 움직이게
    /// 바꿨다(키 지도 moai-hudg): 같은 키가 칸마다 다른 칸을 움직이면 Tab 이 무엇을 바꾸는지
    /// 흐려진다. 상세는 Tab 으로 가서 굴린다.
    ///
    /// 상세의 끝(`End`)은 마지막으로 그린 줄 수로 잰다 — 줄 수는 폭에 달렸고 폭은
    /// 그려야 나온다. 루프는 키 하나마다 한 번 그리므로 그 수는 한 걸음 넘게 낡지 않는다.
    fn step(&mut self, m: Move, rows: usize) {
        match self.focus {
            Pane::Explorer => {
                let at = scroll::cursor(m, self.cursor, || rows);
                self.move_to(at);
            }
            Pane::Detail => self.detail.go(m),
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
            Mode::Grep(input, _) | Mode::Filter(input) => input.key(k),
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
            return self.live();
        }
        // 칸이 안 먹은 키는 표([`keys::PROMPT`])가 가른다 — Enter·Esc. **Ctrl 은 글자가
        // 아니다**: 칸이 안 먹은 Ctrl 조합(Ctrl-Enter 같은 것)은 표에 없어 아무 일도 하지
        // 않는다. Ctrl-C 는 [`App::key`] 가 이미 받았다.
        //
        // **Alt 는 옮기면서 바뀌었다.** 옛 `typing()` 은 Alt 를 안 보고 `Alt-b` 를
        // `b` 로, `Alt-Backspace` 를 한 글자 지우기로 먹었다. 칸은 Alt 조합을
        // 글자로 치지 않으므로(Alt 를 Meta 로 보내는 터미널에서 `b` 가 찍히는 것은
        // 사람이 친 것이 아니다) `Alt-b` 는 아무 일도 하지 않는다. `Alt-Backspace` 는
        // 칸이 Ctrl-W 처럼 낱말 하나를 지운다(moai-979m) — 여기까지 안 온다.
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
                // 붙든 줄은 **거르기 전**에 잰다 — 첨자가 옛 `keep` 을 가리킨다.
                let held = self.current().map(|r| self.anchor_of(&r));
                // 검색이었는지는 **칸을 열기 전**의 것까지 본다 — 검색 칸은 치는 대로 걸어(`live`) 지금
                // `filter_text` 가 이미 검색이다. 빈 글로 Enter 를 치면 그 검색이 풀린다.
                let was = self.searching()
                    || self.grep_was.as_ref().is_some_and(|(t, ..)| t.as_deref().is_some_and(|t| t.starts_with('/')));
                if self.apply(&mode).is_ok() {
                    self.mode = Mode::Browse;
                    self.grep_was = None;
                    // 자른 자리의 줄이 다른 것이면 상세도 첫 줄로(리뷰 moai-3lul.kt0) — 치는 대로 거르는
                    // 길(`live`)이 이미 그렇게 서고, 여기만 커서를 자르고 굴린 자리를 남겼다. **검색이 풀렸으면
                    // 커서는 `after_search` 가 세운다** — 번호로 먼저 자르면 붙든 줄이 그대로 서도 상세가 되감긴다.
                    if !self.after_search(was, held.clone()) {
                        self.settle(held, self.cursor);
                    }
                }
            }
            keys::Prompt::Cancel => {
                // 치는 대로 걸었던 것을 **열기 전으로** 되돌린다 — Esc 는 "안 한 것으로" 다.
                // **커서도 열기 전 자리로 간다** — 검색 칸에서는 커서를 못 옮기니 되돌린 목록은 열기 전 그 목록이고,
                // 그 번호가 열기 전 그 줄이다. 검색을 푸는 길(`after_search`)을 타면 치는 동안 커서 밑에 섰던
                // 줄로 가 "안 한 것" 이 아니게 되고, 되돌린 거름망(`f`)이 가린 줄을 두고 보기에 가렸다고 댄다.
                if let (Mode::Grep(..), Some((text, g, cursor))) = (&self.mode, self.grep_was.take()) {
                    let held = self.current().map(|r| self.anchor_of(&r));
                    self.filter_text = text;
                    self.grep_in = g;
                    self.reapply();
                    self.settle(held, cursor);
                }
                self.mode = Mode::Browse;
            }
            keys::Prompt::NextScope | keys::Prompt::PrevScope => {
                if let Mode::Grep(_, g) = &mut self.mode {
                    *g = if act == keys::Prompt::NextScope { g.next() } else { g.prev() };
                    self.live();
                }
            }
        }
    }

    /// 검색 칸이 **치는 대로 거른다**(moai-00le). 거름망(`f`) 칸은 안 한다 — 반쯤 친
    /// `status=in` 은 틀린 글이라, 치는 동안 목록이 비었다 찼다 한다.
    ///
    /// 커서는 줄 수 안으로만 당긴다 — Enter 로 걸 때와 같은 자다.
    fn live(&mut self) {
        let mode @ Mode::Grep(..) = self.mode.clone() else { return };
        let held = self.current().map(|r| self.anchor_of(&r));
        if self.apply(&mode).is_ok() {
            self.settle(held, self.cursor);
        }
    }

    /// 검색이 풀렸으면 커서를 **보기로 돌아간 목록**에 다시 세운다(moai-gwmc, 사용자 결정). `was` 는 풀기 전에
    /// 검색이 걸려 있었는가, `held` 는 그때 커서가 붙든 줄이다. 커서를 세웠으면 `true` — 아니면 부르는 쪽이
    /// 제 자로 세운다.
    ///
    /// - 붙든 줄이 그대로 서면 거기 선다 — 검색이 풀려 줄이 늘어도 보던 줄을 안 잃는다. 같은 줄이니 상세의
    ///   굴린 자리도 둔다(`stand` 는 정체로 가른다).
    /// - 보기가 도로 가렸으면 **보기를 안 건 차례에서 가장 가까운** 보이는 줄에 서고(아래 먼저), 가려졌다고
    ///   한 줄 댄다. 번호로 자르면 검색 때와 전혀 다른 자리의 줄에 선다. **보기가 가렸을 때만 댄다** — 검색
    ///   대신 건 거름망(`f`)이 가린 줄에 `SPC v a` 를 대면 누른 키가 아무것도 안 한다.
    /// - 검색 중 들어간 폴더가 보기에 가렸으면 보이는 폴더까지 나온다 — 가린 폴더 안에 남으면 "보기에 가려
    ///   비었다" 만 선 화면에서 왜 여기 있는지 모른다. 그때 붙들 줄은 나온 폴더다.
    ///
    /// **보기는 안 건드린다** — 드러내던 것은 검색이었지 보기가 아니다.
    /// 그 정체의 줄이 접힌 폴더 안에 들었을 때 **그 줄을 품은, 화면에 선 가장 깊은 폴더**.
    /// 자리는 [`crate::nav::Index::home_of`] 가 정한 그대로 안쪽부터 훑는다.
    fn folded_into(&self, rows: &[Row], want: &Anchor) -> Option<usize> {
        let Anchor::Issue(id) = want else { return None };
        let home = self.site.index.home_of(self.site.index.find(id)?);
        (1..=home.len()).rev().find_map(|depth| {
            let anchor = match &home[depth - 1] {
                Seg::Epic(id) | Seg::Milestone(Some(id)) | Seg::Issue(id) => Anchor::Issue(id.clone()),
                // 여기 오는 것은 지금 선 프로젝트의 줄뿐이다(위에서 `Anchor::Issue` 로 걸렀다).
                seg @ (Seg::Milestone(None) | Seg::Lost) => Anchor::Bucket(None, seg.clone()),
            };
            self.row_of(rows, &anchor)
        })
    }

    fn after_search(&mut self, was: bool, held: Option<Anchor>) -> bool {
        if !was || self.searching() || self.on_layer() {
            return false;
        }
        let mut want = held.clone();
        while let Some(seg) = self.site.path.last().cloned() {
            let parent: Path = self.site.path[..self.site.path.len() - 1].to_vec();
            let is_it = |e: &Entry| matches!(e, Entry::Dir { seg: s, .. } if *s == seg);
            if self.site.index.entries_where(&self.site.issues, &parent, &|at| self.visible(at)).iter().any(is_it) {
                break;
            }
            let dir = self.site.index.entries(&self.site.issues, &parent).into_iter().find(is_it);
            self.site.path.pop();
            self.site.remembered.pop();
            // 정체만 읽으므로 가지 모양은 뜻이 없다 — 이 줄은 화면에 안 선다.
            want = dir.map(|e| self.anchor_of(&Row::Item(Seat::Here, e, Twig::default())));
        }
        let Some(want) = want else { return false };
        let rows = self.rows();
        if let Some(row) = self.row_of(&rows, &want) {
            self.stand(&rows, row, held.as_ref());
            return true;
        }
        // 보기를 안 건 차례 — 거름망(`f`)은 그대로 건다. 검색이 풀린 뒤라 걸린 것은 거름망뿐이다.
        let all = self.rows_where(&|at| !self.site.veil(at).filtered);
        let near = self.row_of(&all, &want).and_then(|pos| {
            (1..all.len()).find_map(|d| {
                [pos.checked_add(d), pos.checked_sub(d)]
                    .into_iter()
                    .flatten()
                    .filter_map(|p| all.get(p))
                    .find_map(|r| self.row_of(&rows, &self.anchor_of(r)))
            })
        });
        // **검색이 저절로 연 폴더 안에 있던 줄은 접히면서 사라진다**(moai-i5io 리뷰) — 그때는 그
        // 줄을 품은 폴더에 선다. 그 줄은 `all` 에도 없어 위의 이웃 찾기가 못 잡는데, 번호로 세우면
        // 검색 때와 아무 상관 없는 줄에 서고 까닭을 댈 자리도 없다(`stand` 의 "정체로 가른다").
        let folded = near.is_none().then(|| self.folded_into(&rows, &want)).flatten();
        self.stand(&rows, near.or(folded).unwrap_or(self.cursor), None);
        if let Anchor::Issue(id) = &want
            && self.site.index.find(id).map(|at| self.site.veil(at)).is_some_and(|v| v.viewed && !v.filtered)
        {
            let show = keys::label(keys::BROWSE, keys::Browse::ShowAll);
            self.notice = Some(fill(say(self.site.lang, "tui.veiled_row"), &[("id", id), ("show", &show)]));
        }
        true
    }

    /// 거름망이 바뀐 뒤 커서를 `at` 을 줄 수 안으로 자른 자리에 세운다. **그 자리의 줄이
    /// 바뀌었으면 상세를 첫 줄로 되돌린다** — 치는 대로 거르면 같은 번호에 다른 이슈가
    /// 서는데, 굴린 자리가 남으면 그 이슈를 첫 줄부터 못 본다(`move_to` 와 같은 까닭).
    fn settle(&mut self, held: Option<Anchor>, at: usize) {
        let rows = self.rows();
        self.stand(&rows, at, held.as_ref());
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
        // 기다리던 `g` 는 버린다 — 붙인 뒤의 `g` 하나가 붙이기 전의 `g` 와 이어 맨 위로 뛰지 않게.
        self.chord.clear();
        // 층에서는 `/`·`f` 가 안 열린다 — **키 처리와 같은 판정**([`keys::Browse::enabled`])으로
        // 열리는 칸만 대고, 키 이름은 표에서 읽는다.
        let ctx = self.key_ctx(&self.rows());
        let open: Vec<String> = [keys::Browse::Grep, keys::Browse::Filter, keys::Browse::Jot]
            .into_iter()
            .filter(|a| a.enabled(&ctx).is_ok())
            .map(|a| format!("`{}`", keys::label(keys::BROWSE, a)))
            .collect();
        let open = open.join("·");
        match &mut self.mode {
            Mode::Browse => {
                self.notice = Some(fill(say(self.site.lang, "tui.paste.no_field"), &[("open", &open)]));
            }
            Mode::Grep(input, _) => {
                input.paste(s);
                self.live();
            }
            Mode::Filter(input) => input.paste(s),
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
                    let lang = self.site.lang;
                    ask.error = Some(fill(say(lang, "tui.ask.malformed"), &[("example", ask_example(lang))]));
                    return;
                }
            },
            keys::Prompt::Cancel => None,
            keys::Prompt::NextScope | keys::Prompt::PrevScope => return,
        };
        let Mode::Ask(ask) = std::mem::replace(&mut self.mode, Mode::Browse) else { return };
        self.mode = *ask.back;
        if let Some(who) = who {
            self.user = Some(crate::model::label(&who.name, Some(&who.email), crate::config::Naming::Full));
            // 받은 사람이 [NEW] 를 가를 사람이기도 하다(moai-j038.vna) — 헤더는 이 사람을 대는데 안 읽음이
            // 띄울 때의 "모름" 에 머물면 한 화면이 두 사람을 말하고, `SPC m a` 는 늘 "적을 것이 없다" 다.
            self.me = self.user.clone();
            self.recount_unread();
            (ask.then)(self);
        }
    }

    /// 생각 담기 폼의 키. **무엇을 할지는 폼이 정하고**([`Form::key`]) 여기는 그대로 한다.
    ///
    /// Ctrl-C 는 폼보다 먼저 [`App::key`] 가 받는다 — 어느 모드에서든 나가는 길이다. 적던 것은
    /// 그 길로 날아가지만, raw mode 에서 Ctrl-C 를 막으면 멈춘 화면에서 나갈 길이 없어진다.
    fn jot(&mut self, k: KeyEvent) {
        let lang = self.site.lang;
        let Mode::Idea(form) = &mut self.mode else { return };
        match form.key(k, lang) {
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
                Ok(f) => f
                    .status
                    .iter()
                    .find(|s| !crate::report::knows_column(&self.site.issues, &self.site.cfg, s))
                    .map(|s| crate::cmd::unknown_column(s, &self.site.cfg)),
            },
            _ => None,
        }
    }

    /// 들어간 디렉터리에서 커서가 설 줄 — **`..` 너머 첫 줄**(moai-cm13). 비었으면 `..`.
    ///
    /// `..` 에 세우면 들어가자마자 누른 Enter(·`l`) 한 번이 도로 나온다 — 두 번 누르면 들어갔다
    /// 나온 제자리다. 고르기 창(`Picker::new`)이 첫 하위 디렉터리에 서는 것과 같은 자다.
    /// `..` 은 `k`·`gg`·Home 한 번 거리다 — vi 키가 서기 전에는 `..` 에 세워 두는 것이 나가는
    /// 길을 보이는 값이었지만, 이제 `h`·Bksp 가 어느 줄에서든 나간다.
    ///
    /// **프로젝트 뿌리에서는 늘 0 이다** — `..` 은 디렉터리에만 서므로(moai-i784) 넘을 줄이
    /// 없다. 그래서 `enter_project` 는 이것을 안 부른다 — 줄을 비운 뒤 들이므로 붙들 정체가 없어
    /// 들이기(`apply_fresh`)가 이미 첫 줄에 세운다(moai-go4o).
    fn first_row(&self) -> usize {
        let rows = self.rows();
        usize::from(rows.len() > 1 && rows.first() == Some(&Row::Up))
    }

    fn enter(&mut self) {
        let rows = self.rows();
        match self.current_of(&rows) {
            Some(Row::Up) => self.leave(),
            // **자리는 그 줄의 것을 그대로 쓴다**([`App::dir_at`]) — 지금 자리에 제 마디만 이으면,
            // 펼쳐 든 줄(깊이 1 이상)에서 가운데 마디가 빠진 있지도 않은 자리가 서서 들어간 곳이
            // 텅 빈다(리뷰). 그러면 한 키 전에 보였던 멤버가 사라지고 빵조각도 거짓을 말한다.
            Some(Row::Item(..)) => {
                let Some((seat, path)) = self.dir_seat_at(&rows) else { return };
                // **남의 줄이면 그 프로젝트로 먼저 들어간다**(리뷰). 자리는 그 프로젝트의 색인이
                // 푼 것이라, 지금 선 프로젝트(한눈 보기에서는 줄이 빈 것)에 그대로 적으면 화면은
                // 그대로인데 `key_ctx` 의 `root` 만 뒤집혀 Bksp 가 없는 자리를 벗긴다.
                if let Seat::Place(n) = seat {
                    self.enter_project(n);
                    // 못 들어갔으면(못 읽는 프로젝트) 한눈 보기에 그대로 선다 — 자리를 적지 않는다.
                    if self.on_layer() {
                        return;
                    }
                    self.site.remembered = vec![0; path.len()];
                    self.site.path = path;
                    self.cursor = self.first_row();
                    self.detail.rewind();
                    return;
                }
                // 기억한 번호는 **층마다 하나**다(`remembered.len() == path.len()`). 펼쳐 든 줄로
                // 들어가면 층이 한 번에 여럿 깊어지므로, 지나친 층은 0 으로 메운다 — 그 층은
                // 들어간 적이 없어 기억할 자리가 없다(`land` 가 하는 것과 같다).
                self.site.remembered.push(self.cursor);
                self.site.remembered.resize(path.len(), 0);
                self.site.path = path;
                self.cursor = self.first_row();
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
        // **뿌리에서는 아무 일도 없다**(moai-i784). 층으로는 헤더의 `0` 으로 간다 — Bksp 가
        // 디렉터리와 프로젝트 층 두 군데로 가면 같은 키가 어디로 갈지 자리마다 달라진다.
        if self.site.path.is_empty() {
            return;
        }
        if let Some(from) = self.site.path.pop() {
            self.detail.rewind();
            let fallback = self.site.remembered.pop().unwrap_or(0);
            let rows = self.rows();
            self.cursor = rows
                .iter()
                .position(|r| matches!(r, Row::Item(_, Entry::Dir { seg, .. }, _) if *seg == from))
                .unwrap_or(fallback.min(rows.len().saturating_sub(1)));
        }
    }

    /// 지금 어디인가. 뿌리는 `/`.
    pub fn crumbs(&self) -> String {
        // 층이 걷힌 뒤로 이 자리는 **모든 프로젝트를 한 목록으로 보는 화면**이다(moai-3f1b) —
        // 옛 이름(`프로젝트 층`)은 그 밑에 줄이 서지 않던 때의 것이다.
        if self.on_layer() {
            return say(self.site.lang, "tui.crumbs.all_projects").into();
        }
        if self.site.path.is_empty() {
            return "/".into();
        }
        let mut out = String::new();
        for seg in &self.site.path {
            out.push('/');
            out.push_str(&self.seg_label(seg));
        }
        out
    }

    /// 바구니 이름은 [`crate::nav::Index::label`] 과 **같은 키에서** 온다(moai-ra67) — 글자로
    /// 두 벌 들던 판은 한쪽만 고쳐도 경로 줄과 목록 줄이 다른 말을 했다.
    fn seg_label(&self, seg: &Seg) -> String {
        let id = match seg {
            Seg::Milestone(None) => return crate::i18n::say(self.site.lang, "nav.no_milestone").into(),
            Seg::Lost => return crate::i18n::say(self.site.lang, "nav.lost").into(),
            Seg::Milestone(Some(id)) | Seg::Epic(id) | Seg::Issue(id) => id,
        };
        self.site.title_of(id)
    }
}

impl App {
    /// 편집기가 돌려준 것을 받는다(moai-08af). `got` 은 파일의 글이거나, 담지 않을 까닭
    /// (편집기가 0 이 아닌 코드로 끝났다·못 띄웠다·못 읽었다)이다.
    ///
    /// **담는 길은 안 폼과 한 길이다.** 받은 제목·본문으로 폼을 세우고 [`save_idea`] 를
    /// 부른다 — 박힌 곳에 서기(moai-fccv)·`write`·누구냐 묻고 다시 부르기(moai-nmv2)·쓴 뒤
    /// 커서와 알림(moai-064q)이 전부 같다. 그래서 **담기가 실패하면 적은 글이 폼에 열린 채
    /// 남는다** — 까닭은 배너에 서고, 고치고 다시 담거나 Esc 로 버린다. 적은 것이 임시
    /// 파일과 함께 사라지지 않는다.
    ///
    /// 까닭이 왔거나 제목이 비면 **아무것도 안 쓰고** 한 줄 알린다. 폼도 안 연다 — 편집기를
    /// 오류로 끝낸 것(vim 의 `:cq`)과 비워 닫은 것은 그만두겠다는 뜻이다(git 과 같다).
    pub fn edited(&mut self, into: Option<form::Target>, got: Result<String, String>) {
        let text = match got {
            Ok(text) => text,
            Err(why) => {
                let why = crate::text::one_line(&why);
                self.notice = Some(fill(say(self.site.lang, "tui.jot.not_kept"), &[("why", &why)]));
                return;
            }
        };
        let Some((title, body)) = jotfile::parse(&text) else {
            self.notice = Some(say(self.site.lang, "tui.jot.no_title").into());
            return;
        };
        let mut form = Form::new(into);
        form.title = Input::new(&title);
        if let Some(body) = &body {
            form.body = edit::Editor::new(body);
        }
        self.mode = Mode::Idea(form);
        save_idea(self);
    }

    /// **아직 안 담긴 글** — (제목, 본문). 루프가 오류로 끝나며 버릴 뻔한 것을 남기는 쪽이 묻는다
    /// (moai-y3r7). 폼이 열려 있거나(담기 실패·적는 중), 누구냐 묻는 칸 뒤에 폼이 서 있을 때다.
    /// **빈칸뿐인 폼은 없다** — 잃을 것이 없다([`Form::is_blank`]). 제목이 비고 본문만 있어도
    /// 적은 것이라 낸다.
    pub fn unsaved(&self) -> Option<(String, Option<String>)> {
        let form = match &self.mode {
            Mode::Idea(form) => form,
            Mode::Ask(ask) => match ask.back.as_ref() {
                Mode::Idea(form) => form,
                _ => return None,
            },
            _ => return None,
        };
        (!form.is_blank()).then(|| (form.title(), form.body()))
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
    // 선 곳이 다르면 쓰지 않는다. `write` 는 `self.site.repo` 에 쓰므로 여기를 지나면 머리에 보인
    // 곳이 곧 쓰는 곳이다 — 묻고 이어진 쓰기도 이 함수를 다시 지난다.
    if !app.stand_at(into.as_ref()) {
        return;
    }
    let at = crate::model::now();
    let kept = say(app.site.lang, "tui.jot.kept");
    let wrote = app.write(save_idea, move |issues, cfg, reserved, by| {
        let id = crate::store::new_id(issues, cfg, reserved, None, &title);
        let mut idea = Issue::new(id, title, Kind::Idea, Status::new(cfg.first_status()), &at);
        (idea.assignee, idea.assignee_email) = by.as_assignee();
        idea.body = body;
        let (entry, made) = crate::store::admit(issues, cfg, idea, by)?;
        Ok((vec![entry], Touched { id: made.id, done: kept }))
    });
    if wrote.is_some() {
        app.mode = Mode::Browse;
    }
}

/// 누군지 묻는 칸이 보이는 본보기. 거절문이 댄다.
pub fn ask_example(lang: crate::i18n::Lang) -> &'static str {
    say(lang, "tui.ask.example")
}

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

/// 스레드에서 짓는 읽기(다시 읽기·층)를 **끝날 때까지** 받는다. 루프가 하는 것을 흉내 낸다 —
/// 시험 셋(여기·층·등록)이 같은 것을 저마다 적던 자리다(리뷰 moai-3lul.kt0).
#[cfg(test)]
fn settle_reads(a: &mut App) {
    a.follow();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while a.loading() {
        assert!(std::time::Instant::now() < until, "읽기가 끝나지 않는다");
        std::thread::sleep(std::time::Duration::from_millis(2));
        a.follow();
    }
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
    use crate::model::{Kind, Status};
    use crate::scratch::Scratch;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

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

    impl App {
        /// 사람이 적는 키 이름의 열을 누른다 — `"SPC v w"`. 표의 이름과 같은 글로 시험을 적는다.
        pub fn hit(&mut self, names: &str) {
            for k in keys::parse_seq(names).unwrap_or_else(|| panic!("`{names}` 를 키로 못 푼다")) {
                self.key(k);
            }
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn row_ids(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| if let Row::Item(Seat::Here, e, _) = r { e.at() } else { None })
            .map(|at| a.site.issues[at].id.clone())
            .collect()
    }

    /// **처음에는 done 을 숨기고 `SPC v` 가 칸·미룸을 켜고 끈다**(moai-fmv5). 보기는 거름망이
    /// 아니라 Esc 가 안 푼다. 끝난 멤버가 있어도 안 끝난 멤버가 있는 에픽은 선다. 토글 뒤에도
    /// 커서는 보던 줄에 붙는다.
    #[test]
    fn done_starts_hidden_and_the_view_menu_brings_it_back() {
        let mut done_member = member("argos-0003", "argos-0001");
        done_member.status = Status::new("done");
        let mut loose_done = make("argos-0009", Kind::Issue);
        loose_done.status = Status::new("done");
        let mut put_off = make("argos-0010", Kind::Issue);
        put_off.deferred_at = Some("2026-09-02T00:00:00Z".into());
        let issues =
            vec![make("argos-0001", Kind::Epic), done_member, member("argos-0004", "argos-0001"), loose_done, put_off];
        let mut a = App::new(issues, cfg(), Path::new());

        assert_eq!(row_ids(&a), ["argos-0001", "argos-0010"], "done 이 처음부터 보인다");
        a.hit("Enter");
        assert_eq!(row_ids(&a), ["argos-0004"], "에픽 안의 끝난 멤버가 보인다");
        a.hit("Bksp");

        a.cursor = 1;
        a.hit("SPC v d Esc");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0009", "argos-0010"]);
        assert_eq!(row_ids(&a)[a.cursor], "argos-0010", "토글이 커서를 딴 줄로 옮겼다");
        a.hit("Esc");
        assert_eq!(row_ids(&a).len(), 3, "Esc 가 보기를 풀었다");

        a.hit("SPC v l Esc");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0009"], "미룸이 안 숨었다");
        // 설정의 넷째 칸이 done 이다 — 번호로 누른 것과 `d` 가 같은 칸을 만진다.
        a.hit("SPC v 4 Esc");
        assert_eq!(row_ids(&a), ["argos-0001"]);
        a.hit("SPC v a");
        assert_eq!(row_ids(&a).len(), 3, "모두 보이기가 다 안 보인다");
    }

    /// 보기가 done 과 미룬 줄을 숨긴 목록 — 검색 시험의 바닥. 뿌리에 보이는 것은 0001(에픽)·0011 뿐이고,
    /// 0002(끝난 에픽, 멤버 0005)·0009(done)·0010(미룸)이 숨는다.
    fn veiled_app() -> App {
        let mut done_loose = make("argos-0009", Kind::Issue);
        done_loose.status = Status::new("done");
        let mut put_off = make("argos-0010", Kind::Issue);
        put_off.deferred_at = Some("2026-09-02T00:00:00Z".into());
        let mut done_epic = make("argos-0002", Kind::Epic);
        done_epic.status = Status::new("done");
        let mut done_member = member("argos-0005", "argos-0002");
        done_member.status = Status::new("done");
        let issues = vec![
            make("argos-0001", Kind::Epic),
            member("argos-0003", "argos-0001"),
            done_epic,
            done_member,
            done_loose,
            put_off,
            make("argos-0011", Kind::Issue),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        a.hit("SPC v l Esc");
        a
    }

    fn search(a: &mut App, q: &str) {
        a.hit("/");
        for c in q.chars() {
            a.key(key(KeyCode::Char(c)));
        }
    }

    /// **검색은 보기가 숨긴 칸과 미룬 줄까지 세운다**(moai-wkb8, 사용자 결정). Enter 로 칸을 닫아도 검색이
    /// 걸린 동안은 그대로고, 풀면 설정된 보기로 돌아간다 — **보기는 바뀌지도 적히지도 않는다.**
    #[test]
    fn a_search_sees_past_the_view_and_leaves_it_as_it_was() {
        let mut a = veiled_app();
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0011"], "시험의 전제 — done·미룸이 숨었다");
        let (view, look) = (a.view.clone(), a.look_now());

        search(&mut a, "argos-00");
        let ids = row_ids(&a);
        for want in ["argos-0002", "argos-0009", "argos-0010"] {
            assert!(ids.iter().any(|i| i == want), "검색이 보기가 숨긴 {want} 를 못 찾았다: {ids:?}");
        }
        a.hit("Enter");
        assert_eq!(row_ids(&a), ids, "Enter 로 칸을 닫자 보기가 도로 가렸다");
        assert_eq!((&a.view, a.look_now()), (&view, look.clone()), "검색이 보기를 바꿨다");

        a.hit("Esc");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0011"], "검색을 풀었는데 보기로 안 돌아갔다");
        assert_eq!((&a.view, a.look_now()), (&view, look), "검색을 푼 뒤 보기가 달라졌다");

        // 거름망(`f`)은 찾는 물음이 아니라 보기를 따른다.
        a.hit("SPC f");
        for c in "status=done".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.hit("Enter");
        assert!(row_ids(&a).is_empty(), "거름망이 보기가 숨긴 줄까지 세웠다: {:?}", row_ids(&a));
    }

    /// **검색을 풀 때 커서가 선 줄이 보기에 도로 가리면 가장 가까운 보이는 줄로 가고 한 줄 댄다**(moai-gwmc,
    /// 사용자 결정). 가리지 않으면 그 줄을 붙든다 — 번호로 자르면 검색 때와 다른 줄에 선다.
    #[test]
    fn leaving_a_search_puts_the_cursor_next_to_the_row_the_view_hides_again() {
        let mut a = veiled_app();
        search(&mut a, "argos-0011");
        a.hit("Enter");
        a.hit("Esc");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0011", "보이는 줄을 붙들지 못했다");
        assert_eq!(a.notice, None);

        search(&mut a, "argos-0009");
        a.hit("Enter");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0009");
        a.hit("Esc");
        let ids = row_ids(&a);
        assert!(!ids.contains(&"argos-0009".to_string()));
        // 보기를 안 건 차례에서 0009 의 이웃 — 0010(미룸)은 숨었으니 그 너머 0011.
        assert_eq!(ids[a.cursor], "argos-0011", "{ids:?}");
        let told = a.notice.clone().unwrap_or_default();
        assert!(told.contains("argos-0009") && told.contains("SPC v a"), "{told:?}");
        assert_eq!(a.view.hidden, vec!["done".to_string()], "커서를 옮기려고 보기를 풀었다");
    }

    /// **검색 중 들어간 숨은 폴더는 검색을 풀면 나온다**(moai-gwmc) — 가린 폴더 안에 남으면 "보기에 가려
    /// 비었다" 만 선 화면에서 왜 거기 있는지 모른다.
    #[test]
    fn leaving_a_search_climbs_out_of_a_folder_the_view_hides() {
        let mut a = veiled_app();
        search(&mut a, "argos-0005");
        a.hit("Enter");
        // 숨은 에픽이 폴더로 서고, 검색이 맞힌 멤버가 그 밑에 딸려 선다(moai-i5io).
        assert_eq!(row_ids(&a), ["argos-0002", "argos-0005"], "시험의 전제");
        a.hit("Enter");
        assert_eq!(row_ids(&a), ["argos-0005"]);
        a.hit("Esc");
        assert!(a.site.path.is_empty(), "숨은 폴더 안에 남았다: {:?}", a.site.path);
        assert!(a.notice.clone().unwrap_or_default().contains("argos-0002"), "{:?}", a.notice);
        assert!(a.cursor < a.rows().len(), "커서가 목록 밖에 섰다");
    }

    /// **검색 칸의 Esc 는 커서도 열기 전 자리로 돌린다**(moai-qnkn 에픽 리뷰) — 칸에서는 커서를 못 옮기니 되돌린
    /// 목록의 그 번호가 열기 전 그 줄이다. 검색을 푸는 길을 타면 치는 동안 커서 밑에 섰던 줄로 간다.
    #[test]
    fn cancelling_a_search_puts_the_cursor_back_where_it_was() {
        let mut a = veiled_app();
        a.hit("j");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0011", "시험의 전제");
        search(&mut a, "argos-0001");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0001", "시험의 전제 — 치는 동안 커서가 다른 줄에 섰다");
        a.key(key(KeyCode::Esc));
        assert_eq!(row_ids(&a)[a.cursor], "argos-0011", "Esc 가 커서를 열기 전으로 못 돌렸다");
        assert_eq!(a.notice, None);
    }

    /// **검색을 풀어도 붙든 줄이 그대로 서면 상세의 굴린 자리도 둔다**(moai-qnkn 에픽 리뷰) — 같은 줄인데 번호가
    /// 달라졌다고 되감으면 읽던 자리를 잃는다. 커서를 다시 세우는 법은 정체로 가른다(`stand`). Esc 로 풀든
    /// 빈 검색 칸의 Enter 로 풀든 같다.
    #[test]
    fn leaving_a_search_keeps_the_detail_where_it_was_on_the_same_row() {
        for leave in ["Esc", "/ Enter"] {
            let mut a = veiled_app();
            search(&mut a, "argos-0011");
            a.hit("Enter");
            drawn(&mut a, 10, 40);
            a.hit("Ctrl-w w");
            for _ in 0..3 {
                a.key(key(KeyCode::Down));
            }
            a.hit("Ctrl-w w");
            assert_eq!(a.detail.offset(), 3, "시험의 전제");
            a.hit(leave);
            assert!(!a.searching(), "{leave}: 검색이 안 풀렸다");
            assert_eq!(row_ids(&a)[a.cursor], "argos-0011", "{leave}");
            assert_eq!(a.detail.offset(), 3, "{leave}: 같은 줄인데 상세를 되감았다");
        }
    }

    /// **검색 대신 건 거름망이 가린 줄을 보기에 가렸다고 대지 않는다**(moai-qnkn 에픽 리뷰) — `SPC v a` 를 눌러도
    /// 거름망에 여전히 가려 누른 키가 아무것도 안 한다.
    #[test]
    fn a_filter_replacing_a_search_does_not_blame_the_view() {
        let mut a = veiled_app();
        search(&mut a, "argos-0011");
        a.hit("Enter");
        a.hit("SPC f");
        for c in "type=epic".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.hit("Enter");
        assert_eq!(row_ids(&a), ["argos-0001"]);
        assert_eq!(a.notice, None, "거름망이 가린 줄을 보기 탓으로 댔다");
    }

    /// **검색 칸의 `숨김` 셈은 목록의 `숨김` 과 같은 자로 센다**(moai-qnkn 에픽 리뷰). 끝난 부모의 제 줄은 보기가
    /// 숨기지만 안 끝난 자식이 있어 검색을 풀어도 폴더로 선다 — 셈에 넣으면 `숨김 1건` 인데 줄에는 없다.
    #[test]
    fn the_unveiled_count_agrees_with_the_marked_rows() {
        let mut parent = make("argos-0020", Kind::Issue);
        parent.status = Status::new("done");
        let issues = vec![make("argos-0001", Kind::Epic), parent, make("argos-0020.a1b", Kind::Issue)];
        let mut a = App::new(issues, cfg(), Path::new());
        search(&mut a, "argos-0020");
        // 끝난 부모가 폴더로 서고, id 가 그 id 로 시작하는 자식도 검색에 걸려 그 밑에 선다.
        assert_eq!(row_ids(&a), ["argos-0020", "argos-0020.a1b"], "시험의 전제");
        let marked = a.rows().iter().filter(|r| matches!(r, Row::Item(Seat::Here, e, _) if a.unveiled(e))).count();
        assert_eq!((marked, a.unveiled_count()), (0, 0));
    }

    /// **검색 중에 쓴 줄이 검색에도 보기에도 가리면 둘 다 댄다**(moai-qnkn 에픽 리뷰). 검색이 보기를 걷는 동안에도
    /// Esc 는 검색만 풀고, 풀자마자 보기가 그 줄을 도로 가린다 — "Esc 로 푼다" 만 대면 누른 키가 아무것도 안 한다.
    #[test]
    fn a_write_hidden_by_a_search_and_the_view_names_both() {
        let (_scratch, mut a) = writable("land-search-view");
        a.hit("SPC v 1 Esc");
        a.hit("/");
        typed(&mut a, "argos-0001");
        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert_eq!(
            a.notice.as_deref(),
            Some("✓ 담김 · argos-0002 — 거름망과 보기에 가려 안 보인다 · Esc 로 풀고 SPC v a 로 모두 보인다")
        );
    }

    /// **옛 설정에도 새 열이 뜬다**(moai-3fnf 리뷰, 사용자 결정) — `fields` 에 안 적힌 것이 "껐다" 인지
    /// "그 열을 몰랐다" 인지는 `fields_known` 이 가른다. 끈 열은 적히고 나면 끈 채로 남는다.
    #[test]
    fn a_column_the_old_config_never_knew_comes_up_on_its_default() {
        let old = crate::user_config::Look {
            fields: Some(vec!["id".into(), "priority".into()]),
            ..crate::user_config::Look::default()
        };
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.adopt_look(&old, Vec::new());
        assert!(a.fields.shows(view::Field::Names), "옛 설정을 가진 사람에게 열 이름 줄이 안 떴다");
        assert!(a.fields.shows(view::Field::Branch), "⎇ 도 마찬가지다");
        assert!(!a.fields.shows(view::Field::Tally), "옛 설정이 끈 열이 되살아났다");

        // 이 바이너리가 적은 설정은 끈 것을 끈 채로 들고 온다.
        let now = a.look_now();
        assert_eq!(now.fields_known.as_ref().map(Vec::len), Some(view::Field::ALL.len()));
        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.hit("SPC c h Esc");
        let off = b.look_now();
        let mut c = App::new(Vec::new(), cfg(), Path::new());
        c.adopt_look(&off, Vec::new());
        assert!(!c.fields.shows(view::Field::Names), "끈 열이 다음 실행에 되살아났다");
    }

    /// 손으로 적은 빈 `fields_known` 이 내야 할 열(moai-4gy5) — 얼린 아홉([`view::Field::EMPTY_KNOWN`])
    /// 안은 `fields` 에 적힌 것만 켜지고, 밖은 나중에 생긴 열이라 기본값으로 선다.
    ///
    /// **얼린 목록에서 읽는다**(moai-8mq9.p38 리뷰) — 이름을 손으로 적어 두면 열을 더하는 날 이 셋을
    /// 쓰는 시험이 그 열을 빠뜨린 채 붉어지고, 붉은 줄의 글("적힌 fields 를 안 따랐다")이 다음 사람에게
    /// 그 열을 `EMPTY_KNOWN` 에 넣으라고 읽힌다 — 그것이 바로 그 상수가 금지하는 한 줄이다.
    fn empty_known_shows(f: view::Field, on: &[view::Field]) -> bool {
        if view::Field::EMPTY_KNOWN.contains(&f) { on.contains(&f) } else { view::Fields::default().shows(f) }
    }

    /// **빈 `fields_known` 은 "모든 열을 알았다" 다**(moai-4qkj, 사용자 결정 2026-09-18). 이 바이너리는
    /// 그런 설정을 안 쓰니 손으로 적은 것이고, 그 사람이 적은 `fields` 가 곧 켠 열이다. 한때 모든 열이
    /// "몰랐던 열" 이 되어 기본값이 서고 적힌 `fields` 는 한마디 없이 버려졌다. 경고는 안 낸다: 읽기는
    /// 관대하다.
    ///
    /// **그 "모든" 은 얼린 아홉 열이다**(moai-4gy5, 사용자 결정 2026-09-18) — [`view::Field::EMPTY_KNOWN`]
    /// 밖의 열은 나중에 생긴 것이라 기본값으로 선다([`empty_known_shows`]). 오늘은 두 목록이 같아 그
    /// 둘째 갈래가 빈 채로 서지만, 열을 더하는 날 여기가 그것을 잰다 — 그날 이 줄이 붉어지면 고칠 곳은
    /// `EMPTY_KNOWN` 이 아니라 이 시험이 든 기대다.
    #[test]
    fn an_empty_fields_known_takes_the_written_fields_as_they_are() {
        let hand = crate::user_config::Look {
            fields: Some(vec!["id".into()]),
            fields_known: Some(Vec::new()),
            ..crate::user_config::Look::default()
        };
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.adopt_look(&hand, Vec::new());
        for f in view::Field::ALL {
            let want = empty_known_shows(f, &[view::Field::Id]);
            assert_eq!(a.fields.shows(f), want, "{} 이 빈 fields_known 을 잘못 읽었다", f.name());
        }
        assert_eq!(a.notice, None, "관대히 읽을 자리에서 잔소리를 했다");
    }

    /// **`fields` 에 적힌 열은 `fields_known` 에 없어도 켜진다**(moai-svvk 에픽 리뷰) — 적은 쪽이 그
    /// 열을 알았다는 뜻이다. `fields_known` 은 안 적힌 열만 가른다: 목록에 없고 `fields` 에도 없는 열은
    /// 기본값으로 선다. 한때는 적힌 tags 가 "몰랐던 열" 의 기본값(꺼짐)으로 서 말없이 버려졌다.
    #[test]
    fn a_column_written_in_fields_is_on_even_when_fields_known_misses_it() {
        let hand = crate::user_config::Look {
            fields: Some(vec!["id".into(), "tags".into()]),
            fields_known: Some(vec!["id".into(), "assignee".into()]),
            ..crate::user_config::Look::default()
        };
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.adopt_look(&hand, Vec::new());
        assert!(a.fields.shows(view::Field::Tags), "fields 에 적힌 열이 fields_known 에 없다고 버려졌다");
        assert!(a.fields.shows(view::Field::Id));
        assert!(!a.fields.shows(view::Field::Assignee), "알던 열인데 안 적힌 것이 켜졌다");
        assert!(a.fields.shows(view::Field::Priority), "몰랐던 열이 기본값으로 안 섰다");
        assert_eq!(a.notice, None);
    }

    /// **손으로 적은 빈 `fields_known` 은 파일을 한 바퀴 돌고도 적힌 열 그대로다**(moai-svvk 에픽 리뷰).
    /// 첫 토글이 빈 목록에 이 바이너리가 아는 이름을 다 더해 적는다 — 그 뒤의 실행도 켠 열·끈 열이
    /// 그대로여야 한다. 기억 속 `Look` 만 견주는 시험은 `look_words` 가 `[]` 를 어떻게 읽는지도,
    /// `merge_words` 가 그것을 어떻게 다시 적는지도 안 지난다.
    #[test]
    fn a_hand_written_empty_fields_known_survives_a_trip_through_the_file() {
        let s = scratch("fields-known-empty");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\nfields = [\"id\"]\nfields_known = []\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        for f in view::Field::ALL {
            let want = empty_known_shows(f, &[view::Field::Id]);
            assert_eq!(a.fields.shows(f), want, "{} 이 적힌 fields 를 안 따랐다", f.name());
        }
        a.hit("SPC c t Esc");

        let text = std::fs::read_to_string(&user).unwrap();
        let (back, _) = crate::user_config::read_look(Some(&user));
        assert_eq!(
            back.fields_known.map(|k| k.len()),
            Some(view::Field::ALL.len()),
            "빈 목록을 이름으로 안 채웠다\n{text}"
        );
        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.user_config = Some(user);
        b.load_look();
        // **다음 실행이 본 열은 이번 실행이 둔 열이다** — 기대를 이름으로 적지 않는다(moai-8mq9.p38 리뷰).
        // 적어 두면 열을 더하는 날 그 열이 빠진 채 붉어지고, 붉은 줄이 다음 사람을 `EMPTY_KNOWN` 으로
        // 보낸다. 이 시험이 잴 것은 "무엇이 켜지나" 가 아니라 "한 바퀴 돌고도 그대로인가" 다.
        assert!(a.fields.shows(view::Field::Id) && a.fields.shows(view::Field::Tags), "토글이 안 섰다\n{text}");
        for f in view::Field::ALL {
            assert_eq!(b.fields.shows(f), a.fields.shows(f), "{} 이 다음 실행에 달라졌다\n{text}", f.name());
        }
        assert_eq!(b.notice, None, "제가 적은 설정을 읽으며 말이 섰다\n{text}");
    }

    /// **새 바이너리가 적은 열 이름은 오타가 아니다**(moai-zwam). 같은 설정 파일을 옛 바이너리와
    /// 새 바이너리가 번갈아 만지면, 옛 쪽은 `fields` 의 새 이름을 제가 모른다는 이유로 오타로 읽어
    /// 띄울 때마다 잔소리를 했다 — 게다가 고칠 길이 없다: 쓰기 경로(`Doc::merge_look`)는 그 이름을
    /// **일부러 지킨다**(`App::adopt_look` 이 `saved.fields` 에서 걸러 둔다). 읽는 쪽만 오타로 본 것이다.
    ///
    /// 가르는 자는 `fields_known` 이다 — 거기 적혔다는 것은 **적은 쪽이 그 열을 알았다**는 뜻이라
    /// 오타일 수 없다. 목록에 없는 이름은 그대로 잔소리한다(손으로 낸 오타가 그것이다).
    #[test]
    fn a_column_a_newer_binary_wrote_is_not_nagged_as_a_typo() {
        let known: Vec<String> =
            view::Field::ALL.into_iter().map(|f| f.name().to_string()).chain(["futurecol".into()]).collect();
        let newer = crate::user_config::Look {
            fields: Some(vec!["id".into(), "futurecol".into()]),
            fields_known: Some(known),
            ..crate::user_config::Look::default()
        };
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.adopt_look(&newer, Vec::new());
        assert_eq!(a.notice, None, "새 바이너리가 적은 열을 오타로 읽었다 — {:?}", a.notice);
        assert!(a.fields.shows(view::Field::Id), "적힌 열이 안 켜졌다");

        // 손으로 낸 오타는 그대로 잔소리한다 — 아무도 그 이름을 안다고 적지 않았다.
        let typo = crate::user_config::Look {
            fields: Some(vec!["id".into(), "priorty".into()]),
            ..crate::user_config::Look::default()
        };
        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.adopt_look(&typo, Vec::new());
        assert!(b.notice.as_deref().is_some_and(|n| n.contains("priorty")), "오타를 그냥 지나갔다 — {:?}", b.notice);
    }

    /// **옛 바이너리가 한 번 만져도 새 열 이름이 안 사라진다**(moai-zwam) — 매 저장이 파일 전체를
    /// 다시 쓰는 길이라, 모르는 이름을 안 지키면 새 바이너리가 켜 둔 열이 옛 쪽의 토글 한 번에
    /// 지워진다. `moai-zwam` 이 잔소리를 걷은 뒤에도 그 지킴은 그대로여야 한다.
    #[test]
    fn an_older_binary_keeps_the_column_names_it_does_not_know() {
        let s = scratch("fields-newer-binary");
        let user = s.join("user.toml");
        let known: Vec<String> =
            view::Field::ALL.into_iter().map(|f| f.name().to_string()).chain(["futurecol".into()]).collect();
        let quoted = known.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ");
        std::fs::write(&user, format!("[tui]\nfields = [\"id\", \"futurecol\"]\nfields_known = [{quoted}]\n")).unwrap();

        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        assert_eq!(a.notice, None, "제가 모르는 열을 오타로 읽었다 — {:?}", a.notice);
        a.hit("SPC c t Esc");

        let text = std::fs::read_to_string(&user).unwrap();
        let (back, problems) = crate::user_config::read_look(Some(&user));
        assert!(problems.is_empty(), "{problems:?}\n{text}");
        let (fields, known) = (back.fields.unwrap_or_default(), back.fields_known.unwrap_or_default());
        assert!(fields.iter().any(|w| w == "futurecol"), "켜 둔 새 열을 지웠다\n{text}");
        assert!(known.iter().any(|w| w == "futurecol"), "아는 열 목록에서 새 열을 지웠다\n{text}");
        assert!(fields.iter().any(|w| w == "tags"), "이쪽이 켠 열을 안 적었다\n{text}");
    }

    /// **손으로 적은 표 모양의 거절은 한 번만 말한다**(moai-fdq2). 건너뛴 키를 `App::saved` 에 든
    /// 것으로 옮겨 다음 저장이 그 차이를 안 싣는 것이 그 길인데(moai-jr3z), `fields_known` 만은 깃발이
    /// `base != new` 가 아니라 **늘 참**이라 그 길에서 샜다 — 토글마다 같은 알림이 서서 사람이 방금
    /// 띄운 말을 덮었다. 파일의 손으로 적은 모양은 그대로다.
    #[test]
    fn a_refused_fields_known_is_told_once_not_on_every_toggle() {
        let s = scratch("fields-known-refused");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\nfields_known = { id = true }\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();

        a.hit("SPC c t Esc");
        let first = a.notice.take();
        assert!(first.as_deref().is_some_and(|n| n.contains("fields_known")), "첫 거절을 안 말했다 — {first:?}");

        a.hit("SPC c t Esc");
        assert_eq!(a.notice, None, "같은 거절이 토글마다 다시 선다 — {:?}", a.notice);
        a.hit("SPC c a Esc");
        assert_eq!(a.notice, None, "다른 키를 토글해도 같은 거절이 선다 — {:?}", a.notice);

        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("fields_known = { id = true }"), "손으로 적은 모양을 덮었다\n{text}");
        assert!(text.contains("\"assignee\""), "거절된 키 하나가 다른 키의 저장을 막았다\n{text}");
    }

    /// **파일에서 `fields_known` 이 사라지면 다음 저장이 다시 적는다**(리뷰) — 적을 것을 재는 깃발은
    /// *이 세션이 적을 것이 남았는가* 이고(moai-fdq2), 그것만으로는 **파일이 밑에서 바뀐 판**을 못 본다.
    /// 한 번 적고 나면 세션 내내 같은 값이라, 그사이 누가 설정을 갈아 끼우면 다음 토글이 `fields` 만
    /// 적고 이 키는 빼놓는다 — 다음 실행이 그 빈자리를 옛 어휘([`view::Field::BEFORE_KNOWN`])로 읽어
    /// 사람이 끈 열을 도로 켠다.
    #[test]
    fn a_fields_known_that_vanished_from_the_file_is_written_again() {
        let s = scratch("fields-known-vanished");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        a.hit("SPC c t Esc");
        assert!(std::fs::read_to_string(&user).unwrap().contains("fields_known"), "첫 저장이 아는 열 목록을 안 적었다");

        // 옆에서 설정을 갈아 끼운다 — 이 키가 사라졌다.
        std::fs::write(&user, "[tui]\nfields = [\"id\"]\n").unwrap();
        a.hit("SPC c a Esc");
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("fields_known"), "사라진 키를 다시 안 적어, 다음 실행이 끈 열을 도로 켠다\n{text}");
    }

    /// **빈 `fields_known` 을 다시 적어도 남의 글은 그대로다**(moai-4gy5) — 매 저장이 설정 파일 전체를
    /// 다시 쓰는 길이라, 모르는 키·주석·다른 표가 한 번의 토글로 조용히 사라지면 되돌릴 방법이 도구
    /// 밖에만 남는다. 이 저장소가 못 견디는 것이 그 조용한 손실이다.
    ///
    /// 뒤쪽은 다른 것을 잰다 — **켠 것을 도로 끄면 바이트가 돌아온다.** 끈 열을 켰다 끄는 차례로 잰다
    /// (moai-8mq9.p38 리뷰). `merge_words` 는 더할 낱말을 배열 **끝**에 붙이므로, 켠 열을 껐다 켜는
    /// 차례로 재면 그 낱말이 끝으로 옮겨 가 배열이 재배열된다 — 오늘은 `tags` 가 마침 끝이라 지나갔고,
    /// `SPC c i`(id) 로 같은 것을 재면 지금 바이너리에서도 붉어진다. 그 차례로 잰 시험은 멱등성을 열 하나에
    /// 대해서만 증명하면서 모든 열에 대해 증명한 것처럼 읽힌다.
    #[test]
    fn rewriting_an_empty_fields_known_keeps_the_rest_of_the_file_byte_for_byte() {
        let s = scratch("fields-known-empty-keeps");
        let user = s.join("user.toml");
        let before = "# 손으로 적은 설정\n[tui]\n# 켠 열만 적었다\nfields = [\"id\"]\nfields_known = []\nwhat_is_this = 7\n\n[i18n]\nlang = \"en\"\n";
        std::fs::write(&user, before).unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        a.hit("SPC c t Esc");

        let text = std::fs::read_to_string(&user).unwrap();
        // 토글이 실제로 파일을 다시 썼다 — 안 썼으면 아래 것들이 그대로인 것이 아무 말도 안 한다.
        // **키를 갈라 본다**(moai-8mq9.p38 리뷰) — 낱말 하나를 글에서 찾는 것으로는 못 가른다.
        // `fields_known` 이 늘 아홉 이름을 다 실어, `fields` 가 통째로 안 적혀도 `"tags"` 는 글에 선다.
        let (back, _) = crate::user_config::read_look(Some(&user));
        assert_eq!(back.fields, Some(vec!["id".to_string(), "tags".to_string()]), "켠 열을 안 적었다\n{text}");
        assert_eq!(
            back.fields_known.map(|k| k.len()),
            Some(view::Field::ALL.len()),
            "빈 fields_known 을 이름으로 안 채웠다\n{text}"
        );
        assert!(text.contains("# 손으로 적은 설정"), "표 위의 주석이 사라졌다\n{text}");
        assert!(text.contains("# 켠 열만 적었다"), "키 위의 주석이 사라졌다\n{text}");
        assert!(text.contains("what_is_this = 7"), "모르는 키가 사라졌다\n{text}");
        assert!(text.contains("lang = \"en\""), "다른 표가 사라졌다\n{text}");
        assert_eq!(a.notice, None, "관대히 읽을 자리에서 잔소리를 했다\n{text}");

        // 끈 열(담당)을 켰다 끈다 — 위의 글이 대는 까닭으로 켠 열을 껐다 켜지 않는다.
        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.user_config = Some(user.clone());
        b.load_look();
        b.hit("SPC c a Esc");
        // **먼저 달라진 것을 본다** — 안 보면 저장이 통째로 막힌 판에서도 아래 견줌이 그대로 지나간다
        // (`save_look` 은 못 적은 것을 알림으로 삼키고 파일을 안 건드린다).
        let on = std::fs::read_to_string(&user).unwrap();
        assert_ne!(on, text, "둘째 세션의 토글이 파일에 안 닿았다 — 아래 견줌이 헛것이 된다\n{on}");
        b.hit("SPC c a Esc");
        assert_eq!(b.notice, None, "제가 적은 설정을 읽고 쓰며 말이 섰다\n{on}");
        assert_eq!(std::fs::read_to_string(&user).unwrap(), text, "켰다 끈 뒤 파일이 달라졌다");
    }

    /// **끈 새 열은 파일을 한 바퀴 돌고도 꺼진 채다**(moai-6bc0 단계 리뷰). `look_now()` 끼리 견주는
    /// 것만으로는 못 잡는다 — `fields_known` 은 이 바이너리가 아는 열 전부라 세션 내내 같은 값이고,
    /// 적는 길이 `App::saved` 와의 **차이만** 옮기므로 그 키가 파일에 영영 안 적힐 수 있다. 그러면
    /// 끈 것(`fields` 에서 빠진 것)을 다음 실행이 "그 열을 몰랐다" 로 읽어 도로 켠다.
    #[test]
    fn turning_off_a_new_column_survives_a_trip_through_the_file() {
        let s = scratch("fields-known");
        let user = s.join("user.toml");
        // 이 키를 모르던 바이너리가 적어 둔 설정 — `fields_known` 이 없다.
        std::fs::write(&user, "[tui]\nfields = [\"id\", \"priority\", \"tally\"]\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        assert!(a.fields.shows(view::Field::Names), "옛 설정에 새 열이 안 떴다");
        a.hit("SPC c h Esc");
        assert!(!a.fields.shows(view::Field::Names));

        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("fields_known"), "끈 것을 가를 자를 안 적었다\n{text}");
        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.user_config = Some(user);
        b.load_look();
        assert!(!b.fields.shows(view::Field::Names), "끈 열이 다음 실행에 도로 켜졌다\n{text}");
        assert!(b.fields.shows(view::Field::Branch), "같이 안 끈 열까지 꺼졌다\n{text}");
        assert_eq!(b.notice, None, "제가 적은 설정을 읽으며 말이 섰다");
    }

    /// **`SPC s` 가 차례를 고르고, 같은 키를 다시 누르면 거꾸로 선다**(moai-55cp). 다른 키로 가면
    /// 그 키의 제 방향부터다. 커서는 보던 줄에 붙는다.
    #[test]
    fn the_sort_menu_orders_rows_and_the_same_key_reverses() {
        let dated = |id: &str, created: &str| {
            let mut i = make(id, Kind::Issue);
            i.created_at = created.into();
            i
        };
        let issues = vec![
            dated("argos-0001", "2026-09-02T00:00:00Z"),
            dated("argos-0002", "2026-09-03T00:00:00Z"),
            dated("argos-0003", "2026-09-01T00:00:00Z"),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"]);
        a.hit("SPC s c Esc");
        assert_eq!(row_ids(&a), ["argos-0002", "argos-0001", "argos-0003"], "새것이 위가 아니다");
        assert_eq!(a.cursor, 1, "커서가 보던 줄(argos-0001)을 놓쳤다");
        a.hit("SPC s c Esc");
        assert_eq!(row_ids(&a), ["argos-0003", "argos-0001", "argos-0002"], "다시 눌렀는데 안 뒤집혔다");
        // 메뉴의 표시도 같은 한 벌(`Ctx::sorting`)을 읽는다 — 고른 차례에만 붙고 방향은 낱말로 댄다(moai-y61p 단계 리뷰).
        let ctx = a.key_ctx(&a.rows());
        assert_eq!(
            keys::Browse::Sort(keys::Order::Created).state(&ctx),
            Some("[● 거꾸로]"),
            "메뉴가 고른 차례·방향을 모른다"
        );
        assert_eq!(keys::Browse::Sort(keys::Order::Priority).state(&ctx), None, "고르지 않은 차례에 표시가 붙었다");
        a.hit("SPC s t Esc");
        assert_eq!(a.order, keys::Sorting { by: keys::Order::Title, reversed: false }, "다른 키가 거꾸로를 물려받았다");
        a.hit("SPC s p Esc");
        assert_eq!(a.order, Default::default());
    }

    /// 도는 줄이 있는지 — 줄마다의 답([`App::spins`])을 모은 것. 루프가 깨는 것은 이것이
    /// 아니라 **그려진 화면**이다(`App::spun`, draw 시험) — 여기는 줄의 자만 본다.
    fn any_spins(a: &App) -> bool {
        (0..a.site.issues.len()).any(|at| a.site.spins(at))
    }

    /// 돌 줄이 있는가를 적힌 칸·읽은 칸이 속이지 않는다.
    #[test]
    fn a_line_spins_only_while_someone_holds_it() {
        let mut a = app();
        assert!(!any_spins(&a), "todo 뿐인데 돈다고 한다");
        let mut issues = a.site.issues.clone();
        // 에픽의 적힌 칸으로는 안 돈다 — 묶음의 칸은 멤버에서 읽는다.
        issues[0].status = Status::new("in_progress");
        a.adopt(issues.clone());
        assert!(!any_spins(&a), "멤버가 안 움직인 에픽이 적힌 칸으로 돈다");
        // 읽은 칸으로도 안 돈다 — 멤버 하나가 끝나기만 해도 묶음은 `in_progress` 로
        // 읽히는데, 그것으로 깨우면 아무도 손대지 않은 저장소에서 화면이 계속 깬다.
        issues[2].status = Status::new("done");
        a.adopt(issues.clone());
        assert!(!any_spins(&a), "일은 멈췄는데 묶음의 읽은 칸으로 돈다");
        issues[3].status = Status::new("in_progress");
        a.adopt(issues);
        assert!(any_spins(&a), "in_progress 가 있는데 안 돈다고 한다");
    }

    /// **도는 칸은 설정이 정한다**(moai-q59j) — `Config::is_started` 인 칸 모두. 칸 이름을 박아
    /// 두면 `doing` 으로 바꾼 설정에서 아무것도 안 돌았다. `review` 도 시작한 칸이라 돈다. 설정이
    /// 모르는 칸(바꾼 설정의 `in_progress`)은 안 돈다.
    #[test]
    fn every_started_column_spins_whatever_it_is_named() {
        let renamed =
            crate::config::Config::parse("prefix = \"argos\"\nstatuses = \"todo, doing, check, done\"\n").unwrap();
        for (cfg, cases) in [
            (cfg(), vec![("todo", false), ("in_progress", true), ("review", true), ("done", false)]),
            (renamed, vec![("todo", false), ("doing", true), ("check", true), ("done", false), ("in_progress", false)]),
        ] {
            for (st, want) in cases {
                let mut i = make("argos-0001", Kind::Issue);
                i.status = Status::new(st);
                let a = App::new(vec![i], cfg.clone(), Path::new());
                assert_eq!(a.site.spins(0), want, "{st} 칸이 도는가 — {:?}", cfg.statuses);
            }
        }
    }

    /// 줄마다 도는지(`App::spins`) — **묶음은 그 밑에 집은 일이 있을 때만 돈다**
    /// (moai-x5eg). 글리프·빛줄기가 이 답으로 그려지고, 루프는 그려진 것으로 깬다.
    #[test]
    fn a_group_spins_only_while_a_member_is_held() {
        fn spun(a: &App) -> Vec<&str> {
            (0..a.site.issues.len()).filter(|&at| a.site.spins(at)).map(|at| a.site.issues[at].id.as_str()).collect()
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
        assert_eq!((a.site.column(0), a.site.column(1)), ("in_progress", "in_progress"));
        assert!(spun(&a).is_empty(), "반쯤 끝난 묶음이 돈다 — {:?}", spun(&a));
        assert!(!any_spins(&a));

        // 하나를 집으면 그 일과 에픽, 그리고 **에픽을 거쳐 물려받은 마일스톤**까지 돈다.
        issues[3].status = Status::new("in_progress");
        a.adopt(issues.clone());
        assert_eq!(spun(&a), ["argos-0005", "argos-0001", "argos-0003"]);
        assert!(any_spins(&a));

        // **미룬 멤버는 묶음을 안 돌린다.** 칸 셈이 그 멤버를 빼므로(`report::counted`)
        // 곁들이도 뺀다 — 에픽은 끝난 멤버만 남아 `done` 으로 읽힌다. **미룬 일 제 줄도
        // 안 돈다**(moai-tawj) — 계획에서 뺀 일이 "지금 손대는 중" 으로 보이면 안 되고,
        // 돌면 탐색기를 스피너 걸음으로 깨운다. 글리프는 칸을 여전히 말한다(`▸` 가 아닌
        // 멈춘 `in_progress` 글리프).
        issues[3].deferred_at = Some("2026-09-02T00:00:00Z".into());
        a.adopt(issues.clone());
        assert_eq!(a.site.column(1), "done");
        assert_eq!(a.site.column(3), "in_progress");
        assert!(spun(&a).is_empty(), "미룬 일이 돈다 — {:?}", spun(&a));
        assert!(!any_spins(&a), "미룬 일 하나가 탐색기를 빠른 걸음으로 깨운다");

        // **묶음을 미루면 그 밑이 다 멈춘다.** 칸 셈은 묶음 제 미룸으로 멤버를 안 빼
        // 에픽은 여전히 `in_progress` 로 읽히지만, 에픽도 멤버도 계획에서 빠졌다 — 멤버는
        // 물려받은 미룸으로 안 돌고, 묶음이 혼자 돌면 도는 멤버 하나 없이 도는 줄이 선다.
        issues[3].deferred_at = None;
        issues[1].deferred_at = Some("2026-09-02T00:00:00Z".into());
        a.adopt(issues.clone());
        assert_eq!(a.site.column(1), "in_progress", "제 미룸으로 집은 멤버를 뺐다");
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
    }

    use super::settle_reads as settle;

    /// `.moai` 한 벌을 얹은 임시 저장소. 자리를 만들고 지우는 일(터져도 치우는 것까지)은
    /// [`Scratch`] 가 한다.
    fn scratch(name: &str) -> Scratch {
        let dir = Scratch::new(&format!("tui-{name}"));
        std::fs::create_dir_all(dir.join(".moai")).unwrap();
        std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
        dir
    }

    /// 지금 보이는 줄의 id 들 (`..` 은 뺀다).
    fn shown(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(_, e, _) => e.at().map(|at| a.site.issues[at].id.clone()),
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

    /// **들어가면 `..` 너머 첫 줄에 선다**(moai-cm13). `..` 에 세우면 들어가자마자 누른 Enter
    /// 한 번이 도로 나와 Enter 두 번이 제자리다. `..` 은 `k` 한 번 거리에 남는다. 빈
    /// 디렉터리는 `..` 뿐이라 거기 선다.
    #[test]
    fn entering_lands_past_the_up_row() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!((a.site.path.len(), a.cursor), (1, 1), "들어가서 `..` 에 섰다");
        assert!(matches!(a.current(), Some(Row::Item(..))), "{:?}", a.current());
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "들어가자마자 누른 Enter 가 도로 나왔다");
        a.key(key(KeyCode::Char('k')));
        assert_eq!(a.current(), Some(Row::Up), "`..` 이 `k` 한 번 거리에 없다");
        a.key(key(KeyCode::Enter));
        assert!(a.site.path.is_empty());

        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Enter));
        assert_eq!((a.site.path.len(), a.current()), (1, Some(Row::Up)), "빈 디렉터리에서 `..` 말고 설 데가 없다");
    }

    /// 들어갔다 나오면 **있던 자리로 돌아온다**. 매번 맨 위로 튕기면 못 쓴다.
    #[test]
    fn leaving_puts_the_cursor_back_where_it_was() {
        let mut a = app();
        a.key(key(KeyCode::Down)); // 두 번째 에픽
        assert_eq!(a.cursor, 1);
        a.key(key(KeyCode::Enter));
        assert_eq!((a.cursor, a.site.path.len()), (0, 1));
        a.key(key(KeyCode::Backspace));
        assert_eq!((a.cursor, a.site.path.len()), (1, 0), "있던 자리로 안 돌아왔다");
    }

    /// 들어가 있는 동안 **위에 줄이 생겨도** 나오면 방금 나온 디렉터리에 선다.
    /// 기억한 번호로 돌아가면 옆 에픽에 선다.
    #[test]
    fn leaving_finds_the_directory_it_came_from_even_after_a_reload() {
        let mut a = app();
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path, [Seg::Epic("argos-0002".into())]);

        let mut more = a.site.issues.clone();
        more.push(make("argos-0000", Kind::Epic));
        a.adopt(more);
        a.key(key(KeyCode::Backspace));
        assert_eq!(
            a.current().map(|r| match r {
                Row::Item(Seat::Here, e, _) => e,
                other => panic!("{other:?}"),
            }),
            Some(Entry::Dir { seg: Seg::Epic("argos-0002".into()), at: a.site.index.find("argos-0002") }),
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

    /// `Ctrl-w w` 는 앞으로, `Ctrl-w W` 는 뒤로 돈다. 끝에서 처음으로 넘어간다.
    /// `Ctrl-w h`·`Ctrl-w l` 은 순환이 아니라 **한 칸 옆**이라 끝에서 제자리다(moai-oudf).
    /// **`Tab` 은 여기서 아무 일도 안 한다** — 목록의 재귀 펼침이 받을 자리다.
    #[test]
    fn ctrl_w_moves_between_the_panes() {
        let mut a = app();
        assert_eq!(a.focus, Pane::Explorer, "목록에서 시작하지 않는다");
        a.hit("Ctrl-w w");
        assert_eq!(a.focus, Pane::Detail);
        a.hit("Ctrl-w w");
        assert_eq!(a.focus, Pane::Explorer, "끝에서 처음으로 안 돌았다");
        a.hit("Ctrl-w W");
        assert_eq!(a.focus, Pane::Detail, "Ctrl-w W 가 뒤로 안 돌았다");
        a.hit("Ctrl-w W");
        assert_eq!(a.focus, Pane::Explorer, "Ctrl-w W 가 뒤로 안 돌았다");
        // 쪽으로 가는 키는 끝에서 제자리다 — 목록에서 `Ctrl-w h` 는 상세로 돌지 않는다.
        a.hit("Ctrl-w h");
        assert_eq!(a.focus, Pane::Explorer, "왼쪽 끝에서 되돌아 돌았다");
        a.hit("Ctrl-w l");
        assert_eq!(a.focus, Pane::Detail);
        a.hit("Ctrl-w l");
        assert_eq!(a.focus, Pane::Detail, "오른쪽 끝에서 되돌아 돌았다");
        a.hit("Ctrl-w h");
        assert_eq!(a.focus, Pane::Explorer);
        // 옛 키는 남기지 않았다 — `Tab` 은 이제 목록의 것이다.
        a.key(key(KeyCode::Tab));
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(a.focus, Pane::Explorer, "Tab 이 아직 칸을 옮긴다");
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

        a.hit("Ctrl-w w");
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
        a.hit("Ctrl-w w");
        a.key(key(KeyCode::End));
        assert_eq!(a.cursor, a.rows().len() - 1);
        a.key(key(KeyCode::Home));
        assert_eq!(a.cursor, 0);
        a.key(key(KeyCode::PageDown));
        assert_eq!(a.cursor, a.rows().len() - 1, "PageDown 이 목록 밖으로 나갔다");
    }

    /// **드나드는 키(Enter·Backspace)도 포커스를 탄다.** 상세를 읽다 누른 키가
    /// 목록을 옮기면 보던 이슈가 바뀌고 굴린 자리도 잃는다. `→`·`←`(펼침·접기)도 같은 자다 —
    /// 접기는 접을 것이 없으면 나가기와 같은 일을 하므로 여기서 함께 잰다(moai-7qot).
    #[test]
    fn entering_and_leaving_keys_go_to_the_focused_pane() {
        let mut a = app();
        a.hit("Ctrl-w w");
        a.key(key(KeyCode::Enter));
        assert!(a.site.path.is_empty(), "상세에 포커스가 있는데 Enter 가 목록을 들어갔다");
        // 펼침도 목록 포커스를 탄다 — 상세를 읽다 누른 `→` 가 목록의 줄을 늘리면 안 된다.
        a.key(key(KeyCode::Right));
        assert_eq!(row_ids(&a).len(), 3, "상세에 포커스가 있는데 `→` 가 목록을 펼쳤다");
        a.hit("Ctrl-w w");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "목록에 포커스가 있는데 Enter 가 안 들어갔다");
        for k in [KeyCode::Backspace, KeyCode::Left] {
            let mut a = app();
            a.key(key(KeyCode::Enter));
            drawn(&mut a, 10, 40);
            a.hit("Ctrl-w w");
            a.key(key(KeyCode::Down));
            a.key(key(k));
            assert_eq!((a.site.path.len(), a.detail.offset()), (1, 1), "상세에 포커스가 있는데 {k:?} 가 목록을 나갔다");
            a.hit("Ctrl-w w");
            a.key(key(k));
            assert!(a.site.path.is_empty(), "목록에 포커스가 있는데 {k:?} 가 안 나갔다");
        }
    }

    /// 글을 받는 중에는 `Tab` 이 포커스를 안 옮기고 글자로도 안 들어간다 — 적다
    /// 말고 튀면 적던 것을 잃는다.
    #[test]
    fn tab_does_not_move_the_focus_while_typing() {
        for (opener, start) in [("/", Pane::Explorer), ("SPC f", Pane::Explorer), ("/", Pane::Detail)] {
            let mut a = app();
            a.focus = start;
            a.hit(opener);
            a.key(key(KeyCode::Char('a')));
            a.key(key(KeyCode::Tab));
            a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
            assert_eq!(a.focus, start, "{opener:?} 중에 Tab 이 포커스를 옮겼다");
            assert!(matches!(&a.mode, Mode::Grep(b, _) | Mode::Filter(b) if b.text() == "a"), "{:?}", a.mode);
        }
    }

    /// **`j`/`k` 는 포커스 칸을 움직인다**(moai-ob4c, 키 지도 moai-hudg). 옛 뜻 — 목록에 선 채
    /// 상세를 굴리기 — 는 없앴다: 목록에서 `j` 는 커서를 옮기고 상세는 그대로다. 뜻이 조용히
    /// 바뀐 키라 옛 기대를 뒤집어 따로 잰다.
    #[test]
    fn j_and_k_move_the_focused_pane_and_no_longer_scroll_the_detail_from_the_list() {
        let mut a = app();
        drawn(&mut a, 10, 40);
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('k')));
        assert_eq!(
            (a.cursor, a.detail.offset(), a.focus),
            (1, 0, Pane::Explorer),
            "목록 포커스에서 `j` 가 상세를 굴렸다"
        );

        a.hit("Ctrl-w w");
        drawn(&mut a, 10, 40);
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('k')));
        assert_eq!((a.cursor, a.detail.offset()), (1, 1), "상세 포커스에서 `j` 가 목록을 움직였다");
    }

    /// **SPC·`b` 는 더는 상세를 넘기지 않는다** — SPC 는 메뉴(moai-7sjm)의 자리다. 한 쪽은
    /// Ctrl-f·Ctrl-b·PageDown·PageUp 이다.
    #[test]
    fn space_and_b_no_longer_page_the_detail() {
        for start in Pane::ALL {
            let mut a = app();
            a.focus = start;
            drawn(&mut a, 10, 40);
            a.key(key(KeyCode::Char(' ')));
            a.key(key(KeyCode::Char('b')));
            assert_eq!((a.cursor, a.detail.offset(), a.site.path.len()), (0, 0, 0), "{start:?}");
            assert_eq!(a.mode, Mode::Browse);
        }
    }

    /// **vi 이동은 포커스 칸에서 화살표와 같은 일을 한다** — `gg`·`G`(SHIFT 붙어 와도)·
    /// Ctrl-d·Ctrl-u(반 쪽)·Ctrl-f·Ctrl-b(한 쪽). `g` 하나로는 안 움직이고 기다린다.
    #[test]
    fn vi_movement_keys_act_in_the_focused_pane() {
        let many: Vec<Issue> = (1..=30).map(|n| make(&format!("argos-{n:04}"), Kind::Issue)).collect();
        let mut a = App::new(many, cfg(), Path::new());
        let big_g = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        let (half, page) = (scroll::HALF, scroll::PAGE);

        a.key(ctrl('d'));
        assert_eq!(a.cursor, half, "Ctrl-d 가 반 쪽을 안 갔다");
        a.key(ctrl('f'));
        assert_eq!(a.cursor, half + page, "Ctrl-f 가 한 쪽을 안 갔다");
        a.key(ctrl('u'));
        assert_eq!(a.cursor, page);
        a.key(ctrl('b'));
        assert_eq!(a.cursor, 0);
        a.key(big_g);
        assert_eq!(a.cursor, 29, "SHIFT 붙은 `G` 가 맨 아래로 안 갔다");
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.cursor, 29, "`g` 하나에 움직였다");
        assert!(a.chord.waiting());
        a.key(key(KeyCode::Char('g')));
        assert_eq!((a.cursor, a.chord.waiting()), (0, false), "`gg` 가 맨 위로 안 갔다");
        a.key(key(KeyCode::Char('l')));
        assert_eq!(a.site.path.len(), 0, "잎에서 `l` 이 무언가 했다");

        a.hit("Ctrl-w w");
        drawn(&mut a, 10, 40);
        a.key(ctrl('d'));
        assert_eq!(a.detail.offset(), half);
        a.key(ctrl('f'));
        assert_eq!(a.detail.offset(), half + page);
        a.key(ctrl('u'));
        a.key(ctrl('b'));
        assert_eq!(a.detail.offset(), 0);
        a.key(big_g);
        assert_eq!(a.detail.offset(), 30, "상세에서 `G` 가 끝에 안 닿았다");
        a.key(key(KeyCode::Char('g')));
        a.key(key(KeyCode::Char('g')));
        assert_eq!((a.cursor, a.detail.offset()), (0, 0), "상세에서 `gg` 가 목록을 움직였거나 첫 줄로 안 갔다");
    }

    /// **`l` 은 그 자리에서 한 단계 펼친다**(moai-7qot, 사용자 결정) — 들어가는 것이 아니라
    /// 멤버가 그 줄 **바로 밑에** 선다. 깊이는 가지 모양([`Twig`])에 실려 그리는 쪽으로 간다.
    #[test]
    fn l_expands_one_level_in_place() {
        let mut a = app();
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0009"], "시험의 전제");
        a.key(key(KeyCode::Char('l')));
        assert_eq!(a.site.path, Path::new(), "펼치기가 디렉터리에 들어갔다");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0003", "argos-0004", "argos-0002", "argos-0009"]);
        let depths: Vec<usize> = a
            .rows()
            .iter()
            .filter_map(|r| if let Row::Item(Seat::Here, _, t) = r { Some(t.depth()) } else { None })
            .collect();
        assert_eq!(depths, [0, 1, 1, 0, 0], "펼친 멤버의 깊이가 1 이 아니다");
        // 막내만 `└─` 다 — 그리는 쪽이 이 값으로 글자를 고른다.
        let last: Vec<bool> = a
            .rows()
            .iter()
            .filter_map(|r| if let Row::Item(Seat::Here, _, t) = r { Some(t.last()) } else { None })
            .collect();
        assert_eq!(last, [false, false, true, false, false]);
        // 다시 눌러도 한 단계뿐 — 더 펼칠 것이 없다.
        a.key(key(KeyCode::Char('l')));
        assert_eq!(row_ids(&a).len(), 5);
    }

    /// **펼친 줄에서 `h` 는 접는다** — 한 층 나가지 않는다. 접을 것이 없을 때만 나간다.
    #[test]
    fn h_folds_the_row_before_it_leaves() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "시험의 전제 — 에픽 안에 섰다");
        a.key(key(KeyCode::Backspace));
        a.key(key(KeyCode::Char('l')));
        assert_eq!(row_ids(&a).len(), 5, "안 펼쳐졌다");
        a.key(key(KeyCode::Char('h')));
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0009"], "`h` 가 안 접었다");
        assert_eq!(a.site.path, Path::new(), "접으면서 한 층 나갔다");
        a.key(key(KeyCode::Char('h')));
        assert_eq!(a.site.path, Path::new(), "뿌리에서 더 나갈 데가 없다");
    }

    /// **`Tab` 은 재귀로 다 펼치고, 다시 누르면 밑까지 접는다**(moai-7qot, 사용자 결정).
    ///
    /// 다 접을 때 **밑의 펼침까지 걷는다** — 한 단계 접기(`h`)와 다른 자리다. 안을 기억해 두면
    /// 다시 눌렀을 때 접기 전 모양이 아니라 그 이전 모양이 서서 두 번 눌러야 같은 자리로 온다.
    #[test]
    fn tab_expands_the_whole_group_and_folds_it_back() {
        let issues = vec![
            make("argos-0001", Kind::Milestone),
            {
                let mut e = make("argos-0002", Kind::Epic);
                e.milestone = Some("argos-0001".into());
                e
            },
            member("argos-0003", "argos-0002"),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        assert_eq!(row_ids(&a), ["argos-0001"], "시험의 전제 — 마일스톤 하나만 선다");
        a.hit("Tab");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"], "재귀로 안 펼쳤다");
        let depths: Vec<usize> = a
            .rows()
            .iter()
            .filter_map(|r| if let Row::Item(Seat::Here, _, t) = r { Some(t.depth()) } else { None })
            .collect();
        assert_eq!(depths, [0, 1, 2]);
        a.hit("Tab");
        assert_eq!(row_ids(&a), ["argos-0001"], "다시 누른 `Tab` 이 안 접었다");
        a.hit("Tab");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"], "접은 뒤의 `Tab` 이 한 단계만 펼쳤다");
    }

    /// 마일스톤 → 에픽 → 멤버 셋. 트리로 펼친 줄을 재는 시험이 함께 쓴다.
    fn nested() -> Vec<Issue> {
        vec![
            make("argos-0001", Kind::Milestone),
            {
                let mut e = make("argos-0002", Kind::Epic);
                e.milestone = Some("argos-0001".into());
                e
            },
            member("argos-0003", "argos-0002"),
        ]
    }

    /// **멤버 줄의 `h` 는 부모 묶음을 접고 그 줄에 선다**(사용자 결정 2026-09-19) — 한 층 나가면
    /// 펼쳐 보던 트리가 통째로 사라진다. 두 층 밑에서는 한 층씩 올라가며 접는다.
    #[test]
    fn h_on_a_member_folds_its_parent_and_stands_there() {
        let mut a = App::new(nested(), cfg(), Path::new());
        a.hit("Tab");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"], "시험의 전제");
        a.hit("G");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0003");
        a.hit("h");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002"], "부모 에픽이 안 접혔다");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0002", "부모 줄에 안 섰다");
        a.hit("h");
        assert_eq!(row_ids(&a), ["argos-0001"], "한 층 더 올라가며 안 접었다");
        assert_eq!(row_ids(&a)[a.cursor], "argos-0001");
        assert_eq!(a.site.path, Path::new(), "접는 동안 디렉터리를 나갔다");
    }

    /// **펼쳐 든 줄에서 `Enter` 는 그 줄의 제 자리로 들어간다**(리뷰). 지금 자리에 제 마디만
    /// 이으면 가운데 마디가 빠진 있지도 않은 자리가 서서, 한 키 전에 보였던 멤버가 사라진다.
    #[test]
    fn entering_an_inlined_group_goes_to_its_own_place() {
        let mut a = App::new(nested(), cfg(), Path::new());
        a.hit("Tab");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"], "시험의 전제");
        a.cursor = 1;
        a.key(key(KeyCode::Enter));
        assert_eq!(
            a.site.path,
            vec![Seg::Milestone(Some("argos-0001".into())), Seg::Epic("argos-0002".into())],
            "가운데 마디를 빠뜨린 자리로 들어갔다"
        );
        assert_eq!(row_ids(&a), ["argos-0003"], "들어간 디렉터리가 비었다");
        // 기억한 번호는 층마다 하나다 — 지나친 층은 0 으로 메운다.
        assert_eq!(a.site.remembered.len(), a.site.path.len(), "기억 자리가 층 수와 어긋났다");
        // 나오는 길도 한 층씩이다.
        a.key(key(KeyCode::Backspace));
        assert_eq!(a.site.path, vec![Seg::Milestone(Some("argos-0001".into()))]);
    }

    /// **한 단계 접은 뒤의 `Tab` 은 곧바로 펼친다**(리뷰). 밑에 남은 펼침으로 접을지를 가르던
    /// 때는 그 `Tab` 이 기억만 걷고 화면은 그대로여서, 펼치려면 두 번을 눌러야 했다.
    #[test]
    fn tab_expands_right_after_a_one_level_fold() {
        let mut a = App::new(nested(), cfg(), Path::new());
        a.hit("Tab");
        assert_eq!(row_ids(&a).len(), 3, "시험의 전제");
        a.key(key(KeyCode::Char('h')));
        assert_eq!(row_ids(&a), ["argos-0001"], "`h` 가 한 단계 안 접었다");
        a.hit("Tab");
        assert_eq!(row_ids(&a).len(), 3, "접은 뒤의 첫 `Tab` 이 아무 일도 안 했다");
    }

    /// **검색이 저절로 연 줄에서 `h` 는 디렉터리를 나가지 않는다**(리뷰). 그 펼침은 `expanded` 에
    /// 없지만 눈에는 열려 있으므로, 접을 것이 없는 줄로 읽으면 보던 자리를 통째로 잃는다.
    #[test]
    fn h_on_a_group_the_search_opened_keeps_the_place() {
        let mut a = App::new(nested(), cfg(), Path::new());
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "시험의 전제 — 마일스톤 안에 섰다");
        search(&mut a, "argos-0003");
        assert_eq!(row_ids(&a), ["argos-0002", "argos-0003"], "검색이 맞힌 자리를 안 열었다");
        a.cursor = a.rows().iter().position(|r| matches!(r, Row::Item(_, Entry::Dir { .. }, _))).expect("에픽 줄");
        assert!(a.key_ctx(&a.rows()).expanded, "눈에 열린 줄을 안 펼쳐진 것으로 읽는다");
        a.key(key(KeyCode::Char('h')));
        assert_eq!(a.site.path.len(), 1, "`h` 가 마일스톤을 통째로 나갔다");
    }

    /// **검색을 풀면 커서는 그 줄을 품은 폴더에 선다**(리뷰). 검색이 연 폴더가 접히면서 그 줄이
    /// 사라지는데, 번호로 세우면 검색 때와 아무 상관 없는 줄에 서고 까닭을 댈 자리도 없다.
    #[test]
    fn clearing_a_search_stands_on_the_folder_that_swallowed_the_row() {
        let mut is = nested();
        // 접힌 목록이 사라진 줄의 번호보다 길어야 **번호로 세우면 엉뚱한 줄**이 선다 —
        // 그 갈림이 없으면 번호를 줄 수 안으로 자르는 것만으로 우연히 폴더에 선다.
        is.push(make("argos-0008", Kind::Milestone));
        is.push(make("argos-0009", Kind::Milestone));
        let mut a = App::new(is, cfg(), Path::new());
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0008", "argos-0009"], "시험의 전제");
        search(&mut a, "argos-0003");
        // 칸을 Enter 로 닫는다 — 칸 안의 Esc 는 치기 전 자리로 되돌리는 다른 길이다(`grep_was`).
        a.key(key(KeyCode::Enter));
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0003"], "검색이 맞힌 자리를 안 열었다");
        a.cursor = 2;
        a.hit("Esc");
        assert!(!a.searching(), "검색이 안 풀렸다");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0008", "argos-0009"], "도로 안 접혔다");
        assert_eq!(a.cursor, 0, "사라진 줄을 품은 폴더가 아니라 같은 번호의 줄에 섰다");
    }

    /// **펼침은 설정에 안 남는다**(moai-7qot, 사용자 결정) — 보기(`Look`)는 *무엇을 숨기나* 고
    /// 펼침은 *지금 어디를 보나* 다. 설정에 이슈 id 를 쌓으면 지운 줄의 id 가 거기 남는다.
    #[test]
    fn folding_never_reaches_the_config() {
        let mut a = app();
        let before = a.look_now();
        a.key(key(KeyCode::Char('l')));
        a.hit("Tab");
        assert_eq!(a.look_now(), before, "펼침이 설정에 실렸다");
    }

    /// **검색은 맞힌 줄의 자리를 저절로 열고, 거름망은 안 연다**(moai-i5io, 사용자 결정).
    ///
    /// 검색은 한 줄을 찾는 일이라 그 줄이 어디 있는지를 화면이 말해야 한다. 거름망은 오래 걸어
    /// 두고 폴더를 돌아다니는 것이라(Esc 가 안 푸는 보기와 한 자리다) 저절로 열면 `type=issue`
    /// 한 줄에 저장소 전체가 펼쳐진다.
    #[test]
    fn a_search_opens_the_groups_it_matched_but_a_filter_does_not() {
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "type=issue");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0009"], "거름망이 에픽을 펼쳤다");

        let mut a = app();
        search(&mut a, "argos-0004");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0004"], "검색이 맞힌 자리를 안 열었다");
        // 검색을 풀면 도로 접힌다 — 펼침은 `expanded` 에 안 쌓인다.
        a.hit("Esc");
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002", "argos-0009"]);
    }

    /// **펼친 하위도 거름망·보기를 그대로 통과한 것만 선다**(moai-i5io, 사용자 결정) — 층마다 같은
    /// 자다([`crate::nav::Index::entries_tree`]). 정렬은 형제끼리만 매긴다: 멤버가 부모를 넘어
    /// 올라가면 가지가 무엇에 달렸는지 알 수 없다.
    #[test]
    fn the_filter_and_the_order_reach_every_level_of_the_tree() {
        let mut is = vec![
            make("argos-0001", Kind::Epic),
            member("argos-0003", "argos-0001"),
            member("argos-0004", "argos-0001"),
        ];
        is[1].status = Status::new("done");
        // 멤버의 우선순위를 에픽보다 세게 둔다 — 형제끼리만 매기면 에픽 밑에 그대로 남는다.
        is[2].priority = Some(0);
        let mut a = App::new(is, cfg(), Path::new());
        a.key(key(KeyCode::Char('l')));
        // 처음에는 done 을 숨긴다(moai-fmv5) — 펼친 멤버에도 그 보기가 그대로 걸린다.
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0004"], "done 숨김이 펼친 멤버에 안 걸렸다");
        a.hit("SPC v d Esc");
        // 형제끼리의 차례는 고른 정렬이 매긴다 — p0 인 0004 가 0003 앞이다.
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0004", "argos-0003"], "done 을 켰는데 멤버가 안 선다");
        a.hit("SPC s p Esc");
        let ids = row_ids(&a);
        assert_eq!(ids[0], "argos-0001", "멤버가 부모를 넘어 올라갔다: {ids:?}");
    }

    /// **`h` 는 접기고, 접을 것이 없으면 나간다**(moai-7qot) — 그래서 목록 포커스를 탄다.
    /// 펼침(`l`)은 [`App::expanded`] 를 건드리므로 자리를 안 옮긴다.
    #[test]
    fn h_collapses_or_leaves_from_the_list_only() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "Enter 가 안 들어갔다");
        a.hit("Ctrl-w w");
        a.key(key(KeyCode::Char('h')));
        assert_eq!(a.site.path.len(), 1, "상세 포커스에서 `h` 가 나갔다");
        a.hit("Ctrl-w w");
        a.key(key(KeyCode::Char('h')));
        assert!(a.site.path.is_empty(), "접을 것이 없는데 `h` 가 안 나갔다");
    }

    /// **기다리는 `g` 뒤에 뜻 없는 키가 오면 둘 다 버린다** — 그 키도 제 뜻을 안 한다(모르는 키
    /// 무시와 같은 자). 버린 뒤의 `g` 하나는 다시 기다린다.
    #[test]
    fn an_unknown_key_after_g_is_ignored_and_clears_the_wait() {
        let mut a = app();
        a.key(key(KeyCode::Char('j')));
        for k in [
            key(KeyCode::Char('x')),
            key(KeyCode::Char('j')),
            key(KeyCode::Tab),
            key(KeyCode::Enter),
            key(KeyCode::Char('/')),
        ] {
            a.key(key(KeyCode::Char('g')));
            a.key(k);
            assert!(!a.chord.waiting(), "`g` 뒤의 {k:?} 가 열을 안 버렸다");
            assert_eq!(
                (a.cursor, a.focus, a.site.path.len()),
                (1, Pane::Explorer, 0),
                "`g` 뒤의 {k:?} 가 제 뜻을 했다"
            );
            assert_eq!(a.mode, Mode::Browse, "`g` 뒤의 {k:?} 가 칸을 열었다");
        }
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.cursor, 1);
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.cursor, 0);
    }

    /// **기다리는 열은 탐색 밖으로 새지 않는다.** 붙여넣기가 열을 버리고, 글칸에서는
    /// `g`·`j`·`k`·`h`·`l`·`G` 가 글자다. 글칸에 들어간 사이 버린 열은 돌아와 잇지 않는다.
    #[test]
    fn a_waiting_g_does_not_leak_into_paste_or_text_fields() {
        let mut a = app();
        a.key(key(KeyCode::Char('j')));
        a.key(key(KeyCode::Char('g')));
        a.paste("x");
        assert!(!a.chord.waiting(), "붙여넣기가 기다리던 `g` 를 안 버렸다");
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.cursor, 1, "붙여넣기 전의 `g` 와 이어 `gg` 가 됐다");
        a.key(key(KeyCode::Esc));

        a.key(key(KeyCode::Char('/')));
        for c in "gjkhlGg".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert!(
            matches!(&a.mode, Mode::Grep(b, _) if b.text() == "gjkhlGg"),
            "글칸에서 vi 키가 글자가 아니다 — {:?}",
            a.mode
        );
        assert!(!a.chord.waiting(), "글칸의 `g` 가 탐색의 열에 쌓였다");
        // 칸은 치는 대로 걸러 커서를 줄 수 안으로 당긴다 — Esc 가 열기 전 자리로 되돌린다.
        a.key(key(KeyCode::Esc));
        assert_eq!(a.cursor, 1);

        // 키가 아닌 길로 모드가 바뀌어도 — 기다리던 `g` 는 글칸의 키가 버린다
        a.mode = Mode::Browse;
        a.key(key(KeyCode::Char('g')));
        a.mode = Mode::Filter(Input::default());
        a.key(key(KeyCode::Char('x')));
        a.mode = Mode::Browse;
        a.key(key(KeyCode::Char('g')));
        assert_eq!(a.cursor, 1, "글칸을 거친 뒤의 `g` 가 옛 `g` 와 이었다");
    }

    /// 잎에서 Enter 는 아무 일도 하지 않는다 — 들어갈 데가 없다.
    #[test]
    fn entering_a_leaf_does_nothing() {
        let mut a = app();
        a.key(key(KeyCode::End)); // 소속 없는 이슈 (잎)
        let before = a.site.path.clone();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path, before);
    }

    /// `..` 에서 Enter 는 나가기다.
    #[test]
    fn entering_the_up_row_leaves() {
        let mut a = app();
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1);
        a.cursor = 0; // `..`
        a.key(key(KeyCode::Enter));
        assert!(a.site.path.is_empty());
    }

    #[test]
    fn quitting_works_every_documented_way() {
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(a.quit, "Ctrl-C 로 못 나갔다");
        let mut a = app();
        a.hit("SPC q");
        assert!(a.quit, "SPC q 로 못 나갔다");
        // 메뉴가 열린 채로도, 하위 층에서도 Ctrl-C 는 끝낸다.
        for open in ["SPC", "SPC v"] {
            let mut a = app();
            a.hit(open);
            a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(a.quit, "{open} 메뉴에서 Ctrl-C 로 못 나갔다");
        }
    }

    /// **`q`·F10 은 더는 끝내지 않는다**(키 지도 moai-hudg) — 실수로 누를 때마다 앱이 끝나던 것이
    /// 옮긴 까닭이다. 목록·상세 포커스 모두, 층에서도.
    #[test]
    fn q_and_f10_no_longer_quit_in_browse_or_on_the_layer() {
        let layered = || {
            let mut a = app();
            a.layer = Some(layer::fake(
                vec![("one", "/w/one", layer::Look::Shut { state: layer::Shut::Missing, said: String::new() })],
                layer::At::Layer,
            ));
            a
        };
        for (mut a, place) in [(app(), "안"), (layered(), "층")] {
            assert_eq!(a.on_layer(), place == "층");
            for pane in Pane::ALL {
                a.focus = pane;
                for k in [
                    key(KeyCode::Char('q')),
                    key(KeyCode::F(10)),
                    KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT),
                ] {
                    a.key(k);
                    assert!(!a.quit, "{place} {pane:?} 에서 {k:?} 가 끝냈다");
                    assert_eq!(a.mode, Mode::Browse);
                }
            }
        }
    }

    /// **SPC 메뉴는 App 에서 같은 길을 지난다**(moai-7sjm) — 곧바로 열리고, 모르는 키는 알림 없이
    /// 무시하고, Esc 가 닫고(거름망은 그대로), 메뉴로 누른 동작은 바로 누르던 키가 하던 그 일을 한다.
    #[test]
    fn the_spc_menu_opens_at_once_ignores_unknown_keys_and_runs_the_same_actions() {
        let mut a = app();
        a.hit("SPC");
        assert!(menu::open(&a.chord), "SPC 가 메뉴를 곧바로 안 열었다");
        let before = (a.cursor, a.focus, a.site.path.clone());
        for k in [
            key(KeyCode::Char('x')),
            key(KeyCode::Char('j')),
            key(KeyCode::Enter),
            key(KeyCode::Tab),
            key(KeyCode::Char('G')),
        ] {
            a.key(k);
            assert!(menu::open(&a.chord), "{k:?} 가 메뉴를 닫았다");
            assert_eq!(a.notice, None, "{k:?} 가 알림을 달았다");
            assert_eq!((a.cursor, a.focus, a.site.path.clone()), before, "{k:?} 가 메뉴 뒤의 목록을 움직였다");
            assert_eq!(a.mode, Mode::Browse);
        }

        // Esc 는 메뉴를 닫는 것이 먼저다 — 걸어 둔 거름망은 안 푼다.
        a.filter_text = Some("tag=x".into());
        a.key(key(KeyCode::Esc));
        assert!(!menu::open(&a.chord));
        assert_eq!(a.filter_text.as_deref(), Some("tag=x"), "메뉴의 Esc 가 거름망까지 풀었다");
        a.filter_text = None;

        // Bksp 는 한 층 위 — 뒤의 목록을 나가지 않는다.
        a.key(key(KeyCode::Enter));
        let inside = a.site.path.clone();
        a.hit("SPC v");
        a.key(key(KeyCode::Backspace));
        assert_eq!(menu::title(a.chord.held()), "SPC");
        a.key(key(KeyCode::Backspace));
        assert!(!menu::open(&a.chord));
        assert_eq!(a.site.path, inside, "메뉴의 Bksp 가 목록을 나갔다");

        // 같은 동작 — 거름망 칸, 검색 칸, 폼, 원문, 워크트리, 끝내기.
        let mut a = app();
        a.hit("SPC f");
        assert!(matches!(a.mode, Mode::Filter(_)), "{:?}", a.mode);
        let mut a = app();
        a.hit("SPC /");
        assert!(matches!(a.mode, Mode::Grep(..)), "{:?}", a.mode);
        let mut a = app();
        a.hit("SPC n");
        assert!(matches!(a.mode, Mode::Idea(_)), "{:?}", a.mode);
        let mut a = app();
        a.hit("SPC v r Esc");
        assert!(a.raw && !menu::open(&a.chord));
        let was = a.worktree;
        a.hit("SPC v w Esc");
        assert_eq!(a.worktree, !was);
    }

    /// **`SPC m g` 은 커서가 든 묶음까지만 읽는다**(moai-z9pc.9av). 에픽 밖의 줄에서 누르면
    /// 그 줄의 자리가 뿌리(빈 경로)라, 빈 경로로 `starts_with` 를 걸던 옛 식은 저장소의 안 읽은
    /// 줄을 통째로 읽음으로 적었다 — 한 번 적히면 도구 안에 되돌릴 길이 없다.
    #[test]
    fn reading_a_group_never_swallows_the_whole_repo() {
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        // 시계를 고정한다(`an_unread_line_wears_new_until_it_is_read` 와 같은 까닭).
        a.site.now = "2026-09-13T13:42:07Z".into();
        a.me = Some("레이븐 (raven@example.com)".into());
        a.recount_unread();
        let all = a.site.unread.len();
        assert_eq!(all, a.site.issues.len(), "내 줄인데 안 읽음이 빠졌다");

        // 에픽 밖의 줄에서 누른다 — 아무것도 안 읽고 까닭을 댄다.
        let rows = row_ids(&a);
        a.cursor = rows.iter().position(|id| id == "argos-0009").expect("에픽 없는 줄이 없다");
        a.hit("SPC m g");
        assert_eq!(a.site.unread.len(), all, "묶음 밖에서 누른 것이 저장소를 통째로 읽었다");
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("묶음")), "{:?}", a.notice);

        // 에픽 줄에서 누르면 **그 에픽과 그 멤버만** 선다 — 옆 에픽은 그대로다.
        a.cursor = rows.iter().position(|id| id == "argos-0001").expect("에픽 줄이 없다");
        a.hit("SPC m g");
        let left: Vec<&str> = a.site.unread.iter().map(String::as_str).collect();
        assert_eq!(left, ["argos-0002", "argos-0009"], "에픽 하나를 읽었는데 남은 것이 다르다");
        // 적히는 것은 **본 줄의 도장**이다 — 누른 때(`App::now`)가 아니다(moai-lyc1).
        let line = a.site.issues.iter().find(|i| i.id == "argos-0001").unwrap().updated_at.clone();
        assert_ne!(line, a.site.now, "시험이 두 값을 못 가른다");
        assert_eq!(a.site.seen.get("argos-0001"), Some(&line), "본 줄의 updated_at 이 아니라 다른 값을 적었다");
    }

    /// **`SPC m g` 은 그 줄이 사는 묶음을 읽는다 — 지금 디렉터리의 묶음이 아니다**(리뷰). 지금
    /// 디렉터리로 읽던 때는, 펼쳐 든 멤버 줄에 서서 누르면 그 줄의 에픽이 아니라 그 위 마일스톤
    /// 전체가 읽음으로 적혔다 — `SPC m g` 이 `SPC m a` 가 되는 자리고, 되돌릴 길은 도구 밖에만 있다.
    #[test]
    fn reading_a_group_takes_the_group_the_row_lives_in() {
        let mut is = nested();
        // 마일스톤 밑에 에픽 밖의 줄을 하나 더 둔다 — 마일스톤을 읽었는지 이것으로 가른다.
        let mut loose = make("argos-0009", Kind::Issue);
        loose.milestone = Some("argos-0001".into());
        is.push(loose);
        let mut a = App::new(is, cfg(), Path::new());
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.now = "2026-09-13T13:42:07Z".into();
        a.me = Some("레이븐 (raven@example.com)".into());
        a.recount_unread();
        // 마일스톤 안에 서서 에픽을 펼치고, 그 멤버 줄에 선다.
        a.key(key(KeyCode::Enter));
        assert_eq!(a.site.path.len(), 1, "시험의 전제 — 마일스톤 안이다");
        a.key(key(KeyCode::Char('l')));
        a.cursor = a
            .rows()
            .iter()
            .position(|r| matches!(r, Row::Item(Seat::Here, e, t) if t.depth() == 1 && e.at().is_some_and(|at| a.site.issues[at].id == "argos-0003")))
            .expect("펼친 멤버 줄이 없다");
        a.hit("SPC m g");
        let left: Vec<&str> = a.site.unread.iter().map(String::as_str).collect();
        assert_eq!(left, ["argos-0001", "argos-0009"], "멤버 줄에서 누른 것이 마일스톤을 통째로 읽었다");
    }

    /// **바구니도 이슈 폴더도 묶음이 아니다**(moai-j038.vna). 마일스톤이 하나라도 있으면 에픽 없는 줄과
    /// 마일스톤 없는 에픽은 `(마일스톤 없음)` 에 서는데, 경로를 그대로 묶음으로 읽던 때는 거기 선 줄에서
    /// 누른 `SPC m g` 이 마일스톤 밖의 안 읽은 것을 통째로 적었다 — 뿌리에서 막은 것과 같은 일이다. 멤버
    /// 줄에서 누르면 **묶음 줄 자신도** 읽고(`moai read -e` 와 같은 자 — `nav::Index::under_group`), 자식
    /// 있는 멤버의 폴더 안에서 눌러도 그 폴더가 아니라 그것이 든 에픽이다.
    #[test]
    fn reading_a_group_skips_the_buckets_and_takes_the_group_row() {
        let parent = member("argos-0003", "argos-0001");
        let issues = vec![
            make("argos-0100", Kind::Milestone),
            make("argos-0001", Kind::Epic),
            parent,
            make("argos-0003.aa1", Kind::Issue),
            member("argos-0004", "argos-0001"),
            make("argos-0009", Kind::Issue),
            make("argos-0010", Kind::Issue),
        ];
        let mut a = App::new(issues, cfg(), Path::new());
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.now = "2026-09-13T13:42:07Z".into();
        a.me = Some("레이븐 (raven@example.com)".into());
        a.recount_unread();
        let all = a.site.unread.len();
        let stand = |a: &mut App, path: Path, id: &str| {
            a.site.path = path;
            a.cursor = a
                .rows()
                .iter()
                .position(
                    |r| matches!(r, Row::Item(Seat::Here, e, _) if e.at().is_some_and(|at| a.site.issues[at].id == id)),
                )
                .unwrap_or_else(|| panic!("{id} 줄이 없다"));
        };

        // `(마일스톤 없음)` 안의 에픽 없는 줄 — 아무것도 안 읽고 까닭을 댄다.
        stand(&mut a, vec![Seg::Milestone(None)], "argos-0009");
        a.hit("SPC m g");
        assert_eq!(a.site.unread.len(), all, "바구니를 묶음으로 읽었다 — {:?}", a.site.unread);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("묶음")), "{:?}", a.notice);

        // 자식 있는 멤버의 폴더 안에서 누르면 그것이 든 **에픽과 그 밑 전부, 에픽 줄까지**다.
        stand(
            &mut a,
            vec![Seg::Milestone(None), Seg::Epic("argos-0001".into()), Seg::Issue("argos-0003".into())],
            "argos-0003.aa1",
        );
        a.hit("SPC m g");
        let left: Vec<&str> = a.site.unread.iter().map(String::as_str).collect();
        assert_eq!(left, ["argos-0009", "argos-0010", "argos-0100"], "에픽을 다 못 읽었거나 밖을 읽었다");
    }

    /// **`r` 은 `moai read <id>` 와 같은 자다**(moai-j038.vna) — 내게 온 줄이 아니어도, 누군지 몰라도 그
    /// 줄을 적는다. 이미 읽은 줄은 **락 안에서 읽은 파일의 표**로 가려 다시 안 적는다 — 옆 터미널의
    /// `moai read` 가 적은 새 도장을 이 화면이 든 낡은 줄의 도장으로 덮지 않고, 적고 나면 그 표를 들어 [NEW] 가 따라 걷힌다.
    #[test]
    fn r_marks_like_the_cli_and_never_rewinds_a_mark_written_elsewhere() {
        let s = Scratch::new("read-marks-tui");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        // **읽음은 그 프로젝트의 제 파일에 산다**(moai-bwce) — 뿌리를 알아야 그 파일을 고른다.
        let sheet = crate::read_marks::path_for(&config, &root);
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.site.now = "2026-09-13T13:42:07Z".into();
        a.me = None;
        a.recount_unread();
        assert!(a.site.unread.is_empty(), "누군지 모르는데 [NEW] 가 섰다");

        // 누군지 몰라도 `r` 은 그 줄을 적는다 — CLI 의 `moai read <id>` 가 그러듯.
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert_eq!(a.notice.as_deref(), Some("✓ 읽음 · argos-0009"));
        // 적히는 것은 누른 때가 아니라 **본 줄의 도장**이다(moai-lyc1).
        let stamp = a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        assert!(std::fs::read_to_string(&sheet).unwrap().contains(&format!("argos-0009 = \"{stamp}\"")));
        assert!(!config.exists(), "읽음을 설정 파일에 적었다 — 그 자리는 옛 `[read]` 뿐이다(moai-omx7)");

        // 옆 터미널이 더 늦은 도장으로 적어 두었다 — 이 화면은 그것을 모른다.
        let later = format!(
            "path = {:?}\n\n[read]\nargos-0001 = \"2026-09-14T00:00:00Z\"\nargos-0009 = \"{stamp}\"\n",
            root.display().to_string()
        );
        std::fs::write(&sheet, &later).unwrap();
        a.me = Some("레이븐 (raven@example.com)".into());
        a.recount_unread();
        assert!(a.site.unread.contains("argos-0001"), "화면의 표가 옆에서 적은 것을 벌써 안다 — 시험의 전제가 틀렸다");
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0001").expect("줄이 없다");
        a.hit("r");
        assert_eq!(a.notice.as_deref(), Some("읽음으로 적을 것이 없다"), "이미 읽은 줄을 다시 적었다");
        assert_eq!(std::fs::read_to_string(&sheet).unwrap(), later, "옆에서 적은 새 도장을 낡은 도장으로 덮었다");
        assert!(!a.site.unread.contains("argos-0001"), "적은 뒤 파일의 표를 안 들었다");

        // 옆에서 적은 읽음은 **누르지 않아도** 든다 — 걸음이 **읽음 파일의** 표식을 잰다(moai-en4u 가
        // 설정 표식으로 열어 둔 길을, 읽음이 설정 밖으로 나가며 옮겼다 — moai-bwce).
        // 첫 걸음은 재기만 한다: 띄울 때 이미 읽었다.
        a.follow();
        assert!(a.site.unread.contains("argos-0002"));
        std::fs::write(&sheet, format!("{later}argos-0002 = \"2026-09-14T00:00:00Z\"\n")).unwrap();
        assert!(a.site.unread.contains("argos-0002"), "걸음 전에 들었다 — 시험의 전제가 틀렸다");
        a.follow();
        assert!(!a.site.unread.contains("argos-0002"), "옆에서 적은 읽음을 걸음이 안 들었다");
    }

    /// **`r` 은 옛 철자 파일의 읽음을 화면에서 지우지 않는다**(moai-f5e3 리뷰). 쓴 뒤에 화면에 들이는 표를
    /// 이 자리가 따로 겹치던 판은 `read_marks::read` 보다 한 층이 모자라, 옛 철자로 선 파일에만 있는 읽음이
    /// `r` 한 번에 [NEW] 로 돌아왔다.
    ///
    /// **적을 것이 없는 판이 더 나쁘다.** 이미 읽은 줄에 누르면 파일이 안 바뀌어 표식도 그대로라,
    /// [`App::follow_read`] 가 영영 같다고 보아 그 [NEW] 가 이 세션 내내 섰다 — 겹치는 자를 둘로 두면
    /// 안 된다는 것이 이 줄이다.
    #[test]
    fn the_r_key_keeps_what_an_old_spelling_left() {
        let s = Scratch::new("tui-read-past");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();

        // 옛 바이너리가 안 푼 철자로 적어 둔 파일 — 끝 `/` 하나가 딴 이름을 냈다.
        let slashed = s.path().join("proj/");
        let old = crate::store::dir_of(&config)
            .join("read")
            .join(format!("{:016x}.toml", crate::text::fnv1a64(slashed.as_os_str().as_encoded_bytes())));
        assert_ne!(old, crate::read_marks::path_for(&config, &slashed), "시험의 전제 — 옛 이름과 새 이름이 다르다");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(
            &old,
            format!("path = {:?}\n\n[read]\n\"argos-0001\" = \"옛 도장\"\n", slashed.display().to_string()),
        )
        .unwrap();

        // 탐색기는 그 철자(등록 줄의 것)로 든다.
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(slashed.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = None;
        a.load_read();
        assert_eq!(
            a.site.seen.get("argos-0001").map(String::as_str),
            Some("옛 도장"),
            "읽기가 옛 철자 파일을 안 들었다"
        );

        // 아직 안 읽은 줄에 누른다 — 파일이 바뀌는 판.
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert_eq!(
            a.site.seen.get("argos-0001").map(String::as_str),
            Some("옛 도장"),
            "`r` 한 번에 옛 철자 파일의 읽음이 화면에서 사라졌다 — {:?}",
            a.site.seen
        );

        // 이미 읽은 줄에 다시 누른다 — 적을 것이 없어 파일이 안 바뀌는 판.
        a.hit("r");
        assert_eq!(a.notice.as_deref(), Some("읽음으로 적을 것이 없다"));
        assert_eq!(a.site.seen.get("argos-0001").map(String::as_str), Some("옛 도장"), "{:?}", a.site.seen);
    }

    /// **CLI 와 탐색기가 한 자리에 적는다**(moai-bwce) — `r` 이 적은 것을 `moai read` 가 읽는 그 길
    /// ([`crate::read_marks::read`])이 그대로 든다. 쓰는 자 둘 가운데 **하나만** 새 자리로 옮기면 여기가
    /// 붉어진다: 터미널에서 적은 읽음이 떠 있는 탐색기의 [NEW] 를 못 내리고, 그 반대도 그렇다. 이 에픽이
    /// 반으로 갈리는 것을 막는 줄이다.
    ///
    /// 함께 보는 것 둘. **설정 파일에는 안 적는다** — 그 자리는 겹쳐 보는 옛 `[read]` 뿐이다(moai-omx7).
    /// **트래커에 없는 id 는 이 쓰기에서 걷힌다**(moai-dt5q, 사용자 결정 2) — 이 자리는 그 프로젝트의
    /// 줄을 이미 들고 있다. 닫힌 줄은 안 걷는다.
    #[test]
    fn the_key_and_the_cli_write_to_one_place() {
        let s = Scratch::new("read-marks-one-place");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = None;

        // 트래커에 없는 줄이 읽음 파일에 남아 있다 — 지운 이슈의 읽음은 아무도 다시 안 본다.
        crate::read_marks::update(&config, &root, |sheet| {
            sheet.mark(&[("argos-9999".to_string(), "옛것".to_string())].into_iter().collect())
        })
        .unwrap();

        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert_eq!(a.notice.as_deref(), Some("✓ 읽음 · argos-0009"));

        // `moai read` 가 읽는 그 길로 든다 — 같은 파일이라는 말이 곧 이것이다.
        let crate::read_marks::Marks { seen, problems, .. } =
            crate::read_marks::read(&config, &root, &std::collections::BTreeMap::new());
        assert!(problems.is_empty(), "{problems:?}");
        let stamp = a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        assert_eq!(seen.get("argos-0009"), Some(&stamp), "`r` 이 적은 것을 CLI 의 길이 못 든다");
        assert!(!seen.contains_key("argos-9999"), "트래커에 없는 id 를 안 걷었다 — {seen:?}");
        assert!(!config.exists(), "읽음을 설정 파일에 적었다 — 그 자리는 옛 `[read]` 뿐이다");
    }

    /// **자리를 못 풀면 적었다는 말과 함께 그것을 댄다**(moai-ajh2). 쓰는 길이 까닭을 버리던 판은 옛
    /// 철자 자리에 적고도 "✓ 읽음" 만 세워, 눌린 `r` 이 어디에 갔는지 볼 데가 없었다.
    #[test]
    #[cfg(unix)]
    fn marking_says_when_it_could_not_settle_the_root() {
        let s = Scratch::new("read-marks-tui-why");
        let config = s.path().join("user.toml");
        std::fs::write(s.path().join("파일"), "x").unwrap();
        let root = s.path().join("파일/밑");
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(root, cfg()));
        a.user_config = Some(config);
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        let told = a.notice.as_deref().unwrap_or_default();
        assert!(told.starts_with("✓ 읽음 · argos-0009"), "적었다는 말이 없다 — {told}");
        assert!(told.contains("자리를 못 풀어"), "자리를 못 푼 까닭을 안 댔다 — {told}");
    }

    /// **자리 탈을 줄 탈로 부르지 않는다**(moai-hzfu). `Marks::problems` 는 두 갈래를 한 자루에
    /// 담는다 — 자리를 고르다 만난 까닭과 파일 안의 건너뛴 줄이다. 건너뛴 줄이 하나도 없으면 그
    /// 자리 까닭이 곧 `first()` 라 "읽음에 이상한 줄이 있다" 로 이름 붙었고, `r` 이 방금 띄운
    /// "자리를 못 풀어…" 한 줄이 다음 걸음에 그 이름으로 덮였다 — 같은 까닭을 두 이름으로 부르는
    /// 자리다.
    ///
    /// **줄 탈이 있으면 그쪽이 먼저다** — 이 파일을 정말 읽고 만난 것이라 고칠 자리가 또렷하다.
    #[test]
    #[cfg(unix)]
    fn a_place_trouble_is_not_called_a_line_trouble() {
        let s = Scratch::new("read-marks-place-name");
        let config = s.path().join("user.toml");
        std::fs::write(s.path().join("파일"), "x").unwrap();
        let root = s.path().join("파일/밑");
        let mut a = app();
        a.user_config = Some(config.clone());

        // 자리 까닭만 있다 — 읽음 파일은 아예 없다(건너뛸 줄이 없다).
        let got = a.read_marks_of(&root).expect("설정 자리를 줬는데 안 들었다");
        assert!(got.where_why.as_deref().is_some_and(|w| w.contains("자리를 못 풀어")), "{:?}", got.where_why);
        assert!(got.marks.problems.is_empty(), "자리 까닭이 줄 탈 자루에 남았다 — {:?}", got.marks.problems);
        let told = App::take_read(&mut a.site, got).unwrap_or_default();
        assert!(told.starts_with("읽음 자리를 못 풀었다"), "자리 탈을 제 낱말로 안 댄다 — {told}");

        // 건너뛴 줄이 있으면 그쪽이 먼저다 — 자리 까닭은 뒤로 물러난다.
        let sheet = crate::read_marks::path_for(&config, &root);
        std::fs::create_dir_all(sheet.parent().unwrap()).unwrap();
        // 때가 낱말이 아닌 줄 하나 — 그 줄만 건너뛰고 나머지는 든다(`trouble` 이 안 선다).
        // `path` 는 이 뿌리여야 문지기를 지난다(`Sheet::owns`) — 자리를 못 풀었으니 적힌 철자다.
        let body = format!("path = {:?}\n\n[read]\n\"argos-0009\" = 1\n", root.display().to_string());
        std::fs::write(&sheet, body).unwrap();
        let got = a.read_marks_of(&root).expect("읽음 파일을 놓고도 안 들었다");
        assert!(got.where_why.is_some(), "자리 까닭이 사라졌다");
        assert!(!got.marks.problems.is_empty(), "건너뛴 줄을 안 댔다");
        let told = App::take_read(&mut a.site, got).unwrap_or_default();
        assert!(told.starts_with("읽음에 이상한 줄이 있다"), "줄 탈보다 자리 탈을 먼저 댔다 — {told}");
    }

    /// **이 화면이 트래커 전부를 못 봤으면 안 걷는다**(리뷰). 겹쳐 보기를 끄면 `site.issues` 에서 옆
    /// 워크트리의 줄이 빠지는데, 켜고 찍은 도장은 같은 파일에 있다 — 그대로 걷던 판은 `w` 를 껐다
    /// 켜는 것만으로 그 도장이 사라지고 [NEW] 가 도로 섰다. 조용한 손실이라 못 견딘다(CLAUDE.md).
    #[test]
    fn pruning_waits_until_this_screen_has_seen_the_whole_tracker() {
        let s = Scratch::new("read-marks-narrow");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let elsewhere = [("argos-9999".to_string(), "옆에서 찍은 것".to_string())].into_iter().collect();
        let sheet = || crate::read_marks::read(&config, &root, &std::collections::BTreeMap::new()).seen;

        // 겹쳐 보기가 꺼져 있다 — 옆 워크트리의 줄은 이 목록에 없다.
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.worktree = false;
        crate::read_marks::update(&config, &root, |sh| sh.mark(&elsewhere)).unwrap();
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert!(sheet().contains_key("argos-9999"), "겹쳐 보기가 꺼진 채로 옆 워크트리의 도장을 걷었다");

        // 못 읽는 줄이 제 id 조차 안 내놓는다 — 무엇을 지킬지 모른다.
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.site.unreadable = vec![None];
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert!(sheet().contains_key("argos-9999"), "id 를 못 읽은 줄이 있는데 걷었다");

        // 둘 다 서면 걷는다 — 걷기 자체가 죽은 것이 아니다(moai-dt5q, 사용자 결정 2).
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert!(!sheet().contains_key("argos-9999"), "다 보고도 안 걷었다");
    }

    /// **낱말이 아닌 줄 하나가 이 화면의 읽음을 통째로 버리지 않는다**(리뷰). 읽기는 그 줄만 건너뛰고
    /// 나머지를 내는데(`read_marks::read_table`), 까닭이 하나라도 서면 안 들이던 판은 손으로 적은 맨
    /// 점 키 하나에 내게 온 줄이 모두 [NEW] 로 섰다 — 같은 파일을 읽는 `moai read` 는 멀쩡했다.
    #[test]
    fn one_odd_row_in_the_sheet_does_not_blank_the_screen() {
        let s = Scratch::new("read-marks-lenient-tui");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        let stamp = a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        let at = crate::read_marks::path_for(&config, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            &at,
            format!(
                "path = {:?}\n\n[read]\n\"argos-0009\" = \"{stamp}\"\nargos-0002 = 3\n",
                root.display().to_string()
            ),
        )
        .unwrap();

        a.load_read();
        assert_eq!(a.site.seen.get("argos-0009"), Some(&stamp), "성한 줄까지 버렸다 — {:?}", a.site.seen);
        assert!(!a.site.unread.contains("argos-0009"), "읽은 줄에 [NEW] 가 섰다");
        assert!(a.notice.as_deref().is_some_and(|n| n.starts_with("읽음에 이상한 줄이 있다")), "{:?}", a.notice);
    }

    /// **읽음을 못 들었으면 [NEW] 를 안 센다**(moai-2gep) — "누군지 모르면 아무것도 안 센다" 와 같은
    /// 자다. 빈 표로 세면 내게 온 줄이 모두 [NEW] 로 서는데 그것은 "다 안 읽었다" 가 아니라 "모른다"
    /// 이고, 그 화면의 `SPC m a` 한 번이 모르는 것을 다 읽음으로 찍는다.
    ///
    /// 파일이 **없는** 것은 여기 안 든다 — 아직 아무것도 안 읽은 프로젝트의 정상이라 모든 줄이
    /// [NEW] 인 것이 맞다.
    #[test]
    #[cfg(unix)]
    fn a_sheet_we_could_not_read_counts_no_new_at_all() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("read-marks-unknown");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        // 파일이 없는 것은 탈이 아니다 — 그때는 다 [NEW] 가 맞다.
        a.load_read();
        assert!(a.site.unread.contains("argos-0009"), "시험의 전제 — 안 읽은 줄이 [NEW] 로 선다");

        let at = crate::read_marks::path_for(&config, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, format!("path = {:?}\n\n[read]\n", root.display().to_string())).unwrap();
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&at).is_ok() {
            // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다.
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
            return;
        }
        a.load_read();
        // 되돌리는 것이 **견줌보다 앞이다**(리뷰) — 뒤에 두면 견줌이 진 판에서 0o000 인 파일이 그대로
        // 남는다. 옆의 `entering_a_project_carries_the_read_marks_the_row_already_held` 와 같은 자다.
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(a.site.read_tried.trouble, Some(crate::user_config::Trouble::Unreadable), "시험의 전제 — 못 읽었다");
        assert!(a.site.unread.is_empty(), "모르는 것을 안 읽은 것으로 셌다 — {:?}", a.site.unread);
    }

    /// **설정을 아직 안 만든 기계에서도 [NEW] 가 선다**(리뷰) — "읽음을 못 들었으면 안 센다"
    /// ([`App::recount_unread_in`])가 삼키던 자리다. 없는 설정은 [`crate::user_config::Trouble::Gone`]
    /// 이고([`crate::user_config::read`]), 띄우는 길이 그것을 [`App::config_tried`] 에 적는다 —
    /// 그러면 [`App::read_marks_of`] 가 **없는 읽음 파일까지** `Gone` 으로 올려(홈이 끊긴 것과 못
    /// 가르니), 처음 띄운 사람의 화면에서 [NEW] 가 통째로 사라졌다. 까닭도 없다: `Gone` 은
    /// `problems` 가 비어 알림 한 줄도 안 선다.
    ///
    /// 갈래를 가려 푼다 — 읽기가 혼자 세우는 갈래(`Unreadable`·`Reading`·`Broken`)만 "모른다" 로 든다.
    #[test]
    fn a_machine_that_never_made_a_config_still_marks_new() {
        let s = Scratch::new("read-marks-no-config");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        // 띄우는 길이 하는 것 — 설정을 한 번 읽어 갈래를 넘긴다(`cmd::tui`).
        let reg = crate::user_config::read(Some(&config));
        assert_eq!(reg.trouble, Some(crate::user_config::Trouble::Gone), "시험의 전제 — 없는 설정은 Gone 이다");
        a.config_tried.saw(reg.trouble);
        a.load_read();
        assert_eq!(
            a.site.read_tried.trouble,
            Some(crate::user_config::Trouble::Gone),
            "시험의 전제 — 읽음도 Gone 으로 올랐다"
        );
        assert!(
            a.site.unread.contains("argos-0009"),
            "설정 없는 기계에서 [NEW] 가 한 줄도 안 섰다 — {:?}",
            a.site.unread
        );
    }

    /// **홈이 끊기면 읽음도 지난 것을 들고 선다**(moai-4qbv.i0g 리뷰). 읽음 파일은 설정 파일 곁의
    /// 디렉터리에 살아([`crate::read_marks::path_for`]) autofs·sshfs 홈이 끊기면 둘이 함께 사라진다.
    /// 설정만 지키고 읽음을 "아직 아무것도 안 읽은 프로젝트" 로 들면 내 줄이 통째로 [NEW] 로 서고,
    /// 그 화면에서 `SPC m a` 한 번이 프로젝트를 통째로 읽음으로 찍는다 — moai-po6v 가 층에 대해 막은
    /// 그 해가 읽음 쪽에 그대로 남아 있었다.
    ///
    /// **없는 설정과 갈리는 자는 들고 있던 표다** — 위 이웃(`a_machine_that_never_made_a_config_still_marks_new`)
    /// 이 그 반대쪽을 잰다. 둘 다 `Gone` 인데 한쪽은 [NEW] 가 서고 한쪽은 안 선다.
    #[test]
    fn a_home_that_dropped_does_not_turn_every_line_new() {
        let s = Scratch::new("read-marks-home-gone");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        std::fs::write(&config, "[[project]]\npath = \"/a\"\n").unwrap();
        let stamp = &a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        let at = crate::read_marks::path_for(&config, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            &at,
            format!("path = {:?}\n\n[read]\n\"argos-0009\" = \"{stamp}\"\n", root.display().to_string()),
        )
        .unwrap();
        a.load_read();
        assert!(!a.site.unread.contains("argos-0009"), "시험의 전제 — 읽음을 들었다");
        a.follow();
        assert!(a.config_tried.trouble.is_none(), "시험의 전제 — 설정이 멀쩡하다");

        // 홈이 통째로 끊긴다 — 설정도 읽음도 함께 사라진다.
        std::fs::remove_file(&at).unwrap();
        std::fs::remove_file(&config).unwrap();
        a.follow();
        assert_eq!(a.config_tried.trouble, Some(crate::user_config::Trouble::Gone), "설정이 사라진 것을 못 봤다");
        assert!(!a.site.unread.contains("argos-0009"), "사라진 읽음의 빈 표를 들여 읽은 줄이 [NEW] 로 섰다");
        assert_eq!(
            a.site.read_tried.trouble,
            Some(crate::user_config::Trouble::Gone),
            "읽음만 사라진 것과 홈이 끊긴 것을 안 갈랐다"
        );

        // 돌아오면 표식이 그것을 낸다.
        std::fs::write(&config, "[[project]]\npath = \"/a\"\n").unwrap();
        std::fs::write(
            &at,
            format!("path = {:?}\n\n[read]\n\"argos-0009\" = \"{stamp}\"\n", root.display().to_string()),
        )
        .unwrap();
        a.follow();
        assert!(a.config_tried.trouble.is_none() && a.site.read_tried.trouble.is_none(), "돌아왔는데 탈이 남았다");
        assert!(!a.site.unread.contains("argos-0009"), "돌아온 읽음을 안 들었다");
    }

    /// **대기 자리에 적힌 도장도 걸음이 따라간다**(moai-65as). moai-bdej 가 못 푼 판의 도장을
    /// `read/<받은 철자>.pending.toml` 로 보내고 다음 성한 쓰기가 합치게 했는데, 걸음은 쓰는 자리
    /// 하나만 재고 있었다 — 옆 터미널의 `moai read` 가 `ESTALE` 을 한 번 만나 그 자리에 적으면 지금
    /// 자리는 그대로라, 합쳐질 줄이 화면에 내내 [NEW] 로 섰다.
    ///
    /// **재는 자리를 [`ReadStamp::of`] 하나로 모은 것이 그 고침이다** — 읽기가 여는 파일과 걸음이 재는
    /// 파일이 한 자리에서 나온다. 여기서 재는 것은 그 둘이 갈렸는지다.
    #[test]
    fn a_stamp_in_the_pending_place_reaches_the_screen() {
        let s = Scratch::new("read-marks-pending");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        // [NEW] 는 내게 온 줄에 선다 — 안 맡기면 셀 줄이 하나도 없어 전제부터 안 선다.
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        let mark = |id: &str| {
            let stamp = &a.site.issues.iter().find(|i| i.id == id).unwrap().updated_at;
            format!("\"{id}\" = \"{stamp}\"\n")
        };
        let (nine, two) = (mark("argos-0009"), mark("argos-0002"));
        let head = format!("path = {:?}\n\n[read]\n", root.display().to_string());
        let place = crate::read_marks::place_of(&config, &root);
        std::fs::create_dir_all(place.at.parent().unwrap()).unwrap();
        std::fs::write(&place.at, format!("{head}{nine}")).unwrap();
        a.load_read();
        assert!(!a.site.unread.contains("argos-0009"), "시험의 전제 — 쓰는 자리의 읽음을 들었다");
        assert!(a.site.unread.contains("argos-0002"), "시험의 전제 — 아직 안 읽은 줄이 있다");

        // 옆에서 돈 `moai read` 가 자리를 못 풀어 대기 자리에 적었다. **쓰는 자리는 그대로다** —
        // 그것만 재던 걸음은 여기서 아무것도 안 했다.
        let spool = place.pending.clone().expect("성한 판에는 대기 자리가 선다");
        let was = crate::store::stamp(&place.at);
        std::fs::write(&spool, format!("{head}{two}")).unwrap();
        assert_eq!(crate::store::stamp(&place.at), was, "시험의 전제 — 쓰는 자리는 안 바뀌었다");

        a.follow();
        assert!(!a.site.unread.contains("argos-0002"), "대기 자리에 적힌 도장이 화면에 안 닿았다");
    }

    /// **읽음 파일도 다시 읽을 때를 갈래로 잰다**(moai-po6v) — 설정과 **한 자다**([`App::follow_config`],
    /// [`crate::user_config::Again`]). 갈라 두면 같은 `Trouble` 을 두 곳이 달리 읽어, 한쪽을 고친 날
    /// 다른 쪽이 조용히 옛 뜻으로 남는다.
    ///
    /// 권한으로 못 읽는 것은 다시 해도 같아 걸음마다 읽으면 헛돌고, 고치는 `chmod` 은 고친 때도 길이도
    /// 안 바꿔 표식만 보면 영영 못 벗어난다 — 그래서 시계로 다시 본다. 들고 있던 표는 그동안 그대로다:
    /// 빈 표를 들이면 내 줄이 통째로 [NEW] 로 선다.
    #[test]
    #[cfg(unix)]
    fn a_sheet_we_cannot_read_again_is_retried_by_the_clock() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new("read-marks-clock");
        let config = s.path().join("user.toml");
        let root = s.path().join("proj");
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::at(root.clone(), cfg()));
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());

        let mark = |id: &str| {
            let stamp = &a.site.issues.iter().find(|i| i.id == id).unwrap().updated_at;
            format!("\"{id}\" = \"{stamp}\"\n")
        };
        let (nine, two) = (mark("argos-0009"), mark("argos-0002"));
        let at = crate::read_marks::path_for(&config, &root);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        let head = format!("path = {:?}\n\n[read]\n", root.display().to_string());
        std::fs::write(&at, format!("{head}{nine}")).unwrap();
        a.load_read();
        assert!(!a.site.unread.contains("argos-0009"), "시험의 전제 — 읽음을 들었다");
        assert!(a.site.unread.contains("argos-0002"), "시험의 전제 — 아직 안 읽은 줄이 있다");

        // 옆 터미널이 한 줄 더 적었는데 그 읽기가 진다. 표식은 바뀌었다 — 읽기는 한다.
        std::fs::write(&at, format!("{head}{nine}{two}")).unwrap();
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&at).is_ok() {
            // 권한이 안 먹는 자리(root)에서는 흉내 낼 수 없다.
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
            return;
        }
        a.follow();
        assert_eq!(
            a.site.read_tried.trouble,
            Some(crate::user_config::Trouble::Unreadable),
            "권한을 잠깐의 실패로 읽었다"
        );
        assert_eq!(
            a.site.read_stamp,
            Some(ReadStamp::of(&crate::read_marks::place_of(&config, &root))),
            "다시 해도 같은 갈래인데 표식을 물렸다 — 걸음마다 헛 읽는다"
        );
        assert!(!a.site.unread.contains("argos-0009"), "못 읽은 빈 표를 들여 읽은 줄이 [NEW] 로 섰다");

        // 권한만 되돌린다 — 파일은 그대로라 표식도 그대로다.
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        a.follow();
        assert!(a.site.unread.contains("argos-0002"), "다시 해도 같은 갈래를 걸음마다 다시 읽었다");

        a.site.read_tried.at =
            Some(std::time::Instant::now().checked_sub(layer::REREAD_EVERY).expect("시계가 1분도 안 돌았다"));
        a.follow();
        assert!(!a.site.unread.contains("argos-0002"), "시계가 돌았는데 다시 안 읽었다");
        assert_eq!(a.site.read_tried.trouble, None, "읽혔는데 탈이 남았다");
    }

    /// **읽음은 트래커가 사는 뿌리로 고른다 — 선 체크아웃이 아니다**(리뷰). 딸린 워크트리에서 띄우면
    /// `App::here()` 는 그 워크트리고 `repo.root` 는 루트다(`Repo::here`). 읽는 자만 `here()` 로 고르던
    /// 판은 `r` 이 루트의 파일에 적고 걸음이 워크트리의 (없는) 파일을 읽어, [NEW] 가 내렸다가 한 걸음
    /// 뒤에 도로 섰다 — 이 저장소는 일을 모두 워크트리에서 하므로 그때가 늘이다.
    #[test]
    fn the_sheet_is_keyed_by_the_tracker_root_not_the_checkout() {
        let s = Scratch::new("read-marks-worktree");
        let config = s.path().join("user.toml");
        let (root, worktree) = (s.path().join("proj"), s.path().join("proj/.claude/worktrees/w1"));
        let mut a = app();
        for i in &mut a.site.issues {
            i.assignee = Some("레이븐".into());
            i.assignee_email = Some("raven@example.com".into());
        }
        a.site.repo = Some(crate::store::Repo::moved(root.clone(), cfg(), worktree.clone()));
        assert_ne!(a.here().as_deref(), Some(root.as_path()), "시험의 전제 — 선 자리와 뿌리가 갈렸다");
        a.user_config = Some(config.clone());
        a.me = Some("레이븐 (raven@example.com)".into());
        a.recount_unread();

        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");
        assert!(!a.site.unread.contains("argos-0009"), "적고도 [NEW] 가 안 내렸다");
        // 걸음이 **적은 그 파일**을 본다 — 선 자리로 보던 판은 없는 파일을 읽어 방금 내린 [NEW] 를 도로 세웠다.
        a.follow();
        assert!(!a.site.unread.contains("argos-0009"), "걸음이 방금 적은 읽음을 도로 지웠다");
        let empty = std::collections::BTreeMap::new();
        let stamp = a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        assert_eq!(
            crate::read_marks::read(&config, &root, &empty).seen.get("argos-0009"),
            Some(&stamp),
            "뿌리의 파일에 안 적었다"
        );
        assert!(!crate::read_marks::path_for(&config, &worktree).exists(), "워크트리 자리에 읽음 파일을 지었다");
    }

    /// **한눈 보기의 줄은 그 프로젝트의 파일에 적는다**(moai-bwce) — `App` 의 맵 하나를 같이 보던 판은
    /// 이름이 같은 두 저장소의 같은 id 가 서로의 [NEW] 를 내렸다(moai-omx7 의 화면 쪽 모습이다).
    #[test]
    fn a_row_from_another_project_marks_that_projects_sheet() {
        let s = Scratch::new("read-marks-seat");
        let config = s.path().join("user.toml");
        let (here, other) = (s.path().join("a/api"), s.path().join("b/api"));
        let mut a = app();
        a.site.repo = Some(crate::store::Repo::at(here.clone(), cfg()));
        a.user_config = Some(config.clone());

        // 남의 프로젝트에 같은 id 로 읽음이 적혀 있다 — 이쪽 쓰기가 그것을 밀면 안 된다.
        crate::read_marks::update(&config, &other, |sheet| {
            sheet.mark(&[("argos-0009".to_string(), "남의 것".to_string())].into_iter().collect())
        })
        .unwrap();

        a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
        a.hit("r");

        let empty = std::collections::BTreeMap::new();
        let stamp = a.site.issues.iter().find(|i| i.id == "argos-0009").unwrap().updated_at.clone();
        assert_eq!(crate::read_marks::read(&config, &here, &empty).seen.get("argos-0009"), Some(&stamp));
        assert_eq!(
            crate::read_marks::read(&config, &other, &empty).seen.get("argos-0009").map(String::as_str),
            Some("남의 것"),
            "남의 프로젝트의 읽음을 밀었다"
        );
    }

    /// **같은 id 의 쌍둥이 줄은 늦은 도장으로 읽는다**(moai-7c50.exy) — `r`·`SPC m a` 가 가리킨 뒷줄의
    /// 도장만 적으면 [NEW] 를 세운 앞줄이 늘 더 늦어, 몇 번을 눌러도 "읽음으로 적을 것이 없다" 만 나온다.
    #[test]
    fn r_on_a_duplicate_id_clears_new_for_both_twins() {
        let mut early = make("argos-0009", Kind::Issue);
        early.updated_at = "2026-09-05T00:00:00Z".into();
        let issues = vec![make("argos-0001", Kind::Epic), early, make("argos-0009", Kind::Issue)];
        for act in ["r", "SPC m a"] {
            let mut a = App::new(issues.clone(), cfg(), Path::new());
            for i in &mut a.site.issues {
                i.assignee = Some("레이븐".into());
                i.assignee_email = Some("raven@example.com".into());
            }
            a.me = Some("레이븐 (raven@example.com)".into());
            a.recount_unread();
            assert!(a.site.unread.contains("argos-0009"), "{act}: 시험의 전제가 틀렸다 — {:?}", a.site.unread);
            a.cursor = row_ids(&a).iter().position(|id| id == "argos-0009").expect("줄이 없다");
            a.hit(act);
            assert!(!a.site.unread.contains("argos-0009"), "{act}: 쌍둥이의 [NEW] 가 안 내렸다 — {:?}", a.site.seen);
            assert_eq!(a.site.seen.get("argos-0009").map(String::as_str), Some("2026-09-05T00:00:00Z"), "{act}");
        }
    }

    /// **바로 누르던 키는 더는 뜻이 없다**(moai-7sjm) — `f`·`n`·`w`·`a`·`d`·`m`·Delete·F키. 목록·
    /// 상세 포커스 모두, 모드도 토글도 알림도 그대로다.
    ///
    /// `r` 은 빠졌다 — 사용자 결정으로 **읽음**이 그 자리를 받았다(moai-z9pc). 그 키는 제 시험이 본다.
    #[test]
    fn the_old_direct_keys_no_longer_act() {
        let codes = [
            KeyCode::Char('f'),
            KeyCode::Char('n'),
            KeyCode::Char('w'),
            KeyCode::Char('a'),
            KeyCode::Char('d'),
            KeyCode::Char('m'),
            KeyCode::Char('q'),
            KeyCode::Delete,
            KeyCode::F(2),
            KeyCode::F(3),
            KeyCode::F(5),
            KeyCode::F(7),
            KeyCode::F(10),
        ];
        for pane in Pane::ALL {
            let mut a = app();
            a.focus = pane;
            for code in codes {
                let (raw, worktree) = (a.raw, a.worktree);
                a.key(key(code));
                assert_eq!(a.mode, Mode::Browse, "{pane:?} {code:?} 가 칸을 열었다");
                assert_eq!(
                    (a.raw, a.worktree, a.quit),
                    (raw, worktree, false),
                    "{pane:?} {code:?} 가 토글·끝내기를 했다"
                );
                assert_eq!(a.notice, None, "{pane:?} {code:?}");
                assert!(!menu::open(&a.chord));
            }
        }
    }

    /// **글칸에서 SPC 는 글자다** — 검색·거름망·폼. 메뉴는 안 열리고 뒤의 `q` 도 글자다.
    #[test]
    fn spc_is_text_in_text_fields() {
        for opener in ["/", "SPC f"] {
            let mut a = app();
            a.hit(opener);
            a.hit("SPC");
            a.key(key(KeyCode::Char('q')));
            assert!(!menu::open(&a.chord) && !a.quit, "{opener}: 글칸의 SPC 가 메뉴를 열었다");
            assert!(matches!(&a.mode, Mode::Grep(q, _) | Mode::Filter(q) if q.text() == " q"), "{:?}", a.mode);
        }
        let mut a = app();
        a.hit("SPC n");
        a.hit("SPC");
        a.key(key(KeyCode::Char('q')));
        assert!(!menu::open(&a.chord) && !a.quit);
        assert!(matches!(&a.mode, Mode::Idea(f) if f.title.text() == " q"), "{:?}", a.mode);
    }

    /// **메뉴가 열린 채 붙여 넣으면 메뉴가 닫힌다** — 붙인 뒤의 `q` 가 옛 SPC 와 이어 끝내지 않게.
    #[test]
    fn a_paste_closes_an_open_menu() {
        let mut a = app();
        a.hit("SPC v");
        a.paste("q");
        assert!(!menu::open(&a.chord), "붙여넣기가 메뉴를 안 닫았다");
        a.key(key(KeyCode::Char('q')));
        assert!(!a.quit, "붙인 뒤의 q 가 옛 SPC 와 이어 끝냈다");
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
        assert!(matches!(a.mode, Mode::Grep(..)));
        typed(&mut a, "0004");
        assert_eq!(a.mode, Mode::Browse);
        assert_eq!(a.filter_text.as_deref(), Some("/0004"));

        // **뿌리에 그것을 품은 에픽이 서고, 걸린 줄이 그 밑에 딸려 선다**(moai-i5io) — 검색이
        // 맞힌 자리는 저절로 열린다. 들어가서 보는 목록도 같은 줄이다.
        let ids = shown(&a);
        assert_eq!(ids, ["argos-0001", "argos-0004"], "{ids:?}");
        a.key(key(KeyCode::Enter));
        assert_eq!(shown(&a), ["argos-0004"]);
    }

    /// **검색 칸은 치는 대로 거른다**(moai-00le). Esc 는 열기 전 거름망으로 되돌리고, Enter 는 건다.
    #[test]
    fn slash_filters_as_you_type_and_esc_puts_back_what_was_there() {
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "type=epic");
        a.key(key(KeyCode::Char('/')));
        for c in "0004".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert_eq!(a.filter_text.as_deref(), Some("/0004"), "치는 동안 안 걸렸다");
        // 걸린 줄은 그것을 품은 에픽 밑에 딸려 선다(moai-i5io) — 셈은 여전히 걸린 줄 하나다.
        assert_eq!((shown(&a), a.hit_count()), (vec!["argos-0001".to_string(), "argos-0004".to_string()], 1));
        a.key(key(KeyCode::Esc));
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"), "Esc 가 열기 전 거름망을 못 돌렸다");
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);

        // 붙여 넣은 글도 치는 것과 같이 거른다. 비우면 거름망이 없다.
        a.key(key(KeyCode::Char('/')));
        a.paste("0009");
        assert_eq!(shown(&a), ["argos-0009"]);
        a.key(key(KeyCode::Backspace));
        a.key(key(KeyCode::Char('9')));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filter_text.as_deref(), Some("/0009"));
        assert_eq!(a.grep_was, None, "Enter 로 건 뒤에도 되돌릴 자리를 들고 있다");
    }

    /// **Tab·Shift-Tab 이 찾을 자리를 돈다**(moai-kojj) — 전체 → id → 제목 → 태그 → 본문. 좁힌 범위는
    /// 뱃지에 `/<범위>:` 로 서고, 다시 읽어도 그 범위로 다시 건다(moai-fmmg).
    #[test]
    fn tab_turns_what_slash_searches_and_a_reread_keeps_it() {
        let mut a = app();
        a.site.issues[4].tags = vec!["parser".into()];
        a.key(key(KeyCode::Char('/')));
        for c in "pars".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        assert_eq!(shown(&a), ["argos-0009"], "전체가 태그를 안 봤다");
        let mut seen = Vec::new();
        for _ in 0..5 {
            a.key(key(KeyCode::Tab));
            let Mode::Grep(_, g) = a.mode else { panic!("{:?}", a.mode) };
            seen.push((g, a.hit_count()));
        }
        assert_eq!(seen, [(GrepIn::Id, 0), (GrepIn::Title, 0), (GrepIn::Tag, 1), (GrepIn::Body, 0), (GrepIn::All, 1)]);
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(matches!(a.mode, Mode::Grep(_, GrepIn::Body)), "Shift-Tab 이 거꾸로 안 돌았다");
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filter_text.as_deref(), Some("/태그:pars"));
        assert_eq!(a.grep_query(), Some((GrepIn::Tag, "pars")));

        // 다시 읽기 — 뱃지 글이 아니라 들고 있는 범위로 다시 짓는다.
        a.reapply();
        assert_eq!(a.filter_text.as_deref(), Some("/태그:pars"));
        assert_eq!(shown(&a), ["argos-0009"]);

        // 거름망(`f`) 칸의 Tab 은 아무 일도 안 한다.
        a.hit("SPC f");
        a.key(key(KeyCode::Tab));
        assert!(matches!(&a.mode, Mode::Filter(q) if q.text().is_empty()), "{:?}", a.mode);
    }

    /// 거름망은 **CLI 와 같은 문법**이다. 없는 항목은 그 자리에서 나무란다.
    #[test]
    fn f_takes_the_same_grammar_as_the_cli() {
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "type=epic");
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"));
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);

        // 잘못 적으면 걸리지 않고 그 자리에 남는다 — 지우고 다시 치게 하지 않는다
        let mut a = app();
        a.hit("SPC f");
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
        a.hit("SPC f");
        typed(&mut a, "grep=원자적 쓰기");
        assert!(a.input_error().is_none(), "{:?}", a.input_error());
        assert_eq!(shown(&a), ["argos-0009"]);

        // 여러 조건은 여전히 띄어쓰기로 잇는다
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "type=epic status=todo");
        assert_eq!(shown(&a), ["argos-0001", "argos-0002"]);
    }

    /// **모르는 칸은 거절한다.** `cmd/show.rs` 가 쓰는 것과 같은 자다 — 조용히
    /// 0건을 내면 `status=in-progress` 같은 오타가 "그 칸은 비었다" 와
    /// 구별되지 않는다.
    #[test]
    fn an_unknown_column_is_refused_not_silently_empty() {
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "status=in-progress");
        assert!(matches!(a.mode, Mode::Filter(_)), "오타인데 걸렸다");
        assert!(a.input_error().is_some_and(|e| e.contains("칸")), "{:?}", a.input_error());
        assert_eq!(a.filter_text, None);
    }

    /// raw mode 에서는 Ctrl-C 가 신호로 오지 않는다. **글을 받는 중에도** 받아야
    /// 한다 — 글자로 먹으면 검색칸에 `c` 가 찍히고 나갈 길이 하나로 줄어든다.
    #[test]
    fn ctrl_c_quits_even_while_typing() {
        for opener in ["/", "SPC f"] {
            let mut a = app();
            a.hit(opener);
            a.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(a.quit, "{opener:?} 중에 Ctrl-C 를 글자로 먹었다");
            assert!(matches!(&a.mode, Mode::Grep(b, _) | Mode::Filter(b) if b.text().is_empty()));
        }
    }

    /// 파일이 **아직 없는** 저장소에서도 생긴 것을 알아챈다. 표식이 없다고
    /// 한 번 걸러 버리면 그 뒤로 영영 못 알아챈다.
    #[test]
    fn a_file_that_appears_later_is_still_noticed() {
        let scratch = scratch("appear");
        let dir = scratch.path().to_path_buf();
        let repo = Repo::at(dir.clone(), cfg());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        assert!(stamp.is_none() && load.issues.is_empty(), "판이 다르다");
        let (index, ground) = measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);

        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        settle(&mut a);
        assert_eq!(a.site.issues.len(), 1, "없던 파일이 생긴 것을 못 알아챘다");
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
        let issues = vec![make("argos-0001", Kind::Epic), closed, member("argos-0003", "argos-0001")];
        let mut a = App::new(issues, cfg(), Path::new());
        assert_eq!(a.site.column(0), "in_progress", "에픽이 제 손으로 걸린다 — 시험이 자손 길을 안 지난다");
        a.hit("SPC f");
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
        let inside = a.site.path.clone();
        a.key(key(KeyCode::Char('/')));
        for code in [KeyCode::Backspace, KeyCode::Left, KeyCode::Char('a'), KeyCode::Char('c'), KeyCode::Left] {
            a.key(key(code));
        }
        a.key(key(KeyCode::Char('b')));
        assert_eq!(a.site.path, inside, "칸의 키가 탐색기로 샜다");
        assert!(matches!(&a.mode, Mode::Grep(b, _) if b.text() == "abc"), "{:?}", a.mode);
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
        assert_eq!(a.mode, Mode::Grep(Input::new("quit"), GrepIn::All));
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
        assert_eq!(a.mode, Mode::Grep(Input::new("qu it"), GrepIn::All), "줄바꿈이 Enter 로 걸렸다");
        a.key(key(KeyCode::Esc));

        a.hit("SPC f");
        a.paste("status=todo");
        assert_eq!(a.mode, Mode::Filter(Input::new("status=todo")));
        a.key(key(KeyCode::Esc));

        a.hit("SPC n");
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
        assert_eq!(a.site.path.len(), 1);

        // 그 에픽이 사라진 자료로 갈아탄다
        a.adopt(vec![make("argos-0002", Kind::Epic), make("argos-0009", Kind::Issue)]);
        assert!(a.site.path.is_empty(), "없는 자리에 그대로 서 있다");
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
        let was = a.site.path.clone();

        let mut with = a.site.issues.clone();
        with.push(make("argos-9999", Kind::Milestone));
        a.adopt(with);
        // 길은 낡았지만 뿌리로 내려놓지는 않는다 — 에픽이 간 자리로 따라간다
        assert_ne!(a.site.path, was, "트리가 깊어졌는데 길이 그대로다");
        assert_eq!(a.site.path.last(), was.last(), "서 있던 에픽을 놓쳤다");
        assert_eq!(a.site.path.len(), 2, "{:?}", a.site.path);
        assert_eq!(a.site.remembered.len(), a.site.path.len());
    }

    /// 갱신해도 걸어 둔 거름망은 살아 있다. 갱신 한 번에 하던 일이 흩어지면
    /// 갱신을 꺼리게 되고, 그러면 낡은 화면을 본다.
    #[test]
    fn reloading_keeps_the_filter() {
        let mut a = app();
        a.hit("SPC f");
        typed(&mut a, "type=epic");
        assert_eq!(shown(&a).len(), 2);

        let mut more = a.site.issues.clone();
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
        let scratch = scratch("reload");
        let dir = scratch.path().to_path_buf();
        let line = |i: &Issue| format!("{}\n", serde_json::to_string(i).unwrap());
        std::fs::write(dir.join(".moai/issues.jsonl"), line(&make("argos-0001", Kind::Epic))).unwrap();

        let repo = Repo::at(dir.clone(), cfg());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);
        assert_eq!(a.site.issues.len(), 1);

        // 아직 아무도 안 건드렸다 — 읽지 않는다. 읽으면 표식이 같아도 매 걸음
        // 저장소를 통째로 다시 세는 것이다.
        let stamp_before = a.site.stamp;
        a.follow();
        assert!(!a.loading(), "안 바뀌었는데 다시 읽으러 갔다");
        assert_eq!(a.site.stamp, stamp_before);

        // 에픽 안에 들어가 있는 동안 밖에서 한 줄 더한다
        a.key(key(KeyCode::Enter));
        let mut src = std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap();
        src.push_str(&line(&member("argos-0004", "argos-0001")));
        std::fs::write(dir.join(".moai/issues.jsonl"), src).unwrap();

        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 읽으러 가지 않았다");
        assert_eq!(a.site.issues.len(), 1, "스레드가 지을 것을 루프에서 읽었다");
        settle(&mut a);
        assert_eq!(a.site.issues.len(), 2, "바뀐 것을 저절로 안 읽었다");
        assert_eq!(a.site.path, [Seg::Epic("argos-0001".into())], "읽고 나서 자리를 잃었다");
        assert!(a.trouble.is_none());
    }

    /// **연 뒤 커밋 표를 스레드에서 짓고, 커밋이 새로 서면 다시 읽은 뒤 새 표를 짓는다**
    /// (moai-a4i0). 연 순간에는 표가 없다 — 여는 읽기가 git 을 기다리지 않는다. 표를 짓는 것은
    /// 파일을 다시 읽는 일이 아니라 `loading` 이 아니다. 루프에서 도는 다시 읽기(쓰기·SPC v w)도
    /// 표를 짓지 않는다 — 그 자리에서 이력을 걸으면 쓸 때마다 화면이 멈춘다.
    #[test]
    fn it_gathers_commits_after_opening_and_again_when_head_moves() {
        let scratch = scratch("commits");
        let dir = scratch.path().to_path_buf();
        let line = |i: &Issue| format!("{}\n", serde_json::to_string(i).unwrap());
        std::fs::write(dir.join(".moai/issues.jsonl"), line(&make("argos-0001", Kind::Epic))).unwrap();
        let git = |msg: &str| crate::git::tests::run_git(&dir, None, &["commit", "-q", "--allow-empty", "-m", msg]);
        crate::git::tests::run_git(&dir, None, &["init", "-q"]);
        git("feat: 처음 (argos-0001)");

        let repo = Repo::at(dir.clone(), cfg());
        // 탐색기가 여는 그대로 — 겹쳐 본 채로 연다(`cmd::tui::run`).
        let g = crate::worktree::gather(&repo, true).unwrap();
        let stamp = stamp_of(&repo);
        let (index, ground) = measure(&g.load.issues, &repo.config);
        let mut a = App::open(repo, g.load, index, ground, Path::new(), stamp).overlaid(
            g.origin,
            crate::tui::said_trouble(&g.trouble, crate::i18n::Lang::Ko),
            g.watched,
            g.swept,
            &g.sides,
            &g.mine,
        );
        assert!(a.site.commits_of("argos-0001").is_empty(), "여는 읽기가 git 을 기다렸다");
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        a.follow();
        assert!(!a.loading(), "표를 짓느라 파일을 다시 읽으러 갔다");
        while a.gathering_commits() {
            assert!(std::time::Instant::now() < until, "표를 다 못 지었다");
            std::thread::sleep(std::time::Duration::from_millis(2));
            a.follow();
        }
        let subjects = |a: &App| a.site.commits_of("argos-0001").iter().map(|c| c.subject.clone()).collect::<Vec<_>>();
        assert_eq!(subjects(&a), ["feat: 처음 (argos-0001)"]);

        // 다시 읽기가 끝난 뒤 표까지 받는다.
        let gathered = |a: &mut App| {
            settle(a);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while a.loading() || a.gathering_commits() {
                assert!(std::time::Instant::now() < until, "표를 다 못 지었다");
                std::thread::sleep(std::time::Duration::from_millis(2));
                a.follow();
            }
        };
        git("fix: 다음 (argos-0001)");
        gathered(&mut a);
        assert_eq!(
            subjects(&a),
            ["fix: 다음 (argos-0001)", "feat: 처음 (argos-0001)"],
            "커밋이 섰는데 표를 새로 안 가져왔다"
        );

        // 겹쳐 보기를 꺼도(`SPC v w`) HEAD 를 지켜본다 — 끈 읽기도 HEAD 표식을 들고 온다.
        a.worktree = false;
        a.reload();
        assert!(a.commits_job.is_none(), "루프에서 도는 다시 읽기가 표를 그 자리에서 지었다");
        assert!(a.site.watched.iter().any(|(p, _)| p.ends_with("HEAD")), "겹쳐 보기를 끄자 HEAD 를 안 지켜본다");
        // 커밋이 안 섰으니 표도 다시 안 짓는다 — 쓰기 하나마다 이력을 통째로 걷지 않는다.
        assert!(!a.gathering_commits(), "표식이 그대로인데 표를 다시 지으러 갔다");
        gathered(&mut a);
        std::thread::sleep(std::time::Duration::from_millis(10));
        git("fix: 끈 뒤 (argos-0001)");
        gathered(&mut a);
        assert_eq!(
            subjects(&a).first().map(String::as_str),
            Some("fix: 끈 뒤 (argos-0001)"),
            "겹쳐 보기를 끄자 HEAD 를 안 지켜본다"
        );

        // **표가 모르던 id 가 줄에 서면 표식이 그대로여도 한 번 더 짓는다**(`App::commit_ids`).
        // 표는 낱말을 준 id 와 견줘 서므로(moai-ynhj), 커밋이 먼저 있고 줄이 나중에 오는 자리
        // (들여온 줄·되살린 파일)는 이것 없이는 다음 커밋이 설 때까지 칸이 빈다.
        git("feat: 줄보다 먼저 (argos-0009)");
        gathered(&mut a);
        assert!(a.site.commits_of("argos-0009").is_empty(), "줄이 없는 id 가 표에 섰다");
        let mut src = std::fs::read_to_string(dir.join(".moai/issues.jsonl")).unwrap();
        src.push_str(&line(&make("argos-0009", Kind::Issue)));
        std::fs::write(dir.join(".moai/issues.jsonl"), src).unwrap();
        gathered(&mut a);
        assert_eq!(
            a.site.commits_of("argos-0009").iter().map(|c| c.subject.clone()).collect::<Vec<_>>(),
            ["feat: 줄보다 먼저 (argos-0009)"],
            "표가 모르던 id 의 커밋 칸이 비었다"
        );
        // 그 뒤로는 id 가 그대로라 쓰기 하나마다 이력을 다시 걷지 않는다.
        a.reload();
        settle(&mut a);
        assert!(!a.gathering_commits(), "id 도 표식도 그대로인데 표를 다시 지으러 갔다");
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
            Some(Row::Item(Seat::Here, e, _)) => a.site.issues[e.at().unwrap()].id.clone(),
            other => format!("{other:?}"),
        };
        assert_eq!(at(&a), "argos-0004");

        // 위에 줄이 하나 생긴다 — 같은 번호는 이제 0003 이다.
        let mut more = a.site.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(at(&a), "argos-0004", "위에 줄이 생기자 커서가 옆 줄로 튀었다");

        // 위의 줄이 사라진다.
        let fewer: Vec<Issue> =
            a.site.issues.iter().filter(|i| i.id != "argos-0000" && i.id != "argos-0003").cloned().collect();
        a.adopt(fewer);
        assert_eq!(at(&a), "argos-0004", "위의 줄이 사라지자 커서가 튀었다");

        // 보던 줄이 사라지면 목록 안의 이웃 자리로.
        let gone: Vec<Issue> = a.site.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert!(a.cursor < a.rows().len(), "목록 밖에 섰다");

        // `..` 에 서 있으면 `..` 에 남는다. 들어가면 첫 줄에 서므로(moai-cm13) `..` 로 올라간다.
        let mut b = app();
        b.key(key(KeyCode::Enter));
        b.key(key(KeyCode::Home));
        let mut more = b.site.issues.clone();
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
        let mut more = a.site.issues.clone();
        more.push(member("argos-0000", "argos-0001"));
        a.adopt(more);
        assert_eq!(a.detail.offset(), 7, "같은 줄인데 굴린 자리를 잃었다");

        let gone: Vec<Issue> = a.site.issues.iter().filter(|i| i.id != "argos-0004").cloned().collect();
        a.adopt(gone);
        assert_eq!(a.detail.offset(), 0, "다른 줄에 섰는데 굴린 자리가 남았다");
    }

    /// 겹쳐 보는 동안에는 **옆 워크트리 스냅샷이 바뀐 것도** 다시 읽을 까닭이다.
    /// 안 바뀌었으면 읽지 않는다.
    #[test]
    fn a_change_in_a_watched_worktree_snapshot_rereads_too() {
        let scratch = scratch("watched");
        let dir = scratch.path().to_path_buf();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        let repo = Repo::at(dir.clone(), cfg());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);

        let other = dir.join("other.jsonl");
        std::fs::write(&other, "").unwrap();
        a.site.watched = vec![(other.clone(), crate::store::stamp(&other))];
        a.site.now = "읽기 전".into();
        settle(&mut a);
        assert_eq!(a.site.now, "읽기 전", "아무것도 안 바뀌었는데 다시 읽었다");

        std::fs::write(&other, "{}\n").unwrap();
        settle(&mut a);
        assert_ne!(a.site.now, "읽기 전", "옆 스냅샷이 바뀐 것을 못 알아챘다");
    }

    /// **사람이 누른 갱신이 스레드의 늦은 결과에 덮이지 않는다.** 갱신을 부르기 전에
    /// 띄운 읽기는 누른 뒤의 파일보다 옛것일 수 있다.
    #[test]
    fn a_manual_reload_drops_the_read_in_flight() {
        let scratch = scratch("inflight");
        let dir = scratch.path().to_path_buf();
        std::fs::write(dir.join(".moai/issues.jsonl"), "").unwrap();
        let repo = Repo::at(dir.clone(), cfg());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);

        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        a.follow();
        assert!(a.loading());
        a.reload();
        assert!(!a.loading(), "사람이 부른 갱신이 짓던 것을 안 버렸다");
        assert_eq!(a.site.issues.len(), 1);
    }

    /// **다른 프로젝트에서 지은 읽기는 안 들인다.** 같은 id 를 쓰는 두 프로젝트에서
    /// 떠난 쪽의 읽기가 늦게 닿으면, 들이는 순간 지금 프로젝트의 화면이 남의 줄이 된다.
    #[test]
    fn a_read_built_for_another_project_is_not_taken() {
        let (mine_dir, mut a) = writable("receive-mine");
        let (theirs, _) = writable("receive-theirs");
        touch_outside(&theirs);
        let other = Repo::at(theirs.path().to_path_buf(), cfg());
        a.receive(prepare(&other, false, a.site.lang));
        assert_eq!(shown(&a), ["argos-0001"], "남의 프로젝트에서 지은 줄을 들였다");
        assert!(a.trouble.is_none());

        // 제 것은 들인다 — 막은 것이 뿌리 견주기이지 받기 자체가 아니다.
        let mine = a.site.repo.clone().unwrap();
        touch_outside(&mine_dir);
        a.receive(prepare(&mine, false, a.site.lang));
        assert_eq!(a.site.issues.len(), 2);
    }

    /// 판 밖에서 한 줄을 더해 다음 `follow` 가 스레드 읽기를 띄우게 한다.
    fn touch_outside(scratch: &Scratch) {
        let file = scratch.join(".moai/issues.jsonl");
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

    /// 옆 워크트리를 **못 찾는** 읽기 — git 밖 프로젝트를 흉내 낸다. 시험 기계의 git 에 기대지 않는다.
    fn lost(repo: &Repo, worktree: bool, lang: crate::i18n::Lang) -> crate::fail::R<Fresh> {
        let mut f = prepare(repo, worktree, lang)?;
        // **진짜 꼴로 흉내 낸다**(리뷰) — [`prepare`] 가 싣는 것은 이미 편 문장이다
        // ([`crate::view::trouble_line`]). 맨 까닭을 실으면 알림이 제 문장을 한 번 더 달아도
        // 시험이 그것을 못 잡는다 — 실제로 그렇게 서 있었다.
        let why = crate::worktree::Trouble::Unfound {
            lost: crate::worktree::Lost::Failed,
            why: "git 저장소가 아니다".to_string(),
        };
        f.unfound = worktree.then(|| crate::view::trouble_line(lang, &why));
        Ok(f)
    }

    /// **사람이 SPC v w 로 켰는데 옆을 못 찾으면 까닭을 한 번 댄다**(moai-d5vn). 시작할 때의
    /// 겹쳐 보기는 시키지 않은 것이라 말하지 않지만, 누른 사람은 아무것도 안 바뀐 화면만 보면
    /// 키가 고장 난 줄 안다. 알림이라 다음 키에 걷힌다 — 경로 줄에 박아 두면 git 밖 프로젝트를
    /// 볼 때마다 줄을 먹는다.
    #[test]
    fn turning_the_overlay_on_says_why_no_worktree_was_found() {
        let (_scratch, mut a) = writable("overlay-lost");
        a.read = lost;
        assert!(a.worktree);
        a.hit("SPC v w Esc");
        assert!(!a.worktree);
        assert_eq!(a.notice, None, "끌 때 까닭을 댔다");
        a.hit("SPC v w");
        assert!(a.worktree);
        let said = a.notice.clone().expect("켰는데 못 찾은 까닭을 안 댄다");
        assert!(said.contains("git 저장소가 아니다") && said.contains("워크트리를 못 찾았다"), "{said}");
        // **같은 말은 한 번만 선다**(리뷰) — 실린 것이 이미 편 문장이라, 알림이 제 문장을 앞에 또
        // 달면 `⎇ 옆 워크트리를 못 찾았다 — 워크트리를 못 찾았다 — …` 가 된다.
        assert_eq!(said.matches("워크트리를 못 찾았다").count(), 1, "같은 말이 두 번 섰다 — {said}");
        a.hit("Esc");
        // 알림은 다음 키에 걷힌다 — 키 없이 부르는 다시 읽기가 새로 대지 않는지만 본다.
        a.notice = None;
        a.reload();
        assert_eq!(a.notice, None, "시키지 않은 다시 읽기가 까닭을 또 댔다");
        // 찾았는데 옆이 비었으면 까닭 없이 없다고만 한다 — 경로 줄이 비어 달리 알 길이 없다.
        a.read = prepare_found;
        a.hit("SPC v w Esc");
        a.hit("SPC v w");
        let said = a.notice.clone().expect("켰는데 겹칠 것이 없다고 안 한다");
        assert!(said.contains("옆 워크트리 없음") && !said.contains("못 찾았다"), "{said}");
    }

    /// **메뉴를 닫는 Esc 는 토글이 낸 알림을 함께 지우지 않는다**(moai-g56h). 상태를 대는 항목은
    /// 메뉴를 열린 채로 두므로(사용자 결정 2026-09-19, moai-osgw), 알림을 읽고 메뉴를 닫는 Esc 가
    /// 곧 "다음 키" 다 — 거기서 걷으면 알림을 낸 키와 지우는 키가 한 누름이 되고, 알림은 메뉴
    /// 창이 떠 있는 동안만 산다. 걷는 것은 메뉴 밖의 다음 키다.
    #[test]
    fn closing_the_menu_keeps_the_notice_the_toggle_left() {
        let (_scratch, mut a) = writable("menu-notice");
        a.read = lost;
        assert!(a.worktree, "시험의 전제 — 겹쳐 보기가 켜진 채로 시작한다");
        a.hit("SPC v w Esc");
        a.hit("SPC v w");
        let said = a.notice.clone().expect("켰는데 못 찾은 까닭을 안 댄다");
        assert!(super::menu::open(&a.chord), "시험의 전제 — 토글을 누르고도 메뉴가 떠 있다");

        a.hit("Esc");
        assert!(!super::menu::open(&a.chord), "Esc 가 메뉴를 안 닫았다");
        assert_eq!(a.notice.as_deref(), Some(said.as_str()), "메뉴를 닫는 Esc 가 알림을 함께 지웠다");

        a.hit("j");
        assert_eq!(a.notice, None, "메뉴 밖의 다음 키가 알림을 안 걷었다");
    }

    /// **다음 키를 기다리는 접두어도 알림을 안 걷는다**(리뷰) — `gg` 의 첫 `g` 와 메뉴를 여는 SPC 가
    /// 그것이다. 메뉴 안에서만 도로 세우던 때는, 고친 Esc 바로 옆에서 같은 실패가 그대로 섰다:
    /// 알림을 남긴 키와 지우는 키가 한 누름이다. **모르는 키는 여태처럼 걷는다** — `Chord::feed`
    /// 가 뜻 없는 키에만 기다리는 열을 버리므로, 그 하나로 둘을 가른다.
    #[test]
    fn a_prefix_waiting_for_its_next_key_keeps_the_notice() {
        let (_scratch, mut a) = writable("chord-notice");
        a.read = lost;
        // 겹쳐 보기를 껐다 켠다 — 켜는 쪽이 못 찾은 까닭을 알림으로 단다.
        let toggled = |a: &mut App| {
            a.hit("SPC v w Esc");
            a.hit("SPC v w Esc");
            a.notice.clone().expect("켰는데 못 찾은 까닭을 안 댄다")
        };

        let said = toggled(&mut a);
        a.hit("g");
        assert!(a.chord.waiting(), "시험의 전제 — `g` 는 다음 키를 기다린다");
        assert_eq!(a.notice.as_deref(), Some(said.as_str()), "기다리는 접두어가 알림을 걷었다");
        a.hit("g");
        assert_eq!(a.notice, None, "열을 끝내 동작이 돈 키가 알림을 안 걷었다");

        // 메뉴를 여는 SPC 도 아무 동작을 안 돈 키다.
        let said = toggled(&mut a);
        a.hit("SPC");
        assert!(super::menu::open(&a.chord), "시험의 전제 — SPC 가 메뉴를 열었다");
        assert_eq!(a.notice.as_deref(), Some(said.as_str()), "메뉴를 여는 SPC 가 알림을 걷었다");

        // **모르는 키는 여태처럼 걷는다** — 기다리는 열을 버리는 것이 그 하나다.
        a.hit("Esc");
        a.hit("z");
        assert_eq!(a.notice, None, "뜻 없는 키가 알림을 안 걷었다");
    }

    fn prepare_found(repo: &Repo, worktree: bool, lang: crate::i18n::Lang) -> crate::fail::R<Fresh> {
        let mut f = prepare(repo, worktree, lang)?;
        f.unfound = None;
        Ok(f)
    }

    fn boom(_: &Repo, _: bool, _: crate::i18n::Lang) -> crate::fail::R<Fresh> {
        panic!("버린 읽기가 터졌다")
    }

    /// **사람이 누른 갱신이 버린 읽기가 패닉하면 다음 걸음이 되던진다.** 패닉 훅은 이미 터미널을
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
        a.reload();
        assert!(!a.loading(), "사람이 부른 갱신이 짓던 것을 안 버렸다");
        assert!(a.reaping(), "버린 손잡이를 안 들었다 — 루프가 빠른 걸음으로 안 깬다");
        assert_eq!(a.site.issues.len(), 2, "사람이 부른 갱신이 제 자리에서 안 읽었다");

        discarded_settle(&a);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.follow()));
        let payload = caught.expect_err("버린 스레드의 패닉을 삼켰다");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"버린 읽기가 터졌다"));
    }

    /// **버린 읽기가 제대로 끝나면 join 만 하고 결과는 안 들인다.** 다시 읽으러 가지도
    /// 않는다 — 누른 갱신이 표식을 이미 올렸다.
    #[test]
    fn a_discarded_read_that_finishes_is_joined_and_ignored() {
        let (scratch, mut a) = writable("discard-ok");
        touch_outside(&scratch);
        a.follow();
        assert!(a.loading());
        a.reload();
        assert!(a.reaping());
        let stamp = a.site.stamp;

        discarded_settle(&a);
        a.follow();
        assert!(!a.reaping(), "끝난 스레드를 join 안 했다");
        assert!(!a.loading(), "버린 읽기가 끝난 것을 보고 또 읽으러 갔다");
        assert_eq!(a.site.stamp, stamp);
        assert_eq!(a.site.issues.len(), 2);
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
        a.reload();
        assert_eq!(a.discarded.len(), DISCARDED_KEPT, "든 손잡이가 상한을 넘었다");
        // **도는 것을 놓았으면 화면이 말한다**(moai-j9on) — 그 스레드가 터지면 터미널이
        // 걷히는데 되던질 손잡이가 없다. 다시 읽기가 걷는 `trouble` 이 아니라 붙박이다:
        // 누른 갱신이 짓는 읽기가 끝나는 순간 걷히면 몇백 ms 뒤에 사라진다.
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

        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.reload()));
        drop(tx);
        let payload = caught.expect_err("꽉 찼을 때 끝난 패닉을 거두지 않고 놓았다");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"가장 오래된 것이 터졌다"));
    }

    /// 쓰기 시험이 쓰는 판 — 진짜 파일에 줄 하나를 두고 연 탐색기. **사람은
    /// `--user` 로 준다** — `MOAI_ACTOR` 를 시험에서 바꾸면 같은 프로세스의 다른
    /// 시험이 그 값을 본다.
    fn writable(name: &str) -> (Scratch, App) {
        let scratch = scratch(name);
        let dir = scratch.path().to_path_buf();
        std::fs::write(
            dir.join(".moai/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&make("argos-0001", Kind::Epic)).unwrap()),
        )
        .unwrap();
        let repo = Repo::at(dir, cfg());
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let (index, ground) = measure(&load.issues, &repo.config);
        let mut a = App::open(repo, load, index, ground, Path::new(), stamp);
        a.user = Some("레이븐 (raven@example.com)".into());
        (scratch, a)
    }

    /// 생각 하나를 담는 쓰기 — 폼이 부를 모양 그대로다.
    fn add_idea(a: &mut App, id: &'static str) -> Option<String> {
        a.write(
            |_| {},
            move |issues, _, _, by| {
                issues.push(Issue::new(
                    id.into(),
                    "떠오른 것".into(),
                    Kind::Idea,
                    Status::new("todo"),
                    "2026-09-13T00:00:00Z",
                ));
                Ok((
                    vec![crate::model::JournalEntry::create(id, "떠오른 것", "2026-09-13T00:00:00Z", by)],
                    Touched { id: id.into(), done: "담김" },
                ))
            },
        )
    }

    /// **쓰면 파일이 바뀌고, 화면은 그 파일을 다시 읽은 것이다.** 손으로 넣은 것이
    /// 아니라는 증거가 저널과 표식이다 — 표식이 그대로면 다음 걸음이 제가 쓴 것을
    /// "밖에서 바뀌었다" 로 읽고 한 번 더 읽는다.
    #[test]
    fn a_write_lands_in_the_file_and_the_screen_rereads_it() {
        let (scratch, mut a) = writable("write");
        let file = scratch.join(".moai/issues.jsonl");
        a.trouble = Some("다시 읽지 못했다 — 옛 까닭".into());

        assert_eq!(add_idea(&mut a, "argos-0002").as_deref(), Some("argos-0002"));
        assert!(std::fs::read_to_string(&file).unwrap().contains("argos-0002"), "파일에 안 닿았다");
        assert_eq!(a.site.issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["argos-0001", "argos-0002"]);
        assert_eq!(a.site.index.find("argos-0002"), Some(1), "색인이 다시 안 섰다 — 손으로 넣은 것이다");
        let repo = a.site.repo.clone().unwrap();
        assert_eq!(repo.journal_of("argos-0002").unwrap()[0].by, "레이븐");
        assert!(a.trouble.is_none(), "다시 읽었는데 옛 까닭이 남았다");

        assert_eq!(a.site.stamp, stamp_of(&repo), "표식을 다시 안 잡았다");
        a.follow();
        assert!(!a.loading(), "제가 쓴 것을 밖에서 바뀐 것으로 읽었다");
    }

    /// 커서가 선 줄의 id. `..` 이나 바구니면 없다.
    fn on(a: &App) -> Option<String> {
        a.current().and_then(|r| match r {
            Row::Item(_, e, _) => e.at().map(|at| a.site.issues[at].id.clone()),
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
        assert_eq!(a.site.path, [Seg::Epic("argos-0001".into())]);

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert!(a.site.path.is_empty(), "생각은 에픽 밖에 서는데 에픽 안에 남았다 — {:?}", a.site.path);
        assert_eq!(on(&a).as_deref(), Some("argos-0002"));
        assert!(a.site.remembered.is_empty());

        let wrote = a.write(
            |_| {},
            |issues, _, _, by| {
                let at = "2026-09-13T00:00:00Z";
                issues.push(member("argos-0003", "argos-0001"));
                Ok((
                    vec![crate::model::JournalEntry::create("argos-0003", "멤버", at, by)],
                    Touched { id: "argos-0003".into(), done: "만듦" },
                ))
            },
        );
        assert_eq!(wrote.as_deref(), Some("argos-0003"));
        assert_eq!(a.site.path, [Seg::Epic("argos-0001".into())], "에픽 안의 줄인데 그리로 안 갔다");
        assert_eq!(on(&a).as_deref(), Some("argos-0003"));
        assert_eq!(a.site.remembered.len(), a.site.path.len());
        assert_eq!(a.notice.as_deref(), Some("✓ 만듦 · argos-0003"));

        a.key(key(KeyCode::Backspace));
        assert_eq!(on(&a).as_deref(), Some("argos-0001"), "나오니 들어간 에픽에 안 섰다");
    }

    /// **거름망이 새 줄을 가리면 말한다.** 조용히 안 보이면 저장이 실패한 것으로
    /// 읽힌다. 커서도 길도 거름망도 그대로다 — 사람이 건 것을 대신 풀지 않는다.
    #[test]
    fn a_filter_hiding_the_new_line_is_said_not_silent() {
        let (_scratch, mut a) = writable("land-hidden");
        a.hit("SPC f");
        typed(&mut a, "type=epic");
        let (path, cursor) = (a.site.path.clone(), a.cursor);

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert_eq!(a.site.issues.len(), 2, "쓰기가 안 닿았다");
        assert_eq!((a.site.path.clone(), a.cursor), (path, cursor), "가려진 줄을 찾아 자리를 옮겼다");
        assert_eq!(on(&a).as_deref(), Some("argos-0001"));
        assert_eq!(a.filter_text.as_deref(), Some("type=epic"), "거름망을 대신 풀었다");
        assert_eq!(a.notice.as_deref(), Some("✓ 담김 · argos-0002 — 거름망에 가려 안 보인다 · Esc 로 푼다"));
    }

    /// **보기가 새 줄을 가리면 거름망이 아니라 보기를 댄다**(moai-fmv5 리뷰, 시험은 moai-2bzp) —
    /// Esc 는 거름망만 풀어, "Esc 로 푼다" 를 대면 누른 키가 아무것도 안 한다.
    #[test]
    fn the_view_hiding_the_new_line_names_the_view_not_the_filter() {
        let (_scratch, mut a) = writable("land-view");
        a.hit("SPC v 1 Esc");
        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert_eq!(a.site.issues.len(), 2, "쓰기가 안 닿았다");
        assert_eq!(a.notice.as_deref(), Some("✓ 담김 · argos-0002 — 보기에 가려 안 보인다 · SPC v a 로 모두 보인다"));
    }

    /// **거름망과 보기가 함께 가리면 둘 다 댄다**(moai-2kyl 단계 리뷰) — 보기만 대면 `SPC v a` 를 눌러도
    /// 거름망에 여전히 가려 누른 키가 아무것도 안 한다.
    #[test]
    fn the_filter_and_the_view_hiding_the_new_line_are_both_named() {
        let (_scratch, mut a) = writable("land-both");
        a.hit("SPC f");
        typed(&mut a, "type=epic");
        a.hit("SPC v 1 Esc");
        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert_eq!(
            a.notice.as_deref(),
            Some("✓ 담김 · argos-0002 — 거름망과 보기에 가려 안 보인다 · Esc 로 풀고 SPC v a 로 모두 보인다")
        );
    }

    /// **보기·정렬·열은 누를 때마다 사용자 설정에 적히고 다음 실행이 읽는다**(moai-2bzp).
    #[test]
    fn the_look_is_saved_on_each_toggle_and_read_by_the_next_run() {
        let s = scratch("look-save");
        let user = s.join("user.toml");
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.hit("SPC v d Esc");
        a.hit("SPC v l Esc");
        a.hit("SPC s u Esc");
        a.hit("SPC s u Esc");
        a.hit("SPC c a Esc");
        a.hit("SPC c i Esc");
        a.hit("SPC v p Esc");
        let text = std::fs::read_to_string(&user).expect("보기가 설정에 안 적혔다");
        assert!(text.contains("[tui]") && text.contains("sort = \"updated\""), "{text}");

        let mut b = App::new(Vec::new(), cfg(), Path::new());
        b.user_config = Some(user.clone());
        b.load_look();
        assert_eq!(
            (b.view.clone(), b.order, b.fields),
            (a.view.clone(), a.order, a.fields),
            "다음 실행이 다른 보기로 떴다"
        );
        assert!(!b.detail_open && !a.detail_open, "숨긴 상세 칸이 다음 실행에 안 이어졌다");
        assert_eq!(b.notice, None);

        // 모르는 낱말은 알리고 나머지는 입힌다. 모르는 차례의 방향은 우선순위에 입히지 않는다 — 아무도 안
        // 고른 거꾸로가 선다. 겹쳐 적힌 숨김은 하나로 든다(moai-2kyl 단계 리뷰).
        std::fs::write(
            &user,
            "[tui]\nhidden = [\"done\", \"done\"]\nsort = \"nope\"\nsort_reversed = true\nhide_deferred = true\nfields = [\"id\", \"what\"]\n",
        )
        .unwrap();
        let mut c = App::new(Vec::new(), cfg(), Path::new());
        c.user_config = Some(user);
        c.load_look();
        assert!(c.view.hide_deferred, "틀린 키 하나로 나머지를 버렸다");
        assert_eq!(c.order, Default::default(), "모르는 차례의 방향을 우선순위에 입혔다");
        assert!(c.fields.shows(view::Field::Id) && !c.fields.shows(view::Field::Priority));
        let said = c.notice.clone().unwrap_or_default();
        assert!(said.contains("nope") && said.contains("what"), "{said}");

        // 모르는 낱말은 토글 한 번에 지워지지 않는다 — 새 바이너리가 적은 것일 수 있다. 겹쳐 적힌 done 은
        // 한 번에 보인다.
        c.hit("SPC v d Esc");
        assert!(!c.view.hides(crate::config::DONE), "겹쳐 적힌 done 이 한 번 눌러서는 안 보였다");
        let text = std::fs::read_to_string(c.user_config.as_ref().unwrap()).unwrap();
        assert!(
            text.contains("sort = \"nope\"") && text.contains("sort_reversed = true") && text.contains("\"what\""),
            "{text}"
        );
        c.hit("SPC s t Esc");
        let text = std::fs::read_to_string(c.user_config.as_ref().unwrap()).unwrap();
        assert!(
            text.contains("sort = \"title\"") && text.contains("sort_reversed = false") && text.contains("\"what\""),
            "{text}"
        );

        // 차례가 낱말이 아닌 모양이어도 방향만 입히지 않는다(moai-ys7c) — 알림은 차례를 못 읽었다고 한 줄 댄다.
        std::fs::write(c.user_config.as_ref().unwrap(), "[tui]\nsort = 3\nsort_reversed = true\n").unwrap();
        let mut d = App::new(Vec::new(), cfg(), Path::new());
        d.user_config = c.user_config.clone();
        d.load_look();
        assert_eq!(d.order, Default::default(), "못 읽은 차례의 방향을 우선순위에 입혔다");
        assert!(d.notice.clone().unwrap_or_default().contains("tui.sort"), "{:?}", d.notice);
    }

    /// **표 모양 보기 키 하나가 그 세션의 다른 저장을 막지 않는다**(moai-jr3z, 사용자 결정 2026-09-18). 그 키는
    /// 건너뛰고 한 번 알리며, 뒤의 토글은 적힌다 — 전에는 거절된 차이를 다음 저장마다 또 실어 숨김·상세가 하나도
    /// 안 적혔다.
    #[test]
    fn a_hand_written_sort_table_does_not_block_the_other_toggles() {
        let s = scratch("look-odd-sort");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\nsort.by = \"created\"\n").unwrap();
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.user_config = Some(user.clone());
        a.load_look();
        // 알림은 다음 키 하나에 걷힌다 — 메뉴를 닫는 Esc 도 키다. 보고 나서 닫는다.
        a.hit("SPC s t");
        assert!(a.notice.clone().unwrap_or_default().contains("tui.sort"), "{:?}", a.notice);
        a.hit("Esc");
        a.notice = None;
        a.hit("SPC v d");
        assert_eq!(a.notice, None, "건너뛴 키를 다음 저장에 또 실었다");
        a.hit("Esc");
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("sort.by = \"created\"") && text.contains("hidden"), "숨김이 안 적혔다\n{text}");
    }

    /// **깨진 설정은 한 곳에서만 말한다**(moai-5jsn) — 층이 없다는 배너가 파싱 오류를 대므로 보기 알림은 같은
    /// 말을 다시 안 한다. 띄우는 길(`cmd::tui`)과 같은 차례로 보기를 입히고 층을 얹는다.
    #[test]
    fn a_broken_user_config_is_told_once() {
        let s = scratch("look-broken");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui\nsort = \"title\"\n").unwrap();
        let reg = crate::user_config::read(Some(&user));
        let mut a = App::new(Vec::new(), cfg(), Path::new());
        a.adopt_look(&reg.look, reg.look_problems.clone());
        let a = a.attach_layer(layer::Layer::of(&reg, None, crate::i18n::Lang::Ko));
        assert!(
            a.unlayered.as_deref().is_some_and(|u| u.contains("TOML")),
            "층 없음 배너가 까닭을 안 들었다 — {:?}",
            a.unlayered
        );
        assert_eq!(a.notice, None, "같은 파싱 오류를 보기 알림이 또 댔다");
    }

    /// **두 탐색기가 저마다 누른 것이 둘 다 남는다**(moai-2kyl 단계 리뷰). 적는 것은 이 세션이 바꾼 만큼이다
    /// — 화면이 든 보기를 통째로 적으면 옆에서 켠 열을 내 토글 한 번이 지운다.
    #[test]
    fn two_explorers_keep_each_others_toggles() {
        let s = scratch("look-two");
        let user = s.join("user.toml");
        let open = || {
            let mut x = App::new(Vec::new(), cfg(), Path::new());
            x.user_config = Some(user.clone());
            x.load_look();
            x
        };
        let (mut a, mut b) = (open(), open());
        a.hit("SPC c a Esc");
        b.hit("SPC v d Esc");
        b.hit("SPC s u Esc");
        let c = open();
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(c.fields.shows(view::Field::Assignee), "옆 탐색기가 켠 열을 지웠다\n{text}");
        assert!(
            !c.view.hides(crate::config::DONE)
                && c.order == keys::Sorting { by: keys::Order::Updated, reversed: false },
            "{text}"
        );
    }

    /// **숨김은 이 프로젝트의 칸에만 건다**(moai-2kyl 단계 리뷰). 다른 프로젝트에서 숨긴 칸 이름이 이 프로젝트의
    /// 줄(설정에서 이름이 바뀐 옛 칸에 남은 줄)을 말없이 숨기지 않고 — 뱃지도 번호 토글도 없다 — `SPC v a` 도
    /// 그 이름을 걷지 않는다. 그 칸이 있는 프로젝트로 돌아가면 다시 숨는다.
    #[test]
    fn a_hidden_name_this_project_lacks_neither_hides_rows_nor_is_wiped() {
        let mut stale = make("argos-0002", Kind::Issue);
        stale.status = Status::new("blocked");
        let mut a = App::new(vec![make("argos-0001", Kind::Issue), stale], cfg(), Path::new());
        a.view.hidden.push("blocked".into());
        a.see();
        assert_eq!(row_ids(&a), ["argos-0001", "argos-0002"], "설정에 없는 칸 이름이 줄을 말없이 숨겼다");
        a.hit("SPC v a");
        assert!(a.view.hides("blocked"), "모두 보이기가 다른 프로젝트의 칸 이름을 걷었다");
        assert!(!a.view.hides(crate::config::DONE));
    }

    /// **적어 둔 보기를 입힌 뒤에 층을 얹는다**(moai-2kyl 단계 리뷰 — `cmd/tui.rs::run` 의 차례).
    /// 처음값 보기(done 숨김)로 세우면 끝난 줄뿐인 뿌리가 통째로 비어, 층을 먼저 얹은 화면은
    /// 적어 둔 보기가 그 줄을 도로 보여도 커서가 목록 밖에 남는다. 뿌리의 `..` 을 걷은
    /// 뒤(moai-i784)로 `with_layer` 는 커서를 안 건드리므로, 이 시험이 재는 것은 커서가 **첫
    /// 줄**(그 끝난 줄)에 서는가다.
    #[test]
    fn the_saved_look_is_on_before_the_layer_places_the_first_cursor() {
        let s = scratch("look-layer");
        let user = s.join("user.toml");
        std::fs::write(&user, "[tui]\nhidden = []\n").unwrap();
        let mut finished = make("argos-0001", Kind::Issue);
        finished.status = Status::new("done");
        let mut a = App::new(vec![finished], cfg(), Path::new());
        a.user_config = Some(user);
        a.load_look();
        let a = a.with_layer(layer::fake(vec![("argos", "/x", layer::Look::Unread)], layer::At::Project("/x".into())));
        assert_eq!(a.rows().len(), 1, "시험의 전제 — 끝난 줄 하나 (뿌리에 `..` 은 없다)");
        assert_eq!(a.cursor, 0, "첫 화면이 그 줄에 안 섰다");
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
        let file = scratch.join(".moai/issues.jsonl");
        let mut src = std::fs::read_to_string(&file).unwrap();
        src.push_str(&format!("{}\n", serde_json::to_string(&make("argos-0003", Kind::Issue)).unwrap()));
        std::fs::write(&file, src).unwrap();
        a.follow();
        assert!(a.loading(), "판이 다르다 — 밖의 쓰기를 못 봤다");

        assert!(add_idea(&mut a, "argos-0002").is_some());
        assert!(!a.loading(), "쓰기 전에 띄운 읽기가 남았다");
        assert_eq!(a.site.issues.len(), 3, "밖에서 떨어진 줄이나 제가 쓴 줄을 잃었다");
        settle(&mut a);
        assert_eq!(a.site.issues.len(), 3);
    }

    /// **실패는 화면에 선다.** 검증에 걸리면 파일도 화면도 그대로고, 까닭이 남는다 —
    /// 다시 읽으면 그 까닭이 지워지므로 실패한 뒤에는 읽지 않는다.
    #[test]
    fn a_refused_write_says_why_and_touches_nothing() {
        let (scratch, mut a) = writable("write-refused");
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        let stamp = a.site.stamp;
        // 앞 쓰기의 알림이 남아 있으면 실패한 이번 쓰기가 담긴 것으로 읽힌다.
        a.notice = Some("✓ 담김 · argos-0000".into());

        let out = a.write(
            |_| {},
            |issues, _, _, _| {
                issues.push(Issue::new(
                    "argos-0002".into(),
                    "t".into(),
                    Kind::Idea,
                    Status::new("없는칸"),
                    "2026-09-13T00:00:00Z",
                ));
                Ok((vec![], Touched { id: "argos-0002".into(), done: "담김" }))
            },
        );
        assert!(out.is_none(), "거절됐는데 썼다고 한다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        assert_eq!(a.site.issues.len(), 1);
        assert_eq!(a.site.stamp, stamp);
        let t = a.trouble.clone().unwrap_or_default();
        assert!(t.starts_with("쓰지 못했다") && t.contains("칸"), "{t}");
        assert_eq!(a.notice, None, "실패했는데 앞 쓰기의 알림이 남았다");
        assert_eq!(a.cursor, 0);

        // 여러 줄 거절문(고칠 명령까지 내는 것)은 배너 한 줄로 이어진다.
        let out = a.write(
            |_| {},
            |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> {
                Err("첫 줄\n      고칠 명령".into())
            },
        );
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
        let file = scratch.join(".moai/issues.jsonl");
        let out = a.write(
            |_| {},
            |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> { Err("락".into()) },
        );
        assert!(out.is_none());

        let mut src = std::fs::read_to_string(&file).unwrap();
        src.push_str(&format!("{}\n", serde_json::to_string(&make("argos-0003", Kind::Issue)).unwrap()));
        std::fs::write(&file, src).unwrap();
        settle(&mut a);
        assert_eq!(a.site.issues.len(), 2, "밖의 쓰기를 못 읽었다");
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
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.user = Some("이름만".into());

        let out = a.write(
            |_| {},
            |_, _, _, _| -> crate::fail::R<(Vec<crate::model::JournalEntry>, Touched)> {
                panic!("누군지 모르는데 닫는 함수를 불렀다")
            },
        );
        assert!(out.is_none());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        let t = a.trouble.clone().unwrap_or_default();
        assert!(t.starts_with("쓰지 못했다") && !t.contains('\n'), "{t:?}");
        assert_eq!(a.mode, Mode::Browse, "사람이 준 것을 두고 또 물었다");
    }

    /// 누군지 모르는 기계. **이 기계의 git 설정도 `MOAI_ACTOR` 도 안 본다** — 준 것만
    /// 푼다. 진짜 길(`model::actor`)을 쓰면 이 시험들이 돌리는 사람의 설정에 달린다.
    fn nobody(user: Option<&str>, root: &std::path::Path) -> crate::fail::R<crate::model::Actor> {
        match user {
            Some(raw) => crate::model::actor(Some(raw), root),
            None => {
                Err(crate::fail::Fail::coded("누가 하는지 모른다 — 시험\n\n  고칠 명령", crate::fail::code::NO_ACTOR))
            }
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
        a.hit("SPC n");
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
            a.hit("SPC n");
            assert_eq!(a.mode, Mode::Idea(Form::default()), "{inside} {focus:?}");
            a.key(key(KeyCode::Tab));
            assert_eq!(a.focus, focus, "폼의 Tab 이 탐색기 포커스를 옮겼다");
            a.key(key(KeyCode::Esc));
            assert_eq!((&a.mode, a.focus), (&Mode::Browse, focus));
        }
        for opener in ["/", "SPC f"] {
            let mut a = app();
            a.hit(opener);
            a.key(key(KeyCode::Char('n')));
            assert!(matches!(&a.mode, Mode::Grep(q, _) | Mode::Filter(q) if q.text() == "n"), "{:?}", a.mode);
        }
    }

    /// **담으면 파일에 idea 로 선다 — 에픽 없이.** 커서가 에픽 안에 있어도 거기 넣지 않는다.
    /// 만드는 길은 CLI 와 같다: 첫 칸, 만든 사람이 담당, 저널 `create` 한 줄. 본문은 여러
    /// 줄 그대로, 끝의 빈 줄은 뗀다. 담으면 폼이 닫힌다.
    #[test]
    fn ctrl_s_saves_an_idea_without_an_epic_wherever_the_cursor_is() {
        let (_scratch, mut a) = writable("jot");
        a.key(key(KeyCode::Enter)); // 에픽 안
        assert_eq!(a.site.path.len(), 1, "판이 다르다 — 에픽 안에 못 들어갔다");
        jotting(&mut a, "  반짝 떠오른 것 ");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "첫 줄");
        a.key(key(KeyCode::Enter));
        type_in(&mut a, "둘째 줄");
        a.key(key(KeyCode::Enter));
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "담았는데 폼이 안 닫혔다 — {:?}", a.trouble);

        let repo = a.site.repo.clone().unwrap();
        let made = ideas_in(&repo);
        assert_eq!(made.len(), 1, "{made:?}");
        let idea = &made[0];
        assert_eq!(idea.title, "반짝 떠오른 것");
        assert_eq!(idea.body.as_deref(), Some("첫 줄\n둘째 줄"));
        assert_eq!((idea.epic.as_deref(), idea.milestone.as_deref()), (None, None), "커서가 선 에픽에 넣었다");
        assert_eq!(idea.status.as_str(), "todo");
        assert_eq!(
            (idea.assignee.as_deref(), idea.assignee_email.as_deref()),
            (Some("레이븐"), Some("raven@example.com"))
        );
        assert!(idea.id.starts_with("argos-"), "{}", idea.id);
        let journal = repo.journal_of(&idea.id).unwrap();
        assert_eq!(journal.len(), 1);
        assert_eq!(
            (journal[0].kind.as_str(), journal[0].title.as_deref(), journal[0].by.as_str()),
            ("create", Some("반짝 떠오른 것"), "레이븐")
        );
        // 화면은 파일을 다시 읽은 것이고, 커서는 만든 줄에 서며 알림은 하나다 — 폼은 닫기만
        // 하고 뒤처리는 `write` 가 한다(moai-064q). idea 는 에픽에 안 드니 뿌리로 나온다.
        assert!(a.site.index.find(&idea.id).is_some(), "쓰고 다시 안 읽었다");
        assert_eq!((on(&a), a.site.path.len()), (Some(idea.id.clone()), 0), "만든 줄에 안 섰다");
        assert_eq!(a.notice, Some(format!("✓ 담김 · {}", idea.id)));

        // 제목만으로도 담긴다. F2 는 걷었다(moai-7sjm) — 눌러도 폼은 그대로다.
        jotting(&mut a, "하나 더");
        a.key(key(KeyCode::F(2)));
        assert!(matches!(a.mode, Mode::Idea(_)), "걷은 F2 가 담았다");
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse);
        let made = ideas_in(&repo);
        assert!(made.iter().any(|i| i.title == "하나 더" && i.body.is_none()), "{made:?}");
    }

    /// **제목이 비면 담지 않는다. 그 밖에는 아무것도 안 묻는다** — 누군지도 안 푼다.
    /// 파일은 그대로고 폼은 까닭을 달고 제목 칸에 선다.
    #[test]
    fn an_empty_title_is_refused_in_place_and_nothing_is_asked() {
        fn refuse(_: Option<&str>, _: &std::path::Path) -> crate::fail::R<crate::model::Actor> {
            panic!("빈 제목인데 누군지 물었다")
        }
        let (scratch, mut a) = writable("jot-empty");
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.identify = refuse;
        jotting(&mut a, "   ");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "본문만 있다");
        a.key(ctrl('s'));
        let Mode::Idea(form) = &a.mode else { panic!("빈 제목에 폼이 닫혔다 — {:?}", a.mode) };
        assert_eq!(
            (form.error.as_deref(), form.field),
            (Some(form::empty_title(crate::i18n::Lang::Ko)), form::Field::Title)
        );
        assert_eq!(form.body.text(), "본문만 있다", "거절하며 적은 것을 지웠다");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        assert!(a.trouble.is_none(), "빈 제목은 쓰기의 실패가 아니다 — {:?}", a.trouble);
    }

    /// **담을 곳 없이 연 폼은 저장소가 있어도 안 쓴다** — 쓰는 곳은 폼이 박은 곳이지 지금
    /// `repo` 가 아니다(moai-fccv). `n` 은 늘 담을 곳을 박으므로 이것은 속을 직접 세운 폼이다.
    #[test]
    fn a_form_without_a_target_never_writes() {
        let (scratch, mut a) = writable("jot-untargeted");
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        jotting(&mut a, "여기");
        assert!(
            matches!(&a.mode, Mode::Idea(f) if f.into.as_ref().is_some_and(|t| t.path == *scratch.path())),
            "{:?}",
            a.mode
        );
        a.mode = Mode::Idea(Form { title: Input::new("어디에도"), ..Form::default() });
        a.key(ctrl('s'));
        assert!(matches!(a.mode, Mode::Idea(_)), "{:?}", a.mode);
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다")), "{:?}", a.trouble);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    }

    /// **저널만 못 적은 쓰기는 담긴 것으로 닫고 알림이 그렇다고 말한다**(moai-52z9). 실패로
    /// 내면 폼이 열린 채 남아 다시 누르면 같은 것이 둘 선다.
    #[cfg(unix)]
    #[test]
    fn a_save_whose_journal_fails_closes_as_saved_and_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let (scratch, mut a) = writable("jot-nojournal");
        let journal = scratch.join(".moai/journal.jsonl");
        std::fs::write(&journal, "").unwrap();
        std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new().append(true).open(&journal).is_ok() {
            return; // root 는 권한을 안 본다
        }
        jotting(&mut a, "한 번만");
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "담겼는데 폼이 열린 채다 — {:?}", a.trouble);
        assert!(a.trouble.is_none(), "{:?}", a.trouble);
        assert!(a.notice.as_deref().is_some_and(|n| n.contains("이력은 못 남겼다")), "{:?}", a.notice);
        assert_eq!(ideas_in(a.site.repo.as_ref().unwrap()).len(), 1);
    }

    /// **Esc 는 빈 폼을 곧바로 닫고, 적던 것이 있으면 한 번 묻는다.** `y` 만 버린다 —
    /// 다른 키는 폼으로 돌아가고 적던 것은 그대로다.
    #[test]
    fn esc_closes_a_blank_form_at_once_and_asks_before_dropping_what_was_typed() {
        let mut a = app();
        a.hit("SPC n");
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
        let lock = scratch.join(".moai/lock");
        // 락 파일 자리에 디렉터리를 두면 `with_write` 가 락을 못 열고 곧바로 물러난다.
        std::fs::create_dir_all(&lock).unwrap();
        jotting(&mut a, "못 담길 것");
        a.key(ctrl('s'));
        assert!(
            matches!(&a.mode, Mode::Idea(f) if f.title.text() == "못 담길 것"),
            "실패했는데 폼이 닫혔다 — {:?}",
            a.mode
        );
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다")), "{:?}", a.trouble);
        assert!(ideas_in(a.site.repo.as_ref().unwrap()).is_empty());

        // 고치고 다시 누르면 담기고 까닭이 걷힌다
        std::fs::remove_dir(&lock).unwrap();
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert!(a.trouble.is_none());
        assert_eq!(ideas_in(a.site.repo.as_ref().unwrap()).len(), 1);

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
        a.hit("SPC n");
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
        let file = scratch.join(".moai/issues.jsonl");
        let config = scratch.join(".moai/config.toml");
        let (before, config_before) =
            (std::fs::read_to_string(&file).unwrap(), std::fs::read_to_string(&config).unwrap());
        a.user = None;
        a.identify = nobody;
        jotting(&mut a, "떠오른 것");
        a.key(key(KeyCode::Tab));
        type_in(&mut a, "본문");

        a.key(ctrl('s'));
        assert!(
            matches!(&a.mode, Mode::Ask(ask) if ask.why == "누가 하는지 모른다 — 시험"),
            "모르는데 안 물었거나 까닭을 옮기지 않았다 — {:?}",
            a.mode
        );
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
        let repo = a.site.repo.clone().unwrap();
        let made = ideas_in(&repo);
        assert_eq!(made.len(), 1, "받은 뒤에도 파일에 안 닿았다");
        assert_eq!(
            (made[0].title.as_str(), made[0].body.as_deref()),
            ("떠오른 것", Some("본문")),
            "되돌린 폼에서 안 읽었다"
        );
        // 이어진 쓰기도 같은 뒤처리를 받는다 — Enter 가 알림을 걷은 뒤에 쓰기가 제 알림을 단다.
        assert_eq!(on(&a), Some(made[0].id.clone()), "묻고 이어진 쓰기가 만든 줄에 안 섰다");
        assert_eq!(a.notice, Some(format!("✓ 담김 · {}", made[0].id)));
        let journal = repo.journal_of(&made[0].id).unwrap();
        assert_eq!((journal[0].by.as_str(), journal[0].by_email.as_deref()), ("레이븐", Some("raven@example.com")));
        assert_eq!(a.user.as_deref(), Some("레이븐 (raven@example.com)"));
        // 받은 사람이 [NEW] 를 가를 사람이기도 하다(moai-j038.vna) — 헤더만 그 사람을 대고 안 읽음은
        // 띄울 때의 "모름" 에 머물면 방금 담은 제 줄에도 [NEW] 가 안 선다.
        assert_eq!(a.me.as_deref(), Some("레이븐 (raven@example.com)"));
        assert!(a.site.unread.contains(&made[0].id), "받은 사람의 새 줄에 [NEW] 가 안 섰다 — {:?}", a.site.unread);
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
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.user = None;
        a.identify = nobody;
        jotting(&mut a, "적던 것");
        let form = a.mode.clone();

        a.key(ctrl('s'));
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
        fn refuse(_: Option<&str>, _: &std::path::Path) -> crate::fail::R<crate::model::Actor> {
            panic!("읽기가 누군지 물었다")
        }
        let (_scratch, mut a) = writable("ask-read");
        a.user = None;
        a.identify = refuse;
        for k in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace, KeyCode::Tab] {
            a.key(key(k));
        }
        a.hit("SPC v r Esc");
        a.key(key(KeyCode::Char('/')));
        type_in(&mut a, "제목");
        a.key(key(KeyCode::Enter));
        settle(&mut a);
        assert_eq!(a.mode, Mode::Browse);
    }

    /// 편집기가 있는 판에서 `n` 을 눌러 루프에 맡긴 요청을 꺼낸다 — 루프가 하는 것을 흉내 낸다.
    fn ask_editor(a: &mut App) -> Edit {
        a.editor = Some("vi".into());
        a.hit("SPC n");
        assert_eq!(a.mode, Mode::Browse, "편집기가 있는데 안 폼을 열었다");
        a.edit.take().expect("편집기를 청하지 않았다")
    }

    /// **편집기가 있으면 `n` 은 폼을 안 열고 루프에 편집기를 청한다.** 담을 곳은 여는 순간
    /// 박히고 안내 글이 그곳을 댄다. 편집기가 없으면 오늘처럼 안 폼이다.
    #[test]
    fn n_asks_the_loop_for_the_editor_when_there_is_one() {
        let (scratch, mut a) = writable("editor-ask");
        let edit = ask_editor(&mut a);
        assert_eq!(edit.into.as_ref().map(|t| t.path.clone()), Some(scratch.path().to_path_buf()));
        assert_eq!(edit.editor, "vi");
        assert!(edit.text.contains(&scratch.path().display().to_string()), "{}", edit.text);

        let (_s, mut b) = writable("editor-none");
        b.hit("SPC n");
        assert_eq!(b.edit, None, "편집기가 없는데 청했다");
        assert!(matches!(&b.mode, Mode::Idea(f) if f.into.is_some()), "{:?}", b.mode);
    }

    /// **편집기에서 받은 글은 폼과 같은 길로 담긴다** — 첫 줄 제목, 한 줄 띄우고 본문, 주석은
    /// 걷고. 담기면 폼은 안 남고 커서와 알림은 `write` 가 한다.
    #[test]
    fn the_edited_text_is_saved_like_the_form() {
        let (_scratch, mut a) = writable("editor-save");
        let edit = ask_editor(&mut a);
        let text = format!("  편집기에서 온 것 \n\n## 설계\n둘째 줄\n{}", edit.text);
        a.edited(edit.into, Ok(text));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        let repo = a.site.repo.clone().unwrap();
        let made = ideas_in(&repo);
        assert_eq!(made.len(), 1, "{made:?}");
        assert_eq!((made[0].title.as_str(), made[0].body.as_deref()), ("편집기에서 온 것", Some("## 설계\n둘째 줄")));
        assert_eq!(made[0].epic, None);
        assert_eq!(on(&a), Some(made[0].id.clone()));
        assert_eq!(a.notice, Some(format!("✓ 담김 · {}", made[0].id)));
    }

    /// **편집기가 오류로 끝났거나 제목이 비면 아무것도 안 쓴다** — 누군지도 안 묻고, 폼도 안
    /// 열고, 한 줄로 까닭을 댄다.
    #[test]
    fn a_failed_or_empty_edit_writes_nothing_and_says_so() {
        fn refuse(_: Option<&str>, _: &std::path::Path) -> crate::fail::R<crate::model::Actor> {
            panic!("담지 않을 글인데 누군지 물었다")
        }
        let (scratch, mut a) = writable("editor-nothing");
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.identify = refuse;
        for (got, says) in [
            (Err("편집기가 3 로 끝났다(vi)".to_string()), "3 로 끝났다"),
            (Ok(String::new()), "제목이 비었다"),
            // 안 고치고 닫은 안내 글 그대로
            (Ok(jotfile::template(None, crate::i18n::Lang::Ko)), "제목이 비었다"),
        ] {
            let edit = ask_editor(&mut a);
            a.edited(edit.into, got);
            assert_eq!(a.mode, Mode::Browse, "{:?}", a.mode);
            assert!(
                a.notice.as_deref().is_some_and(|n| n.starts_with("담지 않았다") && n.contains(says)),
                "{says} {:?}",
                a.notice
            );
            assert!(a.trouble.is_none(), "그만둔 것은 실패가 아니다 — {:?}", a.trouble);
            assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        }
    }

    /// **누군지 모르면 편집기에서 온 글도 묻고, 받으면 그 글이 박힌 곳에 담긴다.** 묻는 칸
    /// 뒤에 선 것은 받은 글로 채운 폼이다 — Esc 로 그만둬도 글은 폼에 남는다.
    #[test]
    fn an_edit_that_needs_a_name_asks_and_then_lands_in_the_fixed_project() {
        let (scratch, mut a) = writable("editor-ask-who");
        let file = scratch.join(".moai/issues.jsonl");
        let before = std::fs::read_to_string(&file).unwrap();
        a.user = None;
        a.identify = nobody;
        let edit = ask_editor(&mut a);
        a.edited(edit.into, Ok("물어볼 것\n\n본문".into()));
        let Mode::Ask(ask) = &a.mode else { panic!("모르는데 안 물었다 — {:?}", a.mode) };
        assert!(
            matches!(ask.back.as_ref(), Mode::Idea(f) if f.title.text() == "물어볼 것" && f.body.text() == "본문"),
            "{:?}",
            ask.back
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "묻기 전에 썼다");

        type_in(&mut a, "레이븐 (raven@example.com)");
        a.key(key(KeyCode::Enter));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        let made = ideas_in(a.site.repo.as_ref().unwrap());
        assert_eq!((made.len(), made[0].title.as_str(), made[0].body.as_deref()), (1, "물어볼 것", Some("본문")));
    }

    /// **담기가 실패하면 편집기에서 적은 글이 폼에 열린 채 남는다** — 임시 파일과 함께 사라지지
    /// 않는다. 고치고 다시 담으면 담긴다.
    #[test]
    fn a_failed_save_of_an_edit_keeps_the_text_in_the_form() {
        let (scratch, mut a) = writable("editor-fail");
        let lock = scratch.join(".moai/lock");
        std::fs::create_dir_all(&lock).unwrap();
        let edit = ask_editor(&mut a);
        a.edited(edit.into, Ok("못 담길 것\n\n긴 본문".into()));
        assert!(
            matches!(&a.mode, Mode::Idea(f) if f.title.text() == "못 담길 것" && f.body.text() == "긴 본문"),
            "{:?}",
            a.mode
        );
        assert!(a.trouble.as_deref().is_some_and(|t| t.starts_with("쓰지 못했다")), "{:?}", a.trouble);
        std::fs::remove_dir(&lock).unwrap();
        a.key(ctrl('s'));
        assert_eq!(a.mode, Mode::Browse, "{:?}", a.trouble);
        assert_eq!(ideas_in(a.site.repo.as_ref().unwrap()).len(), 1);
    }

    /// **아직 안 담긴 글을 찾는다**(moai-y3r7). 루프가 오류로 끝날 때 남길 글이다 — 폼에
    /// 열린 채든(담기 실패), 누구냐 묻는 칸 뒤에 서 있든. 빈 폼과 탐색은 남길 것이 없다.
    #[test]
    fn unsaved_text_is_found_in_an_open_form_or_behind_a_question() {
        let (scratch, mut a) = writable("unsaved-fail");
        assert_eq!(a.unsaved(), None, "탐색 중인데 남길 글이 있다고 한다");
        std::fs::create_dir_all(scratch.join(".moai/lock")).unwrap();
        let edit = ask_editor(&mut a);
        a.edited(edit.into, Ok("못 담길 것\n\n긴 본문".into()));
        assert_eq!(a.unsaved(), Some(("못 담길 것".to_string(), Some("긴 본문".to_string()))), "{:?}", a.mode);

        let (_s, mut b) = writable("unsaved-ask");
        b.user = None;
        b.identify = nobody;
        let edit = ask_editor(&mut b);
        b.edited(edit.into, Ok("물어볼 것".into()));
        assert!(matches!(b.mode, Mode::Ask(_)), "{:?}", b.mode);
        assert_eq!(b.unsaved(), Some(("물어볼 것".to_string(), None)), "묻는 칸 뒤의 폼을 못 봤다");

        let (_s, mut c) = writable("unsaved-blank");
        c.hit("SPC n");
        assert!(matches!(c.mode, Mode::Idea(_)), "{:?}", c.mode);
        assert_eq!(c.unsaved(), None, "빈 폼을 남길 글로 셌다");
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
