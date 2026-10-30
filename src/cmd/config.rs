//! 저장소의 설정을 읽는다(moai-wk7i.qtq). 쓰는 길은 `Repo::set_archive_days`
//! 하나이며 탐색기의 `SPC o o` 가 부른다.
//!
//! **기본값을 채워 낸다.** 밖에서 그리는 쪽(웹 패널)이 칸 차례·아카이브 날수·위키 자리를 알려면
//! `.moai/config.toml` 을 제가 다시 풀고 기본값을 제가 다시 알아야 했다 — 그 둘째 파서가 이 바이너리와
//! 어긋나는 날 화면이 거짓말한다. 여기서 낸 값이 이 바이너리가 실제로 쓰는 값이다.

use super::{Ctx, R};
use crate::config::{Config, Thresholds};

/// `config --json` 한 객체. **키는 파일의 키와 같은 이름이다** — 문턱도 파일처럼 평평하게
/// (`status_review_days`) 낸다. 읽은 값을 그대로 파일에 옮겨 적을 수 있어야 두 이름을 안 외운다.
#[derive(serde::Serialize)]
struct Out<'a> {
    prefix: &'a str,
    statuses: &'a [String],
    naming: &'static str,
    archive_days: i64,
    wiki_dir: &'a str,
    status_review_days: i64,
    status_wip_days: i64,
    status_blocked_days: i64,
    status_wip_limit: usize,
    status_no_epic_ratio: f64,
    status_no_epic_min: usize,
    status_flow_days: i64,
    status_backlog_pile: usize,
    status_due_days: i64,
    /// `auto` 또는 `off`(moai-dr8a.cf1).
    due_priority: &'static str,
    due_p0_days: i64,
    due_p1_days: i64,
    due_p2_days: i64,
    /// 읽은 파일의 자리.
    path: &'a std::path::Path,
}

impl<'a> Out<'a> {
    fn of(cfg: &'a Config, path: &'a std::path::Path) -> Out<'a> {
        let Thresholds {
            review_days,
            wip_days,
            blocked_days,
            wip_limit,
            no_epic_ratio,
            no_epic_min,
            flow_days,
            backlog_pile,
            due_days,
        } = cfg.status;
        Out {
            prefix: &cfg.prefix,
            statuses: &cfg.statuses,
            naming: cfg.naming.as_str(),
            archive_days: cfg.archive_days,
            wiki_dir: &cfg.wiki_dir,
            status_review_days: review_days,
            status_wip_days: wip_days,
            status_blocked_days: blocked_days,
            status_wip_limit: wip_limit,
            status_no_epic_ratio: no_epic_ratio,
            status_no_epic_min: no_epic_min,
            status_flow_days: flow_days,
            status_backlog_pile: backlog_pile,
            status_due_days: due_days,
            due_priority: cfg.due.mode(),
            due_p0_days: cfg.due.p0_days,
            due_p1_days: cfg.due.p1_days,
            due_p2_days: cfg.due.p2_days,
            path,
        }
    }
}

pub fn run(ctx: &Ctx) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let path = repo.dir().join("config.toml");
    let out = Out::of(&repo.config, &path);
    if ctx.json {
        return super::json_line(&out);
    }
    // 사람의 화면은 파일의 꼴 그대로다 — 그대로 옮겨 적으면 같은 설정이 된다. 차례도 위 `Out` 의 것이다. `[models]` 표는
    // 여기 안 선다 — 사람의 설정 밑에 깔아 읽는 값이라 `moai models` 가 낸다(moai-w25x.jud).
    // `[writing]` 도 같은 이유로 `moai writing` 이 낸다.
    let t = |v: &dyn std::fmt::Display| v.to_string();
    let q = |v: &str| format!("{v:?}");
    let pairs: [(&str, String); 18] = [
        ("prefix", q(out.prefix)),
        ("statuses", q(&out.statuses.join(","))),
        ("naming", q(out.naming)),
        ("archive_days", t(&out.archive_days)),
        ("wiki_dir", q(out.wiki_dir)),
        ("status_review_days", t(&out.status_review_days)),
        ("status_wip_days", t(&out.status_wip_days)),
        ("status_blocked_days", t(&out.status_blocked_days)),
        ("status_wip_limit", t(&out.status_wip_limit)),
        ("status_no_epic_ratio", t(&out.status_no_epic_ratio)),
        ("status_no_epic_min", t(&out.status_no_epic_min)),
        ("status_flow_days", t(&out.status_flow_days)),
        ("status_backlog_pile", t(&out.status_backlog_pile)),
        ("status_due_days", t(&out.status_due_days)),
        ("due_priority", q(out.due_priority)),
        ("due_p0_days", t(&out.due_p0_days)),
        ("due_p1_days", t(&out.due_p1_days)),
        ("due_p2_days", t(&out.due_p2_days)),
    ];
    let mut lines: Vec<String> = pairs.iter().map(|(k, v)| crate::text::one_line(&format!("{k:20} = {v}"))).collect();
    lines.push(crate::i18n::fill(
        crate::i18n::say(ctx.lang(), "models.path"),
        &[("path", &crate::text::one_line(&path.display().to_string()))],
    ));
    Ok(lines)
}
