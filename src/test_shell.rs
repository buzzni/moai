//! 단위 시험의 자식 셸은 시작 파일을 읽지 않는다.
//!
//! 비대화형 bash 도 `BASH_ENV` 를 읽으며, Debian/Ubuntu 에서는 SSH_CLIENT 가 서고 SHLVL 이 0 이면
//! `~/.bashrc` 도 읽는다. 앞은 환경에서 걷고, 뒤는 bash 에만 `--norc` 를 준다. dash 는 그 플래그를 모른다.
//! 시작 파일이 PATH·TMPDIR·stdout 을 바꾸면 시험이 사람의 기계에서만 지고, 느린 초기화도 매번 돈다 —
//! text::tests 는 11.0s 대 0.04s, skill::tests 는 24.8s 대 0.5s 였다(moai-n8po.oj4).
//! HOME 은 그대로 두고, 부르는 쪽은 반환된 명령의 환경을 다시 덮을 수 있다.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

pub fn shell(program: impl AsRef<OsStr>) -> Command {
    let program = program.as_ref();
    let mut command = Command::new(program);
    command.env_remove("BASH_ENV");
    // Windows 에서 찾은 bin/bash.exe 도 같은 껍데기다.
    if Path::new(program).file_stem() == Some(OsStr::new("bash")) {
        command.arg("--norc");
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 띄우지 않고 재므로 bash 가 없는 기계에서도 두 시작 경로의 되돌림을 잡는다.
    #[test]
    fn the_shell_reads_no_start_file() {
        for (program, bash) in [("sh", false), ("bash", true), ("dash", false), ("bin/bash.exe", true)] {
            let command = shell(program);
            assert_eq!(
                command.get_envs().find(|(name, _)| *name == OsStr::new("BASH_ENV")),
                Some((OsStr::new("BASH_ENV"), None)),
                "{program} 이 BASH_ENV 를 물려받는다"
            );
            let args: Vec<_> = command.get_args().collect();
            let expected = if bash { vec![OsStr::new("--norc")] } else { Vec::new() };
            assert_eq!(args, expected, "{program}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_bashrc_read_over_ssh_stays_shut() {
        if !shell("bash").args(["-c", "exit 0"]).output().is_ok_and(|out| out.status.success()) {
            return;
        }
        let s = crate::scratch::Scratch::new("shell-bashrc");
        std::fs::write(s.path().join(".bashrc"), "printf rc\n").unwrap();
        let out = shell("bash")
            .env("HOME", s.path())
            .env("SSH_CLIENT", "127.0.0.1 1 22")
            .env("SHLVL", "0")
            .args(["-c", "printf ok"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(out.stdout, b"ok");
    }
}
