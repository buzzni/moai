//! 등록한 프로젝트 목록을 고치고 본다 — `moai project add|ls|rm`.
//!
//! **저장소가 아니라 사람의 설정이다.** 그래서 `Repo::discover` 를 부르지 않고
//! `.moai` 밖 어디서도 선다. 누가 했는지도 묻지 않는다 — 이력이 남는 파일이
//! 아니라서, 사람을 모르는 기계에서 등록이 멈추면 도구가 고장 난 것으로 보인다.
//!
//! 파일을 읽고 쓰는 길은 전부 `user_config` 에 있다. 여기는 argv 를 그 길로
//! 옮기고 나온 것을 그린다.
//!
//! **상대경로는 지금 자리에 붙인다.** `-C <dir>` 을 주면 `main` 이 먼저 그리로
//! 옮겨 가므로 그 디렉터리가 기준이다 — `git -C` 가 "거기서 시작한 것처럼" 인 것과
//! 같다. `-C` 만 따로 셈하면 같은 플래그가 이 명령에서만 다른 뜻이 된다.

use super::{Ctx, Fail, R, code};
use crate::style::{self, paint};
use crate::text::{sanitize, width};
use crate::user_config::{self, Project};
use std::path::{Path, PathBuf};

pub fn add(ctx: &Ctx, input: &Path) -> R<Vec<String>> {
    let config = writable_config()?;
    let dir = user_config::resolve_dir(input, &cwd()?)?;
    let added = user_config::update(&config, |doc| doc.add(&dir))?;
    // `.moai` 는 **준 디렉터리에서만** 본다. 위로 찾아 올라가면 모노레포의
    // `apps/a` 가 루트의 `.moai` 를 제 것으로 읽고, 그러면 따로 등록한 뜻이 없다.
    let initialized = dir.join(".moai").is_dir();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            path: &'a Path,
            /// 새로 넣었나. `false` 면 이미 있었다 — 실패가 아니다.
            added: bool,
            initialized: bool,
            config: &'a Path,
        }
        return super::json_line(&Out { path: &dir, added, initialized, config: &config });
    }

    let shown = sanitize(&dir.display().to_string());
    let mut out = vec![if added {
        format!("등록함  {shown}")
    } else {
        format!("{}  {shown}", paint(style::DIM, "이미 등록돼 있다"))
    }];
    if !initialized {
        out.push(paint(
            style::DIM,
            &format!("  init 전 — .moai 가 아직 없다. `moai -C {shown} init` 으로 시작하면 보인다"),
        ));
    }
    Ok(out)
}

pub fn rm(ctx: &Ctx, input: &Path) -> R<Vec<String>> {
    let config = writable_config()?;
    let spellings = user_config::spellings(input, &cwd()?);
    // **설정 파일이 없으면 뺄 것도 없다.** 그대로 `update` 로 가면 빈 목록에서
    // 아무것도 안 빼려고 설정 디렉터리를 만든다 — 아무 일도 안 한 명령이 사람의
    // `~/.config` 에 흔적을 남긴다.
    let removed: Vec<PathBuf> = if config.exists() {
        user_config::update(&config, |doc| {
            let hit: Vec<PathBuf> =
                doc.projects().0.into_iter().map(|p| p.path).filter(|p| spellings.contains(p)).collect();
            doc.remove(&spellings);
            Ok(hit)
        })?
    } else {
        Vec::new()
    };

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            /// 준 것을 글자로 정리한 경로. 사라진 디렉터리도 이 철자로 찾는다.
            path: &'a Path,
            /// 실제로 뺀 경로. **비었으면 등록돼 있지 않았다** — 실패가 아니다.
            removed: &'a [PathBuf],
            config: &'a Path,
        }
        return super::json_line(&Out { path: &spellings[0], removed: &removed, config: &config });
    }

    if removed.is_empty() {
        return Ok(vec![format!(
            "{}  {}",
            paint(style::DIM, "등록돼 있지 않다"),
            sanitize(&spellings[0].display().to_string())
        )]);
    }
    let mut out: Vec<String> =
        removed.iter().map(|p| format!("뺌  {}", sanitize(&p.display().to_string()))).collect();
    out.push(paint(style::DIM, "  목록에서만 뺐다 — 디렉터리와 그 .moai 는 그대로다"));
    Ok(out)
}

/// 등록한 프로젝트 하나가 지금 어떤 모양인가. **파일에 적지 않는다** — 볼 때마다
/// 디스크에서 읽는 파생값이다.
///
/// 셋은 `.moai` 밖 한눈 보기(moai-6au6)가 디렉터리 하나를 여는 결과와 한 짝씩
/// 맞춘다 — 그쪽이 들어오면 여기는 그것을 부르는 것으로 바뀌고 모양은 그대로다.
#[derive(serde::Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
enum State {
    /// `.moai` 가 있다.
    Initialized,
    /// 디렉터리는 있는데 `.moai` 가 없다 — 등록한 뒤 `moai init` 하면 보인다.
    Uninitialized,
    /// 디렉터리가 없다. 옮겼거나 지웠다. 목록에서 빼는 것은 사람의 몫이다.
    Missing,
}

#[derive(serde::Serialize)]
struct Row {
    name: String,
    path: PathBuf,
    state: State,
}

/// **실패하지 않는다.** 설정이 깨졌거나 등록한 디렉터리가 사라져도 보이는 것은
/// 다 보이고 까닭은 한 줄씩 선다. 사람의 설정 파일 하나로 비영 종료하면 이것을
/// 부른 스크립트가 도구가 고장 난 줄 안다 (`moai status` 가 막지 않는 것과 같다).
pub fn ls(ctx: &Ctx) -> R<Vec<String>> {
    let reg = user_config::read(user_config::path().as_deref());
    let names = user_config::names(&reg.projects);
    let rows: Vec<Row> = reg
        .projects
        .iter()
        .zip(names)
        .map(|(Project { path }, name)| Row { name, path: path.clone(), state: inspect(path) })
        .collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            config: Option<&'a Path>,
            projects: &'a [Row],
            problems: &'a [String],
        }
        return super::json_line(&Out { config: reg.path.as_deref(), projects: &rows, problems: &reg.problems });
    }

    for p in &reg.problems {
        eprintln!("moai: {}", sanitize(p));
    }
    let mut out = Vec::new();
    if rows.is_empty() {
        out.push("등록한 프로젝트가 없다 — `moai project add <디렉터리>` 로 더한다".into());
    } else {
        let names: Vec<String> = rows.iter().map(|r| sanitize(&r.name)).collect();
        let paths: Vec<String> = rows.iter().map(|r| sanitize(&r.path.display().to_string())).collect();
        let name_w = names.iter().map(|n| width(n)).max().unwrap_or(0);
        let path_w = paths.iter().map(|p| width(p)).max().unwrap_or(0);
        for ((r, name), path) in rows.iter().zip(&names).zip(&paths) {
            let state = match r.state {
                State::Initialized => ".moai 있음".to_string(),
                State::Uninitialized => paint(style::DIM, "init 전"),
                State::Missing => paint(style::WARN, "디렉터리가 없다"),
            };
            out.push(format!(
                "{name}{}  {path}{}  {state}",
                " ".repeat(name_w - width(name)),
                " ".repeat(path_w - width(path)),
            ));
        }
    }
    if let Some(config) = &reg.path {
        out.push(paint(style::DIM, &format!("설정: {}", sanitize(&config.display().to_string()))));
    }
    Ok(out)
}

/// 디렉터리 하나를 들여다본다. **`.moai` 는 준 디렉터리에서만 본다.**
///
/// 이슈 수는 아직 안 낸다. 줄을 세는 것만으로는 칸도 종류도 못 가르고, 칸을
/// 가르려면 저장소를 설정째 여는 길이 있어야 한다 — 그 길은 moai-6au6 이 `store`
/// 에 둔다. 지금 줄 수를 `issues` 로 내보내면 그쪽이 들어오는 날 같은 키의 뜻이
/// 바뀌어, `--json` 을 읽던 쪽이 모르는 채로 다른 수를 읽는다.
fn inspect(dir: &Path) -> State {
    match (dir.is_dir(), dir.join(".moai").is_dir()) {
        (false, _) => State::Missing,
        (true, false) => State::Uninitialized,
        (true, true) => State::Initialized,
    }
}

/// 쓸 설정 파일의 자리. 모르면 **쓰기는 멈춘다** — 어디에 적었는지 모르는 등록은
/// 다음 `ls` 에서 안 보이는 등록이다.
fn writable_config() -> R<PathBuf> {
    user_config::path().ok_or_else(|| {
        Fail::coded(
            "사용자 설정의 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 중 하나를 준다",
            code::ERROR,
        )
    })
}

fn cwd() -> R<PathBuf> {
    std::env::current_dir().map_err(|e| Fail::new(format!("지금 자리를 모른다: {e}")))
}
