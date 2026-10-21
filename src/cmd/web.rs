//! `moai web` — 묶고, 첫 줄을 찍고, 받기 고리로 간다(`crate::web`).
//!
//! **첫 줄은 여기서 찍고 흘려보낸다.** `main` 은 `cmd::run` 이 돌아온 뒤에 찍는데 섬기는 판은 안 돌아온다 —
//! 거기 맡기면 서버를 띄운 쪽(사람이든 시험이든)이 어디로 붙을지를 영영 못 듣는다.

use super::{Ctx, Fail, R, code};
use crate::cli::WebArgs;
use crate::i18n::{fill, say};
use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(serde::Serialize)]
struct Where {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    host: String,
    port: u16,
    root: String,
    here: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    dry_run: bool,
}

/// 브라우저에 칠 주소. **묶은 자리가 "아무 데나"(`0.0.0.0`·`::`)면 루프백으로 댄다** — 그 글자는 붙을 곳이
/// 아니다. 포트를 모르면(`--dry-run --port 0`) 없다.
fn url(host: IpAddr, port: u16) -> Option<String> {
    let shown = match host {
        IpAddr::V4(a) if a.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(a) if a.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        a => a,
    };
    (port != 0).then(|| format!("http://{}/", SocketAddr::new(shown, port)))
}

pub fn run(ctx: &Ctx, a: WebArgs) -> R<Vec<String>> {
    let lang = ctx.lang();
    let repo = super::open_repo(ctx)?;
    let cwd = std::env::current_dir().map_err(|e| Fail::new(e.to_string()))?;
    let host = a.host.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let root = repo.root.display().to_string();
    let here = repo.here().display().to_string();
    if a.dry_run {
        warn(lang, &cwd, host);
        let at = Where { url: url(host, a.port), host: host.to_string(), port: a.port, root, here, dry_run: true };
        return match ctx.json {
            true => super::json_line(&at),
            false => Ok(said(lang, &at, host)),
        };
    }
    // 띄울 바이너리는 묶기 전에 푼다 — 첫 줄을 낸 뒤에 넘어지면 띄운 쪽은 붙을 곳을 들은 채 죽은 서버를 본다.
    let exe = crate::web::run::Spawner::exe_of()
        .map_err(|e| Fail::new(fill(say(lang, "web.no_exe"), &[("said", &e.to_string())])))?;
    let listener = std::net::TcpListener::bind((host, a.port)).map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => {
            Fail::coded(fill(say(lang, "web.port_in_use"), &[("port", &a.port.to_string())]), code::PORT_IN_USE)
        }
        _ => Fail::new(fill(say(lang, "web.bind_failed"), &[("said", &e.to_string())])),
    })?;
    let port = listener.local_addr().map_err(|e| Fail::new(e.to_string()))?.port();
    // **경고는 묶은 뒤에 낸다** — 그 앞에서 넘어진 `--json` 의 stderr 는 오류 객체 하나여야 한다(`main` 의 실패 길).
    // 사람 말 한 줄이 그 앞에 서면 `code` 로 가르던 쪽이 파싱 실패를 만난다. 올라가 잡은 줄도 그래서 여기서 꺼낸다 —
    // 넘어진 판에서는 `main` 이 제 자리에서 댄다.
    warn(lang, &cwd, host);
    let at = Where { url: url(host, port), host: host.to_string(), port, root, here, dry_run: false };
    let first = match ctx.json {
        true => serde_json::to_string(&at).map_err(|e| Fail::new(e.to_string()))?,
        false => said(lang, &at, host).join("\n"),
    };
    // **첫 줄을 못 냈으면 섬기지 않는다** — 띄운 쪽이 어디로 붙을지를 영영 못 듣는다. 받는 쪽이 닫힌 것(EPIPE)은
    // `main` 의 `outln!` 처럼 조용히 그치고, 그 밖의 실패(디스크가 찼다 등)는 그 까닭으로 넘어진다.
    let wrote = {
        let mut out = anstream::stdout().lock();
        writeln!(out, "{first}").and_then(|()| out.flush())
    };
    match wrote {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => return Ok(Vec::new()),
        Err(e) => return Err(Fail::new(fill(say(lang, "warn.stdout_lost"), &[("why", &e.to_string())]))),
    }
    // **자식은 서버가 잡은 트래커의 자리에서 돈다**(`repo.here()` — 친 체크아웃의 그 자리) — 친 자리에서 띄우면 자식이
    // 저마다 다시 올라가 잡고, 서버가 위에서 한 번 댄 그 줄을 봉투마다 `said` 로 또 실었다. 딸린 워크트리는 그대로
    // 그 워크트리라 커밋 표(`HEAD`)도 같다.
    let spawner = crate::web::run::Spawner { exe, cwd: repo.here().to_path_buf(), user: ctx.user.clone() };
    crate::web::serve(listener, crate::web::Site::new(spawner, lang))
}

/// 올라가 잡은 트래커와 루프백 밖의 자리를 stderr 에 댄다. **올라가 잡은 것은 여기서 꺼내 댄다**(`cmd::prime` 의
/// 꼴) — 섬기는 판은 `main` 의 끝 알림까지 안 간다.
fn warn(lang: crate::i18n::Lang, cwd: &std::path::Path, host: IpAddr) {
    if let Some(to) = crate::store::take_climb(cwd) {
        super::tell(&format!(
            "moai: {}",
            fill(
                say(lang, "warn.tracker_climbed"),
                &[("from", &cwd.display().to_string()), ("to", &to.display().to_string())]
            )
        ));
    }
    if !host.is_loopback() {
        super::tell(&format!("moai: {}", fill(say(lang, "web.not_loopback"), &[("host", &host.to_string())])));
    }
}

/// 사람에게 대는 한 줄 — 어디서, 어느 트래커를, 어떻게 끝내나. 포트를 몰라 `url` 이 없으면 묶을 자리를 댄다
/// (`SocketAddr` 꼴 — v6 는 `[::1]:0` 처럼 괄호에 든다).
fn said(lang: crate::i18n::Lang, at: &Where, host: IpAddr) -> Vec<String> {
    let url = at.url.clone().unwrap_or_else(|| SocketAddr::new(host, at.port).to_string());
    let first = match at.dry_run {
        true => fill(say(lang, "web.would_serve"), &[("url", &url), ("root", &at.root)]),
        false => fill(say(lang, "web.serving"), &[("url", &url), ("root", &at.root)]),
    };
    let mut lines = vec![first];
    if at.here != at.root {
        lines.push(fill(say(lang, "web.from_worktree"), &[("here", &at.here)]));
    }
    lines
}
