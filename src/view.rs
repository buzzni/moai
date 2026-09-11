//! 사람이 읽을 줄을 만든다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 표를 쓰는 이상 폭 계산이 필요하다. 한글은 터미널에서 두 칸을 먹으므로
//! `len()` 으로 맞추면 한글 제목이 섞인 표가 전부 어긋난다.

use crate::config::Config;
use crate::model::{Issue, JournalEntry, Kind};
use crate::style::{self, paint};
use anstyle::Style;
use unicode_width::UnicodeWidthStr;

/// 제목이 이보다 길면 자른다. 표가 접히면 표가 아니다.
const TITLE_CAP: usize = 44;

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
pub fn list(issues: &[Issue], cfg: &Config, hidden_done: usize) -> Vec<String> {
    if issues.is_empty() {
        return vec![if hidden_done > 0 {
            format!("없다. done {hidden_done}건은 숨겼다 — `--all`")
        } else {
            "없다.".into()
        }];
    }

    let show_tags = issues.iter().any(|i| !i.tags.is_empty());
    let show_epic = issues.iter().any(|i| i.epic.is_some());
    let titles: Vec<String> = issues.iter().map(|i| clip(&i.title, TITLE_CAP)).collect();
    let tags: Vec<String> = issues.iter().map(tags_of).collect();

    let w_id = issues.iter().map(|i| width(&i.id)).max().unwrap_or(2).max(2);
    let w_title = titles.iter().map(|t| width(t)).max().unwrap_or(4);
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

    for ((i, title), tag) in issues.iter().zip(&titles).zip(&tags) {
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
            row.push_str(&paint(style::DIM, i.epic.as_deref().unwrap_or("—")));
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
        let out = plain(&list(&issues, &cfg(), 0));
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
        let out = plain(&list(&issues, &cfg(), 0));
        let head_at = width(&out[0][..out[0].find("제목").unwrap()]);
        let row_at = width(&out[1][..out[1].find("제목이다").unwrap()]);
        assert_eq!(head_at, row_at, "{out:#?}");
    }

    #[test]
    fn empty_list_says_why() {
        assert_eq!(plain(&list(&[], &cfg(), 0))[0], "없다.");
        assert!(plain(&list(&[], &cfg(), 3))[0].contains("done 3건"));
    }

    #[test]
    fn summary_counts_each_column() {
        let issues = vec![issue("argos-0001", "a", "todo"), issue("argos-0002", "b", "todo")];
        let out = plain(&list(&issues, &cfg(), 5));
        let last = out.last().unwrap();
        assert!(last.starts_with("2건 (todo 2)"), "{last}");
        assert!(last.contains("done 5건 숨김"), "{last}");
    }

    #[test]
    fn long_titles_are_clipped_not_wrapped() {
        let long = "가".repeat(80);
        let out = plain(&list(&[issue("argos-0001", &long, "todo")], &cfg(), 0));
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

    /// 없는 에픽을 가리켜도 상세가 죽지 않는다 — 드러내되 막지 않는다.
    #[test]
    fn a_dangling_epic_is_shown_not_fatal() {
        let mut i = issue("argos-0001", "제목", "todo");
        i.epic = Some("argos-0000".into());
        let out = plain(&detail(&i, None, &[], &[], "2026-09-11T04:12:03Z"));
        assert!(out.iter().any(|l| l.contains("(없는 에픽)")), "{out:#?}");
    }
}
