//! `n` 이 여는 생각 담기 폼(moai-11s4). 제목 한 줄([`Input`]) + 본문 여러 줄([`Editor`]).
//!
//! **키를 받아 제 상태만 바꾸고 무엇을 할지만 돌려준다**([`Act`]). 파일을 쓰는 것도
//! 폼을 닫는 것도 든 쪽(`App`)이 한다 — 여기서 저장소를 알면 시험이 임시 저장소 없이
//! 못 돌고, 쓰기가 [`super::App::write`] 한 구멍 밖으로 샌다.
//!
//! **담는 값이 0 에 가까워야 담는다**(moai-c0ns). 제목이 비었을 때만 거절하고 나머지는
//! 아무것도 안 묻는다 — 우선순위도 에픽도 태그도 없다.

use super::edit::Editor;
use super::input::Input;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 폼 안의 어느 칸이 키를 먹는가. **폼 안의 포커스다** — 탐색기의 [`super::Pane`] 은
/// 폼이 열려 있는 동안 그대로 남고, 닫으면 보던 칸으로 돌아간다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Field {
    #[default]
    Title,
    Body,
}

impl Field {
    /// 칸이 둘이라 앞으로 가든 뒤로 가든 다른 칸이다.
    pub fn other(self) -> Field {
        match self {
            Field::Title => Field::Body,
            Field::Body => Field::Title,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Form {
    pub title: Input,
    pub body: Editor,
    pub field: Field,
    /// 마지막 저장이 거절된 까닭(제목이 빔). **치면 걷힌다** — 누군지 묻는 칸과 같다.
    pub error: Option<String>,
    /// Esc 를 눌러 "적던 것을 버릴까" 를 묻는 중인가.
    pub leaving: bool,
}

/// 폼이 든 쪽에 시키는 것.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// 폼 안에서 끝났다.
    Stay,
    /// 담는다. **제목이 있을 때만 나온다** — 빈 제목은 여기서 거절한다.
    Save,
    /// 닫는다. 적던 것이 있었으면 이미 한 번 물었다.
    Close,
}

/// 빈 제목을 거절하는 말. 무엇을 하면 되는지까지 댄다.
pub const EMPTY_TITLE: &str = "제목이 비었다 — 제목 한 줄이면 담긴다";

impl Form {
    /// 적던 것이 있는가. **빈칸뿐인 것은 적은 것이 아니다** — 날아가도 잃을 것이 없는데
    /// 묻으면 Esc 가 두 번 누르는 키가 된다.
    pub fn is_blank(&self) -> bool {
        self.title.text().trim().is_empty() && self.body.text().trim().is_empty()
    }

    /// 담을 제목. 앞뒤 빈칸을 뗀다 — CLI `add` 와 같다.
    pub fn title(&self) -> String {
        self.title.text().trim().to_string()
    }

    /// 담을 본문. **끝의 빈 줄과 빈칸은 뗀다** — Enter 를 한 번 더 친 것이 파일에 빈 줄로
    /// 남으면 상세가 그만큼 빈 줄을 그린다. 다 떼고 빈 글이면 본문이 없는 것이다.
    pub fn body(&self) -> Option<String> {
        let text = self.body.text();
        let text = text.trim_end();
        (!text.is_empty()).then(|| text.to_string())
    }

    /// 키 하나. Ctrl-C 는 여기 오기 전에 든 쪽이 받는다 — 어느 모드에서든 나가는 길이다.
    ///
    /// - **Ctrl-S·F2** 담기. 제목이 비면 거절하고 제목 칸으로 간다
    /// - **Tab·Shift-Tab** 제목 ↔ 본문
    /// - **Enter** 본문에서는 줄을 나누고, 제목에서는 본문으로 간다. **Enter 는 어디서도
    ///   담지 않는다** — 본문에서 손에 익은 Enter 가 제목에서 반쯤 적은 생각을 파일에
    ///   적으면, 탐색기에는 지우는 길이 없다
    /// - **Esc** 닫기. 적던 것이 있으면 한 번 묻고, `y` 만 버린다. 다른 키는 폼으로
    ///   돌아가며 **글자로 들어가지 않는다** — 물음을 못 보고 친 글자가 엉뚱한 자리에
    ///   찍히면 안 된다
    pub fn key(&mut self, k: KeyEvent) -> Act {
        if self.leaving {
            self.leaving = false;
            let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
            return match k.code {
                KeyCode::Char('y' | 'Y') if plain => Act::Close,
                _ => Act::Stay,
            };
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Char('s' | 'S') if ctrl => return self.save(),
            KeyCode::F(2) => return self.save(),
            // 터미널에 따라 `Shift-Tab` 이 `BackTab` 으로도 Shift 붙은 `Tab` 으로도 온다.
            KeyCode::Tab | KeyCode::BackTab => {
                self.field = self.field.other();
                return Act::Stay;
            }
            KeyCode::Esc if self.is_blank() => return Act::Close,
            KeyCode::Esc => {
                self.leaving = true;
                return Act::Stay;
            }
            _ => {}
        }
        let eaten = match self.field {
            Field::Title => self.title.key(k),
            Field::Body => self.body.key(k),
        };
        if eaten {
            self.error = None;
        } else if self.field == Field::Title && k.code == KeyCode::Enter && !ctrl {
            self.field = Field::Body;
        }
        Act::Stay
    }

    fn save(&mut self) -> Act {
        if self.title().is_empty() {
            self.error = Some(EMPTY_TITLE.into());
            self.field = Field::Title;
            return Act::Stay;
        }
        Act::Save
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(f: &mut Form, code: KeyCode) -> Act {
        f.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn type_in(f: &mut Form, s: &str) {
        for c in s.chars() {
            press(f, KeyCode::Char(c));
        }
    }

    #[test]
    fn tab_moves_between_title_and_body_both_ways() {
        let mut f = Form::default();
        type_in(&mut f, "제목");
        press(&mut f, KeyCode::Tab);
        type_in(&mut f, "본문");
        assert_eq!((f.title.text(), f.body.text().as_str(), f.field), ("제목", "본문", Field::Body));
        f.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(f.field, Field::Title);
        f.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(f.field, Field::Body, "Shift 붙은 Tab 이 안 돌았다");
    }

    /// Enter 는 본문에서 줄을 나누고 제목에서는 본문으로 간다. 어디서도 담지 않는다.
    #[test]
    fn enter_never_saves() {
        let mut f = Form::default();
        type_in(&mut f, "제목");
        assert_eq!(press(&mut f, KeyCode::Enter), Act::Stay);
        assert_eq!(f.field, Field::Body, "제목의 Enter 가 본문으로 안 갔다");
        type_in(&mut f, "한 줄");
        assert_eq!(press(&mut f, KeyCode::Enter), Act::Stay);
        type_in(&mut f, "두 줄");
        assert_eq!((f.title(), f.body().as_deref()), ("제목".to_string(), Some("한 줄\n두 줄")));
    }

    #[test]
    fn ctrl_s_and_f2_save_only_with_a_title() {
        for save in [KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL), KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)] {
            let mut f = Form::default();
            press(&mut f, KeyCode::Tab);
            type_in(&mut f, "본문만");
            assert_eq!(f.key(save), Act::Stay, "{save:?}: 빈 제목으로 담았다");
            assert_eq!((f.error.as_deref(), f.field), (Some(EMPTY_TITLE), Field::Title), "{save:?}");
            // 빈칸뿐인 제목도 빈 제목이다
            type_in(&mut f, "   ");
            assert_eq!(f.key(save), Act::Stay);
            // 치면 까닭이 걷힌다
            type_in(&mut f, "생각");
            assert_eq!(f.error, None, "치기 시작했는데 까닭이 남았다");
            assert_eq!(f.key(save), Act::Save, "{save:?}");
            assert_eq!(f.title(), "생각");
        }
    }

    /// 빈 폼은 Esc 한 번에 닫힌다. 적던 것이 있으면 한 번 묻고 `y` 만 버린다.
    #[test]
    fn esc_asks_once_only_when_something_was_typed() {
        let mut f = Form::default();
        assert_eq!(press(&mut f, KeyCode::Esc), Act::Close);
        let mut f = Form::default();
        type_in(&mut f, "  ");
        assert_eq!(press(&mut f, KeyCode::Esc), Act::Close, "빈칸뿐인데 물었다");

        for typed_in in [Field::Title, Field::Body] {
            let mut f = Form { field: typed_in, ..Form::default() };
            type_in(&mut f, "적던 것");
            assert_eq!(press(&mut f, KeyCode::Esc), Act::Stay, "한 키에 날아갔다");
            assert!(f.leaving);
            // 다른 키는 돌아가고 글자로 안 들어간다
            assert_eq!(press(&mut f, KeyCode::Char('n')), Act::Stay);
            assert!(!f.leaving);
            assert_eq!(f.title.text().len() + f.body.text().len(), "적던 것".len(), "물음 뒤의 키가 글자로 들어갔다");
            // Esc 두 번도 버리지 않는다 — 버리는 것은 y 하나다
            press(&mut f, KeyCode::Esc);
            assert_eq!(press(&mut f, KeyCode::Esc), Act::Stay, "Esc 두 번에 버렸다");
            press(&mut f, KeyCode::Esc);
            assert_eq!(f.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)), Act::Stay, "Ctrl-Y 로 버렸다");
            press(&mut f, KeyCode::Esc);
            assert_eq!(press(&mut f, KeyCode::Char('y')), Act::Close);
        }
    }

    #[test]
    fn trailing_blank_lines_are_not_part_of_the_body() {
        let mut f = Form::default();
        press(&mut f, KeyCode::Tab);
        for code in [KeyCode::Enter, KeyCode::Char('a'), KeyCode::Enter, KeyCode::Char(' '), KeyCode::Enter] {
            press(&mut f, code);
        }
        assert_eq!(f.body().as_deref(), Some("\na"), "앞의 빈 줄은 적은 것이다");
        let mut f = Form::default();
        press(&mut f, KeyCode::Tab);
        press(&mut f, KeyCode::Enter);
        assert_eq!(f.body(), None);
        assert!(f.is_blank());
    }
}
