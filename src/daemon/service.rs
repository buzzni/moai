//! 데몬을 사용자의 서비스 관리자에 맡긴다 — `moai daemon install·uninstall`(moai-upq5).
//!
//! - **등록 상태는 서비스 파일 유무다.** 저장하는 값을 따로 두지 않는다. 그래서 등록에 실패하면 이번에 쓴 파일을
//!   치우고, 다시 쓴 것이 실패하면 옛 글을 되돌려 쓴다 — 파일만 남으면 "등록됐다" 고 거짓말하고, 옛 파일을 잃으면
//!   잘 돌던 등록이 사라진다. 해제도 같은 규칙이다: 파일을 걷으면 관리자가 멈추기를 거절해도 등록은 걷힌 것이다.
//! - **표지([`MARK`])가 없는 파일은 사람의 것이다.** `docs/daemon.md` 를 보고 같은 자리에 손으로 쓴 unit 일 수 있다.
//!   덮지도 지우지도 않는다.
//! - **다른 바이너리를 가리키는 파일도 덮지 않는다.** 한 사용자에 데몬은 하나이고, 어느 설치본이 그것을 맡을지는
//!   사람이 정한다. 둘째 설치가 첫째의 서비스를 말없이 빼앗으면 안 된다. "같은 바이너리" 는 글자가 아니라 같은 파일로
//!   풀리는가다([`Points`]) — 파일은 사람이 치는 링크 경로를 적고, `moai update` 는 푼 경로를 넘긴다.
//! - **moai 의 파일은 지금 쓸 글과 견준다**(moai-tloc.wnw). 같으면 그대로 두고, 다르면(소켓 디렉터리가 바뀌었다) 새
//!   글로 다시 쓰고 서비스를 다시 시작한다. 견주지 않던 판은 설정을 바꾸고 다시 친 `install` 이 옛 파일을 그대로 두어,
//!   서비스가 영영 다른 소켓을 봤다. 그래서 그리는 글에는 시각도 pid 도 없다 — 두 번째에도 같은 글이어야 한다.
//! - **`install`·`uninstall` 은 줄을 선다.** 서비스 파일이 사는 디렉터리를 잠근다([`Turn`]) — 겹친 둘 중 실패한
//!   쪽의 롤백이 성공한 쪽의 등록을 걷던 틈이다. 잠금 파일을 따로 두지 않아 남는 것이 없다.
//! - **관리자는 PATH 에서 찾는다.** 시험은 가짜 `systemctl`·`launchctl` 만 있는 PATH 로 돌아 사람의 서비스에 닿지 않는다.
//!
//! 명령을 부르는 자리는 [`Run`] 하나로 받는다 — launchd 의 글과 차례를 Linux 에서도 잴 수 있게.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const UNIT: &str = "moai-daemon.service";
const LABEL: &str = "com.buzzni.moai-daemon";
/// moai 가 쓴 파일의 표지. 이 글이 없으면 사람의 파일로 읽는다.
const MARK: &str = "moai daemon install wrote this file. moai daemon uninstall removes it.";
/// launchd 의 `bootout` 은 작업이 다 내려가기 전에 돌아올 수 있다 — 곧바로 `bootstrap` 하면 실패한다. 그만큼 기다린다.
const UNLOAD_WAIT: Duration = Duration::from_secs(5);
const UNLOAD_POLL: Duration = Duration::from_millis(100);

/// 명령 하나를 돌린다. 성공이면 표준 출력, 실패면 사람에게 보일 한 줄.
pub type Run<'a> = &'a dyn Fn(&str, &[&str]) -> Result<String, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manager {
    Systemd,
    Launchd,
}

impl Manager {
    pub fn name(self) -> &'static str {
        match self {
            Manager::Systemd => "systemd",
            Manager::Launchd => "launchd",
        }
    }

    /// 서비스 파일의 자리. 집을 모르면 없다.
    pub fn file(self, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
        let set = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        match self {
            Manager::Systemd => {
                let config = set("XDG_CONFIG_HOME")
                    .filter(|p| p.is_absolute())
                    .or_else(|| set("HOME").map(|h| h.join(".config")))?;
                Some(config.join("systemd").join("user").join(UNIT))
            }
            Manager::Launchd => Some(set("HOME")?.join("Library").join("LaunchAgents").join(format!("{LABEL}.plist"))),
        }
    }

    /// 서비스가 셸과 같은 소켓을 찾도록 **소켓 디렉터리를 늘 적는다**(moai-tloc.wnw) — 두 관리자 모두. launchd 는 셸의
    /// `TMPDIR` 을 안 주고, systemd 의 사용자 관리자는 `XDG_RUNTIME_DIR` 을 줄 뿐 셸이 그것을 바꿨는지, `MOAI_CTL_DIR`
    /// 을 셸의 rc 에만 두었는지를 모른다. 셸이 푼 자리를 그대로 적으면 그 물음이 없다.
    fn render(self, exe: &str, pin: &str) -> String {
        match self {
            Manager::Systemd => format!(
                "# {MARK}\n[Unit]\nDescription=moai daemon\n\n[Service]\nEnvironment=\"MOAI_CTL_DIR={}\"\n{}\nRestart=on-failure\nKillMode=mixed\n\n[Install]\nWantedBy=default.target\n",
                unit_escape(pin, false),
                exec_line(exe)
            ),
            Manager::Launchd => format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<!-- {MARK} -->\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key><string>{LABEL}</string>\n    <key>ProgramArguments</key>\n    <array>{}<string>daemon</string><string>start</string></array>\n    <key>EnvironmentVariables</key>\n    <dict><key>MOAI_CTL_DIR</key><string>{}</string></dict>\n    <key>RunAtLoad</key><true/>\n    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n</dict>\n</plist>\n",
                program_string(exe),
                xml_escape(pin)
            ),
        }
    }

    /// 파일에 적힌 바이너리 경로를 읽는다 — 쓸 때 싼 것을 거꾸로 푼다. 손으로 고쳐 못 읽는 꼴이면 없다.
    fn recorded(self, text: &str) -> Option<String> {
        match self {
            Manager::Systemd => {
                let quoted = text.lines().find_map(|line| line.strip_prefix("ExecStart=\""))?;
                let mut out = String::new();
                let mut chars = quoted.chars();
                loop {
                    match chars.next()? {
                        '\\' => out.push(chars.next()?),
                        '"' => break,
                        '%' => {
                            if chars.next()? != '%' {
                                return None;
                            }
                            out.push('%');
                        }
                        '$' => {
                            if chars.next()? != '$' {
                                return None;
                            }
                            out.push('$');
                        }
                        c => out.push(c),
                    }
                }
                (chars.as_str() == " daemon start").then_some(out)
            }
            Manager::Launchd => {
                let (_, args) = text.split_once("<key>ProgramArguments</key>")?;
                let (_, first) = args.split_once("<string>")?;
                let (exe, _) = first.split_once("</string>")?;
                Some(exe.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
            }
        }
    }
}

/// 파일이 가리키는 바이너리가 이 바이너리인가. 글자가 아니라 **같은 파일로 풀리는가** 로 본다(moai-tloc.gci) — 파일은
/// 사람이 치는 링크 경로를 적고, `moai update` 는 푼 경로를 넘긴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Points {
    /// 이 바이너리다.
    This,
    /// 적힌 경로가 더는 없다 — 끊긴 링크, 패키지 관리자가 지운 판. 그 등록은 아무것도 못 띄운다.
    Gone,
    /// 다른 바이너리거나, 적힌 줄을 못 읽는다.
    Other,
}

fn exec_line(exe: &str) -> String {
    format!("ExecStart=\"{}\" daemon start", unit_escape(exe, true))
}

fn program_string(exe: &str) -> String {
    format!("<string>{}</string>", xml_escape(exe))
}

/// 파일이 가리키는 바이너리가 `exe` 와 같은 파일인가. 적힌 경로가 없으면 [`Points::Gone`] — 끊긴 링크도 그렇다.
fn points(manager: Manager, text: &str, exe: &Path) -> Points {
    // `has_root` 다 — 이 판정의 단위 시험은 Windows 에서도 돌고, 그쪽에서 `/home/…` 은 절대 경로가 아니다.
    let Some(written) = manager.recorded(text).map(PathBuf::from).filter(|p| p.has_root()) else {
        return Points::Other;
    };
    match crate::path::canonical(&written) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Points::Gone,
        Err(_) => Points::Other,
        Ok(found) => {
            let this = crate::path::canonical(exe).unwrap_or_else(|_| crate::path::lexical(exe));
            if found == this { Points::This } else { Points::Other }
        }
    }
}

/// systemd 가 따옴표 안에서 다시 읽는 글자를 싼다. `$` 는 `ExecStart` 에서만 뜻이 있다.
fn unit_escape(s: &str, exec: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '%' => out.push_str("%%"),
            '$' if exec => out.push_str("$$"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 줄바꿈이 든 경로는 어느 파일 꼴로도 못 적는다.
fn writable_text(p: &Path) -> Result<&str, Trouble> {
    p.to_str().filter(|s| !s.contains(['\n', '\r'])).ok_or_else(|| Trouble::BadPath(p.display().to_string()))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Trouble {
    /// 표지가 없는 파일이 그 자리에 있다.
    NotOurs,
    /// moai 의 파일이지만 다른 바이너리를 가리킨다.
    OtherBinary,
    /// 파일에 적을 수 없는 경로다. 바이너리일 수도, 고정할 디렉터리일 수도 있어 그 경로를 든다.
    BadPath(String),
    /// 파일을 못 읽거나 못 썼다.
    File(String),
    /// 관리자가 없거나 거절했다.
    Refused(String),
}

/// 이 기계에서 서비스를 둘 수 없는 까닭.
#[derive(Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// 이 OS 에는 관리자가 없다 — Windows 는 다음 에픽이다(`moai-bq5p`).
    NoManager,
    /// 집을 몰라 파일의 경로를 못 정한다.
    NoHome,
}

/// `install` 이 파일에 한 일.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrote {
    /// 파일이 없어 새로 썼다.
    New,
    /// moai 의 파일이 지금 쓸 글과 달라 새 글로 다시 썼고, 서비스를 다시 시작했다.
    Rewritten,
    /// 지금 쓸 글과 같아 그대로 두었다. 멈춘 서비스만 띄웠다.
    Same,
}

/// `uninstall` 이 한 일.
#[derive(Debug, PartialEq, Eq)]
pub enum Removed {
    /// 걷을 것이 없었다.
    Nothing,
    /// 파일을 걷었다. `stopped` 가 `Err` 면 관리자가 멈추기를 거절한 말이다 — 데몬이 아직 돌 수 있다.
    File { stopped: Result<(), String> },
    /// 파일은 없는데 launchd 에 같은 label 의 작업이 올라가 있어 내렸다.
    Unloaded,
}

/// `install`·`uninstall` 의 차례 — 쥐고 있는 동안 같은 자리의 다른 차례가 기다린다. 놓는 것은 `Drop` 이다.
pub struct Turn {
    _held: Option<std::fs::File>,
}

pub struct Service {
    pub manager: Manager,
    pub file: PathBuf,
}

impl Service {
    /// 이 OS 의 서비스.
    pub fn native() -> Result<Service, Unavailable> {
        let manager = if cfg!(target_os = "linux") {
            Manager::Systemd
        } else if cfg!(target_os = "macos") {
            Manager::Launchd
        } else {
            return Err(Unavailable::NoManager);
        };
        let file = manager.file(|k| std::env::var_os(k)).ok_or(Unavailable::NoHome)?;
        Ok(Service { manager, file })
    }

    /// 서비스 파일이 있는가 — 그것이 곧 등록 상태다.
    pub fn registered(&self) -> bool {
        self.file.symlink_metadata().is_ok()
    }

    /// 파일의 글. 없으면 `None`. 글이 아니거나 끊어진 링크면 빈 글로 읽어 "사람의 파일" 로 간다.
    fn text(&self) -> Result<Option<String>, Trouble> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(self.registered().then(String::new)),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Ok(Some(String::new())),
            Err(e) => Err(Trouble::File(e.to_string())),
        }
    }

    /// 차례를 쥔다. 서비스 파일이 사는 디렉터리를 잠근다 — 그 디렉터리는 등록이 있는 한 있고, 잠금 파일을 따로 두면 그
    /// 파일이 남는다. `create` 가 아니면 없는 디렉터리를 만들지 않는다(그때는 걷을 것도 없다). 유닉스 밖에서는 잠그지 않는다
    /// — Windows 는 디렉터리를 파일로 못 열고, 그쪽에는 관리자도 없다.
    pub fn turn(&self, create: bool) -> std::io::Result<Turn> {
        let Some(dir) = self.file.parent() else { return Ok(Turn { _held: None }) };
        if create {
            std::fs::create_dir_all(dir)?;
        }
        #[cfg(unix)]
        {
            match std::fs::File::open(dir) {
                Ok(held) => {
                    fs2::FileExt::lock_exclusive(&held)?;
                    Ok(Turn { _held: Some(held) })
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && !create => Ok(Turn { _held: None }),
                Err(e) => Err(e),
            }
        }
        #[cfg(not(unix))]
        Ok(Turn { _held: None })
    }

    /// 등록하고 지금 시작한다. `pin` 은 서비스에 적을 소켓 디렉터리다 — 늘 적는다([`Manager::render`]).
    pub fn install(&self, exe: &Path, pin: &Path, run: Run) -> Result<Wrote, Trouble> {
        let exe = writable_text(exe)?;
        let pin = writable_text(pin)?;
        let _turn = self.turn(true).map_err(|e| Trouble::File(e.to_string()))?;
        let text = self.manager.render(exe, pin);
        let (wrote, old) = match self.text()? {
            Some(old) if !old.contains(MARK) => return Err(Trouble::NotOurs),
            // 적힌 바이너리가 사라진 파일은 이미 아무것도 못 띄운다 — 다른 바이너리로 거절하지 않고 이 바이너리로 다시 쓴다.
            Some(old) if points(self.manager, &old, Path::new(exe)) == Points::Other => {
                return Err(Trouble::OtherBinary);
            }
            Some(old) if old == text => (Wrote::Same, None),
            Some(old) => (Wrote::Rewritten, Some(old)),
            None => (Wrote::New, None),
        };
        if wrote != Wrote::Same {
            self.write(&text).map_err(|e| Trouble::File(e.to_string()))?;
        }
        if let Err(said) = self.start(run, wrote) {
            match old {
                // 파일 유무가 곧 상태다. 새로 쓴 것은 걷는다.
                None if wrote == Wrote::New => {
                    let _ = self.stop(run);
                    let _ = std::fs::remove_file(&self.file);
                    // `enable --now` 은 시작에 실패해도 wants 링크를 먼저 남긴다 — `disable` 마저 실패했으면 `uninstall` 처럼 걷는다.
                    self.unwant();
                    self.reload(run);
                }
                // 전부터 있던 등록은 잃지 않는다 — 옛 글을 그대로 되돌려 쓰고, 관리자도 그 글로 돌려 본다.
                Some(old) if self.write(&old).is_ok() => {
                    let _ = self.start(run, Wrote::Rewritten);
                }
                Some(_) => {}
                // 그대로인 것은 아무것도 안 걷는다 — 관리자가 잠시 안 답했다고 등록을 잃으면 안 된다.
                None => {}
            }
            return Err(Trouble::Refused(said));
        }
        Ok(wrote)
    }

    /// 멈추고 파일을 걷는다. **멈추기가 거절돼도 파일은 걷는다**(moai-tloc.wnw) — 파일이 곧 등록이고, 남겨 두면 moai 로는
    /// 그 등록을 영영 못 걷는다. 걷은 뒤 관리자의 상태는 그 뒤를 따른다(`daemon-reload`, 로그인 때 안 뜬다).
    pub fn uninstall(&self, run: Run) -> Result<Removed, Trouble> {
        let _turn = self.turn(false).map_err(|e| Trouble::File(e.to_string()))?;
        match self.text()? {
            None => return self.unload_leftover(run),
            Some(text) if !text.contains(MARK) => return Err(Trouble::NotOurs),
            Some(_) => {}
        }
        // 먼저 본다 — 멈춘 뒤에야 못 지우는 것을 알면 꺼진 서비스가 등록된 것으로 남는다.
        if let Some(dir) = self.file.parent()
            && !crate::update::writable(dir)
        {
            let said = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
            return Err(Trouble::File(format!("{}: {said}", dir.display())));
        }
        let stopped = self.stop(run);
        if let Err(e) = std::fs::remove_file(&self.file) {
            // 그래도 못 지웠다. 멈춘 것을 되돌려 "파일이 있다" 와 "켜져 있다" 가 같은 쪽을 말하게 한다.
            if stopped.is_ok() {
                self.resume(run);
            }
            return Err(Trouble::File(e.to_string()));
        }
        self.unwant();
        self.reload(run);
        Ok(Removed::File { stopped })
    }

    /// 파일 없이 남은 launchd 의 작업을 내린다. label 은 moai 만 쓴다 — 손으로 쓰라고 가르치는 것은 다른 label 이다.
    /// systemd 에서 파일 없이 떠 있는 같은 이름의 유닛은 사람이 다른 자리에 쓴 것일 수 있어 건드리지 않는다.
    fn unload_leftover(&self, run: Run) -> Result<Removed, Trouble> {
        if self.manager == Manager::Launchd && self.loaded(run).is_some() {
            run("launchctl", &["bootout", &service_target()]).map_err(Trouble::Refused)?;
            return Ok(Removed::Unloaded);
        }
        Ok(Removed::Nothing)
    }

    /// `moai update` 가 부른다. **moai 가 쓴 파일이고, 이 바이너리를 가리키고, 지금 실행 중일 때만** 다시 시작한다. 사람이 멈춘
    /// 서비스는 안 띄운다. 등록 상태를 바꾸는 호출은 이 길에 없다. 글 전체는 견주지 않는다 — 셸의 환경이 달라 글이 달라도
    /// 새 판으로는 다시 시작해야 한다.
    pub fn restart(&self, exe: &Path, run: Run) -> bool {
        let (Ok(exe), Ok(Some(text))) = (writable_text(exe), self.text()) else {
            return false;
        };
        if !text.contains(MARK) || points(self.manager, &text, Path::new(exe)) != Points::This || !self.active(run) {
            return false;
        }
        let done = match self.manager {
            Manager::Systemd => run("systemctl", &["--user", "restart", UNIT]),
            Manager::Launchd => run("launchctl", &["kickstart", "-k", &service_target()]),
        };
        done.is_ok()
    }

    fn active(&self, run: Run) -> bool {
        match self.manager {
            Manager::Systemd => run("systemctl", &["--user", "is-active", "--quiet", UNIT]).is_ok(),
            Manager::Launchd => self.loaded(run).is_some_and(|said| runs(&said)),
        }
    }

    /// 관리자가 아는 서비스의 데몬 pid — systemd 의 `MainPID`, launchd `print` 의 `pid = N`. 안 돌면 `Ok(None)`.
    /// **관리자에 못 닿으면 `Err` 다**(리뷰 moai-tloc.pjd 3) — systemd 사용자 버스가 없는 셸(`su -`, 어떤 SSH)에서 "안 돈다"
    /// 로 읽으면 서비스의 데몬을 직접 띄운 것으로 오판한다. launchd 의 `print` 실패는 "올라간 작업이 없다" 라 `Ok(None)` 이다.
    /// 상태를 바꾸지 않는 읽기만 부른다.
    pub fn main_pid(&self, run: Run) -> Result<Option<u32>, String> {
        let pid = |said: &str| said.trim().parse::<u32>().ok().filter(|&pid| pid != 0);
        Ok(match self.manager {
            Manager::Systemd => pid(&run("systemctl", &["--user", "show", "-p", "MainPID", "--value", UNIT])?),
            Manager::Launchd => self
                .loaded(run)
                .and_then(|said| said.lines().find_map(|line| line.trim().strip_prefix("pid = ")).and_then(pid)),
        })
    }

    /// launchd 에 이 label 로 올라간 작업이 있으면 `print` 의 답.
    fn loaded(&self, run: Run) -> Option<String> {
        run("launchctl", &["print", &service_target()]).ok()
    }

    fn start(&self, run: Run, wrote: Wrote) -> Result<(), String> {
        match self.manager {
            Manager::Systemd => {
                run("systemctl", &["--user", "daemon-reload"])?;
                // 돌던 서비스에 `enable --now` 는 아무것도 안 한다 — 새 글을 읽히려면 다시 시작해야 한다. `restart` 는 멈춘
                // 서비스도 띄우니 그때는 `--now` 를 안 붙인다: 붙이면 멈춰 있던 서비스를 띄우자마자 다시 띄운다.
                if wrote == Wrote::Rewritten {
                    run("systemctl", &["--user", "enable", UNIT])?;
                    run("systemctl", &["--user", "restart", UNIT])?;
                } else {
                    run("systemctl", &["--user", "enable", "--now", UNIT])?;
                }
            }
            // 파일을 썼으면 같은 label 로 올라가 있는 것은 옛 정의다(다시 쓴 파일의 것이거나, 파일 없이 남은 것). `kickstart`
            // 로 그것을 띄우지 않고 내린 뒤 새 plist 를 올린다.
            Manager::Launchd if wrote != Wrote::Same => {
                if self.loaded(run).is_some() {
                    run("launchctl", &["bootout", &service_target()])?;
                    self.wait_unloaded(run);
                }
                run("launchctl", &["bootstrap", &domain(), &self.file.to_string_lossy()])?;
            }
            Manager::Launchd => match self.loaded(run) {
                None => {
                    run("launchctl", &["bootstrap", &domain(), &self.file.to_string_lossy()])?;
                }
                // 이미 올라간 서비스의 `bootstrap` 은 실패한다. 다만 깨끗이 끝난 서비스는 launchd 가 다시 안 띄우니
                // 여기서 띄운다 — `enable --now` 가 멈춘 서비스를 띄우는 것과 같게.
                Some(said) if !runs(&said) => {
                    run("launchctl", &["kickstart", &service_target()])?;
                }
                Some(_) => {}
            },
        }
        Ok(())
    }

    /// `bootout` 뒤 작업이 다 내려가기를 시한까지 기다린다. 시한이 지나면 그대로 간다 — 뒤의 `bootstrap` 이 실패를 말한다.
    fn wait_unloaded(&self, run: Run) {
        let deadline = Instant::now() + UNLOAD_WAIT;
        while self.loaded(run).is_some() && Instant::now() < deadline {
            std::thread::sleep(UNLOAD_POLL);
        }
    }

    fn stop(&self, run: Run) -> Result<(), String> {
        match self.manager {
            Manager::Systemd => run("systemctl", &["--user", "disable", "--now", UNIT]).map(|_| ()),
            // label 꼴로 내린다 — 파일 경로 꼴은 파일이 이미 없으면 못 선다.
            Manager::Launchd => match self.loaded(run) {
                Some(_) => run("launchctl", &["bootout", &service_target()]).map(|_| ()),
                None => Ok(()),
            },
        }
    }

    /// 멈춘 것을 되돌린다 — 힘껏. 파일을 못 지운 `uninstall` 이 등록을 켜진 채로 돌려놓는다.
    fn resume(&self, run: Run) {
        let _ = match self.manager {
            Manager::Systemd => run("systemctl", &["--user", "enable", "--now", UNIT]),
            // 방금 `bootout` 했다 — 다 내려가기 전에 올리면 `start` 가 기다리는 까닭과 같은 실패로 등록만 남고 꺼진다.
            Manager::Launchd => {
                self.wait_unloaded(run);
                run("launchctl", &["bootstrap", &domain(), &self.file.to_string_lossy()])
            }
        };
    }

    /// `disable` 이 못 지운 `default.target.wants/` 의 링크를 걷는다 — **우리 유닛을 가리키는 링크일 때만.** 따라가서
    /// 지우지 않고, 보통 파일과 다른 것을 가리키는 링크는 둔다.
    fn unwant(&self) {
        if self.manager != Manager::Systemd {
            return;
        }
        let (Some(dir), Some(name)) = (self.file.parent(), self.file.file_name()) else { return };
        let wants = dir.join("default.target.wants");
        let link = wants.join(name);
        if !link.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            return;
        }
        let Ok(to) = std::fs::read_link(&link) else { return };
        if crate::path::lexical(&wants.join(to)) == crate::path::lexical(&self.file) {
            let _ = std::fs::remove_file(&link);
        }
    }

    /// systemd 가 지운 파일을 잊게 한다. 파일을 이미 지운 뒤라 실패해도 등록 상태는 맞다.
    fn reload(&self, run: Run) {
        if self.manager == Manager::Systemd {
            let _ = run("systemctl", &["--user", "daemon-reload"]);
        }
    }

    /// 임시 파일에 쓰고 옮긴다 — 끊기면 반쪽 파일이 등록 상태로 읽힌다.
    fn write(&self, text: &str) -> std::io::Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let half = self.file.with_extension(format!("tmp.{}", std::process::id()));
        let placed = std::fs::write(&half, text)
            .and_then(|()| only_the_owner_writes(&half))
            .and_then(|()| std::fs::rename(&half, &self.file));
        if placed.is_err() {
            let _ = std::fs::remove_file(&half);
        }
        placed
    }
}

/// 사람의 umask 를 따르지 않는다 — launchd 는 그룹이나 남이 쓸 수 있는 plist 를 안 읽는다.
#[cfg(unix)]
fn only_the_owner_writes(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn only_the_owner_writes(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// `launchctl print` 의 답에서 지금 실행 중인가를 읽는다.
fn runs(said: &str) -> bool {
    said.lines().any(|line| line.trim() == "state = running")
}

/// `sudo` 로 불렀는가. 그러면 root 의 서비스가 되어, moai 를 쓰는 사람의 로그인에서 안 돈다.
pub fn under_sudo(env: impl Fn(&str) -> Option<OsString>) -> bool {
    crate::ctl::place::uid() == 0 && env("SUDO_USER").is_some_and(|v| !v.is_empty())
}

/// 관리자를 실제로 부른다. 표준 입력은 막는다 — 설치 스크립트의 표준 입력은 스크립트 자체일 수 있다.
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program).args(args).stdin(Stdio::null()).output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    // `systemctl enable --now` 는 시작에 실패해도 `Created symlink …` 를 먼저 적는다. 까닭은 그 뒤에 있다.
    let first = said.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("Created symlink"));
    Err(match first {
        Some(line) => format!("{program}: {line}"),
        None => format!("{program}: {}", out.status),
    })
}

fn domain() -> String {
    format!("gui/{}", crate::ctl::place::uid())
}

fn service_target() -> String {
    format!("{}/{LABEL}", domain())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::cell::{Cell, RefCell};

    fn service_in(s: &Scratch, manager: Manager) -> Service {
        Service { manager, file: s.path().join("user").join("the-service") }
    }

    /// 부른 명령을 적어 두는 가짜 관리자. `refuse` 에 든 낱말이 줄에 있으면 실패한다.
    struct Fake {
        calls: RefCell<Vec<String>>,
        refuse: &'static [&'static str],
        said: &'static str,
    }

    impl Fake {
        fn new(refuse: &'static [&'static str]) -> Fake {
            Fake { calls: RefCell::new(Vec::new()), refuse, said: "" }
        }

        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            let line = format!("{program} {}", args.join(" "));
            self.calls.borrow_mut().push(line.clone());
            match self.refuse.iter().any(|w| line.contains(w)) {
                true => Err("no".into()),
                false => Ok(self.said.into()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    /// 상태를 든 가짜 launchd. `bootout` 한 뒤에도 `print` 가 `lingers` 번 더 "올라가 있다" 고 답한다 — 작업이 다 내려가기
    /// 전에 돌아오는 `bootout` 이다. 올라가 있을 때의 `bootstrap` 은 실패한다(진짜 launchd 처럼).
    struct Launchd {
        calls: RefCell<Vec<String>>,
        loaded: Cell<bool>,
        lingers: Cell<u32>,
        after_bootout: u32,
    }

    impl Launchd {
        fn new(loaded: bool, after_bootout: u32) -> Launchd {
            Launchd { calls: RefCell::default(), loaded: Cell::new(loaded), lingers: Cell::new(0), after_bootout }
        }

        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            let line = format!("{program} {}", args.join(" "));
            self.calls.borrow_mut().push(line);
            match args.first().copied() {
                Some("print") if self.lingers.get() > 0 => {
                    self.lingers.set(self.lingers.get() - 1);
                    Ok("\tstate = running\n".into())
                }
                Some("print") if self.loaded.get() => Ok("\tstate = running\n".into()),
                Some("print") => Err("Could not find service".into()),
                Some("bootout") => {
                    self.loaded.set(false);
                    self.lingers.set(self.after_bootout);
                    Ok(String::new())
                }
                Some("bootstrap") if self.loaded.get() || self.lingers.get() > 0 => {
                    Err("Bootstrap failed: 5: Input/output error".into())
                }
                Some("bootstrap") => {
                    self.loaded.set(true);
                    Ok(String::new())
                }
                _ => Ok(String::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    const EXE: &str = "/home/me/.local/bin/moai";
    const PIN: &str = "/run/user/1000/moai";

    fn exe() -> &'static Path {
        Path::new(EXE)
    }

    fn pin() -> &'static Path {
        Path::new(PIN)
    }

    #[test]
    fn the_unit_is_the_one_the_manual_taught() {
        let text = Manager::Systemd.render(EXE, PIN);
        assert!(text.contains(MARK));
        assert!(text.contains("ExecStart=\"/home/me/.local/bin/moai\" daemon start\n"), "{text}");
        assert!(text.contains("Restart=on-failure\n"));
        assert!(text.contains("KillMode=mixed\n"));
        assert!(text.contains("WantedBy=default.target\n"));
    }

    /// systemd 도 늘 고정한다(moai-tloc.wnw) — 사용자 관리자가 주는 `XDG_RUNTIME_DIR` 이 셸의 것과 같다고 믿던 판은 셸이
    /// 다른 자리를 쓰면 서비스와 셸이 다른 소켓을 봤다.
    #[test]
    fn both_managers_always_pin_the_socket_directory() {
        let unit = Manager::Systemd.render(EXE, PIN);
        assert!(unit.contains("Environment=\"MOAI_CTL_DIR=/run/user/1000/moai\"\n"), "{unit}");
        let plist = Manager::Launchd.render(EXE, PIN);
        assert!(plist.contains("<key>MOAI_CTL_DIR</key><string>/run/user/1000/moai</string>"), "{plist}");
    }

    #[test]
    fn a_unit_quotes_what_systemd_would_read_again() {
        let text = Manager::Systemd.render("/my dir/50%/$HOME/a\"b\\c/moai", "/run/my 100%/$x");
        assert!(text.contains(r#"ExecStart="/my dir/50%%/$$HOME/a\"b\\c/moai" daemon start"#), "{text}");
        // `Environment=` 는 `$` 를 안 펼친다 — 두 겹으로 싸면 다른 경로가 된다.
        assert!(text.contains("Environment=\"MOAI_CTL_DIR=/run/my 100%%/$x\"\n"), "{text}");
    }

    #[test]
    fn a_plist_pins_the_socket_directory_and_escapes_xml() {
        let text = Manager::Launchd.render("/Users/a&b/<moai>", "/var/folders/x&y/T/moai-501");
        assert!(text.contains(MARK));
        assert!(
            text.contains("<string>/Users/a&amp;b/&lt;moai&gt;</string><string>daemon</string><string>start</string>"),
            "{text}"
        );
        assert!(text.contains("<key>MOAI_CTL_DIR</key><string>/var/folders/x&amp;y/T/moai-501</string>"), "{text}");
        assert!(text.contains(&format!("<key>Label</key><string>{LABEL}</string>")), "{text}");
    }

    // Windows 에서는 `/c` 가 절대 경로가 아니다. 그쪽에는 관리자가 없다.
    #[cfg(unix)]
    #[test]
    fn the_file_follows_the_config_home_and_a_relative_one_is_ignored() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
        };
        assert_eq!(
            Manager::Systemd.file(env(&[("HOME", "/h")])),
            Some(PathBuf::from("/h/.config/systemd/user/moai-daemon.service"))
        );
        assert_eq!(
            Manager::Systemd.file(env(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "/c")])),
            Some(PathBuf::from("/c/systemd/user/moai-daemon.service"))
        );
        assert_eq!(
            Manager::Systemd.file(env(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "rel")])),
            Some(PathBuf::from("/h/.config/systemd/user/moai-daemon.service"))
        );
        assert_eq!(Manager::Systemd.file(env(&[])), None);
        assert_eq!(
            Manager::Launchd.file(env(&[("HOME", "/h")])),
            Some(PathBuf::from("/h/Library/LaunchAgents/com.buzzni.moai-daemon.plist"))
        );
        assert_eq!(Manager::Launchd.file(env(&[("HOME", "")])), None);
    }

    #[test]
    fn a_failed_registration_leaves_no_file_and_an_old_one_stays() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let fake = Fake::new(&["enable"]);
        let run = |p: &str, a: &[&str]| fake.run(p, a);
        let got = svc.install(exe(), pin(), &run);
        assert_eq!(got, Err(Trouble::Refused("no".into())));
        assert!(!svc.registered(), "실패했는데 파일이 남았다");

        // 전부터 있던 등록은 실패해도 안 지운다 — 관리자가 잠시 안 답했다고 등록을 잃으면 안 된다.
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::New));
        assert_eq!(svc.install(exe(), pin(), &run), Err(Trouble::Refused("no".into())));
        assert!(svc.registered(), "전부터 있던 등록을 지웠다");
    }

    /// 설정을 바꿔 다시 친 `install`(리뷰 moai-tsk0 의 195) — 다른 글이면 다시 쓰고 다시 시작하고, 같은 글이면 아무것도
    /// 안 건드린다.
    #[test]
    fn a_changed_file_is_rewritten_and_restarted_and_an_unchanged_one_is_left_alone() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(exe(), Path::new("/a"), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::New));
        assert!(!ok.calls().iter().any(|c| c.contains(" restart ")), "새로 쓴 것을 다시 시작했다 {:?}", ok.calls());

        let moved = Fake::new(&[]);
        assert_eq!(svc.install(exe(), Path::new("/b"), &|p: &str, a: &[&str]| moved.run(p, a)), Ok(Wrote::Rewritten));
        let text = std::fs::read_to_string(&svc.file).unwrap();
        assert!(text.contains("MOAI_CTL_DIR=/b\""), "{text}");
        assert_eq!(
            moved.calls(),
            [
                "systemctl --user daemon-reload",
                "systemctl --user enable moai-daemon.service",
                "systemctl --user restart moai-daemon.service"
            ]
        );

        let again = Fake::new(&[]);
        assert_eq!(svc.install(exe(), Path::new("/b"), &|p: &str, a: &[&str]| again.run(p, a)), Ok(Wrote::Same));
        assert_eq!(std::fs::read_to_string(&svc.file).unwrap(), text, "같은 글인데 파일을 바꿨다");
        assert!(!again.calls().iter().any(|c| c.contains("restart")), "같은 글인데 다시 시작했다 {:?}", again.calls());
    }

    #[test]
    fn a_failed_rewrite_puts_the_old_file_back() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        svc.install(exe(), Path::new("/a"), &|p: &str, a: &[&str]| ok.run(p, a)).unwrap();
        let old = std::fs::read_to_string(&svc.file).unwrap();
        let no = Fake::new(&["restart"]);
        let got = svc.install(exe(), Path::new("/b"), &|p: &str, a: &[&str]| no.run(p, a));
        assert_eq!(got, Err(Trouble::Refused("no".into())));
        assert_eq!(std::fs::read_to_string(&svc.file).unwrap(), old, "실패한 다시 쓰기가 옛 등록을 잃었다");
    }

    #[test]
    fn launchd_loads_once_and_unloads_only_what_is_loaded() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Launchd);
        let file = svc.file.display().to_string();
        let domain = domain();

        let unloaded = Fake::new(&["print"]);
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| unloaded.run(p, a)), Ok(Wrote::New));
        assert_eq!(
            unloaded.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl bootstrap {domain} {file}")]
        );

        let loaded = Fake { calls: RefCell::default(), refuse: &[], said: "\tstate = running\n" };
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| loaded.run(p, a)), Ok(Wrote::Same));
        assert_eq!(loaded.calls(), [format!("launchctl print {domain}/{LABEL}")], "이미 올라간 것을 또 올렸다");

        // 올라가 있어도 멈춰 있으면 지금 띄운다 — `moai daemon stop` 의 깨끗한 종료는 launchd 가 다시 안 띄운다.
        let stopped = Fake { calls: RefCell::default(), refuse: &[], said: "\tstate = not running\n" };
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| stopped.run(p, a)), Ok(Wrote::Same));
        assert_eq!(
            stopped.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl kickstart {domain}/{LABEL}")]
        );

        // 안 올라가 있으면 `bootout` 을 안 부른다.
        let gone = Fake::new(&["print"]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| gone.run(p, a)), Ok(Removed::File { stopped: Ok(()) }));
        assert_eq!(gone.calls(), [format!("launchctl print {domain}/{LABEL}")]);
        assert!(!svc.registered());

        // 내릴 때는 label 꼴이다 — 파일 경로 꼴은 파일이 없으면 못 선다.
        let unloaded = Fake::new(&["print"]);
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| unloaded.run(p, a)), Ok(Wrote::New));
        let loaded = Fake::new(&[]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| loaded.run(p, a)), Ok(Removed::File { stopped: Ok(()) }));
        assert_eq!(
            loaded.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl bootout {domain}/{LABEL}")]
        );
    }

    /// 다시 쓴 plist 는 옛 작업을 내리고, **다 내려간 뒤에** 올린다 — 곧바로 올리면 "Input/output error" 다.
    #[test]
    fn a_rewritten_plist_replaces_the_loaded_job_once_it_is_gone() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Launchd);
        let first = Launchd::new(false, 0);
        assert_eq!(svc.install(exe(), Path::new("/a"), &|p: &str, a: &[&str]| first.run(p, a)), Ok(Wrote::New));

        let lm = Launchd::new(true, 3);
        assert_eq!(svc.install(exe(), Path::new("/b"), &|p: &str, a: &[&str]| lm.run(p, a)), Ok(Wrote::Rewritten));
        let target = service_target();
        let calls = lm.calls();
        let bootout = calls.iter().position(|c| *c == format!("launchctl bootout {target}")).expect("안 내렸다");
        let bootstrap = calls.iter().position(|c| c.starts_with("launchctl bootstrap ")).expect("안 올렸다");
        assert!(bootout < bootstrap, "{calls:?}");
        assert_eq!(calls[bootout + 1..bootstrap].len(), 4, "다 내려가기를 안 기다렸다 {calls:?}");
        assert!(!calls.iter().any(|c| c.contains("kickstart")), "옛 정의를 띄웠다 {calls:?}");
        assert!(std::fs::read_to_string(&svc.file).unwrap().contains("<string>/b</string>"));
    }

    /// 파일 없이 남은 같은 label 의 작업(리뷰 moai-tsk0 의 255) — `install` 은 내리고 새 plist 를 올리고, `uninstall` 은 내린다.
    #[test]
    fn a_launchd_job_left_without_its_file_is_replaced_and_unloaded() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Launchd);
        let lm = Launchd::new(true, 1);
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| lm.run(p, a)), Ok(Wrote::New));
        let calls = lm.calls();
        assert!(calls.contains(&format!("launchctl bootout {}", service_target())), "{calls:?}");
        assert!(calls.last().is_some_and(|c| c.starts_with("launchctl bootstrap ")), "{calls:?}");
        assert!(!calls.iter().any(|c| c.contains("kickstart")), "{calls:?}");

        std::fs::remove_file(&svc.file).unwrap();
        let left = Launchd::new(true, 0);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| left.run(p, a)), Ok(Removed::Unloaded));
        assert!(left.calls().contains(&format!("launchctl bootout {}", service_target())), "{:?}", left.calls());

        let none = Launchd::new(false, 0);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| none.run(p, a)), Ok(Removed::Nothing));
        assert!(!none.calls().iter().any(|c| c.contains("bootout")), "{:?}", none.calls());
    }

    /// 멈추기를 거절해도 등록은 걷는다(리뷰 moai-tsk0 의 220) — `disable` 이 못 지운 wants 링크도 우리 것이면 걷는다.
    #[cfg(unix)]
    #[test]
    fn a_refusal_to_stop_still_removes_the_registration() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(exe(), pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::New));
        let wants = svc.file.parent().unwrap().join("default.target.wants");
        std::fs::create_dir_all(&wants).unwrap();
        std::os::unix::fs::symlink("../the-service", wants.join("the-service")).unwrap();

        let no = Fake::new(&["disable"]);
        let got = svc.uninstall(&|p: &str, a: &[&str]| no.run(p, a));
        assert_eq!(got, Ok(Removed::File { stopped: Err("no".into()) }));
        assert!(!svc.registered(), "멈추기를 거절했다고 등록을 남겼다");
        assert!(wants.join("the-service").symlink_metadata().is_err(), "우리 유닛을 가리키는 링크가 남았다");
        assert!(no.calls().contains(&"systemctl --user daemon-reload".to_string()), "{:?}", no.calls());
    }

    #[cfg(unix)]
    #[test]
    fn only_a_wants_link_to_our_unit_is_removed() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        let wants = svc.file.parent().unwrap().join("default.target.wants");
        std::fs::create_dir_all(&wants).unwrap();
        for leave in ["other", "plain"] {
            svc.install(exe(), pin(), &|p: &str, a: &[&str]| ok.run(p, a)).unwrap();
            let link = wants.join("the-service");
            match leave {
                "other" => std::os::unix::fs::symlink("/somewhere/else/the-service", &link).unwrap(),
                _ => std::fs::write(&link, "a file").unwrap(),
            }
            assert!(matches!(svc.uninstall(&|p: &str, a: &[&str]| ok.run(p, a)), Ok(Removed::File { .. })));
            assert!(link.symlink_metadata().is_ok(), "{leave}: 우리 것이 아닌 것을 지웠다");
            std::fs::remove_file(&link).unwrap();
        }
    }

    /// 반쪽 해제를 안 남긴다(리뷰 moai-tsk0 의 221) — 못 지울 자리면 관리자를 부르기 전에 멈추고, 멈춘 뒤에 못 지웠으면
    /// 되돌린다.
    #[cfg(unix)]
    #[test]
    fn an_uninstall_that_cannot_remove_the_file_leaves_the_service_as_it_was() {
        use std::os::unix::fs::PermissionsExt as _;
        if crate::ctl::place::uid() == 0 {
            return; // root 는 권한이 안 막는다.
        }
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let dir = svc.file.parent().unwrap().to_path_buf();
        let ok = Fake::new(&[]);
        svc.install(exe(), pin(), &|p: &str, a: &[&str]| ok.run(p, a)).unwrap();
        let lock = |mode| std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();

        lock(0o500);
        let looked = Fake::new(&[]);
        let got = svc.uninstall(&|p: &str, a: &[&str]| looked.run(p, a));
        lock(0o700);
        assert!(matches!(got, Err(Trouble::File(_))), "{got:?}");
        assert!(looked.calls().is_empty(), "못 지울 자리인데 관리자를 불렀다 {:?}", looked.calls());
        assert!(svc.registered());

        // 먼저 본 뒤에 자리가 막힌다 — 멈춘 것을 되돌려야 한다.
        let calls = RefCell::new(Vec::new());
        let got = svc.uninstall(&|p: &str, a: &[&str]| {
            calls.borrow_mut().push(format!("{p} {}", a.join(" ")));
            if a.contains(&"disable") {
                lock(0o500);
            }
            Ok(String::new())
        });
        lock(0o700);
        assert!(matches!(got, Err(Trouble::File(_))), "{got:?}");
        assert!(svc.registered());
        assert_eq!(
            calls.borrow().last().map(String::as_str),
            Some("systemctl --user enable --now moai-daemon.service"),
            "멈춘 것을 안 되돌렸다 {:?}",
            calls.borrow()
        );
    }

    /// 겹친 `install`·`uninstall` 은 줄을 선다(리뷰 moai-tsk0 의 203) — 차례를 쥔 동안 다른 쪽은 파일도 관리자도 안 만진다.
    #[cfg(unix)]
    #[test]
    fn install_and_uninstall_wait_their_turn() {
        let s = Scratch::new("service-turn");
        let svc = service_in(&s, Manager::Launchd);
        let calls = std::sync::Mutex::new(Vec::<String>::new());
        let run = |p: &str, a: &[&str]| -> Result<String, String> {
            calls.lock().unwrap().push(format!("{p} {}", a.join(" ")));
            if a.first() == Some(&"print") { Err("Could not find service".into()) } else { Ok(String::new()) }
        };
        let held = svc.turn(true).unwrap();
        std::thread::scope(|t| {
            let installing = t.spawn(|| svc.install(exe(), pin(), &run));
            std::thread::sleep(Duration::from_millis(300));
            assert!(calls.lock().unwrap().is_empty(), "차례를 안 기다리고 관리자를 불렀다");
            assert!(!svc.registered(), "차례를 안 기다리고 파일을 썼다");
            drop(held);
            assert_eq!(installing.join().unwrap(), Ok(Wrote::New));
        });

        let held = svc.turn(false).unwrap();
        calls.lock().unwrap().clear();
        std::thread::scope(|t| {
            let removing = t.spawn(|| svc.uninstall(&run));
            std::thread::sleep(Duration::from_millis(300));
            assert!(calls.lock().unwrap().is_empty(), "차례를 안 기다리고 관리자를 불렀다");
            assert!(svc.registered(), "차례를 안 기다리고 파일을 지웠다");
            drop(held);
            assert_eq!(removing.join().unwrap(), Ok(Removed::File { stopped: Ok(()) }));
        });
        let left: Vec<_> =
            std::fs::read_dir(svc.file.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(left.is_empty(), "잠금이 남긴 것이 있다 {left:?}");
    }

    /// 걷을 것이 없는 `uninstall` 은 없던 디렉터리를 만들지 않는다.
    #[test]
    fn nothing_to_remove_creates_nothing() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let fake = Fake::new(&[]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| fake.run(p, a)), Ok(Removed::Nothing));
        assert!(!svc.file.parent().unwrap().exists());
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn sudo_is_read_only_when_the_effective_user_is_root() {
        let root = crate::ctl::place::uid() == 0;
        assert_eq!(under_sudo(|_| Some("me".into())), root);
        assert!(!under_sudo(|_| None));
        assert!(!under_sudo(|_| Some("".into())));
    }

    #[test]
    fn a_path_with_a_line_break_is_refused_before_anything_is_written() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let fake = Fake::new(&[]);
        let got = svc.install(Path::new("/bad\npath/moai"), pin(), &|p: &str, a: &[&str]| fake.run(p, a));
        assert_eq!(got, Err(Trouble::BadPath("/bad\npath/moai".into())));
        assert!(!svc.registered());
        assert!(fake.calls().is_empty());

        // 고칠 곳은 고정할 디렉터리다 — 바이너리를 대면 멀쩡한 경로를 탓한다.
        let got = svc.install(exe(), Path::new("/bad\ndir"), &|p: &str, a: &[&str]| fake.run(p, a));
        assert_eq!(got, Err(Trouble::BadPath("/bad\ndir".into())));
        assert!(!svc.registered());
    }

    #[cfg(unix)]
    #[test]
    fn the_cause_is_read_past_the_line_about_the_symlink() {
        let got = run("sh", &["-c", "echo 'Created symlink a -> b.' >&2; echo >&2; echo 'Job failed.' >&2; exit 1"]);
        assert_eq!(got, Err("sh: Job failed.".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_leaves_no_half_file() {
        let s = Scratch::new("service-half");
        let dir = s.path().join("user");
        std::fs::create_dir_all(dir.join("the-service")).unwrap();
        // 옮길 자리가 디렉터리라 `rename` 이 실패한다.
        let svc = service_in(&s, Manager::Systemd);
        assert!(svc.write("x").is_err());
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["the-service"], "반쪽 파일이 남았다");
    }

    /// 시험이 바이너리로 쓸 실제 파일. "같은 바이너리" 는 같은 파일로 풀리는가라서 있는 파일이어야 한다.
    fn binary(s: &Scratch, place: &str) -> PathBuf {
        let file = s.path().join(place).join("moai");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
        file
    }

    #[test]
    fn restart_needs_our_file_this_binary_and_a_running_service() {
        for manager in [Manager::Systemd, Manager::Launchd] {
            let s = Scratch::new("service");
            let svc = service_in(&s, manager);
            let (this, other) = (binary(&s, "bin"), binary(&s, "other"));
            let restart_word = if manager == Manager::Systemd { "restart" } else { "kickstart" };
            let running = if manager == Manager::Systemd { "" } else { "\tstate = running\n" };

            // 파일이 없으면 관리자를 아예 안 부른다 — 서비스가 없는 사람의 `moai update` 는 그대로다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(&this, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());

            let ok = Fake::new(&["print"]);
            svc.install(&this, pin(), &|p: &str, a: &[&str]| ok.run(p, a)).unwrap();

            // 실행 중이면 부른다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(svc.restart(&this, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().iter().any(|c| c.contains(restart_word)), "{:?}", fake.calls());

            // 다른 바이너리면 안 부른다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(&other, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());

            // 사람이 멈춘 서비스는 안 띄운다.
            let stopped_words: &'static [&'static str] = if manager == Manager::Systemd { &["is-active"] } else { &[] };
            let fake = Fake { calls: RefCell::default(), refuse: stopped_words, said: "\tstate = not running\n" };
            assert!(!svc.restart(&this, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(!fake.calls().iter().any(|c| c.contains(restart_word)), "{:?}", fake.calls());

            // 다시 시작이 실패하면 거짓을 답한다.
            let refused: &'static [&'static str] =
                if manager == Manager::Systemd { &["restart"] } else { &["kickstart"] };
            let fake = Fake { calls: RefCell::default(), refuse: refused, said: running };
            assert!(!svc.restart(&this, &|p: &str, a: &[&str]| fake.run(p, a)));

            // 적힌 바이너리가 사라졌으면 그 등록은 아무것도 못 띄운다 — 관리자를 안 부른다.
            std::fs::remove_file(&this).unwrap();
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(&other, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());
            std::fs::write(&this, "").unwrap();

            // 손으로 쓴 파일이면 안 부른다.
            let t = this.display();
            std::fs::write(&svc.file, format!("ExecStart=\"{t}\" daemon start\n<string>{t}</string>\n")).unwrap();
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(&this, &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());
        }
    }

    /// 적힌 경로를 읽는 것은 쓴 것의 짝이다 — 싼 글자가 든 경로도 같은 경로로 돌아와야 같은 바이너리로 읽힌다.
    #[test]
    fn the_written_binary_is_read_back_through_the_escaping() {
        let weird = "/my dir/50%/$HOME/a\"b\\c/&<x>/moai";
        for manager in [Manager::Systemd, Manager::Launchd] {
            let text = manager.render(weird, PIN);
            assert_eq!(manager.recorded(&text).as_deref(), Some(weird), "{manager:?}\n{text}");
        }
        // 손으로 고쳐 못 읽는 꼴은 없다 — 다른 바이너리로 읽혀 덮이지 않는다.
        assert_eq!(Manager::Systemd.recorded("ExecStart=/usr/bin/moai daemon start\n"), None);
        assert_eq!(Manager::Systemd.recorded("ExecStart=\"/a%b\" daemon start\n"), None);
        assert_eq!(Manager::Systemd.recorded("ExecStart=\"/usr/bin/moai\" daemon start --x\n"), None);
        assert_eq!(Manager::Launchd.recorded("<plist></plist>"), None);
    }

    /// "같은 바이너리" 는 같은 파일로 풀리는가다(리뷰 moai-tsk0 의 441) — 링크 경로를 적은 파일도, 푼 경로를 적은 옛 파일도
    /// 이 바이너리로 읽히고, 사라진 경로는 다른 바이너리가 아니다.
    #[cfg(unix)]
    #[test]
    fn the_same_binary_is_the_same_file_not_the_same_spelling() {
        let s = Scratch::new("service-points");
        let real = binary(&s, "Cellar/1.2.0/bin");
        let other = binary(&s, "elsewhere");
        let link = s.path().join("bin").join("moai");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        for manager in [Manager::Systemd, Manager::Launchd] {
            let text = |exe: &Path| manager.render(exe.to_str().unwrap(), PIN);
            assert_eq!(points(manager, &text(&link), &real), Points::This, "{manager:?}");
            assert_eq!(points(manager, &text(&real), &link), Points::This, "{manager:?}");
            assert_eq!(points(manager, &text(&other), &real), Points::Other, "{manager:?}");
            assert_eq!(points(manager, &text(Path::new("/gone/moai")), &real), Points::Gone, "{manager:?}");
            assert_eq!(points(manager, "# no exec line", &real), Points::Other, "{manager:?}");
        }
        let dangling = s.path().join("dangling").join("moai");
        std::fs::create_dir_all(dangling.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(s.path().join("nowhere"), &dangling).unwrap();
        let unit = Manager::Systemd.render(dangling.to_str().unwrap(), PIN);
        assert_eq!(points(Manager::Systemd, &unit, &real), Points::Gone, "끊긴 링크는 사라진 경로다");

        // update 는 푼 경로를 넘긴다 — 링크 경로를 적은 우리 파일이어도 다시 시작한다.
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(&link, pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::New));
        assert!(svc.restart(&real, &|p: &str, a: &[&str]| ok.run(p, a)));

        // 푼 경로를 적은 옛 꼴은 이 바이너리로 읽혀 링크 경로로 다시 쓰이고, 사라진 경로를 적은 파일도 다시 쓰인다.
        let old = Manager::Systemd.render(real.to_str().unwrap(), PIN);
        std::fs::write(&svc.file, &old).unwrap();
        assert_eq!(svc.install(&link, pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::Rewritten));
        assert!(std::fs::read_to_string(&svc.file).unwrap().contains(&format!("\"{}\"", link.display())));
        let gone = Manager::Systemd.render("/gone/moai", PIN);
        std::fs::write(&svc.file, &gone).unwrap();
        assert_eq!(svc.install(&link, pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Ok(Wrote::Rewritten));

        // 다른 바이너리와 못 읽는 꼴은 덮지 않는다.
        let theirs = Manager::Systemd.render(other.to_str().unwrap(), PIN);
        std::fs::write(&svc.file, &theirs).unwrap();
        assert_eq!(svc.install(&link, pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Err(Trouble::OtherBinary));
        assert_eq!(std::fs::read_to_string(&svc.file).unwrap(), theirs);
        let hand = format!("# {MARK}\n[Service]\nExecStart={} daemon start\n", real.display());
        std::fs::write(&svc.file, &hand).unwrap();
        assert_eq!(svc.install(&link, pin(), &|p: &str, a: &[&str]| ok.run(p, a)), Err(Trouble::OtherBinary));
    }

    #[test]
    fn the_service_pid_is_read_from_the_manager() {
        let s = Scratch::new("service");
        let systemd = service_in(&s, Manager::Systemd);
        for (said, want) in [("4242\n", Ok(Some(4242))), ("0\n", Ok(None)), ("", Ok(None)), ("x", Ok(None))] {
            let calls = RefCell::new(Vec::new());
            let got = systemd.main_pid(&|p: &str, a: &[&str]| {
                calls.borrow_mut().push(format!("{p} {}", a.join(" ")));
                Ok(said.to_string())
            });
            assert_eq!(got, want, "{said:?}");
            assert_eq!(*calls.borrow(), ["systemctl --user show -p MainPID --value moai-daemon.service"]);
        }
        // 관리자에 못 닿은 것은 "안 돈다" 가 아니다.
        assert_eq!(systemd.main_pid(&|_: &str, _: &[&str]| Err("no".to_string())), Err("no".to_string()));
        let launchd = service_in(&s, Manager::Launchd);
        let said = "gui/501/com.buzzni.moai-daemon = {\n\tstate = running\n\tpid = 777\n}\n";
        assert_eq!(launchd.main_pid(&|_: &str, _: &[&str]| Ok(said.to_string())), Ok(Some(777)));
        assert_eq!(launchd.main_pid(&|_: &str, _: &[&str]| Ok("\tstate = not running\n".to_string())), Ok(None));
        assert_eq!(launchd.main_pid(&|_: &str, _: &[&str]| Err("Could not find service".to_string())), Ok(None));
    }
}
