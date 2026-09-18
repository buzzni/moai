//! SPC 메뉴(moai-7sjm). **누르면 곧바로 뜨고, 뜻은 키 표에서 읽는다** — 메뉴의 목록을 따로
//! 적지 않는다. [`BROWSE`] 에서 SPC 로 시작하는 줄이 곧 메뉴의 트리다.
//!
//! **메뉴가 열렸다는 것과 어느 층인가는 [`Chord`] 의 대기 열 하나다.** 열이 SPC 로 시작하면
//! 열렸고, 그 열이 곧 접두어(`SPC t`)다. `Mode` 로 두지 않는다 — 탐색이 아닌 모드는 전부
//! 글칸이라, 메뉴가 모드면 메뉴 안의 키가 글자로 샌다. 붙여넣기·글칸으로 넘어가는 길이 열을
//! 버리므로 메뉴도 그 길에서 닫힌다.
//!
//! **`gg` 와 다른 규칙 하나**(키 지도 moai-hudg): 메뉴 안에서 모르는 키는 **무시한다** — 닫지도
//! 알리지도 않는다. `gg` 는 뜻 없는 둘째 키가 열을 버리지만([`Chord::feed`]), 메뉴는 떠 있는
//! 창이라 틀린 키 하나에 사라지면 무엇을 누르려 했는지 다시 봐야 한다. Esc 가 닫고 Bksp 가 한
//! 층 올라간다([`MENU`]).
//!
//! **켜진 것만 선다.** 항목마다 [`Browse::enabled`] 를 읽는다 — 키 바와 같은 판정이다. 메뉴에 안
//! 선 키는 눌러도 모르는 키와 같다: 선 것과 도는 것이 갈리지 않는다.
//!
//! **조각이다.** `App`·터미널을 모른다. 켜짐([`Ctx`])은 든 쪽이 재서 넘긴다. 격자를 어떻게
//! 놓는가([`grid`])도 여기서 폭과 높이만 받아 정하고, `draw.rs` 는 그 격자를 칠하기만 한다.

use super::keys::{BROWSE, Bind, Browse, Chord, Ctx, LEADER, Lookup, MENU, Menu, lookup, name_of};
use crate::text::{clip, width};
use ratatui::crossterm::event::KeyEvent;

/// 하위 접두어의 이름. 표에는 동작만 있고 묶음의 이름은 없어 여기 둔다 — 이름 없는 접두어는
/// 시험(`every_prefix_in_the_menu_has_a_name`)이 막는다.
const GROUPS: &[(&str, &str)] = &[("SPC p", "프로젝트"), ("SPC t", "토글"), ("SPC s", "보기"), ("SPC o", "정렬"), ("SPC c", "열"), ("SPC m", "읽음")];

/// 메뉴가 열렸나.
pub fn open(chord: &Chord) -> bool {
    chord.held().first().is_some_and(|k| LEADER.matches(*k))
}

/// 탐색의 키 하나. 메뉴가 닫혀 있으면 표를 그대로 찾고(SPC 는 기다림이라 메뉴가 열린다),
/// 열려 있으면 메뉴의 규칙을 따른다 — Esc 닫기, Bksp 한 층 위, 켜진 동작은 실행하고 닫기,
/// 켜진 하위 접두어는 한 층 내려가기, **그 밖은 아무 일도 없다.**
pub fn feed(chord: &mut Chord, c: &Ctx, k: KeyEvent) -> Option<Browse> {
    if !open(chord) {
        return chord.feed(BROWSE, k);
    }
    match lookup(MENU, &[k]) {
        Lookup::Run(Menu::Close) => {
            chord.clear();
            return None;
        }
        Lookup::Run(Menu::Up) => {
            chord.pop();
            return None;
        }
        _ => {}
    }
    let mut seq = chord.held().to_vec();
    seq.push(k);
    match lookup(BROWSE, &seq) {
        Lookup::Run(act) if act.enabled(c).is_ok() => {
            chord.clear();
            Some(act)
        }
        Lookup::Pending if live(&seq, c) => {
            chord.push(k);
            None
        }
        _ => None,
    }
}

/// 메뉴 한 칸 — `키 : 낱말 [상태]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// 동작의 낱말, 하위 접두어면 `+이름`(doom which-key 의 모양 — moai-apsa).
    pub what: String,
    /// 켜고 끄는 것의 지금 상태(`[켜짐]`·`[원문]`).
    pub state: Option<&'static str>,
}

impl Entry {
    /// 칸의 설명 — 낱말 뒤에 상태. **상태는 글자로 선다** — 색이 혼자 뜻을 지지 않는다.
    pub fn text(&self) -> String {
        match self.state {
            Some(st) => format!("{} {st}", self.what),
            None => self.what.clone(),
        }
    }

    /// 하위 접두어(`+이름`)인가.
    pub fn is_group(&self) -> bool {
        self.what.starts_with('+')
    }
}

/// 지금 층(`held`)에 선 항목. 차례는 표의 차례다. `columns` 는 설정의 칸 이름이다 — 칸 토글
/// (`SPC s 1`)의 낱말이 거기서 온다. 표는 칸 이름을 모른다.
pub fn entries(held: &[KeyEvent], c: &Ctx, columns: &[String]) -> Vec<Entry> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for b in BROWSE.iter().filter(|b| b.seq.len() > held.len() && under(b, held)) {
        let next = b.seq[held.len()];
        if seen.contains(&next) {
            continue;
        }
        seen.push(next);
        let mut seq = held.to_vec();
        seq.push(next.event());
        match lookup(BROWSE, &seq) {
            Lookup::Run(act) if act.enabled(c).is_ok() => {
                let what = match act {
                    // 설정에 이름이 없으면 표의 낱말로 — 낱말은 표 한 곳에만 둔다.
                    Browse::Column(n) => columns.get(usize::from(n)).map_or(act.menu_word(c), String::as_str),
                    _ => act.menu_word(c),
                };
                out.push(Entry { key: next.name(), what: what.into(), state: act.state(c) });
            }
            Lookup::Pending if live(&seq, c) => {
                out.push(Entry { key: next.name(), what: format!("+{}", group(&seq)), state: None });
            }
            _ => {}
        }
    }
    out
}

/// 테두리에 적을 지금 접두어 — `SPC`, `SPC t`.
pub fn title(held: &[KeyEvent]) -> String {
    held.iter().map(|k| name_of(k.code)).collect::<Vec<_>>().join(" ")
}

/// 뿌리의 이름 — 접두어 줄의 `SPC- 메뉴`, 키 바의 `SPC 메뉴`.
pub const ROOT: &str = "메뉴";

/// 접두어 줄에 적을 지금 층의 이름 — 뿌리는 [`ROOT`], 하위 층은 묶음의 이름(`토글`).
pub fn name(held: &[KeyEvent]) -> &'static str {
    if held.len() <= 1 { ROOT } else { group(held) }
}

/// 격자의 줄 수 상한 — 한 열에 6칸, 넘치면 옆 열에 다시 6칸(사용자 결정, moai-r2dt). 처음의
/// "하단 8~10칸"(moai-apsa)은 뿌리 7칸이 넓은 창에서 왼쪽 한 열로만 서 허전했다(moai-ik31).
/// 항목이 적으면 그만큼 낮게 선다.
pub const MAX_ROWS: usize = 6;

/// 메뉴가 떠도 몸통에 남길 높이 — 테두리 둘과 줄 하나. 이보다 낮게 누르면 커서가 선 줄이
/// 안 보여, 메뉴가 무엇에 대한 것인지를 잃는다.
pub const BODY_MIN: usize = 3;

/// 칸의 키와 설명 사이.
pub const SEP: &str = " : ";

/// 열 사이.
pub const GAP: usize = 2;

/// 자른 설명의 최소 폭 — 한글 한 자와 `…`. 이보다 좁히면 `…` 만 남아 뜻이 없다.
const MIN_TEXT: usize = 3;

/// 경로 줄·배너·키 바를 뺀 높이 `left` 에서 격자에 줄 수 있는 줄 수. 가름줄 하나와
/// [`BODY_MIN`] 을 먼저 뺀다. **0 이면 격자를 세우지 않고 접두어 줄 한 줄로 접는다.**
pub fn rows_for(left: usize) -> usize {
    left.saturating_sub(BODY_MIN + 1).min(MAX_ROWS)
}

/// 격자의 한 칸. 둘 다 **이미 채워 둔 글**이다 — 키는 열 안에서 오른쪽 맞춤이라 [`SEP`] 이 줄
/// 서고, 설명은 잘려(`…`) 열 폭까지 채워져 다음 열이 줄 선다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub key: String,
    pub text: String,
    /// 하위 접두어(`+이름`)인가 — 실행 항목과 색을 가른다. 뜻은 `+` 가 글자로 이미 댄다.
    pub group: bool,
}

/// 격자 — 열 우선으로 채운 칸들. `columns[c][r]` 이 c 열 r 줄이다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grid {
    pub rows: usize,
    pub columns: Vec<Vec<Placed>>,
    /// 폭이 모자라 못 세운 항목 수. 말없이 빠지면 없는 줄 안다 — 접두어 줄이 수를 댄다.
    pub hidden: usize,
}

impl Grid {
    /// r 줄에 선 칸들 — 왼쪽 열부터. 열 우선이라 짧은 것은 마지막 열뿐이다.
    pub fn row(&self, r: usize) -> impl Iterator<Item = &Placed> {
        self.columns.iter().filter_map(move |c| c.get(r))
    }
}

/// 항목을 which-key 식 격자로 놓는다(moai-apsa). **순수 함수다** — 폭 `room` 과 줄 상한
/// `max_rows` 만 받는다. 차례는 받은 차례(곧 표의 차례)다.
///
/// - **열 우선.** 한 열을 `min(항목 수, max_rows)` 줄까지 위에서 아래로 채우고 다음 열로 간다
///   — which-key 가 창 높이까지 열을 채우는 것과 같다. 줄 수가 먼저고 열 수는 거기서 나온다.
/// - **열마다 제 폭.** 키 폭·설명 폭은 그 열에서 가장 넓은 것이다(doom 의 `RET`·`SPC` 열).
/// - **넘치면 설명을 줄인다.** 모든 열의 설명에 같은 상한을 걸어 긴 것부터 `…` 로 자른다.
///   그 상한이 [`MIN_TEXT`] 아래로 가야 하면 **뒤 열을 뺀다** — 빠진 수가 `hidden` 이다.
///   한 열도 안 들면 그 한 열을 폭까지 자른다.
pub fn grid(items: &[Entry], room: usize, max_rows: usize) -> Grid {
    if items.is_empty() || max_rows == 0 {
        return Grid { rows: 0, columns: Vec::new(), hidden: items.len() };
    }
    let rows = items.len().min(max_rows);
    let chunks: Vec<&[Entry]> = items.chunks(rows).collect();
    let key_w: Vec<usize> = chunks.iter().map(|c| c.iter().map(|e| width(&e.key)).max().unwrap_or(0)).collect();
    let text_w: Vec<usize> = chunks.iter().map(|c| c.iter().map(|e| width(&e.text())).max().unwrap_or(0)).collect();
    let fixed = |n: usize| key_w[..n].iter().sum::<usize>() + n * width(SEP) + n.saturating_sub(1) * GAP;
    let mut shown = chunks.len();
    let limit = loop {
        let left = room.saturating_sub(fixed(shown));
        let used = |l: usize| text_w[..shown].iter().map(|t| (*t).min(l)).sum::<usize>();
        let widest = text_w[..shown].iter().copied().max().unwrap_or(0);
        if let Some(l) = (MIN_TEXT.min(widest)..=widest).rev().find(|l| used(*l) <= left) {
            break l;
        }
        if shown == 1 {
            break left;
        }
        shown -= 1;
    };
    let columns = chunks[..shown]
        .iter()
        .enumerate()
        .map(|(c, chunk)| {
            let tw = text_w[c].min(limit);
            chunk
                .iter()
                .map(|e| {
                    let text = clip(&e.text(), tw);
                    Placed {
                        key: format!("{}{}", " ".repeat(key_w[c] - width(&e.key)), e.key),
                        text: format!("{text}{}", " ".repeat(tw.saturating_sub(width(&text)))),
                        group: e.is_group(),
                    }
                })
                .collect()
        })
        .collect();
    Grid { rows, columns, hidden: items.len() - (shown * rows).min(items.len()) }
}

/// 하위 접두어의 이름. 이름이 없으면 `…` 만 — 시험이 막으므로 실제로는 안 선다.
fn group(seq: &[KeyEvent]) -> &'static str {
    let t = title(seq);
    GROUPS.iter().find(|(p, _)| *p == t).map_or("…", |(_, n)| n)
}

/// 이 줄이 `seq` 로 시작하나.
fn under(b: &Bind<Browse>, seq: &[KeyEvent]) -> bool {
    b.seq.len() >= seq.len() && b.seq.iter().zip(seq).all(|(key, k)| key.matches(*k))
}

/// 이 접두어 밑에 켜진 동작이 하나라도 있나 — 없으면 하위 접두어도 안 선다.
fn live(seq: &[KeyEvent], c: &Ctx) -> bool {
    BROWSE.iter().any(|b| b.seq.len() > seq.len() && under(b, seq) && b.act.enabled(c).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn k(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// **`detail` 은 켜 둔다** — 탐색기의 처음값은 상세 칸이 보이는 것이고(`App::detail_open`),
    /// 숨김을 fixture 의 처음값으로 두면 Tab·원문↔그리기가 여기서 늘 꺼진 채로 재어진다.
    fn inside() -> Ctx {
        Ctx { list_focus: true, detail: true, ..Ctx::default() }
    }

    fn layer() -> Ctx {
        Ctx { layer: true, ..inside() }
    }

    fn keys_of(e: &[Entry]) -> Vec<&str> {
        e.iter().map(|e| e.key.as_str()).collect()
    }

    /// **SPC 가 곧바로 뿌리를 연다** — 확정한 트리(moai-hudg)가 표의 차례대로 선다.
    #[test]
    fn spc_opens_the_root_in_the_confirmed_order() {
        let mut ch = Chord::default();
        assert_eq!(feed(&mut ch, &inside(), k(' ')), None);
        assert!(open(&ch));
        assert_eq!(title(ch.held()), "SPC");
        let root = entries(ch.held(), &inside(), &[]);
        assert_eq!(keys_of(&root), ["/", "f", "n", "r", "q", "p", "t", "s", "o", "c", "m"]);
        let what: Vec<&str> = root.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(what, [
            "검색",
            "거름망",
            "생각 담기",
            "다시 읽기",
            "끝내기",
            "+프로젝트",
            "+토글",
            "+보기",
            "+정렬",
            "+열",
            "+읽음"
        ]);
    }

    /// **칸 토글은 설정의 칸 이름을 번호에 붙이고, 있는 칸 수만큼만 선다**(moai-fmv5). 숨김은
    /// 낱말로 댄다 — 색이 혼자 뜻을 지지 않는다.
    #[test]
    fn the_view_menu_numbers_the_configured_columns() {
        let columns: Vec<String> = ["todo", "in_progress", "done"].map(String::from).to_vec();
        let c = Ctx { columns: 3, hidden: 0b100, done_hidden: true, ..inside() };
        let items = entries(&[k(' '), k('s')], &c, &columns);
        assert_eq!(keys_of(&items), ["d", "z", "a", "1", "2", "3"]);
        let text: Vec<String> = items.iter().map(Entry::text).collect();
        assert_eq!(text, ["done [숨김]", "미룸 [보임]", "모두 보이기", "todo [보임]", "in_progress [보임]", "done [숨김]"]);
        let mut ch = Chord::default();
        for x in [' ', 's', '4'] {
            assert_eq!(feed(&mut ch, &c, k(x)), None, "없는 칸의 번호가 돌았다");
        }
        assert!(keys_of(&entries(&[k(' ')], &layer(), &columns)).iter().all(|k| *k != "s"), "층에서 보기가 섰다");
    }

    fn e(key: &str, what: &str) -> Entry {
        Entry { key: key.into(), what: what.into(), state: None }
    }

    fn line(g: &Grid, r: usize) -> String {
        g.row(r).map(|p| format!("{}{SEP}{}", p.key, p.text)).collect::<Vec<_>>().join(&" ".repeat(GAP))
    }

    fn letters(n: u8, what: &str) -> Vec<Entry> {
        (0..n).map(|i| e(&char::from(b'a' + i).to_string(), what)).collect()
    }

    /// **격자는 열 우선으로 채우고 열마다 `:` 가 줄 선다**(moai-apsa) — which-key 처럼 한 열을 줄
    /// 상한까지 위에서 아래로 채운 뒤 다음 열로 간다. 키는 열 안에서 오른쪽 맞춤이다(doom 의 `RET`).
    #[test]
    fn the_grid_fills_columns_first_and_lines_up_the_colons() {
        let items: Vec<Entry> = ["a", "b", "SPC", "d", "e", "f", "g", "h", "i", "j"].iter().map(|k| e(k, "설명")).collect();
        let g = grid(&items, 200, 4);
        assert_eq!((g.rows, g.columns.len(), g.hidden), (4, 3, 0));
        let keys = |c: usize| g.columns[c].iter().map(|p| p.key.trim_start()).collect::<Vec<_>>();
        assert_eq!(keys(0), ["a", "b", "SPC", "d"]);
        assert_eq!(keys(1), ["e", "f", "g", "h"]);
        assert_eq!(keys(2), ["i", "j"]);
        let nth = |r: usize, k: usize| {
            let l = line(&g, r);
            l.match_indices(SEP).nth(k).map(|(i, _)| width(&l[..i]))
        };
        assert!((0..4).all(|r| nth(r, 0) == Some(3)), "첫 열의 `:` 가 줄 서지 않는다");
        assert!((0..4).all(|r| nth(r, 1) == nth(0, 1)), "둘째 열의 `:` 가 줄 서지 않는다");
        assert_eq!(line(&g, 0), "  a : 설명  e : 설명  i : 설명");
    }

    /// **줄 수는 상한에서 멈추고 열 수가 거기서 나온다** — 뿌리처럼 항목이 적으면 그만큼 낮다.
    /// 몸통 몫을 남기지 못하는 높이면 격자를 세우지 않는다.
    #[test]
    fn the_grid_caps_rows_and_derives_columns() {
        let items = letters(20, "낱말");
        let g = grid(&items, 200, MAX_ROWS);
        assert_eq!((g.rows, g.columns.len(), g.hidden), (6, 4, 0));
        assert_eq!(grid(&items[..3], 200, MAX_ROWS).rows, 3);
        assert_eq!(grid(&items, 200, 0), Grid { rows: 0, columns: Vec::new(), hidden: 20 });
        assert_eq!(grid(&[], 200, MAX_ROWS), Grid::default());
        assert_eq!(rows_for(40), MAX_ROWS);
        assert_eq!(rows_for(BODY_MIN + 1), 0, "몸통을 남기지 못하는데 격자를 세웠다");
        assert_eq!(rows_for(BODY_MIN + 3), 2);
    }

    /// **좁으면 설명을 `…` 로 자르고, 그래도 안 들면 뒤 열을 빼며 그 수를 댄다.** 한글은 두 칸으로
    /// 잰다. 어느 줄도 폭을 넘지 않고, 아주 좁아도 멈추지 않는다.
    #[test]
    fn a_narrow_grid_clips_descriptions_then_drops_columns_and_counts_them() {
        let items = letters(12, "아주 긴 한글 설명이 붙은 동작");
        let wide = grid(&items, 200, 4);
        assert_eq!((wide.columns.len(), wide.hidden), (3, 0));
        assert!(!line(&wide, 0).contains('…'), "{}", line(&wide, 0));

        let narrow = grid(&items, 60, 4);
        assert_eq!((narrow.columns.len(), narrow.hidden), (3, 0), "{narrow:?}");
        assert!(line(&narrow, 0).contains('…'), "{}", line(&narrow, 0));
        for r in 0..narrow.rows {
            assert!(width(&line(&narrow, r)) <= 60, "{}", line(&narrow, r));
        }

        let tight = grid(&items, 20, 4);
        assert_eq!((tight.columns.len(), tight.hidden), (2, 4), "{tight:?}");
        for r in 0..tight.rows {
            assert!(width(&line(&tight, r)) <= 20, "{}", line(&tight, r));
        }
        for w in [0usize, 1, 5] {
            let g = grid(&items, w, 4);
            assert_eq!(g.columns.len(), 1, "{w}칸");
            assert_eq!(g.hidden, 8, "{w}칸");
        }
    }

    /// **메뉴의 모든 길이 같은 동작을 낸다** — 바로 누르던 키가 하던 그 동작이고, 실행하면 닫힌다.
    #[test]
    fn each_menu_path_runs_its_action_and_closes() {
        let cases: [(&str, Browse, Ctx); 9] = [
            ("/", Browse::Grep, inside()),
            ("f", Browse::Filter, inside()),
            ("n", Browse::Jot, inside()),
            ("r", Browse::Reload, inside()),
            ("q", Browse::Quit, inside()),
            ("pa", Browse::Pick, inside()),
            ("pd", Browse::Unregister, layer()),
            ("tw", Browse::Worktree, inside()),
            ("tr", Browse::Raw, inside()),
        ];
        for (path, act, c) in cases {
            let mut ch = Chord::default();
            feed(&mut ch, &c, k(' '));
            let mut got = None;
            for p in path.chars() {
                got = feed(&mut ch, &c, k(p));
            }
            assert_eq!(got, Some(act), "SPC {path}");
            assert!(!open(&ch), "SPC {path} 를 실행하고도 메뉴가 열려 있다");
        }
    }

    /// **모르는 키는 무시한다** — 닫지도 않는다. Esc 가 닫고 Bksp 가 한 층 올라간다.
    #[test]
    fn unknown_keys_are_ignored_esc_closes_and_bksp_goes_up() {
        let c = inside();
        let mut ch = Chord::default();
        feed(&mut ch, &c, k(' '));
        for x in [k('x'), k('j'), k('g'), KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), k(' ')] {
            assert_eq!(feed(&mut ch, &c, x), None, "{x:?}");
            assert_eq!(title(ch.held()), "SPC", "{x:?} 가 메뉴를 옮기거나 닫았다");
        }
        feed(&mut ch, &c, k('t'));
        assert_eq!(title(ch.held()), "SPC t");
        assert_eq!(keys_of(&entries(ch.held(), &c, &[])), ["w", "r", "d"]);
        feed(&mut ch, &c, k('x'));
        assert_eq!(title(ch.held()), "SPC t", "하위 층의 모르는 키가 메뉴를 옮겼다");
        feed(&mut ch, &c, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(title(ch.held()), "SPC");
        feed(&mut ch, &c, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(!open(&ch), "뿌리의 Bksp 가 안 닫았다");
        feed(&mut ch, &c, k(' '));
        feed(&mut ch, &c, k('p'));
        assert_eq!(feed(&mut ch, &c, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), None);
        assert!(!open(&ch), "하위 층의 Esc 가 안 닫았다");
    }

    /// **켜진 것만 선다** — 층에서는 거름망·워크트리가 빠지고, 해제는 층의 목록 포커스에서만.
    /// 안 선 키는 눌러도 모르는 키다.
    #[test]
    fn entries_hide_what_is_not_enabled_here() {
        let root_in = entries(&[k(' ')], &inside(), &[]);
        let root_layer = entries(&[k(' ')], &layer(), &[]);
        // 층에서는 읽음(`SPC m`)도 안 선다(moai-j038.vna) — 층의 줄은 프로젝트라 읽을 줄이 없고, 서면
        // `SPC m a` 가 늘 "적을 것이 없다" 로 답하면서 그 프로젝트의 [NEW] 는 그대로 남는다.
        assert_eq!(keys_of(&root_layer), ["n", "r", "q", "p", "t"], "층에서 검색·거름망·읽음이 섰다");
        assert!(keys_of(&root_in).contains(&"m"), "프로젝트 안에서 읽음이 안 섰다");
        assert!(keys_of(&root_in).contains(&"f"));
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &inside(), &[])), ["a"], "프로젝트 안에서 해제가 섰다");
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &layer(), &[])), ["a", "d"]);
        let detail = Ctx { list_focus: false, ..layer() };
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &detail, &[])), ["a"], "상세 포커스에서 해제가 섰다");
        // 층에서도 상세 칸은 있다 — 숨기기(`d`)는 서고 워크트리 겹쳐 보기(`w`)만 빠진다.
        assert_eq!(keys_of(&entries(&[k(' '), k('t')], &layer(), &[])), ["r", "d"], "층에서 워크트리가 섰다");

        let mut ch = Chord::default();
        for x in [' ', 't', 'w'] {
            assert_eq!(feed(&mut ch, &layer(), k(x)), None, "층에서 SPC t w 가 돌았다");
        }
        assert_eq!(title(ch.held()), "SPC t", "안 선 키가 메뉴를 닫았다");
        let mut ch = Chord::default();
        for x in [' ', 'f'] {
            assert_eq!(feed(&mut ch, &layer(), k(x)), None);
        }
        assert_eq!(title(ch.held()), "SPC");
    }

    /// **토글은 지금 상태를 낱말로 댄다** — 색이 혼자 뜻을 지지 않는다.
    #[test]
    fn toggles_show_their_state_in_words() {
        let states = |c: Ctx| -> Vec<Option<&'static str>> { entries(&[k(' '), k('t')], &c, &[]).iter().map(|e| e.state).collect() };
        assert_eq!(states(inside()), [Some("[꺼짐]"), Some("[그리기]"), Some("[보임]")]);
        assert_eq!(states(Ctx { worktree: true, raw: true, ..inside() }), [Some("[켜짐]"), Some("[원문]"), Some("[보임]")]);
        // **상세를 숨기면 원문↔그리기가 빠진다** — 그 키는 상세의 글에만 걸려, 서 있어 봐야
        // 눌러도 화면이 그대로다(moai-ymnu 리뷰).
        assert_eq!(states(Ctx { detail: false, ..inside() }), [Some("[꺼짐]"), Some("[숨김]")]);
        assert_eq!(keys_of(&entries(&[k(' '), k('t')], &Ctx { detail: false, ..inside() }, &[])), ["w", "d"]);
    }

    /// **이름 없는 하위 접두어가 없다.** 표에 SPC 줄을 더하며 새 접두어를 만들면 여기서 멈춘다.
    /// 메뉴가 받는 Esc·Bksp 는 메뉴 줄의 키로 쓰이지 않는다 — 쓰이면 그 줄은 영영 못 누른다.
    #[test]
    fn every_prefix_in_the_menu_has_a_name() {
        for b in BROWSE.iter().filter(|b| b.seq.first() == Some(&LEADER)) {
            for n in 2..b.seq.len() {
                let seq: Vec<KeyEvent> = b.seq[..n].iter().map(|k| k.event()).collect();
                assert_ne!(group(&seq), "…", "`{}` 에 이름이 없다 — GROUPS 에 더한다", title(&seq));
            }
            for key in &b.seq[1..] {
                assert_eq!(lookup(MENU, &[key.event()]), Lookup::Unknown, "{:?}", b.seq);
            }
        }
    }
}
