//! 자식 `moai --json …` 한 판을 돌리고 그 답을 봉투에 싼다.
//!
//! **서버는 뜻을 안 짓는다.** 데이터는 전부 자식의 stdout 이다 — 그 꼴은 docs/json.md 의 계약이고, 서버가
//! 고쳐 쓰면 계약이 둘이 된다. 그래서 봉투는 원문을 **글자째** 잇는다: `serde_json::Value` 로 한 번 풀었다
//! 묶으면 키 차례가 바뀐다(`serde_json` 의 `Map` 은 키로 늘어놓는다).
//!
//! **프로세스 안에서 `cmd::*::run` 을 안 부른다** — CLI 한 판을 전제로 한 전역 누적값(`store` 의 알림 칸들,
//! `cmd` 의 종료 코드·`AFTER`)을 `main` 이 끝에서 비우는데 서버는 끝이 없고, `-C` 는 프로세스 cwd 를 바꾼다.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// 자식 한 판의 시한 — 넘으면 죽이고 504 다.
pub const TIME_LIMIT: Duration = Duration::from_secs(30);
/// stdout·stderr 각각의 상한 — 넘으면 죽이고 502 다. 아카이브까지 다 실은 목록이 이 안에 든다.
pub const OUT_MAX: usize = 64 * 1024 * 1024;

/// 자식을 어떻게 띄우나 — 서버를 띄울 때 한 번 정한다.
pub struct Spawner {
    /// `current_exe()` 를 띄울 때 한 번 푼 값. 요청마다 풀면 그 사이 바이너리를 갈아 끼운 판에 반쯤 다른 판이 돈다.
    pub exe: PathBuf,
    /// 친 자리 — `main` 이 `-C` 를 지난 뒤의 `current_dir`.
    pub cwd: PathBuf,
    /// `--user` 로 받은 값 그대로. 읽기만 하는 명령은 안 쓰지만, 받은 것을 떨구면 자식이 다른 사람으로 돈다.
    pub user: Option<String>,
}

/// 자식이 돌고 끝났다 — 무슨 코드로 끝났든.
pub struct Done {
    pub exit: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// 자식이 답을 못 냈다.
#[derive(Debug)]
pub enum Fault {
    Spawn(std::io::Error),
    Timeout,
    TooBig,
    /// 코드 없이 끝났다 — 신호에 죽었다.
    Killed,
}

impl Spawner {
    pub fn run(&self, args: &[String]) -> Result<Done, Fault> {
        let mut cmd = Command::new(&self.exe);
        cmd.arg("--json");
        if let Some(user) = &self.user {
            cmd.arg(format!("--user={user}"));
        }
        cmd.args(args).current_dir(&self.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(Fault::Spawn)?;
        let over = Arc::new(AtomicBool::new(false));
        let out = drain(child.stdout.take(), over.clone());
        let err = drain(child.stderr.take(), over.clone());
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Fault::Spawn(e));
                }
            }
            if over.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Fault::TooBig);
            }
            if started.elapsed() > TIME_LIMIT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Fault::Timeout);
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let stdout = out.join().unwrap_or_default();
        let stderr = err.join().unwrap_or_default();
        // 끝난 뒤에 읽은 나머지가 상한을 넘었을 수 있다 — 고리가 못 본 것이다.
        if over.load(Ordering::Relaxed) {
            return Err(Fault::TooBig);
        }
        let exit = status.code().ok_or(Fault::Killed)?;
        Ok(Done { exit, stdout, stderr })
    }
}

/// 흐름 하나를 끝까지 읽는 스레드. **둘을 따로 읽는다** — 한쪽만 읽으면 다른 쪽 파이프가 차서 자식이 서고,
/// 시한까지 기다린 끝에 504 가 난다. 상한을 넘으면 깃발을 세우고 그만 읽는다(자식은 고리가 죽인다).
fn drain(pipe: Option<impl Read + Send + 'static>, over: Arc<AtomicBool>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut got = Vec::new();
        let Some(mut pipe) = pipe else { return got };
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if got.len() + n > OUT_MAX {
                        over.store(true, Ordering::Relaxed);
                        break;
                    }
                    got.extend_from_slice(&chunk[..n]);
                }
            }
        }
        got
    })
}

/// `{"exit":N,"out":…,"err":…,"said":[…]}` — **서지 않는 키는 빠진다**(null 이 아니다).
///
/// - `out` 은 stdout 이 JSON 값 하나일 때 그 원문
/// - `err` 는 실패(`exit` ≠ 0)이고 stderr 통째가 JSON 객체일 때 그 원문 — `--json` 으로 넘어진 moai 의 꼴이다
/// - 아니면 `said` 에 stderr 의 줄들. 성공한 판의 경고(`moai: …`)가 여기 선다
pub fn envelope(done: &Done) -> String {
    let mut out = format!("{{\"exit\":{}", done.exit);
    let stdout = String::from_utf8_lossy(&done.stdout);
    if let Some(v) = one_value(&stdout) {
        out.push_str(",\"out\":");
        out.push_str(v);
    }
    let stderr = String::from_utf8_lossy(&done.stderr);
    match one_value(&stderr).filter(|v| done.exit != 0 && v.starts_with('{')) {
        Some(v) => {
            out.push_str(",\"err\":");
            out.push_str(v);
        }
        None => {
            let said: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
            if !said.is_empty() {
                out.push_str(",\"said\":");
                out.push_str(&serde_json::to_string(&said).unwrap_or_else(|_| "[]".into()));
            }
        }
    }
    out.push('}');
    out
}

/// 글이 JSON 값 하나면 앞뒤 빈칸을 뗀 그 원문. **재기만 한다**(`IgnoredAny`) — 값을 지으면 차례가 바뀐다.
fn one_value(text: &str) -> Option<&str> {
    let t = text.trim();
    (!t.is_empty() && serde_json::from_str::<serde::de::IgnoredAny>(t).is_ok()).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(exit: i32, stdout: &str, stderr: &str) -> Done {
        Done { exit, stdout: stdout.into(), stderr: stderr.into() }
    }

    #[test]
    fn the_envelope_keeps_the_raw_text_and_its_key_order() {
        let e = envelope(&done(0, "{\"z\":1,\"a\":[2,{\"y\":3,\"b\":4}]}\n", ""));
        assert_eq!(e, r#"{"exit":0,"out":{"z":1,"a":[2,{"y":3,"b":4}]}}"#);
    }

    #[test]
    fn text_that_is_not_one_json_value_is_no_out() {
        assert_eq!(envelope(&done(0, "Issues 3\n", "")), r#"{"exit":0}"#);
        assert_eq!(envelope(&done(0, "{}\n{}\n", "")), r#"{"exit":0}"#, "값 둘은 하나가 아니다");
        assert_eq!(envelope(&done(0, "", "")), r#"{"exit":0}"#);
    }

    #[test]
    fn a_failure_in_json_is_err_and_anything_else_is_said() {
        let e = envelope(&done(1, "", "{\"error\":\"없다\",\"code\":\"not_found\"}\n"));
        assert_eq!(e, r#"{"exit":1,"err":{"error":"없다","code":"not_found"}}"#);
        // 성공한 판의 stderr 는 JSON 꼴이어도 오류가 아니다.
        let e = envelope(&done(0, "[]", "{\"x\":1}\n"));
        assert_eq!(e, r#"{"exit":0,"out":[],"said":["{\"x\":1}"]}"#);
        let e = envelope(&done(2, "", "moai: 하나\n\nmoai: \"둘\"\n"));
        assert_eq!(e, r#"{"exit":2,"said":["moai: 하나","moai: \"둘\""]}"#);
    }
}
