//! 한 줄 글칸. 제목·검색·거름망이 다 이것이다.
//!
//! **`KeyEvent` 하나를 받아 제 상태만 바꾼다.** 터미널도 저장소도 모른다 —
//! 그래서 시험이 화면 없이 돈다. 그리는 쪽은 [`Input::view`] 가 낸 조각과
//! 커서 칸만 받아 찍는다.
//!
//! **글자는 grapheme 으로 센다.** 바이트로 세면 커서가 한글 한 자 가운데
//! 들어가고, `char` 로 세면 `e`+결합 악센트나 ZWJ 로 이은 이모지가 반으로
//! 쪼개진다. 폭은 ratatui 가 칸을 채울 때 쓰는 자([`CellWidth`])를 그대로
//! 쓴다 — 자가 갈라지면 여기서 센 커서 칸과 화면에 찍힌 글자가 어긋난다.

use ratatui::buffer::CellWidth;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    text: String,
    /// 커서의 바이트 자리. **늘 grapheme 경계에 선다** — 여기가 어긋나면
    /// `insert` 가 글자 가운데를 찔러 패닉하거나 이모지를 둘로 가른다.
    at: usize,
}

/// 칸 하나에 그릴 것. [`Input::view`] 가 낸다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View<'a> {
    /// 칸 안에 드는 글. 폭이 칸을 넘지 않고, 양 끝이 글자를 쪼개지 않는다.
    pub text: &'a str,
    /// `text` 첫 칸에서 센 커서의 칸. 칸 폭이 0 이 아니면 늘 그보다 작다.
    pub cursor: usize,
}

impl Input {
    /// 이미 적힌 글로 연다. 커서는 맨 끝에 선다 — 이어 치는 것이 가장 흔하다.
    ///
    /// **제어문자는 여기서도 걸러낸다.** 키로 들어오는 것만 막으면 파일에서
    /// 온 제목이 ESC 를 싣고 칸 안으로 들어온다. 한 줄 칸이라 줄바꿈과 탭도
    /// 뺀다 — 탭은 터미널마다 폭이 달라 커서 칸을 셀 수 없다.
    pub fn new(s: &str) -> Input {
        let text: String = s.chars().filter(|c| !c.is_control()).collect();
        let at = text.len();
        Input { text, at }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// 키 하나를 먹는다. **이 칸이 먹은 키면 참이다** — 바뀐 것이 없어도
    /// 그렇다(빈 칸의 Backspace 도 칸의 것이다). 거짓이면 Enter·Esc·Tab·
    /// Ctrl-C 처럼 칸을 든 쪽이 정할 키다.
    pub fn key(&mut self, k: KeyEvent) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        match k.code {
            // Ctrl-U 는 커서 앞만이 아니라 **전부** 지운다. 지금 `/`·`f` 가 그렇게
            // 하고 있고, 옮기면서 뜻을 바꾸지 않는다.
            KeyCode::Char('u') if ctrl => {
                self.text.clear();
                self.at = 0;
            }
            KeyCode::Char('w') if ctrl => self.rub_word(),
            // 그 밖의 Ctrl·Alt 는 글자가 아니다. raw mode 에서는 Ctrl-C 가
            // 신호로 오지 않으므로, 여기서 `c` 로 먹으면 나갈 길이 막힌다.
            _ if ctrl || alt => return false,
            KeyCode::Char(c) => {
                // 먹기는 하되 넣지는 않는다. 흘려보내면 든 쪽이 그것을 이동키로
                // 읽을 수 있다.
                if !c.is_control() {
                    self.text.insert(self.at, c);
                    self.at += c.len_utf8();
                    self.snap();
                }
            }
            KeyCode::Backspace => {
                let from = self.prev();
                self.text.replace_range(from..self.at, "");
                self.at = from;
                self.settle();
            }
            KeyCode::Delete => {
                let to = self.next();
                self.text.replace_range(self.at..to, "");
                self.settle();
            }
            KeyCode::Left => self.at = self.prev(),
            KeyCode::Right => self.at = self.next(),
            KeyCode::Home => self.at = 0,
            KeyCode::End => self.at = self.text.len(),
            _ => return false,
        }
        true
    }

    /// 폭 `width` 칸에 그릴 조각과 커서 칸.
    ///
    /// **상태를 들지 않는다.** 굴린 자리를 따로 들면 그리는 쪽이 `&mut` 을
    /// 받아야 하는데, `draw` 는 그리기만 한다. 그래서 자리는 매번 커서에서
    /// 되짚는다 — 머리부터 보여서 커서가 들면 머리부터, 안 들면 커서를 오른쪽
    /// 끝에 붙인다.
    ///
    /// 커서가 글자 위에 있으면 **그 글자까지 통째로** 칸 안에 넣는다. 한글
    /// 위에 선 커서가 반 칸만 보이면 어느 글자를 지울지 읽을 수 없다. 맨
    /// 끝이면 커서 자리로 한 칸을 비워 둔다.
    pub fn view(&self, width: usize) -> View<'_> {
        window(&self.text, self.at, width)
    }

    /// 커서 앞 grapheme 의 첫 바이트. 맨 앞이면 0.
    fn prev(&self) -> usize {
        self.text.grapheme_indices(true).map(|(i, _)| i).take_while(|&i| i < self.at).last().unwrap_or(0)
    }

    /// 커서 뒤 grapheme 이 끝나는 바이트. 맨 끝이면 글 길이.
    fn next(&self) -> usize {
        self.boundary_from(self.at + 1)
    }

    /// `from` 이상인 첫 경계.
    fn boundary_from(&self, from: usize) -> usize {
        self.text.grapheme_indices(true).map(|(i, _)| i).find(|&i| i >= from).unwrap_or(self.text.len())
    }

    /// 넣은 글자가 앞뒤를 한 grapheme 으로 이으면(ZWJ·결합 모음) 커서가 그
    /// 가운데 남는다. 이은 것의 끝으로 민다 — 앞으로 당기면 방금 친 글자
    /// 앞에 커서가 서서, 다음 글자가 순서를 뒤집는다.
    fn snap(&mut self) {
        self.at = self.boundary_from(self.at);
    }

    /// 지운 자리의 앞뒤가 한 grapheme 으로 붙으면(`🇰x🇷` 에서 `x` 를 지우면 깃발
    /// 하나가 된다) 커서가 그 가운데 남는다. 붙은 것의 **앞으로** 당긴다 —
    /// 지우기는 커서를 오른쪽으로 옮기지 않는다. 가운데 두면 화면은 커서를
    /// 붙은 것 뒤에 그리는데 다음 글자는 그 가운데 들어간다.
    fn settle(&mut self) {
        let bounds = self.text.grapheme_indices(true).map(|(i, _)| i).chain([self.text.len()]);
        self.at = bounds.take_while(|&i| i <= self.at).last().unwrap_or(0);
    }

    /// Ctrl-W. 커서 앞의 빈칸을 넘고 낱말 하나를 지운다 — 셸과 같다.
    fn rub_word(&mut self) {
        let mut from = self.at;
        let mut seen_word = false;
        for (i, g) in self.text[..self.at].grapheme_indices(true).rev() {
            let blank = g.chars().all(char::is_whitespace);
            if blank && seen_word {
                break;
            }
            seen_word |= !blank;
            from = i;
        }
        self.text.replace_range(from..self.at, "");
        self.at = from;
        self.settle();
    }
}

/// 여러 줄 칸([`super::edit::Editor`])이 줄 하나를 이 칸으로 들고 쓰는 것.
///
/// **글자를 걷는 자는 여기 하나다.** 줄마다 grapheme·폭 셈을 새로 적으면 두 칸의
/// 커서가 같은 글에서 다른 자리에 선다. 여러 줄 칸은 줄을 가르고 잇는 것만 하고,
/// 한 줄 안의 일은 [`Input::key`] 에 맡긴다.
#[cfg_attr(not(test), expect(dead_code, reason = "첫 부르는 곳은 `n` 폼의 본문이다(moai-11s4)"))]
impl Input {
    pub(super) fn at_start(&self) -> bool {
        self.at == 0
    }

    pub(super) fn at_end(&self) -> bool {
        self.at == self.text.len()
    }

    /// 커서 앞 글의 칸 수. 줄 사이를 오갈 때 겨눌 칸이다.
    ///
    /// **글자마다 잰 폭을 더한다** — [`window`] 가 커서 칸을 그렇게 잰다. 글 전체를
    /// 한 번에 재면 글자 사이의 문맥(아랍어 lam-alef 는 합쳐 한 칸)이 끼어 화면의
    /// 커서와 다른 칸을 겨누고, `u16` 을 넘는 긴 줄에서는 수가 돌아 버린다.
    pub(super) fn column(&self) -> usize {
        self.text[..self.at].graphemes(true).map(|g| g.cell_width() as usize).sum()
    }

    /// 커서를 칸 `column` 에 세운다. 그 칸이 글자 가운데면 **그 글자 앞에** 선다 —
    /// 한글 위에서 한 칸 오른쪽을 겨눠도 그 한글을 가리킨다. 줄보다 멀면 끝이다.
    ///
    /// 폭 0 인 글자(ZWSP·BOM·홀로 선 결합 악센트)가 그 칸에 서 있으면 **그 앞에**
    /// 선다. 지나쳐 서면 `seek(0)` 이 줄 머리가 아니게 되어, 줄을 넘어온 커서의
    /// Backspace 가 줄을 잇지 않고 그 글자를 지운다.
    pub(super) fn seek(&mut self, column: usize) {
        let mut used = 0;
        for (i, g) in self.text.grapheme_indices(true) {
            let w = g.cell_width() as usize;
            if used + w > column || (w == 0 && used == column) {
                self.at = i;
                return;
            }
            used += w;
        }
        self.at = self.text.len();
    }

    /// 커서 뒤를 떼어 새 칸으로 낸다. 떼인 칸의 커서는 맨 앞이다 — Enter 가 줄을
    /// 나눈 뒤 커서는 새 줄 머리에 선다.
    pub(super) fn split_off(&mut self) -> Input {
        Input { text: self.text.split_off(self.at), at: 0 }
    }

    /// 뒤에 칸 하나를 잇는다. 커서는 **이은 자리**에 선다 — 앞 줄 끝의 Backspace 도
    /// 줄 끝의 Delete 도 거기다. 이어서 한 글자가 되면(`e` 뒤에 결합 악센트로
    /// 시작하는 줄) 붙은 것의 앞으로 당긴다([`Input::settle`]).
    pub(super) fn append(&mut self, tail: Input) {
        self.at = self.text.len();
        self.text.push_str(&tail.text);
        self.settle();
    }

    /// 커서가 없는 줄로 그릴 조각. 머리부터 칸이 차는 데까지다.
    pub(super) fn head(&self, width: usize) -> &str {
        window(&self.text, 0, width).text
    }
}

/// `text` 를 폭 `width` 칸에 그릴 조각. 커서 `at` 이 칸 안에 들게 민다 — 뜻은
/// [`Input::view`] 에 적었다. 커서 없는 줄은 `at` 을 0 으로 불러 머리를 얻는다.
fn window(text: &str, at: usize, width: usize) -> View<'_> {
    let cells: Vec<(usize, usize)> = text.grapheme_indices(true).map(|(i, g)| (i, g.cell_width() as usize)).collect();
    let here = cells.iter().position(|&(i, _)| i >= at).unwrap_or(cells.len());
    let need = cells.get(here).map_or(1, |&(_, w)| w.max(1));

    let mut start = 0;
    let mut before: usize = cells[..here].iter().map(|&(_, w)| w).sum();
    while start < here && before + need > width {
        before -= cells[start].1;
        start += 1;
    }
    let mut end = start;
    let mut used = 0;
    while end < cells.len() && used + cells[end].1 <= width {
        used += cells[end].1;
        end += 1;
    }
    let byte = |k: usize| cells.get(k).map_or(text.len(), |&(i, _)| i);
    View { text: &text[byte(start)..byte(end)], cursor: before }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(i: &mut Input, code: KeyCode) -> bool {
        i.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(i: &mut Input, c: char) -> bool {
        i.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    fn typed(s: &str) -> Input {
        let mut i = Input::default();
        for c in s.chars() {
            press(&mut i, KeyCode::Char(c));
        }
        i
    }

    /// 커서를 `|` 로 찍은 글. 시험이 기대를 한눈에 읽게 한다.
    fn shown(i: &Input) -> String {
        format!("{}|{}", &i.text[..i.at], &i.text[i.at..])
    }

    #[test]
    fn typing_goes_where_the_cursor_is() {
        let mut i = typed("ac");
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Char('b'));
        assert_eq!(shown(&i), "ab|c");
        press(&mut i, KeyCode::Home);
        press(&mut i, KeyCode::Char('>'));
        assert_eq!(shown(&i), ">|abc");
        press(&mut i, KeyCode::End);
        assert_eq!(shown(&i), ">abc|");
    }

    /// 끝에서 더 가려 해도, 빈 칸을 지워도 아무 일이 없다. 패닉도 없다.
    /// **그래도 칸의 키다** — 흘려보내면 든 쪽이 Backspace 를 "한 층 위로" 로
    /// 읽는다.
    #[test]
    fn the_edges_hold_still() {
        let mut e = Input::default();
        assert!(press(&mut e, KeyCode::Backspace));
        assert!(press(&mut e, KeyCode::Delete));
        assert!(press(&mut e, KeyCode::Left));
        assert!(press(&mut e, KeyCode::Right));
        assert!(ctrl(&mut e, 'w'));
        assert_eq!(shown(&e), "|");

        let mut i = typed("ab");
        press(&mut i, KeyCode::Right);
        press(&mut i, KeyCode::Delete);
        assert_eq!(shown(&i), "ab|");
        press(&mut i, KeyCode::Home);
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Backspace);
        assert_eq!(shown(&i), "|ab");
    }

    #[test]
    fn backspace_and_delete_take_either_side() {
        let mut i = typed("abcd");
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Backspace);
        assert_eq!(shown(&i), "a|cd");
        press(&mut i, KeyCode::Delete);
        assert_eq!(shown(&i), "a|d");
    }

    /// Ctrl-U 는 커서가 가운데 있어도 전부 지운다 — 옮기기 전 `/`·`f` 의 뜻이다.
    #[test]
    fn ctrl_u_clears_everything() {
        let mut i = typed("status=todo");
        press(&mut i, KeyCode::Home);
        press(&mut i, KeyCode::Right);
        assert!(ctrl(&mut i, 'u'));
        assert_eq!(shown(&i), "|");
    }

    #[test]
    fn ctrl_w_takes_one_word_before_the_cursor() {
        let mut i = typed("tag=a  status=todo  ");
        ctrl(&mut i, 'w');
        assert_eq!(shown(&i), "tag=a  |", "끝의 빈칸과 낱말을 함께 지운다");
        ctrl(&mut i, 'w');
        assert_eq!(shown(&i), "|");

        let mut i = typed("한글 낱말뒤");
        press(&mut i, KeyCode::Left);
        ctrl(&mut i, 'w');
        assert_eq!(shown(&i), "한글 |뒤", "커서 뒤는 남는다");
    }

    /// 한글 한 자는 한 걸음이고 한 번에 지워진다. 바이트로 걸으면 커서가
    /// 글자 가운데 서고, 그 자리에 넣는 순간 패닉한다.
    #[test]
    fn hangul_moves_and_erases_whole() {
        let mut i = typed("가나다");
        press(&mut i, KeyCode::Left);
        assert_eq!(shown(&i), "가나|다");
        press(&mut i, KeyCode::Char('x'));
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Backspace);
        assert_eq!(shown(&i), "가|x다");
        press(&mut i, KeyCode::Delete);
        assert_eq!(shown(&i), "가|다");
    }

    /// `e` + 결합 악센트, ZWJ 로 이은 가족 이모지는 `char` 여럿이지만 한 글자다.
    #[test]
    fn clusters_are_one_step() {
        let mut i = Input::new("e\u{301}👨\u{200d}👩");
        press(&mut i, KeyCode::Left);
        assert_eq!(shown(&i), "e\u{301}|👨\u{200d}👩");
        press(&mut i, KeyCode::Left);
        assert_eq!(shown(&i), "|e\u{301}👨\u{200d}👩");
        press(&mut i, KeyCode::Delete);
        assert_eq!(shown(&i), "|👨\u{200d}👩");
    }

    /// 넣은 글자가 앞뒤를 하나로 이으면 커서는 이은 것의 끝으로 간다.
    /// 가운데 남으면 다음 걸음이 경계가 아닌 자리에서 시작한다.
    #[test]
    fn a_joiner_pushes_the_cursor_past_what_it_joined() {
        let mut i = Input::new("👨👩");
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Char('\u{200d}'));
        assert_eq!(shown(&i), "👨\u{200d}👩|");
        press(&mut i, KeyCode::Char('!'));
        assert_eq!(shown(&i), "👨\u{200d}👩!|");
    }

    /// 지워서 앞뒤가 한 글자로 붙으면 커서는 붙은 것 앞에 선다. 가운데 남으면
    /// 화면은 커서를 깃발 뒤에 그리는데 다음 글자는 깃발 가운데 들어간다.
    #[test]
    fn erasing_what_kept_two_apart_leaves_the_cursor_on_a_boundary() {
        let mut i = Input::new("🇰x🇷");
        press(&mut i, KeyCode::Left);
        press(&mut i, KeyCode::Backspace);
        assert_eq!(shown(&i), "|🇰🇷");
        press(&mut i, KeyCode::Char('a'));
        assert_eq!(shown(&i), "a|🇰🇷");

        let mut i = Input::new("🇰x🇷");
        press(&mut i, KeyCode::Home);
        press(&mut i, KeyCode::Right);
        press(&mut i, KeyCode::Delete);
        assert_eq!(shown(&i), "|🇰🇷");
    }

    /// 제어문자는 칸에 안 들어온다 — 키로도, 처음 적힌 글로도. `moai-ovrg`
    /// 가 본문에 낸 판단과 같고, 한 줄 칸이라 탭·줄바꿈도 막는다.
    #[test]
    fn control_characters_never_get_in() {
        let mut i = Input::default();
        for c in ['\u{1b}', '\t', '\n', '\u{7}', '\u{9b}'] {
            assert!(press(&mut i, KeyCode::Char(c)), "{c:?} 가 칸 밖으로 샜다");
        }
        assert_eq!(i.text(), "");
        assert_eq!(Input::new("a\tb\u{1b}[2Jc\n").text(), "ab[2Jc");
    }

    /// 칸이 안 먹는 키는 든 쪽에 돌려준다. Ctrl-C 를 먹으면 나갈 길이 막힌다.
    #[test]
    fn keys_that_belong_to_the_owner_pass_through() {
        let mut i = typed("ab");
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Tab, KeyCode::Up, KeyCode::Down] {
            assert!(!press(&mut i, code), "{code:?} 를 칸이 먹었다");
        }
        assert!(!ctrl(&mut i, 'c'));
        assert!(!i.key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT)));
        assert!(i.key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT)), "대문자가 막혔다");
        assert_eq!(shown(&i), "abC|");
    }

    #[test]
    fn a_wide_box_shows_everything_from_the_start() {
        let i = typed("abc");
        assert_eq!(i.view(20), View { text: "abc", cursor: 3 });
        assert_eq!(Input::default().view(5), View { text: "", cursor: 0 });
    }

    /// 칸보다 긴 글은 커서를 따라 밀린다. 끝에서는 커서 자리 한 칸을 남긴다.
    #[test]
    fn long_text_scrolls_to_keep_the_cursor_in_the_box() {
        let mut i = typed("abcdefghij");
        assert_eq!(i.view(5), View { text: "ghij", cursor: 4 });
        press(&mut i, KeyCode::Home);
        assert_eq!(i.view(5), View { text: "abcde", cursor: 0 });
        press(&mut i, KeyCode::Right);
        press(&mut i, KeyCode::Right);
        assert_eq!(i.view(5), View { text: "abcde", cursor: 2 });
    }

    /// 한글과 이모지는 두 칸이다. 커서가 선 글자는 통째로 보이고, 오른쪽 끝에
    /// 반만 걸치는 글자는 아예 빠진다.
    #[test]
    fn wide_characters_count_two_and_are_never_halved() {
        let mut i = typed("가나다라");
        assert_eq!(i.view(5), View { text: "다라", cursor: 4 });
        press(&mut i, KeyCode::Home);
        press(&mut i, KeyCode::Right);
        assert_eq!(i.view(3), View { text: "나", cursor: 0 }, "커서 밑 글자가 잘렸다");
        assert_eq!(i.view(1), View { text: "", cursor: 0 }, "한 칸에 두 칸 글자를 넣었다");
        press(&mut i, KeyCode::Home);
        assert_eq!(i.view(5), View { text: "가나", cursor: 0 });
        assert_eq!(typed("😀").view(10), View { text: "😀", cursor: 2 });
    }

    /// **폭은 칸으로, 걸음은 글자로 센다.** 한글·이모지·깃발·ZWJ 이모지는 두 칸에
    /// 한 걸음, 결합 악센트는 앞 글자에 붙어 한 칸에 한 걸음이다. 탭은 칸에 못
    /// 들어오므로 폭에도 걸음에도 없다 — 들어오면 터미널마다 폭이 달라 커서 칸을
    /// 셀 수 없다. 칸 수는 커서를 그리는 자(`view`)와 줄 사이를 겨누는 자
    /// (`column`·`seek`)가 같아야 한다.
    #[test]
    fn width_counts_cells_and_steps_count_glyphs() {
        for (s, cells, glyphs) in [
            ("abc", 3, 3),
            ("가나", 4, 2),
            ("😀", 2, 1),
            ("👨\u{200d}👩", 2, 1),
            ("🇰🇷", 2, 1),
            ("e\u{301}", 1, 1),
            ("a\tb", 2, 2),
            ("가😀\tx", 5, 3),
        ] {
            let mut i = Input::new(s);
            assert_eq!((i.column(), i.view(40).cursor), (cells, cells), "{s:?}: 끝의 칸");
            for step in 1..=glyphs {
                assert!(press(&mut i, KeyCode::Left));
                assert_eq!(i.at_start(), step == glyphs, "{s:?}: ← {step} 번");
            }
            i.seek(cells);
            assert!(i.at_end(), "{s:?}: {cells} 칸을 겨눴는데 끝이 아니다");
            for step in 1..=glyphs {
                assert!(press(&mut i, KeyCode::Backspace));
                assert_eq!(i.text().is_empty(), step == glyphs, "{s:?}: Backspace {step} 번");
            }
        }
    }

    /// 어떤 글, 어떤 칸, 어떤 커서에서도: 조각은 칸을 넘지 않고, 경계에서
    /// 잘리며, 커서는 칸 안에 서고 조각 속 제 자리를 가리킨다.
    #[test]
    fn every_view_fits_and_points_at_the_cursor() {
        for s in ["", "abc", "가나다라마바사", "a가b나c다", "😀x👨\u{200d}👩e\u{301}한", "  빈 칸 "] {
            let mut i = Input::new(s);
            press(&mut i, KeyCode::Home);
            let bounds: Vec<usize> = i.text.grapheme_indices(true).map(|(k, _)| k).chain([i.text.len()]).collect();
            loop {
                for width in 0..12 {
                    let v = i.view(width);
                    let start = v.text.as_ptr() as usize - i.text.as_ptr() as usize;
                    assert!(v.text.cell_width() as usize <= width, "{s:?} @ {width}: {v:?}");
                    assert!(width == 0 || v.cursor < width, "{s:?} @ {width}: 커서가 칸 밖 {v:?}");
                    assert!(bounds.contains(&start) && bounds.contains(&(start + v.text.len())), "{s:?} 쪼갬");
                    if width > 0 {
                        assert!(start <= i.at, "{s:?} @ {width}: 커서가 조각 앞에 있다 {v:?}");
                        assert_eq!(i.text[start..i.at].cell_width() as usize, v.cursor, "{s:?} @ {width}");
                    }
                }
                if i.at == i.text.len() {
                    break;
                }
                press(&mut i, KeyCode::Right);
            }
        }
    }

    /// `src/tui` 에서 조각이 **아닌** 파일과 그 까닭. 여기 없는 파일은 전부 조각으로
    /// 보고 [`components_know_neither_the_terminal_nor_the_store`] 가 훑는다 — 새
    /// 파일이 조각이 아니면 이유를 적어 여기 더한다. 목록을 조각 쪽으로 두면 새 조각이
    /// 목록에 안 올라 조용히 안 훑인다.
    const NOT_COMPONENTS: [(&str, &str); 2] =
        [("mod.rs", "App — 저장소(Repo)를 들고 키를 칸에 나눈다"), ("draw.rs", "그림 — Frame 에 찍는다")];

    /// 조각 코드가 **밖으로 뻗는 길**(소문자로 시작하는 `::` 경로) 가운데 허락되지 않은 것.
    ///
    /// 금지 목록이 아니라 **허락 목록**이다. `Frame`·`Repo` 를 막는 목록은
    /// `ratatui::Frame` 을 한 줄에 풀어 쓰거나 `super::` 로 돌아 들어오는 길을 못
    /// 막는다. 허락하는 것은 키 모양(`crossterm::event` 의 **타입**만 — `read`·`poll`
    /// 은 터미널을 읽는다), 폭 셈(`CellWidth`·`unicode_*`·`crate::text`), 옆 조각,
    /// 그리고 부수효과 없는 `std` 다. 경로 없이 화면에 찍는 `println!`·`dbg!` 같은
    /// 매크로도 여기서 잡는다 — 조각이 찍으면 대체 화면이 깨진다.
    fn foreign(code: &str, components: &[&str]) -> Vec<String> {
        const PRIMITIVES: [&str; 18] = [
            "self", "char", "str", "bool", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128",
            "isize", "f32", "f64",
        ];
        const IMPURE_STD: [&str; 8] = ["io", "fs", "os", "env", "process", "net", "thread", "time"];
        const IMPURE_MACROS: [&str; 5] = ["print", "println", "eprint", "eprintln", "dbg"];
        let macros = code.match_indices('!').filter_map(|(k, _)| {
            let name = code[..k].rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).next()?;
            (!code[k + 1..].starts_with('=') && IMPURE_MACROS.contains(&name)).then(|| format!("{name}!"))
        });
        paths(code)
            .into_iter()
            .filter(|p| {
                let mut seg = p.trim_start_matches("::").split("::");
                let (root, second) = (seg.next().unwrap_or(""), seg.next().unwrap_or(""));
                let allowed = match root {
                    r if r.starts_with(char::is_uppercase) || PRIMITIVES.contains(&r) => true,
                    "ratatui" => {
                        p == "ratatui::buffer::CellWidth"
                            || p.strip_prefix("ratatui::crossterm::event::").is_some_and(|t| t.starts_with(char::is_uppercase))
                    }
                    "unicode_segmentation" | "unicode_width" => true,
                    "crate" => second == "text",
                    "super" => components.contains(&second),
                    "std" | "core" | "alloc" => !IMPURE_STD.contains(&second),
                    _ => false,
                };
                !allowed
            })
            .chain(macros)
            .collect()
    }

    /// 코드에 적힌 `::` 경로를 전부 편다. `use a::{B, c::D}` 는 `a::B`·`a::c::D` 둘이다 —
    /// 묶음을 안 펴면 `use ratatui::{Frame, ..}` 가 `ratatui::` 하나로만 보인다.
    fn paths(code: &str) -> Vec<String> {
        let part = |c: char| c.is_alphanumeric() || c == '_' || c == ':';
        let mut out = Vec::new();
        let mut rest = code;
        while let Some(start) = rest.find(part) {
            let before = rest[..start].chars().last();
            rest = &rest[start..];
            let (run, after) = rest.split_at(rest.find(|c| !part(c)).unwrap_or(rest.len()));
            rest = after;
            if run.starts_with("::") && before == Some('>') {
                continue; // `Vec::<u8>::new` 의 뒷토막 — 머리는 `<` 앞에서 이미 봤다
            }
            // turbofish 는 `::` 만 떼고 머리를 본다. 통째로 넘기면
            // `std::fs::read::<&str>` 가 조용히 빠진다
            let run = if after.starts_with('<') { run.strip_suffix("::").unwrap_or(run) } else { run };
            if !run.contains("::") {
                continue; // 경로가 아니거나 `collect::<T>` 의 turbofish
            }
            if run.ends_with("::") && after.starts_with('{') {
                let (mut depth, mut from, mut items) = (0, 1, Vec::new());
                for (k, c) in after.char_indices() {
                    match c {
                        '{' => depth += 1,
                        ',' if depth == 1 => {
                            items.push(&after[from..k]);
                            from = k + 1;
                        }
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                items.push(&after[from..k]);
                                rest = &after[k + 1..];
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                for item in items.iter().map(|i| i.trim()).filter(|i| !i.is_empty()) {
                    out.extend(paths(&format!("{run}{item}")));
                }
                continue;
            }
            out.push(run.to_string());
        }
        out
    }

    /// 허락 목록이 제 일을 한다 — 풀어 쓴 `Frame`, 묶음 속 `Frame`, 저장소, 터미널을
    /// 읽는 함수, `super::` 로 돌아 든 `App` 을 잡고, 말로 적은 것·키 타입·옆 조각·
    /// turbofish·부수효과 없는 `std` 는 그냥 둔다. 이게 없으면 훑기가 늘 비어 나와도
    /// 아무도 모른다.
    #[test]
    fn the_purity_scan_catches_what_it_should() {
        let planted = "use ratatui::Frame;\n\
            use ratatui::{Frame, crossterm::event::{KeyCode, read}};\n\
            fn f(_: &crate::store::Repo) { std::io::stdout(); }\n\
            use super::App;\n\
            use super::scroll::Scroll;\n\
            use ratatui::crossterm::event::KeyEvent;\n\
            let v = xs.iter().map(char::is_whitespace).collect::<Vec<_>>();\n\
            std::mem::take(&mut v); KeyCode::Up; usize::MAX; Vec::<u8>::with_capacity(1);\n\
            <[u8]>::len(&[]); if a != b { std::fs::read::<&str>(p); std::os::unix::fs::symlink(a, b); }\n\
            eprintln!(\"x\"); std::println!(); format!(\"{a}\");";
        assert_eq!(
            foreign(planted, &["scroll"]),
            [
                "ratatui::Frame",
                "ratatui::Frame",
                "ratatui::crossterm::event::read",
                "crate::store::Repo",
                "std::io::stdout",
                "super::App",
                "std::fs::read",
                "std::os::unix::fs::symlink",
                "eprintln!",
                "println!"
            ]
        );
    }

    /// **조각은 터미널도 저장소도 모른다**(moai-0k1p). 조각이 `Frame` 이나 `Repo` 를
    /// 알기 시작하면 `KeyEvent` 를 넣고 상태를 보는 시험이 곧 못 쓰게 되고, 그러면
    /// 다음 화면은 또 손으로 짠다. 그 첫 줄(`use`)이 들어오는 순간 여기서 이름을 대며
    /// 멈춘다. 시험 모듈은 안 훑는다 — 시험이 무엇을 쓰든 조각 코드의 계약은 아니다.
    #[test]
    fn components_know_neither_the_terminal_nor_the_store() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui");
        let entries: Vec<std::fs::DirEntry> =
            std::fs::read_dir(dir).expect("src/tui 를 못 읽었다").map(|e| e.expect("src/tui 를 못 읽었다")).collect();
        // 이 훑기는 한 층만 본다. 폴더로 든 조각(`src/tui/list/mod.rs`)을 말없이 건너뛰면
        // 조각을 목록에 안 올려 조용히 안 훑는 것과 같은 구멍이다
        let nested: Vec<_> = entries.iter().filter(|e| e.path().is_dir()).map(|e| e.file_name()).collect();
        assert!(nested.is_empty(), "src/tui 밑에 폴더가 생겼다: {nested:?} — 폴더 속 조각도 훑게 이 시험을 고친다");
        let mut files: Vec<String> = entries
            .iter()
            .map(|e| e.file_name().into_string().expect("src/tui 에 UTF-8 이 아닌 파일 이름"))
            .filter(|n| n.ends_with(".rs") && !NOT_COMPONENTS.iter().any(|(f, _)| f == n))
            .collect();
        files.sort();
        for known in ["edit.rs", "input.rs", "scroll.rs"] {
            assert!(files.iter().any(|f| f == known), "{known} 가 훑기에서 빠졌다: {files:?}");
        }
        let names: Vec<&str> = files.iter().map(|f| f.trim_end_matches(".rs")).collect();
        for f in &files {
            let src = std::fs::read_to_string(format!("{dir}/{f}")).expect("조각 파일을 못 읽었다");
            let body = src.split("\n#[cfg(test)]\nmod tests").next().unwrap_or("");
            let code: Vec<&str> = body.lines().map(|l| l.split("//").next().unwrap_or("")).collect();
            let bad = foreign(&code.join("\n"), &names);
            assert!(
                bad.is_empty(),
                "src/tui/{f} 가 조각 밖으로 뻗는다: {bad:?}\n\
                 조각이면 KeyEvent 와 잰 값만 받게 고치고, 조각이 아니면 NOT_COMPONENTS 에 까닭과 함께 적는다"
            );
        }
    }
}
