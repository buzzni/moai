//! 데몬에 붙는 쪽 — 연결 하나에 요청 하나. `moai daemon start·stop·status` 가 떠 있는 데몬을 찾고 멈추는 데 쓴다.

use super::rpc::{self, RpcError};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// 붙어서 답을 기다리는 시한. 데몬은 `hello` 를 곧바로 답한다 — 이보다 늦으면 데몬이 아니거나 멈춘 것이다.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// `hello` 가 돌려준 데몬의 얼굴.
#[derive(Debug, Clone, PartialEq)]
pub struct Hello {
    /// 옛 판이나 남의 구현은 안 댈 수 있다.
    pub pid: Option<u32>,
    pub version: String,
    pub proto: u64,
}

impl Hello {
    /// 이 바이너리와 같은 판인가 — 판이 같아도 `proto` 가 다르면 다른 것으로 본다.
    pub fn same_as_this(&self) -> bool {
        self.version == env!("CARGO_PKG_VERSION") && self.proto == u64::from(rpc::PROTO)
    }

    fn of(value: &Value) -> Option<Hello> {
        Some(Hello {
            pid: value.get("pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok()),
            version: value.get("version")?.as_str()?.to_string(),
            proto: value.get("proto")?.as_u64()?,
        })
    }
}

/// 소켓 자리에 무엇이 서 있나.
#[derive(Debug)]
pub enum Probe {
    /// 아무 파일도 없다.
    Absent,
    /// 파일은 있는데 아무도 안 받는다 — 죽은 데몬이 남긴 소켓이거나 소켓이 아닌 파일이다. 지우기 전에 종류·주인을
    /// 보는 것은 부르는 쪽 몫이다.
    Dead,
    /// 데몬이 답했다. 이 바이너리가 말하는 `proto` 를 거절한 옛 데몬도 여기다 — 거절에 실린 판으로 선다.
    Running(Hello),
    /// 데몬이 연결 상한에 찼다고 답했다.
    Busy,
    /// 무언가 받았지만 데몬의 말로 답하지 않았다.
    Foreign(String),
}

pub fn probe(path: &Path) -> Probe {
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Probe::Absent,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => return Probe::Dead,
        Err(e) => return Probe::Foreign(e.to_string()),
    };
    match request(&mut stream, "hello", json!({ "proto": rpc::PROTO })) {
        Ok(Ok(result)) => {
            Hello::of(&result).map_or_else(|| Probe::Foreign("hello answered without a version".into()), Probe::Running)
        }
        Ok(Err(error)) => {
            let data = error.data.clone().unwrap_or(Value::Null);
            match data.get("code").and_then(Value::as_str) {
                Some("busy") => Probe::Busy,
                Some("unsupported_proto") => Hello::of(&data).map_or(Probe::Foreign(error.message), Probe::Running),
                _ => Probe::Foreign(error.message),
            }
        }
        Err(e) => Probe::Foreign(e.to_string()),
    }
}

/// 새 연결로 요청 하나를 보내고 답 하나를 받는다. 바깥 `Err` 는 전송의 실패, 안쪽은 데몬이 돌려준 오류다.
pub fn call(path: &Path, method: &str, params: Value) -> std::io::Result<Result<Value, RpcError>> {
    let mut stream = UnixStream::connect(path)?;
    request(&mut stream, method, params)
}

fn request(stream: &mut UnixStream, method: &str, params: Value) -> std::io::Result<Result<Value, RpcError>> {
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let line = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    writeln!(stream, "{line}")?;
    stream.flush()?;
    let mut reader = BufReader::new(&*stream);
    let reply = match rpc::read_line(&mut reader)? {
        rpc::Line::Text(text) => text,
        rpc::Line::Eof => return Err(bad("the daemon closed the connection without answering")),
        rpc::Line::TooLong => return Err(bad("the daemon's answer exceeds 1 MiB")),
    };
    let reply: Value = serde_json::from_slice(&reply).map_err(|e| bad(&format!("the answer is not JSON: {e}")))?;
    if let Some(error) = reply.get("error") {
        return Ok(Err(RpcError {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(rpc::code::INTERNAL),
            message: error.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
            data: error.get("data").cloned(),
        }));
    }
    match reply.get("result") {
        Some(result) => Ok(Ok(result.clone())),
        None => Err(bad("the answer carries neither result nor error")),
    }
}

fn bad(why: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, why.to_string())
}
