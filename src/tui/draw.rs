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
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;

/// 좌우를 가르는 자리. MC 처럼 반반이되 왼쪽을 조금 넓게 — 제목이 길다.
const LEFT: u16 = 55;

/// 커서 자리. 목록의 글자는 늘 이만큼 안쪽에서 시작한다.
///
/// **우측 패널의 좌우 여백도 이 값이다.** 한쪽만 테두리에 붙으면 같은 화면에서
/// 규칙이 둘이 되고, 붙은 쪽이 답답하게 읽힌다. 한 자리에서 정해 두 패널이
/// 갈라지지 않게 한다.
const LEFT_GUTTER: usize = 2;
const CURSOR: &str = "> ";

pub fn screen(f: &mut Frame, app: &mut App) {
    // 할 말이 있을 때만 배너 줄이 선다. 늘 세워 두면 한 줄이 영영 논다.
    let banner_h = u16::from(banner(app).is_some());
    let [top, note, body, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(banner_h),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(LEFT), Constraint::Min(10)]).areas(body);

    crumbs(f, app, top);
    if let Some((text, urgent)) = banner(app) {
        let style = if urgent {
            Style::new().fg(Color::Black).bg(Color::LightRed)
        } else {
            Style::new().fg(Color::Black).bg(Color::LightYellow)
        };
        let text = clip(&text, note.width as usize);
        f.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), note);
    }
    // 훑는 자리는 App 이 들고 있다. 잠깐 꺼내 그리고 도로 넣는다 — 그래야
    // 나머지 그리기가 `&App` 만 빌리면 된다.
    let mut state = std::mem::take(&mut app.list);
    list(f, app, left, &mut state);
    app.list = state;
    detail(f, app, right);
    // 맨 아랫줄은 하나다 — 글을 받는 중이면 프롬프트가, 아니면 F키 바가 선다.
    match &app.mode {
        Mode::Browse => fkeys(f, app, keys),
        Mode::Grep(q) => prompt(f, app, keys, "검색", q),
        Mode::Filter(q) => prompt(f, app, keys, "거름망", q),
    }
}

/// 화면 안에서 알려야 할 것. **대체 화면 안에서는 `eprintln!` 이 화면을
/// 망가뜨린다** — 적재 오류를 stderr 로 흘리던 CLI 의 길을 여기서는 못 쓴다.
///
/// 한 줄만 쓴다 — 줄을 여럿 세우면 목록이 그만큼 짧아진다. 하지만 **하나만
/// 고르지는 않는다**: 못 읽는 줄은 사람이 파일을 고칠 때까지 붙박이고 "바뀌었다"
/// 는 지나가는 것이라, 붙박이가 이기면 지나가는 알림은 영영 안 보인다.
/// 지금 할 일이 있는 것부터 앞에 놓고 이어 붙인다.
fn banner(app: &App) -> Option<(String, bool)> {
    let mut parts: Vec<String> = Vec::new();
    let mut urgent = false;
    if let Some(t) = &app.trouble {
        parts.push(format!("다시 읽지 못했다 — {t}"));
        urgent = true;
    }
    if app.stale {
        parts.push("파일이 바뀌었다 — F5 로 다시 읽는다".into());
    }
    if app.unreadable > 0 {
        parts.push(format!("읽을 수 없는 줄 {}개 — 그 줄은 빠진 채로 보고 있다", app.unreadable));
        urgent = true;
    }
    if app.warnings > 0 {
        parts.push(format!("드러난 것 {}건 — `moai status` 가 자세히 낸다", app.warnings));
    }
    (!parts.is_empty()).then(|| (format!(" ! {} ", parts.join("   ·   ")), urgent))
}

fn crumbs(f: &mut Frame, app: &App, at: Rect) {
    let w = at.width as usize;
    // **걸린 거름망은 늘 보인다.** 안 보이면 왜 줄이 적은지 알 길이 없고,
    // 그러면 사람이 도구를 의심하는 대신 자료를 의심한다. 그래서 **뱃지 자리를
    // 먼저 뗀다** — 경로를 줄 폭 전체로 자르면 깊이 들어갔을 때 뱃지가 줄
    // 밖으로 밀려 통째로 사라지고, 하필 그때가 목록이 가장 짧아 보이는 때다.
    let badge = app.filter_text.as_ref().map(|t| clip(&format!("[{t}]  Esc 로 푼다"), w));
    let room = match &badge {
        Some(b) => w.saturating_sub(crate::text::width(b) + 3),
        None => w,
    };
    let mut spans = vec![Span::styled(clip(&app.crumbs(), room), bold())];
    if let Some(b) = badge {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(b, Style::new().fg(Color::Black).bg(Color::LightYellow)));
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

fn list(f: &mut Frame, app: &App, at: Rect, state: &mut ListState) {
    let rows = app.rows();
    // 테두리 두 칸을 뺀 안쪽 폭. 좁은 창에서도 음수가 되지 않게 막는다.
    let inner = at.width.saturating_sub(2) as usize;
    let items: Vec<ListItem> = rows.iter().map(|r| ListItem::new(row_line(app, r, inner))).collect();

    // 제목에 칸별 건수를 **config 차례로** 낸다. 칸 이름과 순서는 저장소가
    // 정하는 것이라(`config.statuses`) 여기서 다시 정하지 않는다.
    //
    // **일만 센다.** 에픽도 마일스톤도 묶음이지 일이 아니라고 `report::is_work`
    // 가 정했고, `moai status` 와 오른쪽 롤업이 그 자로 센다 — 여기만 묶음을
    // 같이 세면 한 화면의 두 패널이 같은 디렉터리를 두 수로 말한다.
    let work: Vec<usize> = rows
        .iter()
        .filter_map(|r| match r {
            Row::Item(e) => e.at(),
            Row::Up => None,
        })
        .filter(|&at| crate::report::is_work(&app.issues[at]))
        .collect();
    let counts: Vec<String> = app
        .cfg
        .statuses
        .iter()
        .filter_map(|st| {
            let n = work.iter().filter(|&&at| app.issues[at].status.as_str() == st).count();
            // 글리프만으로는 뜻이 약하다. 칸 이름을 같이 적는다.
            (n > 0).then(|| format!("{} {st} {n}", style::glyph(st)))
        })
        .collect();
    // **줄이 있으면 "비었다" 라고 하지 않는다.** 셈은 config 에 있는 칸의 일만
    // 세므로, 묶음만 있는 디렉터리·바구니만 있는 디렉터리·config 에 없는 칸에
    // 선 줄에서는 비어 있고, 그때 제목이 목록과 정면으로 어긋난다.
    let title = match (counts.is_empty(), rows.is_empty()) {
        (_, true) => " 비었다 ".to_string(),
        (true, false) => format!(" {}줄 ", rows.len()),
        (false, _) => format!(" {} ", counts.join("  ")),
    };
    // **자리는 프레임을 넘어 산다.** `ListState` 를 매번 새로 만들면 훑는 자리가
    // 0 으로 돌아가, 위젯이 커서를 보이게 하려고 커서를 늘 맨 아랫줄에 붙인다 —
    // 커서 아래를 한 줄도 못 보게 된다.
    state.select((!rows.is_empty()).then_some(app.cursor.min(rows.len().saturating_sub(1))));
    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            // 커서는 **색만으로 표시하지 않는다** — 반전과 `>` 를 함께 준다.
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(CURSOR),
        at,
        state,
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

    // 앞에 붙는 것들을 **먼저 만들고 재서** 남는 만큼을 제목에 준다. 손으로
    // 더한 숫자로 어림하면 `p10` 처럼 자리를 더 먹는 값이나 한글이 든 id 에서
    // 어긋나고, 넘친 줄은 위젯이 말없이 잘라 내 **잘렸다는 `…` 마저** 사라진다.
    let head = vec![
        Span::styled(i.id.clone(), dim()),
        Span::raw("  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
        Span::raw(" "),
        // 칸은 글리프로도 말한다. 색이 없는 터미널에서도 뜻이 남아야 한다.
        Span::styled(style::glyph(i.status.as_str()).to_string(), status(i.status.as_str())),
        Span::raw(" "),
    ];
    // `> ` 커서 자리 두 칸 + 머리글 폭.
    let used = 2 + head.iter().map(|s| crate::text::width(&s.content)).sum::<usize>();
    let mut title = clip(&app.index.label(&app.issues, e), budget.saturating_sub(used));
    // **디렉터리 표시는 자른 뒤에 붙인다.** 먼저 붙이면 긴 제목에서 `/` 가
    // 제일 먼저 잘려 나가고, 목록에는 디렉터리라고 말하는 것이 달리 없다.
    if is_dir {
        if crate::text::width(&title) + 1 > budget.saturating_sub(used) {
            title = clip(&title, budget.saturating_sub(used + 1));
        }
        title.push('/');
    }

    let mut spans = head;
    spans.push(Span::raw(title));
    Line::from(spans)
}

/// 커서가 머문 것을 정리해 낸다. 이슈면 그 이슈를, 디렉터리면 그 밑의 셈을.
fn detail(f: &mut Frame, app: &App, at: Rect) {
    // 좌우 여백은 목록의 커서 자리와 같은 폭이다. `inner()` 가 테두리와 여백을
    // 함께 빼 주므로 폭 계산은 아래가 그대로 쓴다.
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(LEFT_GUTTER as u16))
        .title(" 상세 ");
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
    // **끝난 막음은 막지 않는다** — `report::is_blocked` 와 `moai ready` 가 그
    // 자로 세므로, 여기서만 다 "막힘" 이라 적으면 집을 수 있는 것을 못 집을
    // 것으로 읽는다. 뜻은 `report` 가 정하고 여기는 그 답을 그린다.
    for b in &i.blocked_by {
        let done = app.index.find(b).is_some_and(|at| app.issues[at].status.is_done());
        let (label, mark) = if done { ("풀림", "✓") } else { ("막힘", "·") };
        out.push(field(label, &format!("{mark} {b}  {}", app.title_of(b))));
    }
    // CLI 상세와 **같은 자**를 쓴다. 두 표면이 같은 값을 다르게 적으면 보는
    // 쪽이 어느 쪽을 믿을지 정해야 한다.
    out.push(field("생성", &crate::view::stamp(&i.created_at)));
    out.push(field("수정", &crate::view::stamp(&i.updated_at)));

    // 디렉터리면 그 밑의 셈도 함께.
    if matches!(e, Entry::Dir { .. }) {
        out.push(Line::from(""));
        out.extend(rollup(app, &deeper(app, e), w));
    }

    if let Some(body) = &i.body {
        out.push(Line::from(""));
        out.push(Line::from(Span::styled("─".repeat(w.min(40)), dim())));
        out.extend(body_lines(body, w, app.raw));
    }
    out
}

/// 본문. **줄로 펴는 일은 `markdown` 이 한다** — 글머리·들여쓰기 같은 결정이
/// 표면마다 갈라지면 CLI 와 탐색기가 같은 본문을 다르게 그린다. 여기가 할 일은
/// 뜻을 색으로 옮기는 것뿐이다.
fn body_lines<'a>(body: &str, w: usize, raw: bool) -> Vec<Line<'a>> {
    // **파일에서 온 글이다.** 파서를 거쳐도 조각 안에 ESC 가 남으므로 먼저 거른다.
    let clean = crate::text::sanitize(body);
    if raw {
        return clean.lines().map(|l| Line::from(l.to_string())).collect();
    }
    let blocks = crate::markdown::parse(&clean);
    crate::markdown::layout(&blocks, w.max(8))
        .into_iter()
        .map(|line| {
            Line::from(
                line.into_iter()
                    .map(|s| Span::styled(s.text, role_style(s.role)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// 뜻을 ratatui 색으로. **CLI(`view::role_style`)와 같은 뜻이 같은 모양이어야
/// 한다** — 두 표면을 나란히 놓고 보는 사람이 어느 쪽을 믿을지 정하게 두지 않는다.
fn role_style(r: crate::markdown::Role) -> Style {
    use crate::markdown::Role;
    match r {
        Role::Plain => Style::new(),
        Role::Strong | Role::Heading => Style::new().add_modifier(Modifier::BOLD),
        Role::Emphasis => Style::new().add_modifier(Modifier::ITALIC),
        Role::Code => Style::new().fg(Color::Cyan),
        Role::Link => Style::new().fg(Color::LightBlue).add_modifier(Modifier::DIM),
        Role::Mark => dim(),
    }
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

    // `clamp(10, 24)` 뒤에는 10 이상이라 뺄셈이 넘칠 수 없고 6 아래로도 안
    // 간다 — 지키는 척하는 `.saturating_sub`·`.max` 는 지우고 뜻만 남긴다.
    let cells = w.clamp(10, 24) - 4;
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

/// 이름 칸을 **표시 폭으로** 맞춘다. `{k:<5}` 는 글자 수를 세므로 `에픽`(2자
/// 4칸)과 `마일스톤`(4자 8칸)이 두 칸 어긋나 값이 들쭉날쭉해진다 — `text`
/// 모듈이 있는 까닭이 바로 이것이다.
const LABEL: usize = 9;

fn field<'a>(k: &str, v: &str) -> Line<'a> {
    let pad = LABEL.saturating_sub(crate::text::width(k));
    Line::from(vec![
        Span::styled(format!("{k}{}", " ".repeat(pad)), dim()),
        Span::raw(" "),
        Span::raw(v.to_string()),
    ])
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
        key("F5", "갱신"),
        key("F3", if app.raw { "그리기" } else { "원문" }),
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

/// `style` 이 정한 색을 ratatui 쪽으로 **옮기기만** 한다.
///
/// 표를 여기 다시 적지 않는다. 손으로 옮겨 적은 표는 반드시 갈라지고, 실제로
/// 갈라졌다 — 우선순위 표가 `style` 의 "기본값(p2)은 칠하지 않는다" 를 어기고
/// p2 를 흐리게 칠해, 죄다 p2 인 저장소에서 그 열 전체가 죽어 있었다.
fn from_anstyle(s: anstyle::Style) -> Style {
    let mut out = Style::new();
    if let Some(anstyle::Color::Ansi(c)) = s.get_fg_color() {
        out = out.fg(ansi(c));
    }
    let e = s.get_effects();
    if e.contains(anstyle::Effects::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if e.contains(anstyle::Effects::DIMMED) {
        out = out.add_modifier(Modifier::DIM);
    }
    out
}

fn ansi(c: anstyle::AnsiColor) -> Color {
    use anstyle::AnsiColor as A;
    match c {
        A::Black => Color::Black,
        A::Red => Color::Red,
        A::Green => Color::Green,
        A::Yellow => Color::Yellow,
        A::Blue => Color::Blue,
        A::Magenta => Color::Magenta,
        A::Cyan => Color::Cyan,
        A::White => Color::Gray,
        A::BrightBlack => Color::DarkGray,
        A::BrightRed => Color::LightRed,
        A::BrightGreen => Color::LightGreen,
        A::BrightYellow => Color::LightYellow,
        A::BrightBlue => Color::LightBlue,
        A::BrightMagenta => Color::LightMagenta,
        A::BrightCyan => Color::LightCyan,
        A::BrightWhite => Color::White,
    }
}

fn status(s: &str) -> Style {
    from_anstyle(style::status_style(s))
}

fn priority(p: u8) -> Style {
    from_anstyle(style::priority_style(p))
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
    /// **`&mut` 다.** 훑는 자리는 프레임을 넘어 살아야 하므로 `screen` 이
    /// App 에 되적는다 — 시험도 진짜 화면과 같은 길을 지난다.
    pub(super) fn render(app: &mut App, w: u16, h: u16) -> Vec<String> {
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
        let lines = render(&mut app(), 100, 12).join("\n");
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
        let lines = render(&mut app(), 100, 12);
        assert!(lines.iter().any(|l| l.contains('>')), "{lines:?}");
    }

    /// 좁은 창에서 무너지지도, 넘치지도 않는다. 한글이 두 칸을 먹는 것이
    /// 여기서 드러난다 — `len()` 으로 잘랐으면 줄이 테두리를 넘는다.
    #[test]
    fn narrow_windows_neither_panic_nor_overflow() {
        for w in [20, 24, 30, 40, 60, 100] {
            let lines = render(&mut app(), w, 10);
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
        let lines = render(&mut a, 100, 16).join("\n");
        assert!(lines.contains("1/1") || lines.contains("0/1"), "진행이 없다\n{lines}");
        assert!(lines.contains("done"), "칸별 건수가 없다\n{lines}");

        // 에픽 안으로 들어가 멤버(잎)를 본다 → 그 이슈의 낱낱
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 16).join("\n");
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
        let lines = render(&mut a, 100, 20).join("\n");
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
        let lines = render(&mut a, 100, 20).join("\n");
        assert!(lines.contains("앞[2J뒤"), "제어문자가 안 걸러졌다\n{lines}");
    }

    /// 못 읽는 줄은 **화면 안에서** 알린다. 대체 화면 안에서 stderr 로 흘리면
    /// 화면이 망가진다.
    #[test]
    fn load_errors_are_told_inside_the_screen() {
        let mut a = app();
        a.unreadable = 3;
        let lines = render(&mut a, 100, 14).join("\n");
        assert!(lines.contains("읽을 수 없는 줄 3개"), "{lines}");
    }

    /// 파일이 바뀌면 말만 하고 **저절로 읽지 않는다**.
    #[test]
    fn a_changed_file_is_announced_not_swallowed() {
        let mut a = app();
        a.stale = true;
        let lines = render(&mut a, 100, 14).join("\n");
        assert!(lines.contains("F5"), "{lines}");
    }

    /// 디렉터리 표시 `/` 는 **잘려 나가지 않는다.** 목록에서 디렉터리라고
    /// 말하는 것이 그것 하나뿐이라, 긴 제목에서 먼저 잘리면 폴더와 파일이
    /// 구별되지 않는다.
    #[test]
    fn a_clipped_directory_keeps_its_slash() {
        for w in [40, 60, 80, 100] {
            let lines = render(&mut app(), w, 12);
            let row = lines.iter().find(|l| l.contains("argos-0001")).unwrap_or_else(|| {
                panic!("에픽 줄이 없다 (폭 {w})\n{lines:#?}")
            });
            assert!(row.contains('/'), "폭 {w} 에서 디렉터리 표시가 잘려 나갔다: {row:?}");
        }
    }

    /// 머리글은 **일만 센다.** 묶음까지 세면 같은 화면의 왼쪽과 오른쪽이 같은
    /// 디렉터리를 두 수로 말하고, 왼쪽은 `moai status` 와도 어긋난다.
    #[test]
    fn the_header_counts_work_not_groupings() {
        // 뿌리에는 에픽(in_progress) 한 줄뿐 — 묶음은 일이 아니라 칸 셈이 안 선다
        let mut a = app();
        let lines = render(&mut a, 100, 12);
        let title = lines.iter().find(|l| l.contains('┌')).unwrap().clone();
        assert!(!title.contains("in_progress"), "에픽을 일로 셌다 — {title:?}");
        assert!(title.contains("1줄"), "{title:?}");

        // 그 안에는 일이 하나 있다 — 이제 칸 셈이 선다
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 12);
        let title = lines.iter().find(|l| l.contains('┌')).unwrap();
        assert!(title.contains("done 1"), "{title:?}");
    }

    /// **줄이 있으면 "비었다" 라고 하지 않는다.** 셈이 비는 경우(바구니만 있는
    /// 디렉터리, config 에 없는 칸)와 진짜 빈 것은 다르다.
    #[test]
    fn a_pane_with_rows_never_calls_itself_empty() {
        let mut lost = Issue::new(
            "argos-0009".into(),
            "없는 에픽을 가리킨다".into(),
            Kind::Issue,
            Status::new("todo"),
            "2026-09-01T00:00:00Z",
        );
        lost.epic = Some("argos-zzzz".into());
        let mut a = App::new(vec![lost], Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let lines = render(&mut a, 60, 10);
        let title = lines.iter().find(|l| l.contains('┌')).unwrap();
        assert!(lines.iter().any(|l| l.contains("길 잃음")), "{lines:#?}");
        assert!(!title.contains("비었다"), "줄이 있는데 비었다고 한다 — {title:?}");
    }

    /// **끝난 막음은 막지 않는다.** `report::is_blocked` 와 `moai ready` 가 그
    /// 자로 세므로, 여기서만 다 "막힘" 이라 적으면 집을 수 있는 것을 못 집을
    /// 것으로 읽는다.
    #[test]
    fn a_finished_blocker_is_not_drawn_as_blocking() {
        let mut issues = issues();
        issues[0].status = Status::new("done"); // 막는 쪽이 끝났다
        issues[1].blocked_by = vec!["argos-0001".into()];
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 20).join("\n");
        assert!(lines.contains("풀림"), "끝난 막음을 아직 막혔다고 그린다\n{lines}");
        assert!(!lines.contains("막힘"), "{lines}");
    }

    /// 커서가 아래로 가도 **그 아래가 보인다.** 훑는 자리를 프레임마다 새로
    /// 만들면 위젯이 커서를 늘 맨 아랫줄에 붙여, 커서 밑을 한 줄도 못 본다.
    #[test]
    fn the_viewport_does_not_pin_the_cursor_to_the_last_line() {
        let issues: Vec<Issue> = (1..=12)
            .map(|n| {
                Issue::new(
                    format!("argos-{n:04}"),
                    format!("일 {n}"),
                    Kind::Issue,
                    Status::new("todo"),
                    "2026-09-01T00:00:00Z",
                )
            })
            .collect();
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        // 창은 12줄 — 목록 안쪽은 그보다 짧다. 커서를 다섯 칸 내린다.
        for _ in 0..5 {
            a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            let _ = render(&mut a, 60, 12);
        }
        let lines = render(&mut a, 60, 12);
        let at = lines.iter().position(|l| l.contains('>')).expect("커서 줄이 없다");
        let last = lines.iter().rposition(|l| l.contains("argos-")).unwrap();
        assert!(at < last, "커서 밑이 한 줄도 안 보인다\n{lines:#?}");
    }

    /// 이름 칸은 **표시 폭으로** 맞춘다. 글자 수로 맞추면 `에픽`(4칸)과
    /// `마일스톤`(8칸)이 두 칸 어긋나 값이 들쭉날쭉해진다 — `text` 모듈이
    /// 있는 까닭이 바로 이것이다.
    #[test]
    fn detail_labels_line_up_by_display_width() {
        // 값은 마지막 span 이다. 그 앞의 폭이 값이 시작하는 칸이다.
        let starts_at = |l: &Line| {
            l.spans[..l.spans.len() - 1].iter().map(|s| crate::text::width(&s.content)).sum::<usize>()
        };
        let short = field("에픽", "값");
        let long = field("마일스톤", "값");
        assert_eq!(starts_at(&short), starts_at(&long), "값이 다른 칸에서 시작한다");
        assert!(starts_at(&long) > crate::text::width("마일스톤"), "이름과 값이 붙었다");
    }

    /// 우측이 테두리에 붙지 않는다. 좌측은 커서 자리(`> `) 덕에 글자가 늘 두 칸
    /// 안쪽에서 시작하는데, 우측만 붙어 있으면 같은 화면에서 규칙이 둘이 되고
    /// 붙은 쪽이 답답하게 읽힌다.
    #[test]
    fn the_detail_pane_is_not_glued_to_its_border() {
        let lines = render(&mut app(), 100, 14);
        // 테두리가 맞붙어 `││` 라 가운데 빈 조각이 낀다. 빈 조각을 빼면
        // 앞이 좌측, 뒤가 우측이다.
        let panes = |l: &str| -> Vec<String> {
            l.split('│').filter(|s| !s.is_empty()).map(str::to_string).collect()
        };
        let inset = |seg: &str| seg.len() - seg.trim_start_matches(' ').len();

        let row = lines
            .iter()
            .find(|l| {
                let p = panes(l);
                p.len() == 2 && !p[1].trim().is_empty()
            })
            .unwrap_or_else(|| panic!("우측에 글자가 있는 줄이 없다\n{}", lines.join("\n")));

        let right = &panes(row)[1];
        assert!(
            inset(right) >= LEFT_GUTTER,
            "우측이 테두리에 붙었다 (들여쓴 칸 {}) — {row:?}",
            inset(right)
        );
    }

    /// 생성·수정에 **연도가 있다.** 해를 넘긴 저장소에서 작년 9월인지 올해
    /// 9월인지 화면만 보고 알 수 없으면 시각이 시각 구실을 못 한다.
    /// CLI 상세(`view::detail`)가 이미 연도를 내므로 같은 자를 쓴다.
    #[test]
    fn timestamps_carry_the_year_like_the_cli_does() {
        let lines = render(&mut app(), 100, 16).join("\n");
        assert!(lines.contains("2026-09-01"), "연도가 없다\n{lines}");
    }

    /// 본문이 **그려진다.** 기호가 걷히고 목록은 글머리를 얻는다.
    #[test]
    fn the_body_is_drawn_in_the_detail_pane() {
        let mut issues = issues();
        issues[1].body = Some("**굵게** 한 줄\n\n- 하나\n- 둘\n".into());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        let drawn = render(&mut a, 100, 22).join("\n");
        assert!(!drawn.contains("**"), "굵게 기호가 남았다\n{drawn}");
        assert!(drawn.contains('•'), "목록 글머리가 없다\n{drawn}");

        // F3 으로 원문을 본다 — 그린 글은 기호가 지워져 되돌릴 수 없다
        a.key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
        let raw = render(&mut a, 100, 22).join("\n");
        assert!(raw.contains("**굵게**"), "원문이 아니다\n{raw}");
        assert!(raw.contains("- 하나"), "원문이 아니다\n{raw}");
    }

    /// 빈 저장소도 그려진다.
    #[test]
    fn an_empty_repo_still_draws() {
        let mut empty = App::new(Vec::new(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let lines = render(&mut empty, 60, 10).join("\n");
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
        for l in super::tests::render(&mut app, 96, h) {
            println!("{l}");
        }
    }
}
