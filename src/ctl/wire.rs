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
    State {
        rows: bool,
    },
    Keys {
        seqs: Vec<String>,
        text: Option<String>,
    },
    Do {
        name: String,
    },
    Actions,
    Sort {
        words: Vec<String>,
        default: bool,
    },
    Filter {
        query: Option<String>,
    },
    Find {
        query: String,
        scope: String,
    },
    Screen {
        name: String,
    },
    View {
        name: String,
        on: bool,
    },
    Cell {
        name: String,
        on: bool,
    },
    Detail {
        name: String,
    },
    Goto {
        id: Option<String>,
        enter: bool,
        project: Option<String>,
    },
    Read {
        ids: Vec<String>,
        all: bool,
        group: Option<String>,
    },
    Wiki {
        slug: Option<String>,
        search: Option<String>,
    },
    Pane {
        list_width: Option<String>,
        list_height: Option<String>,
        wiki_width: Option<String>,
        gantt_label: Option<String>,
    },
    Zone {
        name: String,
    },
    Theme {
        name: String,
    },
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

#[cfg(test)]
mod tests {
    #[test]
    fn every_ctl_verb_is_in_the_fixture_and_parses() {
        let fixture: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/ctl-verbs.json")).unwrap();
        let mut names = Vec::new();
        for row in fixture {
            let name = row["verb"].as_str().unwrap().to_string();
            let mut wire = row["args"].as_object().unwrap().clone();
            wire.insert("verb".into(), serde_json::json!(name));
            let parsed: super::Verb = serde_json::from_value(serde_json::Value::Object(wire)).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap()["verb"], name);
            assert!(!row["surfaces"].as_array().unwrap().is_empty());
            names.push(name);
        }
        assert_eq!(
            names,
            [
                "hello", "state", "keys", "do", "actions", "sort", "filter", "find", "screen", "view", "cell",
                "detail", "goto", "read", "wiki", "pane", "zone", "theme"
            ]
        );
    }
}
