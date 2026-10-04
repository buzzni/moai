//! `moai hello`·`moai agents` — 출석을 적고 본다(moai-h8tn). 꼴과 까닭은 [`crate::mail`] 에 있다.
//!
//! **트래커를 안 쓴다** — 출석은 `.moai/agents/` 의 파일이고 `issues.jsonl`·저널에 안 든다.

use super::{Ctx, Fail, R, code, tell};
use crate::cli::HelloArgs;
use crate::i18n::{fill, say};
use crate::mail::{self, Presence};
use serde::Serialize;

#[derive(Serialize)]
struct Listed<'a> {
    agents: &'a [Presence],
    swept: &'a [String],
}

pub fn agents(ctx: &Ctx) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let dir = repo.agents_dir();
    // **걷는 자리는 여기다**(설계 노트 "죽은 pid 는 agents 가 걷는다"). 훅과 `send` 는 읽기만 하고 죽은 것을
    // 건너뛴다 — 도구 호출마다 도는 자리가 남의 파일을 지우지 않는다.
    let swept = mail::sweep(&dir);
    let (agents, garbled) = mail::presences(&dir);
    for g in &garbled {
        tell(&fill(
            say(ctx.lang(), "warn.agents_garbled"),
            &[("path", &g.path.display().to_string()), ("why", &crate::text::one_line(&g.why))],
        ));
    }
    if ctx.json {
        return super::json_line(&Listed { agents: &agents, swept: &swept });
    }
    let lang = ctx.lang();
    let mut out: Vec<String> = swept.iter().map(|n| fill(say(lang, "agents.swept"), &[("name", n)])).collect();
    if agents.is_empty() {
        out.push(say(lang, "agents.none").to_string());
        return Ok(out);
    }
    out.extend(table(&agents));
    Ok(out)
}

/// 출석부를 표로 — 이름·벤더·모델·역할·상태·그때·pid. 칸은 화면 폭으로 맞춘다(남이 적은 모델·역할에 두 칸
/// 글자가 들 수 있다).
fn table(agents: &[Presence]) -> Vec<String> {
    let rows: Vec<[String; 7]> = agents
        .iter()
        .map(|p| {
            [
                p.name.clone(),
                cell(&p.vendor),
                cell(&p.model),
                cell(&p.role),
                cell(&p.status),
                cell(&p.since),
                p.pid.to_string(),
            ]
        })
        .collect();
    let width = |k: usize| rows.iter().map(|r| unicode_width::UnicodeWidthStr::width(r[k].as_str())).max().unwrap_or(0);
    let widths: Vec<usize> = (0..7).map(width).collect();
    rows.iter()
        .map(|r| {
            let mut line = String::new();
            for (k, c) in r.iter().enumerate() {
                if k + 1 == r.len() {
                    line.push_str(c);
                } else {
                    let pad = widths[k] - unicode_width::UnicodeWidthStr::width(c.as_str());
                    line.push_str(&format!("{c}{}  ", " ".repeat(pad)));
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}

/// 남이 적은 칸 — 비었으면 `-`, 제어문자는 걷는다.
fn cell(s: &str) -> String {
    if s.is_empty() { "-".to_string() } else { crate::text::one_line(s) }
}

pub fn hello(ctx: &Ctx, args: HelloArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let lang = ctx.lang();
    for (what, v) in [("vendor", &args.vendor), ("model", &args.model), ("role", &args.role)] {
        if let Some(v) = v
            && !is_word(v)
        {
            return Err(Fail::coded(
                fill(say(lang, "refuse.agents_word"), &[("what", what), ("value", &crate::text::one_line(v))]),
                code::BAD_INPUT,
            ));
        }
    }
    if let Some(name) = args.name.as_deref()
        && !mail::is_agent_name(name)
    {
        return Err(super::mail::bad_name(lang, name));
    }

    // **에이전트 프로세스를 찾는다** — 준 pid, 아니면 조상 가운데 이름이 벤더인 것, 그것도 없으면 이 명령을
    // 띄운 셸(사람이 터미널에서 인사한 판)이다. 셸의 pid 도 그 창이 닫히면 죽어 출석이 걷힌다.
    let ancestors = mail::ancestors();
    let (agent, seen) = match args.pid {
        Some(pid) => {
            let p = mail::proc_of(pid).ok_or_else(|| {
                Fail::coded(fill(say(lang, "refuse.agents_no_pid"), &[("pid", &pid.to_string())]), code::NOT_FOUND)
            })?;
            let seen = mail::vendor_of(&p.comm);
            (p, seen)
        }
        None => match mail::agent_among(&ancestors) {
            Some((p, v)) => (p.clone(), Some(v)),
            None => {
                let shell = ancestors
                    .first()
                    .cloned()
                    .ok_or_else(|| Fail::new(say(lang, "refuse.agents_no_parent").to_string()))?;
                (shell, None)
            }
        },
    };

    let dir = repo.agents_dir();
    let (all, _) = mail::presences(&dir);
    let same = |p: &&Presence| {
        p.pid == agent.pid && (p.pid_start.is_none() || agent.start.is_none() || p.pid_start == agent.start)
    };
    let before = all.iter().find(same).cloned();
    let vendor = args
        .vendor
        .clone()
        .or_else(|| before.as_ref().map(|p| p.vendor.clone()).filter(|v| !v.is_empty()))
        .or_else(|| seen.map(str::to_string))
        .unwrap_or_default();
    // 이름: 준 것, 이 에이전트가 이미 든 것, Claude 가 보이는 세션 이름, `<벤더>-<pid>` 차례(사용자 결정).
    let name = args
        .name
        .clone()
        .or_else(|| before.as_ref().map(|p| p.name.clone()))
        .or_else(|| (vendor == "claude").then(|| mail::claude_session_name(agent.pid)).flatten())
        .or_else(|| mail::name_from(&format!("{}-{}", if vendor.is_empty() { "agent" } else { &vendor }, agent.pid)))
        .ok_or_else(|| super::mail::bad_name(lang, &vendor))?;
    // **산 남의 이름은 안 뺏는다** — 두 에이전트가 한 이름이면 편지가 먼저 읽은 쪽으로 샌다.
    if let Some(other) = all.iter().find(|p| p.name == name && !same(p))
        && mail::alive(other.pid, other.pid_start) != Some(false)
    {
        return Err(Fail::coded(
            fill(say(lang, "refuse.agents_name_taken"), &[("name", &name), ("pid", &other.pid.to_string())]),
            code::ALREADY_EXISTS,
        ));
    }

    let now = crate::model::now();
    // **`--pid` 로 남을 가리켰으면 이 셸의 tmux 칸을 안 적는다** — 그 칸은 그 프로세스의 것이 아닐 수 있고,
    // 깨우기가 그 칸에 글자를 친다. 엉뚱한 칸이면 사람의 창에 `moai inbox` 가 쳐진다.
    let (tmux_pane, tmux_socket) = match args.pid {
        None => Presence::tmux_here(),
        Some(_) => before.as_ref().map(|p| (p.tmux_pane.clone(), p.tmux_socket.clone())).unwrap_or_default(),
    };
    let keep = before.clone();
    let presence = Presence {
        v: mail::VERSION,
        name: name.clone(),
        vendor,
        model: args.model.clone().or_else(|| keep.as_ref().map(|p| p.model.clone())).unwrap_or_default(),
        role: args.role.clone().or_else(|| keep.as_ref().map(|p| p.role.clone())).unwrap_or_default(),
        // 인사는 일하는 중에 친다 — 그 턴이 끝나면 `Stop` 훅이 `idle` 을 적는다.
        status: mail::BUSY.to_string(),
        since: match &keep {
            Some(p) if p.status == mail::BUSY => p.since.clone(),
            _ => now,
        },
        pid: agent.pid,
        pid_start: agent.start,
        // **세션 id 는 조상에서 찾은 Claude 에게만 환경에서 읽는다** — `--pid` 로 남을 가리켰으면 이 셸의
        // 세션은 그 프로세스의 것이 아니다. 훅이 적어 둔 것이 있으면 그것이 먼저다.
        session: keep.as_ref().and_then(|p| p.session.clone()).or_else(|| {
            (args.pid.is_none() && seen == Some("claude"))
                .then(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok().filter(|s| !s.is_empty()))
                .flatten()
        }),
        cwd: std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default(),
        tmux_pane,
        tmux_socket,
        rest: keep.as_ref().map(|p| p.rest.clone()).unwrap_or_default(),
    };
    mail::write_presence(&dir, &presence).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;
    // 이름을 바꿨으면 옛 장을 걷는다 — 한 에이전트가 두 이름으로 서면 편지가 둘로 갈린다.
    if let Some(old) = before.filter(|p| p.name != name) {
        let _ = std::fs::remove_file(dir.join(format!("{}.json", old.name)));
    }
    if ctx.json {
        return super::json_line(&presence);
    }
    Ok(vec![fill(
        say(lang, "agents.hello"),
        &[
            ("name", &presence.name),
            ("vendor", &cell(&presence.vendor)),
            ("model", &cell(&presence.model)),
            ("role", &cell(&presence.role)),
        ],
    )])
}

/// 벤더·모델·역할로 받는 낱말 — 한 줄, 제어문자 없음, 64자까지. 표의 한 칸이고 파일에 남는다.
fn is_word(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 64 && !s.chars().any(char::is_control)
}
