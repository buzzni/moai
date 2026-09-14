//! 목록에 **무엇을 보일까**(moai-fmv5). 거름망이 아니라 보기다 — 사람이 적은 물음(`SPC f`·`/`)과
//! 따로 들고, 목록은 둘을 함께 통과한 줄만 세운다. Esc 는 거름망만 푼다: 늘 켜 두는 보기를 실수
//! 한 번에 잃으면 done 이 도로 쏟아진다.
//!
//! **조각이다.** `App` 도 설정도 모른다. 처음 무엇을 숨길지(done)는 든 쪽이 정하고, 줄마다의
//! 사실(묶음이면 멤버에서 읽은 칸, 물려받았든 미뤘는가)도 든 쪽이 재서 넘긴다 — 여기서 이슈를
//! 풀어 칸을 다시 읽으면 목록의 글리프와 숨김이 다른 칸을 본다.

/// 칸마다 보이는가, 미룬 것을 보이는가.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    /// 숨긴 칸의 이름. **보인 쪽이 아니라 숨긴 쪽을 든다** — 설정에 칸이 새로 생기면 저절로
    /// 보인다. 보인 쪽을 들면 새 칸의 줄이 이유 없이 사라진다.
    pub hidden: Vec<String>,
    pub hide_deferred: bool,
}

impl View {
    /// 칸 하나를 숨긴 채로.
    pub fn hiding(column: &str) -> View {
        View { hidden: vec![column.to_string()], hide_deferred: false }
    }

    pub fn hides(&self, column: &str) -> bool {
        self.hidden.iter().any(|h| h == column)
    }

    pub fn toggle(&mut self, column: &str) {
        match self.hidden.iter().position(|h| h == column) {
            Some(at) => {
                self.hidden.remove(at);
            }
            None => self.hidden.push(column.to_string()),
        }
    }

    /// 그 줄이 보이는가. `column` 은 묶음이면 멤버에서 읽은 칸이다.
    pub fn shows(&self, column: &str, deferred: bool) -> bool {
        !self.hides(column) && !(deferred && self.hide_deferred)
    }

    /// 경로 줄에 댈 한 마디 — `done·미룸 숨김`. 숨긴 것이 없으면 없다.
    pub fn badge(&self) -> Option<String> {
        let mut names: Vec<&str> = self.hidden.iter().map(String::as_str).collect();
        if self.hide_deferred {
            names.push("미룸");
        }
        (!names.is_empty()).then(|| format!("{} 숨김", names.join("·")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_column_comes_back_when_toggled_again() {
        let mut v = View::hiding("done");
        assert!(!v.shows("done", false));
        assert!(v.shows("todo", true), "미룬 것은 처음에 보인다");
        v.toggle("done");
        assert_eq!(v, View::default());
        v.toggle("done");
        assert_eq!(v, View::hiding("done"), "두 번 누르면 제자리다");
    }

    #[test]
    fn deferred_hides_on_its_own_axis() {
        let v = View { hide_deferred: true, ..View::default() };
        assert!(!v.shows("todo", true));
        assert!(v.shows("todo", false), "미룸을 숨겨도 안 미룬 칸은 그대로다");
    }

    #[test]
    fn a_column_the_config_adds_later_stays_visible() {
        assert!(View::hiding("done").shows("blocked", false));
    }

    #[test]
    fn the_badge_names_what_is_hidden() {
        assert_eq!(View::default().badge(), None);
        assert_eq!(View::hiding("done").badge().as_deref(), Some("done 숨김"));
        let v = View { hidden: vec!["review".into(), "done".into()], hide_deferred: true };
        assert_eq!(v.badge().as_deref(), Some("review·done·미룸 숨김"));
    }
}
