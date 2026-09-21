//! 실패와 그 **코드**.
//!
//! 코드가 어느 한 층의 것이 아니라 따로 사는 이유가 있다 — `store` 가
//! "못 찾았다" 를 알고 `cmd` 가 그것을 `not_found` 로 옮겨 적으면, 옮겨 적는
//! 곳마다 달라진다. 실제로 `show` 는 `not_found` 를 내는데 `edit`·`note` 는
//! 뭉뚱그린 `error` 를 냈다. `--json` 의 `code` 는 받는 쪽이 분기하는 값이라
//! 명령마다 다르면 계약이 아니다.

#[derive(Debug)]
pub struct Fail {
    pub message: String,
    pub code: &'static str,
}

/// 있는 코드 전부. 새로 만들 때는 여기 적는다.
pub mod code {
    pub const ERROR: &str = "error";
    pub const NOT_FOUND: &str = "not_found";
    pub const BAD_STATUS: &str = "bad_status";
    pub const BAD_FILTER: &str = "bad_filter";
    pub const BAD_TARGET: &str = "bad_target";
    pub const BAD_INPUT: &str = "bad_input";
    /// 누가 하는지 모른다. 받는 쪽이 이것만 보고 사람에게 물을 수 있어야 해
    /// `bad_input` 과 갈라 둔다 — 고칠 곳이 argv 가 아니라 설정이다.
    pub const NO_ACTOR: &str = "no_actor";
    pub const ALREADY_EXISTS: &str = "already_exists";
    pub const LOCKED: &str = "locked";
    pub const BROKEN: &str = "broken";
}

impl Fail {
    pub fn new(message: impl Into<String>) -> Fail {
        Fail { message: message.into(), code: code::ERROR }
    }
    pub fn coded(message: impl Into<String>, code: &'static str) -> Fail {
        Fail { message: message.into(), code }
    }
    /// 없는 id — **`mv`·`defer` 가 여럿을 훑다 대는 줄과 같은 키를 쓴다**(moai-95g1).
    /// 한 도구가 같은 처지를 두 낱말로 말하면 읽는 쪽이 둘을 다른 일로 읽는다.
    pub fn not_found(id: &str, lang: crate::i18n::Lang) -> Fail {
        let said = crate::i18n::fill(crate::i18n::say(lang, "refuse.not_found"), &[("id", id)]);
        Fail::coded(said, code::NOT_FOUND)
    }
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::new(m)
    }
}

impl From<&str> for Fail {
    fn from(m: &str) -> Fail {
        Fail::new(m)
    }
}

pub type R<T> = Result<T, Fail>;
