//! HTTP/1.1 의 요청 한 개를 읽고 응답 한 개를 쓴다 — `moai web` 이 쓰는 만큼만.
//!
//! **연결 하나에 요청 하나다**(`Connection: close`). keep-alive 를 안 하므로 요청 머리 끝(`\r\n\r\n`)
//! 까지만 읽고, 본문은 받지 않는다 — 이 서버는 읽기 전용이라 본문을 실은 요청은 뜻이 없다(400).
//! 그래서 `Transfer-Encoding` 을 풀 일도, 다음 요청과의 경계를 잴 일도 없다.

use std::io::{Read, Write};

/// 요청 머리의 상한 — 넘으면 400 이다. 브라우저의 머리는 쿠키를 다 실어도 몇 KiB 다.
pub const HEAD_MAX: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// 퍼센트를 푼 경로 — `/api/issue/moai-1abc`.
    pub path: String,
    /// 퍼센트를 푼 쿼리 — **적힌 차례 그대로, 같은 이름이 둘이어도 둘 다** 든다. 두 번 준 이름을
    /// 거절하는 것은 받는 쪽(끝점)의 일이다 — 여기서 하나로 접으면 그것을 못 본다.
    pub query: Vec<(String, String)>,
    /// `Host` 머리 — 없으면 `None`. 검사는 [`host_ok`] 가 한다.
    pub host: Option<String>,
    /// `Origin` 머리 — 없으면 `None`. 쓰는 길 하나(`POST /api/read…`)만 잰다([`origin_ok`]).
    pub origin: Option<String>,
}

/// 읽다 만난 탈. [`Bad::Io`] 는 답할 상대가 없는 것이라(시한·끊김) 그냥 닫는다 — 나머지는 400 이다.
#[derive(Debug)]
pub enum Bad {
    Io,
    TooBig,
    Malformed,
    /// 본문을 실었다 — `Content-Length` 가 0 이 아니거나 `Transfer-Encoding` 이 섰다.
    Body,
    /// 퍼센트를 푼 경로나 쿼리가 UTF-8 이 아니다.
    Encoding,
}

/// 머리 끝까지 읽는다. 끝에 닿기 전에 [`HEAD_MAX`] 를 넘으면 [`Bad::TooBig`].
pub fn read(r: &mut impl Read) -> Result<Request, Bad> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let end = loop {
        // 앞에서 본 자리는 다시 안 훑는다 — 끝 표식이 조각 경계에 걸쳐도 잡히게 세 바이트 물린다.
        let from = buf.len().saturating_sub(3);
        let n = match r.read(&mut chunk) {
            Ok(n) => n,
            // **신호 하나에 요청을 버리지 않는다** — 읽기 시한을 건 소켓은 서버가 멈췄다 이어 돌면(`Ctrl-Z`·`fg`)
            // 처리기가 없어도 `EINTR` 을 낸다. 끊긴 상대로 읽으면 반쯤 온 머리가 답 없이 닫혔다(`git::stream_log`
            // 이 같은 덫을 적어 두었다).
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Bad::Io),
        };
        if n == 0 {
            return Err(Bad::Malformed);
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf[from..].windows(4).position(|w| w == b"\r\n\r\n") {
            break from + at;
        }
        if buf.len() > HEAD_MAX {
            return Err(Bad::TooBig);
        }
    };
    if end > HEAD_MAX {
        return Err(Bad::TooBig);
    }
    parse(&buf[..end])
}

fn parse(head: &[u8]) -> Result<Request, Bad> {
    let head = std::str::from_utf8(head).map_err(|_| Bad::Malformed)?;
    let mut lines = head.split("\r\n");
    let first = lines.next().ok_or(Bad::Malformed)?;
    let mut parts = first.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Bad::Malformed);
    };
    if method.is_empty() || !version.starts_with("HTTP/1.") {
        return Err(Bad::Malformed);
    }
    let (mut host, mut origin) = (None, None);
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(Bad::Malformed)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("host") {
            // 둘이면 어느 쪽을 잴지 모른다 — 하나를 고르면 검사를 지나는 쪽을 고르게 된다.
            if host.replace(value.to_string()).is_some() {
                return Err(Bad::Malformed);
            }
        } else if name.eq_ignore_ascii_case("origin") {
            // `Host` 와 같은 까닭이다.
            if origin.replace(value.to_string()).is_some() {
                return Err(Bad::Malformed);
            }
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            || (name.eq_ignore_ascii_case("content-length") && value != "0")
        {
            return Err(Bad::Body);
        }
    }
    // 절대 꼴(`http://…/`)과 `*` 는 프록시에 보내는 꼴이다 — 이 서버는 프록시가 아니다.
    if !target.starts_with('/') {
        return Err(Bad::Malformed);
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = unescape(path, false)?;
    let query = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            Ok((unescape(k, true)?, unescape(v, true)?))
        })
        .collect::<Result<_, Bad>>()?;
    Ok(Request { method: method.to_string(), path, query, host, origin })
}

/// 쓰는 요청이 **이 페이지에서 왔는가** — `Origin` 이 `http://<Host>` 와 글자째 같아야 한다.
///
/// `Host` 검사([`host_ok`])는 DNS 다시 묶기를 막을 뿐이다. 남의 사이트가 띄운 `fetch(…, {method: "POST", mode:
/// "no-cors"})` 는 본문 없는 단순 요청이라 미리 묻기(preflight) 없이 `127.0.0.1` 로 곧장 오고, 그 `Host` 는 이
/// 서버의 것이다. 브라우저는 POST 에 `Origin` 을 늘 싣고 페이지는 그것을 못 고치므로, 그 값이 이 서버의 자리여야
/// 이 페이지가 보낸 것이다. **없으면 거절한다** — `curl` 은 `Origin` 없이 오지만 쓰는 길은 CLI 에 이미 있다.
pub fn origin_ok(origin: Option<&str>, host: Option<&str>) -> bool {
    match (origin, host) {
        (Some(origin), Some(host)) => origin.strip_prefix("http://") == Some(host),
        _ => false,
    }
}

/// `%XX` 를 푼다. 쿼리에서는 `+` 도 빈칸이다(폼 꼴 — 브라우저의 `URLSearchParams` 가 그렇게 쓴다).
/// 푼 바이트가 UTF-8 이 아니면 [`Bad::Encoding`], `%` 뒤가 16진 두 자가 아니면 [`Bad::Malformed`].
fn unescape(s: &str, plus: bool) -> Result<String, Bad> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let hex = b.get(i + 1..i + 3).ok_or(Bad::Malformed)?;
                // **두 자가 다 16진 숫자여야 한다** — `u8::from_str_radix` 는 앞의 `+` 를 부호로 받아 `%+A` 를 줄바꿈으로
                // 풀었다.
                if !hex.iter().all(u8::is_ascii_hexdigit) {
                    return Err(Bad::Malformed);
                }
                let hex = std::str::from_utf8(hex).map_err(|_| Bad::Malformed)?;
                out.push(u8::from_str_radix(hex, 16).map_err(|_| Bad::Malformed)?);
                i += 3;
            }
            b'+' if plus => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| Bad::Encoding)
}

/// `Host` 를 받는가 — **포트를 떼고 `localhost`(대소문자 무시)이거나 IP 글자일 때만**(사람 결정).
///
/// 이 서버는 `127.0.0.1` 에 묶이지만 묶는 자리만으로는 DNS 리바인딩을 못 막는다 — 남의 페이지가 제 이름을
/// `127.0.0.1` 로 돌려 놓으면 브라우저는 그 이름을 `Host` 에 실어 여기로 온다. 이름이 IP 글자나 `localhost`
/// 가 아니면 그런 부름이라 거절한다. 다른 이름을 열 길(`--allow-host`)은 두지 않았다.
pub fn host_ok(host: Option<&str>) -> bool {
    let Some(host) = host else { return false };
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        let Some((inside, after)) = rest.split_once(']') else { return false };
        if inside.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        let port = match after {
            "" => None,
            after => match after.strip_prefix(':') {
                Some(p) => Some(p),
                None => return false,
            },
        };
        (None, port)
    } else {
        match host.split_once(':') {
            Some((n, p)) => (Some(n), Some(p)),
            None => (Some(host), None),
        }
    };
    if port.is_some_and(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit())) {
        return false;
    }
    match name {
        // `[v6]` 는 위에서 이미 쟀다.
        None => true,
        Some(n) => n.eq_ignore_ascii_case("localhost") || n.parse::<std::net::Ipv4Addr>().is_ok(),
    }
}

pub struct Response {
    pub status: u16,
    pub kind: &'static str,
    pub body: Vec<u8>,
    /// 405 의 `Allow` 머리 — 그 길이 받는 메서드. 405 가 아니면 안 쓴다. 쓰는 길(읽음 표시·계획 날)은 `POST` 만 받으므로 모든 405 에
    /// `GET` 을 적던 판은 `POST` 를 받는 길에 거짓을 말했다(moai-m4ez.4zq).
    pub allow: &'static str,
}

impl Response {
    pub fn new(status: u16, kind: &'static str, body: Vec<u8>) -> Response {
        Response { status, kind, body, allow: "GET" }
    }
}

pub const JSON: &str = "application/json";
pub const HTML: &str = "text/html; charset=utf-8";
pub const JS: &str = "text/javascript; charset=utf-8";
pub const CSS: &str = "text/css; charset=utf-8";

/// 페이지가 부를 수 있는 것을 제 자리(`'self'`)로만 좁힌다. **style 도 `'unsafe-inline'` 없이 닫혔다**(moai-m4ez.h2o) —
/// 페이지는 `style` 속성도 런타임 `<style>` 요소도 안 쓴다. 값은 클래스(`web/panel/style/*.css`)로, 계산한 값은
/// CSSOM(`el.style.x = …`·`style.setProperty`·구성 가능한 시트)으로만 넣는다 — 그 길은 CSP 가 막지 않는다.
/// `tests/cli.rs` 의 `web_serves_the_page_and_the_boot` 가 이 글자를 그대로 쥔다.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
                   img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
    }
}

/// 답 하나를 쓴다. `body` 가 거짓이면 머리만 쓴다 — **HEAD 의 답은 본문을 안 싣는다**(RFC 9110 §9.3.2). 실으면
/// HTTP 를 지키는 쪽은 머리 뒤의 바이트를 넘친 것으로 본다(curl 의 `Excess found`). 길이(`Content-Length`)는 본문이
/// 섰을 때의 것 그대로다.
pub fn write(w: &mut impl Write, r: &Response, body: bool) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {CSP}\r\n",
        r.status,
        reason(r.status),
        r.kind,
        r.body.len()
    );
    if r.status == 405 {
        head.push_str(&format!("Allow: {}\r\n", r.allow));
    }
    head.push_str("\r\n");
    w.write_all(head.as_bytes())?;
    if body {
        w.write_all(&r.body)?;
    }
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(raw: &str) -> Result<Request, Bad> {
        read(&mut raw.as_bytes())
    }

    #[test]
    fn a_get_is_read_with_its_path_query_and_host() {
        let r = req(
            "GET /api/rows?q=%ED%8C%8C&filter=status%3Dtodo+tag%3Dbug&q=x HTTP/1.1\r\nHost: localhost:6624\r\n\r\n",
        )
        .unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/api/rows");
        assert_eq!(
            r.query,
            vec![
                ("q".to_string(), "파".to_string()),
                ("filter".to_string(), "status=todo tag=bug".to_string()),
                ("q".to_string(), "x".to_string())
            ],
            "같은 이름 둘을 접지 않는다"
        );
        assert_eq!(r.host.as_deref(), Some("localhost:6624"));
    }

    #[test]
    fn a_path_is_unescaped_and_must_be_utf8() {
        assert_eq!(req("GET /%ED%8C%8C HTTP/1.1\r\n\r\n").unwrap().path, "/파");
        assert!(matches!(req("GET /%FF HTTP/1.1\r\n\r\n"), Err(Bad::Encoding)));
        assert!(matches!(req("GET /%Z1 HTTP/1.1\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET /% HTTP/1.1\r\n\r\n"), Err(Bad::Malformed)));
        // 부호는 16진 숫자가 아니다 — `from_str_radix` 는 `+A` 를 10 으로 읽는다.
        assert!(matches!(req("GET /a%+Ab HTTP/1.1\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET /?q=%+0 HTTP/1.1\r\n\r\n"), Err(Bad::Malformed)));
        // 경로의 `+` 는 글자다.
        assert_eq!(req("GET /a+b HTTP/1.1\r\n\r\n").unwrap().path, "/a+b");
    }

    /// **`EINTR` 은 끊김이 아니다** — 한 번 물러나도 이어 읽어 머리를 다 받는다.
    #[test]
    fn an_interrupted_read_is_read_again() {
        struct Flaky<'a> {
            rest: &'a [u8],
            tripped: bool,
        }
        impl Read for Flaky<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if !self.tripped {
                    self.tripped = true;
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                self.rest.read(buf)
            }
        }
        let mut r = Flaky { rest: b"GET /x HTTP/1.1\r\nHost: localhost\r\n\r\n", tripped: false };
        assert_eq!(read(&mut r).unwrap().path, "/x");
    }

    #[test]
    fn a_head_past_the_limit_is_refused() {
        let big = format!("GET / HTTP/1.1\r\nX-Pad: {}\r\n\r\n", "a".repeat(HEAD_MAX));
        assert!(matches!(req(&big), Err(Bad::TooBig)));
        // 끝 표식이 없는 채로 상한을 넘어도 기다리지 않는다.
        let endless = format!("GET / HTTP/1.1\r\nX-Pad: {}", "a".repeat(HEAD_MAX + 10));
        assert!(matches!(req(&endless), Err(Bad::TooBig)));
        // 상한 바로 안은 받는다.
        let fits = format!("GET / HTTP/1.1\r\nX-Pad: {}\r\n\r\n", "a".repeat(HEAD_MAX - 40));
        assert!(req(&fits).is_ok());
    }

    #[test]
    fn a_body_a_proxy_form_and_a_broken_line_are_refused() {
        assert!(matches!(req("POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc"), Err(Bad::Body)));
        assert!(matches!(req("GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"), Err(Bad::Body)));
        assert!(req("GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n").is_ok());
        assert!(matches!(req("GET http://evil/ HTTP/1.1\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET /\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET / HTTP/1.1\r\nno colon\r\n\r\n"), Err(Bad::Malformed)));
        assert!(matches!(req("GET / HTTP/1.1\r\n"), Err(Bad::Malformed)), "끝 전에 끊겼다");
    }

    #[test]
    fn only_localhost_and_ip_literals_pass_the_host_check() {
        for ok in ["localhost:6624", "LocalHost", "127.0.0.1", "[::1]:1", "[::1]", "192.168.0.3:6624"] {
            assert!(host_ok(Some(ok)), "{ok}");
        }
        for bad in [
            "evil.example",
            "localhost.evil.example",
            "localhost.",
            "127.0.0.1.nip.io",
            "localhost:",
            "localhost:x",
            "[::1]x",
            "[nope]:1",
            "::1",
            "",
        ] {
            assert!(!host_ok(Some(bad)), "{bad}");
        }
        assert!(!host_ok(None), "없는 Host");
    }

    /// **`Allow` 는 응답이 든 메서드를 그대로 쓴다**(moai-m4ez.4zq) — 쓰는 길의 405 는 `POST` 다.
    #[test]
    fn a_405_names_the_methods_the_response_carries() {
        let mut out = Vec::new();
        let mut post = Response::new(405, JSON, b"{}".to_vec());
        post.allow = "POST";
        write(&mut out, &post, true).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("Allow: POST\r\n") && !out.contains("Allow: GET"), "{out}");
        // 405 가 아닌 답에는 `Allow` 가 없다.
        let mut ok = Vec::new();
        write(&mut ok, &Response::new(200, JSON, b"{}".to_vec()), true).unwrap();
        assert!(!String::from_utf8(ok).unwrap().contains("Allow:"));
    }

    #[test]
    fn a_response_carries_the_fixed_headers() {
        let mut out = Vec::new();
        write(&mut out, &Response::new(405, JSON, b"{}".to_vec()), true).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"), "{out}");
        for h in [
            "Content-Length: 2\r\n",
            "Connection: close\r\n",
            "Cache-Control: no-store\r\n",
            "X-Content-Type-Options: nosniff\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "Content-Security-Policy: default-src 'none';",
            "Allow: GET\r\n",
        ] {
            assert!(out.contains(h), "{h}\n{out}");
        }
        assert!(out.ends_with("\r\n\r\n{}"), "{out}");
        // HEAD 의 답 — 같은 머리(길이까지)에 본문만 없다.
        let mut head = Vec::new();
        write(&mut head, &Response::new(405, JSON, b"{}".to_vec()), false).unwrap();
        let head = String::from_utf8(head).unwrap();
        assert!(head.contains("Content-Length: 2\r\n") && head.ends_with("\r\n\r\n"), "{head}");
    }
}
