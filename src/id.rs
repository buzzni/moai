//! 식별자 발급과 형식 검증.
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
//! **점은 부모-자식 파생 관계만 뜻한다.** 에픽·마일스톤 소속은 필드다.

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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
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

    #[test]
    fn accepts_good_shapes() {
        for id in [
            "argos-4aex",
            "argos-4aex.ae3",
            "argos-4aex.ae3.b3e",
            "ai-argos-0000",
            "a-zzzz.999",
        ] {
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
