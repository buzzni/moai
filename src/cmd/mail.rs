//! `moai send`·`moai inbox` — 우편함을 쓰고 읽는다(moai-h8tn). 꼴과 까닭은 [`crate::mail`] 에 있다.
//!
//! **트래커를 안 쓴다** — [`super::open_repo`] 로 자리만 찾고 `with_write`·저널은 안 지난다. 편지는 전달이지
//! 기록이 아니다.

use super::{Ctx, Fail, R, code, note_partial, tell};
use crate::cli::{InboxArgs, SendArgs};
use crate::i18n::{Lang, fill, say};
use crate::mail::{self, Letter, Presence, Stored, Woke};
use crate::style::{self, paint};
use serde::Serialize;

/// `send --json` 의 꼴 — 적은 편지 그대로에 `id` 와(깨웠으면) `wake` 를 얹는다.
#[derive(Serialize)]
struct Sent<'a> {
    id: &'a str,
    #[serde(flatten)]
    letter: &'a Letter,
    #[serde(skip_serializing_if = "Option::is_none")]
    wake: Option<&'a Woke>,
}

/// `inbox --json` 의 편지 하나 — `id`·`read`·`returned` 에 편지 그대로. `returned` 는 보낸 편지가 읽히기 전에 받는 이가
/// 떠나 제 함으로 되돌아온 것이다([`Stored::returned`]) — 그 편지의 `to` 는 떠난 이, `from` 은 나다. 늘 선다.
///
/// **편지가 든 모르는 키가 `id`·`read`·`returned` 면 뺀다**(리뷰 moai-h8tn.x4l) — 모르는 키는 그대로 내는데([`Letter::rest`]),
/// 그 이름이 겉의 키와 겹치면 한 객체에 같은 키가 둘 서고, 읽는 쪽 대부분(jq·python)이 뒤의 것을 믿는다. 손으로
/// 놓거나 다른 판이 쓴 편지 하나가 진짜 id 와 읽음 표를 가린다.
#[derive(Serialize)]
struct Shown<'a> {
    id: &'a str,
    read: bool,
    returned: bool,
    #[serde(flatten)]
    letter: Letter,
}

impl<'a> Shown<'a> {
    fn of(s: &'a Stored) -> Shown<'a> {
        let mut letter = s.letter.clone();
        letter.rest.retain(|k, _| !matches!(k.as_str(), "id" | "read" | "returned"));
        Shown { id: &s.id, read: s.reader.is_some(), returned: s.returned, letter }
    }
}

/// 제목의 상한(글자) — 한 줄이다. 본문은 [`mail::BODY_MAX`] 가 재는데 제목은 재지 않던 판은 13만 바이트 제목을
/// 받아, 훅이 싣는 편지 한 통이 본문 상한을 몇 배 넘었다.
const SUBJECT_MAX: usize = 200;

#[derive(Serialize)]
struct Inbox<'a> {
    me: &'a str,
    letters: Vec<Shown<'a>>,
    lost: Vec<String>,
}

pub fn send(ctx: &Ctx, args: SendArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let to = args.to.trim().to_string();
    if !mail::is_recipient(&to) {
        return Err(bad_name(ctx.lang(), &to));
    }
    let subject = args.subject.trim().to_string();
    if subject.is_empty() || subject.contains(['\n', '\r']) || subject.chars().count() > SUBJECT_MAX {
        let said = fill(say(ctx.lang(), "refuse.mail_subject"), &[("max", &SUBJECT_MAX.to_string())]);
        return Err(Fail::coded(said, code::BAD_INPUT));
    }
    if let Some(id) = args.reply_to.as_deref()
        && !mail::is_id(id)
    {
        return Err(Fail::coded(
            fill(say(ctx.lang(), "refuse.mail_reply_to"), &[("id", &crate::text::one_line(id))]),
            code::BAD_INPUT,
        ));
    }
    // **읽기는 이름을 다 댄 뒤다** — `-b -` 는 stdin 을 기다리므로, 잘못 친 이름은 그 앞에서 멈춘다.
    let (agents, _) = mail::presences(&repo.agents_dir());
    let from = who(ctx, args.sender.as_deref(), &agents)?;
    let body = super::add::read_body_said(args.body, ctx)?.unwrap_or_default();
    if body.len() > mail::BODY_MAX {
        let said = fill(
            say(ctx.lang(), "refuse.mail_body_large"),
            &[("n", &body.len().to_string()), ("max", &mail::BODY_MAX.to_string())],
        );
        return Err(Fail::coded(said, code::BAD_INPUT));
    }
    let letter = Letter {
        v: mail::VERSION,
        to: to.clone(),
        from: from.clone(),
        subject,
        body,
        sent_at: crate::model::now(),
        reply_to: args.reply_to,
        rest: Default::default(),
    };
    let dir = repo.mail_dir();
    mail::migrate(&dir);
    let id = mail::send(&dir, &letter).map_err(|e| Fail::new(format!("{}: {e}", dir.display())))?;

    // **없는 이름에도 보낸다** — 아직 인사하지 않은 에이전트에게 먼저 보내는 것은 흔하다. 오타일 수 있으니
    // 한 줄로만 댄다.
    let named = agents.iter().find(|p| p.name == to);
    if to != mail::ANY_IDLE_WORKER && named.is_none() {
        tell(&fill(say(ctx.lang(), "mail.nobody_named"), &[("to", &to)]));
    }
    let woke = args.wake.then(|| match to.as_str() {
        mail::ANY_IDLE_WORKER => mail::idle_worker(&agents, &from).map(mail::wake),
        _ => named.filter(|p| !p.gone()).map(mail::wake),
    });
    let woke = woke.map(|w| {
        w.unwrap_or_else(|| Woke { to: to.clone(), via: "none", done: false, why: Some("nobody"), since: None })
    });

    if ctx.json {
        return super::json_line(&Sent { id: &id, letter: &letter, wake: woke.as_ref() });
    }
    let mut out = vec![fill(say(ctx.lang(), "mail.sent"), &[("id", &paint(style::ID, &id)), ("to", &to)])];
    out.extend(woke.as_ref().and_then(|w| woke_line(ctx.lang(), ctx.zone(), w)));
    Ok(out)
}

/// 깨운 결과를 한 줄로 — **깨울 길도 이도 없으면 아무 말도 안 한다**(2026-10-04 사용자 결정 "깨우기는 덤"). 낱말
/// (`via`·`why`)은 기계의 것이라 `--json` 은 그 판에도 `wake` 를 그대로 낸다. 사람 말은 여기서 짓는다.
fn woke_line(lang: Lang, zone: &crate::tz::Zone, w: &Woke) -> Option<String> {
    let to = w.to.as_str();
    Some(match (w.done, w.why) {
        (true, _) => fill(say(lang, "mail.wake_done"), &[("to", to), ("via", w.via)]),
        // 기다리는 에이전트는 두드릴 까닭이 없다 — 그 기다림이 편지를 가진다. 말하지 않는다.
        (false, Some("no_way" | "nobody" | "waiting")) => return None,
        (false, Some("ask_sender")) => fill(say(lang, "mail.wake_send_message"), &[("to", to)]),
        // **턴이 끝나면 싣는다고 약속하지 않는다**(moai-u5wr.f29) — 끊긴 턴은 끝이 안 온다. 언제부터인지만 댄다.
        (false, Some("busy")) => {
            let since = w.since.as_deref().map(|s| crate::view::stamp(s, zone)).unwrap_or_else(|| "?".to_string());
            fill(say(lang, "mail.wake_busy"), &[("to", to), ("since", &since)])
        }
        (false, why) => fill(say(lang, "mail.wake_failed"), &[("to", to), ("via", w.via), ("why", why.unwrap_or("?"))]),
    })
}

pub fn inbox(ctx: &Ctx, args: InboxArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let (agents, _) = mail::presences(&repo.agents_dir());
    let me = who(ctx, args.me.as_deref(), &agents)?;
    let role = agents.iter().find(|p| p.name == me).map(|p| p.role.clone()).unwrap_or_default();
    let dir = repo.mail_dir();
    mail::migrate(&dir);

    // **기다림은 훑기를 되풀이한다** — 파일 시스템의 알림(inotify)은 플랫폼마다 다르고 크레이트가 든다. 반 초에
    // 한 번 디렉터리 하나를 읽는 값이 그보다 싸다.
    //
    // **넘치는 값은 끝없이 기다리는 것으로 읽는다**(리뷰 moai-h8tn.x4l) — `Instant + Duration` 은 넘치면 멈춘다(panic).
    // "끝없이" 를 `--wait 9223372036854775807` 로 적는 것은 자연스럽다.
    let until = args.wait.map(|s| std::time::Instant::now().checked_add(std::time::Duration::from_secs(s)));
    // **`idle` 은 실제로 기다리기 시작할 때 적는다**(리뷰 moai-snyk.nic) — 첫 훑기에 편지가 이미 서 있거나 `--wait 0`
    // 이면 기다린 것이 아니다. 훑기 앞에서 적던 판은 `--wait 0` 한 번이 일하는 장을 `idle` 로 남겼고, 편지가 이미 선
    // 판에는 `idle` 을 적었다가 곧장 `busy` 로 되돌려 `since`(얼마나 놀았나)만 새로 세웠다.
    let mut idled = false;
    // 닻을 마지막으로 다시 적으러 간 때 — 기다리는 동안 출석부는 [`mail::SEEN_EVERY`] 에 한 번만 연다(리뷰
    // moai-ew4o.q9f — 반 초마다 모든 장을 읽던 자리다).
    let mut stamped = std::time::Instant::now();
    let (mut mine, garbled) = loop {
        let (all, garbled) = mail::list(&dir, &me, args.all);
        let mine: Vec<Stored> = all.into_iter().filter(|s| mail::for_me(s, &me, &role)).collect();
        let waiting = until.is_some_and(|t| t.is_none_or(|t| std::time::Instant::now() < t));
        if !waiting || mine.iter().any(|s| s.reader.is_none()) {
            break (mine, garbled);
        }
        // 기다리는 동안 닻을 다시 적는다(moai-j3n5) — 안 적으면 오래 기다리는 일꾼이 떠난 것으로 읽혀, 감독이 일감을 보낼
        // 곳을 잃는다. 프로세스를 모르는 장(Codex)만이 아니다(moai-dhxm) — 다른 기계(컨테이너)의 감독은 어느 장이든 닻으로
        // 잰다. 때가 되었을 때만 쓴다([`mail::keep_alive`]).
        if idled {
            if stamped.elapsed().as_secs() >= mail::SEEN_EVERY.unsigned_abs() {
                mail::keep_alive(&repo.agents_dir(), |p| p.name == me);
                stamped = std::time::Instant::now();
            }
        } else {
            attend(&repo, &me, mail::IDLE);
            idled = true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    // 못 읽은 편지는 답을 덜 낸 것이다 — 다 내고 비영으로 끝난다(`show` 의 못 읽는 줄과 같은 자). 자리도 남이 지은
    // 이름이라 한 줄로 접는다 — 파일 이름에 든 제어문자가 터미널을 움직이지 않게.
    //
    // **남의 편지가 깨진 것은 내 답이 덜 난 것이 아니다**(리뷰 moai-h8tn.x4l) — 깨진 편지 하나가 모든 에이전트의 `inbox` 를
    // 비영으로 끝내면 `--ack` 로 이미 읽음이 된 편지를 실패로 읽은 고리가 그 편지를 버린다. 받는 이마다 함이 따로라
    // (moai-ew4o.c92) 여기 오는 것은 제 함과 열린 편지의 함뿐이다. 열린 편지는 감독이 안 가지니 감독에게는 말만 한다.
    for g in &garbled {
        tell(&fill(
            say(ctx.lang(), "warn.mail_garbled"),
            &[("path", &crate::text::one_line(&g.path.display().to_string())), ("why", &crate::text::one_line(&g.why))],
        ));
        if g.mailbox.as_deref() != Some(mail::ANY_IDLE_WORKER) || role != mail::SUPERVISOR {
            note_partial();
        }
    }

    let mut lost = Vec::new();
    // 이 부름이 얻은 편지의 수 — `--ack` 면 가진 것(과 옮기다 못 옮겨 그대로 보인 것)이고, 아니면 보인 못 읽은 것이다.
    let gained = if args.ack {
        let mut kept = Vec::new();
        let mut gained = 0;
        // **열린 편지(`any-idle-worker`)는 한 부름에 한 통만 가진다**(리뷰 moai-snyk.nic) — 훅이 한 번에 한 통만 싣는
        // 것([`crate::hook::deliverable`])과 같은 자다. 일꾼은 이 부름으로 일감을 기다리는데, 쌓인 열린 편지를 먼저 깬
        // 하나가 다 가지면 그 받는 이 낱말이 거짓이 되고, 나머지 일감은 그 일꾼의 읽음 속에 숨어 아무도 못 가진다. 남에게
        // 진 것은 다음 열린 편지로 간다 — 진 채로 멈추면 남은 일감이 이 부름에서 아무에게도 안 간다.
        let mut opened = false;
        for mut s in mine {
            if s.reader.is_some() {
                kept.push(s);
                continue;
            }
            let open = s.open();
            if open && opened {
                continue;
            }
            match mail::take(&dir, &s, &me) {
                Ok(mail::Took::Mine) => {
                    opened |= open;
                    gained += 1;
                    s.reader = Some(me.clone());
                    kept.push(s);
                }
                Ok(mail::Took::Lost) => lost.push(s.id),
                Err(e) => {
                    tell(&format!("{}: {e}", dir.display()));
                    note_partial();
                    gained += 1;
                    kept.push(s);
                }
            }
        }
        mine = kept;
        gained
    } else {
        mine.iter().filter(|s| s.reader.is_none()).count()
    };
    // 기다림이 편지로 끝났으면 일하러 간다 — **편지를 얻었을 때만**(리뷰 moai-snyk.nic). 남이 먼저 가진 열린 편지는
    // 일이 아니다: 편지를 본 자리에서 `busy` 를 적던 판은 겨루기에 진 일꾼을 일 없이 바쁜 것으로 세워, 다시 걸기
    // 전까지 `agents --status idle` 과 `send --wake` 의 후보에서 뺐다. 때가 다 되어 끝났으면 `idle` 그대로다 — 일꾼은
    // 곧 다시 건다.
    if until.is_some() && gained > 0 {
        attend(&repo, &me, mail::BUSY);
    }

    if ctx.json {
        let letters = mine.iter().map(Shown::of).collect();
        return super::json_line(&Inbox { me: &me, letters, lost });
    }
    let lang = ctx.lang();
    let mut out = Vec::new();
    if mine.is_empty() {
        out.push(fill(say(lang, "mail.inbox_empty"), &[("me", &me)]));
    }
    // 편지는 `view::mail` 한 자리에서 선다 — 훅이 세션에 싣는 글과 같은 꼴이다.
    for s in &mine {
        out.extend(crate::view::mail::letter(lang, ctx.zone(), s));
    }
    for id in &lost {
        out.push(fill(say(lang, "mail.lost"), &[("id", id)]));
    }
    Ok(out)
}

/// 기다리는 동안의 출석 — `--wait` 가 그 이름의 장을 `idle` 로, 편지가 오면 `busy` 로 적는다(2026-10-04 사용자 결정,
/// moai-snyk). 일꾼은 턴 **안에서** 기다린다 — Claude 의 `Stop` 훅은 턴이 끝나야 `idle` 을 적고, 훅이 없는 벤더는
/// `hello` 가 적은 `busy` 에 머문다. 그래서 감독이 `agents --status idle` 로 일꾼을 찾으려면 기다리는 자리가 제
/// 상태를 적어야 한다.
///
/// **없는 장은 안 세운다** — 등록은 `hello` 와 훅의 일이다. `--as` 로 남의 이름을 대도 그 이름의 장을 고친다: 그
/// 이름으로 편지를 가지는 것이 곧 그 에이전트로 일하는 것이다. 상태가 같으면 안 쓴다 — `since` 가 "얼마나
/// 놀았나" 를 잰다(`send --wake` 가 가장 오래 논 일꾼을 고른다). 닻만은 때가 되었으면 적는다
/// ([`mail::Presence::due`]). **빈 `since` 는 채운다** — 훅의 `attend` 와 같은
/// 자다. 빈 글은 가장 앞에 서서, 그대로 두면 `send --wake` 가 그 장을 가장 오래 논 일꾼으로 고른다(리뷰
/// moai-snyk.nic). 못 적으면 조용히 지나간다 — 출석은 기록이 아니라 지금의 표다([`mail::write_presence`]).
fn attend(repo: &crate::store::Repo, me: &str, status: &str) {
    let dir = repo.agents_dir();
    // 기다리기 직전·직후에 다시 읽는다 — 앞에서 읽은 장으로 덮으면 그 사이 훅이 고친 칸을 되돌린다.
    let (agents, _) = mail::presences(&dir);
    let Some(mut p) = agents.into_iter().find(|p| p.name == me) else { return };
    let now = crate::model::now();
    let changed = p.status != status || p.since.is_empty();
    // **상태가 같아도 닻은 때가 되었으면 적는다**(리뷰 moai-ew4o.q9f) — 편지가 이미 와 있어 기다림 없이 끝난
    // `inbox --ack --wait` 는 `busy` 를 `busy` 로 적어 아무것도 안 썼고, 그 일꾼의 닻은 그 앞의 인사에 머물렀다. 훅이 안
    // 도는 Codex 일꾼은 그만큼 일찍 걷혔다.
    if !changed && !p.due(&now) {
        return;
    }
    if changed {
        p.status = status.to_string();
        p.since = now.clone();
    }
    p.stamp(&now);
    let _ = mail::write_presence(&dir, &p);
}

/// 이 부름이 **누구의 이름으로 도는가** — `--as`, `MOAI_AGENT`, 이 명령을 띄운 에이전트의 출석 차례다. Codex 의 출석은
/// 프로세스가 아니라 세션 id 로 찾는다.
///
/// **아무도 아니면 멈춘다** — 보낸 이가 없는 편지는 답할 곳이 없고, 받는 이를 모르면 누구의 편지를 보일지
/// 모른다. 사람을 묻는 쓰기가 "누군지 모르면 멈춘다" 와 같은 결이다(CLAUDE.md). 준 이름이 꼴이 아니어도
/// 멈춘다 — 그 이름이 파일 이름이 된다.
fn who(ctx: &Ctx, given: Option<&str>, agents: &[Presence]) -> R<String> {
    if let Some(name) = given {
        let name = name.trim().to_string();
        return if mail::is_agent_name(&name) { Ok(name) } else { Err(bad_name(ctx.lang(), &name)) };
    }
    let ancestors = mail::ancestors();
    let refused = || Fail::coded(say(ctx.lang(), "refuse.mail_who"), code::NO_ACTOR);
    // **Codex 는 세션 id 로 찾는다**(moai-u5wr.7xr) — 그 셸의 조상은 세션 모두가 함께 쓰는 데몬이라 프로세스로는 이 세션을
    // 못 가른다. Codex 가 셸에 세우는 그 세션의 id 가 훅이 장에 적은 세션과 같다([`mail::codex_session`]). 없으면 `--as` 다.
    //
    // **`MOAI_AGENT` 보다 먼저 본다**(리뷰 moai-ew4o.q9f) — 그 셸의 환경은 데몬의 것이라, 첫 창이 데몬을 띄울 때 든
    // `MOAI_AGENT` 가 모든 Codex 창에 선다. 그것을 먼저 읽던 판은 둘째 창의 `inbox --ack` 가 첫 창의 편지를 가져갔다 —
    // 훅도 Codex 에서는 그 값을 안 읽는다(`attendee`).
    if matches!(mail::agent_among(&ancestors), Some((_, "codex"))) {
        let session = mail::codex_session().ok_or_else(refused)?;
        return agents
            .iter()
            .find(|p| p.session.as_deref() == Some(session.as_str()))
            .map(|p| p.name.clone())
            .ok_or_else(refused);
    }
    if let Some(name) = told_name(ctx.lang())? {
        return Ok(name);
    }
    mail::me_among(agents, &ancestors).map(|p| p.name.clone()).ok_or_else(refused)
}

/// `MOAI_AGENT` — 이 창의 이름(moai-ew4o.e1m). 비었으면 없는 것이고, 꼴이 아니면 멈춘다 — 그 이름이 파일 이름이 된다
/// (`who` 와 `hello` 가 한 자로 잰다 — `hello` 만 말없이 넘기던 판은 인사는 지나가고 그 창의 `inbox` 가 멈췄다).
///
/// **부르는 쪽이 Codex 셸이 아닐 때만 부른다** — 그 환경은 세션 모두가 함께 쓰는 데몬의 것이다(moai-sile).
pub(super) fn told_name(lang: Lang) -> R<Option<String>> {
    match std::env::var("MOAI_AGENT").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        Some(name) if !mail::is_agent_name(&name) => Err(bad_name(lang, &name)),
        told => Ok(told),
    }
}

/// 이름이 될 수 없는 글 — 무엇이 되는지를 함께 댄다.
pub(super) fn bad_name(lang: Lang, name: &str) -> Fail {
    Fail::coded(fill(say(lang, "refuse.mail_name"), &[("name", &crate::text::one_line(name))]), code::BAD_INPUT)
}
