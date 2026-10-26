//! 키 목록 창 — `?`·`SPC h`(moai-ucyr, 2026-10-26 사용자 요청).
//!
//! **목록을 따로 적지 않는다.** 줄은 지금 도는 키 표([`Keymap`])에서 읽는다 — `moai keys` 가 읽는 그 표라,
//! `keys.toml` 로 바꾼 키가 그대로 서고 키를 더하면 이 창에도 따라온다. 낱말은 키 바와 SPC 메뉴가 대는 그 낱말이다
//! ([`Browse::what`]·[`Browse::menu_word`]).
//!
//! **절은 화면마다다** — 탐색(SPC 없이 바로 누르는 키)·이동·SPC 메뉴·간트·통계 창·위키 창. 차례 편집 창·디렉터리
//! 고르기 창·물음은 열려 있는 동안 아랫바가 제 키를 다 대므로 여기 안 싣는다.
//!
//! **조각이다.** 표와 [`Ctx`] 만 받는다. 연 동안 키를 받고 그리는 것은 든 쪽(`App::help_key`, `draw::help_sheet`)이다.

use super::keys::{Bind, Browse, Ctx, Keymap, LEADER, Side, Stat, Wiki, move_word, name_of_seq};
use super::scroll::Move;
use crate::i18n::say;
use ratatui::crossterm::event::KeyEvent;

/// 창의 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// 절의 머리 — `탐색`·`SPC 메뉴`.
    Head(String),
    /// 키 이름(여럿이면 `·` 로 잇는다)과 그 낱말.
    Key { keys: String, what: String },
}

/// 열린 창 — 줄과 굴린 자리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub lines: Vec<Line>,
    /// 맨 위에 선 줄.
    pub top: usize,
    /// 마지막으로 그린 높이(줄 수). 반 쪽·한 쪽 굴리기가 이것으로 잰다 — 그리는 쪽이 적는다.
    pub height: usize,
}

impl Sheet {
    /// 지금 표와 자리로 짓는다. 연 뒤에 키 파일이 바뀌어도 이 창은 연 때의 것이다 — 다시 열면 새로 읽는다.
    pub fn open(keys: &Keymap, c: &Ctx) -> Sheet {
        Sheet { lines: lines(keys, c), top: 0, height: 0 }
    }

    /// 굴린다. 끝을 넘지 않는다 — 마지막 줄이 창 바닥에 서면 더 안 내려간다.
    pub fn step(&mut self, m: Move) {
        let page = self.height.max(1);
        let last = self.lines.len().saturating_sub(page);
        self.top = match m {
            Move::LineDown => self.top + 1,
            Move::LineUp => self.top.saturating_sub(1),
            Move::HalfDown => self.top + page.div_ceil(2),
            Move::HalfUp => self.top.saturating_sub(page.div_ceil(2)),
            Move::PageDown => self.top + page,
            Move::PageUp => self.top.saturating_sub(page),
            Move::Top => 0,
            Move::Bottom => last,
        }
        .min(last);
    }
}

/// 줄이 SPC 로 시작하나.
fn leader<A>(b: &Bind<A>) -> bool {
    b.seq.first().is_some_and(|k| k.matches(LEADER.event()))
}

/// 그 동작의 키들 가운데 `pick` 이 고른 것 — **키 열에서 이름을 짓는다**(`moai keys` 와 같다). 표의 이름(`label`)은
/// 바가 대는 첫 키라, 그것만 읽으면 이름 없이 도는 `l`·`h`·`Tab`·화살표가 이 창에서 빠진다.
///
/// **같은 접두어의 같은 길이 열은 첫째만 댄다** — `Ctrl-w w` 곁의 `Ctrl-w Ctrl-w` 는 Ctrl 을 쥔 채 누른 꼴을 받아 주는
/// 것이지 따로 익힐 키가 아니다. 한 키짜리(`=`·`+`)는 다 댄다.
fn names<A: Copy + PartialEq>(table: &[Bind<A>], act: A, pick: impl Fn(&Bind<A>) -> bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut heads: Vec<(usize, KeyEvent)> = Vec::new();
    for b in table.iter().filter(|b| b.act == act && pick(b)) {
        if b.seq.len() > 1 {
            let head = (b.seq.len(), b.seq[0].event());
            if heads.contains(&head) {
                continue;
            }
            heads.push(head);
        }
        let n = name_of_seq(b.seq);
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out.join("·")
}

/// 칸 옮기기의 낱말 — 바는 **지금 가는 칸의 이름**(`상세`)을 대지만, 목록에서는 어느 쪽으로 가는 키인지를 댄다.
fn focus_word(next: bool, side: Option<Side>, lang: crate::i18n::Lang) -> &'static str {
    match (next, side) {
        (_, Some(Side::Left)) => say(lang, "tui.help.focus_left"),
        (_, Some(Side::Right)) => say(lang, "tui.help.focus_right"),
        (_, Some(Side::Up)) => say(lang, "tui.help.focus_up"),
        (_, Some(Side::Down)) => say(lang, "tui.help.focus_down"),
        (true, None) => say(lang, "tui.help.focus_next"),
        (false, None) => say(lang, "tui.help.focus_prev"),
    }
}

/// 표의 동작을 차례대로 한 번씩 — 같은 동작의 줄이 여럿이어도 첫 줄의 자리에 한 번 선다.
fn once<A: Copy + PartialEq>(table: &[Bind<A>], keep: impl Fn(&Bind<A>) -> bool) -> Vec<A> {
    let mut seen: Vec<A> = Vec::new();
    for b in table.iter().filter(|b| keep(b)) {
        if !seen.contains(&b.act) {
            seen.push(b.act);
        }
    }
    seen
}

/// 창의 줄 전부. 키가 다 떼인 동작(`"none"`)은 안 선다 — 누를 키가 없다.
pub fn lines(keys: &Keymap, c: &Ctx) -> Vec<Line> {
    let lang = c.lang;
    let mut out = Vec::new();
    let mut section = |head: &str, rows: Vec<(String, String)>| {
        let rows: Vec<_> = rows.into_iter().filter(|(k, _)| !k.is_empty()).collect();
        if !rows.is_empty() {
            out.push(Line::Head(head.to_string()));
            out.extend(rows.into_iter().map(|(keys, what)| Line::Key { keys, what }));
        }
    };

    // 바로 누르는 키. 이동은 제 절이 있고, 숫자(프로젝트)는 열 줄을 한 줄로 접는다.
    let browse = keys.browse;
    let mut rows = Vec::new();
    for act in once(browse, |b| !leader(b) && !matches!(b.act, Browse::Step(_))) {
        match act {
            Browse::Project(0) => rows.push(("0-9".to_string(), act.what(c).to_string())),
            Browse::Project(_) => {}
            Browse::FocusNext | Browse::FocusPrev | Browse::Focus(_) => {
                let side = if let Browse::Focus(s) = act { Some(s) } else { None };
                let what = focus_word(act == Browse::FocusNext, side, lang);
                rows.push((names(browse, act, |b| !leader(b)), what.to_string()));
            }
            _ => rows.push((names(browse, act, |b| !leader(b)), act.what(c).to_string())),
        }
    }
    section(say(lang, "tui.help.browse"), rows);

    let moves = once(browse, |b| matches!(b.act, Browse::Step(_)));
    let rows = moves
        .into_iter()
        .filter_map(|a| match a {
            Browse::Step(m) => Some((names(browse, a, |_| true), move_word(m, lang).to_string())),
            _ => None,
        })
        .collect();
    section(say(lang, "tui.help.moves"), rows);

    // SPC 메뉴 — 하위 층의 줄은 묶음의 이름을 앞에 단다(`보기 › done`). 칸 토글(`SPC v 1`…)은 한 줄로 접는다.
    let mut rows = Vec::new();
    for act in once(browse, leader) {
        let Some(b) = browse.iter().find(|b| b.act == act && leader(b)) else { continue };
        let word = match act {
            Browse::Column(0) => act.what(c),
            Browse::Column(_) => continue,
            _ => act.menu_word(c),
        };
        let what = if b.seq.len() > 2 {
            let held: Vec<KeyEvent> = b.seq[..b.seq.len() - 1].iter().map(|k| k.event()).collect();
            format!("{} › {word}", super::menu::group(&held, lang))
        } else {
            word.to_string()
        };
        // **SPC 줄은 키 열에서 이름을 짓는다** — 바로 누르는 키가 있는 동작은 SPC 쪽 줄에 이름이 없다(`/` 와 `SPC /`).
        let keys = match act {
            Browse::Column(_) => format!("{}-9", name_of_seq(b.seq)),
            _ => name_of_seq(b.seq),
        };
        rows.push((keys, what));
    }
    section(say(lang, "tui.help.menu"), rows);

    let rows = once(keys.gantt, |_| true)
        .into_iter()
        .map(|a| (names(keys.gantt, a, |_| true), a.what(c).to_string()))
        .collect();
    section(say(lang, "tui.help.gantt"), rows);

    // 통계·위키 창의 이동은 탐색과 같은 키라 이동 절이 이미 댄다.
    let rows = once(keys.stats, |b| !matches!(b.act, Stat::Step(_)))
        .into_iter()
        .map(|a| (names(keys.stats, a, |_| true), a.what(false, lang).to_string()))
        .collect();
    section(say(lang, "tui.help.stats"), rows);

    let rows = once(keys.wiki, |b| !matches!(b.act, Wiki::Step(_)))
        .into_iter()
        .map(|a| {
            let what = match a {
                Wiki::FocusNext => focus_word(true, None, lang),
                Wiki::Focus(s) => focus_word(true, Some(s), lang),
                _ => a.what(true, false, false, lang),
            };
            (names(keys.wiki, a, |_| true), what.to_string())
        })
        .collect();
    section(say(lang, "tui.help.wiki"), rows);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    fn ctx() -> Ctx {
        Ctx { lang: Lang::En, keys: Keymap::default(), ..Ctx::default() }
    }

    fn keys_of<'a>(lines: &'a [Line], what: &str) -> Vec<&'a str> {
        lines
            .iter()
            .filter_map(|l| match l {
                Line::Key { keys, what: w } if w == what => Some(keys.as_str()),
                _ => None,
            })
            .collect()
    }

    /// **SPC 로 시작하지 않는 키가 선다** — 이 창을 만든 까닭이다(moai-ucyr). 읽음(`r`)·접기(`h`)·펼침(`Tab`) 같은
    /// 바로 누르는 키와 이동, 그리고 SPC 메뉴의 줄이 함께 선다.
    #[test]
    fn the_sheet_names_the_keys_that_start_without_spc() {
        let c = ctx();
        let lines = lines(&Keymap::default(), &c);
        let keys: Vec<&str> = lines
            .iter()
            .filter_map(|l| match l {
                Line::Key { keys, .. } => Some(keys.as_str()),
                Line::Head(_) => None,
            })
            .collect();
        for want in ["r", "?", "l", "h", "Tab", "/", "SPC m a", "SPC h", "0-9", "SPC v 1-9"] {
            assert!(keys.iter().any(|k| k.split('·').any(|n| n == want)), "{want} 가 창에 없다: {keys:?}");
        }
        // 바로 누르는 절의 `?` 줄에는 SPC 키가 안 섞이고, SPC 절의 줄은 `SPC h` 만 단다.
        assert_eq!(keys_of(&lines, Browse::Help.what(&c)), ["?", "SPC h"]);
        // 묶음 밑의 줄은 묶음 이름을 앞에 단다.
        assert!(
            lines.iter().any(|l| matches!(l, Line::Key { keys, what } if keys == "SPC m a" && what.contains(" › ")))
        );
        assert!(lines.iter().any(|l| matches!(l, Line::Head(h) if h == "SPC menu")));
    }

    /// **굴리기는 끝을 넘지 않는다** — 마지막 줄이 바닥에 서면 더 안 내려가고, 맨 위에서 위로 가도 0 이다.
    #[test]
    fn scrolling_stops_at_both_ends() {
        let mut s = Sheet::open(&Keymap::default(), &ctx());
        s.height = 10;
        let last = s.lines.len() - 10;
        s.step(Move::Bottom);
        assert_eq!(s.top, last);
        s.step(Move::LineDown);
        assert_eq!(s.top, last);
        s.step(Move::Top);
        s.step(Move::LineUp);
        assert_eq!(s.top, 0);
        s.step(Move::PageDown);
        assert_eq!(s.top, 10);
        s.step(Move::HalfUp);
        assert_eq!(s.top, 5);
    }
}
