//! `moai update` 가 무엇을 돌릴지 고른다(moai-zsfr.2p9, 사람 결정 2026-10-08).
//!
//! 사람이 치던 한 줄 — `curl -fsSL …/install.sh | sh -s -- --force` — 을 명령 하나로 옮긴다. 받는
//! 저장소는 [`crate::latest::repo_from`] 이 고르고, 스크립트는 그 저장소 `main` 의 `install.sh` 다.
//! 그것을 **도는 바이너리의 자리**에 `--dir <자리> --force` 로 돌린다 — `~/.local/bin` 에 박힌 줄은
//! 다른 자리에 깐 moai 를 못 올렸다(`latest::upgrade_line` 이 자리를 보는 까닭과 같다).
//!
//! **Windows 에서는 `install.ps1` 을 PowerShell 로 돌린다**(moai-j9gv.3fl, 사람 결정 2026-10-12) —
//! 거절에 그치지 않고 실제로 올린다. 무엇으로 돌리는지는 [`Runner`] 하나가 고르고, 받을 스크립트의
//! 이름·깔리는 바이너리의 이름·인자의 꼴이 다 그것을 따른다. 리눅스에서도 두 갈래를 다 잴 수 있게
//! `cfg` 는 [`Runner::native`] 한 자리에만 선다.
//!
//! **여기는 고르기만 한다.** 받고 돌리는 것은 `cmd::update` 다 — 고르는 길을 시험이 그물 없이 재려면
//! 둘이 갈려 있어야 한다.
//!
//! **못 올리는 자리는 그물을 타기 전에 멈춘다**([`refusal`], 사람 결정 2026-10-08). 받아 놓고 거절하면
//! 사람은 "이미 받았으니" 하고 넘어가고(`install.sh` 가 깔 자리를 받기 전에 보는 까닭과 같다), 남는
//! 것은 쓸데없이 두드린 GitHub 한 번이다.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// 스크립트를 받을 자리를 바꾸는 환경변수 — **시험이 제 서버를 띄워 붙는 자리다.** `MOAI_API_URL` 이
/// 판 묻기에 하는 일을 이쪽에 한다. 거울을 쓰는 사람도 쓸 수 있지만, 이 값은 셸에 흘릴 글을
/// 고르므로 사람 설정에는 안 둔다 — 그 자리는 저장소 이름(`[update] repo`)까지다.
pub const SCRIPT_VAR: &str = "MOAI_INSTALL_URL";

/// 받은 스크립트를 무엇으로 돌리나 — **설치 스크립트 하나에 셸 하나다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runner {
    /// `install.sh` 를 `sh -s --` 의 표준 입력으로 흘린다 — 사람이 치던 `curl … | sh -s --` 의 뒷반이다.
    Sh,
    /// `install.ps1` 을 `file` 에 적고 `powershell -File` 로 돌린다. **파일이어야 한다** — 표준 입력으로
    /// 흘린 스크립트는 `param()` 을 못 받아 `-Dir`·`-Force` 가 안 서고, `iex` 로 돌리면 스크립트의
    /// `exit` 가 이 프로세스가 띄운 PowerShell 이 아니라 그 안의 흐름을 끊는다.
    ///
    /// `powershell` 은 Windows PowerShell 5.1 이다 — 모든 Windows 에 서 있다. pwsh 7 은 깔아야 선다.
    PowerShell { file: PathBuf },
}

impl Runner {
    /// 이 기계의 것. Windows 면 스크립트를 `temp`(보통 `std::env::temp_dir()`) 밑에 프로세스 번호로 이름
    /// 지은 파일에 적는다 — `--dry-run` 이 찍는 줄에도 그 자리가 그대로 선다.
    pub fn native(temp: &Path) -> Runner {
        if cfg!(windows) {
            Runner::PowerShell { file: temp.join(format!("moai-update-{}.ps1", std::process::id())) }
        } else {
            Runner::Sh
        }
    }

    /// 받을 스크립트의 이름 — 저장소 뿌리의 파일이다.
    pub fn script(&self) -> &'static str {
        match self {
            Runner::Sh => "install.sh",
            Runner::PowerShell { .. } => "install.ps1",
        }
    }

    /// 그 스크립트가 깔 자리에 놓는 바이너리의 이름. 도는 바이너리가 이 이름이 아니면 덮이지 않고
    /// 곁에 하나가 더 선다([`Refusal::NotNamedMoai`]).
    pub fn binary(&self) -> &'static str {
        match self {
            Runner::Sh => "moai",
            Runner::PowerShell { .. } => "moai.exe",
        }
    }

    /// 깔 자리를 주는 플래그 — 거절문이 "이 스크립트로 다시 깔라" 고 댈 때 쓴다.
    pub fn dir_flag(&self) -> &'static str {
        match self {
            Runner::Sh => "--dir",
            Runner::PowerShell { .. } => "-Dir",
        }
    }
}

/// 받을 스크립트의 자리 — [`SCRIPT_VAR`] 가 있으면 그것, 없으면 `repo` 의 `main` 의 그 스크립트
/// ([`Runner::script`]).
pub fn script_url(env: impl Fn(&str) -> Option<OsString>, repo: &str, runner: &Runner) -> String {
    env(SCRIPT_VAR)
        .filter(|v| !v.is_empty())
        .and_then(|v| v.into_string().ok())
        .unwrap_or_else(|| crate::latest::script_named(repo, runner.script()))
}

/// 못 올리는 까닭 — **말이 아니라 자료다**. 펴는 것은 `view::update_refused` 다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// 설치 스크립트가 판을 안 내는 기계([`crate::latest::SERVED`]) — 받아 봐야 "맞는 판이 없다" 로 끝난다.
    NotServed,
    /// 이름이 [`Runner::binary`] 가 아니다. 설치 스크립트는 `<자리>/<그 이름>` 으로 까므로, 돌리면 이
    /// 바이너리는 그대로고 곁에 하나가 더 선다.
    NotNamedMoai { name: String },
    /// cargo 가 지은 자리([`crate::latest::built_by_cargo`]) — 소스로 다시 짓는 쪽이 올린다. 덮으면
    /// 다음 `cargo build` 가 말없이 도로 덮거나, `cargo install` 의 장부가 거짓이 된다.
    BuiltByCargo { dir: PathBuf },
    /// 그 자리에 쓸 수 없다 — root 가 깐 `/usr/local/bin` 따위. **집 밖이라는 것만으로는 안 막는다**
    /// (사람 결정 2026-10-08) — 쓸 수 있으면 그 사람이 깐 자리다.
    NotWritable { dir: PathBuf },
}

/// `exe`(도는 바이너리의 **푼** 자리)를 올릴 수 있는가 — 없으면 그 까닭. `None` 이면 올린다. 바이너리의
/// 이름은 이 기계의 것([`Runner::native`])으로 잰다.
///
/// **차례가 곧 대는 까닭이다.** 판을 안 내는 기계에서는 다른 무엇을 고쳐도 소용없으므로 그것부터,
/// cargo 가 지은 자리는 쓸 수 있어도 덮으면 안 되므로 쓰기보다 먼저다.
///
/// `writable` 을 받는 것은 시험이 권한을 꾸미지 않고 갈래를 재려는 것이다 — root 로 도는 시험
/// 기계에서는 `chmod` 가 아무것도 안 막는다.
pub fn refusal(exe: &Path, served: bool, writable: impl Fn(&Path) -> bool) -> Option<Refusal> {
    refusal_named(exe, served, Runner::native(Path::new("")).binary(), writable)
}

/// [`refusal`] 이되 바이너리의 이름(`want`)을 받는다 — 리눅스에서 Windows 의 이름(`moai.exe`)을 잰다.
pub fn refusal_named(exe: &Path, served: bool, want: &str, writable: impl Fn(&Path) -> bool) -> Option<Refusal> {
    if !served {
        return Some(Refusal::NotServed);
    }
    let name = exe.file_name().unwrap_or_default();
    if name != OsStr::new(want) {
        return Some(Refusal::NotNamedMoai { name: name.to_string_lossy().into_owned() });
    }
    let dir = exe.parent().unwrap_or(Path::new("/"));
    if crate::latest::built_by_cargo(dir) {
        return Some(Refusal::BuiltByCargo { dir: dir.to_path_buf() });
    }
    if !writable(dir) {
        return Some(Refusal::NotWritable { dir: dir.to_path_buf() });
    }
    None
}

/// 이 프로세스가 `dir` 에 파일을 만들고 바꿀 수 있는가 — `access(2)` 의 `W_OK` 로 묻는다. **만들어
/// 보지 않는다**: 묻는 쪽(탐색기를 여는 걸음)은 아무것도 안 쓰는 자리다.
///
/// **unix 밖에서는 읽기 전용 표시만 본다.** Windows 의 디렉터리에서 그 표시는 거의 안 서서,
/// `C:\Program Files` 처럼 ACL 이 막는 자리도 참이 된다 — 그때는 `install.ps1` 이 받기 전에 제
/// 손으로 써 보고 "쓸 수 없다" 로 멈춘다(moai-j9gv.3fl). 여기서 ACL 을 읽으려면 Win32 보안 API 를
/// 끌어와야 하고, 써 보는 길은 위의 "만들어 보지 않는다" 를 깬다.
pub fn writable(dir: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(spelt) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else { return false };
        // SAFETY: `spelt` 는 이 줄이 끝날 때까지 살아 있는 널로 끝나는 버퍼고, `access` 는 그것을
        // 읽기만 한다.
        unsafe { libc::access(spelt.as_ptr(), libc::W_OK) == 0 }
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(dir).is_ok_and(|m| m.is_dir() && !m.permissions().readonly())
    }
}

/// 돌릴 것 하나 — 받을 자리와 셸에 줄 인자. **`--dry-run` 이 찍는 것과 실제로 돌리는 것이 이 값
/// 하나에서 나온다** — 둘이 따로 지어지면 찍은 줄과 돈 줄이 갈리는 날이 온다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 받을 저장소 — 설치 스크립트에 `MOAI_REPO` 로 넘긴다. 안 넘기면 포크의 스크립트가 본가의
    /// 바이너리를 받는다.
    pub repo: String,
    /// 스크립트의 자리.
    pub script: String,
    /// 깔 자리 — 도는 바이너리가 선 디렉터리.
    pub dir: PathBuf,
    /// 무엇으로 돌리나.
    pub runner: Runner,
    /// 띄울 프로그램 — `sh` 나 `powershell`.
    pub program: &'static str,
    /// 프로그램 뒤의 인자. `sh` 면 `-s --` 로 시작한다 — 스크립트는 표준 입력으로 흘린다.
    pub args: Vec<OsString>,
}

/// [`Plan`] 을 짓는다. **`--force` 는 늘 단다**(사람 결정 2026-10-08) — 사람이 치던 줄이 그랬고,
/// 그래야 같은 판을 다시 받는 것도 이 명령 하나로 된다. 판(`version`)은 스크립트가 아니라 스크립트의
/// 인자로 고른다 — 스크립트는 늘 `main` 의 것이다.
pub fn plan(repo: &str, script: String, dir: &Path, version: Option<&str>, runner: Runner) -> Plan {
    let (program, mut args): (&'static str, Vec<OsString>) = match &runner {
        Runner::Sh => {
            let mut args: Vec<OsString> = ["-s", "--", "--dir"].map(OsString::from).into();
            args.push(dir.as_os_str().to_owned());
            args.push("--force".into());
            ("sh", args)
        }
        Runner::PowerShell { file } => {
            // **사람의 프로필을 안 읽고, 묻지도 않는다.** 프로필은 남의 코드를 이 길에 끼우고, 실행
            // 정책(기본 Restricted)은 받은 스크립트를 이 프로세스 하나에서만 돌게 연다 — 기계의 정책은 안
            // 바꾼다.
            let mut args: Vec<OsString> =
                ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"].map(OsString::from).into();
            args.push(file.as_os_str().to_owned());
            args.push("-Dir".into());
            args.push(plain(dir).into_os_string());
            args.push("-Force".into());
            ("powershell", args)
        }
    };
    if let Some(v) = version {
        args.push(match runner {
            Runner::Sh => "--version".into(),
            Runner::PowerShell { .. } => "-Version".into(),
        });
        args.push(v.into());
    }
    Plan { repo: repo.to_string(), script, dir: dir.to_path_buf(), runner, program, args }
}

/// Windows 의 푼 경로에서 `\\?\` 접두어를 걷는다 — `canonicalize` 가 붙이는 것인데, Windows
/// PowerShell 5.1 의 `New-Item`·`Test-Path` 가 그 꼴을 제대로 못 읽는다. `\\?\UNC\서버\…` 는
/// `\\서버\…` 로 되돌린다. 그 밖의 철자는 그대로다 — 글자만 보는 것이라 리눅스에서도 잰다.
fn plain(dir: &Path) -> PathBuf {
    let spelt = dir.to_string_lossy();
    if let Some(rest) = spelt.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match spelt.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => dir.to_path_buf(),
    }
}

/// PowerShell 이 도로 그 낱말 하나로 읽게 싼다 — 작은따옴표 안에서는 `''` 만 가른다. 싸지 않아도
/// 되는 낱말은 그대로 둔다(`install.ps1` 의 `Quote` 와 같은 자).
fn ps_quoted(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_./:\\@%+=,-".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "''"))
}

impl Plan {
    /// 사람이 셸에 그대로 칠 수 있는 꼴의 한 줄 — `sh` 면 `MOAI_REPO=<저장소> sh -s -- --dir <자리> --force …`,
    /// PowerShell 이면 `$env:MOAI_REPO='<저장소>'; powershell … -File <자리> -Dir <자리> -Force …`. 그 셸이
    /// 가르는 글자는 싼다.
    pub fn line(&self) -> String {
        match self.runner {
            Runner::Sh => {
                let words: Vec<String> = self.args.iter().map(|a| crate::text::quoted(&a.to_string_lossy())).collect();
                format!("{}={} sh {}", crate::latest::REPO_VAR, crate::text::quoted(&self.repo), words.join(" "))
            }
            Runner::PowerShell { .. } => {
                let words: Vec<String> = self.args.iter().map(|a| ps_quoted(&a.to_string_lossy())).collect();
                format!(
                    "$env:{}={}; {} {}",
                    crate::latest::REPO_VAR,
                    ps_quoted(&self.repo),
                    self.program,
                    words.join(" ")
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yes(_: &Path) -> bool {
        true
    }

    fn ps(file: &str) -> Runner {
        Runner::PowerShell { file: PathBuf::from(file) }
    }

    /// **못 올리는 자리는 그물 전에 까닭 하나로 멈춘다**(사람 결정 2026-10-08). 차례도 잰다 — 판을 안
    /// 내는 기계는 무엇이 서든 그것이 먼저고, cargo 가 지은 자리는 쓸 수 있어도 막는다.
    #[test]
    fn it_refuses_where_install_sh_cannot_upgrade() {
        let s = crate::scratch::Scratch::new("update-refusal");
        let plain = s.path().join("bin");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(refusal(&plain.join("moai"), true, yes), None, "쓸 수 있는 자리");
        assert_eq!(refusal(Path::new("/opt/tools/moai"), true, yes), None, "집 밖이어도 쓸 수 있으면 올린다");
        assert_eq!(refusal(&plain.join("moai"), false, yes), Some(Refusal::NotServed));
        assert_eq!(
            refusal(&plain.join("moai-dev"), true, yes),
            Some(Refusal::NotNamedMoai { name: "moai-dev".into() })
        );
        assert_eq!(refusal(&plain.join("moai"), true, |_| false), Some(Refusal::NotWritable { dir: plain.clone() }));

        let profile = s.path().join("target").join("release");
        std::fs::create_dir_all(profile.join(".fingerprint")).unwrap();
        assert_eq!(
            refusal(&profile.join("moai"), true, yes),
            Some(Refusal::BuiltByCargo { dir: profile.clone() }),
            "소스에서 지은 판"
        );
        assert_eq!(refusal(&profile.join("moai"), false, yes), Some(Refusal::NotServed), "판을 안 내는 기계가 먼저다");
        let cargo = s.path().join(".cargo");
        std::fs::create_dir_all(cargo.join("bin")).unwrap();
        std::fs::write(cargo.join(".crates.toml"), "[v1]\n\"moai 0.1.3 (path+file:///src/moai)\" = [\"moai\"]\n")
            .unwrap();
        assert_eq!(
            refusal(&cargo.join("bin").join("moai"), true, |_| false),
            Some(Refusal::BuiltByCargo { dir: cargo.join("bin") }),
            "cargo install — 쓰기보다 먼저 막는다"
        );
    }

    /// **Windows 에서는 `moai.exe` 가 제 이름이다**(moai-j9gv.3fl) — `moai` 만 받던 판은 Windows 의 모든
    /// moai 를 "이름이 다르다" 로 거절했다. 이름은 그 기계의 설치 스크립트가 까는 것 하나만 받는다.
    #[test]
    fn windows_takes_moai_exe_and_only_that() {
        let s = crate::scratch::Scratch::new("update-refusal-exe");
        let bin = s.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let want = ps("x").binary();
        assert_eq!(refusal_named(&bin.join("moai.exe"), true, want, yes), None);
        assert_eq!(
            refusal_named(&bin.join("moai"), true, want, yes),
            Some(Refusal::NotNamedMoai { name: "moai".into() }),
            "Windows 의 설치 스크립트는 moai.exe 를 깐다"
        );
        assert_eq!(
            refusal_named(&bin.join("moai.exe"), true, Runner::Sh.binary(), yes),
            Some(Refusal::NotNamedMoai { name: "moai.exe".into() })
        );
    }

    /// **찍는 줄과 도는 인자가 한 값에서 나온다.** `--force` 는 늘 서고, 판은 스크립트의 인자로 간다.
    #[test]
    fn the_plan_carries_dir_force_and_version() {
        let dir = Path::new("/home/u/my bin");
        let p = plan("fork/moai", "https://x/install.sh".into(), dir, None, Runner::Sh);
        let args: Vec<String> = p.args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(p.program, "sh");
        assert_eq!(args, ["-s", "--", "--dir", "/home/u/my bin", "--force"]);
        assert_eq!(p.line(), "MOAI_REPO=fork/moai sh -s -- --dir '/home/u/my bin' --force");
        let p = plan("fork/moai", "https://x/install.sh".into(), dir, Some("v0.9.0"), Runner::Sh);
        assert!(p.line().ends_with("--force --version v0.9.0"), "{}", p.line());
    }

    /// **Windows 의 argv**(moai-j9gv.3fl) — `install.ps1` 을 파일로 받아 `powershell -File` 로 돌린다.
    /// 프로필은 안 읽고 실행 정책은 이 프로세스에서만 연다. `canonicalize` 가 붙인 `\\?\` 는 걷는다 —
    /// Windows PowerShell 5.1 이 그 꼴을 못 읽는다.
    #[test]
    fn the_windows_plan_runs_install_ps1_from_a_file() {
        let file = r"C:\Users\u\AppData\Local\Temp\moai-update-42.ps1";
        let p =
            plan("fork/moai", "https://x/install.ps1".into(), Path::new(r"\\?\C:\Users\u\My Tools"), None, ps(file));
        let args: Vec<String> = p.args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(p.program, "powershell");
        assert_eq!(
            args,
            [
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                file,
                "-Dir",
                r"C:\Users\u\My Tools",
                "-Force"
            ]
        );
        assert_eq!(
            p.line(),
            r"$env:MOAI_REPO=fork/moai; powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File C:\Users\u\AppData\Local\Temp\moai-update-42.ps1 -Dir 'C:\Users\u\My Tools' -Force"
        );
        let p = plan("o/it's", "u".into(), Path::new(r"\\?\UNC\srv\share\bin"), Some("v0.17.0"), ps(file));
        let args: Vec<String> = p.args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(
            args.ends_with(&[r"\\srv\share\bin".into(), "-Force".into(), "-Version".into(), "v0.17.0".into()]),
            "{args:?}"
        );
        assert!(p.line().starts_with("$env:MOAI_REPO='o/it''s'; "), "{}", p.line());
    }

    #[test]
    fn the_script_comes_from_main_unless_turned() {
        assert_eq!(
            script_url(|_| None, "fork/moai", &Runner::Sh),
            "https://raw.githubusercontent.com/fork/moai/main/install.sh"
        );
        assert_eq!(
            script_url(|_| None, "fork/moai", &ps("x")),
            "https://raw.githubusercontent.com/fork/moai/main/install.ps1",
            "Windows 는 install.ps1 을 받는다"
        );
        let turned = |k: &str| (k == SCRIPT_VAR).then(|| OsString::from("http://127.0.0.1:1/install.sh"));
        assert_eq!(script_url(turned, "fork/moai", &Runner::Sh), "http://127.0.0.1:1/install.sh");
        assert_eq!(
            script_url(|k: &str| (k == SCRIPT_VAR).then(OsString::new), crate::latest::DEFAULT_REPO, &Runner::Sh),
            "https://raw.githubusercontent.com/buzzni/moai/main/install.sh",
            "빈 값은 없는 것이다"
        );
    }
}
