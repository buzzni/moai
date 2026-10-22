//! `moai web` — 127.0.0.1 에서 트래커를 페이지로 낸다. 받기 고리와 길.
//!
//! **연결마다 스레드 하나, 나누는 상태가 없다.** 요청마다 자식 `moai --json …` 하나를 띄워 그 답을 옮기므로
//! ([`run`]) 스레드끼리 함께 쥐는 것은 띄우는 법([`Site`])과 도는 연결의 수뿐이다. 비동기 런타임을 안 쓰는
//! 까닭이 이것이다 — 동시에 도는 것이 많아야 [`BUSY_MAX`] 이고, 한 판의 값은 자식 프로세스가 치른다.
//!
//! **트래커는 읽기만 한다.** 길은 GET 이고, 트래커를 바꾸는 길은 CLI 와 탐색기다(사람 결정, v1). 하나뿐인 POST 는
//! 읽음 표시다([`api::write_argv_of`], 2026-10-22 사람 결정, moai-ly88) — 읽음은 사용자 설정 곁의 제 파일에만
//! 적히고, 이 페이지에서 보낸 것만 받는다([`http::origin_ok`]).

pub mod api;
pub mod boot;
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
/// 요청 머리 **전체**를 읽는 시한 — `read` 한 번의 시한이 아니다([`read_head`]).
const READ_LIMIT: Duration = Duration::from_secs(5);
/// 쓰기 한 번이 나아가지 못하고 기다리는 시한.
const WRITE_LIMIT: Duration = Duration::from_secs(10);
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
        Site { spawner, started_in, picked: Mutex::new(Picked { stamp: None, lang: started_in, projects: Vec::new() }) }
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
            turn_away(&stream, site.started_in);
            continue;
        }
        // **센 것은 실을 띄우기 전에 쥔다** — `std::thread::spawn` 은 OS 가 실을 못 내면 이 고리에서 패닉해 서버가
        // 통째로 내려갔다(`git::finish_within` 이 `Builder` 로 띄우는 것과 같은 까닭). 못 띄우면 닫힘째 버려지며
        // 자리를 돌려놓고, 그 연결에는 남겨 둔 손잡이로 503 을 쓴다.
        let spare = stream.try_clone().ok();
        let leave = Leave(busy.clone());
        let site_of = site.clone();
        let spawned = std::thread::Builder::new().spawn(move || {
            let _leave = leave;
            handle(stream, &site_of);
        });
        if spawned.is_err()
            && let Some(spare) = spare
        {
            turn_away(&spare, site.started_in);
        }
    }
}

/// 연결 하나가 쥔 자리 — 스레드가 어떻게 끝나든(패닉도, 못 띄운 판도) 센 것을 돌려놓는다.
struct Leave(Arc<AtomicUsize>);

impl Drop for Leave {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 받을 자리가 없는 연결에 503 `busy` 를 쓰고 닫는다. 받는 고리에서 쓰므로 짧게 기다린다 — 안 읽는 상대 하나가
/// 받기를 세우면 안 된다.
fn turn_away(stream: &TcpStream, lang: Lang) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = http::write(&mut &*stream, &refuse(503, say(lang, "web.busy"), code::BUSY), true);
    close(stream);
}

fn handle(stream: TcpStream, site: &Site) {
    let (answer, body) = match read_head(&stream, READ_LIMIT) {
        // 시한·끊김은 답할 상대가 없다.
        Err(Bad::Io) => return,
        Err(bad) => (refuse(400, &bad_said(&bad, site.lang()), code::BAD_INPUT), true),
        // HEAD 의 답은 머리만 쓴다([`http::write`]).
        Ok(req) => (answer(&req, site), req.method != "HEAD"),
    };
    let _ = http::write(&mut Within::of(&stream, ANSWER_LIMIT), &answer, body);
    close(&stream);
}

/// 요청 머리를 `limit` 안에 **다** 읽는다([`Within`]).
fn read_head(stream: &TcpStream, limit: Duration) -> Result<Request, Bad> {
    http::read(&mut Within::of(stream, limit))
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
        s.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut s = self.stream;
        s.flush()
    }
}

/// 답을 쓴 뒤 닫는다. **안 읽은 바이트를 둔 채 닫으면 커널이 FIN 대신 RST 를 보낸다** — 머리를 안 읽은 503 과
/// 머리를 다 안 읽은 400 이 그 꼴이라, 상대는 방금 쓴 답 대신 "연결이 끊겼다" 를 받았다(503 을 스무 번 불러 열여덟
/// 번이 `ECONNRESET` 이었다). 쓰기를 먼저 닫아 FIN 을 답 바로 뒤에 붙이고, 이미 와 있는 것만 기다리지 않고 비운다.
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
        // **`Allow` 는 그 길이 받는 메서드다** — 읽음 표시 길은 `POST` 만 받는다(`GET` 은 404 라 여기 안 온다).
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
        return match api::write_argv_of(&req.path, &query) {
            Some(Ok(args)) => child(site, &overlay_in_git(args, &cwd), &cwd, lang),
            Some(Err(bad)) => refuse(400, &bad.said(lang), code::BAD_INPUT),
            None => not_allowed(),
        };
    }
    // 자식 없이 답하는 끝점도 받는 이름만 받는다(`/api/boot` 는 없고 `/api/stamp` 는 `worktree=1` 하나) — 말없이 버리면
    // 부른 쪽은 제가 준 것이 먹은 답이라 믿는다([`api::argv`]).
    let bare = || api::none_of(&query).map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT));
    match req.path.as_str() {
        "/api/boot" => match bare().and_then(|()| {
            boot::boot(&cwd, site.spawner.user.as_deref(), lang, &site.registered()).map_err(|e| refuse(500, &e.message, e.code))
        }) {
            Ok(body) => Response::new(200, http::JSON, body.into_bytes()),
            Err(refused) => refused,
        },
        "/api/stamp" => match api::overlay_of(&query)
            .map_err(|bad| refuse(400, &bad.said(lang), code::BAD_INPUT))
            .and_then(|on| api::stamp(&cwd, lang, on).map_err(|e| refuse(500, &e.message, e.code)))
        {
            Ok(stamp) => {
                Response::new(200, http::JSON, format!("{{\"stamp\":\"{stamp}\"}}").into_bytes())
            }
            Err(refused) => refused,
        },
        path => match api::argv_of(path, &query) {
            Some(Ok(args)) => child(site, &overlay_in_git(args, &cwd), &cwd, lang),
            Some(Err(bad)) => refuse(400, &bad.said(lang), code::BAD_INPUT),
            None => {
                let said = fill(say(lang, "web.no_path"), &[("path", &req.path)]);
                refuse(404, &said, code::NOT_FOUND)
            }
        },
    }
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
fn overlay_in_git(mut args: Vec<String>, cwd: &std::path::Path) -> Vec<String> {
    if !cwd.ancestors().any(|dir| dir.join(".git").exists()) {
        args.retain(|arg| arg != "--worktree");
    }
    args
}

fn page(kind: &'static str, body: &'static str) -> Response {
    Response::new(200, kind, body.as_bytes().to_vec())
}

/// 자식을 돌려 봉투로 낸다. **자식이 돌았으면 200 이다** — 자식의 실패는 봉투의 `exit`·`err` 가 말한다.
/// 4xx·5xx 는 서버가 답을 못 낸 판이다.
pub fn child(site: &Site, args: &[String], cwd: &std::path::Path, lang: Lang) -> Response {
    match site.spawner.run_in(args, cwd) {
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
        std::fs::create_dir_all(s.path().join(".git")).unwrap();
        std::fs::create_dir_all(s.path().join("sub")).unwrap();
        assert_eq!(overlay_in_git(asked.clone(), &s.path().join("sub")), asked, "체크아웃 밑의 자리");
    }

    /// **405 의 `Allow` 는 그 길이 받는 메서드다**(moai-m4ez.4zq) — 읽음 표시 길은 `POST`, 나머지는 `GET`. 자식을 안 띄우는
    /// 답이라 없는 바이너리를 준다.
    #[test]
    fn a_405_allows_post_only_on_the_read_mark_paths() {
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
            };
            answer(&req, &site)
        };
        for path in ["/api/read-all", "/api/read/moai-1abc", "/api/read-group/moai-1abc"] {
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
            turn_away(&server, Lang::En);
            drop(server);
            let mut got = Vec::new();
            client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            client.read_to_end(&mut got).expect("답 대신 RST 를 받았다");
            assert!(got.starts_with(b"HTTP/1.1 503 "), "{}", String::from_utf8_lossy(&got));
        }
    }
}
