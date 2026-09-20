//! 이슈와 저널 레코드. **여기는 파일시스템도 터미널도 모른다.**

use crate::fail::{Fail, R, code};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// 우선순위를 안 적었을 때의 값. 기본값은 파일에 쓰지 않는다 (§정규화).
pub const DEFAULT_PRIORITY: u8 = 2;
pub const MAX_PRIORITY: u8 = 3;

/// 한 번에 적는 글 하나의 상한 — 노트·`-m` 의 까닭·제목·본문(moai-m9a8, 2026-09-18 사용자 결정).
///
/// 에이전트가 리뷰 **원문**을 붙이라는 말을 서브에이전트의 **대화록**(JSONL)으로 읽어 노트 한
/// 줄에 36만·40만 자를 두 번 넣었다. 저널은 덧붙이기만 하므로 한 번 커밋되면 영영 남고,
/// `moai show` 의 이력이 그 한 줄에 묻힌다. 잰 값: 제대로 쓴 노트는 가장 큰 것이 2.8만 **바이트**
/// (1.8만 자)다. 재는 것도 바이트(`str::len`)라, 한글만으로는 2.2만 자쯤에서 선다.
///
/// **잘라 적지 않고 거절한다** — 잘라 적으면 원문 일부가 말없이 사라진다(2026-09-15 사용자 결정).
/// 재는 것은 **지금 쓰는 글**뿐이다: 이미 큰 본문을 가진 줄도 옮기거나 고칠 수 있다.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// [`MAX_TEXT_BYTES`] 를 넘는 글을 거절한다. 거절문은 **무엇을 넣어야 했는지**를 댄다 — 상한만
/// 대면 에이전트는 대화록을 반으로 잘라 다시 넣는다.
///
/// 리뷰 줄은 [`crate::guide::REVIEW_OVER_LIMIT`] 에서 온다 — 가르치는 글과 거절문이 갈라지면
/// 큰 리뷰를 닫는 쪽이 두 말 사이에서 멈춘다(moai-b8aj). `<크기>` 는 여기서 채운다: 재 놓은 수를
/// 자리 표시로 도로 내밀면 받는 쪽이 첫 줄에서 그것을 옮겨 적어야 한다.
pub fn check_text_size(id: &str, what: &str, text: &str) -> R<()> {
    if text.len() <= MAX_TEXT_BYTES {
        return Ok(());
    }
    let kb = text.len().div_ceil(1024);
    Err(Fail::coded(
        format!(
            "{id}: {what} 크기가 {kb}KB 다 — 한 번에 {}KB 까지 적는다. 잘라 적지 않는다\n      \
             요약을 적고 원문은 파일로 둔다. 리뷰 원문이면 리뷰가 낸 글이지 그 대화록(JSONL)이 아니다\n      \
             리뷰 원문이면: {}",
            MAX_TEXT_BYTES / 1024,
            // 두 줄짜리 글이다 — 이어 붙인 줄에도 같은 여섯 칸을 준다(`guide::REVIEW_OVER_LIMIT`).
            crate::guide::REVIEW_OVER_LIMIT.replace("<크기>", &kb.to_string()).replace('\n', "\n      ")
        ),
        code::BAD_INPUT,
    ))
}

/// 도구가 **스스로 짓는** 글에 남의 글 한 토막을 담을 때, 그 토막만 `budget` 바이트에 맞춰
/// 줄인다(moai-clta). 넘을 때만 줄이고, 줄였으면 `…` 로 밝힌다.
///
/// [`check_text_size`] 와 가르는 자는 **누가 적었는가**다. 사람이 적은 글은 잘라 적지 않고
/// 거절한다 — 무엇이 사라졌는지 적은 쪽이 안다. 도구가 짓는 노트에는 거절할 사람이 없어,
/// 같은 자로 재면 길이 통째로 막힌다: 제목이 상한 턱밑인 idea 는 `promote` 의 노트 머리말
/// 31바이트 때문에 펼칠 수조차 없었다. 담기는 것이 **가리킴이지 사본이 아닐 때**만 쓴다 —
/// 원본은 제 줄에 그대로 남아 있어야 한다.
pub fn fit(text: &str, budget: usize) -> std::borrow::Cow<'_, str> {
    const MARK: &str = "…";
    if text.len() <= budget {
        return std::borrow::Cow::Borrowed(text);
    }
    // 표식조차 안 들어가면 빈 글이다 — 넘치게 내면 재는 쪽이 도로 거절한다.
    let Some(mut cut) = budget.checked_sub(MARK.len()) else {
        return std::borrow::Cow::Borrowed("");
    };
    // **글자 가운데서 자르지 않는다** — 한글 한 자가 3바이트라 예사로 걸린다.
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    std::borrow::Cow::Owned(format!("{}{MARK}", &text[..cut]))
}

/// 이슈의 구조적 종류. `tags` 와 축이 다르다 —
/// `kind` 는 무엇인가, `tags` 는 어떤 성격인가.
///
/// `Milestone` 은 2단계에 CLI 가 붙지만 **지금도 읽을 줄은 안다.**
/// 모르는 값으로 거절하면 새 바이너리가 쓴 파일을 옛 바이너리가 통째로 못 읽는다.
///
/// `Idea` 는 **status 가 아니라 kind 다.** 칸으로 두면 `statuses` 의 맨 앞이
/// 되고 `report::ready` 가 그 칸을 "지금 집을 수 있는 것" 으로 읽어, 담아 둔
/// 생각이 전부 집을 일로 올라온다. 그것을 막으려면 "집을 수 있는 첫 칸" 이라는
/// 둘째 어휘가 필요한데, 종류로 두면 `is_work` 가 이미 문지기다 — 묶음이
/// 보드와 ready 에서 빠지는 그 길을 그대로 탄다.
///
/// **새 값을 더하면 옛 바이너리는 그 줄을 못 읽는다.** 모르는 값을 기본값으로
/// 접지 않기 때문이고, 접으면 되쓰기 한 번에 원래 값이 조용히 사라진다.
/// 못 읽는 줄은 쓰기를 막지도 않고 사라지지도 않는다 — `store::with_write` 가
/// 그대로 들고 되쓰고, `moai status` 가 어느 줄인지 낸다 (moai-dcee).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Issue,
    Epic,
    Milestone,
    Idea,
}

impl Kind {
    fn is_default(&self) -> bool {
        *self == Kind::Issue
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Issue => "issue",
            Kind::Epic => "epic",
            Kind::Milestone => "milestone",
            Kind::Idea => "idea",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;
    fn from_str(s: &str) -> Result<Kind, String> {
        match s {
            "issue" => Ok(Kind::Issue),
            "epic" => Ok(Kind::Epic),
            "milestone" => Ok(Kind::Milestone),
            "idea" => Ok(Kind::Idea),
            _ => Err(format!("`{s}` 는 종류가 아니다. 종류: issue, epic, milestone, idea")),
        }
    }
}

/// 칸반 컬럼. **enum 이 아니다** — 설정으로 칸을 더할 수 있어야 하기 때문이다.
///
/// 검증은 **쓰기에만** 한다. 읽기에서 거절하면 설정에서 칸 하나를 지운 순간
/// 파일 전체가 안 읽히고, 무엇이 문제인지 볼 방법까지 같이 사라진다.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Status(pub String);

impl Status {
    pub fn new(s: impl Into<String>) -> Status {
        Status(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// 코드가 상태에 묻는 사실상 유일한 질문.
    pub fn is_done(&self) -> bool {
        self.0 == crate::config::DONE
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `.moai/issues.jsonl` 의 한 줄.
///
/// **키 순서가 곧 직렬화 순서다** — `serde_json::Value` 로 쓰면 알파벳으로
/// 정렬돼 `id`·`title` 이 줄 가운데로 밀린다. struct 로 써야 `cut -c1-100`
/// 만으로 파일이 읽힌다. 그리고 `body` 는 유일하게 길어질 수 있어 맨 뒤다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Kind::is_default")]
    pub kind: Kind,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// 담당의 메일. **이름과 한 문자열로 합쳐 두지 않는다** — 합쳐 두면 표기
    /// 방법을 바꾸려 할 때 이미 쓴 줄을 도로 갈라야 하고, 이름에 괄호가 든
    /// 사람에서 그 가르기가 틀린다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee_email: Option<String>,
    /// 소속. **파생이 아니라 필드다** — 부모-자식(id 의 점)과 직교한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    /// 더 큰 소속. 에픽과도 직교한다 — 에픽에 안 붙은 이슈가 마일스톤에는
    /// 붙을 수 있고, 그 반대도 된다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// 나를 막고 있는 것들. **막히는 쪽에 둔다** — 막는 쪽을 닫을 때 이 줄만
    /// 쓰면 된다. 막는 쪽에 두면 닫을 때 상대 줄도 써야 하고, 그건 파생값을
    /// 저장하는 것과 같은 실패다.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    /// 미뤄 둔 때. **세 번째 축이다** — `kind` 는 무엇인가, `status` 는 어디
    /// 있는가, 이것은 *지금 볼 것인가*.
    ///
    /// **칸으로 두지 않은 까닭.** `deferred` 칸은 첫 칸이 아니라 `ready` 에는
    /// 안 걸리지만, `wip_overload` 와 `stale_progress` 가 그것을 "벌여 놓은
    /// 것" 과 "집어 놓고 잊은 것" 으로 센다 — 미룬 것은 정의상 안 건드리는
    /// 것이라 미룰수록 잔소리가 는다. 막으려면 config 에 "이 칸은 벌여 놓은
    /// 것이 아니다" 라는 둘째 어휘가 필요하고, `DONE` 하나로 버티는 이유가
    /// 바로 그 둘째 어휘를 안 만들기 위해서다.
    ///
    /// **종류로 두지 않은 까닭.** `Kind::Deferred` 로 옮기면 그 줄이 원래
    /// 무엇이었는지를 잃는다. idea 는 새 이슈를 낳고 제가 닫히니 괜찮았지만,
    /// 미룬 것은 **같은 줄이 그대로 돌아와야 한다.**
    ///
    /// **bool 이 아니라 시각인 까닭.** "언제부터 미뤄 뒀나" 를 `status` 가
    /// 말해야 잊은 것과 막 미룬 것이 갈린다. `status_since` 는 못 쓴다 —
    /// 미루는 것은 칸을 옮기는 일이 아니다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_at: Option<String>,
    /// **미루거나 도로 집은 때**(moai-l11z) — `defer` 와 `defer --undo` 가 적는다.
    ///
    /// `--worktree` 가 같은 id 의 두 줄 중 무엇을 세울지 [`Issue::planned`] 로 견준다. 칸
    /// 시각(`status_since`)으로만 견주면 옆에서 늦게 미룬 것이 여기서 먼저 옮긴 칸에 가려진다.
    /// 그렇다고 미루기가 `status_since` 를 올리면 방치 경고의 시계가 새로 서서 미루기가 경고를
    /// 지우는 손잡이가 된다 — 그래서 따로 둔다. `deferred_at` 으로는 못 한다: 도로 집으면 지워져
    /// 시각이 안 남는다.
    ///
    /// **칸 이동은 안 적는다** — 그 시각은 `status_since` 가 이미 말하고, 견줄 때 둘 중 늦은
    /// 것을 쓴다. 그래서 이 필드 전의 줄도, 옛 바이너리가 칸만 옮긴 줄도 옳게 선다.
    ///
    /// **겹쳐 보기에만 쓴다. 방치·막힘 시계(`--stale`·`blocked_stale`·묶음의 `Stand::since`)는
    /// 이것을 안 본다**(moai-cxk8). 보면 `defer`→`--undo` 두 번으로 그 경고가 지워져, 이
    /// 필드를 `status_since` 와 따로 둔 까닭이 경고 쪽으로 돌아온다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// `status` 가 마지막으로 바뀐 때. 방치 검사와 "review 에 6일" 이 여기서 나온다.
    pub status_since: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// **시작·끝은 `body` 뒤에, `done_at` 이 먼저다**(리뷰 moai-u5bk.3wq) — 이 필드를 모르는 옛
    /// 바이너리는 둘을 `rest` 에 담아 `body` 뒤에 이름 차례로 되쓴다. 같은 자리·같은 차례로 두어야
    /// 새 바이너리와 옛 바이너리가 번갈아 쓰는 저장소에서 줄마다 헛 diff 가 안 난다.
    ///
    /// **마지막으로 `DONE` 에 든 때**(moai-38mh). 들 때마다 덮고, done 을 떠나도 **지우지 않는다**.
    ///
    /// 되돌렸다 다시 닫으면 마지막 것이다 — 소요(`done_at` − `started_at`)가 되돌린 판까지
    /// 품는다. 지우면 done 을 떠난 줄에서 앞의 판이 통째로 사라지는데, 빈 칸과 0 이 다르듯
    /// "아직 안 끝났다" 와 "그때 끝났었다" 도 다르다(2026-09-18 사용자 결정).
    ///
    /// **지금 끝났는가는 `status` 가 말한다.** 이 값이 섰다고 닫힌 줄이 아니다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<String>,
    /// **처음으로 첫 칸을 떠난 때** — 일을 시작한 때(moai-38mh). 한 번 적고 **덮지 않는다**.
    /// 적는 자리는 [`Issue::move_to`]·[`Issue::arrive`] 둘뿐이다.
    ///
    /// **저널이 아니라 필드인 까닭.** 이 답은 저널의 칸 옮김을 접어야 나오는데, 접는 쪽은
    /// 저널이 빠진 쓰기에서 조용히 틀린다 — "스냅샷 먼저, 저널 나중" 이라 저널만 못 적힌
    /// 쓰기가 있을 수 있고, 통계는 그것을 "안 한 일" 로 읽는다. 여기 두면 칸을 옮기는 그
    /// 쓰기에 같이 실려 갈리지 않는다(2026-09-18 사용자 결정).
    ///
    /// **파생값이 아니다** — 이슈 A 를 옮길 때 A 의 줄에만 쓴다. `status_since` 와 다른 것은
    /// 덮지 않는다는 것 하나고, 그래서 review·done 으로 가도 안 사라진다.
    ///
    /// **모르면 비어 있다.** 이 필드 전에 이미 첫 칸을 떠난 줄(옛 바이너리가 집은 줄 포함)은
    /// 언제 떠났는지 스냅샷이 모른다 — 다음 이동의 시각을 적으면 그 줄을 닫는 순간 시작과 끝이
    /// 같아져 통계가 "0 분에 했다" 로 읽는다. 빈 칸과 0 과 거짓 값은 셋 다 다르다(리뷰 moai-u5bk.3wq).
    ///
    /// **묶음 줄에도 적힌다** — 묶음의 칸은 멤버에서 읽히므로(moai-j3b3) 그 줄의 이 값은
    /// `status`·`status_since` 와 똑같이 "누가 이 줄에 `mv` 를 쳤나" 일 뿐이다. 묶음의 기간은
    /// 멤버의 것으로 잰다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    /// 모르는 필드를 **잃지 않고 되쓴다.**
    ///
    /// 스냅샷은 매 쓰기마다 전체 파일이 다시 써진다. 이것이 없으면 2단계
    /// 필드(`milestone` 등)가 든 파일을 1단계 바이너리가 한 번 건드리는 것만으로
    /// 1만 줄에서 그 필드가 조용히 사라진다. 조용한 손실이 이 설계가 못 견디는
    /// 유일한 실패 모드다. 대신 `moai status` 가 "모르는 필드를 들고 있다" 를 비춘다.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rest: BTreeMap<String, serde_json::Value>,
}

/// 태그 표기를 하나로 맞춘다. 앞의 `#` 은 있어도 없어도 같은 태그고,
/// **대소문자도 가리지 않는다.**
///
/// 소문자로 접는 이유 — 태그를 만드는 것이 주로 에이전트고, 같은 개념을
/// 매번 `Bug`·`bug` 로 달리 적는다. 접지 않으면 한 개념이 둘로 갈라져
/// 어느 `-t` 로도 한 번에 못 찾는다.
///
/// **쓰는 쪽(`add`·`edit`)과 찾는 쪽(`query`)이 같은 함수를 쓴다.** 둘이
/// 갈라지면 방금 붙인 태그를 같은 낱말로 찾지 못한다.
pub fn normalize_tag(raw: &str) -> String {
    raw.trim().trim_start_matches('#').trim().to_lowercase()
}

impl Issue {
    pub fn new(id: String, title: String, kind: Kind, status: Status, at: &str) -> Issue {
        Issue {
            id,
            title,
            kind,
            status,
            priority: None,
            tags: Vec::new(),
            assignee: None,
            assignee_email: None,
            epic: None,
            milestone: None,
            blocked_by: Vec::new(),
            deferred_at: None,
            planned_at: None,
            created_at: at.to_string(),
            updated_at: at.to_string(),
            status_since: at.to_string(),
            started_at: None,
            done_at: None,
            body: None,
            rest: BTreeMap::new(),
        }
    }

    pub fn priority(&self) -> u8 {
        self.priority.unwrap_or(DEFAULT_PRIORITY)
    }

    /// 계획에서의 자리가 마지막으로 바뀐 때 — 칸을 옮긴 때(`status_since`)와 미루거나 도로
    /// 집은 때(`planned_at`) 중 늦은 것. 시각은 RFC3339 UTC 고정폭이라 문자열로 견준다.
    pub fn planned(&self) -> &str {
        match self.planned_at.as_deref() {
            Some(p) if p > self.status_since.as_str() => p,
            _ => &self.status_since,
        }
    }

    /// 지금 계획에서 빠져 있는가.
    pub fn is_deferred(&self) -> bool {
        self.deferred_at.is_some()
    }

    /// 칸을 옮긴다 — 칸·칸 시각·수정 시각과 **시작·끝 시각**(moai-38mh)을 한 번에 적고, 떠난 칸을
    /// 돌려준다.
    ///
    /// **칸을 옮기는 쓰기는 다 이 길을 지난다**(`mv`·`idea promote`). 길마다 손으로 적던 때는
    /// `promote` 가 시각을 빠뜨려, 같은 닫기가 어느 동사로 했느냐에 따라 `done_at` 이 서기도 안
    /// 서기도 했다(리뷰 moai-u5bk.3wq).
    ///
    /// 시작은 **줄이 처음으로 칸을 옮기며 첫 칸을 떠날 때** 한 번 적는다. 칸이 한 번이라도
    /// 바뀌었는지는 `status_since` 가 아직 `created_at` 인지로 안다 — 칸 시각을 옮기는 것은 칸
    /// 옮기기뿐이다. 그래서 이 필드 전에 집었던 줄(첫 칸에 도로 와 있어도), 옛 바이너리가 옮긴
    /// 줄, 설정에서 이름이 바뀐 칸에 선 줄은 **언제 시작했는지 모르므로 비워 둔다** — 이번 이동의
    /// 시각을 적으면 그 줄을 닫는 순간 시작과 끝이 같아진다(`started_at` 의 문서).
    ///
    /// 끝은 `done` 에 들 때마다 덮고, 떠나도 지우지 않는다.
    pub fn move_to(&mut self, to: Status, at: &str, cfg: &crate::config::Config) -> Status {
        let first = cfg.first_status();
        // **덮기 전에** 잰다 — 떠나는 칸과, 칸이 한 번도 안 바뀌었는가.
        let leaves_first = self.status.as_str() == first && self.status_since == self.created_at;
        let was = std::mem::replace(&mut self.status, to);
        self.status_since = at.to_string();
        self.updated_at = at.to_string();
        if self.started_at.is_none() && leaves_first && self.status.as_str() != first {
            self.started_at = Some(at.to_string());
        }
        if self.status.is_done() {
            self.done_at = Some(at.to_string());
        }
        was
    }

    /// 새 줄이 **첫 칸 밖에서** 나면(`add -s`) 만든 때가 곧 첫 칸을 떠난 때고, `done` 에서 나면
    /// 그때 끝났다 — [`Issue::move_to`] 와 같은 뜻을 만드는 쓰기에 적는다. 만드는 쓰기는 다
    /// `store::admit` 을 지나고, 그것이 부른다.
    pub fn arrive(&mut self, cfg: &crate::config::Config) {
        if self.started_at.is_none() && self.status.as_str() != cfg.first_status() {
            self.started_at = Some(self.created_at.clone());
        }
        if self.done_at.is_none() && self.status.is_done() {
            self.done_at = Some(self.created_at.clone());
        }
    }

    /// 쓰기 직전에 한 번. 결정적 출력과 기본값 생략을 여기서 보장한다.
    pub fn normalize(&mut self) {
        for t in self.tags.iter_mut() {
            *t = normalize_tag(t);
        }
        self.tags.retain(|t| !t.is_empty());
        self.tags.sort();
        self.tags.dedup();
        self.blocked_by.sort();
        self.blocked_by.dedup();
        if self.priority == Some(DEFAULT_PRIORITY) {
            self.priority = None; // 기본값을 쓰면 1만 줄이 통째로 diff 에 뜬다
        }
        if self.body.as_deref().is_some_and(str::is_empty) {
            self.body = None;
        }
        // **빈 시각은 없는 시각이다**(moai-38mh) — 손으로 푼 줄의 `"started_at":""` 을 그대로 두면
        // "이미 적혔다" 로 읽혀 `move_to` 가 영영 안 적고, 상세는 빈 시작을 그린다.
        for t in [&mut self.started_at, &mut self.done_at] {
            if t.as_deref().is_some_and(|s| s.trim().is_empty()) {
                *t = None;
            }
        }
        // 담당이 없는데 메일만 남는 것을 막는다. 이름 없는 메일은 어느 화면도
        // 그릴 줄 모르고, 그런 줄은 다음 쓰기까지 조용히 살아 있다.
        //
        // **빈 이름도 없는 이름이다.** `None` 만 보면 손으로 푼 충돌이 남긴
        // `"assignee":""` 한 줄이 이 규칙을 그대로 빠져나가, 상세가
        // ` (raven@buzzni.com)` 를 낸다 — 본문의 빈 문자열을 지우는 것과 같은
        // 이유로 여기서 지운다.
        if self.assignee.as_deref().is_some_and(|a| a.trim().is_empty()) {
            self.assignee = None;
        }
        if self.assignee_email.as_deref().is_some_and(|e| e.trim().is_empty()) {
            self.assignee_email = None;
        }
        if self.assignee.is_none() {
            self.assignee_email = None;
        }
    }

    /// 쓰기는 읽기보다 엄하다. 읽기는 아는 만큼 보여주고, 쓰기는 거부한다.
    pub fn validate(&self, cfg: &crate::config::Config) -> Result<(), String> {
        self.validate_keeping(cfg, false)
    }

    /// [`Issue::validate`] 와 같되, `kept_status` 면 **칸 이름은 다시 묻지 않는다**.
    ///
    /// 엄함은 *지금 쓰는 줄*에 대한 것이고(CLAUDE.md), 그 안에서도 **이번에 쓰는 값**에
    /// 대한 것이다 — `config` 에서 칸 이름을 고치면 옛 이름에 선 줄이 남는데, 그 줄을
    /// 미루거나(`defer`) 제목만 고치려 해도 칸 이름 때문에 막히면 그 줄은 도구 안에서
    /// 영영 못 만진다(moai-hym7, 사람이 정했다). 칸을 **옮기는** 쓰기는 그대로 엄하다 —
    /// 갈 칸은 바뀌는 값이라 `kept_status` 가 서지 않는다.
    pub fn validate_keeping(&self, cfg: &crate::config::Config, kept_status: bool) -> Result<(), String> {
        if !kept_status {
            cfg.require_known(self.status.as_str()).map_err(|e| format!("{}: {e}", self.id))?;
        }
        self.validate_fields()
    }

    /// 칸 이름을 뺀 나머지 검사. **설정이 필요 없는 쪽이다.**
    ///
    /// 갈라 둔 까닭은 머지 드라이버다(moai-x2vs) — git 이 주는 것은 임시 파일 셋뿐이라
    /// 그쪽은 `.moai/config.toml` 을 안 읽는데, 제가 지어 내보내는 줄은 검사해야 한다.
    /// `kept_status` 가 선 [`Issue::validate_keeping`] 은 사실 `cfg` 를 한 번도 안 쓰므로,
    /// 그 사실을 타입으로 적어 두면 검사가 두 벌로 갈라질 자리가 없다.
    pub fn validate_fields(&self) -> Result<(), String> {
        if !crate::id::is_valid(&self.id) {
            return Err(format!("id 형식이 아니다 — {:?}", self.id));
        }
        let t = self.title.trim();
        if t.is_empty() {
            return Err(format!("{}: 제목이 비었다", self.id));
        }
        if self.title.contains('\n') {
            return Err(format!("{}: 제목은 한 줄이다", self.id));
        }
        // 담당도 한 줄이다. `view::history` 와 상세는 "원소 하나가 한 줄" 로
        // 서 있어서, 담당에 든 줄바꿈 하나가 뒤따르는 줄의 열을 통째로 잃게
        // 한다 — 제목에 같은 규칙이 있는 것과 같은 이유다.
        //
        // **메일도 같이 잰다.** 화면에 나가는 것은 둘을 합친 한 줄이라, 어느
        // 쪽에 든 줄바꿈이든 같은 자리를 부순다.
        for (what, v) in [("담당", &self.assignee), ("담당 메일", &self.assignee_email)] {
            if let Some(v) = v
                && v.contains(['\n', '\r'])
            {
                return Err(format!("{}: {what}은 한 줄이다 — {v:?}", self.id));
            }
        }
        if self.priority.is_some_and(|p| p > MAX_PRIORITY) {
            return Err(format!(
                "{}: 우선순위는 0~{MAX_PRIORITY} 다 — {:?}",
                self.id, self.priority
            ));
        }
        for tag in &self.tags {
            if tag.is_empty() || tag.contains(|c: char| c.is_whitespace() || c == ',') {
                return Err(format!("{}: 태그에 공백이나 쉼표를 넣지 않는다 — {tag:?}", self.id));
            }
        }
        for (what, v) in [("에픽", &self.epic), ("마일스톤", &self.milestone)] {
            if let Some(v) = v
                && !crate::id::is_valid(v)
            {
                return Err(format!("{}: {what} id 형식이 아니다 — {v:?}", self.id));
            }
        }
        // **마일스톤 줄은 다른 마일스톤에 들지 않는다**(moai-bg55, 사용자와 정함). 그 필드는
        // 소속으로 안 센다(`report::milestones`, moai-jwnr) — 조용히 받으면 적은 사람은 걸린 줄
        // 안다. 지금 쓰는 줄만 잰다(`store::with_write`): 이미 적힌 옛 줄은 읽히고, 그 줄을
        // 손댈 때 비우는 길을 댄다.
        if self.kind == Kind::Milestone && self.milestone.is_some() {
            return Err(format!(
                "{}: 마일스톤은 다른 마일스톤에 들지 않는다 — 마일스톤은 뿌리에 선다. 만들 때면 `--milestone` 을 빼고, 이미 적힌 줄이면 `moai edit {} --milestone none` 으로 비운다",
                self.id, self.id
            ));
        }
        for b in &self.blocked_by {
            if b == &self.id {
                return Err(format!("{}: 스스로를 막을 수 없다", self.id));
            }
            if !crate::id::is_valid(b) {
                return Err(format!("{}: blocked_by id 형식이 아니다 — {b:?}", self.id));
            }
        }
        Ok(())
    }
}

/// `.moai/journal.jsonl` 의 한 줄. **추가만 한다.**
///
/// 적는 것은 `create`·`status`·`note`·`rm` 넷뿐이다. 필드 변경을 적기
/// 시작하면 이 파일은 이벤트 로그가 되고, 그러면 "스냅샷 대신 이걸 접으면
/// 되지 않나" 가 반드시 돌아온다. **저널은 상태 계산에 읽히지 않는다.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// 맨 앞이라 `sort` 가 그대로 먹는다.
    pub ts: String,
    pub id: String,
    /// enum 이 아니다 — 모르는 kind 는 읽는 쪽이 건너뛴다. 안 읽혀도 상태가
    /// 안 틀리는 유일한 파일이라 여기만 관대해도 된다.
    pub kind: String,
    pub by: String,
    /// 옛 줄에는 없다. `by` 를 객체로 바꾸지 않은 이유가 이것이다 — 타입을
    /// 바꾸면 `journal_of` 의 파싱이 옛 줄에서 실패하고, 실패한 줄은 조용히
    /// 버려져 `moai show` 의 이력이 통째로 사라진다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl JournalEntry {
    fn base(kind: &str, id: &str, at: &str, by: &Actor) -> JournalEntry {
        JournalEntry {
            ts: at.to_string(),
            id: id.to_string(),
            kind: kind.to_string(),
            by: by.name.clone(),
            by_email: Some(by.email.clone()),
            from: None,
            to: None,
            title: None,
            text: None,
            note: None,
        }
    }
    pub fn create(id: &str, title: &str, at: &str, by: &Actor) -> JournalEntry {
        JournalEntry { title: Some(title.to_string()), ..Self::base("create", id, at, by) }
    }
    pub fn status(id: &str, from: &Status, to: &Status, note: Option<String>, at: &str, by: &Actor) -> JournalEntry {
        JournalEntry {
            from: Some(from.0.clone()),
            to: Some(to.0.clone()),
            note,
            ..Self::base("status", id, at, by)
        }
    }
    pub fn note(id: &str, text: &str, at: &str, by: &Actor) -> JournalEntry {
        JournalEntry { text: Some(text.to_string()), ..Self::base("note", id, at, by) }
    }
    pub fn removed(id: &str, title: &str, at: &str, by: &Actor) -> JournalEntry {
        JournalEntry { title: Some(title.to_string()), ..Self::base("rm", id, at, by) }
    }
}

// ── 일한 것 ────────────────────────────────────────────────────────────

/// 이 일을 한 AI 한 줄. 노트의 `model:` 줄에서 읽는다(moai-8f2g).
///
///     model: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)
///
/// **저장하지 않는다.** 스냅샷에도 저널에도 이 모양의 필드가 없다 — 적는 것은 여전히
/// `moai note` 한 줄이고, 이것은 그 글을 읽어 낸 값이다(2026-09-18 사용자 결정, 길 1).
/// 꼴이 틀린 것으로 드러나도 이 파서와 규약 글만 고치면 되고 쌓인 줄은 그대로 남는다.
///
/// 빈 자리는 `None` 이다. **0 과 모름은 다르다** — 토큰을 모르면 `tokens=` 를 빼고,
/// 여기서 `None` 이 된다. 0 을 적으면 0 이다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Work {
    pub provider: Option<String>,
    pub model: String,
    pub tokens: Option<u64>,
    pub grade: Option<String>,
    pub why: Option<String>,
    /// 그 줄을 적은 저널 줄의 것. 일을 끝낸 때가 아니라 **적은 때**다.
    pub at: String,
    pub by: String,
    pub by_email: Option<String>,
}

/// 저널에서 일한 것을 모은다 — 노트와 칸 옮김의 `-m` 둘 다, 한 글 안의 줄마다.
///
/// **읽기는 관대하다.** 꼴에 안 맞는 줄은 값이 안 될 뿐 노트로 그대로 남는다 — 이력에서
/// 빠지는 것은 없다. 옛 줄 `model: opus-5 (medium — …)` 은 회사 없이 읽힌다. 모델 이름에서
/// 회사를 짐작해 채우지 않는다 — 짐작한 값은 통계에서 적힌 값과 갈리지 않는다.
///
/// **울타리(```` ``` ````·`~~~`) 안의 줄은 예다.** 리뷰 원문과 결정 노트가 꼴을 그대로 옮겨
/// 적으니, 그것을 세면 아무도 안 한 일이 토큰째 통계에 선다.
pub fn work_of(journal: &[JournalEntry]) -> Vec<Work> {
    let mut out = Vec::new();
    for e in journal {
        let text = match e.kind.as_str() {
            "note" => e.text.as_deref(),
            "status" => e.note.as_deref(),
            _ => None,
        };
        let mut fenced = false;
        for line in text.unwrap_or_default().lines() {
            let bare = line.trim_start();
            if bare.starts_with("```") || bare.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if let Some(w) = parse_work(line) {
                out.push(Work { at: e.ts.clone(), by: e.by.clone(), by_email: e.by_email.clone(), ..w });
            }
        }
    }
    out
}

/// 저널 한 줄 — **풀기 전의 JSON** — 이 [`work_of`] 에서 값을 낼 수 **있는가**(moai-p8qj). 목록의
/// `--json` 이 저널을 통째로 풀지 않으려고 먼저 거르는 자다(리뷰 moai-u5bk.3wq).
///
/// **값을 내는 줄은 빠뜨리지 않는다.** [`parse_work`] 는 `model:` 로 시작한 글줄만 받으니, 값을
/// 내는 저널 줄의 글에는 `model:` 이 든다. JSON 은 그 글자들을 이스케이프하지 않고 그대로 쓴다 —
/// 이스케이프는 따옴표·역슬래시·제어문자와 `\uXXXX` 뿐이고, 앞의 것들은 `model:` 의 어느 글자도
/// 못 낸다. 그래서 날것에 `model:` 도 `\u` 도 없는 줄은 값을 못 낸다. 손으로 쓴 줄이 글자를
/// `\uXXXX` 로 적었을 수 있어 `\u` 가 든 줄은 다 푼다. 더 받는 것은 괜찮다 — 푼 뒤에
/// [`work_of`] 가 다시 가른다.
pub fn may_hold_work(raw: &str) -> bool {
    raw.contains("model:") || raw.contains("\\u")
}

/// `model: [<회사>/]<모델> [tokens=<수>] [(<등급> — <까닭>)] …` 한 줄. `at`·`by` 는 비워 낸다.
///
/// 괄호 뒤는 무엇이 와도 받는다 — 옛 줄이 거기에 사연을 붙였다. 괄호 **앞**에 모르는 것이
/// 끼면 안 받는다: `tokens 182000` 같은 오타를 모델만 읽고 넘기면 토큰이 조용히 "모름" 이 된다.
/// 같은 까닭으로 **제자리를 벗어난 `tokens=<수>`**(괄호 안·괄호 뒤)도 안 받는다.
///
/// **줄 머리에서 시작한 줄만 받는다.** 들여 쓴 줄은 마크다운의 코드 덩이 — 꼴을 옮겨 적은 예다.
pub(crate) fn parse_work(line: &str) -> Option<Work> {
    let rest = line.strip_prefix("model:")?;
    // `model::actor` 로 시작하는 글줄을 거른다.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let word_end = |s: &str| s.find(|c: char| c.is_whitespace() || c == '(').unwrap_or(s.len());
    let rest = rest.trim_start();
    let head = &rest[..word_end(rest)];
    let (provider, model) = match head.split_once('/') {
        None => (None, head),
        Some((p, m)) if !p.is_empty() && !m.is_empty() && !m.contains('/') => (Some(p.to_string()), m),
        Some(_) => return None,
    };
    // 글줄 끝의 문장 부호는 이름이 아니다 — `opus-5.` 과 `opus-5` 가 통계에서 갈린다.
    let model = model.trim_end_matches(['.', ',', ';', ':']);
    // `model: tokens=3` 은 모델을 빠뜨린 줄이지 모델 이름이 `tokens=3` 인 줄이 아니다.
    if model.is_empty() || head.contains('=') {
        return None;
    }
    let mut rest = rest[head.len()..].trim_start();
    let mut tokens = None;
    if let Some(t) = rest.strip_prefix("tokens=") {
        let end = word_end(t);
        // 숫자만 받는다 — `u64::from_str` 은 `+5` 도 받는다.
        let n = &t[..end];
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        tokens = Some(n.parse::<u64>().ok()?);
        rest = t[end..].trim_start();
    } else if rest
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ','))
        .any(|w| w.strip_prefix("tokens=").is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit())))
    {
        // `(high) tokens=5` 처럼 제자리를 벗어난 토큰을 모델만 읽고 넘기면 토큰이 조용히 "모름" 이 된다.
        return None;
    }
    let (mut grade, mut why) = (None, None);
    if let Some(inner) = rest.strip_prefix('(') {
        let mut depth = 1;
        let close = inner.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        })?;
        let inner = inner[..close].trim();
        // 긴 줄표가 꼴이고, 반각 줄표(`–`, 자동 고침이 바꿔 놓는다)와 손으로 친 붙임표도 받는다.
        let (g, w) = match inner.split_once(['—', '–']).or_else(|| inner.split_once(" - ")) {
            Some((g, w)) => (g.trim(), w.trim()),
            None => (inner, ""),
        };
        // 등급은 한 낱말이다(`low`…`max`). 낱말이 여럿이면 등급을 적지 않은 사연으로 읽는다.
        if g.contains(char::is_whitespace) {
            why = Some(inner.to_string());
        } else {
            grade = (!g.is_empty()).then(|| g.to_string());
            why = (!w.is_empty()).then(|| w.to_string());
        }
    } else if !rest.is_empty() {
        return None;
    }
    Some(Work {
        provider,
        model: model.to_string(),
        tokens,
        grade,
        why,
        at: String::new(),
        by: String::new(),
        by_email: None,
    })
}

// ── 누가 ──────────────────────────────────────────────────────────────

/// 일을 한 사람. 이름과 메일이 **함께** 다닌다 — 이름만으로는 같은 이름이
/// 둘일 때 갈라지지 않고, 메일만으로는 화면에 읽을 것이 없다.
#[derive(Debug, Clone, PartialEq)]
pub struct Actor {
    pub name: String,
    pub email: String,
}

impl Actor {
    /// `이름 (메일)` 한 덩이를 가른다. 괄호 앞 공백은 있어도 없어도 된다.
    ///
    /// 뒤에서부터 여는 괄호를 찾는다 — 이름 안에 괄호가 있는 사람이 실제로 있고,
    /// 앞에서 찾으면 그 괄호에서 잘린다.
    pub fn parse(raw: &str) -> Option<Actor> {
        let (name, email) = raw.trim().strip_suffix(')')?.rsplit_once('(')?;
        let a = Actor { name: name.trim().to_string(), email: email.trim().to_string() };
        a.is_sane().then_some(a)
    }

    /// 이 사람을 담당 칸에 넣는 모양 — `(이름, 메일)`, **갈라진 채로.**
    ///
    /// **만든 사람이 담당이다**는 규칙이 CLI 의 `add` 와 탐색기의 `n` 에 같이 선다.
    /// 합친 한 줄(`label`)로 넘기면 되가르는 쪽이 이름 없는 줄의 메일을 이름 칸에 넣는다.
    pub fn as_assignee(&self) -> (Option<String>, Option<String>) {
        (Some(self.name.clone()), Some(self.email.clone()))
    }

    /// 이름도 **한 줄이다.** 메일만 재고 이름을 안 재면 줄바꿈이 든 `--user` 가
    /// 통과해, `note`·`mv` 가 두 줄짜리 `by` 를 되돌릴 수 없는 저널에 적는다.
    /// `add` 쪽은 더 나쁘다 — id 를 뽑은 **뒤에** `담당은 한 줄이다` 로 죽어서,
    /// 만들어지지도 않은 id 가 오류에 실려 나간다.
    fn is_sane(&self) -> bool {
        !self.name.is_empty()
            && !self.name.contains(['\n', '\r'])
            && !self.email.is_empty()
            && self.email.contains('@')
            && !self.email.contains(char::is_whitespace)
    }
}

/// 이름과 메일을 한 줄로 합치는 **유일한 곳.**
///
/// 합치는 곳이 `view` 와 `tui` 로 갈라지면 그때 둘이 서로 다른 모양을 내고,
/// 한쪽을 고친 사람이 다른 쪽을 못 찾는다.
pub fn label(name: &str, email: Option<&str>, how: crate::config::Naming) -> String {
    use crate::config::Naming;
    let email = email.map(str::trim).filter(|e| !e.is_empty());
    match (how, email) {
        (_, None) => name.to_string(),
        // 이름이 빈 줄은 어느 모양에서도 메일로 낸다. 저널은 정규화를 거치지
        // 않아 옛 줄의 `"by":""` 를 고칠 길이 여기뿐이고, 빈칸을 내면 그 줄이
        // 누구의 것인지 화면에서 사라진다.
        (_, Some(e)) if name.trim().is_empty() => e.to_string(),
        (Naming::Name, _) => name.to_string(),
        (Naming::Email, Some(e)) => e.to_string(),
        (Naming::Full, Some(e)) => format!("{name} ({e})"),
    }
}

/// `-a` 로 받은 한 덩이를 담당 이름과 메일로 가른다. 비었으면 담당 없음이다.
///
/// `이름 (메일)` 이면 갈라 넣고, 이름만이면 이름만 넣는다 — 남의 메일을 모르는
/// 채로 남에게 맡기는 일이 실제로 있어 여기서는 메일을 요구하지 않는다.
pub fn split_assignee(raw: &str) -> (Option<String>, Option<String>) {
    let raw = raw.trim();
    if raw.is_empty() {
        return (None, None);
    }
    match Actor::parse(raw) {
        Some(a) => (Some(a.name), Some(a.email)),
        None => (Some(raw.to_string()), None),
    }
}

/// 테스트가 쓰는 사람 하나. 저널 모양을 보는 테스트가 여러 파일에 흩어져 있어
/// 한 곳에 둔다.
#[cfg(test)]
pub fn someone(name: &str) -> Actor {
    Actor { name: name.to_string(), email: format!("{name}@example.com") }
}

/// 누가 하는가. `--user` → `MOAI_ACTOR` → `git config user.name`+`user.email`.
///
/// 셋 다 없으면 **멈춘다.** 예전에는 `unknown` 으로 적었는데, 그렇게 쌓인 줄은
/// 나중에 누구도 되짚지 못한다 — 이력이 남는 것이 목적인 파일에 이름 없는 줄을
/// 채우느니 한 번 물어보는 편이 싸다. 막는 것은 *사람을 부르는 게이트가 아니라*
/// 입력이 모자라다는 말이고, `--user` 와 `MOAI_ACTOR` 둘 다 사람 없이 채워진다.
///
/// **`root` 는 그 트래커의 `.moai` 뿌리다**(moai-d3sy). git 설정을 거기서 읽어야 이슈가 선 프로젝트와
/// 사람이 같은 자에서 온다 — 뿌리 밑에 딴 저장소가 겹쳐 있을 때 부른 자리가 사람을 정하면, 그 저널에
/// 남의 이름이 영구히 남는다. `--user`·`MOAI_ACTOR` 는 뿌리와 상관없이 그대로 이긴다.
pub fn actor(flag: Option<&str>, root: &Path) -> R<Actor> {
    // 플래그는 비어 있어도 **준 것이다.** `--user "$NAME"` 에서 변수가 비었을 때
    // 조용히 git 설정으로 넘어가면 엉뚱한 사람 이름으로 저널이 쌓인다 — 이
    // 기능이 막으려던 바로 그 실패다. 환경변수는 다르다: 빈 값은 관례상 없는 것이다.
    if let Some(raw) = flag.map(str::trim) {
        return Actor::parse(raw).ok_or_else(|| malformed("--user", raw));
    }
    if let Ok(raw) = std::env::var("MOAI_ACTOR")
        && !raw.trim().is_empty()
    {
        return Actor::parse(raw.trim()).ok_or_else(|| malformed("MOAI_ACTOR", raw.trim()));
    }
    match (git_config(root, "user.name"), git_config(root, "user.email")) {
        // git 이 준 값도 `--user` 와 **같은 자로 잰다.** 한쪽만 통과시키면
        // `--user "레이븐 (raven)"` 은 거절당하는데 `user.email = raven` 은
        // 통과해, 이 도구가 스스로 모양이 아니라고 부르는 값이 되돌릴 수 없는
        // 저널에 영구히 쌓인다.
        (Some(name), Some(email)) => {
            let a = Actor { name, email };
            if a.is_sane() { Ok(a) } else { Err(bad_git_identity(&a)) }
        }
        _ => Err(Fail::coded(NO_ACTOR, code::NO_ACTOR)),
    }
}

const NO_ACTOR: &str = "\
누가 하는지 모른다 — git 사용자 정보가 없다.

  git config user.name  \"이름\"
  git config user.email \"메일\"

이번만 손으로 준다면:  --user \"이름 (메일)\"";

/// 고칠 곳이 argv 가 아니라 설정이라 `NO_ACTOR` 와 같은 코드를 쓴다 — 받는
/// 쪽은 "사용자 정보를 손봐라" 하나로 두 경우를 같이 다룰 수 있어야 한다.
fn bad_git_identity(a: &Actor) -> Fail {
    Fail::coded(
        format!(
            "git 사용자 정보가 `이름 (메일)` 로 쓸 수 없는 모양이다 — {:?}\n\n  \
             git config user.name  \"이름\"\n  git config user.email \"메일\"",
            label(&a.name, Some(&a.email), crate::config::Naming::Full)
        ),
        code::NO_ACTOR,
    )
}

fn malformed(what: &str, raw: &str) -> Fail {
    Fail::coded(
        format!("{what} 가 `이름 (메일)` 모양이 아니다 — {raw:?}"),
        code::BAD_INPUT,
    )
}

/// git 저장소 밖에서도 전역 설정을 읽는다 — moai 는 `.moai/` 만 찾지 git 을
/// 요구하지 않으므로, `git init` 전에도 이 값이 있을 수 있다.
///
/// git 은 [`crate::git::command`] 로 띄운다 — 물려받은 저장소 변수를 걷는 자리가
/// 거기 하나여야, 새 시험이 이 길로 사람을 물어도 바깥 저장소의 이름을 읽지
/// 않는다(moai-g1a3). 릴리스도 거기서 걷는다(moai-ztdf) — 훅 안에서 부른
/// `moai -C <다른 프로젝트>` 가 훅 저장소의 이름을 그 프로젝트 저널에 영구히
/// 적던 자리가 여기다.
///
/// **`-C <뿌리>` 를 댄다**(moai-d3sy). 그 이슈가 어느 프로젝트의 것인지는 `.moai`
/// 뿌리가 정하므로, 사람도 같은 자에서 와야 한다. 프로세스 자리로 읽던 때는 뿌리
/// **밑에** 딴 저장소가 있으면(서브모듈·vendor) 사람은 그쪽에서, 커밋 칸은
/// `-C <뿌리>` 로 이쪽에서 와 한 명령이 두 저장소를 봤다.
///
/// 뿌리가 git 저장소가 아니어도 된다 — git 은 거기서 위로 찾고, 끝내 못 찾으면
/// 전역 설정을 낸다. moai 는 `.moai/` 만 찾지 git 을 요구하지 않는다.
fn git_config(root: &Path, key: &str) -> Option<String> {
    let out = crate::git::command().arg("-C").arg(root).args(["config", key]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!v.is_empty()).then_some(v)
}

// ── 시각 ──────────────────────────────────────────────────────────────
//
// 타임스탬프는 `String` 이다. 구조체로 들고 있으면 라운드트립 바이트 동일성을
// 보장하기 어렵고(`Z` vs `+00:00`, 나노초) 멱등성 테스트가 그것부터 잡는다.
// RFC3339 UTC 고정폭이라 문자열 비교로 정렬이 맞는다.

/// 지금. `MOAI_NOW` 가 있으면 그것을 쓴다 — 시계를 고정해야 "3일 전" 을 시험한다.
pub fn now() -> String {
    if let Ok(t) = std::env::var("MOAI_NOW")
        && !t.trim().is_empty()
    {
        return t.trim().to_string();
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_rfc3339(secs)
}

/// 1970-01-01 부터의 날 수를 (년, 월, 일) 로. Howard Hinnant 의 `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn format_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-09-11T04:12:03Z` → epoch 초. 형식이 아니면 `None`.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0, 4)?, n(5, 7)? as u32, n(8, 10)? as u32);
    let (h, mi, se) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

/// `at` 이 `now` 로부터 며칠 전인가. 파싱이 안 되면 `None`.
///
/// **`at` 이 `now` 보다 뒤면 0 이다**(moai-fix6). `div_euclid` 는 몇 초만 늦어도 -1 을 돌려,
/// `--worktree` 로 겹친 다른 기계의 시계가 조금 빠르면 목록에 "-1일" 이 섰다. 그 줄은 오늘
/// 적힌 것이다. 방치 판정은 안 바뀐다 — 0 은 어느 문턱도 안 넘는다.
pub fn days_since(at: &str, now: &str) -> Option<i64> {
    Some((parse_rfc3339(now)? - parse_rfc3339(at)?).div_euclid(86_400).max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn issue() -> Issue {
        Issue::new(
            "argos-4aex".into(),
            "제목".into(),
            Kind::Issue,
            Status::new("todo"),
            "2026-09-11T04:12:03Z",
        )
    }

    /// [`fit`] 은 **넘을 때만** 줄이고, 줄일 때는 글자 가운데를 안 자른다. 어느 답도
    /// `budget` 을 안 넘는다 — 넘치게 내면 [`check_text_size`] 가 도로 거절해 아무것도 못 고친다.
    #[test]
    fn fit_shortens_only_what_must_and_never_mid_character() {
        assert_eq!(fit("가나다", 9), "가나다", "딱 맞는 글을 줄였다");
        assert_eq!(fit("", 0), "");
        // 표식이 3바이트다 — 8바이트 예산에는 다섯 바이트가 남지만 두 글자는 안 들어간다.
        assert_eq!(fit("가나다", 8), "가…");
        assert_eq!(fit("가나다", 6), "가…");
        assert_eq!(fit("가나다", 5), "…", "글자 가운데서 잘랐다");
        // 표식조차 못 담는 예산에서도 넘치지 않는다.
        for budget in 0..10 {
            assert!(fit("가나다", budget).len() <= budget, "{budget}바이트 예산이 넘쳤다");
        }
    }

    /// 최소 형태는 필수 6개뿐이고 키 순서가 고정이다.
    #[test]
    fn minimal_line_omits_everything_optional() {
        let line = serde_json::to_string(&issue()).unwrap();
        assert_eq!(
            line,
            r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#
        );
    }

    #[test]
    fn full_line_keeps_declared_key_order() {
        let mut i = issue();
        i.kind = Kind::Epic;
        i.priority = Some(1);
        i.tags = vec!["bug".into()];
        i.assignee = Some("claude".into());
        i.epic = Some("argos-9k2p".into());
        i.milestone = Some("argos-m001".into());
        i.body = Some("본문".into());
        i.started_at = Some("2026-09-11T05:00:00Z".into());
        i.done_at = Some("2026-09-11T06:00:00Z".into());
        let line = serde_json::to_string(&i).unwrap();
        let want = [
            "id", "title", "kind", "status", "priority", "tags", "assignee", "epic",
            "milestone", "created_at", "updated_at", "status_since", "body", "done_at", "started_at",
        ];
        let at: Vec<usize> = want
            .iter()
            .map(|k| line.find(&format!("\"{k}\":")).unwrap_or_else(|| panic!("{k} 가 없다 — {line}")))
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{line}");
    }

    /// 읽고 그대로 쓰면 바이트가 같다. 이게 깨지면 매 명령이 헛 diff 를 만든다.
    #[test]
    fn round_trips_byte_identical() {
        for line in [
            r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
            r#"{"id":"argos-9k2p","title":"에픽","kind":"epic","status":"in_progress","priority":1,"tags":["bug"],"assignee":"claude","epic":"argos-0000","milestone":"argos-m001","created_at":"2026-09-10T09:00:00Z","updated_at":"2026-09-11T05:02:44Z","status_since":"2026-09-10T09:30:00Z","body":"여러\n줄"}"#,
            r#"{"id":"argos-4aex","title":"제목","status":"done","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-12T04:00:00Z","status_since":"2026-09-12T04:00:00Z","body":"본문","done_at":"2026-09-12T04:00:00Z","started_at":"2026-09-11T05:00:00Z"}"#,
        ] {
            let i: Issue = serde_json::from_str(line).unwrap();
            assert_eq!(serde_json::to_string(&i).unwrap(), line);
        }
    }

    /// **옛 바이너리가 되쓴 줄과 바이트가 같다**(리뷰 moai-u5bk.3wq). 시작·끝을 모르는 바이너리는
    /// 둘을 `rest` 에 담아 `body` 뒤에 이름 차례로 쓴다 — 새 바이너리가 다른 자리에 쓰면 두 판이
    /// 번갈아 쓰는 저장소에서 찍힌 줄마다 헛 diff 가 난다.
    #[test]
    fn stamps_land_where_an_older_binary_would_put_them() {
        let mut new = issue();
        new.body = Some("본문".into());
        new.started_at = Some("2026-09-11T05:00:00Z".into());
        new.done_at = Some("2026-09-12T04:00:00Z".into());
        let mut old = issue();
        old.body = Some("본문".into());
        old.rest.insert("started_at".into(), serde_json::json!("2026-09-11T05:00:00Z"));
        old.rest.insert("done_at".into(), serde_json::json!("2026-09-12T04:00:00Z"));
        assert_eq!(serde_json::to_string(&new).unwrap(), serde_json::to_string(&old).unwrap());
    }

    /// **시작은 줄이 처음으로 첫 칸을 떠날 때 하나다**(moai-38mh, 리뷰 moai-u5bk.3wq). 칸이 이미 한 번
    /// 바뀐 줄 — 이 필드 전에 집은 줄, 옛 바이너리가 옮긴 줄 — 은 언제 시작했는지 모르므로 비워 둔다.
    /// 다음 이동의 시각을 적으면 그 줄을 닫는 순간 시작과 끝이 같아져 "0 분에 했다" 가 선다.
    #[test]
    fn the_start_is_the_first_departure_and_unknown_stays_empty() {
        let c = cfg();
        let (t1, t2, t3) = ("2026-09-11T05:00:00Z", "2026-09-11T06:00:00Z", "2026-09-11T07:00:00Z");

        let mut fresh = issue();
        assert_eq!(fresh.move_to(Status::new("in_progress"), t1, &c), Status::new("todo"), "떠난 칸을 돌려준다");
        assert_eq!((fresh.status.as_str(), fresh.status_since.as_str(), fresh.updated_at.as_str()), ("in_progress", t1, t1));
        assert_eq!(fresh.started_at.as_deref(), Some(t1));
        fresh.move_to(Status::new("done"), t2, &c);
        fresh.move_to(Status::new("todo"), t3, &c);
        fresh.move_to(Status::new("in_progress"), t3, &c);
        assert_eq!(fresh.started_at.as_deref(), Some(t1), "되집어도 시작은 처음 것이다");
        assert_eq!(fresh.done_at.as_deref(), Some(t2), "떠나도 끝난 때는 안 지운다");

        // 이 필드 전에 review 까지 간 줄 — 닫아도 시작은 모른다. 끝은 선다.
        let mut legacy = issue();
        legacy.status = Status::new("review");
        legacy.status_since = "2026-09-05T00:00:00Z".into();
        legacy.move_to(Status::new("done"), t1, &c);
        assert_eq!((legacy.started_at.as_deref(), legacy.done_at.as_deref()), (None, Some(t1)));

        // 집었다가 첫 칸에 도로 온 옛 줄 — 다시 집은 때는 처음 떠난 때가 아니다.
        let mut back = issue();
        back.status_since = "2026-09-05T00:00:00Z".into();
        back.move_to(Status::new("in_progress"), t1, &c);
        assert_eq!(back.started_at, None, "칸이 이미 바뀐 줄에 다시 집은 때를 적었다");

        // 첫 칸에서 곧바로 닫은 줄 — 떠난 때가 시작이자 끝이다.
        let mut quick = issue();
        quick.move_to(Status::new("done"), t1, &c);
        assert_eq!((quick.started_at.as_deref(), quick.done_at.as_deref()), (Some(t1), Some(t1)));
    }

    /// **첫 칸 밖에서 난 줄은 만든 때에 시작했다**(`add -s`) — `move_to` 와 같은 뜻이다. `done` 에서
    /// 났으면 그때 끝났다. 첫 칸에서 난 줄에는 아무것도 안 적는다.
    #[test]
    fn a_line_born_outside_the_first_column_starts_when_it_is_made() {
        let c = cfg();
        let born = |status: &str| {
            let mut i = issue();
            i.status = Status::new(status);
            i.arrive(&c);
            (i.started_at, i.done_at)
        };
        let made = Some("2026-09-11T04:12:03Z".to_string());
        assert_eq!(born("todo"), (None, None));
        assert_eq!(born("in_progress"), (made.clone(), None));
        assert_eq!(born("done"), (made.clone(), made));
    }

    /// 빈 시각은 없는 시각이다 — `""` 이 남으면 `move_to` 가 "이미 적혔다" 로 읽어 영영 안 적는다.
    #[test]
    fn an_empty_stamp_is_no_stamp() {
        let mut i = issue();
        (i.started_at, i.done_at) = (Some(String::new()), Some(" ".into()));
        i.normalize();
        assert_eq!((i.started_at, i.done_at), (None, None));
    }

    /// **`work` 를 내는 줄은 거르개를 빠짐없이 지난다**(리뷰 moai-u5bk.3wq) — 목록은 거르개에 걸린
    /// 줄만 푼다. 거르개를 좁히다 한 모양이라도 놓치면 목록과 하나를 펼친 쪽의 `work` 가 갈린다.
    #[test]
    fn every_line_that_yields_work_passes_the_prefilter() {
        let by = someone("raven");
        let at = "2026-09-11T04:12:03Z";
        let s = Status::new("todo");
        let d = Status::new("done");
        let yielding = [
            JournalEntry::note("argos-4aex", "model: anthropic/opus-5 tokens=1 (high — 까닭)", at, &by),
            JournalEntry::note("argos-4aex", "앞 줄\nmodel: anthropic/opus-5", at, &by),
            JournalEntry::note("argos-4aex", "앞 줄\r\nmodel: opus-5 (low — 옛 꼴)", at, &by),
            JournalEntry::status("argos-4aex", &s, &d, Some("model: google/gemini-3.8".into()), at, &by),
            JournalEntry::status("argos-4aex", &s, &d, Some("닫는다\n\nmodel: openai/gpt-6 tokens=5".into()), at, &by),
        ];
        for e in &yielding {
            let raw = serde_json::to_string(e).unwrap();
            assert!(!work_of(std::slice::from_ref(e)).is_empty(), "시험이 틀렸다 — 값을 안 내는 줄이다: {raw}");
            assert!(may_hold_work(&raw), "값을 내는 줄을 걸렀다: {raw}");
        }
        // 손으로 쓴 줄이 글자를 이스케이프로 적었어도 푼다.
        assert!(may_hold_work(r#"{"ts":"t","id":"argos-4aex","kind":"note","by":"r","text":"model: opus-5"}"#));
        // `model:` 이 없는 줄은 안 푼다 — 그것이 이 거르개의 값이다.
        let plain = serde_json::to_string(&JournalEntry::note("argos-4aex", "그냥 메모", at, &by)).unwrap();
        assert!(!may_hold_work(&plain), "{plain}");
    }

    /// 계획 시각도 되쓰면 바이트가 같다 — 도로 집은 줄은 `deferred_at` 없이 `planned_at` 만
    /// 든다(moai-l11z). 늦은 계획 시각이 [`Issue::planned`] 가 된다.
    #[test]
    fn round_trips_a_line_with_a_plan_time() {
        for line in [
            r#"{"id":"argos-4aex","title":"제목","status":"todo","deferred_at":"2026-09-12T00:00:00Z","planned_at":"2026-09-12T00:00:00Z","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-12T00:00:00Z","status_since":"2026-09-11T04:12:03Z"}"#,
            r#"{"id":"argos-4aex","title":"제목","status":"todo","planned_at":"2026-09-13T00:00:00Z","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-13T00:00:00Z","status_since":"2026-09-11T04:12:03Z"}"#,
        ] {
            let i: Issue = serde_json::from_str(line).unwrap();
            assert_eq!(serde_json::to_string(&i).unwrap(), line);
            assert_eq!(Some(i.planned()), i.planned_at.as_deref(), "늦은 계획 시각이 계획 자리를 바꾼 때가 아니다");
        }
    }

    /// 뒷 단계가 쓴 필드를 앞 단계 바이너리가 읽고 써도 잃지 않는다.
    ///
    /// 매 쓰기가 전체 재작성이라, 이게 없으면 새 바이너리가 쓴 필드를 옛
    /// 바이너리가 한 번 만지는 것만으로 1만 줄에서 지운다.
    #[test]
    fn unknown_fields_survive() {
        let line = r#"{"id":"argos-4aex","title":"제목","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z","due":"2026-10-01","estimate":90}"#;
        let i: Issue = serde_json::from_str(line).unwrap();
        assert_eq!(i.rest.len(), 2, "{:?}", i.rest);
        let out = serde_json::to_string(&i).unwrap();
        assert!(out.contains(r#""due":"2026-10-01""#), "{out}");
        assert!(out.contains(r#""estimate":90"#), "{out}");
    }

    /// 미룬 줄을 읽고 그대로 쓰면 바이트가 같다. **`blocked_by` 와
    /// `created_at` 사이**다 — 소속·물림 다음, 시각 앞.
    #[test]
    fn round_trips_a_deferred_line() {
        let line = r#"{"id":"argos-4aex","title":"제목","status":"todo","deferred_at":"2026-09-11T04:12:03Z","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#;
        let i: Issue = serde_json::from_str(line).unwrap();
        assert!(i.is_deferred());
        assert_eq!(serde_json::to_string(&i).unwrap(), line);
    }

    /// **안 미룬 줄은 한 글자도 안 바뀐다.** 축을 하나 더해 놓고 1만 줄이
    /// 통째로 diff 에 뜨면 그 축은 값어치보다 비싸다.
    #[test]
    fn not_deferring_writes_nothing() {
        let line = serde_json::to_string(&issue()).unwrap();
        assert!(!line.contains("deferred"), "{line}");
        assert!(!issue().is_deferred());
    }

    /// idea 줄을 읽고 그대로 쓰면 바이트가 같다. 종류 하나를 더했으므로
    /// 멱등성이 그 값까지 덮는지 여기서 못 박는다.
    #[test]
    fn round_trips_an_idea_line() {
        let line = r#"{"id":"argos-4aex","title":"반짝","kind":"idea","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#;
        let i: Issue = serde_json::from_str(line).unwrap();
        assert_eq!(i.kind, Kind::Idea);
        assert_eq!(serde_json::to_string(&i).unwrap(), line);
    }

    /// 모르는 종류는 **조용히 기본값이 되지 않는다.** 기본값으로 접으면 그
    /// 줄을 한 번 되쓰는 것만으로 원래 값이 사라지는데, 조용한 손실이 이
    /// 설계가 못 견디는 유일한 실패 모드다. 대신 줄이 안 읽히고 `status` 가
    /// `unreadable_line` 으로 시끄럽게 말한다 — 파일의 그 줄은 그대로 있다.
    #[test]
    fn an_unknown_kind_is_refused_loudly_not_folded() {
        let line = r#"{"id":"argos-4aex","title":"제목","kind":"몰라","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#;
        assert!(
            serde_json::from_str::<Issue>(line).is_err(),
            "모르는 종류를 조용히 기본값으로 접었다 — 되쓰면 그 값이 사라진다"
        );
    }

    #[test]
    fn idea_parses_from_the_command_line() {
        assert_eq!("idea".parse::<Kind>(), Ok(Kind::Idea));
        assert_eq!(Kind::Idea.as_str(), "idea");
        let e = "아이디어".parse::<Kind>().unwrap_err();
        assert!(e.contains("idea"), "오류 문장이 idea 를 안 댄다 — {e}");
    }

    /// 옛 바이너리가 2단계 종류를 만나도 줄을 버리지 않는다.
    #[test]
    fn reads_milestone_kind() {
        let i: Issue = serde_json::from_str(
            r#"{"id":"argos-4aex","title":"M1","kind":"milestone","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
        )
        .unwrap();
        assert_eq!(i.kind, Kind::Milestone);
    }

    /// 같은 개념이 대소문자로 갈라지지 않는다. 태그를 만드는 것이 주로
    /// 에이전트라 `Bug`·`bug` 를 섞어 쓴다.
    #[test]
    fn tags_fold_to_one_spelling() {
        assert_eq!(normalize_tag("  #Bug "), "bug");
        assert_eq!(normalize_tag("PARSER"), "parser");
        assert_eq!(normalize_tag("한글"), "한글");

        let mut i = issue();
        i.tags = vec!["Bug".into(), "bug".into(), "#BUG".into()];
        i.normalize();
        assert_eq!(i.tags, ["bug"], "한 개념이 둘로 갈라졌다");
    }

    /// `blocked_by` 는 마일스톤 다음, `created_at` 전에 온다.
    #[test]
    fn blocked_by_sits_between_milestone_and_created_at() {
        let mut i = issue();
        i.blocked_by = vec!["argos-0001".into()];
        let line = serde_json::to_string(&i).unwrap();
        let want = ["milestone", "blocked_by", "created_at"];
        // milestone 은 비어 있으니 실제로는 blocked_by 와 created_at 만 있다.
        let at: Vec<usize> = [want[1], want[2]]
            .iter()
            .map(|k| line.find(&format!("\"{k}\":")).unwrap())
            .collect();
        assert!(at[0] < at[1], "{line}");
    }

    /// 중복은 접히고 순서는 결정적이다.
    #[test]
    fn blocked_by_normalizes_sorted_and_deduped() {
        let mut i = issue();
        i.blocked_by = vec!["argos-0002".into(), "argos-0001".into(), "argos-0002".into()];
        i.normalize();
        assert_eq!(i.blocked_by, ["argos-0001", "argos-0002"]);
    }

    #[test]
    fn validate_refuses_self_block_and_bad_blocker_id() {
        let c = cfg();
        let mut i = issue();
        i.blocked_by = vec![i.id.clone()];
        assert!(i.validate(&c).unwrap_err().contains("스스로를 막을"));

        let mut i = issue();
        i.blocked_by = vec!["이상한".into()];
        assert!(i.validate(&c).unwrap_err().contains("blocked_by id 형식"));
    }

    #[test]
    fn kinds_round_trip() {
        assert_eq!("milestone".parse::<Kind>().unwrap(), Kind::Milestone);
        assert_eq!("epic".parse::<Kind>().unwrap(), Kind::Epic);
        assert!("없는것".parse::<Kind>().is_err());
        let i: Issue = serde_json::from_str(
            r#"{"id":"argos-4aex","title":"M1","kind":"milestone","status":"todo","created_at":"2026-09-11T04:12:03Z","updated_at":"2026-09-11T04:12:03Z","status_since":"2026-09-11T04:12:03Z"}"#,
        )
        .unwrap();
        assert_eq!(i.kind, Kind::Milestone);
    }

    #[test]
    fn normalize_sorts_tags_and_drops_defaults() {
        let mut i = issue();
        i.tags = vec!["z".into(), "a".into(), "z".into()];
        i.priority = Some(DEFAULT_PRIORITY);
        i.body = Some(String::new());
        i.normalize();
        assert_eq!(i.tags, ["a", "z"]);
        assert_eq!(i.priority, None);
        assert_eq!(i.body, None);
        assert_eq!(i.priority(), DEFAULT_PRIORITY);
    }

    #[test]
    fn validate_refuses_what_writing_must_not_accept() {
        let c = cfg();
        for (mutate, want) in [
            ((|i: &mut Issue| i.title = "  ".into()) as fn(&mut Issue), "제목이 비었다"),
            (|i| i.title = "두\n줄".into(), "한 줄"),
            (|i| i.id = "argos-4ae".into(), "id 형식"),
            (|i| i.status = Status::new("없는칸"), "라는 칸이 없다"),
            (|i| i.priority = Some(9), "우선순위는"),
            (|i| i.tags = vec!["두 낱말".into()], "공백이나 쉼표"),
            (|i| i.assignee = Some("철수\n악성".into()), "담당은 한 줄"),
            (
                |i| {
                    i.assignee = Some("철수".into());
                    i.assignee_email = Some("a@b.c\n악성".into());
                },
                "담당 메일은 한 줄",
            ),
            (|i| i.epic = Some("이상한".into()), "에픽 id 형식"),
        ] {
            let mut i = issue();
            mutate(&mut i);
            let e = i.validate(&c).unwrap_err();
            assert!(e.contains(want), "{want} 를 기대했는데 {e:?}");
        }
        assert!(issue().validate(&c).is_ok());

        // 마일스톤 줄은 제 milestone 을 못 든다(moai-bg55). 다른 종류는 여전히 든다.
        let mut stone = issue();
        stone.kind = Kind::Milestone;
        assert!(stone.validate(&c).is_ok());
        stone.milestone = Some("argos-9k2p".into());
        let e = stone.validate(&c).unwrap_err();
        assert!(e.contains("다른 마일스톤에 들지 않는다") && e.contains("--milestone none"), "{e:?}");
        for kind in [Kind::Issue, Kind::Epic, Kind::Idea] {
            let mut i = issue();
            i.kind = kind;
            i.milestone = Some("argos-9k2p".into());
            assert!(i.validate(&c).is_ok(), "{kind:?} 가 마일스톤을 못 들었다");
        }
    }

    #[test]
    fn formats_and_parses_time() {
        for (secs, text) in [
            (0, "1970-01-01T00:00:00Z"),
            (1_789_084_800, "2026-09-11T00:00:00Z"),
            (1_789_099_923, "2026-09-11T04:12:03Z"),
        ] {
            assert_eq!(format_rfc3339(secs), text);
            assert_eq!(parse_rfc3339(text), Some(secs));
        }
        for bad in ["2026-09-11", "2026-09-11T04:12:03+09:00", "", "20260911T041203Z", "2026-13-11T04:12:03Z"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn counts_days() {
        assert_eq!(days_since("2026-09-08T00:00:00Z", "2026-09-11T04:12:03Z"), Some(3));
        assert_eq!(days_since("2026-09-11T04:12:03Z", "2026-09-11T23:59:59Z"), Some(0));
        assert_eq!(days_since("어제", "2026-09-11T04:12:03Z"), None);
        // 지금보다 뒤인 시각은 오늘이다 — 몇 초 빠른 옆 기계의 시계가 "-1일" 을 만들던 자리.
        assert_eq!(days_since("2026-09-11T04:12:05Z", "2026-09-11T04:12:03Z"), Some(0), "몇 초 뒤가 -1일이 됐다");
        assert_eq!(days_since("2026-09-20T00:00:00Z", "2026-09-11T04:12:03Z"), Some(0));
    }

    /// 이름 안에 괄호가 있는 사람이 실제로 있다. 앞에서 괄호를 찾으면
    /// 거기서 잘려 메일이 이름으로 들어간다.
    #[test]
    fn a_name_with_brackets_still_parses() {
        let a = Actor::parse("레이븐 (부재중) (raven@buzzni.com)").unwrap();
        assert_eq!(a.name, "레이븐 (부재중)");
        assert_eq!(a.email, "raven@buzzni.com");
        // 괄호 앞 공백은 있어도 없어도 같다
        assert_eq!(Actor::parse("레이븐(raven@buzzni.com)"), Actor::parse("레이븐 (raven@buzzni.com)"));
    }

    /// 모양이 어긋난 것을 조용히 이름으로 삼지 않는다 — 메일 없는 줄이 그렇게
    /// 샌다. 거절해야 `--user` 가 무엇을 받는지가 한 가지로 남는다.
    #[test]
    fn a_shape_that_is_not_name_and_mail_is_refused() {
        for bad in ["레이븐", "레이븐 ()", "()", "(raven@buzzni.com)", "레이븐 (raven)", "레이븐 (a b@c)"] {
            assert_eq!(Actor::parse(bad), None, "{bad:?} 를 받아 버렸다");
        }
        // 이름에 든 줄바꿈은 여기서 걸러야 한다. 통과시키면 두 줄짜리 `by` 가
        // 되돌릴 수 없는 저널에 남고, `add` 는 id 를 뽑은 뒤에야 죽는다.
        for bad in ["철\n수 (a@b.c)", "철\r수 (a@b.c)"] {
            assert_eq!(Actor::parse(bad), None, "{bad:?} 를 받아 버렸다");
        }
    }

    /// 표기를 바꿔도 **저장은 그대로다.** 여기가 흔들리면 설정 하나가
    /// 마이그레이션이 된다.
    #[test]
    fn naming_only_changes_what_is_shown() {
        use crate::config::Naming;
        let (n, e) = ("레이븐", Some("raven@buzzni.com"));
        assert_eq!(label(n, e, Naming::Full), "레이븐 (raven@buzzni.com)");
        assert_eq!(label(n, e, Naming::Name), "레이븐");
        assert_eq!(label(n, e, Naming::Email), "raven@buzzni.com");
        // 메일을 모르는 사람은 어느 모양에서도 이름으로 난다 — 빈칸을 내면
        // 그 줄이 누구의 것인지 화면에서 사라진다.
        for how in [Naming::Full, Naming::Name, Naming::Email] {
            assert_eq!(label(n, None, how), "레이븐");
            // 이름이 빈 줄은 거꾸로 메일로 난다. 저널은 정규화를 거치지 않아
            // 옛 줄의 `"by":""` 를 고칠 길이 여기뿐이다.
            assert_eq!(label("  ", e, how), "raven@buzzni.com");
        }
    }

    /// `-a` 는 메일을 요구하지 않는다. 남의 메일을 모르는 채로 남에게 맡기는
    /// 일이 실제로 있다.
    #[test]
    fn an_assignee_may_be_a_bare_name() {
        assert_eq!(split_assignee("철수"), (Some("철수".into()), None));
        assert_eq!(
            split_assignee("레이븐 (raven@buzzni.com)"),
            (Some("레이븐".into()), Some("raven@buzzni.com".into()))
        );
        assert_eq!(split_assignee("   "), (None, None));
    }

    /// 담당이 없는데 메일만 남으면 어느 화면도 그릴 줄 모른다.
    #[test]
    fn an_email_never_outlives_its_name() {
        let mut i = Issue::new("argos-4aex".into(), "t".into(), Kind::Issue, Status::new("todo"), "2026-09-11T05:02:44Z");
        i.assignee_email = Some("raven@buzzni.com".into());
        i.normalize();
        assert_eq!(i.assignee_email, None);

        // 반대도 같다 — 빈 메일이 남아 있으면 `moai show` 가 이름 뒤에 빈
        // 괄호를 그린다. 손으로 푼 충돌이 남기는 모양이다.
        i.assignee = Some("레이븐".into());
        i.assignee_email = Some("  ".into());
        i.normalize();
        assert_eq!(i.assignee_email, None);
        assert_eq!(i.assignee.as_deref(), Some("레이븐"));

        // 빈 이름도 없는 이름이다. 손으로 푼 충돌이 남기는 모양이라 `None` 만
        // 보면 상세가 ` (raven@buzzni.com)` 를 낸다.
        for blank in ["", "   "] {
            i.assignee = Some(blank.into());
            i.assignee_email = Some("raven@buzzni.com".into());
            i.normalize();
            assert_eq!((i.assignee.as_deref(), i.assignee_email.as_deref()), (None, None));
        }
    }

    /// 저널은 넷만 적는다. 필드 변경을 적기 시작하면 이벤트 로그가 된다.
    #[test]
    fn journal_entries_are_shaped() {
        let e = JournalEntry::status(
            "argos-4aex",
            &Status::new("todo"),
            &Status::new("in_progress"),
            None,
            "2026-09-11T05:02:44Z",
            &someone("claude"),
        );
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"ts":"2026-09-11T05:02:44Z","id":"argos-4aex","kind":"status","by":"claude","by_email":"claude@example.com","from":"todo","to":"in_progress"}"#
        );
    }

    fn said(line: &str) -> Option<(Option<String>, String, Option<u64>, Option<String>, Option<String>)> {
        parse_work(line).map(|w| (w.provider, w.model, w.tokens, w.grade, w.why))
    }

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    /// 꼴 전부, 그리고 빠져도 되는 셋(회사·토큰·괄호).
    #[test]
    fn a_model_line_reads_with_or_without_its_optional_parts() {
        assert_eq!(
            said("model: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)"),
            Some((s("anthropic"), "opus-5".into(), Some(182000), s("high"), s("쓰기 경로")))
        );
        assert_eq!(said("model: anthropic/opus-5 (high — 쓰기 경로)").unwrap().2, None, "모르는 토큰을 채웠다");
        assert_eq!(said("model: google/gemini-3.8"), Some((s("google"), "gemini-3.8".into(), None, None, None)));
        assert_eq!(said("model: openai/gpt-6 tokens=0").unwrap().2, Some(0), "0 은 모름이 아니다");
        assert_eq!(said("model: opus-5(high)"), Some((None, "opus-5".into(), None, s("high"), None)));
        assert_eq!(said("model: opus-5 (high - 손으로 친 붙임표)").unwrap().4, s("손으로 친 붙임표"));
        assert_eq!(said("model: opus-5 (high – 자동 고침의 반각 줄표)").unwrap().3, s("high"), "반각 줄표에 등급을 잃었다");
        assert_eq!(said("model: anthropic/opus-5.").unwrap().1, "opus-5", "문장 부호를 이름에 붙였다");
    }

    /// 이미 쌓인 옛 줄은 회사 없이 값이 된다 — 모델 이름에서 회사를 짐작하지 않는다.
    #[test]
    fn old_model_lines_still_count() {
        assert_eq!(
            said("model: opus (medium — 표시 리팩터 두 파일, 쓰기 경로도 동시성도 안 건드린다). 닫기는 05:03 에 다른 세션이 했다"),
            Some((None, "opus".into(), None, s("medium"), s("표시 리팩터 두 파일, 쓰기 경로도 동시성도 안 건드린다")))
        );
        assert_eq!(said("model: opus-5 (medium — 감독 스킬 글 (표 포함))").unwrap().4, s("감독 스킬 글 (표 포함)"));
    }

    /// 꼴이 아닌 줄은 값이 안 된다 — 노트로만 남는다. 괄호 앞의 오타를 받으면 토큰이 조용히 "모름" 이 된다.
    #[test]
    fn what_is_not_the_shape_is_only_a_note() {
        for line in [
            "model::actor 가 뿌리를 받는다",
            "- model: opus-5",
            "Model: opus-5",
            "model:",
            "model: tokens=3",
            "model: /opus-5",
            "model: a/b/c",
            "model: opus-5 tokens=?",
            "model: opus-5 tokens=182k",
            "model: opus-5 tokens 182000",
            "model: opus-5 (high — 닫히지 않은 괄호",
            "앞에 글이 있는 model: opus-5",
            "model: opus-5 tokens=+5",
            "model: opus-5 (high — 쓰기 경로) tokens=182000",
            "model: opus-5 (high — 쓰기 경로, tokens=182000)",
            "    model: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)",
            " model: opus-5",
        ] {
            assert_eq!(said(line), None, "{line:?} 를 값으로 읽었다");
        }
    }

    /// 노트와 칸 옮김의 `-m` 둘 다, 한 글 안의 줄마다 읽고 적은 사람과 때를 붙인다.
    #[test]
    fn work_is_gathered_from_notes_and_moves() {
        let who = someone("claude");
        let journal = vec![
            // 제목은 읽지 않는다 — 꼴에 맞는 제목이라야 이 줄이 그것을 잰다.
            JournalEntry::create("x-1", "model: sonnet-5", "2026-09-18T01:00:00Z", &who),
            JournalEntry::note("x-1", "고쳤다\nmodel: anthropic/opus-5 tokens=10 (high — a)\nmodel: anthropic/haiku-4.5", "2026-09-18T02:00:00Z", &who),
            // 울타리 안의 줄은 꼴을 옮겨 적은 예다.
            JournalEntry::note("x-1", "꼴은 이렇다\n```\nmodel: anthropic/opus-5 tokens=182000 (high — 쓰기 경로)\n```", "2026-09-18T02:30:00Z", &who),
            JournalEntry::status(
                "x-1",
                &Status::new("review"),
                &Status::new("done"),
                Some("model: openai/gpt-6 tokens=5".into()),
                "2026-09-18T03:00:00Z",
                &who,
            ),
            JournalEntry::note("x-1", "그냥 노트", "2026-09-18T04:00:00Z", &who),
        ];
        let work = work_of(&journal);
        let models: Vec<&str> = work.iter().map(|w| w.model.as_str()).collect();
        assert_eq!(models, ["opus-5", "haiku-4.5", "gpt-6"]);
        assert_eq!(work[0].at, "2026-09-18T02:00:00Z");
        assert_eq!(work[2].at, "2026-09-18T03:00:00Z");
        assert_eq!(work[0].by, "claude");
        assert_eq!(work[0].by_email.as_deref(), Some("claude@example.com"));
    }
}
