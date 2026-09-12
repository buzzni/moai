//! 무엇을 보여줄지 고르고 어떤 차례로 놓을지 정한다.
//! **순수 함수다** — `&[Issue]` 만 본다. 나중에 TUI 가 그대로 쓴다.
//!
//! 문법은 한 문장이다:
//!
//! > **쉼표는 "또는", 플래그 반복은 "그리고", 서로 다른 플래그끼리는 "그리고".**
//!
//! 미니 쿼리 언어(`"status=todo AND tag=bug"`)를 두지 않는다. 에이전트는
//! `--help` 를 읽고 명령을 만드는데 플래그는 도움말이 곧 문법이고, 쿼리
//! 문자열은 Bash heredoc 안에서 따옴표가 겹쳐 자주 깨진다. 서로 다른 필드
//! 사이에 OR 이 필요하다는 요청이 실제로 올 때 다시 본다.

use crate::model::{Issue, Kind, days_since};
use std::collections::BTreeMap;

/// `epic=none` 처럼 "값이 없는 것" 을 고르는 자리.
#[derive(Debug, Clone, PartialEq)]
pub enum Sel {
    Unset,
    Is(String),
}

/// 소속은 **묶음 전체를 봐야** 알 수 있다 — 자식은 조상에게서 물려받고,
/// 마일스톤은 에픽을 거쳐 온다. 그래서 이슈 하나만 보고는 못 고른다.
pub struct Where<'a> {
    pub epic: BTreeMap<&'a str, &'a str>,
    pub milestone: BTreeMap<&'a str, &'a str>,
}

impl<'a> Where<'a> {
    pub fn of(all: &'a [Issue]) -> Where<'a> {
        Where {
            epic: crate::report::groups(all),
            milestone: crate::report::milestones(all),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Filter {
    /// OR. 비면 아무거나.
    pub status: Vec<String>,
    /// AND of OR — 바깥이 그리고, 안쪽이 또는.
    pub tags: Vec<Vec<String>>,
    pub no_tags: Vec<String>,
    /// OR. 비면 아무거나. 쉼표가 또는이라는 규칙이 여기에도 걸린다.
    pub epic: Vec<Sel>,
    pub milestone: Vec<Sel>,
    pub parent: Vec<Sel>,
    pub priority: Vec<u8>,
    /// OR. 비면 아무거나. `Sel::Unset` 이 담당 없는 것이다.
    pub assignee: Vec<Sel>,
    pub kind: Option<Kind>,
    pub grep: Option<String>,
    /// 지금 칸에 이만큼 머문 것.
    pub stale: Option<i64>,
    /// done 을 포함한다.
    pub all: bool,
    /// 미뤄 둔 것만 고른다. `Some(false)` 면 미루지 않은 것만.
    pub deferred: Option<bool>,
    /// 담아 둔 생각까지 포함한다. **`all` 과 같은 자리의 축이다** — 기본으로
    /// 숨는 것을 도로 켜는 스위치가 둘이 되면, 켜는 쪽이 어느 것을 켰는지
    /// 매번 되짚어야 한다.
    pub ideas: bool,
}

/// 한 번만 쓸 수 있는 플래그를 두 번 썼을 때. 규칙(반복=그리고)을 지키면서도
/// 사람이 실제로 저지르는 실수를 잡는다.
fn once(values: &[String], flag: &str, what: &str) -> Result<Vec<String>, String> {
    match values {
        [] => Ok(Vec::new()),
        [one] => Ok(csv(one)),
        [a, b, ..] => Err(format!(
            "{what}는 {a} 이면서 동시에 {b} 일 수 없다.\n      \
             둘 중 하나를 찾는 것이면 `{flag} {a},{b}` 다"
        )),
    }
}

/// 있는 필터 항목. 모르는 키를 만나면 이 목록을 그대로 보여준다.
pub const KEYS: &[&str] = &[
    "status", "tag", "no-tag", "epic", "milestone", "parent", "priority", "assignee", "type",
    "grep", "stale",
];

/// 플래그에서 온 날것. `cmd` 가 argv 를 그대로 옮겨 담아 넘긴다.
///
/// 값이 아직 쪼개지지 않은 채로 온다 — `-s todo,review` 와 `-s todo -s review`
/// 를 구별해야 뒤엣것에 친절한 오류를 낼 수 있어서, 쉼표를 clap 에 맡기지 않는다.
#[derive(Debug, Default)]
pub struct Raw {
    pub status: Vec<String>,
    pub tag: Vec<String>,
    pub no_tag: Vec<String>,
    pub epic: Vec<String>,
    pub milestone: Vec<String>,
    pub parent: Vec<String>,
    pub priority: Vec<String>,
    pub assignee: Vec<String>,
    pub kind: Option<Kind>,
    pub grep: Option<String>,
    pub stale: Option<i64>,
    pub all: bool,
    pub ideas: bool,
    pub deferred: bool,
    pub filter: Vec<String>,
}

impl Filter {
    pub fn build(mut raw: Raw) -> Result<Filter, String> {
        // `--filter` 는 **플래그와 같은 자리에 쌓인다.** 뜻을 정하는 코드가
        // 아래 한 곳뿐이라야 두 표현이 갈라지지 않는다 — 예전처럼 `Filter` 에
        // 직접 쓰면 `--filter` 가 플래그를 조용히 덮어썼고, `-s` 를 두 번 썼을
        // 때 나오는 친절한 오류도 그 길에서만 사라졌다.
        for one in std::mem::take(&mut raw.filter) {
            desugar(&mut raw, &one)?;
        }
        // 콕 집어 묻거나(`--type idea`) 글로 찾을 때는 저절로 켜진다.
        // **이미 적어 둔 생각을 다시 안 적으려면 찾아져야 한다.**
        //
        // `--deferred` 도 콕 집어 묻는 자리다. **`status` 의 `미뤄 둔 것 N건`
        // 은 종류를 안 가리고 세므로**(미뤄 둔 에픽·생각까지), 그 줄이 가리키는
        // 명령이 생각을 숨기면 세어 놓고 못 보여 주는 수가 된다 — `idea_pile`
        // 이 미뤄 둔 것을 빼서 피한 바로 그 덫이고, 여기서는 세는 쪽을 못
        // 좁히니(좁히면 미뤄 둔 에픽이 아무 데서도 안 보인다) 보는 쪽을 연다.
        let ideas =
            raw.ideas || raw.kind == Some(Kind::Idea) || raw.grep.is_some() || raw.deferred;
        // **`--deferred` 는 그것만 본다.** 목록 자리에서 미룬 것은 done 처럼
        // 기본으로 빠지므로, 켜는 말과 좁히는 말이 하나여야 "미룬 것 보기" 가
        // 한 낱말로 끝난다.
        let deferred = raw.deferred.then_some(true);
        Ok(Filter {
            status: once(&raw.status, "-s", "상태")?,
            tags: raw
                .tag
                .iter()
                .map(|t| split_tags(t))
                .filter(|v: &Vec<String>| !v.is_empty())
                .collect(),
            no_tags: raw.no_tag.iter().flat_map(|t| split_tags(t)).collect(),
            epic: sel(once(&raw.epic, "-e", "에픽")?),
            milestone: sel(once(&raw.milestone, "--milestone", "마일스톤")?),
            parent: sel(once(&raw.parent, "--parent", "부모")?),
            priority: parse_priorities(&once(&raw.priority, "-p", "우선순위")?)?,
            assignee: sel(once(&raw.assignee, "-a", "담당")?),
            kind: raw.kind,
            // 한 번만 내려 두면 이슈마다 다시 만들 일이 없다.
            grep: raw.grep.map(|q| q.to_lowercase()),
            stale: raw.stale,
            all: raw.all,
            ideas,
            deferred,
        })
    }

    pub fn matches(&self, i: &Issue, now: &str, wh: &Where) -> bool {
        // **담아 둔 생각은 기본 목록에서 빠진다.** 이 자리는 일을 보는
        // 자리고, 거기에 생각 조각이 섞이면 목록이 흐려진다 — 흐려지면
        // 담기가 꺼려지고, 담는 비용을 0 으로 만든 뜻이 사라진다.
        //
        // 콕 집어 묻거나(`--type idea`) 글로 찾을 때는 나온다. **이미 적어
        // 둔 생각을 다시 안 적으려면 찾아져야 한다.**
        //
        // 여기 한 곳에서만 정한다. 탐색기는 `ideas` 를 켜고 들어와 시키지도
        // 않은 줄을 숨기지 않는다 — `all` 을 켜는 것과 같은 까닭이고, 같은
        // 자리다. 술어가 갈라지면 화면과 CLI 가 다른 것을 센다.
        if crate::report::is_idea(i) && !self.ideas {
            return false;
        }
        // **미뤄 둔 것은 done 과 같은 자리에서 빠진다.** 지금 계획이 아니라는
        // 뜻이 같고, 켜는 말(`--all`)도 같아야 축이 안 는다.
        match self.deferred {
            Some(want) if i.is_deferred() != want => return false,
            None if !self.all && i.is_deferred() => return false,
            _ => {}
        }
        if !self.all && self.status.is_empty() && i.status.is_done() {
            return false;
        }
        if !self.status.is_empty() && !self.status.iter().any(|s| s == i.status.as_str()) {
            return false;
        }
        if !self.tags.iter().all(|any| any.iter().any(|t| i.tags.contains(t))) {
            return false;
        }
        if self.no_tags.iter().any(|t| i.tags.contains(t)) {
            return false;
        }
        // 소속은 **물려받은 것까지** 본다. `-e X` 가 X 밑의 손자를 빠뜨리면
        // 트리가 보여 주는 것과 목록이 고르는 것이 달라진다.
        if !matches_sel(&self.epic, wh.epic.get(i.id.as_str()).copied()) {
            return false;
        }
        if !matches_sel(&self.milestone, wh.milestone.get(i.id.as_str()).copied()) {
            return false;
        }
        if !matches_sel(&self.parent, crate::id::parent_of(&i.id)) {
            return false;
        }
        if !self.priority.is_empty() && !self.priority.contains(&i.priority()) {
            return false;
        }
        if !self.assignee.is_empty() && !self.assignee.iter().any(|w| is_assignee(w, i)) {
            return false;
        }
        if self.kind.is_some_and(|k| i.kind != k) {
            return false;
        }
        if let Some(q) = &self.grep {
            let hit = i.title.to_lowercase().contains(q)
                || i.body.as_deref().is_some_and(|b| b.to_lowercase().contains(q));
            if !hit {
                return false;
            }
        }
        if let Some(d) = self.stale
            && days_since(&i.status_since, now).is_none_or(|n| n < d)
        {
            return false;
        }
        true
    }
}

/// `--filter k=v` 를 플래그와 같은 자리(`Raw`)에 풀어 놓는다. **뜻을 정하지
/// 않는다** — 쪼개고 고르는 일은 `build` 한 곳이 한다.
///
/// **한 번에 한 항목이다.** `;` 로 여럿을 받던 것을 걷어냈다 — 그러면
/// `--filter grep=a;b` 의 `;` 가 글자가 아니라 구분자가 되고, 제목에
/// 세미콜론이 든 이슈를 영영 못 찾는다. 여럿은 플래그를 되풀이한다.
fn desugar(raw: &mut Raw, text: &str) -> Result<(), String> {
    {
        let one = text.trim();
        if one.is_empty() {
            return Ok(());
        }
        let (k, v) = one
            .split_once('=')
            .ok_or_else(|| format!("`{one}` 은 `항목=값` 이 아니다. 있는 항목: {}", KEYS.join(", ")))?;
        let (k, v) = (k.trim(), v.trim().to_string());
        match k {
            "status" => raw.status.push(v),
            "tag" => raw.tag.push(v),
            "no-tag" => raw.no_tag.push(v),
            "epic" => raw.epic.push(v),
            "milestone" => raw.milestone.push(v),
            "parent" => raw.parent.push(v),
            "priority" => raw.priority.push(v),
            "assignee" => raw.assignee.push(v),
            "type" => raw.kind = Some(v.parse()?),
            "grep" => raw.grep = Some(v),
            "stale" => raw.stale = Some(v.parse().map_err(|_| format!("`{v}` 는 날 수가 아니다"))?),
            _ => {
                return Err(format!(
                    "`{k}` 라는 필터 항목이 없다.\n      있는 것: {}",
                    KEYS.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// `a, b ,` → `["a", "b"]`. 쉼표는 또는이라는 규칙이 사는 곳.
fn csv(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// `bug,#parser` → `["bug", "parser"]`. 앞의 `#` 은 있어도 없어도 된다.
fn split_tags(raw: &str) -> Vec<String> {
    csv(raw).iter().map(|t| crate::model::normalize_tag(t)).filter(|s| !s.is_empty()).collect()
}

/// 쉼표는 여기서도 또는이다. `none` 이 섞이면 "없는 것" 도 함께 고른다.
fn sel(values: Vec<String>) -> Vec<Sel> {
    values
        .into_iter()
        .map(|v| if v == "none" { Sel::Unset } else { Sel::Is(v) })
        .collect()
}

/// 담당 한 항을 잰다. **맡길 때 쓴 문자열 그대로 찾을 수 있어야 한다** —
/// 가르는 일을 `model::split_assignee` 에 맡기는 이유다. `-a` 로 맡기는 쪽과
/// 같은 함수라, `이름 (메일)`·이름만·메일만 셋이 저절로 같이 통한다.
///
/// 이름과 메일 중 **하나만 맞아도** 통과다. 이름을 바꾼 사람이 옛 줄에서
/// 사라지지 않고, 남의 메일을 모르는 채 이름으로만 맡긴 줄도 찾힌다.
fn is_assignee(want: &Sel, i: &crate::model::Issue) -> bool {
    let Sel::Is(raw) = want else { return i.assignee.is_none() };
    let (name, email) = crate::model::split_assignee(raw);
    let by_name = name.as_deref().is_some_and(|n| i.assignee.as_deref() == Some(n));
    // 메일은 대소문자를 가리지 않는다. 같은 사람이 저장소마다 다르게 적는다.
    let mail = |e: &str| i.assignee_email.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(e));
    // 괄호 없이 준 것은 `split_assignee` 가 이름으로 본다. 메일일 수도 있어 한 번 더 잰다.
    by_name || email.as_deref().is_some_and(&mail) || mail(raw)
}

fn matches_sel(sel: &[Sel], value: Option<&str>) -> bool {
    sel.is_empty()
        || sel.iter().any(|s| match s {
            Sel::Unset => value.is_none(),
            Sel::Is(want) => value == Some(want.as_str()),
        })
}

fn parse_priorities(raw: &[String]) -> Result<Vec<u8>, String> {
    raw.iter()
        .map(|p| {
            // `p` 한 글자만 벗긴다. `trim_start_matches` 는 `ppp0` 도 받아들인다.
            p.strip_prefix('p')
                .unwrap_or(p.as_str())
                .parse::<u8>()
                .ok()
                .filter(|n| *n <= crate::model::MAX_PRIORITY)
                .ok_or_else(|| format!("`{p}` 는 우선순위가 아니다. 0~{} 다", crate::model::MAX_PRIORITY))
        })
        .collect()
}

/// 화면에 놓는 차례: 우선순위 → id. 급한 것이 위로 오고, 나머지는 파일과
/// 같은 순서다.
///
/// **차례를 정하는 곳은 여기 하나다.** 목록·에픽 표·탐색기가 저마다 같은 규칙을
/// 다시 적으면 언젠가 하나만 고쳐지고, 그러면 한 화면 안에서 차례가 둘이 되어
/// 보는 쪽이 규칙을 못 세운다. 실제로 에픽 표만 파일 순으로 남아 있었다.
pub fn display_order(a: &Issue, b: &Issue) -> std::cmp::Ordering {
    a.priority().cmp(&b.priority()).then_with(|| a.id.cmp(&b.id))
}

pub fn sort_for_display(issues: &mut [Issue]) {
    issues.sort_by(display_order);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    const NOW: &str = "2026-09-11T00:00:00Z";

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn issue(id: &str, status: &str, tags: &[&str]) -> Issue {
        let mut i = Issue::new(
            id.into(),
            format!("{id} 제목"),
            Kind::Issue,
            Status::new(status),
            "2026-09-01T00:00:00Z",
        );
        i.tags = s(tags);
        i
    }

    /// 한 이슈만 두고 고르기. 소속 지도는 그 이슈에서 뽑는다.
    fn hit(f: &Filter, i: &Issue) -> bool {
        let all = [i.clone()];
        f.matches(i, NOW, &Where::of(&all))
    }

    fn f() -> Filter {
        Filter { all: true, ..Filter::default() }
    }

    /// 쉼표는 또는.
    #[test]
    fn commas_are_or() {
        let f = Filter::build(Raw { status: s(&["todo,review"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &[])));
        assert!(hit(&f, &issue("a-0001", "review", &[])));
        assert!(!hit(&f, &issue("a-0001", "done", &[])));
    }

    /// 반복은 그리고.
    #[test]
    fn repeating_a_tag_is_and() {
        let f = Filter::build(Raw { tag: s(&["bug", "p1"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &["bug", "p1"])));
        assert!(!hit(&f, &issue("a-0001", "todo", &["bug"])));
    }

    #[test]
    fn a_comma_inside_one_tag_flag_is_or() {
        let f = Filter::build(Raw { tag: s(&["bug,chore"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &["chore"])));
        assert!(!hit(&f, &issue("a-0001", "todo", &["perf"])));
    }

    /// 한 이슈가 두 칸에 동시에 있을 수 없다는 것을 알려 준다.
    #[test]
    fn repeating_status_is_a_friendly_error() {
        let e = Filter::build(Raw { status: s(&["todo", "review"]), all: true, ..Raw::default() }).unwrap_err();
        assert!(e.contains("동시에") && e.contains("-s todo,review"), "{e}");
    }

    #[test]
    fn none_selects_the_unset() {
        let mut with = issue("a-0001", "todo", &[]);
        with.epic = Some("a-9999".into());
        let without = issue("a-0002", "todo", &[]);

        let f = Filter::build(Raw { epic: s(&["none"]), all: true, ..Raw::default() }).unwrap();
        assert!(!hit(&f, &with) && hit(&f, &without));

        let f = Filter::build(Raw { epic: s(&["a-9999"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &with) && !hit(&f, &without));
    }

    /// `--parent none` 은 최상위만. 부모는 id 에서 유도된다.
    #[test]
    fn parent_none_is_top_level_only() {
        let f = Filter::build(Raw { parent: s(&["none"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &[])));
        assert!(!hit(&f, &issue("a-0001.abc", "todo", &[])));
    }

    #[test]
    fn no_tag_excludes() {
        let f = Filter::build(Raw { no_tag: s(&["wontfix"]), all: true, ..Raw::default() }).unwrap();
        assert!(!hit(&f, &issue("a-0001", "todo", &["wontfix"])));
        assert!(hit(&f, &issue("a-0001", "todo", &["bug"])));
    }

    #[test]
    fn grep_reads_title_and_body() {
        let mut i = issue("a-0001", "todo", &[]);
        i.body = Some("BOM 이 섞여 있다".into());
        let mut f = f();
        f.grep = Some("bom".into()); // 대소문자를 가리지 않는다
        assert!(hit(&f, &i));
        f.grep = Some("없는말".into());
        assert!(!hit(&f, &i));
    }

    /// `--stale` 은 **지금 칸에 머문 기간**이다. 리뷰가 썩는 것을 찾는 데 쓴다.
    #[test]
    fn stale_counts_time_in_the_current_column() {
        let mut i = issue("a-0001", "review", &[]);
        i.status_since = "2026-09-05T00:00:00Z".into(); // 6일
        let mut f = f();
        f.stale = Some(3);
        assert!(hit(&f, &i));
        f.stale = Some(7);
        assert!(!hit(&f, &i));
    }

    /// 기본은 done 을 뺀다. 칸을 콕 집으면 그 말을 따른다.
    #[test]
    fn done_is_hidden_unless_asked_for() {
        let done = issue("a-0001", "done", &[]);
        assert!(!hit(&Filter::default(), &done));
        assert!(hit(&Filter { all: true, ..Filter::default() }, &done));
        let named = Filter::build(Raw { status: s(&["done"]), ..Raw::default() }).unwrap();
        assert!(hit(&named, &done));
    }

    /// 쉼표는 에픽·부모에서도 또는이다. 첫 값만 보고 나머지를 버리면
    /// 두 에픽을 한 번에 훑는 요청이 조용히 반쪽 답을 낸다.
    #[test]
    fn a_comma_is_or_for_epic_and_parent_too() {
        let mut a = issue("a-0001", "todo", &[]);
        a.epic = Some("a-9998".into());
        let mut b = issue("a-0002", "todo", &[]);
        b.epic = Some("a-9999".into());
        let c = issue("a-0003", "todo", &[]);

        for spelling in [
            Raw { epic: s(&["a-9998,a-9999"]), all: true, ..Raw::default() },
            Raw { epic: s(&["a-9999,a-9998"]), all: true, ..Raw::default() },
            Raw { filter: s(&["epic=a-9998,a-9999"]), all: true, ..Raw::default() },
        ] {
            let f = Filter::build(spelling).unwrap();
            assert!(hit(&f, &a) && hit(&f, &b) && !hit(&f, &c));
        }

        // `none` 도 다른 값과 나란히 놓일 수 있다.
        let f = Filter::build(Raw { epic: s(&["none,a-9999"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &b) && hit(&f, &c) && !hit(&f, &a));

        let f = Filter::build(Raw { parent: s(&["a-0001,a-0002"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001.abc", "todo", &[])));
        assert!(hit(&f, &issue("a-0002.abc", "todo", &[])));
        assert!(!hit(&f, &issue("a-0003.abc", "todo", &[])));
    }

    /// `--filter` 는 플래그를 덮어쓰지 않고 **같은 자리에 쌓인다.** 두 표현이
    /// 한 뜻이라면 두 번 쓴 것을 나무라는 자리도 하나여야 한다.
    #[test]
    fn a_filter_string_stacks_with_the_flags_it_mirrors() {
        let e = Filter::build(Raw { status: s(&["todo"]), filter: s(&["status=review"]), ..Raw::default() })
            .unwrap_err();
        assert!(e.contains("동시에"), "{e}");
        let e = Filter::build(Raw { filter: s(&["status=todo", "status=review"]), ..Raw::default() })
            .unwrap_err();
        assert!(e.contains("동시에"), "{e}");

        // 태그는 쌓이는 쪽이라 둘 다 걸린다 (반복=그리고).
        let f = Filter::build(Raw { tag: s(&["bug"]), filter: s(&["tag=parser"]), all: true, ..Raw::default() })
            .unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &["bug", "parser"])));
        assert!(!hit(&f, &issue("a-0002", "todo", &["bug"])));

        // 빈 값은 "거르지 않는다" 다 — 플래그 쪽과 같은 뜻이어야 한다.
        let f = Filter::build(Raw { filter: s(&["tag="]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &[])));
    }

    /// `--filter` 는 플래그와 정확히 같은 뜻이다.
    #[test]
    fn filter_string_equals_the_flags() {
        let flags = Filter::build(Raw { status: s(&["todo"]), tag: s(&["bug"]), epic: s(&["none"]), ..Raw::default() }).unwrap();
        let string = Filter::build(Raw { filter: s(&["status=todo", "tag=bug", "epic=none"]), ..Raw::default() }).unwrap();
        for i in [
            issue("a-0001", "todo", &["bug"]),
            issue("a-0002", "review", &["bug"]),
            issue("a-0003", "todo", &["perf"]),
        ] {
            assert_eq!(hit(&flags, &i), hit(&string, &i), "{}", i.id);
        }
    }

    #[test]
    fn unknown_filter_keys_list_the_real_ones() {
        let e = Filter::build(Raw { filter: s(&["statu=todo"]), ..Raw::default() }).unwrap_err();
        assert!(e.contains("statu") && e.contains("status, tag"), "{e}");
        let e = Filter::build(Raw { filter: s(&["todo"]), ..Raw::default() }).unwrap_err();
        assert!(e.contains("항목=값"), "{e}");
    }

    #[test]
    fn priorities_accept_both_spellings() {
        let f = Filter::build(Raw { priority: s(&["p0,1"]), all: true, ..Raw::default() }).unwrap();
        assert_eq!(f.priority, [0, 1]);
        for bad in ["9", "ppp0"] {
            let e = Filter::build(Raw { priority: s(&[bad]), all: true, ..Raw::default() }).unwrap_err();
            assert!(e.contains("우선순위가 아니다"), "{bad} → {e}");
        }
    }

    /// 이름으로 담당 필터
    #[test]
    fn assignee_matches_by_name() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("chulsu@example.com".into());
        let f = Filter::build(Raw { assignee: s(&["철수"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));
        let f = Filter::build(Raw { assignee: s(&["영희"]), all: true, ..Raw::default() }).unwrap();
        assert!(!hit(&f, &i));
    }

    /// 메일로 담당 필터
    #[test]
    fn assignee_matches_by_email() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("chulsu@example.com".into());
        let f = Filter::build(Raw { assignee: s(&["chulsu@example.com"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));
        let f = Filter::build(Raw { assignee: s(&["other@example.com"]), all: true, ..Raw::default() }).unwrap();
        assert!(!hit(&f, &i));
    }

    /// 쉼표는 또는 — 이름 하나와 남의 메일 하나를 주면 둘 중 하나만 맞아도 통과.
    #[test]
    fn assignee_commas_are_or() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("chulsu@example.com".into());
        let f = Filter::build(Raw {
            assignee: s(&["철수,other@example.com"]),
            all: true,
            ..Raw::default()
        })
        .unwrap();
        assert!(hit(&f, &i));
    }

    /// **맡길 때 쓴 문자열 그대로** 찾을 수 있다. `-a "이름 (메일)"` 은 맡기는
    /// 쪽의 모양이고, 그것을 그대로 필터에 넣는 것이 사람이 실제로 하는 일이다.
    #[test]
    fn assignee_takes_the_same_string_that_assigns() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("chulsu@example.com".into());
        let f = Filter::build(Raw {
            assignee: s(&["철수 (chulsu@example.com)"]),
            all: true,
            ..Raw::default()
        })
        .unwrap();
        assert!(hit(&f, &i));

        // 이름을 바꾼 사람도 옛 줄에서 사라지지 않는다 — 메일 한쪽만 맞아도 된다.
        let f = Filter::build(Raw {
            assignee: s(&["레이븐 (chulsu@example.com)"]),
            all: true,
            ..Raw::default()
        })
        .unwrap();
        assert!(hit(&f, &i));
    }

    /// 메일은 대소문자를 가리지 않는다.
    #[test]
    fn assignee_email_ignores_case() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("Chulsu@Example.com".into());
        let f = Filter::build(Raw {
            assignee: s(&["chulsu@example.com"]),
            all: true,
            ..Raw::default()
        })
        .unwrap();
        assert!(hit(&f, &i));
    }

    /// 메일 없이 이름만으로 맡긴 줄도 이름으로 찾힌다 (`split_assignee` 가 그렇게 둔다).
    #[test]
    fn assignee_without_email_still_matches_by_name() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        let f = Filter::build(Raw { assignee: s(&["철수"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));
    }

    /// -a none 은 담당 없는 것만 고른다
    #[test]
    fn assignee_none_selects_unassigned() {
        let mut assigned = issue("a-0001", "todo", &[]);
        assigned.assignee = Some("철수".into());
        let unassigned = issue("a-0002", "todo", &[]);
        let f = Filter::build(Raw { assignee: s(&["none"]), all: true, ..Raw::default() }).unwrap();
        assert!(!hit(&f, &assigned));
        assert!(hit(&f, &unassigned));
    }

    /// 같은 사람이 둘일 수는 없다 — 다른 필터와 같은 낱말로 거절한다.
    #[test]
    fn repeating_assignee_is_a_friendly_error() {
        let e = Filter::build(Raw { assignee: s(&["철수", "영희"]), all: true, ..Raw::default() }).unwrap_err();
        assert!(e.contains("동시에") && e.contains("-a 철수,영희"), "{e}");
    }

    #[test]
    fn sorting_puts_the_urgent_first_then_id() {
        let mut v = vec![
            issue("a-0003", "todo", &[]),
            issue("a-0001", "todo", &[]),
            issue("a-0002", "todo", &[]),
        ];
        v[0].priority = Some(0);
        sort_for_display(&mut v);
        let ids: Vec<&str> = v.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["a-0003", "a-0001", "a-0002"]);
    }
    /// **담아 둔 생각은 기본 목록에서 빠지고, 글로는 찾아진다.** 규칙이
    /// 여기 한 곳에 있어야 화면과 CLI 가 같은 것을 센다 — 이 시험이 그 자리를
    /// 지킨다.
    #[test]
    fn an_idea_hides_until_it_is_asked_for() {
        let mut thought = issue("argos-0001", "todo", &[]);
        thought.kind = Kind::Idea;
        thought.title = "파서를 다시 쓴다".into();
        let all = vec![thought.clone(), issue("argos-0009", "todo", &[])];
        let wh = Where::of(&all);
        let hits = |raw: Raw| -> Vec<String> {
            let f = Filter::build(raw).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.clone()).collect()
        };

        assert_eq!(hits(Raw::default()), ["argos-0009"], "기본 목록에 idea 가 섞였다");
        assert_eq!(
            hits(Raw { kind: Some(Kind::Idea), ..Raw::default() }),
            ["argos-0001"],
            "콕 집어 물었는데 안 나온다"
        );
        assert_eq!(
            hits(Raw { grep: Some("파서".into()), ..Raw::default() }),
            ["argos-0001"],
            "적어 둔 생각을 글로 못 찾는다 — 그러면 같은 것을 또 적는다"
        );
        assert_eq!(
            hits(Raw { ideas: true, ..Raw::default() }),
            ["argos-0001", "argos-0009"],
            "탐색기가 켜고 들어오는 축이 안 듣는다"
        );
    }
}
