//! 데몬 — 로컬 읽기 API (moai-5n6m).
//!
//! - `rpc` 는 JSON-RPC 2.0 **메시지 층**이다. 전송(Unix 소켓의 줄, 나중의 HTTP)을 모르고 `cmd`·`store` 의 어떤
//!   타입도 이름 대지 않는다.
//! - `server` 는 Unix 소켓의 줄 전송이다 — 연결마다 스레드 하나, 그 연결의 요청은 그 스레드에서 하나씩.
//! - `client` 는 붙는 쪽이다 — `moai daemon start·stop·status` 가 떠 있는 데몬을 찾고 멈추는 데 쓴다.
//!
//! 명령을 메서드에 묶는 자리는 [`methods`] 하나다. 소켓을 모르는 Windows 에서는 `rpc` 만 서고 아무도 안 불러
//! dead_code 가 선다 — 그쪽에서만 걷는다.

#[cfg_attr(not(unix), allow(dead_code))]
pub mod rpc;

#[cfg(unix)]
pub mod changes;
#[cfg(unix)]
pub mod client;
#[cfg(unix)]
pub mod diff;
#[cfg(unix)]
pub mod project;
#[cfg(unix)]
mod read;
#[cfg(unix)]
pub mod server;

/// 데몬을 멈추는 메서드. 판이 달라도 이름은 그대로여야 한다 — 새 바이너리의 `moai daemon stop` 이 옛 판 데몬을
/// 멈추는 길이 이것뿐이다.
#[cfg(unix)]
pub const SHUTDOWN: &str = "daemon.shutdown";

/// 데몬이 섬기는 메서드 표. `hello`·`ping` 위에 읽기 메서드 여섯과 `changes`(`read::register`, moai-5n6m.q16·cb9)를
/// 단다 — 다음 메서드도 여기서 단다. 데몬의 수명에 딸린 [`SHUTDOWN`] 은 `server::serve` 가 제 손으로 단다.
#[cfg(unix)]
pub fn methods() -> rpc::Methods {
    let mut methods = rpc::Methods::new();
    read::register(&mut methods);
    methods
}

/// 사용자당 하나인 소켓의 자리 — `moai ctl` 과 같은 디렉터리(`ctl::place::dir`)의 `daemon.socket`.
///
/// **확장자가 `.sock` 이 아니다.** `moai ctl ls` 는 그 디렉터리의 `*.sock` 을 모두 창으로 여겨 ctl 의 `hello` 를
/// 보낸다 — 데몬이 거기 서면 `ctl ls` 마다 데몬에 붙어 답을 기다리고, 죽은 데몬의 소켓은 ctl 이 지운다.
/// 이름이 갈려야 두 표면이 서로를 모른다.
#[cfg(unix)]
pub fn socket_path() -> std::io::Result<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let path = crate::ctl::place::dir()?.join("daemon.socket");
    // `sockaddr_un` 의 상한 — `ctl::place::socket` 과 같은 값이다.
    let limit = if cfg!(target_os = "macos") { 104 } else { 108 };
    if path.as_os_str().as_bytes().len() >= limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("daemon socket path is too long: {}", path.display()),
        ));
    }
    Ok(path)
}

#[cfg(all(test, unix))]
mod tests {
    use clap::CommandFactory as _;

    /// CLI 에 짝이 없는 메서드 — 데몬이나 API 에만 있는 것.
    const API_ONLY: &[(&str, &str)] = &[
        ("hello", "연결 첫 인사 — 판과 pid 를 맞춘다"),
        ("ping", "살아 있는지 본다"),
        ("changes", "폴링 — 커서가 가리키는 기억한 스냅샷과 견주어 바뀐 id. CLI 에는 같은 명령이 없다"),
    ];

    /// 메서드가 없는 명령과 그 까닭. **규칙**: 최상위 명령이 `<명령>.` 으로 시작하는 메서드를 하나라도 가지면 그 명령은
    /// 하위 명령마다(`project.ls`·`project.add`) 따로 가린다. 하나도 없으면 명령 하나로 센다(`wiki`·`ctl`).
    /// 새 명령이 생기면 이 표에 까닭을 적거나 메서드를 더해야 이 시험이 푸르다.
    const NOT_SERVED: &[(&str, &str)] = &[
        ("image", "파일을 저장한다 — 쓰기"),
        ("ctl", "사람의 열린 탐색기를 몬다 — 화면"),
        ("archive", "파일을 옮긴다 — 쓰기"),
        ("add", "쓰기 — 다음 에픽(moai-itt1)"),
        ("mv", "쓰기 — 다음 에픽(moai-itt1)"),
        ("edit", "쓰기 — 다음 에픽(moai-itt1)"),
        ("rm", "쓰기 — 다음 에픽(moai-itt1)"),
        ("note", "쓰기 — 다음 에픽(moai-itt1)"),
        ("defer", "쓰기 — 다음 에픽(moai-itt1)"),
        ("read", "읽음 표시 — 사용자 설정을 쓴다"),
        ("link", "쓰기 — 다음 에픽(moai-itt1)"),
        ("issue", "종류를 박은 별명 — show·add 따위의 틀. 메서드는 `show` 의 `type` 으로 간다"),
        ("epic", "종류를 박은 별명 — 메서드는 `show` 의 `type` 으로 간다"),
        ("milestone", "종류를 박은 별명 — 메서드는 `show` 의 `type` 으로 간다"),
        ("backlog", "종류를 박은 별명 — 메서드는 `show` 의 `type` 으로 간다"),
        ("wiki", "위키 페이지 — 트래커의 읽기가 아니다"),
        ("tui", "화면"),
        ("web", "화면 — 제 서버가 있다"),
        ("hook", "에이전트 훅의 배관"),
        ("merge-driver", "git 이 부르는 배관"),
        ("skill", "에이전트 스킬 설치 — 배관"),
        ("project.add", "쓰기 — 사용자 설정을 쓴다"),
        ("project.rm", "쓰기 — 사용자 설정을 쓴다"),
        ("project.color", "쓰기 — 사용자 설정을 쓴다"),
        ("due", "쓰기 — 우선순위를 올린다"),
        ("config", "저장소 설정을 읽는다 — 아직 짝이 없다"),
        ("models", "사용자 설정을 읽는다 — 아직 짝이 없다"),
        ("writing", "사용자 설정을 읽는다 — 아직 짝이 없다"),
        ("keys", "화면의 키 표"),
        ("update", "스스로 바꾼다 — 배관"),
        ("init", "쓰기"),
        ("daemon", "데몬 자신 — 소켓을 거치지 않는다"),
        ("watch", "노트를 기다리며 막는다 — 요청 하나가 연결의 스레드를 묶는다. 기다림은 `changes` 폴링이 맡는다"),
        ("supervise", "감독 배관 — 에이전트 조율"),
    ];

    /// 명령 → 메서드 이름들. 하위 명령은 `project.ls` 꼴로 편다.
    fn commands(served: &[&str]) -> Vec<String> {
        let mut out = Vec::new();
        for top in crate::cli::Cli::command().get_subcommands().filter(|c| c.get_name() != "help") {
            let name = top.get_name();
            let split = served.iter().any(|m| m.starts_with(&format!("{name}.")));
            if split {
                out.extend(
                    top.get_subcommands()
                        .filter(|c| c.get_name() != "help")
                        .map(|s| format!("{name}.{}", s.get_name())),
                );
            } else {
                out.push(name.to_string());
            }
        }
        out
    }

    #[test]
    fn every_cli_command_is_served_or_listed_as_not_served() {
        let methods = super::methods();
        let served = methods.names();
        let all = commands(&served);
        assert!(all.len() > 20, "명령 목록을 못 읽었다 — {all:?}");
        let mut loose = Vec::new();
        for c in &all {
            let listed = NOT_SERVED.iter().any(|(n, _)| n == c);
            let has = served.contains(&c.as_str());
            assert!(!(has && listed), "`{c}` 는 메서드이면서 NOT_SERVED 에 있다 — 표에서 뺀다");
            if !has && !listed {
                loose.push(format!("`moai {}`", c.replace('.', " ")));
            }
        }
        assert!(
            loose.is_empty(),
            "{} 는 데몬 메서드도 아니고 NOT_SERVED 에도 없다. 메서드를 달거나(src/daemon) \
             src/daemon/mod.rs 의 NOT_SERVED 에 까닭 한 줄과 함께 적는다",
            loose.join(", ")
        );
        // 표의 이름이 실제 명령이어야 한다 — 지워진 명령이 표에 남으면 붉다.
        for (n, why) in NOT_SERVED {
            assert!(all.iter().any(|c| c == n), "NOT_SERVED 의 `{n}` 은 없는 명령이다");
            assert!(!why.trim().is_empty(), "`{n}` 의 까닭이 비었다");
        }
    }

    #[test]
    fn every_method_maps_to_a_command_or_is_api_only() {
        let methods = super::methods();
        let served = methods.names();
        let all = commands(&served);
        for m in &served {
            let api = API_ONLY.iter().any(|(n, _)| n == m);
            assert!(
                api || all.iter().any(|c| c == m),
                "메서드 `{m}` 는 어느 명령에도 안 묶인다 — 이름을 CLI 명령에 맞추거나 API_ONLY 에 까닭과 함께 적는다"
            );
        }
        for (n, why) in API_ONLY {
            assert!(served.contains(n), "API_ONLY 의 `{n}` 은 달린 메서드가 아니다");
            assert!(!why.trim().is_empty());
        }
    }
}
