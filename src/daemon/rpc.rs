//! JSON-RPC 2.0 메시지 — 요청·응답·오류·메서드 표·줄 읽기.
//!
//! 정한 것:
//! - **판 확인은 `hello` 메서드다.** 모든 요청에 필드를 싣지 않는다 — 연결마다 한 번이면 되고,
//!   옛 클라이언트가 보내지 않는 필드를 요구하지 않는다. `hello` 를 안 불러도 다른 메서드는 돈다
//!   (상태를 두지 않는다 — HTTP 로 실어도 같다).
//! - **환경(`project`·`actor`·`lang`·`tz`)은 params 객체의 같은 이름 필드에서 뽑는다.** params 는 그대로
//!   처리 함수에 간다. 모르는 필드는 무시한다 — 새 클라이언트가 옛 데몬을 넘어뜨리지 않게.
//! - 알림(`id` 없음)은 받되 아무것도 돌리지 않고 답하지 않는다. 배치(배열)는 -32600.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::BufRead;

/// 데몬이 아는 메시지 판. 클라이언트가 더 큰 값을 말하면 `hello` 가 거절한다.
pub const PROTO: u32 = 1;
/// 줄 하나의 상한 — `ctl::wire::LINE_MAX` 와 같은 값.
pub const LINE_MAX: usize = 1024 * 1024;

/// 오류 코드. -32700..-32600 은 JSON-RPC 표준, -32000 대는 서버 정의 범위(-32099 까지)에서 moai 가 쓴다.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    /// moai 의 실패. CLI `--json` 오류 객체의 `code` 문자열이 `error.data.code` 에 그대로 실린다.
    pub const MOAI: i64 = -32000;
    /// 클라이언트의 `proto` 가 데몬이 아는 것보다 크다. `data.code` 는 `unsupported_proto`.
    pub const PROTO_TOO_NEW: i64 = -32001;
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), data: None }
    }
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_PARAMS, message)
    }
    /// moai 의 실패 — `cli_code` 는 `fail::code` 의 문자열(`not_found` 등)로, `data.code` 에 바뀌지 않고 간다.
    pub fn moai(cli_code: &str, message: impl Into<String>) -> Self {
        Self { code: code::MOAI, message: message.into(), data: Some(json!({ "code": cli_code })) }
    }
    fn to_value(&self) -> Value {
        let mut error = json!({ "code": self.code, "message": self.message });
        if let Some(data) = &self.data {
            error["data"] = data.clone();
        }
        error
    }
}

/// 요청마다 따라오는 환경. 전송이 아니라 메시지가 나른다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestEnv {
    pub project: Option<String>,
    pub actor: Option<String>,
    pub lang: Option<String>,
    pub tz: Option<String>,
}

impl RequestEnv {
    fn from_params(params: &Value) -> Self {
        let get = |key: &str| params.get(key).and_then(Value::as_str).map(str::to_string);
        Self { project: get("project"), actor: get("actor"), lang: get("lang"), tz: get("tz") }
    }
}

pub type Handler = Box<dyn Fn(&RequestEnv, Value) -> Result<Value, RpcError> + Send + Sync>;

/// 메서드 이름 → 처리 함수.
pub struct Methods {
    table: BTreeMap<String, Handler>,
}

impl Default for Methods {
    fn default() -> Self {
        Self::new()
    }
}

impl Methods {
    /// `hello` 와 `ping` 만 든 표.
    pub fn new() -> Self {
        let mut methods = Self { table: BTreeMap::new() };
        methods.register("hello", hello);
        methods.register("ping", |_, _| Ok(json!({})));
        methods
    }

    pub fn register(
        &mut self,
        name: &str,
        handler: impl Fn(&RequestEnv, Value) -> Result<Value, RpcError> + Send + Sync + 'static,
    ) {
        self.table.insert(name.to_string(), Box::new(handler));
    }

    /// 줄 하나(끝의 개행은 있어도 없어도 된다)를 처리해 답할 줄을 돌려준다. 알림이면 `None`.
    pub fn handle(&self, line: &[u8]) -> Option<String> {
        if line.len() > LINE_MAX {
            return Some(failure(&Value::Null, &RpcError::new(code::INVALID_REQUEST, "request line exceeds 1 MiB")));
        }
        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(e) => {
                return Some(failure(&Value::Null, &RpcError::new(code::PARSE_ERROR, format!("parse error: {e}"))));
            }
        };
        self.handle_value(value)
    }

    fn handle_value(&self, value: Value) -> Option<String> {
        let Value::Object(object) = value else {
            let why = if value.is_array() { "batch requests are not supported" } else { "request must be an object" };
            return Some(failure(&Value::Null, &RpcError::new(code::INVALID_REQUEST, why)));
        };
        // id 는 문자열·숫자·null 만 되고, 없으면 알림이다. 되돌려 줄 수 없는 id 는 null 로 답한다.
        let id = match object.get("id") {
            None => None,
            Some(id @ (Value::String(_) | Value::Number(_) | Value::Null)) => Some(id.clone()),
            Some(_) => {
                return Some(failure(
                    &Value::Null,
                    &RpcError::new(code::INVALID_REQUEST, "id must be a string, number or null"),
                ));
            }
        };
        let reply_id = id.clone().unwrap_or(Value::Null);
        let invalid = |why: &str| Some(failure(&reply_id, &RpcError::new(code::INVALID_REQUEST, why)));
        if object.get("jsonrpc") != Some(&json!("2.0")) {
            return invalid("jsonrpc must be \"2.0\"");
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return invalid("method must be a string");
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        if !matches!(params, Value::Null | Value::Object(_) | Value::Array(_)) {
            return invalid("params must be an object or an array");
        }
        // 알림: 받았다는 것까지만 — 아무것도 돌리지 않고 답하지 않는다.
        let id = id?;
        let Some(handler) = self.table.get(method) else {
            return Some(failure(&id, &RpcError::new(code::METHOD_NOT_FOUND, format!("method not found: {method}"))));
        };
        let env = RequestEnv::from_params(&params);
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&env, params)));
        Some(match ran {
            Ok(Ok(result)) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
            Ok(Err(e)) => failure(&id, &e),
            Err(_) => failure(&id, &RpcError::new(code::INTERNAL, "internal error")),
        })
    }
}

fn failure(id: &Value, error: &RpcError) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": error.to_value() }).to_string()
}

/// `hello` — 클라이언트가 `proto` 를 말하고, 데몬은 제 `proto` 와 바이너리 판을 답한다.
/// 옛 `proto` 는 받는다(데몬이 더 새로우면 옛 클라이언트의 말을 알아듣는다). 새 `proto` 는 거절한다.
fn hello(_: &RequestEnv, params: Value) -> Result<Value, RpcError> {
    let proto = params
        .get("proto")
        .and_then(Value::as_u64)
        .ok_or_else(|| RpcError::invalid_params("hello needs params.proto (a non-negative integer)"))?;
    if proto > u64::from(PROTO) {
        let mut error = RpcError::new(
            code::PROTO_TOO_NEW,
            format!(
                "client speaks proto {proto}, this daemon speaks {PROTO}: upgrade the daemon (moai {})",
                env!("CARGO_PKG_VERSION")
            ),
        );
        error.data = Some(
            json!({ "code": "unsupported_proto", "proto": PROTO, "client_proto": proto, "version": env!("CARGO_PKG_VERSION") }),
        );
        return Err(error);
    }
    Ok(json!({ "proto": PROTO, "version": env!("CARGO_PKG_VERSION") }))
}

/// 줄 읽기의 결과.
#[derive(Debug, PartialEq)]
pub enum Line {
    /// 연결이 닫혔다.
    Eof,
    /// 개행을 뗀 한 줄.
    Text(Vec<u8>),
    /// 상한을 넘었다 — 그 줄의 나머지는 읽어 버렸다.
    TooLong,
}

/// 줄 하나를 상한까지만 읽는다. 상한을 넘으면 개행까지 버려 다음 줄에서 다시 맞춘다.
pub fn read_line<R: BufRead>(reader: &mut R) -> std::io::Result<Line> {
    let mut line = Vec::new();
    let mut over = false;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok(if over {
                Line::TooLong
            } else if line.is_empty() {
                Line::Eof
            } else {
                Line::Text(line)
            });
        }
        let (take, found) = match buf.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (buf.len(), false),
        };
        if !over {
            let body = if found { take - 1 } else { take };
            if line.len() + body > LINE_MAX {
                over = true;
                line.clear();
            } else {
                line.extend_from_slice(&buf[..body]);
            }
        }
        reader.consume(take);
        if found {
            return Ok(if over { Line::TooLong } else { Line::Text(line) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(methods: &Methods, text: &str) -> Value {
        serde_json::from_str(&methods.handle(text.as_bytes()).expect("a reply")).unwrap()
    }

    fn code_of(reply: &Value) -> i64 {
        reply["error"]["code"].as_i64().unwrap()
    }

    #[test]
    fn truncated_json_is_a_parse_error() {
        let reply = call(&Methods::new(), r#"{"jsonrpc":"2.0","id":1,"meth"#);
        assert_eq!(code_of(&reply), code::PARSE_ERROR);
        assert_eq!(reply["id"], Value::Null);
    }

    #[test]
    fn an_over_long_line_is_refused_and_the_reader_resyncs() {
        let mut data = vec![b'x'; LINE_MAX + 10];
        data.push(b'\n');
        data.extend_from_slice(b"{\"a\":1}\n");
        let mut reader = std::io::BufReader::with_capacity(4096, &data[..]);
        assert_eq!(read_line(&mut reader).unwrap(), Line::TooLong);
        assert_eq!(read_line(&mut reader).unwrap(), Line::Text(b"{\"a\":1}".to_vec()));
        assert_eq!(read_line(&mut reader).unwrap(), Line::Eof);
        let reply = call(&Methods::new(), &"x".repeat(LINE_MAX + 1));
        assert_eq!(code_of(&reply), code::INVALID_REQUEST);
    }

    #[test]
    fn a_line_of_exactly_the_cap_is_read() {
        let mut data = vec![b'x'; LINE_MAX];
        data.push(b'\n');
        let mut reader = std::io::BufReader::new(&data[..]);
        assert!(matches!(read_line(&mut reader).unwrap(), Line::Text(t) if t.len() == LINE_MAX));
    }

    #[test]
    fn an_unknown_method_is_not_found() {
        let reply = call(&Methods::new(), r#"{"jsonrpc":"2.0","id":"a","method":"nope"}"#);
        assert_eq!(code_of(&reply), code::METHOD_NOT_FOUND);
        assert_eq!(reply["id"], "a");
    }

    #[test]
    fn wrong_params_are_invalid_params() {
        let methods = Methods::new();
        for text in [
            r#"{"jsonrpc":"2.0","id":1,"method":"hello"}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":"1"}}"#,
        ] {
            assert_eq!(code_of(&call(&methods, text)), code::INVALID_PARAMS, "{text}");
        }
        // 구조가 없는 params 는 요청 자체가 틀렸다.
        let reply = call(&methods, r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":5}"#);
        assert_eq!(code_of(&reply), code::INVALID_REQUEST);
    }

    #[test]
    fn a_missing_or_wrong_jsonrpc_is_invalid_request() {
        let methods = Methods::new();
        for text in [
            r#"{"id":1,"method":"ping"}"#,
            r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#,
            r#"{"jsonrpc":2,"id":1,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":1}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":7}"#,
            r#"{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}"#,
            "42",
        ] {
            assert_eq!(code_of(&call(&methods, text)), code::INVALID_REQUEST, "{text}");
        }
        // 알아볼 수 있는 id 는 돌려준다.
        assert_eq!(call(&methods, r#"{"id":9,"method":"ping"}"#)["id"], 9);
    }

    #[test]
    fn a_batch_is_invalid_request() {
        let reply = call(&Methods::new(), r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
        assert_eq!(code_of(&reply), code::INVALID_REQUEST);
    }

    #[test]
    fn a_notification_runs_nothing_and_gets_no_reply() {
        let mut methods = Methods::new();
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = ran.clone();
        methods.register("touch", move |_, _| {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(json!(null))
        });
        assert_eq!(methods.handle(br#"{"jsonrpc":"2.0","method":"touch"}"#), None);
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
        let reply = call(&methods, r#"{"jsonrpc":"2.0","id":1,"method":"touch"}"#);
        assert_eq!(reply["result"], Value::Null);
        assert!(ran.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let reply = call(
            &Methods::new(),
            r#"{"jsonrpc":"2.0","id":1,"method":"hello","future":true,"params":{"proto":1,"later":[1,2]}}"#,
        );
        assert_eq!(reply["result"]["proto"], PROTO);
        assert_eq!(reply["result"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], 1);
    }

    #[test]
    fn hello_takes_an_older_proto_and_refuses_a_newer_one() {
        let methods = Methods::new();
        let older = call(&methods, r#"{"jsonrpc":"2.0","id":1,"method":"hello","params":{"proto":0}}"#);
        assert_eq!(older["result"]["proto"], PROTO);
        let newer = call(
            &methods,
            &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"hello","params":{{"proto":{}}}}}"#, PROTO + 1),
        );
        assert_eq!(code_of(&newer), code::PROTO_TOO_NEW);
        assert_eq!(newer["error"]["data"]["code"], "unsupported_proto");
        assert_eq!(newer["error"]["data"]["proto"], PROTO);
        assert_eq!(newer["error"]["data"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn the_env_comes_from_params_and_moai_failures_carry_the_cli_code() {
        let mut methods = Methods::new();
        methods.register("env", |env, _| Ok(json!([env.project, env.actor, env.lang, env.tz])));
        methods.register("gone", |_, _| Err(RpcError::moai("not_found", "no such issue")));
        methods.register("boom", |_, _| panic!("handler bug"));
        let reply = call(
            &methods,
            r#"{"jsonrpc":"2.0","id":1,"method":"env","params":{"project":"/p","actor":"A (a@b)","lang":"ko","tz":"UTC","x":1}}"#,
        );
        assert_eq!(reply["result"], json!(["/p", "A (a@b)", "ko", "UTC"]));
        let reply = call(&methods, r#"{"jsonrpc":"2.0","id":2,"method":"gone"}"#);
        assert_eq!(code_of(&reply), code::MOAI);
        assert_eq!(reply["error"]["data"]["code"], "not_found");
        let reply = call(&methods, r#"{"jsonrpc":"2.0","id":3,"method":"boom"}"#);
        assert_eq!(code_of(&reply), code::INTERNAL);
    }
}
