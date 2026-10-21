//! 읽기 끝점 — 경로마다 자식 `moai --json …` 의 argv 틀 하나([`Profile`])와, 요청에서 받는 값의 이름들.
//!
//! **요청에서 온 값은 틀의 자리에만 든다.** 값은 늘 `--긴이름=값` 한 토막이라 `-` 로 시작해도 플래그로 안 읽히고,
//! 셸을 안 지난다(자식은 `Command::args` 로 뜬다). 거르개·찾기·정렬의 **뜻**과 거절(`bad_filter` 와 그 말)은
//! moai 가 낸다 — 서버는 모양(비지 않음·길이·제어 문자·이름)만 보고 옮긴다. 모양이 틀리면 자식을 띄우기 전에
//! 400 `bad_input` 이다.
//!
//! **`show --archived` 는 산 줄과 아카이브를 함께 낸다**(손으로 쟀다 — 아카이브로 옮긴 줄 하나와 산 줄 하나가
//! 있는 저장소에서 `--archived` 는 둘 다, `--all` 은 산 줄만 냈다. `backlog show` 도 같다). 그래서 페이지는 한 번
//! 부른 답을 그대로 쓰고 산 줄과 합칠 일이 없다.

use crate::i18n::{Lang, fill, say};

/// `/api/rows` 가 싣는 키 — 본문은 안 싣는다(상세가 따로 읽는다).
pub const ROW_FIELDS: &str = "id,title,kind,status,derived_status,priority,epic,derived_epic,milestone,derived_milestone,deferred_at,shelved_by,status_since,created_at,updated_at,started_at,done_at,due_on,tags,assignee,assignee_email,blocked_by,unread,archived";

/// 받는 값의 이름.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Param {
    Archived,
    Filter,
    Q,
    Fields,
    Sort,
    Reverse,
}

impl Param {
    fn name(self) -> &'static str {
        match self {
            Param::Archived => "archived",
            Param::Filter => "filter",
            Param::Q => "q",
            Param::Fields => "fields",
            Param::Sort => "sort",
            Param::Reverse => "reverse",
        }
    }
}

/// 끝점 하나 — 경로, `--json` 뒤의 고정 argv, 받는 이름.
pub struct Profile {
    pub route: &'static str,
    pub head: &'static [&'static str],
    pub params: &'static [Param],
}

const LIST: &[Param] = &[Param::Archived, Param::Filter, Param::Q, Param::Fields];

/// 목록 끝점들. `/api/issue/<id>`·`/api/wiki[/<slug>]` 는 [`argv_of`] 가 따로 짓는다.
pub const PROFILES: &[Profile] = &[
    Profile { route: "/api/rows", head: &["show", "--all"], params: LIST },
    Profile { route: "/api/backlog", head: &["backlog", "show", "--all"], params: LIST },
    Profile { route: "/api/tree", head: &["tui", "--deep"], params: &[Param::Sort, Param::Reverse] },
];

/// 서버가 자식을 띄우기 전에 거절하는 모양 — 말은 [`Bad::said`] 가 고른다.
#[derive(Debug, PartialEq, Eq)]
pub enum Bad {
    /// 모르는 이름이거나 이 끝점이 안 받는 이름.
    Unknown(String),
    /// 같은 이름을 두 번 줬다.
    Twice(String),
    /// 값이 비었다.
    Empty(String),
    /// 값이 상한(바이트)을 넘는다.
    Long(String, usize),
    /// 값에 제어 문자가 있다.
    Control(String),
    /// 값이 그 이름이 받는 모양이 아니다 — 불린은 `1`, `fields` 는 `id`, id·slug 는 글자 몇 개.
    Shape(String),
}

impl Bad {
    pub fn said(&self, lang: Lang) -> String {
        match self {
            Bad::Unknown(n) => fill(say(lang, "web.bad_unknown"), &[("name", n)]),
            Bad::Twice(n) => fill(say(lang, "web.bad_twice"), &[("name", n)]),
            Bad::Empty(n) => fill(say(lang, "web.bad_empty"), &[("name", n)]),
            Bad::Long(n, max) => fill(say(lang, "web.bad_long"), &[("name", n), ("max", &max.to_string())]),
            Bad::Control(n) => fill(say(lang, "web.bad_control"), &[("name", n)]),
            Bad::Shape(n) => fill(say(lang, "web.bad_shape"), &[("name", n)]),
        }
    }
}

/// 경로와 쿼리로 자식의 argv(`--json` 뒤)를 짓는다. 끝점이 아니면 `None`.
pub fn argv_of(path: &str, query: &[(String, String)]) -> Option<Result<Vec<String>, Bad>> {
    if let Some(p) = PROFILES.iter().find(|p| p.route == path) {
        return Some(argv(p, query));
    }
    if path == "/api/wiki" {
        return Some(none_of(query).map(|()| vec!["wiki".into(), "ls".into()]));
    }
    let (head, name, rest): (&[&str], &str, &str) =
        match (path.strip_prefix("/api/issue/"), path.strip_prefix("/api/wiki/")) {
            (Some(id), _) => (&["show"], "id", id),
            (_, Some(slug)) => (&["wiki", "show"], "slug", slug),
            _ => return None,
        };
    Some(none_of(query).and_then(|()| {
        word(name, rest)?;
        let mut out: Vec<String> = head.iter().map(|s| s.to_string()).collect();
        out.push(rest.to_string());
        Ok(out)
    }))
}

/// 이 끝점은 쿼리를 안 받는다.
fn none_of(query: &[(String, String)]) -> Result<(), Bad> {
    match query.first() {
        Some((n, _)) => Err(Bad::Unknown(n.clone())),
        None => Ok(()),
    }
}

/// id·slug — `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`. 첫 글자가 영숫자라 `-` 로 시작하는 플래그꼴이 못 든다.
fn word(name: &str, text: &str) -> Result<(), Bad> {
    let mut chars = text.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    match first && rest && text.len() <= 128 {
        true => Ok(()),
        false => Err(Bad::Shape(name.to_string())),
    }
}

/// 값의 모양 — 비지 않음, 상한, 제어 문자 없음.
fn shape(name: &str, value: &str, max: usize) -> Result<(), Bad> {
    if value.trim().is_empty() {
        return Err(Bad::Empty(name.to_string()));
    }
    if value.len() > max {
        return Err(Bad::Long(name.to_string(), max));
    }
    if value.chars().any(char::is_control) {
        return Err(Bad::Control(name.to_string()));
    }
    Ok(())
}

/// 틀에 쿼리를 채운다. **틀이 안 받는 이름과 두 번 준 이름은 거절**이다 — 하나를 조용히 버리면 페이지가 낸
/// 거르개가 안 먹은 채 전체 목록이 돌아온다.
pub fn argv(profile: &Profile, query: &[(String, String)]) -> Result<Vec<String>, Bad> {
    let mut seen: Vec<Param> = Vec::new();
    for (name, _) in query {
        let Some(p) = profile.params.iter().copied().find(|p| p.name() == name) else {
            return Err(Bad::Unknown(name.clone()));
        };
        // 거르개는 줄을 나눠 여럿이 되는 것이 정상이지만, 이름은 한 번이다.
        if seen.contains(&p) {
            return Err(Bad::Twice(name.clone()));
        }
        seen.push(p);
    }
    let mut out: Vec<String> = profile.head.iter().map(|s| s.to_string()).collect();
    let mut fields = None;
    for (name, value) in query {
        match name.as_str() {
            "archived" | "reverse" => {
                if value != "1" {
                    return Err(Bad::Shape(name.clone()));
                }
                out.push(format!("--{name}"));
            }
            "filter" => {
                shape(name, value, 2048)?;
                for item in crate::query::split_items(value) {
                    out.push(format!("--filter={item}"));
                }
            }
            "q" => {
                shape(name, value, 512)?;
                out.push(format!("--grep={value}"));
            }
            "fields" => {
                shape(name, value, 512)?;
                if value != "id" {
                    return Err(Bad::Shape(name.clone()));
                }
                fields = Some("id");
            }
            _ => {
                shape(name, value, 512)?;
                out.push(format!("--{name}={value}"));
            }
        }
    }
    if profile.params.contains(&Param::Fields) {
        out.push(format!("--fields={}", fields.unwrap_or(ROW_FIELDS)));
    }
    Ok(out)
}

/// FNV-1a 64 — `DefaultHasher` 는 판마다 바뀔 수 있어, 같은 파일이 판을 넘어 같은 표식이어야 하는 자리에 못 쓴다.
fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `/api/stamp` 의 표식 — 탐색기가 다시 읽기를 가를 때 보는 파일들(스냅샷·저널·아카이브·설정)의
/// (경로·수정 시각·크기)를 접은 16진 글. 안 바뀌면 같고, 쓰면 달라진다. **읽기만 하고 자식이 없다.**
pub fn stamp(cwd: &std::path::Path, lang: Lang) -> crate::fail::R<String> {
    let repo = crate::store::Repo::find_from(cwd, || lang)?
        .ok_or_else(|| crate::fail::Fail::new(say(lang, "refuse.not_a_repo")))?;
    let mut marks = vec![(repo.issues_path(), crate::store::stamp(&repo.issues_path()))];
    marks.extend(repo.journal_marks());
    marks.extend(crate::archive::marks(&repo.root)?);
    let config = repo.dir().join("config.toml");
    marks.push((config.clone(), crate::store::stamp(&config)));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, mark) in marks {
        h = fnv(path.to_string_lossy().as_bytes(), h);
        // 파일이 없는 것과 비어 있는 것은 다르다 — 없으면 0xff 한 바이트로 가른다.
        match mark.and_then(|(t, len)| Some((t.duration_since(std::time::UNIX_EPOCH).ok()?, len))) {
            Some((t, len)) => {
                h = fnv(&t.as_nanos().to_le_bytes(), h);
                h = fnv(&len.to_le_bytes(), h);
            }
            None => h = fnv(&[0xff], h),
        }
        // 이웃한 경로 둘이 글자째 이어 붙어 같은 바이트열이 되지 않게 끊는다.
        h = fnv(&[0x00], h);
    }
    Ok(format!("{h:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    fn rows(pairs: &[(&str, &str)]) -> Result<Vec<String>, Bad> {
        argv_of("/api/rows", &q(pairs)).unwrap()
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn with_fields(v: Vec<&str>) -> Vec<String> {
        let mut out = strs(&v);
        out.push(format!("--fields={ROW_FIELDS}"));
        out
    }

    #[test]
    fn an_empty_query_is_the_bare_template() {
        assert_eq!(rows(&[]).unwrap(), with_fields(vec!["show", "--all"]));
        assert_eq!(argv_of("/api/backlog", &[]).unwrap().unwrap(), with_fields(vec!["backlog", "show", "--all"]));
        assert_eq!(argv_of("/api/tree", &[]).unwrap().unwrap(), strs(&["tui", "--deep"]));
    }

    #[test]
    fn a_value_that_looks_like_a_flag_stays_one_token() {
        let v = rows(&[("q", "--json")]).unwrap();
        assert!(v.contains(&"--grep=--json".to_string()), "{v:?}");
        assert_eq!(v.iter().filter(|a| a.starts_with("--grep")).count(), 1);
        assert!(!v.contains(&"--json".to_string()), "{v:?}");
    }

    #[test]
    fn filters_split_into_one_flag_each() {
        let v = rows(&[("filter", "status=todo tag=bug,ui")]).unwrap();
        assert!(v.contains(&"--filter=status=todo".to_string()), "{v:?}");
        assert!(v.contains(&"--filter=tag=bug,ui".to_string()), "{v:?}");
    }

    #[test]
    fn archived_and_fields_and_the_tree_params() {
        let v = rows(&[("archived", "1"), ("fields", "id")]).unwrap();
        assert_eq!(v, strs(&["show", "--all", "--archived", "--fields=id"]));
        let v = argv_of("/api/tree", &q(&[("sort", "title"), ("reverse", "1")])).unwrap().unwrap();
        assert_eq!(v, strs(&["tui", "--deep", "--sort=title", "--reverse"]));
    }

    #[test]
    fn percent_decoded_text_goes_in_as_it_is() {
        // 퍼센트는 http 가 이미 풀었다 — 여기 오는 것은 풀린 글자다.
        let v = rows(&[("q", "파서")]).unwrap();
        assert!(v.contains(&"--grep=파서".to_string()), "{v:?}");
    }

    #[test]
    fn ids_and_slugs_are_words() {
        assert_eq!(argv_of("/api/issue/moai-1abc.x", &[]).unwrap().unwrap(), strs(&["show", "moai-1abc.x"]));
        assert_eq!(argv_of("/api/wiki/guide", &[]).unwrap().unwrap(), strs(&["wiki", "show", "guide"]));
        assert_eq!(argv_of("/api/wiki", &[]).unwrap().unwrap(), strs(&["wiki", "ls"]));
        for bad in ["--all", "-x", ".hidden", "a b", "a/b", "", "한글"] {
            let r = argv_of(&format!("/api/issue/{bad}"), &[]).unwrap();
            assert!(matches!(r, Err(Bad::Shape(_))), "{bad:?}: {r:?}");
        }
        let long = "a".repeat(129);
        assert!(matches!(argv_of(&format!("/api/wiki/{long}"), &[]).unwrap(), Err(Bad::Shape(_))));
        assert!(argv_of(&format!("/api/wiki/{}", "a".repeat(128)), &[]).unwrap().is_ok());
        assert!(argv_of("/api/nope", &[]).is_none());
    }

    #[test]
    fn a_shape_that_is_wrong_is_refused_before_a_child_runs() {
        assert!(matches!(rows(&[("q", "a"), ("q", "b")]), Err(Bad::Twice(_))));
        assert!(matches!(rows(&[("filter", "a=1"), ("filter", "b=2")]), Err(Bad::Twice(_))));
        assert!(matches!(rows(&[("nope", "1")]), Err(Bad::Unknown(_))));
        // 이 끝점이 안 받는 이름도 모르는 이름이다.
        assert!(matches!(rows(&[("sort", "title")]), Err(Bad::Unknown(_))));
        assert!(matches!(argv_of("/api/tree", &q(&[("q", "x")])).unwrap(), Err(Bad::Unknown(_))));
        assert!(matches!(argv_of("/api/wiki", &q(&[("q", "x")])).unwrap(), Err(Bad::Unknown(_))));
        for v in ["true", "0", "", "2", "yes"] {
            assert!(matches!(rows(&[("archived", v)]), Err(Bad::Shape(_))), "{v:?}");
        }
        assert!(matches!(rows(&[("fields", "title")]), Err(Bad::Shape(_))));
        assert!(matches!(rows(&[("q", "")]), Err(Bad::Empty(_))));
        assert!(matches!(rows(&[("filter", "  ")]), Err(Bad::Empty(_))));
        assert!(matches!(rows(&[("q", "a\nb")]), Err(Bad::Control(_))));
        assert!(matches!(rows(&[("q", &"가".repeat(200))]), Err(Bad::Long(_, 512))));
        assert!(rows(&[("q", &"a".repeat(512))]).is_ok());
        assert!(matches!(rows(&[("filter", &"a".repeat(2049))]), Err(Bad::Long(_, 2048))));
        assert!(rows(&[("filter", &"a".repeat(2048))]).is_ok());
    }

    #[test]
    fn the_fnv_is_the_published_one() {
        // 알려진 값: FNV-1a 64 의 "a" → af63dc4c8601ec8c.
        assert_eq!(fnv(b"a", 0xcbf2_9ce4_8422_2325), 0xaf63_dc4c_8601_ec8c);
    }
}
