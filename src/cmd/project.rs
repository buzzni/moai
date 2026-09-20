//! 등록한 프로젝트 목록을 고치고 본다 — `moai project add|ls|rm|color`.
//!
//! **저장소가 아니라 사람의 설정이다.** 그래서 `Repo::discover` 를 부르지 않고
//! `.moai` 밖 어디서도 선다. 누가 했는지도 묻지 않는다 — 이력이 남는 파일이
//! 아니라서, 사람을 모르는 기계에서 등록이 멈추면 도구가 고장 난 것으로 보인다.
//!
//! 파일을 읽고 쓰는 길은 전부 `user_config` 에 있고, 등록·해제의 알맹이는 TUI 와 함께 쓰는
//! `projects::add`·`projects::remove` 다. 여기는 argv 를 그 길로 옮기고 나온 것을 그린다.
//!
//! **상대경로는 지금 자리에 붙인다.** `-C <dir>` 을 주면 `main` 이 먼저 그리로
//! 옮겨 가므로 그 디렉터리가 기준이다 — `git -C` 가 "거기서 시작한 것처럼" 인 것과
//! 같다. `-C` 만 따로 셈하면 같은 플래그가 이 명령에서만 다른 뜻이 된다.

use super::{Ctx, Fail, R, code};
use crate::style::{self, Hue, paint};
use crate::text::{one_line, shell_word, width};
use crate::user_config;
use crate::{model, projects, report};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn add(ctx: &Ctx, input: &Path) -> R<Vec<String>> {
    let config = writable_config()?;
    // 적는 길은 TUI 층의 `a` 와 하나다 — 링크 풀기·멱등·`.moai` 를 준 자리에서만 보기.
    let projects::Added { path: dir, added, initialized, unreadable } = projects::add(&config, input, &cwd()?)?;

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            path: &'a Path,
            /// 새로 넣었나. `false` 면 이미 있었다 — 실패가 아니다.
            added: bool,
            initialized: bool,
            /// 그 저장소를 못 읽는 까닭 — `project ls --json` 의 `unreadable` 상태와 같은 말이다.
            /// **읽을 때는 키가 없다.** 늘 달면 전부터 내던 줄이 바뀐다.
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<&'a str>,
            config: &'a Path,
        }
        let error = unreadable.as_deref();
        return super::json_line(&Out { path: &dir, added, initialized, error, config: &config });
    }

    let shown = one_line(&dir.display().to_string());
    let mut out = vec![if added {
        format!("등록함  {shown}")
    } else {
        format!("{}  {shown}", paint(style::DIM, "이미 등록돼 있다"))
    }];
    // 한 줄은 `project ls` 의 끝 칸과 같은 말이다 — 두 화면이 같은 디렉터리를 달리 부르지 않는다.
    if let Some(error) = &unreadable {
        out.push(format!("  {} 못 읽는다 — {}", paint(style::ERROR, "!"), one_line(error)));
    }
    if !initialized {
        out.push(paint(
            style::DIM,
            &format!(
                "  init 전 — .moai 가 아직 없다. `moai -C {} init` 으로 시작하면 보인다",
                shell_word(&dir.display().to_string())
            ),
        ));
    }
    Ok(out)
}

pub fn rm(ctx: &Ctx, input: &Path) -> R<Vec<String>> {
    let config = writable_config()?;
    // 빼는 길은 TUI 층의 `d` 와 하나다 — 철자 여럿으로 견주고, 설정 파일이 없으면 안 만든다.
    let projects::Removed { spelled, removed } = projects::remove(&config, input, &cwd()?)?;

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            /// 준 것을 글자로 정리한 경로. 사라진 디렉터리도 이 철자로 찾는다.
            path: &'a Path,
            /// 실제로 뺀 경로. **비었으면 등록돼 있지 않았다** — 실패가 아니다.
            removed: &'a [PathBuf],
            config: &'a Path,
        }
        return super::json_line(&Out { path: &spelled, removed: &removed, config: &config });
    }

    if removed.is_empty() {
        return Ok(vec![format!(
            "{}  {}",
            paint(style::DIM, "등록돼 있지 않다"),
            one_line(&spelled.display().to_string())
        )]);
    }
    let mut out: Vec<String> = removed.iter().map(|p| format!("뺌  {}", one_line(&p.display().to_string()))).collect();
    out.push(paint(style::DIM, "  목록에서만 뺐다 — 디렉터리와 그 .moai 는 그대로다"));
    Ok(out)
}

/// 등록한 프로젝트가 입을 색을 정한다. `auto` 는 정한 것을 지워 경로로 고르게 한다.
///
/// - **값부터 잰다.** 팔레트 밖 값으로는 락도 안 잡고 설정 디렉터리도 안 만든다. 자는
///   설정 읽기와 같은 [`user_config::hue_choice`] 다 — 명령이 받은 값을 읽기가 틀렸다고
///   하면 시킨 대로 했는데 알림이 선다
/// - **등록 안 된 디렉터리는 멈춘다**(`not_found`). 저절로 등록하지 않는다 — 색을 고르다
///   목록이 느는 것은 시킨 일이 아니다. 찾는 철자는 `rm` 과 같아 사라진 디렉터리도 된다
/// - 같은 색이면 파일을 안 건드린다(`Doc::set_hue`)
/// - 손으로 적은 표 모양 `color`(`color.x = 1`)는 덮지 않고 멈춘다 — 그 줄을 사람이 고친다
pub fn color(ctx: &Ctx, input: &Path, word: &str) -> R<Vec<String>> {
    let hue = user_config::hue_choice(word).map_err(|e| Fail::coded(e, code::BAD_INPUT))?;
    let config = writable_config()?;
    let spellings = user_config::spellings(input, &cwd()?);
    let not_registered = || {
        let shown = one_line(&spellings[0].display().to_string());
        Fail::coded(
            format!(
                "등록돼 있지 않다 — {shown} · `moai project add {}` 로 먼저 더한다",
                shell_word(&spellings[0].display().to_string())
            ),
            code::NOT_FOUND,
        )
    };
    // 설정 파일이 없으면 등록한 것도 없다 — `update` 로 가면 아무것도 못 바꾸려고 설정 디렉터리를 만든다.
    if !config.exists() {
        return Err(not_registered());
    }
    let (before, changed) = user_config::update(&config, |doc| {
        let found = doc.projects().0.into_iter().find(|p| spellings.contains(&p.path));
        // **맞은 줄이 없어도 부른다**(moai-gmdu 에픽 리뷰) — 목록의 모양이 틀렸으면(`project = [{ … }]`)
        // `projects()` 가 비어 "등록돼 있지 않다" 로 새고, 그 말이 시키는 `add` 는 모양 때문에 거절된다.
        // 목록을 고치는 쓰기가 모두 거절하는 자리(`Doc::set_hue`)까지 가야 까닭이 선다. 맞은 줄이 없으면
        // 아무것도 안 바꾼다. 거절문의 파일 자리는 `update` 가 붙인다.
        doc.set_hue(&spellings, hue)?;
        // 바뀌었는지는 **문서가** 안다 — 앞뒤 색을 견주면 틀린 값(`red` → auto)을 지운 쓰기가
        // "이미 그렇다" 로 선다. 읽기는 틀린 값을 `None` 으로 접기 때문이다.
        Ok(found.map(|p| (p, doc.changed())))
    })?
    .ok_or_else(not_registered)?;

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            /// 설정에 적힌 경로.
            path: &'a Path,
            /// 이제 정한 색. `null` 이면 경로로 고른다.
            color: Option<&'static str>,
            /// 전에 정해 있던 색. 틀린 값이었으면 `null` 이다 — 읽기가 그것을 안 받았다.
            was: Option<&'static str>,
            /// 설정 파일을 고쳐 썼나. `false` 면 이미 그랬다 — 실패가 아니다.
            changed: bool,
            config: &'a Path,
        }
        return super::json_line(&Out {
            path: &before.path,
            color: hue.map(Hue::name),
            was: before.hue.map(Hue::name),
            changed,
            config: &config,
        });
    }

    let shown = one_line(&before.path.display().to_string());
    // 색 낱말을 그 색으로 칠한다 — 낱말이 뜻을 지고 색은 곁들인다. `auto` 면 경로가 고른 색을 댄다.
    let now = hue.unwrap_or_else(|| Hue::of_path(&before.path));
    let word = paint(style::project_colour(&before.path, Some(now)), now.name());
    let said = match hue {
        Some(_) => format!("색 {word}"),
        None => format!("색 {AUTO} — 경로로 고른다 ({word})", AUTO = user_config::AUTO),
    };
    let mut line = format!("{said}  {shown}");
    if !changed {
        line = format!("{}  {line}", paint(style::DIM, "이미 그렇다"));
    }
    Ok(vec![line])
}

/// 등록한 프로젝트 하나가 지금 어떤 모양인가. **파일에 적지 않는다** — 볼 때마다
/// 디스크에서 읽는 파생값이다.
///
/// 여는 길은 `.moai` 밖 한눈 보기와 같은 [`projects::open`] 하나다. 따로 들여다보면
/// (한때 `is_dir` 두 번이었다) 설정이 깨진 프로젝트를 `ls` 는 "있음" 으로, 한눈
/// 보기는 "못 읽는다" 로 말해 두 화면이 같은 디렉터리를 달리 부른다.
///
/// **`--json` 의 `state` 낱말은 옛 것을 그대로 둔다.** 연 것은 한눈 보기의 `ok` 가
/// 아니라 전부터 내던 `initialized` 다 — 이미 나간 값을 바꾸면 읽던 쪽이 모르는 채로
/// 멀쩡한 줄을 모르는 상태로 읽는다. 새로 선 것은 더하기만 한다: `unreadable`(+`error`)
/// 과, 연 것 곁의 `counts`·`unreadable`. 그래서 [`projects::Seen`] 을 그대로 싣지
/// 않고 여기서 한 번 옮긴다 — 나머지 낱말(`uninitialized`·`missing`·`unreadable`)은
/// 그쪽과 같다.
#[derive(serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum State<'a> {
    /// 설정과 스냅샷까지 읽었다.
    Initialized {
        /// 칸별 이슈 수 — 한눈 보기와 같은 자(`report::status(..).counts`)로 센다.
        /// 에픽·마일스톤과 미룬 것은 안 센다. 자를 따로 두면 두 화면의 수가 어긋난다.
        counts: BTreeMap<String, usize>,
        /// 못 읽는 줄의 수. 그 줄의 이슈는 `counts` 에서 빠져 있다.
        unreadable: usize,
        /// 칸의 차례 — 설정에 적힌 대로 그린다. `counts` 는 이름순이라 쓸 수 없다.
        #[serde(skip)]
        columns: &'a [String],
    },
    /// 디렉터리는 있는데 `.moai` 가 없다 — 등록한 뒤 `moai init` 하면 보인다.
    Uninitialized,
    /// 디렉터리가 없다. 옮겼거나 지웠다. 목록에서 빼는 것은 사람의 몫이다.
    Missing,
    /// 설정이 깨졌거나, 스냅샷을 못 읽거나, 등록한 경로가 디렉터리가 아니다.
    /// **"있음" 으로 접지 않는다** — 접으면 `ls` 가 괜찮다고 한 프로젝트를 `status` 가 못 연다.
    Unreadable { error: &'a str },
}

/// 정한 색을 `--json` 에 이름으로 싣는다. `None` 은 `skip_serializing_if` 가 빼므로 여기 안 온다.
fn hue_name<S: serde::Serializer>(hue: &Option<Hue>, s: S) -> Result<S::Ok, S::Error> {
    match hue {
        Some(h) => s.serialize_str(h.name()),
        None => s.serialize_none(),
    }
}

#[derive(serde::Serialize)]
struct Row<'a> {
    name: &'a str,
    path: &'a Path,
    /// 사용자 설정에 정한 색. `--json` 에는 그 **이름**이 `color` 로 선다.
    ///
    /// **정했을 때만 선다** — 키를 더하기만 해, 이미 나간 줄의 모양은 색을 안 정한 사람에게
    /// 한 글자도 안 바뀐다. **경로로 고른 색은 싣지 않는다** — 실으면 팔레트와 해시가 기계
    /// 계약이 되어 못 바꾼다. 사람이 적은 것만 계약이다.
    ///
    /// 이름을 곁에 **따로 들지 않는다**: 이름은 이 값에서 나오는 파생값이라, 두 필드로 두면
    /// 칠하는 쪽과 적는 쪽이 갈라져 초록으로 칠한 줄이 `"color":"blue"` 로 나갈 수 있다.
    #[serde(rename = "color", skip_serializing_if = "Option::is_none", serialize_with = "hue_name")]
    hue: Option<Hue>,
    #[serde(flatten)]
    state: State<'a>,
}

/// **실패하지 않는다.** 설정이 깨졌거나 등록한 디렉터리가 사라져도 보이는 것은
/// 다 보이고 까닭은 한 줄씩 선다. 사람의 설정 파일 하나로 비영 종료하면 이것을
/// 부른 스크립트가 도구가 고장 난 줄 안다 (`moai status` 가 막지 않는 것과 같다).
/// 등록한 프로젝트의 `.moai` 가 깨진 것도 같다 — 그 줄에만 선다.
pub fn ls(ctx: &Ctx) -> R<Vec<String>> {
    // **설정은 [`Ctx::registry`] 로 읽는다**(리뷰) — 아래에서 `ctx.lang()` 으로 말을 묻는데, 제 손으로
    // 한 번 더 읽으면 한 명령이 같은 파일을 두 번 판다(moai-u8cs 가 걷어 낸 그것이다).
    let reg = ctx.registry();
    let projects = projects::open(reg);
    let now = model::now();
    let rows: Vec<Row> =
        projects.iter().map(|p| Row { name: &p.name, path: &p.path, hue: p.hue, state: state(p, &now) }).collect();

    if ctx.json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            config: Option<&'a Path>,
            projects: &'a [Row<'a>],
            problems: &'a [String],
        }
        // 설정의 탈은 **편 뒤에** 싣는다(리뷰) — 화면 말의 탈은 `problems` 가 아니라 `lang_problems`
        // 에 자료로 서므로(moai-dpbi), `reg.problems` 를 그냥 실으면 `lang` 오타가 여기서만 사라진다.
        let problems = crate::view::settings_problems(reg, ctx.lang());
        return super::json_line(&Out { config: reg.path.as_deref(), projects: &rows, problems: &problems });
    }

    for p in crate::view::settings_problems(reg, ctx.lang()) {
        eprintln!("moai: {}", one_line(&p));
    }
    let mut out = Vec::new();
    if rows.is_empty() {
        out.push("등록한 프로젝트가 없다 — `moai project add <디렉터리>` 로 더한다".into());
    } else {
        let names: Vec<String> = rows.iter().map(|r| one_line(r.name)).collect();
        let paths: Vec<String> = rows.iter().map(|r| one_line(&r.path.display().to_string())).collect();
        let name_w = names.iter().map(|n| width(n)).max().unwrap_or(0);
        let path_w = paths.iter().map(|p| width(p)).max().unwrap_or(0);
        for ((r, name), path) in rows.iter().zip(&names).zip(&paths) {
            // 이름은 한눈 보기·탐색기와 **같은 색**을 입는다 — 이 목록이 색의 범례가 된다.
            // 칠하는 것만 더해 글자와 칸 폭은 그대로다.
            out.push(format!(
                "{}{}  {path}{}  {}",
                paint(style::project_colour(r.path, r.hue), name),
                " ".repeat(name_w - width(name)),
                " ".repeat(path_w - width(path)),
                said(&r.state),
            ));
        }
    }
    if let Some(config) = &reg.path {
        out.push(paint(style::DIM, &format!("설정: {}", one_line(&config.display().to_string()))));
    }
    Ok(out)
}

/// 연 프로젝트 하나를 `ls` 의 낱말로 옮긴다. 못 읽는 줄을 넘기는 자는 한눈 보기와 같다
/// — 넘기지 않으면 그 줄이 쓰던 id 와의 중복이 셈에서 달라진다.
fn state<'a>(p: &'a projects::Project, now: &str) -> State<'a> {
    match &p.state {
        projects::State::Open { repo, load } => {
            let unreadable = load.unreadable();
            State::Initialized {
                counts: report::status(&load.issues, &unreadable, &repo.config, now).counts,
                unreadable: load.errors.len(),
                columns: &repo.config.statuses,
            }
        }
        projects::State::Uninit => State::Uninitialized,
        projects::State::Missing => State::Missing,
        projects::State::Unreadable(e) => State::Unreadable { error: e },
    }
}

/// 목록 한 줄의 끝 칸. 칸별 수는 한눈 보기의 보드 줄과 같은 글리프·낱말이되 한 줄에
/// 들게 사이를 좁힌다. **색이 혼자 뜻을 지지 않는다** — 글리프와 낱말이 늘 곁에 선다.
fn said(state: &State) -> String {
    match state {
        State::Initialized { counts, unreadable, columns } => {
            let mut cols: Vec<String> = columns
                .iter()
                .map(|s| {
                    let n = counts.get(s).copied().unwrap_or(0);
                    let style = style::status_style(s);
                    // 칸 이름도 남의 설정 파일에서 온다 — `statuses` 는 제어문자를 거르지 않는다.
                    let word = one_line(s);
                    format!("{} {}", paint(style, style::glyph(s)), paint(style, &format!("{word} {n}")))
                })
                .collect();
            if *unreadable > 0 {
                cols.push(format!("{} 읽을 수 없는 줄 {unreadable}개", paint(style::ERROR, "!")));
            }
            cols.join("  ")
        }
        State::Uninitialized => paint(style::DIM, "init 전"),
        State::Missing => paint(style::WARN, "디렉터리가 없다"),
        // 까닭은 한 줄에 둔다 — 줄바꿈이 섞이면 다음 프로젝트의 줄과 갈리지 않는다.
        State::Unreadable { error } => format!("{} 못 읽는다 — {}", paint(style::ERROR, "!"), one_line(error)),
    }
}

/// 쓸 설정 파일의 자리. 모르면 **쓰기는 멈춘다** — 어디에 적었는지 모르는 등록은
/// 다음 `ls` 에서 안 보이는 등록이다. `moai read` 도 이것을 부른다 — 같은 조건에 두 명령이
/// 다른 말(고칠 길을 대는 말과 안 대는 말)을 하지 않게(moai-j038.vna).
pub(super) fn writable_config() -> R<PathBuf> {
    user_config::path().ok_or_else(|| {
        Fail::coded("사용자 설정의 자리를 모른다 — MOAI_CONFIG·XDG_CONFIG_HOME·HOME 중 하나를 준다", code::ERROR)
    })
}

fn cwd() -> R<PathBuf> {
    std::env::current_dir().map_err(|e| Fail::new(format!("지금 자리를 모른다: {e}")))
}
