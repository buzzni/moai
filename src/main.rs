//! 바이너리 진입점. 몸은 라이브러리(`src/lib.rs`)에 있다 — 여기는 넘겨주기만 한다.

use std::process::ExitCode;

fn main() -> ExitCode {
    moai::run()
}
