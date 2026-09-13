//! 그림. **뜻을 판단하지 않는다** — 무엇이 어디 걸리는지는 `nav`, 무엇을
//! 세는지는 `report` 가 이미 정했다.
//!
//! 규칙 하나를 CLI 에서 그대로 들고 온다: **색이 혼자 뜻을 지지 않는다.**
//! 모든 색에 글리프나 낱말이 붙는다.

use super::form::{Field, Form};
use super::scroll::Scroll;
use super::{App, Input, Mode, Pane, Row};
use crate::nav::Entry;
use crate::style;
use crate::text::clip;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph};
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
    // 누군지 묻는 동안만 아랫줄이 둘이다 — 왜 묻는지와 다시 안 묻게 하는 법은 글칸
    // 뒤에 붙이면 적는 글에 밀려 사라진다. 그 둘이 이 칸의 알맹이다.
    let keys_h = if matches!(app.mode, Mode::Ask(_)) { 2 } else { 1 };
    let [top, note, body, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(banner_h),
        Constraint::Min(1),
        Constraint::Length(keys_h),
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
    list(f, app, left, &rows);
    detail(f, app, right, &rows);
    // 폼은 목록과 상세 자리를 **통째로** 덮는다. 가장자리를 비워 뒤를 비치게 해 봤더니
    // 뒤 칸의 테두리와 커서(`>`)가 폼의 테두리에 겹쳐 어느 선이 어느 칸인지 안 읽혔다.
    // 닫으면 보던 자리 그대로 돌아온다 — 경로 줄은 폼 위에 그대로 서 있다.
    //
    // **담다가 누군지 물으면 폼은 뒤에 남는다.** 묻는 칸이 적던 폼을 들고 있다가
    // 되돌려 놓는데(`Ask::back`), 그동안 폼이 사라지면 적던 것이 날아간 줄 안다. 키는
    // 묻는 칸이 먹으므로 폼의 칸은 굵은 선도 커서도 내려놓는다.
    match &mut app.mode {
        Mode::Idea(form) => jot(f, form, body, true),
        Mode::Ask(ask) => {
            if let Mode::Idea(form) = ask.back.as_mut() {
                jot(f, form, body, false);
            }
        }
        _ => {}
    }

    // 맨 아랫줄은 하나다 — 글을 받는 중이면 프롬프트가, 아니면 F키 바가 선다.
    match &app.mode {
        Mode::Browse => fkeys(f, app, keys),
        Mode::Grep(q) => prompt(f, keys, "검색", q, app.input_error(), "Enter 걸기  Esc 그만"),
        Mode::Filter(q) => prompt(f, keys, "거름망", q, app.input_error(), "Enter 걸기  Esc 그만"),
        Mode::Ask(ask) => {
            // 글칸이 **아래**에서 자리를 먼저 얻는다 — 창이 낮아 한 줄만 남으면 적는 칸이
            // 안내에 가려 어디에 치는지 안 보인다.
            let [why, line] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(keys);
            let text = clip(&ask_why(&ask.why), why.width as usize);
            f.render_widget(Paragraph::new(Line::from(Span::styled(text, Style::new().fg(Color::Black).bg(Color::LightYellow)))), why);
            prompt(f, line, "누구", &ask.input, ask.error.clone(), "이름 (메일)  Enter 쓰기  Esc 그만");
        }
        Mode::Idea(form) => jot_keys(f, form, keys),
    }
}

/// 생각 담기 폼. 제목 칸(세 줄) 밑에 본문 칸이 남은 높이를 다 먹는다.
///
/// **키를 먹는 칸은 굵은 선이다** — 목록·상세의 포커스와 같은 모양([`frame`])이라 한
/// 화면에 규칙이 하나다. 폼이 열려 있는 동안 목록·상세는 굵은 선을 내려놓으므로 화면에
/// 굵은 칸은 늘 하나다. 칸 이름은 테두리에 적는다: 색이 없어도 모양과 이름이 남는다.
///
/// 커서는 터미널 커서가 키를 먹는 칸의 글 안 제자리에 선다 — [`prompt`] 와 같은 까닭이다.
/// `active` 가 거짓이면(다른 칸이 키를 먹는 중이면) 굵은 선도 커서도 없다.
fn jot(f: &mut Frame, form: &mut Form, at: Rect, active: bool) {
    f.render_widget(Clear, at);
    let [title_at, body_at] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(at);

    let field = |which: Field, name: &'static str| {
        let block = Block::default().borders(Borders::ALL).title(name);
        if active && form.field == which {
            block.border_type(BorderType::Thick).border_style(from_anstyle(style::FOCUS))
        } else {
            block
        }
    };

    let title_block = field(Field::Title, " 생각 담기 · 제목 ");
    let inner = title_block.inner(title_at);
    let view = form.title.view(inner.width as usize);
    let title_cursor = (inner.width > 0 && inner.height > 0)
        .then(|| (inner.x + view.cursor as u16, inner.y));
    f.render_widget(Paragraph::new(Line::from(view.text)).block(title_block), title_at);

    let body_block = field(Field::Body, " 본문 · 여러 줄 · 없어도 된다 ");
    let inner = body_block.inner(body_at);
    form.body.fit(inner.height as usize);
    let view = form.body.view(inner.width as usize, inner.height as usize);
    let body_cursor = view.cursor.map(|(x, y)| (inner.x + x as u16, inner.y + y as u16));
    let lines: Vec<Line> = view.lines.into_iter().map(Line::from).collect();
    f.render_widget(Paragraph::new(lines).block(body_block), body_at);
    scroll_mark(f, form.body.scroll(), body_at, "", active && form.field == Field::Body);

    // 묻는 중에는 커서를 안 세운다 — 친 키가 글자로 들어가지 않는데 커서가 글 안에 서
    // 있으면 거기 적힐 것처럼 보인다.
    let cursor = match form.field {
        Field::Title => title_cursor,
        Field::Body => body_cursor,
    };
    if let Some(pos) = cursor.filter(|_| active && !form.leaving) {
        f.set_cursor_position(pos);
    }
}

/// 폼이 열린 동안의 맨 아랫줄 — 담는 법·칸 옮기는 법·닫는 법. **80칸에 다 든다.**
///
/// 거절된 까닭(빈 제목)과 버릴지 묻는 말은 이 줄을 차지한다 — 키 안내는 폼을 연
/// 순간 이미 봤고, 지금 답해야 할 것은 그 말이다.
fn jot_keys(f: &mut Frame, form: &Form, at: Rect) {
    let line = if form.leaving {
        let ask = Style::new().fg(Color::Black).bg(Color::LightYellow);
        Line::from(Span::styled(" 적던 것을 버릴까 — y 버린다 · 다른 키는 폼으로 돌아간다 ", ask))
    } else if let Some(e) = &form.error {
        Line::from(vec![
            Span::styled(" ! ", Style::new().fg(Color::Black).bg(Color::LightRed)),
            Span::styled(format!(" {e}"), Style::new().fg(Color::LightRed)),
        ])
    } else {
        let (next, enter) = match form.field {
            Field::Title => ("본문", "본문으로"),
            Field::Body => ("제목", "줄 나누기"),
        };
        Line::from(vec![
            key("Ctrl-S·F2", "담기"),
            key("Tab", next),
            key("Enter", enter),
            key("Esc", "닫기"),
            Span::styled("   idea 로 담긴다 — 에픽 없이", dim()),
        ])
    };
    let line = fit(line, at.width as usize);
    f.render_widget(Paragraph::new(line), at);
}

/// 누군지 묻는 까닭과 **다시 안 묻게 하는 법.** 받은 것은 이 세션 동안만 들고
/// 어디에도 적지 않으므로(moai-nmv2), 다음에도 묻지 않게 하는 길은 사람이 제 설정에
/// 적는 것 하나다 — 그 길을 여기서 대지 않으면 열 때마다 물음을 받는다.
///
/// **다시 안 묻게 하는 법이 앞이다.** 좁은 창에서는 뒤가 잘리는데, 세션 동안만 든다는
/// 말은 잘려도 사람이 잃는 것이 없고 설정하는 법은 잘리면 다음에 또 묻는다. 그다음이
/// 쓰기를 멈춘 거절문(`why`)이다 — 설정이 없는지 틀렸는지를 그것이 가른다.
fn ask_why(why: &str) -> String {
    format!(" git config user.name·user.email 을 바르게 적어 두면 다시 안 묻는다 · {why} · 받은 것은 이 세션 동안만 든다 ")
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
        // 무엇을 못 했는지는 단 쪽이 적는다 — 다시 읽기와 쓰기가 같은 자리를 쓴다.
        parts.push(t.clone());
        urgent = true;
    }
    if !app.unreadable.is_empty() {
        parts.push(format!("읽을 수 없는 줄 {}개 — 그 줄은 빠진 채로 보고 있다", app.unreadable.len()));
        urgent = true;
    }
    if app.warnings > 0 {
        parts.push(format!("드러난 것 {}건 — `moai status` 가 자세히 낸다", app.warnings));
    }
    // 옆 워크트리의 문제는 **급하지 않다** — 제 파일은 멀쩡하고, 그 줄만 빠진 채로
    // 겹쳐 보고 있다.
    parts.extend(app.elsewhere.iter().cloned());
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
    // **겹쳐 보는 중이면 늘 보인다** — 거름망 뱃지와 같은 까닭이다. 옆에서 온 줄에만
    // `⎇` 가 붙으므로, 옆이 조용하면 켜진 화면과 꺼진 화면이 똑같이 보인다.
    let overlay = app.worktree.then(|| {
        let trees = app.origin.labels();
        let names = if trees.is_empty() { "옆 워크트리 없음".to_string() } else { trees.join(", ") };
        clip(&format!("{} {names}  w 로 끈다", style::BRANCH_GLYPH), w / 2)
    });
    let room = match &overlay {
        Some(o) => room.saturating_sub(crate::text::width(o) + 3),
        None => room,
    };
    // **언제 읽은 화면인지** 댄다. 파일이 바뀌면 저절로 다시 읽으므로(`App::follow`)
    // 배너는 없고, 이 시각이 바뀌는 것이 갱신됐다는 표시다. 자리가 모자라면 이것부터
    // 버린다 — 거름망·겹쳐 보기 뱃지는 줄이 왜 그런지를 말하고, 이것은 곁들임이다.
    let read_at = app.now.get(11..19).map(|t| format!("↻ {t}"));
    let read_at = read_at.filter(|t| crate::text::width(t) + 3 + 8 <= room);
    let room = match &read_at {
        Some(t) => room.saturating_sub(crate::text::width(t) + 3),
        None => room,
    };
    let mut spans = vec![Span::styled(clip(&app.crumbs(), room), bold())];
    if let Some(t) = read_at {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(t, dim()));
    }
    if let Some(o) = overlay {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(o, branch()));
    }
    if let Some(b) = badge {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(b, Style::new().fg(Color::Black).bg(Color::LightYellow)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), at);
}

/// 글을 받는 줄. 잘못 적은 거름망은 그 자리에서 말해 준다.
///
/// **커서는 터미널의 커서다.** 글 끝에 붙여 찍던 `▌` 는 커서가 글 안으로 들어가면
/// 설 자리가 없다 — 글자 위에 찍으면 그 글자를 가리고, 사이에 끼우면 뒤의 글이
/// 한 칸 밀려 [`Input::view`] 가 잰 폭이 틀린다. 터미널 커서는 칸을 차지하지
/// 않고, 한글 입력기가 조합 중인 글자를 띄우는 자리도 거기다.
///
/// 글이 칸보다 길면 조각이 커서를 따라 밀린다. 그러면 뒤의 안내·오류는 줄 밖으로
/// 밀리는데, 옮기기 전에도 그랬다.
fn prompt(f: &mut Frame, at: Rect, what: &str, input: &Input, error: Option<String>, help: &str) {
    let label = format!(" {what} ");
    // 이름표와 그 뒤 빈칸 하나를 뗀 자리가 글칸이다.
    let lead = crate::text::width(&label) + 1;
    let view = input.view((at.width as usize).saturating_sub(lead));
    let mut spans = vec![
        Span::styled(label, Style::new().fg(Color::Black).bg(Color::LightBlue)),
        Span::raw(" "),
        Span::raw(view.text),
        // 커서가 끝에 서면 조각 뒤 한 칸이 커서 자리다. 비워 두지 않으면 안내가
        // 커서 밑으로 붙는다.
        Span::raw(" "),
    ];
    match error {
        Some(e) => {
            spans.push(Span::raw("   "));
            // 여러 줄짜리 도움말은 첫 줄만 — 한 줄 자리다.
            let first = e.lines().next().unwrap_or_default().to_string();
            spans.push(Span::styled(first, Style::new().fg(Color::LightRed)));
        }
        None => {
            spans.push(Span::raw("   "));
            spans.push(Span::styled(help.to_string(), dim()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), at);
    // 칸이 좁아 글칸이 0 이면 커서를 세우지 않는다 — 이름표 위에 서면 어디에
    // 적히는지 거짓말을 한다.
    if (at.width as usize) > lead {
        f.set_cursor_position((at.x + (lead + view.cursor) as u16, at.y));
    }
}

fn list(f: &mut Frame, app: &mut App, at: Rect, rows: &[Row]) {
    // 테두리 두 칸을 뺀 안쪽 폭. 좁은 창에서도 음수가 되지 않게 막는다.
    let inner = at.width.saturating_sub(2) as usize;
    let items: Vec<ListItem> = rows.iter().map(|r| ListItem::new(row_line(app, r, inner))).collect();

    // 제목에 칸별 건수를 **config 차례로** 낸다. 칸 이름과 순서는 저장소가
    // 정하는 것이라(`config.statuses`) 여기서 다시 정하지 않는다.
    //
    // **일만 센다.** 에픽도 마일스톤도 묶음이지 일이 아니라고 `report::is_work`
    // 가 정했고, 오른쪽 롤업이 그 자로 센다 — 여기만 묶음을 같이 세면 한
    // 화면의 두 패널이 같은 디렉터리를 두 수로 말한다.
    //
    // **미뤄 둔 것은 뺀 자(`report::put_off`)를 쓰지 않는다.** `moai status` 의
    // 보드는 그쪽으로 세지만 거기는 "지금 할 수 있는 것" 을 묻는 화면이고,
    // 탐색기는 시키지도 않은 줄을 숨기지 않으므로 미룬 줄이 목록에 그대로
    // 있다. 세는 자를 바꾸면 머리글이 제 밑의 줄 수와 어긋난다(moai-lhbh).
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
    // **자리는 프레임을 넘어 산다**(`App::list`). 매번 새로 세면 훑는 자리가 0 으로
    // 돌아가, 커서를 보이게 하려고 커서를 늘 맨 아랫줄에 붙인다 — 커서 아래를 한
    // 줄도 못 보게 된다. 굴리는 셈은 상세와 같은 조각이 하고, 위젯은 그 자리를 받아
    // 그리기만 한다: 커서가 이미 보이는 자리를 주므로 위젯이 다시 옮기지 않는다.
    // 줄 하나가 한 줄이라 높이가 곧 줄 수다.
    let selected = (!rows.is_empty()).then_some(app.cursor.min(rows.len().saturating_sub(1)));
    app.list.fit(at.height.saturating_sub(2) as usize, rows.len());
    if let Some(at) = selected {
        app.list.reveal(at);
    }
    let mut state = ListState::default().with_offset(app.list.offset()).with_selected(selected);
    f.render_stateful_widget(
        List::new(items)
            .block(frame(app, Pane::Explorer).title(title))
            // 커서는 **색만으로 표시하지 않는다** — 반전과 `>` 를 함께 준다.
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(CURSOR),
        at,
        &mut state,
    );
    scroll_mark(f, &app.list, at, "", app.focus == Pane::Explorer);
}

/// 굴릴 것이 남았으면 **칸의 아래 테두리 오른쪽에** 적는다. 목록이든 상세든 같은
/// 자리, 같은 글이다 — 무엇을 적을지는 조각([`Scroll::mark`])이 정하고 여기는
/// 놓기만 한다. 아래 테두리는 제목도 무엇도 안 쓰는 자리라 본문 한 줄을 안 뺏는다.
///
/// 칸이 좁으면 `…` 를 남기고 자른다. 테두리 모서리는 안 덮는다.
///
/// **포커스 있는 칸에서는 테두리의 초록을 이어받아 흐리게만 한다.** 표시는 테두리
/// 위에 앉으므로 회색으로 덮으면 그 칸만 초록 선이 끊겨, 포커스 칸의 아래 테두리가
/// 굴릴 것이 남았을 때마다 짧아 보인다. 뜻은 여전히 선 모양([`frame`])이 진다.
fn scroll_mark(f: &mut Frame, s: &Scroll, at: Rect, hint: &str, focused: bool) {
    let Some(mark) = s.mark() else { return };
    if at.height < 2 || at.width < 4 {
        return;
    }
    let room = at.width as usize - 2;
    let text = clip(&format!(" {mark}{hint} "), room);
    let w = crate::text::width(&text) as u16;
    let x = at.x + at.width - 1 - w;
    let look = if focused { from_anstyle(style::FOCUS).add_modifier(Modifier::DIM) } else { dim() };
    f.buffer_mut().set_stringn(x, at.y + at.height - 1, &text, room, look);
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
        // 묶음은 멤버에서 읽은 칸이다 — CLI 목록의 S 열과 같은 자. **다만 안 돌린다**:
        // 도는 글리프는 "지금 누가 손대고 있다" 는 말인데 묶음의 `in_progress` 는
        // 멤버 하나가 끝났다는 말일 수도 있다(`App::spinning` 과 같은 자).
        Span::styled(glyph_of(app, at).to_string(), status(app.column(at))),
        Span::raw(" "),
    ];
    // 다른 워크트리에서 온 줄은 제목 **앞에** `⎇ <브랜치>` — CLI 목록과 같은 자리다.
    // 머리글에 넣어 재므로 제목 몫이 그만큼 줄고, 잘린 제목은 여전히 `…` 를 남긴다.
    let mut head = head;
    if let Some(b) = app.origin.branch(&i.id) {
        head.push(Span::styled(format!("{} {}", style::BRANCH_GLYPH, clip(b, BRANCH_CAP)), branch()));
        head.push(Span::raw(" "));
    }
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
    // 테두리는 **나중에** 그린다 — 굴릴 것이 남았다는 표시를 테두리에 얹으려면
    // 줄을 먼저 세야 한다. 자리 계산은 그래도 블록에 맡긴다.
    let block = frame(app, Pane::Detail)
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
    // 수와 화면의 수가 어긋나고, 그러면 굴릴 것이 남았다는 표시도 굴린 자리도
    // 틀린다. 본문은 `markdown::layout` 이 이미 폭에 맞춰 접었고, 머리 줄은
    // `about` 이 접거나 잘라서 낸다.
    //
    // 잰 것을 조각에 넣으면 **끝을 지난 자리가 잘려 도로 들어간다**(`Scroll::fit`)
    // — 그리는 데만 자르면 끝까지 굴린 뒤 `k` 가 한동안 죽는다.
    app.detail.fit(inner.height as usize, lines.len());
    f.render_widget(block.title(" 상세 "), at);
    // **굴리는 키를 표시 곁에서 말한다.** 아래 F키 바는 좁으면 뒤에서부터 키를
    // 떨어뜨리는데, 80칸이면 떨어지는 것이 하필 `j·k` 다. 알림은 그것이
    // 가리키는 것 곁에 둔다.
    scroll_mark(f, &app.detail, at, " (j·k)", app.focus == Pane::Detail);
    // **넘친 줄은 잘렸다고 말한다.** `Wrap` 을 끈 뒤로 폭을 넘는 줄은 위젯이
    // 표시도 없이 잘라 낸다 — 태그 줄, 롤업의 칸별 건수, `F3` 원문, 접지
    // 않기로 한 코드 줄이 그 길로 조용히 꼬리를 잃었다. 만드는 쪽마다 따로
    // 자르면 또 하나를 빠뜨리므로 **나가는 마지막 자리에서 한 번** 자른다.
    let lines: Vec<Line> = lines.into_iter().map(|l| fit(l, inner.width as usize)).collect();
    // 줄 수가 u16 을 넘는 본문이면 거기서 멈춘다 — 넘겨 접으면 첫 줄로 튄다.
    let top = u16::try_from(app.detail.offset()).unwrap_or(u16::MAX);
    f.render_widget(Paragraph::new(lines).scroll((top, 0)), inner);
}

/// 칸의 테두리. **포커스 있는 칸은 굵은 선에 초록이다.**
///
/// 색만 바꾸면 색 없는 터미널·색맹인 눈에서 포커스가 통째로 사라지고, 그러면
/// `Tab` 을 모르고 누른 사람은 `↓` 가 상세를 굴리는 동안 목록이 죽은 줄 안다.
/// 그래서 **모양이 뜻을 지고 색은 곁들인다** — `┏━┓` 와 `┌─┐` 는 글자가 달라
/// 버퍼의 글자만 읽어도 갈린다. 제목 옆 글리프를 따로 두지 않은 것은 목록 제목이
/// 이미 칸별 건수로 차 있어, 좁은 창에서 그 글리프가 먼저 잘려 나가기 때문이다 —
/// 테두리는 잘리지 않는다.
///
/// 두 줄(`╔═╗`)이 아니라 굵은 선을 고른 것은 **모양은 같고 무게만 달라서**다 —
/// 두 칸이 여전히 한 벌로 읽히고, 포커스가 옮겨 갈 때 화면이 다른 종류의 창으로
/// 바뀐 것처럼 보이지 않는다.
fn frame(app: &App, pane: Pane) -> Block<'static> {
    let block = Block::default().borders(Borders::ALL);
    if app.focus == pane {
        block.border_type(BorderType::Thick).border_style(from_anstyle(style::FOCUS))
    } else {
        block
    }
}

/// 칸의 이름. F키 바가 `Tab` 이 **어디로 가는지** 댄다.
fn pane_name(p: Pane) -> &'static str {
    match p {
        Pane::Explorer => "목록",
        Pane::Detail => "상세",
    }
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

/// 그 줄의 글리프. **묶음은 안 돈다** — 도는 것은 지금 누가 손대고 있는 일이다
/// (`App::spinning` 이 같은 자로 깨울 것을 센다).
fn glyph_of(app: &App, at: usize) -> &'static str {
    let col = app.column(at);
    if crate::report::is_group(&app.issues[at]) {
        style::glyph(col)
    } else {
        style::spin_glyph(col, app.spin)
    }
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
    let mut first = vec![Span::styled(i.id.clone(), dim())];
    if let Some(b) = app.origin.branch(&i.id) {
        // 브랜치도 **우리가 자른다** — 좁은 패널에서 긴 브랜치 이름이 위젯에 말없이
        // 잘리면 `…` 도 없이 다른 브랜치 이름처럼 읽힌다.
        let room = w.saturating_sub(crate::text::width(&i.id) + 2);
        first.push(Span::raw("  "));
        first.push(Span::styled(clip(&format!("{} {b}", style::BRANCH_GLYPH), room), branch()));
    }
    let mut out = vec![Line::from(first)];
    out.extend(wrapped(&i.title, w, bold()));
    out.push(Line::from(""));

    // 칸은 글리프와 낱말을 함께 낸다. 색이 없어도 뜻이 남아야 한다.
    let st = app.column(idx).to_string();
    let mut head = vec![
        Span::styled(format!("{} {st}", glyph_of(app, idx)), status(&st)),
        Span::raw("  ·  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
    ];
    // **색은 묶음에만 준다.** `kind != Issue` 로 칠하면 idea 가 에픽과 같은
    // 파랑을 입어, 아무것도 담지 않는 줄이 담는 줄처럼 보인다. CLI 상세가
    // 쓰는 자(`report::is_group`)와 같은 자로 잰다.
    if i.kind != crate::model::Kind::Issue {
        let mark = if crate::report::is_group(i) {
            Style::new().fg(Color::LightBlue)
        } else {
            dim()
        };
        head.push(Span::raw("  ·  "));
        head.push(Span::styled(i.kind.as_str().to_string(), mark));
    }
    // **미룬 것은 여기서 반드시 말한다.** 탐색기는 시키지도 않은 줄을 숨기지
    // 않으므로 미룬 줄이 목록에 그대로 서 있는데, 그것이 `moai ready` 에
    // 안 나오는 까닭은 이 패널 말고는 어디에도 안 적힌다 — 낱말은 CLI 상세와
    // 같은 자리(`view::deferred_for`)에서 받는다. **물려받은 미룸도** 같이 받는다 —
    // 미룬 에픽의 멤버에 표가 없으면 그 까닭이 여기서도 빈다.
    if let Some(d) = crate::view::deferred_for(i, app.index.deferred_root(&i.id), &app.now) {
        head.push(Span::raw("  ·  "));
        head.push(Span::styled(d, Style::new().fg(Color::Yellow)));
    }
    out.push(Line::from(head));
    // 손으로 옮긴 칸이 읽은 칸과 다르면 말한다 — CLI 상세와 같은 말. **제 줄로
    // 낸다**: 머리 줄에 이어 붙이면 80~160칸에서 `fit` 이 그 말을 통째로 잘라, 정작
    // `moai mv <에픽> done` 을 친 사람이 까닭을 못 본다.
    if let Some(n) = crate::view::unread_column(i, &st, &app.cfg) {
        out.extend(wrapped(&n, w, dim()));
    }

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
    // **그 줄이 선 마일스톤을 그린다.** 에픽이 마일스톤을 이기므로(3301f6e) 제 줄에
    // 적은 값은 그 줄이 선 자리와 다를 수 있다 — 그것을 그대로 그리면 패널은 m002 라
    // 말하고 트리는 m001 밑에 둔다(moai-9yrv). 자리는 탐색기가 실제로 둔 경로에서
    // 읽는다(`Index::milestone_of`) — 셈의 지도를 그리면, 에픽의 마일스톤을 세면서도
    // `(마일스톤 없음)` 에 서는 생각에 탐색기가 두지 않은 마일스톤을 댄다.
    // 선 자리와 다른 제 값은 **지우지 않고 그렇다고 적는다** — 적어 둔 값이 화면에서
    // 말없이 사라지면 그게 더 헷갈린다.
    let placed = app.index.milestone_of(&i.id);
    if let Some(id) = placed {
        fields.push(("마일스톤".into(), app.title_of(id)));
    }
    // 까닭은 **선 자리로** 댄다. "에픽의 것을 따른다" 고 적으면, 에픽 없이 부모 밑에
    // 접힌 줄이나 에픽 참조가 끊겨 `(길 잃음)` 에 선 줄에서 거짓이 된다.
    if let Some(own) = i.milestone.as_deref().filter(|own| Some(*own) != placed) {
        let why = if placed.is_some() { "선 자리의 마일스톤을 따른다" } else { "이 줄은 마일스톤 밖에 선다" };
        fields.push(("안 쓰임".into(), format!("제 마일스톤 {own} — {why}")));
    }
    // **막는 것은 제목까지 푼다.** id 만 내면 그것이 무엇인지 또 찾아봐야 한다.
    // **끝난 막음은 막지 않는다** — `report::is_blocked` 와 `moai ready` 가 그
    // 자로 세므로, 여기서만 다 "막힘" 이라 적으면 집을 수 있는 것을 못 집을
    // 것으로 읽는다. 뜻은 `report` 가 정하고 여기는 그 답을 그린다.
    for b in &i.blocked_by {
        let done = app.index.find(b).is_some_and(|at| app.column(at) == crate::config::DONE);
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
        // **없는 것과 안 세는 것은 다르다.** 담아 둔 생각은 자리로는 여기
        // 걸리지만(왼쪽 목록이 그 줄을 낸다) 진행률로는 안 센다 — 세기
        // 시작하면 담을수록 그 부모가 덜 끝난 것으로 보인다. 둘을 한 낱말로
        // 뭉치면 줄이 보이는데 `자식 없음` 이라 말한다(moai-lhbh).
        let word = if kids.is_empty() { "자식 없음" } else { "셀 일 없음" };
        return vec![Line::from(Span::styled(word, dim()))];
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
    // `w` 는 **맨 먼저 떨어진다.** 켜 둔 동안에는 경로 줄의 뱃지가 끄는 법을 대므로,
    // 좁은 창에서 이 자리를 잃어도 나갈 길을 잃지는 않는다.
    let mut optional = vec![
        key("w", if app.worktree { "워크트리 끄기" } else { "워크트리" }),
        key("j·k", "굴리기"),
        // **`F5` 는 `n` 보다 먼저 떨어진다.** 파일이 바뀌면 저절로 다시 읽으므로(`App::follow`)
        // F5 를 누를 일은 드물고, 생각을 담는 길은 이 탐색기가 처음 여는 쓰기다 — 80칸에서
        // 둘 중 하나만 남는다면 담는 길이다.
        key("F5", "갱신"),
        // **`Tab` 은 가는 곳을 댄다** — `F3 원문`·`w 워크트리 끄기` 와 같은 자다.
        // "칸 옮기기" 라 적으면 지금 어디 있는지는 테두리만 말하는데, 가는 곳을 적으면
        // 이 줄도 글자로 지금 자리를 말한다. 자리는 `F3` 앞이다 — 80칸에서 `j·k` 가
        // 떨어진 뒤에도 남는다(`the_key_bar_names_tab_at_eighty_columns`). 여유는 두 칸
        // 뿐이라(`F3 그리기` 로 늘어도 든다), 80칸에서 거름망(`Esc 풀기`)을 걸면
        // `n` 과 함께 `Tab` 이 떨어진다 — 그때도 테두리 모양이 포커스를 말하고, 되돌아올
        // 길(`F3 그리기`·`Esc 풀기`)과 나갈 길(`F10`)이 `Tab` 보다 급하다.
        key("Tab", pane_name(app.focus.next())),
        key("n", "담기"),
        key("F3", if app.raw { "그리기" } else { "원문" }),
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

/// 브랜치 머리표를 이만큼에서 자른다. 긴 브랜치 하나가 제목 몫을 다 먹으면 안 된다.
const BRANCH_CAP: usize = 16;

fn branch() -> Style {
    from_anstyle(style::BRANCH)
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
        // 에픽이 `in_progress` 로 서려면 멤버가 그래야 한다 — 묶음의 칸은 멤버에서
        // 읽는다. 적힌 칸만 옮겨서는 안 선다. 도는 글리프도 이 줄에서 나온다:
        // 묶음은 안 돌리므로(`glyph_of`) 집은 일이 하나는 있어야 한다.
        let mut held = Issue::new(
            "argos-0004".into(),
            "집은 멤버".into(),
            Kind::Issue,
            Status::new("in_progress"),
            "2026-09-01T00:00:00Z",
        );
        held.epic = Some("argos-0001".into());
        vec![epic, member, held]
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
        // 뿌리에는 묶음뿐이다 — **묶음은 안 돈다.** CLI 와 같은 정지 글리프다.
        assert!(lines.contains('▸'), "묶음의 칸 글리프가 없다\n{lines}");
        assert!(!style::SPIN.iter().any(|g| lines.contains(g)), "묶음이 돈다\n{lines}");
        // 그 안의 집은 일은 도는 프레임을 낸다 — 어느 프레임이든 `SPIN` 의 한 글자다.
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let inside = render(&mut a, 100, 12).join("\n");
        assert!(style::SPIN.iter().any(|g| inside.contains(g)), "집은 일이 안 돈다\n{inside}");
        assert!(lines.contains("Enter") && lines.contains("F10"), "F키 바가 없다\n{lines}");
        // 긴 제목은 **잘린다**. 잘렸다는 표시가 남아야 어디까지가 제목인지 안다.
        assert!(lines.contains("아주 긴"), "에픽 제목이 없다\n{lines}");
        assert!(lines.contains('…'), "잘렸는데 표시가 없다\n{lines}");
        // 디렉터리는 제목 뒤에 `/` 가 붙는다
    }

    /// 옆 워크트리에서 온 줄은 목록과 상세 둘 다 `⎇ <브랜치>` 를 댄다. 켜진 동안은
    /// 경로 줄이 그렇다고 말하고, `w` 가 켜고 끈다.
    #[test]
    fn a_line_from_another_worktree_is_marked_in_the_list_and_the_detail() {
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let plain_screen = render(&mut a, 120, 12).join("\n");
        assert!(!plain_screen.contains('⎇'), "안 겹쳤는데 머리표가 섰다\n{plain_screen}");
        assert!(plain_screen.contains("w 워크트리"), "켜는 키를 안 알린다\n{plain_screen}");

        // 저장소 없이 세운 App 이라 `w` 는 켜기만 하고 읽지 않는다 — 겹친 결과는 손으로 넣는다.
        a.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert!(a.worktree, "w 가 안 켰다");
        let mut theirs = issues()[2].clone();
        theirs.status = crate::model::Status::new("review");
        theirs.updated_at = "2026-09-02T00:00:00Z".into();
        let (all, origin) = crate::worktree::overlay(
            issues(),
            vec![("feat/x".into(), std::path::PathBuf::from("/wt"), vec![theirs])],
        );
        a.adopt(all);
        a.origin = origin;
        // 커서를 옆에서 온 줄에 둔다.
        let at = a.rows().iter().position(|r| matches!(r, Row::Item(e) if e.at().is_some_and(|i| a.issues[i].id == "argos-0004"))).unwrap();
        a.cursor = at;
        let lines = render(&mut a, 120, 12);
        let screen = lines.join("\n");
        let row = lines.iter().find(|l| l.contains("argos-0004") && l.contains("집은 멤버")).expect("줄이 없다");
        assert!(row.contains("⎇ feat/x 집은 멤버"), "목록에 머리표가 없다\n{screen}");
        assert!(lines[0].contains("⎇ feat/x") && lines[0].contains("w 로 끈다"), "켜졌다고 안 말한다\n{screen}");
        assert!(screen.matches("⎇ feat/x").count() >= 3, "상세에 머리표가 없다\n{screen}");
        assert!(screen.contains("w 워크트리 끄기"), "{screen}");

        a.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert!(!a.worktree, "w 가 안 껐다");
    }

    /// 걸음은 **그린 횟수가 아니라 시계가** 올린다. 다시 그리기만 해서는
    /// 안 돌아야 하고 — 안 그러면 키를 누르는 동안 타이핑 속도로 돈다 —
    /// 한 화면 안에서는 목록과 상세가 **같은 걸음**을 보여야 한다.
    #[test]
    fn the_spinner_follows_the_clock_and_shows_one_step_per_screen() {
        let mut a = app();
        // 도는 것은 일이다 — 집은 멤버가 있는 에픽 안에서 본다.
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

    /// **상세는 자리를 정한 마일스톤을 그린다.** 에픽이 마일스톤을 이기므로 제
    /// 줄에 적은 값은 셈에 안 쓰인다 — 그것을 `마일스톤` 이라 그리면 패널과
    /// 트리가 같은 줄을 다른 마일스톤에 둔다(moai-9yrv). 제 값은 안 쓰인다고 적는다.
    #[test]
    fn the_detail_shows_the_milestone_that_places_the_row() {
        let make = |id: &str, title: &str, kind: Kind| {
            Issue::new(id.into(), title.into(), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let used = make("argos-0001", "쓰는 판", Kind::Milestone);
        let unused = make("argos-0002", "안 쓰는 판", Kind::Milestone);
        let mut epic = make("argos-0003", "에픽", Kind::Epic);
        epic.milestone = Some("argos-0001".into());
        let mut row = make("argos-0004", "멤버", Kind::Issue);
        row.epic = Some("argos-0003".into());
        row.milestone = Some("argos-0002".into());
        let path = vec![
            crate::nav::Seg::Milestone(Some("argos-0001".into())),
            crate::nav::Seg::Epic("argos-0003".into()),
        ];
        let mut a = App::new(
            vec![used, unused, epic, row],
            Config::parse("prefix = \"argos\"\n").unwrap(),
            path,
        );
        a.cursor = 1; // 0 은 `..` 줄이다
        let lines = render(&mut a, 140, 24);
        let label = |name: &str| lines.iter().find(|l| l.contains(name)).cloned().unwrap_or_default();
        assert!(label("마일스톤").contains("쓰는 판"), "{lines:#?}");
        assert!(!label("마일스톤").contains("안 쓰는 판"), "안 쓰이는 제 값을 마일스톤으로 그렸다\n{lines:#?}");
        assert!(label("안 쓰임").contains("argos-0002"), "제 값이 안 쓰인다고 안 말한다\n{lines:#?}");
    }

    /// 에픽 참조가 끊겨 자리를 정한 마일스톤이 없으면, 안 쓰이는 제 값을
    /// "에픽의 것을 따른다" 고 적지 않는다 — 따를 에픽의 마일스톤이 없다.
    #[test]
    fn an_unused_milestone_without_a_placed_one_does_not_claim_to_follow_the_epic() {
        let make = |id: &str, title: &str, kind: Kind| {
            Issue::new(id.into(), title.into(), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let ms = make("argos-0001", "판", Kind::Milestone);
        let mut row = make("argos-0004", "멤버", Kind::Issue);
        row.epic = Some("argos-9999".into());
        row.milestone = Some("argos-0001".into());
        let mut a = App::new(
            vec![ms, row],
            Config::parse("prefix = \"argos\"\n").unwrap(),
            vec![crate::nav::Seg::Lost],
        );
        a.cursor = 1; // 0 은 `..` 줄이다
        let lines = render(&mut a, 140, 24);
        let unused = lines.iter().find(|l| l.contains("안 쓰임")).cloned().unwrap_or_default();
        assert!(unused.contains("argos-0001"), "{lines:#?}");
        assert!(!unused.contains("에픽의 것을 따른다"), "따를 에픽 마일스톤이 없는데 따른다고 적었다\n{lines:#?}");
    }

    /// **생각은 에픽의 마일스톤을 세도 `(마일스톤 없음)` 에 선다.** 상세가 셈의 지도
    /// (`report::milestones`)를 그리면, 목록은 `(마일스톤 없음)` 인데 상세만 그 에픽의
    /// 마일스톤을 댄다 — 탐색기가 두지 않은 자리다.
    #[test]
    fn a_thought_shows_no_milestone_it_does_not_stand_under() {
        let make = |id: &str, title: &str, kind: Kind| {
            Issue::new(id.into(), title.into(), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let stone = make("argos-0001", "릴리스 판", Kind::Milestone);
        let mut epic = make("argos-0002", "에픽", Kind::Epic);
        epic.milestone = Some("argos-0001".into());
        let mut thought = make("argos-0003", "샤딩", Kind::Idea);
        thought.epic = Some("argos-0002".into());
        let mut a = App::new(
            vec![stone, epic, thought],
            Config::parse("prefix = \"argos\"\n").unwrap(),
            vec![crate::nav::Seg::Milestone(None)],
        );
        a.cursor = 1; // 0 은 `..` 줄이다
        let lines = render(&mut a, 180, 24);
        assert!(lines.iter().any(|l| l.contains("샤딩")), "그 생각이 목록에 없다\n{lines:#?}");
        assert!(!lines.iter().any(|l| l.contains("릴리스 판")), "선 자리에 없는 마일스톤을 댔다\n{lines:#?}");
    }

    /// **물려받은 미룸도 상세가 말한다.** 미룬 에픽의 멤버는 `ready` 에도 보드에도
    /// 없는데, 제 `deferred_at` 만 보면 이 패널에 표가 없어 까닭이 어디에도 안 적힌다.
    #[test]
    fn the_detail_names_an_inherited_deferral() {
        let make = |id: &str, title: &str, kind: Kind| {
            Issue::new(id.into(), title.into(), kind, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let mut epic = make("argos-0001", "다음 분기", Kind::Epic);
        epic.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut row = make("argos-0002", "파서", Kind::Issue);
        row.epic = Some("argos-0001".into());
        let mut a = App::new(
            vec![epic, row],
            Config::parse("prefix = \"argos\"\n").unwrap(),
            vec![crate::nav::Seg::Epic("argos-0001".into())],
        );
        a.cursor = 1; // 0 은 `..` 줄이다
        let lines = render(&mut a, 180, 24);
        assert!(lines.iter().any(|l| l.contains("미룸 — argos-0001 밑")), "{lines:#?}");
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
        assert!(lines.contains("1/2"), "진행이 없다\n{lines}");
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
        a.unreadable = vec![None; 3];
        let lines = render(&mut a, 100, 14).join("\n");
        assert!(lines.contains("읽을 수 없는 줄 3개"), "{lines}");
    }

    /// 쓰기의 실패도 같은 자리에 서고, **무엇을 못 했는지는 단 쪽의 말 그대로다** —
    /// 배너가 "다시 읽지 못했다" 를 덧붙이면 쓰기 실패가 읽기 실패로 거짓말한다.
    #[test]
    fn a_failed_write_is_told_as_a_write() {
        let mut a = app();
        a.trouble = Some("쓰지 못했다 — 락".into());
        let lines = render(&mut a, 100, 14).join("\n");
        assert!(lines.contains("쓰지 못했다 — 락") && !lines.contains("다시 읽지"), "{lines}");
    }

    /// 경로 줄이 **언제 읽은 화면인지** 댄다. 저절로 다시 읽으므로 배너는 없고,
    /// 대신 시각이 바뀌는 것으로 갱신된 줄 안다.
    #[test]
    fn the_crumbs_say_when_the_screen_was_read() {
        let mut a = app();
        a.now = "2026-09-13T13:42:07Z".into();
        let lines = render(&mut a, 100, 14);
        assert!(lines[0].contains("↻ 13:42:07"), "{:?}", lines[0]);
        assert!(!lines.join("\n").contains("파일이 바뀌었다"), "배너가 남았다");
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

        // 그 안에는 일이 둘 있다 — 이제 칸 셈이 선다
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 12);
        let title = lines.iter().find(|l| l.contains('┌')).unwrap();
        assert!(title.contains("done 1") && title.contains("in_progress 1"), "{title:?}");
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
    /// 것으로 읽는다. 막는 것이 묶음이면 **서 있는 칸**으로 끝났는지 본다 —
    /// 적힌 칸만 done 인 에픽은 아직 막는다.
    #[test]
    fn a_finished_blocker_is_not_drawn_as_blocking() {
        let drawn = |close_members: bool| {
            let mut issues = issues();
            issues[0].status = Status::new("done"); // 막는 쪽의 적힌 칸
            if close_members {
                issues[2].status = Status::new("done"); // 막는 쪽이 정말 끝났다
            }
            issues[1].blocked_by = vec!["argos-0001".into()];
            let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
            a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            render(&mut a, 100, 20).join("\n")
        };
        let lines = drawn(true);
        assert!(lines.contains("풀림"), "끝난 막음을 아직 막혔다고 그린다\n{lines}");
        assert!(!lines.contains("막힘"), "{lines}");
        let lines = drawn(false);
        assert!(lines.contains("막힘"), "적힌 칸만 닫힌 에픽을 풀렸다고 그린다\n{lines}");
    }

    /// **목록과 상세가 묶음의 읽은 칸을 그린다** — CLI 와 같은 자. 손으로 옮긴
    /// 칸이 다르면 상세가 낱말로 말한다. **흔한 폭에서 본다** — 그 말을 머리 줄에
    /// 이어 붙이면 80~160칸에서 통째로 잘려, 옮긴 사람이 까닭을 못 본다.
    #[test]
    fn a_grouping_is_drawn_in_the_column_its_members_read() {
        let mut issues = issues();
        issues[0].status = Status::new("done");
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        for w in [100, 200] {
            let lines = render(&mut a, w, 16).join("\n");
            let row = lines.lines().find(|l| l.contains("> argos-0001")).unwrap();
            assert!(row.contains('▸'), "목록이 적힌 칸을 그린다 ({w}칸)\n{lines}");
            // 좁은 폭에서는 접힌다 — 잘리지만 않으면 된다.
            assert!(
                lines.contains("in_progress")
                    && lines.contains("칸은 멤버에서 읽는다")
                    && lines.contains("`done`"),
                "{w}칸에서 잘렸다\n{lines}"
            );
        }
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
        // 테두리가 맞붙어 `┃│` 라 가운데 빈 조각이 낀다. 빈 조각을 빼면
        // 앞이 좌측, 뒤가 우측이다. **포커스 있는 칸은 굵은 선이다** — 한쪽만
        // 가르면 목록 전체가 한 조각으로 붙는다.
        let panes = |l: &str| -> Vec<String> {
            l.split(['│', '┃']).filter(|s| !s.is_empty()).map(str::to_string).collect()
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
        assert!(first.contains("↓ ") && first.contains("줄 (j·k)"), "남은 줄을 안 알린다\n{first}");

        for _ in 0..12 {
            a.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        }
        let last = render(&mut a, 100, 16).join("\n");
        assert!(last.contains("40번째"), "끝까지 못 굴렸다\n{last}");
        assert!(!last.contains("↓ "), "다 보이는데 더 있다고 한다\n{last}");
        assert!(last.contains("· 끝"), "끝까지 굴렸는데 끝이라 안 한다\n{last}");

        // **되돌아오는 길도 한 번에 열린다.** 그리는 데만 자르고 `App` 에 큰
        // 수를 남겨 두면, 끝까지 굴린 뒤 `k` 를 눌러도 그 수가 상한 밑으로
        // 내려올 때까지 화면이 한 칸도 안 움직여 키가 죽은 것처럼 보인다.
        a.key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        let up = render(&mut a, 100, 16).join("\n");
        assert_ne!(up, last, "끝까지 굴린 뒤 k 가 아무 일도 안 한다");
        assert!(up.contains("↓ 1줄"), "되돌아왔는데 남은 줄을 안 알린다\n{up}");
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
        let _ = render(&mut a, 100, 16);
        a.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(a.detail.offset() > 0);
        a.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(a.detail.offset(), 0, "굴린 자리를 들고 다른 줄로 갔다");
    }

    /// **상세에 포커스를 두고 `End` 를 누르면 본문 끝이 보이고, 곧바로 `↑` 가 듣는다.**
    /// `App` 은 줄 수를 모르므로 큰 수를 넣는다 — 그림이 그것을 끝으로 잘라 도로
    /// 넣지 않으면 `↑` 를 수만 번 눌러야 화면이 움직인다.
    #[test]
    fn end_in_the_detail_reaches_the_last_line_and_up_answers_at_once() {
        let mut issues = issues();
        issues[1].body = Some((1..=40).map(|n| format!("{n}번째 줄이다\n\n")).collect::<String>());
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let cursor = a.cursor;

        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let end = render(&mut a, 100, 16).join("\n");
        assert!(end.contains("40번째"), "End 가 본문 끝에 안 닿았다\n{end}");
        assert_eq!(a.cursor, cursor, "상세에서 End 를 눌렀는데 목록이 움직였다");

        a.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let up = render(&mut a, 100, 16).join("\n");
        assert_ne!(up, end, "↑ 를 눌렀는데 화면이 그대로다");
    }

    /// **포커스는 색 없이도 읽힌다.** 버퍼의 글자만 보고 어느 칸이 굵은 선인지
    /// 갈려야 한다 — 색만 바꾸면 색 없는 터미널에서 `Tab` 을 누른 뒤 `↓` 가 상세를
    /// 굴리는데 화면에는 아무 표시가 없어, 목록이 죽은 줄 안다.
    #[test]
    fn the_focused_pane_is_marked_by_its_border_shape_without_colour() {
        let mut a = app();
        let top = |a: &mut App| {
            let lines = render(a, 100, 12);
            let at = lines.iter().position(|l| l.contains('┐') || l.contains('┓')).expect("윗 테두리가 없다");
            (lines[at].clone(), lines.join("\n"))
        };
        let (t, screen) = top(&mut a);
        assert!(t.starts_with('┏') && t.ends_with('┐'), "목록에 포커스가 있는데 모양이 안 갈린다 — {t:?}\n{screen}");
        assert_eq!(screen.matches('┏').count(), 1, "굵은 칸이 둘이다\n{screen}");

        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let (t, screen) = top(&mut a);
        assert!(t.starts_with('┌') && t.ends_with('┓'), "Tab 뒤에 굵은 선이 상세로 안 옮겼다 — {t:?}\n{screen}");
        assert_eq!(screen.matches('┏').count(), 1, "굵은 칸이 둘이다\n{screen}");
        // 옆과 아래 테두리도 같은 모양이다 — 윗줄만 굵으면 긴 목록에서 윗줄이 멀다.
        assert!(screen.contains('┃') && screen.contains('┛'), "굵은 선이 윗줄에만 섰다\n{screen}");

        // 글을 받는 중에도 테두리는 제자리다 — 포커스가 안 옮겼으니 그림도 안 옮긴다.
        a.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let (t, _) = top(&mut a);
        assert!(t.ends_with('┓'), "글 받는 중에 포커스 표시가 옮겼다 — {t:?}");
    }

    /// **초록은 곁들임이지만 빠지면 안 된다** — 사용자 기획이 "초록 테두리" 다.
    /// 모양만 보는 시험으로는 색을 걷어 내도 조용히 지나가므로 따로 본다. 포커스
    /// 없는 칸은 칠하지 않는다 — 둘 다 칠하면 색이 아무 말도 안 한다.
    #[test]
    fn the_focused_border_is_green_and_the_other_is_not() {
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let mut term = Terminal::new(TestBackend::new(100, 12)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let y = (0..buf.area.height).find(|&y| buf[(0, y)].symbol() == "┌").expect("목록 윗 테두리가 없다");
        let right = buf.area.width - 1;
        assert_eq!(buf[(right, y)].symbol(), "┓");
        assert_eq!(buf[(right, y)].fg, Color::Green, "포커스 칸 테두리가 초록이 아니다");
        assert_ne!(buf[(0, y)].fg, Color::Green, "포커스 없는 칸까지 칠했다");
    }

    /// **F키 바가 `Tab` 을 말한다. 80칸에서도.** 모르는 키는 없는 키다 — 그리고
    /// 가는 곳을 대므로 지금 어디 있는지를 글자로도 말한다.
    #[test]
    fn the_key_bar_names_tab_at_eighty_columns() {
        for w in [80u16, 100, 120] {
            let mut a = app();
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.contains("Tab 상세") && bar.contains("F10 끝내기"), "{w}칸 — {bar:?}");
            a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.contains("Tab 목록") && bar.contains("F10 끝내기"), "{w}칸 — {bar:?}");
        }
    }

    /// **목록도 굴릴 것이 남았으면 말한다** — 상세와 같은 자리(아래 테두리), 같은
    /// 글이다. 짧은 목록과 짧은 상세에는 표시가 없다.
    #[test]
    fn both_panes_mark_what_is_left_to_scroll_in_the_same_place() {
        let many: Vec<Issue> = (1..=30)
            .map(|n| {
                Issue::new(format!("argos-{n:04}"), format!("일 {n}"), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z")
            })
            .collect();
        let mut a = App::new(many, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        // 창 14줄 → 경로·배너·키 바를 뺀 몸통 11줄 → 테두리를 뺀 목록 안쪽 9줄.
        // 30줄 중 21줄이 아래에 숨는다.
        let lines = render(&mut a, 100, 14);
        let bottom = lines.iter().rev().find(|l| l.contains('└')).cloned().unwrap_or_default();
        assert!(bottom.contains("↓ 21줄"), "목록 아래 테두리에 표시가 없다\n{}", lines.join("\n"));
        assert!(!lines.iter().any(|l| l.contains("(j·k)")), "짧은 상세에 표시가 섰다\n{}", lines.join("\n"));

        // 커서를 끝으로 — 목록이 따라 굴러 끝이라 말한다
        a.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 14);
        let bottom = lines.iter().rev().find(|l| l.contains('└')).cloned().unwrap_or_default();
        assert!(bottom.contains("↑ 21줄 · 끝"), "{}", lines.join("\n"));
        assert!(lines.iter().any(|l| l.contains("> argos-0030")), "커서가 안 보인다\n{}", lines.join("\n"));

        // 짧은 목록과 다 들어가는 상세에는 표시가 없다
        let lines = render(&mut app(), 100, 20).join("\n");
        assert!(!lines.contains('↓') && !lines.contains('↑'), "굴릴 것이 없는데 표시가 섰다\n{lines}");
    }

    /// **포커스 칸의 스크롤 표시는 테두리의 초록을 끊지 않는다.** 표시가 회색으로
    /// 덮으면 굴릴 것이 남은 포커스 칸만 아래 테두리의 초록이 비어 보인다.
    #[test]
    fn the_scroll_mark_on_a_focused_pane_keeps_the_focus_colour() {
        let many: Vec<Issue> = (1..=30)
            .map(|n| {
                Issue::new(format!("argos-{n:04}"), format!("일 {n}"), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z")
            })
            .collect();
        let mut a = App::new(many, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let mut term = Terminal::new(TestBackend::new(100, 14)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let y = (0..buf.area.height).find(|&y| buf[(0, y)].symbol() == "┗").expect("포커스 칸 아래 테두리가 없다");
        let x = (0..buf.area.width).find(|&x| buf[(x, y)].symbol() == "↓").expect("목록에 스크롤 표시가 없다");
        assert_eq!(buf[(x, y)].fg, Color::Green, "포커스 칸의 표시가 테두리 색을 덮었다");

        // 포커스를 옮기면 목록 표시는 여느 회색으로 돌아간다
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let x = (0..buf.area.width).find(|&x| buf[(x, y)].symbol() == "↓").expect("목록에 스크롤 표시가 없다");
        assert_ne!(buf[(x, y)].fg, Color::Green, "포커스 없는 칸의 표시까지 칠했다");
    }

    /// **목록이 줄면 빈 줄을 보이며 서 있지 않는다.** 끝까지 내려가 있던 목록이
    /// 다시 읽혀 짧아지면 끝으로 당겨 칸을 채운다.
    #[test]
    fn a_shrunk_list_is_pulled_back_to_fill_the_pane() {
        let make = |n: usize| {
            Issue::new(format!("argos-{n:04}"), format!("일 {n}"), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z")
        };
        let mut a = App::new((1..=30).map(make).collect(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let _ = render(&mut a, 100, 14);
        a.cursor = 11; // 다시 읽혀 커서가 위쪽 줄에 섰다
        a.adopt((1..=12).map(make).collect());
        let lines = render(&mut a, 100, 14);
        // 안쪽 9줄에 12줄 — 끝으로 당기면 4 번부터 12 번까지 빈 줄 없이 선다.
        let shown = lines.iter().filter(|l| (l.starts_with('│') || l.starts_with('┃')) && l.contains("argos-")).count();
        assert_eq!(shown, 9, "빈 줄을 보이며 서 있다\n{}", lines.join("\n"));
        assert!(lines.iter().any(|l| l.contains("argos-0004")), "{}", lines.join("\n"));
    }

    /// **누군지 묻는 칸은 왜 묻는지와 다시 안 묻게 하는 법을 함께 댄다.** 받은 것은
    /// 세션 동안만 들므로 설정하는 법을 안 대면 열 때마다 묻는다. 80칸에서도 그 법이
    /// 안 잘린다. 거절된 까닭은 글칸 줄에 선다.
    #[test]
    fn the_question_names_the_git_setting_that_stops_it() {
        let ask = |error: Option<String>| {
            Mode::Ask(super::super::Ask {
                input: Input::new("레이븐"),
                error,
                why: "git 사용자 정보가 `이름 (메일)` 로 쓸 수 없는 모양이다 — \"레이븐 (raven)\"".into(),
                back: Box::new(Mode::Browse),
                then: |_| {},
            })
        };
        let mut a = app();
        a.mode = ask(None);
        let lines = render(&mut a, 80, 12);
        let (why, line) = (&lines[10], &lines[11]);
        assert!(why.contains("git config user.name·user.email") && why.contains("다시 안 묻는다"), "{why}");
        // 넓으면 무엇이 틀렸는지도 선다 — 설정이 있는데 모양이 틀린 사람에게 "모른다" 고만 하지 않는다.
        let wide = render(&mut a, 200, 12);
        assert!(wide[10].contains("쓸 수 없는 모양이다"), "{}", wide[10]);
        // 한 줄만 남아도 글칸이 보인다
        let low = render(&mut a, 80, 4);
        assert!(low[3].contains(" 누구 "), "{low:?}");
        assert!(line.contains(" 누구 ") && line.contains("레이븐") && line.contains("Esc 그만"), "{line}");
        assert!(!lines.join("\n").contains("F10"), "묻는 동안 F키 바가 섰다");

        a.mode = ask(Some("`이름 (메일)` 모양이 아니다".into()));
        let line = render(&mut a, 80, 12)[11].clone();
        assert!(line.contains("모양이 아니다") && !line.contains("Esc 그만"), "{line}");
        // 좁아도 무너지지 않는다
        render(&mut a, 20, 6);
    }

    fn press(a: &mut App, code: KeyCode) {
        a.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn typed(a: &mut App, s: &str) {
        for c in s.chars() {
            press(a, KeyCode::Char(c));
        }
    }

    /// **생각 담기 폼은 80칸에서 색 없이 읽힌다.** 키를 먹는 칸은 굵은 선(`┏`)이고 화면에
    /// 굵은 칸은 그것 하나다 — 폼이 뒤의 목록·상세를 통째로 덮는다. 칸 이름이 테두리에 서고,
    /// 담는 법·칸 옮기는 법·닫는 법이 맨 아랫줄에 다 든다. `Tab` 이면 굵은 선이 본문으로 간다.
    #[test]
    fn the_idea_form_reads_without_colour_at_eighty_columns() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        typed(&mut a, "떠오른 것");
        let lines = render(&mut a, 80, 24);
        let screen = lines.join("\n");
        let thick = |lines: &[String]| lines.iter().filter(|l| l.contains('┏')).cloned().collect::<Vec<_>>();
        let t = thick(&lines);
        assert_eq!(t.len(), 1, "굵은 칸이 하나가 아니다 — 폼이 뒤의 칸을 다 못 덮었다\n{screen}");
        assert!(t[0].contains("제목"), "제목 칸이 굵지 않다\n{screen}");
        assert!(lines.iter().any(|l| l.contains('┌') && l.contains("본문")), "본문 칸이 없다\n{screen}");
        assert!(screen.contains("떠오른 것"), "{screen}");
        let bar = lines.last().unwrap();
        for hint in ["Ctrl-S·F2 담기", "Tab 본문", "Enter 본문으로", "Esc 닫기"] {
            assert!(bar.contains(hint), "80칸에서 `{hint}` 가 없다 — {bar:?}");
        }
        assert!(!screen.contains("F10"), "폼이 열렸는데 F키 바가 섰다\n{screen}");

        press(&mut a, KeyCode::Tab);
        let lines = render(&mut a, 80, 24);
        let t = thick(&lines);
        assert!(t.len() == 1 && t[0].contains("본문"), "Tab 뒤에 굵은 선이 본문으로 안 갔다\n{}", lines.join("\n"));
        let bar = lines.last().unwrap();
        assert!(bar.contains("Tab 제목") && bar.contains("Enter 줄 나누기"), "{bar:?}");
    }

    /// 굵은 선은 **초록이기도 하다** — 목록·상세의 포커스와 같은 색이다. 모양만 보는
    /// 시험으로는 색이 빠져도 조용히 지나간다.
    #[test]
    fn the_focused_form_field_is_green() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let (x, y) = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .find(|&(x, y)| buf[(x, y)].symbol() == "┏")
            .expect("굵은 칸이 없다");
        assert_eq!(buf[(x, y)].fg, Color::Green);
    }

    /// 빈 제목의 거절과 버릴지 묻는 말은 **맨 아랫줄에 낱말로** 선다. 묻는 동안은 친 키가
    /// 글자로 안 들어가므로 터미널 커서도 안 선다.
    #[test]
    fn the_form_says_why_it_refused_and_asks_before_dropping_in_words() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        press(&mut a, KeyCode::F(2));
        let bar = render(&mut a, 80, 24).last().cloned().unwrap_or_default();
        assert!(bar.contains("제목이 비었다"), "{bar:?}");

        typed(&mut a, "적던 것");
        press(&mut a, KeyCode::Esc);
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        assert!(!term.backend().cursor_visible(), "묻는 중인데 글 안에 커서가 섰다");
        let bar = render(&mut a, 80, 24).last().cloned().unwrap_or_default();
        assert!(bar.contains("버릴까") && bar.contains("y 버린다"), "{bar:?}");
    }

    /// 터미널 커서는 **키를 먹는 칸의 글 안 제자리에** 선다 — 제목에서는 제목 끝, 본문에서는
    /// 커서가 선 줄. 칸이 낮아 본문이 굴러도 커서 줄이 보인다.
    #[test]
    fn the_terminal_cursor_stands_in_the_focused_field() {
        use ratatui::backend::Backend;
        let cursor = |a: &mut App, w: u16, h: u16| {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| screen(f, a)).unwrap();
            let buf = term.backend().buffer().clone();
            let at = term.backend_mut().get_cursor_position().unwrap();
            (term.backend().cursor_visible(), at, buf)
        };
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        typed(&mut a, "가a");
        let (shown, at, buf) = cursor(&mut a, 80, 24);
        assert!(shown);
        // 제목 칸 안쪽 첫 칸 뒤로 `가`(2칸)·`a`(1칸)
        let x0 = (0..80).find(|&x| buf[(x, at.y)].symbol() == "가").expect("커서 줄에 제목이 없다");
        assert_eq!(at.x, x0 + 3, "커서가 제목 끝에 안 섰다");

        press(&mut a, KeyCode::Tab);
        for n in 0..30 {
            typed(&mut a, &format!("줄{n}"));
            press(&mut a, KeyCode::Enter);
        }
        typed(&mut a, "끝");
        let (shown, at, buf) = cursor(&mut a, 80, 24);
        assert!(shown);
        assert_eq!(buf[(at.x - 2, at.y)].symbol(), "끝", "커서가 본문 끝 줄에 안 섰다");
        let lines = render(&mut a, 80, 24).join("\n");
        assert!(lines.contains("↑ "), "굴린 본문이 위에 남은 줄을 안 댄다\n{lines}");
    }

    /// **담다가 누군지 물으면 적던 폼이 뒤에 그대로 보인다.** 키는 묻는 칸이 먹으므로 폼에
    /// 굵은 칸은 없고, 커서는 묻는 칸에 선다.
    #[test]
    fn the_form_stays_behind_the_question_without_the_focus() {
        use ratatui::backend::Backend;
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        typed(&mut a, "적던 것");
        let form = std::mem::replace(&mut a.mode, Mode::Browse);
        a.mode = Mode::Ask(super::super::Ask {
            input: Input::new("레이"),
            error: None,
            why: "누가 하는지 모른다".into(),
            back: Box::new(form),
            then: |_| {},
        });
        let lines = render(&mut a, 80, 24);
        let shown = lines.join("\n");
        assert!(shown.contains("적던 것") && shown.contains("생각 담기"), "묻는 동안 폼이 사라졌다\n{shown}");
        assert!(!shown.contains('┏'), "묻는 중인데 폼의 칸이 굵다\n{shown}");
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        assert_eq!(term.backend_mut().get_cursor_position().unwrap().y, 23, "커서가 묻는 칸에 안 섰다");
    }

    /// 폼이 열린 채로 **좁고 낮은 창**에서도 무너지지 않고 줄이 넘치지 않는다.
    #[test]
    fn the_idea_form_survives_tiny_windows() {
        let mut a = app();
        press(&mut a, KeyCode::Char('n'));
        typed(&mut a, "아주 긴 한글 제목이 여기 들어가서 좁은 창을 넘친다");
        press(&mut a, KeyCode::Tab);
        typed(&mut a, "본문");
        press(&mut a, KeyCode::Enter);
        typed(&mut a, "둘째 줄");
        for w in [1u16, 4, 10, 20, 39, 40, 60] {
            for h in [1u16, 2, 3, 4, 6, 10] {
                for l in render(&mut a, w, h) {
                    assert!(crate::text::width(&l) <= w as usize, "{w}x{h}: {l:?}");
                }
            }
        }
    }

    /// **F키 바가 `n` 을 댄다. 80칸에서도** — 담는 길이 안 보이면 없는 길이다. 떨어지는
    /// 것은 저절로 다시 읽어 누를 일이 드문 `F5` 다. `Tab`·`F10` 은 그대로 남는다.
    #[test]
    fn the_key_bar_names_n_at_eighty_columns() {
        let bar = render(&mut app(), 80, 14).last().cloned().unwrap_or_default();
        assert!(bar.contains("n 담기") && bar.contains("Tab 상세") && bar.contains("F10 끝내기"), "{bar:?}");
        let wide = render(&mut app(), 120, 14).last().cloned().unwrap_or_default();
        assert!(wide.contains("F5 갱신") && wide.contains("n 담기"), "{wide:?}");
    }

    /// 빈 저장소도 그려진다.
    #[test]
    fn an_empty_repo_still_draws() {
        let mut empty = App::new(Vec::new(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let lines = render(&mut empty, 60, 10).join("\n");
        assert!(lines.contains("비었다"), "{lines}");
    }

    /// 글칸의 커서는 **터미널 커서**가 글 안 제 자리에 선다. 한글은 두 칸이고,
    /// 글이 칸보다 길면 조각이 커서를 따라 밀려 커서가 줄 밖으로 안 나간다.
    #[test]
    fn the_prompt_puts_the_terminal_cursor_where_typing_lands() {
        use ratatui::backend::Backend;
        let press = |a: &mut App, code| a.key(KeyEvent::new(code, KeyModifiers::NONE));
        let cursor_after = |a: &mut App, w: u16| {
            let mut term = Terminal::new(TestBackend::new(w, 6)).unwrap();
            term.draw(|f| screen(f, a)).unwrap();
            let last = term.backend().buffer().area.height - 1;
            let row: String = (0..w).map(|x| term.backend().buffer()[(x, last)].symbol().to_string()).collect();
            (term.backend().cursor_visible(), term.backend_mut().get_cursor_position().unwrap(), row)
        };

        let mut a = app();
        let (shown, _, _) = cursor_after(&mut a, 60);
        assert!(!shown, "글을 안 받는데 커서가 보인다");

        // ` 검색 ` 여섯 칸 + 빈칸 하나 뒤에서 글이 시작한다.
        press(&mut a, KeyCode::Char('/'));
        for c in "가a".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        let (shown, at, row) = cursor_after(&mut a, 60);
        assert!(shown, "{row}");
        assert_eq!((at.x, at.y), (7 + 3, 5), "{row}");
        press(&mut a, KeyCode::Left);
        press(&mut a, KeyCode::Left);
        assert_eq!(cursor_after(&mut a, 60).1.x, 7, "Home 자리");

        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        for c in "abcdefghijklmnopqrstuvwxyz".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        let (_, at, row) = cursor_after(&mut a, 20);
        assert!(at.x < 20, "커서가 줄 밖에 섰다 {at:?}");
        assert!(row.contains("xyz") && !row.contains("abc"), "끝이 안 보인다: {row}");
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
            let m = super::Mode::Grep(super::Input::new(&q));
            let _ = app.apply(&m);
        }
        if let Ok(q) = std::env::var("EYE_TYPING") {
            app.mode = super::Mode::Filter(super::Input::new(&q));
        }
        let h: u16 = std::env::var("EYE_H").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
        for l in super::tests::render(&mut app, 96, h) {
            println!("{l}");
        }
    }
}
