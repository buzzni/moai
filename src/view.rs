//! 사람이 읽을 줄을 만든다. **순수 함수다** — 아무것도 찍지 않는다.
//!
//! 표를 쓰는 이상 폭 계산이 필요하다. 한글은 터미널에서 두 칸을 먹으므로
//! `len()` 으로 맞추면 한글 제목이 섞인 표가 전부 어긋난다.

use crate::config::Config;
use crate::model::{Issue, JournalEntry, Kind};
use crate::report::{Roll, StatusReport, Warning, is_group};
use crate::style::{self, paint};
use crate::text::{clip, sanitize, width};
use crate::worktree::Origin;
use anstyle::Style;
use std::collections::BTreeMap;

/// 제목이 이보다 길면 자른다. 표가 접히면 표가 아니다.
const TITLE_CAP: usize = 44;
/// 에픽 열은 곁다리라 더 짧게 자른다.
const EPIC_CAP: usize = 20;
/// 진행 막대 칸 수.
const BAR: usize = 10;

/// 칠한 뒤 **칠하지 않은 폭**을 기준으로 채운다. 순서를 바꾸면 이스케이프가
/// 폭에 세어져 표가 어긋난다.
fn cell(style: Style, text: &str, w: usize) -> String {
    let pad = w.saturating_sub(width(text));
    format!("{}{}", paint(style, text), " ".repeat(pad))
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

fn tags_of(i: &Issue) -> String {
    i.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ")
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
pub fn deferred_for(i: &Issue, root: Option<&str>, now: &str) -> Option<String> {
    if let Some(r) = root.filter(|r| *r != i.id) {
        return Some(format!("미룸 — {r} 밑"));
    }
    let at = i.deferred_at.as_deref()?;
    Some(match crate::model::days_since(at, now) {
        Some(d) if d > 0 => format!("미룸 ({d}일)"),
        _ => "미룸".to_string(),
    })
}

/// 계획에서 빠진 줄에 붙이는 한 마디. **도로 집는 말은 실제로 미룬 줄을 댄다** —
/// 물려받은 줄에 `--undo` 를 치면 "이미 그렇다" 로 끝나고 아무것도 안 풀린다.
/// 제가 미룬 줄이면 그 줄이 곧 미룬 곳이라 말이 하나로 되고, 부르는 쪽이 둘을
/// 가르는 `if` 를 둘 까닭이 없다.
pub fn shelved_by(root: &str) -> String {
    format!("{root} 를 미뤄 둬서 보드와 ready 에서는 빠져 있다 — `moai defer {root} --undo`")
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
    fn says(&self) -> Vec<String> {
        [
            (self.done, "done", "--all"),
            (self.deferred, "미룸", "--deferred"),
            (self.ideas, "idea", "--type idea"),
        ]
            .into_iter()
            .filter(|(n, _, _)| *n > 0)
            .map(|(n, what, how)| format!("{what} {n}건 숨김 — `{how}`"))
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
    pub fn note(&self) -> Option<String> {
        let why = self.says();
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
    epics: &BTreeMap<&str, String>,
    asked_deferred: bool,
    wh: &crate::query::Where,
    origin: &Origin,
) -> Vec<String> {
    if issues.is_empty() {
        let why = hidden.says();
        return vec![if why.is_empty() {
            "없다.".into()
        } else {
            format!("없다. {}", why.join(" · "))
        }];
    }

    let show_tags = issues.iter().any(|i| !i.tags.is_empty());
    let show_epic = issues.iter().any(|i| epics.contains_key(i.id.as_str()));
    // 미룸 표를 달지 말지는 **부르는 쪽의 물음**에서 온다. 한때 결과의 내용
    // 으로 정했는데(`any(|i| !i.is_deferred())`), 그러면 `--all` 이 마침 전부
    // 미룬 것만 냈을 때 표가 통째로 사라져 계획 밖의 줄이 일과 똑같이 보인다 —
    // 안 물었는데 사라지는 것이 물어서 붙는 군더더기보다 나쁘다.
    let mark_deferred = !asked_deferred;
    let heads: Vec<(String, usize)> = issues
        .iter()
        .map(|i| marked(origin.branch(&i.id), &i.title, TITLE_CAP, title_style(i)))
        .collect();
    let tags: Vec<String> = issues.iter().map(tags_of).collect();
    // 에픽 열은 **제목**을 보여준다. id 를 보여주면 사람이 그걸 다시 찾아봐야 한다.
    let epics: Vec<String> = issues
        .iter()
        .map(|i| match epics.get(i.id.as_str()) {
            None => "—".into(),
            Some(t) => clip(t, EPIC_CAP),
        })
        .collect();

    let w_id = issues.iter().map(|i| width(&i.id)).max().unwrap_or(2).max(2);
    let w_title = heads.iter().map(|(_, w)| *w).max().unwrap_or(4);
    let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0);

    let mut out = Vec::with_capacity(issues.len() + 2);
    let mut head = format!(
        "{}{}{}{}",
        cell(style::HEAD, "ID", w_id + 3),
        cell(style::HEAD, "P", 4),
        cell(style::HEAD, "S", 3),
        cell(style::HEAD, "제목", if show_tags || show_epic { w_title + 2 } else { 0 }),
    );
    if show_tags {
        head.push_str(&cell(style::HEAD, "태그", if show_epic { w_tags + 2 } else { 0 }));
    }
    if show_epic {
        head.push_str(&paint(style::HEAD, "에픽"));
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
            row.push_str(&format!("   {}", paint(style::DIM, "미룸")));
        }
        out.push(row.trim_end().to_string());
    }

    out.push(String::new());
    out.push(summary(issues, cfg, hidden, wh));
    out
}

/// 칸별 건수. 묶음은 **줄마다 그린 그 칸**으로 센다 — S 열에 `▸` 로 선 에픽을
/// 꼬리에서 `todo` 로 세면 한 화면이 같은 줄을 두 칸으로 말한다.
fn summary(issues: &[Issue], cfg: &Config, hidden: Hidden, wh: &crate::query::Where) -> String {
    let counts: Vec<String> = cfg
        .statuses
        .iter()
        .filter_map(|s| {
            let n = issues.iter().filter(|i| wh.column(i) == s).count();
            (n > 0).then(|| format!("{s} {n}"))
        })
        .collect();
    let mut line = format!("{}건 ({})", issues.len(), counts.join(" · "));
    let why = hidden.says();
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
            format!(
                "{}{}",
                paint(style::status_style("done"), &"█".repeat(filled)),
                paint(style::DIM, &"░".repeat(BAR - filled))
            )
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
    origin: &'a Origin,
}

/// **그린 줄의 첨자도 돌려준다.** 거름망에 안 걸린 줄도 걸린 자손의 조상이면
/// 그려지므로, 꼬리가 "숨겼다" 고 셀 것은 그리지 않은 줄뿐이다. 세는 쪽이
/// 따로 훑으면 방금 그린 줄을 숨겼다고 말한다(moai-wi67).
pub fn tree(
    all: &[Issue],
    index: &crate::nav::Index,
    keep: &dyn Fn(usize) -> bool,
    rolls: &[Roll],
    origin: &Origin,
) -> (Vec<String>, std::collections::BTreeSet<usize>) {
    let mut out = Vec::new();
    let mut drawn = std::collections::BTreeSet::new();
    let cx = Ctx { all, index, keep, rolls, origin };
    walk(&mut out, &mut drawn, &cx, &crate::nav::Path::new(), 0);
    if out.is_empty() {
        out.push("없다.".into());
    }
    (out, drawn)
}

/// 소속 없는 줄인가. **잎이든, 자식을 거느려 디렉터리가 된 줄이든 같다.**
fn is_loose(e: &crate::nav::Entry) -> bool {
    use crate::nav::{Entry, Seg};
    matches!(e, Entry::Leaf { .. } | Entry::Dir { seg: Seg::Issue(_), .. })
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
        for e in &entries {
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
        match path.is_empty().then(|| cx.rolls.iter().find(|r| r.id.is_none())).flatten() {
            Some(roll) => out.push(head(roll, &roll.title, n, None)),
            None => out.push(format!("{}  {n}건", paint(style::HEAD, "에픽 없음"))),
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
            let title = cx.index.label(cx.all, e);
            match cx.rolls.iter().find(|r| r.id.as_deref() == Some(id)) {
                Some(roll) => out.push(head(roll, &title, shown, cx.origin.branch(id))),
                // 집계가 없을 때도 **id 는 낸다** — 제목만 내면 그것을
                // 다시 찾아봐야 하고, 묶음을 펼친 이유가 사라진다.
                None => out.push(format!(
                    "{}  {}  {shown}건",
                    paint(style::ID, id),
                    marked(cx.origin.branch(id), &title, usize::MAX, style::HEAD).0,
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
            out.push(format!(
                "{}  {}건",
                paint(style, &cx.index.label(cx.all, e)),
                under(cx, path, e)
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
    origin: &Origin,
) -> Vec<String> {
    let mut out = Vec::new();
    let cx = Ctx { all, index, keep, rolls, origin };
    walk(&mut out, &mut std::collections::BTreeSet::new(), &cx, at, 0);
    out
}

fn head(roll: &Roll, title: &str, shown: usize, branch: Option<&str>) -> String {
    match &roll.id {
        // 묶음일 뿐 진척을 가진 것이 아니므로, 걸러진 뒤 **보이는** 수를 말한다.
        None => format!("{}  {}건", paint(style::HEAD, title), shown),
        Some(id) => {
            let pct = match roll.percent {
                None => paint(style::DIM, "자식 없음"),
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
        marked(cx.origin.branch(&i.id), &i.title, TITLE_CAP, title_style(i)).0,
    );
    if !i.tags.is_empty() {
        line.push_str(&format!("   {}", paint(style::TAG, &tags_of(i))));
    }
    if i.is_deferred() || cx.index.deferred_root(&i.id).is_some() {
        line.push_str(&format!("   {}", paint(style::DIM, "미룸")));
    }
    out.push(line);
}

/// 경고 하나를 사람 말로. **여기가 이 제품의 목소리다.**
fn says(w: &Warning) -> String {
    let n = w.count;
    match w.kind {
        "no_epic" => format!(
            "에픽 없는 이슈 {n}건 (열린 것의 {}%)",
            (w.ratio.unwrap_or(0.0) * 100.0).round() as u32
        ),
        "no_milestone" => format!("마일스톤에 안 붙은 이슈 {n}건"),
        "stale_review" => format!("review 에 {}일 넘게 멈춘 것 {n}건", w.days.unwrap_or(0)),
        // review 도 벌여 놓은 일이다. "진행 중" 이라고 하면 review 경고와
        // 같은 이슈가 두 번 나오는 것이 말이 안 되게 보인다.
        "wip_overload" => format!("한 번에 벌여 놓은 것 {n}건 — 하나씩 끝내는 편이 낫다"),
        "stale_progress" => format!("집어 놓고 {}일 넘게 안 건드린 것 {n}건", w.days.unwrap_or(0)),
        // `days` 는 "막힌 기간" 이 아니라 "지금 칸에 머문 기간" 이다 — 막 막힌
        // 것을 "며칠째 막혀 있다" 고 잘못 말하지 않으려고 이렇게 적는다.
        "blocked_stale" => format!("막힌 채로 {}일 넘게 멈춰 있는 것 {n}건", w.days.unwrap_or(0)),
        // 막는 쪽이 어느 목록에도 없으므로 **어디서 찾는지를 같이 말한다.**
        // `ready` 가 같은 줄에 대는 말과 같다. **막는 쪽이 아니라 미룬 곳이다** — 막는
        // 줄이 미룬 에픽 밑이면 그 줄에 `--undo` 를 쳐 봐야 "이미 그렇다" 로 끝난다.
        "blocked_by_deferred" => format!("미뤄 둔 것에 막혀 못 집는 일 {n}건 — 미룬 곳을 도로 집거나 막음을 푼다"),
        "empty_epic" => format!("속이 빈 에픽 {n}건 — 계획만 세우고 안 채웠다"),
        // **끊긴 것과 종류가 틀린 것을 한 낱말로 말한다** — 둘을 가려 말하면
        // 고치는 손이 달라지는 것도 아닌데 경고가 둘로 늘어난다.
        "dangling_epic" => format!("에픽으로 쓸 수 없는 것을 가리키는 줄 {n}건"),
        "dangling_milestone" => format!("마일스톤으로 쓸 수 없는 것을 가리키는 줄 {n}건"),
        "orphan_child" => format!("부모 줄이 없는 자식 {n}건"),
        "dangling_blocked_by" => format!("없는 이슈에게 막혀 있다는 것 {n}건"),
        // **알림이지 경고가 아니다.** 고칠 것이 있다는 말이 아니라, 담아 둔
        // 것을 한 번 펼쳐 볼 때가 됐다는 말이다.
        // **오늘 것에 "0일" 을 붙이지 않는다.** 나이를 말하는 까닭은 오래된
        // 것을 드러내려는 것인데, 갓 담은 것에까지 괄호가 붙으면 그 괄호가
        // 뜻을 잃는다.
        "idea_pile" => match w.oldest {
            Some(d) if d > 0 => format!("쌓인 idea {n}건 (가장 오래된 것 {d}일)"),
            _ => format!("쌓인 idea {n}건"),
        },
        "deferred" => match w.oldest {
            Some(d) if d > 0 => format!("미뤄 둔 것 {n}건 (가장 오래된 것 {d}일)"),
            _ => format!("미뤄 둔 것 {n}건"),
        },
        "unknown_field" => format!("모르는 필드를 들고 있는 줄 {n}건 — 새 바이너리가 쓴 파일일 수 있다"),
        // **까닭을 단정하지 않는다.** 머지를 잘못 푼 흔적일 수도, 못 읽는 줄이
        // 산 줄의 id 를 쓰고 있는 것일 수도 있다(moai-4dk4). 둘 다 줄 번호는
        // `moai show` 가 낸다.
        "duplicate_id" => format!("id 가 두 번 있다 {n}건 — 머지 흔적이거나 못 읽는 줄과 겹친다"),
        "unreadable_line" => format!("읽을 수 없는 줄 {n}개"),
        other => format!("{other} {n}건"),
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
    origin: &Origin,
) -> Vec<String> {
    let by_id: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    // **겹쳐 본 화면은 머리에서 그렇다고 말한다.** 줄마다 붙는 `⎇` 는 옆에서 온
    // 줄에만 서므로, 옆 워크트리가 조용하면 겹쳐 본 보드와 제 보드가 똑같이 보인다.
    let trees = origin.labels();
    let overlaid = match trees.is_empty() {
        true => String::new(),
        false => format!(
            "   {}",
            paint(style::BRANCH, &format!("{} {} 겹쳐 봄", style::BRANCH_GLYPH, clip(&trees.join(", "), TITLE_CAP)))
        ),
    };
    let mut out = vec![
        format!(
            "{}  {}       {}{overlaid}",
            paint(style::HEAD, &format!("이슈 {}", st.total)),
            paint(style::DIM, &format!("· 에픽 {}", st.epics.len())),
            paint(style::DIM, at),
        ),
        String::new(),
    ];

    out.push(board(cfg, &st.counts));

    let shelved = crate::report::put_off(issues);
    for (label, rolls) in [("마일스톤", &st.milestones), ("에픽", &st.epics)] {
        if rolls.is_empty() {
            continue;
        }
        out.push(String::new());
        if !st.milestones.is_empty() {
            out.push(paint(style::DIM, label));
        }
        let heads: Vec<(String, usize)> = rolls
            .iter()
            .map(|e| marked(e.id.as_deref().and_then(|id| origin.branch(id)), &e.title, EPIC_CAP, style::EPIC))
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
                _ if put_off => paint(style::DIM, "   미룸"),
                None => paint(style::DIM, "   자식 없음"),
                _ if folded => paint(style::status_style(crate::config::DONE), "   닫힘"),
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
        out.push(format!("{} {}", paint(mark, glyph), says(w)));
        out.extend(preview(w, &by_id, now, origin));
    }
    // **알림만 있는 것은 "아무 문제 없다" 이다.** 알림은 `notices` 에 따로
    // 있으므로 `warnings` 가 비면 고칠 것이 없다 — 생각을 담거나 무언가를 미룬
    // 순간부터 이 줄이 사라지면, 세션을 닫기 전에 "경고가 늘지 않았는지" 보는
    // 사람이 알림을 경고로 읽는다.
    if st.warnings.is_empty() {
        out.push(String::new());
        out.push(format!("{} 드러난 문제 없다", paint(style::status_style("done"), "✓")));
    }

    out.push(String::new());
    let net = st.flow.net;
    out.push(format!(
        "최근 {}일   생성 {}  ·  완료 {}   {}",
        st.flow.days,
        st.flow.created,
        st.flow.done,
        match net.cmp(&0) {
            std::cmp::Ordering::Greater => paint(style::WARN, &format!("쌓이는 중 +{net}")),
            std::cmp::Ordering::Less => paint(style::status_style("done"), &format!("줄어드는 중 {net}")),
            std::cmp::Ordering::Equal => paint(style::DIM, "제자리"),
        }
    ));
    out.push(String::new());
    out.push(paint(style::DIM, "다음:  `moai ready` 로 집을 것을 고른다"));
    out
}

/// 보드 한 줄 — config 의 칸 차례 그대로. 한 프로젝트의 `status` 와 한눈 보기가
/// 같은 줄을 낸다: 둘이 갈라지면 같은 보드를 두 모양으로 읽는다.
fn board(cfg: &Config, counts: &BTreeMap<String, usize>) -> String {
    let cols: Vec<String> = cfg
        .statuses
        .iter()
        .map(|s| {
            let style = style::status_style(s);
            format!(
                "{} {}",
                paint(style, style::glyph(s)),
                paint(style, &format!("{s} {}", counts.get(s).copied().unwrap_or(0)))
            )
        })
        .collect();
    format!("  {}", cols.join("    "))
}

/// 경고마다 앞의 몇 건만 보여 주고 나머지는 세어서 말한다. 다 늘어놓으면
/// 정작 봐야 할 다음 경고가 화면 밖으로 밀린다.
fn preview(w: &Warning, by_id: &BTreeMap<&str, &Issue>, now: &str, origin: &Origin) -> Vec<String> {
    const SHOW: usize = 3;
    let mut out = Vec::new();
    // 벌여 놓은 것과 깨진 것은 id 만 한 줄에 늘어놓는다 — 제목이 정보를 안 준다.
    if matches!(w.kind, "wip_overload" | "duplicate_id" | "orphan_child" | "dangling_blocked_by") {
        if !w.ids.is_empty() {
            out.push(format!("    {}", paint(style::DIM, &w.ids.join("   "))));
        }
        return out;
    }
    // 에픽에 대한 말은 칸도 나이도 뜻이 없다. 어느 에픽인지만 말한다.
    if matches!(
        w.kind,
        "empty_epic" | "unknown_field" | "dangling_epic" | "dangling_milestone"
    ) {
        for id in w.ids.iter().take(SHOW) {
            let title = by_id.get(id.as_str()).map(|i| i.title.as_str()).unwrap_or("");
            out.push(format!(
                "    {}  {}",
                paint(style::ID, id),
                marked(origin.branch(id), title, TITLE_CAP, style::PLAIN).0
            ));
        }
        let rest = w.ids.len().saturating_sub(SHOW);
        if rest > 0 {
            out.push(format!("    {}", paint(style::DIM, &format!("{rest}건 더"))));
        }
        return out;
    }
    for id in w.ids.iter().take(SHOW) {
        let Some(i) = by_id.get(id.as_str()) else {
            out.push(format!("    {}", paint(style::ID, id)));
            continue;
        };
        let age = crate::model::days_since(&i.status_since, now)
            .map(|d| format!("{d}일"))
            .unwrap_or_default();
        out.push(format!(
            "    {}  {}  {}  {}  {}",
            paint(style::ID, &i.id),
            paint(style::priority_style(i.priority()), &format!("p{}", i.priority())),
            paint(style::status_style(i.status.as_str()), style::glyph(i.status.as_str())),
            paint(style::DIM, &format!("{age:>4}")),
            marked(origin.branch(&i.id), &i.title, TITLE_CAP, style::PLAIN).0,
        ));
    }
    let rest = w.ids.len().saturating_sub(SHOW);
    let more = if rest > 0 { format!("{rest}건 더") } else { String::new() };
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

/// 집을 수 있는 일. 그리고 이미 벌여 놓은 것.
pub fn ready(
    picks: &[&Issue],
    epics: &BTreeMap<&str, String>,
    wip: &[&Issue],
    held: &[crate::report::Held],
    origin: &Origin,
) -> Vec<String> {
    let mut out = vec![format!("집을 수 있는 일  {}건", picks.len())];
    if picks.is_empty() {
        out.push(String::new());
        out.push(paint(style::DIM, "없다. `moai show` 로 무엇이 밀려 있는지 본다"));
    } else {
        out.push(String::new());
        let heads: Vec<(String, usize)> = picks
            .iter()
            .map(|i| marked(origin.branch(&i.id), &i.title, TITLE_CAP, style::PLAIN))
            .collect();
        let tags: Vec<String> = picks.iter().map(|i| tags_of(i)).collect();
        let w_id = picks.iter().map(|i| width(&i.id)).max().unwrap_or(2);
        let w_title = heads.iter().map(|(_, w)| *w).max().unwrap_or(4);
        let w_tags = tags.iter().map(|t| width(t)).max().unwrap_or(0);

        for ((i, (title, w_this)), tag) in picks.iter().zip(&heads).zip(&tags) {
            let epic = match epics.get(i.id.as_str()) {
                None => "에픽 없음".to_string(),
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

    // 이미 벌여 놓은 것을 먼저 알린다. 새로 집기 전에 볼 것이다.
    if !wip.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            paint(
                style::DIM,
                &format!("이미 잡고 있는 것 {}건 — 새로 집기 전에 끝내는 편이 낫다", wip.len())
            )
        ));
        // **어느 워크트리에서 잡았는지도 댄다.** `--worktree` 로 부른 쪽이 가장 알고
        // 싶은 것이 "옆에서 누가 무엇을 잡았나" 다 — id 만 늘어놓으면 지금 브랜치의
        // 일과 구별이 안 된다.
        let ids: Vec<String> = wip
            .iter()
            .map(|i| match origin.branch(&i.id) {
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
    if !held.is_empty() {
        out.push(String::new());
        out.push(format!(
            "{} {}",
            paint(style::WARN, "!"),
            paint(
                style::DIM,
                &format!(
                    "미뤄 둔 것에 막혀 못 집는 일 {}건 — 미룬 곳을 도로 집거나 막음을 푼다",
                    held.len()
                )
            )
        ));
        for h in held {
            // **도로 집는 말은 미룬 곳을 댄다.** 막는 줄이 미룬 에픽 밑이면 그
            // 줄에 `--undo` 를 쳐 봐야 "이미 그렇다" 로 끝난다.
            out.push(format!(
                "  {}  {}  {}  {}",
                paint(style::ID, &h.issue.id),
                marked(origin.branch(&h.issue.id), &h.issue.title, TITLE_CAP, style::DIM).0,
                paint(style::DIM, &format!("← {}", h.by.join(" · "))),
                paint(style::DIM, &format!("moai defer {} --undo", h.undo.join(" "))),
            ));
        }
    }
    out
}

/// 줄 하나만 보고는 모르고 **파일 전체를 읽어야 아는 것.** 상세가 제 줄과
/// 자식 줄에 단다.
#[derive(Default)]
pub struct Seen<'a> {
    /// 계획에서 빠진 줄 → 그것을 뺀 줄 (`report::deferred_roots`).
    pub roots: BTreeMap<&'a str, &'a str>,
    /// 묶음 → 멤버에서 읽은 칸 (`report::group_states`).
    pub states: BTreeMap<&'a str, &'a str>,
    /// 다른 워크트리에서 온 줄 (`worktree::overlay`). `--worktree` 가 아니면 비었다.
    pub origin: Option<&'a Origin>,
}

/// **손으로 옮긴 칸이 서 있는 칸과 다르면** 그렇다고 말하는 낱말. CLI 상세와
/// 탐색기가 같은 말을 받는다.
///
/// `moai mv <에픽> done` 을 한 사람이 상세에서 `in_progress` 만 보면 쓰기가 안
/// 먹은 줄 안다. 첫 칸 그대로인 줄은 말하지 않는다 — 묶음은 거의 다 만든 칸에
/// 서 있어, 그것까지 말하면 모든 에픽 상세에 같은 군말이 붙는다.
pub fn unread_column(i: &Issue, col: &str, cfg: &Config) -> Option<String> {
    (col != i.status.as_str() && i.status.as_str() != cfg.first_status())
        .then(|| format!("칸은 멤버에서 읽는다 (적힌 칸 `{}` 은 안 읽는다)", i.status))
}

/// `moai mv <묶음>` 이 내는 한 줄 — 서 있는 칸과, `done` 으로 옮기려 했으면 **실제로
/// 접히는 길.**
///
/// 끝난 멤버가 있으면 남은 멤버를 미뤄 접힌다 — 미룬 멤버는 칸 셈에서 빠지므로 남은
/// 것이 끝난 것뿐이 된다. **끝난 멤버가 하나도 없으면 전부 미뤄도 첫 칸이다**: 셀
/// 멤버가 없는 묶음은 첫 칸에 서므로(`report::group_states`), 그때 "남은 멤버를 미룬다"
/// 를 시키면 시킨 대로 한 뒤에도 같은 말이 돌아온다. 빈 묶음도 마찬가지다. 그 자리에서
/// 실제로 듣는 말은 묶음 제 `defer` 다 — 계획에서 빠지면 보드와 `ready` 에서 함께 빠진다.
pub fn group_moved(id: &str, col: &str, closing: bool, finished: bool) -> String {
    let fold = match (closing, finished) {
        (false, _) => String::new(),
        (true, true) => " — 접으려면 남은 멤버를 `moai defer` 한다".to_string(),
        (true, false) => {
            format!(" — 끝난 멤버가 없어 닫히지 않는다. 계획에서 빼려면 `moai defer {id}`")
        }
    };
    format!("묶음의 칸은 멤버에서 읽는다. 서 있는 칸은 {col}{fold}")
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
    let branch_of = |id: &str| seen.origin.and_then(|o| o.branch(id));
    let mut out = vec![format!(
        "{}   {}",
        paint(style::ID, &i.id),
        marked(branch_of(&i.id), &i.title, usize::MAX, title_style(i)).0
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
    if let Some(d) = deferred_for(i, seen.roots.get(i.id.as_str()).copied(), now) {
        line.push_str(&format!(" · {}", paint(style::WARN, &d)));
    }
    if let Some(n) = unread_column(i, col, cfg) {
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

    if let Some(e) = &i.epic {
        let title = epic.map(|e| e.title.as_str()).unwrap_or("(없는 에픽)");
        out.push(format!("  에픽   {}  {title}", paint(style::ID, e)));
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
        if let Some(d) = deferred_for(c, seen.roots.get(c.id.as_str()).copied(), now) {
            tail.push_str(&format!(" · {}", paint(style::WARN, &d)));
        }
        let ccol = crate::report::column(c, &seen.states);
        out.push(format!(
            "  자식   {}  {}  ({} {}){tail}",
            paint(style::ID, &c.id),
            marked(branch_of(&c.id), &c.title, TITLE_CAP, style::PLAIN).0,
            paint(style::status_style(ccol), style::glyph(ccol)),
            paint(style::DIM, ccol),
        ));
    }

    let age = crate::model::days_since(&i.updated_at, now)
        .map(|d| match d {
            0 => "  (오늘)".to_string(),
            n => format!("  ({n}일 전)"),
        })
        .unwrap_or_default();
    out.push(format!(
        "  생성   {}      수정  {}{}",
        paint(style::DIM, &stamp(&i.created_at)),
        paint(style::DIM, &stamp(&i.updated_at)),
        paint(style::DIM, &age),
    ));

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
    crate::markdown::layout(&blocks, BODY)
        .iter()
        .map(|line| {
            // 빈 줄은 빈 줄이다. 들여쓰기를 얹으면 줄 끝에 뜻 없는 공백이
            // 남아, 본문을 긁어 붙이거나 diff 를 볼 때마다 따라다닌다.
            if line.is_empty() {
                return String::new();
            }
            let painted: String =
                line.iter().map(|s| paint(role_style(s.role), &s.text)).collect();
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

/// 저널을 **그대로 찍는다. 접지 않는다.**
pub fn history(journal: &[JournalEntry], cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    if !journal.is_empty() {
        out.push(String::new());
        out.push(paint(style::HEAD, "이력"));
        for e in journal {
            // 메모는 여러 줄일 수 있다. 한 원소에 `\n` 을 담으면 "원소 하나가
            // 한 줄" 이라는 약속이 깨지고, 이어지는 줄이 열을 잃는다.
            let ts = short_stamp(&e.ts);
            let pad = " ".repeat(width(&ts) + 5);
            for (n, l) in entry(e, cfg).split('\n').enumerate() {
                out.push(match n {
                    0 => format!("  {}   {l}", paint(style::DIM, &ts)),
                    _ => format!("{pad}{l}"),
                });
            }
        }
    }
    out
}

fn entry(e: &JournalEntry, cfg: &Config) -> String {
    let what = match e.kind.as_str() {
        "create" => "생성".to_string(),
        "rm" => "삭제".to_string(),
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
}

/// 한눈 보기에서 연 프로젝트 하나의 집을 것 — `moai ready` 가 `.moai` 밖에서 낸다.
pub struct Picks<'a> {
    pub picks: Vec<&'a Issue>,
    /// 못 읽는 줄의 수. 그 줄에 있던 일은 목록에서 빠져 있다.
    pub unreadable: usize,
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
) -> Vec<String> {
    let mut out = vec![overview_head("등록한 프로젝트", &format!("{}곳", projects.len()), reg)];
    let w_name = projects.iter().map(|p| width(&sanitize(&p.name))).max().unwrap_or(0);
    for (p, s) in projects.iter().zip(seen) {
        out.push(String::new());
        out.push(project_head(p, ""));
        let crate::projects::Seen::Ok(b) = s else {
            out.push(unopened(p, s));
            continue;
        };
        out.push(board(b.cfg, &b.status.counts));
        let shown = &b.picked[..b.picked.len().min(PICKED_SHOWN)];
        // 보인 것끼리 id 폭을 맞춘다 — 자식 id(`x-1a2b.3`)가 섞이면 줄마다 제 폭으로는 제목 칸이 어긋난다.
        let w_id = shown.iter().map(|i| width(&i.id)).max().unwrap_or(0);
        let hue = style::project_colour(&p.path, p.hue);
        for i in shown {
            out.push(format!(
                "  {}{}{}  {}",
                cell(hue, &sanitize(&p.name), w_name + 2),
                cell(hue, &i.id, w_id + 2),
                paint(style::status_style(i.status.as_str()), style::glyph(i.status.as_str())),
                clip(&i.title, TITLE_CAP),
            ));
        }
        let rest = b.picked.len().saturating_sub(PICKED_SHOWN);
        if rest > 0 {
            out.push(format!("  {}", paint(style::DIM, &format!("집은 것 {rest}건 더"))));
        }
        // **알림은 세지 않는다** — `moai status` 의 "드러난 문제 없다" 와 같은 자다.
        let n = b.status.warnings.len();
        let fatal = b.status.warnings.iter().filter(|w| w.fatal).count();
        let go = paint(style::DIM, &format!("→ `moai -C {} status`", shell_arg(&p.path)));
        out.push(match (n, fatal) {
            (0, _) => format!("  {} 드러난 문제 없다", paint(style::status_style("done"), "✓")),
            (_, 0) => format!("  {} 경고 {n}건  {go}", paint(style::WARN, "!")),
            (_, f) => format!("  {} 경고 {n}건 (데이터가 깨졌다 {f}건)  {go}", paint(style::ERROR, "!")),
        });
    }
    problems(&mut out, reg);
    out.push(String::new());
    out.push(paint(style::DIM, "다음:  `moai ready` 로 프로젝트마다 집을 것을 본다"));
    out
}

/// 등록한 프로젝트마다 집을 수 있는 일 — 앞의 몇 건만.
pub fn projects_ready(
    projects: &[crate::projects::Project],
    seen: &[crate::projects::Seen<Picks>],
    reg: &crate::user_config::Registry,
) -> Vec<String> {
    use crate::projects::Seen;
    let total: usize = seen
        .iter()
        .map(|s| match s {
            Seen::Ok(k) => k.picks.len(),
            _ => 0,
        })
        .sum();
    let mut out = vec![overview_head(
        "집을 수 있는 일",
        &format!("프로젝트 {}곳 · {total}건", projects.len()),
        reg,
    )];
    let w_name = projects.iter().map(|p| width(&sanitize(&p.name))).max().unwrap_or(0);
    for (p, s) in projects.iter().zip(seen) {
        out.push(String::new());
        let Seen::Ok(k) = s else {
            out.push(project_head(p, ""));
            out.push(unopened(p, s));
            continue;
        };
        out.push(project_head(p, &format!("{}건", k.picks.len())));
        let shown = &k.picks[..k.picks.len().min(READY_SHOWN)];
        let w_id = shown.iter().map(|i| width(&i.id)).max().unwrap_or(0);
        let hue = style::project_colour(&p.path, p.hue);
        for i in shown {
            out.push(format!(
                "  {}{}{}{}",
                cell(hue, &sanitize(&p.name), w_name + 2),
                cell(hue, &i.id, w_id + 2),
                cell(style::priority_style(i.priority()), &format!("p{}", i.priority()), 4),
                clip(&i.title, TITLE_CAP),
            ));
        }
        let rest = k.picks.len() - shown.len();
        if rest > 0 {
            let go = format!("{rest}건 더 → `moai -C {} ready`", shell_arg(&p.path));
            out.push(format!("  {}", paint(style::DIM, &go)));
        }
        if k.unreadable > 0 {
            out.push(format!(
                "  {} 읽을 수 없는 줄 {}개 — 어느 줄인지는 `moai -C {} show` 가 낸다",
                paint(style::ERROR, "!"),
                k.unreadable,
                shell_arg(&p.path)
            ));
        }
    }
    problems(&mut out, reg);
    out
}

/// 한눈 보기의 머리 — 무엇을 몇이나 봤는지와, 목록을 읽은 사용자 설정 파일.
fn overview_head(what: &str, count: &str, reg: &crate::user_config::Registry) -> String {
    let at = reg.path.as_ref().map(|p| sanitize(&p.display().to_string())).unwrap_or_default();
    format!("{}  {count}       {}", paint(style::HEAD, what), paint(style::DIM, &at))
        .trim_end()
        .to_string()
}

fn project_head(p: &crate::projects::Project, tail: &str) -> String {
    let at = sanitize(&p.path.display().to_string());
    let head = style::project_colour(&p.path, p.hue).effects(style::HEAD.get_effects());
    format!("{}  {}   {tail}", paint(head, &sanitize(&p.name)), paint(style::DIM, &at)).trim_end().to_string()
}

/// 열지 못한 프로젝트의 한 줄. **무엇을 하면 되는지를 함께 댄다.**
pub(crate) fn unopened<T>(p: &crate::projects::Project, s: &crate::projects::Seen<T>) -> String {
    use crate::projects::Seen;
    let at = shell_arg(&p.path);
    match s {
        Seen::Ok(_) => String::new(),
        // init 전은 고칠 것이 아니다 — 나중에 `init` 하면 보이는 것이 요구다. `!` 를 달지 않는다.
        Seen::Uninit => {
            let say = format!("init 전 — `moai -C {at} init` 으로 시작하면 여기 보인다");
            format!("  {} {}", paint(style::DIM, "·"), paint(style::DIM, &say))
        }
        Seen::Missing => {
            let go = format!("→ 옮겼으면 새 자리를 등록하고, 아니면 `moai project rm {at}`");
            format!("  {} 디렉터리가 없다  {}", paint(style::WARN, "!"), paint(style::DIM, &go))
        }
        Seen::Unreadable { error } => format!("  {} 못 읽는다 — {}", paint(style::ERROR, "!"), sanitize(error)),
    }
}

/// 사용자 설정을 읽다 만난 것을 한 줄씩. 목록을 막지 않는다.
fn problems(out: &mut Vec<String>, reg: &crate::user_config::Registry) {
    if reg.problems.is_empty() {
        return;
    }
    out.push(String::new());
    for p in &reg.problems {
        out.push(format!("{} {}", paint(style::WARN, "!"), sanitize(p)));
    }
}

/// 명령 안내에 넣을 경로 — 제어문자를 걷고 셸이 가를 글자가 있으면 감싼다.
fn shell_arg(p: &std::path::Path) -> String {
    crate::text::shell_word(&sanitize(&p.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn cfg() -> Config {
        Config::parse("prefix = \"argos\"\n").unwrap()
    }

    fn no_epics() -> BTreeMap<&'static str, String> {
        BTreeMap::new()
    }

    fn issue(id: &str, title: &str, status: &str) -> Issue {
        Issue::new(id.into(), title.into(), Kind::Issue, Status::new(status), "2026-09-11T04:12:03Z")
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
        let issues = vec![
            issue("argos-0001", "한글 제목이다", "todo"),
            issue("argos-0002", "ascii title", "review"),
        ];
        let out = plain(&list(&issues, &cfg(), Hidden { done: 0, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()));
        let cols: Vec<usize> = out[1..3]
            .iter()
            .map(|l| width(l.split_once("  ").unwrap().0))
            .collect();
        assert_eq!(cols[0], cols[1], "{out:#?}");
        // 제목 시작 열이 같다
        let starts: Vec<usize> = out[1..3]
            .iter()
            .map(|l| width(&l[..l.find(['한', 'a']).unwrap()]))
            .collect();
        assert_eq!(starts[0], starts[1], "{out:#?}");
    }

    /// 헤더의 `제목` 과 줄의 제목이 같은 칸에서 시작한다.
    #[test]
    fn header_lines_up_with_rows() {
        let issues = vec![issue("argos-0001", "제목이다", "todo")];
        let out = plain(&list(&issues, &cfg(), Hidden { done: 0, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()));
        let head_at = width(&out[0][..out[0].find("제목").unwrap()]);
        let row_at = width(&out[1][..out[1].find("제목이다").unwrap()]);
        assert_eq!(head_at, row_at, "{out:#?}");
    }

    #[test]
    fn empty_list_says_why() {
        assert_eq!(plain(&list(&[], &cfg(), Hidden { done: 0, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()))[0], "없다.");
        assert!(plain(&list(&[], &cfg(), Hidden { done: 3, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()))[0].contains("done 3건"));
    }

    #[test]
    fn summary_counts_each_column() {
        let issues = vec![issue("argos-0001", "a", "todo"), issue("argos-0002", "b", "todo")];
        let out = plain(&list(&issues, &cfg(), Hidden { done: 5, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()));
        let last = out.last().unwrap();
        assert!(last.starts_with("2건 (todo 2)"), "{last}");
        assert!(last.contains("done 5건 숨김"), "{last}");
    }

    #[test]
    fn long_titles_are_clipped_not_wrapped() {
        let long = "가".repeat(80);
        let out = plain(&list(&[issue("argos-0001", &long, "todo")], &cfg(), Hidden { done: 0, ..Hidden::default() }, &no_epics(), false, &Default::default(), &Origin::default()));
        assert!(out[1].ends_with('…'), "{:?}", out[1]);
        assert!(width(&out[1]) < 80, "{:?}", out[1]);
    }

    /// 다른 브랜치에서 온 줄은 **색을 꺼도** `⎇ <브랜치>` 로 읽히고, 표는 머리표를
    /// 품은 채로 줄을 맞춘다. 지금 브랜치의 줄에는 아무것도 안 붙는다.
    #[test]
    fn a_line_from_another_branch_carries_its_mark_before_the_title() {
        let mine = vec![issue("argos-0001", "여기 일", "todo")];
        let theirs = vec![issue("argos-0002", "옆 일", "in_progress")];
        let (all, origin) = crate::worktree::overlay(
            mine,
            vec![("feat/x".into(), std::path::PathBuf::from("/wt"), theirs)],
        );
        let tagged: BTreeMap<&str, String> = all.iter().map(|i| (i.id.as_str(), "에픽".to_string())).collect();
        let out = plain(&list(&all, &cfg(), Hidden::default(), &tagged, false, &Default::default(), &origin));
        assert!(out[1].contains("여기 일") && !out[1].contains('⎇'), "{out:#?}");
        assert!(out[2].contains("⎇ feat/x 옆 일"), "{out:#?}");
        let col = |l: &str| width(&l[..l.find("에픽").unwrap()]);
        assert_eq!(col(&out[1]), col(&out[2]), "머리표가 다음 열을 밀었다\n{out:#?}");
        // 색을 켜면 머리표는 제 색을 입는다.
        let painted = list(&all, &cfg(), Hidden::default(), &tagged, false, &Default::default(), &origin);
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

    /// 그린 줄은 폭을 넘지 않는다. 한글이 두 칸이라 글자 수로 세면 걸린다.
    ///
    /// 상한은 `BODY` 에 들여쓰기(`PAD`)를 더한 값이다. 여유를 더 주면 그만큼
    /// 넘치는 줄을 통과시킨다.
    #[test]
    fn drawn_lines_stay_within_the_width() {
        let body = "아주 긴 한글 문장이 폭을 넘도록 이어지고 또 이어지고 계속 이어진다. \
                    여기에 `코드` 와 **굵게** 도 섞여 있어서 접는 자리가 조각 가운데에 걸린다.\n";
        let max = BODY + width(PAD);
        for l in plain(&body_lines(body)) {
            assert!(width(&l) <= max, "{l:?} ({}칸)", width(&l));
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
            let out = plain(&detail(&i, None, &[], &Seen::default(), &cfg(), "2026-09-11T04:12:03Z",raw)).join("\n");
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
        let mut lines = detail(&i, None, &[], &Seen::default(), &cfg(), "2026-09-11T04:12:03Z", false);
        lines.extend(history(&j, &cfg()));
        let out = plain(&lines);
        let joined = out.join("\n");
        assert!(joined.contains("첫 줄") && joined.contains("둘째 줄"), "{joined}");
        assert!(joined.contains("이력"), "{joined}");
        assert!(joined.contains("todo → in_progress"), "{joined}");
        assert!(joined.contains("09-09 14:02"), "{joined}");
    }

    /// 목록의 에픽 열은 id 가 아니라 제목이다. id 를 보여 주면 사람이
    /// 그걸 다시 찾아봐야 한다.
    #[test]
    fn the_epic_column_shows_a_title() {
        let mut i = issue("argos-0002", "멤버", "todo");
        i.epic = Some("argos-0001".into());
        let labels = BTreeMap::from([("argos-0002", "저장 계층".to_string())]);
        let out = plain(&list(&[i.clone()], &cfg(), Hidden::default(), &labels, false, &Default::default(), &Origin::default()));
        assert!(out[1].contains("저장 계층") && !out[1].contains("argos-0001"), "{out:#?}");

        // 없는 에픽을 가리켜도 죽지 않고 그렇다고 말한다
        let dangling = BTreeMap::from([("argos-0002", "(없는 에픽)".to_string())]);
        let out = plain(&list(&[i], &cfg(), Hidden::default(), &dangling, false, &Default::default(), &Origin::default()));
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

        let wide = plain(&list(&all, &cfg(), Hidden::default(), &no_epics(), false, &Default::default(), &Origin::default()));
        assert!(wide[1].contains("미룸"), "미룬 줄이 일과 똑같이 보인다 — {wide:#?}");
        assert!(wide[2].contains("미룸"), "{wide:#?}");

        // 콕 집어 물었을 때는 줄마다 같은 낱말을 달지 않는다.
        let asked = plain(&list(&all, &cfg(), Hidden::default(), &no_epics(), true, &Default::default(), &Origin::default()));
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
        let out = plain(&tree(&all, &index, &keep, &rolls, &Origin::default()).0);
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
        let out = plain(&tree(&all, &index, &|_| true, &rolls, &Origin::default()).0);
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
            let out = plain(&tree(&all, &index, &|_| true, &rolls, &Origin::default()).0);
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
        assert_eq!(plain(&tree(&all, &index, &|_| false, &rolls, &Origin::default()).0), ["없다."]);
        assert!(plain(&tree(&all, &index, &|_| true, &rolls, &Origin::default()).0).join("\n").contains("빈 에픽"));
    }

    /// **다 끝난 묶음은 재촉하지 않고, 접은 묶음은 그렇다고 말한다.** 묶음의 칸은
    /// 멤버에서 읽으므로 100% 면 곧 닫힌 것이라 시킬 말이 없다 — 적힌 칸을 옮기라고
    /// 시키면 어디서도 안 읽히는 칸을 쓰게 한다. 남은 멤버를 미뤄 접은 묶음은 막대가
    /// `1/2` 인 채로 닫혀 있어, 말하지 않으면 굴러가는 묶음과 똑같아 보인다.
    #[test]
    fn a_finished_grouping_is_not_nagged_but_a_folded_one_says_so() {
        let table = |all: &[Issue]| {
            let cfg = cfg();
            let st = crate::report::status(all, &[], &cfg, "2026-09-11T04:12:03Z");
            plain(&status(&st, all, &cfg, "2026-09-11T04:12:03Z", ".moai/issues.jsonl", &Origin::default())).join("\n")
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
        let labels = BTreeMap::from([("argos-0002", "저장 계층".to_string())]);

        let out = plain(&ready(&[&a, &b], &labels, &[&wip], &[], &Origin::default()));
        let joined = out.join("\n");
        assert!(joined.contains("2건"), "{joined}");
        assert!(joined.contains("저장 계층") && joined.contains("에픽 없음"), "{joined}");
        assert!(joined.contains("이미 잡고 있는 것 1건"), "{joined}");
        assert!(joined.contains("argos-0004"), "{joined}");

        let empty = plain(&ready(&[], &labels, &[], &[], &Origin::default())).join("\n");
        assert!(empty.contains("0건") && empty.contains("무엇이 밀려 있는지"), "{empty}");
    }

    /// 없는 에픽을 가리켜도 상세가 죽지 않는다 — 드러내되 막지 않는다.
    #[test]
    fn a_dangling_epic_is_shown_not_fatal() {
        let mut i = issue("argos-0001", "제목", "todo");
        i.epic = Some("argos-0000".into());
        let out = plain(&detail(&i, None, &[], &Seen::default(), &cfg(), "2026-09-11T04:12:03Z",false));
        assert!(out.iter().any(|l| l.contains("(없는 에픽)")), "{out:#?}");
    }
}
