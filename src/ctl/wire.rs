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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum Verb {
    Hello,
    Propose {
        text: String,
        then: Option<Box<Verb>>,
        ttl: u64,
        wait: u64,
    },
    Answer {
        #[serde(rename = "proposal")]
        id: String,
        wait: u64,
    },
    Withdraw {
        #[serde(rename = "proposal")]
        id: String,
    },
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
        #[serde(rename = "issue")]
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
        gantt_grid: Option<String>,
    },
    Zone {
        name: String,
    },
    Theme {
        name: String,
    },
}

impl Verb {
    pub fn proposable(&self) -> bool {
        matches!(
            self,
            Self::Do { .. }
                | Self::Sort { .. }
                | Self::Filter { .. }
                | Self::Find { .. }
                | Self::Screen { .. }
                | Self::View { .. }
                | Self::Cell { .. }
                | Self::Detail { .. }
                | Self::Goto { .. }
                | Self::Read { .. }
                | Self::Wiki { .. }
                | Self::Pane { .. }
                | Self::Zone { .. }
                | Self::Theme { .. }
        )
    }

    pub fn describe(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::say;
        match self {
            Self::Do { name } => format!("{} {name}", say(lang, "tui.propose.verb.do")),
            // The words name what accepting does — `--default`, `--in`, `--enter`, a read's scope and pane values
            // decide it, so they are spelled out as the CLI takes them.
            Self::Sort { words, default } => {
                format!("{} {}{}", say(lang, "tui.act.sort"), words.join(","), if *default { " --default" } else { "" })
            }
            Self::Filter { query: Some(query) } => format!("{} {query}", say(lang, "tui.propose.verb.filter")),
            Self::Filter { query: None } => say(lang, "tui.propose.verb.clear").into(),
            Self::Find { query, scope } if scope != "all" => {
                format!("{} {query} --in {scope}", say(lang, "tui.act.grep"))
            }
            Self::Find { query, .. } => format!("{} {query}", say(lang, "tui.act.grep")),
            Self::Screen { name } => {
                use crate::tui::keys::Screen;
                let screen = match name.as_str() {
                    "list" => Some(Screen::List),
                    "board" => Some(Screen::Board),
                    "gantt" => Some(Screen::Gantt),
                    "stats" => Some(Screen::Stats),
                    "wiki" => Some(Screen::Wiki),
                    _ => None,
                };
                screen.map_or_else(|| name.clone(), |screen| screen.word(lang).into())
            }
            Self::View { name, on } => {
                format!("{} {name} {}", say(lang, "tui.group.view"), if *on { "on" } else { "off" })
            }
            Self::Cell { name, on } => {
                format!("{} {name} {}", say(lang, "tui.group.cell"), if *on { "on" } else { "off" })
            }
            Self::Detail { name } => format!("{} {name}", say(lang, "tui.act.detail")),
            Self::Goto { id, project, enter } => {
                format!(
                    "{} {}{}",
                    say(lang, "tui.propose.verb.goto"),
                    id.as_ref().or(project.as_ref()).map(String::as_str).unwrap_or(""),
                    if *enter { " --enter" } else { "" }
                )
            }
            Self::Read { ids, all, group } => {
                let what = match (all, group) {
                    (true, _) => "--all".to_string(),
                    (false, Some(group)) => format!("--group {group}"),
                    (false, None) => ids.join(" "),
                };
                format!("{} {what}", say(lang, "tui.act.read"))
            }
            Self::Wiki { slug, search } => {
                format!(
                    "{} {}",
                    say(lang, "tui.act.wiki"),
                    slug.as_ref().or(search.as_ref()).map(String::as_str).unwrap_or("")
                )
            }
            Self::Pane { list_width, list_height, wiki_width, gantt_label, gantt_grid } => {
                let set: Vec<String> = [
                    ("--list-width", list_width),
                    ("--list-height", list_height),
                    ("--wiki-width", wiki_width),
                    ("--gantt-label", gantt_label),
                    ("--gantt-grid", gantt_grid),
                ]
                .into_iter()
                .filter_map(|(flag, value)| value.as_ref().map(|value| format!("{flag} {value}")))
                .collect();
                format!("{} {}", say(lang, "tui.propose.verb.pane"), set.join(" "))
            }
            Self::Zone { name } => format!("{} {name}", say(lang, "tui.propose.verb.zone")),
            Self::Theme { name } => format!("{} {name}", say(lang, "tui.propose.verb.theme")),
            _ => "".into(),
        }
    }
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
        target: None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn request_id_does_not_collide_with_a_proposal_or_issue_id() {
        for (verb, field) in [
            (super::Verb::Answer { id: "window/1".into(), wait: 0 }, "proposal"),
            (super::Verb::Withdraw { id: "window/1".into() }, "proposal"),
            (super::Verb::Goto { id: Some("moai-example".into()), enter: false, project: None }, "issue"),
        ] {
            let mut request = super::request(verb, std::time::Duration::from_secs(5));
            request.id = 42;
            let encoded = serde_json::to_value(&request).unwrap();
            assert_eq!(encoded["id"], 42);
            assert!(encoded[field].is_string(), "{encoded}");
            assert_eq!(serde_json::from_value::<super::Request>(encoded).unwrap().id, 42);
        }
    }

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
                "hello", "propose", "answer", "withdraw", "state", "keys", "do", "do", "actions", "sort", "filter",
                "find", "screen", "view", "cell", "detail", "goto", "read", "wiki", "pane", "pane", "zone", "theme"
            ]
        );
    }
}
