//! `moai web` — 127.0.0.1 에서 트래커를 페이지로 낸다. 받기 고리와 길.
//!
//! **연결마다 스레드 하나.** 보통 요청은 자식 `moai --json …` 하나를 띄워 그 답을 옮긴다([`run`]).
//! 사건 흐름은 제 스레드에서 1초마다 표식을 재며, 보통 요청의 [`BUSY_MAX`] 와 따로 [`STREAMS_MAX`] 를 센다.
//! 비동기 런타임은 쓰지 않는다.
//!
//! **트래커에 쓰는 길은 계획 날 하나다.** 보통 API 길은 GET 이고, 쓰기 POST 는 둘이다([`api::write_argv_of`]) — 읽음 표시(2026-10-22
//! 사람 결정, moai-ly88)는 사용자 설정 곁의 제 파일에만 적히고, 간트의 막대 끌기(`/api/plan/<id>`, moai-d86e.3ud)는 자식
//! `moai edit --shift*`(하루 세우기는 `--start --due --if-unplanned`, moai-vj8w.02c)로 트래커에 쓴다 — 쓰기는 CLI 의 그
//! 길(`store::with_write`)을 그대로 지난다. 그 밖에 트래커를
//! 바꾸는 길은 여전히 CLI 와 탐색기다. 제어 탭의 답과 마지막 입력도 별도 POST 로 받는다.
//! POST 는 모두 이 페이지에서 보낸 것만 받는다([`http::origin_ok`]).

pub mod api;
pub mod boot;
pub mod ctl;
pub mod http;
pub mod run;

use crate::fail::code;
use crate::i18n::{Lang, fill, say};
use http::{Bad, Request, Response};
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 동시에 도는 연결의 상한 — 넘치면 503 을 쓰고 닫는다. 페이지 하나가 한 번에 여는 부름은 몇 개다.
pub const BUSY_MAX: usize = 32;
/// Long-lived event streams have their own slots and never occupy ordinary request capacity.
pub const STREAMS_MAX: usize = 8;
/// 요청 머리 **전체**를 읽는 시한 — `read` 한 번의 시한이 아니다([`read_head`]).
const READ_LIMIT: Duration = Duration::from_secs(5);
/// 503 뒤에 늦게 오는 요청 머리를 별도 스레드에서 비우는 전체 시한. 조용하거나 한 바이트씩 보내는
/// 상대도 200ms 뒤에는 놓는다 — 받기 고리는 이 시한을 기다리지 않는다.
const BUSY_READ_LIMIT: Duration = Duration::from_millis(200);
/// 초과 연결을 비우는 스레드의 상한. 정상 연결의 자리와 따로 세며, 다 차면 기다리지 않고 닫는다.
const BUSY_DRAIN_MAX: usize = 4;
/// 쓰기 한 번이 나아가지 못하고 기다리는 시한.
const WRITE_LIMIT: Duration = Duration::from_secs(10);
/// [`Within`] 이 한 번에 쓰는 몫의 상한.
const WRITE_CHUNK: usize = 64 * 1024;
/// 답 **전체**를 쓰는 시한 — 쓰기마다 조금씩 나아가는 느린 상대가 큰 답 하나로 한 자리를 몇 시간 쥐지 않게
/// ([`Within`]). 루프백과 SSH 터널로 보는 페이지에는 넉넉하다.
const ANSWER_LIMIT: Duration = Duration::from_secs(60);

/// 서버를 띄울 때 한 번 정하고 모든 연결이 나눠 읽는 것.
pub struct Site {
    pub spawner: run::Spawner,
    /// 띄울 때 고른 말 — **받는 고리만 쓴다**(503 `busy`·받기 오류). 고리는 파일을 안 연다: 사용자 설정 자리에 무엇이
    /// 서 있든(FIFO 하나면 열기가 끝없이 선다) 다른 연결을 받는 일이 거기 걸리면 안 된다.
    pub started_in: Lang,
    /// 사용자 설정의 표식과 그것에서 읽은 것([`Site::config`]).
    picked: Mutex<Picked>,
    streams: AtomicUsize,
    pub tabs: Arc<ctl::Tabs>,
}

/// 사용자 설정을 한 번 풀어 든 값 — **표식이 같은 동안 다시 안 푼다.** 말과 걸어 둔 프로젝트가 한 번의 풀기를 나눠 쓴다.
struct Picked {
    /// 이 값을 푼 때의 사용자 설정 표식 — 바깥 `None` 은 아직 한 번도 안 풀었다는 뜻이다(설정이 없는 기계의 `Some(None)` 과 다르다).
    stamp: Option<Option<crate::store::Stamp>>,
    lang: Lang,
    /// 걸어 둔 프로젝트의 자리들 — `?project=` 요청마다 설정을 통째로 다시 풀던 판은 2 초 표식 묻기에도 디버그에서
    /// 44ms(릴리스 4~6ms)를 들였다(moai-m4ez.4zq).
    projects: Vec<std::path::PathBuf>,
}

impl Site {
    pub fn new(spawner: run::Spawner, started_in: Lang) -> Site {
        Site {
            spawner,
            started_in,
            picked: Mutex::new(Picked { stamp: None, lang: started_in, projects: Vec::new() }),
            streams: AtomicUsize::new(0),
            tabs: Arc::new(ctl::Tabs::new()),
        }
    }

    /// 이 요청의 말 — **설정이 바뀌면 다시 고른다**([`crate::cmd::lang_of`] 의 자, `MOAI_LANG` 이 이긴다). 자식은
    /// 띄울 때마다 사용자 설정을 다시 읽어 말을 고르는데, 서버만 띄울 때 고른 말을 들던 판은 설정의 `lang` 을 고친
    /// 뒤 `/api/boot`·서버의 거절은 옛 말, 자식의 답은 새 말로 갈렸다 — [`boot`] 가 적어 둔 대로 다시 띄워야 서는
    /// 값은 캐시다.
    ///
    /// **묻는 것은 표식(`stat`)이다** — 요청마다 설정을 통째로 풀던 판은 옛 `[read]` 가 큰 설정에서 정적 파일 하나에
    /// 수백 ms 를 들였다. 표식이 바뀐 때만 다시 읽고, **보통 파일만 연다** — FIFO·장치는 열기가 설 수 있다.
    pub fn lang(&self) -> Lang {
        self.config(|picked| picked.lang)
    }

    /// 걸어 둔 프로젝트들의 자리 — [`Site::lang`] 과 **같은 풀기**를 나눠 쓴다. `?project=` 가 붙은 요청마다(2 초 표식
    /// 묻기 포함) 설정을 새로 풀던 판의 값을 표식이 같은 동안 한 번으로 줄인다(moai-m4ez.4zq).
    fn registered(&self) -> Vec<std::path::PathBuf> {
        self.config(|picked| picked.projects.clone())
    }

    fn config<T>(&self, take: impl FnOnce(&Picked) -> T) -> T {
        let path = crate::user_config::path();
        let stamp = path.as_deref().map(crate::store::stamp);
        let mut picked = self.picked.lock().unwrap_or_else(|e| e.into_inner());
        if picked.stamp != Some(stamp) {
            let regular = path.as_deref().is_none_or(|p| std::fs::metadata(p).map_or(true, |m| m.is_file()));
            let read = match regular {
                true => crate::user_config::read(path.as_deref()),
                false => crate::user_config::read(None),
            };
            *picked = Picked {
                stamp: Some(stamp),
                lang: crate::cmd::lang_of(&read),
                projects: read.projects.iter().map(|p| p.path.clone()).collect(),
            };
        }
        take(&picked)
    }
}

/// 받기 고리 — **안 돌아온다.** Ctrl-C 가 끝이고, 치울 것이 없다(쓴 파일도 쥔 락도 없다).
pub fn serve(listener: TcpListener, site: Site) -> ! {
    let site = Arc::new(site);
    let busy = Arc::new(AtomicUsize::new(0));
    let draining = Arc::new(AtomicUsize::new(0));
    loop {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            // 받기 오류(손이 모자란 판의 EMFILE 따위)는 그 연결 하나의 것이다 — 서버를 안 내린다.
            Err(e) => {
                crate::cmd::tell(&fill(say(site.started_in, "web.accept_failed"), &[("said", &e.to_string())]));
                // 같은 오류가 곧바로 다시 나면 고리가 CPU 를 다 먹는다.
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if busy.fetch_add(1, Ordering::SeqCst) >= BUSY_MAX {
            busy.fetch_sub(1, Ordering::SeqCst);
            turn_away(stream, site.started_in, &draining);
            continue;
        }
        // **센 것은 실을 띄우기 전에 쥔다** — `std::thread::spawn` 은 OS 가 실을 못 내면 이 고리에서 패닉해 서버가
        // 통째로 내려갔다(`git::finish_within` 이 `Builder` 로 띄우는 것과 같은 까닭). 못 띄우면 닫힘째 버려지며
        // 자리를 돌려놓고, 그 연결에는 남겨 둔 손잡이로 503 을 쓴다.
        let spare = stream.try_clone().ok();
        let leave = Leave(busy.clone());
        let site_of = site.clone();
        let spawned = std::thread::Builder::new().spawn(move || {
            handle(stream, &site_of, leave);
        });
        if spawned.is_err()
            && let Some(spare) = spare
        {
            turn_away(spare, site.started_in, &draining);
        }
    }
}

/// 연결이나 drainer 하나가 쥔 자리 — 스레드가 어떻게 끝나든(패닉도, 못 띄운 판도) 센 것을 돌려놓는다.
struct Leave(Arc<AtomicUsize>);

impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 받을 자리가 없는 연결에 503 `busy` 와 FIN 을 쓰고, 늦은 요청은 별도 스레드에서 짧게 비운다.
/// drainer 의 자리도 다 찼으면 즉시 놓는다 — 조용한 초과 연결들이 받기 고리를 줄 세우면 안 된다.
fn turn_away(stream: TcpStream, lang: Lang, draining: &Arc<AtomicUsize>) {
    turn_away_within(stream, lang, draining, BUSY_READ_LIMIT);
}

fn turn_away_within(stream: TcpStream, lang: Lang, draining: &Arc<AtomicUsize>, limit: Duration) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = http::write(&mut &stream, &refuse(503, say(lang, "web.busy"), code::BUSY), true);
    // 요청을 비우는 동안에도 상대는 답 끝을 바로 본다. 읽기 쪽은 늦은 요청을 받을 수 있게 열어 둔다.
    let _ = stream.shutdown(std::net::Shutdown::Write);
    if draining.fetch_add(1, Ordering::SeqCst) >= BUSY_DRAIN_MAX {
        draining.fetch_sub(1, Ordering::SeqCst);
        close(&stream);
        return;
    }
    let leave = Leave(draining.clone());
    // 이미 온 것만 비우고 닫으면 connect 뒤 밀린 상대가 요청을 쓰다가 BrokenPipe 를 받는다. 받기 고리에서는
    // 기다리지 않는다. spawn 실패도 닫힘과 자리 반환으로 끝나며, 스레드를 무제한 띄우거나 대기열을 만들지 않는다.
    let _ = std::thread::Builder::new().name("moai-web-drain".into()).spawn(move || {
        let _leave = leave;
        // 답은 이미 정했다. 제한된 크기·시간 안에서 머리를 버리고, 해석 결과로 다시 답하지 않는다.
        let _ = read_head(&stream, limit);
        close(&stream);
    });
}

fn handle(stream: TcpStream, site: &Site, busy: Leave) {
    let (answer, body) = match read_head(&stream, READ_LIMIT) {
        // 시한·끊김은 답할 상대가 없다.
        Err(Bad::Io) => return,
        Err(bad) => (refuse(400, &bad_said(&bad, site.lang()), code::BAD_INPUT), true),
        Ok(req) if req.path.starts_with("/api/ctl/") => (ctl_post(&stream, &req, site), true),
        Ok(req) if req.content_length != 0 => (refuse(400, say(site.lang(), "web.body"), code::BAD_INPUT), true),
        Ok(req) if req.path == "/api/events" => {
            let target = event_target(&req, site);
            match target {
                Ok((cwd, lang, overlay, first, resume)) => {
                    if site.streams.fetch_add(1, Ordering::SeqCst) >= STREAMS_MAX {
                        site.streams.fetch_sub(1, Ordering::SeqCst);
                        (refuse(503, say(lang, "web.busy"), code::BUSY), true)
                    } else {
                        struct StreamLeave<'a>(&'a AtomicUsize);
                        impl Drop for StreamLeave<'_> {
                            fn drop(&mut self) {
                                self.0.fetch_sub(1, Ordering::SeqCst);
                            }
                        }
                        let _stream = StreamLeave(&site.streams);
                        drop(busy);
                        let ctl_peer = stream.peer_addr().is_ok_and(local_ctl_peer);
                        let _ = stream_events(
                            &stream,
                            &cwd,
                            lang,
                            overlay,
                            first,
                            &site.tabs,
                            ctl_peer,
                            resume.as_ref().map(|(id, token)| (id.as_str(), token.as_str())),
                        );
                        close(&stream);
                        return;
                    }
                }
                Err(refused) => (refused, true),
            }
        }
        // HEAD 의 답은 머리만 쓴다([`http::write`]).
        Ok(req) => (answer(&req, site), req.method != "HEAD"),
    };
    let _busy = busy;
    let _ = http::write(&mut Within::of(&stream, ANSWER_LIMIT), &answer, body);
    close(&stream);
}

fn ctl_post(stream: &TcpStream, req: &Request, site: &Site) -> Response {
    let lang = site.lang();
    if !http::host_ok(req.host.as_deref()) {
        return refuse(403, say(lang, "web.foreign_host"), code::FORBIDDEN_HOST);
    }
    if req.method != "POST" {
        return Response { allow: "POST", ..refuse(405, "control replies require POST", code::METHOD_NOT_ALLOWED) };
    }
    if !http::origin_ok(req.origin.as_deref(), req.host.as_deref()) {
        return refuse(403, say(lang, "web.foreign_origin"), code::FORBIDDEN_ORIGIN);
    }
    let Some(path) = req.path.strip_prefix("/api/ctl/") else {
        return refuse(404, "unknown control path", code::NOT_FOUND);
    };
    let (id, seen, proposal) = if let Some(id) = path.strip_suffix("/seen") {
        (id, true, false)
    } else if let Some(id) = path.strip_suffix("/proposal") {
        (id, false, true)
    } else {
        (path, false, false)
    };
    if id.is_empty() || id.contains('/') {
        return refuse(404, "unknown control tab", code::NOT_FOUND);
    }
    let mut token = None;
    let mut seq = None;
    let mut screen = None;
    let mut passive = None;
    let mut proposal_id = None;
    for (key, value) in &req.query {
        match key.as_str() {
            "token" if token.replace(value.as_str()).is_none() => {}
            "seq" if !seen && seq.replace(value.parse::<u64>().ok()).is_none() => {}
            "screen" if seen && screen.replace(value.as_str()).is_none() => {}
            "passive" if seen && value == "1" && passive.replace(true).is_none() => {}
            "id" if proposal && proposal_id.replace(value.as_str()).is_none() => {}
            _ => return refuse(400, "invalid control query", code::BAD_INPUT),
        }
    }
    let Some(token) = token else { return refuse(400, "control token is required", code::BAD_INPUT) };
    let result = if proposal {
        let Some(proposal_id) = proposal_id else { return refuse(400, "proposal id is required", code::BAD_INPUT) };
        if req.content_type.as_deref() != Some("application/json") {
            return refuse(400, "proposal answer must be application/json", code::BAD_INPUT);
        }
        let bytes = match http::read_body(&mut Within::of(stream, READ_LIMIT), req, crate::ctl::wire::LINE_MAX) {
            Ok(bytes) => bytes,
            Err(Bad::TooBig) => return refuse(413, "proposal answer exceeds 1 MiB", code::TOO_LARGE),
            Err(bad) => return refuse(400, &bad_said(&bad, lang), code::BAD_INPUT),
        };
        let body: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => return refuse(400, "invalid proposal answer", code::BAD_INPUT),
        };
        let accepted = match body["state"].as_str() {
            Some("accepted") => true,
            Some("declined") => false,
            _ => return refuse(400, "proposal answer needs accepted or declined", code::BAD_INPUT),
        };
        let ran = match body.get("ran") {
            Some(value) if !value.is_null() => match serde_json::from_value::<crate::ctl::wire::Reply>(value.clone()) {
                Ok(reply) => Some(reply),
                Err(_) => return refuse(400, "invalid proposal action result", code::BAD_INPUT),
            },
            _ => None,
        };
        return match site.tabs.proposal_reply(id, token, proposal_id, accepted, ran) {
            Ok(reply) if reply.ok => {
                Response::new(200, http::JSON, serde_json::to_vec(&reply.data).unwrap_or_default())
            }
            Ok(reply) => refuse(409, reply.message.as_deref().unwrap_or("proposal already answered"), code::REFUSED),
            Err("gone") => refuse(410, "tab is gone", code::GONE),
            Err(_) => refuse(403, "control token does not match", code::FORBIDDEN_ORIGIN),
        };
    } else if seen {
        if req.content_length != 0 {
            return refuse(400, say(lang, "web.body"), code::BAD_INPUT);
        }
        let Some(screen) = screen else { return refuse(400, "screen is required", code::BAD_INPUT) };
        site.tabs.seen(id, token, screen, passive.is_none())
    } else {
        let Some(Some(seq)) = seq else { return refuse(400, "sequence is required", code::BAD_INPUT) };
        if req.content_type.as_deref() != Some("application/json") {
            return refuse(400, "control reply must be application/json", code::BAD_INPUT);
        }
        let bytes = match http::read_body(&mut Within::of(stream, READ_LIMIT), req, crate::ctl::wire::LINE_MAX) {
            Ok(bytes) => bytes,
            Err(Bad::TooBig) => return refuse(413, "control reply exceeds 1 MiB", code::TOO_LARGE),
            Err(bad) => return refuse(400, &bad_said(&bad, lang), code::BAD_INPUT),
        };
        let reply = match serde_json::from_slice::<crate::ctl::wire::Reply>(&bytes) {
            Ok(reply) => reply,
            Err(_) => return refuse(400, "invalid control reply", code::BAD_INPUT),
        };
        site.tabs.reply(id, token, seq, reply)
    };
    match result {
        Ok(()) => Response::new(200, http::JSON, b"{}".to_vec()),
        Err("gone") => refuse(410, "tab or request is gone", code::GONE),
        Err("forbidden") => refuse(403, "control token does not match", code::FORBIDDEN_ORIGIN),
        Err(_) => refuse(400, "invalid control request", code::BAD_INPUT),
    }
}

fn event_target(
    req: &Request,
    site: &Site,
) -> Result<(std::path::PathBuf, Lang, bool, String, Option<(String, String)>), Response> {
    let lang = site.lang();
    if !http::host_ok(req.host.as_deref()) {
        return Err(refuse(403, say(lang, "web.foreign_host"), code::FORBIDDEN_HOST));
    }
    if req.method != "GET" {
        let said = fill(say(lang, "web.only_get"), &[("method", &req.method), ("path", &req.path)]);
        return Err(Response { allow: "GET", ..refuse(405, &said, code::METHOD_NOT_ALLOWED) });
    }
    let (project, query) =
        api::split_project(&req.query).map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT))?;
    let cwd = match project {
        Some(project) => project_of(site, &project, lang)?,
        None => site.spawner.cwd.clone(),
    };
    let mut resume = None;
    let mut token = None;
    let mut rest = Vec::new();
    for (key, value) in query {
        match key.as_str() {
            "resume" if resume.is_none() && value.len() <= 80 => resume = Some(value),
            "token" if token.is_none() && value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()) => {
                token = Some(value)
            }
            "resume" | "token" => return Err(refuse(400, "invalid event resume", code::BAD_INPUT)),
            _ => rest.push((key, value)),
        }
    }
    if resume.is_some() != token.is_some() {
        return Err(refuse(400, "event resume needs id and token", code::BAD_INPUT));
    }
    let overlay = api::overlay_of(&rest).map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT))?;
    let first = api::stamp(&cwd, lang, overlay).map_err(|e| refuse(500, &e.message, e.code))?;
    Ok((cwd, lang, overlay, first, resume.zip(token)))
}

fn local_ctl_peer(peer: std::net::SocketAddr) -> bool {
    peer.ip().is_loopback()
}

fn stream_events(
    stream: &TcpStream,
    cwd: &std::path::Path,
    lang: Lang,
    overlay: bool,
    first: String,
    tabs: &ctl::Tabs,
    ctl_peer: bool,
    resume: Option<(&str, &str)>,
) -> std::io::Result<()> {
    use std::io::Write as _;
    // Local ctl sockets are Unix-only; the tracker stream remains available on Windows.
    // A remote page can read the web UI when the server binds externally, but it must never receive a ctl request,
    // the caller's actor, or a token that can answer or write as that caller. SSH port forwarding arrives on loopback.
    let joined = if cfg!(unix) && ctl_peer { Some(tabs.join(cwd, overlay, resume)?) } else { None };
    struct TabLeave<'a>(&'a ctl::Tabs, String, u64);
    impl Drop for TabLeave<'_> {
        fn drop(&mut self) {
            self.0.leave(&self.1, self.2);
        }
    }
    let _tab = joined.as_ref().map(|joined| TabLeave(tabs, joined.id.clone(), joined.stream));
    let mut writer = Within::of(stream, WRITE_LIMIT);
    http::write_stream_head(&mut writer)?;
    writer.write_all(b"retry: 3000\n\n")?;
    if let Some(joined) = &joined {
        writer.write_all(
            format!("event: hello\ndata: {}\n\n", serde_json::json!({"tab": joined.id, "token": joined.token}))
                .as_bytes(),
        )?;
    }
    writer.write_all(format!("event: stamp\ndata: {{\"stamp\":\"{first}\"}}\n\n").as_bytes())?;
    writer.flush()?;
    let (until, mut ping, mut seen) = (Instant::now() + Duration::from_secs(30 * 60), Instant::now(), first);
    while Instant::now() < until {
        let pushed = match &joined {
            Some(joined) => joined.incoming.recv_timeout(Duration::from_secs(1)).ok(),
            None => {
                std::thread::sleep(Duration::from_secs(1));
                None
            }
        };
        // A closed tab need not wait for the next changed stamp or fifteen-second ping to return its slot.
        stream.set_read_timeout(Some(Duration::from_millis(1)))?;
        match stream.peek(&mut [0u8; 1]) {
            Ok(0) => break,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => break,
            Ok(_) => {}
        }
        if let Some(push) = pushed {
            let mut writer = Within::of(stream, WRITE_LIMIT);
            let (name, data) = match push {
                ctl::Push::Ctl { seq, request } => ("ctl", serde_json::json!({"seq": seq, "request": request})),
                ctl::Push::ValidateZone { seq, name } => {
                    ("validate_zone", serde_json::json!({"seq": seq, "name": name}))
                }
                ctl::Push::Proposal { entry } => ("proposal", entry),
                ctl::Push::ProposalClosed { id } => ("proposal_closed", serde_json::json!({"id": id})),
            };
            writer.write_all(format!("event: {name}\ndata: {data}\n\n").as_bytes())?;
            writer.flush()?;
        }
        let stamp = match api::stamp(cwd, lang, overlay) {
            Ok(s) => s,
            Err(_) => break,
        };
        if stamp != seen {
            let mut writer = Within::of(stream, WRITE_LIMIT);
            writer.write_all(format!("event: stamp\ndata: {{\"stamp\":\"{stamp}\"}}\n\n").as_bytes())?;
            writer.flush()?;
            seen = stamp;
        }
        if ping.elapsed() >= Duration::from_secs(15) {
            let mut writer = Within::of(stream, WRITE_LIMIT);
            writer.write_all(b": ping\n\n")?;
            writer.flush()?;
            ping = Instant::now();
        }
    }
    Ok(())
}

/// 요청 머리를 `limit` 안에 **다** 읽는다([`Within`]).
fn read_head(stream: &TcpStream, limit: Duration) -> Result<Request, Bad> {
    http::read_allow_body(&mut Within::of(stream, limit))
}

/// 연결 하나를 **한 시한 안에서** 읽고 쓰는 손잡이. `set_read_timeout`·`set_write_timeout` 은 부름 한 번의 시한이라,
/// 몇 초마다 한 바이트씩 흘리는 상대가 머리를 끝없이 늘이고(서른둘이면 모든 부름이 503 이다), 조금씩 읽는 상대가
/// 큰 답 하나로 한 자리를 몇 시간 쥐었다 — 9.5 MB 의 목록을 4 KB 씩 5 초마다 읽는 상대가 50 초 뒤에도 받고 있었다.
/// 그래서 부를 때마다 남은 만큼만 기다린다(쓰기는 그래도 [`WRITE_LIMIT`] 넘게는 안).
struct Within<'a> {
    stream: &'a TcpStream,
    until: Instant,
}

impl<'a> Within<'a> {
    fn of(stream: &'a TcpStream, limit: Duration) -> Within<'a> {
        Within { stream, until: Instant::now() + limit }
    }

    /// 남은 시한 — 다 썼으면 `TimedOut` 이다(`set_*_timeout(Some(0))` 은 거절된다).
    fn left(&self) -> std::io::Result<Duration> {
        let left = self.until.saturating_duration_since(Instant::now());
        match left.is_zero() {
            true => Err(std::io::ErrorKind::TimedOut.into()),
            false => Ok(left),
        }
    }
}

impl std::io::Read for Within<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.left()?))?;
        let mut s = self.stream;
        s.read(buf)
    }
}

impl std::io::Write for Within<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.set_write_timeout(Some(self.left()?.min(WRITE_LIMIT)))?;
        let mut s = self.stream;
        // **한 번에 쓰는 몫을 끊는다** — Windows 의 `send` 는 큰 버퍼를 한 부름에 받아 두고 돌아와, 안 읽는 상대에게도
        // 32 MiB 가 시한 확인 없이 "다 쓰였다". 몫마다 남은 시한을 다시 재야 답 전체의 시한이 어느 OS 에서나 선다.
        s.write(&buf[..buf.len().min(WRITE_CHUNK)])
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut s = self.stream;
        s.flush()
    }
}

/// 답을 쓴 뒤 닫는다. **안 읽은 바이트를 둔 채 닫으면 커널이 FIN 대신 RST 를 보낸다** — 머리를 안 읽은 503 과
/// 머리를 다 안 읽은 400 이 그 꼴이라, 상대는 방금 쓴 답 대신 "연결이 끊겼다" 를 받았다(503 을 스무 번 불러 열여덟
/// 번이 `ECONNRESET` 이었다). 쓰기를 닫아 FIN 을 보낸 뒤 이미 와 있는 것만 기다리지 않고 비운다.
/// 503 은 [`turn_away`] 에서 답 직후에 쓰기를 닫고 별도 drainer 가 늦은 요청을 짧게 기다린다 — 여기서는
/// 그 닫기를 거듭해도 같다.
fn close(stream: &TcpStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    if stream.set_nonblocking(true).is_err() {
        return;
    }
    let mut sink = [0u8; 4096];
    let mut left = http::HEAD_MAX * 4;
    let mut s = stream;
    while left > 0 {
        match s.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => left = left.saturating_sub(n),
        }
    }
}

fn bad_said(bad: &Bad, lang: Lang) -> String {
    match bad {
        Bad::TooBig => fill(say(lang, "web.head_too_big"), &[("max", &(http::HEAD_MAX / 1024).to_string())]),
        Bad::Body => say(lang, "web.body").to_string(),
        Bad::Encoding => say(lang, "web.encoding").to_string(),
        Bad::Malformed | Bad::Io => say(lang, "web.malformed").to_string(),
    }
}

/// 요청 하나의 답. **Host 가 먼저다** — 남의 이름으로 온 부름에는 어떤 길이 있는지도 안 알려 준다.
pub fn answer(req: &Request, site: &Site) -> Response {
    let lang = site.lang();
    if !http::host_ok(req.host.as_deref()) {
        return refuse(403, say(lang, "web.foreign_host"), code::FORBIDDEN_HOST);
    }
    let not_allowed = || {
        let said = fill(say(lang, "web.only_get"), &[("method", &req.method), ("path", &req.path)]);
        // **`Allow` 는 그 길이 받는 메서드다** — 쓰는 길(읽음 표시·계획 날)은 `POST` 만 받는다(`GET` 은 404 라 여기 안 온다).
        let allow = if api::write_argv_of(&req.path, &[]).is_some() { "POST" } else { "GET" };
        Response { allow, ..refuse(405, &said, code::METHOD_NOT_ALLOWED) }
    };
    // **쓰는 길은 이 페이지가 보낸 것만 받는다**([`http::origin_ok`]) — 무엇을 쓰는지 재기 전에, 프로젝트를 찾으러
    // 사용자 설정을 열기 전에 거른다.
    let write = match req.method.as_str() {
        "GET" => false,
        "POST" if api::write_argv_of(&req.path, &[]).is_some() => {
            if !http::origin_ok(req.origin.as_deref(), req.host.as_deref()) {
                return refuse(403, say(lang, "web.foreign_origin"), code::FORBIDDEN_ORIGIN);
            }
            true
        }
        _ => return not_allowed(),
    };
    // **페이지와 그 파일은 쿼리를 안 본다** — 이 범위 전에도 그랬다. `project=` 를 재기 전에 내야, 걷은 프로젝트를 가리키는
    // 책갈피(`/?project=…`)도 페이지가 열리고 그 첫 부름(`/api/boot`)이 `not_registered` 로 까닭을 댄다. 날 JSON 한 장이
    // 페이지 자리에 서면 돌아갈 길도 안 보인다.
    match req.path.as_str() {
        "/" | "/index.html" => return page(http::HTML, include_str!("dist/index.html")),
        "/app.js" => return page(http::JS, include_str!("dist/app.js")),
        // **페이지의 첫 스크립트다**(moai-whvr) — `<head>` 에서 `defer` 없이 불려 몸이 그려지기 전에 고른 테마를 건다.
        // app.js 는 `defer` 라 그 전에 시스템 색이 한 번 그려진다. CSP 가 인라인을 막아 파일로 낸다.
        "/theme.js" => return page(http::JS, include_str!("dist/theme.js")),
        "/app.css" => return page(http::CSS, include_str!("dist/app.css")),
        _ => {}
    }
    let (project, query) = match api::split_project(&req.query) {
        Ok(split) => split,
        Err(bad) => return refuse(400, &bad.said(lang), code::BAD_INPUT),
    };
    // 걸어 둔 목록(`/api/projects`)은 사용자 설정만 읽어 어느 자리에서 돌아도 같은 답이다 — 자리를 안 잰다. 걷은 프로젝트를
    // 연 페이지도 머리글이 그 목록을 받아 다른 프로젝트로 건너간다.
    let cwd = match project.as_deref().filter(|_| req.path != "/api/projects").map(|p| project_of(site, p, lang)) {
        None => site.spawner.cwd.clone(),
        Some(Ok(dir)) => dir,
        Some(Err(refused)) => return refused,
    };
    if write {
        let (ordinary, ctl) = match ctl_write_query(&query) {
            Ok(parts) => parts,
            Err(error) => return error,
        };
        let actor = match ctl {
            Some((seq, token)) => match site.tabs.actor(&token, seq, &cwd) {
                Ok(actor) => Some(actor),
                Err("no_actor") => return refuse(400, "control caller has no actor", code::NO_ACTOR),
                Err("forbidden") => return refuse(403, "control token does not match", code::FORBIDDEN_ORIGIN),
                Err(_) => return refuse(410, "control request is gone", code::GONE),
            },
            None => None,
        };
        return match api::write_argv_of(&req.path, &ordinary) {
            Some(Ok(args)) => child_as(site, &overlay_in_git(args, &cwd), &cwd, None, lang, actor.as_deref()),
            Some(Err(bad)) => refuse(400, &bad.said(lang), code::BAD_INPUT),
            None => not_allowed(),
        };
    }
    // 자식 없이 답하는 끝점도 받는 이름만 받는다(`/api/boot` 는 없고 `/api/stamp` 는 `worktree=1` 하나) — 말없이 버리면
    // 부른 쪽은 제가 준 것이 먹은 답이라 믿는다([`api::argv`]).
    let bare = || api::none_of(&query).map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT));
    match req.path.as_str() {
        "/api/boot" => match bare().and_then(|()| {
            boot::boot(&cwd, site.spawner.user.as_deref(), lang, &site.registered())
                .map_err(|e| refuse(500, &e.message, e.code))
        }) {
            Ok(body) => Response::new(200, http::JSON, body.into_bytes()),
            Err(refused) => refused,
        },
        "/api/stamp" => match api::overlay_of(&query)
            .map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT))
            .and_then(|on| api::stamp(&cwd, lang, on).map_err(|e| refuse(500, &e.message, e.code)))
        {
            Ok(stamp) => Response::new(200, http::JSON, format!("{{\"stamp\":\"{stamp}\"}}").into_bytes()),
            Err(refused) => refused,
        },
        path => match api::argv_of(path, &query) {
            Some(Ok(args)) => child(site, &overlay_in_git(args, &cwd), &cwd, zone_for_child(&query), lang),
            Some(Err(bad)) => refuse(400, &bad.said(lang), code::BAD_INPUT),
            None => {
                let said = fill(say(lang, "web.no_path"), &[("path", &req.path)]);
                refuse(404, &said, code::NOT_FOUND)
            }
        },
    }
}

/// Peel off the live ctl request's identity, leaving the write endpoint's own query untouched.
fn ctl_write_query(query: &[(String, String)]) -> Result<(Vec<(String, String)>, Option<(u64, String)>), Response> {
    let mut ordinary = Vec::new();
    let (mut seq, mut token) = (None, None);
    for (key, value) in query {
        match key.as_str() {
            "ctl" if seq.replace(value.parse::<u64>().ok()).is_none() => {}
            "token" if token.replace(value.clone()).is_none() => {}
            "ctl" | "token" => return Err(refuse(400, "duplicate control query", code::BAD_INPUT)),
            _ => ordinary.push((key.clone(), value.clone())),
        }
    }
    let ctl = match (seq, token) {
        (None, None) => None,
        (Some(Some(seq)), Some(token)) if !token.is_empty() => Some((seq, token)),
        _ => return Err(refuse(400, "control sequence and token must be given together", code::BAD_INPUT)),
    };
    Ok((ordinary, ctl))
}

/// `project=` 의 자리 — **걸어 둔 프로젝트만 읽는다**(moai-o14f). 페이지는 `/api/projects` 가 낸 `path` 를 그대로
/// 되돌려 보내니, 걸어 둔 목록에 없는 자리는 남이 지어 보낸 것이다: 이 서버가 그 디렉터리의 트래커를 읽어 줄 까닭이
/// 없다. 견주기는 `project ls` 와 같은 자([`crate::user_config::same_dir`])다.
///
/// **걸어 둔 목록은 [`Site::registered`] 가 표식이 같은 동안 한 번만 푼 것이다**(moai-m4ez.4zq). 사용자 설정은 보통 파일일
/// 때만 연다([`Site::lang`] 과 같은 자) — 아니면 걸어 둔 것이 없는 것으로 읽는다.
///
/// **걸어 두었어도 제 `.moai` 가 없으면 프로젝트가 아니다**(404 `not_a_project`). 자식이 [`crate::store::Repo::find_from`] 으로
/// 위로 찾아 올라가, `moai project add` 는 받아 주는 init 전 디렉터리(모노레포의 `apps/a`)가 부모의 트래커로 답했다 —
/// 남의 트래커를 이 프로젝트의 것으로 그리는 것이다. 열기는 위로 안 찾는 [`crate::store::Repo::open`] 이고 `project ls`
/// 와 탐색기가 쓰는 그 자라서, 딸린 워크트리의 비친 트래커는 거기서 그대로 선다. **못 연 것(깨진 설정)은 안 막는다** —
/// 자식이 같은 까닭을 제 말로 댄다.
fn project_of(site: &Site, value: &str, lang: Lang) -> Result<std::path::PathBuf, Response> {
    let asked = std::path::Path::new(value);
    let found = match asked.is_absolute() {
        true => site.registered().into_iter().find(|p| crate::user_config::same_dir(p, asked)),
        false => None,
    };
    let Some(dir) = found else {
        let said = fill(say(lang, "web.not_registered"), &[("path", value)]);
        return Err(refuse(403, &said, code::NOT_REGISTERED));
    };
    // **`.moai` 가 섰는지는 `stat` 하나로 가른다**(moai-m4ez.u6a) — `Repo::open` 은 설정을 파싱하고 `held::check` 를 돌려, 2초마다
    // 도는 `/api/stamp` 도 요청마다 그것을 치렀다. 열기와 같은 자리(`opened_root`: 딸린 워크트리는 트래커가 선 주 체크아웃)에서
    // 같은 자(`spot`)로 잰다 — 디렉터리가 아닌 `.moai` 나 못 들여다본 자리는 자식이 제 말로 대니 안 막는다.
    match crate::store::spot(&crate::store::Repo::opened_root(&dir)) {
        crate::store::Spot::Absent => {
            let said = fill(say(lang, "web.not_a_project"), &[("path", value)]);
            Err(refuse(404, &said, code::NOT_A_PROJECT))
        }
        _ => Ok(dir),
    }
}

/// 겹쳐 보기(`--worktree`)는 git 체크아웃 안에서만 붙인다. 밖에서는 옆 워크트리가 있을 수 없는데 `show --worktree` 가
/// 부를 때마다 "옆 워크트리를 못 찾았다" 를 stderr 로 내고(`Gathered::unfound`), 페이지는 겹쳐 보기를 켠 채로 열어(`SPC v w`)
/// 그 말이 읽을 때마다 "moai 가 한 말" 로 섰다. 탐색기는 시키지 않은 겹쳐 보기의 그 까닭을 안 세운다(moai-d5vn) — 떼어도
/// 줄은 같다: 겹칠 옆이 없다. git 이 저장소를 찾는 자리(`.git` 이 선 조상, `worktree::own_git` 과 같은 자)로 잰다 — git 을
/// 안 띄운다.
///
/// **떼는 것은 `--` 앞의 깃발뿐이다**(리뷰 moai-xp7l.mug) — 뒤는 값이다(`/api/wiki-search` 의 낱말, `show -- <id>`). 다 떼던
/// 판은 `--worktree` 라는 낱말을 찾는 위키 검색에서 그 낱말을 지워, 자식이 "낱말이 없다" 로 2 를 냈다.
fn overlay_in_git(mut args: Vec<String>, cwd: &std::path::Path) -> Vec<String> {
    if !cwd.ancestors().any(|dir| dir.join(".git").exists()) {
        let values = args.iter().position(|arg| arg == "--").unwrap_or(args.len());
        let mut at = 0;
        args.retain(|arg| {
            at += 1;
            at > values || arg != "--worktree"
        });
    }
    args
}

/// 페이지가 그리는 시간대(읽기 끝점의 `tz=`, [`api::zone_of`]) 가운데 **이 기계가 여는 것만** 자식의 `TZ` 로 준다
/// (moai-vj8w 리뷰). 브라우저(V8)가 내는 이름은 CLDR 의 정본이라 `Asia/Kolkata` 가 `Asia/Calcutta` 로, `Europe/Kyiv` 가
/// `Europe/Kiev` 로 오는데, 옛 이름을 따로 까는 tzdata(우분투 24.04 는 `tzdata-legacy`)에는 그 파일이 없다. 그대로 넘기면
/// 자식이 UTC 로 떨어져(`tz::Zone::named` — 유닉스는 기계로 안 내려간다) 서버의 시간대로 재던 그 전보다 나빠지고, 그 까닭은
/// 봉투의 `said` 에 묻혀 페이지에 안 선다. 못 여는 것(그런 이름, 오프셋 `+09:00`, tzdb 가 없는 기계의 이름)은 안 넘기고 서버의
/// 시간대로 잰다. 여는 것은 tzdb 의 파일 하나다 — 이름의 모양은 [`api::argv`] 가 이미 쟀다(`.`·절대 경로가 없다).
fn zone_for_child(query: &[(String, String)]) -> Option<&str> {
    api::zone_of(query).filter(|zone| crate::tz::Zone::load(zone).is_ok())
}

fn page(kind: &'static str, body: &'static str) -> Response {
    Response::new(200, kind, body.as_bytes().to_vec())
}

/// 자식을 돌려 봉투로 낸다. **자식이 돌았으면 200 이다** — 자식의 실패는 봉투의 `exit`·`err` 가 말한다.
/// 4xx·5xx 는 서버가 답을 못 낸 판이다.
pub fn child(site: &Site, args: &[String], cwd: &std::path::Path, zone: Option<&str>, lang: Lang) -> Response {
    child_as(site, args, cwd, zone, lang, None)
}

fn child_as(
    site: &Site,
    args: &[String],
    cwd: &std::path::Path,
    zone: Option<&str>,
    lang: Lang,
    actor: Option<&str>,
) -> Response {
    let run = if let Some(actor) = actor {
        let spawner = run::Spawner {
            exe: site.spawner.exe.clone(),
            cwd: site.spawner.cwd.clone(),
            user: Some(actor.to_string()),
        };
        spawner.run_in(args, cwd, zone)
    } else {
        site.spawner.run_in(args, cwd, zone)
    };
    match run {
        Ok(done) => Response::new(200, http::JSON, run::envelope(&done).into_bytes()),
        Err(run::Fault::Timeout) => {
            let secs = run::TIME_LIMIT.as_secs().to_string();
            refuse(504, &fill(say(lang, "web.child_timeout"), &[("secs", &secs)]), code::TIMEOUT)
        }
        Err(run::Fault::TooBig) => {
            let max = (run::OUT_MAX / (1024 * 1024)).to_string();
            refuse(502, &fill(say(lang, "web.child_too_big"), &[("max", &max)]), code::TOO_LARGE)
        }
        Err(run::Fault::Killed) => refuse(502, say(lang, "web.child_killed"), code::ERROR),
        Err(run::Fault::Spawn(e)) => {
            refuse(500, &fill(say(lang, "web.child_spawn"), &[("said", &e.to_string())]), code::ERROR)
        }
    }
}

/// 서버의 거절 — `{"code":"…","error":"…"}`. `moai --json` 이 넘어질 때(`main::fail`)와 같은 꼴, 같은 차례다.
pub fn refuse(status: u16, said: &str, code: &str) -> Response {
    let body = serde_json::json!({ "error": said, "code": code }).to_string();
    Response::new(status, http::JSON, body.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn only_loopback_event_peers_can_join_ctl() {
        assert!(local_ctl_peer("127.0.0.1:6624".parse().unwrap()));
        assert!(local_ctl_peer("[::1]:6624".parse().unwrap()));
        assert!(!local_ctl_peer("192.0.2.3:6624".parse().unwrap()));
        assert!(!local_ctl_peer("[2001:db8::3]:6624".parse().unwrap()));

        // A stream admitted for reading from elsewhere receives no ctl hello and never enters the tab registry.
        let (client, server) = pair();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let tabs = Arc::new(ctl::Tabs::new());
        let of = Arc::clone(&tabs);
        let scratch = crate::scratch::Scratch::new("remote-event-no-ctl");
        let cwd = scratch.path().to_path_buf();
        let worker =
            std::thread::spawn(move || stream_events(&server, &cwd, Lang::En, false, "stamp".into(), &of, false, None));
        let mut reader = std::io::BufReader::new(&client);
        let mut lines = String::new();
        loop {
            let mut line = String::new();
            std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
            assert!(!line.is_empty(), "stream ended before its stamp");
            lines.push_str(&line);
            if line.starts_with("event: stamp") {
                break;
            }
        }
        assert!(!lines.contains("event: hello"), "a remote page received a ctl token");
        let listed = tabs.handle(crate::ctl::wire::request(crate::ctl::wire::Verb::Hello, Duration::from_secs(1)));
        assert!(listed.data.unwrap()["windows"].as_array().unwrap().is_empty(), "a remote page became a ctl target");
        drop(reader);
        drop(client);
        worker.join().unwrap().unwrap();
    }

    /// 루프백에 짝 하나 — (부르는 쪽, 받은 쪽).
    fn pair() -> (TcpStream, TcpStream) {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let client = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (server, _) = l.accept().unwrap();
        (client, server)
    }

    /// **겹쳐 보기는 git 체크아웃 안에서만 붙는다** — 밖에서는 옆이 있을 수 없는데 부를 때마다 "못 찾았다" 를 말했다.
    /// 스크래치는 체크아웃 밖이다(`crate::scratch::base`).
    #[test]
    fn the_overlay_is_asked_only_inside_a_git_checkout() {
        let s = crate::scratch::Scratch::new("web-overlay-git");
        let args = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        let asked = args(&["show", "--all", "--worktree", "--fields=id"]);
        assert_eq!(overlay_in_git(asked.clone(), s.path()), args(&["show", "--all", "--fields=id"]));
        // `--` 뒤는 값이다 — `--worktree` 라는 낱말을 찾는 위키 검색의 낱말을 안 뗀다(리뷰 moai-xp7l.mug).
        let words = args(&["wiki", "search", "-n", "2000", "--", "--worktree"]);
        assert_eq!(overlay_in_git(words.clone(), s.path()), words);
        let one = args(&["show", "--worktree", "--", "moai-1abc"]);
        assert_eq!(overlay_in_git(one, s.path()), args(&["show", "--", "moai-1abc"]));
        std::fs::create_dir_all(s.path().join(".git")).unwrap();
        std::fs::create_dir_all(s.path().join("sub")).unwrap();
        assert_eq!(overlay_in_git(asked.clone(), &s.path().join("sub")), asked, "체크아웃 밑의 자리");
    }

    /// **405 의 `Allow` 는 그 길이 받는 메서드다**(moai-m4ez.4zq) — 쓰는 길(읽음 표시·계획 날)은 `POST`, 나머지는 `GET`. 자식을 안 띄우는
    /// 답이라 없는 바이너리를 준다.
    #[test]
    fn a_405_allows_post_only_on_the_write_paths() {
        let s = crate::scratch::Scratch::new("web-allow");
        let site = Site::new(
            run::Spawner { exe: "/nonexistent/moai".into(), cwd: s.path().to_path_buf(), user: None },
            Lang::En,
        );
        let ask = |method: &str, path: &str| {
            let req = Request {
                method: method.into(),
                path: path.into(),
                query: Vec::new(),
                host: Some("127.0.0.1:1".into()),
                origin: None,
                content_length: 0,
                content_type: None,
                body_prefix: Vec::new(),
            };
            answer(&req, &site)
        };
        for path in ["/api/read-all", "/api/read/moai-1abc", "/api/read-group/moai-1abc", "/api/plan/moai-1abc"] {
            let got = ask("DELETE", path);
            assert_eq!((got.status, got.allow), (405, "POST"), "{path}");
        }
        for path in ["/api/rows", "/api/stamp", "/nope"] {
            let got = ask("POST", path);
            assert_eq!((got.status, got.allow), (405, "GET"), "{path}");
        }
    }

    /// **한 바이트씩 흘려도 머리 전체의 시한에서 끊는다** — `read` 한 번의 시한이던 판은 흘리는 동안 끝없이 쥐었다.
    #[test]
    fn a_trickled_head_is_cut_at_the_whole_limit() {
        let (mut client, server) = pair();
        let feeder = std::thread::spawn(move || {
            for b in b"GET / HTTP/1.1\r\nX-Pad: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
                if client.write_all(&[*b]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let started = Instant::now();
        let got = read_head(&server, Duration::from_millis(300));
        assert!(matches!(got, Err(Bad::Io)), "{got:?}");
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
        drop(server);
        let _ = feeder.join();
    }

    /// **안 읽는 상대에게 큰 답을 써도 답 전체의 시한에서 끊는다** — 쓰기 한 번의 시한이던 판은 조금씩이라도
    /// 나아가는 동안 끝없이 썼다.
    #[test]
    fn a_reader_that_does_not_keep_up_is_cut_at_the_whole_limit() {
        let (client, server) = pair();
        let big = Response::new(200, http::JSON, vec![b' '; 32 * 1024 * 1024]);
        let started = Instant::now();
        let got = http::write(&mut Within::of(&server, Duration::from_millis(300)), &big, true);
        assert!(got.is_err(), "안 읽는 상대에게 32 MiB 를 다 썼다");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
        drop(client);
    }

    /// **안 읽은 요청을 둔 채 답하고 닫아도 상대는 답을 끝까지 받는다** — RST 가 아니라 FIN 이다.
    #[test]
    fn closing_after_an_unread_request_is_not_a_reset() {
        for _ in 0..20 {
            let (mut client, server) = pair();
            client.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            // 요청이 받은 쪽에 닿을 틈 — 닿기 전에 닫으면 RST 를 부르는 안 읽은 바이트가 없다.
            std::thread::sleep(Duration::from_millis(5));
            turn_away(server, Lang::En, &Arc::new(AtomicUsize::new(0)));
            let mut got = Vec::new();
            client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            client.read_to_end(&mut got).expect("답 대신 RST 를 받았다");
            assert!(got.starts_with(b"HTTP/1.1 503 "), "{}", String::from_utf8_lossy(&got));
        }
    }

    /// 요청을 기다리는 drainer 보다 먼저 503 전체와 FIN 이 간다. 상대가 닫으면 긴 시한도 기다리지 않는다.
    #[test]
    fn a_busy_response_ends_before_the_request_drain() {
        let (mut client, server) = pair();
        let draining = Arc::new(AtomicUsize::new(0));
        // FIN 을 빼면 30 초 drain 이 끝나기 전에 5 초 읽기 시한에서 실패한다.
        turn_away_within(server, Lang::En, &draining, Duration::from_secs(30));
        client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut got = Vec::new();
        client.read_to_end(&mut got).expect("503 과 EOF 는 요청 drain 을 기다리지 않는다");
        let mut expected = Vec::new();
        http::write(&mut expected, &refuse(503, say(Lang::En, "web.busy"), code::BUSY), true).unwrap();
        assert_eq!(got, expected, "503 의 머리와 본문을 끝까지 받는다");
        assert_eq!(draining.load(Ordering::SeqCst), 1, "조용한 상대의 요청은 아직 기다린다");
        drop(client);
        let until = Instant::now() + Duration::from_secs(5);
        while draining.load(Ordering::SeqCst) != 0 {
            assert!(Instant::now() < until, "닫힌 상대가 drainer 의 자리를 안 놓았다");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 503 을 받은 뒤 요청을 안 보내는 상대도 drainer 의 자리를 끝없이 붙잡지 못한다.
    #[test]
    fn a_silent_overflow_client_is_only_waited_for_briefly() {
        let (_client, server) = pair();
        let draining = Arc::new(AtomicUsize::new(0));
        turn_away(server, Lang::En, &draining);
        let until = Instant::now() + Duration::from_secs(2);
        while draining.load(Ordering::SeqCst) != 0 {
            assert!(Instant::now() < until, "조용한 상대가 drainer 의 자리를 안 놓았다");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 비우는 자리도 다 찼으면 대기하거나 스레드를 더 띄우지 않고 FIN 과 함께 닫는다.
    #[test]
    fn overflow_past_the_drainer_cap_is_closed_without_waiting() {
        let (mut client, server) = pair();
        let draining = Arc::new(AtomicUsize::new(BUSY_DRAIN_MAX));
        turn_away(server, Lang::En, &draining);
        assert_eq!(draining.load(Ordering::SeqCst), BUSY_DRAIN_MAX);
        client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut got = Vec::new();
        client.read_to_end(&mut got).unwrap();
        assert!(got.starts_with(b"HTTP/1.1 503 "), "{}", String::from_utf8_lossy(&got));
    }
}
