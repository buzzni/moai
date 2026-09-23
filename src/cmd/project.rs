//! 등록한 프로젝트 목록을 고치고 본다 — `moai project add|ls|rm|color`.
//!
//! **저장소가 아니라 사람의 설정이다.** 그래서 `cmd::open_repo` 를 부르지 않고
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
    let config = writable_config(ctx)?;
    // 적는 길은 TUI 층의 `a` 와 하나다 — 링크 풀기·멱등·`.moai` 를 준 자리에서만 보기.
    // **자리는 여는 자리에서 함께 세어 온다**(리뷰 10번) — 그리는 쪽이 다시 물으면 `Repo::open`
    // 이 이미 푼 답을 더 무거운 자로 또 풀고, 그 값을 `project ls` 는 줄마다 치른다.
    let projects::Added { path: dir, added, initialized, tracker_at, unreadable } =
        projects::add(&config, input, &cwd(ctx.lang())?, ctx.lang())?;

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
            /// `init` 이 여기 안 서는 딸린 워크트리면 **트래커가 설 주 체크아웃**(moai-nppo) —
            /// 사람 줄이 대는 그 자리다. **아닐 때는 키가 없다**: 늘 달면 전부터 내던 줄이 바뀌고,
            /// 있는 것 자체가 "여기에 `init` 하지 마라" 라 받는 쪽이 값을 또 가를 것이 없다.
            #[serde(skip_serializing_if = "Option::is_none")]
            tracker_at: Option<PathBuf>,
            config: &'a Path,
        }
        let error = unreadable.as_deref();
        return super::json_line(&Out { path: &dir, added, initialized, error, tracker_at, config: &config });
    }

    let shown = one_line(&dir.display().to_string());
    // **갈래마다 제 `say` 를 적되 갈림은 하나다** — 키를 고르는 `match` 와 칠하는 `match` 를
    // 따로 두면 갈래가 하나 늘 때 한쪽만 고쳐진다(리뷰).
    let mut out = vec![match added {
        true => format!("{}  {shown}", crate::i18n::say(ctx.lang(), "project.added")),
        false => format!("{}  {shown}", paint(style::DIM, crate::i18n::say(ctx.lang(), "project.already"))),
    }];
    // 한 줄은 `project ls` 의 끝 칸과 같은 말이다 — 두 화면이 같은 디렉터리를 달리 부르지 않는다.
    if let Some(error) = &unreadable {
        let why = crate::i18n::fill(
            crate::i18n::say(ctx.lang(), "overview.project_unreadable"),
            &[("why", &one_line(error))],
        );
        out.push(format!("  {} {why}", paint(style::ERROR, "!")));
    }
    if !initialized {
        out.push(paint(style::DIM, &format!("  {}", uninit_line(&dir, tracker_at.as_deref(), ctx.lang()))));
    }
    Ok(out)
}

/// `init` 전인 한 줄. **딸린 워크트리면 여기가 아니라 주 체크아웃을 댄다**(moai-nppo).
///
/// 대던 `moai -C <여기> init` 은 그 자리에서 1 로 끝난다(moai-mz0e 가 거절을 세웠다) — 그러니
/// 등록한 워크트리 한 줄은 영영 `init 전` 으로 서고, 사람이 그 줄을 따라 쳐도 아무것도 안 바뀌었다.
///
/// **대는 명령은 `init` 이 아니라 등록이다**(리뷰 5번). 여기서 자리가 서려면 주 체크아웃에 트래커가
/// **이미** 있어야 하므로([`crate::worktree::tracker_root`]), `moai -C <주 체크아웃> init` 은 0 으로
/// 끝나고 아무것도 안 바꾼다 — 그 줄을 대면 1 로 끝나던 명령이 "0 으로 끝나고 안 열린다" 로
/// 바뀔 뿐이라 기계로 읽는 쪽에는 도리어 나쁘다. 이 줄이 열릴 길은 그쪽을 등록하는 것 하나다.
///
/// 빼는 것(`moai project rm`)은 안 댄다 — 지우는 쪽은 사람이 정한다.
///
/// 글은 한눈 보기와 **한 키에서 온다**(moai-95g1) — 한때 같은 문장을 손으로 맞춰 두었고,
/// 그런 자리는 한쪽만 고쳐지는 날 두 화면이 같은 처지를 달리 부른다.
fn uninit_line(dir: &Path, tracker_at: Option<&Path>, lang: crate::i18n::Lang) -> String {
    let word = |at: &Path| shell_word(&at.display().to_string());
    match tracker_at {
        Some(main) => crate::i18n::fill(
            crate::i18n::say(lang, "overview.uninit_worktree"),
            &[("go", &format!("moai project add {}", word(main)))],
        ),
        None => crate::i18n::fill(
            crate::i18n::say(lang, "overview.uninit"),
            &[("go", &format!("moai -C {} init", word(dir)))],
        ),
    }
}

pub fn rm(ctx: &Ctx, input: &Path) -> R<Vec<String>> {
    let config = writable_config(ctx)?;
    // 빼는 길은 TUI 층의 `d` 와 하나다 — 철자 여럿으로 견주고, 설정 파일이 없으면 안 만든다.
    let projects::Removed { spelled, removed } = projects::remove(&config, input, &cwd(ctx.lang())?, ctx.lang())?;

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
            paint(style::DIM, crate::i18n::say(ctx.lang(), "project.not_registered")),
            one_line(&spelled.display().to_string())
        )]);
    }
    let dropped = crate::i18n::say(ctx.lang(), "project.dropped");
    let mut out: Vec<String> =
        removed.iter().map(|p| format!("{dropped}  {}", one_line(&p.display().to_string()))).collect();
    out.push(paint(style::DIM, &format!("  {}", crate::i18n::say(ctx.lang(), "project.list_only"))));
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
    // 글은 읽기의 알림과 한 자리에서 짓는다([`crate::view::not_a_hue`]) — 재는 자가 하나여도
    // 글이 둘이면 같은 오타에 화면과 거절문이 다른 말을 한다.
    let hue = user_config::hue_choice(word)
        .map_err(|e| Fail::coded(crate::view::not_a_hue(ctx.lang(), &e), code::BAD_INPUT))?;
    let config = writable_config(ctx)?;
    let spellings = user_config::spellings(input, &cwd(ctx.lang())?);
    let not_registered = || {
        let shown = one_line(&spellings[0].display().to_string());
        Fail::coded(
            crate::i18n::fill(
                crate::i18n::say(ctx.lang(), "project.not_registered_yet"),
                &[
                    ("path", &shown),
                    ("go", &format!("moai project add {}", shell_word(&spellings[0].display().to_string()))),
                ],
            ),
            code::NOT_FOUND,
        )
    };
    // 설정 파일이 없으면 등록한 것도 없다 — `update` 로 가면 아무것도 못 바꾸려고 설정 디렉터리를 만든다.
    if !config.exists() {
        return Err(not_registered());
    }
    let (before, changed) = user_config::update(&config, ctx.lang(), |doc| {
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
        Some(_) => crate::i18n::fill(crate::i18n::say(ctx.lang(), "project.colour"), &[("colour", &word)]),
        None => crate::i18n::fill(
            crate::i18n::say(ctx.lang(), "project.colour_auto"),
            &[("auto", user_config::AUTO), ("colour", &word)],
        ),
    };
    let mut line = format!("{said}  {shown}");
    if !changed {
        line = format!("{}  {line}", paint(style::DIM, crate::i18n::say(ctx.lang(), "project.already_so")));
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
        /// 칸별 이슈 수 — 한눈 보기와 같은 자(`report::status_unjudged(..).counts`)로 센다.
        /// 에픽·마일스톤과 미룬 것은 안 센다. 자를 따로 두면 두 화면의 수가 어긋난다.
        /// 한눈 보기가 부르는 `report::status` 는 그 위에 기한 판정만 얹은 것이라 이 수는 같다.
        counts: BTreeMap<String, usize>,
        /// 못 읽는 줄의 수. 그 줄의 이슈는 `counts` 에서 빠져 있다.
        unreadable: usize,
        /// 칸의 차례 — 설정에 적힌 대로 그린다. `counts` 는 이름순이라 쓸 수 없다.
        #[serde(skip)]
        columns: &'a [String],
    },
    /// 디렉터리는 있는데 `.moai` 가 없다 — 등록한 뒤 `moai init` 하면 보인다.
    ///
    /// **딸린 워크트리면 그 `init` 이 여기 안 선다**(moai-nppo) — 그때 `tracker_at` 이 대신 설 주
    /// 체크아웃을 든다. 낱말(`uninitialized`)은 그대로 둔다: 상태는 달라지지 않았고, 이미 나간
    /// 낱말을 바꾸면 읽던 쪽이 멀쩡한 줄을 모르는 상태로 읽는다.
    Uninitialized {
        /// **아닐 때는 키가 없다** — `Initialized` 곁의 값들과 같은 자다.
        #[serde(skip_serializing_if = "Option::is_none")]
        tracker_at: Option<&'a Path>,
    },
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
    let projects = projects::open(reg, ctx.lang());
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
        out.push(crate::i18n::say(ctx.lang(), "project.none").into());
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
                said(&r.state, ctx.lang()),
            ));
        }
    }
    if let Some(config) = &reg.path {
        let line = crate::i18n::fill(
            crate::i18n::say(ctx.lang(), "project.config_at"),
            &[("at", &one_line(&config.display().to_string()))],
        );
        out.push(paint(style::DIM, &line));
    }
    Ok(out)
}

/// 연 프로젝트 하나를 `ls` 의 낱말로 옮긴다. 못 읽는 줄을 넘기는 자는 한눈 보기와 같다
/// — 넘기지 않으면 그 줄이 쓰던 id 와의 중복이 셈에서 달라진다.
///
/// **시간대를 안 든다**(moai-yz4j). 여기서 쓰는 것은 `counts` 하나고 시간대가 닿는 셈은 기한
/// 판정뿐이라([`report::Dues`]) 그 값은 [`report::status_unjudged`] 와 [`report::status`] 에서
/// 한 글자도 다르지 않다. 들던 판은 시각을 한 줄도 안 그리는 이 명령이 tzdb 를 만져,
/// zoneinfo 없는 기계(정적 musl 판, moai-77ap)에서 없던 [`Ctx::zone_trouble`] 줄 하나를
/// stderr 에 냈다.
fn state<'a>(p: &'a projects::Project, now: &str) -> State<'a> {
    match &p.state {
        projects::State::Open { repo, load } => {
            let unreadable = load.unreadable();
            State::Initialized {
                counts: report::status_unjudged(&load.issues, &unreadable, &repo.config, now).counts,
                unreadable: load.errors.len(),
                columns: &repo.config.statuses,
            }
        }
        projects::State::Uninit(at) => State::Uninitialized { tracker_at: at.as_deref() },
        projects::State::Missing => State::Missing,
        projects::State::Unreadable(e) => State::Unreadable { error: e },
    }
}

/// 목록 한 줄의 끝 칸. 칸별 수는 한눈 보기의 보드 줄과 같은 글리프·낱말이되 한 줄에
/// 들게 사이를 좁힌다. **색이 혼자 뜻을 지지 않는다** — 글리프와 낱말이 늘 곁에 선다.
fn said(state: &State, lang: crate::i18n::Lang) -> String {
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
                let why = crate::i18n::fill(
                    crate::i18n::say(lang, "warn.unreadable_line"),
                    &[("n", &unreadable.to_string())],
                );
                cols.push(format!("{} {why}", paint(style::ERROR, "!")));
            }
            cols.join("  ")
        }
        // **워크트리면 여기가 아니라 주 체크아웃이다**(moai-nppo) — `add` 의 줄과 같은 말이다.
        State::Uninitialized { tracker_at: Some(_) } => {
            paint(style::DIM, crate::i18n::say(lang, "project.uninit_worktree"))
        }
        State::Uninitialized { .. } => paint(style::DIM, crate::i18n::say(lang, "project.uninit")),
        State::Missing => paint(style::WARN, crate::i18n::say(lang, "overview.missing")),
        // 까닭은 한 줄에 둔다 — 줄바꿈이 섞이면 다음 프로젝트의 줄과 갈리지 않는다.
        State::Unreadable { error } => {
            let why =
                crate::i18n::fill(crate::i18n::say(lang, "overview.project_unreadable"), &[("why", &one_line(error))]);
            format!("{} {why}", paint(style::ERROR, "!"))
        }
    }
}

/// 쓸 설정 파일의 자리. 모르면 **쓰기는 멈춘다** — 어디에 적었는지 모르는 등록은
/// 다음 `ls` 에서 안 보이는 등록이다. `moai read` 도 이것을 부른다 — 같은 조건에 두 명령이
/// 다른 말(고칠 길을 대는 말과 안 대는 말)을 하지 않게(moai-j038.vna).
///
/// **말이 아니라 [`Ctx`] 를 받는다**(리뷰) — `ctx.lang()` 을 인자로 주면 자리를 아는 판까지
/// 사용자 설정을 열어 파싱한다. 그 첫 부름을 늦춰 둔 것이 [`Ctx::lang`] 의 약속이고, 이 줄은
/// 자리를 **모를 때만** 서므로 거절문을 짓는 자리에서 물으면 된다.
pub(super) fn writable_config(ctx: &Ctx) -> R<PathBuf> {
    // 읽기가 같은 자리를 못 찾았을 때 대는 줄(`ConfigTrouble::NoPlace`)과 **한 뿌리에서 나온다** —
    // 한쪽은 "없다" 고 알리고 다른 쪽은 "고칠 길" 을 대므로 글은 둘이지만, 낱말이 갈리면 같은
    // 처지를 두 말로 읽는다.
    user_config::path().ok_or_else(|| Fail::coded(crate::i18n::say(ctx.lang(), "refuse.config_no_place"), code::ERROR))
}

fn cwd(lang: crate::i18n::Lang) -> R<PathBuf> {
    // OS 가 낸 글은 그대로 나르되 **무엇을 하다 났는지는 우리가 댄다**(리뷰) — errno 한 줄은
    // 주어가 없어, 준 디렉터리가 없는 것인지 설정이 없는 것인지 여기가 없어진 것인지 안 갈린다.
    // 남의 글을 `{said}` 로 감싸는 것은 `refuse.config_unparsable` 과 한 자다.
    std::env::current_dir()
        .map_err(|e| Fail::new(crate::i18n::fill(crate::i18n::say(lang, "refuse.no_cwd"), &[("said", &e.to_string())])))
}
