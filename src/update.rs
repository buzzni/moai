//! `moai update` 가 무엇을 돌릴지 고른다(moai-zsfr.2p9, 사람 결정 2026-10-08).
//!
//! 사람이 치던 한 줄 — `curl -fsSL …/install.sh | sh -s -- --force` — 을 명령 하나로 옮긴다. 받는
//! 저장소는 [`crate::latest::repo_from`] 이 고르고, 스크립트는 그 저장소 `main` 의 `install.sh` 다.
//! 그것을 **도는 바이너리의 자리**에 `--dir <자리> --force` 로 돌린다 — `~/.local/bin` 에 박힌 줄은
//! 다른 자리에 깐 moai 를 못 올렸다(`latest::upgrade_line` 이 자리를 보는 까닭과 같다).
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
/// 판 묻기에 하는 일을 이쪽에 한다. 거울을 쓰는 사람도 쓸 수 있지만, 이 값은 `sh` 에 흘릴 글을
/// 고르므로 사람 설정에는 안 둔다 — 그 자리는 저장소 이름(`[update] repo`)까지다.
pub const SCRIPT_VAR: &str = "MOAI_INSTALL_URL";

/// 받을 스크립트의 자리 — [`SCRIPT_VAR`] 가 있으면 그것, 없으면 `repo` 의 `main` 의 `install.sh`.
pub fn script_url(env: impl Fn(&str) -> Option<OsString>, repo: &str) -> String {
    env(SCRIPT_VAR)
        .filter(|v| !v.is_empty())
        .and_then(|v| v.into_string().ok())
        .unwrap_or_else(|| crate::latest::script_of(repo))
}

/// 못 올리는 까닭 — **말이 아니라 자료다**. 펴는 것은 `view::update_refused` 다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// `install.sh` 가 판을 안 내는 기계([`crate::latest::SERVED`]) — 받아 봐야 "맞는 판이 없다" 로 끝난다.
    NotServed,
    /// 이름이 `moai` 가 아니다. `install.sh` 는 `<자리>/moai` 로 까므로, 돌리면 이 바이너리는 그대로고
    /// 곁에 하나가 더 선다.
    NotNamedMoai { name: String },
    /// cargo 가 지은 자리([`crate::latest::built_by_cargo`]) — 소스로 다시 짓는 쪽이 올린다. 덮으면
    /// 다음 `cargo build` 가 말없이 도로 덮거나, `cargo install` 의 장부가 거짓이 된다.
    BuiltByCargo { dir: PathBuf },
    /// 그 자리에 쓸 수 없다 — root 가 깐 `/usr/local/bin` 따위. **집 밖이라는 것만으로는 안 막는다**
    /// (사람 결정 2026-10-08) — 쓸 수 있으면 그 사람이 깐 자리다.
    NotWritable { dir: PathBuf },
}

/// `exe`(도는 바이너리의 **푼** 자리)를 올릴 수 있는가 — 없으면 그 까닭. `None` 이면 올린다.
///
/// **차례가 곧 대는 까닭이다.** 판을 안 내는 기계에서는 다른 무엇을 고쳐도 소용없으므로 그것부터,
/// cargo 가 지은 자리는 쓸 수 있어도 덮으면 안 되므로 쓰기보다 먼저다.
///
/// `writable` 을 받는 것은 시험이 권한을 꾸미지 않고 갈래를 재려는 것이다 — root 로 도는 시험
/// 기계에서는 `chmod` 가 아무것도 안 막는다.
pub fn refusal(exe: &Path, served: bool, writable: impl Fn(&Path) -> bool) -> Option<Refusal> {
    if !served {
        return Some(Refusal::NotServed);
    }
    let name = exe.file_name().unwrap_or_default();
    if name != OsStr::new("moai") {
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

/// 돌릴 것 하나 — 받을 자리와 `sh` 에 줄 인자. **`--dry-run` 이 찍는 것과 실제로 돌리는 것이 이 값
/// 하나에서 나온다** — 둘이 따로 지어지면 찍은 줄과 돈 줄이 갈리는 날이 온다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 받을 저장소 — `install.sh` 에 `MOAI_REPO` 로 넘긴다. 안 넘기면 포크의 스크립트가 본가의
    /// 바이너리를 받는다.
    pub repo: String,
    /// 스크립트의 자리.
    pub script: String,
    /// 깔 자리 — 도는 바이너리가 선 디렉터리.
    pub dir: PathBuf,
    /// `sh` 뒤의 인자. `-s --` 로 시작한다 — 스크립트는 표준 입력으로 흘린다.
    pub args: Vec<OsString>,
}

/// [`Plan`] 을 짓는다. **`--force` 는 늘 단다**(사람 결정 2026-10-08) — 사람이 치던 줄이 그랬고,
/// 그래야 같은 판을 다시 받는 것도 이 명령 하나로 된다. 판(`version`)은 스크립트가 아니라 스크립트의
/// 인자로 고른다 — 스크립트는 늘 `main` 의 것이다.
pub fn plan(repo: &str, script: String, dir: &Path, version: Option<&str>) -> Plan {
    let mut args: Vec<OsString> = ["-s", "--", "--dir"].map(OsString::from).into();
    args.push(dir.as_os_str().to_owned());
    args.push("--force".into());
    if let Some(v) = version {
        args.push("--version".into());
        args.push(v.into());
    }
    Plan { repo: repo.to_string(), script, dir: dir.to_path_buf(), args }
}

impl Plan {
    /// 사람이 셸에 그대로 칠 수 있는 꼴의 한 줄 — `MOAI_REPO=<저장소> sh -s -- --dir <자리> --force …`.
    /// 셸이 가르는 글자는 싼다([`crate::text::quoted`]).
    pub fn line(&self) -> String {
        let words: Vec<String> = self.args.iter().map(|a| crate::text::quoted(&a.to_string_lossy())).collect();
        format!("{}={} sh {}", crate::latest::REPO_VAR, crate::text::quoted(&self.repo), words.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yes(_: &Path) -> bool {
        true
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

    /// **찍는 줄과 도는 인자가 한 값에서 나온다.** `--force` 는 늘 서고, 판은 스크립트의 인자로 간다.
    #[test]
    fn the_plan_carries_dir_force_and_version() {
        let dir = Path::new("/home/u/my bin");
        let p = plan("fork/moai", "https://x/install.sh".into(), dir, None);
        let args: Vec<String> = p.args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["-s", "--", "--dir", "/home/u/my bin", "--force"]);
        assert_eq!(p.line(), "MOAI_REPO=fork/moai sh -s -- --dir '/home/u/my bin' --force");
        let p = plan("fork/moai", "https://x/install.sh".into(), dir, Some("v0.9.0"));
        assert!(p.line().ends_with("--force --version v0.9.0"), "{}", p.line());
    }

    #[test]
    fn the_script_comes_from_main_unless_turned() {
        assert_eq!(script_url(|_| None, "fork/moai"), "https://raw.githubusercontent.com/fork/moai/main/install.sh");
        let turned = |k: &str| (k == SCRIPT_VAR).then(|| OsString::from("http://127.0.0.1:1/install.sh"));
        assert_eq!(script_url(turned, "fork/moai"), "http://127.0.0.1:1/install.sh");
        assert_eq!(
            script_url(|k: &str| (k == SCRIPT_VAR).then(OsString::new), crate::latest::DEFAULT_REPO),
            "https://raw.githubusercontent.com/buzzni/moai/main/install.sh",
            "빈 값은 없는 것이다"
        );
    }
}
