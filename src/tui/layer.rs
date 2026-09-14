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

use super::{App, Row, Stamp};
use crate::nav::Index;
use crate::projects::{self, State};
use crate::store::{Opened, Repo};
use crate::user_config;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    pub at: At,
    /// 층의 줄. 차례는 띄운 자리(등록 안 됨)가 맨 앞, 나머지는 등록 차례 그대로다.
    pub places: Vec<Place>,
    /// 사용자 설정을 읽다 만난 것. 층에 선 동안 배너가 비춘다.
    pub problems: Vec<String>,
    /// 읽은 사용자 설정 파일. F5 가 여기를 다시 읽는다 — **시험은 제 임시 파일을 준다.**
    /// 환경을 다시 보면 돌리는 사람의 설정을 읽는다.
    pub config: Option<PathBuf>,
    /// `.moai` 안에서 띄웠으면 그 뿌리. 등록돼 있지 않아도 층에 선다.
    launch: Option<PathBuf>,
    /// 스레드에서 읽고 있는 프로젝트들. 끝나면 [`App::follow`] 가 받는다.
    pending: Option<(Receiver<Vec<Looked>>, std::thread::JoinHandle<()>)>,
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
    /// 읽기 **전에** 잰 표식 — `.moai/issues.jsonl` 과 `.moai/config.toml`.
    marks: Marks,
}

/// 설정 표식까지 재는 까닭: `moai init` 은 설정이 먼저 생기고, 깨진 설정을 고친 것은
/// 스냅샷 표식으로는 안 보인다. **디렉터리가 있는지도 잰다** — `.moai` 없는 디렉터리가
/// 지워지거나(init 전 → 없다) 빈 디렉터리로 다시 생기면(없다 → init 전) 두 파일의 표식은
/// 둘 다 `None` 그대로라, 층이 옛 까닭과 옛 고칠 길을 영영 댄다. 셋 다 `stat` 하나라
/// 걸음마다 재도 싸다.
type Marks = (bool, Stamp, Stamp);

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
    /// 드러난 것의 수. 알림은 안 센다.
    pub warnings: usize,
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
    let moai = dir.join(".moai");
    (dir.is_dir(), crate::store::stamp(&moai.join("issues.jsonl")), crate::store::stamp(&moai.join("config.toml")))
}

/// 같은 디렉터리인가. 철자가 같으면 그만이고, 아니면 링크를 풀어 견준다 — 등록은 푼
/// 경로로 적히지만 손으로 적은 줄은 링크 철자일 수 있다.
fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// 연 프로젝트 하나를 센다. **`&[Issue]` 에 대한 셈은 전부 `report` 가 한다.**
pub fn summarize(repo: &Repo, load: &crate::store::Load, now: &str) -> Summary {
    let cfg = &repo.config;
    let unreadable: Vec<crate::report::Unreadable> =
        load.errors.iter().map(|e| crate::report::Unreadable { id: e.id.as_deref() }).collect();
    let st = crate::report::status(&load.issues, &unreadable, cfg, now);
    Summary {
        counts: cfg.statuses.iter().map(|s| (s.clone(), st.counts.get(s).copied().unwrap_or(0))).collect(),
        picked: crate::report::wip(&load.issues, cfg)
            .into_iter()
            .map(|i| Picked { id: i.id.clone(), title: i.title.clone(), column: i.status.as_str().to_string() })
            .collect(),
        warnings: st.warnings.len(),
        unreadable: load.errors.len(),
    }
}

/// 열지 못한 상태를 층의 말로. 말은 CLI 한눈 보기의 것을 그대로 쓴다 — 같은 상태를 두
/// 화면이 달리 부르면 둘을 오가는 사람이 두 낱말을 다 배워야 한다.
fn shut(path: &Path, name: &str, state: State) -> Look {
    let kind = match &state {
        State::Uninit => Shut::Uninit,
        State::Missing => Shut::Missing,
        State::Unreadable(_) | State::Open { .. } => Shut::Unreadable,
    };
    let p = projects::Project { path: path.to_path_buf(), name: name.to_string(), hue: None, state };
    let said = crate::style::plain(&crate::view::unopened(&p, &p.seen(|_, _| ()))).trim().to_string();
    Look::Shut { state: kind, said }
}

/// 경로들을 연다. **표식을 먼저 잰다** — 읽고 나서 재면 그 사이의 쓰기가 "이미 본 것"
/// 으로 적혀 영영 안 보인다(`App::open` 과 같은 까닭). 어느 스레드에서 불러도 같다.
fn look_at(paths: &[PathBuf], now: &str) -> Vec<Looked> {
    let marks: Vec<Marks> = paths.iter().map(|p| marks_of(p)).collect();
    // 여는 길은 한눈 보기와 같은 `projects::open` 이다 — 상태를 가르는 셈을 두 벌 두지 않는다.
    // 이름은 여기서 안 쓴다(층이 목록 전체로 이미 정했다). 말에 이름은 안 든다.
    let reg = user_config::Registry {
        path: None,
        projects: paths.iter().map(|p| user_config::Project { path: p.clone(), hue: None }).collect(),
        problems: Vec::new(),
    };
    projects::open(&reg)
        .into_iter()
        .zip(marks)
        .map(|(p, marks)| {
            let look = match p.state {
                State::Open { repo, load } => Look::Open { sum: summarize(&repo, &load, now) },
                state => shut(&p.path, &p.name, state),
            };
            Looked { path: p.path, marks, look }
        })
        .collect()
}

impl Layer {
    /// 사용자 설정을 읽어 층을 세운다. 줄은 아직 안 읽었다([`Look::Unread`]).
    ///
    /// `launch` 는 `.moai` 안에서 띄웠을 때의 뿌리다. **등록돼 있지 않아도 맨 앞에 선다** —
    /// 빼면 Bksp 로 올라간 뒤 내려올 길이 없어, 디렉터리처럼 드나든다는 약속이 한
    /// 방향으로만 선다. 등록돼 있으면 그 줄에 표시만 붙는다. 이름은 띄운 자리까지 넣고
    /// 가른다 — 같은 화면에 같은 이름이 둘 서면 안 된다.
    pub fn read(config: Option<&Path>, launch: Option<&Path>) -> Layer {
        let reg = user_config::read(config);
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
                marks: (false, None, None),
            })
            .collect();
        let at = match places.iter().find(|p| p.launched) {
            Some(p) => At::Project(p.path.clone()),
            None => At::Layer,
        };
        Layer { at, places, problems: reg.problems, config: config.map(Path::to_path_buf), launch: launch.map(Path::to_path_buf), pending: None }
    }

    /// 등록한 프로젝트가 하나라도 있는가. **없으면 층을 세우지 않는다** — 띄운 자리 하나뿐인
    /// 층은 오늘 화면에 `..` 하나를 더할 뿐이다(결정 3).
    pub fn registered(&self) -> bool {
        self.places.iter().any(|p| p.registered)
    }

    /// 다시 읽어야 할 줄 — 아직 안 읽었거나 표식이 바뀐 것.
    fn stale(&self) -> Vec<PathBuf> {
        self.places
            .iter()
            .filter(|p| matches!(p.look, Look::Unread) || marks_of(&p.path) != p.marks)
            .map(|p| p.path.clone())
            .collect()
    }

    /// 읽어 온 것을 경로로 맞춰 들인다. 그새 목록에서 빠진 경로는 버린다.
    fn adopt(&mut self, looked: Vec<Looked>) {
        for l in looked {
            if let Some(p) = self.places.iter_mut().find(|p| p.path == l.path) {
                p.marks = l.marks;
                p.look = l.look;
            }
        }
    }

    fn position(&self, path: &Path) -> Option<usize> {
        self.places.iter().position(|p| p.path == path)
    }
}

/// 층에서 뜻이 없는 키가 **왜 아무 일도 안 하는지**. 조용히 먹으면 고장 난 것으로 보인다.
///
/// `n` 은 특히 그렇다: 어느 프로젝트에 담을지 안 정해진 채로 띄운 자리에 쓰면, 사람은 보던
/// 줄의 프로젝트에 담긴 줄 안다. 담을 곳을 고르는 길은 moai-fccv 가 정한다.
pub fn refused(k: &KeyEvent) -> Option<&'static str> {
    if k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
        return None;
    }
    match k.code {
        KeyCode::Char('n') => Some("여기는 프로젝트 층이라 담을 곳이 없다 — Enter 로 프로젝트에 들어가서 n"),
        KeyCode::Char('/') | KeyCode::Char('f') | KeyCode::F(7) => {
            Some("거름망은 프로젝트 안의 줄에 건다 — Enter 로 들어가서 건다")
        }
        KeyCode::Char('w') => Some("워크트리 겹쳐 보기는 프로젝트 안에서 켠다 — Enter 로 들어가서 w"),
        _ => None,
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

    /// `.moai` 밖에서 띄운 탐색기 — 층에서 시작하고, **그 자리에서 다 읽는다.** 첫 화면에
    /// 수가 서야 `moai status` 의 한눈 보기와 같은 값을 한다.
    pub fn on_projects(layer: Layer) -> App {
        let mut app = App::build(Vec::new(), Index::of(&[]), blank_config(), Vec::new(), Vec::new());
        app.layer = Some(Layer { at: At::Layer, ..layer });
        app.refresh_layer();
        app
    }

    /// `.moai` 안에서 띄운 탐색기에 층을 얹는다. 첫 화면은 오늘처럼 첫 항목에 선다 — 새로
    /// 선 `..` 에 커서를 두면 여는 순간의 Enter 가 층으로 올라간다.
    pub fn with_layer(mut self, layer: Layer) -> App {
        self.layer = Some(layer);
        if self.path.is_empty() && self.cursor == 0 && self.rows().len() > 1 {
            self.cursor = 1;
        }
        self
    }

    /// 층의 줄로 들어간다. **늘 `Repo::open` 부터 다시 한다** — 층의 셈이 낡았어도 들어가는
    /// 것은 지금의 디렉터리다. 못 열면 층에 선 채 그 까닭을 알림으로 댄다(그 줄도 고쳐 선다).
    ///
    /// 읽기는 그 자리에서 한다. 누른 사람은 결과를 기다리고 있다(`App::reload` 와 같다).
    pub(super) fn enter_project(&mut self, at: usize) {
        let Some(layer) = &mut self.layer else { return };
        let Some(place) = layer.places.get_mut(at) else { return };
        let path = place.path.clone();
        let marks = marks_of(&path);
        let state = match Repo::open(&path) {
            Ok(Opened::Repo(repo)) => Ok(repo),
            Ok(Opened::Uninit) => Err(State::Uninit),
            Ok(Opened::Missing) => Err(State::Missing),
            Err(e) => Err(State::Unreadable(e.message)),
        };
        let repo = match state {
            Ok(repo) => repo,
            Err(state) => {
                place.look = shut(&path, &place.name, state);
                place.marks = marks;
                if let Look::Shut { said, .. } = &place.look {
                    self.notice = Some(said.clone());
                }
                return;
            }
        };
        match (self.read)(&repo, self.worktree) {
            Ok(fresh) => {
                layer.at = At::Project(path);
                self.cfg = repo.config.clone();
                self.repo = Some(repo);
                self.path.clear();
                self.remembered.clear();
                self.cursor = 0;
                self.detail.rewind();
                // 들이기 전 목록은 비었다(층에 선 동안 비워 둔다) — 커서가 붙들 정체는 `..`
                // 뿐이라 남의 프로젝트의 id 가 여기로 새지 않는다.
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
    pub(super) fn climb(&mut self) {
        let Some(layer) = &mut self.layer else { return };
        let At::Project(from) = std::mem::replace(&mut layer.at, At::Layer) else { return };
        if let Some((_, handle)) = self.pending.take() {
            self.discard(handle);
        }
        self.repo = None;
        self.issues = Vec::new();
        self.index = Index::of(&[]);
        self.states = Default::default();
        self.keep = Vec::new();
        self.unreadable = Vec::new();
        self.origin = Default::default();
        self.elsewhere = Vec::new();
        self.watched = Vec::new();
        self.stamp = None;
        self.warnings = 0;
        self.filter_text = None;
        self.trouble = None;
        self.write_failed = false;
        self.path.clear();
        self.remembered.clear();
        self.detail.rewind();
        self.refresh_layer();
        self.cursor = self.layer.as_ref().and_then(|l| l.position(&from)).unwrap_or(0);
    }

    /// 층의 낡은 줄을 **그 자리에서** 읽는다 — 사람의 손(올라가기·F5)이 부른다. 도는 읽기는
    /// 버린다: 누르기 전에 띄운 것이라 늦게 닿으면 방금 읽은 것을 옛 것으로 덮는다.
    fn refresh_layer(&mut self) {
        let now = crate::model::now();
        let Some(layer) = &mut self.layer else { return };
        let pending = layer.pending.take();
        let stale = layer.stale();
        if !stale.is_empty() {
            layer.adopt(look_at(&stale, &now));
        }
        self.now = now;
        if let Some((_, handle)) = pending {
            self.discard(handle);
        }
    }

    /// 층에서 누른 F5 — 사용자 설정부터 다시 읽고 전부 다시 연다. 커서는 보던 프로젝트에 선다.
    pub(super) fn reread_layer(&mut self) {
        let held = self.current().and_then(|r| match r {
            Row::Project(at) => self.place_path(at).map(Path::to_path_buf),
            _ => None,
        });
        let Some(layer) = &mut self.layer else { return };
        let fresh = Layer::read(layer.config.as_deref(), layer.launch.as_deref());
        let old = std::mem::replace(layer, Layer { at: At::Layer, ..fresh });
        if let Some((_, handle)) = old.pending {
            self.discard(handle);
        }
        self.refresh_layer();
        let rows = self.rows().len();
        let found = held.and_then(|h| self.layer.as_ref().and_then(|l| l.position(&h)));
        self.cursor = found.unwrap_or(self.cursor.min(rows.saturating_sub(1)));
        self.detail.rewind();
    }

    /// 걸음마다 층을 본다. 스레드가 읽어 온 것은 **어디 서 있든** 받는다 — 경로로 맞춰
    /// 들이므로 안에 들어간 뒤에 닿아도 섞일 데가 없다. 새로 읽으러 가는 것은 **층에 선
    /// 동안만**이다: 안에 있는 동안 남의 프로젝트를 걸음마다 재고 읽을 까닭이 없고, 올라갈
    /// 때 표식이 바뀐 것만 읽는다(`climb`).
    pub(super) fn follow_layer(&mut self) {
        let Some(layer) = &mut self.layer else { return };
        if let Some((rx, _)) = &layer.pending {
            match rx.try_recv() {
                Err(TryRecvError::Empty) => return,
                Ok(looked) => {
                    layer.pending = None;
                    layer.adopt(looked);
                    if layer.at == At::Layer {
                        self.now = crate::model::now();
                    }
                    return;
                }
                // 읽던 스레드가 죽었다 — 받은 읽기와 같게 되던진다(`App::follow`).
                Err(TryRecvError::Disconnected) => {
                    if let Some((_, handle)) = layer.pending.take()
                        && let Err(payload) = handle.join()
                    {
                        std::panic::resume_unwind(payload);
                    }
                }
            }
        }
        if layer.at != At::Layer {
            return;
        }
        let stale = layer.stale();
        if stale.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let now = crate::model::now();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(look_at(&stale, &now));
        });
        layer.pending = Some((rx, handle));
    }

    /// 층을 읽는 스레드가 도는가.
    pub(super) fn layer_loading(&self) -> bool {
        self.layer.as_ref().is_some_and(|l| l.pending.is_some())
    }
}

/// 층에 선 동안 `App::cfg` 자리를 채우는 설정. **층에서는 아무도 읽지 않는다** — 칸 이름을
/// 묻는 것은 프로젝트 안의 줄뿐이고, 들어가면 그 프로젝트의 설정으로 갈아 끼운다.
fn blank_config() -> crate::config::Config {
    crate::config::Config::parse("prefix = \"moai\"\n").expect("고정된 설정 글이다")
}

/// 그림 시험이 디스크 없이 층을 세운다. 읽을 것이 없게 모든 줄을 이미 본 것으로 둔다 —
/// 없는 경로의 표식은 `(false, None, None)` 이라 [`Layer::stale`] 이 다시 읽으러 가지 않는다.
#[cfg(test)]
pub(super) fn fake(places: Vec<(&str, &str, Look)>, at: At) -> Layer {
    Layer {
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
                marks: (false, None, None),
            })
            .collect(),
        problems: Vec::new(),
        config: None,
        launch: None,
        pending: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path as NavPath;
    use crate::tui::{Mode, stamp_of};

    /// 진짜 디렉터리 여럿과 사용자 설정 한 벌. **돌리는 사람의 설정은 안 읽는다** — 층에
    /// 제 설정 파일을 준다(`Layer::config`).
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "moai-layer-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        /// `.moai` 를 가진 프로젝트 하나. 줄은 `(id, 제목, 칸)`.
        fn project(&self, name: &str, lines: &[(&str, &str, &str)]) -> PathBuf {
            let dir = self.0.join(name);
            std::fs::create_dir_all(dir.join(".moai")).unwrap();
            std::fs::write(dir.join(".moai/config.toml"), "prefix = \"argos\"\n").unwrap();
            write_lines(&dir, lines);
            dir
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.0.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// 사용자 설정에 이 차례로 등록한다.
        fn register(&self, dirs: &[&Path]) -> PathBuf {
            let path = self.0.join("user/config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let body: String = dirs.iter().map(|d| format!("[[project]]\npath = {:?}\n", d.to_str().unwrap())).collect();
            std::fs::write(&path, body).unwrap();
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
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

    /// 스레드에서 읽는 것을 **끝날 때까지** 받는다. 루프가 하는 것을 흉내 낸다.
    fn settle(a: &mut App) {
        a.follow();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while a.loading() {
            assert!(std::time::Instant::now() < until, "읽기가 끝나지 않는다");
            std::thread::sleep(std::time::Duration::from_millis(2));
            a.follow();
        }
    }

    fn names(a: &App) -> Vec<String> {
        a.layer.as_ref().unwrap().places.iter().map(|p| p.name.clone()).collect()
    }

    fn look<'a>(a: &'a App, name: &str) -> &'a Look {
        &a.layer.as_ref().unwrap().places.iter().find(|p| p.name == name).unwrap().look
    }

    /// 보이는 줄의 제목 (`..` 은 뺀다).
    fn titles(a: &App) -> Vec<String> {
        a.rows()
            .iter()
            .filter_map(|r| match r {
                Row::Item(e) => e.at().map(|at| a.issues[at].title.clone()),
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
        let s = Scratch::new("list");
        let (one, two) = twins(&s);
        let bare = s.dir("bare");
        let gone = s.0.join("gone");
        let broken = s.dir("broken");
        std::fs::create_dir_all(broken.join(".moai")).unwrap();
        std::fs::write(broken.join(".moai/config.toml"), "statuses = \n").unwrap();
        let cfg = s.register(&[&one, &two, &bare, &gone, &broken]);

        let a = App::on_projects(Layer::read(Some(&cfg), None));
        assert!(a.on_layer() && a.repo.is_none() && a.issues.is_empty());
        assert_eq!(names(&a), ["one", "two", "bare", "gone", "broken"]);
        assert_eq!(a.rows(), (0..5).map(Row::Project).collect::<Vec<_>>());
        assert!(!a.loading(), "밖에서 띄운 첫 화면을 스레드에 맡겼다 — 수가 비어 선다");

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
        let s = Scratch::new("enter");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));

        a.key(key(KeyCode::Enter));
        assert!(!a.on_layer());
        assert_eq!(a.project().map(|p| p.path.clone()), Some(one.clone()));
        assert_eq!(a.repo.as_ref().map(|r| r.root.clone()), Some(one.clone()), "선 프로젝트와 쓸 저장소가 어긋났다");
        assert_eq!(a.rows().first(), Some(&Row::Up), "프로젝트 뿌리에 층으로 가는 `..` 이 없다");
        assert_eq!(titles(&a), ["one 의 첫 줄", "one 의 둘째 줄"]);

        // one 의 argos-0002 에 서고 거름망을 건다 — two 에서 argos-0002 는 다른 자리의 다른 줄이다.
        a.key(key(KeyCode::End));
        assert_eq!(a.current(), Some(Row::Item(crate::nav::Entry::Leaf { at: 1 })));
        a.key(key(KeyCode::Char('f')));
        for c in "status=in_progress".chars() {
            a.key(key(KeyCode::Char(c)));
        }
        a.key(key(KeyCode::Enter));
        assert_eq!(a.filter_text.as_deref(), Some("status=in_progress"));

        a.key(key(KeyCode::Home));
        a.key(key(KeyCode::Backspace));
        assert!(a.on_layer() && a.repo.is_none() && a.issues.is_empty(), "층에 올라왔는데 프로젝트의 줄이 남았다");
        assert_eq!(a.current(), Some(Row::Project(0)), "떠난 프로젝트에 안 섰다");
        assert_eq!(a.filter_text, None, "한 프로젝트에 건 거름망이 층까지 따라왔다");

        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        assert_eq!(a.repo.as_ref().map(|r| r.root.clone()), Some(two.clone()));
        assert_eq!(titles(&a), ["two 의 집은 줄", "two 의 줄"], "옛 프로젝트의 줄이 섞였다");
        assert_eq!(a.current(), Some(Row::Up), "들어가면 `..` 에 선다 — 옛 커서의 id 를 따라갔다");

        a.key(key(KeyCode::Left));
        assert_eq!(a.current(), Some(Row::Project(1)));
    }

    /// **`.moai` 안에서 띄우면 그 안에서 시작하고, 뿌리에서 Bksp 로 층에 올라가 띄운 자리에
    /// 선다**(결정 3). 등록 안 된 자리는 층 맨 앞에 서서 도로 내려갈 수 있다. 남의 프로젝트는
    /// 올라갈 때 처음 읽는다.
    #[test]
    fn launched_inside_it_starts_inside_and_climbs_to_where_it_was_launched() {
        let s = Scratch::new("inside");
        let (one, two) = twins(&s);
        let here = s.project("here", &[("argos-0009", "여기 줄", "todo")]);
        let cfg = s.register(&[&one, &two]);

        let repo = Repo { root: here.clone(), config: crate::config::Config::parse("prefix = \"argos\"\n").unwrap() };
        let stamp = stamp_of(&repo);
        let load = repo.read().unwrap();
        let index = Index::of(&load.issues);
        let mut a = App::open(repo, load, index, NavPath::new(), stamp).with_layer(Layer::read(Some(&cfg), Some(&here)));
        assert!(!a.on_layer());
        assert_eq!(titles(&a), ["여기 줄"]);
        assert_eq!(a.cursor, 1, "첫 화면이 새로 선 `..` 에 섰다");
        assert!(matches!(look(&a, "one"), Look::Unread), "안에서 띄웠는데 남의 프로젝트를 먼저 읽었다");

        a.key(key(KeyCode::Backspace));
        assert!(a.on_layer());
        assert_eq!(names(&a), ["here", "one", "two"]);
        let at = a.layer.as_ref().unwrap();
        assert!(at.places[0].launched && !at.places[0].registered);
        assert_eq!(a.current(), Some(Row::Project(0)), "띄운 자리에 안 섰다");
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
        let s = Scratch::new("none");
        let here = s.project("here", &[]);
        let cfg = s.register(&[]);
        assert!(!Layer::read(Some(&cfg), Some(&here)).registered());
        assert!(!Layer::read(None, Some(&here)).registered(), "설정 자리를 몰라도 층이 섰다");
        let cfg = s.register(&[&s.0.join("gone")]);
        assert!(Layer::read(Some(&cfg), Some(&here)).registered());
    }

    /// 사용자 설정에 정한 색이 층의 줄까지 실려 온다(moai-o04b) — `draw::project_style` 이 그것을
    /// 입힌다. 띄운 자리로만 선 줄은 설정에 없으니 정한 색도 없다.
    #[test]
    fn a_colour_chosen_in_the_user_config_rides_on_the_place() {
        let s = Scratch::new("hue");
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
        let s = Scratch::new("shut");
        let bare = s.dir("bare");
        let gone = s.0.join("gone");
        let cfg = s.register(&[&bare, &gone]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));

        a.key(key(KeyCode::Enter));
        assert!(a.on_layer() && a.repo.is_none());
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
        assert_eq!(a.repo.as_ref().map(|r| r.root.clone()), Some(bare));
    }

    /// **층에 선 동안 바뀐 프로젝트만 스레드에서 다시 읽는다.** 안에 들어가 있는 동안에는
    /// 남의 프로젝트를 재지도 읽지도 않는다.
    #[test]
    fn only_the_project_that_changed_is_reread_and_only_on_the_layer() {
        let s = Scratch::new("reread");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        let before = a.layer.as_ref().unwrap().places[0].marks;

        a.follow();
        assert!(!a.loading(), "아무것도 안 바뀌었는데 읽으러 갔다");

        write_lines(&two, &[("argos-0001", "two 의 집은 줄", "done")]);
        assert_eq!(a.layer.as_ref().unwrap().stale(), std::slice::from_ref(&two));
        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 안 읽었다");
        settle(&mut a);
        let Look::Open { sum } = look(&a, "two") else { panic!() };
        assert!(sum.picked.is_empty(), "다시 읽은 셈이 안 들어왔다");
        assert_eq!(a.layer.as_ref().unwrap().places[0].marks, before, "안 바뀐 프로젝트까지 다시 읽었다");

        // 안에 들어가면 층의 표식은 안 본다.
        a.key(key(KeyCode::Enter));
        write_lines(&two, &[("argos-0001", "two 의 집은 줄", "in_progress")]);
        a.follow();
        assert!(!a.loading(), "안에 있는 동안 남의 프로젝트를 읽으러 갔다");
        a.key(key(KeyCode::Backspace));
        let Look::Open { sum } = look(&a, "two") else { panic!() };
        assert_eq!(sum.picked.len(), 1, "올라갈 때 바뀐 것을 안 읽었다");
    }

    /// **`.moai` 없는 디렉터리가 사라지거나 다시 생기는 것도 본다.** 두 파일의 표식은 그
    /// 동안 둘 다 없음 그대로라, 디렉터리를 안 재면 층이 "init 전" 을 영영 댄다.
    #[test]
    fn a_bare_directory_that_disappears_is_reread() {
        let s = Scratch::new("vanish");
        let bare = s.dir("bare");
        let cfg = s.register(&[&bare]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Uninit, .. }));

        std::fs::remove_dir_all(&bare).unwrap();
        settle(&mut a);
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Missing, .. }), "사라진 디렉터리를 init 전으로 둔다");

        std::fs::create_dir_all(&bare).unwrap();
        settle(&mut a);
        assert!(matches!(look(&a, "bare"), Look::Shut { state: Shut::Uninit, .. }), "다시 생긴 디렉터리를 없다로 둔다");
    }

    /// **층에서 `n` 은 아무 데도 안 쓴다** — 담을 프로젝트가 안 정해졌다. 폼을 안 열고 왜
    /// 안 되는지를 한 줄로 말한다. 거름망·겹쳐 보기 키도 같다.
    #[test]
    fn keys_that_need_a_project_say_so_on_the_layer_and_touch_nothing() {
        let s = Scratch::new("refuse");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let files = || [&one, &two].map(|d| std::fs::read(d.join(".moai/issues.jsonl")).unwrap());
        let was = files();
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        a.user = Some("레이븐 (raven@example.com)".into());

        for (c, word) in [('n', "담을 곳이 없다"), ('f', "거름망"), ('/', "거름망"), ('w', "워크트리")] {
            a.key(key(KeyCode::Char(c)));
            assert_eq!(a.mode, Mode::Browse, "{c} 가 층에서 칸을 열었다");
            assert!(a.notice.as_deref().is_some_and(|n| n.contains(word)), "{c}: {:?}", a.notice);
        }
        assert!(!a.worktree, "층에서 겹쳐 보기를 켰다");
        assert_eq!(files(), was, "층에서 누른 키가 파일을 바꿨다");
        assert!(!one.join(".moai/journal.jsonl").exists() && !two.join(".moai/journal.jsonl").exists());

        // 들어가면 `n` 은 오늘처럼 폼을 연다.
        a.key(key(KeyCode::Enter));
        a.key(key(KeyCode::Char('n')));
        assert!(matches!(a.mode, Mode::Idea(_)));
    }

    /// **떠난 프로젝트에서 짓던 읽기는 다음 프로젝트에 안 닿는다.** 같은 id 를 쓰는 두
    /// 프로젝트에서 늦게 닿은 읽기가 들어오면 화면이 남의 줄이 된다.
    #[test]
    fn a_read_in_flight_from_the_project_left_behind_never_lands() {
        let s = Scratch::new("inflight");
        let (one, two) = twins(&s);
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));

        a.key(key(KeyCode::Enter));
        write_lines(&one, &[("argos-0001", "one 의 새 줄", "todo")]);
        a.follow();
        assert!(a.loading(), "바뀐 것을 보고도 안 읽었다");
        a.key(key(KeyCode::Backspace));
        a.key(key(KeyCode::Down));
        a.key(key(KeyCode::Enter));
        settle(&mut a);
        assert_eq!(titles(&a), ["two 의 집은 줄", "two 의 줄"], "떠난 프로젝트의 읽기가 들어왔다");
        assert_eq!(a.repo.as_ref().map(|r| r.root.clone()), Some(two));
    }

    /// **층의 F5 는 사용자 설정부터 다시 읽는다** — 밖에서 `moai project add` 한 것이 선다.
    /// 커서는 보던 프로젝트(경로)에 선다.
    #[test]
    fn f5_on_the_layer_rereads_the_registration_and_keeps_the_cursor_on_its_project() {
        let s = Scratch::new("f5");
        let (one, two) = twins(&s);
        let three = s.project("three", &[]);
        let cfg = s.register(&[&one, &two]);
        let mut a = App::on_projects(Layer::read(Some(&cfg), None));
        a.key(key(KeyCode::Down));
        assert_eq!(a.current(), Some(Row::Project(1)));

        s.register(&[&three, &one, &two]);
        a.key(key(KeyCode::F(5)));
        assert_eq!(names(&a), ["three", "one", "two"]);
        assert_eq!(a.current(), Some(Row::Project(2)), "보던 프로젝트를 놓쳤다");
        assert!(matches!(look(&a, "three"), Look::Open { .. }));
    }
}
