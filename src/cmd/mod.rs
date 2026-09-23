//! argv 를 코어 호출로 옮기고 낼 줄을 돌려준다.
//!
//! **이슈의 뜻을 판단하는 `if` 를 여기 두지 않는다.** 어떤 이슈가 무엇인지
//! 정하는 코드가 여기 있으면 나중에 TUI 가 그것을 다시 쓴다.

pub mod add;
pub mod defer;
pub mod edit;
pub mod hook;
pub mod idea;
pub mod init;
pub mod link;
pub mod merge_driver;
pub mod mv;
pub mod note;
pub mod prime;
pub mod project;
pub mod read;
pub mod ready;
pub mod rm;
pub mod show;
pub mod skill;
pub mod status;
pub mod tui;

use crate::cli::{Cli, Cmd, IdeaCmd, ProjectCmd, SkillCmd, Typed};
use crate::model::Kind;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::fail::{Fail, R, code};

pub struct Ctx {
    pub json: bool,
    /// `--user` 를 **푼 값이 아니라 준 값 그대로** 들고 있다. 여기서 미리
    /// 풀면 읽기만 하는 명령(`show`·`status`)까지 사용자 정보를 요구한다.
    pub user: Option<String>,
    /// `-C` 로 자리를 옮겨 불렀는가. 그러면 부른 사람의 셸은 여기가 아니다 — 부른 자리에서
    /// 도는 명령(`init`)을 일러 줄 때 `-C <뿌리>` 를 붙여야 엉뚱한 자리에 심지 않는다.
    pub chdir: bool,
    /// 사용자 설정 — **한 판에 한 번만 읽는다**([`Ctx::registry`]).
    reg: OnceLock<crate::user_config::Registry>,
    /// 이 판의 화면 언어 — 설정에서 한 번 푼 값([`Ctx::lang`]).
    lang: OnceLock<crate::i18n::Lang>,
    /// 이 판의 시간대 — 시스템에서 한 번 푼 값([`Ctx::zone`]).
    zone: OnceLock<(crate::tz::Zone, Option<crate::tz::Trouble>)>,
}

impl Ctx {
    pub fn new(json: bool, user: Option<String>, chdir: bool) -> Ctx {
        Ctx { json, user, chdir, reg: OnceLock::new(), lang: OnceLock::new(), zone: OnceLock::new() }
    }

    /// 사용자 설정. **이 문으로 드는 명령은 한 판에 한 번만 읽는다**(moai-cigu) — 등록 목록도
    /// 화면 언어도 같은 파일에서 오는데, 읽는 자리가 갈리면 한 번 부를 때 같은 TOML 을 두 번
    /// 판다. 그리는 쪽이 제 손으로 다시 읽던 자리(`i18n::current`)를 걷어낸 것이 이 문이다.
    ///
    /// **아직 유일한 문은 아니다**(리뷰) — `cmd::tui` 는 제 손으로 `user_config::read` 를 불러
    /// 층과 보기를 짓는데, 같은 판에서 `ctx.lang()` 도 부르므로 그 명령은 같은 파일을 두 번 판다.
    /// 옮길 자리다. `cmd::project::ls` 와 `cmd::read` 는 `ctx.lang()` 이 들면서 이 문으로 왔다 —
    /// **말을 들이는 명령은 설정도 이 문으로 읽는다**, 안 그러면 두 번 판다.
    pub fn registry(&self) -> &crate::user_config::Registry {
        self.reg.get_or_init(|| crate::user_config::read(crate::user_config::path().as_deref()))
    }

    /// 이 판의 화면 언어. **명령 층이 한 번 풀어 그리는 쪽에 준다**(moai-cigu) — `view` 는
    /// 이것을 인자로 받고 설정을 안 읽는다. 그래야 그리는 시험이 돌리는 사람의 진짜
    /// `~/.config/moai/config.toml` 을 안 읽는다(옛 자리는 기계마다 다른 시험이었다).
    ///
    /// **늦게 읽는다.** 언어가 드는 자리는 `status`·`ready` 와 훅이 세션에 싣는 보드
    /// (`cmd::hook`)뿐인데 `run` 에서 미리 풀면 `moai add` 한 줄에도 설정 파일이 딸려 온다.
    ///
    /// **[`Ctx::registry`] 를 `get_or_init` 안에서 부르는 것은 자물쇠가 둘이라서다**(리뷰
    /// moai-80qw 가 옛 `i18n::picked` 에 적어 둔 덫). 설정을 읽는 길(`user_config::read` →
    /// `Doc::lang`)은 제 글자를 화면에 내는 자리라, 그 글자가 언젠가 이 말로 나가려고
    /// `Ctx::lang` 을 도로 부르면 `lang` 의 `OnceLock` 이 제 초기화 안에서 다시 열린다 —
    /// 재진입은 영영 안 풀리고 모든 명령이 아무 말 없이 멈춘다. 그 줄을 쓰게 되는 날에는
    /// 값을 **먼저 길어 놓고** `get_or_init` 에 넣는다. 옛 자리가 그렇게 썼던 까닭이다.
    pub fn lang(&self) -> crate::i18n::Lang {
        *self.lang.get_or_init(|| lang_of(self.registry()))
    }

    /// 이 판이 시각을 적을 시간대(moai-p5az). **CLI 는 시스템을 그대로 따른다** — `TZ` 가
    /// 먼저고 그다음이 `/etc/localtime` 이다. 설정의 `[tui] timezone` 은 **안 읽는다**: 고르는
    /// 자리가 탐색기 하나(`SPC o t`)라 그 키는 탐색기의 것이고, 고른 적 없으면 두 표면이 같은
    /// 시계로 선다.
    ///
    /// **못 풀어도 막지 않는다** — UTC 로 떨어지고 까닭은 [`Ctx::zone_trouble`] 이 든다
    /// (moai-77ap). 정적 musl 판을 zoneinfo 없는 기계에 받은 자리가 그것이다.
    ///
    /// **말과 같은 결로 늦게 읽는다** — 이 값을 드는 명령만 tzdb 를 만진다.
    ///
    /// **시각을 그리는 명령만이 아니다**(moai-h2th). 기한 판정이 읽는 사람의 달로 서면서
    /// `report::status` 를 부르는 쪽이 모두 이 값을 든다 — `hook` 처럼 시각을 한 줄도 안 그리는
    /// 명령이 거기 든다. 그 대가로 zoneinfo 없는 기계에서는 그 명령들도 [`Ctx::zone_trouble`]
    /// 한 줄을 stderr 에 낸다. 막지 않고 종료 코드도 그대로다.
    ///
    /// **그 줄을 지울지 재 보고 그대로 두었다**(moai-yz4j). 둘을 갈라 봤다.
    ///
    /// - **훅은 이 값을 옳게 든다.** 보드(`UserPromptSubmit`)는 기한 경고를 **그리고**,
    ///   `SessionStart`·`Stop` 의 기준선은 그 경고까지 **센 수**로 세션을 붙든다
    ///   ([`crate::report::StatusReport::judged`] 가 `warnings` 에 접어 넣는 그것이다). 시간대를
    ///   빼면 자정을 넘긴 기한이 경고에서 빠져, 붙들 판을 안 붙든다
    /// - **훅의 stderr 는 아무도 안 읽는다**(2026-09-23 측정, moai-j4ie 가 `skill::command` 에
    ///   표로 적어 둔 그 측정이다) — 종료 0 이든 1 이든 `claude` 의 스트림에 안 서고 `--debug`
    ///   로도 안 선다. 그러니 이 줄이 훅에서 내는 값은 `hook_response` 에 적히는 몇 바이트고,
    ///   그것을 접으려고 "이 판이 훅인가" 를 읽는 자를 `main` 밖에 하나 더 두지 않는다
    ///
    /// **`project ls` 는 아예 안 든다** — 그쪽은 `counts` 만 쓰고 시간대가 닿는 셈은 기한 판정
    /// 하나뿐이라, [`crate::report::status_unjudged`] 로 세면 답이 같다(`cmd::project::state`).
    pub fn zone(&self) -> &crate::tz::Zone {
        &self.zone.get_or_init(crate::tz::Zone::system).0
    }

    /// 시간대를 풀다 만난 것. `None` 이면 아무 일 없다. **[`Ctx::zone`] 을 부른 뒤에 든다** —
    /// 안 부른 판은 시각을 안 그리므로 할 말도 없다.
    pub fn zone_trouble(&self) -> Option<&crate::tz::Trouble> {
        self.zone.get().and_then(|(_, why)| why.as_ref())
    }
}

/// 이미 읽어 든 설정에서 화면 말을 고른다 — [`Ctx::lang`] 과 **같은 자**다. 제 손으로
/// `user_config::read` 를 부르는 길(`cmd::tui::outside`)이 말을 물으려고 `ctx.lang()` 을 부르면
/// [`Ctx::registry`] 가 같은 파일을 한 번 더 읽는다(moai-u8cs 가 걷어 낸 바로 그것이다). 고르는
/// 규칙을 두 벌로 적지 않으려고 여기 한 자리에 둔다.
pub fn lang_of(reg: &crate::user_config::Registry) -> crate::i18n::Lang {
    let env = std::env::var("MOAI_LANG").ok();
    crate::i18n::pick(env.as_deref(), reg.lang.as_deref())
}

/// 할 수 있는 것은 다 하고, 된 것과 안 된 것을 둘 다 보고한 뒤 비영 종료한다.
/// 이 깃발이 서면 결과를 다 낸 **뒤에** 종료 코드가 1 이 된다.
static PARTIAL: AtomicBool = AtomicBool::new(false);

/// `none` 은 "비운다" 는 뜻이다. 제목이 `none` 인 이슈를 만들 일은 없다.
/// `add` 와 `edit` 이 같은 낱말을 써야 한다 — 한쪽만 알면 방금 만든 이슈를
/// 같은 말로 비우지 못한다.
///
/// **`none` 만 본다.** 빈 값이나 앞뒤 공백까지 여기서 접으면 `-e ""` 가 거절에서
/// 비우기로, `-e " <id> "` 가 거절에서 통과로 조용히 바뀐다 — 담당 때문에 옮긴
/// 헬퍼가 에픽·마일스톤의 뜻을 같이 바꾸는 것은 범위 밖이다. 담당 쪽 공백은
/// `model::split_assignee` 가 접는다.
pub fn clearable(v: &str) -> Option<String> {
    (v != "none").then(|| v.to_string())
}

/// **있는 파일이고 내가 그것을 돌릴 수 있는가.** 재는 것은 딱 그것이다.
///
/// **`is_file` 만 보던 판은 거짓말을 한다**(`skill`): 실행 권한이 빠진 파일을 "있다" 고 했고,
/// 훅은 그때 권한 오류를 `|| exit 0` 으로 삼켜 아무 말 없이 아무것도 안 했다.
///
/// **"내가 돌릴 수 있는가" 로 잰다**(moai-dhx9, 2026-09-23 사용자 결정). 한때 `mode & 0o111` 만
/// 보았는데 그것은 "아무나 돌릴 수 있는가" 라, 남의 소유 `0o700` 이나 `noexec` 에 얹힌 파일이
/// 여기를 지나는데 껍데기는 126 을 냈다(`merge_driver::probe` 가 적어 둔 그대로다). 그 값이 제일
/// 비싼 자리가 `skill` 이었다 — 훅은 그 126 을 `|| exit 0` 으로 삼켜 규칙 넷이 조용히 안 서는데
/// `moai skill status` 는 "깔렸다" 고 말했다. 화면과 사실이 갈리던 자리다. **훅 쪽도 그 뒤에
/// 말하게 됐다**(moai-j4ie) — `skill::command` 가 0·1 이 아닌 종료에 알림 한 줄을 낸다
/// (moai-wnnb 가 126·127 에서 넓혔다). **다만 세션·이벤트·종료 값마다 한 번뿐이라**(moai-f7up,
/// 리뷰 moai-514e.hgz) 첫 줄을 놓친 사람에게 남는 자리는 여전히 여기다 — 이 함수가 대는 몫을
/// 훅이 대신하지 않는다. `tests/cli.rs` 의 `skill_status_notices_a_vanished_hook_binary` 가
/// 같은 계약을 적어 둔 짝이고, 한쪽만 고치면 둘이 갈린다.
///
/// 그래서 `access(X_OK)` 를 부른다. 표준 라이브러리에 없어 `libc` 를 직접 의존으로 들였고, 든
/// 값은 `Cargo.toml` 의 그 줄에 적어 두었다 — **크레이트는 0개가 늘었다.**
///
/// **`access` 는 실제 uid·gid 로 잰다** — 유효 uid 가 다른 setuid 프로그램이면 답이 갈린다.
/// moai 는 setuid 로 안 돌고, 그렇게 도는 날에는 여기가 아니라 그 결정이 먼저 틀린 것이다.
///
/// **디렉터리는 위의 `is_file` 이 뺀다** — `access(X_OK)` 는 들어갈 수 있는 디렉터리에도 0 을
/// 내므로, 그 문을 걷으면 `PATH` 앞자리의 `moai` 라는 디렉터리를 셋이 다 골라 든다.
///
/// **못 재는 자리는 "안 돈다" 로 답한다** — 경로에 NUL 이 든 것은 파일 이름이 될 수 없다.
///
/// **세 벌이던 것을 모았다**(moai-p3kb, 리뷰가 셋째를 짚었다) — `skill` 의 `runnable`,
/// `tui` 의 `executable`, `merge_driver` 의 `runnable`. 갈리면 이식성 고침 하나가 고친 사람이
/// 기억하는 한 곳에만 든다. 유닉스가 아닌 데서는 실행 비트가 없어 있는 파일이면 그만이다 —
/// 세 벌 다 그렇게 적혀 있었다.
///
/// **이름으로 PATH 를 묻는 세 자리는 여기 안 든다 — 셋이 저마다 딴 물음이다.**
///
/// - `skill` 은 껍데기의 `command -v` 로 묻는다. 훅이 실제로 치는 것이 그 명령이라 껍데기가 보는
///   것(함수·별칭까지)과 같아야 한다
/// - `tui` 는 기본 편집기 둘(`vi`·`nano`)을 고르려고 `PATH` 를 제 손으로 훑는다. 띄우는 것은
///   여기도 `sh` 라([`crate::tui::jotfile::argv`]) 껍데기와 답이 같아야 하므로 **빈 자리를 안
///   뺀다** — POSIX 가 그 자리를 "지금 자리" 로 읽고, 껍데기도 그렇게 찾는다. `$VISUAL`·`$EDITOR`
///   가 둘 다 비었을 때 띄우며 한 번, 많아야 이름 둘이라 `sh` 를 띄우는 값이 아깝다
/// - `merge_driver` 는 같은 훑기에 **빈 자리를 뺀다.** 거기서 고른 값은 `.git/config` 에 앉아
///   **딴 자리에서** 풀리므로, 껍데기가 지금 그 자리를 쓰는 것과 심어 둘 값으로 쓰는 것이 다르다.
///   그 위에 `--help` 를 실제로 불러 한 겹 더 잰다(`probe`) — 심는 값은 딴 때에 풀리니, 지금
///   돌릴 수 있는가만으로는 모자란다
pub fn runnable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        // 아래 `#[cfg(not(unix))]` 갈래와 **같은 낱말로 적는다** — `Path::is_file` 이 곧
        // `metadata(..).map(|m| m.is_file()).unwrap_or(false)` 라, 두 갈래를 견주는 사람이
        // 같은지 증명하지 않고 그냥 보면 된다.
        if !path.is_file() {
            return false;
        }
        let Ok(spelt) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `spelt` 는 이 줄이 끝날 때까지 살아 있는 널로 끝나는 버퍼고, `access` 는 그것을
        // 읽기만 한다.
        unsafe { libc::access(spelt.as_ptr(), libc::X_OK) == 0 }
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn note_partial() {
    PARTIAL.store(true, Ordering::Relaxed);
}
pub fn had_partial() -> bool {
    PARTIAL.load(Ordering::Relaxed)
}

/// 제 저장소를 읽고, `worktree` 면 다른 워크트리를 겹친다(`worktree::gather`).
///
/// **남의 워크트리에서 만난 문제는 말만 한다** — stderr 로 한 줄씩, 부분 실패
/// 깃발은 안 세운다. 옆 워크트리의 깨진 줄로 `moai status` 가 비영 종료하면
/// 제 파일은 멀쩡한데 도구가 실패로 읽힌다. 제 파일의 못 읽는 줄은 전과 같이
/// `load.errors` 에 남아 부르는 쪽이 제 길로 알린다.
/// **말은 여기서 고른다**(moai-dpbi) — `worktree::gather` 는 자료만 낸다(`worktree::Trouble`).
/// 이 줄은 `moai status` 의 첫 화면 곁에 서므로, 그 화면과 같은 말이어야 한다.
pub fn gather(ctx: &Ctx, repo: &crate::store::Repo, worktree: bool) -> R<crate::worktree::Gathered> {
    let g = crate::worktree::gather(repo, worktree)?;
    for t in g.unfound.iter().chain(&g.trouble) {
        eprintln!("{}", crate::view::trouble_line(ctx.lang(), t));
    }
    Ok(g)
}

/// `.moai` 밖에서 부른 `ready` 가 볼 등록한 프로젝트. 프로젝트마다 연다.
///
/// **등록한 것이 없으면 실패한다** — 대신 등록하는 길을 곁에 댄다. `status` 는 이것을
/// 안 지나고 같은 말을 내며 0 으로 끝난다(moai-ynsb): 세션의 시작점이 제 파일 아닌 것으로
/// 실패해 보이면 안 된다.
///
/// 사용자 설정의 문제는 등록한 것이 있을 때는 화면이 한 줄씩 비추고(`problems`),
/// 없을 때는 실패 말에 붙는다 — 목록이 빈 까닭이 그것일 수 있다.
///
/// `worktree` 면 프로젝트마다 옆 워크트리를 겹친다. 옆에서 만난 문제는 stderr 가 아니라
/// 그 프로젝트의 줄(`Project::trouble`)이 말한다 — 여럿을 한 번에 보는 화면에서 stderr 의
/// 한 줄은 어느 프로젝트의 것인지 모른다.
pub fn registered(ctx: &Ctx, worktree: bool) -> R<(&crate::user_config::Registry, Vec<crate::projects::Project>)> {
    let reg = ctx.registry();
    if reg.projects.is_empty() {
        return Err(nothing_registered(reg, ctx.lang()));
    }
    let projects = crate::projects::open_with(reg, worktree, ctx.lang());
    Ok((reg, projects))
}

/// 이 명령이 설 저장소 — `.moai/` 를 가진 디렉터리를 위로 찾는다([`crate::store::Repo::find`]).
///
/// **못 찾았다는 말은 이 층이 짓는다**(moai-5j49, 2026-09-21 사용자 결정). 저장 계층은 화면 말을
/// 모른 채 둔다(2026-09-20) — 거기서 글을 지으면 `Repo::find_from` 과 `with_write` 의 서명에 화면
/// 말이 번지고, 설정 없이 도는 머지 드라이버까지 닿는다. 그래서 찾기만 저쪽에 두고 말은 여기서 짓는다.
///
/// **다른 곳의 저장소를 부르는 길(`-C`)을 함께 댄다** — 등록한 프로젝트를 한눈에 보는 `status` 에
/// 익은 사람은 `.moai` 밖에서 `add`·`mv` 도 될 줄 알고, 쓰는 명령은 어느 프로젝트인지 모르니
/// 멈추는 것이 맞다(moai-6au6).
///
/// **[`nothing_registered`] 와 같은 키를 쓴다** — 등록한 것이 없는 판은 그 줄 밑에 한 줄을 더
/// 얹을 뿐이라, 두 자리가 앞줄을 달리 말하면 같은 처지가 두 글로 선다.
///
/// **말은 [`Ctx`] 째로 받아 닫힘 안에서 푼다**(리뷰) — `lang` 을 인자로 받으면 러스트가 부름
/// **앞에서** 그것을 셈해, 찾기가 이기는 판(= 거의 모든 판)에도 [`Ctx::lang`] 이 사용자 설정을
/// 열어 파싱한다. 그 자리가 [`Ctx::lang`] 의 "늦게 읽는다" 와 `mv`·`defer`·`edit` 이 저마다
/// 적어 둔 "말은 거절할 때만 푼다" 가 막던 바로 그것이다 — 이 문 하나가 열한 명령을 한꺼번에
/// 그쪽으로 끌고 간다. 닫힘 안이면 `.moai` 를 못 찾은 판에서만 푼다.
pub fn open_repo(ctx: &Ctx) -> R<crate::store::Repo> {
    crate::store::Repo::find(|| ctx.lang())?.ok_or_else(|| Fail::new(crate::i18n::say(ctx.lang(), "refuse.not_a_repo")))
}

/// `.moai` 밖인데 등록한 것도 없을 때의 말 — `ready` 는 이 말로 멈추고, `status` 는
/// 같은 말을 내고 0 으로 끝난다(moai-ynsb).
/// 화면의 `tui` 는 멈추지 않고 빈 층에서 `SPC p a` 를 댄다(moai-r8kl).
/// 목록이 빈 까닭이 사용자 설정의 문제일 수 있어 그것도 붙인다.
pub fn nothing_registered(reg: &crate::user_config::Registry, lang: crate::i18n::Lang) -> Fail {
    // **한 덩이가 한 말로 선다**(moai-5j49) — 앞줄이 `store` 의 것이라 한국어로 서던 자리다.
    // 이제 둘 다 말묶음에서 오고, 앞줄은 [`open_repo`] 가 홀로 쓸 때와 **같은 키**다.
    let mut msg = format!(
        "{}\n{}",
        crate::i18n::say(lang, "refuse.not_a_repo"),
        crate::i18n::say(lang, "opening.nothing_registered")
    );
    // 사람의 설정 파일에서 온 글이다 — 제어문자를 걷고 한 줄로 접는다. 이 말은 줄 단위로
    // 읽히므로(`fail` 이 그대로 stderr 에 쓴다) 여러 줄이 섞이면 어디까지가 한 까닭인지 흐려진다.
    // **읽는 문은 [`crate::view::settings_problems`] 하나다**(리뷰) — 화면 말의 탈은 `problems` 가
    // 아니라 `lang_problems` 에 자료로 서므로(moai-dpbi), `reg.problems` 를 그냥 훑으면 `lang` 오타가
    // 이 줄에서만 조용히 사라진다.
    for p in crate::view::settings_problems(reg, lang) {
        msg.push_str(&format!("\n{}", crate::text::one_line(&p)));
    }
    Fail::new(msg)
}

/// 읽다 만난 잘못된 줄을 stderr 로 알린다. 결과는 그대로 낸다.
pub fn report_load_errors(lang: crate::i18n::Lang, path: &std::path::Path, errors: &[crate::store::LoadError]) {
    if errors.is_empty() {
        return;
    }
    note_partial();
    name_load_errors(lang, path, errors);
}

/// 같은 말을 하되 **부분 실패 깃발은 안 세운다.** 읽기가 답을 덜 낸 자리
/// (`show`·`ready`)는 비영 종료가 맞지만, 쓰기와 짝을 이루는 자리(`promote`
/// 의 연습)는 진짜 실행이 그 줄 때문에 멈추지 않으므로 연습만 실패로 끝나면
/// 안 된다 — 그것을 거절로 읽은 쪽은 도구가 기꺼이 해 줄 계획을 버린다.
///
/// **이 줄도 말묶음에서 온다**(moai-dpbi 리뷰) — 바로 곁에서 `gather` 가 옆 워크트리의 같은
/// 사실을 고른 말로 내므로(`cmd::gather`), 여기만 한국어면 한 명령의 stderr 가 두 말로 선다.
pub fn name_load_errors(lang: crate::i18n::Lang, path: &std::path::Path, errors: &[crate::store::LoadError]) {
    use crate::i18n::{fill, say};
    if errors.is_empty() {
        return;
    }
    let at = path.display().to_string();
    eprintln!("{}", fill(say(lang, "warn.unreadable_file"), &[("at", &at), ("n", &errors.len().to_string())]));
    for e in errors.iter().take(5) {
        eprintln!("{}", fill(say(lang, "warn.unreadable_at"), &[("line", &e.line.to_string()), ("why", &e.message)]));
    }
    if errors.len() > 5 {
        eprintln!("{}", fill(say(lang, "warn.unreadable_more"), &[("n", &(errors.len() - 5).to_string())]));
    }
}

pub fn run(mut cli: Cli) -> R<Vec<String>> {
    let ctx = Ctx::new(cli.json, cli.user.take(), cli.dir.is_some());
    let out = dispatch(&ctx, cli);
    // **시간대를 못 풀었으면 한 줄로 알린다**(moai-77ap) — 막지 않는다. 종료 코드도 안 건드리고,
    // `--json` 은 화면 글을 안 내므로 stderr 뿐이다. 시각을 그린 명령만 이 자리에 닿는다:
    // [`Ctx::zone`] 을 안 부른 판은 할 말이 없다([`Ctx::zone_trouble`]).
    //
    // **한 줄뿐이다.** 정적 musl 판을 zoneinfo 없는 기계에 받으면 이 일이 **매 명령**에 나므로,
    // 고치는 법까지 늘어놓으면 그 기계에서는 모든 출력에 안내문이 한 뭉치씩 붙는다.
    if let Some(why) = ctx.zone_trouble() {
        eprintln!("{}", crate::view::zone_trouble(ctx.lang(), why));
    }
    out
}

fn dispatch(ctx: &Ctx, cli: Cli) -> R<Vec<String>> {
    let Some(cmd) = cli.cmd else {
        return opening(ctx);
    };
    match cmd {
        // 새 명령을 두지 않고 `init` 의 플래그로 둔다 — 고치는 길(`init`)과 보는 길이 한 이름에 있어야
        // `stale` 을 본 사람이 무엇을 칠지 안다(moai-mstm).
        Cmd::Init { check: true, .. } => init::check(ctx),
        // 붙여 넣을 글을 내는 길도 같은 이름 밑이다 — 까닭은 `init::print` 에 있다.
        Cmd::Init { print: true, .. } => init::print(ctx),
        // 필드를 다 적는다 — `..` 로 받으면 `init` 에 새 플래그를 더해도 여기서 조용히 버려진다.
        Cmd::Init { prefix, no_agents, no_driver, check: false, print: false } => {
            init::run(ctx, prefix.as_deref(), no_agents, no_driver)
        }
        Cmd::Hook { event } => hook::run(ctx, event),
        // **저장소를 안 찾는다** — git 이 주는 것은 임시 파일 셋이고, 답을 쓰는 자리도
        // 그중 하나다. `.moai` 를 찾으러 가면 `git worktree` 안이나 서브모듈에서
        // 엉뚱한 트래커를 열고, 사람이 누구인지도 여기서는 물을 일이 없다.
        Cmd::MergeDriver(a) => merge_driver::run(ctx, a),
        Cmd::Skill(SkillCmd::Install { scope, dry_run }) => skill::install(ctx, scope.as_str(), dry_run),
        Cmd::Skill(SkillCmd::Status) => skill::status(ctx),
        Cmd::Skill(SkillCmd::Uninstall { dry_run }) => skill::uninstall(ctx, dry_run),
        // 저장소가 아니라 사람의 설정을 고친다 — `cmd::open_repo` 를 안 지나므로
        // `.moai` 밖에서도 선다.
        Cmd::Project(ProjectCmd::Add { path }) => project::add(ctx, &path),
        Cmd::Project(ProjectCmd::Ls) => project::ls(ctx),
        Cmd::Project(ProjectCmd::Rm { path }) => project::rm(ctx, &path),
        Cmd::Project(ProjectCmd::Color { path, hue }) => project::color(ctx, &path, &hue),
        Cmd::Add(a) => add::run(ctx, a, None),
        Cmd::Show(a) => show::run(ctx, a, None),
        Cmd::Mv(a) => mv::run(ctx, a),
        Cmd::Edit(a) => edit::run(ctx, a),
        Cmd::Rm(a) => rm::run(ctx, a),
        Cmd::Note(a) => note::run(ctx, a),
        Cmd::Link(a) => link::run(ctx, a),
        Cmd::Defer(a) => defer::run(ctx, a),
        Cmd::Read(a) => read::run(ctx, a),
        Cmd::Ready(w) => ready::run(ctx, w.worktree),
        Cmd::Prime(w) => prime::run(ctx, w.worktree),
        Cmd::Status(w) => status::run(ctx, w.worktree),
        Cmd::Tui(a) => tui::run(ctx, a),
        Cmd::Issue(t) => typed(ctx, t, Kind::Issue),
        Cmd::Epic(t) => typed(ctx, t, Kind::Epic),
        Cmd::Milestone(t) => typed(ctx, t, Kind::Milestone),
        // **공통 동사는 `typed()` 를 지난다**(moai-g33x) — 여기서 `add`·`show` 를 다시 적으면
        // `Typed` 에 동사를 더하는 날 idea 만 조용히 안 따라온다.
        Cmd::Idea(IdeaCmd::Common(t)) => typed(ctx, t, Kind::Idea),
        Cmd::Idea(IdeaCmd::Promote(a)) => idea::promote(ctx, a),
    }
}

/// 인자 없이 불렀을 때. **오류가 아니다.**
///
/// 저장소 안이면 `status` — 세션의 시작점이 그것이고, 그래서 `bd prime` 같은
/// 명령을 따로 두지 않았다. 밖이면 도움말과 `init` 안내.
///
/// 처음 만난 쪽이 알아야 할 것은 둘이다: 지금 무슨 상태인가, 다음에 무엇을
/// 치는가. 둘 다 여기서 준다.
fn opening(ctx: &Ctx) -> R<Vec<String>> {
    use clap::CommandFactory;
    let found = crate::store::Repo::find(|| ctx.lang());
    // `.moai` 밖이어도 등록한 프로젝트가 있으면 한눈 보기가 곧 시작점이다 (`status` 가
    // 그 길로 간다).
    //
    // **설정이 깨진 저장소 안(`Err`)은 도움말이 아니다** — 아래 `status` 로 가서 다른 명령과
    // 같은 말·같은 종료 코드로 그 설정을 댄다. 도움말로 접으면 "아직 moai 저장소가 아니다"
    // 를 믿은 사람이 제 저장소에 `init` 을 다시 친다 (moai-byih).
    let outside = matches!(found, Ok(None));
    let reg = outside.then(|| ctx.registry());
    let registered = reg.is_some_and(|r| !r.projects.is_empty());
    if outside && !registered {
        let mut help = Vec::new();
        crate::cli::Cli::command().write_help(&mut help).map_err(|e| Fail::new(e.to_string()))?;
        let mut out: Vec<String> = String::from_utf8_lossy(&help).lines().map(str::to_string).collect();
        out.push(String::new());
        // **이 에픽이 내건 첫 화면이 이것이다**(리뷰 moai-hom6.qd9 4번). `.moai` 밖에서 맨 `moai` 를
        // 친 사람이 가장 먼저 읽는 두 줄인데, 앞의 도움말이 영어로 선 채 여기만 한국어였다.
        // 말은 이미 위에서 설정을 연 판이라(`ctx.registry()`) 새로 여는 것이 없다.
        out.push(crate::i18n::say(ctx.lang(), "opening.not_a_repo_yet").to_string());
        out.push(crate::i18n::say(ctx.lang(), "opening.register_to_view").to_string());
        // **목록이 빈 까닭이 설정의 문제면 그것을 댄다.** 세션은 여기서 시작하는데, 설정이
        // 깨져 등록한 것이 안 읽힌 사람에게 "등록한 것이 없다, 더하라" 만 하면 정반대를
        // 믿고 깨진 파일에 `project add` 를 친다. `status`·`ready` 는 이미 이 줄을 대고
        // (`nothing_registered`), `tui --json` 은 `problems` 에 싣는다. 도움말 자리라 종료 코드는 그대로 0 이다.
        for p in reg.iter().flat_map(|r| crate::view::settings_problems(r, ctx.lang())) {
            out.push(crate::style::paint(crate::style::WARN, &format!("! {}", crate::text::one_line(&p))));
        }
        return Ok(out);
    }

    let mut out = status::run(ctx, false)?;
    if ctx.json {
        return Ok(out);
    }
    out.push(String::new());
    let here = !registered && std::path::Path::new("AGENTS.md").exists();
    // **이 꼬리도 말묶음에서 온다**(리뷰) — 바로 위의 `status` 가 통째로 제 말로 나오는데
    // 여기만 한국어로 박혀 있으면, 세션이 가장 많이 치는 맨몸 `moai` 의 **마지막 줄**이
    // 화면과 다른 말로 선다. **키가 둘인 것은 AGENTS.md 를 댈지 말지가 여기서 정하는
    // 것**이라서다 — 한 키에 넣으면 그 말에서만 빈 꼬리가 남는다(`warn.idea_pile` 과 같다).
    let lang = ctx.lang();
    let tail = match here {
        true => crate::i18n::say(lang, "opening.commands_here"),
        false => crate::i18n::say(lang, "opening.commands"),
    };
    out.push(crate::style::paint(crate::style::DIM, tail));
    Ok(out)
}

fn typed(ctx: &Ctx, cmd: Typed, kind: Kind) -> R<Vec<String>> {
    match cmd {
        Typed::Add(a) => add::run(ctx, a, Some(kind)),
        Typed::Show(a) => show::run(ctx, a, Some(kind)),
    }
}

/// 제목 자리에 온 것이 사실은 오타 난 플래그인가.
///
/// `allow_hyphen_values` 는 모르는 하이픈 토큰을 전부 제목으로 삼킨다.
/// `--json 이 tags 를 빠뜨린다` 같은 제목을 받으려고 켠 것인데, 그 대가로
/// `moai add --dryrun` 이 제목 `"--dryrun"` 인 이슈를 조용히 만든다.
///
/// **띄어쓰기가 가른다.** 사람이 쓰는 제목은 낱말이 여럿이고, 오타 난
/// 플래그는 한 낱말이다. 정말 그 제목을 쓰겠다면 `--` 로 넘긴다.
pub fn refuse_if_flag_like(title: &str, lang: crate::i18n::Lang) -> R<()> {
    // `--` 를 쓴 사람은 "이 뒤는 플래그가 아니다" 라고 이미 말한 것이다.
    //
    // argv 를 다시 훑는 것이 `--json` 때는 틀렸지만 여기서는 맞다 — `--` 는
    // 값이 아니라 구분자라 clap 이 언제나 삼키고, argv 에 남아 있다는 것은
    // 사용자가 그것을 적었다는 뜻 말고 다른 뜻이 없다.
    if std::env::args().any(|a| a == "--") {
        return Ok(());
    }
    if title.starts_with("--") && !title.contains(char::is_whitespace) {
        return Err(Fail::coded(
            format!(
                "{}\n                       {}",
                crate::i18n::fill(crate::i18n::say(lang, "refuse.title_looks_like_a_flag"), &[("title", title)]),
                crate::i18n::fill(crate::i18n::say(lang, "refuse.title_after_dashes"), &[("title", title)]),
            ),
            code::BAD_INPUT,
        ));
    }
    Ok(())
}

/// `--json` 일 때 한 줄로 낸다.
///
/// **`serde_json::Value` 를 거치지 않는다.** `Value` 의 맵은 정렬돼 있어
/// 키가 알파벳 순으로 재배열되고, 그러면 파일과 `--json` 이 서로 다른 순서를
/// 말한다. 눈으로 훑을 때 `id` 가 줄 가운데에 있는 것도 그 탓이다.
pub fn json_line<T: serde::Serialize>(v: &T) -> R<Vec<String>> {
    serde_json::to_string(v).map(|s| vec![s]).map_err(|e| Fail::new(e.to_string()))
}

/// 이슈 한 줄의 기계 출력. 묶음이면 **멤버에서 읽은 칸**을 `derived_status` 로
/// 곁들인다 (`report::group_states`).
///
/// `status` 의 뜻은 안 바꾼다 — 파일에 적힌 값 그대로다. 이미 나간 계약이라,
/// 그 키를 읽은 칸으로 바꾸면 `--json` 을 읽고 되쓰는 쪽이 읽은 값을 적힌
/// 값으로 믿는다. 사람 화면이 `-s` 로 고르고 그리는 칸이 이 키다.
///
/// **줄을 내는 모든 명령이 이것을 지난다.** `show` 만 곁들이면 `add`·`defer`·`link` 를
/// 읽는 쪽은 같은 에픽을 적힌 칸으로 읽는다 — 키가 없다는 것이 "묶음이 아니다" 라는 뜻이다.
///
/// **`rm --json` 만 예외다**(리뷰 moai-fqnr.sqb). 그 `removed` 는 지운 줄의 **파일에 있던 그대로**다 —
/// 파일에서 사라진 줄의 마지막 기록이라, 여기서 걷으면 사용자가 같은 이름으로 둔 제 필드가 어디에도
/// 안 남는다. 덧붙이는 키도 없으니 한 객체에 같은 키가 둘 서지도 않는다.
#[derive(serde::Serialize)]
pub struct Row<'a> {
    #[serde(flatten)]
    pub issue: std::borrow::Cow<'a, crate::model::Issue>,
    /// 줄이 **기본값이라 안 적은** 종류와 우선순위(moai-51it·moai-a4u9). 파일이 기본값을 안 적는
    /// 것은 1만 줄이 통째로 diff 에 뜨는 것을 막으려는 것이고, 그 침묵의 뜻은 파일을 쓰는 쪽만
    /// 안다 — 읽는 쪽에서는 `jq -r .priority` 가 `null` 을 받아 "p2" 와 "모른다" 가 한 값이 된다.
    /// **파일에 안 적는 것과 `--json` 이 안 내는 것은 다른 일이다**(2026-09-21 사용자 결정).
    /// 사람 화면은 같은 줄을 늘 `p2` 로 그려 왔다.
    ///
    /// **줄이 그 키를 제 몸에 들고 있으면 여기는 비운다** — 한 객체에 같은 키가 둘 서지 않는다.
    /// 그래서 값이 적힌 줄의 출력은 한 글자도 안 바뀐다.
    ///
    /// 없을 수 *있는* 키(`epic`·`milestone`·`deferred_at`·`assignee`)는 여기 안 든다 — 그쪽은 키가
    /// 없다는 것이 곧 뜻이다(moai-fqnr). 이 둘은 없을 수가 없다.
    ///
    /// **`epic` 은 그 약속이 반쪽이다**(moai-exh7 뒤). 계획이 세우는 멤버는 소속을 id 에 지고
    /// `epic` 을 안 적으므로, 이 줄에서 키가 없다는 것은 "에픽이 없다" 가 아니라 "여기에는 안
    /// 적혔다" 다 — 답은 `report::groups` 가 안다. 그 답은 [`Row::derived_epic`] 이 낸다
    /// (moai-wuzi, 2026-09-23 사용자 결정): `epic` 은 **파일에 적힌 그대로** 두고, 푼 값은
    /// `derived_status` 처럼 제 키로 곁들인다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_status: Option<&'a str>,
    /// 그 줄이 **든 에픽** — 적어 놓았든 id 로 졌든(`report::groups`). 소속을 묻는 기계는
    /// 이 키 하나만 본다(moai-wuzi).
    ///
    /// **없다는 것은 "어느 에픽에도 안 든다" 는 뜻이다** — `epic` 키의 침묵과 다르다. 그쪽은
    /// "여기에는 안 적혔다" 이고, 계획이 세우는 멤버는 늘 그쪽이 빈다. 한때 `show --json` 은
    /// 그 멤버를 에픽 없는 줄로 내고 `prime --json` 은 물려받은 소속을 내, 한 바이너리의 두
    /// 기계 표면이 "이 줄은 어느 에픽인가" 에 다른 답을 했다.
    ///
    /// **묶음 줄에는 안 선다**(moai-fg0t) — 에픽 줄이 든 `epic` 은 소속이 아니다. 트리도
    /// `-e` 도 그 줄을 에픽 밑에 두지 않으므로, 여기에 대면 적은 적 없는 소속이 선다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_epic: Option<&'a str>,
    /// 다른 워크트리에서 온 줄이면 그 브랜치 (`--worktree`). **키가 없다는 것이 곧
    /// "지금 브랜치의 줄" 이다** — `derived_status` 와 같은 약속이다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<&'a str>,
}

/// **줄 하나의 `--json` 에 moai 가 덧붙이는 키 전부**(moai-qn5d) — 늘 붙이는 것도, 조건에 따라
/// 붙이는 것도, 어느 명령이 붙이는 것도. 이 이름의 모르는 필드는 [`Row::of`] 가 걷는다.
///
/// **한 목록이다.** 명령마다 목록을 두면 걷기가 그 명령에만 선다 — `show <id> --json` 만 걷던
/// 판에는 되쓴 줄을 `ready`·`show` 목록·`edit` 가 그대로 펴서, 막음 없는 일에 옛 `blockers` 가,
/// 멀쩡한 git 옆에 옛 `commits_error` 가 섰다. 키를 더하면 여기에 더한다 — 빠지면 시험이 그
/// 이름을 대며 붉어진다: `show` 가 덧붙이는 것은 [`json_with`] 가(`Row` 의 [`Appendable`] 이 이
/// 목록이다), `Row` 의 제 필드와 `edit` 의 `Out` 은 저마다의 시험이 잡는다.
///
/// 사용자가 같은 이름으로 둔 제 필드도 **줄 출력에서** 숨는다 — 받은 값이다(2026-09-18 사용자
/// 결정). 모르는 필드 보존은 파일의 약속이고, 출력의 약속은 우리 키가 참이라는 것이다. 파일은
/// 그대로다(`moai status` 가 비춘다).
pub const OURS: &[&str] = &[
    // `Row` 가 제 필드로 곁들이는 것.
    "derived_status",
    "derived_epic",
    "branch",
    // 줄이 기본값이라 안 적었을 때 `Row` 가 세우는 것. 줄의 제 키라 모르는 필드로 들어올 일은
    // 없지만, 걷는 목록은 **출력에 서는 우리 키 전부**다 — 여기서 빼면 `Row` 에 필드를 더하는
    // 것을 잡는 시험이 이 둘만 못 잡는다.
    "kind",
    "priority",
    // `show <id> --json` 이 덧붙이는 것(`json_with`).
    "children",
    "journal",
    "members",
    "shelved_by",
    "duplicate_lines",
    "blockers",
    "workplaces",
    "place",
    "commits",
    "commits_error",
    "journal_error",
    "work",
    "spent",
    // `edit --json` 이 곁들이는 남은 소속.
    "inherited_epic",
    "inherited_milestone",
];

/// 기계에 낼 **못 읽은 저널** — `show --json` 의 `journal_error` 한 자리(moai-f2lc).
///
/// `commits_error` 와 같은 꼴이고 같은 까닭이다(`git::Told`): `kind` 가 가르는 자이고 `said` 는
/// 사람이 읽을 한 줄이다. **다른 것은 배열이라는 것뿐** — git 은 한 번 물어 한 번 지지만, 저널은
/// 사람마다 갈린 N 개라 한 판에 여럿이 안 읽힐 수 있고, 그중 하나만 대면 나머지는 조용해진다.
///
/// **경로를 안 자른다.** `commits_error` 는 자르는데(같은 JSON 의 `workplaces` 가 자르므로) 여기는
/// 그 자리가 곧 고치는 법이다 — `chmod` 를 어디에 하는지가 이 값의 쓸모다.
#[derive(Debug, serde::Serialize)]
pub struct JournalError {
    /// `permission`·`failed`([`crate::store::Unread::kind`]). 받는 쪽이 갈라 읽는 것은 이것이다.
    pub kind: &'static str,
    /// 사람이 읽을 한 줄 — `main` 이 stderr 로 내는 줄과 **같은 글이다**. 두 자리에서 따로
    /// 지으면 같은 실패를 화면과 `--json` 이 다른 말로 말한다.
    pub said: String,
}

/// 못 읽은 저널을 기계 꼴로 접는다. `root` 를 주면 **그 저장소의 것만** 고른다 —
/// `show --worktree` 는 겹쳐 온 줄의 이력을 그 워크트리의 저널에서 읽으므로(`show::home`),
/// 한 판의 목록에 여러 체크아웃의 자리가 섞인다. 줄 곁에 다는 키는 **그 줄의 뿌리**의 것이라야
/// "이 줄의 이력이 덜 왔다" 라는 말이 된다. `None` 이면 다 든다(`main` 의 stderr).
///
/// **순수하다** — 전역을 안 읽는다. 읽는 자는 부르는 쪽이다.
pub fn journal_errors(
    lang: crate::i18n::Lang,
    unread: &[crate::store::Unread],
    root: Option<&std::path::Path>,
) -> Vec<JournalError> {
    unread
        .iter()
        .filter(|u| root.is_none_or(|r| u.root == r))
        .map(|u| JournalError {
            kind: u.kind,
            said: crate::i18n::fill(
                crate::i18n::say(lang, "warn.unread_journal"),
                &[("at", &u.at.display().to_string()), ("why", &u.said)],
            ),
        })
        .collect()
}

impl<'a> Row<'a> {
    /// `read` 는 그 줄의 읽은 칸이다. **묶음이 아니면 버린다** — 머지를 잘못 푼
    /// 파일에서 묶음과 id 가 같은 일 줄이 그 묶음의 칸을 입지 않게, `report::column`
    /// 과 같은 자로 묻는다.
    ///
    /// `placed` 는 **소속을 id 에 진 줄**의 답이다(`report::groups_of`) — 줄이 `epic` 을
    /// 적었으면 그 값이 이기므로 부르는 쪽은 지도를 안 지어도 된다([`Row::derived_epic`]).
    ///
    /// `kind_of` 는 **가려진 줄을 가르는 지도**다(`report::Kinds`, moai-53s2) — 위의 소속
    /// 지도가 id 로 짠 것이라, 종류가 다른 쌍둥이에게 가려진 줄은 그 지도에서 쌍둥이의 값을
    /// 받는다. 지도를 안 대려면 `Kinds::no_twins()` 라는 **낱말**을 적어야 하고, 그 자리는
    /// 왜 쌍둥이가 못 서는지를 함께 댄다 — 빈 지도를 그냥 넘기던 꼴은 새 표면이 그대로
    /// 베껴, 시험 전부가 푸른 채로 이 구멍을 다시 연다(리뷰).
    pub fn of(
        issue: &'a crate::model::Issue,
        read: Option<&'a str>,
        placed: Option<&'a str>,
        kind_of: &crate::report::Kinds<'_>,
    ) -> Row<'a> {
        // **차례를 여기서 다시 적지 않는다**(`report::stands_in`) — 적힌 것이 먼저라는 것도,
        // 묶음 줄에는 안 선다는 것도, 가려진 줄에는 안 선다는 것도 이슈의 뜻이라 `report` 가
        // 정한다. 여기 한 벌 더 적으면 `prime` 의 같은 키와 자가 둘이 되고, 그 둘은 언젠가
        // 어긋난다.
        let derived_epic = crate::report::stands_in(kind_of, issue, placed);
        // **이 키들은 우리 것이다**([`OURS`]). `--json` 을 파일에 되써 넣어 그 이름을 모르는
        // 필드로 든 줄이면 화면에서 걷어낸다 — 그대로 두면 한 객체에 같은 키가 둘 서서 깐깐한
        // 파서가 거절하고, 이번에 안 실은 조건부 키는 그 조건이 아닌 지금 옛 값을 말한다.
        // 흔한 길(겹치는 것이 없다)에서는 줄을 복제하지 않는다. 모르는 필드 쪽에서 훑는다 —
        // 거의 모든 줄이 모르는 필드가 없어, 그러면 목록을 한 번도 안 짚는다.
        let issue = match issue.rest.keys().any(|k| OURS.contains(&k.as_str())) {
            false => std::borrow::Cow::Borrowed(issue),
            true => {
                let mut own = issue.clone();
                own.rest.retain(|k, _| !OURS.contains(&k.as_str()));
                std::borrow::Cow::Owned(own)
            }
        };
        // **여기서도 차례를 다시 적지 않는다**(`report::stands_on`) — 묶음만 입는 것도, 가려진
        // 줄에는 안 서는 것도(moai-7iyc.5fz) `report` 가 정한다. `-s` 로 고르는 자와 이 키를 내는
        // 자가 같은 자리에서 갈려야 한 화면이 같은 줄을 두 칸으로 말하지 않는다.
        let derived = crate::report::stands_on(kind_of, &issue, || read);
        // 줄이 안 적은 기본값을 여기서 세운다. 적힌 값은 줄 제 것이 그대로 나간다.
        let kind = issue.kind.is_default().then(|| issue.kind.as_str());
        let priority = issue.priority.is_none().then(|| issue.priority());
        Row { issue, kind, priority, derived_status: derived, derived_epic, branch: None }
    }

    /// 겹쳐 본 줄이면 그 출처를 곁들인다.
    pub fn on(mut self, origin: &'a crate::worktree::Origin) -> Row<'a> {
        self.branch = origin.branch(&self.issue.id);
        self
    }

    /// 락 안에서 챙겨 온 지도(`read_of`)로 짓는다.
    ///
    /// **쓰기 경로에는 쌍둥이가 없다**(리뷰, moai-53s2). `store::with_write` 는 id 가 두 번
    /// 선 파일에 쓰기를 통째로 물리므로(`Trouble::DuplicateId`), 이 줄이 나왔다는 것은 그
    /// 파일에 중복 id 가 없었다는 말이다 — 가려진 줄은 중복 id 로만 생긴다. 그래서 여기서
    /// 종류 지도를 짓지 않는다: 지어도 답을 못 바꾸는데, 그 셈은 **락을 쥔 채** 치른다.
    pub fn from(issue: &'a crate::model::Issue, read: &'a Read) -> Row<'a> {
        Row::of(issue, read.column(&issue.id), read.epic(&issue.id), &crate::report::Kinds::no_twins())
    }
}

/// [`json_with`] 가 키를 덧붙이는 바탕과, **그 바탕에 덧붙일 수 있는 키 전부.** 바탕이 그 이름을
/// 제 몸에 안 들고 있어야 한 객체에 같은 키가 둘 서지 않는다(moai-kgu2).
///
/// 목록을 타입에 매는 까닭 — 부르는 쪽 인자로 받으면 줄이 아닌 것(`Issue` 그대로)도, 줄에
/// [`OURS`] 가 아닌 목록을 준 것도 시험 빌드의 확인을 지나, 걷지 않은 이름이 둘 선다.
pub trait Appendable: serde::Serialize {
    const APPENDED: &'static [&'static str];
}

/// 줄은 [`Row::of`] 가 걷는 그 목록이다.
impl Appendable for Row<'_> {
    const APPENDED: &'static [&'static str] = OURS;
}

/// 시험에서 — `out` 이 줄 `line` 에 **덧붙인** 키. `Row` 와 그것을 편 출력이 더하는 이름이
/// [`OURS`] 에 다 있는지 재는 데 쓴다.
#[cfg(test)]
pub fn keys_beyond<T: serde::Serialize>(line: &crate::model::Issue, out: &T) -> Vec<String> {
    let keys = |v: serde_json::Value| match v {
        serde_json::Value::Object(m) => m.keys().cloned().collect::<Vec<_>>(),
        other => panic!("객체가 아니다: {other}"),
    };
    let bare = keys(serde_json::to_value(line).unwrap());
    keys(serde_json::to_value(out).unwrap()).into_iter().filter(|k| !bare.contains(k)).collect()
}

/// 락 안에서 챙겨 나온, **줄 하나를 기계 꼴로 낼 때 저장소 전체를 봐야 아는 값** —
/// 묶음이 읽은 칸과, 소속을 id 에 진 줄이 든 에픽. 락 밖으로 들고 나가는 모양이라 제
/// 문자열을 쥔다.
///
/// **둘을 한 자리에 묶는다**(moai-wuzi) — 쓰는 명령마다 지도를 따로 세우면 새 표면이
/// 하나를 빠뜨리고, 그러면 `--json` 이 명령마다 다른 말을 한다. 그 어긋남을 고치는 것이
/// 이 필드가 선 까닭이다.
#[derive(Default)]
pub struct Read {
    /// 묶음 id → 멤버에서 읽은 칸(`report::group_states_of`).
    states: BTreeMap<String, String>,
    /// 줄 id → 그 줄이 든 에픽(`report::groups_of`). **`epic` 을 적은 줄도 든다** — 지도를
    /// 지었으면 그 줄에도 값이 선다. 다만 그 값은 안 읽힌다: 줄이 제 몸에 든 것이 먼저라
    /// (`report::stands_in`), 지도를 아예 안 지은 때에도 답이 같다.
    epics: BTreeMap<String, String>,
}

impl Read {
    /// 묶음이 읽은 칸. 묶음이 아니거나 안 챙겼으면 없다.
    pub fn column(&self, id: &str) -> Option<&str> {
        self.states.get(id).map(String::as_str)
    }

    /// 소속을 id 에 진 줄이 든 에픽.
    pub fn epic(&self, id: &str) -> Option<&str> {
        self.epics.get(id).map(String::as_str)
    }

    /// 챙겨 온 묶음과 그 칸 전부 — 읽은 칸으로 **그리는** 쪽이 받아 간다.
    pub fn columns(&self) -> impl Iterator<Item = (&str, &str)> {
        self.states.iter().map(|(id, col)| (id.as_str(), col.as_str()))
    }
}

/// 락 안에서 **낼 줄이 저장소 전체를 봐야 아는 값**을 챙겨 나온다 — 락을 놓은 뒤에 다시
/// 세면 그 사이에 남이 쓴 멤버가 섞인다. 둘 다 게을러서, 묶음이 없으면 칸을
/// (`group_states_of`), 소속을 id 에 진 줄이 없으면 에픽을(`groups_of`) 안 걷는다.
///
/// **소속은 `--json` 일 때만 걷는다**(`json`, 리뷰) — 그 지도를 읽는 자는 [`Row::from`]
/// 하나고 그것을 부르는 자리는 기계 출력뿐이다. 늘 걷으면 사람이 부르는 `moai mv` 도
/// 저장소 전체의 조상 오름을 락을 쥔 채 치르는데, 여기는 세션 예닐곱이 같은 `.moai` 를
/// 두고 줄 서는 저장소다 — 락 안에서 `ctx.lang()` 과 `model::actor` 를 뺀 것과 같은 까닭이다.
/// `ready`·`status` 의 한눈 보기도 같은 문을 쓴다.
pub fn read_of(issues: &[crate::model::Issue], cfg: &crate::config::Config, ids: &[&str], json: bool) -> Read {
    let owned = |m: BTreeMap<&str, &str>| m.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Read {
        states: owned(crate::report::group_states_of(issues, cfg, ids)),
        epics: match json {
            true => owned(crate::report::groups_of(issues, ids)),
            false => BTreeMap::new(),
        },
    }
}

/// `--from` 이 받는 칸 — **아는 칸이거나, 어느 줄이 실제로 서 있는 칸**(moai-hym7).
///
/// 오타는 그대로 거절한다. 거절이 노리는 것은 오타지 낡음이 아니다 — `config` 에서 칸
/// 이름을 하나 고치면 옛 이름에 선 줄이 남는데, 그 이름을 오타로 읽어 막으면 그 줄은
/// **영영** `--from` 으로 못 집는다. "남의 낡은 줄 하나가 모든 쓰기를 막으면 되돌릴
/// 방법이 도구 밖에만 남는다"(CLAUDE.md)와 같은 자리고, `store::with_write` 가 파일
/// 전체가 아니라 **바뀐 줄만** 검사하는 것과도 같은 자다.
///
/// 줄을 봐야 하므로 락 안에서 잰다 — 밖에서 재면 그 사이 마지막 줄이 그 칸을 떠난다.
///
/// **말이 아니라 자료를 낸다**(리뷰) — 글을 여기서 지으면 부르는 쪽이 `ctx.lang()` 을 인자로
/// 넘겨야 하고, 인자는 락 **안에서** 먼저 셈해진다. [`Ctx::lang`] 의 첫 부름은 사용자 설정을
/// 열어 파싱하므로, 오타가 없는 판까지 트래커 락을 쥔 채 남의 파일을 읽게 된다 — `model::actor`
/// 를 락 밖으로 뺀 것과 같은 까닭이다(`cmd/mv.rs`). 거절할 때만 펴면 그 일이 아예 안 난다.
pub fn check_from(
    from: Option<&str>,
    issues: &[crate::model::Issue],
    cfg: &crate::config::Config,
) -> Result<(), crate::config::NoSuchColumn> {
    // 아는가를 가르는 것은 `report` 다 — 읽는 쪽(`show -s`·탐색기 필터)과 **같은 술어**를
    // 써야 옮길 수는 있는데 못 찾는 줄이 안 생긴다.
    let Some(f) = from.filter(|f| !crate::report::knows_column(issues, cfg, f)) else { return Ok(()) };
    Err(unknown_column(f, cfg))
}

/// 줄까지 보고도 모르는 칸 — 쓰기도 읽기도 **같은 자료**를 낸다(moai-fdk7). 글은
/// [`crate::view::no_such_column`] 이 짓는다.
pub fn unknown_column(name: &str, cfg: &crate::config::Config) -> crate::config::NoSuchColumn {
    crate::config::NoSuchColumn { name: name.to_string(), nor_rows: true, known: cfg.statuses.clone() }
}

/// `--from` 이 견줄 칸의 지도 — [`standing_of`] 가 낸다.
///
/// **[`Read`] 와는 다른 자다.** 이쪽은 `--from` 이 견줄 칸 하나만 담고 아무것도 안 낸다.
/// 막는 것은 `Read` 가 구조체가 된 쪽이다 — 필드가 사유라 맨 지도는 [`Row::from`] 에
/// 못 닿는다. 이 이름은 별명이라 그 자체로는 아무것도 안 막고, 갈라 둔 까닭을 적어 둘
/// 뿐이다. 한 이름을 둘이 쓰던 때에는 소속을 안 챙긴 이 지도가 줄을 내는 자리에 흘러도
/// 컴파일이 지났다.
pub type Standing = BTreeMap<String, String>;

/// `--from` 이 견줄 **서 있는 칸** — 물은 줄마다 하나씩, 락 안에서 **한 번** 뜬다.
///
/// [`read_of`] 와 갈리는 곳이 둘이다.
///
/// - **묶음만이 아니라 물은 줄 전부**를 담는다. 묶음인지는 [`crate::report::column`] 이
///   가른다 — 지도를 id 로만 짚던 판은 그 갈림을 혼자 안 지켰다. 머지를 잘못 푼
///   파일에서 묶음과 id 가 같은 일 줄이 그 묶음의 칸을 입는 자리라, `report::column`
///   과 `Row::of` 가 같은 곳에서 같은 `if` 를 쓴다(report.rs 의 "묶음을 가르는 `if`
///   가 표면마다 있으면 하나는 반드시 빠진다").
/// - **한 번만 뜨는 것이 곧 뜻이다.** 돌면서 그때그때 `i.status` 를 보면 같은 id 를
///   두 번 적은 한 명령이 **제가 방금 쓴 값**과 겨룬다 — 옮겨 놓고도 "이미 …다" 로
///   지고, `moved` 와 `stale` 에 같은 줄이 함께 서며, 종료 코드가 0 이 아니다.
///   `--from` 이 재는 것은 *부르는 쪽이 본* 칸이지 이 명령이 만든 칸이 아니다.
pub fn standing_of(issues: &[crate::model::Issue], cfg: &crate::config::Config, ids: &[&str]) -> Standing {
    let states = crate::report::group_states_of(issues, cfg, ids);
    issues
        .iter()
        .filter(|i| ids.contains(&i.id.as_str()))
        // **쌍둥이가 있어도 이 답은 파일에 안 닿는다**(리뷰). `Row::from` 과 달리 여기는 `with_write`
        // 의 **닫힘 안**이라, 중복 id 를 물리는 `first_duplicate` 가 아직 안 돌았다 — 이 줄은 가려진
        // 줄을 볼 수 있다. 그래도 지도를 안 대는 까닭은 그 파일에 대고 쓴 것은 닫힘이 돌아간 뒤
        // `Trouble::DuplicateId` 로 통째로 물리기 때문이다: 여기서 센 `--from` 이 무엇이든 파일은
        // 안 바뀐다. 게다가 묶음에는 `--from` 을 못 쓰므로(`cmd::mv`·`cmd::defer` 가 앞에서 거른다)
        // 여기 남는 것은 일 줄뿐이고, 일 줄은 가려지든 아니든 읽은 칸을 안 입는다(`stands_on`).
        .map(|i| (i.id.clone(), crate::report::column(&crate::report::Kinds::no_twins(), i, &states).to_string()))
        .collect()
}

/// 객체 하나에 필드를 덧붙여 낸다. 선언 순서를 지키려면 직렬화된 뒤에
/// 붙이는 수밖에 없다 — 중간에 `Value` 를 쓰면 순서가 사라진다.
///
/// **덧붙인 키가 이긴다**(moai-kgu2, 사용자와 정함). 줄이 모르는 필드로 같은 이름을 들고 있으면
/// (`--json` 을 파일에 되써 넣은 줄) 한 객체에 같은 키가 둘 선다. 줄이면 그 이름은 [`Row::of`] 가
/// 이미 걷었다 — 이번에 안 실은 조건부 키까지(moai-2l8n). 필드가 선언된 것뿐인 객체
/// (`StatusReport`)는 걷을 것이 없다. 여기서는 걷지 않는다.
///
/// **`extra` 의 키는 모두 바탕의 [`Appendable::APPENDED`] 에 있어야 한다** — 줄이면 [`OURS`],
/// 선언된 객체면 그 명령이 덧붙이는 키. 시험 빌드에서 확인한다. 목록이 덧붙이는 자리와 떨어져
/// 있어, 새 키를 더하고 목록을 잊으면 그 키만 moai-2l8n 이 되살아나는데 그것을 잡을 시험이 따로
/// 없다. 그 키가 한 번이라도 실리는 시험이 여기서 붉어진다.
pub fn json_with<T: Appendable>(base: &T, extra: &[(&str, String)]) -> R<Vec<String>> {
    debug_assert!(
        extra.iter().all(|(k, _)| T::APPENDED.contains(k)),
        "덧붙인 키가 `APPENDED` 에 없다 — 되써 넣은 줄에서 그 키를 못 걷는다: {:?}",
        extra.iter().map(|(k, _)| *k).filter(|k| !T::APPENDED.contains(k)).collect::<Vec<_>>()
    );
    let mut s = serde_json::to_string(base).map_err(|e| Fail::new(e.to_string()))?;
    if !s.ends_with('}') {
        return Err(Fail::new("not an object"));
    }
    let empty = s == "{}";
    s.pop();
    for (i, (k, v)) in extra.iter().enumerate() {
        if !(empty && i == 0) {
            s.push(',');
        }
        s.push_str(&format!("\"{k}\":{v}"));
    }
    s.push('}');
    Ok(vec![s])
}

/// 옮기거나 도로 집었어도 **계획 밖인 줄**과, 그것을 실제로 뺀 줄.
///
/// `mv`·`defer` 의 기계 출력이 같은 모양으로 낸다. 사람 출력은 그 줄에 도로 집을
/// 말을 붙이는데 기계 출력에만 없으면, `--json` 을 읽는 에이전트는 방금 집은 일이
/// 왜 훅의 초점에서 빠졌는지 알 길이 없다.
#[derive(serde::Serialize)]
pub struct Shelved<'a> {
    pub id: &'a str,
    /// 가장 가까운 미룬 곳.
    pub root: &'a str,
    /// 도로 집어야 할 곳 **전부**, 가까운 것부터(moai-g2a1). 하나뿐이면 `root` 와 같아
    /// 안 낸다 — 흔한 경우의 출력을 바꾸지 않는다.
    #[serde(skip_serializing_if = "one_or_none")]
    pub roots: &'a [String],
}

fn one_or_none(roots: &&[String]) -> bool {
    roots.len() <= 1
}

/// `--from` 에 걸려 **손대지 않은 줄**과, 락 안에서 본 그 줄의 지금 칸.
///
/// [`Shelved`] 와 같은 까닭으로 여기 하나다 — `mv` 와 `defer` 의 기계 출력이 같은
/// 모양으로 낸다. 두 곳에 따로 두면 키를 하나 더할 때 한쪽만 늘어, 두 명령을 한
/// 파서로 읽는 쪽이 한쪽에서만 깨진다.
#[derive(serde::Serialize)]
pub struct Stale<'a> {
    pub id: &'a str,
    /// 락 안에서 본 **서 있는 칸** — `--from` 이 견준 그 값이다. 함께 주지 않으면 진
    /// 쪽이 한 번 더 물어야 한다.
    ///
    /// **[`Row`] 의 `status` 와 뜻이 다르다.** 저쪽은 파일에 적힌 값이고 이쪽은 묶음이면
    /// 멤버에서 읽은 칸이다(`standing_of`) — 같은 이름이라 되쓰는 쪽이 파생값을 적힌
    /// 값으로 믿을 수 있다. 여기 이름을 `status` 로 둔 것은 이 값이 그대로 다음 `--from`
    /// 의 인자이기 때문이고, 묶음의 두 칸이 갈리는 곳은 `reference` 가 적어 둔다.
    pub status: &'a str,
}

/// (줄, 락 안에서 본 지금 칸).
pub fn stale(rows: &[(String, String)]) -> Vec<Stale<'_>> {
    rows.iter().map(|(id, status)| Stale { id, status }).collect()
}

/// (줄, 풀어야 할 미룸 전부 — 가까운 것부터).
pub fn shelved(pairs: &[(String, Vec<String>)]) -> Vec<Shelved<'_>> {
    pairs.iter().map(|(id, roots)| Shelved { id, root: roots.first().map_or("", String::as_str), roots }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Issue, Kind, Status};

    /// **실행 비트가 답을 가른다.** `is_file` 만 보던 판은 권한이 빠진 파일을 "돈다" 고 했고,
    /// 훅은 그때 권한 오류를 `|| exit 0` 으로 삼켜 아무 말 없이 아무것도 안 했다.
    ///
    /// 재는 자가 한 자리에 선다 — `skill` 과 `tui` 가 저마다 적던 것을 여기로 모았다(moai-p3kb).
    ///
    /// **`noexec` 로 얹힌 자리에서는 안 잰다**(moai-dhx9). `access(X_OK)` 는 마운트 플래그까지
    /// 보므로(Linux 의 `do_faccessat` 이 `path_noexec` 을 본다 — 이 기계에서 `noexec` 마운트의
    /// `0o755` 파일에 `test -x` 가 아니라고 답하는 것으로 쟀다), `TMPDIR` 이 그런 기계에서는
    /// 실행 비트를 세워도 여기가 아니라고 한다. `mode & 0o111` 을 보던 때는 마운트와 무관했으니
    /// 이 문은 이 바뀜이 새로 만든 자리다. **재지 못하는 것을 실패로 세지 않는다** — 아래 시험의
    /// root 문지기와 같은 자다.
    #[cfg(unix)]
    #[test]
    fn only_a_file_with_the_execute_bit_runs() {
        use std::os::unix::fs::PermissionsExt as _;
        let s = crate::scratch::Scratch::new("cmd-runnable");
        let probe = s.join("probe");
        std::fs::write(&probe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !runnable(&probe) {
            return; // `noexec` 로 얹힌 자리 — 여기서는 실행을 못 잰다.
        }
        let exe = s.join("exe");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        assert!(!runnable(&exe), "실행 비트가 없는데 돈다고 한다");
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(runnable(&exe), "실행 비트가 섰는데 안 돈다고 한다");
        // 디렉터리는 실행 비트가 서 있어도 돌릴 것이 아니고, 없는 자리도 아니다.
        assert!(!runnable(s.path()), "디렉터리를 돌린다고 한다");
        assert!(!runnable(&s.join("없다")), "없는 자리를 돈다고 한다");
    }

    /// **"아무나 돌릴 수 있는가" 와 "내가 돌릴 수 있는가" 는 다르다**(moai-dhx9). 임자만 실행 비트가
    /// 빠진 파일(`0o011`)은 `mode & 0o111` 로는 돈다고 읽히는데, 임자가 부르면 껍데기가 126 을 낸다 —
    /// 남의 소유 `0o700`·`noexec` 마운트와 같은 갈래고, 사람 하나로 지을 수 있는 꼴이 이것이다.
    ///
    /// **root 로 돌 때는 안 잰다** — `access(X_OK)` 는 root 에게 실행 비트가 하나라도 서 있으면 0 을
    /// 내므로 그 자리에는 이 가름이 아예 없다. 재지 못하는 것을 실패로 세지 않는다.
    ///
    /// **문지기는 `getuid` 다, `geteuid` 가 아니다** — `access` 가 보는 것이 실제 uid 라고 위
    /// [`runnable`] 의 글이 적어 두었는데, 문지기만 유효 uid 를 보던 판은 둘이 갈리는 자리에서
    /// 거꾸로 답했다. 실제 uid 가 0 이고 유효 uid 가 아닌 판에서는 안 건너뛰면서 `access` 는 root
    /// 로 재어 이 줄이 까닭 없이 붉어지고, 그 반대 판에서는 잴 수 있는 것을 건너뛴다.
    #[cfg(unix)]
    #[test]
    fn a_file_others_may_run_but_i_may_not_does_not_run() {
        use std::os::unix::fs::PermissionsExt as _;
        // SAFETY: `getuid` 는 인자가 없고 아무것도 안 바꾼다.
        if unsafe { libc::getuid() } == 0 {
            return;
        }
        let s = crate::scratch::Scratch::new("cmd-runnable-mine");
        let exe = s.join("exe");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o011)).unwrap();
        assert!(!runnable(&exe), "임자가 못 돌리는 파일을 돈다고 한다");
    }

    fn row_with(rest: &[(&str, &str)]) -> Issue {
        let mut i =
            Issue::new("argos-0001".into(), "제목".into(), Kind::Epic, Status::new("todo"), "2026-09-11T04:12:03Z");
        for (k, v) in rest {
            i.rest.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        i
    }

    /// 종류도 우선순위도 **기본값이라 줄에 안 적히는** 줄. 파일에서 가장 흔한 꼴이다.
    fn plain() -> Issue {
        Issue::new("argos-0002".into(), "제목".into(), Kind::Issue, Status::new("todo"), "2026-09-11T04:12:03Z")
    }

    /// **기본값이라 줄에서 빠진 키도 `--json` 에는 선다**(moai-51it·moai-a4u9). 파일에 안 적는 것과
    /// `--json` 이 안 내는 것은 다른 일이다 — 읽는 쪽에는 그 침묵의 뜻이 안 보여 `jq -r .priority` 가
    /// 기본값인 줄과 모르는 값을 한 `null` 로 받는다.
    #[test]
    fn a_row_speaks_the_default_kind_and_priority() {
        let i = plain();
        assert!(i.kind.is_default() && i.priority.is_none(), "기본값인 줄이 아니다");
        let out = json_line(&Row::of(&i, None, None, &crate::report::Kinds::no_twins())).unwrap().join("");
        assert!(out.contains(r#""kind":"issue""#), "종류가 빠졌다\n{out}");
        assert!(out.contains(&format!(r#""priority":{}"#, crate::model::DEFAULT_PRIORITY)), "우선순위가 빠졌다\n{out}");
    }

    /// **줄이 든 값은 그대로 한 번만 선다.** 세우는 자리가 줄과 `Row` 둘이라, 줄이 들고 있을 때도
    /// 세우면 한 객체에 같은 키가 둘 서고 깐깐한 파서가 거절한다.
    #[test]
    fn a_row_that_carries_them_is_untouched() {
        let mut i = row_with(&[]);
        i.priority = Some(1);
        let out = json_line(&Row::of(&i, None, None, &crate::report::Kinds::no_twins())).unwrap().join("");
        assert_eq!(out.matches(r#""kind":"#).count(), 1, "종류가 둘 섰다\n{out}");
        assert_eq!(out.matches(r#""priority":"#).count(), 1, "우선순위가 둘 섰다\n{out}");
        assert!(out.contains(r#""kind":"epic""#) && out.contains(r#""priority":1"#), "{out}");
    }

    /// **덧붙인 키가 이긴다**(moai-kgu2) — `show` 가 덧붙이는 키 전부에 같은 자로 선다. 겹치지
    /// 않는 모르는 필드와 곁들인 `derived_status` 는 그대로 남는다.
    #[test]
    fn every_appended_key_wins_over_an_unknown_field_and_nothing_else_is_lost() {
        let i = row_with(&[
            ("members", "가짜"),
            ("shelved_by", "가짜"),
            ("duplicate_lines", "가짜"),
            ("due", "2026-10-01"),
        ]);
        let row = Row::of(&i, Some("in_progress"), None, &crate::report::Kinds::no_twins());
        let extra = [
            ("members", "[]".to_string()),
            ("shelved_by", "\"argos-0002\"".to_string()),
            ("duplicate_lines", "2".to_string()),
        ];
        let out = json_with(&row, &extra).unwrap().join("");
        for (k, v) in &extra {
            assert_eq!(out.matches(&format!("\"{k}\":")).count(), 1, "{k} 가 둘 섰다\n{out}");
            assert!(out.contains(&format!("\"{k}\":{v}")), "{k} 에 우리 값이 안 섰다\n{out}");
        }
        assert!(!out.contains("가짜"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\""), "겹치지 않는 모르는 필드까지 걷었다\n{out}");
        assert!(out.contains("\"derived_status\":\"in_progress\""), "{out}");
    }

    /// **이번에 안 실은 조건부 키도 걷는다**(moai-2l8n) — `OURS` 에 든 이름이면 `extra` 에 없어도
    /// 되써 넣은 옛 값이 안 나간다. `OURS` 에 없는 모르는 필드는 그대로다.
    #[test]
    fn a_conditional_key_left_out_this_time_is_stripped_too() {
        let i = row_with(&[("commits_error", "가짜"), ("due", "2026-10-01")]);
        let row = Row::of(&i, None, None, &crate::report::Kinds::no_twins());
        let out = json_with(&row, &[("commits", "[]".to_string())]).unwrap().join("");
        assert!(!out.contains("commits_error"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\"") && out.contains("\"commits\":[]"), "{out}");
    }

    /// **덧붙이지 않는 명령도 같은 목록으로 걷는다**(moai-qn5d) — 걷는 자리가 `Row::of` 라
    /// `ready`·`add`·`mv` 처럼 `json_line` 으로 줄을 펴는 명령에도 옛 값이 안 나간다.
    #[test]
    fn a_plain_row_drops_every_key_moai_appends() {
        let mut rest: Vec<(&str, &str)> = OURS.iter().map(|k| (*k, "가짜")).collect();
        rest.push(("due", "2026-10-01"));
        let i = row_with(&rest);
        let out = json_line(&Row::of(&i, None, None, &crate::report::Kinds::no_twins())).unwrap().join("");
        assert!(!out.contains("가짜"), "{out}");
        assert!(out.contains("\"due\":\"2026-10-01\""), "{out}");
    }

    /// **`Row` 가 제 필드로 곁들이는 키도 `OURS` 에 있다**(moai-qn5d) — 없으면 되쓴 줄의 같은 이름이
    /// 안 걷혀 한 객체에 둘 선다. `Row` 에 필드를 더하면 여기서 붉어진다.
    #[test]
    fn every_key_a_row_adds_is_in_ours() {
        // **기본값인 줄로 잰다** — 종류와 우선순위는 줄이 안 적었을 때만 `Row` 가 세운다. 적힌
        // 줄로 재면 그 둘이 곁들인 키로 안 잡혀, 목록에서 빠져도 여기가 안 붉어진다.
        let i = plain();
        let row = Row {
            issue: std::borrow::Cow::Borrowed(&i),
            kind: Some(Kind::Issue.as_str()),
            priority: Some(crate::model::DEFAULT_PRIORITY),
            derived_status: Some("todo"),
            derived_epic: Some("moai-0001"),
            branch: Some("feat/x"),
        };
        let added = keys_beyond(&i, &row);
        assert!(!added.is_empty(), "곁들인 키를 못 셌다");
        for k in &added {
            assert!(OURS.contains(&k.as_str()), "`Row` 가 곁들이는 {k} 가 `OURS` 에 없다");
        }
    }

    /// **`APPENDED` 에 안 적은 키를 덧붙이면 시험 빌드가 멈춘다** — 목록이 덧붙이는 자리와 떨어져
    /// 있어 새 키를 더하고 목록을 잊는 것을 잡을 곳이 여기뿐이다(moai-2l8n).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "APPENDED")]
    fn an_appended_key_missing_from_the_list_is_caught() {
        let i = row_with(&[]);
        let _ = json_with(&Row::of(&i, None, None, &crate::report::Kinds::no_twins()), &[("새_키", "[]".to_string())]);
    }

    /// 겹치는 것이 없으면 걷은 모습을 짓지 않는다 — 흔한 길에서 줄을 복제하지 않는다.
    #[test]
    fn nothing_to_strip_means_no_copy() {
        let i = row_with(&[("due", "2026-10-01")]);
        assert!(matches!(
            Row::of(&i, None, None, &crate::report::Kinds::no_twins()).issue,
            std::borrow::Cow::Borrowed(_)
        ));
    }

    fn unread(root: &str, at: &str) -> crate::store::Unread {
        crate::store::Unread {
            root: std::path::PathBuf::from(root),
            at: std::path::PathBuf::from(at),
            kind: "permission",
            said: "Permission denied".to_string(),
        }
    }

    /// **줄 곁에 다는 것은 그 줄의 뿌리의 실패뿐이다**(moai-f2lc). `show --worktree` 는 겹쳐 온
    /// 줄의 이력을 그 워크트리의 저널에서 읽으므로 한 판에 여러 체크아웃의 자리가 섞인다 —
    /// 안 가르면 제 파일은 멀쩡한 줄이 남의 0600 파일 하나로 "이력이 덜 왔다" 를 달고 선다.
    /// `main` 이 종료 코드를 제 뿌리로만 가르는 것(`any_mine`)과 같은 금이다.
    #[test]
    fn a_rows_journal_error_names_only_its_own_root() {
        let all =
            [unread("/w/mine", "/w/mine/.moai/journal/a.jsonl"), unread("/w/side", "/w/side/.moai/journal/b.jsonl")];
        let lang = crate::i18n::Lang::En;

        let mine = journal_errors(lang, &all, Some(std::path::Path::new("/w/mine")));
        assert_eq!(mine.len(), 1, "{mine:?}");
        assert!(mine[0].said.contains("/w/mine/.moai/journal/a.jsonl"), "{}", mine[0].said);
        assert!(!mine[0].said.contains("/w/side"), "옆 체크아웃의 자리를 이 줄에 달았다 — {}", mine[0].said);

        // 뿌리를 안 주면 다 든다 — `main` 의 stderr 는 옆의 것도 말한다.
        assert_eq!(journal_errors(lang, &all, None).len(), 2);
        // 못 읽은 것이 하나도 없으면 한 줄도 없다 — 그 빔이 곧 "이력이 다 왔다" 이다.
        assert!(journal_errors(lang, &[], None).is_empty());
    }

    /// **가르는 자는 `kind` 고 `said` 는 사람의 것이다**(moai-f2lc) — `commits_error` 와 같은
    /// 약속이다. `said` 는 말묶음에서 오므로 말이 갈리면 글자도 갈리는데, `kind` 는 그대로다:
    /// 받는 쪽이 `said` 로 갈라 읽으면 그 고리는 화면 말 설정 하나로 조용히 깨진다.
    #[test]
    fn the_kind_is_the_machines_and_the_said_is_the_persons() {
        let one = [unread("/w/mine", "/w/mine/.moai/journal/a.jsonl")];
        let en = journal_errors(crate::i18n::Lang::En, &one, None);
        let ko = journal_errors(crate::i18n::Lang::Ko, &one, None);
        assert_eq!(en[0].kind, ko[0].kind, "말이 갈렸다고 `kind` 까지 갈렸다");
        assert_ne!(en[0].said, ko[0].said, "말이 안 갈린다 — 말묶음을 안 지났다");
        for e in [&en[0], &ko[0]] {
            assert!(e.said.contains("/w/mine/.moai/journal/a.jsonl"), "{}", e.said);
            assert!(e.said.contains("Permission denied"), "{}", e.said);
        }
    }
}
