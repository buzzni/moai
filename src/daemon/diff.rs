//! `changes` — 옛 스냅샷과 지금 스냅샷을 줄마다 견주어 바뀐 id 와 사라진 id 를 고른다(moai-8ts9). **순수 함수다.**
//!
//! 받는 것은 부르는 쪽이 읽어 건넨 **목록의 줄**(`serde_json::Value`)과 저널의 `note` 줄뿐이다. 파일을 열지도 찍지도
//! 않는다. 무엇을 기억하고 언제 잊는지는 데몬의 몫이다(`daemon::changes`) — 여기는 두 [`Seen`] 의 차만 안다.
//!
//! **`query` 가 아니라 데몬에 선다**(두 번째 리뷰 `moai-5n6m.9dr` 의 8). 받는 것이 `&[Issue]` 가 아니라 `show` 가
//! 낸 줄이라 `query` 의 계약(CLAUDE.md "`report` 와 `query` 는 순수 함수다")에 안 맞고, `query` 는 라이브러리의
//! 공개 모듈이라 거기 두면 약속 없는 공개면이 그만큼 넓어진다. 순수 함수라는 성질은 그대로다.
//!
//! ## 견주는 것
//!
//! - **목록의 줄**: 클라이언트가 `reset` 때 다시 읽는 그 목록(`show` 의 `all: true`)의 줄을 id 마다 지문으로 줄인다.
//!   그 줄에는 파생값이 든다 — 묶음의 칸(`derived_status`), 멤버의 에픽(`derived_epic`)과 물려받은 마일스톤
//!   (`derived_milestone`), 미룬 조상(`shelved_by`), 노트에서 읽은 `work`. 그래서 마지막 멤버가 닫혀 에픽의 칸이
//!   바뀌거나 에픽이 마일스톤에 들어 멤버의 마일스톤이 바뀌면, 파일의 그 줄이 그대로여도 지문이 달라진다
//!   (리뷰 `moai-5n6m.e9f` 의 6). `updated_at` 을 보지 않으므로 `git pull`·병합·손 고침으로 옛 도장 그대로 들어온
//!   줄도 잡는다(같은 리뷰의 4)
//! - **노트**: 메모는 스냅샷의 줄을 안 바꾸므로 저널의 `note` 줄을 id 마다 모아 지문으로 줄인다(2026-11-04 사람의
//!   결정 — 노트가 붙은 줄도 `changed` 다). 글은 지문으로만 들고 내지 않는다. **시각의 창이 없다** — 도장이 아니라
//!   줄의 있고 없음을 견주므로, 시각을 락 밖에서 뜬 `note` 가 락을 오래 기다려 늦게 확정되어도(리뷰의 1),
//!   다른 clone 에서 옛 도장으로 들어와도 다음 견줌에 선다
//!
//! 이것은 **저널을 접어 상태를 만드는 일이 아니다.** 노트에서 얻는 것은 "이 id 를 다시 읽어라" 는 가리킴뿐이고,
//! 스냅샷에도 저널에도 새 필드가 없다. 지운 줄은 저널의 `rm` 이 아니라 두 목록의 차로 안다.
//!
//! **목록 밖의 값은 견주지 않는다.** `show <id>` 만 내는 것(`children`·`members`·`journal`·`commits`·`wiki`)은 목록의
//! 줄에 없다. 펼친 줄 하나를 들고 있는 클라이언트는 그 id 나 그 줄이 대는 id 가 이름에 서면 그것을 다시 읽는다.

use crate::model::JournalEntry;
use ring::digest::{Context, SHA256};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// 줄 하나(또는 노트 묶음 하나)의 지문.
type Mark = [u8; 16];

/// 한 때의 스냅샷을 견줄 꼴로 줄인 것 — id 마다 목록의 줄과 노트의 지문.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Seen {
    rows: BTreeMap<String, Mark>,
    /// 목록에 선 id 의 노트만 든다 — 목록에 없는 줄의 메모는 알릴 것이 없다.
    notes: BTreeMap<String, Mark>,
}

impl Seen {
    /// Keep old marks for unreadable IDs; an unidentified line may hide any missing row.
    pub fn preserve_unread(&mut self, old: &Seen, unread: &[Option<String>]) {
        let unknown = unread.iter().any(Option::is_none);
        for (id, mark) in &old.rows {
            if unread.iter().any(|bad| bad.as_ref() == Some(id)) || (unknown && !self.rows.contains_key(id)) {
                self.rows.insert(id.clone(), *mark);
                match old.notes.get(id) {
                    Some(notes) => {
                        self.notes.insert(id.clone(), *notes);
                    }
                    None => {
                        self.notes.remove(id);
                    }
                }
            }
        }
    }
    /// 목록의 줄(`id` 를 든 객체)과 `note` 저널 줄에서. `id` 가 없는 줄은 견줄 이름이 없어 뺀다.
    ///
    /// **같은 id 가 둘 선 목록**(`duplicate_id`)은 그 줄들을 차례대로 한 지문에 넣는다 — 하나만 바뀌어도 선다.
    /// 노트는 **차례를 안 따진다** — 저널 파일의 차례는 병합이 바꿀 수 있고, 그것은 바뀜이 아니다.
    pub fn of(rows: &[Value], notes: &[JournalEntry]) -> Seen {
        let mut by_id: BTreeMap<&str, Context> = BTreeMap::new();
        for row in rows {
            let Some(id) = row.get("id").and_then(Value::as_str) else { continue };
            // These display fields depend on wall time or heartbeat, not edits to the row.
            let mut compared = row.clone();
            if let Some(object) = compared.as_object_mut() {
                object.remove("session");
                object.remove("archived");
            }
            let bytes = serde_json::to_vec(&compared).unwrap_or_default();
            let ctx = by_id.entry(id).or_insert_with(|| Context::new(&SHA256));
            ctx.update(&(bytes.len() as u64).to_le_bytes());
            ctx.update(&bytes);
        }
        let rows: BTreeMap<String, Mark> = by_id.into_iter().map(|(id, ctx)| (id.to_string(), finish(ctx))).collect();
        let mut each: BTreeMap<&str, Vec<Mark>> = BTreeMap::new();
        for e in notes.iter().filter(|e| rows.contains_key(&e.id)) {
            each.entry(&e.id).or_default().push(mark(&serde_json::to_vec(e).unwrap_or_default()));
        }
        let notes = each
            .into_iter()
            .map(|(id, mut marks)| {
                marks.sort_unstable();
                let mut ctx = Context::new(&SHA256);
                for m in &marks {
                    ctx.update(m);
                }
                (id.to_string(), finish(ctx))
            })
            .collect();
        Seen { rows, notes }
    }

    /// 이 스냅샷 전체의 지문 — 줄 하나·노트 하나만 달라도 다르다. 커서가 이것으로 기억을 찾는다.
    pub fn fingerprint(&self) -> String {
        let mut ctx = Context::new(&SHA256);
        for (tag, map) in [(b"r", &self.rows), (b"n", &self.notes)] {
            for (id, m) in map {
                ctx.update(tag);
                ctx.update(&(id.len() as u64).to_le_bytes());
                ctx.update(id.as_bytes());
                ctx.update(m);
            }
        }
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD.encode(finish(ctx))
    }

    /// 메모리에서 차지하는 크기의 어림(바이트) — 기억의 상한을 셈하는 데 쓴다. 맵의 마디 몫을 줄마다 64 로 친다.
    pub fn weight(&self) -> usize {
        self.rows.keys().chain(self.notes.keys()).map(|id| id.len() + std::mem::size_of::<Mark>() + 64).sum()
    }
}

fn finish(ctx: Context) -> Mark {
    let mut out = [0u8; 16];
    out.copy_from_slice(&ctx.finish().as_ref()[..16]);
    out
}

fn mark(bytes: &[u8]) -> Mark {
    let mut ctx = Context::new(&SHA256);
    ctx.update(bytes);
    finish(ctx)
}

/// 답. 줄 본문은 싣지 않는다 — 스냅샷이 진실이고, 클라이언트가 바뀐 줄을 다시 읽는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub cursor: String,
    /// 옛 스냅샷 뒤로 새로 섰거나, 목록의 줄이 달라졌거나(파생값 포함), 노트가 달라진 id. 차례는 id 순이다.
    pub changed: Vec<String>,
    /// 옛 목록에 있었고 지금 목록에 없는 id — `rm`, 아카이브로 옮김, done 에 `archive_days` 넘게 서서 목록에서 빠짐.
    pub removed: Vec<String>,
    /// 참이면 두 목록은 비었다 — 견줄 옛 스냅샷이 없으니 전체를 다시 읽는다.
    pub reset: bool,
}

/// 옛 스냅샷(`None` 이면 모른다 — 커서가 없거나, 못 읽거나, 기억에서 밀려났다)과 지금을 견준 답. `cursor` 는 `new` 를
/// 가리키는 글이고, 부르는 쪽이 짓는다.
pub fn changes(old: Option<&Seen>, new: &Seen, cursor: String) -> Changes {
    let Some(old) = old else { return Changes { cursor, changed: Vec::new(), removed: Vec::new(), reset: true } };
    let changed = new
        .rows
        .iter()
        .filter(|(id, m)| old.rows.get(*id) != Some(m) || old.notes.get(*id) != new.notes.get(*id))
        .map(|(id, _)| id.clone())
        .collect();
    let removed = old.rows.keys().filter(|id| !new.rows.contains_key(*id)).cloned().collect();
    Changes { cursor, changed, removed, reset: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Actor;
    use serde_json::json;

    fn row(id: &str, title: &str) -> Value {
        json!({ "id": id, "title": title, "status": "todo", "updated_at": "2026-11-04T09:00:00Z" })
    }

    fn note(id: &str, text: &str, at: &str) -> JournalEntry {
        JournalEntry::note(id, text, at, &Actor { name: "나".into(), email: "me@example.com".into() })
    }

    fn diff(old: &Seen, new: &Seen) -> (Vec<String>, Vec<String>) {
        let got = changes(Some(old), new, String::new());
        assert!(!got.reset);
        (got.changed, got.removed)
    }

    #[test]
    fn no_old_snapshot_resets() {
        let new = Seen::of(&[row("a-1", "하나")], &[]);
        let got = changes(None, &new, "c".into());
        assert_eq!(got, Changes { cursor: "c".into(), changed: vec![], removed: vec![], reset: true });
    }

    #[test]
    fn the_same_snapshot_is_the_same_fingerprint_and_no_change() {
        let rows = [row("a-1", "하나"), row("a-2", "둘")];
        let notes = [note("a-1", "메모", "2026-11-04T09:00:00Z")];
        let (a, b) = (Seen::of(&rows, &notes), Seen::of(&rows, &notes));
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(diff(&a, &b), (vec![], vec![]));
    }

    #[test]
    fn clock_and_heartbeat_fields_do_not_change_the_fingerprint() {
        let mut item = row("a-1", "work");
        item["session"] = json!({"silent_minutes": 0, "last_seen": "first", "lost": false});
        let old = Seen::of(&[item.clone()], &[]);
        item["session"] = json!({"silent_minutes": 30, "last_seen": "heartbeat", "lost": true});
        item["archived"] = json!(true);
        let later = Seen::of(&[item.clone()], &[]);
        assert_eq!(old.fingerprint(), later.fingerprint());
        assert_eq!(diff(&old, &later), (vec![], vec![]));
        item["title"] = json!("edited");
        assert_eq!(diff(&old, &Seen::of(&[item], &[])).0, vec!["a-1"]);
    }

    /// **도장은 보지 않는다** — `updated_at` 이 그대로인 채 내용만 바뀐 줄(`git pull` 이 들인 남의 고침)도 선다.
    #[test]
    fn a_row_that_kept_its_stamp_but_changed_is_named() {
        let old = Seen::of(&[row("a-1", "하나"), row("a-2", "둘")], &[]);
        let new = Seen::of(&[row("a-1", "남이 고침"), row("a-2", "둘")], &[]);
        assert_ne!(old.fingerprint(), new.fingerprint());
        assert_eq!(diff(&old, &new), (vec!["a-1".to_string()], vec![]));
    }

    /// 파생값만 달라진 줄도 선다 — 목록의 줄에 실려 있기 때문이다(에픽의 칸, 멤버의 물려받은 마일스톤).
    #[test]
    fn a_derived_value_that_moved_is_named() {
        let mut epic = row("e-1", "에픽");
        let old = Seen::of(&[epic.clone(), row("e-1.x", "멤버")], &[]);
        epic["derived_status"] = json!("done");
        let mut member = row("e-1.x", "멤버");
        member["derived_milestone"] = json!("m-1");
        let new = Seen::of(&[epic, member], &[]);
        assert_eq!(diff(&old, &new), (vec!["e-1".to_string(), "e-1.x".to_string()], vec![]));
    }

    #[test]
    fn a_new_row_is_changed_and_a_missing_row_is_removed() {
        let old = Seen::of(&[row("a-1", "하나"), row("a-2", "둘")], &[]);
        let new = Seen::of(&[row("a-1", "하나"), row("a-3", "셋")], &[]);
        assert_eq!(diff(&old, &new), (vec!["a-3".to_string()], vec!["a-2".to_string()]));
    }

    #[test]
    fn unread_rows_keep_their_old_marks_until_they_can_be_read() {
        let old = Seen::of(&[row("a-1", "one"), row("a-2", "two")], &[]);
        let mut partial = Seen::of(&[], &[]);
        partial.preserve_unread(&old, &[Some("a-1".into())]);
        assert_eq!(diff(&old, &partial), (vec![], vec!["a-2".to_string()]));
        let mut unknown = Seen::of(&[], &[]);
        unknown.preserve_unread(&old, &[None]);
        assert_eq!(diff(&old, &unknown), (vec![], vec![]));
        let repaired = Seen::of(&[row("a-1", "edited"), row("a-2", "two")], &[]);
        assert_eq!(diff(&unknown, &repaired), (vec!["a-1".to_string()], vec![]));
        assert_eq!(diff(&unknown, &Seen::of(&[], &[])).1, vec!["a-1", "a-2"]);
    }

    /// 노트는 시각과 상관없이 선다 — 옛 도장의 노트(늦게 확정됐거나 병합으로 들어온 것)도. 목록에 없는 줄의 노트는
    /// 알릴 것이 없다. 노트의 차례만 바뀐 것은 바뀜이 아니다.
    #[test]
    fn a_note_names_its_row_whatever_its_stamp() {
        let rows = [row("a-1", "하나"), row("a-2", "둘")];
        let first = note("a-1", "첫 메모", "2026-11-04T09:00:00Z");
        let old = Seen::of(&rows, std::slice::from_ref(&first));
        let late = note("a-2", "늦은 메모", "2026-01-01T00:00:00Z");
        let gone = note("nowhere", "없는 줄", "2026-11-04T09:00:00Z");
        let new = Seen::of(&rows, &[late.clone(), first.clone(), gone]);
        assert_eq!(diff(&old, &new), (vec!["a-2".to_string()], vec![]));
        let shuffled = Seen::of(&rows, &[first, late]);
        assert_eq!(shuffled.fingerprint(), new.fingerprint());
    }

    #[test]
    fn a_twin_id_is_named_when_either_line_moves() {
        let old = Seen::of(&[row("a-1", "하나"), row("a-1", "쌍둥이")], &[]);
        let new = Seen::of(&[row("a-1", "하나"), row("a-1", "쌍둥이 고침")], &[]);
        assert_eq!(diff(&old, &new), (vec!["a-1".to_string()], vec![]));
    }

    #[test]
    fn weight_grows_with_rows() {
        let small = Seen::of(&[row("a-1", "하나")], &[]);
        let big = Seen::of(&[row("a-1", "하나"), row("a-2", "둘")], &[]);
        assert!(big.weight() > small.weight() && small.weight() > 0);
    }
}
