//! Claude 에 심을 것을 **글로 만든다.** 순수 함수다 — 파일도 안 쓰고 명령도
//! 안 부른다. 쓰는 일과 `claude` 를 부르는 일은 `cmd/skill.rs` 가 한다.
//!
//! ## 왜 플러그인인가
//!
//! 훅을 사람의 `settings.json` 에 직접 써 넣는 길도 있다. 그 길을 안 가는
//! 까닭은 하나다 — **JSON 에는 마커를 못 넣는다.** `init` 은 AGENTS.md 의
//! 마커 사이만 갈아 끼우고 사람이 쓴 산문은 한 글자도 안 건드리는데,
//! `settings.json` 에서는 그 약속을 못 지킨다. 다시 쓰는 순간 남의 서식이
//! 사라진다.
//!
//! 플러그인으로 심으면 훅이 **우리 디렉터리 안**에 있고, 사람의 설정에는
//! `claude` 가 제 손으로 두 키(`extraKnownMarketplaces`·`enabledPlugins`)만
//! 넣는다. 우리는 남의 JSON 을 만지지 않는다.
//!
//! **하나뿐인 예외**는 옛 판이 커밋된 `.claude/settings.json` 에 적은 한국어
//! 플러그인의 마켓플레이스 선언을 걷는 것이다(사용자 결정 moai-6ugu.aae). 그때도
//! 다시 짓지 않고 그 멤버의 줄만 도려내, 서식은 그대로 남는다([`drop_marketplace`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 심는 자리. 저장소 안이라 팀이 그대로 커밋할 수 있다.
pub const DIR: &str = ".claude/moai-plugin";

/// 심는 스킬의 이름 — 스킬마다 `skills/<이름>/` 디렉터리고 그 `SKILL.md` 머리의 `name:` 이다. 차례는 [`tree`] 가
/// 심는 차례(이슈 트래커·감독·위키)고, [`tree`] 는 디렉터리 이름을 이 목록에서 짓는다. 머리의 `name:` 은 글에 적혀
/// 있어 시험이 이 목록과 견주고(`guide::tests::the_frontmatter_opens_the_skill`), 트리가 이 목록 밖의 스킬을 심으면
/// `the_tree_plants_every_skill_name` 이 붉어진다.
///
/// **위키가 이 이름을 이슈 id 로 안 읽는다**(2026-10-04 사용자 결정, moai-mdzx.3pm) — `moai-wiki` 는 접두어 `moai`
/// 뒤 네 글자라 id 의 꼴이고, 페이지가 스킬을 이름으로 대면 없는 id 로 셌다(`wiki::parse`).
pub const NAMES: [&str; 3] = ["moai", "moai-supervise", "moai-wiki"];

/// Codex 와 Antigravity 가 **함께** 읽는 스킬 자리 — 저장소 뿌리부터의 상대다(moai-xs2h, 2026-10-04 사용자 결정).
/// 두 벤더 문서가 같은 `<저장소>/.agents/skills/<이름>/SKILL.md` 를 들어, 한 벌을 심으면 둘이 다 읽고 커밋돼 팀이
/// 받는다. 매니페스트도 등록도 없다 — 그 디렉터리를 스스로 훑는다. v0.7.0 에서는 이 저장소 안의 자리 하나뿐이고
/// 사용자 범위(`~/.agents/skills`)는 심지 않는다. 훅은 따로 선다(moai-u5wr).
pub const AGENTS_DIR: &str = ".agents/skills";

/// 심는 스킬 하나 — 이름([`NAMES`] 의 한 칸)과 그 디렉터리 안에 심을 글. 경로는 스킬 디렉터리부터의 상대고
/// 첫 글이 `SKILL.md` 다.
pub struct Skill {
    pub name: &'static str,
    pub files: Vec<(&'static str, String)>,
}

/// 심는 스킬 전부 — [`NAMES`] 의 차례로, 글은 `guide` 에서 온다. **트리 둘이 이 하나를 받는다**(moai-xs2h.xgo) —
/// Claude 의 플러그인([`tree`])과 Codex·Antigravity 의 [`AGENTS_DIR`]([`agents_tree`]). 부르는 자리마다 글을 손으로
/// 엮던 판은 `install` 과 커밋된 트리 시험이 같은 글 넷을 따로 늘어놓아, 스킬 하나를 더할 때마다 두 자리를 고쳤다.
pub fn skills() -> Vec<Skill> {
    // 디렉터리 이름은 [`NAMES`] 에서 온다 — 위키가 같은 목록으로 스킬 이름을 id 에서 거르니(moai-mdzx.3pm), 여기 글자를
    // 따로 적으면 이름을 바꿀 때 두 자리가 갈린다.
    let [main, supervisor, wiki] = NAMES;
    vec![
        Skill {
            name: main,
            files: vec![("SKILL.md", crate::guide::skill()), ("references/commands.md", crate::guide::reference())],
        },
        // 감독 스킬은 따로 선다 — `moai` 스킬에 섞으면 감독의 낱말에 `moai` 가 불려 오고, 일꾼이 `moai` 를 부를 때마다
        // 감독의 걸음까지 읽는다. 발동어(description)는 따로 서도 모든 세션에 실리므로, 나눈 것이 그 값을 아끼지는 않는다.
        Skill { name: supervisor, files: vec![("SKILL.md", crate::guide::supervise())] },
        // 위키 스킬도 따로 선다 — 부르는 자리가 에픽 끝(브리프 7-4)과 사람이 청한 훑기라, `moai` 스킬에 섞으면 이슈
        // 하나 세울 때마다 매뉴얼 쓰는 걸음까지 읽는다(moai-bl3x).
        Skill { name: wiki, files: vec![("SKILL.md", crate::guide::wiki())] },
    ]
}

/// 스킬마다의 글을 `<이름>/<상대 경로>` 로 편다 — 두 트리가 이 차례 그대로 받는다.
fn skill_files(skills: &[Skill]) -> impl Iterator<Item = (PathBuf, String)> + '_ {
    skills.iter().flat_map(|s| s.files.iter().map(move |(rel, body)| (Path::new(s.name).join(rel), body.clone())))
}

/// [`AGENTS_DIR`] 에 심을 파일들. 경로는 그 자리부터의 상대다. **Claude 의 트리와 글이 같다** — 다른 것은 매니페스트가
/// 없다는 것뿐이고, 에이전트마다 다른 걸음은 글 안의 낱말표(`guide::VERBS`)가 열로 가른다(사용자 결정 2026-10-04).
pub fn agents_tree(skills: &[Skill]) -> Vec<(PathBuf, String)> {
    skill_files(skills).collect()
}

/// 마켓플레이스 이름. `claude plugin install moai@<이것>` 의 뒷부분이다.
///
/// **저장소마다 달라야 한다.** 이름은 기계 하나에서 전역이라, 고정 이름을 쓰면
/// 둘째 저장소가 `install` 할 때 첫째 저장소의 트리를 가져간다 — 실제로 시험
/// 저장소가 이 저장소의 플러그인을 설치하는 것을 보고 알았다. 조용히 엉뚱한
/// 규칙이 걸리는 쪽이라 눈치채기도 어렵다.
///
/// 접두어에 **저장소 자리**를 섞는다. 접두어만으로는 모자란다 — 접두어는
/// 디렉터리 이름에서 나오므로 `~/work/api` 와 `~/old/api` 가 같은 것을 쓴다.
/// 그때 뒤에 심은 쪽이 등록에 실패하고, 실패한 자리에서 할 수 있는 일이
/// 없다(접두어는 못 바꾼다). 자리를 섞으면 그 막다른 길이 사라진다.
pub fn market(prefix: &str, root: &Path) -> String {
    format!("moai-{prefix}-{:04x}", stable(root.to_string_lossy().as_bytes()) % 0x1_0000)
}

/// 손으로 적은 FNV-1a 64비트([`crate::text::fnv1a64`], moai-2vrw). **`DefaultHasher` 를 쓰지
/// 않는다** — 그 알고리즘은 rustc 판 사이에 바뀌어도 된다고 문서가 밝혀 두었다. 이름과 판이
/// 그것에 기대면 컴파일러를 올린 날 이름이 바뀌고, 옛 등록은 `지우지 않는다` 는 약속 때문에
/// 그대로 남아 훅이 두 벌 돈다 — 보드도 거절문도 두 번이다.
///
/// **너비를 줄이지 않는다.** 값이 바뀌면 이미 심긴 플러그인이 모두 판이 달라진 것으로 보인다.
fn stable(bytes: &[u8]) -> u64 {
    crate::text::fnv1a64(bytes)
}

/// 훅이 걸리는 자리와 그때 부를 이벤트.
///
/// **`SessionStart` 는 보드를 안 싣는다** — 재개에서 그 출력이 대화에 안 붙는
/// 것을 여러 번 확인했다. 접힌 뒤에만 집고 있던 것을 싣는다. **`PreCompact`
/// 는 걸지 않는다** — `claude` 가 그 출력을 거절한다. 까닭은 `hook::Event` 에
/// 적혀 있다.
const HOOKS: &[(&str, &str, &str)] = &[
    ("SessionStart", "session-start", "counting moai warnings..."),
    ("UserPromptSubmit", "user-prompt-submit", "reading the moai board..."),
    ("PreToolUse", "pre-tool-use", "checking the moai rules..."),
    ("Stop", "stop", "comparing the moai state..."),
];

/// `PreToolUse` 가 볼 도구들. 규칙이 뜻을 두는 것만 적는다 — 전부 받으면
/// 읽기만 하는 호출까지 훅을 한 번씩 띄운다.
const WATCHED: &str = "Bash|Edit|Write|NotebookEdit|Skill";

/// 훅이 부를 명령.
///
/// **없으면 조용히 0 이다.** `cargo clean` 한 번이면 바이너리가 사라지는데,
/// 그때 훅이 "command not found" 를 매 세션 뱉으면 사람이 훅을 꺼 버린다 —
/// 꺼진 규칙은 없는 규칙이다. 한 번은 이 가드가 없어 세션 하나가 통째로
/// 잠겼다: 도구 호출마다 훅이 실패해 `Bash` 도 `Write` 도 안 돌았다.
///
/// **있는데 못 도는 판은 한 줄을 낸다**(moai-j4ie, 2026-09-23 사용자 결정). 옛 한 줄은
/// `… && "<exe>" hook <event> || exit 0` 이라 **126(못 돌린다)까지 삼켰다** — 실행 비트가
/// 빠졌거나 `noexec` 에 얹힌 판에서 규칙 넷이 조용히 안 서는데, 그 모습은 "규칙이 통과했다"
/// 와 한 글자도 다르지 않았다. `moai skill` 은 그 판을 이제 "안 돈다" 고 말하지만
/// (`cmd::runnable`, moai-dhx9) 그 말을 읽는 사람이 없는 판은 여전히 조용하다.
///
/// **말하는 길은 `systemMessage` 뿐이다 — 재서 골랐다.** 훅 하나에 네 갈래를 심어
/// `claude -p --output-format stream-json` 으로 재니(2026-09-23, haiku, Bash 한 번):
///
/// | 훅의 종료 | stderr | 세션이 듣는 것 | 도구 |
/// |---|---|---|---|
/// | 0 | 있음 | **아무것도** — 스트림에 없다 | 돈다 |
/// | 1 | 있음 | **아무것도** — 0 과 같다 (`--debug` 로도 없다) | 돈다 |
/// | 2 | 있음 | `tool_result` 에 그 줄 그대로 | **안 돈다** |
/// | 0 + stdout `{"systemMessage":…}` | — | `system/informational` notice 한 줄 | 돈다 |
///
/// 그래서 **비영 종료로는 아무도 못 듣는다** — 들리는 비영 값은 2 하나고 그것은 게이트다
/// (CLAUDE.md: 경고로 비영 종료하지 않는다, 훅은 정당한 쓰기를 막지 않는다). `systemMessage`
/// 는 게이트 없이 들리는 유일한 길이라 그것을 쓴다. `SessionStart` 에서만 notice 가 안 서고
/// `hook_response` 에 남는다(재 본 값) — 그 한 줄을 이벤트마다 달리 적지는 않는다.
///
/// **0 과 1 말고는 다 말한다**(moai-wnnb) — **다만 stdout 이 판정의 꼴(`{…}`)이 아닐 때만이다**
/// (moai-mnhq 가 "비었을 때" 로 좁혔고 moai-45hf.3do 가 꼴로 고쳤다, 아래 "판정을 쓴 뒤에 오는 값" 과
/// "0·1 밖에서는" 문단). 1 로 진 판은 그대로 삼킨다(같은 결정) — `moai` 가
/// 돌기는 했으므로 stdout 에 판정 JSON 이 이미 섰을 수 있고([`crate::cmd::had_partial`] 이 값을
/// 내는 길), 그 뒤에 둘째 객체를 붙이면 그 판정 — `deny` 까지 — 이 파싱에서 통째로 버려진다.
/// 막아야 할 쓰기가 통과하는 쪽으로 지는 것이다.
///
/// **그 밖의 값은 `moai` 가 제 손으로 낼 수 없는 값이다.** `main` 이 내는 것은
/// `ExitCode::SUCCESS` 와 `ExitCode::FAILURE` 둘뿐이라, 0·1 이 아닌 값은 판정을 못 낸
/// 판이다 — 126·127(껍데기가 exec 을 거절했다), clap 이 모르는 부명령에 내는 2(`claude` 는
/// 제 캐시의 `plugin.json` 으로 훅을 부르므로 옛 바이너리가 거절하는 이벤트 이름이 실제로
/// 설 수 있다), 패닉의 101, 시그널의 128+N 이다. 한때 126·127 만 말하던 판은 나머지를
/// 옛날 그대로 조용히 두었고, 그 화면은 규칙이 통과한 것과 한 글자도 다르지 않았다.
/// 126·127 을 "못 돌렸다" 로 읽는 어휘는 `merge_driver::Probe::Dead` 와 같다.
///
/// **판정을 쓴 뒤에 오는 값은 stdout 을 받아서 가른다**(moai-mnhq, 2026-09-23 사용자 결정).
/// 126·127·2 는 `moai` 가 stdout 에 한 바이트도 못 쓴 자리가 맞지만, 패닉의 101 과 시그널의
/// 128+N 은 `main` 이 판정을 이미 흘려보낸 **뒤**에도 온다. 그 자리에서 둘째 객체를 붙이면 위
/// 문단이 1 에 대해 적어 둔 손실이 그대로 난다 — 짐작이 아니라 **진짜 `claude` 로 쟀다**
/// (2026-09-23, haiku): `deny` 하나만 내면 도구가 안 돌고 거절문이 그대로 서는데, 그 뒤에
/// `systemMessage` 를 하나 붙이면 **두 객체가 다 없어지고 도구가 돈다**(`is_error` 가 거짓이고
/// 스트림 어디에도 두 글이 없다). 그래서 이 줄은 `o=$(…)` 로 stdout 을 받아 두고 알림은 그것이
/// **비었을 때만** 냈다 — 지금은 판정의 꼴이 아닐 때다(바로 아래 문단). 받은 것은 `printf '%s\n'`
/// 으로 그대로 흘려보낸다.
///
/// **0·1 밖에서는 "비었는가" 가 아니라 "판정의 꼴인가" 를 본다**(moai-45hf.3do, 2026-09-29 사용자
/// 결정). 비었는지는 "판정을 냈다" 의 대리값일 뿐이고, 둘이 갈리는 자리가 셋 있었다(리뷰
/// moai-514e.0er 6·7번, 2026-09-23 에 쟀다). PIPE_BUF 를 넘는 줄을 쓰다 SIGKILL 을 맞으면 `$o` 가
/// `{"hookSpecificOutp` 로 남고, [`exe_name`] 이 이름으로 물러선 줄에서 PATH 의 `moai`(이 기계에서는
/// 옛 프로젝트의 것)는 사용법을 찍고 64 로 지며, 빈칸 하나만 낸 때도 있다. 셋 다 비지 않았으니 토막이
/// 그대로 건너가고 알림이 접혀, `claude` 는 못 읽고 쓰기는 돌았다 — **알림도 표식도 없이 영영
/// 조용했다.** 이제 `{` 로 시작해 `}` 로 끝나지 않는 글은 버리고 알림을 낸다. 둘째 객체가 붙는 일이
/// 없으니 위의 손실은 안 난다. 0·1 은 `moai` 가 스스로 끝낸 값이라 그대로 흘려보낸다.
///
/// **그 꼴은 대리값이라 0·1 밖에서는 표식을 안 세운다**(리뷰 moai-45hf.nab). 잘린 자리가 마침 `}`
/// 뒤면(안쪽 객체의 `}`, 글 속의 `}`) 토막이 꼴을 지나 건너가는데, 쪽지는 `moai` 가 흘려보내기 **전에**
/// 적으므로 그 판정으로 보드 표식까지 서면 그 세션은 온전한 보드를 영영 못 받는다 — wqg 가 막으려던
/// 바로 그 손실이다(2026-09-29 에 파이프를 채운 채 SIGKILL 을 맞혀 측정해 보니 65,537B 가 그대로 건너가고
/// 표식이 섰다). 온전한 판정 뒤에 죽은 드문 경우는 보드가 한 번 더 실릴 뿐이다.
///
/// **그 가름에 드는 비용은 파이프 하나다 — 프로세스는 안 는다**(리뷰 moai-514e.0er 가 고쳤다).
/// `o=$(단순 명령)` 은 옛 줄이 이미 띄우던 그 자식 하나에서 그대로 exec 한다. `strace -f -c` 로
/// 세니(2026-09-23) dash 는 앞뒤 다 자식 하나(`vfork`)에 `pipe2` 만 0 → 1 이고, bash 는
/// `clone` 310 회로 같고 `pipe2` 가 232 → 233 이다. 잰 +0.30ms(dash)·+0.48ms(bash) 는
/// (`/bin/true` 로 2,000번씩 두 번) 그 파이프와 EOF 까지 읽어 변수에 담는 비용이다. 이 저장소의
/// 훅 한 번은 `moai` 자신이 28ms 를 쓰므로(`pre-tool-use` 30회 평균) 그 1.1~1.7% 다.
/// 아래 "정상 갈래" 문단의 **프로세스를 안 늘린다는 말은 그대로 선다.**
///
/// **그 +0.30ms 는 바닥값이다** — `/bin/true` 는 0바이트를 낸다. 껍데기가 바이트를 읽어 다시
/// 쓰므로 비용이 출력 크기를 따라 는다: dash 로 60번씩 재니(2026-09-23) 0B 와 17.5KB 는
/// +0.5ms 언저리인데 1MB 는 +36ms 였다. 지금 가장 큰 출력은 보드(`user-prompt-submit`, 17KB)라
/// 위 셈이 그대로 서지만, **보드에는 길이 상한이 없다** — 이슈와 경고가 늘면 이 값도 같이 는다.
/// 재려면 `/bin/true` 가 아니라 그때의 보드로 잰다.
///
/// **그 대신 이 줄이 EOF 를 기다린다.** 명령 치환은 `moai` 가 끝날 때가 아니라 **쓰기 끝을 쥔
/// 것이 다 닫힐 때** 끝난다 — `cmd::merge_driver` 의 `Said` 가 파이프를 버리고 파일로
/// 간 까닭이 그것이다(손자가 살아 있으면 읽기가 안 끝난다). 지금은 `moai` 의 자식이 모두
/// `Stdio::piped()`·`Stdio::from(파일)` 이라 fd 1 을 물려받는 것이 없어 깨끗하다. **그 성질에
/// 기대고 있다** — `Stdio::inherit()` 을 쓰는 자식이 하나라도 생기면 이 줄이 매니페스트
/// `timeout` 까지 멈춘다.
///
/// **꼬리의 줄바꿈은 골라진다.** 명령 치환이 끝의 줄바꿈을 **몇 개든 다** 걷고 이 줄이 하나를
/// 다시 단다. `moai` 의 출력은 늘 줄바꿈 하나로 끝나 바이트가 안 달라지고(2026-09-23 에 쟀다:
/// 보드를 싣는 `user-prompt-submit` 이 받기 전후 17,371B 로 같다), 줄바꿈 없이 끝나는 출력만
/// 하나를 얻는다 — JSON 줄에는 뜻이 안 달라진다. **그대로 흘려보낸다고 적을 수 없는 자리가
/// 둘이다**(리뷰 moai-514e.0er): 줄바꿈 둘로 끝나는 출력은 하나로 줄고(시험의 `blank` 가
/// 잰다), NUL 은 통째로 버려진다(bash 는 그때 제 경고를 stderr 에 한 줄 얹는다). `moai` 의
/// JSON 은 U+0000 을 `\u0000` 으로 쓰므로 지금은 둘 다 안 닿는다.
///
/// **`moai` 쪽도 한 자리를 옮겼다**(같은 결정). `src/main.rs` 는 이제
/// `carried`·`unjournaled`·`unread_journals`·`redirected` 를 먼저 돌리고 `print(&lines)` 를 맨
/// 뒤에 둔다 — 넷 다 stderr 에만 쓰므로 stdout 의 차례는 안 바뀌고, 판정을 흘려보낸 뒤에 101 이
/// 올 자리가 그 넷뿐이었으니 이제 **101 은 stdout 이 비었다는 뜻**이다. 셸 한 줄의 가정이 아니라
/// `moai` 의 성질로 선다. 시그널은 그래도 아무 때나 오므로 그쪽은 위의 받아 두는 갈래가 맡는다.
///
/// **이 줄이 못 잡는 판이 하나 있다 — 매니페스트 `timeout` 15초에 끊긴 판이다.** 2026-09-23 에
/// 쟀다: `claude` 는 그 시각에 훅을 **프로세스 그룹째** 죽이고(하위 프로세스로 돌린 `sleep` 이
/// 남지 않았다) 세션에는 아무 말도 안 간다(`stream-json` 에 줄이 없다). 껍데기가 함께 죽으니
/// `case` 뒤가 아예 안 돈다. 잡으려면 도장을 남기고 다음 번이 줍는 새 장치가 드는데, 훅은 병렬로
/// 돌아(같은 날 측정값: 도구 호출 셋의 훅이 겹쳐 섰다) 그 도장이 살아 있는 이웃의 것과
/// 안 갈린다 — **2026-09-23 사용자 결정으로 셸 한 줄만 넓혔다.** 다시 열 때는 그 겹침부터 잰다.
///
/// **받아 두는 갈래가 그 자리에서 잃는 것을 늘렸다**(리뷰 moai-514e.0er). 옛 줄은 `moai` 의 fd 1
/// 이 곧 `claude` 의 파이프라, 죽기 전에 쓴 바이트는 이미 건너가 있었다. 이 줄은 그것을 `$o` 에
/// 들고 있다가 `moai` 가 끝난 **뒤에** 흘려보내므로, 그룹째 죽는 때에는 판정까지 통째로
/// 사라진다(같은 날의 측정값: 옛 줄 53B, 이 줄 0B). `src/main.rs` 가 `print(&lines)` 를 맨 뒤로
/// 옮겨 그 틈이 마이크로초라는 것이 지금 서는 근거고, **그 틈은 0 이 아니다** — 여기서 지는
/// 쪽은 막아야 할 쓰기가 통과하는 쪽이다. 도장을 다시 볼 때 이것도 함께 잰다.
///
/// **그 틈에서 표식까지 잃지는 않는다**(moai-45hf.wqg, 2026-09-29 사용자 결정). 보드와 `Stop` 의
/// 표식을 `moai` 안에서 세우던 판은, 판정이 셸에 묶인 채 죽으면 표식만 남아 그 세션이 보드를
/// **영영** 못 받았다(판정은 다음 도구 호출이 다시 셈하지만 보드는 아니다). 이제 이 줄이 쪽지
/// 자리 `h` 를 [`crate::cmd::hook::HANDOFF`] 로 넘기고, `moai` 는 표식 이름을 거기에 적기만 하며,
/// 이 줄이 `printf` 에 **이긴 뒤에** 그 이름으로 표식을 세운다. 쪽지 이름에 이 셸의 `$$` 가 들어
/// 겹쳐 도는 훅이 안 섞이고, 제 것(`-O`)이 아니거나 링크(`-h`)인 쪽지는 안 읽는다 — 남이 먼저
/// 둔 쪽지를 읽으면 그 이름의 파일을 `true >` 로 세우게 된다. **이미 무엇이 선 자리는 열지도 않는다**
/// (`! [ -e ]`·`! [ -h ]` 뒤의 `set -C`) — 까닭은 아래 알림 표식 문단의 이름 있는 파이프(FIFO)다. 드는
/// 것은 내장뿐이라 정상 갈래의 프로세스는 그대로다. **`moai` 가 쪽지를 못 쓰면 표식은 여전히 `moai`
/// 안에서 선다** — 비었거나 상대 경로인 `TMPDIR`, 남이 먼저 잡은 쪽지 자리가 그렇고, 그 경우 이 틈의
/// 손실도 옛날 그대로다(목록은 `cmd::hook` 의 `once_per_session` 이 든다).
///
/// **`printf` 가 SIGPIPE 에 맞아도 0 으로 나간다**(리뷰 moai-514e.0er). 옛 줄에서는 fd 1 을
/// `moai` 가 쥐었고 `src/main.rs` 의 `outln!` 이 `BrokenPipe` 를 삼켜 0 으로 끝났다. 이 줄은
/// **껍데기가** 그 fd 에 쓰므로 받는 쪽이 먼저 닫으면 껍데기가 시그널에 죽어 **141** 로 나간다
/// (2026-09-23 에 dash·bash 둘 다 그랬다. 옛 줄은 같은 자리에서 0 이었다). 아래 시험이 못박은
/// "종료 코드는 어느 갈래에서도 0" 이 그 한 값으로 깨지고, 훅의 비영 종료가 무엇을 하는지는
/// 위 표에 2 하나만 잰 채다. 그래서 `trap 'exit 0' PIPE` 를 둔다 — 처리기를 단 시그널은 exec 에서
/// 기본값으로 돌아가므로(`SIG_IGN` 과 다르다) `moai` 와 `command -v` 는 그대로다.
///
/// **시그널이 안 오는 실패는 `|| :` 가 맡는다**(리뷰 moai-514e.0er). `trap` 이 잡는 것은
/// SIGPIPE 뿐이고, 닫힌 fd(`>&-`)·읽기 전용 fd·`ENOSPC` 는 `printf` 를 그냥 비영으로 끝낸다.
/// 2026-09-23 에 `printf` 가 목록의 **마지막 자리**(`set -e` 가 면제해 주지 않는 딱 한 자리)에
/// 섰던 줄은 거기서 죽어 `exit 0` 에 못 닿았다 — sh·bash·dash 셋 다 1 로 나갔고 **옛 줄은 같은
/// 자리에서 0 이었다**(옛 줄에는 치환 뒤에 질 수 있는 맨 명령이 없었다). **지금 그 자리에 서는
/// 것은 흘려보내기와 표식 세우기를 묶은 `{ … }` 전체다**(리뷰 moai-45hf.nab) — `printf` 가 지거나
/// 표식이 못 서면(그 디렉터리가 없다) 그 묶음이 비영이고, `|| :` 를 걷으면 dash 는 `-ec` 에서
/// 2 로 나간다. 그 2 가 곧 게이트다. 알림의 `printf` 에도 같이 단다. 그쪽은 옛 줄에도 있던 자리지만,
/// 위 표대로 훅의 비영 종료는 2 하나만 측정해 본 채라 값을 흘리지 않는 편이 싸다.
///
/// **그 `trap` 은 맨 앞이 아니라 있는지 보는 문 바로 뒤에 선다.** [`hook_exe`] 가 이 줄의
/// **머리**(`command -v -- "`)를 글자로 떼어 훅이 부르는 파일을 읽는다 — 앞에 한 마디라도
/// 끼우면 `moai skill status` 가 그 자리를 못 대고, 이 줄을 떼어 다시 쓰는 `MOAI_BLESS` 갈래도
/// 함께 먹통이 된다(시험 `the_hook_exe_round_trips_through_the_manifest` 가 그 자리를 잡는다).
/// 그 문까지는 stdout 에 한 바이트도 안 나가므로 SIGPIPE 가 설 자리도 없다.
///
/// **경로를 그 줄에 적는다**(moai-wza7, 2026-09-23 사용자 결정). 한때 뺐던 까닭은 경로의
/// 제어문자 하나가 JSON 문자열을 깨고 이 한 줄이 통째로 버려지는 것이었다. 그 자([`sayable`])를
/// 따로 두었으므로 이제 실을 수 있는 경로는 실리고, 못 도는 바이너리가 **어느 파일인지**가 그
/// 한 줄에 선다 — 알림이 대는 `moai skill status` 는 방금 exec 에 실패한 그 바이너리라 그
/// 판에서는 같이 못 돈다.
///
/// **못 싣는 경로에서는 자리만 빠진다**(리뷰 moai-514e.hgz 5번). 그 자를 [`quotable`] 과 한
/// 통에 두던 판은 셸에 멀쩡한 경로(탭이 든 디렉터리)까지 이름으로 바꿔 적어, 훅이 **딴
/// 바이너리를 부르게** 했다 — 이 기계의 PATH 의 `moai` 는 옛 moai 의 것이다. 부르는 자와
/// 말하는 자를 가른다: 경로는 그대로 불리고, 알림에서는 그 토막만 빠진다.
///
/// **그래도 `moai skill status` 는 남긴다.** 126 의 까닭은 실행 비트만이 아니라 `noexec` 으로
/// 얹힌 자리와 엉뚱한 아키텍처도 있어 `chmod +x` 를 단정하면 틀린 처방이 되고, PATH 에 도는
/// moai 가 있는 기계에서는 그 말이 그대로 답을 낸다. 그 말을 그대로 쳐서 답이 나와야 하므로
/// 부명령까지 적는다 (맨 `moai skill` 은 clap 이 2 로 거절한다, 리뷰 moai-j4ie).
///
/// **그 줄은 세션마다 한 번만 선다**(moai-f7up, 2026-09-23 사용자 결정). 문턱이 없던 판은 같은
/// 줄을 `PreToolUse` 의 도구마다·프롬프트마다·`Stop` 마다 다시 냈다 — 이 저장소의 세션
/// 216개를 세니 훅이 걸리는 도구 호출이 **한 세션 평균 140번, 가장 많은 세션은 1,257번**이다
/// (2026-09-23). 훅이 매 호출에 떠들면 사람이 훅을 꺼 버리고, 꺼진 규칙은 없는 규칙이다.
///
/// 문턱은 `${TMPDIR:-/tmp}` 의 표식 파일 하나고, 키는 **세션·바이너리 철자·이벤트·종료 값** 넷이다.
/// **이벤트를 뺐던 판은 `SessionStart` 가 그 한 줄을 태웠다**(리뷰 moai-514e.hgz) — 위 표대로
/// 그 이벤트에서만 notice 가 안 서는데 그것이 세션의 맨 앞에서 돌아, 사람이 못 듣는 알림 하나가
/// 표식을 세우고 규칙 넷을 싣는 `PreToolUse` 의 입을 세션 내내 막았다. **종료 값을 뺐던 판은
/// 한 번짜리 죽음이 그 줄을 태웠다**(같은 리뷰) — OOM 의 137 하나가 뒤에 정말 온 126 을
/// 세션 내내 조용하게 했다. 둘 다 `cmd::hook` 의 `session_file` 이 `what` 을 키에 넣은 것과
/// 같은 까닭이고, 값이 달라질 때만 다시 말하니 상황이 정말 바뀐 자리에서만 한 줄이 는다.
/// `cmd::hook` 의 [`once_per_session`](crate::cmd::hook) 은 이 갈래에서 못 쓴다 — 거기서는
/// `moai` 가 아예 안 돌았다. 세션 id 는 훅 입력의 stdin 에 있어 셸 한 줄이 읽으려면 파서가
/// 드는데, **`claude` 가 그것을 환경에도 세운다**(2026-09-23 에 쟀다: 훅이 보는 환경에
/// `CLAUDE_CODE_SESSION_ID` 가 stdin 의 `session_id` 와 같은 값으로 섰고, `$PPID` 는 그
/// `claude` 프로세스였다). 그래서 stdin 을 안 건드린다 — 읽으면 `moai` 가 받을 입력이 사라진다.
/// 그 변수가 없는 판을 위해 `$PPID` 로 물러선다: 창이 하나면 그 값도 세션마다 하나다.
///
/// **표식은 `:` 가 아니라 `true` 로 세운다.** `:` 는 **특수 내장**이라, 리다이렉션이 실패하면
/// POSIX 껍데기가 그 자리에서 죽는다 — dash 로 재 보니(2026-09-23) 못 쓰는 `TMPDIR` 에서
/// `: > "$s" 2>/dev/null || :` 가 `|| :` 에 닿지도 못하고 **종료 2** 로 나갔다. 위 표대로 2 는
/// 세션이 듣는 유일한 값이고 **그것은 게이트다** — 쓸 수 없는 임시 디렉터리 하나가 모든 도구
/// 호출을 막는 자리였다. `true` 는 평범한 내장이라 같은 판에서 그대로 이어 간다(bash 는 둘 다
/// 이어 가므로 bash 로만 재면 안 보인다).
///
/// **표식은 `set -C` 로 한 번에 세우고, 못 세우면 그 자리가 제 것인지 본다**(moai-45hf.6z2,
/// 2026-09-29 사용자 결정, 리뷰 moai-514e.hgz 13·14번). `[ -e ]` 로 보고 세우던 옛 줄은 원자적이지
/// 않아, 겹쳐 도는 훅(2026-09-23 에 쟀다)이 저마다 "없다" 를 보고 저마다 말했다. 그리고 이름의
/// 네 글자는 `plugin.json` 에 커밋돼 있고 `$PPID` 로 물러선 경우 나머지는 작은 정수라, 같은 기계의
/// 남이 그 자리에 파일 하나를 먼저 두면 그 세션의 알림이 **통째로** 막혔다. 이제 `set -C`(곧
/// `O_EXCL`)가 이긴 하나만 말하고, 진 쪽은 그 자리가 제 것(`-O`)인 보통 파일(`-f`)이고 링크(`-h`)가
/// 아닐 때만 조용하다. 남이 잡은 자리와 못 쓰는 자리에서는 매번 말한다 — 조용히 막히는 쪽이 아니라
/// 시끄러워지는 쪽으로 진다. 자리를 사용자 설정 옆으로 옮기는 길도 있었지만, 셸이 `MOAI_CONFIG`·
/// XDG·`HOME` 풀이를 흉내 내야 하고 `mkdir -p`(프로세스 하나)가 들며 표식이 영영 쌓인다.
///
/// **`set -C` 는 있는 보통 파일만 거절한다 — 그래서 없는 자리에서만 연다**(리뷰 moai-45hf.nab).
/// 이름 있는 파이프(FIFO)나 그것을 가리키는 링크가 서 있으면 셸은 `O_EXCL` 없이 쓰기로 열고, 읽는
/// 쪽이 올 때까지 멈춘다. 2026-09-29 에 sh·bash·dash 셋 다 그렇게 멈췄고, 훅이면 매니페스트
/// `timeout` 15초까지 서 있다가 `claude` 가 그 출력째 버린다 — 도구 호출마다 15초에 알림은 없다.
/// dash(이 기계의 `/bin/sh`)는 그때 `O_CREAT` 없이 열어 `fs.protected_fifos` 도 못 막는다. 옛 줄의
/// `[ -e ]` 는 그 자리에서 조용했을 뿐 멈추지는 않았다. 이제 `! [ -e ]`·`! [ -h ]` 로 빈 자리인지 먼저
/// 보고 세운다 — 세우기는 여전히 `set -C` 라 겹쳐 도는 훅 가운데 하나만 이긴다. 둘 사이의 틈은
/// 남는데, 그 틈에 파이프를 끼우는 것은 경합이라 미리 만들어 두는 것으로는 안 된다.
///
/// **`2>/dev/null` 은 `>` 보다 앞에 선다**(리뷰 moai-514e.hgz). 리다이렉션은 왼쪽부터 걸리므로
/// `true > "$s" 2>/dev/null` 은 stderr 를 돌리기 **전에** 껍데기가 제 진단을 원래 stderr 로
/// 이미 뱉는다 — 2026-09-23 에 sh·dash·bash 셋 다 `cannot create …` 를 냈고, 순서를 바꾼
/// `true 2>/dev/null > "$s"` 는 셋 다 조용했다. 표식이 못 서는 자리에서는 그 줄이 도구 호출마다
/// 서니, 조용히 넘어가려던 자리가 도리어 가장 시끄러운 자리가 된다.
///
/// **`set -C` 뒤의 그 돌림은 `2>>` 다**(리뷰 moai-45hf.nab). `set -C` 는 줄 끝까지 서 있어 뒤의
/// `2>/dev/null` 도 noclobber 리다이렉션이 된다 — `/dev/null` 이 보통 파일로 바뀐 컨테이너에서는 그
/// 돌림부터 `File exists` 로 져, 쪽지를 못 읽어 보드가 매 프롬프트에 실리고 알림이 매번 선다. `>>` 는
/// noclobber 가 안 닿고, 문자 장치인 `/dev/null` 에서는 `>` 와 같다.
///
/// **정상 갈래에는 프로세스가 안 는다** — 표식과 알림의 `printf` 는 `case` 안에서만 서고
/// (2026-09-23 에 dash 로 200번씩 재어 한 번 2.66ms → 2.74ms, 껍데기 하나를 새로 실행하는
/// 비용에 묻힌다), 거기에 stdout 을 받는 비용이 얹힌다(위 문단의 파이프 하나, +0.30~0.48ms).
/// 표식이 못 서는 자리(`TMPDIR` 이 못 쓰는 자리, 세션 id 에 `/` 가 든 판)에서는 그대로 매번
/// 말한다 — 문턱이 조용히 실패해 알림을 **잃는** 것보다 낫다. `$PPID` 로 물러선 판에서 pid 가
/// 재사용되면 표식이 남아 한 번을 잃는데, 그 자리는 `TMPDIR` 을 비우는 손이 함께 지운다.
///
/// **이벤트 이름도 `printf` 의 꼴에 안 넣는다** — 꼴 안의 `%` 는 변환 문자로 읽힌다. 인자
/// 자리로 넘기고 [`crate::text::single_quoted`] 로 싼다: 지금 넷은 안전한 낱말이지만, 꼴에
/// 박아 두면 이름이 바뀌는 날 이 줄이 깨지고 그 값은 세션의 모든 도구 호출이다.
fn command(exe: &str, event: &str) -> String {
    // 따옴표를 깨는 경로는 아예 안 쓴다. 셸 한 줄이 깨지면 그 세션의 모든
    // 도구 호출이 막힌다.
    let exe = if quotable(exe) { exe } else { "moai" };
    // **`command -v` 로 본다. `[ -x ]` 가 아니다.** `[ -x "moai" ]` 는 PATH 를
    // 안 보고 `./moai` 를 본다 — PATH 에 moai 가 있는 남의 기계에서 훅이 전부
    // 조용히 `exit 0` 으로 빠지고, 그 모습은 "규칙이 통과했다" 와 똑같다.
    // 이 저장소에서 그 검사가 참으로 보였던 것도 하필 `moai` 라는 **디렉터리**가
    // 있어서였다.
    //
    // **그런데 `command -v` 는 "있는가" 를 껍데기마다 다르게 답한다**(리뷰 moai-j4ie 가
    // 쟀다). 경로를 받으면 dash 는 있는지만 보고 0 을 내는데 bash 는 `access(X_OK)` 까지
    // 보아 실행 비트가 빠진 파일에 1 을 낸다(`bash --posix` 도, `sh` 로 불린 bash 도 같다).
    // 그러면 `|| exit 0` 이 먼저 걸려, **이 문이 겨눈 첫 판 — 실행 비트가 빠진 판 — 이
    // bash 가 `/bin/sh` 인 기계(macOS·Fedora·RHEL)에서 그대로 조용하다.** 그래서 경로
    // 꼴에는 "있는가" 를 `[ -e ]` 로 한 번 더 묻는다: 있으면 돌려 보고 껍데기가 내는
    // 126·127 로 가른다. 이름 꼴에는 안 붙인다 — `[ -e "moai" ]` 는 위의 `[ -x ]` 와 같이
    // `./moai` 를 보므로 그 덫이 그대로 돌아온다.
    let there = if exe.contains('/') { format!(" || [ -e \"{exe}\" ]") } else { String::new() };
    // **종료 값을 `||` 로 받는다.** 맨 명령으로 두면 `set -e` 가 선 껍데기에서 그 자리에서
    // 죽어 `exit 0` 에 닿지 못하고, 그때 나가는 값이 2 면 위 표대로 **게이트가 된다**
    // (1 로 진 판은 이미 stdout 에 쓴 판정까지 함께 버려진다). 옛 한 줄은 `&&`·`||` 목록
    // 하나라 `set -e` 가 손대지 않던 자리였다 — 공짜였던 그 면역을 되돌려 놓는다.
    //
    // 없을 때 빠지는 자리를 `&&` 뒤가 아니라 **앞줄**로 세운 까닭은 뒤의 종료 값을
    // 재야 하기 때문이다 — 한 줄에 매달면 `|| exit 0` 이 둘을 같이 삼킨다.
    let said = crate::text::single_quoted(event);
    // **경로도 인자 자리로 넘긴다** — 꼴에 박으면 경로의 `%` 가 변환 문자로 읽힌다(이벤트
    // 이름과 같은 까닭). `quotable` 을 지난 경로라 작은따옴표 안에서 셸이 아무것도 안 푼다.
    //
    // **실을 수 없는 경로면 자리를 통째로 뺀다**([`sayable`], 리뷰 moai-514e.hgz 5번) — 셸에는
    // 멀쩡하지만 JSON 에는 못 싣는 경로(제어문자)가 그 자리다. 그런 경로도 훅은 그대로 부르니,
    // 빠지는 것은 알림의 한 토막뿐이고 어느 파일인지는 `moai skill status` 가 댄다.
    let (spot, where_) =
        if sayable(exe) { (": %s", format!(" {}", crate::text::single_quoted(exe))) } else { ("", String::new()) };
    // **세션마다 한 줄이다**(moai-f7up) — 표식 이름에 부를 바이너리의 철자를 섞는다. 같은 세션이
    // 저장소 둘을 오가면 둘 다 제 알림을 내야 하는데(제 바이너리가 저마다 못 돌 수 있다),
    // 이름만으로는 첫 저장소의 표식이 둘째의 입을 막는다. 셈은 셸이 못 하므로 **심을 때 박아
    // 둔다**.
    //
    // **키는 저장소가 아니라 그 바이너리다**(리뷰 moai-514e.hgz). 둘은 대개 같지만
    // [`exe_name`] 이 이름(`moai`)으로 적는 줄 — PATH 의 moai 가 곧 이 저장소의 바이너리라
    // 팀이 심은 트리를 그대로 커밋한 자리와 [`quotable`] 이 거절한 자리 — 에서는 갈린다.
    // 그 줄에서는 저장소가 둘이어도 훅이 실제로 부르는 파일이 하나라 알림도 하나가 맞다.
    // 갈리는 것은 저장소마다 PATH 가 다른 경우(direnv)뿐이고, 그것을 가르려면 `market` 을
    // 여기까지 들고 와야 한다 — `cmd::hook` 의 `session_file` 이 `repo.dir()` 를 쓰는 것과
    // 여기가 다른 자리다.
    //
    // **견주는 것은 자리가 아니라 철자다**(리뷰 moai-gu5m.ke0). 훅에는 부른 철자가 적히니(moai-gu5m) 한
    // 파일을 두 철자로 심으면 키도 알림도 둘이다. 푼 자리로 키를 지으면 기계마다 다른 `/tmp/…` 의 해시가
    // 커밋되는 `plugin.json` 에 다시 든다 — moai-gu5m 이 걷은 바로 그 diff 다.
    let whose = stable(exe.as_bytes()) % 0x1_0000;
    // **이벤트도 키에 든다**(리뷰 moai-514e.hgz) — `cmd::hook` 의 `session_file` 이 `what` 을
    // 키에 넣은 것과 같은 까닭이다. 하나로 두던 판은 **`SessionStart` 가 그 한 줄을 태웠다**:
    // 그 이벤트는 세션의 맨 앞에서 돌고, 위 표대로 거기서만 notice 가 안 서고 `hook_response`
    // 에 남는다 — 사람이 못 듣는 알림 하나가 표식을 세워 `PreToolUse`·`UserPromptSubmit`·
    // `Stop` 의 입을 세션 내내 막았다. 규칙 다섯을 싣는 `PreToolUse` 가 매 도구 호출에 져도
    // 화면은 규칙이 통과한 것과 한 글자도 다르지 않았다 — moai-j4ie 가 끝내려던 바로 그 침묵이다.
    // 값은 세션에 넷까지고, 문턱이 겨눈 140~1,257 과는 자릿수가 다르다.
    //
    // 이름은 `HOOKS` 가 든 넷뿐이라 그대로 파일 이름에 적는다 — 모두 ASCII 낱말이다.
    //
    // **종료 값도 키에 든다**(리뷰 moai-514e.hgz). 목록이 0·1 밖 전부로 넓어지며 표식을 태우는
    // 것이 설치와 상관없는 한 번짜리 죽음까지가 됐다 — 이 컨테이너에 이력이 있는 OOM 의 137,
    // SIGSEGV 의 139, 옛 바이너리가 모르는 이벤트 이름에 내는 clap 의 2 다. 그 한 번이 표식을
    // 세우면 **뒤에 정말 온 126·127 이 그 세션 내내 조용하다**, 이벤트를 갈라도 그렇다(같은
    // 이벤트가 그렇게 죽을 수 있다). 값을 함께 키로 두면 값이 달라질 때만 다시 말하니, 상황이
    // 정말 바뀐 자리에서만 한 줄이 는다. `$c` 는 `$?` 가 낸 0~255 이라 파일 이름에 안전하다.
    //
    // **갈래는 셋이고 종료 값은 한 번만 가른다**(리뷰 moai-45hf.nab) — `moai` 가 스스로 끝냈으면(0·1)
    // 받은 것을 흘려보내고 표식을 세우고, 아니면 판정의 꼴일 때 흘려보내기만 하고, 꼴이 아니면 알림을
    // 낸다. 0·1 을 두 자리에 적던 줄은 한쪽만 넓히는 날 토막을 판정으로 흘리거나 알림을 삼킨다.
    let handoff = crate::cmd::hook::HANDOFF;
    format!(
        "command -v -- \"{exe}\" >/dev/null 2>&1{there} || exit 0; trap 'exit 0' PIPE; \
         h=\"${{TMPDIR:-/tmp}}/moai-hook-${{CLAUDE_CODE_SESSION_ID:-$PPID}}-$$.handoff\"; \
         c=0; o=$({handoff}=\"$h\" \"{exe}\" hook {event}) || c=$?; \
         set -C; \
         case \"$c\" in \
         0|1) [ -z \"$o\" ] || {{ printf '%s\\n' \"$o\" && [ -O \"$h\" ] && ! [ -h \"$h\" ] \
         && read -r m 2>>/dev/null < \"$h\" && ! [ -e \"$m\" ] && ! [ -h \"$m\" ] && true 2>>/dev/null > \"$m\"; }} || :;; \
         *) case \"$o\" in '{{'*'}}') printf '%s\\n' \"$o\" || :;; *) \
         s=\"${{TMPDIR:-/tmp}}/moai-hook-{whose:04x}-${{CLAUDE_CODE_SESSION_ID:-$PPID}}.{event}.$c.said\"; \
         {{ ! [ -e \"$s\" ] && ! [ -h \"$s\" ] && true 2>>/dev/null > \"$s\" \
         || ! [ -f \"$s\" ] || ! [ -O \"$s\" ] || [ -h \"$s\" ]; }} && \
         printf '{{\"systemMessage\":\"moai: the %s hook could not run (exit %s){spot}. \
         The five moai rules are not standing - run moai skill status to see why.\"}}' {said} \"$c\"{where_} || :;; \
         esac;; esac; exit 0"
    )
}

/// 심을 파일들. 경로는 `DIR` 부터의 상대다.
///
/// **판(version)은 내용의 해시다.** `claude plugin install` 은 `directory`
/// 원본이어도 제 캐시로 **복사**하고, 그 복사는 `plugin.json` 의 판이 달라질
/// 때만 새로 뜬다. 손으로 세는 판은 반드시 어긋난다 — 내용이 같으면 판도
/// 같아 헛 업데이트가 없고, 한 글자라도 다르면 반드시 달라진다.
///
/// 판이 **내려가도** 괜찮은 까닭은 [`version_of`] 에 있다. 두 자리에 나눠 적으면
/// 한쪽만 고쳐져 갈라진다 — 실제로 여기 적혀 있던 까닭이 틀린 채로 남아 있었다.
pub fn tree(prefix: &str, root: &Path, exe: &str, skills: &[Skill]) -> Vec<(PathBuf, String)> {
    tree_named(&market(prefix, root), exe, skills)
}

/// `tree` 의 몸통. 저장소 자리는 마켓플레이스 이름으로만 들어오므로, 이름을
/// 받아 두면 **커밋된 트리를 그 트리가 적힌 자리 그대로** 다시 낼 수 있다 —
/// 다른 체크아웃(워크트리)에서 부른 시험이 남의 자리를 안 섞는다.
fn tree_named(market: &str, exe: &str, skills: &[Skill]) -> Vec<(PathBuf, String)> {
    let mut files: Vec<(PathBuf, String)> =
        skill_files(skills).map(|(p, b)| (Path::new("skills").join(p), b)).collect();
    let market = (PathBuf::from(".claude-plugin/marketplace.json"), marketplace_json(market));
    // 판은 **매니페스트를 뺀 트리 전부와 판 자리를 비운 매니페스트**에서 나온다.
    // 매니페스트 자신은 그 판을 담고 있으므로 그대로는 셈에 넣을 수 없다 —
    // 넣으면 해시가 제 꼬리를 문다. 그렇다고 매니페스트를 통째로 빼면 그 틀
    // (`timeout`·`description`)만 바꾼 판이 옛 판과 같아 `claude` 가 옛 복사를
    // 계속 쓴다. 훅 명령·실행 파일·이름표는 그 틀과 `marketplace.json` 에 이미
    // 들어 있어 따로 셈하지 않는다 — 따로 적은 목록은 틀이 자랄 때마다 어긋난다.
    let version = version_of(&[files.as_slice(), std::slice::from_ref(&market)].concat(), &plugin_json(exe, ""));
    files.push((PathBuf::from(".claude-plugin/plugin.json"), plugin_json(exe, &version)));
    files.push(market);
    files
}

/// 트리의 내용 해시를 판으로 낸다. **오르내린다** — 해시라 다음 판이 더 낮을 수 있다
/// (실제로 `968.33.712` 다음이 `59.172.404` 이었다).
///
/// 괜찮은 까닭을 확인했다(moai-70ip, 2026-09-15). `claude` 의 플러그인 갱신은 판을 semver
/// 로 견주지 않고 **달라졌는가**만 본다 — 판 글자를 안 바꾸면 갱신이 안 가고, 바꾸면 간다
/// (plugin-marketplaces·plugins-reference 문서). moai 쪽 신선도(`cmd/skill.rs`)도 같은지만
/// 본다. 그래서 내려가는 판이 갱신을 건너뛰게 하지 않는다 — `1.0.1 → 0.0.5` 로 실제로 재 봤다.
///
/// 판이 semver 여야 한다거나 올라야 한다는 요구는 문서에 없다. 그래도 **모양은 semver 로
/// 맞춰 둔다** — 판을 그렇게 읽는 자리가 나중에 생겨도 값이 형식에서 먼저 걸리지는 않는다.
/// 갱신 판정이 semver 비교로 바뀌는 날에는 해시를 버리지 말고 앞자리에 오르는 셈을 붙인다.
fn version_of(files: &[(PathBuf, String)], template: &str) -> String {
    let mut all = String::new();
    for (path, body) in files {
        all.push_str(&path.to_string_lossy());
        all.push('\u{1}');
        all.push_str(body);
        all.push('\u{2}');
    }
    all.push_str(template);
    // semver 세 자리에 나눠 담는다 — 위 주석의 까닭이다.
    let n = stable(all.as_bytes());
    format!("{}.{}.{}", n % 1000, (n / 1000) % 1000, (n / 1_000_000) % 1000)
}

/// **누가 심었는지는 안 적는다.** 심는 사람마다 이 파일이 바뀌면, 팀이
/// 커밋해 두고 쓰는 트리가 사람이 바뀔 때마다 헛 diff 를 낸다. 그리고 그 값이
/// 판(해시)에 안 들어가면 내용이 달라졌는데 판은 그대로인 자리가 생긴다 —
/// `claude` 가 옛 복사를 그대로 쓴다. 안 적는 것이 둘 다 푼다.
fn plugin_json(exe: &str, version: &str) -> String {
    let mut hooks = BTreeMap::new();
    for (at, event, message) in HOOKS {
        let entry = serde_json::json!({
            "type": "command",
            "command": command(exe, event),
            "timeout": 15,
            "statusMessage": message,
        });
        let group = if *at == "PreToolUse" {
            serde_json::json!({ "matcher": WATCHED, "hooks": [entry] })
        } else {
            serde_json::json!({ "hooks": [entry] })
        };
        hooks.insert(*at, vec![group]);
    }
    let manifest = serde_json::json!({
        "name": "moai",
        "description": "This repository's issue tracker. Loads the board into the session and keeps new issues from leaking outside the unit you picked up.",
        "version": version,
        "hooks": hooks,
    });
    pretty(&manifest)
}

fn marketplace_json(name: &str) -> String {
    pretty(&serde_json::json!({
        "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
        "name": name,
        "description": "Planted by moai. Edit it by hand and the next `moai skill install` overwrites it.",
        "owner": { "name": "moai" },
        "plugins": [{
            "name": "moai",
            "description": "This repository's work, rules and board.",
            "source": "./",
            "category": "productivity",
        }],
    }))
}

fn pretty(v: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

/// 훅이 부를 실행 파일을 어떻게 적을까.
///
/// **PATH 에서 같은 파일이 찾아지면 이름만 적는다.** 그러면 심은 트리를 팀이
/// 그대로 커밋해도 남의 기계에서 산다. 아니면 절대 경로다 — 이 저장소처럼
/// PATH 의 `moai` 가 **다른** moai 인 곳에서는 이름을 적는 것이 남의 바이너리를
/// 부르는 일이 된다.
///
/// `spelling` 은 적을 철자다 — `cmd::skill` 의 `invoked` 가 고른 부른 철자이거나 `current_exe` 다.
/// **같은 파일인지는 `resolved`(이 실행 파일의 푼 자리)로 가른다**(리뷰 moai-gu5m.ke0) — 부른 철자는
/// 링크가 안 풀렸을 수 있고(macOS 의 `current_exe` 도 안 푼다), `on_path` 는 `which` 가 푼 자리라 글자로
/// 견주면 자리로 견준 것이다. 푸는 일은 부르는 쪽이 한다 — 이 모듈은 파일 시스템을 안 본다.
pub fn exe_name(spelling: &Path, resolved: &Path, on_path: Option<&Path>) -> String {
    let shown = spelling.display().to_string();
    match on_path {
        Some(p) if p == resolved => "moai".to_string(),
        // 따옴표를 깨는 경로는 훅 한 줄에 못 적어 `command` 가 이름으로 바꿔 적는다.
        // **여기서 먼저 바꾼다** — 안 그러면 판·설치 출력·`status` 는 절대 경로를
        // 말하는데 훅은 PATH 의 `moai` 를 불러, 셋이 서로 다른 것을 가리킨다.
        _ if !quotable(&shown) => "moai".to_string(),
        _ => shown,
    }
}

/// `argv[0]` 이 대는 이 실행 파일의 철자 — **링크를 안 푼다**(moai-gu5m, 사용자 결정 2026-10-03).
///
/// 슬래시가 들면 글자로만 접는다 — 상대 철자는 `cwd` 에 붙인 뒤 `.`·`..` 을 걷는다. 이름뿐이면
/// `None` 이다 — PATH 에서 찾은 것과 같은 파일이면 [`exe_name`] 이 어차피 이름으로 적고, 아니면 부른
/// 쪽이 무엇을 불렀는지 모른다.
///
/// **훅 한 줄에 그대로 못 적는 철자도 `None` 이다**(리뷰 moai-gu5m.ke0). 따옴표를 깨면([`quotable`])
/// [`exe_name`] 이 이름(`moai`)으로 물러서 훅이 PATH 의 딴 moai 를 부르고, UTF-8 이 아니면 `display` 가
/// 없는 자리로 바꿔 적어 훅이 `|| exit 0` 으로 조용히 꺼진다. 푼 자리로 떨어지면 원래 적히던 그 철자라,
/// 부른 철자를 고른 탓에 새로 열리는 길이 없다. **프로세스마다 다른 것을 가리키는 자리**(`/proc`·`/dev`
/// — `/proc/self/exe`·`/dev/fd/N`)도 안 낸다. 이 프로세스 안에서는 이 파일로 풀려 같은 파일 견줌을
/// 지나지만, 훅의 셸에서는 그 셸 자신이거나 없는 자리다.
///
/// **이 값이 이 실행 파일을 가리키는지는 묻지 않는다.** `argv[0]` 은 부른 쪽 마음대로라(`exec -a`)
/// 부르는 쪽이 푼 자리를 `current_exe` 와 견준 뒤에만 쓴다.
pub fn spelled(argv0: Option<&Path>, cwd: Option<&Path>) -> Option<PathBuf> {
    let argv0 = argv0?;
    if argv0.parent().is_none_or(|p| p.as_os_str().is_empty()) {
        return None;
    }
    let full = match argv0.is_absolute() {
        true => crate::path::lexical(argv0),
        false => crate::path::lexical(&cwd.filter(|c| c.is_absolute())?.join(argv0)),
    };
    let per_process = full.starts_with("/proc") || full.starts_with("/dev");
    (!per_process && full.to_str().is_some_and(quotable)).then_some(full)
}

/// 셸 한 줄의 따옴표 안에 그대로 적을 수 있는 경로인가.
///
/// **제어문자는 여기서 안 거른다**(리뷰 moai-514e.hgz 5번). 한때 [`sayable`] 과 한 자였는데,
/// 이 자는 **어느 바이너리를 부를지**를 가르는 자리다 — 탭이 든 디렉터리에 받은 체크아웃은
/// 리눅스에서 멀쩡하고 `"…"` 안에서도 멀쩡한데, 거절하면 훅 넷이 PATH 의 `moai` 를 부른다.
/// 이 저장소가 바로 그 반례라 CLAUDE.md 가 적어 두었다: PATH 의 `moai` 는 옛 moai 의
/// 바이너리다. 조용히 남의 도구가 이 저장소의 규칙을 판정하거나, 없으면 `|| exit 0` 으로
/// 규칙 넷이 다 꺼진다. 둘을 가른 뒤로 그 경로는 그대로 불리고, 알림에만 안 실린다.
fn quotable(exe: &str) -> bool {
    !exe.contains(['"', '\\', '$', '`'])
}

/// 알림의 JSON 문자열 안에 그대로 실을 수 있는 경로인가(moai-wza7).
///
/// 제어문자 하나면 `systemMessage` 안의 날 줄바꿈·탭이 되어 `claude` 가 그 객체를 통째로
/// 버린다 — 알리려던 말이 도리어 사라진다. JSON 은 U+001F 까지를 문자열 안에 날것으로 못 둔다.
/// 못 실을 경로면 [`command`] 가 **자리 없이** 말한다: 틀린 자리를 대느니 안 대는 편이 낫고,
/// 그 자리는 알림이 함께 대는 `moai skill status` 가 댄다.
fn sayable(exe: &str) -> bool {
    quotable(exe) && !exe.contains(char::is_control)
}

/// `claude` 가 장부에 적어 둔 설치 한 건.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Install {
    pub scope: String,
    pub version: String,
    pub install_path: String,
    pub project: Option<String>,
}

/// `installed_plugins.json` 에서 설치 id(`<플러그인>@<마켓플레이스>`) 하나의 설치 중 **이 저장소에 드는 것**
/// 을 고른다 — 사용자 범위이거나 `projectPath` 가 여기인 줄. 옛 판이 moai 곁에 깔던 한국어 글쓰기
/// 플러그인(moai-lr1s 가 깔고 moai-vtfu 가 걷는다)도 이것으로 센다.
///
/// moai 는 마켓플레이스 이름이 저장소마다 달라(`market`) `moai@<이름>` 이면 이미 이
/// 저장소의 것이다. 그래도 `local`·`project` 는 `projectPath` 를 한 번 더
/// 본다 — 같은 이름이 옛 자리에 남아 있는 줄을 제 것으로 걷으면 남의 설정을
/// 건드린다. 자리를 견주는 법(심볼릭 링크 풀기)은 부르는 쪽이 준다. **옛 판이 곁에 깐 것의 id 는 기계에
/// 하나라** 사용자 범위의 줄은 어느 저장소의 것인지 모른다 — 걷는 쪽이 그것을 가른다(`cmd::skill::retire`).
pub fn installs_of(ledger: &serde_json::Value, key: &str, is_here: impl Fn(&str) -> bool) -> Vec<Install> {
    let Some(rows) = ledger.get("plugins").and_then(|p| p.get(key)).and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    let text = |row: &serde_json::Value, k: &str| row.get(k).and_then(|v| v.as_str()).map(str::to_string);
    rows.iter()
        .filter_map(|row| {
            let scope = text(row, "scope")?;
            let project = text(row, "projectPath");
            if scope != "user" && !project.as_deref().is_some_and(&is_here) {
                return None;
            }
            Some(Install {
                scope,
                version: text(row, "version").unwrap_or_default(),
                install_path: text(row, "installPath").unwrap_or_default(),
                project,
            })
        })
        .collect()
}

/// `known_marketplaces.json` 에서 이 이름의 마켓플레이스가 **어느 GitHub 저장소를 가리키는가**(`<owner>/<repo>`).
/// 모르는 이름이면 `None`, GitHub 가 아닌 출처면 빈 글 — 빈 글은 "그 저장소가 아니다" 로 읽힌다.
/// 옛 판이 곁에 깐 것을 걷을 때 그 마켓플레이스가 옛 판이 더하던 저장소인지를 이것으로 잰다
/// (`cmd::skill::retire`, 사용자 결정 moai-vtfu.dvk).
///
/// **주소로 더한 GitHub 도 GitHub 다.** `claude plugin marketplace add https://github.com/<o>/<r>` 는
/// `{"source":"git","url":"….git"}` 로 적힌다 — `repo` 만 읽던 판은 그것을 남의 출처로 보았다(`claude` 는
/// 둘을 같은 마켓플레이스로 본다).
pub fn market_repo(known: &serde_json::Value, name: &str) -> Option<String> {
    let source = known.get(name)?.get("source")?;
    let text = |k: &str| source.get(k).and_then(|v| v.as_str());
    let from_url = || {
        let url = text("url")?;
        let rest = url.strip_prefix("https://github.com/").or_else(|| url.strip_prefix("git@github.com:"))?;
        let rest = rest.trim_end_matches('/');
        Some(rest.strip_suffix(".git").unwrap_or(rest).to_string())
    };
    Some(text("repo").map(str::to_string).or_else(from_url).unwrap_or_default())
}

/// 설정(`settings.json`)의 `extraKnownMarketplaces` 가 이 이름을 **어느 GitHub 저장소로 선언했는가** — 꼴이
/// `known_marketplaces.json` 의 줄과 같아 [`market_repo`] 가 그대로 읽는다(`claude plugin marketplace add
/// --scope <범위>` 가 그 범위의 설정에 같은 `source` 를 적는다).
pub fn declared_repo(settings: &serde_json::Value, name: &str) -> Option<String> {
    market_repo(settings.get("extraKnownMarketplaces")?, name)
}

/// 설정의 `enabledPlugins` 가 이 마켓플레이스(`<플러그인>@<이름>`)에서 든 플러그인 id. **값은 안 본다** —
/// `false` 로 꺼 둔 줄도 그 선언을 가리킨다.
pub fn plugins_from(settings: &serde_json::Value, market: &str) -> Vec<String> {
    let tail = format!("@{market}");
    let Some(enabled) = settings.get("enabledPlugins") else { return Vec::new() };
    let ids: Vec<&str> = match enabled {
        serde_json::Value::Object(m) => m.keys().map(String::as_str).collect(),
        serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
        _ => Vec::new(),
    };
    ids.into_iter().filter(|id| id.ends_with(&tail)).map(str::to_string).collect()
}

/// 커밋된 설정 글(`.claude/settings.json`)에서 `extraKnownMarketplaces` 의 `name` 하나를 **줄째** 지운 글
/// (사용자 결정 moai-6ugu.aae). 옛 `skill install --scope project` 가 곁 플러그인의 마켓플레이스를 그 파일에
/// 선언했고, `claude plugin uninstall` 은 `enabledPlugins` 의 줄만 걷어 선언이 동료에게 계속 권해진다. 걷는
/// 길을 `claude` 에 맡기지 않는 까닭은 `marketplace remove --scope project` 가 다른 범위에 같은 선언이 없으면
/// 기계 전체 장부와 다른 저장소의 설치까지 걷어서다(claude 2.1.287 에서 잼).
///
/// **다른 바이트는 그대로 둔다** — 이 모듈 머리의 "남의 JSON 을 안 만진다" 는 다시 쓰면 서식이 사라져서였다.
/// 그래서 다시 짓지 않고 그 멤버의 글만 도려낸다. 줄 머리에서 선 멤버면 그 줄들을, 한 줄에 몰린 꼴이면 그
/// 멤버만 걷고, 쉼표는 뒤의 것(끝 멤버면 앞의 것) 하나를 함께 걷는다. 하나뿐이던 멤버를 걷으면 `{}` 가 된다 —
/// `claude` 가 빈 선언을 적는 꼴이다.
///
/// **지운 글을 다시 읽어 원래 값에서 그 키 하나만 빠졌을 때만 낸다.** 못 읽는 글(주석 든 JSON 등), 그 이름이
/// 없는 글, 그 이름이 둘 선 글은 `None` 이다(다른 키가 둘 선 것은 그대로 남기고 낸다). 부르는 걸음
/// (`cmd::skill::Undeclare`)은 `None` 이면 손으로 지울 줄을 낸다 — 다만 계획할 때 못 읽은 파일에는 걸음을 안
/// 세워 아무 말이 없고, 부를 때 그 이름이 이미 없으면 이룬 것으로 센다.
pub fn drop_marketplace(text: &str, name: &str) -> Option<String> {
    let before: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut want = before;
    want.get_mut("extraKnownMarketplaces")?.as_object_mut()?.remove(name)?;
    let b = text.as_bytes();
    let (top, _) = json_members(b, json_ws(b, 0))?;
    let outer = top.iter().find(|m| m.key_is(text, "extraKnownMarketplaces"))?;
    let (list, close) = json_members(b, outer.value.0)?;
    let at = list.iter().position(|m| m.key_is(text, name))?;
    let m = &list[at];
    let cut = if list.len() == 1 {
        outer.value.0 + 1..close
    } else if at + 1 < list.len() {
        let comma = json_ws(b, m.value.1);
        let start = line_head(b, m.key.0);
        let end = b[comma + 1..].iter().position(|&c| c == b'\n').map(|n| comma + 1 + n);
        match end {
            Some(end) if start.is_some() && b[comma + 1..end].iter().all(|c| c.is_ascii_whitespace()) => {
                start.unwrap()..end + 1
            }
            _ => m.key.0..list[at + 1].key.0,
        }
    } else {
        let comma = json_ws(b, list[at - 1].value.1);
        let mut end = m.value.1;
        while matches!(b.get(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        comma..end
    };
    let out = format!("{}{}", &text[..cut.start], &text[cut.end..]);
    (serde_json::from_str::<serde_json::Value>(&out).ok()? == want).then_some(out)
}

/// JSON 객체 멤버 하나의 글 자리 — 키(따옴표 포함)와 값의 `[시작, 끝)`.
struct JsonMember {
    key: (usize, usize),
    value: (usize, usize),
}

impl JsonMember {
    fn key_is(&self, text: &str, name: &str) -> bool {
        serde_json::from_str::<String>(&text[self.key.0..self.key.1]).is_ok_and(|k| k == name)
    }
}

/// `i` 부터 빈칸을 건넌 자리.
fn json_ws(b: &[u8], mut i: usize) -> usize {
    while b.get(i).is_some_and(|c| c.is_ascii_whitespace()) {
        i += 1;
    }
    i
}

/// `i` 에서 시작하는 값 하나가 끝나는 자리. 글자열 안의 괄호와 이스케이프를 건넌다. 꼴이 맞는지는 안 잰다 —
/// 그것은 [`drop_marketplace`] 의 다시 읽기가 잰다.
fn json_value_end(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i)? {
        b'"' => {
            let mut j = i + 1;
            loop {
                match b.get(j)? {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
        }
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut j = i;
            loop {
                match b.get(j)? {
                    b'"' => {
                        j = json_value_end(b, j)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        _ => {
            let n = b[i..].iter().position(|c| c.is_ascii_whitespace() || matches!(c, b',' | b'}' | b']'));
            Some(n.map_or(b.len(), |n| i + n))
        }
    }
}

/// `open` 의 `{` 로 여는 객체의 멤버들과 닫는 `}` 의 자리.
fn json_members(b: &[u8], open: usize) -> Option<(Vec<JsonMember>, usize)> {
    if b.get(open) != Some(&b'{') {
        return None;
    }
    let mut out = Vec::new();
    let mut i = json_ws(b, open + 1);
    if b.get(i) == Some(&b'}') {
        return Some((out, i));
    }
    loop {
        if b.get(i) != Some(&b'"') {
            return None;
        }
        let key = (i, json_value_end(b, i)?);
        let colon = json_ws(b, key.1);
        if b.get(colon) != Some(&b':') {
            return None;
        }
        let start = json_ws(b, colon + 1);
        let value = (start, json_value_end(b, start)?);
        out.push(JsonMember { key, value });
        i = json_ws(b, value.1);
        match b.get(i)? {
            b',' => i = json_ws(b, i + 1),
            b'}' => return Some((out, i)),
            _ => return None,
        }
    }
}

/// `at` 앞이 줄 머리까지 빈칸뿐이면 그 줄 머리.
fn line_head(b: &[u8], at: usize) -> Option<usize> {
    let mut i = at;
    while i > 0 && matches!(b[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    (i == 0 || b[i - 1] == b'\n').then_some(i)
}

/// 매니페스트가 훅으로 부르는 실행 파일. `command` 가 적는 모양
/// (`command -v -- "<exe>" …`)에서 따옴표 속을 꺼낸다.
///
/// **설치본의 매니페스트를 읽는다.** 저장소의 트리가 아니라 `claude` 가 복사해
/// 간 쪽이 실제로 불린다 — 둘이 어긋난 채로 저장소만 보면 멀쩡해 보인다.
pub fn hook_exe(plugin_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(plugin_json).ok()?;
    let cmd = v
        .get("hooks")?
        .as_object()?
        .values()
        .filter_map(|groups| groups.get(0)?.get("hooks")?.get(0)?.get("command")?.as_str())
        .next()?;
    let rest = cmd.strip_prefix("command -v -- \"")?;
    Some(rest[..rest.find('"')?].to_string())
}

/// 트리의 매니페스트에 적힌 판.
pub fn version_in(files: &[(PathBuf, String)]) -> Option<String> {
    let (_, body) = files.iter().find(|(p, _)| p.ends_with("plugin.json"))?;
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("version")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{Issue, Kind, Status};

    fn tree_of(exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_for("t", exe, skill)
    }

    fn tree_for(prefix: &str, exe: &str, skill: &str) -> BTreeMap<String, String> {
        tree_at(prefix, Path::new("/repo"), exe, skill)
    }

    fn tree_at(prefix: &str, root: &Path, exe: &str, skill: &str) -> BTreeMap<String, String> {
        keyed(tree(prefix, root, exe, &fake(skill, "감독", "위키")))
    }

    fn keyed(files: Vec<(PathBuf, String)>) -> BTreeMap<String, String> {
        files.into_iter().map(|(p, b)| (p.display().to_string(), b)).collect()
    }

    /// [`skills`] 와 같은 꼴에 글만 갈아 끼운 목록 — 이름과 자리는 [`skills`] 가 낸 그대로다. 손으로 다시 짜면
    /// 스킬 하나가 늘 때 시험만 옛 꼴로 남는다. 참고 문서는 `참고` 로 둔다.
    fn fake(skill: &str, supervise: &str, wiki: &str) -> Vec<Skill> {
        let mut all = skills();
        for (s, body) in all.iter_mut().zip([skill, supervise, wiki]) {
            s.files = s
                .files
                .iter()
                .map(|(rel, _)| (*rel, if *rel == "SKILL.md" { body } else { "참고" }.to_string()))
                .collect();
        }
        all
    }

    /// 초점이 있을 때 **막히는 것이 옳은** 명령들.
    ///
    /// 새 단위를 세우는 자리라 규칙 1 이 잡는 것이 뜻대로다. 목록으로 두는
    /// 까닭은 본문이 바뀔 때 **왜 막히는지 한 번 더 생각하게 하기 위해서**다 —
    /// 시험이 규칙을 다시 구현하면 규칙이 틀렸을 때 시험도 같이 틀린다.
    ///
    /// 목록은 **정확히** 적는다. `moai add "제목" -p 1 -t bug -e <에픽>` 처럼
    /// 앵커가 붙은 줄까지 접두로 싸잡으면, 지나가는 것이 맞는 명령을 시험이
    /// "막혀야 한다" 고 우긴다 — 처음 적을 때 실제로 그랬다.
    ///
    /// 위키 스킬의 훑기가 세우는 일(`moai add 'wiki: …' -t docs`)도 새 단위다 — 사람이 따로 청한
    /// 일이라 집은 일의 에픽에 들지 않는다. 스킬은 이 거절을 미리 말하고 집은 일을 먼저 끝내라 한다.
    const DENIED_WHILE_HELD: &[&str] = &["moai epic add", "moai milestone add", "moai add 'wiki: "];

    /// **심는 글이 가르치는 명령은 규칙에 막히면 안 된다.**
    ///
    /// 이 글은 에이전트가 **가장 먼저** 읽는 것이라, 규칙과 어긋나면 거절문을
    /// 보기도 전에 틀린 길로 간다. 실제로 `-b` 없는 `add` 와 `-m` 없는 `done`
    /// 을 가르치고 있었고, 그것을 잡은 것은 시험이 아니라 리뷰였다.
    ///
    /// **초점이 있는 상태로만 본다.** 앞선 판은 아무것도 안 집은 상태도 함께
    /// 봤는데, 그 상태에서는 `guard_create` 와 `guard_close` 가 명령을 읽기도
    /// 전에 통과한다 — 어떤 글자를 넣어도 초록이라 아무것도 지키지 못했다.
    /// 리뷰어가 나쁜 명령을 일부러 심어도 시험은 웃고 있었다.
    #[test]
    fn what_the_skill_teaches_actually_passes() {
        use crate::hook::{Decision, guard_shell};

        let cfg = Config::parse("prefix = \"t\"\n").unwrap();
        let held = vec![epic_row(), held_row(), review_row("in_progress")];
        let root = Path::new("/repo");

        let mut checked = 0;
        for cmd in taught() {
            let should_deny = DENIED_WHILE_HELD.iter().any(|d| cmd.starts_with(d));
            // 훅이 실제로 부르는 그 차례로 본다 — 손으로 다시 짠 차례는 규칙이
            // 하나 늘 때 여기서 빠진다.
            let got = guard_shell(&held, &cfg, &Default::default(), root, root, &cmd);
            // 막는가로 가른다 — 비추는 줄(`Context`)은 막지 않는다(`idea add` 에 갈림길 1 의 둘째
            // 물음이 실린다). 변형마다 가르던 판은 막혀야 할 명령이 비추기만 해도 "막힌다" 로 적었다.
            match (got.blocks(), should_deny) {
                (false, false) | (true, true) => {}
                (false, true) => panic!("막혀야 하는데 지나간다 — {cmd}\n{got:?}"),
                (true, false) => panic!("가르치는 명령이 막힌다 — {cmd}\n{got:?}"),
            }
            checked += 1;
        }
        assert!(checked > 15, "가르치는 명령을 {checked}개밖에 못 찾았다");
        // 걸음 글에 박힌 줄도 뽑혔다(moai-8na5) — 브리프가 멤버를 집고 닫는 두 줄.
        let all = taught();
        for line in ["moai mv t-1 in_progress --from todo", "moai mv t-1 done"] {
            assert!(all.iter().any(|c| c == line), "걸음 글의 `{line}` 이 훅 시험 밖이다");
        }

        // **아무것도 안 집은 채로도 쓰기 규칙에 안 걸린다.** 가르치는 명령은
        // 트래커를 만질 뿐 저장소 파일을 쓰지 않는다 — `< <리뷰 원문>` 같은
        // 자리표시자의 `>` 가 리다이렉션으로 읽히면 여기서 붉어진다.
        let idle = vec![epic_row()];
        for cmd in taught() {
            assert_eq!(
                guard_shell(&idle, &cfg, &Default::default(), root, root, &cmd),
                Decision::Pass,
                "가르치는 명령이 아무것도 안 집은 채로 막힌다 — {cmd}"
            );
        }
    }

    /// **가르친 대로 세운 리뷰로 곧장 리뷰를 부를 수 있어야 한다.**
    ///
    /// 관점(`-b`)을 요구하는 것은 `guard_review` 인데 위 시험은 그것을 부르지
    /// 않는다. 그래서 가르치는 줄에서 `-b` 를 지워도 초록이었다 — 리뷰어가
    /// 실제로 지워 보고 알아냈다. 여기서 그 다리를 놓는다.
    #[test]
    fn a_review_made_as_taught_can_be_used_at_once() {
        use crate::hook::{Decision, guard_review};

        let cfg = Config::parse("prefix = \"t\"\n").unwrap();
        let made = taught()
            .into_iter()
            .find(|c| c.starts_with("moai add") && c.contains("-t review"))
            .expect("리뷰를 세우는 명령을 안 가르친다");

        // 그 명령이 만들 줄을 세운다 — 본문은 `-b` 가 있을 때만 붙는다.
        let mut review = review_row("in_progress");
        review.body = angle_of(&made);
        let all = vec![epic_row(), held_row(), review];

        assert_eq!(
            guard_review(&all, &cfg, &Default::default()),
            Decision::Pass,
            "가르친 대로 세운 리뷰가 규칙 3 에 막힌다 — {made}"
        );
    }

    /// `-b "<글>"` 이 있으면 그 글. 없으면 `None` — 본문 없는 줄이 선다.
    fn angle_of(cmd: &str) -> Option<String> {
        let at = cmd.find("-b ")?;
        let rest = cmd[at + 3..].trim_start_matches('"');
        let end = rest.find('"').unwrap_or(rest.len());
        let body = rest[..end].trim();
        (!body.is_empty()).then(|| body.to_string())
    }

    /// 심는 글과 AGENTS 블록이 실제로 가르치는 명령들. 자리표시자는 실제 값으로 바꾼다.
    ///
    /// **글자로 고르지 않는다.** 앞서 리뷰 절차를 `contains("review")` 로
    /// 골랐는데, 그 그물은 `moai show -s todo,review` 를 끌어오고 리뷰
    /// 토막의 문구가 바뀌면 조용히 아무것도 안 고른다.
    fn taught() -> Vec<String> {
        // AGENTS 블록도 같은 조각에서 나오고, 감독이 일꾼에게 싣는 글도 리뷰를 세우고
        // 닫는 줄을 같은 조각으로 적으므로 같이 본다.
        let texts = [
            crate::guide::skill(),
            crate::guide::reference(),
            crate::guide::agents(),
            crate::guide::supervise(),
            crate::guide::wiki(),
        ];
        // **걸음 글 안에 박힌 `` `moai …` `` 도 뽑는다**(moai-8na5). 줄 머리만 보던 판은 브리프 2·10·12
        // 의 멤버를 옮기는 줄과 에픽에 남기는 노트를 훅 시험 밖에 두었다 — 거기서 무엇을 바꿔도 초록이었다.
        // 자리표시자를 든 것만 명령이다 — 글 속의 `` `moai add` `` 는 이름을 부른 것이지 칠 줄이 아니다.
        let spans = |t: &str| -> Vec<String> {
            t.split('`')
                .skip(1)
                .step_by(2)
                .map(str::trim)
                .filter(|s| s.starts_with("moai ") && s.contains('<'))
                .map(String::from)
                .collect()
        };
        let cmds: Vec<String> = texts
            .iter()
            .flat_map(|t| t.lines())
            .map(str::trim)
            .filter(|l| l.starts_with("moai "))
            // 치트시트 줄은 **두 칸 이상 띄우고** 설명을 붙인다. 그 뒤는
            // 명령이 아니다 — 안 자르면 "집기 → review → done" 의 `done` 이
            // 인자로 읽혀, 시험이 제가 만든 허깨비를 잡는다.
            .map(|l| l.split("  ").next().unwrap_or(l).trim().to_string())
            .chain(texts.iter().flat_map(|t| t.lines()).flat_map(spans))
            .filter(|l| !l.contains("<command>"))
            .collect();
        let taught: Vec<String> = cmds
            .iter()
            .map(|l| {
                l.replace("<id>", "t-r")
                    .replace("<epic>", "t-e")
                    .replace("<the issue>", "t-1")
                    .replace("<review id>", "t-r")
                    .replace("<review text>", "/tmp/review.txt")
                    .replace("<that file>", "/tmp/review.txt")
                    .replace("<keyword>", "parser")
                    .replace("<milestone>", "t-m")
                    // 치트시트·예시의 제목 자리. 남으면 `<`·`>` 가 리다이렉션으로 읽힌다.
                    .replace("<title>", "title")
                    .replace("<what you found>", "what I found")
                    .replace("<why>", "why")
                    // 감독이 일꾼에게 싣는 글은 멤버를 `<멤버>` 로 부른다. 자리표시자가 남으면 셸
                    // 읽기가 `<`·`>` 를 리다이렉션으로 읽어, 가르친 명령이 아니라 엉뚱한 쓰기를
                    // 잰다 — 9-1 의 노트 줄은 글의 `(` 가 그 쓰기를 못 읽을 것으로 돌려 우연히
                    // 지나갈 뿐이었다. 집은 일(`t-1`)을 주는 것은 그 줄이 가리키는 것이 멤버라서다 —
                    // 걸음 글의 `moai mv <멤버> done` 에 리뷰 id 를 주면 규칙 3 이 `-m` 없는 닫기로 막는다.
                    .replace("<member>", "t-1")
                    // 되짚기(7-1)가 선 에픽의 멤버로 펼치는 idea. 어느 id 든 규칙이 가리지 않는다.
                    .replace("<idea id>", "t-i")
                    // 배포 기준으로 가른 줄(7-2)을 마일스톤 밖으로 보내는 자리. `<id>` 로 부르지
                    // 않는 것은 감독이 채우는 이름과 겹치면 맡긴 idea 의 값이 그 줄에 박혀 와서다.
                    .replace("<that row>", "t-1")
                    // 되짚기의 줄은 워크트리에서 루트를 가리켜 친다(4-1). 감독이 채우는 자리라 여기서는
                    // 시험의 뿌리로 둔다 — 남으면 `<`·`>` 가 리다이렉션으로 읽혀, 가르친 명령이 아니라
                    // 파일에 쓰는 엉뚱한 명령을 잰다.
                    .replace("<root>", "/repo")
                    // 치트시트가 인자 자리를 이렇게 부른다 — 남으면 `<`·`|`·`>` 가 리다이렉션과 파이프로
                    // 읽혀, 가르친 명령이 아닌 것을 잰다(moai-8na5).
                    .replace("<epic|milestone id>", "t-e")
                    .replace("<dir>", "/tmp/elsewhere")
                    .replace("<event>", "stop")
                    // 되짚을 멤버를 넘겨받는 줄(감독 0)은 그 멤버가 선 칸을 그대로 친다(moai-0zjo).
                    .replace("<its column>", "in_progress")
                    // 위키 스킬과 AGENTS 블록이 페이지 하나를 이렇게 부른다(moai-bl3x).
                    .replace("<slug>", "cli")
            })
            .collect();
        // **자리표시자가 남으면 시끄럽게 진다**(moai-8na5). 남은 `<…>` 는 셸 읽기가 리다이렉션으로
        // 읽어, 가르친 명령이 아니라 엉뚱한 쓰기를 잰다 — 그 줄은 우연히 지나가거나 우연히 막힌다.
        let left: Vec<&String> = taught.iter().filter(|c| placeholder(c).is_some()).collect();
        assert!(left.is_empty(), "자리표시자가 남은 명령 — 위의 replace 에 더한다: {left:#?}");
        taught
    }

    /// 따옴표 밖의 `<낱말>` 꼴 자리표시자 — `<` 바로 뒤가 빈칸이 아니고 `>` 로 닫힌다.
    /// `-b - < /tmp/x` 의 리다이렉션은 `<` 뒤가 빈칸이라 아니고, `'<무엇을>'` 처럼 따옴표 안은
    /// 셸이 글자로 읽으니 아니다.
    fn placeholder(cmd: &str) -> Option<String> {
        let mut quote = None;
        let mut chars = cmd.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            match (quote, c) {
                (None, '\'' | '"') => quote = Some(c),
                (Some(q), c) if c == q => quote = None,
                (None, '<') if chars.peek().is_some_and(|(_, n)| !n.is_whitespace() && !matches!(n, '<' | '(')) => {
                    let end = cmd[at..].find('>')?;
                    return Some(cmd[at..=at + end].to_string());
                }
                _ => {}
            }
        }
        None
    }

    fn epic_row() -> Issue {
        Issue::new("t-e".into(), "에픽".into(), Kind::Epic, Status::new("todo"), NOW)
    }

    fn held_row() -> Issue {
        let mut i = Issue::new("t-1".into(), "집은 일".into(), Kind::Issue, Status::new("in_progress"), NOW);
        i.epic = Some("t-e".into());
        i
    }

    fn review_row(status: &str) -> Issue {
        let mut i = Issue::new("t-r".into(), "리뷰".into(), Kind::Issue, Status::new(status), NOW);
        i.epic = Some("t-e".into());
        i.tags = vec!["review".into()];
        i.body = Some("무엇을 왜 보는가".into());
        i
    }

    const NOW: &str = "2026-01-01T00:00:00Z";

    /// 심는 것은 여섯이다 — 스킬, 참고, 감독 스킬, 위키 스킬, 그리고 매니페스트 둘.
    #[test]
    fn the_tree_has_what_claude_needs() {
        let files = tree_of("/bin/moai", "# 스킬");
        for want in [
            "skills/moai/SKILL.md",
            "skills/moai/references/commands.md",
            "skills/moai-supervise/SKILL.md",
            "skills/moai-wiki/SKILL.md",
            ".claude-plugin/plugin.json",
            ".claude-plugin/marketplace.json",
        ] {
            assert!(files.contains_key(want), "{want} 가 없다");
        }
    }

    /// 심는 스킬 디렉터리는 [`NAMES`] 그대로다 — 차례까지. 목록 밖의 스킬을 트리에 더하면 위키가 그 이름을 다시
    /// 이슈 id 로 센다(moai-mdzx.3pm).
    #[test]
    fn the_tree_plants_every_skill_name() {
        let planted: Vec<String> = tree("t", Path::new("/repo"), "/bin/moai", &fake("스킬", "감독", "위키"))
            .into_iter()
            .filter_map(|(p, _)| {
                let dir = p.strip_prefix("skills").ok()?.parent()?;
                (p.file_name()? == "SKILL.md").then(|| dir.display().to_string())
            })
            .collect();
        assert_eq!(planted, NAMES);
    }

    /// **`.agents/skills` 는 Claude 의 트리와 같은 글을 같은 이름 밑에 받는다**(moai-xs2h.xgo, 2026-10-04 사용자 결정) —
    /// 다른 것은 매니페스트가 없다는 것뿐이다. 에이전트마다 글을 따로 내면 같은 것이 세 벌이 되어 갈라지고, 이름이
    /// [`NAMES`] 밖으로 새면 위키가 그 이름을 다시 이슈 id 로 센다(moai-mdzx.3pm).
    #[test]
    fn the_agents_tree_carries_the_same_skills_without_a_manifest() {
        let all = fake("# 스킬", "감독", "위키");
        let claude = keyed(tree("t", Path::new("/repo"), "/bin/moai", &all));
        let shared = keyed(agents_tree(&all));
        let dirs: Vec<String> = agents_tree(&all)
            .into_iter()
            .filter_map(|(p, _)| {
                let dir = p.parent()?;
                (p.file_name()? == "SKILL.md").then(|| dir.display().to_string())
            })
            .collect();
        assert_eq!(dirs, NAMES, "심는 스킬이 NAMES 와 다르다 — 차례까지");
        for (path, body) in &shared {
            assert_eq!(claude.get(&format!("skills/{path}")), Some(body), "{path} 가 Claude 의 트리와 다르다");
        }
        assert_eq!(shared.len() + 2, claude.len(), "매니페스트 둘 말고 다른 것이 갈렸다");
        assert!(!shared.keys().any(|p| p.contains(".claude-plugin")), "매니페스트가 .agents 에 섰다");
    }

    /// **심는 SKILL.md 는 Agent Skills 표준의 머리를 지킨다**(agentskills.io/specification, 세 에이전트가 다 받는다).
    /// `name` 은 제 디렉터리 이름과 같고 소문자·숫자·붙임표 64자 안이며 붙임표로 열거나 닫거나 겹치지 않는다.
    /// `description` 은 비지 않고 1024자 안이다. 어기면 그 에이전트가 스킬을 조용히 안 싣는다 — Claude 만 보던
    /// 판에는 이 금이 글로만 있었다.
    #[test]
    fn every_skill_meets_the_agent_skills_frontmatter() {
        for s in skills() {
            let (first, body) = &s.files[0];
            assert_eq!(*first, "SKILL.md", "{} 의 첫 글이 SKILL.md 가 아니다", s.name);
            let head = body.strip_prefix("---\n").and_then(|b| b.split_once("\n---\n")).map(|(h, _)| h);
            let head = head.unwrap_or_else(|| panic!("{} 에 머리(---)가 없다", s.name));
            let field = |k: &str| head.lines().find_map(|l| l.strip_prefix(&format!("{k}: "))).map(str::trim);
            let name = field("name").unwrap_or_else(|| panic!("{} 의 머리에 name 이 없다", s.name));
            assert_eq!(name, s.name, "머리의 name 이 디렉터리 이름과 다르다");
            let ok = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
            assert!(
                name.len() <= 64 && name.chars().all(ok) && !name.starts_with('-') && !name.ends_with('-'),
                "{name} 이 표준의 name 꼴이 아니다"
            );
            assert!(!name.contains("--"), "{name} 에 붙임표가 겹친다");
            let description = field("description").unwrap_or_else(|| panic!("{name} 의 머리에 description 이 없다"));
            let n = description.chars().count();
            assert!((1..=1024).contains(&n), "{name} 의 description 이 {n}자다 — 1~1024자");
        }
    }

    /// **판은 내용에서 나온다.** 같은 내용이면 같은 판이라 헛 업데이트가 없고,
    /// 한 글자라도 다르면 반드시 달라진다 — 손으로 세면 반드시 어긋난다.
    #[test]
    fn the_version_is_the_content() {
        let a = tree_of("/bin/moai", "# 스킬");
        let b = tree_of("/bin/moai", "# 스킬");
        let c = tree_of("/bin/moai", "# 스킬 (고침)");
        let d = tree_of("/usr/local/bin/moai", "# 스킬");
        let version = |f: &BTreeMap<String, String>| {
            let v: serde_json::Value = serde_json::from_str(&f[".claude-plugin/plugin.json"]).unwrap();
            v["version"].as_str().unwrap().to_string()
        };
        assert_eq!(version(&a), version(&b), "같은 내용인데 판이 다르다");
        assert_ne!(version(&a), version(&c), "본문이 달라졌는데 판이 같다");
        assert_ne!(version(&a), version(&d), "부를 바이너리가 달라졌는데 판이 같다");
        let e = keyed(tree("t", Path::new("/repo"), "/bin/moai", &fake("# 스킬", "감독 (고침)", "위키")));
        assert_ne!(version(&a), version(&e), "감독 스킬이 달라졌는데 판이 같다");
        let f = keyed(tree("t", Path::new("/repo"), "/bin/moai", &fake("# 스킬", "감독", "위키 (고침)")));
        assert_ne!(version(&a), version(&f), "위키 스킬이 달라졌는데 판이 같다");
        // semver 세 자리여야 `claude` 가 읽는다.
        assert_eq!(version(&a).split('.').count(), 3, "{}", version(&a));
    }

    /// **매니페스트의 틀도 판에 든다.** 딸린 파일과 훅 명령만 셈하던 판은
    /// `timeout` 이나 `description` 만 바꾸면 판이 그대로여서, `claude` 가 옛
    /// 복사를 계속 썼다. 셈에 넣는 틀이 실제로 심는 매니페스트와 판 한 자리만
    /// 다른지도 본다 — 다른 틀을 셈하면 이 시험은 통과해도 구멍은 그대로다.
    #[test]
    fn the_manifest_template_is_in_the_version() {
        let files = tree_of("/bin/moai", "# 스킬");
        let mut shipped: serde_json::Value = serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        shipped["version"] = "".into();
        assert_eq!(pretty(&shipped), plugin_json("/bin/moai", ""), "셈한 틀이 심는 매니페스트와 다르다");

        let rest: Vec<(PathBuf, String)> = files
            .into_iter()
            .filter(|(p, _)| p != ".claude-plugin/plugin.json")
            .map(|(p, b)| (PathBuf::from(p), b))
            .collect();
        let template = plugin_json("/bin/moai", "");
        let tweaked = template.replace("\"timeout\": 15", "\"timeout\": 30");
        assert_ne!(template, tweaked, "시험이 틀을 못 바꿨다");
        assert_ne!(version_of(&rest, &template), version_of(&rest, &tweaked), "틀이 바뀌었는데 판이 같다");
    }

    /// **마켓플레이스 이름은 저장소마다 다르다.** 이름은 기계 하나에서
    /// 전역이라, 고정 이름이면 둘째 저장소가 첫째의 트리를 가져간다 — 시험
    /// 저장소가 이 저장소의 플러그인을 설치하는 것을 실제로 보았다.
    #[test]
    fn two_repos_do_not_share_a_marketplace() {
        let one = tree_for("alpha", "/bin/moai", "# 스킬");
        let two = tree_for("beta", "/bin/moai", "# 스킬");
        let name = |f: &BTreeMap<String, String>| {
            let v: serde_json::Value = serde_json::from_str(&f[".claude-plugin/marketplace.json"]).unwrap();
            v["name"].as_str().unwrap().to_string()
        };
        assert!(name(&one).starts_with("moai-alpha-"), "{}", name(&one));
        assert_ne!(name(&one), name(&two));

        // **접두어가 같아도 자리가 다르면 이름이 다르다.** 접두어는 디렉터리
        // 이름에서 나오므로 `~/work/api` 와 `~/old/api` 가 쉽게 겹친다.
        let here = tree_at("api", Path::new("/work/api"), "/bin/moai", "# 스킬");
        let there = tree_at("api", Path::new("/old/api"), "/bin/moai", "# 스킬");
        assert_ne!(name(&here), name(&there), "접두어가 같다고 이름까지 같다");
    }

    /// **이름으로 적을 때도 PATH 를 본다.** `[ -x "moai" ]` 는 PATH 가 아니라
    /// `./moai` 를 보므로, PATH 에 moai 가 있는 남의 기계에서 훅이 전부 조용히
    /// 빠진다 — 그 모습은 "규칙이 통과했다" 와 구별되지 않는다. 이 저장소에서
    /// 그 검사가 참으로 보였던 것도 하필 `moai` 라는 디렉터리가 있어서였다.
    #[test]
    fn a_bare_name_is_looked_up_on_the_path() {
        let cmd = command("moai", "stop");
        assert!(cmd.contains("command -v"), "PATH 를 안 본다 — {cmd}");
        assert!(!cmd.contains("[ -x"), "상대 경로를 본다 — {cmd}");
    }

    /// **아무도 fd 1 을 물려받지 않는다**(리뷰 moai-514e.0er). [`command`] 의 `o=$(…)` 는 `moai` 가
    /// 끝날 때가 아니라 **쓰기 끝을 쥔 것이 다 닫힐 때** 끝난다 — `cmd::merge_driver` 의 `Said` 가
    /// 파이프를 버리고 파일로 간 것과 같은 자리다. `Stdio::inherit()` 을 쓰는 자식이 하나라도 생기면
    /// 그 손자가 사는 동안 훅이 안 끝나고, 매니페스트 `timeout` 15초가 도구 호출마다 선다.
    ///
    /// 지금은 모든 자식이 `Stdio::piped()`·`Stdio::from(파일)` 이라 깨끗하다. **행동으로는 못 잡으니**
    /// (안 걸리면 아무 시험도 안 붉어지고, 걸리면 15초씩 멈춘다) 소스를 글자로 훑는다 —
    /// `main.rs` 의 `the_notices_never_reach_stdout_and_print_stands_last` 와 같은 자다.
    #[test]
    fn nothing_hands_its_stdout_down_to_a_child() {
        let mut dirs = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        let mut found = Vec::new();
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for (n, line) in text.lines().enumerate() {
                        // 주석에 이름을 대는 줄은 센 자리가 아니다 — 이 시험의 까닭이 거기 적혀 있다.
                        let code = line.trim_start();
                        if line.contains(concat!("Stdio", "::inherit"))
                            && !code.starts_with("//")
                            && !code.starts_with("///")
                        {
                            found.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        assert!(found.is_empty(), "자식이 fd 1 을 물려받으면 훅의 `o=$(…)` 가 timeout 까지 멈춘다 — {found:#?}");
    }

    /// 훅은 **없으면 조용히 0** 이다. 한 번은 이 가드가 없어 세션 하나가
    /// 통째로 잠겼다 — 훅이 실패하자 `Bash` 도 `Write` 도 안 돌았다.
    #[test]
    fn a_missing_binary_never_makes_noise() {
        let files = tree_of("/nowhere/moai", "# 스킬");
        let v: serde_json::Value = serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        let mut seen = 0;
        for (_, groups) in v["hooks"].as_object().unwrap() {
            for g in groups.as_array().unwrap() {
                for h in g["hooks"].as_array().unwrap() {
                    let cmd = h["command"].as_str().unwrap();
                    // **`|| exit 0` 으로 잰다 — 맨 `exit 0` 이 아니다**(리뷰 moai-j4ie). 줄 끝의
                    // 무조건 `exit 0` 이 선 뒤로는 맨 낱말로 재면 가드를 통째로 지워도 푸르다.
                    assert!(cmd.contains("|| exit 0"), "없을 때 조용히 빠지는 가드가 없다 — {cmd}");
                    // 경로가 따옴표에 싸이므로 `moai" hook ...` 모양이다.
                    assert!(cmd.contains("/nowhere/moai"), "엉뚱한 것을 부른다 — {cmd}");
                    assert!(cmd.contains("command -v"), "있는지부터 안 본다 — {cmd}");
                    assert!(cmd.contains(" hook "), "훅을 안 부른다 — {cmd}");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, HOOKS.len(), "훅 수가 안 맞는다");
    }

    /// 훅 줄을 **실제 껍데기로 돌려** 갈래마다 무엇이 나가는지 잰다(moai-j4ie).
    ///
    /// 여섯을 가른다 — 없는 것(조용히 0), 못 도는 것(알림 한 줄), 돌고 진 것(1: 그대로 삼킨다),
    /// **판정을 쓴 뒤에 죽은 것**(알림을 안 덧붙인다, moai-mnhq), 도는 것(stdout 이 꼬리의
    /// 줄바꿈 하나 말고는 안 달라진다), **받는 쪽이 먼저 닫은 것**(시그널에 안 죽는다, 리뷰
    /// moai-514e.0er). 꼴만 견주는 시험은 `&&` 와 `;` 를 못 가리는데,
    /// 옛 줄이 126 을 삼킨 까닭이 바로 그 한 글자였다. **종료 코드는 어느 갈래에서도 0 이다** —
    /// 여기가 게이트가 되는 순간 훅 바이너리 권한 하나로 세션의 모든 도구 호출이 멈춘다.
    ///
    /// **못 도는 판은 실행 비트를 빼서 짓는다 — 126 을 내는 글이 아니다**(리뷰 moai-j4ie).
    /// `0o755` 로 심고 `exit 126` 하는 글은 어느 껍데기에서나 `command -v` 를 지나므로 겨눈 그
    /// 판(껍데기가 exec 을 거절하는 판)을 안 잰다. 로더가 내는 127 은 없는 해석기로 짓는다.
    ///
    /// **껍데기를 하나로 두지 않는다.** `command -v` 의 답이 dash 와 bash 에서 갈려(`[ -e ]` 를
    /// 곁들인 까닭, [`command`] 참조), `sh` 하나로 재면 `/bin/sh` 가 dash 인 기계에서만 푸르다.
    /// **`set -e` 로도 한 번 돌린다** — 그 껍데기에서 맨 명령 하나가 지면 `exit 0` 에 못 닿는다.
    #[cfg(unix)]
    #[test]
    fn a_binary_that_cannot_run_says_so_without_blocking() {
        let scratch = crate::scratch::Scratch::new("skill-hookline");
        let at = scratch.path();
        // **복사로 심는다** — 까닭(ETXTBSY)은 [`planted`] 에 있다.
        let plant = |name: &str, body: &str, mode: u32| planted(at, name, body, mode).display().to_string();
        // **판마다 `TMPDIR` 을 새로 준다**(moai-f7up) — 알림의 문턱이 표식 파일 하나라, 한
        // 자리에서 이어 돌리면 첫 판만 말하고 나머지 단언이 모두 빈 stdout 을 본다. 문턱 자체는
        // 아래 `the_notice_stands_once_a_session` 이 같은 자리로 두 번 돌려 잰다.
        let nth = std::cell::Cell::new(0);
        let run = |shell: &str, flags: &str, exe: &str| {
            nth.set(nth.get() + 1);
            let tmp = at.join(format!("tmp{}", nth.get()));
            std::fs::create_dir_all(&tmp).unwrap();
            let out = std::process::Command::new(shell)
                .args([flags, &command(exe, "pre-tool-use")])
                .env("TMPDIR", &tmp)
                .output()
                .unwrap_or_else(|e| panic!("{shell}: {e}"));
            (out.status.code(), String::from_utf8_lossy(&out.stdout).to_string())
        };
        // **stdout 이 못 쓰는 자리일 때를 재는 자**(리뷰 moai-514e.0er). 위 `run` 은 `output()` 이라
        // 늘 stdout 을 비워 주어 이 갈래를 못 잰다. 둘을 가른다 — 파이프의 **읽기 끝을 떨어뜨리면**
        // 껍데기가 진짜 SIGPIPE 를 맞고(`trap` 이 맡는다), **읽기 전용 fd** 를 물려주면 시그널 없이
        // `printf` 가 비영으로 끝난다(`|| :` 가 맡는다). 뒤엣것은 `set -e` 에서만 드러난다.
        let closed = |shell: &str, flags: &str, exe: &str, signal: bool| {
            nth.set(nth.get() + 1);
            let tmp = at.join(format!("tmp{}", nth.get()));
            std::fs::create_dir_all(&tmp).unwrap();
            let sink = if signal {
                std::process::Stdio::piped()
            } else {
                // 읽기로 연 파일을 stdout 으로 준다 — 쓰기가 `EBADF` 로 지고 시그널은 안 온다.
                std::process::Stdio::from(std::fs::File::open(exe).unwrap())
            };
            let mut child = std::process::Command::new(shell)
                .args([flags, &command(exe, "pre-tool-use")])
                .env("TMPDIR", &tmp)
                .stdout(sink)
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap_or_else(|e| panic!("{shell}: {e}"));
            drop(child.stdout.take());
            child.wait().unwrap_or_else(|e| panic!("{shell}: {e}")).code()
        };

        // **도는 때의 계약은 "꼬리의 줄바꿈 하나로 고른다" 다**(moai-mnhq 가 좁혔다, 리뷰
        // moai-514e.0er 가 이 줄을 다시 적었다). 옛 계약은 "한 글자도 안 달라진다" 였고 옛 `live`
        // 가 그것을 못박았는데, `o=$(…)` 가 꼬리의 줄바꿈을 **몇 개든 걷고** 한 줄이 하나를 다시
        // 달면서 그 계약이 못 선다. 지금 서는 것은 이것이다 — 가운데는 한 글자도 안 달라지고,
        // 꼬리는 줄바꿈 하나로 고른다. 넷이 그 네 꼴을 나눠 잰다: `live`(하나로 끝난다 — 안
        // 달라진다), `bare`(없이 끝난다 — 하나를 얻는다), `blank`(둘로 끝난다 — 하나로 준다),
        // `pair`(가운데 줄바꿈은 그대로 산다).
        //
        // **`moai` 가 내는 꼴은 `live` 다** — `cmd::hook::run` 은 어느 갈래에서도 한 줄을 내고
        // `print` 가 줄바꿈 하나를 단다. 그래서 실제로는 바이트가 안 달라진다.
        //
        // 이 줄이 **문지기이기도 하다**: `TMPDIR` 이 `noexec` 으로 얹힌 자리에서는 여기서
        // 실행을 못 재므로 아래를 실패로 세지 않는다(`cmd::runnable` 의 시험과 같은 자다).
        let live = plant("live", "#!/bin/sh\nprintf '%s\\n' '{\"ok\":true}'\n", 0o755);
        // 줄바꿈 없이 끝나는 출력과 여러 줄짜리 출력. 앞은 줄바꿈 하나를 얻고(JSON 줄에는 뜻이
        // 안 달라진다), 뒤는 가운데 줄바꿈이 그대로 산다 — 걷히는 것은 **꼬리**뿐이다.
        let bare = plant("bare", "#!/bin/sh\nprintf '%s' '{\"ok\":true}'\n", 0o755);
        let pair = plant("pair", "#!/bin/sh\nprintf '%s\\n%s\\n' '{\"a\":1}' '{\"b\":2}'\n", 0o755);
        // **꼬리의 줄바꿈은 몇 개든 걷힌다**(리뷰 moai-514e.0er) — 명령 치환이 끝의 줄바꿈을 **다**
        // 걷고 이 줄이 하나를 다시 단다. `moai` 의 훅 출력은 늘 한 줄이라 지금은 안 닿지만, 빈
        // 줄로 끝나는 갈래가 하나라도 생기면 그 바이트가 말없이 준다. 옛 `live` 가 못박던
        // "한 글자도 안 달라진다" 를 이 자리가 대신 적는다.
        let blank = plant("blank", "#!/bin/sh\nprintf '%s\\n\\n' '{\"ok\":true}'\n", 0o755);
        if !crate::cmd::runnable(std::path::Path::new(&live)) {
            return;
        }
        // 껍데기는 [`shells`] 가 고른다 — `dash` 를 이름으로 부르는 까닭도 거기 있다.

        // 못 도는 판 둘 — 실행 비트가 빠진 파일(껍데기가 126)과 없는 해석기(로더가 127).
        let dead = plant("dead", "#!/bin/sh\nexit 0\n", 0o644);
        let gone = plant("gone", "#!/nowhere/interp\nexit 0\n", 0o755);
        // **판정을 못 낸 나머지 값들**(moai-wnnb) — clap 이 모르는 부명령에 내는 2, 패닉의 101,
        // 시그널의 128+N 이다. `moai` 의 `main` 은 0·1 밖에 못 내므로 그 밖의 값은 모두 여기다.
        // 세 글 다 stdout 에 한 글자도 안 쓴다 — 쓰고 죽은 때는 아래 `spoke`·`felled` 가 따로 잰다.
        let clap = plant("clap", "#!/bin/sh\nexit 2\n", 0o755);
        let panic = plant("panic", "#!/bin/sh\nexit 101\n", 0o755);
        let killed = plant("killed", "#!/bin/sh\nkill -9 $$\n", 0o755);
        // 돌고 진 판은 제 stdout 이 우리 것이 아니다 — 판정 JSON 뒤에 둘째 객체를 붙이면
        // 그 판정(`deny` 까지)이 파싱에서 통째로 버려진다.
        let lost = plant("lost", "#!/bin/sh\necho '{\"hookSpecificOutput\":{}}'\nexit 1\n", 0o755);
        // **판정을 쓴 뒤에 죽는 때**(moai-mnhq) — 위의 `panic`·`killed` 와 종료 값은 같은데
        // stdout 에 이미 `deny` 가 섰다. 여기에 알림을 덧붙이면 막아야 할 쓰기가 통과한다.
        let deny = "{\"hookSpecificOutput\":{\"permissionDecision\":\"deny\"}}";
        let spoke = plant("spoke", &format!("#!/bin/sh\necho '{deny}'\nexit 101\n"), 0o755);
        let felled = plant("felled", &format!("#!/bin/sh\necho '{deny}'\nkill -9 $$\n"), 0o755);
        let nowhere = at.join("nowhere").display().to_string();
        // **꼴이 판정이 아닌 글은 판정이 아니다**(moai-45hf.3do, 2026-09-29 사용자 결정). 비었는지만
        // 보던 줄은 셋을 판정으로 읽어 그대로 흘려보내고 알림을 접었다 — PIPE_BUF 를 넘는 줄을 쓰다
        // SIGKILL 을 맞은 토막, PATH 의 남의 `moai` 가 찍은 사용법 글, 빈칸 하나다. `claude` 는
        // 셋 다 못 읽고 쓰기는 돈다. 이제 셋 다 버리고 알림을 낸다.
        let torn = plant("torn", "#!/bin/sh\nprintf '%s' '{\"hookSpecificOutp'\nkill -9 $$\n", 0o755);
        let foreign = plant("foreign", "#!/bin/sh\necho 'usage: moai <command>'\nexit 64\n", 0o755);
        let space = plant("space", "#!/bin/sh\nprintf ' '\nexit 2\n", 0o755);

        for sh in shells() {
            for (exe, code) in [
                (&dead, "126"),
                (&gone, "127"),
                (&clap, "2"),
                (&panic, "101"),
                (&killed, "137"),
                (&torn, "137"),
                (&foreign, "64"),
                (&space, "2"),
            ] {
                let (got, said) = run(sh, "-c", exe);
                assert_eq!(got, Some(0), "{sh}: 못 도는 판이 게이트가 됐다 — {said}");
                let v: serde_json::Value =
                    serde_json::from_str(said.trim()).unwrap_or_else(|e| panic!("{sh}: {e} — {said:?}"));
                let notice = v["systemMessage"].as_str().unwrap_or_default();
                assert!(notice.contains("pre-tool-use"), "{sh}: 어느 훅인지 안 댄다 — {notice}");
                // **꼴째로 견준다**(리뷰 moai-514e.hgz) — 알림이 이제 경로까지 싣고, 스크래치
                // 이름에는 pid 와 ThreadId 가 들어 숫자가 늘 있다. 맨 `"2"` 를 찾던 판은 그
                // 경로에 걸려, `case` 가 2 를 놓치게 되어도 푸르게 지나갔다.
                assert!(notice.contains(&format!("(exit {code})")), "{sh}: 종료 값을 안 댄다 — {notice}");
                // **어느 파일인지도 댄다**(moai-wza7) — 그 말이 대는 `moai skill status` 가
                // 그 판에서는 같이 못 돌 수 있다.
                assert!(notice.contains(exe.as_str()), "{sh}: 어느 파일인지 안 댄다 — {notice}");
                // 그 말을 그대로 쳐서 답이 나와야 한다 — 맨 `moai skill` 은 clap 이 거절한다.
                assert!(notice.contains("moai skill status"), "{sh}: 못 치는 명령을 댄다 — {notice}");
            }

            let (got, said) = run(sh, "-c", &lost);
            assert_eq!(got, Some(0), "{sh}: 진 판이 게이트가 됐다");
            assert_eq!(said, "{\"hookSpecificOutput\":{}}\n", "{sh}: 1 로 진 판에 한 줄을 덧붙였다 — {said}");

            // **판정을 쓴 뒤에 죽은 때에도 안 덧붙인다**(moai-mnhq). 종료 값만 보던 줄은 여기에
            // 둘째 객체를 얹었고, 그러면 `claude` 가 두 객체를 보고 **둘 다 버려** 막아야 할
            // 쓰기가 통과했다(2026-09-23 에 진짜 `claude` 로 측정했다). stdout 이 판정의 꼴(`{…}`)이
            // 아닐 때만 말한다(moai-45hf.3do — 비었는지만 보던 것을 고쳤다).
            for (exe, why) in [(&spoke, "101"), (&felled, "시그널")] {
                let (got, said) = run(sh, "-c", exe);
                assert_eq!(got, Some(0), "{sh}: {why} 로 죽은 때가 게이트가 됐다 — {said}");
                assert_eq!(said, format!("{deny}\n"), "{sh}: {why} 로 죽은 때가 판정에 한 줄을 덧붙였다 — {said}");
            }

            // 없는 것은 여전히 조용하다.
            let (got, said) = run(sh, "-c", &nowhere);
            assert_eq!((got, said.as_str()), (Some(0), ""), "{sh}: 없는 바이너리가 말을 했다");

            assert_eq!(run(sh, "-c", &live), (Some(0), "{\"ok\":true}\n".to_string()), "{sh}: 계약 JSON 이 달라졌다");
            // 꼬리의 줄바꿈만 골라지고 가운데 줄바꿈은 그대로 산다.
            assert_eq!(run(sh, "-c", &bare), (Some(0), "{\"ok\":true}\n".to_string()), "{sh}: 꼬리가 안 섰다");
            assert_eq!(run(sh, "-c", &pair), (Some(0), "{\"a\":1}\n{\"b\":2}\n".to_string()), "{sh}: 줄이 뭉쳤다");
            // 걷히는 것이 꼬리 **전부**라는 것을 적어 둔다 — 둘로 끝난 출력이 하나로 준다.
            assert_eq!(run(sh, "-c", &blank), (Some(0), "{\"ok\":true}\n".to_string()), "{sh}: 빈 줄이 살았다");

            // **stdout 이 못 쓰는 자리여도 0 이다**(리뷰 moai-514e.0er). `trap 'exit 0' PIPE` 를
            // 걷으면 첫 줄이 `None`(141) 이 되고, `|| :` 를 걷으면 `set -e` 갈래가 1 이 된다 —
            // 셋 다 그랬고 **옛 줄은 같은 자리에서 0 이었다**. 훅의 비영 종료가 무엇을 하는지는
            // 위 표에 2 하나만 잰 채라, 여기서 값을 흘리면 재 보지 않은 자리로 들어간다.
            assert_eq!(closed(sh, "-c", &live, true), Some(0), "{sh}: 받는 쪽이 닫힌 때에 시그널로 죽었다");
            for flags in ["-c", "-ec"] {
                assert_eq!(closed(sh, flags, &live, false), Some(0), "{sh} {flags}: 쓰기가 진 때에 값을 흘렸다");
                assert_eq!(closed(sh, flags, &dead, false), Some(0), "{sh} {flags}: 알림의 쓰기가 진 때에 값을 흘렸다");
            }

            // **`set -e` 가 선 껍데기에서도 0 이다.** 맨 명령으로 두던 판은 여기서 그대로
            // 죽어, 1 로 진 판은 stdout 의 판정까지 함께 버려졌다.
            let (got, said) = run(sh, "-ec", &lost);
            assert_eq!(got, Some(0), "{sh}: `set -e` 에서 게이트가 됐다 — {said}");
            let (got, said) = run(sh, "-ec", &dead);
            assert_eq!(got, Some(0), "{sh}: `set -e` 에서 못 도는 판이 게이트가 됐다 — {said}");
            assert!(said.contains("126"), "{sh}: `set -e` 에서 알림이 빠졌다 — {said}");
        }
    }

    /// 규칙이 뜻을 두는 도구만 본다. 전부 받으면 읽기만 하는 호출까지
    /// 훅을 한 번씩 띄운다.
    #[test]
    fn only_the_tools_the_rules_care_about_are_watched() {
        let files = tree_of("/bin/moai", "# 스킬");
        let v: serde_json::Value = serde_json::from_str(&files[".claude-plugin/plugin.json"]).unwrap();
        let matcher = v["hooks"]["PreToolUse"][0]["matcher"].as_str().unwrap();
        assert!(matcher.contains("Bash") && matcher.contains("Edit"), "{matcher}");
        assert!(v["hooks"]["Stop"][0].get("matcher").is_none(), "Stop 에 matcher 가 붙었다");
    }

    /// PATH 의 `moai` 가 **같은 파일일 때만** 이름으로 적는다.
    ///
    /// 이 저장소가 그 반례다 — PATH 의 `moai` 는 옛 moai 의 바이너리이고,
    /// 이름만 적으면 훅이 남의 도구를 부른다.
    #[test]
    fn the_name_is_used_only_when_path_agrees() {
        let here = Path::new("/repo/target/release/moai");
        assert_eq!(exe_name(here, here, Some(here)), "moai");
        assert_eq!(exe_name(here, here, Some(Path::new("/usr/bin/moai"))), "/repo/target/release/moai");
        assert_eq!(exe_name(here, here, None), "/repo/target/release/moai");
        // **견주는 것은 푼 자리다**(moai-gu5m, 리뷰 moai-gu5m.ke0). 부른 철자는 링크가 안 풀렸고 PATH 쪽은
        // `which` 가 푼 자리라, 철자로 견주던 판으로 되돌리면 같은 파일을 남의 것으로 읽어 절대 경로를 적는다.
        let linked = Path::new("/home/me/link/release/moai");
        assert_eq!(exe_name(linked, here, Some(here)), "moai");
        assert_eq!(exe_name(linked, here, None), "/home/me/link/release/moai", "적는 것은 부른 철자다");
    }

    /// **부른 철자는 링크를 안 풀고 `.`·`..` 만 접는다**(moai-gu5m). 이름뿐인 `argv[0]` 은 철자를 안
    /// 낸다 — PATH 의 같은 파일이면 [`exe_name`] 이 이름으로 적는다.
    ///
    /// **글자로 견준다**(리뷰 moai-gu5m.ke0) — `PathBuf` 의 `==` 는 조각으로 돌아 가운데 `.` 을 버려,
    /// 안 접은 철자도 같다고 한다(`crate::path` 의 시험이 적어 둔 덫이다).
    #[test]
    fn the_spelling_is_argv0_folded_against_cwd() {
        let cwd = Path::new("/repo/.claude/worktrees/w");
        let at = |argv0: &str| spelled(Some(Path::new(argv0)), Some(cwd)).map(|p| p.display().to_string());
        assert_eq!(at("../../../target/release/moai").as_deref(), Some("/repo/target/release/moai"));
        assert_eq!(at("./target/release/moai").as_deref(), Some("/repo/.claude/worktrees/w/target/release/moai"));
        assert_eq!(at("/repo/./target/release/moai").as_deref(), Some("/repo/target/release/moai"));
        assert_eq!(at("/repo/scripts/../target/release/moai").as_deref(), Some("/repo/target/release/moai"));
        assert_eq!(at("moai"), None);
        assert_eq!(spelled(None, Some(cwd)), None);
        // `cwd` 를 못 읽으면 상대 철자는 붙일 데가 없다. 절대 철자는 그대로 선다.
        assert_eq!(spelled(Some(Path::new("target/release/moai")), None), None);
        assert_eq!(spelled(Some(Path::new("/x/moai")), None), Some(PathBuf::from("/x/moai")));
    }

    /// **훅 한 줄에 못 적는 철자는 안 낸다**(리뷰 moai-gu5m.ke0) — 그러면 부르는 쪽이 푼 자리로
    /// 떨어진다. 따옴표를 깨는 철자를 내면 [`exe_name`] 이 이름으로 물러서 훅이 PATH 의 딴 moai 를
    /// 부르고, UTF-8 이 아닌 철자는 `display` 가 없는 자리로 바꿔 적는다. 프로세스마다 다른 것을
    /// 가리키는 자리(`/proc`·`/dev`)는 훅의 셸에서 이 파일이 아니다.
    #[test]
    fn a_spelling_the_hook_cannot_carry_is_not_offered() {
        let cwd = Path::new("/repo");
        let at = |argv0: &Path| spelled(Some(argv0), Some(cwd));
        for unwritable in ["/home/we$ird/moai", "../we\"ird/moai", "/home/back\\slash/moai", "/home/tick`/moai"] {
            assert_eq!(at(Path::new(unwritable)), None, "{unwritable}: 못 적는 철자를 냈다");
        }
        for per_process in ["/proc/self/exe", "/proc/4242/exe", "/dev/fd/9", "/proc/self/cwd/target/release/moai"] {
            assert_eq!(at(Path::new(per_process)), None, "{per_process}: 프로세스마다 다른 자리를 냈다");
        }
        // `/process` 는 `/proc` 아래가 아니다 — 조각으로 견준다.
        assert!(at(Path::new("/process/moai")).is_some(), "이름이 `/proc` 으로 시작할 뿐인 자리를 버렸다");
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt as _;
            let lossy = Path::new(std::ffi::OsStr::from_bytes(b"/home/caf\xe9/moai"));
            assert_eq!(at(lossy), None, "UTF-8 이 아닌 철자를 냈다");
        }
    }

    /// **이 저장소의 설치만 고른다.** 같은 이름이 옛 자리에 남은 `local` 줄을
    /// 제 것으로 걷으면 남의 설정을 건드린다. `user` 는 자리가 없으니 이름으로
    /// 족하다.
    #[test]
    fn only_this_repos_installs_are_picked() {
        let ledger = serde_json::json!({"plugins": {
            "moai@m": [
                {"scope": "local", "projectPath": "/repo", "version": "1.2.3", "installPath": "/c/1.2.3"},
                {"scope": "local", "projectPath": "/old/repo", "version": "0.0.1", "installPath": "/c/0.0.1"},
                {"scope": "user", "version": "4.5.6", "installPath": "/c/4.5.6"},
            ],
            "moai@other": [{"scope": "user", "version": "9.9.9", "installPath": "/c/9"}],
        }});
        let got = installs_of(&ledger, "moai@m", |p| p == "/repo");
        let versions: Vec<&str> = got.iter().map(|i| i.version.as_str()).collect();
        assert_eq!(versions, ["1.2.3", "4.5.6"]);
        assert!(installs_of(&serde_json::json!({}), "moai@m", |_| true).is_empty(), "빈 장부에서 무언가 골랐다");
    }

    /// **주소로 더한 GitHub 도 그 저장소다**(리뷰 moai-5wk4.76z). `repo` 만 읽던 판은 `git` 출처를 빈 글로
    /// 읽어 남의 출처로 보았다. GitHub 가 아닌 주소는 여전히 빈 글이다.
    #[test]
    fn a_github_url_marketplace_names_its_repo() {
        let known = serde_json::json!({
            "a": {"source": {"source": "github", "repo": "daleseo/korean-skills"}},
            "b": {"source": {"source": "git", "url": "https://github.com/DaleSeo/korean-skills.git"}},
            "c": {"source": {"source": "git", "url": "git@github.com:DaleSeo/korean-skills"}},
            "d": {"source": {"source": "git", "url": "https://gitlab.com/DaleSeo/korean-skills.git"}},
            "e": {"source": {"source": "directory", "path": "/x"}},
        });
        assert_eq!(market_repo(&known, "a").as_deref(), Some("daleseo/korean-skills"));
        assert_eq!(market_repo(&known, "b").as_deref(), Some("DaleSeo/korean-skills"));
        assert_eq!(market_repo(&known, "c").as_deref(), Some("DaleSeo/korean-skills"));
        assert_eq!(market_repo(&known, "d").as_deref(), Some(""));
        assert_eq!(market_repo(&known, "e").as_deref(), Some(""));
        assert_eq!(market_repo(&known, "z"), None);
    }

    /// **`claude` 가 적은 꼴에서 그 멤버의 줄만 빠진다**(moai-6ugu.aae) — 끝 멤버면 앞 줄의 쉼표, 가운데면 제
    /// 쉼표, 하나뿐이면 `{}`. 다른 바이트는 한 글자도 안 바뀐다.
    #[test]
    fn dropping_a_marketplace_takes_only_its_lines() {
        let head = "{\n  \"enabledPlugins\": {\n    \"a@b\": true\n  },\n  \"extraKnownMarketplaces\": {\n";
        let keep = "    \"keepme\": {\n      \"source\": {\n        \"source\": \"github\",\n        \"repo\": \"a/b\"\n      }\n    }";
        let ks = "    \"korean-skills\": {\n      \"source\": {\n        \"source\": \"github\",\n        \"repo\": \"DaleSeo/korean-skills\"\n      }\n    }";
        let tail = "\n  }\n}\n";
        let last = format!("{head}{keep},\n{ks}{tail}");
        assert_eq!(drop_marketplace(&last, "korean-skills"), Some(format!("{head}{keep}{tail}")));
        let first = format!("{head}{ks},\n{keep}{tail}");
        assert_eq!(drop_marketplace(&first, "korean-skills"), Some(format!("{head}{keep}{tail}")));
        let middle = format!("{head}{keep},\n{ks},\n{}{tail}", keep.replace("keepme", "other"));
        assert_eq!(
            drop_marketplace(&middle, "korean-skills"),
            Some(format!("{head}{keep},\n{}{tail}", keep.replace("keepme", "other")))
        );
        let alone = format!("{head}{ks}{tail}");
        assert_eq!(
            drop_marketplace(&alone, "korean-skills"),
            Some("{\n  \"enabledPlugins\": {\n    \"a@b\": true\n  },\n  \"extraKnownMarketplaces\": {}\n}\n".into())
        );
        // CRLF 도 줄째 빠진다.
        let crlf = last.replace('\n', "\r\n");
        assert_eq!(drop_marketplace(&crlf, "korean-skills"), Some(format!("{head}{keep}{tail}").replace('\n', "\r\n")));
    }

    /// 한 줄에 몰린 꼴은 그 멤버와 쉼표 하나만 빠진다. 글자열 안의 괄호·따옴표는 건넌다.
    #[test]
    fn dropping_a_marketplace_from_one_line() {
        let text = r#"{"x":"}\"{","extraKnownMarketplaces":{"a":{"s":"]"}, "im-not-ai":{"source":{}},"b":1}}"#;
        assert_eq!(
            drop_marketplace(text, "im-not-ai").as_deref(),
            Some(r#"{"x":"}\"{","extraKnownMarketplaces":{"a":{"s":"]"}, "b":1}}"#)
        );
        assert_eq!(
            drop_marketplace(r#"{"extraKnownMarketplaces":{"a":1, "im-not-ai":2 }}"#, "im-not-ai").as_deref(),
            Some(r#"{"extraKnownMarketplaces":{"a":1}}"#)
        );
    }

    /// **그 키 하나만 빠진 값이 아니면 안 쓴다** — 못 읽는 글, 없는 이름, 같은 키가 둘 선 글.
    #[test]
    fn dropping_a_marketplace_refuses_what_it_cannot_prove() {
        for text in [
            "// note\n{\"extraKnownMarketplaces\": {\"k\": 1}}",
            "{\"extraKnownMarketplaces\": {\"other\": 1}}",
            "{\"extraKnownMarketplaces\": {\"k\": 1, \"k\": 2}}",
            "{\"enabledPlugins\": {}}",
            "[]",
        ] {
            assert_eq!(drop_marketplace(text, "k"), None, "{text}");
        }
    }

    #[test]
    fn a_settings_file_names_what_it_declares_and_enables() {
        let settings = serde_json::json!({
            "enabledPlugins": {"korean-skills@korean-skills": false, "x@korean-skills-fork": true},
            "extraKnownMarketplaces": {"korean-skills": {"source": {"source": "github", "repo": "DaleSeo/korean-skills"}}},
        });
        assert_eq!(declared_repo(&settings, "korean-skills").as_deref(), Some("DaleSeo/korean-skills"));
        assert_eq!(declared_repo(&settings, "im-not-ai"), None);
        assert_eq!(plugins_from(&settings, "korean-skills"), ["korean-skills@korean-skills"]);
        assert!(plugins_from(&serde_json::json!({}), "korean-skills").is_empty());
    }

    /// 매니페스트에서 훅이 부르는 실행 파일을 **심은 그대로** 꺼낸다 — 이름이든
    /// 절대 경로든.
    #[test]
    fn the_hook_exe_round_trips_through_the_manifest() {
        for exe in ["/repo/target/release/moai", "moai"] {
            let files = tree("t", Path::new("/repo"), exe, &fake("# 스킬", "감독", "위키"));
            let (_, manifest) = files.iter().find(|(p, _)| p.ends_with("plugin.json")).unwrap();
            assert_eq!(hook_exe(manifest).as_deref(), Some(exe));
            assert!(version_in(&files).is_some_and(|v| v.split('.').count() == 3));
        }
        assert_eq!(hook_exe("{ 깨진 json"), None);
    }

    /// 따옴표를 깨는 경로에는 절대 경로를 안 쓴다. 셸 한 줄이 깨지면 그
    /// 세션의 모든 도구 호출이 막힌다.
    #[test]
    fn a_hostile_path_falls_back_to_the_name() {
        let cmd = command("/tmp/\"moai\"", "stop");
        assert!(!cmd.contains("/tmp/"), "그 경로가 그대로 들어갔다 — {cmd}");
        assert!(cmd.contains("\"moai\" hook stop"), "이름으로도 안 부른다 — {cmd}");
        // **판과 출력도 같은 이름을 말한다.** 매니페스트만 이름으로 바꾸면 설치
        // 출력과 `status` 는 절대 경로를, 훅은 PATH 의 moai 를 가리킨다.
        let named = |p: &str| exe_name(Path::new(p), Path::new(p), None);
        assert_eq!(named("/tmp/we$ird/moai"), "moai");
        assert_eq!(named("/tmp/plain/moai"), "/tmp/plain/moai");

        // **제어문자가 든 경로는 그대로 부르되 알림에만 안 싣는다**(moai-wza7, 리뷰
        // moai-514e.hgz 5번). 한 자로 두던 판은 셸에 멀쩡한 경로까지 이름으로 바꿔 적어 훅이
        // PATH 의 딴 moai 를 불렀다 — 이 기계에서 그것은 옛 moai 의 바이너리다.
        for hostile in ["/tmp/두\n줄/moai", "/tmp/탭\t자리/moai", "/tmp/\u{7f}/moai"] {
            assert_eq!(named(hostile), hostile, "부를 자리를 이름으로 바꿔 적었다");
            let cmd = command(hostile, "stop");
            assert!(cmd.contains(&format!("\"{hostile}\" hook stop")), "그 경로를 안 부른다 — {cmd:?}");
            // 알림에는 그 경로가 안 든다 — JSON 문자열 안의 날 제어문자 하나면 `claude` 가
            // 그 객체를 통째로 버려, 알리려던 말이 도리어 사라진다.
            let (_, notice) = cmd.split_once("systemMessage").expect("알림이 없다");
            assert!(
                !notice.contains('\n') && !notice.contains('\t') && !notice.contains('\u{7f}'),
                "알림에 날 제어문자가 들어갔다 — {notice:?}"
            );
        }
    }

    /// **알림이 어느 파일을 못 돌렸는지 댄다**(moai-wza7). 그 말이 대는 `moai skill status` 는
    /// 방금 exec 에 실패한 바로 그 바이너리라 그 판에서는 같이 못 돌 수 있다 — 경로가 있으면
    /// 사람이 그 자리를 바로 본다.
    ///
    /// 꼴에 박지 않고 인자로 넘기는 것까지 잰다 — 경로의 `%` 가 변환 문자로 읽히면 그 줄이
    /// 엉뚱한 글을 낸다.
    #[test]
    fn the_notice_names_the_binary_it_could_not_run() {
        let cmd = command("/tmp/100%/moai", "pre-tool-use");
        assert!(cmd.contains("'/tmp/100%/moai'"), "경로를 인자로 안 넘긴다 — {cmd}");
        assert!(!cmd.contains("(exit %s): /tmp/"), "경로를 printf 꼴에 박았다 — {cmd}");
    }

    /// **알림은 세션마다 한 번만 선다**(moai-f7up). 문턱이 없던 판은 도구 호출마다 같은 줄을
    /// 냈고 — 이 저장소의 세션 하나가 훅 걸리는 호출을 평균 140번, 많게는 1,257번 낸다 —
    /// 그렇게 떠드는 훅은 사람이 꺼 버린다.
    ///
    /// **같은 자리로 두 번, 새 자리로 한 번 돌려 잰다.** 표식이 서는 자리는 `TMPDIR` 이라,
    /// 그 자리를 바꾸면 다른 세션과 같다. 종료 코드는 세 판 다 0 이어야 한다 — 문턱이
    /// 게이트가 되면 바이너리 권한 하나로 세션이 멈춘다.
    #[cfg(unix)]
    #[test]
    fn the_notice_stands_once_a_session() {
        let scratch = crate::scratch::Scratch::new("skill-hookonce");
        let at = scratch.path();
        // 실행 비트를 뺀 파일 — 껍데기가 126 을 낸다(`a_binary_that_cannot_run…` 과 같은 자).
        // **복사로 심는다**([`planted`]) — 제 손으로 쓴 파일을 바로 돌리면 ETXTBSY 가 126 을 낸다.
        let dead = planted(at, "dead", "#!/bin/sh\nexit 0\n", 0o644);
        // **못 재는 자리인지는 `cmd::runnable` 이 가른다**(리뷰 moai-514e.hgz) — 빈 stdout 을
        // 문지기로 쓰던 판은 "여기서는 exec 을 못 잰다" 와 "알림이 통째로 사라졌다" 를 못 갈라,
        // 알림이 죽은 회차를 푸르게 넘겼다. `a_binary_that_cannot_run…` 이 쓰는 그 문지기다.
        let live = planted(at, "live", "#!/bin/sh\nexit 0\n", 0o755);
        if !crate::cmd::runnable(&live) {
            return;
        }
        let line = command(&dead.display().to_string(), "pre-tool-use");
        // **이벤트가 다르면 표식도 다르다**(리뷰 moai-514e.hgz) — `SessionStart` 의 알림은
        // 사람에게 안 들리는데 그것이 세션의 맨 앞에서 돈다. 키를 하나로 두면 그 한 줄이
        // `PreToolUse` 의 입을 세션 내내 막는다.
        let opening = command(&dead.display().to_string(), "session-start");
        // **종료 값이 다르면 표식도 다르다**(리뷰 moai-514e.hgz) — 설치와 상관없는 한 번짜리
        // 죽음(OOM 의 137, clap 의 2)이 표식을 태우면 뒤에 정말 온 126 이 조용해진다.
        // **한 파일이 값 둘을 낸다** — 바이너리를 갈면 `whose` 가 함께 갈려 종료 값이 아니라
        // 자리를 재게 된다. 두 번째 값은 `TMPDIR` 에 둔 표가 고른다.
        let flaky = planted(at, "flaky", "#!/bin/sh\n[ -e \"$TMPDIR/turned\" ] && exit 101\nexit 2\n", 0o755);
        let once = command(&flaky.display().to_string(), "pre-tool-use");
        // **껍데기를 하나로 두지 않는다**([`shells`]) — 아래 `true` 의 까닭(특수 내장의 리다이렉션
        // 실패)은 dash 에서만 드러나, `sh` 가 bash 인 기계에서 한 벌만 돌리면 푸르게 지나간다.
        let run = |shell: &str, what: &str, tmp: &std::path::Path| {
            let out = std::process::Command::new(shell)
                .args(["-c", what])
                .env("TMPDIR", tmp)
                .output()
                .unwrap_or_else(|e| panic!("{shell}: {e}"));
            (out.status.code(), String::from_utf8_lossy(&out.stdout).to_string())
        };

        for (k, sh) in shells().into_iter().enumerate() {
            let one = at.join(format!("session-one-{k}"));
            std::fs::create_dir_all(&one).unwrap();
            let (code, first) = run(sh, &line, &one);
            assert_eq!(code, Some(0), "{sh}: 첫 번이 게이트가 됐다");
            assert!(first.contains("systemMessage"), "{sh}: 첫 번이 알림을 안 냈다 — {first:?}");

            assert_eq!(run(sh, &line, &one), (Some(0), String::new()), "{sh}: 같은 세션에서 알림이 다시 섰다");

            // **딴 이벤트는 제 표식을 쓴다** — `session-start` 가 먼저 말해도 `pre-tool-use` 는
            // 그 세션에서 제 한 줄을 낸다. 키를 하나로 두면 여기가 빈손이 된다.
            let opened = at.join(format!("session-open-{k}"));
            std::fs::create_dir_all(&opened).unwrap();
            let (code, said) = run(sh, &opening, &opened);
            assert_eq!(code, Some(0), "{sh}: 여는 훅이 게이트가 됐다");
            assert!(said.contains("session-start"), "{sh}: 여는 훅이 알림을 안 냈다 — {said:?}");
            let (code, after) = run(sh, &line, &opened);
            assert_eq!(code, Some(0), "{sh}: 여는 훅 뒤가 게이트가 됐다");
            assert!(
                after.contains("pre-tool-use"),
                "{sh}: 안 들리는 session-start 알림이 pre-tool-use 의 입을 막았다 — {after:?}"
            );

            // **한 번짜리 죽음이 진짜 고장의 입을 막지 않는다.** 같은 파일·같은 이벤트·같은
            // 세션에서 값만 2 → 101 로 바뀌면 둘째 줄이 서야 한다.
            let flip = at.join(format!("session-flip-{k}"));
            std::fs::create_dir_all(&flip).unwrap();
            let (code, two_said) = run(sh, &once, &flip);
            assert_eq!(code, Some(0), "{sh}: 한 번짜리 죽음이 게이트가 됐다");
            assert!(two_said.contains("(exit 2)"), "{sh}: 2 를 안 냈다 — {two_said:?}");
            assert_eq!(run(sh, &once, &flip).1, String::new(), "{sh}: 같은 값이 다시 섰다");
            std::fs::write(flip.join("turned"), "").unwrap();
            let (code, hundred) = run(sh, &once, &flip);
            assert_eq!(code, Some(0), "{sh}: 값이 바뀐 자리가 게이트가 됐다");
            assert!(hundred.contains("(exit 101)"), "{sh}: 한 번짜리 2 가 뒤에 온 101 의 입을 막았다 — {hundred:?}");

            // 자리가 바뀌면 다른 세션이다 — 그쪽은 아직 못 들었으니 다시 선다.
            let two = at.join(format!("session-two-{k}"));
            std::fs::create_dir_all(&two).unwrap();
            let (code, again) = run(sh, &line, &two);
            assert_eq!((code, again), (Some(0), first.clone()), "{sh}: 새 세션이 알림을 못 받았다");

            // **표식을 못 세우는 자리에서도 게이트가 되지 않는다.** `:` 는 특수 내장이라 dash 는
            // 리다이렉션이 실패하면 그 자리에서 죽고, 그때 나가는 **2 가 곧 게이트**다 — 못 쓰는
            // 임시 디렉터리 하나로 세션의 모든 도구 호출이 막혔다. `true` 로 세우는 까닭이다.
            // 문턱은 못 서니 말은 그대로 나온다 — 알림을 잃는 것보다 낫다.
            let (code, said) = run(sh, &line, &at.join("없는-자리"));
            assert_eq!(code, Some(0), "{sh}: 표식을 못 세우는 자리가 게이트가 됐다 — {said}");
            assert_eq!(said, first, "{sh}: 표식을 못 세우는 자리에서 알림이 사라졌다");
        }
    }

    /// **표식 이름에 바이너리 철자와 이벤트가 든다**(moai-f7up). 한 세션이 저장소 둘을 오가는
    /// 것은 드물지 않은데, 이름만 같으면 첫 저장소의 표식이 둘째의 입을 막는다 — 저마다 제
    /// 바이너리가 못 돌 수 있으니 둘 다 제 알림을 내야 한다. 이벤트도 같은 까닭이다
    /// (`session-start` 의 알림은 사람에게 안 들린다, 리뷰 moai-514e.hgz).
    ///
    /// **이름을 다시 지어내지 않는다** — `tests/cli.rs` 의 `baseline` 이 적어 둔 규칙이다.
    /// 표식 이름은 `s=` 가 적은 그 값을 그대로 떼어 본다: 너비를 넓혀도 접두어를 바꿔도
    /// 이 시험이 엉뚱한 까닭으로 붉어지거나 푸르러지지 않는다.
    #[test]
    fn two_repos_do_not_share_the_notice_mark() {
        let mark = |exe: &str, event: &str| {
            // `s="…"` 가 적은 값을 통째로 뗀다([`notice_mark`]). 경로가 아니라 그 대입에서 찾는 까닭은
            // 경로에 `moai-hook-` 이 들면 `find` 가 그쪽을 먼저 집기 때문이고, 끝은 그 대입을 닫는
            // 따옴표다 — 너비도 접두어도 안 박는다.
            let name = notice_mark(&command(exe, event)).to_string();
            assert!(name.contains("${CLAUDE_CODE_SESSION_ID"), "세션이 표식에 안 든다 — {name}");
            name
        };
        let a = "/repo/a/target/release/moai";
        let b = "/repo/b/target/release/moai";
        assert_ne!(mark(a, "stop"), mark(b, "stop"), "바이너리 둘이 표식을 나눠 쓴다");
        assert_ne!(mark(a, "session-start"), mark(a, "pre-tool-use"), "이벤트 둘이 표식을 나눠 쓴다");
        // **같은 철자·같은 이벤트가 같은 표식인 것은 안 잰다** — `command` 는 인자만 보는 순수
        // 함수라 제 자신과 견주는 줄이 된다(리뷰 moai-514e.hgz). 세션 안에서 한 번만 서는지는
        // `the_notice_stands_once_a_session` 이 껍데기로 실제로 두 번 돌려 잰다.
        // 이벤트도 종료 값도 이름에 선다 — `$c` 는 껍데기가 풀 자리라 글자 그대로 남는다.
        assert!(mark(a, "stop").ends_with(".stop.$c.said"), "이벤트·종료 값이 표식에 안 선다 — {}", mark(a, "stop"));
    }

    /// 시험이 쓸 껍데기 — 있는 것만 쓴다. `sh` 는 기계마다 dash 일 수도 bash 일 수도 있어 셋 다 잰다.
    ///
    /// **`dash` 도 이름으로 부른다**(리뷰 moai-514e.hgz) — `set -e` 갈래의 중단 자리와 특수 내장의
    /// 리다이렉션 실패가 껍데기마다 다른데, `sh` 가 bash 인 기계(macOS·Fedora·RHEL)에서는 `sh`·`bash`
    /// 둘만으로 dash 를 한 번도 안 재게 된다. **목록은 여기 하나다**(리뷰 moai-45hf.nab) — 시험마다 베껴
    /// 두던 때는 한쪽에만 `dash` 가 들어 한 번 갈라졌다.
    #[cfg(unix)]
    fn shells() -> Vec<&'static str> {
        let found: Vec<&str> = ["sh", "bash", "dash"]
            .into_iter()
            .filter(|s| std::process::Command::new(s).args(["-c", "exit 0"]).output().is_ok())
            .collect();
        assert!(!found.is_empty(), "껍데기가 하나도 없다");
        found
    }

    /// 알림 표식 이름 — 줄의 `s="…"` 가 적은 값 그대로다(`${TMPDIR:-/tmp}/…` 째로). **이름을 다시 지어내지
    /// 않는다** — `tests/cli.rs` 의 `baseline` 이 적어 둔 규칙이다. **대입 `s=` 부터 찾는다** — 그 앞에 쪽지
    /// 자리 `h=` 가 같은 접두어로 선다(moai-45hf.wqg). 줄의 꼴이 바뀌면 여기 하나만 고친다.
    fn notice_mark(line: &str) -> &str {
        let at = line.find(" s=\"").expect("표식 대입이 없다") + " s=\"".len();
        let rest = &line[at..];
        &rest[..rest.find('"').expect("표식이 안 닫힌다")]
    }

    /// 모드를 세운 파일을 심는다.
    ///
    /// **돌릴 파일은 이 프로세스가 쓴 inode 를 안 쓴다** — 쓰기 fd 가 열린 동안 옆 스레드의
    /// 시험이 fork 하면 그 자식이 fd 를 물려받아, 자식이 exec 할 때까지 Linux 가 이쪽 exec 을
    /// ETXTBSY 로 거절한다. `tests/cli.rs` 의 `place_exe` 가 복사 2,400번으로 측정한 것이 그것이다
    /// (`fs::copy` 345번, `cp` 0번). 모드는 복사 뒤에 세운다 — `chmod` 는 fd 를 안 연다.
    #[cfg(unix)]
    fn planted(at: &Path, name: &str, body: &str, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let src = at.join(format!("{name}.src"));
        let p = at.join(name);
        std::fs::write(&src, body).unwrap();
        let out = std::process::Command::new("cp").arg(&src).arg(&p).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        p
    }

    /// **보드·`Stop` 의 표식은 판정이 건너간 뒤에 선다**(moai-45hf.wqg). `moai` 가 쪽지에 표식 이름을
    /// 적고, 이 줄이 `printf` 에 이긴 뒤에 그 이름으로 세운다. `printf` 가 지면 표식이 안 서서
    /// 다음 프롬프트가 보드를 다시 싣는다 — 옛 판은 `moai` 안에서 먼저 세워, 셸이 그 뒤에 죽으면
    /// 그 세션이 보드를 영영 못 받았다.
    ///
    /// **바이너리는 `moai` 가 쪽지에 하는 일을 흉내 낸다** — 환경의 [`crate::cmd::hook::HANDOFF`]
    /// 자리에 이름 한 줄을 쓴다. `moai` 쪽이 정말 그렇게 쓰는지는 `tests/cli.rs` 가 잰다.
    #[cfg(unix)]
    #[test]
    fn the_mark_stands_only_after_the_answer_crosses() {
        use std::os::unix::fs::FileTypeExt as _;
        let scratch = crate::scratch::Scratch::new("skill-handoff");
        let at = scratch.path();
        let slip = crate::cmd::hook::HANDOFF;
        let board = planted(
            at,
            "board",
            &format!("#!/bin/sh\nprintf '%s\\n' \"$TMPDIR/mark\" > \"${slip}\"\necho '{{\"ok\":true}}'\n"),
            0o755,
        );
        // 쪽지 자리에 **링크**를 둔다 — 남이 먼저 잡은 자리와 같다. 셸은 제 것이 아닌 쪽지를 안 읽는다.
        let linked = planted(
            at,
            "linked",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$TMPDIR/mark\" > \"$TMPDIR/bait\"\nln -s \"$TMPDIR/bait\" \"${slip}\"\n\
                 echo '{{\"ok\":true}}'\n"
            ),
            0o755,
        );
        // **꼴은 판정인데 `moai` 가 스스로 끝내지 못한 경우**(리뷰 moai-45hf.nab) — 쪽지를 적고 판정 꼴을 낸 뒤
        // SIGKILL 을 맞는다. 잘린 자리가 마침 `}` 뒤인 토막과 셸에게는 같은 모양이라, 흘려보내되 표식은
        // 안 세운다. 세우면 그 세션은 온전한 보드를 영영 못 받는다.
        let felled = planted(
            at,
            "felled",
            &format!("#!/bin/sh\nprintf '%s\\n' \"$TMPDIR/mark\" > \"${slip}\"\necho '{{\"ok\":true}}'\nkill -9 $$\n"),
            0o755,
        );
        // **표식이 못 서는 자리를 넘겨도 0 이다**(리뷰 moai-45hf.nab) — 그 디렉터리가 없다. 흘려보내기와
        // 표식 세우기를 묶은 자리의 `|| :` 가 맡는 몫이 이것이고, 걷으면 dash 는 `-ec` 에서 2 로 나간다.
        let astray = planted(
            at,
            "astray",
            &format!("#!/bin/sh\nprintf '%s\\n' \"$TMPDIR/nowhere/mark\" > \"${slip}\"\necho '{{\"ok\":true}}'\n"),
            0o755,
        );
        if !crate::cmd::runnable(&board) {
            return;
        }
        let nth = std::cell::Cell::new(0);
        let fresh = || {
            nth.set(nth.get() + 1);
            let tmp = at.join(format!("tmp{}", nth.get()));
            std::fs::create_dir_all(&tmp).unwrap();
            tmp
        };
        for sh in shells() {
            let tmp = fresh();
            let out = std::process::Command::new(sh)
                .args(["-c", &command(&board.display().to_string(), "user-prompt-submit")])
                .env("TMPDIR", &tmp)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{sh}: 게이트가 됐다");
            assert_eq!(String::from_utf8_lossy(&out.stdout), "{\"ok\":true}\n", "{sh}: 판정이 달라졌다");
            assert!(tmp.join("mark").exists(), "{sh}: 건너간 때에 표식이 안 섰다");

            // **받는 쪽이 못 받으면 표식이 안 선다** — `-ec` 에서도 0 이다.
            for flags in ["-c", "-ec"] {
                let tmp = fresh();
                let status = std::process::Command::new(sh)
                    .args([flags, &command(&board.display().to_string(), "user-prompt-submit")])
                    .env("TMPDIR", &tmp)
                    .stdout(std::process::Stdio::from(std::fs::File::open(&board).unwrap()))
                    .stderr(std::process::Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(status.code(), Some(0), "{sh} {flags}: 쓰기가 진 때에 값을 흘렸다");
                assert!(!tmp.join("mark").exists(), "{sh} {flags}: 못 건넌 때에 표식이 섰다 — 보드를 영영 잃는다");
            }

            let tmp = fresh();
            let out = std::process::Command::new(sh)
                .args(["-c", &command(&linked.display().to_string(), "user-prompt-submit")])
                .env("TMPDIR", &tmp)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{sh}: 링크 쪽지가 게이트가 됐다");
            assert!(!tmp.join("mark").exists(), "{sh}: 링크로 선 쪽지를 읽었다");

            let tmp = fresh();
            let out = std::process::Command::new(sh)
                .args(["-c", &command(&felled.display().to_string(), "user-prompt-submit")])
                .env("TMPDIR", &tmp)
                .stderr(std::process::Stdio::null())
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{sh}: 죽은 판정이 게이트가 됐다");
            assert_eq!(String::from_utf8_lossy(&out.stdout), "{\"ok\":true}\n", "{sh}: 판정 꼴을 안 흘려보냈다");
            assert!(!tmp.join("mark").exists(), "{sh}: 스스로 못 끝낸 판정에 표식을 세웠다 — 잘린 보드면 영영 잃는다");

            for flags in ["-c", "-ec"] {
                let tmp = fresh();
                let out = std::process::Command::new(sh)
                    .args([flags, &command(&astray.display().to_string(), "user-prompt-submit")])
                    .env("TMPDIR", &tmp)
                    .stderr(std::process::Stdio::null())
                    .output()
                    .unwrap();
                assert_eq!(out.status.code(), Some(0), "{sh} {flags}: 못 서는 표식 자리가 게이트가 됐다");
                assert_eq!(String::from_utf8_lossy(&out.stdout), "{\"ok\":true}\n", "{sh} {flags}: 판정이 달라졌다");
            }

            // **선 표식을 비우지 않는 것은 `set -C` 가 맡는다**(리뷰 moai-45hf.nab) — 빈 자리 확인을 걷은
            // 줄로 재야 `set -C` 가 빠진 것이 드러난다(알림 표식 시험의 같은 문단).
            let bare = command(&board.display().to_string(), "user-prompt-submit")
                .replace("! [ -e \"$m\" ] && ! [ -h \"$m\" ] && ", "");
            assert!(!bare.contains("! [ -e \"$m\" ]"), "빈 자리 확인의 글이 바뀌었다 — 이 시험이 걷을 글을 고친다");
            let tmp = fresh();
            std::fs::write(tmp.join("mark"), "x").unwrap();
            let out = std::process::Command::new(sh).args(["-c", &bare]).env("TMPDIR", &tmp).output().unwrap();
            assert_eq!(out.status.code(), Some(0), "{sh}: 선 표식이 게이트가 됐다");
            assert_eq!(
                std::fs::read_to_string(tmp.join("mark")).unwrap(),
                "x",
                "{sh}: 선 표식을 비웠다 — `set -C` 가 없다"
            );

            // **표식 자리에 이미 무엇이 서 있으면 열지 않는다**(리뷰 moai-45hf.nab) — 이름 있는 파이프를
            // 쓰기로 열면 읽는 쪽이 올 때까지 멈추고, 훅이면 `timeout` 에 판정째 버려진다.
            let tmp = fresh();
            if mkfifo(&tmp.join("mark")) {
                let out = bounded({
                    let mut cmd = std::process::Command::new(sh);
                    cmd.args(["-c", &command(&board.display().to_string(), "user-prompt-submit")]).env("TMPDIR", &tmp);
                    cmd
                });
                assert_eq!(out.status.code(), Some(0), "{sh}: 파이프가 선 표식 자리가 게이트가 됐다");
                assert_eq!(String::from_utf8_lossy(&out.stdout), "{\"ok\":true}\n", "{sh}: 판정이 달라졌다");
                assert!(
                    std::fs::symlink_metadata(tmp.join("mark")).is_ok_and(|m| m.file_type().is_fifo()),
                    "{sh}: 표식 자리의 파이프를 건드렸다"
                );
            }
        }
    }

    /// 이름 있는 파이프(FIFO)를 만든다. `mkfifo` 가 없는 기계면 거짓이다 — 그 경우는 파이프 갈래를 못 잰다.
    #[cfg(unix)]
    fn mkfifo(p: &Path) -> bool {
        std::process::Command::new("mkfifo").arg(p).status().is_ok_and(|s| s.success())
    }

    /// 껍데기를 돌리되 **멈추면 붉힌다**. 쓰기로 연 파이프는 읽는 쪽이 올 때까지 멈추고, 훅이면 매니페스트
    /// `timeout` 까지 서 있다가 그 출력까지 버려진다 — 시험이 그 자리에서 같이 멈추지 않게 기한을 둔다.
    #[cfg(unix)]
    fn bounded(mut cmd: std::process::Command) -> std::process::Output {
        let mut child = cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("훅 줄이 10초 넘게 멈췄다 — 이미 선 자리를 쓰기로 열었다");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        child.wait_with_output().unwrap()
    }

    /// **알림 표식은 한 번에 선다**(moai-45hf.6z2, 2026-09-29 사용자 결정). `[ -e ]` 로 보고 세우던
    /// 옛 줄은 겹쳐 도는 훅(2026-09-23 에 측정했다)이 셋 다 "없다" 를 보고 셋 다 말했다. `set -C` 의
    /// `O_EXCL` 로 세우면 하나만 이긴다.
    ///
    /// **남이 먼저 잡은 자리는 시끄러워진다, 조용해지지 않는다.** 표식 이름은 `plugin.json` 에
    /// 커밋돼 있고 `$PPID` 로 물러선 경우 나머지는 작은 정수라, 같은 기계의 남이 그 자리에 파일을
    /// 두면 그 세션의 알림이 통째로 막혔다. 이제 제 것이 아니거나(`-O`) 링크(`-h`)면 매번 말한다 —
    /// 링크가 가리키는 제 파일도 안 건드린다. 남의 소유는 root 없이 못 세우므로 링크로 잰다.
    ///
    /// **이름 있는 파이프(FIFO)로 잡힌 자리에서도 멈추지 않고 말한다**(리뷰 moai-45hf.nab). `set -C` 는 있는
    /// 보통 파일만 거절해, 파이프는 쓰기로 열어 읽는 쪽을 기다렸다 — 훅이면 도구 호출마다 `timeout` 15초에
    /// 알림도 없다. 파이프를 가리키는 링크도 같다.
    #[cfg(unix)]
    #[test]
    fn the_notice_mark_is_taken_once_and_never_squatted() {
        let scratch = crate::scratch::Scratch::new("skill-hookrace");
        let at = scratch.path();
        let dead = planted(at, "dead", "#!/bin/sh\nexit 0\n", 0o644);
        let live = planted(at, "live", "#!/bin/sh\nexit 0\n", 0o755);
        if !crate::cmd::runnable(&live) {
            return;
        }
        let line = command(&dead.display().to_string(), "pre-tool-use");
        // `s="…"` 의 값을 이 시험의 값으로 푼다 — 이름을 다시 지어내지 않는다.
        let mark_in = |dir: &Path, session: &str| {
            notice_mark(&line)
                .replace("${TMPDIR:-/tmp}", &dir.display().to_string())
                .replace("${CLAUDE_CODE_SESSION_ID:-$PPID}", session)
                .replace("$c", "126")
        };
        for (k, sh) in shells().into_iter().enumerate() {
            let race = at.join(format!("race-{k}"));
            std::fs::create_dir_all(&race).unwrap();
            // stderr 는 버린다 — 여덟이 저마다 껍데기의 `Permission denied` 를 시험 출력에 흘린다.
            let kids: Vec<_> = (0..8)
                .map(|_| {
                    std::process::Command::new(sh)
                        .args(["-c", &line])
                        .env("TMPDIR", &race)
                        .env("CLAUDE_CODE_SESSION_ID", "race")
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .unwrap()
                })
                .collect();
            let said = kids
                .into_iter()
                .map(|k| String::from_utf8_lossy(&k.wait_with_output().unwrap().stdout).to_string())
                .filter(|out| out.contains("systemMessage"))
                .count();
            assert_eq!(said, 1, "{sh}: 겹쳐 돈 훅 여덟이 알림을 {said}번 냈다");

            let squat = at.join(format!("squat-{k}"));
            std::fs::create_dir_all(&squat).unwrap();
            let mark = mark_in(&squat, "squat");
            let mine = squat.join("mine");
            std::fs::write(&mine, "keep").unwrap();
            std::os::unix::fs::symlink(&mine, &mark).unwrap();
            for _ in 0..2 {
                let out = std::process::Command::new(sh)
                    .args(["-c", &line])
                    .env("TMPDIR", &squat)
                    .env("CLAUDE_CODE_SESSION_ID", "squat")
                    .output()
                    .unwrap();
                assert_eq!(out.status.code(), Some(0), "{sh}: 잡힌 자리가 게이트가 됐다");
                let said = String::from_utf8_lossy(&out.stdout);
                assert!(said.contains("systemMessage"), "{sh}: 링크로 잡힌 자리가 알림을 막았다 — {said:?}");
            }
            assert_eq!(std::fs::read_to_string(&mine).unwrap(), "keep", "{sh}: 링크 너머의 파일을 건드렸다");

            // **하나만 이기는 것은 `[ -e ]` 가 아니라 `set -C` 가 맡는다**(리뷰 moai-45hf.nab). 빈 자리인지
            // 먼저 보는 줄에서는 위의 여덟 경합이 `set -C` 없이도 대개 푸르다 — 모두가 "없다" 를 보는 틈이
            // 좁다. 그래서 그 확인을 걷은 줄을 제 표식이 이미 선 자리에서 돌린다: `set -C` 가 서 있으면
            // 세우기가 져 조용하고 표식이 그대로고, 없으면 표식을 비우고 말한다 — 경합 없이 한 번에 가른다.
            let bare = line.replace("! [ -e \"$s\" ] && ! [ -h \"$s\" ] && ", "");
            assert_ne!(bare, line, "빈 자리 확인의 글이 바뀌었다 — 이 시험이 걷을 글을 고친다");
            let held = at.join(format!("held-{k}"));
            std::fs::create_dir_all(&held).unwrap();
            let own = mark_in(&held, "held");
            std::fs::write(&own, "x").unwrap();
            let out = std::process::Command::new(sh)
                .args(["-c", &bare])
                .env("TMPDIR", &held)
                .env("CLAUDE_CODE_SESSION_ID", "held")
                .stderr(std::process::Stdio::null())
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{sh}: 선 표식이 게이트가 됐다");
            assert!(out.stdout.is_empty(), "{sh}: 제 표식이 선 자리에서 다시 말했다 — `set -C` 가 없다");
            assert_eq!(std::fs::read_to_string(&own).unwrap(), "x", "{sh}: 선 표식을 비웠다 — `set -C` 가 없다");

            // 파이프로 잡힌 자리 — 그 자리에 선 파이프와, 파이프를 가리키는 링크.
            let piped = at.join(format!("fifo-{k}"));
            std::fs::create_dir_all(&piped).unwrap();
            let fifo = piped.join("fifo");
            if mkfifo(&fifo) && mkfifo(Path::new(&mark_in(&piped, "fifo"))) {
                std::os::unix::fs::symlink(&fifo, mark_in(&piped, "linked")).unwrap();
                for session in ["fifo", "linked"] {
                    let out = bounded({
                        let mut cmd = std::process::Command::new(sh);
                        cmd.args(["-c", &line]).env("TMPDIR", &piped).env("CLAUDE_CODE_SESSION_ID", session);
                        cmd
                    });
                    assert_eq!(out.status.code(), Some(0), "{sh} {session}: 파이프로 잡힌 자리가 게이트가 됐다");
                    let said = String::from_utf8_lossy(&out.stdout);
                    assert!(
                        said.contains("systemMessage"),
                        "{sh} {session}: 파이프로 잡힌 자리가 알림을 막았다 — {said:?}"
                    );
                }
            }
        }
    }

    /// **커밋된 플러그인 트리가 지금의 글과 같다.**
    ///
    /// `.claude/moai-plugin/` 은 `skill install` 이 낸 것을 커밋해 둔 복사라,
    /// `guide.rs` 의 글만 고치면 조용히 낡는다 — 탐색기가 idea 를 담게 된 뒤에도
    /// SKILL.md 가 "읽기 전용" 이라 가르쳤다(`moai-ka9p`). 그 복사를 읽은 세션은
    /// 바이너리가 아니라 옛 글을 배운다.
    ///
    /// **훅의 실행 파일과 마켓플레이스 이름은 커밋된 파일에서 읽는다.** 둘은
    /// 심은 체크아웃의 자리에서 나오는 값이라, 이 시험을 부른 자리(워크트리)로
    /// 다시 셈하면 글이 같아도 늘 어긋난다. 여기서 보는 것은 글과 판뿐이다.
    ///
    /// 다시 쓰는 길: `MOAI_BLESS=1 cargo test checked_in` — 같은
    /// `tree_named` 로 트리 전부를 적힌 자리 그대로 다시 쓴다. `skill install` 은
    /// `claude` 등록까지 건드리고 부른 자리의 경로를 적어, 워크트리에서는 못 쓴다.
    ///
    /// **세 에이전트의 트리를 다 본다**(moai-xs2h.zom) — 이 저장소는 Claude·Codex·Antigravity 로 자기 자신을
    /// 관리한다. Codex 와 Antigravity 는 한 자리([`AGENTS_DIR`])를 함께 읽어 커밋된 트리는 둘이다. 그 자리는
    /// 매니페스트가 없어 읽어 올 값이 없고 글만 견준다. 한쪽만 다시 쓰면 그 에이전트의 세션이 옛 글을 배운다.
    #[test]
    fn the_checked_in_plugin_matches_the_guide() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join(DIR);
        let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
        let bless = blessing(std::env::var("MOAI_BLESS").ok().as_deref());
        // **깨진 파일도 bless 로 되살린다.** 두 값은 JSON 을 읽어야 나오는데, 병합
        // 충돌 표시 한 줄로 그 읽기가 실패하면 다시 쓰기에 닿기 전에 멈춰 도구
        // 밖(`git checkout`)만 남는다. 그래서 bless 일 때만 글자로 찾는다. **아래
        // 비교는 틀린 값을 못 잡는다** — bless 는 찾은 값으로 먼저 쓰고 견주므로
        // 제 자신과 같다. 그래서 이름은 맨 윗단 들여쓰기(두 칸)에 매어 찾는다 —
        // 안 매면 맨 윗단 `name` 이 빠진 글에서 `owner.name` 의 `moai` 를 집는다.
        let manifest = read(".claude-plugin/plugin.json");
        let exe = hook_exe(&manifest)
            .or_else(|| loose(&manifest, "command -v -- \\\"").filter(|_| bless))
            .expect("커밋된 plugin.json 에서 훅의 실행 파일을 못 읽는다 — 깨졌으면 MOAI_BLESS=1 로 다시 쓴다");
        let market = read(".claude-plugin/marketplace.json");
        let name = serde_json::from_str::<serde_json::Value>(&market)
            .ok()
            .and_then(|v| Some(v.get("name")?.as_str()?.to_string()))
            .or_else(|| loose(&market, TOP_NAME).filter(|_| bless))
            .expect("marketplace.json 에서 name 을 못 읽는다 — 깨졌으면 MOAI_BLESS=1 로 다시 쓴다");

        let all = skills();
        for (at, want) in [(DIR, tree_named(&name, &exe, &all)), (AGENTS_DIR, agents_tree(&all))] {
            let stale = checked_in(&root.join(at), &want, bless);
            assert!(
                stale.is_empty(),
                "{at} 이 guide.rs 의 글에서 낡았다: {stale:?}\n  \
                 MOAI_BLESS=1 cargo test checked_in 으로 다시 쓴다"
            );
        }
    }

    /// 커밋된 트리 하나를 `want` 와 견주어 다른 파일을 낸다 — `bless` 면 먼저 그대로 다시 쓴다. 없는 파일도 다른 것이다.
    fn checked_in(dir: &Path, want: &[(PathBuf, String)], bless: bool) -> Vec<String> {
        if bless {
            for (path, body) in want {
                // 새로 느는 파일은 제 디렉터리가 아직 없다 (감독 스킬이 처음 그랬다).
                if let Some(parent) = dir.join(path).parent() {
                    std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
                }
                std::fs::write(dir.join(path), body).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
        want.iter()
            .filter(|(path, body)| std::fs::read_to_string(dir.join(path)).ok().as_deref() != Some(body.as_str()))
            .map(|(path, _)| path.display().to_string())
            .collect()
    }

    /// `MOAI_BLESS` 를 켰다고 읽는가. 있기만 하면 참으로 읽으면 끄려고 적은
    /// `MOAI_BLESS=0` 이 커밋된 트리를 조용히 다시 쓴다.
    fn blessing(value: Option<&str>) -> bool {
        matches!(value.map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("1" | "true" | "yes"))
    }

    /// `marketplace.json` 맨 윗단의 `name`. `pretty` 가 두 칸으로 들여 쓰고
    /// `owner`·`plugins` 의 `name` 은 더 깊다.
    const TOP_NAME: &str = "\n  \"name\": \"";

    /// JSON 으로 못 읽는 글에서 `before` 바로 뒤의 값을 찾는다. 값은 따옴표나
    /// 역슬래시에서 끝난다 — 훅의 실행 파일은 `quotable` 이라 둘 다 못 품는다.
    fn loose(text: &str, before: &str) -> Option<String> {
        let rest = &text[text.find(before)? + before.len()..];
        Some(rest[..rest.find(['"', '\\'])?].to_string()).filter(|v| !v.is_empty())
    }

    #[test]
    fn bless_reads_the_value_not_the_presence() {
        for on in ["1", "true", "YES", " 1 "] {
            assert!(blessing(Some(on)), "{on:?} 를 켜짐으로 못 읽는다");
        }
        for off in ["0", "", "false", "no"] {
            assert!(!blessing(Some(off)), "{off:?} 를 켜짐으로 읽는다");
        }
        assert!(!blessing(None));
    }

    /// 병합 충돌 표시가 끼어 JSON 이 깨져도 bless 가 쓸 두 값은 글자로 나온다.
    #[test]
    fn a_conflicted_tree_still_yields_its_exe_and_name() {
        let exe = "/repo/target/release/moai";
        let files = tree("t", Path::new("/repo"), exe, &fake("# 스킬", "감독", "위키"));
        let body = |end: &str| {
            let (_, b) = files.iter().find(|(p, _)| p.ends_with(end)).unwrap();
            format!("<<<<<<< HEAD\n{b}=======\n")
        };
        let manifest = body("plugin.json");
        assert_eq!(hook_exe(&manifest), None, "시험이 깨진 매니페스트를 만들지 못했다");
        assert_eq!(loose(&manifest, "command -v -- \\\"").as_deref(), Some(exe));
        let name = market("t", Path::new("/repo"));
        assert_eq!(loose(&body("marketplace.json"), TOP_NAME).as_deref(), Some(name.as_str()));
        assert_eq!(loose("{}", TOP_NAME), None);
        // 맨 윗단 name 이 빠지면 owner·plugins 의 `moai` 를 집지 않는다.
        let headless = body("marketplace.json").replace(&format!("\n  \"name\": \"{name}\","), "");
        assert_ne!(headless, body("marketplace.json"), "시험이 name 을 못 뺐다");
        assert_eq!(loose(&headless, TOP_NAME), None);
    }
}
