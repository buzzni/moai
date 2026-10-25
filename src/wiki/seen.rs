//! 교훈이 어느 이슈에서 섰는가 — `Lesson: lessons/<slug>` 노트를 읽은 값(moai-4s5n.c0n). **순수 함수다.**
//!
//! 받는 것은 `&[Issue]` 와 부르는 쪽이 이미 읽은 저널 줄이다 — 파일을 열지 않고 찍지 않는다(CLAUDE.md "`report` 와
//! `query` 는 순수 함수다"). **저장하지 않는다**: 부를 때마다 노트에서 읽고, 칸·종류·순위를 이 값으로 가르지 않는다
//! — 보일 뿐이다(`model::work_of` 의 `model:` 줄과 같은 결).
//!
//! - **서로 다른 이슈 수다.** 같은 이슈에 같은 교훈 노트가 두 번이면 한 번이고, 그 이슈의 날짜는 가장 나중 노트의 것
//! - **이슈 하나의 하위(`<id>.xxx`)에 단 노트는 그 하위의 것이다** — 저널 줄의 `id` 가 갈라 주니 부모로 올려 세지 않는다
//! - 이슈가 스냅샷에 없는 저널 줄(지운 줄)은 안 센다 — 제목을 댈 줄이 없다. 아카이브된 줄은 부르는 쪽이 `issues` 에
//!   얹어 넘긴다

use crate::model::{self, Issue, JournalEntry};
use std::collections::BTreeMap;

/// 교훈 하나가 한 이슈에서 선 자리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub id: String,
    pub title: String,
    /// 그 이슈에서 이 교훈을 **가장 나중에** 적은 노트의 때.
    pub at: String,
    pub by: String,
}

/// 교훈 슬러그(`lessons/<slug>`)마다 선 자리 — 최근 먼저, 이슈마다 하나.
pub type Lessons = BTreeMap<String, Vec<Seen>>;

/// `journal` 은 id 별 저널 줄이다(`cmd::show::journal_of_rows` 가 읽은 그대로).
pub fn seen_in(issues: &[Issue], journal: &BTreeMap<String, Vec<JournalEntry>>) -> Lessons {
    let mut out: Lessons = BTreeMap::new();
    // 살아 있는 줄과 아카이브의 사본이 같은 id 로 겹쳐 올 수 있다(`archive_duplicate_id`) — 이슈는 하나로 센다.
    let mut done: std::collections::HashSet<&str> = Default::default();
    for i in issues {
        if !done.insert(i.id.as_str()) {
            continue;
        }
        let Some(entries) = journal.get(&i.id) else { continue };
        // 슬러그마다 이 이슈의 가장 나중 노트 — 저널은 시각 차례로 오지만 가정하지 않고 큰 쪽을 고른다.
        let mut last: BTreeMap<&str, &JournalEntry> = BTreeMap::new();
        for e in entries {
            for slug in model::note_of(e).into_iter().flat_map(model::lessons_in_note) {
                match last.get(slug) {
                    Some(prev) if prev.ts >= e.ts => {}
                    _ => {
                        last.insert(slug, e);
                    }
                }
            }
        }
        for (slug, e) in last {
            out.entry(slug.to_string()).or_default().push(Seen {
                id: i.id.clone(),
                title: i.title.clone(),
                at: e.ts.clone(),
                by: e.by.clone(),
            });
        }
    }
    for v in out.values_mut() {
        v.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    }
    out
}

/// 이슈 **하나**의 저널이 가리킨 교훈 슬러그들 — 처음 만난 차례로, 겹침 없이.
pub fn lessons_of(journal: &[JournalEntry]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for slug in journal.iter().filter_map(model::note_of).flat_map(model::lessons_in_note) {
        if !out.iter().any(|s| s == slug) {
            out.push(slug.to_string());
        }
    }
    out
}

/// 이 슬러그가 교훈 페이지인가 — 위키 뿌리 밑 `lessons/` 안이다.
pub fn is_lesson(slug: &str) -> bool {
    slug.strip_prefix(super::learn::LESSONS).is_some_and(|r| r.starts_with('/'))
}

/// 교훈 노트는 있는데 이 체크아웃에 페이지가 없는 슬러그들 — 다른 가지에서 아직 안 든 페이지일 수 있다.
pub fn unpaged(lessons: &Lessons, has_page: impl Fn(&str) -> bool) -> Vec<&str> {
    lessons.keys().map(String::as_str).filter(|s| !has_page(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    fn issue(id: &str, title: &str) -> Issue {
        Issue::new(id.into(), title.into(), Kind::Issue, Status::new("todo"), "2026-10-01T00:00:00Z")
    }

    fn note(id: &str, ts: &str, text: &str) -> JournalEntry {
        JournalEntry {
            ts: ts.into(),
            id: id.into(),
            kind: "note".into(),
            by: "me".into(),
            by_email: None,
            from: None,
            to: None,
            title: None,
            text: Some(text.into()),
            note: None,
        }
    }

    fn journal(entries: Vec<JournalEntry>) -> BTreeMap<String, Vec<JournalEntry>> {
        let mut m: BTreeMap<String, Vec<JournalEntry>> = BTreeMap::new();
        for e in entries {
            m.entry(e.id.clone()).or_default().push(e);
        }
        m
    }

    #[test]
    fn counts_distinct_issues_latest_first_and_a_repeat_counts_once() {
        let issues = [issue("a-1", "one"), issue("a-2", "two"), issue("a-1.x", "child")];
        let j = journal(vec![
            note("a-1", "2026-10-01T00:00:00Z", "Lesson: lessons/x"),
            note("a-1", "2026-10-05T00:00:00Z", "Lesson: lessons/x"),
            note("a-2", "2026-10-03T00:00:00Z", "Lesson: lessons/x\nLesson: lessons/y"),
            note("a-1.x", "2026-10-04T00:00:00Z", "Lesson: lessons/x"),
        ]);
        let l = seen_in(&issues, &j);
        let x: Vec<(&str, &str)> = l["lessons/x"].iter().map(|s| (s.id.as_str(), s.at.as_str())).collect();
        assert_eq!(
            x,
            [("a-1", "2026-10-05T00:00:00Z"), ("a-1.x", "2026-10-04T00:00:00Z"), ("a-2", "2026-10-03T00:00:00Z")],
            "하위의 노트는 하위의 것이고, 같은 이슈의 두 번은 한 번(가장 나중 때)이다"
        );
        assert_eq!(l["lessons/y"].len(), 1);
    }

    #[test]
    fn a_row_missing_from_the_snapshot_is_not_counted() {
        let l =
            seen_in(&[issue("a-1", "one")], &journal(vec![note("gone", "2026-10-01T00:00:00Z", "Lesson: lessons/x")]));
        assert!(l.is_empty());
    }

    #[test]
    fn lessons_of_one_issue_keep_first_seen_order() {
        let j = vec![note("a", "1", "Lesson: lessons/b"), note("a", "2", "Lesson: lessons/a\nLesson: lessons/b")];
        assert_eq!(lessons_of(&j), ["lessons/b", "lessons/a"]);
    }

    #[test]
    fn only_slugs_under_lessons_are_lessons() {
        assert!(is_lesson("lessons/x"));
        assert!(!is_lesson("lessons"));
        assert!(!is_lesson("lessonsx/y"));
        assert!(!is_lesson("README"));
    }
}
