//! 훅이 무엇을 낼지 정한다. **순수 함수다** — 이벤트와 `&[Issue]` 만 보고
//! 아무것도 찍지 않고 아무것도 읽지 않는다.
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
    /// 세션이 열렸다. 기준선을 적고, 접힌 뒤면 집고 있던 것을 싣는다
    SessionStart,
    /// 사람이 무언가 시켰다. 보드를 세션당 한 번 싣는다
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
pub fn carried(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>) -> Decision {
    let wip = held(issues, cfg, away);
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
    /// 몇 겹의 묶음 안인가 — `( … )` 와 `{ … }` 를 함께 센다. [`Join::depth`] 와 같은 자다.
    /// [`Seg::depth`] 로 집기의 깊이를 재면 `{ mv; sed -i …; }` 의 `;` 가 집기보다 깊은 자리로
    /// 읽혀, 집기가 져도 도는 쓰기가 샌다.
    level: usize,
    /// 앞 토막과 무엇으로 이었나 — 앞이 이겨야만 도는가(`&&`), 한 파이프라인인가(`|`), 그 이음사를
    /// 몇 겹의 묶음 안에서 읽었나. [`shell_writes`] 가 집기 뒤의 쓰기를 넘길지를 이것으로 가른다.
    join: Join,
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
    /// `;`·`||`·`&`·줄바꿈 — 앞이 져도 돈다. 첫 토막도 이것이다.
    #[default]
    Any,
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
}

/// 다음 낱말이 무엇의 과녁인가.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Aim {
    /// 보통 낱말.
    #[default]
    Word,
    /// `>`·`>>`·`>|`·`&>`·`<>` — 쓰는 파일.
    Write,
    /// `<`·`<<<`·`<&` — 읽는 것. 낱말도 쓰기도 아니다.
    Read,
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
    /// 이 줄이 끝나면 건너뛸 heredoc 본문들 — 종료어와, 앞 탭을 걷는가(`<<-`).
    heredocs: Vec<(String, bool)>,
    /// 지금 몇 겹의 `( … )` 묶음 안인가([`Seg::depth`]).
    group: usize,
    /// 마지막으로 읽은 이음사 — 다음에 쌓이는 토막의 [`Seg::join`] 이 된다.
    join: Join,
    /// 몇 겹의 `{ … }` 와 예약어 묶음(`if … fi`) 안인가. **이음사와 집기의 깊이
    /// ([`Seg::level`])에만 든다** — 하위 셸이
    /// 아니라 그 안의 `cd` 는 뒤로 이어지므로 [`Seg::depth`] 에는 안 든다. 안 세면
    /// `mv && { a; b; }` 의 `;` 가 묶음 밖의 끊김으로 읽혀, 집기가 이겨야만 도는 쓰기를 막는다.
    braces: usize,
}

impl<'a> Lexer<'a> {
    fn new(cmd: &'a str) -> Self {
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
            group: 0,
            join: Join::default(),
            braces: 0,
        }
    }

    fn run(mut self) -> Vec<Seg> {
        while let Some(c) = self.chars.next() {
            match self.stack.last().copied() {
                None => self.plain(c),
                Some(Ctx::Single) => self.single(c),
                Some(Ctx::Ansi) => self.escaped(c, '\''),
                Some(Ctx::Double) => self.double(c),
                Some(Ctx::Subst(depth)) => self.subst(c, depth),
                Some(Ctx::Arith(depth)) => self.arith(c, depth),
            }
        }
        self.end_by(None);
        self.all.into_iter().filter(|s| !s.words.is_empty() || !s.writes.is_empty()).collect()
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
                self.stack.push(Ctx::Subst(1));
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
            '|' if self.chars.next_if_eq(&'|').is_some() => self.end_by(Some(Op::Any)),
            '&' if self.chars.next_if_eq(&'&').is_some() => self.end_by(Some(Op::And)),
            '|' => {
                self.chars.next_if_eq(&'&');
                self.seg.sub = true;
                self.end_by(Some(Op::Pipe));
                self.seg.sub = true;
            }
            '&' => {
                self.flush();
                if !self.seg.words.is_empty() {
                    self.seg.sub = true;
                }
                self.end_by(Some(Op::Any));
            }
            ';' => self.end_by(Some(Op::Any)),
            // 괄호는 이음사가 아니다 — `a && ( b )` 의 `b` 는 `&&` 로 이어진 것이다.
            ')' => {
                self.end_by(None);
                self.group = self.group.saturating_sub(1);
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
            self.stack.push(Ctx::Subst(1));
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
                self.stack.pop();
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
        self.cur.push(c);
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
                    self.aim = Aim::Read;
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
        while let Some(&d) = self.chars.peek() {
            match d {
                '\'' | '"' => {
                    self.chars.next();
                    for e in self.chars.by_ref() {
                        if e == d {
                            break;
                        }
                        tag.push(e);
                    }
                }
                '\\' => {
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
            self.heredocs.push((tag, strip));
        }
    }

    /// 줄이 끝났다 — 적어 둔 heredoc 본문을 차례로 건너뛴다.
    ///
    /// **종료어는 그 줄 전체여야 한다** (`<<-` 는 앞 탭만 걷는다). 셸이 그렇게
    /// 읽는다. 앞뒤를 다듬어 견주던 판은 본문에 인용된 `    MD` 에서 본문을
    /// 끝내, 남은 본문을 명령으로 읽었다.
    fn skip_heredocs(&mut self) {
        for (tag, strip) in std::mem::take(&mut self.heredocs) {
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
                if body.strip_suffix('\r').unwrap_or(body) == tag || !more {
                    break;
                }
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
            Aim::Dup if word.chars().all(|d| d.is_ascii_digit() || d == '-') => {}
            Aim::Dup => self.seg.writes.push(word),
            Aim::Word => {
                // 명령 자리의 `{`·`}` 만 묶음이다 — `echo {` 의 `{` 는 글자다.
                //
                // **닫는 `}` 는 `)` 처럼 묶음을 닫을 뿐 낱말이 아니다.** 낱말로 남으면 제 토막을
                // 세워 안쪽 `;` 를 제 이음사로 들고 서서 — `{ mv; } && sed -i …` 의 집기를 끊고 —
                // 다음 `}` 가 명령 자리를 못 봐 `{ { a; } }` 의 깊이가 하나 남는다.
                if !quoted && word == "{" && command_of(&self.seg.words).is_empty() {
                    self.braces += 1;
                } else if !quoted && word == "}" && self.seg.words.is_empty() && self.braces > 0 {
                    self.braces -= 1;
                    return;
                } else if !quoted && OPENS.contains(&word.as_str()) && command_of(&self.seg.words).is_empty() {
                    // **예약어 묶음도 `{ … }` 처럼 센다**(moai-1jvy). `if …; then …; fi` 의 몸통
                    // `;` 를 묶음 밖의 끊김으로 읽던 판은 `mv && if true; then sed -i …; fi` 를
                    // 막았다 — 잘못 막는 쪽이라 새지는 않지만, 훅이 안 되는 모양을 늘리면 사람은
                    // 규칙을 지키는 법이 아니라 피하는 법부터 배운다.
                    self.braces += 1;
                } else if !quoted && SHUTS.contains(&word.as_str()) && self.seg.words.is_empty() && self.braces > 0 {
                    // 닫는 낱말은 `}` 처럼 묶음을 닫을 뿐 명령이 아니다 — 낱말로 남기면 제 토막을
                    // 세워 안쪽 `;` 를 제 이음사로 들고 선다.
                    self.braces -= 1;
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
        // 빈 토막은 쌓지 않는다 — 어차피 걸러지고, 그 표식(`a |\n b` 의 `sub`)은 다음 토막의 것이다.
        if self.seg.words.is_empty() && self.seg.writes.is_empty() {
            if let Some(op) = op {
                self.join = Join { op, depth: self.level() };
            }
            return;
        }
        let mut seg = std::mem::take(&mut self.seg);
        seg.depth = self.group;
        seg.level = self.level();
        seg.join = self.join;
        self.all.push(seg);
        self.join = Join { op: op.unwrap_or_default(), depth: self.level() };
    }

    /// 지금 몇 겹의 묶음 안인가([`Seg::level`]).
    fn level(&self) -> usize {
        self.group + self.braces
    }
}

/// 묶음을 여는 예약어 — **명령 자리에 섰을 때만이다**(`echo if` 의 `if` 는 글자다).
///
/// 여는 쪽만 센다. `for`·`select`·`while`·`until` 은 `do … done` 으로 닫히는데 `do` 는
/// [`PREFIXES`] 라 명령 자리를 안 옮기므로, 여는 낱말 하나와 `done` 하나로 짝이 맞는다.
const OPENS: &[&str] = &["if", "while", "until", "for", "select", "case"];

/// 묶음을 닫는 예약어. **`case` 의 갈래 `)` 는 여기서 안 센다** — 괄호 깊이는
/// `saturating_sub` 로 0 에 머무르고, 갈래의 몸통은 `esac` 까지 이 묶음 안이다.
const SHUTS: &[&str] = &["fi", "done", "esac"];

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
fn command_of(words: &[String]) -> &[String] {
    let skip = words
        .iter()
        .take_while(|w| PREFIXES.contains(&w.as_str()) || (w.contains('=') && !w.starts_with(['-', '='])))
        .count();
    &words[skip..]
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
pub fn unit_of<'a>(issues: &'a [Issue], focus: &[&'a Issue]) -> BTreeSet<&'a str> {
    let epics = report::groups(issues);
    let stones = report::milestones(issues);
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

/// 이 자리에서 집고 있는 것 — [`report::wip`] 에서 **옆 워크트리가 쥔 일**을 뺀 것.
///
/// 집기 커밋은 main 에 들어가므로(CLAUDE.md "워크트리"), main 과 거기서 뜬 워크트리의
/// 스냅샷에는 옆 세션들이 집은 줄이 다 벌여 놓은 칸에 서 있다. 그것을 제 초점으로
/// 세던 판은 main 의 `moai add` 를 규칙 1 로 막고, 세션을 닫을 때 남의 워크트리 일을
/// 옮기거나 미루라고 붙들었다(moai-0yrv). 워크트리가 원칙이 되면 그 거절은 상시다.
///
/// `away` 는 옆 워크트리의 이름이 가리키는 id 들이다(`worktree::away`) — 여기는 읽지
/// 않는다. 그 줄 자신, 그 밑의 자식, 그 에픽·마일스톤에 든 줄을 뺀다. 옆에서 그 일을
/// 펼쳐 집은 것도 옆의 것이다.
pub fn held<'a>(issues: &'a [Issue], cfg: &Config, away: &BTreeSet<String>) -> Vec<&'a Issue> {
    let wip = report::wip(issues, cfg);
    if away.is_empty() || wip.is_empty() {
        return wip;
    }
    let theirs = theirs(issues, away);
    wip.into_iter().filter(|i| !theirs(i)).collect()
}

/// 이 줄이 **옆 워크트리의 일**인가 — 그 줄 자신이나 조상이 `away` 에 들었거나, 그 에픽·
/// 마일스톤이 들었다. [`held`] 와 그 초점을 쓰는 규칙이 같은 자로 재야 한다 — 초점에서는
/// 뺀 옆의 리뷰 줄을 규칙 3 이 "집으라" 고 대면, 이미 옆에서 집은 줄이라 시킨 대로 해도
/// 안 풀린다.
fn theirs<'a>(issues: &'a [Issue], away: &'a BTreeSet<String>) -> impl Fn(&Issue) -> bool + 'a {
    report::claimed(issues, away)
}

/// **누구의 것인지 모르는** 집은 줄 — 옆 딸린 워크트리의 스냅샷에도 벌여 놓인(또는 거기서 늦게
/// 옮긴) 줄(`elsewhere`, `worktree::held_elsewhere`)이다 (moai-ntl6, 사용자 결정 B).
///
/// 이름이 id 가 아닌 워크트리가 갈라질 때 이미 집혀 있던 일은 그 워크트리의 것일 수 있다.
/// **이 줄로는 막지도 붙들지도 않는다** — 받는 쪽은 이것을 `away` 에 더한 좁은 초점으로 한 번
/// 더 판정해, 풀릴 때만 푼다. 새로 막는 일은 없다. **제 워크트리 이름이 가리키는 일은 확실히
/// 제 것이라 빼지 않는다**(`own`) — 이름으로 가르던 판정은 그대로다. 대가: main 이 제 몫으로
/// 집은 뒤 갈라진 워크트리가 생기면 그 집기는 `Stop` 이 더는 안 붙든다(사용자가 받아들였다).
pub fn unsure(issues: &[Issue], cfg: &Config, elsewhere: &BTreeSet<String>, own: &BTreeSet<String>) -> BTreeSet<String> {
    if elsewhere.is_empty() {
        return BTreeSet::new();
    }
    let named_mine = theirs(issues, own);
    report::wip(issues, cfg)
        .into_iter()
        .filter(|i| elsewhere.contains(&i.id) && !named_mine(i))
        .map(|i| i.id.clone())
        .collect()
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
pub fn guard_create(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>, cmd: &str) -> Decision {
    create_in(issues, &held(issues, cfg, away), cmd, &|_| true)
}

/// [`guard_create`] 를 `only` 가 고른 토막에만 — 다른 트래커를 가리키는 토막은 그 트래커의
/// 줄로 본다([`aimed`]). 초점([`held`])은 [`guard_moai`] 가 한 번 잰 것을 받는다.
fn create_in<'a>(issues: &'a [Issue], focus: &[&'a Issue], cmd: &str, only: &dyn Fn(usize) -> bool) -> Decision {
    if focus.is_empty() {
        return Decision::Pass;
    }
    let unit = unit_of(issues, focus);

    // **토막마다 본다.** `cd /repo && moai add …` 의 뒷토막이 진짜 생성이다.
    let makes = segments(cmd).into_iter().enumerate().filter(|(k, _)| only(*k)).map(|(_, seg)| seg).find(|seg| {
        // `add` 만 본다. `idea add` 는 담는 자리고, `--from` 은 에픽과 그
        // 자식들을 한 단위로 세우는 자리라 새는 줄이 아니다.
        //
        // **`--from` 도 토큰으로 본다.** 글자로 찾으면 제목이 그 낱말을 담은
        // `moai add "--from 을 나중에"` 가 규칙을 통째로 지나간다 — 동사를
        // 자리로 읽기로 한 것과 같은 까닭이다.
        creates(seg)
            && (promotes_into(seg) || !seg.iter().any(|t| t == "--from" || t.starts_with("--from=")))
            // **도움말은 만들지 않는다.** 우리가 심는 스킬이 "모르면
            // `moai <명령> --help` 를 보라" 고 적어 두는데, 그 길을 막으면
            // 규칙이 제가 시킨 것을 막는다.
            && !asks_help(seg)
    });
    let Some(seg) = makes else {
        return Decision::Pass;
    };
    if flag_values(&seg, &["-e", "--epic", "--parent", "--milestone"])
        .iter()
        .any(|v| unit.contains(v.as_str()))
    {
        return Decision::Pass;
    }

    let head = focus[0];
    let held = focus
        .iter()
        .take(3)
        .map(|i| format!("{} {}", i.id, i.title))
        .collect::<Vec<_>>()
        .join(", ");
    // **에픽이 없으면 에픽을 대라고 말하지 않는다.** 없는 에픽 자리에 이슈 id 를
    // 넣어 일러 주던 자리다 — 시키는 대로 치면 `moai add "제목" -e <이슈>` 가
    // 만들어지고, `moai status` 에 "에픽으로 쓸 수 없는 것을 가리키는 줄" 이
    // 하나 는다. 그리고 경고가 늘면 `closing` 이 세션을 붙든다. 훅이 시킨 대로
    // 한 것이 훅에 걸리는 자리는 규칙이 아니라 덫이다.
    let epic = report::groups(issues).get(head.id.as_str()).cloned();
    let into_epic = epic
        .as_ref()
        .map(|e| format!("\x20 moai add \"제목\" -e {e}        같은 에픽 안에\n"))
        .unwrap_or_default();
    // **무엇이 내건 것인지는 갈림길 1 과 같은 자로 댄다 — 에픽이다.** "그 일" 로 적던 판은 집은
    // 이슈가 아니라 에픽이 필요로 하는 것(moai-1k17 이 그 모양)에서 갈림길 1 과 다른 답을 냈다.
    // 에픽이 없을 때만 집은 일 자신이다.
    let aim = epic.as_deref().unwrap_or(head.id.as_str());
    // **첫 칸에 둔 줄이 일을 열어 두는 것은 에픽뿐이다** — 에픽의 칸은 멤버에서 읽지만, 에픽 없는
    // 일은 자식이 첫 칸에 있어도 그대로 닫힌다. 그때 "첫 칸에 두면 안 닫힌다" 를 비치면 거짓이다.
    // 가리키는 줄도 이름으로 댄다 — "위의 줄" 바로 위가 `idea add` 줄이고, idea 도 첫 칸에 선다.
    let keep = if epic.is_some() {
        format!("위의 `moai add` 줄로 세워 첫 칸에 둔다. 밖으로 내보내면 {aim} 가 목적을 못 이룬 채 닫힌다")
    } else {
        format!("위의 `--parent` 줄로 세운다. 에픽 없는 일은 자식이 남아도 닫히니 {aim} 를 닫기 전에 끝낸다")
    };
    refuse(1, format!(
        "지금 집고 있는 것이 있다 — {held}.\n\
         그 단위 안에서 만들거나, 밖의 것이면 담아 둔다. 초점 밖에 이슈를 세우면\n\
         그 줄이 어느 일에서 나왔는지를 잃는다.\n\
         {into_epic}\x20 moai add \"제목\" --parent {}   그 일의 자식으로\n\
         \x20 moai idea add \"제목\"                 지금 할 일이 아니면 담아 둔다\n\
         {aim} 가 내건 것이 이것 없이 안 이뤄지면 idea 가 아니다 — 지금 못 해도\n\
         {keep}",
        head.id
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
pub fn guard_close(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>, cmd: &str) -> Decision {
    close_in(issues, cfg, away, cmd, &|_| true)
}

/// [`guard_close`] 를 `only` 가 고른 토막에만 — [`create_in`] 과 같은 까닭.
fn close_in(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>, cmd: &str, only: &dyn Fn(usize) -> bool) -> Decision {
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
        if !flag_values(&seg, &["-m", "--msg"]).iter().all(|v| v.trim().is_empty()) {
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
                crate::guide::close_steps(&r.id)
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
pub fn guard_edit(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>, root: &Path, target: &str) -> Decision {
    if !counted(target, root) || !held(issues, cfg, away).is_empty() {
        return Decision::Pass;
    }
    let picks = report::ready(issues, cfg)
        .into_iter()
        .take(3)
        .map(|i| format!("  moai mv {} in_progress   {}", i.id, i.title))
        .collect::<Vec<_>>();
    let picks = if picks.is_empty() {
        "  moai add \"제목\" 뒤에 moai mv <id> in_progress".to_string()
    } else {
        picks.join("\n")
    };
    refuse(2, format!(
        "집은 것 없이 {} 를 고치고 있다. 어느 일에서 나온 변경인지가 남지 않는다.\n\
         하나를 집고 다시 부른다.\n{picks}\n\
         계획에 없던 것이면 `moai add \"제목\"` 으로 세우고 그것을 집는다.",
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
    away: &BTreeSet<String>,
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
    away: &BTreeSet<String>,
    root: &Path,
    cwd: &Path,
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
) -> Decision {
    if !held(issues, cfg, away).is_empty() {
        return Decision::Pass;
    }
    for path in shell_writes(cmd, cfg, only) {
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
    away: &BTreeSet<String>,
    root: &Path,
    cwd: &Path,
    cmd: &str,
) -> Decision {
    guard_shell_in(issues, cfg, away, root, cwd, cmd, &|_| true)
}

/// [`guard_shell`] 을 세션 자리의 트래커로 — 만들기·닫기 규칙은 `only` 가 고른 `moai` 토막만
/// 본다. 다른 트래커를 가리키는 토막은 [`guard_moai`] 가 그 트래커의 줄로 본다(moai-23ky).
/// 쓰기와 리뷰는 세션 자리의 규칙이라 명령 전체를 본다 — 쓰기 규칙이 `only` 로 가르는 것은
/// 어느 집기가 이 자리의 것인가 하나다.
pub fn guard_shell_in(
    issues: &[Issue],
    cfg: &Config,
    away: &BTreeSet<String>,
    root: &Path,
    cwd: &Path,
    cmd: &str,
    only: &dyn Fn(usize) -> bool,
) -> Decision {
    // 차례는 [`Decision::then`] 이 정한다 — 먼저 막는 규칙이 이기고, 비추는 줄(`Context`)은 뒤의
    // 규칙이 막을 것을 가리지 않는다.
    guard_moai(issues, cfg, away, cmd, only)
        .then(|| guard_writes_in(issues, cfg, away, root, cwd, cmd, only))
        .then(|| if calls_review(cmd) { guard_review(issues, cfg, away) } else { Decision::Pass })
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
/// **집기로 안 세는 것:** 남의 트래커를 가리킨 집기(`only` 가 안 고른 토막 — 여기서 아무것도
/// 안 쥐어 준다)와 `! moai mv …` (집기가 져야 뒤가 돈다).
fn shell_writes(cmd: &str, cfg: &Config, only: &dyn Fn(usize) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut moved = false;
    // 집기 뒤 `&&` 로만 이어 온 동안의 묶음 깊이 — 이보다 얕거나 같은 자리에서 `&&` 가 아닌
    // 이음사를 만나면 끝난다.
    let mut after_pick: Option<usize> = None;
    // 깊이마다, 지금 열린 파이프라인에 들어설 때의 `after_pick` — 파이프의 칸은 그것을 잇는다.
    let mut heads: Vec<Option<usize>> = Vec::new();
    // [`segments`] 의 토막 번호 — `only` 가 그것으로 가른다. 낱말 없는 토막(`> f` 만)은 안 센다.
    let mut k = 0;
    for seg in parse(cmd) {
        let j = seg.join;
        // 이음사가 제 깊이보다 깊으면 묶음을 막 나온 토막 — `( … ) > f` 의 `> f` 다. 그 묶음에
        // 들어설 때의 판으로 쓰고, 묶음의 값은 그대로 뒤로 흐른다. 이음사로 읽던 판은 안쪽 `;` 로
        // 집기를 끊어 `(mv) > /dev/null && sed -i …` 를 막았다.
        let gate = if j.depth > seg.level {
            heads.get(seg.level).copied().flatten()
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
            after_pick
        };
        let n = k;
        if !seg.words.is_empty() {
            k += 1;
        }
        let words = command_of(&seg.words);
        let head = words.first().map(|w| basename(w));
        // `[[ a > b ]]`·`(( a > b ))` 의 `>` 는 비교다 — 낱말을 가를 때 이미 걸렀다.
        let mut found = seg.writes;
        match head {
            Some("sed") => found.extend(sed_in_place(&words[1..])),
            Some("tee") => found.extend(words[1..].iter().filter(|w| !w.starts_with('-')).cloned()),
            _ => {}
        }
        for path in found {
            if gate.is_some() || unknowable(&path) || (moved && !Path::new(&path).is_absolute()) {
                continue;
            }
            out.push(path);
        }
        if matches!(head, Some("cd" | "pushd" | "popd")) {
            moved = true;
        }
        let negated = seg.words[..seg.words.len() - words.len()].iter().any(|w| w == "!");
        if picks_up(&seg.words, cfg) && only(n) && !negated {
            after_pick = Some(after_pick.map_or(seg.level, |d| d.min(seg.level)));
        }
    }
    out
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
pub fn guard_moai(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>, cmd: &str, only: &dyn Fn(usize) -> bool) -> Decision {
    // 초점은 한 번 잰다 — `held` 는 미룬 줄이 있으면 소속 지도를 다시 짓고, 훅은 도구 호출마다 돈다.
    let focus = held(issues, cfg, away);
    create_in(issues, &focus, cmd, only)
        .then(|| close_in(issues, cfg, away, cmd, only))
        .then(|| aside_in(issues, &focus, cmd, only))
}

/// 이 토막이 **생각을 담는가** — [`adds_idea`] 의 두 철자. 도움말은 아무것도 안 담는다.
fn sets_aside(seg: &[String]) -> bool {
    let Some(args) = moai_args(seg) else { return false };
    adds_idea(args, &positionals(args)) && !asks_help(seg)
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
fn aside_in(issues: &[Issue], focus: &[&Issue], cmd: &str, only: &dyn Fn(usize) -> bool) -> Decision {
    if focus.is_empty() {
        return Decision::Pass;
    }
    let Some(seg) =
        segments(cmd).into_iter().enumerate().find(|(k, seg)| only(*k) && sets_aside(seg)).map(|(_, seg)| seg)
    else {
        return Decision::Pass;
    };
    let epics = report::groups(issues);
    let mut aims: Vec<&str> = Vec::new();
    for e in focus.iter().filter_map(|i| epics.get(i.id.as_str()).copied()) {
        if !aims.contains(&e) && issues.iter().any(|g| g.id == e && g.kind == crate::model::Kind::Epic) {
            aims.push(e);
        }
    }
    let Some(first) = aims.first() else {
        return Decision::Pass;
    };
    let at = moai_args(&seg)
        .and_then(|args| flag_values(args, &["-C", "--dir"]).pop())
        .map(|d| {
            if d.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') {
                format!(" -C '{}'", d.replace('\'', r"'\''"))
            } else {
                format!(" -C {d}")
            }
        })
        .unwrap_or_default();
    Decision::Context(format!(
        "갈림길 1 의 둘째 물음 — {} 가 내건 것이 방금 담은 생각 없이도 이뤄지는가. 아니면 idea 가 아니라 안 끝난 이 일이다.\n\
         그렇다면 지금 못 해도 `moai{at} idea promote <그 id> -e {first} --from -` 로 그 에픽의 멤버로 되찾아 첫 칸에 \
         둔다 — 밖에 두면 에픽이 목적을 못 이룬 채 닫힌다.",
        aims.join("·")
    ))
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
            while outer.len() > seg.depth {
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
pub fn guard_review(issues: &[Issue], cfg: &Config, away: &BTreeSet<String>) -> Decision {
    let out_of_plan = report::put_off(issues);
    // **옆 워크트리의 리뷰는 여기서 안 센다** — 초점에서 뺀 것과 같은 자다([`theirs`]).
    // 세면 main 에서 아무것도 안 집은 세션에 옆이 이미 집은 리뷰를 "집으라" 고 막는다.
    let theirs = theirs(issues, away);
    let open: Vec<&Issue> = issues
        .iter()
        .filter(|i| is_review(i, &out_of_plan) && !i.status.is_done() && !theirs(i))
        .collect();
    let focus = held(issues, cfg, away);

    if focus.is_empty() {
        // 집은 것이 없으면 굴러가는 리뷰도 없다 — `focus` 가 곧 `wip` 이라,
        // 여기서 "굴러가는 리뷰" 를 다시 찾던 조건은 언제나 거짓이었다.
        // 굴러가는 리뷰가 있는 길은 아래 `anchored` 가 맡는다.
        if let Some(idle) = open.first() {
            return refuse(3, format!(
                "리뷰 이슈 {} 가 아직 안 집혔다. 리뷰를 시작하면 그 줄도 같이 움직인다.\n\
                 \x20 moai mv {} in_progress\n\
                 그 리뷰가 아니면 지금 보는 것을 먼저 집고 다시 부른다.",
                idle.id, idle.id
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

/// 거절한다. **어긴 규칙의 이름이 첫 줄이다** — 스킬이 적은 규칙 제목과 글자가
/// 같아야 막힌 쪽이 무엇을 어겼는지 한 번에 찾는다.
fn refuse(rule: usize, why: String) -> Decision {
    Decision::Deny(format!("{}\n{why}", crate::guide::rule_head(rule)))
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
    away: &BTreeSet<String>,
    warnings: usize,
    before: Option<usize>,
) -> Decision {
    let mut lines = Vec::new();
    let mut wip = held(issues, cfg, away);
    // **겹친 줄에서도 집혀 있는 것만 붙든다**(리뷰 moai-dw63.nzw). 트래커는 main 에서 쓰므로 딸린
    // 워크트리의 스냅샷은 갈라진 때에 멈춰 있다 — 거기서 `moai -C <루트>` 로 첫 칸에 되돌리거나 미룬
    // 줄을 제 스냅샷으로만 세면, 그 워크트리에 여는 세션마다 이미 놓은 것을 도로 놓으라고 붙들고,
    // 이미 미룬 줄에는 `mv` 로는 안 풀리는 "첫 칸에 두면 열린 채 남는다" 를 댔다(`mv` 는 미룸을
    // 안 푼다). **빼기만 한다** — 옆이 집은 줄을 제 초점에 더하지 않는다.
    if !wip.is_empty() {
        let still: BTreeSet<&str> = report::wip(latest, cfg).into_iter().map(|i| i.id.as_str()).collect();
        wip.retain(|i| still.contains(i.id.as_str()));
    }
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
                lines.push(crate::guide::close_steps(&i.id));
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
                    "  moai mv {} {} -m \"무엇을 기다리나\"      결정을 기다리는 것이면 — 첫 칸에 두면 {e} 가 열린 채 남는다",
                    i.id,
                    cfg.first_status()
                ));
            }
            let when = match shuts {
                Some(e) => format!("{e} 의 목적을 접을 때만 — 집은 것을 미루면 {e} 에 끝난 멤버만 남아 목적을 못 이룬 채 닫힌다"),
                None => "지금 안 할 것이면".to_string(),
            };
            lines.push(format!("  moai defer {} -m \"왜\"      {when}", i.id));
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
    let settled: BTreeSet<&str> = if unit.is_empty() {
        BTreeSet::new()
    } else {
        let out = report::put_off(latest);
        latest.iter().filter(|i| i.status.is_done() || out.contains(i.id.as_str())).map(|i| i.id.as_str()).collect()
    };
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
                crate::guide::close_steps(&i.id)
            ));
        }
    }
    if let Some(before) = before
        && warnings > before
    {
        lines.push(format!(
            "경고가 {before} 에서 {warnings} 로 늘었다. `moai status` 로 무엇이 늘었는지 본다."
        ));
    }
    if lines.is_empty() { Decision::Pass } else { Decision::Block(lines.join("\n")) }
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

/// 이 파일을 고치는 것이 일에 매여야 하는가.
fn counted(path: &str, root: &Path) -> bool {
    if path.is_empty() {
        return false;
    }
    let Ok(rel) = resolve(path, root).strip_prefix(root).map(Path::to_path_buf) else {
        return false; // 저장소 밖 — 스크래치패드·임시 파일·남의 저장소
    };
    rel.components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .is_some_and(|head| !SKIP.contains(&head))
}

/// **상대 경로는 저장소의 자리로 푼다.** 훅 프로세스가 어디서 도는지는 아무도
/// 약속하지 않았다 — `src/main.rs` 가 저장소 밖으로 보여 규칙이 통째로 샜다.
fn resolve(path: &str, root: &Path) -> PathBuf {
    let p = Path::new(path);
    let joined = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
    // **`..` 를 접는다.** 접지 않으면 판정이 양쪽으로 다 틀린다 —
    // `.moai/../src/store.rs` 는 첫 조각이 `.moai` 라 안 세는 자리로 보이고,
    // `../elsewhere/x.rs` 는 `strip_prefix` 가 그대로 붙어 저장소 안으로 보인다.
    // 파일이 아직 없을 수도 있으므로 디스크를 짚지 않고 글자로만 접는다.
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

fn rel_to(path: &str, root: &Path) -> String {
    resolve(path, root)
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
    fn here() -> BTreeSet<String> {
        BTreeSet::new()
    }

    fn away(ids: &[&str]) -> BTreeSet<String> {
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
        let why = denied(&guard_create(&all, &cfg(), &there, "moai add \"딴 일\"")).to_string();
        assert!(why.contains("t-2") && !why.contains("t-1"), "옆의 일을 초점으로 댄다\n{why}");
        let Decision::Block(why) = closing(&all, &all, &cfg(), &there, 0, None) else {
            panic!("여기서 집은 것을 안 붙든다");
        };
        assert!(why.contains("moai mv t-2") && !why.contains("t-1"), "옆의 일을 옮기라고 한다\n{why}");

        // 다 옆이 쥐었으면 여기서 집은 것이 없다.
        let both = away(&["t-1", "t-2"]);
        assert_eq!(guard_create(&all, &cfg(), &both, "moai add \"딴 일\""), Decision::Pass);
        assert_eq!(closing(&all, &all, &cfg(), &both, 0, None), Decision::Pass);
        assert_eq!(carried(&all, &cfg(), &both), Decision::Pass);
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
        let elsewhere = away(&["t-1", "t-2", "t-4"]);
        let got: Vec<String> = unsure(&all, &cfg(), &elsewhere, &away(&["t-e"])).into_iter().collect();
        assert_eq!(got, ["t-2"], "제 에픽의 일이나 안 집은 줄을 모른다고 했다");
        assert!(unsure(&all, &cfg(), &here(), &here()).is_empty());
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
            guard_shell_in(&mine, &cfg(), &here(), root, root, cmd, &|k| dirs[k].is_none())
        };
        assert_eq!(judge("moai -C /b add \"딴 일\""), Decision::Pass);
        assert_eq!(judge("cd /b && moai add \"딴 일\""), Decision::Pass);
        assert!(denied(&judge("moai -C /b add \"딴 일\" && moai add \"또\"")).contains("t-1"));

        // 가리킨 트래커가 쥔 것이 있으면 그 줄로 막는다.
        let theirs = vec![issue("t-9", "in_progress")];
        let cmd = "moai -C /b add \"딴 일\"";
        let dirs = aimed(cmd, root);
        let why = denied(&guard_moai(&theirs, &cfg(), &here(), cmd, &|k| dirs[k].is_some())).to_string();
        assert!(why.contains("t-9"), "{why}");
    }

    /// **에픽 일을 집은 채 생각을 담으면 둘째 물음을 비춘다 — 막지 않는다**(moai-d4e0). 거절문에만
    /// 실으면 실제로 틀리는 자리인 `idea add` 는 말없이 지나간다.
    #[test]
    fn setting_an_idea_aside_mid_epic_hears_the_second_question() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let root = Path::new("/repo");
        for cmd in ["moai idea add \"떠오른 것\"", "cd /repo && moai add --type idea \"떠오른 것\""] {
            let Decision::Context(said) = guard_shell(&all, &cfg(), &here(), root, root, cmd) else {
                panic!("안 비춘다 — {cmd}");
            };
            assert!(said.contains("t-e 가 내건 것"), "{said}");
            // **이 줄은 생각이 담긴 뒤에 읽힌다** — 새로 세우라고 하면 같은 것이 둘 선다(moai-dw63.e31).
            assert!(said.contains("moai idea promote <그 id> -e t-e --from -"), "담은 것을 되찾는 줄을 안 댄다\n{said}");
            assert!(!said.contains("moai add \"제목\""), "담긴 생각 곁에 같은 것을 또 세우라고 한다\n{said}");
        }
        // 담은 토막이 가리킨 자리도 댄다 — 빼고 치면 되찾는 줄이 세션 자리의 트래커에서 헛돈다.
        let Decision::Context(said) = guard_shell(&all, &cfg(), &here(), root, root, "moai -C /repo/sub idea add \"x\"")
        else {
            panic!("안 비춘다");
        };
        assert!(said.contains("moai -C /repo/sub idea promote <그 id> -e t-e"), "{said}");

        // 집은 것이 없거나, 에픽 없는 일이거나, 도움말이면 조용하다. **에픽 줄이 실제로 안 선
        // 참조**도 조용하다 — 닫힐 에픽이 없고, 그 id 로 되찾게 하면 경고가 하나 는다.
        let loose = vec![issue("t-1", "in_progress")];
        let idle = vec![epic("t-e"), under("t-1", "todo", "t-e")];
        let not_an_epic = vec![issue("t-x", "todo"), under("t-1", "in_progress", "t-x")];
        let dangling = vec![under("t-1", "in_progress", "t-gone")];
        for (all, cmd) in [
            (&loose, "moai idea add \"떠오른 것\""),
            (&idle, "moai idea add \"떠오른 것\""),
            (&not_an_epic, "moai idea add \"떠오른 것\""),
            (&dangling, "moai idea add \"떠오른 것\""),
            (&all, "moai idea add --help"),
            (&all, "moai idea ls"),
            (&all, "moai note t-1 \"idea add 를 적는다\""),
        ] {
            assert_eq!(guard_shell(all, &cfg(), &here(), root, root, cmd), Decision::Pass, "{cmd}");
        }

        // 에픽 둘을 쥐었으면 **집은 차례로** 댄다 — 규칙 1 의 거절문과 같은 에픽을 앞에 둔다.
        let two = vec![epic("t-z"), epic("t-a"), under("t-1", "in_progress", "t-z"), under("t-2", "in_progress", "t-a")];
        let Decision::Context(said) = guard_shell(&two, &cfg(), &here(), root, root, "moai idea add \"x\"") else {
            panic!("안 비춘다");
        };
        assert!(said.contains("t-z·t-a 가 내건 것") && said.contains("-e t-z --from -"), "{said}");
        let refused = guard_shell(&two, &cfg(), &here(), root, root, "moai add \"딴 일\"");
        assert!(denied(&refused).contains("-e t-z"), "두 글이 다른 에픽을 댄다\n{refused:?}");

        // **비추는 줄이 막는 것을 가리지 않는다** — 같은 명령줄의 규칙 1·3 이 먼저다.
        for cmd in ["moai idea add \"a\"; moai add \"딴 일\"", "moai idea add \"a\" && /code-review high"] {
            assert!(matches!(guard_shell(&all, &cfg(), &here(), root, root, cmd), Decision::Deny(_)), "{cmd}");
        }
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
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add \"딴 일\""), Decision::Pass);
    }

    /// 집은 것이 있으면 그 단위 안이어야 한다. 밖이면 고칠 명령이 함께 온다.
    #[test]
    fn outside_the_held_unit_is_refused_with_the_way_out() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let why = denied(&guard_create(&all, &cfg(), &here(), "moai add \"딴 일\"")).to_string();
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

        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add \"안의 일\" -e t-e"), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add \"자식\" --parent t-1"), Decision::Pass);

        // 선 에픽에 펼치는 promote 도 같은 자로 본다 — 안 보면 `idea add` 뒤 `promote -e` 가
        // `add -e` 가 막히는 자리를 지나간다 (moai-f3ml.lm7). 새 에픽을 세우는 promote 는 그대로다.
        let into = |e: &str| format!("moai idea promote t-i -e {e} --from -");
        assert!(matches!(guard_create(&all, &cfg(), &here(), &into("t-x")), Decision::Deny(_)), "남의 에픽에 promote 로 멤버를 세웠다");
        assert_eq!(guard_create(&all, &cfg(), &here(), &into("t-e")), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai idea promote t-i --epic=t-e --from -"), Decision::Pass);
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai idea promote t-i --from -"), Decision::Pass);
    }

    /// **소속은 물려받는다.** 자식 이슈를 집었을 때 그 줄의 `epic` 은 비어
    /// 있지만, 에픽은 부모에게서 온다. 필드만 읽던 판은 옳은 에픽을 댄
    /// 생성까지 거절했다 — 이 규칙을 만든 세션이 제 리뷰 결과를 못 적었다.
    #[test]
    fn the_held_unit_includes_what_was_inherited() {
        let all = vec![epic("t-e"), under("t-1", "todo", "t-e"), issue("t-1.aa", "in_progress")];
        assert_eq!(guard_create(&all, &cfg(), &here(), "moai add \"안의 일\" -e t-e"), Decision::Pass);
        let why = denied(&guard_create(&all, &cfg(), &here(), "moai add \"딴 일\"")).to_string();
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
            "moai idea add \"떠오른 것\"",
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
        assert!(matches!(guard_create(&all, &cfg(), &here(), "moai add \"idea 정리\""), Decision::Deny(_)));
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
            "moai show\nmoai add \"딴 일\"",
            "moai status\nmoai add \"딴 일\"\nmoai ready",
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
            "moai note t-1 -b - <<'MD'\nmoai add \"제목\" -e t-e 라고 일러 준다\nMD",
            "python3 - <<'PY'\nsubprocess.run([\"moai\", \"add\", \"제목\"])\nPY",
        ] {
            assert_eq!(guard_create(&all, &cfg(), &here(), free), Decision::Pass, "막혔다 — {free}");
        }
        // 환경변수를 앞세운 진짜 호출은 여전히 잡힌다.
        assert!(matches!(
            guard_create(&all, &cfg(), &here(), "MOAI_NOW=x moai add \"딴 일\""),
            Decision::Deny(_)
        ));
    }

    /// heredoc 이 닫힌 **뒤**는 다시 명령이다. 속을 건너뛴다고 뒤까지
    /// 놓치면, 글 한 덩이를 앞세우는 것이 그대로 우회로가 된다.
    #[test]
    fn what_follows_a_heredoc_is_a_command_again() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let cmd = "cat <<'MD' > /tmp/x\n아무 글\nMD\nmoai add \"딴 일\"";
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
            "moai epic add \"딴 에픽\"",
            "moai milestone add \"v0.2\"",
            "moai add \"딴 일\" --type epic",
            // 종류를 고정한 쪽이 이긴다 — 이것은 이슈를 만든다.
            "moai issue add \"딴 일\" --type idea",
            // 제목에 든 낱말은 플래그가 아니다.
            "moai add \"--type idea\"",
            // `--` 뒤는 제목이다 — 이슈 `--type=idea` 가 선다.
            "moai add -- --type=idea",
        ] {
            assert!(matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)), "샜다 — {cmd}");
        }
        // 담아 두는 것은 그 어느 철자로도 자유다.
        for cmd in [
            "moai idea add \"떠오른 것\"",
            "moai add \"떠오른 것\" --type idea",
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
            (1, guard_create(&held, &cfg(), &here(), "moai add \"딴 일\"")),
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
            "cd /repo && moai add \"딴 일\"",
            "true; moai add \"딴 일\"",
            "ls | grep x && moai add \"딴 일\"",
            "cd /repo\nmoai add \"딴 일\"",
        ] {
            assert!(
                matches!(guard_create(&all, &cfg(), &here(), cmd), Decision::Deny(_)),
                "지나갔다 — {cmd}"
            );
        }
        // 뒷토막이 담아 두는 것이면 그대로 지나간다.
        assert_eq!(
            guard_create(&all, &cfg(), &here(), "cd /repo && moai idea add \"떠오른 것\""),
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
        let why = denied(&guard_create(&loose, &cfg(), &here(), "moai add \"딴 일\"")).to_string();
        assert!(!why.contains("-e t-1"), "이슈를 에픽이라고 가리킨다\n{why}");
        assert!(!why.contains("-e "), "없는 에픽을 대라고 한다\n{why}");
        assert!(why.contains("--parent t-1"), "자식으로 다는 길이 없다\n{why}");
        assert!(why.contains("idea add"), "담아 두는 길이 없다\n{why}");
        // 에픽 없는 일의 자식은 부모를 안 붙든다 — 첫 칸에 두면 그 일이 안 닫힌다고 비치지 않는다.
        assert!(why.contains("idea 가 아니다"), "일이 이것 없이 안 끝나는 경우를 안 가른다\n{why}");
        assert!(!why.contains("첫 칸에 둔다"), "에픽 없는 일에 자식이 그 일을 열어 둔다고 비친다\n{why}");

        // 에픽이 있으면 그때는 에픽을 가리킨다.
        let held = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        let why = denied(&guard_create(&held, &cfg(), &here(), "moai add \"딴 일\"")).to_string();
        assert!(why.contains("-e t-e"), "{why}");
    }

    /// `--from` 도 **토큰으로** 본다. 글자로 찾으면 제목이 그 낱말을 담은
    /// `moai add "--from 을 나중에"` 가 규칙을 통째로 지나간다.
    #[test]
    fn from_is_a_token_not_a_substring() {
        let all = vec![epic("t-e"), under("t-1", "in_progress", "t-e")];
        assert!(matches!(
            guard_create(&all, &cfg(), &here(), "moai add \"--from 을 나중에 본다\""),
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
            (guard_create(&all, &cfg, &here(), "moai add \"딴 일\""), &all),
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
                "moai mv t-1 todo -m \"무엇을 기다리나\"      결정을 기다리는 것이면 — 첫 칸에 두면 t-e 가 열린 채 남는다"
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
            assert!(why.contains("moai defer t-1 -m \"왜\"      지금 안 할 것이면"), "{why}");
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
                    why.contains(&format!("moai defer {id} -m \"왜\"      t-e 의 목적을 접을 때만")),
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
        assert!(why.contains(&crate::guide::close_steps("t-r")), "{why}");
        assert!(!why.contains("moai mv t-r done     "), "규칙 3 이 막는 줄을 댄다\n{why}");
        assert!(why.contains("moai mv t-r review\n"), "앞 칸은 그대로 댄다\n{why}");
        // 보통 줄은 그대로다.
        assert!(why.contains("moai mv t-1 done     제목"), "{why}");
        let steps = crate::guide::close_steps("t-r");
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
        assert_eq!(carried(&all, &cfg(), &here()), Decision::Pass);
    }

    /// 집은 것은 id 와 제목으로 실린다 — 압축 뒤에 그것으로 다시 찾는다.
    #[test]
    fn what_is_held_survives_the_fold() {
        let all = vec![issue("t-1", "in_progress"), issue("t-2", "todo")];
        let Decision::Context(c) = carried(&all, &cfg(), &here()) else {
            panic!("집은 것이 안 실렸다");
        };
        assert!(c.contains("t-1"), "{c}");
        assert!(!c.contains("t-2"), "{c}");
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
        let cmd = "git commit -m $'don\\'t'\nmoai add \"딴 일\"";
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
        let swallowed = "echo \"a <<EOF b\"\nmoai add \"딴 일\"";
        assert!(matches!(guard_create(&held, &cfg(), &here(), swallowed), Decision::Deny(_)), "샜다 — {swallowed}");
        let quoted = "moai note t-1 -b - <<'MD'\n  MD\nmoai add \"제목\" 이라고 적는다\nMD";
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
        for cmd in ["(moai add \"딴 일\")", "{ moai add \"딴 일\"; }", "if true; then moai add \"딴 일\"; fi"] {
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
        assert_eq!(guard_create(&held, &cfg(), &here(), "moai add \"안의 일\" -et-e"), Decision::Pass);
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
            guard_shell_in(&idle, &cfg(), &here(), root, root, cmd, &|k| dirs[k].is_none())
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
        let focus = |all: &[Issue], names: &BTreeSet<String>| {
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
        for cmd in ["moai mv t-r done && /code-review high", "moai add \"딴 일\" && claude /code-review high"] {
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
        let steps = crate::guide::close_steps("t-r");
        assert!(steps.contains("moai note t-r -b -") && steps.contains("moai mv t-r done -m"), "{steps}");
        let why = denied(&guard_close(&all, &cfg(), &here(), "moai mv t-r done")).to_string();
        assert!(why.contains(&steps), "닫기 거절문이 갈라졌다\n{why}");

        let near = vec![epic("t-e"), under("t-1", "in_progress", "t-e"), review("t-r2", "todo", Some("t-e"))];
        let Decision::Block(held) = closing(&near, &near, &cfg(), &here(), 0, None) else {
            panic!("안 붙들었다");
        };
        assert!(held.contains(&crate::guide::close_steps("t-r2")), "세션 닫기가 갈라졌다\n{held}");
    }
}
