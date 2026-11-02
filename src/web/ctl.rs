//! Visible browser tabs and the replies to requests sent down their event streams.
use crate::ctl::wire::{self, Reply, Request, Verb};
use serde_json::json;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

pub struct Tabs {
    inner: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    next_tab: u64,
    next_seq: u64,
    next_stream: u64,
    tabs: HashMap<String, Tab>,
    anonymous_streams: usize,
}

const RESUME_GRACE: Duration = Duration::from_secs(30);

struct Tab {
    token: String,
    root: String,
    here: String,
    overlay: bool,
    connection: Connection,
    opened: Instant,
    last_input: Instant,
    screen: String,
    pending: HashMap<u64, Pending>,
    proposals: std::sync::Arc<crate::ctl::proposal::Book>,
}

enum Connection {
    Connected { sender: mpsc::Sender<Push>, stream: u64 },
    Detached { at: Instant },
}

struct Pending {
    answer: mpsc::Sender<Reply>,
    actor: Option<String>,
}

pub struct Joined {
    pub id: String,
    pub token: String,
    pub incoming: mpsc::Receiver<Push>,
    pub stream: u64,
}

/// A stream owns either a tab (including its reconnect reservation) or one anonymous slot.
pub struct Stream<'a> {
    tabs: &'a Tabs,
    pub joined: Option<Joined>,
}

impl Drop for Stream<'_> {
    fn drop(&mut self) {
        if let Some(joined) = &self.joined {
            self.tabs.leave(&joined.id, joined.stream);
        } else {
            let mut registry = self.tabs.inner.lock().unwrap_or_else(|e| e.into_inner());
            registry.anonymous_streams -= 1;
        }
    }
}

pub enum Push {
    Ctl { seq: u64, request: serde_json::Value },
    Proposal { entry: serde_json::Value },
    ProposalClosed { id: String },
}

impl Tabs {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Registry::default()) }
    }

    /// Admission and tab registration share the same lock: overflow tabs cannot take a reconnecting tab's slot.
    pub fn stream(
        &self,
        here: &Path,
        overlay: bool,
        resume: Option<(&str, &str)>,
        ctl: bool,
        limit: usize,
    ) -> std::io::Result<Option<Stream<'_>>> {
        if ctl {
            return self
                .join_limited(here, overlay, resume, Some(limit))
                .map(|joined| joined.map(|joined| Stream { tabs: self, joined: Some(joined) }));
        }
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry.prune();
        if registry.occupied() >= limit {
            return Ok(None);
        }
        registry.anonymous_streams += 1;
        Ok(Some(Stream { tabs: self, joined: None }))
    }

    #[cfg(test)]
    pub fn join(&self, here: &Path, overlay: bool, resume: Option<(&str, &str)>) -> std::io::Result<Joined> {
        Ok(self.join_limited(here, overlay, resume, None)?.unwrap())
    }

    fn join_limited(
        &self,
        here: &Path,
        overlay: bool,
        resume: Option<(&str, &str)>,
        limit: Option<usize>,
    ) -> std::io::Result<Option<Joined>> {
        // The token is read before the registry lock, as every ctl request, `seen` and `ls` waits on that lock.
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry.prune();
        registry.next_stream += 1;
        let stream = registry.next_stream;
        let (sender, incoming) = mpsc::channel();
        if let Some((id, token)) = resume
            && let Some(tab) = registry.tabs.get_mut(id)
            && tab.token == token
            && crate::user_config::same_dir(Path::new(&tab.here), here)
        {
            // **The overlay is the page's view, not its identity** — turning the worktree overlay on or off is the
            // page's own reconnect (main.ts `events.reconnect()`), so requiring the old value refused every one of them.
            tab.overlay = overlay;
            tab.connect(sender.clone(), stream);
            for (_, pending) in tab.pending.drain() {
                let _ = pending.answer.send(Reply::error("gone", "tab stream replaced"));
            }
            for row in tab.proposals.rows() {
                let _ = sender.send(Push::Proposal { entry: row.value() });
            }
            return Ok(Some(Joined { id: id.into(), token: token.into(), incoming, stream }));
        }
        if limit.is_some_and(|limit| registry.occupied() >= limit) {
            return Ok(None);
        }
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        registry.next_tab += 1;
        let id = format!("{}.{}", std::process::id(), registry.next_tab);
        registry.tabs.insert(
            id.clone(),
            Tab {
                token: token.clone(),
                root: crate::store::Repo::opened_root(here).display().to_string(),
                here: here.display().to_string(),
                overlay,
                connection: Connection::Connected { sender, stream },
                opened: Instant::now(),
                last_input: Instant::now(),
                screen: "list".into(),
                pending: HashMap::new(),
                proposals: std::sync::Arc::new(crate::ctl::proposal::Book::new()),
            },
        );
        Ok(Some(Joined { id, token, incoming, stream }))
    }

    pub fn leave(&self, id: &str, stream: u64) {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tab) = registry.tabs.get_mut(id)
            && matches!(tab.connection, Connection::Connected { stream: current, .. } if current == stream)
        {
            let now = Instant::now();
            tab.connection = Connection::Detached { at: now };
            // Proposal waiters ride out a reconnect, but a tab nobody reopens must still answer them `gone` — prune only
            // runs on a later join, which may never come.
            tab.proposals.close_at(Some(now + RESUME_GRACE));
            for (_, pending) in tab.pending.drain() {
                let _ = pending.answer.send(Reply::error("gone", "tab closed"));
            }
        }
    }

    pub fn handle(&self, request: Request) -> Reply {
        self.handle_while(request, &|| true)
    }

    pub fn handle_while(&self, request: Request, connected: &dyn Fn() -> bool) -> Reply {
        if matches!(request.verb, Verb::Hello) {
            return Reply::success(json!({"windows": self.windows(request.target.as_deref())}));
        }
        let Some(target) = request.target.clone() else { return Reply::error("bad_input", "target tab is required") };
        match &request.verb {
            Verb::Propose { text, then, ttl, wait } => {
                if then.as_deref().is_some_and(|verb| {
                    matches!(verb, Verb::Screen { name } if !matches!(name.as_str(), "list" | "board" | "gantt" | "wiki"))
                        || matches!(verb, Verb::Find { scope, .. } if scope != "all")
                        || matches!(verb, Verb::View { name, .. } if !matches!(name.as_str(), "deferred" | "backlog" | "archived" | "worktree" | "raw") && !name.starts_with("column:"))
                        // No cell check here: the tab's columns (`counts`, …) are not the explorer's `Field` names, and
                        // the tab refuses an unknown one itself.
                        || matches!(
                            verb,
                            Verb::Pane { wiki_width: Some(_), .. } | Verb::Pane { gantt_label: Some(_), .. }
                        )
                        || matches!(verb, Verb::Do { name } if name.starts_with("proposal."))
                }) {
                    return Reply::error("unsupported", "that action is unavailable in a web tab");
                }
                if let Some(Verb::Do { name }) = then.as_deref() {
                    let mut probe = request.clone();
                    probe.verb = Verb::Actions;
                    probe.deadline_ms = probe.deadline_ms.min(wire::now_ms().saturating_add(5_000));
                    let actions = self.handle(probe);
                    if !actions.ok {
                        return actions;
                    }
                    let known = actions
                        .data
                        .as_ref()
                        .and_then(|data| data["actions"].as_array())
                        .is_some_and(|rows| rows.iter().any(|row| row["name"] == name.as_str()));
                    if !known {
                        return Reply::error("unsupported", format!("unknown web action: {name}"));
                    }
                }
                // `none` returns to the default zone, which every browser draws; the explorer skips it the same way.
                if let Some(Verb::Zone { name }) = then.as_deref()
                    && name != "none"
                {
                    let deadline_ms = request.deadline_ms.min(wire::now_ms().saturating_add(5_000));
                    let checked = self.dispatch(
                        &target,
                        json!({"verb": "validate_zone", "name": name, "deadline_ms": deadline_ms}),
                        None,
                        deadline_ms,
                    );
                    if !checked.ok {
                        return checked;
                    }
                }
                if request.deadline_ms <= wire::now_ms() {
                    return Reply::error("timeout", "control request expired");
                }
                let book = {
                    let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    let Some(tab) = registry.tabs.get(&target).filter(|tab| tab.sender().is_some()) else {
                        return Reply::error("gone", "tab closed");
                    };
                    tab.proposals.clone()
                };
                let reply = book.add(&target, text, then.as_deref().cloned(), *ttl, request.actor.clone());
                if !reply.ok {
                    return reply;
                }
                let entry = reply.data.clone().unwrap_or_default();
                let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(tab) = registry.tabs.get_mut(&target).filter(|tab| tab.sender().is_some()) else {
                    let _ = book.finish(entry["id"].as_str().unwrap_or_default(), "withdrawn", None);
                    return Reply::error("gone", "tab closed");
                };
                if tab.sender().unwrap().send(Push::Proposal { entry: entry.clone() }).is_err() {
                    let _ = book.finish(entry["id"].as_str().unwrap_or_default(), "withdrawn", None);
                    return Reply::error("gone", "tab closed");
                }
                drop(registry);
                return if *wait > 0 {
                    book.wait_while(entry["id"].as_str().unwrap_or_default(), *wait, connected)
                } else {
                    reply
                };
            }
            Verb::Answer { id, wait } => {
                let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(tab) = registry.tabs.get(&target).filter(|tab| tab.resumable()) else {
                    return Reply::error("gone", "tab closed");
                };
                let book = tab.proposals.clone();
                drop(registry);
                return if *wait > 0 { book.wait_while(id, *wait, connected) } else { book.get(id) };
            }
            Verb::Withdraw { id } => {
                let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(tab) = registry.tabs.get_mut(&target).filter(|tab| tab.resumable()) else {
                    return Reply::error("gone", "tab closed");
                };
                let reply = tab.proposals.finish(id, "withdrawn", None);
                if reply.ok
                    && let Some(sender) = tab.sender()
                {
                    let _ = sender.send(Push::ProposalClosed { id: id.clone() });
                }
                return reply;
            }
            _ => {}
        }
        self.dispatch(&target, serde_json::to_value(&request).unwrap(), request.actor.clone(), request.deadline_ms)
    }

    /// All browser requests use one envelope and one pending/reply lifecycle, including read-only validation.
    fn dispatch(&self, target: &str, request: serde_json::Value, actor: Option<String>, deadline_ms: u64) -> Reply {
        if deadline_ms <= wire::now_ms() {
            return Reply::error("timeout", "control request expired");
        }
        let (answer, receiver) = mpsc::channel();
        let seq = {
            let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            registry.next_seq += 1;
            let seq = registry.next_seq;
            let Some(tab) = registry.tabs.get_mut(target).filter(|tab| tab.sender().is_some()) else {
                return Reply::error("gone", "tab closed");
            };
            tab.pending.insert(seq, Pending { answer, actor });
            if tab.sender().unwrap().send(Push::Ctl { seq, request }).is_err() {
                tab.pending.remove(&seq);
                return Reply::error("gone", "tab closed");
            }
            seq
        };
        let wait = Duration::from_millis(deadline_ms.saturating_sub(wire::now_ms()).min(30_000));
        let result =
            receiver.recv_timeout(wait).unwrap_or_else(|_| Reply::error("timeout", "tab did not answer in time"));
        if let Some(tab) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).tabs.get_mut(target) {
            tab.pending.remove(&seq);
        }
        result
    }

    fn windows(&self, target: Option<&str>) -> Vec<serde_json::Value> {
        let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .tabs
            .iter()
            .filter(|(id, tab)| tab.sender().is_some() || (target == Some(id.as_str()) && tab.resumable()))
            .map(|(id, tab)| {
                json!({
                    "id": id, "kind": "web", "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION"),
                    "proto": wire::PROTO, "root": tab.root, "here": tab.here, "layer": tab.overlay,
                    "screen": tab.screen, "last_input_ago_ms": tab.last_input.elapsed().as_millis() as u64,
                    "opened_ago_ms": tab.opened.elapsed().as_millis() as u64,
                })
            })
            .collect()
    }

    pub fn seen(&self, id: &str, token: &str, screen: &str, human: bool) -> Result<(), &'static str> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        // The page's own call reaches a tab that is reconnecting: the focus that follows showing it again usually
        // arrives before the resumed stream does, and dropping it lost the person's return as input.
        let Some(tab) = registry.tabs.get_mut(id).filter(|tab| tab.resumable()) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        if !matches!(screen, "board" | "list" | "gantt" | "wiki") {
            return Err("bad_input");
        }
        if human {
            tab.last_input = Instant::now();
        }
        tab.screen = screen.to_string();
        Ok(())
    }

    pub fn reply(&self, id: &str, token: &str, seq: u64, reply: Reply) -> Result<(), &'static str> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tab) = registry.tabs.get_mut(id) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        let Some(pending) = tab.pending.remove(&seq) else { return Err("gone") };
        let _ = pending.answer.send(reply);
        Ok(())
    }

    /// The caller's identity is attached to this specific live request, not to the web server.
    pub fn actor(&self, token: &str, seq: u64, here: &Path) -> Result<String, &'static str> {
        let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tab) = registry.tabs.values().find(|tab| tab.token == token) else { return Err("forbidden") };
        if !crate::user_config::same_dir(Path::new(&tab.here), here) {
            return Err("forbidden");
        }
        let Some(pending) = tab.pending.get(&seq) else { return Err("gone") };
        pending.actor.clone().ok_or("no_actor")
    }

    pub fn proposal_claim(&self, id: &str, token: &str, proposal: &str) -> Result<Reply, &'static str> {
        let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tab) = registry.tabs.get(id).filter(|tab| tab.resumable()) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        let reply = tab.proposals.claim(proposal);
        if reply.ok
            && let Some(sender) = tab.sender()
        {
            let _ = sender.send(Push::ProposalClosed { id: proposal.into() });
        }
        Ok(reply)
    }

    pub fn proposal_reply(
        &self,
        id: &str,
        token: &str,
        proposal: &str,
        accepted: bool,
        ran: Option<Reply>,
    ) -> Result<Reply, &'static str> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        // Claims and their results remain valid during the reconnect grace. After it, no new action is authorized.
        let Some(tab) = registry.tabs.get_mut(id).filter(|tab| tab.resumable()) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        let reply = if accepted {
            tab.proposals.complete(proposal, ran)
        } else {
            tab.proposals.finish(proposal, "declined", None)
        };
        if reply.ok
            && let Some(sender) = tab.sender()
        {
            let _ = sender.send(Push::ProposalClosed { id: proposal.into() });
        }
        Ok(reply)
    }
}

impl Tab {
    fn sender(&self) -> Option<&mpsc::Sender<Push>> {
        match &self.connection {
            Connection::Connected { sender, .. } => Some(sender),
            Connection::Detached { .. } => None,
        }
    }

    fn connect(&mut self, sender: mpsc::Sender<Push>, stream: u64) {
        self.connection = Connection::Connected { sender, stream };
        self.proposals.close_at(None);
    }

    /// Token calls and proposal wait/withdraw can reach a reconnecting tab; new actions need an open stream.
    fn resumable(&self) -> bool {
        match self.connection {
            Connection::Connected { .. } => true,
            Connection::Detached { at } => at.elapsed() < RESUME_GRACE,
        }
    }
}

impl Registry {
    fn occupied(&self) -> usize {
        self.tabs.len() + self.anonymous_streams
    }

    fn prune(&mut self) {
        self.tabs.retain(|_, tab| {
            if !tab.resumable() {
                tab.proposals.close();
                false
            } else {
                true
            }
        });
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn proposal(target: &str, then: Option<Verb>) -> Request {
        let mut request = wire::request(
            Verb::Propose { text: "May I change this?".into(), then: then.map(Box::new), ttl: 900, wait: 0 },
            Duration::from_secs(10),
        );
        request.target = Some(target.into());
        request
    }

    #[test]
    fn reconnect_slots_are_reserved_and_only_valid_resumes_can_reuse_them() {
        let tabs = Tabs::new();
        let here = Path::new("/tmp");
        let first = tabs.stream(here, false, None, true, 3).unwrap().unwrap();
        let joined = first.joined.as_ref().unwrap();
        let (id, token, old_stream) = (joined.id.clone(), joined.token.clone(), joined.stream);
        let _others: Vec<_> = (0..2).map(|_| tabs.stream(here, false, None, true, 3).unwrap().unwrap()).collect();
        let proposal_id = tabs.handle(proposal(&id, None)).data.unwrap()["id"].as_str().unwrap().to_string();
        drop(first);
        assert!(tabs.windows(None).iter().all(|row| row["id"] != id));
        assert!(tabs.stream(here, false, None, true, 3).unwrap().is_none(), "an overflow tab stole the resume slot");
        assert!(
            tabs.stream(here, false, None, false, 3).unwrap().is_none(),
            "an anonymous stream stole the resume slot"
        );
        assert!(tabs.stream(here, false, Some((&id, "wrong")), true, 3).unwrap().is_none());
        let other = crate::scratch::Scratch::new("stream-resume-other-project");
        assert!(tabs.stream(other.path(), false, Some((&id, &token)), true, 3).unwrap().is_none());
        let resumed = tabs.stream(here, true, Some((&id, &token)), true, 3).unwrap().unwrap();
        let joined = resumed.joined.as_ref().unwrap();
        assert_eq!((&joined.id, &joined.token), (&id, &token));
        assert!(matches!(joined.incoming.try_recv(), Ok(Push::Proposal { entry }) if entry["id"] == proposal_id));
        tabs.leave(&id, old_stream);
        assert!(tabs.windows(None).iter().any(|row| row["id"] == id), "a stale leave detached the resumed stream");
        drop(resumed);
        tabs.inner.lock().unwrap().tabs.get_mut(&id).unwrap().connection =
            Connection::Detached { at: Instant::now() - RESUME_GRACE };
        let fresh = tabs.stream(here, false, None, true, 3).unwrap().unwrap();
        assert_ne!(fresh.joined.as_ref().unwrap().id, id);
        assert!(!tabs.inner.lock().unwrap().tabs.contains_key(&id), "an expired reservation still consumed capacity");
    }

    #[test]
    fn anonymous_streams_return_capacity_on_close_without_a_resume_grace() {
        let tabs = Tabs::new();
        let stream = tabs.stream(Path::new("/tmp"), false, None, false, 1).unwrap().unwrap();
        assert!(stream.joined.is_none());
        assert!(tabs.stream(Path::new("/tmp"), false, None, true, 1).unwrap().is_none());
        drop(stream);
        assert!(tabs.stream(Path::new("/tmp"), false, None, true, 1).unwrap().is_some());
    }

    #[test]
    fn a_visible_reconnect_keeps_the_tab_token_proposals_and_input_age() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let answer = tabs.handle(proposal(&first.id, None));
        let proposal_id = answer.data.unwrap()["id"].as_str().unwrap().to_string();
        tabs.inner.lock().unwrap().tabs.get_mut(&first.id).unwrap().last_input -= Duration::from_secs(10);
        tabs.leave(&first.id, first.stream);
        assert!(
            tabs.handle(wire::request(Verb::Hello, Duration::from_secs(1))).data.unwrap()["windows"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let resumed = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert_eq!((resumed.id.as_str(), resumed.token.as_str()), (first.id.as_str(), first.token.as_str()));
        assert!(matches!(resumed.incoming.try_recv(), Ok(Push::Proposal { entry }) if entry["id"] == proposal_id));
        tabs.seen(&resumed.id, &resumed.token, "board", false).unwrap();
        let listed = tabs.handle(wire::request(Verb::Hello, Duration::from_secs(1))).data.unwrap();
        assert!(listed["windows"][0]["last_input_ago_ms"].as_u64().unwrap() >= 10_000);
        assert_eq!(listed["windows"][0]["screen"], "board");
        tabs.leave(&first.id, first.stream);
        assert!(tabs.handle(proposal(&first.id, None)).ok, "an old stream detached the resumed one");
    }

    #[test]
    fn a_withdrawn_proposal_does_not_return_after_a_detached_tab_resumes() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let id = tabs.handle(proposal(&first.id, None)).data.unwrap()["id"].as_str().unwrap().to_string();
        tabs.leave(&first.id, first.stream);
        assert!(tabs.windows(None).is_empty());
        let mut hello = wire::request(Verb::Hello, Duration::from_secs(1));
        hello.target = Some(first.id.clone());
        assert_eq!(tabs.handle(hello).data.unwrap()["windows"][0]["id"], first.id);
        let mut withdraw = wire::request(Verb::Withdraw { id: id.clone() }, Duration::from_secs(1));
        withdraw.target = Some(first.id.clone());
        assert_eq!(tabs.handle(withdraw).data.unwrap()["state"], "withdrawn");
        let mut answer = wire::request(Verb::Answer { id, wait: 0 }, Duration::from_secs(1));
        answer.target = Some(first.id.clone());
        assert_eq!(tabs.handle(answer).data.unwrap()["state"], "withdrawn");
        assert_eq!(tabs.handle(proposal(&first.id, None)).code.as_deref(), Some("gone"));
        let resumed = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert!(resumed.incoming.try_recv().is_err());
        tabs.leave(&resumed.id, resumed.stream);
        tabs.inner.lock().unwrap().tabs.get_mut(&resumed.id).unwrap().connection =
            Connection::Detached { at: Instant::now() - RESUME_GRACE };
        assert!(tabs.windows(Some(&resumed.id)).is_empty());
    }

    #[test]
    fn an_overlay_reconnect_keeps_the_tab_and_reports_the_new_layer() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let id = tabs.handle(proposal(&first.id, None)).data.unwrap()["id"].as_str().unwrap().to_string();
        assert!(matches!(first.incoming.try_recv(), Ok(Push::Proposal { .. })));
        // The page turns the worktree overlay on: it opens a stream with `worktree=1` before the old one is seen closing.
        let resumed = tabs.join(Path::new("/tmp"), true, Some((&first.id, &first.token))).unwrap();
        assert_eq!(resumed.id, first.id);
        assert!(matches!(resumed.incoming.try_recv(), Ok(Push::Proposal { entry }) if entry["id"] == id));
        let listed = tabs.handle(wire::request(Verb::Hello, Duration::from_secs(1))).data.unwrap();
        assert_eq!(listed["windows"][0]["layer"], true);
        // The replaced stream's channel is closed, so its loop ends (web::stream_events) instead of idling on it.
        assert!(matches!(first.incoming.try_recv(), Err(mpsc::TryRecvError::Disconnected)));
        // Its late leave does not detach the resumed stream.
        tabs.leave(&first.id, first.stream);
        assert!(tabs.handle(proposal(&first.id, None)).ok);
    }

    #[test]
    fn a_waiter_rides_out_a_resume_and_hears_gone_once_the_grace_ends() {
        let tabs = Arc::new(Tabs::new());
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let waiting = |target: &str| {
            let mut request = proposal(target, None);
            if let Verb::Propose { wait, .. } = &mut request.verb {
                *wait = 30;
            }
            let of = Arc::clone(&tabs);
            std::thread::spawn(move || of.handle(request))
        };
        let shown = |joined: &Joined, id: &str| loop {
            match joined.incoming.recv_timeout(Duration::from_secs(2)) {
                Ok(Push::Proposal { entry }) if entry["id"] == id => break,
                Ok(_) => continue,
                Err(e) => panic!("proposal {id} was not pushed: {e}"),
            }
        };
        // The stream ends and resumes: the waiter is still there for the answer given through the resumed tab.
        let answered = waiting(&first.id);
        shown(&first, &format!("{}/1", first.id));
        tabs.leave(&first.id, first.stream);
        let book = tabs.inner.lock().unwrap().tabs[&first.id].proposals.clone();
        assert!(book.closes_at().is_some(), "leave scheduled no end for the tab's waiters");
        let resumed = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert_eq!(book.closes_at(), None, "the resume kept the scheduled end");
        assert!(tabs.proposal_claim(&resumed.id, &resumed.token, &format!("{}/1", first.id)).unwrap().ok);
        tabs.proposal_reply(&resumed.id, &resumed.token, &format!("{}/1", first.id), true, None).unwrap();
        assert_eq!(answered.join().unwrap().data.unwrap()["state"], "accepted");
        // The stream ends for good: the waiter hears `gone` once the grace has run out, though no tab ever joins again.
        let orphaned = waiting(&resumed.id);
        shown(&resumed, &format!("{}/2", resumed.id));
        tabs.leave(&resumed.id, resumed.stream);
        std::thread::sleep(Duration::from_millis(300));
        assert!(!orphaned.is_finished(), "the waiter left before the grace ran out");
        book.close_at(Some(Instant::now())); // as when RESUME_GRACE has passed
        assert_eq!(orphaned.join().unwrap().code.as_deref(), Some("gone"));
    }

    #[test]
    fn the_page_answers_and_reports_input_while_its_stream_reconnects() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let id = tabs.handle(proposal(&first.id, None)).data.unwrap()["id"].as_str().unwrap().to_string();
        tabs.inner.lock().unwrap().tabs.get_mut(&first.id).unwrap().last_input -= Duration::from_secs(10);
        tabs.leave(&first.id, first.stream);
        // Agents cannot reach the tab meanwhile …
        assert_eq!(tabs.handle(proposal(&first.id, None)).code.as_deref(), Some("gone"));
        // … but the page's own calls land: the person came back to the tab and answered its card.
        tabs.seen(&first.id, &first.token, "board", true).unwrap();
        assert!(tabs.proposal_claim(&first.id, &first.token, &id).unwrap().ok);
        let answer = tabs.proposal_reply(&first.id, &first.token, &id, true, None).unwrap();
        assert_eq!(answer.data.unwrap()["state"], "accepted");
        let resumed = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert!(resumed.incoming.try_recv().is_err(), "an answered proposal came back on resume");
        let listed = tabs.handle(wire::request(Verb::Hello, Duration::from_secs(1))).data.unwrap();
        assert!(listed["windows"][0]["last_input_ago_ms"].as_u64().unwrap() < 10_000);
        // Past the grace the tab is gone for the page as well.
        tabs.leave(&resumed.id, resumed.stream);
        tabs.inner.lock().unwrap().tabs.get_mut(&first.id).unwrap().connection =
            Connection::Detached { at: Instant::now() - RESUME_GRACE };
        assert_eq!(tabs.seen(&first.id, &first.token, "list", true), Err("gone"));
    }

    #[test]
    fn a_web_zone_is_validated_by_the_browser_before_the_proposal_is_added() {
        let tabs = Arc::new(Tabs::new());
        let joined = tabs.join(Path::new("/tmp"), false, None).unwrap();
        for (name, valid) in [("Mars/Olympus", false), ("UTC", true)] {
            let request = proposal(&joined.id, Some(Verb::Zone { name: name.into() }));
            let of = Arc::clone(&tabs);
            let running = std::thread::spawn(move || of.handle(request));
            let Push::Ctl { seq, request: sent } = joined.incoming.recv_timeout(Duration::from_secs(2)).unwrap() else {
                panic!("zone validation was not sent to the tab");
            };
            assert_eq!(sent["verb"], "validate_zone");
            assert_eq!(sent["name"], name);
            assert!(sent["deadline_ms"].as_u64().unwrap() > wire::now_ms());
            let reply = if valid { Reply::success(json!({})) } else { Reply::error("bad_input", "unknown time zone") };
            tabs.reply(&joined.id, &joined.token, seq, reply).unwrap();
            let answer = running.join().unwrap();
            assert_eq!(answer.ok, valid);
            if valid {
                assert!(matches!(joined.incoming.recv_timeout(Duration::from_secs(2)), Ok(Push::Proposal { .. })));
            } else {
                assert!(tabs.inner.lock().unwrap().tabs.get(&joined.id).unwrap().proposals.rows().is_empty());
            }
        }
    }

    #[test]
    fn a_claim_needs_a_live_token_and_cannot_race_with_withdrawal() {
        let tabs = Tabs::new();
        let tab = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let id = tabs.handle(proposal(&tab.id, None)).data.unwrap()["id"].as_str().unwrap().to_string();
        assert_eq!(tabs.proposal_claim(&tab.id, "wrong", &id).unwrap_err(), "forbidden");
        assert!(!tabs.proposal_reply(&tab.id, &tab.token, &id, true, None).unwrap().ok);
        assert!(tabs.proposal_claim(&tab.id, &tab.token, &id).unwrap().ok);
        assert!(!tabs.proposal_claim(&tab.id, &tab.token, &id).unwrap().ok);
        let mut withdraw = wire::request(Verb::Withdraw { id: id.clone() }, Duration::from_secs(1));
        withdraw.target = Some(tab.id.clone());
        assert!(!tabs.handle(withdraw).ok);
        assert!(tabs.proposal_reply(&tab.id, &tab.token, &id, true, Some(Reply::success(json!({})))).unwrap().ok);
        tabs.leave(&tab.id, tab.stream);
        tabs.inner.lock().unwrap().tabs.get_mut(&tab.id).unwrap().connection =
            Connection::Detached { at: Instant::now() - RESUME_GRACE };
        assert_eq!(tabs.proposal_claim(&tab.id, &tab.token, &id).unwrap_err(), "gone");
    }

    #[test]
    fn expired_validation_and_replaced_streams_clear_the_same_pending_queue() {
        let tabs = Arc::new(Tabs::new());
        let tab = tabs.join(Path::new("/tmp"), false, None).unwrap();
        let request = json!({"verb": "validate_zone", "name": "UTC", "deadline_ms": wire::now_ms() - 1});
        assert_eq!(tabs.dispatch(&tab.id, request, None, wire::now_ms() - 1).code.as_deref(), Some("timeout"));
        assert!(tab.incoming.try_recv().is_err());
        let of = tabs.clone();
        let id = tab.id.clone();
        let waiting = std::thread::spawn(move || {
            of.dispatch(&id, json!({"verb": "validate_zone", "name": "UTC"}), None, wire::now_ms() + 5000)
        });
        let Push::Ctl { seq, .. } = tab.incoming.recv_timeout(Duration::from_secs(2)).unwrap() else {
            panic!("missing envelope")
        };
        let _resumed = tabs.join(Path::new("/tmp"), false, Some((&tab.id, &tab.token))).unwrap();
        assert_eq!(waiting.join().unwrap().code.as_deref(), Some("gone"));
        assert_eq!(tabs.reply(&tab.id, &tab.token, seq, Reply::success(json!({}))), Err("gone"));
        assert!(tabs.inner.lock().unwrap().tabs[&tab.id].pending.is_empty());
    }

    #[test]
    fn a_resume_token_expires_and_cannot_claim_a_different_tab() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        tabs.leave(&first.id, first.stream);
        let wrong = tabs.join(Path::new("/tmp"), false, Some((&first.id, "00000000000000000000000000000000"))).unwrap();
        assert_ne!(wrong.id, first.id);
        tabs.inner.lock().unwrap().tabs.get_mut(&first.id).unwrap().connection =
            Connection::Detached { at: Instant::now() - RESUME_GRACE - Duration::from_secs(1) };
        let expired = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert_ne!(expired.id, first.id);
        assert!(!tabs.inner.lock().unwrap().tabs.contains_key(&first.id));
    }
}
