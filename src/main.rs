//! CLI 표면.
//!
//! **여기는 렌더러다.** argv 를 읽고, 코어를 부르고, 나온 줄을 찍고,
//! `Result` 를 종료 코드로 바꾼다. 계산이 여기 있으면 나중에 TUI 도
//! 같은 것을 다시 짜야 한다.

mod cli;
mod cmd;
mod config;
mod id;
mod model;
mod query;
mod report;
mod store;
mod style;
mod view;

use clap::Parser;
use std::io::Write;
use std::process::ExitCode;

/// `println!` 은 stdout 이 닫히면 패닉한다. `moai show | head` 처럼 출력을
/// 잘라 쓰는 것은 흔한 사용이라 크래시가 아니라 조용한 종료여야 한다.
///
/// **끝내지 않고 버리기만 한다.** 여기서 `exit` 하면 명령이 제 판정을
/// `main` 까지 못 들고 간다 — `moai show | head` 가 오류를 찾아 놓고 0 으로 끝난다.
macro_rules! outln {
    ($($a:tt)*) => {{
        if let Err(e) = writeln!(anstream::stdout().lock(), $($a)*) {
            // 받는 쪽이 닫혔으면 남은 출력만 버린다. 그 밖의 쓰기 실패(디스크가
            // 찼다 등)는 명령이 알 길이 없어, 여기서 끝내야 성공으로 안 보인다.
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                eprintln!("moai: 출력을 쓰지 못했다: {e}");
                std::process::exit(1);
            }
            return;
        }
    }};
}

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    // `--json` 이면 색을 하드코딩으로 끈다. JSON 에 이스케이프가 섞이면
    // 받는 쪽의 파서가 깨지고, 그건 플래그로 고를 일이 아니다.
    let choice = match (cli.json, cli.no_color, cli.color) {
        (true, _, _) | (_, true, _) | (_, _, cli::ColorArg::Never) => anstream::ColorChoice::Never,
        (_, _, cli::ColorArg::Always) => anstream::ColorChoice::Always,
        _ => anstream::ColorChoice::Auto,
    };
    choice.write_global();

    if let Some(dir) = &cli.dir
        && let Err(e) = std::env::set_current_dir(dir)
    {
        return fail(cli.json, &cmd::Fail::new(format!("{}: {e}", dir.display())));
    }

    // `cmd::run` 이 `cli` 를 삼키기 **전에** 챙긴다. argv 를 다시 훑어
    // `--json` 을 찾으면 제목이나 메모가 그 낱말일 때(`add -- "--json"`)
    // 아무도 시키지 않은 기계 출력이 나온다.
    let json = cli.json;
    match cmd::run(cli) {
        Ok(lines) => {
            print(&lines);
            if cmd::had_partial() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        Err(e) => fail(json, &e),
    }
}

fn print(lines: &[String]) {
    for l in lines {
        outln!("{l}");
    }
}

fn fail(json: bool, e: &cmd::Fail) -> ExitCode {
    if json {
        let v = serde_json::json!({"error": e.message, "code": e.code});
        eprintln!("{v}");
    } else {
        // `eprintln!` 은 anstream 을 안 거치므로 색을 끄는 판단이 적용되지
        // 않는다 — `NO_COLOR` 로도 `--no-color` 로도 빨강이 그대로 샌다.
        let _ = writeln!(
            anstream::stderr().lock(),
            "{}{}",
            style::paint(style::ERROR, "moai: "),
            e.message
        );
    }
    ExitCode::FAILURE
}
