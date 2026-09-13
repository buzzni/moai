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
use std::path::{Component, Path, PathBuf};

/// 등록한 프로젝트 하나를 연 것.
pub struct Project {
    /// 등록한 철자 그대로 — 사람이 적은 경로다.
    pub path: PathBuf,
    /// 화면에 댈 이름. 디렉터리 이름이고, 겹치면 위 디렉터리를 붙여 가른다([`names`]).
    pub name: String,
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
    let paths: Vec<&Path> = reg.projects.iter().map(|p| p.path.as_path()).collect();
    reg.projects
        .iter()
        .zip(names(&paths))
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
            Project { path: p.path.clone(), name, state }
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

/// 화면에 댈 이름들. 디렉터리 이름이 겹치면 **겹치는 것끼리만** 위 디렉터리를 하나씩
/// 붙인다 — `work/api`·`play/api`. 겹치지 않는 것은 짧은 채로 둔다.
///
/// 이름은 등록 목록이 바뀌면 달라질 수 있다(같은 이름이 새로 들면). 그래서 **정체로
/// 쓰지 않는다** — 정체는 경로다. 색 배정(moai-xs9x)도 경로로 한다.
pub fn names(paths: &[&Path]) -> Vec<String> {
    let parts: Vec<Vec<String>> = paths
        .iter()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let label = |k: usize, depth: usize| -> String {
        let c = &parts[k];
        match c.is_empty() {
            // `/` 처럼 이름 댈 조각이 없는 경로는 경로가 이름이다.
            true => paths[k].display().to_string(),
            false => c[c.len().saturating_sub(depth)..].join("/"),
        }
    };
    let mut depth = vec![1usize; paths.len()];
    loop {
        let labels: Vec<String> = (0..paths.len()).map(|k| label(k, depth[k])).collect();
        let mut grew = false;
        for k in 0..paths.len() {
            let clash = labels.iter().enumerate().any(|(j, l)| j != k && *l == labels[k]);
            if clash && depth[k] < parts[k].len() {
                depth[k] += 1;
                grew = true;
            }
        }
        if !grew {
            return labels;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(paths: &[&str]) -> Vec<String> {
        let p: Vec<&Path> = paths.iter().map(Path::new).collect();
        names(&p)
    }

    #[test]
    fn names_are_the_directory_unless_they_clash() {
        assert_eq!(named(&["/w/api", "/w/web"]), ["api", "web"]);
        assert_eq!(named(&["/work/api", "/play/api", "/w/web"]), ["work/api", "play/api", "web"]);
        // 겹치는 조각이 깊어도 갈릴 때까지만 붙인다.
        assert_eq!(named(&["/a/x/api", "/b/x/api"]), ["a/x/api", "b/x/api"]);
        // 한쪽이 더 못 올라가도 끝난다.
        assert_eq!(named(&["/api", "/x/api"]), ["api", "x/api"]);
        assert_eq!(named(&["/"]), ["/"]);
    }
}
