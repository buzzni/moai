//! 그림. **뜻을 판단하지 않는다** — 무엇이 어디 걸리는지는 `nav`, 무엇을
//! 세는지는 `report` 가 이미 정했다.
//!
//! 규칙 하나를 CLI 에서 그대로 들고 온다: **색이 혼자 뜻을 지지 않는다.**
//! 모든 색에 글리프나 낱말이 붙는다.

use super::{App, Mode, Row};
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
    detail(f, app, right);
    // 맨 아랫줄은 하나다 — 글을 받는 중이면 프롬프트가, 아니면 F키 바가 선다.
    match &app.mode {
        Mode::Browse => fkeys(f, app, keys),
        Mode::Grep(q) => prompt(f, app, keys, "검색", q),
        Mode::Filter(q) => prompt(f, app, keys, "거름망", q),
    }
}

fn crumbs(f: &mut Frame, app: &App, at: Rect) {
    let mut spans = vec![Span::styled(
        clip(&app.crumbs(), at.width as usize),
        Style::new().add_modifier(Modifier::BOLD),
    )];
    // **걸린 거름망은 늘 보인다.** 안 보이면 왜 줄이 적은지 알 길이 없고,
    // 그러면 사람이 도구를 의심하는 대신 자료를 의심한다.
    if let Some(t) = &app.filter_text {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!("[{t}]  Esc 로 푼다"),
            Style::new().fg(Color::Black).bg(Color::LightYellow),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), at);
}

/// 글을 받는 줄. 잘못 적은 거름망은 그 자리에서 말해 준다.
fn prompt(f: &mut Frame, app: &App, at: Rect, what: &str, buf: &str) {
    let mut spans = vec![
        Span::styled(format!(" {what} "), Style::new().fg(Color::Black).bg(Color::LightBlue)),
        Span::raw(" "),
        Span::raw(buf.to_string()),
        Span::styled("▌", Style::new().fg(Color::LightBlue)),
    ];
    match app.input_error() {
        Some(e) => {
            spans.push(Span::raw("   "));
            // 여러 줄짜리 도움말은 첫 줄만 — 한 줄 자리다.
            let first = e.lines().next().unwrap_or_default().to_string();
            spans.push(Span::styled(first, Style::new().fg(Color::LightRed)));
        }
        None => {
            spans.push(Span::raw("   "));
            spans.push(Span::styled("Enter 걸기  Esc 그만", dim()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), at);
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

/// 커서가 머문 것을 정리해 낸다. 이슈면 그 이슈를, 디렉터리면 그 밑의 셈을.
fn detail(f: &mut Frame, app: &App, at: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" 상세 ");
    let inner = block.inner(at);
    f.render_widget(block, at);

    let lines = match app.current() {
        None => vec![Line::from(Span::styled("없다", dim()))],
        Some(Row::Up) => vec![Line::from(Span::styled("한 층 위로", dim()))],
        Some(Row::Item(e)) => match e.at() {
            Some(idx) => about(app, idx, &e, inner.width as usize),
            // 바구니는 제 줄이 없다. 밑에 무엇이 있는지만 센다.
            None => {
                let mut out = vec![
                    Line::from(Span::styled(app.index.label(&app.issues, &e), bold())),
                    Line::from(""),
                ];
                out.extend(rollup(app, &deeper(app, &e), inner.width as usize));
                out
            }
        },
    };
    f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), inner);
}

/// 그 항목 안으로 들어간 경로. 요약을 세려면 그 밑을 봐야 한다.
fn deeper(app: &App, e: &Entry) -> crate::nav::Path {
    let mut p = app.path.clone();
    if let Entry::Dir { seg, .. } = e {
        p.push(seg.clone());
    }
    p
}

/// 이슈 하나의 낱낱.
fn about<'a>(app: &App, idx: usize, e: &Entry, w: usize) -> Vec<Line<'a>> {
    let i = &app.issues[idx];
    let mut out = vec![
        Line::from(Span::styled(i.id.clone(), dim())),
        Line::from(Span::styled(i.title.clone(), bold())),
        Line::from(""),
    ];

    // 칸은 글리프와 낱말을 함께 낸다. 색이 없어도 뜻이 남아야 한다.
    let st = i.status.as_str().to_string();
    let mut head = vec![
        Span::styled(format!("{} {st}", style::glyph(&st)), status(&st)),
        Span::raw("  ·  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
    ];
    if i.kind != crate::model::Kind::Issue {
        head.push(Span::raw("  ·  "));
        head.push(Span::styled(i.kind.as_str().to_string(), Style::new().fg(Color::LightBlue)));
    }
    out.push(Line::from(head));

    if !i.tags.is_empty() {
        let tags = i.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
        out.push(Line::from(Span::styled(tags, Style::new().fg(Color::Cyan))));
    }
    if let Some(a) = &i.assignee {
        out.push(field("담당", a));
    }
    if let Some(id) = &i.epic {
        out.push(field("에픽", &app.title_of(id)));
    }
    if let Some(id) = &i.milestone {
        out.push(field("마일스톤", &app.title_of(id)));
    }
    // **막는 것은 제목까지 푼다.** id 만 내면 그것이 무엇인지 또 찾아봐야 한다.
    for b in &i.blocked_by {
        out.push(field("막힘", &format!("{b}  {}", app.title_of(b))));
    }
    out.push(field("생성", &stamp(&i.created_at)));
    out.push(field("수정", &stamp(&i.updated_at)));

    // 디렉터리면 그 밑의 셈도 함께.
    if matches!(e, Entry::Dir { .. }) {
        out.push(Line::from(""));
        out.extend(rollup(app, &deeper(app, e), w));
    }

    if let Some(body) = &i.body {
        out.push(Line::from(""));
        out.push(Line::from(Span::styled("─".repeat(w.min(40)), dim())));
        // **파일에서 온 글이다.** 제어문자를 걸러서 그린다.
        for l in crate::text::sanitize(body).lines() {
            out.push(Line::from(l.to_string()));
        }
    }
    out
}

/// 그 밑의 진척. **`nav` 가 자리를 정한 그대로 센다** — `report::rollup_of` 로
/// 세면 자리 규칙과 세는 규칙이 달라 머리글과 줄 수가 어긋난다.
fn rollup<'a>(app: &App, path: &crate::nav::Path, w: usize) -> Vec<Line<'a>> {
    let kids = app.index.descendants(path);
    let work: Vec<usize> =
        kids.iter().copied().filter(|&at| crate::report::is_work(&app.issues[at])).collect();
    if work.is_empty() {
        return vec![Line::from(Span::styled("자식 없음", dim()))];
    }
    let done = work.iter().filter(|&&at| app.issues[at].status.is_done()).count();
    let percent = (done * 100 / work.len()) as u8;

    let cells = w.clamp(10, 24).saturating_sub(4).max(4);
    let filled = crate::text::bar_fill(Some(percent), cells);
    let mut out = vec![Line::from(vec![
        Span::styled("█".repeat(filled), Style::new().fg(Color::Green)),
        Span::styled("░".repeat(cells - filled), dim()),
        Span::raw(format!("  {done}/{}  {percent}%", work.len())),
    ])];

    // 칸별 건수는 `config` 차례로. **0인 칸은 빼서** 좁은 패널에서 줄이 접히지
    // 않게 한다 — CLI 요약이 쓰는 규칙과 같다.
    let counts: Vec<Span> = app
        .cfg
        .statuses
        .iter()
        .filter_map(|st| {
            let n = work.iter().filter(|&&at| app.issues[at].status.as_str() == st).count();
            (n > 0).then(|| Span::styled(format!("{} {st} {n}   ", style::glyph(st)), status(st)))
        })
        .collect();
    out.push(Line::from(counts));
    out
}

fn field<'a>(k: &str, v: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{k:<5}"), dim()),
        Span::raw(" "),
        Span::raw(v.to_string()),
    ])
}

/// `2026-09-11T15:18:26Z` → `09-11 15:18`.
fn stamp(at: &str) -> String {
    match (at.get(5..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_string(),
    }
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// 맨 아래 MC 풍 F키 바. **아직 없는 것은 적지 않는다** — 눌러도 아무 일이
/// 없는 키를 적어 두면 그것부터 도구를 못 믿게 된다.
fn fkeys(f: &mut Frame, app: &App, at: Rect) {
    let mut spans = vec![
        key("Enter", "들어가기"),
        key("Backspace", "나가기"),
        key("/", "검색"),
        key("f", "거름망"),
    ];
    if app.filter_text.is_some() {
        spans.push(key("Esc", "풀기"));
    }
    spans.push(key("F10", "끝내기"));
    f.render_widget(Paragraph::new(Line::from(spans)), at);
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
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    /// 커서가 디렉터리면 그 밑의 셈이, 이슈면 그 낱낱이 오른쪽에 나온다.
    #[test]
    fn the_right_pane_follows_the_cursor() {
        let mut a = app();
        // 커서가 에픽(디렉터리)에 있다 → 진행과 칸별 건수
        let lines = render(&a, 100, 16).join("\n");
        assert!(lines.contains("1/1") || lines.contains("0/1"), "진행이 없다\n{lines}");
        assert!(lines.contains("done"), "칸별 건수가 없다\n{lines}");

        // 에픽 안으로 들어가 멤버(잎)를 본다 → 그 이슈의 낱낱
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let lines = render(&a, 100, 16).join("\n");
        assert!(lines.contains("argos-0003"), "이슈 id 가 없다\n{lines}");
        assert!(lines.contains("멤버"), "제목이 없다\n{lines}");
        // 칸은 글리프와 낱말을 함께 낸다
        assert!(lines.contains("✓ done"), "칸이 낱말 없이 나왔다\n{lines}");
        // 소속은 id 가 아니라 제목으로 푼다
        assert!(lines.contains("에픽"), "에픽 줄이 없다\n{lines}");
    }

    /// 막는 것은 **제목까지 풀어서** 낸다. id 만 내면 또 찾아봐야 한다.
    #[test]
    fn blockers_are_resolved_to_titles() {
        let mut issues = issues();
        issues[1].blocked_by = vec!["argos-0001".into()];
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let lines = render(&a, 100, 20).join("\n");
        assert!(lines.contains("막힘"), "{lines}");
        assert!(lines.contains("아주 긴"), "막는 것의 제목이 없다\n{lines}");
    }

    /// 본문에 든 ESC 가 화면을 다시 칠하지 못한다.
    #[test]
    fn a_body_cannot_repaint_the_screen() {
        let mut issues = issues();
        issues[1].body = Some("앞\u{1b}[2J뒤".into());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let lines = render(&a, 100, 20).join("\n");
        assert!(lines.contains("앞[2J뒤"), "제어문자가 안 걸러졌다\n{lines}");
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
        let start = std::env::var("EYE_PATH").ok();
        let cfg = crate::config::Config::parse("prefix = \"moai\"\n").unwrap();
        let index = crate::nav::Index::of(&load.issues);
        let path = match start.as_deref().and_then(|id| load.issues.iter().position(|i| i.id == id)) {
            Some(at) => {
                let mut p = index.home_of(at).clone();
                p.push(crate::nav::Seg::Epic(load.issues[at].id.clone()));
                p
            }
            None => crate::nav::Path::new(),
        };
        let mut app = super::App::new(load.issues, cfg, path);
        app.cursor = std::env::var("EYE_CUR").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        if let Ok(q) = std::env::var("EYE_GREP") {
            let m = super::Mode::Grep(q);
            let _ = app.apply(&m);
        }
        if let Ok(q) = std::env::var("EYE_TYPING") {
            app.mode = super::Mode::Filter(q);
        }
        let h: u16 = std::env::var("EYE_H").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
        for l in super::tests::render(&app, 96, h) {
            println!("{l}");
        }
    }
}
