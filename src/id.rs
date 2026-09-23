//! 식별자 발급과 형식 검증, 그리고 **줄 하나에서 id 를 읽는 한 자**([`id_of`]).
//!
//! 접두어는 그 저장소의 프로젝트명이다 (`.moai/config.toml` 의 `prefix`).
//! `moai` 는 이 도구의 이름이지 접두어가 아니다.
//!
//! 형식은 두 가지뿐이다.
//!
//! | 모양 | 예 |
//! |---|---|
//! | 최상위 | `argos-4aex` |
//! | 자식 | `argos-4aex.ae3`, `argos-4aex.ae3.b3e` (몇 단이든) |
//!
//! **점은 부모-자식 관계다.** 그 위에 소속이 얹힌다 — 부모가 에픽이면 그 에픽이 자식의
//! 소속이고(moai-9t3l), 계획이 세우는 멤버는 그래서 `epic` 을 안 적는다(moai-exh7,
//! `cmd::add::create_drafts`). 읽는 자는 `report::groups` 하나고, 제 `epic` 을 적은 줄은
//! 그것이 이긴다 — 옛 평평한 멤버가 그 모양이다. 마일스톤 소속은 여전히 필드다.

use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// 최상위 id 의 본체 길이. 36^4 = 1,679,616.
const ROOT_LEN: usize = 4;
/// 자식 한 단의 길이. 36^3 = 46,656 — 한 부모 밑의 형제 수는 훨씬 적다.
const CHILD_LEN: usize = 3;

fn base36(mut n: u64, len: usize) -> String {
    let mut out = vec![b'0'; len];
    for slot in out.iter_mut().rev() {
        *slot = ALPHABET[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(out).expect("알파벳이 ASCII")
}

fn segment(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// 씨앗을 돌려 아직 쓰이지 않은 문자열을 찾는다.
///
/// 암호학적 난수가 아니라 씨앗 해시다. 필요한 것은 예측 불가능성이 아니라
/// 다른 브랜치와 겹치지 않을 확률이기 때문이다.
///
/// **카운터를 쓰지 않는다.** 두 브랜치가 각자 같은 순번을 발급하면 머지가
/// 둘 다 받아들여 중복이 생기고, 중복 id 를 되돌리는 비용이 제일 크다.
fn mint(seed: &str, len: usize, taken: &BTreeSet<String>, wrap: impl Fn(&str) -> String) -> String {
    (0u32..)
        .map(|n| {
            let mut h = DefaultHasher::new();
            (seed, n).hash(&mut h);
            wrap(&base36(h.finish(), len))
        })
        .find(|id| !taken.contains(id))
        .expect("36^n 개가 다 차지 않는다")
}

/// 새 최상위 id. `taken` 은 **락 안에서 읽은** 집합이어야 한다.
pub fn generate(prefix: &str, taken: &BTreeSet<String>, seed: &str) -> String {
    mint(seed, ROOT_LEN, taken, |body| format!("{prefix}-{body}"))
}

/// `parent` 밑의 새 자식 id. 부모 카운터를 쓰지 않는 이유는 `mint` 참조.
pub fn generate_child(parent: &str, taken: &BTreeSet<String>, seed: &str) -> String {
    mint(seed, CHILD_LEN, taken, |body| format!("{parent}.{body}"))
}

/// 호출마다 다른 씨앗. 제목이 같은 이슈를 같은 초에 두 번 만들어도 갈린다.
pub fn seed(title: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{title}\u{0}{nanos}\u{0}{}", std::process::id())
}

/// 형식에 맞는가. 접두어가 무엇인지는 보지 않는다 — 그건 `has_prefix` 다.
///
/// 접두어에 `-` 가 들어가도(`ai-argos`) 마지막 `-` 를 기준으로 갈리므로
/// 모호하지 않다. 본체는 소문자·숫자뿐이라 `-` 를 담을 수 없기 때문이다.
pub fn is_valid(id: &str) -> bool {
    let Some((prefix, body)) = id.rsplit_once('-') else {
        return false;
    };
    if prefix.is_empty() {
        return false;
    }
    let mut parts = body.split('.');
    let Some(root) = parts.next() else {
        return false;
    };
    segment(root, ROOT_LEN) && parts.all(|p| segment(p, CHILD_LEN))
}

/// **줄 하나가 쓰고 있는 id.** 이슈로 안 읽히는 줄에서도 id 까지는 대개 읽힌다.
///
/// **이 저장소에서 줄의 id 를 읽는 자는 여기 하나다**(moai-ijfy). `store::parse_issues` 의
/// `LoadError.id` 와 `cmd::merge_driver::row` 가 같은 자를 쓴다 — 갈려 있던 동안, 머지
/// 드라이버가 짝지은 깨진 줄의 id 를 `Load::reserved_ids` 가 안 잡아 두어 `id::generate` 가
/// 그 id 를 다시 지을 수 있었고, `report` 의 `duplicate_id` 도 그 id 를 못 대어
/// `moai status` 가 `Unreadable rows` 만 말했다. 산 줄과 그 줄의 깨진 쌍둥이가 함께 선 파일이
/// 바로 그 판이고, 머지 드라이버는 그것을 보는데 쓰기 경로는 못 봤다.
///
/// 두 단이다 — `serde_json::Value` 로 한 단 아래에서 읽고([`in_value`]), 그것도 지면 [`scraped`]
/// 가 머리에서 긁는다. 앞엣것이 본체와 같은 파서라 어긋날 자리가 없고, 뒤엣것은 이 도구가 제
/// 손으로 쓰는 한 꼴만 읽는다.
///
/// **두 단을 다 이름 붙여 둔 것은 부르는 쪽이 갈라서 쓰기 때문이다**(리뷰 moai-47yo.76c).
/// `cmd::merge_driver::row` 는 `Value` 를 제가 이미 들고 있어(읽은 값을 [`crate::model::Issue`]
/// 로 한 번 더 재 본다) 이 문을 통째로 못 부르는데, 윗단을 손으로 베껴 적으면 그 베낌이 곧
/// 둘째 파서다 — moai-ijfy 가 없앤 그 갈림이 이름만 바꿔 돌아온다.
pub fn id_of(line: &str) -> Option<String> {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(v) => in_value(&v).map(str::to_string),
        Err(_) => scraped(line),
    }
}

/// [`id_of`] 의 **윗단** — 이미 읽은 `Value` 가 쓰고 있는 id.
///
/// `Value` 를 손에 쥔 자리가 이것을 부른다(`cmd::merge_driver::row`). 한 줄짜리지만 자리마다
/// 손으로 적으면 언젠가 한 곳이 갈리고, 갈린 그날 머지 드라이버가 짝지은 id 와 `moai status` 가
/// 대는 id 가 달라진다.
pub fn in_value(v: &serde_json::Value) -> Option<&str> {
    v.get("id")?.as_str()
}

/// **JSON 이 깨진 줄의 머리에서 `id` 만 글자로 긁는다**(moai-47yo.q3o, 2026-09-23 사용자 결정).
///
/// 짝짓고 잡아 두는 데만 쓴다 — 여기서 돌려받은 id 로 줄을 고쳐 쓰는 일은 없다.
///
/// **이 도구가 제 손으로 쓰는 한 꼴만 읽는다.** `store::render_issues` 는 `Issue` 를 필드
/// 차례로 직렬화하고 `id` 가 첫 필드라 줄이 언제나 `{"id":"…"` 로 선다. 넓히지 않는 것은
/// 헐거운 짐작이 **틀린 짝**을 짓기 때문이다: 한 자리라도 어긋나면 남의 이슈를 사람에게
/// 내민다. 머리가 이 꼴이 아닌 줄은 열쇠 없는 줄로 떨어진다.
///
/// **이 말을 지키는 자는 `cmd::merge_driver` 의 `the_head_scrape_matches_the_shape_store_writes`
/// 다.** `id` 를 뒤로 옮기는 날 그 시험이 붉어진다 — `src/model.rs` 의 판 50개가 모두 `id` 를
/// 첫 필드로 두었고 `#[serde(rename)]` 도 `skip_serializing_if` 도 붙은 적이 없다.
///
/// **처음 이 자리에 적혔던 수는 걷었다**(리뷰 moai-47yo.era 6번). "판 1,236개 1,161,083줄이
/// 예외 없이 그랬다" 는 맞는 수였지만 **이 함수가 못 보는 줄을 센 것**이다 — 부르는 자는
/// `serde_json` 이 진 줄에서만 여기 오는데, 그 1,161,083줄 가운데 진 줄이 하나도 없다. 겨눈
/// 모집단에서 다시 재니(줄 400개를 바이트 자리마다 잘라 낸 236,989벌) 틀린 짝은 0이었고, 못
/// 긁은 것이 6,652벌이다. 수를 안 남기는 것은 그 수가 트래커 커밋마다 늘어 다음 사람이 "더
/// 쌓였다" 와 "규칙이 깨졌다" 를 못 가르기 때문이다.
///
/// `\` 가 든 id 는 안 받는다. 따옴표 이스케이프가 끼면 여기서 자른 자리가 진짜 id 의 끝이
/// 아닌데, moai 가 짓는 id 에는 `\` 가 없어 걸러도 잃는 것이 없다.
///
/// **머리에 통째로 선 값이 있는 줄은 안 받는다**(리뷰 moai-47yo.era 1번). 줄바꿈 하나가 빠져
/// 이슈 둘이 한 줄에 붙으면 그 줄도 JSON 이 아닌데, 짝지으면 **앞 이슈의 표식 안에 뒤 이슈가
/// 통째로 실린다** — 뒤 이슈는 제 줄을 잃어 거기 말고는 어디에도 없으니, 사람이 저쪽을 골라 그
/// 칸을 지우는 순간 아무 자취 없이 사라진다(조용한 손실). 짝을 안 지으면 그 줄은 표식 **밖**에
/// 실려 나가 어느 쪽을 골라도 남는다. 꼬리가 잘린 줄은 머리에 통째로 선 값이 없어 여기 안 걸린다.
///
/// **`id` 가 한 줄에 두 번 적힌 줄에서는 [`id_of`] 의 윗단과 답이 갈린다**(리뷰
/// moai-47yo.era 4번). 여기는 머리에서 첫 `id` 를 집고 `Value` 는 겹친 키를 마지막 것으로
/// 접는다. 뒤엣것을 집으러 줄 전체를 훑는 것은 이 자가 "머리 한 꼴만 읽는다" 를 그만두는 일이라
/// 안 한다 — 그런 줄은 `store::parse_issues` 도 못 읽는 줄로 세니 `moai status` 가 이미 치명으로
/// 말한다.
pub fn scraped(line: &str) -> Option<String> {
    // **싼 자부터 잰다**(리뷰 moai-47yo.76c). 아래 넷은 모두 `그리고` 로 묶이니 차례를 바꿔도
    // 답이 안 바뀌는데, 맨 아래 것만 줄 전체를 한 번 훑는다 — 머리가 이 꼴이 아닌 줄(엉킨 줄,
    // `id` 가 첫 필드가 아닌 줄, 이슈가 아닌 줄)은 그 훑기에 닿기 전에 여기서 떨어진다.
    let rest = line.trim_start().strip_prefix(r#"{"id":""#)?;
    let (id, _) = rest.split_once('"')?;
    // **긁은 것이 id 꼴이 아니면 안 받는다.** 긁기는 짐작이고, 받아들일 짐작은 이 도구가 제 손으로
    // 짓는 꼴 하나다 — `Issue::validate` 가 쓰는 길목에서 그 꼴을 이미 요구하므로([`is_valid`]),
    // 여기서 같은 자를 쓰는 것이 곧 "쓰는 꼴만 되읽는다" 이다. 줄이 엉켜 따옴표가 한참 뒤에야
    // 나오면 긁힌 값이 몇 킬로바이트짜리 "id" 가 되는데, 본체를 재는 이 자가 그것을 막는다.
    //
    // **다만 [`is_valid`] 는 접두어를 안 본다**(리뷰 moai-47yo.76c) — 길이도 글자도 안 재어
    // `"A"×300 + ESC + "zz-0001"` 같은 값이 지난다. 그 값은 이제 `LoadError::id` 를 지나
    // `report` 의 `duplicate_id` 로, 곧 보드와 `status --json` 으로 나가므로 여기서 제어문자를
    // 따로 막는다. 접두어가 제어문자를 담는 일은 이 도구가 짓는 꼴에 없어, 막아도 잃는 것이 없다.
    // 남은 길이는 이 자리의 것이 아니다 — `Value` 로 읽히는 줄도 같은 길로 나가므로, 자른다면
    // 내보내는 쪽에서 자른다.
    //
    // `\` 는 따로 막는다 — 접두어는 [`is_valid`] 가 글자를 안 보는 자리라 `a\b-0001` 이 지나는데,
    // 이스케이프된 따옴표에서 잘린 값이 바로 그 꼴이다.
    if id.contains('\\') || id.chars().any(char::is_control) || !is_valid(id) {
        return None;
    }
    // **통째로 선 값이 머리에 있는가.** `from_str` 은 "뒤에 글자가 남았다" 와 "줄이 먼저 끝났다"
    // 를 둘 다 탈로만 내어 못 가른다. 한 값만 읽어 보면 갈린다 — 읽히면 앞이 온전한 줄이다.
    if serde_json::Deserializer::from_str(line).into_iter::<serde_json::Value>().next().is_some_and(|v| v.is_ok()) {
        return None;
    }
    Some(id.to_string())
}

/// `argos-4aex.ae3` → `argos-4aex`. 최상위면 `None`.
///
/// **부모는 저장하지 않고 여기서 유도한다.** 필드로 두면 id 와 필드 중
/// 어느 쪽이 진실인지 정할 방법이 없고, 반드시 둘이 어긋난다.
pub fn parent_of(id: &str) -> Option<&str> {
    id.rsplit_once('.').map(|(head, _)| head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn generates_valid_ids() {
        let id = generate("argos", &taken(&[]), "s");
        assert!(is_valid(&id), "{id}");

        let child = generate_child(&id, &taken(&[]), "s");
        assert!(is_valid(&child), "{child}");
        assert_eq!(parent_of(&child), Some(id.as_str()));

        let grand = generate_child(&child, &taken(&[]), "s");
        assert!(is_valid(&grand), "{grand}");
        assert_eq!(parent_of(&grand), Some(child.as_str()));
    }

    #[test]
    fn generate_avoids_taken() {
        let first = generate("argos", &taken(&[]), "s");
        let second = generate("argos", &taken(&[&first]), "s");
        assert_ne!(first, second, "같은 씨앗인데 이미 쓴 id 를 다시 냈다");
        assert!(is_valid(&second));
    }

    /// 한 부모 밑에서 100개를 연달아 발급해도 겹치지 않는다.
    #[test]
    fn many_children_do_not_collide() {
        let parent = "argos-4aex";
        let mut used = taken(&[]);
        for i in 0..100 {
            let id = generate_child(parent, &used, &format!("자식 {i}"));
            assert!(used.insert(id.clone()), "{id} 가 두 번 나왔다");
            assert!(is_valid(&id), "{id}");
        }
    }

    #[test]
    fn prefix_may_contain_dash() {
        let id = generate("ai-argos", &taken(&[]), "s");
        assert!(is_valid(&id), "{id}");
        assert!(id.starts_with("ai-argos-"), "{id}");
    }

    /// 이 도구가 제 손으로 쓰는 줄의 꼴. `store::render_issues` 가 내는 것과 같은 차례다.
    fn written(id: &str) -> String {
        format!(
            "{{\"id\":\"{id}\",\"title\":\"{id} 제목\",\"status\":\"todo\",\
             \"created_at\":\"2026-09-01T00:00:00Z\",\"updated_at\":\"2026-09-01T00:00:00Z\",\
             \"status_since\":\"2026-09-01T00:00:00Z\"}}"
        )
    }

    /// [`scraped`] 는 **이 도구가 제 손으로 쓰는 한 꼴만** 읽는다. 넓히면 틀린 짝이 남의 이슈를
    /// 사람에게 내민다.
    ///
    /// **`src/cmd/merge_driver.rs` 에서 여기로 옮겼다**(moai-ijfy). 긁는 자를 옮기면서 그 시험을
    /// 두고 오면 옮긴 자리에 시험이 하나도 없고, 남은 시험은 옮긴 자를 안 잰다 — 리뷰
    /// moai-47yo.era 8번이 `src/path.rs` 에서 잰 자리와 같은 값이다.
    #[test]
    fn the_head_scrape_reads_one_shape_only() {
        assert_eq!(scraped("{\"id\":\"moai-0001\",\"title\":\"잘린").as_deref(), Some("moai-0001"));
        assert_eq!(scraped("  {\"id\":\"moai-0001\",").as_deref(), Some("moai-0001"), "들여 쓴 줄");
        assert_eq!(scraped("{ \"id\" : \"moai-0001\""), None, "빈칸이 낀 꼴은 이 도구가 안 쓴다");
        assert_eq!(scraped("{\"title\":\"먼저\",\"id\":\"moai-0001\""), None, "id 가 첫 필드가 아니다");
        assert_eq!(scraped("{\"id\":\"\","), None, "빈 id");
        assert_eq!(scraped("{\"id\":\"a\\\"b\","), None, "이스케이프가 낀 id");
        // **id 꼴이 아닌 것은 안 받는다.** 줄이 엉켜 따옴표가 한참 뒤에야 나오면 긁힌 값이 그대로
        // 머지 드라이버의 거절문과 `--json` 의 `conflicts` 에 실린다 — 길이도 글자도 이 한 자가 막는다.
        let tangled = format!("{{\"id\":\"{}\",", "가".repeat(400));
        assert_eq!(scraped(&tangled), None, "id 꼴이 아닌 긴 값을 긁었다");
        assert_eq!(scraped("{\"id\":\"moai\u{0}0001\","), None, "제어문자가 낀 id");
        assert_eq!(scraped("{\"id\":\"제목이다\","), None, "id 꼴이 아니다");
        // **머리에 통째로 선 값이 있으면 안 긁는다** — 줄바꿈 하나가 빠져 이슈 둘이 한 줄에 붙은
        // 줄을 앞 이슈로 짝지으면, 뒤 이슈가 앞 이슈의 표식 안에 통째로 실려 말없이 사라진다.
        let glued = format!("{}{}", written("moai-0001"), written("moai-0002"));
        assert_eq!(scraped(&glued), None, "이슈 둘이 붙은 줄을 앞 이슈로 짝지었다");
        assert_eq!(scraped(&written("moai-0001")), None, "안 깨진 줄은 여기 오지도 않는다");
    }

    /// **줄의 id 를 읽는 자는 하나다**(moai-ijfy). 윗단이 `Value` 고 아랫단이 [`scraped`] 라,
    /// `store::parse_issues` 의 `LoadError.id` 와 머지 드라이버가 같은 것을 본다.
    #[test]
    fn one_reader_answers_what_id_a_line_carries() {
        // 이슈로 읽히는 줄도, `Issue` 로만 안 읽히는 줄도 윗단이 답한다.
        assert_eq!(id_of(&written("moai-0001")).as_deref(), Some("moai-0001"));
        assert_eq!(id_of("{\"id\":\"moai-0001\",\"kind\":\"몰라\"}").as_deref(), Some("moai-0001"));
        // 겹친 키는 `Value` 가 마지막 것으로 접는다 — [`scraped`] 와 갈리는 그 자리다.
        assert_eq!(id_of("{\"id\":\"moai-0001\",\"id\":\"moai-0002\"}").as_deref(), Some("moai-0002"));
        // JSON 이 깨지면 아랫단이 답한다. **머지 드라이버가 짝지은 것과 같은 id 다.**
        let cut = written("moai-0001").strip_suffix('}').expect("쓴 줄은 `}` 로 끝난다").to_string();
        assert_eq!(id_of(&cut).as_deref(), Some("moai-0001"));
        assert_eq!(id_of(&cut), scraped(&cut), "두 단이 같은 줄에서 갈렸다");
        // 둘 다 못 읽는 줄에서는 지어내지 않는다.
        assert_eq!(id_of("{깨짐"), None);
        assert_eq!(id_of("{\"id\":\"moai-00"), None, "따옴표가 안 닫힌 id");
        assert_eq!(id_of("[1,2,3]"), None, "이슈가 아닌 값");
    }

    #[test]
    fn accepts_good_shapes() {
        for id in ["argos-4aex", "argos-4aex.ae3", "argos-4aex.ae3.b3e", "ai-argos-0000", "a-zzzz.999"] {
            assert!(is_valid(id), "{id}");
        }
    }

    #[test]
    fn rejects_bad_shapes() {
        for id in [
            "argos-4ae",       // 최상위는 4자리
            "argos-4aexx",     // 5자리
            "argos-4aex.ae",   // 자식은 3자리
            "argos-4aex.ae3f", // 4자리
            "argos-4AEX",      // 대문자
            "argos-4aex.",     // 빈 자식
            "argos-",
            "-4aex", // 접두어 없음
            "4aex",  // `-` 없음
            "",
        ] {
            assert!(!is_valid(id), "{id} 를 받아들였다");
        }
    }
}
