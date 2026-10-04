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

/// `inbox --json` 의 편지 하나 — `id` 와 `read` 에 편지 그대로.
///
/// **편지가 든 모르는 키가 `id`·`read` 면 뺀다**(리뷰 moai-h8tn.x4l) — 모르는 키는 그대로 내는데([`Letter::rest`]),
/// 그 이름이 겉의 키와 겹치면 한 객체에 같은 키가 둘 서고, 읽는 쪽 대부분(jq·python)이 뒤의 것을 믿는다. 손으로
/// 놓거나 다른 판이 쓴 편지 하나가 진짜 id 와 읽음 표를 가린다.
#[derive(Serialize)]
struct Shown<'a> {
    id: &'a str,
    read: bool,
    #[serde(flatten)]
    letter: Letter,
}

impl<'a> Shown<'a> {
    fn of(s: &'a Stored) -> Shown<'a> {
        let mut letter = s.letter.clone();
        letter.rest.retain(|k, _| k != "id" && k != "read");
        Shown { id: &s.id, read: s.reader.is_some(), letter }
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
    if !mail::is_name(&to) {
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
    let woke =
        woke.map(|w| w.unwrap_or_else(|| Woke { to: to.clone(), via: "none", done: false, why: Some("nobody") }));

    if ctx.json {
        return super::json_line(&Sent { id: &id, letter: &letter, wake: woke.as_ref() });
    }
    let mut out = vec![fill(say(ctx.lang(), "mail.sent"), &[("id", &paint(style::ID, &id)), ("to", &to)])];
    if let Some(w) = &woke {
        out.push(woke_line(ctx.lang(), w));
    }
    Ok(out)
}

/// 깨운 결과를 한 줄로. 낱말(`via`·`why`)은 기계의 것이고, 사람 말은 여기서 짓는다.
fn woke_line(lang: Lang, w: &Woke) -> String {
    let to = w.to.as_str();
    match (w.done, w.why) {
        (true, _) => fill(say(lang, "mail.wake_done"), &[("to", to), ("via", w.via)]),
        (false, Some("ask_sender")) => fill(say(lang, "mail.wake_send_message"), &[("to", to)]),
        (false, Some("busy")) => fill(say(lang, "mail.wake_busy"), &[("to", to)]),
        (false, Some("nobody")) => fill(say(lang, "mail.wake_nobody"), &[("to", to)]),
        (false, Some("no_way")) => fill(say(lang, "mail.wake_no_way"), &[("to", to)]),
        (false, why) => fill(say(lang, "mail.wake_failed"), &[("to", to), ("via", w.via), ("why", why.unwrap_or("?"))]),
    }
}

pub fn inbox(ctx: &Ctx, args: InboxArgs) -> R<Vec<String>> {
    let repo = super::open_repo(ctx)?;
    let (agents, _) = mail::presences(&repo.agents_dir());
    let me = who(ctx, args.me.as_deref(), &agents)?;
    let role = agents.iter().find(|p| p.name == me).map(|p| p.role.clone()).unwrap_or_default();
    let dir = repo.mail_dir();

    // **기다림은 훑기를 되풀이한다** — 파일 시스템의 알림(inotify)은 플랫폼마다 다르고 크레이트가 든다. 반 초에
    // 한 번 디렉터리 하나를 읽는 값이 그보다 싸다.
    //
    // **넘치는 값은 끝없이 기다리는 것으로 읽는다**(리뷰 moai-h8tn.x4l) — `Instant + Duration` 은 넘치면 멈춘다(panic).
    // "끝없이" 를 `--wait 9223372036854775807` 로 적는 것은 자연스럽다.
    let until = args.wait.map(|s| std::time::Instant::now().checked_add(std::time::Duration::from_secs(s)));
    let (mut mine, garbled) = loop {
        let (all, garbled) = mail::list(&dir, args.all.then_some(me.as_str()));
        let mine: Vec<Stored> = all.into_iter().filter(|s| mail::for_me(s, &me, &role)).collect();
        let waiting = until.is_some_and(|t| t.is_none_or(|t| std::time::Instant::now() < t));
        if !waiting || mine.iter().any(|s| s.reader.is_none()) {
            break (mine, garbled);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    // 못 읽은 편지는 답을 덜 낸 것이다 — 다 내고 비영으로 끝난다(`show` 의 못 읽는 줄과 같은 자). 자리도 남이 지은
    // 이름이라 한 줄로 접는다 — 파일 이름에 든 제어문자가 터미널을 움직이지 않게.
    //
    // **남의 편지가 깨진 것은 말만 한다**(리뷰 moai-h8tn.x4l, `cmd::mod` 의 "남의 워크트리에서 만난 문제" 와 같은 자) —
    // 받는 이를 읽어 내가 아니면 내 답이 덜 난 것이 아니다. 그것까지 세던 판은 깨진 편지 하나가 지워질 때까지 모든
    // 에이전트의 `inbox` 를 비영으로 끝냈고, `--ack` 로 이미 읽음이 된 편지를 실패로 읽은 고리는 그 편지를 버렸다.
    for g in &garbled {
        tell(&fill(
            say(ctx.lang(), "warn.mail_garbled"),
            &[("path", &crate::text::one_line(&g.path.display().to_string())), ("why", &crate::text::one_line(&g.why))],
        ));
        if g.to.as_deref().is_none_or(|to| to == me || to == mail::ANY_IDLE_WORKER) {
            note_partial();
        }
    }

    let mut lost = Vec::new();
    if args.ack {
        let mut kept = Vec::new();
        for mut s in mine {
            if s.reader.is_some() {
                kept.push(s);
                continue;
            }
            match mail::take(&dir, &s.id, &me) {
                Ok(mail::Took::Mine) => {
                    s.reader = Some(me.clone());
                    kept.push(s);
                }
                Ok(mail::Took::Lost) => lost.push(s.id),
                Err(e) => {
                    tell(&format!("{}: {e}", dir.display()));
                    note_partial();
                    kept.push(s);
                }
            }
        }
        mine = kept;
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

/// 이 부름이 **누구의 이름으로 도는가** — `--as`, `MOAI_AGENT`, 이 명령을 띄운 에이전트의 출석 차례다.
///
/// **아무도 아니면 멈춘다** — 보낸 이가 없는 편지는 답할 곳이 없고, 받는 이를 모르면 누구의 편지를 보일지
/// 모른다. 사람을 묻는 쓰기가 "누군지 모르면 멈춘다" 와 같은 결이다(CLAUDE.md). 준 이름이 꼴이 아니어도
/// 멈춘다 — 그 이름이 파일 이름이 된다.
fn who(ctx: &Ctx, given: Option<&str>, agents: &[Presence]) -> R<String> {
    let told = given.map(str::to_string).or_else(|| std::env::var("MOAI_AGENT").ok().filter(|v| !v.trim().is_empty()));
    if let Some(name) = told {
        let name = name.trim().to_string();
        return if mail::is_agent_name(&name) { Ok(name) } else { Err(bad_name(ctx.lang(), &name)) };
    }
    mail::me_among(agents, &mail::ancestors())
        .map(|p| p.name.clone())
        .ok_or_else(|| Fail::coded(say(ctx.lang(), "refuse.mail_who"), code::NO_ACTOR))
}

/// 이름이 될 수 없는 글 — 무엇이 되는지를 함께 댄다.
pub(super) fn bad_name(lang: Lang, name: &str) -> Fail {
    Fail::coded(fill(say(lang, "refuse.mail_name"), &[("name", &crate::text::one_line(name))]), code::BAD_INPUT)
}
