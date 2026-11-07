//! 데몬을 사용자의 서비스 관리자에 맡긴다 — `moai daemon install·uninstall`(moai-upq5).
//!
//! - **등록 상태는 서비스 파일 유무다.** 저장하는 값을 따로 두지 않는다. 그래서 등록에 실패하면 이번에 쓴 파일을
//!   치운다 — 파일만 남으면 "등록됐다" 고 거짓말하는 상태가 된다.
//! - **표지([`MARK`])가 없는 파일은 사람의 것이다.** `docs/daemon.md` 를 보고 같은 자리에 손으로 쓴 unit 일 수 있다.
//!   덮지도 지우지도 않는다.
//! - **다른 바이너리를 가리키는 파일도 덮지 않는다.** 한 사용자에 데몬은 하나이고, 어느 설치본이 그것을 맡을지는
//!   사람이 정한다. 둘째 설치가 첫째의 서비스를 말없이 빼앗으면 안 된다.
//! - **관리자는 PATH 에서 찾는다.** 시험은 가짜 `systemctl`·`launchctl` 만 있는 PATH 로 돌아 사람의 서비스에 닿지 않는다.
//!
//! 명령을 부르는 자리는 [`Run`] 하나로 받는다 — launchd 의 글과 차례를 Linux 에서도 잴 수 있게.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const UNIT: &str = "moai-daemon.service";
const LABEL: &str = "com.buzzni.moai-daemon";
/// moai 가 쓴 파일의 표지. 이 글이 없으면 사람의 파일로 읽는다.
const MARK: &str = "moai daemon install wrote this file. moai daemon uninstall removes it.";

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

    /// 서비스가 셸과 같은 소켓을 찾도록 고정할 디렉터리. launchd 는 셸의 `TMPDIR` 을 안 믿으니 늘 고정하고, systemd 는
    /// 사람이 `MOAI_CTL_DIR` 을 준 때만 고정한다 — 사용자 관리자가 `XDG_RUNTIME_DIR` 을 셸과 똑같이 준다.
    pub fn pin(self, dir: &Path, env: impl Fn(&str) -> Option<OsString>) -> Option<&Path> {
        match self {
            Manager::Launchd => Some(dir),
            Manager::Systemd => env("MOAI_CTL_DIR").map(|_| dir),
        }
    }

    fn render(self, exe: &str, pin: Option<&str>) -> String {
        match self {
            Manager::Systemd => {
                let env = pin
                    .map(|dir| format!("Environment=\"MOAI_CTL_DIR={}\"\n", unit_escape(dir, false)))
                    .unwrap_or_default();
                format!(
                    "# {MARK}\n[Unit]\nDescription=moai daemon\n\n[Service]\n{env}{}\nRestart=on-failure\nKillMode=mixed\n\n[Install]\nWantedBy=default.target\n",
                    exec_line(exe)
                )
            }
            Manager::Launchd => {
                let env = pin
                    .map(|dir| {
                        format!(
                            "    <key>EnvironmentVariables</key>\n    <dict><key>MOAI_CTL_DIR</key><string>{}</string></dict>\n",
                            xml_escape(dir)
                        )
                    })
                    .unwrap_or_default();
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<!-- {MARK} -->\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key><string>{LABEL}</string>\n    <key>ProgramArguments</key>\n    <array>{}<string>daemon</string><string>start</string></array>\n{env}    <key>RunAtLoad</key><true/>\n    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n</dict>\n</plist>\n",
                    program_string(exe)
                )
            }
        }
    }

    /// 이 파일이 이 바이너리를 띄우는가.
    fn names(self, text: &str, exe: &str) -> bool {
        match self {
            Manager::Systemd => text.lines().any(|line| line == exec_line(exe)),
            Manager::Launchd => text.contains(&program_string(exe)),
        }
    }
}

fn exec_line(exe: &str) -> String {
    format!("ExecStart=\"{}\" daemon start", unit_escape(exe, true))
}

fn program_string(exe: &str) -> String {
    format!("<string>{}</string>", xml_escape(exe))
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

    /// 등록하고 지금 시작한다. 이번에 파일을 썼으면 `true`, 이미 등록돼 있었으면 `false`.
    pub fn install(&self, exe: &Path, pin: Option<&Path>, run: Run) -> Result<bool, Trouble> {
        let exe = writable_text(exe)?;
        let pin = pin.map(writable_text).transpose()?;
        let wrote = match self.text()? {
            Some(text) if !text.contains(MARK) => return Err(Trouble::NotOurs),
            Some(text) if !self.manager.names(&text, exe) => return Err(Trouble::OtherBinary),
            Some(_) => false,
            None => {
                self.write(&self.manager.render(exe, pin)).map_err(|e| Trouble::File(e.to_string()))?;
                true
            }
        };
        if let Err(said) = self.start(run) {
            // 파일 유무가 곧 상태다. 이번에 쓴 것은 걷는다 — 전부터 있던 등록은 그대로 둔다.
            if wrote {
                let _ = self.stop(run);
                let _ = std::fs::remove_file(&self.file);
                self.reload(run);
            }
            return Err(Trouble::Refused(said));
        }
        Ok(wrote)
    }

    /// 멈추고 파일을 지운다. 지웠으면 `true`, 파일이 없었으면 `false`.
    pub fn uninstall(&self, run: Run) -> Result<bool, Trouble> {
        match self.text()? {
            None => return Ok(false),
            Some(text) if !text.contains(MARK) => return Err(Trouble::NotOurs),
            Some(_) => {}
        }
        self.stop(run).map_err(Trouble::Refused)?;
        std::fs::remove_file(&self.file).map_err(|e| Trouble::File(e.to_string()))?;
        self.reload(run);
        Ok(true)
    }

    /// `moai update` 가 부른다. **moai 가 쓴 파일이고, 이 바이너리를 가리키고, 지금 실행 중일 때만** 다시 시작한다. 사람이 멈춘
    /// 서비스는 안 띄운다. 등록 상태를 바꾸는 호출은 이 길에 없다.
    pub fn restart(&self, exe: &Path, run: Run) -> bool {
        let (Ok(exe), Ok(Some(text))) = (writable_text(exe), self.text()) else {
            return false;
        };
        if !text.contains(MARK) || !self.manager.names(&text, exe) || !self.active(run) {
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
            Manager::Launchd => run("launchctl", &["print", &service_target()]).is_ok_and(|said| runs(&said)),
        }
    }

    fn start(&self, run: Run) -> Result<(), String> {
        match self.manager {
            Manager::Systemd => {
                run("systemctl", &["--user", "daemon-reload"])?;
                run("systemctl", &["--user", "enable", "--now", UNIT])?;
            }
            Manager::Launchd => match run("launchctl", &["print", &service_target()]) {
                Err(_) => {
                    let file = self.file.to_string_lossy();
                    run("launchctl", &["bootstrap", &domain(), &file])?;
                }
                // 이미 올라간 서비스의 `bootstrap` 은 실패한다. 다만 깨끗이 끝난 서비스는 launchd 가 다시 안 띄우니
                // 여기서 띄운다 — `enable --now` 가 멈춘 서비스를 띄우는 것과 같게.
                Ok(said) if !runs(&said) => {
                    run("launchctl", &["kickstart", &service_target()])?;
                }
                Ok(_) => {}
            },
        }
        Ok(())
    }

    fn stop(&self, run: Run) -> Result<(), String> {
        match self.manager {
            Manager::Systemd => run("systemctl", &["--user", "disable", "--now", UNIT]).map(|_| ()),
            Manager::Launchd => {
                if run("launchctl", &["print", &service_target()]).is_ok() {
                    let file = self.file.to_string_lossy();
                    run("launchctl", &["bootout", &domain(), &file])?;
                }
                Ok(())
            }
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
    use std::cell::RefCell;

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

    const EXE: &str = "/home/me/.local/bin/moai";

    #[test]
    fn the_unit_is_the_one_the_manual_taught() {
        let text = Manager::Systemd.render(EXE, None);
        assert!(text.contains(MARK));
        assert!(text.contains("ExecStart=\"/home/me/.local/bin/moai\" daemon start\n"), "{text}");
        assert!(text.contains("Restart=on-failure\n"));
        assert!(text.contains("KillMode=mixed\n"));
        assert!(text.contains("WantedBy=default.target\n"));
        assert!(!text.contains("Environment"), "고정할 것이 없으면 환경을 안 적는다\n{text}");
    }

    #[test]
    fn a_unit_quotes_what_systemd_would_read_again() {
        let text = Manager::Systemd.render("/my dir/50%/$HOME/a\"b\\c/moai", Some("/run/my 100%/$x"));
        assert!(text.contains(r#"ExecStart="/my dir/50%%/$$HOME/a\"b\\c/moai" daemon start"#), "{text}");
        // `Environment=` 는 `$` 를 안 펼친다 — 두 겹으로 싸면 다른 경로가 된다.
        assert!(text.contains("Environment=\"MOAI_CTL_DIR=/run/my 100%%/$x\"\n"), "{text}");
    }

    #[test]
    fn a_plist_pins_the_socket_directory_and_escapes_xml() {
        let text = Manager::Launchd.render("/Users/a&b/<moai>", Some("/var/folders/x&y/T/moai-501"));
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
    fn systemd_only_pins_a_directory_the_person_named() {
        let dir = Path::new("/d");
        assert_eq!(Manager::Systemd.pin(dir, |_| None), None);
        assert_eq!(Manager::Systemd.pin(dir, |_| Some("/d".into())), Some(dir));
        assert_eq!(Manager::Launchd.pin(dir, |_| None), Some(dir));
    }

    #[test]
    fn a_failed_registration_leaves_no_file_and_an_old_one_stays() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let fake = Fake::new(&["enable"]);
        let run = |p: &str, a: &[&str]| fake.run(p, a);
        let got = svc.install(Path::new(EXE), None, &run);
        assert_eq!(got, Err(Trouble::Refused("no".into())));
        assert!(!svc.registered(), "실패했는데 파일이 남았다");

        // 전부터 있던 등록은 실패해도 안 지운다 — 관리자가 잠시 안 답했다고 등록을 잃으면 안 된다.
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| ok.run(p, a)), Ok(true));
        assert_eq!(svc.install(Path::new(EXE), None, &run), Err(Trouble::Refused("no".into())));
        assert!(svc.registered(), "전부터 있던 등록을 지웠다");
    }

    #[test]
    fn launchd_loads_once_and_unloads_only_what_is_loaded() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Launchd);
        let file = svc.file.display().to_string();
        let domain = domain();

        let unloaded = Fake::new(&["print"]);
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| unloaded.run(p, a)), Ok(true));
        assert_eq!(
            unloaded.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl bootstrap {domain} {file}")]
        );

        let loaded = Fake { calls: RefCell::default(), refuse: &[], said: "\tstate = running\n" };
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| loaded.run(p, a)), Ok(false));
        assert_eq!(loaded.calls(), [format!("launchctl print {domain}/{LABEL}")], "이미 올라간 것을 또 올렸다");

        // 올라가 있어도 멈춰 있으면 지금 띄운다 — `moai daemon stop` 의 깨끗한 종료는 launchd 가 다시 안 띄운다.
        let stopped = Fake { calls: RefCell::default(), refuse: &[], said: "\tstate = not running\n" };
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| stopped.run(p, a)), Ok(false));
        assert_eq!(
            stopped.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl kickstart {domain}/{LABEL}")]
        );

        // 안 올라가 있으면 `bootout` 을 안 부른다 — 부르면 실패해 파일을 못 지운다.
        let gone = Fake::new(&["print"]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| gone.run(p, a)), Ok(true));
        assert_eq!(gone.calls(), [format!("launchctl print {domain}/{LABEL}")]);
        assert!(!svc.registered());

        let unloaded = Fake::new(&["print"]);
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| unloaded.run(p, a)), Ok(true));
        let loaded = Fake::new(&[]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| loaded.run(p, a)), Ok(true));
        assert_eq!(
            loaded.calls(),
            [format!("launchctl print {domain}/{LABEL}"), format!("launchctl bootout {domain} {file}")]
        );
    }

    #[test]
    fn a_refusal_to_stop_keeps_the_registration() {
        let s = Scratch::new("service");
        let svc = service_in(&s, Manager::Systemd);
        let ok = Fake::new(&[]);
        assert_eq!(svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| ok.run(p, a)), Ok(true));
        let no = Fake::new(&["disable"]);
        assert_eq!(svc.uninstall(&|p: &str, a: &[&str]| no.run(p, a)), Err(Trouble::Refused("no".into())));
        assert!(svc.registered(), "멈추지 못했는데 파일을 지웠다");
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
        let got = svc.install(Path::new("/bad\npath/moai"), None, &|p: &str, a: &[&str]| fake.run(p, a));
        assert_eq!(got, Err(Trouble::BadPath("/bad\npath/moai".into())));
        assert!(!svc.registered());
        assert!(fake.calls().is_empty());

        // 고칠 곳은 고정할 디렉터리다 — 바이너리를 대면 멀쩡한 경로를 탓한다.
        let got = svc.install(Path::new(EXE), Some(Path::new("/bad\ndir")), &|p: &str, a: &[&str]| fake.run(p, a));
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

    #[test]
    fn restart_needs_our_file_this_binary_and_a_running_service() {
        for manager in [Manager::Systemd, Manager::Launchd] {
            let s = Scratch::new("service");
            let svc = service_in(&s, manager);
            let restart_word = if manager == Manager::Systemd { "restart" } else { "kickstart" };
            let running = if manager == Manager::Systemd { "" } else { "\tstate = running\n" };

            // 파일이 없으면 관리자를 아예 안 부른다 — 서비스가 없는 사람의 `moai update` 는 그대로다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(Path::new(EXE), &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());

            let ok = Fake::new(&["print"]);
            svc.install(Path::new(EXE), None, &|p: &str, a: &[&str]| ok.run(p, a)).unwrap();

            // 실행 중이면 부른다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(svc.restart(Path::new(EXE), &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().iter().any(|c| c.contains(restart_word)), "{:?}", fake.calls());

            // 다른 바이너리면 안 부른다.
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(Path::new("/other/moai"), &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());

            // 사람이 멈춘 서비스는 안 띄운다.
            let stopped_words: &'static [&'static str] = if manager == Manager::Systemd { &["is-active"] } else { &[] };
            let fake = Fake { calls: RefCell::default(), refuse: stopped_words, said: "\tstate = not running\n" };
            assert!(!svc.restart(Path::new(EXE), &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(!fake.calls().iter().any(|c| c.contains(restart_word)), "{:?}", fake.calls());

            // 다시 시작이 실패하면 거짓을 답한다.
            let refused: &'static [&'static str] =
                if manager == Manager::Systemd { &["restart"] } else { &["kickstart"] };
            let fake = Fake { calls: RefCell::default(), refuse: refused, said: running };
            assert!(!svc.restart(Path::new(EXE), &|p: &str, a: &[&str]| fake.run(p, a)));

            // 손으로 쓴 파일이면 안 부른다.
            std::fs::write(&svc.file, format!("ExecStart=\"{EXE}\" daemon start\n<string>{EXE}</string>\n")).unwrap();
            let fake = Fake { calls: RefCell::default(), refuse: &[], said: running };
            assert!(!svc.restart(Path::new(EXE), &|p: &str, a: &[&str]| fake.run(p, a)));
            assert!(fake.calls().is_empty(), "{:?}", fake.calls());
        }
    }
}
