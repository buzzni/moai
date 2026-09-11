//! argv 를 코어 호출로 옮기고 낼 줄을 돌려준다.
//!
//! **이슈의 뜻을 판단하는 `if` 를 여기 두지 않는다.** 어떤 이슈가 무엇인지
//! 정하는 코드가 여기 있으면 나중에 TUI 가 그것을 다시 쓴다.

pub mod add;
pub mod init;
pub mod show;

use crate::cli::{Cli, Cmd, Typed};
use crate::model::Kind;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Fail {
    pub message: String,
    pub code: &'static str,
}

impl Fail {
    pub fn new(message: impl Into<String>) -> Fail {
        Fail { message: message.into(), code: "error" }
    }
    pub fn coded(message: impl Into<String>, code: &'static str) -> Fail {
        Fail { message: message.into(), code }
    }
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::new(m)
    }
}

pub type R<T> = Result<T, Fail>;

pub struct Ctx {
    pub json: bool,
}

/// 할 수 있는 것은 다 하고, 된 것과 안 된 것을 둘 다 보고한 뒤 비영 종료한다.
/// 이 깃발이 서면 결과를 다 낸 **뒤에** 종료 코드가 1 이 된다.
static PARTIAL: AtomicBool = AtomicBool::new(false);

pub fn note_partial() {
    PARTIAL.store(true, Ordering::Relaxed);
}
pub fn had_partial() -> bool {
    PARTIAL.load(Ordering::Relaxed)
}

/// 읽다 만난 잘못된 줄을 stderr 로 알린다. 결과는 그대로 낸다.
pub fn report_load_errors(path: &std::path::Path, errors: &[crate::store::LoadError]) {
    if errors.is_empty() {
        return;
    }
    note_partial();
    eprintln!(
        "{}: 읽을 수 없는 줄 {}개",
        path.display(),
        errors.len()
    );
    for e in errors.iter().take(5) {
        eprintln!("  {}줄: {}", e.line, e.message);
    }
    if errors.len() > 5 {
        eprintln!("  … {}개 더", errors.len() - 5);
    }
}

pub fn run(cli: Cli) -> R<Vec<String>> {
    let ctx = Ctx { json: cli.json };
    match cli.cmd {
        Cmd::Init { prefix } => init::run(&ctx, prefix.as_deref()),
        Cmd::Add(a) => add::run(&ctx, a, None),
        Cmd::Show(a) => show::run(&ctx, a, None),
        Cmd::Issue(t) => typed(&ctx, t, Kind::Issue),
        Cmd::Epic(t) => typed(&ctx, t, Kind::Epic),
    }
}

fn typed(ctx: &Ctx, cmd: Typed, kind: Kind) -> R<Vec<String>> {
    match cmd {
        Typed::Add(a) => add::run(ctx, a, Some(kind)),
        Typed::Show(a) => show::run(ctx, a, Some(kind)),
    }
}

/// `--json` 일 때 한 줄로 낸다.
///
/// **`serde_json::Value` 를 거치지 않는다.** `Value` 의 맵은 정렬돼 있어
/// 키가 알파벳 순으로 재배열되고, 그러면 파일과 `--json` 이 서로 다른 순서를
/// 말한다. 눈으로 훑을 때 `id` 가 줄 가운데에 있는 것도 그 탓이다.
pub fn json_line<T: serde::Serialize>(v: &T) -> R<Vec<String>> {
    serde_json::to_string(v)
        .map(|s| vec![s])
        .map_err(|e| Fail::new(e.to_string()))
}

/// 객체 하나에 필드를 덧붙여 낸다. 선언 순서를 지키려면 직렬화된 뒤에
/// 붙이는 수밖에 없다 — 중간에 `Value` 를 쓰면 순서가 사라진다.
pub fn json_with<T: serde::Serialize>(base: &T, extra: &[(&str, String)]) -> R<Vec<String>> {
    let mut s = serde_json::to_string(base).map_err(|e| Fail::new(e.to_string()))?;
    if !s.ends_with('}') {
        return Err(Fail::new("객체가 아니다"));
    }
    let empty = s == "{}";
    s.pop();
    for (i, (k, v)) in extra.iter().enumerate() {
        if !(empty && i == 0) {
            s.push(',');
        }
        s.push_str(&format!("\"{k}\":{v}"));
    }
    s.push('}');
    Ok(vec![s])
}
