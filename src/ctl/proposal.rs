//! Suggestions live for one visible window. The book wakes socket waiters without blocking its owner loop.
use super::wire::{Reply, Verb};
use serde_json::{Value, json};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const MAX_OPEN: usize = 5;
const RETAIN: Duration = Duration::from_secs(600);

pub struct Book {
    inner: Mutex<Entries>,
    changed: Condvar,
}

#[derive(Default)]
struct Entries {
    next: u32,
    rows: Vec<Entry>,
    closed: bool,
    /// When a web tab whose stream ended stops counting as reconnecting ([`Book::close_at`]).
    closes_at: Option<Instant>,
}

#[derive(Clone)]
pub struct Entry {
    pub id: String,
    pub text: String,
    pub from: Option<String>,
    pub then: Option<Verb>,
    pub state: String,
    pub ran: Option<Reply>,
    pub made: Instant,
    pub ttl: Duration,
    finished: Option<Instant>,
}

impl Entry {
    pub fn value(&self) -> Value {
        json!({"id":self.id,"text":self.text,"from":self.from,"then":self.then,"state":self.state,"ran":self.ran,
            "remaining_ms": self.ttl.saturating_sub(self.made.elapsed()).as_millis() as u64})
    }
}

impl Book {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Entries::default()), changed: Condvar::new() }
    }

    pub fn add(&self, window: &str, text: &str, then: Option<Verb>, ttl: u64, from: Option<String>) -> Reply {
        if ttl == 0 || ttl > 3600 {
            return Reply::error("bad_input", "proposal ttl must be 1 to 3600 seconds");
        }
        let clean: String = text.chars().filter(|c| *c == '\n' || !c.is_control()).collect();
        if clean.trim().is_empty() || clean.chars().count() > 500 || clean.split('\n').count() > 3 {
            return Reply::error("bad_input", "proposal needs 1 to 500 characters and at most three lines");
        }
        if then.as_ref().is_some_and(|v| !v.proposable()) {
            return Reply::error("unsupported", "this action cannot follow a proposal");
        }
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.closed {
            return Reply::error("gone", "window closed");
        }
        expire(&mut inner);
        if inner.rows.iter().filter(|row| row.state == "pending").count() >= MAX_OPEN {
            return Reply::error("refused", "five proposals are already open");
        }
        inner.next += 1;
        let id = format!("{window}/{}", inner.next);
        let row = Entry {
            id,
            text: clean,
            from,
            then,
            state: "pending".into(),
            ran: None,
            made: Instant::now(),
            ttl: Duration::from_secs(ttl),
            finished: None,
        };
        let answer = Reply::success(row.value());
        inner.rows.push(row);
        self.changed.notify_all();
        answer
    }

    pub fn rows(&self) -> Vec<Entry> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        expire(&mut inner);
        inner.rows.iter().filter(|row| row.state == "pending").cloned().collect()
    }

    pub fn next_expiry_in(&self) -> Option<Duration> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .rows
            .iter()
            .filter(|row| row.state == "pending")
            .map(|row| row.ttl.saturating_sub(row.made.elapsed()))
            .min()
    }

    pub fn get(&self, id: &str) -> Reply {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        expire(&mut inner);
        match inner.rows.iter().find(|row| row.id == id) {
            Some(row) => Reply::success(row.value()),
            None => Reply::error("not_found", "proposal not found"),
        }
    }

    pub fn finish(&self, id: &str, state: &str, ran: Option<Reply>) -> Reply {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        expire(&mut inner);
        let Some(row) = inner.rows.iter_mut().find(|row| row.id == id) else {
            return Reply::error("not_found", "proposal not found");
        };
        if row.state != "pending" {
            return Reply::error("refused", "proposal already answered");
        }
        row.state = state.into();
        row.ran = ran;
        row.finished = Some(Instant::now());
        let answer = Reply::success(row.value());
        self.changed.notify_all();
        answer
    }

    #[cfg(test)]
    pub fn wait(&self, id: &str, seconds: u64) -> Reply {
        self.wait_while(id, seconds, || true)
    }

    /// Check the caller between wakeups so a vanished socket does not occupy a waiter slot until the TTL.
    pub fn wait_while(&self, id: &str, seconds: u64, mut connected: impl FnMut() -> bool) -> Reply {
        let end = Instant::now() + Duration::from_secs(seconds);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if inner.closed || inner.closes_at.is_some_and(|at| Instant::now() >= at) {
                return Reply::error("gone", "window closed");
            }
            expire(&mut inner);
            let Some(row) = inner.rows.iter().find(|row| row.id == id) else {
                return Reply::error("not_found", "proposal not found");
            };
            if row.state != "pending" || Instant::now() >= end {
                return Reply::success(row.value());
            }
            if !connected() {
                return Reply::error("gone", "proposal waiter disconnected");
            }
            let left = end.min(row.made + row.ttl).saturating_duration_since(Instant::now());
            (inner, _) = self
                .changed
                .wait_timeout(inner, left.min(Duration::from_millis(200)))
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.closed = true;
        self.changed.notify_all();
    }

    /// A web tab whose stream ended may come back within its resume grace, so its waiters stay until `at` and then
    /// get `gone`; `None` cancels that when the stream resumes. Waiters see it at their next wakeup in
    /// [`Book::wait_while`], so nothing has to run when the grace ends — a tab nobody reopens still answers them.
    pub fn close_at(&self, at: Option<Instant>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.closes_at = at;
        self.changed.notify_all();
    }

    // Only `web::ctl`'s tests read it, and those are Unix-only (`#[cfg(all(test, unix))]`).
    #[cfg(all(test, unix))]
    pub fn closes_at(&self) -> Option<Instant> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).closes_at
    }
}

fn expire(inner: &mut Entries) {
    let now = Instant::now();
    for row in &mut inner.rows {
        if row.state == "pending" && now.duration_since(row.made) >= row.ttl {
            row.state = "expired".into();
            row.finished = Some(now);
        }
    }
    inner.rows.retain(|row| row.finished.is_none_or(|at| now.duration_since(at) < RETAIN));
}

impl Default for Book {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sixth_proposal_is_refused_and_answers_free_a_slot() {
        let book = Book::new();
        for _ in 0..MAX_OPEN {
            assert!(book.add("window", "hello", None, 900, None).ok);
        }
        assert_eq!(book.add("window", "sixth", None, 900, None).code.as_deref(), Some("refused"));
        assert_eq!(book.finish("window/1", "declined", None).data.unwrap()["state"], "declined");
        assert!(book.add("window", "next", None, 900, None).ok);
    }

    #[test]
    fn waiting_returns_the_answer_and_a_short_wait_leaves_it_pending() {
        let book = std::sync::Arc::new(Book::new());
        book.add("window", "hello", None, 900, None);
        assert_eq!(book.wait("window/1", 0).data.unwrap()["state"], "pending");
        let other = book.clone();
        let waited = std::thread::spawn(move || other.wait("window/1", 2));
        book.finish("window/1", "accepted", Some(Reply::success(json!({"changed":true}))));
        let result = waited.join().unwrap();
        assert_eq!(result.data.unwrap()["ran"]["data"]["changed"], true);
    }

    #[test]
    fn bad_text_and_an_unreadable_then_are_refused_before_showing() {
        let book = Book::new();
        assert_eq!(book.add("window", "\u{0001}", None, 900, None).code.as_deref(), Some("bad_input"));
        assert_eq!(
            book.add("window", "hello", Some(Verb::Keys { seqs: vec!["y".into()], text: None }), 900, None)
                .code
                .as_deref(),
            Some("unsupported")
        );
        assert!(book.rows().is_empty());
    }

    #[test]
    fn closing_the_window_wakes_a_waiter_with_gone() {
        let book = std::sync::Arc::new(Book::new());
        book.add("window", "hello", None, 900, None);
        let other = book.clone();
        let waited = std::thread::spawn(move || other.wait("window/1", 5));
        book.close();
        assert_eq!(waited.join().unwrap().code.as_deref(), Some("gone"));
    }

    #[test]
    fn a_book_closing_later_keeps_its_waiters_until_then_and_a_resume_cancels_it() {
        let book = std::sync::Arc::new(Book::new());
        book.add("window", "hello", None, 900, None);
        // The tab's stream ended and came back: the closing it had scheduled no longer applies.
        book.close_at(Some(Instant::now()));
        book.close_at(None);
        let other = book.clone();
        let waited = std::thread::spawn(move || other.wait("window/1", 5));
        book.finish("window/1", "accepted", None);
        assert_eq!(waited.join().unwrap().data.unwrap()["state"], "accepted");
        // No resume within the grace: a waiter is told the window is gone, not left until its own wait ends.
        book.add("window", "again", None, 900, None);
        book.close_at(Some(Instant::now() + Duration::from_millis(50)));
        let started = Instant::now();
        assert_eq!(book.wait("window/2", 30).code.as_deref(), Some("gone"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn an_unanswered_proposal_expires_and_is_kept_for_a_late_answer() {
        let book = Book::new();
        book.add("window", "hello", None, 1, None);
        book.inner.lock().unwrap().rows[0].made -= Duration::from_secs(2);
        assert_eq!(book.wait("window/1", 10).data.unwrap()["state"], "expired");
        assert!(book.rows().is_empty());
        assert_eq!(book.get("window/1").data.unwrap()["state"], "expired");
    }
}
