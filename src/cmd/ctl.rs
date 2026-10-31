//! CLI rendering for the local control transport; selection lives in `ctl::call`.

use super::{Ctx, R};
use crate::cli::{Cli, Cmd, CtlCmd};
use crate::ctl::{call, wire};
use crate::fail::{Fail, code};
use clap::Parser;
use std::time::Duration;

pub fn ls(ctx: &Ctx, all: bool) -> R<Vec<String>> {
    let root = if all { None } else { call::current_root(ctx.lang())? };
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

pub fn propose(
    ctx: &Ctx,
    target: Option<&str>,
    text: String,
    then_cmd: Option<String>,
    wait: u64,
    ttl: u64,
) -> R<Vec<String>> {
    if wait > 3600 || ttl == 0 || ttl > 3600 {
        return Err(Fail::coded(
            "--wait and --ttl must be at most 3600 seconds; ttl must be positive",
            code::BAD_INPUT,
        ));
    }
    let then = then_cmd.as_deref().map(then_verb).transpose()?.map(Box::new);
    unix_only()?;
    let windows = call::windows()?;
    let window = window_for(ctx, &windows, target)?;
    let mut request = wire::request(wire::Verb::Propose { text, then, ttl, wait }, Duration::from_secs(wait + 10));
    request.lang = format!("{:?}", ctx.lang()).to_lowercase();
    request.actor = crate::model::actor(ctx.user.as_deref(), std::path::Path::new(&window.root))
        .ok()
        .map(|a| format!("{} ({})", a.name, a.email));
    proposal_reply(ctx, call::send(window, &request)?)
}

pub fn answer(ctx: &Ctx, target: Option<&str>, id: String, wait: u64) -> R<Vec<String>> {
    if wait > 3600 {
        return Err(Fail::coded("--wait must be at most 3600 seconds", code::BAD_INPUT));
    }
    unix_only()?;
    let windows = call::windows()?;
    let window = window_for(ctx, &windows, target.or_else(|| id.rsplit_once('/').map(|(w, _)| w)))?;
    let request = wire::request(wire::Verb::Answer { id, wait }, Duration::from_secs(wait + 10));
    proposal_reply(ctx, call::send(window, &request)?)
}

pub fn withdraw(ctx: &Ctx, target: Option<&str>, id: String) -> R<Vec<String>> {
    unix_only()?;
    let windows = call::windows()?;
    let window = window_for(ctx, &windows, target.or_else(|| id.rsplit_once('/').map(|(w, _)| w)))?;
    let request = wire::request(wire::Verb::Withdraw { id }, Duration::from_secs(5));
    proposal_reply(ctx, call::send(window, &request)?)
}

/// The window a request goes to: the one `-t` names, else the most recent of the caller's repository. The root is read
/// only when no id names the window, so a broken tracker here never blocks an explicit `-t`.
fn window_for<'a>(ctx: &Ctx, windows: &'a [call::Window], target: Option<&str>) -> R<&'a call::Window> {
    let root = if target.is_some() { None } else { call::current_root(ctx.lang())? };
    call::pick(windows, root.as_deref(), target)
}

/// A command that needs a window says `unsupported` where no window can listen, as `state` and the verbs do; `ls`
/// alone answers an empty list there.
fn unix_only() -> R<()> {
    if cfg!(unix) { Ok(()) } else { Err(Fail::coded("control sockets are unavailable on this OS", code::UNSUPPORTED)) }
}

fn proposal_reply(ctx: &Ctx, reply: wire::Reply) -> R<Vec<String>> {
    if !reply.ok {
        return Err(Fail::coded(
            reply.message.unwrap_or_else(|| "proposal refused".into()),
            code_of(reply.code.as_deref()),
        ));
    }
    let data = reply.data.unwrap_or(serde_json::Value::Null);
    if ctx.json { super::json_line(&data) } else { Ok(vec![data.to_string()]) }
}

fn then_verb(text: &str) -> R<wire::Verb> {
    let words = split_words(text)?;
    if words.is_empty() {
        return Err(Fail::coded("--then needs a ctl verb", code::BAD_INPUT));
    }
    // Filters are naturally a phrase; the outer --then quotes already delimit it.
    let words = if words[0] == "filter" && words.len() > 2 && !words[1].starts_with('-') {
        vec!["filter".into(), words[1..].join(" ")]
    } else {
        words
    };
    let is_view = words.first().is_some_and(|w| w == "view");
    let parsed = Cli::try_parse_from(["moai", "ctl"].into_iter().map(str::to_owned).chain(words))
        .map_err(|e| Fail::coded(e.to_string(), code::BAD_INPUT))?;
    let Some(Cmd::Ctl(cmd)) = parsed.cmd else {
        return Err(Fail::coded("--then needs a ctl verb", code::BAD_INPUT));
    };
    let verb = match cmd {
        CtlCmd::Do { name, target: None } => wire::Verb::Do { name },
        CtlCmd::Sort { words, default, target: None } => {
            wire::Verb::Sort { words: words.split(',').map(str::to_owned).collect(), default }
        }
        CtlCmd::Filter { query, clear, target: None } if clear != query.is_some() => wire::Verb::Filter { query },
        CtlCmd::Find { query, scope, target: None } => wire::Verb::Find { query, scope },
        CtlCmd::Screen { name, target: None } => wire::Verb::Screen { name },
        CtlCmd::View { name, value, target: None } | CtlCmd::Cell { name, value, target: None }
            if value == "on" || value == "off" =>
        {
            if is_view {
                wire::Verb::View { name, on: value == "on" }
            } else {
                wire::Verb::Cell { name, on: value == "on" }
            }
        }
        CtlCmd::Detail { name, target: None } => wire::Verb::Detail { name },
        CtlCmd::Goto { id, enter, project, target: None } if id.is_some() != project.is_some() => {
            wire::Verb::Goto { id, enter, project }
        }
        CtlCmd::Read { ids, all, group, target: None } => wire::Verb::Read { ids, all, group },
        CtlCmd::Wiki { slug, search, target: None } if slug.is_some() != search.is_some() => {
            wire::Verb::Wiki { slug, search }
        }
        CtlCmd::Pane { list_width, list_height, wiki_width, gantt_label, gantt_grid, target: None } => {
            wire::Verb::Pane { list_width, list_height, wiki_width, gantt_label, gantt_grid }
        }
        CtlCmd::Zone { name, target: None } => wire::Verb::Zone { name },
        CtlCmd::Theme { name, target: None } => wire::Verb::Theme { name },
        _ => return Err(Fail::coded("--then accepts a named ctl verb or do, without -t", code::BAD_INPUT)),
    };
    validate_verb(&verb)?;
    Ok(verb)
}

fn split_words(text: &str) -> R<Vec<String>> {
    let (mut words, mut word, mut quote, mut escaped) = (Vec::new(), String::new(), None, false);
    for ch in text.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            ch if ch.is_whitespace() && quote.is_none() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(ch),
        }
    }
    if escaped || quote.is_some() {
        return Err(Fail::coded("unclosed quote or escape in --then", code::BAD_INPUT));
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

pub fn state(ctx: &Ctx, target: Option<&str>, rows: bool) -> R<Vec<String>> {
    #[cfg(not(unix))]
    return Err(Fail::coded("control sockets are unavailable on this OS", code::UNSUPPORTED));
    #[cfg(unix)]
    {
        let windows = call::windows()?;
        let window = window_for(ctx, &windows, target)?;
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

pub fn filter(ctx: &Ctx, target: Option<&str>, query: Option<String>, clear: bool) -> R<Vec<String>> {
    if clear == query.is_some() {
        return Err(Fail::coded("give a filter query or --clear", code::BAD_INPUT));
    }
    verb(ctx, target, wire::Verb::Filter { query })
}

pub fn toggle(ctx: &Ctx, target: Option<&str>, kind: &str, name: String, value: String) -> R<Vec<String>> {
    let on = match value.as_str() {
        "on" => true,
        "off" => false,
        _ => return Err(Fail::coded("use on or off", code::BAD_INPUT)),
    };
    let verb = match kind {
        "view" => wire::Verb::View { name, on },
        _ => wire::Verb::Cell { name, on },
    };
    verb_request(ctx, target, verb)
}

pub fn verb(ctx: &Ctx, target: Option<&str>, verb: wire::Verb) -> R<Vec<String>> {
    validate_verb(&verb)?;
    verb_request(ctx, target, verb)
}

fn validate_verb(verb: &wire::Verb) -> R<()> {
    use wire::Verb;
    let bad = |message: &str| Fail::coded(message, code::BAD_INPUT);
    match verb {
        Verb::Sort { words, .. } if crate::tui::App::sorting_of(words).is_none() => {
            return Err(bad("invalid explorer order; use title, priority, status, or another explorer field, not id"));
        }
        Verb::Find { scope, .. } if crate::query::GrepIn::named(scope).is_none() => {
            return Err(bad("--in must be all, id, title, tag, body, or note"));
        }
        Verb::Screen { name } if !matches!(name.as_str(), "list" | "board" | "gantt" | "stats" | "wiki") => {
            return Err(bad("screen must be list, board, gantt, stats, or wiki"));
        }
        Verb::Detail { name } if !matches!(name.as_str(), "on" | "off" | "right" | "bottom" | "left" | "top") => {
            return Err(bad("detail must be on, off, right, bottom, left, or top"));
        }
        Verb::Goto { id, project, .. } if id.is_some() == project.is_some() => {
            return Err(bad("give an issue id or --project, not both"));
        }
        Verb::Read { ids, all, group } if (*all as u8 + group.is_some() as u8 + !ids.is_empty() as u8) != 1 => {
            return Err(bad("give issue ids, --all, or --group"));
        }
        Verb::Wiki { slug, search } if slug.is_some() == search.is_some() => {
            return Err(bad("give a wiki slug or --search"));
        }
        Verb::Pane { list_width, list_height, wiki_width, gantt_label, gantt_grid }
            if [list_width, list_height, wiki_width, gantt_label, gantt_grid].iter().all(|v| v.is_none()) =>
        {
            return Err(bad("give at least one pane width or height"));
        }
        Verb::Theme { name } if !matches!(name.as_str(), "system" | "light" | "dark") => {
            return Err(bad("theme must be system, light, or dark"));
        }
        _ => {}
    }
    if let Verb::Pane { list_width, list_height, wiki_width, gantt_label, gantt_grid } = verb {
        for value in [list_width, list_height, wiki_width, gantt_label, gantt_grid].into_iter().flatten() {
            if value != "none" && value.parse::<i64>().is_err() {
                return Err(bad("pane values must be numbers or none"));
            }
        }
    }
    Ok(())
}

fn verb_request(ctx: &Ctx, target: Option<&str>, verb: wire::Verb) -> R<Vec<String>> {
    invoke(ctx, target, verb, true)
}

fn invoke(ctx: &Ctx, target: Option<&str>, verb: wire::Verb, needs_actor: bool) -> R<Vec<String>> {
    #[cfg(not(unix))]
    return Err(Fail::coded("control sockets are unavailable on this OS", code::UNSUPPORTED));
    #[cfg(unix)]
    {
        let windows = call::windows()?;
        let window = window_for(ctx, &windows, target)?;
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
        Some("not_found") => code::NOT_FOUND,
        Some("broken") => code::BROKEN,
        Some("error") => code::ERROR,
        _ => code::REFUSED,
    }
}

#[cfg(test)]
mod proposal_tests {
    use super::*;

    #[test]
    fn control_preserves_broken_and_error_codes() {
        assert_eq!(code_of(Some("broken")), code::BROKEN);
        assert_eq!(code_of(Some("error")), code::ERROR);
    }

    #[test]
    fn then_uses_the_same_named_verb_shape_and_keeps_a_filter_phrase() {
        let wire::Verb::Filter { query } = then_verb("filter tag=parser status=todo").unwrap() else {
            panic!("not a filter")
        };
        assert_eq!(query.as_deref(), Some("tag=parser status=todo"));
        assert!(matches!(then_verb("screen board").unwrap(), wire::Verb::Screen { name } if name == "board"));
        assert!(then_verb("keys SPC a y").is_err());
        assert!(then_verb("propose hello").is_err());
        assert!(then_verb("screen missing").is_err());
        assert!(then_verb("sort id").is_err());
        assert!(then_verb("theme neon").is_err());
    }
}
