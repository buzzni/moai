//! 화면 글자의 말묶음(moai-slfv). **기본은 영어고, 없는 키는 영어로 떨어진다.**
//!
//! 언어마다 JSON 하나(`i18n/<code>.json`)를 두고 **빌드에 싣는다**(사용자 결정). 번역자는 그
//! 파일만 고치면 되고, 배포는 바이너리 하나로 끝나 파일을 못 찾아 글자가 통째로 사라지는
//! 자리가 없다. 다섯 언어를 실어도 수백KB 라 바이너리 상한(15MB)에 멀다.
//!
//! **크레이트를 들이지 않는다**(사용자 결정). 이 도구의 글에는 복수형·지역 서식 규칙이 거의
//! 없어 표 하나와 [`say`] 하나면 되고, 새 축의 의존성은 전이 의존과 빌드 시간을 새로 진다.

// **아직 부르는 자리가 없다.** 뼈대(moai-slfv)와 글자 옮기기(moai-zeyv)를 한 커밋에 섞지
// 않으려고 가른 것이라, 첫 표면이 `t!` 를 부르는 그 커밋에서 이 줄을 지운다.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

/// 화면에 쓸 수 있는 언어. **영어가 기본이다** — 다른 언어는 영어 위에 얹는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Ko,
    Zh,
    Ja,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::En, Lang::Ko, Lang::Zh, Lang::Ja, Lang::Es];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
            Lang::Es => "es",
        }
    }

    /// `ko`·`ko_KR.UTF-8`·`ko-KR`·`KO` 를 모두 받는다 — `LANG` 이 주는 모양이 기계마다 다르다.
    /// 모르는 값은 `None` 이고, 부르는 쪽이 영어로 떨어진다.
    pub fn parse(raw: &str) -> Option<Lang> {
        let head = raw.trim().split(['_', '-', '.']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == head)
    }

    fn bundle(self) -> &'static str {
        match self {
            Lang::En => include_str!("../i18n/en.json"),
            Lang::Ko => include_str!("../i18n/ko.json"),
            Lang::Zh => include_str!("../i18n/zh.json"),
            Lang::Ja => include_str!("../i18n/ja.json"),
            Lang::Es => include_str!("../i18n/es.json"),
        }
    }
}

/// 이 판이 쓸 언어 — `MOAI_LANG` 이 먼저고, 그다음이 설정, 없으면 영어다.
///
/// **환경이 설정을 이긴다**: 설정은 그 사람의 평소 값이고 환경은 이번 한 번이다. 둘 다
/// 모르는 값이면 영어로 떨어진다 — 여기서 멈추면 글자 하나 때문에 도구가 안 도는 셈이다.
pub fn pick(env: Option<&str>, setting: Option<&str>) -> Lang {
    env.filter(|s| !s.trim().is_empty())
        .and_then(Lang::parse)
        .or_else(|| setting.and_then(Lang::parse))
        .unwrap_or_default()
}

/// 이 판이 실제로 쓸 언어 — **한 프로세스에 한 번만 정한다.**
///
/// 매 줄 설정 파일을 다시 읽을 까닭이 없고, 한 번 돌 동안 말이 바뀌면 같은 화면에 두 말이
/// 섞인다. 고르는 자(`MOAI_LANG` → 설정 → 영어)는 [`pick`] 하나고 그쪽이 시험을 받는다 —
/// 여기는 환경과 파일에서 값을 길어 오는 껍데기다.
pub fn current() -> Lang {
    static PICKED: OnceLock<Lang> = OnceLock::new();
    *PICKED.get_or_init(|| {
        let env = std::env::var("MOAI_LANG").ok();
        let setting = crate::user_config::read(crate::user_config::path().as_deref()).lang;
        pick(env.as_deref(), setting.as_deref())
    })
}

/// 지금 언어로 그 키의 글자. 부르는 자리는 이것 하나만 쓴다.
pub fn t(key: &str) -> &'static str {
    say(current(), key)
}

/// 그 언어의 표. 한 번만 읽어 들고 있는다 — 매 줄 JSON 을 다시 푸는 자리가 아니다.
fn table(lang: Lang) -> &'static HashMap<String, String> {
    static TABLES: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    let all = TABLES.get_or_init(|| {
        Lang::ALL
            .into_iter()
            .map(|l| {
                // **깨진 JSON 은 빈 표다.** 여기서 멈추면 번역 파일 하나가 도구를 통째로
                // 세운다 — 영어로 떨어지는 편이 싸다. 영어 표가 온전한지는 시험이 잰다.
                let one: HashMap<String, String> = serde_json::from_str(l.bundle()).unwrap_or_default();
                (l.code(), one)
            })
            .collect()
    });
    &all[lang.code()]
}

/// 그 키의 글자. **없으면 영어로, 영어에도 없으면 키 그대로** 낸다.
///
/// 키를 내는 것이 빈 줄보다 낫다 — 화면에 `status.issues` 가 보이면 무엇이 빠졌는지 그 자리에서
/// 읽히고, 빈 줄은 무엇이 사라졌는지도 안 알려 준다. 영어 표가 모든 키를 갖는 것은 시험이
/// 잰다(moai-f2a6) — 다른 언어는 비어 있어도 통과한다. 다섯을 함께 채우게 하면 글 한 줄 고칠
/// 때마다 다섯을 고쳐야 하고, 모르는 언어에 기계번역이 들어온다.
pub fn say(lang: Lang, key: &str) -> &'static str {
    table(lang)
        .get(key)
        .or_else(|| table(Lang::En).get(key))
        .map(String::as_str)
        .unwrap_or_else(|| leak(key))
}

/// 못 찾은 키를 화면에 그대로 낼 때 쓴다. 키는 소스에 박힌 몇 개뿐이라 새는 양이 유한하다.
fn leak(key: &str) -> &'static str {
    static SEEN: OnceLock<std::sync::Mutex<Vec<&'static str>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    let mut seen = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match seen.iter().find(|s| **s == key) {
        Some(s) => s,
        None => {
            let s: &'static str = Box::leak(key.to_string().into_boxed_str());
            seen.push(s);
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **기계가 주는 모양은 제각각이다** — `ko`·`ko_KR.UTF-8`·`ko-KR`·대문자를 다 받는다.
    #[test]
    fn a_language_tag_is_read_by_its_head() {
        for raw in ["ko", "ko_KR.UTF-8", "ko-KR", "KO", " ko "] {
            assert_eq!(Lang::parse(raw), Some(Lang::Ko), "{raw:?}");
        }
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::parse("C"), None, "로캘 없음은 언어가 아니다");
        assert_eq!(Lang::parse("kr"), None, "없는 코드를 받아 주면 오타가 조용히 산다");
    }

    /// **환경이 먼저, 그다음이 설정, 없으면 영어다.** 모르는 값에서 멈추지 않는다 —
    /// 글자 하나 때문에 도구가 안 도는 셈이 된다.
    #[test]
    fn the_environment_wins_then_the_setting_then_english() {
        assert_eq!(pick(Some("ko"), Some("ja")), Lang::Ko, "환경이 설정에 졌다");
        assert_eq!(pick(None, Some("ja")), Lang::Ja);
        assert_eq!(pick(Some(""), Some("ja")), Lang::Ja, "빈 환경변수는 안 준 것이다");
        assert_eq!(pick(None, None), Lang::En);
        assert_eq!(pick(Some("kr"), None), Lang::En, "모르는 값에서 멈췄다");
        assert_eq!(pick(Some("kr"), Some("ko")), Lang::Ko, "모르는 환경값이 설정까지 버렸다");
    }

    /// **없는 키는 영어로 떨어지고, 영어에도 없으면 키 그대로 난다.** 빈 줄은 무엇이
    /// 사라졌는지도 안 알려 준다.
    #[test]
    fn a_missing_key_falls_back_to_english_then_to_the_key_itself() {
        assert_eq!(say(Lang::Ko, "status.issues"), "이슈 {n}");
        // ja 에는 아직 이 키가 없다 — 영어가 받는다.
        assert_eq!(say(Lang::Ja, "status.epics"), say(Lang::En, "status.epics"));
        assert_eq!(say(Lang::En, "nothing.here"), "nothing.here");
        assert_eq!(say(Lang::Ko, "nothing.here"), "nothing.here");
    }

    /// 실린 표가 다섯 다 읽힌다 — JSON 하나가 깨져 조용히 빈 표가 되는 것을 여기서 잡는다.
    #[test]
    fn every_bundle_parses() {
        for lang in Lang::ALL {
            let one: HashMap<String, String> =
                serde_json::from_str(lang.bundle()).unwrap_or_else(|e| panic!("{}: {e}", lang.code()));
            assert!(!one.is_empty(), "{} 표가 비었다", lang.code());
        }
    }
}
