//! 거름망 칸(`SPC f`)의 안내 — 쓸 수 있는 항목과 예, 그리고 값을 고르는 목록(moai-h2rh).
//!
//! **위쪽은 터미널도 저장소도 모른다.** 친 글과 커서만 받아 커서가 선 값 자리([`Slot`])를 읽고, 건넨 값([`Offer`])을
//! 친 글로 좁힌다. 값 자리를 읽는 데는 거름망과 한 벌인 쪼개는 자(`query::items`)를 빌린다 — 그래서 조각의 허락
//! 목록(`input::foreign`)으로 재면 조각이 아니다(리뷰 moai-mkyg.n60). 아래 `impl App` 이 저장소의 줄에서 고를 값을
//! 모으고 키를 받는다 — 줄(`crate::model`)을 읽으므로 이 파일은 조각 목록(`input::NOT_COMPONENTS`)에 든다. `zones.rs`
//! 와 같은 꼴이다.
//!
//! 사용자가 정한 것(2026-10-03)
//!
//! - **자리는 입력 칸 위의 패널이다.** SPC 메뉴처럼 목록을 밀어 올리고 덮지 않는다. 커서가 값 자리
//!   (`assignee=`·`tag=`·`milestone=`)에 서면 값 목록, 아니면 쓸 수 있는 항목과 예다
//! - **위·아래 화살표로 옮기고 Enter 가 넣는다.** 값 목록이 선 동안 Enter 는 값을 넣고, 안 선 때만 거름망을 건다
//! - 사람은 `이름 (메일)` 로 보이고 메일을 넣는다. 메일이 없으면 이름이다
//! - 친 값의 부분 일치로 좁힌다. 대소문자는 안 가린다
//!
//! **글을 쪼개는 자는 `query` 의 것 하나다**(`query::items`, moai-mkyg.dcf) — 빈칸으로 가른 낱말 가운데 `=` 가 든
//! 낱말이 새 항목을 열고, `=` 바로 뒤의 따옴표(`"…"`·`'…'`)는 닫힐 때까지 한 값이다. 거름망이 읽는 글과 같은 걸음이
//! 항목마다 선 자리를 함께 내고, 여기서는 그 자리로 커서가 선 값만 읽는다.

use super::input::Input;
use std::ops::Range;

/// 패널이 세우는 줄 수의 위. 항목과 예가 80칸에서 이만큼 든다 — 더 받으면 목록이 그만큼 준다.
pub const ROWS: usize = 6;

/// 항목 안내 밑에 서는 예. **예는 여기 하나에 둔다** — 문법이 바뀌면 이 줄만 고친다. 시험이 하나하나를 거르개에
/// 걸어 본다(`every_example_is_a_filter_the_explorer_takes`).
pub const EXAMPLES: &[&str] = &[
    "status=todo,review priority=p0,p1 tag=bug no-tag=docs",
    "assignee=me assignee=ana@example.com",
    "done_at=\"2026-10-03 00:00~2026-10-05 23:59\"",
    "created_at=2026-10-02 started_at=2026-10-01~",
];

/// 저장소에서 모으거나 정해진 값으로 완성하는 항목.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Assignee,
    Tag,
    Milestone,
    Status,
    Priority,
    Type,
}

impl Field {
    /// 항목 이름의 값 목록. **`no-tag` 도 태그 이름을 받는다.**
    pub fn of(key: &str) -> Option<Field> {
        match key {
            "assignee" => Some(Field::Assignee),
            "tag" | "no-tag" => Some(Field::Tag),
            "milestone" => Some(Field::Milestone),
            "status" => Some(Field::Status),
            "priority" => Some(Field::Priority),
            "type" => Some(Field::Type),
            _ => None,
        }
    }
}

/// 커서가 선 값 자리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot<'a> {
    /// `=` 앞의 항목 이름.
    pub key: &'a str,
    pub field: Field,
    /// 커서 앞까지 친 값 — 여는 따옴표와 앞의 쉼표 값은 뺀 것. 좁히는 데 쓴다.
    pub typed: &'a str,
    /// 고른 값으로 갈아 끼울 자리. 이 값이 따옴표로 열렸으면 여는 따옴표부터 닫는 따옴표까지다.
    pub span: Range<usize>,
    /// 갈아 끼울 자리가 `=` 바로 뒤에서 시작한다 — **고른 값을 따옴표로 묶을 수 있는 자리는 여기뿐이다**(문법이
    /// `=` 바로 뒤의 따옴표만 읽는다). 쉼표 뒤의 값도, 이미 연 따옴표 안의 값도 아니다 — 따옴표 안에서 또 묶으면
    /// `assignee=""Kim Lee",bob"` 처럼 짝이 깨진다.
    pub first: bool,
}

/// `text` 의 `at`(바이트) 자리에 선 커서가 **값 목록이 서는 항목의 값**을 적고 있으면 그 자리.
///
/// 값 자리는 `항목=` 이 든 낱말 안이거나, 그 값이 따옴표로 열렸으면 닫힐 때까지다. **따옴표 없이 빈칸 뒤에 이어
/// 적은 낱말은 아니다** — 문법은 그것을 앞 값에 붙여 읽지만, 거기서 치는 것은 대개 다음 항목이다. 거기서 목록이
/// 서면 `tag=bug st` 의 Enter 가 거름망을 걸지 않고 값을 넣는다.
///
/// **닫지 않은 따옴표의 갈 자리는 커서가 선 낱말까지다.** 문법으로는 줄 끝까지 한 값이지만, 줄 끝까지 갈면 커서
/// 뒤에 이미 친 항목이 고른 값에 먹힌다 — `assignee="Ki| tag=bug` 에서 고르면 `tag=bug` 가 사라졌다.
pub fn slot(text: &str, at: usize) -> Option<Slot<'_>> {
    let (item, eq) = crate::query::items(text)
        .into_iter()
        .filter_map(|it| it.eq.map(|eq| (it, eq)))
        .find(|(it, eq)| *eq < at && at <= it.end)?;
    let key = &text[item.start..eq];
    let field = Field::of(key)?;
    let open = eq + 1;
    // 따옴표의 자리는 쪼개는 자가 댄 것을 읽는다 — 여기서 `=` 로 셈하면 문법이 둘이 된다.
    let inner = item.quote.map_or(open, |(q, _)| q + 1);
    let at = at.max(inner);
    // 따옴표 안의 글 — 닫히지 않았으면 커서가 선 낱말의 끝까지.
    let inner_end = match item.quote {
        Some((_, Some(close))) => close,
        Some((_, None)) => text[at..].find(char::is_whitespace).map_or(text.len(), |p| at + p),
        None => item.end,
    };
    if at > inner_end {
        return None; // 닫는 따옴표 뒤 — 값은 끝났다
    }
    // 쉼표 뒤의 값만 본다 — 쉼표는 또는이다.
    let seg = text[inner..at].rfind(',').map_or(inner, |p| inner + p + 1);
    let end = text[at..inner_end].find(',').map_or(inner_end, |p| at + p);
    // 따옴표로 연 값을 통째로 갈 때만 따옴표까지 간다 — 반만 걷으면 짝이 깨진다.
    let span = match item.quote {
        Some((q, close)) if seg == inner && end == inner_end => q..close.map_or(inner_end, |c| c + 1),
        _ => seg..end,
    };
    let first = span.start == open;
    Some(Slot { key, field, typed: &text[seg..at], span, first })
}

/// 고를 값 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// 목록에 서는 글 — 사람은 `이름 (메일)`, 마일스톤은 `id  제목`.
    pub shown: String,
    /// 칸에 넣는 값.
    pub put: String,
    /// 곁에 흐리게 서는 말 — `me`·`none` 처럼 글만으로는 뜻이 안 읽히는 값에. 화면의 말로 이미 옮긴 글이다.
    pub note: Option<&'static str>,
}

/// 친 값으로 좁힌다 — 보이는 글이나 넣을 값에 든 것. **친 값이 이미 어느 값과 같으면 아무것도 안 낸다** — 다 친
/// `assignee=me` 에서 Enter 가 거름망을 걸어야 한다. 목록이 서 있으면 Enter 는 값을 넣는다.
pub fn narrow(offers: Vec<Offer>, typed: &str) -> Vec<Offer> {
    let typed = typed.trim().to_lowercase();
    if !typed.is_empty() && offers.iter().any(|o| o.put.to_lowercase() == typed) {
        return Vec::new();
    }
    offers
        .into_iter()
        .filter(|o| o.shown.to_lowercase().contains(&typed) || o.put.to_lowercase().contains(&typed))
        .collect()
}

/// 고른 값을 칸에 넣을 글. 빈칸·`=`·쉼표가 든 값은 따옴표로 묶는다 — `=` 바로 뒤일 때만 묶을 수 있다. **따옴표로
/// 시작하는 값도 묶는다** — 맨몸으로 `=` 뒤에 서면 문법이 그 따옴표를 여는 따옴표로 읽어 벗긴다.
///
/// 쉼표는 묶어도 값 하나로 남지 않는다 — 문법이 따옴표를 벗긴 뒤에 쉼표로 가른다(또는). 묶는 것은 빈칸과 함께
/// 든 쉼표가 낱말을 흩지 않게 할 뿐이다.
pub fn quoted(put: &str, first: bool) -> String {
    let needs = put.starts_with(['"', '\'']) || put.chars().any(|c| c.is_whitespace() || c == '=' || c == ',');
    match (needs && first, put.contains('"')) {
        (false, _) => put.to_string(),
        (true, false) => format!("\"{put}\""),
        (true, true) => format!("'{put}'"),
    }
}

/// 값 목록에서 겨눈 줄. **겨눈 그 글과 커서에서만 선다** — 고른 값을 넣어 글이 바뀌면 첫 줄로 돌아간다.
///
/// 글과 커서만으로는 모자라다 — 칸을 닫았다 다시 열어 같은 `tag=` 를 치거나, 한 자 쳤다 지우면 같은 글과 커서로
/// 돌아와 옛 줄이 되살아난다. 그래서 칸이 키나 붙여 넣기를 먹을 때마다 [`App`] 이 이것을 걷는다(`typing`·`paste`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aim {
    text: String,
    at: usize,
    row: usize,
}

impl Aim {
    fn set(&mut self, input: &Input, row: usize) {
        *self = Aim { text: input.text().to_string(), at: input.cursor(), row };
    }
    /// 줄 `n` 개인 목록에서 겨눈 줄.
    pub fn row(&self, input: &Input, n: usize) -> usize {
        if self.text == input.text() && self.at == input.cursor() { self.row.min(n.saturating_sub(1)) } else { 0 }
    }

    /// 한 줄 옮긴다. 끝에서는 멈춘다 — 목록의 커서와 같다.
    pub fn step(&mut self, input: &Input, n: usize, down: bool) {
        let row = self.row(input, n);
        let row = if down { (row + 1).min(n.saturating_sub(1)) } else { row.saturating_sub(1) };
        *self = Aim { text: input.text().to_string(), at: input.cursor(), row };
    }
}

/// One Tab cycle retains the original candidates while its replacement changes the input.
#[derive(Debug, Clone)]
pub(super) struct Completion {
    text: String,
    at: usize,
    span: Range<usize>,
    value: Option<(String, Field, bool)>,
    offers: Vec<Offer>,
    row: usize,
}

impl Completion {
    fn matches(&self, q: &Input) -> bool {
        self.text == q.text() && self.at == q.cursor()
    }
}

/// Key names use the query table's order. Quoted values never become key slots.
fn key_span(text: &str, at: usize) -> Option<Range<usize>> {
    if crate::query::items(text).iter().any(|i| i.eq.is_some_and(|eq| eq < at && at <= i.end)) {
        return None;
    }
    let start = text[..at].rfind(char::is_whitespace).map_or(0, |p| p + text[p..].chars().next().unwrap().len_utf8());
    let end = text[at..].find(|c: char| c.is_whitespace() || c == '=').map_or(text.len(), |p| at + p);
    Some(start..end)
}

/// `rows` 줄 창에 겨눈 줄 `row` 가 들게 굴린 첫 줄. 겨눈 줄이 창 밑으로 내려가면 그 줄을 맨 밑에 둔다.
pub fn window(row: usize, rows: usize) -> usize {
    (row + 1).saturating_sub(rows)
}

use super::{App, Mode};
use crate::i18n::say;
use crate::model::{Issue, Kind};

impl App {
    pub(super) fn completing_value(&self) -> bool {
        let Mode::Filter(q) = &self.mode else { return false };
        self.completion.as_ref().is_some_and(|c| c.matches(q) && c.value.is_some())
    }
    pub(super) fn completed_key(&self) -> Option<&str> {
        let Mode::Filter(q) = &self.mode else { return None };
        let c = self.completion.as_ref().filter(|c| c.matches(q) && c.value.is_none())?;
        Some(&c.offers[c.row].shown)
    }
    /// 거름망 칸의 커서가 선 값 자리와 고를 값. **목록이 안 서면 `None`** 이다 — 값 자리가 아니거나, 좁혀 남은
    /// 것이 없거나, 친 값이 이미 어느 값과 같거나(Tab 순환 중은 원래 후보를 보여 준다), **창이 낮아 안내 칸이 한 줄도 못 서면**(`hint_room`). 안 보이는
    /// 목록이 Enter 를 먹으면 거름망을 걸려던 사람의 칸에 보이지 않던 값이 들어간다.
    pub(super) fn offered(&self) -> Option<(Slot<'_>, Vec<Offer>)> {
        let Mode::Filter(q) = &self.mode else { return None };
        if self.hint_room == Some(0) {
            return None;
        }
        if let Some(c) = &self.completion
            && c.matches(q)
            && let Some((key, field, first)) = &c.value
        {
            return Some((
                Slot { key, field: *field, typed: "", span: c.span.clone(), first: *first },
                c.offers.clone(),
            ));
        }
        let slot = slot(q.text(), q.cursor())?;
        // Enter retains its existing meaning on fixed fields; Tab opens their cycle.
        if matches!(slot.field, Field::Status | Field::Priority | Field::Type) {
            return None;
        }
        let offers = narrow(offers(slot.field, &self.site.issues, self.site.lang), slot.typed);
        (!offers.is_empty()).then_some((slot, offers))
    }

    /// Insert a candidate and wrap through the original list on subsequent Tabs.
    pub(super) fn complete_filter(&mut self, forward: bool) {
        let Mode::Filter(q) = &self.mode else { return };
        let continuing = self.completion.as_ref().is_some_and(|c| c.matches(q));
        let mut c = if continuing {
            let mut c = self.completion.take().unwrap();
            c.row = if forward { (c.row + 1) % c.offers.len() } else { (c.row + c.offers.len() - 1) % c.offers.len() };
            c
        } else if let Some(sl) = slot(q.text(), q.cursor()) {
            let values = match sl.field {
                Field::Status => self.site.cfg.statuses.clone(),
                Field::Priority => (0..=3).map(|p| format!("p{p}")).collect(),
                Field::Type => [Kind::Issue, Kind::Epic, Kind::Milestone, Kind::Idea]
                    .into_iter()
                    .map(|k| k.as_str().to_string())
                    .collect(),
                _ => Vec::new(),
            };
            let all = if values.is_empty() {
                offers(sl.field, &self.site.issues, self.site.lang)
            } else {
                values.into_iter().map(|put| Offer { shown: put.clone(), put, note: None }).collect()
            };
            let offers = narrow(all, sl.typed);
            if offers.is_empty() {
                return;
            }
            let row = self.offer_aim.row(q, offers.len());
            let row = if forward { row } else { offers.len() - 1 };
            Completion {
                text: String::new(),
                at: 0,
                span: sl.span.clone(),
                value: Some((sl.key.into(), sl.field, sl.first)),
                offers,
                row,
            }
        } else {
            let Some(mut span) = key_span(q.text(), q.cursor()) else { return };
            let prefix = &q.text()[span.start..q.cursor()];
            let names: Vec<_> = crate::query::KEYS.iter().filter(|k| k.starts_with(prefix)).collect();
            if names.is_empty() {
                return;
            }
            let single = names.len() == 1;
            if single && q.text().as_bytes().get(span.end) == Some(&b'=') {
                span.end += 1;
            }
            let offers: Vec<_> = names
                .into_iter()
                .map(|k| Offer { shown: (*k).into(), put: format!("{k}{}", if single { "=" } else { "" }), note: None })
                .collect();
            let row = if forward { 0 } else { offers.len() - 1 };
            Completion { text: String::new(), at: 0, span, value: None, offers, row }
        };
        let text = quoted(&c.offers[c.row].put, c.value.as_ref().is_some_and(|v| v.2));
        let Mode::Filter(q) = &mut self.mode else { return };
        let start = c.span.start;
        q.splice(c.span.clone(), &text);
        c.span = start..q.cursor();
        c.text = q.text().into();
        c.at = q.cursor();
        self.offer_aim.set(q, c.row);
        // A unique key ends this cycle so the next Tab can start completing its value.
        self.completion = if c.value.is_none() && c.offers.len() == 1 { None } else { Some(c) };
    }

    /// 값 목록의 겨눈 줄을 옮긴다. 목록이 안 섰으면 아무것도 안 한다.
    pub(super) fn aim_offer(&mut self, down: bool) {
        let Some(n) = self.offered().map(|(_, o)| o.len()) else { return };
        if let Mode::Filter(q) = &self.mode {
            self.offer_aim.step(q, n, down);
        }
    }

    /// 겨눈 값을 칸에 넣는다. **넣었으면 참이다** — 목록이 안 섰으면 거짓이고 Enter 는 거름망을 건다.
    ///
    /// 값이 칸 끝에 서면 빈칸 하나를 붙인다 — 다음 항목을 바로 치고, 목록이 닫혀 다음 Enter 가 거름망을 건다.
    pub(super) fn put_offer(&mut self) -> bool {
        let Some((slot, offers)) = self.offered() else { return false };
        let Mode::Filter(q) = &self.mode else { return false };
        let picked = &offers[self.offer_aim.row(q, offers.len())];
        let (span, text) = (slot.span.clone(), quoted(&picked.put, slot.first));
        let Mode::Filter(q) = &mut self.mode else { return false };
        q.splice(span, &text);
        if q.cursor() == q.text().len() {
            q.insert(" ");
        }
        true
    }
}

/// 항목의 값 목록 — 저장소의 줄에서 모은다.
///
/// - 담당: `me`·`none` 다음에 담당으로 선 사람, `이름 (메일)` 의 차례. **같은 사람은 메일로 하나다** — 메일이 없으면
///   이름으로. 화면 모양(`naming`)을 안 따른다 — 이름만 보이면 이름이 같은 두 사람을 못 가른다(사용자 결정)
/// - 태그: 줄에 선 태그, 이름의 차례
/// - 마일스톤: `none` 다음에 마일스톤 줄, 새로 만든 것부터 — 값은 id 다
fn offers(field: Field, issues: &[Issue], lang: crate::i18n::Lang) -> Vec<Offer> {
    let word = |s: &str, note: &'static str| Offer { shown: s.to_string(), put: s.to_string(), note: Some(note) };
    match field {
        Field::Assignee => {
            // 열쇠는 거름망이 사람을 가르는 자와 같다(`query::is_assignee`) — 메일은 대소문자를 접고 이름은 글자
            // 그대로다. 이름까지 접으면 `Anna` 와 `anna` 가 한 줄로 서고, 그 줄의 값은 둘 중 하나만 고른다.
            let mut people = std::collections::BTreeMap::new();
            for i in issues {
                let Some(name) = i.assignee.as_deref().filter(|n| !n.trim().is_empty()) else { continue };
                let email = i.assignee_email.as_deref().map(str::trim).filter(|e| !e.is_empty());
                let key = match email {
                    Some(e) => (true, e.to_lowercase()),
                    None => (false, name.to_string()),
                };
                people.entry(key).or_insert_with(|| Offer {
                    shown: crate::model::label(name, email, crate::config::Naming::Full),
                    put: email.unwrap_or(name).to_string(),
                    note: None,
                });
            }
            let mut people: Vec<Offer> = people.into_values().collect();
            people.sort_by_cached_key(|o| o.shown.to_lowercase());
            [word("me", say(lang, "tui.hint.me")), word("none", say(lang, "tui.hint.nobody"))]
                .into_iter()
                .chain(people)
                .collect()
        }
        Field::Tag => {
            // **거름망이 태그를 재는 자(`model::normalize_tag`)로 접는다** — 읽기는 정규화를 안 거쳐, 손으로 푼 머지의
            // `Bug`·`#bug` 가 그대로 온다. 안 접으면 거름망에서는 한 태그인 것이 목록에 셋으로 선다.
            let tags: std::collections::BTreeSet<String> = issues
                .iter()
                .flat_map(|i| i.tags.iter().map(|t| crate::model::normalize_tag(t)))
                .filter(|t| !t.is_empty())
                .collect();
            tags.into_iter().map(|t| Offer { shown: t.clone(), put: t, note: None }).collect()
        }
        Field::Milestone => {
            let mut stones: Vec<&Issue> = issues.iter().filter(|i| i.kind == Kind::Milestone).collect();
            stones.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| a.id.cmp(&b.id)));
            let stones = stones.into_iter().map(|i| Offer {
                shown: format!("{}  {}", i.id, crate::text::one_line(&i.title)),
                put: i.id.clone(),
                note: None,
            });
            std::iter::once(word("none", say(lang, "tui.hint.no_milestone"))).chain(stones).collect()
        }
        Field::Status | Field::Priority | Field::Type => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    /// `|` 가 커서다.
    fn at(s: &str) -> (String, usize) {
        let at = s.find('|').expect("커서가 없다");
        (s.replacen('|', "", 1), at)
    }

    fn slot_of(s: &str) -> Option<(String, Field, String, String, bool)> {
        let (text, at) = at(s);
        slot(&text, at)
            .map(|sl| (sl.key.to_string(), sl.field, sl.typed.to_string(), text[sl.span.clone()].to_string(), sl.first))
    }

    #[test]
    fn the_slot_is_the_value_of_a_key_that_lists() {
        assert_eq!(slot_of("assignee=|"), Some(("assignee".into(), Field::Assignee, "".into(), "".into(), true)));
        assert_eq!(
            slot_of("tag=bug assignee=ra|"),
            Some(("assignee".into(), Field::Assignee, "ra".into(), "ra".into(), true))
        );
        // 커서가 값 가운데 서면 친 것은 커서 앞까지, 갈 자리는 값 전부다.
        assert_eq!(
            slot_of("tag=bu|gfix status=todo"),
            Some(("tag".into(), Field::Tag, "bu".into(), "bugfix".into(), true))
        );
        assert_eq!(slot_of("no-tag=d|"), Some(("no-tag".into(), Field::Tag, "d".into(), "d".into(), true)));
        assert_eq!(slot_of("milestone=|"), Some(("milestone".into(), Field::Milestone, "".into(), "".into(), true)));
    }

    #[test]
    fn no_slot_where_the_key_has_no_list_or_the_cursor_is_not_in_a_value() {
        for s in ["|", "sta|", "grep=to|", "epic=p|", "tag|=bug", "tag=bug |", "tag=bug st|", "assignee=a b|"] {
            assert_eq!(slot_of(s), None, "{s}");
        }
    }

    /// 쉼표 뒤의 값만 좁히고 그 값만 간다 — 쉼표는 또는이다.
    #[test]
    fn a_comma_starts_the_next_value() {
        assert_eq!(slot_of("tag=bug,ui|"), Some(("tag".into(), Field::Tag, "ui".into(), "ui".into(), false)));
        assert_eq!(slot_of("tag=bug,u|i,docs"), Some(("tag".into(), Field::Tag, "u".into(), "ui".into(), false)));
        assert_eq!(slot_of("tag=b|ug,ui"), Some(("tag".into(), Field::Tag, "b".into(), "bug".into(), true)));
    }

    /// `=` 바로 뒤의 따옴표는 닫힐 때까지 한 값이다 — 빈칸이 들어도 값 자리이고, 갈 때는 따옴표까지 간다.
    #[test]
    fn a_quoted_value_is_one_slot_up_to_its_closing_quote() {
        assert_eq!(
            slot_of("assignee=\"Kim L|"),
            Some(("assignee".into(), Field::Assignee, "Kim L".into(), "\"Kim L".into(), true))
        );
        assert_eq!(
            slot_of("assignee='Kim |Lee' tag=x"),
            Some(("assignee".into(), Field::Assignee, "Kim ".into(), "'Kim Lee'".into(), true))
        );
        assert_eq!(slot_of("assignee=\"Kim Lee\" |"), None);
        // 따옴표 안의 `=` 는 새 항목이 아니다.
        assert_eq!(slot_of("grep=\"tag=x\" tag=|"), Some(("tag".into(), Field::Tag, "".into(), "".into(), true)));
        assert_eq!(slot_of("grep=\"a tag=|"), None, "따옴표 안의 tag= 가 항목으로 읽혔다");
    }

    /// **이미 연 따옴표 안에서는 다시 묶지 않는다** — 쉼표 앞의 첫 값만 갈 때 `first` 가 서면 `""Kim Lee",bob"` 처럼
    /// 짝이 깨진다. 묶을 수 있는 것은 갈 자리가 `=` 바로 뒤에서 시작할 때뿐이다.
    #[test]
    fn a_value_inside_an_open_quote_is_not_quoted_again() {
        assert_eq!(
            slot_of("assignee=\"Ki|,bob\""),
            Some(("assignee".into(), Field::Assignee, "Ki".into(), "Ki".into(), false))
        );
        assert_eq!(
            slot_of("assignee=\"bob,Ki|\""),
            Some(("assignee".into(), Field::Assignee, "Ki".into(), "Ki".into(), false))
        );
        // 따옴표 없는 첫 값은 쉼표가 뒤에 있어도 `=` 바로 뒤라 묶을 수 있다.
        assert_eq!(
            slot_of("assignee=Ki|,bob"),
            Some(("assignee".into(), Field::Assignee, "Ki".into(), "Ki".into(), true))
        );
    }

    /// **닫지 않은 따옴표의 갈 자리는 커서가 선 낱말까지다** — 줄 끝까지 가면 커서 뒤에 친 항목이 고른 값에 먹힌다.
    #[test]
    fn an_unclosed_quote_does_not_swallow_what_follows_the_cursor() {
        assert_eq!(
            slot_of("assignee=\"Ki| tag=bug"),
            Some(("assignee".into(), Field::Assignee, "Ki".into(), "\"Ki".into(), true))
        );
        assert_eq!(
            slot_of("assignee=\"Kim L|e tag=bug"),
            Some(("assignee".into(), Field::Assignee, "Kim L".into(), "\"Kim Le".into(), true))
        );
    }

    fn offer(shown: &str, put: &str) -> Offer {
        Offer { shown: shown.into(), put: put.into(), note: None }
    }

    #[test]
    fn narrowing_matches_anywhere_and_ignores_case() {
        let all =
            vec![offer("Raven (raven@x.io)", "raven@x.io"), offer("joseph (jo@x.io)", "jo@x.io"), offer("Kim", "Kim")];
        let puts = |typed: &str| narrow(all.clone(), typed).into_iter().map(|o| o.put).collect::<Vec<_>>();
        assert_eq!(puts(""), ["raven@x.io", "jo@x.io", "Kim"]);
        assert_eq!(puts("RAV"), ["raven@x.io"]);
        assert_eq!(puts("x.io"), ["raven@x.io", "jo@x.io"]);
        assert_eq!(puts("zz"), Vec::<String>::new());
        // 다 친 값이면 목록이 안 선다 — Enter 가 거름망을 건다.
        assert_eq!(puts("kim"), Vec::<String>::new());
        assert_eq!(puts("jo@x.io "), Vec::<String>::new());
    }

    #[test]
    fn only_a_value_right_after_the_equals_sign_is_quoted() {
        assert_eq!(quoted("raven@x.io", true), "raven@x.io");
        assert_eq!(quoted("Kim Lee", true), "\"Kim Lee\"");
        assert_eq!(quoted("say \"hi\"", true), "'say \"hi\"'");
        assert_eq!(quoted("Kim Lee", false), "Kim Lee");
        // 따옴표로 시작하는 값은 맨몸이면 문법이 여는 따옴표로 읽어 벗긴다.
        assert_eq!(quoted("'Bob'", true), "\"'Bob'\"");
        assert_eq!(quoted("\"Bob\"", true), "'\"Bob\"'");
    }

    #[test]
    fn the_aim_stands_only_on_the_text_and_cursor_it_was_taken_on() {
        let q = Input::new("tag=");
        let mut aim = Aim::default();
        assert_eq!(aim.row(&q, 3), 0);
        aim.step(&q, 3, true);
        aim.step(&q, 3, true);
        aim.step(&q, 3, true);
        assert_eq!(aim.row(&q, 3), 2, "끝에서 멈춘다");
        aim.step(&q, 3, false);
        assert_eq!(aim.row(&q, 3), 1);
        assert_eq!(aim.row(&q, 1), 0, "목록이 줄면 안으로 당긴다");
        assert_eq!(aim.row(&Input::new("tag=b"), 3), 0, "글이 바뀌면 첫 줄이다");
    }

    #[test]
    fn the_window_follows_the_aimed_row() {
        assert_eq!(window(0, 3), 0);
        assert_eq!(window(2, 3), 0);
        assert_eq!(window(3, 3), 1);
        assert_eq!(window(9, 3), 7);
    }

    fn row(id: &str, kind: Kind) -> Issue {
        Issue::new(id.into(), format!("{id} 제목"), kind, crate::model::Status::new("todo"), "2026-09-01T00:00:00Z")
    }

    #[test]
    fn people_are_listed_once_by_email_with_me_and_none_first() {
        let mut a = row("t-1", Kind::Issue);
        (a.assignee, a.assignee_email) = (Some("레이븐".into()), Some("raven@x.io".into()));
        let mut b = row("t-2", Kind::Issue);
        (b.assignee, b.assignee_email) = (Some("Raven".into()), Some("RAVEN@x.io".into()));
        let mut c = row("t-3", Kind::Issue);
        c.assignee = Some("anna".into());
        let d = row("t-4", Kind::Issue);
        // 메일 없는 이름은 글자 그대로 가른다 — 거름망(`query::is_assignee`)이 이름을 대소문자째 잰다.
        let mut e = row("t-5", Kind::Issue);
        e.assignee = Some("Anna".into());
        let got: Vec<(String, String)> =
            offers(Field::Assignee, &[a, b, c, d, e], Lang::En).into_iter().map(|o| (o.shown, o.put)).collect();
        assert_eq!(got.len(), 5, "{got:?}");
        assert_eq!(got[..2], [("me".into(), "me".into()), ("none".into(), "none".into())]);
        assert!(got.contains(&("anna".into(), "anna".into())), "{got:?}");
        assert!(got.contains(&("Anna".into(), "Anna".into())), "이름만 다른 대소문자가 한 줄로 접혔다: {got:?}");
        assert_eq!(got[4], ("레이븐 (raven@x.io)".into(), "raven@x.io".into()));
    }

    #[test]
    fn tags_are_listed_once_and_milestones_newest_first_by_id() {
        let mut a = row("t-1", Kind::Issue);
        a.tags = vec!["ui".into(), "bug".into()];
        let mut b = row("t-2", Kind::Issue);
        b.tags = vec!["bug".into()];
        // 읽기는 정규화를 안 거친다 — 손으로 푼 머지의 `Bug`·`#bug` 도 거름망에서는 한 태그다.
        let mut c = row("t-3", Kind::Issue);
        c.tags = vec!["Bug".into(), "#bug".into(), " ".into()];
        let tags: Vec<String> = offers(Field::Tag, &[a, b, c], Lang::En).into_iter().map(|o| o.put).collect();
        assert_eq!(tags, ["bug", "ui"]);

        let old = row("t-9", Kind::Milestone);
        let mut new = row("t-5", Kind::Milestone);
        new.created_at = "2026-10-01T00:00:00Z".into();
        let got: Vec<(String, String)> = offers(Field::Milestone, &[old, new, row("t-1", Kind::Epic)], Lang::En)
            .into_iter()
            .map(|o| (o.shown, o.put))
            .collect();
        assert_eq!(
            got,
            [
                ("none".into(), "none".into()),
                ("t-5  t-5 제목".into(), "t-5".into()),
                ("t-9  t-9 제목".into(), "t-9".into())
            ]
        );
    }

    /// 담당 셋(레이븐·joseph·메일 없는 Kim Lee), 태그 셋(bug·docs·ui), 마일스톤 하나인 탐색기.
    ///
    /// **보는 사람을 박아 둔다** — 예의 `assignee=me` 는 사람을 푼다. 안 박으면 사람을 모르는 기계(CI)에서 `me` 를
    /// 거절해 예 시험이 붉어지고, 아는 기계에서는 시험이 그 사람을 묻는 프로세스를 띄운다.
    fn explorer() -> App {
        let mut a = row("argos-0001", Kind::Issue);
        (a.assignee, a.assignee_email) = (Some("레이븐".into()), Some("raven@x.io".into()));
        a.tags = vec!["bug".into(), "docs".into()];
        let mut b = row("argos-0002", Kind::Issue);
        (b.assignee, b.assignee_email) = (Some("joseph".into()), Some("jo@x.io".into()));
        b.tags = vec!["ui".into()];
        let m = row("argos-0003", Kind::Milestone);
        let mut k = row("argos-0004", Kind::Issue);
        k.assignee = Some("Kim Lee".into());
        let cfg = crate::config::Config::parse("prefix = \"argos\"\n").unwrap();
        let mut app = App::new(vec![a, b, m, k], cfg, crate::nav::Path::new());
        app.site.me = Some("레이븐 (raven@x.io)".into());
        app
    }

    fn press(a: &mut App, code: ratatui::crossterm::event::KeyCode) {
        a.key(ratatui::crossterm::event::KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE));
    }

    fn type_in(a: &mut App, s: &str) {
        for c in s.chars() {
            press(a, ratatui::crossterm::event::KeyCode::Char(c));
        }
    }

    fn typed(a: &App) -> &str {
        match &a.mode {
            Mode::Filter(q) => q.text(),
            m => panic!("거름망 칸이 아니다: {m:?}"),
        }
    }

    #[test]
    fn tab_completes_unique_keys_then_their_values() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "mil");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "milestone=");
        assert!(a.offered().is_some());
        press(&mut a, Tab);
        assert_eq!(typed(&a), "milestone=none");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "milestone=argos-0003");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "milestone=none");
    }

    #[test]
    fn key_candidates_cycle_in_table_order_and_other_keys_commit() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "s");
        for want in ["status", "stale", "since", "started_at", "status"] {
            press(&mut a, Tab);
            assert_eq!(typed(&a), want);
        }
        press(&mut a, BackTab);
        assert_eq!(typed(&a), "started_at");
        press(&mut a, Char('='));
        press(&mut a, Tab);
        assert_eq!(typed(&a), "started_at=", "date fields have no value candidates");
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "done");
        press(&mut a, BackTab);
        assert_eq!(typed(&a), "done_at");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "done");
    }

    #[test]
    fn tab_uses_the_aimed_value_and_keeps_the_list_and_aim_in_sync() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        press(&mut a, Down);
        press(&mut a, Tab);
        assert_eq!(typed(&a), "tag=docs");
        let (_, values) = a.offered().unwrap();
        let Mode::Filter(q) = &a.mode else { panic!() };
        assert_eq!(a.offer_aim.row(q, values.len()), 1);
        press(&mut a, BackTab);
        assert_eq!(typed(&a), "tag=bug");
        press(&mut a, BackTab);
        assert_eq!(typed(&a), "tag=ui");
        press(&mut a, Enter);
        assert_eq!(a.mode, Mode::Browse);
    }

    #[test]
    fn tab_completes_fixed_fields_without_changing_enter_or_grep() {
        use ratatui::crossterm::event::KeyCode::*;
        for (prefix, expected) in [("status=", "todo"), ("priority=p", "p0"), ("type=", "issue")] {
            let mut a = explorer();
            a.hit("SPC f");
            type_in(&mut a, prefix);
            press(&mut a, Tab);
            let key = prefix.split_once('=').unwrap().0;
            assert_eq!(typed(&a), format!("{key}={expected}"));
            press(&mut a, Tab);
            assert_ne!(typed(&a), format!("{key}={expected}"));
        }
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "status=todo");
        press(&mut a, Enter);
        assert_eq!(a.mode, Mode::Browse);
        a.hit("SPC /");
        type_in(&mut a, "mil");
        press(&mut a, Tab);
        assert!(matches!(&a.mode, Mode::Grep(q, _) if q.text() == "mil"));
    }

    #[test]
    fn completion_preserves_quotes_commas_suffixes_and_paste_resets_the_cycle() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=docs,b");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "tag=docs,bug");
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "assignee=Ki");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "assignee=\"Kim Lee\"");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "assignee=\"Kim Lee\"");
        a.paste(" tag=");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "assignee=\"Kim Lee\" tag=bug");
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "grep=\"x mil");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "grep=\"x mil");
    }

    #[test]
    fn completion_at_the_cursor_keeps_existing_equals_and_other_fields() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=bug milestone=none");
        for _ in 0..12 {
            press(&mut a, Left);
        }
        press(&mut a, Tab);
        assert_eq!(typed(&a), "tag=bug milestone=none");
        let Mode::Filter(q) = &a.mode else { panic!() };
        assert_eq!(q.cursor(), "tag=bug milestone=".len());
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "assignee=레이 tag=docs");
        for _ in 0..9 {
            press(&mut a, Left);
        }
        press(&mut a, Tab);
        assert_eq!(typed(&a), "assignee=raven@x.io tag=docs");
    }

    #[test]
    fn a_tab_cycle_never_returns_after_escape_or_editing() {
        use ratatui::crossterm::event::KeyCode::*;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "s");
        press(&mut a, Tab);
        press(&mut a, Tab);
        assert_eq!(typed(&a), "stale");
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "stale");
        press(&mut a, Tab);
        assert_eq!(typed(&a), "stale=");
        press(&mut a, Esc);
        a.hit("SPC f");
        type_in(&mut a, "s");
        press(&mut a, Tab);
        press(&mut a, Backspace);
        press(&mut a, Tab);
        assert_eq!(typed(&a), "status=");
    }

    /// **값 목록이 선 동안 Enter 는 값을 넣고, 안 선 때 거름망을 건다**(사용자 결정).
    #[test]
    fn enter_puts_the_value_in_while_the_list_stands_and_applies_once_it_does_not() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=bug assignee=JO");
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "tag=bug assignee=jo@x.io ", "메일을 넣고 빈칸 하나를 붙인다");
        assert!(a.offered().is_none(), "넣은 뒤에도 목록이 선다");
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.mode, Mode::Browse, "목록이 없는데 Enter 가 거름망을 안 걸었다");
        assert!(a.hung.is_some());
    }

    #[test]
    fn up_and_down_aim_the_value_enter_puts_in() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        press(&mut a, KeyCode::Up);
        for _ in 0..5 {
            press(&mut a, KeyCode::Down);
        }
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "tag=docs ", "bug·docs·ui 의 끝에서 하나 올라온 줄이 아니다");
        // 커서가 값 머리에 서도 값 전부를 간다 — 친 것이 없으니 첫 줄(bug)이다.
        for _ in 0..5 {
            press(&mut a, KeyCode::Left);
        }
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "tag=bug ", "커서가 선 값을 통째로 안 갈았다");
    }

    /// **겨눈 줄은 칸을 다시 열거나 같은 글로 돌아와도 되살아나지 않는다** — 글과 커서만으로 가르던 때는 닫았다 다시
    /// 열어 같은 `tag=` 를 치면, 또 한 자 쳤다 지우면 옛 줄이 겨눠져 Enter 가 첫 줄 아닌 값을 넣었다.
    #[test]
    fn the_aim_does_not_come_back_on_the_same_text() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Esc);
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "tag=bug ", "다시 연 칸에 옛 겨눔이 되살아났다");

        press(&mut a, KeyCode::Esc);
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        press(&mut a, KeyCode::Down);
        type_in(&mut a, "x");
        press(&mut a, KeyCode::Backspace);
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "tag=bug ", "쳤다 지운 글에 옛 겨눔이 되살아났다");
    }

    /// **창이 낮아 안내 칸이 한 줄도 못 서면 Enter 는 건다** — 안 보이는 목록이 Enter 를 먹으면 보이지 않던 값이
    /// 칸에 들어간다. 위·아래도 안 보이는 줄을 옮기지 않는다.
    #[test]
    fn a_list_the_window_cannot_show_does_not_take_enter() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        let screen = super::super::draw::tests::render(&mut a, 100, 7).join("\n");
        assert!(!screen.contains("> bug"), "낮은 창에 목록이 섰다\n{screen}");
        assert!(a.offered().is_none(), "안 그린 목록이 선 것으로 읽힌다");
        press(&mut a, KeyCode::Down);
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.mode, Mode::Browse, "안 보이는 목록이 Enter 를 먹었다");
        // 창이 다시 넓어지면 목록이 돌아온다.
        a.hit("SPC f");
        type_in(&mut a, "tag=");
        super::super::draw::tests::render(&mut a, 100, 30);
        assert!(a.offered().is_some());
    }

    /// **넣은 값은 거르개가 그대로 받는다** — 빈칸 든 이름을 따옴표로 묶어 넣으면 `query::split_items` 가 한 값으로
    /// 읽어 그 사람의 줄만 남는다. 묶지 않거나 따옴표를 글자로 읽으면 아무 줄도 안 남는다(리뷰 moai-h2rh.koh 6번).
    #[test]
    fn a_quoted_name_put_in_filters_that_person() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "assignee=Kim");
        press(&mut a, KeyCode::Enter);
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.mode, Mode::Browse);
        let kept: Vec<&str> =
            a.site.issues.iter().zip(&a.site.keep).filter(|(_, k)| **k).map(|(i, _)| i.id.as_str()).collect();
        assert_eq!(kept, ["argos-0004"]);
    }

    /// 고른 값이 따옴표 안팎에서 짝을 지킨다 — 빈칸 든 이름은 `=` 바로 뒤에서만 묶고, 닫지 않은 따옴표 뒤에 친
    /// 항목은 남는다.
    #[test]
    fn putting_a_value_keeps_the_quotes_balanced_and_the_rest_of_the_line() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "assignee=Kim");
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "assignee=\"Kim Lee\" ");

        // 닫지 않은 따옴표 뒤에 이미 친 항목은 고른 값에 안 먹힌다.
        a.mode = Mode::Filter(Input::new("assignee=\"Ki tag=bug"));
        for _ in 0.." tag=bug".len() {
            press(&mut a, KeyCode::Left);
        }
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "assignee=\"Kim Lee\" tag=bug");

        // 따옴표 안의 첫 값을 갈아도 다시 묶지 않는다.
        a.mode = Mode::Filter(Input::new("assignee=\"Ki,jo@x.io\""));
        for _ in 0..",jo@x.io\"".len() {
            press(&mut a, KeyCode::Left);
        }
        press(&mut a, KeyCode::Enter);
        assert_eq!(typed(&a), "assignee=\"Kim Lee,jo@x.io\"");
    }

    /// 다 친 값이면 목록이 안 선다 — `milestone=none` 의 Enter 가 값을 다시 넣지 않고 건다.
    #[test]
    fn a_value_typed_out_whole_applies_on_the_first_enter() {
        use ratatui::crossterm::event::KeyCode;
        let mut a = explorer();
        a.hit("SPC f");
        type_in(&mut a, "milestone=none");
        press(&mut a, KeyCode::Enter);
        assert_eq!(a.mode, Mode::Browse);
    }

    /// 칸 위에 안내가 선다 — 항목과 예, 값 자리에서는 고를 값과 그것을 고르는 키.
    #[test]
    fn the_panel_over_the_field_shows_keys_and_examples_then_values() {
        let mut a = explorer();
        a.hit("SPC f");
        let lang = a.site.lang;
        let screen = super::super::draw::tests::render(&mut a, 100, 30).join("\n");
        assert!(screen.contains(say(lang, "tui.hint.keys")), "{screen}");
        assert!(screen.contains("status tag no-tag"), "{screen}");
        assert!(screen.contains(EXAMPLES[0]), "{screen}");
        type_in(&mut a, "assignee=");
        let screen = super::super::draw::tests::render(&mut a, 100, 30).join("\n");
        assert!(screen.contains("> me"), "겨눈 첫 줄에 글리프가 없다\n{screen}");
        assert!(screen.contains("레이븐 (raven@x.io)"), "{screen}");
        assert!(screen.contains("Up·Down"), "고르는 키를 안 댄다\n{screen}");
        assert!(!screen.contains(EXAMPLES[0]), "값 자리인데 예가 섰다\n{screen}");
    }

    /// 예는 탐색기가 받는 거르개다 — 문법이 바뀌어 예가 낡으면 여기서 붉어진다.
    #[test]
    fn every_example_is_a_filter_the_explorer_takes() {
        let mut a = explorer();
        for ex in EXAMPLES {
            a.mode = Mode::Filter(Input::new(ex));
            assert_eq!(a.input_error(), None, "`{ex}` 를 안 받는다");
        }
    }
}
