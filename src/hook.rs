//! 훅이 무엇을 낼지 정한다. **순수 함수다** — 이벤트와 `&[Issue]` 만 보고
//! 아무것도 찍지 않고 저장소를 읽지 않는다.
//!
//! **예외는 한 자리, [`settled`] 뿐이다**(2026-09-20 사용자 결정, 리뷰 moai-1upp.nwq).
//! 규칙 2 는 "이 경로가 저장소 안인가" 를 묻는데, 같은 자리를 가리키는 두 철자를 한
//! 자리로 보려면 링크를 풀어야 하고 그것은 파일 시스템에만 있다. 판정을 `cmd/` 로
//! 올리면 `guard_shell_in` 이 셸 토막에서 캐낸 경로마다 푸는 손잡이를 인자로 더 받아야
//! 하는데, 그 함수는 이미 인자 여덟이다. 읽는 것은 **경로 한 줄**이지 저장소가 아니다 —
//! 이슈도 설정도 여전히 `cmd/hook.rs` 가 읽어 넘긴다.
//!
//! **여기가 에이전트와의 계약이다.** `cmd/hook.rs` 는 stdin 을 풀고, 저장소를
//! 읽고, 여기가 낸 답을 계약 JSON 으로 옮기기만 한다. 판단을 `cmd/` 에 두면
//! 시험이 프로세스를 띄워야 하고, 다음 표면이 같은 판단을 다시 짠다.
//!
//! ## 왜 훅인가
//!
//! 규칙을 산문으로만 적어 두면 읽히지 않는다. 훅은 읽히는 자리에 규칙을
//! 놓는다 — 다만 **사람을 부르지 않는다.** 막는 것은 막힌 쪽이 제자리에서
//! 스스로 고칠 수 있는 것뿐이고, 고칠 명령을 함께 낸다. 옛 moai 의 게이트는
//! 사람이 답할 때까지 일이 멈췄고, 이것은 멈추지 않는다. 그리고 훅은
//! **에이전트의 도구 호출에만** 걸린다 — 터미널에서 사람이 친 `moai` 는 이
//! 길을 지나지 않고, `moai status` 는 여전히 아무것도 막지 않는다.

use crate::config::Config;
use crate::guide::REVIEW_STEPS;
use crate::model::Issue;
use crate::report;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 훅이 걸리는 자리.
///
/// **`SessionStart` 는 아무것도 싣지 않는다.** 재개(`source=resume`)에서 그
/// 출력이 이어지는 대화에 안 붙는 것을 네 번 재개하며 확인했다 (평문이든
/// 계약 JSON 이든 같았고, 같은 세션의 다른 플러그인 훅도 함께 안 붙었다).
/// 안 붙는 자리에 대고 실으면 훅이 사는지 죽었는지 모른 채 규칙만 남는다.
/// 그래서 보드는 `UserPromptSubmit` 이 세션당 한 번 싣고, `SessionStart` 는
/// `Stop` 이 견줄 기준선만 적는다.
///
/// **단 접힌 뒤(`source=compact`)는 싣는다.** 그 자리의 출력은 붙는다 —
/// `/compact` 뒤에 같은 세션의 다른 플러그인이 `SessionStart:compact` 로 실은
/// 글이 대화에 그대로 있었다 (moai-91jk). 집고 있던 것은 거기서 싣는다.
///
/// **`PreCompact` 는 걸지 않는다.** 한때 거기서 집은 것을 실었는데, `claude` 가
/// 그 이벤트의 `hookSpecificOutput` 을 검증에서 거절했다 — 받는 이벤트 목록에
/// `PreCompact` 가 없다. 훅은 돌았고 id 도 옳게 골랐지만 한 줄도 안 붙었고,
/// stdout 만 보던 시험은 초록이었다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Event {
    // **`hook --help` 의 이벤트 목록이 이 글을 옮겨 적는다**(moai-h0r2) — clap 이 붙이는 값 목록은
    // `-h` 에서 80칸을 넘어 숨겼다. `the_hook_help_lists_every_event` 가 둘을 견준다.
    /// 기준선을 적는다. 접힌 뒤면 집은 것을 싣는다
    SessionStart,
    /// 사람이 시켰다. 보드를 세션당 한 번 싣는다
    UserPromptSubmit,
    /// 도구를 부르기 직전. 규칙이 여기서 선다
    PreToolUse,
    /// 턴이 끝난다. 상태가 실제와 맞는지 본다
    Stop,
}

impl Event {
    /// 계약이 쓰는 이름. clap 의 kebab-case 와 모양이 달라 손으로 적는다.
    pub fn wire(self) -> &'static str {
        match self {
            Event::SessionStart => "SessionStart",
            Event::UserPromptSubmit => "UserPromptSubmit",
            Event::PreToolUse => "PreToolUse",
            Event::Stop => "Stop",
        }
    }
}

/// 훅이 stdin 으로 받는 것.
///
/// **모르는 필드는 버린다.** 계약이 자라도 우리가 안 깨져야 한다 — 실제로
/// 재개에서 오는 이벤트에는 `context_tokens`·`transcript_path` 같은 것이
/// 열 개 넘게 딸려 온다.
///
/// **`cwd` 와 `session_id` 는 여기 있는 것이 진짜다.** 환경변수에는 없다.
/// `CLAUDE_SESSION_ID` 를 찾다가 빈손으로 떨어진 기준선이 다음 세션 것과
/// 섞이는 것을 시험판에서 겪었다.
#[derive(Debug, Default, Deserialize)]
pub struct Input {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// `SessionStart` 가 왜 열렸나 — `startup`·`resume`·`clear`·`compact`.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: serde_json::Value,
    /// 이미 한 번 붙들었다는 표. **이것을 안 보면 무한히 돈다.**
    #[serde(default)]
    pub stop_hook_active: bool,
}

/// 훅이 내는 답.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// 아무 말도 하지 않는다. **훅의 보통 상태다.**
    Pass,
    /// 이만큼을 컨텍스트에 싣는다.
    Context(String),
    /// 이 도구 호출을 막는다. **까닭에는 고칠 명령이 함께 든다** — 사람을
    /// 부르지 않고 막힌 쪽이 제자리에서 풀 수 있어야 게이트가 아니다.
    Deny(String),
    /// 턴을 끝내지 않고 이것부터 보게 한다.
    Block(String),
}

impl Decision {
    /// 이 답이 **막는가** — 도구 호출을 막거나(`Deny`) 턴을 붙든다(`Block`). 비추는 줄(`Context`)은
    /// 안 막는다.
    ///
    /// **"안 막는다" 를 `== Pass` 로 읽지 않는다.** `Context` 가 생기기 전에 선 자리가 그렇게 읽어,
    /// 옆 워크트리와 겹쳐 본 판정이 비추기만 하는데도 막힌 것으로 쳐서 낡은 스냅샷의 거절을 도로
    /// 냈다(moai-dw63.e31) — `idea add` 하나를 곁들인 명령줄이 이미 집은 일을 집으라고 막혔다.
    pub fn blocks(&self) -> bool {
        matches!(self, Decision::Deny(_) | Decision::Block(_))
    }

    /// 판정을 잇는다 — **막는 답이 이긴다.** 앞이 막으면 뒤는 묻지 않고, 뒤가 막으면 앞의 비추는
    /// 줄을 버린다. 둘 다 안 막으면 비추는 줄을 모은다 — 트래커 둘이 저마다 비춘 줄을 하나만
    /// 남기면 뒤의 물음이 말없이 빠진다.
    ///
    /// **판정의 차례는 여기 하나다.** 규칙마다(`guard_shell_in`), 트래커마다(`cmd/hook.rs`) 손으로
    /// 적던 차례는 이미 서로 다른 답을 내고 있었다.
    pub fn then(self, next: impl FnOnce() -> Decision) -> Decision {
        if self.blocks() {
            return self;
        }
        match (self, next()) {
            (_, later) if later.blocks() => later,
            (Decision::Context(a), Decision::Context(b)) => Decision::Context(format!("{a}\n\n{b}")),
            (Decision::Pass, later) => later,
            (earlier, _) => earlier,
        }
    }
}

/// 세션에 싣는 머리말. 보드만 실으면 그것이 무엇을 하라는 뜻인지가 안 붙는다.
const LEAD: &str = "이 저장소의 할 일은 moai 에 있다. TodoWrite 나 마크다운 TODO 목록을 쓰지 않는다.";

/// 보드를 실을 글로 만든다.
///
/// 보드 줄은 `view::status` 가 이미 만든 것을 그대로 받는다 — 훅이 제 손으로
/// 다시 그리면 화면과 훅이 같은 저장소를 두 모양으로 말한다.
pub fn board(lines: &[String]) -> Decision {
    // 싣기 전에 색을 걷는다. 까닭은 `style::plain` 에 있다.
    let body = crate::style::plain(&lines.join("\n"));
    if body.trim().is_empty() {
        return Decision::Pass;
    }
    Decision::Context(format!("{LEAD}\n\n{body}"))
}

/// 접힌 뒤에도 잃으면 안 되는 것 — 지금 집고 있는 일.
///
/// 집은 것이 없으면 아무 말도 하지 않는다. **빈 목록을 싣지 않는다** — 막
/// 접어 비운 자리에 "없다" 를 적는 것은 그 값을 치를 일이 아니다.
///
/// **`closing` 과 같은 자로 잰다**([`holding`]) — 겹친 줄(`latest`)에서 더는 안 집힌 줄은 안 싣는다.
/// 따로 재던 판은 main 이 이미 닫거나 놓은 줄을 딸린 워크트리의 낡은 스냅샷대로 "압축 전부터 집고
/// 있다" 로 실었고, 같은 세션의 `Stop` 은 그 줄을 안 붙들어 두 말이 갈렸다(리뷰 moai-3k2d.1df).
pub fn carried(issues: &[Issue], latest: &[Issue], cfg: &Config, away: &Away) -> Decision {
    let wip = holding(issues, latest, cfg, away);
    if wip.is_empty() {
        return Decision::Pass;
    }
    let mut out = String::from("압축 전부터 이것을 집고 있다:");
    for i in wip {
        out.push_str(&format!("\n  {}  {}", i.id, i.title));
    }
    Decision::Context(out)
}

/// 도구가 하려는 일 중 **규칙이 뜻을 두는 것**만 추린 모양.
///
/// 계약의 `tool_name`·`tool_input` 을 여기까지 접어 두면, 판정 함수들이 JSON
/// 모양에 묶이지 않고 시험이 값 하나만 만들면 된다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Call<'a> {
    /// 껍데기에 친 명령.
    Shell(&'a str),
    /// 이 파일을 고치려 한다.
    Edits(&'a str),
    /// 리뷰를 부르려 한다.
    Review,
    /// 규칙이 뜻을 두지 않는 것.
    Other,
}

impl<'a> Call<'a> {
    /// 계약이 준 것을 규칙이 아는 모양으로 접는다.
    ///
    /// **도구 이름으로 가른다. 입력에 든 글자로 가르지 않는다.** 모든 입력에서
    /// 낱말을 찾던 판은 리뷰를 설명하는 문서를 쓰는 것만으로 리뷰 규칙에
    /// 걸렸다 — 규칙이 제 이야기를 적는 것까지 막으면 그 규칙은 못 쓴다.
    ///
    /// **껍데기는 언제나 `Shell` 이다.** 리뷰를 부르는지는 `guard_shell` 이
    /// 토막마다 본다. 명령 전체를 리뷰로 접던 판은 `moai mv <리뷰> done &&
    /// /code-review high` 한 줄로 닫기 규칙과 규칙 1 을 통째로 넘겼다.
    pub fn read(tool: Option<&str>, input: &'a serde_json::Value) -> Call<'a> {
        let text = |k: &str| input.get(k).and_then(|v| v.as_str());
        match tool {
            Some("Bash") => Call::Shell(text("command").unwrap_or_default()),
            Some("Edit" | "Write" | "NotebookEdit") => {
                Call::Edits(text("file_path").or_else(|| text("notebook_path")).unwrap_or_default())
            }
            Some("Skill") => {
                let named = format!(
                    "{} {}",
                    text("skill").unwrap_or_default(),
                    text("args").unwrap_or_default()
                );
                if named.contains(REVIEW_CMD) { Call::Review } else { Call::Other }
            }
            _ => Call::Other,
        }
    }
}

/// 리뷰를 부르는 명령의 이름.
const REVIEW_CMD: &str = "code-review";
/// 리뷰 이슈에 붙는 태그. 글은 `guide` 가 쓰므로 거기 둔다.
pub use crate::guide::REVIEW_TAG;

/// 껍데기에 친 명령이 **리뷰를 부르는가.**
///
/// 명령 자리만 본다. 명령줄 어디서든 낱말을 찾으면 리뷰 결과를 이슈에 적는
/// `moai note` 가 리뷰 규칙에 걸린다 — 시험판에서 실제로 걸렸고, 그때 이
/// 세션은 제 리뷰 결과를 적지 못했다.
fn calls_review(cmd: &str) -> bool {
    segments(cmd).iter().any(|seg| match command_of(seg).split_first() {
        Some((first, rest)) => {
            let head = first.trim_start_matches('/');
            head == REVIEW_CMD
                || (head.ends_with("claude")
                    && rest.first().is_some_and(|w| w.trim_start_matches('/') == REVIEW_CMD))
        }
        None => false,
    })
}

/// 명령줄을 **토막마다** 토큰으로 가른다. 따옴표 안은 한 토큰이다.
///
/// 네 가지를 같이 해야 한다. 넷 다 실제로 틀려 본 자리다.
///
/// - **제목이 동사로 오해받지 않아야 한다.** `moai add "idea 정리"` 의 `idea`
///   는 제목이지 하위 명령이 아니다.
/// - **이어 붙인 명령을 버리지 않아야 한다.** `;`·`&&`·`|` 에서 잘라 버리던
///   판은 `cd /repo && moai add "딴 일"` 하나로 규칙을 통째로 지나갔다.
/// - **줄바꿈도 토막을 가른다.** 앞서 `'\n'` 을 갈래에 적어 두고도 그 앞의
///   `is_whitespace()` 가 먼저 잡아, 여러 줄 명령이 한 토막으로 뭉쳤다.
///   `moai show\nmoai add "딴 일"` 이 그대로 지나갔다 — 시험도 있었지만
///   하필 첫 줄이 `cd` 라 우연히 통과해 거짓 안심만 줬다.
/// - **heredoc 의 속은 명령이 아니다.** `python3 - <<'PY' … PY` 의 본문에
///   `moai add "제목" -e <에픽>` 이라는 **글자**가 있다고 그것을 생성으로 읽으면,
///   리뷰 글을 이슈에 적는 일이 막힌다. 실제로 막혔다.
fn segments(cmd: &str) -> Vec<Vec<String>> {
    parse(cmd).into_iter().map(|s| s.words).filter(|w| !w.is_empty()).collect()
}

/// 글 안의 **명령 치환들**(`$( … )`·`` ` … ` ``) — 따옴표 없는 heredoc 본문에서 셸이 돌릴 것을
/// 고른다(moai-t863). 여는 글자 뒤부터 짝이 맞는 닫는 글자까지를 낸다.
///
/// **작은따옴표 안은 안 본다** — 셸도 heredoc 본문에서는 따옴표를 안 보지만, 치환 **안**의 따옴표는
/// 본다([`closes`]). 겹친 치환은 바깥 것 하나로 낸다 — 다시 읽는 렉서가 그 안을 또 가른다.
///
/// **본문 전체를 한 번에 받는다** — 줄마다 부르던 판은 줄을 넘는 `$( … )` 를 줄 끝에서 잘라,
/// 이어지는 줄의 `moai add` 를 아무 규칙에도 안 보였다(리뷰 moai-p836.rv).
fn substitutions(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            // 여기는 heredoc 본문이라 따옴표가 글자다 — `\` 만 다음 한 글자를 감싼다.
            b'\\' => i += 2,
            b'$' if b.get(i + 1) == Some(&b'(') => {
                // **안 닫힌 것은 명령이 아니다** — 셸이 그 줄을 아예 못 읽는다.
                let Some(end) = closes(text, i + 2, Some(b'('), b')') else { break };
                // `$((…))` 는 셈이지 명령이 아니다 — 여는 자리에서 가른다.
                let body = &text[i + 2..end];
                if !body.starts_with('(') {
                    out.push(body.to_string());
                }
                i = end + 1;
            }
            b'`' => {
                let Some(end) = closes(text, i + 1, None, b'`') else { break };
                out.push(text[i + 1..end].to_string());
                i = end + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// 여는 글자 **뒤**(`at`)부터 짝이 맞는 닫는 글자의 **바이트 자리** — **못 찾으면 `None`** 이다.
/// 안 닫힌 것은 셸도 못 읽어 그 줄이 아예 안 돈다. 글 끝을 닫는 자리로 치던 판은 markdown 산문의
/// 짝 없는 백틱 하나로 그 뒤 본문을 통째로 명령으로 읽어, `moai note <id> -b - <<EOF` 로 리뷰 글을
/// 붙이는 길을 막았다(리뷰 moai-p836.rv).
///
/// **따옴표 안의 괄호는 안 센다.** 셸이 그렇게 읽는다 — 괄호만 세던 판은
/// `$(echo ")" > src/x.rs)` 를 `echo "` 에서 끊어, 그 안의 쓰기를 규칙 2 에 안 보였다.
/// `open` 이 `None` 이면 겹치지 않는다(`` ` … ` ``).
///
/// 돌려주는 자리는 언제나 ASCII 글자의 자리라, 글자 경계에서 자른다.
fn closes(text: &str, at: usize, open: Option<u8>, shut: u8) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 1usize;
    let mut i = at;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'\'' if open.is_some() => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
            }
            b'"' if open.is_some() => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += usize::from(b[i] == b'\\') + 1;
                }
            }
            c if Some(c) == open => depth += 1,
            c if c == shut => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 이 토막이 **셸에 넘기는 글** — `bash -c '…'`·`sh -c`·`eval …` 이다(moai-455j). 그 글은 낱말이
/// 아니라 명령줄이라, 렉서가 다시 읽어야 규칙이 본다([`Lexer::relex`]).
///
/// **옵션이 끝난 자리의 첫 낱말이 글이다** — `-c` 를 봤으면 그 낱말이 명령 글이고, 안 봤으면
/// 스크립트 이름이다(그 글은 여기서 안 돈다). `eval` 은 인자를 공백으로 이어 붙인 것이 글이다 —
/// 셸이 그렇게 한다. 곁에 **그 글이 새 셸에서 도는가**(`eval` 은 지금 셸이다)와 **띄울 때 이미
/// errexit 가 켜졌는가**를 함께 낸다.
///
/// **옵션이 아닌 낱말에서 멈춘다** — 셸도 그렇다. 끝까지 훑던 판은 `bash 스크립트.sh -c '…'` 의
/// `-c` 를 명령 글로 읽어, 셸이 스크립트의 인자로만 넘기는 글을 규칙에 비췄다(잘못 막음,
/// 리뷰 moai-p836.rv).
///
/// **`-c` 에서 곧바로 뒤 낱말을 집지 않는다**(리뷰 moai-k8j1.034) — 셸은 `-c` 를 보고도 옵션을 계속
/// 읽고, 옵션이 다 끝난 자리의 첫 낱말을 글로 돌린다. 곧바로 집던 판은 `bash -co errexit '집기; 쓰기'`
/// 의 글로 `errexit` 를 집고, `bash -oe pipefail -c '쓰기'` 는 `pipefail` 에서 멈춰 아예 None 을 냈다 —
/// 둘 다 그 안의 쓰기가 규칙에 안 보여 빈손의 쓰기가 샜다. 셋 다 실제로 도는 줄이다(`bash -co errexit
/// 'echo hi'` 가 `hi` 를 찍는다).
fn shell_text(words: &[String]) -> Option<Handed> {
    let cmd = command_of(words);
    let (head, rest) = cmd.split_first()?;
    // **셸을 여는 스위치가 넘긴 글도 여기서 읽는다**(moai-drli) — `env -S` 와 `sudo -s`.
    // `command_of` 는 그 앞에서 멈추고([`Wrapped::Hands`]) 그 뒤는 명령이 아니라 글이다. 멈추기만
    // 하고 아무도 안 읽던 판은 `env -S "moai add x"` 와 `sudo -s moai add x` 가 규칙을 통째로 지나갔다.
    if let Some(Wrapped::Hands { at, glued, words: split }) = wrapped(basename(head), rest) {
        let tail = &rest[at..];
        // **이미 갈린 낱말은 도로 감싸서 잇는다** — `sudo -s <명령…>` 은 argv 를 통째로 escape 해
        // 셸에 `-c` 로 넘긴다(sudo 의 `parse_args.c`). 맨 빈칸으로만 잇던 판은 바깥 껍데기가 이미
        // 벗긴 따옴표를 못 되살려 두 쪽으로 틀렸다 — `sudo -s git commit -m 'a > b'` 의 `>` 가
        // 리다이렉션으로 읽혀 없는 파일 `b` 로 규칙 2 가 **잘못 막았고**, `sudo -s bash -c '쓰기'` 는
        // 안쪽 글이 낱말로 흩어져 첫 낱말만 `-c` 의 글이 되어 그 쓰기가 **통째로 샜다**.
        //
        // **`env -S` 의 글은 제 자로 가른다**([`split_string`]) — 셸이 아니라 낱말로 가를 뿐이다.
        // 셸로 읽던 판은 양쪽으로 틀렸다. 없는 쓰기를 지어내 **잘못 막았고**(`env -S 'echo x > f'`
        // 는 `>` 를 낱말로 넘길 뿐이다), 없는 **집기**를 지어내 규칙 2 를 채웠다 —
        // `env -S 'true && moai mv <id> in_progress' && 쓰기` 는 `/bin/true` 에 낱말을 넘길 뿐
        // moai 를 안 돌리는데, 그 집기가 뒤의 빈손 쓰기를 풀어 줬다. 더 보는 쪽이 늘 안전하다는
        // 셈이 **집기 축에서는 거꾸로** 선다: 지어낸 집기는 규칙을 덜 막게 한다.
        //
        // **`env -S` 가 가르는 것은 제 글 하나뿐이다** — 그 뒤의 피연산자는 env 가 통째로 argv 로
        // 넘긴다(안 가른다). 이어 붙여 함께 가르던 판은 없는 낱말을 지어냈고, 그것이 또 집기가 됐다:
        // `env -S "moai mv" "t-1 in_progress --from todo" && 쓰기` 는 실제로 자리 인자 **하나**를 든
        // `moai mv` 라 실패해 `&&` 가 끊기는데, 여섯 낱말로 갈라 읽은 판은 그것을 집기로 세어 뒤의
        // 빈손 쓰기를 풀어 줬다. 주석 하나(`env -S '#c' sed -i …`)에 글이 통째로 비던 것도 여기다 —
        // env 는 주석을 버리고 피연산자를 그대로 돌린다.
        let split_out;
        let tail: &[String] = if split {
            tail
        } else {
            let (raw, more) = match &glued {
                Some(g) => (Some(g.as_str()), tail),
                None => (tail.first().map(String::as_str), tail.get(1..).unwrap_or_default()),
            };
            split_out = raw.map(split_string).unwrap_or_default().into_iter().chain(more.iter().cloned()).collect::<Vec<_>>();
            &split_out
        };
        let text = tail.iter().map(|w| crate::text::quoted(w)).collect::<Vec<_>>().join(" ");
        // `sudo -s` 혼자는 사람이 쓸 셸을 띄운다 — 넘긴 글이 없다.
        return (!text.is_empty()).then_some(Handed { text, fork: true, strict: false });
    }
    match basename(head) {
        "bash" | "sh" | "zsh" | "dash" | "ksh" => {
            let mut it = rest.iter();
            // **띄울 때 켠 errexit**(moai-j9tx) — `bash -e`·`bash -o errexit` 로 띄운 셸은 글의 첫
            // 줄부터 `set -e` 아래다. `+e`·`+o errexit` 가 끄는 것도 셸이 읽는 차례 그대로다.
            let mut strict = false;
            // `-c` 를 봤는가 — 옵션 뒤의 첫 낱말이 글인지 스크립트인지를 이것이 가른다.
            let mut hands = false;
            let text = loop {
                let Some(w) = it.next() else { break None };
                // `--` 뒤는 옵션이 아니다 — `bash -c -- 'echo hi'` 가 `hi` 를 찍는다(리뷰 moai-p836.rv).
                if w == "--" {
                    break it.next();
                }
                // 값을 따로 받는 긴 옵션 — 그 값은 명령이 아니다.
                if matches!(w.as_str(), "--rcfile" | "--init-file" | "--wordexp") {
                    it.next();
                    continue;
                }
                if w.starts_with("--") {
                    continue;
                }
                // `-`·`+` 로 여는 짧은 옵션 뭉치. 아니면 옵션이 끝난 것이고, 그 낱말이 글이거나 스크립트다.
                let Some(flags) = w.strip_prefix(['-', '+']).filter(|f| !f.is_empty()) else { break Some(w) };
                let on = w.starts_with('-');
                hands |= flags.contains('c');
                // 뭉치의 글자를 **셸이 읽는 차례 그대로** 본다 — `-o pipefail`·`-O extglob` 은 저마다
                // 뒤 낱말 하나를 값으로 받고(뭉치의 끝이 아니어도 그렇다: `bash -oe pipefail`), 그중
                // `-o errexit` 만 뜻을 바꾼다.
                for f in flags.chars() {
                    match f {
                        'e' => strict = on,
                        'o' | 'O' => {
                            let val = it.next();
                            if f == 'o' && val.is_some_and(|v| v == "errexit") {
                                strict = on;
                            }
                        }
                        _ => {}
                    }
                }
            };
            match (hands, text) {
                (true, Some(t)) => Some(Handed { text: t.clone(), fork: true, strict }),
                _ => None,
            }
        }
        "eval" if !rest.is_empty() => Some(Handed { text: rest.join(" "), fork: false, strict: false }),
        _ => None,
    }
}

/// `env -S` 가 글 하나를 **낱말로** 가르는 자 — 셸이 아니다(GNU coreutils 의 `--split-string`).
///
/// 빈칸으로 가르고 `'`·`"`·`\` 만 본다. `&&`·`|`·`;`·`>`·`$( … )` 는 뜻 없는 낱말로 남는다 —
/// env 는 그것을 그대로 argv 로 넘긴다. **여기서 셸을 부르지 않는다**: [`shell_text`] 는
/// [`Lexer::relex`] 가 부르는데, 안에서 다시 렉서를 세우면 `env -S "env -S …"` 가 겹마다
/// 겹 셈([`Lexer::DEEP`])을 0 으로 되돌려 끝없이 판다.
///
/// **`\` 뒤의 글자는 env 의 표대로 푼다.** 맨 글자로 읽던 판은 `\_` 를 밑줄로 읽어
/// `env -S'sed\_-i\_s/a/b/\_src/x.rs'` 를 낱말 **하나**로 만들었고, 그 한 줄에서 규칙이 통째로
/// 꺼졌다(coreutils 9.4 에서 그 줄은 실제로 파일을 고친다). 덜 보는 쪽이 곧 구멍인 자리다.
fn split_string(text: &str) -> Vec<String> {
    let (mut out, mut word, mut open, mut had) = (Vec::new(), String::new(), None, false);
    let mut it = text.chars();
    let push = |out: &mut Vec<String>, word: &mut String, had: &mut bool| {
        if *had {
            out.push(std::mem::take(word));
            *had = false;
        }
    };
    while let Some(c) = it.next() {
        match c {
            // 작은따옴표 안에서는 `\` 도 그냥 글자다.
            '\\' if open != Some('\'') => match it.next() {
                // `\_` 는 **낱말을 가른다**(따옴표 안이면 그냥 빈칸), `\c` 는 글을 거기서 끝낸다.
                Some('_') if open.is_none() => push(&mut out, &mut word, &mut had),
                Some('c') => break,
                Some(e) => {
                    word.push(match e {
                        '_' => ' ',
                        't' => '\t',
                        'n' => '\n',
                        'f' => '\u{c}',
                        'r' => '\r',
                        'v' => '\u{b}',
                        e => e,
                    });
                    had = true;
                }
                None => {}
            },
            '\'' | '"' if open.is_none() => (open, had) = (Some(c), true),
            c if open == Some(c) => open = None,
            // 낱말을 **여는** `#` 는 글 끝까지 주석이다.
            '#' if open.is_none() && !had => break,
            c if open.is_none() && c.is_whitespace() => push(&mut out, &mut word, &mut had),
            c => {
                word.push(c);
                had = true;
            }
        }
    }
    push(&mut out, &mut word, &mut had);
    out
}

/// 셸에 넘긴 글 하나([`shell_text`]).
struct Handed {
    text: String,
    /// 새 셸에서 도는가 — `eval` 은 지금 셸에서 돈다.
    fork: bool,
    /// 띄울 때 이미 errexit 가 켜졌는가 — 그 글은 첫 줄부터 `set -e` 아래다([`Layer::Shell`]).
    strict: bool,
}

/// 명령줄의 한 토막 — 낱말들과, 리다이렉션이 쓰는 자리.
#[derive(Debug, Default)]
struct Seg {
    words: Vec<String>,
    /// `>`·`>>`·`>|`·`&>`·`>& 파일` 의 과녁. **낱말에서는 빠진다** — 빠지지 않으면
    /// `moai mv t-r done > /dev/null` 의 갈 칸이 `/dev/null` 로 읽혀, 리뷰를
    /// 닫는 규칙이 리다이렉션 하나로 샌다. 읽는 쪽(`<`·`<<<`·`<&`)의 과녁은
    /// 어디에도 안 든다 — 같은 까닭으로 낱말에 남으면 안 되고, 쓰는 것도 아니다.
    writes: Vec<String>,
    /// 몇 겹의 `( … )` 안인가 — 하위 셸이라 그 안의 `cd` 는 괄호를 나오면 풀린다([`aimed`]).
    depth: usize,
    /// 파이프의 한 칸이거나 `&` 로 띄운 것 — 제 하위 셸에서 돌아 `cd` 가 뒤로 안 이어진다.
    sub: bool,
    /// **뒤로 띄운 것 안에 있다**(moai-99df) — 제 토막이 `&` 로 끝났거나, `&` 로 띄운 묶음
    /// (`{ …; exit 1; } &`·`( … ) &`·`fi &`) 안의 줄이다. `Op::Any` 는 `;`·줄바꿈·`&` 를 한 낱말로
    /// 읽어 이것을 못 가른다 — 그래서 렉서가 `&` 를 보는 자리에서 바로 적는다.
    ///
    /// 두 자리가 읽는다. `exit` 은 **뒤로 띄운 것 안에서는 껍데기를 안 끝내고**([`shell_writes`] 의
    /// `quits`), 셸에 넘긴 글이 이것으로 끝나면 그 글의 값은 집기의 값이 아니다([`Lexer::relex`] 의
    /// `apace`). 뒤엣것을 `sub` 로 어림잡던 판은 `{ … } &` 를 못 보고(묶음 뒤의 `&` 는 빈 토막에 온다),
    /// 뒤로 띄운 파이프라인(`a | 집기 &`)을 파이프라고만 읽었다(리뷰 moai-k8j1.209 의 4·7번).
    bg: bool,
    /// 몇 겹의 묶음 안인가 — `( … )` 와 `{ … }` 를 함께 센다. [`Join::depth`] 와 같은 자다.
    /// [`Seg::depth`] 로 집기의 깊이를 재면 `{ mv; sed -i …; }` 의 `;` 가 집기보다 깊은 자리로
    /// 읽혀, 집기가 져도 도는 쓰기가 샌다.
    level: usize,
    /// 앞 토막과 무엇으로 이었나 — 앞이 이겨야만 도는가(`&&`), 한 파이프라인인가(`|`), 그 이음사를
    /// 몇 겹의 묶음 안에서 읽었나. [`shell_writes`] 가 집기 뒤의 쓰기를 넘길지를 이것으로 가른다.
    join: Join,
    /// 앞 토막을 떠나 여기 오기까지 **가장 얕았던** `( … )` 깊이(앞 토막 자신의 깊이는 안 든다).
    /// 깊이가 같아도 이것이 더 얕으면 딴 하위 셸이다 — 깊이만 보던 판은 `(cd /tmp); (echo x > f)`
    /// 의 `cd` 와 `(set -e; mv); (sed -i …)` 의 `set -e` 를 뒤 괄호로 넘겨 쓰기가 샜다.
    low: usize,
    /// 같은 것을 묶음 깊이([`Seg::level`])로 — `if …; fi` 를 나왔다 다시 든 것도 딴 묶음이다.
    floor: usize,
    /// 이 토막을 다시 읽은 겹들, 바깥 것부터([`Layer`]) — 비어 있지 않으면 제 목록의 명령이 아니라
    /// 바깥 토막이 든 글의 일부다. 겹의 종류가 둘이라 한 수로 세지 않는다: **치환의 값은 바깥 명령의
    /// 값이 아니지만**(`echo "$(moai mv …)" && sed -i …` 의 `&&` 는 `echo` 가 이겼다는 것뿐이다),
    /// **셸에 넘긴 글의 값은 바깥 토막의 값이다**(`bash -c 'moai mv …' && sed -i …`). 한 수로 세던 판은
    /// 뒤의 것까지 나올 때 집기를 버려 시킨 대로 친 줄을 막았다(moai-plfi, [`shell_writes`]).
    /// **뒤로 띄운 것으로 끝나는 글은 예외다**([`Layer::Shell`] 의 `apace`) — 그 글은 0 으로 끝나니
    /// 값이 안 흐른다. 치환처럼 센다.
    ///
    /// `eval` 의 글은 겹이 아니다 — 지금 셸에서 도니 `{ … }` 묶음과 같다([`Lexer::relex`]).
    nested: Vec<Layer>,
    /// 앞 토막 뒤로 닫힌 `if`·`case`·`while`·`for` 묶음 가운데 가장 바깥 것의 깊이 — `fi`·`esac`·`done`
    /// 을 지났다. 그 묶음의 값은 몸통이 안 돌았으면 0 이라, 몸통 안의 집기가 묶음 밖으로 이어지지 않는다.
    /// `{ … }` 는 몸통이 늘 돌아 안 센다.
    shut: Option<usize>,
    /// 이 토막에 **글로 흘러드는 것** — 이 토막이 연 heredoc(`<<`)의 본문(제 명령 치환 안의 것도)과
    /// here-string(`<<<`)의 낱말. 낱말에도 쓰기에도 안 든다. [`writes_korean`] 이 stdin 과 명령 치환으로
    /// 받는 글을 여기서 읽는다 — 명령줄 전체에서 찾던 판은 옆 토막의 한글(커밋 메시지·경로)에 알림을 달았다.
    fed: Vec<String>,
    /// 이 토막이 연 heredoc 의 번호([`Lexer::docs`]) — 본문은 토막을 닫은 뒤에 읽혀, `run` 이 끝에서 `fed` 로 옮긴다.
    docs: Vec<usize>,
}

/// 다시 읽은 글 한 겹([`Seg::nested`]) — **새 셸에서 도는 글**이고, 그 글의 맨 윗자리 묶음 깊이
/// ([`Seg::level`])를 든다. 새 셸은 제 `set -e` 를 제 맨 윗자리에서 센다([`shell_writes`], moai-tmi0).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Layer {
    /// 명령 치환(`$( … )`·`` `…` ``)과 따옴표 없는 heredoc 본문의 치환 — 바깥 명령의 **낱말**이 된다.
    /// 그 값은 바깥 명령의 값이 아니고, bash 는 그 안에 바깥의 `set -e` 를 안 물려준다. 바깥 목록이
    /// 그 안의 errexit 를 끌 수도 있다 — `x=$(set -e; …) || true`.
    Subst(usize),
    /// `bash -c '…'`·`sh -c` 의 글 — 새 프로세스라 바깥의 `set -e` 도 `||` 도 안 닿는다.
    Shell {
        /// 그 글의 맨 윗자리 묶음 깊이([`Seg::level`]) — 새 셸은 제 `set -e` 를 여기서 센다.
        top: usize,
        /// 그 글의 맨 윗자리 **하위 셸** 깊이([`Seg::depth`]) — 겹을 나올 때 끝내는 묶음을 푸는지를
        /// 이것으로 가른다(moai-4arw). 그 묶음이 이보다 깊으면 `exit` 는 하위 셸만 끝내니, 자식
        /// 셸의 값이 집기의 값이라고 말할 수 없다 — `집기 || ( …; exit 1 )` 이 그 꼴이다.
        ///
        /// **겹마다 한 번 재어 여기 둔다** — 겹에 들어선 토막에서 베껴 오던 판은 글이 `( … )` 로
        /// 시작하면 그 깊이를 부풀려, 같은 집안을 두 갈래로 갈랐다(리뷰 moai-k8j1.209 의 6번).
        /// 그 하위 셸이 글의 **마지막**이면 bash 는 그 값을 그대로 내니 실은 풀어도 되지만, 그것을
        /// 알려면 "마지막인가" 라는 자가 하나 더 든다 — 지금은 집안째 막는 쪽이다.
        deep: usize,
        /// **띄울 때 이미 errexit 가 켜졌는가**(moai-j9tx) — `bash -e -c '…'` 의 글은 첫 줄부터
        /// `set -e` 아래라, 그 겹을 열 때 `strict` 를 켠 채로 연다.
        strict: bool,
        /// **글이 뒤로 띄운 것으로 끝나는가** — `bash -c '집기 &'` 는 집기가 돌기도 전에 0 으로
        /// 끝나, 그 값이 집기의 값이 아니다. 나올 때 들어설 때의 집기를 도로 세운다(치환과 같다).
        /// **겹을 치환으로 바꿔 적던 판은 그 셸의 errexit 까지 함께 버렸다**(moai-54pk) — 값만
        /// 안 흘릴 뿐 그 글이 `set -e` 아래 돈 것은 그대로다.
        apace: bool,
    },
}

impl Layer {
    /// 바깥 렉서가 이 겹을 제 자리로 옮긴다 — 깊이는 [`Seg::level`] 과 같이 민다.
    fn shift(self, by: usize, under: usize) -> Layer {
        match self {
            Layer::Subst(l) => Layer::Subst(l + by),
            Layer::Shell { top, deep, strict, apace } => {
                Layer::Shell { top: top + by, deep: deep + under, strict, apace }
            }
        }
    }
}

/// 토막 사이의 이음사([`Seg::join`]).
#[derive(Debug, Default, Clone, Copy)]
struct Join {
    op: Op,
    /// 이음사를 읽은 자리의 묶음 깊이([`Seg::level`]).
    depth: usize,
}

/// 앞 토막이 어떻게 끝나야 이 토막이 도는가.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Op {
    /// `;`·`&`·줄바꿈 — 앞이 져도 돈다. 첫 토막도 이것이다.
    #[default]
    Any,
    /// `||` — 앞이 **져야** 돈다. [`Any`](Op::Any) 와 한 낱말로 읽던 판은 `mv … || exit 1;
    /// sed -i …` 를 막았다 — 겨루다 진 쪽을 끊으려고 흔히 쓰는 모양인데, 그 `exit` 가 돌면
    /// 뒤는 아예 안 돈다(moai-gtkn).
    Or,
    /// `&&` — 앞이 0 으로 끝나야 돈다.
    And,
    /// `|`·`|&` — 앞 칸과 한 파이프라인이다. 앞 칸이 져도 함께 돌고, **그 파이프라인에 들어선
    /// 판을 잇는다** — `&&` 보다 단단히 묶여 `mv && a | tee f` 의 `tee` 는 집기 뒤다.
    Pipe,
}

/// 명령을 **셸이 읽는 대로 한 걸음에** 읽는다 — 따옴표·명령 치환·산술·heredoc
/// 을 같은 자리에서 센다.
///
/// 앞선 판은 heredoc 을 줄 단위로 먼저 걷고 그 뒤에 따옴표를 셌다. 두 걸음이
/// 서로를 몰라 양쪽으로 틀렸다 — 따옴표나 주석 속 `<<` 가 뒤 줄을 통째로
/// 삼켰고, 본문에 인용된 `    MD` 가 본문을 일찍 끝내 나머지가 명령으로 읽혔다.
/// 규칙 2 가 생긴 뒤로 명령으로 잘못 읽힌 `a -> b` 는 곧 **아무것도 안 쓰는
/// 명령을 막는 거절**이다. `"$(echo "it's")"` 의 안쪽 `"` 를 바깥을 닫는 것으로
/// 읽어, 따옴표가 뒤 줄 전부에서 뒤집힌 것도 같은 뿌리다.
fn parse(cmd: &str) -> Vec<Seg> {
    Lexer::new(cmd).run()
}

/// [`parse`] 에 **마지막 토막 뒤로 닫힌 예약어 묶음**([`Lexer::run_over`])을 곁들여 — 글이
/// `fi`·`esac`·`done` 으로 끝나면 그 표식을 받을 토막이 없어 [`Seg::shut`] 으로는 안 온다.
/// 쓰기는 그 뒤가 없어 셈이 같지만, 기록([`shell_scan`])은 여기서 걷어야 안 돈 집기를 안 떠안는다.
fn parse_over(cmd: &str) -> (Vec<Seg>, Option<usize>) {
    Lexer::new(cmd).run_over()
}

/// 지금 무엇의 안에 있는가.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ctx {
    /// `'…'` — 아무것도 특별하지 않다.
    Single,
    /// `$'…'` — `\` 가 다음 글자를 감싼다. `\'` 는 닫지 않는다.
    Ansi,
    /// `"…"` — `\` 와 `$(` 만 특별하다.
    Double,
    /// `$( … )`·`<( … )`·`>( … )` — 안은 새 명령이라 따옴표를 새로 센다. 괄호
    /// 깊이를 든다. **안의 글은 바깥 명령의 낱말 하나다** — 토막으로 가르지 않는다.
    Subst(usize),
    /// `(( … ))`·`$(( … ))` — `>`·`<`·`;`·`&` 가 연산자가 아니라 산술이다.
    /// 괄호 깊이를 든다.
    Arith(usize),
    /// `` `…` `` — 옛 꼴의 명령 치환. [`Subst`](Ctx::Subst) 처럼 안의 글은 바깥의 낱말 하나고,
    /// 따로 새 명령으로 다시 읽는다([`Lexer::inner`]). `\` 가 `` ` ``·`\`·`$` 를 감싼다.
    Tick,
}

/// 다음 낱말이 무엇의 과녁인가.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Aim {
    /// 보통 낱말.
    #[default]
    Word,
    /// `>`·`>>`·`>|`·`&>`·`<>` — 쓰는 파일.
    Write,
    /// `<`·`<&` — 읽는 것. 낱말도 쓰기도 아니다.
    Read,
    /// `<<<` — 읽는 글 그 자체. 낱말도 쓰기도 아니고, 이 토막에 흘러드는 글이다([`Seg::fed`]).
    Here,
    /// `>&` — 숫자나 `-` 면 fd 를 잇는 것이고, 아니면 그 파일에 쓴다.
    Dup,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    all: Vec<Seg>,
    seg: Seg,
    cur: String,
    /// 따옴표가 있었다 — `""` 도 낱말 하나다.
    had: bool,
    aim: Aim,
    stack: Vec<Ctx>,
    /// `[[ … ]]` 안 — `>`·`<`·`&&`·`||`·괄호가 비교와 묶음이다.
    test: bool,
    /// 이 줄이 끝나면 건너뛸 heredoc 본문들 — 종료어와, 앞 탭을 걷는가(`<<-`), 본문을 적을 [`Lexer::docs`] 의 번호.
    heredocs: Vec<(String, bool, usize, bool)>,
    /// 건너뛴 heredoc 본문들 — 연 토막이 [`Seg::docs`] 로 번호를 든다.
    docs: Vec<String>,
    /// 지금 몇 겹의 `( … )` 묶음 안인가([`Seg::depth`]).
    group: usize,
    /// 마지막으로 읽은 이음사 — 다음에 쌓이는 토막의 [`Seg::join`] 이 된다.
    join: Join,
    /// 몇 겹의 `{ … }` 와 예약어 묶음(`if … fi`) 안인가. **이음사와 집기의 깊이
    /// ([`Seg::level`])에만 든다** — 하위 셸이
    /// 아니라 그 안의 `cd` 는 뒤로 이어지므로 [`Seg::depth`] 에는 안 든다. 안 세면
    /// `mv && { a; b; }` 의 `;` 가 묶음 밖의 끊김으로 읽혀, 집기가 이겨야만 도는 쓰기를 막는다.
    braces: usize,
    /// 이 토막의 낱말에 든 명령 치환들의 글 — 토막을 닫을 때 새 명령으로 다시 읽어 **그 앞에**
    /// 쌓는다(moai-xe6e). 치환의 글을 바깥 낱말 하나로만 두던 판은 `echo "$(moai add x)"` 와
    /// `` `moai mv <리뷰> done` `` 을 아무 규칙도 안 보고 넘겼다 — 셸은 그것을 먼저 돌린다.
    inner: Vec<String>,
    /// 맨 바깥 치환이 `cur` 의 어디서 시작했나 — 그 안에 든 치환은 다시 읽을 때 센다.
    mark: Option<usize>,
    /// 마지막으로 쌓은 토막 뒤로 내려간 가장 얕은 `( … )` 깊이와 묶음 깊이([`Seg::low`]·[`Seg::floor`]).
    /// 쌓을 때마다 비운다(`usize::MAX`).
    low: usize,
    floor: usize,
    /// 마지막으로 쌓은 토막 뒤로 닫힌 예약어 묶음의 가장 얕은 깊이([`Seg::shut`]).
    shut: Option<usize>,
    /// **명령으로 다시 읽을 heredoc 본문의 치환들**([`Lexer::relex`]) — 따옴표 없는 본문의 `$( … )`
    /// 와 백틱이다(moai-t863). 낱말로만 두던 판은 셸이 실제로 돌리는 그 명령을 아무 규칙에도
    /// 안 보였다. **본문 번호([`Lexer::docs`])를 함께 든다** — 그 본문을 연 토막 바로 앞에 심어야
    /// 차례가 맞는다.
    later: Vec<(usize, String)>,
    /// 다시 읽기를 몇 겹까지 왔나 — `bash -c "bash -c '…'"` 가 끝없이 파고들지 않게 막는다.
    deep: usize,
}

impl<'a> Lexer<'a> {
    fn new(cmd: &'a str) -> Self {
        Lexer::at(cmd, 0)
    }

    /// 다시 읽기의 겹을 이어받는 렉서 — [`Lexer::relex`] 와 치환이 쓴다.
    fn at(cmd: &'a str, deep: usize) -> Self {
        Lexer {
            chars: cmd.chars().peekable(),
            all: Vec::new(),
            seg: Seg::default(),
            cur: String::new(),
            had: false,
            aim: Aim::Word,
            stack: Vec::new(),
            test: false,
            heredocs: Vec::new(),
            docs: Vec::new(),
            group: 0,
            join: Join::default(),
            braces: 0,
            inner: Vec::new(),
            mark: None,
            low: usize::MAX,
            floor: usize::MAX,
            shut: None,
            later: Vec::new(),
            deep,
        }
    }

    /// 다시 읽기의 상한 — 이보다 깊으면 글을 글로 둔다. 막는 것은 **되돌이가 스택을 넘는 것**이다:
    /// `$(`5000겹은 상한이 없으면 `fatal runtime error: stack overflow` 로 훅을 죽이고, 죽은 훅은
    /// 규칙 넷을 통째로 연다(리뷰 moai-p836.rv).
    ///
    /// **치환([`Lexer::end_by`])과 셸에 넘긴 글([`Lexer::relex`])이 한 예산을 나눠 쓴다.** 치환이
    /// 겹을 0 으로 되돌리던 판은 `$(bash -c "$( … )")` 한 줄로 이 상한을 통째로 지나갔다.
    ///
    /// **숫자는 넉넉히 잡는다.** 상한을 넘은 글은 그냥 지나가므로(새는 쪽), 상한이 낮으면 그것이
    /// 곧 한 줄짜리 우회로다 — 여덟이던 판은 `$( … )` 아홉 겹 46바이트로 규칙 1 을 껐다. 실제
    /// 명령줄은 서넛을 안 넘고, 예순넷 겹은 스택에도 시간에도 값이 없다.
    const DEEP: usize = 64;

    /// 묶음을 나왔다 — 다음에 쌓는 토막이 그 사이의 가장 얕은 자리를 안다([`Seg::low`]).
    fn sank(&mut self) {
        self.low = self.low.min(self.group);
        self.floor = self.floor.min(self.level());
    }

    /// **방금 닫힌 묶음의 토막마다 표를 단다** — `{ …; exit 1; } &`·`{ …; exit 1; } | cat` 의
    /// `&`·`|` 는 낱말 없는 토막에 와, 하위 셸로 도는 것은 그 묶음 쪽이다. 지금 자리보다 깊은
    /// 자리에 선 토막들이 그것이다([`Seg::sub`]·[`Seg::bg`]).
    ///
    /// **앞선 형제 묶음까지 거슬러 가지 않는다.** 깊이만 보고 끝까지 훑던 판은
    /// `{ a; }; { b; } &` 의 `a` 까지 띄운 것으로 세, 그 안의 `exit`·`set -e`·`cd` 를 통째로
    /// 잃었다 — `집기 || { …; exit 1; }; { … } | cat; 쓰기` 가 이긴 집기를 놓쳐 막혔다. 묶음의
    /// 첫 토막은 **거기 오기까지 지나온 가장 얕은 자리**([`Seg::floor`])가 지금 자리까지 올라간
    /// 것이라, 그것을 적고 멈춘다.
    ///
    /// 멈추는 것은 값이기도 하다 — `{ … } | { … } | …` 처럼 모든 토막이 묶음 안에 선 줄에서는
    /// 끝까지 훑는 판이 연산자마다 목록 전체를 되짚어 줄 길이의 **제곱**이 된다(6,400 묶음
    /// 134KB 한 줄에 12초). 훅이 제 시간에 못 끝나면 규칙이 통째로 열린다.
    /// **제 토막에 표를 단다** — `cmd &`·`cmd | cat` 의 그 토막이다. 낱말도 쓰는 자리도 없는
    /// 토막은 토막이 아니라 빈 자리라 안 단다([`Lexer::end_by`] 가 그런 것을 안 쌓는다).
    ///
    /// **쓰는 자리만 있는 토막도 토막이다**(리뷰 moai-k8j1.udq 의 3번) — `{ …; } >/dev/null &` 의
    /// `>/dev/null` 은 묶음 **밖**에 제 토막으로 서고, 그것이 글의 마지막 토막이다. 낱말만 보던
    /// 판은 거기에 `bg` 를 안 달아, `bash -c '{ 집기; } >/dev/null &' && 쓰기` 의 글이 뒤로 띄운
    /// 것으로 안 읽혔다([`Lexer::relex`] 의 `apace` 가 마지막 토막을 본다).
    fn mark_here(&mut self, mark: impl Fn(&mut Seg)) {
        if !self.seg.words.is_empty() || !self.seg.writes.is_empty() {
            mark(&mut self.seg);
        }
    }

    fn mark_group(&mut self, mark: impl Fn(&mut Seg)) {
        // `}|` 처럼 사이에 빈칸이 없으면 닫는 낱말이 아직 `cur` 에 있다 — 흘려야 묶음이 닫히고
        // 이 토막이 비었는지가 제대로 보인다.
        self.flush();
        if !self.seg.words.is_empty() || self.join.depth <= self.level() {
            return;
        }
        let home = self.level();
        for s in self.all.iter_mut().rev() {
            if s.level <= home {
                break;
            }
            let first = s.floor <= home;
            mark(s);
            if first {
                break;
            }
        }
    }

    /// 토막 하나를 쌓는다 — 앞 토막 뒤로 지나온 가장 얕은 자리를 적고 새로 잰다.
    fn push(&mut self, mut seg: Seg) {
        seg.low = self.low.min(seg.depth);
        seg.floor = self.floor.min(seg.level);
        seg.shut = seg.shut.into_iter().chain(self.shut.take()).min();
        self.low = usize::MAX;
        self.floor = usize::MAX;
        self.all.push(seg);
    }

    /// 명령 치환을 연다 — 여는 글자는 이미 `cur` 에 있다. **맨 바깥 것만 자리를 적는다.**
    fn enter(&mut self, ctx: Ctx) {
        if !self.opaque() {
            self.mark = Some(self.cur.len());
        }
        self.stack.push(ctx);
    }

    /// 명령 치환을 닫았다 — 닫는 글자는 아직 `cur` 에 안 넣었다. 맨 바깥 것이었으면 그 글을 적는다.
    fn leave(&mut self) {
        if !self.opaque()
            && let Some(at) = self.mark.take()
        {
            self.inner.push(self.cur[at..].to_string());
        }
    }

    /// `` `…` `` 안.
    fn tick(&mut self, c: char) {
        match c {
            '`' => {
                self.stack.pop();
                self.leave();
                self.cur.push(c);
            }
            '\\' => match self.chars.next() {
                Some(n @ ('`' | '\\' | '$')) => self.cur.push(n),
                Some(n) => {
                    self.cur.push('\\');
                    self.cur.push(n);
                }
                None => self.cur.push('\\'),
            },
            _ => self.cur.push(c),
        }
    }

    fn run(self) -> Vec<Seg> {
        self.run_over().0
    }

    /// [`Lexer::run`] 에 **마지막 토막 뒤로 닫힌 예약어 묶음**([`Seg::shut`])을 곁들여 낸다.
    ///
    /// 글이 `fi`·`esac`·`done` 으로 끝나면 그 표식을 받을 토막이 그 글 안에 없어 버려진다. 셸에
    /// 넘긴 글에서는 **그 글을 낸 토막이 바로 그 자리**라([`Lexer::relex`]), 버리면 몸통이 안 돈
    /// 묶음 안의 집기가 바깥 `&&` 로 이어진다 — `bash -c 'if false; then 집기; fi' && 쓰기` 가
    /// 아무것도 안 집은 채 지나갔다(새는 쪽).
    fn run_over(mut self) -> (Vec<Seg>, Option<usize>) {
        while let Some(c) = self.chars.next() {
            match self.stack.last().copied() {
                None => self.plain(c),
                Some(Ctx::Single) => self.single(c),
                Some(Ctx::Ansi) => self.escaped(c, '\''),
                Some(Ctx::Double) => self.double(c),
                Some(Ctx::Subst(depth)) => self.subst(c, depth),
                Some(Ctx::Arith(depth)) => self.arith(c, depth),
                Some(Ctx::Tick) => self.tick(c),
            }
        }
        self.end_by(None);
        self.relex();
        let docs = std::mem::take(&mut self.docs);
        let over = self.shut;
        let segs = self
            .all
            .into_iter()
            .filter(|s| !s.words.is_empty() || !s.writes.is_empty())
            .map(|mut s| {
                // 번호는 이 렉서의 것이다 — 비워 두지 않으면 바깥 렉서가 치환 안의 토막을 다시 쌓을 때
                // 제 번호로 읽어 남의 본문을 붙인다.
                let own = std::mem::take(&mut s.docs);
                s.fed.extend(own.into_iter().filter_map(|n| docs.get(n).cloned()));
                s
            })
            .collect();
        (segs, over)
    }

    /// **셸이 글을 명령으로 돌리는 자리를 다시 읽는다** — 따옴표 없는 heredoc 본문의 치환
    /// ([`Lexer::later`], moai-t863)과 `bash -c '…'`·`eval '…'` 의 글([`shell_text`], moai-455j)이다.
    ///
    /// **그 글을 낸 토막 바로 앞에 심는다.** 뒤에 몰아 쌓던 판은 줄 끝의 판을 물려받아, 뒤따르는
    /// `cd` 가 그 앞의 쓰기를 모르는 자리로 지웠고(샜다), 앞선 `집기 && bash -c '쓰기'` 의 `&&` 를
    /// 잃어 시킨 대로 쓴 줄을 막았다(리뷰 moai-p836.rv). 심는 셈은 치환([`Lexer::end_by`])의 것과
    /// 같다 — 첫 토막이 바깥 이음사를 받고, 나머지는 안쪽이 잰 자리를 그대로 옮긴다.
    ///
    /// **글의 값은 그 글을 낸 토막의 값이다** — `bash -c`·`eval` 은 글의 마지막 명령의 값으로 끝난다.
    /// 그래서 그 글은 `( … )` 묶음처럼 한 겹 깊이 심고, 글을 낸 토막은 **그 묶음을 막 나온 토막**으로
    /// 선다 — 이음사의 깊이를 제 깊이보다 깊게 둔다(`( … ) > f` 의 `> f` 와 같은 자리다). 그러면
    /// [`shell_writes`] 가 그 토막에서 집기를 끊지 않고 묶음의 값을 뒤로 흘린다(moai-plfi). 치환은
    /// 바깥 명령의 낱말일 뿐이라 이렇게 서지 않는다 — **뒤로 띄운 것으로 끝나는 글도 그렇다**
    /// (`apace`, 리뷰 moai-k8j1.209): 그 값은 0 이라 바깥 토막의 값이 아니니, `feeds` 도 `handed` 도
    /// 안 선다. 서게 두던 판은 그 토막이 제 이음사를 안 읽어 `집기; bash -c '무엇 &' && 쓰기` 가 샜다.
    ///
    /// **`eval` 은 하위 셸이 아니다** — 지금 셸에서 돌아 그 글의 `cd` 가 바깥에 남는다. 그래서 `{ … }`
    /// 처럼 묶음 깊이([`Seg::level`])만 더하고 하위 셸 깊이([`Seg::depth`])와 겹([`Seg::nested`])은 안
    /// 더한다. 글을 그 자리에 납작하게 풀지 않는 것은 `집기 && eval 'a; b'` 의 `b` 도 집기 뒤라서다 —
    /// 풀면 `;` 가 바깥 목록의 끊김으로 읽혀 시킨 대로 친 줄을 막는다. 그 대가로 `eval 'set -e'` 는
    /// `{ set -e; }` 처럼 묶음 밖으로 안 이어진다(모르는 쪽, 막는 쪽으로 선다).
    ///
    /// **제 토막만 본다**(겹이 없는 것). 치환에서 옮겨 온 토막은 그 안쪽 렉서가 이미 다시 읽었다 —
    /// 다시 훑던 판은 `$(bash -c "$( … )")` 겹마다 같은 글을 두 번 읽어, 아홉 겹 211바이트 한 줄에
    /// 27초를 썼다(리뷰 moai-p836.rv). 훅이 제 시간에 못 끝나면 규칙이 통째로 열린다.
    fn relex(&mut self) {
        if self.deep >= Lexer::DEEP {
            return;
        }
        let docs = std::mem::take(&mut self.later);
        let deep = self.deep;
        let mut all = Vec::with_capacity(self.all.len());
        for mut seg in std::mem::take(&mut self.all) {
            if !seg.nested.is_empty() {
                all.push(seg);
                continue;
            }
            // 글마다 무슨 겹으로 심는가 — `None` 이면 겹 없이(`eval`).
            let mut texts: Vec<(&str, Option<Layer>)> = docs
                .iter()
                .filter(|(n, _)| seg.docs.contains(n))
                .map(|(_, t)| (t.as_str(), Some(Layer::Subst(0))))
                .collect();
            let own = shell_text(&seg.words);
            texts.extend(own.iter().map(|h| {
                (h.text.as_str(), h.fork.then_some(Layer::Shell { top: 0, deep: 0, strict: h.strict, apace: false }))
            }));
            let (depth, level, join, apart) = (seg.depth, seg.level, seg.join, seg.sub);
            // 앞 토막 뒤로 지나온 자리는 처음 심는 토막이 든다. 글을 낸 토막은 그 글을 막 나온 자리다.
            let (low, floor) = (seg.low, seg.floor);
            let mut first = true;
            // 셸에 넘긴 글이 토막을 냈다 — 글을 낸 토막이 그 묶음을 닫는다.
            let mut handed = false;
            // 그 글 끝에서 닫힌 예약어 묶음([`Seg::shut`]) — 받을 토막이 글 안에 없어 이 토막이 든다.
            let mut over: Option<usize> = None;
            for (text, layer) in texts {
                // 하위 셸이면 한 겹 깊다 — 그 안의 `cd` 가 바깥 자리를 안 흔든다. **`eval` 도 제 토막이
                // 파이프의 칸이거나 `&` 로 띄운 것이면 하위 셸이다**([`Seg::sub`]) — `eval 'cd /b' | cat`
                // 의 `cd` 는 뒤로 안 이어지고 `집기 || eval 'exit 1' | cat` 의 `exit` 는 그 칸만 끝낸다.
                // 지금 셸에서 돈다고만 세던 판은 그 둘을 바깥 셸의 것으로 읽어, 아무것도 안 집은 채
                // 쓰는 줄이 샜다.
                let (segs, left) = Lexer::at(text, deep + 1).run_over();
                // **뒤로 띄운 것으로 끝나는 글의 값은 0 이다** — `bash -c '집기 &'` 는 집기가 돌기도
                // 전에 0 으로 끝난다. 그 값은 이 토막의 값이 아니니 치환으로 심는다: 나올 때 집기를
                // 도로 세운다. 파이프의 마지막 칸은 아니다 — `cat f | 집기` 의 값은 집기의 값이다.
                // **글이 뒤로 띄운 것으로 끝나는가**(moai-99df) — 렉서가 `&` 를 보는 자리에서 적어
                // 둔 표를 읽는다([`Seg::bg`]). `sub` 로 어림잡던 판은 `{ … } &` 로 끝나는 글을 못 보고
                // (묶음 뒤의 `&` 는 빈 토막에 온다), 뒤로 띄운 파이프라인은 파이프라고만 읽었다.
                let apace = segs.last().is_some_and(|s| s.bg);
                // 셸에 넘긴 글은 **겹을 그대로 두고 표만 단다**(moai-54pk) — 치환으로 바꿔 적던 판은
                // 값을 안 흘리는 김에 그 셸의 errexit 까지 버렸다. `eval` 의 글은 겹이 없으니 그때만
                // 치환으로 심는다: 값이 제 것이 아닌 것은 같고, 버릴 errexit 도 없다.
                let layer = match (apace, layer) {
                    // 이름을 `deep` 으로 두면 이 함수 머리의 **다시 읽기 겹**(`let deep = self.deep`)을
                    // 가린다(리뷰 moai-k8j1.udq) — 둘 다 `usize` 라 한쪽을 다른 쪽으로 옮겨 적어도
                    // 컴파일이 되고, 그 겹은 훅이 스택을 안 넘게 막는 유일한 자다.
                    (true, Some(Layer::Shell { top, deep: under, strict, .. })) => {
                        Some(Layer::Shell { top, deep: under, strict, apace })
                    }
                    (true, None) => Some(Layer::Subst(0)),
                    (_, layer) => layer,
                };
                let sub = usize::from(layer.is_some() || apart);
                // 치환의 묶음은 바깥 토막의 것이 아니다 — 그 값도 바깥 명령의 값이 아니다.
                // **뒤로 띄운 것으로 끝나는 글은 값이 안 흐른다**(리뷰 moai-k8j1.209) — `apace` 가
                // 뜻하는 것이 바로 그것이다. 겹만 `Shell` 로 되돌리고 이 자를 안 고치던 판은 그 토막을
                // **묶음을 막 나온 토막**으로 세워(`handed`) 제 이음사를 안 읽게 만들었고,
                // `집기; bash -c '무엇 &' && 쓰기` 의 `;` 가 끊은 사슬이 도로 살아나 빈손의 쓰기가 샜다.
                let feeds = !matches!(layer, Some(Layer::Subst(_)) | Some(Layer::Shell { apace: true, .. }));
                // **글 끝에서 닫힌 묶음의 표식은 값과 따로 논다** — 그 묶음은 값이 흐르든 말든 정말
                // 닫혔고, 몸통이 안 돌았으면 그 안의 집기는 안 돌았다. 값과 한 자로 세던 판은
                // `bash -c 'if false; then 집기 & fi'` 의 안 돈 집기를 기록에 남겼다.
                if !matches!(layer, Some(Layer::Subst(_))) {
                    over = over.into_iter().chain(left.map(|l| l + level + 1)).min();
                }
                for (m, mut s) in segs.into_iter().enumerate() {
                    if m == 0 {
                        // 바깥에서 내려온 토막 — 지나온 자리는 바깥 자리다. 맨 처음 것은 이음사도,
                        // 앞 토막 뒤로 지나온 자리도 바깥의 것을 받는다(`집기 && bash -c '쓰기'`).
                        if std::mem::take(&mut first) {
                            (s.low, s.floor, s.join) = (low, floor, join);
                        } else {
                            (s.low, s.floor) = (depth, level);
                            s.join.depth += level + 1;
                        }
                    } else {
                        s.low += depth + sub;
                        s.floor += level + 1;
                        s.join.depth += level + 1;
                    }
                    s.depth += depth + sub;
                    s.level += level + 1;
                    s.shut = s.shut.map(|l| l + level + 1);
                    // 제자리에서 민다 — 새 목록을 지으면 토막마다 겹마다 힙을 한 번씩 잡아,
                    // `$( … )` 예순네 겹 202바이트 한 줄이 1,104번에서 3,184번으로 뛴다.
                    for l in &mut s.nested {
                        *l = l.shift(level + 1, depth + sub);
                    }
                    if let Some(l) = layer {
                        s.nested.insert(0, l.shift(level + 1, depth + sub));
                    }
                    handed |= feeds;
                    all.push(s);
                }
            }
            if !first {
                // 심은 글에서 제 자리로 돌아왔다 — 지나온 자리는 이제 제 자리다.
                (seg.low, seg.floor) = (depth, level);
            }
            if handed {
                seg.join.depth = level + 1;
            }
            // **글 끝에서 닫힌 묶음은 이 토막이 지나온 것이다** — 몸통이 안 돌았으면 그 값은 0 이라,
            // 그 안의 집기가 이 토막의 `&&` 로 이어지지 않는다([`shell_writes`]).
            seg.shut = seg.shut.into_iter().chain(over).min();
            all.push(seg);
        }
        self.all = all;
    }

    /// 따옴표 밖, 명령 치환 밖 — 셸의 연산자가 뜻을 갖는 자리.
    ///
    /// **리다이렉션은 여기서만 읽는다.** 토큰이 된 뒤에는 `">"` 와 `>` 가 같은
    /// 글자라, 그때 가서 찾으면 `moai note t-1 "a > src/x.rs"` 가 쓰기로 읽혀
    /// 메모가 막힌다.
    fn plain(&mut self, c: char) {
        // `]]` 는 비교를 닫는다. 붙어 온 `&&` 는 그 뒤의 것이다.
        if self.test && self.cur == "]]" && matches!(c, '&' | '|' | ';' | '<' | '>' | '(' | ')') {
            self.flush();
        }
        let next = self.chars.peek().copied();
        match c {
            '\'' => self.open(Ctx::Single),
            '"' => self.open(Ctx::Double),
            '$' if next == Some('\'') => {
                self.chars.next();
                self.open(Ctx::Ansi);
            }
            '$' if next == Some('(') => self.dollar(),
            '`' => {
                self.cur.push(c);
                self.had = true;
                self.enter(Ctx::Tick);
            }
            // 따옴표 밖의 `\>` 는 글자다. 줄 끝의 `\` 는 줄을 잇는다.
            '\\' => match self.chars.next() {
                Some('\n') | None => {}
                Some(n) => self.cur.push(n),
            },
            // 낱말 머리의 `#` 부터 줄 끝까지는 주석이다. 주석 속 `a -> b` 를
            // 쓰기로 읽으면 명령이 엉뚱하게 막힌다.
            '#' if self.at_word_start() => {
                while self.chars.next_if(|d| *d != '\n').is_some() {}
            }
            // `[[ a > b || c < d ]]` 는 비교다. 토막도 가르지 않는다.
            '<' | '>' | '&' | '|' | '(' | ')' if self.test => self.cur.push(c),
            '(' if next == Some('(') && self.at_word_start() => {
                self.chars.next();
                self.cur.push_str("((");
                self.stack.push(Ctx::Arith(2));
            }
            // `<( … )`·`>( … )` 는 프로세스 치환이다. 파일이 아니다.
            '<' | '>' if next == Some('(') && self.at_word_start() => {
                self.chars.next();
                self.cur.push(c);
                self.cur.push('(');
                self.enter(Ctx::Subst(1));
            }
            '<' => self.read_from(),
            '>' => self.write_to(),
            // `&>`·`&>>` 는 stdout·stderr 를 파일에 쓴다. 토막을 가르지 않는다.
            '&' if next == Some('>') => {
                self.chars.next();
                self.chars.next_if_eq(&'>');
                self.flush();
                self.aim = Aim::Write;
            }
            // **토막을 먼저 가른다.** 공백 갈래가 먼저 오면 줄바꿈이 낱말만
            // 끊고 토막은 안 끊는다 — 그 한 줄 차이로 규칙이 통째로 샜다.
            // 빈 토막 뒤 줄바꿈은 **묶음을 막 나온 뒤에만** 이음사다. 이음사나 `(` 뒤의 줄바꿈은
            // 셸이 건너뛴다 — `a &&\n b` 는 `&&` 로, `a; (\n b )` 는 `;` 로 이어진 것이다. 앞의
            // `;` 를 괄호 안 깊이의 줄바꿈으로 덮던 판은 그 `b` 를 집기 뒤로 읽어 샜다. 묶음을
            // 나온 뒤(`a && ( b )\n c`)의 이음사는 괄호 안의 것이라 여기서 바깥 것으로 선다.
            '\n' => {
                self.flush();
                let empty = self.seg.words.is_empty() && self.seg.writes.is_empty();
                let left_group = self.join.depth > self.level();
                self.end_by(if empty && !left_group { None } else { Some(Op::Any) });
                self.skip_heredocs();
            }
            // `||`·`&&` 는 이어 도는 갈래다. 홀로 선 `|`·`|&` 는 양쪽을, `&` 는 앞을 하위 셸로
            // 돌린다 — 그 `cd` 는 뒤로 안 이어진다([`Seg::sub`]).
            '|' if self.chars.next_if_eq(&'|').is_some() => self.end_by(Some(Op::Or)),
            '&' if self.chars.next_if_eq(&'&').is_some() => self.end_by(Some(Op::And)),
            '|' => {
                self.chars.next_if_eq(&'&');
                self.mark_here(|s| s.sub = true);
                // **묶음을 파이프에 문 `|` 도 빈 토막에 온다**(moai-4arw) — `{ …; exit 1; } | cat` 의
                // `|` 앞에는 낱말이 없어, 하위 셸로 도는 것은 방금 닫힌 묶음 쪽이다. 적지 않던 판은
                // 그 안의 `exit` 를 껍데기를 끝내는 것으로 읽어,
                // `bash -c '집기 || { exit 1; } | cat' && 쓰기` 가 집기 없이 지나갔다
                // (리뷰 moai-k8j1.209 의 8번).
                self.mark_group(|s| s.sub = true);
                self.end_by(Some(Op::Pipe));
                self.seg.sub = true;
            }

            '&' => {
                self.flush();
                self.mark_here(|s| {
                    s.sub = true;
                    s.bg = true;
                });
                // **묶음을 띄운 `&` 도 빈 토막에 온다**(moai-99df) — `{ …; exit 1; } &` 의 `&` 앞에는
                // 낱말이 없다. 적지 않던 판은 그 안의 `exit` 를 껍데기를 끝내는 것으로 읽어,
                // `집기 || { …; exit 1; } & 쓰기` 가 집기 없이 지나갔다.
                //
                // **`sub` 도 함께 단다** — 띄운 묶음은 제 하위 셸에서 돈다. `bg` 만 달던 판은
                // `{ cd /b; } & moai add x` 의 `cd` 를 바깥에 남겨([`aimed`] 가 `sub` 를 읽는다)
                // 그 `moai` 를 남의 트래커로 돌렸다 — `( cd /b ) &` 와 같은 줄이 갈라져 있었다.
                self.mark_group(|s| {
                    s.sub = true;
                    s.bg = true;
                });
                self.end_by(Some(Op::Any));
            }
            ';' => self.end_by(Some(Op::Any)),
            // 괄호는 이음사가 아니다 — `a && ( b )` 의 `b` 는 `&&` 로 이어진 것이다.
            ')' => {
                self.flush();
                let closes = !self.seg.words.is_empty() || !self.seg.writes.is_empty();
                self.end_by(None);
                // **묶음을 닫는 `)` 는 접두어만 선 토막이어도 이음사를 덮는다** — 덮지 않는 것은 `(` 앞의
                // `time`·`!` 뿐이다. 안 덮던 판은 `mv && (FOO=1)` 뒤 줄바꿈을 묶음 밖의 끊김으로 못 읽어,
                // 다음 줄을 `&&` 뒤로 넘겼다(리뷰 moai-ju21.70g).
                if closes {
                    self.join = Join { op: Op::default(), depth: self.level() };
                }
                self.group = self.group.saturating_sub(1);
                self.sank();
            }
            // `( cd /tmp && … )` 의 괄호는 묶음이다 — 명령 자리가 그 뒤에서 다시 선다.
            '(' if self.at_word_start() => {
                self.end_by(None);
                self.group += 1;
            }
            c if c.is_whitespace() => self.flush(),
            c => self.cur.push(c),
        }
    }

    fn open(&mut self, ctx: Ctx) {
        self.stack.push(ctx);
        self.had = true;
    }

    /// 따옴표를 닫는다. 명령 치환 안이면 글자째 남긴다 — 그 글은 바깥의 낱말 하나다.
    fn close(&mut self, c: char) {
        self.stack.pop();
        if self.opaque() {
            self.cur.push(c);
        }
    }

    /// 명령 치환 안인가. 그 안은 규칙이 가르지 않는 한 덩이다.
    fn opaque(&self) -> bool {
        self.stack.iter().any(|c| matches!(c, Ctx::Subst(_)))
    }

    fn single(&mut self, c: char) {
        if c == '\'' {
            self.close(c);
        } else {
            self.cur.push(c);
        }
    }

    /// `\` 가 다음 글자를 감싸는 따옴표 안. `quote` 가 나오면 닫는다.
    fn escaped(&mut self, c: char, quote: char) {
        if c == quote {
            self.close(c);
        } else if c == '\\' {
            if self.opaque() {
                self.cur.push(c);
            }
            if let Some(n) = self.chars.next() {
                self.cur.push(n);
            }
        } else {
            self.cur.push(c);
        }
    }

    /// `"…"` 안. `$(` 는 **새 명령을 연다** — 그 안의 `"` 는 바깥을 닫지 않는다.
    fn double(&mut self, c: char) {
        if c == '$' && self.chars.peek() == Some(&'(') {
            self.dollar();
        } else if c == '`' && !self.opaque() {
            self.cur.push(c);
            self.enter(Ctx::Tick);
        } else {
            self.escaped(c, '"');
        }
    }

    /// `$(` 나 `$((` — `$` 는 이미 읽었고 다음 글자가 `(` 다.
    fn dollar(&mut self) {
        self.chars.next();
        if self.chars.next_if_eq(&'(').is_some() {
            self.cur.push_str("$((");
            self.stack.push(Ctx::Arith(2));
        } else {
            self.cur.push_str("$(");
            self.enter(Ctx::Subst(1));
        }
    }

    /// 명령 치환 안 — 괄호와 따옴표만 세어 **어디서 닫히는지**를 찾는다.
    fn subst(&mut self, c: char, depth: usize) {
        if c == '$' && matches!(self.chars.peek(), Some('(')) {
            self.dollar();
            return;
        }
        self.cur.push(c);
        match c {
            '(' => self.retop(Ctx::Subst(depth + 1)),
            ')' if depth == 1 => {
                self.cur.pop();
                self.stack.pop();
                self.leave();
                self.cur.push(c);
            }
            ')' => self.retop(Ctx::Subst(depth - 1)),
            '\'' => self.stack.push(Ctx::Single),
            '"' => self.stack.push(Ctx::Double),
            '$' if self.chars.peek() == Some(&'\'') => {
                self.chars.next();
                self.cur.push('\'');
                self.stack.push(Ctx::Ansi);
            }
            '\\' => {
                if let Some(n) = self.chars.next() {
                    self.cur.push(n);
                }
            }
            // 안에서도 heredoc 은 heredoc 이다 — `git commit -m "$(cat <<'EOF' … EOF )"`.
            '<' if self.chars.peek() == Some(&'<') => {
                self.chars.next();
                if self.chars.next_if_eq(&'<').is_none() {
                    self.heredoc();
                }
            }
            '\n' => self.skip_heredocs(),
            _ => {}
        }
    }

    /// 산술 안 — `>` 는 비교고 `;` 는 `for ((…; …; …))` 의 칸막이다.
    fn arith(&mut self, c: char, depth: usize) {
        if c.is_whitespace() && self.stack.len() == 1 {
            self.flush();
            return;
        }
        // 산술 안의 명령 치환도 먼저 돈다 — `$(( $(moai add x) ))` 를 글자로만 넘기던 자리다.
        if c == '$' && self.chars.peek() == Some(&'(') {
            self.dollar();
            return;
        }
        self.cur.push(c);
        if c == '`' {
            self.enter(Ctx::Tick);
            return;
        }
        match c {
            '(' => self.retop(Ctx::Arith(depth + 1)),
            ')' if depth == 1 => {
                self.stack.pop();
            }
            ')' => self.retop(Ctx::Arith(depth - 1)),
            _ => {}
        }
    }

    fn retop(&mut self, ctx: Ctx) {
        if let Some(top) = self.stack.last_mut() {
            *top = ctx;
        }
    }

    /// `<` 를 읽었다. **읽는 리다이렉션의 과녁은 낱말이 아니다** — 남으면 `tee`
    /// 가 그것을 쓰는 파일로 세고(`tee /tmp/x <<'EOF'` 가 `<<EOF` 를 고친다고
    /// 막혔다), `moai mv t-r done < /dev/null` 의 갈 칸이 가려진다.
    fn read_from(&mut self) {
        self.drop_fd();
        self.flush();
        match self.chars.peek() {
            Some('<') => {
                self.chars.next();
                if self.chars.next_if_eq(&'<').is_some() {
                    self.aim = Aim::Here;
                } else {
                    self.heredoc();
                }
            }
            Some('&') => {
                self.chars.next();
                self.aim = Aim::Read;
            }
            Some('>') => {
                self.chars.next();
                self.aim = Aim::Write;
            }
            _ => self.aim = Aim::Read,
        }
    }

    /// `>` 를 읽었다.
    fn write_to(&mut self) {
        self.drop_fd();
        self.flush();
        self.chars.next_if_eq(&'>');
        if self.chars.next_if_eq(&'&').is_some() {
            // `2>&1`·`>&-` 는 fd 를 잇고, `>& 파일` 은 그 파일에 쓴다.
            self.aim = Aim::Dup;
        } else {
            // `>|` 는 noclobber 를 넘는 쓰기다. 갈래로 읽으면 안 된다.
            self.chars.next_if_eq(&'|');
            self.aim = Aim::Write;
        }
    }

    /// `2>` 의 `2` 는 낱말이 아니라 fd 다.
    fn drop_fd(&mut self) {
        if !self.had && !self.cur.is_empty() && self.cur.chars().all(|d| d.is_ascii_digit()) {
            self.cur.clear();
        }
    }

    /// `<<` 뒤의 종료어를 읽어, 줄이 끝나면 건너뛸 본문으로 적는다. 따옴표는
    /// 걷는다 — `<<'MD'`·`<<"MD"`·`<<\MD` 의 종료어는 모두 `MD` 다.
    fn heredoc(&mut self) {
        let strip = self.chars.next_if_eq(&'-').is_some();
        while self.chars.next_if(|d| *d == ' ' || *d == '\t').is_some() {}
        let mut tag = String::new();
        // **종료어에 따옴표가 있었나** — 없으면 셸이 본문의 `$(…)`·백틱을 푼다(moai-t863).
        let mut quoted = false;
        while let Some(&d) = self.chars.peek() {
            match d {
                '\'' | '"' => {
                    quoted = true;
                    self.chars.next();
                    for e in self.chars.by_ref() {
                        if e == d {
                            break;
                        }
                        tag.push(e);
                    }
                }
                '\\' => {
                    quoted = true;
                    self.chars.next();
                    if let Some(e) = self.chars.next() {
                        tag.push(e);
                    }
                }
                d if d.is_whitespace() || matches!(d, ';' | '|' | '&' | '<' | '>' | '(' | ')') => break,
                d => {
                    self.chars.next();
                    tag.push(d);
                }
            }
        }
        if !tag.is_empty() {
            let n = self.docs.len();
            self.docs.push(String::new());
            self.seg.docs.push(n);
            self.heredocs.push((tag, strip, n, quoted));
        }
    }

    /// 줄이 끝났다 — 적어 둔 heredoc 본문을 차례로 건너뛴다.
    ///
    /// **종료어는 그 줄 전체여야 한다** (`<<-` 는 앞 탭만 걷는다). 셸이 그렇게
    /// 읽는다. 앞뒤를 다듬어 견주던 판은 본문에 인용된 `    MD` 에서 본문을
    /// 끝내, 남은 본문을 명령으로 읽었다.
    fn skip_heredocs(&mut self) {
        for (tag, strip, n, quoted) in std::mem::take(&mut self.heredocs) {
            loop {
                let mut line = String::new();
                let mut more = false;
                for d in self.chars.by_ref() {
                    if d == '\n' {
                        more = true;
                        break;
                    }
                    line.push(d);
                }
                let body = if strip { line.trim_start_matches('\t') } else { line.as_str() };
                if body.strip_suffix('\r').unwrap_or(body) == tag || (!more && body.is_empty()) {
                    break;
                }
                // 본문은 명령이 아니지만 그 토막에 흘러드는 글이다([`Seg::fed`]).
                let doc = &mut self.docs[n];
                doc.push_str(body);
                doc.push('\n');
                if !more {
                    break;
                }
            }
            // **따옴표 없는 종료어면 본문의 치환은 명령이다**(moai-t863) — 셸이 그것을 돌려 값을
            // 본문에 끼운다. `cat <<EOF` 안의 `$(moai add x)` 가 규칙 1 을 그냥 지나가던 자리다.
            //
            // **본문을 다 모은 뒤에 한 번 본다.** 줄마다 보던 판은 줄을 넘는 `$( … )` 를 줄
            // 끝에서 잘라, 이어지는 줄의 `moai add` 를 놓쳤다(리뷰 moai-p836.rv).
            if !quoted {
                let found = substitutions(&self.docs[n]);
                self.later.extend(found.into_iter().map(|t| (n, t)));
            }
        }
    }

    fn at_word_start(&self) -> bool {
        self.cur.is_empty() && !self.had
    }

    fn flush(&mut self) {
        if !self.had && self.cur.is_empty() {
            return;
        }
        let word = std::mem::take(&mut self.cur);
        let quoted = std::mem::take(&mut self.had);
        match std::mem::take(&mut self.aim) {
            Aim::Write => self.seg.writes.push(word),
            Aim::Read => {}
            Aim::Here => self.seg.fed.push(word),
            Aim::Dup if word.chars().all(|d| d.is_ascii_digit() || d == '-') => {}
            Aim::Dup => self.seg.writes.push(word),
            Aim::Word => {
                // 명령 자리의 `{`·`if`… 만 묶음을 연다 — `echo {`·`echo if` 는 글자다.
                //
                // **닫는 `}`·`fi`… 는 `)` 처럼 묶음을 닫을 뿐 낱말이 아니다.** 낱말로 남으면 제 토막을
                // 세워 안쪽 `;` 를 제 이음사로 들고 서서 — `{ mv; } && sed -i …` 의 집기를 끊고 —
                // 다음 `}` 가 명령 자리를 못 봐 `{ { a; } }` 의 깊이가 하나 남는다.
                if !quoted && OPENS.contains(&word.as_str()) && command_of(&self.seg.words).is_empty() {
                    self.braces += 1;
                } else if !quoted && SHUTS.contains(&word.as_str()) && self.seg.words.is_empty() && self.braces > 0 {
                    self.braces -= 1;
                    self.sank();
                    if word != "}" {
                        self.shut = Some(self.shut.map_or(self.level(), |s| s.min(self.level())));
                    }
                    return;
                }
                if !quoted && word == "[[" && command_of(&self.seg.words).is_empty() {
                    self.test = true;
                } else if !quoted && word == "]]" {
                    self.test = false;
                }
                self.seg.words.push(word);
            }
        }
    }

    /// 토막을 닫고, 읽은 이음사를 다음 토막 몫으로 적는다 — `None` 은 이음사가 아닌 것(괄호).
    ///
    /// **빈 토막을 닫는 이음사는 앞의 것을 덮지 않을 때가 있다.** `a &&\n b` 의 줄바꿈과
    /// `a && ( b )` 의 괄호는 `&&` 를 그대로 둔다. `( a ) ; b` 의 `;` 는 빈 토막에 오지만
    /// 이음사다 — 덮는다.
    fn end_by(&mut self, op: Option<Op>) {
        self.flush();
        self.aim = Aim::Word;
        self.test = false;
        // **치환은 제 토막보다 먼저 돈다** — 그 앞에, 한 겹 깊은 하위 셸로 쌓는다(moai-xe6e). 깊이를
        // 더해야 치환 안의 `cd`·`;` 가 바깥 토막의 자리와 집기를 안 흔든다. 첫 토막은 바깥 토막의
        // 이음사를 받는다 — `mv && echo "$(sed -i …)"` 의 치환도 집기가 이겨야 돈다.
        //
        // **낱말 없는 토막의 치환도 쌓는다.** `…; done < <(moai add x)` 의 토막은 읽는 리다이렉션뿐이라
        // 안 쌓이는데, 치환까지 그 자리에서 버리던 판은 그 안의 명령을 아무 규칙에도 안 보였다.
        let (group, level) = (self.group, self.level());
        let inner = std::mem::take(&mut self.inner);
        let opened = !inner.is_empty();
        // 바깥 토막의 이음사는 **처음 쌓는 토막**이 받는다 — 첫 치환이 아무것도 안 내면(`$(<f)`·`$()`)
        // 다음 치환의 첫 토막이다. 첫 치환에 매던 판은 그때 바깥의 `;` 를 잃었다.
        let mut first = true;
        // **다시 읽기의 겹은 치환에도 든다**([`Lexer::DEEP`]) — `Lexer::new` 로 0 부터 다시 세던 판은
        // `$(bash -c "$( … )")` 가 겹마다 셈을 되돌려, 상한이 아무것도 막지 못했다(리뷰 moai-p836.rv).
        let deep = self.deep + 1;
        for text in inner {
            if deep > Lexer::DEEP {
                break;
            }
            // 치환마다 바깥 자리에서 새 하위 셸을 연다 — 앞 치환의 셸과 깊이는 같아도 딴 셸이다.
            self.low = self.low.min(group);
            self.floor = self.floor.min(level);
            for (m, mut s) in Lexer::at(&text, deep).run().into_iter().enumerate() {
                if m > 0 {
                    // 치환 안에서 지나온 자리는 안쪽이 잰 그대로다.
                    self.low = s.low + group + 1;
                    self.floor = s.floor + level + 1;
                }
                s.depth += group + 1;
                s.level += level + 1;
                s.shut = s.shut.map(|l| l + level + 1);
                s.join = if std::mem::take(&mut first) {
                    self.join
                } else {
                    Join { op: s.join.op, depth: s.join.depth + level + 1 }
                };
                for l in &mut s.nested {
                    *l = l.shift(level + 1, group + 1);
                }
                s.nested.insert(0, Layer::Subst(level + 1));
                self.push(s);
            }
        }
        if opened {
            // 치환을 나와 바깥 자리로 돌아왔다.
            self.low = self.low.min(group);
            self.floor = self.floor.min(level);
        }
        // 빈 토막은 쌓지 않는다 — 어차피 걸러지고, 그 표식(`a |\n b` 의 `sub`)은 다음 토막의 것이다.
        if self.seg.words.is_empty() && self.seg.writes.is_empty() {
            if let Some(op) = op {
                self.join = Join { op, depth: self.level() };
            }
            return;
        }
        let mut seg = std::mem::take(&mut self.seg);
        seg.depth = group;
        seg.level = level;
        seg.join = self.join;
        // **접두어만 선 토막은 이음사를 덮지 않는다**(moai-gtkn). `a && time ( b )` 의 `(` 는
        // `time` 토막을 닫는데, 그때 `&&` 를 `Any` 로 덮던 판은 괄호 안의 `b` 를 앞이 져도 도는
        // 것으로 읽어 `mv && time (sed -i …)` 를 막았다. `! (`·`if (` 도 같은 자리다.
        if let Some(op) = op {
            self.join = Join { op, depth: level };
        } else if !command_of(&seg.words).is_empty() {
            self.join = Join { op: Op::default(), depth: level };
        }
        self.push(seg);
    }

    /// 지금 몇 겹의 묶음 안인가([`Seg::level`]).
    fn level(&self) -> usize {
        self.group + self.braces
    }
}

/// 묶음을 여는 낱말 — `{` 와 예약어. **명령 자리에 섰을 때만이다**(`echo if` 의 `if` 는 글자다).
///
/// **예약어 묶음도 `{ … }` 처럼 센다**(moai-1jvy). `if …; then …; fi` 의 몸통 `;` 를 묶음 밖의
/// 끊김으로 읽던 판은 `mv && if true; then sed -i …; fi` 를 막았다 — 잘못 막는 쪽이라 새지는 않지만,
/// 훅이 안 되는 모양을 늘리면 사람은 규칙을 지키는 법이 아니라 피하는 법부터 배운다.
///
/// 여는 쪽만 센다. `for`·`select`·`while`·`until` 은 `do … done` 으로 닫히는데 `do` 는
/// [`PREFIXES`] 라 명령 자리를 안 옮기므로, 여는 낱말 하나와 `done` 하나로 짝이 맞는다.
const OPENS: &[&str] = &["{", "if", "while", "until", "for", "select", "case"];

/// 묶음을 닫는 낱말. **`case` 의 갈래 `)` 는 여기서 안 센다** — 괄호 깊이는
/// `saturating_sub` 로 0 에 머무르고, 갈래의 몸통은 `esac` 까지 이 묶음 안이다.
const SHUTS: &[&str] = &["}", "fi", "done", "esac"];

/// 명령 자리 앞에 설 수 있는 것 — 예약어와, 뒤의 명령을 그대로 부르는 것.
///
/// 여기서 멈추면 `{ cd /tmp; … }` 의 `cd` 를 못 봐 뒤의 상대 경로를 저장소에
/// 풀고(아무것도 안 쓰는 명령이 막혔다), `time moai mv <리뷰> done` 의 `moai`
/// 를 못 봐 닫기 규칙이 샜다.
const PREFIXES: &[&str] = &[
    "!", "{", "if", "then", "elif", "else", "do", "while", "until", "time", "builtin", "command",
    "exec", "nohup",
];

/// 토막에서 **명령 자리**부터의 낱말들. 앞에 붙은 환경변수 대입(`FOO=1 cmd`)과
/// 예약어·접두 명령을 지나친다. 셋이 따로 세던 것을 여기 하나로 모았다 —
/// 서로 다른 셈이 이미 서로 다른 답을 내고 있었다.
///
/// **감싸는 명령도 넘는다**(moai-455j) — `env`·`timeout`·`nice`·`stdbuf`·`sudo`·`doas` 는 뒤에 오는
/// 명령을 그대로 돌린다. 못 넘던 판은 `env moai add x`·`timeout 5 moai add x`·`sudo tee src/x.rs`
/// 를 아무 규칙에도 안 보였다 — 규칙 1~3 이 낱말 하나로 샜다.
///
/// **옵션이 값을 먹는지 알아야 한다** — `env -u NAME moai add` 의 `NAME` 을 명령으로 읽으면 그 줄은
/// 다시 아무것도 아니게 된다. 모르는 옵션을 만나면 **거기서 멈춘다**: 넘겨짚어 명령 자리를 옮기면
/// 그 줄이 엉뚱한 명령으로 읽혀, 새는 것보다 나쁜 잘못 막음이 난다.
///
/// **셸을 여는 것은 안 넘는다** — `sudo -s`·`env -S`·`bash -c '…'` 의 뒤는 명령이 아니라 글이다.
/// 그 글을 읽는 것은 렉서의 일이다([`Lexer::relex`]).
///
/// **감싸는 명령이 붙박이를 돌리는 일은 없다**([`BUILTINS`]) — `sudo cd /tmp`·`env set -e`·
/// `sudo exit 1` 은 그런 이름의 프로그램이 없어 그냥 진다. 넘어가서 그것을 `cd`·`set`·`exit` 로
/// 읽던 판은 자리를 옮긴 것으로·errexit 를 켠 것으로·껍데기를 끝낸 것으로 세, 세 자리에서
/// 한꺼번에 샜다(리뷰 moai-p836.rv).
fn command_of(words: &[String]) -> &[String] {
    let mut at = 0;
    loop {
        let rest = &words[at..];
        let lead = rest
            .iter()
            .take_while(|w| PREFIXES.contains(&w.as_str()) || (w.contains('=') && !w.starts_with(['-', '='])))
            .count();
        at += lead;
        let Some(head) = words.get(at).map(|w| basename(w)) else { return &words[at..] };
        let Some(Wrapped::Runs(skip)) = wrapped(head, &words[at + 1..]) else { return &words[at..] };
        let next = at + 1 + skip;
        // 감싸는 명령 뒤가 붙박이면 그 줄은 그냥 진다 — 넘지 않는다([`BUILTINS`]).
        if words.get(next).map(|w| basename(w)).is_some_and(|h| BUILTINS.contains(&h)) {
            return &words[at..];
        }
        at = next;
    }
}

/// 껍데기의 **붙박이와 예약어** — `env`·`sudo` 같은 감싸는 명령이 못 돌린다. 그런 이름의
/// 프로그램이 없어 `sudo cd /tmp` 는 자리를 안 옮기고 `env set -e` 는 errexit 를 안 켠다.
///
/// 이름이 겹쳐도 **프로그램이 따로 있는 것은 안 든다** — `echo`·`printf`·`test`·`kill`·`pwd`·
/// `true`·`false`·`time`·`nohup` 은 `/usr/bin` 에 있어 감싸는 명령이 그대로 돌린다.
const BUILTINS: &[&str] = &[
    "cd", "pushd", "popd", "set", "exit", "eval", "source", ".", "export", "unset", "shift",
    "return", "exec", "builtin", "command", "trap", "local", "declare", "typeset", "readonly",
    "alias", "unalias", "shopt", "ulimit", "umask", "read", "wait", "jobs", "fg", "bg", "let",
    "if", "then", "else", "elif", "fi", "while", "until", "for", "select", "do", "done", "case",
    "esac", "function", "in", "{", "}", "!", "[[", "]]", "coproc",
];

/// [`wrapped`] 가 감싸는 명령 하나를 읽은 결과 — 명령 자리가 어디인가, 아니면 왜 못 가는가.
enum Wrapped {
    /// 뒤 낱말 이만큼을 넘으면 **명령 자리**다.
    Runs(usize),
    /// 뒤에 오는 것이 명령이 아니라 **셸에 넘기는 글**이다(moai-drli) — `env -S` 와 `sudo -s`.
    /// 붙여 온 값이 있으면 그 글이 먼저고, 뒤 낱말들이 그 뒤에 이어 붙는다.
    Hands {
        /// 뒤 낱말 이만큼을 넘은 자리부터가 글이다.
        at: usize,
        /// 낱말 안에 붙어 온 글 — `env -SCMD`·`env --split-string=CMD`.
        glued: Option<String>,
        /// 뒤엣것이 **껍데기가 이미 가른 낱말**인가(`sudo -s 명령 …`), 아니면 안 갈린 원문
        /// 한 낱말인가(`env -S '글'`). 갈린 낱말은 도로 감싸서 이어야 바깥 껍데기가 벗긴
        /// 따옴표가 되살아난다 — sudo 가 argv 를 셸에 넘길 때 하는 일이 그 escape 다.
        words: bool,
    },
    /// 여기서 멈춘다 — 아무것도 안 돌리거나(`sudo -l`) 딴 자리에서 돌린다(`env -C`).
    Stops,
}

/// 감싸는 명령 하나를 읽는다 — 그 이름을 모르면 `None` 이고, 알면 [`Wrapped`] 로 답한다
/// ([`command_of`]·[`shell_text`]).
///
/// 옵션 꼴은 GNU coreutils 와 sudo 의 것이다. `--long=값` 은 한 낱말이고, 값을 따로 받는 짧은 옵션만
/// 하나를 더 먹는다. `--` 뒤는 곧 명령이다.
fn wrapped(head: &str, rest: &[String]) -> Option<Wrapped> {
    /// 감싸는 명령 하나를 아는 만큼.
    ///
    /// 위치로만 갈리던 다섯 자리 튜플을 이름으로 바꿨다 — `takes`·`long`·`stops` 는 셋 다
    /// `&[&str]` 이라 자리를 바꿔 적어도 컴파일이 되고, 그러면 명령 자리가 조용히 어긋난다.
    struct Wrapper {
        name: &'static str,
        /// 값을 **따로** 받는 짧은 옵션.
        takes: &'static [&'static str],
        /// 값을 따로 받는 긴 옵션.
        long: &'static [&'static str],
        /// 값을 **안** 받는 긴 옵션 — 모르는 것은 아래에서 멈춘다. 프로그램마다 따로 든다:
        /// 한 통에 모으면 `timeout --set-home` 처럼 그 프로그램에 없는 이름이 통과한다.
        free: &'static [&'static str],
        /// 값을 **붙여서만** 받는 짧은 옵션(`sudo -hHOST`) — 혼자 서면 값이 없다. 그 뒤의 글자는
        /// 스위치가 아니라 값이라, 거기서 낱말이 끝난다.
        attach: &'static [&'static str],
        /// 옵션 뒤에 오는 제 자리 인자 수(`timeout 5 …`).
        args: usize,
        /// **여기서 멈추는 스위치** — 둘 중 하나다. 뒤의 명령을 아예 안 돌리거나(`sudo -l`·`-v`·
        /// `doas -s`·`-L`·`-C`), 그 명령을 **딴 자리에서** 돌린다(`env -C DIR`·`sudo -D DIR`·
        /// `sudo -i`).
        /// 뒤엣것은 [`aimed`] 와 `moved` 가 그 자리를 모르는데, 넘겨 주면 남의 트래커에 선 집기가
        /// 여기 규칙 2 를 채우고 남의 트래커에 세우는 줄이 여기 규칙 1 에 막힌다(리뷰
        /// moai-p836.rv). 모르는 자리는 지어내지 않는다.
        stops: &'static [&'static str],
        /// **그 뒤가 셸에 넘기는 글인 스위치**(moai-drli) — `env -S` 와 `sudo -s`.
        /// 한때 `stops` 에 함께 있었는데, 멈추는 까닭이 "그 뒤는 명령이 아니라 글이고 그 글을 읽는
        /// 것은 렉서의 일" 이면서 정작 렉서([`shell_text`])는 `bash -c` 와 `eval` 만 알아, 그 글을
        /// 읽는 것이 **아무도 없었다** — `env -S "moai add x"` 와 `sudo -s moai add x` 가 규칙을
        /// 통째로 지나갔다. `bash -c` 와 같은 표의 줄로 둔다.
        ///
        /// `env -S` 는 사실 셸이 아니라 낱말로 가를 뿐이라, 그 글의 `&&`·`|`·`>` 는 낱말로 남는다 —
        /// 그래서 [`shell_text`] 가 그것을 [`split_string`] 으로 가른다. 한때 셸의 글로 읽어
        /// **더 많이 보는** 쪽으로 어림잡았는데, 그 셈이 **집기 축에서는 거꾸로** 선다: 지어낸
        /// 집기는 규칙 2 를 채워 빈손의 쓰기를 풀어 준다(`env -S 'true && moai mv <id> in_progress'
        /// && 쓰기`). 더 보는 것이 늘 안전한 것은 막는 축뿐이다.
        hands: &'static [&'static str],
        /// `hands` 스위치가 **값을 받는가** — `env -S` 는 그 자리에서 글을 받아(`-SCMD`·
        /// `--split-string=CMD`·뒤 낱말) 옵션 읽기가 거기서 끝나고, `sudo -s` 는 값 없는 깃발이라
        /// 옵션 읽기가 **계속된다**. 받는 것을 안 받는 것으로 적으면 뭉치의 남은 글자(`sudo -si` 의
        /// `i`)가 글의 첫 낱말이 되고, 거꾸로 적으면 그 뒤의 옵션이 글의 첫 낱말이 된다 —
        /// `sudo -s -u 남 moai add x` 와 `sudo -su 남 moai add x` 의 글은 `moai add x` 지 `-u 남 …`
        /// 이 아니다(둘 다 규칙이 통째로 샜다).
        glued: bool,
    }
    const COMMON: &[&str] = &["--debug", "--verbose", "--version", "--help"];
    const WRAPPERS: &[Wrapper] = &[
        Wrapper {
            name: "env",
            takes: &["-u"],
            long: &["--unset"],
            // `--block-signal[=SIG]` 셋은 값을 **붙여서만** 받는다 — 다음 낱말을 먹는 것으로 적던
            // 판은 `env --block-signal moai add x` 의 `moai` 를 값으로 삼켜 규칙 1 을 놓쳤다.
            free: &["--ignore-environment", "--null", "--block-signal", "--default-signal", "--ignore-signal", "--list-signal-handling"],
            attach: &[],
            args: 0,
            stops: &["-C", "--chdir"],
            hands: &["-S", "--split-string"],
            glued: true,
        },
        Wrapper {
            name: "timeout",
            takes: &["-k", "-s"],
            long: &["--kill-after", "--signal"],
            free: &["--preserve-status", "--foreground"],
            attach: &[],
            args: 1,
            stops: &[],
            hands: &[],
            glued: false,
        },
        Wrapper {
            name: "nice",
            takes: &["-n"],
            long: &["--adjustment"],
            free: &[],
            attach: &[],
            args: 0,
            stops: &[],
            hands: &[],
            glued: false,
        },
        Wrapper {
            name: "stdbuf",
            takes: &["-i", "-o", "-e"],
            long: &["--input", "--output", "--error"],
            free: &[],
            attach: &[],
            args: 0,
            stops: &[],
            hands: &[],
            glued: false,
        },
        Wrapper {
            name: "sudo",
            // `-h` 는 `-hHOST` 로 붙여서만 값을 받고 혼자 서면 도움말이다 — 값을 먹는 것으로 적던
            // 판은 `sudo -h moai add x` 를 `add` 라는 명령으로 읽었다.
            takes: &["-u", "-g", "-p", "-C", "-U", "-r", "-t", "-T", "-R", "-a", "-c"],
            long: &["--user", "--group", "--prompt", "--host", "--other-user", "--role", "--type", "--command-timeout", "--chroot", "--close-from", "--auth-type", "--login-class"],
            free: &["--non-interactive", "--preserve-env", "--set-home", "--stdin", "--background", "--remove-timestamp", "--reset-timestamp", "--askpass", "--bell", "--preserve-groups"],
            attach: &["-h"],
            args: 0,
            // `-l`·`-v` 는 뒤의 명령을 **안 돌린다**(될지만 본다) — 넘기면 안 도는 줄을 막는다.
            // **`-i`·`--login` 도 여기다**(2026-09-20 사용자 결정, 리뷰 moai-jlon.yeg 5번) — 로그인
            // 셸은 대상 사용자의 홈으로 옮겨 가 **딴 자리에서** 돈다. 2026-09-19 결정은 이것을
            // `-s` 와 한 줄로 묶었는데, 같은 결정이 "딴 자리에서 돌리는 것은 멈춘다" 도 함께
            // 세웠다 — 목록과 잣대가 어긋났고 잣대를 따랐다. 넘겨 주면 ~root 의 딴 트래커에서
            // 돌거나 아예 실패하는 집기가 여기 규칙 2 를 채운다(`env -C`·`sudo -D` 와 같은 자리).
            stops: &["-i", "--login", "-e", "--edit", "-l", "--list", "-v", "--validate", "-D", "--chdir"],
            // `sudo -s <명령>` 은 그 낱말들을 이어 붙여 셸에 `-c` 로 넘긴다 — 자리는 그대로다.
            hands: &["-s", "--shell"],
            glued: false,
        },
        // `doas -C <설정>` 은 규칙을 시험해 보고 찍기만 한다 — 뒤의 명령을 안 돌린다.
        // **`-s` 도 같다**(2026-09-20 사용자 결정, 리뷰 moai-jlon.yeg 6번) — OpenBSD doas 는 `-s` 에
        // argv 를 `$SHELL` 로 **갈아치운다**. 뒤에 적은 명령은 넘어가는 것이 아니라 사라진다.
        // sudo 의 `-s` 와 철자가 같아 한 줄로 묶었던 자리고, 넘겨 주면 돌지도 않는 집기가 규칙 2 를
        // 채우고 일어날 수 없는 쓰기를 규칙 2 가 막았다.
        Wrapper {
            name: "doas",
            takes: &["-u", "-a"],
            long: &[],
            free: &[],
            attach: &[],
            args: 0,
            stops: &["-s", "-L", "-C"],
            hands: &[],
            glued: false,
        },
    ];
    let w = WRAPPERS.iter().find(|w| w.name == head)?;
    let (takes, long, stops, hands) = (w.takes, w.long, w.stops, w.hands);
    // `-c` 같은 글자 하나를 `format!` 없이 견준다 — 훅은 Bash 한 번마다 돈다.
    let letter = |set: &[&str], c: char| c.is_ascii() && set.iter().any(|f| f.as_bytes() == [b'-', c as u8]);
    // 값이 모자라 셸이 거절할 줄 — 넘겨짚지 않는다([`command_of`] 가 여기서 멈춘다).
    macro_rules! need {
        ($e:expr) => {
            if $e.is_none() {
                return Some(Wrapped::Stops);
            }
        };
    }
    let mut n = 0;
    let mut seen = 0;
    // **값 없는 글 스위치를 이미 봤는가** — `sudo -s` 는 깃발이라 옵션 읽기가 거기서
    // 안 끝난다(`glued` 가 거짓인 줄). 곧장 그 뒤부터를 글로 읽던 판은 `sudo -s -u 남 moai add x` 와
    // `sudo -su 남 moai add x` 의 글을 `-u 남 moai add x` 로 잡아 그 글의 명령 자리가 `-u` 가 됐고,
    // 규칙 1~2 가 이 스위치를 도로 통째로 지나갔다. 옵션이 다 끝난 자리가 곧 글이다.
    let mut handed = false;
    while let Some(word) = rest.get(n) {
        if word == "--" {
            n += 1;
            // **`--` 는 옵션만 끝낸다** — 제 자리 인자는 그 뒤에 온다. 곧장 나가던 판은
            // `timeout -- 5 moai add x` 의 `5` 를 명령으로 읽었다.
            while seen < w.args {
                need!(rest.get(n));
                seen += 1;
                n += 1;
            }
            break;
        }
        // `env -` 는 `-i` 와 같다(환경을 비운다) — 자리 인자로 읽던 판은 `-` 를 명령으로 읽었다.
        if word == "-" && head == "env" {
            n += 1;
            continue;
        }
        if !word.starts_with('-') || word == "-" {
            // 제 자리 인자(`timeout 5 …`)를 다 먹었으면 여기가 명령 자리다.
            if seen == w.args {
                break;
            }
            seen += 1;
            n += 1;
            continue;
        }
        // `--chdir=DIR` 처럼 값을 붙여 온 것도 같은 스위치다.
        if stops.iter().any(|f| word == f || word.strip_prefix(f).is_some_and(|r| r.starts_with('='))) {
            return Some(Wrapped::Stops);
        }
        // **셸에 넘기는 글은 여기서부터다**(moai-drli). `--split-string=글` 은 그 낱말 안에 글이 있다.
        // 값을 받는 스위치(`env -S`)는 여기서 옵션이 끝나고, 값 없는 깃발(`sudo -s`)은 표만 달고
        // 옵션을 마저 읽는다 — 글은 옵션이 다 끝난 자리다.
        if let Some(glued) = hands.iter().find_map(|f| {
            (word == f)
                .then_some(None)
                .or_else(|| word.strip_prefix(f).filter(|r| r.starts_with('=')).map(|r| Some(r[1..].to_string())))
        }) {
            if w.glued {
                return Some(Wrapped::Hands { at: n + 1, glued, words: false });
            }
            // 값 없는 깃발에 `=` 를 단 꼴(`sudo -s=x`)은 그 프로그램이 거절한다 — 붙은 것을 글로
            // 읽지 않는다. 표만 달고 옵션을 마저 읽는다.
            handed = true;
            n += 1;
            continue;
        }
        if takes.contains(&word.as_str()) {
            // 값이 없으면(`env -u`) 그 줄은 셸이 거절한다 — 넘겨짚지 않는다.
            need!(rest.get(n + 1));
            n += 2;
            continue;
        }
        // `--이름=값` 은 값을 달고 있어 한 낱말이다 — 모르는 이름이어도 명령 자리를 안 흔든다.
        if word.starts_with("--") && word.contains('=') {
            n += 1;
            continue;
        }
        if long.contains(&word.as_str()) {
            need!(rest.get(n + 1));
            n += 2;
            continue;
        }
        // **모르는 긴 옵션에서는 멈춘다** — 값을 따로 받는 것이면 그 값을 명령으로 읽는다.
        // `nice --10` 처럼 숫자만 붙은 것은 값을 안 받는다.
        if let Some(name) = word.strip_prefix("--") {
            if !w.free.contains(&word.as_str())
                && !COMMON.contains(&word.as_str())
                && !name.chars().all(|c| c.is_ascii_digit())
            {
                return Some(Wrapped::Stops);
            }
            n += 1;
            continue;
        }
        // 짧은 스위치 뭉치 — 글자마다 본다. 값을 받는 글자를 만나면 그 뒤가 붙은 값이거나
        // 다음 낱말이다(`-o0`·`-u NAME`·`-iu NAME`). 맨 앞 글자만 보던 판은 `sudo -nu bob moai add x`
        // 와 `env -iu FOO moai add x` 에서 멈춰, 규칙을 통째로 껐다.
        let mut eats = false;
        let mut stop = true;
        let bytes = word.as_bytes();
        for (at, c) in word.char_indices().skip(1) {
            if letter(stops, c) {
                return Some(Wrapped::Stops);
            }
            // **뭉치 안의 글 스위치**(moai-drli) — `env -iS '글'`·`sudo -ns 명령`. 값을 받는 것
            // (`env -S`)은 남은 글자가 곧 글이라 옵션이 여기서 끝나고, 값 없는 깃발(`sudo -s`)은
            // 뭉치의 남은 글자도 스위치다 — `sudo -su 남 moai add x` 의 `u` 가 그렇다.
            if letter(hands, c) {
                if w.glued {
                    let left = &word[at + c.len_utf8()..];
                    return Some(Wrapped::Hands { at: n + 1, glued: (!left.is_empty()).then(|| left.to_string()), words: false });
                }
                handed = true;
                stop = false;
                continue;
            }
            if letter(takes, c) {
                // 뒤에 붙은 것이 값이고, 없으면 다음 낱말이다.
                eats = at + c.len_utf8() == bytes.len();
                stop = false;
                break;
            }
            // 붙여서만 값을 받는 글자 — 남은 글자는 스위치가 아니라 그 값이다(`sudo -hHOST`).
            if letter(w.attach, c) {
                stop = false;
                break;
            }
            // 값을 안 받는 글자만 묶였으면 넘긴다 — `nice -5` 도 여기다.
            if !(c.is_ascii_alphanumeric() || c == '.') {
                stop = true;
                break;
            }
            stop = false;
        }
        if stop {
            return Some(Wrapped::Stops);
        }
        if eats {
            need!(rest.get(n + 1));
        }
        n += 1 + usize::from(eats);
    }
    // 값 없는 글 스위치를 봤으면 **옵션이 끝난 이 자리부터가 글이다**. 뒤가 비어도 괜찮다 —
    // `sudo -s` 혼자는 사람이 쓸 셸을 띄우고, [`shell_text`] 가 빈 글을 안 읽는다.
    if handed {
        return Some(Wrapped::Hands { at: n, glued: None, words: true });
    }
    // 뒤에 명령이 없으면 감싸는 것이 아니다(`env` 혼자는 환경을 찍는다).
    need!(rest.get(n));
    Some(Wrapped::Runs(n))
}

fn basename(word: &str) -> &str {
    word.rsplit(['/', '\\']).next().unwrap_or(word)
}

/// 이 토막이 `moai` 를 부른다면, 그 뒤의 인자들.
///
/// **명령 자리에 있어야 한다.** 어디에 있든 `moai` 라는 낱말을 찾던 판은
/// `echo moai add hello` 를 생성으로 보아 막았고, 무엇보다 리뷰 글을 담은
/// heredoc 을 막았다 — 규칙이 제가 시킨 일을 막는 자리가 또 나온 것이다.
fn moai_args(seg: &[String]) -> Option<&[String]> {
    let (head, rest) = command_of(seg).split_first()?;
    (basename(head) == "moai").then_some(rest)
}

/// 값을 받는 플래그 — 그 값은 자리 인자가 아니다. 전역 플래그(`-C 경로`·
/// `--user "이름"`·`--color 어떻게`)와 `mv` 의 `-m 글`·`--from 칸`.
///
/// **`mv` 에 값 받는 플래그를 더하면 여기도 더한다.** 빠뜨리면 그 값이 자리
/// 인자로 남아 **맨 끝이 갈 칸**이라는 셈이 통째로 어긋난다 — `moai mv <id>
/// in_progress --from todo` 는 `todo` 로 옮기는 것으로 읽혀 [`picks_up`] 이
/// 집기를 못 보고, `moai mv <리뷰> done --from review` 는 갈 칸이 `done` 이
/// 아닌 것으로 읽혀 리뷰 닫기 규칙이 통째로 샌다.
const TAKES_VALUE: &[&str] = &["-C", "--dir", "--user", "--color", "-m", "--msg", "--from"];

/// `moai` 뒤의 **자리 인자들** — 플래그와 그 값을 걷은 것.
///
/// 첫 플래그에서 멈추던 판은 `moai --json mv t-r done` 을 동사 없는 명령으로,
/// `moai mv t-r --json done` 을 `t-r` 로 옮기는 명령으로 읽었다 — 전역 플래그
/// 하나로 규칙 1 과 리뷰 닫기가 통째로 샜다. clap 은 플래그를 어디에 두든 받는다.
fn positionals(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.extend(it.map(String::as_str));
            break;
        }
        if a.starts_with('-') && a.len() > 1 {
            if TAKES_VALUE.contains(&a.as_str()) {
                it.next();
            }
            continue;
        }
        out.push(a.as_str());
    }
    out
}

/// 이 토막이 **일을 새로 세우는가.**
///
/// `moai add` 와 `moai issue add` 는 같은 일이다. 앞의 것만 보던 판은 뒤의
/// 것을 그냥 보냈다 — 같은 연산의 두 철자가 다르게 움직이면, 규칙을 아는
/// 쪽은 그것을 우회로로 쓰고 모르는 쪽은 왜 한 번은 막히고 한 번은 안
/// 막히는지 모른다. `idea add` 는 여기서도 자유롭다.
///
/// **`add --type idea` 도 `idea add` 와 같은 연산이다.** 앞의 것만 풀어 주던
/// 판은 뒤의 것을 생성으로 읽어 막았다 — 위와 같은 까닭의 반대쪽이다. 종류를
/// 고정한 네임스페이스(`issue add --type idea`)는 고정한 쪽이 이기므로
/// (`add::run` 의 `kind_override.or(args.kind)`) 그대로 생성이다.
///
/// **`idea promote -e <에픽>` 도 생성이다**(moai-f3ml.lm7). 새 에픽을 세우는 promote 는
/// 그 자체로 한 단위라 자유롭지만, `-e` 는 이미 선 에픽에 멤버를 넣는다 — 안 보면
/// `idea add` 뒤에 `promote -e <남의 에픽>` 으로 `add -e <남의 에픽>` 이 막히는 자리를 지나간다.
fn creates(seg: &[String]) -> bool {
    let Some(args) = moai_args(seg) else { return false };
    let verbs = positionals(args);
    match verbs.first().copied() {
        Some("add") => !adds_idea(args, &verbs),
        Some("issue" | "epic" | "milestone") => verbs.get(1).copied() == Some("add"),
        Some("idea") => promotes_into(seg),
        _ => false,
    }
}

/// `idea add` 나 `add --type idea` — **생각을 담는** 두 철자. [`creates`] 는 이것을 풀어 주고
/// [`sets_aside`] 는 이것을 비춘다. 둘이 따로 세던 판은 한쪽에만 철자를 더하면 그 철자가 막히지도
/// 비치지도 않고 지나가는 자리였다 — 한 셈을 둘이 나눠 쓴다.
fn adds_idea(args: &[String], verbs: &[&str]) -> bool {
    match verbs.first().copied() {
        Some("idea") => verbs.get(1).copied() == Some("add"),
        Some("add") => flag_values(args, &["--type"]).last().map(String::as_str) == Some("idea"),
        _ => false,
    }
}

/// 도움말을 부르는가. 도움말은 아무것도 안 만들고 안 옮기고 0 으로 끝난다.
///
/// **`moai` 의 인자만 준다.** 토막 전부를 주던 자리는 감싸는 명령의 `-h` 까지 세, `sudo -h moai
/// add x` 를 도움말로 읽고 규칙 1 을 통째로 넘겼다(리뷰 moai-p836.rv) — `sudo -h` 는 sudo 의
/// 호스트 스위치고, 그 뒤의 `moai add` 는 그대로 만든다.
fn asks_help(words: &[String]) -> bool {
    words.iter().any(|t| t == "-h" || t == "--help")
}

/// 선 에픽에 멤버로 펼치는 `idea promote -e` 인가. `--from` 을 늘 들고 오므로 `--from` 을
/// 한 단위로 읽어 풀어 주는 자리에서 이것만은 빼야 한다.
fn promotes_into(seg: &[String]) -> bool {
    let Some(args) = moai_args(seg) else { return false };
    positionals(args).get(..2) == Some(&["idea", "promote"][..])
        && !flag_values(args, &["-e", "--epic"]).is_empty()
}

/// 지금 집고 있는 것에 매인 단위들 — 그 이슈 자신, 그 에픽, 그 마일스톤, 그 부모.
///
/// **소속은 물려받는다.** `epic` 필드만 읽으면 자식 이슈를 집었을 때 그 줄의
/// `epic` 은 비어 있어, 옳은 에픽을 댄 생성까지 거절당한다 — 시험판이 실제로
/// 그랬고, 이 규칙을 만든 세션이 제 리뷰 결과를 이슈로 적지 못했다.
/// `report::groups` 와 `report::milestones` 가 이미 그 상속을 푼다.
///
/// **두 지도를 [`report::ties`] 한 벌로 받는다**(moai-c4nk) — `milestones` 는 제 안에서
/// `groups` 를 다시 지어, 나란히 부르면 소속 지도가 두 벌 선다. 훅은 도구 호출마다 이 길을
/// 지난다. `report` 쪽의 같은 자리는 moai-3prn 이 이미 걷었다.
pub fn unit_of<'a>(issues: &'a [Issue], focus: &[&'a Issue]) -> BTreeSet<&'a str> {
    let (epics, stones) = report::ties(issues);
    let mut out = BTreeSet::new();
    for i in focus {
        out.insert(i.id.as_str());
        if let Some(e) = epics.get(i.id.as_str()) {
            out.insert(e);
        }
        if let Some(m) = stones.get(i.id.as_str()) {
            out.insert(m);
        }
        if let Some(p) = crate::id::parent_of(&i.id) {
            out.insert(p);
        }
    }
    out
}

/// 이 자리에서 **누구의 것인지** 가르는 이름들 — [`held`] 가 초점에서 뺄 것을 잰다.
///
/// 한때 옆 워크트리의 이름 한 벌(`BTreeSet`)만 받아, 그 이름이 가리키는 줄을 거리와 상관없이 뺐다.
/// 그러면 머지하고 안 치운 `worktree-<에픽>` 이 그 에픽에 뒤이어 집은 멤버를, 그 멤버를 제
/// 이름으로 띄운 워크트리에서도 쥐었다(moai-m62u) — 그 세션의 초점이 비어 규칙 2 에 막혔다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Away {
    /// 옆 워크트리들의 이름 후보(`worktree::away`) — 그 줄 자신, 그 밑의 자식, 그 에픽·마일스톤에
    /// 든 줄을 가리킨다.
    pub names: BTreeSet<String>,
    /// 이 자리 워크트리의 이름 후보. 옆 이름과 **거리를 겨룬다** — 옆 이름이 더 가까이 가리킬 때만
    /// 옆의 것이다([`report::claims_over`]). 같으면 제 것이다.
    pub own: BTreeSet<String>,
    /// 누구의 것인지 모르는 줄([`unsure`]) — 거리와 상관없이 그 밑의 자식까지 뺀다. **풀기만 하는
    /// 판정**에만 싣는다(`cmd/hook.rs` 의 `settle`·`Stop`·접힌 뒤 싣는 것) — 막지도 붙들지도 않기로 한 줄이다.
    pub unsure: BTreeSet<String>,
    /// 이 세션이 마지막으로 **확실히** 집은 줄([`Picks::mine`]) — `unsure` 가 그 조상을 가리켜도 빼지 않는다(리뷰
    /// moai-3k2d.1df). 남이 부모를 집고 이 세션이 그 자식(`--parent` 로 세운 리뷰 줄 따위)을 집으면, 자식까지
    /// 빼던 판은 제 일을 통째로 놓아 `Stop` 이 안 붙들고 접힌 뒤에도 안 실었고, 좁힌 판정이 `-m` 없는 리뷰
    /// 닫기를 넘겼다. `unsure` 와 함께만 싣는다.
    pub picked: BTreeSet<String>,
}

impl Away {
    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.unsure.is_empty()
    }
}

/// 이 자리에서 집고 있는 것 — [`report::wip`] 에서 **옆 워크트리가 쥔 일**을 뺀 것.
///
/// 집기 커밋은 main 에 들어가므로(CLAUDE.md "워크트리"), main 과 거기서 뜬 워크트리의
/// 스냅샷에는 옆 세션들이 집은 줄이 다 벌여 놓은 칸에 서 있다. 그것을 제 초점으로
/// 세던 판은 main 의 `moai add` 를 규칙 1 로 막고, 세션을 닫을 때 남의 워크트리 일을
/// 옮기거나 미루라고 붙들었다(moai-0yrv). 워크트리가 원칙이 되면 그 거절은 상시다.
///
/// 무엇을 빼는지는 [`Away`] 가 정한다 — 여기는 디스크를 읽지 않는다.
pub fn held<'a>(issues: &'a [Issue], cfg: &Config, away: &Away) -> Vec<&'a Issue> {
    let wip = report::wip(issues, cfg);
    if away.is_empty() || wip.is_empty() {
        return wip;
    }
    let theirs = theirs(issues, away);
    wip.into_iter().filter(|i| !theirs(i)).collect()
}

/// 이 줄이 **옆의 일**인가 — 옆 이름이 제 이름보다 가까이 가리키거나, 누구의 것인지 모른다.
/// [`held`] 와 그 초점을 쓰는 규칙이 같은 자로 재야 한다 — 초점에서는
/// 뺀 옆의 리뷰 줄을 규칙 3 이 "집으라" 고 대면, 이미 옆에서 집은 줄이라 시킨 대로 해도
/// 안 풀린다.
///
/// 소속 지도는 **한 벌만** 짓는다([`report::ties`]) — 옆 이름과 모르는 줄을 같은 지도로 잰다. 둘이 제
/// 지도를 따로 짓던 판은 좁힌 판정마다 두 벌을 지었다.
fn theirs<'a>(issues: &'a [Issue], away: &'a Away) -> impl Fn(&Issue) -> bool + 'a {
    let (epics, stones) = if away.is_empty() { Default::default() } else { report::ties(issues) };
    move |i: &Issue| {
        report::claims_over(&epics, &stones, &away.names, &away.own, i)
            || (report::claims(&epics, &stones, &away.unsure, i) && !away.picked.contains(&i.id))
    }
}

/// 세션마다 적어 둔 집기를 이 세션의 눈으로 가른 것(moai-4jsy, 사용자 결정) — 줄마다 **마지막으로
/// 집은** 세션이 이 세션이면 `mine`, 다른 세션이면 `theirs` 다. 다만 **돌았는지 모르는 집기**
/// (moai-hze6)로는 남이 쥔 줄을 안 가져오고, **남의 것도 그렇다**(moai-dbzs) — 표는 누가 적었든 같은
/// 뜻이다([`Picks::fold`]). 적는 것과 읽는 것은 `cmd/hook.rs` 가 하고, 여기는 그 답만 받는다.
///
/// **트래커에 적지 않는다.** 세션은 이슈의 뜻이 아니라 이 기계의 사정이다 — 훅이 제 표(`board`·
/// `warn`)를 두는 임시 디렉터리에 둔다. 훅 밖(사람의 터미널)에서 집은 줄은 아무 데도 안 적혀 전과
/// 같이 판정한다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Picks {
    pub mine: BTreeSet<String>,
    pub theirs: BTreeSet<String>,
}

impl Picks {
    /// 기록이 줄의 **지금 칸보다 이만큼 앞서면** 그 뒤 훅 밖에서 옮긴 줄의 것으로 본다([`Picks::fold`]).
    /// 칸의 때(`status_since`)는 명령이 돈 때라 기록보다 조금 늦고, 락을 기다리면 더 늦는다 — 그 틈을 준다.
    const SLACK_SECS: i64 = 60;

    /// 집기 한 번을 적는 줄 — `때\tid\t초\t?`. `때` 는 차례를 가르는 벽시계 나노초, `초` 는 줄의 칸
    /// 시각과 견줄 `model::now` 의 초다(못 읽었으면 빈 칸). 끝의 `?` 는 **돌았는지 모르는 집기**다
    /// (moai-hze6·moai-dbzs). **어느 자리가 그런가는 `shell_scan` 의 `certain` 하나가 정한다** —
    /// 여기 그 목록을 베껴 적으면 한쪽만 고쳐지는 날 이 글이 거짓이 된다. 지금 그것이 세는 것은
    /// 앞선 이음사(`&&`·`||`, 그리고 `set -e` 아래서 그렇게 읽히는 `;`), 머리가 안 돌 수 있는
    /// 파이프라인의 칸, 조건에 매여 들어선 묶음 안, 함수 몸통이다. 읽는 쪽은
    /// [`Picks::fold`] 하나다 — 꼴이 두 자리에 서면 한쪽만 고쳐지는 날 기록이 통째로 안 읽힌다.
    ///
    /// **칸을 늘리기만 한다.** 옛 바이너리의 `fold` 는 앞 셋만 읽고 나머지를 버려, 새 줄도 그대로 읽는다.
    pub fn line(at: u128, stamp: Option<i64>, id: &str, sure: bool) -> String {
        let maybe = if sure { "" } else { "\t?" };
        match stamp {
            Some(secs) => format!("{at}\t{id}\t{secs}{maybe}\n"),
            None => format!("{at}\t{id}\t{maybe}\n"),
        }
    }

    /// 세션마다 적어 둔 집기(`(세션, 그 세션의 글)`)를 `me` 의 눈으로 가른다 — 줄마다 **마지막으로** 집은
    /// 세션이 이긴다. 이 세션과 다른 세션이 같은 때에 집었으면 모르는 쪽(`theirs`)이다 — 그쪽은 풀기만 한다.
    /// **양쪽 다 `?` 가 안 붙은 줄로만 겨룬다**(moai-hze6·moai-dbzs) — 남이 **확실히** 쥔 줄이 아예 없을
    /// 때만 `?` 줄도 제 것이다.
    /// **판단은 여기 둔다** — `cmd/hook.rs` 는 파일을 읽어 넘길 뿐이다(판단은 `cmd` 에 없다).
    ///
    /// 못 읽는 줄과 **끝이 안 닫힌 줄**은 건너뛴다(리뷰 moai-3k2d.1df) — 적는 중에 읽은 끝토막은
    /// `moai-x.abc` 가 잘린 `moai-x` 처럼 부모 id 일 수 있고, 그러면 남의 집기가 아닌 부모와 그 밑이 통째로
    /// 모르는 줄이 된다.
    ///
    /// **줄이 그 뒤 옮겨졌으면 기록은 끝났다**(리뷰 moai-3k2d.1df) — 기록이 그 줄의 지금 칸이 선 때(`since`,
    /// 초)보다 [`Picks::SLACK_SECS`] 넘게 앞서면 버린다. 집기를 놓는 것(`mv todo`·`done`)도, 훅 밖(사람의
    /// 터미널·`moai tui`)에서 다시 집는 것도 안 적히는데, 버리지 않으면 죽은 세션의 옛 기록이 그 줄을 언제까지나
    /// 쥐었다 — 남의 것으로 풀리거나, 제 것으로 붙들고 막았다. 버린 줄은 기록이 없던 때처럼 판정한다.
    pub fn fold<S: AsRef<str>, T: AsRef<str>>(
        me: &str,
        files: impl IntoIterator<Item = (S, T)>,
        since: &dyn Fn(&str) -> Option<i64>,
    ) -> Picks {
        /// 줄 하나에 쌓인 마지막 기록 — 이름을 달아 둔다([`Bailout`] 과 같은 까닭이다).
        /// `None` 은 어느 때보다 이르다.
        #[derive(Default)]
        struct Seen {
            /// 이 세션이 **확실히 도는 집기**로 적은 마지막 때 — [`Picks::line`] 의 `?` 가 안 붙은 줄이다.
            sure: Option<u128>,
            /// 이 세션이 적은 마지막 때 — `?` 가 붙은 줄까지. 남이 쥔 줄이 없을 때만 쓴다.
            mine: Option<u128>,
            /// 다른 세션이 **확실히 도는 집기**로 적은 마지막 때 — 제 집기와 겨루는 것은 이것뿐이다.
            sure_theirs: Option<u128>,
            /// 다른 세션이 적은 마지막 때 — `?` 가 붙은 줄까지. **있는가만 본다**: 이 줄이 서면 제
            /// `?` 로는 못 가져오고, 비면 제 `?` 한 줄로도 제 것이다. 때를 겨루는 것은 `sure_theirs` 뿐이다.
            theirs: Option<u128>,
        }
        let mut last: std::collections::BTreeMap<String, Seen> = Default::default();
        for (sid, text) in files {
            let mine = sid.as_ref() == me;
            for line in text.as_ref().split_inclusive('\n') {
                let Some(line) = line.strip_suffix('\n') else { continue };
                let mut parts = line.split('\t');
                let (Some(at), Some(id)) = (parts.next(), parts.next()) else { continue };
                let Ok(at) = at.parse::<u128>() else { continue };
                let stamp = parts.next().and_then(|s| s.parse::<i64>().ok());
                if let (Some(stamp), Some(since)) = (stamp, since(id))
                    && stamp + Self::SLACK_SECS < since
                {
                    continue;
                }
                let sure = parts.next() != Some("?");
                let seen = last.entry(id.to_string()).or_default();
                if !mine {
                    seen.theirs = seen.theirs.max(Some(at));
                    if sure {
                        seen.sure_theirs = seen.sure_theirs.max(Some(at));
                    }
                    continue;
                }
                // **확실한 것과 모르는 것을 따로 든다**(리뷰 moai-51h9.k8j1). 마지막 한 줄만 들던 판은
                // 뒤에 선 `?` 한 줄이 앞서 확실히 집은 줄을 덮어, 정말 쥔 줄을 남에게 내줬다. `max` 로
                // 드는 것은 한 `record_picks` 의 줄들이 같은 때를 달고 나가기 때문이다 — 차례로 가르면
                // 같은 때의 두 줄에서 먼저 적힌 쪽이 이겨, 답이 적는 차례에 매인다.
                seen.mine = seen.mine.max(Some(at));
                if sure {
                    seen.sure = seen.sure.max(Some(at));
                }
            }
        }
        let mut out = Picks::default();
        for (id, Seen { sure, mine: m, sure_theirs: st, theirs: t }) in last {
            // **돌았는지 모르는 집기는 남의 기록을 안 지운다**(moai-hze6, 사용자 결정). 적기는 적는다 —
            // 기록은 "이 세션이 집었을 수 있다" 를 모으는 자리다. 다만 그것으로 남이 쥔 줄을 제 것으로
            // 삼으면, 안 돈 집기 하나가 남의 일을 초점에 세워 규칙 1 이 막고 `Stop` 이 그것을 닫으라고
            // 붙든다 — 기록은 풀기만 한다(moai-m5mg 의 elsewhere 보존과 같은 자다).
            //
            // **그 표는 남의 줄에서도 같은 뜻이다**(moai-dbzs, 사용자 결정 2026-09-20). 쓰는 쪽만 표를
            // 달고 읽는 쪽은 버리던 판은, 옆 세션의 안 돈 집기 한 줄이 이 세션이 확실히 쥔 줄을 가져갔다
            // (리뷰 moai-51h9.q5l 의 6번). 겨루는 자를 `sure_theirs` 로 좁혀 그 줄을 안 빼앗기되, 제
            // 기록이 아예 없는 줄은 여전히 `?` 한 줄로도 모르는 것이 된다(`t`) — 그쪽이 쥐었을 수 있다는
            // 뜻이고, **그 뒤 칸(`t`)으로는 새로 막지 않는다**: 모르는 줄은 풀기만 한다(moai-4jsy).
            //
            // **앞 칸(`sure > st`)은 그렇지 않다**(리뷰 moai-51h9.3jh). 이쪽은 `mine` 을 넓히고
            // `mine` 은 `Away::picked` 로 흘러(`cmd/hook.rs`) `unsure` 가 그 줄을 빼는 것을 막는다 —
            // 옆 워크트리 스냅샷이 쥔 줄(`elsewhere`, moai-m5mg)까지 그 자리에서 안 풀린다. 남의 `?`
            // 한 줄에 제 초점을 안 내준다는 사용자 결정(2026-09-20 결정 2)이 치르는 값이고, 대신
            // 넘겨받기가 약해진다 — 남이 `&&` 로 이어 집으면 제 옛 확실한 줄이 그대로 이긴다.
            //
            // **확실한 기록이 한 쪽에도 없으면 마지막에 적은 쪽이 이긴다**(moai-kjeh, 사용자 결정
            // 2026-09-20). `?` 끼리 겨루는 자를 안 두던 판은 그 줄을 어느 쪽에도 안 줘, 두 세션이
            // 모두 `cd … && moai mv X in_progress --from todo` 로 집은 줄이 주인 없이 남았다 —
            // `Stop` 이 아무도 안 붙들고 규칙 1 도 그 줄로 안 묶인다(리뷰 moai-51h9.3jh 의 5번).
            // 남이 **확실히** 쥔 줄은 여전히 `?` 로 못 가져오니 결정 2 는 그대로다.
            let mine = match (sure, st) {
                (None, None) => m > t,
                _ => sure > st,
            };
            match mine {
                true => out.mine.insert(id),
                false => out.theirs.insert(id),
            };
        }
        out
    }
}

/// **누구의 것인지 모르는** 집은 줄 — 옆 딸린 워크트리의 스냅샷에도 벌여 놓인(또는 거기서 늦게
/// 옮긴) 줄(`elsewhere`, `worktree::held_elsewhere`)과 **다른 세션이 마지막으로 확실히 집은** 줄(`picks`)이다
/// (moai-ntl6 사용자 결정 B, moai-4jsy).
///
/// 이름이 id 가 아닌 워크트리가 갈라질 때 이미 집혀 있던 일은 그 워크트리의 것일 수 있다.
/// **이 줄로는 막지도 붙들지도 않는다** — 받는 쪽은 이것을 `away` 에 더한 좁은 초점으로 한 번
/// 더 판정해, 풀릴 때만 푼다. 새로 막는 일은 없다. **제 워크트리 이름이 가리키는 일은 확실히
/// 제 것이라 빼지 않는다**(`own`) — 이름으로 가르던 판정은 그대로다. 대가: main 이 제 몫으로
/// 집은 뒤 갈라진 워크트리가 생기면 그 집기는 `Stop` 이 더는 안 붙든다(사용자가 받아들였다).
///
/// **제 이름이 거리 0 으로 가리키는 줄은 기록보다 앞이다**(사용자 결정 2026-09-19, moai-u8al).
/// `worktree-<그 id>` 안에 선 세션은 그 일의 자리에 있다 — 거두기(죽은 세션의 멤버를 이어받기)와
/// `/clear` 뒤의 새 세션은 앞 세션의 기록을 못 물려받아, 기록만 보면 제 일을 통째로 놓았다. 대가:
/// 남의 워크트리에 들어간 세션은 그 이름이 거리 0 으로 가리키는 줄을 다시 떠안는다 — 그 자리에
/// 들어가 있는 것이 곧 그 일을 보고 있다는 뜻이라고 본다.
///
/// **세션의 기록은 그 밖에서는 이름보다 앞이다**(moai-4jsy). 남의 워크트리에 잠깐 들어간 세션은 그 워크트리의
/// 이름을 제 이름으로 달아, 거기서 집힌 줄이 "확실히 제 것" 이 됐다 — `Stop` 이 그 세션에 남의 일을
/// 닫거나 미루라고 댔다. 남이 머지 직후 워크트리를 치운 줄도 루트의 아무 세션에게 그렇게 섰다.
/// 가르는 자는 [`Picks::fold`] 다 — **마지막으로 확실히 집은** 세션이 이기고, 아무도 확실하지 않으면
/// **마지막에 적은** 쪽이다(moai-kjeh). "마지막으로 집었으면" 으로만 적던 판은 `?` 가 선 뒤로 틀렸다
/// (moai-dbzs).
pub fn unsure(
    issues: &[Issue],
    cfg: &Config,
    elsewhere: &BTreeSet<String>,
    own: &BTreeSet<String>,
    picks: &Picks,
) -> BTreeSet<String> {
    if elsewhere.is_empty() && picks.theirs.is_empty() {
        return BTreeSet::new();
    }
    // **값싼 것을 먼저 거른다** — 기록은 지금은 안 집힌 줄까지 들고 오므로 위의 빠른 길은 곧 안 선다.
    // 모를 만한 줄이 없으면 소속 지도(`named_mine`)를 안 짓는다.
    //
    // **제 기록은 남의 기록만 지운다**(리뷰 moai-3k2d.1df 의 moai-ydtm). 옆 스냅샷이 쥔 줄
    // (`elsewhere`, moai-ntl6)까지 지우던 판은, 안 돌 수도 있는 토막(`a || moai mv Y …`)이 적은
    // 기록 하나로 그 줄을 도로 초점에 세워 **새로 막았다** — 사용자 결정(moai-4jsy)이 "새로 막는
    // 일은 없다" 였다. 기록으로는 풀기만 한다.
    let maybe: Vec<&Issue> = report::wip(issues, cfg)
        .into_iter()
        .filter(|i| !own.contains(&i.id))
        .filter(|i| (picks.theirs.contains(&i.id) && !picks.mine.contains(&i.id)) || elsewhere.contains(&i.id))
        .collect();
    if maybe.is_empty() {
        return BTreeSet::new();
    }
    let named_mine = report::claimed(issues, own);
    maybe
        .into_iter()
        .filter(|i| (picks.theirs.contains(&i.id) && !picks.mine.contains(&i.id)) || !named_mine(i))
        .map(|i| i.id.clone())
        .collect()
}

/// 이 명령줄이 **집는 id 들** — `only` 가 고른 토막의 `moai mv <id>… <벌여 놓는 칸>` 이다([`picks_up`]).
/// 훅이 세션의 집기를 적는 데 쓴다([`Picks`]).
///
/// **`--from` 에 질 집기는 안 낸다**(리뷰 moai-3k2d.1df). 훅은 명령이 돌기 전에 적으므로, 본 칸이 그 토막이
/// 겨눈 트래커의 지금 칸(`stands` — 토막 번호와 id 로 묻는다)과 다르면 `moai mv` 는 안 옮기는데 적기만 한다.
/// 그러면 겨루다 진 쪽이 마지막으로 집은 세션이 되어 이긴 쪽의 줄을 제 것으로 들고(`Stop` 이 남의 일을
/// 닫거나 미루라고 붙들고 규칙 1 이 막는다) 이긴 쪽은 제 줄을 놓는다. 지금 칸을 모르면(못 읽었다·그 줄이
/// 없다) 낸다 — 전과 같다. `--from` 없이 이미 선 칸으로 다시 집는 것은 넘겨받기라 낸다.
pub fn picked_in(
    cmd: &str,
    cfg: &Config,
    only: &dyn Fn(usize) -> bool,
    stands: &dyn Fn(usize, &str) -> Option<String>,
) -> Vec<(String, bool)> {
    // 훅은 도구 호출마다 돈다 — `mv` 라는 글자도 없으면 명령줄을 다시 가르지 않는다.
    if !cmd.contains("mv") {
        return Vec::new();
    }
    // **쓰기 규칙이 집기로 센 토막만 적는다**(moai-m5mg, 사용자 결정) — `! moai mv …`·`a || moai mv …`
    // 는 안 돌 수 있거나 져야 뒤가 돈다. 이음사를 안 보던 판은 그 id 도 이 세션의 집기로 적어, 남이 쥔
    // 줄을 제 것으로 붙들었다.
    //
    // **셈은 토막을 안 가리고 낸 뒤 여기서 고른다** — `only` 를 [`shell_scan`] 에 넘기면 그것이 고르는
    // 것은 낼 토막만이 아니라 **사슬**이다(`picked_before`). 남의 트래커를 보는 판에서는 제 토막의
    // 집기가 안 세어져 `a || moai -C /x mv B …` 의 B 가 통째로 빠졌다 — 앞이 지면 B 는 정말 돈다.
    let counted = shell_scan(cmd, cfg, &|_| true).1;
    let mut out = Vec::new();
    for (k, seg) in segments(cmd).into_iter().enumerate() {
        if !only(k) {
            continue;
        }
        // **한 토막이 두 번 세어졌으면 모르는 쪽이 이긴다** — `if ! bash -c '집기 A || 집기 B'; then
        // exit 1; fi` 는 묶음을 지나온 것이 조건이 이겼다는 뜻일 뿐, `||` 뒤의 B 가 돌았다는 뜻은
        // 아니다(A 가 이겼으면 B 는 안 돈다). 차례에 기대던 판은 같은 물음에 push 순서로 답했다.
        let Some(sure) = counted.iter().filter(|(n, _)| *n == k).map(|(_, sure)| *sure).reduce(|a, b| a && b) else {
            continue;
        };
        let Some(args) = moai_args(&seg) else { continue };
        let seen = flag_values(args, &["--from"]).pop();
        let verbs = positionals(args);
        let Some((_, ids)) = verbs.get(1..).and_then(<[&str]>::split_last) else { continue };
        out.extend(
            ids.iter()
                .filter(|id| seen.as_deref().is_none_or(|from| stands(k, id).is_none_or(|now| now == from)))
                .map(|id| (id.to_string(), sure)),
        );
    }
    out
}

/// [`picked_in`] 의 id 만 — 확실한가를 안 보는 시험이 쓴다. **한 자리에 둔다**: 시험마다 제 어댑터를
/// 두던 판은 그 하나가 `picked_in` 이라는 이름을 가려, 읽는 사람이 어느 것을 부르는지 몰랐다.
#[cfg(test)]
fn picked_ids(cmd: &str, cfg: &Config, only: &dyn Fn(usize) -> bool, stands: &dyn Fn(usize, &str) -> Option<String>) -> Vec<String> {
    picked_in(cmd, cfg, only, stands).into_iter().map(|(id, _)| id).collect()
}

/// 규칙 1 — **집은 것 밖에 새 이슈를 세우지 않는다.**
///
/// 초점 밖에 세우면 그 줄이 어느 일에서 나왔는지를 잃고, 에픽을 닫아도 남은
/// 것이 어디 있는지 아무도 모른다. 지금 할 일이 아니면 `idea` 로 담는다 —
/// 그쪽은 이 규칙에서 언제나 자유롭다. 단 **그 에픽(없으면 그 일)이 내건 것이 이것 없이
/// 안 이뤄지면 idea 가 아니다**(moai-l288) — 거절문이 그 자를 함께 댄다. 훅이 그것을
/// 가를 수는 없으니 막지는 않고 말만 한다. "첫 칸에 두면 안 닫힌다" 는 에픽이 있을 때만
/// 비친다 — 에픽 없는 일의 자식은 부모를 붙들지 않는다.
///
/// 훅은 토막을 고르는 [`guard_shell_in`] 으로 부른다. 토막 전부를 보는 이 모양은 시험이 쓴다.
#[cfg(test)]
pub fn guard_create(issues: &[Issue], cfg: &Config, away: &Away, cmd: &str) -> Decision {
    create_in(issues, &held(issues, cfg, away), cmd, &|_| true, &|_| None)
}

/// [`guard_create`] 를 **내미는 줄이 겨눌 트래커와 함께** — 그 자리를 푸는 것은 `cmd/hook.rs` 라
/// ([`Toward`]) 시험이 대신 댄다.
#[cfg(test)]
pub fn guard_create_toward(issues: &[Issue], cfg: &Config, away: &Away, cmd: &str, at: &Path) -> Decision {
    create_in(issues, &held(issues, cfg, away), cmd, &|_| true, &|_| Some(at))
}

/// [`guard_create`] 를 `only` 가 고른 토막에만 — 다른 트래커를 가리키는 토막은 그 트래커의
/// 줄로 본다([`aimed`]). 초점([`held`])은 [`guard_moai`] 가 한 번 잰 것을 받는다.
fn create_in<'a>(
    issues: &'a [Issue],
    focus: &[&'a Issue],
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
    aim: Toward<'_>,
) -> Decision {
    if focus.is_empty() {
        return Decision::Pass;
    }
    let unit = unit_of(issues, focus);

    // **토막마다 본다.** `cd /repo && moai add …` 의 뒷토막이 진짜 생성이다. **세우는 토막은 모두
    // 본다** — 첫 것만 보던 판은 단위 안에 세운 앞 토막 하나로 뒤의 맨 `moai add` 를 넘겼다. 치환은
    // 바깥 토막보다 먼저 쌓여, `x=$(moai add 'a' -e <에픽>); moai add 'b'` 가 그 모양이다(리뷰 moai-ju21.70g).
    //
    // **앞 토막의 에픽을 뒤로 물려주지 않는다**(moai-ean3, 2026-09-19 사용자 결정) — 물려주면
    // `moai add 'a' -e <에픽> && moai add 'b'` 의 `b` 가 어느 일에서 나왔는지를 잃는데, 그것이
    // 이 규칙이 지키려던 단 하나다. 토막마다 제 소속을 댄다
    // ([`tests::every_creating_segment_names_its_own_unit`] 이 그 뜻을 못박는다).
    let makes = segments(cmd).into_iter().enumerate().filter(|(k, _)| only(*k)).find(|(_, seg)| {
        // `add` 만 본다. `idea add` 는 담는 자리고, `--from` 은 에픽과 그
        // 자식들을 한 단위로 세우는 자리라 새는 줄이 아니다.
        //
        // **`--from` 도 토큰으로 본다.** 글자로 찾으면 제목이 그 낱말을 담은
        // `moai add "--from 을 나중에"` 가 규칙을 통째로 지나간다 — 동사를
        // 자리로 읽기로 한 것과 같은 까닭이다.
        // **플래그는 `moai` 의 인자에서만 읽는다** — 토막 전부를 훑던 판은 감싸는 명령의 것까지
        // 세, `sudo -h moai add x` 를 도움말로 읽었다(리뷰 moai-p836.rv).
        let Some(args) = moai_args(seg) else { return false };
        creates(seg)
            && (promotes_into(seg) || !args.iter().any(|t| t == "--from" || t.starts_with("--from=")))
            // **도움말은 만들지 않는다.** 우리가 심는 스킬이 "모르면
            // `moai <명령> --help` 를 보라" 고 적어 두는데, 그 길을 막으면
            // 규칙이 제가 시킨 것을 막는다.
            && !asks_help(args)
            // 단위 안에 세우는 것은 지나간다.
            && !flag_values(args, &["-e", "--epic", "--parent", "--milestone"]).iter().any(|v| unit.contains(v.as_str()))
    });
    let Some((at, seg)) = makes else {
        return Decision::Pass;
    };

    let shown = &focus[..focus.len().min(3)];
    let held = shown.iter().map(|i| format!("{} {}", i.id, i.title)).collect::<Vec<_>>().join(", ");
    // **막힌 토막이 겨눈 트래커를 댄다**(moai-nxw8, moai-v9sa) — 사람이 친 `-C` 를 옮겨 적는 것이
    // 아니라 [`aimed`] 가 푼 자리다. 옮겨 적던 판은 `moai -C .`·`moai -C ..` 로 막힌 사람에게 그
    // 글자를 도로 내밀어, 딴 자리에서 치면 엉뚱한 트래커를 겨누고 그 자리가 딸린 워크트리면
    // 워크트리의 스냅샷에 줄을 세웠다 — 루트에는 안 서고 병합에서 스냅샷이 겨룬다. 글자로만은
    // 못 푸는 `-C`(`$VAR`·`~`·`$( … )`)만 그대로 옮긴다([`echo_moai`]) — 버리면 남의 자리를 겨눈
    // 줄이 이 트래커를 겨눈 줄로 바뀐다.
    let moai = echo_moai(aim(at), &seg);
    // **에픽이 없으면 에픽을 대라고 말하지 않는다.** 없는 에픽 자리에 이슈 id 를
    // 넣어 일러 주던 자리다 — 시키는 대로 치면 `moai add '제목' -e <이슈>` 가
    // 만들어지고, `moai status` 에 "에픽으로 쓸 수 없는 것을 가리키는 줄" 이
    // 하나 는다. 그리고 경고가 늘면 `closing` 이 세션을 붙든다. 훅이 시킨 대로
    // 한 것이 훅에 걸리는 자리는 규칙이 아니라 덫이다.
    //
    // **여럿 집었으면 집은 것마다 댄다**(moai-nxw8) — 첫 것만 대던 판은 둘째 일에서 나온 것을
    // 첫 일의 에픽에 세우게 했다. 에픽은 겹치면 한 번이다.
    let (epics, loose) = epics_of(issues, shown);
    let into_epic: String =
        epics.iter().map(|e| format!("\x20 {moai} add '제목' -e {e}        같은 에픽 안에\n")).collect();
    let under: String =
        shown.iter().map(|i| format!("\x20 {moai} add '제목' --parent {}   그 일의 자식으로\n", i.id)).collect();
    // **무엇이 내건 것인지는 갈림길 1 과 같은 자로 댄다 — 에픽이다.** "그 일" 로 적던 판은 집은
    // 이슈가 아니라 에픽이 필요로 하는 것(moai-1k17 이 그 모양)에서 갈림길 1 과 다른 답을 냈다.
    // 에픽이 없는 집은 일만 그 일 자신이다.
    // 여럿 집어 에픽이 여럿이면 그 모두다 — idea add 에 비추는 줄([`aside_in`])과 같은 꼴이다.
    // 이름은 [`aside_in`] 의 같은 값과 맞춘다 — `aim` 으로 적던 판은 겨눌 트래커를 묻는 매개변수
    // (`aim: Toward`)를 이 자리에서 가려, 위의 `echo_moai(aim(at))` 를 한 줄만 내려도 안 되는 글이 됐다.
    let aims = epics.iter().chain(&loose).copied().collect::<Vec<_>>().join("·");
    // **첫 칸에 둔 줄이 일을 열어 두는 것은 에픽뿐이다** — 에픽의 칸은 멤버에서 읽지만, 에픽 없는
    // 일은 자식이 첫 칸에 있어도 그대로 닫힌다. 그때 "첫 칸에 두면 안 닫힌다" 를 비치면 거짓이다.
    // 가리키는 줄도 이름으로 댄다 — "위의 줄" 바로 위가 `idea add` 줄이고, idea 도 첫 칸에 선다.
    // **에픽 있는 일과 없는 일을 함께 쥐었으면 둘 다 댄다** — 첫 것만 보던 판은 에픽 없는 일의 경고를
    // 말없이 뺐다(리뷰 moai-ju21.70g).
    let mut keep = Vec::new();
    if !epics.is_empty() {
        keep.push(format!("위의 `moai add` 줄로 세워 첫 칸에 둔다. 밖으로 내보내면 {} 가 목적을 못 이룬 채 닫힌다", epics.join("·")));
    }
    if !loose.is_empty() {
        keep.push(format!("위의 `--parent` 줄로 세운다. 에픽 없는 일은 자식이 남아도 닫히니 {} 를 닫기 전에 끝낸다", loose.join("·")));
    }
    let keep = keep.join("\n");
    // 둘째 물음의 글은 규칙 글과 한 출처다(moai-nxw8) — 손으로 옮겨 적던 판은 한쪽만 고쳐도
    // 안 붉어졌다.
    let pledge = crate::guide::PLEDGE;
    refuse(1, format!(
        "지금 집고 있는 것이 있다 — {held}.\n\
         그 단위 안에서 만들거나, 밖의 것이면 담아 둔다. 초점 밖에 이슈를 세우면\n\
         그 줄이 어느 일에서 나왔는지를 잃는다.\n\
         {into_epic}{under}\
         \x20 {moai} idea add '제목'                 지금 할 일이 아니면 담아 둔다\n\
         {aims} 가 {pledge} idea 가 아니다 — 지금 못 해도\n\
         {keep}"
    ))
}

/// 규칙 3 의 뒷짝 — **리뷰를 닫을 때 무엇이 나왔는지를 남긴다.**
///
/// `moai mv <리뷰> done` 을 가로챈다. `-m` 이 없으면 거절한다 — 그 한 줄은
/// 저널에 남아, 다음 사람이 `moai show` 로 그 리뷰가 무엇을 냈고 무엇을
/// 넘겼는지 읽는다.
///
/// **`-m` 의 있고 없음만 본다. 저널을 읽지 않는다.** "이 리뷰에 결과가
/// 적혔나" 를 저널에 물으면 그때부터 규칙이 저널을 접기 시작하고, 그 길 끝에
/// 옛 `event.rs` 가 있다. 명령줄에 답이 있으면 명령줄에서 읽는다.
///
/// **`defer` 는 막지 않는다.** 안 하기로 한 리뷰에 결과를 적으라고 하면
/// 그것은 규칙이 아니라 덫이다.
///
/// 훅은 토막을 고르는 [`guard_shell_in`] 으로 부른다. 토막 전부를 보는 이 모양은 시험이 쓴다.
#[cfg(test)]
pub fn guard_close(issues: &[Issue], cfg: &Config, away: &Away, cmd: &str) -> Decision {
    close_in(issues, cfg, away, cmd, &|_| true, &|_| None)
}

/// [`guard_close`] 를 `only` 가 고른 토막에만 — [`create_in`] 과 같은 까닭.
fn close_in(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
    aim: Toward<'_>,
) -> Decision {
    for (k, seg) in segments(cmd).into_iter().enumerate() {
        if !only(k) {
            continue;
        }
        let Some(args) = moai_args(&seg) else { continue };
        let verbs = positionals(args);
        // `moai mv <id>... <칸>` — 맨 끝이 갈 칸이고 그 앞이 전부 옮길 것이다.
        let Some((&"mv", rest)) = verbs.split_first() else { continue };
        let Some((&to, ids)) = rest.split_last() else { continue };
        if to != "done" {
            continue;
        }
        // **빈 값은 안 적은 것이다.** 있고 없음만 보던 판은 `-m ""` 두 글자로
        // 지나갔다 — 이 규칙이 지키려던 단 하나(다음 사람이 읽을 한 줄)가
        // 그대로 무너진다. 값을 보는 것은 여전히 명령줄만 읽는 일이라
        // "저널을 안 읽는다" 는 결정과 어긋나지 않는다.
        if !flag_values(args, &["-m", "--msg"]).iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        // **지금 보는 것에 매인 리뷰만 본다.** 저장소 전체를 보던 판은, 옛
        // 세션이 남긴 리뷰 줄을 치우려는 사람에게 **돌린 적도 없는 리뷰의
        // 결과**를 지어내라고 요구했다. `closing` 이 같은 줄을 아예 안 세기로
        // 한 것과도 어긋난다 — 한 규칙의 두 짝은 같은 셈법을 써야 한다. 옆 워크트리의 리뷰도
        // 그래서 뺀다 — `closing`·[`guard_review`] 가 안 세는 줄이다(리뷰 moai-dw63.nzw).
        let unit = unit_of(issues, &held(issues, cfg, away));
        let epics = report::groups(issues);
        let out = report::put_off(issues);
        let theirs = theirs(issues, away);
        let open_review = ids.iter().find_map(|id| {
            issues.iter().find(|i| {
                i.id == *id
                    && is_review(i, &out)
                    && !i.status.is_done()
                    && !theirs(i)
                    && (unit.contains(i.id.as_str())
                        || epics.get(i.id.as_str()).is_some_and(|e| unit.contains(e))
                        || crate::id::parent_of(&i.id).is_some_and(|p| unit.contains(p)))
            })
        });
        if let Some(r) = open_review {
            return refuse(3, format!(
                "리뷰 {} 를 닫으면서 무엇이 나왔는지를 안 남긴다.\n{}\n\
                 넘긴 것은 이슈 번호와 함께 적는다 — \"넘겼다\" 만 적힌 줄은 아무도\n\
                 다시 안 본다.",
                r.id,
                // **닫는 두 걸음도 그 토막이 겨눈 트래커를 댄다**(moai-j2vp) — 규칙 1 과 한 자다.
                crate::guide::close_steps(&r.id, &echo_moai(aim(k), &seg))
            ));
        }
    }
    Decision::Pass
}

/// 리뷰 줄인가. 태그 하나로 가른다. **계획에서 빠진 줄은 리뷰로 안 친다** —
/// 제 줄을 미뤘든, 미룬 부모·에픽·마일스톤 밑이든 같다(`report::put_off`).
/// `report::wip` 가 물려받은 미룸으로 초점에서 빼는 줄을 여기서만 열린 리뷰로
/// 치면, 규칙 3 이 이미 집은 리뷰를 "집으라" 고 거듭 막아 시킨 대로 해도 안
/// 풀린다.
fn is_review(i: &Issue, out_of_plan: &BTreeSet<&str>) -> bool {
    i.tags.iter().any(|t| t == REVIEW_TAG) && !out_of_plan.contains(i.id.as_str())
}

/// 규칙 2 — **저장소를 고치기 전에 하나를 집는다.**
///
/// **도구가 아니라 고치는 파일로 가른다.** 도구로 가르면 스크래치패드 메모와
/// `src/` 의 한 줄이 같은 값으로 막히고, 그래서 세션당 한 번으로 풀어야 했다 —
/// 느슨해진 규칙은 정작 막아야 할 것을 놓친다.
pub fn guard_edit(issues: &[Issue], cfg: &Config, away: &Away, root: &Path, target: &str) -> Decision {
    if !counted(target, root) || !held(issues, cfg, away).is_empty() {
        return Decision::Pass;
    }
    // **본 칸을 함께 준다**(`--from`). 이 줄은 여럿이 같은 트래커를 쓰는 저장소에서 지어지므로, 짓고
    // 치는 사이에 옆이 그 일을 집거나 닫을 수 있다 — 그러면 옮기지 않고 말한다. 안 주던 판은 낡은
    // 스냅샷의 "집으라" 대로 이미 닫힌 일을 도로 열었다(리뷰 moai-ju21.70g).
    let picks: String = report::ready(issues, cfg)
        .into_iter()
        .take(3)
        .map(|i| format!("  moai mv {} in_progress --from {}   {}\n", i.id, i.status.as_str(), i.title))
        .collect();
    // **내미는 명령은 한 줄에 하나다** — 붙여 넣는 쪽이 줄째 옮겨 치기 때문이다. 두 명령을 한 줄에
    // 싣던 판은 그 줄이 갈리는 자리마다 한쪽만 옮겨져 "못 찾았다" 로 끝났다(리뷰 moai-ju21.70g).
    refuse(2, format!(
        "집은 것 없이 {} 를 고치고 있다. 어느 일에서 나온 변경인지가 남지 않는다.\n\
         하나를 집고 다시 부른다.\n{picks}\
         계획에 없던 것이면 세우고 그 id 를 집는다.\n\
         \x20 moai add '제목'\n\
         \x20 moai mv <id> in_progress",
        rel_to(target, root)
    ))
}

/// 규칙 2 의 껍데기 쪽 — **`Bash` 로 쓰는 파일도 센다.**
///
/// `Edit`·`Write` 만 보던 판은 `sed -i`·`>`·heredoc 을 그대로 보냈다. 규칙이
/// 못 보는 길이 따로 있으면 규칙은 절반만 서 있다.
///
/// 상대 경로는 **껍데기의 자리(`cwd`)** 로 푼다. 저장소 뿌리로 풀면 하위
/// 디렉터리에서 친 `echo > x` 가 엉뚱한 파일로 읽힌다.
#[cfg(test)]
pub fn guard_writes(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    root: &Path,
    cwd: &Path,
    cmd: &str,
) -> Decision {
    guard_writes_in(issues, cfg, away, root, cwd, cmd, &|_| true)
}

/// [`guard_writes`] 를 세션 자리의 트래커로 — 집기는 `only` 가 고른 `moai` 토막의 것만 센다.
/// `moai -C <남의 트래커> mv … in_progress` 는 여기서 아무것도 안 쥐어 주니, 그 뒤의 쓰기는
/// 집은 채로 쓰는 것이 아니다.
fn guard_writes_in(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    root: &Path,
    cwd: &Path,
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
) -> Decision {
    // **싼 것부터 잰다**(moai-xppm) — 쓰는 파일이 없는 명령이 대부분이고, 명령줄을 읽는 것은
    // 소속 지도를 짓는 [`held`] 보다 싸다. 거꾸로 재던 판은 Bash 마다 초점부터 지었다.
    let writes = shell_writes(cmd, cfg, only);
    if writes.is_empty() || !held(issues, cfg, away).is_empty() {
        return Decision::Pass;
    }
    for path in writes {
        let at = cwd.join(&path);
        if let deny @ Decision::Deny(_) = guard_edit(issues, cfg, away, root, &at.to_string_lossy()) {
            return deny;
        }
    }
    Decision::Pass
}

/// `Bash` 한 번을 규칙에 비춘다 — 만드는 것·닫는 것·쓰는 것, 그리고 리뷰를
/// 부르는 것. **먼저 걸리는 쪽이 이긴다** — 거절문은 하나면 된다.
///
/// 차례를 `cmd/` 가 아니라 여기 둔다. 거기 두면 시험이 같은 차례를 손으로 다시
/// 짜고, 실제로 그렇게 짠 시험은 쓰기 규칙을 아무것도 안 집은 상태에서만 봤다.
///
/// **리뷰를 부르는 명령도 나머지 규칙을 지난다.** 명령 전체를 리뷰로만 보던
/// 판은 `moai mv <리뷰> done && /code-review high` 한 줄로 닫기 규칙과 규칙 1 을
/// 통째로 넘겼다.
///
/// 훅은 토막을 고르는 [`guard_shell_in`] 으로 부른다. 토막 전부를 보는 이 모양은 시험이 쓴다.
#[cfg(test)]
pub fn guard_shell(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    root: &Path,
    cwd: &Path,
    cmd: &str,
) -> Decision {
    guard_shell_in(issues, cfg, away, root, cwd, cmd, &Segs { judges: &|_| true, picks: &|_| true }, &|_| None)
}

/// 토막마다 **이 자리의 것인가** — 규칙마다 답이 다르다.
///
/// **판정과 집기를 한 자로 두지 않는다**(moai-acf7 의 뒷짝). 딴 트래커를 가리킨 토막은 그
/// 트래커가 판정하지만([`Segs::judges`], moai-23ky), 갈라 놓은 **제** 트래커를 `-C` 로 가리킨
/// 토막(`MOAI_HERE=1` 워크트리)의 집기는 여전히 이 세션의 것이다.
/// 한 자로 두던 판은 규약이 시키는 `moai -C <루트> mv <id> in_progress && sed -i …` 를 규칙 2 가
/// "집은 것 없이 고친다" 며 막았다 — 방금 집은 그 id 를 집으라고 내밀면서.
pub struct Segs<'a> {
    /// `moai` 규칙(1·3)이 볼 토막 — 딴 트래커를 가리킨 것은 그 트래커가 본다.
    pub judges: &'a dyn Fn(usize) -> bool,
    /// 규칙 2 가 **이 자리의 집기**로 셀 토막.
    pub picks: &'a dyn Fn(usize) -> bool,
}

/// [`guard_shell`] 을 세션 자리의 트래커로 — 만들기·닫기 규칙은 [`Segs::judges`] 가 고른 `moai`
/// 토막만 본다. 다른 트래커를 가리키는 토막은 [`guard_moai`] 가 그 트래커의 줄로 본다(moai-23ky).
/// 쓰기와 리뷰는 세션 자리의 규칙이라 명령 전체를 본다 — 쓰기 규칙이 토막으로 가르는 것은
/// 어느 집기가 이 자리의 것인가 하나고, 그래서 [`Segs::picks`] 로 따로 묻는다.
pub fn guard_shell_in(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    root: &Path,
    cwd: &Path,
    cmd: &str,
    segs: &Segs<'_>,
    aim: Toward<'_>,
) -> Decision {
    // 차례는 [`Decision::then`] 이 정한다 — 먼저 막는 규칙이 이기고, 비추는 줄(`Context`)은 뒤의
    // 규칙이 막을 것을 가리지 않는다. 되돌릴 수 없는 것을 먼저 본다.
    guard_tmux(cmd)
        .then(|| guard_moai(issues, cfg, away, cmd, segs.judges, aim))
        .then(|| guard_writes_in(issues, cfg, away, root, cwd, cmd, segs.picks))
        .then(|| if calls_review(cmd) { guard_review(issues, cfg, away) } else { Decision::Pass })
}

/// 규칙 4 — **사람의 tmux 서버를 죽이지 않는다**(moai-zis7, 사용자 결정).
///
/// 세션이 tmux 안에서 돌면 `$TMUX` 가 서 있고, `-L`·`-S` 없는 tmux 는 `TMUX_TMPDIR` 를 무시하고
/// 그 서버에 붙는다. 2026-09-18 리뷰 서브에이전트의 `TMUX_TMPDIR=… tmux kill-server` 한 줄이
/// 사람의 서버를 죽여 감독과 일꾼 다섯이 한꺼번에 꺼졌다. 그때 막은 것은 사람 한 명의 개인
/// 훅뿐이라, 다른 기계에서는 글만이 그 사이에 선다 — 그래서 심는 훅에 싣는다.
///
/// **다른 규칙보다 먼저다** — 집은 것이 있든 없든, 어느 트래커를 가리키든 같다. 트래커를 안 묻는
/// 규칙이라 훅은 자리와 트래커를 찾기 전에 이것부터 부른다(`cmd/hook.rs` 의 `run`) — 세션 자리가
/// 트래커 밖이어도 선다.
/// 토막 안의 **어느 낱말이든** `tmux` 면 본다: `env -u TMUX tmux …`·`sudo tmux …` 의 tmux 는
/// 명령 자리에 있지 않다. 따옴표로 묶인 글(`moai note … "tmux kill-server"`)은 낱말 하나라
/// `tmux` 가 아니다. 창·칸을 닫는 `kill-window`·`kill-pane` 은 서버를 안 끝내 안 본다.
pub fn guard_tmux(cmd: &str) -> Decision {
    // tmux 는 모호하지 않은 앞머리도 그 명령으로 받는다 — `kill-ser` 가 `kill-server` 다.
    let kills = |w: &str| w.len() > "kill-s".len() && ["kill-server", "kill-session"].iter().any(|k| k.starts_with(w));
    for seg in segments(cmd) {
        for (i, word) in seg.iter().enumerate() {
            let rest = &seg[i + 1..];
            match basename(word) {
                "tmux" => {
                    let Some(at) = rest.iter().position(|w| kills(w)) else { continue };
                    // `-L default` 는 이름을 댔어도 사람의 기본 서버다 — 맨 tmux 가 붙는 바로 그 소켓이다.
                    let own = rest[..at].iter().enumerate().any(|(n, f)| match f.as_str() {
                        "-L" => rest.get(n + 1).is_none_or(|v| v != "default"),
                        f => f.starts_with("-S") || (f.starts_with("-L") && f != "-Ldefault"),
                    });
                    if own {
                        continue;
                    }
                    return refuse(4, format!(
                        "`tmux {}` 가 -L·-S 없이 사람의 tmux 서버를 겨눈다.\n\
                         세션이 tmux 안에서 돌면 맨 tmux 는 TMUX_TMPDIR 를 무시하고 그 서버에 붙어, 이 한 줄이\n\
                         그 안의 세션을 모두 끈다. 시험용 서버를 따로 띄워 거기에 친다.\n\
                         \x20 {}",
                        rest[at],
                        crate::guide::TMUX_OWN.replace('…', &rest[at..].join(" "))
                    ));
                }
                "pkill" | "killall" if rest.iter().any(|w| w.split(|c: char| !c.is_alphanumeric()).any(|p| p == "tmux")) => {
                    return refuse(4, format!(
                        "`{}` 가 tmux 를 겨눈다 — 사람의 tmux 서버까지 끈다.\n\
                         시험용 서버를 따로 띄웠으면 그 서버에 kill-server 를 친다.\n\
                         \x20 {}",
                        basename(word),
                        crate::guide::TMUX_OWN.replace('…', "kill-server")
                    ));
                }
                _ => {}
            }
        }
    }
    Decision::Pass
}

/// 이 명령이 **쓰는 파일들.** 흔한 모양만 본다 — `>`·`>>` 리다이렉션,
/// `sed -i`, `tee`.
///
/// **완벽을 노리지 않는다.** 껍데기가 파일을 쓰는 길은 `cp`·`mv`·`install`·
/// `truncate`·`python -c "open(...)"` 까지 끝이 없고, 그 목록은 반드시 샌다.
/// 그 대신 **잘못 막지 않는다** — 못 잡는 것보다 엉뚱한 것을 막는 쪽이 훨씬
/// 나쁘다. 규칙 1 이 명령줄 글자를 훑다가 `moai note` 를 막던 때가 그 증거다.
/// 그래서 어디인지 모르는 과녁은 버린다: 변수·틸드·글롭·프로세스 치환이 든
/// 것, 그리고 `cd` 뒤의 상대 경로.
///
/// **하나를 집는 명령 뒤에 `&&` 로 이은 쓰기는 세지 않는다.** 훅은 명령이 돌기 전의
/// 상태를 본다 — `moai mv <id> in_progress && …` 를 막으면, 규칙이 시킨 차례를 한 줄로
/// 친 명령이 "하나를 집고 다시 부른다" 는 거절을 받는다.
///
/// **`;`·`||`·`|`·`&`·줄바꿈 뒤는 센다.** 그 뒤는 집기가 져도 돈다 — `--from` 으로 겨루다 진
/// 집기 뒤의 `; sed -i …` 는 아무것도 안 쥔 채 저장소를 고친다. 첫 토막에서 멈추던 판이 그
/// 길을 통째로 열어 두었다(moai-gbqb). 묶음 안에서 읽은 이음사는 그 묶음 안의 것이라,
/// `&&` 로 들어간 `( a; b )`·`{ a; b; }` 는 통째로 집기 뒤다. 파이프는 `&&` 보다 단단히 묶여
/// `mv && a | tee f` 의 `tee` 도 집기 뒤고, 집기가 든 파이프라인(`mv | tee f`)은 집기 뒤가 아니다.
///
/// 다만 **집기가 이긴 것이 확실하면 `;` 뒤도 집기 뒤다**(moai-gtkn). 둘이다 — 집기 목록 바로 뒤의
/// `|| exit`(그 뒤로는 집기가 늘 이겼다. 제 묶음을 나오면 걷는다 — `( mv || exit 1 )`·`if …; then mv
/// || exit 1; fi`)와, 맨 바깥에서 `set -e` 아래 **홀로 선** 집기 뒤의 `;`(그 집기가 지면 껍데기가
/// 끝난다). bash 의 errexit 는 `&&`·`||` 목록의 앞 칸, `if`·`while` 의 조건, 파이프의 칸, `&` 로 띄운
/// 것, `!` 에서는 안 끝난다 — 그 뒤의 `;` 는 여전히 집기를 끊는다. 모든 `;` 를 `&&` 로 읽던 판은
/// `x || exit 1; mv --from todo; sed -i …` 와 `set -e; mv && echo; sed -i …` 에서 진 집기 뒤의 쓰기를
/// 넘겼다(리뷰 moai-ju21.70g).
///
/// **몸통이 안 돌았을 수 있는 묶음 안의 집기는 그 묶음 밖으로 안 이어진다** — `if`·`case`·`while`·
/// `for` 와 `a || { … }`. 치환 안의 집기도 바깥 명령의 `&&` 로 안 이어진다 — 바깥 명령의 값은 치환의
/// 값이 아니다.
///
/// **집기로 안 세는 것:** 남의 트래커를 가리킨 집기(`only` 가 안 고른 토막 — 여기서 아무것도
/// 안 쥐어 준다)와 `! moai mv …`·`! ( moai mv … )` (집기가 져야 뒤가 돈다), `a || moai mv …` (앞이
/// 이기면 집기가 안 돈다 — 앞도 집기면 어느 쪽이든 하나를 쥔다).
fn shell_writes(cmd: &str, cfg: &Config, only: &dyn Fn(usize) -> bool) -> Vec<String> {
    shell_scan(cmd, cfg, only).0
}

/// [`shell_writes`] 의 한 걸음 — 쓰는 파일들과 함께 **집기로 센 토막의 번호**([`segments`] 의 번호)를 낸다.
/// [`picked_in`] 이 그 번호로 세션의 집기를 적는다(moai-m5mg) — 집기를 두 자리에서 따로 가르면 한쪽만
/// 고쳐지는 날 `! moai mv …` 가 쓰기에는 빈손인데 기록에는 제 집기로 선다.
fn shell_scan(cmd: &str, cfg: &Config, only: &dyn Fn(usize) -> bool) -> (Vec<String>, Vec<(usize, bool)>) {
    /// `if ! 집기; then exit 1; fi` 의 **조건에 선 집기**([`Bailout::cond`]) — 토막 번호와 그것이
    /// 확실히 도는가다. 번호만 들던 판은 그 집기를 늘 확실한 것으로 세워, `a && if ! 집기; then exit 1;
    /// fi` 처럼 `if` 자신이 조건에 매인 줄에서 안 돌 수도 있는 집기를 확실한 것으로 적었다(moai-dbzs).
    #[derive(Clone, Copy)]
    struct Cond {
        /// 그 집기의 토막 번호([`segments`] 의 번호).
        at: usize,
        /// 그 집기가 확실히 도는가 — `shell_scan` 의 `certain` 이다.
        sure: bool,
    }
    /// **집기가 지면 끝내는 묶음** 하나([`shell_writes`], moai-ncay).
    ///
    /// 이름 없는 네 자리 튜플로 두면 `floor`·`depth`·`held` 가 자리로만 갈려, 하나를 바꿔 적어도
    /// 컴파일이 된다 — 그러면 집기가 조용히 새거나 조용히 막힌다.
    struct Bailout {
        /// 그 묶음의 깊이([`Seg::level`]). 여기보다 얕은 자리를 지나오면 묶음을 나온 것이다.
        floor: usize,
        /// 그 묶음이 선 하위 셸의 깊이([`Seg::depth`]) — `( … )` 안의 `exit` 는 그 하위 셸만 끝낸다.
        /// 더 얕은 자리로 나왔으면 바깥 셸은 살아 있다.
        depth: usize,
        /// 들어설 때의 `after_pick` — 묶음을 지나면 도로 세운다.
        held: Option<usize>,
        /// `if ! 집기; then exit 1; fi` 의 **조건에 선 집기**([`Cond`]). 그 집기는 뒤집혀 있어
        /// 적히지 않지만(`negated`), 묶음을 지나온 것은 그것이 이겼다는 뜻이라 쓰기 규칙은 집은
        /// 것으로 센다 — 기록도 같이 세워야 두 자리가 안 갈린다(moai-m5mg).
        cond: Option<Cond>,
        /// 지금까지 본 것이 그 꼴인가 — 묶음의 마지막 줄이 `exit` 여야 한다.
        ends: bool,
        /// 그 `exit` 가 **0 아닌 값**을 대는가(moai-4arw, 리뷰 moai-k8j1.209 의 3번) — 겹을 나올 때만
        /// 본다. 맨 바깥에서는 `exit 0` 도 껍데기를 끝내 뒤가 아예 안 돌지만, `bash -c` 의 글에서는
        /// 0 으로 끝난 자식이 바깥 `&&` 에 그대로 닿는다 — 그때 여기 온 것은 집기가 이겼다는 뜻이
        /// 아니다. 맨 낱말로 잰다: `exit` 혼자는 앞 명령의 값이고 `exit "$n"` 은 글자만으로 모른다.
        ///
        /// **`hard` 면 `ends` 다** — 둘은 같은 자리에서 서고 이쪽이 더 좁다(`ends && 0 아닌 값`).
        /// 푸는 자리가 둘을 함께 묻지 않는 것은 그래서다([`Bailout::resolves`]).
        ///
        /// 몸통이 겹문인 `if ! 집기; then <묶음>; fi` 를 이것이 막는다고 적었던 판은 틀렸다
        /// (리뷰 moai-k8j1.udq 의 3번) — `if` 꼴은 둘 다 참으로 시작하고, 겹문인 몸통은 제 자리에
        /// `exit` 토막을 안 내 둘 다 참으로 남았다. 그 줄을 막는 것은 `own` 이다(아래).
        hard: bool,
    }
    impl Bailout {
        /// **겹을 나오며 이 묶음을 풀 수 있는가** — 전제 다섯을 한 자리에 모은다. 가름자를 두 벌로
        /// 적던 판은 쓰기 규칙과 기록이 갈릴 자리를 열어 뒀다(moai-m5mg 가 닫은 그 갈림이다).
        fn resolves(&self, top: usize, deep: usize, iffy: Option<usize>) -> bool {
            self.hard && self.floor >= top && self.depth <= deep && iffy.is_none_or(|c| self.floor < c)
        }
    }
    /// 다시 읽은 겹 하나([`Layer`])에 들어설 때의 판 — 나오면 되돌린다.
    ///
    /// **한 셸에 매인 것은 모두 여기 든다**(moai-9xbq) — `set -e`(`strict`)와 홀로 선 명령(`lone`)뿐
    /// 아니라 `|| exit` 로 이긴 집기(`sure`·`sure_e`)와 집기가 지면 끝내는 묶음(`bailout`)도 그렇다.
    /// 넷만 들던 판은 `bash -c '집기 || exit 1; echo done' && 쓰기` 를 막았다 — 안에서 이긴 집기가
    /// 겹을 나오며 사라졌다.
    struct Frame {
        layer: Layer,
        /// 들어설 때의 `after_pick` — 치환을 나오면 도로 세운다.
        held: Option<usize>,
        strict: Option<usize>,
        lone: Option<usize>,
        sure: Option<usize>,
        sure_e: Option<usize>,
        bailout: Option<Bailout>,
    }
    /// `set -e` 가 껍데기를 끝내는 맨 윗자리 — 지금 겹의 셸의 것이다. 치환 안이면 없다([`Layer::Subst`]).
    fn errexit_top(frames: &[Frame]) -> Option<usize> {
        match frames.last().map(|f| f.layer) {
            None => Some(0),
            Some(Layer::Shell { top, .. }) => Some(top),
            Some(Layer::Subst(_)) => None,
        }
    }
    let mut out = Vec::new();
    // `cd` 를 지난 하위 셸의 깊이 — 그 뒤의 상대 경로는 어디인지 모른다. **하위 셸을 나오면
    // 걷는다**: 치환(moai-xe6e)과 `( cd … )` 의 `cd` 는 괄호 밖으로 안 이어진다. 안 걷던 판은 그
    // 뒤의 상대 경로 쓰기를 모른다며 버려 샜다. `{ cd …; }` 는 하위 셸이 아니라 안 걷는다.
    let mut moved: Option<usize> = None;
    // 집기 뒤 `&&` 로만 이어 온 동안의 묶음 깊이 — 이보다 얕거나 같은 자리에서 `&&` 가 아닌
    // 이음사를 만나면 끝난다.
    let mut after_pick: Option<usize> = None;
    // 깊이마다, 지금 열린 파이프라인에 들어설 때의 `after_pick` — 파이프의 칸은 그것을 잇는다.
    let mut heads: Vec<Option<usize>> = Vec::new();
    // 같은 것을 기록 쪽으로 — 깊이마다 그 파이프라인의 **머리가 확실히 도는가**다(`certain`).
    // `heads` 와 한 자리에서 서고 한 자로 걷힌다. `|` 는 `&&`·`||` 보다 단단히 묶여 `a && b | 집기`
    // 의 집기도 `a` 가 져야 안 도는데, 그 집기의 이음사는 `Op::Pipe` 라 제 자리만 봐서는 모른다.
    let mut piped: Vec<bool> = Vec::new();
    // [`segments`] 의 토막 번호 — `only` 가 그것으로 가른다. 낱말 없는 토막(`> f` 만)은 안 센다.
    let mut k = 0;
    // `set -e` 를 켠 묶음의 깊이. 묶음을 나오면 걷는다 — `{ set -e; }` 뒤처럼 bash 가 이어 가는 자리도
    // 모르는 쪽(세는 쪽)으로 선다.
    let mut strict: Option<usize> = None;
    // 마지막 **홀로 선** 명령의 묶음 깊이 — `set -e` 아래서 같은 깊이의 다음 `;` 는 그 명령이 이겨야 닿는다.
    let mut lone: Option<usize> = None;
    // 집기가 **이긴 것이 확실한** 묶음의 깊이 — 집기 목록 뒤의 `|| exit` 를 지났다.
    let mut sure: Option<usize> = None;
    // 같은 것을 `set -e` 로 — 맨 바깥에서 홀로 선 집기를 지났다. `set +e` 가 함께 걷는다: 그 뒤의 `;` 는
    // 다시 집기를 끊는다고 읽는다(모르는 쪽으로 선다).
    let mut sure_e: Option<usize> = None;
    // `! ( … )` 의 `!` 가 여는 묶음 — 그 안의 집기는 져야 뒤가 도니 아무것도 안 쥔다.
    let mut negated_at: Option<usize> = None;
    // 집기 없이 `||` 로 들어간 묶음의 깊이와 **그 `||` 를 읽은 토막의 번호** — 앞이 이기면 그 묶음은
    // 통째로 안 돈다. 번호를 함께 드는 것은 기록([`picked`]) 때문이다: 치환은 바깥 토막보다 먼저
    // 쌓이므로 `x=$(집기) || { … }` 의 집기는 그 `||` 를 읽기 전에 **이미 돌았다**. 깊이로만 걷던 판은
    // 그것을 함께 버려, 이 저장소가 스스로 일러 주는 집기 꼴(`src/guide.rs` 의 bash 고리)이 아무것도
    // 기록하지 않았다.
    let mut orelse: Option<(usize, usize)> = None;
    // **조건에 매여 들어간 묶음의 가장 얕은 깊이**(리뷰 moai-k8j1.udq 의 2·4번) — `a && { … }` 의
    // 묶음도, 함수 몸통(`f() { … }`)도 아예 안 돌 수 있다. `orelse` 와 가르는 것은 쓰는 자리다:
    // 저쪽은 묶음을 **나올 때** 집기를 걷지만, 이것은 겹을 나올 때 끝내는 묶음을 푸는 자리만 막는다.
    // 걷는 쪽으로 같이 쓰면 `a && { 집기; } && 쓰기` 를 막는다 — 그 쓰기는 묶음이 돌아야 닿으니
    // 집기도 돈 것이다.
    let mut iffy: Option<usize> = None;
    // **조건에 매여 들어간 묶음의 가장 얕은 깊이 — 기록 쪽의 자**(moai-dbzs). `iffy` 와 같은 자리에서
    // 서고 같은 자로 걷히되, **쥔 것이 있어도 센다**: 저쪽은 "이 묶음이 돌았으면 집기가 이겼다" 를
    // 나중에 재는 자라 `집기 && { … }` 를 안 세지만, 기록은 명령이 **돌기 전에** 적히니 `집기A &&
    // { 집기B; }` 의 B 도 A 가 져서 안 돌 수 있다. 그 자리를 확실한 집기로 적으면 정말 쥔 세션에게서
    // 초점을 빼앗는다([`Picks::fold`]).
    let mut chancy: Option<usize> = None;
    // 함수 정의의 머리(`f(`)를 본 자리 — 다음 토막이 그 몸통이다.
    let mut fndef: Option<usize> = None;
    // 다시 읽은 겹마다([`Seg::nested`]), 그 겹에 들어설 때의 판 — **나오면 되돌린다.** 새 셸은 제
    // `set -e` 와 홀로 선 명령을 제 맨 윗자리에서 센다(moai-tmi0): 맨 바깥 깊이 0 에만 매던 판은
    // `bash -c 'set -e; 집기; 쓰기'` 를 막았다. 치환이면 `after_pick` 도 되돌린다 — 바깥 명령의 값은
    // 치환의 값이 아니라, 치환 안의 집기는 바깥의 `&&` 로 안 이어진다(리뷰 moai-ju21.70g). 셸에 넘긴
    // 글이면 안 되돌린다 — 그 글의 값이 바깥 토막의 값이다(moai-plfi).
    let mut frames: Vec<Frame> = Vec::new();
    // **집기가 지면 끝내는 묶음**(moai-ncay) — `집기 || { …; exit 1; }` 와 `if ! 집기; then exit; fi` 다.
    // 나올 때 그 꼴이면 집기가 이긴 채로 잇는다 — 그 묶음이 돌았으면 뒤는 아예 안 돈다.
    let mut bailout: Option<Bailout> = None;
    // 집기로 센 토막의 번호와 그 묶음 깊이, 그리고 **그 집기가 확실히 도는가**. 몸통이 안 돌았을 수
    // 있는 묶음을 나오면 그 안의 집기는 쥔 것이 없으니 함께 걷고, 안 돌 수도 있는 자리의 집기는 적되
    // 확실하지 않다고 적는다(moai-hze6·moai-dbzs). **그 자리를 정하는 자는 `certain` 하나고**
    // [`Picks::fold`] 가 그 답을 읽는다 — 목록을 여기 베껴 적지 않는다.
    let mut picked: Vec<(usize, usize, bool)> = Vec::new();
    // 겹마다 **그 겹에 처음 든 토막의 번호** — 묶음을 나올 때 **그 묶음 안에서** 적은 집기만 걷는다.
    // 깊이만 보던 판은 같은 깊이의 **앞선 형제** 묶음까지 함께 버렸다 — `(집기 A); if …; then 집기 B;
    // fi` 가 늘 도는 A 까지 잃었다. `seg.floor` 로 걷는 것은 옆의 `strict`·`sure` 와 같은 자다.
    let mut enters: Vec<usize> = Vec::new();
    // 걷을 자리를 한 곳에 둔다 — 두 벌로 적던 자리마다 한쪽만 고쳐졌다(moai-ju21.70g·moai-ncay).
    // `from` 보다 앞서 적힌 집기는 그 까닭이 서기 전에 이미 돈 것이라 남긴다.
    /// **세우는 자리도 한 곳에 둔다**(리뷰 moai-k8j1.udq) — `prune` 의 짝이다. 끝내는 묶음을 푼
    /// 자리 셋이 저마다 꼬리(`true`)를 손으로 적던 판은, 꼴이 바뀌면 둘만 고쳐질 자리였다.
    fn credit(picked: &mut Vec<(usize, usize, bool)>, b: Option<&Bailout>, home: usize) {
        if let Some(c) = b.and_then(|b| b.cond) {
            picked.push((c.at, home, c.sure));
        }
    }
    fn prune(picked: &mut Vec<(usize, usize, bool)>, from: usize, deeper_than: usize) {
        picked.retain(|(n, at, _)| *n < from || *at <= deeper_than);
    }
    let (segs, over) = parse_over(cmd);
    for mut seg in segs {
        // **몸통이 안 돌았을 수 있는 묶음을 나오면 그 안의 집기는 끝난다**(리뷰 moai-ju21.70g) —
        // `fi`·`esac`·`done` 의 묶음은 몸통이 안 돌아도 0 이고, `a || { mv; }` 는 `a` 가 이기면 안 돈다.
        // `if false; then mv; fi && sed -i …` 의 `sed` 는 집기 없이 돈다. **치환에 들어서기 전에 본다** —
        // 다음 명령의 치환 토막이 먼저 쌓여 이 표식을 받는다.
        //
        // **기록은 `after_pick` 과 따로 걷는다** — 그쪽은 "지금 집은 채인가" 고 이쪽은 "그 집기가
        // 돌았는가" 라, `if false; then 집기; echo x; fi` 처럼 몸통 안의 `;` 가 사슬을 이미 끊은 판에서
        // 갈린다. 그때 `after_pick` 은 비어 이 블록이 통째로 안 돌았고, 기록만 남아 쓰기 규칙과
        // 어긋났다.
        //
        // **그 안에서 선 "이긴 집기" 도 함께 걷는다**(리뷰 moai-k8j1.034) — 몸통이 안 돌았으면 그
        // 안의 `|| exit` 도 안 돌았다. `sure`·`sure_e` 는 아래 `seg.floor` 로 걷히는데 그 자는 겹을
        // 나오는 자리(`frames`)보다 **뒤**에 서서, `bash -c 'if false; then 집기 || exit 1; fi'
        // && 쓰기` 가 안 돈 집기를 겹 밖으로 들고 나가 빈손으로 샜다.
        if let Some(l) = seg.shut {
            if after_pick.is_some_and(|d| d > l) {
                after_pick = None;
            }
            for won in [&mut sure, &mut sure_e] {
                if won.is_some_and(|d| d > l) {
                    *won = None;
                }
            }
            // **안 돈 몸통 안에서 열린 끝내는 묶음도 걷는다**(moai-4arw) — `if false; then 집기 ||
            // { exit 1; }; fi` 의 `exit` 는 안 돈다. 닫힌 묶음 **바로 밑**(`l + 1`)에 선 것은 그
            // 묶음 자신이라 남긴다: `if ! 집기; then exit 1; fi` 의 묶음이 그 자리다.
            bailout.take_if(|b| b.floor > l + 1);
            prune(&mut picked, enters.get(l + 1).copied().unwrap_or(0), l);
        }
        // **그 묶음 안의 겹에서 선 것도 함께 버린다**(리뷰 moai-k8j1.034) — 안 돈 묶음 안의
        // `bash -c '집기 || exit 1'` 은 집기를 돌린 적이 없다. 아래 겹을 나오는 자리가 그것을
        // 집기로 옮겨 적어, `make build || bash -c '집기 || exit 1' && 쓰기` 가 아무것도 안 집은 채
        // 샜다.
        let mut unrun = None;
        if let Some((c, from)) = orelse
            && seg.floor < c
        {
            if after_pick.is_some_and(|d| d >= c) {
                after_pick = None;
            }
            // 같은 까닭으로 `a || { 집기 || { exit 1; }; }` 의 안쪽 묶음도 걷는다 — `a` 가 이기면
            // 그 `exit` 는 안 돈다.
            bailout.take_if(|b| b.floor >= c);
            prune(&mut picked, from, c.saturating_sub(1));
            (orelse, unrun) = (None, Some(c));
        }
        // 나온 겹을 걷고 든 겹을 연다. 같은 깊이라도 종류가 다르면 딴 겹이다(heredoc 치환 뒤의 `bash -c`
        // 글) — 그래서 겹치는 앞머리를 한 번 세고 그 뒤를 걷는다. 걷을 때마다 앞머리를 다시 훑던 판은
        // 깊이의 제곱을 썼고([`Lexer::DEEP`] 이 예순넷이다), 빈 스택을 막는 줄이 닿지 않는 자리에 섰다.
        let kept = frames.iter().zip(&seg.nested).take_while(|(f, l)| f.layer == **l).count();
        for f in frames.drain(kept..).rev() {
            match f.layer {
                Layer::Subst(_) => after_pick = f.held,
                // **새 셸 안에서 집기가 이긴 채로 끝났으면 그 셸의 값도 이긴 것이다**(moai-9xbq) —
                // `bash -c '집기 || exit 1; echo done'` 은 집기가 지면 거기서 끝나, 여기 온 것은
                // 이겼다는 뜻이다. 그 안의 `sure` 는 아래 자로 걷히니 나올 때 집기로 옮겨 적는다.
                // `.or` 로 묶던 자리다 — 그것은 "둘 중 하나가 이 자리보다 깊은가" 가 아니라 "먼저
                // 선 쪽" 이라, `sure` 가 얕게 서 있으면 깊이 선 `sure_e` 를 가린다. `max` 는 둘 중
                // 깊은 쪽을 본다.
                // **안 풀린 채 끝난 끝내는 묶음도 그 셸이 무엇으로 끝났는가로 푼다**(moai-4arw).
                // 그 묶음의 모든 길이 `exit <0 아닌 값>` 이면, 자식 셸이 0 으로 끝나 바깥의 `&&` 에
                // 닿은 것은 집기가 이겼다는 뜻이다 — 묶음을 나오는 토막이 글 안에 없어 위의 푸는
                // 자리가 안 돌 뿐이다. 기록도 함께 세운다(moai-m5mg) — 쓰기와 기록이 두 자리로
                // 갈리지 않는다.
                //
                // **전제 넷을 한꺼번에 세운다**(moai-54pk 에서 한 번 되돌린 자리다, 리뷰
                // moai-k8j1.209). 조각조각 더하다 구멍을 넷 냈다.
                // - 그 `exit` 가 **0 아닌 값**을 댈 것([`Bailout::hard`]) — 겹 안의 `exit 0` 은
                //   자식을 0 으로 끝내 집기가 져도 바깥 `&&` 에 닿는다
                // - 그 묶음이 **정말 돌았을** 것 — 안 돈 몸통 안에서 열린 묶음은 위의 `shut`·
                //   `orelse` 가 걷고, 몸통이 겹문인 `then` 가지는 `hard` 가 함께 걷는다
                // - **하위 셸의 `exit`** 가 아닐 것(`b.depth <= deep`) — `bash -c '( 집기 ||
                //   { exit 1; } ); echo d'` 의 괄호는 제 안만 끝내고 자식 셸은 0 이다. 재는 자는
                //   **겹이 들고 있는** 깊이다([`Layer::Shell`] 의 `deep`) — 겹에 들어선 토막에서
                //   베껴 오던 판은 글이 `( … )` 로 시작하면 그 깊이를 부풀렸다(리뷰 6번)
                // - 글이 **뒤로 띄운 것으로 끝나지 않을** 것 — 위의 `apace` 가지가 먼저 받는다
                //   (moai-99df 가 렉서에 그 표를 놓았다)
                //
                // **뒤로 띄운 것으로 끝나는 글의 값은 집기의 값이 아니다** — 치환처럼 들어설 때의
                // 집기를 도로 세운다(moai-54pk). errexit 는 그 겹 안에서 이미 제 몫을 했다.
                Layer::Shell { apace: true, .. } => after_pick = f.held,
                Layer::Shell { top, deep, .. }
                    if unrun.is_none_or(|c| top < c)
                        && (sure.max(sure_e).is_some_and(|l| l >= top)
                            || bailout.as_ref().is_some_and(|b| b.resolves(top, deep, iffy))) =>
                {
                    let home = top.saturating_sub(1);
                    after_pick = Some(after_pick.map_or(home, |d| d.min(home)));
                    credit(&mut picked, bailout.as_ref().filter(|b| b.resolves(top, deep, iffy)), home);
                }
                Layer::Shell { .. } => {}
            }
            (strict, lone, sure, sure_e, bailout) = (f.strict, f.lone, f.sure, f.sure_e, f.bailout);
        }
        // **집기가 지면 끝내는 묶음을 나왔다**(moai-ncay) — 그 안의 모든 길이 `exit` 면 여기 온 것은
        // 집기가 이겼다는 뜻이다. 들어설 때의 집기를 도로 세운다.
        //
        // **나온 것은 `floor` 가 아니라 `seg.floor` 로 본다** — 형제 묶음이 같은 깊이에서 곧바로
        // 열리면(`… || { exit 1; }; for f in …; do 쓰기; done`) 그 안의 토막은 제 깊이가 `floor` 와
        // 같거나 깊어, 묶음을 나온 적이 없는 것으로 읽혔다. 옆의 `strict`·`sure` 가 `seg.floor` 를
        // 쓰는 것과 같은 까닭이다(리뷰 moai-p836.rv).
        //
        // **하위 셸을 나온 것이면 안 세운다** — `집기 || ( …; exit 1 )` 의 `exit` 는 그 괄호만 끝내고
        // 바깥 셸은 살아 있다. 깊이를 안 보던 판은 그 줄의 뒤 쓰기를 집기 뒤로 읽어 샜다.
        if let Some(b) = &bailout
            && seg.floor < b.floor
        {
            if b.ends && seg.depth >= b.depth {
                // 여기 온 것은 집기가 이겼다는 뜻이다 — 이 묶음에서는 이제 이긴 채다(`sure`).
                // `after_pick` 만 세우면 바로 뒤의 `;` 가 그것을 도로 끊는다. 세우는 깊이는 그 묶음을
                // 감싼 목록의 것이다 — 지금 토막의 깊이로 세우면 `( 집기 || { exit 1; } ); 쓰기` 처럼
                // 하위 셸 안의 집기가 괄호 밖까지 번진다.
                let home = b.floor.saturating_sub(1);
                after_pick = b.held.or(Some(home));
                sure = Some(sure.map_or(home, |l| l.min(home)));
                // **기록도 함께 세운다**(moai-m5mg) — `if ! 집기; then exit 1; fi` 의 조건은 뒤집혀
                // 있어 위에서 안 적혔지만, 여기 온 것은 그 집기가 이겼다는 뜻이다. 쓰기 규칙만 세우던
                // 판은 같은 줄을 "쓰기에는 집은 채, 기록에는 빈손" 으로 갈라 놨다.
                credit(&mut picked, Some(b), home);
            }
            bailout = None;
        }
        // **나온 묶음의 것은 걷는다** — 깊이가 같아도 형제 괄호와 다시 든 `if` 는 딴 묶음이다
        // ([`Seg::low`]·[`Seg::floor`]).
        if moved.is_some_and(|d| seg.low < d) {
            moved = None;
        }
        for scope in [&mut strict, &mut lone, &mut sure, &mut sure_e, &mut negated_at, &mut iffy, &mut chancy] {
            if scope.is_some_and(|l| seg.floor < l) {
                *scope = None;
            }
        }
        // 이 토막의 이음사는 **든 겹 바깥**의 것이다 — 첫 토막이 바깥 이음사를 받는다([`Lexer::relex`]).
        // 그래서 아래 `set -e` 의 셈은 **겹을 열기 전의** 판과 그 판의 맨 윗자리로 한다. 겹을 열었을
        // 때만 이 값을 쓰고 아니면 그 자리에서 다시 재던 판은 같은 값을 두 갈래로 적어, 사이에
        // `strict` 를 건드리는 줄이 하나 들면 한쪽만 조용히 달라졌다.
        let (was_strict, was_lone, top) = (strict, lone, errexit_top(&frames));
        // 여기서 여는 겹들 가운데 가장 바깥 것 — 바깥 셸에 대해 선 것은 그 겹에 적어야 나올 때 선다.
        let base = frames.len();
        for layer in &seg.nested[base..] {
            let held =
                Frame { layer: *layer, held: after_pick, strict, lone, sure, sure_e, bailout: bailout.take() };
            frames.push(held);
            // 새 셸은 바깥의 errexit 도, 바깥이 열어 둔 끝내는 묶음도 안 물려받는다.
            //
            // **이긴 집기(`sure`·`sure_e`)는 그대로 둔다** — 그것은 "여기 왔으니 집기가 이겼다" 는
            // 사실이라 새 셸 안에서도, 치환 안에서도 참이다. 새 셸에서 걷던 판은 `집기 || exit 1;
            // bash -c '쓰기'` 를 막았다(리뷰 moai-k8j1.034). 안쪽에서 선 것이 밖으로 새지 않는 것은
            // 위의 틀이 나올 때 되돌려 주기 때문이고, 이 겹보다 얕게 선 것은 아래 나올 때의
            // `l >= base` 가 가른다.
            (strict, lone) = (None, None);
            if let Layer::Shell { top, strict: on, .. } = *layer {
                // 띄울 때 켠 errexit 는 그 글의 첫 줄부터 선다(moai-j9tx).
                strict = on.then_some(top);
            }
        }
        let words = command_of(&seg.words);
        let head = words.first().map(|w| basename(w));
        let prefix = &seg.words[..seg.words.len() - words.len()];
        // 제 접두어가 이 토막을 뒤집는가 — 아래 세 자리가 같은 것을 묻는다.
        let bang = prefix.iter().any(|w| w == "!");
        // **이 토막이 제 셸을 끝내는가** — `exit` 은 붙박이라 경로로 오지 않는다(`basename` 을 안 쓴다).
        // `&` 로 띄우거나 파이프의 칸이면 그 하위 셸만 끝난다. 셋이 따로 세던 것을 여기 하나로 모았다 —
        // 한쪽만 `basename` 을 써서 `/bin/exit` 를 끝내는 줄로 읽던 자리다.
        // **뒤로 띄운 것 안의 `exit` 는 껍데기를 안 끝낸다**(moai-99df) — `{ …; exit 1; } &` 는 제
        // 하위 셸만 끝내고 바깥은 그대로 돈다. `sub` 만 보던 판은 묶음을 띄운 `&` 를 못 봤다.
        let quits = !seg.sub && !seg.bg && words.first().map(String::as_str) == Some("exit");
        let mut j = seg.join;
        // `||` 바로 뒤의 토막 — 앞이 져야 돈다. 묶음 밖에서 읽은 `||`(`a || { … }`)는 묶음 안의 첫
        // 명령과 따로 본다. 그 앞이 집기 목록이었는지도 적어 둔다 — `mv A || mv B && …` 는 어느 쪽이든
        // 하나를 쥐지만, `a || mv B && …` 는 `a` 가 이기면 빈손이다.
        let or = j.op == Op::Or && j.depth == seg.level;
        let picked_before = after_pick.is_some();
        // **조건에 매여 묶음에 들어섰다** — 앞이 지거나(`&&`) 이기면(`||`) 이 묶음은 안 돈다. 쥔 것이
        // 있으면 그 조건은 집기 자신이라(`집기 && { … }`) 세지 않는다 — 묶음이 돌았다는 것이 곧
        // 집기가 이겼다는 뜻이다.
        if matches!(j.op, Op::Or | Op::And) && j.depth < seg.level {
            chancy = Some(chancy.map_or(seg.level, |l| l.min(seg.level)));
            if !picked_before {
                iffy = Some(iffy.map_or(seg.level, |l| l.min(seg.level)));
            }
        }
        // **함수 몸통도 안 돈다** — `f() { … }` 는 정의일 뿐이고 그 값은 늘 0 이다. 렉서는 머리를
        // `f(` 한 낱말로 내니(붙은 괄호가 낱말을 안 가른다), 그 **다음** 토막이 몸통의 첫 줄이다.
        // 머리에서 바로 적으면 안 된다 — 몸통의 첫 줄은 머리 자리를 지나 내려오니(`seg.floor`)
        // 위의 걷기가 그것을 곧바로 지운다.
        if let Some(at) = fndef.take()
            && seg.level > at
        {
            // 둘을 한 자리에서 세운다 — 함수 몸통은 쓰기 규칙에도 기록에도 똑같이 "안 돈다" 라,
            // 따로 적으면 한쪽만 고쳐지는 날 갈린다. 걷는 자리도 한 벌이다(위의 `scope` 고리).
            for slot in [&mut iffy, &mut chancy] {
                *slot = Some(slot.map_or(seg.level, |l| l.min(seg.level)));
            }
        }
        if seg.words.last().is_some_and(|w| w.ends_with('(')) {
            fndef = Some(seg.level);
        }
        if j.op == Op::Or && j.depth < seg.level && !picked_before {
            // 번호는 **이 토막**의 것이다 — 여기부터가 "앞이 이기면 안 도는" 자리고, 그 앞에 적힌
            // 집기(먼저 쌓인 치환의 것을 포함해)는 이미 돌았다.
            orelse = Some(orelse.map_or((j.depth + 1, k), |(c, m)| (c.min(j.depth + 1), m.min(k))));
        }
        // `|| exit 1` 자신은 집기를 안 끊는다 — 집기가 이겼으면 안 돌고, 졌으면 껍데기가 끝난다. 제 셸을
        // 끝낼 때만이다: `&` 로 띄우거나 파이프의 칸이면 그 하위 셸만 끝나고, 함수 밖의 `return` 은 bash 가
        // 꾸짖고 다음 명령으로 간다(리뷰 moai-ju21.70g).
        let bail = or && quits;
        if bail {
            j.op = Op::And;
        }
        // `set -e` 아래 홀로 선 명령 뒤의 `;` — 그 명령이 졌으면 여기 못 온다. 거기까지 집기 사슬이 이어
        // 왔으면 이제 이긴 채다 — 뒤의 파이프·목록이 사슬을 끊어도 집기는 그대로다. **맨 바깥에서만이다**:
        // 묶음이 `&&`·`||` 목록의 앞 칸이면(`{ mv; sed; } || true`) bash 는 그 안의 errexit 를 안 보는데,
        // 그것은 묶음을 닫은 뒤에야 안다(리뷰 moai-ju21.70g).
        //
        // **맨 바깥은 그 셸의 맨 윗자리다**(moai-tmi0) — `bash -c` 의 글은 제 셸의 맨 윗자리에서 센다.
        // 치환 안은 세지 않는다: 바깥 목록이 그 안의 errexit 를 끌 수 있는데(`x=$( … ) || true`), 바깥
        // 토막은 치환 뒤에 온다.
        if j.op == Op::Any && was_strict.is_some() && top.is_some_and(|t| j.depth == t && was_lone == Some(t)) {
            j.op = Op::And;
            if after_pick.is_some() {
                sure_e = top;
                // **이 셈은 바깥 셸의 것이다** — 겹을 열며 섰으면 그 겹을 나올 때도 서 있어야 한다.
                // 지금 값만 세우던 판은 겹을 나오며 들어설 때의 것으로 되돌려, `set -e; 집기;
                // bash -c '…' | cat; 쓰기` 의 이긴 집기를 잃고 시킨 대로 친 줄을 막았다.
                if let Some(f) = frames.get_mut(base) {
                    f.sure_e = top;
                }
            }
        }
        // **이 토막이 확실히 도는가**(moai-dbzs, 사용자 결정 2026-09-20) — 기록에 `?` 가 안 붙는
        // 자리다([`Picks::line`]). 앞선 이음사에 매이지도, 조건에 매여 들어선 묶음 안(`chancy`)도,
        // 머리가 안 돌 수 있는 파이프라인의 칸(`piped`)도 아니다. `||` 만 재던 판은 `cargo test &&
        // moai mv X review` 를 확실한 집기로 적어, 앞이 지면 안 도는 줄을 정말 쥔 세션에게서
        // 빼앗았다(리뷰 moai-51h9.q5l 의 7번). 그만큼 넘겨받기(moai-4jsy)가 약해지는 것은 받아들인
        // 대가다 — `git pull && moai mv X in_progress` 도 이제 `?` 로 적힌다.
        //
        // **고쳐 적은 `j.op` 로 잰다 — 고치기 전의 것이 아니다**(리뷰 moai-51h9.3jh). 바로 위에서
        // `set -e` 아래 홀로 선 명령 뒤의 `;` 는 `Op::And` 로 다시 적히는데, 그 앞에서 재던 판은
        // `set -e; cargo test; moai mv X review` 를 확실한 집기로 적었다 — 껍데기는 `cargo test` 가
        // 지면 거기서 끝나 집기가 안 도는데, 같은 뜻을 `&&` 로 쓴 줄만 `?` 였다. 쓰기 규칙이 이미
        // 그 자리를 `&&` 로 읽으니 기록도 같은 답을 받는다(두 자리가 안 갈린다, moai-m5mg). 대가:
        // 앞이 `set -e` 뿐인 `set -e; moai mv X in_progress` 도 `?` 다 — 앞 명령이 무엇인지는 안 보기
        // 때문이고, `?` 도 남이 적은 것이 없으면 제 것이다([`Picks::fold`]).
        let certain = match j.op {
            // 파이프의 칸은 제 머리의 답을 잇는다 — `|` 는 `&&` 보다 단단히 묶여 `cargo test &&
            // echo x | moai mv X in_progress` 의 집기도 `cargo test` 가 져야 안 돈다. 제 이음사만
            // 보던 판은 그것을 확실한 집기로 적었고, `a || echo x | moai mv X …` 는 moai-hze6 이
            // 닫으려던 `||` 자리마저 샜다. 머리를 못 찾으면 모르는 쪽으로 선다.
            Op::Pipe => piped.get(j.depth).copied().unwrap_or(false),
            _ => !matches!(j.op, Op::And | Op::Or) && chancy.is_none(),
        };
        // 이음사가 제 깊이보다 깊으면 묶음을 막 나온 토막 — `( … ) > f` 의 `> f` 다. 그 묶음에
        // 들어설 때의 판으로 쓰고, 묶음의 값은 그대로 뒤로 흐른다. 이음사로 읽던 판은 안쪽 `;` 로
        // 집기를 끊어 `(mv) > /dev/null && sed -i …` 를 막았다.
        //
        // 셸에 넘긴 글을 낸 토막도 이 자리다([`Lexer::relex`]) — 그 글의 값이 이 토막의 값이다. 다만 그
        // 토막이 부정이면(`! bash -c '집기' && …`) 집기가 져야 뒤가 도니, 들어설 때의 판으로 돌린다.
        //
        // **넘긴 글에서 나온 집기** — 부정이 아래에서 이것을 지우기 전에 집어 둔다. 글의 값이 이
        // 토막의 값이니, 여기 선 것은 "그 글이 집기로 끝났다" 는 뜻이다(아래 `if ! bash -c '집기'`).
        let came = after_pick;
        let gate = if j.depth > seg.level {
            let entry = heads.get(seg.level).copied().flatten();
            if bang {
                after_pick = entry;
            }
            entry
        } else {
            after_pick = match j.op {
                Op::Pipe => heads.get(j.depth).copied().flatten(),
                _ => after_pick.and_then(|d| match j {
                    j if j.depth > d => Some(d),
                    j if j.op == Op::And => Some(j.depth),
                    _ => None,
                }),
            };
            // 이 토막이 여는 파이프라인들 — 이음사의 깊이부터(파이프면 그 다음부터) 제 깊이까지.
            heads.truncate(j.depth + usize::from(j.op == Op::Pipe));
            heads.resize(seg.level + 1, after_pick);
            // 기록 쪽도 같은 자로 잇는다 — 파이프의 칸은 제 머리가 확실히 도는가를 그대로 받는다.
            piped.truncate(j.depth + usize::from(j.op == Op::Pipe));
            piped.resize(seg.level + 1, certain);
            after_pick
        };
        let gate = gate.or(sure).or(sure_e);
        let n = k;
        if !seg.words.is_empty() {
            k += 1;
        }
        // **이 토막이 막 나온 한 겹 깊은 자리에 처음 든 토막의 번호** — 셸에 넘긴 글의 토막들은
        // 그 글을 낸 토막 바로 앞에 한 겹 깊게 심긴다([`Lexer::relex`]). 아래 `if ! bash -c '집기'`
        // 가 "그 집기가 **이 글 안의** 것인가" 를 이것으로 가른다. 자르기 전에 집는다.
        let handed_from = enters.get(seg.level + 1).copied();
        // 이 토막이 여는 묶음마다 제 번호를 적어 둔다([`enters`]) — 나올 때 그 안의 집기만 걷는다.
        // `seg.floor` 까지 걷고 다시 쌓으므로, 같은 깊이에 곧바로 열린 형제 묶음은 새 번호를 받는다.
        enters.truncate(seg.floor + 1);
        while enters.len() <= seg.level {
            enters.push(n);
        }
        // `[[ a > b ]]`·`(( a > b ))` 의 `>` 는 비교다 — 낱말을 가를 때 이미 걸렀다.
        let mut found = std::mem::take(&mut seg.writes);
        match head {
            Some("sed") => found.extend(sed_in_place(&words[1..])),
            Some("tee") => found.extend(words[1..].iter().filter(|w| !w.starts_with('-')).cloned()),
            _ => {}
        }
        for path in found {
            if gate.is_some() || unknowable(&path) || (moved.is_some() && !Path::new(&path).is_absolute()) {
                continue;
            }
            out.push(path);
        }
        // **하위 셸의 `cd` 는 바깥 자리를 안 옮긴다**(리뷰 moai-k8j1.udq 의 6번) — 파이프의 칸이나
        // 뒤로 띄운 것은 제 셸에서 돌고 끝난다. 여기만 그 둘을 안 보던 판은 `{ cd /tmp; } & sed -i
        // src/x.rs` 의 뒤 쓰기를 "어디인지 모른다" 며 버려, 집기 없는 쓰기가 샜다. 괄호는 `seg.low`
        // 가 나올 때 걷어 주지만 `{ … } &` 에는 괄호가 없다. `aimed` 는 이미 `sub` 를 읽는다
        // (idea moai-9abk 가 그 어긋남을 적어 둔 자리다).
        if matches!(head, Some("cd" | "pushd" | "popd")) && !seg.sub && !seg.bg {
            moved = Some(moved.map_or(seg.depth, |d| d.min(seg.depth)));
        }
        // **`! ( moai mv … )` 의 `!` 는 괄호 밖에 선다**(moai-gtkn). 제 토막의 접두어만 보던 판은
        // 그 부정을 못 봐, 집기가 져야 도는 쓰기를 집기 뒤로 읽었다 — 새는 쪽이다. 그 묶음을 나오면
        // 위에서 걷었다.
        let negated = bang || negated_at.is_some();
        // `!` 가 뒤에 여는 괄호, 또는 `! { …`·`! if …` 처럼 제 토막에서 연 묶음 — 그 묶음 전체가 부정이다.
        if let Some(at) = prefix.iter().position(|w| w == "!") {
            if words.is_empty() {
                negated_at = Some(seg.level + 1);
            } else if prefix[at..].iter().any(|w| OPENS.contains(&w.as_str())) {
                negated_at = Some(negated_at.map_or(seg.level, |l| l.min(seg.level)));
            }
        }
        if picks_up(&seg.words, cfg) && only(n) && !negated && (!or || picked_before) {
            after_pick = Some(after_pick.map_or(seg.level, |d| d.min(seg.level)));
            // 안 돌 수도 있는 자리의 집기는 적되 확실하지 않다고 적는다 — 그 자리를 재는 자는
            // `certain` 하나다(moai-hze6·moai-dbzs).
            picked.push((n, seg.level, certain));
        }
        // **집기가 지면 끝내는 묶음이 여기서 열리는가**(moai-ncay) — 두 꼴이고, 먼저 열린 하나만
        // 든다. `|| exit` 한 꼴만 알던 판은 겨루다 진 쪽을 끊는 이 흔한 두 꼴에서 집기를 잃어,
        // 시킨 대로 쓴 줄을 막았다.
        // **끝내는 묶음은 그것을 연 낱말을 읽은 셸의 것이다**(moai-9xbq, 리뷰 moai-k8j1.034). 이 토막이
        // 겹을 열었으면(`base`) `||` 는 겹 **밖**에서 읽은 것이라 그 겹의 틀에 적어야 나올 때 서고,
        // `if`·`!` 는 이 글 **안**의 낱말이라 이 셸의 것이다. 겹 안에 적던 판은 `집기 || { bash -c '…';
        // exit 1; }; 쓰기` 가 집기를 잃었고, 둘 다 바깥에 적던 판은 `bash -c 'if ! 집기; then exit 1;
        // fi; 쓰기'` 의 묶음을 겹 안에서 안 보이게 두어 `then exit 1` 이 `ends` 를 못 세웠다.
        //
        // 묶음(과 그것이 바깥 셸의 것인가)을 먼저 고르고, 자리는 한 곳에서 한 번만 고른다 — 읽는
        // 자리와 적는 자리로 갈라 두면 한쪽만 고쳐지는 날 묶음이 조용히 딴 데 서거나 사라진다.
        let mut opened: Option<(Bailout, bool)> = None;
        // **`집기 || { …; exit 1; }`** — 집기 뒤에 `||` 로 연 묶음이다.
        if j.op == Op::Or && j.depth < seg.level && picked_before {
            // **묶음의 자리는 이음사가 댄다**(moai-9xbq) — 그 묶음의 첫 명령이 셸에 글을 넘기면
            // ([`Lexer::relex`]) 심은 토막이 한 겹 더 깊은 자리로 서서, 정작 그 묶음의 `exit` 가
            // 묶음 **밖**으로 읽혔다. 그러면 나올 때 아무것도 안 세워 `집기 || { bash -c '…';
            // exit 1; }; 쓰기` 를 막았다. 깊이도 **이 토막이 여는** 겹만큼 얕다 — 겹마다 하위 셸
            // 하나다. 이미 든 겹(`base`)까지 빼던 판은 그 깊이를 이 셸보다 얕게 적어, `bash -c
            // '집기 || { echo a; exit 1; }; 쓰기'` 의 `exit` 가 제 묶음을 못 닫았다(리뷰 moai-k8j1.034).
            let opens = seg.nested.len().saturating_sub(base);
            let (floor, depth) = (j.depth + 1, seg.depth.saturating_sub(opens));
            opened = Some((Bailout { floor, depth, held: after_pick, cond: None, ends: false, hard: false }, true));
        // **`if ! 집기; then exit 1; fi`** — 조건이 부정된 집기고 몸통이 `exit` 뿐이다. 조건은
        // 집기로 안 세지만(`negated`), 그 묶음을 지나온 것은 집기가 이겼다는 뜻이다.
        //
        // **조건이 셸에 넘긴 글이어도 같다**(moai-9xbq) — `if ! bash -c '집기'; then exit 1; fi` 의
        // 집기는 그 글 안에 선다. 제 토막의 낱말만 보던 판은 거기서 집기를 못 봐 묶음을 안 열었고,
        // `fi` 를 지나며 집기를 잃어 뒤의 쓰기를 막았다.
        } else if prefix.iter().any(|w| w == "if") && negated {
            // 제 낱말이 집기이거나, **그 글이 집기로 끝났다**(`came`). 둘을 가리는 것이 이 조건의
            // 값이 집기의 값인가다 — 깊이 든 집기를 아무거나 세던 판은 `if ! bash -c '집기; echo ok'`
            // 처럼 집기의 값이 조건에 안 닿는 줄과, `( ( 집기 ) ); if ! bash -c 'true'` 처럼 앞선 딴
            // 겹의 집기까지 이긴 것으로 읽어 빈손의 쓰기를 넘겼다(리뷰 moai-k8j1.034). 그 글의
            // 토막들은 이 토막 바로 앞에 심겼으니(`handed_from`) 그 번호부터 거슬러 센다.
            let own = (picks_up(&seg.words, cfg) && only(n)).then_some(Cond { at: n, sure: certain });
            let cond = own.or_else(|| {
                let from = handed_from.filter(|_| bang && came.is_some_and(|d| d > seg.level))?;
                // 넘긴 글 안의 집기는 제 표를 이미 달고 있다 — `if` 자신이 조건에 매였으면 그 위에
                // 한 번 더 매인다(`certain`). 둘 다 서야 확실한 집기다.
                let &(c, _, sure) = picked.iter().rev().take_while(|(m, ..)| *m >= from).find(|(_, at, _)| *at > seg.level)?;
                shell_text(&seg.words).map(|_| Cond { at: c, sure: sure && certain })
            });
            if let Some(c) = cond {
                let b =
                    Bailout { floor: seg.level, depth: seg.depth, held: after_pick, cond: Some(c), ends: true, hard: true };
                opened = Some((b, false));
            }
        }
        if let Some((b, outer)) = opened {
            let slot = match frames.get_mut(base).filter(|_| outer) {
                Some(f) => &mut f.bailout,
                None => &mut bailout,
            };
            // 먼저 열린 하나만 든다.
            if slot.is_none() {
                *slot = Some(b);
            }
        }
        // 묶음 안에서 본 것 — 마지막이 `exit` 여야(그리고 `if` 꼴은 몸통이 모두 `exit` 여야) 끝내는
        // 묶음이다. **`exit` 로 세는 것은 그 묶음의 제 줄뿐이다**(`own`) — 더 깊은 자리의 `exit` 는
        // 제 묶음(`( exit 1 )`·`if …; then exit 1; fi`)만 끝내고, 그 묶음의 값은 몸통이 안 돌면 0 이다.
        //
        // **깊은 줄을 그냥 지나치지 않는다**(리뷰 moai-4arw.r1) — `seg.level == b.floor` 만 보고
        // 걸러 내던 판은 몸통이 겹문인 줄(`then { echo lost; }`·`then if …; fi`·`then while …; done`)
        // 을 아예 안 읽어, `if` 꼴이 들고 시작한 `ends`·`hard` 의 참이 그대로 남았다. 그래서
        // `bash -c 'if ! 집기; then { echo lost; }; fi' && 쓰기` 가 — bash 로 재면 집기가 져도 0 으로
        // 끝나는 줄이 — 집기 없이 지나갔다. 읽을 수 없는 줄은 `exit` 가 아닌 것으로 센다.
        //
        // 겹 안의 줄은 여기 안 온다 — 겹에 들어설 때 `bailout` 은 틀로 옮겨진다(`bailout.take()`).
        // `집기 || { bash -c '…'; exit 1; }` 의 심긴 토막이 이 자리를 안 흔드는 것이 그 덕이다.
        if let Some(b) = &mut bailout
            && seg.level >= b.floor
            && seg.depth >= b.depth
            && !prefix.iter().any(|w| w == "if")
        {
            let own = seg.level == b.floor && seg.depth == b.depth;
            // **그 줄이 늘 도는 자리에 서야 한다** — `{ false && exit 1; }` 의 `exit` 는 앞이 이겨야
            // 돌아, 묶음이 0 으로 끝나고 뒤가 그대로 돈다. 머리만 보던 판은 그 줄을 샜다.
            let runs = seg.join.op == Op::Any || seg.join.depth < seg.level;
            let branch = prefix.iter().any(|w| w == "then" || w == "else" || w == "elif");
            let ends = own && quits && runs;
            // 0 아닌 값을 대는 `exit` 인가 — 겹을 나올 때만 쓴다([`Bailout::hard`]).
            let hard = ends && words.get(1).is_some_and(|a| a.parse::<i64>().is_ok_and(|n| n % 256 != 0));
            b.ends = if branch { b.ends && ends } else { ends };
            b.hard = if branch { b.hard && hard } else { hard };
        }
        // **홀로 선 명령인가** — 제 목록의 첫 칸이고(이음사가 `;`·줄바꿈이거나 묶음 밖에서 읽혔다) 파이프의
        // 칸도 `&` 로 띄운 것도 부정도 조건도 아니다. 뒤에 `&&`·`||` 가 붙으면 다음 토막이 그 이음사를
        // 들고 와 위에서 안 바뀐다. 다시 읽은 겹 안의 셈은 그 겹의 것이라 나오면 걷힌다 — 안 걷으면
        // `set -e; mv; echo "$(date)" > f` 의 바깥 토막이 치환 토막을 제 앞 명령으로 읽는다.
        let leads = seg.join.op == Op::Any || seg.join.depth < seg.level;
        let cond = prefix.iter().any(|w| matches!(w.as_str(), "if" | "elif" | "while" | "until"));
        lone = (leads && !seg.sub && !negated && !cond).then_some(seg.level);
        // `set -e`·`set -o errexit` 을 켜고 `set +e`·`set +o errexit` 가 끈다 — 껍데기가 그렇게 읽는다.
        // 파이프의 칸은 제 하위 셸만 바꾸고, `--`·`-` 뒤는 자리 인자다(`set -- -e`).
        if !seg.sub
            && let Some(("set", args)) = words.split_first().map(|(h, r)| (basename(h), r))
        {
            for (i, a) in args.iter().enumerate() {
                match a.as_str() {
                    "--" | "-" => break,
                    "-o" if args.get(i + 1).is_some_and(|v| v == "errexit") => strict = Some(seg.level),
                    "+o" if args.get(i + 1).is_some_and(|v| v == "errexit") => (strict, sure_e) = (None, None),
                    a if a.starts_with('-') && !a.starts_with("--") && a.contains('e') => strict = Some(seg.level),
                    a if a.starts_with('+') && a.contains('e') => (strict, sure_e) = (None, None),
                    _ => {}
                }
            }
        }
        // 집기 목록 바로 뒤의 `|| exit` 를 지났다 — 이 묶음에서는 이제 집기가 이긴 채다.
        if bail && after_pick.is_some() {
            sure = Some(sure.map_or(seg.level, |l| l.min(seg.level)));
        }
    }
    // 줄이 `fi` 로 끝나 위의 되세움이 안 돈 **집기가 지면 끝내는 묶음** — `if ! 집기; then exit 1; fi`
    // 는 여기 온 것 자체가 집기가 이겼다는 뜻이다. 쓰기는 뒤가 없어 셈이 같지만 기록은 남아야 한다.
    if let Some(b) = bailout.as_ref().filter(|b| b.ends) {
        credit(&mut picked, Some(b), b.floor.saturating_sub(1));
    }
    // 줄이 끝나도록 안 닫힌 묶음도 나온 것으로 친다 — `a || (moai mv …)` 와 `if …; then 집기; fi` 처럼
    // 뒤 토막이 없으면 위의 걷기가 안 돈다(예약어 묶음의 표식은 [`parse_over`] 가 낸다). 쓰기는 그 뒤가
    // 없어 셈이 같지만, 기록은 여기서 걷어야 안 돈 집기를 떠안지 않는다.
    if let Some(l) = over {
        prune(&mut picked, enters.get(l + 1).copied().unwrap_or(0), l);
    }
    if let Some((c, from)) = orelse {
        prune(&mut picked, from, c.saturating_sub(1));
    }
    (out, picked.into_iter().map(|(n, _, sure)| (n, sure)).collect())
}

/// 글자만으로는 **어디인지 모르는 경로** — 변수·틸드·글롭·프로세스 치환·`-`. 모르는 자리는
/// 지어내지 않는다.
fn unknowable(path: &str) -> bool {
    path.is_empty()
        || path == "-"
        || path.starts_with('~')
        || path.contains(['$', '`', '*', '?', '[', '(', ')', '{', '}'])
}

/// 만들기·닫기 규칙 — 트래커 하나에 대고 `only` 가 고른 `moai` 토막을 본다.
///
/// 다른 트래커를 가리키는 토막([`aimed`])은 **그 트래커의 줄로** 본다(moai-23ky). 세션 자리의
/// 줄로 보던 판은 남의 프로젝트에 세우는 줄을 제 초점으로 막았고, 남의 프로젝트가 쥔 초점은 못 봤다.
///
/// 막을 것이 없으면 담는 줄에 한 줄 비출 수 있다([`aside_in`], `Decision::Context`). **비추는
/// 것은 막는 것을 가리지 않는다** — 차례는 [`Decision::then`] 이 정한다.
pub fn guard_moai(
    issues: &[Issue],
    cfg: &Config,
    away: &Away,
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
    aim: Toward<'_>,
) -> Decision {
    // **`moai` 를 부르는 토막이 없으면 볼 것이 없다**(moai-xppm) — 세 규칙 모두 그 토막만 본다.
    // 초점을 먼저 짓던 판은 `cargo test` 하나에도 소속 지도를 지었다.
    if !segments(cmd).iter().enumerate().any(|(k, seg)| only(k) && moai_args(seg).is_some()) {
        return Decision::Pass;
    }
    // 초점은 한 번 잰다 — `held` 는 미룬 줄이 있으면 소속 지도를 다시 짓고, 훅은 도구 호출마다 돈다.
    let focus = held(issues, cfg, away);
    create_in(issues, &focus, cmd, only, aim)
        .then(|| close_in(issues, cfg, away, cmd, only, aim))
        .then(|| aside_in(issues, &focus, cmd, only, aim))
}

/// 집은 줄들의 에픽과, 에픽 없는 집은 줄 — **에픽 줄이 실제로 선 것만** 에픽이다. 집은 차례로, 에픽은
/// 겹치면 한 번. 끊긴 참조나 에픽 아닌 줄을 가리키는 `epic` 은 닫힐 에픽이 없고, 그 id 를 `-e` 로 대면
/// 시킨 대로 친 줄이 "에픽으로 쓸 수 없는 것을 가리키는 줄" 경고를 하나 늘려 `closing` 에 걸린다.
/// **[`create_in`] 의 거절문과 [`aside_in`] 의 비춤이 함께 쓴다** — 따로 세던 판은 거절문만 에픽
/// 아닌 줄을 `-e` 로 댔다(리뷰 moai-ju21.70g).
fn epics_of<'a>(issues: &'a [Issue], focus: &[&'a Issue]) -> (Vec<&'a str>, Vec<&'a str>) {
    let groups = report::groups(issues);
    let (mut epics, mut loose): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    for i in focus {
        match groups.get(i.id.as_str()).copied().filter(|e| issues.iter().any(|g| g.id == *e && g.kind == crate::model::Kind::Epic)) {
            Some(e) if !epics.contains(&e) => epics.push(e),
            Some(_) => {}
            None => loose.push(i.id.as_str()),
        }
    }
    (epics, loose)
}

/// 이 토막이 **생각을 담는가** — [`adds_idea`] 의 두 철자. 도움말은 아무것도 안 담는다.
fn sets_aside(seg: &[String]) -> bool {
    let Some(args) = moai_args(seg) else { return false };
    adds_idea(args, &positionals(args)) && !asks_help(args)
}

/// 규칙 1 의 옆짝 — **에픽 일을 집은 채 생각을 담으면 갈림길 1 의 둘째 물음을 비춘다**(moai-d4e0).
///
/// 둘째 물음은 `moai add` 의 거절문에만 실렸는데, 실제로 틀리는 자리는 말없이 지나가는
/// `idea add` 다 — moai-1k17 의 세션은 곧장 그쪽으로 갔다. **막지 않는다**: 에픽이 내건 것인지는
/// 훅이 못 가르고, idea 는 이 규칙에서 언제나 자유롭다.
///
/// **이 줄은 생각이 이미 담긴 뒤에 읽힌다.** `PreToolUse` 의 `additionalContext` 는 도구 결과 곁에
/// 붙는다 — 막지 않으니 명령은 돌고, 모델은 그 다음 요청에서 읽는다. 그래서 새로 세우라고 하지 않고
/// 담은 것을 되찾는 길(`idea promote -e`)을 댄다. `moai add -e` 를 대던 판은 시킨 대로 치면 같은
/// 것이 에픽 멤버와 담아 둔 생각으로 둘이 섰다. 담은 토막이 `-C` 로 다른 자리를 가리켰으면 그 자리도
/// 댄다 — 빼고 치면 되찾는 줄이 세션 자리의 트래커에서 헛돈다.
///
/// 에픽이 있는 집기만 본다 — 첫 칸에 둔 멤버가 일을 열어 두는 것은 에픽뿐이다([`create_in`]).
/// **에픽 줄이 실제로 선 것만** 댄다. 끊긴 참조나 에픽 아닌 줄을 가리키는 `epic` 은 닫힐 에픽이
/// 없고, 그 id 로 되찾으라고 하면 시킨 대로 친 줄이 경고를 하나 늘려 `closing` 에 걸린다. 차례는
/// 집은 차례다 — 규칙 1 의 거절문이 `focus[0]` 의 에픽을 대는 것과 같은 에픽을 앞에 둔다.
fn aside_in(issues: &[Issue], focus: &[&Issue], cmd: &str, only: &dyn Fn(usize) -> bool, aim: Toward<'_>) -> Decision {
    if focus.is_empty() {
        return Decision::Pass;
    }
    let Some((at, seg)) = segments(cmd).into_iter().enumerate().find(|(k, seg)| only(*k) && sets_aside(seg)) else {
        return Decision::Pass;
    };
    let (aims, _) = epics_of(issues, focus);
    let Some(first) = aims.first() else {
        return Decision::Pass;
    };
    let moai = echo_moai(aim(at), &seg);
    Decision::Context(format!(
        "갈림길 1 의 둘째 물음 — {} 가 내건 것이 방금 담은 생각 없이도 이뤄지는가. 아니면 idea 가 아니라 안 끝난 이 일이다.\n\
         그렇다면 지금 못 해도 `{moai} idea promote <그 id> -e {first} --from -` 로 그 에픽의 멤버로 되찾아 첫 칸에 \
         둔다 — 밖에 두면 에픽이 목적을 못 이룬 채 닫힌다.",
        aims.join("·")
    ))
}

/// 글을 적는 `moai` 인가 — 제목·본문·노트·`-m` 을 받는 동사. **읽기는 안 센다** — `show -g 한글` 도,
/// 네임스페이스 밑의 읽기(`idea ls -g 한글`·`epic show -g …`)도. 동사는 [`creates`] 와 같은 자로 가른다 —
/// 첫 낱말만 보던 판은 네임스페이스의 읽기에 "방금 넣은 글" 이라며 알림을 달았다.
fn writes_text(verbs: &[&str]) -> bool {
    match verbs.first().copied() {
        Some("add" | "edit" | "note" | "mv" | "defer") => true,
        Some("idea") => matches!(verbs.get(1).copied(), Some("add" | "promote")),
        Some("issue" | "epic" | "milestone") => verbs.get(1).copied() == Some("add"),
        _ => false,
    }
}

/// 값이 글이 아닌 플래그 — 자리(`-C`)·사람(`--user`·`-a`)·칸(`-s`·`--from`)·태그·소속·종류. 사람 이름도
/// 태그와 칸 이름도 한글일 수 있다 — 칸을 한글로 지은 보드에서는 `mv` 마다 헛 알림이 섰다.
const NOT_TEXT: &[&str] = &[
    "-C", "--dir", "--user", "-a", "--assignee", "-t", "--tag", "--untag", "-s", "--status", "--from", "-e", "--epic",
    "--milestone", "--parent", "-p", "--priority", "--type",
];

/// 꼴이 정해진 줄의 머리 — 읽는 쪽이 그 꼴로 세니 다듬지 않는다(`guide::KOREAN`).
const FIXED_HEADS: &[&str] = &["model:", "다음:", "Regression-of:"];

/// 이 명령줄이 **한국어 글을 moai 에 넣는가**(moai-6rrb) — 글을 적는 `moai` 토막의 글 인자에 한글이 들었거나,
/// 그 토막에 흘러드는 글([`Seg::fed`] — stdin 의 heredoc·here-string·파이프 앞 칸, 명령 치환 속 heredoc)에
/// 한글이 들었다. 넣으면 그 토막의 자리(`-C` 로 가리켰으면 ` -C <그곳>`, 아니면 빈 글)를 낸다 — 알림이 고쳐
/// 적는 명령에 옮겨 적는다([`aside_in`] 과 같은 까닭). `only` 는 볼 토막을 고른다([`segments`] 의 번호) — 받는
/// 쪽이 트래커가 없는 자리를 가리킨 토막을 뺀다. 그 `moai` 는 스스로 실패해 아무것도 안 넣는다.
///
/// **꼴이 정해진 줄만 있으면 아니다.** `model: …`·`다음: …`·`Regression-of: …` 는 다듬지 않는 글이라,
/// 거기 알림을 달면 닫을 때마다 헛 알림이 선다. 줄 머리에서 시작한 줄만 그 꼴이다 — `model::parse_work` 와
/// 같은 자다. 파일에서 흘린 본문(`< 파일`)은 모른다 — 훅이 남의 파일을 읽지 않는다. 판정은 **글의 글자**다 —
/// 화면 말 설정과는 무관하다(`guide::KOREAN`).
///
/// **흘러드는 글은 그 토막의 것만 본다.** 명령줄 전체에서 한글을 찾던 판은 옆 토막의 커밋 메시지·`--user`
/// 이름·경로에 알림을 달았고, `"$(cat <<'EOF' … EOF)"` 로 넣은 글은 렉서가 본문을 건너뛰어 놓쳤다.
pub fn korean_write(cmd: &str, only: &dyn Fn(usize) -> bool, aim: Toward<'_>) -> Option<String> {
    let hangul = |t: &str| t.chars().any(|c| matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}'));
    // 렉서는 받은 글자를 옮기기만 하니 명령줄에 한글이 없으면 볼 것이 없다 — 훅은 Bash 마다 돈다.
    if !hangul(cmd) {
        return None;
    }
    let fixed = |l: &str| {
        FIXED_HEADS.iter().any(|h| l.strip_prefix(h).is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace)))
    };
    let unfixed = |t: &str| t.lines().any(|l| hangul(l) && !fixed(l));
    // 번호는 [`segments`] 와 같게 센다 — `only` 가 그 번호로 고른다.
    let segs: Vec<Seg> = parse(cmd).into_iter().filter(|s| !s.words.is_empty()).collect();
    segs.iter().enumerate().find_map(|(k, seg)| {
        let args = moai_args(&seg.words).filter(|_| only(k))?;
        let verbs = positionals(args);
        // 도움말과 연습(`--dry-run`)은 아무것도 안 넣는다.
        if asks_help(args) || args.iter().any(|a| a == "--dry-run") || !writes_text(&verbs) {
            return None;
        }
        let texts = if matches!(verbs.first().copied(), Some("mv" | "defer")) {
            // 자리 인자는 id 와 갈 칸이다 — 글은 `-m` 하나다.
            flag_values(args, &["-m", "--msg"])
        } else {
            prose(args)
        };
        let stdin = flag_values(args, &["-b", "--body", "--from"]).iter().any(|v| v == "-");
        let substituted = texts.iter().any(|t| t.contains("$(") || t.contains('`'));
        let mut fed: Vec<&str> = Vec::new();
        if stdin || substituted {
            fed.extend(seg.fed.iter().map(String::as_str));
        }
        // 파이프의 앞 칸이 내는 글 — `printf '…' |`·`cat <<'B' |`. 다른 명령의 인자는 파일·필터라 글이 아니다.
        let mut j = k;
        while stdin && j > 0 && segs[j].join.op == Op::Pipe {
            j -= 1;
            let head = command_of(&segs[j].words);
            if head.first().is_some_and(|w| matches!(basename(w), "echo" | "printf")) {
                fed.extend(head[1..].iter().map(String::as_str));
            }
            fed.extend(segs[j].fed.iter().map(String::as_str));
        }
        if !texts.iter().map(String::as_str).chain(fed).any(unfixed) {
            return None;
        }
        // **겨눈 트래커를 댄다 — 친 글자가 아니다**(moai-j2vp, moai-v9sa 와 한 자). 옮겨 적던 판은
        // `cd src && moai -C .. note …` 에 `moai -C .. edit` 를, 워크트리 안의 `moai -C . note …` 에
        // `moai -C . edit` 를 내밀었다 — 앞엣것은 어디서 치느냐로 자리가 바뀌고, 뒤엣것은 v9sa 가
        // 닫은 바로 그 길(워크트리 스냅샷)이다. 같은 훅 한 판이 거절문과 다른 자리를 대면 안 된다.
        Some(aim_flag(aim(k), &seg.words))
    })
}

/// [`korean_write`] 를 토막 가림 없이 — 판정만 보는 시험이 쓴다.
#[cfg(test)]
fn writes_korean(cmd: &str) -> bool {
    korean_write(cmd, &|_| true, &|_| None).is_some()
}

/// 글을 적는 토막의 인자 가운데 **글인 것** — [`NOT_TEXT`] 의 플래그와 그 값을 뺀다. `--tag=파서` 와 짧은
/// 플래그에 붙여 쓴 `-t파서` 도 뺀다([`flag_values`] 가 받는 모양). `--` 뒤는 모두 자리 인자다.
fn prose(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.extend(it.cloned());
            break;
        }
        let attached = !a.starts_with("--")
            && NOT_TEXT.iter().any(|f| f.len() == 2 && a.len() > 2 && a.starts_with(f));
        if NOT_TEXT.contains(&a.as_str()) {
            it.next();
        } else if !attached && !a.split_once('=').is_some_and(|(f, _)| NOT_TEXT.contains(&f)) {
            out.push(a.clone());
        }
    }
    out
}

/// 한국어 글을 넣은 뒤 비추는 한 줄(moai-6rrb). **막지 않는다** — 글 스타일 검사를 게이트로 두지 않는다는
/// 결정(moai-mthy)을 지킨다. `PreToolUse` 의 비춤은 명령이 돈 뒤에 읽히니([`aside_in`]) "다듬었는가" 를
/// 묻고 고쳐 적는 길을 댄다. `at` 은 그 글을 넣은 토막의 자리([`korean_write`]) — 고쳐 적는 명령도 같은
/// 트래커를 겨눈다. `missing` 은 이 저장소에 안 깔린 플러그인 — 있으면 까는 길을 **사람에게** 청하라고
/// 한다. 에이전트가 제 손으로 깔지 않는다(사용자 결정, moai-5wk4).
///
/// **노트와 `-m` 은 고쳐 적으라고 하지 않는다.** 둘은 저널에만 쌓여 고칠 수 없다 — `moai note` 로 다시 적으라던
/// 판은 같은 글을 한 벌 더 영영 남기게 했다. 고칠 수 있는 제목·본문만 `moai edit` 를 대고, 나머지는 다음 글부터다.
pub fn korean_notice(at: &str, missing: &[&str]) -> Decision {
    let mut said = format!(
        "방금 moai 에 넣은 한국어 글을 다듬었는가 — `korean-skills:humanizer`, 20줄을 넘으면 \
         `humanize-korean:humanize-korean`, 마지막에 `korean-skills:grammar-checker`. 안 다듬은 제목·본문은 \
         다듬어 `moai{at} edit` 로 고쳐 적는다. 노트와 `-m` 은 저널에만 쌓여 고칠 수 없으니 같은 글을 다시 적지 \
         말고 다음 글부터 넣기 전에 다듬는다. id·명령·경로·수·코드 조각과 꼴이 정해진 줄은 그대로 둔다."
    );
    if !missing.is_empty() {
        said.push_str(&format!(
            "\n이 저장소에 {} 이 깔려 있지 않다 — 제 손으로 깔지 말고 사람에게 `moai skill install` 을 다시 \
             불러 달라고 청한다. 그것이 moai 와 같은 범위로 함께 깐다. 그 명령이 `건너뛰었다` 를 내면 같은 \
             이름의 마켓플레이스가 남의 저장소를 가리키는 것이니, 그 줄이 함께 내는 \
             `claude plugin marketplace remove` 를 먼저 친다 — 그 전에는 몇 번을 불러도 건너뛴다.",
            missing.join("·")
        ));
    }
    Decision::Context(said)
}

/// 토막마다 **그 `moai` 가 도는 자리** — `-C`·`--dir` 나 앞의 `cd`·`pushd` 가 세션의 자리
/// (`cwd`)를 옮겼으면 그 디렉터리, 아니면 `None`. 차례는 [`segments`] 와 같다 — 받는 쪽이
/// 토막 번호로 가른다. `moai` 가 아닌 토막은 언제나 `None` 이다.
///
/// **모르면 `None` 이다** — 세션의 자리로 본다. 변수·틸드 뒤, `cd -`·`popd`·인자 없는 `cd`
/// 뒤는 어디인지 글자로 모른다. 지어낸 자리로 보내면 규칙이 새고, 세션 자리로 보는 것은 고치기
/// 전의 판정 그대로다.
///
/// **하위 셸의 `cd` 는 뒤로 안 이어진다** — `( … )` 묶음을 나오면 들어가기 전 자리로 돌아오고,
/// 파이프의 칸이나 `&` 로 띄운 `cd` 는 아무것도 안 옮긴다. 가르지 않던 판은
/// `(cd <남의 트래커> && moai status); moai add "딴 일"` 의 뒷토막을 남의 트래커로 보내, 실제로는
/// 세션 자리에 서는 줄이 규칙 1 을 넘었다.
pub fn aimed(cmd: &str, cwd: &Path) -> Vec<Option<PathBuf>> {
    let here = resolve(".", cwd);
    let mut at = Some(here.clone());
    // 묶음 겹마다 들어가기 전의 자리.
    let mut outer: Vec<Option<PathBuf>> = Vec::new();
    parse(cmd)
        .into_iter()
        .filter(|s| !s.words.is_empty())
        .map(|seg| {
            // 앞 토막 뒤로 내려간 자리까지 푼다 — 깊이가 같아도 형제 괄호(`(cd /b); (moai add x)`)는 딴 셸이다.
            while outer.len() > seg.low {
                if let Some(before) = outer.pop() {
                    at = before;
                }
            }
            while outer.len() < seg.depth {
                outer.push(at.clone());
            }
            let words = command_of(&seg.words);
            match words.first().map(|w| basename(w)) {
                Some("cd" | "pushd" | "popd") if seg.sub => None,
                Some("cd" | "pushd") => {
                    let arg = words[1..].iter().find(|w| !w.starts_with('-') || w.as_str() == "-");
                    at = match (at.take(), arg) {
                        // `pushd +1`·`-1` 은 스택을 돌리는 것이지 경로가 아니다 — 어디인지 모른다.
                        (Some(base), Some(a)) if !unknowable(a) && !a.starts_with('+') => Some(resolve(a, &base)),
                        _ => None,
                    };
                    None
                }
                Some("popd") => {
                    at = None;
                    None
                }
                _ => {
                    let args = moai_args(&seg.words)?;
                    let base = at.clone()?;
                    let dir = match flag_values(args, &["-C", "--dir"]).last() {
                        Some(d) if unknowable(d) => return None,
                        Some(d) => resolve(d, &base),
                        None => base,
                    };
                    (dir != here).then_some(dir)
                }
            }
        })
        .collect()
}

/// 토막마다 **`-C`·`--dir` 를 제 낱말로 적었는가** — 차례는 [`aimed`] 와 같다.
///
/// 아직 없는 자리를 가리킨 토막이 내미는 줄에 그 자리를 댈지를 가른다(moai-j2vp). [`aimed`] 는
/// 적은 `-C` 와 앞의 `cd` 가 옮긴 자리를 한 값으로 내는데, 없는 자리에서는 둘이 다르다 — 적은
/// `-C` 는 곧 만들어질 자리지만, `cd` 는 실패하면 세션 자리에서 돈다.
pub fn spells_dir(cmd: &str) -> Vec<bool> {
    segments(cmd)
        .iter()
        .map(|seg| moai_args(seg).is_some_and(|args| !flag_values(args, &["-C", "--dir"]).is_empty()))
        .collect()
}

/// 이 토막이 **하나를 집는가** — `moai mv <id>… <칸>` 의 칸이 벌여 놓는 칸이다.
/// `report::wip` 와 같은 셈이다: 설정이 아는 칸 중 첫 칸도 끝난 칸도 아닌 것.
fn picks_up(seg: &[String], cfg: &Config) -> bool {
    let Some(args) = moai_args(seg) else { return false };
    // 도움말은 아무것도 안 옮기고 0 으로 끝난다 — `mv … --help && sed -i …` 는 빈손으로 쓴다.
    if asks_help(args) {
        return false;
    }
    let verbs = positionals(args);
    match verbs.split_first() {
        Some((&"mv", rest)) if rest.len() >= 2 => {
            let to = rest[rest.len() - 1];
            cfg.knows(to) && cfg.is_started(to)
        }
        _ => false,
    }
}

/// `sed` 의 인자에서 **제자리로 고치는 파일들.** `-i` 가 없으면 아무것도 안
/// 쓴다 — stdout 으로 낼 뿐이다.
///
/// 스크립트는 `-e`·`-f` 로 따로 받지 않았으면 첫 자리 인자다. 그것을 파일로
/// 세면 `sed -i 's/a/b/' x` 가 `s/a/b/` 라는 파일을 쓰는 것으로 읽힌다.
fn sed_in_place(args: &[String]) -> Vec<String> {
    let mut in_place = false;
    let mut script_given = false;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            rest.extend(it.by_ref().cloned());
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((n, _)) => (n, true),
                None => (long, false),
            };
            match name {
                "in-place" => in_place = true,
                "expression" | "file" => {
                    script_given = true;
                    if !inline {
                        it.next();
                    }
                }
                "line-length" if !inline => {
                    it.next();
                }
                _ => {}
            }
            continue;
        }
        if let Some(short) = a.strip_prefix('-').filter(|s| !s.is_empty()) {
            for (n, ch) in short.char_indices() {
                let last = n + ch.len_utf8() == short.len();
                match ch {
                    // 뒤에 붙은 것은 백업 접미사다 (`-i.bak`). 따로 선 `-i` 뒤의
                    // `''`·`.bak` 도 접미사다 — BSD(macOS) sed 의 모양이다. 그것을
                    // 스크립트로 세면 진짜 스크립트가 파일로 읽혀, 저장소 밖을
                    // 고치는 흔한 명령이 막힌다.
                    'i' => {
                        in_place = true;
                        if last && it.clone().next().is_some_and(|s| s.is_empty() || s.starts_with('.')) {
                            it.next();
                        }
                        break;
                    }
                    'e' | 'f' | 'l' => {
                        script_given |= ch != 'l';
                        if last {
                            it.next();
                        }
                        break;
                    }
                    _ => {}
                }
            }
            continue;
        }
        rest.push(a.clone());
    }
    if !in_place {
        return Vec::new();
    }
    if !script_given && !rest.is_empty() {
        rest.remove(0);
    }
    rest
}

/// 이 리뷰 줄에 **무엇을 왜 보는지**가 적혀 있는가.
///
/// 본문을 본다. **저널이 아니라 스냅샷을 본다** — 저널을 접어야 답이 나오는
/// 질문을 규칙이 묻기 시작하면, 그 답은 스냅샷의 필드가 되어야 한다는 결정이
/// 곧장 무너진다. 관점은 이미 필드(`body`)이므로 물을 것이 없다.
fn has_angle(i: &Issue) -> bool {
    i.body.as_deref().is_some_and(|b| !b.trim().is_empty())
}

/// 규칙 3 — **리뷰도 이슈다. 그 리뷰는 지금 보는 것에 매여야 한다.**
///
/// 저장소 어딘가에 열린 리뷰 줄이 하나 있다는 것으로는 안 된다. 옛 리뷰 한
/// 줄이 뒤따르는 모든 리뷰의 면죄부가 되면, 거기 적히는 결과가 무엇의
/// 결과인지를 잃는다.
pub fn guard_review(issues: &[Issue], cfg: &Config, away: &Away) -> Decision {
    let out_of_plan = report::put_off(issues);
    // **옆 워크트리의 리뷰는 여기서 안 센다** — 초점에서 뺀 것과 같은 자다([`theirs`]).
    // 세면 main 에서 아무것도 안 집은 세션에 옆이 이미 집은 리뷰를 "집으라" 고 막는다.
    let theirs = theirs(issues, away);
    let open: Vec<&Issue> = issues
        .iter()
        .filter(|i| is_review(i, &out_of_plan) && !i.status.is_done() && !theirs(i))
        .collect();
    // [`held`] 와 같은 초점이다 — 위에서 지은 `theirs` 를 그대로 쓴다(moai-xppm). `held` 를 부르던
    // 판은 같은 소속 지도를 한 번 더 지었다.
    let focus: Vec<&Issue> = report::wip(issues, cfg).into_iter().filter(|i| !theirs(i)).collect();

    if focus.is_empty() {
        // 집은 것이 없으면 굴러가는 리뷰도 없다 — `focus` 가 곧 `wip` 이라,
        // 여기서 "굴러가는 리뷰" 를 다시 찾던 조건은 언제나 거짓이었다.
        // 굴러가는 리뷰가 있는 길은 아래 `anchored` 가 맡는다.
        if let Some(idle) = open.first() {
            return refuse(3, format!(
                "리뷰 이슈 {} 가 아직 안 집혔다. 리뷰를 시작하면 그 줄도 같이 움직인다.\n\
                 \x20 moai mv {} in_progress --from {}\n\
                 그 리뷰가 아니면 지금 보는 것을 먼저 집고 다시 부른다.",
                idle.id,
                idle.id,
                idle.status.as_str()
            ));
        }
        return refuse(3, format!(
            "리뷰는 이슈로 남긴다. 집은 것이 없으니 무엇을 보는지부터 정한다 —\n\
             보는 것을 집거나, 리뷰 이슈를 세워 그것을 집는다.\n  {}\n{REVIEW_STEPS}",
            crate::guide::make_review("-e <에픽>")
        ));
    }

    let unit = unit_of(issues, &focus);
    let epics = report::groups(issues);
    let anchored: Vec<&&Issue> = open
        .iter()
        .filter(|i| {
            unit.contains(i.id.as_str())
                || epics.get(i.id.as_str()).is_some_and(|e| unit.contains(e))
                || crate::id::parent_of(&i.id).is_some_and(|p| unit.contains(p))
        })
        .collect();
    // **관점이 적힌 리뷰만 리뷰로 친다.** 제목뿐인 리뷰 줄은 무엇을 왜 보는지를
    // 남기지 않아, 다음 사람이 그 리뷰가 무엇을 훑었는지 영영 모른다 — 리뷰가
    // 낸 글 중 값이 큰 쪽이 통째로 사라지는 자리다.
    if anchored.iter().any(|i| has_angle(i)) {
        return Decision::Pass;
    }
    if let Some(empty) = anchored.first() {
        return refuse(3, format!(
            "리뷰 이슈 {} 에 무엇을 왜 보는지가 없다. 적고 다시 부른다.\n\
             \x20 moai edit {} -b -\n\
             관점 없이 돌린 리뷰는 무엇을 훑었는지가 안 남아, 다음 사람이 같은 자리를\n\
             다시 훑는다.",
            empty.id, empty.id
        ));
    }
    let head = focus[0];
    let stray = if open.is_empty() {
        String::new()
    } else {
        format!(
            "열린 리뷰 줄이 있지만 지금 보는 것에 안 매여 있다: {}\n",
            open.iter()
                .take(3)
                .map(|i| format!(
                    "{}({})",
                    i.id,
                    epics.get(i.id.as_str()).copied().unwrap_or("에픽 없음")
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    refuse(3, format!(
        "리뷰는 이슈로 남긴다. 지금 보는 것({} {})에 매인 리뷰 이슈를 먼저 세운다.\n\
         {stray}\x20 {}\n{REVIEW_STEPS}",
        head.id,
        head.title,
        crate::guide::make_review(&format!("--parent {}", head.id))
    ))
}

/// 거절문과 비춤이 내미는 줄이 **겨눌 트래커** — 토막 번호([`segments`] 의 번호)로 묻는다.
/// `None` 이면 이 자리의 트래커라 맨 `moai` 로 낸다. 자리를 푸는 것은 파일 계통을 아는 `cmd/hook.rs`
/// 고(`aimed` → `Repo::find_from`, 딸린 워크트리는 루트로 옮겨진다), 여기는 그 답만 받는다.
pub type Toward<'a> = &'a dyn Fn(usize) -> Option<&'a Path>;

/// 내미는 줄의 머리 — 겨눌 트래커가 이 자리면 맨 `moai`, 아니면 `moai -C <그 자리>` 다.
/// `-C` 를 고르는 자는 [`aim_flag`] 고, 여기는 머리를 붙일 뿐이다.
fn echo_moai(dir: Option<&Path>, seg: &[String]) -> String {
    format!("moai{}", aim_flag(dir, seg))
}

/// [`echo_moai`] 의 머리에 붙는 `-C` 조각 — 겨눌 트래커가 이 자리면 빈 글이다.
///
/// **한국어 알림([`korean_notice`])이 이 꼴로 받는다.** 머리를 지어 놓고 도로 떼어 내던 판은 두
/// 함수를 글자 수술(`trim_start_matches("moai")`)로 묶어, `echo_moai` 의 머리를 한 글자만 바꿔도
/// 알림이 말없이 망가졌다.
///
/// **[`echo_dir`] 이 아니라 [`crate::text::shell_word`] 로 감싼다.** 저쪽은 사람이 친 글자를 옮겨
/// 적는 자리라 `$HOME`·`` `…` `` 을 큰따옴표로 감싸 옮겨 친 셸이 **다시 풀게** 두는데, 여기 드는
/// 값은 이미 푼 참 경로다 — 다시 풀리면 `/tmp/a$b` 가 `/tmp/a` 로 줄고 `` /srv/`id` `` 는 내민 줄이
/// 명령을 돌린다. 참 경로는 글자 그대로 한 낱말이어야 한다.
///
/// **자리를 못 푼 토막은 친 글자를 그대로 옮긴다**(`$VAR`·`~`·`$( … )`·글롭 — [`unknowable`]).
/// `aimed` 가 그런 `-C` 를 `None` 으로 내 이 자리의 트래커와 구별되지 않는데, 버리고 맨 `moai` 를
/// 내면 `moai -C "$OTHER" add …` 로 막힌 사람에게 **이 트래커에** 세우라고 시킨다. 그 글자는 옮겨
/// 친 셸이 처음처럼 푼다.
///
/// 이름이 UTF-8 이 아니면 대지 않는다 — `display()` 가 그 바이트를 U+FFFD 로 바꿔, 내민 줄이 있지도
/// 않은 자리를 겨눈다. 빈 글을 내면 받는 쪽의 맨 `moai` 가 적어도 이 자리에서 돈다.
fn aim_flag(dir: Option<&Path>, seg: &[String]) -> String {
    if let Some(d) = dir.and_then(Path::to_str) {
        return format!(" -C {}", crate::text::shell_word(d));
    }
    moai_args(seg)
        .and_then(|args| flag_values(args, &["-C", "--dir"]).pop())
        .filter(|d| dir.is_none() && unknowable(d))
        .map_or_else(String::new, |d| format!(" -C {}", echo_dir(&d)))
}

/// 막힌 토막에 적힌 `-C` 값을 내미는 줄에 옮겨 적는다 — 셸이 한 낱말로 읽게. **한국어 알림
/// ([`korean_write`])이 쓴다** — 한쪽만 감싸던 판은 `-C '/a b'` 로 막힌 줄을 `moai -C /a b add …` 로
/// 일러 줬다(리뷰 moai-ju21.70g).
///
/// 렉서는 풀 자리(`$HOME`·`$(…)`·백틱)를 글자째 남긴다 — 그런 값은 큰따옴표로 감싸 옮겨 친 셸이 처음처럼
/// 푼다. 셸이 가르는 글자(빈칸·따옴표·`&`·`;`…)가 들면 작은따옴표로([`crate::text::shell_word`]), 아니면
/// 적힌 그대로다 — 맨 앞의 `~` 도 처음처럼 푼다. **푼 경로에는 쓰지 않는다** — [`echo_moai`] 를 본다.
fn echo_dir(d: &str) -> String {
    if d.contains(['$', '`']) {
        format!("\"{d}\"")
    } else if !d.is_empty() && d.chars().all(|c| c.is_alphanumeric() || "/._-+,:@%=~".contains(c)) {
        d.to_string()
    } else {
        crate::text::shell_word(d)
    }
}

/// 거절한다. **어긴 규칙의 이름이 첫 줄이다** — 스킬이 적은 규칙 제목과 글자가
/// 같아야 막힌 쪽이 무엇을 어겼는지 한 번에 찾는다.
fn refuse(rule: usize, why: String) -> Decision {
    Decision::Deny(format!("{}\n{why}", crate::guide::rule_head(rule)))
}

/// 이 자리에서 **아직** 집고 있는 것 — [`held`] 에서 겹친 줄(`latest`)로는 더는 안 집힌 줄을 뺀 것.
/// [`closing`] 과 접힌 뒤 싣는 것([`carried`])이 같은 자로 잰다.
///
/// **겹친 줄에서도 집혀 있는 것만 센다**(리뷰 moai-dw63.nzw). 트래커는 main 에서 쓰므로 딸린
/// 워크트리의 스냅샷은 갈라진 때에 멈춰 있다 — 거기서 `moai -C <루트>` 로 첫 칸에 되돌리거나 미룬
/// 줄을 제 스냅샷으로만 세면, 그 워크트리에 여는 세션마다 이미 놓은 것을 도로 놓으라고 붙들고,
/// 이미 미룬 줄에는 `mv` 로는 안 풀리는 "첫 칸에 두면 열린 채 남는다" 를 댔다(`mv` 는 미룸을
/// 안 푼다). **빼기만 한다** — 옆이 집은 줄을 제 초점에 더하지 않는다.
fn holding<'a>(issues: &'a [Issue], latest: &[Issue], cfg: &Config, away: &Away) -> Vec<&'a Issue> {
    let mut wip = held(issues, cfg, away);
    if !wip.is_empty() {
        let still: BTreeSet<&str> = report::wip(latest, cfg).into_iter().map(|i| i.id.as_str()).collect();
        wip.retain(|i| still.contains(i.id.as_str()));
    }
    wip
}

/// 세션을 닫기 전에 — **상태가 실제와 맞는가.**
///
/// 붙드는 것은 세션당 한 번이다. 규칙 2 가 초점을 요구하므로, 그것 없이는
/// 일하는 내내 매 턴이 붙들린다 — 같은 잔소리를 매번 들으면 아무도 안 읽는다.
///
/// `latest` 는 에픽이 닫히는지를 잴 줄이다([`shelving_closes`]) — 받는 쪽이 옆 워크트리까지 겹쳐
/// 넘긴다. 겹칠 것이 없으면 `issues` 그대로다. 집은 줄이 **아직 집혀 있는지도** 거기서 되짚는다.
pub fn closing(
    issues: &[Issue],
    latest: &[Issue],
    cfg: &Config,
    away: &Away,
    warnings: usize,
    before: Option<usize>,
) -> Decision {
    let wip = holding(issues, latest, cfg, away);
    // **집은 것이 없으면 붙들 것은 늘어난 경고뿐이다** — 아래는 집은 줄과 그것에 매인 리뷰만 센다. `Stop` 은
    // 붙들 것이 없으면 표를 안 남겨 턴마다 여기를 다시 지나므로, 빈 초점에 소속 지도를 짓지 않는다(리뷰
    // moai-3k2d.1df).
    if wip.is_empty() {
        return grown(warnings, before).map_or(Decision::Pass, Decision::Block);
    }
    let mut lines = Vec::new();
    let epics = report::groups(issues);
    let out_of_plan = report::put_off(issues);
    let closes = shelving_closes(latest, cfg, &wip);
    if !wip.is_empty() {
        lines.push(
            "아직 집고 있는 것이 있다. 실제로 끝났으면 옮기고, 안 할 것이면 미루고, 이어서 할 것이면 다음 세션에 한 줄 남긴다."
                .to_string(),
        );
        for i in &wip {
            // **그 줄이 갈 수 있는 칸만 댄다.** 모두에게 `review|done` 을 일러 주던
            // 판은 이미 review 인 줄에 제자리걸음을 시켰고, `|` 는 그대로 치면 파이프다.
            let ahead: Vec<&str> = cfg
                .statuses
                .iter()
                .map(String::as_str)
                .skip_while(|s| *s != i.status.as_str())
                .skip(1)
                .collect();
            let (last, between) =
                ahead.split_last().map_or((crate::config::DONE, &[][..]), |(last, between)| (*last, between));
            for col in between {
                lines.push(format!("  moai mv {} {col}", i.id));
            }
            // **리뷰 줄은 낸 글과 함께 닫는 걸음을 댄다**(리뷰 moai-dw63.nzw). `-m` 없는 `done` 은
            // 규칙 3 이 막는다 — 훅이 일러 준 명령을 훅이 막는 자리는 덫이다(`guide::REVIEW_STEPS`).
            if last == crate::config::DONE && is_review(i, &out_of_plan) {
                lines.push(crate::guide::close_steps(&i.id, "moai"));
            } else {
                lines.push(format!("  moai mv {} {last}     {}", i.id, i.title));
            }
            // **미루면 에픽이 닫히는 줄에는 미룸의 값을 함께 댄다**(moai-8ema). 미룬 멤버는
            // 에픽의 칸에서 빠지므로, 끝난 멤버 곁에 집은 것만 남은 에픽은 미루는 순간 목적을 못
            // 이룬 채 `done` 으로 선다 — 결정을 기다리는 멤버에 "지금 안 할 것이면" 만 대던 판은
            // moai-l288 이 막은 문을 훅이 도로 열었다.
            //
            // **그 줄에는 첫 칸으로 되돌리는 길도 댄다**(moai-1plu, 사용자 결정 A). 갈림길 1 은
            // 결정을 기다리는 멤버를 첫 칸에 두라 한다. 쥔 채 이어받을 줄만 대던 판은 그 줄을
            // `in_progress` 에 세워 두어 `stale_progress`·`wip_overload` 가 서고, 워크트리를 치우면
            // `stranded` 로 서서 그 힌트가 미룸 — 에픽을 닫는 수 — 을 첫 칸과 나란히 댔다. 첫 칸의
            // 멤버도 에픽을 열어 둔다. 되돌아가는 칸은 이 줄에만 댄다 — 보통 줄은 앞 칸뿐이다
            // (`closing_offers_only_the_columns_ahead`).
            //
            // **무엇을 기다리는지를 `-m` 으로 남긴다**(리뷰 moai-dw63.nzw). 첫 칸의 줄은 `ready` 에
            // 서고, `ready` 는 끝나가는 에픽을 먼저 세워 마지막 멤버가 맨 위로 온다 — 까닭 없이
            // 놓으면 다음 세션이 결정 없이 집는다. **리뷰 줄에는 대지 않는다** — 리뷰가 결정을
            // 기다리면 그 결정은 멤버로 세우고(갈림길 1) 리뷰는 낸 글과 함께 닫는다(규칙 3). 리뷰를
            // 첫 칸에 되돌리면 낸 글 없이 `ready` 에 서서, 다음 세션이 리뷰 한 판을 다시 돈다.
            let shuts = closes.get(i.id.as_str()).copied();
            if let Some(e) = shuts.filter(|_| !is_review(i, &out_of_plan)) {
                lines.push(format!(
                    "  moai mv {} {} -m '무엇을 기다리나'      결정을 기다리는 것이면 — 첫 칸에 두면 {e} 가 열린 채 남는다",
                    i.id,
                    cfg.first_status()
                ));
            }
            let when = match shuts {
                Some(e) => format!("{e} 의 목적을 접을 때만 — 집은 것을 미루면 {e} 에 끝난 멤버만 남아 목적을 못 이룬 채 닫힌다"),
                None => "지금 안 할 것이면".to_string(),
            };
            // 자유 글은 작은따옴표다(moai-1yya) — 이 줄은 옮겨 치는 글이고, 큰따옴표 안의 백틱은 bash 가
            // 명령으로 푼다.
            lines.push(format!("  moai defer {} -m '왜'      {when}", i.id));
            lines.push(format!("  {}      이어서 할 것이면", crate::guide::handoff(&i.id)));
        }
    }
    // **굴러가는 리뷰와 지금 집은 것에 매인 리뷰만 센다.** 저장소에 남은 옛
    // 리뷰 줄까지 세면 매 세션 같은 줄이 나오고, 그러면 아무도 안 읽는다.
    let unit = unit_of(issues, &wip);
    // **옆 워크트리의 리뷰는 안 센다** — 규칙 3([`guard_review`]·[`close_in`])과 같은 자다(리뷰
    // moai-dw63.nzw). 세면 옆 일꾼이 같은 에픽에서 돌리는 리뷰를 이 세션에 "낸 글을 붙이고
    // 닫으라" 고 붙든다. **겹친 줄에서 이미 닫았거나 미룬 리뷰도 안 센다** — 위에서 집은 줄을
    // 되짚은 것과 같은 자다. 안 그러면 main 에서 닫은 리뷰가 집은 줄에서 빠지는 대신 여기로 돌아온다.
    let theirs = theirs(issues, away);
    let out = report::put_off(latest);
    let settled: BTreeSet<&str> =
        latest.iter().filter(|i| i.status.is_done() || out.contains(i.id.as_str())).map(|i| i.id.as_str()).collect();
    for i in issues.iter().filter(|i| {
        is_review(i, &out_of_plan) && !i.status.is_done() && !theirs(i) && !settled.contains(i.id.as_str())
    }) {
        // **규칙 3 과 같은 셈법이어야 한다.** 여기서 부모를 빼면, 거절문이
        // 시킨 대로 `--parent` 로 세운 리뷰가 규칙 3 은 지나가면서 닫을 때는
        // 아무도 안 챙기는 줄이 된다 — 한 규칙의 두 짝이 서로 다른 말을 한다.
        let mine = unit.contains(i.id.as_str())
            || epics.get(i.id.as_str()).is_some_and(|e| unit.contains(e))
            || crate::id::parent_of(&i.id).is_some_and(|p| unit.contains(p));
        if mine && !wip.iter().any(|w| w.id == i.id) {
            lines.push(format!(
                "리뷰 이슈 {} 가 아직 열려 있다. 낸 글을 붙이고 닫는다.\n{}",
                i.id,
                crate::guide::close_steps(&i.id, "moai")
            ));
        }
    }
    lines.extend(grown(warnings, before));
    if lines.is_empty() { Decision::Pass } else { Decision::Block(lines.join("\n")) }
}

/// 세션을 여는 때보다 경고가 늘었으면 그 한 줄([`closing`]).
fn grown(warnings: usize, before: Option<usize>) -> Option<String> {
    before
        .filter(|before| warnings > *before)
        .map(|before| format!("경고가 {before} 에서 {warnings} 로 늘었다. `moai status` 로 무엇이 늘었는지 본다."))
}

/// 집은 것을 미루면 **목적을 못 이룬 채 `done` 으로 서는 에픽** — 집은 줄 id → 그 에픽 id.
///
/// **칸을 읽는 자 그대로 잰다** — 집은 줄을 미룬 스냅샷을 지어 [`report::group_states`] 에 묻는다.
/// 멤버를 손으로 세던 판은 셋을 틀렸다. 미룸은 밑으로 물려주므로(`report::put_off`) `--parent`
/// 로 세운 리뷰 자식이 함께 빠지는데 그것을 남은 멤버로 세어, 규칙 3 이 시킨 모양 그대로에서 경고를
/// 놓쳤다. 같은 에픽의 집은 줄 둘을 서로의 "남은 멤버" 로 세어, `closing` 이 줄마다 댄 미룸을 다
/// 치면 닫히는 것을 못 봤다. 묶음이 제 조상에게서 받은 미룸으로는 멤버를 안 빼는 칸의 셈과 달라,
/// 안 닫히는 에픽을 닫힌다고 했다.
///
/// **집은 것을 한꺼번에 미룬다** — `closing` 이 줄마다 미룸을 대므로, 시킨 대로 다 치면 서는 칸이
/// 물을 칸이다. 에픽 줄이 실제로 선 것만 잰다(끊긴 참조는 닫힐 에픽이 없다).
fn shelving_closes<'a>(latest: &'a [Issue], cfg: &Config, wip: &[&Issue]) -> std::collections::BTreeMap<&'a str, &'a str> {
    let held: BTreeSet<&str> = wip.iter().map(|i| i.id.as_str()).collect();
    if held.is_empty() {
        return Default::default();
    }
    let epics = report::groups(latest);
    let aims: std::collections::BTreeMap<&str, &str> = latest
        .iter()
        .filter(|i| held.contains(i.id.as_str()) && !i.status.is_done())
        .filter_map(|i| Some((i.id.as_str(), *epics.get(i.id.as_str())?)))
        .filter(|(_, e)| latest.iter().any(|g| g.id == *e && g.kind == crate::model::Kind::Epic))
        .collect();
    if aims.is_empty() {
        return aims;
    }
    let mut shelved = latest.to_vec();
    for i in shelved.iter_mut().filter(|i| held.contains(i.id.as_str()) && !i.is_deferred()) {
        i.deferred_at = Some(i.updated_at.clone());
    }
    let after = report::group_states(&shelved, cfg);
    aims.into_iter().filter(|(_, e)| after.get(e) == Some(&crate::config::DONE)).collect()
}

/// 세지 않는 자리. 저장소 밖, 트래커 자신, 도구 설정, 빌드 산출물.
/// 여기를 고치는 것은 "일" 이 아니다 — 일을 하러 가는 길이다.
const SKIP: &[&str] = &[".moai", ".claude", ".git", "target", "node_modules"];

/// **`_workspace` 도 세지 않는다 — 어느 깊이에서든**(사용자, moai-5wk4). `humanize-korean` 은 **cwd** 에 이
/// 폴더를 만든다 — 안내는 저장소 밖에서 돌리라고 하지만, 저장소 안에서 돌렸다고 한국어 글을 다듬는 일이 규칙 2
/// 에 막히면 안내가 시킨 일을 규칙이 막는다. 뿌리의 것만 빼던 판은 하위 디렉터리에 선 세션을 그대로 막았다 —
/// 이 저장소 `.gitignore` 의 `_workspace/` 도 어느 깊이에서든 걸린다.
const SCRATCH: &str = "_workspace";

/// 이 파일을 고치는 것이 일에 매여야 하는가.
fn counted(path: &str, root: &Path) -> bool {
    if path.is_empty() {
        return false;
    }
    let Ok(rel) = settled(path, root).strip_prefix(root).map(Path::to_path_buf) else {
        return false; // 저장소 밖 — 스크래치패드·임시 파일·남의 저장소
    };
    let mut parts = rel.components().map(|c| c.as_os_str().to_str());
    parts.next().flatten().is_some_and(|head| !SKIP.contains(&head) && head != SCRATCH)
        && !parts.any(|p| p == Some(SCRATCH))
}

/// **상대 경로는 저장소의 자리로 푼다.** 훅 프로세스가 어디서 도는지는 아무도
/// 약속하지 않았다 — `src/main.rs` 가 저장소 밖으로 보여 규칙이 통째로 샜다.
fn resolve(path: &str, root: &Path) -> PathBuf {
    let p = Path::new(path);
    let joined = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
    // **`..` 를 접는다.** 접지 않으면 판정이 양쪽으로 다 틀린다 —
    // `.moai/../src/store.rs` 는 첫 조각이 `.moai` 라 안 세는 자리로 보이고,
    // `../elsewhere/x.rs` 는 `strip_prefix` 가 그대로 붙어 저장소 안으로 보인다.
    // 파일이 아직 없을 수도 있으므로 디스크를 짚지 않고 글자로만 접는다 — 링크를 푸는 것은
    // 규칙 2 의 판정([`settled`])만 따로 한다.
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// **규칙 2 가 견주는 자리.** [`resolve`] 위에 링크 철자를 푸는 한 겹을 얹는다.
///
/// 같은 자리를 두 철자로 부르면 `strip_prefix` 가 어긋나 저장소 안의 파일이 밖으로 보이고,
/// 규칙 2 가 통째로 샌다 — 훅의 `root` 는 `current_dir()` 에서 와 늘 풀린 철자인데(`getcwd` 가
/// 링크를 푼다) Claude 가 주는 `file_path` 는 사람이 친 철자 그대로다. `TMPDIR` 이 링크인 기계,
/// macOS 의 `/tmp`·`/var`, 링크로 건 프로젝트가 다 그 자리다.
///
/// **푸는 것은 여기뿐이다.** [`resolve`] 는 [`aimed`] 가 `-C` 와 `cd` 를 좇는 데도 쓰는데, 그
/// 값은 거절문이 사람에게 내미는 명령의 경로로 그대로 선다 — 거기서 링크를 풀면 사람이 친 적
/// 없는 철자를 옮겨 치라고 내민다. 판정은 두 철자를 한 자리로 봐야 하고, 내미는 글은 사람이 친
/// 철자를 지켜야 한다.
fn settled(path: &str, root: &Path) -> PathBuf {
    let folded = resolve(path, root);
    // **있는 가장 긴 윗자리를 풀고 나머지를 다시 붙인다**(2026-09-19 사용자 결정) — 아직 없는
    // 파일도 그 윗자리까지는 같게 풀리므로, 새로 만드는 파일과 이미 있는 파일이 같은 답을 받는다.
    // 통째로 `canonicalize` 하던 길은 없는 파일에서 실패해 준 철자를 그대로 돌려주고, 그러면
    // 만드는 쪽에서만 규칙이 꺼진다. **`..` 는 [`resolve`] 가 이미 접었다** — 다시 접지 않는다.
    for head in folded.ancestors() {
        if let Ok(real) = std::fs::canonicalize(head) {
            // **떼어 내기는 실패하지 않는다** — `head` 는 `folded` 의 조상이다. 이것을 `if let` 으로
            // 받아 넘기면 못 뗀 자리에서 한 칸 더 짧은 조상으로 내려가, 엉뚱한 윗자리로 푼 경로를
            // 아무 말 없이 답으로 낸다. 남은 조각이 비면 `join` 이 끝에 가름선만 붙이는데,
            // `Path` 의 견주기와 `strip_prefix` 는 조각으로 도니 두 쪽 다 같은 답이다.
            return real.join(folded.strip_prefix(head).expect("조상에서 떼어 낸다"));
        }
    }
    folded
}

fn rel_to(path: &str, root: &Path) -> String {
    settled(path, root)
        .strip_prefix(root)
        .map(|r| r.display().to_string())
        .unwrap_or_else(|_| path.to_string())
}

/// 명령줄에서 이 플래그들에 딸린 값을 모은다. `-e x`·`-e=x`, 그리고 짧은
/// 플래그에 **붙여 쓴** `-ex` 를 다 받는다 — clap 이 받는 모양을 못 읽으면
/// `moai mv t-r done -m"반영"` 처럼 옳게 친 명령이 막힌다.
///
/// **`--` 뒤는 플래그가 아니다.** clap 은 그 뒤를 자리 인자로 받는다 — 여기서
/// 계속 훑으면 `moai add -- --type=idea` 가 `idea` 로 읽혀 지나가는데, 실제로는
/// 제목이 `--type=idea` 인 이슈가 선다.
fn flag_values(seg: &[String], flags: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts = seg.iter().peekable();
    while let Some(t) = parts.next() {
        if t == "--" {
            break;
        }
        if let Some((f, v)) = t.split_once('=')
            && flags.contains(&f)
        {
            out.push(v.to_string());
        } else if flags.contains(&t.as_str()) {
            if let Some(v) = parts.peek() {
                out.push(v.to_string());
            }
        } else if !t.starts_with("--")
            && let Some(v) = flags
                .iter()
                .filter(|f| f.len() == 2 && !f.starts_with("--"))
                .find_map(|f| t.strip_prefix(*f))
                .filter(|v| !v.is_empty())
        {
            out.push(v.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Status};

    fn cfg() -> Config {
        Config::parse("prefix = \"t\"\n").unwrap()
    }

    fn issue(id: &str, status: &str) -> Issue {
        Issue::new(id.into(), "제목".into(), Kind::Issue, Status::new(status), "2026-01-01T00:00:00Z")
    }

    fn epic(id: &str) -> Issue {
        let mut i = issue(id, "todo");
        i.kind = Kind::Epic;
        i.title = "저장 계층".into();
        i
    }

    fn under(id: &str, status: &str, e: &str) -> Issue {
        let mut i = issue(id, status);
        i.epic = Some(e.into());
        i
    }

    fn shell(cmd: &str) -> serde_json::Value {
        serde_json::json!({ "command": cmd })
    }

    fn denied(d: &Decision) -> &str {
        match d {
            Decision::Deny(r) => r,
            other => panic!("막지 않았다 — {other:?}"),
        }
    }

    /// 옆 워크트리가 쥔 것이 없다 — 대부분의 시험이 선 자리.
    fn here() -> Away {
        Away::default()
    }

    fn away(ids: &[&str]) -> Away {
        Away { names: set(ids), ..Away::default() }
    }

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    // ── 옆 워크트리가 쥔 일 ──────────────────────────────────────────

    /// **옆 워크트리가 쥔 일은 제 초점이 아니다.** main 에 들어온 집기 커밋을 제
    /// 것으로 세던 판은 main 의 `moai add` 를 막고, 닫을 때 남의 일을 옮기라고
    /// 붙들었다(moai-0yrv).
    #[test]
    fn what_another_worktree_holds_is_not_my_focus() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), issue("t-2", "in_progress")];
        let there = away(&["t-1"]);
        let mine: Vec<&str> = held(&all, &cfg(), &there).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(mine, ["t-2"]);

        // 여기서 집은 것은 여전히 초점이다 — 거절문도 그것만 댄다.
        let why = denied(&guard_create(&all, &cfg(), &there, "moai add '딴 일'")).to_string();
        assert!(why.contains("t-2") && !why.contains("t-1"), "옆의 일을 초점으로 댄다\n{why}");
        let Decision::Block(why) = closing(&all, &all, &cfg(), &there, 0, None) else {
            panic!("여기서 집은 것을 안 붙든다");
        };
        assert!(why.contains("moai mv t-2") && !why.contains("t-1"), "옆의 일을 옮기라고 한다\n{why}");

        // 다 옆이 쥐었으면 여기서 집은 것이 없다.
        let both = away(&["t-1", "t-2"]);
        assert_eq!(guard_create(&all, &cfg(), &both, "moai add '딴 일'"), Decision::Pass);
        assert_eq!(closing(&all, &all, &cfg(), &both, 0, None), Decision::Pass);
        assert_eq!(carried(&all, &all, &cfg(), &both), Decision::Pass);
        // 그러면 규칙 2 가 선다 — 저장소를 고치려면 여기서 하나를 집는다.
        assert!(matches!(
            guard_edit(&all, &cfg(), &both, Path::new("/repo"), "/repo/src/store.rs"),
            Decision::Deny(_)
        ));
    }

    /// 옆에서 그 일을 펼쳐 집은 것도 옆의 것이다 — 그 밑의 자식, 그 에픽에 든 줄.
    /// id 가 아닌 이름(`main`)이나 없는 id 는 아무것도 안 뺀다.
    #[test]
    fn what_hangs_under_the_named_work_goes_with_it() {
        let all = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            issue("t-2", "todo"),
            issue("t-2.aa", "in_progress"),
        ];
        assert!(held(&all, &cfg(), &away(&["t-e", "t-2"])).is_empty());
        assert_eq!(held(&all, &cfg(), &away(&["main", "t-9"])).len(), 2);
    }

    /// **옆이 집은 리뷰를 여기서 집으라고 하지 않는다.** 초점에서 뺀 리뷰 줄을 규칙 3 이
    /// 그대로 세면, 아무것도 안 집은 main 세션에 "moai mv t-1.aa in_progress" 를 댄다 —
    /// 이미 그 칸이라 시킨 대로 해도 안 풀린다.
    #[test]
    fn a_review_another_worktree_holds_is_not_named_here() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-1.aa", "in_progress", None)];
        let why = denied(&guard_review(&all, &cfg(), &away(&["t-1"]))).to_string();
        assert!(!why.contains("t-1.aa"), "옆의 리뷰를 집으라고 한다\n{why}");
    }

    /// 옆 딸린 워크트리에도 벌여 놓인 줄만 누구의 것인지 모른다 — **제 워크트리 이름이 가리키는
    /// 일은 확실히 제 것이라** 거기 있어도 빼지 않는다(moai-ntl6).
    #[test]
    fn only_work_also_held_elsewhere_is_unsure_and_my_named_work_stays_mine() {
        let all = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            issue("t-2", "in_progress"),
            issue("t-3", "in_progress"),
            issue("t-4", "todo"),
        ];
        let elsewhere = set(&["t-1", "t-2", "t-4"]);
        let got: Vec<String> = unsure(&all, &cfg(), &elsewhere, &set(&["t-e"]), &Picks::default()).into_iter().collect();
        assert_eq!(got, ["t-2"], "제 에픽의 일이나 안 집은 줄을 모른다고 했다");
        assert!(unsure(&all, &cfg(), &set(&[]), &set(&[]), &Picks::default()).is_empty());
    }

    /// **제 이름이 에픽 이름을 이긴다**(moai-m62u·moai-cle9). 머지하고 안 치운(또는 아직 도는) 에픽
    /// 워크트리가 옆에 있어도, 그 에픽에 뒤이어 집은 멤버를 제 이름 워크트리에서 하는 세션의 초점은
    /// 그 멤버다 — 한때 에픽 이름에 뺏겨 규칙 2 가 저장소를 막았다. 제 이름이 없는 자리(루트)에서는
    /// 여전히 옆 에픽 워크트리의 일이다.
    #[test]
    fn my_own_name_outranks_an_epic_worktree_beside_me() {
        let all = vec![epic("t-e"), under("t-1", "done", "t-e"), under("t-2", "in_progress", "t-e")];
        let beside = Away { names: set(&["t-e"]), own: set(&["t-2"]), ..Away::default() };
        let mine: Vec<&str> = held(&all, &cfg(), &beside).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(mine, ["t-2"], "에픽 워크트리가 제 이름 워크트리의 멤버를 쥐었다");
        assert_eq!(guard_edit(&all, &cfg(), &beside, Path::new("/repo"), "/repo/src/x.rs"), Decision::Pass);

        let root = Away { names: set(&["t-e", "t-2"]), own: set(&["develop"]), ..Away::default() };
        assert!(held(&all, &cfg(), &root).is_empty(), "루트가 옆 워크트리의 일을 제 초점으로 셌다");

        // 모르는 줄은 제 이름이 가리켜도 뺀다 — 좁힌 초점은 풀기만 하는 판정에만 실린다.
        let narrowed = Away { unsure: set(&["t-2"]), ..beside };
        assert!(held(&all, &cfg(), &narrowed).is_empty());
    }

    /// **돌았는지 모르는 집기는 남의 기록을 안 지운다**(moai-hze6, 사용자 결정) — `집기 && 무엇 ||
    /// 집기` 의 뒤 집기는 앞이 이기면 안 돈다. 적기는 적되([`Picks::line`] 의 `?`) 그것으로 남이 쥔
    /// 줄을 제 것으로 삼지 않는다 — 기록은 풀기만 한다.
    #[test]
    fn a_pick_that_might_not_have_run_never_takes_a_row_from_another_session() {
        let never = |_: &str| None;
        let read = |files: &[(&str, String)]| {
            Picks::fold("me", files.iter().map(|(s, t)| (*s, t.as_str())).collect::<Vec<_>>(), &never)
        };
        let theirs = ("you", Picks::line(1, None, "t-1", true));
        let sure = ("me", Picks::line(2, None, "t-1", true));
        let maybe = ("me", Picks::line(2, None, "t-1", false));

        // 확실히 도는 집기는 남의 기록을 지운다 — 마지막으로 집은 세션이 이긴다(moai-4jsy).
        assert_eq!(read(&[theirs.clone(), sure.clone()]).mine, set(&["t-1"]), "넘겨받은 줄을 제 것으로 안 든다");
        // **돌았는지 모르는 집기는 안 지운다**(moai-hze6, 사용자 결정) — 남이 쥔 줄을 제 초점에 세우면
        // 규칙 1 이 막고 `Stop` 이 그것을 닫으라고 붙든다. 기록은 풀기만 한다.
        let read_maybe = read(&[theirs.clone(), maybe.clone()]);
        assert!(read_maybe.mine.is_empty(), "안 돈 집기가 남의 기록을 지웠다 — {read_maybe:?}");
        assert_eq!(read_maybe.theirs, set(&["t-1"]));
        // 남의 기록이 없으면 그대로 제 것이다 — 적는 것 자체는 그대로다(그 줄이 정말 돌았을 수 있다).
        assert_eq!(read(std::slice::from_ref(&maybe)).mine, set(&["t-1"]), "적기는 적는다");
        // **`?` 끼리는 마지막이 이긴다**(moai-kjeh, 사용자 결정 2026-09-20) — 두 세션이 모두 `&&` 로
        // 집으면 양쪽이 `?` 다. 겨루는 자를 안 두던 판은 그 줄을 어느 쪽에도 안 줘 주인 없이 남겼다.
        let their_early = ("you", Picks::line(1, None, "t-1", false));
        let mine_late = ("me", Picks::line(2, None, "t-1", false));
        assert_eq!(read(&[their_early.clone(), mine_late]).mine, set(&["t-1"]), "`?` 끼리 겨뤄 주인이 없어졌다");
        // 늦게 적은 쪽이 남이면 남의 것이다 — 같은 때면 모르는 쪽이다(위와 같은 자).
        assert!(read(&[("me", Picks::line(1, None, "t-1", false)), ("you", Picks::line(2, None, "t-1", false))]).mine.is_empty());
        assert!(read(&[("me", Picks::line(1, None, "t-1", false)), ("you", Picks::line(1, None, "t-1", false))]).mine.is_empty());
        // 남의 **확실한** 줄은 제 `?` 가 아무리 늦어도 못 가져온다 — 결정 2 가 그대로 선다.
        assert!(read(&[("you", Picks::line(1, None, "t-1", true)), ("me", Picks::line(9, None, "t-1", false))]).mine.is_empty());
        // 내가 뒤에 확실히 집으면 다시 제 것이다 — 표는 줄마다 선다.
        assert_eq!(read(&[theirs.clone(), maybe.clone(), ("me", Picks::line(3, None, "t-1", true))]).mine, set(&["t-1"]));
        // **뒤에 선 `?` 한 줄이 앞서 확실히 집은 줄을 안 덮는다**(리뷰 moai-51h9.k8j1) — 마지막 한 줄만
        // 들던 판은 정말 쥔 줄을 남에게 내줬다. 같은 때의 두 줄도 적는 차례를 안 탄다.
        let later = ("me", Picks::line(3, None, "t-1", false));
        assert_eq!(read(&[theirs.clone(), sure.clone(), later]).mine, set(&["t-1"]), "뒤의 `?` 가 확실한 집기를 덮었다");
        let tie = ("me", Picks::line(2, None, "t-1", true));
        assert_eq!(read(&[theirs.clone(), maybe, tie]).mine, set(&["t-1"]), "같은 때의 두 줄이 차례를 탔다");

        // **남이 적은 `?` 줄도 같은 뜻으로 읽는다**(moai-dbzs, 사용자 결정 2026-09-20) — 표를 쓰기만
        // 하고 읽을 때 버리던 판은, 옆 세션의 안 돈 집기 한 줄이 이 세션이 확실히 쥔 줄을 가져갔다
        // (리뷰 moai-51h9.q5l 의 6번).
        let their_maybe = ("you", Picks::line(3, None, "t-1", false));
        let held = read(&[sure.clone(), their_maybe.clone()]);
        assert_eq!(held.mine, set(&["t-1"]), "남의 `?` 한 줄이 제 집기를 가져갔다 — {held:?}");
        // 그래도 읽기는 읽는다 — 제 기록이 없는 줄은 "그쪽이 쥐었을 수 있다" 로 남는다. 모르는 줄은
        // 풀기만 하니 이것으로 새로 막지 않는다(moai-4jsy).
        let only_theirs = read(std::slice::from_ref(&their_maybe));
        assert!(only_theirs.mine.is_empty());
        assert_eq!(only_theirs.theirs, set(&["t-1"]), "남의 `?` 줄을 아예 안 읽었다");
        // 남이 뒤에 **확실히** 집었으면 넘어간다 — 마지막으로 집은 세션이 이기는 자는 그대로다(moai-4jsy).
        assert!(read(&[sure.clone(), ("you", Picks::line(3, None, "t-1", true))]).mine.is_empty(), "넘겨준 줄을 붙들었다");
        // `?` 끼리 겨루는 자는 위에 있다(moai-kjeh) — 마지막에 적은 쪽이 이긴다.
        let both_maybe = read(&[("me", Picks::line(4, None, "t-1", false)), their_maybe]);
        assert_eq!(both_maybe.mine, set(&["t-1"]), "뒤에 적은 `?` 가 졌다 — {both_maybe:?}");

        // **표가 없는 옛 줄은 확실한 것으로 읽는다** — 새 칸을 늘리기만 했다.
        let old = ("me", "2\tt-1\t\n".to_string());
        assert_eq!(read(&[theirs.clone(), old]).mine, set(&["t-1"]), "옛 줄을 못 읽었다");
        // 두 칸뿐인 더 옛 줄(`때\tid`)도 같다 — 옛 바이너리가 `초` 를 못 읽었을 때 적던 꼴이다.
        assert_eq!(read(&[theirs, ("me", "2\tt-1\n".to_string())]).mine, set(&["t-1"]), "두 칸짜리 옛 줄을 못 읽었다");
        assert!(Picks::line(7, Some(42), "t-1", false).ends_with("\t?\n"), "{:?}", Picks::line(7, Some(42), "t-1", false));
        assert_eq!(Picks::line(7, Some(42), "t-1", true), "7\tt-1\t42\n");
    }

    /// **세션의 기록이 이름보다 앞이다**(moai-4jsy) — 다만 **제 이름이 거리 0 으로 가리키는 줄은
    /// 기록보다 앞이다**(사용자 결정 2026-09-19, moai-u8al). 남의 워크트리에 들어간 세션은 그 이름이
    /// 에픽·조상으로 가리키는 줄을 모른다고 하지만, `worktree-<그 id>` 자리의 줄은 이어받은 세션의
    /// 것으로 본다 — 거두기와 `/clear` 뒤의 새 세션은 앞 세션의 기록을 못 물려받는다.
    ///
    /// **제 기록은 남의 기록만 지운다**(moai-ydtm) — 옆 스냅샷이 쥔 줄은 그대로 모른다. 안 돌 수도 있는
    /// 토막이 적은 기록 하나가 새로 막지 않게 한다. 기록이 없는 줄은 전과 같다.
    #[test]
    fn a_pick_another_session_recorded_is_not_mine() {
        let all = vec![issue("t-1", "in_progress"), issue("t-2", "in_progress"), issue("t-3", "in_progress")];
        let picks = Picks { mine: set(&["t-2"]), theirs: set(&["t-1", "t-2"]) };
        let got: Vec<String> = unsure(&all, &cfg(), &set(&["t-2", "t-3"]), &set(&["t-1"]), &picks).into_iter().collect();
        assert_eq!(got, ["t-2", "t-3"], "제 이름 자리의 줄을 기록에 넘겼거나, 옆이 쥔 줄을 제 기록으로 지웠다");

        // 제 이름이 안 가리키는 줄은 남의 기록이 이긴다 — 그것이 moai-4jsy 다.
        let picks = Picks { theirs: set(&["t-1"]), ..Picks::default() };
        let got: Vec<String> = unsure(&all, &cfg(), &set(&[]), &set(&["t-9"]), &picks).into_iter().collect();
        assert_eq!(got, ["t-1"]);
        // 기록도 옆도 없으면 아무것도 모르지 않는다.
        assert!(unsure(&all, &cfg(), &set(&[]), &set(&[]), &Picks::default()).is_empty());

        // 집는 id 는 `only` 가 고른 `mv … <벌여 놓는 칸>` 에서만 읽는다.
        let stands = |_: usize, _: &str| None;
        let cmd = "moai mv t-1 in_progress --from todo && moai mv t-2 done && moai -C /x mv t-3 review";
        assert_eq!(picked_ids(cmd, &cfg(), &|_| true, &stands), ["t-1", "t-3"]);
        assert_eq!(picked_ids(cmd, &cfg(), &|k| k == 0, &stands), ["t-1"]);
        assert!(picked_ids("moai mv t-1 in_progress --help", &cfg(), &|_| true, &stands).is_empty());
    }

    /// **기록하는 집기는 쓰기 규칙이 집기로 센 것과 같다**(moai-m5mg, 사용자 결정) — 이음사를 안 보던
    /// 판은 `! moai mv Y …`·`a || moai mv Y …` 처럼 **안 돌 수도 있는** 토막의 id 까지 이 세션의 집기로
    /// 적었다. 그 기록 하나가 남이 쥔 줄을 제 것으로 붙들어, 규칙 1 이 그 줄로 막고 `Stop` 이 남의 일을
    /// 닫으라고 댔다.
    #[test]
    fn a_pick_that_may_not_run_is_not_recorded_as_mine() {
        let stands = |_: usize, _: &str| None;
        let mine = |cmd: &str| picked_ids(cmd, &cfg(), &|_| true, &stands);
        for cmd in [
            "! moai mv t-1 in_progress",
            "! (moai mv t-1 in_progress)",
            "make build || moai mv t-1 in_progress",
            "cargo test || (moai mv t-1 in_progress)",
        ] {
            assert!(mine(cmd).is_empty(), "안 돌 수도 있는 집기를 적었다 — {cmd}");
        }
        // 앞도 집기면 어느 쪽이든 하나를 쥔다 — 쓰기 규칙과 같은 자다.
        assert_eq!(mine("moai mv t-1 in_progress --from todo || moai mv t-2 in_progress"), ["t-1", "t-2"]);
        // **그 둘 가운데 뒤엣것은 `?` 로 적힌다**(moai-hze6) — 앞이 이기면 안 돈다. 표를 내는 자를
        // 여기서 못박는다: `Picks::fold` 만 재던 판은 `sure` 를 늘 `true` 로 되돌려도 초록이었다.
        assert_eq!(
            picked_in("moai mv t-1 in_progress --from todo || moai mv t-2 in_progress", &cfg(), &|_| true, &stands),
            [("t-1".to_string(), true), ("t-2".to_string(), false)]
        );
        // **`&&` 로 이은 집기도 `?` 로 적는다**(moai-dbzs, 사용자 결정 2026-09-20) — 앞이 지면 안 돈다.
        // `||` 만 재던 판은 `cargo test && moai mv X review` 를 확실한 집기로 적어, 시험이 져서 안 돈
        // 집기가 정말 쥔 세션에게서 초점을 빼앗았다(리뷰 moai-51h9.q5l 의 7번).
        let flags = |cmd: &str| picked_in(cmd, &cfg(), &|_| true, &stands);
        let one = |id: &str, sure: bool| vec![(id.to_string(), sure)];
        assert_eq!(flags("cargo test && moai mv t-1 review"), one("t-1", false));
        // 조건에 매여 들어선 묶음 안도 같다 — 앞이 집기여도 그 집기가 질 수 있고, 기록은 명령이 돌기
        // 전에 적힌다. 쓰기 규칙의 `iffy` 가 이 자리를 안 세는 것과 갈리는 곳이다.
        assert_eq!(flags("a && { moai mv t-1 in_progress; }"), one("t-1", false));
        // **묶음 안의 뒤엣줄이 `chancy` 를 실제로 부린다**(리뷰 moai-51h9.3jh) — 첫 줄은 바깥
        // 이음사를 제 것으로 들고 와 `j.op` 만으로 갈려, `chancy` 를 통째로 지워도 안 붉어졌다.
        assert_eq!(flags("a && { echo x; moai mv t-1 in_progress; }"), one("t-1", false));
        assert_eq!(flags("a && ( echo x; moai mv t-1 in_progress )"), one("t-1", false));
        assert_eq!(
            flags("moai mv t-1 in_progress --from todo && { moai mv t-2 in_progress; }"),
            [("t-1".to_string(), true), ("t-2".to_string(), false)]
        );
        // 그 묶음을 나오면 표는 걷힌다 — 형제 묶음도 다시 선 목록도 확실한 집기다.
        assert_eq!(flags("a && { echo x; }; { moai mv t-1 in_progress; }"), one("t-1", true));
        assert_eq!(flags("a && b; moai mv t-1 in_progress"), one("t-1", true));
        // 셸에 넘긴 글도 그 토막이 매였으면 함께 매인다. 겹 안에서 선 표는 겹 밖으로 안 샌다.
        assert_eq!(flags("bash -c 'moai mv t-1 in_progress'"), one("t-1", true));
        assert_eq!(flags("cargo test && bash -c 'moai mv t-1 in_progress'"), one("t-1", false));
        assert_eq!(flags("bash -c 'a && b'; moai mv t-1 in_progress"), one("t-1", true));
        // **`set -e` 아래의 `;` 도 `&&` 다**(리뷰 moai-51h9.3jh) — 껍데기가 앞에서 끝나면 집기가 안
        // 돈다. 쓰기 규칙은 이미 그렇게 고쳐 읽는데(`j.op = Op::And`) 기록만 **고치기 전의** 이음사로
        // 재던 판은 같은 뜻을 `&&` 로 쓴 줄만 `?` 로 적었다. 대가: 앞이 `set -e` 뿐이어도 `?` 다 —
        // 앞 명령이 무엇인지는 안 보기 때문이고, 안 돈 것을 확실하다고 적는 쪽이 더 비싸다.
        assert_eq!(flags("set -e; cargo test; moai mv t-1 review"), one("t-1", false));
        assert_eq!(flags("set -e\ncargo test\nmoai mv t-1 review"), one("t-1", false));
        assert_eq!(flags("bash -c 'set -e; cargo test; moai mv t-1 review'"), one("t-1", false));
        assert_eq!(flags("set -e; moai mv t-1 in_progress"), one("t-1", false));
        // `set -e` 를 안 켰으면 그대로다 — `;` 뒤는 앞이 져도 돈다.
        assert_eq!(flags("cargo test; moai mv t-1 review"), one("t-1", true));
        // **파이프의 칸은 제 머리의 답을 잇는다**(`piped`) — `|` 는 `&&`·`||` 보다 단단히 묶여 머리가
        // 안 돌면 칸도 안 돈다. 제 이음사(`Op::Pipe`)만 보던 판은 그 집기를 확실한 것으로 적어,
        // moai-hze6 이 닫으려던 `||` 자리마저 샜다.
        assert_eq!(flags("cargo test && echo x | moai mv t-1 in_progress"), one("t-1", false));
        assert_eq!(flags("a || echo x | moai mv t-1 in_progress"), one("t-1", false));
        assert_eq!(flags("a && b | { moai mv t-1 in_progress; }"), one("t-1", false));
        // 머리가 확실하면 칸도 확실하다 — 파이프라인은 제 칸을 모두 돌린다.
        assert_eq!(flags("echo x | moai mv t-1 in_progress"), one("t-1", true));
        // **끝내는 묶음의 조건도 같은 자로 잰다**([`Bailout::cond`]) — 그 집기는 뒤집혀 있어 제 자리에
        // 안 적히고 묶음을 지나며 세워지는데, 꼬리를 `true` 로 박아 두던 판은 `if` 자신이 매인 줄에서
        // 안 돌 수도 있는 집기를 확실한 것으로 적었다.
        assert_eq!(flags("if ! moai mv t-1 in_progress --from todo; then exit 1; fi"), one("t-1", true));
        assert_eq!(flags("cargo test && if ! moai mv t-1 in_progress --from todo; then exit 1; fi"), one("t-1", false));
        assert_eq!(flags("set -e; cargo test; if ! moai mv t-1 in_progress --from todo; then exit 1; fi"), one("t-1", false));
        // **겹을 나오며 세우는 길도 같은 자로 잰다** — 앞이 집기라 `iffy` 가 비어, 이 줄만 `Layer::Shell`
        // 을 나오는 `credit` 으로 세워진다. 앞의 줄들은 `fi` 로 끝나 끝자락의 `credit` 으로 가니,
        // 꼬리를 `true` 로 되돌리면 여기가 그것을 잡는다.
        assert_eq!(
            flags("moai mv t-0 in_progress --from todo && bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi'"),
            [("t-0".to_string(), true), ("t-1".to_string(), false)]
        );
        // **넘긴 글 안의 집기는 제 표와 `if` 의 표를 둘 다 든다**(`sure && certain`) — 한쪽만 들면
        // 어느 쪽으로도 샌다. 둘 중 무엇을 지워도 안 붉어지던 자리다(리뷰 moai-51h9.3jh): 앞 줄이
        // 글 안의 표를, 뒤 줄이 `if` 자신의 표를 부린다.
        assert_eq!(flags("if ! bash -c 'a && moai mv t-1 in_progress'; then exit 1; fi"), one("t-1", false));
        assert_eq!(flags("cargo test && if ! bash -c 'moai mv t-1 in_progress'; then exit 1; fi"), one("t-1", false));
        // 남의 트래커를 가리킨 토막은 그 트래커에 적힌다 — 여기서는 `only` 가 뺀다.
        assert_eq!(picked_ids("moai -C /x mv t-9 in_progress && moai mv t-1 in_progress", &cfg(), &|k| k == 1, &stands), ["t-1"]);
        // 첫 칸으로 되돌리는 것과 닫는 것은 벌여 놓는 칸이 아니다.
        assert!(mine("moai mv t-1 todo && moai mv t-2 done").is_empty());
        // 셸에 넘긴 글 안의 집기도 센다(moai-k8j1) — 그 글은 명령이다.
        assert_eq!(mine("bash -c 'moai mv t-1 in_progress'"), ["t-1"]);
        // **쓰기 규칙이 이긴 것으로 세는 꼴은 기록도 센다**(moai-9xbq) — 겹을 넘어도 같다.
        assert_eq!(mine("bash -c 'moai mv t-1 in_progress --from todo || exit 1; echo done'"), ["t-1"]);
        assert_eq!(mine("if ! bash -c 'moai mv t-1 in_progress --from todo'; then exit 1; fi"), ["t-1"]);
        assert_eq!(mine("moai mv t-1 in_progress --from todo || { bash -c 'echo fail'; exit 1; }"), ["t-1"]);
        // 안 도는 글의 집기는 적지 않는다 — `-c` 가 스크립트의 인자인 줄이다(moai-j9tx 의 곁).
        assert!(mine("bash -e script.sh -c 'moai mv t-1 in_progress --from todo'").is_empty());

        // **몸통이 안 돌 수도 있는 묶음은 뒤 토막이 없어도 걷는다**([`parse_over`]) — 표식을 받을
        // 토막이 없어 버려지던 판은 `fi` 로 끝나는 줄에서만 안 돈 집기를 적었다. 몸통 안의 `;` 가
        // 사슬을 이미 끊은 판도 같다 — 걷기를 `after_pick` 에 매던 자리다.
        for cmd in [
            "if false; then moai mv t-1 in_progress; fi",
            "if false; then moai mv t-1 in_progress; echo skip; fi",
            "if false; then moai mv t-1 in_progress; echo skip; fi; sed -i s/a/b/ src/x.rs",
            "for f in a b; do moai mv t-1 in_progress; echo ok; done; echo hi",
            "while read x; do moai mv t-1 in_progress; echo ok; done",
            "case x in a) moai mv t-1 in_progress; echo ok;; esac",
            "make build || { moai mv t-1 in_progress; }",
            "a || x=$(moai mv t-1 in_progress)",
        ] {
            assert!(mine(cmd).is_empty(), "안 돌 수도 있는 집기를 적었다 — {cmd}");
        }

        // **늘 도는 집기는 뒤에 무엇이 오든 남는다** — 깊이로만 걷던 판은 뒤에 선 남남의 `|| { … }`
        // 와 `if …; fi` 로 앞선 형제 묶음의 집기까지 버렸다. 잃은 기록은 제 줄을 "옆이 쥐었을 일" 로
        // 세워, 규칙 2 가 제 쓰기를 막는다.
        for cmd in [
            "( moai mv t-1 in_progress; echo x ); a || ( echo y ); echo hi",
            "{ moai mv t-1 in_progress; echo x; }; a || { echo y; }; echo hi",
            "(moai mv t-1 in_progress); if true; then echo a; fi; sed -i s/a/b/ src/x.rs",
            "(moai mv t-1 in_progress); if true; then moai mv t-2 in_progress; fi; sed -i s/a/b/ src/x.rs",
            "(moai mv t-1 in_progress); false || (moai mv t-2 in_progress); echo x",
        ] {
            assert_eq!(mine(cmd), ["t-1"], "늘 도는 집기를 버렸다 — {cmd}");
        }

        // **치환은 바깥 토막보다 먼저 돈다** — 이 저장소가 스스로 일러 주는 겨루기 꼴(`src/guide.rs`)이
        // 그 모양이다. 깊이로만 걷던 판은 뒤의 `|| { … }` 로 이미 돈 집기를 버려, 겨뤄 이긴 세션이
        // 제 줄을 안 적었다.
        for cmd in [
            "claim=$(moai mv t-1 in_progress --json --from todo) || { [ -n \"$claim\" ] || exit 1; continue; }",
            "claim=$(moai mv t-1 in_progress) || { echo lost; continue; }; echo x > src/store.rs",
        ] {
            assert_eq!(mine(cmd), ["t-1"], "치환 안의 집기를 버렸다 — {cmd}");
        }

        // **`if ! 집기; then exit 1; fi` 는 쓰기 규칙이 집은 것으로 센다**(moai-ncay) — 기록도 같다.
        // 한쪽만 세던 판이 바로 이 규칙 글이 못 박은 "쓰기에는 빈손, 기록에는 제 집기" 의 거울이었다.
        for cmd in [
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi",
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi; sed -i s/a/b/ src/x.rs",
        ] {
            assert_eq!(mine(cmd), ["t-1"], "집기가 이긴 묶음을 안 적었다 — {cmd}");
            assert!(shell_writes(cmd, &cfg(), &|_| true).is_empty(), "쓰기 규칙과 갈렸다 — {cmd}");
        }

        // **남의 트래커의 집기도 제 사슬로 잰다** — `only` 를 셈에 넘기던 판은 그 판에서 제 토막의
        // 집기가 안 세어져, 앞이 지면 정말 도는 `a || moai -C /x mv B …` 의 B 를 통째로 버렸다.
        let his = |k: usize| k == 1;
        assert_eq!(picked_ids("moai mv t-1 in_progress || moai -C /x mv t-2 in_progress", &cfg(), &his, &stands), ["t-2"]);
        assert_eq!(picked_ids("moai mv t-1 in_progress && moai -C /x mv t-2 in_progress", &cfg(), &his, &stands), ["t-2"]);
    }

    /// **모르는 줄 밑에서 제가 집은 자식은 제 것이다**(리뷰 moai-3k2d.1df). 모름은 그 줄과 **그 밑을**
    /// 통째로 빼므로, 남이 부모를 집고 이 세션이 그 자식(`--parent` 로 세운 리뷰 줄)을 집으면 제 일이
    /// 초점에서 통째로 빠졌다 — `Stop` 이 안 붙들고, 좁힌 판정이 `-m` 없는 리뷰 닫기를 넘겼다.
    #[test]
    fn my_own_pick_under_an_unsure_parent_stays_mine() {
        let all = vec![issue("t-1", "in_progress"), issue("t-1.aa", "in_progress")];
        let unsure = Away { unsure: set(&["t-1"]), ..Away::default() };
        assert!(held(&all, &cfg(), &unsure).is_empty(), "모르는 줄과 그 밑을 안 뺐다");
        let mine = Away { picked: set(&["t-1.aa"]), ..unsure };
        let focus: Vec<&str> = held(&all, &cfg(), &mine).iter().map(|i| i.id.as_str()).collect();
        assert_eq!(focus, ["t-1.aa"], "제가 집은 자식을 부모의 모름에 딸려 보냈다");
    }

    /// **집기가 지면 끝내는 묶음을 지나면 집기가 이긴 채다**(moai-ncay) — `집기 || { …; exit 1; }` 와
    /// `if ! 집기; then exit 1; fi` 다. `|| exit` 한 꼴만 알던 판은 겨루다 진 쪽을 끊는 흔한 두 꼴에서
    /// 집기를 잃어, 시킨 대로 쓴 줄을 규칙 2 로 막았다(잘못 막음).
    ///
    /// **끝내지 않는 묶음은 그대로 막는다** — 그 묶음이 돌고도 뒤가 돌면 집기 없이 쓰는 것이다.
    /// 집기 없이 `||` 로 연 묶음도 마찬가지다.
    #[test]
    fn a_group_that_ends_the_shell_keeps_the_pick() {
        let root = Path::new("/repo");
        let wrote = |cmd: &str| guard_writes(&[], &cfg(), &here(), root, root, cmd);
        for cmd in [
            "moai mv t-1 in_progress --from todo || { echo lost >&2; exit 1; }; sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { echo lost; exit 1; }\nsed -i s/a/b/ src/x.rs",
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi; sed -i s/a/b/ src/x.rs",
            "if ! moai mv t-1 in_progress --from todo; then echo lost; exit 1; fi; sed -i s/a/b/ src/x.rs",
            // **바로 뒤에 형제 묶음이 서도 같다**(리뷰 moai-p836.rv). 묶음을 나온 것을 `seg.level` 로
            // 보던 판은 같은 깊이에서 곧바로 열린 `for`·`if`·`{ }`·`( )` 를 그 묶음 안으로 읽어,
            // 집기를 도로 못 세우고 시킨 대로 쓴 줄을 막았다 — 옆의 `sure`·`strict` 와 같은
            // `seg.floor` 를 쓴다.
            "moai mv t-1 in_progress --from todo || { exit 1; }\nfor f in a b; do sed -i s/a/b/ src/x.rs; done",
            "moai mv t-1 in_progress --from todo || { exit 1; }; { sed -i s/a/b/ src/x.rs; }",
            "moai mv t-1 in_progress --from todo || { exit 1; }; ( sed -i s/a/b/ src/x.rs )",
            "moai mv t-1 in_progress --from todo || { exit 1; }\nif true; then tee src/x.rs; fi",
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi\nfor f in a b; do sed -i s/a/b/ src/x.rs; done",
            // **`&`·`|` 가 다는 표는 방금 닫힌 묶음까지다**(리뷰 moai-k8j1.udq 의 4·5번) — 깊이만
            // 보고 끝까지 훑던 판은 앞선 형제 묶음(`|| { exit 1; }`)까지 띄운 것으로 세, 그 안의
            // `exit` 를 잃고 시킨 대로 친 줄을 막았다.
            "moai mv t-1 in_progress --from todo || { echo fail; exit 1; }; { echo z; } & sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { echo fail; exit 1; }; { echo z; } | cat; sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { echo fail; exit 1; }; ( echo z ) & sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { exit 1; }; if true; then echo y; fi & sed -i s/a/b/ src/x.rs",
        ] {
            assert_eq!(wrote(cmd), Decision::Pass, "집기가 이긴 채인데 막았다 — {cmd}");
        }
        for cmd in [
            // 묶음이 안 끝낸다 — 집기가 져도 뒤가 돈다.
            "moai mv t-1 in_progress --from todo || { echo lost >&2; }; sed -i s/a/b/ src/x.rs",
            // **뒤로 띄운 묶음은 껍데기를 안 끝낸다**(moai-99df) — `&` 로 띄운 `{ …; exit 1; }` 은 제
            // 하위 셸만 끝내고 바깥은 그대로 돈다. `Op::Any` 가 `;`·`&` 를 한 낱말로 읽어 그 `exit`
            // 를 껍데기의 끝으로 세던 판은 집기 없이 쓰는 줄을 넘겼다.
            "moai mv t-1 in_progress --from todo || { echo lost; exit 1; } & sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || ( echo lost; exit 1 ) & sed -i s/a/b/ src/x.rs",
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi & sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || exit 1 & sed -i s/a/b/ src/x.rs",
            "if ! moai mv t-1 in_progress --from todo; then echo lost; fi; sed -i s/a/b/ src/x.rs",
            // 몸통에 `exit` 아닌 길이 있다 — 그 길로 오면 집기는 졌다.
            "if ! moai mv t-1 in_progress --from todo; then exit 1; else echo ok; fi; sed -i s/a/b/ src/x.rs",
            // 집기가 아니라 딴 명령이 앞에 섰다.
            "grep x f || { echo lost; exit 1; }; sed -i s/a/b/ src/x.rs",
            // **하위 셸의 `exit` 는 그 괄호만 끝낸다**(리뷰 moai-p836.rv) — 바깥 셸은 살아 있어 뒤가
            // 그대로 돈다. 괄호 깊이를 안 보던 판은 이 줄을 집기 뒤로 읽어 샜다.
            "moai mv t-1 in_progress --from todo || ( echo lost >&2; exit 1 )\nsed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { echo x; ( exit 1 ); }; sed -i s/a/b/ src/x.rs",
            "( moai mv t-1 in_progress --from todo || { exit 1; } ); sed -i s/a/b/ src/x.rs",
            // **늘 도는 자리에 선 `exit` 여야 한다** — 앞이 이겨야 도는 것은 안 돌 수 있고, 그러면
            // 묶음이 0 으로 끝나 뒤가 그대로 돈다. 머리만 보던 판은 두 줄 다 샜다.
            "moai mv t-1 in_progress --from todo || { false && exit 1; }; sed -i s/a/b/ src/x.rs",
            "moai mv t-1 in_progress --from todo || { echo lost >&2 || exit 1; }; sed -i s/a/b/ src/x.rs",
            // **감싸는 명령은 붙박이를 못 돌린다** — `sudo exit` 는 그냥 지고 껍데기는 살아 있다.
            "moai mv t-1 in_progress --from todo || sudo exit 1; sed -i s/a/b/ src/x.rs",
        ] {
            assert!(matches!(wrote(cmd), Decision::Deny(_)), "집기 없이 쓰는 줄이 샜다 — {cmd}");
        }
        // **`&` 로 띄운 묶음은 아직 못 가른다** — 렉서가 `&` 와 `;` 를 한 이음사(`Op::Any`)로 읽어,
        // `집기 || { …; exit 1; } & 쓰기` 의 `exit` 가 제 하위 셸만 끝내는 것을 여기서 모른다.
        // 그 줄은 지금 지나간다(샌다). 흔한 꼴이 아니라 적어 두고 idea 로 넘긴다(moai-ncay 의 노트).
    }

    /// **감싸는 명령은 명령 자리를 안 가린다**(moai-455j) — `env`·`timeout`·`nice`·`stdbuf`·`sudo` 뒤의
    /// 명령을 규칙이 본다. 값을 먹는 옵션도 안다. 모르는 꼴이면 **거기서 멈춘다** — 넘겨짚어 엉뚱한
    /// 낱말을 명령으로 읽으면 새는 것보다 나쁜 잘못 막음이 난다.
    #[test]
    fn a_wrapping_command_does_not_hide_what_it_runs() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        for cmd in [
            "env moai add '딴 일'",
            "env -u MOAI_NOW moai add '딴 일'",
            "env -i FOO=1 moai add '딴 일'",
            "timeout 5 moai add '딴 일'",
            "timeout -k 1 5s moai add '딴 일'",
            "nice -n 5 moai add '딴 일'",
            "nice -5 moai add '딴 일'",
            "stdbuf -o0 moai add '딴 일'",
            "sudo moai add '딴 일'",
            "sudo -u 남 -- moai add '딴 일'",
            "env timeout 5 sudo moai add '딴 일'",
            // **값을 붙여서만 받는 옵션은 다음 낱말을 안 먹는다**(리뷰 moai-p836.rv) —
            // `env --block-signal[=SIG]` 셋과 `sudo -h[HOST]` 다. 먹던 판은 `moai` 를 값으로 삼켜
            // `add` 를 명령으로 읽었다.
            "env --block-signal moai add '딴 일'",
            "env --default-signal moai add '딴 일'",
            "env --ignore-signal moai add '딴 일'",
            "env --block-signal=INT moai add '딴 일'",
            "sudo -h moai add '딴 일'",
            "sudo -hHOST moai add '딴 일'",
            // **`--` 는 옵션만 끝낸다** — 제 자리 인자는 그 뒤에 온다.
            "timeout -- 5 moai add '딴 일'",
            // **값을 따로 받는 짧은 옵션을 빠뜨리면 그 값이 명령으로 읽힌다.**
            "sudo -T 5 moai add '딴 일'",
            "sudo -R /chroot moai add '딴 일'",
            "sudo -a krb5 moai add '딴 일'",
            "sudo -c fast moai add '딴 일'",
            // **뭉친 스위치의 값 받는 글자가 맨 앞이 아니어도 읽는다** — 멈추던 판은 흔한
            // `sudo -nu 남` 과 `env -iu FOO` 에서 규칙을 통째로 껐다.
            "sudo -nu 남 moai add '딴 일'",
            "env -iu FOO moai add '딴 일'",
            // `env -` 는 `-i` 다(환경을 비운다). 자리 인자로 읽던 판은 `-` 를 명령으로 읽었다.
            "env - moai add '딴 일'",
            "nice --10 moai add '딴 일'",
            "sudo --command-timeout 5 moai add '딴 일'",
            "sudo --bell moai add '딴 일'",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "감싸는 명령이 규칙 1 을 가렸다 — {cmd}");
        }
        // 빈손의 쓰기도 같다.
        for cmd in ["env sed -i s/a/b/ src/x.rs", "sudo tee src/x.rs", "timeout 5 tee -a src/x.rs"] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "감싸는 명령이 규칙 2 를 가렸다 — {cmd}\n{got:?}");
        }
        // **모르는 긴 옵션**에서는 멈춘다 — 값을 따로 받는 것이면 그 값을 명령으로 읽어, 새는
        // 것보다 나쁜 잘못 막음이 난다. 뒤의 명령을 **아예 안 돌리는** 것(`sudo -l`·`-v`·`doas -C`)과
        // **딴 자리에서** 돌리는 것(`env -C`·`sudo -D`)도 같다 — 자리를 넘겨 주면 남의 트래커의
        // 집기가 여기 규칙 2 를 채운다(리뷰 moai-p836.rv).
        //
        // **셸을 여는 스위치 둘은 이제 여기가 아니다**(moai-drli) — `env -S`·`sudo -s` 뒤는 명령이
        // 아니라 글이고, 그 글은 [`shell_text`] 가 읽는다
        // (`a_shell_opening_switch_hands_its_text_to_the_lexer`).
        //
        // **`sudo -i` 와 `doas -s` 는 여기 남는다**(2026-09-20 사용자 결정, 리뷰 moai-jlon.yeg
        // 5·6번) — 로그인 셸은 대상 사용자의 홈에서 돌고(`sudo -D` 와 같은 자리), `doas -s` 는
        // argv 를 `$SHELL` 로 갈아치워 뒤의 명령을 아예 안 돌린다(`doas -C` 와 같은 자리).
        // 넘겨 주면 **돌지도 않거나 딴 트래커에서 도는 집기**가 여기 규칙 2 를 채운다.
        for cmd in [
            "env --weird moai add '딴 일'",
            "sudo -l moai add '딴 일'",
            "sudo -v moai add '딴 일'",
            "doas -C /etc/doas.conf moai add '딴 일'",
            "env -C /남의/저장소 moai add '딴 일'",
            "env --chdir=/남의/저장소 moai add '딴 일'",
            "sudo -D /남의/저장소 moai add '딴 일'",
            "sudo --chdir /남의/저장소 moai add '딴 일'",
            "sudo -i moai add '딴 일'",
            "sudo --login moai add '딴 일'",
            "sudo -i -n moai add '딴 일'",
            "doas -s moai add '딴 일'",
            "doas -u 남 -s moai add '딴 일'",
            "doas -su 남 moai add '딴 일'",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "모르는 꼴을 명령으로 읽었다 — {cmd}");
        }
        // **돌지도 않는 집기로 빈손의 쓰기가 풀리지 않는다** — 멈추는 까닭이 바로 이것이다.
        for cmd in [
            "sudo -i moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/x.rs",
            "doas -s moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/x.rs",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "딴 자리에서 도는 집기가 규칙 2 를 채웠다 — {cmd}\n{got:?}");
        }
        // **감싸는 명령은 껍데기 붙박이를 못 돌린다**(리뷰 moai-p836.rv) — `sudo cd`·`env set -e` 는
        // 그런 이름의 프로그램이 없어 그냥 진다. 넘어가서 읽던 판은 자리를 옮긴 것으로·errexit 를
        // 켠 것으로 세, 그 뒤의 쓰기를 통째로 넘겼다.
        for cmd in [
            "sudo cd /tmp; sed -i s/a/b/ src/x.rs",
            "env cd /tmp; sed -i s/a/b/ src/x.rs",
            "env set -e; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/x.rs",
            // 남의 트래커에 선 집기는 여기 규칙 2 를 못 채운다 — 자리를 못 따라가니 안 넘는다.
            "env -C /남의/저장소 moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/x.rs",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "감싸는 명령이 붙박이를 돌린 것으로 읽었다 — {cmd}\n{got:?}");
        }
        // 감싸는 명령 혼자는 그 자체로 명령이다(`env` 는 환경을 찍는다).
        assert_eq!(command_of(&["env".to_string()]).first().map(String::as_str), Some("env"));
    }

    /// **셸에 넘긴 글은 명령이다**(moai-455j) — `bash -c '…'`·`eval '…'` 의 글을 렉서가 다시 읽는다.
    /// 안 읽던 판은 그 한 낱말 뒤에서 규칙 1~2 가 통째로 샜다. 끝없이 파고들지는 않는다.
    #[test]
    fn a_string_handed_to_a_shell_is_read_as_commands() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        for cmd in ["bash -c \"moai add '딴 일'\"", "sh -c \"moai add '딴 일'\"", "eval \"moai add '딴 일'\"", "bash -lc \"moai add '딴 일'\""] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "셸에 넘긴 글을 안 읽었다 — {cmd}");
        }
        let wrote = guard_writes(&[], &cfg(), &here(), root, root, "bash -c \"echo x > src/store.rs\"");
        assert!(matches!(wrote, Decision::Deny(_)), "셸에 넘긴 쓰기를 안 봤다 — {wrote:?}");
        // 겹쳐도 판정은 같고, 상한을 넘으면 글로 둔다(끝없이 안 돈다).
        let deep = "bash -c \"bash -c \\\"moai add '딴 일'\\\"\"";
        assert!(matches!(guard_create(&all, &cfg(), &here(), deep), Decision::Deny(_)), "두 겹을 안 읽었다");
        // **상한은 되돌이가 스택을 넘지 않게 하는 것뿐이다** — 실제 명령줄이 닿을 자리가 아니다.
        // 여덟이던 판은 `$( … )` 아홉 겹 46바이트가 규칙 1 을 껐다(리뷰 moai-p836.rv).
        let nest = |n: usize| format!("{}moai add '딴 일'{}", "$(".repeat(n), ")".repeat(n));
        assert!(matches!(guard_create(&all, &cfg(), &here(), &nest(9)), Decision::Deny(_)), "아홉 겹 치환이 규칙 1 을 껐다");
        assert_eq!(guard_create(&all, &cfg(), &here(), &nest(Lexer::DEEP + 4)), Decision::Pass, "상한 없이 파고든다");
        // 상한이 실제로 되돌이를 막는다 — 막던 것이 없으면 여기서 스택이 넘쳐 훅이 통째로 죽는다.
        let clock = std::time::Instant::now();
        let _ = guard_create(&all, &cfg(), &here(), &nest(5000));
        assert!(clock.elapsed() < std::time::Duration::from_secs(5), "겹을 안 막는다 — {:?}", clock.elapsed());
        // **그 글은 제 토막 자리에 심는다**(리뷰 moai-p836.rv) — 뒤에 몰아 쌓던 판은 줄 끝의 판을
        // 물려받아, 앞선 `&&` 집기를 잃고(잘못 막음) 뒤따르는 `cd` 로 제 쓰기를 지웠다(샜다).
        for cmd in [
            "moai mv t-1 in_progress --from todo && bash -c 'sed -i s/a/b/ src/x.rs'",
            "moai mv t-1 in_progress --from todo && eval 'sed -i s/a/b/ src/x.rs'",
            "moai mv t-1 in_progress --from todo && bash -c \"echo y > src/x.rs\"",
        ] {
            assert_eq!(guard_writes(&[], &cfg(), &here(), root, root, cmd), Decision::Pass, "집기 뒤의 글을 막았다 — {cmd}");
        }
        for cmd in [
            "bash -c 'sed -i s/a/b/ src/x.rs'; cd /tmp",
            "eval 'sed -i s/a/b/ src/x.rs'; cd /tmp",
            "bash -c 'sed -i s/a/b/ src/x.rs'; moai mv t-1 in_progress --from todo || exit 1",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "집기 앞의 쓰기가 뒤의 판을 물려받았다 — {cmd}\n{got:?}");
        }
        // **옵션이 아닌 낱말에서 멈춘다** — `bash 스크립트.sh -c '…'` 의 `-c` 는 그 스크립트의
        // 인자지 명령 글이 아니다. 끝까지 훑던 판은 안 도는 글을 규칙에 비췄다(잘못 막음).
        for cmd in ["bash script.sh -c \"moai add '딴 일'\"", "bash -- -c \"moai add '딴 일'\"", "env eval \"moai add '딴 일'\""] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "안 도는 글을 명령으로 읽었다 — {cmd}");
        }
        // `$( … )` 를 사이에 끼워 겹을 되돌리지 못한다 — 되돌리던 판은 아홉 겹 211바이트 한 줄에
        // 27초를 썼고, 훅이 제 시간에 못 끝나면 규칙이 통째로 열린다.
        let mut mixed = "moai add '딴 일'".to_string();
        for _ in 0..9 {
            mixed = format!("bash -c \"$({mixed})\"");
        }
        let clock = std::time::Instant::now();
        let _ = guard_create(&all, &cfg(), &here(), &mixed);
        assert!(clock.elapsed() < std::time::Duration::from_secs(2), "겹마다 같은 글을 다시 읽는다 — {:?}", clock.elapsed());
    }

    /// **셸을 여는 스위치가 넘긴 글도 명령이다**(moai-drli) — `env -S` 와 `sudo -s`.
    /// `command_of` 는 그 앞에서 멈추면서 "그 뒤는 글이고 그 글을 읽는 것은 렉서의 일" 이라 적어
    /// 뒀는데, 정작 렉서는 `bash -c` 와 `eval` 만 알아 **그 글을 읽는 것이 아무도 없었다** —
    /// `env -S "moai add x"` 와 `sudo -s moai add x` 가 규칙 1 을, `sudo -s sed -i …` 가 규칙 2 를
    /// 통째로 지나갔다. `bash -c` 와 같은 표의 줄로 둔다(2026-09-19 사용자 결정).
    ///
    /// **`sudo -i` 와 `doas -s` 는 그 줄이 아니다**(2026-09-20 사용자 결정, 리뷰 moai-jlon.yeg
    /// 5·6번) — 로그인 셸은 대상 사용자의 홈으로 옮겨 가 **딴 자리에서** 돌고, `doas -s` 는 argv 를
    /// `$SHELL` 로 갈아치워 뒤의 명령을 **아예 안 돌린다**. 둘 다 `stops` 가 가리려던 바로 그
    /// 자리라, 2026-09-19 결정의 목록이 아니라 그 결정이 스스로 세운 잣대를 따랐다.
    #[test]
    fn a_shell_opening_switch_hands_its_text_to_the_lexer() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        for cmd in [
            "env -S \"moai add '딴 일'\"",
            "env --split-string \"moai add '딴 일'\"",
            "env --split-string=\"moai add '딴 일'\"",
            "env -iS \"moai add '딴 일'\"",
            "env -u FOO -S \"moai add '딴 일'\"",
            "sudo -s moai add '딴 일'",
            "sudo --shell moai add '딴 일'",
            "sudo -u 남 -s moai add '딴 일'",
            // **깃발 뒤에도 옵션은 이어진다** — `sudo -s` 는 값을 안 받는 깃발이라 명령은 옵션이
            // **다 끝난** 자리다. 곧장 그 뒤부터를 글로 읽던 판은 글의 명령 자리가 `-n`·`남`·`--`
            // 가 되어, 옵션 하나만 더 적으면 규칙이 도로 통째로 샜다.
            "sudo -s -n moai add '딴 일'",
            "sudo -s -u 남 moai add '딴 일'",
            "sudo -su 남 moai add '딴 일'",
            "sudo -s --non-interactive moai add '딴 일'",
            "sudo -s -- moai add '딴 일'",
            // **따옴표가 되살아난다** — 바깥 껍데기가 벗긴 것을 도로 감싸서 잇는다. 맨 빈칸으로만
            // 잇던 판은 제목 안의 `-e <에픽>` 이 진짜 플래그로 읽혀 단위 안에 세운 것이 됐다.
            "sudo -s moai add '제목 -e t-e'",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "셸을 여는 스위치가 글을 가렸다 — {cmd}");
        }
        // 규칙 2 도 같은 길로 샜다 — `sudo -s sed -i …` 는 실제로 그 파일을 고친다.
        for cmd in [
            "sudo -s sed -i s/a/b/ src/x.rs",
            "env -S \"sed -i s/a/b/ src/x.rs\"",
            "sudo -s -n sed -i s/a/b/ src/x.rs",
            // **안쪽 `-c` 의 글은 한 낱말로 남아야 한다** — 맨 빈칸으로 잇던 판은 그 글이 낱말로
            // 흩어져 `bash -c sed` 가 되고, 안쪽 렉서가 `sed` 하나만 글로 받아 쓰기가 통째로 샜다.
            "sudo -s bash -c 'sed -i s/a/b/ src/x.rs'",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "셸을 여는 스위치가 쓰기를 가렸다 — {cmd}\n{got:?}");
        }
        // **없는 쓰기를 지어내지 않는다** — sudo 는 argv 를 escape 해 셸에 넘기므로 따옴표 안의
        // `>`·`;` 는 연산자가 아니다. 맨 빈칸으로 잇던 판은 그것을 리다이렉션으로 읽어, 아무것도
        // 안 쓰는 줄을 "집은 것 없이 src/x.rs 를 고친다" 며 **잘못 막았다** — 새는 것보다 나쁘다.
        for cmd in [
            "sudo -s git commit -m '고침: a > src/x.rs'",
            "sudo -s echo 'a; sed -i s/a/b/ src/x.rs'",
            "sudo -s moai note t-1 'a > src/x.rs 로 고쳤다'",
            // **`env -S` 는 셸이 아니다** — `>` 를 낱말로 넘길 뿐이라 아무것도 안 쓴다.
            "env -S \"echo done > src/x.rs\"",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert_eq!(got, Decision::Pass, "따옴표 안의 글자를 연산자로 읽어 없는 쓰기를 지어냈다 — {cmd}\n{got:?}");
        }
        // **지어낸 집기는 규칙을 덜 막게 한다** — 더 보는 쪽이 늘 안전하다는 셈이 집기 축에서는
        // 거꾸로 선다. `env -S` 는 `&&` 를 낱말로 넘겨 `/bin/true` 하나를 돌릴 뿐인데, 그 안의
        // `moai mv` 를 집기로 세던 판은 뒤의 **빈손 쓰기**를 그것으로 풀어 줬다.
        let faked = "env -S \"true && moai mv t-1 in_progress --from todo\" && sed -i s/a/b/ src/x.rs";
        let got = guard_writes(&[], &cfg(), &here(), root, root, faked);
        assert!(matches!(got, Decision::Deny(_)), "돌지도 않는 집기로 빈손의 쓰기가 샜다\n{got:?}");
        // 같은 자가 **진짜 셸**은 그대로 읽는다 — `env -S` 가 연 셸 안의 집기는 정말 돈다.
        // **두 쪽을 함께 잰다** — Pass 하나만 두면 글을 아예 안 읽어도 Pass 라, 아무것도 안 잰다.
        let real = |inner: &str| format!("env -S \"bash -c '{inner}'\"");
        let blind = real("sed -i s/a/b/ src/x.rs");
        let got = guard_writes(&[], &cfg(), &here(), root, root, &blind);
        assert!(matches!(got, Decision::Deny(_)), "진짜 셸 안의 쓰기를 안 읽었다 — {blind}\n{got:?}");
        let picked = real("moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/x.rs");
        assert_eq!(guard_writes(&[], &cfg(), &here(), root, root, &picked), Decision::Pass, "진짜 셸 안의 집기를 안 읽었다");
        // **`\\_` 는 낱말을 가른다** — 밑줄로 읽던 판은 이 줄을 낱말 하나로 만들어 규칙을 껐다.
        for cmd in [r"env -S'sed\_-i\_s/a/b/\_src/x.rs'", "env -S \"#주석\" sed -i s/a/b/ src/x.rs"] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, cmd);
            assert!(matches!(got, Decision::Deny(_)), "`env -S` 의 낱말 가르기가 쓰기를 가렸다 — {cmd}\n{got:?}");
        }
        // **피연산자는 env 가 안 가른다** — 이어 붙여 함께 가르던 판은 없는 집기를 지어냈다.
        let faked = "env -S \"moai mv\" \"t-1 in_progress --from todo\" && sed -i s/a/b/ src/x.rs";
        let got = guard_writes(&[], &cfg(), &here(), root, root, faked);
        assert!(matches!(got, Decision::Deny(_)), "피연산자를 갈라 없는 집기를 지어냈다\n{got:?}");
        // 그 글 안의 집기도 같은 자리에서 읽힌다 — 집고 쓰는 한 줄은 지나간다.
        let picked = "sudo -s moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/x.rs";
        assert_eq!(guard_writes(&[], &cfg(), &here(), root, root, picked), Decision::Pass, "글 안의 집기를 안 읽었다");
        // **글이 없으면 사람이 쓸 셸이다** — 아무 명령도 없으니 아무것도 비추지 않는다.
        for cmd in ["sudo -s", "env -S"] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "빈 셸을 명령으로 읽었다 — {cmd}");
        }
        // **뭉치의 남은 글자는 `env -S` 에서만 값이다** — `sudo -sn` 의 `n` 은 스위치지 글이 아니다.
        assert!(
            matches!(guard_create(&all, &cfg(), &here(), "sudo -sn moai add '딴 일'"), Decision::Deny(_)),
            "뭉친 스위치의 남은 글자를 글의 첫 낱말로 읽었다"
        );
        // **뭉치 안의 멈추는 글자는 뭉쳐도 멈춘다** — `sudo -si` 는 로그인 셸이라 딴 자리에서 돈다.
        assert_eq!(
            guard_create(&all, &cfg(), &here(), "sudo -si moai add '딴 일'"),
            Decision::Pass,
            "뭉친 `-i` 를 못 보고 딴 자리의 줄을 여기 것으로 읽었다"
        );
    }

    /// **따옴표 없는 heredoc 본문의 치환은 명령이다**(moai-t863) — 셸이 그것을 돌려 값을 본문에 끼운다.
    /// 종료어에 따옴표가 있으면 본문은 글 그대로라 아무 명령도 아니다(리뷰 원문을 그대로 붙이는 길이다).
    #[test]
    fn an_unquoted_heredoc_body_runs_its_substitutions() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        let open = "cat <<EOF\n$(moai add '딴 일')\nEOF";
        assert!(matches!(guard_create(&all, &cfg(), &here(), open), Decision::Deny(_)), "따옴표 없는 본문의 치환을 안 읽었다");
        let tick = "cat <<EOF\n`moai add '딴 일'`\nEOF";
        assert!(matches!(guard_create(&all, &cfg(), &here(), tick), Decision::Deny(_)), "백틱을 안 읽었다");
        let wrote = guard_writes(&[], &cfg(), &here(), root, root, "cat <<EOF\n$(echo x > src/store.rs)\nEOF");
        assert!(matches!(wrote, Decision::Deny(_)), "본문 안의 쓰기를 안 봤다 — {wrote:?}");

        for quoted in ["cat <<'EOF'\n$(moai add '딴 일')\nEOF", "cat <<\"EOF\"\n$(moai add '딴 일')\nEOF", "cat <<\\EOF\n$(moai add '딴 일')\nEOF"] {
            assert_eq!(guard_create(&all, &cfg(), &here(), quoted), Decision::Pass, "따옴표 친 본문을 명령으로 읽었다 — {quoted}");
        }
        // **줄을 넘는 치환도 한 치환이다**(리뷰 moai-p836.rv) — 줄마다 보던 판은 줄 끝에서 잘라,
        // 이어지는 줄의 명령을 아무 규칙에도 안 보였다.
        for cmd in ["cat <<EOF\n$(echo hi\nmoai add '딴 일')\nEOF", "cat <<EOF\n`\nmoai add '딴 일'\n`\nEOF"] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "줄을 넘는 치환을 잘랐다 — {cmd}");
        }
        // **따옴표 안의 괄호는 안 센다** — 셸이 그렇게 읽는다. 괄호만 세던 판은 거기서 끊어 뒤의
        // 명령을 통째로 잃었다.
        for cmd in ["cat <<EOF\n$(echo ')' ; moai add '딴 일')\nEOF", "cat <<EOF\n$(echo \"a)b\"; moai add '딴 일')\nEOF"] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "따옴표 안의 괄호에서 끊었다 — {cmd}");
        }
        // 셈(`$((…))`)은 명령이 아니다.
        assert_eq!(guard_create(&all, &cfg(), &here(), "cat <<EOF\n$((1 + 2))\nEOF"), Decision::Pass);
        // 본문은 여전히 그 토막에 흘러드는 글이다 — 리뷰 원문을 넣는 길이 안 막힌다.
        let note = "moai note t-1 -b - <<EOF\n무엇을 봤나\nEOF";
        assert_eq!(guard_create(&all, &cfg(), &here(), note), Decision::Pass);
        // **짝 없는 여는 글자는 명령이 아니다** — 셸도 그 줄을 못 읽어 아무것도 안 돈다. 글 끝까지를
        // 본문으로 치던 판은 markdown 산문의 백틱 하나로 그 뒤를 통째로 명령으로 읽어, 리뷰 글을
        // 붙이는 바로 그 길을 막았다(리뷰 moai-p836.rv).
        for prose in [
            "moai note t-1 -b - <<EOF\n- 남은 것: `sed -i s/a/b/ src/x.rs\nEOF",
            "moai note t-1 -b - <<EOF\n50% 쯤 됐다 $(sed -i s/a/b/ src/x.rs\nEOF",
        ] {
            let got = guard_writes(&[], &cfg(), &here(), root, root, prose);
            assert_eq!(got, Decision::Pass, "짝 없는 글자 뒤를 명령으로 읽었다 — {prose}\n{got:?}");
        }
    }

    // ── 명령이 가리키는 트래커 ────────────────────────────────────────

    /// 토막마다 그 `moai` 가 도는 자리를 읽는다 — `-C`·`--dir`·앞의 `cd`. 모르는 자리는
    /// 세션의 자리(`None`)다.
    #[test]
    fn each_moai_segment_knows_where_it_runs() {
        let at = |cmd: &str| -> Vec<Option<String>> {
            aimed(cmd, Path::new("/a/b")).into_iter().map(|d| d.map(|p| p.display().to_string())).collect()
        };
        let there = |p: &str| Some(p.to_string());
        assert_eq!(at("moai add x"), [None]);
        assert_eq!(at("moai -C /c add x"), [there("/c")]);
        assert_eq!(at("moai --dir=../c add x"), [there("/a/c")]);
        assert_eq!(at("moai add x -C/c"), [there("/c")]);
        assert_eq!(at("moai -C . add x"), [None], "제자리를 남의 자리로 읽었다");
        assert_eq!(at("cd /c && moai add x; moai -C d mv t-1 done"), [None, there("/c"), there("/c/d")]);
        assert_eq!(at("cd .. && moai add x"), [None, there("/a")]);
        // 모르는 자리는 지어내지 않는다.
        assert_eq!(at("cd $HOME && moai add x"), [None, None]);
        assert_eq!(at("cd /c && cd - && moai add x"), [None, None, None]);
        assert_eq!(at("cd && moai add x"), [None, None]);
        assert_eq!(at("pushd /c && pushd +1 && moai add x"), [None, None, None], "스택 돌리기를 경로로 읽었다");
        assert_eq!(at("moai -C $X add x"), [None]);
        // 하위 셸의 `cd` 는 뒤로 안 이어진다 — 묶음을 나오면 제자리, 파이프·`&` 는 안 옮긴다.
        assert_eq!(at("(cd /c && moai add x); moai add y"), [None, there("/c"), None], "묶음의 cd 가 샜다");
        assert_eq!(at("cd /c && ( cd /d; moai add x ) && moai add y"), [None, None, there("/d"), there("/c")]);
        assert_eq!(at("cd /c | moai add x; moai add y"), [None, None, None], "파이프의 cd 를 이어 읽었다");
        assert_eq!(at("cd /c & moai add x"), [None, None], "& 로 띄운 cd 를 이어 읽었다");
        assert_eq!(at("cd /c || moai add x"), [None, there("/c")]);
        assert_eq!(at("cd /c &&\nmoai add x"), [None, there("/c")]);
        // `moai` 가 아닌 토막은 어디도 안 가리킨다.
        assert_eq!(at("echo -C /c"), [None]);
    }

    /// 다른 트래커를 가리키는 토막은 제 초점으로 안 본다 — 그 토막만 뺀다(moai-23ky).
    #[test]
    fn a_segment_aimed_elsewhere_is_left_to_that_tracker() {
        let mine = vec![issue("t-1", "in_progress")];
        let root = Path::new("/a");
        let judge = |cmd: &str| {
            let dirs = aimed(cmd, root);
            let own = |k: usize| dirs[k].is_none();
            guard_shell_in(&mine, &cfg(), &here(), root, root, cmd, &Segs { judges: &own, picks: &own }, &|k| dirs[k].as_deref())
        };
        assert_eq!(judge("moai -C /b add \"딴 일\""), Decision::Pass);
        assert_eq!(judge("cd /b && moai add '딴 일'"), Decision::Pass);
        assert!(denied(&judge("moai -C /b add \"딴 일\" && moai add '또'")).contains("t-1"));

        // 가리킨 트래커가 쥔 것이 있으면 그 줄로 막는다.
        let theirs = vec![issue("t-9", "in_progress")];
        let cmd = "moai -C /b add \"딴 일\"";
        let dirs = aimed(cmd, root);
        let why =
            denied(&guard_moai(&theirs, &cfg(), &here(), cmd, &|k| dirs[k].is_some(), &|k| dirs[k].as_deref())).to_string();
        assert!(why.contains("t-9"), "{why}");
    }

    /// **에픽 일을 집은 채 생각을 담으면 둘째 물음을 비춘다 — 막지 않는다**(moai-d4e0). 거절문에만
    /// 실으면 실제로 틀리는 자리인 `idea add` 는 말없이 지나간다.
    #[test]
    fn setting_an_idea_aside_mid_epic_hears_the_second_question() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        for cmd in ["moai idea add '떠오른 것'", "cd /repo && moai add --type idea \"떠오른 것\""] {
            let Decision::Context(said) = guard_shell(&all, &cfg(), &here(), root, root, cmd) else {
                panic!("안 비춘다 — {cmd}");
            };
            assert!(said.contains("t-e 가 내건 것"), "{said}");
            // **이 줄은 생각이 담긴 뒤에 읽힌다** — 새로 세우라고 하면 같은 것이 둘 선다(moai-dw63.e31).
            assert!(said.contains("moai idea promote <그 id> -e t-e --from -"), "담은 것을 되찾는 줄을 안 댄다\n{said}");
            assert!(!said.contains("moai add '제목'"), "담긴 생각 곁에 같은 것을 또 세우라고 한다\n{said}");
        }
        // 담은 토막이 **겨눈 자리**도 댄다 — 빼고 치면 되찾는 줄이 세션 자리의 트래커에서 헛돈다.
        // 친 글자가 아니라 푼 자리다(moai-v9sa).
        let at = Path::new("/repo/sub");
        let all_of = Segs { judges: &|_| true, picks: &|_| true };
        let aside = guard_shell_in(&all, &cfg(), &here(), root, root, "moai -C .. idea add \"x\"", &all_of, &|_| Some(at));
        let Decision::Context(said) = aside else {
            panic!("안 비춘다 — {aside:?}");
        };
        assert!(said.contains("moai -C /repo/sub idea promote <그 id> -e t-e"), "{said}");

        // 집은 것이 없거나, 에픽 없는 일이거나, 도움말이면 조용하다. **에픽 줄이 실제로 안 선
        // 참조**도 조용하다 — 닫힐 에픽이 없고, 그 id 로 되찾게 하면 경고가 하나 는다.
        let loose = vec![issue("t-1", "in_progress")];
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let not_an_epic = vec![issue("t-x", "todo"), under("t-1", "in_progress", "t-x")];
        let dangling = vec![under("t-1", "in_progress", "t-gone")];
        for (all, cmd) in [
            (&loose, "moai idea add '떠오른 것'"),
            (&idle, "moai idea add '떠오른 것'"),
            (&not_an_epic, "moai idea add '떠오른 것'"),
            (&dangling, "moai idea add '떠오른 것'"),
            (&all, "moai idea add --help"),
            (&all, "moai idea ls"),
            (&all, "moai note t-1 \"idea add 를 적는다\""),
        ] {
            assert_eq!(guard_shell(all, &cfg(), &here(), root, root, cmd), Decision::Pass, "{cmd}");
        }

        // 에픽 둘을 쥐었으면 **집은 차례로** 댄다 — 규칙 1 의 거절문과 같은 에픽을 앞에 둔다.
        let two = vec![epic("t-z"), epic("t-a"), under("t-1", "in_progress", "t-z"), under("t-2", "in_progress", "t-a")];
        let Decision::Context(said) = guard_shell(&two, &cfg(), &here(), root, root, "moai idea add 'x'") else {
            panic!("안 비춘다");
        };
        assert!(said.contains("t-z·t-a 가 내건 것") && said.contains("-e t-z --from -"), "{said}");
        let refused = guard_shell(&two, &cfg(), &here(), root, root, "moai add '딴 일'");
        assert!(denied(&refused).contains("-e t-z"), "두 글이 다른 에픽을 댄다\n{refused:?}");

        // **비추는 줄이 막는 것을 가리지 않는다** — 같은 명령줄의 규칙 1·3 이 먼저다.
        for cmd in ["moai idea add 'a'; moai add '딴 일'", "moai idea add 'a' && /code-review high"] {
            assert!(matches!(guard_shell(&all, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "{cmd}");
        }
    }

    /// **명령 치환 안의 `moai` 도 규칙을 지난다**(moai-xe6e). 셸은 치환을 먼저 돌린다 — 그 글을
    /// 바깥 낱말 하나로만 두던 판은 `$(…)`·`` `…` `` 하나로 규칙 1·2·3 을 통째로 넘겼다.
    #[test]
    fn a_command_substitution_is_read_as_a_command() {
        let root = Path::new("/repo");
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r", "in_progress", Some("t-e"))];
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for (all, cmd) in [
            // 규칙 1 — 집은 것 밖의 생성.
            (&held, "echo \"$(moai add '딴 일')\""),
            (&held, "echo `moai add 딴일`"),
            (&held, "x=$(moai add '딴 일' --json)"),
            (&held, "echo \"$(echo \"$(moai add 딴일)\")\""),
            // 규칙 3 — 낸 글 없이 리뷰를 닫는다.
            (&held, "echo $(moai mv t-r done)"),
            // 규칙 2 — 집지 않고 쓴다.
            (&idle, "echo \"$(sed -i s/a/b/ src/store.rs)\""),
            (&idle, "echo `echo x > src/store.rs`"),
            (&idle, "cat <(sed -i s/a/b/ src/store.rs)"),
            // 치환은 제 토막의 이음사를 받는다 — `;` 뒤의 치환은 집기가 져도 돈다.
            (&idle, "moai mv t-1 in_progress; echo \"$(sed -i s/a/b/ src/store.rs)\""),
        ] {
            assert!(matches!(guard_shell(all, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        for (all, cmd) in [
            // 치환 안이 아닌 글자 — 홑따옴표, 감싼 백틱, heredoc 본문.
            (&held, "moai note t-1 '$(moai add x) 와 `moai add y`'"),
            (&held, "moai note t-1 \"\\`moai add x\\` 는 막힌다\""),
            (&held, "moai note t-1 -b \"$(cat <<'EOF'\nmoai add '딴 일'\nEOF\n)\""),
            (&held, "git commit -m \"$(cat <<'EOF'\nfix: `moai add` 를 막는다\nEOF\n)\""),
            // 치환도 집기 뒤에 서면 집기 뒤다.
            (&idle, "moai mv t-1 in_progress && echo \"$(sed -i s/a/b/ src/store.rs)\""),
            // 치환 안의 `cd` 는 바깥으로 안 샌다 — 뒤의 상대 경로는 저장소 밖이 아니다.
            (&held, "echo $(cd /tmp) && echo x > /tmp/y"),
            // 치환 안의 집기 뒤는 그 치환 안에서는 집기 뒤다.
            (&idle, "echo \"$(moai mv t-1 in_progress && sed -i s/a/b/ src/store.rs)\""),
            // 세우는 토막이 모두 단위 안이면 지나간다.
            (&held, "moai add 'a' -e t-e && moai add 'b' --parent t-1"),
        ] {
            assert_eq!(guard_shell(all, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        // 치환 안의 `cd` 가 바깥 토막의 자리를 옮기지 않는다 — 옮기면 상대 경로를 버려 쓰기가 샌다.
        // **뒤로 띄운 것과 파이프의 칸도 같다**(리뷰 moai-k8j1.udq 의 6번) — 괄호는 `seg.low` 가
        // 걷어 주지만 `{ … } &` 에는 괄호가 없어, 그 `cd` 가 바깥 자리를 옮긴 것으로 서 있었다.
        for cmd in [
            "echo $(cd /tmp); echo x > src/store.rs",
            "(cd /tmp); echo x > src/store.rs",
            "{ cd /tmp; } & sed -i s/a/b/ src/store.rs",
            "cd /tmp & sed -i s/a/b/ src/store.rs",
            "{ cd /tmp; } | cat; sed -i s/a/b/ src/store.rs",
            "if true; then cd /tmp; fi & sed -i s/a/b/ src/store.rs",
        ] {
            assert!(matches!(guard_shell(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "하위 셸의 cd 가 바깥으로 샜다 — {cmd}");
        }
        // **치환 밖의 명령은 치환의 값을 모른다**(리뷰 moai-ju21.70g) — 치환 안의 집기는 바깥의 `&&` 로
        // 안 이어진다. `echo` 는 집기가 져도 이긴다.
        for cmd in [
            "true && echo \"$(moai mv t-1 in_progress --from todo)\" && sed -i s/a/b/ src/store.rs",
            "true && echo `moai mv t-1 in_progress --from todo` && sed -i s/a/b/ src/store.rs",
            "true && cat <(moai mv t-1 in_progress --from todo) && sed -i s/a/b/ src/store.rs",
            "set -e; echo \"$(moai mv t-1 in_progress --from todo)\"; sed -i s/a/b/ src/store.rs",
            // 첫 치환이 아무것도 안 내도 바깥의 `;` 는 그대로다.
            "moai mv t-1 in_progress --from todo; echo \"$(<x)\" \"$(sed -i s/a/b/ src/store.rs)\"",
        ] {
            assert!(matches!(guard_shell(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        // **세우는 토막은 모두 본다**(리뷰 moai-ju21.70g) — 단위 안의 앞 토막 하나가 뒤를 가리지 않는다.
        // 치환은 바깥 토막보다 먼저 쌓여 첫 것이 되기 쉽다.
        for cmd in [
            "x=$(moai add 'a' -e t-e --json); moai add 'b'",
            "moai add 'a' -e t-e && moai add 'b'",
            "id=`moai add 'a' --parent t-1 --json`; moai add 'b'",
            // 낱말 없는 토막과 산술 안의 치환도 명령이다.
            "while read l; do :; done < <(moai add x)",
            "echo $(( $(moai add x) ))",
        ] {
            assert!(matches!(guard_shell(&held, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **형제 괄호는 딴 셸이다**(리뷰 moai-ju21.70g) — 깊이가 같아도 앞 괄호의 `cd` 는 뒤 괄호의 `moai`
    /// 를 옮기지 않는다. 옮겨 읽던 판은 그 `moai` 를 트래커 없는 자리로 보내 아무도 판정하지 않았다.
    #[test]
    fn a_sibling_subshell_does_not_inherit_a_cd() {
        let at = |cmd: &str| aimed(cmd, Path::new("/repo"));
        for cmd in ["(cd /b); (moai add x)", "echo $(cd /b) $(moai add x)", "diff <(cd /b) <(moai add x)"] {
            assert!(at(cmd).iter().all(Option::is_none), "{cmd} → {:?}", at(cmd));
        }
        // 같은 괄호 안의 `cd` 는 이어진다.
        assert_eq!(at("(cd /b; moai add x)").last().cloned().flatten(), Some(PathBuf::from("/b")));
        assert_eq!(at("echo $(cd /b && moai add x)").iter().flatten().next().cloned(), Some(PathBuf::from("/b")));
    }

    /// **`cd` 가 옮긴 자리와 제 낱말로 적은 `-C` 는 다르다**([`spells_dir`], 리뷰 moai-51h9.k8j1) —
    /// [`aimed`] 는 둘을 한 값으로 내는데, 아직 없는 자리에서는 적은 `-C` 만 곧 만들어질 자리다.
    /// 차례는 [`aimed`] 와 같아야 한다 — 어긋나면 `cmd/hook.rs` 의 `route` 가 옆 토막의 답을 읽는다.
    #[test]
    fn only_a_spelled_dir_flag_counts_as_a_place_the_command_names() {
        let at = Path::new("/repo");
        for cmd in ["cd /b && moai add x", "moai -C /b add x", "moai add x", "cd /b; ls; moai --dir /c add x"] {
            assert_eq!(spells_dir(cmd).len(), aimed(cmd, at).len(), "차례가 갈라졌다 — {cmd}");
        }
        assert_eq!(spells_dir("cd /b && moai add x"), [false, false]);
        assert_eq!(spells_dir("moai -C /b add x"), [true]);
        assert_eq!(spells_dir("cd /b; ls; moai --dir /c add x"), [false, false, true]);
        // `moai` 가 아닌 토막은 제 `-C` 가 있어도 아니다 — [`aimed`] 도 그 토막은 안 본다.
        assert_eq!(spells_dir("git -C /b status && moai add x"), [false, false]);
    }

    /// **판정을 잇는 차례는 하나다**(`Decision::then`) — 막는 답이 이기고, 앞이 막으면 뒤는 묻지도
    /// 않고, 둘 다 안 막으면 비추는 줄을 모은다. 규칙마다·트래커마다 손으로 적던 차례는 이미
    /// 서로 다른 답을 냈고, 옆과 겹쳐 다시 보는 자리(`settle`)는 비추는 줄을 거절로 읽었다.
    #[test]
    fn a_blocking_verdict_wins_and_notes_are_kept_together() {
        let note = |s: &str| Decision::Context(s.into());
        let deny = || Decision::Deny("막는다".into());
        assert!(deny().blocks() && Decision::Block("붙든다".into()).blocks());
        assert!(!note("비춘다").blocks() && !Decision::Pass.blocks());

        assert_eq!(Decision::Pass.then(|| note("a")), note("a"));
        assert_eq!(note("a").then(|| Decision::Pass), note("a"));
        assert_eq!(note("a").then(deny), deny(), "비추는 줄이 뒤의 거절을 가린다");
        assert_eq!(deny().then(|| panic!("앞이 막았는데 뒤를 물었다")), deny());
        assert_eq!(note("a").then(|| note("b")), note("a\n\nb"), "둘째 트래커의 물음을 버린다");
    }

    // ── 무엇을 부르려는가 ────────────────────────────────────────────

    /// 도구 이름으로 가른다. **입력에 든 글자로 가르지 않는다** — 리뷰를
    /// 설명하는 글을 쓰는 것만으로 리뷰 규칙에 걸리면 그 규칙은 못 쓴다.
    #[test]
    fn what_a_tool_calls_is_read_from_its_name() {
        let write = serde_json::json!({"file_path": "/x/y.rs", "content": "code-review 를 부른다"});
        assert_eq!(Call::read(Some("Write"), &write), Call::Edits("/x/y.rs"));

        let note = shell("moai note t-1 \"code-review 가 낸 것\"");
        assert!(matches!(Call::read(Some("Bash"), &note), Call::Shell(_)));
        assert!(!calls_review("moai note t-1 \"code-review 가 낸 것\""));

        let grep = shell("grep -rn code-review .");
        assert!(matches!(Call::read(Some("Bash"), &grep), Call::Shell(_)));
        assert!(!calls_review("grep -rn code-review ."));

        // 껍데기는 언제나 `Shell` 이고, 리뷰를 부르는지는 토막마다 본다.
        let real = shell("/code-review high");
        assert_eq!(Call::read(Some("Bash"), &real), Call::Shell("/code-review high"));
        assert!(calls_review("/code-review high"));

        let skill = serde_json::json!({"skill": "code-review", "args": "low"});
        assert_eq!(Call::read(Some("Skill"), &skill), Call::Review);
    }

    // ── 규칙 1 — 초점 밖에 세우지 않는다 ────────────────────────────

    /// 집은 것이 없으면 아무것도 막지 않는다. 초점 없는 규칙은 규칙이 아니다.
    #[test]
    fn with_nothing_held_creation_is_free() {
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add '딴 일'"), Decision::Pass);
    }

    /// **규칙 4 — 사람의 tmux 서버를 죽이지 않는다**(moai-zis7). 집은 것과 무관하게 막고, 제
    /// 서버를 가리킨 것은 지난다. 거절문이 댄 줄은 그대로 치면 지나간다.
    #[test]
    fn a_bare_tmux_kill_is_refused() {
        let root = Path::new("/repo");
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "tmux kill-server",
            "TMUX_TMPDIR=/tmp/x tmux kill-server",
            "tmux kill-session -t moai",
            "env -u FOO tmux kill-server",
            "sudo tmux -f /dev/null kill-server",
            "/usr/bin/tmux kill-server",
            "cargo test; tmux kill-server",
            "echo \"$(tmux kill-server)\"",
            "pkill tmux",
            "pkill -f 'tmux: server'",
            "killall tmux",
            // tmux 는 모호하지 않은 앞머리도 받는다(리뷰 moai-ju21.70g).
            "tmux kill-ser",
            "tmux kill-sess -t x",
            // `-L default` 는 맨 tmux 가 붙는 바로 그 서버다.
            "tmux -L default kill-server",
            "tmux -Ldefault kill-session",
        ] {
            for all in [&held, &idle] {
                let why = denied(&guard_shell(all, &cfg(), &here(), root, root, cmd)).to_string();
                assert!(why.starts_with(&crate::guide::rule_head(4)), "규칙 4 가 안 섰다 — {cmd}\n{why}");
            }
        }
        for cmd in [
            "tmux -L moai-ju21 kill-server",
            "env -u TMUX tmux -L t kill-session -t x",
            "tmux -S /tmp/scratch/sock kill-server",
            "tmux -Lfoo kill-server",
            "tmux kill-pane -t 1",
            "tmux ls",
            "moai note t-1 \"tmux kill-server 를 막는다\"",
            "echo 'tmux kill-server'",
            "pkill cargo",
        ] {
            assert_eq!(guard_shell(&held, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        // 거절문이 댄 줄은 자리표시자만 채우면 지나간다.
        let why = denied(&guard_shell(&idle, &cfg(), &here(), root, root, "tmux kill-server")).to_string();
        let line = why.lines().last().unwrap().trim().replace("<고유 이름>", "moai-t");
        assert!(line.ends_with("kill-server"), "{why}");
        assert_eq!(guard_shell(&idle, &cfg(), &here(), root, root, &line), Decision::Pass, "{line}");
    }

    /// **`hook --help` 의 이벤트 목록은 `Event` 의 글과 같다**(moai-h0r2) — clap 의 값 목록을 숨기고
    /// 손으로 옮겨 적었으니, 이벤트를 더하거나 글을 고치면 여기서 붉어진다.
    #[test]
    fn the_hook_help_lists_every_event() {
        use clap::{CommandFactory, ValueEnum};
        let cmd = crate::cli::Cli::command();
        let hook = cmd.find_subcommand("hook").expect("hook 명령이 없다");
        let after = hook.get_after_help().expect("hook 의 after_help 가 없다").to_string();
        for event in Event::value_variants() {
            let value = event.to_possible_value().unwrap();
            let line = format!("{:<20}{}", value.get_name(), value.get_help().unwrap());
            assert!(after.contains(&line), "목록이 `Event` 와 갈라졌다 — {line}\n{after}");
        }
    }

    /// **거절문의 모서리 셋**(moai-nxw8) — 막힌 토막이 **겨눈 트래커**를 대고, 여럿 집었으면 집은
    /// 것마다 대고, 둘째 물음의 글은 규칙 글과 한 출처다.
    ///
    /// **댈 자리는 푼 자리지 사람이 친 글자가 아니다**(moai-v9sa, 사용자 결정). `-C` 를 옮겨 적던 판은
    /// `moai -C .`·`cd src && moai -C ..` 를 그대로 내밀어, 옮겨 친 사람이 어디에 섰느냐에 따라 엉뚱한
    /// 트래커를 겨눴고 그 자리가 딸린 워크트리면 워크트리의 스냅샷에 줄을 세웠다.
    #[test]
    fn the_rule_one_refusal_keeps_the_tracker_and_every_held_unit() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        // 워크트리에서 루트를 가리켜 막힌 줄 — 옮겨 치면 루트에 서야 한다. 친 글자가 `.`·상대 경로여도
        // 내미는 줄은 푼 자리다.
        let root = Path::new("/repo");
        for cmd in ["moai -C /repo add \"딴 일\"", "moai add '딴 일' -C .", "moai --dir=../.. add \"딴 일\""] {
            let why = denied(&guard_create_toward(&all, &cfg(), &here(), cmd, root)).to_string();
            for line in ["moai -C /repo add '제목' -e t-e", "moai -C /repo add '제목' --parent t-1", "moai -C /repo idea add"] {
                assert!(why.contains(line), "{cmd} 가 겨눈 트래커를 안 댔다 — {line}\n{why}");
            }
            assert!(!why.contains("-C ."), "친 글자를 그대로 옮겨 적었다\n{why}");
        }
        // 이 자리의 트래커면 맨 `moai` 다 — 딸린 워크트리에서 루트로 옮겨 간 줄도 여기 선다(moai-y7go).
        for cmd in ["moai add '딴 일'", "moai -C . add '딴 일'"] {
            let why = denied(&guard_create(&all, &cfg(), &here(), cmd)).to_string();
            assert!(!why.contains("-C"), "{cmd}\n{why}");
        }

        // 둘을 집었으면 둘 다 댄다 — 에픽이 같으면 에픽 줄은 한 번이다.
        let two = vec![epic("t-z"), epic("t-a"), under("t-1", "in_progress", "t-z"), under("t-2", "in_progress", "t-a")];
        let why = denied(&guard_create(&two, &cfg(), &here(), "moai add '딴 일'")).to_string();
        for line in ["-e t-z ", "-e t-a ", "--parent t-1 ", "--parent t-2 "] {
            assert!(why.contains(line), "집은 것 하나를 빠뜨렸다 — {line}\n{why}");
        }
        assert!(why.contains("t-z·t-a 가 내건 것"), "{why}");
        let same = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "in_progress", "t-e")];
        let why = denied(&guard_create(&same, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert_eq!(why.matches("-e t-e ").count(), 1, "같은 에픽을 두 번 댄다\n{why}");

        // 제목 자리는 작은따옴표다(moai-1yya) — 거절문은 에이전트가 그대로 옮겨 치는 글이고, 큰따옴표
        // 안의 백틱은 bash 가 명령으로 풀어 제목이 잘린다. 규칙 2 의 거절문도 같다.
        assert!(!why.contains("add \""), "거절문이 제목을 큰따옴표로 가르친다\n{why}");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let wrote = denied(&guard_edit(&idle, &cfg(), &here(), Path::new("/repo"), "/repo/src/x.rs")).to_string();
        assert!(wrote.contains("add '제목'") && !wrote.contains("add \""), "{wrote}");

        // 둘째 물음은 규칙 글과 같은 글이다.
        let pledge = crate::guide::PLEDGE;
        assert!(why.contains(&format!("t-e 가 {pledge}")), "{why}");
        assert!(crate::guide::agents().contains(&format!("에픽이 {pledge}")), "규칙 글이 갈라졌다");
    }

    /// **거절문의 모서리 넷**(리뷰 moai-ju21.70g) — 옮겨 친 `-C` 는 한 낱말로 감싸고, 에픽 아닌 줄은
    /// `-e` 로 안 대고, 에픽 있는 일과 없는 일을 함께 쥐었으면 둘 다 대고, 규칙 2 의 줄은 한 줄에 명령
    /// 하나다(붙여 넣는 쪽이 줄째 옮겨 친다).
    #[test]
    fn the_refusals_stay_runnable_as_written() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        // 옮겨 친 셸이 처음처럼 읽게 — 가르는 글자가 든 자리는 작은따옴표로, 그 밖은 그대로.
        // 자리는 푼 것이라(moai-v9sa) 변수도 `~` 도 아닌 참 경로다. **푼 경로의 `$`·백틱은 글자다** —
        // 큰따옴표로 감싸던 판은 옮겨 친 셸이 그것을 다시 풀어 딴 트래커를 겨눴고, 백틱이면 내민 줄이
        // 명령을 돌렸다.
        for (at, echoed) in [
            ("/a b", "'/a b'"),
            ("/tmp/R&D", "'/tmp/R&D'"),
            ("/repo/sub", "/repo/sub"),
            ("/srv/a$b/repo", "'/srv/a$b/repo'"),
            ("/srv/a`id`b", "'/srv/a`id`b'"),
        ] {
            let why =
                denied(&guard_create_toward(&all, &cfg(), &here(), "moai -C /x add '딴 일'", Path::new(at))).to_string();
            assert!(why.contains(&format!("moai -C {echoed} add '제목' -e t-e")), "{at} 를 {echoed} 로 안 옮겼다\n{why}");
        }

        // **자리를 못 푼 `-C` 는 친 글자를 그대로 옮긴다** — `aimed` 가 `$VAR`·`~`·`$( … )` 를 `None` 으로
        // 내는데, 그것을 "이 자리" 로 읽어 버리던 판은 `moai -C "$OTHER" add …` 로 막힌 사람에게 **이**
        // 트래커에 세우라고 시켰다. 그 글자는 옮겨 친 셸이 처음처럼 푼다.
        for (typed, echoed) in [
            ("\"$HOME/My Projects/x\"", "\"$HOME/My Projects/x\""),
            ("\"$(pwd)\"", "\"$(pwd)\""),
            ("~/x", "~/x"),
        ] {
            let why = denied(&guard_create(&all, &cfg(), &here(), &format!("moai -C {typed} add '딴 일'"))).to_string();
            assert!(why.contains(&format!("moai -C {echoed} add '제목' -e t-e")), "-C {typed} 를 버렸다\n{why}");
        }
        // 푼 자리가 있으면 그것이 이긴다 — 친 글자가 상대 경로여도 내미는 줄은 푼 경로다.
        let why =
            denied(&guard_create_toward(&all, &cfg(), &here(), "moai -C .. add '딴 일'", Path::new("/repo"))).to_string();
        assert!(why.contains("moai -C /repo add '제목' -e t-e") && !why.contains("-C .."), "{why}");

        let not_an_epic = vec![issue("t-x", "todo"), under("t-1", "in_progress", "t-x")];
        let why = denied(&guard_create(&not_an_epic, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(!why.contains("-e t-x"), "에픽 아닌 줄을 에픽으로 댄다\n{why}");
        assert!(why.contains("--parent t-1") && why.contains("t-1 를 닫기 전에 끝낸다"), "{why}");

        let mixed = vec![epic("t-e"), issue("t-a", "in_progress"), under("t-1", "in_progress", "t-e")];
        let why = denied(&guard_create(&mixed, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(why.contains("t-e 가 목적을 못 이룬 채") && why.contains("t-a 를 닫기 전에 끝낸다"), "한쪽 경고를 뺐다\n{why}");

        let wrote = denied(&guard_edit(&[], &cfg(), &here(), Path::new("/repo"), "/repo/src/x.rs")).to_string();
        for line in wrote.lines().filter(|l| l.contains("moai ")) {
            assert!(line.trim_start().starts_with("moai ") && line.matches("moai ").count() == 1, "한 줄에 명령이 둘이다 — {line}\n{wrote}");
        }
        assert!(wrote.contains("\n  moai add '제목'\n  moai mv <id> in_progress"), "{wrote}");
        // 집으라는 줄은 본 칸을 함께 준다 — 짓고 치는 사이에 옆이 그 일을 집거나 닫았으면 멈춘다.
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let wrote = denied(&guard_edit(&idle, &cfg(), &here(), Path::new("/repo"), "/repo/src/x.rs")).to_string();
        assert!(wrote.contains("moai mv t-1 in_progress --from todo"), "{wrote}");
    }

    /// 집은 것이 있으면 그 단위 안이어야 한다. 밖이면 고칠 명령이 함께 온다.
    #[test]
    fn outside_the_held_unit_is_refused_with_the_way_out() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let why = denied(&guard_create(&all, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(why.contains("-e t-e"), "에픽을 안 가리킨다\n{why}");
        assert!(why.contains("--parent t-1"), "자식으로 다는 길이 없다\n{why}");
        assert!(why.contains("idea add"), "담아 두는 길이 없다\n{why}");
        // idea 로 가는 문만 열어 두면 에픽이 내건 것 자체도 그리로 나가 에픽이
        // 목적을 못 이룬 채 닫힌다 (moai-l288).
        // 무엇이 내건 것인지는 갈림길 1 처럼 에픽으로 댄다 — 집은 이슈로 대면 에픽만 필요로
        // 하는 것에서 두 글이 다른 답을 낸다 (moai-dw63.gwf 4번).
        assert!(why.contains("t-e 가 내건 것"), "idea 가 아닌 경우를 에픽으로 안 가른다\n{why}");
        // 세울 줄은 이름으로 가리킨다 — "위의 줄" 바로 위가 `idea add` 줄이고 idea 도 첫 칸에 선다.
        assert!(why.contains("위의 `moai add` 줄로 세워 첫 칸에 둔다"), "에픽이 내건 것을 세울 줄을 안 가리킨다\n{why}");

        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add '안의 일' -e t-e"), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add '자식' --parent t-1"), Decision::Pass);

        // 선 에픽에 펼치는 promote 도 같은 자로 본다 — 안 보면 `idea add` 뒤 `promote -e` 가
        // `add -e` 가 막히는 자리를 지나간다 (moai-f3ml.lm7). 새 에픽을 세우는 promote 는 그대로다.
        let into = |e: &str| format!("moai idea promote t-i -e {e} --from -");
        assert!(matches!(guard_create(&all, &cfg(), &here(), &into("t-x")), Decision::Deny(_)), "남의 에픽에 promote 로 멤버를 세웠다");
        assert_eq!(guard_create(&all, &cfg(), &here(), &into("t-e")), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai idea promote t-i --epic=t-e --from -"), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai idea promote t-i --from -"), Decision::Pass);
    }

    /// **만드는 토막은 저마다 소속을 댄다**(moai-ean3, 2026-09-19 사용자 결정) — 한 줄에 `moai add`
    /// 가 여럿이면 앞 토막이 단위 안에 세웠다고 뒤 토막이 풀리지 않는다. 첫 토막의 에픽을 뒤로
    /// 물려주는 길은 고르지 않았다: 물려주면 `moai add 'a' -e <에픽> && moai add 'b'` 의 `b` 가
    /// 어느 일에서 나왔는지를 잃는데, 그것이 규칙 1 이 지키려던 단 하나다.
    ///
    /// `create_in` 이 **첫 만드는 토막만** 재던 자리다 — 리뷰 moai-ju21.70g 의 고침(0de7d49)이
    /// `find` 의 거르개를 토막마다 돌리면서 함께 닫혔고, 이 시험이 그 뜻을 못박는다. 자유로운 둘
    /// (`idea add`·`add --from`)은 그대로 둔다.
    #[test]
    fn every_creating_segment_names_its_own_unit() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in [
            "moai add '안' -e t-e && moai add '딴 일'",
            "moai add '안' -e t-e; moai add '딴 일'",
            "moai add '딴 일' && moai add '안' -e t-e",
            "moai add '안' --parent t-1 && moai add '딴 일'",
            // 치환은 바깥 토막보다 먼저 쌓인다 — 차례로 첫 토막을 고르면 단위 안의 것이 앞선다.
            "x=$(moai add '안' -e t-e); moai add '딴 일'",
            "( moai add '안' -e t-e ); moai add '딴 일'",
            "moai add '안' -e t-e | tee log && moai add '딴 일'",
            // 자유로운 앞 토막도 뒤를 안 풀어 준다.
            "moai idea add '떠오른 것' && moai add '딴 일'",
            "moai add --from - && moai add '딴 일'",
            "moai idea promote t-i --from - && moai add '딴 일'",
            // 세우는 철자가 달라도 같다.
            "moai add '안' -e t-e && moai issue add '딴 일'",
            "moai add '안' -e t-e && moai epic add '딴 에픽'",
            "moai add '안' -e t-e && moai milestone add '딴 마일스톤'",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "앞 토막의 소속이 뒤 토막을 풀어 줬다 — {cmd}");
        }
        // 토막마다 소속을 댔으면 지나간다 — 규칙이 요구하는 것은 그것뿐이다.
        for cmd in [
            "moai add '안' -e t-e && moai add '또 안' --parent t-1",
            "moai add '안' -e t-e && moai idea add '떠오른 것'",
            "moai add '안' -e t-e && moai add '딴 일' --from -",
            "moai add '안' -e t-e && moai add '떠오른 것' --type idea",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "저마다 소속을 댄 줄을 막았다 — {cmd}");
        }
    }

    /// **소속은 물려받는다.** 자식 이슈를 집었을 때 그 줄의 `epic` 은 비어
    /// 있지만, 에픽은 부모에게서 온다. 필드만 읽던 판은 옳은 에픽을 댄
    /// 생성까지 거절했다 — 이 규칙을 만든 세션이 제 리뷰 결과를 못 적었다.
    #[test]
    fn the_held_unit_includes_what_was_inherited() {
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e"), issue("t-1.aa", "in_progress")];
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add '안의 일' -e t-e"), Decision::Pass);
        let why = denied(&guard_create(&all, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(why.contains("-e t-e"), "물려받은 에픽을 안 가리킨다\n{why}");
    }

    /// 도움말을 보는 것은 만드는 것이 아니다. 우리가 심는 스킬이 바로 그
    /// 길을 일러 주므로, 막으면 규칙이 제가 시킨 것을 막는다.
    #[test]
    fn asking_for_help_is_not_creating() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in ["moai add --help", "moai add -h", "moai idea add --help"] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "{cmd}");
        }
    }

    /// 담아 두는 것은 언제나 자유고, 계획을 한 번에 세우는 것도 그렇다.
    ///
    /// `--from` 이 만드는 것은 에픽과 그 자식들이라 **그 자체로 한 단위**다.
    /// 막으면 계획을 세울 때마다 탈출구를 써야 하고, 탈출구가 일상이 되는
    /// 순간 규칙은 아무것도 안 지킨다.
    #[test]
    fn capturing_and_planning_stay_free() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in [
            "moai idea add '떠오른 것'",
            "moai add --from -",
            "moai add --from plan.md --dry-run",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "{cmd}");
        }
    }

    /// 동사를 **자리로** 읽는다. 글자로 찾던 판은 메모를 생성으로 보아 막고,
    /// 제목에 `idea` 가 든 생성은 반대로 통과시켰다.
    #[test]
    fn the_verb_is_a_position_not_a_word() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for free in ["moai note t-1 \"add 는 나중에\"", "moai show -g add", "moai mv t-1 done"] {
            assert_eq!(guard_create(&all, &cfg(), &here(), free), Decision::Pass, "{free}");
        }
        assert!(matches!(guard_create(&all, &cfg(), &here(), "moai add 'idea 정리'"), Decision::Deny(_)));
    }

    /// **줄바꿈도 토막을 가른다.** 갈래는 적혀 있었지만 그 앞의 공백 갈래가
    /// 먼저 잡아, 줄바꿈은 낱말만 끊고 토막은 안 끊었다. 여러 줄 명령이 한
    /// 토막으로 뭉쳐 규칙이 통째로 샜다.
    ///
    /// 앞선 시험이 이 길을 못 밟은 까닭도 적어 둔다 — 첫 줄이 `cd` 라
    /// 뭉쳐도 `moai add` 가 여전히 그 토막의 첫 명령이었다. **우연히 통과한
    /// 시험은 없는 시험보다 나쁘다.** 여기서는 첫 줄에 다른 `moai` 를 둔다.
    #[test]
    fn a_newline_also_ends_a_segment() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in [
            "moai show\nmoai add '딴 일'",
            "moai status\nmoai add '딴 일'\nmoai ready",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        assert!(calls_review("echo hi\n/code-review high"), "리뷰 호출이 샜다");
    }

    /// **`moai` 는 명령 자리에 있어야 한다.** 어디에 있든 그 낱말을 찾던 판은
    /// 글에 적힌 `moai add` 까지 생성으로 읽었다 — 리뷰 글을 이슈에 적는 일이
    /// 그래서 막혔다. 규칙이 제가 시킨 일을 막는 자리가 또 하나 있었던 것이다.
    #[test]
    fn moai_must_be_the_command_not_a_word() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for free in [
            "echo moai add hello",
            "grep -rn \"moai add\" .",
            "moai note t-1 -b - <<'MD'\nmoai add '제목' -e t-e 라고 일러 준다\nMD",
            "python3 - <<'PY'\nsubprocess.run([\"moai\", \"add\", \"제목\"])\nPY",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), free), Decision::Pass, "막혔다 — {free}");
        }
        // 환경변수를 앞세운 진짜 호출은 여전히 잡힌다.
        assert!(matches!(
            guard_create(&all, &cfg(), &here(), "MOAI_NOW=x moai add '딴 일'"),
            Decision::Deny(_)
        ));
    }

    /// heredoc 이 닫힌 **뒤**는 다시 명령이다. 속을 건너뛴다고 뒤까지
    /// 놓치면, 글 한 덩이를 앞세우는 것이 그대로 우회로가 된다.
    #[test]
    fn what_follows_a_heredoc_is_a_command_again() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let cmd = "cat <<'MD' > /tmp/x\n아무 글\nMD\nmoai add '딴 일'";
        assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다");
    }

    /// **같은 연산의 두 철자가 같이 움직인다.** `moai add` 만 보던 판은
    /// `moai issue add` 를 그냥 보냈다 — 규칙을 아는 쪽은 그것을 우회로로
    /// 쓰고, 모르는 쪽은 왜 한 번은 막히고 한 번은 안 막히는지 모른다.
    #[test]
    fn the_kind_namespaces_create_too() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in [
            "moai issue add \"딴 일\"",
            "moai epic add '딴 에픽'",
            "moai milestone add 'v0.2'",
            "moai add '딴 일' --type epic",
            // 종류를 고정한 쪽이 이긴다 — 이것은 이슈를 만든다.
            "moai issue add \"딴 일\" --type idea",
            // 제목에 든 낱말은 플래그가 아니다.
            "moai add '--type idea'",
            // `--` 뒤는 제목이다 — 이슈 `--type=idea` 가 선다.
            "moai add -- --type=idea",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        // 담아 두는 것은 그 어느 철자로도 자유다.
        for cmd in [
            "moai idea add '떠오른 것'",
            "moai add '떠오른 것' --type idea",
            "moai add --type=idea \"떠오른 것\"",
            "moai --json add --type idea \"떠오른 것\"",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), cmd), Decision::Pass, "막혔다 — {cmd}");
        }
    }

    // ── 규칙 2 — 고치기 전에 하나를 집는다 ──────────────────────────

    /// **도구가 아니라 고치는 파일로 가른다.** 도구로 가르면 스크래치패드
    /// 메모와 `src/` 의 한 줄이 같은 값으로 막힌다.
    #[test]
    fn only_the_work_in_the_repo_needs_a_held_issue() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for counted in ["/repo/src/store.rs", "/repo/CLAUDE.md", "src/main.rs"] {
            assert!(
                matches!(guard_edit(&all, &cfg(), &here(), root, counted), Decision::Deny(_)),
                "{counted} 를 안 셌다"
            );
        }
        for free in [
            "/repo/.moai/config.toml",
            "/repo/.claude/settings.json",
            "/repo/target/debug/x",
            "/tmp/scratch/memo.md",
            "/other/repo/src/main.rs",
        ] {
            assert_eq!(guard_edit(&all, &cfg(), &here(), root, free), Decision::Pass, "{free}");
        }
    }

    /// **껍데기로 쓰는 파일도 센다.** `Edit`·`Write` 만 보던 판은 `sed -i` 와
    /// 리다이렉션을 그대로 보냈다 — 규칙이 못 보는 길이 따로 있으면 규칙은
    /// 절반만 서 있다.
    #[test]
    fn writing_through_the_shell_is_counted_too() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "echo x > src/store.rs",
            "echo x >> src/store.rs",
            "printf x>src/store.rs",
            "cargo run 2> src/log.rs",
            "cat > src/store.rs <<'MD'\n본문\nMD",
            "sed -i 's/a/b/' src/store.rs",
            "sed -i.bak -e 's/a/b/' README.md src/store.rs",
            "sed --in-place=.bak -E 's/a/b/' src/store.rs",
            "sed -ni 's/a/b/p' src/store.rs",
            "echo x | tee src/store.rs",
            "echo x | tee -a /repo/CLAUDE.md",
            "cd /repo && echo x > /repo/src/store.rs",
            "echo x >| src/store.rs",
            "FOO=1 sed -i s/a/b/ src/store.rs",
        ] {
            assert!(
                matches!(guard_writes(&all, &cfg(), &here(), root, root, cmd), Decision::Deny(_)),
                "샜다 — {cmd}"
            );
        }
    }

    /// **잘못 막지 않는다.** 못 잡는 것보다 엉뚱한 것을 막는 쪽이 훨씬 나쁘다 —
    /// 규칙 1 이 명령줄 글자를 훑다가 `moai note` 를 막던 때가 그 증거다.
    #[test]
    fn the_shell_rule_never_blocks_what_writes_nothing_here() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "grep -rn x src > /dev/null",
            "cargo test 2>&1 | tail -20",
            "cargo build >&2",
            "echo x > /tmp/scratch/memo.md",
            "echo x > target/log.txt",
            "moai note t-1 \"a > src/store.rs 를 짚었다\"",
            "moai note t-1 'sed -i s/a/b/ src/store.rs'",
            "echo a \\> src/store.rs",
            "sed 's/a/b/' src/store.rs",
            "sed -n '1,20p' src/store.rs | tee /tmp/out",
            "echo x > \"$TMPDIR/x\"",
            "echo x > ~/notes.md",
            "cd /elsewhere && echo x > notes.md",
            "[[ a > b ]] && echo yes",
            "(( n > 3 )) && echo yes",
            "diff <(sort a) <(sort b)",
            "echo x | tee >(cat)",
            "moai add --from - <<'MD'\n# 에픽\n- 줄 > src/store.rs\nMD",
            // 따옴표는 줄을 넘는다. 줄마다 따옴표를 새로 세던 판은 둘째 줄을
            // 명령으로 읽어 `->` 의 `>` 를 리다이렉션으로 보았다.
            "git commit -m \"feat: x\n\n- draft -> accepted 로 바꾼다\"",
            "moai note t-1 \"첫 줄\n둘째 줄 > src/store.rs\"",
            // `#` 뒤는 주석이다.
            "cargo build  # draft -> accepted",
        ] {
            assert_eq!(guard_writes(&all, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
    }

    /// 상대 경로는 **껍데기의 자리**로 푼다. 저장소 뿌리로 풀면 하위 디렉터리에서
    /// 친 쓰기가 엉뚱한 파일로 읽힌다.
    #[test]
    fn a_shell_write_lands_where_the_shell_stands() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let why = denied(&guard_writes(&all, &cfg(), &here(), root, Path::new("/repo/docs"), "echo > a.md"))
            .to_string();
        assert!(why.contains("docs/a.md"), "{why}");
        // 저장소 밖에 서 있으면 상대 경로도 밖이다.
        assert_eq!(guard_writes(&all, &cfg(), &here(), root, Path::new("/tmp"), "echo > a.md"), Decision::Pass);
        // `.moai` 안에 서서 쓰는 것은 트래커 자신이다.
        assert_eq!(
            guard_writes(&all, &cfg(), &here(), root, Path::new("/repo/.moai"), "echo > x"),
            Decision::Pass
        );
    }

    /// 하나를 집으면 껍데기 쓰기도 그대로 지나간다.
    #[test]
    fn holding_one_opens_the_shell_too() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        assert_eq!(guard_writes(&all, &cfg(), &here(), root, root, "sed -i s/a/b/ src/store.rs"), Decision::Pass);
    }

    /// **리다이렉션은 낱말이 아니다.** 낱말로 두던 판은 `moai mv t-r done >
    /// /dev/null` 의 갈 칸을 `/dev/null` 로 읽어, 리뷰를 닫는 규칙이 출력을
    /// 버리는 것 하나로 샜다.
    #[test]
    fn a_redirection_does_not_hide_the_column() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in ["moai mv t-r done > /dev/null", "moai mv t-r done 2>/dev/null", "moai mv t-r done >/dev/null 2>&1"] {
            assert!(matches!(guard_close(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **거절문은 어긴 규칙의 이름으로 시작한다.** 스킬이 적은 규칙 제목과
    /// 글자가 같아야, 막힌 쪽이 무엇을 어겼는지 두 번 읽지 않는다.
    #[test]
    fn every_refusal_names_its_rule() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let reviewing = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for (n, d) in [
            (1, guard_create(&held, &cfg(), &here(), "moai add '딴 일'")),
            (2, guard_edit(&idle, &cfg(), &here(), root, "/repo/src/store.rs")),
            (2, guard_writes(&idle, &cfg(), &here(), root, root, "echo x > src/store.rs")),
            (3, guard_review(&held, &cfg(), &here())),
            (3, guard_review(&[epic("t-e"), review("t-r", "todo", Some("t-e"))], &cfg(), &here())),
            (3, guard_close(&reviewing, &cfg(), &here(), "moai mv t-r done")),
        ] {
            let why = denied(&d).to_string();
            let rule = crate::guide::rule_head(n);
            assert!(why.starts_with(&rule), "`{rule}` 로 시작하지 않는다\n{why}");
        }
    }

    /// 하나를 집으면 그대로 지나간다. 값은 왕복 한 번이다.
    #[test]
    fn holding_one_opens_the_repo() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        assert_eq!(guard_edit(&all, &cfg(), &here(), root, "/repo/src/store.rs"), Decision::Pass);
    }

    /// 막을 때는 집을 것을 함께 낸다. 고칠 명령 없는 거절은 게이트다.
    #[test]
    fn the_refusal_names_what_to_pick_up() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let why = denied(&guard_edit(&all, &cfg(), &here(), root, "/repo/src/store.rs")).to_string();
        assert!(why.contains("moai mv t-1 in_progress"), "집을 것을 안 낸다\n{why}");
        assert!(why.contains("src/store.rs"), "무엇을 고치려 했는지가 없다\n{why}");
    }

    // ── 규칙 3 — 리뷰도 이슈다 ──────────────────────────────────────

    /// 관점이 적힌 리뷰. **이것이 보통이다** — 규칙이 그것을 요구한다.
    fn review(id: &str, status: &str, epic: Option<&str>) -> Issue {
        let mut i = blank_review(id, status, epic);
        i.body = Some("락을 잡는 자리와 그 뒤의 쓰기를 본다".into());
        i
    }

    /// 제목만 있는 리뷰 줄.
    fn blank_review(id: &str, status: &str, epic: Option<&str>) -> Issue {
        let mut i = issue(id, status);
        i.title = "리뷰 — 저장 계층".into();
        i.tags = vec![REVIEW_TAG.to_string()];
        i.epic = epic.map(str::to_string);
        i
    }

    /// 리뷰 이슈가 지금 보는 것에 매여 있어야 지나간다.
    #[test]
    fn a_review_must_be_anchored_to_what_is_seen() {
        let cfg = cfg();
        let mut all = vec![epic("t-e"), epic("t-f"), under("t-1", "in_progress", "t-e")];

        all.push(review("t-r", "todo", Some("t-f")));
        let why = denied(&guard_review(&all, &cfg, &here())).to_string();
        assert!(why.contains("t-r(t-f)"), "안 매인 줄을 안 짚는다\n{why}");
        assert!(why.contains("--parent t-1"), "세울 길이 없다\n{why}");

        // 같은 에픽으로 옮기면 지나간다.
        all.last_mut().unwrap().epic = Some("t-e".into());
        assert_eq!(guard_review(&all, &cfg, &here()), Decision::Pass);
    }

    /// 집은 것이 없으면 **굴러가고 있는** 리뷰만 리뷰로 친다. 놀고 있는
    /// 리뷰 줄은 집으라고 말한다 — 그래야 상태가 실제를 가리킨다.
    #[test]
    fn with_nothing_held_only_a_running_review_counts() {
        let cfg = cfg();
        let mut all = vec![epic("t-e"), review("t-r", "todo", Some("t-e"))];
        let why = denied(&guard_review(&all, &cfg, &here())).to_string();
        assert!(why.contains("moai mv t-r in_progress"), "{why}");

        all[1].status = Status::new("in_progress");
        assert_eq!(guard_review(&all, &cfg, &here()), Decision::Pass);
    }

    /// 끝난 리뷰는 다음 리뷰의 면죄부가 되지 않는다.
    #[test]
    fn a_finished_review_is_no_pass_for_the_next() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r", "done", Some("t-e"))];
        assert!(matches!(guard_review(&all, &cfg(), &here()), Decision::Deny(_)));
    }

    /// **이어 붙인 명령을 버리지 않는다.** `cd … && …` 는 피하려는 수가 아니라
    /// 에이전트의 보통 말투다. 앞서 `;`·`&&`·`|` 에서 잘라 버리던 판은
    /// 그 말투 하나로 규칙 1 과 규칙 3 을 통째로 지나갔다.
    #[test]
    fn a_joined_command_does_not_slip_past() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in [
            "cd /repo && moai add '딴 일'",
            "true; moai add '딴 일'",
            "ls | grep x && moai add '딴 일'",
            "cd /repo\nmoai add '딴 일'",
        ] {
            assert!(
                matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)),
                "지나갔다 — {cmd}"
            );
        }
        // 뒷토막이 담아 두는 것이면 그대로 지나간다.
        assert_eq!(
            guard_create(&all, &cfg(), &here(), "cd /repo && moai idea add '떠오른 것'"),
            Decision::Pass
        );

        assert!(calls_review("cd /repo && /code-review high"));
    }

    /// **에픽이 없으면 에픽을 대라고 말하지 않는다.** 없는 에픽 자리에 이슈 id
    /// 를 넣어 일러 주던 자리다 — 시키는 대로 치면 `moai status` 에 "에픽으로
    /// 쓸 수 없는 것을 가리키는 줄" 이 늘고, 경고가 늘면 `closing` 이 세션을
    /// 붙든다. 훅이 시킨 대로 한 것이 훅에 걸리면 그건 규칙이 아니라 덫이다.
    #[test]
    fn the_refusal_never_invents_an_epic() {
        let loose = vec![issue("t-1", "in_progress")];
        let why = denied(&guard_create(&loose, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(!why.contains("-e t-1"), "이슈를 에픽이라고 가리킨다\n{why}");
        assert!(!why.contains("-e "), "없는 에픽을 대라고 한다\n{why}");
        assert!(why.contains("--parent t-1"), "자식으로 다는 길이 없다\n{why}");
        assert!(why.contains("idea add"), "담아 두는 길이 없다\n{why}");
        // 에픽 없는 일의 자식은 부모를 안 붙든다 — 첫 칸에 두면 그 일이 안 닫힌다고 비치지 않는다.
        assert!(why.contains("idea 가 아니다"), "일이 이것 없이 안 끝나는 경우를 안 가른다\n{why}");
        assert!(!why.contains("첫 칸에 둔다"), "에픽 없는 일에 자식이 그 일을 열어 둔다고 비친다\n{why}");

        // 에픽이 있으면 그때는 에픽을 가리킨다.
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let why = denied(&guard_create(&held, &cfg(), &here(), "moai add '딴 일'")).to_string();
        assert!(why.contains("-e t-e"), "{why}");
    }

    /// `--from` 도 **토큰으로** 본다. 글자로 찾으면 제목이 그 낱말을 담은
    /// `moai add "--from 을 나중에"` 가 규칙을 통째로 지나간다.
    #[test]
    fn from_is_a_token_not_a_substring() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        assert!(matches!(
            guard_create(&all, &cfg(), &here(), "moai add '--from 을 나중에 본다'"),
            Decision::Deny(_)
        ));
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add --from -"), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add --from=plan.md"), Decision::Pass);
    }

    /// **`..` 를 접는다.** 접지 않으면 판정이 양쪽으로 다 틀린다 — 저장소 안의
    /// 파일이 안 세는 자리로 보이고, 저장소 밖의 파일이 안으로 보인다.
    #[test]
    fn a_dotdot_path_lands_where_it_really_is() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        // 저장소 안이다 — `.moai` 를 지나왔어도 닿는 곳은 `src/store.rs` 다.
        assert!(matches!(
            guard_edit(&all, &cfg(), &here(), root, ".moai/../src/store.rs"),
            Decision::Deny(_)
        ));
        // 저장소 밖이다 — 붙여 놓은 글자만 보면 안으로 보인다.
        assert_eq!(guard_edit(&all, &cfg(), &here(), root, "../elsewhere/x.rs"), Decision::Pass);
        assert_eq!(guard_edit(&all, &cfg(), &here(), root, "/repo/../elsewhere/x.rs"), Decision::Pass);
    }

    /// **링크 철자로 부른 파일도 저장소 안이다.** 훅의 `root` 는 `current_dir()` 이 준 풀린
    /// 철자인데 `file_path` 는 사람이 친 철자라, 글자로만 견주면 규칙 2 가 통째로 샌다.
    ///
    /// **아직 없는 파일도 같게 풀린다** — 새로 만드는 자리에서만 규칙이 꺼지면 막아야 할 것을
    /// 되레 놓친다. 저장소 밖은 링크를 풀어도 밖이어야 한다.
    ///
    /// 링크를 만드는 시험이라 unix 에서만 돈다 — 가리지 않으면 unix 가 아닌 곳에서 이 한 시험이
    /// 아니라 바이너리의 시험 전부가 컴파일되지 않는다(`user_config`·`read_marks` 의 링크 시험과 같다).
    #[test]
    #[cfg(unix)]
    fn a_linked_spelling_still_lands_inside_the_repo() {
        let s = crate::scratch::Scratch::real("hooklink");
        let root = s.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/store.rs"), "").unwrap();
        std::fs::create_dir_all(s.path().join("elsewhere")).unwrap();
        let link = s.path().join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();

        let all = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let judge = |at: PathBuf| guard_edit(&all, &cfg(), &here(), &root, &at.to_string_lossy());
        // 있는 파일도, 아직 없는 파일도 링크를 지나 같은 자리로 풀린다.
        assert!(matches!(judge(link.join("src/store.rs")), Decision::Deny(_)));
        assert!(matches!(judge(link.join("src/새파일.rs")), Decision::Deny(_)));
        // 안 세는 자리와 저장소 밖은 링크를 풀어도 그대로다.
        assert_eq!(judge(link.join(".moai/issues.jsonl")), Decision::Pass);
        assert_eq!(judge(s.path().join("elsewhere/x.rs")), Decision::Pass);
    }

    /// 닫을 때의 셈법이 규칙 3 과 같아야 한다. 거절문이 시킨 대로 `--parent`
    /// 로 세운 리뷰가 규칙 3 은 지나가면서 닫을 때는 아무도 안 챙기면, 한
    /// 규칙의 두 짝이 서로 다른 말을 한다.
    #[test]
    fn both_halves_of_the_review_rule_agree() {
        let cfg = cfg();
        let mut child = review("t-1.aa", "todo", None);
        child.epic = None;
        let all = vec![issue("t-1", "in_progress"), child];

        assert_eq!(guard_review(&all, &cfg, &here()), Decision::Pass, "규칙 3 이 안 받는다");
        let Decision::Block(why) = closing(&all, &all, &cfg, &here(), 0, None) else {
            panic!("닫을 때 그 리뷰를 안 챙긴다");
        };
        assert!(why.contains("리뷰 이슈 t-1.aa"), "{why}");
    }

    /// 일러 주는 줄은 **모두 같은 자리에서 시작한다.** 줄 잇기가 첫 줄의
    /// 들여쓰기를 먹어, 첫 명령만 왼쪽 끝에 붙던 자리다.
    #[test]
    fn every_offered_command_lines_up() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "todo", None)];
        let why = denied(&guard_review(&all, &cfg(), &here())).to_string();
        let lines: Vec<&str> = why.lines().filter(|l| l.trim_start().starts_with("moai ")).collect();
        assert!(lines.len() >= 3, "일러 주는 줄이 모자라다\n{why}");
        assert!(
            lines.iter().all(|l| l.starts_with("  ")),
            "줄마다 시작이 다르다\n{why}"
        );
    }

    /// **관점 없는 리뷰 줄은 리뷰로 안 친다.** 제목뿐인 줄은 무엇을 왜 보는지를
    /// 남기지 않아, 다음 사람이 그 리뷰가 무엇을 훑었는지 영영 모른다 — 리뷰가
    /// 낸 글 중 값이 큰 쪽이 통째로 사라지는 자리다.
    #[test]
    fn a_review_without_an_angle_is_not_a_review_yet() {
        let cfg = cfg();
        let mut all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        all.push(blank_review("t-r", "todo", Some("t-e")));

        let why = denied(&guard_review(&all, &cfg, &here())).to_string();
        assert!(why.contains("moai edit t-r -b -"), "적을 길을 안 낸다\n{why}");

        // 적으면 지나간다. 값은 왕복 한 번이다.
        all.last_mut().unwrap().body = Some("락을 잡는 자리를 본다".into());
        assert_eq!(guard_review(&all, &cfg, &here()), Decision::Pass);
    }

    /// 공백만 적은 것은 안 적은 것이다.
    #[test]
    fn whitespace_is_not_an_angle() {
        let mut all = vec![issue("t-1", "in_progress"), blank_review("t-r", "todo", None)];
        all[1].body = Some("  \n \t\n".into());
        assert!(matches!(guard_review(&all, &cfg(), &here()), Decision::Deny(_)));
    }

    // ── 규칙 3 의 뒷짝 — 닫을 때 무엇이 나왔는지 ────────────────────

    /// 리뷰를 닫을 때 한 줄을 남긴다. **`-m` 의 있고 없음만 본다** — 저널을
    /// 접어야 답이 나오는 질문을 규칙이 묻기 시작하면 옛 `event.rs` 로 간다.
    #[test]
    fn closing_a_review_leaves_a_line() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        let why = denied(&guard_close(&all, &cfg(), &here(), "moai mv t-r done")).to_string();
        assert!(why.contains("moai note t-r"), "원문을 붙일 길이 없다\n{why}");
        assert!(why.contains("moai mv t-r done -m"), "닫을 길이 없다\n{why}");

        for ok in [
            "moai mv t-r done -m \"넷을 반영하고 하나는 t-9 로 넘겼다\"",
            "moai mv t-r done --msg \"반영\"",
            "moai mv t-r done --msg=반영",
        ] {
            assert_eq!(guard_close(&all, &cfg(), &here(), ok), Decision::Pass, "{ok}");
        }
    }

    /// **리뷰가 아닌 것은 안 막는다.** 보통 이슈를 닫는 데 한 줄을 요구하면
    /// 그건 딴 규칙이고, 그 규칙은 아무도 원한 적이 없다.
    #[test]
    fn closing_ordinary_work_needs_no_line() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "todo", None)];
        assert_eq!(guard_close(&all, &cfg(), &here(), "moai mv t-1 done"), Decision::Pass);
        assert_eq!(guard_close(&all, &cfg(), &here(), "moai mv t-1 review"), Decision::Pass);
    }

    /// **미루는 것은 막지 않는다.** 안 하기로 한 리뷰에 결과를 적으라고 하면
    /// 그것은 규칙이 아니라 덫이다.
    #[test]
    fn deferring_a_review_is_free() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "todo", None)];
        assert_eq!(guard_close(&all, &cfg(), &here(), "moai defer t-r -m \"다음 분기\""), Decision::Pass);
        assert_eq!(guard_close(&all, &cfg(), &here(), "moai defer t-r"), Decision::Pass);
    }

    /// **미룬 일 밑의 리뷰는 열린 리뷰가 아니다** — 제 줄을 미룬 리뷰와 같다.
    /// `wip` 는 물려받은 미룸으로 그 리뷰를 초점에서 빼는데 규칙 3 만 열린 리뷰로
    /// 치면, 이미 집은 리뷰를 "집으라" 고 막고 그대로 쳐도 같은 거절이 돌아온다.
    #[test]
    fn a_review_under_deferred_work_is_not_an_open_review() {
        let cfg = cfg();
        let mut work = issue("t-1", "in_progress");
        work.deferred_at = Some("2026-01-01T00:00:00Z".into());
        let all = vec![work, review("t-1.aa", "in_progress", None)];

        let why = denied(&guard_review(&all, &cfg, &here())).to_string();
        assert!(!why.contains("moai mv t-1.aa in_progress"), "이미 집은 리뷰를 집으라 한다\n{why}");
        assert_eq!(closing(&all, &all, &cfg, &here(), 0, None), Decision::Pass, "계획 밖의 리뷰로 세션을 붙든다");
    }

    /// 이미 닫힌 리뷰를 다시 옮기는 것도 막지 않는다. 막을 것이 없다.
    #[test]
    fn a_closed_review_is_not_closed_again() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "done", None)];
        assert_eq!(guard_close(&all, &cfg(), &here(), "moai mv t-r done"), Decision::Pass);
    }

    /// 여럿을 한 번에 옮길 때도 그중 리뷰가 있으면 본다.
    #[test]
    fn a_review_hidden_in_a_batch_still_counts() {
        let all = vec![
            issue("t-1", "in_progress"),
            issue("t-2", "in_progress"),
            review("t-r", "in_progress", None),
        ];
        assert!(matches!(guard_close(&all, &cfg(), &here(), "moai mv t-1 t-2 t-r done"), Decision::Deny(_)));
    }

    /// **일러 준 명령은 그대로 쳐서 지나가야 한다.**
    ///
    /// 이 시험이 없어 같은 덫을 세 번 놨다 — `-e <이슈>` 를 일러 줘 경고를
    /// 만들게 했고, `-b` 없는 `add` 를 일러 줘 관점이 없다고 막았고, `-m` 없는
    /// `done` 을 일러 줘 결과가 없다고 막았다. 거절문에서 명령 줄을 뽑아
    /// **도로 판정에 먹인다.** 자리표시자는 실제 id 로 바꾼다.
    #[test]
    fn every_offered_command_actually_passes() {
        let cfg = cfg();
        let all = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            blank_review("t-r", "in_progress", Some("t-e")),
        ];
        let reviewed = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            review("t-r", "in_progress", Some("t-e")),
        ];

        let refusals = [
            (guard_create(&all, &cfg, &here(), "moai add '딴 일'"), &all),
            (guard_review(&all, &cfg, &here()), &all),
            (guard_close(&reviewed, &cfg, &here(), "moai mv t-r done"), &reviewed),
        ];
        let mut checked = 0;
        for (decision, issues) in &refusals {
            let Decision::Deny(why) = decision else {
                panic!("막지 않았다 — {decision:?}");
            };
            for line in why.lines() {
                let line = line.trim();
                if !line.starts_with("moai ") {
                    continue;
                }
                // 사람이 채울 자리는 실제 값으로 바꿔 친다.
                let cmd = line
                    .replace("<id>", "t-r")
                    .replace("<에픽>", "t-e")
                    .replace("<리뷰 원문>", "/tmp/review.txt");
                assert_eq!(
                    guard_create(issues, &cfg, &here(), &cmd),
                    Decision::Pass,
                    "일러 준 명령이 규칙 1 에 막힌다 — {cmd}"
                );
                assert_eq!(
                    guard_close(issues, &cfg, &here(), &cmd),
                    Decision::Pass,
                    "일러 준 명령이 닫기 규칙에 막힌다 — {cmd}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 6, "일러 주는 명령을 {checked}개밖에 못 찾았다");
    }

    /// 일러 준 대로 리뷰를 세우면 **그 리뷰로 곧장 리뷰를 부를 수 있다.**
    /// 관점 없이 세우게 일러 주면 왕복이 한 번 더 는다.
    #[test]
    fn the_offered_review_is_born_with_its_angle() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let Decision::Deny(why) = guard_review(&all, &cfg(), &here()) else {
            panic!("막지 않았다");
        };
        let made = why
            .lines()
            .find(|l| l.trim_start().starts_with("moai add"))
            .expect("리뷰를 세울 명령이 없다");
        assert!(made.contains("-t review"), "{made}");
        assert!(made.contains("-b "), "관점 없이 세우라고 한다 — {made}");
    }

    /// **빈 `-m` 은 안 적은 것이다.** 있고 없음만 보던 판은 두 글자로 지나갔다 —
    /// 이 규칙이 지키려던 단 하나(다음 사람이 읽을 한 줄)가 그대로 무너진다.
    #[test]
    fn an_empty_message_is_no_message() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for sneaky in [
            "moai mv t-r done -m \"\"",
            "moai mv t-r done --msg=",
            "moai mv t-r done -m \"   \"",
        ] {
            assert!(
                matches!(guard_close(&all, &cfg(), &here(), sneaky), Decision::Deny(_)),
                "빈 한 줄로 지나갔다 — {sneaky}"
            );
        }
    }

    /// **안 매인 옛 리뷰 줄은 그냥 닫게 둔다.** 저장소 전체를 보던 판은, 옛
    /// 세션이 남긴 줄을 치우려는 사람에게 돌린 적도 없는 리뷰의 결과를
    /// 지어내라고 했다. `closing` 이 같은 줄을 아예 안 세는 것과도 어긋났다.
    #[test]
    fn a_stale_review_can_just_be_closed() {
        let cfg = cfg();
        let all = vec![
            epic("t-e"),
            epic("t-f"),
            under("t-1", "in_progress", "t-e"),
            review("t-old", "todo", Some("t-f")),
        ];
        assert_eq!(guard_close(&all, &cfg, &here(), "moai mv t-old done"), Decision::Pass);

        // 지금 보는 것에 매인 리뷰는 그대로 붙든다.
        let mine = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            review("t-r", "in_progress", Some("t-e")),
        ];
        assert!(matches!(guard_close(&mine, &cfg, &here(), "moai mv t-r done"), Decision::Deny(_)));
    }

    // ── 세션을 닫을 때 ──────────────────────────────────────────────

    /// 집은 채 닫으려 하면 붙든다. 옮길 길과 미룰 길을 함께 낸다.
    #[test]
    fn closing_holds_on_what_is_still_held() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("moai mv t-1 review\n") && why.contains("moai mv t-1 done"), "{why}");
        assert!(!why.contains('|'), "그대로 치면 파이프가 되는 줄을 일러 준다\n{why}");
        assert!(why.contains("moai defer t-1"), "{why}");
        assert!(why.contains(&crate::guide::handoff("t-1")), "이어받을 한 줄을 안 일러 준다\n{why}");
    }

    /// **이미 review 인 줄에 review 로 옮기라고 하지 않는다.** 집은 것은 첫 칸도
    /// 끝난 칸도 아닌 칸 전부라 review 도 집은 것이다 — 갈 곳은 그 뒤 칸뿐이다. 되돌아가는
    /// 칸은 미루면 에픽이 닫히는 줄에만 선다(`closing_warns_before_deferring_the_last_member`).
    #[test]
    fn closing_offers_only_the_columns_ahead() {
        let all = vec![epic("t-e"), under("t-1", "review", "t-e")];
        let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 0, None) else {
            panic!("review 인 줄을 안 붙들었다");
        };
        assert!(why.contains("moai mv t-1 done"), "{why}");
        assert!(!why.contains("moai mv t-1 review"), "제자리걸음을 시킨다\n{why}");
        assert!(!why.contains("moai mv t-1 todo"), "보통 줄에 되돌아가는 칸을 댄다\n{why}");
    }

    /// **에픽을 열어 두는 마지막 멤버에 미룸을 "지금 안 할 것이면" 으로만 대지 않는다**(moai-8ema).
    /// 끝난 멤버 곁에 그것 하나 남았으면, 시킨 대로 미루는 순간 에픽이 목적을 못 이룬 채 `done`
    /// 으로 선다 — moai-l288 이 막은 문을 훅이 도로 연다.
    #[test]
    fn closing_warns_before_deferring_the_last_member() {
        let mut shelved = under("t-3", "todo", "t-e");
        shelved.deferred_at = Some("2026-01-01T00:00:00Z".into());
        // 끝난 것 하나, 미룬 것 하나, 이것 — 미루면 셀 멤버가 끝난 것뿐이다.
        let last = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "done", "t-e"), shelved];
        let Decision::Block(why) = closing(&last, &last, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("t-e 의 목적을 접을 때만"), "마지막 멤버를 그냥 미루라고 한다\n{why}");
        assert!(!why.contains("지금 안 할 것이면"), "{why}");
        // **결정을 기다리는 것이면 첫 칸으로 되돌린다**(moai-1plu, 사용자 결정 A) — 갈림길 1 과 한 말.
        // 무엇을 기다리는지를 `-m` 으로 남긴다 — 첫 칸의 줄은 `ready` 에 선다(리뷰 moai-dw63.nzw).
        assert!(
            why.contains(
                "moai mv t-1 todo -m '무엇을 기다리나'      결정을 기다리는 것이면 — 첫 칸에 두면 t-e 가 열린 채 남는다"
            ),
            "결정을 기다리는 마지막 멤버에 첫 칸을 안 댄다\n{why}"
        );
        // 쥔 채 이어 할 길도 그대로 선다.
        assert!(why.contains(&format!("{}      이어서 할 것이면", crate::guide::handoff("t-1"))), "{why}");

        // **첫 칸은 설정에서 읽는다** — 첫 칸이 `todo` 가 아닌 저장소에서 `todo` 를 대면 `mv` 가 거절한다.
        let custom = Config::parse("prefix = \"t\"\nstatuses = \"backlog, doing, done\"\n").unwrap();
        let there = vec![epic("t-e"), under("t-1", "doing", "t-e"), under("t-2", "done", "t-e")];
        let Decision::Block(why) = closing(&there, &there, &custom, &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("moai mv t-1 backlog -m"), "설정의 첫 칸을 안 댄다\n{why}");
        assert!(!why.contains("moai mv t-1 todo"), "{why}");

        // 남은 멤버가 있으면 미뤄도 에픽이 안 닫힌다 — 보통 줄이다.
        let more = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "done", "t-e"), under("t-3", "todo", "t-e")];
        // 끝난 멤버가 없으면 미뤄도 에픽은 첫 칸이다.
        let fresh = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        // 에픽 없는 일은 붙들 에픽이 없다.
        let loose = vec![issue("t-1", "in_progress"), issue("t-2", "done")];
        // 제 조상에게서 미룸을 받은 에픽 — 칸의 셈은 그 미룸으로 멤버를 안 빼니, 이것을 미뤄도
        // 첫 칸의 자식이 에픽을 열어 둔다. 멤버를 손으로 세던 판은 여기서 닫힌다고 했다.
        let mut parked = issue("t-x", "todo");
        parked.deferred_at = Some("2026-01-01T00:00:00Z".into());
        let nested = vec![
            parked,
            epic("t-x.e"),
            under("t-1", "in_progress", "t-x.e"),
            under("t-2", "done", "t-x.e"),
            issue("t-x.e.c", "todo"),
        ];
        for all in [more, fresh, loose, nested] {
            let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 0, None) else {
                panic!("안 붙들었다");
            };
            assert!(why.contains("moai defer t-1 -m '왜'      지금 안 할 것이면"), "{why}");
            assert!(!why.contains("목적을 접을 때만"), "닫히지 않는 에픽을 댄다\n{why}");
            // 되돌아가는 칸은 미루면 닫히는 줄에만 댄다 — 보통 줄에는 앞 칸뿐이다.
            assert!(!why.contains("moai mv t-1 todo"), "{why}");
        }
    }

    /// **미루면 밑의 줄도 함께 빠진다** — 칸을 읽는 자 그대로 잰다(moai-dw63.e31). 규칙 3 이 시킨
    /// 대로 `--parent` 로 세운 리뷰 자식은 부모의 미룸을 물려받는데, 그것을 남은 멤버로 세던 판은
    /// 가장 흔한 모양에서 경고를 놓쳤다. 같은 에픽을 둘 집었으면 줄마다 댄 미룸을 다 치는 순간이
    /// 물을 칸이다.
    #[test]
    fn closing_warns_when_deferring_what_is_held_would_close_the_epic() {
        let open_child = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            under("t-2", "done", "t-e"),
            review("t-1.r", "todo", None),
        ];
        let held_child = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            under("t-2", "done", "t-e"),
            review("t-1.r", "in_progress", None),
        ];
        let two_held = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            under("t-2", "done", "t-e"),
            under("t-4", "in_progress", "t-e"),
        ];
        // (본 것, 미루면 닫힌다고 댈 줄, 그 가운데 첫 칸 되돌리기를 댈 줄). **리뷰 줄에는 첫 칸을 안
        // 댄다**(리뷰 moai-dw63.nzw) — 리뷰가 결정을 기다리면 그 결정은 멤버로 세우고 리뷰는 낸 글과
        // 함께 닫는다. 첫 칸에 되돌린 리뷰는 낸 글 없이 `ready` 에 선다.
        for (all, warned, parked) in [
            (open_child, vec!["t-1"], vec!["t-1"]),
            (held_child, vec!["t-1", "t-1.r"], vec!["t-1"]),
            (two_held, vec!["t-1", "t-4"], vec!["t-1", "t-4"]),
        ] {
            let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 0, None) else {
                panic!("안 붙들었다");
            };
            for id in &warned {
                assert!(
                    why.contains(&format!("moai defer {id} -m '왜'      t-e 의 목적을 접을 때만")),
                    "미루면 닫히는 에픽을 안 댄다 — {id}\n{why}"
                );
                let offered = why.contains(&format!("moai mv {id} todo -m"));
                assert_eq!(offered, parked.contains(id), "첫 칸 되돌리기 — {id}\n{why}");
            }
            assert!(!why.contains("지금 안 할 것이면"), "{why}");
        }

        // **잴 줄은 받는 쪽이 겹쳐 준 것이다** — 워크트리의 스냅샷은 main 에서 끝낸 멤버를 모른다.
        let stale = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "in_progress", "t-e")];
        let latest = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "done", "t-e")];
        let beside = away(&["t-2"]);
        let Decision::Block(why) = closing(&stale, &stale, &cfg(), &beside, 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("지금 안 할 것이면"), "{why}");
        let Decision::Block(why) = closing(&stale, &latest, &cfg(), &beside, 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("t-e 의 목적을 접을 때만"), "main 에서 끝낸 멤버를 못 보고 그냥 미루라고 한다\n{why}");
    }

    /// **main 에서 이미 놓은 줄은 안 붙든다**(리뷰 moai-dw63.nzw). 딸린 워크트리의 스냅샷은 그 줄을
    /// 아직 집은 것으로 든다 — 그것만 보면 `moai -C <루트>` 로 첫 칸에 되돌리거나 미룬 줄을, 그
    /// 워크트리에 여는 세션마다 도로 놓으라고 붙든다. 미룬 줄에 대던 "첫 칸에 두면 열린 채
    /// 남는다" 는 거짓이었다 — `mv` 는 미룸을 안 푼다.
    #[test]
    fn closing_lets_go_of_what_main_has_already_released() {
        let stale = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), under("t-2", "done", "t-e")];
        let parked = vec![epic("t-e"), under("t-1", "todo", "t-e"), under("t-2", "done", "t-e")];
        let mut shelved = under("t-1", "in_progress", "t-e");
        shelved.deferred_at = Some("2026-01-01T00:00:00Z".into());
        let deferred = vec![epic("t-e"), shelved, under("t-2", "done", "t-e")];
        for latest in [parked, deferred] {
            assert_eq!(closing(&stale, &latest, &cfg(), &here(), 0, None), Decision::Pass, "{latest:?}");
        }
        // 겹친 줄에서도 집혀 있으면 그대로 붙든다.
        let Decision::Block(why) = closing(&stale, &stale, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("moai mv t-1 todo -m"), "{why}");

        // **main 에서 닫거나 미룬 리뷰는 집은 줄에서 빠진 뒤 "열린 리뷰" 로 돌아오지 않는다.**
        let reviewing = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            under("t-2", "done", "t-e"),
            review("t-1.r", "in_progress", None),
        ];
        let mut closed = reviewing.clone();
        closed[3].status = Status::new("done");
        let mut put_by = reviewing.clone();
        put_by[3].deferred_at = Some("2026-01-01T00:00:00Z".into());
        for latest in [closed, put_by] {
            let Decision::Block(why) = closing(&reviewing, &latest, &cfg(), &here(), 0, None) else {
                panic!("여기서 집은 t-1 을 안 붙든다");
            };
            assert!(why.contains("moai defer t-1 "), "{why}");
            assert!(!why.contains("t-1.r"), "main 에서 놓은 리뷰를 도로 댄다\n{why}");
        }
    }

    /// **집은 리뷰 줄은 낸 글과 함께 닫는 걸음을 댄다**(리뷰 moai-dw63.nzw). `-m` 없는 `done` 을
    /// 대면 시킨 대로 친 줄을 규칙 3 이 막는다 — 훅이 일러 준 명령을 훅이 막는 덫이다.
    #[test]
    fn closing_offers_a_held_review_the_steps_rule_three_lets_through() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains(&crate::guide::close_steps("t-r", "moai")), "{why}");
        assert!(!why.contains("moai mv t-r done     "), "규칙 3 이 막는 줄을 댄다\n{why}");
        assert!(why.contains("moai mv t-r review\n"), "앞 칸은 그대로 댄다\n{why}");
        // 보통 줄은 그대로다.
        assert!(why.contains("moai mv t-1 done     제목"), "{why}");
        let steps = crate::guide::close_steps("t-r", "moai");
        let close = steps.lines().last().unwrap().trim();
        assert_eq!(guard_close(&all, &cfg(), &here(), close), Decision::Pass, "{close}");
    }

    /// 다 옮겼고 경고도 안 늘었으면 조용히 보낸다.
    #[test]
    fn a_clean_session_closes_quietly() {
        let all = vec![epic("t-e"), under("t-1", "done", "t-e")];
        assert_eq!(closing(&all, &all, &cfg(), &here(), 3, Some(3)), Decision::Pass);
    }

    /// 경고가 늘었으면 그 사실만 말한다.
    #[test]
    fn a_growing_warning_count_is_named() {
        let all = vec![epic("t-e"), under("t-1", "done", "t-e")];
        let Decision::Block(why) = closing(&all, &all, &cfg(), &here(), 5, Some(3)) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("3 에서 5"), "{why}");
    }

    /// **굴러가는 리뷰와 매인 리뷰만 센다.** 저장소에 남은 옛 리뷰 줄까지
    /// 세면 매 세션 같은 줄이 나오고, 그러면 아무도 안 읽는다.
    #[test]
    fn an_unrelated_open_review_does_not_nag_forever() {
        let cfg = cfg();
        let far = vec![epic("t-e"), epic("t-f"), under("t-1", "done", "t-e"), review("t-r", "todo", Some("t-f"))];
        assert_eq!(closing(&far, &far, &cfg, &here(), 0, None), Decision::Pass);

        let near = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r", "todo", Some("t-e"))];
        let Decision::Block(why) = closing(&near, &near, &cfg, &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("리뷰 이슈 t-r"), "{why}");
    }

    /// **옆 워크트리가 돌리는 리뷰로는 안 붙든다**(리뷰 moai-dw63.nzw) — 규칙 3 과 같은 자다. 같은
    /// 에픽의 옆 일꾼이 `--parent` 로 세운 리뷰를 이 세션에 닫으라고 하면, 돌린 적도 없는 리뷰의
    /// 결과를 적으라는 말이 된다.
    #[test]
    fn closing_leaves_the_review_another_worktree_runs() {
        let all = vec![
            epic("t-e"),
            under("t-1", "in_progress", "t-e"),
            under("t-2", "in_progress", "t-e"),
            review("t-2.r", "in_progress", None),
        ];
        let beside = away(&["t-2"]);
        let Decision::Block(why) = closing(&all, &all, &cfg(), &beside, 0, None) else {
            panic!("여기서 집은 것을 안 붙든다");
        };
        assert!(why.contains("moai defer t-1"), "{why}");
        assert!(!why.contains("t-2.r"), "옆 워크트리의 리뷰를 닫으라고 한다\n{why}");
        let rule = denied(&guard_review(&all, &cfg(), &beside)).to_string();
        assert!(!rule.contains("t-2.r"), "규칙 3 의 두 짝이 갈렸다\n{rule}");
        // 닫기 규칙도 같은 자다 — 세지 않는 리뷰에 돌린 적 없는 결과를 요구하지 않는다.
        assert_eq!(guard_close(&all, &cfg(), &beside, "moai mv t-2.r done"), Decision::Pass);
        // 여기서 집은 일에 매인 리뷰는 그대로 붙든다.
        let mut own = all.clone();
        own.push(review("t-1.r", "todo", None));
        let Decision::Block(why) = closing(&own, &own, &cfg(), &beside, 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(why.contains("리뷰 이슈 t-1.r"), "{why}");
        assert!(matches!(guard_close(&own, &cfg(), &beside, "moai mv t-1.r done"), Decision::Deny(_)));
    }

    /// 보드는 머리말과 함께 실린다. 머리말이 없으면 보드가 무엇을 하라는
    /// 뜻인지가 안 붙는다.
    #[test]
    fn the_board_carries_its_lead() {
        let Decision::Context(c) = board(&["이슈 3".into(), "todo 3".into()]) else {
            panic!("보드가 안 실렸다");
        };
        assert!(c.starts_with(LEAD), "{c}");
        assert!(c.contains("이슈 3"), "{c}");
    }

    /// 싣는 글에 색이 섞이면 안 된다. 계약 JSON 안의 이스케이프는 아무도
    /// 안 걷어내고, 받는 쪽 화면에 그 글자가 그대로 뜬다.
    #[test]
    fn no_color_leaks_into_what_is_carried() {
        let painted = crate::style::paint(crate::style::HEAD, "이슈 3");
        assert!(painted.contains('\u{1b}'), "시험이 칠하지 못했다 — {painted:?}");
        let Decision::Context(c) = board(&[painted]) else {
            panic!("보드가 안 실렸다");
        };
        assert!(!c.contains('\u{1b}'), "{c:?}");
        assert!(c.contains("이슈 3"), "{c:?}");
    }

    /// 낼 것이 없으면 아무 말도 안 한다. 빈 보드를 싣는 것은 값만 치른다.
    #[test]
    fn an_empty_board_says_nothing() {
        assert_eq!(board(&[]), Decision::Pass);
        assert_eq!(board(&["   ".into()]), Decision::Pass);
    }

    /// 집은 것이 없으면 접힌 뒤에도 조용하다.
    #[test]
    fn nothing_carried_stays_quiet() {
        let all = vec![issue("t-1", "todo"), issue("t-2", "done")];
        assert_eq!(carried(&all, &all, &cfg(), &here()), Decision::Pass);
    }

    /// 집은 것은 id 와 제목으로 실린다 — 압축 뒤에 그것으로 다시 찾는다.
    #[test]
    fn what_is_held_survives_the_fold() {
        let all = vec![issue("t-1", "in_progress"), issue("t-2", "todo")];
        let Decision::Context(c) = carried(&all, &all, &cfg(), &here()) else {
            panic!("집은 것이 안 실렸다");
        };
        assert!(c.contains("t-1"), "{c}");
        assert!(!c.contains("t-2"), "{c}");
    }

    /// **main 에서 이미 놓은 줄은 접힌 뒤에도 안 싣는다**(리뷰 moai-3k2d.1df) — `closing` 과 같은 자다
    /// (`closing_lets_go_of_what_main_has_already_released`). 따로 재던 판은 딸린 워크트리의 낡은 스냅샷대로
    /// main 이 닫은 줄을 "압축 전부터 집고 있다" 로 실었는데, 같은 세션의 `Stop` 은 그 줄로 안 붙들었다.
    #[test]
    fn what_main_has_already_released_is_not_carried() {
        let stale = vec![issue("t-1", "in_progress"), issue("t-2", "in_progress")];
        let latest = vec![issue("t-1", "done"), issue("t-2", "in_progress")];
        let Decision::Context(c) = carried(&stale, &latest, &cfg(), &here()) else {
            panic!("아직 집은 줄이 안 실렸다");
        };
        assert!(c.contains("t-2") && !c.contains("t-1"), "main 이 닫은 줄을 실었다\n{c}");
        assert!(closing(&stale, &latest, &cfg(), &here(), 0, None).blocks(), "Stop 과 접힌 뒤가 갈렸다");
        let done = vec![issue("t-1", "done"), issue("t-2", "done")];
        assert_eq!(carried(&stale, &done, &cfg(), &here()), Decision::Pass);
    }

    // ── 셸이 읽는 대로 읽는다 ────────────────────────────────────────

    /// **읽는 리다이렉션은 쓰기도 낱말도 아니다.** `tee /tmp/x <<'EOF'` 의
    /// `<<EOF` 를 `tee` 가 쓰는 파일로 세어, 저장소 밖에 쓰는 흔한 명령이 막혔다.
    /// 낱말에 남은 `< /dev/null` 은 리뷰를 닫는 명령의 갈 칸도 가렸다.
    #[test]
    fn an_input_redirection_is_neither_a_word_nor_a_write() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "tee /tmp/notes.md <<'EOF'\nhello\nEOF",
            "tee /tmp/notes.md > /dev/null <<EOF\nhello\nEOF",
            "tee -a /tmp/log <<< \"hi\"",
            "tee /tmp/copy.txt < README.md",
            "sed -i 's/a/b/' /tmp/x < /dev/null",
            "sed -i -f /dev/stdin /tmp/x <<'EOF'\ns/a/b/\nEOF",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        let real = "tee src/store.rs <<'EOF'\nx\nEOF";
        assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, real), Decision::Deny(_)), "샜다 — {real}");

        let reviewing = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in [
            "moai mv t-r done < /dev/null",
            "moai mv t-r done <<< ''",
            "moai mv t-r done 0</dev/null",
            "moai mv t-r done <&-",
            "moai mv t-r done >& /dev/null",
        ] {
            assert!(matches!(guard_close(&reviewing, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// `>& 파일` 은 stdout·stderr 를 그 파일에 쓴다. fd 를 잇는 `>&2` 와 가른다.
    #[test]
    fn a_dup_redirection_to_a_file_is_a_write() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in ["echo x >& src/store.rs", "echo x >&src/store.rs", "echo x &> src/store.rs", "echo x &>> src/store.rs"] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        for cmd in ["cargo build >&2", "cargo test 2>&1 | tail -5", "exec 3>&-"] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
    }

    /// **산술과 `[[ ]]` 안의 `>` 는 비교다** — 안에서 `;`·`&&`·`||` 가 나와도,
    /// `$(( ))` 로 불러도. 토막마다 `((` 를 찾던 판은 `for ((i=10; i>0; i--))` 를
    /// `0` 에 쓰는 것으로 읽었다.
    #[test]
    fn a_comparison_is_never_a_write() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "for ((i=10; i>0; i--)); do echo $i; done",
            "(( a > 0 && b > 0 )) && echo yes",
            "if (( count > 0 || errors > 0 )); then echo bad; fi",
            "echo $(( 5 > 3 ))",
            "x=$(( a > b ? a : b ))",
            "[[ $a == x || $b > y ]] && echo yes",
            "[[ ( a > b ) ]]",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            "[[ -f x ]] && echo x > src/store.rs",
            "[[ -f x ]]&& echo x > src/store.rs",
            "(( n > 3 )) && echo x > src/store.rs",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **따옴표 안의 명령 치환은 따옴표를 새로 연다.** `"$(echo "it's")"` 의 안쪽
    /// `"` 를 바깥을 닫는 것으로 읽던 판은, 그 뒤집힘을 줄 너머까지 끌고 가
    /// 뒤 줄의 커밋 본문을 쓰기로 막고 뒤 줄의 `moai mv` 는 못 봤다.
    #[test]
    fn a_nested_quote_does_not_flip_the_rest() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "echo \"$(grep -rn \"x > y \" src | wc -l)\"",
            "git commit -m $'fix: don\\'t break\\n\\n- draft -> accepted'",
            "gh pr comment 1 --body \"$(echo \"it's merged\")\"\ngit commit -m \"note: we don't\n- draft -> accepted\"",
            "git commit -m \"$(cat <<'EOF'\nit's (a) fix -> done\nEOF\n)\"",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }

        let reviewing = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in [
            "gh pr comment 1 --body \"$(echo \"it's merged\")\"\nmoai mv t-r done",
            "git commit -m \"$(cat <<'EOF'\nit's done\nEOF\n)\" && moai mv t-r done",
        ] {
            assert!(matches!(guard_close(&reviewing, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let cmd = "git commit -m $'don\\'t'\nmoai add '딴 일'";
        assert!(matches!(guard_create(&held, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
    }

    /// **heredoc 은 셸이 끝내는 자리에서 끝난다.** 종료어는 그 줄 전체여야 하고,
    /// 한 줄에 둘이면 둘 다 건너뛰며, 따옴표·주석·산술 속 `<<` 는 heredoc 이
    /// 아니다. 이 저장소가 가르치던 글에 들여 쓴 `    MD` 가 있어, 그 글을 heredoc
    /// 으로 옮기는 것만으로 막히던 자리다. 지금 가르치는 글은 왼쪽 끝에서 닫고 종료어를
    /// `MD`·`EOF` 와 가른다 — 같은 종료어면 셸도 이 파서도 바깥 heredoc 을 그 줄에서 끝낸다.
    #[test]
    fn a_heredoc_ends_where_the_shell_ends_it() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "cat > /tmp/x <<EOF\n  EOF\n- a -> b\nEOF",
            "cat > /tmp/g.md <<'MD'\n예시:\n    moai add --from - <<'MD'\n    # 에픽\n    MD\n집기 -> review 로 옮긴다\nMD",
            "grep -c \"<<\" src/hook.rs; cat <<'EOF' > /tmp/out\na -> b\nEOF",
            "grep -q x <<< \"$v\" && cat <<'EOF' > /tmp/out\na -> b\nEOF",
            "cat <<A; cat <<B\na\nA\nx -> y\nB",
            "cat <<-EOF > /tmp/x\n\tbody -> b\n\tEOF",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            "python3 -c \"print(1<<3)\"\nsed -i s/a/b/ src/store.rs",
            "echo $((1 << 2))\necho x > src/store.rs",
            "# cat <<EOF 로 쓴다\necho x > src/store.rs",
            "cat <<\\EOF > /tmp/x\nbody\nEOF\necho x > src/store.rs",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let swallowed = "echo \"a <<EOF b\"\nmoai add '딴 일'";
        assert!(matches!(guard_create(&held, &cfg(), &here(), swallowed), Decision::Deny(_)), "샜다 — {swallowed}");
        let quoted = "moai note t-1 -b - <<'MD'\n  MD\nmoai add '제목' 이라고 적는다\nMD";
        assert_eq!(guard_create(&held, &cfg(), &here(), quoted), Decision::Pass, "막혔다 — {quoted}");
    }

    /// **명령 자리는 묶음과 예약어 뒤에서도 선다.** `{ cd /tmp; … }` 의 `cd` 를
    /// 못 보면 뒤의 상대 경로를 저장소에 풀어 아무것도 안 쓰는 명령을 막고,
    /// `(moai add …)` 의 `moai` 를 못 보면 규칙 1 이 샌다.
    #[test]
    fn the_command_word_stands_after_grouping_and_keywords() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "{ cd /tmp; echo x > notes.md; }",
            "( cd /tmp && echo x > notes.md )",
            "(cd /tmp && echo x > notes.md)",
            "if cd /tmp; then echo x > notes.md; fi",
            "builtin cd /tmp && echo x > notes.md",
            "time cd /tmp && echo x > notes.md",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        let sub = "(echo x > src/store.rs)";
        assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, sub), Decision::Deny(_)), "샜다 — {sub}");

        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in ["(moai add '딴 일')", "{ moai add '딴 일'; }", "if true; then moai add '딴 일'; fi"] {
            assert!(matches!(guard_create(&held, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        let reviewing = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in ["time moai mv t-r done", "(moai mv t-r done)", "if true; then moai mv t-r done; fi"] {
            assert!(matches!(guard_close(&reviewing, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **플래그는 어디에 두어도 플래그다.** 첫 플래그에서 동사 읽기를 멈추던 판은
    /// `moai --json mv t-r done` 을 동사 없는 명령으로 읽어 규칙 1 과 리뷰 닫기를
    /// 넘겼고, 붙여 쓴 값(`-m"반영"`)은 못 읽어 옳게 친 명령을 막았다.
    #[test]
    fn a_flag_anywhere_is_still_a_flag() {
        let reviewing = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in [
            "moai --json mv t-r done",
            "moai -C /repo mv t-r done",
            "moai --user \"나 (me@x.com)\" mv t-r done",
            "moai mv --json t-r done",
            "moai mv t-r --json done",
            "moai mv t-r -m '' done",
        ] {
            assert!(matches!(guard_close(&reviewing, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        for ok in ["moai mv t-r done -m\"반영\"", "moai mv -m \"반영\" t-r done", "moai --json mv t-r done --msg=반영"] {
            assert_eq!(guard_close(&reviewing, &cfg(), &here(), ok), Decision::Pass, "막혔다 — {ok}");
        }

        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        for cmd in ["moai --json add \"딴 일\"", "moai -C . add \"딴 일\""] {
            assert!(matches!(guard_create(&held, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        assert_eq!(guard_create(&held, &cfg(), &here(), "moai add '안의 일' -et-e"), Decision::Pass);
    }

    /// **하나를 집는 명령 뒤의 쓰기는 집은 채로 쓰는 것이다.** 훅은 명령이 돌기
    /// 전의 상태를 보므로, 규칙이 시킨 차례를 한 줄로 친 명령을 막을 뻔했다.
    #[test]
    fn a_write_after_picking_one_up_is_held() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "moai mv t-1 in_progress && echo x > src/store.rs",
            "moai mv t-1 review && sed -i s/a/b/ src/store.rs",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            "moai mv t-1 done && echo x > src/store.rs",
            "moai mv t-1 todo && echo x > src/store.rs",
            "echo x > src/store.rs && moai mv t-1 in_progress",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **집기가 져도 도는 쓰기는 집은 채로 쓰는 것이 아니다** (moai-gbqb). 첫 토막에서
    /// 멈추던 판은 `--from` 으로 겨루다 진 집기 뒤의 `; sed -i …` 를 안 세어, 아무것도 안
    /// 쥔 채 저장소가 고쳐졌다. `&&` 로만 이어 온 동안이 집기 뒤다.
    #[test]
    fn only_an_and_chain_carries_the_pick_up() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo\nsed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress | tee src/store.rs",
            "moai mv t-1 in_progress & echo x > src/store.rs",
            "moai mv t-1 in_progress && echo ok || echo x > src/store.rs",
            "moai mv t-1 in_progress && echo ok; echo x > src/store.rs",
            "(moai mv t-1 in_progress); echo x > src/store.rs",
            "(moai mv t-1 in_progress && echo ok) || echo x > src/store.rs",
            "moai mv t-1 in_progress && (echo ok); (echo x > src/store.rs)",
            "(moai mv t-1 in_progress; echo x > src/store.rs)",
            "moai mv t-1 in_progress && (echo ok)\necho x > src/store.rs",
            "moai mv t-1 in_progress && { echo ok; }; echo x > src/store.rs",
            "moai mv t-1 in_progress; { echo x > src/store.rs; }",
            // `{ … }` 안의 집기도 그 안의 `;` 로 끊긴다 — 집기의 깊이를 `( … )` 로만 재던 판이 샜다.
            "{ moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs; }",
            "{ moai mv t-1 in_progress\necho x > src/store.rs\n}",
            "moai mv t-1 in_progress && { { echo ok; } }; echo x > src/store.rs",
            // 이음사나 `(` 뒤의 줄바꿈은 앞의 `;`·`||` 를 괄호 안 깊이로 덮지 않는다.
            "moai mv t-1 in_progress --from todo; (\nsed -i s/a/b/ src/store.rs\n)",
            "moai mv t-1 in_progress\n(\necho x > src/store.rs\n)",
            "moai mv t-1 in_progress || (\necho x > src/store.rs\n)",
            // 집기가 든 파이프라인은 집기의 성패와 무관하게 돈다.
            "(moai mv t-1 in_progress && echo ok) | tee src/store.rs",
            "moai mv t-1 in_progress |\n tee src/store.rs",
            "moai mv t-1 in_progress && echo ok | tee /tmp/x; echo x > src/store.rs",
            // 집기가 져야 도는 것, 아무것도 안 옮기는 것.
            "! moai mv t-1 in_progress && echo x > src/store.rs",
            "moai mv t-1 in_progress --help && echo x > src/store.rs",
            // 묶음의 리다이렉션은 묶음이 돌기 전에 연다 — 안의 집기와 무관하다.
            "(moai mv t-1 in_progress) > src/cli.rs && echo x > /tmp/y",
            "(moai mv t-1 in_progress; echo ok) > /dev/null && echo x > src/store.rs",
            // 예약어 묶음을 세도 **집기와 무관하게 도는 몸통은 그대로 막는다**(moai-1jvy).
            "if true; then echo x > src/store.rs; fi",
            "moai mv t-1 in_progress; if true; then echo x > src/store.rs; fi",
            "moai mv t-1 in_progress && if true; then echo ok; fi; echo x > src/store.rs",
            // 낱말로 선 `if`·`fi` 는 묶음이 아니다 — 깊이를 늘리면 뒤의 `;` 가 집기 뒤로 읽힌다.
            "moai mv t-1 in_progress && echo if; echo x > src/store.rs",
            "moai mv t-1 in_progress && echo done; echo x > src/store.rs",
            // **괄호 밖의 `!` 도 집기의 부정이다**(moai-gtkn) — 집기가 져야 뒤가 도니 빈손이다.
            "! (moai mv t-1 in_progress) && echo x > src/store.rs",
            "! (moai mv t-1 in_progress && echo ok) && sed -i s/a/b/ src/store.rs",
            // `set +e` 는 `set -e` 를 되돈다 — 그 뒤의 `;` 는 다시 집기를 끊는다.
            "set -e; set +e; moai mv t-1 in_progress; echo x > src/store.rs",
            "set -e; moai mv t-1 in_progress; set +o errexit; echo x > src/store.rs",
            // `|| exit` 가 끊는 것은 바로 그 줄뿐이다 — 그 뒤의 진 집기는 여전히 빈손이다.
            "echo hi || exit 1; echo x > src/store.rs",
            "git pull || exit 1; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs",
            // 하위 셸의 `exit`·`set -e` 는 괄호만 끝낸다 — 괄호 밖의 `;` 는 집기가 져도 돈다.
            "(moai mv t-1 in_progress --from todo || exit 1); sed -i s/a/b/ src/store.rs",
            "(set -e; moai mv t-1 in_progress); echo x > src/store.rs",
            "moai mv t-1 in_progress --from todo || echo lost; sed -i s/a/b/ src/store.rs",
            // **형제 괄호·다시 든 `if` 는 딴 묶음이다**(리뷰 moai-ju21.70g) — 깊이가 같아도 앞 묶음의
            // `set -e`·`|| exit`·`cd` 를 물려받지 않는다.
            "(set -e; moai mv t-1 in_progress); (sed -i s/a/b/ src/store.rs)",
            "(moai mv t-1 in_progress || exit 1); (sed -i s/a/b/ src/store.rs)",
            "if false; then moai mv t-1 in_progress || exit 1; fi; sed -i s/a/b/ src/store.rs",
            "(cd /tmp); (echo x > src/store.rs)",
            "echo $(cd /tmp) $(echo x > src/store.rs)",
            // **`set -e` 는 져도 안 끝나는 자리가 있다** — `&&` 목록의 앞 칸, `if` 의 조건, `&`, 파이프의
            // 칸. 그 뒤의 `;` 는 집기가 져도 돈다.
            "set -e; moai mv t-1 in_progress --from todo && echo ok; sed -i s/a/b/ src/store.rs",
            "set -e; if moai mv t-1 in_progress; then :; fi; sed -i s/a/b/ src/store.rs",
            "set -e; if moai mv t-1 in_progress; then :; else sed -i s/a/b/ src/store.rs; fi",
            "set -e; moai mv t-1 in_progress & sed -i s/a/b/ src/store.rs",
            "set -e | true; moai mv t-1 in_progress; sed -i s/a/b/ src/store.rs",
            // `--` 뒤는 자리 인자다 — errexit 가 아니다.
            "set -- -e; moai mv t-1 in_progress; sed -i s/a/b/ src/store.rs",
            // `||` 뒤의 집기는 앞이 이기면 안 돈다.
            "true || moai mv t-1 in_progress && sed -i s/a/b/ src/store.rs",
            // 제 셸을 못 끝내는 `|| exit` — 함수 밖의 `return`, `&` 로 띄운 것, 파이프의 칸.
            "moai mv t-1 in_progress --from todo || return 1; echo x > src/store.rs",
            "moai mv t-1 in_progress --from todo || exit 1 & sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || exit 1 | cat; sed -i s/a/b/ src/store.rs",
            // 묶음을 닫는 `)` 는 접두어만 선 토막이어도 이음사를 덮는다 — 다음 줄은 `&&` 뒤가 아니다.
            "moai mv t-1 in_progress --from todo && (FOO=1)\nsed -i s/a/b/ src/store.rs",
            // 낱말 없는 토막의 치환과 산술 안의 치환도 명령이다.
            "while read l; do :; done < <(sed -i s/a/b/ src/store.rs)",
            "echo $(( $(echo x > src/store.rs) ))",
            // **`fi`·`esac`·`done` 을 지나면 몸통 안의 집기는 끝난다** — 몸통이 안 돌아도 그 묶음은 0 이다.
            "if [ -f /nonexistent ]; then moai mv t-1 in_progress --from todo; fi && sed -i s/a/b/ src/store.rs",
            "case x in y) moai mv t-1 in_progress;; esac && sed -i s/a/b/ src/store.rs",
            "while false; do moai mv t-1 in_progress; done && sed -i s/a/b/ src/store.rs",
            "{ if false; then moai mv t-1 in_progress; fi; } && sed -i s/a/b/ src/store.rs",
            "set -e; if [ -f /nonexistent ]; then moai mv t-1 in_progress; fi; sed -i s/a/b/ src/store.rs",
            // `! { … }` 도 묶음 전체의 부정이고, 묶음 안의 `||` 뒤 집기는 앞이 이기면 안 돈다.
            "! { true; moai mv t-1 in_progress --from todo; } && sed -i s/a/b/ src/store.rs",
            "{ true || moai mv t-1 in_progress; } && sed -i s/a/b/ src/store.rs",
            // `||` 로 들어간 묶음은 앞이 이기면 통째로 안 돈다 — 그 안의 집기는 묶음 밖으로 안 이어진다.
            "true || { moai mv t-1 in_progress --from todo; } && sed -i s/a/b/ src/store.rs",
            "make || (moai mv t-1 in_progress --from todo) && echo x > src/store.rs",
            // 묶음을 닫은 뒤 다음 명령의 치환이 먼저 쌓여도 닫힌 것은 닫힌 것이다.
            "if [ -f /nope ]; then moai mv t-1 in_progress --from todo; fi && echo \"$(date)\" > src/store.rs",
            "while false; do moai mv t-1 in_progress; done && echo `date` > src/store.rs",
            // 목록의 앞 칸인 묶음 안에서는 bash 가 errexit 를 안 본다.
            "set -e; { moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs; } || true",
            "set -e\n(\n  moai mv t-1 in_progress --from todo\n  sed -i s/a/b/ src/store.rs\n) || echo failed",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        for cmd in [
            "moai mv t-1 in_progress --from todo && echo ok && sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress &&\n  sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress && (cd src; echo x > /repo/src/store.rs; echo y > /repo/src/cli.rs)",
            "(moai mv t-1 in_progress) && echo x > src/store.rs",
            "(moai mv t-1 in_progress && echo ok) && echo x > src/store.rs",
            "moai mv t-1 in_progress && { echo ok; echo x > src/store.rs; }",
            // 진 줄 뒤에 다시 집으면 그 뒤는 또 집기 뒤다.
            "moai mv t-1 in_progress; moai mv t-1 in_progress && echo x > src/store.rs",
            // 저장소 밖의 쓰기는 이음사와 무관하게 지난다 — 규칙 2 가 세는 것은 저장소 안뿐이다.
            "moai mv t-1 in_progress; echo x > /tmp/notes",
            // `{ mv; }` 의 값은 집기의 값이다 — 닫는 `}` 가 안쪽 `;` 를 들고 서면 안 된다.
            "{ moai mv t-1 in_progress; } && echo x > src/store.rs",
            // 파이프는 `&&` 보다 단단히 묶인다 — `&&` 뒤의 파이프라인은 통째로 집기 뒤다.
            "moai mv t-1 in_progress && echo x | tee src/store.rs",
            "moai mv t-1 in_progress && cargo test 2>&1 | tee out.log",
            "moai mv t-1 in_progress && grep a b | sort > out.txt",
            "moai mv t-1 in_progress && echo x |\n  tee src/store.rs",
            "moai mv t-1 in_progress && (echo ok | tee src/store.rs)",
            // 파이프라인의 값은 끝 칸의 것이다.
            "echo ok | moai mv t-1 in_progress && echo x > src/store.rs",
            "moai mv t-1 in_progress && {\necho x > src/store.rs\n}",
            "moai mv t-1 in_progress && (\necho x > src/store.rs\n)",
            // 묶음 뒤의 리다이렉션은 묶음의 것이다 — 이음사로 읽어 집기를 끊지 않는다.
            "(moai mv t-1 in_progress) > /dev/null && echo x > src/store.rs",
            "{ moai mv t-1 in_progress; } > /dev/null && echo x > src/store.rs",
            "moai mv t-1 in_progress && (echo ok) > src/store.rs",
            // **예약어 묶음의 몸통도 집기 뒤다**(moai-1jvy) — `{ … }` 와 같은 자다.
            "moai mv t-1 in_progress && if true; then sed -i s/a/b/ src/store.rs; fi",
            "moai mv t-1 in_progress && if true; then echo x > src/store.rs; else echo y > src/cli.rs; fi",
            "moai mv t-1 in_progress && while read f; do sed -i s/a/b/ src/store.rs; done",
            "moai mv t-1 in_progress && until false; do echo x > src/store.rs; done",
            "moai mv t-1 in_progress && for f in a b; do echo x > src/store.rs; done",
            "moai mv t-1 in_progress && case x in a) echo x > src/store.rs;; esac",
            "moai mv t-1 in_progress && if true; then { echo x > src/store.rs; }; fi",
            "moai mv t-1 in_progress && if true; then\nsed -i s/a/b/ src/store.rs\nfi",
            // 묶음을 닫은 뒤의 `&&` 는 다시 바깥 깊이다 — 닫는 낱말이 깊이를 안 돌려주면 여기가 샌다.
            "moai mv t-1 in_progress && if true; then echo ok; fi && echo x > src/store.rs",
            // **집기가 이긴 것이 확실하면 `;` 뒤도 집기 뒤다**(moai-gtkn) — 집기 목록 뒤의 `|| exit`,
            // `set -e` 아래 홀로 선 집기.
            "moai mv t-1 in_progress --from todo || exit 1; sed -i s/a/b/ src/store.rs",
            // 맨 바깥에서는 `exit 0` 도 껍데기를 끝낸다 — 겹 안에서와 다르다(리뷰 moai-k8j1.209).
            "moai mv t-1 in_progress --from todo || { echo fail; exit 0; }; sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || exit; echo x > src/store.rs",
            "moai mv t-1 in_progress --from todo || exit 1; echo ok; sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || exit 1\nsed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || exit 1; x=1 || true; sed -i s/a/b/ src/store.rs",
            // 앞도 집기면 `||` 뒤의 집기는 어느 쪽이든 하나를 쥔다.
            "moai mv t-1 in_progress --from todo || moai mv t-2 in_progress --from todo && sed -i s/a/b/ src/store.rs",
            // `||` 로 들어간 묶음 안에서는 집기 뒤다.
            "true || { moai mv t-1 in_progress --from todo && sed -i s/a/b/ src/store.rs; }",
            // `set -e` 아래 홀로 선 집기를 지났으면 뒤의 파이프·목록이 사슬을 끊어도 집기는 그대로다.
            "set -euo pipefail\nmoai mv t-1 in_progress --from todo\ncargo build 2>&1 | tail -3\nsed -i s/a/b/ src/store.rs",
            "set -e; moai mv t-1 in_progress --from todo; ls | head -1; sed -i s/a/b/ src/store.rs",
            "set -e\nmoai mv t-1 in_progress --from todo\nfor f in a b; do echo $f; done\necho x > src/store.rs",
            "set -e; moai mv t-1 in_progress; sed -i s/a/b/ src/store.rs",
            "set -euo pipefail; moai mv t-1 in_progress; echo x > src/store.rs",
            "set -o errexit; moai mv t-1 in_progress; echo x > src/store.rs",
            "set -e; moai mv t-1 in_progress; echo ok; sed -i s/a/b/ src/store.rs",
            // 치환 토막은 바깥 토막의 일부다 — 바깥 토막의 앞 명령을 가리지 않는다.
            "set -e; moai mv t-1 in_progress; echo \"$(date)\" > src/store.rs",
            // 형제 괄호의 `!` 는 제 괄호만 부정한다.
            "! (true); (moai mv t-1 in_progress) && sed -i s/a/b/ src/store.rs",
            // 접두어 뒤의 괄호는 이음사를 안 덮는다 — 괄호 안은 여전히 집기 뒤다.
            "moai mv t-1 in_progress && time (sed -i s/a/b/ src/store.rs)",
            "moai mv t-1 in_progress && ! (echo x > src/store.rs)",
            "moai mv t-1 in_progress && if (echo x > src/store.rs); then echo ok; fi",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
    }

    /// **남의 트래커에서 집은 것은 여기서 아무것도 안 쥐어 준다.** 집기를 어디서 도는지 안
    /// 보던 판은 `moai -C <남> mv … in_progress && sed -i …` 로 빈손의 쓰기를 넘겼다.
    #[test]
    fn a_pick_up_in_another_tracker_holds_nothing_here() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let judge = |cmd: &str| {
            let dirs = aimed(cmd, root);
            let own = |k: usize| dirs[k].is_none();
            guard_shell_in(&idle, &cfg(), &here(), root, root, cmd, &Segs { judges: &own, picks: &own }, &|k| dirs[k].as_deref())
        };
        for cmd in [
            "moai -C /b mv t-1 in_progress && echo x > src/store.rs",
            "cd /b && moai mv t-1 in_progress && echo x > /repo/src/store.rs",
        ] {
            assert!(matches!(judge(cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        assert_eq!(judge("moai -C . mv t-1 in_progress && echo x > src/store.rs"), Decision::Pass);
        assert_eq!(judge("cat a > /tmp/x && moai mv t-1 in_progress && echo x > src/store.rs"), Decision::Pass);
    }

    /// **셸에 넘긴 글의 값은 바깥 토막의 값이다**(moai-plfi, moai-tmi0) — `bash -c '…'`·`eval '…'` 은 그 글의
    /// 마지막 명령의 값으로 끝난다. 치환(`$( … )`)과 한 표식으로 들던 판은 그 안의 집기를 나올 때 버려,
    /// 시킨 대로 친 `bash -c '집기' && 쓰기` 를 막았다. `bash -c` 는 **새 셸**이라 `set -e` 를 제 글
    /// 안에서 따로 세고, 바깥의 것을 물려받지 않는다. `eval` 은 지금 셸에서 돌아 `cd` 가 바깥에 남는다.
    #[test]
    fn a_string_handed_to_a_shell_ends_with_its_last_command() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "bash -c 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "eval 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "sh -c \"moai mv t-1 in_progress --from todo && echo ok\" && echo x > src/store.rs",
            "echo go | bash -c 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo && eval 'echo ok; sed -i s/a/b/ src/store.rs'",
            "bash -c 'moai mv t-1 in_progress --from todo' > /dev/null && sed -i s/a/b/ src/store.rs",
            // 새 셸의 `set -e` 는 그 글 안에서 선다 — 맨 바깥 깊이에만 매던 판이 막았다(moai-tmi0).
            "bash -c 'set -euo pipefail; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -c 'set -e\nmoai mv t-1 in_progress --from todo\nsed -i s/a/b/ src/store.rs'",
            "bash -c 'set -e; moai mv t-1 in_progress --from todo; echo ok' && sed -i s/a/b/ src/store.rs",
            // 바깥의 `||` 는 새 셸의 errexit 를 못 끈다 — 딴 프로세스다.
            "bash -c 'set -e; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs' || true",
            // 바깥 `set -e` 아래 홀로 선 `bash -c`·`eval` 은 그 글의 값으로 껍데기를 끝낸다.
            "set -e; bash -c 'moai mv t-1 in_progress --from todo'; sed -i s/a/b/ src/store.rs",
            "set -e; eval 'moai mv t-1 in_progress --from todo'; sed -i s/a/b/ src/store.rs",
            // `eval` 의 `cd` 는 바깥에 남는다 — 그 뒤의 상대 경로는 저장소 밖이다.
            "eval 'cd /tmp'; echo x > src/store.rs",
            // 글 안의 `|| exit` 는 그 셸을 끝낸다. 겹친 셸과 이어 부른 셸도 같다.
            "bash -c 'moai mv t-1 in_progress --from todo || exit 1; sed -i s/a/b/ src/store.rs'",
            "bash -c \"bash -c 'moai mv t-1 in_progress --from todo'\" && sed -i s/a/b/ src/store.rs",
            "bash -c 'set -e; bash -c \"moai mv t-1 in_progress --from todo\"; sed -i s/a/b/ src/store.rs'",
            // **안 풀린 채 끝난 묶음은 그 셸이 무엇으로 끝났는가로 푼다**(moai-4arw) — 그 묶음의
            // 모든 길이 `exit <0 아닌 값>` 이면 자식 셸은 집기가 져야 0 이 아니니, 바깥의 `&&` 에
            // 닿은 것은 집기가 이겼다는 뜻이다. 전제 넷(0 아닌 값·정말 돈 묶음·하위 셸이 아닐 것·
            // 뒤로 안 띄운 글)이 다 서야 푼다 — 하나씩 어긋난 꼴은 막는 쪽 목록에 있다.
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 1; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi' && sed -i s/a/b/ src/store.rs",
            "sh -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 1; }' && echo x > src/store.rs",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then echo fail; exit 1; fi' && sed -i s/a/b/ src/store.rs",
            // **뒤로 띄운 것으로 끝나도 띄울 때 켠 errexit 는 그대로다**(moai-54pk) — 글의 값만 0 이지
            // 그 셸이 errexit 아래 돈 것은 바뀌지 않는다.
            "bash -e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs; echo done &'",
            "bash -euo pipefail -c 'moai mv t-1 in_progress --from todo; echo x > src/store.rs; sleep 1 &'",
            "bash -c 'moai mv t-1 in_progress --from todo' && bash -c 'sed -i s/a/b/ src/store.rs'",
            // **한 셸에 매인 것은 겹 경계를 넘어도 그 셸의 것이다**(moai-9xbq) — `|| exit` 로 이긴
            // 집기(`sure`)와 집기가 지면 끝내는 묶음(`bailout`)이다. 겹마다 넷만 들던 판은 이 셋을
            // 잘못 막았다. 첫 줄은 이 에픽이 낸 되돌림이다.
            "moai mv t-1 in_progress --from todo || { bash -c 'echo fail'; exit 1; }; sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo || exit 1; echo done' && sed -i s/a/b/ src/store.rs",
            "if ! bash -c 'moai mv t-1 in_progress --from todo'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            // 같은 꼴을 eval 과 sh 로, 그리고 하위 셸로 연 묶음으로.
            "moai mv t-1 in_progress --from todo || { eval 'echo fail'; exit 1; }; sed -i s/a/b/ src/store.rs",
            "sh -c 'moai mv t-1 in_progress --from todo || exit 1; echo done' && sed -i s/a/b/ src/store.rs",
            // **띄울 때 켠 errexit 도 errexit 다**(moai-j9tx) — 그 글은 첫 줄부터 `set -e` 아래다.
            // 옵션 뭉치에서 `-e` 를 보고도 그냥 넘기던 판은 에이전트가 흔히 쓰는 이 꼴을 막았다.
            "bash -euo pipefail -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -o errexit -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "sh -ec 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -eu -c 'moai mv t-1 in_progress --from todo; echo ok; sed -i s/a/b/ src/store.rs'",
            // 스크립트를 돌리는 줄의 `-c` 는 그 스크립트의 인자다 — 그 글은 안 돌아 쓰기도 없다.
            "bash -e script.sh -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // 바깥 `set -e` 아래 이긴 집기는 그 뒤의 겹을 지나도 이긴 채다 — 겹을 열며 선 셈을 나올 때
            // 되돌리던 판은 이 줄을 막았다.
            "set -e; moai mv t-1 in_progress --from todo; bash -c 'echo ok' | cat; sed -i s/a/b/ src/store.rs",
            "set -e; moai mv t-1 in_progress --from todo; bash -c 'echo ok' & sed -i s/a/b/ src/store.rs",
            // 파이프의 마지막 칸은 그 파이프의 값이다 — 뒤로 띄운 것과 가른다.
            "bash -c 'cat /dev/null | moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            // 뒤로 띄운 것으로 끝나는 글은 제 집기를 안 넘길 뿐, 바깥에서 이긴 집기는 그대로 잇는다.
            "moai mv t-1 in_progress --from todo && bash -c 'echo ok &' && sed -i s/a/b/ src/store.rs",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            // 띄울 때 끈 것은 안 켠 것이다 — `+e` 와 `-o` 가 아닌 `+o errexit` 도 같다.
            "bash -e +e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash +o errexit -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // eval 은 새 셸이 아니라 띄울 플래그가 없다 — 바깥의 errexit 를 물려받지도 않는다.
            "set -e; eval 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // 글의 값은 **마지막** 명령의 값이다.
            "bash -c 'moai mv t-1 in_progress --from todo; echo ok' && sed -i s/a/b/ src/store.rs",
            // **끝내는 묶음이 아니면 안 푼다**(moai-54pk) — 그 묶음이 돌고도 셸이 0 으로 끝나면
            // 집기가 졌어도 바깥의 `&&` 에 닿는다.
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo || ( echo fail; exit 1 )' && sed -i s/a/b/ src/store.rs",
            // **전제 넷이 하나씩 어긋난 꼴**(moai-4arw) — 셋은 실제로 0 으로 끝나고(bash 로 쟀다),
            // 넷째는 하위 셸이 끝나도 뒤가 더 도는 글이다.
            // - 뒤로 띄운 묶음: `exit` 가 그 하위 셸만 끝내고 글은 곧바로 0 이다
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 1; } &' && sed -i s/a/b/ src/store.rs",
            // - 안 돈 몸통 안에서 열린 묶음: 그 `exit` 는 돌지 않는다
            "bash -c 'if false; then moai mv t-1 in_progress --from todo || { exit 1; }; fi' && sed -i s/a/b/ src/store.rs",
            // - 파이프의 칸: 값은 마지막 칸의 것이라 `exit` 가 안 닿는다
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 1; } | cat' && sed -i s/a/b/ src/store.rs",
            // **조건에 매여 들어간 묶음은 아예 안 돌 수 있다**(리뷰 moai-k8j1.udq 의 2·4번) —
            // `a && { … }` 의 묶음도 함수 몸통도 그렇다. 안 돌면 집기도 `exit` 도 안 돌고 자식 셸은
            // 0 으로 끝난다. bash 로 쟀다.
            "bash -c 'test -f Cargo.toml && { moai mv t-1 in_progress --from todo || { exit 1; }; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'false && { moai mv t-1 in_progress --from todo || { exit 1; }; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'f() { moai mv t-1 in_progress --from todo || { exit 1; }; }' && sed -i s/a/b/ src/store.rs",
            // **몸통이 겹문인 `then` 가지도 `exit` 가 아니다**(리뷰 3번) — 제 자리에 `exit` 토막을
            // 안 내니, `if` 꼴이 들고 시작한 참이 그대로 남던 판은 이 넷을 통째로 넘겼다.
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then { echo no; }; fi' && sed -i s/a/b/ src/store.rs",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then while :; do break; done; fi' && sed -i s/a/b/ src/store.rs",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then if true; then echo n; fi; fi' && sed -i s/a/b/ src/store.rs",
            "if ! moai mv t-1 in_progress --from todo; then { echo no; }; fi; sed -i s/a/b/ src/store.rs",
            "if ! moai mv t-1 in_progress --from todo; then { exit 1; }; fi & sed -i s/a/b/ src/store.rs",
            // **묶음에 건 리다이렉션은 묶음 밖에 제 토막으로 선다**(리뷰 1번) — 그것이 글의 마지막
            // 토막이라, 거기에 `bg` 를 안 달면 뒤로 띄운 글이 안 띄운 것으로 읽힌다.
            "bash -c '{ moai mv t-1 in_progress --from todo; } >/dev/null &' && sed -i s/a/b/ src/store.rs",
            "bash -c '( moai mv t-1 in_progress --from todo ) >/dev/null &' && sed -i s/a/b/ src/store.rs",
            // - 하위 셸 안의 묶음: 괄호만 끝나고 그 뒤가 더 돈다. 겹의 깊이를 **겹에 들어선 토막**
            //   에서 베껴 오던 판은 이 글이 `( … )` 로 시작한다는 이유로 그 자를 부풀려 놓쳤다
            //   (리뷰 moai-k8j1.209 의 6번)
            "bash -c '( moai mv t-1 in_progress --from todo || { exit 1; } ); echo done' && sed -i s/a/b/ src/store.rs",
            // **하위 셸이 글의 마지막이면 bash 는 그 값을 그대로 낸다** — 이 셋은 실제로 집기가 져야
            // 0 이 아니다. 그래도 막는다(moai-4arw): 겹이 들고 있는 깊이로 재는 한 `집기 || ( …;
            // exit 1 )`(위)과 이 꼴이 한 집안인데, 둘을 가르려면 "그 묶음이 글의 마지막인가" 라는
            // 자를 새로 들여야 한다. 지금은 집안째 막는 쪽으로 둔다 — 넓히는 것은 idea 로 담았다.
            "bash -c '( moai mv t-1 in_progress --from todo || { exit 1; } )' && sed -i s/a/b/ src/store.rs",
            "bash -c \"bash -c '( moai mv t-1 in_progress --from todo || { exit 1; } )'\" && sed -i s/a/b/ src/store.rs",
            "bash -c '( if ! moai mv t-1 in_progress --from todo; then exit 1; fi )' && sed -i s/a/b/ src/store.rs",
            // 뒤로 띄운 것으로 끝나면 글의 값은 집기의 값이 아니다 — errexit 를 켜고 띄워도 같다.
            "bash -e -c 'moai mv t-1 in_progress --from todo &' && sed -i s/a/b/ src/store.rs",
            "eval 'moai mv t-1 in_progress --from todo; echo ok' && sed -i s/a/b/ src/store.rs",
            // 부정·`||` 뒤·`;` 뒤·`&` 로 띄운 것.
            "! bash -c 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "! eval 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "true || bash -c 'moai mv t-1 in_progress --from todo' && sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo'; sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo' & sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo' | cat && sed -i s/a/b/ src/store.rs",
            // 새 셸은 바깥의 `set -e` 를 안 물려받고, 그 안의 `set -e`·`exit` 는 그 셸만 끝낸다.
            "set -e; bash -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -c 'set -e; moai mv t-1 in_progress --from todo'; sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo || exit 1'; sed -i s/a/b/ src/store.rs",
            "bash -c 'set -e'; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs",
            "bash -c \"bash -c 'moai mv t-1 in_progress --from todo'; echo ok\" && sed -i s/a/b/ src/store.rs",
            "set -e; bash -c 'moai mv t-1 in_progress --from todo' | tee /tmp/log; sed -i s/a/b/ src/store.rs",
            // 앞 괄호의 `cd` 는 뒤 괄호 안의 셸 글로 안 이어진다 — 상대 경로는 저장소 안이다.
            "(cd /tmp); (bash -c 'echo x > src/store.rs')",
            // 치환의 값은 여전히 바깥 명령의 값이 아니고, 그 안의 `set -e` 는 바깥 목록이 끌 수 있다.
            "echo \"$(moai mv t-1 in_progress --from todo)\" && sed -i s/a/b/ src/store.rs",
            "echo \"$(set -e; moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs)\"",
            "set -e; echo \"$(moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs)\"",
            "bash -c \"echo \\\"\\$(moai mv t-1 in_progress --from todo)\\\"\" && sed -i s/a/b/ src/store.rs",
            // **몸통이 안 돌았을 수 있는 묶음이 글 끝에서 닫힌다** — 그 표식을 받을 토막이 글 안에 없어
            // 버리던 판은, 안 돈 몸통의 집기를 바깥 `&&` 로 이어 아무것도 안 집은 쓰기를 넘겼다.
            "bash -c 'if false; then moai mv t-1 in_progress --from todo; fi' && sed -i s/a/b/ src/store.rs",
            "sh -c 'case y in x) moai mv t-1 in_progress --from todo;; esac' && sed -i s/a/b/ src/store.rs",
            "eval 'while false; do moai mv t-1 in_progress --from todo; done' && sed -i s/a/b/ src/store.rs",
            "bash -c \"bash -c 'if false; then moai mv t-1 in_progress --from todo; fi'\" && sed -i s/a/b/ src/store.rs",
            // **0 으로 끝내는 묶음은 안 푼다**(리뷰 moai-k8j1.209) — 자식 셸이 0 으로 끝나면 집기가
            // 졌어도 바깥 `&&` 에 닿는다. 맨 바깥의 `exit 0` 은 뒤가 아예 안 돌아 다르다(아래 목록).
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 0; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit; }' && sed -i s/a/b/ src/store.rs",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 0; fi' && sed -i s/a/b/ src/store.rs",
            // **파이프의 칸이거나 `&` 로 띄운 `eval` 은 하위 셸이다** — 그 `exit` 는 그 칸만 끝낸다.
            // 묶음째 문 것도 같다(moai-4arw) — `|` 앞의 빈 토막에 거슬러 적는다.
            "moai mv t-1 in_progress --from todo || { echo fail; exit 1; } | cat; sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || eval 'exit 1' | cat; sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo || eval 'exit 1' & sed -i s/a/b/ src/store.rs",
            // 그 `cd` 도 뒤로 안 이어진다 — 뒤의 상대 경로는 저장소 안이다.
            "eval 'cd /tmp' | cat; echo x > src/store.rs",
            // **뒤로 띄운 것으로 끝나는 글의 값은 0 이다** — 집기가 돌기도 전에 `&&` 가 넘어간다.
            "bash -c 'moai mv t-1 in_progress --from todo &' && sed -i s/a/b/ src/store.rs",
            "eval 'echo a; moai mv t-1 in_progress --from todo &' && sed -i s/a/b/ src/store.rs",
            // **그 글은 묶음을 막 나온 토막도 아니다**(리뷰 moai-k8j1.209) — 값이 안 흐르니 그 토막은
            // 제 이음사를 그대로 읽어야 한다. 겹만 `Shell` 로 되돌리고 `feeds` 를 안 고치던 판은
            // 앞의 `;` 가 끊은 사슬을 그 토막에서 도로 살려, 아무것도 안 쥔 쓰기를 넘겼다.
            "moai mv t-1 in_progress --from todo; bash -c 'cargo build &' && sed -i s/a/b/ src/store.rs",
            "moai mv t-1 in_progress --from todo\nsh -c 'sleep 1 &' && echo x > src/store.rs",
            "true && sh -c '( moai mv t-1 in_progress --from todo & )' && sed -i s/a/b/ src/store.rs",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        // `eval` 은 지금 셸에서 돈다 — 그 `cd` 는 뒤 토막이 가리키는 트래커를 옮긴다. 파이프의 칸이면
        // 제 하위 셸이라 안 옮긴다.
        assert_eq!(aimed("eval 'cd /b'; moai add x", root), [None, None, Some(PathBuf::from("/b"))]);
        assert_eq!(aimed("eval 'cd /b' | cat; moai add x", root), [None, None, None, None]);
    }

    /// **끝내는 묶음은 그것을 연 낱말을 읽은 셸의 것이다**(moai-9xbq, 리뷰 moai-k8j1.034) — `집기 ||
    /// { …; exit 1; }` 의 `||` 는 겹 밖에서 읽었고, `if ! 집기; then exit 1; fi` 의 `if`·`!` 는 그 글
    /// 안의 낱말이다. 둘을 같이 바깥 틀에 적던 판은 셸 **안**의 `if !` 꼴을 통째로 잃었다.
    ///
    /// **그 조건이 집기의 값을 실어 오는가를 함께 본다** — 깊이 든 집기를 아무거나 세던 판은
    /// `if ! bash -c '집기; echo ok'`(집기의 값이 조건에 안 닿는다)와 `( ( 집기 ) ); if ! bash -c
    /// 'true'`(앞선 딴 겹의 집기다) 를 이긴 집기로 읽어 빈손의 쓰기를 넘겼다.
    #[test]
    fn a_bailout_group_belongs_to_the_shell_that_read_it() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e"), under("t-2", "todo", "t-e")];
        for cmd in [
            // 셸 안의 두 꼴 — 겹 밖 틀에 적던 판이 막던 자리다.
            "bash -c 'moai mv t-1 in_progress --from todo || { echo fail; exit 1; }; sed -i s/a/b/ src/store.rs'",
            "bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi; sed -i s/a/b/ src/store.rs'",
            "sh -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi; tee src/store.rs'",
            "bash -c 'set -euo pipefail; moai mv t-1 in_progress --from todo || { echo lost; exit 1; }; sed -i s/a/b/ src/store.rs'",
            // **이긴 집기는 새 셸 안에서도 이긴 채다** — 겹에 들어서며 걷던 판이 막던 자리다.
            "moai mv t-1 in_progress --from todo || exit 1; bash -c 'sed -i s/a/b/ src/store.rs'",
            "if ! moai mv t-1 in_progress --from todo; then exit 1; fi; bash -c 'sed -i s/a/b/ src/store.rs'",
            // 조건이 셸에 넘긴 글이고 그 글이 집기로 끝난다.
            "if ! bash -c 'moai mv t-1 in_progress --from todo'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "if ! eval 'moai mv t-1 in_progress --from todo'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            // **그 글의 값이 집기의 값이 아니다** — 뒤에 명령이 더 섰거나, `|| true` 가 값을 덮었거나,
            // 뒤로 띄워 값이 늘 0 이다.
            "if ! bash -c 'moai mv t-1 in_progress --from todo; echo ok'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "if ! eval 'moai mv t-1 in_progress --from todo; echo ok'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "if ! bash -c 'moai mv t-1 in_progress --from todo || true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "if ! bash -c 'moai mv t-1 in_progress --from todo &'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            // **앞선 딴 겹의 집기는 이 조건의 것이 아니다** — 깊이로만 훑던 판이 죄다 넘기던 자리다.
            "bash -c \"bash -c 'moai mv t-1 in_progress --from todo'\"; if ! bash -c 'true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "( ( moai mv t-1 in_progress --from todo ) ); if ! bash -c 'true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "{ { moai mv t-1 in_progress --from todo; }; }; if ! bash -c 'true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "echo \"$(bash -c 'moai mv t-1 in_progress --from todo')\"; if ! bash -c 'true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            "bash -c \"bash -c 'moai mv t-2 in_progress --from todo'\"; if ! bash -c 'true'; then exit 1; fi; sed -i s/a/b/ src/store.rs",
            // **안 돌 수도 있는 자리에서 이긴 집기는 겹을 나와도 이긴 것이 아니다.**
            "true || bash -c 'moai mv t-1 in_progress --from todo || exit 1' && sed -i s/a/b/ src/store.rs",
            "bash -c 'if false; then moai mv t-1 in_progress --from todo || exit 1; fi' && sed -i s/a/b/ src/store.rs",
            "sh -c 'case y in x) moai mv t-1 in_progress --from todo || exit 1;; esac' && sed -i s/a/b/ src/store.rs",
            // 하위 셸의 `exit` 는 바깥 셸을 안 끝낸다.
            "moai mv t-1 in_progress --from todo || ( bash -c 'echo fail'; exit 1 ); sed -i s/a/b/ src/store.rs",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        // **쓰기와 기록이 안 갈린다**(moai-m5mg) — 조건에 선 집기는 뒤집혀 있어 그 자리에서 안 적히고,
        // 묶음을 지나올 때 적힌다. 겹 안에서도 같다.
        let stands = |_: usize, _: &str| None;
        let mine = |cmd: &str| picked_ids(cmd, &cfg(), &|_| true, &stands);
        assert_eq!(mine("if ! bash -c 'moai mv t-1 in_progress --from todo'; then exit 1; fi"), ["t-1"]);
        assert_eq!(mine("bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi; echo ok'"), ["t-1"]);
        assert!(mine("if ! bash -c 'moai mv t-1 in_progress --from todo &'; then exit 1; fi").is_empty());
        // **안 풀린 채 끝난 묶음을 나올 때도 같다**(moai-4arw) — 쓰기가 집은 것으로 세는 줄은 기록도
        // 센다. 안 푸는 꼴(`exit 0`·안 돈 몸통)은 기록도 빈손이다.
        assert_eq!(mine("bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 1; fi'"), ["t-1"]);
        assert!(mine("bash -c 'if ! moai mv t-1 in_progress --from todo; then exit 0; fi'").is_empty());
        assert!(mine("bash -c 'if false; then moai mv t-1 in_progress --from todo || { exit 1; }; fi'").is_empty());
        // 하위 셸의 `exit` 는 자식 셸을 안 끝내, 그 줄의 **쓰기**는 빈손이다(위 목록). 기록은 다르다 —
        // 그 집기는 첫 칸에서 정말 돈다. 기록이 모으는 것은 "집었을 수 있다" 지 "이겼다" 가 아니다.
        assert_eq!(mine("bash -c 'moai mv t-1 in_progress --from todo || ( exit 1 )'"), ["t-1"]);
        // **뒤로 띄운 집기가 안 돈 몸통 안에 있으면 기록도 안 된다**(리뷰 moai-k8j1.209) — 글 끝에서
        // 닫힌 묶음의 표식은 값이 흐르는가와 따로 서야 이 줄이 걷힌다([`Lexer::relex`] 의 `over`).
        assert!(mine("bash -c 'if false; then moai mv t-1 in_progress --from todo & fi'").is_empty());
        assert!(mine("sh -c 'case y in x) moai mv t-1 in_progress --from todo & ;; esac'").is_empty());
    }

    /// **띄울 때의 플래그는 셸이 읽는 그대로 읽는다**(moai-j9tx, 리뷰 moai-k8j1.034) — `-o`·`-O` 는
    /// 뭉치의 끝이 아니어도 뒤 낱말 하나를 값으로 받고, `-c` 는 **옵션이 다 끝난 자리**의 첫 낱말을
    /// 글로 돌린다. `-c` 에서 곧바로 뒤 낱말을 집던 판은 `bash -co errexit '…'` 의 글로 `errexit` 를
    /// 집고 `bash -oe pipefail -c '…'` 는 `pipefail` 에서 멈춰, 둘 다 그 안의 쓰기를 규칙에 안 보였다.
    /// 셋 다 실제로 도는 줄이다 — `bash -co errexit 'echo hi'` 가 `hi` 를 찍는다.
    #[test]
    fn launch_flags_are_read_the_way_the_shell_reads_them() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            // 띄울 때 켠 errexit — 뭉치 어디에 있어도, `-o` 의 값으로 와도 같다.
            "bash -e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -ce 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -oe pipefail -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -co errexit 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -c -o errexit 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // 옵션이 아닌 낱말에서 멈춘다 — 그 글은 스크립트의 인자라 안 돈다.
            "bash script.sh -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -e -- -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        for cmd in [
            // **값을 받는 옵션의 값은 글이 아니다** — 글은 그 뒤에 선다. 이 줄들은 errexit 가 안 켜져
            // 집기와 쓰기가 `;` 로 갈린 채다.
            "bash -Oc extglob 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -c -- 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // 그 글이 아예 안 보이면 안 된다 — 집기 없는 쓰기다.
            "bash -oe pipefail -c 'sed -i s/a/b/ src/store.rs'",
            "bash -co errexit 'sed -i s/a/b/ src/store.rs'",
            "bash -c -o errexit 'sed -i s/a/b/ src/store.rs'",
            "bash -Oc extglob 'sed -i s/a/b/ src/store.rs'",
            // 끈 것과 대문자 `-E`(errtrace)는 errexit 가 아니다.
            "bash -e +e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash +o errexit -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            "bash -E -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
            // `--rcfile` 은 뒤 낱말을 제 값으로 받는다 — 그 `-e` 는 파일 이름이다.
            "bash --rcfile -e -c 'moai mv t-1 in_progress --from todo; sed -i s/a/b/ src/store.rs'",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// **머지하고 안 치운 워크트리의 이름이 닫힌 줄을 가리켜도 훅은 할 말이 없다**(moai-9a8m) —
    /// 닫힌 줄은 애초에 초점에 없어, 목록이 그 줄에 `⎇` 를 달던 문제를 훅은 안 안는다.
    ///
    /// **그 이름은 닫힌 줄의 자식을 여전히 쥔다**(사용자 결정, 리뷰 moai-dfv2.c9t). 풀던 판은
    /// 워크트리 안에서 줄을 닫고 그 자리에서 `--parent` 자식을 이어 하는 산 일을 "자리 없다" 로
    /// 세웠다. 치우지 않은 워크트리가 쥐는 쪽은 규약대로 치우면 풀린다 — 되돌리려면 그 결정부터.
    #[test]
    fn a_leftover_worktree_of_a_closed_row_leaves_the_focus_alone_but_keeps_its_children() {
        let root = Path::new("/repo");
        let all = vec![epic("t-e"), under("t-1", "done", "t-e"), under("t-2", "in_progress", "t-e")];
        let focus = |all: &[Issue], names: &Away| {
            held(all, &cfg(), names).iter().map(|i| i.id.clone()).collect::<Vec<_>>()
        };
        assert_eq!(focus(&all, &away(&["t-1", "worktree-t-1"])), focus(&all, &here()), "닫힌 줄의 이름이 초점을 바꿨다");
        assert_eq!(guard_edit(&all, &cfg(), &away(&["t-1"]), root, "/repo/src/x.rs"), Decision::Pass);

        let child = vec![epic("t-e"), under("t-1", "done", "t-e"), under("t-1.x", "in_progress", "t-e")];
        assert!(focus(&child, &away(&["t-1"])).is_empty(), "닫힌 줄의 이름이 자식을 놓았다 — 사용자 결정과 다르다");
    }

    /// **`--from <칸>` 의 값은 갈 칸이 아니다** (moai-f8q1). 값 받는 플래그로 안 세던
    /// 판은 `moai mv t-1 in_progress --from todo` 를 `todo` 로 옮기는 것으로 읽어,
    /// 겨루지 않고 집으라고 만든 그 플래그를 쓴 순간 규칙 2 가 집기를 못 봤다.
    #[test]
    fn a_from_column_is_not_the_column_moved_to() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in [
            "moai mv t-1 in_progress --from todo && echo x > src/store.rs",
            "moai mv t-1 --from todo in_progress && echo x > src/store.rs",
            "moai mv t-1 in_progress --from=todo && echo x > src/store.rs",
        ] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        // 갈 칸이 여전히 첫 칸·끝난 칸이면 집은 것이 아니다 — `--from` 이 그것을 가리지 않는다.
        for cmd in [
            "moai mv t-1 done --from in_progress && echo x > src/store.rs",
            "moai mv t-1 todo --from in_progress && echo x > src/store.rs",
        ] {
            assert!(matches!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// 같은 빠뜨림의 반대쪽 — `--from` 의 값이 자리 인자로 남으면 갈 칸이 `done` 이
    /// 아닌 것으로 읽혀 **리뷰 닫기 규칙이 통째로 샌다.**
    #[test]
    fn a_from_column_does_not_hide_the_closing_column() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        for cmd in ["moai mv t-r done --from review", "moai mv t-r --from review done"] {
            assert!(matches!(guard_close(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
    }

    /// BSD(macOS) sed 의 `-i ''`·`-i .bak` 은 접미사다. 스크립트로 세면 진짜
    /// 스크립트가 파일로 읽혀, 저장소 밖을 고치는 흔한 명령이 막힌다.
    #[test]
    fn a_bsd_sed_suffix_is_not_the_script() {
        let root = Path::new("/repo");
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        for cmd in ["sed -i '' 's/foo/bar/' /tmp/notes.txt", "sed -i .bak 's/a/b/' /tmp/x", "sed -i '' -E 's/a/b/' /tmp/x"] {
            assert_eq!(guard_writes(&idle, &cfg(), &here(), root, root, cmd), Decision::Pass, "막혔다 — {cmd}");
        }
        let why = denied(&guard_writes(&idle, &cfg(), &here(), root, root, "sed -i '' 's/a/b/' src/store.rs")).to_string();
        assert!(why.contains("src/store.rs"), "엉뚱한 파일을 댄다\n{why}");
    }

    /// **리뷰를 부르는 명령도 나머지 규칙을 지난다.** 한 줄에 리뷰 호출이 끼면
    /// 명령 전체를 리뷰로만 보던 판은, 그 한 토막으로 닫기 규칙과 규칙 1 을 넘겼다.
    #[test]
    fn a_review_call_does_not_carry_the_rest_past_the_rules() {
        let root = Path::new("/repo");
        let reviewing = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r", "in_progress", Some("t-e"))];
        assert_eq!(guard_shell(&reviewing, &cfg(), &here(), root, root, "/code-review high"), Decision::Pass);
        for cmd in ["moai mv t-r done && /code-review high", "moai add '딴 일' && claude /code-review high"] {
            assert!(matches!(guard_shell(&reviewing, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let why = denied(&guard_shell(&idle, &cfg(), &here(), root, root, "cd /repo && /code-review high")).to_string();
        assert!(why.starts_with(&crate::guide::rule_head(3)), "리뷰 규칙이 안 섰다\n{why}");
    }

    /// **닫는 걸음은 한 출처다.** 거절문과 세션 닫기가 손으로 적던 두 벌은 이미
    /// 서로 다른 글을 내고 있었다.
    #[test]
    fn the_closing_steps_come_from_the_guide() {
        let all = vec![issue("t-1", "in_progress"), review("t-r", "in_progress", None)];
        let steps = crate::guide::close_steps("t-r", "moai");
        assert!(steps.contains("moai note t-r -b -") && steps.contains("moai mv t-r done -m"), "{steps}");
        let why = denied(&guard_close(&all, &cfg(), &here(), "moai mv t-r done")).to_string();
        assert!(why.contains(&steps), "닫기 거절문이 갈라졌다\n{why}");

        let near = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r2", "todo", Some("t-e"))];
        let Decision::Block(held) = closing(&near, &near, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(held.contains(&crate::guide::close_steps("t-r2", "moai")), "세션 닫기가 갈라졌다\n{held}");
    }
}

#[cfg(test)]
mod korean_tests {
    use super::*;

    /// **한국어 글을 넣는 쓰기만 비춘다**(moai-6rrb). 읽기·영어 글·사람 이름·꼴이 정해진 줄은 아니다 —
    /// 헛 알림이 잦으면 알림을 안 읽게 된다.
    #[test]
    fn only_korean_text_going_into_moai_is_noticed() {
        for cmd in [
            "moai add '빈 태그를 못 거른다' -t bug",
            "moai note t-1 '발견한 것'",
            "moai mv t-1 done -m '리뷰를 반영했다'",
            "moai -C /repo idea add '떠오른 것'",
            "moai edit t-1 -b - <<'B'\n- 무엇: 어긋났다\nB",
            "cd /repo && moai add --from - <<'PLAN'\n# 에픽\n- [p1] 첫 이슈\nPLAN",
            "moai idea promote t-1 --from - <<'PLAN'\n# 에픽\n- [p1] 첫 이슈\nPLAN",
            // 명령 치환 속 heredoc — 렉서가 본문을 낱말에 안 남기는 자리다(리뷰 moai-5wk4.76z).
            "moai note t-1 \"$(cat <<'EOF'\n한글 노트 본문\nEOF\n)\"",
            "moai mv t-1 done -m \"$(cat <<'EOF'\n리뷰를 반영했다\nEOF\n)\"",
            // 파이프 앞 칸과 here-string 이 내는 글.
            "printf '한글 노트' | moai note t-1 -b -",
            "cat <<'B' | moai note t-1 -b -\n한글 노트\nB",
            "moai note t-1 -b - <<< '한글 노트'",
            // 꼴을 닮았을 뿐인 글 — 줄 머리가 아니거나 머리 뒤가 빈칸이 아니다(`model::parse_work` 와 같은 자).
            "moai note t-1 'model::label 이 이름과 메일을 합친다'",
            "moai mv t-1 done -m '  model: anthropic/opus-5 (low — 들여 쓴 예)'",
        ] {
            assert!(writes_korean(cmd), "안 비춘다 — {cmd:?}");
        }
        for cmd in [
            "moai show -g 한국어",
            "moai add 'fix the empty tag' -t bug",
            "moai --user '레이븐 (r@x)' add 'english title'",
            "moai note t-1 'model: anthropic/opus-5 (high — 쓰기 경로)'",
            "moai note t-1 '다음: 훅 멤버를 이어서 한다'",
            "moai add '제목' --help",
            "echo '한글' | grep moai",
            "moai note t-1 -b - < /tmp/review.md",
            // 네임스페이스 밑의 읽기(리뷰 moai-5wk4.76z).
            "moai idea ls -g 한국어",
            "moai epic show -g 저장",
            "moai issue show -t 파서",
            "moai milestone ls -g 출시",
            // 글이 아닌 값 — 태그·칸·붙여 쓴 사람.
            "moai add 'fix the parser' -t 파서",
            "moai edit t-1 --tag=파서",
            "moai mv t-1 진행 --from 할일",
            "moai defer t-1 --from 할일",
            "moai add 'english' -a레이븐",
            // 연습은 아무것도 안 넣는다.
            "moai add --from - --dry-run <<'PLAN'\n# 에픽\n- [p1] 첫 이슈\nPLAN",
            // stdin 의 영어 본문 곁의 한글 — 옆 토막의 커밋 메시지·사람·경로·파일 뒤의 말.
            "moai note t-1 -b - <<'B'\nenglish only\nB\ngit commit -m 'chore(tracker): t-1 노트를 담는다' -- .moai/",
            "moai --user '레이븐 (r@x.y)' note t-1 -b - <<'B'\nenglish\nB",
            "MOAI_ACTOR='레이븐 (r@x.y)' moai note t-1 -b - <<'B'\nenglish\nB",
            "moai -C /home/레이븐/repo note t-1 -b - <<'B'\nenglish\nB",
            "moai note t-1 -b - < /tmp/review.md && echo '완료'",
            "moai note t-1 -b - <<'B'\nmodel: anthropic/opus-5 (high — 쓰기 경로)\nB\ngit commit -m '닫는다'",
        ] {
            assert!(!writes_korean(cmd), "헛 비춘다 — {cmd:?}");
        }
    }

    /// **트래커가 없는 자리를 가리킨 토막은 안 센다** — 받는 쪽이 `only` 로 뺀다. 글이 **간 자리**는
    /// 알림의 고쳐 적는 명령으로 옮긴다 — 친 `-C` 글자가 아니라 [`aimed`] 가 푼 자리다(moai-j2vp,
    /// moai-v9sa 와 한 자). `cd src && moai -C .. note …` 와 워크트리 안의 `moai -C . note …` 가 그
    /// 글자를 도로 내밀면, 옮겨 친 사람이 딴 트래커나 워크트리의 스냅샷을 고친다.
    #[test]
    fn the_korean_write_names_where_it_went() {
        let here = |_: usize| None;
        let there = |_: usize| Some(Path::new("/repo/sub"));
        assert_eq!(korean_write("moai note t-1 '한글'", &|_| true, &here).as_deref(), Some(""));
        assert_eq!(korean_write("moai -C .. note t-1 '한글'", &|_| true, &here).as_deref(), Some(""), "친 글자를 옮겨 적었다");
        assert_eq!(korean_write("moai -C /x note t-1 '한글'", &|_| true, &there).as_deref(), Some(" -C /repo/sub"));
        let spaced = |_: usize| Some(Path::new("/a b"));
        assert_eq!(korean_write("moai -C '/a b' note t-1 '한글'", &|_| true, &spaced).as_deref(), Some(" -C '/a b'"));
        // 못 푼 자리(변수·`~`)는 친 글자를 그대로 되돌려 준다 — 그 셸이 다시 풀면 같은 자리다.
        assert_eq!(korean_write("moai -C \"$OTHER\" note t-1 '한글'", &|_| true, &here).as_deref(), Some(" -C \"$OTHER\""));
        assert_eq!(korean_write("moai -C /nowhere note t-1 '한글'", &|_| false, &here), None);
        assert_eq!(
            korean_write("moai -C /nowhere note t-1 '한글'; moai note t-2 '둘째'", &|k| k == 1, &here).as_deref(),
            Some("")
        );
    }

    /// heredoc 본문과 here-string 은 **그것을 연 토막**에 흘러든다 — 줄이 끝난 뒤에 읽혀도, 명령 치환 안에서
    /// 열려도 같다. 다른 토막에는 안 섞인다.
    #[test]
    fn fed_text_stays_with_the_segment_that_opened_it() {
        let segs = parse("moai note t-1 -b - <<'B' && git commit -m x\n본문\nB\necho y <<< '여기'\nmoai note t-2 \"$(cat <<'E'\n안쪽\nE\n)\"");
        let fed: Vec<(&str, Vec<&str>)> = segs
            .iter()
            .filter(|s| s.nested.is_empty())
            .map(|s| (s.words[0].as_str(), s.fed.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(
            fed,
            [("moai", vec!["본문\n"]), ("git", vec![]), ("echo", vec!["여기"]), ("moai", vec!["안쪽\n"])],
            "{segs:?}"
        );
    }

    /// **막지 않고, 없는 플러그인은 사람에게 청하게 한다.** 에이전트가 제 손으로 깔면 사용자 결정
    /// (moai-5wk4)을 뒤집는다. 비추는 스킬 이름은 안내와 같은 플러그인의 것이다.
    #[test]
    fn the_korean_notice_never_blocks_and_asks_a_person_to_install() {
        let Decision::Context(said) = korean_notice("", &["korean-skills@korean-skills"]) else {
            panic!("비추는 답이 아니다");
        };
        assert!(said.contains("moai skill install") && said.contains("사람에게"), "{said}");
        assert!(!said.contains("claude plugin install"), "제 손으로 깔라고 한다\n{said}");
        for (id, _) in crate::guide::KOREAN_PLUGINS {
            let plugin = id.split_once('@').unwrap().0;
            assert!(said.contains(&format!("`{plugin}:")), "{plugin} 의 스킬을 안 댄다\n{said}");
        }
        let Decision::Context(quiet) = korean_notice("", &[]) else { panic!("비추는 답이 아니다") };
        assert!(!quiet.contains("moai skill install"), "다 깔렸는데 깔라고 한다\n{quiet}");
    }

    /// **노트와 `-m` 은 고쳐 적으라고 하지 않는다**(리뷰 moai-5wk4.76z) — 저널에만 쌓여, 다시 적으면 같은 글이
    /// 두 벌 남는다. 고칠 수 있는 제목·본문은 그 글을 넣은 트래커로 `moai edit` 를 댄다.
    #[test]
    fn the_korean_notice_only_offers_edits_that_exist() {
        let Decision::Context(said) = korean_notice(" -C /repo", &[]) else { panic!("비추는 답이 아니다") };
        assert!(said.contains("`moai -C /repo edit`"), "{said}");
        assert!(!said.contains("`moai note` 로 고쳐") && said.contains("저널에만"), "{said}");
    }

    /// `humanize-korean` 이 cwd 에 만드는 `_workspace/` 는 규칙 2 가 세지 않는다 — 하위 디렉터리에 선
    /// 세션이 만든 것도.
    #[test]
    fn the_humanizer_workspace_is_not_counted() {
        assert!(!counted("_workspace/2026-09-18-001/final.md", Path::new("/repo")));
        assert!(!counted("/repo/sub/_workspace/2026-09-18-001/01_input.txt", Path::new("/repo")));
        assert!(counted("src/main.rs", Path::new("/repo")));
        assert!(counted("src/_workspace_notes.rs", Path::new("/repo")));
    }
}
