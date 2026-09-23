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
use std::collections::{BTreeMap, BTreeSet};

/// `epic=none` 처럼 "값이 없는 것" 을 고르는 자리.
#[derive(Debug, Clone, PartialEq)]
pub enum Sel {
    Unset,
    Is(String),
}

/// 소속은 **묶음 전체를 봐야** 알 수 있다 — 자식은 조상에게서 물려받고,
/// 마일스톤은 에픽을 거쳐 온다. 그래서 이슈 하나만 보고는 못 고른다.
#[derive(Default)]
pub struct Where<'a> {
    pub epic: crate::report::Handing<'a>,
    /// 줄을 id 로 찾는 지도와 뿌리로 올라간 생각 (`report::Lines`). 마일스톤을 **줄마다**
    /// 묻는 재료다 — 위의 `milestone` 지도는 id 로 짠 것이라 같은 id 의 앞줄이 뒷줄의
    /// 릴리스를 입는다(moai-jk2u.wvn).
    ///
    /// **빈 것은 빈 저장소를 뜻한다**(`Where::default`, 그림 시험이 쓴다) — 조상이 하나도
    /// 없으니 에픽 없는 줄은 제 `milestone` 에 선다. 옛 빈 지도가 `없음` 을 답하던 것과
    /// 다른데, 그쪽이 사실이 아니었다: 줄 하나짜리 저장소에서 `milestones_in` 이 내는 답이
    /// 이것이다.
    pub(crate) lines: crate::report::Lines<'a>,
    /// 물려받은 것까지 친 미룸 (`report::Shelved`). 미룸도 소속처럼 묶음을 타고 내려온다.
    ///
    /// **줄로 묻는다** — 상세가 그 답을 그리는 자와 한 그릇이라, 한 화면이 제 말을 뒤집지
    /// 않는다(moai-wre3). 한때 여기만 접은 지도를 짚고 상세만 줄로 물어, 상세가 `미룸` 을
    /// 다는 묶음 줄을 `--deferred` 가 안 냈다.
    pub shelved: crate::report::Shelved<'a>,
    /// 묶음 → 멤버에서 읽은 칸. 묶음의 칸도 제 줄만 보고는 모른다.
    pub states: BTreeMap<&'a str, &'a str>,
    /// 묶음 → 그 칸의 셈이 마지막으로 움직인 때 (`report::Stand::since`).
    pub since: BTreeMap<&'a str, &'a str>,
    /// id → 그 id 를 마지막으로 든 줄의 종류 (`report::kinds`). 종류가 다른 쌍둥이에게 id 가
    /// 가려진 줄을 가르는 지도다 — 위의 소속 지도는 id 로 짠 것이라 그 줄에는 쌍둥이의 값이
    /// 나온다. 비었으면(`Where::default`) 가려진 줄이 없는 것으로 친다.
    ///
    /// **판정이 아니라 지도를 든다**(moai-53s2). 한때 판정 하나를 상자에 담아 들었는데,
    /// 줄을 내는 쪽(`cmd::Row::of` → `report::stands_in`)도 같은 지도가 있어야 가려진 줄에
    /// 쌍둥이의 에픽을 안 단다 — 닫힌 상자에서는 그 지도를 못 꺼낸다. **든 꼴 그대로**
    /// 나르므로(`report::Kinds`) 꼴이 다른 쪽이 걸음마다 지도를 새로 짓지 않는다(리뷰).
    pub(crate) kinds: crate::report::Kinds<'a>,
    /// 길 잃은 줄 **밑에 접힌** 줄 (`report::under_lost`). 트리가 `(길 잃음)` 안에 그리고
    /// status 가 `no_epic`·`no_milestone` 으로 안 세는 그 집합이다 — `none` 거름이 같은 자로
    /// 고르게 한다(moai-phw9).
    pub(crate) folded: BTreeSet<&'a str>,
}

impl<'a> Where<'a> {
    /// **한 걸음으로 잰다**([`crate::report::Soil`]) — 소속·미룸·묶음 칸·가려짐·길 잃음은 서로가
    /// 서로의 재료라, 따로 부르면 `groups` 만 서너 번 돈다. 이미 잰 것을 든 쪽(탐색기의 `tui::Ground`)은
    /// 이것을 안 부르고 제 지도를 빌려 **필드 이름으로** 짓는다(moai-fbdg) — 같은 타입의 지도가 넷이라
    /// 차례로 넘기면 `states` 와 `since` 가 바뀌어도 컴파일된다.
    // 바이너리는 지도를 든 [`Where::from_soil`] 을 부른다(moai-g0zx) — 이 꼴은 시험의 짧은 길이다.
    #[cfg(test)]
    pub fn of(all: &'a [Issue], cfg: &'a crate::config::Config) -> Where<'a> {
        Where::from_soil(all, cfg, crate::report::Soil::of(all))
    }

    /// [`Where::of`] 와 같은 것. **이미 잰 지도를 받는다** — `moai show --tree` 는 같은 명령 안에서
    /// 색인(`nav::Index`)과 에픽 굴림도 지으므로, 저마다 재면 `groups` 가 세 벌 돈다(moai-g0zx).
    ///
    /// **지도를 통째로 받는다**(`&Soil` 이 아니다) — 거름망은 그 지도를 제 필드로 들고 사는데,
    /// 빌려 받으면 그 지도가 사는 동안 거름망도 거기 매인다. 받은 쪽이 옮겨 담는 것은 한 번이고,
    /// 부르는 쪽은 색인처럼 지도를 먼저 쓰는 것을 다 쓴 뒤에 이것을 짓는다.
    pub fn from_soil(all: &'a [Issue], cfg: &'a crate::config::Config, soil: crate::report::Soil<'a>) -> Where<'a> {
        let stands = soil.stands(all, cfg);
        let states = stands.iter().map(|(id, s)| (*id, s.column)).collect();
        let since = stands.into_iter().map(|(id, s)| (id, s.since)).collect();
        let crate::report::Soil { epic, roots, shelved, kinds, folded, lines, .. } = soil;
        // **짓기 전에 빈지 본다**(리뷰 moai-jk2u.hr4). 줄마다 갈리는 id 는 같은 id 가 두 줄일
        // 때만 서는데, `kinds` 는 id 마다 한 칸이라 그 수가 줄 수와 같으면 id 가 다 다르다 —
        // 성한 저장소에서 거름망을 짓는 걸음마다 목록을 두 번 더 걷던 자리다.
        let split = match kinds.len() == all.len() {
            true => BTreeSet::new(),
            false => crate::report::split_roots(all, &shelved),
        };
        // 갈리는 id 의 줄만 넘긴다 — 성한 저장소에서는 빈 목록이라 걷는 값도 드는 자리도 없다.
        let rows: Vec<(&Issue, Option<&str>)> =
            all.iter().zip(&shelved).filter(|(i, _)| split.contains(i.id.as_str())).map(|(i, r)| (i, *r)).collect();
        let shelved = crate::report::Shelved::kept(roots, split, rows);
        let kinds = crate::report::Kinds::Own(kinds);
        Where { epic, lines, shelved, states, since, kinds, folded }
    }

    /// 그 줄이 종류가 다른 쌍둥이에게 id 가 가려졌는가 (`report::is_eclipsed`).
    pub fn eclipsed(&self, i: &Issue) -> bool {
        self.kinds.eclipses(i)
    }

    /// 그 줄이 서 있는 칸 (`report::column`).
    pub fn column<'x>(&'x self, i: &'x Issue) -> &'x str {
        crate::report::column(&self.kinds, i, &self.states)
    }

    /// 그 줄이 **든 에픽** (`report::stands_in`) — `--json` 의 `derived_epic` 과 같은 답이다.
    pub fn epic_of<'x>(&'x self, i: &'x Issue) -> Option<&'x str> {
        crate::report::stands_in(&self.kinds, i, self.epic.handed().get(i.id.as_str()).copied())
    }

    /// 그 줄이 **선 마일스톤** (`report::stood_at_line`) — 롤업과 `moai show <마일스톤>` 의
    /// 멤버가 세는 곳과 같은 답이다.
    ///
    /// **지도를 곧바로 안 짚는다**(리뷰). 마일스톤은 에픽을 타고 오는데([`crate::report::milestones_in`])
    /// 그 에픽이 줄마다 갈리므로, 지도를 짚으면 같은 id 를 든 앞줄이 뒷줄의 에픽을 타고 남의
    /// 릴리스로 간다 — `moai show <마일스톤>` 이 멤버로 그린 줄을 `moai show --milestone <그것>`
    /// 은 안 내고 `--milestone none` 이 냈다.
    /// **가려짐은 여기서 가른다** — 에픽 쪽 [`Where::epic_of`] 가 `report::stands_in` 안에서
    /// 그러는 것과 짝이다. `report::stood_at_line` 자신은 가려짐을 안 본다(세는 쪽은 `work_under`
    /// 가 그 줄을 미리 걸러 넘긴다) — 문을 한 겹 위에 두는 것이 두 축에서 같은 꼴이다.
    pub fn milestone_of<'x>(&'x self, i: &'x Issue) -> Option<&'x str> {
        if self.eclipsed(i) {
            return None;
        }
        crate::report::stood_at_line(i, self.epic_of(i), &self.lines)
    }

    /// 그 줄이 **지금 칸에 들어선 때** — `--stale` 이 재는 시각.
    ///
    /// 묶음이면 읽은 칸의 셈이 마지막으로 움직인 때다. 적힌 `status_since` 는 아무
    /// 데서도 안 읽히는 칸의 시각이라, 그것으로 재면 오늘 진행 중이 된 에픽이
    /// `-s in_progress --stale 10` 에 걸린다 — 고르는 자와 재는 자가 어긋난다.
    pub fn since<'x>(&'x self, i: &'x Issue) -> &'x str {
        // **고르는 자와 재는 자가 한 문을 지난다**(리뷰, `report::stands_on`). 위의 `column` 만
        // 가려진 줄을 거르면, `-s` 가 제 칸으로 고른 그 줄의 나이는 쌍둥이 묶음의 셈에서 와
        // `--stale` 이 265일 된 줄을 하루짜리로 잰다.
        crate::report::stands_on(&self.kinds, i, || self.since.get(i.id.as_str()).copied())
            .unwrap_or(i.status_since.as_str())
    }

    /// 목록에서 미룬 것으로 치는가. **제 줄의 미룸이나 물려받은 미룸.**
    ///
    /// 끝난 줄의 제 미룸도 여기 든다 — 그 줄은 done 규칙이 따로 숨기고,
    /// `--all` 은 그것을 `미룸` 표와 함께 연다. 물려받은 것만 보면 닫고 미룬
    /// 줄이 `--all` 에서 표를 잃는다.
    ///
    /// **물려받은 것은 줄마다 묻는다**(moai-u3ta). 접은 지도는 미룬 릴리스에 든 앞줄의 미룸을
    /// 산 릴리스에 든 뒷줄에 그대로 주었다 — `show --deferred` 가 두 줄을 다 내고 `ready` 는
    /// 둘 다 안 내주면서, `show <산 릴리스>` 는 그 줄을 산 멤버로 셌다. 되묻는 것은 답이
    /// 줄마다 갈리는 id 뿐이고(`report::Shelved`), 성한 저장소에서는 접은 지도 한 번 짚기다.
    ///
    /// **묶음 줄도 같은 자로 묻는다**(2026-09-23 사용자 결정, moai-wre3). 한때 여기만
    /// `!is_group` 문으로 접은 지도에 갔는데, 그리는 쪽이 줄로 답하게 된 뒤로는 그 문이 곧
    /// 상세와 이 목록이 갈리는 자리였다 — 묶음의 읽은 칸을 가르는 `report::counted` 도 이미
    /// 그 줄에서 올라가므로(`shelf.every(g)`), 줄마다의 판정이 묶음에도 참이다.
    pub fn deferred(&self, i: &Issue) -> bool {
        i.is_deferred() || self.shelved.root(i).is_some()
    }
}

/// 글로 찾을 때 **어디를 보는가**(moai-kojj). TUI 검색 칸의 Tab 이 이 차례로 돈다.
///
/// `All` 은 넷을 다 본다 — CLI 의 `-g` 도 이것이다. 한때 제목·본문만 봤는데, 그러면 id
/// 조각이나 태그로 찾은 것이 `All` 에서는 안 걸리고 좁힌 범위에서만 걸린다. 좁힌 것이
/// 넓은 것보다 더 찾으면 "전체" 라는 이름이 거짓말이 된다.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GrepIn {
    #[default]
    All,
    Id,
    Title,
    Tag,
    Body,
}

impl GrepIn {
    const ORDER: [GrepIn; 5] = [GrepIn::All, GrepIn::Id, GrepIn::Title, GrepIn::Tag, GrepIn::Body];

    /// 화면에 적는 이름. 거름망 뱃지의 `/id:…` 앞머리이기도 하다.
    pub fn name(self) -> &'static str {
        match self {
            GrepIn::All => "전체",
            GrepIn::Id => "id",
            GrepIn::Title => "제목",
            GrepIn::Tag => "태그",
            GrepIn::Body => "본문",
        }
    }

    /// Tab 의 다음 범위. 끝에서 처음으로 돈다.
    pub fn next(self) -> GrepIn {
        let at = Self::ORDER.iter().position(|g| *g == self).unwrap_or(0);
        Self::ORDER[(at + 1) % Self::ORDER.len()]
    }

    /// Shift-Tab 의 앞 범위.
    pub fn prev(self) -> GrepIn {
        let at = Self::ORDER.iter().position(|g| *g == self).unwrap_or(0);
        Self::ORDER[(at + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }

    /// id·제목을 이 범위가 보는가 — 목록 줄에서 찾은 글자를 칠할 자리를 가른다.
    pub fn sees_id(self) -> bool {
        matches!(self, GrepIn::All | GrepIn::Id)
    }

    pub fn sees_title(self) -> bool {
        matches!(self, GrepIn::All | GrepIn::Title)
    }

    /// 태그·본문을 이 범위가 보는가 — 상세 칸에서 찾은 글자를 칠할 자리를 가른다(moai-lw7i).
    pub fn sees_tag(self) -> bool {
        matches!(self, GrepIn::All | GrepIn::Tag)
    }

    pub fn sees_body(self) -> bool {
        matches!(self, GrepIn::All | GrepIn::Body)
    }

    /// `q` 는 이미 소문자다([`Filter::build`]).
    ///
    /// **어느 자리를 보는지는 `sees_*` 가 정한다**(moai-xemz 리뷰) — 탐색기가 찾은 글자를 칠할 자리도 그
    /// 넷으로 가르므로, 여기서 범위를 따로 적으면 범위 하나를 고친 날 걸린 줄에 칠이 빠지거나 안 걸린
    /// 자리에 선다.
    pub fn hits(self, i: &Issue, q: &str) -> bool {
        let has = |s: &str| s.to_lowercase().contains(q);
        (self.sees_id() && has(&i.id))
            || (self.sees_title() && has(&i.title))
            || (self.sees_tag() && i.tags.iter().any(|t| has(t)))
            || (self.sees_body() && i.body.as_deref().is_some_and(has))
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
    /// `grep` 이 어디를 보는가. CLI 는 늘 [`GrepIn::All`] 이고, TUI 의 검색 칸이 Tab 으로 돌린다.
    pub grep_in: GrepIn,
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
pub const KEYS: &[&str] =
    &["status", "tag", "no-tag", "epic", "milestone", "parent", "priority", "assignee", "type", "grep", "stale"];

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
    pub grep_in: GrepIn,
    pub stale: Option<i64>,
    pub all: bool,
    pub ideas: bool,
    pub deferred: bool,
    pub filter: Vec<String>,
}

/// 기본 목록이 줄을 숨긴 까닭. **그 줄을 여는 한 낱말로 가른다.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hide {
    /// `--all` 이 연다.
    Done,
    /// `--type idea` 가 연다.
    Idea,
    /// `--deferred` 가 연다.
    Deferred,
    /// 어느 한 낱말로도 안 열린다 (닫아 둔 생각). 세지 않는다.
    Unopenable,
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
        let ideas = raw.ideas || raw.kind == Some(Kind::Idea) || raw.grep.is_some() || raw.deferred;
        // **`--deferred` 는 그것만 본다.** 목록 자리에서 미룬 것은 done 처럼
        // 기본으로 빠지므로, 켜는 말과 좁히는 말이 하나여야 "미룬 것 보기" 가
        // 한 낱말로 끝난다.
        let deferred = raw.deferred.then_some(true);
        Ok(Filter {
            status: once(&raw.status, "-s", "상태")?,
            tags: raw.tag.iter().map(|t| split_tags(t)).filter(|v: &Vec<String>| !v.is_empty()).collect(),
            no_tags: raw.no_tag.iter().flat_map(|t| split_tags(t)).collect(),
            epic: sel(once(&raw.epic, "-e", "에픽")?),
            milestone: sel(once(&raw.milestone, "--milestone", "마일스톤")?),
            parent: sel(once(&raw.parent, "--parent", "부모")?),
            priority: parse_priorities(&once(&raw.priority, "-p", "우선순위")?)?,
            assignee: sel(once(&raw.assignee, "-a", "담당")?),
            kind: raw.kind,
            // 한 번만 내려 두면 이슈마다 다시 만들 일이 없다.
            grep: raw.grep.map(|q| q.to_lowercase()),
            grep_in: raw.grep_in,
            stale: raw.stale,
            all: raw.all,
            ideas,
            deferred,
        })
    }

    /// 기본 목록이 이 줄을 숨기는가, 숨긴다면 **어느 한 낱말이 그것을 여는가.**
    ///
    /// **숨김 규칙은 여기 하나다.** `matches` 도 이것으로 거르고, 숨긴 수를
    /// 세는 쪽(`moai show` 의 꼬리)도 이것을 받아 세기만 한다. 한때 `show` 가
    /// 같은 세 규칙을 다시 적었고, 두 벌이라 실제로 갈라졌다(moai-nnul).
    ///
    /// - **담아 둔 생각은 기본 목록에서 빠진다.** 이 자리는 일을 보는 자리고,
    ///   생각 조각이 섞이면 목록이 흐려져 담기가 꺼려진다. 콕 집어 묻거나
    ///   (`--type idea`) 글로 찾을 때는 나온다 — 이미 적어 둔 생각을 다시 안
    ///   적으려면 찾아져야 한다. 탐색기는 `ideas` 를 켜고 들어와 시키지도 않은
    ///   줄을 숨기지 않는다.
    /// - **미뤄 둔 것은 done 과 같은 자리에서 빠진다.** 지금 계획이 아니라는
    ///   뜻이 같고, 켜는 말(`--all`)도 같아야 축이 안 는다. 물려받은 미룸도
    ///   미룸이다 — 미룬 에픽의 멤버가 목록에 남으면 `ready` 와 보드가 빼 둔
    ///   것을 목록만 계획으로 낸다.
    ///
    /// 까닭은 **그 줄을 실제로 여는 한 낱말**로 가른다. 첫 까닭으로 가르면
    /// 닫아 둔 생각이 `idea N건 숨김 — --type idea` 로 서는데 그 명령은 done 을
    /// 여전히 숨겨 아무것도 안 낸다.
    ///   `--type idea` 는 idea 만 연다 (done·미룸은 그대로 숨긴다)
    ///   `--deferred`  는 미룸을 열고 생각까지 같이 연다 (done 은 아니다)
    ///   `--all`       은 done 과 미룸을 연다 (생각은 아니다)
    pub fn hidden_by(&self, i: &Issue, wh: &Where) -> Option<Hide> {
        let idea = crate::report::is_idea(i) && !self.ideas;
        let deferred = self.deferred.is_none() && !self.all && wh.deferred(i);
        // 묶음은 **읽은 칸**으로 닫혔는지 본다 — 멤버가 남은 에픽을 손으로
        // `done` 에 뒀다고 목록에서 숨기면, 진행 중인 묶음이 사라진다.
        let done = !self.all && self.status.is_empty() && wh.column(i) == crate::config::DONE;
        match (idea, deferred, done) {
            (false, false, false) => None,
            (true, false, false) => Some(Hide::Idea),
            (_, true, false) => Some(Hide::Deferred),
            (false, _, true) => Some(Hide::Done),
            _ => Some(Hide::Unopenable),
        }
    }

    pub fn matches(&self, i: &Issue, now: &str, wh: &Where) -> bool {
        // `--deferred` 는 **좁히는 말**이기도 하다 — 미룬 것만 본다. 숨김이
        // 아니라 고르기라 `hidden_by` 에 넣지 않는다.
        if self.deferred.is_some_and(|want| wh.deferred(i) != want) {
            return false;
        }
        if self.hidden_by(i, wh).is_some() {
            return false;
        }
        // `-s todo` 는 **서 있는 칸**으로 고른다. 멤버가 집힌 에픽을 손으로 둔
        // 칸으로 고르면 "할 일" 에 진행 중인 묶음이 섞인다(moai-j3b3).
        if !self.status.is_empty() && !self.status.iter().any(|s| s == wh.column(i)) {
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
        //
        // **가려진 줄은 어느 소속으로도 안 고른다** — `-e none` 도. 지도의 값은 종류가
        // 다른 쌍둥이의 것이고, 트리는 그 줄을 `(길 잃음)` 에 두며 롤업은 어느 묶음에도
        // 안 센다(moai-2m9p). 여기서 지도를 그대로 읽으면 `moai show <에픽>` 이 `0/0` 이라
        // 말하는 에픽을 `moai show -e <에픽>` 은 그 줄로 채운다.
        let eclipsed = wh.eclipsed(i);
        // **길 잃은 줄 밑에 접힌 줄은 `none` 으로 안 고른다**(moai-phw9, 사용자와 정함). 소속
        // 지도에 없다는 사실만 보면 트리가 `(길 잃음)` 안에 그리고 status 가 안 세는 줄을
        // "없는 것" 으로 고른다. 고칠 곳은 부모의 끊긴 참조라 `-e none` 으로 찾을 줄이 아니다.
        // 이름으로 고르는 `-e X` 는 그대로다.
        let folded = wh.folded.contains(i.id.as_str());
        let placed = |sel: &[Sel], value: Option<&str>| {
            sel.is_empty()
                || (!eclipsed
                    && sel
                        .iter()
                        .any(|s| !(folded && matches!(s, Sel::Unset)) && matches_sel(std::slice::from_ref(s), value)))
        };
        // **두 축 다 지도를 곧바로 안 짚는다**(moai-7iyc.rt6, 2026-09-23 사용자 결정). 지도는 id 로
        // 짠 것이라 같은 id 를 든 줄이 둘이면 앞줄이 뒷줄의 값을 입는다 — `-e <에픽>` 이
        // `derived_epic` 과 다른 답을 하던 자리다. **마일스톤도 같다**(리뷰): 에픽이 마일스톤을
        // 이기므로(`report::milestones_in`) 줄에 적힌 `milestone` 은 이미 졌지만, 이긴 그 에픽이
        // 줄마다 갈려 지도의 값도 앞줄의 것이 못 된다.
        // **마일스톤은 물을 때만 잰다**(리뷰 moai-jk2u.m60) — `placed` 는 빈 거르개에 참을 내지만
        // 인자는 그 앞에 셈해진다. 값이 지도 짚기이던 때는 공짜였는데, 줄마다 묻게 된 뒤로
        // (`report::stood_at_line`) 에픽 없는 줄마다 조상을 타고 오르는 걸음이라 그렇지 않다 —
        // 탐색기의 거름망은 키 하나에 줄마다 한 번 여기를 지난다.
        if !placed(&self.epic, wh.epic_of(i)) {
            return false;
        }
        if !self.milestone.is_empty() && !placed(&self.milestone, wh.milestone_of(i)) {
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
        if let Some(q) = &self.grep
            && !self.grep_in.hits(i, q)
        {
            return false;
        }
        // 머문 기간도 **서 있는 칸**의 것이다 (`Where::since`). `-s` 는 읽은 칸으로
        // 고르는데 나이만 적힌 칸의 시각으로 재면, 한 물음의 두 조각이 다른 칸을 본다.
        if let Some(d) = self.stale
            && days_since(wh.since(i), now).is_none_or(|n| n < d)
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
                return Err(format!("`{k}` 라는 필터 항목이 없다.\n      있는 것: {}", KEYS.join(", ")));
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
    values.into_iter().map(|v| if v == "none" { Sel::Unset } else { Sel::Is(v) }).collect()
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

/// 사람이 고르는 차례(moai-55cp). 기본은 [`display_order`] 다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Priority,
    Created,
    Updated,
    Status,
    Assignee,
    Title,
}

/// 고른 차례로 두 줄을 견준다. 줄마다 **칸을 곁에 받는다** — 묶음의 칸은 멤버에서 읽은
/// 것이라 `Issue::status` 만 보면 목록의 글리프와 차례가 다른 칸을 본다. `statuses` 는 설정의
/// 칸 차례다(칸 순서는 설정이 정한다). 설정에 없는 칸은 뒤로 간다.
///
/// - 제 방향은 사람이 먼저 보고 싶은 쪽이다 — 우선순위는 급한 것, 생성·수정은 **새것**,
///   칸은 설정의 앞 칸, 담당·제목은 가나다. 담당 없는 줄은 뒤로 간다
/// - 담당은 **화면에 선 이름**(`model::label`, `naming`)으로 견준다 — 이름만 견주면 `naming = "email"`
///   에서 담당 열이 가나다로 안 선다(moai-2kyl 단계 리뷰)
/// - 같으면 [`display_order`] 로 가른다 — 차례가 흔들리지 않는다
/// - `reversed` 는 가른 것까지 통째로 뒤집는다
pub fn order_by(
    key: SortKey,
    reversed: bool,
    a: (&Issue, &str),
    b: (&Issue, &str),
    statuses: &[String],
    naming: crate::config::Naming,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let rank = |column: &str| statuses.iter().position(|s| s == column).unwrap_or(statuses.len());
    let shown = |i: &Issue, name: &str| crate::model::label(name, i.assignee_email.as_deref(), naming);
    let natural = match key {
        SortKey::Priority => Ordering::Equal,
        SortKey::Created => b.0.created_at.cmp(&a.0.created_at),
        SortKey::Updated => b.0.updated_at.cmp(&a.0.updated_at),
        SortKey::Status => rank(a.1).cmp(&rank(b.1)),
        SortKey::Assignee => match (&a.0.assignee, &b.0.assignee) {
            (Some(x), Some(y)) => caseless(&shown(a.0, x), &shown(b.0, y)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        SortKey::Title => caseless(&a.0.title, &b.0.title),
    };
    let order = natural.then_with(|| display_order(a.0, b.0));
    if reversed { order.reverse() } else { order }
}

/// **안 읽은 줄** — 내게 온 것 가운데 내가 마지막으로 본 뒤에 바뀐 것(moai-50mn).
///
/// `seen` 은 이슈 id → 마지막으로 본 줄의 `updated_at`(내 설정의 `[read]` — 옛 바이너리는 본 때를 적었다,
/// moai-lyc1). 없는 id 는 **한 번도 안 본 것**이라
/// 안 읽음이다. 바뀐 때는 스냅샷의 `updated_at` 으로 잰다(사용자 결정 2026-09-15) — 저널의 노트는
/// 안 센다: 그것을 세려면 저널을 상태 계산에 읽어야 하고, `note` 는 스냅샷을 안 바꾼다.
///
/// **내게 온 것**은 담당이 나인 줄과 그 **밑**이다(사용자 결정) — 자식(`부모.자식`)과 그 에픽의
/// 멤버. 내 에픽에 남이 달아 둔 리뷰를 놓치지 않는다. 담당을 가르는 자는 거름망과 같은
/// [`Sel::Is`] 하나라 `이름 (메일)`·이름만·메일만이 다 통한다.
pub fn unread<'a>(issues: &'a [Issue], me: &str, seen: &BTreeMap<String, String>) -> BTreeSet<&'a str> {
    let want = Sel::Is(me.to_string());
    let mine: BTreeSet<String> = issues.iter().filter(|i| is_assignee(&want, i)).map(|i| i.id.clone()).collect();
    if mine.is_empty() {
        return BTreeSet::new();
    }
    // **소속은 `report::groups` 에 묻는다**(moai-50mn.mgo). 줄마다 `epic` 필드를 손으로 훑으면
    // 소속을 재는 자가 둘이 된다 — `epic_from_parent` 가 적어 둔 그대로다: *소속을 따로 재면 둘은
    // 언젠가 어긋난다.* 이 지도는 `Where::of` 도 같은 자에게 묻는다.
    //
    // **걸음도 따로 두지 않는다**(moai-j038.vna) — 제가·조상이 내 것이거나 저나 조상의 에픽이 내 것인가는
    // 워크트리의 일을 가르는 [`crate::report::claims`] 와 같은 물음이라 그것을 부른다. 손으로 옮겨 둔
    // 걸음은 한쪽만 고쳐지는 날 훅이 세는 "그 일" 과 [NEW] 가 서는 "내게 온 것" 을 갈라놓는다.
    // **마일스톤은 안 센다**(사용자 결정: 담당·조상·에픽) — 에픽 축만 잰 재료로 든다
    // (`Ties::epics_only`). 한때 빈 마일스톤 지도로 같은 뜻을 졌는데, 그 축의 답은 이제
    // 지도가 아니라 줄에서 나오므로 빈 지도가 "안 센다" 를 못 뜻한다(moai-jk2u.ipf).
    let ties = crate::report::Ties::epics_only(issues);
    issues
        .iter()
        .filter(|i| crate::report::claims(&ties, &mine, i))
        .filter(|i| changed_since_seen(i, seen))
        .map(|i| i.id.as_str())
        .collect()
}

/// 그 줄이 **내가 마지막으로 본 뒤에 바뀌었나** — 한 번도 안 봤거나(`seen` 에 없다) 적힌 값보다 늦게
/// 고쳐졌다. 때는 스냅샷의 `updated_at` 이다(사용자 결정 2026-09-15). 안 읽음([`unread`])도, 읽음을 적을
/// 때 이미 읽은 줄을 거르는 것(`moai read`·탐색기의 `r`)도 이 하나로 잰다(moai-j038.vna).
///
/// **견주는 식은 `>` 그대로다**(사용자 결정 2026-09-19, moai-lyc1). 적힌 값은 이제 본 줄의 `updated_at`
/// 이지만([`read_marks_of`]) 옛 바이너리가 적은 값(본 때)은 모양으로 못 가른다 — `!=` 로 견주면 옛 값이
/// 도장과 거의 다 달라 업그레이드 뒤 읽은 줄 전부가 한 번에 [NEW] 로 선다. `>` 는 옛 값을 옛 뜻대로
/// 읽고, 다시 읽는 줄부터 새 값으로 바뀐다. 대가로 적힌 것보다 **이른** 도장으로 바뀐 줄(시계가 뒤진
/// 기계, 옛 도장을 들고 온 머지)과 같은 초 안의 고침은 놓친다.
pub fn changed_since_seen(i: &Issue, seen: &BTreeMap<String, String>) -> bool {
    seen.get(&i.id).is_none_or(|when| i.updated_at.as_str() > when.as_str())
}

/// 읽음으로 적을 값 — 이 가운데 **본 뒤로 바뀐 줄**([`changed_since_seen`])마다 id → **그 줄의
/// `updated_at`**(사용자 결정 2026-09-19, moai-lyc1). `moai read` 와 탐색기의 `r`·`SPC m` 이 이 하나로 적는다.
///
/// 옛 값은 이 기계의 시계로 잰 "본 때" 였다. 줄의 도장은 그 줄을 쓴 기계·가지가 찍은 것이라, 워크트리
/// 가지에서 01:30 에 고치고 03:00 에 develop 에 머지한 줄을 02:00 에 읽었으면 [NEW] 가 영영 안 섰고
/// (이 저장소의 평소 흐름이다), 시계가 앞선 기계가 쓴 줄은 읽어도 안 내렸다. 본 줄의 도장을 적으면
/// 둘 다 풀린다 — 무엇과 견주는지가 같은 시계에서 온다. 적는 것은 **본 그 줄**의 도장이다 — 탐색기는
/// 제 화면의 줄을 준다. 화면이 낡았으면 새 도장이 적힌 것보다 늦어 다시 [NEW] 가 선다.
///
/// **같은 id 의 줄은 다 받아 가장 늦은 도장을 적는다**(moai-7c50.exy). [`unread`] 는 쌍둥이 가운데
/// 하나만 바뀌어도 그 id 를 세우는데, 뒷줄 하나의 도장만 적으면 앞줄이 늘 더 늦어 [NEW] 가 영영 안
/// 내리고 `moai read --all` 은 "적을 것이 없다" 만 되뇐다. 본 때를 적던 때는 그 때가 둘 다를 덮었다.
/// 부르는 쪽은 id 로 거르지 말고 그 id 의 줄을 전부 넘긴다.
pub fn read_marks_of<'a>(
    lines: impl IntoIterator<Item = &'a Issue>,
    seen: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut marks: BTreeMap<String, String> = BTreeMap::new();
    for i in lines.into_iter().filter(|i| changed_since_seen(i, seen)) {
        match marks.get_mut(&i.id) {
            Some(at) if *at >= i.updated_at => {}
            Some(at) => at.clone_from(&i.updated_at),
            None => {
                marks.insert(i.id.clone(), i.updated_at.clone());
            }
        }
    }
    marks
}

/// 대소문자를 접어 견준다 — **견줄 때마다 소문자 문자열을 짓지 않는다**(moai-zrzo). 정렬은 줄 수 × log
/// 번 견주고 목록은 키마다 센다. 차례는 `to_lowercase()` 로 지어 견준 것과 **한 치도 안 갈린다**(moai-y61p
/// 단계 리뷰):
/// - **바이트가 같은 머리는 건너뛴다** — 접어도 같다. 담당은 대개 한 사람이고 제목도 머리가 같은 것이 흔한데,
///   같은 글을 끝까지 글자마다 접어 걷던 것이 옛 식(한 번에 접는 ASCII 길)보다 느렸다
/// - **둘 다 ASCII 면 바이트로 접는다** — `Update…`·`update…` 처럼 머리에서 대소문자만 갈리면 건너뛸 머리가 없다
/// - **Σ 가 든 글은 옛 식대로 지어 견준다.** 문자열의 `to_lowercase` 는 낱말 끝 Σ 를 앞뒤 글자를 보고 ς 로
///   접는데 글자마다 접으면 늘 σ 다 — 같은 글끼리만이 아니라 `ΟΔΟΣ ΑΛΦΑ`·`οδος βητα` 처럼 다른 글의 차례도
///   뒤집혔다. 앞뒤를 보고 접히는 글자는 이 하나뿐이다. 머리를 건너뛰기 **전에** 본다 — 앞 글자가 머리에 있다
/// - 그 밖에는 자른 자리를 글자 머리로 물린다(두 글에서 같은 자리다) — 바이트가 같으면 글자 경계도 같다
///
/// 이름이 `folded` 가 아닌 것은 이 모듈에서 그 낱말이 이미 "길 잃은 줄 밑에 접힌 줄"(`Where::folded`)이라서다.
fn caseless(a: &str, b: &str) -> std::cmp::Ordering {
    let same = |a: &str, b: &str| a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    if a.is_ascii() && b.is_ascii() {
        let n = same(a, b);
        return a.as_bytes()[n..]
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.as_bytes()[n..].iter().map(u8::to_ascii_lowercase));
    }
    if a.contains('Σ') || b.contains('Σ') {
        return a.to_lowercase().cmp(&b.to_lowercase());
    }
    let mut n = same(a, b);
    while !a.is_char_boundary(n) {
        n -= 1;
    }
    a[n..].chars().flat_map(char::to_lowercase).cmp(b[n..].chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    /// **안 읽은 줄은 내게 온 것 가운데 마지막으로 본 뒤에 바뀐 것**(moai-50mn, 사용자 결정) —
    /// 자식과 에픽 멤버까지 세고, 한 번도 안 본 줄은 안 읽음이며, 남의 줄은 세지 않는다.
    #[test]
    fn unread_counts_what_came_to_me_and_changed_since_i_looked() {
        let at = |id: &str, who: Option<&str>, when: &str| {
            let mut i =
                Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new("todo"), "2026-09-01T00:00:00Z");
            i.assignee = who.map(String::from);
            // 사람마다 제 메일 — 하나로 뭉뚱그리면 메일로 찾을 때 남의 줄까지 걸린다.
            i.assignee_email =
                who.map(|w| if w == "레이븐" { "raven@buzzni.com" } else { "narae@buzzni.com" }.to_string());
            i.updated_at = when.to_string();
            i
        };
        let (early, late) = ("2026-09-10T00:00:00Z", "2026-09-14T00:00:00Z");
        let mut epic = at("a-0001", Some("레이븐"), early);
        epic.kind = Kind::Epic;
        let mut member = at("a-0002", None, late);
        member.epic = Some("a-0001".into());
        let issues = vec![
            epic,
            member,                              // 내 에픽의 멤버 — 남이 만들어도 내게 온 것
            at("a-0002.rv", None, late),         // 그 줄의 리뷰(자식)
            at("a-0003", Some("나래"), late),    // 남의 줄
            at("a-0004", Some("레이븐"), early), // 내 줄, 본 뒤로 안 바뀜
        ];
        let seen: BTreeMap<String, String> =
            [("a-0001".to_string(), early.to_string()), ("a-0004".to_string(), early.to_string())].into();
        let ids = |me: &str, seen: &BTreeMap<String, String>| unread(&issues, me, seen).into_iter().collect::<Vec<_>>();
        assert_eq!(ids("레이븐", &seen), ["a-0002", "a-0002.rv"], "자식·에픽 멤버를 안 세거나 남의 줄을 셌다");

        // 한 번도 안 본 줄은 안 읽음이다 — 본 적 없는 에픽이 목록에 든다.
        let none = BTreeMap::new();
        assert_eq!(ids("레이븐", &none), ["a-0001", "a-0002", "a-0002.rv", "a-0004"]);

        // 메일로도 같은 사람이다 — 담당을 가르는 자가 거름망과 하나다.
        assert_eq!(ids("raven@buzzni.com", &none).len(), 4);
        // 담당이 나인 줄이 하나도 없으면 아무것도 안 센다.
        assert!(ids("아무개", &none).is_empty());
    }

    /// **고른 차례는 제 방향이 있고, 같으면 기본 차례로 가르며, 뒤집으면 통째로 뒤집는다**(moai-55cp).
    #[test]
    fn order_by_each_key_and_its_reverse() {
        let at = |id: &str, p: u8, created: &str, who: Option<&str>| {
            let mut i = Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new("todo"), created);
            i.priority = Some(p);
            i.assignee = who.map(String::from);
            i
        };
        let issues = [
            at("a-1", 2, "2026-09-01T00:00:00Z", Some("나래")),
            at("a-2", 1, "2026-09-03T00:00:00Z", None),
            at("a-3", 2, "2026-09-02T00:00:00Z", Some("가람")),
        ];
        let columns = ["review", "todo", "done"];
        let statuses: Vec<String> = ["todo", "review", "done"].map(String::from).to_vec();
        let sorted = |key, reversed| {
            let mut idx = [0, 1, 2];
            idx.sort_by(|&x, &y| {
                order_by(
                    key,
                    reversed,
                    (&issues[x], columns[x]),
                    (&issues[y], columns[y]),
                    &statuses,
                    crate::config::Naming::Full,
                )
            });
            idx.iter().map(|&i| issues[i].id.as_str()).collect::<Vec<_>>()
        };
        assert_eq!(sorted(SortKey::Priority, false), ["a-2", "a-1", "a-3"], "기본 차례와 다르다");
        assert_eq!(sorted(SortKey::Priority, true), ["a-3", "a-1", "a-2"]);
        assert_eq!(sorted(SortKey::Created, false), ["a-2", "a-3", "a-1"], "새것이 위가 아니다");
        assert_eq!(sorted(SortKey::Created, true), ["a-1", "a-3", "a-2"]);
        assert_eq!(sorted(SortKey::Status, false), ["a-2", "a-1", "a-3"], "설정의 칸 차례가 아니다");
        assert_eq!(sorted(SortKey::Assignee, false), ["a-3", "a-1", "a-2"], "담당 없는 줄이 뒤로 안 갔다");

        // **담당은 화면에 선 이름으로 선다**(moai-2kyl 단계 리뷰) — 메일로 대면 메일의 가나다다.
        let mut mailed = [issues[0].clone(), issues[2].clone()];
        mailed[0].assignee_email = Some("abe@example.com".into()); // 나래
        mailed[1].assignee_email = Some("zed@example.com".into()); // 가람
        let by = |naming| {
            let mut idx = [0, 1];
            idx.sort_by(|&x, &y| {
                order_by(SortKey::Assignee, false, (&mailed[x], "todo"), (&mailed[y], "todo"), &statuses, naming)
            });
            idx.map(|i| mailed[i].id.as_str())
        };
        assert_eq!(by(crate::config::Naming::Name), ["a-3", "a-1"]);
        assert_eq!(by(crate::config::Naming::Email), ["a-1", "a-3"], "메일로 선 담당 열이 가나다가 아니다");

        // **제목·담당은 대소문자를 접어 가나다로 선다**(moai-y61p 단계 리뷰). 접지 않고 견주면 `Banana` 가 `apple`
        // 앞에 서고, 견줌이 같다고 내면 기본 차례(a-1 먼저)로 선다 — 둘 다 아래 차례와 갈린다.
        let mut cased = [issues[0].clone(), issues[2].clone()];
        (cased[0].title, cased[0].assignee) = ("Banana".into(), Some("Bob".into())); // a-1
        (cased[1].title, cased[1].assignee) = ("apple".into(), Some("alice".into())); // a-3
        let by_key = |key| {
            let mut idx = [0, 1];
            idx.sort_by(|&x, &y| {
                order_by(key, false, (&cased[x], "todo"), (&cased[y], "todo"), &statuses, crate::config::Naming::Full)
            });
            idx.map(|i| cased[i].id.as_str())
        };
        assert_eq!(by_key(SortKey::Title), ["a-3", "a-1"], "제목이 대소문자를 접어 가나다로 안 섰다");
        assert_eq!(by_key(SortKey::Assignee), ["a-3", "a-1"], "담당이 대소문자를 접어 가나다로 안 섰다");
    }

    /// **접어 견준 차례는 소문자로 지어 견준 차례와 같다**(moai-y61p 단계 리뷰) — 낱말 끝 Σ 까지. 글자마다만
    /// 접으면 대문자로 적은 그리스어 제목·담당이 소문자로 적은 것과 자리를 바꿔 섰다, 다른 글이어도.
    /// 같은 머리를 건너뛰는 자리가 글자 가운데에 떨어지는 글(`é`·`É` 는 첫 바이트가 같다)도 함께 본다.
    #[test]
    fn caseless_orders_exactly_like_lowercased_strings() {
        let pairs = [
            ("ΟΔΟΣ ΑΛΦΑ", "οδος βητα"),
            ("ΟΔΟΣ", "οδος"),
            ("ΝΙΚΟΣ (a@x)", "Νικος (m@x)"),
            ("ΣΣ", "Σσ"),
            ("ΑΣ한", "ΑΣΑ"),
            ("레이븐 (raven@buzzni.com)", "레이븐 (raven@buzzni.com)"),
            ("Bump serde from 1.0.1 to 1.0.2", "bump Serde from 1.0.1 to 1.0.10"),
            ("Update the loader", "update the Loader"),
            ("a[", "A_"),
            ("İstanbul", "i\u{307}stanbul"),
            ("é", "É"),
            ("aé", "aÉb"),
            ("", "a"),
        ];
        for (a, b) in pairs {
            assert_eq!(caseless(a, b), a.to_lowercase().cmp(&b.to_lowercase()), "{a:?} · {b:?}");
            assert_eq!(caseless(b, a), b.to_lowercase().cmp(&a.to_lowercase()), "{b:?} · {a:?}");
        }
    }

    const NOW: &str = "2026-09-11T00:00:00Z";

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn issue(id: &str, status: &str, tags: &[&str]) -> Issue {
        let mut i =
            Issue::new(id.into(), format!("{id} 제목"), Kind::Issue, Status::new(status), "2026-09-01T00:00:00Z");
        i.tags = s(tags);
        i
    }

    fn cfg() -> crate::config::Config {
        crate::config::Config::parse("prefix = \"argos\"\n").unwrap()
    }

    /// 한 이슈만 두고 고르기. 소속 지도는 그 이슈에서 뽑는다.
    fn hit(f: &Filter, i: &Issue) -> bool {
        let all = [i.clone()];
        f.matches(i, NOW, &Where::of(&all, &cfg()))
    }

    fn f() -> Filter {
        Filter { all: true, ..Filter::default() }
    }

    /// **숨긴 까닭은 그 줄을 여는 한 낱말이다.** `matches` 와 꼬리 셈이 이
    /// 하나를 받는다 — 두 벌로 적었을 때 실제로 갈라졌다(moai-nnul).
    #[test]
    fn hidden_by_names_the_one_word_that_opens_the_row() {
        let plain = Filter::default();
        let why = |i: &Issue| {
            let all = [i.clone()];
            plain.hidden_by(i, &Where::of(&all, &cfg()))
        };
        let mut thought = issue("a-0001", "todo", &[]);
        thought.kind = Kind::Idea;
        let mut closed_thought = thought.clone();
        closed_thought.status = Status::new("done");
        let mut shelved = issue("a-0002", "todo", &[]);
        shelved.deferred_at = Some(NOW.into());
        let mut shelved_done = shelved.clone();
        shelved_done.status = Status::new("done");

        assert_eq!(why(&issue("a-0003", "todo", &[])), None);
        assert_eq!(why(&issue("a-0003", "done", &[])), Some(Hide::Done));
        assert_eq!(why(&thought), Some(Hide::Idea));
        assert_eq!(why(&shelved), Some(Hide::Deferred));
        // 닫고 미룬 줄은 `--all` 이 연다 — `--deferred` 는 done 을 그대로 숨긴다.
        assert_eq!(why(&shelved_done), Some(Hide::Done));
        // 닫은 생각은 어느 한 낱말로도 안 열린다.
        assert_eq!(why(&closed_thought), Some(Hide::Unopenable));

        // 숨김이 있으면 `matches` 도 안 고른다 — 한 규칙이다.
        for i in [&thought, &shelved, &closed_thought] {
            assert!(!hit(&plain, i), "{i:?}");
        }
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

    /// 범위마다 제 자리만 본다. **전체는 넷을 다 본다** — 좁힌 범위가 전체보다 더 찾으면 안 된다.
    #[test]
    fn grep_in_narrows_to_one_field_and_all_sees_every_one() {
        let mut i = issue("a-0042", "todo", &["parser"]);
        i.title = "저장 계층".into();
        i.body = Some("원자적 쓰기".into());
        let cases = [("0042", GrepIn::Id), ("저장", GrepIn::Title), ("pars", GrepIn::Tag), ("원자", GrepIn::Body)];
        for (q, only) in cases {
            let mut f = f();
            f.grep = Some(q.into());
            f.grep_in = GrepIn::All;
            assert!(hit(&f, &i), "전체가 {q} 를 못 찾았다");
            for g in GrepIn::ORDER.into_iter().filter(|g| *g != GrepIn::All) {
                f.grep_in = g;
                assert_eq!(hit(&f, &i), g == only, "{} 범위가 {q} 에 틀렸다", g.name());
            }
        }
        // 차례는 전체 → id → 제목 → 태그 → 본문 → 전체, 거꾸로도 돈다.
        let mut g = GrepIn::All;
        for want in [GrepIn::Id, GrepIn::Title, GrepIn::Tag, GrepIn::Body, GrepIn::All] {
            g = g.next();
            assert_eq!(g, want);
            assert_eq!(g.prev().next(), g);
        }
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
        let e =
            Filter::build(Raw { status: s(&["todo"]), filter: s(&["status=review"]), ..Raw::default() }).unwrap_err();
        assert!(e.contains("동시에"), "{e}");
        let e = Filter::build(Raw { filter: s(&["status=todo", "status=review"]), ..Raw::default() }).unwrap_err();
        assert!(e.contains("동시에"), "{e}");

        // 태그는 쌓이는 쪽이라 둘 다 걸린다 (반복=그리고).
        let f =
            Filter::build(Raw { tag: s(&["bug"]), filter: s(&["tag=parser"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &["bug", "parser"])));
        assert!(!hit(&f, &issue("a-0002", "todo", &["bug"])));

        // 빈 값은 "거르지 않는다" 다 — 플래그 쪽과 같은 뜻이어야 한다.
        let f = Filter::build(Raw { filter: s(&["tag="]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &issue("a-0001", "todo", &[])));
    }

    /// `--filter` 는 플래그와 정확히 같은 뜻이다.
    #[test]
    fn filter_string_equals_the_flags() {
        let flags = Filter::build(Raw { status: s(&["todo"]), tag: s(&["bug"]), epic: s(&["none"]), ..Raw::default() })
            .unwrap();
        let string =
            Filter::build(Raw { filter: s(&["status=todo", "tag=bug", "epic=none"]), ..Raw::default() }).unwrap();
        for i in
            [issue("a-0001", "todo", &["bug"]), issue("a-0002", "review", &["bug"]), issue("a-0003", "todo", &["perf"])]
        {
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
        let f = Filter::build(Raw { assignee: s(&["철수,other@example.com"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));
    }

    /// **맡길 때 쓴 문자열 그대로** 찾을 수 있다. `-a "이름 (메일)"` 은 맡기는
    /// 쪽의 모양이고, 그것을 그대로 필터에 넣는 것이 사람이 실제로 하는 일이다.
    #[test]
    fn assignee_takes_the_same_string_that_assigns() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("chulsu@example.com".into());
        let f =
            Filter::build(Raw { assignee: s(&["철수 (chulsu@example.com)"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));

        // 이름을 바꾼 사람도 옛 줄에서 사라지지 않는다 — 메일 한쪽만 맞아도 된다.
        let f =
            Filter::build(Raw { assignee: s(&["레이븐 (chulsu@example.com)"]), all: true, ..Raw::default() }).unwrap();
        assert!(hit(&f, &i));
    }

    /// 메일은 대소문자를 가리지 않는다.
    #[test]
    fn assignee_email_ignores_case() {
        let mut i = issue("a-0001", "todo", &[]);
        i.assignee = Some("철수".into());
        i.assignee_email = Some("Chulsu@Example.com".into());
        let f = Filter::build(Raw { assignee: s(&["chulsu@example.com"]), all: true, ..Raw::default() }).unwrap();
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
        let mut v = vec![issue("a-0003", "todo", &[]), issue("a-0001", "todo", &[]), issue("a-0002", "todo", &[])];
        v[0].priority = Some(0);
        sort_for_display(&mut v);
        let ids: Vec<&str> = v.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["a-0003", "a-0001", "a-0002"]);
    }
    /// **묶음은 서 있는 칸으로 고르고 숨긴다** (moai-j3b3). 멤버가 집힌 에픽이
    /// `-s todo` 에 걸리거나, 손으로 `done` 에 둔 진행 중인 에픽이 목록에서
    /// 사라지면 거름망이 화면과 다른 칸을 본다.
    #[test]
    fn a_group_is_filtered_by_the_column_it_stands_in() {
        let mut epic = issue("argos-0001", "done", &[]);
        epic.kind = Kind::Epic;
        let mut held = issue("argos-0002", "in_progress", &[]);
        held.epic = Some("argos-0001".into());
        let mut shut = issue("argos-0003", "todo", &[]);
        shut.kind = Kind::Epic;
        let mut closed = issue("argos-0004", "done", &[]);
        closed.epic = Some("argos-0003".into());
        let all = vec![epic, held, shut, closed];
        let cfg = cfg();
        let wh = Where::of(&all, &cfg);
        let picked = |raw: Raw| -> Vec<&str> {
            let f = Filter::build(raw).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.as_str()).collect()
        };
        assert_eq!(picked(Raw { status: s(&["todo"]), ..Raw::default() }), Vec::<&str>::new());
        assert_eq!(picked(Raw { status: s(&["in_progress"]), ..Raw::default() }), ["argos-0001", "argos-0002"]);
        assert_eq!(picked(Raw::default()), ["argos-0001", "argos-0002"], "읽은 칸으로 숨기지 않았다");
    }

    /// **종류가 다른 쌍둥이에게 가려진 줄은 어느 소속으로도 안 골린다.** 뒷줄 생각이
    /// 적은 에픽이 앞줄 이슈에 흘러, `moai show <에픽>` 은 `0/0` 이라 말하고 트리는 그
    /// 줄을 `(길 잃음)` 에 두는데 `-e <에픽>` 만 그 줄을 멤버로 냈다(moai-2m9p).
    #[test]
    fn an_eclipsed_row_is_picked_by_no_membership() {
        let mut epic = issue("argos-0001", "todo", &[]);
        epic.kind = Kind::Epic;
        let mut thought = issue("argos-0002", "todo", &[]);
        thought.kind = Kind::Idea;
        thought.epic = Some("argos-0001".into());
        let all = vec![epic, issue("argos-0002", "todo", &[]), thought];
        let cfg = cfg();
        let wh = Where::of(&all, &cfg);
        let picked = |raw: Raw| -> Vec<(&str, Kind)> {
            let f = Filter::build(raw).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| (i.id.as_str(), i.kind)).collect()
        };
        assert!(crate::report::group_members(&all, &all[0]).is_empty());
        assert_eq!(picked(Raw { epic: s(&["argos-0001"]), ..Raw::default() }), []);
        assert_eq!(picked(Raw { epic: s(&["none"]), ..Raw::default() }), [("argos-0001", Kind::Epic)]);
        assert_eq!(picked(Raw::default()), [("argos-0001", Kind::Epic), ("argos-0002", Kind::Issue)]);
    }

    /// **`--milestone` 은 마일스톤 줄을 다른 마일스톤의 것으로 안 고른다**(moai-8tav). 마일스톤 줄이
    /// 제 `milestone` 필드로 M2 를 들어도 `moai show M2` 는 `멤버 0/0` 이고 트리는 그 줄을 뿌리에
    /// 둔다 — 한때 `--milestone M2` 만 그 줄을 냈다. 그 줄 밑의 일은 여전히 제 마일스톤으로 골린다.
    #[test]
    fn a_milestone_line_is_not_picked_by_another_milestone() {
        let stone = |id: &str, m: Option<&str>| {
            let mut i = issue(id, "todo", &[]);
            i.kind = Kind::Milestone;
            i.milestone = m.map(Into::into);
            i
        };
        let all = vec![
            stone("argos-m002", None),
            stone("argos-m001", Some("argos-m002")),
            issue("argos-m001.aa1", "todo", &[]),
        ];
        let cfg = cfg();
        let wh = Where::of(&all, &cfg);
        let picked = |m: &str| -> Vec<&str> {
            let f = Filter::build(Raw { milestone: s(&[m]), ..Raw::default() }).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.as_str()).collect()
        };
        assert!(crate::report::group_members(&all, &all[0]).is_empty());
        assert_eq!(picked("argos-m002"), Vec::<&str>::new(), "마일스톤 줄을 다른 마일스톤의 것으로 골랐다");
        assert_eq!(picked("argos-m001"), ["argos-m001.aa1"]);
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
        let cfg = cfg();
        let wh = Where::of(&all, &cfg);
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
