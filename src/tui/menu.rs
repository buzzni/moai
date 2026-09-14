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
//! **조각이다.** `App`·터미널을 모른다. 켜짐([`Ctx`])은 든 쪽이 재서 넘긴다.

use super::keys::{BROWSE, Bind, Browse, Chord, Ctx, LEADER, Lookup, MENU, Menu, lookup, name_of};
use ratatui::crossterm::event::KeyEvent;

/// 하위 접두어의 이름. 표에는 동작만 있고 묶음의 이름은 없어 여기 둔다 — 이름 없는 접두어는
/// 시험(`every_prefix_in_the_menu_has_a_name`)이 막는다.
const GROUPS: &[(&str, &str)] = &[("SPC p", "프로젝트"), ("SPC t", "토글")];

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

/// 메뉴 한 줄 — `키  낱말 [상태]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// 동작의 낱말, 하위 접두어면 `이름 …`.
    pub what: String,
    /// 켜고 끄는 것의 지금 상태(`[켜짐]`·`[원문]`).
    pub state: Option<&'static str>,
}

/// 지금 층(`held`)에 선 항목. 차례는 표의 차례다.
pub fn entries(held: &[KeyEvent], c: &Ctx) -> Vec<Entry> {
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
                out.push(Entry { key: next.name(), what: act.menu_word(c).into(), state: act.state(c) });
            }
            Lookup::Pending if live(&seq, c) => {
                out.push(Entry { key: next.name(), what: format!("{} …", group(&seq)), state: None });
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

    fn inside() -> Ctx {
        Ctx { list_focus: true, ..Ctx::default() }
    }

    fn layer() -> Ctx {
        Ctx { layer: true, list_focus: true, ..Ctx::default() }
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
        let root = entries(ch.held(), &inside());
        assert_eq!(keys_of(&root), ["/", "f", "n", "r", "q", "p", "t"]);
        let what: Vec<&str> = root.iter().map(|e| e.what.as_str()).collect();
        assert_eq!(what, ["검색", "거름망", "생각 담기", "다시 읽기", "끝내기", "프로젝트 …", "토글 …"]);
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
        assert_eq!(keys_of(&entries(ch.held(), &c)), ["w", "r"]);
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
        let root_in = entries(&[k(' ')], &inside());
        let root_layer = entries(&[k(' ')], &layer());
        assert_eq!(keys_of(&root_layer), ["n", "r", "q", "p", "t"], "층에서 검색·거름망이 섰다");
        assert!(keys_of(&root_in).contains(&"f"));
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &inside())), ["a"], "프로젝트 안에서 해제가 섰다");
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &layer())), ["a", "d"]);
        let detail = Ctx { list_focus: false, ..layer() };
        assert_eq!(keys_of(&entries(&[k(' '), k('p')], &detail)), ["a"], "상세 포커스에서 해제가 섰다");
        assert_eq!(keys_of(&entries(&[k(' '), k('t')], &layer())), ["r"], "층에서 워크트리가 섰다");

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
        let states = |c: Ctx| -> Vec<Option<&'static str>> { entries(&[k(' '), k('t')], &c).iter().map(|e| e.state).collect() };
        assert_eq!(states(inside()), [Some("[꺼짐]"), Some("[그리기]")]);
        assert_eq!(states(Ctx { worktree: true, raw: true, ..inside() }), [Some("[켜짐]"), Some("[원문]")]);
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
