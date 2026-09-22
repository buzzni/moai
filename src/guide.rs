//! 에이전트에게 주는 글. **한 출처다** — `init` 이 AGENTS.md 에 쓰는 블록,
//! `skill install` 이 심는 SKILL.md 와 참고 문서, 훅이 내는 거절문, 그리고 쓰기가
//! 막을 때의 거절문(`model::check_text_size`)이 모두 여기서 조각을 가져간다.
//!
//! 마지막 하나는 훅이 아니다 — `add`·`edit`·`mv -m`·`note` 가 모두 지나는 쓰기 검사라,
//! 여기서 리뷰 얘기를 길게 붙이면 제목이 큰 것을 막을 때도 그 글이 함께 나온다.
//!
//! ## 왜 한 출처인가
//!
//! 같은 것을 두 벌로 두면 반드시 갈라지고, 갈라진 둘 중 어느 것이 참인지
//! 아무도 모른다. 실제로 AGENTS 블록에는 `tui`·`edit` 이 있고 스킬에는
//! 없었으며, 스킬에는 규칙 셋이 있고 AGENTS 블록에는 없었다.
//!
//! ## 왜 그대로 복사하지 않는가
//!
//! 모양이 다르다. AGENTS 블록은 언제나 읽히는 산문이라 길어도 되고, SKILL.md
//! 는 언제 부르는가가 frontmatter 에 있고 본문이 한 화면이어야 한다. 그래서
//! **조각을 한 군데 두고 표면마다 필요한 만큼 엮는다.** 조각 안의 문장은
//! 어느 표면에서 읽어도 뜻이 서야 한다 — "위에서 말했듯" 같은 말을 넣지 않는다.
//!
//! 순수 모듈이다. 파일을 쓰는 것은 `cmd/init.rs` 와 `cmd/skill.rs` 가 한다.

/// 규칙 넷의 이름. **스킬이 적은 규칙과 훅이 낸 거절문이 같은 이름을 댄다** —
/// 다르면 막힌 쪽이 무엇을 어겼는지 두 번 읽어야 한다.
pub const RULES: [&str; 4] = [
    "New issues stay inside what you picked up",
    "Pick something up before you change the repository",
    "A review is an issue too",
    "Never kill the person's tmux server",
];

/// 거절문의 머리. 스킬의 규칙 제목과 글자가 같다.
pub fn rule_head(n: usize) -> String {
    format!("Rule {n} — {}.", RULES[n - 1])
}

/// 리뷰 이슈를 세운 뒤의 세 걸음. **훅의 거절문과 스킬이 이것을 그대로 쓴다.**
///
/// **여기 적힌 명령은 그대로 쳐서 지나가야 한다.** `-m` 없는 `done` 을 일러
/// 줘 결과가 없다고 막은 적이 있다 — 규칙이 제가 일러 준 명령을 막는 자리는
/// 규칙이 아니라 덫이다.
// **줄 잇기(`\`)를 쓰지 않는다.** 그것은 개행과 함께 다음 줄의 앞 공백까지
// 먹어, 첫 명령만 왼쪽 끝에 붙는다.
pub const REVIEW_STEPS: &str = concat!(
    "  moai mv <id> in_progress      when the review starts\n",
    "  moai note <id> -b - < <review text>   the reviewer's own words (summarize past 64KB)\n",
    "  moai mv <id> done -m '<what you took in, what you handed on>'",
);

/// 리뷰 원문이 한 번에 적는 상한([`crate::model::MAX_TEXT_BYTES`])을 넘을 때의 길
/// (moai-b8aj, 2026-09-19 사용자 결정). **요약하되 요약이라고 밝힌다.** 원문 노트와 판단 노트를
/// 가르는 까닭이 리뷰어가 한 말과 이쪽이 정한 것을 가르는 데 있으니, 줄인 글은 줄였다고 적혀
/// 있어야 읽는 쪽이 둘을 안 섞는다. 건의 번호와 자리를 두는 것은 판단 노트가 "3번" 으로 가리키기
/// 때문이다. 규칙 3 의 글·참고 문서·`note` 의 거절문이 이 글을 쓴다.
///
/// **울타리와 들여쓰기를 지키라는 말이 함께 선다**(리뷰 moai-4u6b.5hl). 리뷰 원문은 일한 AI 줄의
/// 꼴을 그대로 옮겨 적고, [`crate::model::work_of`] 는 울타리와 들여쓰기로 그것을 예로 읽는다 —
/// 줄이면서 울타리를 풀면 아무도 안 한 일이 토큰째 통계에 선다. 저널은 덧붙이기만 하니 되돌릴
/// 길도 없다.
///
/// **첫 줄이 원문을 가리킨다**(2026-09-19 사용자 결정, 리뷰 moai-4u6b.5hl 13번). 크기만 적던 판은
/// 줄인 글만 남기고 원문으로 돌아갈 길을 안 남겼다 — 대화록 파일 이름은 끝났다는 알림의 `task-id`
/// 에만 실려 온다. 긴 집 경로는 기계마다 달라 안 싣고 `task-id` 만 싣는다.
///
/// **두 줄이다.** 싣는 쪽이 제 들여쓰기를 붙인다 — 한 줄로 두면 AGENTS 블록과 참고 문서에
/// 옆 줄의 두 배가 넘는 줄이 서고, 낱말 하나를 고쳐도 그 줄이 통째로 diff 에 뜬다.
pub const REVIEW_OVER_LIMIT: &str = concat!(
    "If the text runs past 64KB, summarize it — put `Summary: original <size>KB agent-<task-id>` on\n",
    "the first line, keep every finding's number and place, and shorten only the sentences. Leave fences and indentation alone",
);

/// 리뷰 이슈에 붙는 태그. 훅은 리뷰 줄을 이 글자로 가르고, 가르치는 글은 같은
/// 글자로 세우게 한다 — 둘이 다르면 시킨 대로 세운 리뷰를 규칙이 못 알아본다.
pub const REVIEW_TAG: &str = "review";

/// 리뷰 이슈를 세우는 줄. `anchor` 는 `--parent <id>` 나 `-e <에픽>` 이다 — 규칙
/// 셋의 글과 두 거절문이 이 한 줄에서 나온다.
pub fn make_review(anchor: &str) -> String {
    format!(
        "moai add 'review — <what you are looking at>' -t {REVIEW_TAG} {anchor} -b '<what you are looking for and why>'"
    )
}

/// 리뷰를 닫는 두 걸음 — `REVIEW_STEPS` 에서 시작 걸음을 빼고 **실제 id** 를 넣은
/// 것. 닫기 거절문과 세션을 닫을 때의 붙듦이 쓴다. 손으로 다시 적던 두 자리는
/// 이미 서로 다른 글을 내고 있었다.
///
/// **`moai` 는 겨눌 트래커를 단 머리로 받는다**(moai-j2vp) — 남의 트래커를 보는 판에서 규칙 1 은
/// `moai -C /other` 를 대는데 이 줄만 맨 `moai` 를 내던 판은, 옮겨 친 두 줄이 이 트래커를 겨눠
/// 거기 없는 리뷰를 닫으라고 했다. 이 자리의 트래커면 맨 `moai` 다.
pub fn close_steps(id: &str, moai: &str) -> String {
    // 머리는 줄마다 같다 — `map` 안에서 짓던 판은 줄 수만큼 다시 지었다. 줄의 **첫** `moai ` 만
    // 바꾼다: `REVIEW_STEPS` 의 줄은 들여쓰기 뒤 곧바로 그 낱말로 시작한다
    // (`the_skill_names_each_rule_as_the_hook_does` 가 그것을 못박는다).
    let head = format!("{moai} ");
    REVIEW_STEPS
        .lines()
        .skip(1)
        .map(|l| l.replace("<id>", id).replacen("moai ", &head, 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 난이도 한 낱말 — **모델과 리뷰 등급을 함께 정하는 그 축**이다. 낱말·모델·잣대 셋.
/// 에픽 밖 이슈 하나는 그 낱말이 곧 리뷰 등급이고, 에픽은 멤버를 따로 안 보니 이 잣대로
/// 멤버를 재어 에픽 끝의 `xhigh`·`max` 를 가른다(`EPIC_MAX`, moai-bx6t).
///
/// 감독이 읽는 표와 일꾼이 받는 글이 같은 잣대를 두 벌 적으면 한쪽만 고쳐도 아무도
/// 안 붉어진다 — 실제로 두 벌이 서 있었고 이미 낱말이 갈라져 있었다. 두 표면 모두
/// 여기서 글을 받고, 모델 사다리(`haiku → …`)와 에픽 끝의 모델도 여기서 읽는다.
///
/// **잣대의 글은 이 저장소 CLAUDE.md 의 리뷰 표와 같다** — 같은 축이라고 적어 두고 high 에서
/// `동시성` 이 빠져 있었다. 한 파일 안의 동시성 고침이 medium 으로 읽혀 싼 모델과 싼 리뷰를
/// 받는다. `the_rubric_is_the_review_table` 이 둘을 견준다.
const DIFFICULTY: [(&str, &str, &str); 3] = [
    ("low", "haiku", "text, comments, a one-line fix; behaviour unchanged"),
    ("medium", "sonnet", "a behaviour change inside one file, ringed by tests"),
    ("high", "opus", "several files, the write path, concurrency, the storage format, hooks; hard to undo"),
];

/// 에픽 끝 리뷰를 `max` 로 올리는 멤버. **멤버를 따로 안 보니**(moai-bx6t) 이 한 줄이 그
/// 멤버가 받는 유일한 비싼 눈이다. 일꾼 브리프 7 과 이 저장소 CLAUDE.md 의 리뷰 표가 같은
/// 글을 쓴다 — 손으로 두 벌 적은 첫 판은 둘 다 `high` 잣대의 `동시성` 을 빠뜨렸다(위
/// `DIFFICULTY` 가 한 번 겪은 그 샘이다). `the_rubric_is_the_review_table` 이 둘을 견준다.
const EPIC_MAX: &str = "any member touched the write path, concurrency, the storage format or hooks";

/// 감독이 읽는 표 — 머리까지 여기서 낸다. 머리는 표면에, 칸의 차례는 여기에 두던 판은
/// 칸을 바꿔 끼워도 머리가 엉뚱한 칸을 이름 짓는 채로 아무도 안 붉어졌다.
///
/// **리뷰 칸은 없다.** 감독이 보내는 일은 늘 에픽으로 펼쳐지고 멤버는 따로 안 본다 —
/// 난이도마다 같은 이름의 리뷰 등급을 적어 두면 아무도 안 부르는 `low` 리뷰를 가르친다.
fn difficulty_table() -> String {
    let rows = DIFFICULTY.iter().map(|(level, model, how)| format!("| `{level}` | {how} | `{model}` |"));
    ["| Level | How it is measured | Model |".to_string(), "|---|---|---|".to_string()]
        .into_iter()
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 일꾼이 받는 글의 잣대. 모델 칸은 뺀다 — 일꾼의 모델은 머리의 `모델:` 줄이 준다.
/// 들여쓰기는 부르는 쪽이 `indent` 로 한다.
fn difficulty_rubric() -> String {
    DIFFICULTY.iter().map(|(level, _, how)| format!("`{level}` — {how}")).collect::<Vec<_>>().join("\n")
}

/// 난이도 낱말들 — `` `low`·`medium`·`high` ``.
fn difficulty_levels() -> String {
    DIFFICULTY.iter().map(|(level, _, _)| format!("`{level}`")).collect::<Vec<_>>().join("·")
}

/// 제일 비싼 모델. 에픽 끝 리뷰가 `high` 부터 이것으로 본다.
fn top_model() -> &'static str {
    DIFFICULTY[DIFFICULTY.len() - 1].1
}

/// 에픽 끝 리뷰의 등급 — **가장 무거운 멤버의 난이도에서 한 칸 위**(moai-bx6t, 2026-09-18
/// 사용자 결정). 첫 판은 모든 에픽을 `xhigh` 이상 opus 로 봐, 글 한 줄 고친 멤버 하나짜리
/// 에픽까지 가장 비싼 리뷰를 받았다 — 그날 에픽 6개 중 4개가 멤버 하나짜리라 멤버 리뷰를
/// 걷은 까닭(값)을 거꾸로 거슬렀다. 모델은 그 등급을 따른다. `medium` 이면 가운데 모델이다.
fn epic_review_rule() -> String {
    let mid = DIFFICULTY[1].1;
    let top = top_model();
    format!(
        "**The grade is one step above the heaviest member's difficulty** — `medium` if the members\n\
         are all `low`, `high` if one is `medium`, `xhigh` if one is `high`.\n\
         **If {EPIC_MAX}**, it is `max`.\n\
         Raise it one more step if the epic crosses surfaces or carries several design decisions. If you hesitate, raise it.\n\
         The model follows that grade — `medium` means `{mid}`, `high` and up means `{top}`."
    )
}

/// 일꾼이 받는 글의 머리에 서는 `모델:` 줄. 새 일(`brief`)과 거둔 일(감독 0)이 **같은 줄**을
/// 받는다 — 손으로 두 벌 적던 판은 거둔 쪽이 일꾼에게 없는 감독의 절(2-1)을 가리켰다.
///
/// **`/model` 은 사람만 친다.** 에이전트는 붙박이 명령을 못 부르고, 설정 파일의 모델은 새
/// 세션에만 든다 — 그래서 바꾸기는 창을 보는 사람에게 청한다(12 의 `/clear` 와 같은 길).
/// 제 손으로 치라고 하면 일꾼은 올렸다고 믿고 9-1 에 안 돈 모델을 적는다. 결정은 "맞추거나
/// 올린다" 였다 — 제안과 다른 모델로 뜬 창은 먼저 맞춘다.
///
/// **올릴 때는 다시 잰 난이도의 짝으로 간다**(2026-09-18 사용자 결정). "한 단계 올린다" 만
/// 주던 판은 `low` 로 받아 읽어 보니 `high` 인 일이 가운데 모델에 멈춰, 쓰기 경로를 싼 모델이
/// 했다 — 난이도 한 낱말이 모델을 정한다는 축과 어긋난다.
///
/// 자리 이름은 `<난이도>` 다. `<등급>` 는 7 에서 개발해 본 일꾼이 고르는 리뷰 등급의
/// 자리라, 감독이 채우는 목록에 같은 이름을 넣으면 읽기 전의 제안이 그 명령에 박힌다.
fn model_line() -> String {
    let ladder = DIFFICULTY.iter().map(|(_, model, _)| *model).collect::<Vec<_>>().join(" → ");
    let top = top_model();
    format!(
        r#"Model: <model> (<difficulty> — <why>) — a suggestion picked by difficulty. The model is
   changed with `/model`, and only a person types that — if this window is not on that model,
   ask the person watching the window to match it, and if it reads harder than it looked,
   raise it the same way to the model that pairs with the difficulty you just measured — not
   one step at a time ({ladder}). Handed `low` but it is `high`, the model is `{top}`.
   The grade of the epic-end review (7) is measured on this same rubric, member by member"#
    )
}

/// 일꾼 글 머리의 옆 일 줄(moai-alsi). 새 일(`brief`)과 거둔 일(감독 0)이 **같은 줄**을 받는다 —
/// 4-3 이 "머리의 `옆에서 도는 일`" 을 가리키니, 거둔 쪽 머리에 없으면 이어받은 일꾼은 가리키는
/// 줄이 없는 규칙을 받는다.
const BESIDE: &str = "Work running alongside: <other work> — do not touch those files (4-3)";

/// 루트 HEAD 를 대조하는 글(moai-gokz, 2026-09-18 사용자 결정). 새 일과 거둔 일이 같은 글을
/// 받는다 — 거둔 일은 4-1 부터만 이어 받아, 머리에 없으면 8 의 병합 말고는 트래커 커밋이 대조
/// 없이 엉뚱한 HEAD 에 선다.
const BRANCH_CHECK: &str = r#"Before you commit or merge in the root, **only check** that the root still stands on that
branch — if `git -C <root> symbolic-ref -q HEAD` is not `refs/heads/<base branch>` (detached, or
someone switched the branch), do not run it: tell the supervisor and stop. A merge that lands on
the wrong HEAD leaves no reference at all once `branch -d` runs"#;

/// 밖의 idea 를 도는 마일스톤 안으로 들이는 한 줄(moai-6qgz). 감독의 1 과 일꾼의 1 이 **같은 줄**을
/// 받는다 — 감독은 "들일 것인가" 를 정하고 일꾼이 실제로 단다. 한쪽만 고치면 감독이 들이기로 한
/// 일이 밖에 선 채로 돌거나, 일꾼이 무엇을 달지 모른다. `promote` 에는 이 플래그가 없다.
const MILESTONE_ATTACH: &str = "moai edit <epic> --milestone <milestone>";

/// 모노레포 하위로 드는 한 줄(moai-ay3b). 새 일의 3 과 거둔 일의 워크트리 걸음이 같은 줄을 쓴다 —
/// 거둔 일은 3 을 안 받아(4-1 부터), 여기 없으면 이어받은 일꾼만 워크트리 꼭대기에 선다.
const SUBDIR: &str = r#"cd "$(git -C <root> rev-parse --show-prefix)""#;

/// **`moai read` 는 여기에 안 든다**(사용자 결정 2026-09-18, `moai-ha5d`). 이 저장소의 에이전트는
/// 사람과 같은 git 신원·HOME 으로 돌아 사람의 `[read]` 표를 같이 쓴다 — 브리프가 `moai read --all`
/// 을 가르치면 에이전트가 방금 바꾼 줄에서 **사람의 `[NEW]` 가 내려간다.** 읽음은 사람마다 다른
/// 값이라는 것이 `cmd/read.rs` 의 전제인데, 신원이 하나면 그 전제가 선 채로 깨진다. 가르치려면
/// 에이전트용 신원(`MOAI_CONFIG` 를 가르는 길)이 먼저다.
const CHEATSHEET: &str = r#"    moai status                            board · warnings · flow (start a session here)
    moai prime                             what you hold and what is next, nothing else
    moai ready                             what you can pick up right now
    moai show <id>                         body, children, history. Why it was decided is here
    moai show -g <keyword>                 find out whether it is written down already
    moai show -s todo -t bug               filters (comma = or, repeated flag = and)
    moai show --tree                       epic → issue → child
    moai ready --worktree                  overlay what the other worktrees picked up
    moai tui                               walk the explorer. SPC n parks a thought
                                           on a group row: l one step · Tab expand all · h fold
    moai add '<title>' -p 1 -t bug -e <epic>   create
    moai mv <id> in_progress               pick up  →  review  →  done
    moai edit <id> --tag parser            change
    moai note <id> '<what you found>'      a memo for whoever comes next
    moai defer <id> -m '<why>'             take work out of the plan for now

Every command takes `--json`. `ready --json` gives `{"ready":[…],"held":[…]}` —
`held` is what is deferred or blocked behind an empty group, and where to pick it up
again. That is enough to build a loop that runs without a person — one such loop, in
bash and jq alone, is the moai repository's `examples/bash-agent/agent.sh`.

**A key that cannot be absent is never absent.** `kind` and `priority` hold a default,
and the file leaves a default out, but `--json` fills it back in — `jq -r .priority`
on a row gives `2`, never `null`. Keys that genuinely can be absent (`epic`,
`milestone`, `deferred_at`) stay absent, and that absence is the answer.

**When several sessions share one repository, pick up with
`moai mv <id> in_progress --from todo`.** It moves only while the column you saw
still holds, so it never overwrites work someone else picked up first — the loser
gets one line on stderr (`stale` under `--json`) and a non-zero exit code, and
moves on. `defer` takes the same `--from`. Without it nothing is blocked, as before.
**Pass one id only** — several at once mix winners and losers into a single exit
code, so you pick a row up and then throw it away.

**A `moai` run inside a linked git worktree reads and writes the main checkout's
tracker.** Editing the worktree's `.moai` makes that file conflict on the merge,
and then the only way out is outside the tool — one line on stderr says where it
wrote. So that worktree's `.moai/issues.jsonl` stays as it was when the worktree
split off, and the current rows are in the main checkout's file."#;

const NO_GATE: &str = "There is no approval gate — create anything, move anything. Do not ask a human.";

const FORKS: &str = r#"**1. `add` or `idea`** — what decides is *whether you would pick it up now.*
If you would, `moai add`; if it is for later, `moai idea add '<what came to mind>'`.
An idea stands on neither the board nor `ready`, so it does not blur the plan.
**Walking past it without writing it down is the worst of all.**

If it came out of an epic, ask one more question first — *can this epic deliver
what it promised without this?* If not, it is not for later: it is this work,
unfinished. Even when you cannot do it now (waiting on a person's decision, the
work beside you holds that file), create it as a member with `-e <epic>` and
leave it in the first column — a member still standing keeps the epic from
closing by itself. Send it out as an idea and the epic stands `done` without
having delivered what it promised. To `defer` such a member is to decide to give
that promise up.

**2. `defer` or `done`** — never move to `done` what you decided not to do.
`moai defer <id> -m '<why>'` changes neither the column nor the kind, and
`--undo` brings the same row back as it was. An idea is "not work yet"; a defer
is "work, but not now".

**3. Is it worth splitting into an epic** — if the request does not end inside one
file, show the person **once**, before writing code, a plan split into one epic
plus three to seven issues, and ask. On a "yes", create it in one go with
`moai add --from -` (`--dry-run` shows it first).

```sh
moai add --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
- [p2] second issue
PLAN
```"#;

/// 에이전트가 이슈에 적는 글의 모양(moai-j8aq). **권고다** — 어겨도 아무것도 막히지 않는다.
/// 훅이 이것을 검사하지 않는 것은 결정이다(사용자, moai-mthy): 글 스타일 검사는 린트이고,
/// 린트는 곧 게이트다.
const WRITING: &str = r#"**Pass the title and the body separately.** The title is an argument; the body is
markdown streamed in from stdin with `-b -` — a heredoc is easiest. Do not repeat
the title inside the body.

- **Keep the title short.** One line on what went wrong. The board, `ready` and the explorer list show the title alone
- **Do not write the body as a narrative.** Instead of laying out what happened in paragraphs, split it into a list —
  what went wrong, what you saw, where it gets fixed
- **Do not use emoji** — not in the title, not in the body. Their width differs per terminal, so the board and the tables come out crooked

The one thing that matters is that the next session reads this with
`moai show <id>`. These three are advice for that reason, not rules anything checks."#;

/// 한국어 글을 다듬는 두 플러그인 — `(설치 id, 마켓플레이스 저장소)`. **`moai skill install` 이
/// moai 와 같은 scope 로 함께 깔고**(moai-lr1s), 훅은 이 id 로 깔렸는지 본다(moai-6rrb). 안내 글과
/// 설치와 훅이 다른 이름을 대면 시킨 대로 깔아도 알림이 끝내 안 꺼진다. 설치 id 의 `@` 앞이
/// 플러그인 이름이고, 스킬은 `<플러그인>:<스킬>` 로 불린다 — 안내 글이 스킬을 그 이름으로 대는 까닭이다.
///
/// **사용자 전역에 깔지 않는다**(사용자, moai-5wk4 둘째 판). 첫 판은 "없으면 사용자 전역에
/// 설치한다" 고 가르쳐, 남의 기계의 에이전트가 묻지 않고 버전 고정 없는 바깥 코드를 전역에
/// 들이고 `~/.claude/settings.json` 을 고칠 수 있었다(리뷰 moai-5wk4.ydh 8번). 까는 것은 사람이
/// 부르는 `moai skill install` 하나다.
pub const KOREAN_PLUGINS: [(&str, &str); 2] =
    [("korean-skills@korean-skills", "DaleSeo/korean-skills"), ("humanize-korean@im-not-ai", "epoko77-ai/im-not-ai")];

/// 한국어 글을 넣기 전에 다듬는다(사용자, moai-5wk4). **권고다** — `WRITING` 과 같은 까닭으로
/// 막지 않는다. 훅이 알림을 덧붙이는 것(moai-6rrb)도 알림일 뿐이다.
///
/// **늘 읽히는 자리에는 이만큼만 둔다**(사용자, 리뷰 13번). AGENTS 블록과 SKILL.md 는 한국어를 안
/// 쓰는 저장소에서도 모든 세션이 읽는다 — 자세한 절차는 참고 문서의 `KOREAN_DETAIL` 로 내린다.
///
/// 판정이 **글의 글자**인 것은 결정이다: 화면 말(`MOAI_LANG`)은 사람이 읽는 말이지 에이전트가
/// 적는 말이 아니다 — 영어 화면에서 한국어 이슈를 적는 사람도 있다.
///
/// **맞춤법이 마지막이다**(사용자, 리뷰 11번). 윤문이 제일 크게 고치니, 그 뒤를 맞춤법이 다시 본다.
/// `humanize-korean` 을 긴 글에만 쓰는 까닭은 값이다 — 한 번에 서브에이전트를 1~3번 넘게 부른다.
///
/// **용어 보존이 보존 목록과 따로 서는 까닭**(moai-gw5e): 위의 보존 목록은 *글자 그대로 옮겨
/// 적을 것*(id·명령·경로·숫자)이라 기술 명사가 거기 안 든다. 그 빈자리로 `layer` 가 "층" 이 되고
/// `latest::Seen::Unasked` 가 "못 물었다" 가 된 글이 이 저장소에 쌓였다 — 일반명사는 뜻이 여러
/// 개라 원어를 지우면 읽는 쪽이 코드로 못 돌아간다. 윤문 플러그인은 이미 같은 것을 말하지만
/// (`humanize-korean` 의 `ai-tell-taxonomy.md`), 그 스킬은 글이 다 쓰인 뒤에 돌고 하는 일이
/// AI 티 제거라 용어를 되살리지 않는다. 그러니 쓰기 전에 읽히는 이 자리에 둔다.
const KOREAN: &str = r#"**Polish Korean text before it goes into moai** — any title, body, note or `-m`
that carries even one Hangul character, review text included. Text written only in
English goes in as it is.

- Run `korean-skills:humanizer` to take the AI tell out, add `humanize-korean:humanize-korean`
  when it runs past 20 lines, and finish with `korean-skills:grammar-checker` for spelling and spacing
- Leave ids, commands, paths, numbers, code fragments and the fixed-form lines
  (`model: …`, `Next: …`, `Regression-of: …`, `Summary: original …`) exactly as they are
- **Keep the technical term, and never drop the original.** Do not swap `layer`,
  `network` or `wrapper` for an everyday word and delete the English behind it — an
  everyday word carries several meanings, so nobody can read the sentence back to the
  code. A name that came from the code (`Seen::Unasked`) goes in exactly as it is;
  gloss it in parentheses if the sentence needs it
- `moai skill install` installs both plugins together. The detail is under "Korean text"
  in the moai skill's `references/commands.md`"#;

/// 한국어 글의 자세한 절차 — 참고 문서에만 둔다. 부를 때만 읽힌다.
///
/// **리뷰 원문도 다듬는다**(사용자 결정). 규칙 3 의 `리뷰가 낸 글을 그대로` 와 어긋나 보이니
/// 그 말이 무엇을 막는지를 여기 적는다 — 줄이거나 제 판단을 섞는 것이지 문장을 다듬는 것이 아니다.
///
/// **긴 글은 저장소 밖에서 다듬는다**(사용자, 리뷰 1·12·14번). `humanize-korean` 은 cwd 에
/// `_workspace/` 를 만들고 글의 사본을 남긴다 — 저장소 안에서 돌리면 `.gitignore` 를 고치는 편집이
/// 저장소마다 하나 늘고, 사본은 끝없이 쌓인다.
///
/// **다 쓰면 저장소로 돌아온다**(리뷰 moai-5wk4.76z). 훅은 세션의 자리로 트래커를 찾는다 — 밖에 선 채로
/// 남으면 그 세션의 모든 규칙과 알림이 말없이 꺼진다(`cmd::hook::decide`).
const KOREAN_DETAIL: &str = r#"The always-visible rule is under "Korean text" in `SKILL.md`. This is the procedure.

- For review text, polish the sentences only — do not shorten it and do not mix your own call in.
  That is what rule 3 means by taking the reviewer's own words. Shorten it only past 64KB, and say so with `Summary:`
- Move text longer than 20 lines outside the repository (a scratchpad or a temporary directory),
  make that the cwd, and call `humanize-korean:humanize-korean` there. The skill creates `_workspace/` in the cwd — delete it when you are done
- Come back into the repository afterwards — the hook finds the tracker from where the session stands, and standing outside it no rule stands at all
- On a term's first mention in a body, put the original in parentheses after the Korean
  (`레이어(layer)`) and use the Korean alone from there. A repository that wants a fixed
  list of its own terms keeps that list in its own docs — this rule stands without one
- If polishing changed the meaning, go back to the original text. Polishing fixes sentences, not facts
- If a plugin is missing, do not install it yourself — ask the person to run `moai skill install` again.
  That installs both of the plugins below in the same scope as moai. Without them moai blocks nothing"#;

/// 한국어 글의 참고 절 — 절차에 두 플러그인의 이름을 붙인다. 이름은 `KOREAN_PLUGINS` 한 곳에서 온다.
fn korean_detail() -> String {
    let plugins = KOREAN_PLUGINS
        .iter()
        .map(|(id, repo)| format!("    {id:<32}https://github.com/{repo}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{KOREAN_DETAIL}\n\n{plugins}")
}

/// 글 스타일의 예시(moai-1xf2). **참고 문서에만 둔다** — AGENTS 블록과 SKILL.md 는 언제나
/// 읽히는 자리라 예시 한 벌이 모든 세션의 값이 된다. 규칙은 짧게 늘 보이고, 예시는 부를 때 온다.
///
/// **예시는 moai 의 동작을 주장하지 않는다.** 이 글은 모든 저장소에 심긴다 — 읽는 쪽의 코드를
/// 두고 쓴 예시라야 어디서 읽어도 뜻이 서고, 이 저장소의 파일 이름을 박으면 남의 저장소에서는
/// 아무것도 안 가리킨다. 첫 판은 `moai edit --tag` 가 빈 태그를 그대로 쓴다고 적었는데
/// (`src/cmd/edit.rs` 와 `query::split_tags` 는 처음부터 빈 낱말을 걸렀다) 없는 버그를
/// 예시로 내민 셈이었다. 자리는 `<파일>:<줄>` 처럼 자리 표시로 둔다.
const WRITING_EXAMPLE: &str = r#"The rule is under "What to write in an issue" in `SKILL.md`. This is one set that keeps it.
The title goes as an argument, the body through `-b -`.

```sh
moai add 'an empty tag slips past the filter' -t bug -e <epic> -b - <<'BODY'
- What: normalizing tags leaves an empty word in place
- What I saw: a list filtered by that tag filters nothing and returns everything
- Where: where tags are normalized (<file>:<line>). Dropping the empty word ends it
BODY
```

Three common ways it goes wrong.

- **The title carries the whole story** — "yesterday while I was … it … so I think … needs checking".
  The board and `ready` cut that line off, and what went wrong is usually not in the part that survives
- **The body is written in paragraphs** — the next session has to dig "where it gets fixed" back out of them.
  Split the call, the evidence and the next step into lines and one `moai show` is enough
- **Emoji mark what is urgent** — urgency goes in the priority (`-p 1`). That is the one `ready` reads"#;

const IDEAS: &str = r#"    moai idea add '<what just came to mind>'       park it
    moai idea add '<a longer thought>' -b -        the body comes from stdin
    moai idea ls                                   see what has piled up
    moai show -g <keyword>                         find out whether it is written down already

When the time comes, unfold one into an epic and issues. Unfolding closes the thought.

```sh
moai idea promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
PLAN
```

**A line in the plan becomes the issue title verbatim.** Copy over an idea title
that grew long while you parked it and that length spreads into the issues, so
write a short new title when you unfold — the original text stays on that idea,
and the history line about being unfolded from it leads back there."#;

/// 머지 드라이버는 **클론마다 한 번** 심는다(moai-x129). 심는 법이 `moai merge-driver --help`
/// 에만 있어, 그 명령을 아는 사람만 심을 수 있었다 — 안 심은 클론은 이슈 줄이 이웃이라는
/// 이유로 부딪치고, 그 충돌을 손으로 푼다.
///
/// **심는 자리의 경고도 함께 옮긴다.** 안 심는 것은 무르지만 잘못 심는 것은 무르지 않다 —
/// 적히는 경로가 사라지면 git 이 표식 없는 파일을 남기고, 그것을 `git add` 하는 순간 저쪽이
/// 통째로 사라진다. 치는 법만 옮기고 그 줄을 `--help` 에 두고 오면, 이 글을 읽고 치는 쪽은
/// 그 덫을 못 본 채 친다.
const MERGE_DRIVER: &str = r#"In `.moai/issues.jsonl` one line is one issue and the file is sorted by id, so two
branches that changed different issues collide for no reason other than their lines
being neighbours. Install the merge driver and git resolves that per issue, 3-way.

    moai merge-driver --install

**Run it once per clone.** git reads the driver command from config only, and
config is not committed. In a clone without it, `merge=moai` in `.gitattributes`
is simply ignored and git's default merge runs — merging is exactly as it was
before. `moai status` still says one line about it not being installed: a reminder
not to quietly miss the one thing each clone needs, and it blocks nothing.

**If the path it recorded disappears, merging falls back to git's default.** What
gets recorded is the absolute path of the binary that was running. When that path
is empty git treats it as a conflict but leaves this side's file as it was, and
without markers in it whoever runs `git add` throws the other side away whole —
which is why the install writes neither an answer nor markers and re-merges the
failed run with `git merge-file`. The markers stand, and the worst case is the
same as a clone with nothing installed. Keeping the path alive is still better:
the fallback loses the per-issue resolution. The place it records (`--local`) is
shared by the clone, so running it from a linked worktree's `target/` puts every
checkout in that state the moment the worktree is removed. Pass `--as` with a
path that will not disappear. What is merged and how is in
`moai merge-driver --help`."#;

const DEFERRING: &str = r#"    moai defer <id> -m '<next quarter>'    take it out of the plan for a while
    moai defer <id> --undo                 take it back
    moai show --deferred                   see only what is deferred

Neither the column nor the kind changes — the same row comes back as it was.
What is deferred drops out of `moai ready`, the board and the warnings, and
`moai status` shines one line on it. Defer an epic, a milestone or a parent and
the work under it drops out with it."#;

const GROUPS: &str = r#"    moai epic add '<storage layer>'                an epic
    moai milestone add 'v0.1'                      a milestone
    moai add '<title>' -e <epic> --milestone <milestone>
    moai show <epic|milestone id>                  what stands under it
    moai show --milestone <id>                     everything attached to that milestone

**Belonging is inherited.** A child inherits its parent's epic, an issue inherits
its epic's milestone. A child created with `--parent <epic>` belongs to that epic.
Do not write it again on every issue — move the epic and the members come along.

**A group's column is read from its members.** Do not `moai mv` an epic or a
milestone — pick up one member and it stands `in_progress`, finish them all and it
stands `done`, by itself. To give up on a group while members are left, `moai defer`
those members — and if no member ever finished, deferring them leaves it in the
first column, so defer the group itself to take it out of the plan.

**While a milestone is running, what is inside it comes first.** Whether it is
running is read the same way as above — if even one member stands in a started
column, it is running. There is no command that opens it and no new field.

- `moai ready` gives out the work inside it and leaves only `p0` outside. What is
  running, and how many it held back, is one line under the list; on the board it is a `moai status` notice
- **`p0` gets picked up whether or not it is in the milestone** — that is the hotfix
  slot. In the ordering `p0` comes first and the milestone second
- **Nothing is blocked.** A `moai mv` that picks up work from outside goes straight
  through. What is already picked up is simply finished — the same ground as never taking work back late
- If two milestones are running, both are inside, and the ordering within them is as it always was (`p` · age)
- **A setup with only two columns has no running milestone** — there is no column
  that says "started but not finished", so the rule itself does not stand. In that
  setup `ready` and the board are as they were"#;

/// 여러 프로젝트. **`main` 에 있는 것만 적는다** — 탐색기의 프로젝트 층이나
/// 프로젝트 색처럼 아직 서지 않은 것을 적으면, 시킨 대로 친 명령이 없는 것을 찾는다.
const PROJECTS: &str = r#"    moai project add <dir>                 register it in my config (accepted even without `.moai`)
    moai project ls                        what is registered and how it stands

Called outside a `.moai`, `moai`, `moai status` and `moai ready` give an overview
of every registered project, one block each. Add `--worktree` and each project
overlays its sibling worktrees too. Other commands cannot tell which project you
mean, so call them as `moai -C <dir> <command>`. In `moai tui`, `0` puts every
registered project into one list — a header row per project with that project's
rows under it. Outside a `.moai` it opens there; inside one it opens within that
project. The top header numbers each project, and pressing that number jumps
straight to it — `0` is the single list.
On a header row, `l`/`→` expands it (that is when the project is read) and `h`/`←`
folds it; `Enter` goes inside. Parking (`SPC n`) and marking read (`r`) go to the
project of the row under the cursor, and search and filters apply within a project only.
From there, `SPC p a` picks a directory to register (a monorepo subdirectory
counts separately) and `SPC p d` takes one off the list. With nothing registered,
opening outside still shows an empty list that points at `SPC p a`."#;

/// 화면의 말(moai-acy5). AGENTS.md 에만 적혀 있어 스킬만 읽는 세션은 `MOAI_LANG` 을 몰랐다 —
/// 조각으로 빼 참고 문서도 같이 읽는다.
const LANGUAGE: &str = r#"English is the default. For another language, pass it as in `MOAI_LANG=ko moai status`,
or write `lang = "ko"` under `[i18n]` in your user config (the environment variable
wins over the config). The languages are en, ko, zh, ja and es, and text a language
does not carry yet comes out in English — the two that are full right now are en and ko.
**The system locale (`LANG`, `LC_ALL`) is not read**: it changes only through one of
those two ways. How to add a translation is in the moai repository's `i18n/README.md`."#;

/// 새 판을 묻는 일과 그것을 끄는 두 길(moai-5gka). **손잡이가 코드에만 있으면 끄는 법을 아무도
/// 모른다** — 에이전트가 도는 기계에서 그것을 끄는 사람이 이 글을 읽는 사람이다. `MOAI_API_URL`
/// 은 안 적는다: 거울과 시험이 쓰는 자리지 사람이 고를 손잡이가 아니다.
const UPDATES: &str = r#"The explorer asks GitHub once a day whether a newer release is out, and the version
line in its header says which of four it is — a new release, the latest, ahead of the
latest (a build from source), or not asked. **Nothing is blocked**: it is one line, and
the exit code never changes.

It only asks where a person is watching. `--json`, a pipe and anything that is not a
terminal never ask, so a machine running agents does not knock on the outside every run.
The answer, and when it was asked, are held next to your user config in `latest.toml`.

    [update]
    check = false        # in your user config: never ask on this machine

    MOAI_NO_UPDATE_CHECK=1 moai tui     # or just for this run"#;

/// 커밋과 이슈를 잇는 고리(moai-wqm7). **새 저장소는 이 저장소의 CLAUDE.md 규약을 모른다** —
/// 여기 안 적으면 커밋 칸(`show <id>`·탐색기 상세)이 늘 빈다.
///
/// **예시 제목의 id 는 `<id>` 로 둔다.** 이 글은 모든 저장소에 심긴다 — 이 저장소의 이슈 id 를
/// 박으면 남의 저장소에서는 아무것도 안 가리키고, 접두사가 다른 저장소에 `moai-` 를 가르친다.
const COMMITS: &str = r#"Put the id of the issue a commit touched in the commit subject — `feat: draw the blocked line (<id>)`.
moai does not store commits on the issue. `moai show <id>` and the explorer detail
find that issue's commits on the spot, **by the id written in the subject**. Do not
copy hashes into notes — one squash or rebase makes them stale, and the commit that
closes an issue does not know its own hash in advance.

- Put it **in the subject**. In the body, only lines that open with a trailer word count — `Refs:`, `Closes:`, `Fixes:`.
  An id written anywhere else in the body does not (a tracker commit lists a whole run of
  ids, and they must not all stand as that issue's commits)
- **A repository that squashes adds the trailer.** A squash moves the subjects of the
  commits it folded into the body, so by subject alone those commits vanish whole —
  one `Refs: <id>` line keeps them
- Ids match whole-word. A child's commit (`<id>.x1y`) is not the parent's — a commit
  that takes review findings in belongs to the review by the review issue's id, and the
  `merge: … (<id>)` that folds a worktree in belongs to the work
- A commit that carries nothing but a pick-up or a close starts with `chore(tracker):`.
  The detail leaves those out of the drawing (they stay in `--json`'s `commits`, flagged `tracker`)
- `commits` in `moai show <id> --json` is **always there.** An empty array means "no
  commit names that id", and `commits_error` stands only when git could not be read —
  it is there so a machine can tell "nobody has touched it yet" from "it could not be
  asked here". That value is an object with `kind` and `said`. What you branch on is
  `kind` (`no_git`, `not_a_repo`, `stream`, `encoding`, `failed`); `said` is one line
  for a person to read, so do not match on its words"#;

/// 일한 AI 한 줄(moai-8f2g). **꼴은 `model::parse_work` 가 읽는 그것이다** — 이 글과 9-1 의
/// 노트 줄이 다른 꼴을 가르치면 감독 아래의 일이 모두 회사·토큰이 빈 줄을 적어 통계가 빈다.
/// 저장하지 않는다: 적는 것은 `note` 이고 `work` 는 그 글을 읽은 값이다.
const WORK: &str = r#"Before you close it, leave one line on the issue naming the AI that actually did the
work. It is a note, not a field.

    moai note <id> 'model: <vendor>/<model> tokens=<count> (<grade> — <why>)'

- The vendor is `anthropic`, `openai` or `google`; the model is its real name (`opus-5`, `sonnet-5`); the grade uses the same words as the review grades
- **If you do not know the token count, drop `tokens=`.** Do not write 0 and do not estimate — blank, 0 and a false number are three different things
- **One line per id.** Write the same line on several ids and the tokens multiply by the number of ids
- **Quote free text with single quotes** — inside double quotes the shell expands
  backticks and `$(…)` as commands, so the text is cut off and the call ends in 0.
  If the text itself contains a single quote, stream it from stdin with `-b -`
- `work` in `moai show <id> --json` reads those lines out. It is **always an array**,
  and a line that does not fit the form simply does not become a value — it stays a
  note. Only lines that start at the beginning of a line count; indented lines and
  lines inside a fence are read as examples
- A list, `moai show [filters] --json`, gives the same `work` on every row — when you
  are adding several issues up, call the list once instead of calling per id"#;

const PEOPLE: &str = r#"**The assignee comes for free** — whoever created it is the assignee. To hand it to
someone else, `-a "Name (email)"`; to leave it unowned, `-a none`. The name and
email come from `git config`, and when they are not there you pass them with
`--user "Name (email)"` or `MOAI_ACTOR`."#;

const CLOSING: &str = r#"Run `moai status` once more and see whether the warnings grew. Warnings block
nothing — they shine a light on issues with no epic, reviews stalled for a long
time, and how much you have open at once. Ideas piling up and what is deferred are
not warnings; they stand apart as notices (`notices`).

If you end the session still holding something, leave one line on that issue for
the next session to take over from. The next session reads it in the history under
`moai show <id>`.

    moai note <id> 'Next: <what comes next>'"#;

/// 집은 채 닫을 때 남기는 한 줄. `CLOSING` 과 세션을 닫을 때의 붙듦이 같은
/// 글을 내야 한다 — 안내가 가르친 줄과 훅이 내민 줄이 다르면 둘 다 안 믿는다.
///
/// **`status` 가 이것을 비추지 않는다.** 마지막 note 는 저널을 훑어야 나오는
/// 값이라, 비추는 순간 저널이 `status` 에 읽힌다. `show` 를 한 번 더 치는 것이
/// 실제로 불편해지면 그때 스냅샷 필드를 논의한다 (moai-0rui).
pub fn handoff(id: &str) -> String {
    format!("moai note {id} 'Next: <what comes next>'")
}

/// 갈림길 1 의 둘째 물음 — 규칙 1 의 글과 그 거절문(`hook::create_in`)이 함께 쓴다. 손으로 옮겨
/// 적던 두 벌은 한쪽만 고쳐도 안 붉어졌다(moai-nxw8). 앞의 임자(`에픽이`·`<id> 가`)는 부르는 쪽이 붙인다.
pub const PLEDGE: &str = "cannot deliver what it promised without this";

/// 규칙 넷. 제목은 `RULES`, 리뷰 걸음은 `REVIEW_STEPS` 에서 온다.
fn rules() -> String {
    let [one, two, three, four] = RULES;
    let steps = indent(REVIEW_STEPS, "  ");
    let make = make_review("--parent <the issue>");
    format!(
        r#"**1. {one}.** The issue in focus is the one you picked up — it has left the
first column and is not closed yet (`in_progress`·`review`).
Anything that comes out of that work belongs in the same epic (`-e <epic>`) or
under that issue (`--parent <id>`). If the epic {PLEDGE}, it is one of those two
even when you cannot do it now (fork 1). If it is not for now, park it with
`moai idea add` — an idea is always free of this rule, and so is
`moai add --from` (what it creates is an epic and its children, one unit on its own).

**2. {two}.** `moai mv <id> in_progress`.
What counts is work inside the repository — `.moai/`, `.claude/`, `target/` and
anything outside the repository (scratchpad, temporary files) do not. Shell
writes (`>`, `>>`, `sed -i`, `tee`) count as much as `Edit` and `Write`. If it
was not in the plan, create it with `moai add 'a title'` and pick that up.

**3. {three}.** Before you call `/code-review`, create a review issue tied to
what you are reviewing.

    {make}
{steps}

The angle (`-b`) and the closing line (`-m`) are **actually required.** Calling
without them is refused, and the refusal hands you the command to fix it.
**Do not ask a human** — running that command as given goes through.

**Keep the reviewer's words and your own call in two notes** — what the reviewer
said and what you decided are different texts. Write what you handed on **with
the issue id**. A line that only says "handed on" is never read again. Where the
review text lives is in the skill's `references/commands.md`.
{REVIEW_OVER_LIMIT}.

**4. {four}.** `tmux kill-server` and `kill-session` without `-L`/`-S`, and
`pkill`/`killall` aimed at tmux, are refused. When the session runs inside tmux
`$TMUX` is set, so a bare `tmux` ignores `TMUX_TMPDIR` and attaches to that
server — one line kills every session in it. A tmux you are testing gets its
own server.

    {TMUX_OWN}"#
    )
}

/// 시험용 tmux 를 띄우는 줄 — 규칙 4 의 글과 거절문이 함께 쓴다.
pub const TMUX_OWN: &str = "env -u TMUX tmux -L <unique name> …";

/// `init` 이 AGENTS.md 의 마커 사이에 쓰는 블록. **언제나 읽히는 산문이다.**
///
/// 한때 여기에 "정적이라 `bd prime` 같은 명령을 따로 두지 않는다 — `moai status` 가
/// prime 이다" 가 적혀 있었다. `moai prime` 이 서면서 그 말이 틀렸는데(moai-5ok8), **이
/// 블록이 곧 에이전트가 읽는 글이라** 고치지 않으면 도구가 제 명령을 없다고 가르친다.
/// 나뉜 자리는 이렇다 — 보드(`status`)는 사람이 한 화면으로 훑는 것이고, `prime` 은
/// 세션 첫머리와 접힌 뒤에 **다시 주입되는** 짧은 한 판이다. 둘 다 [`CHEATSHEET`] 에 선다.
pub fn agents() -> String {
    format!(
        r#"## Issue tracker — moai

This repository's work lives in `.moai/issues.jsonl`.
Do not use TodoWrite or a markdown TODO list. {NO_GATE}

Start a session by running `moai status`. The board and the warnings come up on one screen.

{CHEATSHEET}

{PEOPLE}

### The three forks

{FORKS}

### What to write in an issue

{WRITING}

### Korean text

{KOREAN}

### Park what is out of scope

{IDEAS}

### When work already created is not for now

{DEFERRING}

### There are two kinds of group

{GROUPS}

### Several projects

{PROJECTS}

### The language on screen

{LANGUAGE}

### Checking for a new release

{UPDATES}

### When a feature request comes in

1. Look at the epics that already exist with `moai status`. If it may overlap, `moai show -g <keyword>`.
2. Show a plan split the way fork 3 says, once, and on a "yes" create it in one go.
3. Pick it up with `moai mv <id> in_progress`, and move it to `done` when it is finished.
4. Park what you find along the way that is out of scope with `moai idea add` — if the
   epic promised it, it is not an idea even when you cannot do it now (fork 1).
5. Why it was decided goes on the issue with `moai note <id>`. The next session reads
   it with `moai show <id>`.

### Name the id in the commit

{COMMITS}

When the tool grows and this block goes stale, call `moai init` again. It touches
neither the issues nor the journal; it rewrites this block only. To see only
whether it is stale, `moai init --check` — it writes nothing and answers
`current`, `stale` or `missing`.

### When the issue file has to be merged

{MERGE_DRIVER}

### Name the AI that did the work

{WORK}

### The four things the hook actually watches

They stand once `moai skill install` has planted the hooks into Claude.

{rules}

### The supervisor

`moai skill install` also plants a second skill, `moai-supervise`. Call it to hand
the ideas that have piled up, one at a time, to the sessions idling on the same
repository, and to take their reports — the supervisor picks, sends and checks;
it does not fix and it does not merge.

### Before you close the session

{CLOSING}
"#,
        rules = rules()
    )
}

/// 스킬의 SKILL.md. **본문은 한 화면이다** — 전체 명령은 `reference` 로 내린다.
///
/// 발동어는 frontmatter 의 `description` 이다. 항상 켜져 있는 비용이 이 한
/// 줄이라, 여기에 낱말을 더하는 것은 모든 세션에 값을 매기는 일이다.
///
/// **이 줄은 산문이 아니라 짝맞추개다**(moai-54k2 리뷰). 심는 글을 영어로 통일하면서 한국어
/// 발동어를 걷었더니, 한국어로 묻는 저장소에서 "뭐부터 할까" 가 짝맞출 글자를 잃었다 — 그러면
/// 이 스킬이 안 서고 세션은 이 줄의 마지막 문장이 막는 TodoWrite 로 돌아간다. 읽는 글은 영어로
/// 두고 **발동어는 두 말을 함께 싣는다**: 여기서 아끼는 것은 토큰이 아니라 발동이다.
/// `the_skill_description_keeps_its_korean_triggers` 가 그것을 못박는다.
pub fn skill() -> String {
    format!(
        r#"---
name: moai
description: Use for this repository's work, issues and plans. "what should I do first", "sort out the to-dos", "create an issue", "how is it going", "let us do this later", "뭐부터 할까", "할 일 정리", "이슈 만들어", "진행 상황", "이거 나중에 하자", when a feature request has to be split into several parts, or when something out of scope comes to mind mid-task. Use this instead of TodoWrite or a markdown TODO list.
---

# moai — this repository's issue tracker

The work lives in `.moai/issues.jsonl`. {NO_GATE}

{CHEATSHEET}

Whoever creates an issue is its assignee, for free.

## The three forks

{FORKS}

## What to write in an issue

{WRITING}

## Korean text

{KOREAN}

## The four things the hook actually watches

{rules}

## Before you close the session

{CLOSING}

Every command and the `--from` syntax are in `references/commands.md`.
"#,
        rules = rules()
    )
}

/// 스킬의 참고 문서. 부를 때만 읽힌다.
pub fn reference() -> String {
    format!(
        r#"# Every command

`moai --help` and `moai <command> --help` are the truth. This file is a summary of
them, so where they differ the help wins.

## There are two kinds of group

{GROUPS}

## Filters

A comma means "or"; the same flag twice means "and".

    moai show -s todo -t bug          todo and bug
    moai show -s todo,review          todo or review
    moai show -e none                 the ones with no epic
    moai show --deferred              only what is deferred
    moai show --stale 7               stuck in the current column for more than seven days
    moai show --tree                  epic → issue → child

## Seeing the worktrees together

When agents each take a git worktree, this worktree's board knows nothing of what
was picked up and moved beside it. Add `--worktree` to `status`, `ready` or `show`
and the sibling worktrees' issues are overlaid. The explorer (`moai tui`) opens with
them overlaid and `SPC v w` turns it off and on.

    moai ready --worktree             work picked up beside you drops out and stands under "held"
    moai status --worktree            the board header says "⎇ <worktrees> overlaid"
    moai show --worktree --json       only rows from beside you carry a "branch" key

For the same id, the row that changed its place in the plan last wins — the later
column move (`status_since`) or defer / undefer (`planned_at`); on a tie the later
`updated_at`; on a tie again the row on the current branch. So a field-only edit does
not undo a pick-up made beside you, and a later defer made beside you is not hidden
behind a column you picked up here first.
A row `rm`-ed here does not come back as the sibling's row — if it existed at the
point they split (`git merge-base`), it is read as deleted here. A row picked up or
changed beside you after that does stand.
A memo (`moai note`) does not change the snapshot, so it does not count as a change —
a row that only got a memo beside you stays hidden, and that memo surfaces from the
journal when the branches are merged.
A row that is not on the current branch carries `⎇ <branch>` before its title.
**They are overlaid for showing only** — no file changes.

**Writes go to the root's tracker.** A `moai` called inside a linked worktree reads
and writes the main checkout's `.moai` — editing the worktree's snapshot makes that
file conflict on the merge, and then the only way out is outside the tool. One line
says where it wrote. To edit that worktree's tracker on purpose, pass `MOAI_HERE=1` —
that is the place that builds a state where the sibling snapshots have diverged.

## Contested pick-up — `--from <column>` on `mv` and `defer`

When several sessions share one `.moai`, a session beside you picks up the same row
between your `ready` and your `mv`. `--from <column>` moves it **only while the
column you saw still holds** — it is re-read inside the lock, so a row that changed
in between is not touched. Without it nothing is blocked, as before.

The shape of the call is shown end to end in the moai repository's `examples/bash-agent/agent.sh`.
Cut short it is this — `col` is the column of that row as `ready` gave it, and `moved`
says whether you got it.

```sh
row=$(moai ready --json | jq -c '.ready[0]')
id=$(jq -r '.id' <<<"$row")
col=$(jq -r '.status' <<<"$row")
claim=$(moai mv "$id" in_progress --json --from "$col") || {{
  [ -n "$claim" ] || exit 1          # empty stdout is a failure, not a lost contest
  continue                           # lost — on to the next
}}
[ "$(jq '.moved | length' <<<"$claim")" -gt 0 ] || continue
```

- **Pass one id at a time.** Several at once mix the rows you won and the rows you
  lost into **one exit code** — the won rows have already moved while the caller
  believes it picked up nothing
- A lost row stands as one line on stderr (`moai: <id> already stands <column> — not moved`)
  and as `stale: [{{"id":…,"status":<the column it stands in>}}]` under `--json`. The
  `already <column>` on stdout is **a different thing** — that row was already in the column
  you asked for, and the exit code is 0. Both lines come out of the language bundle, so they
  read in whatever language the screen is set to — tell the two apart by the exit code and
  by `--json`, never by the words
- **Not every non-zero code means "lost".** No identity, the lock being held, a
  mistyped column and a broken row all come back with the same code. Losing a contest
  puts the row on stdout; a failure puts `{{"code":…}}` on stderr and leaves stdout
  empty — read a failure as "someone else took it" and the same row comes round again
  and stops with the wrong reason
- **`--from` matching is not the same as picking up.** In a setup where the first
  column is also the working column, the column matches, nothing moves, and it exits 0
  with `already` — look at `moved` as well
- **`--from` cannot be used on a group (epic, milestone)** — it is refused
  (`bad_status`). A group's column is read from its members while the write goes to the
  column on the row itself (`deferred_at` for a defer), so the axis you measure and the
  axis you write diverge and **both** contestants win. A guard that pretends to bite is
  worse than no guard — pick up a member, or give up on the group with `moai defer` and
  no `--from`
- `defer`'s `--from` also looks at the **column**. A defer does not change the column,
  so it filters out a row whose column moved because it was picked up beside you, but a
  contest where **both sides defer** is not decided by it
- `--from` takes **a column it knows, or a column some row actually stands in**. A name
  nobody stands in is refused (`bad_status`) — read a typo as "it did not match" and it
  does nothing while returning a code, and the caller cannot read why. What the refusal
  is aimed at is the typo, not staleness, so after you rename a column in `config` a row
  still standing in the old name can be picked up by that old name

## Creating in one go

A `#` line is an epic; a `-` line is an issue of the epic just above it. `[pN]` and
`#tag` are optional. If a title starts with `[` or ends with `#word`, write it as
`\[` / `\#` (`- \[WIP] issue \#12`).

```sh
moai add --from - <<'PLAN'
# Storage layer
- [p1] write atomically #enhancement
- recover a truncated line #bug
PLAN
```

`--dry-run` keeps a heredoc typo from creating six of the wrong things.

Keep a plan you repeat in a file and fill `{{{{name}}}}` with `--var name=value` (the
name takes letters, digits, `_` and `-`, no spaces). By convention it lives in the
repository at `.moai/templates/<name>.md` and is called with `--from <path>`.
Every variable is required — a name you did not fill, an empty value, a value with a
newline in it, a name not in the plan, and the same name twice are all refused and
create nothing. The value becomes the title text exactly as written, so put variables
in title positions only (in a tag or priority position it is refused). To put a literal
`{{{{` in a template title, write `\{{{{`. A backslash immediately before `{{{{` is counted
in pairs — for a literal backslash followed by a variable, write `\\{{{{name}}}}`
(`C:\\{{{{dir}}}}`). `idea promote --from` takes the same `--var`.

    moai add --from .moai/templates/release.md --var version=1.2 --dry-run

## Several projects

{PROJECTS}

## The language on screen

{LANGUAGE}

## Checking for a new release

{UPDATES}

## Unfolding a parked thought

{IDEAS}

## Deferring

{DEFERRING}

## People

{PEOPLE}

## What to write in an issue — an example

{WRITING_EXAMPLE}

## Korean text

{korean}

## Name the id in the commit

{COMMITS}

## When the issue file has to be merged

{MERGE_DRIVER}

## Name the AI that did the work

{WORK}

## How to find what a review said

The full review text stays in a file. The notice that it finished carries a
`task-id`, and that is the file name.

    ~/.claude/projects/<project>/<session>/subagents/agent-<task-id>.jsonl

The full text is the **`message` of the last `SubagentHandback` call**. Only when
there is no such call is it the **last `text` block**.

```sh
python3 -c "
import json,sys
b=[c for l in open(sys.argv[1])
   for c in json.loads(l).get('message',{{}}).get('content') or []
   if isinstance(c, dict)]
h=[(c.get('input') or {{}}).get('message') or '' for c in b
   if c.get('type') == 'tool_use' and c.get('name') == 'SubagentHandback']
t=[c['text'] for c in b if c.get('type') == 'text']
print(h[-1] if h else t[-1] if t else '')" <that file> | moai note <review id> -b -
```

**Where the report was handed back, the last `text` block is not the review text.**
A subagent that handed its report over through that call writes one more closing line
after it, something like "report sent" — take the last `text` only and a 200-byte
closing line stands in as the review text, and the size check measures that instead.
It goes wrong quietly, so the one-liner above looks at the handed-back report first.

**Do not simply take the last line.** One turn's blocks are written line by line with
thinking and tool calls mixed in, so the last line is sometimes not text at all. Then
an empty text is passed on and `moai note` stops, saying the memo is empty. It stops loudly with a
non-zero exit code, so nothing is lost — but do not match on the words: that line is screen text
and comes out in whatever language the screen speaks. Getting it right the first time is better.

**Do not keep the summary and throw the original away.** A summary is your own call;
the original is what the reviewer said. A call can be made again; a discarded original
cannot be recovered.

**Shorten it only when it overflows.** One write takes up to 64KB, so `moai note`
refuses text larger than that.
{REVIEW_OVER_LIMIT}.
Saying it is a summary keeps the next person from reading it as the reviewer's words,
and the `agent-<task-id>` on the first line is the file name above, so the way back to
the original stays open. **Do not split it across several notes** — the journal only
appends, so a split stays forever, and the way the person chose is the summary (moai-b8aj).
Fences and indentation are left alone because a `model:` line copied over, standing at
the beginning of a line, puts work nobody did into `work`.

## The hook

    moai hook <event>    Claude's hooks call this. A person never runs it by hand

The plugin planted by `moai skill install` is what calls it. Whatever goes wrong the
exit code is 0 — a noisy hook gets turned off, and a rule that is off is no rule.

**It may be called from inside a git hook.** git hooks and a linked worktree's
`rebase -x` export the `GIT_DIR` family, and those beat `git -C <path>`, but moai
strips those variables before it starts git. So which **repository** is read is decided
by `-C` or by the search for `.moai` — a `moai -C B` called from repository A's hook
reads only B's history for the commit column and for `--worktree`.

**The person comes from B too.** A value a hook inherits because the caller ran
`git -c user.name=…` is stripped as well — carry that through a `-C` write into
someone else's project and their name stays in B's journal forever. So a name that
exists only in A's repository config is no longer read: with no name in B and none
globally, the write cannot find a person and stops, and stopping inside a hook fails
that commit. In that spot, pin it with `--user "Name (email)"` or `MOAI_ACTOR`.

**It is the same whichever directory you call it from.** Both the person and the
commit column are read from that tracker's `.moai` root — even with another repository
nested inside the project (a submodule, `vendor`), a `moai` called in there does not
write that repository's name.
"#,
        korean = korean_detail()
    )
}

/// 둘째 스킬 `moai-supervise` 의 SKILL.md. 같은 저장소에서 놀고 있는 세션에
/// idea 를 하나씩 나눠 주고 보고를 받는 감독의 걸음이다 (moai-hxma).
///
/// **idea 를 일감으로 바꾸는 길은 `promote` 하나다** (사용자 결정). `moai edit`
/// 에 `--type` 이 없어 제자리에서 못 바꾸는데, 첫 실행의 일꾼은 `add` 로 새 줄을
/// 세우고 idea 를 손으로 닫았다. 길이 둘이면 일꾼마다 다르게 고르고, `add` 는
/// 집은 것이 있는 세션에서 규칙 1 에 걸린다. `promote` 는 idea 를 저절로 닫고
/// 출처를 저널에 남긴다.
///
/// **`~/.claude/sessions/*.json` 은 Claude Code 의 속 파일이다.** 문서에 없고
/// 판마다 바뀔 수 있어, 글이 그렇다고 밝히고 못 읽을 때의 길을 함께 준다.
///
/// **모든 저장소에 심긴다.** 이 저장소의 이슈 id 를 글에 적지 않고, 가지 이름을 박지
/// 않는다 — 감독이 루트 체크아웃의 지금 가지를 읽어 `<본 가지>` 에 채운다(사용자 결정,
/// moai-7ljm). 일꾼이 뜨고 병합하는 곳이 그 체크아웃이라 원격 기본 가지보다 덜 어긋난다.
/// 루트가 detached 면 감독이 보내지 않고, 바퀴 사이에 루트의 가지가 바뀌면 일꾼이 루트 커밋·
/// 병합 직전의 대조(`BRANCH_CHECK`)로 멈춘다(moai-gokz).
/// 워크트리 안의 규칙 3 이 루트에서 집은 리뷰를 못 보는 것은 moai-iz38(에픽 moai-wofj)이지만,
/// 남의 저장소에서 그 id 는 아무것도 안 가리키고 고친 뒤에는 거짓이 된다.
pub fn supervise() -> String {
    let brief = brief();
    let table = difficulty_table();
    let epic_rule = epic_review_rule();
    let reclaim_model = indent(&model_line(), "      ");
    let reclaim_check = indent(BRANCH_CHECK, "      ");
    format!(
        r#"---
name: moai-supervise
description: Use when handing the ideas piled up on one repository, one at a time, to the Claude sessions idling on it and taking their reports. Triggers on "supervise", "hand out the ideas", "put the idle sessions to work", "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜".
---

# moai-supervise — hand ideas out to the sessions that are idling

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.claude/worktrees/`, and `moai init` writes that path into the gitignore.

**Read the base branch once, at the start of the round.** The place a worker branches
its worktree from and merges back into is the root checkout, so that checkout's current
branch is the base branch — the remote's default branch may differ from the root and may
be stale. The root checkout is the first entry of `git worktree list`, so the line below
gives the root's branch no matter where in the repository you call it, inside a worktree
included. If nothing comes out, report the error git gave and stop.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); if [ -n "$b" ]; then echo "$b"; else echo "the root is detached" >&2; fi; fi
```

**If the root is detached, do not send.** The worker's pick-up commit and its merge
land on a HEAD with no branch, the check (`merge-base <base branch>`) and `worktree add`
fail, and `branch -d` deletes that work's only reference. Do not read the remote's
default branch instead — the root does not stand on that branch, so it is the same
accident. Ask the person to put the root on a branch, and stop.

Fill the name you read into `<base branch>` in the commands below and in the text you
send the worker. **The worker does not read it again** — read inside a worktree, it
gives that worktree's own branch.

## One round

**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new ideas.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the four words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if the session list in 2 shows a session living
in the root it may be that one, so ask that session what it is holding before handing
the work on.

The `Place` line (`place` under `--json`) has four values. **Only `none` is handed on.**

    <path> (<branch>)  at        it runs there. Go in and carry on
    not showing yet    fresh     just picked up — the gap while the worker raises its worktree. Leave it
    unknown            unknown   **a sibling worktree could not be read.** It may be there, so do not hand it on
    none               lost      it lost its place — only this one is reclaimed

**`stranded` being quiet does not mean there is nothing to reclaim.** If a sibling
snapshot that names no picked-up row cannot be read, the place verdict folds into
`unknown` for everything and this warning is locked for the whole repository. When the
`unreadable_worktrees` key stands under `status --json`, **fix that worktree and look
again** — an empty list read before that is not "none", it is "not counted".

**The `Trouble in sibling worktrees <n>` on the person's screen is a different number.**
That one counts **every** worktree it could not read among the snapshots it opened, and
counts the other problems met while overlaying too (a snapshot with unparseable rows,
a worktree list that could not be read) — a worktree that could not be read but whose
name points at a picked-up row does not hide the verdict, because that row already
stands in its own place, so while only such rows stand you can trust `stranded` as it
is. It says there is something to fix, not that it could not count — whether it could
count is answered by `unreadable_worktrees` alone. A worktree with a broken snapshot is
named to machines too, by `broken_worktrees` under `status --json` — look at that key
when you are hunting for the worktree to fix. But it is **among the snapshots opened**:
if the names point at every picked-up row, not one sibling snapshot is opened and the
key does not stand even though one is broken. No key does not mean "nothing is broken".

A row picked up less than an hour ago does not show (that is the gap while a worker
raises its worktree). **A worktree that is still there while the session working in it
died does not show under `stranded`** — it is the worktree in `git worktree list` that
the script in 2 does not print as a `worktree` row.

- When there is such work, hand carrying it on to one idle session **before any new
  idea**. Send the text below in place of the text in 3, and after it append the text
  of 3 whole, **from 4-1 to the end** — leave 4-1 out and the worker taking over edits
  the tracker inside the worktree; cut the end off and the step after closing (clearing
  the window) is missing. Fill the placeholders as 3 says to (`<other work>` too — 4-3
  points at that line)

      Supervisor session (<my name>) is handing you the stalled work in <epic> — the previous session did not finish it.
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)
{reclaim_model}
      {BESIDE}
      Base branch: <base branch> — the supervisor read it in the root and filled it in; do not read it again.
{reclaim_check}
      Root: <root> — the `root dir` from 2. The tracker you edit is always the one there (4-1 of the text in 3)
      - If the worktree is there, go in with EnterWorktree(path), read how far it got with
        `git log <base branch>..HEAD` and `git status`, and carry on
      - If it is not, raise it again from the root. If the branch survives, on that branch
        (`git worktree add .claude/worktrees/<epic> worktree-<epic>`); if it does not,
        `git worktree add -b worktree-<epic> .claude/worktrees/<epic> <base branch>`
      - **If the root is not the top of the repository** (a subdirectory project in a
        monorepo), go into the worktree and then move to the same subdirectory inside it and
        work there — standing at the top, `moai` finds and writes the root's `.moai`, and the
        hook does not count edits under `.claude/`
          {SUBDIR}
      - The member's column is already picked up — do not pick it up again
      - The note in 9-1 records this window's share only. Append `reclaimed work, the previous
        session's share is unknown` to the end of the reason — the previous session's model and
        tokens are written nowhere, and without it the whole member reads as this window's work
      - The `2` the steps below point at is **the tracker commit with a path** — the root is
        shared by every session, so run `git commit -m "…" -- .moai/`. If a merge is open
        (MERGE_HEAD) git refuses it, so wait for that merge to finish and run it again

- **Whether it is carried on or put down is not the supervisor's call.** If it looks like
  work to put down (`moai mv <id> todo`, `moai defer <id> -m '<why>'`), ask the person
- Work handed on to be carried is, like an idea, not sent again until its report is checked

**1. Pick.** Out of the ideas that have piled up, keep only the ones that do not collide
with what is open right now.

    moai idea ls                           what has piled up
    moai show -s in_progress,review        what is picked up
    moai show <id>                         what that idea touches

Look at `git worktree list` too. An idea that touches the **same files, the same area**
as a worktree already standing or an epic already picked up comes out of this round —
when two of them change the same place, one waits for the other at the merge. **Compare
the ideas you send in this same round against each other too** — a worker only raises
its worktree after it receives the work, so what you just sent is not in the lists above
yet. Do this count again for every further idea. An epic left open with only
first-column members (what brief 7-1 left behind) shows as `in_progress` but is not
picked up — there is no worktree and no picked-up member, so do not drop ideas over it.

**If a milestone is running, what is inside it comes first.** The header of `moai ready`
says what is running and how many it held back outside it, and the `moai status` notice
shines on the same thing. Then what you send this round is work attached to that
milestone — an idea from outside waits for the next round unless it should stand as `p0`.
**The tool does not block this** (a pick-up goes straight through), which is why the
place to decide is here. If two milestones are running, both are inside.

**An idea from outside gets in only by being brought in.** `moai idea promote` has no flag
for a milestone and does not carry over the one the idea itself holds, so the epic a worker
unfolds stands outside the release until it is attached. The worker hangs it on in brief 1 —
`{MILESTONE_ATTACH}` — and what it writes there is the `<milestone>` you fill in 3. So the
call is yours, here, before you send: either this idea belongs in the release that is
running and you send it with that milestone, or it does not and you do not send it this
round. **Telling the worker not to attach a milestone is the same as handing out work
from outside** — that is how a worker came to pick up a row outside the running release
(2026-09-21), and the person, not the tool, is what caught it. With nothing running,
`<milestone>` is `none`.

**An idea you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai idea ls`, and the same idea goes to a second worker.

**2. Find a worker.** `ListAgents` does not show a session's place (cwd). Read
`~/.claude/sessions/*.json`, which Claude Code writes per session (under
`CLAUDE_CONFIG_DIR` if you moved it). For a subdirectory project in a monorepo, the
subdirectory that has `.moai` is the root. The script prints that `<root>` on the first
line, `root dir` (a session row below is labelled `root` — a different word on purpose,
so the path and a session never get read for each other), and if the root is not the top of the repository it prints the path from
the top down to the root on a `subdir` line — a worktree stands for the whole
repository, so the worker has to go into the same subdirectory inside it (brief 3).

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import glob, json, os, subprocess, sys
if not sys.argv[1]:
    sys.exit("call this inside a git repository")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
trees = os.path.join(root, ".claude", "worktrees") + os.sep
print("root dir", root)
if os.path.relpath(here, top) != ".":
    print("subdir", os.path.relpath(here, top))
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
def parents(pid):
    while pid > 1:
        yield pid
        try:
            out = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        except OSError:
            return
        pid = int(out) if out.isdigit() else 0
def reachable():
    """Can this supervisor reach the tmux server. If not, it cannot ask about a single pane."""
    try:
        r = subprocess.run(["tmux", "display-message", "-p", '#{{pid}}'], capture_output=True, text=True)
    except OSError:
        return False
    return r.returncode == 0 and r.stdout.strip().isdigit()
tmux_up = reachable()
def detached(s):
    """Was the tmux pane the session file names not spawned by that session on this server —
    that is, a pane on a separate test server.

    **None when it cannot be asked** — that is unknown, not detached. When the supervisor runs
    outside tmux or a sandbox blocks the socket, every question comes back empty, and reading
    that as detached filtered out every session in the root so nobody got any work (with no
    line saying why).
    """
    pane = str(s.get("tmux") or "").rpartition(".")[2]
    if not pane.startswith("%"):
        return False
    if not tmux_up:
        return None
    try:
        r = subprocess.run(["tmux", "display-message", "-p", "-t", pane, '#{{pane_pid}}'], capture_output=True, text=True)
    except OSError:
        return None
    owner = r.stdout.strip()
    # Reaching the server but not finding the pane means it is not this server's pane — that is detached.
    if r.returncode != 0 or not owner.isdigit():
        return True
    return int(owner) not in parents(int(s["pid"]))
if not tmux_up:
    print("cannot reach tmux — a test pane cannot be told apart. A test claude may be mixed into `root` below")
unread = 0
for f in glob.glob(os.path.join(home, "sessions", "*.json")):
    try:
        s = json.load(open(f))
        os.kill(s["pid"], 0)
        cwd = os.path.realpath(s["cwd"]) if s["cwd"] else None
    except OSError:
        continue
    except (ValueError, KeyError, TypeError, OverflowError):
        unread += 1
        continue
    if cwd is None:
        unread += 1
    elif cwd == root and detached(s):
        print("test pane", s.get("status"), s.get("name"))
    elif cwd == root:
        print("root    ", s.get("status"), s.get("name"))
    elif cwd.startswith(trees):
        print("worktree", s.get("status"), s.get("name"), cwd)
if unread:
    print("unreadable files", unread)
PY
```

- **Hand work only to a session whose place is the root and whose status is `idle` or
  `waiting`.** `busy` is working, and the other values (`shell` and such) mean something
  you do not know, so do not hand work to them
- **Leave out a session whose sent idea has not had its report checked.** A worker is in
  the root while it unfolds, picks up and merges — it shows `waiting` when it is waiting
  on a person's answer or a permission, and `idle` when it finishes a turn
- **Do not hand work to a `test pane` row.** The tmux pane the session file names was not
  spawned by that session on this server — it is a test `claude` raised on a separate
  tmux server (`-L`). Even with the root as its place, it is not a worker. The row is
  labelled `test pane`, not `detached`, on purpose: in this skill `detached` is the root's
  git HEAD (`If the root is detached, do not send`), and one word for both would stop a
  whole round over a test pane
- **If tmux cannot be reached at all, do not filter.** When the supervisor runs outside
  tmux or a sandbox blocks the socket, not one pane can be asked about — read that as
  detached and every session in the root drops out and nobody gets any work. The script
  prints one line, `cannot reach tmux`, and leaves them in, so take it that a test
  `claude` may be mixed into `root` and look at the names once more with `ListAgents`
  before you send
- A session whose place is `<root>/.claude/worktrees/*` is **working** in this
  repository. Watch it, but do not hand it work
- **Do not touch sessions in other directories**
- **A session that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person — once one has refused,
  do not even leave a notification on it after that
- The file survives the session, and if another process reuses that pid it reads as
  alive. Before you send, check once that the name also shows in `ListAgents`
- This file is an internal one, not in the documentation, so its fields may differ from
  release to release. If the script prints `unreadable files` or does not run, look at
  the names with `ListAgents` and tell them apart by asking — **only sessions on this
  machine** — for `pwd` and what they are doing; a remote or cloud session is a
  different checkout even when it names the same path

**2-1. Pick a model — by one word of difficulty.** The grade of the epic-end review
(brief 7) is measured on this same rubric, member by member. Keep two axes and the brief
carries two sets of judgement, and on the day they differ the cheap model takes the
write path.

{table}

The epic-end review is that epic's only review, because the members are not reviewed
separately (brief 5 and 7 carry that to the worker).

{epic_rule}

The supervisor picks before reading any code, so this is a suggestion; the last word
belongs to the worker who read the issue. A running session's model cannot be changed by
`SendMessage` and cannot be changed by config — the person in that window changes it
with `/model`.

**3. Send.** Send **one** idea to one idle session with `SendMessage`. The worker knows
nothing of this conversation, so send the text below **whole** — it is all the worker
receives, so everything the worker has to keep is inside it.
Fill in `<my name>`, `<id>`, `<title>`, `<base branch>`, `<milestone>`, `<model>`, `<difficulty>`, `<why>`, `<other work>` and `<root>`.
`<root>` is the `root dir` from 2. **Leave it unfilled** and the worker, inside its worktree,
reads its own place as the root.
`<milestone>` is the milestone you decided on in 1 — the one that is running, or `none`
when none is. **Leave it unfilled** and the worker hangs the placeholder itself on the
epic, which the tool refuses because it is not an id at all. **A wrong id it does not
refuse** — the check is the shape, not whether that milestone stands, so a stale one goes
in quietly and surfaces only later as a `dangling_milestone` warning. Copy it off the
`moai ready` header; do not write it from memory.
`<model>`, `<difficulty>` and `<why>` are the pair you picked in 2-1 and your reason.
**Leave them unfilled** and those placeholders travel as they are, so the note the worker
leaves when it closes says `<model>` instead of what actually did the work.
Do not use a single quote inside `<why>` — it closes the single quote in 9-1 and the rest
of the text leaks into the shell.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(brief 4-3).
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` or `<count>` either — those are the vendor and the token count the
worker reads in its own window in 9-1.

{brief}

**4. Wait.** On a working session, leave `notify_when_idle: true` with no message.
**Do not sweep `ListAgents` over and over** — the notification comes. It comes once only,
so if it arrives without a report (the worker asked the person something and finished its
turn), leave it again.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

**A member left because the work beside it holds the file** (brief 4-3) goes to an idle
session after that other work's report is checked. Send the text of 3 but fill `<id>`
with the epic, and in place of 1 write "this epic is already unfolded — do not promote;
pick up the first-column members from 2 on". Until then, count it in 1 as work holding
that file. Like the ones from 7-1, that member is right even when it is not done — it
keeps the epic open, so the epic is not done either.

A member the report says was left in the first column by brief 7-1 is right even when it
is not done — that member keeps the epic open, so the epic is not done either. It is not
an idea and it does not show in 1's list, so pass it to the person along with the reason
it was left (a person's decision, a file held beside it).

If the report holds and **that session has finished its turn** (the report is 11 and the
worker still has 12 to do), you may point out that the window is at a good place to be
cleared — the worker says the same in its own window (brief 12). The three checks look
only at the merge, the closing and the worktree; they do not read the notes. **Once you
have pointed it out, send the next idea only after the person has cleared that window or
said they will not** — a message sent before that disappears with a late `/clear`, and
that idea and that session sit out of the candidates waiting for a report that will never
come.

`<epic>` is the epic id carried in the report. An idea is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that idea's history
line about being unfolded.

If the three hold — on tmux, clear the window first with 5-1 — send the next idea to that
session. If they do not, ask that session what is left, and do not finish it in its place.

**5-1. On tmux, the supervisor clears the window.** Instead of pointing it out to the
person and waiting, the supervisor types `/clear` into that pane. Call it only after the
three hold, and only after the `Next:` note the worker leaves in 12 stands in the history
of `moai show <epic>` — that note is the last step of 12, so without it the worker is
still moving things into the tracker. Only a note that stands **after the report** counts
— `Next:` is also the hand-over line a session leaves when it could not finish, so an
epic reclaimed in 0 already has the previous session's one. The report (11) comes before
12, so when a report arrives the note is usually not there yet and the worker is `busy` —
do not point it out to the person: leave the notification from 4 and look after it
arrives. Clearing erases the whole conversation that worker holds, so never call it
before the check. `<session>` is the name of the session that sent the report, and
`<root>` is the `root dir` from 2.

```sh
python3 - '<session>' '<epic>' '<my name>' '<root>' <<'PY'
import glob, json, os, re, subprocess, sys, time
name, epic, me, root = sys.argv[1:5]
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
erased = False
def gone(kept, left):
    """**Only the lines erased** from the draft. What is left stands in order as lines of the
    earlier screen (`shrunk`), so those are subtracted and the rest returned. If what is left is
    not a line of the draft (the person typed meanwhile), None — what was erased cannot be told."""
    rest = iter(kept.split("\n"))
    out = []
    for line in left.split("\n"):
        if not line:
            continue
        for k in rest:
            if k == line:
                break
            out.append(k)
        else:
            return None
    out.extend(rest)
    return "\n".join(out)
def skip(why, then="only point out to the person that it can be cleared"):
    print("not clearing —", why, "—", then)
    if erased:
        # **Give back only what was erased.** Stop after erasing one line and the rest is still
        # in the box — give the whole draft back and the person pastes the lines still in their
        # box on top of it, so the same lines stand twice.
        left = draft(pane)
        if left and dim_only(pane):
            left = ""
        back = None if left is None else gone(kept, left)
        if back is None:
            print("erased part of the draft — look at what is left in that box and give the person of that window the `draft` copied above, minus the lines that overlap")
        elif not back.strip("\n"):
            # Only blank lines left means nothing was erased — blank lines in the box are skipped
            # above, so the draft's blank lines flow in here unpaired. Calling that "erased" would
            # tell the supervisor to hand back an empty text.
            print("the draft is still in that box — there is nothing to hand back")
        elif back == kept:
            print("the draft is already erased — give the person of that window the `draft` copied above")
        else:
            print("erased from the draft — hand back only this to the person of that window. The rest is still in that box")
            print(back)
    sys.exit(0)
def tmux(*args):
    return subprocess.run(["tmux", *args], capture_output=True, text=True)
def read(f):
    try:
        with open(f) as fh:
            s = json.load(fh)
        os.kill(s["pid"], 0)
        return s
    except (OSError, ValueError, KeyError, TypeError, OverflowError):
        return None
def session():
    found = [(f, s) for f in glob.glob(os.path.join(home, "sessions", "*.json")) for s in [read(f)] if s and s.get("name") == name]
    return found[0] if len(found) == 1 else (None, None)
def parents(pid):
    while pid > 1:
        yield pid
        try:
            out = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        except OSError:
            return
        pid = int(out) if out.isdigit() else 0
PROMPT = "\u276f"
SGR = "\x1b\\[([0-9;:]*)m"
def screen(pane, colour=False):
    args = ["capture-pane", "-p"] + (["-e"] if colour else []) + ["-t", pane]
    return tmux(*args).stdout.split("\n")
def draft(pane):
    lines = screen(pane)
    at = [i for i, l in enumerate(lines) if l.startswith(PROMPT)]
    box = []
    for line in lines[at[-1] :] if at else []:
        if line.startswith("─"):
            # The first line is the prompt and one space, the following lines two — strip only
            # that much and the indentation survives. Lines without that prefix are not cut: the
            # day the screen draws differently two characters would vanish silently, and the copy
            # is the only one the person has left, so nobody would see it shrink.
            head = (PROMPT + " ", "  ")
            return "\n".join((l[2:] if l[:2] in head else l[1:] if l[:1] == PROMPT else l).rstrip() for l in box).strip("\n")
        box.append(line)
def grey(code):
    """Is this foreground colour a dim grey. Look at both the 256-colour grey ramp and true colour
    (r=g=b) — Claude Code's colours are the theme's hex values, so a pane that takes true colour
    gets `38;2;136;136;136` rather than `38;5;244`. The black end is not grey — a light theme draws
    text the person typed as `rgb(0,0,0)`."""
    n = code.split(";")
    if code == "90":
        return True
    if n[:2] == ["38", "5"] and len(n) == 3 and n[2].isdigit():
        return int(n[2]) == 8 or 238 <= int(n[2]) <= 247
    if n[:2] == ["38", "2"] and len(n) == 5 and all(p.isdigit() for p in n[2:]):
        return len(set(n[2:])) == 1 and 64 <= int(n[2]) < 160
    return False
def sgr(code, was):
    """Fold one SGR piece into (dim attribute, dim foreground, reverse). tmux emits the foreground
    separately (`\x1b[2m\x1b[37m`) and gathers attributes into one piece — drop one attribute and it
    prefixes a reset, `0;2`; set two at once and it is `2;3`. So attribute pieces are read one by one."""
    attr, fg, rev = was
    n = code.split(";")
    if n[0] in ("38", "39") or (len(n) == 1 and n[0].isdigit() and (30 <= int(n[0]) <= 37 or 90 <= int(n[0]) <= 97)):
        return attr, grey(code), rev
    if n[0] in ("48", "58"):
        return was
    for p in n:
        if p in ("", "0"):
            attr, fg, rev = False, False, False
        elif p in ("2", "22"):
            attr = p == "2"
        elif p in ("7", "27"):
            rev = p == "7"
    return attr, fg, rev
def dim_only(pane):
    """Is every visible character in the input box dim — that is, Claude Code's suggestion text
    rather than text the person typed."""
    lines = screen(pane, True)
    bare = lambda l: re.sub(SGR, "", l)
    # Open the input box on the **same line** as `draft`. Searching with `in` lets a prompt glyph
    # inside text the person typed drag it further down, so the person's text above is missed. The
    # end of the box is read with `startswith` too — return true on one dash inside a pasted line
    # and `/clear` lands after the person's text.
    at = [i for i, l in enumerate(lines) if bare(l).startswith(PROMPT)]
    if not at:
        return False
    # Fold the colours from the top of the screen — tmux does not re-emit the same colour across a
    # line break, so the second line of a wrapped suggestion arrives with no colour piece. If no dim
    # character was seen at all, it is not true.
    was, prompt, cursor, seen = (False, False, False), True, True, False
    for n, line in enumerate(lines):
        if n > at[-1] and bare(line).startswith("─"):
            return seen
        for i, piece in enumerate(re.split(SGR, line)):
            if i % 2:
                was = sgr(piece, was)
                continue
            if n < at[-1]:
                continue
            if n == at[-1] and prompt and piece:
                piece, prompt = piece[1:], False
            # Claude Code draws the cursor of an empty box reversed over the first character of the
            # suggestion (with no dim). If the first character of the prompt line is reversed, take
            # that one cell out as the cursor and count the rest as it is.
            if n == at[-1] and cursor and piece.strip():
                cursor = False
                if was[2] and not (was[0] or was[1]):
                    piece = piece.lstrip()[1:]
            if piece.strip():
                if not (was[0] or was[1]):
                    return False
                seen = True
    return False
def looks(fmt):
    return tmux("display-message", "-p", "-t", pane, fmt).stdout.strip()
def shrunk(now, was):
    """Is this text the erasing pared down. One erase only empties a line and pulls the next one up,
    so every line left stands in order as **the same line** of the earlier screen (a blank line is
    the one just emptied). A line that does not match is text the person typed meanwhile — testing
    only whether a line is contained in the earlier screen cannot tell them apart, because one newly
    typed character or a retyped prefix is contained in some earlier line."""
    rest = iter(was.split("\n"))
    return all(not line or line in rest for line in now.split("\n"))
QUIET = '#{{pane_in_mode}}#{{pane_synchronized}}'
if not os.environ.get("TMUX"):
    skip("outside tmux")
try:
    tmux("-V")
except OSError:
    skip("no tmux")
f, s = session()
if not s:
    skip("could not find exactly one live session named " + name)
pane = str(s.get("tmux") or "").rpartition(".")[2]
if not pane.startswith("%"):
    skip("no pane in the session file")
if pane == os.environ.get("TMUX_PANE"):
    skip("the supervisor's own window")
if s.get("status") != "idle":
    skip("not idle — " + str(s.get("status")), "leave the notification from 4 and call again after it arrives")
cwd = str(s.get("cwd") or "")
if not cwd or os.path.realpath(cwd) != os.path.realpath(root):
    skip("not in the root — still inside a worktree")
owner = looks('#{{pane_pid}}')
if not owner.isdigit() or int(owner) not in parents(int(s["pid"])):
    skip("pane " + pane + " does not belong to that session")
if looks(QUIET) != "00":
    skip("the pane is in copy mode (the person is scrolling to read) or synchronized with others")
kept = draft(pane)
if kept is None:
    skip("could not read the input box")
if kept:
    print("draft —", name, pane)
    print(kept)
seen, ghost = kept, None
for _ in range(20):
    left = draft(pane)
    if left == "" and ghost is None:
        break
    if left is None or looks(QUIET) != "00":
        skip("lost the input box while erasing")
    # If text taken for dim changed when erased, it was not suggestion text — suggestions do not
    # erase. It is already copied above.
    if ghost is not None and left != ghost:
        skip("the person is typing", "stopped erasing. Also hand back the `dim text that appeared meanwhile` copied above to the person of that window")
    # Text the person typed while erasing was never copied out — erase more and no copy is left for
    # them (user decision).
    if not shrunk(left, seen):
        if not dim_only(pane):
            print("typed meanwhile —", name, pane)
            print(left)
            skip("the person is typing", "stopped erasing. Only point out to the person that it can be cleared")
        # A box emptied of the person's text gets dim suggestion text again — it is not typed text,
        # so do not stop. Do not tell them apart by colour alone either: copy it out and try
        # erasing, and if it erases it was the person's text drawn dim (`ghost` above).
        print("dim text that appeared meanwhile —", name, pane)
        print(left)
        ghost = left
    seen = left
    erased = True
    tmux("send-keys", "-t", pane, "C-e", "C-u", "DC")
    time.sleep(0.2)
else:
    # Tell them apart by erasing (user decision): if text that survived even the last stroke is all
    # dim, it is not what the person typed but Claude Code's suggestion — that does not land in front
    # of `/clear`, so type it. Do not require it to equal the draft — when a suggestion reappears in
    # a box emptied of the person's text, the draft is already copied out and only the suggestion is
    # left.
    rest = draft(pane)
    if ghost is not None and rest != ghost:
        skip("the person is typing", "stopped erasing. Also hand back the `dim text that appeared meanwhile` copied above to the person of that window")
    if rest and rest == left and dim_only(pane):
        if rest == kept:
            print("the `draft` above was dim suggestion text — the person did not type it")
            erased = False
            # It is not the person's text, so do not say "copied into the supervisor's window" on the
            # status line — a person who reads that goes hunting in the supervisor's window for text
            # they never wrote.
            kept = ""
        elif rest == ghost:
            print("the `dim text that appeared meanwhile` above was suggestion text — the person did not type it")
    elif rest != "":
        # If nothing was erased, that text is still in the box — say "already erased" and the
        # supervisor hands the person a second copy of text that is sitting right there.
        erased = rest != kept
        skip("could not empty the input box")
if (read(f) or {{}}).get("status") != "idle":
    skip("no longer idle")
if looks(QUIET) != "00":
    skip("the pane went into copy mode meanwhile")
# If the person typed after the last read, `/clear` lands after that text — read once more right
# before typing. Type it when the box is empty, unchanged, or holds only new dim suggestion text.
last = draft(pane)
if last is None or (last not in ("", left) and not dim_only(pane)):
    skip("text appeared in the input box meanwhile", "stopped erasing. Only point out to the person that it can be cleared")
say ="supervisor " + me + ": checked the report for " + epic + " — clearing this window" + (". The draft is copied into the supervisor's window" if kept else "")
for client in tmux("list-clients", "-t", pane, "-F", '#{{client_name}}').stdout.split("\n"):
    if client:
        tmux("display-message", "-c", client, "-d", "8000", "-t", pane, say)
tmux("send-keys", "-t", pane, "-l", "/clear")
time.sleep(0.5)
tmux("send-keys", "-t", pane, "Enter")
for _ in range(30):
    time.sleep(0.5)
    now = read(f)
    if now and now.get("sessionId") != s.get("sessionId"):
        print("cleared —", name, pane, epic)
        sys.exit(0)
print("cannot tell whether it cleared —", name, pane, "— look at that window before sending the next text")
PY
```

- **Call it only on a session you handed work to.** The session that sent the report is the
  worker, so the supervisor's own window and another supervisor's window are already out by
  name. The script filters its own pane (`$TMUX_PANE`) once more as well
- **With no tmux it skips quietly.** If `$TMUX` is unset or `tmux` is missing it prints one
  `not clearing` line and exits 0 — then point it out to the person and wait, as above. When
  the script prints `not clearing`, do what the end of that line says — only `not idle` means
  wait for the notification and call again; for the rest, point it out to the person
- **Type only into an `idle` pane.** Typing into `busy`, `waiting` or `shell` slips characters
  into a running turn or between a person's answers. Read once more right before sending. The
  times the worker said "do not clear" in 12 — a review running in the background, a merge
  conflict being resolved, waiting on a person's answer — hold here too. If the report or a
  message after it says any of that is left, do not call it
- **Do not type into a pane in copy mode, or a pane tied together by `synchronize-panes`
  either.** Copy mode means the person is scrolling to read, and the characters typed go to
  copy-mode keys where `/` opens a search. In a tied pane the characters go to every pane of
  that window and erase the conversation of the worker next door too
- **Read the pane from the session file's `tmux` field (`session:@window.%pane`) and check
  that the pane's process spawned that session.** The session file survives the session and
  pane numbers are reused, so someone else's session lives in the pane a stale file names
- **Erase the draft before typing** (user decision). Type over it and `<draft>/clear` goes to
  the worker as a prompt. Before erasing, read that text off the screen and copy it into the
  supervisor's window as `draft`, and one line on the status line of every client watching
  that pane says so — the person looks for that text in the supervisor's window. Claude
  Code's `Ctrl+Y` restores only the last line of a multi-line text, so it cannot be relied
  on. The input box is read as the last line of the Claude Code screen where the prompt glyph
  (U+276F) stands. That screen, like the session file, is undocumented, so where it cannot be
  read it falls towards not typing. If erasing stops midway, **only the lines erased so far**
  come out with it — `the draft is already erased` when it all went, and `erased from the
  draft` gives the lines when it stopped after one. The rest is still in the box, so handing
  the whole draft back would make the same lines stand twice. If what is left is not a line
  of the draft (the person typed meanwhile) what was erased cannot be told, and then it says
  to look at that box and leave the overlapping lines out
- **Strip only the two leading columns when copying** (user decision). The first line is the
  prompt and one space, the following lines two spaces, and the rest is the screen as it is —
  trim every line and indented code comes back flattened. Lines that do not carry that prefix
  are **not cut** — the day the screen draws differently two characters would be shaved off
  silently, and the copy is the only one the person has left, so nobody would see it shrink.
  A line the screen wrapped cannot be told from a newline the person typed, so the copy may
  show one newline more than there was
- **Tell text that will not erase apart by erasing it** (user decision). The dim suggestion
  Claude Code floats in an empty box is not what the person typed, and it does not erase.
  When it survives twenty strokes and all of it is dim (`capture-pane -e`), take it as a
  suggestion and type `/clear` — a suggestion does not land in front of `/clear`. Colour
  alone does not decide it, because on a build that draws the person's text dim `/clear`
  would land after that text. Text that erases is always taken as the person's. Claude Code
  draws the cursor of an empty box reversed over the suggestion's first character, so that one
  cell does not count as text. A suggestion reappearing in a box emptied of the person's text
  is the same — the draft is already copied out, so type it
- **Stop if the person types while you erase** (user decision). Read the input box again after
  every stroke, and the moment text appears that was not pared down from the earlier screen,
  stop and do not type `/clear` — that text was never copied out, so erasing more leaves the
  person no copy. Copy the new text into the supervisor's window as `typed meanwhile`, and
  `the draft is already erased` speaks for what went before. Merging them and erasing on is
  not the road taken — while the person types, `/clear` lands after their text. A line left
  must be **equal** to a line of the earlier screen to count as pared down — test only
  whether it is contained and one newly typed character is contained in some earlier line.
  If the new text is all dim it may be a suggestion that reappeared in a box emptied of the
  person's text, so do not stop: copy it out as `dim text that appeared meanwhile` and try
  erasing — if it erases, take it as the person's text and stop. Read the input box once more
  right before typing `/clear` too
- **Do not clear and assign in one breath.** `/clear` erases what is queued along with it.
  Send the next idea after the script prints `cleared` — after the session id changed — and
  on `cannot tell whether it cleared`, do not send before you have looked at that window
- **Leave one line in your own window when you clear** — `cleared <session> pane %N (<epic>)`.
  A person who was watching that window finds in the supervisor's window why the screen went away
- **Do not type into a live worker's window while testing.** Raise the pane you are testing
  on a **separate tmux server** and give the same name to **every** call that reaches that
  server — `new-session`, `send-keys`, `capture-pane`, `display-message`, `list-clients`,
  `kill-session`. Put the epic id in the name so it cannot collide with the test servers of
  the workers beside you or of a review subagent. To run the script in that pane, put a
  `tmux` wrapper that inserts `-L` at the front of `PATH` — the wrapper has to call the real
  `tmux` **by absolute path** so it does not call itself again. Call the script itself without
  `env -u TMUX` — with no `$TMUX` it skips as `outside tmux`

      env -u TMUX tmux -L <unique name> new-session -d -s <pane> …
      env -u TMUX tmux -L <unique name> capture-pane -p -t <pane>
      mkdir -p <scratchpad>/bin; printf '#!/bin/sh\nexec env -u TMUX %s -L <unique name> "$@"\n' "$(command -v tmux)" > <scratchpad>/bin/tmux
      chmod +x <scratchpad>/bin/tmux; PATH=<scratchpad>/bin:$PATH python3 - …      the script into that pane
      env -u TMUX tmux -L <unique name> kill-server          to clean up — only the server of that name dies

  **Never use `tmux kill-server` or `kill-session` without `-L`/`-S`.** Inside tmux a bare
  `tmux` follows `$TMUX` to the person's default server, and every pane and session on that
  machine dies at once. `TMUX_TMPDIR` does not fence it in — `$TMUX` wins.
  A bare `tmux new-session -d` is not isolation either, because it raises the pane on the
  default server — cleaning up then means running `kill-*` against the default server, and
  that road has killed a whole server before

## The shared root

The root checkout is **shared by every session.** While one session has a merge open
(`MERGE_HEAD`), another session committing a tracker note seals that merge with its own
subject — that has actually happened. So in the root, supervisor and worker alike:

- Give the tracker commit a path — `git commit -m "…" -- .moai/`. With a merge open git
  refuses a commit with a path, so wait until the session that opened it finishes and run it
  again. A `git commit` without a path seals that merge even when you ran `git status` first
- Finish your own merge in one call, `git merge --no-ff <branch> -m "…"`. Do not use
  `--no-commit`. If it stops on a conflict, do not resolve it in the root: `git merge --abort`

## When to stop

- If there is no idea that does not collide, or no session idling, say so to the person and
  stop — do not force a colliding idea out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
"#
    )
}

/// 되짚기(7-1)가 에픽 목적에 걸리는 idea 를 선 에픽의 멤버로 되찾는 줄(moai-l288).
/// **이것도 promote 다**(moai-f3ml) — `add` 와 손 닫기로 적었던 판은 "idea 를 일감으로 바꾸는
/// 길은 promote 하나" 를 어겼고, 그것을 지키던 시험에서 이 줄을 빼야 했다.
///
/// **`-C <루트>` 를 줄에 박는다.** 7-1 은 워크트리에서 치는데, 글로만 "4-1 대로" 라고 적고 줄을
/// 맨 `moai` 로 두면 그대로 옮겨 친 줄이 워크트리의 `.moai` 에 멤버를 세운다 — 병합에서 스냅샷이
/// 부딪히거나, 4-1 이 시키는 `git checkout -- .moai` 로 그 멤버가 사라진 채 에픽이 닫힌다.
/// `<루트>` 는 감독이 채우는 자리라 받은 줄에 실제 자리가 박혀 온다. 다른 자리 이름은 그 목록과
/// 겹치지 않는다 — 겹치면 맡긴 idea 의 값이 이 줄에 미리 박힌다(`<등급>` 와 같은 덫).
const RECALL: &str = "moai -C <root> idea promote <idea id> -e <epic> --from -";

/// 감독이 일꾼에게 `SendMessage` 로 싣는 글. **일꾼이 받는 것은 이것뿐이다** — 감독
/// 스킬의 다른 절을 가리키면 일꾼에게 없는 글을 가리키는 것이라(첫 판의 "아래 공유
/// main" 이 그랬다), 일꾼이 지킬 것은 모두 여기 적는다.
///
/// **리뷰 줄은 규칙 3 의 조각으로 적는다.** 손으로 줄인 `-t review --parent <에픽>` 은
/// 관점(`-b`)이 빠져, 에픽 리뷰가 무엇을 왜 보는지 없이 섰다.
///
/// **닫기는 워크트리를 지운 뒤다.** 워크트리가 남아 있으면 훅이 그 에픽을 옆 워크트리의
/// 일로 읽어(`worktree::away`) `-m` 없는 리뷰 닫기를 못 막고, 루트의 편집은 규칙 2 로
/// 막는다 — 그래서 충돌과 시험은 워크트리에서 풀고, 루트 병합이 막히면 되돌리고 돌아간다.
///
/// **넘칠 때의 길도 여기 적는다**(리뷰 moai-4u6b.5hl). 닫는 걸음이 싣는 것은 `(64KB 를 넘으면
/// 요약)` 한 마디뿐이라, 이 글만 받은 일꾼은 줄이라는 말은 읽고 **요약이라고 밝히라는 말은**
/// 못 읽었다 — 밝히지 않은 요약은 다음 사람이 리뷰어의 말로 읽는다.
fn brief() -> String {
    let review = make_review("--parent <epic>");
    let close = indent(&close_steps("<review id>", "moai"), "       ");
    let over = indent(REVIEW_OVER_LIMIT, "       ");
    let model = indent(&model_line(), "    ");
    let levels = difficulty_levels();
    let rubric = indent(&difficulty_rubric(), "       ");
    let top = top_model();
    let epic_rule = indent(&epic_review_rule(), "       ");
    let branch_check = indent(BRANCH_CHECK, "    ");
    format!(
        r#"    Supervisor session (<my name>) is handing you idea <id> — <title>.
    Read first: moai show <id>
{model}
    {BESIDE}
    Base branch: <base branch> — the branch name below. The supervisor read it in the root and filled it in; do not read it again.
{branch_check}
    1. Unfold it in the root — the one way to turn an idea into work is
       `moai idea promote <id> --from -`. Unfold into an epic plus issues even for a single
       issue. Look at `--dry-run` first — that is for this window to see, not to show a person
       and ask. Showing a split plan to a person once is a step of work a person asked for
       directly; what a supervisor hands you is work a person already passed on. Design
       decisions are asked in 4.
       If that idea is already done (someone unfolded it), do not unfold: tell the supervisor —
       unfolding again puts up two epics. **Write a short new title** — a line in the plan
       becomes the issue title verbatim, so copying over an idea title that grew long while it
       was parked spreads that length into the issues. The original text stays on that idea and
       the history leads back to it.
       Then hang the milestone on the epic you unfolded — `promote` has no flag for it, and a
       milestone is inherited, so the epic alone carries it to every member and to the members
       added later in 4-3 and 7-1. If `<milestone>` is `none`, nothing is running and there is
       nothing to hang
         {MILESTONE_ATTACH}
    2. Pick the members up with `moai mv <member> in_progress --from todo` and commit in the root.
       **Pass the column you saw** — this is a place where several sessions share one `.moai`,
       and overwriting a row picked up beside you means two of you do the same work. A non-zero
       code means it is taken, so leave that member and tell the supervisor. The root is shared
       by every session — a commit made while someone has a merge open (MERGE_HEAD) seals that
       merge with its own subject. So give the tracker commit a path. With a merge open git
       refuses it, so wait for that merge to finish and run it again
         git commit -m "chore(tracker): pick <epic> up in a worktree" -- .moai/
    3. Right after the commit in 2, branch from the local <base branch> with
       `git worktree add -b worktree-<epic> .claude/worktrees/<epic> <base branch>` and go in with
       EnterWorktree(path). The name is the unfolded epic's id, not the idea's. Until the worktree
       stands, the other sessions in the root read this member as their own focus.
       **If the root is not the top of the repository** (a subdirectory project in a monorepo) the
       worktree stands for the whole repository, so once inside, move to the same subdirectory in
       it and work there — standing at the worktree top, `moai` walks up and finds the root's
       `.moai` to write, and the hook does not count edits under `.claude/`
         {SUBDIR}
    4. Do not guess a design decision that is not in the notes — ask with AskUserQuestion; a
       person is watching the worker's window
    4-1. **The tracker you edit is always the root's.** `<root>` is the root checkout's place,
       filled in by the supervisor — do not guess it from inside the worktree. **The tool moves
       that by itself** — even a bare `moai` typed inside the worktree reads and writes the
       root's tracker, and when it writes, one line says where. `moai -C <root> <command>` lands
       in the same place, so writing it that way is fine too. Short of a branch with no tracker
       in the root, the only thing that stops the move is `MOAI_HERE`, so **do not turn it on** —
       turn it on and that worktree's `.moai` changes, and the snapshots conflict on the merge
       (and merging them overwrites someone else's rows).
       Put `-e <epic>` on an idea you park mid-epic — it does not keep the epic open, and 7-1
       reclaims it through that even if the window is cleared or the work is taken over.
       **Give a review subagent the same words.** If that worktree's `.moai` changed anyway,
       undo it with `git checkout -- .moai`, and if the row was already committed, undo that
       commit too and park it again from the root
    4-2. **If you test tmux, do it on a separate server only** — `env -u TMUX tmux -L <unique name>`
       on every call. Put the epic id in the name so it cannot collide with the test servers of
       the workers beside you or of a review subagent. `-S <socket>` works too, but a socket path
       does not stand past the unix limit (about 100 bytes), and a scratchpad path is usually too
       long. Never use `kill-server` or `kill-session` without `-L`/`-S`: inside tmux a bare
       `tmux` goes to the person's default server and kills every session, and `TMUX_TMPDIR` does
       not fence it in. A script that calls `tmux` inside itself cannot be given `-L` by hand, so
       run it with a wrapper at the front of `PATH` that calls the real `tmux` by absolute path
       and inserts `-L`. Do not send keys into a pane someone else raised. If you raise a test
       `claude` on that server, keep its cwd outside the root (the scratchpad) — a session raised
       in the root slips into the supervisor's session list as an idle worker.
       **Give a review subagent these words too** — it was a review subagent that killed a whole server
    4-3. **If you would have to touch a file that work running alongside holds, do not fix it** —
       the files named by `Work running alongside` in the header, or files a sibling branch in
       `git worktree list` already changed
       (`git diff --name-only <base branch>...<sibling branch>`). When two of them change the
       same place, one waits for the other at the merge. If this epic cannot deliver what it
       promised without that, it is not an idea but a member — create it with
       `moai -C <root> add '<what>' -e <epic>`, leave it in the first column, and name it in 11
       as **a member left because the work beside it holds the file**, together with that other
       work. The supervisor sends it once that work is done. Do not defer it
    4-4. **Polish Korean text going into moai with the Korean writing plugins** — every title,
       body, note, `-m` and review-text note that carries Hangul. Polish with
       `korean-skills:humanizer`, and when it runs past 20 lines put it through
       `humanize-korean:humanize-korean` outside the repository (the scratchpad), delete that
       `_workspace/`, come back into the worktree (standing outside it the hook cannot find the
       tracker and no rule stands), and finish with `korean-skills:grammar-checker` for spelling.
       Leave ids, commands, paths and numbers as they are, and do not polish the model line in
       9-1, the `Next:` line in 12, a `Regression-of:` line, or the `Summary:` first line of a
       shortened review text. **Give a review subagent these words too**
    5. **Do not review member by member.** When one member is finished, run the tests, commit and
       move to the next — the review looks at the whole epic once, in 7, after every member is
       finished. One review is expensive; do not call it as many times as there are members. The
       cost of member 2 piling onto a bug in member 1 is paid in that one review.
       {levels} is the rubric the model in the header was picked on, and the same rubric measures
       the members when you pick the grade in 7.
{rubric}
    6. When the members' work is all done, pull <base branch> into the worktree, resolve the
       conflicts and run the tests. Fix things here — while the worktree stands, rule 2 blocks
       edits in the root
    7. Before merging, review the whole epic with `/code-review <grade> --fix` — the members were
       not reviewed separately, so this once is the only review.
{epic_rule}
       If the window is not on that model, ask the person watching it for `/model <that model>`
       before you call — as in `/model {top}` (a review agent inherits the window's model).
       Write the grade you picked and why in one line in the angle (`-b`). The diff runs from
       where the branch left <base branch> (`git merge-base <base branch> HEAD`). You pulled
       <base branch> in 6, so the conflict resolution is inside it too. Create the review issue
       (rule 3)
         {review}
       This line is called from the worktree too, so run it as `moai -C <root>`, per 4-1. What
       you take in goes in a separate fix: commit; what you hand on goes in a note with the issue id.
       Only when the worktree's hook cannot see a review issue created or picked up in the root
       and blocks you — a binary from before the hook moved the tracker to the root reads that
       worktree's snapshot only — run a review subagent with the same angle, grade and `--fix`
       scope. A subagent inherits the window's model, so pass the model for the grade above in
       `Agent`'s `model`. Keep the review issue, the angle (`-b`), the text note and the closing
       `-m` as they are. Any other refusal, such as a missing angle, is not worked around: fix it
       the way the refusal's own command says
    7-1. Before merging, go back over the ideas parked mid-epic
       (`moai -C <root> show --type idea -e <epic>` and what this window remembers) and what the
       review handed on — **can the epic deliver what it promised without them.** If not, it is
       not an idea but an unfinished member. What you sorted as "not for now" while parking has
       these mixed in — the one waiting on a person's decision, the one pushed out because a
       worker beside you held that file. This step sits after 7 so that it sees what 7's review
       handed on too. Unfold such an idea as a member of the epic already standing — write only
       `- issue` lines in the plan; the idea closes by itself and its source stays. You type this
       from the worktree, so pin the root into the line (4-1). **If that idea is already done, do
       not unfold it** — someone unfolded it, or you came back from 8 and are going round again.
       promote unfolds a closed idea too, and the same member stands twice
         {RECALL}
       Do not do a reclaimed member here: merge with it left in the first column — work that has
       not been through 7's review does not get mixed into the merge, and a member still standing
       keeps the epic open. Do not `defer` that member. Deferring it closes the epic without its
       promise delivered. 7's review did not see that member, so write it in the `Next:` note in
       12 — the window that closes the epic with that member calls the epic-end review again
    7-2. **If the epic ran inside a milestone, sort what you handed on once more, by the release
       bar.** A review makes its findings without regard to the release bar, so fixing all of them
       inside pushes the release out by as many findings as there are, and sending all of them
       outside ships with bugs in. This window is what sorts them — you already decided in 7 what
       to take in and what to hand on, so this is the extension of that. **Bug-level stays
       inside**: create it as a member of that epic (`-e <epic>`) or under an epic in the same
       milestone. **What this release can do without goes outside** — not "not doing it", but
       "not in this release"
         moai -C <root> edit <that row> -e none --milestone none
       **Pass `-e none` with it.** A milestone is inherited from the epic, so on an epic member
       `--milestone none` alone changes nothing and comes back with one line saying the place
       comes from the epic and cannot be cut — a row 7-1 reclaimed stands as that epic's member,
       so take it out of the epic here as well. That the row stops keeping the epic open is the
       point: it is a row decided out of this release.
       **Bug-level is measured with the words that already exist** — does a `#bug` tag fit, and
       can a `Regression-of:` line be written (did something already merged break). Those two are
       inside; the rest is outside. A new axis is not made because it would become a fourth
       vocabulary beside the column, the kind and the defer
    8. Come back to the root with ExitWorktree(keep) — remove it from inside the worktree and the
       session's place stays in a directory that is gone, and the supervisor never sees this
       session in the root again.
       Before merging, check that the root stands on <base branch> — if it does not, do not merge:
       tell the supervisor
         git symbolic-ref -q HEAD                  it has to be refs/heads/<base branch>
       Merge in the root, **in one call**. Overlap with the workers beside you was split when the
       supervisor sent the work, and where it still collides, undo as below and resolve in the
       worktree — do not go looking for the other session to tell it. Do not use `--no-commit`.
       Without `--no-ff` it ends as a fast-forward and no merge commit stands
         git merge --no-ff worktree-<epic> -m "merge: …"
       If the root's `.moai` holds uncommitted rows from another session the merge is refused —
       take them in first with a commit with a path, as in 2. If it stops on a conflict, do not
       resolve it in the root — undo with `git merge --abort`, go back into the worktree with
       EnterWorktree(path) and run again from 6
    9. Once the merge has really landed, remove the worktree and the branch from the root with
       `git worktree remove .claude/worktrees/<epic>` and `git branch -d worktree-<epic>`
    9-1. Before closing, leave one line per member **on what did this work** in this window —
       leaving out the members left in the first column by 7-1 and 4-3, which nobody did. Not the
       suggestion in the header but the model that **actually ran** in this window. The line below
       was filled in by the supervisor as a suggestion, so if you raised it, or the window was on
       a different model from the start, correct the model and the difficulty to the real ones and
       write why in the reason — the next person reads "what was put on work of this size" there.
       It is a note, not a field: the journal is not read to compute state and derived values are
       not stored. The supervisor does not fill `<vendor>` or `<count>` — the vendor is
       `anthropic`, `openai` or `google`, and the model is its real name (`opus-5`), not the
       `/model` alias — the `<model>` the supervisor filled in is an alias (`opus`), so correct it
       to the real name even if you did not change models.
       `<count>` is the tokens this window used. **If you do not know the token count, drop
       `tokens=<count>` whole** — do not write 0 and do not estimate. **One line per id** —
       write the same line on several ids and the tokens multiply by the number of ids. A window's
       tokens cannot be split per member, so write them on **one member only** and leave
       `tokens=<count>` out of the other members' lines.
       Quote free text with single quotes — inside double quotes the shell expands backticks and
       `$(…)` as commands. If the text itself contains a single quote, stream it from stdin with `-b -`
         moai note <member> 'model: <vendor>/<model> tokens=<count> (<difficulty> — <why>)'
    10. Close them after that. **Run `moai mv <member> done` only once that merge has really
       landed** — a worker moved them before the merge and had to undo it. Do not close the
       members left in the first column by 7-1 and 4-3 — those members keep the epic open. While
       the worktree still stands, the hook reads this work as a sibling worktree's and cannot
       refuse a review closed without `-m`. Close the review issue leaving what came out of it
{close}
{over}
       Leave the tests passing in the root with a commit with a path, as in 2
    11. Report with SendMessage to "<my name>" — the merge hash, the unfolded epic's id, a line or
       two of summary, what you handed on and any new ideas, the members reclaimed in 7-1 and left
       in the first column, and the members left in 4-3 because the work beside you held the file,
       with that other work named
    12. Finally, **say when the window can be cleared.** Leave the line to take over from
       (`moai note <epic> 'Next: …'`), take it into the root with a commit with a path as in 2 —
       it is written after the commit in 10, so leaving it out leaves it in the shared root where
       someone else's commit sweeps it up — and tell the person watching that window, in one line,
       that a `/clear` is fine now. The context lives in the tracker, not in the conversation:
       issue bodies, notes, review texts, commit messages. If you can see your own context usage,
       put that number in the line too.
       **Say the opposite in the same line** — not to clear while a review is running in the
       background, while a merge conflict is being resolved, while waiting on a person's answer,
       or after the supervisor's next message has arrived in this window. Clearing then loses what
       is not yet moved into the tracker, or the message that arrived.
       On tmux the supervisor may check the report and type `/clear` into this window itself — the
       supervisor types it once it sees the `Next:` note stand. That is why the note is the last
       step: if anything is left (a background review, say), finish it before the note, and if you
       cannot, send the supervisor one more line saying so before the note (the report already went
       in 11) — the supervisor does not clear such a window"#
    )
}

/// 줄마다 앞에 붙인다. 빈 줄은 빈 채로 둔다 — 꼬리 공백은 diff 를 더럽힌다.
fn indent(text: &str, by: &str) -> String {
    text.lines().map(|l| if l.is_empty() { String::new() } else { format!("{by}{l}") }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 감독이 채우는 자리 목록 한 줄 — 셋이 이 줄에서 자리 이름을 센다. 손으로 세 벌 찾던 판은
    /// 줄의 모양이 바뀔 때 한 곳만 고쳐졌다.
    fn slot_list(supervise: &str) -> &str {
        supervise.lines().find(|l| l.starts_with("Fill in `<my name>`")).expect("감독이 채울 자리 목록이 없다")
    }

    /// **공통 조각은 두 표면에 똑같이 든다.** 한쪽만 고치면 여기서 붉어진다 —
    /// 조각을 거치지 않고 표면에 글을 직접 적는 순간 두 벌이 다시 생긴다.
    #[test]
    fn both_surfaces_carry_the_same_pieces() {
        let (agents, skill, reference) = (agents(), skill(), reference());
        assert!(reference.contains(&korean_detail()), "참고 문서에 한국어 글 절차가 없다");
        for piece in [CHEATSHEET, FORKS, NO_GATE, WRITING, KOREAN, CLOSING] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(skill.contains(piece), "스킬에 없다 — {head}");
        }
        assert!(CLOSING.contains(&handoff("<id>")), "안내의 핸드오프 줄이 훅과 갈라졌다");
        let rules = rules();
        assert!(agents.contains(&rules) && skill.contains(&rules), "규칙 셋이 갈라졌다");
        for piece in [GROUPS, IDEAS, DEFERRING, PEOPLE, PROJECTS, LANGUAGE, UPDATES, COMMITS] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(reference.contains(piece), "참고 문서에 없다 — {head}");
        }
    }

    /// **안내가 부르라는 스킬이 설치한 플러그인의 것이다**(moai-5wk4). 설치 명령은 `korean` 이
    /// `KOREAN_PLUGINS` 에서 뽑으니 따로 안 잰다 — 손으로 적는 것은 스킬 이름뿐이다. 플러그인
    /// 스킬은 `<플러그인>:<스킬>` 로 불려, 설치 id 의 `@` 앞과 스킬 이름의 `:` 앞이 갈라지면 시킨
    /// 대로 설치해도 부를 스킬이 없다. 첫 판은 `im-not-ai 의 humanize-korean` 처럼 마켓플레이스
    /// 이름을 댔다. 워커 브리프도 같은 이름을 싣는다.
    #[test]
    fn the_korean_piece_names_skills_of_the_plugins_it_installs() {
        let brief = brief();
        for (id, _) in KOREAN_PLUGINS {
            let (plugin, _) = id.split_once('@').unwrap();
            assert!(KOREAN.contains(&format!("`{plugin}:")), "안내가 {plugin} 의 스킬을 안 댄다");
            assert!(brief.contains(&format!("`{plugin}:")), "브리프가 {plugin} 의 스킬을 안 댄다");
        }
    }

    /// **가르친 커밋 제목을 커밋 칸이 실제로 읽는다**(moai-wqm7). 예시 제목에서 id 를 못 뽑거나
    /// 트래커 커밋의 머리가 `git` 이 거르는 머리와 다르면, 시킨 대로 커밋해도 칸이 비거나 트래커
    /// 커밋이 상세에 선다.
    ///
    /// **가르친 트레일러 낱말도 함께 잰다**(moai-dig5). 이 글은 squash 로 합치는 저장소에
    /// `Refs: <id>` 를 달라고 시킨다 — 그 낱말을 `git::trailed` 가 안 읽으면 시킨 대로 단
    /// 저장소의 커밋 칸이 통째로 빈다. 낱말은 글에서 뽑아 견준다: 손으로 두 벌 적으면
    /// 한쪽만 고쳐도 여기가 안 붉어진다.
    #[test]
    fn the_commit_guide_teaches_what_the_commit_column_reads() {
        let example = COMMITS.split('`').nth(1).expect("예시 제목이 없다");
        assert!(example.contains("<id>"), "예시 제목에 이 저장소의 id 를 박았다 — {example:?}");
        // 자리에 어떤 저장소의 id 가 들어가도 그 id 하나만 읽힌다.
        let subject = example.replace("<id>", "web-a1b2");
        assert_eq!(crate::git::ids_in(&subject).collect::<Vec<_>>(), ["web-a1b2"], "예시 제목 {subject:?}");
        assert!(
            COMMITS.contains(&format!("`{}:`", crate::git::TRACKER)),
            "트래커 커밋의 머리가 git 이 거르는 것과 다르다"
        );

        let rule =
            COMMITS.lines().find(|l| l.contains("only lines that open with a trailer word")).expect("본문 규칙이 없다");
        let heads: Vec<&str> = rule.split('`').skip(1).step_by(2).collect();
        assert!(heads.len() >= 3, "가르친 트레일러 낱말이 셋이 안 된다 — {rule:?}");
        for head in heads {
            let line = format!("{head} web-a1b2");
            assert!(
                crate::git::trailed(&line).any(|w| w == "web-a1b2"),
                "가르친 트레일러를 커밋 칸이 안 읽는다 — {line:?}"
            );
            // squash 가 담는 모양(네 칸 들여쓴 줄)도 같게 읽는다.
            assert!(
                crate::git::trailed(&format!("    {line}")).any(|w| w == "web-a1b2"),
                "squash 가 들여쓴 트레일러를 못 읽는다 — {line:?}"
            );
        }
    }

    /// **이모지를 쓰지 말라는 글이 제 손으로 이모지를 쓰지 않는다**(moai-j8aq). 가르치는 글이
    /// 제 규칙을 어기면 읽는 쪽은 그것을 규칙이 아니라 취향으로 읽는다.
    ///
    /// 세는 자리는 넷이다 — 이모지 판(U+1F300 위), **이모지로 그리라는 표시(U+FE0F)**, 그리고
    /// 기호 판의 이모지 구역(U+2600–U+27BF·U+2B00–U+2BFF). 판 위만 세면 `⚠️`·`❗`·`✅` 가
    /// 그대로 지나간다 — 실제로 에이전트가 제일 잘 쓰는 것이 그 셋이다.
    ///
    /// `⎇`·`→` 같은 글리프는 구역 밖이라 저절로 살고, 구역 안의 `✓` 만 따로 뺀다 — 보드가 색
    /// 대신 쓰는 기호라 여기서 막으면 안 된다. 색이 혼자 뜻을 지지 않게 하는 쪽이 먼저다.
    #[test]
    fn the_style_piece_obeys_itself() {
        assert!(WRITING.contains("emoji"), "글 스타일에 이모지 이야기가 없다");
        let emoji = |c: char| {
            c != '✓'
                && (c >= '\u{1F300}' || matches!(c, '\u{FE0F}' | '\u{2600}'..='\u{27BF}' | '\u{2B00}'..='\u{2BFF}'))
        };
        for (surface, text) in
            [("AGENTS 블록", agents()), ("스킬", skill()), ("참고 문서", reference()), ("감독 스킬", supervise())]
        {
            let found: String = text.chars().filter(|c| emoji(*c)).collect();
            assert!(found.is_empty(), "{surface} 이 이모지를 쓴다 — {found}");
        }
    }

    /// **심는 글이 가리키는 파일은 이 저장소의 것이라고 말한다**(moai-nnda). 이 글은 모든
    /// 저장소에 심긴다 — 상대 경로로 적으면 남의 저장소에서는 없는 파일을 가리킨다.
    #[test]
    fn the_example_link_says_whose_repository_it_is() {
        let path = "examples/bash-agent/agent.sh";
        // **집는 것은 모든 자리다.** 첫 자리만 보면 뒤에 맨 경로를 하나 더 적어도
        // 이 시험이 지나간다 — 걸러야 할 것은 바로 그 둘째 줄이다.
        for (surface, text) in [("AGENTS 블록", agents()), ("스킬", skill()), ("참고 문서", reference())] {
            for (at, _) in text.match_indices(path) {
                assert!(
                    text[..at].ends_with("moai repository's `"),
                    "{surface} 이 {path} 를 어느 저장소의 것인지 없이 가리킨다"
                );
            }
        }
        // **가리키기는 하는지도 본다.** 위 고리는 자리마다 재는 것이라 글에서
        // 통째로 빠지면 한 번도 안 돌고 지나간다 — 사라지는 쪽이 어긋나는 쪽보다 흔하다.
        for (surface, text) in [("AGENTS 블록", agents()), ("스킬", skill())] {
            assert!(text.contains(path), "{surface} 이 사람 없이 도는 예제를 더는 가리키지 않는다");
        }
    }

    /// **예시가 제 스타일을 지킨다**(moai-1xf2). 스타일을 가르치는 글에서 예시가 어긋나면
    /// 읽는 쪽은 규칙이 아니라 예시를 따라 적는다 — 예시가 실제 글이고 규칙은 설명이다.
    ///
    /// 제목의 잣대는 **보드가 실제로 쓰는 자**다 — `view::TITLE_CAP` 을 `text::width` 로 잰다.
    /// 손으로 적은 숫자를 `chars().count()` 로 재던 판은 둘 다 틀렸다: 한글 한 글자는 두 칸이라
    /// 글자 수로 재면 상한이 두 배로 늘고, 그렇게 지나간 첫 예시 제목이 48칸으로 보드에서
    /// 실제로 잘렸다 — 짧게 쓰라고 가르치는 글이 잘리는 제목을 내밀고 있었다.
    ///
    /// 규칙이 아니라 **예시에만** 매는 잣대다 — 이슈 제목을 셈해 막는 자리는 없다.
    #[test]
    fn the_style_example_obeys_the_style() {
        let reference = reference();
        assert!(reference.contains(WRITING_EXAMPLE), "참고 문서에 글 스타일 예시가 없다");
        // 여는 `moai add '` 에 맨다 — 첫 따옴표로 찾으면 앞 산문에 따옴표가 하나 들면
        // 조용히 엉뚱한 토막을 제목으로 재고도 초록이다.
        let title = WRITING_EXAMPLE
            .split_once("moai add '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(t, _)| t)
            .expect("예시 명령에 제목이 없다");
        let cap = crate::view::TITLE_CAP;
        let w = crate::text::width(title);
        assert!(w <= cap, "예시 제목이 보드({cap}칸)에서 잘린다 — {w}칸, {title}");
        assert!(!title.contains('\n'), "예시 제목이 두 줄이다 — {title}");
        // 본문은 목록이고, 가르친 갈래는 셋이다(무엇이 어긋났는가·무엇을 봤는가·어디를 고치는가).
        // 여는 줄에서 자른다 — 닫는 줄(`BODY\n`)로 자르면 heredoc 뒤의 글을 본문으로 읽는다.
        let body = WRITING_EXAMPLE.split("<<'BODY'\n").nth(1).expect("예시 본문이 없다");
        let lines = body.lines().take_while(|l| *l != "BODY");
        assert!(lines.clone().count() >= 3, "예시 본문이 가르친 세 갈래를 다 안 보여 준다");
        for line in lines {
            assert!(line.starts_with("- "), "예시 본문이 목록이 아니다 — {line}");
        }
    }

    /// 규칙의 이름이 스킬에 그대로 선다. 훅의 거절문 쪽은 `hook` 의 시험이 본다.
    #[test]
    fn the_skill_names_each_rule_as_the_hook_does() {
        let skill = skill();
        for n in 1..=RULES.len() {
            let title = format!("**{n}. {}.**", RULES[n - 1]);
            assert!(skill.contains(&title), "스킬에 규칙 {n} 의 이름이 없다 — {title}");
        }
        assert!(skill.contains(&make_review("--parent <the issue>")), "리뷰를 세우는 줄이 갈라졌다");
        for step in REVIEW_STEPS.lines() {
            assert!(skill.contains(step.trim()), "리뷰 걸음이 갈라졌다 — {step}");
            // **줄은 들여쓰기 뒤 곧바로 `moai ` 로 시작한다** — [`close_steps`] 가 그 첫 낱말을
            // 겨눌 트래커의 머리로 바꾼다(moai-j2vp). 설명 글이 앞서는 줄이 하나라도 서면 `-C` 가
            // 그 글 안에 박히고, 아예 없으면 말없이 맨 `moai` 가 남는다.
            assert!(step.trim_start().starts_with("moai "), "리뷰 걸음이 moai 로 안 시작한다 — {step}");
        }
    }

    /// **리뷰 원문을 그대로 붙이라는 글과 64KB 거절문이 한 길을 댄다**(moai-b8aj). 안내는
    /// `note -b -` 로 그대로, 거절문은 "요약하고 원문은 파일로" 라 서로 어긋나 있었다 — 큰 리뷰를
    /// 닫는 일꾼마다 밟는다.
    ///
    /// **상한은 표면마다 모든 자리를 잰다**(리뷰 moai-4u6b.5hl). 두 상수만 재던 판은 규칙 3·참고
    /// 문서·한국어 절차에 손으로 적은 `64KB` 를 안 봐, 상한을 올리면 그 셋이 옛 수를 대는 채로
    /// 초록이었다. 수째 읽어 견준다 — `contains("4KB")` 는 `64KB` 에도 맞는다.
    #[test]
    fn the_review_note_and_the_size_limit_say_one_thing() {
        let limit = crate::model::MAX_TEXT_BYTES / 1024;
        // `<크기>KB` 는 자리 표시라 수가 없다 — 그것만 건너뛰고 적힌 수는 모두 잰다.
        let stated = |text: &str| -> Vec<String> {
            text.match_indices("KB")
                .map(|(at, _)| {
                    let head = &text[..at];
                    head[head.trim_end_matches(|c: char| c.is_ascii_digit()).len()..].to_string()
                })
                .filter(|digits| !digits.is_empty())
                .collect()
        };
        for (surface, text) in
            [("AGENTS 블록", agents()), ("SKILL.md", skill()), ("참고 문서", reference()), ("감독 스킬", supervise())]
        {
            for said in stated(&text) {
                assert_eq!(
                    said.parse::<usize>().ok(),
                    Some(limit),
                    "{surface}: 적힌 상한 {said}KB 가 실제({limit}KB)와 다르다"
                );
            }
        }
        for (what, text) in [("REVIEW_OVER_LIMIT", REVIEW_OVER_LIMIT), ("REVIEW_STEPS", REVIEW_STEPS)] {
            let said = stated(text);
            assert!(
                !said.is_empty() && said.iter().all(|d| d.parse::<usize>().ok() == Some(limit)),
                "{what}: 글의 상한이 실제와 다르다 — {said:?}"
            );
        }
        // 요약 표식도 한 출처여야 한다 — 한국어 절차가 그것을 손으로 옮겨 적는다.
        let mark = "`Summary:";
        assert!(REVIEW_OVER_LIMIT.contains(mark) && KOREAN_DETAIL.contains(mark), "요약 표식이 갈라졌다");
        assert!(rules().contains(REVIEW_OVER_LIMIT), "규칙 3 이 넘칠 때의 길을 안 댄다");
        assert!(reference().contains(REVIEW_OVER_LIMIT), "참고 문서가 넘칠 때의 길을 안 댄다");
        assert!(brief().contains(&indent(REVIEW_OVER_LIMIT, "       ")), "일꾼 브리프가 넘칠 때의 길을 안 댄다");
        // 상한 자체에서 넘는 글을 짓는다 — 손으로 적은 수는 상한이 그것을 넘어서면 `unwrap_err` 가
        // 엉뚱한 패닉으로 터진다. `가` 는 3바이트다.
        let big = "가".repeat(crate::model::MAX_TEXT_BYTES / 3 + 1);
        let refused = crate::model::check_text_size(|| "t-r".into(), "note", &big).unwrap_err().to_string();
        let said = REVIEW_OVER_LIMIT.replace("<size>", &big.len().div_ceil(1024).to_string()).replace('\n', "\n      ");
        assert!(refused.contains(&said), "거절문이 다른 길을 댄다\n{refused}");
        // 거절문은 잰 수를 그대로 내민다 — 자리 표시가 남으면 받는 쪽이 첫 줄을 손으로 채운다.
        assert!(!refused.contains("<size>"), "거절문이 `<size>` 를 안 채웠다\n{refused}");
    }

    /// 포맷 문자열 안의 `{{`·`}}` 가 제대로 풀렸는가. 참고 문서의 파이썬 한 줄이
    /// 딕셔너리를 쓰므로, 한 번 틀리면 복사해 친 명령이 문법 오류로 죽는다.
    ///
    /// **여러 줄 명령은 들여쓰지 않는다.** 4칸 들여쓴 채 복사하면 파이썬 소스 줄이 공백으로
    /// 시작해 `IndentationError` 로 죽고, 파이프 끝의 `moai note` 는 빈 글을 받아 원문을 못 남긴다.
    #[test]
    fn the_python_one_liner_survives_formatting() {
        let reference = reference();
        assert!(reference.contains(".get('message',{}).get('content')"));
        assert!(reference.contains("\npython3 -c \"\nimport json,sys\n"), "리뷰 원문을 꺼내는 파이썬이 들여써졌다");
    }

    /// **넘겨준 보고를 집는다**(리뷰 moai-4u6b.5hl). 서브에이전트가 `SubagentHandback` 으로 보고를
    /// 넘기면 그 뒤에 "보고를 보냈다" 같은 맺음말을 한 줄 더 적는다 — 마지막 `text` 블록만 집던
    /// 한 줄은 그 맺음말을 원문으로 적고도 조용했다(이 기계의 대화록 487개 중 486개가 그 꼴이고,
    /// 리뷰만한 크기의 24개 중 8개가 보고를 넘겨준 판이었다). 빈 글이 아니라 `moai note` 의
    /// "메모가 비었다" 도 안 서고, 넘는지 재는 것도 그 맺음말로 잰다.
    #[test]
    fn the_one_liner_picks_the_handed_back_report() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let reference = reference();
        let head = "python3 -c \"";
        let open = reference.find("\npython3 -c \"\n").expect("리뷰 원문을 꺼내는 한 줄이 없다") + 1 + head.len();
        let script = &reference[open..];
        // 파이썬 안에는 큰따옴표가 없다 — 셸이 `-c "…"` 로 싸는 글이라 그것이 곧 닫는 자다.
        let script = &script[..script.find('"').expect("파이썬이 안 닫힌다")];
        let s = crate::scratch::Scratch::new("handback");
        let line = |v: serde_json::Value| format!("{}\n", serde_json::json!({"message": {"content": v}}));
        let text = |t: &str| serde_json::json!([{"type": "text", "text": t}]);
        let run = |name: &str, body: &str| -> String {
            let path = s.path().join(name);
            std::fs::write(&path, body).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let mut child = Command::new("python3")
                .arg("-")
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("python3 를 실행하지 못했다 — 이 시험에는 python3 가 있어야 한다");
            child.stdin.take().expect("stdin").write_all(script.as_bytes()).expect("스크립트를 못 넘겼다");
            let out = child.wait_with_output().expect("python3 가 안 끝났다");
            assert!(out.status.success(), "{name}: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8_lossy(&out.stdout).trim_end().to_string()
        };
        let report = "리뷰 원문 — 1. src/x.rs:1 어긋났다";
        let handed = format!(
            "{}{}{}",
            line(text("보겠다")),
            line(serde_json::json!([{"type": "tool_use", "name": "SubagentHandback", "input": {"message": report}}])),
            line(text("보고를 넘겼다.")),
        );
        assert_eq!(run("handed.jsonl", &handed), report, "넘겨준 보고 대신 맺음말을 집는다");
        // 넘겨주는 호출이 없는 판은 그대로 마지막 `text` 블록이다 — 그 길을 걷어내지 않는다.
        let plain = format!("{}{}", line(text("보겠다")), line(text(report)));
        assert_eq!(run("plain.jsonl", &plain), report, "넘겨주지 않은 판의 원문을 못 집는다");
    }

    /// 템플릿 문법의 `{{`·`\{{` 도 포맷을 지나 그대로 선다. 한 번 틀려 참고 문서가 `{이름}` 과 `\{` 를
    /// 가르쳤고, 그대로 쓴 템플릿은 변수가 아니라 글자가 됐다.
    #[test]
    fn the_template_syntax_survives_formatting() {
        let reference = reference();
        for want in ["`{{name}}`", "`{{`", "`\\{{`"] {
            assert!(reference.contains(want), "참고 문서에 {want} 가 없다 — 포맷이 중괄호를 깎았다");
        }
        assert!(!reference.contains("`{name}`"), "참고 문서가 `{{{{name}}}}` 을 `{{name}}` 으로 깎았다");
    }

    /// **발동어는 두 말을 함께 싣는다**(moai-54k2 리뷰). 심는 글은 영어지만 이 줄은 산문이 아니라
    /// 짝맞추개다 — 한국어로 묻는 저장소에서 한국어 발동어를 걷으면 스킬이 안 서고, 세션은 이 줄이
    /// 막으려는 TodoWrite 로 돌아간다. 걷혔던 낱말들을 이름째 잰다: 글을 다시 쓸 때 조용히 빠지는
    /// 자리라, 머리만 재던 시험은 발동이 떨어지는 것을 못 봤다.
    #[test]
    fn the_skill_description_keeps_its_korean_triggers() {
        let head =
            |text: &str| text.lines().find(|l| l.starts_with("description: ")).expect("발동어 줄이 없다").to_string();
        for (whose, said, triggers) in [
            (
                "moai",
                head(&skill()),
                ["뭐부터 할까", "할 일 정리", "이슈 만들어", "진행 상황", "이거 나중에 하자"].as_slice(),
            ),
            (
                "moai-supervise",
                head(&supervise()),
                ["감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜"].as_slice(),
            ),
        ] {
            for trigger in triggers {
                assert!(said.contains(trigger), "{whose} 의 발동어에서 {trigger} 가 빠졌다 — {said}");
            }
            // 영어 쪽도 함께 선다 — 한쪽만 남기면 다른 말로 묻는 쪽이 못 부른다.
            assert!(said.contains("Use "), "{whose} 의 발동어에 영어가 없다 — {said}");
            // 발동어 줄은 1,024자까지다. 두 말을 실어도 그 안이어야 한다.
            assert!(said.chars().count() <= 1024, "{whose} 의 발동어가 상한을 넘었다 — {}자", said.chars().count());
        }
    }

    /// 스킬의 frontmatter 는 **첫 줄**에서 시작해야 읽힌다.
    #[test]
    fn the_frontmatter_opens_the_skill() {
        let skill = skill();
        // 글자 단위로 자른다 — 바이트로 자르면 한글 한가운데서 끊겨, 실패를 알리려던
        // 자리가 제가 먼저 죽는다.
        let head: String = skill.chars().take(40).collect();
        assert!(skill.starts_with("---\nname: moai\ndescription: "), "{head}");
        assert!(supervise().starts_with("---\nname: moai-supervise\ndescription: "), "감독 스킬의 머리가 없다");
    }

    /// **감독이 일꾼에게 가르치는 펼치기는 참고 문서의 그 명령이다.** 길이 둘로
    /// 갈라지면 일꾼마다 다르게 고른다 — 첫 실행에서 `add` 로 새 줄을 세운 일꾼이 있었다.
    /// 리뷰 이슈를 세우는 줄은 규칙 3 의 그 조각이라 빼고 센다 — `moai add` 를 통째로
    /// 막던 판은 그 조각을 못 실어, 관점(`-b`) 없는 줄을 손으로 줄여 적었다.
    #[test]
    fn the_supervisor_teaches_promote_as_the_one_way() {
        let (supervise, reference) = (supervise(), reference());
        let promote = "moai idea promote <id> --from -";
        assert!(reference.contains(promote), "참고 문서의 펼치기 줄이 바뀌었다");
        assert!(supervise.contains(promote), "감독이 promote 를 안 가르친다");
        let review = make_review("--parent <epic>");
        assert!(supervise.contains(&review), "에픽 리뷰를 규칙 3 의 줄로 안 세운다");
        assert!(!supervise.replace(&review, "").contains("moai add"), "감독이 promote 말고 다른 길을 가르친다");
        // 되짚기(7-1)의 줄도 promote 이고, 루트의 그 에픽을 가리킨다. `brief.contains(RECALL)` 는
        // 글이 그 상수를 끼워 넣는 한 늘 참이라, 줄의 모양은 여기서 따로 맨다 — `moai idea add` 로
        // 바꿔 7-1 이 거꾸로 idea 로 내보내라고 가르쳐도 다른 시험은 다 초록이었다.
        assert!(
            RECALL.starts_with("moai -C <root> idea promote ") && RECALL.contains(" -e <epic> "),
            "되짚기가 루트의 그 에픽에 promote 로 멤버를 세우지 않는다 — {RECALL}"
        );
        // 되짚기의 자리는 일꾼이 채운다 — 감독이 채우는 목록(3)에 같은 이름이 들면 맡긴 idea 의 값이
        // 그 줄에 미리 박혀 온다(첫 판의 `<제목>` 이 그랬다). `<루트>` 만 감독이 채우라고 둔 자리다.
        let list = slot_list(&supervise);
        let seven = supervise.find("\n    7-1.").expect("되짚기 걸음이 없다");
        let eight = seven + supervise[seven..].find("\n    8.").expect("병합 걸음이 없다");
        let slots = supervise[seven..eight]
            .split('<')
            .skip(1)
            .filter_map(|s| s.split_once('>'))
            .map(|(s, _)| format!("`<{s}>`"));
        for slot in slots.filter(|s| s != "`<root>`") {
            assert!(!list.contains(&slot), "감독이 되짚기의 자리 {slot} 를 채운다 — {list}");
        }
    }

    /// **일꾼이 받는 글(`brief`)에 첫 실행에서 넘어진 자리가 선다.** 감독 스킬의 다른
    /// 절에만 적으면 시험은 초록인데 일꾼은 못 받는다 — `MERGE_HEAD` 가 실제로 그랬다.
    /// 하나라도 빠지면 다음 일꾼이 같은 자리에서 또 넘어진다.
    #[test]
    fn the_worker_brief_carries_what_the_first_run_tripped_on() {
        let (brief, supervise) = (brief(), supervise());
        assert!(supervise.contains(&brief), "감독이 싣는 글이 brief 가 아니다");
        let review = make_review("--parent <epic>");
        for (piece, why) in [
            // **4-1 도 "리뷰 서브에이전트" 를 말한다.** 글자만 보면 7 의 길이 통째로 빠져도
            // 초록이라, 7 의 그 줄에만 있는 앞말까지 매어 찾는다.
            ("run a review subagent with the same angle", "워크트리에서 /code-review 가 막힐 때의 길이 없다"),
            ("reads that\n       worktree's snapshot only", "막히는 까닭이 없어 다른 거절까지 돌아간다"),
            ("only once that merge has really", "병합 전에 done 으로 옮기지 말라는 말이 없다"),
            ("Do not review member by member", "멤버 리뷰를 걷었다는 말이 없어 일꾼이 멤버마다 리뷰한다"),
            ("`low`·`medium`·`high`", "멤버를 잴 난이도의 폭이 없다"),
            ("/code-review <grade> --fix", "에픽 끝의 xhigh·max 리뷰가 없다"),
            ("in one line in the angle (`-b`)", "고른 등급의 까닭을 남기라는 말이 없다"),
            (review.as_str(), "에픽 리뷰 이슈를 관점과 함께 에픽에 매는 줄이 없다"),
            ("The tracker you edit is always the root's", "워크트리의 트래커를 고쳐 병합에서 스냅샷이 충돌한다"),
            ("`MOAI_HERE`", "옮김을 끄는 손잡이를 켜지 말라는 말이 없다"),
            ("review subagent", "서브에이전트가 워크트리의 .moai 를 고친다"),
            ("ExitWorktree(keep)", "루트로 돌아오는 걸음이 없다"),
            ("MERGE_HEAD", "남이 열어 둔 병합을 봉인하지 말라는 말이 없다"),
            ("-- .moai/", "트래커 커밋이 열린 병합을 봉인한다"),
            ("Do not use `--no-commit`", "병합을 한 번에 끝내라는 말이 없다"),
            ("git merge --no-ff", "fast-forward 로 끝나 병합 커밋이 안 선다"),
            ("git merge --abort", "루트 병합이 충돌할 때의 길이 없다"),
            ("the unfolded epic's id", "보고에 에픽이 없어 감독이 멤버를 못 본다"),
        ] {
            assert!(brief.contains(piece), "{why} — {piece}");
        }
        // 리뷰는 무엇이 나왔는지를 남기며 닫는다. 워크트리가 남아 있으면 훅이 그 에픽을 옆의
        // 일로 읽어 `-m` 없는 닫기를 못 막으니, 닫기는 워크트리를 지운 뒤다.
        for step in close_steps("<review id>", "moai").lines() {
            assert!(brief.contains(step.trim()), "리뷰 닫기 걸음이 갈라졌다 — {step}");
        }
        let removed = brief.find("git worktree remove").expect("워크트리를 지우는 걸음이 없다");
        let closed = brief.find("moai mv <review id> done").expect("리뷰를 닫는 걸음이 없다");
        assert!(removed < closed, "워크트리를 지우기 전에 리뷰를 닫는다");
        // 에픽 리뷰는 본 가지를 받은 뒤다 — 먼저 보면 충돌을 푼 자리가 리뷰 없이 본 가지에 선다.
        let synced = brief.find("pull <base branch> into the worktree").expect("본 가지를 받는 걸음이 없다");
        let reviewed = brief.find("/code-review <grade> --fix").expect("에픽 리뷰 걸음이 없다");
        assert!(synced < reviewed, "본 가지를 받기 전에 에픽 전체를 리뷰한다");
        // 되짚기는 에픽 리뷰 뒤·병합 앞이다 — 앞에 두면 그 리뷰가 넘긴 것을 못 보고, 뒤에 두면
        // 에픽이 이미 닫혔다. 없으면 에픽이 내건 것이 idea 로 빠진 채 닫힌다(moai-l288).
        let recalled = brief.find("7-1. Before merging").expect("병합 전에 idea 를 되짚는 걸음이 없다");
        let merged = brief.find("merge --no-ff").expect("병합 걸음이 없다");
        assert!(reviewed < recalled && recalled < merged, "되짚기가 에픽 리뷰 뒤·병합 앞이 아니다");
        assert!(brief.contains(RECALL), "되짚은 것을 멤버로 세우는 줄이 없다");
        assert!(brief.contains("already done"), "누가 펼친 idea 를 또 펼쳐 에픽이 둘 선다");
        for (piece, why) in [
            // promote 는 닫힌 idea 도 또 펼친다 — 8 에서 돌아와 다시 도는 7-1 이 같은 멤버를 둘 세운다.
            ("If that idea is already done, do\n       not unfold it", "다시 도는 되짚기가 닫힌 idea 를 또 펼친다"),
            // 되짚을 것을 창의 기억에만 두면 창을 비우거나 일을 이어받은 창이 아무것도 못 찾는다.
            ("Put `-e <epic>` on an idea you park mid-epic", "도중 담는 idea 에 에픽을 안 달아 7-1 이 되찾지 못한다"),
            ("show --type idea -e <epic>", "도중 담은 idea 를 트래커에서 찾는 길이 없다"),
            // 7-1 이 첫 칸에 남긴 멤버는 아무도 안 했다 — 9-1 이 그 멤버에 모델 줄을 적거나 10 이
            // 그 멤버를 닫으면 통계가 거짓이 되거나 에픽이 목적을 못 이룬 채 닫힌다.
            ("which nobody did", "아무도 안 한 멤버에 일한 모델을 남긴다"),
            (
                "members left in the first column by 7-1 and 4-3 — those members keep the epic open",
                "남긴 멤버를 10 에서 닫아 에픽이 목적을 못 이룬 채 닫힌다",
            ),
        ] {
            assert!(brief.contains(piece), "{why} — {piece}");
        }
        for (piece, why) in [
            ("A session that refused the work", "맡기기를 거절한 세션을 빼라는 말이 없다"),
            ("in this same round against each other", "같은 바퀴에 보낸 둘이 같은 곳을 고친다"),
            ("comes out of the candidates until its report is checked", "보낸 idea 가 둘째 일꾼에게 또 간다"),
            ("That member is the worker's", "감독이 훅에 떠밀려 일꾼의 멤버를 옮긴다"),
            ("only sessions on this\n  machine", "원격 세션에 루트를 묻는다"),
            ("whose sent idea has not had its report checked", "맡긴 일을 하던 세션에 또 맡긴다"),
            ("moai show <epic>", "idea 로 확인하면 멤버가 안 보인다"),
            ("and `<root>`.", "감독이 루트 자리를 안 채워 일꾼이 제 워크트리를 루트로 읽는다"),
            // 거둔 일을 맡기는 글은 brief 의 일부만 잇는다 — 그 범위가 4-1 위에서 끊기면
            // 이어받은 일꾼만 워크트리의 `.moai` 를 고친다. **끝은 번호로 적지 않는다**:
            // `11 까지` 로 적어 둔 뒤 12 가 붙자 이어받은 일꾼만 12 를 못 받았다.
            ("of 3 whole, **from 4-1 to the end**", "거둔 일을 맡기는 글이 4-1 을 빼거나 끝을 자른다"),
            // 7-1 이 첫 칸에 남긴 멤버는 에픽을 연 채 둔다 — 감독의 확인(5)이 그것을 어긋남으로 읽으면
            // 시킨 대로 한 보고마다 그 창이 안 비워지고 다음 idea 도 못 받는다.
            (
                "A member the report says was left in the first column by brief 7-1",
                "감독이 일부러 남긴 멤버를 어긋난 보고로 읽는다",
            ),
        ] {
            assert!(supervise.contains(piece), "{why} — {piece}");
        }
        // 거둔 일을 맡기는 글의 모델 줄은 새 일의 그 줄과 같은 조각이다 — 손으로 두 벌 적던
        // 판은 거둔 쪽만 일꾼에게 없는 감독의 절(2-1)을 가리켰고, 지워도 초록이었다.
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];
        assert!(head.contains(&indent(&model_line(), "      ")), "거둔 일을 맡기는 글에 모델 줄이 없다");
    }

    /// **마일스톤 우선 규칙은 글이 유일한 자리다**(moai-s526, 2026-09-20 사용자 결정).
    ///
    /// 도구는 이것을 **막지 않는다** — `ready` 의 차례와 보드의 알림뿐이라, 규칙을 아는 길은
    /// 세 글(AGENTS 블록·감독 1·일꾼 브리프)밖에 없다. 한 곳에만 적으면 그 글을 안 읽는 쪽이
    /// 규칙을 모르는 채 밖의 일을 집고, 도구는 그것을 그대로 지나 보낸다.
    ///
    /// **"막지 않는다" 를 함께 맨다.** 그 줄이 빠지면 다음 사람이 훅에 규칙을 심어 게이트로
    /// 만든다 — 옛 moai 가 죽은 자리다.
    #[test]
    fn the_milestone_first_rule_stands_in_all_three_teachings() {
        let (agents, supervise, brief) = (agents(), supervise(), brief());
        // 감독 쪽은 **브리프 앞에서만** 잰다 — 감독 스킬은 브리프를 품고 있다.
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];

        for (piece, why) in [
            ("While a milestone is running, what is inside it comes first", "AGENTS 블록에 규칙이 없다"),
            ("if even one member stands in a started\ncolumn, it is running", "무엇으로 시작을 재는지 안 적었다"),
            ("**`p0` gets picked up whether or not it is in the milestone**", "핫픽스 자리를 안 적었다"),
            (
                "**Nothing is blocked.** A `moai mv` that picks up work from outside goes straight\n  through",
                "막지 않는다는 것을 안 적었다",
            ),
        ] {
            assert!(agents.contains(piece), "{why} — {piece}");
        }
        for (piece, why) in [
            ("If a milestone is running, what is inside it comes first", "감독이 고르는 자리에 규칙이 없다"),
            ("The tool does not block this", "감독이 도구가 막아 줄 것으로 읽는다"),
        ] {
            assert!(head.contains(piece), "{why} — {piece}");
        }
        // 일꾼 쪽은 배포 기준으로 가르는 걸음이다(moai-589b) — 리뷰가 넘긴 것을 안팎에 둔다.
        for (piece, why) in [
            ("7-2.", "넘긴 것을 가르는 걸음이 없다"),
            ("--milestone none", "밖으로 보내는 길을 안 댄다"),
            ("does a `#bug` tag fit", "버그 수준을 있는 낱말로 안 잰다"),
            ("can a `Regression-of:` line be written", "되돌림을 안 센다"),
        ] {
            assert!(brief.contains(piece), "{why} — {piece}");
        }
    }

    /// **밖의 idea 는 마일스톤을 달아야 들어온다**(moai-6qgz, 2026-09-21).
    ///
    /// 앞 시험이 매는 "안의 것이 먼저다" 는 감독이 **무엇을 고를지**만 정한다. 고른 것이 밖의
    /// idea 일 때 그것을 안으로 들이는 길은 아무 데도 없었고, 그래서 감독이 브리프에 "마일스톤은
    /// 달지 마라" 고 적어 일꾼이 도는 판 밖의 일을 집었다. 도구는 그것을 그대로 지나 보낸다.
    ///
    /// **두 글이 한 줄에 매인다.** 감독의 1 은 들일지를 정하고 일꾼의 1 이 실제로 단다 —
    /// `MILESTONE_ATTACH` 하나에서 둘 다 나오므로, 한쪽만 고치면 여기서 붉어진다.
    #[test]
    fn an_idea_from_outside_comes_in_on_a_milestone() {
        let (supervise, brief) = (supervise(), brief());
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];

        for (piece, why) in [
            (MILESTONE_ATTACH, "감독이 들이는 길을 안 가리킨다"),
            ("**An idea from outside gets in only by being brought in.**", "밖의 idea 를 들이는 걸음이 없다"),
            (
                "**Telling the worker not to attach a milestone is the same as handing out work
from outside**",
                "마일스톤을 빼라고 적는 것이 밖의 일을 맡기는 것과 같다는 말이 없다",
            ),
            (
                "**Leave it unfilled** and the worker hangs the placeholder itself",
                "안 채운 자리가 무엇이 되는지 안 적었다",
            ),
        ] {
            assert!(head.contains(piece), "{why} — {piece}");
        }
        // **자리 이름은 목록 줄에서 찾는다** — 바로 아래 풀이 글도 `<milestone>` 를 적어, 감독 쪽
        // 전체에서 찾으면 목록에서 빠져도 초록이다. 줄의 모양이 바뀌어도 `slot_list` 하나만 따른다.
        let list = slot_list(head);
        assert!(list.contains("`<milestone>`"), "3 의 채우는 자리에 마일스톤이 없다 — {list}");

        // 일꾼 쪽은 **1 안에서** 잰다 — 펼치기와 같은 걸음이라야 워크트리가 서기 전에 달린다.
        let one = brief.find("\n    1.").expect("일꾼 글에 1 이 없다");
        // 2 는 **1 뒤에서** 찾는다 — 앞에서 찾으면 걸음 앞에 `\n    2.` 로 읽히는 줄이 하나 드는
        // 날 슬라이스가 거꾸로 서서 시험이 패닉으로 죽는다(옆 시험들이 쓰는 걸음과 같은 꼴이다).
        let two = one + brief[one..].find("\n    2.").expect("일꾼 글에 2 가 없다");
        for (piece, why) in [
            (MILESTONE_ATTACH, "일꾼이 마일스톤을 다는 줄이 1 에 없다"),
            ("If `<milestone>` is `none`", "아무것도 안 도는 판을 안 적었다"),
        ] {
            assert!(brief[one..two].contains(piece), "{why} — {piece}");
        }
    }

    /// **난이도 한 낱말이 모델과 리뷰 등급을 함께 정한다**(moai-84kd, 2026-09-15 사용자 결정).
    ///
    /// 축을 따로 두면 브리프가 판단을 두 벌 들고, 둘이 어긋나는 날 싼 모델이 쓰기 경로를 맡는다.
    /// 그래서 짝은 리뷰 등급과 같은 낱말 위에 선다 — low·medium·high 가 그대로 haiku·sonnet·opus 다.
    /// **브리프에 실려야 뜻이 있다**: 감독 스킬에만 적힌 규칙은 일꾼이 받는 글에 없다(`brief`).
    #[test]
    fn the_supervisor_picks_a_model_by_difficulty() {
        let (supervise, brief) = (supervise(), brief());
        // 감독 쪽은 **브리프 앞에서만** 잰다 — 감독 스킬은 브리프를 품고 있어, 통째로 재면
        // 브리프에 든 같은 글이 감독 쪽에서 빠진 자리를 메운다(표의 잣대가 실제로 그랬다).
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];
        // **잣대는 한 벌이다.** 표도 일꾼의 잣대도 `DIFFICULTY` 에서 나온다 — 한쪽을 손으로
        // 다시 적으면 여기서 붉어진다.
        let table = difficulty_table();
        assert!(head.contains(&table), "2-1 의 표가 DIFFICULTY 에서 안 나온다");
        assert!(
            brief.contains(&indent(&difficulty_rubric(), "       ")),
            "일꾼이 받는 잣대가 DIFFICULTY 에서 안 나온다"
        );
        // **칸째로 맨다** — 머리의 `모델` 칸 자리에 짝이 서는지 본다. 줄 어디에 `` `haiku` ``
        // 가 들기만 하면 되던 판은 모델 칸과 리뷰 칸을 바꿔 끼워도 초록이었다.
        let cells = |l: &str| l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect::<Vec<_>>();
        let mut lines = table.lines();
        let header = cells(lines.next().expect("표에 머리가 없다"));
        let col =
            |name: &str| header.iter().position(|c| c == name).unwrap_or_else(|| panic!("표 머리에 {name} 칸이 없다"));
        let (level_col, model_col) = (col("Level"), col("Model"));
        let rows: Vec<Vec<String>> = lines.skip(1).map(cells).collect();
        for (level, model) in [("low", "haiku"), ("medium", "sonnet"), ("high", "opus")] {
            let row = rows
                .iter()
                .find(|r| r.get(level_col) == Some(&format!("`{level}`")))
                .unwrap_or_else(|| panic!("난이도 {level} 의 줄이 표에 없다"));
            assert_eq!(row.get(model_col), Some(&format!("`{model}`")), "{level} 의 짝이 {model} 이 아니다 — {row:?}");
        }
        // 에픽 끝은 그 에픽의 유일한 리뷰다. 등급은 가장 무거운 멤버의 한 칸 위, 모델은 그 등급을
        // 따른다(moai-bx6t) — 감독의 2-1 에 서고 **그 리뷰를 부르는 일꾼에게도 같은 글로 선다.**
        // 감독의 2-1 에만 적혀 있던 판은 그 리뷰가 도는 창에 한 번도 안 닿았다(리뷰 에이전트는
        // 창의 모델을 물려받는다). 첫 판은 "늘 opus" 를 붙들어, 글 한 줄 고친 에픽도 가장 비싼
        // 리뷰를 받았다 — 이제 가운데 모델로 보는 자리(`medium`)가 두 글 모두에 서는지 본다.
        let rule = epic_review_rule();
        assert!(
            rule.contains("`medium` means `sonnet`") && rule.contains("`high` and up means `opus`"),
            "에픽 끝 리뷰의 모델이 등급을 안 따른다 — {rule}"
        );
        assert!(head.contains(&rule), "감독의 2-1 에 에픽 끝 등급·모델 규칙이 없다");
        let epic = brief.find("/code-review <grade> --fix").expect("에픽 리뷰 걸음이 없다");
        let step = &brief[epic..brief[epic..].find("\n    8.").map_or(brief.len(), |n| epic + n)];
        assert!(step.contains(&indent(&rule, "       ")), "브리프 7 에 에픽 끝 등급·모델 규칙이 없다");
        assert!(step.contains("`/model opus`"), "에픽 끝 리뷰의 모델을 맞추라는 말이 일꾼에게 없다");
        // 훅에 막혀 돌리는 리뷰 서브에이전트도 창의 모델을 물려받는다 — 창을 못 맞췄으면 싼 모델이
        // 쓰기 경로를 본다. 서브에이전트에는 모델을 직접 준다.
        assert!(step.contains("`Agent`'s `model`"), "리뷰 서브에이전트의 모델을 안 준다");
        // 일꾼이 받는 글에 그 자리가 있어야 감독이 채운다. **목록 줄에서 찾는다** — 바로 아래
        // 풀이 글도 세 자리를 적어, 감독 쪽 전체에서 찾으면 목록에서 빠져도 초록이었다.
        assert!(brief.contains("Model:"), "브리프에 모델 자리가 없다 — 감독이 골라도 일꾼은 모른다");
        let list = slot_list(head);
        for slot in ["<model>", "<difficulty>", "<why>"] {
            assert!(
                list.contains(&format!("`{slot}`")),
                "감독이 채울 자리 목록에 {slot} 가 없다 — 그대로 실려 노트에 자리표시자가 남는다"
            );
        }
        // **감독이 채우는 자리와 일꾼이 고르는 자리는 이름이 다르다.** 7 의 `/code-review <등급>`
        // 는 개발해 본 일꾼이 고르는 자리다 — 목록에 같은 이름이 들면 읽기 전의 제안이 그 명령에
        // 미리 박힌다.
        assert!(brief.contains("/code-review <grade> --fix"), "일꾼이 고르는 리뷰 등급 자리가 없다");
        assert!(!list.contains("`<grade>`"), "감독이 일꾼의 리뷰 등급 자리를 채운다 — {list}");
        // **멤버는 따로 리뷰하지 않는다**(moai-bx6t, 2026-09-18 사용자 결정). 멤버마다
        // `/code-review` 를 부르던 판으로 돌아가면 리뷰가 두 벌 돌아 시간과 토큰이 곱으로 든다 —
        // 브리프에 에픽 끝 말고 다른 `/code-review` 가 서면 붉어진다.
        assert_eq!(
            brief.matches("/code-review").count(),
            1,
            "브리프에 에픽 끝 말고도 리뷰가 선다 — 멤버마다 보던 판이 돌아왔다"
        );
        assert!(brief.contains("Do not review member by member"), "브리프 5 가 멤버 리뷰를 걷었다고 말하지 않는다");
    }

    /// **잣대의 글은 이 저장소 CLAUDE.md 의 리뷰 표와 같다.** 같은 축이라고 적어 두고 high 에서
    /// `동시성` 이 빠져 있었다 — 한 파일 안의 동시성 고침이 medium 으로 읽혀, 이 도구가 못 견디는
    /// 조용한 손실 쪽에 싼 모델과 싼 리뷰가 붙는다. 표를 옮겨 적은 두 자리는 여기서 견준다.
    #[test]
    fn the_rubric_is_the_review_table() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("CLAUDE.md");
        let claude = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (level, _, how) in DIFFICULTY {
            let row = format!("| `{level}` | {how} |");
            assert!(claude.contains(&row), "CLAUDE.md 의 리뷰 표와 {level} 의 잣대가 갈라졌다 — {row}");
        }
        // 에픽 끝을 `max` 로 올리는 멤버도 두 자리가 같은 글이다 — 멤버를 따로 안 보니
        // 여기서 `동시성` 이 빠지면 그 멤버는 한 번도 비싼 눈을 안 받는다.
        assert!(claude.contains(EPIC_MAX), "CLAUDE.md 의 에픽 끝 max 줄이 브리프와 갈라졌다 — {EPIC_MAX}");
        assert!(brief().contains(EPIC_MAX), "브리프 7 이 에픽 끝 max 줄을 안 쓴다 — {EPIC_MAX}");
    }

    /// **일꾼이 창을 비워도 되는 때를 알린다**(moai-gu5g, 2026-09-15 사용자 결정).
    ///
    /// 맥락은 대화가 아니라 트래커에 산다 — 이슈 본문·노트·리뷰 원문·커밋 메시지. 머지가 들고
    /// 보고가 나간 자리에서는 지워도 잃을 것이 없다. **반대도 함께 말한다**: 리뷰가 도는 중·
    /// 충돌을 푸는 중·사람의 답을 기다리는 중에 지우면 아직 트래커에 안 옮긴 것이 사라진다.
    #[test]
    fn the_brief_says_when_the_pane_can_be_cleared() {
        let brief = brief();
        let at = brief.find("/clear").expect("창을 비워도 되는 때를 안 알린다");
        // **보고 뒤다** — 보고 전에 지우면 보고에 담을 것이 대화에만 있던 채로 사라진다.
        let report = brief.find("SendMessage to").expect("보고 걸음이 없다");
        assert!(at > report, "보고보다 먼저 비우라고 한다");
        // **남긴 줄은 담은 뒤에 비운다.** 이어받을 줄은 10 의 트래커 커밋 뒤에 적으므로, 담지
        // 않으면 공유 루트의 `.moai` 가 더러운 채로 남아 남의 커밋에 쓸려 들어간다.
        let step = brief.rfind("\n    12.").expect("창을 비우는 걸음이 없다");
        assert!(brief[step..at].contains("a commit with a path"), "비우라고 하기 전에 남긴 줄을 안 담는다");
        // **까닭과 반대를 갈라 찾는다** — 둘이 서로의 낱말(`트래커`·`리뷰`)을 품어, 한 덩어리로
        // 찾던 판은 어느 한쪽을 지워도 초록이었다.
        let (reason, against) =
            brief[at..].split_once("Say the opposite").expect("지우지 말 때를 같은 줄에서 안 말한다");
        assert!(reason.contains("lives in the tracker"), "왜 지워도 되는지가 없다");
        for (piece, missing) in [
            ("a review is running", "리뷰가 도는 중에는 지우지 말라는 말이 없다"),
            ("conflict", "머지 충돌을 푸는 중을 안 가린다"),
            ("waiting on a person", "답을 기다리는 중을 안 가린다"),
            // 감독은 보고를 확인하면 같은 창에 다음 글을 보낸다 — 그 뒤의 `/clear` 는 그 글을 지운다.
            ("next message", "감독의 다음 글이 온 뒤에는 지우지 말라는 말이 없다"),
        ] {
            assert!(against.contains(piece), "{missing}");
        }
    }

    /// **tmux 면 감독이 일꾼의 창을 비운다**(2026-09-18 사용자 결정 — 곧바로 친다, 치던 글은
    /// 지우고 친다, 상태줄과 감독 창에 한 줄씩).
    ///
    /// 비우기는 그 일꾼이 쥔 유일한 대화를 지운다 — 그래서 울타리가 전부 **치기 전에** 서야 한다.
    /// 순서를 매는 것은 그 까닭이다: 확인 뒤에 부르고, 거르는 것은 모두 `/clear` 보다 앞이다.
    #[test]
    fn the_supervisor_clears_a_tmux_pane_only_behind_its_fences() {
        let supervise = supervise();
        let at = supervise.find("**5-1. On tmux").expect("감독이 창을 비우는 걸음이 없다");
        let checks = supervise.find("git merge-base --is-ancestor <merge hash>").expect("보고 확인이 없다");
        assert!(checks < at, "보고를 확인하기 전에 창을 비운다");
        let open = at + supervise[at..].find("python3 - '<session>'").expect("비우는 스크립트가 없다");
        let script = &supervise[open..open + supervise[open..].find("\nPY\n").expect("스크립트가 안 닫힌다")];
        let send = script.find("\"-l\", \"/clear\"").expect("/clear 를 안 친다");
        for (fence, why) in [
            ("os.environ.get(\"TMUX\")", "tmux 밖에서도 친다"),
            ("except OSError:\n    skip(\"no tmux\")", "tmux 가 없으면 오류로 죽는다"),
            ("TMUX_PANE", "감독이 제 창을 비운다"),
            ("!= \"idle\"", "idle 이 아닌 판에 친다"),
            ("no longer idle", "치기 직전에 다시 안 본다"),
            ("not in the root", "아직 워크트리에서 일하는 창을 비운다"),
            ("pane_pid", "낡은 세션 파일이 가리키는 남의 판을 비운다"),
            ("pane_in_mode", "사람이 복사 모드로 스크롤해 읽는 판에 친다 — `/` 가 검색을 연다"),
            ("pane_synchronized", "묶인 판에 쳐 옆 일꾼의 대화까지 지운다"),
            ("could not empty the input box", "치던 글 뒤에 /clear 가 붙어 프롬프트로 간다"),
            // 부르는 자리로 찾는다 — `dim_only(pane)` 만 찾으면 `def` 줄이 먼저 걸려, 부르는 줄을
            // 지워도 초록이다.
            ("and dim_only(pane)", "흐린 제안 글에 막혀 창이 영영 안 비워진다"),
            ("erased = rest != kept", "하나도 안 지운 글을 이미 지웠다고 해 사람에게 한 벌 더 돌려준다"),
            // 한 줄만 지우고 멈추면 나머지는 아직 그 칸에 있다 — 통째로 돌려주면 두 벌이 된다.
            ("def gone(kept, left)", "지우다 멈췄을 때 칸에 남은 줄까지 돌려줘 같은 줄이 두 벌 선다"),
            ("def grey(code)", "참색(`38;2;…`)으로 그린 흐린 글을 못 알아봐 창이 안 비워진다"),
            (
                "bare(l).startswith(PROMPT)",
                "사람이 친 글 속의 프롬프트 표시나 붙임표를 제안 글로 읽어 그 글 뒤에 /clear 가 붙는다",
            ),
            ("l[:2] in head", "옮긴 치던 글이 들여쓰기를 잃거나 앞머리 아닌 줄까지 두 글자 깎인다"),
            ("left is None", "입력 칸을 놓친 화면에 지우는 키를 계속 친다"),
            // 마지막으로 읽은 뒤·치기 전의 틈에 친 글 뒤에도 `/clear` 가 붙는다.
            ("text appeared in the input box meanwhile", "지운 뒤 치기 전에 사람이 친 글 뒤에 /clear 가 붙는다"),
            ("the draft is already erased", "지우다 멈추면 사람의 글이 말없이 사라진다"),
            ("list-clients", "상태줄의 한 줄이 일꾼의 판이 아니라 감독의 클라이언트에 뜬다"),
            ("print(kept)", "치던 글을 지우기 전에 감독 창에 안 옮긴다"),
            ("copied into the supervisor's window", "치던 글이 어디 갔는지 사람에게 안 알린다"),
            ("display-message", "판을 보는 사람에게 무엇을 왜 비우는지 안 알린다"),
        ] {
            let pos = script.find(fence).unwrap_or_else(|| panic!("{why} — {fence}"));
            assert!(pos < send, "{why} — /clear 뒤에 거른다: {fence}");
        }
        // 못 치면 **조용히** 넘어간다 — 오류가 아니라 한 줄과 0 이다. tmux 없는 사람의 화면에
        // 오류가 뜨면 안 된다.
        assert!(script.contains("sys.exit(0)"), "건너뛸 때 0 으로 안 끝난다");
        assert!(!script.contains("sys.exit(1)"), "건너뛰기가 실패로 끝난다");
        // **비우기와 다음 배정을 한 호흡에 하지 않는다** — `/clear` 는 큐의 글을 함께 지운다.
        // 세션 id 가 바뀐 것을 보고서야 `비웠다` 를 내고, 글은 그 뒤에 보낸다.
        assert!(script[send..].contains("sessionId"), "비워졌는지를 안 본다");
        // 비운 뒤에는 **찾은 그 파일**을 다시 읽는다 — 이름으로 다시 찾으면 `/clear` 뒤에 이름이
        // 바뀌거나 같은 이름이 하나 더 서는 순간 `비웠다` 를 영영 못 낸다.
        assert!(
            script[send..].contains("read(f)") && !script[send..].contains("session()"),
            "비운 뒤에 이름으로 다시 찾는다"
        );
        let rest = &supervise[open..];
        assert!(rest.contains("the script prints `cleared`"), "비운 뒤에 다음 idea 를 보내라는 말이 없다");
        // 일꾼 쪽: 감독은 `Next:` 노트를 보고 친다 — 그 노트가 일꾼의 마지막 걸음이어야 한다.
        assert!(supervise[at..open].contains("`Next:` note"), "감독이 12 를 마쳤는지 안 본다");
        let brief = brief();
        let twelve = brief.rfind("\n    12.").expect("창을 비우는 걸음이 없다");
        assert!(
            brief[twelve..].contains("the supervisor may check the report"),
            "일꾼이 감독이 비울 수 있다는 것을 모른다"
        );
    }

    #[test]
    fn erasing_stops_when_the_person_types_between_the_strokes() {
        // **지우는 사이에 사람이 친 글은 옮긴 적이 없다**(2026-09-18 사용자 결정). 한 번 읽고 스무
        // 번 지우던 판은 그 4초 남짓에 친 글자를 옮기지 않고 지웠다. 지우기가 깎은 글과 새 글을
        // `shrunk` 가 가르는지 실제 파이썬으로 돌려 본다.
        const CASES: &str = r##"
import sys
CASES = [
    ("첫 줄이 비고 다음 줄이 올라왔다", "b\nc", "a\nb\nc", True),
    ("다 지웠다", "", "a\nb", True),
    ("안 지워지는 제안 글", "Try it", "Try it", True),
    ("줄 끝에 더 쳤다", "b!\nc", "a\nb\nc", False),
    ("새 줄을 쳤다", "b\nc\nd", "a\nb\nc", False),
    ("한 줄뿐인 칸에 더 쳤다", "hello there", "hello", False),
    ("방금 비운 줄이 아직 빈 채로 섰다", "a\n\nc", "a\nb\nc", True),
    # 줄 안에 드는지만 보던 판은 이 둘을 깎인 글로 읽어 옮기지 않고 지웠다.
    ("다 지운 칸에 새로 친 한 글자", "o", "hello", False),
    ("사라진 글을 다시 치는 앞머리", "hel", "hello", False),
]
bad = [why for why, now, was, want in CASES if shrunk(now, was) != want]
print("\n".join(bad))
sys.exit(1 if bad else 0)
"##;
        use std::io::Write;
        use std::process::{Command, Stdio};
        let supervise = supervise();
        let open = supervise.find("python3 - '<session>'").expect("비우는 스크립트가 없다");
        let script = &supervise[open..open + supervise[open..].find("\nPY\n").expect("스크립트가 안 닫힌다")];
        let from = script.find("def shrunk(").expect("지우기가 깎은 글을 가르는 함수가 없다");
        let to = from + script[from..].find("\nQUIET = ").expect("shrunk 뒤의 줄이 없다");
        let loop_at = script.find("for _ in range(20):").expect("지우는 고리가 없다");
        assert!(script[loop_at..].contains("if not shrunk(left, seen):"), "지우는 고리가 사람이 친 글을 안 본다");
        // 사람의 글을 다 지운 빈 칸에 다시 선 흐린 제안 글은 친 글이 아니다 — 그것에 멈추면 "제안 글이
        // 다시 서도 그대로 친다"(사용자 결정)가 죽는다. 흐린 글은 옮겨 두고 지워 보아 가른다.
        let stop = loop_at + script[loop_at..].find("if not shrunk(left, seen):").unwrap();
        let fresh = &script[stop..stop + script[stop..].find("\n    seen = left").expect("고리가 본 글을 안 넘긴다")];
        assert!(fresh.contains("if not dim_only(pane):"), "다시 선 제안 글을 사람이 친 글로 읽어 창이 안 비워진다");
        assert!(fresh.contains("ghost = left"), "흐린 글을 지워 보지 않고 색으로만 가른다");
        assert!(
            script[loop_at..].contains("if ghost is not None and left != ghost:"),
            "지워진 흐린 글을 사람의 글로 안 본다"
        );
        let program = format!("{}{CASES}", &script[from..to]);
        let mut child = Command::new("python3")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 를 실행하지 못했다 — 이 시험에는 python3 가 있어야 한다");
        child.stdin.take().expect("stdin").write_all(program.as_bytes()).expect("스크립트를 못 넘겼다");
        let out = child.wait_with_output().expect("python3 가 안 끝났다");
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "지우기가 깎은 글과 사람이 친 글을 못 가른다:\n{said}");
    }

    /// **흐린 제안 글은 tmux 가 실제로 내보내는 모양으로 가른다.** 위의 울타리는 함수가 있는지만
    /// 보고 무엇을 읽는지는 못 본다. Claude Code(2.1.276)는 빈 칸의 커서를 제안 글 첫 글자에
    /// `\x1b[7m` 으로 그리고 tmux 는 그 다음을 `0;2` 한 조각으로 내보내, 제안 글을 한 번도 못
    /// 알아봤다. 거꾸로 밝은 테마의 사람 글(`38;2;0;0;0`)과 흐린 글 뒤의 `0;1` 은 흐린 글로 읽혔다.
    /// 화면은 `capture-pane -e` 가 내는 모양 그대로 적고, 맨 캡처는 거기서 색만 뺀 것이다.
    #[test]
    fn the_supervisor_reads_the_dim_suggestion_as_tmux_captures_it() {
        const HEAD: &str = r##"import re, sys
SCREEN = [""]
class Captured:
    def __init__(self, stdout):
        self.stdout = stdout
def tmux(*args):
    shown = SCREEN[0]
    return Captured(shown if "-e" in args else re.sub("\x1b\\[[0-9;:]*m", "", shown))
"##;
        const CASES: &str = r##"
E = "\x1b"
RULE = E + "[38;5;244m" + "─" * 8 + E + "[39m"
def box(*rows):
    return "\n".join(["지난 대화", RULE] + list(rows) + [RULE, "  ? for shortcuts", ""])
CASES = [
    ("제안 글 — 첫 글자에 뒤집힌 커서, 이어서 0;2", box("❯ " + E + "[7mT" + E + "[0;2mry it" + E + "[0m"), True, "Try it"),
    ("흐림 뒤에 글자색", box("❯ " + E + "[2m" + E + "[37mTry it" + E + "[0m"), True, "Try it"),
    ("흐림과 기울임이 한 조각", box("❯ " + E + "[2;3mTry it" + E + "[0m"), True, "Try it"),
    ("참색 회색", box("❯ " + E + "[38;2;136;136;136mTry it" + E + "[39m"), True, "Try it"),
    ("접힌 제안 글의 둘째 줄은 색 조각 없이 온다", box("❯ " + E + "[2mTry this", "  and that" + E + "[0m"), True, "Try this\nand that"),
    ("사람의 글", box("❯ hello"), False, "hello"),
    ("밝은 테마의 사람 글", box("❯ " + E + "[38;2;0;0;0mhello" + E + "[39m"), False, "hello"),
    ("흐린 글 뒤의 굵은 사람 글", box("❯ " + E + "[2mx" + E + "[0;1mBOLD" + E + "[0m"), False, "xBOLD"),
    ("첫 줄에 프롬프트 표시만 친 사람 글", box("❯ ❯❯"), False, "❯❯"),
    ("커서가 맨 앞에 선 사람 글", box("❯ " + E + "[7mh" + E + "[0mello"), False, "hello"),
    ("커서 한 칸뿐인 사람 글", box("❯ " + E + "[7mx" + E + "[0m"), False, "x"),
    ("흐린 첫 줄 아래의 사람 글", box("❯ " + E + "[2mTry" + E + "[0m", "  human"), False, "Try\nhuman"),
    ("빈 칸", box("❯"), False, ""),
    ("들여쓴 사람 글", box("❯ def f():", "      return 1"), False, "def f():\n    return 1"),
]
bad = []
for why, shown, dim, text in CASES:
    SCREEN[0] = shown
    got = (dim_only("%1"), draft("%1"))
    if got != (dim, text):
        bad.append(why + " — " + repr(got) + ", 바란 것 " + repr((dim, text)))
print("\n".join(bad))
sys.exit(1 if bad else 0)
"##;
        use std::io::Write;
        use std::process::{Command, Stdio};
        let supervise = supervise();
        let open = supervise.find("python3 - '<session>'").expect("비우는 스크립트가 없다");
        let script = &supervise[open..open + supervise[open..].find("\nPY\n").expect("스크립트가 안 닫힌다")];
        let from = script.find("PROMPT = ").expect("프롬프트 표시가 없다");
        let to = script.find("def looks(").expect("판을 읽는 함수가 없다");
        let program = format!("{HEAD}{}{CASES}", &script[from..to]);
        let mut child = Command::new("python3")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 를 실행하지 못했다 — 이 시험에는 python3 가 있어야 한다");
        child.stdin.take().expect("stdin").write_all(program.as_bytes()).expect("스크립트를 못 넘겼다");
        let out = child.wait_with_output().expect("python3 가 안 끝났다");
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "입력 칸을 잘못 읽는다:\n{said}");
    }

    /// **tmux 시험은 떼어 낸 서버에서만 가르친다**(2026-09-18 사용자 규칙). 스킬이 맨
    /// `tmux new-session -d` 를 가르치던 날, 그 길을 따른 리뷰 서브에이전트가 맨 `kill-server` 로
    /// 사람의 tmux 서버를 통째로 죽였다 — tmux 안에서는 `$TMUX` 가 `TMUX_TMPDIR` 를 이긴다.
    /// 감독이 읽는 글과 일꾼이 받는 글 **둘 다** 에 서야 한다: 시험을 실제로 치는 것은 일꾼과 그
    /// 리뷰 서브에이전트다.
    #[test]
    fn tmux_tests_are_taught_on_a_separate_server() {
        let (supervise, brief) = (supervise(), brief());
        // 감독 쪽은 **브리프를 뺀 글**로 잰다 — 감독 스킬은 브리프를 품어, 통째로 재면 브리프의
        // 같은 줄이 감독 쪽에서 빠진 자리를 메운다. 브리프를 못 찾으면 `replace` 가 말없이 통째로
        // 남기니 먼저 본다.
        assert!(supervise.contains(&brief), "감독이 싣는 글이 brief 가 아니다");
        let own = supervise.replace(&brief, "");
        for (name, text) in [("감독 스킬", own.as_str()), ("일꾼 글", brief.as_str())] {
            assert!(text.contains("env -u TMUX tmux -L"), "{name}: 떼어 낸 서버로 시험하라는 말이 없다");
            assert!(text.contains("without `-L`/`-S`"), "{name}: 맨 kill-server 를 막는 말이 없다");
            assert!(text.contains("TMUX_TMPDIR"), "{name}: TMUX_TMPDIR 로 안 갇힌다는 말이 없다");
            // 속에서 `tmux` 를 부르는 스크립트(5-1)에는 손으로 `-L` 을 못 준다 — 그것을 시험하는
            // 일꾼에게도 가둘 길이 있어야 하고, 그 감싸개가 PATH 로 제 자신을 부르면 끝나지 않는다.
            assert!(
                text.contains("wrapper") && text.contains("absolute path"),
                "{name}: 스크립트를 떼어 낸 서버에 돌릴 길이 없다"
            );
        }
        assert!(
            brief.contains("**Give a review subagent these words too**"),
            "리뷰 서브에이전트가 tmux 규칙을 못 받는다"
        );
        // 시험용 판에 닿는 명령은 모두 떼어 낸 서버에 선다 — 셸 줄의 낱말 `tmux` 와 `` `tmux …` ``
        // 로 적은 글을 함께 본다. 줄 머리로 가르지 않는다: 서버를 죽인 한 줄이
        // `TMUX_TMPDIR=… tmux kill-server` 였다. 맨 명령을 **하지 말라고** 적은 줄(`없는`·`맨 `)만
        // 뺀다. 본 기능의 `tmux("send-keys", …)` 는 낱말이 `tmux` 가 아니라 안 걸린다 — 실제 일꾼
        // 판을 비우는 그쪽은 기본 서버가 맞다.
        const SERVER: [&str; 7] = [
            "new-session",
            "kill-server",
            "kill-session",
            "send-keys",
            "capture-pane",
            "display-message",
            "list-clients",
        ];
        let isolated = |cmd: &str| {
            let words: Vec<&str> = cmd.split_whitespace().collect();
            words.iter().enumerate().filter(|(_, w)| **w == "tmux").all(|(t, _)| {
                // `-L`/`-S` 는 하위 명령 **앞**에 서야 서버를 고른다 — 뒤에 서면 그 명령의 깃발이다.
                words[t..]
                    .iter()
                    .position(|w| SERVER.contains(w))
                    .is_none_or(|sub| words[t..t + sub].iter().any(|w| w.starts_with("-L") || w.starts_with("-S")))
            })
        };
        for line in supervise.lines() {
            let spans = line.split('`').skip(1).step_by(2);
            for cmd in std::iter::once(line).chain(spans) {
                // 맨 명령을 **하지 말라고** 적은 줄만 뺀다 — 영어 글에서 그 자리를 대는 낱말 셋이다.
                let warned = line.contains("Never use") || line.contains("A bare") || line.contains("a bare");
                assert!(isolated(cmd) || warned, "기본 서버를 쓰는 tmux 를 가르친다 — {}", line.trim());
            }
        }
        assert!(!supervise.contains("isolation with `tmux new-session -d`"), "맨 new-session 을 격리라고 가르친다");
    }

    #[test]
    fn reclaimed_work_says_it_lost_the_earlier_share() {
        // **거둔 일의 노트는 앞 세션 몫을 모른다고 말한다**(2026-09-18 사용자 결정). 일한 모델은
        // 닫을 때만 적어(결정 4), 앞 세션이 하다 죽은 멤버를 거둔 창이 닫으면 통째로 제 몫이 된다.
        // 집을 때도 적게 넓히지 않고, 통계가 그 줄을 가를 수 있게 까닭에 표시만 한다.
        let supervise = supervise();
        let at = supervise.find("is handing you the stalled work in").expect("거둔 일을 맡기는 글이 없다");
        let end = supervise[at..].find("**1. Pick.**").map_or(supervise.len(), |n| at + n);
        assert!(
            supervise[at..end].contains("the previous\n        session's share is unknown"),
            "거둔 일의 노트가 앞 세션 몫을 삼킨다"
        );
    }

    /// **일꾼이 마지막 자이고, 일한 모델은 닫을 때 남는다**(moai-lzfq, 2026-09-15 사용자 결정).
    ///
    /// 감독은 코드를 읽기 전에 고르므로 제안일 뿐이다 — 읽어 보니 쓰기 경로면 일꾼이 올린다.
    /// 남기는 자리는 **노트 하나**다: 저널은 상태 계산에 안 읽히고 파생값은 저장하지 않으니
    /// 필드가 아니라 이력으로 남는다(CLAUDE.md 의 되돌리지 않을 결정 둘).
    #[test]
    fn the_worker_may_raise_the_model_and_records_it_when_closing() {
        let brief = brief();
        // **올리는 길이 명령으로 서고, 그 명령을 칠 수 있는 자에게 간다.** "올린다" 만 적으면
        // 일꾼이 무엇을 쳐야 하는지 모른다. 그런데 `/model` 은 사람만 친다 — 에이전트는 붙박이
        // 명령을 못 불러, 제 손으로 치라고 하면 올렸다고 믿고 9-1 에 안 돈 모델을 적는다.
        let raise = brief
            .find("the model that pairs with the difficulty you just measured")
            .expect("일꾼이 모델을 올릴 길이 없다");
        // **한 칸씩이 아니다** — 두 칸 어긋난 제안이 가운데 모델에 멈추면 쓰기 경로를 싼 모델이 한다.
        assert!(!brief.contains("raise it one step"), "두 칸 어긋난 제안이 한 칸만 오른다");
        // 알리는 글은 **글자로 자른다** — 바이트로 자르면 한글 한가운데서 끊겨, 실패를
        // 알리려던 자리가 제가 먼저 죽는다.
        let shown = brief[raise..].chars().take(40).collect::<String>();
        assert!(brief[..raise].contains("`/model`"), "무엇으로 올리는지가 없다 — {shown}");
        assert!(
            brief[..raise].contains("ask the person watching the window"),
            "`/model` 을 사람에게 청하라는 말이 없다 — {shown}"
        );
        assert!(brief.contains(&indent(&model_line(), "    ")), "새 일의 모델 줄이 조각에서 안 나온다");
        // **닫는 자리에 선다** — 워크트리를 지운 뒤(맨 `moai` 가 루트를 읽는다), 멤버를 닫기
        // 전. 자리를 바이트 거리로 재던 판은 9-1 이 9 위로 올라가도 초록이었다.
        let removed = brief.find("git worktree remove").expect("워크트리를 지우는 걸음이 없다");
        let note = brief.find("model:").expect("일한 모델을 남기는 걸음이 없다");
        let done = brief.find("moai mv <member> done").expect("멤버를 닫는 걸음이 없다");
        assert!(removed < note && note < done, "모델 노트가 워크트리를 지운 뒤·멤버를 닫기 전이 아니다");
        // **남기는 것은 실제로 돈 모델이다.** 머리의 제안은 감독이 채워 보내 이 줄에도 박혀
        // 오므로, 다르면 고쳐 적으라는 말이 같은 걸음에 서야 한다 — 없으면 제안이 일한 것으로
        // 남는다. 그 까닭이 남을 자리가 이 노트다.
        let step = brief.find("\n    9-1.").expect("모델을 남기는 걸음이 없다");
        assert!(brief[step..note].contains("actually ran"), "제안이 아니라 실제로 돈 모델을 남기라는 말이 없다");
        let line = brief[note..].lines().next().unwrap_or_default();
        assert!(line.contains("<why>"), "노트에 까닭 자리가 없다 — {line}");
        // `moai note` 로 남긴다. 9-1 은 8 에서 루트로 돌아오고 9 에서 워크트리를 지운
        // 뒤라 맨 `moai` 가 맞다 — 4-1 의 `-C <루트>` 는 워크트리 안에서만 드는 손잡이다.
        let line = brief[..note].lines().last().unwrap_or_default();
        assert!(line.trim().starts_with("moai note "), "노트가 아닌 것으로 남긴다 — {line}");
    }

    /// **가르치는 꼴이 `model::parse_work` 가 읽는 꼴이다**(moai-jo8d). 9-1 이 회사·토큰 없는 옛
    /// 꼴을 가르치던 동안 감독 아래의 일은 모두 빈 칸을 적어 `work` 통계가 늘 비었다 — 글과
    /// 파서가 따로 서면 다시 갈린다. 자리를 채워 파서에 넣어, 회사·토큰까지 값이 되는지 본다.
    /// 토큰을 모를 때 `tokens=<수>` 를 통째로 빼도 읽혀야 한다 — 0 을 적게 두면 통계가 "공짜" 로 읽는다.
    #[test]
    fn the_taught_model_line_is_the_one_the_parser_reads() {
        let texts = [("브리프 9-1", brief()), ("AGENTS 블록", agents()), ("스킬 참고 문서", reference())];
        for (whose, text) in &texts {
            let line = text
                .lines()
                .map(str::trim)
                .find(|l| l.starts_with("moai note ") && l.contains("'model: "))
                .unwrap_or_else(|| panic!("{whose} 에 일한 AI 를 남기는 줄이 없다"));
            let said = &line[line.find("'model: ").unwrap() + 1..line.rfind('\'').unwrap()];
            let filled = said
                .replace("<vendor>", "anthropic")
                .replace("<model>", "opus-5")
                .replace("<count>", "182000")
                .replace("<difficulty>", "high")
                .replace("<grade>", "high")
                .replace("<why>", "write path");
            let w = crate::model::parse_work(&filled)
                .unwrap_or_else(|| panic!("{whose} 의 꼴을 파서가 못 읽는다 — {said}"));
            assert_eq!(w.provider.as_deref(), Some("anthropic"), "{whose} 가 회사를 안 가르친다 — {said}");
            assert_eq!(w.tokens, Some(182000), "{whose} 가 토큰을 안 가르친다 — {said}");
            assert_eq!((w.grade.as_deref(), w.why.as_deref()), (Some("high"), Some("write path")), "{whose} — {said}");
            let unknown = crate::model::parse_work(&filled.replace(" tokens=182000", ""));
            assert_eq!(unknown.map(|w| w.tokens), Some(None), "{whose} 의 꼴이 토큰을 빼면 안 읽힌다 — {said}");
            assert!(text.contains("If you do not know the token count"), "{whose} 에 토큰을 모를 때 빼라는 말이 없다");
        }
    }

    /// **가르치는 자유 글은 작은따옴표로 싼다**(2026-09-18 사용자 결정). 큰따옴표로 가르친 `-m`·`-b`·
    /// 노트는 채운 글에 백틱이나 `$(…)` 가 들면 bash 가 명령 치환을 해 글이 잘린 채 0 으로 끝난다 —
    /// 이 저장소의 노트는 백틱을 자주 쓴다. 한 표면이라도 큰따옴표로 돌아가면 여기서 붉어진다.
    #[test]
    fn free_text_is_taught_in_single_quotes() {
        let texts = [("AGENTS 블록", agents()), ("스킬", skill()), ("참고 문서", reference()), ("감독", supervise())];
        for (whose, text) in &texts {
            for line in text.lines().filter(|l| l.contains("moai ")) {
                // 제목 자리도 자유 글이다(moai-1yya) — 이 저장소 제목 1,199개 중 39개에 백틱이 든다.
                for bad in
                    ["-m \"", "-b \"", "moai note <id> \"", "moai note <member> \"", "moai note <epic> \"", "add \""]
                {
                    assert!(!line.contains(bad), "{whose} 가 자유 글을 큰따옴표로 가르친다 — {line}");
                }
            }
        }
        assert!(
            make_review("-e <epic>").contains("-b '<what you are looking for and why>'"),
            "리뷰 줄의 관점이 작은따옴표가 아니다"
        );
        assert!(handoff("t-1").ends_with("'Next: <what comes next>'"), "이어받을 줄이 작은따옴표가 아니다");
    }

    /// **감독 스킬은 모든 저장소에 심긴다.** 이 저장소의 이슈 id 를 적으면 남의 저장소에서는
    /// 아무것도 안 가리키고, 고친 뒤에는 거짓이 된다.
    #[test]
    fn the_supervisor_names_no_issue_of_this_repo() {
        let text = supervise();
        let ids: Vec<&str> = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter(|w| {
                w.strip_prefix("moai-").is_some_and(|rest| {
                    rest.len() == 4 && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                })
            })
            .collect();
        assert!(ids.is_empty(), "이 저장소의 이슈 id 가 섰다 — {ids:?}");
    }
    #[test]
    fn the_supervised_worker_settles_the_three_old_questions() {
        // 2026-09-18 사용자 결정 셋. (1) 보낸 일은 늘 워크트리 (2) 일꾼은 병합 전에 옆 세션을 찾아
        // 알리지 않는다 — 찾을 길이 없는 말은 지킬 수 없다 (3) 펼칠 안을 사람에게 다시 묻지 않는다.
        let (supervise, brief) = (supervise(), brief());
        let head = &supervise[..supervise.find("## One round").expect("한 바퀴가 없다")];
        assert!(head.contains("**Work you send out is always done in a worktree**"), "워크트리 전제가 머리에 없다");
        assert!(!supervise.contains("tell the other worker first"), "일꾼이 찾을 길 없는 옆 세션에 알리라고 한다");
        assert!(!brief.contains("tell the other worker first"), "일꾼이 찾을 길 없는 옆 세션에 알리라고 한다");
        let one = brief.find("\n    1. ").expect("1 이 없다");
        let two = brief.find("\n    2. ").expect("2 가 없다");
        assert!(brief[one..two].contains("not to show a person"), "맡긴 idea 를 펼칠 때 사람에게 또 묻는다");
    }

    #[test]
    fn a_file_held_next_door_becomes_a_member_not_an_edit() {
        // **에픽 도중 새로 필요해진 파일을 옆이 쥐었으면 멤버로 남기고 알린다**(2026-09-18 사용자
        // 결정). 감독이 보내기 전에 파일을 재도 도중에 새로 필요해진 파일은 못 잰다 — 그런 일이
        // idea 로 밖에 나가 에픽이 목적을 못 이룬 채 닫힌 적이 있다. 일정 문제가 범위 결정으로 위장한다.
        let (supervise, brief) = (supervise(), brief());
        assert!(brief.contains("Work running alongside: <other work>"), "일꾼이 옆에서 쥔 파일을 모른다");
        let list = slot_list(&supervise);
        assert!(list.contains("`<other work>`"), "감독이 옆 일을 안 채운다 — {list}");
        let at = brief.find("\n    4-3. ").expect("4-3 이 없다");
        let end = brief.find("\n    5. ").expect("5 가 없다");
        assert!(
            brief[at..end].contains("-e <epic>") && brief[at..end].contains("Do not defer it"),
            "옆이 쥔 일이 멤버로 안 남는다"
        );
        let report = brief.find("\n    11. ").expect("11 이 없다");
        assert!(
            brief[report..].contains("because the work beside you held the file"),
            "보고가 옆이 쥐어 남긴 멤버를 안 댄다"
        );
        assert!(
            supervise.contains("**A member left because the work beside it holds the file**"),
            "감독이 그 멤버를 언제 보낼지 모른다"
        );
        // 4-3 이 남긴 멤버도 아무도 안 했고 에픽을 열어 둔다 — 9-1 이 모델 줄을 적거나 10 이 닫으면
        // 에픽이 목적을 못 이룬 채 닫힌다(7-1 의 멤버와 같은 덫).
        let noted = brief.find("\n    9-1. ").expect("9-1 이 없다");
        let closed = brief.find("\n    10. ").expect("10 이 없다");
        assert!(
            brief[noted..closed].contains("first column by 7-1 and 4-3"),
            "9-1 이 4-3 의 멤버에 일한 모델을 남긴다"
        );
        assert!(brief[closed..report].contains("first column by 7-1 and 4-3"), "10 이 4-3 의 멤버를 닫는다");
        // 거둔 일도 4-1 부터 끝까지 받아 4-3 을 받는다 — 4-3 이 가리키는 머리 줄이 거기에도 서야 한다.
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];
        assert!(head.contains(&format!("      {BESIDE}")), "거둔 일의 머리에 4-3 이 가리키는 옆 일 줄이 없다");
    }

    #[test]
    fn a_test_claude_on_a_detached_server_is_no_worker() {
        // **떼어 낸 tmux 서버의 시험용 claude 는 일꾼이 아니다**(2026-09-18 사용자 결정). 그 세션도
        // 세션 파일을 쓰고 cwd 가 루트면 2 의 훑기가 놀고 있는 세션으로 읽어 일을 맡긴다. 판 주인을
        // 5-1 처럼 보고, 시험하는 쪽에도 루트 밖에서 띄우라고 한다 — 둘 중 하나만 서면 다른 쪽이 샌다.
        let (supervise, brief) = (supervise(), brief());
        let two = supervise.find("**2. Find a worker.**").expect("2 가 없다");
        let three = supervise.find("**2-1. ").expect("2-1 이 없다");
        let step = &supervise[two..three];
        assert!(step.contains("elif cwd == root and detached(s):"), "떼어 낸 판을 루트 세션으로 읽는다");
        // `ps` 가 없는 기계에서 훑기 전체가 역추적으로 죽지 않는다 — 5-1 의 `parents` 와 같은 울타리.
        let walk = step.find("def parents(pid):").expect("판 주인을 거슬러 오르는 함수가 없다");
        let walk = &step[walk..walk + step[walk..].find("def detached(").expect("detached 가 없다")];
        assert!(walk.contains("except OSError:"), "ps 가 없으면 세션 목록이 통째로 죽는다");
        assert!(step.contains("'#{pane_pid}'"), "판 주인을 포맷이 깨뜨렸다");
        assert!(step.contains("**Do not hand work to a `test pane` row.**"), "떼어 낸 판을 어떻게 할지 없다");
        // **그 줄의 이름은 `detached` 가 아니다**(리뷰). 이 스킬에서 `detached` 는 루트의 git HEAD 라,
        // 한 낱말로 둘을 부르면 시험용 판 하나가 루트의 detached 로 읽혀 한 바퀴가 통째로 멈춘다.
        assert!(step.contains("print(\"test pane\","), "떼어 낸 판의 이름이 루트의 HEAD 와 같은 낱말이다");
        assert!(brief.contains("keep its cwd outside the root"), "시험용 claude 를 루트에서 띄운다");
        // **못 닿는 것은 떼어 낸 판이 아니다**(moai-gmut). 감독이 tmux 밖에서 돌거나 소켓이 막히면
        // 물음마다 빈 값이 오는데, 그것을 떼어 낸 판으로 읽던 판은 루트의 세션을 **모두** 걸러
        // 아무도 일을 못 받았다 — 까닭을 대는 줄도 없어 감독이 무엇이 어긋났는지 몰랐다.
        assert!(step.contains("if not tmux_up:\n        return None"), "tmux 에 못 닿는 것을 떼어 낸 판으로 읽는다");
        assert!(step.contains("cannot reach tmux"), "못 닿을 때 까닭을 대는 줄이 없다");
    }

    #[test]
    fn the_worker_steps_into_the_subproject_of_a_monorepo() {
        // **모노레포의 하위가 루트면 워크트리 안의 같은 하위에서 일한다**(2026-09-18 사용자 결정).
        // 워크트리 꼭대기에 선 일꾼은 `moai` 가 공유 루트의 `.moai` 를 찾아 쓰고, 훅 규칙 2 는
        // `.claude/` 아래라 편집을 안 센다.
        let (supervise, brief) = (supervise(), brief());
        assert!(supervise.contains("print(\"subdir\", os.path.relpath(here, top))"), "감독이 하위 경로를 안 낸다");
        let three = brief.find("\n    3. ").expect("3 이 없다");
        let four = brief.find("\n    4. ").expect("4 가 없다");
        assert!(brief[three..four].contains(SUBDIR), "일꾼이 워크트리 안의 하위로 안 들어간다");
        // 거둔 일은 4-1 부터만 받아 3 이 없다 — 그 머리에도 같은 줄이 서야 이어받은 일꾼이 꼭대기에 안 선다.
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];
        assert!(head.contains(SUBDIR), "거둔 일의 일꾼이 워크트리 꼭대기에 선다");
    }

    /// **일꾼에게 싣는 글은 가지 이름을 박지 않는다.** `main` 을 박으면 `develop`·`trunk`
    /// 저장소에서 워크트리 뜨기부터 실패한다. 감독이 읽어 채울 자리와 읽는 한 줄이 선다.
    ///
    /// **낱말로 가른다.** 글자로 찾으면 `master`·`develop` 을 박은 글은 지나가고 `remain` 은
    /// 막힌다. 감독이 제 손으로 치는 병합 확인도 같은 자리를 쓰는지 본다.
    #[test]
    fn the_brief_names_no_branch() {
        let (brief, supervise) = (brief(), supervise());
        let named: Vec<&str> = brief
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter(|w| ["main", "master", "develop", "trunk"].contains(w))
            .collect();
        assert!(named.is_empty(), "일꾼 글이 가지 이름을 박았다 — {named:?}");
        assert!(brief.contains("<base branch>"), "일꾼 글에 본 가지 자리가 없다");
        assert!(
            supervise.contains("git merge-base --is-ancestor <merge hash> <base branch>"),
            "감독의 병합 확인이 본 가지 자리를 안 쓴다"
        );
        // 루트의 가지는 `git worktree list` 의 첫 자리에서 읽는다 — `-C <루트>` 로 읽으면 옮겨
        // 적은 경로가 틀릴 때 조용히 `main` 이 나오고, 워크트리 안에서 짐작한 자리는 제 가지를 낸다.
        assert!(supervise.contains("if w=$(git worktree list --porcelain); then"), "감독이 루트의 가지를 안 읽는다");
        // 포맷 문자열의 `${{b:-main}}` 이 셸의 `${b:-main}` 으로 풀렸는가.
        assert!(supervise.contains("echo \"$b\""), "본 가지 한 줄이 포맷에서 깨졌다");
        // **detached 면 멈춘다**(2026-09-18 사용자 결정). `origin/HEAD` 로 대신 읽던 판은 일꾼의
        // 병합을 가지 없는 HEAD 에 세워 `branch -d` 뒤에 그 일의 참조가 하나도 안 남았다.
        assert!(!supervise.contains("origin/HEAD"), "detached 루트에서 원격의 기본 가지로 대신 읽는다");
        assert!(
            supervise.contains("**If the root is detached, do not send.**"),
            "detached 루트에서 멈추라는 말이 없다"
        );
        // 일꾼은 다시 읽지 않되 **대조한다** — 바퀴 중에 루트의 가지가 바뀌어도 병합이 엉뚱한
        // HEAD 에 서지 않게.
        let merge = brief.find("git merge --no-ff worktree-<epic>").expect("병합 걸음이 없다");
        let eight = brief.find("\n    8.").expect("8 이 없다");
        assert!(brief[eight..merge].contains("symbolic-ref -q HEAD"), "병합 전에 루트의 가지를 대조하지 않는다");
        // 커밋 전의 대조는 머리에 선다 — 거둔 일은 머리를 따로 받으니 거기에도 같은 글이 서야 한다.
        assert!(brief.contains(&indent(BRANCH_CHECK, "    ")), "새 일의 머리에 루트 대조가 없다");
        let head = &supervise[..supervise.find(&brief).expect("감독이 싣는 글이 brief 가 아니다")];
        assert!(
            head.contains(&indent(BRANCH_CHECK, "      ")),
            "거둔 일의 트래커 커밋이 대조 없이 엉뚱한 HEAD 에 선다"
        );
    }

    /// **heredoc 은 들여쓰지 않는다.** 4칸 들여쓴 블록을 그대로 복사하면 닫는 표시도
    /// 들여써져 셸이 끝을 못 찾는다 — `<<-` 는 탭만 벗긴다. 여는 줄도 닫는 줄도 왼쪽 끝이다.
    ///
    /// **종료어는 글을 싸는 데 흔히 쓰는 `MD`·`EOF` 가 아니다.** 가르친 글을 제 heredoc
    /// (`moai note <id> -b - <<'MD'`, 커밋의 `<<'EOF'`)에 옮겨 담으면 셸이 인용된 그 줄에서
    /// 바깥 heredoc 을 끝내, 메모는 잘려 적히고 남은 글은 명령으로 돈다.
    #[test]
    fn heredocs_are_copyable_as_written() {
        // 표면마다 센다 — 합쳐 세면 두 표면에 드는 조각이, 한 표면에만 있는 heredoc 이 빠진
        // 자리를 메운다.
        for (name, text, want) in [
            ("agents", agents(), 2),
            ("skill", skill(), 1),
            ("reference", reference(), 3),
            ("supervise", supervise(), 1),
        ] {
            let lines: Vec<&str> = text.lines().collect();
            let mut seen = 0;
            for (i, line) in lines.iter().enumerate() {
                let Some(tag) = heredoc_tag(line) else { continue };
                assert!(!line.starts_with([' ', '\t']), "{name}: heredoc 여는 줄이 들여써졌다 — {line}");
                assert!(
                    !["MD", "EOF"].contains(&tag.as_str()),
                    "{name}: 종료어 {tag} 가 글을 싸는 heredoc 과 겹친다 — {line}"
                );
                // 닫는 줄은 그 블록 안에서 찾는다 — 펜스나 다음 heredoc 을 넘어가면 뒤의 같은
                // 종료어가, 빠지거나 들여써진 닫는 줄을 가린다.
                let close = lines[i + 1..]
                    .iter()
                    .take_while(|l| !l.trim_start().starts_with("```") && heredoc_tag(l).is_none())
                    .find(|l| l.trim() == tag)
                    .unwrap_or_else(|| panic!("{name}: 닫는 {tag} 가 그 블록 안에 없다 — {line}"));
                assert_eq!(*close, tag, "{name}: 닫는 {tag} 가 왼쪽 끝에 없다 — {line}");
                seen += 1;
            }
            assert!(seen >= want, "{name} 에서 heredoc 을 {seen}개밖에 못 찾았다 — {want}개는 있다");
        }
    }

    /// 줄에서 heredoc 을 여는 `<<` 의 종료어. 셸이 읽는 모양을 따른다 — `<<-`, 띄어 쓴
    /// `<< 'X'`, 따옴표나 역슬래시로 싼 것, 맨 낱말. `<<<` 와 산술(`$((x << 2))`) 속 `<<` 는
    /// heredoc 이 아니다 — 숫자로 여는 맨 낱말도 자리 옮김으로 읽는다.
    fn heredoc_tag(line: &str) -> Option<String> {
        let mut from = 0;
        loop {
            let at = from + line[from..].find("<<")?;
            let rest = &line[at + 2..];
            from = at + 2 + (rest.len() - rest.trim_start_matches('<').len());
            let before = &line[..at];
            if rest.starts_with('<') || before.matches("((").count() > before.matches("))").count() {
                continue;
            }
            let word = rest.strip_prefix('-').unwrap_or(rest).trim_start_matches([' ', '\t']);
            let bare = |s: &str| s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
            let tag = match word.chars().next()? {
                q @ ('\'' | '"') => word[1..].split_once(q)?.0.to_string(),
                '\\' => bare(&word[1..]),
                d if d.is_ascii_digit() => continue,
                _ => bare(word),
            };
            return (!tag.is_empty()).then_some(tag);
        }
    }
}
