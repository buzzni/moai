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
pub mod ready;
pub mod rm;
pub mod show;
pub mod skill;
pub mod status;
pub mod tui;

use crate::cli::{Cli, Cmd, IdeaCmd, SkillCmd, Typed};
use crate::model::Kind;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::fail::{Fail, R, code};

pub struct Ctx {
    pub json: bool,
    /// `--user` 를 **푼 값이 아니라 준 값 그대로** 들고 있다. 여기서 미리
    /// 풀면 읽기만 하는 명령(`show`·`status`)까지 사용자 정보를 요구한다.
    pub user: Option<String>,
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
    let ctx = Ctx { json: cli.json, user: cli.user };
    let Some(cmd) = cli.cmd else {
        return opening(&ctx);
    };
    match cmd {
        Cmd::Init { prefix, no_agents } => init::run(&ctx, prefix.as_deref(), no_agents),
        Cmd::Hook { event } => hook::run(&ctx, event),
        Cmd::Skill(SkillCmd::Install { scope, dry_run }) => {
            skill::install(&ctx, scope.as_str(), dry_run)
        }
        Cmd::Skill(SkillCmd::Status) => skill::status(&ctx),
        Cmd::Skill(SkillCmd::Uninstall { dry_run }) => skill::uninstall(&ctx, dry_run),
        Cmd::Add(a) => add::run(&ctx, a, None),
        Cmd::Show(a) => show::run(&ctx, a, None),
        Cmd::Mv(a) => mv::run(&ctx, a),
        Cmd::Edit(a) => edit::run(&ctx, a),
        Cmd::Rm(a) => rm::run(&ctx, a),
        Cmd::Note(a) => note::run(&ctx, a),
        Cmd::Link(a) => link::run(&ctx, a),
        Cmd::Defer(a) => defer::run(&ctx, a),
        Cmd::Ready => ready::run(&ctx),
        Cmd::Status => status::run(&ctx),
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
    if crate::store::Repo::discover().is_err() {
        let mut help = Vec::new();
        crate::cli::Cli::command()
            .write_help(&mut help)
            .map_err(|e| Fail::new(e.to_string()))?;
        let mut out: Vec<String> =
            String::from_utf8_lossy(&help).lines().map(str::to_string).collect();
        out.push(String::new());
        out.push("여기는 아직 moai 저장소가 아니다 — `moai init` 으로 시작한다".into());
        return Ok(out);
    }

    let mut out = status::run(ctx)?;
    if ctx.json {
        return Ok(out);
    }
    out.push(String::new());
    let here = std::path::Path::new("AGENTS.md").exists();
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
}

/// 읽은 칸의 키. 모르는 필드로 같은 이름을 든 줄을 가려내는 데도 쓴다.
const DERIVED: &str = "derived_status";

impl<'a> Row<'a> {
    /// `read` 는 그 줄의 읽은 칸이다. **묶음이 아니면 버린다** — 머지를 잘못 푼
    /// 파일에서 묶음과 id 가 같은 일 줄이 그 묶음의 칸을 입지 않게, `report::column`
    /// 과 같은 자로 묻는다.
    pub fn of(issue: &'a crate::model::Issue, read: Option<&'a str>) -> Row<'a> {
        // **이 키는 우리 것이다.** `--json` 을 파일에 되써 넣어 `derived_status` 를
        // 모르는 필드로 든 줄이면 그 값을 화면에서 걷어낸다 — 그대로 두면 한 객체에
        // 같은 키가 둘 서서 깐깐한 파서가 거절하고, 묶음 아닌 줄에는 읽은 칸이
        // 있는 것처럼 보인다. 파일의 값은 그대로 둔다(`moai status` 가 비춘다).
        let issue = match issue.rest.contains_key(DERIVED) {
            false => std::borrow::Cow::Borrowed(issue),
            true => {
                let mut own = issue.clone();
                own.rest.remove(DERIVED);
                std::borrow::Cow::Owned(own)
            }
        };
        let derived = read.filter(|_| crate::report::is_group(&issue));
        Row { issue, derived_status: derived }
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

/// 객체 하나에 필드를 덧붙여 낸다. 선언 순서를 지키려면 직렬화된 뒤에
/// 붙이는 수밖에 없다 — 중간에 `Value` 를 쓰면 순서가 사라진다.
pub fn json_with<T: serde::Serialize>(base: &T, extra: &[(&str, String)]) -> R<Vec<String>> {
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
    pub root: &'a str,
}

pub fn shelved(pairs: &[(String, String)]) -> Vec<Shelved<'_>> {
    pairs.iter().map(|(id, root)| Shelved { id, root }).collect()
}
