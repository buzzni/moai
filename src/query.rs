//! 무엇을 보여줄지 고르고 어떤 차례로 놓을지 정한다.
//! **순수 함수다** — `&[Issue]` 와, 부르는 쪽이 읽어 건넨 저널 줄(노트·`rm` 줄)만 본다. 나중에 TUI 가
//! 그대로 쓴다.
//!
//! 문법은 한 문장이다:
//!
//! > **쉼표는 "또는", 플래그 반복은 "그리고", 서로 다른 플래그끼리는 "그리고".**
//!
//! 예외는 담당 하나다 — 되풀이도 또는이다(moai-97tn, 2026-10-03 사용자 결정). 한 줄의 담당은 하나라 그리고는
//! 늘 0건이었다.
//!
//! 미니 쿼리 언어(`"status=todo AND tag=bug"`)를 두지 않는다. 에이전트는
//! `--help` 를 읽고 명령을 만드는데 플래그는 도움말이 곧 문법이고, 쿼리
//! 문자열은 Bash heredoc 안에서 따옴표가 겹쳐 자주 깨진다. 서로 다른 필드
//! 사이에 OR 이 필요하다는 요청이 실제로 올 때 다시 본다.

use crate::model::{Issue, Kind, days_since, parse_date, parse_instant, parse_rfc3339};
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
    ///
    /// **읽는 자는 [`Where::milestone_of`] 하나다** — 거르개에 `--milestone` 이 없으면 아무도 안
    /// 읽는다. 그래서 줄만 든 쪽(탐색기의 `tui::Ground`)은 미리 안 짓고 처음 물을 때 짓는다
    /// ([`Lined::Later`], moai-6mnm).
    pub(crate) lines: Lined<'a>,
    /// 물려받은 것까지 친 미룸 (`report::Shelved`). 미룸도 소속처럼 묶음을 타고 내려온다.
    ///
    /// **줄로 묻는다** — 상세가 그 답을 그리는 자와 한 그릇이라, 한 화면이 제 말을 뒤집지
    /// 않는다(moai-wre3). 한때 여기만 접은 지도를 짚고 상세만 줄로 물어, 상세가 `미룸` 을
    /// 다는 묶음 줄을 `--deferred` 가 안 냈다.
    pub shelved: crate::report::Shelved<'a>,
    /// 묶음 → 멤버에서 읽은 칸. 묶음의 칸도 제 줄만 보고는 모른다.
    pub states: BTreeMap<crate::report::GroupKey<'a>, &'a str>,
    /// 묶음 → 읽은 칸에 든 때 (`report::Stand::entered`) — `--stale`·`--done`·아카이브가 읽는다. 막힘의
    /// 시계(`Stand::since`, `blocked_since`)와는 따로다: done 이 아닌 묶음에서는 같은 값이고, 셈에서 미뤄 빠진
    /// 멤버가 묶음을 닫았으면 그 미룸의 때다(moai-23q4).
    pub since: BTreeMap<crate::report::GroupKey<'a>, &'a str>,
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
    /// 이슈 id → 그 이슈의 노트 글([`NoteView`], moai-efoc.zyc). `-g` 가 노트까지 보는 재료다.
    ///
    /// **여기는 읽지 않는다 — 받는다**(2026-09-30 사용자 결정). 노트는 저널에 살고 이 모듈은 I/O 없는
    /// 순수 함수라, 저널을 읽는 것은 `cmd::show` 와 탐색기(`tui::Ground::read_notes`, moai-wcy8.vip)고
    /// 맞는지 가르는 것은 여기다. 안 실렸으면(`None`, `Where::default`) 노트를 안 본다 — 없는 노트와
    /// 안 읽은 노트를 가를 일이 이 모듈에는 없다. 상태 계산이 아니라 글 찾기라 "저널은 상태 계산에 안
    /// 읽힌다" 와 안 부딪친다.
    pub notes: Option<NoteView<'a>>,
    /// 읽는 사람의 시간대(moai-efoc). `YYYY-MM-DD` 로 친 때의 끝([`End::Wall`])을 그 사람의 날로 재는
    /// 재료다 — 마일스톤 기한이 `report::Dues::split` 에서 받는 것과 같은 값이다. **받는다**: 시간대를 푸는
    /// 것은 명령 레이어(`cmd`)고, 날로 친 끝이 없으면 풀지도 얹지도 않는다([`Filter::needs_zone`]) — 탐색기도
    /// 같은 문으로 제 화면의 시간대(`tui::App::zone`)를 얹는다. 안 실렸으면(`None`) 도장 그대로, 곧 UTC 로 잰다.
    pub zone: Option<&'a crate::tz::Zone>,
    /// done 칸에 든 지 이 날수가 지나면 아카이브다(`Config::archive_days`, moai-47mz). **설정에서 온다** —
    /// [`Where::from_soil`] 이 설정을 받아 옮기고, 탐색기의 거름망은 적재가 설정에서 옮겨 둔 값을 받는다
    /// (`tui::Ground::here`). 0 이면(`Where::default`) 아카이브가 없다.
    pub archive_days: i64,
    /// `.moai/archive` 에서만 온 줄의 id — 산 줄이 없는 id 다(moai-bth3 리뷰). 시계와 상관없이 아카이브다
    /// ([`crate::report::is_put_away`]): `archive_days` 를 0 으로 끄거나 늘려도 `--all` 에 돌아오지 않는다. 아카이브를
    /// 겹쳐 읽은 쪽(`cmd::show`)이 싣고, 안 실었으면(`Where::default`) 시계만 본다.
    pub stored: BTreeSet<&'a str>,
}

/// 이슈 id → 그 이슈에 붙은 노트 글들(`model::note_of` — `moai note` 의 글과 칸 옮김의 `-m`).
/// **이슈의 필드가 아니다** — 스냅샷에 안 적히고 저널에서 읽힌 값이라, 줄(`Issue`) 곁에 따로 든다.
pub type Notes = BTreeMap<String, Vec<String>>;

/// [`Where::notes`] 가 받은 노트 — **이미 접었는가**를 함께 든다(리뷰 moai-wcy8.rbj).
///
/// 거름망은 노트를 [`Filter::build`] 가 `q` 를 접는 것과 같은 `to_lowercase` 로 접어 견준다. 그 접기를 언제
/// 하느냐가 부르는 쪽마다 다르다 — 한 번 거르고 끝나는 CLI(`show -g`)는 숨길 줄의 노트까지 미리 접을 까닭이
/// 없고, 키마다 다시 거르는 탐색기는 한 번 접어 두어야 한다. 이 저장소의 노트 4.4MB 를 키마다 접으면
/// release 로 85ms 가 들고, 접어 둔 것에서 찾으면 1ms 도 안 든다. 어느 쪽이든 답은 같다.
#[derive(Debug, Clone, Copy)]
pub enum NoteView<'a> {
    /// 받은 글 그대로 — 견줄 때 그 줄의 노트만 접는다.
    Raw(&'a Notes),
    /// [`fold_notes`] 로 이미 접은 글.
    Folded(&'a Notes),
}

/// 노트를 거름망의 자로 **한 번** 접는다 — [`NoteView::Folded`] 가 드는 꼴이다. 접는 법은 [`Filter::build`] 가
/// `q` 를 접는 것과 같은 `to_lowercase` 하나다.
pub fn fold_notes(notes: &Notes) -> Notes {
    notes.iter().map(|(id, texts)| (id.clone(), texts.iter().map(|t| t.to_lowercase()).collect())).collect()
}

/// 노트 글에서 `q` 가 든 **줄**(moai-wcy8.3v9) — 탐색기 상세가 노트에서 걸린 줄이 왜 걸렸는지 그리는
/// 재료다. `q` 는 친 그대로 받아 여기서 접는다 — [`Filter::build`] 와 같은 `to_lowercase` 라, 거름망이
/// 노트로 건 줄([`Where::noted`])에는 여기서도 걸린 줄이 선다. 거름망은 노트를 통째로 견주지만, 탐색기의
/// 검색 칸은 줄바꿈을 빈칸으로 받으므로 친 글이 두 줄에 걸칠 일이 없다.
///
/// 빈칸뿐인 글은 아무 줄도 안 낸다 — 칠하는 쪽(`tui::draw::mark`)도 그런 글은 안 칠한다. 모든 노트 줄을
/// 늘어놓는 것은 "왜 걸렸나" 의 답이 아니다.
pub fn noted_lines<'n>(texts: &'n [String], q: &str) -> Vec<&'n str> {
    if q.trim().is_empty() {
        return Vec::new();
    }
    let q = q.to_lowercase();
    texts.iter().flat_map(|t| t.lines()).filter(|l| l.to_lowercase().contains(&q)).collect()
}

/// [`Where::lines`] 의 그릇 — 지은 것이거나, 처음 물을 때 지을 줄이다.
///
/// **탐색기의 거르개가 키마다 [`crate::report::Lines`] 를 짓던 자리다**(moai-6mnm, 리뷰
/// moai-jk2u.m60 4번). 1만 줄에서 opt-level 3 으로 3.8ms, dev 로 50ms 쯤이다. 그 지도는
/// `&Issue` 를 들어 줄과 함께 사는 `tui::Site` 에 담으면 제 필드를 빌리는 구조체가 된다.
/// 읽는 자가 `--milestone` 거르개 하나이니, 담지 않고 **안 물으면 안 짓는다.** 물으면
/// 그 거르개 한 판에 한 번 짓는다 — 예전과 같은 값이다.
pub(crate) enum Lined<'a> {
    Ready(crate::report::Lines<'a>),
    Later(&'a [Issue], std::cell::OnceCell<crate::report::Lines<'a>>),
}

impl Default for Lined<'_> {
    fn default() -> Self {
        Lined::Ready(crate::report::Lines::default())
    }
}

impl<'a> Lined<'a> {
    /// 그 줄로 **처음 물을 때** 짓는다.
    pub(crate) fn later(all: &'a [Issue]) -> Lined<'a> {
        Lined::Later(all, std::cell::OnceCell::new())
    }

    fn get(&self) -> &crate::report::Lines<'a> {
        match self {
            Lined::Ready(lines) => lines,
            Lined::Later(all, built) => built.get_or_init(|| crate::report::Lines::of(all)),
        }
    }
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
        let since = stands.into_iter().map(|(id, s)| (id, s.entered)).collect();
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
        Where {
            epic,
            lines: Lined::Ready(lines),
            shelved,
            states,
            since,
            kinds,
            folded,
            notes: None,
            zone: None,
            archive_days: cfg.archive_days,
            stored: BTreeSet::new(),
        }
    }

    /// 그 줄의 노트 가운데 `q` 가 든 것이 있는가(moai-efoc.zyc). `q` 는 이미 소문자다([`Filter::build`]).
    /// 노트가 안 실렸으면 거짓이다 — [`Where::notes`]. 받은 글 그대로면 여기서 접고, 접어 둔 것이면 그대로
    /// 찾는다([`NoteView`]).
    pub fn noted(&self, i: &Issue, q: &str) -> bool {
        match self.notes {
            None => false,
            Some(NoteView::Raw(n)) => {
                n.get(&i.id).is_some_and(|texts| texts.iter().any(|t| t.to_lowercase().contains(q)))
            }
            Some(NoteView::Folded(n)) => n.get(&i.id).is_some_and(|texts| texts.iter().any(|t| t.contains(q))),
        }
    }

    /// 그 줄이 종류가 다른 쌍둥이에게 id 가 가려졌는가 (`report::is_eclipsed`).
    pub fn eclipsed(&self, i: &Issue) -> bool {
        self.kinds.eclipses(i)
    }

    /// 그 줄이 서 있는 칸 (`report::column`).
    pub fn column<'x>(&'x self, i: &'x Issue) -> &'x str {
        crate::report::column(i, &self.states)
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
        self.epic.stood(i, self.lines.get())
    }

    /// 그 줄이 **지금 칸에 들어선 때** — `--stale`·`--done`·아카이브가 재는 시각.
    ///
    /// 묶음이면 읽은 칸에 든 때다(`report::Stand::entered`) — 셈이 마지막으로 움직인 때거나, done 이면 남은
    /// 멤버를 미룬 때 가운데 늦은 것. 적힌 `status_since` 는 아무 데서도 안 읽히는 칸의 시각이라, 그것으로
    /// 재면 오늘 진행 중이 된 에픽이 `-s in_progress --stale 10` 에 걸린다 — 고르는 자와 재는 자가 어긋난다.
    pub fn since<'x>(&'x self, i: &'x Issue) -> &'x str {
        // **고르는 자와 재는 자가 한 문을 지난다**(리뷰, `report::stands_on`). 위의 `column` 만
        // 가려진 줄을 거르면, `-s` 가 제 칸으로 고른 그 줄의 나이는 쌍둥이 묶음의 셈에서 와
        // `--stale` 이 265일 된 줄을 하루짜리로 잰다.
        crate::report::stands_on(i, |k| self.since.get(&k).copied()).unwrap_or(i.status_since.as_str())
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

    /// 아카이브인가(moai-47mz) — 칸과 그 칸에 든 때를 **목록이 고르고 재는 바로 그 자**([`Where::column`]·
    /// [`Where::since`])로 읽어 [`crate::report::archived`] 에 댄다. `-s` 가 고른 칸과 `--done <폭>` 이 잰 때가
    /// 아카이브와 한 줄을 다르게 보지 않는다.
    pub fn archived(&self, i: &Issue, now: &str) -> bool {
        let stored = self.stored.contains(i.id.as_str());
        crate::report::is_put_away(self.column(i), self.since(i), now, self.archive_days, stored)
    }
}

/// 글로 찾을 때 **어디를 보는가**(moai-kojj). TUI 검색 칸의 Tab 이 이 차례로 돈다.
///
/// `All` 은 다른 범위를 다 본다 — CLI 의 `-g` 도 이것이다. 한때 제목·본문만 봤는데, 그러면 id
/// 조각이나 태그로 찾은 것이 `All` 에서는 안 걸리고 좁힌 범위에서만 걸린다. 좁힌 것이
/// 넓은 것보다 더 찾으면 "전체" 라는 이름이 거짓말이 된다.
///
/// **노트는 `All` 과 `Note` 가 본다**(moai-efoc.zyc·moai-wcy8.3v9, [`GrepIn::sees_notes`]) — 노트가 실렸을
/// 때만([`Where::notes`]). 노트는 줄의 필드가 아니라 저널에서 읽힌 글이라, 실은 쪽(CLI `show`·탐색기의
/// 거름망, [`Filter::sees_notes`])이 없으면 두 범위 다 노트로는 아무것도 못 찾는다.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GrepIn {
    #[default]
    All,
    Id,
    Title,
    Tag,
    Body,
    /// 노트만 — `moai note` 의 글과 칸 옮김의 `-m`(`model::note_of`). **맨 끝에 선다**: 줄에 적힌
    /// 넷을 다 돈 뒤에 저널로 넘어간다(2026-09-30 사용자 결정, moai-wcy8).
    Note,
}

impl GrepIn {
    const ORDER: [GrepIn; 6] = [GrepIn::All, GrepIn::Id, GrepIn::Title, GrepIn::Tag, GrepIn::Body, GrepIn::Note];

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

    /// 노트를 이 범위가 보는가(moai-efoc.zyc·moai-wcy8.3v9). 글은 줄 곁에 따로 오므로 [`GrepIn::hits`] 가
    /// 아니라 [`Filter::matches`] 가 [`Where::noted`] 와 함께 묻는다. 탐색기 상세가 걸린 노트 줄을 그릴지도
    /// 이것으로 가른다 — 칠할 자리를 가르는 `sees_*` 넷과 같은 까닭이다.
    pub fn sees_notes(self) -> bool {
        matches!(self, GrepIn::All | GrepIn::Note)
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
    /// 줄 자신의 `updated_at` 이 든 폭(`--since`·`updated_at=`, moai-efoc.ip5). **AND of OR** — `tags` 와 같은
    /// 꼴이다: 플래그를 되풀이하면 그리고(폭이 겹치는 곳), 쉼표는 또는.
    pub updated: Vec<Vec<Span>>,
    /// `created_at` 이 든 폭(`--created`·`created_at=`).
    pub created: Vec<Vec<Span>>,
    /// **지금 done 에 선 줄이 거기 든 때**의 폭(`--done`). 재는 자는 [`Where::since`] 다 — 묶음이면 멤버가
    /// 마지막으로 done 에 든 때거나 남은 멤버를 미룬 때 가운데 늦은 것이다. 지우거나 빼서 닫힌 묶음은 흔적이
    /// 없어 끝난 멤버의 때로 선다(`report::Stand::entered`).
    pub done: Vec<Vec<Span>>,
    /// 줄 자신의 `started_at` 이 든 폭(`started_at=`, moai-97tn). **묶음도 제 필드만 본다** — 멤버로 재지 않는다
    /// (2026-10-03 사용자 결정). 필드가 없으면 모르는 것이라 어느 폭에도 안 든다.
    pub started: Vec<Vec<Span>>,
    /// 줄 자신의 `done_at` 이 든 폭(`done_at=`, moai-97tn) — 마지막으로 done 에 든 때라 되돌린 줄도 걸린다.
    /// [`Filter::done`] 과 다른 물음이다: 저쪽은 지금 선 칸으로 재고 묶음은 멤버로 잰다. 키 이름이 `--json` 의
    /// 필드라 값도 그 필드 그대로다(2026-10-03 사용자 결정).
    pub done_at: Vec<Vec<Span>>,
    /// done 을 포함한다. **아카이브는 아니다**(moai-47mz) — 그것은 [`Filter::archived`] 가 연다.
    pub all: bool,
    /// 아카이브(done 에 든 지 오래된 줄, [`Where::archived`])까지 포함한다 — `--archived`. 글로 찾거나(`-g`)
    /// 때로 물으면([`Filter::times`] — `--since`·`--created`·`--done`·`*_at=`) 저절로 켜진다: 찾는 물음과 때를 콕
    /// 집은 물음이다.
    pub archived: bool,
    /// 미뤄 둔 것만 고른다. `Some(false)` 면 미루지 않은 것만.
    pub deferred: Option<bool>,
    /// 담아 둔 생각까지 포함한다. **`all` 과 같은 자리의 축이다** — 기본으로
    /// 숨는 것을 도로 켜는 스위치가 둘이 되면, 켜는 쪽이 어느 것을 켰는지
    /// 매번 되짚어야 한다.
    pub backlog: bool,
}

/// 거르개가 값을 거절한 까닭. **자료만 든다**(moai-2htt) — 말은 [`crate::view::bad_filter`] 가 고른 말로
/// 짓는다. 여기서 글을 지으면 영어 화면과 `--json` 의 `error` 에도 한국어가 선다. 읽음 표(`read_marks`)와
/// 설정(`config::Trouble`)이 자료만 내고 `view` 가 펴는 것과 같은 꼴이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BadFilter {
    /// 한 번만 쓰는 거르개를 두 번 썼다 — `-s todo -s review`. `a`·`b` 는 앞의 두 값이고, `rest` 는 그 뒤의 값이다.
    /// **셋째부터도 든다**(리뷰 moai-efoc.3e1) — 앞의 둘만 들던 판은 `-s todo -s review -s done` 에 `-s todo,review` 를
    /// 대어, 그대로 친 사람이 done 의 줄을 말없이 잃었다.
    Twice { field: Once, a: String, b: String, rest: Vec<String> },
    /// `--done` 에 done 을 안 든 `-s` 를 함께 줬다 — 두 거르개에 함께 걸리는 줄이 없다. `asked` 는 `-s` 의
    /// 값을 쉼표로 이은 것이다.
    DoneOutside { asked: String },
    /// 끝이 하나도 없는 폭(`~`·`..`).
    Endless(String),
    /// 끝이 앞보다 이른 폭.
    Backwards(String),
    /// 때로 못 읽는 값.
    NotATime(String),
    /// 때가 하나도 없는 값(`--since ''`·`--created ,`).
    NoTime,
    /// `--filter` 의 항목이 `항목=값` 꼴이 아니다.
    NotAPair(String),
    /// 없는 항목 이름(`--filter statu=todo`).
    NoSuchKey(String),
    /// `stale=` 의 값이 날 수가 아니다.
    NotDays(String),
    /// 우선순위로 못 읽는 값.
    NotAPriority(String),
    /// `type=` 의 값이 종류가 아니다. **`Kind` 의 거절문을 그대로 든다** — 그 글은 `--type` 을 푸는 clap 과
    /// 한 덩이로 서도록 영어 하나로 정했다(moai-ivt9). 같은 값을 두 자리가 다른 말로 거절하면 안 된다.
    /// **`Kind` 처럼 moai 가 글을 쥔 자리에만 선다** — `--stale x` 는 clap 이 수로 풀다 제 영어(표준
    /// 라이브러리의 글)로 거절해 나눠 쓸 글이 없으므로, `stale=` 은 [`BadFilter::NotDays`] 로 고른 말을 따른다.
    NotAKind(String),
}

/// 한 번만 쓰는 거르개 — [`BadFilter::Twice`] 가 어느 것을 두 번 썼는지 댄다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Once {
    Status,
    Epic,
    Milestone,
    Parent,
    Priority,
    /// `-g`·`grep=` — 또는이 없는 거르개다. 쉼표는 찾을 글의 글자라 고칠 글을 쉼표로 잇지 않는다(moai-ltsv.auf).
    Grep,
    /// `--type`·`type=` — 한 줄의 종류는 하나라, 둘을 함께 주면 늘 0건이었다.
    Kind,
    /// `--stale`·`stale=` — 날수 하나다.
    Stale,
}

impl Once {
    /// 그 거르개의 플래그 — 거절문이 고쳐 칠 명령에 그대로 싣는다.
    pub fn flag(self) -> &'static str {
        match self {
            Once::Status => "-s",
            Once::Epic => "-e",
            Once::Milestone => "--milestone",
            Once::Parent => "--parent",
            Once::Priority => "-p",
            Once::Grep => "-g",
            Once::Kind => "--type",
            Once::Stale => "--stale",
        }
    }

    /// 그 거르개의 항목 이름 — `--filter`·탐색기 거름망의 `항목=값` 에서 `=` 앞에 서는 낱말이다([`KEYS`]).
    /// 거름망의 거절문이 고쳐 칠 글을 이것으로 짓는다(moai-tckz).
    pub fn key(self) -> &'static str {
        match self {
            Once::Status => "status",
            Once::Epic => "epic",
            Once::Milestone => "milestone",
            Once::Parent => "parent",
            Once::Priority => "priority",
            Once::Grep => "grep",
            Once::Kind => "type",
            Once::Stale => "stale",
        }
    }

    /// 값을 쉼표로 이어 "어느 것이든" 을 물을 수 있는가. `false` 인 셋은 한 값만 받는다 — 거절문이 쉼표로 이은
    /// 글을 대면 그대로 친 사람이 또 거절되거나(`type=`·`stale=`) 쉼표가 든 글을 찾는다(`grep=`).
    pub fn joins(self) -> bool {
        !matches!(self, Once::Grep | Once::Kind | Once::Stale)
    }
}

/// 값 하나만 드는 거르개(`-g`·`--type`·`--stale`)를 플래그와 `--filter` 를 아울러 두 번 넘게 줬는가(moai-ltsv.auf).
///
/// **[`desugar`] 를 돌기 전에 잰다.** 그 셋은 [`Raw`] 에 `Option` 하나로 서서, 거르개 글을 펴는 동안에는 뒤의 값이
/// 앞의 값을 말없이 덮었다 — `--filter grep=one --filter grep=two` 가 two 의 줄만 냈다. 펴는 중에 거절하면
/// 셋째부터의 값이 `rest` 에 못 든다. `Raw` 를 `Vec` 로 바꾸지 않는 까닭은 탐색기의 `/` 검색이 `grep` 을 그 꼴로
/// 채우기 때문이다 — 그쪽에는 두 번째 값이 올 길이 없다.
fn single(raw: &Raw) -> Result<(), BadFilter> {
    for field in [Once::Grep, Once::Kind, Once::Stale] {
        let given = match field {
            Once::Grep => raw.grep.clone(),
            Once::Kind => raw.kind.map(|k| k.as_str().to_string()),
            Once::Stale => raw.stale.map(|d| d.to_string()),
            _ => None,
        };
        // 항목 이름과 값을 자르는 법은 `desugar` 와 같다 — 빈칸을 걷고 첫 `=` 에서 가른다.
        let typed = raw.filter.iter().filter_map(|one| {
            let (k, v) = one.trim().split_once('=')?;
            (k.trim() == field.key()).then(|| v.trim().to_string())
        });
        if let [a, b, rest @ ..] = given.into_iter().chain(typed).collect::<Vec<_>>().as_slice() {
            return Err(BadFilter::Twice { field, a: a.clone(), b: b.clone(), rest: rest.to_vec() });
        }
    }
    Ok(())
}

/// 한 번만 쓸 수 있는 플래그를 두 번 썼을 때. 규칙(반복=그리고)을 지키면서도
/// 사람이 실제로 저지르는 실수를 잡는다.
fn once(values: &[String], field: Once) -> Result<Vec<String>, BadFilter> {
    match values {
        [] => Ok(Vec::new()),
        [one] => Ok(csv(one)),
        [a, b, rest @ ..] => Err(BadFilter::Twice { field, a: a.clone(), b: b.clone(), rest: rest.to_vec() }),
    }
}

/// 있는 필터 항목. 모르는 키를 만나면 이 목록을 그대로 보여준다.
pub const KEYS: &[&str] = &[
    "status",
    "tag",
    "no-tag",
    "epic",
    "milestone",
    "parent",
    "priority",
    "assignee",
    "type",
    "grep",
    "stale",
    "since",
    "created",
    "done",
    "created_at",
    "updated_at",
    "started_at",
    "done_at",
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
    pub grep_in: GrepIn,
    pub stale: Option<i64>,
    /// `--since`·`--created`·`--done` 의 날것 — 아직 안 쪼갰다(moai-efoc.ip5).
    pub since: Vec<String>,
    pub created: Vec<String>,
    pub done: Vec<String>,
    /// `--filter` 의 `updated_at=`·`started_at=`·`done_at=` 의 날것(moai-97tn). `created_at=` 은 `created` 에
    /// 쌓인다 — 같은 필드를 같은 자로 잰다. `updated_at=` 은 `since` 와 필드는 같지만 한 날을 그 하루로 읽어
    /// 따로 든다.
    pub updated_at: Vec<String>,
    pub started_at: Vec<String>,
    pub done_at: Vec<String>,
    pub all: bool,
    /// `--archived` — 아카이브까지 연다. done 도 연다(`all` 을 품는다).
    pub archived: bool,
    pub backlog: bool,
    pub deferred: bool,
    pub filter: Vec<String>,
}

/// 기본 목록이 줄을 숨긴 까닭. **그 줄을 여는 한 낱말로 가른다.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hide {
    /// `--all` 이 연다.
    Done,
    /// `--type backlog` 가 연다.
    Backlog,
    /// `--deferred` 가 연다.
    Deferred,
    /// `--archived` 가 연다 — done 칸에 든 지 오래된 줄(moai-47mz). `--all` 로는 안 열린다.
    Archived,
    /// 어느 한 낱말로도 안 열린다 (닫아 둔 생각). 세지 않는다.
    Unopenable,
}

impl Filter {
    pub fn build(mut raw: Raw) -> Result<Filter, BadFilter> {
        // `--filter` 는 **플래그와 같은 자리에 쌓인다.** 뜻을 정하는 코드가
        // 아래 한 곳뿐이라야 두 표현이 갈라지지 않는다 — 예전처럼 `Filter` 에
        // 직접 쓰면 `--filter` 가 플래그를 조용히 덮어썼고, `-s` 를 두 번 썼을
        // 때 나오는 친절한 오류도 그 길에서만 사라졌다.
        single(&raw)?;
        for one in std::mem::take(&mut raw.filter) {
            desugar(&mut raw, &one)?;
        }
        // 콕 집어 묻거나(`--type backlog`) 글로 찾을 때는 저절로 켜진다.
        // **이미 적어 둔 생각을 다시 안 적으려면 찾아져야 한다.**
        //
        // `--deferred` 도 콕 집어 묻는 자리다. **`status` 의 `미뤄 둔 것 N건`
        // 은 종류를 안 가리고 세므로**(미뤄 둔 에픽·생각까지), 그 줄이 가리키는
        // 명령이 생각을 숨기면 세어 놓고 못 보여 주는 수가 된다 — `backlog_pile`
        // 이 미뤄 둔 것을 빼서 피한 바로 그 덫이고, 여기서는 세는 쪽을 못
        // 좁히니(좁히면 미뤄 둔 에픽이 아무 데서도 안 보인다) 보는 쪽을 연다.
        //
        // 때로 물으면 이것도 함께 연다 — 맨 끝의 "숨김을 다 연다" 다.
        let backlog = raw.backlog || raw.kind == Some(Kind::Backlog) || raw.grep.is_some() || raw.deferred;
        // **`--deferred` 는 그것만 본다.** 목록 자리에서 미룬 것은 done 처럼
        // 기본으로 빠지므로, 켜는 말과 좁히는 말이 하나여야 "미룬 것 보기" 가
        // 한 낱말로 끝난다.
        let deferred = raw.deferred.then_some(true);
        let status = once(&raw.status, Once::Status)?;
        let done = spans(&raw.done, Span::parse)?;
        // **`--done` 은 지금 done 에 선 줄만 본다** — `-s` 가 done 을 안 들면 두 거르개에 함께 걸리는 줄이
        // 없어 늘 0건이다. 조용히 0건을 내면 "그때 닫힌 것이 없었다" 와 안 갈린다(`-s todo -s review` 를
        // 거절하는 `once` 와 같은 까닭이다). `-s done,review` 처럼 done 이 든 목록은 그대로 받는다.
        if !done.is_empty() && !status.is_empty() && !status.iter().any(|s| s == crate::config::DONE) {
            return Err(BadFilter::DoneOutside { asked: status.join(",") });
        }
        let archived = raw.archived || raw.stale.is_some() || raw.grep.is_some();
        let mut filter = Filter {
            status,
            tags: raw.tag.iter().map(|t| split_tags(t)).filter(|v: &Vec<String>| !v.is_empty()).collect(),
            no_tags: raw.no_tag.iter().flat_map(|t| split_tags(t)).collect(),
            epic: sel(once(&raw.epic, Once::Epic)?),
            milestone: sel(once(&raw.milestone, Once::Milestone)?),
            parent: sel(once(&raw.parent, Once::Parent)?),
            priority: parse_priorities(&once(&raw.priority, Once::Priority)?)?,
            // **담당은 되풀이도 또는이다**(moai-97tn, 2026-10-03 사용자 결정) — `-a 철수 -a 영희` 는 둘 가운데 하나다.
            // 되풀이를 또는으로 읽는 거르개는 이것 하나다: 한 줄의 담당은 하나라 그리고는 늘 0건이었고, 그래서 두 번
            // 쓰면 거절하던 자리다. 거르개 글에서 사람을 여럿 고르려면 `assignee=` 를 되풀이하는 것이 자연스럽다.
            assignee: sel(raw.assignee.iter().flat_map(|v| csv(v)).collect()),
            kind: raw.kind,
            // 한 번만 내려 두면 이슈마다 다시 만들 일이 없다.
            grep: raw.grep.map(|q| q.to_lowercase()),
            grep_in: raw.grep_in,
            stale: raw.stale,
            // `since=` 는 그때부터, `updated_at=` 은 그 폭이다 — 같은 필드라 한 묶음에 쌓는다(그리고).
            updated: [spans(&raw.since, Span::since)?, spans(&raw.updated_at, Span::parse)?].concat(),
            created: spans(&raw.created, Span::parse)?,
            done,
            started: spans(&raw.started_at, Span::parse)?,
            done_at: spans(&raw.done_at, Span::parse)?,
            // **아카이브를 여는 말은 셋이다**(moai-47mz, 2026-10-03 사용자 결정). `--archived` 는 done 까지 열어
            // 옛 `--all` 과 같은 줄을 낸다. 글로 찾으면(`-g`) 연다 — 아카이브는 숨는 것이지 지운 것이 아니라
            // 찾아져야 한다. 때로 물으면 연다 — 아래의 "숨김을 다 연다" 가 함께 연다. **`--stale` 도 때로 묻는
            // 말이다**(리뷰 moai-47mz.5il) — `-s done --stale 20` 은 아카이브 날수를 넘긴 줄만 고르는 물음이라, 안
            // 열면 늘 0건이다. 다만 `--stale` 은 숨김을 다 열지는 않는다(옛날처럼 done 은 `-s done` 이 연다).
            // **`-g` 는 done 은 안 연다** — 찾은 아카이브 줄은 done 처럼 꼬리에 세이고 `--all` 이 연다.
            all: raw.all || raw.archived,
            archived,
            backlog,
            deferred,
        };
        // **시간으로 물으면 숨김을 다 연다**(moai-efoc.ip5, 2026-09-30 사용자 결정) — done·미룸·생각·아카이브까지.
        // 그 물음은 "그사이 무엇이 바뀌었나" 이고 그사이 닫힌 줄도 바뀐 줄이다. 기본 숨김을 그대로 두면
        // `--since` 로 증분을 받는 쪽이 닫힌 줄을 말없이 놓친다 — 사람 화면은 꼬리에 숨긴 수를 대지만
        // `--json` 에는 그 꼬리가 없다. 다시 좁히는 것은 `-s`(칸을 적는다)와 `--type` 이다 — `--deferred` 는
        // 미룬 것**만** 남기고, 미룬 줄을 빼는 말은 없다.
        //
        // **물었는지는 [`Filter::times`] 에서 읽는다**(리뷰 moai-97tn.p44) — 시간대를 묻는 자([`Filter::needs_zone`])와
        // 한 목록이다. 날것(`Raw`)의 이름을 여기 따로 늘어놓던 때는 `updated_at=` 을 빼도 아무 시험이 안 붉어졌다.
        // 빈 값은 `spans` 가 거절하므로 날것이 섰으면 폭도 선다 — 같은 답이다.
        if filter.times().iter().any(|v| !v.is_empty()) {
            (filter.all, filter.archived, filter.backlog) = (true, true, true);
        }
        Ok(filter)
    }

    /// 때로 묻는 폭 전부 — `--since`·`updated_at=`(한 묶음), `--created`·`created_at=`, `--done`, `started_at=`,
    /// `done_at=`. 숨김을 여는 자([`Filter::build`])와 시간대를 묻는 자([`Filter::needs_zone`])가 이 하나를 읽는다.
    ///
    /// **필드를 다 푼다**(`..` 없이, `cmd::show::first_given` 과 같은 까닭) — 거르개에 필드가 더해지면 여기가
    /// 컴파일되지 않아, 그것이 때로 묻는 폭인지 정하게 된다. 두 자가 저마다 이름을 늘어놓던 때는 한쪽을 빠뜨려도
    /// 컴파일이 됐다 — 숨김을 안 열면 그사이 닫힌 줄을 말없이 놓치고, 시간대를 안 물으면 그 키만 UTC 의 날로 잰다.
    fn times(&self) -> [&Vec<Vec<Span>>; 5] {
        let Filter {
            status: _,
            tags: _,
            no_tags: _,
            epic: _,
            milestone: _,
            parent: _,
            priority: _,
            assignee: _,
            kind: _,
            grep: _,
            grep_in: _,
            stale: _,
            updated,
            created,
            done,
            started,
            done_at,
            all: _,
            archived: _,
            deferred: _,
            backlog: _,
        } = self;
        [updated, created, done, started, done_at]
    }

    /// 노트를 봐야 답하는가 — 글로 찾고, 그 범위가 노트를 본다([`GrepIn::sees_notes`]). 없으면 부르는 쪽이
    /// 저널을 풀지 않는다(탐색기의 `App::apply`, 리뷰 moai-wcy8.rbj) — [`Filter::needs_zone`] 과 같은 결이다.
    pub fn sees_notes(&self) -> bool {
        self.grep.is_some() && self.grep_in.sees_notes()
    }

    /// 읽는 사람의 시간대가 있어야 답하는가 — 날(`YYYY-MM-DD`)이나 분(`YYYY-MM-DD HH:MM`)으로 친 때가 있다
    /// ([`End::Wall`]). 없으면 부르는 쪽이 시간대를 풀지 않는다: 시각을 안 그리는 목록은 tzdb 를 안 만진다(moai-s3i7).
    /// 폭의 목록은 숨김을 여는 자와 한 벌이다([`Filter::times`]).
    pub fn needs_zone(&self) -> bool {
        self.times().iter().any(|v| v.iter().flatten().any(Span::walled))
    }

    /// 기본 목록이 이 줄을 숨기는가, 숨긴다면 **어느 한 낱말이 그것을 여는가.**
    ///
    /// **숨김 규칙은 여기 하나다.** `matches` 도 이것으로 거르고, 숨긴 수를
    /// 세는 쪽(`moai show` 의 꼬리)도 이것을 받아 세기만 한다. 한때 `show` 가
    /// 같은 세 규칙을 다시 적었고, 두 벌이라 실제로 갈라졌다(moai-nnul).
    ///
    /// - **담아 둔 생각은 기본 목록에서 빠진다.** 이 자리는 일을 보는 자리고,
    ///   생각 조각이 섞이면 목록이 흐려져 담기가 꺼려진다. 콕 집어 묻거나
    ///   (`--type backlog`) 글로 찾을 때는 나온다 — 이미 적어 둔 생각을 다시 안
    ///   적으려면 찾아져야 한다. 탐색기는 `backlog` 를 켜고 들어와 시키지도 않은
    ///   줄을 숨기지 않는다.
    /// - **미뤄 둔 것은 done 과 같은 자리에서 빠진다.** 지금 계획이 아니라는
    ///   뜻이 같고, 켜는 말(`--all`)도 같아야 축이 안 는다. 물려받은 미룸도
    ///   미룸이다 — 미룬 에픽의 멤버가 목록에 남으면 `ready` 와 보드가 빼 둔
    ///   것을 목록만 계획으로 낸다.
    ///
    /// 까닭은 **그 줄을 실제로 여는 한 낱말**로 가른다. 첫 까닭으로 가르면
    /// 닫아 둔 생각이 `backlog N건 숨김 — --type backlog` 로 서는데 그 명령은 done 을
    /// 여전히 숨겨 아무것도 안 낸다.
    ///   `--type backlog` 는 backlog 만 연다 (done·미룸은 그대로 숨긴다)
    ///   `--deferred`  는 미룸을 열고 생각까지 같이 연다 (done 은 아니다)
    ///   `--all`       은 done 과 미룸을 연다 (생각과 아카이브는 아니다)
    ///   `--archived`  는 아카이브까지 연다 (done·미룸도 연다, 생각은 아니다)
    ///
    /// - **아카이브는 done 안의 한 겹 더다**(moai-47mz). done 을 연 목록(`--all`·`-s done`)에서도 done 에 든 지
    ///   오래된 줄은 빠지고, 그 줄을 여는 한 낱말은 `--archived` 다. 그래서 `--all` 없이 숨은 아카이브 줄도
    ///   done 이 아니라 아카이브로 센다 — done 으로 세면 꼬리가 대는 `--all` 이 그 줄을 안 낸다.
    ///   잴 때는 부르는 쪽이 건넨 `now` 다
    pub fn hidden_by(&self, i: &Issue, now: &str, wh: &Where) -> Option<Hide> {
        let backlog = crate::report::is_backlog(i) && !self.backlog;
        if !self.archived && wh.archived(i, now) {
            return Some(if backlog { Hide::Unopenable } else { Hide::Archived });
        }
        let deferred = self.deferred.is_none() && !self.all && wh.deferred(i);
        // 묶음은 **읽은 칸**으로 닫혔는지 본다 — 멤버가 남은 에픽을 손으로
        // `done` 에 뒀다고 목록에서 숨기면, 진행 중인 묶음이 사라진다.
        let done = !self.all && self.status.is_empty() && wh.column(i) == crate::config::DONE;
        match (backlog, deferred, done) {
            (false, false, false) => None,
            (true, false, false) => Some(Hide::Backlog),
            (_, true, false) => Some(Hide::Deferred),
            (false, _, true) => Some(Hide::Done),
            _ => Some(Hide::Unopenable),
        }
    }

    /// 기본 목록의 숨김을 다 연 거름망 — done·미룸·생각·아카이브(리뷰 moai-47mz.5il). **좁히는 말은 그대로 든다**
    /// (`-s`·`--deferred`·`--type`…). 숨긴 수를 세는 목록(`cmd::show`)과 숨김 없이 세는 통계
    /// (`report::stats::select`)가 이 하나로 연다 — 저마다 적던 때는 숨김 축이 하나 늘 때마다 두 자리를 다 찾아
    /// 고쳐야 했고, 하나를 빠뜨려도 `..` 가 컴파일을 통과시켰다.
    pub fn unhidden(&self) -> Filter {
        Filter { all: true, backlog: true, archived: true, ..self.clone() }
    }

    pub fn matches(&self, i: &Issue, now: &str, wh: &Where) -> bool {
        // `--deferred` 는 **좁히는 말**이기도 하다 — 미룬 것만 본다. 숨김이
        // 아니라 고르기라 `hidden_by` 에 넣지 않는다.
        if self.deferred.is_some_and(|want| wh.deferred(i) != want) {
            return false;
        }
        if self.hidden_by(i, now, wh).is_some() {
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
        //
        // **쌍둥이 부모 밑에서 소속을 못 정한 줄도 그렇다**(moai-mibi.wpj) — 에픽이 없는 것이 아니라
        // 못 정한 것이라 `-e none` 이 고르면 `twin_parent` 가 댄 줄을 `no_epic` 의 힌트가 또 낸다.
        let eclipsed = wh.eclipsed(i) || wh.epic.lost(i);
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
        // 머문 기간도 **서 있는 칸**의 것이다 (`Where::since`). `-s` 는 읽은 칸으로
        // 고르는데 나이만 적힌 칸의 시각으로 재면, 한 물음의 두 조각이 다른 칸을 본다.
        if let Some(d) = self.stale
            && days_since(wh.since(i), now).is_none_or(|n| n < d)
        {
            return false;
        }
        // 못 읽는 시각은 **어느 폭에도 안 든다** — 손으로 고친 줄의 `updated_at` 이 깨졌으면 그 줄이 그
        // 뒤에 바뀌었는지 모른다. 폭을 안 물었으면(빈 목록) **시각을 풀지도 않는다** — 인자는 부르기 전에
        // 셈해지고(위의 `--milestone` 과 같은 까닭), 탐색기의 거름망은 키 하나에 줄마다 여기를 지난다.
        // 날로 친 끝은 읽는 사람의 벽시계로 견준다([`End::Wall`]) — 시간대가 안 실렸으면(`None`) 도장 그대로다.
        let within = |spans: &[Vec<Span>], at: Option<i64>| spans_hold(spans, at, wh.zone);
        if !self.updated.is_empty() && !within(&self.updated, parse_rfc3339(&i.updated_at)) {
            return false;
        }
        if !self.created.is_empty() && !within(&self.created, parse_rfc3339(&i.created_at)) {
            return false;
        }
        // **끝난 때는 지금 done 에 선 줄에만 있다.** `done_at` 은 되돌려도 남으니(moai-38mh) 그것만 보면
        // 다시 연 줄이 "그 주에 닫힌 것" 으로 선다 — 지금 닫혔는가는 칸이 말한다. 때는 **그 칸에 든 때**
        // ([`Where::since`])로 잰다: done 에 선 줄이면 `done_at` 과 같은 값이고, `done_at` 전에 닫힌 옛
        // 줄에도 있다. 묶음이면 멤버가 마지막으로 done 에 든 때거나 남은 멤버를 미룬 때 가운데 늦은 것이다
        // (`Stand::entered`, moai-23q4). 남은 멤버를 지우거나 빼서 닫힌 묶음은 되짚을 줄이 없어(지운 줄은
        // 없고 뺀 줄에는 흔적이 없다) 끝난 멤버의 때로 선다 — 도움말이 그렇게 댄다.
        if !self.done.is_empty()
            && !within(&self.done, (wh.column(i) == crate::config::DONE).then(|| parse_rfc3339(wh.since(i))).flatten())
        {
            return false;
        }
        // `*_at=` 은 **줄 자신의 그 필드**다(moai-97tn) — 묶음도 멤버로 안 잰다. 없는 필드는 모르는 것이라 어느
        // 폭에도 안 든다(`spans_hold` 가 `None` 을 그렇게 읽는다) — 이 필드 전에 집은 줄을 0분으로 세지 않는 것과
        // 같은 결이다.
        let field = |at: &Option<String>| at.as_deref().and_then(parse_rfc3339);
        if !self.started.is_empty() && !within(&self.started, field(&i.started_at)) {
            return false;
        }
        if !self.done_at.is_empty() && !within(&self.done_at, field(&i.done_at)) {
            return false;
        }
        // **글은 맨 끝에 찾는다**(리뷰 moai-97tn.p44) — 줄의 글과 노트를 다 소문자로 접어 훑는 값비싼 물음이라, 도장
        // 하나를 견주는 때의 폭이 먼저 거른 줄만 훑는다. 갈래는 다 부작용 없는 그리고라 차례가 답을 안 바꾼다.
        if let Some(q) = &self.grep
            && !self.grep_in.hits(i, q)
            && !(self.grep_in.sees_notes() && wh.noted(i, q))
        {
            return false;
        }
        true
    }
}

/// 때 한 폭 — **두 끝이 다 든다**(moai-efoc.ip5). 끝이 없으면 그쪽으로 열렸다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub from: Option<End>,
    pub to: Option<End>,
}

/// 폭의 한 끝 — **무엇과 견주는지가 끝마다 다르다**(moai-efoc, 2026-09-30 사용자 결정).
///
/// - `At` 은 `…T…Z` 로 친 한 순간이다 — UTC epoch 초로 줄의 도장과 곧바로 견준다
/// - `Wall` 은 `YYYY-MM-DD`(날)나 `YYYY-MM-DD HH:MM`(분, moai-97tn)으로 친 끝이다 — **읽는 사람의 벽시계**로 잰
///   epoch 초 꼴이라, 줄의 도장도 그 사람의 시간대로 옮겨(`tz::Zone::local`) 견준다. 사람이 치는 날은 제 날이고,
///   상세가 대는 날짜와 마일스톤 기한(moai-h2th)도 그 날로 선다 — UTC 로 재면 서울의 0~9시에 만든 줄을 화면은
///   그날이라 대는데 `--created <그날>` 에는 안 걸렸다. 시간대는 [`Where::zone`] 이 든다
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    At(i64),
    Wall(i64),
}

impl End {
    /// 이 끝과 견줄 줄의 값과 이 끝의 값 — `utc` 는 줄의 도장, `wall` 은 그것을 읽는 사람의 벽시계로 옮긴 것.
    fn against(self, utc: i64, wall: i64) -> (i64, i64) {
        match self {
            End::At(e) => (utc, e),
            End::Wall(e) => (wall, e),
        }
    }
}

impl Span {
    fn holds(&self, utc: i64, wall: i64) -> bool {
        self.from.is_none_or(|f| {
            let (v, e) = f.against(utc, wall);
            v >= e
        }) && self.to.is_none_or(|t| {
            let (v, e) = t.against(utc, wall);
            v <= e
        })
    }

    /// 읽는 사람의 벽시계로 잰 끝(날·분)이 있는가 — 있으면 시간대를 풀어야 한다([`Filter::needs_zone`]).
    fn walled(&self) -> bool {
        [self.from, self.to].iter().any(|e| matches!(e, Some(End::Wall(_))))
    }

    /// `--since <때>` — 그때부터 열린 폭. `~`·`..` 를 받지 않는다: 한 끝만 받는 플래그다.
    fn since(raw: &str) -> Result<Span, BadFilter> {
        Ok(Span { from: Some(instant(raw, false)?), to: None })
    }

    /// `from~to` 나 `from..to` — 한쪽은 비워도 된다. 가르개 없이 한 때만 주면 **그 꼴이 댄 폭 전체**다 — 날이면
    /// 그 하루, `HH:MM` 이면 그 1분, 순간이면 그 초. `--created 2026-09-15` 를 "그날 만든 것" 말고 달리 읽을 길이
    /// 없다.
    ///
    /// **`~` 를 먼저 본다**(moai-97tn) — 사람이 친 문법이 `~` 이고, `..` 는 옛 CLI 꼴로 남긴 것이다. 둘이 한 값에
    /// 섞이면 `~` 로 가른 쪽이 때로 안 읽혀 거절된다.
    fn parse(raw: &str) -> Result<Span, BadFilter> {
        let (from, to) = match raw.split_once('~').or_else(|| raw.split_once("..")) {
            None => (Some(instant(raw, false)?), Some(instant(raw, true)?)),
            Some((a, b)) => {
                let (a, b) = (a.trim(), b.trim());
                if a.is_empty() && b.is_empty() {
                    return Err(BadFilter::Endless(raw.to_string()));
                }
                let from = (!a.is_empty()).then(|| instant(a, false)).transpose()?;
                let to = (!b.is_empty()).then(|| instant(b, true)).transpose()?;
                (from, to)
            }
        };
        // 거꾸로 선 폭은 늘 0건이다 — 조용히 0건을 내면 "그때는 아무것도 없었다" 와 안 갈린다. 두 끝의 자가
        // 다르면(날과 순간) 시간대를 모르는 여기서는 **하루 안쪽의 어긋남만** 못 가린다 — 시간대가 옮기는 폭은
        // 하루보다 좁으므로, 그보다 더 거꾸로 선 폭은 어느 시간대에서도 0건이라 거절한다. 섞였다고 통째로
        // 넘기던 코드는 `2026-12-01..2026-01-01T00:00:00Z` 를 거절 없이 0건으로 받았다(리뷰 moai-efoc.ln9).
        let backwards = match (from, to) {
            (Some(End::At(f)), Some(End::At(t))) | (Some(End::Wall(f)), Some(End::Wall(t))) => f > t,
            (Some(End::At(f) | End::Wall(f)), Some(End::At(t) | End::Wall(t))) => f > t + 86_400,
            _ => false,
        };
        if backwards {
            return Err(BadFilter::Backwards(raw.to_string()));
        }
        Ok(Span { from, to })
    }
}

/// 때 한 끝 — `YYYY-MM-DDTHH:MM:SSZ` 면 그 순간([`End::At`]), `YYYY-MM-DD` 면 **읽는 사람의 그날**
/// ([`End::Wall`], 2026-09-30 사용자 결정). 처음에는 UTC 의 하루로 정했는데 그 결정이 댄 "마일스톤 기한과
/// 같은 자" 는 moai-h2th 뒤로 참이 아니었다 — 기한과 상세의 날짜는 읽는 사람의 시간대로 선다. 바로잡은
/// 물음에 사람이 다시 골랐다. `YYYY-MM-DD HH:MM` 은 **읽는 사람의 그 1분**이다(moai-97tn, 2026-10-03 사용자
/// 결정) — 날과 같은 벽시계라 시간대를 따로 적을 자리가 없다.
///
/// 여는 끝이면 그 꼴이 댄 폭의 첫 초, 닫는 끝(`end`)이면 마지막 초다 — `..2026-09-15` 가 15일을 통째로,
/// `~2026-10-05 23:59` 가 23:59:59 까지 품는다. 없는 날(`2026-02-30`)·없는 시각(`24:00`)과 부호를 거절한다 —
/// 사람이 이번에 치는 값이라 엄한 자(`parse_date`·`parse_instant`)로 잰다. 파일을 읽는 관대한 자
/// (`parse_rfc3339`)로 재면 오타가 말없이 옆 날로 샌다.
fn instant(raw: &str, end: bool) -> Result<End, BadFilter> {
    let raw = raw.trim();
    if let Some(t) = parse_instant(raw) {
        return Ok(End::At(t));
    }
    if let Some(day) = parse_date(raw) {
        return Ok(End::Wall(day * 86_400 + if end { 86_399 } else { 0 }));
    }
    match wall_minute(raw) {
        Some(m) => Ok(End::Wall(m + if end { 59 } else { 0 })),
        None => Err(BadFilter::NotATime(raw.to_string())),
    }
}

/// `YYYY-MM-DD HH:MM` — 그 1분의 첫 초를 벽시계의 epoch 초 꼴로. 날과 시각 사이는 빈칸이고(여럿이어도 된다),
/// 시와 분은 두 자리다. 초는 안 받는다 — 사람이 치는 꼴은 분까지고, 초가 필요하면 순간(`…T…Z`)이 있다.
///
/// **재는 자는 순간의 것이다**([`parse_instant`], 리뷰 moai-97tn.p44) — 날과 시각을 그 꼴(`…T…:00Z`)로 이어
/// 같은 엄한 자에 건넨다. 따로 재던 때는 시각 자리의 숫자·부호·범위를 두 자가 저마다 적어, 한쪽만 고치는 날
/// `+1:10` 이나 `24:00` 을 한 꼴만 받게 된다. 벽시계의 epoch 초 꼴은 순간의 것과 셈이 같다.
fn wall_minute(raw: &str) -> Option<i64> {
    let (day, clock) = raw.split_once(char::is_whitespace)?;
    parse_instant(&format!("{day}T{}:00Z", clock.trim_start()))
}

/// 때 `at` 이 폭 묶음에 드는가 — 바깥이 그리고, 안쪽이 또는([`Filter::updated`] 의 꼴). 날로 친 끝은 읽는
/// 사람의 벽시계로 견준다([`End::Wall`]) — 시간대가 없으면(`None`) 도장 그대로다. **못 읽은 때(`None`)는
/// 어느 폭에도 안 든다.** 빈 묶음은 늘 든다 — 폭을 안 물은 것이다.
///
/// 줄의 도장([`Filter::matches`])과 저널 줄의 도장([`removed`])이 **이 한 자로** 잰다 — 둘로 두면 같은
/// `--since` 가 목록과 `--removed` 에서 다른 날을 가리키는 날이 온다.
fn spans_hold(spans: &[Vec<Span>], at: Option<i64>, zone: Option<&crate::tz::Zone>) -> bool {
    let Some(t) = at else { return spans.is_empty() };
    // 벽시계는 **줄마다 한 번** 옮긴다(리뷰 moai-efoc.ln9) — 폭마다 옮기면 같은 값을 폭 수만큼 다시 잰다.
    let wall = zone.map_or(t, |z| z.local(t));
    spans.iter().all(|any| any.iter().any(|s| s.holds(t, wall)))
}

/// 지운 이슈의 저널 줄 가운데 `--since` 의 폭에 든 것 — 받은 차례 그대로(moai-7dmq, `moai show --removed`).
///
/// **이력을 늘어놓을 뿐 접지 않는다**(2026-09-30 사용자 결정). 어느 줄이 보드에 서는지를 정하지 않고
/// 스냅샷과 견주지도 않는다 — 지운 뒤 같은 id 가 되살아났어도 그 `rm` 줄은 그대로 선다. 빼려면 저널을
/// 스냅샷에 접어야 하고, 그것이 저널이 상태의 원천이 되는 첫걸음이다.
///
/// 못 읽는 줄을 지운 `rm` 은 이슈를 지운 것이 아니라 빠진다([`JournalEntry::removes_issue`]). 도장을 못
/// 읽는 줄은 `--since` 가 있으면 어느 폭에도 안 든다 — 줄의 `updated_at` 과 같은 약속이다.
pub fn removed<'a>(
    entries: &'a [crate::model::JournalEntry],
    since: &[Vec<Span>],
    zone: Option<&crate::tz::Zone>,
) -> Vec<&'a crate::model::JournalEntry> {
    entries
        .iter()
        .filter(|e| e.removes_issue())
        .filter(|e| since.is_empty() || spans_hold(since, parse_rfc3339(&e.ts), zone))
        .collect()
}

/// 저널의 못 푼 조각이 `--since` 의 폭에 들었을 수 있는가 — `moai show --removed` 가 그 조각을 댈지 가른다
/// (moai-g8ho 리뷰). 조각이 도장을 댔으면 [`removed`] 와 같은 자로 재고, 못 읽었으면 들었을 수 있다.
///
/// **폭 밖의 조각은 대지 않는다.** 온전했어도 이 목록에 안 섰을 줄이라 빠진 것이 없다 — 대면 증분으로 받는
/// 쪽은 오래전에 끊긴 줄 하나로 부를 때마다 실패하고, 도구 안에는 그 줄을 치울 길도 없다. 폭이 없으면 늘 댄다.
pub fn may_fall_in(since: &[Vec<Span>], ts: Option<&str>, zone: Option<&crate::tz::Zone>) -> bool {
    since.is_empty() || ts.and_then(parse_rfc3339).is_none_or(|t| spans_hold(since, Some(t), zone))
}

/// 플래그 되풀이는 그리고, 쉼표는 또는 — `--created a..b,c..d` 는 두 폭 가운데 하나다. `one` 이 한 조각을
/// 폭으로 읽는다(`--since` 는 한 끝만, 나머지는 `from~to`·`from..to`).
///
/// **때가 하나도 없는 값은 거절한다**(moai-efoc 리뷰) — `--since ''`·`--filter since=`·`--created ,` 는 빈
/// 또는-묶음이 되어 어느 줄도 못 지나, 숨김을 다 연 채 말없이 0건을 냈다. 비어 있던 커서 변수로 증분을
/// 받는 쪽은 그것을 "바뀐 것이 없다" 로 읽는다. 끝이 하나도 없는 `..` 를 거절하는 것과 같은 까닭이다 — 태그처럼
/// "거르지 않는다" 로 읽으면 같은 값이 숨긴 줄을 안 연 기본 목록을 내, 그사이 닫힌 줄을 말없이 놓친다.
fn spans(raw: &[String], one: fn(&str) -> Result<Span, BadFilter>) -> Result<Vec<Vec<Span>>, BadFilter> {
    raw.iter()
        .map(|v| {
            let pieces = csv(v);
            if pieces.is_empty() {
                return Err(BadFilter::NoTime);
            }
            pieces.iter().map(|w| one(w)).collect()
        })
        .collect()
}

/// `--filter k=v` 를 플래그와 같은 자리(`Raw`)에 풀어 놓는다. **뜻을 정하지
/// 않는다** — 쪼개고 고르는 일은 `build` 한 곳이 한다.
///
/// **한 번에 한 항목이다.** `;` 로 여럿을 받던 것을 걷어냈다 — 그러면
/// `--filter grep=a;b` 의 `;` 가 글자가 아니라 구분자가 되고, 제목에
/// 세미콜론이 든 이슈를 영영 못 찾는다. 여럿은 플래그를 되풀이한다.
///
/// **항목을 읽는 자는 이것 하나다** — `moai show --removed` 도 `--filter since=…` 를 가려내려고 이것을
/// 부른다(`cmd::show`, moai-7dmq 리뷰). 거기서 따로 쪼개던 때는 `--filter since`(`=` 없음)가 목록과
/// `--removed` 에서 다른 말로 거절됐다.
pub fn desugar(raw: &mut Raw, text: &str) -> Result<(), BadFilter> {
    {
        let one = text.trim();
        if one.is_empty() {
            return Ok(());
        }
        let (k, v) = one.split_once('=').ok_or_else(|| BadFilter::NotAPair(one.to_string()))?;
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
            "type" => raw.kind = Some(v.parse().map_err(BadFilter::NotAKind)?),
            "grep" => raw.grep = Some(v),
            "stale" => raw.stale = Some(v.parse().map_err(|_| BadFilter::NotDays(v))?),
            "since" => raw.since.push(v),
            "created" | "created_at" => raw.created.push(v),
            "done" => raw.done.push(v),
            "updated_at" => raw.updated_at.push(v),
            "started_at" => raw.started_at.push(v),
            "done_at" => raw.done_at.push(v),
            _ => return Err(BadFilter::NoSuchKey(k.to_string())),
        }
    }
    Ok(())
}

/// 거르개 글 한 줄을 `--filter` 항목들로 쪼갠다 — 탐색기의 거름망 칸(`SPC f`)이 친 글을 [`desugar`] 에 넘기기
/// 전에 부른다. CLI 의 `--filter` 는 셸이 이미 쪼개 한 번에 한 항목이라 이것을 안 지난다.
///
/// - **`항목=` 이 시작하는 데서만 쪼갠다.** 그냥 띄어쓰기로 쪼개면 값에 빈칸이 든 것(`grep=원자적 쓰기`,
///   `status=to do`)을 이 칸에서는 아예 적을 수 없다 — CLI 는 그것을 인자 하나로 받으므로, "CLI 와 같은
///   문법" 이라던 약속이 거기서 깨진다. `=` 없는 낱말은 앞 항목의 값에 빈칸 하나로 붙는다
/// - **`=` 바로 뒤의 따옴표(`"…"`·`'…'`)는 한 값이다**(moai-97tn) — 사람이 든 예가
///   `done_at="2026-10-03 00:00~2026-10-05 23:59"` 다. 따옴표는 벗기고, 안의 빈칸과 `=` 는 글자 그대로다
///   (`grep="a=b c"`). 다만 값 양 끝의 빈칸은 [`desugar`] 가 CLI 의 `--filter` 처럼 걷는다 — `grep=" a "` 는
///   `a` 다. **다른 자리의 따옴표는 글자다** — 셸처럼 어디서나 따옴표를 열면 `grep=don't` 의 `'` 가 줄 끝까지를
///   삼킨다
/// - **닫지 않은 따옴표는 줄 끝까지다** — 이 칸은 치는 동안 걸음마다 판정되므로([`desugar`] 의 거절문이 칸 밑에
///   선다) 닫기 전의 반쪽 글을 거절하면 치는 내내 붉다
///
/// 쪼개는 자는 [`items`] 다 — 이것은 그 글만 든다.
pub fn split_items(text: &str) -> Vec<String> {
    items(text).into_iter().map(|it| it.text).collect()
}

/// 거르개 글의 한 항목과 그것이 선 자리 — 자리는 다 바이트다([`items`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 거름망이 받는 글 — 여는 따옴표와 닫는 따옴표를 벗기고, 뒤에 이어 적은 낱말을 빈칸 하나로 붙인 것.
    pub text: String,
    /// 이 항목을 연 낱말의 첫 자리.
    pub start: usize,
    /// 그 낱말의 첫 `=`. 글 맨 앞의 `=` 없는 낱말이 연 항목에는 없다.
    pub eq: Option<usize>,
    /// 연 낱말의 끝 — `=` 바로 뒤의 따옴표로 열었으면 닫는 따옴표를 지나 그 뒤에 붙은 글자까지, 안 닫혔으면 글
    /// 끝이다. **뒤에 이어 적은 낱말은 안 든다** — 그것은 `text` 에만 붙는다.
    pub end: usize,
    /// `=` 바로 뒤의 여는 따옴표와 닫는 따옴표. 안 닫혔으면 닫는 쪽이 없다.
    pub quote: Option<(usize, Option<usize>)>,
}

/// 거르개 글을 항목으로 쪼개며 **항목마다 선 자리를 함께 낸다**(moai-mkyg.dcf). 쪼개는 법은 [`split_items`] 에 적은
/// 그것이다. 거름망이 읽는 글([`split_items`])과 탐색기의 값 안내가 커서 밑의 항목을 찾는 자리(`tui::hint::slot`)가
/// 이 하나를 읽는다 — 한때 안내가 같은 규칙(`=` 가 든 낱말이 항목을 열고 `=` 바로 뒤의 따옴표는 닫힐 때까지)을 따로
/// 적어, 한쪽만 고치면 칸의 안내가 가리키는 항목과 거름망이 읽는 항목이 갈릴 자리였다.
pub fn items(text: &str) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let mut chars = text.char_indices().peekable();
    loop {
        while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}
        let Some(&(start, _)) = chars.peek() else { return out };
        let mut word = String::new();
        let (mut eq, mut quote) = (None, None);
        while let Some((at, c)) = chars.next_if(|(_, c)| !c.is_whitespace()) {
            word.push(c);
            if c == '=' && eq.is_none() {
                eq = Some(at);
                if let Some((open, q)) = chars.next_if(|(_, c)| matches!(c, '"' | '\'')) {
                    let mut close = None;
                    for (at, c) in chars.by_ref() {
                        if c == q {
                            close = Some(at);
                            break;
                        }
                        word.push(c);
                    }
                    quote = Some((open, close));
                }
            }
        }
        // 낱말을 멈춘 글자의 자리가 곧 끝이다 — 안 닫힌 따옴표는 글을 다 먹었으니 글 끝이다.
        let end = chars.peek().map_or(text.len(), |&(at, _)| at);
        match out.last_mut() {
            Some(prev) if eq.is_none() => {
                prev.text.push(' ');
                prev.text.push_str(&word);
            }
            _ => out.push(Item { text: word, start, eq, end, quote }),
        }
    }
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
pub(crate) fn is_assignee(want: &Sel, i: &crate::model::Issue) -> bool {
    // **빈 담당은 없는 것이다**(`Issue::normalize` 와 같은 자) — 읽기는 정규화를 안 거쳐 손으로 푼 머지의
    // `"assignee":""` 가 그대로 온다. `-a none` 과 `ready` 의 `unowned`(`report::owner`)가 이 한 자로 갈린다.
    let Sel::Is(raw) = want else { return i.assignee.as_deref().is_none_or(|a| a.trim().is_empty()) };
    let (name, email) = crate::model::split_assignee(raw);
    let by_name = name.as_deref().is_some_and(|n| i.assignee.as_deref() == Some(n));
    // 메일은 대소문자를 가리지 않는다. 같은 사람이 저장소마다 다르게 적는다.
    let mail = |e: &str| i.assignee_email.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(e));
    // 괄호 없이 준 것은 `split_assignee` 가 이름으로 본다. 메일일 수도 있어 한 번 더 잰다.
    by_name || email.as_deref().is_some_and(&mail) || mail(raw)
}

/// **지금 사람** — 이 줄이 내 것인가를 묻는 한 항(moai-0zjo, 2026-10-02 사용자 결정).
///
/// 가르는 자는 [`is_assignee`] 하나다 — `-a me` 가 고르는 줄과 `ready` 가 "내 것" 으로 내미는 줄과
/// 훅이 초점에 남기는 줄이 같은 자로 갈린다. 이름이나 메일 하나만 맞아도 내 것이고, 메일은
/// 대소문자를 접는다. 자를 따로 세우면 `show -a me` 에 서는 줄을 `ready` 가 남의 것으로 내민다.
///
/// **담당은 줄마다 본다** — 에픽이나 부모에게서 물려받지 않는다. 내 에픽 밑에 남이 맡은 줄은 남의
/// 것이다. 담당 없는 줄도 내 것이 아니다(사용자 결정) — 묻고 집는다.
///
/// 사람을 푸는 일(`model::actor`)은 부르는 쪽 몫이다 — 이 모듈은 순수 함수라 git 설정을 안 연다.
#[derive(Debug, Clone, PartialEq)]
pub struct Me(Sel);

impl Me {
    /// 푼 사람 하나로 짓는다. 담당 칸에 적히는 것과 같은 `이름 (메일)` 한 줄로 든다 — `-a me` 가
    /// 그 줄로 풀리는 것과 같은 자리다(`cmd::resolve_me`).
    pub fn of(a: &crate::model::Actor) -> Me {
        Me::label(&crate::model::label(&a.name, Some(&a.email), crate::config::Naming::Full))
    }

    /// 이미 푼 사람의 한 줄(`이름 (메일)`)로 짓는다 — [`Me::of`] 가 짓는 바로 그 글이다. 탐색기는 사람을 읽는
    /// 스레드에서 그 꼴로 풀어 든다(`tui::Site::me`, moai-oagj.y88).
    pub fn label(label: &str) -> Me {
        Me(Sel::Is(label.to_string()))
    }

    /// 이 줄의 담당이 나인가.
    pub fn owns(&self, i: &crate::model::Issue) -> bool {
        is_assignee(&self.0, i)
    }
}

fn matches_sel(sel: &[Sel], value: Option<&str>) -> bool {
    sel.is_empty()
        || sel.iter().any(|s| match s {
            Sel::Unset => value.is_none(),
            Sel::Is(want) => value == Some(want.as_str()),
        })
}

fn parse_priorities(raw: &[String]) -> Result<Vec<u8>, BadFilter> {
    raw.iter()
        .map(|p| {
            // `p` 한 글자만 벗긴다. `trim_start_matches` 는 `ppp0` 도 받아들인다.
            p.strip_prefix('p')
                .unwrap_or(p.as_str())
                .parse::<u8>()
                .ok()
                .filter(|n| *n <= crate::model::MAX_PRIORITY)
                .ok_or_else(|| BadFilter::NotAPriority(p.clone()))
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
    /// **안 움직이는 차례**(moai-efoc) — id 는 한 번 서면 안 바뀐다. `--after` 로 넘길 때
    /// 앞 쪽을 받는 사이 다른 줄의 우선순위나 칸이 바뀌어도 이 차례는 안 밀린다. 탐색기의
    /// 차례 표(`tui::keys::Order`)에는 없다 — id 는 씨앗 해시(`id::mint`)라 만든 차례도 아니고
    /// 사람이 훑는 화면에서는 뜻이 없다. 쓸모는 쪽을 넘기는 기계의 커서 하나다. 안 움직이는
    /// 차례는 이것과 [`SortKey::Created`] 둘이다 — 생성 차례의 동점도 id 로만 가른다(moai-psyu).
    Id,
}

/// 고른 차례와 그 방향 — `moai show --sort`·`--reverse` 가 드는 한 벌이다(moai-efoc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sort {
    pub key: SortKey,
    pub reversed: bool,
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
/// - **생성·수정만은 같으면 id 로만 가른다**(moai-psyu, 사용자 결정 2026-10-02). 계획 하나가 한 초에
///   서서 같은 초를 나눈 줄이 흔하고(이 저장소 1561 중 859), 우선순위로 가르면 쪽을 넘기는 사이 우선순위를
///   고친 줄이 커서([`page`])를 넘는다. id 는 씨앗 해시라 같은 초 안의 차례가 만든 차례는 아니지만,
///   한 번 서면 안 바뀐다
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
        SortKey::Id => a.0.id.cmp(&b.0.id),
    };
    let order = natural.then_with(|| match key {
        SortKey::Created | SortKey::Updated => a.0.id.cmp(&b.0.id),
        _ => display_order(a.0, b.0),
    });
    if reversed { order.reverse() } else { order }
}

/// 고른 줄을 `sort` 차례로 세우고, 커서(`after`) **뒤**에 선 줄부터 `limit` 줄만 남긴다(moai-efoc) —
/// `moai show` 의 목록이 지나는 자리다. 돌려주는 것은 `limit` 에 잘려 나간 줄 수다 — 사람 화면이
/// "N건 더" 를 댄다.
///
/// 칸은 **목록의 글리프와 같은 자**([`Where::column`])로 읽는다 — 묶음의 칸은 멤버에서 읽은 것이다.
/// 담당은 화면에 선 이름으로 견준다(`cfg.naming`). 탐색기와 같은 [`order_by`] 를 지나므로 같은 낱말이
/// 두 표면에서 같은 차례로 선다. 기본값(`Sort::default`)은 [`display_order`] 와 한 치도 안 갈린다.
///
/// **커서는 자리가 아니라 값이다**(키셋). 커서 줄의 **지금** 값으로 견주어 그보다 뒤인 줄만 남기므로,
/// 앞 쪽을 받은 뒤 다른 줄이 생기거나 지워져도 밀리거나 겹치지 않는다 — offset 으로 넘기면 세션
/// 여럿이 쓰는 사이 줄이 밀려 빠지거나 겹친다(에픽 본문의 결정). 커서 줄은 걸러져 목록에 없어도
/// 된다 — 닫혀 숨었어도 값은 있다. **쪽 사이에 차례 안의 자리가 바뀐 줄은 커서 줄이든 다른 줄이든
/// 커서를 넘어 겹치거나 빠진다**(moai-efoc 리뷰). 아무 고침에도 안 움직이는 차례는 [`SortKey::Id`] 와
/// [`SortKey::Created`] 다 — 생성 때도 id 도 한 번 서면 안 바뀌고, 생성 차례의 동점은 id 로만
/// 가른다(moai-psyu). `Updated` 는 동점을 같이 가르지만 고칠 때마다 제 값이 바뀌어 움직인다.
///
/// **한 id 의 줄은 한 덩어리다**(moai-efoc 리뷰). 머지가 남긴 쌍둥이는 값이 달라 차례에서 떨어져 설 수
/// 있는데 커서는 id 하나라 어느 줄에서 끊겼는지 모른다 — 줄 하나(`Load::get` 의 뒷줄)로 넘던 때는 사이의
/// 줄을 건너뛰거나 같은 쪽을 끝없이 되받았다. 그래서 id 마다 **머리 줄**(그 id 의 줄 가운데 이 차례에서 맨
/// 앞에 서는 것)을 세우고 쌍둥이를 그 자리에 모아 세운다. 커서도 머리로 넘어 그 id 의 줄은 다 앞 쪽에 선
/// 것으로 치고, `limit` 은 한 id 의 줄을 가르지 않는다 — 가르느니 그 쪽을 늘린다(줄이면 받는 쪽이 짧은
/// 쪽을 "끝났다" 로 읽는다). 머리는 **걸러지기 전의 줄 전부**(`all`)에서 고른다 — 걸러진 줄로 고르면 쪽마다
/// 머리가 달라질 수 있다. 쌍둥이가 없는 목록에서는 머리가 곧 그 줄이라 아무것도 안 바뀐다.
///
/// 견주는 자가 둘이 아니다 — 세우는 것과 커서를 넘는 것이 같은 `cmp` 를 지나야, 커서 줄 바로 뒤의
/// 줄이 두 판정 사이에서 갈리지 않는다.
pub fn page(
    shown: &mut Vec<Issue>,
    all: &[Issue],
    wh: &Where,
    cfg: &crate::config::Config,
    sort: Sort,
    after: Option<&Issue>,
    limit: Option<usize>,
) -> usize {
    use std::cmp::Ordering;
    let cmp = |a: &Issue, b: &Issue| {
        order_by(sort.key, sort.reversed, (a, wh.column(a)), (b, wh.column(b)), &cfg.statuses, cfg.naming)
    };
    let mut heads: BTreeMap<&str, &Issue> = BTreeMap::new();
    for i in all {
        if !heads.get(i.id.as_str()).is_some_and(|h| cmp(h, i) != Ordering::Greater) {
            heads.insert(i.id.as_str(), i);
        }
    }
    shown.sort_by(|a, b| cmp(head_of(&heads, a), head_of(&heads, b)).then_with(|| cmp(a, b)));
    if let Some(c) = after {
        let c = head_of(&heads, c);
        shown.retain(|i| cmp(head_of(&heads, i), c) == Ordering::Greater);
    }
    let Some(n) = limit else { return 0 };
    // 끊는 자리가 한 id 의 줄 사이면 그 id 의 남은 줄까지 이 쪽에 싣는다.
    let mut end = n;
    while end > 0 && end < shown.len() && shown[end].id == shown[end - 1].id {
        end += 1;
    }
    cut(shown, Some(end))
}

/// 그 줄의 id 의 머리 줄([`page`]). 머리를 모르는 줄(`all` 밖에서 온 줄)은 제가 제 머리다.
fn head_of<'x>(heads: &BTreeMap<&str, &'x Issue>, i: &'x Issue) -> &'x Issue {
    heads.get(i.id.as_str()).copied().unwrap_or(i)
}

/// 앞에서 `limit` 줄만 남기고 잘린 수를 돌려준다 — `moai show` 의 쪽([`page`])과 `moai ready -n` 이 같은
/// 자로 자른다(moai-efoc 리뷰: 두 벌이던 자리다). 차례는 부르는 쪽이 이미 세웠다.
pub fn cut<T>(rows: &mut Vec<T>, limit: Option<usize>) -> usize {
    match limit {
        Some(n) if rows.len() > n => {
            let gone = rows.len() - n;
            rows.truncate(n);
            gone
        }
        _ => 0,
    }
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
        // **id 차례는 우선순위를 안 본다**(moai-efoc) — 급한 a-2 가 앞으로 나오면 안 움직이는 차례가 아니다.
        assert_eq!(sorted(SortKey::Id, false), ["a-1", "a-2", "a-3"], "id 차례에 우선순위가 끼었다");
        assert_eq!(sorted(SortKey::Id, true), ["a-3", "a-2", "a-1"]);

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
            plain.hidden_by(i, NOW, &Where::of(&all, &cfg()))
        };
        let mut thought = issue("a-0001", "todo", &[]);
        thought.kind = Kind::Backlog;
        let mut closed_thought = thought.clone();
        closed_thought.status = Status::new("done");
        let mut shelved = issue("a-0002", "todo", &[]);
        shelved.deferred_at = Some(NOW.into());
        let mut shelved_done = shelved.clone();
        shelved_done.status = Status::new("done");

        assert_eq!(why(&issue("a-0003", "todo", &[])), None);
        assert_eq!(why(&issue("a-0003", "done", &[])), Some(Hide::Done));
        assert_eq!(why(&thought), Some(Hide::Backlog));
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

    /// done 칸에 그때 든 줄.
    fn closed_at(id: &str, at: &str) -> Issue {
        let mut i = issue(id, "done", &[]);
        i.status_since = at.into();
        i.done_at = Some(at.into());
        i
    }

    /// **아카이브는 done 안의 한 겹 더고, 그것을 여는 한 낱말은 `--archived` 다**(moai-47mz, 2026-10-03 사용자
    /// 결정). done 을 연 목록(`--all`·`-s done`)에서도 done 에 든 지 14일이 꼬박 찬 줄은 빠진다. 글로 찾거나
    /// 때로 물으면 열리고, `-g` 는 done 은 그대로 숨긴다.
    #[test]
    fn archive_hides_aged_done_rows_until_archived_opens_them() {
        let why = |f: &Filter, i: &Issue| {
            let all = [i.clone()];
            f.hidden_by(i, NOW, &Where::of(&all, &cfg()))
        };
        let build = |raw: Raw| Filter::build(raw).unwrap();
        // NOW 는 09-11 이다 — 꼬박 14일 앞과 그보다 한 초 뒤.
        let aged = closed_at("a-0001", "2026-08-28T00:00:00Z");
        let fresh = closed_at("a-0002", "2026-08-28T00:00:01Z");

        let plain = Filter::default();
        assert_eq!(why(&plain, &aged), Some(Hide::Archived), "done 으로 세면 꼬리가 대는 --all 이 그 줄을 안 낸다");
        assert_eq!(why(&plain, &fresh), Some(Hide::Done));
        let all = build(Raw { all: true, ..Raw::default() });
        assert_eq!((why(&all, &aged), why(&all, &fresh)), (Some(Hide::Archived), None), "--all 이 아카이브를 열었다");
        let done_col = build(Raw { status: s(&["done"]), ..Raw::default() });
        assert_eq!((why(&done_col, &aged), why(&done_col, &fresh)), (Some(Hide::Archived), None));
        assert!(!hit(&all, &aged) && hit(&all, &fresh), "matches 가 hidden_by 와 갈렸다");

        let archived = build(Raw { archived: true, ..Raw::default() });
        assert_eq!((why(&archived, &aged), why(&archived, &fresh)), (None, None), "--archived 는 done 까지 연다");
        let grep = build(Raw { grep: Some("제목".into()), ..Raw::default() });
        assert_eq!(why(&grep, &aged), Some(Hide::Done), "-g 가 done 까지 열었거나 아카이브를 안 열었다");
        let grep_all = build(Raw { grep: Some("제목".into()), all: true, ..Raw::default() });
        assert_eq!(why(&grep_all, &aged), None, "-g --all 이 아카이브를 못 찾는다");
        for timed in [
            Raw { done: s(&["2026-08-01..2026-09-30"]), ..Raw::default() },
            Raw { since: s(&["2026-08-01"]), ..Raw::default() },
        ] {
            assert_eq!(why(&build(timed), &aged), None, "때로 물은 목록이 아카이브를 숨겼다");
        }
        // `--stale` 도 때로 묻는 말이다(리뷰 moai-47mz.5il) — `-s done --stale 14` 가 늘 0건이던 자리다. done 은
        // 여느 때처럼 `-s done` 이 연다.
        let stale = build(Raw { status: s(&["done"]), stale: Some(14), ..Raw::default() });
        assert!(hit(&stale, &aged), "-s done --stale 이 아카이브를 못 고른다");
        assert_eq!(why(&build(Raw { stale: Some(14), ..Raw::default() }), &aged), Some(Hide::Done));

        // 다시 연 줄은 `done_at` 이 남아도 done 이 아니다.
        let mut reopened = aged.clone();
        reopened.status = Status::new("todo");
        assert_eq!(why(&plain, &reopened), None);
        // 못 읽는 시각은 아카이브가 아니다 — 숨기면 손으로 고친 줄이 말없이 사라진다.
        let mut unreadable = aged.clone();
        unreadable.status_since = "언젠가".into();
        assert_eq!(why(&all, &unreadable), None);
        // 닫은 생각은 아카이브여도 한 낱말로 안 열린다 — `--type backlog --archived` 둘이 든다.
        let mut thought = aged.clone();
        thought.kind = Kind::Backlog;
        assert_eq!(why(&archived, &thought), Some(Hide::Backlog));
        assert_eq!(why(&all, &thought), Some(Hide::Unopenable));
        let backlog = build(Raw { kind: Some(Kind::Backlog), all: true, ..Raw::default() });
        assert_eq!(why(&backlog, &thought), Some(Hide::Archived), "닫힌 backlog 도 같은 규칙이다");

        // `archive_days = 0` 이면 아카이브가 없다.
        let off = crate::config::Config::parse("prefix = \"argos\"\narchive_days = 0\n").unwrap();
        let one = [aged.clone()];
        assert_eq!(all.hidden_by(&aged, NOW, &Where::of(&one, &off)), None);
    }

    /// **묶음은 멤버가 마지막으로 done 에 든 때로 잰다**(moai-47mz, 사용자 결정) — 멤버 하나라도 최근에 끝났으면
    /// 묶음은 안 숨는다. 묶음 제 줄에 손으로 친 `done` 과 그 시각은 안 본다.
    #[test]
    fn a_group_ages_by_its_last_member() {
        let mut epic = issue("a-0010", "done", &[]);
        epic.kind = Kind::Epic;
        epic.status_since = "2026-01-01T00:00:00Z".into();
        let member = |id: &str, at: &str| {
            let mut i = closed_at(id, at);
            i.epic = Some("a-0010".into());
            i
        };
        let old = member("a-0011", "2026-08-01T00:00:00Z");
        let late = member("a-0012", "2026-09-10T00:00:00Z");
        let (both, c) = ([epic.clone(), old.clone(), late.clone()], cfg());
        let wh = Where::of(&both, &c);
        assert!(!wh.archived(&epic, NOW), "최근에 끝난 멤버가 있는데 묶음이 숨었다");
        assert!(wh.archived(&old, NOW) && !wh.archived(&late, NOW));
        let aged = [epic.clone(), old.clone(), member("a-0012", "2026-08-02T00:00:00Z")];
        assert!(Where::of(&aged, &cfg()).archived(&epic, NOW), "멤버가 다 오래 끝난 묶음이 안 숨는다");
        // 멤버가 남은 묶음은 제 줄을 손으로 done 에 뒀어도 done 이 아니다.
        let mut open = late.clone();
        open.status = Status::new("todo");
        assert!(!Where::of(&[epic.clone(), old, open], &cfg()).archived(&epic, NOW));
    }

    /// **남은 멤버를 미뤄 닫은 묶음은 미룬 때 done 에 든다**(moai-23q4, 2026-10-03 사용자 결정). 끝난 멤버가
    /// 오래전에 끝났어도 그날로 아카이브에 숨지 않고, `--done` 도 그날로 잰다 — 둘은 한 시계다([`Where::since`]).
    /// 멤버가 빠진 때는 그것을 뺀 미룸 가운데 **가장 이른** 것이다: 오래전에 미룬 부모 밑의 자식을 오늘 또
    /// 미뤄도 묶음의 칸은 오늘 안 바뀌었다.
    #[test]
    fn a_group_closed_by_deferring_enters_done_when_deferred() {
        let mut epic = issue("a-0010", "todo", &[]);
        epic.kind = Kind::Epic;
        let deferred = |id: &str, at: &str| {
            let mut i = issue(id, "todo", &[]);
            i.deferred_at = Some(at.into());
            i
        };
        let in_epic = |mut i: Issue| {
            i.epic = Some("a-0010".into());
            i
        };
        let old = in_epic(closed_at("a-0011", "2026-08-01T00:00:00Z"));
        let c = cfg();
        let today = [epic.clone(), old.clone(), in_epic(deferred("a-0012", "2026-09-10T00:00:00Z"))];
        let wh = Where::of(&today, &c);
        assert_eq!((wh.column(&epic), wh.since(&epic)), ("done", "2026-09-10T00:00:00Z"));
        assert!(!wh.archived(&epic, NOW), "미뤄 어제 닫힌 묶음이 그날로 숨었다");
        let done = |span: &str| Filter::build(Raw { done: s(&[span]), ..Raw::default() }).unwrap();
        assert!(done("2026-09-10").matches(&epic, NOW, &wh), "`--done` 이 미룬 날을 안 잡았다");
        assert!(!done("2026-08-01").matches(&epic, NOW, &wh), "`--done` 이 끝난 멤버의 날로 쟀다");
        // 끝난 멤버보다 먼저 미뤘으면 그 멤버가 끝난 때다.
        let early = [epic.clone(), old.clone(), in_epic(deferred("a-0012", "2026-07-01T00:00:00Z"))];
        assert_eq!(Where::of(&early, &c).since(&epic), "2026-08-01T00:00:00Z");
        // 자식은 부모에게서 에픽을 받는다 — 부모를 미룬 때(08-05)부터 셈 밖이었다.
        let nested = [
            epic.clone(),
            old,
            in_epic(deferred("a-0013", "2026-08-05T00:00:00Z")),
            deferred("a-0013.abc", "2026-09-10T00:00:00Z"),
        ];
        let wh = Where::of(&nested, &c);
        assert_eq!(wh.since(&epic), "2026-08-05T00:00:00Z", "자식을 늦게 또 미룬 때로 쟀다");
        assert!(wh.archived(&epic, NOW));
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
        assert_eq!(e, BadFilter::Twice { field: Once::Status, a: "todo".into(), b: "review".into(), rest: vec![] });
        // 셋째부터도 든다 — 고칠 글이 그것을 빼면 그대로 친 사람이 그 줄을 잃는다.
        let e = Filter::build(Raw { status: s(&["todo", "review", "done"]), all: true, ..Raw::default() }).unwrap_err();
        assert_eq!(
            e,
            BadFilter::Twice { field: Once::Status, a: "todo".into(), b: "review".into(), rest: vec!["done".into()] }
        );
        // 고쳐 칠 명령(`-s todo,review`)은 글을 펴는 쪽이 갈래마다 잰다
        // (`view::tests::a_bad_filter_speaks_the_language_it_is_handed`).
    }

    /// **`Once::key` 는 `desugar` 가 그 거르개로 읽는 낱말이다**(moai-tckz). 거름망의 거절문이 이 낱말로 고쳐 칠
    /// 글을 짓는다 — 둘이 어긋나면 `parent=a,b` 를 대고 그 글이 우선순위나 모르는 항목으로 읽힌다.
    #[test]
    fn each_once_key_reads_back_as_its_own_filter() {
        use Once::*;
        for field in [Status, Epic, Milestone, Parent, Priority, Grep, Kind, Stale] {
            let pairs = [format!("{}=1", field.key()), format!("{}=2", field.key())];
            let e = Filter::build(Raw { filter: pairs.to_vec(), ..Raw::default() }).unwrap_err();
            assert_eq!(e, BadFilter::Twice { field, a: "1".into(), b: "2".into(), rest: vec![] }, "{field:?}");
        }
    }

    /// **값 하나만 드는 셋도 두 번 주면 거절한다**(moai-ltsv.auf, 사람이 정했다) — 한때 뒤의 값이 앞의 값을 말없이
    /// 덮어 `--filter grep=one --filter grep=two` 가 two 의 줄만 냈다. 플래그와 `--filter` 를 섞어도, 셋째부터는
    /// `rest` 에 든다. `grep` 을 그리고로 읽지 않는 것도 그날 정했다.
    #[test]
    fn a_single_value_filter_refuses_a_second_value() {
        let twice = |field, a: &str, b: &str, rest: &[&str]| BadFilter::Twice {
            field,
            a: a.into(),
            b: b.into(),
            rest: rest.iter().map(|r| r.to_string()).collect(),
        };
        let e = Filter::build(Raw { filter: s(&["grep=one", "grep=two"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice(Once::Grep, "one", "two", &[]));
        // 플래그가 앞이다 — `-g a --filter grep=b`.
        let e = Filter::build(Raw { grep: Some("a".into()), filter: s(&["grep=b"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice(Once::Grep, "a", "b", &[]));
        let e = Filter::build(Raw { grep: Some("a".into()), filter: s(&["grep=b", " grep = c "]), ..Raw::default() })
            .unwrap_err();
        assert_eq!(e, twice(Once::Grep, "a", "b", &["c"]));
        // 종류는 준 글 그대로 — 플래그로 온 것은 `Kind` 의 이름으로 돌아온다.
        let e = Filter::build(Raw { kind: Some(Kind::Epic), filter: s(&["type=issue"]), ..Raw::default() })
            .unwrap_err();
        assert_eq!(e, twice(Once::Kind, "epic", "issue", &[]));
        let e = Filter::build(Raw { filter: s(&["type=epic", "type=issue", "type=backlog"]), ..Raw::default() })
            .unwrap_err();
        assert_eq!(e, twice(Once::Kind, "epic", "issue", &["backlog"]));
        let e = Filter::build(Raw { stale: Some(3), filter: s(&["stale=7"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice(Once::Stale, "3", "7", &[]));
        let e = Filter::build(Raw { filter: s(&["stale=3", "stale=7", "stale=9"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice(Once::Stale, "3", "7", &["9"]));
        // 하나씩이면 그대로 선다 — 플래그만, 거르개 글만, 서로 다른 셋을 하나씩.
        let f = Filter::build(Raw { grep: Some("A".into()), filter: s(&["type=epic", "stale=2"]), ..Raw::default() })
            .unwrap();
        assert_eq!((f.grep.as_deref(), f.kind, f.stale), (Some("a"), Some(Kind::Epic), Some(2)));
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
                assert_eq!(hit(&f, &i), g == only, "{g:?} 범위가 {q} 에 틀렸다");
            }
        }
        // 차례는 전체 → id → 제목 → 태그 → 본문 → 노트 → 전체, 거꾸로도 돈다. 노트 범위가 제 글을 찾는 것은
        // 노트를 실어야 재므로 `grep_sees_the_notes_it_is_handed_in_the_whole_and_note_scopes` 가 본다.
        let mut g = GrepIn::All;
        for want in [GrepIn::Id, GrepIn::Title, GrepIn::Tag, GrepIn::Body, GrepIn::Note, GrepIn::All] {
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
        let twice = BadFilter::Twice { field: Once::Status, a: "todo".into(), b: "review".into(), rest: vec![] };
        let e =
            Filter::build(Raw { status: s(&["todo"]), filter: s(&["status=review"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice);
        let e = Filter::build(Raw { filter: s(&["status=todo", "status=review"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, twice);

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
        assert_eq!(e, BadFilter::NoSuchKey("statu".into()));
        let e = Filter::build(Raw { filter: s(&["todo"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, BadFilter::NotAPair("todo".into()));
        // 있는 항목의 목록은 글을 펴는 쪽(`view::bad_filter`)이 이것에서 읽는다 — **대는 항목은 다 받아야 한다.**
        // 목록과 `desugar` 의 갈래는 손으로 적는 두 벌이라, 목록에만 남은 항목은 거절문이 대고 곧바로 거절한다.
        for k in KEYS {
            let said = desugar(&mut Raw::default(), &format!("{k}=1"));
            assert_ne!(said, Err(BadFilter::NoSuchKey((*k).to_string())), "`{k}` 를 대면서 안 받는다");
        }
    }

    #[test]
    fn priorities_accept_both_spellings() {
        let f = Filter::build(Raw { priority: s(&["p0,1"]), all: true, ..Raw::default() }).unwrap();
        assert_eq!(f.priority, [0, 1]);
        for bad in ["9", "ppp0"] {
            let e = Filter::build(Raw { priority: s(&[bad]), all: true, ..Raw::default() }).unwrap_err();
            assert_eq!(e, BadFilter::NotAPriority(bad.into()), "{bad}");
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

    /// 메일에 괄호가 든 사람도 제 줄을 제 것으로 읽는다(moai-v4p4.6w1) — [`Me`] 는 지금 사람을 `이름 (메일)` 한
    /// 줄로 들고 [`is_assignee`] 가 그 줄을 되가른다. 줄의 이름은 옛 이름이라 메일로만 갈린다.
    #[test]
    fn me_with_brackets_in_the_email_owns_its_rows() {
        let mut i = issue("a-0001", "todo", &[]);
        (i.assignee, i.assignee_email) = (Some("옛 이름".into()), Some("a(b)@x.io".into()));
        let me = Me::of(&crate::model::Actor { name: "레이븐".into(), email: "a(b)@x.io".into() });
        assert!(me.owns(&i), "제 메일이 든 줄을 남의 것으로 읽었다");
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

    /// **담당은 되풀이도 또는이다**(moai-97tn, 2026-10-03 사용자 결정) — 한때 두 번 쓰면 거절했다. 한 줄의 담당은
    /// 하나라 그리고는 늘 0건이었고, 사람을 여럿 고르는 글(`assignee=a assignee=b`)은 되풀이로 쓰는 것이 자연스럽다.
    /// 쉼표·되풀이·거르개 글이 한 묶음으로 쌓인다.
    #[test]
    fn repeating_assignee_is_or() {
        let who = |name: &str, email: &str| {
            let mut i = issue("a-0001", "todo", &[]);
            (i.assignee, i.assignee_email) = (Some(name.into()), Some(email.into()));
            i
        };
        let (cs, yh, mj) = (who("철수", "cs@x.io"), who("영희", "yh@x.io"), who("민지", "mj@x.io"));
        for raw in [
            Raw { assignee: s(&["철수", "yh@x.io"]), ..Raw::default() },
            Raw { filter: s(&["assignee=철수", "assignee=yh@x.io"]), ..Raw::default() },
            Raw { assignee: s(&["철수"]), filter: s(&["assignee=영희 (yh@x.io)"]), ..Raw::default() },
            Raw { assignee: s(&["철수,yh@x.io"]), ..Raw::default() },
        ] {
            let said = format!("{raw:?}");
            let f = Filter::build(Raw { all: true, ..raw }).unwrap();
            assert!(hit(&f, &cs) && hit(&f, &yh), "둘 가운데 하나를 놓쳤다 — {said}");
            assert!(!hit(&f, &mj), "안 고른 사람이 걸렸다 — {said}");
        }
    }

    /// **때의 폭은 두 끝을 다 품고, 날은 하루를 통째로 품는다**(moai-efoc.ip5). 닫는 끝의 날짜는 그날을 통째로
    /// 품는다 — 안 품으면 `..2026-09-03` 이 3일 0시에서 끊겨 그날 한 일이 빠진다. 여기서는 시간대 없이(UTC)
    /// 잰다 — 날이 읽는 사람의 날이라는 것은 `a_day_is_the_readers_day_and_an_instant_is_utc` 가 잰다.
    #[test]
    fn a_time_span_holds_both_ends_and_a_day_is_whole() {
        let t = |s: &str| parse_rfc3339(s).unwrap();
        // 시간대 없이(UTC) 잰다 — 벽시계가 도장과 같다.
        let at = |span: &Span, s: &str| span.holds(t(s), t(s));
        let range = Span::parse("2026-09-02..2026-09-03").unwrap();
        assert!(at(&range, "2026-09-02T00:00:00Z") && at(&range, "2026-09-03T23:59:59Z"), "끝을 안 품었다");
        assert!(!at(&range, "2026-09-01T23:59:59Z") && !at(&range, "2026-09-04T00:00:00Z"), "폭 밖을 품었다");
        let one = Span::parse("2026-09-02").unwrap();
        assert!(at(&one, "2026-09-02T12:00:00Z") && !at(&one, "2026-09-03T00:00:00Z"), "한 날이 하루가 아니다");
        assert_eq!(Span::parse("..2026-09-02").unwrap().from, None, "빈 앞끝이 열리지 않았다");
        assert_eq!(
            Span::parse("2026-09-02T01:02:03Z..").unwrap(),
            Span { from: Some(End::At(t("2026-09-02T01:02:03Z"))), to: None }
        );
        // 조용히 0건을 내는 대신 거절한다 — 오타와 "그때는 없었다" 가 안 갈린다. 날과 순간이 섞인 폭도 하루를
        // 넘게 거꾸로 섰으면 어느 시간대에서도 0건이다(리뷰 moai-efoc.ln9).
        for bad in [
            "..",
            "2026-02-30",
            "어제",
            "2026-9-2",
            "2026-09-03..2026-09-02",
            "2026-12-01..2026-01-01T00:00:00Z",
            "2026-12-01T00:00:00Z..2026-01-01",
        ] {
            assert!(Span::parse(bad).is_err(), "{bad} 를 받았다");
        }
        assert!(Span::since("2026-09-02..").is_err(), "--since 가 폭을 받았다");
        assert!(Span::since("2026-09-02").is_ok());
        // 치는 시각도 날짜처럼 엄하다 — 없는 날·부호·`:60` 을 옆 날로 넘기지 않고 거절한다(moai-efoc 리뷰).
        for bad in [
            "2026-02-30T00:00:00Z",
            "2026-09-31T00:00:00Z..",
            "+026-09-02T00:00:00Z..",
            "2026-09-02T-1:00:00Z..",
            "..2026-02-28T23:59:60Z",
        ] {
            assert!(Span::parse(bad).is_err(), "{bad} 를 받았다");
        }
        assert!(Span::since("2026-02-30T00:00:00Z").is_err(), "--since 가 없는 날을 받았다");
    }

    /// **날로 친 때는 읽는 사람의 날이고, 시각으로 친 때는 그 순간이다**(moai-efoc, 2026-09-30 사용자 결정) —
    /// 서울의 10-01 05:00 은 UTC 로 09-30 20:00 이다. 화면은 그 줄을 10-01 에 만든 것으로 대므로
    /// `--created 2026-10-01` 에 걸려야 한다. `…Z` 로 친 끝은 시간대와 무관하다.
    #[test]
    fn a_day_is_the_readers_day_and_an_instant_is_utc() {
        let mut dawn = issue("a-0001", "todo", &[]);
        dawn.created_at = "2026-09-30T20:00:00Z".into();
        let all = vec![dawn];
        let c = cfg();
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let pick = |zone: Option<&crate::tz::Zone>, created: &str| {
            let f = Filter::build(Raw { created: s(&[created]), ..Raw::default() }).unwrap();
            let mut wh = Where::of(&all, &c);
            wh.zone = zone;
            all.iter().any(|i| f.matches(i, NOW, &wh))
        };
        assert!(pick(Some(&seoul), "2026-10-01"), "서울의 그날 만든 줄이 그날에 안 걸렸다");
        assert!(!pick(Some(&seoul), "2026-09-30"), "서울에서 전날에 걸렸다");
        assert!(pick(None, "2026-09-30") && !pick(None, "2026-10-01"), "시간대 없이는 UTC 의 날이다");
        // 순간으로 친 끝은 시간대가 안 옮긴다.
        assert!(pick(Some(&seoul), "2026-09-30T20:00:00Z"), "시각 끝을 시간대로 옮겼다");
        assert!(!pick(Some(&seoul), "2026-09-30T20:00:01Z.."), "시각 끝을 시간대로 옮겼다");
        // 날과 순간이 섞인 폭은 **하루 안쪽이면 받는다** — UTC 로는 거꾸로 서도 서울에서는 그 줄을 품는다.
        assert!(pick(Some(&seoul), "2026-10-01..2026-09-30T20:00:00Z"), "하루 안쪽의 섞인 폭을 거절했다");
        // 날로 친 끝이 있을 때만 시간대를 푼다.
        let needs = |raw: Raw| Filter::build(raw).unwrap().needs_zone();
        assert!(needs(Raw { since: s(&["2026-10-01"]), ..Raw::default() }));
        assert!(!needs(Raw { since: s(&["2026-10-01T00:00:00Z"]), ..Raw::default() }));
        assert!(!needs(Raw::default()));
        // 세 거르개가 다 묻는다 — 하나라도 빠지면 그 거르개만 말없이 UTC 의 날로 잰다(리뷰 moai-efoc.ln9).
        assert!(needs(Raw { created: s(&["2026-09-30T00:00:00Z..2026-10-01"]), ..Raw::default() }));
        assert!(needs(Raw { done: s(&["2026-10-01"]), ..Raw::default() }));
        // 새 키도 묻는다(moai-97tn) — 분까지 친 때도 날과 같은 벽시계다.
        for item in
            ["started_at=2026-10-01", "done_at=2026-10-01 10:10", "updated_at=~2026-10-01", "created_at=2026-10-01"]
        {
            assert!(needs(Raw { filter: s(&[item]), ..Raw::default() }), "{item} 가 시간대를 안 물었다");
        }
        assert!(!needs(Raw { filter: s(&["done_at=2026-10-01T00:00:00Z~"]), ..Raw::default() }));
    }

    /// **`HH:MM` 은 그 1분이고, `~` 는 `..` 와 같은 폭이다**(moai-97tn, 2026-10-03 사용자 결정). 한 때만 주면 그 꼴이
    /// 댄 폭 전체다 — 날은 하루, 분은 1분. 닫는 끝도 그 꼴의 마지막 초까지라 사람이 든 `00:00~23:59` 가 23:59:59 를
    /// 품는다. 여기서는 시간대 없이(UTC) 잰다 — 분이 읽는 사람의 시계라는 것은 아래
    /// `the_peoples_examples_read_on_their_own_clock` 가 잰다.
    #[test]
    fn a_minute_is_whole_and_a_tilde_is_a_range() {
        let t = |s: &str| parse_rfc3339(s).unwrap();
        let at = |span: &Span, s: &str| span.holds(t(s), t(s));
        let minute = Span::parse("2026-10-10 10:10").unwrap();
        assert!(at(&minute, "2026-10-10T10:10:00Z") && at(&minute, "2026-10-10T10:10:59Z"), "1분을 통째로 안 품었다");
        assert!(!at(&minute, "2026-10-10T10:09:59Z") && !at(&minute, "2026-10-10T10:11:00Z"), "1분 밖을 품었다");
        assert!(minute.walled(), "분이 읽는 사람의 시계가 아니다");
        let range = Span::parse("2026-10-03 00:00~2026-10-05 23:59").unwrap();
        assert!(at(&range, "2026-10-03T00:00:00Z") && at(&range, "2026-10-05T23:59:59Z"), "끝을 안 품었다");
        assert!(!at(&range, "2026-10-02T23:59:59Z") && !at(&range, "2026-10-06T00:00:00Z"), "폭 밖을 품었다");
        // `~` 와 `..` 는 같은 폭이고, 날과 분과 순간을 섞어도 된다. 빈칸은 끝마다 걷는다.
        assert_eq!(Span::parse("2026-10-03~2026-10-05").unwrap(), Span::parse("2026-10-03..2026-10-05").unwrap());
        assert_eq!(Span::parse(" 2026-10-03  ~  2026-10-05 ").unwrap(), Span::parse("2026-10-03~2026-10-05").unwrap());
        assert_eq!(Span::parse("2026-10-10   10:10").unwrap(), minute, "날과 시각 사이의 빈칸 여럿을 못 읽었다");
        assert!(Span::parse("2026-10-03 09:00~2026-10-03T12:00:00Z").is_ok());
        // 한쪽은 비워도 된다 — `..` 와 같다.
        assert_eq!(Span::parse("2026-10-03~").unwrap().to, None, "빈 뒤끝이 열리지 않았다");
        assert_eq!(Span::parse("~2026-10-05 23:59").unwrap().from, None, "빈 앞끝이 열리지 않았다");
        assert_eq!(Span::parse("~2026-10-05 23:59").unwrap().to, Span::parse("2026-10-05 23:59").unwrap().to);
        // `--since` 는 여전히 한 끝만 받는다 — 분은 그 첫 초부터다.
        assert_eq!(Span::since("2026-10-10 10:10").unwrap().from, minute.from);
        assert!(Span::since("2026-10-10~").is_err(), "since 가 폭을 받았다");
        for bad in [
            "~",
            "2026-10-10 24:00",
            "2026-10-10 10:60",
            "2026-10-10 1:10",
            // 부호는 시각 자리에서도 오타다 — `"-1".parse::<i64>()` 는 받으므로 숫자인지를 따로 봐야 한다.
            "2026-10-10 -1:30",
            "2026-10-10 +1:30",
            "2026-10-10 10:+5",
            "2026-10-10 10:10:00",
            "2026-10-10T10:10",
            "2026-02-30 10:10",
            "2026-10-05~2026-10-03",
            "2026-10-10 10:11~2026-10-10 10:10",
            "2026-10-03~2026-10-04..2026-10-05",
        ] {
            assert!(Span::parse(bad).is_err(), "{bad:?} 를 받았다");
        }
    }

    /// **`*_at=` 은 줄 자신의 그 필드다**(moai-97tn, 2026-10-03 사용자 결정) — 키 이름이 `--json` 의 필드라 값도
    /// 그 필드 그대로다. 묶음도 멤버로 안 잰다. 없는 필드는 모르는 것이라 활짝 연 폭에도 안 든다.
    /// 옛 `done=` 은 칸으로 재고, `since=` 는 그때부터다 — 새 이름이 옛 뜻을 안 바꾼다.
    #[test]
    fn the_at_keys_read_the_rows_own_field() {
        let mut epic = issue("a-0001", "todo", &[]);
        epic.kind = Kind::Epic;
        let mut member = issue("a-0001.m1", "done", &[]);
        member.started_at = Some("2026-10-02T09:00:00Z".into());
        member.done_at = Some("2026-10-02T15:00:00Z".into());
        member.status_since = "2026-10-02T15:00:00Z".into();
        member.updated_at = "2026-10-04T00:00:00Z".into();
        // 닫았다가 다시 연 줄 — `done_at` 은 남고 칸은 todo 다.
        let mut reopened = issue("a-0002", "todo", &[]);
        reopened.started_at = Some("2026-10-01T09:00:00Z".into());
        reopened.done_at = Some("2026-10-02T10:00:00Z".into());
        reopened.updated_at = "2026-10-02T11:00:00Z".into();
        let all = vec![epic, member, reopened];
        let c = cfg();
        let wh = Where::of(&all, &c);
        let pick = |items: &[&str]| -> Vec<&str> {
            let f = Filter::build(Raw { filter: s(items), ..Raw::default() }).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.as_str()).collect()
        };
        assert_eq!(pick(&["started_at=2026-10-02"]), ["a-0001.m1"], "묶음을 멤버의 시작으로 골랐다");
        assert_eq!(pick(&["started_at=2000-01-01~"]), ["a-0001.m1", "a-0002"], "모르는 시작을 폭에 넣었다");
        assert_eq!(pick(&["done_at=2026-10-02"]), ["a-0001.m1", "a-0002"], "되돌린 줄의 done_at 을 안 봤다");
        assert_eq!(pick(&["done_at=2026-10-02", "status=done"]), ["a-0001.m1"]);
        // `done_at=` 은 칸을 안 본다 — done 이 빠진 `status=` 와 함께 줘도 거절하지 않는다(`done=` 의 `DoneOutside` 와
        // 다르다, 리뷰 moai-97tn.p44). 닫았다가 다시 연 줄을 찾는 물음이 이것이다.
        assert_eq!(pick(&["done_at=2026-10-02", "status=todo"]), ["a-0002"], "다시 연 줄을 못 찾았다");
        // 옛 `done=` 은 지금 done 에 선 줄을 그 칸에 든 때로 — 묶음은 멤버로 닫혔다.
        assert_eq!(pick(&["done=2026-10-02"]), ["a-0001", "a-0001.m1"], "옛 done= 의 뜻이 바뀌었다");
        assert_eq!(pick(&["done_at=2026-10-02 10:00"]), ["a-0002"], "1분으로 못 골랐다");
        // `updated_at=` 은 그 하루, `since=` 는 그때부터 — 같은 필드라 함께 주면 그리고다.
        assert_eq!(pick(&["updated_at=2026-10-02"]), ["a-0002"]);
        assert_eq!(pick(&["since=2026-10-02"]), ["a-0001.m1", "a-0002"]);
        assert_eq!(pick(&["since=2026-10-03", "updated_at=2026-10-02"]), Vec::<&str>::new());
        // `created_at=` 은 옛 `created=` 와 같은 자다.
        assert_eq!(pick(&["created_at=2026-09-01"]), pick(&["created=2026-09-01"]));
        assert_eq!(pick(&["created_at=2026-09-01"]).len(), 3);
        // 때로 물으면 숨김을 다 연다 — 새 키도 옛 키와 같다(done 에 선 멤버가 기본 목록에서 숨는다).
        let f = Filter::build(Raw { filter: s(&["done_at=2026-10-02"]), ..Raw::default() }).unwrap();
        assert!(f.all && f.archived && f.backlog, "새 시각 키가 숨김을 안 열었다");

        // **손으로 옮긴 묶음은 제 필드로 걸린다**(리뷰 moai-97tn.p44) — 위의 묶음은 그 필드가 없어 "안 걸린다" 만
        // 잰다. 묶음을 아예 빼는 고침(통계의 소요처럼)이 와도 그것만으로는 푸르다. 멤버의 때와 다르게 둔다.
        let mut moved = issue("b-0001", "done", &[]);
        moved.kind = Kind::Epic;
        moved.started_at = Some("2026-09-20T09:00:00Z".into());
        moved.done_at = Some("2026-09-25T09:00:00Z".into());
        let mut kid = issue("b-0001.k1", "done", &[]);
        kid.started_at = Some("2026-09-21T09:00:00Z".into());
        kid.done_at = Some("2026-09-24T09:00:00Z".into());
        let groups = vec![moved, kid];
        let wg = Where::of(&groups, &c);
        let pick = |items: &[&str]| -> Vec<&str> {
            let f = Filter::build(Raw { filter: s(items), ..Raw::default() }).unwrap();
            groups.iter().filter(|i| f.matches(i, NOW, &wg)).map(|i| i.id.as_str()).collect()
        };
        assert_eq!(pick(&["started_at=2026-09-20"]), ["b-0001"], "묶음의 제 시작을 안 봤다");
        assert_eq!(pick(&["done_at=2026-09-25"]), ["b-0001"], "묶음의 제 끝을 안 봤다");
        assert_eq!(pick(&["done_at=2026-09-24"]), ["b-0001.k1"], "묶음을 멤버의 끝으로 골랐다");
    }

    /// **때를 묻는 항목은 다 숨김을 열고, 날로 치면 시간대를 묻는다**(리뷰 moai-97tn.p44) — 키를 더하는 날 한쪽만
    /// 알면 그 키만 그사이 닫힌 줄을 말없이 놓치거나 UTC 의 날로 잰다. `updated_at=` 이 숨김을 여는 자리를 빼도 아무
    /// 시험이 안 붉어지던 판에서 세웠다.
    ///
    /// **항목은 손으로 적지 않고 [`KEYS`] 에서 고른다** — 때가 아닌 값을 때가 아니라고 거절하는 항목이 때 항목이다.
    /// 새 때 항목은 저절로 여기 든다. 고르는 자가 빗나가 몇을 안 고르는 판은 아래 바닥이 막는다 — `KEYS` 에서 빠진
    /// 항목도 거기 걸린다(거절문이 그 항목을 안 댄다).
    #[test]
    fn every_time_key_opens_what_is_hidden_and_asks_for_the_zone() {
        let build = |item: String| Filter::build(Raw { filter: vec![item], ..Raw::default() });
        let timed: Vec<&str> = KEYS
            .iter()
            .copied()
            .filter(|k| matches!(build(format!("{k}=어제")), Err(BadFilter::NotATime(_))))
            .collect();
        for k in ["since", "created", "done", "created_at", "updated_at", "started_at", "done_at"] {
            assert!(timed.contains(&k), "`{k}` 를 때 항목으로 못 골랐다 — {timed:?}");
        }
        for k in timed {
            let f = build(format!("{k}=2026-10-01")).unwrap();
            assert!(f.all && f.archived && f.backlog, "`{k}=` 가 숨김을 안 열었다");
            assert!(f.needs_zone(), "`{k}=` 가 시간대를 안 물었다");
            // 순간으로만 치면 시간대를 안 푼다 — 시각을 안 그리는 목록은 tzdb 를 안 만진다(moai-s3i7).
            let f = build(format!("{k}=2026-10-01T00:00:00Z")).unwrap();
            assert!(f.all && f.archived && f.backlog, "`{k}=` 순간이 숨김을 안 열었다");
            assert!(!f.needs_zone(), "`{k}=` 순간이 시간대를 물었다");
        }
    }

    /// **거르개 글은 `항목=` 에서 쪼개고, `=` 바로 뒤의 따옴표는 한 값이다**(moai-97tn). 사람이 든 예가 그대로
    /// 서야 하고, 따옴표 없이 쓰던 글(`grep=원자적 쓰기`)은 뜻이 안 바뀐다. 다른 자리의 따옴표는 글자다.
    #[test]
    fn split_items_keeps_a_quoted_value_whole() {
        let cut = |text: &str| split_items(text);
        assert_eq!(
            cut(
                r#"assignee=raven@buzzni.com assignee=joshep@buzzni.com tag=bug done_at="2026-10-03 00:00~2026-10-05 23:59""#
            ),
            [
                "assignee=raven@buzzni.com",
                "assignee=joshep@buzzni.com",
                "tag=bug",
                "done_at=2026-10-03 00:00~2026-10-05 23:59"
            ]
        );
        assert_eq!(cut(r#"done_at="2026-10-02""#), ["done_at=2026-10-02"]);
        assert_eq!(cut("done_at='2026-10-10 10:10'"), ["done_at=2026-10-10 10:10"]);
        // 따옴표 없이도 `항목=` 이 없는 낱말은 앞 값에 붙는다 — 옛 칸의 뜻 그대로다.
        assert_eq!(cut("grep=원자적   쓰기 status=todo"), ["grep=원자적 쓰기", "status=todo"]);
        assert_eq!(cut("done_at=2026-10-10 10:10"), ["done_at=2026-10-10 10:10"]);
        // 따옴표 안의 `=` 와 빈칸은 글자다.
        assert_eq!(cut(r#"grep="a=b  c" status=todo"#), ["grep=a=b  c", "status=todo"]);
        // 값 머리가 아닌 따옴표는 글자 그대로다 — 셸처럼 읽으면 `'` 가 줄 끝까지를 삼킨다.
        assert_eq!(cut("grep=don't stop tag=bug"), ["grep=don't stop", "tag=bug"]);
        assert_eq!(cut(r#"grep='say "hi"' tag=bug"#), [r#"grep=say "hi""#, "tag=bug"]);
        // 닫지 않은 따옴표는 줄 끝까지다 — 치는 동안 거절하지 않는다.
        assert_eq!(cut(r#"tag=bug done_at="2026-10-03 00:00~"#), ["tag=bug", "done_at=2026-10-03 00:00~"]);
        assert!(cut("   ").is_empty());
        // `항목=` 없이 시작하면 그대로 넘겨 `desugar` 가 거절한다.
        assert_eq!(cut("todo"), ["todo"]);
        assert_eq!(
            Filter::build(Raw { filter: cut("todo"), ..Raw::default() }).unwrap_err(),
            BadFilter::NotAPair("todo".into())
        );
    }

    /// **항목마다 선 자리를 글과 함께 낸다**(moai-mkyg.dcf) — 탐색기의 값 안내(`tui::hint::slot`)가 커서 밑의 항목을 이
    /// 자리로 찾는다. 자리는 바이트다(한글은 세 바이트). 연 낱말의 끝은 닫는 따옴표 뒤에 붙은 글자까지고, 뒤에 이어
    /// 적은 낱말은 글에만 붙는다. 맨 앞의 `=` 없는 낱말도 항목이다 — `desugar` 가 그것을 거절한다.
    #[test]
    fn items_say_where_each_item_stands() {
        let at = |text: &str| -> Vec<(String, usize, Option<usize>, usize, Option<(usize, Option<usize>)>)> {
            items(text).into_iter().map(|it| (it.text, it.start, it.eq, it.end, it.quote)).collect()
        };
        assert_eq!(
            at(r#"foo grep="a b"c tag=bug x assignee='Kim"#),
            [
                ("foo".into(), 0, None, 3, None),
                ("grep=a bc".into(), 4, Some(8), 15, Some((9, Some(13)))),
                ("tag=bug x".into(), 16, Some(19), 23, None),
                ("assignee=Kim".into(), 26, Some(34), 39, Some((35, None))),
            ]
        );
        assert_eq!(
            at("tag=버그  grep=\"원자적 쓰기\""),
            [
                ("tag=버그".into(), 0, Some(3), 10, None),
                ("grep=원자적 쓰기".into(), 12, Some(16), 35, Some((17, Some(34))))
            ]
        );
        assert!(at(" \t ").is_empty());
    }

    /// **사람이 든 예가 그 사람의 시계로 선다**(moai-97tn) — 서울에서 친 `2026-10-03 00:00` 은 UTC 로 10-02
    /// 15:00 이다. 칸에 친 글 그대로([`split_items`])를 거름망에 건다.
    #[test]
    fn the_peoples_examples_read_on_their_own_clock() {
        let row = |id: &str, who: &str, done_at: &str| {
            let mut i = issue(id, "done", &["bug"]);
            i.assignee_email = Some(who.into());
            i.assignee = Some(who.split('@').next().unwrap().into());
            i.done_at = Some(done_at.into());
            i
        };
        let all = vec![
            row("a-0001", "raven@buzzni.com", "2026-10-02T15:00:00Z"), // 서울 10-03 00:00
            row("a-0002", "joshep@buzzni.com", "2026-10-05T14:59:59Z"), // 서울 10-05 23:59:59
            row("a-0003", "raven@buzzni.com", "2026-10-02T14:59:59Z"), // 서울 10-02 23:59:59
            row("a-0004", "mina@buzzni.com", "2026-10-03T01:00:00Z"),
            row("a-0005", "raven@buzzni.com", "2026-10-10T01:10:30Z"), // 서울 10-10 10:10:30
        ];
        let c = cfg();
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let mut wh = Where::of(&all, &c);
        wh.zone = Some(&seoul);
        let pick = |text: &str| -> Vec<&str> {
            let f = Filter::build(Raw { filter: split_items(text), ..Raw::default() }).unwrap();
            assert!(f.needs_zone(), "{text} 가 시간대를 안 물었다");
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.as_str()).collect()
        };
        assert_eq!(
            pick(
                r#"assignee=raven@buzzni.com assignee=joshep@buzzni.com tag=bug done_at="2026-10-03 00:00~2026-10-05 23:59""#
            ),
            ["a-0001", "a-0002"]
        );
        assert_eq!(pick(r#"done_at="2026-10-02""#), ["a-0003"]);
        assert_eq!(pick(r#"done_at="2026-10-10 10:10""#), ["a-0005"]);
    }

    /// **지운 줄도 목록의 `--since` 와 같은 자로 잰다**(moai-7dmq) — 날로 친 때는 읽는 사람의 날이고, 못
    /// 읽는 도장은 폭이 있으면 어느 폭에도 안 든다. 못 읽는 줄을 지운 `rm`(제목 없음)과 다른 `kind` 는 빠지고,
    /// 남은 줄은 받은 차례 그대로다.
    #[test]
    fn removed_keeps_issue_removals_within_since() {
        use crate::model::JournalEntry;
        let by = crate::model::someone("raven");
        let entries = [
            JournalEntry::removed("a-0003", "셋", "2026-09-29T00:00:00Z", &by),
            JournalEntry::removed_line(Some("a-0004"), "{깨진", "2026-09-30T21:00:00Z", &by),
            JournalEntry::note("a-0005", "rm", "2026-09-30T22:00:00Z", &by),
            JournalEntry::removed("a-0001", "하나", "2026-09-30T20:00:00Z", &by),
            JournalEntry::removed("a-0002", "둘", "어제", &by),
        ];
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let pick = |since: &[&str], zone: Option<&crate::tz::Zone>| {
            let f = Filter::build(Raw { since: s(since), ..Raw::default() }).unwrap();
            removed(&entries, &f.updated, zone).into_iter().map(|e| e.id.as_str()).collect::<Vec<_>>()
        };
        assert_eq!(pick(&[], None), ["a-0003", "a-0001", "a-0002"], "`--since` 없이는 지운 이슈 전부다");
        // 서울의 10-01 05:00 은 UTC 로 09-30 20:00 이다.
        assert_eq!(pick(&["2026-10-01"], Some(&seoul)), ["a-0001"], "읽는 사람의 날로 안 쟀다");
        assert!(pick(&["2026-10-01"], None).is_empty(), "시간대 없이는 UTC 의 날이다");
        assert_eq!(pick(&["2026-09-29T00:00:00Z"], Some(&seoul)), ["a-0003", "a-0001"], "못 읽는 도장이 폭에 들었다");
    }

    /// **못 푼 조각은 온전했으면 섰을 폭에서만 댄다**(moai-g8ho 리뷰) — 도장을 읽었으면 [`removed`] 와 같은 자로
    /// 재고, 못 읽었으면 들었을 수 있다. 폭이 없으면 늘 댄다.
    #[test]
    fn a_torn_line_counts_only_where_it_could_have_stood() {
        let seoul = crate::tz::Zone::fixed("Asia/Seoul", 9 * 3600);
        let may = |since: &[&str], ts: Option<&str>| {
            let f = Filter::build(Raw { since: s(since), ..Raw::default() }).unwrap();
            may_fall_in(&f.updated, ts, Some(&seoul))
        };
        assert!(may(&[], Some("2026-09-01T00:00:00Z")), "폭이 없는데 조각을 뺐다");
        assert!(may(&[], None), "폭이 없는데 조각을 뺐다");
        assert!(!may(&["2026-09-29T00:00:00Z"], Some("2026-09-28T23:59:59Z")), "폭 밖의 조각을 댔다");
        assert!(may(&["2026-09-29T00:00:00Z"], Some("2026-09-29T00:00:00Z")), "폭 안의 조각을 뺐다");
        // 서울의 10-01 은 UTC 로 09-30 15:00 부터다 — 날로 친 폭은 읽는 사람의 날로 잰다.
        assert!(
            may(&["2026-10-01"], Some("2026-09-30T15:00:00Z")) && !may(&["2026-10-01"], Some("2026-09-30T14:59:59Z"))
        );
        for unread in [None, Some("2026-09-1"), Some("어제")] {
            assert!(may(&["2026-09-29T00:00:00Z"], unread), "못 읽는 도장의 조각을 뺐다: {unread:?}");
        }
    }

    /// **때로 물으면 숨긴 줄을 다 연다**(2026-09-30 사용자 결정) — 그사이 닫힌 줄도 바뀐 줄이다. `--done` 은
    /// **지금 done 에 선** 줄만 본다: `done_at` 은 되돌려도 남으니 그것만 보면 다시 연 줄이 닫힌 것으로 선다.
    #[test]
    fn asking_by_time_opens_what_is_hidden_and_done_reads_the_column() {
        let at = |id: &str, status: &str, kind: Kind, when: &str| {
            let mut i = issue(id, status, &[]);
            i.kind = kind;
            (i.updated_at, i.status_since) = (when.to_string(), when.to_string());
            i
        };
        let mut reopened = at("a-0003", "todo", Kind::Issue, "2026-09-06T00:00:00Z");
        reopened.done_at = Some("2026-09-05T00:00:00Z".into());
        let mut closed = at("a-0002", "done", Kind::Issue, "2026-09-05T00:00:00Z");
        closed.done_at = Some("2026-09-05T00:00:00Z".into());
        // `done_at` 전에 닫힌 옛 줄 — 그 칸에 든 때로 잰다.
        let old_close = at("a-0005", "done", Kind::Issue, "2026-09-04T00:00:00Z");
        let all = vec![
            at("a-0001", "todo", Kind::Issue, "2026-09-01T00:00:00Z"),
            closed,
            reopened,
            at("a-0004", "todo", Kind::Backlog, "2026-09-05T00:00:00Z"),
            old_close,
        ];
        let c = cfg();
        let wh = Where::of(&all, &c);
        let pick = |raw: Raw| {
            let f = Filter::build(raw).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.as_str()).collect::<Vec<_>>()
        };
        assert_eq!(pick(Raw { since: s(&["2026-09-05"]), ..Raw::default() }), ["a-0002", "a-0003", "a-0004"]);
        assert_eq!(
            pick(Raw { since: s(&["2026-09-05"]), status: s(&["todo"]), ..Raw::default() }),
            ["a-0003", "a-0004"]
        );
        assert_eq!(
            pick(Raw { done: s(&["2026-09-01.."]), ..Raw::default() }),
            ["a-0002", "a-0005"],
            "다시 연 줄이 섰다"
        );
        assert_eq!(pick(Raw { done: s(&["2026-09-04"]), ..Raw::default() }), ["a-0005"], "옛 줄의 끝난 때를 못 쟀다");
        // 되풀이는 그리고(겹치는 곳), 쉼표는 또는.
        let both = Raw { since: s(&["2026-09-05", "..2026-09-05"]), ..Raw::default() };
        assert!(Filter::build(both).is_err(), "--since 가 폭을 받았다");
        let and = Raw { done: s(&["2026-09-04..", "..2026-09-04"]), ..Raw::default() };
        assert_eq!(pick(and), ["a-0005"], "되풀이가 그리고가 아니다");
        let or = Raw { done: s(&["2026-09-04,2026-09-05"]), ..Raw::default() };
        assert_eq!(pick(or), ["a-0002", "a-0005"], "쉼표가 또는이 아니다");
        // `--filter` 도 같은 자리에 쌓인다.
        assert_eq!(pick(Raw { filter: s(&["since=2026-09-06"]), ..Raw::default() }), ["a-0003"]);
        // 때가 하나도 없는 값은 거절한다 — 빈 또는-묶음은 어느 줄도 못 지나 말없이 0건이 됐다(moai-efoc 리뷰).
        for empty in [
            Raw { since: s(&[""]), ..Raw::default() },
            Raw { since: s(&[" , "]), ..Raw::default() },
            Raw { since: s(&["2026-09-01", ""]), ..Raw::default() },
            Raw { created: s(&[""]), ..Raw::default() },
            Raw { done: s(&[","]), ..Raw::default() },
            Raw { filter: s(&["since="]), ..Raw::default() },
        ] {
            assert!(Filter::build(empty).is_err(), "빈 때를 받았다");
        }
        // `--done` 은 done 칸의 줄만 본다 — done 이 없는 `-s` 와 함께 쓰면 늘 0건이라 거절한다.
        let e = Filter::build(Raw { done: s(&["2026-09-05"]), status: s(&["review"]), ..Raw::default() }).unwrap_err();
        assert_eq!(e, BadFilter::DoneOutside { asked: "review".into() });
        assert_eq!(
            pick(Raw { done: s(&["2026-09-04.."]), status: s(&["todo,done"]), ..Raw::default() }),
            ["a-0002", "a-0005"],
            "done 이 든 `-s` 를 거절했다"
        );
    }

    /// **`-g` 는 실린 노트까지 본다**(moai-efoc.zyc) — 전체 범위와 노트 범위만(moai-wcy8.3v9). 노트가 안
    /// 실렸으면 안 본다. 노트 범위는 노트**만** 본다 — 제목에 든 글로는 안 걸린다. 받은 글 그대로 실든
    /// 미리 접어 싣든([`NoteView`], 리뷰 moai-wcy8.rbj) 답이 같다.
    #[test]
    fn grep_sees_the_notes_it_is_handed_in_the_whole_and_note_scopes() {
        let all = vec![issue("a-0001", "todo", &[]), issue("a-0002", "todo", &[])];
        let c = cfg();
        let notes: Notes = [("a-0002".to_string(), vec!["사용자 결정: 둘째 길 (Recommended)".to_string()])].into();
        let folded = fold_notes(&notes);
        let mut wh = Where::of(&all, &c);
        let pick = |wh: &Where, grep_in: GrepIn, q: &str| {
            let f = Filter::build(Raw { grep: Some(q.into()), grep_in, ..Raw::default() }).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, wh)).map(|i| i.id.as_str()).collect::<Vec<_>>()
        };
        assert!(pick(&wh, GrepIn::All, "둘째 길").is_empty(), "안 실린 노트를 봤다");
        for view in [NoteView::Raw(&notes), NoteView::Folded(&folded)] {
            wh.notes = Some(view);
            assert_eq!(pick(&wh, GrepIn::All, "둘째 길"), ["a-0002"], "실린 노트를 안 봤다 — {view:?}");
            assert_eq!(pick(&wh, GrepIn::All, "RECOMMENDED"), ["a-0002"], "노트는 대소문자를 안 가린다 — {view:?}");
            assert!(pick(&wh, GrepIn::Title, "둘째 길").is_empty(), "좁힌 범위가 노트를 봤다 — {view:?}");
            assert_eq!(pick(&wh, GrepIn::Note, "둘째 길"), ["a-0002"], "노트 범위가 노트를 안 봤다 — {view:?}");
            // 제목에 걸린 줄은 노트 없이도 그대로 걸린다. 노트 범위는 제목을 안 본다.
            assert_eq!(pick(&wh, GrepIn::All, "a-0001 제목"), ["a-0001"]);
            assert!(pick(&wh, GrepIn::Note, "a-0001 제목").is_empty(), "노트 범위가 제목을 봤다 — {view:?}");
        }
    }

    /// **상세가 그리는 것은 노트에서 걸린 줄뿐이고, 그 줄은 거름망이 노트로 건 줄에 늘 선다**(moai-wcy8.3v9) —
    /// 친 그대로 받아 거름망과 같이 접는다. 빈칸뿐인 글은 아무 줄도 안 낸다.
    ///
    /// **거름망과 나란히 잰다**(리뷰 moai-wcy8.rbj) — 둘 가운데 하나만 접는 법이 바뀌면(`draw::mark` 의 `ς`→`σ`,
    /// 앞뒤 빈칸 걷기) 노트로 걸린 줄의 상세가 빈다. 글자째 적은 답만 보던 판은 그 어긋남에 안 붉어졌다. 두
    /// 글자로 접히는 `İ` 와 낱말 끝 `Σ` 를 줄 끝과 줄 가운데에 둔다 — 통째로 접을 때와 줄마다 접을 때가
    /// 갈리면 여기서 선다.
    #[test]
    fn the_note_lines_that_matched_are_the_lines_the_filter_saw() {
        let texts = vec![
            "첫 줄\n사용자 결정: 둘째 길 (Recommended)\n끝".to_string(),
            "둘째 노트의 한 줄".to_string(),
            "ΟΔΟΣ\nİstanbul ΟΔΟΣ ΕΝΑ".to_string(),
        ];
        assert_eq!(noted_lines(&texts, "RECOMMENDED"), ["사용자 결정: 둘째 길 (Recommended)"]);
        assert_eq!(noted_lines(&texts, "둘째"), ["사용자 결정: 둘째 길 (Recommended)", "둘째 노트의 한 줄"]);
        assert!(noted_lines(&texts, "없는 말").is_empty());
        assert!(noted_lines(&texts, "  ").is_empty(), "빈칸으로 모든 줄을 냈다");

        let all = vec![issue("a-0001", "todo", &[])];
        let c = cfg();
        let notes: Notes = [("a-0001".to_string(), texts.clone())].into();
        let folded = fold_notes(&notes);
        let mut wh = Where::of(&all, &c);
        wh.notes = Some(NoteView::Folded(&folded));
        // 줄바꿈을 든 글은 안 잰다 — 탐색기의 검색 칸은 그것을 빈칸으로 받는다([`noted_lines`]).
        for q in ["recommended", "둘째", "없는 말", "οδος", "οδοσ", "ΟΔΟΣ", "i̇stanbul", "İSTANBUL", "ος ενα"]
        {
            let f = Filter::build(Raw { grep: Some(q.into()), grep_in: GrepIn::Note, ..Raw::default() }).unwrap();
            assert_eq!(
                f.matches(&all[0], NOW, &wh),
                !noted_lines(&texts, q).is_empty(),
                "거름망과 상세가 {q:?} 에 다른 답을 냈다"
            );
        }
    }

    /// **목록의 기본 차례는 급한 것 → id 다** — 차례를 고르지 않은 [`page`] 가 [`display_order`] 와 같다.
    #[test]
    fn sorting_puts_the_urgent_first_then_id() {
        let mut v = vec![issue("a-0003", "todo", &[]), issue("a-0001", "todo", &[]), issue("a-0002", "todo", &[])];
        v[0].priority = Some(0);
        let all = v.clone();
        let c = cfg();
        page(&mut v, &all, &Where::of(&all, &c), &c, Sort::default(), None, None);
        let ids: Vec<&str> = v.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["a-0003", "a-0001", "a-0002"]);
    }

    /// **커서는 자리가 아니라 값이다**(moai-efoc.ku7) — 앞 쪽을 받은 뒤 줄이 지워지거나 생겨도 다음 쪽이
    /// 밀리지 않고, 커서 줄이 목록에서 빠져도(닫혀 숨었다) 그 값으로 넘는다. offset 이면 둘 다 어긋난다.
    #[test]
    fn a_page_starts_after_the_cursor_value_not_its_position() {
        let all: Vec<Issue> = (1..=5).map(|n| issue(&format!("a-000{n}"), "todo", &[])).collect();
        let c = cfg();
        let by_id = Sort { key: SortKey::Id, reversed: false };
        let run = |rows: &[Issue], sort: Sort, after: Option<&str>, limit: Option<usize>| {
            let wh = Where::of(&all, &c);
            let mut v = rows.to_vec();
            let cursor = after.map(|id| all.iter().find(|i| i.id == id).unwrap());
            let cut = page(&mut v, &all, &wh, &c, sort, cursor, limit);
            (v.into_iter().map(|i| i.id).collect::<Vec<_>>(), cut)
        };
        assert_eq!(run(&all, by_id, None, Some(2)), (s(&["a-0001", "a-0002"]), 3), "첫 쪽");
        assert_eq!(run(&all, by_id, Some("a-0002"), Some(2)), (s(&["a-0003", "a-0004"]), 1), "둘째 쪽");
        assert_eq!(run(&all, by_id, Some("a-0004"), Some(2)), (s(&["a-0005"]), 0), "끝 쪽 — 잘린 것이 없다");
        // 앞 쪽을 받은 뒤 a-0001 이 지워졌다 — offset 2 면 a-0004 부터 받아 a-0003 을 놓친다.
        let gone: Vec<Issue> = all.iter().filter(|i| i.id != "a-0001").cloned().collect();
        assert_eq!(run(&gone, by_id, Some("a-0002"), Some(2)).0, s(&["a-0003", "a-0004"]), "지운 줄에 쪽이 밀렸다");
        // 커서 줄이 걸러져 목록에 없다 — 값으로 넘으므로 그래도 그 뒤부터다.
        let hidden: Vec<Issue> = all.iter().filter(|i| i.id != "a-0002").cloned().collect();
        assert_eq!(run(&hidden, by_id, Some("a-0002"), None).0, s(&["a-0003", "a-0004", "a-0005"]));
        // 뒤집은 차례에서 "뒤" 는 뒤집은 차례의 뒤다.
        let back = Sort { key: SortKey::Id, reversed: true };
        assert_eq!(
            run(&all, back, Some("a-0004"), None).0,
            s(&["a-0003", "a-0002", "a-0001"]),
            "뒤집은 차례의 뒤가 아니다"
        );
    }

    /// **한 id 의 줄은 한 덩어리로 넘는다**(moai-efoc 리뷰) — 머지가 남긴 쌍둥이가 차례에서 떨어져 서도, 커서를
    /// 따라 쪽을 넘기는 쪽은 줄마다 꼭 한 번 받고 끝난다. 뒷줄(`Load::get`) 하나로 넘던 때는 사이의 줄을
    /// 건너뛰거나 같은 쪽을 끝없이 되받거나 `-n` 이 가른 쌍둥이를 잃었다.
    #[test]
    fn a_page_walk_delivers_every_twin_line_once() {
        let c = cfg();
        let line = |id: &str, p: u8, title: &str| {
            let mut i = issue(id, "todo", &[]);
            i.priority = Some(p);
            i.title = title.to_string();
            i
        };
        // `--json` 으로 도는 쪽 그대로 — 마지막 줄의 id 를 커서로 주고, `-n` 보다 짧은 쪽이 오면 멈춘다.
        let walk = |all: &[Issue], sort: Sort, n: usize| {
            let wh = Where::of(all, &c);
            let mut got: Vec<String> = Vec::new();
            let mut after: Option<String> = None;
            for _ in 0..20 {
                let mut v = all.to_vec();
                let cursor = after.as_deref().map(|id| all.iter().rfind(|i| i.id == id).unwrap());
                page(&mut v, all, &wh, &c, sort, cursor, Some(n));
                got.extend(v.iter().map(|i| i.title.clone()));
                if v.len() < n {
                    got.sort();
                    return got;
                }
                after = v.last().map(|i| i.id.clone());
            }
            panic!("쪽 넘기기가 안 끝났다 — {got:?}");
        };
        let by_priority = Sort::default();
        let by_id = Sort { key: SortKey::Id, reversed: false };
        let shapes = [
            // 앞줄이 먼저 선다 — 뒷줄로 넘으면 사이의 b·c 를 건너뛰었다.
            (
                vec![line("a-0001", 1, "a1"), line("a-0001", 3, "a3"), line("a-0002", 2, "b"), line("a-0003", 2, "c")],
                by_priority,
                1,
            ),
            // 뒷줄이 먼저 선다 — 같은 쪽을 끝없이 되받았다.
            (
                vec![
                    line("a-0001", 3, "a3"),
                    line("a-0001", 1, "a1"),
                    line("a-0002", 2, "b"),
                    line("a-0003", 4, "c"),
                    line("a-0009", 0, "x"),
                ],
                by_priority,
                2,
            ),
            // id 차례에서 `-n 1` 이 쌍둥이를 갈랐다.
            (vec![line("a-0001", 1, "a1"), line("a-0001", 3, "a3"), line("a-0002", 2, "b")], by_id, 1),
            (vec![line("a-0001", 3, "a3"), line("a-0001", 1, "a1"), line("a-0002", 2, "b")], by_id, 1),
            // 값이 같은 쌍둥이(머지의 흔한 흔적)도 `-n` 이 가르면 뒷줄을 잃었다.
            (vec![line("a-0001", 2, "ours"), line("a-0001", 2, "theirs"), line("a-0002", 2, "b")], by_priority, 1),
        ];
        for (all, sort, n) in shapes {
            let mut want: Vec<String> = all.iter().map(|i| i.title.clone()).collect();
            want.sort();
            assert_eq!(walk(&all, sort, n), want, "줄을 잃거나 두 번 받았다 — {sort:?} -n {n}");
        }
        // 쌍둥이는 머리 줄 자리에 모여 서고, 쪽은 그 둘을 가르지 않는다 — 가르느니 그 쪽을 늘린다.
        let all = vec![line("a-0001", 1, "a1"), line("a-0001", 3, "a3"), line("a-0002", 2, "b")];
        let mut v = all.clone();
        let more = page(&mut v, &all, &Where::of(&all, &c), &c, by_priority, None, Some(1));
        assert_eq!(v.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["a1", "a3"], "쌍둥이를 갈랐다");
        assert_eq!(more, 1, "잘린 수가 틀렸다");
    }

    /// **생성·수정 차례의 동점은 id 로만 가른다**(moai-psyu) — 계획 하나가 한 초에 서므로 같은 `created_at` 은
    /// 흔하다. 우선순위로 가르면 쪽 사이에 우선순위를 고친 줄이 커서를 넘어 빠지거나 겹친다. 다른 차례의 동점은
    /// 그대로 [`display_order`] 다.
    #[test]
    fn created_and_updated_ties_fall_to_id_alone() {
        let c = cfg();
        // `issue` 는 다 같은 초에 만들고 고친 줄이다.
        let urgent = |id: &str, p: u8| {
            let mut i = issue(id, "todo", &[]);
            i.priority = Some(p);
            i
        };
        let all = vec![urgent("a-0001", 2), urgent("a-0002", 0), urgent("a-0003", 1)];
        let ids = |v: &[Issue]| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        let sorted = |key, reversed| {
            let mut v = all.clone();
            page(&mut v, &all, &Where::of(&all, &c), &c, Sort { key, reversed }, None, None);
            ids(&v)
        };
        for key in [SortKey::Created, SortKey::Updated] {
            assert_eq!(sorted(key, false), s(&["a-0001", "a-0002", "a-0003"]), "{key:?} 의 동점에 우선순위가 끼었다");
            assert_eq!(sorted(key, true), s(&["a-0003", "a-0002", "a-0001"]), "{key:?} 를 뒤집은 동점");
        }
        for key in [SortKey::Priority, SortKey::Status, SortKey::Assignee] {
            assert_eq!(sorted(key, false), s(&["a-0002", "a-0003", "a-0001"]), "{key:?} 의 동점이 기본 차례가 아니다");
        }

        // 우선순위가 다 같은 줄로 첫 쪽을 받은 뒤 a-0003 을 p0 으로 고쳤다 — 우선순위로 가르면 커서(a-0001)
        // 앞으로 올라가 빠진다. 첫 쪽은 옛 차례와 새 차례가 같아야 고친 줄이 넘는지를 잰다.
        let level = vec![urgent("a-0001", 2), urgent("a-0002", 2), urgent("a-0003", 2)];
        let by_created = Sort { key: SortKey::Created, reversed: false };
        let mut first = level.clone();
        page(&mut first, &level, &Where::of(&level, &c), &c, by_created, None, Some(1));
        assert_eq!(ids(&first), s(&["a-0001"]));
        let mut edited = level.clone();
        edited[2].priority = Some(0);
        let mut rest = edited.clone();
        page(&mut rest, &edited, &Where::of(&edited, &c), &c, by_created, Some(&edited[0]), None);
        assert_eq!(ids(&rest), s(&["a-0002", "a-0003"]), "우선순위를 고친 줄이 커서를 넘었다");
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
        thought.kind = Kind::Backlog;
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

    /// **쌍둥이 부모 밑에서 소속을 못 정한 줄도 어느 소속으로도 안 고른다**(moai-mibi.wpj) —
    /// `-e none`·`--milestone none` 도. 에픽이 없는 것이 아니라 못 정한 것이고, 트리는 그 줄을
    /// `(길 잃음)` 에 둔다. 한때 뒷줄의 에픽으로 골렸다. **두 차례를 다 잰다**(리뷰 moai-mibi.ndh) — 한
    /// 차례만 재면 뒷줄이 마일스톤 없는 에픽을 넘기는 판이라, `-e argos-0001`·`--milestone argos-0009`
    /// 가 옛 바이너리에서도 푸르게 선다.
    #[test]
    fn a_child_under_disagreeing_twin_parents_is_picked_by_no_membership() {
        let group = |id: &str, kind: Kind| {
            let mut i = issue(id, "todo", &[]);
            i.kind = kind;
            i
        };
        let parent = |to: &str| {
            let mut i = issue("argos-0010", "todo", &[]);
            i.epic = Some(to.into());
            i
        };
        for (a, b) in [("argos-0001", "argos-0002"), ("argos-0002", "argos-0001")] {
            let mut e1 = group("argos-0001", Kind::Epic);
            e1.milestone = Some("argos-0009".into());
            let all = vec![
                e1,
                group("argos-0002", Kind::Epic),
                group("argos-0009", Kind::Milestone),
                parent(a),
                parent(b),
                issue("argos-0010.aa1", "todo", &[]),
            ];
            let cfg = cfg();
            let wh = Where::of(&all, &cfg);
            let picked = |raw: Raw| -> bool {
                let f = Filter::build(raw).unwrap();
                f.matches(&all[5], NOW, &wh)
            };
            for sel in ["argos-0001", "argos-0002", "none"] {
                assert!(!picked(Raw { epic: s(&[sel]), ..Raw::default() }), "{b} 가 뒤: -e {sel} 가 골랐다");
            }
            for sel in ["argos-0009", "none"] {
                assert!(
                    !picked(Raw { milestone: s(&[sel]), ..Raw::default() }),
                    "{b} 가 뒤: --milestone {sel} 가 골랐다"
                );
            }
            assert!(picked(Raw::default()), "{b} 가 뒤: 거르개 없이도 안 나온다");
        }
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
    fn an_backlog_hides_until_it_is_asked_for() {
        let mut thought = issue("argos-0001", "todo", &[]);
        thought.kind = Kind::Backlog;
        thought.title = "파서를 다시 쓴다".into();
        let all = vec![thought.clone(), issue("argos-0009", "todo", &[])];
        let cfg = cfg();
        let wh = Where::of(&all, &cfg);
        let hits = |raw: Raw| -> Vec<String> {
            let f = Filter::build(raw).unwrap();
            all.iter().filter(|i| f.matches(i, NOW, &wh)).map(|i| i.id.clone()).collect()
        };

        assert_eq!(hits(Raw::default()), ["argos-0009"], "기본 목록에 backlog 가 섞였다");
        assert_eq!(
            hits(Raw { kind: Some(Kind::Backlog), ..Raw::default() }),
            ["argos-0001"],
            "콕 집어 물었는데 안 나온다"
        );
        assert_eq!(
            hits(Raw { grep: Some("파서".into()), ..Raw::default() }),
            ["argos-0001"],
            "적어 둔 생각을 글로 못 찾는다 — 그러면 같은 것을 또 적는다"
        );
        assert_eq!(
            hits(Raw { backlog: true, ..Raw::default() }),
            ["argos-0001", "argos-0009"],
            "탐색기가 켜고 들어오는 축이 안 듣는다"
        );
    }
}
