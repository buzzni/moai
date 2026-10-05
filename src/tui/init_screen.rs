//! `moai init` 의 선택 상자(moai-zynt.a7y). 고르는 상태와 규칙은 [`crate::init_choice`] 에 있고, 여기는
//! 터미널을 켜고 그리고 키를 넘기는 일만 한다.
//!
//! **대체 화면을 안 켠다** — 몇 줄짜리 인라인 뷰포트다. 고른 뒤 `init` 이 낸 보고가 셸 스크롤에 그대로
//! 남아야 한다. 끝나면 그 몇 줄을 지우고 보고가 그 자리에 선다.

use crate::i18n::{Lang, fill, say};
use crate::init_choice::{Act, FIELDS, Field, Form, Lock, Plan};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, TerminalOptions, Viewport};
use unicode_width::UnicodeWidthStr;

/// 머리 · 빈 줄 · 칸들 · 빈 줄 · 알림 · 키 — 칸 말고 다섯 줄이다. 칸이 숨으면 아래가 빈 줄로 남는다 — 높이를
/// 그때마다 바꾸면 인라인 뷰포트가 셸 스크롤을 밀어 올린다.
const HEIGHT: u16 = 5 + FIELDS.len() as u16;

/// 사람이 보는 터미널인가 — **읽는 쪽과 쓰는 쪽이 둘 다** 터미널이어야 묻는다. 한쪽이라도 파이프면
/// 부른 것은 에이전트나 스크립트다: 묻는 순간 아무도 답하지 않는 키를 영영 기다린다.
pub fn on_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// 묻고 계획을 돌려준다. `None` 은 사람이 그만뒀다 — 아무것도 안 썼다. `check` 는 심으려는 계획을
/// 재어 못 심으면 까닭을 댄다(그 잣대는 `init` 이 쥔다). `Err` 는 터미널을 못 켠 까닭이다.
pub fn ask(
    lang: Lang,
    dir: &str,
    mut form: Form,
    check: &dyn Fn(&Plan) -> Option<String>,
) -> Result<Option<Plan>, String> {
    let mut term = ratatui::try_init_with_options(TerminalOptions { viewport: Viewport::Inline(HEIGHT) })
        .map_err(|e| e.to_string())?;
    let out = (|| -> std::io::Result<Option<Plan>> {
        loop {
            term.draw(|f| draw(f, &form, lang, dir))?;
            let Event::Key(k) = event::read()? else { continue };
            // 윈도의 crossterm 은 놓는 것도 키로 낸다 — 한 번 친 글자가 두 번 들어간다.
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match form.key(k) {
                Act::Stay => {}
                Act::Stop => return Ok(None),
                Act::Plant => match check(&form.plan()) {
                    Some(why) => form.problem = Some(why),
                    None => return Ok(Some(form.plan())),
                },
            }
        }
    })();
    // **`ratatui::restore` 를 안 부른다** — 그것은 대체 화면을 떠나는 `?1049l` 도 내는데, 그 화면에 든
    // 적이 없는 터미널은 그 글을 "저장한 커서로 돌아가라" 로 읽어 커서가 엉뚱한 줄로 뛴다. 켠 것은
    // raw mode 하나라 그것만 끈다. 그 몇 줄은 지워 `init` 의 보고가 그 자리에서 시작하게 한다.
    // 지운 뒤 커서는 줄 가운데(마지막에 그린 자리)에 남는다 — 줄 머리로 돌려야 보고의 첫 줄이 거기서 선다.
    let _ = term.clear();
    let _ = ratatui::crossterm::execute!(std::io::stdout(), ratatui::crossterm::cursor::MoveToColumn(0));
    let _ = term.show_cursor();
    let _ = ratatui::crossterm::terminal::disable_raw_mode();
    out.map_err(|e| e.to_string())
}

/// 칸의 이름과, 고르는 칸이면 선택지의 이름들(왼쪽부터).
fn names(f: Field, lang: Lang) -> (&'static str, Vec<&'static str>) {
    match f {
        Field::Prefix => (say(lang, "init.ask_prefix"), Vec::new()),
        Field::Tracking => (
            say(lang, "init.ask_tracking"),
            vec![
                say(lang, "init.ask_tracking_commit"),
                say(lang, "init.ask_tracking_exclude"),
                say(lang, "init.ask_tracking_gitignore"),
            ],
        ),
        Field::Guide => (
            say(lang, "init.ask_guide"),
            vec![
                say(lang, "init.ask_guide_block"),
                say(lang, "init.ask_guide_file"),
                say(lang, "init.ask_guide_hook"),
                say(lang, "init.ask_guide_none"),
            ],
        ),
        Field::Skill => (say(lang, "init.ask_skill"), vec![say(lang, "init.ask_skill_on"), say(lang, "init.ask_skill_off")]),
        Field::Driver => (say(lang, "init.ask_driver"), vec![say(lang, "init.ask_driver_on"), say(lang, "init.ask_driver_off")]),
        Field::Project => {
            (say(lang, "init.ask_project"), vec![say(lang, "init.ask_project_on"), say(lang, "init.ask_project_off")])
        }
    }
}

fn draw(f: &mut Frame, form: &Form, lang: Lang, dir: &str) {
    let area = f.area();
    // 이름 칸의 너비는 숨은 칸까지 재어 둔다 — 칸이 숨었다 서면 값 칸이 옆으로 뛰지 않게.
    let pad = FIELDS.iter().map(|f| names(*f, lang).0.width()).max().unwrap_or(0);
    let mut lines = vec![
        Line::from(Span::styled(fill(say(lang, "init.ask_title"), &[("dir", dir)]), Style::new().bold())),
        Line::raw(""),
    ];
    let mut cursor_at: Option<Position> = None;
    for (i, field) in form.rows().into_iter().enumerate() {
        let (label, options) = names(field, lang);
        let here = form.cursor == field;
        let locked = form.locked(field);
        let head = format!("{} {label}{}  ", if here { '▸' } else { ' ' }, " ".repeat(pad - label.width()));
        let mut spans = vec![Span::raw(head.clone())];
        match form.option(field) {
            None => {
                spans.push(Span::styled(form.typed.clone(), Style::new().add_modifier(Modifier::UNDERLINED)));
                if here && !locked {
                    let x = area.x + (head.width() + form.typed.width()) as u16;
                    cursor_at = Some(Position::new(x.min(area.right().saturating_sub(1)), area.y + 2 + i as u16));
                }
            }
            Some((_, at)) => {
                let shown: Vec<String> =
                    options.iter().enumerate().map(|(j, name)| format!("{} {name}", mark(j == at))).collect();
                spans.push(Span::raw(shown.join("   ")));
            }
        }
        match form.locked_by(field) {
            Some(Lock::Flag) => spans.push(Span::raw(format!("   · {}", say(lang, "init.ask_by_flag")))),
            Some(Lock::Hook) => spans.push(Span::raw(format!("   · {}", say(lang, "init.ask_by_hook")))),
            None => {}
        }
        let style = if locked {
            Style::new().add_modifier(Modifier::DIM)
        } else if here {
            Style::new().bold()
        } else {
            Style::new()
        };
        lines.push(Line::from(spans).style(style));
    }
    lines.push(Line::raw(""));
    // 틀린 접두어는 `!` 를 달고 선다 — 색이 혼자 뜻을 지지 않는다.
    lines.push(match &form.problem {
        Some(why) => Line::from(Span::styled(format!("! {why}"), Style::new().bold())),
        None => {
            let shown = if form.typed.is_empty() { "…" } else { form.typed.as_str() };
            Line::from(Span::styled(
                fill(say(lang, "init.ask_prefix_note"), &[("prefix", shown)]),
                Style::new().add_modifier(Modifier::DIM),
            ))
        }
    });
    lines.push(Line::from(Span::styled(say(lang, "init.ask_keys"), Style::new().add_modifier(Modifier::DIM))));
    f.render_widget(Paragraph::new(lines), Rect { height: area.height.min(HEIGHT), ..area });
    if let Some(p) = cursor_at {
        f.set_cursor_position(p);
    }
}

/// 고른 값의 표시. 글리프로 가른다 — 색 없이도 읽힌다.
fn mark(on: bool) -> &'static str {
    if on { "(•)" } else { "( )" }
}
