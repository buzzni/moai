//! JSON-RPC 2.0 메시지 — 요청·응답·오류·메서드 표·줄 읽기.
//!
//! 정한 것:
//! - **판 확인은 `hello` 메서드다.** 모든 요청에 필드를 싣지 않는다 — 연결마다 한 번이면 되고,
//!   옛 클라이언트가 보내지 않는 필드를 요구하지 않는다. `hello` 를 안 불러도 다른 메서드는 돈다
//!   (상태를 두지 않는다 — HTTP 로 실어도 같다).
//! - **환경(`project`·`actor`·`lang`·`tz`)은 params 객체의 같은 이름 필드에서 뽑는다.** params 는 그대로
//!   처리 함수에 간다. 모르는 필드는 무시한다 — 새 클라이언트가 옛 데몬을 넘어뜨리지 않게.
//! - 알림(`id` 없음)은 받되 아무것도 돌리지 않고 답하지 않는다. 배치(배열)는 -32600.

use serde::Serialize;
use serde_json::value::RawValue;
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

/// 메서드가 돌려주는 결과 — 이미 쓰인 JSON 글 그대로다. `Value` 로 거치면 객체 키가 abc 차례로 바뀌어(`preserve_order`
/// 가 꺼져 있다) CLI 가 낸 바이트를 잃는다. `RawValue` 는 글을 파싱만 하고 그대로 싣는다(moai-3hgm.whp).
pub type Reply = Box<RawValue>;

/// 직렬화할 수 있는 값을 [`Reply`] 로 쓴다. 키 차례는 그 값의 직렬화가 정한 차례다.
pub fn reply(value: &impl Serialize) -> Result<Reply, RpcError> {
    serde_json::value::to_raw_value(value).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))
}

/// 한 줄의 JSON 글을 파싱만 하고 바이트 그대로 [`Reply`] 로 쓴다. JSON 이 아니면 `None`.
pub fn raw_line(line: &str) -> Option<Reply> {
    RawValue::from_string(line.to_string()).ok()
}

pub type Handler = Box<dyn Fn(&RequestEnv, Value) -> Result<Reply, RpcError> + Send + Sync>;

/// 단 메서드 하나 — 처리 함수와, 그것이 트래커를 바꾸는가.
struct Entry {
    handler: Handler,
    /// 쓰기 메서드다. 멈추는 데몬은 돌던 쓰기를 끝까지 기다리고 읽기는 `DRAIN` 까지만 기다린다(사람의 결정 B,
    /// moai-3hgm). 그 갈림을 전송(`server`)이 알아야 해 메서드를 달 때 받는다 — 이름으로 가리는 표를 따로 두면
    /// 새 쓰기 메서드가 그 표를 빠뜨려 말없이 읽기로 끊긴다.
    write: bool,
}

/// 메서드 이름 → 처리 함수.
pub struct Methods {
    table: BTreeMap<String, Entry>,
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
        methods.register("ping", |_, _| reply(&json!({})));
        methods
    }

    /// 단 메서드 이름들 — 명령 훑기 시험이 쓴다.
    #[cfg(test)]
    pub fn names(&self) -> Vec<&str> {
        self.table.keys().map(String::as_str).collect()
    }

    /// 읽기 메서드를 단다.
    pub fn register(
        &mut self,
        name: &str,
        handler: impl Fn(&RequestEnv, Value) -> Result<Reply, RpcError> + Send + Sync + 'static,
    ) {
        self.table.insert(name.to_string(), Entry { handler: Box::new(handler), write: false });
    }

    /// 쓰기 메서드를 단다 — 멈추는 데몬이 돌던 이 요청을 시한 없이 끝까지 기다린다([`Entry::write`]).
    pub fn register_write(
        &mut self,
        name: &str,
        handler: impl Fn(&RequestEnv, Value) -> Result<Reply, RpcError> + Send + Sync + 'static,
    ) {
        self.table.insert(name.to_string(), Entry { handler: Box::new(handler), write: true });
    }

    /// 줄 하나(끝의 개행은 있어도 없어도 된다)를 처리해 답할 줄을 돌려준다. 알림이면 `None`.
    #[cfg(test)]
    pub fn handle(&self, line: &[u8]) -> Option<String> {
        match self.prepare(line) {
            Prepared::Answered(answer) => answer,
            Prepared::Call(call) => Some(call.run()),
        }
    }

    /// 줄 하나를 파싱해 부를 처리 함수까지 찾는다 — 부르지는 않는다. 둘로 가른 까닭: 전송이 그 사이에서 요청을
    /// 세어 들인다(`server` 의 `Work::admit`). 쓰기인지는 메서드를 찾아야 알고, 들이지 않은 요청은 처리 함수에
    /// 닿지 않아야 한다(moai-3hgm.uz0).
    pub fn prepare(&self, line: &[u8]) -> Prepared<'_> {
        if line.len() > LINE_MAX {
            return Prepared::Answered(Some(too_long()));
        }
        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(e) => {
                let error = RpcError::new(code::PARSE_ERROR, format!("parse error: {e}"));
                return Prepared::Answered(Some(failure(&Value::Null, &error)));
            }
        };
        self.prepare_value(value)
    }

    fn prepare_value(&self, value: Value) -> Prepared<'_> {
        let answered = |id: &Value, error: RpcError| Prepared::Answered(Some(failure(id, &error)));
        let Value::Object(object) = value else {
            let why = if value.is_array() { "batch requests are not supported" } else { "request must be an object" };
            return answered(&Value::Null, RpcError::new(code::INVALID_REQUEST, why));
        };
        // id 는 문자열·숫자·null 만 되고, 없으면 알림이다. 되돌려 줄 수 없는 id 는 null 로 답한다.
        let id = match object.get("id") {
            None => None,
            Some(id @ (Value::String(_) | Value::Number(_) | Value::Null)) => Some(id.clone()),
            Some(_) => {
                return answered(
                    &Value::Null,
                    RpcError::new(code::INVALID_REQUEST, "id must be a string, number or null"),
                );
            }
        };
        let reply_id = id.clone().unwrap_or(Value::Null);
        let invalid = |why: &str| answered(&reply_id, RpcError::new(code::INVALID_REQUEST, why));
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
        let Some(id) = id else { return Prepared::Answered(None) };
        let Some(entry) = self.table.get(method) else {
            return answered(&id, RpcError::new(code::METHOD_NOT_FOUND, format!("method not found: {method}")));
        };
        Prepared::Call(Call { id, entry, params })
    }
}

/// [`Methods::prepare`] 의 결과.
pub enum Prepared<'a> {
    /// 처리 함수 없이 정해진 답 — 틀린 요청·없는 메서드·상한을 넘은 줄. `None` 은 알림이라 답하지 않는다.
    Answered(Option<String>),
    /// 처리 함수를 부를 요청 — 부르는 것은 [`Call::run`].
    Call(Call<'a>),
}

/// 파싱을 마치고 처리 함수를 찾은 요청 하나. 아직 아무것도 돌리지 않았다.
pub struct Call<'a> {
    id: Value,
    entry: &'a Entry,
    params: Value,
}

impl Call<'_> {
    /// 쓰기 메서드를 부르는가([`Methods::register_write`]).
    pub fn writes(&self) -> bool {
        self.entry.write
    }

    /// 처리 함수를 돌려 답할 줄을 쓴다. 처리 함수가 터져도 답한다(`-32603`).
    pub fn run(self) -> String {
        let Call { id, entry, params } = self;
        let env = RequestEnv::from_params(&params);
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (entry.handler)(&env, params)));
        match ran {
            Ok(Ok(result)) => success(&id, &result),
            Ok(Err(e)) => failure(&id, &e),
            Err(_) => failure(&id, &RpcError::new(code::INTERNAL, "internal error")),
        }
    }

    /// 돌리지 않고 거절하는 답 — 이 요청의 `id` 를 싣는다.
    pub fn refuse(&self, error: &RpcError) -> String {
        failure(&self.id, error)
    }
}

/// 성공 답 한 줄. `result` 는 [`Reply`] 의 바이트가 그대로 들어간다(키 차례를 안 바꾼다).
fn success(id: &Value, result: &RawValue) -> String {
    #[derive(Serialize)]
    struct Success<'a> {
        id: &'a Value,
        jsonrpc: &'static str,
        result: &'a RawValue,
    }
    // 이 글은 `Value`·`RawValue` 뿐이라 직렬화가 실패할 길이 없다. 그래도 터뜨리지 않고 오류 답으로 바꾼다.
    serde_json::to_string(&Success { id, jsonrpc: "2.0", result })
        .unwrap_or_else(|e| failure(id, &RpcError::new(code::INTERNAL, e.to_string())))
}

fn failure(id: &Value, error: &RpcError) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": error.to_value() }).to_string()
}

/// 요청을 못 읽어 `id` 를 모르는 거절 — 전송이 줄을 다 읽기 전에 내는 답(연결 상한 등)이 쓴다.
pub fn refusal(error: &RpcError) -> String {
    failure(&Value::Null, error)
}

/// 요청 줄의 `method` — 객체가 아니거나 이름이 글이 아니면 `None`. 전송이 줄을 돌리기 전에 메서드를 가려야 할 때
/// (상한에 찬 데몬의 수명 길) 쓴다. 판단은 거기까지다 — 요청이 맞는지는 [`Methods::handle`] 이 본다.
pub fn method_of(line: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(line).ok()?;
    Some(value.get("method")?.as_str()?.to_string())
}

/// 상한을 넘은 줄의 답. [`read_line`] 이 `Line::TooLong` 을 낸 자리와 [`Methods::handle`] 이 같은 글로 답한다.
pub fn too_long() -> String {
    refusal(&RpcError::new(code::INVALID_REQUEST, "request line exceeds 1 MiB"))
}

/// `hello` — 클라이언트가 `proto` 를 말하고, 데몬은 제 `proto` 와 바이너리 판, 프로세스 번호(`pid`)를 답한다.
/// `pid` 는 `moai daemon status`·`stop` 이 대는 값이다 — 거절하는 답의 `data` 에도 싣는다.
/// 옛 `proto` 는 받는다(데몬이 더 새로우면 옛 클라이언트의 말을 알아듣는다). 새 `proto` 는 거절한다.
fn hello(_: &RequestEnv, params: Value) -> Result<Reply, RpcError> {
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
            json!({ "code": "unsupported_proto", "proto": PROTO, "client_proto": proto, "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id() }),
        );
        return Err(error);
    }
    reply(&json!({ "proto": PROTO, "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id() }))
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
        // 신호에 끊긴 읽기는 다시 읽는다 — 시한을 건 소켓 읽기는 `SA_RESTART` 로도 다시 안 돈다(signal(7)). 넘기면 반쯤
        // 읽은 요청이 답 없이 끊긴다(리뷰 moai-3hgm.vjv.yqs 6번).
        let buf = match reader.fill_buf() {
            Ok(buf) => buf,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
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

    #[test]
    fn a_read_cut_by_a_signal_is_read_again() {
        struct Cut(bool, &'static [u8]);
        impl std::io::Read for Cut {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if std::mem::take(&mut self.0) {
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                std::io::Read::read(&mut self.1, buf)
            }
        }
        let mut reader = std::io::BufReader::new(Cut(true, b"{\"a\":1}\n"));
        assert_eq!(read_line(&mut reader).unwrap(), Line::Text(b"{\"a\":1}".to_vec()));
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
            reply(&json!(null))
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
        assert_eq!(reply["result"]["pid"], std::process::id());
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
        methods.register("env", |env, _| reply(&json!([env.project, env.actor, env.lang, env.tz])));
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
