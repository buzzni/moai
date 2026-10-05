//! `moai hello`·`moai agents` — 출석을 적고 본다(moai-h8tn). 꼴과 까닭은 [`crate::mail`] 에 있다.
//!
//! **트래커를 안 쓴다** — 출석은 `.moai/agents/` 의 파일이고 `issues.jsonl`·저널에 안 든다.

use super::{Ctx, Fail, R, code, tell};
use crate::cli::{AgentsArgs, HelloArgs};
use crate::i18n::{fill, say};
use crate::mail::{self, Presence};
use serde::Serialize;

#[derive(Serialize)]
struct Listed<'a> {
    agents: &'a [Presence],
    swept: &'a [String],
}

pub fn agents(ctx: &Ctx, args: AgentsArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let dir = repo.agents_dir();
    // **걷는 자리는 여기다**(설계 노트 "죽은 pid 는 agents 가 걷는다"). 훅과 `send` 는 읽기만 하고 죽은 것을
    // 건너뛴다 — 도구 호출마다 도는 자리가 남의 파일을 지우지 않는다.
    let swept = mail::sweep(&dir, &repo.mail_dir());
    // 읽은 지 오래된 편지도 여기서 걷는다(moai-kxkw.my1) — 죽은 장을 걷는 자리가 우편함도 치운다. 말없이 걷는다: 읽은
    // 편지는 전달을 마친 것이고, 날수는 저장소의 설정이 이미 말한다.
    mail::sweep_read(&repo.mail_dir(), repo.config.mail_read_days);
    let (mut agents, garbled) = mail::presences(&dir);
    // 거르개는 걷기 **뒤**다 — 걸러 낸 줄도 죽었으면 걷힌다. 거르개가 걷기를 좁히면 감독이 부를 때마다 남의 죽은
    // 줄이 남는다.
    //
    // 빈 끝의 말은 **거르기 전에 줄이 있었는가**로 가른다(리뷰 moai-snyk.nic) — 아무도 없으면 거르개를 줬어도 등록하는
    // 길(`moai hello`)을 대는 말이 맞다. 감독의 2 가 빈손일 때 사람이 보는 자리다.
    let registered = !agents.is_empty();
    // **20분 넘게 조용한 장은 떠난 것으로 보인다**(2026-10-05 사용자 결정, moai-j3n5) — 프로세스로 못 재는 장(pid 를 모르는
    // Codex 장, 다른 기계의 장 — moai-dhxm)이다. 지우지 않고 상태만 그때 잰 낱말로 바꿔 보인다(파일은 안 고친다).
    // `--status idle` 이 그 장을 안 내니 감독이 떠난 일꾼에게 일감을 안 보낸다.
    let now = crate::model::now();
    for p in agents.iter_mut().filter(|p| p.stale(&now)) {
        p.status = mail::GONE.to_string();
    }
    let want = |given: &Option<String>, have: &str| given.as_deref().is_none_or(|w| w.trim() == have);
    agents.retain(|p| want(&args.role, &p.role) && want(&args.status, &p.status));
    for g in &garbled {
        tell(&fill(
            say(ctx.lang(), "warn.agents_garbled"),
            &[
                ("path", &crate::text::one_line(&g.path.display().to_string())),
                ("why", &crate::text::one_line(&g.said(ctx.lang()))),
            ],
        ));
    }
    if ctx.json {
        let names: Vec<String> = swept.iter().map(|s| s.name.clone()).collect();
        return super::json_line(&Listed { agents: &agents, swept: &names });
    }
    let lang = ctx.lang();
    // **걷은 까닭을 갈라 말한다**(moai-dhxm) — 프로세스가 죽은 장은 편지가 보낸 이에게 돌아갔고, 하루 넘게 아무것도 안 적어
    // 걷은 장(Codex 의 장, 다른 기계의 장)은 그 함이 남아 그 세션을 기다린다. 다른 기계에서 아직 돌 수 있는 장을 "그
    // 프로세스가 없다" 로 대지 않는다.
    let mut out: Vec<String> = swept
        .iter()
        .map(|s| {
            let said = match (s.dead, s.elsewhere) {
                (true, _) => say(lang, "agents.swept"),
                (false, true) => say(lang, "agents.swept_elsewhere"),
                (false, false) => say(lang, "agents.swept_quiet"),
            };
            fill(said, &[("name", &s.name)])
        })
        .collect();
    if agents.is_empty() {
        // 키는 `say` 에 글자째 적는다 — 소스가 부르는 키를 i18n 시험이 그 글자로 센다.
        out.push(if registered { say(lang, "agents.none_match") } else { say(lang, "agents.none") }.to_string());
        return Ok(out);
    }
    out.extend(table(&agents, ctx.zone()));
    Ok(out)
}

/// 출석부를 표로 — 이름·벤더·모델·역할·상태·그때·pid. 칸은 화면 폭으로 맞춘다(남이 적은 모델·역할에 두 칸
/// 글자가 들 수 있다). **그때는 보는 사람의 시간대로 적는다**(`view::stamp`, moai-p5az) — `--json` 은 UTC 그대로다.
fn table(agents: &[Presence], zone: &crate::tz::Zone) -> Vec<String> {
    let rows: Vec<[String; 7]> = agents
        .iter()
        .map(|p| {
            [
                p.name.clone(),
                cell(&p.vendor),
                cell(&p.model),
                cell(&p.role),
                cell(&p.status),
                if p.since.is_empty() { cell(&p.since) } else { crate::view::stamp(&p.since, zone) },
                pid_cell(p),
            ]
        })
        .collect();
    let width = |k: usize| rows.iter().map(|r| crate::text::width(&r[k])).max().unwrap_or(0);
    let widths: Vec<usize> = (0..7).map(width).collect();
    rows.iter()
        .map(|r| {
            let mut line = String::new();
            for (k, c) in r.iter().enumerate() {
                if k + 1 == r.len() {
                    line.push_str(c);
                } else {
                    let pad = widths[k] - crate::text::width(c);
                    line.push_str(&format!("{c}{}  ", " ".repeat(pad)));
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}

/// pid 칸 — **다른 기계의 pid 는 그 기계의 이름을 단다**(moai-dhxm). 맨 숫자로 두면 사람이 이 기계에서 그 pid 를 찾거나
/// 죽인다 — 여기서는 남의 프로세스다. 이름을 모르거나 비었으면 `?` 다. 표와 `hello` 의 거절이 이 하나로 쓴다.
fn pid_cell(p: &Presence) -> String {
    if p.pid == 0 || p.here() {
        return p.pid.to_string();
    }
    let host = p.host.as_deref().map(crate::text::one_line).filter(|h| !h.trim().is_empty());
    format!("{}@{}", p.pid, host.unwrap_or_else(|| "?".to_string()))
}

/// 남이 적은 칸 — 비었으면 `-`, 제어문자는 걷는다.
fn cell(s: &str) -> String {
    if s.is_empty() { "-".to_string() } else { crate::text::one_line(s) }
}

/// 이 부름이 출석을 적을 에이전트 — 그 프로세스와, 조상에서 읽은 벤더.
///
/// **Codex 는 프로세스로 안 잇는다**(moai-u5wr.7xr, moai-sile) — 그 셸은 한 사람의 Codex 세션 모두가 함께 쓰는
/// `codex app-server` 데몬 밑에서 돈다. 데몬의 pid 로 이으면 Codex 창 둘이 `hello` 한 번씩에 한 장을 빼앗고, 데몬이 뜬
/// 칸의 `TMUX_PANE` 을 적어 `send --wake` 가 남의 창에 글자를 친다. 그래서 Codex 는 pid 를 모름(0)으로 두고, Codex 가
/// 셸에 세우는 세션 id 로 잇는다([`mail::codex_session`]) — 훅이 그 세션에 지은 장과 같은 장이다.
enum Agent {
    /// 프로세스를 아는 에이전트 — 준 pid, 조상의 에이전트, 그도 없으면 이 명령을 띄운 셸.
    Known(mail::Proc, Option<&'static str>),
    /// Codex — 프로세스로 못 가르고, 그 셸이 대는 세션 id 로 가른다. 그 값이 없는 판(옛 Codex)은 `None` 이다.
    Codex(Option<String>),
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
    for name in [args.name.as_deref(), args.as_.as_deref()].into_iter().flatten() {
        if !mail::is_agent_name(name) {
            return Err(super::mail::bad_name(lang, name));
        }
    }
    // **낱말은 앞뒤를 다듬어 적는다**(리뷰 moai-h8tn.x4l) — `--role ' supervisor'` 를 그대로 적으면 `supervisor` 와 안
    // 맞아 감독이 `any-idle-worker` 일감을 가진다.
    let word = |v: &Option<String>| v.as_deref().map(|v| v.trim().to_string());
    let (vendor_given, model_given, role_given) = (word(&args.vendor), word(&args.model), word(&args.role));

    let dir = repo.agents_dir();
    let (all, _) = mail::presences(&dir);
    // **`--as` 는 이미 선 장을 이어받는다**(moai-u5wr.7xr) — 훅이 그 세션에 지어 준 장이다. 셸에서 제 세션을 못 찾는
    // Codex 가 그 이름을 대고 역할을 단다. 프로세스·세션·칸은 그 장의 것을 그대로 둔다.
    let adopted = match args.as_.as_deref() {
        Some(name) => Some(all.iter().find(|p| p.name == name).cloned().ok_or_else(|| {
            Fail::coded(fill(say(lang, "refuse.agents_no_card"), &[("name", name)]), code::NOT_FOUND)
        })?),
        None => None,
    };
    let agent = match (args.pid, &adopted) {
        (_, Some(p)) => Agent::Known(mail::Proc { pid: p.pid, ppid: 0, start: p.pid_start, comm: String::new() }, None),
        (Some(pid), None) => {
            let p = mail::proc_of(pid).ok_or_else(|| {
                Fail::coded(fill(say(lang, "refuse.agents_no_pid"), &[("pid", &pid.to_string())]), code::NOT_FOUND)
            })?;
            let seen = mail::vendor_of(&p.comm);
            Agent::Known(p, seen)
        }
        (None, None) => {
            let ancestors = mail::ancestors();
            match mail::agent_among(&ancestors) {
                Some((_, "codex")) => Agent::Codex(mail::codex_session()),
                Some((p, v)) => Agent::Known(p.clone(), Some(v)),
                None => {
                    let shell = ancestors
                        .first()
                        .cloned()
                        .ok_or_else(|| Fail::new(say(lang, "refuse.agents_no_parent").to_string()))?;
                    Agent::Known(shell, None)
                }
            }
        }
    };
    // **`MOAI_AGENT` 가 이 창의 이름이다**(moai-ew4o.e1m) — `send`·`inbox` 가 그 이름으로 돌고 훅도 그 이름으로 장을 세운다.
    // 다른 `--name` 은 다음 훅이 도로 옮기니 받지 않는다.
    //
    // **이 창이 제 장을 적을 때만 읽는다**(리뷰 moai-ew4o.q9f) — `--pid`·`--as` 가 가리킨 장은 남의 창의 것이다. 읽던 판은
    // 사람의 터미널이 세운 이름을 그 장에 씌워 `--pid <일꾼> --name w1` 을 거절하고, `--name` 이 없으면 그 일꾼의 장을
    // 사람의 이름으로 옮겨 편지까지 데려갔다 — 이 셸의 tmux 칸과 `CLAUDE_CODE_SESSION_ID` 를 안 적는 것과 같은 까닭이다.
    // **Codex 셸에서도 안 읽는다** — 그 환경은 세션 모두가 함께 쓰는 데몬의 것이라 첫 창이 든 이름이 모든 창에 선다(훅의
    // `attendee` 가 Codex 에서 안 읽는 것과 같은 자, moai-sile).
    let told = match &agent {
        Agent::Known(..) if args.pid.is_none() && adopted.is_none() => super::mail::told_name(lang)?,
        _ => None,
    };
    if let (Some(name), Some(told)) = (args.name.as_deref(), told.as_deref())
        && name != told
    {
        return Err(Fail::coded(
            fill(say(lang, "refuse.agents_told_name"), &[("name", name), ("told", told)]),
            code::BAD_INPUT,
        ));
    }
    // 이 에이전트가 이미 든 장 — 이어받은 장, 아니면 같은 프로세스의 장. Codex 는 같은 세션의 장이다 — 훅이 지었다.
    let before = adopted.clone().or_else(|| match &agent {
        Agent::Known(proc, _) => all.iter().find(|p| p.runs_as(proc)).cloned(),
        Agent::Codex(session) => {
            session.as_deref().and_then(|s| all.iter().find(|p| p.session.as_deref() == Some(s))).cloned()
        }
    });
    let seen = match &agent {
        Agent::Known(_, seen) => *seen,
        Agent::Codex(_) => Some("codex"),
    };
    let vendor = vendor_given
        .or_else(|| before.as_ref().map(|p| p.vendor.clone()).filter(|v| !v.is_empty()))
        .or_else(|| seen.map(str::to_string))
        .unwrap_or_default();
    // 이름: 준 것, `MOAI_AGENT`, 이 에이전트가 이미 든 것, Claude 가 보이는 세션 이름, `<벤더>-<pid>` 차례(사용자 결정).
    // **Codex 는 데몬의 pid 로 짓지 않는다** — 창마다 같다. 훅이 짓는 것과 같은 `codex-<세션 id 앞 8자>` 이고, 남이 쥐었으면
    // 토막을 이어 가른다([`mail::codex_name`]). 세션 id 도 없으면 짓지 않는다 — 훅이 지어 준 장을 `--as` 로 잇거나 이름을 댄다.
    let generated = || match &agent {
        Agent::Known(proc, _) => (vendor == "claude")
            .then(|| mail::claude_session_name(proc.pid))
            .flatten()
            .or_else(|| mail::name_with(if vendor.is_empty() { "agent" } else { &vendor }, &proc.pid.to_string())),
        Agent::Codex(session) => session.as_deref().and_then(|s| mail::codex_name(&all, s)),
    };
    let given = args.name.clone().or(told).or_else(|| before.as_ref().map(|p| p.name.clone()));
    // 지은 이름인가 — 준 것도 대는 것도 이어 쓰는 것도 없어 여기서 지었다(moai-nas5). 겨루는 자가 갈린다(아래).
    let made = given.is_none();
    let name = match given.or_else(generated) {
        Some(name) => name,
        None if matches!(agent, Agent::Codex(_)) => {
            return Err(Fail::coded(say(lang, "refuse.agents_codex_who").to_string(), code::NO_ACTOR));
        }
        None => return Err(super::mail::bad_name(lang, &vendor)),
    };
    // **산 남의 이름은 안 뺏는다** — 두 에이전트가 한 이름이면 편지가 먼저 읽은 쪽으로 샌다. **대소문자만 다른
    // 이름도 같은 이름이다**(리뷰 moai-h8tn.x4l) — 이름이 파일 이름이라, 대소문자를 안 가리는 파일 시스템(macOS
    // 기본)에서는 `Worker` 와 `worker` 가 한 장이다.
    // 산 것을 찾는다 — 대소문자로 겹치는 장은 여럿일 수 있어, 첫 장이 죽은 것이면 뒤의 산 장을 가린다.
    // **지은 이름은 다른 기계의 조용한 장이 하루 쥔다**(moai-nas5, [`mail::Presence::holds_made_name`]) — 우연히 겹친
    // 이름으로 아직 산 저쪽 세션의 장과 편지를 가져가지 않는다. 대거나 이어 쓰는 이름은 떠나면 놓는다.
    let mine = |p: &Presence| before.as_ref().is_some_and(|b| b.name == p.name);
    let holds = |p: &Presence| if made { p.holds_made_name() } else { !p.gone() };
    if let Some(other) = all.iter().find(|p| p.name.eq_ignore_ascii_case(&name) && !mine(p) && holds(p)) {
        // **떠난 것으로 보이는 장이 쥔 지은 이름은 "도는 에이전트" 라고 안 댄다**(리뷰 moai-nas5.cn7) — `moai agents` 는 그
        // 장을 `gone` 으로 보이는데 거절이 도는 에이전트라고 하면 둘이 엇갈린다. 하루 쥐는 까닭과, 그 이름을 대면 넘겨받는다는
        // 것을 댄다. 다른 기계의 장일 때만 그 말을 고른다 — 이 기계의 장은 `holds` 가 산 것으로 잰 장이라, 그 사이 죽었어도
        // "다른 기계" 라고 대지 않는다.
        let held = !other.here() && other.gone();
        let said = if held { say(lang, "refuse.agents_name_held") } else { say(lang, "refuse.agents_name_taken") };
        return Err(Fail::coded(
            // 다른 기계의 장이면 pid 에 그 기계의 이름을 단다([`pid_cell`]) — 이 이름을 비우려는 사람이 이 기계에서 그 맨
            // 숫자를 죽이면 남의 프로세스다(moai-dhxm).
            fill(said, &[("name", &name), ("pid", &pid_cell(other))]),
            code::ALREADY_EXISTS,
        ));
    }

    let now = crate::model::now();
    let keep = before.clone();
    let (pid, pid_start) = match &agent {
        Agent::Known(proc, _) => (proc.pid, proc.start),
        Agent::Codex(_) => (0, None),
    };
    // **남을 가리켰으면(`--pid`·`--as`) 이 셸의 tmux 칸을 안 적는다** — 그 칸은 그 프로세스의 것이 아닐 수 있고,
    // 깨우기가 그 칸에 글자를 친다. 엉뚱한 칸이면 사람의 창에 `moai inbox` 가 쳐진다. **Codex 도 안 적는다** — 그 셸의
    // 칸은 데몬이 뜬 칸이다(moai-u5wr.7xr 노트).
    let (tmux_pane, tmux_socket) = match (&agent, args.pid.is_some() || adopted.is_some()) {
        (Agent::Codex(_), _) => (None, None),
        (_, false) => Presence::tmux_here(),
        (_, true) => keep.as_ref().map(|p| (p.tmux_pane.clone(), p.tmux_socket.clone())).unwrap_or_default(),
    };
    let presence = Presence {
        v: mail::VERSION,
        name: name.clone(),
        vendor,
        model: model_given.or_else(|| keep.as_ref().map(|p| p.model.clone())).unwrap_or_default(),
        role: role_given.or_else(|| keep.as_ref().map(|p| p.role.clone())).unwrap_or_default(),
        // 인사는 일하는 중에 친다 — 그 턴이 끝나면 `Stop` 훅이 `idle` 을 적는다.
        status: mail::BUSY.to_string(),
        since: match &keep {
            Some(p) if p.status == mail::BUSY => p.since.clone(),
            _ => now.clone(),
        },
        pid,
        pid_start,
        // 기계는 아래에서 pid 와 함께 적는다([`Presence::at`]).
        machine: None,
        host: None,
        // **세션 id 는 조상에서 찾은 Claude 에게만 환경에서 읽는다** — `--pid` 로 남을 가리켰으면 이 셸의
        // 세션은 그 프로세스의 것이 아니다. 훅이 적어 둔 것이 있으면 그것이 먼저다.
        // Codex 는 그 셸이 대는 세션 id 를 적는다 — 훅이 그 세션의 장을 이것으로 찾는다. **Codex 셸의
        // `CLAUDE_CODE_SESSION_ID` 는 안 읽는다** — 데몬의 환경에서 새어 든 남의 Claude 세션이다(2026-10-05 쟀다).
        session: keep.as_ref().and_then(|p| p.session.clone()).or_else(|| match &agent {
            Agent::Codex(session) => session.clone(),
            Agent::Known(..) => (args.pid.is_none() && seen == Some("claude"))
                .then(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok().filter(|s| !s.is_empty()))
                .flatten(),
        }),
        cwd: std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default(),
        tmux_pane,
        tmux_socket,
        // 닻은 아래에서 적는다([`Presence::stamp`]).
        seen: None,
        rest: keep.as_ref().map(|p| p.rest.clone()).unwrap_or_default(),
    };
    // **`--as` 는 그 장의 pid 를 잇고, 그 pid 가 선 기계도 그 장의 것이다**(moai-dhxm) — 이 셸의 기계를 적으면 다른 기계의
    // 장이 이 기계의 pid 로 읽혀 `moai agents` 가 걷는다. 그 밖은 이 기계의 프로세스다.
    let mut presence = match &adopted {
        Some(a) => Presence { machine: a.machine.clone(), host: a.host.clone(), ..presence },
        None => presence.at(pid, pid_start),
    };
    // 닻을 적는다(moai-j3n5, moai-dhxm) — 인사한 것이 곧 산 것이다. 프로세스를 아는 장도 다른 기계에서는 이것으로 잰다.
    presence.stamp(&now);
    // **떠난 장의 이름을 넘겨받으면 그 함부터 비운다**(moai-ew4o.l3n) — 그 세션 앞으로 남은 편지가 보낸 이에게 돌아간다.
    // 이어 쓰는 제 장(`before`)은 넘겨받는 것이 아니다([`mail::take_over`]).
    let mail_dir = repo.mail_dir();
    mail::take_over(&mail_dir, &all, &name, before.as_ref().map(|b| b.name.as_str()));
    // 이름을 바꿨으면 옛 장을 걷고 편지를 데려간다 — 한 에이전트가 두 이름으로 서면 편지가 둘로 갈리고, 옛 이름 앞의 안
    // 읽은 편지는 아무도 못 읽고 남는다([`mail::rename_card`]).
    let wrote = match before.filter(|p| p.name != name) {
        Some(old) => mail::rename_card(&dir, &mail_dir, &presence, &old.name),
        None => mail::write_presence(&dir, &presence),
    };
    wrote.map_err(|e| Fail::new(mail::refusal(lang, &dir, &e)))?;
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
