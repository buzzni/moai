use serde::{Deserialize, Serialize};

pub const PROTO: u32 = 1;
pub const LINE_MAX: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub proto: u32,
    pub id: u64,
    #[serde(flatten)]
    pub verb: Verb,
    pub actor: Option<String>,
    pub deadline_ms: u64,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum Verb {
    Hello,
    State { rows: bool },
    Keys { seqs: Vec<String>, text: Option<String> },
    Do { name: String },
    Actions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Reply {
    pub fn success(data: serde_json::Value) -> Self {
        Self { ok: true, code: None, message: None, data: Some(data) }
    }
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self { ok: false, code: Some(code.into()), message: Some(message.into()), data: None }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

pub fn request(verb: Verb, timeout: std::time::Duration) -> Request {
    Request {
        proto: PROTO,
        id: now_ms(),
        verb,
        actor: None,
        deadline_ms: now_ms().saturating_add(timeout.as_millis() as u64),
        lang: "en".into(),
    }
}
