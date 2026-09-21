//! 화면 글자의 말묶음(moai-slfv). **기본은 영어고, 없는 키는 영어로 떨어진다.**
//!
//! 언어마다 JSON 하나(`i18n/<code>.json`)를 두고 **빌드에 싣는다**(사용자 결정). 번역자는 그
//! 파일만 고치면 되고, 배포는 바이너리 하나로 끝나 파일을 못 찾아 글자가 통째로 사라지는
//! 자리가 없다. 다섯 언어를 실어도 수백KB 라 바이너리 상한(15MB)에 멀다.
//!
//! **크레이트를 들이지 않는다**(사용자 결정). 이 도구의 글에는 복수형·지역 서식 규칙이 거의
//! 없어 표 하나와 [`say`] 하나면 되고, 새 축의 의존성은 전이 의존과 빌드 시간을 새로 진다.

use std::collections::HashMap;
use std::sync::OnceLock;

/// 화면에 쓸 수 있는 언어. **영어가 기준이다** — 없는 키는 영어로 떨어지고, 새 키는 영어부터다.
/// **기본값도 영어다**(moai-bn1j, 2026-09-20 사용자 결정 — v0.1.0 은 영어로 나간다).
///
/// 한때 기본이 한국어였다(리뷰 moai-80qw.cb8). 말묶음에 든 글이 열 줄뿐이던 때라, 영어로 두면
/// 아무것도 안 고른 사람의 화면이 영어 두어 줄과 한국어 열 몇 줄로 **섞였기** 때문이다 — 그
/// 조건이 "옮김이 화면을 덮을 때 `#[default]` 를 `En` 으로 옮긴다" 였고, 여기가 그 자리다.
/// **되돌리지 않는다**: 받는 사람이 읽을 수 있는 말이 무엇인지는 옮김의 양이 아니라 배포가
/// 정하고, 덜 옮긴 화면은 영어로 떨어져 읽히지만 안 배운 말로 선 화면은 안 읽힌다.
/// 한국어는 `MOAI_LANG=ko`·설정으로 본다 — 두 길 다 [`pick`] 이 받는다.
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

/// 이 판이 쓸 언어 — `MOAI_LANG` 이 먼저고, 그다음이 설정, 없으면 기본값([`Lang::default`])이다.
///
/// **환경이 설정을 이긴다**: 설정은 그 사람의 평소 값이고 환경은 이번 한 번이다. 둘 다
/// 모르는 값이면 기본값으로 떨어진다 — 여기서 멈추면 글자 하나 때문에 도구가 안 도는 셈이다.
///
/// **시스템 로캘(`LANG`·`LC_ALL`·`LC_MESSAGES`)은 안 읽는다**(moai-gv9n, 2026-09-20 사용자
/// 결정). [`Lang::parse`] 가 `ko_KR.UTF-8` 모양을 받는 것은 `MOAI_LANG` 에 로캘을 그대로 붙여
/// 넣는 사람을 받자는 것이지 로캘을 읽는다는 뜻이 아니다.
///
/// **그 결정의 여는 조건이 왔고, 사용자가 다시 안 읽기로 정했다**(moai-bn1j, 2026-09-20).
/// 조건은 "`#[default]` 를 `En` 으로 옮기는 날" 이었는데, 옮기고 보니 까닭이 그대로 남았다 —
/// 영어 표가 백 키를 넘긴 지금 ja·zh·es 는 아홉·열 키뿐이라, 로캘을 읽으면 `ja_JP` 기계가 일본어
/// 열 줄과 영어 아흔 줄로 **섞인 화면**을 본다. 그것이 gv9n 이 이 층을 안 연 바로 그 까닭이다.
/// 다 찬 말은 ko 하나고, 그 하나를 위해 로캘을 읽으면 "다 찬 말" 이라는 둘째 어휘가 코드에
/// 생긴다. **다시 여는 조건**: ja·zh·es 가 영어 표를 덮는 날 셋을 함께 잰다.
pub fn pick(env: Option<&str>, setting: Option<&str>) -> Lang {
    env.filter(|s| !s.trim().is_empty())
        .and_then(Lang::parse)
        .or_else(|| setting.and_then(Lang::parse))
        .unwrap_or_default()
}

/// 글 안의 `{이름}` 자리를 채운다 — `fill(say(lang, "status.issues"), &[("n", "12")])`.
///
/// **자리는 이름으로 둔다**(`{n}`), 차례가 아니다. 번역은 말차례가 달라지는 일이라, 자리를
/// 차례로 두면 번역자가 순서를 바꾸는 순간 값이 엉뚱한 자리에 든다. 모르는 이름은 그대로
/// 남는다 — 화면에 `{n}` 이 보이면 무엇이 안 채워졌는지 그 자리에서 읽힌다.
///
/// **한 번만 훑는다**(리뷰 moai-80qw) — 채운 값은 다시 안 본다. 이름마다 `replace` 를 걸면
/// 앞서 채운 값 **안의** `{이름}` 을 뒤의 짝이 또 채운다: 제목이 `{n}` 인 이슈 하나가 제
/// 자리에 셈을 받아 들이고 진짜 셈 자리는 사라진다. 그러면 `vars` 의 차례가 결과를 바꾸는
/// 셈이라, 이름으로 둔 뜻이 거기서 도로 무너진다.
pub fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // 닫히지 않은 `{` 는 글자다 — 남은 것을 그대로 넘긴다.
        let Some(close) = rest.find('}') else { break };
        match vars.iter().find(|(name, _)| *name == &rest[1..close]) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(&rest[..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// 그 언어의 표. 한 번만 읽어 들고 있는다 — 매 줄 JSON 을 다시 푸는 자리가 아니다.
fn table(lang: Lang) -> &'static HashMap<String, String> {
    static TABLES: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    static NONE: OnceLock<HashMap<String, String>> = OnceLock::new();
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
    // **`ALL` 에서 빠진 갈래도 빈 표다.** `code()`·`bundle()` 은 새 갈래를 빠뜨리면 컴파일이
    // 잡지만 `ALL: [Lang; 5]` 은 그대로 서고, 색인(`all[…]`)으로 들면 그 갈래가 화면에 닿는
    // 순간 패닉이다 — 깨진 JSON 을 빈 표로 받는 것과 같은 까닭으로 여기서도 멈추지 않는다.
    all.get(lang.code()).unwrap_or_else(|| NONE.get_or_init(Default::default))
}

/// 그 키의 글자. **없으면 영어로, 영어에도 없으면 키 그대로** 낸다.
///
/// 키를 내는 것이 빈 줄보다 낫다 — 화면에 `status.issues` 가 보이면 무엇이 빠졌는지 그 자리에서
/// 읽히고, 빈 줄은 무엇이 사라졌는지도 안 알려 준다. 영어 표가 모든 키를 갖는 것은 시험이
/// 잰다(moai-f2a6) — 다른 언어는 비어 있어도 통과한다. 다섯을 함께 채우게 하면 글 한 줄 고칠
/// 때마다 다섯을 고쳐야 하고, 모르는 언어에 기계번역이 들어온다.
/// **키는 소스에 박힌 글자다**(`&'static str`, 리뷰 moai-slfv.vrw). 자료로 지은 키
/// (`format!("kind.{k}")`)를 받으면 못 찾은 키를 화면에 내려고 그 글자를 영영 들고 있어야
/// 하고, 그러면 새는 양이 자료 수만큼 는다. 타입이 그것을 막으면 그럴 자리가 없다 — 칸 이름
/// 처럼 설정에서 오는 낱말은 애초에 번역할 것이 아니라 그대로 내는 값이다.
pub fn say(lang: Lang, key: &'static str) -> &'static str {
    table(lang).get(key).or_else(|| table(Lang::En).get(key)).map_or(key, String::as_str)
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

    /// **환경이 먼저, 그다음이 설정, 없으면 기본값이다.** 모르는 값에서 멈추지 않는다 —
    /// 글자 하나 때문에 도구가 안 도는 셈이 된다. 기본값이 무엇인지는 여기서 안 박는다 —
    /// 이 시험이 재는 것은 **고르는 차례**뿐이다. 그 값은 [`nothing_picked_means_english`] 가
    /// 이름을 대고 못 박는다.
    #[test]
    fn the_environment_wins_then_the_setting_then_the_default() {
        assert_eq!(pick(Some("ko"), Some("ja")), Lang::Ko, "환경이 설정에 졌다");
        assert_eq!(pick(None, Some("ja")), Lang::Ja);
        assert_eq!(pick(Some(""), Some("ja")), Lang::Ja, "빈 환경변수는 안 준 것이다");
        assert_eq!(pick(None, None), Lang::default());
        assert_eq!(pick(Some("kr"), None), Lang::default(), "모르는 값에서 멈췄다");
        // 영어는 **기준**이라 고르면 늘 잡힌다 — 기본값이 무엇이든.
        assert_eq!(pick(Some("en"), Some("ko")), Lang::En);
        assert_eq!(pick(Some("kr"), Some("ko")), Lang::Ko, "모르는 환경값이 설정까지 버렸다");
    }

    /// **아무것도 안 고르면 영어다**(moai-bn1j, 2026-09-20 사용자 결정 — v0.1.0 은 영어로 나간다).
    ///
    /// 위 시험은 **고르는 차례**만 재고 기본값이 무엇인지는 일부러 안 박는다. 그 값은 한때
    /// 옮김이 얼마나 찼는지에 따라 바뀌는 값이었지만, 이제는 배포가 정한 값이다 — 한 줄
    /// (`#[default]`)이라 리팩터 중에 말없이 미끄러지기 쉬워 여기서 이름을 대고 못 박는다.
    /// **로캘도 여기서 잰다**: 로캘 모양의 값을 환경에 둬도 `MOAI_LANG` 이 아니면 안 읽는다
    /// ([`pick`]). 이 자를 시험 밖에서 재는 것은 `tests/cli.rs` 가 화면으로 한다.
    #[test]
    fn nothing_picked_means_english() {
        assert_eq!(Lang::default(), Lang::En);
        assert_eq!(pick(None, None), Lang::En);
        assert_eq!(pick(Some(""), None), Lang::En, "빈 환경변수는 안 준 것이다");
        assert_eq!(pick(Some("kr"), None), Lang::En, "모르는 값에서 멈췄다");
    }

    /// **채운 값은 다시 안 본다**(리뷰 moai-80qw) — 그리고 `vars` 의 차례가 결과를 안 바꾼다.
    ///
    /// 이름마다 `replace` 를 걸던 때는 앞서 넣은 값 **안의** `{이름}` 을 뒤의 짝이 또 채웠다.
    /// 이 트래커에서 `{n}` 은 얼마든지 있을 수 있는 제목이라, 그런 이슈 하나가 제 자리에 셈을
    /// 받아 들이고 진짜 셈 자리는 화면에서 사라진다.
    #[test]
    fn a_filled_value_is_not_filled_again_and_the_order_does_not_matter() {
        assert_eq!(fill("{title} — {n}건", &[("title", "{n} 를 고친다"), ("n", "3")]), "{n} 를 고친다 — 3건");
        assert_eq!(fill("{title} — {n}건", &[("n", "3"), ("title", "{n} 를 고친다")]), "{n} 를 고친다 — 3건");
        // 모르는 이름은 그대로 남는다 — 무엇이 안 채워졌는지 그 자리에서 읽힌다.
        assert_eq!(fill("{a}{b}", &[("a", "1")]), "1{b}");
        // 닫히지 않은 `{` 도 글자다. 자리가 없는 글은 그대로 지난다.
        assert_eq!(fill("{ 열린 채", &[("n", "1")]), "{ 열린 채");
        assert_eq!(fill("자리 없음", &[("n", "1")]), "자리 없음");
    }

    /// **없는 키는 영어로 떨어지고, 영어에도 없으면 키 그대로 난다.** 빈 줄은 무엇이
    /// 사라졌는지도 안 알려 준다.
    #[test]
    fn a_missing_key_falls_back_to_english_then_to_the_key_itself() {
        assert_eq!(say(Lang::Ko, "status.issues"), "이슈 {n}");
        // **번역은 원래 덜 된 채로 산다** — 다섯을 함께 채우게 하지 않는 것이 결정이고, 빠진
        // 키는 영어가 받는다. **어느 키가 비었는지는 여기서 고른다**(리뷰 moai-80qw): 특정
        // 말의 특정 구멍을 글자로 박으면, 안내가 시킨 대로 그 자리를 채운 번역자가 제 번역과
        // 아무 상관없는 이 시험을 깬다. 시험 때문에 실려 나가는 번역 파일은 시험 자료가 아니다.
        // 다 채워진 날에는 잴 구멍이 없고, 그것도 정상이다.
        let en = table(Lang::En);
        let hole = Lang::ALL.into_iter().filter(|l| *l != Lang::En).find_map(|l| {
            let theirs = table(l);
            en.keys().find(|k| !theirs.contains_key(*k)).map(|k| (l, k.as_str()))
        });
        if let Some((lang, key)) = hole {
            assert_eq!(say(lang, key), say(Lang::En, key), "{}: `{key}` 가 영어로 안 떨어졌다", lang.code());
        }
        assert_eq!(say(Lang::En, "nothing.here"), "nothing.here"); // i18n:없는-키
        assert_eq!(say(Lang::Ko, "nothing.here"), "nothing.here"); // i18n:없는-키
    }

    /// **영어 표가 소스가 쓰는 키를 다 갖는다**(moai-f2a6) — 새 키는 영어부터다.
    ///
    /// 소스에서 `say(…, "…")` 가 든 키를 읽어 영어 표와 견준다. 없는 키는 화면에
    /// `status.issues` 같은 글자로 그대로 나므로, 이 시험이 그 자리를 **붙이기 전에** 잡는다.
    ///
    /// **그러니 키는 `say` 부름에 그대로 적는다.** 도우미에 키만 넘기면(`one("warn.…")`)
    /// 읽는 자가 그 줄을 못 보고, 이 시험은 파란 채로 구멍이 뚫린다 — `view::says` 의
    /// 도우미가 **찾아 온 글**을 받는 까닭이 이것이다.
    /// 다른 언어는 **안 잰다**(사용자 결정) — 다섯을 함께 채우게 하면 글 한 줄 고칠 때마다
    /// 다섯을 고쳐야 하고, 모르는 언어에 기계번역이 들어온다. 그쪽은 영어로 떨어진다.
    #[test]
    fn english_has_every_key_the_source_asks_for() {
        let en = table(Lang::En);
        let mut asked: Vec<(String, String)> = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for (n, line) in text.lines().enumerate() {
                    // 일부러 없는 키를 부르는 줄(떨어짐 시험)은 뺀다 — 표에 있으면 그 시험이 죽는다.
                    if line.contains("i18n:없는-키") {
                        continue;
                    }
                    for key in keys_in(line) {
                        asked.push((key, format!("{}:{}", path.display(), n + 1)));
                    }
                }
            }
        }
        assert!(asked.len() >= 2, "소스에서 키를 하나도 못 읽었다 — 읽는 자가 헛돈다");
        let missing: Vec<&(String, String)> = asked.iter().filter(|(k, _)| !en.contains_key(k)).collect();
        assert!(missing.is_empty(), "영어 표에 없는 키를 소스가 부른다 — {missing:#?}");
        // **반대쪽도 센다**(리뷰) — 위의 비교는 읽는 자가 **본** 키만 재므로, 키를 도우미에
        // 숨기면(`one("warn.…")`) 그 키는 양쪽에서 함께 사라져 이 시험이 파란 채로 구멍이
        // 뚫린다. 실제로 `view::says` 의 스물한 줄이 그렇게 숨었고, 바로 위 주석이 "시험이
        // 잡는다" 고 말하는 동안 틀린 키를 적어도 아무 데서도 안 붉어졌다. 영어 표에 있는데
        // 아무도 안 부르는 키는 **지운 자리의 찌꺼기이거나 숨은 부름**이고, 둘 다 여기서
        // 이름을 댄다 — 아직 안 이은 키를 미리 적어 두지 않는다.
        let unasked: Vec<&String> = en.keys().filter(|k| !asked.iter().any(|(a, _)| a == *k)).collect();
        assert!(
            unasked.is_empty(),
            "영어 표에 있는데 소스가 안 부르는 키 — 지웠으면 표에서도 지우고, \
             부르고 있다면 키를 `say(lang, \"…\")` 에 그대로 적는다: {unasked:#?}"
        );
    }

    /// `say(…, "키")` 의 키만 뽑는다. 이 시험 자신이 쓰는 글(`"nothing.here"` 같은)은
    /// 부르는 모양이 아니라 안 걸린다.
    ///
    /// **부르는 모양은 하나다.** 한때 `t("키")` 도 읽었는데 그 함수는 걷혔다(moai-cigu) —
    /// 없는 모양을 계속 읽으면 다음 사람이 두 입구가 다 산 줄로 읽는다.
    fn keys_in(line: &str) -> Vec<String> {
        const HEAD: &str = "say(";
        let mut out = Vec::new();
        let mut rest = line;
        let mut eaten = 0usize;
        while let Some(at) = rest.find(HEAD) {
            // **이름 끝의 `say(` 는 이 부름이 아니다** — `essay("…")` 같은 이름이 그렇게 걸린다.
            // 앞 글자가 이름의 일부면 건너뛴다. `i18n::say(` 의 `:` 는 이름 글자가 아니라 지나간다.
            let before = line[..eaten + at].chars().next_back();
            let joined = before.is_some_and(|c| c.is_alphanumeric() || c == '_');
            eaten += at + HEAD.len();
            rest = &rest[at + HEAD.len()..];
            if joined {
                continue;
            }
            // `say(lang, "키")` 는 따옴표가 둘째 인자다 — 여는 따옴표까지 건너뛴다.
            let Some(quoted) = rest.split_once('"').map(|(_, r)| r) else { continue };
            if let Some((key, _)) = quoted.split_once('"')
                && key.contains('.')
                && !key.contains(' ')
            {
                out.push(key.to_string());
            }
        }
        out
    }

    /// 실린 표가 다섯 다 읽힌다 — JSON 하나가 깨져 조용히 빈 표가 되는 것을 여기서 잡는다.
    ///
    /// **읽히는 길로도 잰다**([`table`]). 여기서만 따로 파싱하면 `table` 의
    /// `unwrap_or_default` 가 삼킨 것이 안 보이고, 시험은 통과하는데 화면은 영어로 떨어진다.
    /// **어느 갈래가 남의 파일을 가리키는지도 잰다** — `bundle()` 의 `include_str!` 은 다섯
    /// 줄이 나란해 한 줄을 잘못 이어도 컴파일이 통과하고, ja·zh·es 는 담긴 키가 같아
    /// 나머지 시험이 그것을 못 본다.
    ///
    /// **제 이름의 파일과 글자로 견준다**(리뷰 moai-80qw). 겹치는지만 보면 두 갈래가 서로의
    /// 파일을 **맞바꾼** 것을 못 본다 — 둘 다 남아 있으니 겹치지 않고, `table` 과의 비교는
    /// 같은 `bundle()` 을 양쪽에 놓아 제 자신과 견주는 셈이다. 실제로 ja·zh 를 맞바꿔 재 보니
    /// 일곱 시험이 다 통과했고, 일본어를 고른 사람에게 중국어가 나갔다.
    #[test]
    fn every_bundle_parses_and_is_wired_to_its_own_file() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n"));
        for lang in Lang::ALL {
            let one: HashMap<String, String> =
                serde_json::from_str(lang.bundle()).unwrap_or_else(|e| panic!("{}: {e}", lang.code()));
            assert!(!one.is_empty(), "{} 표가 비었다", lang.code());
            assert_eq!(table(lang), &one, "{} 는 실릴 때와 읽힐 때가 다르다", lang.code());
            let file = dir.join(format!("{}.json", lang.code()));
            let said = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            assert_eq!(lang.bundle(), said, "{} 가 제 말묶음 파일을 안 가리킨다", lang.code());
        }
        // **디렉터리에 있는 말묶음은 다 실린다.** `ALL` 은 손으로 적는 목록이라 새 갈래를
        // `code()`·`bundle()` 에만 더하면 컴파일이 통과하고(둘은 빠짐없는 `match` 지만
        // `[Lang; 5]` 는 그대로 선다), 그 말은 `parse` 에도 안 걸리고 시험도 안 훑어 영영
        // 안 선다 — 안내가 대는 "다섯 자리" 중 이 자리만 컴파일러가 안 잡는다.
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        let mut listed: Vec<String> = Lang::ALL.into_iter().map(|l| l.code().to_string()).collect();
        on_disk.sort();
        listed.sort();
        assert_eq!(on_disk, listed, "i18n/ 의 말묶음과 `Lang::ALL` 이 다르다 — 갈래를 더하며 `ALL` 을 빠뜨렸다");
    }

    /// **번역이 자리를 잃으면 수가 화면에서 사라진다.** `"ready.count": "着手できる作業"` 처럼
    /// `{n}` 을 떨어뜨린 줄은 컴파일도 파싱도 통과하고, 그 언어로 도는 사람만 셈이 없는 화면을
    /// 본다 — 빠진 키와 달리 영어로 떨어지지도 않는다(값이 **있으니** 그것이 답이다). 빈 값도
    /// 같다: 머리 줄이 통째로 사라진다.
    ///
    /// 그래서 **번역이 든 키만** 영어와 견준다 — 자리 이름이 같은지, 값이 비지 않았는지.
    /// 키가 아예 없는 것은 안 잰다(es 의 `status.milestone_label`): 그쪽은 영어가 받고,
    /// 다섯을 함께 채우게 하지 않는 것이 결정이다.
    /// **한국어 표는 영어 표의 키를 다 든다**(리뷰 moai-hom6.qd9 8번). ko 는 en 과 함께 **다 찬**
    /// 두 말이라(AGENTS.md "The language on screen"), 한 키가 빠지면 한국어를 고른 사람이 그 줄만
    /// 영어로 받는다 — 영어로 떨어지니 화면이 비지 않아 어느 시험도 안 붉어진다. 화면마다 한글이
    /// 한 자라도 섰는가만 재던 판(`the_init_screen_stands_in_one_language`)은 새 키 다섯을 지워도
    /// 푸르렀다: 한 화면에 한 줄만 한국어로 남아도 지나간다.
    ///
    /// **다른 셋(zh·ja·es)은 안 잰다** — 빈 키를 영어가 받게 두는 것이 결정이다(위 시험의 글).
    #[test]
    fn korean_carries_every_key_english_does() {
        let (en, ko) = (table(Lang::En), table(Lang::Ko));
        let missing: Vec<&str> = en.keys().filter(|k| !ko.contains_key(*k)).map(String::as_str).collect();
        assert!(missing.is_empty(), "한국어 표에 영어 키가 빠졌다 — {missing:?}");
    }

    #[test]
    fn every_translation_keeps_the_places_english_marks() {
        let en = table(Lang::En);
        for (key, text) in en {
            // **키에는 점이 있다.** `english_has_every_key_the_source_asks_for` 의 읽는 자가
            // 점으로 키를 가려내므로, 점 없는 키는 그 시험을 그냥 지나간다.
            assert!(key.contains('.'), "영어 표의 `{key}` 에 점이 없다 — 소스를 훑는 시험이 이 키를 못 본다");
            assert!(!text.trim().is_empty(), "영어 표의 `{key}` 가 비었다");
        }
        // **셈이 드는 자리는 영어에 있어야 한다**(리뷰 moai-80qw). 아래 비교는 번역을 영어와
        // 견주므로, 영어가 `{n}` 을 잃으면 번역도 나란히 잃은 채로 통과한다 — 그러면 모든
        // 말에서 수가 사라지고, 화면을 견주는 시험도 제 기댓값을 같은 표에서 길어 못 본다.
        for key in ["status.issues", "status.epics", "ready.count"] {
            let text = en.get(key).unwrap_or_else(|| panic!("영어 표에 `{key}` 가 없다"));
            assert!(places(text).contains("n"), "영어 표의 `{key}` 에 `{{n}}` 이 없다 — 그 줄에서 수가 사라진다");
        }
        for lang in Lang::ALL {
            for (key, text) in table(lang) {
                assert!(!text.trim().is_empty(), "{}: `{key}` 가 비었다 — 그 줄이 화면에서 사라진다", lang.code());
                // **한 줄이어야 한다**(리뷰 moai-80qw). JSON 값에는 `\n` 도 ESC 도 담기고,
                // 안내는 번역자에게 러스트를 몰라도 된다고 말한다 — 머리 글에 개행 하나가 들면
                // 그 아래 표가 통째로 어긋나고 ESC 는 화면을 다시 칠한다. `view` 가 파일 밖에서
                // 온 글을 `text::one_line` 으로 거르는 것과 같은 자리고, 여기는 실릴 때 한 번만
                // 재면 되므로 매 줄 거르는 값을 안 치른다.
                assert!(
                    !text.chars().any(char::is_control),
                    "{}: `{key}` 에 제어 글자가 들었다 — 화면 글은 한 줄이다",
                    lang.code()
                );
                let Some(source) = en.get(key) else {
                    panic!("{}: `{key}` 를 영어 표가 모른다 — 새 키는 영어부터고, 지운 키는 함께 지운다", lang.code())
                };
                assert_eq!(places(source), places(text), "{}: `{key}` 의 자리가 영어와 다르다", lang.code());
            }
        }
    }

    /// 글 안의 `{이름}` 들. [`fill`] 이 채우는 자리와 같은 것을 읽는다.
    fn places(text: &str) -> std::collections::BTreeSet<&str> {
        let mut out = std::collections::BTreeSet::new();
        let mut rest = text;
        while let Some(at) = rest.find('{') {
            rest = &rest[at + 1..];
            if let Some((name, tail)) = rest.split_once('}') {
                out.insert(name);
                rest = tail;
            }
        }
        out
    }
}
