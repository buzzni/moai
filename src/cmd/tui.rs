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
    let Some(repo) = Repo::find(|| ctx.lang())? else {
        return outside(ctx, args);
    };
    // **재는 것이 읽는 것보다 먼저다.** 읽고 나서 재면 그 사이에 떨어진 쓰기가
    // "이미 본 것" 으로 적혀 그 뒤로 영영 바뀐 줄 모른다. 먼저 재면 최악이
    // 헛 알림 하나고, 빠진 알림보다 헛 알림이 싸다.
    let stamp = crate::tui::stamp_of(&repo);
    // 자리 판정이 보는 옆 워크트리도 **읽기 전에** 잰다 — 다시 읽기(`tui::prepare`)가 지켜보는 목록과
    // 같은 모양이어야 첫 다시 읽기가 안 바뀐 커밋 표를 다시 짓지 않고, git 을 못 불러도 옆 워크트리를
    // 치운 것을 안다. 화면을 안 켜는 `--json` 은 지켜볼 것이 없다.
    let places = if ctx.json { Vec::new() } else { crate::worktree::place_marks(repo.here()) };
    // 탐색기는 옆 워크트리를 겹친 채로 연다(`App::worktree`). `--json` 은 겹치지 않는다 —
    // 기계로 읽는 쪽의 출력 모양은 `status`·`ready`·`show` 처럼 `--worktree` 없이 그대로다.
    // 찾지 못한 까닭(`unfound`)은 배너에 안 올린다 — 시키지 않은 겹쳐 보기다(`Gathered::unfound`).
    let crate::worktree::Gathered { load, origin, trouble, mut watched, swept, sides, mine, .. } =
        crate::worktree::gather(&repo, !ctx.json)?;
    crate::tui::watch(&mut watched, places);
    // **한 걸음으로 잰다**(moai-fbdg) — 색인과 묶음 칸을 한 지도에서 짓는다. 따로 부르면 첫 화면 앞에서
    // 소속 지도를 두 번 잰다(moai-xemz 리뷰).
    // **노트는 여기서 안 읽는다**(리뷰 moai-wcy8.rbj) — `/` 가 노트를 처음 볼 때 읽는다(`tui::Ground::read_notes`).
    // 여기서 읽던 판은 첫 화면 앞에서, 터미널이 없어 거절될 판에서도 저널 전체를 풀었다.
    let (index, ground) = crate::tui::measure(&load.issues, &repo.config);
    let path = resolve(&index, &load.issues, args.path.as_deref(), ctx.lang())?;

    // `--json` 은 화면을 켜지 않는다. 기계로 읽는 쪽과 통합 시험이 이 길로 온다.
    if ctx.json {
        // **못 읽은 줄을 삼키지 않는다.** 화면 쪽은 배너로 말하지만 이 길에는
        // 배너가 없다 — 여기서 안 알리면 목록이 조용히 짧아지고, 부른 쪽은
        // 그 이슈가 없다고 읽는다. 다른 읽기 명령과 같은 길로 간다.
        super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);
        let states = ground.columns();
        let rows: Vec<Row> = index
            .entries(&load.issues, &path)
            .iter()
            .map(|e| Row::of(&index, &load.issues, &states, e, ctx.lang()))
            .collect();
        return super::json_line(&rows);
    }

    refuse_without_terminal(ctx.lang())?;

    // 등록한 프로젝트가 있으면 층을 얹는다 — 헤더의 `0` 이 그리로 간다(moai-i784·moai-o133).
    // **남의 프로젝트는 여기서 안 읽는다**: 처음 올라갈 때 읽는다. 안에서 띄운 사람의 첫
    // 화면을 등록한 저장소 수만큼 늦출 까닭이 없다.
    let config = crate::user_config::path();
    // 표식은 **읽기 전에** 잰다 — 뒤에 재면 읽고 첫 걸음 사이에 옆이 쓴 것을 놓친다(`App::config_stamp`).
    let config_stamp = config.as_deref().map(crate::store::stamp);
    // 설정은 **한 번 읽어** `tui::layer` 와 보기와 새 판 묻기가 나눠 쓴다(moai-u8cs, moai-d74q).
    let reg = crate::user_config::read(config.as_deref());
    // **띄운 자리는 세션이 선 체크아웃이다**(`repo.here()`, 리뷰 moai-71ht 셋째 판) — 트래커는 루트로
    // 옮겨 가지만 띄운 곳은 이 워크트리다. 트래커의 자리로 적던 판은 층으로 올라갔다 그 줄로 다시
    // 들어오는 걸음에서 `Repo::open(<루트>)` 을 열어 `here()` 가 루트로 뒤집혔고, 그때부터 커밋 표가
    // 이 가지의 커밋을 잃고 집기 표식도 안 적혔다.
    // 층의 말은 여기서 준다(moai-9it4) — 층이 세우며 짓는 글이 그 말로 선다. 얹는 문
    // (`App::with_layer`)이 화면의 말과 다시 맞춘다(moai-ra67).
    let layer = crate::tui::layer::Layer::of(&reg, Some(repo.here()), ctx.lang());
    // 옆 워크트리의 문제는 **펴서** 싣는다(moai-dpbi). 다시 읽기(`tui::prepare`)도 제 말을 들고
    // 가므로(moai-9it4) 여는 화면과 같은 자로 편다 — 둘이 갈리면 배너가 걸음마다 말을 바꾼다.
    let trouble = crate::tui::said_trouble(&trouble, ctx.lang());
    let mut app = App::open(repo, load, index, ground, path, stamp);
    // **탐색기도 고른 말로 선다**(moai-ra67) — 명령 층에서 한 번 푼 것을 화면에 놓는다.
    // **겹치기 전에 놓는다**(moai-9it4) — `overlaid` 가 자리 판정의 글(`tui::placed` 의
    // `view::unread_worktree`)을 화면의 말로 편다. 뒤에 놓던 판은 그 한 줄만 도구의 기본 말로
    // 서서, 바로 위에서 고른 말로 편 `trouble` 과 한 배너에 두 말이 섞였다.
    app.site.lang = ctx.lang();
    let mut app = app.overlaid(origin, trouble, watched, swept, &sides, &mine);
    // **판 것은 여기서 버린다**(moai-kos1) — 옆 스냅샷의 줄은 이미 `load` 에 겹쳐 들어왔고,
    // 쓰는 자리는 바로 위 하나다. 안 버리면 탐색기가 도는 내내 워크트리마다 한 벌씩 그대로
    // 남아, 겹쳐 본 저장소의 줄을 두 번 들고 산다(다시 읽기는 `tui::prepare` 가 제 것을 판다).
    // 겹치기 전에 잰 바닥(moai-mafv)도 같은 자리에서 버린다 — 다시 읽기는 제 것을 잰다.
    drop(sides);
    drop(mine);
    app.user = ctx.user.clone();
    // 누군지는 **띄울 때** 푼다(moai-z9pc) — 못 풀면 [NEW] 가 안 설 뿐이고, 탐색기는 그대로 뜬다. 헤더와
    // 같은 자(`App::whoami`)라 `--user` 도 같이 먹는다. 프로젝트를 옮기면 그 뿌리에서 다시 푼다.
    let root = app.here().unwrap_or_else(|| ".".into());
    app.site.me = app.whoami(&root);
    // 층이 없어도 `a` 로 첫 등록을 한다 — 그때 쓸 설정 자리와 고르기 창이 처음 열 자리(moai-plvy).
    app.user_config = config;
    app.config_stamp = config_stamp;
    // **띄울 때 진 읽기도 걸음이 갚는다**(moai-po6v) — 표식은 읽기 전에 쟀으니 진 뒤에도 파일의 것과
    // 같아, 갈래를 안 넘기면 그 한 번의 실패가 세션 내내 남는다(`App::config_tried`).
    app.config_tried.saw(reg.trouble);
    // 적어 둔 보기(칸 숨김·정렬·열)를 입힌다(moai-2bzp). 못 읽은 설정의 까닭은 보기가 아니라 층이 댄다 —
    // 얹는 쪽(`App::attach_layer`)이 배너에 달고 `look_problems` 에는 그 까닭이 없다(moai-5jsn). 그래서 둘의
    // 차례에 걸린 것은 없다(moai-gmdu 에픽 리뷰). 한때는 `with_layer` 가 첫 화면의 커서를 `..` 너머로 밀어
    // 차례가 걸렸는데, 뿌리의 `..` 을 걷으면서(moai-i784) 그 밀기는 없어졌다(moai-2kyl 단계 리뷰).
    // **말은 화면이 이미 든 것이다**(`App::site.lang`) — 여기서 `ctx.lang()` 을 다시 물으면 밖에서 띄운
    // 길(`outside`)이 같은 설정을 한 번 더 판다(`super::lang_of` 가 적어 둔 그 까닭이다).
    app.adopt_look(&reg.look, crate::view::look_problems(&reg, app.site.lang));
    // 읽음은 이 저장소의 제 파일에 산다(moai-omx7) — 설정에서 오는 것은 겹쳐 볼 옛 `[read]` 뿐이다.
    app.legacy_read = reg.read;
    app.load_read();
    let mut app = app.attach_layer(layer);
    app.launched_at = std::env::current_dir().ok();
    app.editor = editor();
    let config = app.user_config.clone();
    // 설정이 무엇이라 했는지는 **위에서 한 번 읽은 것**을 그대로 든다(moai-d74q). `reg.read` 가
    // 옮겨 간 뒤에도 이 필드는 그대로 읽힌다 — 옮긴 것은 그 필드 하나다.
    ask_latest(ctx, &mut app, config.as_deref(), reg.update_check);
    screen(app)
}

/// 새 판을 묻는 실을 띄운다 — **문을 지날 때만**(moai-3gia, 사용자 결정 2026-09-21).
///
/// 문은 넷이다([`crate::latest::gate`]) — 환경(`MOAI_NO_UPDATE_CHECK`), 설정(`[update] check`),
/// `--json`, 그리고 사람이 보는 화면인가. 탐색기는 화면을 켜는 길이라 뒤의 둘은 거의 늘 참이지만,
/// **묻는 쪽이 답을 내야** 한다는 계약은 여기서도 지킨다 — `gate` 가 인자를 갈라 받는 까닭이 그것이다.
///
/// **답을 둘 자리가 없으면 안 묻는다.** 설정 파일의 자리를 모르는 기계(`HOME` 도 `XDG_CONFIG_HOME`
/// 도 없다)에서 묻기 시작하면 창을 닫을 자리가 없어 **부를 때마다** 바깥을 두드리는데, 그것이 이
/// 기능이 피하려던 바로 그 일이다.
fn ask_latest(ctx: &Ctx, app: &mut crate::tui::App, config: Option<&std::path::Path>, says: Option<bool>) {
    let Some(dir) = asking_from(config, says, ctx.json, crate::latest::on_screen(), |k| std::env::var_os(k)) else {
        return;
    };
    app.ask_latest(dir, crate::latest::url_from(|k| std::env::var_os(k)));
}

/// 물을 것인가, 물으면 답을 어디에 둘 것인가 — **판단만 한다**(리뷰).
///
/// 문을 재는 자리를 실을 띄우는 자리에서 갈라 둔 것은 시험이 그것을 잴 수 있게 하려는 것이다.
/// 붙여 두면 `is_some()` 을 `is_none()` 으로 뒤집어도 온 시험이 푸르게 서 있었다 — 문이 반대로
/// 서면 `--json` 이 매번 바깥을 두드리는데, 그것이 이 기능이 피하려던 바로 그 일이다.
fn asking_from(
    config: Option<&std::path::Path>,
    says: Option<bool>,
    json: bool,
    on_screen: bool,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<std::path::PathBuf> {
    let dir = config.and_then(|p| p.parent())?;
    crate::latest::gate(env, says, json, on_screen).is_none().then(|| dir.to_path_buf())
}

/// `.moai` 밖에서 부른 탐색기 — 등록한 프로젝트의 층.
///
/// **등록한 것이 없어도 화면은 빈 층을 연다**(moai-r8kl, 사용자와 정함). 층이 `SPC p a` 를
/// 대므로 빈 화면이 성공으로 읽히지 않고, 그 자리에서 첫 등록을 한다 — 전에는 그러려면 어느
/// `.moai` 안에서 띄워야 했다.
///
/// **`--json` 은 한눈 보기(`status --json`)와 같은 객체다**(moai-yxae, 2026-09-14 사람의 결정) —
/// `{"projects":[…],"problems":[…],"config":…}`. 맨 배열이던 때는 사용자 설정의 문제를 실을
/// 자리가 없어 stderr 로만 냈고, 등록한 것이 없으면 오류로 멈춰 stdout 에는 JSON 이 없었다.
/// 이제 등록한 것이 없어도, 설정이 깨져도 0 이다 — 빈 `projects` 가 "다 비었다" 로 잘못 읽히지
/// 않는 것은 곁의 `problems` 가 까닭을 대기 때문이다(`status` 와 같은 자리, moai-ynsb).
fn outside(ctx: &Ctx, args: TuiArgs) -> R<Vec<String>> {
    let config = crate::user_config::path();
    // 표식은 **읽기 전에** 잰다(`App::config_stamp`) — `--json` 에는 안 쓰지만 stat 하나라 가른다.
    let config_stamp = config.as_deref().map(crate::store::stamp);
    // 설정은 **한 번 읽어** `--json`·층·보기가 나눠 쓴다(moai-u8cs).
    let reg = crate::user_config::read(config.as_deref());
    // `--path` 는 한 프로젝트 안의 id 다. 어느 프로젝트인지 모르는 채로 받으면 id 가 겹치는
    // 두 프로젝트 중 하나를 말없이 고르게 된다.
    if args.path.is_some() {
        return Err(Fail::coded(
            crate::i18n::say(super::lang_of(&reg), "refuse.tui_path_outside"),
            super::code::BAD_INPUT,
        ));
    }
    if ctx.json {
        let now = crate::model::now();
        let projects = crate::projects::open(&reg, super::lang_of(&reg));
        let rows: Vec<ProjectRow> = projects
            .iter()
            .map(|p| {
                let seen = p.seen(|repo, load| {
                    let sum = crate::tui::layer::summarize(repo, load, &now, &p.dug());
                    Counted {
                        counts: sum.counts.into_iter().collect(),
                        picked: sum.picked.into_iter().map(|i| i.id).collect(),
                        // **여기서 달을 입힌다**(moai-fgjj) — 셈은 시간대에 안 닿고, 이 길은 낼
                        // 것을 그 자리에서 다 내는 표면이다. 어느 시계로 셀지는 그대로 `Ctx::zone`
                        // 이다(탐색기의 `[tui] timezone` 과 갈리는 것은 idea moai-dux7 이 든 자리다).
                        warnings: sum.warnings.count(&now, ctx.zone()),
                        notices: sum.notices,
                        stranded: sum.stranded,
                        unreadable_worktrees: sum.blind,
                        broken_worktrees: sum.unread,
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
        // 사용자 설정의 문제는 `problems` 에 싣는다 — 한눈 보기·`project ls` 와 같은 자리다.
        // 제어문자는 serde 가 이스케이프한다. **거쳐 가는 문은 `view::settings_problems` 하나다**(리뷰) —
        // 화면 말의 탈은 자료로 따로 서므로(moai-dpbi) `reg.problems` 만 실으면 `lang` 오타를 잃는다.
        // 말은 이미 읽은 설정에서 고른다([`super::lang_of`]) — `ctx.lang()` 은 같은 파일을 또 판다.
        let problems = crate::view::settings_problems(&reg, super::lang_of(&reg));
        return super::json_line(&Layered { projects: rows, problems: &problems, config: reg.path.as_deref() });
    }
    refuse_without_terminal(ctx.lang())?;
    let layer = crate::tui::layer::Layer::of(&reg, None, ctx.lang());
    // **시간대는 여기서 안 준다**(moai-fgjj) — 이 줄이 첫 쓸기를 띄우지만(`App::on_projects` →
    // `Layer::launch`) 그 쓸기가 세는 것에 기한 판정이 안 접혀 오므로, 아래 `adopt_look` 이 시간대를
    // 정하고 나면 첫 화면의 `+N` 이 그 시간대로 선다. 한때 여기서 한 벌 베껴 넘겼다(리뷰 moai-pmhv.x3r 4번).
    let mut app = App::on_projects(layer);
    app.site.lang = ctx.lang();
    app.user = ctx.user.clone();
    // 밖에서 띄워도 누군지는 같은 자로 푼다(moai-z9pc.9av). 층에는 저장소가 없으니 지금 디렉터리에서
    // 묻는다 — 전역 git 설정이면 그것으로 선다. 층에서 프로젝트로 들어가면 그 뿌리에서 다시 푼다
    // (`App::enter_project`) — 프로젝트에만 적힌 git 설정이어도 [NEW] 가 선다.
    app.site.me = app.whoami(&std::env::current_dir().unwrap_or_else(|_| ".".into()));
    app.user_config = config;
    app.config_stamp = config_stamp;
    // **띄울 때 진 읽기도 걸음이 갚는다**(moai-po6v) — 표식은 읽기 전에 쟀으니 진 뒤에도 파일의 것과
    // 같아, 갈래를 안 넘기면 그 한 번의 실패가 세션 내내 남는다(`App::config_tried`).
    app.config_tried.saw(reg.trouble);
    // 적어 둔 보기(칸 숨김·정렬·열)를 입힌다(moai-2bzp).
    app.adopt_look(&reg.look, crate::view::look_problems(&reg, app.site.lang));
    app.legacy_read = reg.read;
    app.load_read();
    app.launched_at = std::env::current_dir().ok();
    app.editor = editor();
    let config = app.user_config.clone();
    ask_latest(ctx, &mut app, config.as_deref(), reg.update_check);
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

/// `.moai` 밖 `tui --json` 전체. **키가 한눈 보기의 [`crate::projects::Overview`] 와 같다** —
/// 다른 것은 `projects` 원소가 탐색기 줄 모양([`ProjectRow`])이라는 것 하나다.
#[derive(serde::Serialize)]
struct Layered<'a> {
    projects: Vec<ProjectRow<'a>>,
    problems: &'a [String],
    config: Option<&'a std::path::Path>,
}

/// 연 프로젝트의 셈 — 화면의 층이 쓰는 그 셈(`layer::summarize`)이다.
#[derive(serde::Serialize)]
struct Counted {
    counts: std::collections::BTreeMap<String, usize>,
    picked: Vec<String>,
    warnings: usize,
    /// 설치가 어긋난 것과 쌓인 것을 대는 알림의 수(moai-prdh) — 화면의 층이 `+N` 으로 대는 그
    /// 수고, 안쪽 `moai status` 가 세우는 알림과 같은 자다(`layer::Summary::notices`).
    /// **경고가 아니다** — 위의 `warnings` 와 겹치지 않고 종료 코드와도 상관없다.
    /// 없으면 키를 안 단다 — 곁의 셋과 같은 까닭이다.
    #[serde(skip_serializing_if = "is_zero")]
    notices: usize,
    /// 그중 집었는데 일하는 워크트리가 없는 줄(moai-p3bs) — 화면의 층이 낱말로 대는 그 수다.
    /// 없으면 키를 안 단다: 늘 `0` 을 달면 옛 판과 견주는 쪽이 새 뜻을 얻은 줄 모른다.
    #[serde(skip_serializing_if = "is_zero")]
    stranded: usize,
    /// 스냅샷을 못 읽어 **자리 판정을 가린** 워크트리의 수(`report::blinding`) — 있으면 위의 수는
    /// "센 결과 0" 이 아니라 "못 셌다" 다. **못 읽은 것 전부가 아니다**(moai-rgz9): 이름이 집은 줄을
    /// 가리키는 워크트리는 못 읽어도 판정을 안 가리니 안 든다. 키 이름은 이미 나간 값이라 그대로
    /// 둔다 — 안쪽 `status --json` 의 같은 키와 같은 뜻이다. 사람 화면의 층은 깨진 스냅샷 자체를
    /// 따로 대므로(`layer::Summary::unread`) 그 둘이 여기서 갈린다.
    #[serde(skip_serializing_if = "is_zero")]
    unreadable_worktrees: usize,
    /// 스냅샷을 못 읽은 워크트리 **전부**의 수(moai-zah3) — 화면의 층이 `layer::Summary::unread` 로
    /// 대는 그 수다. `status --json` 의 `broken_worktrees` 와 같은 뜻이고, 없으면 키를 안 단다.
    #[serde(skip_serializing_if = "is_zero")]
    broken_worktrees: usize,
    unreadable: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// **TTY 가 아니면 켜지 않는다.** 파이프에 대고 대체 화면을 켜면 그 자리에서
/// 멈춰 서고, 부른 쪽은 왜 멈췄는지 알 길이 없다.
fn refuse_without_terminal(lang: crate::i18n::Lang) -> R<()> {
    if std::io::stdout().is_terminal() {
        return Ok(());
    }
    Err(Fail::coded(
        format!(
            "{}\n      {}",
            crate::i18n::say(lang, "refuse.tui_needs_a_terminal"),
            crate::i18n::say(lang, "refuse.tui_json_instead"),
        ),
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
    let lang = app.site.lang;
    let mut term = ratatui::try_init().map_err(|e| {
        Fail::new(crate::i18n::fill(crate::i18n::say(lang, "tui.no_terminal"), &[("why", &e.to_string())]))
    })?;
    // **붙여넣기를 글로 받는다**(moai-od9q). 안 켜면 붙인 글이 키 하나하나로 와서, 탭은 폼의
    // 칸을 옮기고 줄바꿈은 Enter 로 검색을 걸거나 제목을 떠나며, 탐색 중에 붙인 `q` 는 끝낸다.
    // 끄는 길은 둘이다 — 여기 아래의 정상 끝과 **패닉 훅.** ratatui 의 훅은 raw mode 와 대체
    // 화면만 걷으므로, 안 걸면 패닉 뒤 사용자 셸에 붙인 글이 `200~…201~` 에 싸여 들어간다.
    // 훅은 `try_init` **뒤에** 건다 — 그래야 끄기가 ratatui 의 복구를 감싸 먼저 돈다. 켜기를
    // 못 해도 멈추지 않는다: 붙여넣기가 옛날처럼 키로 올 뿐이다.
    modes_off_on_panic();
    // **마우스도 같은 두 길로 끈다**(moai-irrj.9xq) — 안 끄고 나가면 셸에서 마우스를 움직이거나 누를
    // 때마다 `^[[<0;12;5M` 같은 글이 프롬프트에 찍힌다. 놓아 둔 사람(`SPC o m`)에게는 켜지 않는다.
    modes_on(app.wants_mouse());
    // **패닉으로 끝나도 여기서 걷는다**(moai-46xe). 다시 읽기 스레드의 패닉은 루프가 `resume_unwind`
    // 로 되던지는데 그것은 훅을 안 지난다 — 훅이 이미 걷은 터미널을 편집기에서 돌아오며 다시
    // 올렸다면 raw·대체 화면인 채로 셸에 남는다. 걷은 뒤 패닉 글을 **한 번 더** 낸다: 훅이 낸
    // 글은 그 뒤에도 루프가 그린 한 프레임에 덮였을 수 있다.
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop_until_quit(&mut term, &mut app)));
    // 루프는 사건마다 터미널의 잡기를 `wants_mouse` 에 맞춰 두므로, 제대로 끝난 자리의 값이 곧 터미널의 것이다.
    // 오류·패닉으로 끝난 길에서는 어긋날 수 있는데, 그때 드는 것은 헛 왕복 한 번이지 새는 글이 아니다.
    modes_off_draining(app.wants_mouse(), app.rolled_at);
    ratatui::restore();
    // 오류·패닉으로 끝났으면 폼에 남은 글부터 건진다 — 터미널을 걷은 **뒤라** 그 말이 셸에 보이고,
    // 패닉을 되던지기 **전이라** 남길 기회가 있다(moai-y3r7).
    if !matches!(out, Ok(Ok(()))) {
        keep_unsaved(&app);
    }
    let out = out.unwrap_or_else(|payload| {
        let why = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or(crate::i18n::say(lang, "tui.panic_unknown_why"));
        eprintln!("moai: {}", crate::i18n::fill(crate::i18n::say(lang, "tui.panicked"), &[("why", why)]));
        std::panic::resume_unwind(payload)
    });
    out.map_err(|e| Fail::new(e.to_string()))?;
    // 새 판을 올리는 줄은 터미널을 걷은 **뒤에** 낸다 — 배너에서는 80칸에 잘렸다(moai-8rmw.665).
    if let Some(note) = app.upgrade_note() {
        eprintln!("moai: {note}");
    }
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
fn resolve(index: &Index, issues: &[Issue], want: Option<&str>, lang: crate::i18n::Lang) -> R<Path> {
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
            return Err(Fail::not_found(want, lang));
        }
        return Ok(path);
    }
    let at = index.find(want).ok_or_else(|| Fail::not_found(want, lang))?;
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
    fn of(
        index: &Index,
        issues: &[Issue],
        states: &std::collections::BTreeMap<crate::report::GroupKey<'_>, &str>,
        e: &Entry,
        lang: crate::i18n::Lang,
    ) -> Row {
        let dir = matches!(e, Entry::Dir { .. });
        let title = index.label(issues, e, lang);
        match e.at() {
            Some(at) => Row {
                id: Some(issues[at].id.clone()),
                title,
                kind: issues[at].kind.as_str(),
                dir,
                path: issues[at].id.clone(),
                status: Some(issues[at].status.as_str().to_string()),
                // 묶음만 읽은 칸을 받고 가려진 줄은 안 입는다 — `report::stands_on` 과 같은
                // 자다(moai-7iyc.5fz). 판정을 여기 베끼면 `show --json` 의 같은 키와 갈린다.
                derived_status: crate::report::stands_on(&issues[at], |k| states.get(&k).copied()).map(str::to_string),
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
///
/// **영어다**(moai-l5uf, 2026-09-20 사용자 결정). 한때 `없음`·`길잃음` 이었는데, 도움말이
/// 영어로 서면서 영어 화면이 받는 값만 한국어로 남았다. 옛 낱말은 **안 받는다** — 받는 말을
/// 둘로 두면 그것이 곧 둘째 어휘고, 지우려면 남의 스크립트를 깨야 한다. v0.1.0 을 안 내보낸
/// 지금은 밖에서 이 낱말을 치는 사람이 없어 값이 0 이다.
///
/// **`--json` 의 `path` 도 이 낱말을 낸다.** 내는 말과 받는 말이 갈리면 그 출력을 그대로
/// `--path` 에 넣는 고리가 끊긴다 — 기계가 읽고 다시 치는 자리라 한 낱말이어야 한다.
const NO_MILESTONE: &str = "none";
const LOST: &str = "lost";

// ── 화면 ──────────────────────────────────────────────────────────────

use crate::tui::App;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, Event, KeyEventKind, MouseEventKind,
};

/// bracketed paste 를 켜거나 끈다 — xterm 의 2004 번. 쓰는 곳을 받는 것은 시험이 그 글을
/// 보려고서다([`modes_off_on_panic`] 이 같은 글을 낸다).
fn bracketed_paste(out: &mut impl std::io::Write, on: bool) -> std::io::Result<()> {
    if on {
        ratatui::crossterm::execute!(out, EnableBracketedPaste)
    } else {
        ratatui::crossterm::execute!(out, DisableBracketedPaste)
    }
}

/// 마우스 잡기를 켜거나 끈다 — xterm 의 1000 번(누르고 뗌)·1002 번(누른 채 끌기)·1006 번(SGR 좌표).
///
/// **켤 때는 crossterm 의 `EnableMouseCapture` 를 안 쓴다.** 그것은 1003 번(누르지 않은 움직임까지 전부)도 켜는데,
/// 루프는 사건이 오면 깨어 그린다 — 마우스를 화면 위로 지나가게만 해도 이슈 1만 개 저장소에서 22ms 짜리 프레임이
/// 움직이는 내내 돈다. 듣는 것은 누르기·휠·끌기뿐이고 휠도 제 좌표를 실어 오므로, 올라선 칸(hover)을 알려고
/// 움직임을 받을 까닭이 없다. 1015 번(urxvt 좌표)도 안 켠다 — 1006 을 아는 터미널은 그쪽을 쓰고, 둘을 다 켜면
/// 어느 꼴로 올지가 터미널마다 갈린다. **켜기 전에 둘을 끈다**(리뷰) — 편집기 같은 남이 켜 둔 채 끝났으면 그대로
/// 남아 움직임마다 사건이 온다.
///
/// **끌 때는 `DisableMouseCapture` 를 쓴다**(리뷰) — 켠 차례의 거꾸로이고 1003·1015 번까지 끈다. 안 켠 것을 끄는
/// 것은 터미널에 아무 일도 아니다.
///
/// 쓰는 곳을 받는 것은 [`bracketed_paste`] 와 같은 까닭이다 — 시험이 그 글을 본다.
fn mouse_capture(out: &mut impl std::io::Write, on: bool) -> std::io::Result<()> {
    if !on {
        return ratatui::crossterm::execute!(out, DisableMouseCapture);
    }
    out.write_all(b"\x1b[?1003l\x1b[?1015l\x1b[?1000h\x1b[?1002h\x1b[?1006h")?;
    out.flush()
}

/// 화면이 잡는 모드를 켠다 — bracketed paste 와, 놓아 두지 않았으면 마우스. 띄울 때와 편집기에서 돌아올 때가
/// 같은 차례다. **켜기를 못 해도 멈추지 않는다** — 붙여넣기는 옛날처럼 키로 오고, 마우스는 터미널의 것으로 남는다.
fn modes_on(mouse: bool) {
    let _ = bracketed_paste(&mut std::io::stdout(), true);
    let _ = mouse_capture(&mut std::io::stdout(), mouse);
}

/// 화면이 잡은 모드를 다 놓는다 — 켠 차례의 거꾸로다. 끝낼 때·패닉 훅·편집기로 넘길 때가 같은 글을 낸다
/// (리뷰) — 셋이 저마다 적으면 모드를 하나 더하는 날 한 곳이 빠져, 그 길로 나간 셸에 날 글이 찍힌다.
fn modes_off() {
    let _ = mouse_capture(&mut std::io::stdout(), false);
    let _ = bracketed_paste(&mut std::io::stdout(), false);
}

/// 모드를 다 놓고, 마우스를 잡고 있었으면(`mouse`) **길에 남은 보고를 버린다**([`drain_reports`]) — 끝낼 때와
/// 편집기로 넘길 때가 같은 차례다. raw mode 를 걷기 **전에** 부른다. `rolled` 는 루프가 휠·끌기를 마지막으로 받은
/// 때다([`App::rolled_at`]).
///
/// 패닉 훅은 [`modes_off`] 만 부른다 — 훅은 아무 스레드에서나 돌고, 루프 스레드가 키를 읽는 동안 같은 tty 를
/// 읽으면 둘이 바이트를 나눠 가져 키도 답도 반쪽이 된다.
fn modes_off_draining(mouse: bool, rolled: Option<std::time::Instant>) {
    modes_off();
    if mouse {
        drain_reports(rolled);
    }
}

/// 마우스를 놓은 뒤에도 tty 에 쌓인 보고를 **셸이나 편집기가 읽기 전에 버린다**(moai-1thb). 휠을 굴리다 곧바로
/// 끝내면(트랙패드의 관성, 느린 ssh) 놓는 글이 터미널에 닿기 전에 나온 보고가 쌓여 있다가, raw mode 를 걷은 셸
/// 프롬프트에 `65;40;12M` 같은 글로 찍히거나 편집기에 키로 들어간다.
///
/// **다 왔는지는 터미널에 물어 안다**(DA1, 사용자 결정 2026-10-01). 놓는 글 뒤에 `ESC[c` 를 쓰면 터미널은 받은
/// 차례대로 답하므로, 그 답(`ESC[?…c`)이 오면 앞서 나온 보고는 다 온 것이다. 그때까지 온 것은 다 버린다 — 그사이
/// 친 키도 함께 버려진다(왕복 한 번). 시간만 재고 비우면 느린 ssh 에서 아직 길에 있던 보고가 지나가고, 이미 온
/// 것만 비우면(`poll(ZERO)`) 그마저 놓친다. 답 **뒤**의 글은 안 건드린다 — 한 바이트씩 읽어 답에서 멈춘다.
/// **휠·끌기 직후에 끝낸 때만 예외다**([`drain_answered`], moai-pvpb.m2f) — mosh 처럼 답이 먼저 오는 자리에서 답
/// 뒤로 이어지는 보고까지 버리고, 보고가 아닌 첫 바이트에서 멈춘다(그 한 바이트는 먹힌다).
///
/// **답이 안 오면 [`ANSWER_WAIT`] 에서 끊는다.** 늦게 온 답은 그 자체가 셸에 찍히니 넉넉히 둔다.
/// **raw mode 가 이미 걷혔으면 안 묻는다** — 줄 단위 입력에서는 줄바꿈 없는 답이 끝내 안 읽혀 끊을 때까지
/// 기다리고, 답은 그대로 셸에 남는다. 루프 스레드가 패닉하면 ratatui 의 훅이 `screen` 보다 먼저 걷는다.
#[cfg(unix)]
fn drain_reports(rolled: Option<std::time::Instant>) {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    if !ratatui::crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        return;
    }
    // crossterm 이 키를 읽는 그 자리다 — stdin 이 터미널이면 stdin, 아니면 `/dev/tty`.
    let tty;
    let fd = if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        libc::STDIN_FILENO
    } else {
        match std::fs::File::open("/dev/tty") {
            Ok(f) => {
                tty = f;
                tty.as_raw_fd()
            }
            Err(_) => return,
        }
    };
    // **답을 못 읽을 자리에는 묻지 않는다.** poll 이 그 fd 를 못 보거나(POLLNVAL) 끊겼으면(POLLHUP·POLLERR)
    // 아래 기다림은 곧바로 손을 떼고, 그 뒤에 온 답은 버려지지 않은 채 셸에 찍힌다 — 묻지 않은 것보다 나쁘다.
    let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
    if unsafe { libc::poll(&mut p, 1, 0) } < 0 || p.revents & (libc::POLLNVAL | libc::POLLHUP | libc::POLLERR) != 0 {
        return;
    }
    let mut out = std::io::stdout();
    if out.write_all(b"\x1b[c").and_then(|()| out.flush()).is_err() {
        return;
    }
    drain_answered(fd, || rolled_lately(rolled, std::time::Instant::now()));
}

/// DA1 의 답까지 버리고, **답을 봤고 휠·끌기 직후에 끝낸 때(`rolled`)만** 답 뒤의 보고까지 더 버린다(moai-pvpb.m2f,
/// 2026-10-02 사용자 결정) — 까닭은 [`drain_trailing`] 의 문서다. 휠을 안 쓴 끝내기는 전처럼 답에서 멈춰 그 뒤를 안
/// 건드린다. 묻는 글(`ESC[c`)을 쓰는 쪽과 가른 것은 시험이 이 문을 파이프로 재려고서다.
///
/// `rolled` 는 **답을 본 뒤에** 묻는다 — 답을 기다린 동안도 휠에서 멀어진 때로 센다.
#[cfg(unix)]
fn drain_answered(fd: std::os::fd::RawFd, rolled: impl FnOnce() -> bool) {
    if drain_until_answer(fd, std::time::Instant::now() + ANSWER_WAIT) && rolled() {
        drain_trailing(fd, TRAIL_QUIET, std::time::Instant::now() + ANSWER_WAIT);
    }
}

/// 끝내기 [`ROLLED_LATELY`] 안에 휠·끌기가 있었는가 — `rolled` 는 루프가 그것을 마지막으로 받은 때다
/// ([`App::rolled_at`]). 없었으면 답 뒤로 올 보고도 없다고 본다.
///
/// **그 때는 `App` 이 든다**(리뷰). 한때 프로세스 하나의 전역에 두었는데, 읽는 두 자리([`screen`]·[`suspend`])가 다
/// `App` 을 쥐고 있어 전역일 까닭이 없었다 — 전역인 [`EDITING`] 은 아무 스레드에서나 도는 패닉 훅이 읽어서다. 전역이면
/// 나란히 도는 시험 하나가 휠을 [`take`] 에 먹이는 날 다른 시험의 "오래전 휠" 이 깨진다.
#[cfg(unix)]
fn rolled_lately(rolled: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    rolled.is_some_and(|at| now.saturating_duration_since(at) <= ROLLED_LATELY)
}

/// 휠·끌기 뒤 이만큼 안에 끝내면 답 뒤를 더 비운다(사용자 결정). 트랙패드의 관성과 원격의 왕복이 이 안에 든다.
#[cfg(unix)]
const ROLLED_LATELY: std::time::Duration = std::time::Duration::from_secs(1);

/// 답 뒤로 보고가 끊긴 채 이만큼 조용하면 다 온 것으로 본다(사용자 결정). mosh 의 왕복이 대개 이 안이다.
#[cfg(unix)]
const TRAIL_QUIET: std::time::Duration = std::time::Duration::from_millis(300);

/// DA1 의 답 **뒤로** 마우스 보고가 이어지는 동안 더 버린다(moai-pvpb.m2f, 2026-10-02 사용자 결정). 버린 보고 수를 낸다.
///
/// **mosh 아래에서는 답이 먼저 온다.** mosh-server 가 `ESC[c` 에 제 손으로 곧바로 답하는데, 사람의 터미널이
/// 낸 휠 보고는 아직 망을 건너오고 있다 — [`drain_until_answer`] 가 그 이른 답에서 멈추면 늦은 보고가 셸 프롬프트나
/// `$EDITOR` 에 찍힌다. tmux 는 마우스를 끈 칸에 보고를 안 넘겨 이 일이 없다. GNU screen 은 기본값(`mousetrack off`)
/// 에서 제가 답한 뒤에 온 보고를 그대로 넘겨 mosh 와 같은 일이 난다(리뷰 — screen 의 `display.c` 를 읽었다).
///
/// **조용한 틈([`TRAIL_QUIET`])마다 다시 잰다** — 바이트 하나를 받을 때마다 틈이 새로 열리고, 틈 안에 아무것도 안
/// 오면 끝낸다. 가장 긴 때는 `until` 이다.
///
/// **보고가 아닌 키가 오면 멈춘다 — 그 키 하나는 먹힌다**(사람이 받아들인 비용이다). tty 는 엿볼 수 없어, 읽어 봐야
/// 키인 줄 안다. 그 비용을 휠을 굴린 직후에 끝낸 때에만 치르게 하는 것은 부르는 쪽의 문([`drain_answered`])이다
/// — 휠을 안 쓴 끝내기는 전처럼 안 기다리고 키도 안 먹는다. 편집기로 넘길 때([`suspend`])도 같은 문을 지난다.
///
/// **먹는 것은 키 하나를 끝까지다**(2026-10-02 사용자 결정, 리뷰 moai-pvpb.rrr 8번) — 멈춘 바이트가 여러 바이트 키의
/// 머리면 그 키의 끝 바이트까지 읽어 버린다([`Rest`]): 수를 단 키(Delete `ESC [ 3 ~`, Ctrl-← `ESC [ 1 ; 5 D`)는 끝
/// 글자까지, F1 같은 `ESC O P` 는 한 바이트 더, 한글 한 자는 UTF-8 의 나머지 바이트까지. 한 바이트에서 멈추던 판은
/// 그 꼬리(`~`·`;5D`·깨진 글자)를 셸 프롬프트에 남겼다. **새 `ESC` 는 처음부터 다시 잰다** — 혼자 온 Esc 키 뒤에 보고가
/// 이어지면(`ESC ESC [ < …`) 앞의 Esc 만 먹고 보고는 계속 버린다.
///
/// 알아보는 보고는 켠 모드의 둘이다([`mouse_capture`]): 1006 의 `ESC [ < 수;수;수 M|m` 과, 1006 을 모르는 터미널의
/// `ESC [ M` 과 세 바이트.
#[cfg(unix)]
fn drain_trailing(fd: std::os::fd::RawFd, quiet: std::time::Duration, until: std::time::Instant) -> usize {
    let mut report = Report::default();
    let mut dropped = 0;
    while let Some(byte) = next_byte(fd, quiet, until) {
        match report.push(byte) {
            Step::Report => dropped += 1,
            Step::More => {}
            Step::Key(rest) => {
                swallow(fd, rest, quiet, until);
                return dropped;
            }
        }
    }
    dropped
}

/// 키의 나머지를 읽어 버린다([`Rest`]). 키의 바이트는 함께 오므로 조용한 틈 안에 안 오면 거기서 끝낸다.
#[cfg(unix)]
fn swallow(fd: std::os::fd::RawFd, rest: Rest, quiet: std::time::Duration, until: std::time::Instant) {
    match rest {
        Rest::Done => {}
        Rest::Final => {
            // 매개변수·중간 바이트(`0x20..=0x3F`)를 지나 끝 글자(`0x40..=0x7E`)까지.
            while let Some(b) = next_byte(fd, quiet, until) {
                if !(0x20..=0x3f).contains(&b) {
                    return;
                }
            }
        }
        Rest::Bytes(n) => {
            for _ in 0..n {
                if next_byte(fd, quiet, until).is_none() {
                    return;
                }
            }
        }
    }
}

/// 한 바이트를 기다려 읽는다 — `quiet` 와 `until` 중 이른 때까지. 안 오거나 못 읽으면 `None` 이다.
#[cfg(unix)]
fn next_byte(fd: std::os::fd::RawFd, quiet: std::time::Duration, until: std::time::Instant) -> Option<u8> {
    loop {
        let left = until.saturating_duration_since(std::time::Instant::now()).min(quiet);
        if left.is_zero() {
            return None;
        }
        let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let ms = i32::try_from(left.as_millis()).unwrap_or(i32::MAX).max(1);
        let ready = unsafe { libc::poll(&mut p, 1, ms) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if ready <= 0 || p.revents & libc::POLLIN == 0 {
            return None;
        }
        let mut byte = 0u8;
        match unsafe { libc::read(fd, (&raw mut byte).cast(), 1) } {
            1 => return Some(byte),
            0 => return None,
            _ => {
                let e = std::io::Error::last_os_error().kind();
                if !matches!(e, std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) {
                    return None;
                }
            }
        }
    }
}

/// [`Report::push`] 의 답 — 보고가 막 끝났다, 아직이다, 보고가 아닌 키다(그 나머지와 함께).
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Report,
    More,
    Key(Rest),
}

/// 보고가 아닌 키에서 **아직 안 읽은 나머지**.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
enum Rest {
    /// 다 읽었다.
    Done,
    /// CSI 키 — 끝 글자(`0x40..=0x7E`)까지.
    Final,
    /// 바이트 몇 개 더 — `ESC O x` 의 `x` 하나, UTF-8 글자의 나머지.
    Bytes(u8),
}

/// 이 바이트로 시작하는 UTF-8 글자의 나머지 바이트 수. ASCII 와 이어지는 바이트는 0 이다.
#[cfg(unix)]
fn utf8_rest(lead: u8) -> u8 {
    match lead {
        0xc0..=0xdf => 1,
        0xe0..=0xef => 2,
        0xf0..=0xf7 => 3,
        _ => 0,
    }
}

/// 마우스 보고 하나를 한 바이트씩 알아본다 — [`Answer`] 와 같은 꼴이다. 보고가 아닌 키를 만나면 그 키의 나머지를
/// 함께 댄다([`Step::Key`]).
#[cfg(unix)]
#[derive(Default)]
struct Report(Reading);

#[cfg(unix)]
#[derive(Default, Clone, Copy)]
enum Reading {
    #[default]
    Start,
    Esc,
    Csi,
    /// 1006 의 수와 `;`.
    Sgr,
    /// 1006 을 모르는 터미널의 `ESC [ M` 뒤 — 남은 바이트 수.
    Raw(u8),
}

#[cfg(unix)]
impl Report {
    fn push(&mut self, byte: u8) -> Step {
        use Reading::*;
        let key = |rest| {
            // 키를 만나면 처음으로 돌아간다 — 부르는 쪽은 거기서 멈추지만, 상태를 남기지 않는다.
            (Start, Step::Key(rest))
        };
        let (next, step) = match (self.0, byte) {
            // 옛 꼴의 세 바이트는 무엇이든 보고다(좌표가 `ESC` 와 같은 값일 수도 있다).
            (Raw(1), _) => (Start, Step::Report),
            (Raw(n), _) => (Raw(n - 1), Step::More),
            // **새 `ESC` 는 처음부터 다시 잰다** — 앞의 것은 혼자 온 Esc 키였다.
            (_, 0x1b) => (Esc, Step::More),
            (Start, b) => key(Rest::Bytes(utf8_rest(b))),
            (Esc, b'[') => (Csi, Step::More),
            // SS3 키(F1 `ESC O P`, 어떤 터미널의 화살표) — 한 바이트 더.
            (Esc, b'O') => key(Rest::Bytes(1)),
            // Alt 와 함께 친 글자 — 그 글자의 나머지까지.
            (Esc, b) => key(Rest::Bytes(utf8_rest(b))),
            (Csi, b'<') => (Sgr, Step::More),
            (Csi, b'M') => (Raw(3), Step::More),
            (Sgr, b'0'..=b'9' | b';') => (Sgr, Step::More),
            (Sgr, b'M' | b'm') => (Start, Step::Report),
            // CSI 키 — 끝 글자면 다 왔고, 매개변수·중간 바이트면 끝 글자까지 더 읽는다.
            (Csi | Sgr, 0x40..=0x7e) => key(Rest::Done),
            (Csi | Sgr, 0x20..=0x3f) => key(Rest::Final),
            (Csi | Sgr, _) => key(Rest::Done),
        };
        self.0 = next;
        step
    }
}

/// 윈도의 콘솔은 마우스를 글이 아니라 입력 레코드로 주고, 셸은 그것을 키로 읽지 않는다 — 버릴 것이 없다.
#[cfg(not(unix))]
fn drain_reports(_rolled: Option<std::time::Instant>) {}

/// DA1 의 답을 기다리는 가장 긴 때. 터미널은 거의 다 답하고 곁에서는 1ms 안이다 — 이만큼 기다리는 것은 답하지 않는
/// 터미널이고, 그 사람은 마우스를 잡은 채 끝낼 때마다 이만큼 늦는다.
#[cfg(unix)]
const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// `fd` 에서 DA1 의 답이 올 때까지 **한 바이트씩** 읽어 버린다. 답을 보면 참이고, 때가 지나거나 못 읽으면 거짓이다.
/// 쓰는 쪽과 가른 것은 시험이 파이프로 재려고서다.
#[cfg(unix)]
fn drain_until_answer(fd: std::os::fd::RawFd, until: std::time::Instant) -> bool {
    let mut answer = Answer::default();
    loop {
        let left = until.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return false;
        }
        let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let ms = i32::try_from(left.as_millis()).unwrap_or(i32::MAX).max(1);
        let ready = unsafe { libc::poll(&mut p, 1, ms) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if ready == 0 {
            continue;
        }
        if ready < 0 || p.revents & libc::POLLIN == 0 {
            return false;
        }
        let mut byte = 0u8;
        match unsafe { libc::read(fd, (&raw mut byte).cast(), 1) } {
            1 => {
                if answer.push(byte) {
                    return true;
                }
            }
            0 => return false,
            _ => {
                let e = std::io::Error::last_os_error().kind();
                if !matches!(e, std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock) {
                    return false;
                }
            }
        }
    }
}

/// DA1 의 답 `ESC [ ? <수와 ;> c` 를 한 바이트씩 받아 알아본다. 마우스 보고(`ESC [ < … M`, 1006 을 모르는 터미널의
/// `ESC [ M` 과 세 바이트)도, kitty 의 키 플래그 답(`ESC [ ? … u`)도, DA2(`ESC [ > … c`)도 이 꼴이 아니다.
///
/// 유닉스에서만 짓는다 — 읽는 자([`drain_until_answer`])가 유닉스에만 있어, 다른 데서는 죽은 코드다.
#[cfg(unix)]
#[derive(Default)]
struct Answer(Heard);

#[cfg(unix)]
#[derive(Default, Clone, Copy)]
enum Heard {
    #[default]
    Nothing,
    Esc,
    Csi,
    Params,
}

#[cfg(unix)]
impl Answer {
    /// 한 바이트를 받는다. 답이 막 끝났으면 참이다.
    fn push(&mut self, byte: u8) -> bool {
        use Heard::*;
        self.0 = match (self.0, byte) {
            (_, 0x1b) => Esc,
            (Esc, b'[') => Csi,
            (Csi, b'?') => Params,
            (Params, b'0'..=b'9' | b';') => Params,
            (Params, b'c') => return true,
            _ => Nothing,
        };
        false
    }
}

/// 패닉하면 **먼저 마우스 잡기와 bracketed paste 를 끄고** 걸려 있던 훅(ratatui 의 터미널 복구)으로
/// 넘긴다. 어느 스레드의 패닉에도 돈다 — 버린 다시 읽기 스레드가 터져도 셸이 붙여넣기를 싸서 받지
/// 않고, 마우스를 움직일 때마다 프롬프트에 좌표 글이 찍히지 않는다. 마우스를 놓아 둔 때에도 끄는 글을
/// 낸다 — 안 켠 것을 끄는 것은 터미널에 아무 일도 아니고, 훅은 지금 무엇이 켜졌는지 모른다.
///
/// **편집기가 터미널을 쥔 동안에는 편집기가 끝날 때까지 기다린다**([`EDITING`]).
fn modes_off_on_panic() {
    let next = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        EDITING.wait();
        modes_off();
        next(info);
    }));
}

/// 편집기에 터미널을 넘긴 동안 **다른 스레드의 패닉 훅을 세워 두는 문**(moai-46xe).
///
/// 편집기가 도는 동안 루프 스레드는 편집기를 기다리지만, 짓던·버린 다시 읽기 스레드는 계속
/// 돈다. 그것이 터지면 훅이 raw mode 를 끄고 대체 화면을 걷어 편집기 화면이 흐트러진다.
///
/// **건너뛰지 않고 기다린다.** 건너뛰면 패닉 글이 사라진다. 기다렸다가 평소대로 돌면 훅이
/// 걷고 루프가 되던져 끝낸다. 기다리는 쪽은 그리지 않는 스레드라 세워 둬도 해가 없다.
///
/// **문이 막는 것은 쥔 뒤에 시작한 패닉뿐이다.** 쥐기 직전에 훅을 지난 패닉은 이미 터미널을
/// 걷었고, 편집기에서 돌아오며 다시 올린 화면은 훅이 다시 안 걷는다 — 그 끝은 `screen` 의
/// `catch_unwind` 가 맡는다. 문은 편집기 화면을 지키고, 셸을 되돌리는 것은 그쪽이다.
///
/// **문을 쥔 스레드 자신은 안 기다린다.** 편집기를 부르는 길에서 루프 스레드가 터지면 풀어
/// 줄 스레드가 자기뿐이라 영영 멈춘다.
static EDITING: Gate = Gate::new();

struct Gate {
    holder: std::sync::Mutex<Option<std::thread::ThreadId>>,
    opened: std::sync::Condvar,
}

impl Gate {
    const fn new() -> Gate {
        Gate { holder: std::sync::Mutex::new(None), opened: std::sync::Condvar::new() }
    }

    /// 이 스레드가 문을 쥔다.
    fn hold(&self) {
        *self.lock() = Some(std::thread::current().id());
    }

    /// 문을 연다 — 기다리던 훅이 전부 깬다.
    fn release(&self) {
        *self.lock() = None;
        self.opened.notify_all();
    }

    /// 남이 쥐고 있으면 열릴 때까지 선다.
    fn wait(&self) {
        let me = std::thread::current().id();
        let mut holder = self.lock();
        while holder.is_some_and(|h| h != me) {
            holder = self.opened.wait(holder).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// **훅 안에서 부르므로 독든 자물쇠에도 패닉하지 않는다** — 훅 안의 패닉은 프로세스를 끊는다.
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<std::thread::ThreadId>> {
        self.holder.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 생각 담기를 적을 편집기(moai-08af). **환경과 PATH 를 읽는 것은 여기다** — 고르는 차례는
/// 조각([`crate::tui::jotfile::pick`])이 정한다. 띄울 때 한 번 고른다: 도는 중에 `$EDITOR` 가
/// 바뀔 길은 없고, 키마다 PATH 를 훑을 까닭도 없다.
fn editor() -> Option<String> {
    let var = |k: &str| std::env::var(k).ok();
    crate::tui::jotfile::pick(var("VISUAL").as_deref(), var("EDITOR").as_deref(), on_path)
}

/// PATH 에 그 이름의 실행 파일이 있는가. **실행할 수 있는가는 [`super::runnable`] 이 답한다** —
/// `skill` 과 같은 자다(moai-p3kb).
///
/// 여기서 `PATH` 를 제 손으로 훑는 것은 껍데기를 띄우지 않으려는 것이다. 편집기 후보가 여럿이고
/// 띄울 때 한 번 고르는 자리라, 이름마다 `sh -c 'command -v'` 를 띄우면 그 값이 얻는 값보다 크다.
fn on_path(name: &str) -> bool {
    let Some(dirs) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&dirs).any(|d| super::runnable(&d.join(name)))
}

/// 터미널을 **내린다** — 편집기에 터미널을 넘기는 자리(moai-08af).
///
/// 끝낼 때(`screen`)와 같은 차례다: 마우스 잡기와 bracketed paste 를 끄고, 마우스를 잡고 있었으면(`mouse`) 길에
/// 남은 보고를 버리고([`drain_reports`] — 휠·끌기 직후(`rolled`)면 답 뒤까지), raw mode·대체 화면을 걷는다.
/// 안 끄면 편집기에 붙인 글이 `200~…201~` 에 싸여 들어가고, 마우스를 모르는 편집기에 누른 자리가 글로
/// 찍힌다. 편집기가 도는 동안 **루프 스레드는
/// 편집기를 기다리며 서 있다** — 그리기·스피너·다시 읽기 받기(`follow`)가 전부 멈춘다. 다시
/// 읽기 스레드는 계속 짓지만 그리지 않고, 돌아오면 다음 걸음이 받는다. 쥔 뒤에 그 스레드가
/// 터지면 훅은 [`EDITING`] 앞에서 선다 — 문은 **걷기 전에** 쥐고, 여는 것은 루프가 올린 뒤다.
fn suspend(mouse: bool, rolled: Option<std::time::Instant>) {
    EDITING.hold();
    modes_off_draining(mouse, rolled);
    ratatui::restore();
}

/// 내린 터미널을 다시 올리고 **다음 그림이 통째로 다시 그리게** 한다. ratatui 는 바뀐 칸만
/// 내보내는데, 편집기가 지나간 화면은 ratatui 가 아는 앞 그림과 다르다.
///
/// **`Terminal::clear` 를 안 쓴다.** ratatui-core 의 `clear` 는 커서 자리를 터미널에 물어(DSR)
/// 되돌리는데, 답하지 않는 터미널(pty 를 잇는 `script`, 느린 원격)에서는 몇 초 뒤 오류가 나
/// 루프가 끝난다 — 실제로 그렇게 끝났다. 온 화면을 지우고 앞 그림을 비우면(`swap_buffers`)
/// 다음 그림이 빈칸 아닌 칸을 모두 다시 낸다. `clear` 가 온 화면 뷰포트에서 하는 일과 같고
/// 묻는 것만 없다. 커서는 그림마다 숨기거나 두므로 따로 안 만진다.
///
/// 마우스는 **잡을 때만 다시 잡는다**(`mouse`, [`App::wants_mouse`]) — 놓아 둔 사람에게도, 편집기에서 돌아와 선
/// 폼 위에서도 안 잡는다.
fn resume(term: &mut DefaultTerminal, mouse: bool) -> std::io::Result<()> {
    use ratatui::crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, enable_raw_mode};
    enable_raw_mode()?;
    ratatui::crossterm::execute!(std::io::stdout(), EnterAlternateScreen, Clear(ClearType::All))?;
    modes_on(mouse);
    term.swap_buffers();
    Ok(())
}

/// `text` 를 임시 파일에 적고 `editor` 로 연 뒤 **고친 글**을 돌려준다. `Err` 는 담지 않을
/// 까닭이다 — 편집기가 0 이 아닌 코드로 끝났다(vim 의 `:cq`), 못 띄웠다, 못 읽었다.
///
/// 파일은 **어느 길로든 지운다.** 받은 글은 이미 돌려주었고, 담기가 실패해도 그 글은 폼에
/// 열린 채 남는다([`App::edited`]) — 파일에 둘 까닭이 없다. 편집기는 이 프로세스의 stdin·
/// stdout 을 그대로 받는다. 기다리는 동안 Ctrl-C 가 신호로 오면(raw 로 돌지 않는 편집기)
/// moai 도 같이 끝난다 — 터미널은 이미 내려 둔 채라 셸은 멀쩡하고, 파일 하나가 남는다.
fn write_in_editor(editor: &str, text: &str, dir: &std::path::Path, lang: crate::i18n::Lang) -> Result<String, String> {
    use crate::i18n::{fill, say};
    use std::io::Write;
    // **키는 낱말째 적는다** — 소스를 훑는 시험(`i18n::tests`)이 `say(…, "키")` 모양만 읽어,
    // 키를 닫힘에 넘기면 그 눈에서 통째로 사라진다.
    let (path, mut file) =
        scratch_file(dir).map_err(|e| fill(say(lang, "tui.editor_no_temp_file"), &[("why", &e.to_string())]))?;
    let written = file.write_all(text.as_bytes()).and_then(|()| file.sync_all());
    drop(file);
    let got = written
        .map_err(|e| fill(say(lang, "tui.editor_no_write"), &[("why", &e.to_string())]))
        .and_then(|()| {
            std::process::Command::new("sh")
                .args(crate::tui::jotfile::argv(editor, &path))
                .status()
                .map_err(|e| fill(say(lang, "tui.editor_not_started"), &[("editor", editor), ("why", &e.to_string())]))
        })
        .and_then(|status| match status.code() {
            Some(0) => std::fs::read_to_string(&path)
                .map_err(|e| fill(say(lang, "tui.editor_no_read"), &[("why", &e.to_string())])),
            Some(code) => Err(fill(say(lang, "tui.editor_exit"), &[("editor", editor), ("code", &code.to_string())])),
            None => Err(fill(say(lang, "tui.editor_signal"), &[("editor", editor)])),
        });
    let _ = std::fs::remove_file(&path);
    got
}

/// 편집기에 넘길 새 파일. **남이 못 읽게(0600) 새로 만든다** — 공유 임시 디렉터리라 이름을
/// 짐작한 남이 먼저 둔 파일(심볼릭 링크 포함)을 열면 적은 생각이 그리로 샌다. `create_new` 는
/// 있는 것을 안 연다. `.md` 는 편집기가 본문을 마크다운으로 칠하게 한다.
fn scratch_file(dir: &std::path::Path) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    private_file(dir, "moai-idea")
}

/// `dir` 안에 `<stem>-<pid>-<n>.md` 로 **남이 못 읽는 새 파일**을 만든다 — [`scratch_file`] 과
/// [`rescue`] 가 같이 쓴다. 이름이 있으면 다음 번호로 넘어간다.
fn private_file(dir: &std::path::Path, stem: &str) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..64 {
        let path = dir.join(format!("{stem}-{}-{}.md", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let mut open = std::fs::OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
        match open.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    // io 의 오류 글이라 영어 하나다 — 부르는 쪽이 제 말로 감싼다(moai-na0d).
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free name"))
}

/// 버릴 뻔한 글을 `dir` 의 새 파일에 **편집기 글과 같은 모양**으로 적고 그 경로를 돌려준다
/// (moai-y3r7) — 첫 줄 제목, 한 줄 띄우고 본문. **파일에는 적은 글이 바이트 그대로 다 든다.**
/// 다만 편집기 길(`jotfile::parse`)로 도로 읽으면 그 형식의 규칙을 탄다 — 제목이 비면 본문 첫
/// 줄이 제목이 되고, `# ` 로 시작하는 줄은 안내 주석으로 걷힌다(리뷰 moai-y3r7.u3p). 사람이
/// 열어 옮겨 담는 것이 목적이라 그대로 둔다. 파일은 남이 못 읽게 새로 만든다([`private_file`])
/// — 적은 생각이 공유 임시 디렉터리로 새지 않게.
fn rescue(title: &str, body: Option<&str>, dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    let (path, mut file) = private_file(dir, "moai-unsaved")?;
    let text = match body {
        Some(body) => format!("{title}\n\n{body}\n"),
        None => format!("{title}\n"),
    };
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(path)
}

/// 루프가 오류·패닉으로 끝났을 때 **폼에 남은 글을 잃지 않는다**(moai-y3r7). 조용한 손실은 이
/// 도구가 못 견디는 유일한 실패 모드라, 올리기 실패만이 아니라 루프를 끝낸 오류면 무엇이든
/// 이 길을 지난다(사람이 정했다). 파일로도 못 남기면 **글 자체를 stderr 에 낸다** — 터미널을
/// 걷은 뒤에 부르므로 셸에 보인다.
fn keep_unsaved(app: &App) {
    let Some((title, body)) = app.unsaved() else { return };
    match rescue(&title, body.as_deref(), &std::env::temp_dir()) {
        Ok(path) => eprintln!(
            "moai: {}\n      {}",
            crate::i18n::fill(
                crate::i18n::say(app.site.lang, "tui.unsaved_kept"),
                &[("at", &path.display().to_string())]
            ),
            crate::i18n::say(app.site.lang, "tui.unsaved_kept_how")
        ),
        Err(e) => eprintln!(
            "moai: {}\n{title}\n\n{}",
            crate::i18n::fill(crate::i18n::say(app.site.lang, "tui.unsaved_lost"), &[("why", &e.to_string())]),
            body.unwrap_or_default()
        ),
    }
}

/// 루프가 받은 사건 하나를 탐색기에 넘긴다. **터미널 없이 시험된다.**
///
/// - 키는 **누를 때만** 받는다. crossterm 은 kitty 프로토콜 터미널에서 뗄 때도 보내므로,
///   거르지 않으면 키 하나가 두 번 먹는다
/// - 붙여넣기는 글로 넘긴다([`App::paste`]) — 키로 풀지 않는다
/// - 마우스는 그대로 넘긴다([`App::mouse`]) — 어느 칸을 눌렀는지는 탐색기가 지난 그림으로 맞힌다
/// - 창 크기 따위는 받을 것이 없다. 다음 그림이 새 크기로 그린다
fn take(app: &mut App, ev: Event) {
    // 휠·끌기를 받은 때를 적는다 — 끝낼 때 답 뒤를 더 비울지 가르는 자다([`rolled_lately`], moai-pvpb.m2f).
    if rolls(&ev) {
        app.rolled_at = Some(std::time::Instant::now());
    }
    match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press => app.key(k),
        Event::Paste(text) => app.paste(&text),
        Event::Mouse(m) => app.mouse(m),
        _ => {}
    }
}

/// 몰아 받는 사건인가 — 휠과 누른 채 끌기(moai-irrj, 리뷰). 둘 다 한 손짓에 칸마다 하나씩 오는데, 사건마다 한 프레임을
/// 그리면 큰 저장소(이슈 1만 개에 한 프레임 22ms)에서 손을 멈춘 뒤에도 목록이 한참 따라 구른다.
///
/// **누르기·뗌·키는 안 몬다** — 하나 받고 그린다. 키는 편집기를 부르거나(`App::edit`) 끝낼 수 있고, 누르기는 다음
/// 누르기가 맞힐 화면을 바꾼다(커서·포커스·알림 줄). 몰아 받는 동안 맞히는 바탕은 지난 프레임이고, 그동안 안 그렸으니
/// 그것이 곧 사람이 보는 화면이다.
fn rolls(ev: &Event) -> bool {
    use MouseEventKind as K;
    matches!(
        ev,
        Event::Mouse(m) if matches!(m.kind, K::Drag(_) | K::Moved | K::ScrollUp | K::ScrollDown | K::ScrollLeft | K::ScrollRight)
    )
}

/// 몰아 받는 데 쓰는 가장 긴 때. 넘으면 받던 것을 두고 한 번 그린다 — 목록 위의 휠은 사건마다 목록을 세므로, 끝없이
/// 오는 휠에 그림이 안 서면 화면이 멈춘 것으로 보인다.
const BURST: std::time::Duration = std::time::Duration::from_millis(50);

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
    // 터미널이 지금 마우스를 잡고 있는가 — `screen` 이 띄울 때 `app.wants_mouse()` 대로 켰다. `SPC o m` 이 바꾼
    // 값도, 폼·글 받는 칸·고르는 창이 열리고 닫히며 바뀐 값도 사건을 받은 바로 뒤에 터미널로 낸다(아래). 편집기에서
    // 돌아올 때(`resume`)는 그때의 `wants_mouse` 대로 켜고 이 값도 그것으로 고쳐 적는다.
    let mut caught = app.wants_mouse();
    // 앞 프레임을 들고 그린다 — 넓은 글자의 반쪽이 남긴 칸을 다시 내려고(moai-c6go, `draw::Painter`).
    let mut painter = crate::tui::draw::Painter::default();
    while !app.quit {
        let began = std::time::Instant::now();
        painter.draw(term, app)?;
        let step = spin_step(began.elapsed());
        // **화면에 도는 것이 없으면 빠른 걸음으로 깨지 않는다.** 다 끝난 판을 열어 둔
        // 채로 둔 사람의 CPU 를 초당 여덟 번 깨울 까닭이 없고, 집은 일이 다른 에픽 안이나
        // 스크롤 밖에 있어 안 보일 때도 같다(moai-5jh6). 판단은 방금 그린 버퍼가 한다
        // (`draw::screen` 이 `App::spun` 에 적는다) — 보이는 줄을 여기서 다시 세면 목록의
        // 스크롤 창·상세의 굴림·폼의 덮음을 그리는 쪽과 따로 맞춰야 한다. 스크롤·이동·다시
        // 읽기로 보이는 것이 바뀌면 다음 프레임이 그린 뒤 따라온다.
        let spinning = app.spun;
        // **시간으로 센다, 한가함으로 세지 않는다.** 이벤트가 오는 동안에만
        // 안 보면 — 키를 누르고 있거나 창을 끄는 내내 — 바뀐 것을 못 본다.
        // 하필 그때가 쓰는 사람이 화면을 보고 있는 때다.
        let wake = if spinning { stale_due.min(spin_due) } else { stale_due };
        let wait = wake.saturating_duration_since(std::time::Instant::now());
        if event::poll(wait)? {
            // **휠·끌기는 몰아 받는다**(`rolls`) — 이어 와 있는 동안은 그리지 않고 받고, 그 밖의 사건 하나에서 멈춰
            // 그것까지 받은 뒤 그린다. 받는 때는 [`BURST`] 로 묶는다.
            let until = std::time::Instant::now() + BURST;
            let mut ev = event::read()?;
            while rolls(&ev) && std::time::Instant::now() < until && event::poll(std::time::Duration::ZERO)? {
                take(app, ev);
                ev = event::read()?;
            }
            take(app, ev);
        }
        if app.wants_mouse() != caught {
            caught = app.wants_mouse();
            let _ = mouse_capture(&mut std::io::stdout(), caught);
        }
        // 키가 편집기를 청했으면(`n`) 터미널을 넘긴다. 받은 글은 App 이 담는다 — 담을 곳은 연
        // 순간 박힌 그대로 요청에 실려 왔다.
        if let Some(edit) = app.edit.take() {
            suspend(caught, app.rolled_at);
            let got = write_in_editor(&edit.editor, &edit.text, &std::env::temp_dir(), app.site.lang);
            // **받은 글을 올리기보다 먼저 담는다.** 올리기가 실패하면 루프가 끝나는데, 먼저 담아
            // 두면 적은 것은 파일에 있다 — 거꾸로 하면 편집기에서 적은 글이 임시 파일과 함께 사라진다.
            app.edited(edit.into, got);
            // **올린 뒤에 연다.** 먼저 열면 기다리던 훅이 이미 내린 터미널을 걷고, 그 뒤에 올린
            // 화면은 `resume_unwind` 가 훅 없이 끝내며 raw·대체 화면인 채로 셸에 남는다. 올리기가
            // 실패해도 연다 — 기다리던 훅이 패닉 글을 내야 한다.
            //
            // **다시 잡은 값을 `caught` 에 적는다.** 담기가 폼을 열어 두면(못 담음·누구냐 물음) 여기서는 안 잡는데,
            // 옛 값을 들고 있으면 그 폼을 바로 닫은 사건 뒤에 `wants_mouse` 가 옛 값과 같아 다시 잡지 않는다.
            caught = app.wants_mouse();
            let up = resume(term, caught);
            painter.forget();
            EDITING.release();
            up?;
        }
        let now = std::time::Instant::now();
        // 키가 읽기를 띄웠으면(층으로 올라가기 따위) 느린 걸음까지 기다리지 않고 받으러 깬다.
        if app.loading() {
            stale_due = stale_due.min(now + LOAD_POLL);
        }
        if now >= stale_due {
            app.follow();
            stale_due = now + if app.loading() || app.reaping() || app.gathering_commits() { LOAD_POLL } else { TICK };
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
    /// **문이 반대로 서면 `--json` 이 매번 바깥을 두드린다**(moai-3gia, 리뷰). `latest::gate` 는
    /// 제자리에서 잘 재지만, 그 답을 **쓰는** 자리가 안 재이던 자리였다.
    #[test]
    fn the_explorer_asks_only_when_every_door_is_open() {
        let s = crate::scratch::Scratch::new("tui-latest-gate");
        let at = s.path().join("config.toml");
        std::fs::write(&at, "").unwrap();
        let none = |_: &str| None;
        // **설정이 무엇이라 했는지는 이제 부르는 쪽이 들고 온다**(moai-d74q) — 설정을 읽는 것은
        // `user_config::read` 한 번이고, 이 자리는 그 답으로 문만 잰다.
        let says = |src: &str| crate::user_config::read(Some(&at_with(&at, src))).update_check;
        assert_eq!(super::asking_from(Some(&at), None, false, true, none).as_deref(), Some(s.path()));
        // `--json` 은 화면이 터미널이어도 안 묻는다 — 에이전트가 치는 자리다.
        assert_eq!(super::asking_from(Some(&at), None, true, true, none), None);
        // 사람이 보는 화면이 아니면 안 묻는다(파이프).
        assert_eq!(super::asking_from(Some(&at), None, false, false, none), None);
        // 설정이 껐으면 안 묻는다.
        assert_eq!(super::asking_from(Some(&at), says("[update]\ncheck = false\n"), false, true, none), None);
        // **틀린 값은 끄지 않는다** — 읽은 것이 없으니 켠 것이고, 까닭은 알림으로 선다.
        assert_eq!(
            super::asking_from(Some(&at), says("[update]\ncheck = \"no\"\n"), false, true, none).as_deref(),
            Some(s.path())
        );
        // 환경이 껐으면 안 묻는다.
        let off = |k: &str| (k == crate::latest::OFF_VAR).then(|| std::ffi::OsString::from("1"));
        assert_eq!(super::asking_from(Some(&at), None, false, true, off), None);
        // 답을 둘 자리를 모르면 안 묻는다 — 창을 닫을 자리가 없어 부를 때마다 두드리게 된다.
        assert_eq!(super::asking_from(None, None, false, true, none), None);
    }

    /// 설정 파일에 `src` 를 적고 그 자리를 돌려준다.
    fn at_with(at: &std::path::Path, src: &str) -> std::path::PathBuf {
        std::fs::write(at, src).unwrap();
        at.to_path_buf()
    }

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

    /// **붙여넣기는 글로 가고 키로 안 풀린다**(moai-od9q). 루프가 받은 사건을 나누는 자리를
    /// 터미널 없이 본다 — 탐색 중에 붙인 `q` 는 끝내지 않고, 검색칸에 붙인 줄바꿈은 걸지 않는다.
    /// 누른 키만 먹는 것(kitty 의 뗌)도 같은 자리다.
    #[test]
    fn a_paste_event_is_text_and_only_pressed_keys_act() {
        use crate::tui::{Mode, input::Input};
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventState, KeyModifiers};
        let mut app = App::new(Vec::new(), crate::config::Config::parse("prefix = \"argos\"\n").unwrap(), Vec::new());
        take(&mut app, Event::Paste("q".into()));
        assert!(!app.quit, "붙인 q 가 끝냈다");
        take(&mut app, Event::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)));
        take(&mut app, Event::Paste("a\tb\r".into()));
        assert_eq!(app.mode, Mode::Grep(Input::new("a b"), crate::query::GrepIn::All));
        let release = KeyEvent::new_with_kind_and_state(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
            KeyEventState::NONE,
        );
        take(&mut app, Event::Key(release));
        assert!(matches!(app.mode, Mode::Grep(..)), "뗀 키를 먹었다");
    }

    /// 켜고 끄는 글은 **xterm 의 2004 번**이다. 끄는 글이 패닉 훅에도 쓰이므로 여기서 박아 둔다 —
    /// 안 끄고 나가면 사용자 셸에 붙인 글이 `200~…201~` 에 싸여 들어간다.
    #[test]
    fn bracketed_paste_is_mode_2004_on_and_off() {
        let mut out = Vec::new();
        bracketed_paste(&mut out, true).unwrap();
        bracketed_paste(&mut out, false).unwrap();
        assert_eq!(out, b"\x1b[?2004h\x1b[?2004l");
    }

    /// 마우스 잡기는 **xterm 의 1000·1002·1006 번**이고 끌 때는 거꾸로 걷는다(moai-irrj.9xq). 1003 번(누르지
    /// 않은 움직임)이 끼면 마우스가 화면을 지날 때마다 한 프레임을 그린다 — 그래서 crossterm 의
    /// `EnableMouseCapture` 를 안 쓰고 여기서 박아 둔다. **켜기 전에 1003·1015 번을 끄고**, 끌 때는
    /// `DisableMouseCapture` 대로 둘까지 끈다(리뷰) — 남이 켜 둔 것이 남지 않게. 끄는 글은 패닉 훅과 편집기로
    /// 넘기는 길도 쓴다(`modes_off`).
    #[test]
    fn mouse_capture_is_modes_1000_1002_1006_and_never_1003() {
        let mut out = Vec::new();
        mouse_capture(&mut out, true).unwrap();
        assert_eq!(out, b"\x1b[?1003l\x1b[?1015l\x1b[?1000h\x1b[?1002h\x1b[?1006h");
        out.clear();
        mouse_capture(&mut out, false).unwrap();
        assert_eq!(out, b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l");
    }

    /// 받은 바이트를 넘기며 **DA1 의 답이 끝나는 자리**를 댄다 — 없으면 `None`.
    #[cfg(unix)]
    fn answered_at(bytes: &[u8]) -> Option<usize> {
        let mut a = Answer::default();
        bytes.iter().position(|&b| a.push(b))
    }

    /// **DA1 의 답만 답이다**(moai-1thb). 터미널마다 매개변수가 다르고(xterm `64;…`, 리눅스 콘솔 `6`, tmux `1;2`),
    /// 그 앞에 마우스 보고와 키가 섞여 와도 답에서 멈춘다. 마우스 보고·kitty 의 키 플래그 답·DA2 를 답으로 읽으면
    /// 보고가 다 오기 전에 비우기를 멈춘다.
    #[cfg(unix)]
    #[test]
    fn only_a_da1_reply_ends_the_drain() {
        for reply in [&b"\x1b[?64;1;2;6;9;15;18;21;22c"[..], b"\x1b[?6c", b"\x1b[?1;2c", b"\x1b[?c"] {
            assert_eq!(
                answered_at(reply),
                Some(reply.len() - 1),
                "{:?} 를 답으로 못 읽었다",
                String::from_utf8_lossy(reply)
            );
        }
        let mixed = b"\x1b[<65;40;12M\x1b[<64;40;12Mjk\x1b\x1b[?62;22c";
        assert_eq!(answered_at(mixed), Some(mixed.len() - 1), "앞에 보고와 키가 섞이자 답을 놓쳤다");
        for not in [
            &b"\x1b[<65;40;12M\x1b[<0;3;4m"[..],
            b"\x1b[M`!!",
            b"\x1b[?1u",
            b"\x1b[>1;10;0c",
            b"?64c",
            b"\x1b[?64;1x2c",
        ] {
            assert_eq!(answered_at(not), None, "{:?} 를 답으로 읽었다", String::from_utf8_lossy(not));
        }
    }

    /// 파이프를 하나 열고 `bytes` 를 써 둔다 — 읽는 쪽과 쓰는 쪽. 쓰는 쪽을 버리면 읽는 쪽은 남은 것 뒤에서 끝난다.
    #[cfg(unix)]
    fn pipe_with(bytes: &[u8]) -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
        use std::os::fd::FromRawFd;
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let wrote = unsafe { libc::write(fds[1], bytes.as_ptr().cast(), bytes.len()) };
        assert_eq!(usize::try_from(wrote).ok(), Some(bytes.len()));
        unsafe { (std::os::fd::OwnedFd::from_raw_fd(fds[0]), std::os::fd::OwnedFd::from_raw_fd(fds[1])) }
    }

    /// 읽는 쪽에 남은 것 전부.
    #[cfg(unix)]
    fn rest_of(read: std::os::fd::OwnedFd) -> Vec<u8> {
        let mut rest = Vec::new();
        std::io::Read::read_to_end(&mut std::fs::File::from(read), &mut rest).unwrap();
        rest
    }

    /// **답까지 버리고 답 뒤는 남긴다**(moai-1thb) — 쌓인 보고는 셸에 안 가고, 답 뒤에 친 키는 셸이 받는다. 덩어리로
    /// 읽으면 답 뒤의 키까지 삼킨다.
    #[cfg(unix)]
    #[test]
    fn the_drain_eats_up_to_the_reply_and_leaves_what_follows() {
        use std::os::fd::AsRawFd;
        let (read, write) = pipe_with(b"\x1b[<65;40;12M\x1b[<65;40;12M\x1b[<64;41;12Mq\x1b[?62;22cls\n");
        drop(write);
        let until = std::time::Instant::now() + Duration::from_secs(5);
        assert!(drain_until_answer(read.as_raw_fd(), until), "답을 보고도 참을 안 냈다");
        assert_eq!(rest_of(read), b"ls\n", "답 뒤의 글을 건드렸다");
    }

    /// **답 뒤로 이어지는 보고까지 버린다**(moai-pvpb.m2f, 2026-10-02 사용자 결정) — mosh 처럼 답이 먼저 오는 자리다.
    /// 1006 꼴과 옛 `ESC [ M` 꼴을 다 보고로 읽고, 보고가 아닌 첫 바이트에서 멈춘다 — 그 한 바이트는 먹히고 그 뒤는
    /// 남는다. 아무것도 안 오면 조용한 틈에서 끝낸다.
    #[cfg(unix)]
    #[test]
    fn the_trailing_drain_eats_reports_after_the_reply_and_stops_at_a_key() {
        use std::os::fd::AsRawFd;
        let (read, write) = pipe_with(b"\x1b[<65;40;12M\x1b[<64;40;12m\x1b[M`!!ls\n");
        let until = std::time::Instant::now() + Duration::from_secs(5);
        assert_eq!(drain_trailing(read.as_raw_fd(), Duration::from_millis(200), until), 3, "보고를 다 안 버렸다");
        drop(write);
        assert_eq!(rest_of(read), b"s\n", "키의 첫 바이트 뒤를 건드렸다");

        // 보고가 끊기면 조용한 틈에서 끝낸다 — 쓰는 쪽이 열려 있어도 기다림이 그 틈을 안 넘는다.
        let (read, write) = pipe_with(b"\x1b[<65;40;12M");
        let began = std::time::Instant::now();
        assert_eq!(drain_trailing(read.as_raw_fd(), Duration::from_millis(40), began + Duration::from_secs(5)), 1);
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(40) && took < Duration::from_secs(1), "틈을 안 지켰다 — {took:?}");
        drop(write);
        assert_eq!(rest_of(read), b"");

        // 화살표 키(`ESC [ A`)는 보고가 아니다 — 거기서 멈춘다.
        let (read, write) = pipe_with(b"\x1b[Ax");
        drop(write);
        assert_eq!(drain_trailing(read.as_raw_fd(), Duration::from_millis(40), until), 0);
        assert_eq!(rest_of(read), b"x");
    }

    /// **멈춘 키는 끝까지 먹는다**(2026-10-02 사용자 결정, 리뷰 moai-pvpb.rrr 8번) — 여러 바이트 키의 머리에서 멈추면
    /// 그 꼬리가 셸 프롬프트에 남았다(Delete 의 `~`, Ctrl-← 의 `;5D`, F1 의 `P`, 한글 한 자의 나머지 바이트). 그 뒤의
    /// 글은 안 건드린다. 혼자 온 Esc 키 뒤에 보고가 이어지면 Esc 만 먹고 보고는 계속 버린다.
    #[cfg(unix)]
    #[test]
    fn the_trailing_drain_eats_the_whole_key_it_stops_at() {
        use std::os::fd::AsRawFd;
        let until = std::time::Instant::now() + Duration::from_secs(5);
        let eaten = |sent: &[u8]| {
            let (read, write) = pipe_with(sent);
            drop(write);
            let dropped = drain_trailing(read.as_raw_fd(), Duration::from_millis(40), until);
            (dropped, rest_of(read))
        };
        for (what, key) in [
            ("Delete", &b"\x1b[3~"[..]),
            ("Ctrl-←", b"\x1b[1;5D"),
            ("F1", b"\x1bOP"),
            ("Alt-x", b"\x1bx"),
            ("한글", "한".as_bytes()),
            ("ASCII", b"l"),
        ] {
            let mut sent = b"\x1b[<65;40;12M".to_vec();
            sent.extend_from_slice(key);
            sent.extend_from_slice(b"ls\n");
            assert_eq!(eaten(&sent), (1, b"ls\n".to_vec()), "{what} 의 꼬리를 남겼거나 그 뒤를 먹었다");
        }
        // 혼자 온 Esc 뒤의 보고 — Esc 를 먹고 보고도 버린다.
        assert_eq!(eaten(b"\x1b\x1b[<65;40;12Mx"), (1, b"".to_vec()));
    }

    /// **문은 [`drain_answered`] 하나다**(moai-pvpb.m2f, 사용자 결정) — 휠을 안 쓴 끝내기는 답에서 멈춰 그 뒤의 보고도
    /// 키도 안 건드리고, 휠 직후의 끝내기만 답 뒤의 보고를 버린 뒤 보고가 아닌 첫 바이트에서 멈춘다. 문을 걷거나 늘
    /// 열어 두면 여기가 붉어진다.
    #[cfg(unix)]
    #[test]
    fn only_a_recent_wheel_drains_past_the_answer() {
        use std::os::fd::AsRawFd;
        let sent = b"\x1b[<65;40;12M\x1b[?62c\x1b[<64;40;12Mls\n";
        let (read, write) = pipe_with(sent);
        drop(write);
        drain_answered(read.as_raw_fd(), || false);
        assert_eq!(rest_of(read), b"\x1b[<64;40;12Mls\n", "휠을 안 쓴 끝내기가 답 뒤를 건드렸다");
        let (read, write) = pipe_with(sent);
        drop(write);
        drain_answered(read.as_raw_fd(), || true);
        assert_eq!(rest_of(read), b"s\n", "휠 직후의 끝내기가 답 뒤의 보고를 안 버렸다");
    }

    /// **답 뒤를 더 비우는 것은 휠·끌기 직후에 끝낸 때뿐이다**(사용자 결정) — 휠을 안 쓴 끝내기는 전처럼 안 기다리고
    /// 키도 안 먹는다. 그 때는 루프가 사건을 넘기는 [`take`] 가 `App` 에 적고, 키는 적지 않는다.
    #[cfg(unix)]
    #[test]
    fn only_a_recent_wheel_opens_the_trailing_drain() {
        let now = std::time::Instant::now();
        assert!(!rolled_lately(None, now), "휠이 없었는데 답 뒤를 비웠다");
        assert!(!rolled_lately(Some(now - Duration::from_secs(5)), now), "오래전 휠로 답 뒤를 비웠다");
        assert!(rolled_lately(Some(now - Duration::from_millis(200)), now));
        let wheel = Event::Mouse(ratatui::crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        });
        let key = ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('j'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        );
        let cfg = crate::config::Config::parse("prefix = \"argos\"\n").unwrap();
        let mut app = App::new(Vec::new(), cfg, crate::nav::Path::new());
        take(&mut app, Event::Key(key));
        assert_eq!(app.rolled_at, None, "키를 휠로 적었다");
        take(&mut app, wheel);
        assert!(rolled_lately(app.rolled_at, std::time::Instant::now()), "받은 휠을 안 적었다");
    }

    /// **답이 안 오면 때에서 끊는다** — 답하지 않는 터미널에서 끝내기가 멈추지 않는다. 그동안 온 보고는 버린다.
    #[cfg(unix)]
    #[test]
    fn the_drain_gives_up_when_no_reply_comes() {
        use std::os::fd::AsRawFd;
        let (read, write) = pipe_with(b"\x1b[<65;40;12M\x1b[<64;40;12M");
        let began = std::time::Instant::now();
        assert!(!drain_until_answer(read.as_raw_fd(), began + Duration::from_millis(40)), "답 없이 참을 냈다");
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(40), "때보다 먼저 끊었다 — {took:?}");
        assert!(took < Duration::from_secs(1), "때를 넘겨 기다렸다 — {took:?}");
        drop(write);
        assert_eq!(rest_of(read), b"", "기다리는 동안 온 보고를 남겼다");
    }

    /// **휠·끌기만 몰아 받는다**(리뷰) — 누르기·뗌·키·붙여넣기는 하나씩 받고 그린다. 키는 편집기를 부르거나 끝낼 수 있고,
    /// 누르기는 다음 누르기가 맞힐 화면을 바꾼다.
    #[test]
    fn only_wheel_and_drag_events_roll_together() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};
        let mouse = |kind| Event::Mouse(MouseEvent { kind, column: 3, row: 4, modifiers: KeyModifiers::NONE });
        assert!(rolls(&mouse(MouseEventKind::ScrollDown)));
        assert!(rolls(&mouse(MouseEventKind::ScrollUp)));
        assert!(rolls(&mouse(MouseEventKind::Drag(MouseButton::Left))));
        assert!(!rolls(&mouse(MouseEventKind::Down(MouseButton::Left))));
        assert!(!rolls(&mouse(MouseEventKind::Up(MouseButton::Left))));
        assert!(!rolls(&Event::Key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))));
        assert!(!rolls(&Event::Paste("x".into())));
    }

    /// **편집기가 쥔 문 앞에서 남의 훅은 서고, 쥔 스레드 자신은 안 선다**(moai-46xe). 전역
    /// [`EDITING`] 이 아니라 따로 세운 문으로 본다 — 시험은 한 프로세스에서 나란히 돈다.
    #[test]
    fn a_panic_elsewhere_waits_for_the_editor_but_the_holder_does_not() {
        use std::sync::{Arc, mpsc};
        let gate = Arc::new(Gate::new());
        gate.hold();
        gate.wait(); // 쥔 스레드 — 서면 여기서 시험이 멈춘다
        let (tx, rx) = mpsc::channel();
        let other = Arc::clone(&gate);
        let waiter = std::thread::spawn(move || {
            other.wait();
            tx.send(()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(150)).is_err(), "편집기가 도는데 훅이 지나갔다");
        gate.release();
        rx.recv_timeout(Duration::from_secs(5)).expect("문을 열었는데 훅이 안 깼다");
        waiter.join().unwrap();
        gate.wait(); // 연 문은 누구도 안 세운다
    }

    /// 편집기 시험의 임시 자리. 이름에 **빈칸**을 넣는다 — 경로가 셸에서 쪼개지면 여기서 드러난다.
    /// 만들고 지우는 일(터져도 치우는 것까지)은 [`Scratch`](crate::scratch::Scratch) 가 한다.
    struct Dir(crate::scratch::Scratch);

    impl Dir {
        fn new(name: &str) -> Dir {
            Dir(crate::scratch::Scratch::new(&format!("editor {name}")))
        }

        /// 가짜 편집기 — 받은 인자와 파일의 권한·글을 옆에 적고, `script` 를 돈다. **진짜 편집기는
        /// 안 띄운다.** `"$@"` 의 마지막이 파일이다.
        fn editor(&self, script: &str) -> String {
            let path = self.0.join("fake editor.sh");
            let seen = self.0.join("seen");
            std::fs::write(
                &path,
                format!(
                    "for a in \"$@\"; do f=\"$a\"; done\nprintf '%s\\n' \"$@\" > '{seen}/args'\nstat -c %a \"$f\" > '{seen}/mode'\ncp \"$f\" '{seen}/text'\n{script}\n",
                    seen = seen.display()
                ),
            )
            .unwrap();
            std::fs::create_dir_all(&seen).unwrap();
            // 인자가 붙은 편집기(`code --wait`)와 같은 모양 — 셸 규칙으로 쪼개져야 선다.
            format!("sh '{}' --wait", path.display())
        }

        fn seen(&self, what: &str) -> String {
            std::fs::read_to_string(self.0.join("seen").join(what)).unwrap_or_default()
        }

        /// 편집기가 끝난 뒤 남은 임시 파일.
        fn leftovers(&self) -> Vec<String> {
            std::fs::read_dir(self.0.path())
                .unwrap()
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| n.starts_with("moai-idea-"))
                .collect()
        }
    }

    /// **편집기가 고친 글이 돌아오고, 파일은 남이 못 읽게 만들어졌다가 지워진다.** 편집기
    /// 글의 인자(`--wait`)는 셸 규칙으로 서고, 빈칸 든 경로는 한 인자로 간다.
    #[test]
    fn the_editor_gets_the_template_and_its_edit_comes_back() {
        let d = Dir::new("ok");
        let editor = d.editor("f=\"$2\"; printf '제목\\n\\n본문\\n' >> \"$f\"");
        let got = write_in_editor(&editor, "# 안내\n", &d.0, crate::i18n::Lang::Ko).expect("편집기가 돌려주지 않았다");
        assert_eq!(got, "# 안내\n제목\n\n본문\n");
        assert_eq!(d.seen("text"), "# 안내\n", "편집기가 안내 글을 못 봤다");
        assert_eq!(d.seen("mode").trim(), "600", "임시 파일을 남도 읽게 만들었다");
        let args: Vec<String> = d.seen("args").lines().map(str::to_string).collect();
        assert_eq!(args.len(), 2, "인자가 쪼개졌다 — {args:?}");
        assert_eq!(args[0], "--wait");
        assert!(args[1].starts_with(&d.0.display().to_string()) && args[1].ends_with(".md"), "{args:?}");
        assert_eq!(d.leftovers(), Vec::<String>::new(), "임시 파일이 남았다");
    }

    /// **편집기가 0 이 아닌 코드로 끝나면 글을 안 돌려준다** — 그만두겠다는 뜻이다(vim 의 `:cq`).
    /// 없는 편집기도 같은 길이다(셸이 127). 둘 다 임시 파일은 지운다.
    #[test]
    fn a_failing_editor_gives_a_reason_and_leaves_nothing() {
        let d = Dir::new("fail");
        let editor = d.editor("printf '담길 뻔한 것\\n' > \"$2\"; exit 3");
        let why =
            write_in_editor(&editor, "", &d.0, crate::i18n::Lang::Ko).expect_err("비영으로 끝났는데 글을 돌려줬다");
        assert!(why.contains('3'), "{why}");
        assert_eq!(d.leftovers(), Vec::<String>::new());

        let why = write_in_editor("moai-없는-편집기-08af", "", &d.0, crate::i18n::Lang::Ko)
            .expect_err("없는 편집기인데 글을 돌려줬다");
        assert!(why.contains("127"), "{why}");
        assert_eq!(d.leftovers(), Vec::<String>::new());
    }

    /// **루프가 오류로 끝나며 버릴 뻔한 글은 파일로 남는다**(moai-y3r7). 편집기 글과 같은 모양이라
    /// 그대로 편집기에 열거나 도로 읽을 수 있고, 공유 임시 디렉터리라 남이 못 읽는다(0600).
    /// 부를 때마다 새 이름이다 — 앞서 남긴 것을 덮지 않는다.
    #[test]
    fn unsaved_text_is_rescued_to_a_private_file_in_editor_form() {
        let d = Dir::new("rescue");
        let one = rescue("못 담길 것", Some("## 설계\n둘째 줄"), &d.0).expect("못 남겼다");
        let text = std::fs::read_to_string(&one).unwrap();
        assert_eq!(
            crate::tui::jotfile::parse(&text),
            Some(("못 담길 것".to_string(), Some("## 설계\n둘째 줄".to_string()))),
            "{text:?}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&one).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let two = rescue("제목만", None, &d.0).expect("못 남겼다");
        assert_ne!(one, two, "앞서 남긴 파일을 덮었다");
        assert_eq!(
            crate::tui::jotfile::parse(&std::fs::read_to_string(&two).unwrap()),
            Some(("제목만".to_string(), None))
        );
    }

    /// 임시 파일은 **있는 이름을 안 연다** — 남이 먼저 둔 파일에 적은 생각을 쓰지 않는다.
    #[test]
    fn the_scratch_file_never_reuses_an_existing_name() {
        let d = Dir::new("name");
        let (a, _fa) = scratch_file(&d.0).unwrap();
        // 다음에 고를 이름들을 남이 먼저 둔다 — 카운터만 믿으면 `create_new` 갈래를 한 번도 안 지난다.
        let n: usize = a.to_string_lossy().rsplit('-').next().unwrap().trim_end_matches(".md").parse().unwrap();
        let theirs: Vec<std::path::PathBuf> =
            (n + 1..=n + 8).map(|k| d.0.join(format!("moai-idea-{}-{k}.md", std::process::id()))).collect();
        for p in &theirs {
            std::fs::write(p, "남의 것").unwrap();
        }
        let (b, _fb) = scratch_file(&d.0).unwrap();
        assert_ne!(a, b);
        assert!(!theirs.contains(&b), "남이 둔 이름을 열었다 — {}", b.display());
        for p in &theirs {
            assert_eq!(std::fs::read_to_string(p).unwrap(), "남의 것", "남이 둔 파일을 덮었다");
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
