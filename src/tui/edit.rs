//! 여러 줄 글칸. 본문이 이것이다.
//!
//! **줄 하나가 한 줄 칸([`Input`])이다.** 글자를 넣고 지우고 걷는 것, 제어문자를
//! 막는 것, 폭을 세는 것은 전부 그쪽이 한다. 여기는 줄을 가르고(Enter) 잇고
//! (줄 머리의 Backspace·줄 끝의 Delete) 줄 사이를 오가는(↑↓) 것만 한다 — 한 줄
//! 칸의 규칙을 여기 다시 적으면 제목과 본문의 커서가 같은 글에서 다른 자리에 선다.
//!
//! **순수하다.** `KeyEvent` 를 받아 제 상태만 바꾸고, 그리는 쪽은 [`Editor::fit`]
//! 으로 높이를 넣은 뒤 [`Editor::view`] 가 낸 조각만 찍는다(moai-0k1p).
//!
//! **저장 키를 모른다.** Enter 는 줄을 나눈다 — Enter 가 저장이면 여러 줄을 못
//! 적는다. 무엇이 저장인지는 칸을 든 쪽이 정하고(moai-11s4), 여기는 Ctrl·Alt
//! 조합과 Esc·Tab 을 돌려준다.
//!
//! **긴 줄을 접지 않는다(v1).** 접으면 커서 자리가 화면 줄과 글 줄로 갈라지고
//! ↑↓ 가 어느 쪽을 걷는지부터 정해야 한다. 대신 커서가 선 줄만 가로로 밀린다 —
//! [`Input::view`] 그대로다. 다른 줄은 머리부터 칸이 차는 데까지 보인다(nano 와
//! 같다). 줄마다 같은 칸만큼 밀면 좁은 칸에서 두 칸 글자가 반으로 잘린다.

use super::input::Input;
use super::scroll::{PAGE, Scroll};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// 늘 한 줄 이상이다. 빈 글도 빈 줄 하나다 — 커서가 설 줄이 있어야 한다.
    lines: Vec<Input>,
    /// 커서가 선 줄.
    row: usize,
    /// ↑↓ 가 겨누는 칸. **짧은 줄을 지나도 잃지 않는다** — 긴 줄 40 칸에서 빈 줄을
    /// 지나 다시 긴 줄로 오면 40 칸에 선다. 매번 지금 칸에서 재면 짧은 줄 하나가
    /// 커서를 머리로 끌어당긴다. ↑↓ 가 아닌 키를 먹으면 버린다.
    goal: Option<usize>,
    scroll: Scroll,
}

impl Default for Editor {
    fn default() -> Editor {
        Editor { lines: vec![Input::default()], row: 0, goal: None, scroll: Scroll::default() }
    }
}

/// 칸 하나에 그릴 것. [`Editor::view`] 가 낸다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View<'a> {
    /// 보이는 줄. 칸 높이를 넘지 않고, 줄마다 폭이 칸을 넘지 않는다.
    pub lines: Vec<&'a str>,
    /// 칸 안에서 센 커서의 (칸, 줄). 칸이 비었거나 커서 줄이 안 보이면 `None` —
    /// [`Editor::fit`] 을 같은 높이로 먼저 불렀으면 칸이 빈 때뿐이다.
    pub cursor: Option<(usize, usize)>,
}

impl Editor {
    /// 이미 적힌 글로 연다. 커서는 맨 끝에 선다 — [`Input::new`] 와 같다.
    ///
    /// **줄바꿈은 줄을 가르는 데만 쓴다.** 그 밖의 제어문자는 줄마다
    /// [`Input::new`] 가 걸러낸다 — `\r\n` 의 `\r` 도, 탭도 거기서 빠진다.
    pub fn new(s: &str) -> Editor {
        let lines: Vec<Input> = s.split('\n').map(Input::new).collect();
        let row = lines.len() - 1;
        Editor { lines, row, ..Editor::default() }
    }

    /// 적힌 글. 줄은 `\n` 으로 잇는다.
    pub fn text(&self) -> String {
        let lines: Vec<&str> = self.lines.iter().map(Input::text).collect();
        lines.join("\n")
    }

    /// 굴린 자리. 테두리에 `↓ 12줄` 을 붙이는 쪽이 읽는다([`Scroll::mark`]).
    pub fn scroll(&self) -> &Scroll {
        &self.scroll
    }

    /// 키 하나를 먹는다. **이 칸이 먹은 키면 참이다** — 끝에서 더 가려는 ↑ 처럼
    /// 바뀐 것이 없어도 그렇다. 거짓이면 Esc·Tab·Ctrl-C·저장 키처럼 칸을 든 쪽이
    /// 정할 키다.
    ///
    /// Ctrl-U·Ctrl-W·Alt-Backspace 는 한 줄 칸의 것을 그대로 받는다 — Ctrl-U 가 지우는 것은
    /// **커서가 선 줄**이다. 본문 전부를 한 키에 날리면 되돌릴 길이 없다. Ctrl-W·Alt-Backspace 도
    /// 커서 줄 안에서만 지운다 — 줄 첫머리에서는 윗줄과 잇지 않는다(그냥 Backspace 만 잇는다).
    pub fn key(&mut self, k: KeyEvent) -> bool {
        let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let last = self.lines.len() - 1;
        let line = &mut self.lines[self.row];
        match k.code {
            KeyCode::Up if plain => return self.walk(-1),
            KeyCode::Down if plain => return self.walk(1),
            KeyCode::PageUp if plain => return self.walk(-(PAGE as isize)),
            KeyCode::PageDown if plain => return self.walk(PAGE as isize),
            KeyCode::Enter if plain => {
                let rest = line.split_off();
                self.row += 1;
                self.lines.insert(self.row, rest);
            }
            KeyCode::Backspace if plain && line.at_start() && self.row > 0 => {
                let line = self.lines.remove(self.row);
                self.row -= 1;
                self.lines[self.row].append(line);
            }
            KeyCode::Delete if plain && line.at_end() && self.row < last => {
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].append(next);
            }
            // 줄 머리의 ← 는 윗줄 끝으로, 줄 끝의 → 는 아랫줄 머리로 간다. 줄 안에서
            // 멈추면 줄을 넘는 길이 ↑↓ 와 Home/End 두 번뿐이다.
            KeyCode::Left if plain && line.at_start() && self.row > 0 => {
                self.row -= 1;
                self.lines[self.row].seek(usize::MAX);
            }
            KeyCode::Right if plain && line.at_end() && self.row < last => {
                self.row += 1;
                self.lines[self.row].seek(0);
            }
            _ => {
                if !line.key(k) {
                    return false;
                }
            }
        }
        self.goal = None;
        true
    }

    /// 붙여 넣은 글을 커서 자리에 넣는다(moai-od9q). **글의 줄바꿈이 줄을 가른다** — Enter 를
    /// 친 것과 같은 자리에서. 그래서 커서는 넣은 글 뒤, 원래 커서 뒤에 있던 글 앞에 선다.
    ///
    /// 탭은 **빈칸 넷**이다. 한 줄 칸([`Input::paste`])은 빈칸 하나지만 여기는 마크다운 본문
    /// 이라 탭 하나가 네 칸이다 — 버리면 들여 쓴 줄이 붙고, 하나로 줄이면 들여쓰기의 뜻이
    /// 바뀐다. 적힌 본문을 여는 [`Editor::new`] 는 탭을 버리는데, 그쪽은 파일에서 온 글을
    /// 칸이 못 세는 글자로 막는 것이고 여기는 사람이 붙인 글의 뜻을 옮기는 것이다.
    ///
    /// 줄은 **한 번에 끼운다.** 줄마다 `Vec::insert` 로 끼우면 긴 글 하나를 붙이는 값이
    /// 줄 수의 제곱이 된다.
    pub fn paste(&mut self, s: &str) {
        let text = super::input::line_breaks(s);
        let mut pieces = text.split('\n').map(|p| p.replace('\t', "    "));
        let tail = self.lines[self.row].split_off();
        self.lines[self.row].insert(&pieces.next().unwrap_or_default());
        let mut more: Vec<Input> = pieces
            .map(|p| {
                let mut line = Input::default();
                line.insert(&p);
                line
            })
            .collect();
        // 원래 커서 뒤에 있던 글은 마지막 줄 뒤에 붙는다 — 커서는 그 이은 자리에 선다.
        match more.last_mut() {
            Some(last) => last.append(tail),
            None => self.lines[self.row].append(tail),
        }
        let at = self.row + 1;
        self.row += more.len();
        self.lines.splice(at..at, more);
        self.goal = None;
    }

    /// 줄 사이를 `delta` 줄 걷는다. 첫 줄·끝 줄 밖으로는 안 나가고, 거기서 누른
    /// 것도 먹는다 — 흘려보내면 든 쪽이 ↑ 를 "윗칸으로" 로 읽어 커서가 칸 밖으로 샌다.
    fn walk(&mut self, delta: isize) -> bool {
        let goal = *self.goal.get_or_insert(self.lines[self.row].column());
        let to = self.row.saturating_add_signed(delta).min(self.lines.len() - 1);
        if to != self.row {
            self.row = to;
            self.lines[to].seek(goal);
        }
        true
    }

    /// 그림이 잰 높이를 들이고 **커서 줄이 보이게 굴린다.** [`Editor::view`] 전에
    /// 같은 높이로 부른다.
    ///
    /// 키가 굴리지 않는 까닭: 키를 받을 때는 높이를 모른다. 커서만 옮겨 두고 그릴
    /// 때 [`Scroll::reveal`] 로 가장 적게 굴린다 — 칸 안에서 움직이는 커서는 화면을
    /// 흔들지 않는다.
    pub fn fit(&mut self, height: usize) {
        self.scroll.fit(height, self.lines.len());
        self.scroll.reveal(self.row);
    }

    /// 폭 `width`·높이 `height` 칸에 그릴 것.
    ///
    /// 커서 줄은 [`Input::view`] 로 커서를 따라 밀리고, 나머지는 머리부터 보인다.
    pub fn view(&self, width: usize, height: usize) -> View<'_> {
        let top = self.scroll.offset().min(self.lines.len());
        let bottom = (top + height).min(self.lines.len());
        let shown = |r: usize| {
            if r == self.row { self.lines[r].view(width).text } else { self.lines[r].head(width) }
        };
        let lines = (top..bottom).map(shown).collect();
        let cursor = (width > 0 && (top..bottom).contains(&self.row))
            .then(|| (self.lines[self.row].view(width).cursor, self.row - top));
        View { lines, cursor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(e: &mut Editor, code: KeyCode) -> bool {
        e.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn typed(s: &str) -> Editor {
        let mut e = Editor::default();
        for c in s.chars() {
            press(&mut e, if c == '\n' { KeyCode::Enter } else { KeyCode::Char(c) });
        }
        e
    }

    /// 커서를 `|` 로 찍은 글. 줄은 `\n` 이다.
    fn shown(e: &Editor) -> String {
        let mut before = e.lines[e.row].clone();
        let after = before.split_off();
        let mut out: Vec<&str> = e.lines.iter().map(Input::text).collect();
        let here = format!("{}|{}", before.text(), after.text());
        out[e.row] = &here;
        out.join("\n")
    }

    #[test]
    fn empty_is_one_empty_line() {
        let mut e = Editor::default();
        assert_eq!((e.text(), shown(&e)), (String::new(), "|".to_string()));
        for code in [KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right, KeyCode::Backspace, KeyCode::Delete] {
            assert!(press(&mut e, code), "{code:?} 를 칸이 안 먹었다");
        }
        assert_eq!(shown(&e), "|");
        e.fit(5);
        assert_eq!(e.view(10, 5), View { lines: vec![""], cursor: Some((0, 0)) });
        assert_eq!(Editor::new(""), Editor::default());
    }

    #[test]
    fn enter_splits_and_the_cursor_starts_the_new_line() {
        let mut e = typed("abcd");
        press(&mut e, KeyCode::Left);
        press(&mut e, KeyCode::Left);
        press(&mut e, KeyCode::Enter);
        assert_eq!(shown(&e), "ab\n|cd");
        press(&mut e, KeyCode::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(shown(&e), "ab\ncd\n|");
        press(&mut e, KeyCode::Enter);
        assert_eq!(e.text(), "ab\ncd\n\n");
    }

    #[test]
    fn backspace_at_the_head_joins_with_the_line_above() {
        let mut e = typed("ab\ncd");
        press(&mut e, KeyCode::Home);
        press(&mut e, KeyCode::Backspace);
        assert_eq!(shown(&e), "ab|cd");
        press(&mut e, KeyCode::Home);
        assert!(press(&mut e, KeyCode::Backspace), "첫 줄 머리의 Backspace 가 샜다");
        assert_eq!(shown(&e), "|abcd");
    }

    #[test]
    fn delete_at_the_end_joins_the_next_line() {
        let mut e = typed("ab\ncd\nef");
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::End);
        press(&mut e, KeyCode::Delete);
        assert_eq!(shown(&e), "ab|cd\nef");
        press(&mut e, KeyCode::Down);
        press(&mut e, KeyCode::End);
        assert!(press(&mut e, KeyCode::Delete), "끝 줄 끝의 Delete 가 샜다");
        assert_eq!(shown(&e), "abcd\nef|");
    }

    /// 이어서 한 글자가 되면 커서는 붙은 것 앞에 선다 — 한 줄 칸의 지우기와 같다.
    #[test]
    fn joining_into_one_cluster_leaves_the_cursor_on_a_boundary() {
        let mut e = Editor::new("e\n\u{301}x");
        press(&mut e, KeyCode::Home);
        press(&mut e, KeyCode::Backspace);
        assert_eq!(shown(&e), "|e\u{301}x");
        press(&mut e, KeyCode::Char('a'));
        assert_eq!(e.text(), "ae\u{301}x");
    }

    #[test]
    fn left_and_right_cross_line_edges() {
        let mut e = typed("ab\ncd");
        press(&mut e, KeyCode::Home);
        press(&mut e, KeyCode::Left);
        assert_eq!(shown(&e), "ab|\ncd");
        press(&mut e, KeyCode::Right);
        assert_eq!(shown(&e), "ab\n|cd");

        // 글의 양 끝에서는 넘을 줄이 없다. 서 있되 칸이 먹는다 — 흘려보내면 든 쪽이
        // → 를 "옆 칸으로" 로 읽는다
        press(&mut e, KeyCode::End);
        assert!(press(&mut e, KeyCode::Right), "끝 줄 끝의 → 가 샜다");
        assert_eq!(shown(&e), "ab\ncd|");
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Home);
        assert!(press(&mut e, KeyCode::Left), "첫 줄 머리의 ← 가 샜다");
        assert_eq!(shown(&e), "|ab\ncd");
    }

    /// ↑↓ 가 겨누는 칸은 이모지·ZWJ 이모지를 **두 칸**으로 센다. 두 칸 글자의 오른쪽
    /// 반을 겨누면 그 글자 앞에 선다. 탭은 줄에 못 들어오므로 칸 수에도 없다.
    #[test]
    fn the_goal_column_counts_emoji_as_two_and_tabs_as_nothing() {
        let mut e = Editor::new("😀😀x\n👨\u{200d}👩z\na\tbcdef");
        press(&mut e, KeyCode::Home);
        for _ in 0..3 {
            press(&mut e, KeyCode::Right);
        }
        assert_eq!(shown(&e), "😀😀x\n👨\u{200d}👩z\nabc|def", "걸러진 탭이 한 걸음을 먹었다");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "😀😀x\n👨\u{200d}👩z|\nabcdef", "ZWJ 이모지를 두 칸으로 안 셌다");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "😀|😀x\n👨\u{200d}👩z\nabcdef", "3 칸은 둘째 이모지의 오른쪽 반이다");
        press(&mut e, KeyCode::Down);
        press(&mut e, KeyCode::Down);
        assert_eq!(shown(&e), "😀😀x\n👨\u{200d}👩z\nabc|def");

        press(&mut e, KeyCode::Home);
        press(&mut e, KeyCode::Right);
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "😀😀x\n|👨\u{200d}👩z\nabcdef", "1 칸은 첫 글자의 오른쪽 반이다");
    }

    /// 칸이 글보다 크면 **어느 키로도 안 구른다.** 줄이 다 보이고 커서는 제 줄에 선다.
    #[test]
    fn a_box_taller_than_the_text_never_scrolls() {
        let mut e = Editor::new("a\nb\nc");
        for code in [KeyCode::PageDown, KeyCode::End, KeyCode::Down, KeyCode::PageUp, KeyCode::Up, KeyCode::Home] {
            assert!(press(&mut e, code), "{code:?} 를 칸이 안 먹었다");
            e.fit(10);
            assert_eq!((e.scroll().offset(), e.scroll().mark(crate::i18n::Lang::Ko)), (0, None), "{code:?} 에 굴렀다");
            let v = e.view(5, 10);
            assert_eq!(v.lines, vec!["a", "b", "c"], "{code:?}");
            assert_eq!(v.cursor.map(|(_, y)| y), Some(e.row), "{code:?}: 커서가 제 줄에 없다");
        }
    }

    /// ↑↓ 는 **처음 겨눈 칸**을 짧은 줄 너머로 가져간다. 한글은 두 칸이라 한 칸
    /// 어긋난 자리를 겨누면 그 글자 앞에 선다.
    #[test]
    fn up_and_down_keep_the_goal_column_across_short_and_wide_lines() {
        let mut e = typed("abcdef\nx\n\n가나다라\nabcdefgh");
        press(&mut e, KeyCode::Home);
        for _ in 0..5 {
            press(&mut e, KeyCode::Right);
        }
        assert_eq!(shown(&e), "abcdef\nx\n\n가나다라\nabcde|fgh");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "abcdef\nx\n\n가나|다라\nabcdefgh", "5 칸은 다 의 한가운데다");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "abcdef\nx\n|\n가나다라\nabcdefgh");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "abcdef\nx|\n\n가나다라\nabcdefgh");
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "abcde|f\nx\n\n가나다라\nabcdefgh", "짧은 줄을 지나며 겨눈 칸을 잃었다");
        assert!(press(&mut e, KeyCode::Up), "첫 줄의 ↑ 가 샜다");
        assert_eq!(shown(&e), "abcde|f\nx\n\n가나다라\nabcdefgh");
        press(&mut e, KeyCode::PageDown);
        assert_eq!(shown(&e), "abcdef\nx\n\n가나다라\nabcde|fgh");
        assert!(press(&mut e, KeyCode::Down), "끝 줄의 ↓ 가 샜다");

        // 다른 키를 먹으면 겨눈 칸은 지금 칸에서 다시 잰다
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Left);
        press(&mut e, KeyCode::Down);
        assert_eq!(shown(&e), "abcdef\nx\n\n가나다라\nab|cdefgh", "← 로 선 2 칸을 겨눈다");
    }

    #[test]
    fn home_and_end_stay_on_the_line() {
        let mut e = typed("ab\ncd\nef");
        press(&mut e, KeyCode::Up);
        press(&mut e, KeyCode::Home);
        assert_eq!(shown(&e), "ab\n|cd\nef");
        press(&mut e, KeyCode::End);
        assert_eq!(shown(&e), "ab\ncd|\nef");
    }

    /// 제어문자는 줄에 안 들어온다. **줄바꿈도 글자로는 안 들어온다** — 줄은
    /// Enter 로만 갈린다. 처음 적힌 글의 `\n` 은 줄을 가르고 나머지는 빠진다.
    #[test]
    fn control_characters_never_get_in_and_newlines_come_only_from_enter() {
        let mut e = Editor::default();
        for c in ['\n', '\r', '\t', '\u{1b}', '\u{9b}'] {
            assert!(press(&mut e, KeyCode::Char(c)), "{c:?} 가 칸 밖으로 샜다");
        }
        assert_eq!((e.text(), e.lines.len()), (String::new(), 1));
        let e = Editor::new("a\tb\r\n\u{1b}[2Jc\n");
        assert_eq!(e.text(), "ab\n[2Jc\n");
        assert_eq!(shown(&e), "ab\n[2Jc\n|");
    }

    /// **붙여 넣은 글의 줄바꿈은 줄을 가른다**(moai-od9q) — Enter 를 친 것과 같은 자리에서.
    /// `\r\n`·`\r` 도 줄바꿈이다(터미널은 붙여넣기 속 줄바꿈을 `\r` 로 보내기도 한다). 탭은
    /// 빈칸 넷 — 본문은 마크다운이라 탭 하나가 네 칸이고, 버리면 들여 쓴 줄이 붙는다. 그 밖의
    /// 제어문자는 키와 같이 걸러낸다. 커서는 넣은 글 뒤, 원래 뒤에 있던 글 앞에 선다.
    #[test]
    fn a_paste_splits_lines_where_its_own_newlines_are() {
        let mut e = typed("ab");
        press(&mut e, KeyCode::Left);
        e.paste("x\r\ny\rz\tw\u{1b}");
        assert_eq!(shown(&e), "ax\ny\nz    w|b");
        e.paste("\n");
        assert_eq!(shown(&e), "ax\ny\nz    w\n|b");
        // ↑ 가 겨누는 칸은 붙여넣기 뒤 커서에서 새로 잰다
        press(&mut e, KeyCode::Up);
        assert_eq!(shown(&e), "ax\ny\n|z    w\nb");
    }

    /// Esc·Tab·Ctrl·Alt 조합은 든 쪽의 것이다. 저장 키가 무엇이 되든 여기서 안 먹는다.
    #[test]
    fn keys_that_belong_to_the_owner_pass_through() {
        let mut e = typed("ab\ncd");
        for code in [KeyCode::Esc, KeyCode::Tab, KeyCode::F(2)] {
            assert!(!press(&mut e, code), "{code:?} 를 칸이 먹었다");
        }
        for m in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            for code in [KeyCode::Enter, KeyCode::Char('s'), KeyCode::Char('c'), KeyCode::Up] {
                assert!(!e.key(KeyEvent::new(code, m)), "{m:?}+{code:?} 를 칸이 먹었다");
            }
        }
        assert!(!e.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL)), "Ctrl+Backspace 를 칸이 먹었다");
        assert_eq!(shown(&e), "ab\ncd|");
        // **Alt-Backspace 는 한 줄 칸과 같이 커서 줄의 낱말 하나를 지운다**(moai-979m) — 본문도 줄의 키를
        // `Input::key` 에 맡기므로 Ctrl-W·Ctrl-U 처럼 칸의 것이다.
        let mut words = typed("ab\ncd ef");
        assert!(words.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT)), "본문 칸이 Alt-Backspace 를 안 먹었다");
        assert_eq!(shown(&words), "ab\ncd |");
        assert!(e.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)));
        assert_eq!(shown(&e), "ab\n|", "Ctrl-U 가 커서 줄 밖을 지웠다");
    }

    /// 커서가 칸 밖으로 나갈 때만 가장 적게 굴린다. 줄이 줄면 끝으로 당긴다.
    #[test]
    fn vertical_scroll_follows_the_cursor_and_clamps() {
        let mut e = typed("0\n1\n2\n3\n4\n5\n6\n7\n8\n9");
        e.fit(3);
        assert_eq!(e.view(5, 3), View { lines: vec!["7", "8", "9"], cursor: Some((1, 2)) });
        press(&mut e, KeyCode::Up);
        e.fit(3);
        assert_eq!(e.view(5, 3).cursor, Some((1, 1)), "칸 안에서 움직였는데 굴렀다");
        for _ in 0..3 {
            press(&mut e, KeyCode::Up);
        }
        e.fit(3);
        assert_eq!(e.view(5, 3), View { lines: vec!["5", "6", "7"], cursor: Some((1, 0)) });
        assert_eq!(e.scroll().mark(crate::i18n::Lang::Ko).as_deref(), Some("↑ 5줄 · ↓ 2줄"));

        // 칸이 커지면 끝을 지난 자리를 자른다
        e.fit(20);
        assert_eq!(e.view(5, 20).lines.len(), 10);
        assert_eq!(e.scroll().offset(), 0);

        // 줄을 지워 짧아지면 빈 줄을 보이며 서 있지 않는다
        let mut e = typed("0\n1\n2\n3\n4");
        e.fit(2);
        assert_eq!(e.scroll().offset(), 3);
        for _ in 0..4 {
            press(&mut e, KeyCode::Home);
            press(&mut e, KeyCode::Backspace);
        }
        e.fit(2);
        assert_eq!(shown(&e), "0|1234");
        assert_eq!(e.view(5, 2), View { lines: vec!["01234"], cursor: Some((1, 0)) });
    }

    /// 커서 줄만 가로로 밀린다. 다른 줄은 머리부터, 칸이 차는 데까지다.
    #[test]
    fn only_the_cursor_line_scrolls_sideways() {
        let mut e = typed("가나다라마\nabcdefghij");
        e.fit(2);
        assert_eq!(e.view(5, 2), View { lines: vec!["가나", "ghij"], cursor: Some((4, 1)) });
        press(&mut e, KeyCode::Up);
        e.fit(2);
        assert_eq!(e.view(5, 2), View { lines: vec!["라마", "abcde"], cursor: Some((4, 0)) }, "↑ 는 10 칸을 겨눠 끝에 선다");
        assert_eq!(e.view(0, 2).cursor, None);
        assert_eq!(e.view(5, 0), View { lines: vec![], cursor: None });
    }

    /// 어떤 글, 어떤 칸, 어떤 커서에서도: 줄 수와 폭이 칸을 넘지 않고, 커서는
    /// 칸 안에서 제 줄을 가리킨다.
    #[test]
    fn every_view_fits_and_points_at_the_cursor() {
        let text = "가나다\n\nabc😀def\ne\u{301}👨\u{200d}👩\n  빈 칸 \nx";
        let mut e = Editor::new(text);
        let moves = [KeyCode::Up, KeyCode::Left, KeyCode::Up, KeyCode::End, KeyCode::Down, KeyCode::Home, KeyCode::Right];
        for (step, code) in moves.iter().cycle().take(60).enumerate() {
            press(&mut e, *code);
            for height in 0..5 {
                e.fit(height);
                for width in 0..9 {
                    let v = e.view(width, height);
                    assert!(v.lines.len() <= height, "{step} {width}x{height}: {v:?}");
                    for l in &v.lines {
                        assert!(crate::text::width(l) <= width, "{step} {width}x{height}: {v:?}");
                    }
                    if width > 0 && height > 0 {
                        let (x, y) = v.cursor.unwrap_or_else(|| panic!("{step} {width}x{height}: 커서가 안 보인다"));
                        assert!(x < width && y < height, "{step} {width}x{height}: {v:?}");
                        assert_eq!(v.lines[y], e.lines[e.row].view(width).text);
                    }
                }
            }
        }
        assert_eq!(e.text(), text);
    }

    /// 폭 0 인 글자로 시작하는 줄(ZWSP·BOM·홀로 선 결합 악센트)에서도 → 는 줄
    /// **머리**에 선다. 머리를 지나쳐 서면 곧이은 Backspace 가 줄을 잇지 않고 그
    /// 글자를 지운다.
    #[test]
    fn crossing_into_a_line_that_starts_with_a_zero_width_glyph_lands_on_its_head() {
        for head in ["\u{200b}", "\u{feff}", "\u{301}"] {
            let mut e = Editor::new(&format!("ab\n{head}x"));
            press(&mut e, KeyCode::Up);
            press(&mut e, KeyCode::End);
            press(&mut e, KeyCode::Right);
            assert_eq!(shown(&e), format!("ab\n|{head}x"), "{head:?}");
            press(&mut e, KeyCode::Up);
            press(&mut e, KeyCode::Home);
            press(&mut e, KeyCode::Down);
            assert_eq!(shown(&e), format!("ab\n|{head}x"), "{head:?}: ↓ 가 0 칸을 겨눴는데 머리를 지났다");
        }
    }

    /// ↑↓ 가 겨누는 칸은 **그리는 자와 같은 자**로 잰다. 글 전체의 폭은 글자마다 잰
    /// 폭의 합과 다를 수 있다(아랍어 lam-alef 는 합쳐 한 칸) — 그러면 화면의 커서와
    /// 다른 칸을 겨눈다.
    #[test]
    fn the_goal_column_is_the_drawn_cursor_column() {
        let mut e = Editor::new("abcd\nلا");
        let drawn = e.lines[e.row].view(10).cursor;
        press(&mut e, KeyCode::Up);
        assert_eq!(e.lines[e.row].view(10).cursor, drawn);
    }
}
