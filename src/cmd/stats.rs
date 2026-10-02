//! 거르개가 고른 줄을 센다 — 분포·흐름·소요·AI 작업(moai-1hka.k16).
//!
//! **여기는 잇기만 한다.** 무엇을 고르고 어떻게 세는지는 `report::stats` 가, 어떻게 보일지는 `view::stats` 가
//! 정한다. 탐색기의 통계 창도 같은 둘을 부른다 — 여기서 하나라도 세면 두 표면이 다른 수를 낸다.

use super::{Ctx, R};
use crate::cli::{AxisArg, BucketArg, StatsArgs};
use crate::model;
use crate::report::stats::{self, Ask, Axis, Bucket};

pub fn run(ctx: &Ctx, args: StatsArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    // **겹쳐 보지 않는다** — `--worktree` 를 안 받는다(`cli::StatsArgs`).
    let crate::worktree::Gathered { load, origin, .. } = super::gather(ctx, &repo, false)?;
    super::report_load_errors(ctx.lang(), &repo.issues_path(), &load.errors);
    let StatsArgs { by, bucket, last, filter } = args;
    let kind = filter.kind;
    // **목록과 한 자로 거른다**(`show::filter_of`) — 같은 거르개가 두 명령에서 다른 줄을 고르지 않게.
    let filter = super::show::filter_of(ctx, &repo, &load.issues, filter, kind)?;
    let now = model::now();
    // `-g` 는 노트도 본다 — 목록과 같은 길이다(`show::run`). 그때 읽은 저널은 `work` 도 쓴다: 노트 줄과 `model:`
    // 줄을 함께 거르므로, 저널을 두 번 풀지 않는다.
    let noted = filter.grep.is_some().then(|| {
        super::show::journal_of_rows(&repo, &origin, &load.issues, |l| {
            model::may_hold_note(l) || model::may_hold_work(l)
        })
    });
    let notes = noted.as_ref().map(super::show::notes_of);
    let soil = crate::report::Soil::of(&load.issues);
    let mut wh = crate::query::Where::from_soil(&load.issues, &repo.config, soil);
    wh.notes = notes.as_ref().map(crate::query::NoteView::Raw);
    // **흐름의 날은 늘 읽는 사람의 날이다.** 목록은 날로 친 때가 있을 때만 시간대를 풀지만
    // (`Filter::needs_zone`), 여기는 흐름이 늘 날로 가른다.
    wh.zone = Some(ctx.zone());
    let rows = stats::select(&load.issues, &filter, &wh, &now);
    // **센 줄의 `work` 만 가른다** — 고르지 않은 줄의 `model:` 줄은 어느 수에도 안 든다. 푸는 양은 줄지 않는다:
    // 저널은 줄마다 다 푼 뒤에 id 로 거른다(`Repo::journal_by_id`). 고른 줄이 없으면 아무 저널도 안 연다.
    //
    // **`work` 를 그리는 자리에서만 푼다** — 사람 화면의 `--by` 는 물은 축만 그리고 AI 작업은 안 낸다(`view::stats`).
    // 거기서도 풀면 못 읽는 저널 하나가 빠짐없이 그린 화면에 "이력이 빠졌다" 는 말과 비영 종료를 얹는다.
    let drawn = ctx.json || by.is_empty();
    let journal = match noted {
        Some(j) => j,
        None if drawn => super::show::journal_of_rows(&repo, &origin, rows.iter().copied(), model::may_hold_work),
        None => Default::default(),
    };
    let work = super::show::work_in(&journal, rows.iter().copied());
    // `--bucket`·`--last` 는 흐름을 묻는 말이다 — `--by` 곁에서도 사람 화면이 흐름을 그린다(`view::stats`).
    let flow_asked = bucket.is_some() || last.is_some();
    let bucket = match bucket {
        None | Some(BucketArg::Week) => Bucket::Week,
        Some(BucketArg::Day) => Bucket::Day,
    };
    let ask = Ask {
        // **센 종류는 지은 거르개에서 읽는다** — `--filter type=…` 은 `Filter::build` 안에서야 풀린다. argv 의
        // `--type` 으로 정하면 그 종류의 줄만 골라 놓고 `issue` 로 세어 모든 수가 0 이다. 탐색기의 창도 이 값을 읽는다.
        kind: filter.kind,
        by: by.into_iter().map(axis_of).collect(),
        bucket,
        last: last.unwrap_or_else(|| bucket.default_last()),
    };
    let st = stats::of(&rows, &wh, &repo.config, &work, &ask, &now);
    if ctx.json {
        // **덜 온 셈은 기계에도 댄다** — 저널을 못 읽으면 `work`·`reviews` 가 모자란데, 그 말이 stderr 에만
        // 서면 `--json` 으로 도는 고리는 모자란 수를 다 센 수로 읽는다. 목록의 줄마다 서는 `journal_error` 와
        // 같은 이름·같은 약속이다: 없는 것이 곧 "다 읽었다" 다.
        #[derive(serde::Serialize)]
        struct Said<'a> {
            #[serde(flatten)]
            stats: &'a stats::Stats,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            journal_error: Vec<super::JournalError>,
        }
        let unread = crate::store::journal_unread();
        let journal_error = super::journal_errors(ctx.lang(), &unread, Some(&repo.root));
        return super::json_line(&Said { stats: &st, journal_error });
    }
    let screen = crate::view::Screen::new(ctx.lang()).at(ctx.clock()).over(&origin);
    // `--by` 를 줬으면 물은 축만 통째로 그린다 — 한눈 보기는 그것을 안 물었을 때다. 흐름을 물었으면 그 밑에 흐름도.
    Ok(crate::view::stats(&st, &load.issues, &repo.config, !ask.by.is_empty(), flow_asked, screen))
}

/// argv 의 낱말을 `report` 의 축으로 잇는다 — `report` 는 clap 을 모른다(`show::sort_of` 와 같은 자리).
fn axis_of(a: AxisArg) -> Axis {
    match a {
        AxisArg::Status => Axis::Status,
        AxisArg::Kind => Axis::Kind,
        AxisArg::Priority => Axis::Priority,
        AxisArg::Tag => Axis::Tag,
        AxisArg::Assignee => Axis::Assignee,
        AxisArg::Epic => Axis::Epic,
        AxisArg::Milestone => Axis::Milestone,
    }
}
