//! CLI 표면.
//!
//! **여기는 렌더러다.** argv 를 읽고, 코어를 부르고, 나온 줄을 찍고,
//! `Result` 를 종료 코드로 바꾼다. 계산이 여기 있으면 나중에 TUI 도
//! 같은 것을 다시 짜야 한다.

mod cli;
mod cmd;
mod config;
mod draft;
mod fail;
mod git;
mod git_leaks;
mod guide;
mod hook;
mod i18n;
mod id;
mod latest;
mod markdown;
mod model;
mod nav;
mod projects;
mod query;
mod read_marks;
mod report;
#[cfg(test)]
mod scratch;
mod skill;
mod store;
mod style;
mod text;
mod tui;
mod tz;
mod user_config;
mod view;
mod worktree;

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
                eprintln!("moai: {}", i18n::fill(i18n::say(said_lang(), "warn.stdout_lost"), &[("why", &e.to_string())]));
                std::process::exit(1);
            }
            return;
        }
    }};
}

/// 이 파일의 알림이 쓸 화면 말(moai-vjlh).
///
/// **[`cmd::Ctx`] 는 여기까지 안 온다** — `cmd::run` 이 `cli` 째로 삼키고 끝난다. 그래서 사용자
/// 설정을 한 번 더 읽는데, 이 줄들은 할 말이 있을 때만 서므로(`carried`·`unjournaled`·
/// `unread_journals`·`redirected` 넷 다 먼저 비었는지 본다) 거의 모든 판에서 읽지 않는다.
/// **넷 다 그래야 한다** — 하나라도 그냥 부르면 도구 호출마다 도는 훅까지 이 읽기를 치른다.
/// `OnceLock` 은 한 판에 여러 줄이 설 때(프로젝트마다 한 줄) 같은 파일을 그만큼 다시 파지
/// 않게 한다.
///
/// **값을 먼저 길어 놓고 넣는다**(리뷰) — `cmd::Ctx::lang` 이 이름 대어 적어 둔 덫이다. 설정을
/// 읽는 길이 언젠가 제 까닭을 `outln!` 로 한 줄 내면, 그 `outln!` 이 여기를 도로 불러 제
/// 초기화 안에서 `get_or_init` 을 다시 연다 — 재진입은 안 풀리고 모든 명령이 말없이 멈춘다.
fn said_lang() -> i18n::Lang {
    static LANG: std::sync::OnceLock<i18n::Lang> = std::sync::OnceLock::new();
    if let Some(lang) = LANG.get() {
        return *lang;
    }
    let picked = cmd::lang_of(&user_config::read(user_config::path().as_deref()));
    *LANG.get_or_init(|| picked)
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

    // **어디서 쳤는지를 `-C` 가 옮기기 전에 적어 둔다**(moai-y7go, 리뷰 moai-71ht 셋째 판) — 집은
    // 표식은 "어느 체크아웃이 집었나" 를 이것으로 가른다. 트래커를 찾은 자리로 가르면 규약이 권하는
    // `moai -C <루트> mv <id> in_progress` 를 워크트리에서 친 집기가 통째로 빠진다.
    store::remember_invoked();
    if let Some(dir) = &cli.dir
        && let Err(e) = std::env::set_current_dir(dir)
    {
        return fail(cli.json, &cmd::Fail::new(format!("{}: {e}", dir.display())));
    }

    // `cmd::run` 이 `cli` 를 삼키기 **전에** 챙긴다. argv 를 다시 훑어
    // `--json` 을 찾으면 제목이나 메모가 그 낱말일 때(`add -- "--json"`)
    // 아무도 시키지 않은 기계 출력이 나온다.
    let json = cli.json;
    // **훅에서는 올라가 잡았다는 줄을 접는다**(`cli::Cmd::Hook` 의 도움말: "훅이 에러를 뱉으면
    // 매 세션 시작이 시끄럽고, 그러면 사람이 훅을 꺼 버린다"). 훅은 도구 호출마다 도는데, git
    // 아닌 저장소의 하위 디렉터리에서 돌면 그 줄이 부른 횟수만큼 선다 — 알림을 둔 까닭이 소음을
    // 줄이는 것인데 가장 잦은 부름에서 소음이 된다.
    //
    // **접는 것은 그 한 줄뿐이다** — `carried`·`unjournaled` 와 쓴 자리를 대는 `redirects` 는
    // 훅에서도 그대로 나간다. 잦다는 까닭은 *찾기*마다 서는 줄에 대한 것이지, 정말 일어난
    // 쓰기를 대는 줄에 대한 것이 아니다.
    let quiet = matches!(cli.cmd, Some(cli::Cmd::Hook { .. }));
    match cmd::run(cli) {
        Ok(lines) => {
            print(&lines);
            carried();
            unjournaled();
            unread_journals();
            redirected(!quiet);
            if cmd::had_partial() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        // **넘어진 길에서도 어느 트래커를 봤는지는 댄다**(moai-a2kn) — 올라가 잡은 자리는
        // *찾기*의 결과라 실패한 명령도 이미 그것을 썼다. 안 대던 판은 `~/.moai` 를 잡은 `show`
        // 가 "못 찾았다" 만 내어, 엉뚱한 트래커를 본 줄 모르고 없는 줄을 찾게 했다.
        //
        // **`--json` 은 빼고 댄다.** 넘어진 길의 stderr 는 기계의 것이다 — `fail` 이 거기에
        // 오류 객체를 통째로 낸다(성공한 길은 stdout 이라 경고가 섞여도 됐다). 그 앞에 사람
        // 말 한 줄을 끼우면 stderr 가 더는 JSON 값 하나가 아니고, 그것을 파싱하던 고리는
        // `code` 로 갈라지는 대신 파싱 실패를 만난다 — 진짜 오류가 도구 고장으로 읽힌다.
        Err(e) => {
            if !json {
                // **넘어진 길에서도 `chmod` 할 자리는 댄다**(리뷰) — 종료 코드는 이미 1 이지만,
                // 고치는 법을 아는 줄은 이것뿐이다. 다음 판이 성공해야 비로소 듣게 두면, 그 사이의
                // 답은 이력이 빠진 줄 모르고 쓰인다. `--json` 을 뺀 까닭은 아래 `redirected` 와 같다.
                //
                // **`redirected` 보다 먼저다** — 성공한 길과 같은 차례다. 이쪽이 `Repo::find` 를
                // 한 번 더 지나므로, 뒤에 두면 그 찾기가 적은 자리가 이미 지나간 줄 뒤에 남는다.
                unread_journals();
                redirected(!quiet);
            }
            fail(json, &e)
        }
    }
}

/// 쓰기가 못 읽는 줄을 그대로 들고 넘어갔으면 말한다.
///
/// **막지 않는 대신 시끄럽다.** 막으면 남의 낡은 줄 하나가 모든 쓰기를
/// 막고, 잃으면 조용한 손실이다. 셋째 길은 들고 가되 말하는 것이다.
/// 종료 코드는 건드리지 않는다 — 쓰기는 성공했고, 깨진 데이터로 비영
/// 종료하는 것은 `moai status` 한 곳이다.
fn carried() {
    // **쓴 자리와 안 쓴 자리의 말이 다르다.** 안 쓴 명령(`note`·이미 그런
    // `defer`)이 "그대로 두고 썼다" 고 하면 일어나지 않은 쓰기를 주장하고,
    // 아무 말도 안 하면 그 동사만 쓰는 쪽이 상한 파일을 영영 모른다(moai-relb).
    //
    // 셈은 호출마다가 아니라 프로세스 전체의 것이다(`store::Tally`) — `moai tui` 는
    // 여러 번 쓰고 여기를 한 번 지난다. 썼으되 들고 간 줄이 없었으면(깨끗할 때 쓰고
    // 그 뒤에 상했으면) "안 건드렸다" 도 "그대로 두고 썼다" 도 참이 아니라 있다고만 한다.
    //
    // **프로젝트마다 한 줄이다.** 탐색기는 층에서 여러 프로젝트에 쓴다 — 여기서 부른
    // 자리의 저장소가 아닌 곳이면 `-C` 로 그 뿌리를 댄다. 대지 않으면 `moai show` 가
    // 엉뚱한 저장소의 줄을 내거나 "init 하라" 고 한다.
    let tallies = store::unreadable_tallies();
    if tallies.iter().all(|(_, t)| t.carried == 0 && t.seen == 0) {
        return;
    }
    let lang = said_lang();
    let here = store::Repo::find(said_lang).ok().flatten().map(|r| r.root);
    for (root, tally) in tallies {
        // **갈래마다 제 `say` 를 적는다** — 키를 변수로 고르면 소스를 훑는 시험
        // (`i18n::tests::keys_in`)의 눈에서 그 키가 사라진다.
        let said = match tally {
            store::Tally { carried: n @ 1.., .. } => {
                i18n::fill(i18n::say(lang, "warn.carried_wrote"), &[("n", &n.to_string())])
            }
            store::Tally { seen: 0, .. } => continue,
            store::Tally { wrote: true, seen: n, .. } => {
                i18n::fill(i18n::say(lang, "warn.carried_seen"), &[("n", &n.to_string())])
            }
            store::Tally { wrote: false, seen: n, .. } => {
                i18n::fill(i18n::say(lang, "warn.carried_untouched"), &[("n", &n.to_string())])
            }
        };
        let show = match &here {
            Some(h) if *h == root => "moai show".to_string(),
            _ => format!("moai -C {} show", root.display()),
        };
        let _ = writeln!(
            anstream::stderr().lock(),
            // **어느 줄인지 아는 명령을 댄다.** `moai status` 는 수만 말하고
            // 줄 번호와 까닭은 `report_load_errors` 를 지나는 쪽(`show`·`ready`)
            // 만 낸다 — 없는 답을 가리키면 손으로 고칠 길이 도구 밖에만 남는다.
            "{}{}",
            style::paint(style::WARN, "moai: "),
            i18n::fill(i18n::say(lang, "warn.carried_where"), &[("said", &said), ("show", &show)])
        );
    }
}

/// **어디에 썼는지 옮겨 쓴 자리마다 한 줄로 알린다**(moai-y7go) — 딸린 워크트리에서 친 `moai` 는
/// 루트의 트래커를 고친다(`store::Repo::find_from`). 조용히 옮기면 시킨 쪽은 제가 선 자리에
/// 썼다고 믿고, 그 워크트리의 `.moai` 가 왜 안 바뀌는지를 딴 데서 찾는다. 자리가 둘일 수 있는 것은
/// 탐색기다 — 층에서 등록한 워크트리 여럿에 쓴다(`carried` 와 같은 자).
///
/// **찍는 자리가 `store` 가 아니라 여기인 까닭 셋**(리뷰 moai-71ht.jlh). 쓰기 경로 안에서
/// `eprintln!` 하면 (1) 대체 화면을 쥔 탐색기의 그림을 쓸 때마다 망가뜨리고(`tui::draw` 의 배너가
/// 그래서 있다), (2) 락·검증에 걸려 아무것도 안 쓴 명령까지 "썼다" 고 말하고, (3) 한 프로세스가
/// 열 번 쓰면 같은 줄이 열 번 선다. 여기는 색과 `moai: ` 머리를 다른 경고와 한 자로 쓰고,
/// stderr 가 닫혀도 `writeln!` 의 실패를 버린다 — `eprintln!` 은 거기서 패닉한다.
/// `climbs` 가 거짓이면 **올라가 잡았다는 줄만** 접는다(훅). 아래 `redirects()` 는 그대로
/// 나간다 — 그 줄은 쓴 자리를 대는 것이라 잦기와 상관없이 빠지면 안 된다. 둘을 한 깃발로
/// 묶어 두던 판은 훅이 트래커에 한 줄이라도 쓰는 날 그 알림을 말없이 잃는다(지금은
/// `cmd::hook` 이 `with_write` 를 안 지나 `MOVED` 가 늘 비어 있을 뿐이고, 그것을 세우는
/// 시험이 없다).
fn redirected(climbs: bool) {
    // **올라가 잡은 것은 제 체크아웃 밖일 때만 선다**(moai-a2kn) — 제 저장소의 하위 디렉터리에
    // 선 사람에게는 한 줄도 안 나간다. `store::CLIMBED` 가 그 가름을 적는다.
    //
    // **"썼다" 고 하지 않는다.** 이 줄은 *찾기*의 결과라 `status`·`ready`·`show` 도 지나는데,
    // 아래 `redirects()` 처럼 쓴 것으로 적으면 읽기만 한 명령이 없던 쓰기를 주장한다.
    for (from, to) in climbs.then(store::climbs).unwrap_or_default() {
        let _ = writeln!(
            anstream::stderr().lock(),
            "{}{}",
            style::paint(style::WARN, "moai: "),
            i18n::fill(
                i18n::say(said_lang(), "warn.tracker_climbed"),
                &[("from", &from.display().to_string()), ("to", &to.display().to_string())]
            )
        );
    }
    for (from, to) in store::redirects() {
        let _ = writeln!(
            anstream::stderr().lock(),
            "{}{}",
            style::paint(style::WARN, "moai: "),
            i18n::fill(
                i18n::say(said_lang(), "warn.tracker_moved"),
                &[("from", &from.display().to_string()), ("to", &to.display().to_string())]
            )
        );
    }
}

/// 스냅샷은 썼는데 저널에 못 적었으면 말한다(moai-52z9).
///
/// **종료 코드는 0 이다.** 쓰기는 담겼다 — 비영으로 끝나면 사람이 다시 부르고, `add` 는
/// 같은 이슈를 하나 더 세운다. 그래서 "다시 부르지 않는다" 를 함께 댄다.
fn unjournaled() {
    let missed = store::journal_misses();
    // **빈 것이면 자리를 안 잰다** — `Repo::find` 는 조상 훑기와 설정 읽기를 치른다(`carried` 와
    // 같은 자리에 이미 이 갈래가 있다). 거의 모든 명령이 여기를 빈 채로 지난다.
    if missed.is_empty() {
        return;
    }
    let lang = said_lang();
    let here = store::Repo::find(said_lang).ok().flatten().map(|r| r.root);
    for (root, why) in missed {
        let whose = match &here {
            Some(h) if *h == root => String::new(),
            _ => format!(" ({})", root.display()),
        };
        let _ = writeln!(
            anstream::stderr().lock(),
            "{}{}",
            style::paint(style::WARN, "moai: "),
            i18n::fill(i18n::say(lang, "warn.unjournaled"), &[("whose", &whose), ("why", &why)])
        );
    }
}

/// 못 읽어 건너뛴 저널 자리를 말한다(moai-6924).
///
/// **관대하되 시끄럽게**(2026-09-21 사용자 결정). [`store::Repo::journal_by_id`] 는 못 읽는 파일
/// 하나에 통째로 지지 않고 읽은 것을 낸다 — 남의 `<메일>.jsonl` 이 0600 으로 서는 것만으로 제
/// 이력까지 안 보이던 자리다. 그 관대함이 **조용한 손실이 되지 않게** 하는 자가 이 함수다.
///
/// **종료 코드를 0 이 아니게 한다.** [`carried`]·[`unjournaled`] 와 갈리는 지점이다: 저쪽은
/// 쓰기가 담긴 판이라 말만 하지만, 여기는 사람이 **덜 받은 답**을 손에 쥔 판이다. 스냅샷의
/// 못 읽는 줄을 `cmd::report_load_errors` 가 그렇게 다루는 것과 같은 자다. `--json` 을 읽는
/// 기계는 이 코드로 "이 판이 온전치 않다" 를 안다. 어느 이슈의 **이력이** 덜 왔는지는
/// `show --json` 의 `journal_error` 가 따로 말하지만(moai-f2lc), 그 키는 저널을 읽는 명령에만
/// 서므로 나머지 판을 가르는 자는 여전히 이 코드다 — 이 줄이 무너지면 조용한 손실이다.
///
/// **자리를 통째로 댄다.** [`unjournaled`] 처럼 저장소 뿌리로 줄이지 않는다 — 여기서 알아야 할
/// 것은 "어느 저장소냐" 가 아니라 *어느 파일에 `chmod` 를 하느냐* 이고, 그것이 곧 고치는 법이다.
///
/// **제 뿌리의 것만 코드를 바꾼다**(리뷰). `show --worktree` 는 겹쳐 온 줄의 이력을 그 워크트리의
/// 저널에서 읽으므로 옆 체크아웃의 자리가 섞인다 — `cmd::gather` 가 옆 워크트리의 깨진 줄에 대해
/// 못박은 금이 여기도 그대로다. 말은 둘 다 한다.
fn unread_journals() {
    let unread = store::journal_unread();
    // **빈 것이면 말을 안 고른다** — [`said_lang`] 은 사용자 설정을 한 번 더 파므로, 여기가
    // 그것을 그냥 부르면 *모든* 명령이(도구 호출마다 도는 훅까지) 그 읽기를 치른다.
    // `carried`·`unjournaled`·`redirected` 셋이 먼저 비었는지 보는 것과 같은 까닭이다.
    if unread.is_empty() {
        return;
    }
    let here = store::Repo::find(said_lang).ok().flatten().map(|r| r.root);
    tell_unread(said_lang(), here.as_deref(), &unread);
}

/// 길어 온 값에 대고 말하고 깃발을 세운다. **전역을 읽는 것은 위가 하고 여기는 안 읽는다** —
/// 시험이 이 자리를 직접 불러 종료 코드까지 잰다([`tests::an_unread_journal_makes_the_run_fail`]).
///
/// `here` 를 못 찾았으면(`.moai` 밖에서 끝난 판) 가르지 않고 세운다 — 못 가릴 때 조용한 쪽으로
/// 떨어지면 그것이 곧 조용한 손실이다.
fn tell_unread(lang: i18n::Lang, here: Option<&std::path::Path>, unread: &[store::Unread]) {
    let lines = unread_lines(lang, unread);
    if lines.is_empty() {
        return;
    }
    for l in lines {
        let _ = writeln!(anstream::stderr().lock(), "{}{}", style::paint(style::WARN, "moai: "), l);
    }
    // **말한 뒤에 세운다** — 이 줄이 안 나갔는데 코드만 1 이면 사람은 까닭 없는 실패를 본다.
    if any_mine(here, unread) {
        cmd::note_partial();
    }
}

/// 못 읽은 것 가운데 **제 저장소의 것**이 있는가 — 종료 코드를 바꿀지 가르는 자. **순수하다.**
///
/// `here` 가 없으면(`.moai` 밖에서 끝난 판) 가르지 못하니 참이다 — 못 가릴 때 조용한 쪽으로
/// 떨어지면 그것이 곧 조용한 손실이다.
fn any_mine(here: Option<&std::path::Path>, unread: &[store::Unread]) -> bool {
    unread.iter().any(|u| here.is_none_or(|h| h == u.root))
}

/// 자리마다 한 줄. **순수하다** — 찍지도, 전역을 건드리지도 않는다.
///
/// **글을 여기서 안 짓는다**(moai-f2lc) — `--json` 의 `journal_error` 가 같은 줄을 내므로
/// [`cmd::journal_errors`] 하나가 짓고 여기는 그 `said` 를 편다. 두 자리에서 지으면 같은
/// 실패를 stderr 와 기계 출력이 다른 말로 말한다.
fn unread_lines(lang: i18n::Lang, unread: &[store::Unread]) -> Vec<String> {
    cmd::journal_errors(lang, unread, None).into_iter().map(|e| e.said).collect()
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
        let _ = writeln!(anstream::stderr().lock(), "{}{}", style::paint(style::ERROR, "moai: "), e.message);
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn at(root: &str, at: &str, why: &str) -> store::Unread {
        store::Unread { root: PathBuf::from(root), at: PathBuf::from(at), kind: "permission", said: why.to_string() }
    }

    fn one(at: &str, why: &str) -> Vec<store::Unread> {
        vec![self::at("/tmp/moai-6924", at, why)]
    }

    /// **못 읽은 자리와 까닭을 둘 다 댄다**(moai-6924). 고치는 법이 곧 그 자리에 대는 `chmod`
    /// 라, 자리를 안 대면 사람은 무엇을 고쳐야 하는지 모른 채 이력만 잃는다.
    #[test]
    fn an_unread_journal_names_the_place_and_the_reason() {
        let said = unread_lines(i18n::Lang::En, &one(".moai/journal/b_x_com.jsonl", "Permission denied"));
        assert_eq!(said.len(), 1);
        assert!(said[0].contains(".moai/journal/b_x_com.jsonl"), "자리를 안 댄다 — {}", said[0]);
        assert!(said[0].contains("Permission denied"), "까닭을 안 댄다 — {}", said[0]);

        // **말묶음에서 온다** — 한국어 판도 같은 두 값을 든다. 하드코딩한 글이면 여기가 붉어진다.
        let ko = unread_lines(i18n::Lang::Ko, &one(".moai/journal/b_x_com.jsonl", "Permission denied"));
        assert_ne!(ko[0], said[0], "말이 안 갈린다");
        assert!(ko[0].contains(".moai/journal/b_x_com.jsonl") && ko[0].contains("Permission denied"));
    }

    /// 자리마다 한 줄이고, 없으면 한 줄도 없다 — 빈 것이 곧 [`tell_unread`] 의 문지기다.
    #[test]
    fn nothing_unread_says_nothing() {
        assert!(unread_lines(i18n::Lang::En, &[]).is_empty());
        assert_eq!(unread_lines(i18n::Lang::En, &one("a.jsonl", "x")).len(), 1);
    }

    /// **못 읽은 이력이 있으면 종료 코드가 0 이 아니다**(2026-09-21 사용자 결정, moai-6ney).
    ///
    /// `show --json` 의 `journal_error` 가 어느 줄이 값을 치렀는지를 대고(moai-f2lc), 저널을
    /// 안 읽는 명령에서 "이 판이 온전치 않다" 를 말하는 자는 이 코드 하나로 남는다.
    /// `main` 이 [`cmd::had_partial`] 로 갈리므로 그 깃발을 직접 잰다 —
    /// 깃발은 한 번 서면 안 내려가니 병렬 시험끼리 섞여도 이 쪽은 흔들리지 않는다.
    #[test]
    fn an_unread_journal_makes_the_run_fail() {
        let here = PathBuf::from("/tmp/moai-6924");
        tell_unread(
            i18n::Lang::En,
            Some(&here),
            &one("/tmp/moai-6924/.moai/journal/b_x_com.jsonl", "Permission denied"),
        );
        assert!(cmd::had_partial(), "덜 받은 답을 0 으로 끝냈다");
    }

    /// **옆 워크트리의 못 읽는 저널은 말만 하고 종료 코드를 안 바꾼다**(리뷰). `show --worktree`
    /// 는 겹쳐 온 줄의 이력을 그 워크트리의 저널에서 읽으므로, 여기서 안 가르면 제 파일은
    /// 멀쩡한 판이 남의 0600 파일 하나로 실패로 읽힌다 — `cmd::gather` 가 옆 워크트리의 깨진
    /// 줄에 대해 못박은 것과 같은 금이다("옆 워크트리의 깨진 줄로 `moai status` 가 비영 종료하면
    /// 제 파일은 멀쩡한데 도구가 실패로 읽힌다").
    ///
    /// 깃발은 한 번 서면 안 내려가니 가르는 자([`any_mine`])를 직접 잰다 — 같은 판의 다른
    /// 시험이 세워 둔 깃발과 섞이지 않는다. **말은 둘 다 한다**: 줄은 그대로 선다.
    #[test]
    fn a_neighbours_unread_journal_is_told_but_does_not_fail_the_run() {
        let mine = PathBuf::from("/tmp/moai-6924");
        let theirs =
            vec![at("/tmp/moai-6ney/side", "/tmp/moai-6ney/side/.moai/journal/b_x_com.jsonl", "Permission denied")];
        assert!(!any_mine(Some(&mine), &theirs), "옆 워크트리의 것으로 종료 코드를 바꿨다");
        assert!(any_mine(Some(&mine), &one("/tmp/moai-6924/.moai/journal/b_x_com.jsonl", "denied")));
        // 어느 저장소인지 못 찾은 판은 가르지 못하니 세운다 — 못 가릴 때 조용해지지 않는다.
        assert!(any_mine(None, &theirs));

        // **말은 그래도 한다** — 옆의 것도 자리와 까닭을 댄다.
        let said = unread_lines(i18n::Lang::En, &theirs);
        assert_eq!(said.len(), 1);
        assert!(said[0].contains("/tmp/moai-6ney/side/.moai/journal/b_x_com.jsonl"), "{}", said[0]);
    }
}
