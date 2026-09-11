//! 탐색기 화면. **읽기 전용이다** — `with_write` 를 부르지 않는다.
//!
//! 여기는 얇다. 무엇이 어느 디렉터리에 걸리는지는 [`crate::nav`] 가 정하고,
//! 무엇을 세는지는 `report` 가 정한다. 이 파일은 잇고 그리기만 한다.

use super::{Ctx, Fail, R};
use crate::cli::TuiArgs;
use crate::model::{Issue, Kind};
use crate::nav::{Entry, Index, Path, Seg};
use crate::store::Repo;
use std::io::IsTerminal;

pub fn run(ctx: &Ctx, args: TuiArgs) -> R<Vec<String>> {
    let repo = Repo::discover()?;
    let load = repo.read()?;
    let index = Index::of(&load.issues);
    let path = resolve(&index, &load.issues, args.path.as_deref())?;

    // `--json` 은 화면을 켜지 않는다. 기계로 읽는 쪽과 통합 시험이 이 길로 온다.
    if ctx.json {
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

    // 터미널 복구는 `ratatui::run` 에 맡긴다 — raw mode·대체 화면을 켜고,
    // **되돌리는 패닉 훅까지** 걸고, 끝나면 restore 한다. 손으로 짜면 어느
    // 이른 return 하나가 사용자 셸을 망가뜨린다.
    let mut app = App::new(load.issues, repo.config.clone(), path);
    ratatui::run(|term| loop_until_quit(term, &mut app)).map_err(|e| Fail::new(e.to_string()))?;
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
fn resolve(index: &Index, issues: &[Issue], want: Option<&str>) -> R<Path> {
    let Some(want) = want else { return Ok(Path::new()) };
    match want {
        "없음" => return Ok(vec![Seg::Milestone(None)]),
        "길잃음" => return Ok(vec![Seg::Lost]),
        _ => {}
    }
    let at = issues.iter().position(|i| i.id == want).ok_or_else(|| Fail::not_found(want))?;
    let mut path = index.home_of(at).clone();
    let seg = match issues[at].kind {
        Kind::Milestone => Some(Seg::Milestone(Some(issues[at].id.clone()))),
        Kind::Epic => Some(Seg::Epic(issues[at].id.clone())),
        Kind::Issue => Some(Seg::Issue(issues[at].id.clone())),
    };
    // 자식이 없는 이슈는 들어갈 데가 없다 — 그것이 든 디렉터리를 그대로 둔다.
    if let Some(seg) = seg
        && !index.entries(issues, &{
            let mut p = path.clone();
            p.push(seg.clone());
            p
        })
        .is_empty()
    {
        path.push(seg);
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
                status: Some(issues[at].status.as_str().to_string()),
                priority: Some(issues[at].priority()),
            },
            None => Row { id: None, title, kind: "bucket", dir, status: None, priority: None },
        }
    }
}

// ── 화면 ──────────────────────────────────────────────────────────────

use crate::tui::App;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn loop_until_quit(term: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    while !app.quit {
        term.draw(|f| crate::tui::draw::screen(f, app))?;
        // **누를 때만 받는다.** crossterm 은 kitty 프로토콜 터미널에서 뗄 때도
        // 보내므로, 거르지 않으면 키 하나가 두 번 먹는다.
        if let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            app.key(k);
        }
    }
    Ok(())
}
