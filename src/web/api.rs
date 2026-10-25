//! 읽기 끝점 — 경로마다 자식 `moai --json …` 의 argv 틀 하나([`Profile`])와, 요청에서 받는 값의 이름들.
//!
//! **요청에서 온 값은 틀의 자리에만 든다.** 쿼리의 값은 늘 `--긴이름=값` 한 토막이고 경로의 값(id·위키 쪽)은 `--`
//! 뒤의 한 토막이라, `-` 로 시작해도 플래그로 안 읽히고 셸을 안 지난다(자식은 `Command::args` 로 뜬다).
//! 거르개·찾기·정렬의 **뜻**과 거절(`bad_filter` 와 그 말)은 moai 가 낸다 — 서버는 모양(비지 않음·길이·제어 문자·
//! 이름)만 보고 옮긴다. 모양이 틀리면 자식을 띄우기 전에 400 `bad_input` 이다.
//!
//! **`show --archived` 는 산 줄과 아카이브를 함께 낸다**(손으로 쟀다 — 아카이브로 옮긴 줄 하나와 산 줄 하나가
//! 있는 저장소에서 `--archived` 는 둘 다, `--all` 은 산 줄만 냈다. `backlog show` 도 같다). 그래서 페이지는 한 번
//! 부른 답을 그대로 쓰고 산 줄과 합칠 일이 없다.

use crate::i18n::{Lang, fill, say};

/// `/api/rows` 가 싣는 키 — 본문은 안 싣는다(상세가 따로 읽는다). `branch` 는 옆 워크트리에서 온 줄에만 선다
/// (`--worktree`, `cmd::Row::on`) — 빼면 `--fields` 가 그것을 걷어 페이지의 `⎇` 가 영영 안 섰다. 없는 줄에는 키가 없다.
pub const ROW_FIELDS: &str = "id,title,kind,status,derived_status,priority,epic,derived_epic,milestone,derived_milestone,deferred_at,shelved_by,status_since,created_at,updated_at,started_at,done_at,due_on,starts_on,tags,assignee,assignee_email,blocked_by,unread,archived,branch";

/// 받는 값의 이름.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Param {
    Archived,
    Filter,
    Q,
    Fields,
    Sort,
    Reverse,
    Worktree,
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
            Param::Worktree => "worktree",
        }
    }
}

/// 끝점 하나 — 경로, `--json` 뒤의 고정 argv, 받는 이름.
pub struct Profile {
    pub route: &'static str,
    pub head: &'static [&'static str],
    pub params: &'static [Param],
    /// 아카이브를 **달라고 한 때만** 싣는 끝점이다(`/api/tree`, moai-m4ez.4zq) — 다른 끝점은 `archived=1` 을 줘야 싣고
    /// 이쪽도 같은 말이지만, 자식의 기본이 반대라(`tui --json` 은 다 낸다) 안 줬을 때 `--no-archive` 를 얹는다.
    pub without_archive: bool,
}

const LIST: &[Param] = &[Param::Archived, Param::Filter, Param::Q, Param::Fields];
/// 산 줄의 목록만 옆 워크트리를 겹친다(`--worktree`, 탐색기의 `SPC v w`) — backlog 는 겹칠 줄이 없다.
const ROWS: &[Param] = &[Param::Archived, Param::Filter, Param::Q, Param::Fields, Param::Worktree];

/// 자식을 띄우는 끝점들 — **쿼리를 재는 자가 하나다**([`argv`]). 받는 이름이 없는 끝점도 여기 서야 거기 붙인
/// 쿼리가 거절된다(`/api/wiki?worktree=1` 을 말없이 맨 목록으로 답하던 판이 있었다). 경로에 값이 드는
/// `/api/issue/<id>`·`/api/wiki/<slug>` 만 [`argv_of`] 가 따로 짓는다.
pub const PROFILES: &[Profile] = &[
    Profile { route: "/api/status", head: &["status"], params: &[Param::Worktree], without_archive: false },
    Profile { route: "/api/rows", head: &["show", "--all"], params: ROWS, without_archive: false },
    Profile { route: "/api/backlog", head: &["backlog", "show", "--all"], params: LIST, without_archive: false },
    Profile {
        route: "/api/tree",
        head: &["tui", "--deep"],
        params: &[Param::Sort, Param::Reverse, Param::Worktree, Param::Archived],
        without_archive: true,
    },
    Profile { route: "/api/wiki", head: &["wiki", "ls"], params: &[], without_archive: false },
    // 걸어 둔 프로젝트들 — 사용자 설정을 읽으니 어느 자리에서 돌아도 같은 답이다. 페이지는 그 `path` 를
    // `project=` 로 되돌려 보내 그 프로젝트를 읽는다([`super::project_of`]).
    Profile { route: "/api/projects", head: &["project", "ls"], params: &[], without_archive: false },
];

/// 거르개 줄 하나의 상한(바이트) — 줄을 나눠 여러 `--filter` 가 된다.
const FILTER_MAX: usize = 2048;
/// 그 밖의 값 하나의 상한(바이트).
const VALUE_MAX: usize = 512;

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
    /// 값이 그 이름이 받는 모양이 아니다 — 불린은 `1`, `fields` 는 `id`, id 는 이슈 id 의 꼴.
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
    // 경로의 값은 `--` 뒤에 선다 — `-` 로 시작해도 플래그로 안 읽힌다. **위키의 쪽 이름은 모양만 잰다** — `wiki ls`
    // 가 내는 낱말은 파일 자리라 `/`(디렉터리 밑의 쪽)·한글·빈칸이 들고, `wiki show` 는 그것을 걸어 둔 쪽들에서 찾을
    // 뿐이라 자리를 벗어날 길이 없다. id 의 모양으로 재던 판은 그 쪽들을 400 으로 막았다.
    type Check = fn(&str) -> Result<(), Bad>;
    let (head, rest, check, overlaid): (&[&str], &str, Check, bool) =
        match (path.strip_prefix("/api/issue/"), path.strip_prefix("/api/wiki/")) {
            (Some(id), _) => (&["show"], id, issue_id, true),
            (_, Some(slug)) => (&["wiki", "show"], slug, |slug| shape("slug", slug, VALUE_MAX), false),
            _ => return None,
        };
    // **한 줄의 상세도 겹쳐 보기를 알아야 한다**(moai-m4ez.yb1) — 옆 워크트리에만 있는 줄은 `show -- <id>` 가 "없다" 로 답해,
    // 목록과 트리가 그린 줄을 눌러도 상세가 안 열렸다. 켠 페이지만 `worktree=1` 을 보낸다. 위키는 겹칠 것이 없어 안 받는다.
    Some(
        match overlaid {
            true => overlay_of(query),
            false => none_of(query).map(|()| false),
        }
        .and_then(|on| {
            check(rest)?;
            let mut out: Vec<String> = head.iter().map(|s| s.to_string()).collect();
            if on {
                out.push("--worktree".to_string());
            }
            out.push("--".to_string());
            out.push(rest.to_string());
            Ok(out)
        }),
    )
}

/// 걸어 둔 프로젝트의 자리 하나의 상한(바이트) — 경로라 다른 값보다 넉넉하다.
const PROJECT_MAX: usize = 4096;

/// 쿼리에서 `project=` 를 떼어 낸다 — **모든 길이 받는 이름이다**(moai-o14f). 페이지가 고른 걸어 둔 프로젝트를 읽는
/// 자리라 끝점마다 틀에 적지 않고 여기서 한 번 뗀 뒤, 나머지를 끝점이 잰다. 두 번 주면 어느 쪽을 읽을지 모른다.
pub fn split_project(query: &[(String, String)]) -> Result<(Option<String>, Vec<(String, String)>), Bad> {
    let (mut project, mut rest) = (None, Vec::new());
    for (name, value) in query {
        if name != "project" {
            rest.push((name.clone(), value.clone()));
        } else if project.replace(value.clone()).is_some() {
            return Err(Bad::Twice(name.clone()));
        } else {
            shape(name, value, PROJECT_MAX)?;
        }
    }
    Ok((project, rest))
}

/// 쓰는 길 — **읽음 표시 셋과 계획 날 하나다**. 읽음은 사용자 설정 곁의 제 파일에만 적히고 트래커에는 안
/// 적힌다(`moai read --help`, 2026-10-22 사람 결정, moai-ly88). **계획 날(`/api/plan/<id>`)은 트래커에 쓰는 하나뿐인
/// 길이다**(moai-d86e.3ud) — 간트의 막대 끌기가 `moai edit <id> --start <날> --due <날>` 을 자식으로 부르니
/// `store::with_write` 를 지나고, 거절(시작이 기한 뒤·위 기한 넘김)도 `edit` 의 말 그대로다. 끝점이 아니면 `None`.
pub fn write_argv_of(path: &str, query: &[(String, String)]) -> Option<Result<Vec<String>, Bad>> {
    let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if let Some(id) = path.strip_prefix("/api/plan/") {
        return Some(issue_id(id).and_then(|()| plan_argv(id, query)));
    }
    let built = match (path, path.strip_prefix("/api/read/"), path.strip_prefix("/api/read-group/")) {
        ("/api/read-all", _, _) => Ok(strs(&["read", "--all"])),
        (_, Some(id), _) => issue_id(id).map(|()| strs(&["read", "--", id])),
        (_, _, Some(id)) => issue_id(id).map(|()| vec!["read".to_string(), format!("--epic={id}")]),
        _ => return None,
    };
    // **겹쳐 보기를 켠 페이지는 읽음도 그 판에서 적는다**(moai-m4ez.gcp) — 옆 워크트리에만 있는 줄도, 옆이 더 늦게 고친
    // 줄도 보이는 그 `updated_at` 으로 적혀야 [NEW] 가 내린다. `--` 앞에 놓는다(뒤는 값 하나만 선다).
    Some(overlay_of(query).and_then(|on| {
        let mut args = built?;
        if on {
            args.insert(1, "--worktree".to_string());
        }
        Ok(args)
    }))
}

/// `/api/plan/<id>` 의 argv — `start`·`due` 는 **바뀌는 쪽만** 온다(`Issue::shifted` 가 내는 꼴, 페이지는
/// view/gantt-model.ts `shiftPlan`). 안 온 쪽은 `edit` 이 그대로 둔다 — 못 읽는 글이 적힌 쪽을 다시 적으면 그 글을 지운다.
///
/// **값은 날 하나(`YYYY-MM-DD`)만 받는다** — `none`·`+N` 도 `edit` 은 받지만 끌기는 비우지도 오늘에 기대지도 않는다.
/// 둘 다 없으면 쓸 것이 없으니 거절이다. 겹쳐 보기(`worktree`)는 안 받는다: 쓰기는 늘 주 체크아웃의 트래커에 간다.
fn plan_argv(id: &str, query: &[(String, String)]) -> Result<Vec<String>, Bad> {
    let (mut start, mut due) = (None, None);
    for (name, value) in query {
        let slot = match name.as_str() {
            "start" => &mut start,
            "due" => &mut due,
            _ => return Err(Bad::Unknown(name.clone())),
        };
        if crate::model::parse_date(value).is_none() {
            return Err(Bad::Shape(name.clone()));
        }
        if slot.replace(value.clone()).is_some() {
            return Err(Bad::Twice(name.clone()));
        }
    }
    if start.is_none() && due.is_none() {
        return Err(Bad::Empty("start".to_string()));
    }
    let mut out = vec!["edit".to_string()];
    out.extend(start.map(|d| format!("--start={d}")));
    out.extend(due.map(|d| format!("--due={d}")));
    out.extend(["--".to_string(), id.to_string()]);
    Ok(out)
}

/// 쿼리가 `worktree=1` 하나뿐이거나 비었는가 — 받는 이름이 이것뿐인 끝점(`/api/stamp`·`/api/issue/<id>`·읽음 길)이
/// 쓴다. 다른 이름은 [`none_of`] 와 같이 거절한다.
pub fn overlay_of(query: &[(String, String)]) -> Result<bool, Bad> {
    let mut on = false;
    for (name, value) in query {
        if name != "worktree" {
            return Err(Bad::Unknown(name.clone()));
        }
        if on {
            return Err(Bad::Twice(name.clone()));
        }
        if value != "1" {
            return Err(Bad::Shape(name.clone()));
        }
        on = true;
    }
    Ok(on)
}

/// 이 끝점은 쿼리를 안 받는다 — 자식 없이 답하는 `/api/boot` 도 이것으로 잰다(`/api/stamp` 는 `worktree=1` 을 받아
/// [`overlay_of`] 로 잰다).
pub fn none_of(query: &[(String, String)]) -> Result<(), Bad> {
    match query.first() {
        Some((n, _)) => Err(Bad::Unknown(n.clone())),
        None => Ok(()),
    }
}

/// `/api/issue/<id>` 의 id — **이슈 id 의 꼴이어야 한다**([`crate::id::is_valid`]). `show` 의 자리는 종류 낱말
/// (`issue`·`epic`…)도 받아 그 종류의 목록을 내므로, 글자만 재던 판은 `/api/issue/issue` 에 줄 하나 대신 목록을
/// 돌려주었다. 글자는 id 의 자([`crate::git::id_char`])로 재고, 길이는 [`VALUE_MAX`] 안이다.
fn issue_id(text: &str) -> Result<(), Bad> {
    let ok = text.len() <= VALUE_MAX && text.chars().all(crate::git::id_char) && crate::id::is_valid(text);
    ok.then_some(()).ok_or_else(|| Bad::Shape("id".to_string()))
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
///
/// **값은 이름을 푼 [`Param`] 으로 가른다** — 글자로 다시 가르던 판은 `Param` 을 하나 더하면 그 이름이 첫 걸음은
/// 지나고 둘째 걸음의 나머지 갈래로 떨어져, 컴파일러가 아무 말 없이 엉뚱한 플래그를 지었다.
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
    for (p, (name, value)) in seen.into_iter().zip(query) {
        match p {
            Param::Archived | Param::Reverse | Param::Worktree => {
                if value != "1" {
                    return Err(Bad::Shape(name.clone()));
                }
                // 아카이브를 기본으로 싣는 자식에게는 달라는 말이 따로 없다 — 안 줄 때만 뺀다(아래).
                if !(p == Param::Archived && profile.without_archive) {
                    out.push(format!("--{}", p.name()));
                }
            }
            Param::Filter => {
                shape(name, value, FILTER_MAX)?;
                out.extend(crate::query::split_items(value).into_iter().map(|item| format!("--filter={item}")));
            }
            Param::Q => {
                shape(name, value, VALUE_MAX)?;
                out.push(format!("--grep={value}"));
            }
            Param::Fields => {
                shape(name, value, VALUE_MAX)?;
                if value != "id" {
                    return Err(Bad::Shape(name.clone()));
                }
                fields = Some("id");
            }
            Param::Sort => {
                shape(name, value, VALUE_MAX)?;
                // **차례의 말은 moai 의 것이다**(`query::parse_order`) — 모르는 낱말을 자식에게 넘기면 clap 이 `code` 없는
                // 글로 거절해 페이지가 갈래를 못 읽었다(moai-m4ez.4zq). 여기서 재서 `bad_input` 으로 같은 길에 세운다.
                if crate::query::parse_order(value).is_err() {
                    return Err(Bad::Shape(name.clone()));
                }
                out.push(format!("--sort={value}"));
            }
        }
    }
    if profile.without_archive && !query.iter().any(|(n, _)| n == Param::Archived.name()) {
        out.push("--no-archive".to_string());
    }
    if profile.params.contains(&Param::Fields) {
        out.push(format!("--fields={}", fields.unwrap_or(ROW_FIELDS)));
    }
    Ok(out)
}

/// `/api/stamp` 의 표식 — 끝점의 답을 바꾸는 파일들의 (경로·수정 시각·크기)를 접은 16진 글. 안 바뀌면 같고,
/// 쓰면 달라진다. **읽기만 하고 자식이 없다.**
///
/// 접는 것은 트래커(스냅샷·저널·아카이브·`.moai/config.toml`)와 **부르는 사람 쪽 둘** — 사용자 설정(화면 말,
/// 옛 `[read]`)과 그 트래커의 읽음 파일(`/api/rows` 의 `unread`)이다. 트래커만 접던 판은 다른 창에서 `moai read`
/// 를 친 뒤에도 표식이 그대로라, 표식이 움직일 때만 다시 읽는 페이지가 [NEW] 를 내내 세웠다.
///
/// 셈은 [`crate::text::fnv1a64_from`] 이다 — 상수와 그 까닭(`DefaultHasher` 를 안 쓴다)은 거기 한 벌이다.
pub fn stamp(cwd: &std::path::Path, lang: Lang, overlay: bool) -> crate::fail::R<String> {
    let repo = crate::store::Repo::find_from(cwd, || lang)?
        .ok_or_else(|| crate::fail::Fail::new(say(lang, "refuse.not_a_repo")))?;
    let mut marks = vec![(repo.issues_path(), crate::store::stamp(&repo.issues_path()))];
    marks.extend(repo.journal_marks());
    marks.extend(crate::archive::marks(&repo.root)?);
    let config = repo.dir().join("config.toml");
    marks.push((config.clone(), crate::store::stamp(&config)));
    // **겹쳐 보기를 켠 페이지만** 옆 워크트리의 스냅샷도 접는다(moai-m4ez.gcp) — 옆의 쓰기가 겹친 목록의 답을 바꾸기
    // 때문이다. 탐색기가 자리 판정을 지켜보려고 재는 그 표(`worktree::place_marks`: 워크트리 목록·가지 HEAD·옆 스냅샷·
    // `.git`)를 그대로 쓴다 — 읽기 전에 재는 `stat` 뿐이고 git 을 안 띄운다. 끄고 보는 페이지는 옆이 써도 안 움직인다.
    if overlay {
        marks.extend(crate::worktree::place_marks(&repo.root));
    }
    if let Some(user) = crate::user_config::path() {
        let read = crate::read_marks::place_of(&user, &repo.root);
        let sheets = std::iter::once(read.at).chain(read.pending).chain(read.past);
        marks.extend(std::iter::once(user).chain(sheets).map(|p| {
            let s = crate::store::stamp(&p);
            (p, s)
        }));
    }
    let fold = crate::text::fnv1a64_from;
    let mut h = crate::text::FNV64_OFFSET;
    for (path, mark) in marks {
        h = fold(h, path.as_os_str().as_encoded_bytes());
        // 파일이 없는 것과 비어 있는 것은 다르다 — 없으면 0xff 한 바이트로 가른다.
        match mark.and_then(|(t, len)| Some((t.duration_since(std::time::UNIX_EPOCH).ok()?, len))) {
            Some((t, len)) => {
                h = fold(h, &t.as_nanos().to_le_bytes());
                h = fold(h, &len.to_le_bytes());
            }
            None => h = fold(h, &[0xff]),
        }
        // 이웃한 경로 둘이 글자째 이어 붙어 같은 바이트열이 되지 않게 끊는다.
        h = fold(h, &[0x00]);
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
        assert_eq!(argv_of("/api/tree", &[]).unwrap().unwrap(), strs(&["tui", "--deep", "--no-archive"]));
        assert_eq!(argv_of("/api/status", &[]).unwrap().unwrap(), strs(&["status"]));
        assert_eq!(argv_of("/api/wiki", &[]).unwrap().unwrap(), strs(&["wiki", "ls"]));
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
        assert_eq!(v, strs(&["tui", "--deep", "--sort=title", "--reverse", "--no-archive"]));
        // 아카이브를 보이는 페이지만 달라고 한다 — 안 보이는 동안 3천 줄의 95% 를 싣지 않는다(moai-m4ez.4zq).
        let v = argv_of("/api/tree", &q(&[("archived", "1"), ("worktree", "1")])).unwrap().unwrap();
        assert_eq!(v, strs(&["tui", "--deep", "--worktree"]));
        assert!(matches!(argv_of("/api/tree", &q(&[("archived", "0")])).unwrap(), Err(Bad::Shape(_))));
    }

    /// **차례의 말은 자식을 띄우기 전에 잰다** — 모르는 필드·방향·겹침은 `bad_input` 이다(moai-m4ez.4zq). 전에는 clap 이
    /// `code` 없는 글로 거절했다.
    #[test]
    fn a_sort_the_child_would_refuse_is_refused_here() {
        let tree = |sort: &str| argv_of("/api/tree", &q(&[("sort", sort)])).unwrap();
        for ok in ["title", "priority:desc", "priority:desc,title:asc", "updated"] {
            assert!(tree(ok).is_ok(), "{ok}");
        }
        for bad in ["bogus", "priority:up", "priority,priority:desc", "title,", ",title", ":asc"] {
            assert_eq!(tree(bad), Err(Bad::Shape("sort".into())), "{bad:?}");
        }
    }

    #[test]
    fn percent_decoded_text_goes_in_as_it_is() {
        // 퍼센트는 http 가 이미 풀었다 — 여기 오는 것은 풀린 글자다.
        let v = rows(&[("q", "파서")]).unwrap();
        assert!(v.contains(&"--grep=파서".to_string()), "{v:?}");
    }

    /// **id 는 이슈 id 의 꼴이다** — `show` 가 종류 낱말로 읽는 `issue`·`epic`… 은 목록을 내므로 막는다.
    #[test]
    fn an_issue_path_takes_an_issue_id_after_a_double_dash() {
        assert_eq!(argv_of("/api/issue/moai-1abc", &[]).unwrap().unwrap(), strs(&["show", "--", "moai-1abc"]));
        assert_eq!(
            argv_of("/api/issue/ai-argos-1abc.x1y", &[]).unwrap().unwrap(),
            strs(&["show", "--", "ai-argos-1abc.x1y"])
        );
        for bad in ["issue", "epic", "milestone", "backlog", "idea", "--all", "-x", "a b", "a/b", "", "한글-1abc"] {
            let r = argv_of(&format!("/api/issue/{bad}"), &[]).unwrap();
            assert!(matches!(r, Err(Bad::Shape(_))), "{bad:?}: {r:?}");
        }
        assert!(matches!(argv_of("/api/issue/moai-1abc", &q(&[("q", "x")])).unwrap(), Err(Bad::Unknown(_))));
        // 겹쳐 보기를 켠 페이지는 상세도 옆 워크트리를 본다(moai-m4ez.yb1) — `--` 앞에 놓는다. 위키는 겹칠 것이 없다.
        assert_eq!(
            argv_of("/api/issue/moai-1abc", &q(&[("worktree", "1")])).unwrap().unwrap(),
            strs(&["show", "--worktree", "--", "moai-1abc"])
        );
        assert!(matches!(argv_of("/api/issue/moai-1abc", &q(&[("worktree", "0")])).unwrap(), Err(Bad::Shape(_))));
        assert!(matches!(
            argv_of("/api/issue/moai-1abc", &q(&[("worktree", "1"), ("worktree", "1")])).unwrap(),
            Err(Bad::Twice(_))
        ));
        assert!(matches!(argv_of("/api/wiki/guide", &q(&[("worktree", "1")])).unwrap(), Err(Bad::Unknown(_))));
        assert!(argv_of("/api/nope", &[]).is_none());
    }

    /// **위키의 쪽은 `wiki ls` 가 낸 낱말 그대로다** — 디렉터리 밑(`guide/start`), 한글, `_` 로 시작하는 것, 빈칸이
    /// 든 것, `-` 로 시작하는 것까지. 막는 것은 빈 것·제어 문자·상한뿐이다.
    #[test]
    fn a_wiki_path_takes_the_slug_as_it_is_listed() {
        for slug in ["guide", "guide/start", "안내", "_Sidebar", "my notes", "-dash"] {
            let v = argv_of(&format!("/api/wiki/{slug}"), &[]).unwrap().unwrap();
            assert_eq!(v, strs(&["wiki", "show", "--", slug]), "{slug:?}");
        }
        assert!(matches!(argv_of("/api/wiki/", &[]).unwrap(), Err(Bad::Empty(_))));
        assert!(matches!(argv_of("/api/wiki/a\nb", &[]).unwrap(), Err(Bad::Control(_))));
        let long = "a".repeat(VALUE_MAX + 1);
        assert!(matches!(argv_of(&format!("/api/wiki/{long}"), &[]).unwrap(), Err(Bad::Long(_, VALUE_MAX))));
        assert!(argv_of(&format!("/api/wiki/{}", "a".repeat(VALUE_MAX)), &[]).unwrap().is_ok());
    }

    #[test]
    fn a_shape_that_is_wrong_is_refused_before_a_child_runs() {
        assert!(matches!(rows(&[("q", "a"), ("q", "b")]), Err(Bad::Twice(_))));
        assert!(matches!(rows(&[("filter", "a=1"), ("filter", "b=2")]), Err(Bad::Twice(_))));
        assert!(matches!(rows(&[("nope", "1")]), Err(Bad::Unknown(_))));
        // 이 끝점이 안 받는 이름도 모르는 이름이다.
        assert!(matches!(rows(&[("sort", "title")]), Err(Bad::Unknown(_))));
        assert!(matches!(argv_of("/api/tree", &q(&[("q", "x")])).unwrap(), Err(Bad::Unknown(_))));
        assert_eq!(
            argv_of("/api/tree", &q(&[("worktree", "1")])).unwrap().unwrap(),
            strs(&["tui", "--deep", "--worktree", "--no-archive"])
        );
        assert!(matches!(argv_of("/api/wiki", &q(&[("q", "x")])).unwrap(), Err(Bad::Unknown(_))));
        // 받는 이름이 없는 끝점도 같은 자로 잰다 — 말없이 버리면 `?archived=1` 이 맨 보드로 답했다.
        assert!(matches!(argv_of("/api/wiki", &q(&[("worktree", "1")])).unwrap(), Err(Bad::Unknown(_))));
        assert!(matches!(argv_of("/api/status", &q(&[("archived", "1")])).unwrap(), Err(Bad::Unknown(_))));
        // 상세의 롤업 막대·범례는 `/api/status` 에서 오고 n/n 글은 겹친 트리에서 온다 — 둘이 같은 판이어야 한다(moai-m4ez.u6a).
        assert_eq!(argv_of("/api/status", &q(&[("worktree", "1")])).unwrap().unwrap(), strs(&["status", "--worktree"]));
        assert_eq!(argv_of("/api/status", &[]).unwrap().unwrap(), strs(&["status"]));
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

    /// **쓰는 길은 읽음 셋과 계획 날 하나이고, 값은 `--` 뒤나 `--플래그=` 한 토막에 든다**(moai-ly88, moai-d86e.3ud).
    #[test]
    fn writes_are_the_three_read_marks_and_the_plan() {
        let w = |path: &str| write_argv_of(path, &[]).map(|r| r.unwrap());
        assert_eq!(w("/api/read/moai-1abc"), Some(strs(&["read", "--", "moai-1abc"])));
        assert_eq!(w("/api/read-all"), Some(strs(&["read", "--all"])));
        assert_eq!(w("/api/read-group/moai-1abc"), Some(strs(&["read", "--epic=moai-1abc"])));
        for bad in ["/api/read/issue", "/api/read/-e", "/api/read-group/--all", "/api/read/"] {
            assert!(matches!(write_argv_of(bad, &[]), Some(Err(Bad::Shape(_)))), "{bad}");
        }
        assert!(matches!(write_argv_of("/api/read-all", &q(&[("x", "1")])), Some(Err(Bad::Unknown(_)))));
        // 겹쳐 보기를 켠 페이지는 `--worktree` 를 `--` 앞에 실어 보낸다(moai-m4ez.gcp). 그 밖의 값은 거절한다.
        let on = q(&[("worktree", "1")]);
        let w = |path: &str, query: &[(String, String)]| write_argv_of(path, query).unwrap();
        assert_eq!(w("/api/read/moai-1abc", &on), Ok(strs(&["read", "--worktree", "--", "moai-1abc"])));
        assert_eq!(w("/api/read-all", &on), Ok(strs(&["read", "--worktree", "--all"])));
        assert_eq!(w("/api/read-group/moai-1abc", &on), Ok(strs(&["read", "--worktree", "--epic=moai-1abc"])));
        assert_eq!(w("/api/read-all", &q(&[("worktree", "0")])), Err(Bad::Shape("worktree".into())));
        assert_eq!(w("/api/read-all", &q(&[("worktree", "1"), ("worktree", "1")])), Err(Bad::Twice("worktree".into())));
        assert_eq!(overlay_of(&[]), Ok(false));
        assert_eq!(overlay_of(&on), Ok(true));
        assert!(matches!(overlay_of(&q(&[("archived", "1")])), Err(Bad::Unknown(_))));
        // 계획 날 — 바뀌는 쪽만 실리고, 날 하나(`YYYY-MM-DD`)만 받는다. `none`·`+N` 은 끌기가 내지 않는다.
        let plan = |query: &[(&str, &str)]| write_argv_of("/api/plan/moai-1abc", &q(query)).unwrap();
        assert_eq!(
            plan(&[("start", "2026-10-26"), ("due", "2026-10-30")]),
            Ok(strs(&["edit", "--start=2026-10-26", "--due=2026-10-30", "--", "moai-1abc"]))
        );
        assert_eq!(plan(&[("due", "2026-10-30")]), Ok(strs(&["edit", "--due=2026-10-30", "--", "moai-1abc"])));
        assert_eq!(plan(&[("start", "2026-10-26")]), Ok(strs(&["edit", "--start=2026-10-26", "--", "moai-1abc"])));
        assert_eq!(plan(&[]), Err(Bad::Empty("start".into())));
        for v in ["none", "+3", "2026-1-1", "2026-13-01", "-e", ""] {
            assert_eq!(plan(&[("due", v)]), Err(Bad::Shape("due".into())), "{v:?}");
        }
        assert_eq!(plan(&[("due", "2026-10-30"), ("due", "2026-10-31")]), Err(Bad::Twice("due".into())));
        assert_eq!(plan(&[("worktree", "1")]), Err(Bad::Unknown("worktree".into())));
        for bad in ["/api/plan/issue", "/api/plan/--due", "/api/plan/"] {
            assert!(matches!(write_argv_of(bad, &q(&[("due", "2026-10-30")])), Some(Err(Bad::Shape(_)))), "{bad}");
        }
        for none in ["/api/rows", "/api/status", "/api/readall", "/api/read", "/api/plan"] {
            assert!(write_argv_of(none, &[]).is_none(), "{none}");
        }
    }

    /// **`project=` 는 어느 길에서나 떼어 내고, 두 번이면 거절한다**(moai-o14f).
    #[test]
    fn the_project_is_split_off_every_query() {
        let (p, rest) = split_project(&q(&[("filter", "a"), ("project", "/x y"), ("q", "b")])).unwrap();
        assert_eq!(p.as_deref(), Some("/x y"));
        assert_eq!(rest, q(&[("filter", "a"), ("q", "b")]));
        assert_eq!(split_project(&q(&[("q", "b")])).unwrap(), (None, q(&[("q", "b")])));
        assert!(matches!(split_project(&q(&[("project", "/a"), ("project", "/a")])), Err(Bad::Twice(_))));
        assert!(matches!(split_project(&q(&[("project", "")])), Err(Bad::Empty(_))));
        assert!(matches!(split_project(&q(&[("project", "/a\nb")])), Err(Bad::Control(_))));
    }
}
