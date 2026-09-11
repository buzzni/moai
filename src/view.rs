//! 사람이 읽을 줄을 만든다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 표를 쓰는 이상 폭 계산이 필요하다. 한글은 터미널에서 두 칸을 먹으므로
//! `len()` 으로 맞추면 한글 제목이 섞인 표가 전부 어긋난다.

use crate::config::Config;
use crate::model::{Issue, JournalEntry, Kind};
use crate::report::{Roll, StatusReport, Warning};
use crate::style::{self, paint};
use anstyle::Style;
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

/// 제목이 이보다 길면 자른다. 표가 접히면 표가 아니다.
const TITLE_CAP: usize = 44;
/// 에픽 열은 곁다리라 더 짧게 자른다.
const EPIC_CAP: usize = 20;
/// 진행 막대 칸 수.
const BAR: usize = 10;

fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// 표시 폭 기준으로 자르고 `…` 를 붙인다.
fn clip(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        let w = UnicodeWidthStr::width(c.encode_utf8(&mut [0u8; 4]) as &str);
        if width(&out) + w > max.saturating_sub(1) {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// 칠한 뒤 **칠하지 않은 폭**을 기준으로 채운다. 순서를 바꾸면 이스케이프가
/// 폭에 세어져 표가 어긋난다.
fn cell(style: Style, text: &str, w: usize) -> String {
    let pad = w.saturating_sub(width(text));
    format!("{}{}", paint(style, text), " ".repeat(pad))
}

fn stamp(at: &str) -> String {
    match (at.get(..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_string(),
    }
}

fn short_stamp(at: &str) -> String {
    match (at.get(5..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_string(),
    }
}

fn tags_of(i: &Issue) -> String {
    i.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
}

fn title_style(i: &Issue) -> Style {
    // 제목은 칠하지 않는다 — 내용은 기본색, 주변만 칠한다.
    // 에픽만 예외다. 계획 계층이 한눈에 떠야 한다.
    if i.kind == Kind::Issue { style::PLAIN } else { style::EPIC }
}

/// 목록. 비어 있으면 빈 줄이 아니라 왜 비었는지를 말한다.
pub fn list(
    issues: &[Issue],
    cfg: &Config,
    hidden_done: usize,
    epics: &BTreeMap<&str, String>,
) -> Vec<String> {
    if issues.is_empty() {
        return vec![if hidden_done > 0 {
            format!("없다. done {hidden_done}건은 숨겼다 — `--all`")
        } else {
            "없다.".into()
        }];
    }

    let show_tags = issues.iter().any(|i| !i.tags.is_empty());
    let show_epic = issues.iter().any(|i| epics.contains_key(i.id.as_str()));
    let heads: Vec<String> = issues.iter().map(|i| clip(&i.title, TITLE_CAP)).collect();
    let tags: Vec<String> = issues.iter().map(tags_of).collect();
    // 에픽 열은 **제목**을 보여준다. id 를 보여주면 사람이 그걸 다시 찾아봐야 한다.
    let epics: Vec<String> = issues
        .iter()
        .map(|i| match epics.get(i.id.as_str()) {
            None => "—".into(),
            Some(t) => clip(t, EPIC_CAP),
        })
        .collect();

    let w_id = issues.iter().map(|i| width(&i.id)).max().unwrap_or(2).max(2);
    let w_title = heads.iter().map(|t| width(t)).max().unwrap_or(4);
    let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0);

    let mut out = Vec::with_capacity(issues.len() + 2);
    let mut head = format!(
        "{}{}{}{}",
        cell(style::HEAD, "ID", w_id + 3),
        cell(style::HEAD, "P", 4),
        cell(style::HEAD, "S", 3),
        cell(style::HEAD, "제목", if show_tags || show_epic { w_title + 2 } else { 0 }),
    );
    if show_tags {
        head.push_str(&cell(style::HEAD, "태그", if show_epic { w_tags + 2 } else { 0 }));
    }
    if show_epic {
        head.push_str(&paint(style::HEAD, "에픽"));
    }
    out.push(head.trim_end().to_string());

    for (((i, title), tag), epic) in issues.iter().zip(&heads).zip(&tags).zip(&epics) {
        let st = style::status_style(i.status.as_str());
        let mut row = format!(
            "{}{}{}  {}",
            cell(style::ID, &i.id, w_id + 3),
            cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
            paint(st, style::glyph(i.status.as_str())),
            cell(title_style(i), title, if show_tags || show_epic { w_title + 2 } else { 0 }),
        );
        if show_tags {
            row.push_str(&cell(style::TAG, tag, if show_epic { w_tags + 2 } else { 0 }));
        }
        if show_epic {
            row.push_str(&paint(style::EPIC_REF, epic));
        }
        out.push(row.trim_end().to_string());
    }

    out.push(String::new());
    out.push(summary(issues, cfg, hidden_done));
    out
}

fn summary(issues: &[Issue], cfg: &Config, hidden_done: usize) -> String {
    let counts: Vec<String> = cfg
        .statuses
        .iter()
        .filter_map(|s| {
            let n = issues.iter().filter(|i| i.status.as_str() == s).count();
            (n > 0).then(|| format!("{s} {n}"))
        })
        .collect();
    let mut line = format!("{}건 ({})", issues.len(), counts.join(" · "));
    if hidden_done > 0 {
        line.push_str(&paint(
            style::DIM,
            &format!("     done {hidden_done}건 숨김 — `--all`"),
        ));
    }
    line
}

/// 채운 칸과 빈 칸. 멤버가 없으면 막대 대신 빈 자리를 준다 —
/// 0% 막대를 그리면 "아직 안 한 에픽" 과 "속을 안 채운 에픽" 이 같아 보인다.
pub fn bar(percent: Option<u8>) -> String {
    match percent {
        None => paint(style::DIM, &"░".repeat(BAR)),
        Some(p) => {
            let filled = (p as usize * BAR).div_ceil(100).min(BAR);
            format!(
                "{}{}",
                paint(style::status_style("done"), &"█".repeat(filled)),
                paint(style::DIM, &"░".repeat(BAR - filled))
            )
        }
    }
}

/// 에픽 → 멤버 → 자식으로 접어 낸다.
///
/// `shown` 은 이미 걸러진 것들이다. **걸러진 뒤에도 에픽 줄은 남긴다** —
/// 멤버가 하나도 안 걸리면 그 에픽은 아예 빼되, 걸린 것이 있으면 어느
/// 에픽 밑인지 보여야 목록이 뜻을 갖는다.
pub fn tree(shown: &[Issue], rolls: &[Roll], groups: &BTreeMap<&str, &str>) -> Vec<String> {
    let mut out = Vec::new();
    for roll in rolls {
        let mine: Vec<&Issue> = shown
            .iter()
            .filter(|i| {
                let group = groups.get(i.id.as_str()).copied();
                match &roll.id {
                    Some(e) => group == Some(e.as_str()),
                    None => group.is_none() && crate::report::is_work(i),
                }
            })
            .collect();
        // 자기 자신이 걸러졌으면 빈 에픽도 보여 준다 (계획만 세운 것).
        let epic_shown = roll.id.as_ref().is_some_and(|e| shown.iter().any(|i| &i.id == e));
        if mine.is_empty() && !epic_shown {
            continue;
        }
        if !out.is_empty() {
            out.push(String::new());
        }
        out.push(head(roll, mine.len()));
        let tops: Vec<&Issue> = mine
            .iter()
            .copied()
            .filter(|i| {
                crate::id::parent_of(&i.id).is_none_or(|p| !mine.iter().any(|m| m.id == p))
            })
            .collect();
        for i in tops {
            branch(&mut out, shown, i, 1);
        }
    }
    if out.is_empty() {
        out.push("없다.".into());
    }
    out
}

/// 머리글 없이 멤버와 그 자식만. 에픽 상세에서 쓴다 — 상세가 이미 제목을
/// 냈는데 트리 머리글이 또 내면 같은 줄이 두 번 나온다.
pub fn members(shown: &[Issue]) -> Vec<String> {
    let mut out = Vec::new();
    let tops: Vec<&Issue> = shown
        .iter()
        .filter(|i| crate::id::parent_of(&i.id).is_none_or(|p| !shown.iter().any(|m| m.id == p)))
        .collect();
    for i in tops {
        branch(&mut out, shown, i, 0);
    }
    out
}

fn head(roll: &Roll, shown: usize) -> String {
    match &roll.id {
        // 묶음일 뿐 진척을 가진 것이 아니므로, 걸러진 뒤 **보이는** 수를 말한다.
        None => format!("{}  {}건", paint(style::HEAD, &roll.title), shown),
        Some(id) => {
            let pct = match roll.percent {
                None => paint(style::DIM, "자식 없음"),
                Some(p) => format!("{p:>3}%"),
            };
            format!(
                "{}  {}   {}/{}  {}  {}",
                paint(style::ID, id),
                paint(style::EPIC, &clip(&roll.title, TITLE_CAP)),
                roll.done,
                roll.total,
                bar(roll.percent),
                pct,
            )
        }
    }
}

/// 한 이슈와 그 밑의 자식들. 깊이는 id 의 점 수와 같다.
fn branch(out: &mut Vec<String>, shown: &[Issue], i: &Issue, depth: usize) {
    let st = style::status_style(i.status.as_str());
    let mut line = format!(
        "{}{}  {}  {}  {}",
        "  ".repeat(depth + 1),
        paint(style::ID, &i.id),
        paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
        paint(st, style::glyph(i.status.as_str())),
        paint(title_style(i), &clip(&i.title, TITLE_CAP)),
    );
    if !i.tags.is_empty() {
        line.push_str(&format!("   {}", paint(style::TAG, &tags_of(i))));
    }
    out.push(line);
    for c in shown.iter().filter(|c| crate::id::parent_of(&c.id) == Some(i.id.as_str())) {
        branch(out, shown, c, depth + 1);
    }
}

/// 경고 하나를 사람 말로. **여기가 이 제품의 목소리다.**
fn says(w: &Warning) -> String {
    let n = w.count;
    match w.kind {
        "no_epic" => format!(
            "에픽 없는 이슈 {n}건 (열린 것의 {}%)",
            (w.ratio.unwrap_or(0.0) * 100.0).round() as u32
        ),
        "no_milestone" => format!("마일스톤에 안 붙은 이슈 {n}건"),
        "stale_review" => format!("review 에 {}일 넘게 멈춘 것 {n}건", w.days.unwrap_or(0)),
        // review 도 벌여 놓은 일이다. "진행 중" 이라고 하면 review 경고와
        // 같은 이슈가 두 번 나오는 것이 말이 안 되게 보인다.
        "wip_overload" => format!("한 번에 벌여 놓은 것 {n}건 — 하나씩 끝내는 편이 낫다"),
        "stale_progress" => format!("집어 놓고 {}일 넘게 안 건드린 것 {n}건", w.days.unwrap_or(0)),
        // `days` 는 "막힌 기간" 이 아니라 "지금 칸에 머문 기간" 이다 — 막 막힌
        // 것을 "며칠째 막혀 있다" 고 잘못 말하지 않으려고 이렇게 적는다.
        "blocked_stale" => format!("막힌 채로 {}일 넘게 멈춰 있는 것 {n}건", w.days.unwrap_or(0)),
        "empty_epic" => format!("속이 빈 에픽 {n}건 — 계획만 세우고 안 채웠다"),
        "finished_epic" => format!("다 끝났는데 안 닫힌 에픽 {n}건"),
        "dangling_epic" => format!("없는 에픽을 가리키는 것 {n}건"),
        "orphan_child" => format!("부모 줄이 없는 자식 {n}건"),
        "dangling_blocked_by" => format!("없는 이슈에게 막혀 있다는 것 {n}건"),
        "unknown_field" => format!("모르는 필드를 들고 있는 줄 {n}건 — 새 바이너리가 쓴 파일일 수 있다"),
        "duplicate_id" => format!("id 가 두 번 있다 {n}건 — 머지를 잘못 풀었다"),
        "unreadable_line" => format!("읽을 수 없는 줄 {n}개"),
        other => format!("{other} {n}건"),
    }
}

/// 보드 · 경고 · 흐름.
///
/// **아무것도 막지 않는다.** 종료 코드는 데이터가 깨졌을 때만 0 이 아니다 —
/// 경고로 비영 종료하는 순간 부르는 쪽이 이것을 "실패" 로 읽고, 그러면 이건
/// 린트고, 린트는 곧 게이트다.
pub fn status(st: &StatusReport, issues: &[Issue], cfg: &Config, now: &str, at: &str) -> Vec<String> {
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut out = vec![
        format!(
            "{}  {}       {}",
            paint(style::HEAD, &format!("이슈 {}", st.total)),
            paint(style::DIM, &format!("· 에픽 {}", st.epics.len())),
            paint(style::DIM, at),
        ),
        String::new(),
    ];

    // 보드 — config 의 칸 차례 그대로.
    let board: Vec<String> = cfg
        .statuses
        .iter()
        .map(|s| {
            let style = style::status_style(s);
            format!(
                "{} {}",
                paint(style, style::glyph(s)),
                paint(style, &format!("{s} {}", st.counts.get(s).copied().unwrap_or(0)))
            )
        })
        .collect();
    out.push(format!("  {}", board.join("    ")));

    for (label, rolls) in [("마일스톤", &st.milestones), ("에픽", &st.epics)] {
        if rolls.is_empty() {
            continue;
        }
        out.push(String::new());
        if !st.milestones.is_empty() {
            out.push(paint(style::DIM, label));
        }
        let w_title = rolls.iter().map(|e| width(&clip(&e.title, EPIC_CAP))).max().unwrap_or(4);
        for e in rolls {
            // 이미 닫힌 묶음에 "닫을 때가 됐다" 를 내면, 시킨 대로 했는데도
            // 잔소리가 남는다. 한 번 하면 사라져야 말을 듣는다.
            let still_open = e
                .id
                .as_deref()
                .and_then(|id| by_id.get(id))
                .is_some_and(|i| !i.status.is_done());
            let note = match e.percent {
                None => paint(style::DIM, "   자식 없음"),
                Some(100) if still_open => paint(style::WARN, "   닫을 때가 됐다"),
                _ => String::new(),
            };
            out.push(format!(
                "  {}  {}  {}  {}/{}{}",
                paint(style::ID, e.id.as_deref().unwrap_or("")),
                cell(style::EPIC, &clip(&e.title, EPIC_CAP), w_title + 2),
                bar(e.percent),
                e.done,
                e.total,
                note,
            ));
        }
    }

    // 경고는 **에픽 표 바로 다음**이다. 화면 아래로 밀면 페이저에 잘린다.
    for w in &st.warnings {
        out.push(String::new());
        let mark = if w.fatal { style::ERROR } else { style::WARN };
        out.push(format!("{} {}", paint(mark, "!"), says(w)));
        out.extend(preview(w, &by_id, now));
    }
    if st.warnings.is_empty() {
        out.push(String::new());
        out.push(format!("{} 드러난 문제 없다", paint(style::status_style("done"), "✓")));
    }

    out.push(String::new());
    let net = st.flow.net;
    out.push(format!(
        "최근 {}일   생성 {}  ·  완료 {}   {}",
        st.flow.days,
        st.flow.created,
        st.flow.done,
        match net.cmp(&0) {
            std::cmp::Ordering::Greater => paint(style::WARN, &format!("쌓이는 중 +{net}")),
            std::cmp::Ordering::Less => paint(style::status_style("done"), &format!("줄어드는 중 {net}")),
            std::cmp::Ordering::Equal => paint(style::DIM, "제자리"),
        }
    ));
    out.push(String::new());
    out.push(paint(style::DIM, "다음:  `moai ready` 로 집을 것을 고른다"));
    out
}

/// 경고마다 앞의 몇 건만 보여 주고 나머지는 세어서 말한다. 다 늘어놓으면
/// 정작 봐야 할 다음 경고가 화면 밖으로 밀린다.
fn preview(w: &Warning, by_id: &BTreeMap<&str, &Issue>, now: &str) -> Vec<String> {
    const SHOW: usize = 3;
    let mut out = Vec::new();
    // 벌여 놓은 것과 깨진 것은 id 만 한 줄에 늘어놓는다 — 제목이 정보를 안 준다.
    if matches!(w.kind, "wip_overload" | "duplicate_id" | "orphan_child" | "dangling_blocked_by") {
        if !w.ids.is_empty() {
            out.push(format!("    {}", paint(style::DIM, &w.ids.join("   "))));
        }
        return out;
    }
    // 에픽에 대한 말은 칸도 나이도 뜻이 없다. 어느 에픽인지만 말한다.
    if matches!(w.kind, "empty_epic" | "finished_epic" | "unknown_field" | "dangling_epic") {
        for id in w.ids.iter().take(SHOW) {
            let title = by_id.get(id.as_str()).map(|i| i.title.as_str()).unwrap_or("");
            out.push(format!("    {}  {}", paint(style::ID, id), clip(title, TITLE_CAP)));
        }
        let rest = w.ids.len().saturating_sub(SHOW);
        if rest > 0 {
            out.push(format!("    {}", paint(style::DIM, &format!("{rest}건 더"))));
        }
        return out;
    }
    for id in w.ids.iter().take(SHOW) {
        let Some(i) = by_id.get(id.as_str()) else {
            out.push(format!("    {}", paint(style::ID, id)));
            continue;
        };
        let age = crate::model::days_since(&i.status_since, now)
            .map(|d| format!("{d}일"))
            .unwrap_or_default();
        out.push(format!(
            "    {}  {}  {}  {}  {}",
            paint(style::ID, &i.id),
            paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
            paint(style::status_style(i.status.as_str()), style::glyph(i.status.as_str())),
            paint(style::DIM, &format!("{age:>4}")),
            clip(&i.title, TITLE_CAP),
        ));
    }
    let rest = w.ids.len().saturating_sub(SHOW);
    let more = if rest > 0 { format!("{rest}건 더") } else { String::new() };
    if !more.is_empty() || w.hint.is_some() {
        out.push(format!(
            "    {}{}",
            cell(style::DIM, &more, if w.hint.is_some() { 12 } else { 0 }),
            w.hint.as_deref().map(|h| paint(style::DIM, &format!("→ `{h}`"))).unwrap_or_default(),
        ));
    }
    out
}

/// 집을 수 있는 일. 그리고 이미 벌여 놓은 것.
pub fn ready(picks: &[&Issue], epics: &BTreeMap<&str, String>, wip: &[&Issue]) -> Vec<String> {
    let mut out = vec![format!("집을 수 있는 일  {}건", picks.len())];
    if picks.is_empty() {
        out.push(String::new());
        out.push(paint(style::DIM, "없다. `moai show` 로 무엇이 밀려 있는지 본다"));
    } else {
        out.push(String::new());
        let heads: Vec<String> = picks.iter().map(|i| clip(&i.title, TITLE_CAP)).collect();
        let tags: Vec<String> = picks.iter().map(|i| tags_of(i)).collect();
        let w_id = picks.iter().map(|i| width(&i.id)).max().unwrap_or(2);
        let w_title = heads.iter().map(|t| width(t)).max().unwrap_or(4);
        let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0);

        for ((i, title), tag) in picks.iter().zip(&heads).zip(&tags) {
            let epic = match epics.get(i.id.as_str()) {
                None => "에픽 없음".to_string(),
                Some(t) => clip(t, EPIC_CAP),
            };
            out.push(
                format!(
                    "  {}{}{}{}",
                    cell(style::ID, &i.id, w_id + 3),
                    cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
                    cell(style::PLAIN, title, w_title + 3),
                    if w_tags > 0 {
                        format!("{}{}", cell(style::TAG, tag, w_tags + 3), paint(style::EPIC_REF, &epic))
                    } else {
                        paint(style::EPIC_REF, &epic)
                    },
                )
                .trim_end()
                .to_string(),
            );
        }
    }

    // 이미 벌여 놓은 것을 먼저 알린다. 새로 집기 전에 볼 것이다.
    if !wip.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            paint(
                style::DIM,
                &format!("이미 잡고 있는 것 {}건 — 새로 집기 전에 끝내는 편이 낫다", wip.len())
            )
        ));
        out.push(format!(
            "  {}",
            paint(style::DIM, &wip.iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join("  "))
        ));
    }
    out
}

/// 단건 상세. 이력은 저널을 **그대로 찍는다. 접지 않는다.**
pub fn detail(
    i: &Issue,
    epic: Option<&Issue>,
    children: &[&Issue],
    journal: &[JournalEntry],
    now: &str,
) -> Vec<String> {
    let mut out = vec![format!(
        "{}   {}",
        paint(style::ID, &i.id),
        paint(title_style(i), &i.title)
    )];

    let st = style::status_style(i.status.as_str());
    let mut line = format!(
        "  {} {} · {}",
        paint(st, style::glyph(i.status.as_str())),
        paint(st, i.status.as_str()),
        paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
    );
    if i.kind != Kind::Issue {
        line.push_str(&format!(" · {}", paint(style::EPIC, i.kind.as_str())));
    }
    if !i.tags.is_empty() {
        line.push_str(&format!(" · {}", paint(style::TAG, &tags_of(i))));
    }
    if let Some(a) = &i.assignee {
        line.push_str(&format!(" · {}", paint(style::DIM, a)));
    }
    out.push(line);

    if let Some(e) = &i.epic {
        let title = epic.map(|e| e.title.as_str()).unwrap_or("(없는 에픽)");
        out.push(format!("  에픽   {}  {title}", paint(style::ID, e)));
    }
    for c in children {
        out.push(format!(
            "  자식   {}  {}  ({} {})",
            paint(style::ID, &c.id),
            clip(&c.title, TITLE_CAP),
            paint(style::status_style(c.status.as_str()), style::glyph(c.status.as_str())),
            paint(style::DIM, c.status.as_str()),
        ));
    }

    let age = crate::model::days_since(&i.updated_at, now)
        .map(|d| match d {
            0 => "  (오늘)".to_string(),
            n => format!("  ({n}일 전)"),
        })
        .unwrap_or_default();
    out.push(format!(
        "  생성   {}      수정  {}{}",
        paint(style::DIM, &stamp(&i.created_at)),
        paint(style::DIM, &stamp(&i.updated_at)),
        paint(style::DIM, &age),
    ));

    if let Some(body) = &i.body {
        out.push(String::new());
        out.extend(body.lines().map(|l| format!("  {l}")));
    }

    out.extend(history(journal));
    out
}

/// 저널을 **그대로 찍는다. 접지 않는다.**
pub fn history(journal: &[JournalEntry]) -> Vec<String> {
    let mut out = Vec::new();
    if !journal.is_empty() {
        out.push(String::new());
        out.push(paint(style::HEAD, "이력"));
        for e in journal {
            // 메모는 여러 줄일 수 있다. 한 원소에 `\n` 을 담으면 "원소 하나가
            // 한 줄" 이라는 약속이 깨지고, 이어지는 줄이 열을 잃는다.
            let ts = short_stamp(&e.ts);
            let pad = " ".repeat(width(&ts) + 5);
            for (n, l) in entry(e).split('\n').enumerate() {
                out.push(match n {
                    0 => format!("  {}   {l}", paint(style::DIM, &ts)),
                    _ => format!("{pad}{l}"),
                });
            }
        }
    }
    out
}

fn entry(e: &JournalEntry) -> String {
    let what = match e.kind.as_str() {
        "create" => "생성".to_string(),
        "rm" => "삭제".to_string(),
        "status" => format!(
            "{} → {}",
            e.from.as_deref().unwrap_or("?"),
            paint(style::status_style(e.to.as_deref().unwrap_or("")), e.to.as_deref().unwrap_or("?"))
        ),
        "note" => format!("note: {}", e.text.as_deref().unwrap_or("")),
        other => other.to_string(),
    };
    let note = e.note.as_deref().map(|n| format!("  — {n}")).unwrap_or_default();
    format!("{what}{}  {}", paint(style::DIM, &note), paint(style::DIM, &e.by))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn no_epics() -> BTreeMap<&'static str, String> {
        BTreeMap::new()
    }

    fn issue(id: &str, title: &str, status: &str) -> Issue {
        Issue::new(id.into(), title.into(), Kind::Issue, Status::new(status), "2026-09-11T04:12:03Z")
    }

    fn plain(lines: &[String]) -> Vec<String> {
        // 테스트는 칠하지 않은 모양을 본다 — 표 정렬은 색과 무관해야 한다.
        lines
            .iter()
            .map(|l| {
                let mut out = String::new();
                let mut esc = false;
                for c in l.chars() {
                    match (esc, c) {
                        (false, '\u{1b}') => esc = true,
                        (true, 'm') => esc = false,
                        (true, _) => {}
                        (false, _) => out.push(c),
                    }
                }
                out
            })
            .collect()
    }

    /// 한글 제목과 ASCII 제목이 같은 열에서 만난다.
    #[test]
    fn cjk_titles_line_up() {
        let issues = vec![
            issue("argos-0001", "한글 제목이다", "todo"),
            issue("argos-0002", "ascii title", "review"),
        ];
        let out = plain(&list(&issues, &cfg(), 0, &no_epics()));
        let cols: Vec<usize> = out[1..3]
            .iter()
            .map(|l| width(l.split_once("  ").unwrap().0))
            .collect();
        assert_eq!(cols[0], cols[1], "{out:#?}");
        // 제목 시작 열이 같다
        let starts: Vec<usize> = out[1..3]
            .iter()
            .map(|l| width(&l[..l.find(['한', 'a']).unwrap()]))
            .collect();
        assert_eq!(starts[0], starts[1], "{out:#?}");
    }

    /// 헤더의 `제목` 과 줄의 제목이 같은 칸에서 시작한다.
    #[test]
    fn header_lines_up_with_rows() {
        let issues = vec![issue("argos-0001", "제목이다", "todo")];
        let out = plain(&list(&issues, &cfg(), 0, &no_epics()));
        let head_at = width(&out[0][..out[0].find("제목").unwrap()]);
        let row_at = width(&out[1][..out[1].find("제목이다").unwrap()]);
        assert_eq!(head_at, row_at, "{out:#?}");
    }

    #[test]
    fn empty_list_says_why() {
        assert_eq!(plain(&list(&[], &cfg(), 0, &no_epics()))[0], "없다.");
        assert!(plain(&list(&[], &cfg(), 3, &no_epics()))[0].contains("done 3건"));
    }

    #[test]
    fn summary_counts_each_column() {
        let issues = vec![issue("argos-0001", "a", "todo"), issue("argos-0002", "b", "todo")];
        let out = plain(&list(&issues, &cfg(), 5, &no_epics()));
        let last = out.last().unwrap();
        assert!(last.starts_with("2건 (todo 2)"), "{last}");
        assert!(last.contains("done 5건 숨김"), "{last}");
    }

    #[test]
    fn long_titles_are_clipped_not_wrapped() {
        let long = "가".repeat(80);
        let out = plain(&list(&[issue("argos-0001", &long, "todo")], &cfg(), 0, &no_epics()));
        assert!(out[1].ends_with('…'), "{:?}", out[1]);
        assert!(width(&out[1]) < 80, "{:?}", out[1]);
    }

    #[test]
    fn detail_shows_body_and_history() {
        let mut i = issue("argos-0001", "제목", "in_progress");
        i.body = Some("첫 줄\n둘째 줄".into());
        let j = vec![
            JournalEntry::create("argos-0001", "제목", "2026-09-09T14:02:11Z", "raven"),
            JournalEntry::status(
                "argos-0001",
                &Status::new("todo"),
                &Status::new("in_progress"),
                None,
                "2026-09-10T10:11:00Z",
                "claude",
            ),
        ];
        let out = plain(&detail(&i, None, &[], &j, "2026-09-11T04:12:03Z"));
        let joined = out.join("\n");
        assert!(joined.contains("첫 줄") && joined.contains("둘째 줄"), "{joined}");
        assert!(joined.contains("이력"), "{joined}");
        assert!(joined.contains("todo → in_progress"), "{joined}");
        assert!(joined.contains("09-09 14:02"), "{joined}");
    }

    /// 목록의 에픽 열은 id 가 아니라 제목이다. id 를 보여 주면 사람이
    /// 그걸 다시 찾아봐야 한다.
    #[test]
    fn the_epic_column_shows_a_title() {
        let mut i = issue("argos-0002", "멤버", "todo");
        i.epic = Some("argos-0001".into());
        let labels = BTreeMap::from([("argos-0002", "저장 계층".to_string())]);
        let out = plain(&list(&[i.clone()], &cfg(), 0, &labels));
        assert!(out[1].contains("저장 계층") && !out[1].contains("argos-0001"), "{out:#?}");

        // 없는 에픽을 가리켜도 죽지 않고 그렇다고 말한다
        let dangling = BTreeMap::from([("argos-0002", "(없는 에픽)".to_string())]);
        let out = plain(&list(&[i], &cfg(), 0, &dangling));
        assert!(out[1].contains("(없는 에픽)"), "{out:#?}");
    }

    /// 멤버 없는 에픽은 0% 가 아니라 막대 없음이다 — "아직 안 한 것" 과
    /// "속을 안 채운 것" 은 다르다.
    #[test]
    fn an_empty_bar_is_not_zero_percent() {
        assert!(!plain(&[bar(None)])[0].contains('█'));
        assert_eq!(plain(&[bar(Some(0))])[0].matches('█').count(), 0);
        assert_eq!(plain(&[bar(Some(100))])[0].matches('█').count(), BAR);
        // 1% 도 한 칸은 찬다 — 시작한 것이 안 시작한 것처럼 보이면 안 된다
        assert_eq!(plain(&[bar(Some(1))])[0].matches('█').count(), 1);
    }

    #[test]
    fn tree_nests_members_then_children() {
        let mut epic = issue("argos-0001", "저장 계층", "in_progress");
        epic.kind = Kind::Epic;
        let mut member = issue("argos-0002", "원자적 쓰기", "todo");
        member.epic = Some("argos-0001".into());
        let child = issue("argos-0002.aaa", "회귀 테스트", "todo");
        let loose = issue("argos-0009", "떠 있는 것", "todo");

        let shown = vec![member.clone(), child.clone(), loose.clone()];
        let all = vec![epic, member, child, loose];
        let rolls = crate::report::rollup(&all, &cfg());
        let groups = crate::report::groups(&all);
        let out = plain(&tree(&shown, &rolls, &groups));
        let joined = out.join("\n");

        assert!(joined.contains("저장 계층"), "{joined}");
        let at_member = out.iter().position(|l| l.contains("원자적 쓰기")).unwrap();
        let at_child = out.iter().position(|l| l.contains("회귀 테스트")).unwrap();
        assert!(at_child == at_member + 1, "자식이 부모 바로 밑이 아니다\n{joined}");
        // 자식이 더 깊게 들어간다
        let indent = |l: &str| l.len() - l.trim_start().len();
        assert!(indent(&out[at_child]) > indent(&out[at_member]), "{joined}");
        assert!(joined.contains("에픽 없음"), "{joined}");
    }

    /// 걸러진 뒤 멤버가 하나도 안 남은 에픽은 빼되, 자기 자신이 걸렸으면 남긴다.
    #[test]
    fn tree_drops_epics_with_nothing_to_show() {
        let mut epic = issue("argos-0001", "빈 에픽", "todo");
        epic.kind = Kind::Epic;
        let all = vec![epic.clone()];
        let rolls = crate::report::rollup(&all, &cfg());

        let groups = crate::report::groups(&all);
        assert_eq!(plain(&tree(&[], &rolls, &groups)), ["없다."]);
        assert!(plain(&tree(&[epic], &rolls, &groups)).join("\n").contains("빈 에픽"));
    }

    /// 시킨 대로 닫았는데도 잔소리가 남으면 다음부터 안 듣는다.
    #[test]
    fn a_closed_grouping_stops_nagging() {
        let mut epic = issue("argos-0001", "다 끝난 에픽", "todo");
        epic.kind = Kind::Epic;
        let mut member = issue("argos-0002", "멤버", "done");
        member.epic = Some("argos-0001".into());
        let all = vec![epic.clone(), member];
        let cfg = cfg();

        let open = crate::report::status(&all, &[], &cfg, "2026-09-11T04:12:03Z");
        let text = plain(&status(&open, &all, &cfg, "2026-09-11T04:12:03Z", ".moai/issues.jsonl")).join("\n");
        assert!(text.contains("닫을 때가 됐다"), "{text}");

        let mut all = all;
        all[0].status = Status::new("done");
        let closed = crate::report::status(&all, &[], &cfg, "2026-09-11T04:12:03Z");
        let text = plain(&status(&closed, &all, &cfg, "2026-09-11T04:12:03Z", ".moai/issues.jsonl")).join("\n");
        assert!(!text.contains("닫을 때가 됐다"), "{text}");
    }

    #[test]
    fn ready_names_where_each_pick_belongs() {
        let mut a = issue("argos-0002", "멤버", "todo");
        a.epic = Some("argos-0001".into());
        let b = issue("argos-0003", "떠 있는 것", "todo");
        let wip = issue("argos-0004", "잡고 있는 것", "in_progress");
        let labels = BTreeMap::from([("argos-0002", "저장 계층".to_string())]);

        let out = plain(&ready(&[&a, &b], &labels, &[&wip]));
        let joined = out.join("\n");
        assert!(joined.contains("2건"), "{joined}");
        assert!(joined.contains("저장 계층") && joined.contains("에픽 없음"), "{joined}");
        assert!(joined.contains("이미 잡고 있는 것 1건"), "{joined}");
        assert!(joined.contains("argos-0004"), "{joined}");

        let empty = plain(&ready(&[], &labels, &[])).join("\n");
        assert!(empty.contains("0건") && empty.contains("무엇이 밀려 있는지"), "{empty}");
    }

    /// 없는 에픽을 가리켜도 상세가 죽지 않는다 — 드러내되 막지 않는다.
    #[test]
    fn a_dangling_epic_is_shown_not_fatal() {
        let mut i = issue("argos-0001", "제목", "todo");
        i.epic = Some("argos-0000".into());
        let out = plain(&detail(&i, None, &[], &[], "2026-09-11T04:12:03Z"));
        assert!(out.iter().any(|l| l.contains("(없는 에픽)")), "{out:#?}");
    }
}
