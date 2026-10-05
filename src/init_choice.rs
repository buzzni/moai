//! `moai init` 이 처음 심을 때 고르는 것들(moai-zynt.a7y). **순수하다** — 터미널도 파일도 모른다.
//!
//! 길은 하나다: 플래그와 선택 상자가 저마다 [`Choices`] 를 만들고, 같은 [`resolve`] 를 지나 [`Plan`] 이
//! 된다. `init::run` 은 `Plan` 만 받는다 — 그래서 화면으로 고른 것과 플래그로 준 것이 다른 결과를 낼
//! 길이 구조로 없다.
//!
//! **저장하는 것은 사람이 고른 값뿐이다.** 칸마다 `Option` 이고 `None` 은 "기본값을 따른다" 다.
//! 기본값·보이는가·잠겼는가는 고른 값에서 그때마다 다시 셈한다 — 파생값을 저장하지 않는다는 저장소의
//! 규약을 화면에도 그대로 건다.
//!
//! **칸 사이의 규칙은 한 방향으로만 흐른다**: 추적 → 안내 → 설치 → 드라이버(moai-zynt.a7y 노트,
//! 2026-10-06 사용자 결정). 앞 칸은 뒤 칸의 기본값·보임·잠김을 정할 수 있고 뒤 칸은 앞 칸을 못
//! 건드린다 — 그래서 [`resolve`] 는 그 차례로 한 번 훑으면 답이 난다. 거스르는 규칙이 필요해지면
//! 그때가 설계를 다시 볼 때다. 지금 선 칸(접두어·안내·드라이버) 사이에는 아직 규칙이 없다.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 사람이 고른 값. `None` 은 고르지 않았다 — 기본값을 따른다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choices {
    /// id 접두어. `None` 이면 `run` 이 디렉터리 이름에서 짓는다 — 파일 시스템을 알아야 해서 여기서
    /// 셈하지 않는다.
    pub prefix: Option<String>,
    /// AGENTS.md 에 안내 블록을 쓰는가.
    pub agents: Option<bool>,
    /// `.git/config` 에 머지 드라이버를 심는가.
    pub driver: Option<bool>,
}

impl Choices {
    /// 플래그가 고른 것. **플래그는 끄는 쪽만 있다** — 켜는 쪽은 기본값이라, 준 적 없는 것과 같다.
    pub fn from_flags(prefix: Option<&str>, no_agents: bool, no_driver: bool) -> Choices {
        Choices {
            prefix: prefix.map(str::to_string),
            agents: no_agents.then_some(false),
            driver: no_driver.then_some(false),
        }
    }

    /// `self` 가 고른 칸은 `self` 의 것, 나머지는 `under` 의 것. 플래그(`self`)를 화면(`under`) 위에 얹는다.
    pub fn over(&self, under: &Choices) -> Choices {
        Choices {
            prefix: self.prefix.clone().or_else(|| under.prefix.clone()),
            agents: self.agents.or(under.agents),
            driver: self.driver.or(under.driver),
        }
    }

    /// 묻지 않아도 되는가 — 모든 칸이 이미 골라졌다. **접두어도 센다**: 나중에 못 바꾸는 값이라
    /// 기본값(디렉터리 이름)을 그대로 쓰더라도 한 번 보이는 값어치가 있다.
    pub fn complete(&self) -> bool {
        self.prefix.is_some() && self.agents.is_some() && self.driver.is_some()
    }
}

/// 고르지 않은 칸이 따를 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub agents: bool,
    pub driver: bool,
}

/// 선택 상자가 미리 골라 두는 값.
pub const SCREEN: Defaults = Defaults { agents: true, driver: true };

/// 터미널이 아닌 곳(에이전트·스크립트)과 `--yes` 의 값. **지금까지의 `init` 과 바이트째 같아야 한다**
/// (2026-10-06 사용자 결정) — 에이전트가 부르던 결과를 이 묶음이 지킨다. 화면의 기본값이 달라져도
/// 이쪽은 안 따라간다.
pub const PLAIN: Defaults = Defaults { agents: true, driver: true };

/// 빈칸 없는 계획. `init::run` 은 이것만 받고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// `None` 이면 디렉터리 이름에서 짓는다(위 [`Choices::prefix`]).
    pub prefix: Option<String>,
    pub agents: bool,
    pub driver: bool,
}

/// 고른 값과 기본값으로 계획을 낸다. 규칙이 서면 위 머리글의 차례(추적 → 안내 → 설치 → 드라이버)로
/// 여기에 적는다 — 앞 칸을 먼저 정하고, 뒤 칸은 이미 정한 앞 칸만 읽는다.
pub fn resolve(c: &Choices, d: &Defaults) -> Plan {
    let agents = c.agents.unwrap_or(d.agents);
    let driver = c.driver.unwrap_or(d.driver);
    Plan { prefix: c.prefix.clone(), agents, driver }
}

/// 화면의 한 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Prefix,
    Agents,
    Driver,
}

/// 화면에 서는 차례. 규칙의 차례(머리글)와 맞춘다 — 앞 칸을 고르면 아래 칸이 바뀌는 쪽이 읽기 쉽다.
pub const FIELDS: [Field; 3] = [Field::Prefix, Field::Agents, Field::Driver];

/// 키 하나를 받은 뒤 부르는 쪽이 할 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// 화면에 머문다.
    Stay,
    /// 심는다. 접두어를 재는 것은 부르는 쪽이다 — 틀렸으면 [`Form::problem`] 을 세우고 머문다.
    Plant,
    /// 아무것도 안 쓰고 그만둔다.
    Stop,
}

/// 선택 상자의 상태. **조각이다**(`tui/picker.rs` 꼴) — 키를 받아 무엇을 할지만 돌려준다. 그리는 쪽과
/// 심는 쪽은 부르는 쪽(`init`)이다.
#[derive(Debug, Clone)]
pub struct Form {
    /// 플래그가 고른 것. 이 칸들은 잠긴다.
    pub fixed: Choices,
    /// 화면에서 고른 것.
    pub picked: Choices,
    pub defaults: Defaults,
    /// 디렉터리 이름에서 지은 접두어 — 접두어 칸이 처음 보이는 글이다.
    pub suggested: String,
    /// 접두어 칸에 지금 적힌 글.
    pub typed: String,
    pub cursor: usize,
    /// 심으려다 접두어가 틀려 멈춘 까닭. 접두어를 고치면 걷는다.
    pub problem: Option<String>,
}

impl Form {
    pub fn new(fixed: Choices, defaults: Defaults, suggested: String) -> Form {
        let typed = fixed.prefix.clone().unwrap_or_else(|| suggested.clone());
        let mut form = Form { fixed, picked: Choices::default(), defaults, suggested, typed, cursor: 0, problem: None };
        // 잠기지 않은 첫 칸에 선다 — 플래그로 접두어를 줬으면 그 칸에 서 봐야 고칠 것이 없다.
        form.cursor = FIELDS.iter().position(|f| !form.locked(*f)).unwrap_or(0);
        form
    }

    /// 플래그가 고른 칸인가.
    pub fn locked(&self, f: Field) -> bool {
        match f {
            Field::Prefix => self.fixed.prefix.is_some(),
            Field::Agents => self.fixed.agents.is_some(),
            Field::Driver => self.fixed.driver.is_some(),
        }
    }

    /// 지금 고른 것 — 플래그를 화면 위에 얹는다. 접두어는 적힌 글이 지은 값과 같으면 `None` 으로 둔다:
    /// `run` 이 그때만 "디렉터리 이름이 길어 줄였다" 를 알린다.
    pub fn choices(&self) -> Choices {
        let mut picked = self.picked.clone();
        if self.typed != self.suggested {
            picked.prefix = Some(self.typed.clone());
        }
        self.fixed.over(&picked)
    }

    pub fn plan(&self) -> Plan {
        resolve(&self.choices(), &self.defaults)
    }

    /// 한 칸의 실제 값 — 켜기·끄기 칸만. 그리는 쪽이 이것으로 `(•)` 를 고른다.
    pub fn on(&self, f: Field) -> bool {
        let plan = self.plan();
        match f {
            Field::Agents => plan.agents,
            Field::Driver => plan.driver,
            Field::Prefix => true,
        }
    }

    pub fn field(&self) -> Field {
        FIELDS[self.cursor]
    }

    pub fn key(&mut self, k: KeyEvent) -> Act {
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            return match k.code {
                KeyCode::Char('c') | KeyCode::Char('d') => Act::Stop,
                _ => Act::Stay,
            };
        }
        match k.code {
            KeyCode::Esc => return Act::Stop,
            KeyCode::Enter => return Act::Plant,
            KeyCode::Up | KeyCode::BackTab => self.step(-1),
            KeyCode::Down | KeyCode::Tab => self.step(1),
            _ => self.edit(k.code),
        }
        Act::Stay
    }

    /// 잠긴 칸을 건너 옮긴다. 끝에서 멈춘다 — 감아 돌면 몇 칸 안 되는 화면에서 어디 섰는지 잃는다.
    fn step(&mut self, by: isize) {
        let mut at = self.cursor as isize;
        loop {
            at += by;
            if at < 0 || at >= FIELDS.len() as isize {
                return;
            }
            if !self.locked(FIELDS[at as usize]) {
                self.cursor = at as usize;
                return;
            }
        }
    }

    fn edit(&mut self, code: KeyCode) {
        let f = self.field();
        if self.locked(f) {
            return;
        }
        match f {
            Field::Prefix => {
                match code {
                    KeyCode::Backspace => {
                        self.typed.pop();
                    }
                    // 받는 글자를 거르지 않는다 — 틀린 글자는 심을 때 까닭과 함께 잰다. 여기서 말없이
                    // 삼키면 대문자를 친 사람은 왜 안 들어가는지 모른다.
                    KeyCode::Char(c) if !c.is_control() => self.typed.push(c),
                    _ => return,
                }
                self.problem = None;
            }
            Field::Agents | Field::Driver => {
                // 켜는 값이 왼쪽, 끄는 값이 오른쪽에 그려진다 — 화살표는 그 자리로 가고, 띄어쓰기는 뒤집는다.
                let on = match code {
                    KeyCode::Left => true,
                    KeyCode::Right => false,
                    KeyCode::Char(' ') => !self.on(f),
                    _ => return,
                };
                match f {
                    Field::Agents => self.picked.agents = Some(on),
                    Field::Driver => self.picked.driver = Some(on),
                    Field::Prefix => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn form(fixed: Choices) -> Form {
        Form::new(fixed, SCREEN, "moai".to_string())
    }

    /// 플래그 셋의 모든 조합이 화면에서 키로 고른 것과 같은 계획을 낸다 — 길이 하나라는 약속을 잰다.
    #[test]
    fn a_flag_and_the_same_pick_on_screen_make_one_plan() {
        for no_agents in [false, true] {
            for no_driver in [false, true] {
                for prefix in [None, Some("abc")] {
                    let by_flags = resolve(&Choices::from_flags(prefix, no_agents, no_driver), &SCREEN);
                    let mut f = form(Choices::default());
                    if let Some(p) = prefix {
                        f.typed.clear();
                        p.chars().for_each(|c| _ = f.key(press(KeyCode::Char(c))));
                    }
                    f.key(press(KeyCode::Down));
                    if no_agents {
                        f.key(press(KeyCode::Right));
                    }
                    f.key(press(KeyCode::Down));
                    if no_driver {
                        f.key(press(KeyCode::Right));
                    }
                    assert_eq!(f.plan(), by_flags, "prefix {prefix:?} no_agents {no_agents} no_driver {no_driver}");
                }
            }
        }
    }

    /// 터미널이 아닌 `init` 은 지금까지와 같다 — 안내 블록도 드라이버도 심는다.
    #[test]
    fn plain_defaults_keep_what_init_always_did() {
        assert_eq!(resolve(&Choices::default(), &PLAIN), Plan { prefix: None, agents: true, driver: true });
    }

    #[test]
    fn a_flag_locks_its_row_and_the_cursor_skips_it() {
        let mut f = form(Choices::from_flags(None, true, false));
        assert!(f.locked(Field::Agents));
        f.key(press(KeyCode::Down));
        assert_eq!(f.field(), Field::Driver, "the locked row is stepped over");
        f.key(press(KeyCode::Up));
        assert_eq!(f.field(), Field::Prefix);
        // 잠긴 칸은 키로 못 바꾼다.
        f.cursor = 1;
        f.key(press(KeyCode::Right));
        assert!(!f.plan().agents);
    }

    #[test]
    fn a_prefix_left_as_suggested_stays_unchosen() {
        let mut f = form(Choices::default());
        assert_eq!(f.choices().prefix, None);
        f.key(press(KeyCode::Backspace));
        f.key(press(KeyCode::Char('i')));
        assert_eq!(f.choices().prefix, None, "typed back to the suggestion");
        f.key(press(KeyCode::Char('x')));
        assert_eq!(f.choices().prefix.as_deref(), Some("moaix"));
    }

    #[test]
    fn editing_the_prefix_clears_the_problem() {
        let mut f = form(Choices::default());
        f.problem = Some("bad".into());
        f.key(press(KeyCode::Backspace));
        assert_eq!(f.problem, None);
    }

    #[test]
    fn enter_plants_esc_and_ctrl_c_stop() {
        let mut f = form(Choices::default());
        assert_eq!(f.key(press(KeyCode::Enter)), Act::Plant);
        assert_eq!(f.key(press(KeyCode::Esc)), Act::Stop);
        assert_eq!(f.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), Act::Stop);
    }

    #[test]
    fn only_every_row_chosen_is_complete() {
        assert!(!Choices::from_flags(None, true, true).complete());
        assert!(!Choices::from_flags(Some("a"), true, false).complete());
        assert!(Choices { prefix: Some("a".into()), agents: Some(true), driver: Some(false) }.complete());
    }
}
