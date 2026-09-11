//! 탐색기 화면. **읽기 전용이다** — `with_write` 를 부르지 않는다.
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
    let repo = Repo::discover()?;
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
        let rows: Vec<Row> = index
            .entries(&load.issues, &path)
            .iter()
            .map(|e| Row::of(&index, &load.issues, e))
            .collect();
        return super::json_line(&rows);
    }

    // **TTY 가 아니면 켜지 않는다.** 파이프에 대고 대체 화면을 켜면 그 자리에서
    // 멈춰 서고, 부른 쪽은 왜 멈췄는지 알 길이 없다.
    if !std::io::stdout().is_terminal() {
        return Err(Fail::coded(
            "터미널이 아니라 탐색기를 띄우지 않는다.\n      \
             목록만 필요하면 `moai tui --json` 이다",
            super::code::BAD_INPUT,
        ));
    }

    // 터미널 복구는 ratatui 에 맡긴다 — `try_init` 이 raw mode·대체 화면을 켜고
    // **되돌리는 패닉 훅까지** 건다. 손으로 짜면 어느 이른 return 하나가
    // 사용자 셸을 망가뜨린다.
    //
    // **`ratatui::run` 이 아니라 `try_init` 이다.** `run` 은 안에서 `init()` 을
    // 부르고 그것은 `.expect()` 다 — 통제 터미널이 없거나 크기를 못 얻으면
    // 101 번 패닉이 나고, `--json` 으로 부른 쪽은 약속된 오류 객체 대신
    // 역추적 문구를 받는다. 여기서 받아 `Fail` 로 바꾼다.
    let mut app = App::open(repo, load, index, path, stamp);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u8>,
}

impl Row {
    fn of(index: &Index, issues: &[Issue], e: &Entry) -> Row {
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

/// 키를 기다리다 이따금 깬다. **깨는 것은 파일이 바뀌었는지 보려는 것뿐이다** —
/// 저절로 다시 읽지는 않는다. 커서가 튀면 읽던 자리를 잃는다.
const TICK: std::time::Duration = std::time::Duration::from_millis(700);

fn loop_until_quit(term: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut due = std::time::Instant::now() + TICK;
    while !app.quit {
        term.draw(|f| crate::tui::draw::screen(f, app))?;
        // **시간으로 센다, 한가함으로 세지 않는다.** 이벤트가 오는 동안에만
        // 안 보면 — 키를 누르고 있거나 창을 끄는 내내 — 바뀐 것을 못 본다.
        // 하필 그때가 쓰는 사람이 화면을 보고 있는 때다.
        let wait = due.saturating_duration_since(std::time::Instant::now());
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
        if now >= due {
            app.check_stale();
            due = now + TICK;
        }
    }
    Ok(())
}
