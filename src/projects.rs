//! 등록한 프로젝트들을 **프로젝트마다 따로** 연다.
//!
//! `.moai` 밖에서 부른 `status`·`ready` 가 여기서 읽는다. 여러 프로젝트의 줄을
//! 한 `Vec<Issue>` 로 합치지 않는다 — 프로젝트마다 id 가 겹칠 수 있고(접두어가 같은
//! 두 저장소), 합친 순간 `report` 가 남의 줄을 제 부모·막음으로 읽는다. 옛 moai 의
//! `merge=union` 이 같은 자리에서 id 중복을 만들었다.
//!
//! **출처는 저장하지 않는다.** 줄이 어느 프로젝트의 것인지는 보여줄 때만 아는
//! 파생값이라 [`Project`] 가 제 `Load` 를 곁에 들고, `report`·`query` 는 여전히 한
//! 프로젝트의 `&[Issue]` 만 받는다(`worktree::Origin` 과 같은 약속).
//!
//! **읽기는 관대하다.** 사라진 디렉터리·init 전·깨진 설정은 그 프로젝트의 상태로
//! 설 뿐 다른 프로젝트를 막지 않는다. 등록한 것이 남의 파일일 때 도구가 실패로
//! 보이면 안 된다.

use crate::store::{Load, Opened, Repo};
use crate::user_config::Registry;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 등록한 프로젝트 하나를 연 것.
pub struct Project {
    /// 등록한 철자 그대로 — 사람이 적은 경로다.
    pub path: PathBuf,
    /// 화면에 댈 이름. 디렉터리 이름이고, 겹치면 위 디렉터리를 붙여 가른다
    /// ([`crate::user_config::names`] — `moai project ls` 와 같은 자다). 등록 목록이 바뀌면
    /// 달라질 수 있어 **정체로 쓰지 않는다** — 정체는 `path` 다.
    pub name: String,
    /// 사용자 설정에 정한 색 — 없으면 경로로 고른다. 칠하는 쪽은 늘
    /// `style::project_colour(&path, hue)` 로 부른다.
    pub hue: Option<crate::style::Hue>,
    pub state: State,
}

pub enum State {
    Open { repo: Repo, load: Load },
    /// 디렉터리는 있는데 `.moai/` 가 없다 — 나중에 `moai init` 하면 보인다.
    Uninit,
    /// 디렉터리가 없다.
    Missing,
    /// 설정이 깨졌거나 못 읽는다. 사람이 읽을 한 줄.
    Unreadable(String),
}

/// 등록 목록 차례 그대로 연다. **실패하지 않는다.**
pub fn open(reg: &Registry) -> Vec<Project> {
    reg.projects
        .iter()
        .zip(crate::user_config::names(&reg.projects))
        .map(|(p, name)| {
            let state = match Repo::open(&p.path) {
                Ok(Opened::Repo(repo)) => match repo.read() {
                    Ok(load) => State::Open { repo, load },
                    Err(e) => State::Unreadable(e.message),
                },
                Ok(Opened::Uninit) => State::Uninit,
                Ok(Opened::Missing) => State::Missing,
                Err(e) => State::Unreadable(e.message),
            };
            Project { path: p.path.clone(), name, hue: p.hue, state }
        })
        .collect()
}

impl Project {
    /// 연 프로젝트면 `f` 로 그 프로젝트 하나만 본 것을 들고, 아니면 그 상태를 든다.
    ///
    /// 받는 쪽이 넷을 매번 `match` 하지 않고 **연 것에 대해서만** 말하게 한다 —
    /// `--json` 과 사람 화면이 같은 [`Seen`] 을 받아 상태 낱말이 갈라지지 않는다.
    pub fn seen<'a, T>(&'a self, f: impl FnOnce(&'a Repo, &'a Load) -> T) -> Seen<'a, T> {
        match &self.state {
            State::Open { repo, load } => Seen::Ok(f(repo, load)),
            State::Uninit => Seen::Uninit,
            State::Missing => Seen::Missing,
            State::Unreadable(e) => Seen::Unreadable { error: e },
        }
    }
}

/// 프로젝트 하나에서 본 것. `--json` 에서는 `state` 키로 가른다 — `ok` 이면 `T` 의
/// 필드가 곁에 선다.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Seen<'a, T> {
    Ok(T),
    /// `moai project ls --json` 과 같은 낱말이다 — 같은 상태를 두 명령이 달리 부르면
    /// 둘을 함께 읽는 쪽이 두 낱말을 다 알아야 한다.
    #[serde(rename = "uninitialized")]
    Uninit,
    Missing,
    Unreadable { error: &'a str },
}

impl<'a, T> Seen<'a, T> {
    /// 연 것만 바꾼다. 셈(`Board`)과 기계 출력의 모양을 한 상태 낱말로 잇는다.
    pub fn map<'b, U>(&'b self, f: impl FnOnce(&'b T) -> U) -> Seen<'a, U> {
        match self {
            Seen::Ok(t) => Seen::Ok(f(t)),
            Seen::Uninit => Seen::Uninit,
            Seen::Missing => Seen::Missing,
            Seen::Unreadable { error } => Seen::Unreadable { error },
        }
    }
}

/// `--json` 의 프로젝트 한 줄 — 이름·경로 곁에 본 것.
#[derive(Serialize)]
pub struct Entry<'a, T> {
    pub name: &'a str,
    pub path: &'a Path,
    #[serde(flatten)]
    pub seen: Seen<'a, T>,
}

/// `--json` 의 한눈 보기 전체. **`projects` 키가 곧 "여러 프로젝트를 봤다" 는 뜻이다**
/// — `.moai` 안에서 부른 `status --json` 에는 이 키가 없다.
#[derive(Serialize)]
pub struct Overview<'a, T> {
    pub projects: Vec<Entry<'a, T>>,
    /// 사용자 설정을 읽다 만난 것. 비었으면 아무 일 없다.
    pub problems: &'a [String],
    /// 읽은 사용자 설정 파일. 자리를 모르면 `null`.
    pub config: Option<&'a Path>,
}
