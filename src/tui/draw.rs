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

/// 커서. 목록의 글자는 늘 이 폭만큼 안쪽에서 시작한다.
const CURSOR: &str = "> ";

/// 좌우 여백. **`CURSOR` 에서 잰다** — 우측 패널의 여백도 이 값인데, 숫자를
/// 따로 적어 두면 커서 글리프를 바꾼 날 두 패널이 말없이 갈라진다. 한쪽만
/// 테두리에 붙으면 같은 화면에서 규칙이 둘이 되고, 붙은 쪽이 답답하게 읽힌다.
fn left_gutter() -> usize {
    crate::text::width(CURSOR)
}

pub fn screen(f: &mut Frame, app: &mut App) {
    // **목록은 한 프레임에 한 번만 센다.** 세는 데 이슈 전부를 훑고 정렬까지
    // 하므로, 목록 패널과 상세 패널이 각자 세면 그 일이 한 키 누름에 두 번 더
    // 돈다. 1,600 이슈에서 그 한 번이 20ms 다.
    let rows = app.rows();
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
    list(f, app, left, &rows, &mut state);
    app.list = state;
    detail(f, app, right, &rows);

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

fn list(f: &mut Frame, app: &App, at: Rect, rows: &[Row], state: &mut ListState) {
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
            (n > 0).then(|| format!("{} {st} {n}", style::spin_glyph(st, app.spin)))
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
        Span::styled(style::spin_glyph(i.status.as_str(), app.spin).to_string(), status(i.status.as_str())),
        Span::raw(" "),
    ];
    // 커서 자리 + 머리글 폭. **`CURSOR` 에서 잰다** — 숫자를 손으로 적으면
    // 글리프를 바꾼 날 제목 몫이 한두 칸 넉넉해지고, 넘친 줄은 위젯이 말없이
    // 잘라 내 잘렸다는 `…` 마저 사라진다.
    let used =
        crate::text::width(CURSOR) + head.iter().map(|s| crate::text::width(&s.content)).sum::<usize>();
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
fn detail(f: &mut Frame, app: &mut App, at: Rect, rows: &[Row]) {
    // 좌우 여백은 목록의 커서 자리와 같은 폭이다. `inner()` 가 테두리와 여백을
    // 함께 빼 주므로 폭 계산은 아래가 그대로 쓴다.
    // 테두리는 **나중에** 그린다 — 제목에 "몇 줄 더" 를 얹으려면 줄을 먼저
    // 세야 한다. 자리 계산은 그래도 블록에 맡긴다.
    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(left_gutter() as u16));
    let inner = block.inner(at);

    let lines = match app.current_of(rows) {
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

    // **줄 수를 우리가 안다.** `Wrap` 을 켜면 위젯이 줄을 더 늘려 우리가 센
    // 수와 화면의 수가 어긋나고, 그러면 "몇 줄 더" 도 굴린 자리도 틀린다.
    // 본문은 `markdown::layout` 이 이미 폭에 맞춰 접었고, 머리 줄은
    // `about` 이 접거나 잘라서 낸다.
    let height = inner.height as usize;
    let hidden = lines.len().saturating_sub(height);
    // 끝을 지나 굴리지 않는다 — 빈 화면을 보여 주고 되돌아올 길을 잃게 한다.
    //
    // **자른 값을 도로 넣는다.** 그리는 데만 자르면 `App` 에는 큰 수가 남아,
    // 끝까지 굴린 뒤 `k` 를 눌러도 그 수가 다시 상한 밑으로 내려올 때까지
    // 화면이 한 칸도 안 움직인다 — 굴리는 키가 죽은 것처럼 보인다. 창을
    // 줄였을 때 옛 수가 살아나 화면이 갑자기 끝으로 튀는 것도 같은 뿌리다.
    let scroll = (app.scroll as usize).min(hidden);
    app.scroll = scroll as u16;
    let more = hidden - scroll;

    // **더 있는데 안 보이면 말한다.** 잘린 줄이 조용히 사라지면 보는 쪽은
    // 그것이 본문의 끝인 줄 안다. 제목에 얹으면 본문 한 줄을 안 뺏는다.
    // **굴리는 키를 여기서 말한다.** 아래 F키 바는 좁으면 뒤에서부터 키를
    // 떨어뜨리는데, 80칸이면 떨어지는 것이 하필 `j·k` 다 — 그때 화살표만
    // 가리키면 사람은 `↓` 를 누르고, 그것은 왼쪽 커서를 옮겨 보던 이슈를
    // 잃는다. 알림은 그것이 가리키는 것 곁에 둔다.
    let title = match (more, scroll) {
        (0, 0) => " 상세 ".to_string(),
        (0, _) => " 상세 · 끝 (k 로 위) ".to_string(),
        (n, _) => format!(" 상세 · {n}줄 더 (j·k) "),
    };
    f.render_widget(block.title(title), at);
    // **넘친 줄은 잘렸다고 말한다.** `Wrap` 을 끈 뒤로 폭을 넘는 줄은 위젯이
    // 표시도 없이 잘라 낸다 — 태그 줄, 롤업의 칸별 건수, `F3` 원문, 접지
    // 않기로 한 코드 줄이 그 길로 조용히 꼬리를 잃었다. 만드는 쪽마다 따로
    // 자르면 또 하나를 빠뜨리므로 **나가는 마지막 자리에서 한 번** 자른다.
    let lines: Vec<Line> = lines.into_iter().map(|l| fit(l, inner.width as usize)).collect();
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

/// 한 줄을 패널 폭에 맞춘다. 넘치면 `…` 를 남긴다.
fn fit(line: Line<'_>, room: usize) -> Line<'_> {
    let w: usize = line.spans.iter().map(|s| crate::text::width(&s.content)).sum();
    if w <= room {
        return line;
    }
    // `…` 한 칸을 남겨 두고 조각을 차례로 담는다. **표시는 한 번만 붙인다** —
    // 조각마다 `clip` 을 부르면 `…` 가 조각 수만큼 붙는다.
    let mut out: Vec<Span> = Vec::new();
    let mut used = 0usize;
    for s in line.spans {
        let left = room.saturating_sub(1).saturating_sub(used);
        if left == 0 {
            break;
        }
        let piece = clip(&s.content, left);
        // 잘려서 `…` 만 남은 조각은 버린다 — 아래에서 한 번 붙인다.
        let piece = piece.trim_end_matches('…').to_string();
        if piece.is_empty() {
            break;
        }
        used += crate::text::width(&piece);
        out.push(Span::styled(piece, s.style));
    }
    out.push(Span::styled("…", dim()));
    Line::from(out)
}

/// 글 하나를 폭에 맞춰 접어 여러 줄로. 접는 자는 본문과 같은 것을 쓴다.
fn wrapped<'a>(text: &str, w: usize, style: Style) -> Vec<Line<'a>> {
    let spans = [crate::markdown::Span {
        text: crate::text::sanitize(text),
        role: crate::markdown::Role::Plain,
    }];
    crate::markdown::wrap_spans(&spans, w.max(2))
        .into_iter()
        .map(|line| {
            Line::from(
                line.into_iter().map(|s| Span::styled(s.text, style)).collect::<Vec<_>>(),
            )
        })
        .collect()
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
    // **제목은 우리가 접는다.** `Wrap` 을 끈 것은 줄 수를 정확히 알기 위해서고,
    // 그 대가로 넘친 줄을 위젯이 표시도 없이 잘라 낸다 — 잘렸다는 `…` 마저
    // 사라지는 것이 이 저장소가 막아 온 실패다.
    let mut out = vec![Line::from(Span::styled(i.id.clone(), dim()))];
    out.extend(wrapped(&i.title, w, bold()));
    out.push(Line::from(""));

    // 칸은 글리프와 낱말을 함께 낸다. 색이 없어도 뜻이 남아야 한다.
    let st = i.status.as_str().to_string();
    let mut head = vec![
        Span::styled(format!("{} {st}", style::spin_glyph(&st, app.spin)), status(&st)),
        Span::raw("  ·  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
    ];
    if i.kind != crate::model::Kind::Issue {
        head.push(Span::raw("  ·  "));
        head.push(Span::styled(i.kind.as_str().to_string(), Style::new().fg(Color::LightBlue)));
    }
    out.push(Line::from(head));

    // 라벨 줄은 **모아 두고 폭을 재서** 낸다.
    let mut fields: Vec<(String, String)> = Vec::new();
    if !i.tags.is_empty() {
        let tags = i.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
        out.push(Line::from(Span::styled(tags, Style::new().fg(Color::Cyan))));
    }
    if let Some(a) = &i.assignee {
        fields.push((
            "담당".into(),
            crate::model::label(a, i.assignee_email.as_deref(), app.cfg.naming),
        ));
    }
    if let Some(id) = &i.epic {
        fields.push(("에픽".into(), app.title_of(id)));
    }
    if let Some(id) = &i.milestone {
        fields.push(("마일스톤".into(), app.title_of(id)));
    }
    // **막는 것은 제목까지 푼다.** id 만 내면 그것이 무엇인지 또 찾아봐야 한다.
    // **끝난 막음은 막지 않는다** — `report::is_blocked` 와 `moai ready` 가 그
    // 자로 세므로, 여기서만 다 "막힘" 이라 적으면 집을 수 있는 것을 못 집을
    // 것으로 읽는다. 뜻은 `report` 가 정하고 여기는 그 답을 그린다.
    for b in &i.blocked_by {
        let done = app.index.find(b).is_some_and(|at| app.issues[at].status.is_done());
        let (label, mark) = if done { ("풀림", "✓") } else { ("막힘", "·") };
        fields.push((label.into(), format!("{mark} {b}  {}", app.title_of(b))));
    }
    // CLI 상세와 **같은 자**를 쓴다. 두 표면이 같은 값을 다르게 적으면 보는
    // 쪽이 어느 쪽을 믿을지 정해야 한다.
    fields.push(("생성".into(), crate::view::stamp(&i.created_at)));
    fields.push(("수정".into(), crate::view::stamp(&i.updated_at)));
    let w_label = label_width(&fields);
    out.extend(fields.iter().map(|(k, v)| field(k, v, w_label, w)));

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
    // **패널 폭 그대로 편다.** 넉넉한 바닥값을 얹으면 좁은 창에서 패널보다 넓은
    // 줄이 나오고, 그 줄은 `Paragraph` 가 말없이 다시 접는다 — 다시 접힌 줄은
    // 글머리 밑으로 물리지 않고 표의 칸도 맞지 않는다. `markdown` 은 0 도 받는다.
    crate::markdown::layout(&blocks, w)
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
            (n > 0).then(|| Span::styled(format!("{} {st} {n}   ", style::spin_glyph(st, app.spin)), status(st)))
        })
        .collect();
    out.push(Line::from(counts));
    out
}

/// 이름 칸을 **표시 폭으로** 맞춘다. `{k:<5}` 는 글자 수를 세므로 `에픽`(2자
/// 4칸)과 `마일스톤`(4자 8칸)이 두 칸 어긋나 값이 들쭉날쭉해진다 — `text`
/// 모듈이 있는 까닭이 바로 이것이다.
/// 라벨 칸을 **그 화면에 실제로 쓰인 라벨에 맞춘다.**
///
/// 가장 긴 라벨(`마일스톤`, 8칸)로 못 박으면 마일스톤을 안 쓰는 이슈에서도 그
/// 폭이 새어 나가, 좁은 패널에서 시각이 다음 줄로 밀리고 시계가 값도 라벨도
/// 아닌 숫자로 홀로 남는다. 목록의 열 폭을 자료에서 재는 것(`view::list`)과
/// 같은 규칙이다.
fn label_width(rows: &[(String, String)]) -> usize {
    rows.iter().map(|(k, _)| crate::text::width(k)).max().unwrap_or(0)
}

fn field<'a>(k: &str, v: &str, w: usize, room: usize) -> Line<'a> {
    let pad = w.saturating_sub(crate::text::width(k));
    // 값이 넘치면 **잘렸다고 말하며** 자른다. 위젯에 맡기면 표시 없이 사라진다.
    let v = crate::text::clip(v, room.saturating_sub(w + 1));
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
    // **덜 급한 것부터 떨어뜨린다.** 키를 더할 때마다 줄이 길어져 맨 끝이
    // 말없이 잘리는데, 맨 끝은 늘 나가는 길이다 — 나갈 길을 못 찾는 것이
    // 빽빽한 줄보다 나쁘다. 폭이 모자라면 앞쪽부터 버린다.
    let mut optional = vec![
        key("j·k", "굴리기"),
        key("F3", if app.raw { "그리기" } else { "원문" }),
        key("F5", "갱신"),
        key("f", "거름망"),
        key("/", "검색"),
        key("Bksp", "나가기"),
        key("Enter", "들어가기"),
    ];
    // 늘 남는 것: 나가는 길, 그리고 걸어 둔 거름망을 푸는 길.
    let mut keep: Vec<Span> = Vec::new();
    if app.filter_text.is_some() {
        keep.push(key("Esc", "풀기"));
    }
    keep.push(key("F10", "끝내기"));

    let width = |v: &[Span]| v.iter().map(|s| crate::text::width(&s.content)).sum::<usize>();
    let room = at.width as usize;
    let mut spans: Vec<Span> = Vec::new();
    while let Some(next) = optional.pop() {
        if width(&spans) + crate::text::width(&next.content) + width(&keep) > room {
            break;
        }
        spans.push(next);
    }
    spans.extend(keep);
    f.render_widget(Paragraph::new(Line::from(spans)), at);
}

/// F키 하나. **뒤에 공백을 두지 않는다** — 앞뒤로 두면 칸 사이가 두 칸이 되고,
/// 그 여섯 칸 때문에 80칸 터미널에서 줄이 넘쳐 맨 끝의 `F10 끝내기` 가 말없이
/// 잘린다. 나갈 길을 못 찾는 것이 빽빽한 줄보다 나쁘다.
fn key<'a>(k: &'a str, what: &'a str) -> Span<'a> {
    Span::styled(format!(" {k} {what}"), dim())
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
        // `in_progress` 는 정지 글리프가 아니라 도는 프레임을 낸다 — 어느
        // 프레임이든 `style::SPIN` 의 한 글자여야 한다.
        assert!(style::SPIN.iter().any(|g| lines.contains(g)), "칸 글리프가 없다\n{lines}");
        assert!(lines.contains("Enter") && lines.contains("F10"), "F키 바가 없다\n{lines}");
        // 긴 제목은 **잘린다**. 잘렸다는 표시가 남아야 어디까지가 제목인지 안다.
        assert!(lines.contains("아주 긴"), "에픽 제목이 없다\n{lines}");
        assert!(lines.contains('…'), "잘렸는데 표시가 없다\n{lines}");
        // 디렉터리는 제목 뒤에 `/` 가 붙는다
    }

    /// 걸음은 **그린 횟수가 아니라 시계가** 올린다. 다시 그리기만 해서는
    /// 안 돌아야 하고 — 안 그러면 키를 누르는 동안 타이핑 속도로 돈다 —
    /// 한 화면 안에서는 목록과 상세가 **같은 걸음**을 보여야 한다.
    #[test]
    fn the_spinner_follows_the_clock_and_shows_one_step_per_screen() {
        let mut a = app();
        let shown = |lines: &str| -> Vec<&'static str> {
            style::SPIN.iter().copied().filter(|g| lines.contains(g)).collect()
        };
        let first = render(&mut a, 100, 12).join("\n");
        assert_eq!(shown(&first).len(), 1, "한 화면에 걸음이 섞였다\n{first}");
        let again = render(&mut a, 100, 12).join("\n");
        assert_eq!(shown(&first), shown(&again), "그리기만으로 걸음이 갔다\n{again}");
        a.spin += 1;
        let next = render(&mut a, 100, 12).join("\n");
        assert_eq!(shown(&next).len(), 1, "한 화면에 걸음이 섞였다\n{next}");
        assert_ne!(shown(&first), shown(&next), "걸음이 갔는데 글리프가 그대로다\n{next}");
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
        let w = label_width(&[("에픽".into(), "값".into()), ("마일스톤".into(), "값".into())]);
        let short = field("에픽", "값", w, 40);
        let long = field("마일스톤", "값", w, 40);
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
            inset(right) >= left_gutter(),
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

    /// **나갈 길은 80칸에서도 보인다.** F키 바는 접히지 않고 위젯이 말없이
    /// 잘라 내므로, 줄이 넘치면 맨 끝의 `F10 끝내기` 부터 사라진다 — 끝내는
    /// 법을 모르는 화면은 도구가 아니라 덫이다.
    #[test]
    fn the_key_bar_still_says_how_to_quit_at_eighty_columns() {
        for w in [80u16, 100, 120] {
            let lines = render(&mut app(), w, 14);
            let bar = lines.last().cloned().unwrap_or_default();
            assert!(bar.contains("F10 끝내기"), "{w}칸에서 나갈 길이 잘렸다 — {bar:?}");
            assert!(bar.contains("F3"), "{w}칸에서 원문 키가 잘렸다 — {bar:?}");
        }
    }

    /// **좁은 창에서 본문을 그려도 무너지지 않는다.**
    /// `narrow_windows_neither_panic_nor_overflow` 는 본문 없는 이슈로 그리므로
    /// 겹친 목록과 표를 지나는 이 길을 한 번도 밟지 않는다. 줄이 패널 폭 안에
    /// 드는지는 `markdown::laid_out_prose_never_exceeds_the_width_it_was_given`
    /// 이 원천에서 재고, 여기서는 좁은 폭에서 셈이 터지지 않는지를 본다.
    #[test]
    fn a_body_in_a_narrow_pane_neither_panics_nor_overflows() {
        let mut issues = issues();
        issues[1].body = Some(
            "겹친 목록과 표가 함께 있는 본문이다.\n\n- 하나\n  - 둘의 속이 길게 이어진다\n\n\
             | 후보 | 판 | 무엇 |\n|---|---|---|\n| `termimad` | 0.35.4 | 렌더러 |\n"
                .into(),
        );
        for w in [20u16, 24, 30, 40, 60, 80] {
            let mut a =
                App::new(issues.clone(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
            a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            for l in render(&mut a, w, 30) {
                assert!(crate::text::width(&l) <= w as usize, "{w}칸을 넘었다 — {l:?}");
            }
        }
    }

    /// 좁은 패널에서도 **시각이 한 줄에 들어간다.** 넘치면 날짜만 남고 시계가
    /// 다음 줄에 홀로 떨어져, 값도 아니고 라벨도 아닌 숫자가 된다.
    #[test]
    fn a_timestamp_fits_on_one_line_in_a_narrow_pane() {
        for w in [60, 66, 70, 80, 100] {
            let lines = render(&mut app(), w, 16);
            let at = lines
                .iter()
                .position(|l| l.contains("생성"))
                .unwrap_or_else(|| panic!("폭 {w}: 생성 줄이 없다\n{}", lines.join("\n")));
            assert!(
                lines[at].contains("2026-09-01") && lines[at].contains("00:00"),
                "폭 {w} 에서 시각이 두 줄로 갈렸다 — {:?} / {:?}",
                lines[at],
                lines.get(at + 1)
            );
        }
    }

    /// 긴 본문은 **끝을 볼 수 있다.** 굴리면 뒷부분이 나오고, 남은 줄 수를
    /// 말해 준다 — 잘린 줄이 조용히 사라지면 그것이 본문의 끝인 줄 안다.
    #[test]
    fn a_long_body_can_be_scrolled_to_its_end() {
        let mut issues = issues();
        let body: String = (1..=40).map(|n| format!("{n}번째 줄이다\n\n")).collect();
        issues[1].body = Some(body);
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        let first = render(&mut a, 100, 16).join("\n");
        assert!(first.contains("1번째"), "{first}");
        assert!(!first.contains("40번째"), "다 보이면 굴릴 것이 없다\n{first}");
        assert!(first.contains("줄 더"), "남은 줄을 안 알린다\n{first}");

        for _ in 0..12 {
            a.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        }
        let last = render(&mut a, 100, 16).join("\n");
        assert!(last.contains("40번째"), "끝까지 못 굴렸다\n{last}");
        assert!(!last.contains("줄 더"), "다 보이는데 더 있다고 한다\n{last}");

        // **되돌아오는 길도 한 번에 열린다.** 그리는 데만 자르고 `App` 에 큰
        // 수를 남겨 두면, 끝까지 굴린 뒤 `k` 를 눌러도 그 수가 상한 밑으로
        // 내려올 때까지 화면이 한 칸도 안 움직여 키가 죽은 것처럼 보인다.
        a.key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let up = render(&mut a, 100, 16).join("\n");
        assert_ne!(up, last, "끝까지 굴린 뒤 k 가 아무 일도 안 한다");
        assert!(up.contains("줄 더"), "되돌아왔는데 남은 줄을 안 알린다\n{up}");
    }

    /// **넘친 줄은 잘렸다고 말한다.** `Wrap` 을 끈 뒤로 폭을 넘는 줄은 위젯이
    /// 표시도 없이 잘라 낸다 — 태그 줄·롤업의 칸별 건수·`F3` 원문·접지 않기로
    /// 한 코드 줄이 그 길로 조용히 꼬리를 잃었다. 이 시험은 버퍼 폭이 아니라
    /// **잘린 자리에 표시가 있는지**를 본다.
    #[test]
    fn a_line_too_wide_for_the_detail_pane_says_it_was_cut() {
        let mut issues = issues();
        issues[1].tags =
            vec!["parser".into(), "storage".into(), "renderer".into(), "markdown".into()];
        issues[1].body = Some("```\nlet very_long = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\";\n```\n".into());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        for raw in [false, true] {
            if raw {
                a.key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
            }
            let lines = render(&mut a, 60, 24);
            let cut = lines.iter().any(|l| l.contains("#parser") && l.contains('…'));
            assert!(cut, "태그 줄이 표시 없이 잘렸다 (raw={raw})\n{}", lines.join("\n"));
            let code = lines.iter().any(|l| l.contains("aaaa"));
            assert!(
                !code || lines.iter().any(|l| l.contains("aaaa") && l.contains('…')),
                "코드 줄이 표시 없이 잘렸다 (raw={raw})\n{}",
                lines.join("\n")
            );
        }
    }

    /// **끝을 지나 굴려도 `k` 가 곧바로 듣는다.** 자른 값을 `App` 에 도로
    /// 넣지 않으면, 큰 수가 상한 밑으로 내려올 때까지 화면이 한 칸도 안
    /// 움직여 굴리는 키가 죽은 것처럼 보인다.
    #[test]
    fn scrolling_past_the_end_does_not_deaden_the_key() {
        let mut issues = issues();
        issues[1].body = Some((1..=40).map(|n| format!("{n}번째 줄이다\n\n")).collect::<String>());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        for _ in 0..80 {
            a.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        }
        let end = render(&mut a, 100, 16).join("\n");
        assert!(end.contains("40번째"), "끝을 지나쳐 빈 화면이 됐다\n{end}");

        a.key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let up = render(&mut a, 100, 16).join("\n");
        assert_ne!(up, end, "k 를 눌렀는데 화면이 그대로다");
    }

    /// **폭을 넘는 줄은 잘렸다고 말한다.** `Wrap` 을 끈 뒤로 위젯이 표시 없이
    /// 잘라 내므로, 태그가 길면 꼬리가 조용히 사라진다.
    #[test]
    fn an_overlong_line_says_it_was_cut() {
        let mut issues = issues();
        issues[1].tags = (1..=20).map(|n| format!("아주긴태그이름{n}")).collect();
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let out = render(&mut a, 100, 20);
        let tagline = out.iter().find(|l| l.contains("아주긴태그이름1")).expect("태그 줄이 없다");
        assert!(tagline.contains('…'), "잘렸는데 표시가 없다 — {tagline:?}");
    }

    /// **커서를 옮기면 굴린 자리가 돌아온다.** 안 그러면 다른 이슈의 첫 줄부터
    /// 못 본다.
    #[test]
    fn moving_the_cursor_rewinds_the_detail() {
        let mut issues = issues();
        issues[1].body = Some((1..=40).map(|n| format!("{n}번째\n\n")).collect::<String>());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(a.scroll > 0);
        a.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(a.scroll, 0, "굴린 자리를 들고 다른 줄로 갔다");
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
