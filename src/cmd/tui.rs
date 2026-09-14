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
    // 탐색기는 옆 워크트리를 겹친 채로 연다(`App::worktree`). `--json` 은 겹치지 않는다 —
    // 기계로 읽는 쪽의 출력 모양은 `status`·`ready`·`show` 처럼 `--worktree` 없이 그대로다.
    let crate::worktree::Gathered { load, origin, trouble, watched } = crate::worktree::gather(&repo, !ctx.json)?;
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
    let mut app = App::open(repo, load, index, path, stamp).overlaid(origin, trouble, watched).attach_layer(layer);
    app.user = ctx.user.clone();
    // 층이 없어도 `a` 로 첫 등록을 한다 — 그때 쓸 설정 자리와 고르기 창이 처음 열 자리(moai-plvy).
    app.user_config = config;
    app.launched_at = std::env::current_dir().ok();
    app.editor = editor();
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
    app.editor = editor();
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
    // **붙여넣기를 글로 받는다**(moai-od9q). 안 켜면 붙인 글이 키 하나하나로 와서, 탭은 폼의
    // 칸을 옮기고 줄바꿈은 Enter 로 검색을 걸거나 제목을 떠나며, 탐색 중에 붙인 `q` 는 끝낸다.
    // 끄는 길은 둘이다 — 여기 아래의 정상 끝과 **패닉 훅.** ratatui 의 훅은 raw mode 와 대체
    // 화면만 걷으므로, 안 걸면 패닉 뒤 사용자 셸에 붙인 글이 `200~…201~` 에 싸여 들어간다.
    // 훅은 `try_init` **뒤에** 건다 — 그래야 끄기가 ratatui 의 복구를 감싸 먼저 돈다. 켜기를
    // 못 해도 멈추지 않는다: 붙여넣기가 옛날처럼 키로 올 뿐이다.
    paste_off_on_panic();
    let _ = bracketed_paste(&mut std::io::stdout(), true);
    let out = loop_until_quit(&mut term, &mut app);
    let _ = bracketed_paste(&mut std::io::stdout(), false);
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
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};

/// bracketed paste 를 켜거나 끈다 — xterm 의 2004 번. 쓰는 곳을 받는 것은 시험이 그 글을
/// 보려고서다([`paste_off_on_panic`] 이 같은 글을 낸다).
fn bracketed_paste(out: &mut impl std::io::Write, on: bool) -> std::io::Result<()> {
    if on {
        ratatui::crossterm::execute!(out, EnableBracketedPaste)
    } else {
        ratatui::crossterm::execute!(out, DisableBracketedPaste)
    }
}

/// 패닉하면 **먼저 bracketed paste 를 끄고** 걸려 있던 훅(ratatui 의 터미널 복구)으로 넘긴다.
/// 어느 스레드의 패닉에도 돈다 — 버린 다시 읽기 스레드가 터져도 셸이 붙여넣기를 싸서 받지 않는다.
fn paste_off_on_panic() {
    let next = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = bracketed_paste(&mut std::io::stdout(), false);
        next(info);
    }));
}

/// 생각 담기를 적을 편집기(moai-08af). **환경과 PATH 를 읽는 것은 여기다** — 고르는 차례는
/// 조각([`crate::tui::jotfile::pick`])이 정한다. 띄울 때 한 번 고른다: 도는 중에 `$EDITOR` 가
/// 바뀔 길은 없고, 키마다 PATH 를 훑을 까닭도 없다.
fn editor() -> Option<String> {
    let var = |k: &str| std::env::var(k).ok();
    crate::tui::jotfile::pick(var("VISUAL").as_deref(), var("EDITOR").as_deref(), on_path)
}

/// PATH 에 그 이름의 실행 파일이 있는가.
fn on_path(name: &str) -> bool {
    let Some(dirs) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&dirs).any(|d| std::fs::metadata(d.join(name)).is_ok_and(|m| m.is_file() && executable(&m)))
}

#[cfg(unix)]
fn executable(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    true
}

/// 터미널을 **내리고** `f` 를 부른 뒤 다시 올린다 — 편집기에 터미널을 넘기는 자리(moai-08af).
///
/// 내리는 것은 끝낼 때(`screen`)와 같은 차례다: bracketed paste 를 끄고 raw mode·대체 화면을
/// 걷는다. 안 끄면 편집기에 붙인 글이 `200~…201~` 에 싸여 들어간다. 올릴 때는 거꾸로 켜고
/// **화면을 비워 다음 그림이 통째로 다시 그리게 한다** — ratatui 는 바뀐 칸만 내보내는데,
/// 편집기가 지나간 화면은 ratatui 가 아는 앞 그림과 다르다. 커서는 그림마다 숨기거나 두므로
/// 따로 안 만진다.
///
/// `f` 가 도는 동안 **이 스레드는 거기 서 있다** — 그리기·스피너·다시 읽기 받기(`follow`)가
/// 전부 멈춘다. 다시 읽기 스레드는 계속 짓지만 그리지 않고, 돌아오면 다음 걸음이 받는다.
/// 다시 올리기를 못 하면 오류로 루프를 끝낸다 — `screen` 이 그 뒤를 끝낼 때처럼 걷는다.
fn suspended<T>(term: &mut DefaultTerminal, f: impl FnOnce() -> T) -> std::io::Result<T> {
    let _ = bracketed_paste(&mut std::io::stdout(), false);
    ratatui::restore();
    let out = f();
    ratatui::crossterm::terminal::enable_raw_mode()?;
    ratatui::crossterm::execute!(std::io::stdout(), ratatui::crossterm::terminal::EnterAlternateScreen)?;
    let _ = bracketed_paste(&mut std::io::stdout(), true);
    term.clear()?;
    Ok(out)
}

/// `text` 를 임시 파일에 적고 `editor` 로 연 뒤 **고친 글**을 돌려준다. `Err` 는 담지 않을
/// 까닭이다 — 편집기가 0 이 아닌 코드로 끝났다(vim 의 `:cq`), 못 띄웠다, 못 읽었다.
///
/// 파일은 **어느 길로든 지운다.** 받은 글은 이미 돌려주었고, 담기가 실패해도 그 글은 폼에
/// 열린 채 남는다([`App::edited`]) — 파일에 둘 까닭이 없다. 편집기는 이 프로세스의 stdin·
/// stdout 을 그대로 받는다. 기다리는 동안 Ctrl-C 가 신호로 오면(raw 로 돌지 않는 편집기)
/// moai 도 같이 끝난다 — 터미널은 이미 내려 둔 채라 셸은 멀쩡하고, 파일 하나가 남는다.
fn write_in_editor(editor: &str, text: &str, dir: &std::path::Path) -> Result<String, String> {
    use std::io::Write;
    let (path, mut file) = scratch_file(dir).map_err(|e| format!("임시 파일을 못 만들었다 — {e}"))?;
    let written = file.write_all(text.as_bytes()).and_then(|()| file.sync_all());
    drop(file);
    let got = written
        .map_err(|e| format!("임시 파일에 못 적었다 — {e}"))
        .and_then(|()| {
            std::process::Command::new("sh")
                .args(crate::tui::jotfile::argv(editor, &path))
                .status()
                .map_err(|e| format!("편집기를 못 띄웠다({editor}) — {e}"))
        })
        .and_then(|status| match status.code() {
            Some(0) => std::fs::read_to_string(&path).map_err(|e| format!("편집기가 남긴 파일을 못 읽었다 — {e}")),
            Some(code) => Err(format!("편집기가 {code} 로 끝났다({editor})")),
            None => Err(format!("편집기가 신호로 끝났다({editor})")),
        });
    let _ = std::fs::remove_file(&path);
    got
}

/// 편집기에 넘길 새 파일. **남이 못 읽게(0600) 새로 만든다** — 공유 임시 디렉터리라 이름을
/// 짐작한 남이 먼저 둔 파일(심볼릭 링크 포함)을 열면 적은 생각이 그리로 샌다. `create_new` 는
/// 있는 것을 안 연다. `.md` 는 편집기가 본문을 마크다운으로 칠하게 한다.
fn scratch_file(dir: &std::path::Path) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..64 {
        let path = dir.join(format!("moai-idea-{}-{}.md", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "빈 이름을 못 찾았다"))
}

/// 루프가 받은 사건 하나를 탐색기에 넘긴다. **터미널 없이 시험된다.**
///
/// - 키는 **누를 때만** 받는다. crossterm 은 kitty 프로토콜 터미널에서 뗄 때도 보내므로,
///   거르지 않으면 키 하나가 두 번 먹는다
/// - 붙여넣기는 글로 넘긴다([`App::paste`]) — 키로 풀지 않는다
/// - 창 크기 따위는 받을 것이 없다. 다음 그림이 새 크기로 그린다
fn take(app: &mut App, ev: Event) {
    match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press => app.key(k),
        Event::Paste(text) => app.paste(&text),
        _ => {}
    }
}

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
            take(app, event::read()?);
        }
        // 키가 편집기를 청했으면(`n`) 터미널을 넘긴다. 받은 글은 App 이 담는다 — 담을 곳은 연
        // 순간 박힌 그대로 요청에 실려 왔다.
        if let Some(edit) = app.edit.take() {
            let got = suspended(term, || write_in_editor(&edit.editor, &edit.text, &std::env::temp_dir()))?;
            app.edited(edit.into, got);
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
        assert_eq!(app.mode, Mode::Grep(Input::new("a b")));
        let release = KeyEvent::new_with_kind_and_state(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Release, KeyEventState::NONE);
        take(&mut app, Event::Key(release));
        assert!(matches!(app.mode, Mode::Grep(_)), "뗀 키를 먹었다");
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

    /// 편집기 시험의 임시 자리. 이름에 **빈칸**을 넣는다 — 경로가 셸에서 쪼개지면 여기서 드러난다.
    struct Dir(std::path::PathBuf);

    impl Dir {
        fn new(name: &str) -> Dir {
            let d = std::env::temp_dir().join(format!("moai-editor {name}-{}-{:?}", std::process::id(), std::thread::current().id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Dir(d)
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
            std::fs::read_dir(&self.0)
                .unwrap()
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| n.starts_with("moai-idea-"))
                .collect()
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// **편집기가 고친 글이 돌아오고, 파일은 남이 못 읽게 만들어졌다가 지워진다.** 편집기
    /// 글의 인자(`--wait`)는 셸 규칙으로 서고, 빈칸 든 경로는 한 인자로 간다.
    #[test]
    fn the_editor_gets_the_template_and_its_edit_comes_back() {
        let d = Dir::new("ok");
        let editor = d.editor("f=\"$2\"; printf '제목\\n\\n본문\\n' >> \"$f\"");
        let got = write_in_editor(&editor, "# 안내\n", &d.0).expect("편집기가 돌려주지 않았다");
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
        let why = write_in_editor(&editor, "", &d.0).expect_err("비영으로 끝났는데 글을 돌려줬다");
        assert!(why.contains('3'), "{why}");
        assert_eq!(d.leftovers(), Vec::<String>::new());

        let why = write_in_editor("moai-없는-편집기-08af", "", &d.0).expect_err("없는 편집기인데 글을 돌려줬다");
        assert!(why.contains("127"), "{why}");
        assert_eq!(d.leftovers(), Vec::<String>::new());
    }

    /// 임시 파일은 **있는 이름을 안 연다** — 남이 먼저 둔 파일에 적은 생각을 쓰지 않는다.
    #[test]
    fn the_scratch_file_never_reuses_an_existing_name() {
        let d = Dir::new("name");
        let (a, _fa) = scratch_file(&d.0).unwrap();
        let (b, _fb) = scratch_file(&d.0).unwrap();
        assert_ne!(a, b);
        std::fs::write(&a, "남의 것").unwrap();
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "남의 것");
    }

    /// 아무리 비싼 프레임에서도 **멈춘 것처럼 보이지는 않는다.** 멈춘 스피너는
    /// 일이 멈췄다는 거짓말이다.
    #[test]
    fn even_a_hopeless_frame_keeps_the_glyph_turning() {
        assert_eq!(spin_step(Duration::from_secs(30)), SPIN_SLOWEST);
        assert!(spin_step(Duration::from_secs(30)) <= Duration::from_secs(1));
    }
}
