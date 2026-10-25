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
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 자식 한 판의 시한 — 넘으면 죽이고 504 다.
pub const TIME_LIMIT: Duration = Duration::from_secs(30);
/// stdout·stderr 각각의 상한 — 넘으면 죽이고 502 다. 아카이브까지 다 실은 목록이 이 안에 든다.
pub const OUT_MAX: usize = 64 * 1024 * 1024;

/// 자식을 어떻게 띄우나 — 서버를 띄울 때 한 번 정한다.
pub struct Spawner {
    /// 띄울 바이너리 — [`Spawner::exe_of`] 가 띄울 때 한 번 고른다.
    pub exe: PathBuf,
    /// 자식이 도는 자리 — 서버가 잡은 트래커의, 친 체크아웃 쪽 뿌리(`Repo::here`). 서버가 프로세스 안에서 답하는
    /// 끝점(`/api/boot`·`/api/stamp`)도 여기서 트래커를 찾는다.
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
    /// 자식으로 띄울 바이너리. **리눅스는 도는 이미지 그 자체다**(`/proc/self/exe`) — 같은 경로에 새 바이너리를
    /// 지어 넣거나(`cargo build`·`moai update`) 지워도 자식은 서버와 한 판이라, 서버가 프로세스 안에서 짓는 답
    /// (`/api/boot` 의 판·거르개 낱말, 이 바이너리에 든 페이지, 끝점의 argv 틀)과 자식의 답이 안 갈린다. 경로
    /// (`current_exe()`)를 띄우던 판은 그 경로에 선 새 파일을 띄웠고, 지운 뒤에는 모든 자식 끝점이 500 이었다.
    ///
    /// 그 밖의 OS 는 경로를 띄운다 — 갈아 끼운 뒤의 요청은 새 판이 받는다.
    pub fn exe_of() -> std::io::Result<PathBuf> {
        let image = std::path::Path::new("/proc/self/exe");
        match cfg!(target_os = "linux") && image.exists() {
            true => Ok(image.to_path_buf()),
            false => std::env::current_exe(),
        }
    }

    /// `cwd` 에서 돌린다 — 보통은 [`Spawner::cwd`] 고, 페이지가 고른 걸어 둔 프로젝트면 그 자리다
    /// (`project=`, [`super::project_of`]).
    pub fn run_in(&self, args: &[String], cwd: &std::path::Path, zone: Option<&str>) -> Result<Done, Fault> {
        self.run_within(args, cwd, zone, TIME_LIMIT)
    }

    /// [`Spawner::run_in`] 의 몸통 — 시한을 받는 것은 시험이 짧게 재려고서다.
    ///
    /// **파이프가 닫히기를 먼저 기다린다**(`git::finish_within` 과 같은 차례) — 자식이 끝나면 곧 닫히고, 상한을
    /// 넘긴 쪽은 거기서 그만 읽고 바로 알린다. 일정한 간격으로 깨어 묻던 판은 끝난 자식을 그 간격만큼 늦게 알았고,
    /// 자식이 끝난 뒤의 읽기에는 시한이 없어 파이프를 쥔 손자 하나에 요청이 끝없이 섰다.
    fn run_within(
        &self,
        args: &[String],
        cwd: &std::path::Path,
        zone: Option<&str>,
        limit: Duration,
    ) -> Result<Done, Fault> {
        let mut cmd = Command::new(&self.exe);
        cmd.arg("--json");
        if let Some(user) = &self.user {
            cmd.arg(format!("--user={user}"));
        }
        // 페이지가 날짜를 그리는 시간대(`/api/tree?tz=`)를 자식의 시간대로 준다 — `tz::Zone::system` 이 읽는 `TZ` 하나다.
        // 새 깃발이나 설정이 아니고, 안 준 끝점은 서버의 환경을 그대로 물려받는다.
        if let Some(zone) = zone {
            cmd.env("TZ", zone);
        }
        cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(Fault::Spawn)?;
        let deadline = Instant::now() + limit;
        let left = || deadline.saturating_duration_since(Instant::now());
        let (tx, rx) = mpsc::channel();
        // **실을 못 띄워도 자식을 남기지 않는다** — `std::thread::spawn` 은 그 자리에서 패닉해, 떠 있는 자식을
        // 죽이지도 거두지도 않은 채 버렸다. 띄운 쪽 실은 자식이 죽어 파이프가 닫히면 혼자 끝난다.
        let drained = drain(child.stdout.take(), Stream::Out, tx.clone())
            .and_then(|()| drain(child.stderr.take(), Stream::Err, tx));
        if let Err(e) = drained {
            give_up(child);
            return Err(Fault::Spawn(e));
        }
        let (mut stdout, mut stderr) = (None, None);
        while stdout.is_none() || stderr.is_none() {
            match rx.recv_timeout(left()) {
                Ok((Stream::Out, Some(got))) => stdout = Some(got),
                Ok((Stream::Err, Some(got))) => stderr = Some(got),
                Ok((_, None)) => {
                    give_up(child);
                    return Err(Fault::TooBig);
                }
                Err(_) => {
                    give_up(child);
                    return Err(Fault::Timeout);
                }
            }
        }
        // 파이프를 다 닫은 자식은 곧 끝난다 — 닫고도 안 끝나는 것은 남은 시한까지만 기다린다.
        let Some(status) = crate::git::waited(&mut child, left()) else {
            give_up(child);
            return Err(Fault::Timeout);
        };
        let exit = status.code().ok_or(Fault::Killed)?;
        Ok(Done { exit, stdout: stdout.unwrap_or_default(), stderr: stderr.unwrap_or_default() })
    }
}

/// 넘긴 자식을 죽이고 **거두는 일은 딴 실로 보낸다**(`git::finish_within` 과 같은 자) — 죽은 마운트에 걸린
/// 프로세스는 SIGKILL 을 받고도 커널 안에서 한참 안 끝날 수 있어, 여기서 `wait` 하면 시한을 넘긴 요청이 그동안
/// [`super::BUSY_MAX`] 의 한 자리를 쥔다. 실을 못 띄우면 안 거두고 간다 — 좀비는 서버가 끝날 때 걷힌다.
fn give_up(mut child: Child) {
    let _ = child.kill();
    let _ = std::thread::Builder::new().spawn(move || child.wait());
}

/// 어느 흐름의 것인가.
enum Stream {
    Out,
    Err,
}

/// 흐름 하나를 끝까지 읽어 보내는 실. **둘을 따로 읽는다** — 한쪽만 읽으면 다른 쪽 파이프가 차서 자식이 서고,
/// 시한까지 기다린 끝에 504 가 난다. 상한을 넘으면 그만 읽고 `None` 을 보낸다(자식은 받는 쪽이 죽인다).
/// 실을 못 띄우면 `Err` 다([`std::thread::Builder`] — `git::finish_within` 의 `drained` 와 같은 자).
fn drain(
    pipe: Option<impl Read + Send + 'static>,
    which: Stream,
    tx: mpsc::Sender<(Stream, Option<Vec<u8>>)>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .spawn(move || {
            let mut got = Vec::new();
            if let Some(mut pipe) = pipe {
                let mut chunk = [0u8; 64 * 1024];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                        Ok(n) if got.len() + n > OUT_MAX => {
                            let _ = tx.send((which, None));
                            return;
                        }
                        Ok(n) => got.extend_from_slice(&chunk[..n]),
                    }
                }
            }
            let _ = tx.send((which, Some(got)));
        })
        .map(|_| ())
}

/// `{"exit":N,"out":…,"err":…,"said":[…]}` — **서지 않는 키는 빠진다**(null 이 아니다).
///
/// - `out` 은 stdout 이 JSON 값 하나일 때 그 원문
/// - `err` 는 실패(`exit` ≠ 0)한 판의 오류 객체 원문 — `--json` 으로 넘어진 moai 는 그것을 stderr 의 **맨 끝 한
///   줄**로 낸다(`main::fail`). stderr 통째가 그 객체인 것이 흔한 판이지만, 그 앞에 `cmd::tell` 이 그 자리에서
///   낸 줄(못 읽은 줄의 알림 따위)이 설 수 있다 — 통째만 보던 판은 스냅샷에 깨진 줄 하나만 있어도 `code` 를
///   `said` 의 글 한 줄로 묻어, 페이지가 `not_found`·`bad_filter` 를 못 갈랐다
/// - 나머지 stderr 의 줄은 `said` 에. 성공한 판의 경고(`moai: …`)가 여기 선다
pub fn envelope(done: &Done) -> String {
    let mut out = format!("{{\"exit\":{}", done.exit);
    let stdout = String::from_utf8_lossy(&done.stdout);
    if let Some(v) = one_value(&stdout) {
        out.push_str(",\"out\":");
        out.push_str(v);
    }
    let stderr = String::from_utf8_lossy(&done.stderr);
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let (err, said) = match (done.exit != 0).then(|| one_object(&stderr)).flatten() {
        Some(whole) => (Some(whole), &[][..]),
        None => match lines.split_last() {
            Some((last, before)) if done.exit != 0 && one_object(last).is_some() => (one_object(last), before),
            _ => (None, &lines[..]),
        },
    };
    if let Some(v) = err {
        out.push_str(",\"err\":");
        out.push_str(v);
    }
    if !said.is_empty() {
        out.push_str(",\"said\":");
        out.push_str(&serde_json::to_string(said).unwrap_or_else(|_| "[]".into()));
    }
    out.push('}');
    out
}

/// 글이 JSON 값 하나면 앞뒤 빈칸을 뗀 그 원문. **재기만 한다**(`IgnoredAny`) — 값을 지으면 차례가 바뀐다.
fn one_value(text: &str) -> Option<&str> {
    let t = text.trim();
    (!t.is_empty() && serde_json::from_str::<serde::de::IgnoredAny>(t).is_ok()).then_some(t)
}

/// [`one_value`] 가 **객체**일 때만 — 넘어진 moai 의 오류 꼴(`{"code":…,"error":…}`)이다.
fn one_object(text: &str) -> Option<&str> {
    one_value(text).filter(|v| v.starts_with('{'))
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

    /// **앞에 경고가 서도 오류 객체는 `err` 다** — 못 읽은 줄의 알림은 넘어진 판에서도 그 자리에서 나간다.
    #[test]
    fn warnings_ahead_of_the_failure_stay_said_and_the_failure_is_err() {
        let stderr =
            "x.jsonl: unreadable rows 1\n  line 3: expected value\n{\"code\":\"not_found\",\"error\":\"없다\"}\n";
        let e = envelope(&done(1, "", stderr));
        assert_eq!(
            e,
            r#"{"exit":1,"err":{"code":"not_found","error":"없다"},"said":["x.jsonl: unreadable rows 1","  line 3: expected value"]}"#
        );
        // 끝 줄이 객체가 아니면 다 `said` 다 — 중간의 객체 꼴 줄을 골라 오류로 삼지 않는다.
        let e = envelope(&done(1, "", "{\"code\":\"x\"}\nmoai: 끝\n"));
        assert_eq!(e, r#"{"exit":1,"said":["{\"code\":\"x\"}","moai: 끝"]}"#);
    }

    /// 셸 글 하나를 돌릴 파일로 심는다. **이 프로세스가 쓴 inode 를 안 돌린다** — 쓰기 fd 가 열린 동안 옆 시험이
    /// fork 하면 그 자식이 fd 를 물려받아 Linux 가 exec 을 ETXTBSY 로 거절한다(`skill` 시험의 `planted` 와 같은 자,
    /// `tests/cli.rs` 의 `place_exe` 가 잰 자리). 쓴 것을 `cp` 로 옮기고, 모드는 그 뒤에 세운다.
    #[cfg(unix)]
    fn planted(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let (src, at) = (dir.join(format!("{name}.src")), dir.join(name));
        std::fs::write(&src, body).unwrap();
        let out = Command::new("cp").arg(&src).arg(&at).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
        at
    }

    /// **`--json` 이 맨 앞이고, 받은 `--user` 는 한 토막으로 넘어간다** — 떨구면 자식이 다른 사람으로 돈다.
    #[cfg(unix)]
    #[test]
    fn the_child_gets_json_first_and_the_user_as_one_word() {
        let s = crate::scratch::Scratch::new("web-run-argv");
        let exe = planted(s.path(), "echo.sh", "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        let spawner = Spawner { exe, cwd: s.path().to_path_buf(), user: Some("Bob Lee (b@b)".into()) };
        let got = spawner.run_in(&["show".into(), "--grep=a b".into()], &spawner.cwd, None).unwrap();
        assert_eq!(got.exit, 0);
        assert_eq!(String::from_utf8(got.stdout).unwrap(), "--json\n--user=Bob Lee (b@b)\nshow\n--grep=a b\n");
    }

    /// **시한을 넘긴 자식은 죽이고 기다리지 않는다** — 504 가 시한에서 선다. 둘을 잰다: 제가 안 끝나는 자식, 그리고
    /// 곧 끝났는데 **파이프를 쥔 손자**를 남긴 자식이다 — 끝난 뒤의 읽기에 시한이 없던 판은 손자가 끝날 때까지 섰다.
    /// 손자는 일부러 남기되 짧게 둔다(`git` 의 `a_call_past_its_budget_is_not_waited_for` 와 같은 꼴).
    #[cfg(unix)]
    #[test]
    fn a_child_past_its_limit_is_cut_at_the_limit() {
        let s = crate::scratch::Scratch::new("web-run-limit");
        // `exec` 로 갈아 끼운다 — 껍데기가 `sleep` 을 따로 띄우면 죽이는 것은 껍데기뿐이라 시험 뒤에도 그것이 남는다.
        for (name, body) in [("slow.sh", "#!/bin/sh\nexec sleep 20\n"), ("held.sh", "#!/bin/sh\nsleep 3 &\necho hi\n")]
        {
            let spawner = Spawner { exe: planted(s.path(), name, body), cwd: s.path().to_path_buf(), user: None };
            let started = Instant::now();
            let got = spawner.run_within(&[], &spawner.cwd, None, Duration::from_millis(300));
            assert!(matches!(got, Err(Fault::Timeout)), "{name}: {:?}", got.err());
            assert!(started.elapsed() < Duration::from_secs(2), "{name}: {:?}", started.elapsed());
        }
    }
}
