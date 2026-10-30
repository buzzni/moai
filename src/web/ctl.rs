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
    tabs: HashMap<String, Tab>,
}

struct Tab {
    token: String,
    root: String,
    here: String,
    overlay: bool,
    sender: mpsc::Sender<Push>,
    opened: Instant,
    last_input: Instant,
    screen: String,
    pending: HashMap<u64, Pending>,
}

struct Pending {
    answer: mpsc::Sender<Reply>,
    actor: Option<String>,
}

pub struct Joined {
    pub id: String,
    pub token: String,
    pub incoming: mpsc::Receiver<Push>,
}

pub struct Push {
    pub seq: u64,
    pub request: Request,
}

impl Tabs {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Registry::default()) }
    }

    pub fn join(&self, here: &Path, overlay: bool) -> std::io::Result<Joined> {
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let (sender, incoming) = mpsc::channel();
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        registry.next_tab += 1;
        let id = format!("{}.{}", std::process::id(), registry.next_tab);
        registry.tabs.insert(
            id.clone(),
            Tab {
                token: token.clone(),
                root: crate::store::Repo::opened_root(here).display().to_string(),
                here: here.display().to_string(),
                overlay,
                sender,
                opened: Instant::now(),
                last_input: Instant::now(),
                screen: "list".into(),
                pending: HashMap::new(),
            },
        );
        Ok(Joined { id, token, incoming })
    }

    pub fn leave(&self, id: &str) {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tab) = registry.tabs.remove(id) {
            for (_, pending) in tab.pending {
                let _ = pending.answer.send(Reply::error("gone", "tab closed"));
            }
        }
    }

    pub fn handle(&self, request: Request) -> Reply {
        if matches!(request.verb, Verb::Hello) {
            return Reply::success(json!({"windows": self.windows()}));
        }
        let Some(target) = request.target.clone() else { return Reply::error("bad_input", "target tab is required") };
        let (answer, receiver) = mpsc::channel();
        let seq = {
            let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            registry.next_seq += 1;
            let seq = registry.next_seq;
            let Some(tab) = registry.tabs.get_mut(&target) else { return Reply::error("gone", "tab closed") };
            tab.pending.insert(seq, Pending { answer, actor: request.actor.clone() });
            if tab.sender.send(Push { seq, request: request.clone() }).is_err() {
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

    pub fn seen(&self, id: &str, token: &str, screen: &str) -> Result<(), &'static str> {
        let mut registry = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(tab) = registry.tabs.get_mut(id) else { return Err("gone") };
        if tab.token != token {
            return Err("forbidden");
        }
        if !matches!(screen, "board" | "list" | "gantt" | "wiki") {
            return Err("bad_input");
        }
        tab.last_input = Instant::now();
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
}
