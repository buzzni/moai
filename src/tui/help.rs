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

use super::keys::{Bind, Browse, Ctx, Key, LEADER, Side, Stat, Wiki, move_word, name_of_seq};
use super::scroll::Move;
use crate::i18n::say;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    /// 마지막으로 그린 판의 줄 수 — 여러 열로 나눴으면 가장 긴 열의 길이다([`lay_out`]). 굴리기의 끝이 이것이다.
    /// 아직 안 그렸으면 한 열의 줄 수(줄 전부)다.
    pub span: usize,
}

impl Sheet {
    /// 지금 자리로 짓는다 — 키 표는 [`Ctx::keys`] 다. 연 뒤에 키 파일이 바뀌어도 이 창은 연 때의 것이다 — 다시 열면
    /// 새로 읽는다.
    pub fn open(c: &Ctx) -> Sheet {
        let lines = lines(c);
        Sheet { span: lines.len(), lines, top: 0, height: 0 }
    }

    /// 굴린다. 끝을 넘지 않는다 — 마지막 줄이 창 바닥에 서면 더 안 내려간다.
    pub fn step(&mut self, m: Move) {
        let page = self.height.max(1);
        let last = self.span.saturating_sub(page);
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

    /// 휠 — `by` 줄을 굴린다(음수면 위로). 목록·상세와 같은 걸음([`super::mouse::WHEEL`])을 부르는 쪽이 준다.
    pub fn roll(&mut self, by: isize) {
        let last = self.span.saturating_sub(self.height.max(1));
        self.top = self.top.saturating_add_signed(by).min(last);
    }
}

/// 열의 수 상한(2026-10-26 사용자 요청 — 화면 크기에 따라 두 열 또는 세 열).
pub const MAX_COLUMNS: usize = 3;

/// 열 사이의 빈칸.
pub const GAP: usize = 3;

/// 놓인 열 하나 — 줄과, 그 열의 키 칸 폭·열 폭. 폭은 열마다 제 것이다: 긴 SPC 줄이 든 열이 짧은 이동 열까지
/// 넓히면 세 열이 들 폭에서 두 열밖에 못 선다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column<'a> {
    pub lines: Vec<&'a Line>,
    pub key_w: usize,
    pub width: usize,
}

/// 여러 열로 놓은 판 — 열들과 가장 긴 열의 줄 수.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laid<'a> {
    pub columns: Vec<Column<'a>>,
    pub rows: usize,
}

impl Laid<'_> {
    /// 열 폭과 사이 빈칸을 더한 폭.
    pub fn width(&self) -> usize {
        self.columns.iter().map(|c| c.width).sum::<usize>() + GAP * self.columns.len().saturating_sub(1)
    }
}

/// 줄을 `n` 열로 나눈다 — **열 우선**(위에서 아래로 채우고 다음 열로), 열마다 `ceil(줄 수 / n)` 줄. 열이 절의
/// 머리로 끝나면 그 머리를 다음 열로 넘긴다 — 바닥에 홀로 선 머리는 무엇의 머리인지 안 보인다. 넘긴 만큼 다음
/// 열이 길어질 수 있고, 마지막 열은 남은 줄을 다 받는다.
pub fn split(lines: &[Line], n: usize) -> Vec<Vec<&Line>> {
    let n = n.max(1);
    let per = lines.len().div_ceil(n).max(1);
    let mut out: Vec<Vec<&Line>> = vec![Vec::new()];
    for l in lines {
        let more = out.len() < n;
        let col = out.last_mut().expect("늘 한 열은 있다");
        if col.len() >= per && more {
            let carry = if col.len() > 1 && matches!(col.last(), Some(Line::Head(_))) { col.pop() } else { None };
            out.push(carry.into_iter().collect());
        }
        out.last_mut().expect("늘 한 열은 있다").push(l);
    }
    out
}

/// 폭 `room`·높이 `rows` 안에 놓는다. **다 드는 가장 적은 열**을 고른다 — 한 열로 다 들면 한 열이다. 어느 수로도
/// 높이에 다 안 들면 **폭에 드는 가장 많은 열**이다(굴리는 줄이 가장 적다). 어느 수도 폭에 안 들면 한 열이고, 그리는
/// 쪽이 자른다.
pub fn lay_out(lines: &[Line], room: usize, rows: usize) -> Laid<'_> {
    use crate::text::width;
    let laid = |n: usize| {
        let columns: Vec<Column> = split(lines, n)
            .into_iter()
            .map(|col| {
                let (mut key_w, mut what_w, mut head_w) = (0, 0, 0);
                for l in &col {
                    match l {
                        Line::Head(t) => head_w = head_w.max(width(t)),
                        Line::Key { keys, what } => {
                            key_w = key_w.max(width(keys));
                            what_w = what_w.max(width(what));
                        }
                    }
                }
                // 키 칸은 머리보다 한 칸 들어간다 — 절의 머리가 눈에 먼저 선다.
                Column { lines: col, key_w, width: (1 + key_w + 2 + what_w).max(head_w) }
            })
            .collect();
        let rows = columns.iter().map(|c| c.lines.len()).max().unwrap_or(0);
        Laid { columns, rows }
    };
    // **열을 늘린다고 판이 늘 넓어지지는 않는다**(리뷰) — 열 폭은 열마다 제 줄의 최댓값이라, 넓은 줄이 한 열에 모이면 세
    // 열이 두 열보다 좁을 수 있다. 그래서 폭을 넘은 수에서 멈추지 않고 셋을 다 잰다. 한 열은 폭에 안 들어도 쥐고 있는다 —
    // 아무 수도 안 들면 그것이 답이다.
    let mut best: Option<Laid> = None;
    for n in 1..=MAX_COLUMNS {
        let l = laid(n);
        let fits = l.width() <= room;
        if fits && l.rows <= rows {
            return l;
        }
        if fits || best.is_none() {
            best = Some(l);
        }
    }
    best.expect("한 열은 늘 잰다")
}

/// 줄이 SPC 로 시작하나.
fn leader<A>(b: &Bind<A>) -> bool {
    b.seq.first().is_some_and(|k| k.matches(LEADER.event()))
}

/// 그 동작의 키들 가운데 `pick` 이 고른 것 — **키 열에서 이름을 짓는다**(`moai keys` 와 같다). 표의 이름(`label`)은
/// 바가 대는 첫 키라, 그것만 읽으면 이름 없이 도는 `l`·`h`·`Tab`·화살표가 이 창에서 빠진다.
///
/// **Ctrl 을 쥔 채 이어 누른 꼴은 첫째만 댄다** — `Ctrl-w w` 곁의 `Ctrl-w Ctrl-w`·`Ctrl-w W` 는 손이 Ctrl·Shift 를 놓지
/// 않은 꼴을 받아 주는 것이지 따로 익힐 키가 아니다([`held_down`]). 접두어와 길이가 같아도 뒤 키가 다르면 다 댄다(리뷰) —
/// `keys.toml` 의 `top = ["g g", "g t"]` 는 둘 다 사람이 고른 키다. 한 키짜리(`=`·`+`)도 다 댄다.
fn names<A: Copy + PartialEq>(table: &[Bind<A>], act: A, pick: impl Fn(&Bind<A>) -> bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<&[Key]> = Vec::new();
    for b in table.iter().filter(|b| b.act == act && pick(b)) {
        if b.seq.len() > 1 && seen.iter().any(|s| held_down(s, b.seq)) {
            continue;
        }
        seen.push(b.seq);
        let n = name_of_seq(b.seq);
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out.join("·")
}

/// 두 열이 **Ctrl·Shift 를 쥐었는가만** 다른가 — 첫 키(접두어)가 같고, 뒤 키들이 Ctrl·Shift 와 글자의 대소문자를 빼면 같다.
fn held_down(a: &[Key], b: &[Key]) -> bool {
    fn loose(k: &Key) -> (KeyCode, KeyModifiers) {
        let e = k.event();
        let code = match e.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        (code, e.modifiers.difference(KeyModifiers::CONTROL | KeyModifiers::SHIFT))
    }
    a.len() == b.len()
        && a.first().map(|k| k.event()) == b.first().map(|k| k.event())
        && a[1..].iter().zip(&b[1..]).all(|(x, y)| loose(x) == loose(y))
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

/// 창의 줄 전부 — 키 표는 [`Ctx::keys`] 에서 읽는다. 키가 다 떼인 동작(`"none"`)은 안 선다 — 누를 키가 없다.
pub fn lines(c: &Ctx) -> Vec<Line> {
    let (lang, keys) = (c.lang, &c.keys);
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
    //
    // **자리마다 갈리는 낱말은 둘을 다 댄다**(리뷰) — 이 창은 그 창의 지금 상태를 모른다. 바의 낱말(`b 날마다`)은 누르면
    // 갈 곳이라, 하나만 박으면 주마다로 연 통계 창에서 `b` 가 이 창이 대는 것과 거꾸로 간다.
    let both = |a: &str, b: &str| if a == b { a.to_string() } else { format!("{a} / {b}") };
    let rows = once(keys.stats, |b| !matches!(b.act, Stat::Step(_)))
        .into_iter()
        .map(|a| (names(keys.stats, a, |_| true), both(a.what(true, lang), a.what(false, lang))))
        .collect();
    section(say(lang, "tui.help.stats"), rows);

    let rows = once(keys.wiki, |b| !matches!(b.act, Wiki::Step(_)))
        .into_iter()
        .map(|a| {
            let what = match a {
                Wiki::FocusNext => focus_word(true, None, lang).to_string(),
                Wiki::Focus(s) => focus_word(true, Some(s), lang).to_string(),
                // 목록에서는 읽기, 본문에서는 링크 고르기 — 둘 다 댄다(위의 `both`).
                _ => both(a.what(false, false, false, lang), a.what(true, false, false, lang)),
            };
            (names(keys.wiki, a, |_| true), what)
        })
        .collect();
    section(say(lang, "tui.help.wiki"), rows);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    use super::super::keys::Keymap;

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
        let lines = lines(&c);
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

    /// **Ctrl·Shift 를 쥔 꼴만 접고, 사람이 고른 다른 키는 다 댄다**(리뷰) — 기본 표의 `Ctrl-w Ctrl-w` 는 접히고,
    /// `keys.toml` 이 한 동작에 준 같은 접두어·같은 길이의 두 열(`g g`·`g t`)은 둘 다 선다.
    #[test]
    fn a_held_ctrl_variant_folds_but_a_second_key_of_your_own_stands() {
        let c = ctx();
        let base = lines(&c);
        // 탐색과 위키 창이 저마다 한 줄씩 — 둘 다 `Ctrl-w Ctrl-w`·`Ctrl-w W` 를 접는다.
        assert_eq!(keys_of(&base, "next pane"), ["Ctrl-w w", "Ctrl-w w"], "기본 표의 Ctrl 쥔 꼴이 따로 섰다");
        let (keys, problems) = super::super::keymap::build("[moves]\ntop = [\"g g\", \"g t\"]\n");
        assert_eq!(problems, Vec::new(), "시험의 전제 — 키 파일이 성하다");
        let mine = lines(&Ctx { keys, ..ctx() });
        assert!(
            mine.iter().any(|l| matches!(l, Line::Key { keys, .. } if keys.split('·').any(|n| n == "g t"))),
            "사람이 더한 `g t` 가 창에서 빠졌다: {mine:?}"
        );
    }

    /// **휠은 `by` 줄씩 굴리고 끝을 안 넘는다** — 걸음은 목록·상세와 같은 수를 부르는 쪽이 준다.
    #[test]
    fn the_wheel_rolls_by_its_step_and_stops_at_the_ends() {
        let mut s = Sheet::open(&ctx());
        s.height = 10;
        s.roll(3);
        assert_eq!(s.top, 3);
        s.roll(-5);
        assert_eq!(s.top, 0);
        s.roll(isize::MAX);
        assert_eq!(s.top, s.lines.len() - 10);
    }

    /// **굴리기는 끝을 넘지 않는다** — 마지막 줄이 바닥에 서면 더 안 내려가고, 맨 위에서 위로 가도 0 이다.
    #[test]
    fn scrolling_stops_at_both_ends() {
        let mut s = Sheet::open(&ctx());
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

    /// **열은 위에서 아래로 채우고, 머리로 끝나지 않는다**(moai-c4tz) — 줄의 차례가 열을 건너도 그대로고, 바닥에
    /// 홀로 선 머리는 다음 열의 첫 줄로 넘어간다.
    #[test]
    fn columns_fill_downwards_and_never_end_on_a_head() {
        let lines = lines(&ctx());
        for n in 1..=MAX_COLUMNS {
            let cols = split(&lines, n);
            assert!(cols.len() <= n, "{n} 열이 {} 열로 섰다", cols.len());
            let flat: Vec<&Line> = cols.iter().flatten().copied().collect();
            assert_eq!(flat, lines.iter().collect::<Vec<_>>(), "{n} 열이 줄의 차례를 바꿨다");
            for (k, col) in cols.iter().enumerate().take(cols.len() - 1) {
                assert!(!matches!(col.last(), Some(Line::Head(_))), "{n} 열의 {k} 번째 열이 머리로 끝난다");
            }
        }
        // 손으로 지은 판 — 셋째 줄이 머리라 첫 열(두 줄)의 바닥에 안 남는다.
        let key = |k: &str| Line::Key { keys: k.into(), what: "w".into() };
        let small = [Line::Head("a".into()), key("1"), Line::Head("b".into()), key("2")];
        let cols = split(&small, 2);
        assert_eq!(cols, [vec![&small[0], &small[1]], vec![&small[2], &small[3]]]);
        let odd = [Line::Head("a".into()), key("1"), key("2"), Line::Head("b".into()), key("3")];
        let cols = split(&odd, 2);
        assert_eq!(cols, [vec![&odd[0], &odd[1], &odd[2]], vec![&odd[3], &odd[4]]]);
    }

    /// **높이에 다 드는 가장 적은 열, 안 들면 폭에 드는 가장 많은 열**(moai-c4tz, 사용자 요청: 화면 크기에 따라 두
    /// 열 또는 세 열) — 폭을 넘는 판은 안 고르고, 한 열도 안 들면 한 열이다.
    #[test]
    fn the_layout_takes_the_fewest_columns_that_fit_else_the_most_that_are_wide_enough() {
        let lines = lines(&ctx());
        let n = |room: usize, rows: usize| lay_out(&lines, room, rows).columns.len();
        assert_eq!(n(400, 1000), 1, "한 열로 다 드는데 나눴다");
        let two = lay_out(&lines, 400, 1000).rows.div_ceil(2) + 2;
        assert_eq!(n(400, two), 2);
        assert_eq!(n(400, 10), MAX_COLUMNS, "높이가 모자라면 폭에 드는 가장 많은 열이다");
        let one = lay_out(&lines, 400, 1000).width();
        let width2 = lay_out(&lines, 400, two).width();
        assert!(width2 > one, "두 열이 한 열보다 좁다");
        assert_eq!(n(width2, 10), 2, "세 열이 안 드는 폭에서 두 열이 아니다");
        assert_eq!(n(10, 10), 1, "한 열도 안 드는 폭은 한 열이다");
        for room in [10, width2, 400] {
            let l = lay_out(&lines, room, 10);
            assert!(l.columns.len() == 1 || l.width() <= room, "{room} 칸을 넘는 판을 골랐다");
        }
        // 넓은 줄이 한 열에 모이면 세 열이 두 열보다 좁다(리뷰) — 두 열(34+3+34=71)이 60 칸을 넘어도 세 열(5+3+34+3+5=50)을
        // 잰다. 두 열에서 멈추면 높이에 다 드는 판을 두고 한 열로 굴린다.
        let key = |what: usize| Line::Key { keys: "k".into(), what: "w".repeat(what) };
        let lumpy = [key(1), key(1), key(30), key(30), key(1), key(1)];
        let l = lay_out(&lumpy, 60, 2);
        assert_eq!((l.columns.len(), l.width(), l.rows), (3, 50, 2), "두 열이 폭을 넘자 세 열을 안 쟀다");
    }
}
