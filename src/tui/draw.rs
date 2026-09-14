//! 그림. **뜻을 판단하지 않는다** — 무엇이 어디 걸리는지는 `nav`, 무엇을
//! 세는지는 `report` 가 이미 정했다.
//!
//! 규칙 하나를 CLI 에서 그대로 들고 온다: **색이 혼자 뜻을 지지 않는다.**
//! 모든 색에 글리프나 낱말이 붙는다.

use super::form::{Field, Form, Target};
use super::keys::{self, BROWSE, Browse, CONFIRM, Confirm, Ctx, Goto, JOT, Jot, LEADER, MENU, Menu, PATH, PICK, PROMPT, Pick, Prompt, label, labels};
use super::menu;
use super::scroll::Move;
use super::scroll::Scroll;
use super::layer::{Look, Place, Shut};
use super::picker::{self, Picker};
use super::{App, Input, Mode, Pane, Row};
use crate::nav::Entry;
use crate::query::GrepIn;
use crate::report::Blocker;
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
    // SPC 메뉴는 탐색 중에만 선다 — 글칸으로 넘어가면 열이 버려져 저절로 닫힌다.
    //
    // **창은 몸통을 밀어 올린다**(moai-apsa). 덮으면 커서가 선 줄이 창 밑에 숨어, 메뉴가 무엇에
    // 대한 것인지를 잃는다. 목록·상세는 줄어든 높이로 커서를 드러내므로(`Scroll::reveal`) 따로
    // 굴리지 않는다. 격자에 줄 높이는 몸통의 몫([`menu::BODY_MIN`])을 먼저 남기고 정하고, 그것도
    // 없으면 격자 없이 접두어 줄 한 줄로 접는다([`menu_line`]).
    let area = f.area();
    let open_menu = (matches!(app.mode, Mode::Browse) && menu::open(&app.chord)).then(|| {
        let items = menu::entries(app.chord.held(), &app.key_ctx(&rows), &app.cfg.statuses);
        let left = area.height.saturating_sub(1 + banner_h + keys_h) as usize;
        let grid = menu::grid(&items, (area.width as usize).saturating_sub(2), menu::rows_for(left));
        (items, grid)
    });
    let panel_h = open_menu.as_ref().map_or(0, |(_, g)| if g.rows == 0 { 0 } else { g.rows as u16 + 1 });
    let [top, note, body, panel, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(banner_h),
        Constraint::Min(1),
        Constraint::Length(panel_h),
        Constraint::Length(keys_h),
    ])
    .areas(area);
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(LEFT), Constraint::Min(10)]).areas(body);

    crumbs(f, app, &rows, top);
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
    // 담을 곳의 색은 **폼을 그리기 전에** 층에서 찾아 둔다 — 폼은 `app.mode` 를 빌려 쓰는
    // 동안 층을 못 본다. 폼의 `Target` 은 조각(`form.rs`)이라 `style` 을 모르므로 색을 들지 않고,
    // 정한 색(moai-o04b)은 경로로 층에서 다시 찾는다.
    let tint = match &app.mode {
        Mode::Idea(form) => form.into.as_ref(),
        Mode::Ask(ask) => match ask.back.as_ref() {
            Mode::Idea(form) => form.into.as_ref(),
            _ => None,
        },
        _ => None,
    }
    .map(|into| project_style_at(app, &into.path))
    .unwrap_or_default();
    match &mut app.mode {
        Mode::Idea(form) => jot(f, form, body, true, tint),
        Mode::Pick(p) => pick(f, p, body),
        Mode::Ask(ask) => {
            if let Mode::Idea(form) = ask.back.as_mut() {
                jot(f, form, body, false, tint);
            }
        }
        _ => {}
    }
    if let Some((_, grid)) = &open_menu {
        menu_panel(f, grid, panel);
    }
    // **도는 것이 화면에 남았는지는 다 그린 버퍼에서 읽는다**(moai-5jh6). 목록은 스크롤 창
    // 밖의 줄도 짓고, 상세는 굴린 위쪽 줄도 짓고, 폼은 둘을 통째로 덮는다 — 짓는 쪽에서 세면
    // 그 셋을 따로 따져야 하고, 하나를 빠뜨리면 보이는 스피너가 멈추거나 안 보이는 스피너로
    // 깬다. 버퍼에 스피너 글자가 있으면 그것은 보이는 것이다. 틀리는 쪽은 제목·본문에
    // 스피너 글자를 적은 경우 하나고, 그 손해는 오늘까지의 깨움과 같다(`SPIN_BUDGET` 로 묶인다).
    // 빛줄기는 따로 안 본다 — 같은 줄의 글리프(`App::spins`)가 늘 그 왼쪽에 서고, 메뉴는 몸통을
    // 밀어 올릴 뿐 덮지 않으며 폼은 통째로 덮어, 빛만 보이고 글리프가 가려지는 화면이 없다.
    app.spun = spinner_on(f.buffer_mut());

    // 맨 아랫줄은 하나다 — 글을 받는 중이면 프롬프트가, 아니면 키 바가 선다.
    match &app.mode {
        Mode::Browse => match &open_menu {
            Some((items, grid)) => menu_line(f, app, items, grid, keys),
            None => fkeys(f, app, &rows, keys),
        },
        // 안내 속 키 이름은 표에서 읽는다 — 키를 옮기면 안내도 따라온다.
        Mode::Grep(q, g) => prompt(f, keys, &grep_label(*g), q, app.input_error(), &grep_help(app, q)),
        Mode::Filter(q) => prompt(f, keys, "거름망", q, app.input_error(), &prompt_help("걸기")),
        Mode::Ask(ask) => {
            // 글칸이 **아래**에서 자리를 먼저 얻는다 — 창이 낮아 한 줄만 남으면 적는 칸이
            // 안내에 가려 어디에 치는지 안 보인다.
            let [why, line] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(keys);
            let text = clip(&ask_why(&ask.why), why.width as usize);
            f.render_widget(Paragraph::new(Line::from(Span::styled(text, Style::new().fg(Color::Black).bg(Color::LightYellow)))), why);
            prompt(f, line, "누구", &ask.input, ask.error.clone(), &format!("이름 (메일)  {}", prompt_help("쓰기")));
        }
        Mode::Idea(form) => jot_keys(f, form, keys),
        Mode::Pick(p) => match &p.typing {
            Some(input) => {
                let help = format!("{} 가기  {} 그만", label(PATH, Goto::Go), label(PATH, Goto::Cancel));
                prompt(f, keys, "경로", input, p.error.clone(), &help)
            }
            None => pick_keys(f, p, keys),
        },
        Mode::Unregister(u) => {
            let ask = Style::new().fg(Color::Black).bg(Color::LightYellow);
            // **이름은 반까지만 받는다.** 이름은 겹치면 위 조각이 붙어 자라는 파생값이고
            // (`user_config::names`) 한글은 두 칸을 먹는다 — 그대로 두면 `fit` 이 뒤에서부터
            // 자를 때 답하는 키(`y 뺀다`)가 먼저 밀려나, 무엇으로 답하는지 없는 물음이 선다.
            let room = keys.width as usize;
            let name = clip(&crate::text::one_line(&u.name), (room / 2).max(1));
            let line = Line::from(Span::styled(
                format!(" {name} 을 목록에서 뺄까 — {} 뺀다 · 다른 키는 그만 · 디렉터리와 .moai 는 그대로다 ", label(CONFIRM, Confirm::Yes)),
                ask,
            ));
            f.render_widget(Paragraph::new(fit(line, keys.width as usize)), keys);
        }
    }
}

/// 그린 화면에 도는 글리프가 한 칸이라도 있는가. 글자는 `style::SPIN` 에서 읽는다 —
/// 도는 칸이 쓰는 글자와 찾는 글자가 따로 적히면 한쪽만 바뀐다.
fn spinner_on(buf: &ratatui::buffer::Buffer) -> bool {
    buf.content.iter().any(|c| style::SPIN.contains(&c.symbol()))
}

/// 디렉터리 고르기 창(moai-plvy). 목록·상세 자리를 **폼처럼 통째로** 덮는다 — 뒤 칸의
/// 테두리와 커서가 비치면 어느 `>` 가 이 창의 것인지 안 읽힌다([`jot`] 와 같은 까닭).
///
/// 창의 테두리가 곧 지금 디렉터리를 댄다. 줄마다 `.moai` 와 `✓ 등록됨` 을 **낱말로** 붙인다 —
/// 색이 혼자 뜻을 지지 않는다. 감춘 점 디렉터리와 상한에 잘린 수는 아래 테두리 왼쪽에
/// 말한다: 조용히 안 보이면 거기 없는 줄 안다. 굴릴 것이 남았다는 표시는 목록과 같은 자리(오른쪽)다.
fn pick(f: &mut Frame, p: &mut Picker, at: Rect) {
    f.render_widget(Clear, at);
    let inner = at.width.saturating_sub(2) as usize;
    let rows = p.rows();
    let items: Vec<ListItem> = rows.iter().map(|r| ListItem::new(dent_line(p, *r, inner))).collect();
    let dir = crate::text::one_line(&p.at.dir.display().to_string());
    // 경로는 **뒤가 값지다** — 깊이 들어갈수록 앞은 늘 같은 홈이다. 넘치면 앞을 자른다.
    let room = inner.saturating_sub(crate::text::width(" 프로젝트 등록 ·  ") + 1);
    let title = format!(" 프로젝트 등록 · {} ", crate::text::clip_front(&dir, room));
    let mut foot: Vec<String> = Vec::new();
    if p.at.hidden > 0 {
        foot.push(format!("숨은 것 {}개 · {} 로 보인다", p.at.hidden, label(PICK, Pick::Hidden)));
    }
    if p.at.cut > 0 {
        foot.push(format!("그 밖 {}개 — {} 로 경로를 적는다", p.at.cut, label(PICK, Pick::Path)));
    }
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(from_anstyle(style::FOCUS))
        .title(title);
    if !foot.is_empty() {
        // 오른쪽의 굴림 표시(`↑ N줄 · ↓ N줄`)와 겹치지 않게 그 몫을 남긴다.
        block = block.title_bottom(Line::from(Span::styled(clip(&format!(" {} ", foot.join(" · ")), inner.saturating_sub(20)), dim())));
    }
    let selected = (!rows.is_empty()).then_some(p.cursor.min(rows.len().saturating_sub(1)));
    p.list.fit(at.height.saturating_sub(2) as usize, rows.len());
    if let Some(at) = selected {
        p.list.reveal(at);
    }
    let mut state = ListState::default().with_offset(p.list.offset()).with_selected(selected);
    f.render_stateful_widget(
        List::new(items).block(block).highlight_style(Style::new().add_modifier(Modifier::REVERSED)).highlight_symbol(CURSOR),
        at,
        &mut state,
    );
    scroll_mark(f, &p.list, at, "", true);
}

/// 창의 한 줄: `apps/  .moai  ✓ 등록됨`. `./` 은 지금 디렉터리, `..` 은 위로.
fn dent_line<'a>(p: &Picker, r: picker::Row, budget: usize) -> Line<'a> {
    let room = budget.saturating_sub(crate::text::width(CURSOR));
    let (mut spans, moai, registered) = match r {
        picker::Row::Here => (
            vec![Span::styled("./", bold()), Span::styled("  이 디렉터리", dim())],
            p.at.moai,
            p.at.registered,
        ),
        picker::Row::Up => return Line::from(vec![Span::styled("..", dim()), Span::styled("  위로", dim())]),
        picker::Row::Dir(i) => {
            let Some(d) = p.at.entries.get(i) else { return Line::from("") };
            // 이름이 줄을 다 먹으면 표시가 안 보인다 — 반까지만, `/` 는 자른 뒤에 붙인다.
            let mut name = clip(&crate::text::one_line(&d.name), (room / 2).max(2).saturating_sub(1));
            name.push('/');
            (vec![Span::raw(name)], d.moai, d.registered)
        }
    };
    if moai {
        spans.push(Span::styled("  .moai", Style::new().fg(Color::Cyan)));
    }
    if registered {
        spans.push(Span::styled("  ✓ 등록됨", status("done")));
    }
    fit(Line::from(spans), room)
}

/// 창이 열린 동안의 맨 아랫줄. 못 한 까닭이 있으면 그것이 줄을 차지한다 — 폼과 같다.
/// 등록과 닫기는 **늘 남는다**: 창을 연 까닭과 나갈 길이다.
fn pick_keys(f: &mut Frame, p: &Picker, at: Rect) {
    if let Some(e) = &p.error {
        let line = Line::from(vec![
            Span::styled(" ! ", Style::new().fg(Color::Black).bg(Color::LightRed)),
            Span::styled(format!(" {e}"), Style::new().fg(Color::LightRed)),
        ]);
        return f.render_widget(Paragraph::new(fit(line, at.width as usize)), at);
    }
    // `g` 가 기다리는 동안은 무엇을 기다리는지 댄다 — 탐색의 바와 같은 모양이다.
    if !p.chord.held().is_empty() {
        let next = keys::next_keys(PICK, p.chord.held())
            .into_iter()
            .map(|(k, a)| (k, if let Pick::Step(m) = a { keys::move_word(m) } else { a.what(p.show_hidden) }))
            .collect();
        return bar(f, at, Vec::new(), vec![waiting(p.chord.held(), next)]);
    }
    // 이름과 낱말은 키 표([`PICK`])에서 읽는다. 여기서 정하는 것은 차례뿐이다.
    let hint = |a: Pick| key(&label(PICK, a), a.what(p.show_hidden));
    let optional = vec![hint(Pick::Hidden), hint(Pick::Path), hint(Pick::Up), hint(Pick::Enter)];
    bar(f, at, optional, vec![hint(Pick::Register), hint(Pick::Close)]);
}

/// 생각 담기 폼. 제목 칸(세 줄) 밑에 본문 칸이 남은 높이를 다 먹는다.
///
/// **키를 먹는 칸은 굵은 선이다** — 목록·상세의 포커스와 같은 모양([`frame`])이라 한
/// 화면에 규칙이 하나다. 폼이 열려 있는 동안 목록·상세는 굵은 선을 내려놓으므로 화면에
/// 굵은 칸은 늘 하나다. 칸 이름은 테두리에 적는다: 색이 없어도 모양과 이름이 남는다.
///
/// 커서는 터미널 커서가 키를 먹는 칸의 글 안 제자리에 선다 — [`prompt`] 와 같은 까닭이다.
/// `active` 가 거짓이면(다른 칸이 키를 먹는 중이면) 굵은 선도 커서도 없다.
fn jot(f: &mut Frame, form: &mut Form, at: Rect, active: bool, tint: Style) {
    f.render_widget(Clear, at);
    // 머리 줄이 따로 설 자리가 없으면(머리 1 + 제목 3 + 본문 칸 3 이 안 들면) 담을 곳을 제목
    // 칸 테두리로 접는다 — 낮은 창에서 본문 칸이 테두리만 남는 것보다 낫고, 어느 프로젝트에
    // 담기는지는 창이 낮아도 빠지면 안 된다.
    let roomy = at.height >= JOT_HEAD_ROOM;
    let head_h = u16::from(form.into.is_some() && roomy);
    let [head_at, title_at, body_at] =
        Layout::vertical([Constraint::Length(head_h), Constraint::Length(3), Constraint::Min(0)]).areas(at);
    let title_name = match &form.into {
        Some(into) if roomy => {
            f.render_widget(Paragraph::new(jot_head(into, head_at.width as usize, tint)), head_at);
            Line::from(" 생각 담기 · 제목 ")
        }
        // 테두리에 접을 때도 **이름은 반까지만** 받는다(`jot_head` 와 같은 자). 안 자르면
        // 테두리가 뒤에서부터 잘려 ` · 제목 ` 이 먼저 빠지고, 이 칸이 제목 칸이라는 말이 사라진다.
        Some(into) => {
            let room = (title_at.width as usize).saturating_sub(2);
            let label = crate::text::width(" 담을 곳  · 제목 ");
            let name = clip(&crate::text::one_line(&into.name), room.saturating_sub(label).min(room / 2).max(1));
            Line::from(vec![Span::raw(" 담을 곳 "), Span::styled(name, tint), Span::raw(" · 제목 ")])
        }
        None => Line::from(" 생각 담기 · 제목 "),
    };

    let field = |which: Field, name: Line<'static>| {
        let block = Block::default().borders(Borders::ALL).title(name);
        if active && form.field == which {
            block.border_type(BorderType::Thick).border_style(from_anstyle(style::FOCUS))
        } else {
            block
        }
    };

    let title_block = field(Field::Title, title_name);
    let inner = title_block.inner(title_at);
    let view = form.title.view(inner.width as usize);
    let title_cursor = (inner.width > 0 && inner.height > 0)
        .then(|| (inner.x + view.cursor as u16, inner.y));
    f.render_widget(Paragraph::new(Line::from(view.text)).block(title_block), title_at);

    let body_block = field(Field::Body, Line::from(" 본문 · 여러 줄 · 없어도 된다 "));
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

/// 폼 머리 줄이 따로 서는 높이 — 머리 1 + 제목 칸 3 + 본문 칸(테두리 2 + 한 줄) 3.
const JOT_HEAD_ROOM: u16 = 7;

/// 폼 머리 — **어느 프로젝트에 담기는가**(moai-fccv). `담을 곳  이름  경로`.
///
/// 이름은 프로젝트 색을 입는다([`project_style`], 경로 줄·층과 같은 색). 색이 혼자 말하지 않게
/// 낱말(`담을 곳`)과 이름·경로가 곁에 선다. 이름은 반까지만 받는다 — 경로가 같은 이름의 두
/// 프로젝트를 가르는 것이라 이름이 줄을 다 먹으면 안 된다. 층이 없어도 선다: 위로 찾아
/// 올라간 저장소가 어디인지는 층이 없어도 헷갈린다.
fn jot_head<'a>(into: &Target, w: usize, tint: Style) -> Line<'a> {
    const LABEL: &str = " 담을 곳  ";
    let room = w.saturating_sub(crate::text::width(LABEL));
    let name = clip(&crate::text::one_line(&into.name), (room / 2).max(1));
    let path = crate::text::one_line(&into.path.display().to_string());
    let line = Line::from(vec![
        Span::styled(LABEL, bold()),
        Span::styled(name, tint),
        Span::styled(format!("  {path}"), dim()),
    ]);
    fit(line, w)
}

/// 폼이 열린 동안의 맨 아랫줄 — 담는 법·칸 옮기는 법·닫는 법. **80칸에 다 든다.**
///
/// 거절된 까닭(빈 제목)과 버릴지 묻는 말은 이 줄을 차지한다 — 키 안내는 폼을 연
/// 순간 이미 봤고, 지금 답해야 할 것은 그 말이다.
fn jot_keys(f: &mut Frame, form: &Form, at: Rect) {
    let line = if form.leaving {
        let ask = Style::new().fg(Color::Black).bg(Color::LightYellow);
        Line::from(Span::styled(format!(" 적던 것을 버릴까 — {} 버린다 · 다른 키는 폼으로 돌아간다 ", label(CONFIRM, Confirm::Yes)), ask))
    } else if let Some(e) = &form.error {
        Line::from(vec![
            Span::styled(" ! ", Style::new().fg(Color::Black).bg(Color::LightRed)),
            Span::styled(format!(" {e}"), Style::new().fg(Color::LightRed)),
        ])
    } else {
        // 이름과 낱말은 키 표([`JOT`])에서 읽는다 — `Tab`·`Enter` 의 낱말은 포커스 칸에 달렸다.
        let title = form.field == Field::Title;
        let hint = |a: Jot| key(&label(JOT, a), a.what(title));
        Line::from(vec![
            hint(Jot::Save),
            hint(Jot::Switch),
            hint(Jot::Next),
            hint(Jot::Close),
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
/// 시험이 배너 글을 본다 — 그리지 않고 [`banner`] 가 낼 글만.
#[cfg(test)]
pub(crate) fn tests_banner(app: &mut App) -> String {
    banner(app).map(|(t, _)| t).unwrap_or_default()
}

fn banner(app: &App) -> Option<(String, bool)> {
    let mut parts: Vec<String> = Vec::new();
    let mut urgent = false;
    if let Some(t) = &app.trouble {
        // 무엇을 못 했는지는 단 쪽이 적는다 — 다시 읽기와 쓰기가 같은 자리를 쓴다.
        parts.push(t.clone());
        urgent = true;
    }
    // 쓰기의 알림은 **실패 바로 뒤, 붙박이들 앞이다.** 다음 키에 사라지는 말이라 뒤에
    // 서면 80칸에서 "드러난 것 N건" 에 밀려 잘리고, 그러면 담긴 것을 확인할 길이 없다.
    // 실패보다 앞서지 않는다 — 둘이 함께 서는 것은 담긴 뒤 다시 읽기가 실패했을 때고,
    // 그때 사람이 할 일은 실패 쪽에 있다. 알림만으로는 급하지 않다(`✓` 가 뜻을 진다).
    if let Some(n) = &app.notice {
        parts.push(n.clone());
    }
    // 도는 채로 놓은 다시 읽기는 **붙박이다** — 그것이 터지면 화면이 걷히고 탐색기는
    // 모른다. 사람이 할 수 있는 것은 나갔다 다시 여는 것뿐이다(moai-j9on). 급하지만 알림
    // **뒤에** 선다: 세션 내내 남는 긴 말이 앞에 서면 80칸에서 이후 모든 쓰기의 알림을 밀어낸다.
    if app.let_go > 0 {
        parts.push(format!("멈춘 다시 읽기 {}개를 놓았다 — 그것이 터지면 화면이 걷힌다 · 나갔다 다시 연다", app.let_go));
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
    // 사용자 설정의 문제는 **층에서만** 말한다 — 등록 목록의 일이라 프로젝트 안의 화면과는
    // 상관이 없고, 급하지도 않다(읽을 수 있는 항목은 그대로 섰다).
    if app.on_layer()
        && let Some(l) = &app.layer
    {
        parts.extend(l.problems.iter().map(|p| crate::text::one_line(p)));
    }
    // 층이 **안 선** 까닭은 프로젝트 안에서도 댄다 — 그 화면에서는 층이 없다는 것 말고
    // 달리 알 길이 없다. 급하지 않다(이 프로젝트는 멀쩡하다).
    if app.layer.is_none()
        && let Some(u) = &app.unlayered
    {
        parts.push(u.clone());
    }
    // 알림 하나뿐이면 `!` 를 안 붙인다 — 담긴 것을 경보처럼 말하면 담을 때마다 무언가
    // 잘못된 줄 안다.
    let lead = if parts.len() == 1 && app.notice.is_some() { "" } else { "! " };
    (!parts.is_empty()).then(|| (format!(" {lead}{} ", parts.join("   ·   ")), urgent))
}

fn crumbs(f: &mut Frame, app: &App, rows: &[Row], at: Rect) {
    let w = at.width as usize;
    // **걸린 거름망은 늘 보인다.** 안 보이면 왜 줄이 적은지 알 길이 없고,
    // 그러면 사람이 도구를 의심하는 대신 자료를 의심한다. 그래서 **뱃지 자리를
    // 먼저 뗀다** — 경로를 줄 폭 전체로 자르면 깊이 들어갔을 때 뱃지가 줄
    // 밖으로 밀려 통째로 사라지고, 하필 그때가 목록이 가장 짧아 보이는 때다.
    let badge = app.filter_text.as_ref().map(|t| clip(&format!("[{t}]  {} 로 푼다", label(BROWSE, Browse::ClearFilter)), w));
    let room = match &badge {
        Some(b) => w.saturating_sub(crate::text::width(b) + 3),
        None => w,
    };
    // **옆 워크트리가 있어 겹쳐 보는 중이면 늘 보인다** — 거름망 뱃지와 같은 까닭이다. 옆에서
    // 온 줄에만 `⎇` 가 붙으므로, 옆이 조용하면 켜진 화면과 꺼진 화면이 똑같이 보인다.
    // **옆이 없으면 안 세운다**(moai-d5vn). 겹쳐 보기는 켜진 채로 시작하므로(moai-zcuh) "옆
    // 워크트리 없음" 을 세우면 모든 프로젝트의 경로 줄 절반을 늘 먹는다 — 옆이 없으면 켜진 화면과
    // 꺼진 화면이 정말로 같아 가를 것이 없다. 켜짐은 메뉴의 `[켜짐]` 이 댄다. 못 찾은 까닭은
    // `SPC t w` 로 켰을 때 알림이 댄다(`App::unfound`).
    // **끄는 법은 끌 수 있는 자리에서만 댄다** — 층에서는 `SPC t w` 가 메뉴에 안 서고 말없이
    // 꺼져 있으므로(`Browse::enabled`), 적어 두면 눌러도 아무 일이 없는 키가 된다. 키 이름은
    // 표에서 읽는다 — `w` 가 `SPC t w` 로 옮겨 간 뒤에도 옛 이름을 대던 자리다.
    let trees = app.origin.labels();
    let overlay = (app.worktree && !trees.is_empty()).then(|| {
        let names = trees.join(", ");
        let off = match Browse::Worktree.enabled(&app.key_ctx(rows)) {
            Ok(()) => format!("  {} 로 끈다", label(BROWSE, Browse::Worktree)),
            Err(_) => String::new(),
        };
        clip(&format!("{} {names}{off}", style::BRANCH_GLYPH), w / 2)
    });
    let room = match &overlay {
        Some(o) => room.saturating_sub(crate::text::width(o) + 3),
        None => room,
    };
    // **보기가 숨긴 것을 댄다**(moai-fmv5) — done 을 숨긴 채 시작하므로, 안 대면 끝난 일이 사라진
    // 줄 안다. 거름망 뱃지와 달리 **늘 서 있는 것**이라 경로의 몫을 굶기지 않는다: 경로에 여덟 칸이
    // 안 남으면 뺀다. 키는 안 적는다 — 메뉴의 `SPC s` 가 댄다. 층에서는 보기가 뜻이 없다.
    // 기본이 아닌 차례도 같은 뱃지에 댄다(moai-55cp) — 차례가 바뀐 줄 모르면 줄이 뒤섞인 줄 안다.
    let sorted = (app.order != Default::default()).then(|| {
        format!("정렬 {}{}", app.order.0.word(), if app.order.1 { " 거꾸로" } else { "" })
    });
    let parts: Vec<String> = [app.view.badge(), sorted].into_iter().flatten().collect();
    let look = (!parts.is_empty() && !app.on_layer()).then(|| format!("[{}]", parts.join(" · ")));
    let look = look.filter(|l| crate::text::width(l) + 3 + 8 <= room);
    let room = match &look {
        Some(l) => room.saturating_sub(crate::text::width(l) + 3),
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
    // **어느 프로젝트인지 늘 앞에 선다**(층이 있을 때). 경로는 뒤에서부터 잘리므로 이름이
    // 앞이면 깊이 들어가도 남는다 — 대신 이름이 줄을 다 먹지 않게 반까지만 준다.
    let mut spans = Vec::new();
    let mut room = room;
    if let Some(p) = app.project() {
        let name = clip(&crate::text::one_line(&p.name), (room / 2).max(1));
        room = room.saturating_sub(crate::text::width(&name) + 1);
        spans.push(Span::styled(name, project_style(p)));
        spans.push(Span::styled(":", bold()));
    }
    spans.push(Span::styled(clip(&app.crumbs(), room), bold()));
    if let Some(t) = read_at {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(t, dim()));
    }
    if let Some(o) = overlay {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(o, branch()));
    }
    if let Some(l) = look {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(l, dim()));
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
/// 글이 칸보다 길면 조각이 커서를 따라 밀린다. **오류는 그 전에 제 몫을 먼저
/// 받는다**([`prompt_room`]) — 긴 거름망일수록 오타가 잦은데, 폭을 글칸에 다 주면
/// 바로 그때 오류가 줄 밖으로 밀려 안 보였다(moai-1vjh). 오류를 윗줄로 올리지
/// 않는 까닭은 그 줄이 배너 자리라서다: 쓰기의 알림과 실패가 차례를 정해 서
/// 있고(moai-064q), 치는 동안 매 키에 나타났다 사라지는 말이 거기 끼면 그 차례가
/// 흔들린다. 무엇이 틀렸는지는 친 글 바로 옆에 있어야 읽힌다.
///
/// 안내(`help`)는 몫을 안 받는다 — Enter·Esc 는 한 번 읽으면 끝이고, 긴 글을
/// 치는 사람에게는 글이 더 값지다. 그래서 오류가 서고 걷힐 때 긴 글의 조각이
/// 그 몫만큼 밀렸다 돌아온다. 짧은 글에서는 아무것도 안 움직인다.
fn prompt(f: &mut Frame, at: Rect, what: &str, input: &Input, error: Option<String>, help: &str) {
    let label = format!(" {what} ");
    // 이름표와 그 뒤 빈칸 하나를 뗀 자리가 글칸이다.
    let lead = crate::text::width(&label) + 1;
    let avail = (at.width as usize).saturating_sub(lead);
    // 여러 줄짜리 도움말은 첫 줄만 — 한 줄 자리다.
    let error = error.map(|e| e.lines().next().unwrap_or_default().to_string());
    let (field, error) = match error {
        Some(e) => {
            let (field, room) = prompt_room(avail, crate::text::width(&e));
            (field, Some(clip(&e, room)))
        }
        None => (avail, None),
    };
    let view = input.view(field);
    let mut spans = vec![
        Span::styled(label, Style::new().fg(Color::Black).bg(Color::LightBlue)),
        Span::raw(" "),
        Span::raw(view.text),
        // 커서가 끝에 서면 조각 뒤 한 칸이 커서 자리다. 비워 두지 않으면 안내가
        // 커서 밑으로 붙는다.
        Span::raw(" "),
        Span::raw("   "),
    ];
    match error {
        Some(e) => spans.push(Span::styled(e, Style::new().fg(Color::LightRed))),
        None => spans.push(Span::styled(help.to_string(), dim())),
    }
    f.render_widget(Paragraph::new(Line::from(spans)), at);
    // 칸이 좁아 글칸이 0 이면 커서를 세우지 않는다 — 이름표 위에 서면 어디에
    // 적히는지 거짓말을 한다.
    if (at.width as usize) > lead {
        f.set_cursor_position((at.x + (lead + view.cursor) as u16, at.y));
    }
}

/// 글칸 뒤에 조각 밖으로 붙는 칸 — 커서가 글 가운데 서도 조각 뒤에 붙는 빈칸
/// 하나와, 오류 앞의 빈칸 셋.
const PROMPT_GAP: usize = 1 + 3;

/// 이름표 뒤 `avail` 칸을 글칸과 오류(`error` 칸)에 나눈다. `(글칸, 오류가 쓸 칸)`.
///
/// **오류가 먼저 받되, 글칸의 삼분의 일은 남긴다.** 다 주면 좁은 창에서 글칸이
/// 0 이 되어 무엇을 치는지도 커서가 어디 섰는지도 안 보인다 — 틀렸다는 말만
/// 있고 고칠 자리가 없다. 그래서 모자라면 오류를 `…` 로 자른다. 오류는 앞에서
/// 무엇이 틀렸는지를 말하고(`` `xyz` 라는 칸이 없다 ``) 뒤에서 있는 것을 늘어놓으므로,
/// 잘려도 앞이 남는 쪽이 잃는 것이 적다. 삼분의 일은 창을 따라 자란다 — 칸 수를
/// 박아 두면 넓은 창에서는 괜히 좁고 좁은 창에서는 오류가 통째로 사라진다.
///
/// 커서는 늘 글칸 안에 선다: 조각이 커서를 따라 밀리고([`Input::view`]), 글칸은
/// `avail` 을 안 넘는다.
///
/// 오류가 한 칸도 못 받으면 글칸을 줄이지 않는다 — 줄여 봐야 오류는 안 보이고
/// 친 글만 가려진다.
fn prompt_room(avail: usize, error: usize) -> (usize, usize) {
    let field = avail.saturating_sub(PROMPT_GAP + error).max(avail / 3);
    match avail.saturating_sub(field + PROMPT_GAP) {
        0 => (avail, 0),
        room => (field, room),
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
            Row::Up | Row::Project(_) => None,
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
            (n > 0).then(|| format!("{} {st} {n}", count_glyph(app, &work, st)))
        })
        .collect();
    // **줄이 있으면 "비었다" 라고 하지 않는다.** 셈은 config 에 있는 칸의 일만
    // 세므로, 묶음만 있는 디렉터리·바구니만 있는 디렉터리·config 에 없는 칸에
    // 선 줄에서는 비어 있고, 그때 제목이 목록과 정면으로 어긋난다.
    // `..` 은 줄로 안 센다 — 층이 있으면 프로젝트 뿌리에도 서므로, 세면 빈 프로젝트가
    // " 1줄 " 로 서고 "비었다" 에 영영 못 닿는다.
    let lines = rows.iter().filter(|r| !matches!(r, Row::Up)).count();
    let title = match (counts.is_empty(), lines == 0) {
        // 층의 줄은 일이 아니라 프로젝트다 — 칸 셈은 줄마다 곁에 선다.
        _ if app.on_layer() => format!(" 프로젝트 {}곳 ", rows.len()),
        (_, true) => " 비었다 ".to_string(),
        (true, false) => format!(" {lines}줄 "),
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
///
/// 에픽·마일스톤 줄은 **끝난/일 셈을 테두리 끝에 오른쪽 정렬로** 붙인다 — 포커스 없이
/// 여러 묶음의 진척을 한 줄로 훑어 내려간다. 셀 일이 없으면 셈도 없다.
///
/// **진행 바탕은 두 번 깔아 보고 걷었다**(moai-u3r2). 반전도 막대 색 초록도 줄마다 덩어리가
/// 서서 목록이 정신없었다 — 사용자 판단. 셈 글자만으로 한눈에 읽힌다.
fn row_line<'a>(app: &App, r: &Row, budget: usize) -> Line<'a> {
    let e = match r {
        Row::Item(e) => e,
        Row::Up => return Line::from(Span::styled("..", dim())),
        Row::Project(at) => return place_line(app, *at, budget),
    };
    let is_dir = matches!(e, Entry::Dir { .. });
    let Some(at) = e.at() else {
        // 바구니는 제 줄이 없다 — 이름만 낸다.
        return Line::from(Span::styled(format!("{}/", app.index.label(&app.issues, e)), dim()));
    };
    let i = &app.issues[at];
    // 걸린 검색이 이 자리를 보면 찾은 글자를 칠한다(moai-yio7).
    let grep = app.grep_query();
    let in_id = grep.filter(|(g, _)| g.sees_id()).map(|(_, q)| q);
    let in_title = grep.filter(|(g, _)| g.sees_title()).map(|(_, q)| q);

    // 앞에 붙는 것들을 **먼저 만들고 재서** 남는 만큼을 제목에 준다. 손으로
    // 더한 숫자로 어림하면 `p10` 처럼 자리를 더 먹는 값이나 한글이 든 id 에서
    // 어긋나고, 넘친 줄은 위젯이 말없이 잘라 내 **잘렸다는 `…` 마저** 사라진다.
    let head = vec![
        Span::styled(i.id.clone(), dim()),
        Span::raw("  "),
        Span::styled(format!("p{}", i.priority()), priority(i.priority())),
        Span::raw(" "),
        // 칸은 글리프로도 말한다. 색이 없는 터미널에서도 뜻이 남아야 한다.
        // 묶음은 멤버에서 읽은 칸이다 — CLI 목록의 S 열과 같은 자. **다만 집은 멤버가
        // 있을 때만 돌린다**: 도는 글리프는 "지금 누가 손대고 있다" 는 말인데 묶음의
        // `in_progress` 는 멤버 하나가 끝났다는 말일 수도 있다(`App::spins`).
        Span::styled(row_glyph(app, at), status(app.column(at))),
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
    let mut head_w = head.iter().map(|s| crate::text::width(&s.content)).sum::<usize>();
    // 셈은 **제목보다 먼저 자리를 얻는다** — 긴 제목에 밀려 잘리면 진척을 말하는 것이
    // 사라진다. 자는 상세 롤업과 같다(`Index::progress`).
    let tally = match is_dir.then(|| app.index.progress(&app.issues, &deeper(app, e))) {
        Some(p) if p.percent().is_some() => format!("  {}/{}", p.done, p.work.len()),
        _ => String::new(),
    };
    // **좁으면 스피너부터 걷는다**(moai-q59j). 목록 줄의 글리프는 두 칸(`⠋▸`·` ·`)인데, 제목
    // 한 글자와 디렉터리 `/` 가 들어갈 자리가 없으면 멈춘 글리프 한 칸만 남긴다 — 뜻은 글리프가
    // 지고 움직임은 곁들이라, 잘려도 되는 것부터 뺀다. 안 빼면 `/` 가 잘려 폴더와 파일이 안 갈린다.
    if crate::text::width(CURSOR) + head_w + crate::text::width(&tally) + 2 > budget {
        let still = style::glyph(app.column(at)).to_string();
        head_w = head_w - crate::text::width(&head[4].content) + crate::text::width(&still);
        head[4] = Span::styled(still, status(app.column(at)));
    }
    let used = crate::text::width(CURSOR) + head_w + crate::text::width(&tally);
    let mut title = clip(&app.index.label(&app.issues, e), budget.saturating_sub(used));
    // **디렉터리 표시는 자른 뒤에 붙인다.** 먼저 붙이면 긴 제목에서 `/` 가
    // 제일 먼저 잘려 나가고, 목록에는 디렉터리라고 말하는 것이 달리 없다.
    if is_dir {
        if crate::text::width(&title) + 1 > budget.saturating_sub(used) {
            title = clip(&title, budget.saturating_sub(used + 1));
        }
        title.push('/');
    }

    let title_w = crate::text::width(&title);
    let mut spans = head;
    // id 는 칠하면 조각이 갈라진다 — 머리글의 자리(`head[4]`)를 다 쓴 **뒤에** 편다.
    let id = spans.remove(0);
    spans.splice(0..0, mark(vec![id], in_id));
    // **도는 줄의 제목에는 빛줄기가 흐른다**(moai-fy99). 자는 글리프와 같은 `App::spins` 하나다 —
    // 일은 집었을 때, 에픽·마일스톤은 그 밑에 집은 일이 실제로 있을 때(moai-x5eg), 미룬 것은 안
    // 돈다(moai-tawj). 묶음을 따로 빼 두면 목록 뿌리에서 무엇이 움직이는지 글리프 한 칸으로만
    // 읽혀, 여러 에픽을 훑어 내릴 때 눈에 안 걸린다(moai-eomg). 뜻은 여전히 글리프가 진다.
    if app.spins(at) {
        spans.extend(mark(shimmer(title, app.spin), in_title));
    } else {
        spans.extend(mark(vec![Span::raw(title)], in_title));
    }
    if !tally.is_empty() {
        // 셈은 **테두리 끝에 오른쪽 정렬**한다(moai-1krv) — 줄마다 제목 길이를 따라 들쭉날쭉하면
        // 여러 에픽의 셈을 한 줄로 훑어 내려갈 수 없다. 제목과의 틈은 채움 칸이 진다.
        let room = budget.saturating_sub(crate::text::width(CURSOR));
        let gap = room.saturating_sub(head_w + title_w + crate::text::width(&tally));
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(tally, dim()));
    }
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
        // **빈 층은 할 일을 댄다**(moai-r8kl). 등록이 0 인 채 `.moai` 밖에서 띄운 자리다 — "없다"
        // 만 서면 밖에서 부른 실수가 멀쩡한 빈 목록으로 읽힌다.
        None if app.on_layer() => {
            let mut out = vec![Line::from(Span::styled("등록한 프로젝트가 없다", bold())), Line::from("")];
            let how = format!("{} 로 디렉터리를 골라 등록한다", label(BROWSE, Browse::Pick));
            out.extend(wrapped(&how, inner.width as usize, dim()));
            out
        }
        None => vec![Line::from(Span::styled("없다", dim()))],
        // 프로젝트 뿌리의 `..` 은 층으로 간다 — 어디로 가는지 말한다.
        Some(Row::Up) if app.path.is_empty() => vec![Line::from(Span::styled("프로젝트 층으로", dim()))],
        Some(Row::Up) => vec![Line::from(Span::styled("한 층 위로", dim()))],
        Some(Row::Project(at)) => place_about(app, at, inner.width as usize),
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
    // **굴리는 키를 표시 곁에서 말한다.** 아래 키 바는 좁으면 뒤에서부터 키를
    // 떨어뜨리는데, 80칸이면 떨어지는 것이 하필 `j·k` 다. 알림은 그것이
    // 가리키는 것 곁에 둔다. **지금 자리에서 듣는 키를 댄다** — `j·k` 는 포커스 칸을
    // 움직이므로(moai-ob4c) 목록에 선 사람에게 `j·k` 라 적으면 누른 대로 커서가 옮겨 가고
    // 보던 상세가 바뀐다. 목록에서는 상세로 가는 키를 댄다.
    let hint = if app.focus == Pane::Detail {
        labels(BROWSE, &[Browse::Step(Move::LineDown), Browse::Step(Move::LineUp)])
    } else {
        label(BROWSE, Browse::FocusNext)
    };
    let hint = format!(" ({hint})");
    scroll_mark(f, &app.detail, at, &hint, app.focus == Pane::Detail);
    // **넘친 줄은 잘렸다고 말한다.** `Wrap` 을 끈 뒤로 폭을 넘는 줄은 위젯이
    // 표시도 없이 잘라 낸다 — 태그 줄, 롤업의 칸별 건수, `SPC t r` 원문, 접지
    // 않기로 한 코드 줄이 그 길로 조용히 꼬리를 잃었다. 만드는 쪽마다 따로
    // 자르면 또 하나를 빠뜨리므로 **나가는 마지막 자리에서 한 번** 자른다.
    let lines: Vec<Line> = lines.into_iter().map(|l| fit(l, inner.width as usize)).collect();
    // 줄 수가 u16 을 넘는 본문이면 거기서 멈춘다 — 넘겨 접으면 첫 줄로 튄다.
    let top = u16::try_from(app.detail.offset()).unwrap_or(u16::MAX);
    f.render_widget(Paragraph::new(lines).scroll((top, 0)), inner);
}

/// 빛줄기가 지나간 뒤 다시 들어오기까지 쉬는 칸 수. 쉬지 않으면 띠가 끝에 닿자마자 앞에서
/// 다시 떠서 흐르는 것이 아니라 깜빡이는 것으로 보인다.
const GLINT_REST: usize = 8;

/// 빛줄기의 띠 — **가장자리는 보통 노랑, 가운데는 밝은 노랑 굵게.** 16색 안에서 짓는
/// 그라데이션이다(`style` 의 기본 16색 규칙, 사용자 선택 moai-fy99). 노랑은 집은 칸의 색
/// (`style::IN_PROGRESS`)이라 빛이 칸의 뜻과 어긋나지 않는다. 흰색은 밝은 바탕에서 사라져 안 쓴다.
fn glint() -> [Style; 3] {
    let edge = Style::new().fg(Color::Yellow);
    [edge, from_anstyle(style::IN_PROGRESS), edge]
}

/// 도는 줄(집은 일, 집은 일을 품은 묶음)의 제목을 **빛줄기가 왼쪽에서 오른쪽으로 흐르는** 조각들로 낸다. 걸음은 도는
/// 글리프와 같은 `App::spin` 이다 — 따로 시계를 두면 둘이 다른 박자로 움직이고, 도는
/// 것이 안 보일 때 루프가 안 깨우는 규칙(`App::spun`)도 그대로 따른다.
///
/// **뜻은 글리프가 진다.** 빛줄기는 곁들임이라 색이 없는 화면에서 사라져도 잃는 것이 없다.
/// 글자와 폭은 그대로다 — 스타일만 칸마다 바꾼다. 두 칸 글자는 **덮는 칸 중 가장 밝은 띠**를
/// 받는다: 시작 칸으로만 재면 가운데가 뒤 칸에 서는 걸음마다 굵은 칸이 사라져, 한글 제목에서
/// 빛이 흐르지 않고 한 걸음 걸러 깜빡인다.
fn shimmer(title: String, frame: usize) -> Vec<Span<'static>> {
    let band = glint();
    let cycle = crate::text::width(&title) + band.len() + GLINT_REST;
    // 띠의 앞머리(오른쪽 끝 바로 뒤)가 서는 칸. 0 에서 들어와 제목 끝을 지나 쉰다.
    let head = frame % cycle;
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;
    let mut col = 0usize;
    for c in title.chars() {
        let w = crate::text::width(c.encode_utf8(&mut [0; 4]));
        // 띠 번호가 가운데(1)에 가까울수록 밝다. 덮는 칸들 중 가장 밝은 것을 고른다.
        let style = (col..col + w.max(1))
            .filter(|&x| x < head)
            .map(|x| head - 1 - x)
            .filter(|&back| back < band.len())
            .min_by_key(|&back| back.abs_diff(1))
            .map(|back| band[back]);
        if style != run_style && !run.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut run), run_style.unwrap_or_default()));
        }
        run_style = style;
        run.push(c);
        col += w;
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style.unwrap_or_default()));
    }
    spans
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

/// 칸의 이름. 키 바가 `Tab` 이 **어디로 가는지** 댄다.
pub(super) fn pane_name(p: Pane) -> &'static str {
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

/// 그 줄의 글리프. 도는 것은 지금 누가 손대고 있는 줄이다 — **돌지는 [`App::spins`]
/// 가 정한다**(루프는 그려진 글리프로 깬다 — `App::spun`).
fn glyph_of(app: &App, at: usize) -> &'static str {
    let col = app.column(at);
    if app.spins(at) { style::spin_frame(app.spin) } else { style::glyph(col) }
}

/// 목록 줄의 글리프 — **도는 줄은 스피너 뒤에 그 칸의 멈춘 글리프를 붙인다**(moai-q59j).
/// 시작한 칸이 모두 돌면서 `in_progress` 와 `review` 가 같은 스피너가 됐고, 목록 줄에는 칸
/// 이름이 없어 색만으로 갈렸다 — 색이 혼자 뜻을 지면 안 된다. 움직임은 곁들이고 뜻은
/// 글리프가 진다(`⠋▸`·`⠋?`). 안 도는 줄은 한 칸 띄워 두 글자 자리를 맞춘다 — 줄마다 제목이
/// 들쭉날쭉하면 훑어 내려갈 수 없다. 상세 머리와 건수는 칸 이름을 곁에 적으므로 [`glyph_of`] 다.
fn row_glyph(app: &App, at: usize) -> String {
    let col = style::glyph(app.column(at));
    if app.spins(at) { format!("{}{col}", style::spin_frame(app.spin)) } else { format!(" {col}") }
}

/// 칸별 건수의 글리프. **센 줄 가운데 도는 줄이 있을 때만 돈다**([`App::spins`]) — 칸
/// 이름만 보고 돌리면 미룬 `in_progress` 하나뿐인 칸이, 줄은 다 멈췄는데 건수만 돌아 계획에서
/// 뺀 일을 "지금 손대는 중" 이라 말하고 그 스피너로 루프를 깨운다(`App::spun`).
fn count_glyph(app: &App, work: &[usize], st: &str) -> &'static str {
    let turning = work.iter().any(|&at| app.issues[at].status.as_str() == st && app.spins(at));
    if turning { style::spin_frame(app.spin) } else { style::glyph(st) }
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
    let placed = app.index.milestone_of(idx);
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
    // **막는가는 `report::blocker` 가 가른다** — `moai ready` 가 고르는 그 자다. 한때
    // 여기가 그 규칙을 손으로 베껴, 없는 id 를 가리키는 막음을 `ready` 는 안 막힌
    // 것으로 고르는데 이 패널은 "막힘" 이라 그렸다(moai-af64). 재료(서 있는 칸·미룸)는
    // 적재 때 센 것을 대고, 여기는 받은 답을 낱말로 옮기기만 한다.
    for b in &i.blocked_by {
        let at = app.index.find(b);
        let root = app.index.deferred_root(b);
        let (waiting, aside) = at.map_or((crate::report::Waiting::Live, &[][..]), |at| app.waits(at));
        let (label, text) = match crate::report::blocker(at.map(|at| app.column(at)), root.is_some(), waiting) {
            Blocker::Missing => ("끊김", format!("! {b}  없는 이슈라 막지 않는다")),
            Blocker::Done => ("풀림", format!("✓ {b}  {}", app.title_of(b))),
            Blocker::Open => ("막힘", format!("· {b}  {}", app.title_of(b))),
            // **미룬 막음도 막는다** — 미룬 일은 끝난 일이 아니다. 다만 그 줄은 보드에도
            // `ready` 에도 없으므로 미뤘다는 말을 붙인다. 낱말은 상세 머리가 쓰는 자리다.
            // **제목 앞에 둔다** — 값은 오른쪽부터 잘리므로, 뒤에 붙이면 흔한 길이의
            // 제목에서 이 줄을 그냥 "막힘" 과 가르는 유일한 말이 통째로 사라진다.
            // 미뤄 뺀 멤버만 기다리는 묶음이면 묶음은 미룬 적이 없다 — 그 멤버를 댄다.
            // **첫 멤버와 남은 수만** 댄다: 값은 오른쪽부터 잘리므로 다 늘어놓으면 큰 에픽을
            // 미뤘을 때 제목이 통째로 사라진다(리뷰 moai-2sea.tns).
            Blocker::Deferred if !aside.is_empty() && root.is_none() => {
                let more = if aside.len() > 1 { format!(" 외 {}", aside.len() - 1) } else { String::new() };
                ("막힘", format!("· {b}  미룬 멤버 {}{more}  {}", aside[0], app.title_of(b)))
            }
            // 멤버가 없는 묶음 — 기다릴 일이 없어도 막는다(moai-1c2l). 채울 자리라고 댄다.
            Blocker::Empty => ("막힘", format!("· {b}  멤버 없음  {}", app.title_of(b))),
            Blocker::Deferred => {
                let shelf = at
                    .and_then(|at| crate::view::deferred_for(&app.issues[at], root, &app.now))
                    .unwrap_or_else(|| "미룸".into());
                ("막힘", format!("· {b}  {shelf}  {}", app.title_of(b)))
            }
        };
        fields.push((label.into(), text));
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
/// 세면 자리 규칙과 세는 규칙이 달라 머리글과 줄 수가 어긋난다. 목록 줄의 진행
/// 바탕도 같은 셈([`crate::nav::Index::progress`])을 쓴다.
fn rollup<'a>(app: &App, path: &crate::nav::Path, w: usize) -> Vec<Line<'a>> {
    let progress = app.index.progress(&app.issues, path);
    let Some(percent) = progress.percent() else {
        // **없는 것과 안 세는 것은 다르다.** 담아 둔 생각은 자리로는 여기
        // 걸리지만(왼쪽 목록이 그 줄을 낸다) 진행률로는 안 센다 — 세기
        // 시작하면 담을수록 그 부모가 덜 끝난 것으로 보인다. 둘을 한 낱말로
        // 뭉치면 줄이 보이는데 `자식 없음` 이라 말한다(moai-lhbh).
        let word = if progress.kids == 0 { "자식 없음" } else { "셀 일 없음" };
        return vec![Line::from(Span::styled(word, dim()))];
    };
    let (work, done) = (&progress.work, progress.done);

    // `clamp(10, 24)` 뒤에는 10 이상이라 뺄셈이 넘칠 수 없고 6 아래로도 안
    // 간다 — 지키는 척하는 `.saturating_sub`·`.max` 는 지우고 뜻만 남긴다.
    let cells = w.clamp(10, 24) - 4;
    let filled = crate::text::bar_fill(Some(percent), cells);
    let mut out = vec![Line::from(vec![
        // 16색 10번 — SPC 메뉴의 키와 같은 색(사용자 결정, moai-a46g).
        Span::styled("█".repeat(filled), Style::new().fg(Color::LightGreen)),
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
            (n > 0).then(|| Span::styled(format!("{} {st} {n}   ", count_glyph(app, &work, st)), status(st)))
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

/// 프로젝트 이름을 칠하는 **한 곳** — 경로 줄·층의 줄·층의 상세가 모두 이것을 부른다.
///
/// 색은 CLI 한눈 보기와 같은 `style::project_colour` 다(moai-xs9x). **경로로 고른다** —
/// 이름은 등록 목록을 따라 바뀌는 파생값이라, 이름으로 고르면 프로젝트 하나를 더할 때
/// 옆 프로젝트의 색이 바뀌고 두 표면이 같은 프로젝트를 다른 색으로 칠한다. 무게는 CLI
/// 머리와 같은 `HEAD` 다. 칠하는 곳에는 늘 이름이 곁에 선다 — 색이 혼자 뜻을 지지 않는다.
/// 사용자 설정에 색을 정했으면(`Place::hue`, moai-o04b) 그것이 경로 해시를 이긴다 — 그 판단도
/// `style::project_colour` 안에 있어 CLI 와 갈라지지 않는다. `SPC r` 이 설정을 다시 읽으면 따라온다.
fn project_style(place: &Place) -> Style {
    paint_project(&place.path, place.hue)
}

/// 층의 줄을 들고 있지 않은 자리(폼의 담을 곳)에서 경로로 칠한다. 정한 색은 층에서 같은
/// 경로를 찾아 쓰고, 층이 없으면 경로 해시다 — 층이 없으면 사용자 설정에 프로젝트가 없다.
fn project_style_at(app: &App, path: &std::path::Path) -> Style {
    let hue = app.layer.as_ref().and_then(|l| l.places.iter().find(|p| p.path == path)).and_then(|p| p.hue);
    paint_project(path, hue)
}

fn paint_project(path: &std::path::Path, hue: Option<style::Hue>) -> Style {
    from_anstyle(style::project_colour(path, hue).effects(style::HEAD.get_effects()))
}

/// 층의 한 줄: `이름/  ·3 ▸1 ✓12  여기  /경로`. 들어갈 수 있는 것만 `/` 가 붙는다.
///
/// **칸은 글리프와 수로** 선다 — 칸 이름까지 적으면 80칸의 왼쪽 칸에 두 칸도 안 들어간다.
/// 글리프가 뜻을 지므로 색이 혼자 말하지 않고, 이름은 오른쪽 상세가 댄다. 못 여는 것은
/// CLI 한눈 보기와 같은 말을 잘라서 낸다 — 무엇인지는 앞머리(`init 전`·`디렉터리가 없다`·
/// `못 읽는다`)에 있어 잘려도 남는다.
fn place_line<'a>(app: &App, at: usize, budget: usize) -> Line<'a> {
    let Some(p) = app.layer.as_ref().and_then(|l| l.places.get(at)) else {
        return Line::from("");
    };
    let room = budget.saturating_sub(crate::text::width(CURSOR));
    let enterable = matches!(p.look, Look::Open { .. } | Look::Unread);
    // 이름이 줄을 다 먹으면 무엇이 서 있는지가 안 보인다 — 반까지만. `/` 는 자른 뒤에 붙인다.
    let mut name = clip(&crate::text::one_line(&p.name), (room / 2).max(2).saturating_sub(1));
    if enterable {
        name.push('/');
    }
    let mut spans = vec![Span::styled(name, project_style(p)), Span::raw("  ")];
    match &p.look {
        Look::Unread => spans.push(Span::styled("읽는 중", dim())),
        Look::Open { sum, .. } => {
            let shown: Vec<Span> = sum
                .counts
                .iter()
                .filter(|(_, n)| *n > 0)
                .flat_map(|(st, n)| [Span::styled(format!("{}{n}", style::glyph(st)), status(st)), Span::raw(" ")])
                .collect();
            if shown.is_empty() {
                spans.push(Span::styled("비었다", dim()));
            } else {
                spans.extend(shown);
            }
            if sum.warnings > 0 || sum.unreadable > 0 {
                spans.push(Span::styled(" !", from_anstyle(style::WARN)));
            }
        }
        Look::Shut { state, said } => spans.push(Span::styled(said.clone(), shut_style(*state))),
    }
    if p.launched {
        spans.push(Span::styled(if p.registered { "  여기" } else { "  여기 · 등록 안 됨" }, dim()));
    }
    spans.push(Span::styled(format!("  {}", crate::text::one_line(&p.path.display().to_string())), dim()));
    fit(Line::from(spans), room)
}

/// 못 여는 프로젝트의 색 — CLI 한눈 보기(`view::unopened`)와 같은 무게다. init 전은
/// 고칠 것이 아니라 흐리게, 사라진 것은 경고, 못 읽는 것은 오류. 뜻은 말이 진다.
fn shut_style(s: Shut) -> Style {
    match s {
        Shut::Uninit => dim(),
        Shut::Missing => from_anstyle(style::WARN),
        Shut::Unreadable => from_anstyle(style::ERROR),
    }
}

/// 층의 줄에 커서가 섰을 때 — 그 프로젝트의 한눈 보기. `moai status` 가 `.moai` 밖에서
/// 내는 한 덩어리와 같은 셈이다(칸별 수·집은 것·드러난 것).
fn place_about<'a>(app: &App, at: usize, w: usize) -> Vec<Line<'a>> {
    let Some(p) = app.layer.as_ref().and_then(|l| l.places.get(at)) else {
        return vec![Line::from(Span::styled("없다", dim()))];
    };
    // 이 층의 글은 전부 **남의 것**이다 — 한 줄 자리는 `one_line` 하나를 지난다. `sanitize` 가
    // 남긴 줄바꿈·탭은 한 줄 `Line` 안에서 위젯이 말없이 버려 다른 이름으로 읽힌다(moai-9tww).
    let mut out = wrapped(&crate::text::one_line(&p.name), w, project_style(p));
    out.extend(wrapped(&crate::text::one_line(&p.path.display().to_string()), w, dim()));
    match (p.launched, p.registered) {
        (true, true) => out.push(Line::from(Span::styled("여기서 띄웠다", dim()))),
        // 고칠 명령에는 **그 뿌리를** 댄다. `.` 이라 적으면 하위 디렉터리에서 띄운 사람이 그
        // 하위 디렉터리를 등록한다 — 그곳은 `.moai` 가 없어 "init 전" 으로 선다.
        (true, false) => {
            // 명령에 넣는 철자는 `one_line` 을 안 지난다 — 탭·줄바꿈이 빈칸이 되면 다른 디렉터리다.
            let at = crate::text::shell_word(&p.path.display().to_string());
            out.extend(wrapped(&format!("여기서 띄웠다 · 등록 안 됨 — `moai project add {at}` 로 더하면 어디서든 보인다"), w, dim()))
        }
        _ => {}
    }
    out.push(Line::from(""));
    match &p.look {
        Look::Unread => out.push(Line::from(Span::styled("읽는 중", dim()))),
        Look::Shut { state, said } => out.extend(wrapped(said, w, shut_style(*state))),
        Look::Open { sum, .. } => {
            for (st, n) in &sum.counts {
                // 칸 이름은 **남의 설정 파일**에서 온다 — 옆 줄들처럼 한 줄로 거른다.
                out.push(Line::from(Span::styled(
                    format!("{} {} {n}", style::glyph(st), crate::text::one_line(st)),
                    status(st),
                )));
            }
            out.push(Line::from(""));
            out.push(Line::from(Span::styled(format!("집은 것 {}건", sum.picked.len()), bold())));
            for i in &sum.picked {
                out.push(Line::from(vec![
                    Span::styled(crate::text::one_line(&i.id), dim()),
                    Span::raw("  "),
                    Span::styled(style::glyph(&i.column).to_string(), status(&i.column)),
                    Span::raw(" "),
                    Span::raw(crate::text::one_line(&i.title)),
                ]));
            }
            out.push(Line::from(""));
            if sum.warnings == 0 {
                out.push(Line::from(vec![Span::styled("✓", status("done")), Span::raw(" 드러난 문제 없다")]));
            } else {
                out.push(Line::from(vec![
                    Span::styled("!", from_anstyle(style::WARN)),
                    Span::raw(format!(" 드러난 것 {}건 — 들어가서 `moai status`", sum.warnings)),
                ]));
            }
            if sum.unreadable > 0 {
                out.push(Line::from(vec![
                    Span::styled("!", from_anstyle(style::ERROR)),
                    Span::raw(format!(" 읽을 수 없는 줄 {}개", sum.unreadable)),
                ]));
            }
            out.push(Line::from(""));
            out.push(Line::from(Span::styled(format!("{} 로 들어간다", label(BROWSE, Browse::Enter)), dim())));
        }
    }
    out
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// 맨 아래 키 바. **아직 없는 것은 적지 않는다** — 눌러도 아무 일이
/// 없는 키를 적어 두면 그것부터 도구를 못 믿게 된다.
///
/// 메뉴가 열린 동안은 이 바 대신 접두어 줄([`menu_line`])이 선다.
fn fkeys(f: &mut Frame, app: &App, rows: &[Row], at: Rect) {
    let c = app.key_ctx(rows);
    // `g` 가 기다리는 동안은 메뉴와 같은 자리에서 **무엇을 기다리는지** 댄다(moai-k3yi). 뜻 없는
    // 키를 누르면 열이 버려져([`keys::Chord::feed`]) 바가 저절로 돌아온다.
    if !app.chord.held().is_empty() {
        let next = keys::next_keys(BROWSE, app.chord.held())
            .into_iter()
            .filter(|(_, a)| a.enabled(&c).is_ok())
            .map(|(k, a)| (k, if let Browse::Step(m) = a { keys::move_word(m) } else { a.what(&c) }))
            .collect();
        return bar(f, at, Vec::new(), vec![waiting(app.chord.held(), next)]);
    }
    let (optional, keep) = browse_hints(app, &c);
    let spans = |v: Vec<(String, &str)>| v.iter().map(|(k, what)| key(k, what)).collect();
    bar(f, at, spans(optional), spans(keep));
}

/// 바의 한 칸 — `(키 이름, 낱말)`.
type Hint = (String, &'static str);

/// 접두어를 누르고 기다리는 동안의 바 한 칸 — `g → g 맨 위 · p 경로 적기`. 이어 누를 키는 든
/// 쪽이 표([`keys::next_keys`])에서 읽어 넘긴다.
fn waiting(held: &[ratatui::crossterm::event::KeyEvent], next: Vec<Hint>) -> Span<'static> {
    let next: Vec<String> = next.iter().map(|(k, what)| format!("{k} {what}")).collect();
    key(&menu::title(held), &format!("→ {}", next.join(" · ")))
}

/// 탐색 바에 적을 것 — `(키 이름, 낱말)`. 앞 묶음은 폭이 모자라면 앞쪽부터 떨어지고, 뒤 묶음은
/// 늘 남는다([`bar`]).
///
/// **이름·낱말·켜짐은 키 표에서 읽는다**([`keys::BROWSE`]·[`keys::Browse::enabled`]). 여기서
/// 정하는 것은 **차례**뿐이다 — 무엇이 먼저 떨어지는가. 켜지지 않은 동작은 적지 않으므로
/// 키 처리와 바가 한 판정을 읽고, 둘이 갈릴 수 없다.
fn browse_hints(app: &App, c: &Ctx) -> (Vec<Hint>, Vec<Hint>) {
    use Browse as B;
    let hint = |acts: &[Browse]| -> Hint { (labels(BROWSE, acts), acts[0].what(c)) };
    let shown = |order: &[&[Browse]]| -> Vec<Hint> {
        order.iter().filter(|acts| acts[0].enabled(c).is_ok()).map(|acts| hint(acts)).collect()
    };
    // **덜 급한 것부터 떨어뜨린다.** 폭이 모자라면 앞쪽부터 버리고, 뒤 묶음(`keep`)은 늘 남는다.
    // 바로 누르는 키는 이동·포커스·`/`·드나들기뿐이고(moai-7sjm) 나머지는 `SPC 메뉴` 한 칸이
    // 댄다 — 메뉴는 그 자리에서 켜진 것만 세우므로 바와 같은 판정을 읽는다. 끝내기(`SPC q`)도
    // 메뉴에 있고 Ctrl-C 는 어디서든 끝낸다.
    // **층에서는 층에서 듣는 키만 적는다** — `/` 는 층에서 까닭만 말하고(`Browse::enabled` 가
    // 걸러 여기 안 선다) 나가기는 위가 없다.
    // **차례는 커서를 따라 안 바뀐다**(moai-k3yi). 커서가 잎이면 Enter, 뿌리면 Bksp 가 `enabled`
    // 에서 빠질 뿐 나머지 칸은 제자리다 — 80칸에서 하나도 안 떨어지므로 빠진 자리 말고는
    // 흔들리는 것이 없다(`the_key_bar_keeps_its_order_as_the_cursor_moves_at_eighty_columns`).
    // `h·l`·`Ctrl-C` 는 적지 않는다: 드나들기의 이름은 Enter·Bksp 하나고(층의 거절문도 그 이름을
    // 댄다), 끝내기는 메뉴의 `q` 가 대며 Ctrl-C 까지 늘 남기면 80칸 거름망 켠 목록에서 `j·k` 가
    // 떨어진다. 둘 다 `moai tui --help` 에 있다.
    let optional = if app.on_layer() {
        shown(&[&[B::Step(Move::LineDown), B::Step(Move::LineUp)], &[B::FocusNext], &[B::Enter]])
    } else {
        shown(&[
            &[B::Step(Move::LineDown), B::Step(Move::LineUp)],
            // **`Tab` 은 가는 곳을 댄다** — 가는 곳을 적으면 이 줄도 글자로 지금 자리를 말한다.
            &[B::FocusNext],
            &[B::Grep],
            // **드나드는 키는 목록에서만 적는다.** `Browse::enabled` 가 Enter·Bksp 를 포커스에
            // 태워 상세에서는 아무 일도 안 하므로, 거기서 적어 두면 "눌러도 아무 일이 없는 키"
            // 가 된다. 실제로 눌러 재는 것은 `the_key_bar_names_only_keys_that_act_in_the_focused_pane` 이다.
            &[B::Leave],
            &[B::Enter],
        ])
    };
    // 늘 남는 것: 걸어 둔 거름망을 푸는 길, 그리고 나머지 전부로 가는 메뉴.
    let mut keep = Vec::new();
    if app.filter_text.is_some() {
        keep.push(hint(&[B::ClearFilter]));
    }
    keep.push((menu::title(&[LEADER.event()]), menu::ROOT));
    (optional, keep)
}

/// SPC 메뉴 창(moai-7sjm, 모양은 moai-apsa 의 doom emacs which-key 식). 목록·상세 **아래 전체
/// 폭**에 서고 몸통을 밀어 올린다. 칸은 `키 : 낱말 [상태]` — 격자는 [`menu::grid`] 가 놓았고 여기는
/// 칠하기만 한다. 키는 굵게, 키·`:`·실행 낱말·`+묶음` 이 제 색([`MENU_KEY`]…)을 입지만 **색
/// 없이도 키와 낱말이 글자로 서고**, 묶음은 `+`, 토글은 `[켜짐]` 이 댄다.
///
/// 위 가름줄 하나가 몸통과 가른다 — 몸통의 아래 테두리에 바로 붙으므로 선이 없으면 격자의 첫
/// 줄이 목록의 줄로 읽힌다. 가름줄은 굵지 않다 — 키 먹는 칸의 굵은 선은 목록·상세의 것이다.
fn menu_panel(f: &mut Frame, grid: &menu::Grid, at: Rect) {
    let room = at.width.saturating_sub(2) as usize;
    let lines: Vec<Line> = (0..grid.rows)
        .map(|r| {
            let mut spans = Vec::new();
            for (c, p) in grid.row(r).enumerate() {
                if c > 0 {
                    spans.push(Span::raw(" ".repeat(menu::GAP)));
                }
                spans.push(Span::styled(p.key.clone(), bold().fg(MENU_KEY)));
                spans.push(Span::styled(menu::SEP, Style::new().fg(MENU_SEP)));
                spans.push(Span::styled(p.text.clone(), menu_word(p.group)));
            }
            // 격자가 이미 폭에 맞췄다. 한 열도 안 드는 좁은 창만 여기서 잘린다.
            fit(Line::from(spans), room)
        })
        .collect();
    let block = Block::default().borders(Borders::TOP).border_style(dim()).padding(Padding::horizontal(1));
    f.render_widget(Clear, at);
    f.render_widget(Paragraph::new(lines).block(block), at);
}

/// SPC 메뉴의 색(사용자 결정, moai-r2dt·moai-1uod). 터미널 16색 번호로 준 것이다 — sixteen-colour-table
/// 의 `f10b00` 은 글자색 10·바탕색 0 이지 hex 가 아니다. 키 10(밝은 초록)·`:` 8(밝은 검정)·실행
/// 12(밝은 파랑)·`+묶음` 13(밝은 자홍). 바탕은 깔지 않는다. 뜻은 여전히 글자(`+`)가 진다.
const MENU_KEY: Color = Color::LightGreen;
const MENU_SEP: Color = Color::DarkGray;
const MENU_RUN: Color = Color::LightBlue;
const MENU_GROUP: Color = Color::LightMagenta;

/// 메뉴 칸 낱말의 색 — 묶음이면 [`MENU_GROUP`], 실행이면 [`MENU_RUN`].
fn menu_word(group: bool) -> Style {
    Style::new().fg(if group { MENU_GROUP } else { MENU_RUN })
}

/// 메뉴가 열린 동안의 맨 아랫줄 — 접두어 줄(doom 의 `SPC- <leader>`). 왼쪽에 지금 접두어와 층의
/// 이름(`SPC t- 토글`), 오른쪽 끝에 나가는 법(`Esc 닫기`·하위 층이면 `Bksp 위로`). 탐색의 키는
/// 메뉴 안에서 안 들으므로 바의 자리를 이 줄이 통째로 쓴다. 폭이 모자라 못 세운 항목이 있으면
/// 그 수를 댄다 — 말없이 빠지면 없는 줄 안다.
///
/// **격자 설 높이가 없으면 여기로 접는다** — `SPC-  / 검색  f 거름망 …`. 항목이 먼저고 나가는
/// 법은 자리가 남을 때만 붙는다: Esc 는 어디서든 닫고, 못 누르는 항목은 댈 수 없다.
fn menu_line(f: &mut Frame, app: &App, items: &[menu::Entry], grid: &menu::Grid, at: Rect) {
    let held = app.chord.held();
    let room = at.width as usize;
    let mut spans = vec![Span::styled(format!("{}-", menu::title(held)), bold())];
    if grid.rows == 0 {
        for e in items {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(e.key.clone(), bold().fg(MENU_KEY)));
            spans.push(Span::styled(format!(" {}", e.text()), menu_word(e.is_group())));
        }
    } else {
        spans.push(Span::raw(format!(" {}", menu::name(held))));
        if grid.hidden > 0 {
            spans.push(Span::styled(format!("  그 밖 {}개 — 창을 넓히면 선다", grid.hidden), dim()));
        }
    }
    let mut exits = vec![key(&label(MENU, Menu::Close), Menu::Close.what())];
    if held.len() > 1 {
        exits.push(key(&label(MENU, Menu::Up), Menu::Up.what()));
    }
    let width = |v: &[Span]| v.iter().map(|s| crate::text::width(&s.content)).sum::<usize>();
    let used = width(&spans) + width(&exits);
    if used <= room {
        spans.push(Span::raw(" ".repeat(room - used)));
        spans.extend(exits);
    }
    f.render_widget(Paragraph::new(fit(Line::from(spans), room)), at);
}

/// 키 바를 폭에 맞춰 놓는다. `optional` 은 **뒤에서부터** 들어가고 모자라면 앞쪽이 떨어진다.
fn bar(f: &mut Frame, at: Rect, mut optional: Vec<Span<'_>>, keep: Vec<Span<'_>>) {
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

/// 바의 키 하나. **뒤에 공백을 두지 않는다** — 앞뒤로 두면 칸 사이가 두 칸이 되고,
/// 그 여섯 칸 때문에 80칸 터미널에서 줄이 넘쳐 맨 끝의 `SPC 메뉴` 가 말없이
/// 잘린다. 나갈 길을 못 찾는 것이 빽빽한 줄보다 나쁘다.
fn key(k: &str, what: &str) -> Span<'static> {
    Span::styled(format!(" {k} {what}"), dim())
}

/// 글칸 안내 — `Enter <무엇>  Esc 그만`. 키 이름은 표([`PROMPT`])에서 읽는다.
fn prompt_help(apply: &str) -> String {
    format!("{} {apply}  {} 그만", label(PROMPT, Prompt::Apply), label(PROMPT, Prompt::Cancel))
}

/// 검색 칸 이름표 — 좁힌 범위면 `검색·id` 처럼 붙인다(moai-kojj). 전체면 옛 이름 그대로다.
fn grep_label(g: GrepIn) -> String {
    match g {
        GrepIn::All => "검색".to_string(),
        g => format!("검색·{}", g.name()),
    }
}

/// 검색 칸 안내 — `3건  Tab·Shift-Tab 범위  Enter 걸기  Esc 그만`. 셈은 **친 글이 있을 때만**
/// 낸다(moai-00le): 빈 칸은 거름망이 없는 것이라 전체 수가 "걸린 수" 로 읽힌다.
fn grep_help(app: &App, q: &Input) -> String {
    let scope = format!("{} 범위  {}", labels(PROMPT, &[Prompt::NextScope, Prompt::PrevScope]), prompt_help("걸기"));
    match q.text().trim().is_empty() {
        true => scope,
        false => format!("{}건  {scope}", app.hit_count()),
    }
}

/// 찾은 글자 — 밝은 파랑에 **굵게**(moai-yio7). 색만으로 말하지 않는다: 색이 없는 터미널에서도
/// 굵기가 남고, 무엇을 찾았는지는 뱃지·검색 칸이 글로 말한다.
fn found() -> Style {
    Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD)
}

/// `spans` 의 글에서 `q` 가 든 자리를 [`found`] 로 덧칠한다. 대소문자를 가리지 않는다.
///
/// **글자 단위로 견준다** — 통째로 `to_lowercase` 한 글은 바이트 길이가 달라질 수 있어
/// (`İ`) 찾은 자리를 원문에 되짚을 수 없다. 이미 칠한 조각(빛줄기·흐림)은 제 스타일 위에 덧댄다.
fn mark(spans: Vec<Span<'static>>, q: Option<&str>) -> Vec<Span<'static>> {
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let needle: Vec<char> = match q {
        Some(q) if !q.trim().is_empty() => q.chars().map(fold).collect(),
        _ => return spans,
    };
    let hay: Vec<char> = spans.iter().flat_map(|s| s.content.chars()).map(fold).collect();
    let mut hit = vec![false; hay.len()];
    let mut at = 0;
    while at + needle.len() <= hay.len() {
        if hay[at..at + needle.len()] == needle[..] {
            hit[at..at + needle.len()].iter_mut().for_each(|h| *h = true);
            at += needle.len();
        } else {
            at += 1;
        }
    }
    if !hit.contains(&true) {
        return spans;
    }
    let mut out = Vec::new();
    let mut n = 0;
    for span in spans {
        let mut run = String::new();
        let mut on = None;
        for c in span.content.chars() {
            if on.is_some_and(|o| o != hit[n]) {
                let style = if on == Some(true) { span.style.patch(found()) } else { span.style };
                out.push(Span::styled(std::mem::take(&mut run), style));
            }
            on = Some(hit[n]);
            run.push(c);
            n += 1;
        }
        if !run.is_empty() {
            let style = if on == Some(true) { span.style.patch(found()) } else { span.style };
            out.push(Span::styled(run, style));
        }
    }
    out
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
pub(super) mod tests {
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
        // 묶음은 집은 멤버가 있을 때만 돌므로(`App::spins`) 집은 일이 하나는 있어야 한다.
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
    pub(in crate::tui) fn render(app: &mut App, w: u16, h: u16) -> Vec<String> {
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

    /// **다 보이는 채로 세운다.** 그림 시험의 줄에는 끝난 멤버가 있고, 시험은 그 줄을 그리는
    /// 법을 본다 — 처음 done 을 숨기는 보기(moai-fmv5)는 제 시험이 따로 본다.
    fn app() -> App {
        every(issues())
    }

    fn every(issues: Vec<Issue>) -> App {
        let mut a = App::new(issues, Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        a.view = super::super::view::View::default();
        a.see();
        a
    }

    /// 빛줄기는 **글자와 폭을 그대로 두고** 스타일만 칸마다 바꾸며, 한 걸음에 한 칸씩 흐르고,
    /// 제목 끝을 지나 쉬었다가 앞에서 다시 들어온다.
    #[test]
    fn a_glint_flows_one_cell_per_step_and_keeps_the_text() {
        let title = "집은 멤버 제목";
        let lit = |spans: &[Span]| -> Vec<usize> {
            let mut col = 0;
            let mut out = Vec::new();
            for s in spans {
                for c in s.content.chars() {
                    if s.style.add_modifier.contains(Modifier::BOLD) {
                        out.push(col);
                    }
                    col += crate::text::width(c.encode_utf8(&mut [0; 4]));
                }
            }
            out
        };
        let cycle = crate::text::width(title) + glint().len() + GLINT_REST;
        for frame in 0..cycle * 2 {
            let spans = shimmer(title.to_string(), frame);
            let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
            assert_eq!(text, title, "글자가 바뀌었다 @ {frame}");
        }
        // 들어오기 전과 쉬는 동안에는 빛이 없다.
        assert!(lit(&shimmer(title.to_string(), 0)).is_empty(), "들어오기 전에 빛났다");
        assert!(lit(&shimmer(title.to_string(), cycle - 1)).is_empty(), "쉬는 동안 빛났다");
        // 가운데 칸(굵게)이 걸음마다 오른쪽으로 간다. 가운데는 앞머리 두 칸 뒤라, 걸음 4 는
        // `은`(2칸), 걸음 7 은 `멤`(5칸)에 선다. 두 칸 글자의 **뒤 칸**에 서는 걸음(5)에도 그
        // 글자가 굵다 — 시작 칸으로만 재면 거기서 빛이 꺼져 한 걸음 걸러 깜빡인다.
        let a = lit(&shimmer(title.to_string(), 4));
        let back = lit(&shimmer(title.to_string(), 5));
        let b = lit(&shimmer(title.to_string(), 7));
        assert_eq!((a.as_slice(), b.as_slice()), ([2].as_slice(), [5].as_slice()), "빛이 제자리에 안 선다");
        assert_eq!(back, vec![2], "가운데가 두 칸 글자의 뒤 칸에 서자 빛이 꺼졌다");
        assert!(!a.is_empty() && !b.is_empty() && b[0] > a[0], "빛이 안 흐른다 — {a:?} → {b:?}");
        // 제목 안의 모든 걸음에서 굵은 칸이 하나는 있다 — 한글 제목에서 깜빡이지 않는다.
        for frame in 3..=crate::text::width(title) + 1 {
            assert!(!lit(&shimmer(title.to_string(), frame)).is_empty(), "걸음 {frame} 에 빛이 꺼졌다");
        }
        assert_eq!(shimmer(title.to_string(), 5), shimmer(title.to_string(), 5 + cycle), "한 바퀴가 제자리로 안 온다");
    }

    /// 목록에서 **집은 이슈의 제목은** 빛나고 끝난 멤버는 안 흐른다. 묶음 줄은
    /// `a_group_title_glints_exactly_when_it_spins` 가 본다.
    #[test]
    fn only_a_held_issue_title_glints_in_the_list() {
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // 커서는 `..` 에 둔다 — 들어가면 첫 줄(끝난 멤버)에 서는데(moai-cm13), 커서 줄의 모양이
        // 제목 칸을 가려 여기서 재려는 빛과 섞인다.
        a.key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        // 가운데 칸이 제목 첫 글자에 오는 걸음.
        a.spin = 2;
        let (w, h) = (80u16, 10u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let text = render(&mut a, w, h);
        // **제목 칸만** 본다 — 칸 글리프(`IN_PROGRESS`, 밝은 노랑 굵게)가 같은 줄에 있어 줄
        // 전체를 보면 빛줄기 없이도 참이 된다.
        let glints_from = |row: &str, first: &str| {
            let y = text.iter().position(|l| l.contains(row)).unwrap_or_else(|| panic!("{row} 가 없다")) as u16;
            let x0 = (0..w).find(|&x| buf[(x, y)].symbol() == first).unwrap_or_else(|| panic!("{first} 가 없다"));
            (x0..w / 2).any(|x| buf[(x, y)].modifier.contains(Modifier::BOLD) && buf[(x, y)].fg == Color::LightYellow)
        };
        assert!(glints_from("argos-0004", "집"), "집은 이슈 제목에 빛이 없다\n{}", text.join("\n"));
        assert!(!glints_from("argos-0003", "멤"), "끝난 멤버가 빛났다\n{}", text.join("\n"));
    }

    /// **묶음 줄도 돌 때 빛난다**(moai-eomg) — 자는 도는 글리프와 같은 `App::spins` 다. 밑에
    /// 집은 일이 있는 에픽·마일스톤은 흐르고, 멤버가 첫 칸뿐이거나 집은 멤버를 미룬 에픽은 안
    /// 흐른다. 마일스톤도 묶음이라 같은 자다.
    #[test]
    fn a_group_title_glints_exactly_when_it_spins() {
        let make = |id: &str, title: &str, kind: Kind, st: &str| {
            Issue::new(id.into(), title.into(), kind, Status::new(st), "2026-09-01T00:00:00Z")
        };
        let member = |id: &str, epic: &str, st: &str| {
            let mut m = make(id, "멤버", Kind::Issue, st);
            m.epic = Some(epic.into());
            m
        };
        let mut first = make("argos-0001", "빈돌", Kind::Milestone, "todo");
        first.priority = Some(0);
        let stone = make("argos-0010", "빛돌", Kind::Milestone, "todo");
        let mut under = make("argos-0011", "돌밑에픽", Kind::Epic, "todo");
        under.milestone = Some("argos-0010".into());
        let lit = make("argos-0041", "빛에픽", Kind::Epic, "todo");
        let idle = make("argos-0021", "멈춘에픽", Kind::Epic, "todo");
        let shelved = make("argos-0031", "미룬에픽", Kind::Epic, "todo");
        let mut put_off = member("argos-0032", "argos-0031", "in_progress");
        put_off.deferred_at = Some("2026-09-02T00:00:00Z".into());
        let issues = vec![
            first,
            stone,
            under,
            member("argos-0012", "argos-0011", "in_progress"),
            lit,
            member("argos-0042", "argos-0041", "in_progress"),
            idle,
            member("argos-0022", "argos-0021", "todo"),
            shelved,
            put_off,
        ];
        let cfg = || Config::parse("prefix = \"argos\"\n").unwrap();
        // 가운데 칸이 제목 첫 글자에 오는 걸음. 커서는 재지 않는 줄에 둔다 — 커서 줄의 모양이
        // 제목 칸을 가린다.
        let glints = |a: &mut App, row: &str, first: &str| {
            a.spin = 2;
            let (w, h) = (100u16, 14u16);
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| screen(f, a)).unwrap();
            let buf = term.backend().buffer().clone();
            let text = render(a, w, h);
            let y = text.iter().position(|l| l.contains(row)).unwrap_or_else(|| panic!("{row} 가 없다\n{}", text.join("\n"))) as u16;
            let x0 = (0..w).find(|&x| buf[(x, y)].symbol() == first).unwrap_or_else(|| panic!("{first} 가 없다"));
            let lit = (x0..w / 2).any(|x| buf[(x, y)].modifier.contains(Modifier::BOLD) && buf[(x, y)].fg == Color::LightYellow);
            (lit, text.join("\n"))
        };

        // 뿌리는 마일스톤이다 — 커서는 빈 마일스톤에 선다.
        let mut root = App::new(issues.clone(), cfg(), Path::new());
        let (on, text) = glints(&mut root, "argos-0010", "빛");
        assert!(on, "집은 일이 밑에 있는 마일스톤이 안 빛난다\n{text}");
        let (on, text) = glints(&mut root, "argos-0001", "빈");
        assert!(!on, "빈 마일스톤이 빛났다\n{text}");

        // 마일스톤 없는 에픽들 — 커서는 `..` 에 선다.
        let mut basket = App::new(issues, cfg(), vec![crate::nav::Seg::Milestone(None)]);
        let (on, text) = glints(&mut basket, "argos-0041", "빛");
        assert!(on, "집은 멤버가 있는 에픽이 안 빛난다\n{text}");
        let (on, text) = glints(&mut basket, "argos-0021", "멈");
        assert!(!on, "멤버가 첫 칸뿐인 에픽이 빛났다\n{text}");
        let (on, text) = glints(&mut basket, "argos-0031", "미");
        assert!(!on, "집은 멤버를 미룬 에픽이 빛났다\n{text}");
    }

    /// 에픽 줄은 **끝난/일 셈을 테두리 바로 앞에 오른쪽 정렬로** 댄다 — 제목 길이와 상관없이
    /// 셈이 한 세로줄에 선다. 바탕색은 깔지 않는다(moai-u3r2).
    #[test]
    fn an_epic_row_ends_with_its_tally_against_the_border() {
        let mut a = app();
        let (w, h) = (80u16, 10u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let text = render(&mut a, w, h);
        let y = text.iter().position(|l| l.contains("argos-0001")).expect("에픽 줄이 없다") as u16;
        let title = (0..w).find(|&x| buf[(x, y)].symbol() == "아").expect("제목이 없다");
        // 포커스 칸의 테두리는 굵은 선이다(`frame`).
        let border = (title..w).find(|&x| buf[(x, y)].symbol() == "┃").expect("테두리가 없다");
        assert_eq!(
            (buf[(border - 3, y)].symbol(), buf[(border - 2, y)].symbol(), buf[(border - 1, y)].symbol()),
            ("1", "/", "2"),
            "셈이 테두리 끝에 붙지 않았다\n{}",
            text.join("\n")
        );
        assert!((0..w).all(|x| buf[(x, y)].bg != Color::Green), "바탕이 깔렸다");
    }

    /// 색 없이 글자만 봐도 읽힌다 — 경로, id, 우선순위, 칸 글리프, 제목, 키 바.
    #[test]
    fn the_screen_reads_without_colour() {
        let lines = render(&mut app(), 100, 12).join("\n");
        assert!(lines.contains('/'), "경로가 없다\n{lines}");
        assert!(lines.contains("argos-0001"), "id 가 없다\n{lines}");
        assert!(lines.contains("p1"), "우선순위가 없다\n{lines}");
        // 뿌리에는 묶음뿐이다 — 그 밑에 집은 일이 있으니 **묶음이 돈다.** 어느 프레임이든
        // `SPIN` 의 한 글자다(집은 일이 없으면 안 도는 쪽은 `a_half_done_group_stands_still`).
        assert!(style::SPIN.iter().any(|g| lines.contains(g)), "집은 멤버가 있는 묶음이 안 돈다\n{lines}");
        // 그 안의 집은 일도 도는 프레임을 낸다.
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let inside = render(&mut a, 100, 12).join("\n");
        assert!(style::SPIN.iter().any(|g| inside.contains(g)), "집은 일이 안 돈다\n{inside}");
        assert!(lines.contains("Enter") && lines.contains("SPC 메뉴"), "키 바가 없다\n{lines}");
        // 긴 제목은 **잘린다**. 잘렸다는 표시가 남아야 어디까지가 제목인지 안다.
        assert!(lines.contains("아주 긴"), "에픽 제목이 없다\n{lines}");
        assert!(lines.contains('…'), "잘렸는데 표시가 없다\n{lines}");
        // 디렉터리는 제목 뒤에 `/` 가 붙는다
    }

    /// 옆 워크트리에서 온 줄은 목록과 상세 둘 다 `⎇ <브랜치>` 를 댄다. 켜진 동안은
    /// 경로 줄이 그렇다고 말하고, `w` 가 켜고 끈다. **켜진 채로 시작한다**(moai-zcuh).
    #[test]
    fn a_line_from_another_worktree_is_marked_in_the_list_and_the_detail() {
        let mut a = app();
        assert!(a.worktree, "겹쳐 보기가 꺼진 채로 시작했다");
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.hit("SPC t w");
        assert!(!a.worktree, "SPC t w 가 안 껐다");
        let plain_screen = render(&mut a, 120, 12).join("\n");
        assert!(!plain_screen.contains('⎇'), "안 겹쳤는데 머리표가 섰다\n{plain_screen}");
        // 켜는 키는 메뉴가 상태 낱말과 함께 댄다.
        a.hit("SPC t");
        let menu_screen = render(&mut a, 120, 12).join("\n");
        assert!(menu_screen.contains("w : 워크트리 겹쳐 보기 [꺼짐]"), "켜는 키를 안 알린다\n{menu_screen}");
        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // 저장소 없이 세운 App 이라 `w` 는 켜기만 하고 읽지 않는다 — 겹친 결과는 손으로 넣는다.
        a.hit("SPC t w");
        assert!(a.worktree, "SPC t w 가 안 켰다");
        let mut theirs = issues()[2].clone();
        theirs.status = crate::model::Status::new("review");
        theirs.updated_at = "2026-09-02T00:00:00Z".into();
        let (all, origin) = crate::worktree::overlay(
            issues(),
            vec![crate::worktree::Side::new("feat/x", "/wt", vec![theirs])],
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
        assert!(lines[0].contains("⎇ feat/x") && lines[0].contains("SPC t w 로 끈다"), "켜졌다고 안 말한다\n{screen}");
        assert!(screen.matches("⎇ feat/x").count() >= 3, "상세에 머리표가 없다\n{screen}");

        a.hit("SPC t w");
        assert!(!a.worktree, "SPC t w 가 안 껐다");
    }

    /// **옆 워크트리가 없으면 경로 줄은 겹쳐 보기를 말하지 않는다**(moai-d5vn). 겹쳐 보기는 켜진
    /// 채로 시작하므로(moai-zcuh) 없을 때도 세우면 모든 프로젝트의 경로 줄 절반을 먹는다 — 옆이
    /// 없으면 켜진 화면과 꺼진 화면이 정말로 같으니 가를 것이 없다. 켜짐은 메뉴의 `[켜짐]` 이 댄다.
    #[test]
    fn the_path_line_says_nothing_of_the_overlay_when_no_worktree_sits_beside() {
        let mut a = app();
        assert!(a.worktree, "겹쳐 보기가 꺼진 채로 시작했다");
        assert!(a.origin.labels().is_empty());
        let lines = render(&mut a, 120, 12);
        assert!(!lines[0].contains('⎇') && !lines[0].contains("워크트리"), "옆이 없는데 경로 줄이 겹쳐 보기를 댄다\n{}", lines[0]);
        a.hit("SPC t");
        let menu = render(&mut a, 120, 12).join("\n");
        assert!(menu.contains("워크트리 겹쳐 보기 [켜짐]"), "켜진 것을 댈 자리가 없다\n{menu}");
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let lines = render(&mut a, 100, 20).join("\n");
        assert!(lines.contains("막힘"), "{lines}");
        assert!(lines.contains("아주 긴"), "막는 것의 제목이 없다\n{lines}");
    }

    /// 본문에 든 ESC 가 화면을 다시 칠하지 못한다.
    #[test]
    fn a_body_cannot_repaint_the_screen() {
        let mut issues = issues();
        issues[1].body = Some("앞\u{1b}[2J뒤".into());
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

    /// **쓰기의 알림은 80칸에서도 붙박이에 밀려 잘리지 않는다.** 드러난 것·못 읽는
    /// 줄이 함께 서도 만든 id 가 보이고, 거름망에 가렸다는 말도 선다. 알림만이면
    /// 급한 색이 아니다 — 뜻은 `✓` 와 낱말이 진다.
    #[test]
    fn a_write_notice_survives_eighty_columns_beside_the_standing_banner() {
        let mut a = app();
        a.warnings = 4;
        a.unreadable = vec![None; 2];
        a.notice = Some("✓ 담김 · argos-0002 — 거름망에 가려 안 보인다 · Esc 로 푼다".into());
        let lines = render(&mut a, 80, 12);
        assert!(lines[1].contains("✓ 담김 · argos-0002 — 거름망에 가려 안 보인다"), "{}", lines.join("\n"));
        assert!(lines.iter().all(|l| crate::text::width(l) <= 80));

        a.warnings = 0;
        a.unreadable.clear();
        assert_eq!(banner(&a), Some((" ✓ 담김 · argos-0002 — 거름망에 가려 안 보인다 · Esc 로 푼다 ".into(), false)));
        // 다시 읽기가 실패했으면 실패가 앞에 선다.
        a.trouble = Some("다시 읽지 못했다 — 락".into());
        let (text, urgent) = banner(&a).unwrap();
        assert!(urgent && text.find("다시 읽지").unwrap() < text.find("담김").unwrap(), "{text}");
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
            let mut a = every(issues);
            a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            render(&mut a, 100, 20).join("\n")
        };
        let lines = drawn(true);
        assert!(lines.contains("풀림"), "끝난 막음을 아직 막혔다고 그린다\n{lines}");
        assert!(!lines.contains("막힘"), "{lines}");
        let lines = drawn(false);
        assert!(lines.contains("막힘"), "적힌 칸만 닫힌 에픽을 풀렸다고 그린다\n{lines}");
    }

    /// **상세의 막음 줄은 `moai ready` 와 같은 답을 한다.** 한때 여기가
    /// `report::is_blocked` 를 손으로 베껴, 없는 id 를 가리키는 막음을 `ready` 는
    /// 안 막힌 것으로 고르는데 탐색기는 "막힘" 이라 그렸다(moai-af64). 끊긴 막음·
    /// 미뤄 둔 막음·멤버가 다 끝난 묶음 막음을 한 자리에서 `ready` 와 견준다.
    #[test]
    fn the_detail_blocker_lines_agree_with_ready() {
        let make = |id: &str, title: &str, kind: Kind, status: &str| {
            Issue::new(id.into(), title.into(), kind, Status::new(status), "2026-09-01T00:00:00Z")
        };
        // (막는 쪽을 꾸미는 법, 상세에 나와야 할 낱말, 나오면 안 될 낱말)
        type Case = (&'static str, fn(&mut Vec<Issue>), &'static [&'static str], &'static [&'static str]);
        let cases: [Case; 6] = [
            // 멤버가 없는 묶음 — 풀지 않고 비었다고 댄다(moai-1c2l).
            ("빈 묶음", |v| v[6].epic = None, &["막힘", "멤버 없음"], &["풀림"]),
            ("끊긴 막음", |_| {}, &["끊김", "argos-9999"], &["막힘", "풀림"]),
            ("미뤄 둔 막음", |v| v[4].deferred_at = Some("2026-09-02T00:00:00Z".into()), &["막힘", "미룸"], &["풀림"]),
            ("멤버가 다 끝난 묶음", |v| v[6].status = Status::new("done"), &["풀림"], &["막힘"]),
            ("멤버가 남은 묶음", |_| {}, &["막힘"], &["풀림", "미룸"]),
            // 끝난 멤버 하나에 남은 멤버를 미뤄 접은 묶음 — 칸은 done 이어도 아직 막는다(moai-0gxf).
            ("미뤄 접은 묶음", |v| {
                v[6].deferred_at = Some("2026-09-02T00:00:00Z".into());
                let mut closed = Issue::new("argos-0009".into(), "끝난 멤버".into(), Kind::Issue, Status::new("done"), "2026-09-01T00:00:00Z");
                closed.epic = Some("argos-0007".into());
                v.push(closed);
            }, &["막힘", "미룬 멤버 argos-0008"], &["풀림"]),
        ];
        for (name, arrange, want, deny) in cases {
            let mut all = issues();
            let mut blocked = make("argos-0005", "막힌 일", Kind::Issue, "todo");
            blocked.epic = Some("argos-0001".into());
            blocked.blocked_by = vec![match name {
                "끊긴 막음" => "argos-9999",
                "미뤄 둔 막음" => "argos-0006",
                _ => "argos-0007",
            }
            .into()];
            all.push(blocked);
            // **제목은 흔한 길이로 둔다** — 짧으면 미룸 낱말이 잘려 나가도 이 시험이 못 본다.
            all.push(make("argos-0006", "미룰 일 — 제목이 흔한 이슈만큼 길어 패널 폭을 넘는다", Kind::Issue, "todo"));
            all.push(make("argos-0007", "막는 에픽", Kind::Epic, "todo"));
            let mut inner = make("argos-0008", "막는 에픽의 멤버", Kind::Issue, "in_progress");
            inner.epic = Some("argos-0007".into());
            all.push(inner);
            arrange(&mut all);

            let cfg = Config::parse("prefix = \"argos\"\n").unwrap();
            let picked = crate::report::ready(&all, &cfg).iter().any(|i| i.id == "argos-0005");
            let path = vec![crate::nav::Seg::Epic("argos-0001".into())];
            let mut a = App::new(all, cfg, path);
            a.cursor = a
                .rows()
                .iter()
                .position(|r| matches!(r, Row::Item(e) if e.at().is_some_and(|i| a.issues[i].id == "argos-0005")))
                .unwrap_or_else(|| panic!("{name}: 막힌 일이 목록에 없다"));
            let lines = render(&mut a, 120, 24).join("\n");
            for w in want {
                assert!(lines.contains(w), "{name}: `{w}` 가 없다\n{lines}");
            }
            for d in deny {
                assert!(!lines.contains(d), "{name}: `{d}` 가 나왔다\n{lines}");
            }
            assert_eq!(
                picked,
                !lines.contains("막힘"),
                "{name}: ready 는 {} 탐색기는 다르게 그린다\n{lines}",
                if picked { "고르는데" } else { "안 고르는데" }
            );
        }
    }

    /// **반쯤 끝난 묶음은 안 돈다** — 목록 줄도 상세 머리도. 읽은 칸은 `in_progress` 지만
    /// 그 밑에서 아무도 손대지 않는다. 멤버 하나를 집는 순간 둘 다 돌고, 그 판단은
    /// 깨우는 쪽(`App::spun`)도 그린 것을 따른다(moai-x5eg·moai-5jh6).
    #[test]
    fn a_half_done_group_stands_still() {
        let mut issues = issues();
        issues[2].status = Status::new("todo");
        let cfg = || Config::parse("prefix = \"argos\"\n").unwrap();
        let mut a = App::new(issues.clone(), cfg(), Path::new());
        assert_eq!(a.column(0), "in_progress");
        let lines = render(&mut a, 120, 16).join("\n");
        assert!(lines.contains("▸ in_progress"), "상세 머리가 정지 글리프가 아니다\n{lines}");
        assert!(!style::SPIN.iter().any(|g| lines.contains(g)), "아무도 손대지 않는 묶음이 돈다\n{lines}");
        assert!(!a.spun, "안 도는 화면이 빠른 걸음으로 깨운다");

        issues[2].status = Status::new("in_progress");
        let mut a = App::new(issues, cfg(), Path::new());
        let lines = render(&mut a, 120, 16).join("\n");
        let row = lines.lines().find(|l| l.contains("> argos-0001")).unwrap();
        assert!(style::SPIN.iter().any(|g| row.contains(g)), "집은 멤버가 있는데 묶음이 안 돈다\n{lines}");
        assert!(!lines.contains("▸ in_progress"), "상세 머리만 멈췄다\n{lines}");
        assert!(a.spun, "도는 화면을 안 깨운다");
    }

    /// **도는 목록 줄은 색 없이도 칸이 갈린다**(moai-q59j). 시작한 칸이 모두 돌아 `in_progress`
    /// 와 `review` 가 같은 스피너가 되므로, 스피너 뒤에 그 칸의 멈춘 글리프가 붙는다. 칸 이름을
    /// 바꾼 설정에서도 시작한 칸이 돌고 루프를 깨운다.
    #[test]
    fn a_spinning_row_still_says_which_column_without_colour() {
        let row = |id: &str, title: &str, st: &str| {
            crate::model::Issue::new(id.into(), title.into(), Kind::Issue, Status::new(st), "2026-09-01T00:00:00Z")
        };
        let issues = vec![row("argos-0001", "집은 일", "in_progress"), row("argos-0002", "리뷰 기다리는 일", "review"), row("argos-0003", "안 한 일", "todo")];
        let mut a = every(issues);
        let lines = render(&mut a, 120, 16);
        let line_of = |id: &str| lines.iter().find(|l| l.contains(id)).cloned().unwrap_or_else(|| panic!("{id} 줄이 없다\n{}", lines.join("\n")));
        let (held, waiting, idle) = (line_of("argos-0001"), line_of("argos-0002"), line_of("argos-0003"));
        for l in [&held, &waiting] {
            assert!(style::SPIN.iter().any(|g| l.contains(g)), "시작한 줄이 안 돈다\n{l}");
        }
        assert!(style::SPIN.iter().any(|g| held.contains(&format!("{g}▸"))), "집은 줄에 칸 글리프가 없다\n{held}");
        assert!(style::SPIN.iter().any(|g| waiting.contains(&format!("{g}?"))), "리뷰 줄에 칸 글리프가 없다 — 색만으로 갈린다\n{waiting}");
        assert!(!style::SPIN.iter().any(|g| idle.contains(g)) && idle.contains(" ·"), "안 도는 줄의 글리프 자리가 어긋났다\n{idle}");
        assert!(a.spun, "도는 줄을 그리고도 안 깨운다");

        // 칸 이름을 바꾼 설정에서도 시작한 칸이 돈다 — 멈춘 글리프는 설정으로 더한 칸의 `○`.
        let renamed = Config::parse("prefix = \"argos\"\nstatuses = \"todo, doing, check, done\"\n").unwrap();
        let mut a = App::new(vec![row("argos-0001", "하는 일", "doing")], renamed, Path::new());
        let lines = render(&mut a, 120, 16).join("\n");
        assert!(style::SPIN.iter().any(|g| lines.contains(&format!("{g}○"))), "바꾼 칸이 안 돈다\n{lines}");
        assert!(a.spun, "바꾼 칸의 스피너가 루프를 안 깨운다");
    }

    /// **미룬 `in_progress` 하나뿐인 칸은 건수 글리프도 안 돈다**(moai-tawj) — 줄은 멈추는데
    /// 목록 머리·롤업의 건수만 칸 이름으로 돌리면, 루프가 빠른 걸음으로 안 깨우는 스피너의
    /// 한 프레임에 멈춰 선다.
    #[test]
    fn a_deferred_pick_does_not_spin_the_counts_either() {
        let mut issues = issues();
        issues[2].deferred_at = Some("2026-09-02T00:00:00Z".into());
        let mut a = every(issues);
        let root = render(&mut a, 120, 16).join("\n");
        assert!(!a.spun);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let inside = render(&mut a, 120, 16).join("\n");
        for lines in [&root, &inside] {
            assert!(!style::SPIN.iter().any(|g| lines.contains(g)), "안 깨우는 스피너가 섰다\n{lines}");
        }
        assert!(inside.contains("▸ in_progress 1"), "건수에 정지 글리프가 없다\n{inside}");
    }

    /// **루프는 화면에 도는 것이 그려졌을 때만 빠른 걸음으로 깬다**(moai-5jh6). 집은 일이
    /// 스크롤 밖 에픽 안에 있으면 안 깨고, 굴려 보이면 깬다. SPC 메뉴가 떠도 뒤의 줄이
    /// 보이면 깨고, 폼이 목록·상세를 통째로 덮으면 안 깬다.
    #[test]
    fn the_loop_wakes_fast_only_for_a_spinner_on_the_screen() {
        let make = |id: &str, kind: Kind, st: &str| {
            Issue::new(id.into(), format!("{id} 제목"), kind, Status::new(st), "2026-09-01T00:00:00Z")
        };
        // 앞선 에픽들이 창을 채우고, 집은 멤버가 있는 에픽은 우선순위가 낮아 맨 뒤에 선다.
        let mut issues: Vec<Issue> = (101..113)
            .map(|n| {
                let mut e = make(&format!("argos-{n:04}"), Kind::Epic, "todo");
                e.priority = Some(1);
                e
            })
            .collect();
        let mut held_epic = make("argos-0001", Kind::Epic, "todo");
        held_epic.priority = Some(3);
        let mut held = make("argos-0002", Kind::Issue, "in_progress");
        held.epic = Some("argos-0001".into());
        issues.extend([held_epic, held]);
        let mut a = every(issues);
        assert!(a.spins(a.index.find("argos-0001").unwrap()), "시험의 전제 — 그 에픽은 돈다");

        let off = render(&mut a, 100, 10).join("\n");
        assert!(!off.contains("argos-0001"), "시험의 전제 — 도는 에픽이 창 밖이어야 한다\n{off}");
        assert!(!a.spun, "안 보이는 집은 일로 빠른 걸음으로 깬다\n{off}");

        a.hit("G");
        let on = render(&mut a, 100, 10).join("\n");
        assert!(on.contains("argos-0001"), "끝으로 안 갔다\n{on}");
        assert!(a.spun, "보이는 도는 줄을 안 깨운다\n{on}");

        // 그리기만 다시 해도 답이 따라온다 — 지난 프레임의 값이 남지 않는다.
        a.key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        let back = render(&mut a, 100, 10).join("\n");
        assert!(!back.contains("argos-0001"), "맨 위로 안 굴렀다\n{back}");
        assert!(!a.spun, "굴려 치운 줄로 여전히 깬다\n{back}");

        a.hit("G");
        a.hit("SPC");
        let menu = render(&mut a, 160, 24).join("\n");
        assert!(menu::open(&a.chord), "메뉴가 안 떴다");
        assert!(a.spun, "메뉴 뒤로 보이는 도는 줄을 안 깨운다\n{menu}");

        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        a.hit("SPC n");
        assert!(matches!(a.mode, Mode::Idea(_)), "{:?}", a.mode);
        let form = render(&mut a, 160, 24).join("\n");
        assert!(!a.spun, "폼이 덮은 도는 줄로 깬다\n{form}");
    }

    /// **상세에만 선 스피너도 깨운다.** `(마일스톤 없음)` 바구니는 제 줄에 글리프가 없어 목록은
    /// 안 돌지만, 커서를 올리면 상세 롤업의 칸별 건수가 돈다.
    #[test]
    fn a_spinner_only_in_the_detail_still_wakes_the_loop() {
        let make = |id: &str, kind: Kind, st: &str| {
            Issue::new(id.into(), format!("{id} 제목"), kind, Status::new(st), "2026-09-01T00:00:00Z")
        };
        let stone = make("argos-0001", Kind::Milestone, "todo");
        let mut epic = make("argos-0002", Kind::Epic, "todo");
        epic.milestone = Some("argos-0001".into());
        let mut member = make("argos-0003", Kind::Issue, "todo");
        member.epic = Some("argos-0002".into());
        let loose = make("argos-0004", Kind::Issue, "in_progress");
        let mut a = App::new(vec![stone, epic, member, loose], Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());

        let still = render(&mut a, 120, 16).join("\n");
        assert!(!a.spun, "안 도는 화면으로 깬다\n{still}");
        a.hit("j");
        let basket = render(&mut a, 120, 16).join("\n");
        assert!(basket.contains("마일스톤 없음"), "바구니에 안 섰다\n{basket}");
        assert!(a.spun, "상세의 도는 건수를 안 깨운다\n{basket}");
    }

    /// **목록과 상세가 묶음의 읽은 칸을 그린다** — CLI 와 같은 자. 손으로 옮긴
    /// 칸이 다르면 상세가 낱말로 말한다. **흔한 폭에서 본다** — 그 말을 머리 줄에
    /// 이어 붙이면 80~160칸에서 통째로 잘려, 옮긴 사람이 까닭을 못 본다.
    #[test]
    fn a_grouping_is_drawn_in_the_column_its_members_read() {
        let mut issues = issues();
        issues[0].status = Status::new("done");
        // 집은 멤버를 내려놓는다 — 끝난 것 하나와 첫 칸 하나로도 에픽은 `in_progress` 로
        // 읽히고, 그때는 안 도므로 정지 글리프 `▸` 로 읽은 칸을 확인할 수 있다.
        issues[2].status = Status::new("todo");
        let mut a = every(issues);
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
        let mut a = every(issues);
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let drawn = render(&mut a, 100, 22).join("\n");
        assert!(!drawn.contains("**"), "굵게 기호가 남았다\n{drawn}");
        assert!(drawn.contains('•'), "목록 글머리가 없다\n{drawn}");

        // SPC t r 로 원문을 본다 — 그린 글은 기호가 지워져 되돌릴 수 없다
        a.hit("SPC t r");
        let raw = render(&mut a, 100, 22).join("\n");
        assert!(raw.contains("**굵게**"), "원문이 아니다\n{raw}");
        assert!(raw.contains("- 하나"), "원문이 아니다\n{raw}");
    }

    /// **메뉴로 가는 길은 80칸에서도 보인다.** 바는 접히지 않고 위젯이 말없이 잘라 내므로 늘
    /// 남는 묶음의 맨 끝에 둔다 — 끝내기·거름망·담기·원문이 모두 그 뒤에 있다(moai-7sjm).
    #[test]
    fn the_key_bar_still_names_the_menu_at_eighty_columns() {
        for w in [80u16, 100, 120] {
            let mut a = app();
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.trim_end().ends_with("SPC 메뉴"), "{w}칸에서 메뉴 키가 잘렸다 — {bar:?}");
            a.filter_text = Some("tag=x".into());
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.contains("Esc 풀기") && bar.trim_end().ends_with("SPC 메뉴"), "{w}칸 — {bar:?}");
            // 옮긴 키는 바에 없다.
            // `h·l`·`Ctrl-C` 는 바에 안 세운다(moai-k3yi) — 도움말이 댄다.
            for gone in ["F10", "F5", "F3", "F7", "끝내기", "n 담기", "f 거름망", "w 워크트리", "Ctrl-C", "h·", "·l"] {
                assert!(!bar.contains(gone), "{w}칸 바에 옮긴 키 `{gone}` 가 남았다 — {bar:?}");
            }
        }
    }

    /// 층을 그림 시험용으로 세운다 — 연 것 하나, init 전 하나, 사라진 것 하나.
    fn layered(at: super::super::layer::At) -> App {
        use super::super::layer::{Look, Picked, Shut, Summary};
        let open = Look::Open {
            sum: Summary {
                counts: vec![("todo".into(), 3), ("in_progress".into(), 1), ("review".into(), 0), ("done".into(), 12)],
                picked: vec![Picked { id: "argos-0004".into(), title: "집은 멤버".into(), column: "in_progress".into() }],
                warnings: 2,
                unreadable: 0,
            },
        };
        let bare = Look::Shut { state: Shut::Uninit, said: "· init 전 — `moai -C /w/bare init` 으로 시작하면 여기 보인다".into() };
        let gone = Look::Shut { state: Shut::Missing, said: "! 디렉터리가 없다  → 옮겼으면 새 자리를 등록하고".into() };
        let mut a = app();
        a.layer = Some(super::super::layer::fake(vec![("one", "/w/one", open), ("bare", "/w/bare", bare), ("gone", "/w/gone", gone)], at));
        a
    }

    /// **층도 색 없이 80칸에서 읽힌다** — 어디인지(경로 줄), 프로젝트마다 이름·들어갈 수 있는지
    /// (`/`)·칸 글리프와 수·못 여는 까닭, 오른쪽에 칸 이름별 수와 집은 것. 아래 줄은 층에서
    /// 듣는 키만 적는다 — 거름망·나가기는 층에서 까닭만 말하고, `n` 은 커서의 프로젝트에 담는다.
    #[test]
    fn the_project_layer_reads_without_colour_at_eighty_columns() {
        use super::super::layer::At;
        let mut a = layered(At::Layer);
        a.issues.clear();
        a.index = crate::nav::Index::of(&[]);
        a.keep.clear();
        a.warnings = 0;
        let lines = render(&mut a, 80, 22);
        let screen = lines.join("\n");
        assert!(lines[0].starts_with("프로젝트 층"), "{:?}", lines[0]);
        assert!(screen.contains("프로젝트 3곳"), "{screen}");
        let row = lines.iter().find(|l| l.contains("one/")).unwrap_or_else(|| panic!("{screen}"));
        assert!(row.contains("·3") && row.contains("▸1") && row.contains("✓12") && !row.contains("?0"), "{row:?}");
        assert!(lines.iter().any(|l| l.contains("bare") && !l.contains("bare/") && l.contains("init 전")), "{screen}");
        assert!(lines.iter().any(|l| l.contains("gone") && l.contains("디렉터리가 없다")), "{screen}");
        assert!(screen.contains("in_progress 1") && screen.contains("집은 것 1건") && screen.contains("집은 멤버"), "{screen}");
        assert!(screen.contains("드러난 것 2건"), "{screen}");
        let bar = lines.last().unwrap();
        assert!(bar.contains("Enter 들어가기") && bar.contains("SPC 메뉴"), "{bar:?}");
        for absent in ["거름망", "검색", "Bksp", "워크트리", "F3", "F10"] {
            assert!(!bar.contains(absent), "층에서 안 듣는 키를 적었다 — {absent} in {bar:?}");
        }
        // 옆 워크트리가 없으면 겹쳐 보기가 켜져 있어도 뱃지가 안 선다(moai-d5vn).
        assert!(a.worktree && !lines[0].contains('⎇'), "{:?}", lines[0]);
        // 옆이 있으면 층에서도 뱃지가 서지만, `SPC t w` 는 층의 메뉴에 안 서므로 끄는 법을 대지 않는다.
        let (_, origin) =
            crate::worktree::overlay(Vec::new(), vec![crate::worktree::Side::new("feat/x", "/wt", vec![issues()[2].clone()])]);
        let plain = std::mem::replace(&mut a.origin, origin);
        let top = render(&mut a, 80, 22).remove(0);
        assert!(top.contains("⎇ feat/x"), "{top:?}");
        assert!(!top.contains("로 끈다"), "층에서 안 듣는 끄는 키를 댄다 — {top:?}");
        a.origin = plain;
        for l in &lines {
            assert!(crate::text::width(l) <= 80, "넘쳤다: {l:?}");
        }
        // 층의 메뉴에는 담기·등록·해제가 서고 거름망·검색·워크트리는 안 선다.
        a.hit("SPC");
        let screen = render(&mut a, 80, 22).join("\n");
        assert!(screen.contains("n : 생각 담기") && screen.contains("p : +프로젝트"), "{screen}");
        assert!(!screen.contains("f : 거름망") && !screen.contains("/ : 검색"), "{screen}");
        a.hit("p");
        let screen = render(&mut a, 80, 22).join("\n");
        assert!(screen.contains("a : 등록") && screen.contains("d : 목록에서 빼기"), "{screen}");
        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        // `SPC p d` 는 목록 포커스에서만 듣는다 — 상세 포커스의 메뉴에는 없다. `a` 는 남는다.
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let bar = render(&mut a, 80, 22).last().cloned().unwrap_or_default();
        assert!(!bar.contains("Enter"), "{bar:?}");
        a.hit("SPC p");
        let screen = render(&mut a, 80, 22).join("\n");
        assert!(screen.contains("a : 등록") && !screen.contains("목록에서 빼기"), "{screen}");
    }

    /// **디렉터리 고르기 창도 색 없이 80칸에서 읽힌다** — 테두리가 지금 디렉터리를 대고(길면
    /// 앞을 자른다), 줄마다 `./`·`..`·`이름/` 에 `.moai`·`✓ 등록됨` 이 낱말로 붙는다. 감춘 수와
    /// 잘린 수가 아래 테두리에, 등록·닫기가 아랫줄에 늘 선다. 굵은 칸은 창 하나다.
    #[test]
    fn the_directory_picker_reads_without_colour_at_eighty_columns() {
        use super::super::layer::At;
        use super::super::picker::{Dent, Listing};
        let mut a = layered(At::Layer);
        let dir = "/home/raven/아주/깊은/작업/디렉터리/여러/겹/mono";
        let at = Listing {
            dir: dir.into(),
            entries: vec![
                Dent { name: "apps".into(), moai: false, registered: false },
                Dent { name: "argos".into(), moai: true, registered: true },
                Dent { name: "bare".into(), moai: false, registered: true },
            ],
            cut: 7,
            hidden: 2,
            moai: true,
            registered: false,
        };
        a.mode = Mode::Pick(Picker::new(at));
        let lines = render(&mut a, 80, 16);
        let screen = lines.join("\n");
        assert!(lines.iter().any(|l| l.contains("프로젝트 등록") && l.contains("mono")), "{screen}");
        assert!(lines.iter().any(|l| l.contains("./") && l.contains("이 디렉터리") && l.contains(".moai")), "{screen}");
        assert!(lines.iter().any(|l| l.contains("..") && l.contains("위로")), "{screen}");
        assert!(lines.iter().any(|l| l.contains("> apps/")), "커서가 첫 하위 디렉터리에 안 섰다\n{screen}");
        assert!(lines.iter().any(|l| l.contains("argos/") && l.contains(".moai") && l.contains("✓ 등록됨")), "{screen}");
        assert!(lines.iter().any(|l| l.contains("bare/") && !l.contains(".moai") && l.contains("✓ 등록됨")), "{screen}");
        assert!(screen.contains("숨은 것 2개") && screen.contains("그 밖 7개"), "{screen}");
        assert_eq!(lines.iter().filter(|l| l.contains('┏')).count(), 1, "창이 뒤 칸을 다 못 덮었다\n{screen}");
        let bar = lines.last().unwrap().clone();
        for hint in ["Enter 들어가기", "Bksp 위로", "g p 경로 적기", "a 등록", "Esc 닫기"] {
            assert!(bar.contains(hint), "80칸에서 `{hint}` 가 없다 — {bar:?}");
        }
        // `g` 를 누르면 무엇을 기다리는지 댄다(moai-k3yi). 뜻 없는 키가 열을 버리면 바가 돌아온다.
        a.key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        let waiting = render(&mut a, 80, 16).last().cloned().unwrap_or_default();
        assert_eq!(waiting, " g → g 맨 위 · p 경로 적기");
        a.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(matches!(a.mode, Mode::Pick(_)), "뜻 없는 키가 창을 닫았다");
        assert_eq!(render(&mut a, 80, 16).last(), Some(&bar), "창의 바가 안 돌아왔다");
        for l in &lines {
            assert!(crate::text::width(l) <= 80, "넘쳤다: {l:?}");
        }
        // 좁고 낮아도 무너지지 않는다
        for (w, h) in [(20u16, 5u16), (40, 8), (1, 1)] {
            for l in render(&mut a, w, h) {
                assert!(crate::text::width(&l) <= w as usize, "{w}x{h}: {l:?}");
            }
        }

        // 해제를 묻는 줄 — 무엇을 빼는지, y, 디렉터리는 그대로라는 것.
        a.mode = Mode::Unregister(super::super::register::Unregister { path: "/w/one".into(), name: "one".into() });
        let bar = render(&mut a, 80, 16).last().cloned().unwrap_or_default();
        assert!(bar.contains("one 을 목록에서 뺄까") && bar.contains("y 뺀다"), "{bar:?}");
        let wide = render(&mut a, 120, 16).last().cloned().unwrap_or_default();
        assert!(wide.contains("디렉터리와 .moai 는 그대로다"), "{wide:?}");
    }

    /// **프로젝트 안에서는 경로 줄이 늘 어느 프로젝트인지 댄다.** 뿌리에는 층으로 가는 `..`
    /// 이 서고 오른쪽이 그곳이 어디인지 말한다. 깊이 들어가 경로가 잘려도 이름은 남는다.
    #[test]
    fn inside_a_project_the_path_line_names_it_and_the_root_climbs_to_the_layer() {
        use super::super::layer::At;
        let mut a = layered(At::Project("/w/one".into()));
        a.cursor = 0;
        let lines = render(&mut a, 80, 12);
        assert!(lines[0].starts_with("one:/"), "{:?}", lines[0]);
        let screen = lines.join("\n");
        assert!(screen.contains("..") && screen.contains("프로젝트 층으로"), "{screen}");

        a.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(a.path.len(), 1);
        let lines = render(&mut a, 40, 12);
        assert!(lines[0].starts_with("one:/아주"), "깊이 들어가자 프로젝트 이름이 잘렸다 — {:?}", lines[0]);
    }

    /// **프로젝트 이름은 CLI 한눈 보기와 같은 색이다** — 경로 줄에서도 층의 줄에서도
    /// `style::project_colour(경로)` 를 입는다. 두 표면이 같은 프로젝트를 다른 색으로 칠하면
    /// 색으로 알아보라던 약속이 거꾸로 선다.
    #[test]
    fn a_project_name_wears_the_same_colour_as_in_the_cli_overview() {
        use super::super::layer::At;
        let one = std::path::Path::new("/w/one");
        let want = from_anstyle(style::project_colour(one, None)).fg;
        let colour_of = |app: &mut App, y: u16, text: &str| {
            let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
            term.draw(|f| screen(f, app)).unwrap();
            let buf = term.backend().buffer().clone();
            let row: String = (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect();
            let x = row.find(text).map(|b| row[..b].chars().count() as u16).unwrap_or_else(|| panic!("{text} 가 없다 — {row:?}"));
            buf[(x, y)].fg
        };
        let mut inside = layered(At::Project("/w/one".into()));
        assert_eq!(Some(colour_of(&mut inside, 0, "one")), want, "경로 줄의 이름이 한눈 보기와 다른 색이다");
        let mut on = layered(At::Layer);
        on.issues.clear();
        on.index = crate::nav::Index::of(&[]);
        on.keep.clear();
        assert_eq!(Some(colour_of(&mut on, 3, "one/")), want, "층의 줄 이름이 한눈 보기와 다른 색이다");

        // **설정에 정한 색이 해시를 이긴다** — 해시가 고른 것과 다른 색을 골라 두 자리가 따라오는지 본다.
        let other = style::Hue::names().iter().filter_map(|n| style::Hue::named(n)).find(|h| *h != style::Hue::of_path(one)).unwrap();
        let chosen = from_anstyle(style::project_colour(one, Some(other))).fg;
        assert_ne!(chosen, want);
        for app in [&mut inside, &mut on] {
            app.layer.as_mut().unwrap().places.iter_mut().filter(|p| p.path == one).for_each(|p| p.hue = Some(other));
        }
        assert_eq!(Some(colour_of(&mut inside, 0, "one")), chosen, "경로 줄이 정한 색을 안 입었다");
        assert_eq!(Some(colour_of(&mut on, 3, "one/")), chosen, "층의 줄이 정한 색을 안 입었다");
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let first = render(&mut a, 100, 16).join("\n");
        assert!(first.contains("1번째"), "{first}");
        assert!(!first.contains("40번째"), "다 보이면 굴릴 것이 없다\n{first}");
        // 목록에 섰으니 상세로 가는 키를 댄다 — `j·k` 는 여기서 커서를 옮긴다(moai-ob4c).
        assert!(first.contains("↓ ") && first.contains("줄 (Tab)"), "남은 줄을 안 알린다\n{first}");
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let focused = render(&mut a, 100, 16).join("\n");
        assert!(focused.contains("줄 (j·k)"), "상세 포커스에서 굴리는 키를 안 댄다\n{focused}");

        for _ in 0..12 {
            a.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
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
    /// 표시도 없이 잘라 낸다 — 태그 줄·롤업의 칸별 건수·`SPC t r` 원문·접지 않기로
    /// 한 코드 줄이 그 길로 조용히 꼬리를 잃었다. 이 시험은 버퍼 폭이 아니라
    /// **잘린 자리에 표시가 있는지**를 본다.
    #[test]
    fn a_line_too_wide_for_the_detail_pane_says_it_was_cut() {
        let mut issues = issues();
        issues[1].tags =
            vec!["parser".into(), "storage".into(), "renderer".into(), "markdown".into()];
        issues[1].body = Some("```\nlet very_long = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\";\n```\n".into());
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        for raw in [false, true] {
            if raw {
                a.hit("SPC t r");
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        for _ in 0..80 {
            a.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let _ = render(&mut a, 100, 16);
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(a.detail.offset() > 0);
        a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
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
        let mut a = every(issues);
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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

    /// **키 바가 `Tab` 을 말한다. 80칸에서도.** 모르는 키는 없는 키다 — 그리고
    /// 가는 곳을 대므로 지금 어디 있는지를 글자로도 말한다.
    #[test]
    fn the_key_bar_names_tab_at_eighty_columns() {
        for w in [80u16, 100, 120] {
            let mut a = app();
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.contains("Tab 상세") && bar.contains("SPC 메뉴"), "{w}칸 — {bar:?}");
            a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            assert!(bar.contains("Tab 목록") && bar.contains("SPC 메뉴"), "{w}칸 — {bar:?}");
        }
    }

    /// **키 바는 포커스 칸에서 듣는 키만 적는다.** 적힌 목록을 손으로 다시 적지 않고
    /// 키를 **실제로 눌러** 잰다 — 누르고 화면이 그대로면 그 칸에서 안 듣는 키고,
    /// 그런 키는 바에 없어야 한다. 반대로 듣는 키는 바에 있어야 한다. `App::key` 가
    /// 드나드는 키를 어느 칸에 태우든, `fkeys` 가 따로 따라가지 않으면 여기서 갈린다.
    ///
    /// 드나들 데가 있는 자리와 없는 자리를 다 누른다(moai-k3yi) — 뿌리의 에픽·잎, 에픽 안의
    /// 잎·`..`, 층이 있는 프로젝트 뿌리. 잎의 Enter 와 층 없는 뿌리의 Bksp 는 아무 일도 없고
    /// 바에도 없어야 한다.
    #[test]
    fn the_key_bar_names_only_keys_that_act_in_the_focused_pane() {
        let press = |k: KeyCode| KeyEvent::new(k, KeyModifiers::NONE);
        for (hint, code) in [("Enter 들어가기", KeyCode::Enter), ("Bksp 나가기", KeyCode::Backspace)] {
            for place in Place::ALL {
                for pane in Pane::ALL {
                    let mut a = place.app();
                    a.focus = pane;
                    let before = render(&mut a, 120, 14);
                    a.key(press(code));
                    let acts = render(&mut a, 120, 14) != before;
                    let bar = before.last().cloned().unwrap_or_default();
                    assert_eq!(bar.contains(hint), acts, "{place:?}·{pane:?} 에서 {code:?} 가 듣는가 {acts} — 바 {bar:?}");
                }
            }
        }
    }

    /// 커서를 세우는 자리 — 드나드는 키가 듣고 안 듣는 곳을 고루.
    #[derive(Debug, Clone, Copy)]
    enum Place {
        /// 프로젝트 뿌리(층 없음), 커서가 에픽(디렉터리) 위.
        RootDir,
        /// 프로젝트 뿌리(층 없음), 커서가 잎 위.
        RootLeaf,
        /// 에픽 안, 커서가 잎 위.
        InsideLeaf,
        /// 에픽 안, 커서가 `..` 위.
        InsideUp,
        /// 층이 있는 프로젝트 뿌리, 커서가 잎 위 — Bksp 가 층으로 올라간다.
        LayeredRootLeaf,
        /// 층이 있는 프로젝트의 에픽 안, 커서가 `..` 위 — 드나드는 키가 둘 다 듣는다.
        LayeredInsideUp,
    }

    impl Place {
        const ALL: [Place; 6] =
            [Place::RootDir, Place::RootLeaf, Place::InsideLeaf, Place::InsideUp, Place::LayeredRootLeaf, Place::LayeredInsideUp];

        /// 그 자리에 목록 포커스로 선 앱. 커서는 줄의 **종류**로 찾는다 — 번호로 박으면 fixture 의
        /// 차례가 바뀐 날 엉뚱한 줄에서 잰다.
        fn app(self) -> App {
            use super::super::layer::At;
            let mut a = match self {
                Place::LayeredRootLeaf | Place::LayeredInsideUp => layered(At::Project("/w/one".into())),
                _ => app(),
            };
            // fixture 의 뿌리에는 에픽뿐이다 — 뿌리의 잎 하나를 더한다.
            let mut all = a.issues.clone();
            all.push(Issue::new("argos-0009".into(), "홀로 선 일".into(), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z"));
            a.adopt(all);
            let find = |a: &App, want: &dyn Fn(&Row) -> bool| a.rows().iter().position(want).expect("그런 줄이 없다");
            let dir = |r: &Row| matches!(r, Row::Item(Entry::Dir { .. }));
            a.focus = Pane::Explorer;
            if matches!(self, Place::InsideLeaf | Place::InsideUp | Place::LayeredInsideUp) {
                a.cursor = find(&a, &dir);
                a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                assert!(!a.path.is_empty(), "{self:?}: 에픽에 못 들어갔다");
            }
            a.cursor = match self {
                Place::RootDir => find(&a, &dir),
                Place::InsideUp | Place::LayeredInsideUp => find(&a, &|r| *r == Row::Up),
                _ => find(&a, &|r| matches!(r, Row::Item(Entry::Leaf { .. }))),
            };
            a
        }
    }

    /// **커서가 잎이면 Enter, 층 없는 뿌리면 Bksp 가 바에서 빠지고 그 키는 조용히 아무 일도 안
    /// 한다**(moai-k3yi, moai-uowi 흡수). 층이 있는 프로젝트 뿌리의 Bksp 는 층으로 올라가므로 선다.
    #[test]
    fn the_key_bar_follows_the_row_under_the_cursor() {
        let bar_at = |a: &mut App| render(a, 80, 14).last().cloned().unwrap_or_default();

        let mut a = Place::RootLeaf.app();
        let bar = bar_at(&mut a);
        assert!(!bar.contains("Enter") && !bar.contains("Bksp"), "잎·층 없는 뿌리 — {bar:?}");
        let (cursor, path) = (a.cursor, a.path.clone());
        a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        a.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!((a.cursor, &a.path, &a.notice), (cursor, &path, &None), "잎의 Enter·뿌리의 Bksp 가 무언가 했다");

        let mut a = Place::RootDir.app();
        let bar = bar_at(&mut a);
        assert!(bar.contains("Enter 들어가기") && !bar.contains("Bksp"), "에픽 위·층 없는 뿌리 — {bar:?}");

        let mut a = Place::LayeredRootLeaf.app();
        let bar = bar_at(&mut a);
        assert!(bar.contains("Bksp 나가기") && !bar.contains("Enter"), "층이 있는 뿌리의 잎 — {bar:?}");

        let mut a = Place::InsideUp.app();
        let bar = bar_at(&mut a);
        assert!(bar.contains("Bksp 나가기") && bar.contains("Enter 들어가기"), "에픽 안의 `..` — {bar:?}");
    }

    /// **커서가 옮겨 가도 바의 칸은 제자리다** — 빠지는 것은 그 자리에서 안 듣는 Enter·Bksp 뿐이고
    /// 나머지는 차례도 글자도 그대로다. 80칸 거름망 켠 목록에서 바를 **통째로** 견준다: 늘 서는
    /// 칸이 밀리거나 떨어지면 여기서 갈린다.
    #[test]
    fn the_key_bar_keeps_its_order_as_the_cursor_moves_at_eighty_columns() {
        for place in Place::ALL {
            let mut a = place.app();
            a.filter_text = Some("tag=x".into());
            for at in 0..a.rows().len() {
                a.cursor = at;
                let c = a.key_ctx(&a.rows());
                // 앞 묶음은 뒤에서부터 놓인다([`bar`]) — 떨어지는 차례의 거꾸로가 화면의 차례다.
                let want: String = [
                    (!c.leaf).then_some("Enter 들어가기"),
                    (!c.root).then_some("Bksp 나가기"),
                    Some("/ 검색"),
                    Some("Tab 상세"),
                    Some("j·k 이동"),
                    Some("Esc 풀기"),
                    Some("SPC 메뉴"),
                ]
                .into_iter()
                .flatten()
                .map(|h| format!(" {h}"))
                .collect();
                let bar = render(&mut a, 80, 14).last().cloned().unwrap_or_default();
                assert_eq!(bar, want, "{place:?} 줄 {at} ({c:?})");
            }
        }
    }

    /// **`g` 를 누르고 기다리는 동안 바가 그것을 말한다**(moai-k3yi) — 메뉴가 서는 자리에서, 이어
    /// 누를 키와 낱말을 표에서 읽어. 다음 키가 동작이든(`gg`) 뜻이 없든(`g j`) 열이 비면 바가
    /// 돌아온다. 메뉴 창은 안 뜬다 — `g` 는 메뉴가 아니다.
    #[test]
    fn a_pending_g_names_what_it_waits_for_and_clears() {
        let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        for pane in Pane::ALL {
            let mut a = app();
            a.focus = pane;
            let before = render(&mut a, 80, 14);
            a.key(g);
            let lines = render(&mut a, 80, 14);
            assert_eq!(lines.last().map(String::as_str), Some(" g → g 맨 위"), "{pane:?}");
            assert!(!lines.iter().any(|l| l.contains("┌ SPC")), "`g` 가 메뉴 창을 띄웠다");
            a.key(g);
            assert_eq!(render(&mut a, 80, 14).last(), before.last(), "{pane:?}: `gg` 뒤에 바가 안 돌아왔다");
            a.key(g);
            a.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
            assert_eq!(render(&mut a, 80, 14).last(), before.last(), "{pane:?}: 뜻 없는 키 뒤에 바가 안 돌아왔다");
        }
    }

    /// **80칸에서 바의 키가 하나도 안 떨어진다** — 옮긴 키가 빠져 자리가 났다(moai-7sjm). 상세
    /// 포커스에서는 드나드는 키가 빠진다.
    ///
    /// 드나드는 키가 둘 다 듣는 자리(층이 있는 프로젝트의 에픽 안 `..`)에서 잰다 — 바가 가장 긴 곳이다.
    #[test]
    fn the_key_bar_fits_whole_at_eighty_columns() {
        for w in [80u16, 100, 120] {
            let mut a = Place::LayeredInsideUp.app();
            a.filter_text = Some("tag=x".into());
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            for shown in ["j·k 이동", "Tab 상세", "/ 검색", "Bksp 나가기", "Enter 들어가기", "Esc 풀기", "SPC 메뉴"] {
                assert!(bar.contains(shown), "{w}칸 목록 포커스에 {shown:?} 가 없다 — {bar:?}");
            }
            a.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            let bar = render(&mut a, w, 14).last().cloned().unwrap_or_default();
            for shown in ["j·k 굴리기", "Tab 목록", "/ 검색", "SPC 메뉴"] {
                assert!(bar.contains(shown), "{w}칸 상세 포커스에 {shown:?} 가 없다 — {bar:?}");
            }
            assert!(!bar.contains("Enter") && !bar.contains("Bksp"), "{w}칸 — {bar:?}");
        }
    }

    /// **탐색 바에 적힌 키는 표에 있고, 그 자리에서 켜진 동작이다**(moai-nc7w). 층·프로젝트 안,
    /// 목록·상세 포커스, 거름망, 토글을 다 돌려 바에 선 이름을 키로 풀고 표에서 찾는다 — 표에서
    /// 키를 옮기고 바의 차례만 남겨 두면 여기서 갈린다. 낱말도 표가 낸 그대로다.
    #[test]
    fn every_key_on_the_browse_bar_is_an_enabled_row_of_the_table() {
        use super::super::keys::{Lookup, lookup, parse};
        use super::super::layer::At;
        // 층, 그리고 프로젝트 안에서 커서가 선 줄(잎·디렉터리·`..` × 층 없는 뿌리·층 있는 뿌리·에픽 안).
        let mut seen = std::collections::HashSet::new();
        for place in [None].into_iter().chain(Place::ALL.map(Some)) {
            for pane in Pane::ALL {
                for on in [false, true] {
                    let mut a = place.map_or_else(|| layered(At::Layer), Place::app);
                    a.focus = pane;
                    a.filter_text = on.then(|| "tag=x".to_string());
                    (a.worktree, a.raw) = (on, on);
                    let c = a.key_ctx(&a.rows());
                    seen.insert((c.list_focus, c.leaf, c.root));
                    let (optional, keep) = browse_hints(&a, &c);
                    assert!(keep.last().is_some_and(|(k, w)| k == "SPC" && *w == "메뉴"), "{c:?}: 메뉴로 가는 길이 늘 남지 않는다");
                    assert_eq!(lookup(BROWSE, &[parse("SPC").unwrap()]), Lookup::Pending, "SPC 가 메뉴를 안 연다");
                    for (names, what) in optional.iter().chain(&keep[..keep.len() - 1]) {
                        for name in names.split('·') {
                            let k = parse(name).unwrap_or_else(|| panic!("{c:?}: 바의 `{name}` 를 키로 못 푼다"));
                            let Lookup::Run(act) = lookup(BROWSE, &[k]) else { panic!("{c:?}: 바의 `{name}` 가 표에 없다") };
                            assert_eq!(act.enabled(&c), Ok(()), "{c:?}: 켜지지 않은 `{name}` 가 바에 섰다");
                            assert_eq!(act.what(&c), *what, "{c:?}: `{name}`");
                        }
                    }
                }
            }
        }
        // 조합이 커서의 사실을 정말 고루 돌았는가 — 목록 포커스에서 잎·뿌리가 켜지고 꺼진 네 가지.
        for leaf in [false, true] {
            for root in [false, true] {
                assert!(seen.contains(&(true, leaf, root)), "잎 {leaf}·뿌리 {root} 를 안 돌았다 — {seen:?}");
            }
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
        assert!(!lines.join("\n").contains("SPC 메뉴"), "묻는 동안 탐색 바가 섰다");

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
        a.hit("SPC n");
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
        for hint in ["Ctrl-S 담기", "Tab 본문", "Enter 본문으로", "Esc 닫기"] {
            assert!(bar.contains(hint), "80칸에서 `{hint}` 가 없다 — {bar:?}");
        }
        assert!(!screen.contains("SPC 메뉴"), "폼이 열렸는데 탐색 바가 섰다\n{screen}");

        press(&mut a, KeyCode::Tab);
        let lines = render(&mut a, 80, 24);
        let t = thick(&lines);
        assert!(t.len() == 1 && t[0].contains("본문"), "Tab 뒤에 굵은 선이 본문으로 안 갔다\n{}", lines.join("\n"));
        let bar = lines.last().unwrap();
        assert!(bar.contains("Tab 제목") && bar.contains("Enter 줄 나누기"), "{bar:?}");
    }

    /// **폼 머리가 어느 프로젝트에 담기는지 댄다** — 색 없이도 `담을 곳  이름  경로` 가 80칸에
    /// 들고, 이름은 경로 줄·층과 같은 프로젝트 색을 입는다. 담을 곳 없이 세운 화면(저장소 없음)
    /// 에는 머리가 안 선다.
    #[test]
    fn the_idea_form_names_the_project_it_saves_into() {
        use super::super::layer::At;
        let mut a = layered(At::Project("/w/one".into()));
        a.hit("SPC n");
        typed(&mut a, "떠오른 것");
        let lines = render(&mut a, 80, 24);
        let shown = lines.join("\n");
        let (y, head) = lines.iter().enumerate().find(|(_, l)| l.contains("담을 곳")).unwrap_or_else(|| panic!("머리가 없다\n{shown}"));
        assert!(head.contains("담을 곳  one  /w/one"), "{head:?}");
        let title = lines.iter().position(|l| l.contains("제목")).unwrap();
        assert!(y < title, "머리가 제목 칸 아래에 섰다\n{shown}");
        for l in &lines {
            assert!(crate::text::width(l) <= 80, "넘쳤다: {l:?}");
        }

        let want = from_anstyle(style::project_colour(std::path::Path::new("/w/one"), None)).fg;
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let row: String = (0..80).map(|x| buf[(x, y as u16)].symbol().to_string()).collect();
        let x = row.find("one").map(|b| row[..b].chars().count() as u16).unwrap();
        assert_eq!(Some(buf[(x, y as u16)].fg), want, "머리의 이름이 경로 줄과 다른 색이다");

        // **설정에 정한 색(moai-o04b)도 따라온다** — 폼의 담을 곳은 층의 줄을 안 들고 경로만
        // 드므로, 층에서 같은 경로의 정한 색을 찾아 입는다. 해시와 다른 색을 골라 본다.
        let one = std::path::Path::new("/w/one");
        let other = style::Hue::names().iter().filter_map(|n| style::Hue::named(n)).find(|h| *h != style::Hue::of_path(one)).unwrap();
        a.layer.as_mut().unwrap().places.iter_mut().filter(|p| p.path == one).for_each(|p| p.hue = Some(other));
        let chosen = from_anstyle(style::project_colour(one, Some(other))).fg;
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        assert_eq!(Some(buf[(x, y as u16)].fg), chosen, "머리의 이름이 설정에 정한 색을 안 입었다");

        // 좁고 긴 경로에서도 이름이 남는다
        let lines = render(&mut a, 24, 12);
        assert!(lines.iter().any(|l| l.contains("담을 곳  one")), "{lines:?}");

        // 낮은 창에서는 머리가 제목 칸 테두리로 접힌다 — 본문 칸이 테두리만 남지 않고,
        // 담을 곳은 빠지지 않는다.
        let lines = render(&mut a, 80, 8);
        let shown = lines.join("\n");
        assert!(!lines.iter().any(|l| l.contains("담을 곳  one")), "낮은 창에 머리 줄이 따로 섰다\n{shown}");
        assert!(lines.iter().any(|l| l.contains("담을 곳 one · 제목")), "낮은 창에서 담을 곳이 빠졌다\n{shown}");
        assert!(lines.iter().any(|l| l.contains("본문")), "{shown}");

        let mut bare = app();
        bare.hit("SPC n");
        assert!(!render(&mut bare, 80, 24).join("\n").contains("담을 곳"), "담을 곳 없는 폼에 머리가 섰다");
    }

    /// 굵은 선은 **초록이기도 하다** — 목록·상세의 포커스와 같은 색이다. 모양만 보는
    /// 시험으로는 색이 빠져도 조용히 지나간다.
    #[test]
    fn the_focused_form_field_is_green() {
        let mut a = app();
        a.hit("SPC n");
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
        a.hit("SPC n");
        a.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
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
        a.hit("SPC n");
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
        a.hit("SPC n");
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
        a.hit("SPC n");
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

    /// **SPC 메뉴는 색 없이 80칸에서 읽힌다**(moai-7sjm, 모양은 moai-apsa). 아래 전체 폭에 가름줄과
    /// `키 : 낱말` 격자가 서고, 맨 아랫줄은 접두어 줄(`SPC- 메뉴`)이 되어 나가는 법을 오른쪽 끝에
    /// 댄다. 묶음은 `+`, 토글은 상태를 낱말로 단다. 하위 층이면 `SPC t- 토글` 과 Bksp 가 선다.
    /// 동작을 실행하면 창이 걷히고 바가 돌아온다.
    #[test]
    fn the_menu_reads_without_colour_at_eighty_columns() {
        let mut a = app();
        let before = render(&mut a, 80, 20);
        a.hit("SPC");
        let lines = render(&mut a, 80, 20);
        let screen = lines.join("\n");
        for row in ["/ : 검색", "f : 거름망", "n : 생각 담기", "r : 다시 읽기", "q : 끝내기", "p : +프로젝트", "t : +토글"] {
            assert!(screen.contains(row), "{row:?} 가 없다\n{screen}");
        }
        // 뿌리 일곱 칸은 6칸 한 열과 옆 열 1칸으로 선다(moai-r2dt) — 가름줄 · 격자 6줄 · 접두어 줄.
        let n = lines.len();
        assert_eq!(lines[n - 8], "─".repeat(80), "전체 폭 가름줄이 아니다\n{screen}");
        assert!(lines[n - 9].starts_with(['└', '┗']), "몸통이 창 위로 밀려 올라가지 않았다\n{screen}");
        assert!(lines[n - 7].contains("/ : 검색") && lines[n - 7].contains("t : +토글"), "일곱째 칸이 옆 열로 안 넘어갔다\n{screen}");
        let bar = &lines[n - 1];
        assert!(bar.starts_with("SPC- 메뉴") && bar.ends_with("Esc 닫기"), "{bar:?}");
        assert!(!bar.contains("Bksp") && !bar.contains("SPC 메뉴"), "{bar:?}");
        for l in &lines {
            assert!(crate::text::width(l) <= 80, "넘쳤다: {l:?}");
        }

        a.hit("t");
        let lines = render(&mut a, 80, 20);
        let screen = lines.join("\n");
        let bar = lines.last().unwrap();
        assert!(bar.starts_with("SPC t- 토글") && bar.ends_with("Esc 닫기 Bksp 위로"), "{bar:?}");
        assert!(screen.contains("w : 워크트리 겹쳐 보기 [켜짐]") && screen.contains("r : 원문↔그리기 [그리기]"), "{screen}");
        assert!(!screen.contains("q : 끝내기"), "하위 층에 뿌리가 남았다\n{screen}");
        assert_eq!(lines[lines.len() - 4], "─".repeat(80), "하위 층의 창이 제 높이로 줄지 않았다\n{screen}");

        a.hit("r");
        assert!(a.raw);
        let lines = render(&mut a, 80, 20);
        assert!(!lines.iter().any(|l| *l == "─".repeat(80)), "실행했는데 창이 남았다");
        assert_eq!(lines.last(), before.last(), "실행한 뒤 바가 돌아오지 않았다");
    }

    /// **메뉴 칸은 키·`:`·실행 낱말·`+묶음` 이 제 색을 입는다**(moai-r2dt). 뜻은 글자가 지므로 색은
    /// 덧칠이다 — 여기서는 칠한 자리만 본다.
    #[test]
    fn the_menu_paints_keys_colons_actions_and_groups_apart() {
        let mut a = app();
        a.hit("SPC");
        let (w, h) = (80u16, 20u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let buf = term.backend().buffer().clone();
        let text = render(&mut a, w, h);
        let y = text.iter().rposition(|l| l.contains("/ : 검색")).expect("메뉴 칸이 없다") as u16;
        let at = |s: &str| (0..w).find(|&x| buf[(x, y)].symbol() == s).unwrap_or_else(|| panic!("{s:?} 가 없다"));
        assert_eq!(buf[(at("/"), y)].fg, MENU_KEY);
        assert!(buf[(at("/"), y)].modifier.contains(Modifier::BOLD), "키가 굵지 않다");
        assert_eq!(buf[(at(":"), y)].fg, MENU_SEP);
        assert_eq!(buf[(at("검"), y)].fg, MENU_RUN);
        assert_eq!(buf[(at("+"), y)].fg, MENU_GROUP);
        assert_eq!(buf[(at("토"), y)].fg, MENU_GROUP);
    }

    /// **메뉴 창은 어느 폭에서도 `:` 가 줄 서고, 몸통을 밀어 올려도 커서를 잃지 않는다**(moai-apsa).
    /// 커서를 맨 끝에 두고 창을 연다 — 창이 몸통을 덮으면 커서가 선 줄이 그 밑에 숨는다.
    #[test]
    fn the_menu_panel_lines_up_and_keeps_the_cursor_in_sight_at_any_width() {
        for w in [80u16, 120, 200] {
            let mut a = app();
            // 에픽 안으로 들어가 줄이 몸통보다 많은 목록에서 맨 끝 줄에 선다.
            a.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            a.hit("G");
            let last = a.cursor;
            assert!(last > 0, "줄이 하나뿐인 목록이라 커서를 가릴 수 없다");
            a.hit("SPC");
            let lines = render(&mut a, w, 16);
            let screen = lines.join("\n");
            let n = lines.len();
            let sep = lines.iter().rposition(|l| *l == "─".repeat(w as usize)).expect("가름줄이 없다");
            assert_eq!(a.cursor, last, "메뉴가 커서를 옮겼다");
            assert!(lines[..sep].iter().any(|l| l.starts_with("┃> ")), "{w}칸: 커서가 창에 가려졌다\n{screen}");
            let colon = |l: &str| l.find(" : ").map(|i| crate::text::width(&l[..i]));
            let at: Vec<Option<usize>> = lines[sep + 1..n - 1].iter().map(|l| colon(l)).collect();
            assert!(at.iter().all(|c| c.is_some() && *c == at[0]), "{w}칸: `:` 가 줄 서지 않는다 {at:?}\n{screen}");
            for l in &lines {
                assert!(crate::text::width(l) <= w as usize, "{w}칸 넘쳤다: {l:?}");
            }
        }
    }

    /// **낮은 창에서도 메뉴 항목이 안 사라진다** — 높이가 모자라면 줄을 줄이고 열을 늘리며, 몸통을
    /// 남기고 격자 설 높이도 없으면 접두어 줄 한 줄로 접는다. 어느 줄도 폭을 넘지 않는다.
    #[test]
    fn the_menu_folds_into_columns_or_a_line_in_a_low_window() {
        for (w, h) in [(80u16, 8u16), (80, 7), (80, 6), (80, 5), (40, 8), (30, 3), (12, 5), (200, 10)] {
            let mut a = app();
            a.hit("SPC");
            let lines = render(&mut a, w, h);
            let screen = lines.join("\n");
            for l in &lines {
                assert!(crate::text::width(l) <= w as usize, "{w}x{h} 넘쳤다: {l:?}");
            }
            if w >= 80 {
                for key in ["/ ", "f ", "n ", "q ", "t "] {
                    assert!(screen.contains(key), "{w}x{h}: {key:?} 가 안 보인다\n{screen}");
                }
            }
            if h >= 4 {
                assert!(screen.contains("SPC"), "{w}x{h}: 접두어가 없다\n{screen}");
            }
        }
    }

    /// **글칸에서는 메뉴가 안 선다** — SPC 는 글자다.
    #[test]
    fn the_menu_never_draws_over_a_text_field() {
        let mut a = app();
        a.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        a.hit("SPC");
        let screen = render(&mut a, 80, 20).join("\n");
        assert!(!screen.contains("┌ SPC"), "{screen}");
    }

    /// **찾은 글자는 밝은 파랑에 굵게 선다**(moai-yio7) — 그 범위가 보는 자리에서만. 검색 칸은
    /// 좁힌 범위를 이름표에, 걸린 수를 안내에 적는다(moai-00le).
    #[test]
    fn slash_paints_what_it_found_only_where_the_scope_looks() {
        let found_text = |a: &mut App| {
            let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
            term.draw(|f| screen(f, a)).unwrap();
            let buf = term.backend().buffer().clone();
            let mut out = String::new();
            for y in 0..buf.area.height {
                for x in 0..buf.area.width {
                    let c = &buf[(x, y)];
                    if c.fg == Color::LightBlue && c.modifier.contains(Modifier::BOLD) && c.bg != Color::LightBlue {
                        out.push_str(c.symbol());
                    }
                }
                out.push('\n');
            }
            out
        };
        let mut a = app();
        let id = a.issues[0].id.clone();
        let q = id[id.len() - 3..].to_uppercase();
        a.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in q.chars() {
            a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let prompt = render(&mut a, 100, 20).join("\n");
        let n = a.hit_count();
        assert!(prompt.contains(&format!("{n}건")), "걸린 수가 없다\n{prompt}");
        assert!(prompt.contains("Tab·Shift-Tab 범위"), "범위 키가 없다\n{prompt}");
        let painted = found_text(&mut a);
        assert!(painted.contains(&q.to_lowercase()), "찾은 글자를 안 칠했다\n{painted}");

        // 태그 범위는 id·제목을 안 본다 — 칠할 것이 없다. 이름표가 범위를 말한다.
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        a.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        let prompt = render(&mut a, 100, 20).join("\n");
        assert!(prompt.contains(" 검색·태그 "), "{prompt}");
        assert!(found_text(&mut a).trim().is_empty(), "보지 않는 자리를 칠했다");
    }

    /// 빈 저장소도 그려진다.
    #[test]
    fn an_empty_repo_still_draws() {
        let mut empty = App::new(Vec::new(), Config::parse("prefix = \"argos\"\n").unwrap(), Path::new());
        let lines = render(&mut empty, 60, 10).join("\n");
        assert!(lines.contains("비었다"), "{lines}");
    }

    /// **층의 한 줄 자리는 `text::one_line` 을 지난다**(moai-9tww) — 칸 이름은 남의
    /// `.moai/config.toml` 이, id·제목은 남의 스냅샷이, 이름·경로는 남의 디렉터리가 정한다.
    /// `sanitize` 는 줄바꿈과 탭을 남기는데, 한 줄짜리 `Line` 에 든 그것은 위젯이 말없이
    /// 버려 `in<LF>prog` 가 `inprog` 라는 다른 칸 이름으로 읽힌다. CLI 한눈 보기
    /// (`view::unopened`·`.moai` 밖 status)와 같은 자로 잇는다.
    #[test]
    fn the_layer_folds_foreign_names_into_one_line() {
        use super::super::layer::{At, Look, Picked, Summary};
        let mut a = layered(At::Layer);
        let place = &mut a.layer.as_mut().unwrap().places[0];
        place.name = "on\te".into();
        place.look = Look::Open {
            sum: Summary {
                counts: vec![("in\nprog\tress\u{1b}[2J".into(), 1)],
                picked: vec![Picked { id: "argos\t0004".into(), title: "첫 줄\n둘째\t줄".into(), column: "in_progress".into() }],
                warnings: 0,
                unreadable: 0,
            },
        };
        let screen = render(&mut a, 140, 22).join("\n");
        assert!(screen.contains("in  prog ress[2J 1"), "칸 이름이 한 줄로 안 접혔다\n{screen}");
        assert!(screen.contains("argos 0004"), "id 의 탭이 사라졌다\n{screen}");
        assert!(screen.contains("첫 줄  둘째 줄"), "제목이 한 줄로 안 접혔다\n{screen}");
        assert!(screen.contains("on e/"), "목록 줄의 이름에서 탭이 사라졌다\n{screen}");
        assert!(screen.contains("│  on e "), "상세 머리의 이름에서 탭이 사라졌다\n{screen}");
        assert!(!screen.contains('\u{1b}'), "{screen}");
    }

    /// **층으로 가는 `..` 은 줄로 안 센다** — 층이 있으면 빈 프로젝트의 뿌리에도 `..` 이
    /// 서는데, 그것을 세면 목록 제목이 " 1줄 " 이 되어 빈 프로젝트를 비었다고 못 한다.
    #[test]
    fn an_empty_project_under_the_layer_is_still_called_empty() {
        use super::super::layer::At;
        let mut a = layered(At::Project("/w/one".into()));
        a.issues.clear();
        a.index = crate::nav::Index::of(&[]);
        a.keep.clear();
        a.warnings = 0;
        a.cursor = 0;
        assert_eq!(a.rows(), [Row::Up]);
        let lines = render(&mut a, 60, 10);
        let title = lines.iter().find(|l| l.contains('┌')).unwrap();
        assert!(title.contains("비었다") && !title.contains("1줄"), "{title:?}");
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

    /// 거름망 칸에 `q` 를 쳐 넣고 `w` 칸 창의 맨 아랫줄(글자만, [`render`] 처럼 두 칸
    /// 글자 뒤 칸은 건너뛴다)과 터미널 커서 칸을 꺼낸다.
    fn filter_line(q: &str, w: u16) -> (String, u16) {
        let (row, x, _) = filter_cells(q, w);
        (row, x)
    }

    /// [`filter_line`] 에 더해 커서 앞 칸과 커서 칸의 글자를 꺼낸다.
    fn filter_cells(q: &str, w: u16) -> (String, u16, (String, String)) {
        use ratatui::backend::Backend;
        let mut a = app();
        a.hit("SPC f");
        for c in q.chars() {
            a.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let mut term = Terminal::new(TestBackend::new(w, 6)).unwrap();
        term.draw(|f| screen(f, &mut a)).unwrap();
        let x = term.backend_mut().get_cursor_position().unwrap().x;
        let buf = term.backend().buffer();
        let mut row = String::new();
        let mut skip = 0usize;
        for c in 0..w {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let sym = buf[(c, 5)].symbol();
            row.push_str(sym);
            skip = crate::text::width(sym).saturating_sub(1);
        }
        let at = |c: u16| if c < w { buf[(c, 5)].symbol().to_string() } else { String::new() };
        (row, x, (at(x.wrapping_sub(1)), at(x)))
    }

    /// **글이 길어도 오류가 줄 안에 선다** (moai-1vjh). 긴 거름망일수록 오타가 잦은데,
    /// 폭을 글칸에 다 주면 바로 그때 오류가 줄 밖으로 밀렸다. 커서는 여전히 친 글 끝에
    /// 서고, 끝이 보인다.
    #[test]
    fn a_long_filter_still_shows_its_error() {
        let q = "tag=parser grep=원자적 쓰기 원자적 쓰기 원자적 쓰기 원자적 쓰기 status=xyz";
        let (row, x, around) = filter_cells(q, 80);
        assert!(row.contains("`xyz` 라는 칸이 없다"), "오류가 밀려났다: {row}");
        assert!(!row.contains("Enter 걸기"), "{row}");
        // 커서 바로 앞 칸이 친 글의 마지막 글자이고, 커서 칸은 비었다.
        assert_eq!((around.0.as_str(), around.1.as_str()), ("z", " "), "{x} {row}");
        assert!(row.contains("status=xyz") && !row.contains("tag=parser"), "끝이 안 보인다: {row}");
    }

    /// 짧은 글은 그대로다 — 글 뒤 네 칸을 띄우고 오류가 붙는다. 몫을 떼는 것은 글이
    /// 칸을 넘칠 때만 보인다.
    #[test]
    fn a_short_filter_draws_as_before() {
        let (row, x) = filter_line("status=xyz", 80);
        assert!(row.starts_with(" 거름망  status=xyz    `xyz` 라는 칸이 없다"), "{row}");
        assert_eq!(x, 9 + 10);
        let (row, _) = filter_line("tag=parser", 80);
        assert!(row.starts_with(" 거름망  tag=parser    Enter 걸기  Esc 그만"), "{row}");
    }

    /// **좁으면 오류를 `…` 로 자르고 글칸은 남긴다.** 오류에 다 주면 틀렸다는 말만 있고
    /// 고칠 자리가 안 보인다. 어느 폭에서도 커서는 줄 안, 이름표 뒤에 선다.
    #[test]
    fn a_narrow_prompt_clips_the_error_and_keeps_the_cursor() {
        let q = "tag=parser grep=원자적 쓰기 status=xyz";
        let (row, x) = filter_line(q, 40);
        assert!(row.contains("`xyz") && row.contains('…'), "오류가 안 보이거나 안 잘렸다: {row}");
        assert!(row.contains("=xyz"), "치는 자리가 안 보인다: {row}");
        assert!((9..40).contains(&x), "{x} {row}");
        for w in 1..=40 {
            let (row, x) = filter_line(q, w);
            assert!(w <= 9 || (9..w).contains(&x), "{w}칸: 커서가 이름표 위나 줄 밖에 섰다 {x} {row}");
        }
    }

    /// 나누기의 약속: 글칸은 `avail` 을 안 넘고 삼분의 일 밑으로 안 줄며, 둘에 틈을
    /// 더해도 줄을 안 넘는다. 자리가 있으면 오류는 안 잘린다.
    #[test]
    fn the_prompt_room_never_overflows() {
        for avail in 0..120 {
            for error in 0..100 {
                let (field, room) = prompt_room(avail, error);
                assert!(field <= avail && field >= avail / 3, "{avail} {error} → {field}");
                if room > 0 {
                    assert!(field + PROMPT_GAP + room <= avail, "{avail} {error} → {field} {room}");
                } else {
                    assert_eq!(field, avail, "오류가 못 받는데 글칸을 줄였다 {avail} {error}");
                }
                if avail / 3 + PROMPT_GAP + error <= avail {
                    assert_eq!(room, error, "자리가 있는데 잘랐다 {avail} {error}");
                }
            }
        }
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
            let m = super::Mode::Grep(super::Input::new(&q), crate::query::GrepIn::All);
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
