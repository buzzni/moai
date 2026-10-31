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
}

const RESUME_GRACE: Duration = Duration::from_secs(30);

struct Tab {
    token: String,
    root: String,
    here: String,
    overlay: bool,
    sender: Option<mpsc::Sender<Push>>,
    stream: u64,
    detached: Option<Instant>,
    opened: Instant,
    last_input: Instant,
    screen: String,
    pending: HashMap<u64, Pending>,
    proposals: std::sync::Arc<crate::ctl::proposal::Book>,
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

pub enum Push {
    Ctl { seq: u64, request: Request },
    ValidateZone { seq: u64, name: String },
    Proposal { entry: serde_json::Value },
    ProposalClosed { id: String },
}

impl Tabs {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Registry::default()) }
    }

    pub fn join(&self, here: &Path, overlay: bool, resume: Option<(&str, &str)>) -> std::io::Result<Joined> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry.prune();
        if let Some((id, token)) = resume {
            let (sender, incoming) = mpsc::channel();
            registry.next_stream += 1;
            let stream = registry.next_stream;
            if let Some(tab) = registry.tabs.get_mut(id)
                && tab.token == token
                && crate::user_config::same_dir(Path::new(&tab.here), here)
                && tab.overlay == overlay
            {
                tab.stream = stream;
                tab.detached = None;
                tab.sender = Some(sender.clone());
                for (_, pending) in tab.pending.drain() {
                    let _ = pending.answer.send(Reply::error("gone", "tab stream replaced"));
                }
                for row in tab.proposals.rows() {
                    let _ = sender.send(Push::Proposal { entry: row.value() });
                }
                return Ok(Joined { id: id.into(), token: token.into(), incoming, stream });
            }
        }
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let (sender, incoming) = mpsc::channel();
        registry.next_tab += 1;
        registry.next_stream += 1;
        let stream = registry.next_stream;
        let id = format!("{}.{}", std::process::id(), registry.next_tab);
        registry.tabs.insert(
            id.clone(),
            Tab {
                token: token.clone(),
                root: crate::store::Repo::opened_root(here).display().to_string(),
                here: here.display().to_string(),
                overlay,
                sender: Some(sender),
                stream,
                detached: None,
                opened: Instant::now(),
                last_input: Instant::now(),
                screen: "list".into(),
                pending: HashMap::new(),
                proposals: std::sync::Arc::new(crate::ctl::proposal::Book::new()),
            },
        );
        Ok(Joined { id, token, incoming, stream })
    }

    pub fn leave(&self, id: &str, stream: u64) {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tab) = registry.tabs.get_mut(id)
            && tab.stream == stream
        {
            tab.sender = None;
            tab.detached = Some(Instant::now());
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
            return Reply::success(json!({"windows": self.windows()}));
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
                if let Some(Verb::Zone { name }) = then.as_deref() {
                    let checked = self.validate_zone(&target, name);
                    if !checked.ok {
                        return checked;
                    }
                }
                if request.deadline_ms <= wire::now_ms() {
                    return Reply::error("timeout", "control request expired");
                }
                let book = {
                    let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    let Some(tab) = registry.tabs.get(&target).filter(|tab| tab.sender.is_some()) else {
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
                let Some(tab) = registry.tabs.get_mut(&target).filter(|tab| tab.sender.is_some()) else {
                    let _ = book.finish(entry["id"].as_str().unwrap_or_default(), "withdrawn", None);
                    return Reply::error("gone", "tab closed");
                };
                if tab.sender.as_ref().unwrap().send(Push::Proposal { entry: entry.clone() }).is_err() {
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
                let Some(tab) = registry.tabs.get(&target).filter(|tab| tab.sender.is_some()) else {
                    return Reply::error("gone", "tab closed");
                };
                let book = tab.proposals.clone();
                drop(registry);
                return if *wait > 0 { book.wait_while(id, *wait, connected) } else { book.get(id) };
            }
            Verb::Withdraw { id } => {
                let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(tab) = registry.tabs.get_mut(&target).filter(|tab| tab.sender.is_some()) else {
                    return Reply::error("gone", "tab closed");
                };
                let reply = tab.proposals.finish(id, "withdrawn", None);
                if reply.ok {
                    let _ = tab.sender.as_ref().unwrap().send(Push::ProposalClosed { id: id.clone() });
                }
                return reply;
            }
            _ => {}
        }
        let (answer, receiver) = mpsc::channel();
        let seq = {
            let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            registry.next_seq += 1;
            let seq = registry.next_seq;
            let Some(tab) = registry.tabs.get_mut(&target).filter(|tab| tab.sender.is_some()) else {
                return Reply::error("gone", "tab closed");
            };
            tab.pending.insert(seq, Pending { answer, actor: request.actor.clone() });
            if tab.sender.as_ref().unwrap().send(Push::Ctl { seq, request: request.clone() }).is_err() {
                tab.pending.remove(&seq);
                return Reply::error("gone", "tab closed");
            }
            seq
        };
        let wait = Duration::from_millis(request.deadline_ms.saturating_sub(wire::now_ms()).min(30_000));
        let result =
            receiver.recv_timeout(wait).unwrap_or_else(|_| Reply::error("timeout", "tab did not answer in time"));
        if result.code.as_deref() == Some("timeout")
            && let Some(tab) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).tabs.get_mut(&target)
        {
            tab.pending.remove(&seq);
        }
        result
    }

    fn windows(&self) -> Vec<serde_json::Value> {
        let registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .tabs
            .iter()
            .filter(|(_, tab)| tab.sender.is_some())
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
        let Some(tab) = registry.tabs.get_mut(id).filter(|tab| tab.sender.is_some()) else { return Err("gone") };
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

    pub fn proposal_reply(
        &self,
        id: &str,
        token: &str,
        proposal: &str,
        accepted: bool,
        ran: Option<Reply>,
    ) -> Result<Reply, &'static str> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tab) = registry.tabs.get_mut(id).filter(|tab| tab.sender.is_some()) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        let reply = tab.proposals.finish(proposal, if accepted { "accepted" } else { "declined" }, ran);
        if reply.ok {
            if let Some(sender) = &tab.sender {
                let _ = sender.send(Push::ProposalClosed { id: proposal.into() });
            }
        }
        Ok(reply)
    }

    fn validate_zone(&self, id: &str, name: &str) -> Reply {
        let (answer, receiver) = mpsc::channel();
        let seq = {
            let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            registry.next_seq += 1;
            let seq = registry.next_seq;
            let Some(tab) = registry.tabs.get_mut(id).filter(|tab| tab.sender.is_some()) else {
                return Reply::error("gone", "tab closed");
            };
            tab.pending.insert(seq, Pending { answer, actor: None });
            if tab.sender.as_ref().unwrap().send(Push::ValidateZone { seq, name: name.into() }).is_err() {
                tab.pending.remove(&seq);
                return Reply::error("gone", "tab closed");
            }
            seq
        };
        let result = receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| Reply::error("timeout", "tab did not validate time zone"));
        if let Some(tab) = self.inner.lock().unwrap_or_else(|e| e.into_inner()).tabs.get_mut(id) {
            tab.pending.remove(&seq);
        }
        result
    }
}

impl Registry {
    fn prune(&mut self) {
        self.tabs.retain(|_, tab| {
            if tab.detached.is_some_and(|at| at.elapsed() >= RESUME_GRACE) {
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
        assert_eq!(tabs.handle(proposal(&first.id, None)).ok, true, "an old stream detached the resumed one");
    }

    #[test]
    fn a_web_zone_is_validated_by_the_browser_before_the_proposal_is_added() {
        let tabs = Arc::new(Tabs::new());
        let joined = tabs.join(Path::new("/tmp"), false, None).unwrap();
        for (name, valid) in [("Mars/Olympus", false), ("UTC", true)] {
            let request = proposal(&joined.id, Some(Verb::Zone { name: name.into() }));
            let of = Arc::clone(&tabs);
            let running = std::thread::spawn(move || of.handle(request));
            let Push::ValidateZone { seq, name: sent } = joined.incoming.recv_timeout(Duration::from_secs(2)).unwrap()
            else {
                panic!("zone validation was not sent to the tab");
            };
            assert_eq!(sent, name);
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
    fn a_resume_token_expires_and_cannot_claim_a_different_tab() {
        let tabs = Tabs::new();
        let first = tabs.join(Path::new("/tmp"), false, None).unwrap();
        tabs.leave(&first.id, first.stream);
        let wrong = tabs.join(Path::new("/tmp"), false, Some((&first.id, "00000000000000000000000000000000"))).unwrap();
        assert_ne!(wrong.id, first.id);
        tabs.inner.lock().unwrap().tabs.get_mut(&first.id).unwrap().detached =
            Some(Instant::now() - RESUME_GRACE - Duration::from_secs(1));
        let expired = tabs.join(Path::new("/tmp"), false, Some((&first.id, &first.token))).unwrap();
        assert_ne!(expired.id, first.id);
        assert!(!tabs.inner.lock().unwrap().tabs.contains_key(&first.id));
    }
}
