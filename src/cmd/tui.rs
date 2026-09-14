//! 탐색기 화면. 쓰기는 `App::write` 하나를 지나 `with_write` 에 닿는다 — 여기서
//! 따로 부르지 않는다.
//!
//! 여기는 얇다. 무엇이 어느 디렉터리에 걸리는지는 [`crate::nav`] 가 정하고,
//! 무엇을 세는지는 `report` 가 정한다. 이 파일은 잇고 그리기만 한다.

use super::{Ctx, Fail, R};
use crate::cli::TuiArgs;
use crate::model::Issue;
use crate::nav::{Entry, Index, Path, Seg};
use crate::store::Repo;
use std::io::IsTerminal;

pub fn run(ctx: &Ctx, args: TuiArgs) -> R<Vec<String>> {
    // `.moai` 밖이면 등록한 프로젝트의 층에서 시작한다(moai-ujpu). **설정이 깨진 저장소
    // 안(`Err`)은 층으로 새지 않는다** — 제 저장소의 깨진 설정이 남의 목록 뒤에 숨는다.
    let Some(repo) = Repo::find()? else {
        return outside(ctx, args);
    };
    // **재는 것이 읽는 것보다 먼저다.** 읽고 나서 재면 그 사이에 떨어진 쓰기가
    // "이미 본 것" 으로 적혀 그 뒤로 영영 바뀐 줄 모른다. 먼저 재면 최악이
    // 헛 알림 하나고, 빠진 알림보다 헛 알림이 싸다.
    let stamp = crate::tui::stamp_of(&repo);
    let load = repo.read()?;
    let index = Index::of(&load.issues);
    let path = resolve(&index, &load.issues, args.path.as_deref())?;

    // `--json` 은 화면을 켜지 않는다. 기계로 읽는 쪽과 통합 시험이 이 길로 온다.
    if ctx.json {
        // **못 읽은 줄을 삼키지 않는다.** 화면 쪽은 배너로 말하지만 이 길에는
        // 배너가 없다 — 여기서 안 알리면 목록이 조용히 짧아지고, 부른 쪽은
        // 그 이슈가 없다고 읽는다. 다른 읽기 명령과 같은 길로 간다.
        super::report_load_errors(&repo.issues_path(), &load.errors);
        let states = crate::report::group_states(&load.issues, &repo.config);
        let rows: Vec<Row> = index
            .entries(&load.issues, &path)
            .iter()
            .map(|e| Row::of(&index, &load.issues, &states, e))
            .collect();
        return super::json_line(&rows);
    }

    refuse_without_terminal()?;

    // 등록한 프로젝트가 있으면 층을 얹는다 — 뿌리에서 한 칸 더 올라가면 층이다(결정 3).
    // **남의 프로젝트는 여기서 안 읽는다**: 처음 올라갈 때 읽는다. 안에서 띄운 사람의 첫
    // 화면을 등록한 저장소 수만큼 늦출 까닭이 없다.
    let config = crate::user_config::path();
    let layer = crate::tui::layer::Layer::read(config.as_deref(), Some(&repo.root));
    let mut app = App::open(repo, load, index, path, stamp).attach_layer(layer);
    app.user = ctx.user.clone();
    // 층이 없어도 `a` 로 첫 등록을 한다 — 그때 쓸 설정 자리와 고르기 창이 처음 열 자리(moai-plvy).
    app.user_config = config;
    app.launched_at = std::env::current_dir().ok();
    screen(app)
}

/// `.moai` 밖에서 부른 탐색기 — 등록한 프로젝트의 층.
///
/// **등록한 것이 없으면 `status`·`ready` 와 같은 말로 멈춘다**(`cmd::nothing_registered`).
/// 보여줄 것이 없는데 빈 화면을 켜면 `.moai` 밖에서 부른 실수가 성공으로 읽힌다.
fn outside(ctx: &Ctx, args: TuiArgs) -> R<Vec<String>> {
    let config = crate::user_config::path();
    let reg = crate::user_config::read(config.as_deref());
    if reg.projects.is_empty() {
        return Err(super::nothing_registered(&reg));
    }
    // `--path` 는 한 프로젝트 안의 id 다. 어느 프로젝트인지 모르는 채로 받으면 id 가 겹치는
    // 두 프로젝트 중 하나를 말없이 고르게 된다.
    if args.path.is_some() {
        return Err(Fail::coded(
            "`--path` 는 프로젝트 안의 id 다 — `moai -C <dir> tui --path <id>` 로 그 프로젝트에서 연다",
            super::code::BAD_INPUT,
        ));
    }
    if ctx.json {
        let now = crate::model::now();
        let projects = crate::projects::open(&reg);
        let rows: Vec<ProjectRow> = projects
            .iter()
            .map(|p| {
                let seen = p.seen(|repo, load| {
                    let sum = crate::tui::layer::summarize(repo, load, &now);
                    Counted {
                        counts: sum.counts.into_iter().collect(),
                        picked: sum.picked.into_iter().map(|i| i.id).collect(),
                        warnings: sum.warnings,
                        unreadable: sum.unreadable,
                    }
                });
                ProjectRow {
                    title: &p.name,
                    kind: "project",
                    dir: matches!(seen, crate::projects::Seen::Ok(_)),
                    path: &p.path,
                    seen,
                }
            })
            .collect();
        // 사용자 설정의 문제는 말만 한다. **한눈 보기와 다르다** — `status`·`ready`·
        // `project ls` 의 `--json` 은 `problems` 배열에 싣지만, 이 줄들은 탐색기의 줄
        // (`Row`)과 같은 배열 하나라 실을 자리가 없다. 그래서 stderr 로 가고, 거기서도
        // `project ls` 와 같은 모양이다: `moai: ` 를 달고 한 줄로 접는다(남의 설정 파일
        // 에서 온 글이라 제어문자가 들 수 있다).
        for problem in &reg.problems {
            eprintln!("moai: {}", crate::text::one_line(problem));
        }
        return super::json_line(&rows);
    }
    refuse_without_terminal()?;
    let mut app = App::on_projects(crate::tui::layer::Layer::read(config.as_deref(), None));
    app.user = ctx.user.clone();
    app.user_config = config;
    app.launched_at = std::env::current_dir().ok();
    screen(app)
}

/// 층의 `--json` 한 줄. **탐색기 줄(`Row`)과 같은 키를 쓴다** — `title`·`kind`·`dir`·`path`.
/// 다른 것은 손잡이의 뜻 하나다: 프로젝트 줄의 `path` 는 `--path` 가 아니라 디렉터리라
/// `moai -C <path> tui --json` 으로 들어간다.
///
/// **상태 낱말은 한눈 보기(`status`·`ready`)와 같은 [`crate::projects::Seen`] 이다** —
/// 연 것이 `ok` 다. `project ls --json` 만 다르다: 그쪽은 이 기능보다 먼저 `initialized`
/// 를 내보냈고, 이미 나간 값이라 안 바꾼 것이다(`cmd::project::State` 에 그 까닭이 있다).
/// 나머지 셋(`uninitialized`·`missing`·`unreadable`)은 셋이 다 같다.
#[derive(serde::Serialize)]
struct ProjectRow<'a> {
    title: &'a str,
    kind: &'static str,
    dir: bool,
    path: &'a std::path::Path,
    #[serde(flatten)]
    seen: crate::projects::Seen<'a, Counted>,
}

/// 연 프로젝트의 셈 — 화면의 층이 쓰는 그 셈(`layer::summarize`)이다.
#[derive(serde::Serialize)]
struct Counted {
    counts: std::collections::BTreeMap<String, usize>,
    picked: Vec<String>,
    warnings: usize,
    unreadable: usize,
}

/// **TTY 가 아니면 켜지 않는다.** 파이프에 대고 대체 화면을 켜면 그 자리에서
/// 멈춰 서고, 부른 쪽은 왜 멈췄는지 알 길이 없다.
fn refuse_without_terminal() -> R<()> {
    if std::io::stdout().is_terminal() {
        return Ok(());
    }
    Err(Fail::coded(
        "터미널이 아니라 탐색기를 띄우지 않는다.\n      \
         목록만 필요하면 `moai tui --json` 이다",
        super::code::BAD_INPUT,
    ))
}

/// 화면을 켜고 끝날 때까지 돈다.
fn screen(mut app: App) -> R<Vec<String>> {
    // 터미널 복구는 ratatui 에 맡긴다 — `try_init` 이 raw mode·대체 화면을 켜고
    // **되돌리는 패닉 훅까지** 건다. 손으로 짜면 어느 이른 return 하나가
    // 사용자 셸을 망가뜨린다.
    //
    // **`ratatui::run` 이 아니라 `try_init` 이다.** `run` 은 안에서 `init()` 을
    // 부르고 그것은 `.expect()` 다 — 통제 터미널이 없거나 크기를 못 얻으면
    // 101 번 패닉이 나고, `--json` 으로 부른 쪽은 약속된 오류 객체 대신
    // 역추적 문구를 받는다. 여기서 받아 `Fail` 로 바꾼다.
    let mut term = ratatui::try_init().map_err(|e| Fail::new(format!("터미널을 열지 못했다: {e}")))?;
    let out = loop_until_quit(&mut term, &mut app);
    ratatui::restore();
    out.map_err(|e| Fail::new(e.to_string()))?;
    Ok(Vec::new())
}

/// `--path <id>` 를 경로로 옮긴다.
///
/// **id 하나만 받는다.** 마디를 슬래시로 죽 적게 하면 사람도 에이전트도 조상을
/// 다 알아야 하는데, 그건 `nav` 가 이미 아는 것이다 — `home_of` 가 그 이슈가
/// 걸린 자리를 알므로 거기에 제 마디만 얹으면 된다.
///
/// 디렉터리(마일스톤·에픽·자식 있는 이슈)면 그 **안**을, 아니면 그것이 **든**
/// 디렉터리를 연다. 파일을 열라고 하면 그 파일이 있는 폴더를 여는 것과 같다.
///
/// **비었는지로 묻지 않는다** — 그것은 `nav::Index::is_dir` 이 이미 아는
/// 것이고, 비었다고 부모를 대신 열면 빈 에픽을 물었을 때 그 형제들이 답으로
/// 나와 훑는 쪽이 제자리를 돈다.
fn resolve(index: &Index, issues: &[Issue], want: Option<&str>) -> R<Path> {
    let Some(want) = want else { return Ok(Path::new()) };
    // 바구니는 제 줄이 없어 id 로 못 부른다. 대신 **없는 바구니는 거절한다** —
    // 없는 자리에 세워 두면 빈 목록이 나오고, 사람은 자료가 사라진 줄 안다.
    if let Some(seg) = match want {
        NO_MILESTONE => Some(Seg::Milestone(None)),
        LOST => Some(Seg::Lost),
        _ => None,
    } {
        let path = vec![seg.clone()];
        if index.entries(issues, &path).is_empty() {
            return Err(Fail::not_found(want));
        }
        return Ok(path);
    }
    let at = index.find(want).ok_or_else(|| Fail::not_found(want))?;
    let mut path = index.home_of(at).clone();
    if index.is_dir(issues, at) {
        path.push(index.seg_of(issues, at));
    }
    Ok(path)
}

/// `--json` 한 줄의 원소. 파일의 키 순서와 같은 뜻으로 앞에서부터 읽힌다.
#[derive(serde::Serialize)]
struct Row {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    title: String,
    /// `milestone`·`epic`·`issue`·`bucket`.
    kind: &'static str,
    /// 들어갈 수 있는가.
    dir: bool,
    /// **그대로 `--path` 에 도로 넣는 손잡이.** 바구니는 제 줄이 없어 id 가
    /// 없는데, `dir: true` 만 내고 손잡이를 안 주면 훑는 쪽은 들어갈 수 있다는
    /// 말만 듣고 들어갈 방법이 없다 — 제목에서 낱말을 되짚어 내야 했다.
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    /// 묶음이면 멤버에서 읽은 칸. `show --json` 과 같은 키, 같은 뜻이다.
    #[serde(skip_serializing_if = "Option::is_none")]
    derived_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u8>,
}

impl Row {
    fn of(index: &Index, issues: &[Issue], states: &std::collections::BTreeMap<&str, &str>, e: &Entry) -> Row {
        let dir = matches!(e, Entry::Dir { .. });
        let title = index.label(issues, e);
        match e.at() {
            Some(at) => Row {
                id: Some(issues[at].id.clone()),
                title,
                kind: issues[at].kind.as_str(),
                dir,
                path: issues[at].id.clone(),
                status: Some(issues[at].status.as_str().to_string()),
                // 묶음만 읽은 칸을 받는다 — `report::column` 과 같은 자다.
                derived_status: crate::report::is_group(&issues[at])
                    .then(|| states.get(issues[at].id.as_str()).map(|s| s.to_string()))
                    .flatten(),
                priority: Some(issues[at].priority()),
            },
            None => Row {
                id: None,
                title,
                kind: "bucket",
                dir,
                path: match e {
                    Entry::Dir { seg: Seg::Lost, .. } => LOST.into(),
                    _ => NO_MILESTONE.into(),
                },
                status: None,
                derived_status: None,
                priority: None,
            },
        }
    }
}

/// 바구니를 `--path` 로 부르는 이름. 바구니는 제 줄이 없어 id 가 없다.
const NO_MILESTONE: &str = "없음";
const LOST: &str = "길잃음";

// ── 화면 ──────────────────────────────────────────────────────────────

use crate::tui::App;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

/// 파일이 바뀌었는지 보러 깨는 걸음. 바뀌었으면 저절로 다시 읽는다(`App::follow`) —
/// 커서는 줄의 정체를 따라가므로 읽던 자리를 잃지 않는다.
const TICK: std::time::Duration = std::time::Duration::from_millis(700);

/// 스레드가 다시 읽기를 짓는 동안 받으러 깨는 걸음. 파일을 보는 걸음(`TICK`)에
/// 맡기면 다 지어 놓고도 최대 700ms 를 화면이 옛 것으로 서 있다. 받는 것은 채널을
/// 한 번 들여다보는 일이라 싸다.
const LOAD_POLL: std::time::Duration = std::time::Duration::from_millis(30);

/// 도는 글리프가 한 칸 가는 **가장 빠른** 걸음. ora 가 80ms 로 돌린다. 그보다
/// 느긋해도 회전으로 읽히는데, **파일을 보는 걸음(700ms)에 얹으면 한 바퀴가
/// 7초라 도는 것이 아니라 글자가 이따금 바뀌는 것으로 보인다** — 그래서 걸음을
/// 따로 둔다. 반대로 빠르게 한다고 파일을 그만큼 자주 보게 하지도 않는다.
/// `stat` 은 디스크를 만지고, 글리프 한 칸은 바뀐 칸만 내보내면 끝이다.
const SPIN_TICK: std::time::Duration = std::time::Duration::from_millis(120);

/// 걸음이 아무리 늘어져도 여기까지. 넘기면 도는 것이 멈춘 것으로 보이고,
/// 멈춘 스피너는 "일이 멈췄다" 는 거짓말을 한다.
const SPIN_SLOWEST: std::time::Duration = std::time::Duration::from_millis(1000);

/// 그리는 데 쓸 몫. **한 프레임이 비싼 저장소에서는 걸음이 스스로 늘어난다** —
/// 재 보니(release, `TestBackend` 120x45, 에픽 하나에 들어간 채) 이슈 1만 개에서
/// 한 프레임이 22ms 고 45개는 1.2ms 다. 그걸 120ms 마다 그리면 가만히 둔
/// 탐색기가 CPU 18% 를 먹는다. 그린 시간의 이만큼을 쉬게 하면 느려지는 것은
/// 회전뿐이고 비용은 1/n 로 묶인다. 느린 ssh 에서도 같은 자가 듣는다 — 그쪽은
/// 내보내는 데 걸린 시간이 곧 프레임 값이다.
const SPIN_BUDGET: u32 = 10;

/// 이번 프레임을 그린 값으로 다음 걸음을 정한다. **터미널 없이 시험된다** —
/// 루프는 TTY 가 있어야 돌지만 이 셈은 없어도 돈다.
fn spin_step(drew: std::time::Duration) -> std::time::Duration {
    (drew * SPIN_BUDGET).clamp(SPIN_TICK, SPIN_SLOWEST)
}

fn loop_until_quit(term: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    // 층에서 시작하면 읽기가 이미 돌 수 있다 — 첫 걸음부터 빠르게 받는다.
    let mut stale_due = std::time::Instant::now() + if app.loading() { LOAD_POLL } else { TICK };
    let mut spin_due = std::time::Instant::now() + SPIN_TICK;
    while !app.quit {
        let began = std::time::Instant::now();
        term.draw(|f| crate::tui::draw::screen(f, app))?;
        let step = spin_step(began.elapsed());
        // **돌 것이 없으면 빠른 걸음으로 깨지 않는다.** 다 끝난 판을 열어 둔
        // 채로 둔 사람의 CPU 를 초당 여덟 번 깨울 까닭이 없다.
        let spinning = app.spinning();
        // **시간으로 센다, 한가함으로 세지 않는다.** 이벤트가 오는 동안에만
        // 안 보면 — 키를 누르고 있거나 창을 끄는 내내 — 바뀐 것을 못 본다.
        // 하필 그때가 쓰는 사람이 화면을 보고 있는 때다.
        let wake = if spinning { stale_due.min(spin_due) } else { stale_due };
        let wait = wake.saturating_duration_since(std::time::Instant::now());
        if event::poll(wait)? {
            // **누를 때만 받는다.** crossterm 은 kitty 프로토콜 터미널에서 뗄 때도
            // 보내므로, 거르지 않으면 키 하나가 두 번 먹는다.
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                app.key(k);
            }
        }
        let now = std::time::Instant::now();
        // 키가 읽기를 띄웠으면(층으로 올라가기 따위) 느린 걸음까지 기다리지 않고 받으러 깬다.
        if app.loading() {
            stale_due = stale_due.min(now + LOAD_POLL);
        }
        if now >= stale_due {
            app.follow();
            stale_due = now + if app.loading() || app.reaping() { LOAD_POLL } else { TICK };
        }
        // **걸음은 시계가 올린다, 그린 횟수가 올리지 않는다.** 그릴 때마다
        // 올리면 키를 누르는 내내 타이핑 속도로 돌고, 가만히 두면 파일을 보는
        // 걸음으로 느려진다. 돌 것이 없는 동안에도 **때는 미뤄 둔다** — 안 그러면
        // 다시 생긴 순간 지나간 때가 걸려 한 칸이 곧바로 튄다.
        if now >= spin_due {
            if spinning {
                app.spin = app.spin.wrapping_add(1);
            }
            spin_due = now + step;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 걸음은 그린 값의 [`SPIN_BUDGET`] 배다 — **싼 프레임에서 느려지지 않고,
    /// 비싼 프레임에서 CPU 를 독차지하지 않는다.** 재 본 값으로 두 끝을 박아
    /// 둔다: 이슈 45개 1.2ms, 1만 개 22ms.
    #[test]
    fn the_step_buys_the_drawing_a_fixed_share_of_the_time() {
        assert_eq!(spin_step(Duration::from_micros(1158)), SPIN_TICK, "싼 프레임이 느려졌다");
        assert_eq!(spin_step(Duration::from_millis(22)), Duration::from_millis(220));
        // 그리는 몫은 어디서나 1/SPIN_BUDGET 아래다 — 두 끝 사이에서는 정확히 그 값이다.
        for ms in [13, 22, 50, 99] {
            let drew = Duration::from_millis(ms);
            assert_eq!(spin_step(drew), drew * SPIN_BUDGET, "{ms}ms 에서 몫이 어긋났다");
        }
    }

    /// 아무리 비싼 프레임에서도 **멈춘 것처럼 보이지는 않는다.** 멈춘 스피너는
    /// 일이 멈췄다는 거짓말이다.
    #[test]
    fn even_a_hopeless_frame_keeps_the_glyph_turning() {
        assert_eq!(spin_step(Duration::from_secs(30)), SPIN_SLOWEST);
        assert!(spin_step(Duration::from_secs(30)) <= Duration::from_secs(1));
    }
}
