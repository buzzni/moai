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

fn code_of(code_name: Option<&str>) -> &'static str {
    match code_name {
        Some("no_window") => code::NO_WINDOW,
        Some("gone") => code::GONE,
        Some("person_busy") => code::PERSON_BUSY,
        Some("unsupported") => code::UNSUPPORTED,
        Some("bad_input") => code::BAD_INPUT,
        Some("timeout") => code::TIMEOUT,
        Some("busy") => code::BUSY,
        _ => code::REFUSED,
    }
}
