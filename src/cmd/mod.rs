//! argv 를 코어 호출로 옮기고 낼 줄을 돌려준다.
//!
//! **이슈의 뜻을 판단하는 `if` 를 여기 두지 않는다.** 어떤 이슈가 무엇인지
//! 정하는 코드가 여기 있으면 나중에 TUI 가 그것을 다시 쓴다.

pub mod add;
pub mod defer;
pub mod edit;
pub mod hook;
pub mod idea;
pub mod init;
pub mod link;
pub mod mv;
pub mod note;
pub mod read;
pub mod project;
pub mod ready;
pub mod rm;
pub mod show;
pub mod skill;
pub mod status;
pub mod tui;

use crate::cli::{Cli, Cmd, IdeaCmd, ProjectCmd, SkillCmd, Typed};
use crate::model::Kind;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::fail::{Fail, R, code};

pub struct Ctx {
    pub json: bool,
    /// `--user` 를 **푼 값이 아니라 준 값 그대로** 들고 있다. 여기서 미리
    /// 풀면 읽기만 하는 명령(`show`·`status`)까지 사용자 정보를 요구한다.
    pub user: Option<String>,
    /// `-C` 로 자리를 옮겨 불렀는가. 그러면 부른 사람의 셸은 여기가 아니다 — 부른 자리에서
    /// 도는 명령(`init`)을 일러 줄 때 `-C <뿌리>` 를 붙여야 엉뚱한 자리에 심지 않는다.
    pub chdir: bool,
}

/// 할 수 있는 것은 다 하고, 된 것과 안 된 것을 둘 다 보고한 뒤 비영 종료한다.
/// 이 깃발이 서면 결과를 다 낸 **뒤에** 종료 코드가 1 이 된다.
static PARTIAL: AtomicBool = AtomicBool::new(false);

/// `none` 은 "비운다" 는 뜻이다. 제목이 `none` 인 이슈를 만들 일은 없다.
/// `add` 와 `edit` 이 같은 낱말을 써야 한다 — 한쪽만 알면 방금 만든 이슈를
/// 같은 말로 비우지 못한다.
///
/// **`none` 만 본다.** 빈 값이나 앞뒤 공백까지 여기서 접으면 `-e ""` 가 거절에서
/// 비우기로, `-e " <id> "` 가 거절에서 통과로 조용히 바뀐다 — 담당 때문에 옮긴
/// 헬퍼가 에픽·마일스톤의 뜻을 같이 바꾸는 것은 범위 밖이다. 담당 쪽 공백은
/// `model::split_assignee` 가 접는다.
pub fn clearable(v: &str) -> Option<String> {
    (v != "none").then(|| v.to_string())
}

pub fn note_partial() {
    PARTIAL.store(true, Ordering::Relaxed);
}
pub fn had_partial() -> bool {
    PARTIAL.load(Ordering::Relaxed)
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리를 겹친다(`worktree::gather`).
///
/// **남의 워크트리에서 만난 문제는 말만 한다** — stderr 로 한 줄씩, 부분 실패
/// 깃발은 안 세운다. 옆 워크트리의 깨진 줄로 `moai status` 가 비영 종료하면
/// 제 파일은 멀쩡한데 도구가 실패로 읽힌다. 제 파일의 못 읽는 줄은 전과 같이
/// `load.errors` 에 남아 부르는 쪽이 제 길로 알린다.
pub fn gather(repo: &crate::store::Repo, worktree: bool) -> R<crate::worktree::Gathered> {
    let g = crate::worktree::gather(repo, worktree)?;
    for t in g.unfound.iter().chain(&g.trouble) {
        eprintln!("{t}");
    }
    Ok(g)
}

/// `.moai` 밖에서 부른 `ready` 가 볼 등록한 프로젝트. 프로젝트마다 연다.
///
/// **등록한 것이 없으면 실패한다** — 대신 등록하는 길을 곁에 댄다. `status` 는 이것을
/// 안 지나고 같은 말을 내며 0 으로 끝난다(moai-ynsb): 세션의 시작점이 제 파일 아닌 것으로
/// 실패해 보이면 안 된다.
///
/// 사용자 설정의 문제는 등록한 것이 있을 때는 화면이 한 줄씩 비추고(`problems`),
/// 없을 때는 실패 말에 붙는다 — 목록이 빈 까닭이 그것일 수 있다.
///
/// `worktree` 면 프로젝트마다 옆 워크트리를 겹친다. 옆에서 만난 문제는 stderr 가 아니라
/// 그 프로젝트의 줄(`Project::trouble`)이 말한다 — 여럿을 한 번에 보는 화면에서 stderr 의
/// 한 줄은 어느 프로젝트의 것인지 모른다.
pub fn registered(worktree: bool) -> R<(crate::user_config::Registry, Vec<crate::projects::Project>)> {
    let reg = crate::user_config::read(crate::user_config::path().as_deref());
    if reg.projects.is_empty() {
        return Err(nothing_registered(&reg));
    }
    let projects = crate::projects::open_with(&reg, worktree);
    Ok((reg, projects))
}

/// `.moai` 밖인데 등록한 것도 없을 때의 말 — `ready` 는 이 말로 멈추고, `status` 는
/// 같은 말을 내고 0 으로 끝난다(moai-ynsb).
/// 화면의 `tui` 는 멈추지 않고 빈 층에서 `SPC p a` 를 댄다(moai-r8kl).
/// 목록이 빈 까닭이 사용자 설정의 문제일 수 있어 그것도 붙인다.
pub fn nothing_registered(reg: &crate::user_config::Registry) -> Fail {
    let mut msg = format!(
        "{}\n등록한 프로젝트도 없다 — `moai project add <dir>` 로 더하면 `.moai` 밖에서 한눈에 본다",
        crate::store::NOT_A_REPO
    );
    // 사람의 설정 파일에서 온 글이다 — 제어문자를 걷고 한 줄로 접는다. 이 말은 줄 단위로
    // 읽히므로(`fail` 이 그대로 stderr 에 쓴다) 여러 줄이 섞이면 어디까지가 한 까닭인지 흐려진다.
    for p in &reg.problems {
        msg.push_str(&format!("\n{}", crate::text::one_line(p)));
    }
    Fail::new(msg)
}

/// 읽다 만난 잘못된 줄을 stderr 로 알린다. 결과는 그대로 낸다.
pub fn report_load_errors(path: &std::path::Path, errors: &[crate::store::LoadError]) {
    if errors.is_empty() {
        return;
    }
    note_partial();
    name_load_errors(path, errors);
}

/// 같은 말을 하되 **부분 실패 깃발은 안 세운다.** 읽기가 답을 덜 낸 자리
/// (`show`·`ready`)는 비영 종료가 맞지만, 쓰기와 짝을 이루는 자리(`promote`
/// 의 연습)는 진짜 실행이 그 줄 때문에 멈추지 않으므로 연습만 실패로 끝나면
/// 안 된다 — 그것을 거절로 읽은 쪽은 도구가 기꺼이 해 줄 계획을 버린다.
pub fn name_load_errors(path: &std::path::Path, errors: &[crate::store::LoadError]) {
    if errors.is_empty() {
        return;
    }
    eprintln!(
        "{}: 읽을 수 없는 줄 {}개",
        path.display(),
        errors.len()
    );
    for e in errors.iter().take(5) {
        eprintln!("  {}줄: {}", e.line, e.message);
    }
    if errors.len() > 5 {
        eprintln!("  … {}개 더", errors.len() - 5);
    }
}

pub fn run(cli: Cli) -> R<Vec<String>> {
    let ctx = Ctx { json: cli.json, user: cli.user, chdir: cli.dir.is_some() };
    let Some(cmd) = cli.cmd else {
        return opening(&ctx);
    };
    match cmd {
        // 새 명령을 두지 않고 `init` 의 플래그로 둔다 — 고치는 길(`init`)과 보는 길이 한 이름에 있어야
        // `stale` 을 본 사람이 무엇을 칠지 안다(moai-mstm).
        Cmd::Init { check: true, .. } => init::check(&ctx),
        // 필드를 다 적는다 — `..` 로 받으면 `init` 에 새 플래그를 더해도 여기서 조용히 버려진다.
        Cmd::Init { prefix, no_agents, check: false } => init::run(&ctx, prefix.as_deref(), no_agents),
        Cmd::Hook { event } => hook::run(&ctx, event),
        Cmd::Skill(SkillCmd::Install { scope, dry_run }) => {
            skill::install(&ctx, scope.as_str(), dry_run)
        }
        Cmd::Skill(SkillCmd::Status) => skill::status(&ctx),
        Cmd::Skill(SkillCmd::Uninstall { dry_run }) => skill::uninstall(&ctx, dry_run),
        // 저장소가 아니라 사람의 설정을 고친다 — `Repo::discover` 를 안 지나므로
        // `.moai` 밖에서도 선다.
        Cmd::Project(ProjectCmd::Add { path }) => project::add(&ctx, &path),
        Cmd::Project(ProjectCmd::Ls) => project::ls(&ctx),
        Cmd::Project(ProjectCmd::Rm { path }) => project::rm(&ctx, &path),
        Cmd::Project(ProjectCmd::Color { path, hue }) => project::color(&ctx, &path, &hue),
        Cmd::Add(a) => add::run(&ctx, a, None),
        Cmd::Show(a) => show::run(&ctx, a, None),
        Cmd::Mv(a) => mv::run(&ctx, a),
        Cmd::Edit(a) => edit::run(&ctx, a),
        Cmd::Rm(a) => rm::run(&ctx, a),
        Cmd::Note(a) => note::run(&ctx, a),
        Cmd::Link(a) => link::run(&ctx, a),
        Cmd::Defer(a) => defer::run(&ctx, a),
        Cmd::Read(a) => read::run(&ctx, a),
        Cmd::Ready(w) => ready::run(&ctx, w.worktree),
        Cmd::Status(w) => status::run(&ctx, w.worktree),
        Cmd::Tui(a) => tui::run(&ctx, a),
        Cmd::Issue(t) => typed(&ctx, t, Kind::Issue),
        Cmd::Epic(t) => typed(&ctx, t, Kind::Epic),
        Cmd::Milestone(t) => typed(&ctx, t, Kind::Milestone),
        Cmd::Idea(IdeaCmd::Add(a)) => add::run(&ctx, a, Some(Kind::Idea)),
        Cmd::Idea(IdeaCmd::Show(a)) => show::run(&ctx, a, Some(Kind::Idea)),
        Cmd::Idea(IdeaCmd::Promote(a)) => idea::promote(&ctx, a),
    }
}

/// 인자 없이 불렀을 때. **오류가 아니다.**
///
/// 저장소 안이면 `status` — 세션의 시작점이 그것이고, 그래서 `bd prime` 같은
/// 명령을 따로 두지 않았다. 밖이면 도움말과 `init` 안내.
///
/// 처음 만난 쪽이 알아야 할 것은 둘이다: 지금 무슨 상태인가, 다음에 무엇을
/// 치는가. 둘 다 여기서 준다.
fn opening(ctx: &Ctx) -> R<Vec<String>> {
    use clap::CommandFactory;
    let found = crate::store::Repo::find();
    // `.moai` 밖이어도 등록한 프로젝트가 있으면 한눈 보기가 곧 시작점이다 (`status` 가
    // 그 길로 간다).
    //
    // **설정이 깨진 저장소 안(`Err`)은 도움말이 아니다** — 아래 `status` 로 가서 다른 명령과
    // 같은 말·같은 종료 코드로 그 설정을 댄다. 도움말로 접으면 "아직 moai 저장소가 아니다"
    // 를 믿은 사람이 제 저장소에 `init` 을 다시 친다 (moai-byih).
    let outside = matches!(found, Ok(None));
    let reg = outside.then(|| crate::user_config::read(crate::user_config::path().as_deref()));
    let registered = reg.as_ref().is_some_and(|r| !r.projects.is_empty());
    if outside && !registered {
        let mut help = Vec::new();
        crate::cli::Cli::command()
            .write_help(&mut help)
            .map_err(|e| Fail::new(e.to_string()))?;
        let mut out: Vec<String> =
            String::from_utf8_lossy(&help).lines().map(str::to_string).collect();
        out.push(String::new());
        out.push("여기는 아직 moai 저장소가 아니다 — `moai init` 으로 시작한다".into());
        out.push("다른 곳의 프로젝트를 여기서 한눈에 보려면 `moai project add <dir>` 로 등록한다".into());
        // **목록이 빈 까닭이 설정의 문제면 그것을 댄다.** 세션은 여기서 시작하는데, 설정이
        // 깨져 등록한 것이 안 읽힌 사람에게 "등록한 것이 없다, 더하라" 만 하면 정반대를
        // 믿고 깨진 파일에 `project add` 를 친다. `status`·`ready` 는 이미 이 줄을 대고
        // (`nothing_registered`), `tui --json` 은 `problems` 에 싣는다. 도움말 자리라 종료 코드는 그대로 0 이다.
        for p in reg.iter().flat_map(|r| &r.problems) {
            out.push(crate::style::paint(crate::style::WARN, &format!("! {}", crate::text::one_line(p))));
        }
        return Ok(out);
    }

    let mut out = status::run(ctx, false)?;
    if ctx.json {
        return Ok(out);
    }
    out.push(String::new());
    let here = !registered && std::path::Path::new("AGENTS.md").exists();
    out.push(crate::style::paint(
        crate::style::DIM,
        if here {
            "명령: `moai --help`   ·   이 저장소에서 일하는 법: AGENTS.md"
        } else {
            "명령: `moai --help`"
        },
    ));
    Ok(out)
}

fn typed(ctx: &Ctx, cmd: Typed, kind: Kind) -> R<Vec<String>> {
    match cmd {
        Typed::Add(a) => add::run(ctx, a, Some(kind)),
        Typed::Show(a) => show::run(ctx, a, Some(kind)),
    }
}

/// 제목 자리에 온 것이 사실은 오타 난 플래그인가.
///
/// `allow_hyphen_values` 는 모르는 하이픈 토큰을 전부 제목으로 삼킨다.
/// `--json 이 tags 를 빠뜨린다` 같은 제목을 받으려고 켠 것인데, 그 대가로
/// `moai add --dryrun` 이 제목 `"--dryrun"` 인 이슈를 조용히 만든다.
///
/// **띄어쓰기가 가른다.** 사람이 쓰는 제목은 낱말이 여럿이고, 오타 난
/// 플래그는 한 낱말이다. 정말 그 제목을 쓰겠다면 `--` 로 넘긴다.
pub fn refuse_if_flag_like(title: &str) -> R<()> {
    // `--` 를 쓴 사람은 "이 뒤는 플래그가 아니다" 라고 이미 말한 것이다.
    //
    // argv 를 다시 훑는 것이 `--json` 때는 틀렸지만 여기서는 맞다 — `--` 는
    // 값이 아니라 구분자라 clap 이 언제나 삼키고, argv 에 남아 있다는 것은
    // 사용자가 그것을 적었다는 뜻 말고 다른 뜻이 없다.
    if std::env::args().any(|a| a == "--") {
        return Ok(());
    }
    if title.starts_with("--") && !title.contains(char::is_whitespace) {
        return Err(Fail::coded(
            format!(
                "`{title}` 은 제목이 아니라 플래그로 보인다.\n                       정말 제목이면 `--` 뒤에 둔다 — `moai add -- {title}`"
            ),
            code::BAD_INPUT,
        ));
    }
    Ok(())
}

/// `--json` 일 때 한 줄로 낸다.
///
/// **`serde_json::Value` 를 거치지 않는다.** `Value` 의 맵은 정렬돼 있어
/// 키가 알파벳 순으로 재배열되고, 그러면 파일과 `--json` 이 서로 다른 순서를
/// 말한다. 눈으로 훑을 때 `id` 가 줄 가운데에 있는 것도 그 탓이다.
pub fn json_line<T: serde::Serialize>(v: &T) -> R<Vec<String>> {
    serde_json::to_string(v)
        .map(|s| vec![s])
        .map_err(|e| Fail::new(e.to_string()))
}

/// 이슈 한 줄의 기계 출력. 묶음이면 **멤버에서 읽은 칸**을 `derived_status` 로
/// 곁들인다 (`report::group_states`).
///
/// `status` 의 뜻은 안 바꾼다 — 파일에 적힌 값 그대로다. 이미 나간 계약이라,
/// 그 키를 읽은 칸으로 바꾸면 `--json` 을 읽고 되쓰는 쪽이 읽은 값을 적힌
/// 값으로 믿는다. 사람 화면이 `-s` 로 고르고 그리는 칸이 이 키다.
///
/// **줄을 내는 모든 명령이 이것을 지난다.** `show` 만 곁들이면 `add`·`defer`·`link` 를
/// 읽는 쪽은 같은 에픽을 적힌 칸으로 읽는다 — 키가 없다는 것이 "묶음이 아니다" 라는 뜻이다.
#[derive(serde::Serialize)]
pub struct Row<'a> {
    #[serde(flatten)]
    pub issue: std::borrow::Cow<'a, crate::model::Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_status: Option<&'a str>,
    /// 다른 워크트리에서 온 줄이면 그 브랜치 (`--worktree`). **키가 없다는 것이 곧
    /// "지금 브랜치의 줄" 이다** — `derived_status` 와 같은 약속이다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<&'a str>,
}

/// **줄 하나의 `--json` 에 moai 가 덧붙이는 키 전부**(moai-qn5d) — 늘 붙이는 것도, 조건에 따라
/// 붙이는 것도, 어느 명령이 붙이는 것도. 이 이름의 모르는 필드는 [`Row::of`] 가 걷는다.
///
/// **한 목록이다.** 명령마다 목록을 두면 걷기가 그 명령에만 선다 — `show <id> --json` 만 걷던
/// 판에는 되쓴 줄을 `ready`·`show` 목록·`edit` 가 그대로 펴서, 막음 없는 일에 옛 `blockers` 가,
/// 멀쩡한 git 옆에 옛 `commits_error` 가 섰다. 키를 더하면 여기에 더한다 — [`json_with`] 가
/// 시험 빌드에서 빠진 이름을 대며 멈춘다(`show` 가 이 목록을 `may` 로 준다).
///
/// 사용자가 같은 이름으로 둔 제 필드도 **줄 출력에서** 숨는다 — 받은 값이다(2026-09-18 사용자
/// 결정). 모르는 필드 보존은 파일의 약속이고, 출력의 약속은 우리 키가 참이라는 것이다. 파일은
/// 그대로다(`moai status` 가 비춘다).
pub const OURS: &[&str] = &[
    // `Row` 가 제 필드로 곁들이는 것.
    "derived_status",
    "branch",
    // `show <id> --json` 이 덧붙이는 것(`json_with`).
    "children",
    "journal",
    "members",
    "shelved_by",
    "duplicate_lines",
    "blockers",
    "workplaces",
    "place",
    "commits",
    "commits_error",
    "work",
    // `edit --json` 이 곁들이는 남은 소속.
    "inherited_epic",
    "inherited_milestone",
];

impl<'a> Row<'a> {
    /// `read` 는 그 줄의 읽은 칸이다. **묶음이 아니면 버린다** — 머지를 잘못 푼
    /// 파일에서 묶음과 id 가 같은 일 줄이 그 묶음의 칸을 입지 않게, `report::column`
    /// 과 같은 자로 묻는다.
    pub fn of(issue: &'a crate::model::Issue, read: Option<&'a str>) -> Row<'a> {
        // **이 키들은 우리 것이다**([`OURS`]). `--json` 을 파일에 되써 넣어 그 이름을 모르는
        // 필드로 든 줄이면 화면에서 걷어낸다 — 그대로 두면 한 객체에 같은 키가 둘 서서 깐깐한
        // 파서가 거절하고, 이번에 안 실은 조건부 키는 그 조건이 아닌 지금 옛 값을 말한다.
        // 흔한 길(겹치는 것이 없다)에서는 줄을 복제하지 않는다.
        let issue = match OURS.iter().any(|k| issue.rest.contains_key(*k)) {
            false => std::borrow::Cow::Borrowed(issue),
            true => {
                let mut own = issue.clone();
                own.rest.retain(|k, _| !OURS.contains(&k.as_str()));
                std::borrow::Cow::Owned(own)
            }
        };
        let derived = read.filter(|_| crate::report::is_group(&issue));
        Row { issue, derived_status: derived, branch: None }
    }

    /// 겹쳐 본 줄이면 그 출처를 곁들인다.
    pub fn on(mut self, origin: &'a crate::worktree::Origin) -> Row<'a> {
        self.branch = origin.branch(&self.issue.id);
        self
    }

    /// 락 안에서 챙겨 온 지도(`read_of`)로 짓는다.
    pub fn from(issue: &'a crate::model::Issue, read: &'a Read) -> Row<'a> {
        Row::of(issue, read.get(&issue.id).map(String::as_str))
    }
}

/// 묶음 id → 멤버에서 읽은 칸. 락 밖으로 들고 나가는 모양이라 제 문자열을 쥔다.
pub type Read = BTreeMap<String, String>;

/// 락 안에서 **낼 줄 가운데 묶음의 읽은 칸**을 챙겨 나온다 — 락을 놓은 뒤에 다시 세면
/// 그 사이에 남이 쓴 멤버가 섞인다. 묶음이 없으면 걷지 않는다(`group_states_of`).
pub fn read_of(issues: &[crate::model::Issue], cfg: &crate::config::Config, ids: &[&str]) -> Read {
    crate::report::group_states_of(issues, cfg, ids)
        .into_iter()
        .map(|(id, col)| (id.to_string(), col.to_string()))
        .collect()
}

/// `--from` 이 받는 칸 — **아는 칸이거나, 어느 줄이 실제로 서 있는 칸**(moai-hym7).
///
/// 오타는 그대로 거절한다. 거절이 노리는 것은 오타지 낡음이 아니다 — `config` 에서 칸
/// 이름을 하나 고치면 옛 이름에 선 줄이 남는데, 그 이름을 오타로 읽어 막으면 그 줄은
/// **영영** `--from` 으로 못 집는다. "남의 낡은 줄 하나가 모든 쓰기를 막으면 되돌릴
/// 방법이 도구 밖에만 남는다"(CLAUDE.md)와 같은 자리고, `store::with_write` 가 파일
/// 전체가 아니라 **바뀐 줄만** 검사하는 것과도 같은 자다.
///
/// 줄을 봐야 하므로 락 안에서 잰다 — 밖에서 재면 그 사이 마지막 줄이 그 칸을 떠난다.
pub fn check_from(from: Option<&str>, issues: &[crate::model::Issue], cfg: &crate::config::Config) -> R<()> {
    // 아는가를 가르는 것은 `report` 다 — 읽는 쪽(`show -s`·탐색기 필터)과 **같은 술어**를
    // 써야 옮길 수는 있는데 못 찾는 줄이 안 생긴다.
    let Some(f) = from.filter(|f| !crate::report::knows_column(issues, cfg, f)) else { return Ok(()) };
    Err(Fail::coded(unknown_column(f, cfg), code::BAD_STATUS))
}

/// 모르는 칸을 댈 때의 한 줄 — 쓰기도 읽기도 같은 말을 한다.
pub fn unknown_column(name: &str, cfg: &crate::config::Config) -> String {
    format!("`{name}` 라는 칸이 없고 거기 선 줄도 없다. 있는 칸: {}", cfg.statuses.join(", "))
}

/// `--from` 이 견줄 **서 있는 칸** — 물은 줄마다 하나씩, 락 안에서 **한 번** 뜬다.
///
/// [`read_of`] 와 갈리는 곳이 둘이다.
///
/// - **묶음만이 아니라 물은 줄 전부**를 담는다. 묶음인지는 [`crate::report::column`] 이
///   가른다 — 지도를 id 로만 짚던 판은 그 갈림을 혼자 안 지켰다. 머지를 잘못 푼
///   파일에서 묶음과 id 가 같은 일 줄이 그 묶음의 칸을 입는 자리라, `report::column`
///   과 `Row::of` 가 같은 곳에서 같은 `if` 를 쓴다(report.rs 의 "묶음을 가르는 `if`
///   가 표면마다 있으면 하나는 반드시 빠진다").
/// - **한 번만 뜨는 것이 곧 뜻이다.** 돌면서 그때그때 `i.status` 를 보면 같은 id 를
///   두 번 적은 한 명령이 **제가 방금 쓴 값**과 겨룬다 — 옮겨 놓고도 "이미 …다" 로
///   지고, `moved` 와 `stale` 에 같은 줄이 함께 서며, 종료 코드가 0 이 아니다.
///   `--from` 이 재는 것은 *부르는 쪽이 본* 칸이지 이 명령이 만든 칸이 아니다.
pub fn standing_of(issues: &[crate::model::Issue], cfg: &crate::config::Config, ids: &[&str]) -> Read {
    let states = crate::report::group_states_of(issues, cfg, ids);
    issues
        .iter()
        .filter(|i| ids.contains(&i.id.as_str()))
        .map(|i| (i.id.clone(), crate::report::column(i, &states).to_string()))
        .collect()
}

/// 객체 하나에 필드를 덧붙여 낸다. 선언 순서를 지키려면 직렬화된 뒤에
/// 붙이는 수밖에 없다 — 중간에 `Value` 를 쓰면 순서가 사라진다.
///
/// **덧붙인 키가 이긴다**(moai-kgu2, 사용자와 정함). 줄이 모르는 필드로 같은 이름을 들고 있으면
/// (`--json` 을 파일에 되써 넣은 줄) 한 객체에 같은 키가 둘 선다. 줄이면 그 이름은 [`Row::of`] 가
/// 이미 걷었다 — 이번에 안 실은 조건부 키까지(moai-2l8n). 필드가 선언된 것뿐인 객체
/// (`StatusReport`)는 걷을 것이 없다. 여기서는 걷지 않는다.
///
/// **`may` 는 걷는 쪽이 아는 이름이다** — 줄이면 [`OURS`], 선언된 객체면 그 명령이 덧붙이는 키.
/// `extra` 의 키는 모두 거기 있어야 한다 — 시험 빌드에서 확인한다. 목록이 덧붙이는 자리와 떨어져
/// 있어, 새 키를 더하고 목록을 잊으면 그 키만 moai-2l8n 이 되살아나는데 그것을 잡을 시험이 따로
/// 없다. 그 키가 한 번이라도 실리는 시험이 여기서 붉어진다.
pub fn json_with<T: serde::Serialize>(base: &T, extra: &[(&str, String)], may: &[&str]) -> R<Vec<String>> {
    debug_assert!(
        extra.iter().all(|(k, _)| may.contains(k)),
        "덧붙인 키가 `may` 에 없다 — 되써 넣은 줄에서 그 키를 못 걷는다: {:?}",
        extra.iter().map(|(k, _)| *k).filter(|k| !may.contains(k)).collect::<Vec<_>>()
    );
    let mut s = serde_json::to_string(base).map_err(|e| Fail::new(e.to_string()))?;
    if !s.ends_with('}') {
        return Err(Fail::new("객체가 아니다"));
    }
    let empty = s == "{}";
    s.pop();
    for (i, (k, v)) in extra.iter().enumerate() {
        if !(empty && i == 0) {
            s.push(',');
        }
        s.push_str(&format!("\"{k}\":{v}"));
    }
    s.push('}');
    Ok(vec![s])
}

/// 옮기거나 도로 집었어도 **계획 밖인 줄**과, 그것을 실제로 뺀 줄.
///
/// `mv`·`defer` 의 기계 출력이 같은 모양으로 낸다. 사람 출력은 그 줄에 도로 집을
/// 말을 붙이는데 기계 출력에만 없으면, `--json` 을 읽는 에이전트는 방금 집은 일이
/// 왜 훅의 초점에서 빠졌는지 알 길이 없다.
#[derive(serde::Serialize)]
pub struct Shelved<'a> {
    pub id: &'a str,
    /// 가장 가까운 미룬 곳.
    pub root: &'a str,
    /// 도로 집어야 할 곳 **전부**, 가까운 것부터(moai-g2a1). 하나뿐이면 `root` 와 같아
    /// 안 낸다 — 흔한 경우의 출력을 바꾸지 않는다.
    #[serde(skip_serializing_if = "one_or_none")]
    pub roots: &'a [String],
}

fn one_or_none(roots: &&[String]) -> bool {
    roots.len() <= 1
}

/// `--from` 에 걸려 **손대지 않은 줄**과, 락 안에서 본 그 줄의 지금 칸.
///
/// [`Shelved`] 와 같은 까닭으로 여기 하나다 — `mv` 와 `defer` 의 기계 출력이 같은
/// 모양으로 낸다. 두 곳에 따로 두면 키를 하나 더할 때 한쪽만 늘어, 두 명령을 한
/// 파서로 읽는 쪽이 한쪽에서만 깨진다.
#[derive(serde::Serialize)]
pub struct Stale<'a> {
    pub id: &'a str,
    /// 락 안에서 본 **서 있는 칸** — `--from` 이 견준 그 값이다. 함께 주지 않으면 진
    /// 쪽이 한 번 더 물어야 한다.
    ///
    /// **[`Row`] 의 `status` 와 뜻이 다르다.** 저쪽은 파일에 적힌 값이고 이쪽은 묶음이면
    /// 멤버에서 읽은 칸이다(`standing_of`) — 같은 이름이라 되쓰는 쪽이 파생값을 적힌
    /// 값으로 믿을 수 있다. 여기 이름을 `status` 로 둔 것은 이 값이 그대로 다음 `--from`
    /// 의 인자이기 때문이고, 묶음의 두 칸이 갈리는 곳은 `reference` 가 적어 둔다.
    pub status: &'a str,
}

/// (줄, 락 안에서 본 지금 칸).
pub fn stale(rows: &[(String, String)]) -> Vec<Stale<'_>> {
    rows.iter().map(|(id, status)| Stale { id, status }).collect()
}

/// (줄, 풀어야 할 미룸 전부 — 가까운 것부터).
pub fn shelved(pairs: &[(String, Vec<String>)]) -> Vec<Shelved<'_>> {
    pairs
        .iter()
        .map(|(id, roots)| Shelved { id, root: roots.first().map_or("", String::as_str), roots })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, Kind, Status};

    fn row_with(rest: &[(&str, &str)]) -> Issue {
        let mut i = Issue::new("argos-0001".into(), "제목".into(), Kind::Epic, Status::new("todo"), "2026-09-11T04:12:03Z");
        for (k, v) in rest {
            i.rest.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        i
    }

    /// **덧붙인 키가 이긴다**(moai-kgu2) — `show` 가 덧붙이는 키 전부에 같은 자로 선다. 겹치지
    /// 않는 모르는 필드와 곁들인 `derived_status` 는 그대로 남는다.
    #[test]
    fn every_appended_key_wins_over_an_unknown_field_and_nothing_else_is_lost() {
        let i = row_with(&[("members", "가짜"), ("shelved_by", "가짜"), ("duplicate_lines", "가짜"), ("due", "2026-10-01")]);
        let row = Row::of(&i, Some("in_progress"));
        let extra = [
            ("members", "[]".to_string()),
            ("shelved_by", "\"argos-0002\"".to_string()),
            ("duplicate_lines", "2".to_string()),
        ];
        let out = json_with(&row, &extra, OURS).unwrap().join("");
        for (k, v) in &extra {
            assert_eq!(out.matches(&format!("\"{k}\":")).count(), 1, "{k} 가 둘 섰다\n{out}");
            assert!(out.contains(&format!("\"{k}\":{v}")), "{k} 에 우리 값이 안 섰다\n{out}");
        }
        assert!(!out.contains("가짜"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\""), "겹치지 않는 모르는 필드까지 걷었다\n{out}");
        assert!(out.contains("\"derived_status\":\"in_progress\""), "{out}");
    }

    /// **이번에 안 실은 조건부 키도 걷는다**(moai-2l8n) — `OURS` 에 든 이름이면 `extra` 에 없어도
    /// 되써 넣은 옛 값이 안 나간다. `OURS` 에 없는 모르는 필드는 그대로다.
    #[test]
    fn a_conditional_key_left_out_this_time_is_stripped_too() {
        let i = row_with(&[("commits_error", "가짜"), ("due", "2026-10-01")]);
        let row = Row::of(&i, None);
        let out = json_with(&row, &[("commits", "[]".to_string())], OURS).unwrap().join("");
        assert!(!out.contains("commits_error"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\"") && out.contains("\"commits\":[]"), "{out}");
    }

    /// **덧붙이지 않는 명령도 같은 목록으로 걷는다**(moai-qn5d) — 걷는 자리가 `Row::of` 라
    /// `ready`·`add`·`mv` 처럼 `json_line` 으로 줄을 펴는 명령에도 옛 값이 안 나간다.
    #[test]
    fn a_plain_row_drops_every_key_moai_appends() {
        let mut rest: Vec<(&str, &str)> = OURS.iter().map(|k| (*k, "가짜")).collect();
        rest.push(("due", "2026-10-01"));
        let i = row_with(&rest);
        let out = json_line(&Row::of(&i, None)).unwrap().join("");
        assert!(!out.contains("가짜"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\""), "{out}");
        assert_eq!(i.rest.len(), OURS.len() + 1, "출력에서 걷으려다 줄을 바꿨다");
    }

    /// **`may` 에 안 적은 키를 덧붙이면 시험 빌드가 멈춘다** — 목록이 덧붙이는 자리와 떨어져 있어
    /// 새 키를 더하고 목록을 잊는 것을 잡을 곳이 여기뿐이다(moai-2l8n).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "may")]
    fn an_appended_key_missing_from_may_is_caught() {
        let i = row_with(&[]);
        let _ = json_with(&Row::of(&i, None), &[("새_키", "[]".to_string())], OURS);
    }

    /// 겹치는 것이 없으면 걷은 모습을 짓지 않는다 — 흔한 길에서 줄을 복제하지 않는다.
    #[test]
    fn nothing_to_strip_means_no_copy() {
        let i = row_with(&[("due", "2026-10-01")]);
        assert!(matches!(Row::of(&i, None).issue, std::borrow::Cow::Borrowed(_)));
    }
}
