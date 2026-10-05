//! 훅의 입출력. **판단은 여기 없다** — `hook` 모듈이 정하고 여기는 stdin 을
//! 풀고 저장소를 읽고 답을 계약 JSON 으로 옮긴다.
//!
//! ## 이 명령은 절대 실패하지 않는다
//!
//! 무엇이 어긋나도 빈 출력과 종료 코드 0 이다. 훅이 에러를 뱉으면 매 세션
//! 시작이 시끄럽고, 그러면 사람이 훅을 꺼 버린다 — 꺼진 규칙은 없는 규칙이다.
//! 저장소가 아니어도, stdin 이 JSON 이 아니어도, 줄이 깨져 있어도 조용히
//! 지나간다. `moai status` 가 아무것도 막지 않는 것과 같은 이유다.
//!
//! ## 세 에이전트의 말씨
//!
//! **판정은 하나고 말씨만 셋이다**(moai-u5wr). 들어온 것은 [`arrived`] 가 Claude 의 꼴([`Input`])과 이벤트로 옮기고,
//! 나가는 것은 [`answer`] 가 그 에이전트의 꼴로 옮긴다 — 그 사이는 `--dialect` 를 모른다(출석만 안다,
//! [`attendee`]·[`rest`]). 꼴은 2026-10-04 에 사람이 띄운 대화형 codex 0.160·agy 1.2.16 에서 기록했다(moai-u5wr.amg
//! 노트, `tests/hooks/`).
//!
//! - **Codex** 는 들고 나는 꼴이 Claude 와 같다. 다른 것은 `apply_patch` 하나다 — 패치가 고치는 파일마다
//!   `Edit` 한 번으로 접는다(셸로 친 `apply_patch <<'EOF'` 도). Codex 는 셸·`apply_patch`·MCP 부름에만 훅을
//!   낸다(openai/codex#20204)
//! - **Antigravity** 는 camelCase 로 들어오고(`conversationId`·`workspacePaths`·`toolCall{name,args}`) 맨 윗단
//!   `decision` 으로 나간다. `UserPromptSubmit` 이 없어 턴의 첫 모델 부름(`PreInvocation`, `invocationNum: 0`)이
//!   그 자리에 서고, 싣는 글은 `injectSteps` 의 `ephemeralMessage` 다. `SessionStart` 도 없어 기준선은 턴 머리에서
//!   없을 때만 적는다

use super::{Ctx, R};
use crate::cli::Dialect;
use crate::hook::{Decision, Event, Input};
use crate::report;
use crate::store::Repo;
use crate::{mail, model, view};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 계약이 받는 모양. 이벤트 이름이 안에 한 번 더 들어간다.
#[derive(Serialize)]
struct Out<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: Specific<'a>,
}

#[derive(Serialize)]
struct Specific<'a> {
    #[serde(rename = "hookEventName")]
    event: &'a str,
    #[serde(rename = "additionalContext")]
    context: String,
}

/// 도구 호출을 막을 때의 모양. **까닭이 곧 화면에 뜨는 글이다** — 막힌 쪽이
/// 읽고 그대로 고칠 수 있어야 한다.
#[derive(Serialize)]
struct Refusal<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: RefusalBody<'a>,
}

#[derive(Serialize)]
struct RefusalBody<'a> {
    #[serde(rename = "hookEventName")]
    event: &'a str,
    #[serde(rename = "permissionDecision")]
    decision: &'a str,
    #[serde(rename = "permissionDecisionReason")]
    reason: String,
}

/// 턴을 끝내지 않게 붙드는 모양 — Antigravity 는 도구 부름을 막는 답도 이 꼴이다([`antigravity_answer`]).
#[derive(Serialize)]
struct Hold {
    decision: Verdict,
    reason: String,
}

/// 맨 윗단 `decision` 의 낱말. **`allow` 가 없다** — Antigravity 에 `allow` 를 내면 사람이 물어야 할 도구 부름까지
/// 허락한다([`antigravity_answer`]). 낱말을 글로 두던 판은 그 한 줄을 막을 것이 없었다.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Verdict {
    /// Claude·Codex 의 `Stop` 이 턴을 붙든다.
    Block,
    /// Antigravity 가 도구 부름을 막는다.
    Deny,
    /// Antigravity 의 `Stop` 이 턴을 붙든다.
    Continue,
}

pub fn run(ctx: &Ctx, event: Event, dialect: Dialect) -> R<Vec<String>> {
    // 색은 언제나 끈다. 훅의 stdout 은 사람이 아니라 파서가 읽는다 —
    // 이스케이프가 한 바이트라도 섞이면 계약 JSON 이 통째로 버려진다.
    anstream::ColorChoice::Never.write_global();

    // **넘길 자리의 옛 쪽지는 맨 먼저 걷는다**(moai-45hf.wqg). 그 이름은 셸의 pid 로 지어져, pid 가
    // 돌아오면 먼저 죽은 셸이 못 올린 쪽지가 이번 호출의 것처럼 읽힌다 — 보드를 안 지은 호출이 남의
    // 표식을 세운다. 이번 호출이 쓸 것은 [`once_per_session`] 이 새로 쓴다. **어느 이벤트든 걷는다** —
    // 셸은 `moai` 가 무엇이든 내면 쪽지를 읽으니(`PreToolUse` 의 거절도), 보드를 안 짓는 이벤트가 곧
    // 그 호출이다.
    if let Some(slip) = handoff() {
        let _ = std::fs::remove_file(slip);
    }

    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return Ok(Vec::new());
    }
    // **부름 하나가 판정 여럿일 수 있다** — Codex 의 패치 하나가 파일 여럿을 고친다([`arrived`]). 차례는
    // `Decision::then` 이 정한다: 한 파일이라도 막으면 그 패치를 막고, 뒤의 파일은 묻지 않는다. 이벤트도 말씨가 옮긴
    // 것을 쓴다 — 오류로 끝난 Antigravity 의 `Stop` 은 `StopFailure` 다([`from_antigravity`]).
    let (event, inputs) = arrived(dialect, event, &raw);
    let decision = inputs.iter().fold(Decision::Pass, |done, input| done.then(|| judge(ctx, event, dialect, input)));
    Ok(answer(event, decision, dialect).into_iter().collect())
}

/// 들어온 것 하나를 판정한다 — Claude 의 꼴로 옮긴 뒤다([`arrived`]).
fn judge(ctx: &Ctx, event: Event, dialect: Dialect, input: &Input) -> Decision {
    // **명령줄은 이 세션에서 한 번만 읽는다**(moai-uc5v). 규칙마다 제 [`crate::hook::Line`] 을
    // 세우던 판은 Bash 한 번에 같은 글을 여덟 번 읽었고, 겹친 치환은 그 한 번을 이미 비싸게
    // 만든다. `Line` 은 게으르니 명령줄이 없는 호출(`Edit`·`Skill`)은 여기서 아무것도 안 읽는다.
    //
    // **접은 것과 그 명령줄은 함께 내려간다**(리뷰 moai-uc5v.ssb). `decide` 가 제 손으로 다시 접던
    // 판은 규칙이 판정하는 `Call` 과 규칙이 읽는 `Line` 이 따로 서서, 한쪽만 고치는 날 `Call::Shell`
    // 갈래가 빈 명령줄을 받는다 — 토막이 0개라 규칙 1·2·3 이 모두 지나가고, 훅은 아무 말도 안 해
    // "명령이 멀쩡했다" 와 구별되지 않는다.
    let call = crate::hook::Call::read(input.tool_name.as_deref(), &input.tool_input);
    let line = crate::hook::Line::new(match call {
        crate::hook::Call::Shell(cmd) => cmd,
        _ => "",
    });

    // **규칙 4 는 자리도 트래커도 묻기 전에 본다**(moai-zis7) — 사람의 tmux 서버는 트래커와 무관하다.
    // 트래커를 찾은 뒤에만 보던 판은 스크래치패드로 `cd` 해 둔 세션(그 자리가 이미 지워졌어도)의
    // `tmux kill-server` 를 그대로 보냈고, 막을 때마다 옆 워크트리를 겹쳐 다시 재는 값(`settle`)을
    // 치렀다 — 스냅샷이 바뀌어도 답이 같은 판정이다(리뷰 moai-ju21.70g).
    if event == Event::PreToolUse
        && matches!(call, crate::hook::Call::Shell(_))
        && let refusal @ Decision::Deny(_) = crate::hook::guard_tmux(&line)
    {
        return refusal;
    }
    // **서브에이전트는 부모의 이름으로 우편함을 안 만진다**(moai-ew4o.4fv) — 이것도 자리·트래커와 무관하다. 본 세션의 부름은
    // `agent_id` 가 없어 명령줄을 다시 훑지 않는다.
    if event == Event::PreToolUse
        && input.agent_id.is_some()
        && matches!(call, crate::hook::Call::Shell(_))
        && let refusal @ Decision::Deny(_) = crate::hook::guard_subagent_mail(&line)
    {
        return refusal;
    }

    // **자리는 stdin 이 정한다.** 훅 프로세스가 어디서 도는지는 아무도
    // 약속하지 않았다 — 시험판이 제 cwd 로 상대 경로를 풀다가 저장소 안의
    // 파일을 저장소 밖으로 보아 규칙이 통째로 샜다. Antigravity 는 훅을 hooks.json 이 놓인 디렉터리에서
    // 띄운다(2026-10-04 실측) — 작업 자리가 아니다.
    if let Some(cwd) = input.cwd.as_deref()
        && std::env::set_current_dir(cwd).is_err()
    {
        return Decision::Pass;
    }

    decide(ctx, event, dialect, input, call, &line).unwrap_or(Decision::Pass)
}

/// 답을 내되, 못 내면 아무 말도 하지 않는다(`None`).
///
/// **`Ctx` 를 통째로 받는다** — 화면 언어([`Ctx::lang`])가 드는 것은 **글을 싣는 갈래**뿐이다
/// (`PreCompact` 의 `carried`, `SessionStart` 의 보드, `Stop` 의 `closing`). 여기서 미리 풀면
/// 툴 부름마다 도는 `PreToolUse` 가 사람의 설정 파일을 매번 읽는데, 그 갈래는 지금 `ctx.lang()`
/// 을 한 번도 안 부른다 — 규칙의 거절문을 말묶음으로 옮기는 날 그 갈래에 `say(ctx.lang(), …)`
/// 를 놓으면 이 값이 도로 돌아온다. 그때는 말을 거절하는 가지 안에서 푼다.
fn decide(
    ctx: &Ctx,
    event: Event,
    dialect: Dialect,
    input: &Input,
    call: crate::hook::Call<'_>,
    line: &crate::hook::Line<'_>,
) -> Option<Decision> {
    let cwd = std::env::current_dir().ok()?;
    // **`Stop` 없이 끝난 턴은 출석만 적는다**(moai-u5wr.f29) — 트래커를 안 읽는다. `SessionEnd` 는 Claude 가 1.5초
    // 안에 끝내라고 하고, 실을 글도 없다(셋 다 출력을 안 읽는다). **이벤트를 다 적어 가른다** — 새 이벤트를 더하면
    // 컴파일러가 여기서 어느 쪽인지 묻는다. `matches!` 로 가르던 판은 아래 판정의 빈 갈래만 채우면 컴파일이 되어, 새 끝
    // 이벤트가 트래커를 통째로 읽고 출석도 안 돌렸다.
    //
    // **설정도 안 읽는다 — 출석부가 선 뿌리만 찾는다**(moai-jzym.uxa, [`crate::store::tracker_in_use`]). [`Repo`] 를
    // 세우던 판은 루트의 `config.toml` 을 파싱했고, 거기 충돌 표시가 끼면 아래의 물러서는 길이 워크트리의 `.moai` 를 열어
    // 루트의 장을 놓쳤다 — Codex 의 `SessionEnd` 가 장을 안 걷고 `Interrupt` 가 `idle` 로 안 돌렸다. 그 길은 규칙을 위한
    // 것이지 출석을 위한 것이 아니다.
    match event {
        Event::StopFailure | Event::Interrupt | Event::SessionEnd => {
            rest(input, &crate::store::tracker_in_use(&cwd)?, dialect, event);
            return Some(Decision::Pass);
        }
        Event::SessionStart | Event::UserPromptSubmit | Event::PreToolUse | Event::Stop => {}
    }
    // **옮겨 갈 루트를 못 읽어도 규칙은 선다**(리뷰 moai-71ht.i1u). 트래커가 루트로 옮겨 가면서
    // (moai-y7go) 루트의 깨진 `config.toml` 하나가 저장소의 **모든** 워크트리에서 훅을 조용히
    // 껐다 — 고장의 크기가 규칙의 크기가 되면 안 된다. `moai` 자신은 그 자리에서 크게 실패하고
    // (사람이 그것을 본다), 훅은 이 자리의 트래커로 선다. 읽는 것은 갈라진 스냅샷이지만 아무
    // 말도 안 하는 것보다 낫다.
    //
    // **여기서는 말을 안 짓는다** — 못 찾은 것을 값으로만 가른다(moai-5j49). 훅은 화면이 아니라
    // 보드 한 덩이를 얹는 자리라, 찾기가 진 까닭을 사람에게 낼 일이 없다.
    //
    // **출석과 우편함은 물러서지 않는다**(리뷰 moai-jzym.a9k) — 끝 이벤트와 같은 자리, 트래커의 뿌리(`post`)다. 물러선
    // 트래커에 적던 판은 루트의 설정이 깨진 채 연 세션의 장을 워크트리의 `.moai` 에 세웠고(끝 이벤트는 루트를 보니 그 장을
    // 영영 안 돌리고 안 걷었다), 깨지기 전에 연 세션에는 같은 이름의 장을 하나 더 세워 루트의 장이 낡은 상태로 남았다.
    // 루트의 우편함에 와 있던 편지도 그동안 안 실렸다. 그 출석부는 아무도 안 읽는다 — `moai agents`·`send` 는 루트를
    // 읽거나 크게 실패한다. 찾기가 이긴 판은 `repo.root` 가 곧 그 뿌리라 다시 안 찾는다(도구 부름마다 지나는 길이다).
    let (repo, post) = match Repo::find_from(&cwd, crate::i18n::Lang::default) {
        Ok(Some(repo)) => {
            let post = repo.root.clone();
            (repo, post)
        }
        Ok(None) => return None,
        Err(_) => {
            let here = Repo::find_here(&cwd, crate::i18n::Lang::default).ok()??;
            let post = crate::store::tracker_in_use(&cwd).unwrap_or_else(|| here.root.clone());
            (here, post)
        }
    };
    let agents = crate::store::agents_at(&post);
    let load = repo.read().ok()?;
    // **못 읽은 줄을 그대로 넘긴다.** 빈 슬라이스를 넘기면 보드에서
    // `unreadable_line` 경고만 조용히 빠지는데, 그것은 실린 보드 말고는
    // 에이전트가 알아낼 길이 없는 유일한 경고다 — 기준선도 같은 만큼
    // 낮게 잡혀 `Stop` 이 "늘었다" 를 영영 못 본다.
    let unreadable = load.unreadable();

    let decision = match event {
        // **접힌 뒤는 같은 세션이다.** 기준선을 다시 적으면 접기 전에 늘린
        // 경고가 물려받은 빚에 묻혀 `Stop` 이 못 본다. 대신 보드의 표를
        // 지워 다음 프롬프트가 보드를 다시 싣게 한다 — 접힐 때 같이 떨어졌다.
        Event::SessionStart if input.source.as_deref() == Some("compact") => {
            // **없을 때만 적는다.** 여는 훅이 안 돌았던 세션(도중에 심었거나 임시
            // 디렉터리가 비워졌다)은 여기서 적지 않으면 `Stop` 이 끝까지 견줄 것이 없다.
            if baseline(input, &repo).is_none() {
                write_baseline(input, &repo, &load.issues, &unreadable, ctx.zone());
            }
            if let Some(path) = session_file(input, &repo, "board") {
                let _ = std::fs::remove_file(path);
            }
            // 누구의 것인지 모르는 줄은 싣지 않는다 — 남의 일을 "압축 전부터 집고 있다" 로 떠안긴다(moai-4jsy).
            // `Stop` 이 붙드는 것과 같은 자로 잰다([`releasing`]).
            let (away, latest) = releasing(input, &repo, &load.issues, &person_at(&repo, ctx));
            let carried = crate::hook::carried(
                &load.issues,
                latest.as_deref().unwrap_or(&load.issues),
                &repo.config,
                &away,
                ctx.lang(),
            );
            // **접힌 뒤는 편지도 싣는다** — 붙는 것을 잰 자리가 여기다(2026-10-04 사용자 결정). 편지는 같은 칸에 먼저 선
            // 줄 다음 자리에 든다([`crate::hook::letters_room`]).
            //
            // **상태는 안 바꾼다**(리뷰 moai-h8tn.x4l) — 저절로 접히는 것은 턴 한가운데라 이미 `busy` 고, 사람이 친
            // `/compact` 뒤에는 프롬프트도 `Stop` 도 안 와 `busy` 로 적으면 노는 세션이 내내 일하는 것으로 남는다.
            // 처음 서는 장만 `busy` 로 적는다. 출석은 편지를 옮기기 전에 적는다 — 옮긴 뒤의 쓰기가 늦어 훅이 시간을
            // 넘기면 그 편지는 아무에게도 안 실린다.
            let me = attendee(input, &post, dialect);
            let status = me.as_ref().map(|p| p.status.clone()).filter(|s| !s.is_empty());
            attend(&agents, me.clone(), status.as_deref().unwrap_or(mail::BUSY));
            let letters = me.as_ref().and_then(|p| {
                deliver(&post, p, ctx, crate::hook::letters_room(&carried), waiting(&post, p, Mine::All))
            });
            carried.then(|| letters.map_or(Decision::Pass, Decision::Context))
        }
        // 기준선만 적고 아무것도 싣지 않는다. 까닭은 `hook::Event` 에 있다. **출석은 적는다**(moai-h8tn) — 편지는
        // 안 싣는다: 여기 출력이 대화에 붙는다고 잰 것은 접힌 뒤뿐이라, 실으면 읽음으로 옮긴 편지가 아무에게도 안
        // 실릴 수 있다. 첫 `UserPromptSubmit`·`Stop` 이 싣는다(2026-10-04 사용자 결정).
        Event::SessionStart => {
            write_baseline(input, &repo, &load.issues, &unreadable, ctx.zone());
            attend(&agents, attendee(input, &post, dialect), mail::IDLE);
            Decision::Pass
        }
        Event::UserPromptSubmit => {
            // **턴 머리에서도 기준선이 없으면 적는다** — 접힌 뒤와 같은 자다. 안 적으면 그 세션의 `Stop` 이 끝까지 견줄
            // 것이 없다. Antigravity 는 `SessionStart` 가 아예 없고, Claude·Codex 도 그 훅이 안 돈 세션이 있다 — 첫 프롬프트
            // 뒤에 `/hooks` 에서 믿어 준 Codex, 도중에 심었거나 임시 디렉터리가 비워진 세션(리뷰 moai-u5wr.e74). 말씨로
            // 가르던 판은 그 Codex 세션이 경고를 아무리 늘려도 `Stop` 이 한 번도 안 붙들었다.
            if baseline(input, &repo).is_none() {
                write_baseline(input, &repo, &load.issues, &unreadable, ctx.zone());
            }
            let board = once_per_session(input, &repo, "board", || {
                let now = model::now();
                let mut st = report::status(&load.issues, &unreadable, &repo.config, &now, ctx.zone());
                // `moai status` 와 **같은 자**로 싣는다([`crate::cmd::status::install_notices`]) — 낡은
                // AGENTS.md 를 모르고 시작하는 것이 바로 이 보드를 받는 새 세션이다. 셋을 여기서 따로
                // 적던 때는 한쪽에 알림을 더하면 다른 쪽이 조용했다(moai-6k1r). 세션의 셸 자리는 stdin 의
                // `cwd` 라 이미 여기로 옮겨 왔으므로 `chdir` 은 `false` 다 (`-C` 가 아니다).
                st.notices.extend(crate::cmd::status::install_notices(&repo, false));
                // 보드가 **정말 읽은 파일**을 댄다(`cmd::status::source_of` 와 같은 자) — 워크트리
                // 세션의 보드는 루트의 트래커에서 온다(moai-y7go).
                let source = crate::cmd::status::source_of(&repo);
                // 겹쳐 보지 않는다 — 훅의 보드는 제 저장소의 줄만 싣는다. 그래서 출처가 없는
                // 화면이고(`view::Screen::new`), 빈 `Origin` 을 지어 빌려 줄 일이 없다.
                let lines = view::status(
                    &st,
                    &load.issues,
                    &repo.config,
                    &now,
                    &source,
                    0,
                    view::Screen::new(ctx.lang()).at(ctx.clock()),
                );
                crate::hook::board(&lines, ctx.lang())
            });
            // 사람이 물었으니 일하는 중이다. 편지는 **매 프롬프트** 싣는다 — 보드처럼 한 번이 아니다(moai-h8tn). 보드와
            // 한 칸이라 그 다음 자리에 든다([`crate::hook::letters_room`]). 출석은 편지를 옮기기 전에 적는다(위와 같은 까닭).
            //
            // **프롬프트는 새 턴이다 — 이미 일하는 중인 장이어도 `since` 를 새로 댄다**(리뷰 moai-u5wr.e74). Claude 와
            // Antigravity 는 사람이 Esc 로 끊은 턴에 훅을 하나도 안 내 장이 `busy` 로 남는데, 상태가 같다고 그때를 두던 판은
            // `send --wake` 가 몇 시간 전에 끊긴 턴의 시각을 "그때부터 일하는 중" 으로 댔다 — 그 값을 댄 까닭과 거꾸로다.
            let me = attendee(input, &post, dialect).map(|p| mail::Presence { since: String::new(), ..p });
            attend(&agents, me.clone(), mail::BUSY);
            // **Codex 세션에는 제 장의 이름을 댄다**(moai-u5wr.7xr) — 그 셸은 세션 모두가 함께 쓰는 데몬 밑에서 돌아 `moai` 가
            // 조상으로 이 세션을 못 찾는다. 보드와 함께 세션에 한 번 싣고, `hello`·`inbox`·`send` 는 그 이름을 `--as` 로 받는다.
            let board = match (&me, dialect) {
                (Some(p), Dialect::Codex) if board != Decision::Pass => board.then(|| {
                    Decision::Context(crate::i18n::fill(
                        crate::i18n::say(ctx.lang(), "hook.you_are"),
                        &[("name", &p.name)],
                    ))
                }),
                _ => board,
            };
            let letters = me
                .as_ref()
                .and_then(|p| deliver(&post, p, ctx, crate::hook::letters_room(&board), waiting(&post, p, Mine::All)));
            board.then(|| letters.map_or(Decision::Pass, Decision::Context))
        }
        Event::PreToolUse => {
            use crate::hook::Call;
            // **장은 도구 부름에도 닻을 적는다**(moai-j3n5) — 프로세스로 못 재는 장은 오래 안 적히면 떠난 것으로 읽힌다.
            // 프롬프트도 턴 끝도 없이 도구만 부르는 긴 턴이 그 사이에 떠난 것으로 읽히지 않게, 때가 되었을 때만 다시 적는다
            // ([`mail::keep_alive`]). **Codex 만이 아니다**(moai-dhxm) — 프로세스를 아는 장도 같은 저장소를 쓰는 다른
            // 기계(컨테이너)에서는 닻으로만 잰다. 일꾼의 턴은 몇 시간을 가니, 여기서 안 적으면 그 기계의 감독에게 일하는
            // 일꾼이 20분 만에 떠난 것으로 보인다.
            if let Some(session) = input.session_id.as_deref().filter(|s| !s.trim().is_empty()) {
                mail::keep_alive(&agents, |p| p.session.as_deref() == Some(session));
            }
            let cwd = cwd.clone();
            // **토막이 가리킨 자리는 한 번만 푼다**(moai-47zz) — [`route`] 와 아래 `stands` 가 저마다
            // 부르던 판은 같은 명령줄에 같은 답을 두 번 물었다. 한 자리에서 풀어 나눠 쓴다.
            let dirs = match call {
                Call::Shell(_) => crate::hook::aimed(line, &cwd),
                _ => Vec::new(),
            };
            let (routes, there, aims) = match call {
                Call::Shell(_) => route(&repo, line, &dirs),
                _ => (Vec::new(), Vec::new(), Vec::new()),
            };
            let mine = |k: usize| !matches!(routes.get(k), Some(r) if *r != Route::Here);
            // **갈라 놓은 제 트래커를 가리킨 토막의 집기도 이 세션의 것이다**(moai-acf7 의 뒷짝).
            // 규칙 1·3 은 그 트래커가 보지만([`Route::There`]), 규칙 2 의 집기 셈까지 그 자리에서
            // 꺼지면 규약이 시키는 `moai -C <루트> mv <id> in_progress && sed -i …` 가 집고도 막힌다 —
            // 방금 집은 그 id 를 집으라고 내밀면서. 가르는 자는 `Repo::seen_from` 이 적어 둔 것
            // 하나다: 그 트래커를 **이 체크아웃의 눈으로** 본다면 그 집기는 여기 것이다. 등록한 옆
            // 프로젝트와 `.moai` 가 여럿인 모노레포의 옆 칸은 제 자리를 그대로 들어 여기 안 든다
            // (moai-23ky 가 그은 트래커 경계다).
            let ours = |k: usize| match routes.get(k) {
                Some(Route::There(n)) => there.get(*n).is_some_and(|r| same(r.here(), repo.here())),
                r => !matches!(r, Some(Route::Nowhere)),
            };
            let segs = crate::hook::Segs { judges: &mine, picks: &ours };
            // **이 줄을 훑는 것도 한 번이다**(moai-44wr) — 규칙 2 와 아래 기록이 같은 걸음을 따로
            // 걷던 판은 토막마다 그 값을 두 번 치렀고, 막는 판에서는 `settle` 이 판정을 다시 부를
            // 때마다 또 한 번 걸었다. 설정이 함께 드는 까닭은 [`crate::hook::Scan`] 에 있다.
            let scan = crate::hook::Scan::new(line, &repo.config);
            // **내미는 줄은 그 토막이 겨눈 트래커를 댄다**(moai-v9sa, 사용자 결정) — 사람이 친 `-C` 의
            // 글자가 아니라 [`route`] 가 푼 자리다. `Repo::find_from` 이 딸린 워크트리를 루트로 옮기니
            // (moai-y7go) 거절문이 워크트리의 스냅샷을 겨누는 길이 닫히고, `moai -C .`·`cd src && moai -C ..`
            // 처럼 어디서 쳤느냐에 따라 달라지는 상대 경로도 풀려 나온다.
            let toward = |k: usize| {
                aims.get(k).and_then(Option::as_ref).map(|(at, tracker)| crate::hook::Aimed { at, tracker: *tracker })
            };
            // **사람은 묻는 자리에서 푼다**(moai-0zjo 리뷰, [`crate::hook::Person`]) — 여기서는 푸는 길만 건넨다.
            let me = person_at(&repo, ctx);
            let decision = settle(input, &repo, &load.issues, away_of(&repo, &load.issues, &me), &|issues, away| {
                match call {
                    // 규칙의 차례는 `guard_shell_in` 이 정한다. 여기는 껍데기의 자리와 제 토막만 준다.
                    // **세는 자리는 세션이 선 체크아웃이다**(moai-y7go) — 트래커는 루트로 옮겨 가지만
                    // (`Repo::find_from`) 고치는 파일은 이 워크트리의 것이다. `repo.root` 로 세던 판은
                    // 워크트리의 파일이 죄다 루트의 `.claude/worktrees/…` 밑으로 보여 규칙 2 가 통째로 꺼졌다.
                    Call::Shell(_) => {
                        crate::hook::guard_shell_in(issues, away, repo.here(), &cwd, &scan, &segs, &toward)
                    }
                    Call::Edits(path) => crate::hook::guard_edit(issues, &repo.config, away, repo.here(), path),
                    Call::Review => crate::hook::guard_review(issues, &repo.config, away),
                    Call::Other => Decision::Pass,
                }
            });
            // 다른 트래커를 가리키는 토막은 **그 트래커가 본다**(moai-23ky). 판정을 잇는 차례는
            // `Decision::then` 이 정한다 — 막으면 남의 트래커는 묻지 않고, 남이 막으면 제 비춤을 버린다.
            let mut decision = decision;
            if let Call::Shell(_) = call {
                for (n, other) in there.iter().enumerate() {
                    decision = decision.then(|| {
                        let Ok(load) = other.read() else { return Decision::Pass };
                        let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                        // 사람은 **그 트래커의 뿌리에서** 푼다 — 남의 저장소의 git 설정이 그 저장소의 사람이다.
                        let away = away_of(other, &load.issues, &person_at(other, ctx));
                        settle(input, other, &load.issues, away, &|issues, away| {
                            crate::hook::guard_moai(issues, &other.config, away, line, &only, &|_| {
                                Some(crate::hook::Aimed::stands(other.root.as_path()))
                            })
                        })
                    });
                }
            }
            // **막지 않은 집기는 이 세션의 것으로 적는다**(moai-4jsy). 판정 뒤에 적는다 — 막힌 명령은
            // 안 돈다. **`--from` 에 질 집기는 안 적는다**(`hook::picked_in`) — 그 토막이 겨눈 트래커의 지금
            // 칸을 댄다. 적던 판은 진 쪽을 마지막으로 집은 세션으로 세어, 진 쪽이 이긴 쪽의 줄로 붙들리고
            // 막혔다(리뷰 moai-3k2d.1df). 명령이 돌기 전과 도는 사이에 옆이 집는 틈은 남는다.
            if let Call::Shell(_) = call
                && !decision.blocks()
            {
                // **같은 트래커를 토막마다 다시 열지 않는다**(moai-47zz) — 토막과 id 마다 디스크를
                // 훑어 스냅샷을 통째로 다시 읽던 판은, 이 저장소가 스스로 일러 주는
                // `moai -C <루트> mv <id> …` 꼴에서 그 파일을 토막 수 곱하기 id 수만큼 읽었다.
                // 가리킨 자리가 같으면 한 번만 읽고, [`route`] 가 이미 푼 트래커는 그대로 쓴다.
                //
                // **`--from` 이 없으면 아무도 안 묻는다**(리뷰 moai-bujq.91c) — `picked_in` 은
                // `--from` 이 선 토막에서만 `stands` 를 부른다. 미리 읽어 두던 판은 `moai -C <루트>
                // show` 한 줄에도 그 스냅샷을 통째로 갈라, 이 줄이 줄이려던 값을 **도구 호출마다**
                // 도로 치렀다(이 저장소의 1,700줄짜리 트래커로 재서 한 판에 67ms).
                //
                // **글자로 본다** — `picked_in` 이 `mv` 를 그렇게 보는 것과 같은 자다. 따옴표를
                // 끼워 적은 `--fro"m"` 은 여기 안 걸리지만 저쪽 `mv` 문턱도 같이 안 걸리니, 새
                // 어긋남을 여는 것이 아니다. 훅은 도구 호출마다 돈다.
                let asks = matches!(call, Call::Shell(cmd) if cmd.contains("--from"));
                // 자리마다 그 스냅샷의 자리 — `None` 이면 이 세션의 것이다.
                let mut seen: Vec<(&Path, Option<usize>)> = Vec::new();
                let mut snaps: Vec<Option<Vec<model::Issue>>> = Vec::new();
                // 토막마다 그 스냅샷의 자리 — 없으면 이 세션의 것이다(가리킨 곳이 없는 흔한 토막).
                let mut snap_at: Vec<Option<usize>> = Vec::new();
                for (k, dir) in dirs.iter().enumerate().filter(|_| asks) {
                    let Some(dir) = dir else {
                        snap_at.push(None);
                        continue;
                    };
                    if let Some((_, at)) = seen.iter().find(|(p, _)| *p == dir.as_path()) {
                        snap_at.push(*at);
                        continue;
                    }
                    let found = match routes.get(k) {
                        Some(Route::There(n)) => there.get(*n).map(std::borrow::Cow::Borrowed),
                        _ => {
                            Repo::find_from(dir, crate::i18n::Lang::default).ok().flatten().map(std::borrow::Cow::Owned)
                        }
                    };
                    // **이 세션의 트래커면 이미 손에 있다** — 같은 파일을 한 판에 두 번 가르지
                    // 않는다. `cd <제 밑> && moai mv …` 와 워크트리에서 루트를 겨눈 흔한 꼴이 그
                    // 자리다([`crate::store::Repo::find_from`] 이 루트로 옮겨 준다).
                    let at = match found {
                        Some(r) if same(&r.root, &repo.root) => None,
                        found => {
                            snaps.push(found.and_then(|r| r.read().ok()).map(|l| l.issues));
                            Some(snaps.len() - 1)
                        }
                    };
                    seen.push((dir.as_path(), at));
                    snap_at.push(at);
                }
                let stands = |k: usize, id: &str| -> Option<String> {
                    let issues = match snap_at.get(k).copied().flatten() {
                        None => &load.issues,
                        Some(n) => snaps[n].as_deref()?,
                    };
                    issues.iter().find(|i| i.id == id).map(|i| i.status.as_str().to_string())
                };
                record_picks(input, &repo, &crate::hook::picked_in(&scan, &mine, &stands));
                // **남의 트래커는 제 설정으로 다시 훑는다** — 훑는 답이 `cfg` 에 달렸다
                // ([`crate::hook::Scan`]). 흔한 줄에는 이 고리가 아예 안 돈다.
                //
                // **이름을 달리 준다** — 위의 `scan` 과 형이 같아 그냥 `scan` 으로 두면 이 줄을
                // 지우거나 밖으로 옮겨도 컴파일이 되고, 그때 집기는 **제 설정의 칸 이름**으로
                // 세어진다(`picks_up` 이 `cfg` 에서 읽는 그것이다).
                for (n, other) in there.iter().enumerate() {
                    let only = |k: usize| routes.get(k) == Some(&Route::There(n));
                    let theirs = crate::hook::Scan::new(line, &other.config);
                    record_picks(input, other, &crate::hook::picked_in(&theirs, &only, &stands));
                }
            }
            decision
        }
        // **편지가 먼저다**(moai-h8tn) — 이 세션에 온 편지가 있으면 그것으로 턴을 붙든다. 닫기 물음(`closing`)은
        // 편지를 다 본 뒤의 `Stop` 이 묻는다: 함께 실으면 [`Decision::then`] 이 막는 답 하나만 남기는데, 버려진
        // 쪽이 편지면 읽음으로 옮긴 편지가 아무에게도 안 실린다. 편지는 실은 만큼 줄어 붙듦에 끝이 있으므로
        // `stop_hook_active` 여도 싣는다 — 보낸 이가 쉬지 않고 보내면 일이 쉬지 않고 오는 것이다.
        //
        // **편지 뒤의 `Stop` 은 `stop_hook_active` 로 온다**(리뷰 moai-h8tn.x4l) — 그 표만 보고 보내던 판은 편지가 온
        // 턴마다 닫기 물음을 통째로 걸렀다(그 턴에 쥔 일·늘어난 경고를 아무도 안 물었다). 편지로 붙들 때 세션 표
        // (`letters`)를 남기고, 그 표가 선 `Stop` 은 표를 걷으며 닫기 물음을 묻는다. 그래도 끝없이 돌지 않는 것은
        // 표가 한 번 걷히고 닫기 물음이 세션에 한 번(`once_per_session`)이기 때문이다.
        //
        // **출석은 편지를 옮기기 전에 적는다**(moai-jzym.flj) — 접힌 뒤·턴 머리와 같은 까닭이다: 옮긴 뒤의 쓰기가 늦어(저장소가
        // 선 Ceph 가 멈춘 날) 훅이 시간을 넘기면, 심은 셸 줄은 `moai` 가 끝난 뒤에야 글을 흘려 `decision: block` 이 버려지고
        // 편지만 읽음으로 남는다. 판정 뒤에 적던 판이 그 자리였다 — 붙들었는지를 판정 뒤에야 알아서다. 실을 편지가 있으면
        // 붙드니 `busy` 로 먼저 적고, 없으면 옮길 것이 없어 예전처럼 판정 뒤에 한 번 적는다. 먼저 적은 뒤 한 통도 못 옮기고
        // 붙들지도 않은 판만 한 번 더 쓴다 — 남이 먼저 가진 `any-idle-worker` 편지, 그 사이 `inbox --ack` 나 걷기가 옮긴
        // 편지, 옮기기가 진 판(`read/` 를 못 짓거나 `rename` 이 진다 — 이어 지면 그 편지가 기다리는 `Stop` 마다다)이다.
        // **창이 다 닫힌 것은 아니다**(리뷰 moai-jzym.a9k) — 첫 편지를 옮긴 뒤에도 나머지 편지의 `rename` 과 `TMPDIR` 의
        // 표(`letters`) 쓰기가 남아, 그 사이에 멈추면 같은 일이 난다. 닫으려면 옮기기를 출력 뒤로 미뤄야 한다.
        Event::Stop => {
            let me = attendee(input, &post, dialect);
            let pending = me.as_ref().map(|p| waiting(&post, p, Mine::Others)).unwrap_or_default();
            let busy_first = !pending.is_empty();
            if busy_first {
                attend(&agents, me.clone(), mail::BUSY);
            }
            let letters = me.as_ref().and_then(|p| deliver(&post, p, ctx, hold_room(dialect), pending));
            let held_by_letters = session_file(input, &repo, "letters");
            let decision = match letters {
                Some(said) => {
                    if let Some(path) = &held_by_letters {
                        let _ = std::fs::write(path, "");
                    }
                    Decision::Block(said)
                }
                None => {
                    let after_letters = held_by_letters.is_some_and(|path| std::fs::remove_file(path).is_ok());
                    // **이미 한 번 붙들었으면 보낸다.** 이 표를 안 보면 무한히 돈다 — 편지가 붙든 턴은 빼고.
                    if input.stop_hook_active && !after_letters {
                        Decision::Pass
                    } else {
                        closing_hold(input, &repo, &load.issues, &unreadable, ctx)
                    }
                }
            };
            // 턴이 끝나면 논다 — 붙들었으면 아직 일하는 중이다(먼저 적은 `busy` 가 그대로 맞다).
            let held = decision.blocks();
            if !(busy_first && held) {
                attend(&agents, me, if held { mail::BUSY } else { mail::IDLE });
            }
            decision
        }
        // 위에서 이미 보냈다 — 트래커를 찾기 전이다.
        Event::StopFailure | Event::Interrupt | Event::SessionEnd => Decision::Pass,
    };
    Some(decision)
}

/// `Stop` 이 붙드는 까닭(`reason`)에 편지가 들 자리 — 말씨마다 다르다(moai-rxro). Codex 는 그 글을 이어 가는
/// 프롬프트로 실어 기본 상한에 묶고, 심은 파일의 `additionalContextLimit` 은 거기 안 닿는다([`crate::hook::CODEX_HOLD`]).
/// 칸 하나([`crate::hook::Room::CONTEXT`])로 재던 판은 그 상한을 넘긴 한국어 편지의 가운데를 Codex 가 파일로 빼, 읽음으로
/// 옮긴 그 자리를 아무도 못 봤다.
///
/// **Antigravity 는 칸 하나를 통째로 싣는다**(moai-jzym.4pm, 2026-10-05 실측) — 상한이 문서에 없어 사람이 띄운 대화형 agy
/// 창에 한국어 편지를 보내 쟀다. 칸을 거의 다 채운 글이 턴 머리의 `ephemeralMessage` 로도, 여기 `Stop` 의
/// `decision: continue` 로도 자르지도 파일로 빼지도 않고 그대로 실렸다(가운데에 고루 박은 표지 스물이 다 섰다).
///
/// - agy 1.2.16 — 영문이 섞인 편지, UTF-16 9,665·9,874 단위(UTF-8 21KB 남짓)
/// - agy 1.2.17 — **한글로만 채운** 편지, UTF-16 9,822·10,024 단위(UTF-8 28.2·28.4KB). 칸은 UTF-16 으로 세므로 바이트로
///   가장 큰 편지가 이 꼴이다 — agy 의 선이 Codex 처럼 바이트로 서 있다면 여기서 드러났다(리뷰 moai-jzym.a9k 가 짚은 틈)
///
/// 그 위의 선은 안 쟀다 — 우리가 내는 글이 이 칸을 안 넘으니 물을 까닭이 없다.
fn hold_room(dialect: Dialect) -> crate::hook::Room {
    match dialect {
        Dialect::Codex => crate::hook::CODEX_HOLD,
        Dialect::Claude | Dialect::Antigravity => crate::hook::Room::CONTEXT,
    }
}

/// 턴 끝의 닫기 물음 — 세션에 한 번 붙든다(`once_per_session`). 편지가 붙든 턴은 그 뒤의 `Stop` 이 묻는다([`decide`]).
fn closing_hold(
    input: &Input,
    repo: &Repo,
    issues: &[model::Issue],
    unreadable: &[report::Unreadable],
    ctx: &Ctx,
) -> Decision {
    once_per_session(input, repo, "stop", || {
        let now = model::now();
        let st = report::status(issues, unreadable, &repo.config, &now, ctx.zone());
        // **고칠 것만 센다.** 알림(쌓인 생각·미뤄 둔 것)은 `notices` 에 따로
        // 있다 — 여기 섞이던 때 `defer` 만 해도 "경고가 늘었다" 로 세션이
        // 붙들렸다(moai-c8lb). 기준선도 같은 자로 잰다.
        let warnings: usize = st.warnings.iter().map(|w| w.count).sum();
        // **누구의 것인지 모르는 줄로는 붙들지 않는다**(moai-ntl6). 에픽이 닫히는지와 집은 줄이 아직
        // 집혀 있는지는 옆까지 겹친 줄로 잰다(moai-8ema). 둘 다 [`releasing`] 이 잰다.
        let (away, latest) = releasing(input, repo, issues, &person_at(repo, ctx));
        crate::hook::closing(
            issues,
            latest.as_deref().unwrap_or(issues),
            &repo.config,
            &away,
            warnings,
            baseline(input, repo),
            ctx.lang(),
        )
    })
}

/// 판정을 계약 JSON 한 줄로 옮긴다 — `Pass` 는 아무 말도 안 한다. Codex 의 꼴은 Claude 와 같다(2026-10-04 실측:
/// `permissionDecision: deny` 가 막고 `decision: block` 이 붙들었다).
fn answer(event: Event, decision: Decision, dialect: Dialect) -> Option<String> {
    if dialect == Dialect::Antigravity {
        return antigravity_answer(event, decision);
    }
    match decision {
        Decision::Pass => None,
        Decision::Context(context) => {
            serde_json::to_string(&Out { specific: Specific { event: event.wire(), context } }).ok()
        }
        Decision::Deny(reason) => {
            serde_json::to_string(&Refusal { specific: RefusalBody { event: event.wire(), decision: "deny", reason } })
                .ok()
        }
        Decision::Block(reason) => serde_json::to_string(&Hold { decision: Verdict::Block, reason }).ok(),
    }
}

/// Antigravity 의 꼴(2026-10-04 agy 1.2.16 실측, moai-u5wr.amg 노트). 막는 것은 맨 윗단 `decision: deny`, 턴을 붙드는
/// 것은 `Stop` 의 `decision: continue`(까닭이 시스템 메시지로 선다), 싣는 것은 `injectSteps` 의 `ephemeralMessage` 다.
///
/// **`allow` 는 안 낸다** — 그 답은 사람이 물어야 할 도구 부름까지 허락해 버린다. 지나가는 것은 빈 출력이다(재 보니
/// 빈 출력에서 도구가 그대로 돌았다).
///
/// **도구 부름 앞의 비추는 줄은 버린다.** `PreToolUse` 의 답에는 글을 실을 칸이 `reason` 뿐이고 그 칸은 `decision`
/// 에 딸린다 — `deny` 로 실으면 막고, `allow` 로 실으면 허락한다. Claude·Codex 가 받는 그 줄(fork 1 의 둘째 물음
/// 같은 것)이 여기서는 안 선다.
fn antigravity_answer(event: Event, decision: Decision) -> Option<String> {
    match decision {
        Decision::Pass => None,
        Decision::Deny(reason) => serde_json::to_string(&Hold { decision: Verdict::Deny, reason }).ok(),
        Decision::Block(reason) => serde_json::to_string(&Hold { decision: Verdict::Continue, reason }).ok(),
        Decision::Context(text) => match event {
            Event::UserPromptSubmit | Event::SessionStart => {
                serde_json::to_string(&serde_json::json!({ "injectSteps": [{ "ephemeralMessage": text }] })).ok()
            }
            _ => None,
        },
    }
}

/// 에이전트가 준 것을 판정이 읽는 꼴([`Input`])로 옮긴다 — 대개 하나고, 판정할 것이 없으면 비었다.
///
/// - **Claude** 는 그대로다
/// - **Codex** 도 키가 같다. 패치(`apply_patch` 도구와 셸로 친 `apply_patch`)만 고치는 파일마다 `Edit` 하나로
///   편다([`patched`])
/// - **Antigravity** 는 [`from_antigravity`] 가 옮긴다 — 이벤트도 옮길 수 있어 함께 낸다
fn arrived(dialect: Dialect, event: Event, raw: &str) -> (Event, Vec<Input>) {
    match dialect {
        Dialect::Claude => (event, vec![serde_json::from_str(raw).unwrap_or_default()]),
        Dialect::Codex => (event, patched(serde_json::from_str(raw).unwrap_or_default())),
        Dialect::Antigravity => match from_antigravity(event, raw) {
            Some((event, input)) => (event, vec![input]),
            None => (event, Vec::new()),
        },
    }
}

/// Codex 의 패치를 고치는 파일마다 `Edit` 하나로 편다 — 패치 글은 `tool_input.command` 에 든다(Codex 의 훅 문서).
/// **고치는 자리는 패치의 머리 줄이 댄다** — `*** Add File:`·`*** Update File:`·`*** Delete File:`·`*** Move to:` 다.
/// 옮기는 패치는 떠나는 자리와 닿는 자리를 둘 다 고치는 것으로 본다. 상대 경로는 패치가 풀리는 자리에 붙인다 — 세션의
/// 자리(`cwd`)이고, 셸로 친 패치가 `cd` 로 옮겨 갔으면 그 자리다.
///
/// **패치는 두 길로 온다**(리뷰 moai-u5wr.e74) — `apply_patch` 도구(`tool_name: "apply_patch"`)와, 셸 도구로 친
/// `apply_patch <<'EOF' … EOF`·`cd <자리> && apply_patch <<'EOF' …` 다. 뒤의 것은 훅에 셸 부름(`Bash`)으로만 오고, Codex 는
/// 셸을 안 띄운 채 그 자리에서 패치를 푼다(codex-rs `exec_command.rs` 의 `intercept_apply_patch`). 셸 줄로만 보던 판은
/// 거기서 쓰는 것을 못 봐 규칙 2 가 통째로 샜다 — 기록한 두 판의 모델도 셸로 고쳤다. 셸로 온 패치는 파일들을 먼저 보고
/// 그 셸 줄도 그대로 판정한다. 파일을 먼저 보는 것은 막힐 때 셸 줄이 남기는 기록(집기)이 안 서게 하려는 것이다.
///
/// **머리 줄은 앞 빈칸을 걷고 읽는다** — Codex 의 풀이가 그렇다(`apply-patch` 의 `line.trim()`). 줄 머리에서만 찾던
/// 판은 Codex 가 그대로 푸는 들여 쓴 머리 줄의 파일을 판정 없이 보냈다.
///
/// **자리를 하나도 못 읽은 패치는 그대로 둔다** — 도구 이름이 `apply_patch` 라 판정이 아무 뜻도 안 둔다(`Call::Other`).
/// 꼴을 모르는 패치를 막으면 고칠 길이 없는 거절이 선다.
fn patched(input: Input) -> Vec<Input> {
    let text = input.tool_input.get("command").and_then(|v| v.as_str()).unwrap_or_default();
    // 패치가 풀리는 자리(`cd` 로 옮겨 간 곳)와, 셸로 온 패치인가.
    let (under, shell) = match input.tool_name.as_deref() {
        Some("apply_patch") => (None, false),
        Some("Bash") => match shell_patch(text) {
            Some(under) => (under, true),
            None => return vec![input],
        },
        _ => return vec![input],
    };
    let paths = patch_paths(text);
    if paths.is_empty() {
        return vec![input];
    }
    let base = match (input.cwd.as_deref(), under.as_deref()) {
        (Some(cwd), Some(dir)) => Some(Path::new(cwd).join(dir)),
        (Some(cwd), None) => Some(PathBuf::from(cwd)),
        (None, Some(dir)) if Path::new(dir).is_absolute() => Some(PathBuf::from(dir)),
        (None, _) => None,
    };
    let at = |path: &str| match &base {
        Some(base) if Path::new(path).is_relative() => base.join(path).display().to_string(),
        _ => path.to_string(),
    };
    let mut fanned: Vec<Input> = paths
        .iter()
        .map(|path| Input {
            session_id: input.session_id.clone(),
            cwd: input.cwd.clone(),
            source: input.source.clone(),
            tool_name: Some("Edit".to_string()),
            tool_input: serde_json::json!({ "file_path": at(path) }),
            stop_hook_active: input.stop_hook_active,
            model_raw: input.model_raw.clone(),
            agent_id: input.agent_id.clone(),
        })
        .collect();
    if shell {
        fanned.push(input);
    }
    fanned
}

/// 패치 글의 머리 줄이 대는 자리 — 나온 차례로, 겹친 것은 한 번.
fn patch_paths(patch: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for line in patch.lines() {
        let named = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"]
            .iter()
            .find_map(|head| line.trim_start().strip_prefix(head))
            .map(str::trim)
            .filter(|p| !p.is_empty());
        if let Some(path) = named
            && !paths.iter().any(|p| p == path)
        {
            paths.push(path.to_string());
        }
    }
    paths
}

/// 셸로 친 패치인가 — Codex 가 셸을 안 띄우고 그 자리에서 푸는 꼴(`apply_patch <<'EOF' …`, `cd <자리> && apply_patch
/// <<'EOF' …`, 이름이 `applypatch` 여도 같다)이면 `cd` 가 옮겨 간 자리를(안 옮겼으면 `None`) 낸다. 첫 줄만 본다 —
/// 패치의 몸은 그 뒤의 heredoc 이다.
fn shell_patch(cmd: &str) -> Option<Option<String>> {
    let head = cmd.lines().next()?.trim_start();
    let (under, rest) = match head.strip_prefix("cd").filter(|after| after.starts_with(char::is_whitespace)) {
        Some(after) => {
            let (dir, rest) = after.split_once("&&")?;
            let dir = dir.trim();
            let bare = ['\'', '"'].iter().find_map(|q| dir.strip_prefix(*q).and_then(|d| d.strip_suffix(*q)));
            (Some(bare.unwrap_or(dir).to_string()), rest.trim_start())
        }
        None => (None, head),
    };
    let word = rest.split(|c: char| c.is_whitespace() || c == '<').next()?;
    matches!(word, "apply_patch" | "applypatch").then_some(under)
}

/// Antigravity 가 준 것을 [`Input`] 으로 — 판정할 것이 없으면 `None` 이다(2026-10-04 agy 1.2.16 실측).
///
/// - 세션은 `conversationId`, 모델은 `modelName` 이다
/// - **자리는 그 명령이 도는 자리(`toolCall.args.Cwd`)가 먼저고, 없으면 첫 작업 자리(`workspacePaths`)다** — 훅
///   프로세스는 hooks.json 이 놓인 디렉터리에서 돈다. 셸 명령은 `Cwd` 에서 돌고, 판정은 거기서 상대 경로를 푼다
/// - `run_command` 는 셸(`Bash`)이고, 파일을 쓰는 셋(`write_to_file`·`replace_file_content`·
///   `multi_replace_file_content`)은 `TargetFile` 을 고치는 것(`Edit`)이다. **나머지는 이름에 접두어를 붙여
///   넘긴다** — 같은 이름의 Claude 도구로 읽히지 않게
/// - `Stop` 이 이미 한 번 붙들었는지는 `executionNum` 이 댄다(붙든 뒤의 `Stop` 이 1 이었다)
/// - **턴 머리만 `UserPromptSubmit` 이다** — `PreInvocation` 은 모델을 부를 때마다 오고, 사람이 친 턴의 첫 부름이
///   `invocationNum: 0` 이다. 그 뒤의 부름은 판정할 것이 없다
/// - **오류로 끝난 실행의 `Stop` 은 `StopFailure` 다**(리뷰 moai-u5wr.e74) — agy 는 API 오류로 끝난 실행에도 `Stop` 을
///   내고 그 까닭을 `error`(정상 판은 빈 글이다)·`terminationReason` 에 싣는다. 보통 `Stop` 으로 판정하던 판은 편지를
///   읽음으로 옮겨 `decision: continue` 로 실패하는 백엔드에 도로 밀어 넣고, 세션에 한 번인 닫기 물음을 그 판에 써
///   버렸다. Claude 의 API 오류(`StopFailure`)처럼 출석만 `idle` 로 돌린다
fn from_antigravity(event: Event, raw: &str) -> Option<(Event, Input)> {
    use serde_json::Value;
    let v: Value = serde_json::from_str(raw).unwrap_or_default();
    if event == Event::UserPromptSubmit && v.get("invocationNum").and_then(Value::as_u64).is_some_and(|n| n != 0) {
        return None;
    }
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let failed = text(&v, "error").is_some_and(|e| !e.trim().is_empty())
        || text(&v, "terminationReason").is_some_and(|r| r.eq_ignore_ascii_case("error"));
    let event = if event == Event::Stop && failed { Event::StopFailure } else { event };
    let call = v.get("toolCall").cloned().unwrap_or_default();
    let args = call.get("args").cloned().unwrap_or_default();
    let workspace = v.get("workspacePaths").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str);
    let cwd = text(&args, "Cwd").filter(|c| Path::new(c).is_absolute()).or(workspace.map(str::to_string));
    let (tool_name, tool_input) = match text(&call, "name").as_deref() {
        None => (None, Value::Null),
        Some("run_command") => {
            (Some("Bash".to_string()), serde_json::json!({ "command": text(&args, "CommandLine").unwrap_or_default() }))
        }
        Some("write_to_file" | "replace_file_content" | "multi_replace_file_content") => (
            Some("Edit".to_string()),
            serde_json::json!({ "file_path": text(&args, "TargetFile").unwrap_or_default() }),
        ),
        Some(other) => (Some(format!("antigravity:{other}")), args),
    };
    let input = Input {
        session_id: text(&v, "conversationId"),
        cwd,
        source: None,
        tool_name,
        tool_input,
        stop_hook_active: v.get("executionNum").and_then(Value::as_u64).is_some_and(|n| n > 0),
        model_raw: v.get("modelName").cloned().unwrap_or_default(),
        agent_id: None,
    };
    Some((event, input))
}

/// 판정하되, **막으면 옆 워크트리와 겹쳐 한 번 더 본다**(moai-w2iy).
///
/// 워크트리의 스냅샷은 main 에서 방금 세우고 집은 줄을 모른다 — 그것만 보고 막으면 시킨 대로
/// 한 일이 막힌다. 겹쳐 봐도 막힐 때만 막고, **까닭은 마지막으로 다시 본 판의 것**이다
/// (moai-15c2) — 겹친 줄과 모름을 함께 본 판이 지금 가장 참에 가깝다. 고칠 명령이 이 자리의
/// 트래커에 듣는 것은 [`crate::hook::Toward`] 가 따로 지킨다.
/// **겹쳐 보기는 막을 때만 치른다** — 지나가는 호출은 전과 같은 값이다.
///
/// **다시 본 판정이 안 막으면 풀린 것이다** — 비추는 줄(`Context`)도 푼 답이라 그대로 낸다.
/// `Pass` 만 풀린 것으로 치던 판은 `idea add` 하나를 곁들인 명령줄을 낡은 스냅샷의 거절로 도로
/// 막았다(moai-dw63.e31) — 그 거절은 이미 집은 일을 집으라고 시켰다.
///
/// **겹친 판의 이름도 사람을 싣는다**(moai-0zjo) — `worktree::fresh` 가 짓는 이름에는 사람이 없어,
/// 안 실으면 겹쳐 본 판정에서만 남의 줄이 도로 제 초점이 된다. `base` 의 사람을 그대로 옮긴다 — 한
/// 칸을 나눠 쓰니([`crate::hook::Person`]) 다시 안 푼다.
fn settle(
    input: &Input,
    repo: &Repo,
    issues: &[model::Issue],
    base: crate::hook::Away,
    judge: &dyn Fn(&[model::Issue], &crate::hook::Away) -> Decision,
) -> Decision {
    let first = judge(issues, &base);
    if first == Decision::Pass {
        return first;
    }
    // **겹침과 모름을 한 판에 얹어 한 번 다시 본다**(moai-15c2, 사용자 결정). 따로 보던 판은 둘이
    // 함께일 때 풀릴 것을 못 풀었다 — 겹친 줄의 판정은 모름을 안 뺐고, 모름을 뺀 판정은 낡은 제
    // 스냅샷으로 쟀다. **겹친 줄은 겹친 목록의 이름으로 잰다** — 옆 이름도 제 이름도 그 목록에서
    // 읽는다(moai-m62u). `base` 는 제 스냅샷에 집은 줄이 없으면 워크트리를 안 읽어([`away_of`]) 제
    // 이름이 비는데, 겹쳐 보는 까닭이 바로 그 스냅샷이 main 의 집기를 모를 때다 — 빈 이름으로
    // 재던 판은 제 이름 워크트리의 멤버를 옆 에픽 워크트리에 넘겨 규칙 2 로 막았다(리뷰
    // moai-3k2d.1df). **겹쳐 보기는 막을 때만 치른다** — 비추기만 하는 답은 제 줄로 좁히기만 한다.
    let overlaid = first.blocks().then(|| crate::worktree::fresh(repo, issues.to_vec())).flatten();
    let seen = overlaid.is_some();
    // **빌려 쓴다** — 겹치지 않은 판의 줄은 부르는 쪽의 것 그대로다. 통째로 베끼던 판은 막거나 비추는
    // 호출마다 스냅샷 전체를 복제했고, 훅은 도구 호출마다 돈다.
    let (rows, mut narrow): (std::borrow::Cow<'_, [model::Issue]>, _) = match overlaid {
        Some((fresh, beside)) => (std::borrow::Cow::Owned(fresh), crate::hook::Away { me: base.me.clone(), ..beside }),
        None => (std::borrow::Cow::Borrowed(issues), base),
    };
    // **겹쳐 보기만으로 풀리면 거기서 끝낸다** — 모름을 재는 값(옆 스냅샷을 다시 읽고 세션의 기록을
    // 훑는 일)을 안 치른다. 겹치지 않은 판은 `first` 가 곧 이 답이라 다시 재지 않는다.
    let wide = if seen { judge(&rows, &narrow) } else { first };
    if wide == Decision::Pass {
        return wide;
    }
    // **모르는 줄도 같은 판에서 뺀다**(moai-ntl6, 사용자 결정 B) — 옆 워크트리가 쥐었을 일을 초점으로
    // 대지 않는다. **비추는 줄도 같은 자로 좁힌다** — 막지도 붙들지도 않기로 한 줄의 에픽을 제 물음으로
    // 비추면, 그 세션을 남의 에픽에 세우는 길로 보낸다(`idea promote -e <남의 에픽>`).
    let added = add_unsure(input, repo, &rows, &mut narrow);
    if !added {
        return wide;
    }
    // 막히면 **그 판정의 까닭**을 낸다 — 겹친 줄과 모름을 함께 본 판이 지금 가장 참에 가깝다.
    let again = judge(&rows, &narrow);
    // **좁힌 초점은 풀기만 한다**: 덜 좁힌 판이 안 막는데 좁혀서 막으면 덜 좁힌 답을 낸다. 둘을 함께
    // 본 판이 겹쳐 보기가 푼 것을 도로 막던 자리다 — 모름은 초점에서 빼기만 하고, 초점이 비면 규칙 2 가
    // 막는다. 사용자 결정 moai-4jsy 의 "새로 막는 일은 없다" 가 여기에도 선다.
    //
    // **`first` 를 따로 다시 대지 않는다** — `first` 가 안 막으면 겹쳐 보지도 않아(`first.blocks()`)
    // `wide` 가 곧 `first` 다. 두 자로 적던 판은 같은 것을 두 번 물었다.
    if again.blocks() && !wide.blocks() { wide } else { again }
}

/// **이 세션의 사람을 푸는 길** — 그 트래커의 뿌리에서 푼다(`cmd::me_of`, `ready`·`prime` 과 한 자). 푸는 것은
/// 줄의 담당을 실제로 가르는 규칙이 처음 물을 때 한 번이다([`crate::hook::Person`]) — `model::actor` 는
/// `git` 을 두 번 띄우는데, 훅은 도구 호출마다 돌고 그 태반(`ls`·`cargo test`·저장소 밖의 쓰기)은 초점을
/// 안 읽는다.
fn person_at(repo: &Repo, ctx: &Ctx) -> crate::hook::Person {
    let (user, root) = (ctx.user.clone(), repo.root.clone());
    crate::hook::Person::asked(move || super::me_of(user.as_deref(), &root))
}

/// **옆 워크트리가 쥔 일은 제 초점이 아니다** (`hook::held`). 워크트리 목록은 집은 것이 있을 때만
/// 읽는다 — 훅은 도구 호출마다 돌고, 집은 것이 없으면 뺄 것도 없다.
///
/// **담당이 내가 아닌 줄도 뺀다**(moai-0zjo) — 사람은 늘 싣되 묻는 자리에서 푼다([`person_at`]). 집은 것이
/// 없어도 싣는다 — 규칙 5 는 집은 것이 없는 세션의 첫 집기에서 가장 자주 선다.
fn away_of(repo: &Repo, issues: &[model::Issue], me: &crate::hook::Person) -> crate::hook::Away {
    if report::wip(issues, &repo.config).is_empty() {
        return crate::hook::Away { me: me.clone(), ..Default::default() };
    }
    // **제 이름은 세션이 선 체크아웃에서 읽는다 — 트래커의 자리가 아니다**(moai-y7go). 트래커를
    // 찾는 길이 딸린 워크트리를 루트로 옮기므로(`Repo::find_from`) `repo.root` 는 늘 루트다. 거기서
    // 이름을 읽으면 워크트리 안에서 도는 세션이 제 이름을 잃고, 제 워크트리가 쥔 일이 통째로 "옆의
    // 것" 이 되어 규칙 2 가 그 자리의 쓰기를 막는다.
    crate::hook::Away { me: me.clone(), ..crate::worktree::away(repo.here()) }
}

/// `away` 에 **누구의 것인지 모르는** 집은 줄(`hook::unsure`)을 더한다 — 옆 딸린 워크트리가 쥐었을 수
/// 있거나 다른 세션이 마지막으로 집은 줄이다. 이 세션이 마지막으로 집은 줄도 함께 싣는다(`Away::picked`) —
/// 모르는 줄 밑에 있어도 제 것이다. 제 이름은 `away` 의 것을 쓴다. 더한 것이 없으면 거짓이다.
fn add_unsure(input: &Input, repo: &Repo, issues: &[model::Issue], away: &mut crate::hook::Away) -> bool {
    let elsewhere = crate::worktree::held_elsewhere(repo.here(), issues, &repo.config);
    let picks = read_picks(input, repo, issues);
    let unsure = crate::hook::unsure(issues, &repo.config, &elsewhere, &away.own, &picks);
    if unsure.is_empty() {
        return false;
    }
    away.unsure.extend(unsure);
    away.picked.extend(picks.mine);
    true
}

/// 풀기만 하는 판정(`Stop` 과 접힌 뒤 싣는 것)의 자 — 누구의 것인지 모르는 줄을 더한 `Away`
/// ([`add_unsure`])와, 집은 줄이 아직 집혀 있는지·에픽이 닫히는지 되짚을 겹친 줄(`worktree::fresh`,
/// moai-8ema·리뷰 moai-dw63.nzw)이다. 둘 다 집은 것이 남을 때만 잰다 — `Stop` 은 턴마다 돈다.
///
/// **두 자리가 한 자를 쓴다**(리뷰 moai-3k2d.1df) — 따로 적던 판은 접힌 뒤 싣는 쪽이 되짚기를 빠뜨려,
/// main 이 이미 닫은 줄을 "압축 전부터 집고 있다" 로 실었다.
fn releasing(
    input: &Input,
    repo: &Repo,
    issues: &[model::Issue],
    me: &crate::hook::Person,
) -> (crate::hook::Away, Option<Vec<model::Issue>>) {
    let mut away = away_of(repo, issues, me);
    if crate::hook::held(issues, &repo.config, &away).is_empty() {
        return (away, None);
    }
    let still =
        !add_unsure(input, repo, issues, &mut away) || !crate::hook::held(issues, &repo.config, &away).is_empty();
    let latest = still.then(|| crate::worktree::fresh(repo, issues.to_vec())).flatten().map(|(fresh, _)| fresh);
    (away, latest)
}

/// 세션의 집기를 적는 자리(moai-4jsy) — **같은 저장소의 같은 트래커면 어느 워크트리에서 재도 같은
/// 자리다**(`worktree::tracker_place` — 공용 git 디렉터리와 그 안의 트래커 자리). 집기는 루트에서 치고
/// 일은 딸린 워크트리에서 하므로, 워크트리마다 제 뿌리로 잡으면 루트에서 적은 것을 워크트리에서 못
/// 읽는다. main 워크트리의 뿌리로 잡던 판은 main 이 없는 맨몸 저장소에서 그렇게 갈렸다(리뷰
/// moai-3k2d.1df). 세션마다 파일 하나다 — 여럿이 한 파일에 덧붙이면 겨룬다.
///
/// **std 의 해셔를 안 쓴다**(moai-2vrw, `text::fnv1a64`) — 세션마다 따로 뜨는 훅이 같은 자리를 찾아야
/// 하는데, `DefaultHasher` 는 러스트 판마다 값이 달라져도 된다고 문서가 밝혀 다시 빌드한 바이너리가 옛
/// 기록을 못 찾는다.
///
/// **자리는 사용자 설정 곁이다 — 누구나 쓰는 temp 가 아니다**(moai-59k3.yo6, 2026-09-29 사용자 결정).
/// temp 의 이름은 넘겨짚을 수 있어, 남이 미리 만든 디렉터리나 심어 둔 기록 하나가 규칙 1·2 를 조용히
/// 껐다 — 기록이 없거나 남는 것은 일부러 조용히 지나가게 둔 자리라 아무 말도 안 났다. 설정의 자리를
/// 모르거나 상대경로면 적지도 읽지도 않는다 — 상대경로는 훅이 선 자리마다 딴 디렉터리가 되고, 빠진
/// 기록은 전과 같은 판정이다.
fn picks_dir(repo: &Repo) -> Option<PathBuf> {
    let config = crate::user_config::path().filter(|p| p.is_absolute())?;
    Some(config.parent()?.join("picks").join(format!("{:016x}", picks_key(&repo.root))))
}

/// [`picks_dir`] 의 이름 — 트래커의 자리를 센 값이다.
///
/// **`MOAI_HERE` 로 루트에 안 옮긴 딸린 워크트리는 제 뿌리로 센다**(moai-59k3.yo6). 그 워크트리와
/// 루트는 `tracker_place` 가 같은 값(공용 디렉터리, 꼭대기에서 뿌리까지)을 내는데 보는 스냅샷은
/// 다르다 — 한 자리에 적던 판은 루트의 남의 집기 하나가 워크트리의 `held()` 를 비지 않게 만들어
/// 규칙 2 를 껐다. 옮길 루트가 있는데([`crate::worktree::tracker_root`]) 이 뿌리가 거기 없다는 것이
/// 곧 갈라 놓은 트래커다. 맨몸 저장소의 워크트리는 옮길 루트가 없어 전처럼 한 자리를 함께 쓴다.
fn picks_key(root: &Path) -> u64 {
    let shared = match crate::worktree::tracker_root(root) {
        Some(_) => None,
        None => crate::worktree::tracker_place(root),
    };
    match shared {
        Some((common, rel)) => crate::text::fnv1a64_from(
            crate::text::fnv1a64_from(crate::text::fnv1a64(common.as_os_str().as_encoded_bytes()), &[0]),
            rel.as_os_str().as_encoded_bytes(),
        ),
        None => crate::text::fnv1a64(crate::path::real(root).as_os_str().as_encoded_bytes()),
    }
}

/// 이 세션이 `ids` 를 지금 집었다고 적는다. 못 적으면 조용히 넘어간다 — 빠진 기록은 전과 같은 판정이다.
fn record_picks(input: &Input, repo: &Repo, ids: &[(String, bool)]) {
    use std::io::Write;
    if ids.is_empty() {
        return;
    }
    let Some(sid) = safe_sid(input) else { return };
    let Some(dir) = picks_dir(repo) else { return };
    let Some(home) = dir.parent() else { return };
    // **남이 못 쓰는 자리에 짓는다**(리뷰 moai-59k3.4c3) — 설정 디렉터리가 그룹에 열려 있고 umask 가
    // `002` 면 같은 그룹이 기록을 심어 temp 에서 막으려던 것이 그대로 돌아온다. `read_marks` 와 같이
    // **처음 지을 때만** 닫는다 — 사람이 나중에 연 권한을 쓰기마다 되돌리지 않는다.
    let fresh = !home.exists();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    #[cfg(unix)]
    if fresh {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700));
    }
    let _ = fresh;
    // **벽시계로 적는다 — `model::now` 가 아니다.** 그쪽은 `MOAI_NOW` 로 멈춰 초 단위라, 두 세션의
    // 집기가 같은 때로 서서 누가 마지막인지 못 가린다. 이 기록은 트래커가 아니라 이 기계의 표다.
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else { return };
    let now = now.as_nanos();
    // 줄의 칸 시각(`status_since`)과 견줄 때는 같은 시계로 잰다 — `MOAI_NOW` 가 서면 그것이다.
    let stamp = crate::model::parse_rfc3339(&model::now());
    let lines: String = ids.iter().map(|(id, sure)| crate::hook::Picks::line(now, stamp, id, *sure)).collect();
    let mut open = std::fs::OpenOptions::new();
    open.create(true).append(true);
    // **링크를 따라 쓰지 않는다**(리뷰 moai-59k3.4c3) — 심어 둔 링크 하나가 남의 파일에 이 줄을 덧붙인다.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut open, libc::O_NOFOLLOW);
    if let Ok(mut f) = open.open(dir.join(&sid)) {
        let _ = f.write_all(lines.as_bytes());
    }
    prune_picks(home, &dir, &sid);
}

/// **오래 안 적힌 세션의 기록은 치운다**(리뷰 moai-3k2d.1df). 읽는 쪽([`read_picks`])은 판정마다 이 디렉터리를
/// 통째로 읽는데, 세션마다 파일이 하나씩 쌓이고 아무도 안 지우면 그 값이 기계가 떠 있는 동안 는다. 두 주 넘게
/// 한 줄도 안 적은 세션의 기록을 지운다 — 지운 줄은 기록이 없던 때처럼 판정한다. 적을 때만 치운다 — 드물다.
///
/// **다른 트래커의 자리도 치운다**(리뷰 moai-59k3.4c3). temp 에 둘 때는 기계가 치웠지만 설정 곁은 아무도
/// 안 치운다 — 지운 워크트리·옮긴 저장소의 자리는 다시 안 적혀, 제 자리만 치우던 판에서는 영영 남았다.
/// 옆 자리는 오래된 기록만 지우고, 빈 디렉터리는 걷는다(`remove_dir` 는 빈 것만 지운다).
fn prune_picks(home: &Path, dir: &Path, sid: &str) {
    const KEEP: std::time::Duration = std::time::Duration::from_secs(14 * 24 * 60 * 60);
    let stale = |entry: &std::fs::DirEntry| {
        entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP)
    };
    let sweep = |at: &Path, keep: Option<&str>| {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for entry in entries.filter_map(Result::ok) {
            if keep.is_some_and(|k| entry.file_name() == k) {
                continue;
            }
            if stale(&entry) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    };
    sweep(dir, Some(sid));
    let Ok(others) = std::fs::read_dir(home) else { return };
    for entry in others.filter_map(Result::ok) {
        if entry.path() == dir || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        sweep(&entry.path(), None);
        let _ = std::fs::remove_dir(entry.path());
    }
}

/// 적어 둔 집기를 읽어 이 세션의 눈으로 가른다 — 가르는 자는 `hook::Picks::fold` 다. 여기는 읽기만 한다.
/// 기록이 끝났는지는 `issues` 의 칸 시각으로 잰다.
fn read_picks(input: &Input, repo: &Repo, issues: &[model::Issue]) -> crate::hook::Picks {
    let Some(me) = safe_sid(input) else { return Default::default() };
    let Some(Ok(dir)) = picks_dir(repo).map(std::fs::read_dir) else { return Default::default() };
    // **보통 파일만 읽는다** — 파이프가 서 있으면 여는 자리에서 훅이 멈추고, 링크는 남의 파일을
    // 읽힌다(리뷰 moai-3k2d.1df). 설정 곁으로 옮긴 뒤에도 손으로 놓을 수 있는 자리라 그대로 둔다.
    // 적는 쪽은 보통 파일만 만든다.
    let files =
        dir.filter_map(Result::ok).filter(|entry| entry.file_type().is_ok_and(|t| t.is_file())).filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path()).ok()?;
            Some((entry.file_name().to_string_lossy().into_owned(), text))
        });
    let since: std::collections::BTreeMap<&str, i64> =
        issues.iter().filter_map(|i| Some((i.id.as_str(), crate::model::parse_rfc3339(&i.status_since)?))).collect();
    crate::hook::Picks::fold(&me, files, &|id| since.get(id).copied())
}

/// 세션 id 를 경로 조각으로 — 사람이 준 글자를 그대로 쓰지 않는다.
fn safe_sid(input: &Input) -> Option<String> {
    let safe: String = input
        .session_id
        .as_deref()?
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    (!safe.is_empty()).then_some(safe)
}

/// 껍데기 토막 하나를 판정할 트래커.
///
/// **같은 저장소 안은 모두 세션의 눈이다**(moai-y7go). 트래커를 찾는 길이 딸린 워크트리를 루트로
/// 옮기므로([`crate::store::Repo::find_from`]) `-C <워크트리>`·`cd <워크트리>` 는 세션이 이미 보는
/// 그 트래커를 가리킨다 — 판정도 쓰기도 한 파일이다. 워크트리 세션이 `-C <루트>` 로 가리켜도 같다:
/// 안 그러면 `-C <루트>` 한 번으로 규칙 1 을 넘는다. 누구의 초점인가는 여전히 **이름**이 가른다
/// (`away_of` 가 `Repo::here` 로 읽는다), 가리킨 디렉터리가 아니다.
///
/// [`Route::There`] 는 그래서 대개 **다른 저장소**의 트래커다 — 등록한 옆 프로젝트다(moai-23ky).
/// 한때 같은 저장소의 옆 워크트리도 여기로 갔는데, 그때는 그 워크트리의 `.moai` 가 따로 있었다.
/// **같은 저장소 안이어도 트래커가 정말 갈린 자리는 여기로 간다**(moai-acf7) — `MOAI_HERE=1` 을 든
/// 워크트리가 그 자리고, 그 토막이 **제 낱말로 `-C` 를 적었을 때만**이다. `.moai` 가 여럿인
/// 모노레포는 `worktree::same_repo` 가 애초에 안 세 이 커밋 전부터 여기로 왔다 — 그쪽의 집기는
/// [`crate::hook::Segs::picks`] 도 이 자리의 것으로 안 센다.
#[derive(Debug, PartialEq)]
enum Route {
    /// 세션 자리의 트래커 — 같은 저장소 안이면 대개 어디를 가리켜도 여기다(`worktree::same_repo`).
    Here,
    /// 가리킨 자리에 트래커가 없다. `moai` 가 스스로 실패하니 아무도 판정하지 않는다.
    Nowhere,
    /// 다른 트래커 — `route` 가 돌려준 목록의 자리.
    There(usize),
}

/// 토막마다 판정할 트래커를 가른다(`hook::aimed`). 가리킨 곳이 없으면 디스크를 안 짚는다 —
/// 대부분의 호출은 `-C`·`cd` 가 없어 여기서 아무것도 안 읽는다.
fn route(
    repo: &Repo,
    line: &crate::hook::Line<'_>,
    dirs: &[Option<PathBuf>],
) -> (Vec<Route>, Vec<Repo>, Vec<Option<(PathBuf, bool)>>) {
    let mut there: Vec<Repo> = Vec::new();
    // 토막마다 **내미는 줄이 겨눌 자리**([`crate::hook::Toward`]) — 판정할 트래커와 따로 든다. 판정은
    // 이 트래커가 하면서도 겨눌 자리는 딴 곳인 경우가 있다(아직 없는 자리, 아래).
    let mut aims: Vec<Option<(PathBuf, bool)>> = Vec::new();
    // 아직 없는 자리를 가리킨 토막에서만 읽는다 — 대부분의 호출은 여기 안 와 명령줄을 다시 안 가른다.
    let spelled = std::cell::OnceCell::new();
    let routes = dirs
        .iter()
        .enumerate()
        .map(|(k, dir)| {
            let mut aim = None;
            let spells = || spelled.get_or_init(|| crate::hook::spells_dir(line)).get(k).copied().unwrap_or(false);
            let route = route_one(repo, &mut there, &mut aim, dir.clone(), &spells);
            aims.push(aim);
            route
        })
        .collect();
    (routes, there, aims)
}

/// 두 뿌리가 같은 자리인가 — 등록 목록이 쓰는 그 자([`crate::user_config::same_dir`])다. 여기에
/// 따로 두면 훅이 "같은 트래커" 라고 본 줄을 옆 표면이 다른 것으로 세어 조용히 갈린다.
fn same(a: &Path, b: &Path) -> bool {
    crate::user_config::same_dir(a, b)
}

/// [`route`] 의 토막 하나 — 판정할 트래커를 가르고, 내미는 줄이 겨눌 자리를 `aim` 에 적는다.
/// `spells` 는 그 토막이 **제 낱말로 `-C`·`--dir` 를 적었는가**([`crate::hook::spells_dir`])다.
fn route_one(
    repo: &Repo,
    there: &mut Vec<Repo>,
    aim: &mut Option<(PathBuf, bool)>,
    dir: Option<PathBuf>,
    spells: &dyn Fn() -> bool,
) -> Route {
    let Some(dir) = dir else { return Route::Here };
    // **자리가 있는지는 한 번만 묻는다** — 두 번 물으면 그 사이에 옆 세션의 `mkdir` 이 끼어, 한 물음은
    // "없다" 로 다른 물음은 "있다" 로 답한 토막이 아무 데서도 판정되지 않는다.
    let here_now = dir.is_dir();
    // **`cd` 로만 옮긴 없는 자리는 세션의 눈으로 본다** — `cd /없는곳; moai add` 는 `cd` 가 실패해
    // 세션 자리에서 돈다. 적지도 않은 `-C` 를 지어 내밀면 거절문이 있지도 않은 곳을 겨눠, 옮겨 친
    // 줄이 그대로 실패한다(`git worktree add … && cd <새 워크트리> && moai add` 가 그 모양이다).
    if !here_now && !spells() {
        return Route::Here;
    }
    // **아직 없는 자리도 [`Repo::find_from`] 이 그대로 답한다** — 위로 찾는 길(`store::found_root`)은
    // 글자로 올라가며 `.moai` 만 짚어, 끝 자리가 없어도 만들어졌을 때와 같은 트래커를 낸다. 통째로
    // `Nowhere` 로 보내던 판은 `mkdir d && moai -C d add` 를 아무도 판정하지 않았는데, 실행할 때는
    // `d` 가 있어 `moai` 가 위로 찾아 이 트래커에 세운다 — 규칙 1 이 샜다. 반대로 이 트래커로만
    // 보던 판은 `mkdir -p <남의 저장소>/새것 && moai -C <남의 저장소>/새것 add` 를 여기서 판정하고
    // 거절문에는 `-C <남의 저장소>/새것` 을 댔다 — 이 트래커의 에픽 id 를 단 채라, 옮겨 친 줄이 남의
    // 트래커에 끊긴 참조를 세웠다(리뷰 moai-51h9.k8j1).
    // **말은 안 묻는다** — 넘어진 까닭을 사람에게 낼 일이 없는 자리다(`broken` 만 쓴다).
    let (found, broken) = match Repo::find_from(&dir, crate::i18n::Lang::default) {
        Ok(found) => (found, false),
        Err(_) => (None, true),
    };
    let Some(found) = found else {
        // **아직 트래커가 없는 새 자리는 그 자리를 댄다**(moai-j2vp) — 판정은 이 트래커가 맡되,
        // 옮겨 친 줄이 이 트래커에 서면 안 된다. 있는 자리인데 트래커가 없으면 그 `moai` 는 스스로
        // 실패하니 아무도 판정하지 않는다.
        if here_now {
            return Route::Nowhere;
        }
        // **읽다 넘어진 트래커는 "없는" 트래커가 아니다**(리뷰) — 위에 `.moai` 가 서 있는데 그
        // 설정이 깨져 못 읽은 자리다. 없는 것으로 적으면 거절문이 `moai -C <그 밑> init` 을 대,
        // 이미 트래커가 선 저장소 안에 둘째 `.moai` 를 심으라고 시킨다(moai-23ky 가 그은 경계를
        // 훅이 스스로 넘으라고 하는 꼴이다). 모르는 자리는 안 겨눈다.
        if !broken {
            *aim = Some((dir, false));
        }
        return Route::Here;
    };
    // **제 낱말로 `-C` 를 적어 딴 트래커를 가리킨 토막은 그 트래커가 본다**(moai-acf7, 2026-09-19
    // 사용자 결정). 같은 저장소 안이라도 `Repo::find_from` 이 옮겨 주지 않는 자리가 있다 —
    // `MOAI_HERE=1` 로 제 `.moai` 를 든 워크트리와, `.moai` 가 여럿인 모노레포다. 그 자리에서
    // `moai -C <루트> mv <리뷰> done` 은 루트의 트래커에 쓰는데, 이 스냅샷에는 그 리뷰가 없어
    // 규칙 3 이 통째로 샜다. 옮겨 주는 흔한 자리(맨 워크트리 세션)는 위의 `same` 이 먼저 잡으니
    // **값은 갈라진 트래커를 `-C` 로 가리킨 호출에만 붙는다**.
    //
    // **`cd` 로 옮긴 토막은 지금대로 여기다** — 규칙 2 는 `There` 로 보낸 토막을 안 세므로
    // (`guard_moai` 만 돈다), `cd <루트> && sed -i …` 를 넘기면 쓰기 셈이 그 자리에서 꺼진다.
    // `-C` 를 적은 토막은 `moai` 를 부르는 토막이라 셀 쓰기가 없다([`crate::hook::spells_dir`]).
    if same(&found.root, &repo.root)
        || (!crate::worktree::is_linked(&found.root)
            && crate::worktree::same_repo(&found.root, &repo.root)
            && !spells())
    {
        return Route::Here;
    }
    // **갈라 놓은 제 트래커에서 루트를 가리킨 것은 여전히 이 세션이다**(moai-acf7). 스냅샷만 루트의
    // 것으로 바꾸고 **누구인가는 이 체크아웃에서 읽는다**(`Repo::here`) — 루트의 눈으로 이름까지
    // 읽으면 이 워크트리가 쥔 일이 통째로 "옆의 것" 이 되어, 시킨 대로 세우고 집은 리뷰를 규칙 3 이
    // 못 본다. `away_of`·`worktree::fresh` 가 이미 `here()` 로 이름을 읽는 그 자다(moai-y7go).
    //
    // **닿는 것은 `MOAI_HERE=1` 워크트리 하나다** — `tracker_root` 는 git 꼭대기의 트래커만 답해,
    // `.moai` 가 여럿인 모노레포는 여기 안 든다. 그쪽은 `same_repo` 도 못 세 이 줄 전에 이미
    // `There` 로 가고(`tracker_place` 의 뿌리 상대경로가 갈린다) 집기도 여기 것이 아니다 — 이
    // 커밋 전부터 그랬다. 옆 칸의 집기를 이 칸의 것으로 세려면 "같은 저장소인가" 라는 **다른 자**가
    // 있어야 하고, 그것은 moai-23ky 가 정한 트래커 경계를 다시 여는 결정이다.
    //
    // **트래커를 이미 옮겨 온 세션에는 안 묻는다** — 옮겨 왔으면 `repo.root` 가 곧 그 루트라
    // ([`crate::store::Repo::from_found`]) 그 물음의 답이 `repo.root` 고, 여기 오는 길은
    // `found.root != repo.root` 뿐이라 늘 거짓이다. 맨 `Path` 견줌 하나로 거르지 않으면 토막마다
    // `tracker_root` 가 디스크를 훑는다 — 흔한 워크트리 세션이 답 없는 물음의 값만 치른다.
    let own = repo.here() == repo.root.as_path();
    let found = match own.then(|| crate::worktree::tracker_root(repo.here())).flatten() {
        Some(root) if same(&root, &found.root) => found.seen_from(repo.here().to_path_buf()),
        _ => found,
    };
    let n = match there.iter().position(|r| same(&r.root, &found.root)) {
        Some(n) => n,
        None => {
            there.push(found);
            there.len() - 1
        }
    };
    // **겨눌 자리는 `there` 가 이미 든 철자다** — 토막마다 제가 푼 철자를 들던 판은, 링크로 같은
    // 저장소를 두 철자로 가리킨 줄에서 규칙 1(`there` 로 판정한다)과 그때 있던 한국어 알림(`aims` 로
    // 냈다)이 한 훅 판에 서로 다른 `-C` 를 댔다. 거절문이 내미는 줄도 `aims` 를 읽는다.
    *aim = Some((there[n].root.clone(), true));
    Route::There(n)
}

/// 이 세션의 출석(moai-h8tn) — **옛 장을 걷고 이름을 옮기는 것 말고는 적지 않는다**, 상태와 함께 적는 것은 [`attend`]
/// 다. 그 쓰기는 편지를 읽음으로 옮기기 **전에** 선다(moai-jzym.flj) — `Stop` 이 판정 뒤에 다시 적는 것은 상태가
/// 바뀌었을 때뿐이다. `root` 는 트래커의 뿌리다 — 규칙이 물러선 자리가 아니다([`crate::store::tracker_in_use`]).
///
/// 세션 id 로 찾고, 없으면 같은 에이전트 프로세스(pid·선 때)의 장을 이 세션으로 잇는다 — `/clear` 는 세션
/// id 만 바꾸고, `moai hello` 로 지은 이름과 역할은 남아야 한다. 그것도 없으면 새로 짓는다: 이름은 `MOAI_AGENT`,
/// Claude 가 보이는 세션 이름, 못 읽으면 `<벤더>-<세션 id 앞 8자>` 차례다(2026-10-04 사용자 결정). 지은 이름을 남이
/// 쥐었으면 세션 토막을 붙여 가른다([`mail::made_name`]) — 두 세션이 한 이름이면 편지가 먼저 읽는 쪽으로 샌다. 다른 기계의
/// 장은 떠난 것으로 읽혀도 하루 동안은 그 이름을 쥔다(moai-nas5). `MOAI_AGENT` 는 산 남이 쥐었으면 버리고 지은 이름으로
/// 선다 — 창이 대는 이름이라 떠난 장의 것이면 되찾는다(moai-dhxm).
///
/// **`MOAI_AGENT` 가 이 세션의 이름이다**(moai-ew4o.e1m) — `moai send`·`inbox` 가 그 이름으로 돌므로([`super::mail`] 의
/// `who`), 훅이 그것을 모르면 한 세션이 두 이름으로 서서 그 이름 앞의 답장이 영영 안 실렸다. 이어 쓰는 장의 이름이
/// 다르면 그 이름으로 옮기고 편지를 데려간다([`mail::rename_card`]) — 산 남이 그 이름을 쥐었으면 안 옮긴다. **Codex 는
/// 안 읽는다** — 그 훅의 환경은 세션 여럿이 함께 쓰는 데몬의 것이다(moai-sile).
///
/// **떠난 장의 이름을 새 장이 넘겨받으면 그 함부터 비운다**(moai-ew4o.l3n, [`mail::take_over`]) — Claude 의 세션 이름은
/// 디렉터리마다 256개라 떠난 세션 앞으로 남은 편지가 같은 이름을 받은 새 세션에 실렸다. 이어 쓰는 장은 넘겨받는 것이
/// 아니다.
///
/// **세션 id 로 찾은 장도 그 프로세스가 살아 있을 때만 그대로 쓴다**(리뷰 moai-h8tn.x4l) — `claude --resume` 은 세션
/// id 를 그대로 들고 **새 프로세스**로 뜬다. 죽은 pid 를 든 채 다시 적던 판은 `moai agents` 가 산 세션의 장을
/// 걷었고, `moai send`·`inbox` 는 조상의 pid 로 나를 못 찾았으며, 걷힌 뒤에는 새 이름·빈 역할로 다시 서서 감독이
/// 일감을 가졌다. 그 장은 지금 프로세스와 칸으로 다시 잇는다 — 이름·역할은 그대로다. **다른 기계의 장도 그렇다**
/// (moai-dhxm) — 그 장은 죽었는지를 이 기계에서 못 재 닻이 새로운 동안 떠난 것으로 안 읽히지만, 이 훅이 이 기계에서
/// 돈다는 것이 곧 그 세션이 지금 여기 있다는 것이다(컨테이너를 다시 띄우고 `claude --resume` 으로 이었다). 그대로 이어
/// 쓰던 판은 앞 기계의 pid 를 든 장에 닻만 새로 적어 영영 안 낡게 했고, 이 기계의 `moai inbox`·`send` 는 조상으로 나를
/// 못 찾았다([`mail::Presence::runs_as`]). 기계를 안 적은 옛 장은 그 pid 가 이 기계에 살아 있으면 기계를 단다
/// ([`mail::Presence::claimed`]). **한 프로세스는 장 하나다** —
/// 다시 이은 프로세스가 다른 이름의 장도 들고 있으면(`/resume` 으로 세션을 갈아탄 프로세스) 그 장을 걷고 그 함을
/// 비운다. 두 이름으로 서면 `moai inbox` 와 훅이 서로 다른 이름의 편지를 본다.
///
/// **세션 id 가 없으면 아무도 아니다** — 누구의 편지를 실을지 모르고, 실으면 남의 것을 읽음으로 옮긴다.
///
/// 에이전트는 이 훅을 띄운 셸의 부모다(`claude` → `sh` → `moai`, `agy` → `sh -c` → `moai`). 조상에서 이름이 벤더인
/// 것을 찾고, 없으면 셸의 부모를 쓴다 — 그때의 벤더는 훅을 심은 말씨(`--dialect`)의 것이다.
///
/// **Codex 는 세션 id 로만 잇는다**(moai-sile) — 그 훅은 TUI 가 아니라 세션 여럿이 함께 쓰는 `codex app-server` 데몬이
/// 띄우고, 환경도 그 데몬이 떠오른 자리의 것이다(2026-10-04 실측). 조상으로 잇던 판은 Codex 세션 둘이 한 데몬 pid 로
/// 서로의 장을 가져갔고, 데몬의 `TMUX_PANE` 을 적어 `send --wake` 가 남의 창에 글자를 쳤다. 그래서 pid 와 tmux 칸을
/// 모름으로 둔다. 모르는 pid 의 장은 닻으로 산 것을 잰다([`mail::Presence::stale`]) — **세션 id 로 찾은 Codex 장은 닻이
/// 낡았어도 그대로 잇는다**: 그 세션이 돌아온 것이다. `SessionEnd` 는 그 장을 걷는다([`rest`]).
fn attendee(input: &Input, root: &Path, dialect: Dialect) -> Option<mail::Presence> {
    let session = input.session_id.as_deref().filter(|s| !s.trim().is_empty())?;
    let dir = crate::store::agents_at(root);
    let mail_dir = crate::store::mail_at(root);
    let (all, roster) = mail::presences(&dir);
    // **못 연 출석부면 아무도 아니다**(리뷰 moai-kxkw.k2f) — 세션 id 가 없을 때와 같은 자리다. 그때 빈 출석부는 "아무도 없다"
    // 가 아니라 누가 있는지 모른다는 뜻이라, 그것으로 이름을 짓고 넘겨받던 판은 이 세션의 되돌아온 편지를 떠난 이의 것으로
    // 읽음에 치웠고(`take_over`), 역할이 빈 새 장으로 감독 세션이 `any-idle-worker` 일감을 가졌다. 장은 어차피 못 쓴다.
    if mail::roster_fenced(&roster).is_some() {
        return None;
    }
    // 모델은 장에 없을 때만 훅의 입력으로 채운다 — `moai hello --model` 이 적은 것이 먼저다.
    let model = |p: &mail::Presence| if p.model.is_empty() { input.model() } else { p.model.clone() };
    let asked = (dialect != Dialect::Codex)
        .then(|| std::env::var("MOAI_AGENT").ok())
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| mail::is_agent_name(v));
    // 산 남이 쥔 이름인가 — 창이 대는 이름(`MOAI_AGENT`)을 재는 자다(`hello` 가 대는 이름을 재는 자와 같다). 지은 이름은
    // [`mail::made_name`] 이 다른 자로 잰다(moai-nas5). 대소문자만 다른 이름도 같은 장이다. `mine` 은 그 이름을 쥐어도 남이
    // 아닌 장이다.
    let held_by_other = |name: &str, mine: &str| {
        all.iter().any(|p| p.name.eq_ignore_ascii_case(name) && !p.name.eq_ignore_ascii_case(mine) && !p.gone())
    };
    // 이어 쓰는 장을 `MOAI_AGENT` 의 이름으로 옮긴다 — 옮길 수 없으면 그대로다.
    let renamed = |p: mail::Presence| match asked.as_deref() {
        Some(want) if want != p.name && !held_by_other(want, &p.name) => {
            mail::take_over(&mail_dir, &all, want, Some(&p.name));
            let moved = mail::Presence { name: want.to_string(), ..p.clone() };
            match mail::rename_card(&dir, &mail_dir, &moved, &p.name) {
                Ok(()) => moved,
                Err(_) => p,
            }
        }
        _ => p,
    };
    let found = all.iter().find(|p| p.session.as_deref() == Some(session));
    if let Some(p) = found.filter(|p| dialect == Dialect::Codex || (p.here() && !p.gone())) {
        return Some(renamed(mail::Presence { model: model(p), ..p.clone() }.claimed()));
    }
    let cwd = input.cwd.clone().unwrap_or_default();
    let short: String = session.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    // 새 장의 뼈대 — 이름·벤더·프로세스·tmux 칸만 갈린다. 넘겨받는 이름이면 그 함부터 비운다.
    let fresh =
        |name: String, vendor: &str, pid: u32, pid_start: Option<u64>, tmux: (Option<String>, Option<String>)| {
            mail::take_over(&mail_dir, &all, &name, None);
            mail::Presence {
                v: mail::VERSION,
                name,
                vendor: vendor.to_string(),
                model: input.model(),
                role: String::new(),
                status: String::new(),
                since: String::new(),
                pid: 0,
                pid_start: None,
                machine: None,
                host: None,
                session: Some(session.to_string()),
                cwd: cwd.clone(),
                tmux_pane: tmux.0,
                tmux_socket: tmux.1,
                seen: None,
                rest: Default::default(),
            }
            .at(pid, pid_start)
        };
    if dialect == Dialect::Codex {
        return Some(fresh(mail::codex_name(&all, session)?, "codex", 0, None, (None, None)));
    }
    let ancestors = mail::ancestors();
    let (agent, vendor) = match mail::agent_among(&ancestors) {
        Some((p, v)) => (p.clone(), v),
        None => (ancestors.get(1).or(ancestors.first())?.clone(), dialect.as_str()),
    };
    let tmux = mail::Presence::tmux_here();
    if let Some(p) = found.or_else(|| all.iter().find(|p| p.runs_as(&agent))) {
        // **`MOAI_AGENT` 가 이른 장은 걷어도 그 함을 안 비운다**(리뷰 moai-ew4o.q9f) — 그 이름은 세션이 아니라 이 창의 것이라,
        // 이 창이 다른 세션을 이어 써도(`/resume`) 그 이름 앞의 편지는 이 창의 몫이다. 비우던 판은 떠나지도 않은 창의 편지를
        // 보낸 이에게 "읽기 전에 떠났다" 로 되돌렸다. 이어 쓰는 장은 다음 훅의 `renamed` 가 그 이름으로 옮기고 편지를 합친다
        // — 이 판의 출석부(`all`)에는 걷은 장이 아직 산 것으로 서 있어 지금은 못 옮긴다.
        for other in all.iter().filter(|o| o.name != p.name && o.runs_as(&agent)) {
            let windows = asked.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&other.name));
            if mail::forget(&dir, &other.name).is_ok() && !windows {
                mail::retire(&mail_dir, &other.name);
            }
        }
        return Some(renamed(
            mail::Presence {
                model: model(p),
                session: Some(session.to_string()),
                cwd: cwd.clone(),
                tmux_pane: tmux.0,
                tmux_socket: tmux.1,
                ..p.clone()
            }
            .at(agent.pid, agent.start),
        ));
    }
    // `MOAI_AGENT` 는 창이 대는 이름이라 떠난 장의 것이면 그대로 되찾는다(moai-dhxm) — 토막을 붙여 가르는 것은 지은
    // 이름뿐이다. **지은 이름은 다른 기계의 조용한 장이 하루 쥔다**(moai-nas5, [`mail::made_name`]) — Claude 의 세션 이름은
    // 컨테이너마다 따로 세어 겹친다. 떠난 것으로 읽힌(20분) 저쪽 장의 이름을 내주던 판은 그 장을 덮고 아직 산 저쪽 세션의
    // 편지를 보낸 이에게 되돌렸다.
    if let Some(name) = asked.filter(|n| !held_by_other(n, "")) {
        return Some(fresh(name, vendor, agent.pid, agent.start, tmux));
    }
    let name = (vendor == "claude")
        .then(|| mail::claude_session_name(agent.pid))
        .flatten()
        .or_else(|| mail::name_with(vendor, &short))?;
    Some(fresh(mail::made_name(&all, name, session)?, vendor, agent.pid, agent.start, tmux))
}

/// `Stop` 없이 끝난 턴(moai-u5wr.f29) — 이 세션의 장이 있으면 `idle` 로 적는다. **장을 새로 짓지는 않는다** — 끝나는
/// 세션에 이름을 지어 주면 아무도 안 쓸 장이 하나 선다.
///
/// **Codex 의 `SessionEnd` 는 그 장을 걷고 그 함을 비운다**(moai-sile, moai-ew4o.l3n) — Codex 의 장은 pid 를 몰라
/// ([`attendee`]) `moai agents` 가 닻이 낡을 때까지 안 걷고, 끝난 세션 앞의 편지는 보낸 이에게 돌아가야 한다. Claude 의
/// 장은 남긴다: `/clear` 도 `SessionEnd` 를 내는데 같은 프로세스가 곧 새 세션으로 그 장을 잇는다(`moai hello` 로 지은
/// 이름과 역할이 거기 있다).
///
/// **받는 것은 트래커의 뿌리 하나다**(moai-jzym.uxa) — 설정을 안 읽은 자리다([`crate::store::tracker_in_use`]). 드는 것은
/// 출석부와 우편함뿐이라 [`Repo`] 를 받으면 그것을 세우는 값(루트의 `config.toml` 파싱)을 끝나는 세션마다 치르고,
/// 그 파일이 깨진 날은 엉뚱한 출석부를 받는다.
fn rest(input: &Input, root: &Path, dialect: Dialect, event: Event) {
    let Some(session) = input.session_id.as_deref().filter(|s| !s.trim().is_empty()) else { return };
    let dir = crate::store::agents_at(root);
    let (all, _) = mail::presences(&dir);
    let Some(p) = all.into_iter().find(|p| p.session.as_deref() == Some(session)) else { return };
    if dialect == Dialect::Codex && event == Event::SessionEnd {
        if mail::forget(&dir, &p.name).is_ok() {
            mail::retire(&crate::store::mail_at(root), &p.name);
        }
        return;
    }
    // **이미 노는 장은 다시 안 쓴다**(리뷰 moai-u5wr.e74) — 디스크에서 읽은 그대로라 바뀔 것이 없다. Claude 의 흔한 끝
    // (`Stop` 뒤의 `SessionEnd`, `/clear` 마다)이 그 자리고, 저장소가 선 자리(Ceph RBD)가 멈춘 날 그 쓰기 하나가
    // `SessionEnd` 의 짧은 상한을 넘긴다. 닻을 적을 때가 되었으면 쓴다(moai-j3n5) — **세션이 닫힐 때는 빼고**(moai-dhxm).
    // 닻은 "아직 산다" 다: 닫히는 세션에 적으면 한참 놀다 닫힌 세션이 다른 기계에 20분 동안 산 일꾼으로 다시 서고, `/clear`
    // 에서는 곧 이을 `SessionStart` 가 어차피 다시 적는다. 닻이 모든 장의 것이 되며 이 쓰기가 Claude 의 그 끝에 되살아났었다.
    if p.status == mail::IDLE && !p.since.is_empty() && (event == Event::SessionEnd || !p.due(&model::now())) {
        return;
    }
    attend(&dir, Some(p), mail::IDLE);
}

/// 출석을 이 상태로 적는다 — 상태가 바뀔 때만 `since` 를 새로 댄다(얼마나 놀았나를 `send --wake` 가 잰다). 닻도
/// 적는다(moai-j3n5, moai-dhxm) — 훅이 돈 것이 곧 그 세션이 산 것이다. 못 적으면 조용히 지나간다 — 훅은 실패하지
/// 않는다(머리글).
fn attend(agents: &Path, presence: Option<mail::Presence>, status: &str) {
    let Some(mut p) = presence else { return };
    let now = model::now();
    if p.status != status || p.since.is_empty() {
        p.status = status.to_string();
        p.since = now.clone();
    }
    p.stamp(&now);
    let _ = mail::write_presence(agents, &p);
}

/// [`waiting`] 이 고르는 편지 — 이 세션에 온 것 모두, 아니면 남이 보낸 것만.
///
/// **턴을 붙드는 `Stop` 은 제가 보낸 편지로 붙들지 않는다**(리뷰 moai-h8tn.x4l) — 붙듦에 끝이 있는 것은 실은 편지만큼
/// 우편함이 주는 까닭인데(그래서 `stop_hook_active` 여도 싣는다), 제게 쓴 편지에 실린 말("`moai send <보낸 이>` 로
/// 답한다")을 따르면 답이 도로 제게 와 턴이 끝없이 붙들린다. 제가 쓴 편지는 다음 프롬프트가 붙들지 않고 싣는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mine {
    All,
    Others,
}

/// 이 세션 앞에 와 있는 편지 — **아직 안 옮겼다**. 옮기는 것은 [`deliver`] 다. 둘을 가른 것은 그 사이에 출석을 적게
/// 하려는 것이다(moai-jzym.flj) — 옮긴 뒤의 쓰기가 늦으면 옮긴 편지가 아무에게도 안 실린다.
fn waiting(root: &Path, me: &mail::Presence, which: Mine) -> Vec<mail::Stored> {
    // 여는 자리는 제 함과 열린 편지의 함 둘뿐이다(moai-ew4o.c92) — 남에게 간 편지를 열어 가르지 않는다.
    let (all, _) = mail::list(&crate::store::mail_at(root), &me.name, false);
    all.into_iter()
        .filter(|s| mail::for_me(s, &me.name, &me.role))
        .filter(|s| which == Mine::All || s.letter.from != me.name)
        .collect()
}

/// [`waiting`] 이 고른 편지를 읽음으로 옮기며 실을 글을 낸다 — 없으면 `None`. **옮긴 것만 싣는다** — 남이 먼저
/// 가진 `any-idle-worker` 편지는 빠진다. 말은 실을 편지가 있을 때만 푼다(사용자 설정을 여는 값이다).
///
/// `room` 은 이 글이 들 자리다([`crate::hook::letters_room`]·[`hold_room`]) — 그 칸을 넘기면 에이전트가 글을 파일로
/// 빼 읽음으로 옮긴 편지를 아무도 못 본다([`crate::hook::CONTEXT_CAP`]·[`crate::hook::CODEX_HOLD`]).
fn deliver(
    root: &Path,
    me: &mail::Presence,
    ctx: &Ctx,
    room: crate::hook::Room,
    mine: Vec<mail::Stored>,
) -> Option<String> {
    if mine.is_empty() {
        return None;
    }
    let dir = crate::store::mail_at(root);
    let (lang, zone) = (ctx.lang(), ctx.zone());
    let (picked, left) = crate::hook::deliverable(&mine, room, lang, zone);
    // 고른 차례(앞에서부터)대로 옮긴다. 받은 편지를 그대로 넘긴다 — 다시 베끼지 않는다. 한 통도 못 옮겼으면
    // [`crate::hook::letters`] 가 `None` 을 낸다.
    let taken: Vec<mail::Stored> = mine
        .into_iter()
        .enumerate()
        .filter(|(k, _)| picked.contains(k))
        .map(|(_, s)| s)
        .filter(|s| matches!(mail::take(&dir, s, &me.name), Ok(mail::Took::Mine)))
        .collect();
    crate::hook::letters(&me.name, &taken, left, lang, zone, room)
}

/// 이 세션이 열릴 때 적어 둔 경고 수. 없으면 견줄 것이 없다.
fn baseline(input: &Input, repo: &Repo) -> Option<usize> {
    let path = session_file(input, repo, "warn")?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// 세션마다 한 번만. **매 프롬프트에 보드를 붙이면 그것대로 자리값을 잃는다.**
///
/// 표를 남기는 자리는 시스템 임시 디렉터리다. `.moai/` 에 두면 세션 부스러기가
/// 저장소에 쌓이고, 그것을 `.gitignore` 로 막는 일이 또 생긴다.
///
/// **표식은 글이 건너간 뒤에 선다**(moai-45hf.wqg, 2026-09-29 사용자 결정). 심은 셸 줄은 `moai` 의
/// stdout 을 `$o` 로 받아 두었다가 `moai` 가 끝난 **뒤에** 흘려보낸다(`skill::command`). 여기서 표식을
/// 세우면 그 사이에 셸이 죽거나(매니페스트 `timeout` 이 프로세스 그룹째 죽인다) `printf` 가 지면 표식만
/// 남고, 그 세션은 보드를 영영 못 받는다 — 판정은 다음 도구 호출이 다시 셈하지만 보드는 아니다.
/// 그래서 셸이 [`HANDOFF`] 로 쪽지 자리를 주면 표식 이름을 거기에 적어 넘기고, 셸이 `printf` 에
/// 이긴 뒤에 그 이름으로 표식을 세운다.
///
/// **쪽지를 못 쓰면 옛 길로 간다** — 표식을 여기서 바로 세운다. 쪽지 자리를 안 주는 셸은 이 버전 전에
/// 심은 줄이고(다시 심을 때까지 그 복사가 돈다), 그 줄에서 표식을 안 세우면 보드가 매 프롬프트에
/// 실린다. 쪽지 자리를 남이 먼저 잡은 경우도 같다: 셸은 제 것이 아닌 쪽지를 안 읽으므로 여기서
/// 안 세우면 아무도 안 세운다. 쪽지 자리나 표식 이름이 상대 경로인 경우도 같다([`hand_off`]).
fn once_per_session(input: &Input, repo: &Repo, what: &str, make: impl FnOnce() -> Decision) -> Decision {
    let Some(path) = session_file(input, repo, what) else {
        // 누구인지 모르면 한 번을 보장할 수 없다. **그러면 싣지 않는다** —
        // 한 번 빠지는 것이 매 프롬프트 도배보다 싸다.
        return Decision::Pass;
    };
    if path.exists() {
        return Decision::Pass;
    }
    let decision = make();
    if decision != Decision::Pass && !hand_off(&path) {
        let _ = std::fs::write(&path, "");
    }
    decision
}

/// 심은 셸 줄이 쪽지 자리를 넘기는 환경 변수. 셸 줄(`skill::command`)이 이 이름을 그대로 적는다.
pub const HANDOFF: &str = "MOAI_HOOK_HANDOFF";

/// 셸이 준 쪽지 자리. 빈 값은 없는 것이다.
///
/// **절대 경로만 받는다**(리뷰 moai-45hf.nab). 셸은 `$h` 를 제 자리에서 푸는데 `moai` 는 [`run`] 에서
/// stdin 의 `cwd` 로 옮긴 **뒤에** 쪽지를 쓴다. 상대 `TMPDIR` 이고 두 자리가 갈리면(훅 프로세스의 자리는
/// 아무도 약속하지 않았다) 둘이 다른 파일을 가리켜, 셸은 쪽지를 못 찾고 `moai` 는 표식을 안 세워 보드가
/// 매 프롬프트에, `Stop` 의 붙듦이 매 턴에 섰다. 안 받으면 옛 길이라 한 번은 그대로 한 번이다.
fn handoff() -> Option<PathBuf> {
    std::env::var_os(HANDOFF).filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// 표식 이름을 쪽지에 적는다. 적었으면 참이다.
///
/// **새로 만들 때만 적는다**(`create_new`, 곧 `O_CREAT|O_EXCL`) — 그 자리에 이미 무엇이 있으면(남의
/// 파일, 링크) 거기에 안 쓴다. `run` 이 맨 앞에서 제 옛 쪽지를 걷었으니 남은 것은 못 걷은 것이다.
///
/// **줄바꿈이 든 이름은 안 넘긴다** — 셸은 `read -r` 로 첫 줄만 읽으니, 그 이름은 딴 파일을 세운다.
/// **덜 쓴 쪽지는 따로 걷지 않는다**(리뷰 moai-45hf.nab) — 줄바꿈이 맨 끝 바이트라 쓰기가 지면 쪽지는
/// 줄바꿈 없이 끝나고, 셸의 `read` 는 줄바꿈 전에 끝난 줄에 비영을 내 그 이름으로 아무것도 안 세운다.
/// 남은 쪽지는 [`prune_slips`] 가 걷는다.
///
/// **상대 이름도 안 넘긴다**(리뷰 moai-45hf.nab). 표식 자리는 `std::env::temp_dir()` 에서 오는데, 그
/// 값은 `TMPDIR` 을 거르지 않고 돌려줘 빈 값(`TMPDIR=`)이면 빈 경로다. 셸의 `${TMPDIR:-/tmp}` 는 그것을
/// `/tmp` 로 읽어 쪽지 자리만 절대가 되고, 상대 이름은 셸이 제 자리에 세우는 동안 `moai` 는 옮겨 간
/// stdin 의 `cwd` 에서 찾는다. 그래서 [`handoff`] 와 따로 본다.
///
/// **쪽지는 제 것만 읽고 쓰게(`0600`) 만든다**(리뷰 moai-45hf.nab). 셸은 주인(`-O`)만 보고 적힌 글은
/// 믿는다 — 느슨한 umask(`002` 에 같은 그룹)에서는 그 사이에 옆 사람이 이름을 바꿔 적을 수 있고, 그러면
/// 셸이 남이 고른 자리에 빈 파일을 세운다.
fn hand_off(mark: &Path) -> bool {
    use std::io::Write as _;
    let Some(slip) = handoff() else { return false };
    if !mark.is_absolute() {
        return false;
    }
    let Some(name) = mark.to_str().filter(|n| !n.contains('\n')) else { return false };
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    let wrote = open.open(&slip).is_ok_and(|mut file| writeln!(file, "{name}").is_ok());
    prune_slips(&slip);
    wrote
}

/// **다 쓴 쪽지는 다음 쪽지를 쓸 때 걷는다**(리뷰 moai-45hf.nab). 셸은 내장만 써서 읽은 쪽지를 못
/// 지우고, [`run`] 이 맨 앞에서 걷는 것은 제 셸의 `$$` 로 지은 이름 하나뿐이다 — 앞선 호출의 쪽지는
/// 아무도 안 걷어 보드와 `Stop` 을 넘길 때마다 하나씩 영영 쌓였다. 셸은 넘겨받은 쪽지를 매니페스트
/// `timeout`(15초) 안에 읽거나 그 전에 죽으므로, 그보다 한참 오래된 쪽지는 읽을 이가 없다.
///
/// 지우는 것은 같은 자리의 `moai-hook-*.handoff` 보통 파일뿐이다(링크는 안 따라간다). 넘길 때만
/// 치운다 — 세션에 한두 번이라, 임시 디렉터리를 훑는 비용을 도구 호출마다 치르지 않는다.
fn prune_slips(mine: &Path) {
    const KEEP: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    let Some(dir) = mine.parent() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let ours = name.to_str().is_some_and(|n| n.starts_with("moai-hook-") && n.ends_with(".handoff"));
        if !ours || entry.path() == mine {
            continue;
        }
        // `DirEntry::metadata` 는 링크를 안 따라간다 — 링크는 보통 파일로 안 센다.
        let stale = entry
            .metadata()
            .ok()
            .filter(std::fs::Metadata::is_file)
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > KEEP);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// `Stop` 이 견줄 기준선 — 세션이 열릴 때의 경고 수.
///
/// 훅이 여는 세션마다 덮어쓴다. 재개도 새 세션이고, 재개 시점의 경고가
/// 그 세션이 물려받은 빚이다.
fn write_baseline(
    input: &Input,
    repo: &Repo,
    issues: &[crate::model::Issue],
    unreadable: &[report::Unreadable],
    zone: &crate::tz::Zone,
) {
    let Some(path) = session_file(input, repo, "warn") else {
        return;
    };
    let now = model::now();
    let st = report::status(issues, unreadable, &repo.config, &now, zone);
    // `Stop` 과 같은 자 — 알림은 안 센다.
    let n: usize = st.warnings.iter().map(|w| w.count).sum();
    let _ = std::fs::write(path, n.to_string());
}

/// 세션의 표가 놓이는 자리.
///
/// **저장소도 키에 든다.** 한 세션이 저장소 둘을 오가는 것은 드물지 않은데,
/// 세션 id 만으로 키를 잡으면 둘째 저장소는 첫째의 표를 보고 제 보드를
/// 영영 안 싣는다 — 조용히 빠지는 쪽이라 아무도 못 알아챈다.
fn session_file(input: &Input, repo: &Repo, what: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    // 세션 id 가 경로 조각이 되므로 사람이 준 글자를 그대로 쓰지 않는다.
    let safe = safe_sid(input)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo.dir().hash(&mut h);
    let at = h.finish();
    Some(std::env::temp_dir().join(format!("moai-hook-{safe}-{at:x}.{what}")))
}
