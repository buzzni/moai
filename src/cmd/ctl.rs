//! CLI rendering for the local control transport; selection lives in `ctl::call`.

use super::{Ctx, R};
use crate::ctl::{call, wire};
use crate::fail::{Fail, code};
use std::time::Duration;

pub fn ls(ctx: &Ctx, all: bool) -> R<Vec<String>> {
    let root = call::current_root();
    let windows: Vec<_> =
        call::windows()?.into_iter().filter(|w| all || root.as_deref().is_none_or(|r| w.root == r)).collect();
    if ctx.json {
        return super::json_line(&serde_json::json!({"windows": windows}));
    }
    let mut lines = vec!["ID  KIND  REPOSITORY  SCREEN  LAST INPUT".into()];
    lines.extend(
        windows.iter().map(|w| format!("{}  {}  {}  {}  {}ms", w.id, w.kind, w.root, w.screen, w.last_input_ago_ms)),
    );
    Ok(lines)
}

pub fn state(ctx: &Ctx, target: Option<&str>, rows: bool) -> R<Vec<String>> {
    #[cfg(not(unix))]
    return Err(Fail::coded("control sockets are unavailable on this OS", code::UNSUPPORTED));
    #[cfg(unix)]
    {
        let windows = call::windows()?;
        let root = call::current_root();
        let window = call::pick(&windows, root.as_deref(), target)?;
        let reply = call::send(window, &wire::request(wire::Verb::State { rows }, Duration::from_secs(5)))?;
        if !reply.ok {
            return Err(Fail::coded(
                reply.message.unwrap_or_else(|| "window refused".into()),
                code_of(reply.code.as_deref()),
            ));
        }
        let data = reply.data.unwrap_or(serde_json::Value::Null);
        if ctx.json {
            return super::json_line(&data);
        }
        Ok(vec![format!("{}: {}", window.id, data)])
    }
}

pub fn keys(ctx: &Ctx, target: Option<&str>, seqs: Vec<String>, text: Option<String>) -> R<Vec<String>> {
    if seqs.is_empty() && text.is_none() {
        return Err(Fail::coded("give a key sequence or --text", code::BAD_INPUT));
    }
    for seq in &seqs {
        if crate::tui::keys::parse_seq(seq).is_none() {
            return Err(Fail::coded(format!("unknown key sequence: {seq}"), code::BAD_INPUT));
        }
    }
    invoke(ctx, target, wire::Verb::Keys { seqs, text }, true)
}

pub fn named(ctx: &Ctx, target: Option<&str>, name: String) -> R<Vec<String>> {
    invoke(ctx, target, wire::Verb::Do { name }, true)
}

pub fn actions(ctx: &Ctx, target: Option<&str>) -> R<Vec<String>> {
    invoke(ctx, target, wire::Verb::Actions, false)
}

fn invoke(ctx: &Ctx, target: Option<&str>, verb: wire::Verb, needs_actor: bool) -> R<Vec<String>> {
    #[cfg(not(unix))]
    return Err(Fail::coded("control sockets are unavailable on this OS", code::UNSUPPORTED));
    #[cfg(unix)]
    {
        let windows = call::windows()?;
        let root = call::current_root();
        let window = call::pick(&windows, root.as_deref(), target)?;
        let mut request = wire::request(verb, Duration::from_secs(5));
        request.lang = format!("{:?}", ctx.lang()).to_lowercase();
        if needs_actor {
            request.actor = match crate::model::actor(ctx.user.as_deref(), std::path::Path::new(&window.root)) {
                Ok(a) => Some(format!("{} ({})", a.name, a.email)),
                Err(e @ crate::model::NoActor::Malformed { .. }) => return Err(Fail::no_actor(&e, ctx.lang())),
                Err(_) => None,
            };
        }
        let reply = call::send(window, &request)?;
        if !reply.ok {
            return Err(Fail::coded(
                reply.message.unwrap_or_else(|| "window refused".into()),
                code_of(reply.code.as_deref()),
            ));
        }
        let data = reply.data.unwrap_or(serde_json::Value::Null);
        if ctx.json {
            return super::json_line(&data);
        }
        if let wire::Verb::Actions = request.verb {
            return Ok(data["actions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| {
                    let name = a["name"].as_str().unwrap_or_default();
                    let keys =
                        a["keys"].as_array().into_iter().flatten().filter_map(|k| k.as_str()).collect::<Vec<_>>();
                    let keys = if keys.is_empty() { "(unbound)".into() } else { keys.join(", ") };
                    if a["enabled"] == false {
                        match a["why"].as_str() {
                            Some(why) => format!("· {name}  {keys}  (off: {why})"),
                            None => format!("· {name}  {keys}  (off)"),
                        }
                    } else {
                        format!("  {name}  {keys}")
                    }
                })
                .collect());
        }
        Ok(vec![data.to_string()])
    }
}

fn code_of(code_name: Option<&str>) -> &'static str {
    match code_name {
        Some("no_window") => code::NO_WINDOW,
        Some("gone") => code::GONE,
        Some("person_busy") => code::PERSON_BUSY,
        Some("unsupported") => code::UNSUPPORTED,
        Some("bad_input") => code::BAD_INPUT,
        Some("timeout") => code::TIMEOUT,
        Some("busy") => code::BUSY,
        Some("no_actor") => code::NO_ACTOR,
        _ => code::REFUSED,
    }
}
