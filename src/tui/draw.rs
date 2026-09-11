//! 그림. **뜻을 판단하지 않는다** — 무엇이 어디 걸리는지는 `nav`, 무엇을
//! 세는지는 `report` 가 이미 정했다.
//!
//! 규칙 하나를 CLI 에서 그대로 들고 온다: **색이 혼자 뜻을 지지 않는다.**
//! 모든 색에 글리프나 낱말이 붙는다.

use super::{App, Row};
use crate::nav::Entry;
use crate::style;
use crate::text::clip;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// 좌우를 가르는 자리. MC 처럼 반반이되 왼쪽을 조금 넓게 — 제목이 길다.
const LEFT: u16 = 55;

pub fn screen(f: &mut Frame, app: &App) {
    let [top, body, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .areas(f.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(LEFT), Constraint::Min(10)]).areas(body);

    crumbs(f, app, top);
    list(f, app, left);
    // 오른쪽은 뒤따르는 이슈(moai-2xcm)가 채운다. 빈 테두리라도 먼저 세워 두면
    // 좌우 폭이 그때 가서 바뀌지 않는다.
    f.render_widget(Block::default().borders(Borders::ALL).title(" 상세 "), right);
    fkeys(f, keys);
}

fn crumbs(f: &mut Frame, app: &App, at: Rect) {
    let here = clip(&app.crumbs(), at.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(here, Style::new().add_modifier(Modifier::BOLD)))),
        at,
    );
}

fn list(f: &mut Frame, app: &App, at: Rect) {
    let rows = app.rows();
    // 테두리 두 칸을 뺀 안쪽 폭. 좁은 창에서도 음수가 되지 않게 막는다.
    let inner = at.width.saturating_sub(2) as usize;
    let items: Vec<ListItem> = rows.iter().map(|r| ListItem::new(row_line(app, r, inner))).collect();

    // 제목에 칸별 건수를 **config 차례로** 낸다. 칸 이름과 순서는 저장소가
    // 정하는 것이라(`config.statuses`) 여기서 다시 정하지 않는다.
    let counts: Vec<String> = app
        .cfg
        .statuses
        .iter()
        .filter_map(|st| {
            let n = rows
                .iter()
                .filter_map(|r| match r {
                    Row::Item(e) => e.at(),
                    Row::Up => None,
                })
                .filter(|&at| app.issues[at].status.as_str() == st)
                .count();
            // 글리프만으로는 뜻이 약하다. 칸 이름을 같이 적는다.
            (n > 0).then(|| format!("{} {st} {n}", style::glyph(st)))
        })
        .collect();
    let title = if counts.is_empty() {
        " 비었다 ".to_string()
    } else {
        format!(" {} ", counts.join("  "))
    };
    let mut state = ListState::default();
    state.select((!rows.is_empty()).then_some(app.cursor.min(rows.len().saturating_sub(1))));
    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            // 커서는 **색만으로 표시하지 않는다** — 반전과 `>` 를 함께 준다.
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> "),
        at,
        &mut state,
    );
}

/// 한 줄: `id  p· 제목`. 디렉터리는 제목 뒤에 `/` 가 붙는다 — MC 와 같다.
fn row_line<'a>(app: &App, r: &Row, budget: usize) -> Line<'a> {
    let Row::Item(e) = r else {
        return Line::from(Span::styled("..", dim()));
    };
    let is_dir = matches!(e, Entry::Dir { .. });
    let Some(at) = e.at() else {
        // 바구니는 제 줄이 없다 — 이름만 낸다.
        return Line::from(Span::styled(format!("{}/", app.index.label(&app.issues, e)), dim()));
    };
    let i = &app.issues[at];

    let mut title = app.index.label(&app.issues, e);
    if is_dir {
        title.push('/');
    }
    // `>` 한 칸 + id + 공백 + p# + 글리프 + 공백 을 뺀 나머지가 제목 몫이다.
    let used = 2 + i.id.chars().count() + 2 + 2 + 1 + 2;
    let title = clip(&title, budget.saturating_sub(used).max(4));

    Line::from(vec![
        Span::styled(i.id.clone(), dim()),
        Span::raw("  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
        Span::raw(" "),
        // 칸은 글리프로도 말한다. 색이 없는 터미널에서도 뜻이 남아야 한다.
        Span::styled(style::glyph(i.status.as_str()).to_string(), status(i.status.as_str())),
        Span::raw(" "),
        Span::raw(title),
    ])
}

/// 맨 아래 MC 풍 F키 바. **아직 없는 것은 적지 않는다** — 눌러도 아무 일이
/// 없는 키를 적어 두면 그것부터 도구를 못 믿게 된다.
fn fkeys(f: &mut Frame, at: Rect) {
    let bar = Line::from(vec![
        key("Enter", "들어가기"),
        key("Backspace", "나가기"),
        key("F10", "끝내기"),
    ]);
    f.render_widget(Paragraph::new(bar), at);
}

fn key<'a>(k: &'a str, what: &'a str) -> Span<'a> {
    Span::styled(format!(" {k} {what} "), dim())
}

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// CLI 와 같은 뜻의 색. `style` 이 정한 것을 ratatui 쪽으로 옮기기만 한다.
fn status(s: &str) -> Style {
    match s {
        "in_progress" => Style::new().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        "review" => Style::new().fg(Color::LightMagenta),
        "done" => Style::new().fg(Color::Green).add_modifier(Modifier::DIM),
        "todo" => Style::new(),
        _ => Style::new().fg(Color::Cyan),
    }
}

fn priority(p: u8) -> Style {
    match p {
        0 => Style::new().fg(Color::LightRed).add_modifier(Modifier::BOLD),
        1 => Style::new().fg(Color::Red),
        _ => dim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};
    use crate::nav::Path;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn issues() -> Vec<Issue> {
        let mut epic = Issue::new(
            "argos-0001".into(),
            "아주 긴 한글 에픽 제목이 여기 들어간다 잘려야 한다".into(),
            Kind::Epic,
            Status::new("in_progress"),
            "2026-09-01T00:00:00Z",
        );
        epic.priority = Some(1);
        let mut member = Issue::new(
            "argos-0003".into(),
            "멤버".into(),
            Kind::Issue,
            Status::new("done"),
            "2026-09-01T00:00:00Z",
        );
        member.epic = Some("argos-0001".into());
        vec![epic, member]
    }

    /// 그려 보고 **글자만** 꺼낸다. 색은 여기서 따지지 않는다 —
    /// 색이 혼자 뜻을 지지 않는다는 규칙이 참이면 글자만으로 읽혀야 한다.
    ///
    /// 두 칸짜리 글자는 칸 하나에 담기고 **다음 칸은 공백으로 채워진다.**
    /// 그대로 이어 붙이면 "상 세" 가 되어, 있는 글자를 못 찾고 폭도 부풀어
    /// 센다. 앞 글자의 폭만큼 건너뛰어야 화면에 있는 것과 같은 줄이 된다.
    pub(super) fn render(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| screen(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                let mut line = String::new();
                let mut skip = 0usize;
                for x in 0..buf.area.width {
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    let sym = buf[(x, y)].symbol();
                    line.push_str(sym);
                    skip = crate::text::width(sym).saturating_sub(1);
                }
                line.trim_end().to_string()
            })
            .collect()
    }

    fn app() -> App {
        App::new(issues(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new())
    }

    /// 색 없이 글자만 봐도 읽힌다 — 경로, id, 우선순위, 칸 글리프, 제목, F키 바.
    #[test]
    fn the_screen_reads_without_colour() {
        let lines = render(&app(), 100, 12).join("\n");
        assert!(lines.contains('/'), "경로가 없다\n{lines}");
        assert!(lines.contains("argos-0001"), "id 가 없다\n{lines}");
        assert!(lines.contains("p1"), "우선순위가 없다\n{lines}");
        assert!(lines.contains('▸'), "칸 글리프가 없다\n{lines}");
        assert!(lines.contains("Enter") && lines.contains("F10"), "F키 바가 없다\n{lines}");
        // 긴 제목은 **잘린다**. 잘렸다는 표시가 남아야 어디까지가 제목인지 안다.
        assert!(lines.contains("아주 긴"), "에픽 제목이 없다\n{lines}");
        assert!(lines.contains('…'), "잘렸는데 표시가 없다\n{lines}");
        // 디렉터리는 제목 뒤에 `/` 가 붙는다
    }

    /// 커서 줄은 **색만으로** 표시하지 않는다. `>` 가 함께 있어야 한다.
    #[test]
    fn the_cursor_is_marked_with_a_glyph_not_only_colour() {
        let lines = render(&app(), 100, 12);
        assert!(lines.iter().any(|l| l.contains('>')), "{lines:?}");
    }

    /// 좁은 창에서 무너지지도, 넘치지도 않는다. 한글이 두 칸을 먹는 것이
    /// 여기서 드러난다 — `len()` 으로 잘랐으면 줄이 테두리를 넘는다.
    #[test]
    fn narrow_windows_neither_panic_nor_overflow() {
        for w in [20, 24, 30, 40, 60, 100] {
            let lines = render(&app(), w, 10);
            for l in &lines {
                assert!(
                    crate::text::width(l) <= w as usize,
                    "폭 {w} 에서 줄이 넘쳤다: {l:?} ({}칸)",
                    crate::text::width(l)
                );
            }
        }
    }

    /// 빈 저장소도 그려진다.
    #[test]
    fn an_empty_repo_still_draws() {
        let empty = App::new(Vec::new(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let lines = render(&empty, 60, 10).join("\n");
        assert!(lines.contains("비었다"), "{lines}");
    }
}

#[cfg(test)]
mod eyeball {
    /// 눈으로 보려고 두는 것. `cargo test -- --ignored --nocapture eyeball` 로 부른다.
    #[test]
    #[ignore]
    fn print_the_screen() {
        let load = crate::store::parse_issues(
            &std::fs::read_to_string(".moai/issues.jsonl").unwrap_or_default(),
        );
        let app = super::App::new(
            load.issues,
            crate::config::Config::parse("prefix = \"moai\"\n").unwrap(),
            crate::nav::Path::new(),
        );
        for l in super::tests::render(&app, 96, 16) {
            println!("{l}");
        }
    }
}
