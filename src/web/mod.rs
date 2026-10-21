//! `moai web` — 127.0.0.1 에서 트래커를 페이지로 낸다. 받기 고리와 길.
//!
//! **연결마다 스레드 하나, 나누는 상태가 없다.** 요청마다 자식 `moai --json …` 하나를 띄워 그 답을 옮기므로
//! ([`run`]) 스레드끼리 함께 쥐는 것은 띄우는 법([`Site`])과 도는 연결의 수뿐이다. 비동기 런타임을 안 쓰는
//! 까닭이 이것이다 — 동시에 도는 것이 많아야 [`BUSY_MAX`] 이고, 한 판의 값은 자식 프로세스가 치른다.
//!
//! **읽기만 한다.** 길은 모두 GET 이고, 쓰는 길은 CLI 와 탐색기다(사람 결정, v1).

pub mod api;
pub mod boot;
pub mod http;
pub mod run;

use crate::fail::code;
use crate::i18n::{Lang, fill, say};
use http::{Bad, Request, Response};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 동시에 도는 연결의 상한 — 넘치면 503 을 쓰고 닫는다. 페이지 하나가 한 번에 여는 부름은 몇 개다.
pub const BUSY_MAX: usize = 32;
const READ_LIMIT: Duration = Duration::from_secs(5);
const WRITE_LIMIT: Duration = Duration::from_secs(10);

/// 서버를 띄울 때 한 번 정하고 모든 연결이 나눠 읽는 것.
pub struct Site {
    pub spawner: run::Spawner,
    pub lang: Lang,
}

/// 받기 고리 — **안 돌아온다.** Ctrl-C 가 끝이고, 치울 것이 없다(쓴 파일도 쥔 락도 없다).
pub fn serve(listener: TcpListener, site: Site) -> ! {
    let site = Arc::new(site);
    let busy = Arc::new(AtomicUsize::new(0));
    loop {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            // 받기 오류(손이 모자란 판의 EMFILE 따위)는 그 연결 하나의 것이다 — 서버를 안 내린다.
            Err(e) => {
                crate::cmd::tell(&fill(say(site.lang, "web.accept_failed"), &[("said", &e.to_string())]));
                // 같은 오류가 곧바로 다시 나면 고리가 CPU 를 다 먹는다.
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if busy.fetch_add(1, Ordering::SeqCst) >= BUSY_MAX {
            busy.fetch_sub(1, Ordering::SeqCst);
            // 받는 고리에서 쓰므로 짧게 기다린다 — 안 읽는 상대 하나가 받기를 세우면 안 된다.
            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
            let _ = http::write(&mut &stream, &refuse(503, say(site.lang, "web.busy"), code::BUSY));
            continue;
        }
        let (site, busy) = (site.clone(), busy.clone());
        std::thread::spawn(move || {
            // 스레드가 어떻게 끝나든(패닉도) 센 것을 돌려놓는다.
            struct Leave(Arc<AtomicUsize>);
            impl Drop for Leave {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _leave = Leave(busy);
            handle(stream, &site);
        });
    }
}

fn handle(stream: TcpStream, site: &Site) {
    let _ = stream.set_read_timeout(Some(READ_LIMIT));
    let _ = stream.set_write_timeout(Some(WRITE_LIMIT));
    let answer = match http::read(&mut &stream) {
        // 시한·끊김은 답할 상대가 없다.
        Err(Bad::Io) => return,
        Err(bad) => refuse(400, bad_said(&bad, site.lang), code::BAD_INPUT),
        Ok(req) => answer(&req, site),
    };
    let _ = http::write(&mut &stream, &answer);
}

fn bad_said(bad: &Bad, lang: Lang) -> &'static str {
    match bad {
        Bad::TooBig => say(lang, "web.head_too_big"),
        Bad::Body => say(lang, "web.body"),
        Bad::Encoding => say(lang, "web.encoding"),
        Bad::Malformed | Bad::Io => say(lang, "web.malformed"),
    }
}

/// 요청 하나의 답. **Host 가 먼저다** — 남의 이름으로 온 부름에는 어떤 길이 있는지도 안 알려 준다.
pub fn answer(req: &Request, site: &Site) -> Response {
    if !http::host_ok(req.host.as_deref()) {
        return refuse(403, say(site.lang, "web.foreign_host"), code::FORBIDDEN_HOST);
    }
    if req.method != "GET" {
        let said = fill(say(site.lang, "web.only_get"), &[("method", &req.method)]);
        return refuse(405, &said, code::METHOD_NOT_ALLOWED);
    }
    match req.path.as_str() {
        "/" | "/index.html" => page(http::HTML, include_str!("dist/index.html")),
        "/app.js" => page(http::JS, include_str!("dist/app.js")),
        "/app.css" => page(http::CSS, include_str!("dist/app.css")),
        "/api/boot" => match boot::boot(&site.spawner.cwd, site.lang) {
            Ok(body) => Response { status: 200, kind: http::JSON, body: body.into_bytes() },
            Err(e) => refuse(500, &e.message, e.code),
        },
        "/api/status" => child(site, &["status".to_string()]),
        "/api/stamp" => match api::stamp(&site.spawner.cwd, site.lang) {
            Ok(stamp) => {
                Response { status: 200, kind: http::JSON, body: format!("{{\"stamp\":\"{stamp}\"}}").into_bytes() }
            }
            Err(e) => refuse(500, &e.message, e.code),
        },
        path => match api::argv_of(path, &req.query) {
            Some(Ok(args)) => child(site, &args),
            Some(Err(bad)) => refuse(400, &bad.said(site.lang), code::BAD_INPUT),
            None => {
                let said = fill(say(site.lang, "web.no_path"), &[("path", &req.path)]);
                refuse(404, &said, code::NOT_FOUND)
            }
        },
    }
}

fn page(kind: &'static str, body: &'static str) -> Response {
    Response { status: 200, kind, body: body.as_bytes().to_vec() }
}

/// 자식을 돌려 봉투로 낸다. **자식이 돌았으면 200 이다** — 자식의 실패는 봉투의 `exit`·`err` 가 말한다.
/// 4xx·5xx 는 서버가 답을 못 낸 판이다.
pub fn child(site: &Site, args: &[String]) -> Response {
    match site.spawner.run(args) {
        Ok(done) => Response { status: 200, kind: http::JSON, body: run::envelope(&done).into_bytes() },
        Err(run::Fault::Timeout) => {
            let secs = run::TIME_LIMIT.as_secs().to_string();
            refuse(504, &fill(say(site.lang, "web.child_timeout"), &[("secs", &secs)]), code::TIMEOUT)
        }
        Err(run::Fault::TooBig) => refuse(502, say(site.lang, "web.child_too_big"), code::TOO_LARGE),
        Err(run::Fault::Killed) => refuse(502, say(site.lang, "web.child_killed"), code::ERROR),
        Err(run::Fault::Spawn(e)) => {
            refuse(500, &fill(say(site.lang, "web.child_spawn"), &[("said", &e.to_string())]), code::ERROR)
        }
    }
}

/// 서버의 거절 — `{"error":"…","code":"…"}`, `moai --json` 이 넘어질 때와 같은 꼴이다.
pub fn refuse(status: u16, said: &str, code: &str) -> Response {
    let body = serde_json::json!({ "error": said, "code": code }).to_string();
    Response { status, kind: http::JSON, body: body.into_bytes() }
}
