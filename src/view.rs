//! 사람이 읽을 줄을 만든다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 표를 쓰는 이상 폭 계산이 필요하다. 한글은 터미널에서 두 칸을 먹으므로
//! `len()` 으로 맞추면 한글 제목이 섞인 표가 전부 어긋난다.

use crate::config::Config;
use crate::i18n::{Lang, fill, say};
use crate::model::{Issue, JournalEntry, Kind};
use crate::report::{Roll, StatusReport, Warning, is_group};
use crate::style::{self, paint};
use crate::text::{clip, one_line, width};
use crate::worktree::Origin;
use anstyle::Style;
use std::collections::BTreeMap;

/// 제목이 이보다 길면 자른다. 표가 접히면 표가 아니다.
///
/// **`guide` 의 예시가 이 자를 빌려 쓴다** — 제목을 짧게 쓰라고 가르치는 예시가 제 보드에서
/// 잘리지 않는지 보려면 숫자를 옮겨 적는 대신 여기를 읽어야 한다.
pub const TITLE_CAP: usize = 44;
/// 에픽 열은 곁다리라 더 짧게 자른다.
const EPIC_CAP: usize = 20;
/// 진행 막대 칸 수.
const BAR: usize = 10;

/// 그리기 한 판의 맥락 — **어느 말로 그리고, 옆 워크트리의 줄이 겹쳐 있는가.**
///
/// 둘 다 화면 전체에 드는 한 가지인데 자리마다 인자로 따라다녔다(moai-4xib). 값은 [`status`] 가
/// 인자 여덟이 되어 `clippy::too_many_arguments` 에 걸린 것이고, 남은 표면을 말묶음으로 옮길
/// 때마다 그리는 자리와 부르는 자리가 다 같이 바뀌었다.
///
/// **`Copy` 다.** 그리는 자리가 받아 아래 도우미로 그대로 넘기므로, 빌림으로 들면 그 자리마다
/// `&` 가 늘고 두 낱말짜리 값을 베끼는 편이 싸다.
///
/// [`Seen`] 과 **가르는 것**: 그쪽은 줄 하나를 파일 전체에서 읽어 낸 것이고 이쪽은 어떻게 그릴
/// 것인가다. [`status`]·[`ready`] 는 `Seen` 의 `roots`·`states`·`blocks`·`places` 를 쓰지 않으니,
/// `Seen` 에 `lang` 을 더하는 길로 가면 그 넷을 안 쓰는 자리가 넷을 받는다.
///
/// **`Default` 은 안 든다**(리뷰) — `Screen::default()` 는 말을 묻지 않고 `Lang::default` 를
/// 집어, `MOAI_LANG` 도 `[i18n] lang` 도 안 읽은 화면을 조용히 한국어로 그린다. 말을 명령 층에서
/// 한 번 풀어 아래로 준다는 것이 이 값이 선 까닭이라(`Ctx::lang`, moai-cigu), 그것을 건너뛰는
/// 입구를 열어 두지 않는다 — 들어오는 길은 [`Screen::new`] 하나다.
///
/// **그래서 두 필드가 이 모듈 안에 머문다**(리뷰) — `pub` 으로 두면 `Screen { lang:
/// Lang::default(), origin: None }` 이 어느 모듈에서나 서서, `Default` 를 걷어 막으려던 그 길이
/// 이름만 바꿔 그대로 열려 있다. 밖에서 그 둘을 읽는 자리가 없으니 닫는 값이 0 이다.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    /// 화면의 말. 명령 층에서 한 번 풀어 아래로 준다(`Ctx::lang`, moai-cigu).
    lang: Lang,
    /// 다른 워크트리에서 온 줄 (`worktree::overlay`). `None` 이면 안 겹쳤다 — 빈 [`Origin`] 을
    /// 빌려 주는 것과 **뜻이 같아서**([`Origin::branch`]·[`Origin::labels`] 가 둘 다 빈 답을 낸다)
    /// 겹칠 것이 없는 자리가 빈 값을 지어낼 일이 없다.
    origin: Option<&'a Origin>,
}

impl<'a> Screen<'a> {
    /// 겹쳐 보지 않은 화면.
    pub fn new(lang: Lang) -> Self {
        Self { lang, origin: None }
    }

    /// 같은 말로, 이 출처를 겹친 화면. **한눈 보기가 프로젝트마다 이것으로 바꿔 쓴다** —
    /// 출처는 프로젝트마다 다르므로 화면 하나에 하나로 들 수 없다.
    pub fn over(self, origin: &'a Origin) -> Self {
        Self { origin: Some(origin), ..self }
    }

    /// 이 줄을 보여 준 옆 가지. 안 겹쳤으면 `None`.
    fn branch(&self, id: &str) -> Option<&'a str> {
        self.origin.and_then(|o| o.branch(id))
    }

    /// 겹쳐 본 워크트리의 이름들. 안 겹쳤으면 비었다.
    fn labels(&self) -> Vec<&'a str> {
        self.origin.map(Origin::labels).unwrap_or_default()
    }
}

/// 칠한 뒤 **칠하지 않은 폭**을 기준으로 채운다. 순서를 바꾸면 이스케이프가
/// 폭에 세어져 표가 어긋난다.
fn cell(style: Style, text: &str, w: usize) -> String {
    let pad = w.saturating_sub(width(text));
    format!("{}{}", paint(style, text), " ".repeat(pad))
}

/// 오른쪽에 붙인 칸 — 수처럼 **끝이 맞아야** 읽히는 열. [`cell`] 과 같이 칸 수로 잰다.
///
/// **글자 수로 재지 않는다**(리뷰). `format!("{v:>4}")` 는 `char` 를 세므로 `19일`(3자, 5칸)과
/// 빈 글(0자, 4칸)이 한 열에서 한 칸씩 어긋났고, 그 글이 말묶음으로 옮겨 가면서
/// (`status.age`) 어긋나는 폭을 번역이 정하게 됐다.
fn rcell(style: Style, text: &str, w: usize) -> String {
    let pad = w.saturating_sub(width(text));
    format!("{}{}", " ".repeat(pad), paint(style, text))
}

/// `2026-09-11T15:18:26Z` → `2026-09-11 15:18`.
///
/// **연도를 낸다.** 상세는 정확해야 하는 자리다 — 해를 넘긴 저장소에서
/// `09-11` 만 보이면 작년인지 올해인지 화면으로는 못 가린다. 짧게 적는 것은
/// [`short_stamp`] 고, 그쪽은 줄이 빽빽한 이력에만 쓴다.
pub fn stamp(at: &str) -> String {
    match (at.get(..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_string(),
    }
}

/// `2026-09-11T15:18:26Z` → `09-11 15:18`. **이력 줄 전용이다** — 한 줄에
/// 시각·글·사람이 함께 들어가는 자리라 연도까지 적을 칸이 없다. 언제인지가
/// 뜻을 갖는 자리(생성·수정)는 [`stamp`] 를 쓴다.
pub fn short_stamp(at: &str) -> String {
    match (at.get(5..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_string(),
    }
}

/// `#a #b` — 태그를 사람에게 댈 때의 모양. CLI 표·상세와 탐색기의 목록 열·상세가 같이 쓴다.
pub fn tags_of(i: &Issue) -> String {
    tag_line(&i.tags)
}

/// 태그 표기가 정해지는 **한 자리**. 아직 `Issue` 가 아닌 것 — `add --from` 의 연습과
/// `idea promote` 미리보기가 그리는 초안 — 도 이것을 쓴다. `add` 가 손으로 짓던 판은 표기를
/// 바꾸면 show·status·탐색기만 따라가, 방금 만든 줄의 태그가 확인 줄에서 다르게 보였다.
pub fn tag_line(tags: &[String]) -> String {
    tag_parts(tags).map(|(mark, t)| format!("{mark}{t}")).collect()
}

/// [`tag_line`] 의 조각 — 태그마다 앞머리(`#`, 둘째부터 ` #`)와 태그 이름. **표기는 여기서만 정한다** —
/// `tag_line` 은 이것을 잇고, 탐색기 상세는 찾은 글자를 태그마다 따로 칠하려고(moai-lw7i) 조각째 받는다.
pub fn tag_parts(tags: &[String]) -> impl Iterator<Item = (&'static str, &str)> {
    tags.iter().enumerate().map(|(n, t)| (if n == 0 { "#" } else { " #" }, t.as_str()))
}

/// "미룸 (3일)" 이나 "미룸 — <줄> 밑" — 계획에 있으면 `None`.
///
/// **탐색기도 이 낱말을 쓴다.** 같은 사실을 두 표면이 다른 말로 하면, 나란히
/// 놓고 보는 사람이 어느 쪽을 믿을지 정하게 된다.
///
/// `root` 는 그 줄을 계획에서 뺀 줄이다(`report::deferred_roots`). **물려받은
/// 미룸도 여기서 말한다** — 미룬 에픽의 멤버를 펼쳤는데 표가 없으면, 이 낱말을
/// 쓰는 두 상세가 답하기로 한 "왜 ready 에 안 나오나" 가 빈다. 제가 미룬 줄은
/// 전처럼 제 시각으로 나이를 댄다.
pub fn deferred_for(i: &Issue, root: Option<&str>, now: &str, lang: Lang) -> Option<String> {
    if let Some(r) = root.filter(|r| *r != i.id) {
        return Some(fill(say(lang, "detail.deferred_under"), &[("id", r)]));
    }
    let at = i.deferred_at.as_deref()?;
    Some(match crate::model::days_since(at, now) {
        Some(d) if d > 0 => fill(say(lang, "detail.deferred_days"), &[("n", &d.to_string())]),
        _ => say(lang, "detail.deferred").to_string(),
    })
}

/// 계획에서 빠진 줄에 붙이는 한 마디. **도로 집는 말은 실제로 미룬 줄을 댄다** —
/// 물려받은 줄에 `--undo` 를 치면 "이미 그렇다" 로 끝나고 아무것도 안 풀린다.
/// 제가 미룬 줄이면 그 줄이 곧 미룬 곳이라 말이 하나로 되고, 부르는 쪽이 둘을
/// 가르는 `if` 를 둘 까닭이 없다.
///
/// `roots` 는 풀어야 할 미룸 전부다(`report::deferred_sources`, 가까운 것부터). **다 댄다** —
/// 하나만 대면 그것을 풀고도 여전히 빠진 채 그제야 다음을 댄다(moai-phzi).
pub fn shelved_by<S: AsRef<str>>(roots: &[S], lang: Lang) -> String {
    let roots: Vec<&str> = roots.iter().map(AsRef::as_ref).collect();
    fill(say(lang, "mv.shelved_by"), &[("ids", &roots.join(" · ")), ("undo", &roots.join(" "))])
}

/// 한 줄에 이름을 대는 줄의 수. 넘으면 수로만 댄다.
const TRAIL_SHOWN: usize = 3;

/// `moai mv <id> done` 이 **그 쓰기가 연 것**을 대는 줄들(moai-j4xs, `report::freed`).
/// 셋 다 비면 빈 목록 — 말할 것이 없을 때 출력이 전과 같다.
///
/// **흐린 곁들임이다.** 옮긴 줄이 주인공이다. 색이 혼자 뜻을 지지 않게 머리에 낱말을 둔다.
///
/// **셋을 한 줄로 합치지 않는다.** 뜻이 다르기 때문이다 — 풀린 것은 집을 수 있고, 닫을 수
/// 있는 것은 이미 쥔 것이고, 다음 것은 아직 아무도 안 쥔 것이다. 한 줄에 몰면 읽는 쪽이
/// 다음 수를 그 줄에서 못 고른다.
/// **키를 표에 접지 않는다** — `say(lang, "…")` 를 소스에서 읽는 시험
/// (`i18n::tests::english_has_every_key_the_source_asks_for`)이 표에 숨은 키를 못 본다.
pub fn freed_lines(freed: &[Issue], closable: &[Issue], next: &[Issue], lang: Lang) -> Vec<String> {
    // **풀린 줄만 `moai ready` 를 곁들인다**: 그것은 이제 그 목록에 서는 줄이라 이어서 볼
    // 곳이 있고, 닫을 수 있는 부모는 `ready` 에 안 선다.
    [
        trail(say(lang, "mv.unblocked"), freed, say(lang, "mv.unblocked_more")),
        trail(say(lang, "mv.closable"), closable, say(lang, "mv.trail_more")),
        trail(say(lang, "mv.next"), next, say(lang, "mv.trail_more")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// 머리 낱말 하나에 줄 몇을 단 흐린 한 줄. 비면 `None`.
///
/// 여럿이면 앞 몇 건만 대고 나머지는 수로 넘긴다 — 제목을 다 달면 막음 하나를 푼 에픽
/// 닫기가 화면을 채운다.
fn trail(head: &str, rows: &[Issue], more: &str) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let named: Vec<String> = rows
        .iter()
        .take(TRAIL_SHOWN)
        .map(|i| {
            format!("{} {}", paint(style::ID, &one_line(&i.id)), paint(style::DIM, &clip(&one_line(&i.title), 40)))
        })
        .collect();
    let more = match rows.len().saturating_sub(TRAIL_SHOWN) {
        0 => String::new(),
        n => paint(style::DIM, &fill(more, &[("n", &n.to_string())])),
    };
    Some(format!("{}  {}{more}", paint(style::DIM, head), named.join(&paint(style::DIM, " · "))))
}

/// 칠한 글과 **칠하지 않은 폭**을 받아 채운다 — [`cell`] 이 한 가지 색만 칠할 수
/// 있어, 브랜치 머리표처럼 두 색이 든 칸이 이 길로 온다.
fn pad(painted: &str, w_text: usize, w: usize) -> String {
    format!("{painted}{}", " ".repeat(w.saturating_sub(w_text)))
}

/// 제목, 다른 브랜치에서 온 줄이면 그 앞에 `⎇ <브랜치>`. (칠한 글, 칠하지 않은 폭).
///
/// **앞에 단다.** 뒤에 달면 긴 제목의 `…` 뒤로 밀리고, 표의 다음 열과 붙어 어느
/// 열의 말인지 흐려진다. 제목은 전처럼 `cap` 에서 자르고 머리표는 따로 센다 —
/// 머리표 때문에 제목이 짧아지면 지금 브랜치의 같은 줄과 다른 글로 읽힌다.
/// 브랜치 이름도 자른다: 긴 브랜치 하나가 표 전체를 밀어낸다.
fn marked(branch: Option<&str>, title: &str, cap: usize, style: Style) -> (String, usize) {
    let t = clip(title, cap);
    match branch {
        None => (paint(style, &t), width(&t)),
        Some(b) => {
            let m = format!("{} {}", style::BRANCH_GLYPH, clip(b, EPIC_CAP));
            (format!("{} {}", paint(style::BRANCH, &m), paint(style, &t)), width(&m) + 1 + width(&t))
        }
    }
}

fn title_style(i: &Issue) -> Style {
    // 제목은 칠하지 않는다 — 내용은 기본색, 주변만 칠한다.
    // 묶음만 예외다. 계획 계층이 한눈에 떠야 한다.
    //
    // **일이 아닌 것이 곧 묶음인 것은 아니다.** `kind != Issue` 로 물으면
    // idea 가 묶음 색을 입어 목록에서 에픽처럼 보인다 — `cmd/add.rs` 가
    // 만드는 순간에는 안 그런데 `show` 에서만 그러면 같은 줄이 두 색이다.
    if is_group(i) { style::EPIC } else { style::PLAIN }
}

/// 목록이 안 낸 것. **수와 함께 무엇으로 켜는지까지 들고 다닌다** — 숫자
/// 둘을 맨몸으로 넘기면 부르는 쪽이 순서를 바꿔도 컴파일러가 안 잡는다.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hidden {
    pub done: usize,
    pub ideas: usize,
    pub deferred: usize,
}

impl Hidden {
    /// "무엇 N건 숨김 — `플래그`" 조각들. 숨긴 것이 없으면 비어 있다.
    ///
    /// **`done`·`idea` 는 번역하지 않는다** — 칸 이름과 종류는 설정과 자료에서 오는 낱말이고,
    /// 바로 뒤의 플래그(`--all`·`--type idea`)가 그 글자를 그대로 받는다. 옮기면 화면이 대는
    /// 낱말과 쳐야 할 낱말이 갈린다. 미룸만 낱말이라 말묶음에서 온다(`status.put_off`).
    fn says(&self, lang: Lang) -> Vec<String> {
        [
            (self.done, "done", "--all"),
            (self.deferred, say(lang, "status.put_off"), "--deferred"),
            (self.ideas, "idea", "--type idea"),
        ]
        .into_iter()
        .filter(|(n, _, _)| *n > 0)
        .map(|(n, what, how)| fill(say(lang, "list.hidden"), &[("what", what), ("n", &n.to_string()), ("how", how)]))
        .collect()
    }

    /// 숨긴 줄 하나를 그것을 여는 낱말 밑에 센다. 어느 낱말로도 안 열리는
    /// 것은 안 센다 — 못 보여 줄 수를 대느니 말을 안 한다.
    pub fn add(&mut self, why: crate::query::Hide) {
        use crate::query::Hide;
        match why {
            Hide::Done => self.done += 1,
            Hide::Idea => self.ideas += 1,
            Hide::Deferred => self.deferred += 1,
            Hide::Unopenable => {}
        }
    }

    /// 숨긴 것을 흐린 한 줄로. **요약을 안 내는 표면(트리)이 쓴다** — 거기서
    /// 이 말을 빠뜨리면 머리글은 세는데 그 밑에 없는 줄이 까닭 없이 사라진다.
    pub fn note(&self, lang: Lang) -> Option<String> {
        let why = self.says(lang);
        (!why.is_empty()).then(|| paint(style::DIM, &why.join(" · ")))
    }
}

/// 목록. 비어 있으면 빈 줄이 아니라 왜 비었는지를 말한다.
///
/// `asked_deferred` 는 **부르는 쪽이 미룬 것만 달라고 했는가**다. 그때는
/// 줄마다 `미룸` 을 달아 봐야 자리만 먹는다.
pub fn list(
    issues: &[Issue],
    cfg: &Config,
    hidden: Hidden,
    epics: &crate::report::EpicLabels,
    asked_deferred: bool,
    wh: &crate::query::Where,
    screen: Screen,
) -> Vec<String> {
    let lang = screen.lang;
    if issues.is_empty() {
        let why = hidden.says(lang);
        return vec![match why.is_empty() {
            true => say(lang, "list.none").to_string(),
            false => format!("{} {}", say(lang, "list.none"), why.join(" · ")),
        }];
    }

    let show_tags = issues.iter().any(|i| !i.tags.is_empty());
    let show_epic = issues.iter().any(|i| epics.contains_key(&(i.id.as_str(), i.kind)));
    // 미룸 표를 달지 말지는 **부르는 쪽의 물음**에서 온다. 한때 결과의 내용
    // 으로 정했는데(`any(|i| !i.is_deferred())`), 그러면 `--all` 이 마침 전부
    // 미룬 것만 냈을 때 표가 통째로 사라져 계획 밖의 줄이 일과 똑같이 보인다 —
    // 안 물었는데 사라지는 것이 물어서 붙는 군더더기보다 나쁘다.
    let mark_deferred = !asked_deferred;
    let heads: Vec<(String, usize)> =
        issues.iter().map(|i| marked(screen.branch(&i.id), &i.title, TITLE_CAP, title_style(i))).collect();
    let tags: Vec<String> = issues.iter().map(tags_of).collect();
    // 에픽 열은 **제목**을 보여준다. id 를 보여주면 사람이 그걸 다시 찾아봐야 한다.
    let epics: Vec<String> = issues
        .iter()
        // `—` 는 낱말이 아니라 빈 칸의 표다 — 말묶음에 넣을 것이 없다.
        .map(|i| match epics.get(&(i.id.as_str(), i.kind)) {
            None => "—".into(),
            Some(t) => clip(t, EPIC_CAP),
        })
        .collect();

    // **머리글도 열 폭에 든다.** `ID` 가 `.max(2)` 로 이미 그렇게 잰다 — 자료만 재면 제목이
    // 머리글보다 짧은 표에서 `Title` 이 제 칸을 넘어 `Tags` 와 붙는다(`TitleTagsEpic`). 한국어의
    // `제목`(4칸)일 때는 한 글자짜리 제목에서만 나던 것이, 말이 바뀌면 머리글 길이가 그 문턱을
    // 정한다.
    let w_id = issues.iter().map(|i| width(&i.id)).max().unwrap_or(2).max(2);
    let w_title = heads.iter().map(|(_, w)| *w).max().unwrap_or(4).max(width(say(lang, "col.title")));
    let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0).max(width(say(lang, "col.tags")));

    let mut out = Vec::with_capacity(issues.len() + 2);
    let mut head = format!(
        "{}{}{}{}",
        cell(style::HEAD, "ID", w_id + 3),
        cell(style::HEAD, "P", 4),
        cell(style::HEAD, "S", 3),
        // `ID`·`P`·`S` 는 열 이름이 아니라 머리글자라 말묶음에 안 든다 — 어느 말에서도 같다.
        cell(style::HEAD, say(lang, "col.title"), if show_tags || show_epic { w_title + 2 } else { 0 }),
    );
    if show_tags {
        head.push_str(&cell(style::HEAD, say(lang, "col.tags"), if show_epic { w_tags + 2 } else { 0 }));
    }
    if show_epic {
        head.push_str(&paint(style::HEAD, say(lang, "col.epic")));
    }
    out.push(head.trim_end().to_string());

    for (((i, (title, w_this)), tag), epic) in issues.iter().zip(&heads).zip(&tags).zip(&epics) {
        // 묶음은 **멤버에서 읽은 칸**을 그린다. 손으로 둔 칸을 그리면 진행 중인
        // 에픽이 `·` 로 서서 롤업과 한 화면에서 모순된다(moai-j3b3).
        let col = wh.column(i);
        let mut row = format!(
            "{}{}{}  {}",
            cell(style::ID, &i.id, w_id + 3),
            cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
            paint(style::status_style(col), style::glyph(col)),
            pad(title, *w_this, if show_tags || show_epic { w_title + 2 } else { 0 }),
        );
        if show_tags {
            row.push_str(&cell(style::TAG, tag, if show_epic { w_tags + 2 } else { 0 }));
        }
        if show_epic {
            row.push_str(&paint(style::EPIC_REF, epic));
        }
        // **섞여 나올 때만 뜻이 있다.** `--deferred` 는 전부 미룬 것이라 줄마다
        // 같은 낱말이 붙어 봐야 자리만 먹는데, `--all` 은 섞여 나오므로 표가
        // 없으면 어느 줄이 계획 밖인지 알 길이 없다. 열을 늘리지 않고 꼬리에
        // 단다 — 미루지 않은 줄이 그 자리를 비워 두면 그게 더 시끄럽다.
        // 물려받은 미룸도 단다 — `--all` 에서 미룬 에픽의 멤버가 표 없이 서면
        // 계획 밖의 줄이 일과 똑같이 보인다.
        if wh.deferred(i) && mark_deferred {
            row.push_str(&format!("   {}", paint(style::DIM, say(lang, "status.put_off"))));
        }
        out.push(row.trim_end().to_string());
    }

    out.push(String::new());
    out.push(summary(issues, cfg, hidden, wh, lang));
    out
}

/// 칸별 건수. 묶음은 **줄마다 그린 그 칸**으로 센다 — S 열에 `▸` 로 선 에픽을
/// 꼬리에서 `todo` 로 세면 한 화면이 같은 줄을 두 칸으로 말한다.
fn summary(issues: &[Issue], cfg: &Config, hidden: Hidden, wh: &crate::query::Where, lang: Lang) -> String {
    let counts: Vec<String> = cfg
        .statuses
        .iter()
        .filter_map(|s| {
            let n = issues.iter().filter(|i| wh.column(i) == s).count();
            (n > 0).then(|| format!("{s} {n}"))
        })
        .collect();
    // 칸 이름은 설정에서 오는 낱말이라 그대로 센다 — 옮기는 것은 셈을 대는 틀뿐이다.
    let mut line = fill(say(lang, "list.summary"), &[("n", &issues.len().to_string()), ("cols", &counts.join(" · "))]);
    let why = hidden.says(lang);
    if !why.is_empty() {
        line.push_str(&paint(style::DIM, &format!("     {}", why.join(" · "))));
    }
    line
}

/// 채운 칸과 빈 칸. 멤버가 없으면 막대 대신 빈 자리를 준다 —
/// 0% 막대를 그리면 "아직 안 한 에픽" 과 "속을 안 채운 에픽" 이 같아 보인다.
pub fn bar(percent: Option<u8>) -> String {
    match percent {
        None => paint(style::DIM, &"░".repeat(BAR)),
        Some(p) => {
            let filled = crate::text::bar_fill(Some(p), BAR);
            format!("{}{}", paint(style::BAR, &"█".repeat(filled)), paint(style::DIM, &"░".repeat(BAR - filled)))
        }
    }
}

/// 에픽 → 멤버 → 자식으로 접어 낸다.
///
/// `shown` 은 이미 걸러진 것들이다. **걸러진 뒤에도 에픽 줄은 남긴다** —
/// 멤버가 하나도 안 걸리면 그 에픽은 아예 빼되, 걸린 것이 있으면 어느
/// 에픽 밑인지 보여야 목록이 뜻을 갖는다.
/// 트리를 훑는 동안 **한 번도 안 바뀌는 것들.** 줄마다 달라지는 것은
/// `path` 와 `depth` 뿐이라, 그 둘만 인자로 남기면 어느 것이 상태인지가
/// 부르는 자리에서 바로 보인다.
struct Ctx<'a> {
    all: &'a [Issue],
    index: &'a crate::nav::Index,
    keep: &'a dyn Fn(usize) -> bool,
    rolls: &'a [Roll],
    screen: Screen<'a>,
}

/// **그린 줄의 첨자도 돌려준다.** 거름망에 안 걸린 줄도 걸린 자손의 조상이면
/// 그려지므로, 꼬리가 "숨겼다" 고 셀 것은 그리지 않은 줄뿐이다. 세는 쪽이
/// 따로 훑으면 방금 그린 줄을 숨겼다고 말한다(moai-wi67).
pub fn tree(
    all: &[Issue],
    index: &crate::nav::Index,
    keep: &dyn Fn(usize) -> bool,
    rolls: &[Roll],
    screen: Screen,
) -> (Vec<String>, std::collections::BTreeSet<usize>) {
    let mut out = Vec::new();
    let mut drawn = std::collections::BTreeSet::new();
    let cx = Ctx { all, index, keep, rolls, screen };
    walk(&mut out, &mut drawn, &cx, &crate::nav::Path::new(), 0);
    if out.is_empty() {
        out.push(say(screen.lang, "list.none").to_string());
    }
    (out, drawn)
}

/// 소속 없는 줄인가. **잎이든, 자식을 거느려 디렉터리가 된 줄이든 같다.**
fn is_loose(e: &crate::nav::Entry) -> bool {
    use crate::nav::{Entry, Seg};
    matches!(e, Entry::Leaf { .. } | Entry::Dir { seg: Seg::Issue(_), .. })
}

/// 머리글로 그려지는가 — 빈 줄로 갈라 서고 그 밑을 거느린다 ([`place`]).
fn is_head(e: &crate::nav::Entry) -> bool {
    use crate::nav::{Entry, Seg};
    matches!(e, Entry::Dir { seg: Seg::Milestone(_) | Seg::Epic(_), at: Some(_) } | Entry::Dir { at: None, .. })
}

fn is_lost(e: &crate::nav::Entry) -> bool {
    use crate::nav::{Entry, Seg};
    matches!(e, Entry::Dir { seg: Seg::Lost, .. })
}

/// 이 자리에 **에픽이 설 수 있는가.** 뿌리와 마일스톤 밑이 그렇다.
///
/// 설 수 있는 자리에서만 소속 없는 줄을 따로 묶는다. 에픽 안에서는 모두가
/// 그 에픽의 멤버라 "에픽 없음" 이 뜻을 잃고, 실제로 `moai show <에픽>` 이
/// 제 멤버를 그 이름으로 불렀다.
fn groups_here(path: &crate::nav::Path) -> bool {
    matches!(path.last(), None | Some(crate::nav::Seg::Milestone(_)))
}

/// 그 디렉터리 안으로 들어간 경로. 잎은 들어갈 데가 없다.
fn into_dir(path: &crate::nav::Path, e: &crate::nav::Entry) -> Option<crate::nav::Path> {
    match e {
        crate::nav::Entry::Dir { seg, .. } => {
            let mut p = path.clone();
            p.push(seg.clone());
            Some(p)
        }
        crate::nav::Entry::Leaf { .. } => None,
    }
}

/// 그 밑에 걸린 줄의 수. 제 줄은 안 센다 — 부르는 쪽이 더한다.
fn under(cx: &Ctx, path: &crate::nav::Path, e: &crate::nav::Entry) -> usize {
    match into_dir(path, e) {
        Some(p) => cx.index.descendants(&p).iter().filter(|&&d| (cx.keep)(d)).count(),
        None => 0,
    }
}

fn blank(out: &mut Vec<String>) {
    if !out.is_empty() {
        out.push(String::new());
    }
}

/// 한 자리를 그리고 그 밑으로 내려간다.
///
/// **묶음 · 소속 없는 것 · 바구니 순으로 가른다.** `nav` 는 잎과 디렉터리를
/// 우선순위 하나로 섞어 차례를 정하므로(탐색기에는 그것이 맞다), 받은 차례
/// 그대로 훑으면 소속 없는 줄이 남의 에픽 바로 밑에 같은 들여쓰기로 끼어
/// 그 에픽의 멤버처럼 읽힌다. 보고서에서는 **무엇에 딸렸는지가 차례보다
/// 앞선다.**
fn walk(
    out: &mut Vec<String>,
    drawn: &mut std::collections::BTreeSet<usize>,
    cx: &Ctx,
    path: &crate::nav::Path,
    depth: usize,
) {
    let entries = cx.index.entries_where(cx.all, path, cx.keep);
    if !groups_here(path) {
        // **머리글 없는 줄을 먼저, 머리글을 뒤에.** 에픽 안에는 머리글이 설 것이 없어
        // 차례 그대로지만, `(길 잃음)` 안에는 마일스톤이 끊긴 에픽이 머리글로 서고
        // 에픽이 끊긴 잎이 곁에 선다. 받은 차례대로 놓으면 그 잎이 에픽 머리글 바로
        // 밑에 들여쓰여 멤버로 읽혔다(moai-44k8). 앞에 두면 바구니 머리글 밑에 서고,
        // 에픽 머리글은 빈 줄로 갈라 제 멤버만 거느린다 — 아래 소속 없는 줄을
        // 머리글로 가르는 것과 같은 까닭이다.
        let (heads, rows): (Vec<&crate::nav::Entry>, Vec<&crate::nav::Entry>) =
            entries.iter().partition(|e| is_head(e));
        for e in rows.into_iter().chain(heads) {
            place(out, drawn, cx, path, e, depth);
        }
        return;
    }

    for e in entries.iter().filter(|e| !is_loose(e) && !is_lost(e)) {
        place(out, drawn, cx, path, e, depth);
    }

    // 소속 없는 것도 **머리글을 갖는다.** CLI 트리는 보고서라, 소속 없는 일이
    // 몇 건인지가 정보다 — `moai status` 가 그것부터 드러내는 이유와 같다.
    // **자식을 거느린 줄도 소속 없는 것이다** — 잎만 세면 그런 줄이 머리글도
    // 셈도 없이 앞쪽으로 흘러나가고, 셈은 그만큼 모자라게 나온다.
    let loose: Vec<&crate::nav::Entry> = entries.iter().filter(|e| is_loose(e)).collect();
    if !loose.is_empty() {
        let n: usize = loose.iter().map(|e| 1 + under(cx, path, e)).sum();
        blank(out);
        // 집계는 **뿌리에서만** 빌린다. 마일스톤 밑의 `id` 없는 집계는
        // "마일스톤 없음" 이지 "에픽 없음" 이 아니다.
        let lang = cx.screen.lang;
        // **이름은 말묶음에서 온다 — 집계의 `title` 이 아니다.** `report` 는 `&[Issue]` 에 대한
        // 순수 함수라 화면 말을 모르고, 그 자리(`report::loose_title`)의 글자는 한국어로 박혀
        // 있다. 집계가 있든 없든 여기서 대는 이름이 같아야 한 화면이 한 말로 선다 — `ready` 가
        // 같은 줄에 대는 말과 **키도 같다**.
        let title = say(lang, "ready.no_epic");
        match path.is_empty().then(|| cx.rolls.iter().find(|r| r.id.is_none())).flatten() {
            Some(roll) => out.push(head(roll, title, n, None, lang)),
            None => out.push(format!(
                "{}  {}",
                paint(style::HEAD, title),
                fill(say(lang, "tree.count"), &[("n", &n.to_string())])
            )),
        }
        // **머리글이 들여쓰이지 않으니 그 밑도 한 칸이다.** `depth` 를 더하면
        // 마일스톤 안의 소속 없는 줄만 두 칸 들어가, 같은 머리글 밑에서
        // 뿌리와 마일스톤의 들여쓰기가 어긋난다.
        for e in loose {
            place(out, drawn, cx, path, e, 1);
        }
    }

    // 바구니는 늘 끝에. 정상인 것이 먼저 보여야 한다 — `nav` 가 목록을 그렇게
    // 세우는 것과 같은 뜻이다.
    for e in entries.iter().filter(|e| is_lost(e)) {
        place(out, drawn, cx, path, e, depth);
    }
}

/// 줄 하나를 놓고, 디렉터리면 그 밑으로 내려간다.
///
/// **머리글을 여기서 만들지 않는다** — 소속 없는 것을 묶는 일은 보고서의
/// 뿌리에서만 뜻이 있고, `members` 는 이미 제 제목을 낸 뒤라 머리글을 또
/// 내면 안 된다(`moai show <에픽>` 이 제 멤버를 `에픽 없음` 이라 불렀다).
fn place(
    out: &mut Vec<String>,
    drawn: &mut std::collections::BTreeSet<usize>,
    cx: &Ctx,
    path: &crate::nav::Path,
    e: &crate::nav::Entry,
    depth: usize,
) {
    use crate::nav::{Entry, Seg};
    // 제 줄이 있는 것은 줄이든 머리글이든 **여기서 그려진다.** 바구니만 제
    // 줄이 없다.
    if let Some(at) = e.at() {
        drawn.insert(at);
    }
    let deeper = into_dir(path, e).unwrap_or_else(|| path.clone());
    match e {
        // 묶음은 머리글을 갖는다 — 집계는 `report` 가 이미 했다.
        // **제 줄이 있는 것만 여기 온다.** 바구니(`at` 이 없는 것)를 같이
        // 받으면 `id` 가 `None` 이라 "에픽 없음" 집계에 걸려, `(마일스톤 없음)`
        // 바구니가 남의 이름표를 달고 남의 건수를 말한다.
        Entry::Dir { seg: Seg::Milestone(_) | Seg::Epic(_), at: Some(at) } => {
            blank(out);
            let id = cx.all[*at].id.as_str();
            let shown = under(cx, path, e);
            // **이름은 제 줄에서 읽는다.** 집계는 id 로 찾으므로 같은 id 의 줄이
            // 둘이면 남의 줄 것일 수 있다 — 그러면 두 줄이 한 제목을 달고 폴더인
            // 줄의 이름은 트리에서 사라진다(moai-sfml). 수는 id 가 같으면 같다.
            let title = cx.index.label(cx.all, e, cx.screen.lang);
            let lang = cx.screen.lang;
            match cx.rolls.iter().find(|r| r.id.as_deref() == Some(id)) {
                Some(roll) => out.push(head(roll, &title, shown, cx.screen.branch(id), lang)),
                // 집계가 없을 때도 **id 는 낸다** — 제목만 내면 그것을
                // 다시 찾아봐야 하고, 묶음을 펼친 이유가 사라진다.
                None => out.push(format!(
                    "{}  {}  {}",
                    paint(style::ID, id),
                    marked(cx.screen.branch(id), &title, usize::MAX, style::HEAD).0,
                    fill(say(lang, "tree.count"), &[("n", &shown.to_string())]),
                )),
            }
            walk(out, drawn, cx, &deeper, 1);
        }
        // 바구니도 머리글을 갖는다. **조용히 빼지 않는다** — 자리를 못
        // 정한 줄이 트리에서 사라지면 그 줄은 어디에도 없는 것이 된다.
        // 이름은 `nav` 에게 묻는다: 바구니는 제 줄이 없어 집계도 없다.
        Entry::Dir { at: None, .. } => {
            blank(out);
            let style = if is_lost(e) { style::WARN } else { style::HEAD };
            // **바구니 이름은 `nav` 가 짓는다**(`Index::label`) — 아직 말묶음에 안 든 글이라
            // 여기서는 셈만 옮긴다. 그 이름을 옮기는 자리는 `nav` 쪽이다.
            out.push(format!(
                "{}  {}",
                paint(style, &cx.index.label(cx.all, e, cx.screen.lang)),
                fill(say(cx.screen.lang, "tree.count"), &[("n", &under(cx, path, e).to_string())])
            ));
            walk(out, drawn, cx, &deeper, 1);
        }
        Entry::Dir { seg: Seg::Issue(_), at: Some(at) } => {
            row(out, cx, &cx.all[*at], depth.max(1));
            walk(out, drawn, cx, &deeper, depth.max(1) + 1);
        }
        // 제 줄이 있는 잃은 에픽·마일스톤도 여기로 온다 — 줄만 내고 만다.
        Entry::Dir { seg: Seg::Lost, at: Some(at) } => row(out, cx, &cx.all[*at], depth.max(1)),
        Entry::Leaf { at } => row(out, cx, &cx.all[*at], depth.max(1)),
    }
}

/// 머리글 없이 멤버와 그 자식만. 에픽 상세에서 쓴다 — 상세가 이미 제목을
/// 냈는데 트리 머리글이 또 내면 같은 줄이 두 번 나온다.
///
/// `rolls` 를 받는다. **빈 것을 넘기면** 그 밑의 에픽 줄이 집계를 잃고
/// `에픽 1건` 처럼 나와, 같은 에픽이 `moai show --tree` 와 다르게 읽힌다.
pub fn members(
    all: &[Issue],
    index: &crate::nav::Index,
    keep: &dyn Fn(usize) -> bool,
    rolls: &[Roll],
    at: &crate::nav::Path,
    screen: Screen,
) -> Vec<String> {
    let mut out = Vec::new();
    let cx = Ctx { all, index, keep, rolls, screen };
    walk(&mut out, &mut std::collections::BTreeSet::new(), &cx, at, 0);
    out
}

fn head(roll: &Roll, title: &str, shown: usize, branch: Option<&str>, lang: Lang) -> String {
    match &roll.id {
        // 묶음일 뿐 진척을 가진 것이 아니므로, 걸러진 뒤 **보이는** 수를 말한다.
        None => {
            format!("{}  {}", paint(style::HEAD, title), fill(say(lang, "tree.count"), &[("n", &shown.to_string())]))
        }
        Some(id) => {
            let pct = match roll.percent {
                // `status` 의 에픽 표와 **키가 같다** — 같은 사실을 두 말로 말하지 않는다.
                None => paint(style::DIM, say(lang, "status.no_children")),
                Some(p) => format!("{p:>3}%"),
            };
            format!(
                "{}  {}   {}/{}  {}  {}",
                paint(style::ID, id),
                marked(branch, title, TITLE_CAP, style::EPIC).0,
                roll.done,
                roll.total,
                bar(roll.percent),
                pct,
            )
        }
    }
}

/// 한 이슈와 그 밑의 자식들. 깊이는 id 의 점 수와 같다.
/// 트리의 한 줄. **내려가는 일은 `walk` 가 한다** — 여기서 자식을 다시 찾으면
/// 자리를 정하는 코드가 또 둘이 된다.
/// 트리의 줄 하나. **계획 밖이면 그렇다고 단다** — 목록이 꼬리에 다는 것과 같은
/// 낱말, 같은 자(제 미룸이나 물려받은 미룸)다. 트리는 걸린 자손의 조상도 그리므로,
/// 에픽의 미룸을 받은 생각이 제 자식 때문에 조상으로 서면 표 없이는 일과 똑같이
/// 보이고 꼬리는 그 줄을 숨긴 수에서 뺀다.
fn row(out: &mut Vec<String>, cx: &Ctx, i: &Issue, depth: usize) {
    // **여기 오는 것은 일과 생각뿐이다** — 묶음은 `nav` 가 언제나 디렉터리로 세우고
    // (`Index::is_dir`), `place` 가 머리글로 받는다. 그래서 읽은 칸을 물을 것이 없다.
    // 예외는 같은 id 의 쌍둥이에게 폴더를 내준 **가려진 묶음 줄** 하나다 — 깨진
    // 자료(`duplicate_id`)를 숨기지 않으려 잎으로 세운 것이라 적힌 칸을 그대로 낸다.
    let col = i.status.as_str();
    let mut line = format!(
        "{}{}  {}  {}  {}",
        "  ".repeat(depth + 1),
        paint(style::ID, &i.id),
        paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
        paint(style::status_style(col), style::glyph(col)),
        marked(cx.screen.branch(&i.id), &i.title, TITLE_CAP, title_style(i)).0,
    );
    if !i.tags.is_empty() {
        line.push_str(&format!("   {}", paint(style::TAG, &tags_of(i))));
    }
    if i.is_deferred() || cx.index.deferred_root(&i.id).is_some() {
        line.push_str(&format!("   {}", paint(style::DIM, say(cx.screen.lang, "status.put_off"))));
    }
    out.push(line);
}

/// 경고 하나를 사람 말로. **여기가 이 제품의 목소리다.**
///
/// **글은 말묶음에서 온다**(moai-7cyf) — 이 화면은 세션이 시작하는 자리라, 여기만 한국어로
/// 박혀 있으면 다른 말로 고른 사람은 머리 두 줄만 제 말이고 아래는 통째로 한국어를 본다.
/// **키는 소스에 박힌 글자다**([`say`]) — `format!("warn.{kind}")` 로 짓지 않는다. 아래 `match`
/// 가 갈래마다 제 키를 적으므로, 갈래를 더하며 글을 빠뜨리면 영어 표를 훑는 시험이 잡는다.
///
/// **`say` 부름을 갈래마다 제 줄에 쓴다** — 도우미에 키만 넘기지 않는다(리뷰). 소스를 훑는
/// 자(`i18n::tests::keys_in`)는 `say(…, "키")` 모양만 읽으므로, `one("warn.…")` 으로 넘기면
/// 그 키가 시험의 눈에서 통째로 사라진다. 그러면 en.json 에 없는 키를 적어도 시험이 다 파랗고,
/// [`say`] 가 키를 그대로 돌려줘 보드에 `warn.…` 이라는 글자가 선다 — 위 문단이 믿으라는
/// 바로 그 그물에 구멍이 난다. 도우미는 **이미 찾아 온 글**만 받는다.
///
/// **`{d}` 에 넣을 수는 부르는 자리가 고른다.** `days` 는 넘긴 문턱이고 `oldest` 는 실제
/// 나이라 뜻이 다른데(`report::Warning::oldest`), 도우미가 둘 중 하나를 미리 쥐면 나이를
/// 말하는 줄이 말없이 문턱을 낸다 — `idea_pile` 은 `days` 가 아예 없어 `0일` 이 된다.
fn says(w: &Warning, screen: Screen) -> String {
    let lang = screen.lang;
    let n = w.count.to_string();
    let one = |said: &str| fill(said, &[("n", &n)]);
    let aged = |said: &str, d: i64| fill(said, &[("n", &n), ("d", &d.to_string())]);
    match w.kind {
        "no_epic" => fill(
            say(lang, "warn.no_epic"),
            &[("n", &n), ("percent", &((w.ratio.unwrap_or(0.0) * 100.0).round() as u32).to_string())],
        ),
        "no_milestone" => one(say(lang, "warn.no_milestone")),
        "stale_review" => aged(say(lang, "warn.stale_review"), w.days.unwrap_or(0)),
        // review 도 벌여 놓은 일이다. "진행 중" 이라고 하면 review 경고와
        // 같은 이슈가 두 번 나오는 것이 말이 안 되게 보인다.
        "wip_overload" => one(say(lang, "warn.wip_overload")),
        "stale_progress" => aged(say(lang, "warn.stale_progress"), w.days.unwrap_or(0)),
        // **죽었다고 단정하지 않는다.** 워크트리 없이 main 에서 하는 일일 수도 있다 — 그래서 고칠
        // 손을 하나로 정하지 않고, 이어 할 것이면 워크트리를 다시 띄우라는 길까지 댄다.
        "stranded" => one(say(lang, "warn.stranded")),
        // `days` 는 "막힌 기간" 이 아니라 "지금 칸에 머문 기간" 이다 — 막 막힌
        // 것을 "며칠째 막혀 있다" 고 잘못 말하지 않으려고 이렇게 적는다.
        "blocked_stale" => aged(say(lang, "warn.blocked_stale"), w.days.unwrap_or(0)),
        // 막는 쪽이 어느 목록에도 없으므로 **어디서 찾는지를 같이 말한다.**
        // `ready` 가 같은 줄에 대는 말과 같다 — **키도 같다**(moai-7cyf): 둘로 두면 번역이
        // 갈라져, 한 도구가 같은 것을 두 말로 말한다. **막는 쪽이 아니라 미룬 곳이다** — 막는
        // 줄이 미룬 에픽 밑이면 그 줄에 `--undo` 를 쳐 봐야 "이미 그렇다" 로 끝난다.
        "blocked_by_deferred" => one(say(lang, "warn.blocked_by_deferred")),
        "empty_epic" => one(say(lang, "warn.empty_epic")),
        // **알림이지 경고가 아니다**(moai-tvvb) — 지금 무엇이 먼저인지를 대는 줄이다. 어느
        // 마일스톤이 도는지는 `preview` 가 id 와 제목으로 낸다.
        "milestone_focus" => one(say(lang, "warn.milestone_focus")),
        // **끊긴 것과 종류가 틀린 것을 한 낱말로 말한다** — 둘을 가려 말하면
        // 고치는 손이 달라지는 것도 아닌데 경고가 둘로 늘어난다.
        "dangling_epic" => one(say(lang, "warn.dangling_epic")),
        "dangling_milestone" => one(say(lang, "warn.dangling_milestone")),
        "orphan_child" => one(say(lang, "warn.orphan_child")),
        "dangling_blocked_by" => one(say(lang, "warn.dangling_blocked_by")),
        // 도구는 제 시계로만 적으므로 이런 시각은 손으로 고친 줄이나 틀린 시계다(moai-ugjp).
        "future_timestamp" => one(say(lang, "warn.future_timestamp")),
        // **알림이지 경고가 아니다.** 고칠 것이 있다는 말이 아니라, 담아 둔
        // 것을 한 번 펼쳐 볼 때가 됐다는 말이다.
        // **오늘 것에 "0일" 을 붙이지 않는다.** 나이를 말하는 까닭은 오래된
        // 것을 드러내려는 것인데, 갓 담은 것에까지 괄호가 붙으면 그 괄호가
        // 뜻을 잃는다. **그래서 키가 둘이다** — 괄호를 붙이고 말고는 번역이 정할 것이 아니라
        // 여기서 정하는 것이고, 한 키에 넣으면 그 말에서만 빈 괄호가 남는다.
        "idea_pile" => match w.oldest {
            Some(d) if d > 0 => aged(say(lang, "warn.idea_pile_aged"), d),
            _ => one(say(lang, "warn.idea_pile")),
        },
        "deferred" => match w.oldest {
            Some(d) if d > 0 => aged(say(lang, "warn.deferred_aged"), d),
            _ => one(say(lang, "warn.deferred")),
        },
        // **낡음의 두 얼굴을 다른 낱말로 낸다**(moai-mj45). 앞의 것은 "다시 빌드부터" 고, 뒤의
        // 것은 "손질이 사라진다" 다 — 한 낱말로 뭉치면 그 중 한쪽이 반드시 거짓말이 된다.
        // **어느 값인지는 여기서 안 댄다**(리뷰 moai-80qw) — 파일 자리까지 든 줄은 길어 표를
        // 밀어내므로 부르는 쪽이 stderr 로 이미 한 줄씩 냈다. 여기 서는 뜻은 "그 말을 놓쳤으면
        // 위를 봐라" 다: 이 줄이 없으면 보드가 "드러난 문제 없다" 로 방금 한 말을 뒤집는다.
        "user_config" => one(say(lang, "warn.user_config")),
        "agents_stale" => one(say(lang, "warn.agents_stale")),
        "agents_hand_edited" => one(say(lang, "warn.agents_hand_edited")),
        // **파일마다 결과를 따로 말한다**(moai-2f99) — `.gitignore` 에 `/.claude/worktrees/` 가
        // 없는 것과 `.gitattributes` 에 `merge=union` 이 없는 것은 결과가 아주 다르다(옆 워크트리가
        // `add -A` 에 딸려가는 것과, 저널이 머지에서 충돌하는 것). 한 낱말로 뭉치면 그 중 한쪽이
        // 반드시 거짓말이 된다(바로 위 `agents_stale` 을 가른 것과 같은 까닭).
        // **무엇이 빠졌는지는 `preview` 가 한 줄씩 낸다** — 규칙 줄은 제 안에 띄어쓰기를 여럿 들어
        // (`.moai/journal.jsonl  text eol=lf merge=union`) 한 줄에 이어 붙이면 어디서 한 줄이
        // 끝나는지 안 보인다.
        "gitignore_rules" => one(say(lang, "warn.gitignore_rules")),
        "gitattributes_rules" => one(say(lang, "warn.gitattributes_rules")),
        // **"안 심었다" 가 아니라 "못 돈다" 다**(moai-2ewr) — 안 심은 것은 밑의
        // `merge_driver_absent` 가 제 낱말로 말한다. 여기는 심어 놓고 그 자리가 빈 판이고,
        // 이슈마다 푸는 것이 돈다고 믿는 쪽만 손해를 본다. 어느 경로인지는 `preview` 가 한 줄로 낸다.
        "merge_driver_rotten" => one(say(lang, "warn.merge_driver_rotten")),
        // **"안 돈다" 와 "낡았다" 를 가른다**(moai-h54i). 앞의 것은 자리가 빈 것이고 뒤의 것은
        // 그 줄에 내려앉는 마디가 없는 것이다 — 고칠 명령이 비슷해도 무엇이 어긋났는지가 다르다.
        "merge_driver_stale" => one(say(lang, "warn.merge_driver_stale")),
        // **"안 돈다" 와 "딴 것이 선다" 도 가른다**(moai-zdw4). 자리가 빈 것과 그 이름에 다른
        // 도구가 선 것은 사람이 찾으러 갈 곳이 다르다 — 뭉치면 없는 파일을 찾으러 간다.
        "merge_driver_alien" => one(say(lang, "warn.merge_driver_alien")),
        // **안 심은 클론**(moai-9khu). 저장소가 `merge=moai` 를 걸어 뒀을 때만 선다 — 드라이버를
        // 안 쓰기로 한 저장소는 그 선언이 없어 이 줄을 아예 안 본다. 댈 경로가 없으니 `ids` 도
        // 없고, 칠 줄은 `hint` 가 낸다.
        "merge_driver_absent" => one(say(lang, "warn.merge_driver_absent")),
        "unknown_field" => one(say(lang, "warn.unknown_field")),
        // **까닭을 단정하지 않는다.** 머지를 잘못 푼 흔적일 수도, 못 읽는 줄이
        // 산 줄의 id 를 쓰고 있는 것일 수도 있다(moai-4dk4). 둘 다 줄 번호는
        // `moai show` 가 낸다.
        "duplicate_id" => one(say(lang, "warn.duplicate_id")),
        "unreadable_line" => one(say(lang, "warn.unreadable_line")),
        // **모르는 갈래도 말은 한다.** 옛 스냅샷이나 새 바이너리가 낸 갈래를 화면에서 지우지
        // 않는다 — 갈래 이름은 자료라 번역하지 않고, 셈을 세는 말만 말묶음에서 온다.
        other => fill(say(lang, "warn.other"), &[("n", &n), ("kind", other)]),
    }
}

/// 보드 · 경고 · 흐름.
///
/// **아무것도 막지 않는다.** 종료 코드는 데이터가 깨졌을 때만 0 이 아니다 —
/// 경고로 비영 종료하는 순간 부르는 쪽이 이것을 "실패" 로 읽고, 그러면 이건
/// 린트고, 린트는 곧 게이트다.
pub fn status(
    st: &StatusReport,
    issues: &[Issue],
    cfg: &Config,
    now: &str,
    at: &str,
    trouble: usize,
    screen: Screen,
) -> Vec<String> {
    let lang = screen.lang;
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    let overlaid = overlaid(screen);
    let mut out = vec![
        format!(
            "{}  {}       {}{overlaid}",
            paint(style::HEAD, &fill(say(lang, "status.issues"), &[("n", &st.total.to_string())])),
            paint(style::DIM, &fill(say(lang, "status.epics"), &[("n", &st.epics.len().to_string())])),
            paint(style::DIM, at),
        ),
        String::new(),
    ];

    out.push(board(cfg, &st.counts));

    let shelved = crate::report::put_off(issues);
    for (label, rolls) in
        [(say(lang, "status.milestone_label"), &st.milestones), (say(lang, "status.epic_label"), &st.epics)]
    {
        if rolls.is_empty() {
            continue;
        }
        out.push(String::new());
        if !st.milestones.is_empty() {
            out.push(paint(style::DIM, label));
        }
        let heads: Vec<(String, usize)> = rolls
            .iter()
            .map(|e| marked(e.id.as_deref().and_then(|id| screen.branch(id)), &e.title, EPIC_CAP, style::EPIC))
            .collect();
        let w_title = heads.iter().map(|(_, w)| *w).max().unwrap_or(4);
        for (e, (title, w_this)) in rolls.iter().zip(&heads) {
            // **미뤄 둔 묶음은 낱말로 말한다** — 색만으로 뜻을 지는 자리를 만들지
            // 않는다. 물려받은 미룸도 친다. "닫을 때가 됐다" 는 더 안 낸다: 묶음의
            // 칸은 멤버에서 읽으므로 100% 면 곧 닫힌 것이다(moai-j3b3).
            let put_off = e.id.as_deref().is_some_and(|id| shelved.contains(id));
            // **접은 묶음은 그렇다고 말한다.** 남은 멤버를 미뤄 닫은 묶음은 막대가
            // `1/2` 인 채로 done 에 서는데(칸은 미룬 멤버를 빼고 센다), 말하지 않으면
            // 세션이 시작하는 이 화면에서 접은 것과 굴러가는 것이 똑같아 보인다.
            let folded = e.column.as_deref() == Some(crate::config::DONE) && e.percent != Some(100);
            let note = match e.percent {
                _ if put_off => paint(style::DIM, &format!("   {}", say(lang, "status.put_off"))),
                None => paint(style::DIM, &format!("   {}", say(lang, "status.no_children"))),
                _ if folded => {
                    paint(style::status_style(crate::config::DONE), &format!("   {}", say(lang, "status.folded")))
                }
                _ => String::new(),
            };
            out.push(format!(
                "  {}  {}  {}  {}/{}{}",
                paint(style::ID, e.id.as_deref().unwrap_or("")),
                pad(title, *w_this, w_title + 2),
                bar(e.percent),
                e.done,
                e.total,
                note,
            ));
        }
    }

    // 경고는 **에픽 표 바로 다음**이다. 화면 아래로 밀면 페이저에 잘린다.
    // 고칠 것 다음에 알림. **자리가 갈렸어도 한 화면에 같은 모양으로 낸다** —
    // 글리프(`!`·`+`)가 둘을 가른다.
    for w in st.warnings.iter().chain(&st.notices) {
        out.push(String::new());
        // **알림은 경고처럼 보이면 안 된다.** `!` 를 달면 "쌓인 idea 6건" 이
        // 꾸지람으로 읽히고, 그러면 담는 것을 멈춘다 — 담는 비용을 0 으로
        // 만든 뜻이 거기서 사라진다. 그래서 흐린 `+` 다: 쌓였다는 말이지
        // 잘못됐다는 말이 아니고, `+` 는 흐름 줄의 `쌓이는 중 +3` 과 이미 같은
        // 뜻으로 서 있다.
        //
        // **`?` 는 못 쓴다** — 보드에서 `review` 칸의 글리프다. 한 화면에서
        // 한 글자가 두 뜻을 지면 어느 쪽도 못 믿는다.
        let (mark, glyph) = match (w.fatal, w.notice) {
            (true, _) => (style::ERROR, "!"),
            (_, true) => (style::DIM, "+"),
            _ => (style::WARN, "!"),
        };
        out.push(format!("{} {}", paint(mark, glyph), says(w, screen)));
        out.extend(preview(w, &by_id, now, screen));
    }
    // **알림만 있는 것은 "아무 문제 없다" 이다.** 알림은 `notices` 에 따로
    // 있으므로 `warnings` 가 비면 고칠 것이 없다 — 생각을 담거나 무언가를 미룬
    // 순간부터 이 줄이 사라지면, 세션을 닫기 전에 "경고가 늘지 않았는지" 보는
    // 사람이 알림을 경고로 읽는다.
    //
    // **옆 워크트리의 문제는 화면에서만 문제로 센다**(moai-cuw2). `report` 에는 넣지 않는다 —
    // 그쪽은 이 프로젝트의 `&[Issue]` 만 받고, 옆 파일은 이 데이터가 아니다. 그래도 보드가
    // "✓ 문제 없다" 를 말하면 한 줄씩 알린 stderr 와 제 말을 뒤집는다. 종료 코드는 안 바꾼다.
    if trouble > 0 {
        out.push(String::new());
        let said = fill(say(lang, "status.worktree_trouble"), &[("n", &trouble.to_string())]);
        out.push(format!("{} {said}", paint(style::WARN, "!")));
    } else if st.warnings.is_empty() {
        out.push(String::new());
        out.push(format!("{} {}", paint(style::status_style("done"), "✓"), say(lang, "status.all_clear")));
    }

    out.push(String::new());
    let net = st.flow.net;
    // **부호는 글이 아니라 값에 붙인다** — 번역이 `+` 를 빠뜨리면 늘어난 것과 줄어든 것이
    // 화면에서 같은 모양이 된다. 낱말(`쌓이는 중`)만 말묶음에서 오고 `{n}` 은 부호째 든다.
    let flow = match net.cmp(&0) {
        std::cmp::Ordering::Greater => {
            paint(style::WARN, &fill(say(lang, "status.piling"), &[("n", &format!("+{net}"))]))
        }
        std::cmp::Ordering::Less => {
            paint(style::status_style("done"), &fill(say(lang, "status.draining"), &[("n", &net.to_string())]))
        }
        std::cmp::Ordering::Equal => paint(style::DIM, say(lang, "status.steady")),
    };
    out.push(fill(
        say(lang, "status.flow"),
        &[
            ("days", &st.flow.days.to_string()),
            ("created", &st.flow.created.to_string()),
            ("done", &st.flow.done.to_string()),
            ("flow", &flow),
        ],
    ));
    out.push(String::new());
    out.push(paint(style::DIM, say(lang, "status.next")));
    out
}

/// 보드 한 줄 — config 의 칸 차례 그대로. 한 프로젝트의 `status` 와 한눈 보기가
/// 같은 줄을 낸다: 둘이 갈라지면 같은 보드를 두 모양으로 읽는다.
///
/// **칸 이름은 걸러서 찍는다.** 한눈 보기가 여기 넘기는 `cfg` 는 **남의 저장소의**
/// 설정이고, `config.rs` 는 `statuses` 의 글자를 재지 않는다 — ESC 가 든 칸 이름을
/// 그대로 찍으면 그 줄이 화면을 다시 칠한다. 찾는 자(`status_style`·`glyph`)는 날
/// 이름으로 두고 **찍는 낱말만** 거른다(`project ls` 와 같은 자).
fn board(cfg: &Config, counts: &BTreeMap<String, usize>) -> String {
    let cols: Vec<String> = cfg
        .statuses
        .iter()
        .map(|s| {
            let style = style::status_style(s);
            let word = one_line(s);
            format!(
                "{} {}",
                paint(style, style::glyph(s)),
                paint(style, &format!("{word} {}", counts.get(s).copied().unwrap_or(0)))
            )
        })
        .collect();
    format!("  {}", cols.join("    "))
}

/// 경고마다 앞의 몇 건만 보여 주고 나머지는 세어서 말한다. 다 늘어놓으면
/// 정작 봐야 할 다음 경고가 화면 밖으로 밀린다.
fn preview(w: &Warning, by_id: &BTreeMap<&str, &Issue>, now: &str, screen: Screen) -> Vec<String> {
    let lang = screen.lang;
    const SHOW: usize = 3;
    let mut out = Vec::new();
    // 벌여 놓은 것과 깨진 것은 id 만 한 줄에 늘어놓는다 — 제목이 정보를 안 준다.
    if matches!(w.kind, "wip_overload" | "duplicate_id" | "orphan_child" | "dangling_blocked_by" | "future_timestamp") {
        if !w.ids.is_empty() {
            out.push(format!("    {}", paint(style::DIM, &w.ids.join("   "))));
        }
        return out;
    }
    // 에픽에 대한 말은 칸도 나이도 뜻이 없다. 어느 에픽인지만 말한다.
    if matches!(w.kind, "empty_epic" | "unknown_field" | "dangling_epic" | "dangling_milestone" | "milestone_focus") {
        for id in w.ids.iter().take(SHOW) {
            let title = by_id.get(id.as_str()).map(|i| i.title.as_str()).unwrap_or("");
            out.push(format!(
                "    {}  {}",
                paint(style::ID, id),
                marked(screen.branch(id), title, TITLE_CAP, style::PLAIN).0
            ));
        }
        let rest = w.ids.len().saturating_sub(SHOW);
        if rest > 0 {
            out.push(format!("    {}", paint(style::DIM, &more_of(rest, lang))));
        }
        return out;
    }
    for id in w.ids.iter().take(SHOW) {
        let Some(i) = by_id.get(id.as_str()) else {
            // **여기 오는 것이 늘 id 는 아니다** — `gitattributes_rules` 는 규칙 줄을,
            // `merge_driver_rotten` 은 `.git/config` 에 적힌 **경로**를 든다. 손으로 고칠 수
            // 있는 파일에서 온 글이라 제어문자를 걷고 그린다(리뷰 moai-h6aq.cx8) — ESC 가
            // 그대로 나가면 그 줄이 화면을 다시 칠한다(`text::sanitize` 가 선 까닭).
            out.push(format!("    {}", paint(style::ID, &crate::text::sanitize(id))));
            continue;
        };
        // **판정한 나이를 댄다**(`Warning::ages`, moai-7azq). 여기서 새로 재면 `blocked_stale`
        // 처럼 칸 나이로 안 거는 경고에서 판정과 표시가 갈라진다. 안 실린 경고만 칸 나이다.
        let age = w
            .ages
            .get(id)
            .copied()
            .or_else(|| crate::model::days_since(&i.status_since, now))
            .map(|d| fill(say(lang, "status.age"), &[("d", &d.to_string())]))
            .unwrap_or_default();
        out.push(format!(
            "    {}  {}  {}  {}  {}",
            paint(style::ID, &i.id),
            paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
            paint(style::status_style(i.status.as_str()), style::glyph(i.status.as_str())),
            rcell(style::DIM, &age, 4),
            marked(screen.branch(&i.id), &i.title, TITLE_CAP, style::PLAIN).0,
        ));
    }
    let rest = w.ids.len().saturating_sub(SHOW);
    let more = if rest > 0 { more_of(rest, lang) } else { String::new() };
    if !more.is_empty() || w.hint.is_some() {
        out.push(format!(
            "    {}{}",
            // 뒤따를 힌트를 "N건 더" 열에 맞춰 띄운다. **줄이 하나도 없으면
            // 맞출 열도 없다** — 그때까지 띄우면 가리키는 것 없는 들여쓰기만
            // 남는다.
            cell(style::DIM, &more, if w.hint.is_some() && !w.ids.is_empty() { 12 } else { 0 }),
            w.hint.as_deref().map(|h| paint(style::DIM, &format!("→ `{h}`"))).unwrap_or_default(),
        ));
    }
    out
}

/// 목록에서 안 보인 나머지 — "N건 더". `status` 의 경고 밑에서만 선다.
///
/// **[`Screen`] 이 아니라 말만 받는다**(리뷰) — 셈을 글로 옮기는 것뿐이라 겹침을 볼 일이 없다.
/// 그리기 맥락을 통째로 받으면 이 두 줄이 옆 워크트리에 매인 것으로 읽힌다.
fn more_of(rest: usize, lang: Lang) -> String {
    fill(say(lang, "status.more"), &[("n", &rest.to_string())])
}

/// `moai prime` 한 판. **마크다운이고 칠하지 않는다**(moai-5ok8).
///
/// 이 글은 세션 첫머리와 접힌 뒤에 **다시 주입되는 맥락**이라, 보드처럼 표로 서지 않는다.
/// 표는 폭을 맞추려고 제목을 자르는데 읽는 쪽이 사람이 아니면 그 자름은 그냥 잃은 글이고,
/// 칸을 맞춘 빈칸은 세는 쪽에 값만 더한다. 그래서 제목은 안 자르고([`one_line`] 으로 한 줄로만
/// 편다) 줄은 목록으로 선다.
///
/// **색을 안 입힌다.** [`paint`] 는 터미널이 붙어 있으면 칠하는데, 이 판은 그때도 문서다 —
/// 마크다운 안의 이스케이프는 읽는 쪽이 사람이든 기계든 뜻이 없다.
///
/// **아무것도 막지 않는다.** 경고도 흐름도 안 싣는다 — 그것을 여기 실으면 세션마다 처음 읽는
/// 글이 잔소리가 되고, 잔소리는 곧 린트고 린트는 곧 게이트다.
pub fn prime(p: &crate::report::Prime, epics: &crate::report::EpicLabels, screen: Screen) -> Vec<String> {
    let lang = screen.lang;
    let mut out = vec![format!("# {}", say(lang, "prime.title")), String::new()];

    // **모든 칸이 [`one_line`] 을 지난다 — id 와 칸까지.** 읽기는 관대해서 줄바꿈이 든 id 나
    // 칸이 파일에 실제로 설 수 있는데(손으로 푼 머지), 이 판은 사람이 읽는 표가 아니라
    // 에이전트의 맥락에 그대로 실리는 마크다운이다. 한 줄만 새면 `## 머리글` 과 가짜 `- \`id\``
    // 줄이 이 판의 글로 서서, 읽는 쪽이 그것을 자료가 아니라 시킴으로 읽는다. 곁의
    // [`trail`] 은 id 를 이미 이렇게 편다 — 한 diff 에 선 두 표면이 갈려 있었다.
    let row = |i: &Issue, column: bool| {
        let epic = epics.get(&(i.id.as_str(), i.kind)).map(|t| format!(" ({})", one_line(t))).unwrap_or_default();
        let col = if column { format!(" · {}", one_line(i.status.as_str())) } else { String::new() };
        // **남의 가지에서 온 줄에는 그 가지를 단다**(`--worktree`). 안 달면 옆 워크트리가
        // 집은 일이 이 판의 "집은 것" 에 섞여, 읽는 쪽이 제가 쥔 것으로 읽고 이어서 한다.
        let branch =
            screen.branch(&i.id).map(|b| format!(" · {} {}", style::BRANCH_GLYPH, one_line(b))).unwrap_or_default();
        format!("- `{}` p{}{}{} — {}{}", one_line(&i.id), i.priority(), col, branch, one_line(&i.title), epic)
    };

    out.push(format!("## {}", say(lang, "prime.held")));
    out.push(String::new());
    if p.held.is_empty() {
        out.push(say(lang, "prime.held_none").to_string());
    } else {
        // 집은 것에는 **칸을 함께 단다** — `in_progress` 와 `review` 는 다음 수가 다르다.
        out.extend(p.held.iter().map(|i| row(i, true)));
    }
    out.push(String::new());

    out.push(format!("## {}", say(lang, "prime.ready")));
    out.push(String::new());
    if p.picks.is_empty() {
        out.push(say(lang, "prime.ready_none").to_string());
    } else {
        out.extend(p.picks.iter().map(|i| row(i, false)));
        // **잘린 것을 댄다.** 안 대면 이 판이 "집을 것이 셋뿐" 으로 읽혀, 다음에 무엇을
        // 집을지 고르는 쪽이 `moai ready` 를 아예 안 부른다.
        //
        // **`status` 의 꼬리와 키가 같다**([`more_of`], `status.more`) — 같은 "N건 더" 를 두
        // 키로 두면 한쪽만 옮긴 말에서 한 화면이 두 모양으로 센다. 뒤의 명령은 자료라 말묶음
        // 밖에 둔다: 번역이 그것을 만지면 훅에 걸린 세션이 못 치는 명령을 받는다
        // (`projects_ready` 가 같은 자리를 같은 꼴로 푼다).
        if p.rest > 0 {
            out.push(format!("- {} — `moai ready`", more_of(p.rest, lang)));
        }
    }
    // 도는 마일스톤은 **목록 밑에서** 댄다 — `ready` 가 짧아진 까닭을 그 자리에서 읽는다.
    if !p.focus.running.is_empty() {
        let ids: Vec<&str> = p.focus.running.iter().map(|m| m.id.as_str()).collect();
        out.push(String::new());
        out.push(format!("> {}", fill(say(lang, "prime.milestone"), &[("ids", &ids.join(", "))])));
    }
    out.push(String::new());

    out.push(format!("## {}", say(lang, "prime.closing")));
    out.push(String::new());
    out.extend(prime_closing(lang).into_iter().map(|said| format!("- {said}")));
    out.push(String::new());

    out.push(format!("## {}", say(lang, "prime.commands")));
    out.push(String::new());
    out.extend(prime_commands(lang).into_iter().map(|(run, said)| format!("- `{run}` — {said}")));
    out
}

/// 트래커를 못 읽은 자리의 [`prime`]. **머리글이 한 자리에만 있다** — `cmd/prime.rs` 가 같은
/// `# 제목` 을 손으로 한 벌 더 짓던 판은 이 함수를 고쳐도 그쪽이 옛 모양으로 남았다.
///
/// **닫기 전 목록과 명령은 여기서도 선다.** `--json` 이 그 둘을 싣는데 사람 쪽만 빼면,
/// 훅에 거는 쪽이 두 표면 중 하나를 못 믿는다([`prime_closing`] 의 까닭 그대로).
pub fn prime_no_repo(lang: Lang) -> Vec<String> {
    let mut out = vec![format!("# {}", say(lang, "prime.title")), String::new()];
    out.push(say(lang, "prime.no_repo").to_string());
    out.push(String::new());
    out.push(format!("## {}", say(lang, "prime.closing")));
    out.push(String::new());
    out.extend(prime_closing(lang).into_iter().map(|said| format!("- {said}")));
    out.push(String::new());
    out.push(format!("## {}", say(lang, "prime.commands")));
    out.push(String::new());
    out.extend(prime_commands(lang).into_iter().map(|(run, said)| format!("- `{run}` — {said}")));
    out
}

/// 닫기 전에 볼 것 — **[`prime`] 이 그리는 것과 같은 줄**. `--json` 이 이것으로 같은 말을
/// 낸다. 두 표면이 목록을 따로 들면 한쪽만 자라고, 그때 받는 쪽은 어느 것이 참인지 못 가린다.
///
/// **차례는 훅이 실제로 보는 규칙과 같다** — 이 글만 읽은 세션이 거절문을 처음 만나는 자리가
/// 없게 한다.
///
/// **키를 표에 접어 두지 않는다.** 키를 `&[&str]` 에 모아 놓고 도는 판은 `say(lang, "…")` 를
/// 소스에서 읽는 시험(`i18n::tests::english_has_every_key_the_source_asks_for`)의 눈에 안
/// 띄어, 오타 난 키가 아무 데서도 안 붉어진다 — `view::says` 의 스물한 줄이 그렇게 숨었던
/// 자리다. 한 줄씩 적어 두면 표와 소스가 한 자리에서 견줘진다.
pub fn prime_closing(lang: Lang) -> Vec<&'static str> {
    vec![
        say(lang, "prime.close_pick"),
        say(lang, "prime.close_scope"),
        say(lang, "prime.close_review"),
        say(lang, "prime.close_model"),
        say(lang, "prime.close_handoff"),
        say(lang, "prime.close_status"),
    ]
}

/// 명령과 그 한 줄 — [`prime_closing`] 과 같은 까닭으로 [`prime`] 과 한 자리를 쓴다.
///
/// **예닐곱 개다.** 여기가 길어지면 `--help` 가 된다 — 이 판의 값은 짧다는 것 하나다.
pub fn prime_commands(lang: Lang) -> Vec<(&'static str, &'static str)> {
    vec![
        ("moai status", say(lang, "prime.cmd_status")),
        ("moai ready", say(lang, "prime.cmd_ready")),
        ("moai show <id>", say(lang, "prime.cmd_show")),
        ("moai mv <id> in_progress --from todo", say(lang, "prime.cmd_mv")),
        ("moai note <id> '…'", say(lang, "prime.cmd_note")),
        ("moai mv <id> done -m '…'", say(lang, "prime.cmd_done")),
        ("moai idea add '…'", say(lang, "prime.cmd_idea")),
    ]
}

/// 집을 수 있는 일. 그리고 이미 벌여 놓은 것.
pub fn ready(
    picks: &[&Issue],
    epics: &crate::report::EpicLabels,
    wip: &[&Issue],
    held: &[crate::report::Held],
    focus: &crate::report::Focus,
    screen: Screen,
) -> Vec<String> {
    let lang = screen.lang;
    let mut out = vec![fill(say(lang, "ready.count"), &[("n", &picks.len().to_string())])];
    if picks.is_empty() {
        out.push(String::new());
        out.push(paint(style::DIM, say(lang, "ready.none")));
    } else {
        out.push(String::new());
        let heads: Vec<(String, usize)> =
            picks.iter().map(|i| marked(screen.branch(&i.id), &i.title, TITLE_CAP, style::PLAIN)).collect();
        let tags: Vec<String> = picks.iter().map(|i| tags_of(i)).collect();
        let w_id = picks.iter().map(|i| width(&i.id)).max().unwrap_or(2);
        let w_title = heads.iter().map(|(_, w)| *w).max().unwrap_or(4);
        let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0);

        for ((i, (title, w_this)), tag) in picks.iter().zip(&heads).zip(&tags) {
            let epic = match epics.get(&(i.id.as_str(), i.kind)) {
                None => say(lang, "ready.no_epic").to_string(),
                Some(t) => clip(t, EPIC_CAP),
            };
            out.push(
                format!(
                    "  {}{}{}{}",
                    cell(style::ID, &i.id, w_id + 3),
                    cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
                    pad(title, *w_this, w_title + 3),
                    if w_tags > 0 {
                        format!("{}{}", cell(style::TAG, tag, w_tags + 3), paint(style::EPIC_REF, &epic))
                    } else {
                        paint(style::EPIC_REF, &epic)
                    },
                )
                .trim_end()
                .to_string(),
            );
        }
    }

    // **목록이 왜 짧은지를 목록 바로 밑에서 댄다**(moai-q04l). 안 대면 도는 마일스톤이 밖의
    // 일을 뺀 것과 정말로 할 일이 없는 것이 화면에서 같아 보인다 — 미뤄 둔 것에 막힌 줄을
    // 아래에서 대는 것과 같은 까닭이다.
    //
    // **글리프는 `+` 다.** `!` 는 고칠 것이고 이것은 규칙이 서 있다는 알림이라, 보드가 쌓인
    // idea 에 `+` 를 쓰는 것과 같은 자리다. 꾸지람으로 읽히면 사람이 규칙을 끄고 싶어진다.
    // **뺀 것이 없으면 말하지 않는다**(리뷰 6) — "밖의 일 0건은 안 냈다" 는 아무것도 안
    // 말하면서 자리만 차지한다. 보드의 알림도 같은 자로 0 을 거른다.
    if !focus.outside.is_empty() {
        out.push(String::new());
        let said = fill(say(lang, "ready.outside_held"), &[("n", &focus.outside.len().to_string())]);
        out.push(format!("{} {}", paint(style::DIM, "+"), paint(style::DIM, &said)));
        for m in &focus.running {
            out.push(format!(
                "  {}  {}",
                paint(style::ID, &one_line(&m.id)),
                marked(screen.branch(&m.id), &m.title, TITLE_CAP, style::EPIC).0,
            ));
        }
    }

    // 이미 벌여 놓은 것을 먼저 알린다. 새로 집기 전에 볼 것이다.
    if !wip.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            paint(style::DIM, &fill(say(lang, "ready.picked"), &[("n", &wip.len().to_string())]))
        ));
        // **어느 워크트리에서 잡았는지도 댄다.** `--worktree` 로 부른 쪽이 가장 알고
        // 싶은 것이 "옆에서 누가 무엇을 잡았나" 다 — id 만 늘어놓으면 지금 브랜치의
        // 일과 구별이 안 된다.
        let ids: Vec<String> = wip
            .iter()
            .map(|i| match screen.branch(&i.id) {
                None => paint(style::DIM, &i.id),
                Some(b) => format!(
                    "{} {}",
                    paint(style::DIM, &i.id),
                    paint(style::BRANCH, &format!("{} {}", style::BRANCH_GLYPH, clip(b, EPIC_CAP)))
                ),
            })
            .collect();
        out.push(format!("  {}", ids.join("  ")));
    }

    // **미뤄 둔 것에 막힌 일은 까닭과 함께 댄다.** 막는 줄은 보드에도 `ready`
    // 에도 없으므로, 여기서 안 대면 목록이 왜 비었는지 아무 데서도 안 나온다.
    let shelved: Vec<&crate::report::Held> = held.iter().filter(|h| !h.by.is_empty()).collect();
    if !shelved.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            // `status` 의 같은 경고와 **키가 같다** — 한 도구가 같은 것을 두 말로 말하지 않는다.
            paint(style::DIM, &fill(say(lang, "warn.blocked_by_deferred"), &[("n", &shelved.len().to_string())]))
        ));
        for h in shelved {
            // **도로 집는 말은 미룬 곳을 댄다.** 막는 줄이 미룬 에픽 밑이면 그
            // 줄에 `--undo` 를 쳐 봐야 "이미 그렇다" 로 끝난다.
            out.push(format!(
                "  {}  {}  {}  {}",
                paint(style::ID, &h.issue.id),
                marked(screen.branch(&h.issue.id), &h.issue.title, TITLE_CAP, style::DIM).0,
                paint(style::DIM, &format!("← {}", h.by.join(" · "))),
                paint(style::DIM, &format!("moai defer {} --undo", h.undo.join(" "))),
            ));
        }
    }

    // **멤버가 없는 묶음에 막힌 일도 댄다**(moai-1c2l). 그 묶음은 영영 안 풀리는데 끝난
    // 것도 미룬 것도 아니라, 여기서 안 대면 목록이 까닭 없이 빈다. 도로 집을 것이 없으니
    // 막음을 푸는 말을 댄다 — 채우면 보통 막음이 된다.
    let bare: Vec<&crate::report::Held> = held.iter().filter(|h| !h.empty.is_empty()).collect();
    if !bare.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            paint(style::DIM, &fill(say(lang, "ready.held_by_empty"), &[("n", &bare.len().to_string())]))
        ));
        for h in bare {
            let unblock: Vec<String> =
                h.empty.iter().map(|g| format!("moai link {g} --unblocks {}", h.issue.id)).collect();
            out.push(format!(
                "  {}  {}  {}  {}",
                paint(style::ID, &h.issue.id),
                marked(screen.branch(&h.issue.id), &h.issue.title, TITLE_CAP, style::DIM).0,
                paint(
                    style::DIM,
                    &format!("← {}", fill(say(lang, "ready.no_members"), &[("groups", &h.empty.join(" · "))]))
                ),
                paint(style::DIM, &unblock.join("  ")),
            ));
        }
    }
    out
}

/// 줄 하나만 보고는 모르고 **파일 전체를 읽어야 아는 것.** 상세가 제 줄과
/// 자식 줄에 단다. 그리기 맥락([`Screen`])을 함께 든다 — 상세의 맥락은 인자 하나다.
///
/// **`Default` 은 안 든다**(moai-uhzk) — [`Screen`] 이 말을 물어야 서므로, 이것도 말 없이는
/// 안 선다. 짓는 자리는 둘뿐이라 도우미를 두지 않았다: `cmd::edit` 과 `cmd::show` 가 칸마다
/// 제 값을 채우고, 읽은 것이 없는 화면을 짓는 것은 시험의 `tests::bare_seen` 하나다.
pub struct Seen<'a> {
    /// 계획에서 빠진 줄 → 그것을 뺀 줄 (`report::deferred_roots`).
    pub roots: BTreeMap<&'a str, &'a str>,
    /// 묶음 → 멤버에서 읽은 칸 (`report::group_states`).
    pub states: BTreeMap<&'a str, &'a str>,
    /// 어느 말로 그리고 어느 워크트리의 줄이 겹쳐 있는가 ([`Screen`]). 겹침을 묻는 길이
    /// [`Screen::branch`] 하나가 되어, 상세가 제 손으로 [`Origin`] 을 뒤지지 않는다.
    pub screen: Screen<'a>,
    /// 펼친 줄의 막음을 하나씩 가른 것 (`report::blocks_of`). 막음이 없으면 비었다.
    pub blocks: Vec<crate::report::Block<'a>>,
    /// 펼친 줄이 **어디에 서 있는가** (`report::places`, moai-6opu). `None` 이면 줄을 안 세운다 — 안
    /// 집은 줄, 워크트리를 안 쓰는 저장소, **안 재 본 자리**(쓰는 길인 `edit`, 겹쳐 보지 않은
    /// 딸린 워크트리). 자리가 안 보이는 까닭(`Fresh`·`Unknown`·`Lost`)은 `Place` 가 이미 갈랐으니
    /// 여기서 다시 판단하지 않는다 — 안 잰 것(`None`)만이 아무 말도 안 하는 자리라, 그것을
    /// "자리 없다" 로 말하는 일은 없다.
    pub places: Option<crate::report::Place<'a>>,
}

/// **손으로 옮긴 칸이 서 있는 칸과 다르면** 그렇다고 말하는 낱말. CLI 상세와
/// 탐색기가 같은 말을 받는다.
///
/// `moai mv <에픽> done` 을 한 사람이 상세에서 `in_progress` 만 보면 쓰기가 안
/// 먹은 줄 안다. 첫 칸 그대로인 줄은 말하지 않는다 — 묶음은 거의 다 만든 칸에
/// 서 있어, 그것까지 말하면 모든 에픽 상세에 같은 군말이 붙는다.
pub fn unread_column(i: &Issue, col: &str, cfg: &Config, lang: Lang) -> Option<String> {
    (col != i.status.as_str() && i.status.as_str() != cfg.first_status())
        .then(|| fill(say(lang, "detail.unread_column"), &[("col", i.status.as_str())]))
}

/// `moai mv <묶음>` 이 내는 한 줄 — 서 있는 칸과, `done` 으로 옮기려 했으면 **실제로
/// 접히는 길.**
///
/// 끝난 멤버가 있으면 남은 멤버를 미뤄 접힌다 — 미룬 멤버는 칸 셈에서 빠지므로 남은
/// 것이 끝난 것뿐이 된다. **끝난 멤버가 하나도 없으면 전부 미뤄도 첫 칸이다**: 셀
/// 멤버가 없는 묶음은 첫 칸에 서므로(`report::group_states`), 그때 "남은 멤버를 미룬다"
/// 를 시키면 시킨 대로 한 뒤에도 같은 말이 돌아온다. 빈 묶음도 마찬가지다. 그 자리에서
/// 실제로 듣는 말은 묶음 제 `defer` 다 — 계획에서 빠지면 보드와 `ready` 에서 함께 빠진다.
pub fn group_moved(id: &str, col: &str, closing: bool, finished: bool, lang: Lang) -> String {
    // **갈래마다 제 `say` 를 적는다**([`says`] 와 같은 까닭) — 키를 도우미로 넘기면 소스를 훑는
    // 시험(`i18n::tests::keys_in`)이 그 키를 못 본다.
    let fold = match (closing, finished) {
        (false, _) => String::new(),
        (true, true) => say(lang, "mv.group_fold").to_string(),
        (true, false) => fill(say(lang, "mv.group_stuck"), &[("id", id)]),
    };
    format!("{}{fold}", fill(say(lang, "mv.group_column"), &[("col", col)]))
}

/// 상세의 막음 한 줄. **탐색기 상세(`tui::draw`)와 같은 낱말이다** — 막힘·풀림·끊김, 미룬
/// 막음은 "막힘" 에 미룬 까닭을 제목 앞에 둔다(값이 오른쪽부터 잘려도 남는다). 색은 글리프와
/// 낱말에 함께 붙어 혼자 뜻을 지지 않는다.
fn block_line(b: &crate::report::Block, branch: Option<&str>, now: &str, lang: Lang) -> String {
    use crate::report::Blocker;
    let title = b.issue.map(|x| marked(branch, &x.title, TITLE_CAP, style::PLAIN).0).unwrap_or_default();
    // **`say` 부름은 갈래마다 제 줄이다**([`says`] 와 같은 까닭) — 키를 도우미에 넘기면 소스를
    // 훑는 시험(`i18n::tests::keys_in`)의 눈에서 그 키가 통째로 사라진다. 그래서 `막힘` 셋이
    // 같은 키를 세 번 적는다.
    let (label, mark, glyph, what) = match b.blocker {
        Blocker::Missing => (say(lang, "block.broken"), style::ERROR, "!", say(lang, "block.missing_says").to_string()),
        Blocker::Done => (say(lang, "block.freed"), style::status_style("done"), "✓", title),
        Blocker::Open => (say(lang, "block.held"), style::WARN, "·", title),
        // 미뤄 뺀 멤버만 기다리는 묶음 — 묶음은 미룬 적이 없으니 그 멤버를 댄다. 첫 멤버와 남은 수만.
        Blocker::Deferred if !b.aside.is_empty() && b.root.is_none() => {
            let rest = b.aside.len() - 1;
            let more = match rest {
                0 => String::new(),
                n => format!(" {}", fill(say(lang, "block.and_more"), &[("n", &n.to_string())])),
            };
            let said = fill(say(lang, "block.deferred_members"), &[("ids", &format!("{}{more}", b.aside[0]))]);
            (say(lang, "block.held"), style::WARN, "·", format!("{}  {title}", paint(style::WARN, &said)))
        }
        // 멤버가 없는 묶음 — 기다릴 일이 없어도 막는다(moai-1c2l).
        Blocker::Empty => (
            say(lang, "block.held"),
            style::WARN,
            "·",
            format!("{}  {title}", paint(style::WARN, say(lang, "block.no_members"))),
        ),
        Blocker::Deferred => {
            let shelf = b
                .issue
                .and_then(|x| deferred_for(x, b.root, now, lang))
                .unwrap_or_else(|| say(lang, "status.put_off").to_string());
            (say(lang, "block.held"), style::WARN, "·", format!("{}  {title}", paint(style::WARN, &shelf)))
        }
    };
    // **이름 칸은 상세의 다른 줄과 한 폭이다**([`label_width`]) — 한국어는 `막힘`·`풀림`·`끊김`
    // 이 다 두 글자라 채울 것이 없었지만, `held`(4)·`freed`(5)·`broken`(6)은 제각각이라 글리프와
    // id 칸이 줄마다 옮겨 갔다. **칠한 뒤에 채운다**([`cell`] 과 같은 차례) — 먼저 채우면
    // 이스케이프 뒤의 빈칸까지 색이 물린다.
    format!(
        "  {}   {} {}  {what}",
        pad(&paint(mark, label), width(label), label_width(lang)),
        paint(mark, glyph),
        paint(style::ID, b.id)
    )
}

/// 상세의 **시작·끝** 두 값(moai-38mh) — CLI 상세와 탐색기 상세가 이 한 자로 그린다. 따로 두면
/// 어느 줄을 안 세울지가 표면마다 갈린다.
///
/// **안 세우는 줄이 둘이다.** 아직 한 번도 첫 칸을 안 떠난 줄은 빈 자리를 그리지 않는다. 그리고
/// **묶음은 제 줄의 시각을 안 그린다** — 묶음의 칸은 멤버에서 읽고 제 줄에 적힌 칸·칸 시각은 안
/// 읽는다([`unread_column`], `report` 의 묶음 `since`). 그 줄에 손으로 `mv` 를 쳐서 선 시각을
/// 그리면 머리 줄은 `in_progress` 인데 그 밑에 `끝` 이 선다(리뷰 moai-u5bk.3wq).
///
/// 빈 쪽은 `—` 다 — 이 필드 전에 집은 줄을 닫으면 시작은 모르고 끝만 선다.
pub fn span_of(i: &Issue) -> Option<(String, String)> {
    if is_group(i) || (i.started_at.is_none() && i.done_at.is_none()) {
        return None;
    }
    let at = |t: &Option<String>| t.as_deref().map_or_else(|| "—".to_string(), stamp);
    Some((at(&i.started_at), at(&i.done_at)))
}

/// 단건 상세. **이력은 부르는 쪽이 [`history`] 로 붙인다** — 묶음을 펼치면 멤버를
/// 이력 앞에 끼워야 해서, 여기서 붙이면 끼울 자리가 없다.
///
/// `seen.roots` 가 제 줄과 자식 줄의 미룸 표를 **물려받은 것까지** 말하게 하고,
/// `seen.states` 가 묶음의 칸을 멤버에서 읽게 한다.
pub fn detail(
    i: &Issue,
    epic: Option<&Issue>,
    children: &[&Issue],
    seen: &Seen,
    cfg: &Config,
    now: &str,
    raw: bool,
) -> Vec<String> {
    let mut out = vec![format!(
        "{}   {}",
        paint(style::ID, &i.id),
        marked(seen.screen.branch(&i.id), &i.title, usize::MAX, title_style(i)).0
    )];

    let col = crate::report::column(i, &seen.states);
    let st = style::status_style(col);
    let mut line = format!(
        "  {} {} · {}",
        paint(st, style::glyph(col)),
        paint(st, col),
        paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
    );
    // 종류는 이슈가 아닐 때만 적는다. **색은 묶음에만 준다** — idea 에
    // 묶음 색을 주면 상세 한 줄이 "이것도 무언가를 담는다" 고 말한다.
    if i.kind != Kind::Issue {
        let mark = if is_group(i) { style::EPIC } else { style::DIM };
        line.push_str(&format!(" · {}", paint(mark, i.kind.as_str())));
    }
    // **미룬 것은 상세에서 반드시 말한다.** 목록에서는 아예 안 보이므로,
    // id 로 콕 집어 펼친 이 화면이 "왜 ready 에 안 나오나" 에 답하는 자리다.
    if let Some(d) = deferred_for(i, seen.roots.get(i.id.as_str()).copied(), now, seen.screen.lang) {
        line.push_str(&format!(" · {}", paint(style::WARN, &d)));
    }
    if let Some(n) = unread_column(i, col, cfg, seen.screen.lang) {
        line.push_str(&format!(" · {}", paint(style::DIM, &n)));
    }
    if !i.tags.is_empty() {
        line.push_str(&format!(" · {}", paint(style::TAG, &tags_of(i))));
    }
    if let Some(a) = &i.assignee {
        line.push_str(&format!(
            " · {}",
            paint(style::DIM, &crate::model::label(a, i.assignee_email.as_deref(), cfg.naming))
        ));
    }
    out.push(line);

    let lang = seen.screen.lang;
    if let Some(e) = &i.epic {
        let title = epic.map(|e| e.title.as_str()).unwrap_or(say(lang, "detail.missing_epic"));
        // **묶음 줄의 `epic` 은 소속이 아니다**(moai-fg0t) — 트리도 `-e` 도 그 줄을 에픽 밑에 안
        // 둔다. 멤버와 같은 모양으로 그리면 적은 사람은 에픽 밑에 넣은 줄 안다.
        let stray = if crate::report::is_group(i) {
            format!("  {}", paint(style::DIM, say(lang, "detail.group_has_no_epic")))
        } else {
            String::new()
        };
        out.push(format!("  {}   {}  {title}{stray}", row_label(say(lang, "detail.epic"), lang), paint(style::ID, e)));
    }
    // **어디서 하던 일인지 댄다**(moai-6opu) — 세션이 죽은 뒤 이어받는 쪽이 들어갈 자리다. 없으면
    // 없다고 한다: 같은 자(`report::places`)로 잰 지금의 자리다.
    //
    // **`status` 의 `stranded` 와 같은 답이다**(moai-xn9n) — 한 시간 틈도, 못 읽은 워크트리도
    // `report::places` 가 한 곳에서 가른다. 한때 여기만 틈이 없어, 규약대로 집고 커밋한 뒤
    // 워크트리를 띄우는 사이에 펼치면 멀쩡한 줄이 버려진 것처럼 섰다.
    let place = row_label(say(lang, "detail.place"), lang);
    match &seen.places {
        Some(crate::report::Place::At(trees)) => {
            for t in trees {
                out.push(format!(
                    "  {place}   {}  {}",
                    t.path.display(),
                    paint(style::BRANCH, &format!("({})", t.branch))
                ));
            }
        }
        Some(crate::report::Place::Fresh) => {
            out.push(format!("  {place}   {}", paint(style::DIM, say(lang, "detail.place_fresh"))))
        }
        Some(crate::report::Place::Unknown) => {
            out.push(format!("  {place}   {}", paint(style::DIM, say(lang, "detail.place_unknown"))))
        }
        Some(crate::report::Place::Lost) => {
            out.push(format!("  {place}   {}", paint(style::WARN, say(lang, "detail.place_lost"))))
        }
        None => {}
    }
    // **막음도 상세에서 말한다**(moai-rvcb). id 로 콕 집어 펼친 이 화면이 "왜 ready 에 안
    // 나오나" 에 답하는 자리인데, 막힘·미룬 막음·끊긴 막음이 탐색기에만 있었다. 막는가는
    // `report::blocks_of` 가 `ready` 의 자로 가르고, 여기는 받은 답을 낱말로만 옮긴다.
    for b in &seen.blocks {
        out.push(block_line(b, seen.screen.branch(b.id), now, lang));
    }
    for c in children {
        // **자식 줄도 제 종류와 미룸을 말한다.** 이 목록은 걸러지지 않으므로
        // 담아 둔 생각과 미뤄 둔 것이 그대로 서는데, 표가 없으면 `ready` 도
        // 보드도 안 세는 줄이 일과 똑같이 보인다 — 낱말은 머리글이 쓰는 그
        // 자리(`deferred_for`)에서 같이 받는다.
        let mut tail = String::new();
        if c.kind != Kind::Issue {
            tail.push_str(&format!(" · {}", paint(style::DIM, c.kind.as_str())));
        }
        if let Some(d) = deferred_for(c, seen.roots.get(c.id.as_str()).copied(), now, seen.screen.lang) {
            tail.push_str(&format!(" · {}", paint(style::WARN, &d)));
        }
        let ccol = crate::report::column(c, &seen.states);
        out.push(format!(
            "  {}   {}  {}  ({} {}){tail}",
            row_label(say(lang, "detail.child"), lang),
            paint(style::ID, &c.id),
            marked(seen.screen.branch(&c.id), &c.title, TITLE_CAP, style::PLAIN).0,
            paint(style::status_style(ccol), style::glyph(ccol)),
            paint(style::DIM, ccol),
        ));
    }

    let age = crate::model::days_since(&i.updated_at, now)
        .map(|d| match d {
            0 => format!("  {}", say(lang, "detail.today")),
            n => format!("  {}", fill(say(lang, "detail.days_ago"), &[("d", &n.to_string())])),
        })
        .unwrap_or_default();
    // **두 줄의 이름 칸을 같은 폭으로 맞춘다.** 한국어는 `생성`·`시작` 이 두 글자로 나란했지만
    // 말이 바뀌면 길이가 갈린다 — 폭을 여기서 재야 `시작` 줄의 시각이 `생성` 줄의 시각 밑에
    // 선다(아래 `gap` 이 그것을 잇는다).
    let (left, right) = stamp_labels(lang);
    out.push(format!(
        "  {}   {}      {}  {}{}",
        left(say(lang, "detail.created")),
        paint(style::DIM, &stamp(&i.created_at)),
        right(say(lang, "detail.updated")),
        paint(style::DIM, &stamp(&i.updated_at)),
        paint(style::DIM, &age),
    ));
    // **시작·끝도 사람에게 보인다**(moai-38mh). 적히기만 하고 어느 화면에도 안 서는 필드는
    // 틀려도 아무도 모른다 — `--json` 만 보는 것은 기계뿐이다. 없으면 줄을 안 세운다: 아직
    // 첫 칸인 줄에 빈 자리를 그리면 생성·수정 줄이 두 배로 길어진다.
    // **끝은 `done_at` 이 섰다고 닫힌 것이 아니다** — 되돌린 줄에도 남는다. 그래서 칸은 위의
    // 머리 줄이 말하고 여기는 시각만 말한다. 어느 줄에 세우는지는 `span_of` 가 정한다.
    if let Some((start, end)) = span_of(i) {
        // 빈 시작(`—`)은 **윗줄의 생성 시각과 같은 폭으로** 채운다 — 안 채우면 `끝` 이 `수정`
        // 밑에서 열다섯 칸 왼쪽으로 붙는다.
        let gap = width(&stamp(&i.created_at)).saturating_sub(width(&start));
        out.push(format!(
            "  {}   {}{}      {}  {}",
            left(say(lang, "detail.started")),
            paint(style::DIM, &start),
            " ".repeat(gap),
            right(say(lang, "detail.finished")),
            paint(style::DIM, &end)
        ));
    }

    if let Some(body) = &i.body {
        out.push(String::new());
        // **제어문자는 어느 길로도 화면에 닿지 않는다.** 걸러는 일을 `raw` 를
        // 가리기 **전에** 둔다 — 뒤에 두면 `--raw` 만 걸러지지 않고, 파일에
        // 심긴 ESC 한 줄이 화면을 다시 칠한다. 그 길을 하나 더 여는 것이
        // `--raw` 를 더한 값어치보다 비싸다.
        let clean = crate::text::sanitize(body);
        // **원문 그대로 볼 길을 남긴다.** 그린 글은 기호가 지워져 되돌릴 수
        // 없다 — 본문을 긁어 붙이거나 마크다운을 고칠 때 이 길이 필요하다.
        if raw {
            out.extend(clean.lines().map(|l| format!("{PAD}{l}")));
        } else {
            out.extend(body_lines(&clean));
        }
    }

    out
}

/// 상세 왼쪽 이름 칸의 폭 — **그 말에서 가장 긴 이름 하나로 잰다.**
///
/// **한국어는 이 자가 없어도 섰다.** `에픽`·`자식`·`자리`·`멤버`·`생성`·`시작`·`막힘` 이 모두
/// 두 글자였기 때문이다. 말이 바뀌면 그 우연이 사라져 `Epic`·`Child`·`Members`·`held`·`broken`
/// 이 제각각 서고, 그 뒤의 id 열이 줄마다 어긋난다 — 열을 자료에서 재는 목록과 같은 자다.
///
/// **한 화면의 이름을 다 센다.** 한때 넷(`에픽`·`자식`·`자리`·`멤버`)만 세고 시각 줄과 막음 줄은
/// 저마다 쟀는데, 영어에서 `Created`(7)와 `Members`(7)가 마침 같아 그 갈림이 안 보였다 —
/// `held`(4)·`freed`(5)·`broken`(6)은 실제로 어긋나 막음 세 줄의 글리프 칸이 줄마다 옮겨 갔다.
/// 재는 곳이 하나여야 어느 말에서도 한 폭으로 선다.
///
/// **`중복` 은 안 센다** — 같은 id 의 줄이 둘일 때만 서는 경고 줄이고, 영어 `Duplicate`(9)를
/// 세면 그 드문 줄 하나 때문에 모든 상세가 두 칸씩 넓어진다. 그 줄은 [`row_label`] 을 지나며
/// 폭이 모자랄 때만 채워지고, 넘치면 넘친 채로 선다.
fn label_width(lang: Lang) -> usize {
    [
        say(lang, "detail.epic"),
        say(lang, "detail.child"),
        say(lang, "detail.place"),
        say(lang, "detail.members"),
        say(lang, "detail.created"),
        say(lang, "detail.started"),
        say(lang, "block.held"),
        say(lang, "block.freed"),
        say(lang, "block.broken"),
    ]
    .into_iter()
    .map(width)
    .max()
    .unwrap_or(0)
}

/// 상세 왼쪽의 이름 칸 — [`label_width`] 에 맞춰 뒤를 채운다. 그보다 긴 이름은 그대로 선다.
///
/// **멤버 줄은 `cmd::show` 가 굴림을 들고 세우므로** 그리는 곳이 갈려 있는데, 폭을 저마다
/// 재면 한 화면의 왼쪽 칸이 두 폭으로 선다.
pub(crate) fn row_label(what: &str, lang: Lang) -> String {
    pad(what, width(what), label_width(lang))
}

/// 상세의 `멤버` 이름 칸 — `cmd::show` 가 굴림과 함께 세운다. 위의 이름들과 같은 폭이다.
pub fn members_label(lang: Lang) -> String {
    row_label(say(lang, "detail.members"), lang)
}

/// 생성·수정 줄과 시작·끝 줄의 이름 칸을 **한 폭으로** 맞추는 두 자. 왼쪽 칸은
/// `생성`·`시작`, 오른쪽 칸은 `수정`·`끝` 이 나눠 쓴다.
///
/// 한국어에서는 `생성`(4칸)과 `시작`(4칸), `수정`(4칸)과 `끝`(2칸)이 뒤의 빈칸으로 맞춰져
/// 있었다 — 그 빈칸이 글에 박혀 있어, 말이 바뀌면 시각 열이 두 줄 사이에서 어긋났다.
///
/// **왼쪽 칸은 [`label_width`] 를 그대로 쓴다** — `생성`·`시작` 만 따로 재면 그 둘이 `에픽`·
/// `자식` 과 다른 폭으로 서서, 한 상세 안에서 왼쪽 칸이 두 번 갈린다.
fn stamp_labels(lang: Lang) -> (impl Fn(&str) -> String, impl Fn(&str) -> String) {
    let l = label_width(lang);
    let r = width(say(lang, "detail.updated")).max(width(say(lang, "detail.finished")));
    let pad_to = |w: usize| move |what: &str| pad(what, width(what), w);
    (pad_to(l), pad_to(r))
}

/// 본문을 그린다. 기호를 걷어내고 뜻을 색·속성으로 옮긴다.
///
/// **원문 그대로도 볼 수 있어야 한다** — 그린 글은 기호가 지워져 되돌릴 수
/// 없다. `moai show --raw` 가 그 길이고, `--json` 의 `body` 는 언제나 원문이다.
pub fn body_lines(body: &str) -> Vec<String> {
    // 파일에서 온 글이다. 그리기 전에 제어문자를 걷어낸다 — ESC 가 든 줄은
    // 그대로 찍으면 화면을 다시 칠한다. **부르는 쪽을 믿지 않는다** — 두 번
    // 걸러도 결과는 같고, 한 번 빠뜨리면 화면이 남의 손에 넘어간다.
    let blocks = crate::markdown::parse(&crate::text::sanitize(body));
    // **줄로 펴는 일은 `markdown` 이 한다.** 글머리·들여쓰기 같은 결정이
    // 표면마다 갈라지면 CLI 와 탐색기가 같은 본문을 다르게 그린다.
    // 여기가 할 일은 뜻을 색으로 옮기는 것뿐이다.
    // 셸은 폭을 넘긴 줄을 화면에서만 접는다 — 긴 인라인 코드를 끊지 않아야
    // 복사한 명령이 온전하다(moai-krh7).
    crate::markdown::layout(&blocks, BODY, crate::markdown::Overflow::Keep)
        .iter()
        .map(|line| {
            // 빈 줄은 빈 줄이다. 들여쓰기를 얹으면 줄 끝에 뜻 없는 공백이
            // 남아, 본문을 긁어 붙이거나 diff 를 볼 때마다 따라다닌다.
            if line.is_empty() {
                return String::new();
            }
            let painted: String = line.iter().map(|s| paint(role_style(s.role), &s.text)).collect();
            format!("{PAD}{painted}")
        })
        .collect()
}

/// 본문을 접는 폭. 터미널 폭을 묻지 않는다 — 이 저장소의 본문은 이미 손으로
/// 이만큼에 맞춰 쓰여 있고, 파이프로 넘길 때 폭이 매번 달라지면 diff 가 튄다.
const BODY: usize = 76;
/// 본문은 상세의 다른 줄과 같은 만큼 들어간다.
const PAD: &str = "  ";

/// 뜻을 색·속성으로. **여기가 이 표면의 몫이다** — `markdown` 은 뜻만 낸다.
fn role_style(r: crate::markdown::Role) -> Style {
    use crate::markdown::Role;
    match r {
        Role::Plain => style::PLAIN,
        Role::Strong | Role::Heading => style::STRONG,
        Role::Emphasis => style::EM,
        Role::Code => style::CODE,
        Role::Link => style::EPIC_REF,
        Role::Mark => style::DIM,
    }
}

/// 펼친 id 가 여러 줄에 쓰였다는 한 줄 (`report::duplicate_lines`). **뒷줄을 열었다고
/// 말한다** — 트리·탐색기가 고르는 줄과 같다는 것까지 알아야 앞줄을 찾으러 간다.
pub fn duplicate_note(lines: usize, lang: Lang) -> String {
    // 이름 칸은 상세의 다른 줄과 같은 자로 잰다([`label_width`]) — 이 낱말이 그보다 짧은 말에서
    // 왼쪽 칸이 혼자 좁게 서던 자리다. 길면 그대로 넘친다: 드문 경고 줄 하나 때문에 모든 상세를
    // 넓히지 않는다.
    let what = say(lang, "detail.duplicate");
    format!(
        "  {}   {}",
        pad(&paint(style::WARN, what), width(what), label_width(lang)),
        paint(style::WARN, &fill(say(lang, "detail.duplicate_says"), &[("n", &lines.to_string())]))
    )
}

/// 이 이슈에 닿은 커밋 — 짧은 해시와 제목(moai-emcv). 이력 바로 앞에 선다.
pub fn commits(commits: &[crate::git::Commit], lang: Lang) -> Vec<String> {
    let drawn = commit_lines(commits);
    if drawn.is_empty() {
        return Vec::new();
    }
    let mut out = vec![String::new(), paint(style::HEAD, say(lang, "detail.commits"))];
    out.extend(drawn.into_iter().map(|(short, subject)| format!("  {}   {subject}", paint(style::ID, short))));
    out
}

/// 사람 화면에 그릴 커밋 — (짧은 해시, 걸러 낸 제목). **CLI 상세와 탐색기가 함께 쓴다**
/// (moai-a4i0) — 무엇을 빼고 어떻게 줄이는지가 표면마다 갈리면 같은 이슈가 다르게 읽힌다.
///
/// **트래커 커밋은 그리지 않는다.** 집기·닫기만 적은 커밋이라 사람이 찾는 "무엇이 고쳤나"
/// 가 아니고, 이력이 이미 같은 것을 말한다. `--json` 은 `tracker` 표시와 함께 전부 낸다.
/// 제목도 파일 밖에서 온 글이라 **한 줄짜리로 걷어낸다**(`text::one_line`) — `sanitize` 가
/// 남기는 탭이 그대로 나가면 CLI 에서는 탭 자리까지 칸이 밀리고 탐색기에서는 폭을 재는
/// 자가 0으로 세어 글자째 사라진다. 한 커밋은 한 줄이라야 해시와 제목이 짝으로 읽힌다.
pub fn commit_lines(commits: &[crate::git::Commit]) -> Vec<(&str, String)> {
    commits
        .iter()
        .filter(|c| !c.tracker)
        .map(|c| (c.hash.get(..7).unwrap_or(&c.hash), crate::text::one_line(&c.subject)))
        .collect()
}

/// 저널을 **그대로 찍는다. 접지 않는다.**
pub fn history(journal: &[JournalEntry], cfg: &Config, lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    if !journal.is_empty() {
        out.push(String::new());
        out.push(paint(style::HEAD, say(lang, "detail.history")));
        for e in journal {
            // 메모는 여러 줄일 수 있다. 한 원소에 `\n` 을 담으면 "원소 하나가
            // 한 줄" 이라는 약속이 깨지고, 이어지는 줄이 열을 잃는다.
            let ts = short_stamp(&e.ts);
            let pad = " ".repeat(width(&ts) + 5);
            for (n, l) in entry(e, cfg, lang).split('\n').enumerate() {
                out.push(match n {
                    0 => format!("  {}   {l}", paint(style::DIM, &ts)),
                    _ => format!("{pad}{l}"),
                });
            }
        }
    }
    out
}

/// **칸 이름과 모르는 갈래는 그대로 낸다** — 칸은 설정에서 오는 낱말이고, 모르는 갈래는 새
/// 바이너리나 옛 줄이 적은 자료다. 옮기는 것은 이쪽이 아는 갈래의 낱말뿐이다. `note:` 도
/// 그대로다 — 저널에 적힌 갈래 이름이라, 옮기면 화면의 낱말과 파일의 낱말이 갈린다.
fn entry(e: &JournalEntry, cfg: &Config, lang: Lang) -> String {
    let what = match e.kind.as_str() {
        "create" => say(lang, "journal.create").to_string(),
        "rm" => say(lang, "journal.remove").to_string(),
        "status" => format!(
            "{} → {}",
            e.from.as_deref().unwrap_or("?"),
            paint(style::status_style(e.to.as_deref().unwrap_or("")), e.to.as_deref().unwrap_or("?"))
        ),
        "note" => format!("note: {}", e.text.as_deref().unwrap_or("")),
        other => other.to_string(),
    };
    let note = e.note.as_deref().map(|n| format!("  — {n}")).unwrap_or_default();
    // 이름도 메일도 없는 줄은 낼 것이 없다. `trim_end` 가 없으면 그 자리에
    // 꼬리 공백 두 칸이 남는다.
    format!(
        "{what}{}  {}",
        paint(style::DIM, &note),
        paint(style::DIM, &crate::model::label(&e.by, e.by_email.as_deref(), cfg.naming))
    )
    .trim_end()
    .to_string()
}

/// 한눈 보기에서 연 프로젝트 하나를 셈한 것 — `moai status` 가 `.moai` 밖에서 낸다.
pub struct Board<'a> {
    pub cfg: &'a Config,
    pub status: StatusReport,
    /// 집은 것 (`report::wip`).
    pub picked: Vec<&'a Issue>,
    /// `--worktree` 로 겹쳤으면 줄마다의 출처 (`Project::origin`).
    pub origin: &'a Origin,
    /// 옆 워크트리를 겹치다 만난 것 (`Project::trouble`).
    pub trouble: &'a [crate::worktree::Trouble],
    /// 자리를 재다 **못 읽어 판정을 가린** 워크트리들(moai-p3bs.op2, `report::blinding`). 그런
    /// 워크트리가 있으면 자리 판정이 `모른다` 로 접혀 경고가 조용해지는데, 여기서 세지 않으면
    /// 이 덩어리가 "드러난 문제 없다" 로 그 침묵을 덮는다 — 안쪽 `moai status` 는 같은 사실을
    /// stderr 와 `옆 워크트리 문제` 로 이미 말한다.
    ///
    /// **수가 아니라 목록으로 든다** — 사람 화면은 한 줄씩 대고(아래 `projects_status`), `--json` 은
    /// 안쪽 `status` 와 같은 모양으로 이 목록을 그대로 낸다.
    pub blind: Vec<crate::report::Workplace>,
    /// 판 것 가운데 스냅샷을 **못 읽은 워크트리 전부**(`worktree::Unread::all`) — 사람 화면이 한 줄씩 대는 것은
    /// 이쪽이다(사용자 결정 2026-09-18, 리뷰 moai-rgz9.7vt). 판정을 안 가려도 깨진 파일은 고칠
    /// 사람이 알아야 하고, `blind` 는 "그래서 자리를 다 못 셌다" 라는 다른 말이다.
    pub unread: Vec<crate::report::Workplace>,
    /// 옆 워크트리를 빠짐없이 열어 봤는가 (`Project::swept`) — 그러면 `unread` 는 `trouble` 이 이미
    /// 말했다. 사람 화면은 두 번 안 세고, `--json` 은 `blind`·`unread` 를 그대로 낸다(안쪽 `status` 와 같다).
    pub swept: bool,
}

/// 한눈 보기에서 연 프로젝트 하나의 집을 것 — `moai ready` 가 `.moai` 밖에서 낸다.
pub struct Picks<'a> {
    pub picks: Vec<&'a Issue>,
    /// 도는 마일스톤이 이 목록에 한 일(`report::ready_in`). **한눈 보기에서도 댄다** — 저장소
    /// 안의 `ready` 가 목록이 왜 짧은지를 대는데 여기만 입을 다물면, 같은 명령이 선 자리에
    /// 따라 짧아진 목록을 "할 일이 없다" 로 읽는다.
    pub focus: crate::report::Focus<'a>,
    /// 못 읽는 줄의 수. 그 줄에 있던 일은 목록에서 빠져 있다.
    pub unreadable: usize,
    pub origin: &'a Origin,
    pub trouble: &'a [crate::worktree::Trouble],
}

/// 겹쳐 본 화면의 머리 꼬리 — `   ⎇ <워크트리들> 겹쳐 봄`. 안 겹쳤으면 빈 글.
///
/// **겹쳐 본 화면은 머리에서 그렇다고 말한다.** 줄마다 붙는 `⎇` 는 옆에서 온
/// 줄에만 서므로, 옆 워크트리가 조용하면 겹쳐 본 보드와 제 보드가 똑같이 보인다.
///
/// **이 꼬리도 말묶음에서 온다**(리뷰) — `status` 의 머리 줄에 그대로 이어 붙으므로, 여기만
/// 한국어로 박혀 있으면 `--worktree` 로 부른 사람의 **첫 줄**이 반쪽만 제 말이 된다.
fn overlaid(screen: Screen) -> String {
    let lang = screen.lang;
    let trees = screen.labels();
    match trees.is_empty() {
        true => String::new(),
        false => format!(
            "   {}",
            paint(
                style::BRANCH,
                &format!(
                    "{} {}",
                    style::BRANCH_GLYPH,
                    fill(say(lang, "status.overlaid"), &[("trees", &clip(&trees.join(", "), TITLE_CAP))])
                )
            )
        ),
    }
}

/// 옆 워크트리를 겹치다 만난 것을 한 줄씩. **막지 않는다** — `!` 로 말만 한다.
fn troubles(out: &mut Vec<String>, trouble: &[crate::worktree::Trouble], lang: Lang) {
    said_troubles(out, trouble.iter().map(|t| trouble_line(lang, t)));
}

/// 이미 지어진 줄을 같은 모양으로 — 못 읽은 워크트리 줄([`unread_worktree`])이 이 길로 온다.
fn said_troubles(out: &mut Vec<String>, said: impl Iterator<Item = String>) {
    for t in said {
        out.push(format!("  {} {}", paint(style::WARN, "!"), one_line(&t)));
    }
}

/// 한 프로젝트에서 집은 것을 몇 줄까지 보이나. 한눈 보기는 프로젝트가 여럿이라 짧게 끊는다.
const PICKED_SHOWN: usize = 3;
/// 한 프로젝트에서 집을 것을 몇 줄까지 보이나.
const READY_SHOWN: usize = 5;

/// 등록한 프로젝트마다 보드 요약 — 칸별 수, 집은 것, 경고 수.
///
/// **줄마다 프로젝트 이름을 id 곁에 단다.** 프로젝트끼리 id 가 겹칠 수 있고(접두어가
/// 같은 두 저장소), 머리에만 이름을 두면 `grep` 으로 뽑은 줄이 어느 것인지 모른다.
/// 이름 칸과 id 칸, 머리의 이름에 프로젝트 색([`style::project_colour`])을 얹는다 — 머리가
/// 색의 범례가 되고, 색을 꺼도 이름이 남아 **색이 혼자 뜻을 지지 않는다.** 칠하는 것만
/// 더해 글자와 칸 폭은 그대로라, 색을 끈 화면은 색을 얹기 전과 바이트까지 같다.
pub fn projects_status(
    projects: &[crate::projects::Project],
    seen: &[crate::projects::Seen<Board>],
    reg: &crate::user_config::Registry,
    screen: Screen,
) -> Vec<String> {
    let lang = screen.lang;
    let mut out = vec![overview_head(
        say(lang, "overview.projects"),
        &fill(say(lang, "overview.places"), &[("places", &projects.len().to_string())]),
        reg,
    )];
    let w_name = projects.iter().map(|p| width(&one_line(&p.name))).max().unwrap_or(0);
    for (p, s) in projects.iter().zip(seen) {
        out.push(String::new());
        let crate::projects::Seen::Ok(b) = s else {
            out.push(project_head(p, ""));
            out.push(unopened(p, s, lang));
            continue;
        };
        // **이 프로젝트의 화면으로 갈아 끼운다** — 머리 꼬리와 아래 줄마다의 `⎇` 가 같은 출처를
        // 본다. 한쪽만 `over` 를 지나고 다른 쪽이 `b.origin` 을 직접 물으면, 같은 것을 묻는 길이
        // 한 고리 안에 둘이 된다.
        let screen = screen.over(b.origin);
        out.push(project_head(p, overlaid(screen).trim_start()));
        out.push(board(b.cfg, &b.status.counts));
        let shown = &b.picked[..b.picked.len().min(PICKED_SHOWN)];
        // **id 도 제목도 남의 스냅샷에서 온다** — 읽기는 관대해 `\n` 말고는 아무것도 안 막으므로,
        // 걸러서 찍는다(이 줄의 이름·경로가 이미 그렇다). 폭도 거른 뒤에 잰다: 날 글자로 재면
        // 걸러 낸 만큼 칸이 남아 제목 칸이 줄마다 어긋난다.
        let ids: Vec<String> = shown.iter().map(|i| one_line(&i.id)).collect();
        // 보인 것끼리 id 폭을 맞춘다 — 자식 id(`x-1a2b.3`)가 섞이면 줄마다 제 폭으로는 제목 칸이 어긋난다.
        let w_id = ids.iter().map(|id| width(id)).max().unwrap_or(0);
        let hue = style::project_colour(&p.path, p.hue);
        for (i, id) in shown.iter().zip(&ids) {
            out.push(format!(
                "  {}{}{}  {}",
                cell(hue, &one_line(&p.name), w_name + 2),
                cell(hue, id, w_id + 2),
                paint(style::status_style(i.status.as_str()), style::glyph(i.status.as_str())),
                marked(screen.branch(&i.id), &one_line(&i.title), TITLE_CAP, style::PLAIN).0,
            ));
        }
        let rest = b.picked.len().saturating_sub(PICKED_SHOWN);
        if rest > 0 {
            let said = fill(say(lang, "overview.picked_more"), &[("n", &rest.to_string())]);
            out.push(format!("  {}", paint(style::DIM, &said)));
        }
        troubles(&mut out, b.trouble, lang);
        // **`gather` 가 이미 낸 워크트리는 두 번 안 센다** — `cmd::status` 의 `said_already` 와 같은
        // 자(`Gathered::swept`)다. 옆 스냅샷을 빠짐없이 열었으면 같은 워크트리를 `⎇ <가지>: …` 로
        // 이미 `trouble` 에 담았다. 겹쳐 세면 깨진 워크트리 하나가 `옆 워크트리 문제 2건` 으로 서서
        // 보는 쪽이 두 곳이 깨진 줄로 읽는다. git 이 없어 `gather` 가 한 곳도 못 세었을 때는 이쪽이
        // 말해야 한다(리뷰 moai-ya06). 한때 여기만 가지 이름 앞머리로 견줘, 같은 커밋에 떼어 낸
        // HEAD 둘을 하나로 읽었다(moai-rgz9).
        //
        // **센 것은 한 줄씩 댄다** — 안쪽 `moai status` 가 stderr 에 내는 그 말이다. 수만 세고 줄을
        // 안 내면 아래의 `옆 워크트리 문제 N건 — 위 줄` 이 없는 줄을 가리키고, 보는 쪽은 어느
        // 워크트리를 고칠지 모른다.
        // 글은 [`unread_worktree`] 한 자리에서 짓는다 — 안쪽 `moai status` 가 stderr 에 내는
        // 그 줄과 같은 것이어야 한다(moai-dpbi). **셈은 줄에서 다시 세지 않는다**(리뷰) — 낸 줄을
        // 한 벌 더 들고 있을 까닭이 없다.
        let unread = match b.swept {
            true => 0,
            false => {
                said_troubles(&mut out, b.unread.iter().map(|t| unread_worktree(lang, &t.branch, &t.path)));
                b.unread.len()
            }
        };
        // **알림은 세지 않는다** — `moai status` 의 "드러난 문제 없다" 와 같은 자다.
        let n = b.status.warnings.len();
        let fatal = b.status.warnings.iter().filter(|w| w.fatal).count();
        let go = paint(style::DIM, &format!("→ `moai -C {} status`", shell_arg(&p.path)));
        // 옆 워크트리의 문제는 화면에서만 센다 — 위에 `!` 줄로 섰는데 밑에서 "문제 없다" 면
        // 덩어리가 제 말을 뒤집는다(moai-cuw2, `status` 와 같은 자).
        let t = b.trouble.len() + unread;
        let beside = match t {
            0 => String::new(),
            _ => format!(" · {}", fill(say(lang, "overview.worktree_trouble"), &[("n", &t.to_string())])),
        };
        // **`say` 부름은 갈래마다 제 줄이다**([`says`] 와 같은 까닭) — 키를 도우미에 넘기면
        // 소스를 훑는 시험(`i18n::tests::keys_in`)의 눈에서 그 키가 통째로 사라진다.
        // **"드러난 문제 없다" 는 저장소 안의 `status` 와 키가 같다** — 한 도구가 같은 것을
        // 두 말로 말하지 않는다.
        out.push(match (n, fatal) {
            (0, _) if t == 0 => {
                format!("  {} {}", paint(style::status_style("done"), "✓"), say(lang, "status.all_clear"))
            }
            (0, _) => format!(
                "  {} {}",
                paint(style::WARN, "!"),
                fill(say(lang, "overview.worktree_trouble_above"), &[("n", &t.to_string())])
            ),
            (_, 0) => format!(
                "  {} {}{beside}  {go}",
                paint(style::WARN, "!"),
                fill(say(lang, "overview.warnings"), &[("n", &n.to_string())])
            ),
            (_, f) => format!(
                "  {} {}{beside}  {go}",
                paint(style::ERROR, "!"),
                fill(say(lang, "overview.warnings_fatal"), &[("n", &n.to_string()), ("f", &f.to_string())])
            ),
        });
    }
    problems(&mut out, reg, lang);
    out.push(String::new());
    out.push(paint(style::DIM, say(lang, "overview.next")));
    out
}

/// 등록한 프로젝트마다 집을 수 있는 일 — 앞의 몇 건만.
pub fn projects_ready(
    projects: &[crate::projects::Project],
    seen: &[crate::projects::Seen<Picks>],
    reg: &crate::user_config::Registry,
    screen: Screen,
) -> Vec<String> {
    use crate::projects::Seen;
    let lang = screen.lang;
    let total: usize = seen
        .iter()
        .map(|s| match s {
            Seen::Ok(k) => k.picks.len(),
            _ => 0,
        })
        .sum();
    // **한 명령이 두 말로 말하지 않는다**(리뷰 moai-80qw.cb8) — 저장소 안의 `ready` 는
    // 말묶음에서 머리를 읽는데 여기만 한국어로 박혀 있으면, 같은 명령이 선 자리에 따라
    // 다른 말로 답한다. 덜 옮긴 것과 서로 어긋나는 것은 다른 일이다.
    let mut out = vec![overview_head(
        say(lang, "overview.ready"),
        &fill(say(lang, "overview.tally"), &[("places", &projects.len().to_string()), ("n", &total.to_string())]),
        reg,
    )];
    let w_name = projects.iter().map(|p| width(&one_line(&p.name))).max().unwrap_or(0);
    for (p, s) in projects.iter().zip(seen) {
        out.push(String::new());
        let Seen::Ok(k) = s else {
            out.push(project_head(p, ""));
            out.push(unopened(p, s, lang));
            continue;
        };
        // 머리와 줄이 같은 출처를 본다 — `projects_status` 와 같은 자리다.
        let screen = screen.over(k.origin);
        // **셈 하나에도 말이 든다** — 한국어의 `건` 이 여기 박혀 있던 동안, 말묶음에서 온
        // `overlaid` 꼬리와 한 줄에 서서 `1건   ⎇ feat/x overlaid` 가 나왔다(리뷰 moai-4y5s.jy3 #5).
        let count = fill(say(lang, "overview.picks"), &[("n", &k.picks.len().to_string())]);
        out.push(project_head(p, &format!("{count}{}", overlaid(screen))));
        let shown = &k.picks[..k.picks.len().min(READY_SHOWN)];
        // 남의 스냅샷에서 온 글자다 — 걸러서 찍고 걸러서 잰다(`projects_status` 와 같은 까닭).
        let ids: Vec<String> = shown.iter().map(|i| one_line(&i.id)).collect();
        let w_id = ids.iter().map(|id| width(id)).max().unwrap_or(0);
        let hue = style::project_colour(&p.path, p.hue);
        for (i, id) in shown.iter().zip(&ids) {
            out.push(format!(
                "  {}{}{}{}",
                cell(hue, &one_line(&p.name), w_name + 2),
                cell(hue, id, w_id + 2),
                cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
                marked(screen.branch(&i.id), &one_line(&i.title), TITLE_CAP, style::PLAIN).0,
            ));
        }
        let rest = k.picks.len() - shown.len();
        if rest > 0 {
            // **`status` 의 경고 꼬리와 키가 같다**(`status.more`) — 같은 "N건 더" 를 두 키로
            // 두면 한쪽만 옮긴 말에서 한 화면이 두 모양으로 센다. 뒤의 명령은 자료라 그대로다.
            let more = fill(say(lang, "status.more"), &[("n", &rest.to_string())]);
            let go = format!("{more} → `moai -C {} ready`", shell_arg(&p.path));
            out.push(format!("  {}", paint(style::DIM, &go)));
        }
        // **짧아진 목록은 왜 짧은지를 여기서도 댄다**(저장소 안의 `ready` 와 같은 글자·같은
        // 글리프). 뺀 것이 없으면 댈 것도 없으니 입을 다문다 — 어느 마일스톤이 도는지는 그
        // 프로젝트의 `ready` 가 낸다.
        if !k.focus.outside.is_empty() {
            let said = fill(say(lang, "ready.outside_held"), &[("n", &k.focus.outside.len().to_string())]);
            out.push(format!("  {} {}", paint(style::DIM, "+"), paint(style::DIM, &said)));
        }
        // 목록 꼬리("N건 더") 뒤에 둔다 — 앞에 두면 그 꼬리가 문제 줄의 연속으로 읽힌다(`projects_status` 와 같은 차례).
        troubles(&mut out, k.trouble, lang);
        if k.unreadable > 0 {
            let go = format!("moai -C {} show", shell_arg(&p.path));
            let said = fill(say(lang, "overview.unreadable"), &[("n", &k.unreadable.to_string()), ("go", &go)]);
            out.push(format!("  {} {said}", paint(style::ERROR, "!")));
        }
    }
    problems(&mut out, reg, lang);
    out
}

/// 한눈 보기의 머리 — 무엇을 몇이나 봤는지와, 목록을 읽은 사용자 설정 파일.
fn overview_head(what: &str, count: &str, reg: &crate::user_config::Registry) -> String {
    let at = reg.path.as_ref().map(|p| one_line(&p.display().to_string())).unwrap_or_default();
    format!("{}  {count}       {}", paint(style::HEAD, what), paint(style::DIM, &at)).trim_end().to_string()
}

fn project_head(p: &crate::projects::Project, tail: &str) -> String {
    let at = one_line(&p.path.display().to_string());
    let head = style::project_colour(&p.path, p.hue).effects(style::HEAD.get_effects());
    format!("{}  {}   {tail}", paint(head, &one_line(&p.name)), paint(style::DIM, &at)).trim_end().to_string()
}

/// 열지 못한 프로젝트의 한 줄. **무엇을 하면 되는지를 함께 댄다.**
///
/// **이 줄도 한눈 보기의 몸통이다**(moai-el7z, 리뷰 moai-hom6.soc 의 3번). 등록만 하고 아직
/// `init` 하지 않은 프로젝트 하나면 영어로 고른 화면에 이 줄이 한국어로 선다 — 옮긴 나머지와
/// 한 덩어리 안에서 말이 갈린다.
///
/// **명령은 자료라 `{go}` 한 자리로 든다.** 번역자가 옮길 것은 그 앞의 문장뿐이고, `moai -C …`
/// 는 붙여 넣으면 도는 글자 그대로여야 한다.
pub(crate) fn unopened<T>(p: &crate::projects::Project, s: &crate::projects::Seen<T>, lang: Lang) -> String {
    use crate::projects::Seen;
    let at = shell_arg(&p.path);
    match s {
        Seen::Ok(_) => String::new(),
        // init 전은 고칠 것이 아니다 — 나중에 `init` 하면 보이는 것이 요구다. `!` 를 달지 않는다.
        //
        // **딸린 워크트리면 여기가 아니라 주 체크아웃을 댄다**(moai-nppo). `init` 은 그 자리를 이미
        // 거절하는데(moai-mz0e) 이 줄만 그 갈래를 몰라, 등록한 워크트리 한 줄이 영영 `init 전` 으로
        // 서고 그 줄이 대는 명령은 1 로 끝났다. 가르는 자는 [`crate::store::init_belongs_at`] 하나다.
        Seen::Uninit { tracker_at } => {
            // **키는 낱말째 적는다** — 소스를 훑는 시험(`i18n::tests::keys_in`)은 `say(…, "키")` 모양만
            // 읽어, 변수로 넘기면 두 키가 그 눈에서 통째로 사라진다.
            //
            // **자리는 받아 쓰기만 한다**(리뷰 10번) — 이 모듈은 순수 함수라는 글을 머리에 달고 있고,
            // 여기서 물으면 탐색기의 층이 줄마다 걸음마다 `canonicalize` 를 치른다. 세는 자리는
            // 프로젝트를 여는 쪽 하나다(`projects::State::Uninit`).
            //
            // 대는 명령이 `init` 이 아니라 등록인 까닭은 `cmd::project::uninit_line` 에 있다 —
            // 그 자리가 서려면 주 체크아웃에 트래커가 이미 있어야 해서 `init` 은 아무것도 안 바꾼다.
            let (said, go) = match tracker_at {
                Some(main) => (say(lang, "overview.uninit_worktree"), format!("moai project add {}", shell_arg(main))),
                None => (say(lang, "overview.uninit"), format!("moai -C {at} init")),
            };
            let said = fill(said, &[("go", &go)]);
            format!("  {} {}", paint(style::DIM, "·"), paint(style::DIM, &said))
        }
        Seen::Missing => {
            let go = fill(say(lang, "overview.missing_go"), &[("go", &format!("moai project rm {at}"))]);
            format!("  {} {}  {}", paint(style::WARN, "!"), say(lang, "overview.missing"), paint(style::DIM, &go))
        }
        // **까닭은 한 줄에 둔다** — `sanitize` 는 줄바꿈을 남기므로 그대로 쓰면 뒤가
        // 다음 줄로 흘러 옆 프로젝트의 줄과 안 갈린다. 이 글은 층의 알림(`layer::shut`)
        // 으로도 그대로 가는데 거기는 한 줄짜리 자리다.
        Seen::Unreadable { error } => format!(
            "  {} {}",
            paint(style::ERROR, "!"),
            fill(say(lang, "overview.project_unreadable"), &[("why", &one_line(error))])
        ),
    }
}

/// 사용자 설정을 읽다 만난 것을 한 줄씩. 목록을 막지 않는다.
///
/// **화면 말의 탈은 말묶음에서 온다**([`problem`], moai-dpbi) — 설정을 읽는 길은 말을 모른다.
/// 나머지 줄(항목의 탈)은 아직 지어진 글 그대로다.
fn problems(out: &mut Vec<String>, reg: &crate::user_config::Registry, lang: Lang) {
    let said = settings_problems(reg, lang);
    if said.is_empty() {
        return;
    }
    out.push(String::new());
    for p in said {
        out.push(format!("{} {}", paint(style::WARN, "!"), one_line(&p)));
    }
}

/// 사용자 설정을 읽다 만난 것 — 지어진 줄과 말묶음에서 펴는 줄을 **한 차례로** 잇는다
/// (moai-dpbi). 사람 화면과 `--json` 의 `problems` 가 이것을 나눠 쓴다: 갈라 적으면 한쪽만
/// 고쳐져 기계가 받는 목록이 화면과 달라진다.
pub fn settings_problems(reg: &crate::user_config::Registry, lang: Lang) -> Vec<String> {
    let at = reg.path.as_deref();
    let mine = reg.problems.iter().map(|t| config_problem(lang, at, t));
    let said = reg.lang_problems.iter().map(|t| problem(lang, at, t));
    mine.chain(said).collect()
}

/// 탐색기의 보기 알림에 설 줄 — 보기를 읽다 만난 까닭과 **옛 `[read]` 에서 건너뛴 줄**(moai-rtji).
///
/// 둘 다 **자료로 온다**(moai-uzgp 이 보기 쪽도 옮겼다) — 설정을 읽는 자리는 화면 말을 안 묻으므로
/// 여기서 편다. **자리를 머리에 붙인다** — 한때 설정 쪽이 글에 붙이던 그 꼴(`{설정}: …`)이라,
/// 안 붙이면 "손으로 고친다" 가 어느 파일인지 모르는 말이 된다.
///
/// **띄우는 길과 시험이 이 한 자를 지난다**(리뷰) — `cmd::tui` 에만 두던 판은 시험의 `App::load_look` 이
/// 옛 `[read]` 의 줄을 못 받아, 그 줄을 빠뜨려도 시험이 푸르렀다. [`settings_problems`] 와 같은 자리다.
pub fn look_problems(reg: &crate::user_config::Registry, lang: Lang) -> Vec<String> {
    let at = reg.path.as_deref();
    let told = |said: String| match at {
        Some(at) => format!("{}: {said}", at.display()),
        None => said,
    };
    let looks = reg.look_problems.iter().map(|why| told(look_trouble(lang, why)));
    let reads = reg.read_problems.iter().map(|why| told(skipped(lang, why)));
    looks.chain(reads).collect()
}

/// 쓰기가 거절한 까닭의 글([`crate::model::Invalid`], moai-yve0).
///
/// **가리키는 말은 부르는 쪽이 준다**(`at`) — 이번 쓰기가 짓는 줄은 거절 뒤에 그 id 가 어디에도
/// 안 남으므로(moai-1rkl), 이미 선 줄이면 id 를, 새 줄이면 `model::unwritten` 의 제목을 준다.
/// **id 가 모양에 안 맞는 줄만 그 머리가 없다** — 댈 id 가 없는 것이 곧 그 까닭이라서다.
pub fn invalid(lang: Lang, at: &crate::store::At, why: &crate::model::Invalid) -> String {
    let at = match at {
        crate::store::At::Id(id) => id.clone(),
        crate::store::At::Unwritten(title) => fill(say(lang, "invalid.unwritten"), &[("title", title)]),
    };
    invalid_at(lang, &at, why)
}

/// [`invalid`] 의 속 — 가리키는 말이 이미 지어졌을 때.
fn invalid_at(lang: Lang, at: &str, why: &crate::model::Invalid) -> String {
    use crate::model::{Field, Invalid};
    // **낱말은 키로 적는다** — 소스를 훑는 시험이 `say(…, "키")` 모양만 읽는다.
    let field = |f: &Field| match f {
        Field::Assignee => say(lang, "invalid.field_assignee"),
        Field::AssigneeEmail => say(lang, "invalid.field_assignee_email"),
        Field::Epic => say(lang, "invalid.field_epic"),
        Field::Milestone => say(lang, "invalid.field_milestone"),
    };
    let said = match why {
        Invalid::Id { id } => return fill(say(lang, "invalid.id"), &[("id", id)]),
        Invalid::EmptyTitle => say(lang, "invalid.empty_title").to_string(),
        Invalid::TitleLines => say(lang, "invalid.title_lines").to_string(),
        Invalid::FieldLines { field: f, value } => {
            fill(say(lang, "invalid.field_lines"), &[("field", field(f)), ("value", value)])
        }
        Invalid::Priority { max, value } => {
            fill(say(lang, "invalid.priority"), &[("max", &max.to_string()), ("value", value)])
        }
        Invalid::Tag { tag } => fill(say(lang, "invalid.tag"), &[("tag", tag)]),
        Invalid::GroupId { field: f, value } => {
            fill(say(lang, "invalid.group_id"), &[("field", field(f)), ("value", value)])
        }
        Invalid::MilestoneInMilestone { id } => fill(say(lang, "invalid.milestone_in_milestone"), &[("id", id)]),
        Invalid::SelfBlock => say(lang, "invalid.self_block").to_string(),
        Invalid::BlockedId { value } => fill(say(lang, "invalid.blocked_id"), &[("value", value)]),
        Invalid::NoSuchColumn(e) => no_such_column(lang, e),
    };
    format!("{at}: {said}")
}

/// 쓰기 경로가 자료로 들고 나온 까닭의 글([`crate::store::Trouble`], moai-iq7j).
///
/// **저장 계층은 화면 말을 모른다**(2026-09-20 사용자 결정) — 락을 쥔 채 도는 자리고, 머지
/// 드라이버는 사용자 설정을 아예 안 연다. 그래서 글을 짓는 자리가 여기 하나다.
pub fn store_trouble(lang: Lang, why: &crate::store::Trouble) -> String {
    use crate::store::Trouble;
    match why {
        Trouble::NotADirectory { at } => fill(say(lang, "store.not_a_directory"), &[("at", at)]),
        Trouble::DuplicateId { id } => fill(say(lang, "store.duplicate_id"), &[("id", id)]),
        Trouble::LockBusy { secs } => fill(say(lang, "store.lock_busy"), &[("secs", &secs.to_string())]),
        // **말이 함께 사라진 줄만 이름을 댄다** — 하나도 없으면 io 가 낸 줄 그대로다.
        Trouble::JournalLost { said, ids } if ids.is_empty() => said.clone(),
        Trouble::JournalLost { said, ids } => {
            fill(say(lang, "store.journal_lost"), &[("said", said), ("ids", &ids.join(" "))])
        }
        Trouble::Invalid { at, why } => invalid(lang, at, why),
    }
}

/// 보기 설정(`[tui]`)을 읽다 만난 한 줄([`crate::user_config::LookTrouble`], moai-uzgp).
///
/// **자리(`{설정}: …`)는 안 붙인다** — 붙이는 자는 [`look_problems`] 하나고, 여기서도 붙이면
/// 두 번 선다.
pub fn look_trouble(lang: Lang, why: &crate::user_config::LookTrouble) -> String {
    use crate::user_config::{LookTrouble, TUI, Want};
    // **키는 낱말째 적는다** — 소스를 훑는 시험(`i18n::tests`)은 `say(…, "키")` 모양만 읽어,
    // 키를 변수나 `match` 의 팔로 넘기면 그 눈에서 통째로 사라진다.
    let want = |w: &Want| match w {
        Want::Bool => say(lang, "look.want_bool"),
        Want::Word => say(lang, "look.want_word"),
        Want::Words => say(lang, "look.want_words"),
    };
    match why {
        LookTrouble::NotATable { found } => fill(say(lang, "look.not_a_table"), &[("key", TUI), ("found", found)]),
        LookTrouble::Want { key, want: w, found } => {
            fill(say(lang, "look.want"), &[("key", &format!("{TUI}.{key}")), ("want", want(w)), ("found", found)])
        }
        LookTrouble::NotAWord { key, value } => {
            fill(say(lang, "look.not_a_word"), &[("key", &format!("{TUI}.{key}")), ("value", value)])
        }
    }
}

/// 설정을 읽다 만난 한 줄([`crate::user_config::ConfigTrouble`], moai-aiid).
///
/// **자리를 앞에 단다** — [`problem`] 과 같은 까닭이고 같은 모양이다. 자리를 모른다는 줄만
/// 그 앞이 빈다: 붙일 파일이 없어서 그 줄이 선 것이다.
pub fn config_problem(lang: Lang, at: Option<&std::path::Path>, why: &crate::user_config::ConfigTrouble) -> String {
    use crate::user_config::{ConfigTrouble, PROJECT};
    let said = match why {
        ConfigTrouble::NoPlace => say(lang, "warn.config_no_place").to_string(),
        // **남의 글은 옮기지 않는다** — io·toml 이 낸 줄이라 말묶음에 키를 둘 자리가 없다.
        ConfigTrouble::Said { said } => said.clone(),
        ConfigTrouble::NotTables { found } => {
            fill(say(lang, "warn.config_not_tables"), &[("key", PROJECT), ("is", found)])
        }
        ConfigTrouble::Entry { nth, why } => fill(
            say(lang, "warn.entry_nth"),
            &[("key", PROJECT), ("n", &nth.to_string()), ("said", &entry_problem(lang, why))],
        ),
    };
    match at {
        Some(at) => format!("{}: {said}", at.display()),
        None => said,
    }
}

/// `[[project]]` 항목 하나의 탈. **뒷말은 한 자리에서 붙인다**
/// ([`crate::user_config::EntryTrouble::falls_back`]) — 줄을 안 버리고 경로로 고른 색으로
/// 세우는 갈래가 셋이라, 갈래마다 키를 나누면 같은 뒷말이 표에 세 번 선다.
fn entry_problem(lang: Lang, why: &crate::user_config::EntryTrouble) -> String {
    use crate::user_config::{COLOR, COLOUR, EntryTrouble, PATH};
    // **`say` 부름은 갈래마다 제 줄이다**([`problem`] 과 같은 까닭) — 키를 도우미에 넘기면
    // 소스를 훑는 시험(`i18n::tests::keys_in`)의 눈에서 그 키가 사라진다.
    let said = match why {
        EntryTrouble::NoPath => fill(say(lang, "warn.entry_no_path"), &[("key", PATH)]),
        // **`path` 와 `color` 가 한 키를 나눠 쓴다** — 두 줄은 `{key}` 만 다른 같은 글이라,
        // 키를 갈라 두면 말묶음 다섯이 같은 문장을 두 번씩 이고 한쪽만 고쳐질 자리가 생긴다.
        EntryTrouble::PathNotAWord { found } => fill(say(lang, "warn.entry_word"), &[("key", PATH), ("is", found)]),
        // **적힌 값은 따옴표째 낸다** — 빈 값이나 공백만 적은 것이 그대로면 아무것도 안 보인다.
        EntryTrouble::PathNotAbsolute { raw } => {
            fill(say(lang, "warn.entry_path_abs"), &[("key", PATH), ("raw", &format!("{raw:?}"))])
        }
        EntryTrouble::HueNotAWord { found } => fill(say(lang, "warn.entry_word"), &[("key", COLOR), ("is", found)]),
        EntryTrouble::HueUnknown(e) => {
            fill(say(lang, "warn.entry_hue_unknown"), &[("key", COLOR), ("said", &not_a_hue(lang, e))])
        }
        EntryTrouble::ColourInstead => fill(say(lang, "warn.entry_colour_instead"), &[("bad", COLOUR), ("key", COLOR)]),
        EntryTrouble::ColourIgnored => fill(say(lang, "warn.entry_colour_ignored"), &[("bad", COLOUR), ("key", COLOR)]),
    };
    match why.falls_back() {
        true => fill(say(lang, "warn.entry_hue_falls_back"), &[("said", &said)]),
        false => said,
    }
}

/// 설정을 고치다 멈춘 한 줄([`crate::user_config::WriteTrouble`], moai-wflg).
///
/// **자리를 앞에 단다** — [`config_problem`] 과 같은 까닭이고 같은 모양이다. 자리를 제 글에
/// 이미 든 갈래(링크·경로)만 앞이 빈다.
pub fn write_trouble(lang: Lang, at: Option<&std::path::Path>, why: &crate::user_config::WriteTrouble) -> String {
    use crate::user_config::{COLOR, PROJECT, TUI, WriteTrouble};
    // **갈래마다 제 `say` 를 적는다**([`problem`] 과 같은 까닭) — 키를 도우미에 넘기면 소스를
    // 훑는 시험(`i18n::tests::keys_in`)의 눈에서 그 키가 사라진다.
    let said = match why {
        // **남의 글은 옮기지 않는다** — toml 이 낸 줄이라 말묶음에 키를 둘 자리가 없다.
        WriteTrouble::Unparsable { said } => fill(say(lang, "refuse.config_unparsable"), &[("said", said)]),
        WriteTrouble::LinkDangling { from, to } => fill(
            say(lang, "refuse.config_link_dangling"),
            &[("from", &from.display().to_string()), ("to", &to.display().to_string())],
        ),
        WriteTrouble::NotTables { found } => {
            fill(say(lang, "refuse.project_not_tables"), &[("key", PROJECT), ("is", found)])
        }
        WriteTrouble::HueNotPlain { at, found } => {
            fill(say(lang, "refuse.hue_not_plain"), &[("at", &at.display().to_string()), ("key", COLOR), ("is", found)])
        }
        WriteTrouble::LookNotATable { found } => {
            fill(say(lang, "refuse.look_not_a_table"), &[("key", TUI), ("is", found)])
        }
        WriteTrouble::LookKeyNotPlain { key, found } => {
            fill(say(lang, "refuse.look_key_not_plain"), &[("key", &format!("{TUI}.{key}")), ("is", found)])
        }
        // **적힌 바이트째 낸다** — `display()` 는 탈이 난 바로 그 바이트를 U+FFFD 로 바꿔, 제 까닭을
        // 지운 글이 된다(리뷰). `Debug` 는 `\xNN` 으로 펴므로 사람이 어느 자리인지 보고 되칠 수 있다.
        // 바로 아래 `PathNotAbsolute` 와 한 모양이다.
        WriteTrouble::PathNotUtf8 { at } => fill(say(lang, "refuse.path_not_utf8"), &[("at", &format!("{at:?}"))]),
        WriteTrouble::PathNotAbsolute { raw } => {
            fill(say(lang, "refuse.path_not_absolute"), &[("raw", &format!("{raw:?}"))])
        }
        WriteTrouble::NotADirectory { at } => {
            fill(say(lang, "refuse.not_a_directory"), &[("at", &at.display().to_string())])
        }
    };
    match at {
        Some(at) => format!("{}: {said}", at.display()),
        None => said,
    }
}

/// 읽음 표를 읽고 쓰다 만난 것 가운데 **안 막는 것**의 글([`crate::read_marks::SheetTrouble`], moai-rtji).
/// 어느 파일인지를 머리에 붙인다 — 이름이 뿌리의 해시라 사람이 짐작할 수 없어, 안 붙이면 "손으로
/// 지운다" 가 갈 곳 없는 말이 된다.
///
/// **말은 부르는 쪽이 준다** — 읽음 모듈은 화면 말을 모른다. `moai read` 는 `Ctx` 로, 탐색기는
/// `Site::lang` 으로 받아 여기로 넘긴다.
pub fn sheet_trouble(lang: Lang, why: &crate::read_marks::SheetTrouble) -> String {
    use crate::read_marks::SheetTrouble;
    let (at, said) = match why {
        // **남의 글은 옮기지 않는다** — io·toml 이 낸 줄이라 말묶음에 키를 둘 자리가 없다.
        SheetTrouble::Said { at, said } => (at, said.clone()),
        SheetTrouble::Unsettled { at, said } => (at, fill(say(lang, "sheet.unsettled"), &[("said", said)])),
        SheetTrouble::NotOurs { at, root } => {
            (at, fill(say(lang, "sheet.not_ours"), &[("root", &root.display().to_string())]))
        }
        SheetTrouble::Skipped { at, why } => (at, skipped(lang, why)),
        SheetTrouble::SpoolLeft { at, said } => (at, fill(say(lang, "sheet.spool_left"), &[("said", said)])),
        // 막힌 id 가 없으면 파일을 다 못 읽은 것이다 — 그 까닭은 같은 판에 함께 실린 줄이 댄다.
        SheetTrouble::SpoolKept { at, held } if held.is_empty() => (at, say(lang, "sheet.spool_kept").to_string()),
        SheetTrouble::SpoolKept { at, held } => (at, fill(say(lang, "sheet.spool_held"), &[("ids", &held.join(", "))])),
    };
    format!("{}: {said}", at.display())
}

/// `[read]` 표에서 건너뛴 줄 하나의 글([`crate::read_marks::Skipped`]). **어느 파일인지는 안 붙인다** —
/// 같은 줄이 읽음 파일에서도 설정의 옛 `[read]` 에서도 오므로, 붙이는 것은 그 자리를 아는 쪽이다
/// ([`sheet_trouble`]·옛 `[read]` 의 [`look_problems`]).
fn skipped(lang: Lang, why: &crate::read_marks::Skipped) -> String {
    use crate::read_marks::{READ, Skipped};
    match why {
        Skipped::NotATable { found } => fill(say(lang, "sheet.not_a_table"), &[("table", READ), ("is", found)]),
        Skipped::NotAStamp { id, found } => {
            fill(say(lang, "sheet.not_a_stamp"), &[("key", &format!("{READ}.{id}")), ("is", found)])
        }
    }
}

/// 읽음 표에 **안 쓰고 멈춘** 까닭의 글([`crate::read_marks::SheetRefusal`], moai-rtji). 파일은
/// `read_marks::update` 가 준다 — 그 자리를 아는 것이 거기 하나라서다.
///
/// 손으로 적은 자리([`crate::read_marks::SheetRefusal::Hand`])는 읽는 길이 건너뛰는 것과 **같은 사실**이지만
/// 글이 다르다 — 읽기는 "건너뛴다" 고 지나가고, 쓰기는 "안 적는다 — 손으로 고친다" 고 멈춘다.
pub fn sheet_refusal(lang: Lang, at: &std::path::Path, why: &crate::read_marks::SheetRefusal) -> String {
    use crate::read_marks::{READ, SheetRefusal, Skipped};
    let said = match why {
        SheetRefusal::Unparsable { said } => fill(say(lang, "sheet.refuse_unparsable"), &[("said", said)]),
        SheetRefusal::NotOurs { root } => {
            fill(say(lang, "sheet.refuse_not_ours"), &[("root", &root.display().to_string())])
        }
        SheetRefusal::Hand(Skipped::NotATable { found }) => {
            fill(say(lang, "sheet.refuse_not_a_table"), &[("table", READ), ("is", found)])
        }
        SheetRefusal::Hand(Skipped::NotAStamp { id, found }) => {
            fill(say(lang, "sheet.refuse_not_a_stamp"), &[("table", READ), ("id", id), ("is", found)])
        }
    };
    format!("{}: {said}", at.display())
}

/// 모르는 칸 한 줄([`crate::config::NoSuchColumn`], moai-fdk7).
///
/// **두 거절은 잰 것이 다르다.** 설정만 보고 거절한 자리(`add -s`·`mv <칸>`)는 "그런 칸이
/// 없다" 고, 줄까지 보고 거절한 자리(`--from`·`show -s`·탐색기 거름망)는 "거기 선 줄도
/// 없다" 고 말한다 — 뒤쪽은 옛 이름에 선 줄이면 받아 주므로(moai-hym7), 같은 말을 하면
/// 받아 주는 자리와 아닌 자리를 읽는 쪽이 못 가린다.
pub fn no_such_column(lang: Lang, why: &crate::config::NoSuchColumn) -> String {
    let known = why.known.join(", ");
    // **갈래마다 제 `say` 를 적는다**([`problem`] 과 같은 까닭) — 키를 삼항으로 고르면
    // 소스를 훑는 시험(`i18n::tests::keys_in`)의 눈에서 그 키가 사라진다.
    match why.nor_rows {
        true => fill(say(lang, "refuse.no_column_nor_rows"), &[("name", &why.name), ("known", &known)]),
        false => fill(say(lang, "refuse.no_column"), &[("name", &why.name), ("known", &known)]),
    }
}

/// 누가 하는지 모를 때의 글([`crate::model::NoActor`], moai-ivt9).
///
/// **무엇을 주면 되는지를 함께 댄다** — 이 거절은 게이트가 아니라 입력이 모자라다는 말이고,
/// `--user` 와 `MOAI_ACTOR` 둘 다 사람 없이 채워진다. 어디를 고치는지 안 대면 그 자리에서
/// 사람을 부르는 것과 같아진다.
pub fn no_actor(lang: Lang, why: &crate::model::NoActor) -> String {
    use crate::model::NoActor;
    // **고칠 명령은 안 옮긴다** — 그대로 쳐야 하는 글자다(`.gitattributes` 가 심는 줄과 같은 까닭).
    // 말묶음에 안 두는 까닭이 하나 더 있다: 실린 글은 한 줄이어야 해서
    // (`i18n::tests::every_translation_keeps_the_places_english_marks`) 여러 줄은 여기서 잇는다.
    const HOW: &str = "  git config user.name  \"Name\"\n  git config user.email \"email\"";
    match why {
        NoActor::Unknown => {
            format!("{}\n\n{HOW}\n\n{}", say(lang, "refuse.no_actor"), say(lang, "refuse.actor_by_hand"))
        }
        NoActor::BadIdentity { label } => {
            let said = fill(say(lang, "refuse.bad_git_identity"), &[("label", &format!("{label:?}"))]);
            format!("{said}\n\n{HOW}")
        }
        NoActor::Malformed { what, raw } => {
            fill(say(lang, "refuse.actor_malformed"), &[("what", what), ("raw", &format!("{raw:?}"))])
        }
    }
}

/// git 에게 이력을 물었는데 못 받은 한 줄([`crate::git::Error`], moai-ivt9).
///
/// **무엇을 못 했는지까지 적는다**: "git 이 없다" 와 "저장소가 아니다" 는 받는 쪽이 할 일이
/// 다르다. 기계가 가르는 값은 이 글이 아니라 `Told::kind` 다 — 글은 사람의 것이라 화면 말로 선다.
pub fn git_trouble(lang: Lang, why: &crate::git::Error) -> String {
    use crate::git::Error;
    let said = why.said();
    match why {
        Error::Spawn(_) => fill(say(lang, "git.spawn"), &[("said", &said)]),
        Error::Failed(_) => fill(say(lang, "git.failed"), &[("said", &said)]),
        Error::Stream(_) => fill(say(lang, "git.stream"), &[("said", &said)]),
        Error::NotUtf8(_) => fill(say(lang, "git.not_utf8"), &[("said", &said)]),
    }
}

/// 저장소 설정(`.moai/config.toml`)을 읽다 멈춘 한 줄([`crate::config::Trouble`], moai-ivt9).
///
/// **자리는 [`config_refused`] 가 앞에 단다** — 읽은 파일을 아는 자는 [`crate::config::Config::load`]
/// 뿐이고, 글만 쓰는 [`crate::config::Config::parse`] 는 그 자리를 모른다.
pub fn config_trouble(lang: Lang, why: &crate::config::Trouble) -> String {
    use crate::config::{Thresholds, Trouble, Want};
    // **키는 낱말째 적는다** — 소스를 훑는 시험(`i18n::tests::keys_in`)은 `say(…, "키")` 모양만
    // 읽어, 키를 `match` 의 팔로 넘기면 그 눈에서 통째로 사라진다([`look_trouble`] 과 같은 자).
    let want = |w: &Want| match w {
        Want::Whole => say(lang, "config.want_whole"),
        Want::Fraction => say(lang, "config.want_fraction"),
    };
    let quoted = |s: &str| format!("{s:?}");
    match why {
        Trouble::Unreadable { said } => said.clone(),
        Trouble::Unbalanced { line } => fill(say(lang, "config.unbalanced"), &[("line", &line.to_string())]),
        Trouble::NotAPair { line } => fill(say(lang, "config.not_a_pair"), &[("line", &line.to_string())]),
        Trouble::NotQuoted { line, key, raw } => {
            fill(say(lang, "config.not_quoted"), &[("line", &line.to_string()), ("key", key), ("raw", &quoted(raw))])
        }
        Trouble::NumberQuoted { line, key, raw } => {
            fill(say(lang, "config.number_quoted"), &[("line", &line.to_string()), ("key", key), ("raw", raw)])
        }
        Trouble::NotANumber { line, key, want: w, raw } => fill(
            say(lang, "config.not_a_number"),
            &[("line", &line.to_string()), ("key", key), ("want", want(w)), ("raw", &quoted(raw))],
        ),
        Trouble::RatioRange { key, value } => fill(
            say(lang, "config.ratio_range"),
            &[("key", key), ("want", want(&Want::Fraction)), ("value", &value.to_string())],
        ),
        Trouble::PrefixCharset { raw } => fill(say(lang, "config.prefix_charset"), &[("raw", &quoted(raw))]),
        Trouble::PrefixDash { raw } => fill(say(lang, "config.prefix_dash"), &[("raw", &quoted(raw))]),
        Trouble::FlowDaysZero => say(lang, "config.flow_days_zero").to_string(),
        Trouble::NoSuchThreshold { line, key } => fill(
            say(lang, "config.no_such_threshold"),
            &[("line", &line.to_string()), ("key", key), ("known", &Thresholds::KEYS.join(", "))],
        ),
        Trouble::ThresholdInTable { line, named } => {
            fill(say(lang, "config.threshold_in_table"), &[("line", &line.to_string()), ("named", named)])
        }
        Trouble::NoPrefix => say(lang, "config.no_prefix").to_string(),
        Trouble::NoStatuses => say(lang, "config.no_statuses").to_string(),
        Trouble::NoDone { raw } => {
            fill(say(lang, "config.no_done"), &[("done", crate::config::DONE), ("raw", &quoted(raw))])
        }
        Trouble::StatusTwice { status } => fill(say(lang, "config.status_twice"), &[("status", status)]),
        Trouble::NamingUnknown { raw } => fill(
            say(lang, "config.naming_unknown"),
            &[("known", &crate::config::Naming::ALL.join("·")), ("raw", &quoted(raw))],
        ),
    }
}

/// [`config_trouble`] 에 읽던 파일의 자리를 앞에 단다([`crate::config::Refused`]).
///
/// **자리를 다는 자가 하나다**([`problem`]·[`config_problem`] 과 같은 까닭) — 부르는 쪽마다
/// 붙이면 같은 까닭이 자리 있는 모양과 없는 모양으로 갈린다.
pub fn config_refused(lang: Lang, why: &crate::config::Refused) -> String {
    format!("{}: {}", why.at.display(), config_trouble(lang, &why.why))
}

/// 팔레트 밖의 색 낱말 한 줄([`crate::user_config::NotAHue`]).
///
/// **읽기의 알림과 `moai project color` 의 거절문이 이 하나를 나눠 쓴다** — 명령이 받은 값을
/// 읽기가 틀렸다고 하거나 그 반대면, 고친 대로 적었는데 또 알림이 선다.
pub fn not_a_hue(lang: Lang, why: &crate::user_config::NotAHue) -> String {
    fill(
        say(lang, "warn.not_a_hue"),
        &[
            ("raw", &format!("{:?}", why.raw)),
            ("known", &crate::style::Hue::names().join("·")),
            ("auto", crate::user_config::AUTO),
        ],
    )
}

/// 설정에 적은 화면 말이 어긋난 한 줄([`crate::user_config::LangTrouble`]).
///
/// **자리를 앞에 단다** — 어느 파일의 어느 값인지 없으면 고칠 데를 못 찾는다. 설정을 읽는 쪽이
/// 안 붙이는 것은 그 반쪽이 말묶음에 들어야 해서다(`user_config::read`).
pub fn problem(lang: Lang, at: Option<&std::path::Path>, why: &crate::user_config::LangTrouble) -> String {
    use crate::user_config::{I18N, LANG, LangTrouble};
    let key = format!("{I18N}.{LANG}");
    let said = match why {
        LangTrouble::NotATable { found } => fill(say(lang, "warn.lang_table"), &[("key", I18N), ("is", found)]),
        LangTrouble::NotAWord { found } => fill(say(lang, "warn.lang_word"), &[("key", &key), ("is", found)]),
        LangTrouble::Unknown { raw } => {
            let known: Vec<&str> = Lang::ALL.into_iter().map(Lang::code).collect();
            // **적힌 값은 따옴표째 낸다** — 빈 값이나 공백만 적은 것이 그대로면 아무것도 안 보인다.
            let raw = format!("{raw:?}");
            fill(say(lang, "warn.lang_unknown"), &[("key", &key), ("known", &known.join("·")), ("raw", &raw)])
        }
    };
    match at {
        Some(at) => format!("{}: {said}", at.display()),
        None => said,
    }
}

/// 옆 워크트리를 겹치다 만난 한 줄([`crate::worktree::Trouble`], moai-dpbi).
///
/// **글리프와 자리는 말이 아니다** — 가지 이름과 경로는 자료고 `⎇` 는 표라, 말묶음에는 `{at}`
/// 한 자리로 든다(밖 한눈 보기의 `overview.unread_snapshot` 과 같은 자). 번역자가 옮길 것은 그
/// 앞뒤의 문장뿐이다.
pub fn trouble_line(lang: Lang, why: &crate::worktree::Trouble) -> String {
    use crate::worktree::{Lost, Trouble};
    match why {
        // **여기만 말묶음을 안 지난다** — 이 갈래는 가지와 열다 진 까닭만 댄다. `--worktree` 를
        // 안 줬을 때 같은 사실을 대는 [`unread_worktree`] 는 문장을 두르므로, 한 워크트리가 깨진
        // 것을 두 말로 대고 있다(리뷰가 짚었다). 문장을 맞추는 일은 그 두 갈래를 한 줄로 셀지
        // (`the_overview_counts_work_with_no_live_worktree` 가 지금 꼴로 가른다)부터 정할 자리라
        // 탐색기의 말을 옮기는 일(moai-ra67)과 함께 본다.
        Trouble::Unread { branch, why } => at_branch(branch, why),
        Trouble::Skipped { branch, path, lines } => fill(
            say(lang, "trouble.skipped"),
            &[("at", &at_branch(branch, &path.display().to_string())), ("n", &lines.to_string())],
        ),
        // **키는 줄마다 그대로 적는다** — 표와 소스를 견주는 시험(`i18n` 의 훑기)이 `say(lang, "…")`
        // 를 글자로 읽는다. 키를 값으로 고르면 그 키는 양쪽에서 함께 숨어 훑기에 구멍이 난다.
        Trouble::Unfound { lost, why } => {
            let said = match lost {
                Lost::NoGit => say(lang, "trouble.no_git"),
                Lost::Failed => say(lang, "trouble.failed"),
                Lost::Stream => say(lang, "trouble.stream"),
                Lost::Encoding => say(lang, "trouble.encoding"),
            };
            fill(said, &[("why", why)])
        }
    }
}

/// 스냅샷을 못 읽은 옆 워크트리 한 줄 — `moai status` 의 stderr 와 밖 한눈 보기가 **같은 글을
/// 쓴다**(moai-dpbi). 갈라 적던 판은 같은 일을 두 말로 댔다.
pub fn unread_worktree(lang: Lang, branch: &str, path: &std::path::Path) -> String {
    let at = at_branch(branch, &path.display().to_string());
    fill(say(lang, "overview.unread_snapshot"), &[("at", &at)])
}

/// `⎇ <가지>: <무엇>` — 옆 워크트리를 대는 자리의 한 모양.
fn at_branch(branch: &str, tail: &str) -> String {
    format!("{} {branch}: {tail}", style::BRANCH_GLYPH)
}

/// 명령 안내에 넣을 경로 — 붙여 넣으면 그 디렉터리로 풀리게 감싼다. `one_line` 을
/// 지나지 않는다: 화면용 접기가 탭·줄바꿈을 빈칸으로 바꾸면 없는 디렉터리를 가리킨다.
/// 한 줄 자리를 지키는 것은 `shell_word` 의 `$'…'` 다.
fn shell_arg(p: &std::path::Path) -> String {
    crate::text::shell_word(&p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    /// **고를 것이 있는 거절은 고를 것을 댄다**(moai-ivt9). 글이 말묶음으로 가면서 자료 쪽
    /// 시험은 갈래만 재게 됐는데, 그 갈래가 아는 목록을 안 달고 나가면 "그 값이 아니다" 만
    /// 듣고 무엇을 적어야 하는지는 설정 파일 어디에도 없다 — 옛 글이 목록을 달고 있던 까닭이다.
    #[test]
    fn config_trouble_names_what_it_needs() {
        use crate::config::Trouble;
        let said = |why: &Trouble| config_trouble(Lang::En, why);
        let naming = said(&Trouble::NamingUnknown { raw: "Full".into() });
        for want in crate::config::Naming::ALL {
            assert!(naming.contains(want), "{want} 가 빠졌다 — {naming}");
        }
        let threshold = said(&Trouble::NoSuchThreshold { line: 2, key: "status_reveiw_days".into() });
        for want in crate::config::Thresholds::KEYS {
            assert!(threshold.contains(want), "{want} 가 빠졌다 — {threshold}");
        }
        let done = said(&Trouble::NoDone { raw: "todo,review".into() });
        assert!(done.contains(crate::config::DONE), "{done}");
        // 자리를 다는 자는 하나다 — `Refused` 를 지나야 파일 이름이 앞에 선다.
        let at = std::path::PathBuf::from("/x/.moai/config.toml");
        let refused = config_refused(Lang::En, &crate::config::Refused { at, why: Trouble::NoPrefix });
        assert!(refused.starts_with("/x/.moai/config.toml: "), "{refused}");
        assert!(!said(&Trouble::NoPrefix).contains(".moai"), "글 쪽이 자리를 또 단다");
    }

    /// **태그 표기는 `tag_parts` 한 자리에서 정한다**(`tag_line` 은 그 조각을 잇는다). `add` 의 확인 줄과
    /// 연습이 손으로 지어, 표기를 바꾸면 방금 만든 줄의 태그만 옛 모양으로 보였다. 표면 코드에 같은 짓기가
    /// 다시 서면 여기서 이름을 대며 붉어진다 — 초안의 `\#` 풀기(`draft.rs`)는 표기가 아니라 글이다.
    /// 탐색기 상세가 태그마다 칠하려고 앞머리를 손으로 적었던 적이 있다(moai-xemz 리뷰) — 그 짓기는 옛
    /// 바늘에 안 걸려, 조각을 `view` 로 올리며 바늘도 조각의 표기로 옮기고 옛 바늘은 어디서도 안 서게 센다.
    ///
    /// **짓는 자리를 빼지 않고 센다** — `git.rs` 의 `git_is_spawned_only_through_command` 와 같은
    /// 모양이다. 이유가 둘이다. 하나, 파일 이름으로 빼면 `src/tui/view.rs` 까지 같이 빠진다
    /// (`Path::ends_with` 는 글자가 아니라 마디로 견준다) — 태그를 실제로 그리는 탐색기가 바로
    /// 거기라, 빼 두면 하필 제일 샐 만한 자리가 안 보인다. 둘, `tag_line` 이 다시 쓰여 바늘이
    /// 아무 데도 안 걸리는 날 "한 자리도 없음" 은 통과로 읽힌다 — **하나여야 한다**로 세면 그날
    /// 여기가 먼저 터져, 아무것도 안 지키는 시험이 초록으로 남지 않는다.
    #[test]
    fn tags_are_spelled_in_one_place() {
        // 바늘을 쪼개 둔다 — 한 조각이 통째로 적히면 이 줄이 제 바늘에 걸린다.
        let needle = concat!("if n == 0 { \"#\" }", " else { \" #\" }");
        let old = concat!("format!(\"#{t}\")", ").collect");
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src.clone()];
        let mut built = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
                let path = entry.expect("디렉터리 항목").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&path).expect("소스를 못 읽는다");
                    for (n, line) in text.lines().enumerate() {
                        if line.contains(needle) || line.contains(old) {
                            built.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        let here = src.join("view.rs").display().to_string();
        assert!(
            built.len() == 1 && built[0].starts_with(&here),
            "태그 표기를 정하는 자리는 view::tag_parts 하나다 — {built:#?}"
        );
        assert_eq!(tag_line(&["a".into(), "b".into()]), "#a #b");
        assert_eq!(tag_line(&[]), "");
    }

    /// 파일에서 읽은 것이 없는 상세의 맥락 — 겹침도, 미룸 뿌리도, 묶음의 칸도 묻지 않는다.
    /// [`Seen`] 은 `Default` 를 안 들므로(말을 물어야 선다) 시험이 이 자로 짓는다.
    fn bare_seen(lang: Lang) -> Seen<'static> {
        Seen {
            screen: Screen::new(lang),
            roots: BTreeMap::new(),
            states: BTreeMap::new(),
            blocks: Vec::new(),
            places: None,
        }
    }

    fn no_epics() -> crate::report::EpicLabels<'static> {
        BTreeMap::new()
    }

    /// **미룸 한 마디와 묶음 칸 한 줄도 고른 말로 선다**(moai-ra67). 목록은 이미 말묶음에서
    /// 오는데 이 둘만 한국어로 박혀 있어, 영어로 고른 화면이 목록에서는 `deferred` 라 하고
    /// 상세 머리 줄에서는 `미룸` 이라 했다 — 나란히 놓고 보는 사람이 어느 쪽을 믿을지 정하게
    /// 되는 자리다(리뷰 moai-hom6.soc 의 5·6번).
    #[test]
    fn the_deferred_word_and_the_unread_column_follow_the_chosen_language() {
        let mut i = issue("argos-0001", "일", "in_progress");
        i.deferred_at = Some("2026-09-08T04:12:03Z".into());
        let now = "2026-09-11T04:12:03Z";
        assert_eq!(deferred_for(&i, None, now, Lang::Ko).as_deref(), Some("미룸 (3일)"));
        assert_eq!(deferred_for(&i, None, now, Lang::En).as_deref(), Some("deferred (3d)"));
        // 나이를 못 재면 낱말 하나다.
        let mut fresh = i.clone();
        fresh.deferred_at = Some(now.into());
        assert_eq!(deferred_for(&fresh, None, now, Lang::En).as_deref(), Some("deferred"));
        // 물려받은 미룸은 뺀 줄을 댄다 — id 는 자료라 말이 바뀌어도 그대로다.
        let under = deferred_for(&i, Some("argos-0009"), now, Lang::En);
        assert_eq!(under.as_deref(), Some("deferred — under argos-0009"));
        assert_eq!(deferred_for(&i, Some("argos-0009"), now, Lang::Ko).as_deref(), Some("미룸 — argos-0009 밑"));

        // 손으로 적은 칸이 읽은 칸과 다를 때만 말한다 — 그 한 줄도 같은 말로 선다.
        let mut group = issue("argos-000e", "에픽", "done");
        group.kind = Kind::Epic;
        let said = unread_column(&group, "in_progress", &cfg(), Lang::En);
        assert_eq!(
            said.as_deref(),
            Some("the column is read from its members (the written column `done` is not read)")
        );
        assert!(unread_column(&group, "in_progress", &cfg(), Lang::Ko).is_some_and(|s| s.contains("적힌 칸 `done`")));
        // 서 있는 칸과 같으면 두 말 다 아무 말도 안 한다.
        for lang in [Lang::En, Lang::Ko] {
            assert_eq!(unread_column(&group, "done", &cfg(), lang), None, "{lang:?}");
        }
    }

    fn issue(id: &str, title: &str, status: &str) -> Issue {
        Issue::new(id.into(), title.into(), Kind::Issue, Status::new(status), "2026-09-11T04:12:03Z")
    }

    /// **경고 목록은 판정한 나이를 댄다**(moai-7azq). 칸에 30일 선 줄이 막음이 5일 전 다시
    /// 선 것으로 `blocked_stale` 에 걸리면 "5일" 이다 — 여기서 칸 나이를 새로 재면 "3일 넘게
    /// 막힘" 밑에 "30일" 이 선다.
    #[test]
    fn a_warning_row_shows_the_age_it_was_judged_by() {
        let now = "2026-10-11T00:00:00Z";
        let mut blocker = issue("argos-0001", "막는 일", "todo");
        blocker.status_since = "2026-10-06T00:00:00Z".into(); // 5일 전 done 에서 되돌아 나왔다
        let mut stuck = issue("argos-0002", "막힌 일", "todo"); // `issue` 은 09-11 — 칸에 30일
        stuck.blocked_by = vec!["argos-0001".into()];
        let issues = vec![blocker, stuck];
        let st = crate::report::status(&issues, &[], &cfg(), now, Lang::Ko);
        let w = st.warnings.iter().find(|w| w.kind == "blocked_stale").expect("막힘 경고가 없다");
        let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
        let out = plain(&preview(w, &by_id, now, Screen::new(Lang::Ko)));
        let row = out.iter().find(|l| l.contains("argos-0002")).expect("막힌 줄이 목록에 없다");
        assert!(row.contains(" 5일"), "판정한 나이를 안 댔다 — {row}");
        assert!(!row.contains("30일"), "칸 나이를 댔다 — {row}");
    }

    /// 미룬 것에 막힌 줄은 **막는 줄을 미룬 지 며칠**로 선다(moai-hcx3) — 칸 나이 "30일" 이
    /// 아니다.
    #[test]
    fn a_row_held_by_a_deferral_shows_how_long_ago_it_was_deferred() {
        let now = "2026-10-11T00:00:00Z";
        let mut shelved = issue("argos-0001", "미룬 일", "todo");
        shelved.deferred_at = Some("2026-09-29T00:00:00Z".into()); // 12일 전
        let mut held = issue("argos-0002", "막힌 일", "todo"); // `issue` 은 09-11 — 칸에 30일
        held.blocked_by = vec!["argos-0001".into()];
        let issues = vec![shelved, held];
        let st = crate::report::status(&issues, &[], &cfg(), now, Lang::Ko);
        let w = st.warnings.iter().find(|w| w.kind == "blocked_by_deferred").expect("미룬 것에 막힘 경고가 없다");
        let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
        let out = plain(&preview(w, &by_id, now, Screen::new(Lang::Ko)));
        let row = out.iter().find(|l| l.contains("argos-0002")).expect("막힌 줄이 목록에 없다");
        assert!(row.contains("12일"), "미룬 지 며칠을 안 댔다 — {row}");
        assert!(!row.contains("30일"), "칸 나이를 댔다 — {row}");
    }

    fn plain(lines: &[String]) -> Vec<String> {
        // 테스트는 칠하지 않은 모양을 본다 — 표 정렬은 색과 무관해야 한다.
        lines
            .iter()
            .map(|l| {
                let mut out = String::new();
                let mut esc = false;
                for c in l.chars() {
                    match (esc, c) {
                        (false, '\u{1b}') => esc = true,
                        (true, 'm') => esc = false,
                        (true, _) => {}
                        (false, _) => out.push(c),
                    }
                }
                out
            })
            .collect()
    }

    /// 한글 제목과 ASCII 제목이 같은 열에서 만난다.
    #[test]
    fn cjk_titles_line_up() {
        let issues = vec![issue("argos-0001", "한글 제목이다", "todo"), issue("argos-0002", "ascii title", "review")];
        let out = plain(&list(
            &issues,
            &cfg(),
            Hidden { done: 0, ..Hidden::default() },
            &no_epics(),
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        let cols: Vec<usize> = out[1..3].iter().map(|l| width(l.split_once("  ").unwrap().0)).collect();
        assert_eq!(cols[0], cols[1], "{out:#?}");
        // 제목 시작 열이 같다
        let starts: Vec<usize> = out[1..3].iter().map(|l| width(&l[..l.find(['한', 'a']).unwrap()])).collect();
        assert_eq!(starts[0], starts[1], "{out:#?}");
    }

    /// 헤더의 `제목` 과 줄의 제목이 같은 칸에서 시작한다.
    #[test]
    fn header_lines_up_with_rows() {
        let issues = vec![issue("argos-0001", "제목이다", "todo")];
        let out = plain(&list(
            &issues,
            &cfg(),
            Hidden { done: 0, ..Hidden::default() },
            &no_epics(),
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        let head_at = width(&out[0][..out[0].find("제목").unwrap()]);
        let row_at = width(&out[1][..out[1].find("제목이다").unwrap()]);
        assert_eq!(head_at, row_at, "{out:#?}");
    }

    #[test]
    fn empty_list_says_why() {
        assert_eq!(
            plain(&list(
                &[],
                &cfg(),
                Hidden { done: 0, ..Hidden::default() },
                &no_epics(),
                false,
                &Default::default(),
                Screen::new(Lang::Ko)
            ))[0],
            "없다."
        );
        assert!(
            plain(&list(
                &[],
                &cfg(),
                Hidden { done: 3, ..Hidden::default() },
                &no_epics(),
                false,
                &Default::default(),
                Screen::new(Lang::Ko)
            ))[0]
                .contains("done 3건")
        );
    }

    #[test]
    fn summary_counts_each_column() {
        let issues = vec![issue("argos-0001", "a", "todo"), issue("argos-0002", "b", "todo")];
        let out = plain(&list(
            &issues,
            &cfg(),
            Hidden { done: 5, ..Hidden::default() },
            &no_epics(),
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        let last = out.last().unwrap();
        assert!(last.starts_with("2건 (todo 2)"), "{last}");
        assert!(last.contains("done 5건 숨김"), "{last}");
    }

    #[test]
    fn long_titles_are_clipped_not_wrapped() {
        let long = "가".repeat(80);
        let out = plain(&list(
            &[issue("argos-0001", &long, "todo")],
            &cfg(),
            Hidden { done: 0, ..Hidden::default() },
            &no_epics(),
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        assert!(out[1].ends_with('…'), "{:?}", out[1]);
        assert!(width(&out[1]) < 80, "{:?}", out[1]);
    }

    /// 다른 브랜치에서 온 줄은 **색을 꺼도** `⎇ <브랜치>` 로 읽히고, 표는 머리표를
    /// 품은 채로 줄을 맞춘다. 지금 브랜치의 줄에는 아무것도 안 붙는다.
    #[test]
    fn a_line_from_another_branch_carries_its_mark_before_the_title() {
        let mine = vec![issue("argos-0001", "여기 일", "todo")];
        let theirs = vec![issue("argos-0002", "옆 일", "in_progress")];
        let (all, origin) = crate::worktree::overlay(mine, &[crate::worktree::Side::new("feat/x", "/wt", theirs)]);
        let tagged: crate::report::EpicLabels =
            all.iter().map(|i| ((i.id.as_str(), i.kind), "에픽".to_string())).collect();
        let out = plain(&list(
            &all,
            &cfg(),
            Hidden::default(),
            &tagged,
            false,
            &Default::default(),
            Screen::new(Lang::Ko).over(&origin),
        ));
        assert!(out[1].contains("여기 일") && !out[1].contains('⎇'), "{out:#?}");
        assert!(out[2].contains("⎇ feat/x 옆 일"), "{out:#?}");
        let col = |l: &str| width(&l[..l.find("에픽").unwrap()]);
        assert_eq!(col(&out[1]), col(&out[2]), "머리표가 다음 열을 밀었다\n{out:#?}");
        // 색을 켜면 머리표는 제 색을 입는다.
        let painted = list(
            &all,
            &cfg(),
            Hidden::default(),
            &tagged,
            false,
            &Default::default(),
            Screen::new(Lang::Ko).over(&origin),
        );
        assert!(painted[2].contains(&paint(style::BRANCH, "⎇ feat/x")), "{:?}", painted[2]);
    }

    /// 본문이 **그려진다.** 기호가 걷히고 목록은 글머리를 얻는다.
    #[test]
    fn the_body_is_drawn_not_echoed() {
        let out = plain(&body_lines("**굵게** 한 줄\n\n- 하나\n- 둘\n")).join("\n");
        assert!(!out.contains("**"), "굵게 기호가 남았다\n{out}");
        assert!(out.contains("굵게 한 줄"), "{out}");
        assert!(out.contains('•'), "목록 글머리가 없다\n{out}");
        assert!(!out.contains("- 하나"), "목록 기호가 남았다\n{out}");
    }

    /// **색을 꺼도 코드는 코드로 남는다.** 색으로만 표시하면 `0.2` 가 판인지
    /// 숫자인지 구별할 길이 사라진다 — 이 저장소의 규칙에 걸린다.
    #[test]
    fn code_keeps_a_mark_that_survives_without_colour() {
        let out = plain(&body_lines("판은 `0.2` 다\n")).join("\n");
        assert!(out.contains("`0.2`"), "색을 끄니 코드가 그냥 글이 됐다\n{out}");
    }

    /// 그린 줄은 폭을 넘지 않는다 — **폭을 넘기는 인라인 코드 한 덩이만 빼고**
    /// (moai-krh7). 한글이 두 칸이라 글자 수로 세면 걸린다.
    ///
    /// 상한은 `BODY` 에 들여쓰기(`PAD`)를 더한 값이다. 여유를 더 주면 그만큼
    /// 넘치는 줄을 통과시킨다. 셸은 넘긴 줄을 화면에서만 접어 복사하면 온전하니
    /// 코드는 끊지 않는데(`Overflow::Keep`), **넘길 수 있는 것이 그것뿐이라는
    /// 것까지 여기서 잰다** — 안 그러면 산문이 넘쳐도 이 시험이 지나간다.
    #[test]
    fn drawn_lines_stay_within_the_width() {
        let long = "moai add \"아주 긴 제목을 가진 이슈\" -t bug -e moai-4aex --milestone v0.1 -b -";
        let body = format!(
            "아주 긴 한글 문장이 폭을 넘도록 이어지고 또 이어지고 계속 이어진다. \
             여기에 `코드` 와 **굵게** 도 섞여 있어서 접는 자리가 조각 가운데에 걸린다. \
             폭을 넘기는 것은 `{long}` 한 덩이뿐이다.\n"
        );
        let max = BODY + width(PAD);
        let drawn = plain(&body_lines(&body));
        // **넘기는 줄이 실제로 나야 아래 고리가 뜻이 있다.** 코드를 끊기 시작하면
        // 넘는 줄이 하나도 없어져 이 시험이 빈 채로 지나간다.
        assert!(drawn.iter().any(|l| l.contains(long)), "긴 명령이 갈렸다 — {drawn:?}");
        for l in &drawn {
            if width(l) <= max {
                continue;
            }
            assert!(l.contains(long), "코드도 아닌 줄이 폭을 넘었다 — {l:?} ({}칸)", width(l));
        }
    }

    /// **`--raw` 도 제어문자를 걸러 낸다.** 그리는 길만 걸러 두면 파일에 심긴
    /// ESC 한 줄이 `--raw` 를 타고 화면에 닿아 커서를 옮기고 화면을 지운다 —
    /// 탐색기의 원문 보기는 이미 걸러므로, 안 걸러면 두 표면이 갈라진다.
    #[test]
    fn the_raw_body_cannot_repaint_the_screen() {
        let mut i = issue("argos-0001", "제목", "todo");
        i.body = Some("앞\u{1b}[2J\u{7}뒤".into());
        for raw in [true, false] {
            let out =
                plain(&detail(&i, None, &[], &bare_seen(Lang::Ko), &cfg(), "2026-09-11T04:12:03Z", raw)).join("\n");
            assert!(!out.contains('\u{1b}'), "ESC 가 화면에 닿았다 (raw={raw})\n{out:?}");
            assert!(!out.contains('\u{7}'), "벨이 화면에 닿았다 (raw={raw})\n{out:?}");
        }
    }

    #[test]
    fn detail_shows_body_and_history() {
        let mut i = issue("argos-0001", "제목", "in_progress");
        i.body = Some("첫 줄\n둘째 줄".into());
        let j = vec![
            JournalEntry::create("argos-0001", "제목", "2026-09-09T14:02:11Z", &crate::model::someone("raven")),
            JournalEntry::status(
                "argos-0001",
                &Status::new("todo"),
                &Status::new("in_progress"),
                None,
                "2026-09-10T10:11:00Z",
                &crate::model::someone("claude"),
            ),
        ];
        // 이력은 부르는 쪽이 붙인다 — `moai show` 가 묶음의 멤버를 그 앞에 끼운다.
        let mut lines = detail(&i, None, &[], &bare_seen(Lang::Ko), &cfg(), "2026-09-11T04:12:03Z", false);
        lines.extend(history(&j, &cfg(), Lang::Ko));
        let out = plain(&lines);
        let joined = out.join("\n");
        assert!(joined.contains("첫 줄") && joined.contains("둘째 줄"), "{joined}");
        assert!(joined.contains("이력"), "{joined}");
        assert!(joined.contains("todo → in_progress"), "{joined}");
        assert!(joined.contains("09-09 14:02"), "{joined}");
    }

    /// **상세의 왼쪽 이름 칸은 어느 말에서도 한 폭이다**(`label_width`).
    ///
    /// 한국어는 `에픽`·`자식`·`생성`·`막힘` 이 모두 두 글자라 이 자가 없어도 섰고, 영어는
    /// `Created`(7)와 `Members`(7)가 마침 같아 폭을 두 자로 나눠 재도 안 드러났다. 실제로
    /// 어긋난 것은 막음 줄이다 — `held`(4)·`freed`(5)·`broken`(6)이 제각각 서서 글리프와 id
    /// 칸이 줄마다 옮겨 갔다. **그러니 우연이 아니라 자로 잰다**: 값이 시작하는 칸이 줄마다
    /// 같은지를 다섯 말 모두에서 본다.
    #[test]
    fn the_left_name_column_stands_in_every_language() {
        use crate::report::{Block, Blocker};
        // `  이름   값` 에서 값이 시작하는 칸. 이름 뒤 빈칸 묶음이 끝나는 자리다.
        let value_col = |line: &str| {
            let body = line.trim_start();
            let gap = body.find("   ").unwrap_or_else(|| panic!("이름 칸이 없다 — {line:?}"));
            width(line) - width(body[gap..].trim_start())
        };
        let blocker = issue("argos-0009", "막는 일", "todo");
        let mut i = issue("argos-0002", "멤버", "in_progress");
        i.epic = Some("argos-0001".into());
        i.started_at = Some("2026-09-10T10:11:00Z".into());
        let epic = issue("argos-0001", "에픽", "in_progress");
        let child = issue("argos-0002.a1b", "자식", "todo");
        for lang in Lang::ALL {
            let mut seen = bare_seen(lang);
            seen.blocks = vec![
                Block {
                    id: "argos-0009",
                    issue: Some(&blocker),
                    blocker: Blocker::Open,
                    root: None,
                    aside: Vec::new(),
                },
                Block { id: "argos-0008", issue: Some(&epic), blocker: Blocker::Done, root: None, aside: Vec::new() },
                Block { id: "argos-0007", issue: None, blocker: Blocker::Missing, root: None, aside: Vec::new() },
            ];
            let mut lines = detail(&i, Some(&epic), &[&child], &seen, &cfg(), "2026-09-11T04:12:03Z", false);
            // 멤버 줄은 `cmd::show` 가 굴림을 들고 세운다 — 같은 자를 쓰는지 여기서 함께 본다.
            lines.push(format!("  {}   1/2", members_label(lang)));
            // 이름 칸을 가진 줄만 — 머리 두 줄(제목·칸)은 이름 칸이 없다.
            let cols: Vec<(usize, String)> = plain(&lines)
                .into_iter()
                .filter(|l| l.starts_with("  ") && l.trim_start().contains("   "))
                .map(|l| (value_col(&l), l))
                .collect();
            assert!(cols.len() >= 6, "{}: 잴 줄이 모자라다 — {cols:#?}", lang.code());
            let first = cols[0].0;
            assert!(cols.iter().all(|(c, _)| *c == first), "{}: 왼쪽 이름 칸이 줄마다 갈렸다 — {cols:#?}", lang.code());
        }
    }

    /// 목록의 에픽 열은 id 가 아니라 제목이다. id 를 보여 주면 사람이
    /// 그걸 다시 찾아봐야 한다.
    #[test]
    fn the_epic_column_shows_a_title() {
        let mut i = issue("argos-0002", "멤버", "todo");
        i.epic = Some("argos-0001".into());
        let labels = BTreeMap::from([(("argos-0002", Kind::Issue), "저장 계층".to_string())]);
        let out = plain(&list(
            &[i.clone()],
            &cfg(),
            Hidden::default(),
            &labels,
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        assert!(out[1].contains("저장 계층") && !out[1].contains("argos-0001"), "{out:#?}");

        // 없는 에픽을 가리켜도 죽지 않고 그렇다고 말한다
        let dangling = BTreeMap::from([(("argos-0002", Kind::Issue), "(없는 에픽)".to_string())]);
        let out =
            plain(&list(&[i], &cfg(), Hidden::default(), &dangling, false, &Default::default(), Screen::new(Lang::Ko)));
        assert!(out[1].contains("(없는 에픽)"), "{out:#?}");
    }

    /// 멤버 없는 에픽은 0% 가 아니라 막대 없음이다 — "아직 안 한 것" 과
    /// "속을 안 채운 것" 은 다르다.
    #[test]
    fn an_empty_bar_is_not_zero_percent() {
        assert!(!plain(&[bar(None)])[0].contains('█'));
        assert_eq!(plain(&[bar(Some(0))])[0].matches('█').count(), 0);
        assert_eq!(plain(&[bar(Some(100))])[0].matches('█').count(), BAR);
        // 1% 도 한 칸은 찬다 — 시작한 것이 안 시작한 것처럼 보이면 안 된다
        assert_eq!(plain(&[bar(Some(1))])[0].matches('█').count(), 1);
    }

    /// **묶음 색은 묶음만 입는다.** idea 도 이슈가 아니지만 아무것도 담지
    /// 않으므로, `kind != Issue` 로 칠하면 목록에서 에픽처럼 보인다 —
    /// `moai idea add` 가 만드는 순간에는 안 그런데 `moai show` 에서만
    /// 그러면 같은 줄이 두 색이다. 저절로 되돌아갈 자리라 못 박는다.
    #[test]
    fn only_a_grouping_wears_the_grouping_colour() {
        let work = issue("argos-0009", "진짜 일", "todo");
        let mut thought = issue("argos-0001", "반짝", "todo");
        thought.kind = Kind::Idea;
        let mut epic = issue("argos-0002", "저장 계층", "todo");
        epic.kind = Kind::Epic;
        let mut stone = issue("argos-0003", "v0.1", "todo");
        stone.kind = Kind::Milestone;

        assert_eq!(title_style(&thought), title_style(&work), "idea 가 묶음 색을 입었다");
        assert_eq!(title_style(&epic), style::EPIC);
        assert_eq!(title_style(&stone), style::EPIC);
    }

    /// **안 물었는데 표가 사라지지 않는다.** 표를 달지 말지를 결과의 내용으로
    /// 정하면(`전부 미룬 것인가`) `--all` 이 마침 미룬 것만 냈을 때 계획 밖의
    /// 줄이 일과 똑같이 보인다. 물어서 붙는 군더더기보다 안 물었는데 사라지는
    /// 것이 나쁘다 — 가르는 것은 부르는 쪽의 물음이다.
    #[test]
    fn a_list_of_only_deferred_rows_still_marks_them() {
        let mut a = issue("argos-0001", "하나", "todo");
        a.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let mut b = issue("argos-0002", "둘", "todo");
        b.deferred_at = Some("2026-09-01T00:00:00Z".into());
        let all = vec![a, b];

        let wide = plain(&list(
            &all,
            &cfg(),
            Hidden::default(),
            &no_epics(),
            false,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        assert!(wide[1].contains("미룸"), "미룬 줄이 일과 똑같이 보인다 — {wide:#?}");
        assert!(wide[2].contains("미룸"), "{wide:#?}");

        // 콕 집어 물었을 때는 줄마다 같은 낱말을 달지 않는다.
        let asked = plain(&list(
            &all,
            &cfg(),
            Hidden::default(),
            &no_epics(),
            true,
            &Default::default(),
            Screen::new(Lang::Ko),
        ));
        assert!(!asked[1].contains("미룸"), "물어서 낸 목록에 군더더기가 붙었다 — {asked:#?}");
    }

    #[test]
    fn tree_nests_members_then_children() {
        let mut epic = issue("argos-0001", "저장 계층", "in_progress");
        epic.kind = Kind::Epic;
        let mut member = issue("argos-0002", "원자적 쓰기", "todo");
        member.epic = Some("argos-0001".into());
        let child = issue("argos-0002.aaa", "회귀 테스트", "todo");
        let loose = issue("argos-0009", "떠 있는 것", "todo");

        let all = vec![epic, member.clone(), child.clone(), loose.clone()];
        let rolls = crate::report::rollup(&all, &cfg());
        let index = crate::nav::Index::of(&all);
        // 걸러진 것만 보여 준다 — 에픽 줄 자체는 걸러 놓고 그 밑을 본다.
        let shown: std::collections::BTreeSet<&str> =
            [member.id.as_str(), child.id.as_str(), loose.id.as_str()].into_iter().collect();
        let keep = |at: usize| shown.contains(all[at].id.as_str());
        let out = plain(&tree(&all, &index, &keep, &rolls, Screen::new(Lang::Ko)).0);
        let joined = out.join("\n");

        assert!(joined.contains("저장 계층"), "{joined}");
        let at_member = out.iter().position(|l| l.contains("원자적 쓰기")).unwrap();
        let at_child = out.iter().position(|l| l.contains("회귀 테스트")).unwrap();
        assert!(at_child == at_member + 1, "자식이 부모 바로 밑이 아니다\n{joined}");
        // 자식이 더 깊게 들어간다
        let indent = |l: &str| l.len() - l.trim_start().len();
        assert!(indent(&out[at_child]) > indent(&out[at_member]), "{joined}");
        assert!(joined.contains("에픽 없음"), "{joined}");
    }

    /// **소속 없는 것은 제 머리글을 갖는다.** 마일스톤 밑에서도 그렇다 —
    /// 앞선 에픽 밑에 그대로 붙으면 그 에픽의 멤버로 읽힌다.
    #[test]
    fn loose_issues_never_hide_under_the_previous_epic() {
        let mut milestone = issue("argos-m001", "v0.1", "todo");
        milestone.kind = Kind::Milestone;
        let mut epic = issue("argos-e001", "에픽", "todo");
        epic.kind = Kind::Epic;
        epic.milestone = Some("argos-m001".into());
        let mut member = issue("argos-0020", "에픽 멤버", "todo");
        member.epic = Some("argos-e001".into());
        let mut parent = issue("argos-0030", "소속 없는 부모", "todo");
        parent.milestone = Some("argos-m001".into());
        let mut child = issue("argos-0030.aa1", "그 자식", "todo");
        child.milestone = Some("argos-m001".into());

        let all = vec![milestone, epic, member, parent, child];
        let rolls = crate::report::rollup(&all, &cfg());
        let index = crate::nav::Index::of(&all);
        let out = plain(&tree(&all, &index, &|_| true, &rolls, Screen::new(Lang::Ko)).0);
        let joined = out.join("\n");

        let at_epic = out.iter().position(|l| l.contains("에픽 멤버")).unwrap();
        let at_loose = out.iter().position(|l| l.contains("소속 없는 부모")).unwrap();
        let header = out[at_epic..at_loose].iter().any(|l| l.contains("에픽 없음"));
        assert!(header, "소속 없는 것이 에픽 머리글 밑에 그대로 붙었다\n{joined}");

        // 들여쓰기도 같아야 한다. 머리글이 안 들여쓰였는데 그 밑만 더
        // 들어가면, 앞선 에픽의 멤버보다 한 칸 깊어 남의 손자로 읽힌다.
        let pad = |l: &str| l.len() - l.trim_start().len();
        assert_eq!(pad(&out[at_loose]), pad(&out[at_epic]), "들여쓰기가 어긋났다\n{joined}");
    }

    /// **`(길 잃음)` 안에서도 남의 머리글 밑에 붙지 않는다** (moai-44k8). 마일스톤이
    /// 끊긴 에픽은 바구니 안에서 머리글(0/0)을 달고, 에픽이 끊긴 이슈는 같은 바구니의
    /// 잎이다. 받은 차례대로 놓으면 잎이 그 에픽 머리글 바로 밑에 한 칸 들여쓰여 멤버로
    /// 읽혔다 — 뿌리·마일스톤에서 소속 없는 줄을 머리글로 가른 것과 같은 자리다. 그
    /// 에픽의 진짜 멤버는 여전히 그 밑에 선다.
    #[test]
    fn a_lost_leaf_is_not_drawn_under_a_lost_epic() {
        let mut milestone = issue("argos-m001", "v0.1", "todo");
        milestone.kind = Kind::Milestone;
        let mut epic = issue("argos-e001", "잃은 에픽", "todo");
        epic.kind = Kind::Epic;
        epic.milestone = Some("argos-zzzz".into());
        let mut stray = issue("argos-0001", "끊긴 이슈", "todo");
        stray.epic = Some("argos-zzzz".into());
        let mut member = issue("argos-0002", "잃은 에픽의 멤버", "todo");
        member.epic = Some("argos-e001".into());

        let all = vec![milestone, epic, stray, member];
        let rolls = crate::report::rollup(&all, &cfg());
        let index = crate::nav::Index::of(&all);
        let out = plain(&tree(&all, &index, &|_| true, &rolls, Screen::new(Lang::Ko)).0);
        let joined = out.join("\n");
        let at =
            |needle: &str| out.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle}\n{joined}"));

        let (bucket, head, stray, member) = (at("(길 잃음)"), at("argos-e001"), at("끊긴 이슈"), at("멤버"));
        assert!(bucket < stray && stray < head, "끊긴 이슈가 잃은 에픽 머리글 밑에 섰다\n{joined}");
        assert!(head < member, "잃은 에픽의 멤버가 제 머리글 밑을 떠났다\n{joined}");
    }

    /// **같은 id 의 에픽 줄 둘이어도 멤버는 한 번, 두 줄은 다 보인다.** 폴더는
    /// id 가 가리키는 뒷줄이고(`nav::Index::is_dir`), 앞줄은 잎으로 선다. 머리글이
    /// 제 이름을 집계에서 id 로 찾으면 두 줄이 같은 제목을 달고 뒷줄은 트리
    /// 어디에도 안 나온다 — 깨진 자료를 숨기는 셈이다(moai-sfml). 집계의 차례가
    /// 어느 줄을 먼저 두든 같아야 하므로 두 차례 모두 본다.
    #[test]
    fn a_duplicate_epic_id_draws_its_members_once_and_both_lines() {
        for (front, back) in [("가 앞줄", "나 뒷줄"), ("나 앞줄", "가 뒷줄")] {
            let mut milestone = issue("argos-m001", "v0.1", "todo");
            milestone.kind = Kind::Milestone;
            let mut first = issue("argos-e001", front, "todo");
            first.kind = Kind::Epic;
            first.milestone = Some("argos-m001".into());
            let mut second = issue("argos-e001", back, "todo");
            second.kind = Kind::Epic;
            let mut member = issue("argos-0020", "멤버", "todo");
            member.epic = Some("argos-e001".into());

            let all = vec![milestone, first, second, member];
            let rolls = crate::report::rollup(&all, &cfg());
            let index = crate::nav::Index::of(&all);
            let out = plain(&tree(&all, &index, &|_| true, &rolls, Screen::new(Lang::Ko)).0);
            let joined = out.join("\n");
            let count = |needle: &str| out.iter().filter(|l| l.contains(needle)).count();
            assert_eq!(count("멤버"), 1, "멤버가 두 번 그려졌다\n{joined}");
            assert_eq!(count(front), 1, "앞줄이 사라졌거나 겹쳤다\n{joined}");
            assert_eq!(count(back), 1, "뒷줄이 사라졌거나 겹쳤다\n{joined}");
            let at_member = out.iter().position(|l| l.contains("멤버")).unwrap();
            assert!(out[at_member - 1].contains(back), "멤버가 폴더인 뒷줄 밑이 아니다\n{joined}");
        }
    }

    /// 걸러진 뒤 멤버가 하나도 안 남은 에픽은 빼되, 자기 자신이 걸렸으면 남긴다.
    #[test]
    fn tree_drops_epics_with_nothing_to_show() {
        let mut epic = issue("argos-0001", "빈 에픽", "todo");
        epic.kind = Kind::Epic;
        let all = vec![epic.clone()];
        let rolls = crate::report::rollup(&all, &cfg());

        let index = crate::nav::Index::of(&all);
        assert_eq!(plain(&tree(&all, &index, &|_| false, &rolls, Screen::new(Lang::Ko)).0), ["없다."]);
        assert!(plain(&tree(&all, &index, &|_| true, &rolls, Screen::new(Lang::Ko)).0).join("\n").contains("빈 에픽"));
    }

    /// **다 끝난 묶음은 재촉하지 않고, 접은 묶음은 그렇다고 말한다.** 묶음의 칸은
    /// 멤버에서 읽으므로 100% 면 곧 닫힌 것이라 시킬 말이 없다 — 적힌 칸을 옮기라고
    /// 시키면 어디서도 안 읽히는 칸을 쓰게 한다. 남은 멤버를 미뤄 접은 묶음은 막대가
    /// `1/2` 인 채로 닫혀 있어, 말하지 않으면 굴러가는 묶음과 똑같아 보인다.
    #[test]
    fn a_finished_grouping_is_not_nagged_but_a_folded_one_says_so() {
        let table = |all: &[Issue]| {
            let cfg = cfg();
            let st = crate::report::status(all, &[], &cfg, "2026-09-11T04:12:03Z", Lang::Ko);
            plain(&status(&st, all, &cfg, "2026-09-11T04:12:03Z", ".moai/issues.jsonl", 0, Screen::new(Lang::Ko)))
                .join("\n")
        };
        let mut epic = issue("argos-0001", "다 끝난 에픽", "todo");
        epic.kind = Kind::Epic;
        let mut member = issue("argos-0002", "멤버", "done");
        member.epic = Some("argos-0001".into());
        let mut all = vec![epic, member];

        let text = table(&all);
        assert!(!text.contains("닫을 때가 됐다") && !text.contains("안 닫힌"), "{text}");
        assert!(!text.contains("닫힘"), "다 끝난 줄에 군말을 달았다 — {text}");

        // 남은 멤버 하나를 미루면 그 묶음은 `1/2` 인 채로 닫힌다.
        let mut rest = issue("argos-0003", "남은 멤버", "todo");
        rest.epic = Some("argos-0001".into());
        rest.deferred_at = Some("2026-09-10T00:00:00Z".into());
        all.push(rest);
        let text = table(&all);
        assert!(text.contains("1/2") && text.contains("닫힘"), "접은 묶음을 안 비춘다 — {text}");
    }

    #[test]
    fn ready_names_where_each_pick_belongs() {
        let mut a = issue("argos-0002", "멤버", "todo");
        a.epic = Some("argos-0001".into());
        let b = issue("argos-0003", "떠 있는 것", "todo");
        let wip = issue("argos-0004", "잡고 있는 것", "in_progress");
        let labels = BTreeMap::from([(("argos-0002", Kind::Issue), "저장 계층".to_string())]);

        let lang = Lang::Ko;
        let none = crate::report::Focus::default();
        let out = plain(&ready(&[&a, &b], &labels, &[&wip], &[], &none, Screen::new(lang)));
        let joined = out.join("\n");
        // **글자는 말묶음에서 온다**(moai-zeyv) — 여기에 한국어를 박으면 기본 언어(영어)에서 깨진다.
        // **셈이 화면에 닿는지는 따로 잰다**(리뷰 moai-80qw) — 기댓값도 같은 `say` 를 지나므로,
        // 머리 글이 `{n}` 을 잃으면 양쪽이 나란히 잃어 이 줄만으로는 아무것도 안 잡힌다.
        assert!(joined.contains(&fill(say(lang, "ready.count"), &[("n", "2")])), "{joined}");
        assert!(out[0].contains('2'), "머리 줄에 셈이 없다 — {:?}", out[0]);
        assert!(joined.contains("저장 계층") && joined.contains("에픽 없음"), "{joined}");
        assert!(joined.contains("이미 잡고 있는 것 1건"), "{joined}");
        assert!(joined.contains("argos-0004"), "{joined}");

        let empty = plain(&ready(&[], &labels, &[], &[], &none, Screen::new(lang)));
        let joined = empty.join("\n");
        assert!(
            joined.contains(&fill(say(lang, "ready.count"), &[("n", "0")])) && joined.contains(say(lang, "ready.none")),
            "{joined}"
        );
        assert!(empty[0].contains('0'), "빈 머리 줄에 셈이 없다 — {:?}", empty[0]);
    }

    /// **그리는 쪽은 말을 인자로 받는다**(moai-cigu) — 제 손으로 설정을 다시 읽지 않는다.
    ///
    /// 그래서 이 시험은 돌리는 사람의 `~/.config/moai/config.toml` 과 상관이 없다. 전역에
    /// 박아 두던 때는 한 프로세스에서 여러 갈래로 도는 시험 가운데 먼저 박은 쪽이 이겨,
    /// 말을 고르는 시험을 아예 못 썼다.
    ///
    /// **경고 글까지 말묶음에서 온다**(moai-7cyf) — 머리 두 줄만 옮겨 두면 다른 말로 고른
    /// 사람의 화면이 제 말 두 줄과 한국어 열 몇 줄로 섞인다.
    #[test]
    fn the_screen_speaks_the_language_it_is_handed() {
        let issues = vec![issue("argos-0001", "첫 일", "todo")];
        let cfg = cfg();
        let now = "2026-09-11T04:12:03Z";
        let st = crate::report::status(&issues, &[], &cfg, now, Lang::Ko);
        let draw =
            |lang| plain(&status(&st, &issues, &cfg, now, ".moai/issues.jsonl", 0, Screen::new(lang))).join("\n");
        let (ko, en) = (draw(Lang::Ko), draw(Lang::En));
        assert_ne!(ko, en, "두 말이 같은 화면을 냈다 — 말이 화면에 안 닿는다");
        for (lang, screen) in [(Lang::Ko, &ko), (Lang::En, &en)] {
            // 흐름 줄과 닫는 줄 — 이 화면의 머리가 아니라 **꼬리**다.
            assert!(screen.contains(say(lang, "status.next")), "{}: 닫는 줄이 제 말이 아니다\n{screen}", lang.code());
            assert!(
                screen.contains(&fill(say(lang, "status.piling"), &[("n", "+1")])),
                "{}: 흐름 줄\n{screen}",
                lang.code()
            );
            // 경고 글. 에픽 없는 이슈 하나뿐이라 `no_epic` 이 선다.
            let warned = fill(say(lang, "warn.no_epic"), &[("n", "1"), ("percent", "100")]);
            assert!(screen.contains(&warned), "{}: 경고가 제 말이 아니다\n{screen}", lang.code());
        }
    }

    /// **안 겹친 화면과 빈 출처를 겹친 화면은 같은 답을 낸다**(moai-4xib). [`Screen::origin`] 의
    /// `None` 이 빈 [`Origin`] 을 빌려 주던 자리와 뜻이 같다는 것이 그 값의 약속이고, 훅의 보드가
    /// 그 약속 위에서 빈 `Origin` 을 안 짓는다(`cmd::hook`). 약속이 글로만 있으면 [`Origin`] 에
    /// `None` 과 빈 값이 갈리는 물음이 하나라도 생기는 날 훅의 보드만 조용히 달라진다.
    ///
    /// **겹친 것이 있으면 머리와 줄이 그렇다고 말한다** — [`Screen::over`] 가 출처를 실제로 들고
    /// 내려가는지는 여기서 잰다. 인자를 묶는 리팩터에서 빈 화면만 그려 보면, 출처를 잃은 화면도
    /// 시험이 다 파랗다.
    ///
    /// **줄은 줄에서 잰다**(리뷰) — [`overlaid`] 가 머리 꼬리에 [`style::BRANCH_GLYPH`] 를 이미
    /// 넣으므로, 화면 전체에서 그 글자를 찾는 자는 줄이 표를 잃어도 머리 하나로 초록이 된다.
    /// 그러면 이 시험이 잡겠다고 적어 둔 바로 그 되돌림(줄의 `screen.branch` 가 사라지는 것)을
    /// 못 잡는다.
    #[test]
    fn an_empty_origin_draws_like_none_and_a_real_one_marks_the_rows() {
        let cfg = cfg();
        // **`now` 는 두 줄의 시각보다 뒤다**(리뷰) — `issue` 는 09-11 을, 옆에서 집은 줄은 09-12
        // 를 적는다. `now` 를 09-11 에 두면 옆 줄이 `report::FUTURE_SLACK_SECS`(24시간)를 네 시간
        // 남기고 스치는데, 그 상한을 스치는 날 `future_timestamp` 경고가 끼어든다 — 그쪽
        // 미리보기는 id 만 내고 `⎇` 를 안 달아, 아래 줄 판정이 조용히 다른 줄을 보게 된다.
        let now = "2026-09-13T00:00:00Z";
        let lang = Lang::Ko;
        let mine = vec![issue("argos-0001", "제 줄", "todo")];
        let draw = |issues: &[Issue], screen: Screen| {
            let st = crate::report::status(issues, &[], &cfg, now, Lang::Ko);
            plain(&status(&st, issues, &cfg, now, ".moai/issues.jsonl", 0, screen))
        };
        let bare = Origin::default();
        assert_eq!(
            draw(&mine, Screen::new(lang)),
            draw(&mine, Screen::new(lang).over(&bare)),
            "빈 출처를 겹친 화면이 안 겹친 화면과 다르다"
        );

        // 옆 워크트리가 같은 줄을 나중에 집었다 — 겹치면 그 줄이 이기고 출처가 선다.
        let mut theirs = issue("argos-0001", "옆에서 집은 줄", "in_progress");
        theirs.status_since = "2026-09-12T00:00:00Z".into();
        theirs.updated_at = "2026-09-12T00:00:00Z".into();
        let side = crate::worktree::Side::new("feat/x", "/tmp/feat-x", vec![theirs]);
        let (shown, origin) = crate::worktree::overlay(mine.clone(), &[side]);
        let over = draw(&shown, Screen::new(lang).over(&origin));
        let head = over.first().map(String::as_str).unwrap_or_default();
        assert!(
            head.contains(&fill(say(lang, "status.overlaid"), &[("trees", "feat/x")])),
            "머리가 겹쳐 봤다고 안 한다\n{over:#?}"
        );
        // 머리는 id 를 안 대므로 id 로 고른 줄은 머리가 아니다 — 그 줄에서 머리표를 찾는다.
        let mark = format!("{} feat/x", style::BRANCH_GLYPH);
        let row = over
            .iter()
            .find(|l| l.contains("argos-0001"))
            .unwrap_or_else(|| panic!("겹쳐 온 줄이 화면에 없다\n{over:#?}"));
        assert!(row.contains(&mark), "겹쳐 온 줄에 가지 표가 없다\n{over:#?}");
        let off = draw(&shown, Screen::new(lang));
        assert!(!off.iter().any(|l| l.contains(style::BRANCH_GLYPH)), "안 겹친 화면에 가지 표가 섰다\n{off:#?}");
    }

    /// 없는 에픽을 가리켜도 상세가 죽지 않는다 — 드러내되 막지 않는다.
    #[test]
    fn a_dangling_epic_is_shown_not_fatal() {
        let mut i = issue("argos-0001", "제목", "todo");
        i.epic = Some("argos-0000".into());
        let out = plain(&detail(&i, None, &[], &bare_seen(Lang::Ko), &cfg(), "2026-09-11T04:12:03Z", false));
        assert!(out.iter().any(|l| l.contains("(없는 에픽)")), "{out:#?}");
    }

    /// **읽음 표의 글은 말마다 그 파일과 그 줄을 댄다**(moai-rtji 리뷰). 글은 말묶음의 자리(`{key}`·`{id}`·
    /// `{root}`·`{said}`)를 `fill` 에 준 이름으로 채우는데, 두 이름이 갈리면 `{key}` 가 글자 그대로 서고 어느
    /// 줄인지가 사라진다. 말묶음 시험은 번역을 영어와만 견주므로 영어와 한국어가 함께 갈리면 못 잡고, 읽음
    /// 모듈의 시험은 자료만 견준다 — 그래서 갈래마다 **모든 말로** 펴 보고 채울 자리가 남았는지 잰다.
    ///
    /// **남의 것이라는 글은 "아니다" 로 선다** — `{root}` 는 우리 뿌리다. 영어 글이 "이것은 {root} 의
    /// 읽음이다, 우리 것이 아니다" 로 뒤집혀 섰던 적이 있다(리뷰).
    #[test]
    fn the_read_sheet_texts_name_the_line_in_every_language() {
        use crate::read_marks::{SheetRefusal, SheetTrouble, Skipped};
        let at = std::path::PathBuf::from("/홈/설정/read/0123456789abcdef.toml");
        let root = std::path::PathBuf::from("/w/proj");
        let stamp = Skipped::NotAStamp { id: "argos-0002".into(), found: "integer".into() };
        let table = Skipped::NotATable { found: "integer".into() };
        let not_ours = SheetTrouble::NotOurs { at: at.clone(), root: root.clone() };
        for lang in Lang::ALL {
            let said = [
                (sheet_trouble(lang, &SheetTrouble::Said { at: at.clone(), said: "EACCES".into() }), "EACCES"),
                (sheet_trouble(lang, &SheetTrouble::Unsettled { at: at.clone(), said: "ELOOP".into() }), "ELOOP"),
                (sheet_trouble(lang, &not_ours), "/w/proj"),
                (sheet_trouble(lang, &SheetTrouble::Skipped { at: at.clone(), why: stamp.clone() }), "read.argos-0002"),
                (sheet_trouble(lang, &SheetTrouble::Skipped { at: at.clone(), why: table.clone() }), "[read]"),
                (sheet_trouble(lang, &SheetTrouble::SpoolLeft { at: at.clone(), said: "EROFS".into() }), "EROFS"),
                // 막힌 id 가 없는 판은 **그 글 자체**를 바늘로 든다 — 빈 바늘은 무엇이 서도 맞아, 갈래를 가르는
                // 조건이 뒤틀려 없는 줄을 고치라는 글이 서도 이 시험이 푸르렀다.
                (
                    sheet_trouble(lang, &SheetTrouble::SpoolKept { at: at.clone(), held: Vec::new() }),
                    say(lang, "sheet.spool_kept"),
                ),
                (
                    sheet_trouble(lang, &SheetTrouble::SpoolKept { at: at.clone(), held: vec!["argos-0002".into()] }),
                    "argos-0002",
                ),
                (sheet_refusal(lang, &at, &SheetRefusal::Unparsable { said: "TOML".into() }), "TOML"),
                (sheet_refusal(lang, &at, &SheetRefusal::NotOurs { root: root.clone() }), "/w/proj"),
                (sheet_refusal(lang, &at, &SheetRefusal::Hand(stamp.clone())), "argos-0002"),
                (sheet_refusal(lang, &at, &SheetRefusal::Hand(table.clone())), "[read]"),
            ];
            for (said, names) in &said {
                let code = lang.code();
                assert!(said.starts_with(&format!("{}: ", at.display())), "{code}: 어느 파일인지를 안 댔다 — {said}");
                assert!(said.contains(names), "{code}: `{names}` 를 안 댔다 — {said}");
                assert!(!said.contains('{'), "{code}: 채울 자리가 남았다 — {said}");
            }
        }
        for said in [
            sheet_trouble(Lang::En, &not_ours),
            sheet_refusal(Lang::En, &at, &SheetRefusal::NotOurs { root: root.clone() }),
        ] {
            assert!(said.contains("not /w/proj's"), "남의 것이라는 글이 뒤집혔다 — {said}");
        }
    }
}
