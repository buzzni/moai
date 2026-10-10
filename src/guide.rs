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

/// 규칙 다섯의 이름. **스킬이 적은 규칙과 훅이 낸 거절문이 같은 이름을 댄다** —
/// 다르면 막힌 쪽이 무엇을 어겼는지 두 번 읽어야 한다.
pub const RULES: [&str; 5] = [
    "New issues stay inside what you picked up",
    "Pick something up before you change the repository",
    "A review is an issue too",
    "Never kill the person's tmux server",
    "Ask before you pick up someone else's work",
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

/// 에픽 둘에 걸친 묶음의 등급(moai-h89f, 2026-09-22 사용자 결정). 옆 워크트리가 파일을 쥐어
/// 서로 다른 에픽의 멤버가 한 가지에 실리는 판이 2026-09-20 에만 두 번 섰는데, 리뷰 표는
/// `에픽 끝` 과 `에픽 밖 이슈 하나` 둘만 재어 그 판을 아무도 안 잰다 — 그날은 감독과 일꾼이
/// 그때그때 골라 둘 다 `max` 였다. 맞았지만 규약이 시킨 것은 아니다.
///
/// **한 에픽처럼 재고 한 칸 더 올린다.** 후보 셋(한 덩이로 재기·에픽마다 재서 높은 쪽·무조건
/// 한 칸 더) 가운데 첫째와 셋째를 합친 것이다 — 리뷰가 두 에픽의 계약을 같이 보아야 하니
/// `표면을 가로지르면 한 칸 더` 와 같은 결이고, 실제로 선 판 둘의 `max` 와도 맞는다.
/// 이 저장소 CLAUDE.md 의 리뷰 표와 `the_rubric_is_the_review_table` 이 이 글을 견준다.
const BUNDLE: &str =
    "A worktree that carries members of two epics is measured as one epic and then raised one more step";

/// 리뷰가 되풀이해 잡는 다섯 자리(moai-jza6). 2026-09-20~21 에 리뷰가 잡았지만 브리프에는
/// 없던 것들이고, 다섯 다 한 번이 아니라 여러 판에서 되풀이됐다 — 그때까지는 감독이 브리프마다
/// 손으로 실었고, 실으면 잡히고 안 실으면 리뷰가 그때그때 운으로 잡았다.
///
/// **어제의 버그 목록이 되지 않게 되풀이된 것만 싣는다**(backlog `moai-53wq` 가 며칠 묵힌 까닭이
/// 그것이다). 한 번 잡힌 것을 여기 박으면 브리프가 그날의 고침 목록으로 자란다.
///
/// **관점을 대신하지 않는다.** 이 에픽이 실제로 한 일이 관점의 본체고, 이 다섯은 그 위에
/// 얹는다 — 이것만 적은 `-b` 는 어느 에픽에나 같은 글이라 다음 사람이 읽을 것이 없다.
///
/// **`-b` 에만 적으면 리뷰에 안 닿는다.** 이슈의 관점은 다음 사람이 읽는 글이고 리뷰 명령은
/// 이슈를 안 읽는다 — 그래서 마지막 문단이 다섯을 리뷰 쪽으로 보내는 길을 댄다. 그 문단에
/// 리뷰 명령의 이름을 적지 않는 것은 `the_supervisor_picks_a_model_by_difficulty` 가 브리프의
/// `/code-review` 를 하나로 매기 때문이다.
const REVIEW_ANGLE: &str = r#"**Five places the review keeps finding.** They do not stand in for the angle — what this
epic actually did is the angle, and these go on top of it
1. A struct or function inserted above another takes over the doc block of the item below
   it, and that comment now sits on code it does not describe
2. Does a test actually go red on a revert — is what it measures in one place. A count
   held per row whose inside is a `OnceCell` is 0 or 1 whatever happens, so the timing it
   was meant to pin went back whole with nothing red
3. Is there only one place that sets it up — a value put in place once at start-up is put
   back to its default by every other path that builds the same thing again
4. Do the comments and the docs say what the code actually does
5. Does anything newly open on a path that never opened it — not "is a lock held while
   opening", which is half of it. A read path that never opened the config and now parses
   it stops on a config that is a FIFO, lock or no lock

**They have to reach the review itself, not only `-b`.** The angle on the issue is what the
next person reads; the review command does not read the issue. Hold these five against what
came back before you take the findings in"#;

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
         **{BUNDLE}** — the review has to read both epics' contracts at once.\n\
         `max` is the top of the ladder: a step above it is still `max`.\n\
         The model follows that grade — `medium` means `{mid}`, `high` and up means `{top}`."
    )
}

/// 메시지 머리의 `Model:` 줄 — 감독이 2-1 에서 골라 채운다. 새 일과 거둔 일(감독 0)이 **같은 줄**을 싣는다 — 손으로 두
/// 벌 적던 판은 거둔 쪽이 일꾼에게 없는 감독의 절(2-1)을 가리켰다.
///
/// 자리 이름은 `<difficulty>` 다. `<grade>` 는 7 에서 개발해 본 일꾼이 고르는 리뷰 등급의 자리라, 감독이 채우는
/// 목록에 같은 이름을 넣으면 읽기 전의 제안이 그 명령에 박힌다.
const MODEL_SLOT: &str = "Model: <model> (<difficulty> — <why>)";

/// 그 줄을 일꾼이 어떻게 읽는가 — 일꾼 글([`worker`])의 "The assignment" 에 선다(moai-obxm). 메시지의 머리에는
/// 자리만 싣고 읽는 법은 그 아래 일꾼 글에 둔다.
///
/// **모델은 사람만 바꾼다.** 에이전트는 붙박이 명령을 못 부르고, 설정 파일의 모델은 새 세션에만 든다 — 그래서
/// 바꾸기는 창을 보는 사람에게 청한다(`/model`). 제 손으로 치라고 하면 일꾼은 올렸다고 믿고 9-1 에 안 돈
/// 모델을 적는다. 결정은 "맞추거나 올린다" 였다 — 제안과 다른 모델로 뜬 창은 먼저 맞춘다.
///
/// **올릴 때는 다시 잰 난이도의 짝으로 간다**(2026-09-18 사용자 결정). "한 단계 올린다" 만 주던 판은 `low` 로 받아
/// 읽어 보니 `high` 인 일이 가운데 모델에 멈춰, 쓰기 경로를 싼 모델이 했다 — 난이도 한 낱말이 모델을 정한다는 축과
/// 어긋난다.
///
/// **사다리는 Claude 의 것이다** — 감독 스킬이 Claude Code 에만 심기니(moai-obxm) 받는 창도 Claude Code 다.
fn model_rule() -> String {
    let ladder = DIFFICULTY.iter().map(|(_, model, _)| *model).collect::<Vec<_>>().join(" → ");
    let top = top_model();
    format!(
        r#"`Model:` is a suggestion picked by difficulty before anyone read the code. The model is
changed by the person with `/model`, never by you — if this window is not on
that model, ask the person watching the window to match it, and if it reads harder than it
looked, raise it the same way to the model that pairs with the difficulty you just measured —
not one step at a time ({ladder}). Handed `low` but it is `high`, the model is `{top}`.
The grade of the epic-end review (7) is measured on this same rubric, member by member"#
    )
}

/// 메시지 머리의 옆 일 줄(moai-alsi). 새 일과 거둔 일(감독 0)이 **같은 줄**을 싣는다 — 4-3 이 "메시지의
/// `Work running alongside`" 를 가리키니, 거둔 일의 메시지에 없으면 이어받은 일꾼은 가리키는 줄이 없는 규칙을 받는다.
const BESIDE: &str = "Work running alongside: <other work> — do not touch those files (4-3)";

/// 루트 HEAD 를 대조하는 글(moai-gokz, 2026-09-18 사용자 결정). 일꾼 글([`worker`])의 걸음 앞에 한 번 선다 — 새 일과
/// 거둔 일이 같은 스킬을 읽으니 두 벌이 없다(moai-snyk). 그 전에는 감독 글이 새 일과 거둔 일의 머리에 따로 실어,
/// 한쪽에 없으면 그 일꾼의 트래커 커밋이 대조 없이 엉뚱한 HEAD 에 섰다.
///
/// **대조하는 가지는 루트의 가지다**(2026-10-08 사용자 결정, moai-nvju). 마일스톤 일의 `<base branch>` 는
/// `milestone/<id>` 라 루트가 설 수 없는 가지다 — 그 가지는 마일스톤 워크트리에 체크아웃돼 있다. 그래서 루트는
/// `<root branch>` 와 견주고, 마일스톤 일의 병합은 그 워크트리에서 같은 대조를 받는다.
const BRANCH_CHECK: &str = r#"Before you commit or merge in the root, **only check** that the root still stands on
`<root branch>` — run `git symbolic-ref -q HEAD` **in the root**. If it is not `refs/heads/<root branch>`
(detached, or someone switched the branch), do not run that commit or merge: tell the supervisor and
stop. A merge that lands on the wrong HEAD leaves no reference at all once `branch -d` runs.
Work inside a milestone merges in the milestone's worktree, not in the root, and that worktree
gets the same check before the merge ("The milestone branch").

**Ask it where you already are.** Before the first tracker commit you are still in the root, so
it is one command. From inside the worktree, do not ask with `git -C <root> …` — that shape is
refused there (the git shapes below): `ExitWorktree(keep)`, ask, and if work is left in that
worktree go back in with `EnterWorktree(path)`"#;

/// **격리 가드가 읽을 수 있는 꼴로 이른다**(moai-rgp9). `EnterWorktree` 로 들어간 세션의 Bash
/// 호출은 하네스가 정적으로 읽고, git 이 그 워크트리를 겨눈다는 것을 증명 못 하면 거절한다 —
/// 판정은 "위험하다" 가 아니라 **"확인 불가"** 다.
///
/// 2026-09-29 에 이 저장소의 전사 246개(561MB)를 훑어 **714건**을 셌다. 복합 명령이 든 것이
/// 608건(85%)이고, 꾸밈 하나 없는 단일 명령 28건은 전부 `git -C <루트>` 처럼 **과녁이 밖**인
/// 것이었다. 그 28건 안에 옛 [`BRANCH_CHECK`] 가 이르던 `git -C <root> symbolic-ref -q HEAD`
/// 가 있었다 — 심은 글이 거절되는 명령을 시키고 있었다.
///
/// **Claude Code 의 것이다**(moai-snyk) — 그 하네스의 격리 가드가 거절한 꼴을 센 글이라, Codex·Antigravity 창은
/// 이 걸음 없이 `cd` 로 든다. 일꾼 글에 한 번 서서 새 일과 거둔 일이 같은 글을 읽는다.
const GIT_SHAPES: &str = r#"**Git in a Claude Code worktree session: one plain command per call.** The harness reads each Bash call
and refuses what it cannot prove stays inside your worktree, so the shape matters more than the
intent.
- One command per call. `git add X && git commit …` is refused whole — so is anything with
  `&&`, `;` or `||`
- `-m "…"` on one plain command is fine; a **heredoc** message is refused.
  When the message runs past one line, write it with Write and use `git commit -F <that file>`
- Several git steps in a row: put them in a script file and call it as a bare
  `bash /abs/path/script.sh` with literal arguments and nothing appended — `&&`, a pipe or
  `$PWD` after it is refused
- Never build a command or a path with a variable or `$(…)` — that is refused as
  `computed at runtime`
- **Do not aim git at the root from inside the worktree.** `git -C <root> status`, `commit` and
  `symbolic-ref` are refused even as single plain commands. Root work happens after
  ExitWorktree(keep), and the tracker needs no `-C` at all — `moai` moves that by itself
- Before a tracker commit in the root, look at `git status -- .moai/` first. The path keeps the
  commit from sealing someone's open merge, but it cannot keep it from carrying rows another
  session has not committed yet"#;

/// 밖의 backlog 를 도는 마일스톤 안으로 들이는 한 줄(moai-6qgz). 감독의 1 과 일꾼의 1 이 **같은 줄**을
/// 받는다 — 감독은 "들일 것인가" 를 정하고 일꾼이 실제로 단다. 한쪽만 고치면 감독이 들이기로 한
/// 일이 밖에 선 채로 돌거나, 일꾼이 무엇을 달지 모른다. `promote` 에는 이 플래그가 없다.
const MILESTONE_ATTACH: &str = "moai edit <epic> --milestone <milestone>";

/// 그 마일스톤 id 를 **어디서 베끼는가**(리뷰 moai-9tlp.67r 3번·11번). 혼자 펼치는 세션의 글과
/// 감독의 3 이 같은 자리를 대므로 한 자리에서 나오게 둔다 — 다음에 화면이 바뀌어도 고칠 곳이 하나다.
///
/// **2026-09-25 에 가리키는 자리가 바뀌었다**(리뷰 moai-pdlp.4ox 2번). 도는 마일스톤을 제 손으로
/// 달지 않기로 한 뒤에는 `moai ready` 가 대는 **도는** 릴리스가 베낄 값이 아니다 — 베낄 것은 그
/// 생각이 이미 선 릴리스이고, 그것을 내주는 자는 `moai show --milestone` 하나다(그 절이 같은 말을
/// 한다). 옛 자리를 그대로 두면 글이 시키는 대로 베낀 값이 곧 규칙이 막는 값이 된다.
const MILESTONE_FROM: &str = "the release `moai show --milestone` stands that backlog under";

/// 모노레포 하위로 드는 한 줄(moai-ay3b). 새 일의 3 과 거둔 일의 워크트리 걸음이 같은 줄을 쓴다 —
/// 거둔 일은 3 을 안 받아(4-1 부터), 여기 없으면 이어받은 일꾼만 워크트리 꼭대기에 선다.
///
/// **자리를 감독이 채운다**(리뷰 moai-rgp9.sdj 1번). 옛 줄은
/// `cd "$(git -C <root> rev-parse --show-prefix)"` 였는데, 그 한 줄이 [`GIT_SHAPES`] 가 거절된다고
/// 적어 둔 두 꼴을 동시에 든다 — `$(…)` 로 만든 명령과 워크트리 밖을 겨눈 `git -C <root>` 다.
/// 게다가 이 걸음은 **워크트리 안으로** 드는 것이라 `ExitWorktree` 로 피할 수도 없었다. 감독은
/// 2 의 스크립트에서 이미 그 상대 경로를 `subdir` 줄로 읽으므로, 값을 채워 보내면 일꾼은 `cd` 만
/// 한다. 꼭대기 프로젝트에는 그 줄이 안 서고 이 걸음도 없다.
const SUBDIR: &str = "cd <subdir>";

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
    moai show -s todo -t bug               filters (comma = or, repeated flag = and, except -a: or)
    moai show --tree                       epic → issue → child
    moai ready --worktree                  overlay what the other worktrees picked up
    moai stats                             counts: columns, flow, lead and cycle time, AI work
    moai tui                               walk the explorer. SPC n parks a thought
                                           on a group row: l one step · Tab expand all · h fold
    moai add '<title>' -p 1 -t bug -e <epic>   create
    moai mv <id> in_progress               pick up  →  review  →  done
    moai mv <id> in_progress --take        take over someone else's row (ask first)
    moai edit <id> --tag parser            change
    moai note <id> '<what you found>'      a memo for whoever comes next
    moai defer <id> -m '<why>'             take work out of the plan for now

Every command takes `--json`. `ready --json` gives `{"ready":[…],"others":[…],"held":[…]}` —
`ready` is yours to pick up, `others` is ready work that is someone else's or nobody's
(ask first), and `held` is what is deferred or blocked behind an empty group, and where
to pick it up again. A session a person opened reads that shape to choose its next row —
the moai binary never launches or drives a session itself. Only the supervisor skill, inside
tmux, may clear a worker's pane, paste into it, or open one once the person says yes.

**A key that cannot be absent is never absent.** `kind` and `priority` hold a default,
and the file leaves a default out, but `--json` fills it back in — `jq -r .priority`
on a row gives `2`, never `null`. Keys that genuinely can be absent (`epic`,
`milestone`, `deferred_at`) stay absent, and that absence is the answer.

**Which epic a row is in, you read from `derived_epic`.** `epic` is what the file
says, and a row whose id sits under an epic (`<epic>.<body>`) reads its epic from
that id and writes no `epic` of its own — so on those rows `epic` is absent and
`derived_epic` names the epic. On a row that carries it, absence means the row is in
no epic at all, and on a group row it never stands. One row reads the two keys against
each other: where the same id stands twice and the other line is a different `kind`,
this line is counted into no group anywhere — the tree draws it under `(lost)`, `-e`
picks it up for no epic, and `derived_epic` is absent even when `epic` is written.
`duplicate_id` on the board names that id. A row below an id that stands twice, where
the lines hand down different answers, is counted into no group either — which line
it hangs from cannot be told — and `twin_parent` names it. Two surfaces carry neither key —
`rm --json` hands the removed lines back exactly as the file held them, and `tui
--json` prints the explorer's own shorter row — and there you read the epic off the id.

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

/// 승인 게이트가 없다는 말 — **한 자리만 판다**(moai-0zjo, 2026-10-02 사용자 결정): 남의 줄이나 담당
/// 없는 줄을 집기 전에는 사람에게 묻는다. 그 밖은 여전히 묻지 않는다.
const NO_GATE: &str = "There is no approval gate — create anything, move anything. Do not ask a human, \
except before you pick up work that is someone else's or nobody's (hook rule 5).";

const FORKS: &str = r#"**1. `add` or `backlog`** — what decides is *whether you would pick it up now.*
If you would, `moai add`; if it is for later, `moai backlog add '<what came to mind>'`.
A backlog item stands on neither the board nor `ready`, so it does not blur the plan.
**Walking past it without writing it down is the worst of all.**

If it came out of an epic, ask one more question first — *can this epic deliver
what it promised without this?* If not, it is not for later: it is this work,
unfinished. Even when you cannot do it now (waiting on a person's decision, the
work beside you holds that file), create it as a member with `-e <epic>` and
leave it in the first column — a member still standing keeps the epic from
closing by itself. Send it out as a backlog item and the epic stands `done` without
having delivered what it promised. To `defer` such a member is to decide to give
that promise up.

**2. `defer` or `done`** — never move to `done` what you decided not to do.
`moai defer <id> -m '<why>'` changes neither the column nor the kind, and
`--undo` brings the same row back as it was. A backlog item is "not work yet"; a defer
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
```

**A running milestone is the person's to fill.** When one is running, ask in the same
breath as the plan whether this bundle belongs in it, and attach it only on a yes —
`moai add --from - --milestone <id>`. It goes onto the epics the plan creates and the
members inherit it; without it the whole plan stands outside the release, and of it
`moai ready` then hands out only what is `p0`. Do not hang a running release on a plan
because the plan looks urgent: that is the release changing size while it runs.

**`--body` says why these issues are one bundle.** It goes onto the first epic the
plan creates, which is where `moai show <epic>` reads it from. `--body <text>` takes
the text itself (a file path there becomes the body as written), `--body -` reads
stdin, and only `--from <file>` reads a file. `--body -` and `--from -` cannot both
read stdin, so either put the plan in a file and stream the body —
`moai add --from plan.md --body -` — or keep the plan on stdin and pass the body as
text — `moai add --from - --body '<text>'`."#;

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

/// 담아 둔 생각을 펼치는 절. **펼치기 뒤의 두 걸음이 여기 함께 선다**(moai-fww7).
///
/// 마일스톤 걸음은 감독 길에만 있었다(moai-6qgz) — 감독 없이 혼자 펼치는 세션이 읽는 글에는
/// 없어, 도는 마일스톤 밖에 에픽이 그대로 섰다. 다는 줄은 [`MILESTONE_ATTACH`] 하나에서 나온다.
///
/// 본문 걸음은 어느 글에도 없었다. `promote` 는 본문을 안 데려가, 2026-09-22 에 한 번에 펼친
/// 에픽 셋이 모두 0자로 섰고 사람이 손으로 채웠다.
fn backlog() -> String {
    format!(
        r#"    moai backlog add '<what just came to mind>'       park it
    moai backlog add '<a longer thought>' -b -        the body comes from stdin
    moai backlog ls                                   see what has piled up
    moai show -g <keyword>                         find out whether it is written down already

When the time comes, unfold one into an epic and issues. Unfolding closes the thought.

```sh
moai backlog promote <id> --from - <<'PLAN'
# Epic title
- [p1] first issue #enhancement
PLAN
```

**A line in the plan becomes the issue title verbatim.** Copy over a backlog item title
that grew long while you parked it and that length spreads into the issues, so
write a short new title when you unfold — the original text stays on that backlog,
and the history line about being unfolded from it leads back there.

**The backlog's milestone and body go onto the epic by themselves.** `promote` puts
both on the epic it unfolds — a milestone is inherited, so the epic alone carries
it to every member, the ones added later included, and the body is what lets
`moai show <epic>` say why these issues are one bundle. Not onto every issue: the
original stays on the closed backlog and the history leads back to it, and the one
place worth filling is the epic, so the window that picks a member up does not
have to press every member to find out what this is.

**The milestone that comes over is the one `moai show --milestone` stands the backlog
under**, not whatever its own field says — a backlog item parked inside an epic comes over
in that epic's release even with an empty field of its own, and a field of its own
that loses to the epic it sits in never reaches the new epic. One reader answers
where a row belongs, on every surface.

**Unfolding into a standing epic (`-e <epic>`) carries neither.** That epic is
already the owner — its members inherit its milestone, and writing the backlog's over
theirs would stand one bundle in two places.

**If what came over is not the release that is running, leave it where it stands.**
Work is never pulled into a running release, so the epic stays outside it and what
you do is say so — the person attaches it, with this line, if it belongs in the
release:

    {MILESTONE_ATTACH}

`promote` carries the release the backlog stands in whatever state that release is in,
so a backlog item parked with no milestone, one parked under a release that has since
shipped, and one parked under a milestone since deferred all come over exactly as
they stood. **A dead one you do clear yourself** — nobody chose it here and it hides
the new epic: `moai edit <epic> --milestone none` on a release that has shipped or
been deferred. What standing outside costs meanwhile: every member under that epic
is work picked up from outside the running release, of which `moai ready` hands out
only what is `p0`; and under a deferred milestone the whole plan is out of the plan
the moment it is created — not in `ready`, not in `held`, and no warning says so.
**A dead release is said out loud**: unfolding into a deferred or closed milestone
prints one line on stderr naming it, and nothing is blocked.

The id in that line is {MILESTONE_FROM}. Only its shape is checked, so `moai-zzzz`
goes in with exit 0 — but one line on stderr says there is no such milestone,
and `moai status` counts the row as `dangling_milestone`."#
    )
}

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

const ARCHIVE_STORAGE: &str = r#"    moai archive --dry-run     preview eligible closed bundles
    moai archive               move them to .moai/archive/<year>.jsonl
    moai archive --drop <id>   remove the stale archive copies of a live row

Moving is explicit; ordinary writes never archive work. An epic and its members,
and a parent and its children, move together only once the whole bundle has
stood closed for `archive_days`. Milestones stay in the active snapshot.
`status`, `ready`, `prime` and hook boards count the active snapshot and read
archived rows only as context: parents, blockers and milestones. `show <id>`,
`show --archived`, search, statistics and the explorer can list archived rows.
Reopening an archived row with `moai mv <id> todo --from done` restores only that
selected row. Its former bundle stays archived. `moai defer <id> --undo` on an
archived row — a deferred epic whose member you reopened — brings that row back the
same way. Archived IDs stay reserved, and
`status` names an ID that stands both live and archived as `archive_duplicate_id`;
`moai archive --drop <id>` repairs it and keeps the live row.
Yearly archive files use the same `merge=moai` driver as the active snapshot."#;

const DEFERRING: &str = r#"    moai defer <id> -m '<next quarter>'    take it out of the plan for a while
    moai defer <id> --undo                 take it back
    moai show --deferred                   see only what is deferred

Neither the column nor the kind changes — the same row comes back as it was.
What is deferred drops out of `moai ready`, the board and the warnings, and
`moai status` shines one line on it. Defer an epic, a milestone or a parent and
the work under it drops out with it."#;

const GROUPS: &str = r#"    moai epic add '<storage layer>'                an epic
    moai milestone add 'v0.1' --start 2026-09-05 --due 2026-09-20
    moai add '<title>' -e <epic> --milestone <milestone>
    moai show <epic|milestone id>                  what stands under it
    moai show --milestone <id>                     everything attached to that milestone

**A milestone is the one row that carries dates.** `--start` and `--due` take a
calendar day, `YYYY-MM-DD`, and `moai edit <milestone> --due none` clears one.
They stand on a milestone row only — on anything else the write is refused. A
deadline that has passed, and one falling due within `status_due_days` (3 unless
your config says otherwise), is one warning line on the board; **nothing is
blocked and the exit code never changes.**

**`moai show <milestone>` also says how long it took.** That is read from the
closed members' start and finish right then — no field holds it. It comes with
the number it could measure ("2 of 3 closed"), because a member with no
`started_at` is *unknown*, not zero, and it is wall clock, not effort: sessions
running beside each other overlap, and waiting on a person counts too.

**Belonging is inherited.** A child inherits its parent's epic, an issue inherits
its epic's milestone. A child created with `--parent <epic>` belongs to that epic.
Do not write it again on every issue — move the epic and the members come along.

**A plan gives its members the epic's own id.** `moai add --from` and `moai backlog
promote` mint `<epic>.<body>` for every issue in the plan, the way `--parent
<epic>` always did for a review — one subject, one id. The member carries no
`epic` field of its own, so `jq -r .epic` on it is `null` while `jq -r
.derived_epic` names the epic; `moai show <epic>` lists it under `members` and
`moai show -e <epic>` picks it up. Rows created before keep the ids they have:
nothing is ever relabelled.

**An id cannot move, so such a member cannot leave its epic.** `moai edit
<member> -e none` says so and changes nothing; `-e <another epic>` does move it,
and the id it keeps still reads `<first epic>.<body>`. Aim a plan before you
unfold it — for a member that has to stand somewhere else, create it there.

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
- **Work is never pulled into a running milestone.** What ships was decided before it
  started, and three doors put work in afterwards — the person opens two of them. They
  attach it (`moai edit <id> --milestone <milestone>`); or they say yes to a plan you
  showed them and you attach it in that same breath (fork 3); or it came out of a member
  you are working on and is created inside that member's epic (`-e <that epic>`), where
  the release is inherited. Writing `--milestone <the running one>` on a row that stood
  outside, or moving such a row under an epic that is in it, is none of those three: it is
  you deciding what the release contains, so say it to the person and leave the row where
  it is
- **Nothing is blocked.** A `moai mv` that picks up work from outside goes straight
  through, and so does a `--milestone` that carries a row in — the rule above is a rule
  for you, not a refusal. What is already picked up is simply finished — the same ground as never taking work back late
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
The answer, when it was asked and where it was asked are held next to your user config
in `latest.toml`. Ask somewhere else and the answer from the other place is not reused.

    [update]
    check = false        # in your user config: never ask on this machine
    repo = "owner/name"  # where releases come from — buzzni/moai unless written

    MOAI_NO_UPDATE_CHECK=1 moai tui     # or just for this run

`moai update` upgrades the moai that is running: it runs that repository's
`install.sh` over the binary's own directory. `MOAI_REPO` wins over `repo` for one
run, and the repository's `.moai/config.toml` is never read for it. A build from
source and a directory you cannot write are refused before anything is fetched."#;

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
  are adding several issues up, call the list once instead of calling per id, with
  `--archived` so every closed row is in (`--all` leaves out what has stood in done
  for `archive_days`), or let `moai stats --json` add them up: its `work` sums tokens
  by model and by grade and counts the lines that carry none apart"#;

/// 통계(moai-1hka). **세는 자는 `report::stats` 하나다** — CLI 와 탐색기(`SPC g s`)가 같은 값을
/// 읽으므로, 여기 적는 계약은 둘 다의 것이다. 읽는 것은 스냅샷의 필드와 노트의 `model:` 줄뿐이고
/// 저널의 칸 옮김은 접지 않는다(CLAUDE.md "저널은 상태 계산에 읽히지 않는다").
const STATS: &str = r#"    moai stats                           columns, priorities, flow, lead and cycle time, AI work
    moai stats -e <epic> --by tag,assignee   one epic, two axes in full
    moai stats --bucket day --last 14    flow per day for the last fourteen days
    moai stats --json                    the same numbers for a machine

It takes the filters `moai show` takes and counts one kind — `issue` unless
`--type` names another; a group is measured through its members (`-e`,
`--milestone`). Done, deferred, backlog and the archive are opened, because a
count of history that hides what closed would say nothing closed. In the explorer
`SPC g s` opens the same numbers as bars, narrowed by the filter that is hung.

- **Unknown is not zero.** A done row with no `started_at` (it closed before that
  field existed) is counted under `unknown` in `cycle_time`, never as 0 minutes;
  `tokens` is `null` when no line carried a count, and the lines without one are
  counted apart. Durations are wall clock in minutes, not effort
- **When a row closed** is the time `--done` reads — rows standing in done now, at
  the time they got there. A reopened row is not counted as closed
- **Flow buckets are cut in the screen's time zone**, the zone `--created <day>`
  reads; `flow.zone` names it. Weeks start on Monday
- **An axis need not add up to `rows`.** A row counts once per tag, and a row
  that stands in no group — a twin's eclipsed line, which `-e none` leaves out
  too — is under no epic and no milestone, not under `null`
- **Every key is always there** except `by` (narrowed by `--by`), `email` on an
  assignee with none, and `journal_error` (only when a journal could not be read)
- `moai show <milestone>`'s "Spent" folds closed children into their parent's
  span and ends at `done_at`; `stats` takes each row as its own sample and ends
  when the row entered done (the time `--done` reads). So the two can differ on
  the same milestone — where children closed, and on rows whose `done_at` and
  column time disagree (closed before `done_at` existed, or edited by hand)"#;

const PEOPLE: &str = r#"**The assignee comes for free** — whoever created it is the assignee. To hand it to
someone else, `-a "Name (email)"`; to leave it unowned, `-a none`. The name and
email come from `git config`, and when they are not there you pass them with
`--user "Name (email)"` or `MOAI_ACTOR`.

**Work that is not yours is asked about.** `moai ready` and `moai prime` hand out
only your own rows; a row assigned to someone else, or to nobody, stands apart under
`others` (`owner` is `theirs` or `unowned`). Ask the person before you pick one up,
and on a yes take it over and say who said yes:

    moai mv <id> in_progress --from todo --take -m '<who said yes>'

You become the assignee in the same write and a note `Taken-over: <who it was|none>`
keeps whose it was — it also takes a row that already stands in that column, so give
`--from` the column you saw: if the owner picked it up meanwhile, nothing is taken. Without
`--take` a person in a terminal still moves it (one line on stderr says whose it is);
the hook refuses it (rule 5). Who you are is matched by name or email, the same as
`-a me`; when it is unknown nothing is set apart."#;

const CLOSING: &str = r#"Run `moai status` once more and see whether the warnings grew. Warnings block
nothing — they shine a light on issues with no epic, reviews stalled for a long
time, and how much you have open at once. Backlog piling up and what is deferred are
not warnings; they stand apart as notices (`notices`).

If you end the session still holding something, leave one line on that issue for
the next session to take over from. The next session reads it in the history under
`moai show <id>`.

    moai note <id> 'Next: <what comes next>'"#;

/// 위키를 AGENTS 블록과 참고 문서에 알리는 조각(moai-bl3x). **짧게 둔다** — 어떻게 쓰는가는
/// 셋째 스킬 [`wiki`] 에 있고, 언제나 읽히는 블록에 그 본문을 얹으면 모든 세션이 그 값을 낸다.
/// 여기는 "있다·어디 있다·누가 고친다·아무것도 안 막는다" 네 가지만 말한다.
const WIKI: &str = r#"The repository's manual is markdown pages under one directory — `docs` unless `wiki_dir`
in `.moai/config.toml` says otherwise. `moai wiki ls` lists them and `moai wiki show <slug>`
prints one. When an epic changes what a person does, the window that did it fixes the page
on its branch before the merge, and `moai skill install` plants a skill for it, `moai-wiki`,
that says how — and sweeps the wiki when a person calls it. Nothing checks this."#;

/// 위키를 고칠지 가르는 물음의 낱말 — **에픽이 사람의 쓰임을 바꿨는가.** 일꾼 브리프 7-4 와 위키
/// 스킬이 같은 물음을 묻는다. 두 벌로 적으면 한쪽에만 낱말이 늘어, 브리프로 물은 일꾼과 스킬로
/// 물은 창이 같은 에픽을 다르게 가른다.
const CHANGED_USE: &str = "a key, a command, a flag, a file, a format, a procedure";

/// 위키 페이지를 머지에 태우는 커밋 줄. **일꾼 브리프 7-4 와 위키 스킬이 이 한 줄을 쓴다** — 7-3 의
/// CHANGELOG 줄처럼 손으로 두 벌 적으면 한쪽만 고쳐져, 브리프를 따른 일꾼과 스킬을 따른 창이
/// 다른 제목과 다른 경로로 커밋한다. `<wiki dir>` 은 `moai wiki ls --json` 의 `dir` 이다.
///
/// **`add` 가 앞에 선다**(리뷰 moai-bl3x.vbw 1번). `git commit -- <디렉터리>` 는 git 이 아직 모르는 파일을
/// 안 담는다 — 고친 페이지가 하나라도 있으면 새 페이지만 빠진 채 0 으로 끝나고, 새 페이지뿐이면
/// "nothing added" 로 진다. 새 페이지는 위키 걸음이 가장 자주 쓰는 것이라, 빠진 페이지는 머지에 안
/// 실리고 `git worktree remove` 가 그 untracked 파일로 멈춘다. 두 줄로 두는 것은 `&&` 로 이은 한 줄을
/// 워크트리 세션의 셸 가드가 통째로 거절해서다.
const WIKI_COMMIT: &str = r#"git add -- <wiki dir>
git commit -m "docs(wiki): <what changed> (<epic>)" -- <wiki dir>"#;

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

/// 낱말표의 열 — 스킬을 심는 세 에이전트(moai-xs2h, 2026-10-04 사용자 결정). 차례가 [`Verb::words`] 의 차례다.
pub const VENDORS: [&str; 3] = ["Claude Code", "Codex", "Antigravity"];

/// 그 에이전트에 없는 걸음, 또는 moai 가 아직 모르는 걸음.
const NO_VERB: &str = "—";

/// 벤더 낱말표의 한 줄(moai-xs2h.r2d) — 걸음 하나를 **하는 일**로 이름 짓고, 세 에이전트가 그것을 치는 글을
/// [`VENDORS`] 의 차례로 든다.
pub struct Verb {
    pub step: &'static str,
    pub words: [&'static str; 3],
}

/// 리뷰를 부르는 걸음. 규칙 3 의 글이 칸을 여기서 읽는다 — 표와 규칙에 따로 적으면 한쪽만 고쳐진다.
///
/// **리뷰는 일꾼의 대화형 세션 안에서 돈다**(2026-10-04 사용자 결정, moai-5kk1). Codex·Antigravity 칸에 적었던
/// `codex review`·`agy -p` 는 새 에이전트 프로세스를 띄우는 길이었다 — 감독도 일꾼도 에이전트를 직접 실행하지 않고
/// 헤드리스로 돌리지 않는다. 그 세션에 리뷰 기능이 있으면 그것을, 없으면 일꾼이 스스로 diff 를 읽는다. 두 칸은
/// 같은 글이다 — 규칙 3 은 그것을 한 번 싣는다.
const REVIEW_VERB: Verb =
    Verb { step: "Review the work", words: ["`/code-review`", REVIEW_IN_SESSION, REVIEW_IN_SESSION] };

/// Claude Code 밖의 리뷰 — 그 세션 안의 리뷰, 없으면 일꾼 스스로. [`REVIEW_VERB`] 의 두 칸이다.
const REVIEW_IN_SESSION: &str = "the review this session has, else read the diff yourself";

/// **벤더 낱말표**(moai-xs2h.r2d, 2026-10-04 사용자 결정) — Claude Code 에만 있는 걸음을 세 에이전트의 열로 가른다.
/// 스킬 본문은 하는 일로 말하고 칸을 이 표에서 읽게 한다. 에이전트마다 SKILL.md 를 따로 내면 같은 글이 세 벌이 되어
/// 반드시 갈라진다(이 모듈 머리의 "왜 한 출처인가") — 그래서 세 트리가 같은 글을 받고, 다른 것은 이 표의 열 하나다.
///
/// **칸의 글은 기획 노트(마일스톤 moai-5m2h)의 조사와 사용자 결정에서 왔다** — Codex 의 `request_user_input`·`/new`,
/// Antigravity 의 `/clear`(2026-10-04 사용자 결정, 이 컨테이너에서는 못 쟀다 — 에이전트를 띄우지 않는다). 못 확인한
/// 것은 지어 적지 않고 [`NO_VERB`] 로 둔다: 지어낸 명령은 치면 실패하고, 빈 칸은 표 아래 한 줄(사람에게 알리고
/// 건너뛴다)이 받는다.
///
/// **세션 사이 통신은 표에 없다**(moai-obxm) — moai 는 통신을 안 든다. 깨우기 줄(`moai send --wake`)은 우편함과 함께
/// 걷었고, tmux 밖에서는 창을 비우는 것이 사람의 몫이다(tmux 안의 감독은 [`tmux`] 스킬로 칸을 비운다, moai-u99i).
///
/// 표를 싣는 것은 `moai` 스킬 하나다([`verbs_section`]) — 세 벤더에 다 심기는 스킬이다. 감독 스킬과 그 일꾼 글은
/// Claude Code 에만 가니(moai-obxm) 도구를 바로 적는다.
pub const VERBS: [Verb; 8] = [
    Verb {
        step: "Enter the worktree",
        words: [
            "`EnterWorktree(path)` from the root",
            "`cd` into it and run every command there",
            "`cd` into it and run every command there",
        ],
    },
    Verb {
        step: "Come back to the root",
        words: [
            "`ExitWorktree(keep)`",
            "`cd` to the root and run every command there",
            "`cd` to the root and run every command there",
        ],
    },
    Verb {
        step: "Ask the person watching",
        words: ["`AskUserQuestion`", "`request_user_input`", "ask in the conversation and wait"],
    },
    REVIEW_VERB,
    Verb { step: "Change the model (the person does it)", words: ["`/model`", "`/model`", NO_VERB] },
    Verb { step: "Clear the window (the person does it)", words: ["`/clear`", "`/new`", "`/clear`"] },
    Verb {
        step: "Call a skill (the person does it)",
        words: ["`/<skill>`", "`$<skill>`", "ask for the skill by name"],
    },
    Verb { step: "Stop what a review left running", words: ["`TaskStop`", NO_VERB, NO_VERB] },
];

/// 낱말표 절 — `moai` 스킬이 싣는다(사용자 결정 2026-10-04, moai-obxm 에서 감독·일꾼이 빠졌다). 머리까지 여기서 낸다 —
/// 열의 차례는 [`VENDORS`] 에 있어, 머리를 표면에 따로 적으면 열을 바꿔 끼워도 머리가 엉뚱한 열을 이름 짓는다
/// ([`difficulty_table`] 이 한 번 겪은 자리다).
fn verbs_section() -> String {
    let head = format!("| Step | {} |", VENDORS.join(" | "));
    let rule = format!("|---|{}", "---|".repeat(VENDORS.len()));
    let rows: Vec<String> = VERBS.iter().map(|v| format!("| {} | {} |", v.step, v.words.join(" | "))).collect();
    format!(
        r#"## Words per agent

moai plants this skill for Claude Code, Codex and Antigravity alike, so a step that differs
per agent is named by what it does — each row of this table is one. Each agent types a step
its own way — read your own column.

{head}
{rule}
{rows}

A `{NO_VERB}` is a step that agent does not have, or one moai does not know yet: tell the
person watching and go on without it."#,
        rows = rows.join("\n")
    )
}

/// 규칙 다섯. 제목은 `RULES`, 리뷰 걸음은 `REVIEW_STEPS` 에서 온다.
fn rules() -> String {
    let [one, two, three, four, five] = RULES;
    let steps = indent(REVIEW_STEPS, "  ");
    let make = make_review("--parent <the issue>");
    // **세 칸을 다 싣는다**(리뷰 moai-xs2h.dir 9번) — 이 글은 AGENTS 블록에도 서고, 그 블록만 읽는 Codex 창에는
    // 낱말표가 든 스킬이 안 심겼을 수 있다. 표를 가리키기만 하던 판은 그 창을 없는 표로 보냈다. Codex 와
    // Antigravity 의 칸은 한 글이라([`REVIEW_IN_SESSION`]) 한 번 싣는다 — 둘이 갈리면
    // `the_skills_carry_the_words_table_and_rule_three_points_at_it` 가 빠진 칸을 댄다.
    let [claude, elsewhere, _] = REVIEW_VERB.words;
    format!(
        r#"**1. {one}.** The issue in focus is the one you picked up — it has left the
first column and is not closed yet (`in_progress`·`review`).
Anything that comes out of that work belongs in the same epic (`-e <epic>`) or
under that issue (`--parent <id>`). If the epic {PLEDGE}, it is one of those two
even when you cannot do it now (fork 1). If it is not for now, park it with
`moai backlog add` — a backlog item is always free of this rule, and so is
`moai add --from` (what it creates is an epic and its children, one unit on its own).

**2. {two}.** `moai mv <id> in_progress`.
What counts is work inside the repository — `.moai/`, `.claude/`, `.agents/`, `.codex/`,
`.worktrees/`, `target/` and anything outside the repository (scratchpad, temporary
files) do not. Shell writes (`>`, `>>`, `sed -i`, `tee`) count as much as `Edit` and
`Write`. If it was not in the plan, create it with `moai add 'a title'` and pick that up.

**3. {three}.** Before you call a review, create a review issue tied to
what you are reviewing. The review is {claude} in Claude Code; in Codex and
Antigravity it is {elsewhere}.
It runs inside your own session — never start another agent program for it. The other
steps that differ per agent are under "Words per agent" in the `moai` skill.

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

    {TMUX_OWN}

**5. {five}.** A `moai mv` into a started column on a row whose assignee is
someone else — or nobody — is refused unless it carries `--take`. Ask the person
watching first. On a yes, take it over and say who said yes:

    {TAKE_OVER}

You become the assignee in the same write, and a note `Taken-over: <who it was|none>`
keeps whose it was. `moai ready` hands out only your own rows and sets the rest
apart (`others`), and what someone else picked up is not your focus. When who you
are is unknown, nothing is refused.

**Where no hook stands, the rules are words only.** The hooks stand where
`moai skill install` planted them for your agent — Claude's plugin, Codex's
`.codex/hooks.json` once a person has trusted it in `/hooks`, Antigravity's
`.agents/hooks.json`. An agent without them, and a tool call that sends no hook
(Codex sends them only for its shell, `apply_patch` and MCP calls) or that the
hooks do not read (input typed into a command already running), is refused
nothing — keep the five yourself. Rules 4 and 5 most of all: they guard the
person's other sessions and other people's work, and nothing else will."#
    )
}

/// 남의 줄을 넘겨받는 줄 — 규칙 5 의 글(`rules`)이 싣는다(moai-0zjo). 꼬리는 [`TAKE_YES`] 다.
pub const TAKE_OVER: &str = "moai mv <id> in_progress --from todo --take -m '<who said yes>'";

/// 넘겨받는 줄의 꼬리 — 규칙 5 의 거절문(`hook::take_in`)과 `mv` 의 알림(`cmd::mv`)이 이것으로 줄을 짓고,
/// [`TAKE_OVER`] 가 이것으로 끝난다(`the_take_over_line_ends_with_its_tail` 이 맨다). 손으로 따로 적던 판은
/// 이 상수의 글이 거절문에 닿는다고 적어 두고 실제로는 어디에도 안 닿았다(moai-0zjo 리뷰).
pub const TAKE_YES: &str = "--take -m '<who said yes>'";

/// 시험용 tmux 를 띄우는 줄 — 규칙 4 의 글과 거절문이 함께 쓴다.
pub const TMUX_OWN: &str = "env -u TMUX tmux -L <unique name> …";

/// 안내 전문이 사는 파일(moai-cbfz) — `init --guide file` 이 쓴다. 트래커 디렉터리 안이라 뿌리를 안 어지르고,
/// 추적 여부를 `.moai` 와 함께 따른다.
pub const GUIDE_FILE: &str = ".moai/guide.md";

/// 안내 전문의 첫머리 — 제목과 첫 문장이다. **`.moai/guide.md` 가 moai 의 안내인지 이 글로 알아본다**(리뷰 moai-8gwh
/// 5번): `--guide block` 이 그 파일을 걷을 때 사람이 같은 이름으로 둔 메모까지 지우지 않으려는 것이다. 그 파일이
/// 생긴 판(moai-cbfz)부터 [`agents`] 가 이 두 줄로 열었으니(제목과 첫 문장은 moai-54k2 의 영어 통일부터 그대로다)
/// 어느 판이 쓴 전문이든 이것으로 연다. 바꾸면 옛 판이 쓴 파일을 못 알아봐 남긴다 — 시험이 [`agents`] 의 첫머리와 맨다.
pub const GUIDE_OPENING: &str = "## Issue tracker — moai\n\nThis repository's work lives in `.moai/issues.jsonl`.\n";

/// 링크 블록의 끝줄 머리 — **링크 모드는 이 줄로 알아본다**(리뷰 moai-zynt.63u). 블록이 `.moai/guide.md` 라는
/// 글을 품는지로 가르던 판은 그 경로를 적은 손질 한 줄에 블록 모드 저장소를 링크 모드로 갈아 끼웠다.
pub const GUIDE_MARK: &str = "<!-- moai:guide";

/// `init --guide file` 이 AGENTS.md 블록에 두는 몇 줄. 파일이 없는 자리(그 파일을 안 담은 클론)를 위해 같은
/// 글을 내는 명령을 함께 댄다 — 바이너리만 있으면 어디서든 같은 글을 얻는다.
///
/// **끝줄이 가리키는 전문의 해시를 든다**(리뷰 moai-zynt.63u). 전문이 바뀌면 링크도 바뀌어야 블록의 마커가
/// "어느 바이너리가 쓴 그대로" 를 가린다(`Stale::Binary`, moai-mj45) — 바이너리마다 같은 링크를 두던 판은
/// 새 바이너리가 쓴 전문을 옛 바이너리가 "손으로 고쳤다" 로 읽어, 따라 친 `init` 이 새 안내를 옛 글로 되돌렸다.
pub fn agents_link() -> String {
    format!(
        "## Issue tracker — moai\n\n\
         This repository's work lives in `.moai/issues.jsonl`, and moai is the tool for it.\n\
         Read [`{GUIDE_FILE}`]({GUIDE_FILE}) before you start — it says how to work here.\n\
         If that file is not there, `moai init --print` prints the same text.\n\
         {GUIDE_MARK} hash:{:08x} -->\n",
        crate::text::fnv1a32(agents().as_bytes())
    )
}

/// `init` 이 AGENTS.md 의 마커 사이에 쓰는 블록. **언제나 읽히는 산문이다.**
///
/// 한때 여기에 "정적이라 `bd prime` 같은 명령을 따로 두지 않는다 — `moai status` 가
/// prime 이다" 가 적혀 있었다. `moai prime` 이 서면서 그 말이 틀렸는데(moai-5ok8), **이
/// 블록이 곧 에이전트가 읽는 글이라** 고치지 않으면 도구가 제 명령을 없다고 가르친다.
/// 나뉜 자리는 이렇다 — 보드(`status`)는 사람이 한 화면으로 훑는 것이고, `prime` 은
/// 세션 첫머리와 접힌 뒤에 **다시 주입되는** 짧은 한 판이다. 둘 다 [`CHEATSHEET`] 에 선다.
pub fn agents() -> String {
    let backlog = backlog();
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

### Park what is out of scope

{backlog}

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
4. Park what you find along the way that is out of scope with `moai backlog add` — if the
   epic promised it, it is not a backlog item even when you cannot do it now (fork 1).
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

### Move old closed work into archive files

{ARCHIVE_STORAGE}

### Name the AI that did the work

{WORK}

### The five things the hook actually watches

They stand once `moai skill install` has planted the hooks for your agent.

{rules}

### The supervisor and its workers

In Claude Code, `moai skill install` also plants `moai-supervise`. A person calls it in
one window to hand the backlog that have piled up, one at a time, to the other sessions
of this repository and take their reports. Every idle session of this repository that
`ListAgents` shows is a worker — nobody registers. The supervisor sends each one its
assignment (`SendMessage`) with a line naming the file of the worker's steps to read, and
the worker reports the same way. moai carries no messaging, and the moai binary never
launches or drives a session. When the supervisor runs inside tmux, its companion skill
`moai-tmux` lets it clear a worker's pane, paste a message into it, and open new worker
panes — an ordinary interactive `claude` the person sees, opened only after the person says
yes. That skill is planted only where it is chosen: `moai skill install --with moai-tmux`
plants it and `--without` takes it out. Nothing runs headless. The supervisor picks, sends and checks; it does not fix and it
does not merge.

### The wiki

{WIKI}

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

## The five things the hook actually watches

{rules}

{verbs}

## Before you close the session

{CLOSING}

Every command and the `--from` syntax are in `references/commands.md`.
"#,
        rules = rules(),
        verbs = verbs_section()
    )
}

/// 스킬의 참고 문서. 부를 때만 읽힌다.
pub fn reference() -> String {
    let backlog = backlog();
    format!(
        r#"# Every command

`moai --help` and `moai <command> --help` are the truth. This file is a summary of
them, so where they differ the help wins.

## There are two kinds of group

{GROUPS}

## Filters

A comma means "or"; the same flag twice means "and" — except `-a`, where twice
means either one, since a row has one assignee.

    moai show -s todo -t bug          todo and bug
    moai show -s todo,review          todo or review
    moai show -a raven -a joshep      raven's or joshep's
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

In shell, as one turn of a loop that takes the next row each time round, it is this —
`col` is the column of that row as `ready` gave it, and `moved` says whether you got it.

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

`--dry-run` keeps a heredoc typo from creating six of the wrong things. It also says
which epic a `--body` would land on, and refuses a body the write would refuse.

`--body` says why these issues are one bundle. It goes onto the first epic the plan
creates, which is where `moai show <epic>` reads it from. `--body <text>` takes the
text itself — a file path there becomes the body as written — `--body -` reads
stdin, and only `--from <file>` reads a file, so a file goes in as the body with
`--body - < <file>`. `--body -` and `--from -` cannot both read stdin: put the plan
in a file and stream the body, or keep the plan on stdin and pass the body as text.
`moai add --from plan.md --body -` and `moai add --from - --body '<text>'` both work.

Keep a plan you repeat in a file and fill `{{{{name}}}}` with `--var name=value` (the
name takes letters, digits, `_` and `-`, no spaces). By convention it lives in the
repository at `.moai/templates/<name>.md` and is called with `--from <path>`.
Every variable is required — a name you did not fill, an empty value, a value with a
newline in it, a name not in the plan, and the same name twice are all refused and
create nothing. The value becomes the title text exactly as written, so put variables
in title positions only (in a tag or priority position it is refused). To put a literal
`{{{{` in a template title, write `\{{{{`. A backslash immediately before `{{{{` is counted
in pairs — for a literal backslash followed by a variable, write `\\{{{{name}}}}`
(`C:\\{{{{dir}}}}`). `backlog promote --from` takes the same `--var`.

    moai add --from .moai/templates/release.md --var version=1.2 --dry-run

## Several projects

{PROJECTS}

## The language on screen

{LANGUAGE}

## Checking for a new release

{UPDATES}

## Unfolding a parked thought

{backlog}

## Deferring

{DEFERRING}

## People

{PEOPLE}

## What to write in an issue — an example

{WRITING_EXAMPLE}

## Name the id in the commit

{COMMITS}

## When the issue file has to be merged

{MERGE_DRIVER}

## Move old closed work into archive files

{ARCHIVE_STORAGE}

## Name the AI that did the work

{WORK}

## Statistics

{STATS}

## The wiki

{WIKI}

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

    moai hook <event>    An agent's hooks call this. A person never runs it by hand

The hooks `moai skill install` plants are what call it — Claude's plugin, Codex's
`.codex/hooks.json` and Antigravity's `.agents/hooks.json`, each with its `--dialect`.
Whatever goes wrong the exit code is 0 — a noisy hook gets turned off, and a rule that
is off is no rule.

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
"#
    )
}

/// 셋째 스킬 `moai-wiki` 의 SKILL.md — 저장소의 매뉴얼(위키)을 개발 진척에 맞춰 고치는 걸음이다
/// (moai-bl3x, 사용자 결정 2026-10-04).
///
/// **부르는 자리가 둘이다.** 일꾼이 에픽 끝(브리프 7-4)에서 그 에픽이 사람의 쓰임을 바꿨는지 묻고
/// 고치는 것이 주된 길이고, 사람이 부르면 지난 릴리스 뒤 닫힌 에픽과 에픽 밖 이슈를 훑는다. 브리프의 7-4 는 짧게
/// 두고 본문은 여기 둔다 — 브리프는 일꾼이 매 바퀴 통째로 읽는 글이라 한 줄이 일꾼 수만큼 값을 낸다.
///
/// **아무것도 막지 않는다.** 페이지가 안 고쳐졌다고 붉어지는 자리를 만들면 그것이 게이트고, 글이
/// 릴리스를 세운다. 그래서 "하지 않는 것" 에 게이트를 첫 줄로 적는다.
///
/// **`--json` 의 키 이름은 위키 저장 에픽(moai-ihu4)이 정한 모양이다.** 그쪽이 키를 바꾸면 이 글이
/// 없는 키를 가르친다 — `the_wiki_skill_names_the_wiki_commands` 가 이 글이 대는 키를 그 목록과 견주고,
/// **그 목록은 실제 출력에 묶여 있다**(moai-ihu4.l2e): `the_wiki_skill_teaches_the_keys_the_wiki_prints` 가
/// 명령과 같은 자(`cmd::wiki::listed_json`·`page_json`)로 지은 `--json` 에서 키와 `kind` 값을 읽어 두 쪽을 견준다.
/// 명령이 키를 바꾸거나 더하면 거기서 붉어진다.
///
/// 발동어는 [`skill`] 과 같은 까닭으로 두 말을 함께 싣는다.
pub fn wiki() -> String {
    let [one, two, ..] = RULES;
    let commit = indent(WIKI_COMMIT, "       ");
    format!(
        r#"---
name: moai-wiki
description: Use when the repository's manual (its wiki) has to catch up with what the work changed — at the end of an epic, or when a person asks. "update the wiki", "write the manual", "document this", "위키 갱신", "매뉴얼 써", "문서화해 줘", "wiki 정리". The pages are markdown files in the repository, listed by `moai wiki ls`; nothing checks them.
---

# moai-wiki — keep the manual in step with the work

The wiki is the repository's manual: markdown pages committed beside the code. It
follows the work — **when an epic changes what a person does, the page that teaches
it changes in the same merge.** Nothing checks this and nothing is blocked. An epic
that changed nothing a person does writes nothing.

## What the wiki is

    moai wiki ls                           the pages: slug and title, and what does not resolve
    moai wiki ls --json                    the same with each page's path, for a machine
    moai wiki show <slug>                  one page, and the pages that link to it

- **One directory.** `dir` in `moai wiki ls --json` names it — `docs` unless
  `wiki_dir` in `.moai/config.toml` says otherwise. When it cannot be read, `error`
  stands in place of `pages`, and its `kind` says why: `no_dir` is a repository with
  no wiki yet, `outside` and `not_a_dir` are a `wiki_dir` pointing where moai will
  not read — fix the key, not the pages — and `failed` is the disk refusing to open it
- **One page is one `.md` file** — `path` in the list. Its `slug` is that path below
  `dir` without `.md`. Name a new file in lowercase ASCII kebab case
  (`merge-driver.md`) so its slug reads the same on every machine. `README.md` or
  `index.md` at the top of `dir` is the home page
- **One `# Title` line opens the page.** It is the `title` the list shows; without it
  the file name stands in
- **Pages link with plain relative links** — `[the explorer](explorer.md)`. Not
  `[[wiki links]]`: GitHub does not draw them, and one link with two spellings is one
  vocabulary too many
- **A link can land on a heading** — `[epic](glossary.md#epic)`, or `[above](#epic)`
  on the same page. The part after `#` is the heading's anchor as GitHub makes it:
  lowercase, punctuation dropped, each space a `-` (`## The --json contract` is
  `#the---json-contract`), and a repeated heading `-1`, `-2`. The same link lands on
  that heading on GitHub and in the explorer's wiki window. Link to the heading that
  says it, not to the top of a long page
- **An issue is named by its bare id**, as a whole word — not a link. moai finds it
  in prose and in inline code; an id inside a fenced or indented code block is read
  as an example and is not counted
- **The pages ride the branch.** Unlike `.moai/`, nothing moves them to the main
  checkout: a page written in a worktree stands in that worktree and is merged with
  the code, by git's own 3-way merge

Every row under `pages` also says what to fix: `issues` (each `id`, and `exists`
false for an id that names no issue), `links` (the links to other pages — each `text`,
`to` as the slug it lands on, and `resolved` false when no such page stands; a link
with a `#` also carries `anchor`, the part after it, and `anchor_resolved` false when
that page has no such heading — absent when the page could not be read, so nobody can
tell — and a same-page `#anchor` stands with `to` naming its own page) and
`conflict` (true while merge conflict markers stand in the page). `linked_from` turns
`links` around: the slugs of the pages that link to this one, in list order, `[]` when
none does. A page other than the home page that nothing links to is found only through
the list — link it from the page that should lead there. Judge that from
`moai wiki ls --json`, not from one page: a page that could not be read links nowhere,
and only the list shows it, with its `error`. `moai wiki show <slug> --json` says when
its own count may be short — `linked_from_partial` stands `true` when some other page
could not be read or the walk left a spot out, and is absent when every page was
counted. A page that could
not be read still stands in the list, under its file name, with an `error` of its own
whose `kind` says why — `too_large`, `refused` or `failed`. What the walk had to
leave out stands under `skipped`, each with its `path` and a `kind` — `dir_link` (a
link to a directory, not followed), `not_utf8` (a file name that cannot be a slug) or
`unreadable` (a directory it could not open or list to the end, or a name whose kind it
could not read); with nothing left out the key is absent,
and with it the exit code is non-zero although `pages` is whole. Branch on `kind`, not
on the words beside it. Look at all of these after you write.

## Two kinds of page

- **Reference — generated.** A page whose first paragraph says it is generated and
  not to be edited by hand ("Generated from … Do not edit by hand") is written by
  the generator that paragraph names. Never edit it: when its source changed, run
  the generator and commit what it wrote
- **Guides — written by hand.** A guide teaches what a person does: what they came
  to do, the steps in order, why, and what goes wrong. **Do not copy `--help` into
  a guide** — name the command and say when to reach for it. The help is the truth
  for flags, and a copy goes stale the day a flag changes

## At the end of an epic

Most pages are written here — the window that did the epic is the only one that knows
what changed and why. It runs on that epic's branch — in its worktree, where it has
one — after the review and the CHANGELOG line and before the merge.

1. Read four things: `moai show <epic> --json` (the body and the notes say why it was
   decided), the CHANGELOG line the epic wrote if the repository keeps one, the
   `--help` of each command the epic touched, and `moai wiki ls --json`
2. Ask once: **did this epic change what a person does** —
   {CHANGED_USE}. A refactor
   inside, or a fix that left the behaviour as it was, changed nothing a person
   does. If nothing changed, stop — write nothing
3. Change the page that teaches it — find it in `moai wiki ls`, or grep `dir` for the
   command or the key. If no page covers it, write one from the template below. A
   page ends with one `Decided in:` line naming the epics whose decisions it carries;
   put this epic's id on it
4. Commit on that branch, before the merge — the pages then ride the same merge, and
   the merge diff is where they get read

{commit}

   `<wiki dir>` is `dir` from `moai wiki ls --json`. **Do not skip the `add`**: with a
   path, `git commit` leaves out a file git does not know yet, so a new page stays
   behind without a word
5. Name the pages you changed in your report, or say that none changed

## When a person asks you to sweep

A person calls this skill to catch the wiki up — before a release, or after work
merged without touching it.

1. Find when the last release went out — `git tag --sort=-creatordate` lists the tags
   newest first; take the newest release tag, and `git log -1 --format=%cs <tag>`
   gives its day. Not `git describe`: it sees only the tags this branch can reach,
   and a release tagged on another branch (a `main` that only releases merge into)
   is not one of them. With no tag, ask the person how far back to go
2. List the epics closed since the day before that, and the issues closed outside any
   epic — `%cs` is the day on the tag's own clock and `--done` reads yours, so a day
   earlier keeps what closed in between; one row too many is only one more to ask about

       moai show --type epic --done '<day>..' --json
       moai show --type issue -e none --done '<day>..' --json

3. Ask each of them the question from the end of an epic (step 2 there), against
   the pages `moai wiki ls --json` lists, and gather what to change — one line each:
   the page, what changes in it, which epic or issue
4. **Show the person that list once** and wait for a yes. They may cut it
5. Pick the work up before you write — the pages are files in the repository, so
   the hook counts them as a change, by rule 2:
   "{two}"

       moai add 'wiki: <what>' -t docs
       moai mv <id> in_progress

   If you still hold other work, the hook refuses that `add`, by rule 1:
   "{one}". The sweep is not part of that work, so
   finish it first or ask the person
6. Write the pages where this repository does its work — in a worktree if it uses
   them — commit with `docs(wiki): <what> (<id>)`, a `git add` first as in step 4 of
   the end of an epic, and move the issue to `done`

## A new page

```markdown
# <Title — what the reader came to do>

<One paragraph: what this page is for and when to reach for it.>

## <A task>

<The steps in order. Each command in a code span, with what it is for — not its flags.>

## When it goes wrong

<What the reader sees, and what to do about it.>

Decided in: <epic>
```

## What this skill does not do

- **No gate.** Nothing checks that a page was written, and no warning stands for a
  stale one. Do not add a check — a check here is a gate, and a release does not
  wait on prose
- **No page per epic.** Pages follow what a person does, not the order things were
  built in. The history is the tracker and the CHANGELOG
- **No to-do list in the wiki.** Work not done goes into the tracker — `moai add`, or
  `moai backlog add` for later
- **No token counts.** What did the work and what it cost is a note on the issue,
  never a page — and never an estimate
"#
    )
}

/// 둘째 스킬 `moai-supervise` 의 SKILL.md. 같은 저장소에서 놀고 있는 세션에 backlog 를 하나씩 나눠 주고 보고를 받는
/// 감독의 걸음이다 (moai-hxma).
///
/// **Claude Code 에만 심는다**(2026-10-06 사용자 결정, moai-obxm). moai 는 세션 사이의 통신을 안 든다 — 우편함
/// (`moai send`·`moai inbox`)과 출석(`moai hello`·`moai agents`)을 걷었다(moai-5uwh). 감독과 일꾼은 그 벤더의 제 수단으로
/// 말하는데, 그것이 알려진 것은 Claude Code 의 `ListAgents`·`SendMessage` 뿐이다. Codex·Antigravity 에는 같은 수단이
/// 알려지지 않아 감독 스킬을 안 심는다([`crate::skill::Skill::claude_only`]).
///
/// **일꾼은 이 저장소의 놀고 있는 세션 전부다** — 감독 자신만 뺀다. 등록도, 사람에게 어느 창인지 묻기도 없다. 출석부를
/// 따로 두면 그것이 `ListAgents` 와 어긋나는 둘째 진실이 된다.
///
/// **일꾼의 걸음은 파일이고, 메시지는 그것을 읽으라고만 한다**([`worker`], 이 스킬의 `references/worker.md`,
/// 2026-10-07 사용자 결정, moai-fim6). 일꾼 창도 같은 저장소의 같은 플러그인을 읽어 그 파일이 거기 있다 — 감독은 제
/// 스킬의 기준 디렉터리("Base directory for this skill")에 `/references/worker.md` 를 붙인 절대 경로를 적는다.
/// Claude Code 는 디렉터리 마켓플레이스의 플러그인을 **그 자리에서** 읽는다 — 제 캐시의 사본에는
/// `references/worker.md` 가 없으니 그 사본을 읽는다고 보고 경로를 짓지 않는다. 그래도 `.claude/moai-plugin` 을 박지
/// 않는 것은 그 자리가 설치 범위와 저장소에 따라 달라서다 — 스킬이 읽힌 자리는 Claude Code 가 알린 그 디렉터리
/// 하나다. 글 전부를 붙여
/// 보내던 판은 32KB 를 보낼 때마다 손으로 옮겨 출력 토큰 8~10k 가 들었고, 줄이거나 바꿔 옮긴 글을 아무것도 못 잡았다.
/// `@path` 는 아무것도 안 붙이므로 경로는 일꾼이 `Read` 로 연다.
///
/// **tmux 밖에서는 창을 비우는 것이 사람의 몫이다.** tmux 칸에 `/clear` 를 쳐 넣던 5-1(약 480줄)을 통째로 걷었다 —
/// 일꾼이 보고 끝에 언제 비워도 되는지를 사람에게 한 줄로 말한다(moai-obxm).
///
/// **tmux 안의 감독은 칸을 만진다**(2026-10-10 사용자 결정, moai-u99i). `$TMUX` 가 서 있으면 [`tmux`] 스킬을 읽고,
/// 보고를 확인한 일꾼의 칸을 비우고(5), 닿지 않은 메시지를 칸에 붙이고(3), 멈춘 칸을 한 번 읽고(4), 놀고 있는 일꾼이
/// 없으면 사람에게 물어 새 칸을 연다(2). 옛 5-1 이 초안을 건지느라 480줄이 된 것을 되풀이하지 않으려고, 입력 칸에
/// 무엇이든 있으면 치지 않고 사람에게 말한다 — 그 하나로 둔다. **바이너리는 여전히 아무것도 띄우거나 몰지 않고**,
/// 헤드리스·`-p`·`--dangerously-*`·권한 모드 바꾸기도 없다. 연 칸은 사람이 보고 칠 수 있는 보통의 대화형 `claude` 다.
///
/// **backlog 를 일감으로 바꾸는 길은 `promote` 하나다** (사용자 결정). `moai edit` 에 `--type` 이 없어 제자리에서 못
/// 바꾸는데, 첫 실행의 일꾼은 `add` 로 새 줄을 세우고 backlog 를 손으로 닫았다. 길이 둘이면 일꾼마다 다르게 고르고,
/// `add` 는 집은 것이 있는 세션에서 규칙 1 에 걸린다. `promote` 는 backlog 를 저절로 닫고 출처를 저널에 남긴다.
///
/// **모든 저장소에 심긴다.** 이 저장소의 이슈 id 를 글에 적지 않고, 가지 이름을 박지 않는다 — 감독이 루트
/// 체크아웃의 지금 가지를 읽어 `<root branch>` 에 채운다(사용자 결정, moai-7ljm — 마일스톤 가지 전에는 `<base branch>` 였다). 일꾼이 뜨고 병합하는 곳이 그
/// 체크아웃이라 원격 기본 가지보다 덜 어긋난다. 루트가 detached 면 감독이 보내지 않고, 바퀴 사이에 루트의 가지가
/// 바뀌면 일꾼이 루트 커밋·병합 직전의 대조(`BRANCH_CHECK`)로 멈춘다(moai-gokz).
///
/// **마일스톤 일은 마일스톤 가지에서 뜨고 거기로 병합한다**(2026-10-08 사용자 결정, moai-nvju). 그래서 감독이 읽는
/// 루트의 가지는 `<root branch>` 이고, `<base branch>` 는 일마다 갈린다 — 마일스톤 안의 일은 `milestone/<milestone>`,
/// 밖의 일(`p0` 따위)은 루트의 가지. 병합 확인(5)도 그 일의 `<base branch>` 를 본다 — 마일스톤 일의 병합은 릴리스 전에는
/// 루트의 가지에 없다. `milestone/` 은 가지 이름이 아니라 이 규약의 앞붙이라 `the_brief_names_no_branch` 에 안 걸린다.
pub fn supervise() -> String {
    let message = message();
    let table = difficulty_table();
    let epic_rule = epic_review_rule();
    format!(
        r#"---
name: moai-supervise
description: Use in Claude Code when handing the backlog piled up on one repository, one at a time, to the idle Claude Code sessions of that repository as workers, and taking their reports. Triggers on "supervise", "hand out the backlog", "put the idle sessions to work", "감독해 줘", "backlog 나눠 줘", "놀고 있는 세션에 일 시켜".
---

# moai-supervise — hand backlog out to the idle sessions of this repository

The supervisor **picks, sends and checks.** It does not fix code, it does not merge,
and it does not settle design in a worker's place. The workers merge. Overlapping
merges are prevented by splitting the files when the supervisor sends (1), and where
they still collide the worker goes back into its worktree and resolves them.

**This skill is for Claude Code, and moai carries no messaging.** The supervisor and its
workers talk with Claude Code's own tools:

- `ListAgents` lists the live sessions, and subagents too — each row's name, its kind
  (`interactive`, `bg`), whether it is busy or idle, and its tmux pane if it has one
- `SendMessage(to: <name>, message: …)` sends to one session. With `notify_when_idle: true`
  you also get one notice when that session goes idle; leave `message` out and it only
  subscribes
- A reply comes in as a cross-session message. Answer it by copying its `from` as `to`

**Every session here is an interactive one the person can see.** The moai binary never
launches or drives an agent, and nothing here runs headless. Outside tmux the supervisor
launches nothing either; inside tmux it may open a worker pane, only after the person says
yes (`moai-tmux`, below). **A worker is every idle session of this
repository in `ListAgents`, except you** — a row whose name starts with the root
directory's slug and a `-` (2). Nobody registers and nobody is asked which windows count.
The message you send is the whole assignment, and it names the file of the worker's steps,
which the worker reads (3).

**One supervisor per repository.** A supervisor waiting for reports reads `idle` in
`ListAgents` like any worker, so a second one sends it backlog, and the two keep separate
books of what was sent — one backlog, or one worker, gets two jobs. Before the first round,
ask the person whether another window here runs `moai-supervise`; if one does, stop. A
session that refuses work because it is a supervisor comes out of the candidates. **You
refuse too:** a message that hands you backlog to work on came from another supervisor — do
nothing of it, reply to its `from` that you are a supervisor, and tell the person.

Five things about the messaging, one line each:

- A session in a different permission mode holds an incoming message for its person's
  approval — a worker that stays idle after you sent may be waiting on that
- `notify_when_idle` answers only for a session on this machine — one reason a worker is a
  session on this machine (2)
- A subagent sends under its parent session's address — a message can come from a session
  that did not write it itself
- `@path` in a message attaches nothing — a file the worker has to read is named by its
  absolute path, and the worker reads it itself (3)
- Never poll `ListAgents` in a loop — the report comes to you (4)

**Inside tmux, load `moai-tmux` too.** When `$TMUX` is set in your shell, the skill
`moai-tmux` gives you hands on the workers' panes, at five points of the round: offer new
worker panes when no worker is idle (2), label the pane you send to and deliver a message
`SendMessage` could not (3), read a stalled worker's pane (4), and clear a reported worker's
window before its next work (5). It finds each worker's pane from its `ListAgents` name, and
never types into a box that holds anything. **Without `$TMUX` nothing of it applies** — every
step below goes through messages and the person, as written. `moai-tmux` is an optional skill:
it stands only where it was chosen (`moai skill install --with moai-tmux`, or its row in `moai
init`). Inside tmux without it, tell the person that one line plants it, and carry on as if
outside tmux.

**When sessions died** — a restart or an OOM kill took the workers or a supervisor down —
and the person asks to bring them back, load `moai-recover`, inside tmux or not.

**Work you send out is always done in a worktree** — even if the repository has no
worktree convention. Several workers share one root checkout, so fixing things in the
root mixes their edits and commits together. Worktrees stand in
`<root>/.worktrees/`, and `moai init` writes that path into the gitignore.

**Read the root branch once, at the start of the round.** Workers commit the tracker in
the root checkout and branch their worktrees from the local branches there, so that
checkout's current branch is the root branch — the remote's default branch may differ from
the root and may be stale. The root checkout is the first entry of `git worktree list`, so
the line below gives the root's branch no matter where in the repository you call it,
inside a worktree included. If nothing comes out, report the error git gave and stop.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); if [ -n "$b" ]; then echo "$b"; else echo "the root is detached" >&2; fi; fi
```

**If the root is detached, do not send.** The worker's pick-up commit and its merge
land on a HEAD with no branch, the check (`merge-base`) and `worktree add`
fail, and `branch -d` deletes that work's only reference. Do not read the remote's
default branch instead — the root does not stand on that branch, so it is the same
accident. Ask the person to put the root on a branch, and stop.

Fill the name you read into `<root branch>` in the message you send the worker.

**Each work has a base branch** — the branch its worktree splits from and merges back
into. It is decided per work, by the release the work stands under — the one
`moai show --milestone` lists it under, read in 1:

- **Outside every milestone** — it stands under none, or under one that has shipped or been
  deferred (a `p0` fix, say) — it is the root branch, as it always was
- **Inside a live milestone** it is that milestone's own branch, `milestone/<milestone id>`,
  even while nothing runs yet and `<milestone>` in 3 says `none` — the first epic sent is
  what starts it. Every epic of the milestone merges there, and the root branch takes the
  milestone branch in once, at the release, so what a milestone has not shipped yet does
  not stand on the root branch. The branch is checked out in a long-lived worktree of its
  own, `.worktrees/milestone-<milestone id>`, because the root stays on the root branch. The
  worker handed the milestone's first epic raises it when it is missing (its "The milestone
  branch") — you do not raise it, and you do not remove it; it goes after the release

Fill that into `<base branch>` in the message and in the check of 5. For stalled work (0)
it is the release its epic stands under (`moai show <epic>`). **The worker does not
read either again** — asked inside a worktree, git answers with that worktree's own branch.

## One round

**0. Reclaim first — work that lost its place.** When a session dies the row it picked
up stays `in_progress` and nobody carries it on. Look at this before picking new backlog.

    moai status --json                     the ids of warnings whose kind is "stranded"
                                           (inside a worktree it stands only with `--worktree`)
    moai show <id>                         the `Place` line — one of the four words below
                                           (inside a worktree this too needs `--worktree`)

`stranded` is a row that was picked up while no live worktree holds that work — either
the worktree is gone, or **the work was being done in the root with no worktree**. This
row alone does not tell the two apart: if `ListAgents` shows a `busy` session of this
repository, it may be that one, so ask what it is holding before handing the work on —
a worker with a message, any other window through the person.

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
died does not show under `stranded`** — it is a worktree in `git worktree list` whose
worker — the session you sent that work to — no longer answers. **A name gone from
`ListAgents` is not an ended session**: the name belongs to the process, so a window resumed
with `claude --resume` comes back under a new name, still in that worktree. A session that
still stands there, idle, has not ended either: its person may be answering it, or it may be
holding your message for approval. Look for the worker by its worktree, not its name — ask
the person which window works in it. Hand its work on
only once the person says that window has ended; until then it is that worker's. When it
comes back under a new name, move what you keep under the old one — the work you sent, a
refusal — to the new name (2).

- When there is such work, hand carrying it on to one idle worker **before any new
  backlog**. Send the message in 3 with its first two lines changed to the two below, and
  the rest filled as 3 says (`<other work>` too — 4-3 points at that line). The steps file
  has the section the first line names

      You are handed the stalled work in <epic> — the previous session did not finish it. Read <steps file> and follow its steps from "Carrying on stalled work".
      Read first: moai show <epic> (history and notes) · moai show <member> (the place too — a place stands on work only)

- **Whether it is carried on or put down is not the supervisor's call.** If it looks like
  work to put down (`moai mv <id> todo`, `moai defer <id> -m '<why>'`), ask the person
- Work handed on to be carried is, like a backlog item, not sent again until its report is checked

**1. Pick.** Out of the backlog that have piled up, keep only the ones that do not collide
with what is open right now.

    moai backlog ls                           what has piled up
    moai show -s in_progress,review        what is picked up
    moai show <id>                         what that backlog touches

Look at `git worktree list` too. A backlog item that touches the **same files, the same area**
as a worktree already standing or an epic already picked up comes out of this round —
when two of them change the same place, one waits for the other at the merge. **Compare
the backlog you send in this same round against each other too** — a worker only raises
its worktree after it receives the work, so what you just sent is not in the lists above
yet. Do this count again for every further backlog. An epic left open with only
first-column members (what the worker's 7-1 left behind) shows as `in_progress` but is not
picked up — there is no worktree and no picked-up member, so do not drop backlog over it.

**If a milestone is running, what is inside it comes first.** The line `moai ready` prints
under its list says what is running and how many it held back outside it, and the
`moai status` notice shines on the same thing. Then what you send this round is work
attached to that milestone — a backlog item from outside waits for the next round unless it
should stand as `p0`.
**The tool does not block this** (a pick-up goes straight through), which is why the
place to decide is here. If two milestones are running, both are inside.

**Work is never pulled into a running milestone — the supervisor does not bring an
outside backlog in.** `moai backlog promote` carries over the body and the release the backlog
stands in — the one `moai show --milestone` lists it under, not its own field — so an
backlog parked outside the release unfolds into an epic that stands outside it, and there it
stays. What you send while a release runs is work that already stands in it; a backlog item from
outside waits for the next round, unless it should stand as `p0` or the person attaches
the release themselves. **So `<milestone>` in 3 is the release that backlog already stands
under, never one you picked for it**: the line the worker runs in its step 1 —
`{MILESTONE_ATTACH}` — re-affirms what `promote` carried and is not a door you open. With
nothing running, and for a backlog item that stands under no release, it is `none`.
The 2026-09-21 round is why both halves are written down: a worker picked up a row outside
the running release, and the person, not the tool, is what caught it. The answer is to
stop sending outside work while a release runs, not to hang the release on it — hanging it
on would make the release grow after it started, and that is the person's call alone.
**The tool refuses none of this**, so this paragraph is the only thing holding it.

**A backlog item you sent comes out of the candidates until its report is checked.** Until the
worker unfolds it, it stays in `moai backlog ls`, and the same backlog goes to a second worker.
So the moment you send it, mark the row you sent — the backlog item, or the epic when the work
is already unfolded — and take the note into the root with a commit with a path. Without it
the send lives only in this conversation, and a supervisor that starts again (4) cannot see it.

    moai note <id> 'Sent: <worker>'

**Send only what is yours.** A backlog item or member whose assignee is someone else — or
nobody — is asked about first: ask the person, and send it only on a yes, writing in the
message who said yes so the worker takes it over (`--take`, hook rule 5). `moai ready` sets
such rows apart under `others`.

**2. Find a worker.** Call `ListAgents` once. A worker is a row that

- belongs to this repository. `ListAgents` shows no directory; a session takes its name from
  the directory it was opened in, slugged — lowercased, every run of characters other than
  `a-z` and `0-9` turned into one `-`, cut at 4 words or 40 characters — then `-` and a short
  hex suffix: `moa-issue-bc` for `moa-issue`, `tvshop-updater-ca` for `tvshop_updater`. A row
  whose name does not start with the root's slug and a `-` — renamed, or opened somewhere
  else — is not one. **A name that does start so is still only a candidate**: `api-gateway-1c`
  starts with `api-`, and a session opened in a clone named `moai-web` starts with `moai-`.
  The worker confirms it: the message carries `Root:`, and a session standing in
  another repository refuses the work, so it comes out of the candidates (below)
- is a session a person opened — under "Peer sessions" and `interactive`. Not a subagent,
  yours or another session's (they stand under "Subagents", and a message to one resumes that
  subagent instead), and not a `bg` session
- runs on this machine — a Remote Control or cloud session cannot read the steps file at the
  path you name (3), and sends no idle notice
- reads `idle`
- is not you, and not a supervisor (one per repository, above)

Its name is what you send to. Nobody registers: any idle session a person opened here is a
worker, and the message is the whole assignment.

- **Leave out a worker whose sent work has not had its report checked.** It goes idle
  whenever its turn ends — while it asks its person something, say — and it is still
  holding your work. A resumed window comes back under a new name (0): while a worker you
  sent to is gone from `ListAgents` with its report unchecked, a name you have not sent to
  may be that worker — ask the person before sending to it
- **If no row is left, nobody is free here.** Tell the person, and stop — do not send to a
  session of another repository. Inside tmux, ask the person first whether to open new worker
  panes (`moai-tmux`, "No idle worker left"); on a no, stop
- **A worker that refused the work comes out of the candidates and is not sent to
  again.** Some sessions take work only from their own person
- **A test agent is no worker.** One raised for a test is opened outside the repository (a
  scratchpad), so its name is not this repository's

The root checkout and, for a subdirectory project in a monorepo, the path down to it come
from the lines below. The root is where `.moai` stands, so for a monorepo it is the
subdirectory that has `.moai`, and a worktree stands for the whole repository — the worker
has to go into the same subdirectory inside it (its step 3). The first line is `root dir`;
a `subdir` line stands only when the root is not the top of the repository.

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import os, sys
if not sys.argv[1]:
    sys.exit("call this inside a git repository")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
print("root dir", root)
if os.path.relpath(here, top) != ".":
    print("subdir", os.path.relpath(here, top))
PY
```

**2-1. Pick a model — by one word of difficulty.** The grade of the epic-end review
(the worker's 7) is measured on this same rubric, member by member. Keep two axes and the
message carries two sets of judgement, and on the day they differ the cheap model takes the
write path.

{table}

The epic-end review is that epic's only review, because the members are not reviewed
separately (the worker's 5 and 7).

{epic_rule}

The supervisor picks before reading any code, so this is a suggestion; the last word
belongs to the worker who read the issue. A running session's model cannot be changed by
a message and cannot be changed by config — the person in that window changes it with
`/model`.

**3. Send.** Send **one** backlog to one idle worker. The message is the lines below, every
slot filled. The worker knows nothing of this conversation, so what is not in the message
does not reach it — except the worker's steps: they stand in this skill's
`references/worker.md`, and the first line tells the worker to `Read` that file and follow
it. The worker is a session of this same repository and loaded the same plugin, so the file
is there for it. **Do not copy the file into the message** — name it.

    SendMessage(to: <worker>, message: <the lines below>, notify_when_idle: true)

`<steps file>` is that file's absolute path: this skill's base directory — Claude Code shows
it as "Base directory for this skill" when the skill loads — followed by
`/references/worker.md`. Write it out whole; `@path` attaches nothing.
**If that base directory lies outside `<root>`** — the plugin installed at user scope, in
Claude Code's plugin cache — the worker's read of it is a read outside its working directory,
and in the default permission mode Claude Code asks its person first; the plugin cache is no
exception. A worker whose person is away waits on that prompt and sends nothing. Tell the
person once, before the first send, and let them choose: a person in the worker's window
answers it, or `permissions.additionalDirectories` in their settings holding that plugin
directory lets it through. The settings are theirs — do not write them.
Inside tmux, label the worker's pane as you send and, if the message does not arrive, deliver
it into the pane — `moai-tmux`, "Label the pane" and "When a message does not arrive".
Fill in `<id>`, `<title>`, `<steps file>`, `<root branch>`, `<base branch>`, `<milestone>`, `<model>`, `<difficulty>`, `<why>`, `<other work>`, `<root>`, `<person>` and — only for a subdirectory project — `<subdir>`.
`<root>` is the `root dir` from 2. **Leave it unfilled** and the worker, inside its worktree,
reads its own place as the root. With no `subdir` line in 2, leave the `Subdir:` line out
of the message.
`<milestone>` is the release that backlog already stands under **and that is still alive**,
read in 1 — `none` when it stands under none, `none` when the one it stands under has
shipped or been deferred (the worker would otherwise re-open a release that is already
out, which is what `promote` itself declines to carry), and `none` when nothing is
running. It is never a release you picked for it: work is not pulled into a running
milestone (1). **Leave it unfilled** and the worker hangs the placeholder itself on the
epic, which the tool refuses because it is not an id at all. **A wrong id it does not
refuse** — the check is the shape, not whether that milestone stands, so a stale one goes
in with exit 0: one line on stderr says there is no such milestone, and `moai status`
counts the epic as `dangling_milestone`. Copy it off
{MILESTONE_FROM}; do not write it from memory.
`<model>`, `<difficulty>` and `<why>` are the pair you picked in 2-1 and your reason.
**Leave them unfilled** and those placeholders travel as they are, so the note the worker
leaves when it closes says `<model>` instead of what actually did the work.
Do not use a single quote inside `<why>` — it closes the single quote in the worker's 9-1
and the rest of the text leaks into the shell.
`<other work>` is the sibling worktrees you measured in 1, the work you send in this same
round, and the files that work holds — `none` if there is none. What the supervisor
measured before sending cannot cover a file that turns out to be needed mid-epic, so when
the worker meets such a file it does not fix it: it leaves it as a member and reports it
(its 4-3).
`<person>` is `here`, or `away` when the person told you they are stepping away — the
worker then settles a design question by its own recommendation instead of waiting on an
answer, writes down what it decided, and stops at what cannot be undone.
Do not fill `<grade>` — that is the review grade the worker picks in 7, after developing.
Do not fill `<vendor>` or `<count>` either — those are the vendor and the token count the
worker reads in its own window in 9-1.

{message}

**4. Wait.** End your turn. The report comes back as a cross-session message from the
worker, `report: <epic>` at its head, and it wakes you. The idle notice that
`notify_when_idle` sends is not a report: a worker goes idle whenever its turn ends — while
it waits on its person's answer, say — and one that asked its person something sends nothing
until it is answered. **Do not poll `ListAgents`** — the report comes to you. Inside tmux, an
idle notice with no report is the moment to read that worker's pane once (`moai-tmux`, "When a
worker stalls") and tell the person what it shows.

**A report reaches only the name it was sent to.** A supervisor that started again —
restarted, or resumed with `claude --resume` — stands under a new name, and a worker whose
report to the old one fails leaves it on the epic as a note with `report: <epic>` at its
head. So when you start or resume, before waiting, read what the tracker holds:

    moai show -s in_progress,review        the work sent and picked up, not done
    moai show -g 'Sent:'                   sent and not done — a backlog not unfolded yet, or an
                                           epic not picked up yet; neither is a candidate
    moai show -g 'report:' --all           the epics carrying a report nobody received

**Only a note that opens with the marker counts** — `-g` matches any text, and a body or a
note that discusses this protocol carries the same words. Read each match with
`moai show <id>` and look at its history: a report stands checked once a `Report-checked:`
note follows it on the same epic. Check each one that does not as in 5. A worker holding
sent work you have no report for is still left out in 2 — ask it, or its person, how it
stands.

If the supervisor is in the root, then in the gap after the worker picks the member up
and before it raises its worktree, the hook holds that member as "still picked up" when
the supervisor's turn ends. **That member is the worker's** — do not move it, do not
defer it, do not put a note on it; just finish the turn.

**5. Check the report and send the next.** Look at three things before you believe a report.

    git merge-base --is-ancestor <merge hash> <base branch> && echo yes   is the merge on the base branch
    moai show <epic>                       are the unfolded epic and its members done
    git worktree list                      is that worktree gone

`<base branch>` here is the one you sent with that work. For work inside a milestone it is
`milestone/<milestone id>` — the merge lands there, not on the root branch, which takes it
in only at the release, so checking the root branch reads a good merge as missing. In
`git worktree list` the epic's worktree is gone and `.worktrees/milestone-<milestone id>`
stays — that one is the milestone's, not leftover work.

**A member left because the work beside it holds the file** (the worker's 4-3) goes to an
idle worker after that other work's report is checked. Send the message of 3 with `<id>`
filled with the epic and its first line changed to the one below. Until then, count it in 1
as work holding that file.

    You are handed epic <id> — <title>. It is already unfolded; do not promote. Read <steps file> and follow its steps from step 2.

Like the ones from 7-1, that member is right even when it is not done — it keeps the epic
open, so the epic is not done either.

A member the report says was left in the first column by the worker's 7-1 is right even
when it is not done — that member keeps the epic open, so the epic is not done either. It
is not a backlog item and it does not show in 1's list, so pass it to the person along with the
reason it was left (a person's decision, a file held beside it).

`<epic>` is the epic id carried in the report. A backlog item is already done once it is
unfolded and it does not show its members, so `moai show <id>` cannot tell you whether
the work finished — when the report does not carry it, read it from that backlog's history
line about being unfolded.

If the three hold, mark the report checked on the epic and take it into the root with a
commit with a path — a supervisor that starts again reads that line, not this conversation (4).

    moai note <epic> 'Report-checked: <merge hash>'

Then send the next backlog to an idle worker. **Outside tmux, clearing a window is the
person's** — the supervisor never types into a window. The report ends with the worker
telling its person when its window can be cleared, so a message you send to that same window
right away can be erased by a clear that comes after it, and that backlog then waits for a
report that never comes. Send to that window once the person has cleared it or said they will
not — a clear does not show from here, so ask the person — or send to another idle worker.
**Inside tmux you may clear it yourself** — only after the report is checked and the note above
is written, only while `ListAgents` and its session record read `idle`, and only when its input
box is empty (`moai-tmux`, "Clear a worker's window"). If any of these fails, do as the
paragraph above says. Remove that pane's label once the report is checked (`moai-tmux`).

If they do not hold, ask that worker with a message what is left, and do not finish it in
its place.

## The shared root

The root checkout is **shared by every session.** While one session has a merge open
(`MERGE_HEAD`), another session committing a tracker note seals that merge with its own
subject — that has actually happened. So in the root, supervisor and worker alike:

- Give the tracker commit a path — `git commit -m "…" -- .moai/`. With a merge open git
  refuses a commit with a path, so wait until the session that opened it finishes and run it
  again. A `git commit` without a path seals that merge even when you ran `git status` first
- Finish your own merge in one call, `git merge --no-ff <branch> -m "…"`. Do not use
  `--no-commit`. If it stops on a conflict, do not resolve it in the root: `git merge --abort`
- Work inside a milestone does not merge in the root at all — it merges in the milestone's
  worktree (the worker's 8), and the root branch takes the milestone branch in at the
  release. Branches go in with `--no-ff` as they stand, never rebased or squashed

## When to stop

- If there is no backlog that does not collide, or no idle worker, say so to the person and
  stop — do not force a colliding backlog out
- If a worker is waiting on a person's decision, the supervisor does not answer in their
  place. The decision is the person's
"#
    )
}

/// 살아 있는 Claude Code 세션마다 `ListAgents` 의 이름과 tmux 칸을 잇는 짝 — **한 자리에만 선다**(moai-u99i.xo8).
/// `moai-tmux`([`tmux`])가 `format!` 으로 싣고, 되살리기 스킬([`recover`], moai-uqf7)이 죽은 세션까지 읽으려고 같은
/// 상수를 싣는다. 그래서 거르지 않고 첫 칸에 `alive`·`dead` 를 적는다 — 읽는 쪽이 고른다. 두 벌로 적으면 기록의 꼴이
/// 바뀌는 날 한쪽만 고쳐진다.
///
/// **읽는 것은 Claude Code 가 프로세스마다 남기는 `~/.claude/sessions/<pid>.json` 이다.** 죽은 프로세스의 파일도,
/// 다른 pid 이름공간(`pidDomain`)의 파일도 거기 남는다. 그래서 pid 가 살아 있는 것만으로는 모자라고 — pid 는 다시
/// 쓰인다 — 기록의 `procStart` 가 `/proc/<pid>/stat` 의 22번째 칸(프로세스 시작 시각)과 같아야 `alive` 다. `tmux` 는
/// `"<세션>:@<창>.%<칸>"` 이고, `-t` 가 받는 것은 끝의 `%<칸>` 이다. tmux 밖의 세션은 칸이 `-` 다.
///
/// **`%<칸>` 은 tmux 서버마다 따로 센다** — 기록에는 서버가 없어서, 다른 서버(`tmux -L …`)에서 도는 세션의 `%4` 를
/// 그대로 내면 부르는 쪽 서버의 엉뚱한 칸 `%4` 에 친다. 그래서 부르는 셸에 `$TMUX` 가 서 있으면, 산 세션의
/// `/proc/<pid>/environ` 의 `TMUX` 소켓이 부르는 쪽의 소켓과 같을 때만 칸을 내고 아니면 `-` 다. 죽은 줄과 `$TMUX` 없이
/// 부른 판은 기록의 칸을 그대로 낸다. 죽은 줄의 칸은 아무도 겨누지 않는다 — 되살리기(moai-uqf7)는 제가 연 칸에만 친다.
///
/// 칸은 탭으로 가른다 — tmux 세션 이름에 빈칸이 들 수 있다(`Shopping Crawler:@16.%39`). 줄의 꼴이 계약이다:
/// `state  name  pane  status  cwd  sessionId`. `the_session_map_reads_the_records` 가 실제 기록 꼴로 돌려 잰다.
pub const SESSIONS: &str = r#"python3 - <<'PY'
import glob, json, os
mine = os.environ.get("TMUX", "").split(",")[0]
for path in sorted(glob.glob(os.path.expanduser("~/.claude/sessions/*.json"))):
    try:
        with open(path) as f:
            r = json.load(f)
        pid = int(r["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        continue
    try:
        with open(f"/proc/{pid}/stat") as f:
            stat = f.read()
        alive = stat[stat.rindex(")") + 2:].split()[19] == str(r.get("procStart"))
    except (OSError, ValueError, IndexError):
        alive = False
    tmux = r.get("tmux") or ""
    pane = tmux.rpartition(".")[2] if "%" in tmux else ""
    if alive and pane and mine:
        try:
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(v.split(b"=", 1) for v in f.read().split(b"\0") if b"=" in v)
            theirs = env.get(b"TMUX", b"").split(b",")[0].decode(errors="replace")
        except OSError:
            theirs = ""
        if theirs != mine:
            pane = ""
    cols = [r.get("name"), pane, r.get("status"), r.get("cwd"), r.get("sessionId")]
    print("\t".join(["alive" if alive else "dead"] + [str(c or "-") for c in cols]))
PY"#;

/// 넷째 스킬 `moai-tmux` 의 SKILL.md — tmux 안에서 도는 감독의 손이다(2026-10-10 사용자 결정, moai-u99i).
///
/// **moai-obxm 의 두 결정을 tmux 쓰는 사람에게만 뒤집는다.** "창을 비우는 것은 사람의 몫 — 감독은 칸에 아무것도 치지
/// 않는다" 와 "감독도 에이전트를 띄우지 않는다" 였다. 이제 `$TMUX` 가 서 있으면 감독이 칸을 비우고, 닿지 않은
/// 메시지를 붙이고, 사람이 그러라고 한 뒤에 새 일꾼 칸을 연다. 참인 쪽은 그대로다 — **바이너리는 아무것도 띄우거나
/// 몰지 않고**(이 글은 스킬의 말일 뿐 새 명령이 없다), 헤드리스·`-p`·`--dangerously-*`·권한 모드 바꾸기는 없다.
///
/// **치기 전의 잣대는 하나다 — 입력 칸에 무엇이든 있으면 안 친다.** 옛 5-1 은 사람이 쓰던 초안을 건지느라 480줄이
/// 되었다. 초안인지 자리글(placeholder)인지 못 가르면 있는 것으로 읽고 사람에게 말한다.
///
/// **칸은 언제나 짝([`SESSIONS`])에서 찾은 `%id` 로 겨눈다.** 사람의 tmux 서버라, 맨 `kill-*` 하나가 그 서버의 모든
/// 세션을 죽인다(2026-09-18). 감독 제 칸·다른 저장소의 칸·시험 서버의 칸에는 치지 않는다.
///
/// **칸 이름표는 칸 사용자 옵션 `@moai` 다.** Claude Code 가 `pane_title` 을 제 화제로 덮어 써서 제목은 못 쓴다.
/// 보일지는 바퀴마다 사람에게 묻고, 그렇다고 하면 **도는 서버의** `pane-border-format` 앞에 붙인다 — 사람의 설정
/// 파일은 안 고친다.
///
/// Claude Code 에만 심는다([`crate::skill::Skill::claude_only`]) — 감독 스킬과 같은 까닭이다.
pub fn tmux() -> String {
    format!(
        r#"---
name: moai-tmux
description: Use in Claude Code together with moai-supervise when the supervisor runs inside tmux ($TMUX is set) — to find a worker's pane from its ListAgents name, label it, clear a reported worker's window, deliver a message SendMessage could not, read why a worker stalled, and open new worker panes once the person says yes. Triggers on "worker pane", "open a worker", "clear the worker", "일꾼 칸", "일꾼 창 열어", "칸 비워".
---

# moai-tmux — the supervisor's hands on the workers' panes

This is the supervisor's (`moai-supervise`) companion **when it runs inside tmux** — `$TMUX`
is set in its shell. Without `$TMUX` none of this applies: the supervisor works through
messages and the person alone. The tmux server here is **the person's own**: every session
they have lives on it, and every pane you touch is one they are looking at.

## What never happens

- **No headless run.** A worker you open is an ordinary interactive `claude` the person can
  see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`. The moai binary launches and drives nothing; this skill is you
  typing tmux commands where the person can watch
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux — one bare `kill-server` once took down every session the
  person had (hook rule 4, and the person's own guard hook refuses it too). A worker that
  is done stays open; closing a pane is the person's
- **Only a worker's pane.** Type only into a pane found through the session map below for a
  worker of this repository — never your own pane (`$TMUX_PANE`), never a session of another
  repository, never a test server's pane. Every call names its pane, `-t <pane>`, as `%N`
- **Never over the person's words.** If a worker's input box holds anything, do not type into
  it — tell the person
- **Never in their place.** A permission prompt or a question in a worker's pane is the
  person's to answer — you read it and tell them; you press no key
- **No polling.** Every look below is one look. A second one comes after your next step, as
  its own call — never a loop and never a `sleep`
- **The person's tmux config file stays theirs.** What you set is on the running server

## Which pane is which worker

`ListAgents` names a session; tmux needs a pane. Claude Code keeps one record per process
under `~/.claude/sessions/`, and the lines below print one row per record, tab-separated:

    state  name  pane  status  cwd  sessionId

```sh
{SESSIONS}
```

- **Read only `alive` rows.** A `dead` row is a record a process left behind — its pid is gone,
  or now belongs to another process (the start time differs)
- `name` is the `ListAgents` name, `pane` the `%N` that `-t` takes — `-` when that session
  is not in tmux or runs on another tmux server than yours (pane ids are counted per server,
  so another server's `%4` is a different pane here), and then this skill has nothing for
  that worker
- `cwd` is where the session stands: the root, or one of the worktrees `git worktree list`
  names — wherever they stand. A row standing elsewhere is not a worker of this repository, whatever its name
- `status` is `idle`, `busy` or another word. It has to agree with `ListAgents` where a step
  below asks for `idle`
- Your own row is the one whose pane is `$TMUX_PANE`

Run it when a step below needs a pane, once.

## Is the input box empty

    tmux display -p -t <pane> '#{{pane_in_mode}}'
    tmux capture-pane -p -e -t <pane>

The first line prints `1` while the person is scrolling the pane (copy mode) — keys you send
then go to tmux's copy mode, not to Claude Code, so a pane in a mode is theirs: do not type.
Claude Code's input box is the line that starts with `❯`, under the conversation, between two
`─` rules. **Empty** is `❯` followed by nothing, or by Claude Code's dim placeholder — `-e`
keeps the colours, and the placeholder is drawn dim (SGR `2`, or a grey foreground) where the
person's text is not. Anything
else — a word, a pasted block, a half-typed command — is the person's, and if you cannot tell
the placeholder from their draft, it is theirs. **Then do not type. Tell the person which pane
holds what, and go on as if this skill were not here.** A pane with no `❯` box at all (a shell
prompt, a dialog) is not a box to type into either.

## Label the pane

When you send work (the supervisor's 3), write the worker and the work onto its pane:

    tmux set-option -p -t <pane> @moai '<worker> <id>'

`<id>` is the backlog or epic you sent. Claude Code writes its own topic into the pane title,
so the label lives in the pane option `@moai`. Remove it when that work's report is checked
(the supervisor's 5):

    tmux set-option -p -u -t <pane> @moai

**Showing it is the person's choice, asked once per round.** Ask whether the labels should
show on the pane borders. On a yes, read the running server's two settings:

    tmux show -gv pane-border-format
    tmux show -gv pane-border-status

and put the label in front of the format you read:

    tmux set -g pane-border-format '#{{?@moai,#{{@moai}} ,}}<the value you read>'
    tmux set -g pane-border-status top

Set `pane-border-status` only when it read `off`. If the format you read already starts with
`#{{?@moai,`, it is shown already — leave it. If that value holds a single quote, do not build
the line; tell the person what to add. Never write the person's tmux config file.

## Clear a worker's window

Before you send a worker its next work, you may clear its window (`/clear`) yourself — all of
these first:

1. **Its report is checked** — the supervisor's 5 held all three checks and the
   `Report-checked:` note is written
2. **It reads `idle`** — in `ListAgents` and in its row of the session map
3. **Its input box is empty** (above)

Then:

    tmux send-keys -t <pane> /clear Enter

and, as a separate call, look once: the session map reads `idle` for it and `capture-pane`
shows the cleared screen — the conversation gone, an empty box. Then send with `SendMessage`
as the supervisor's 3 says. **If any condition fails, or the look does not show it cleared,
do not type again** — do what the supervisor's 5 says without tmux: ask the person, or send
to another idle worker. When it was the look that failed, tell the person that `/clear` may
stand typed in that pane's box: pressed later, it would erase the next message sent there.

## When a message does not arrive

When `SendMessage` to a worker fails — an error, no such session — and that worker has a pane
whose input box is empty (above), you may put the message into the box yourself. **A message
held for the person's approval is not one that failed:** that hold is the person's gate, like
a permission prompt, and the held message still arrives once they approve it — pasting it too
skips their gate and hands the worker the same work twice. Tell the person it waits for them
instead. Typed keys submit at every newline, so paste it as one block:

1. Write the message to a file in your scratchpad, with one line at the end naming you — a
   pasted message carries no sender, and the worker reports to the message's `from`. The
   first line stays the message's own, which names the work and the step to start from:
   `from: <your ListAgents name>`
2. Paste and submit it:

       tmux load-buffer -b moai-send <file>
       tmux paste-buffer -p -d -b moai-send -t <pane>
       tmux send-keys -t <pane> Enter

3. **Tell the person** you did, and into which pane

The worker still answers with `SendMessage`. A worker that answered your message by refusing
the work is not a delivery that failed — it comes out of the candidates (the supervisor's 2).

## When a worker stalls

When a `notify_when_idle` notice comes with no report, or the person asks about a worker that
has read `busy` far longer than its work should take, look at its pane once:

    tmux capture-pane -p -t <pane> -S -40

and read why it stopped — a permission prompt, a question (`AskUserQuestion`), an API or
rate-limit error, or a process that ended (a shell prompt where the box was). **Tell the
person** what the pane shows and which pane it is. Do not answer the prompt, do not press a
key, and do not look again in a loop. A process that ended is a dead session: bringing it
back is `moai-recover`, once the person asks for it.

## No idle worker left

When the supervisor's 2 finds no worker, ask the person **once** whether to open new worker
panes, and how many — one question covers several panes. Open a pane only on their yes, or
when they asked you for it. On a yes, for each pane:

    tmux split-window -P -F '#{{pane_id}}' -t "$TMUX_PANE" -c <root> 'claude --model <model>; exec bash'
    tmux select-layout -t "$TMUX_PANE" tiled

`<root>` is the `root dir` of the supervisor's 2 and `<model>` the model picked in 2-1. **No
other flag.** The first line prints the new pane's id; `exec bash` keeps a shell in the pane
when `claude` exits, so what it said stays readable. The pane belongs to the same person —
it is their interactive session like any other.

Once it has started (a separate call: `capture-pane` of the new pane shows the `❯` box), look
at `ListAgents` once more. The new row is a worker like any other (the supervisor's 2) — send
to it as in 3. If it does not show yet, look once more after your next step; if a prompt
stands in the new pane (trusting the folder, say), tell the person — it is theirs to answer.
"#
    )
}

/// 다섯째 스킬 `moai-recover` 의 SKILL.md — 죽은 세션을 되살려 이어 가게 한다(2026-10-10 사용자 결정, moai-uqf7).
///
/// **2026-10-10 에 실제로 걸은 길을 글로 옮긴 것이다.** 컨테이너가 다시 서며 감독(루트)과 일꾼 둘(워크트리)이 한꺼번에
/// 죽었고, 사람이 손으로 기록을 뒤져 `split-window … claude --resume` 으로 셋을 되살렸다. 일꾼 둘은 백그라운드
/// 서브에이전트가 끝나지 못한 채 커밋 안 된 고침을 워크트리에 남겼고, `/tmp` 가 비어 `target` 링크가 끊겼다. 그 걸음을
/// 다음 사람이 다시 짓지 않게 한다.
///
/// **새 moai 명령이 없다**(사람 결정) — Claude Code 의 세션 기록을 읽는 일은 Claude 만의 것이고 바이너리는 범용
/// 트래커다. 그래서 Claude Code 에만 심는다([`crate::skill::Skill::claude_only`]).
///
/// **세션과 칸을 잇는 짝은 [`SESSIONS`] 하나다.** `moai-tmux` 와 같은 상수를 `format!` 으로 싣는다 — 둘째 짝을
/// 적으면 기록의 꼴이 바뀌는 날 한쪽만 고쳐진다. 짝에 없는 것(역할·죽은 때·끝나지 못한 백그라운드 일)은 기록이 아니라
/// 대화 기록(`~/.claude/projects/…/<sessionId>.jsonl`)에서 읽는다. 워크트리에 든 일꾼의 대화 기록은 그 세션이 **처음
/// 선 자리**(루트)의 슬러그 밑에 남아 있었고(2026-10-10 에 본 `~/.claude/projects` — 일꾼 워크트리의 슬러그
/// 디렉터리는 비어 있었다), 같은 날 다른 판(2.1.296)에서는 워크트리의 슬러그 밑에 섰다. 판마다 갈리니 슬러그를 셈하지
/// 않고 id 로 찾는다.
///
/// **되살려 달라는 말이 곧 허락이다**(사람 결정) — tmux 안이면 묻지 않고 칸을 연다. 그 밖의 금은 `moai-tmux` 와
/// 같다: 헤드리스·`--dangerously-*`·권한 모드가 없고, 아무것도 죽이지 않고, 한 번에 한 번 보고, 빈 입력 칸에만
/// 붙인다. tmux 밖이면 아무것도 안 띄우고 칠 줄과 붙일 글만 낸다. 차례는 일꾼 먼저, 감독 마지막이다 — 감독의
/// `ListAgents` 가 일꾼을 보고 시작해야 한다.
pub fn recover() -> String {
    format!(
        r#"---
name: moai-recover
description: Use in Claude Code when the person asks to bring back the sessions of this repository that died — after a restart, an OOM kill or a crash — so the supervisor and its workers carry on where they stopped. Finds the dead sessions, draws the state each was in, points out background work that never returned and broken target links, and resumes each one in a new tmux pane, or prints the command to type. Triggers on "recover the sessions", "bring the sessions back", "resume the dead sessions", "되살려", "세션 복구", "이어 가게 해".
---

# moai-recover — bring back the sessions that died

Use this when the person asks for it: the sessions of this repository — a supervisor, its
workers — died together, after a restart, an OOM kill or a crash, and the person wants them
to carry on. **Their asking is the yes:** inside tmux you open the panes without asking again.
None of this is a moai command; it is you reading Claude Code's own records and typing where
the person can watch.

## What never happens

- **No headless run.** A session you bring back is an ordinary interactive `claude --resume`
  the person can see and type into. Never `claude -p`, never a `--dangerously-*` flag, never
  `--permission-mode`
- **Nothing is killed.** Never `kill-server`, `kill-session` or `kill-pane`, never `pkill`
  or `killall` aimed at tmux (hook rule 4). A session that is alive is left alone
- **Only the panes you opened.** Type only into a pane this skill opened, by the `%N` that
  `split-window` printed — every call names it, `-t <pane>`
- **Never over the person's words.** Paste only into an empty input box (`moai-tmux`, "Is the
  input box empty"); otherwise tell the person
- **No polling.** Every look is one look; the next comes after your next step, as its own
  call — never a loop, never a `sleep`
- **Their work stays as it is.** No commit, no checkout, no stash, no build, no `moai` write —
  what a dead session left is for that session to pick up

## 1. Find the dead

Claude Code keeps one record per process under `~/.claude/sessions/`, and a process that dies
leaves its record behind. The lines below print one row per record, tab-separated:

    state  name  pane  status  cwd  sessionId

```sh
{SESSIONS}
```

- A **candidate** is a `dead` row with a `sessionId` whose `cwd` is the root or one of the
  worktrees `git worktree list` names
- **Drop it when it is back already** — a live row carries the same `sessionId` (`--resume`
  keeps the id), **your own row included**: a session the person resumed first and then asked
  for this is back already, and resuming it again puts one conversation in two windows. In a
  worktree, also drop it when a live row other than your own stands in that same `cwd`. The
  root is not such a place — several live sessions stand there at once, and one of them being
  alive says nothing about the dead one
- **Drop old crashes.** Records of earlier deaths stay too. Keep the candidates whose
  transcript (below) last moved around the same time; name an older one to the person apart,
  and bring it back only if they say so
- When several candidates stand in the same `cwd`, ask the person **once** — one question
  for all of them — which to bring back

**A session's transcript** is `~/.claude/projects/<slug>/<sessionId>.jsonl`, one JSON object
per line. The slug is a directory with every character that is not a letter or a digit
turned into `-` (`/home/me/repo/.worktrees/moai-ab12` is `-home-me-repo--worktrees-moai-ab12`).
Which directory is not fixed: a worker that entered its worktree from the root has kept its
transcript under the root's slug on one Claude Code version and under the worktree's on
another, so do not work the slug out — find it by id:

    ls ~/.claude/projects/*/<sessionId>.jsonl

**When the records are gone too**, look under the root's slug and under each worktree's slug
for every `*.jsonl` that last moved around the time they died — the root's slug may hold the
supervisor's and a worker's both, so not only the newest one. A line's `cwd` and `sessionId`
say where that session last stood and which id to resume; its `timestamp` says when it last
moved.

## 2. Draw the state each was in

Read each candidate's transcript from the end — the last user line and the last assistant
line — and look at where it stood:

- **Role.** The supervisor's transcript runs `moai-supervise`; a worker's holds the
  supervisor's message naming `references/worker.md`. A session that is neither is brought
  back the same way, as a worker
- **Work.** The issue or epic id it held, and what it was doing or waiting for — a report,
  a review, a person's decision, a build
- **Died at** — the `timestamp` of its last line
- `git -C <cwd> status --short` — what it left uncommitted
- `moai show <id>` — a `Next:` note on that id says where the session meant to go on

Show the person **one table**, a row per session: role, name, `cwd`, work id, what it was
waiting for, died at, uncommitted files. Then go on — they asked for recovery already.

## 3. Point out what died with it

- **Background work that never returned.** A background launch is an `Agent` or `Bash`
  `tool_use` whose result reads `Async agent launched` (with its `agentId`), `Command running
  in background with ID: <id>`, or — a command that ran past its timeout —
  `moved to the background (ID: <id>)`. Whether the input carries
  `"run_in_background": true` differs by version and the timeout case carries nothing, so read
  the result; when it ends, a later user line carries a
  `<task-notification>` whose `<task-id>` is that id. A launch with no such line after it died
  with the session — the resumed session will never hear from it, and what it
  changed is uncommitted in that session's `cwd`
- **A broken target link.** In the root and each worktree, `target` may be a link to
  `/tmp/cargo-target/<name>`, and a restart can empty `/tmp`:

      readlink <cwd>/target
      test -e <cwd>/target || echo dangling

  Make the directory again, `mkdir -p /tmp/cargo-target/<name>`, so builds land there. The
  build output is gone: `./target/release/moai` needs `cargo build --release` before anything
  calls it. Do not build it yourself — the resumed session does

## 4. The text each one gets

One block per session, written to a file in your scratchpad:

    Recovery: this session died at <time> (<what happened>) and was resumed.
    - Background work that never returned: <each launch and what it was for>. It is dead,
      do not wait for it; what it changed is uncommitted in <cwd>.
    - git status --short in <cwd>: <files, or clean>.
    - target -> /tmp/cargo-target/<name> was gone and is made again; build before you
      call ./target/release/moai.
    Go on from where you stopped: <work id>, <what it was waiting for>.

Leave out a line that does not hold. The supervisor's block adds one line: **the workers are
back in new sessions and their names may have changed — run `ListAgents` again** before you
send or wait for a report.

**Workers first, the supervisor last**, in both ways below — the supervisor's `ListAgents`
has to see the workers when it starts.

## 5. Inside tmux — open a pane each

`$TMUX` is set in your shell. For each session, in that order:

    tmux split-window -P -F '#{{pane_id}}' -t "$TMUX_PANE" -c <cwd> 'claude --resume <sessionId>; exec bash'
    tmux select-layout -t "$TMUX_PANE" tiled

**No other flag.** The first line prints the new pane's `%N`; `exec bash` keeps a shell there
when `claude` exits. Then, as a separate call, look once: when its `❯` box shows and is empty
(`moai-tmux`, "Is the input box empty"), paste the block:

    tmux load-buffer -b moai-recover <file>
    tmux paste-buffer -p -d -b moai-recover -t <pane>
    tmux send-keys -t <pane> Enter

If the box has not shown yet, look again after your next step. A prompt in the pane (trusting
the folder, say) is the person's to answer — tell them which pane. When all are open, tell the
person which pane is which session.

## 6. Outside tmux — say what to type

`$TMUX` is not set: open nothing. Print, per session in the same order, the line the person
types in a terminal of their own, and under it the block to paste once its box shows:

    cd <cwd> && claude --resume <sessionId>
"#
    )
}

/// 되짚기(7-1)가 에픽 목적에 걸리는 backlog 를 선 에픽의 멤버로 되찾는 줄(moai-l288).
/// **이것도 promote 다**(moai-f3ml) — `add` 와 손 닫기로 적었던 판은 "backlog 를 일감으로 바꾸는
/// 길은 promote 하나" 를 어겼고, 그것을 지키던 시험에서 이 줄을 빼야 했다.
///
/// **`-C <루트>` 를 줄에 박는다.** 7-1 은 워크트리에서 치는데, 글로만 "4-1 대로" 라고 적고 줄을
/// 맨 `moai` 로 두면 그대로 옮겨 친 줄이 워크트리의 `.moai` 에 멤버를 세운다 — 병합에서 스냅샷이
/// 부딪히거나, 4-1 이 시키는 `git checkout -- .moai` 로 그 멤버가 사라진 채 에픽이 닫힌다.
/// `<루트>` 는 감독이 채우는 자리라 받은 줄에 실제 자리가 박혀 온다. 다른 자리 이름은 그 목록과
/// 겹치지 않는다 — 겹치면 맡긴 backlog 의 값이 이 줄에 미리 박힌다(`<등급>` 와 같은 덫).
const RECALL: &str = "moai -C <root> backlog promote <backlog id> -e <epic> --from -";

/// 7 에서 리뷰 이슈를 세우며 멤버를 `review` 칸에 세우는 줄(사용자 결정 2026-10-03, moai-vxld).
/// 리뷰가 도는 동안 아무도 손대지 않는 멤버가 보드에서 `in_progress` 로 "하는 중" 을 말했고,
/// 브리프가 칸을 말하지 않아 창마다 옮기기도 두기도 했다.
///
/// **`--from in_progress` 가 첫 칸의 멤버를 거른다.** 4-3·7-1 로 남긴 멤버는 이 리뷰가 안 본
/// 일이라 `review` 에 서면 안 되는데, 글로 "빼라" 고만 하면 옮겨 친 줄이 그것까지 세운다 —
/// 본 칸이 다르면 옮기지 않는 `--from` 이 그 판단을 대신 한다.
const TO_REVIEW: &str = "moai -C <root> mv <member> review --from in_progress";

/// `review` 칸이 없는 저장소에서 [`TO_REVIEW`] 가 받는 거절의 머리. 칸 이름은 config 가
/// 정하는데, 브리프는 2 에서 이미 `in_progress`·`todo` 를 이름으로 박았다 — 그래서 칸이 있는지
/// 미리 알아내는 자리(감독이 채우는 `<review column>`, 보드를 먼저 읽는 걸음)를 두지 않고 이
/// 거절을 답으로 읽게 한다(사용자 결정, moai-vxld). 기본 칸과 `init` 이 적는 config 에는
/// `review` 가 든다 — 없는 것은 `statuses` 를 손으로 바꾼 저장소다.
/// `the_brief_stands_members_in_review` 가 이 글을 `refuse.no_column` 의 영어 글과 견준다.
const NO_REVIEW_COLUMN: &str = "`review` is not a column";

/// 감독이 일꾼에게 `SendMessage` 로 보내는 메시지 — 맡길 일(첫 줄)과 감독이 채운 자리들(moai-obxm). 첫 줄이
/// [`worker`] 의 파일(`<steps file>`)을 읽고 몇째 걸음부터 할지를 댄다(moai-fim6). **일꾼이 아는 것은 이 메시지와 그 파일뿐이다** — 감독 스킬의 다른 절을 가리키면 일꾼에게
/// 없는 글을 가리키는 것이라(첫 판의 "아래 공유 main" 이 그랬다), 감독만 아는 값은 모두 자리로 싣는다. 자리마다 읽는
/// 법은 일꾼 글의 "The assignment" 가 댄다.
///
/// **보고할 곳은 자리가 아니다** — 메시지의 `from` 이 감독이다. 옛 편지의 `Supervisor <my name>` 은 출석(`hello`)이
/// 준 이름을 실었는데 출석을 걷었다. `After the report:` 도 걷었다 — 일꾼은 보고하고 턴을 끝내고, 다음 메시지가 깨운다.
///
/// **줄 머리의 낱말이 계약이다** — 일꾼 글이 `Model:`·`Base branch:` 따위를 그 이름으로 가리킨다.
/// `the_message_carries_every_slot_the_steps_read` 가 둘을 견준다.
fn message() -> String {
    format!(
        r#"    You are handed backlog <id> — <title>. Read <steps file> and follow its steps from step 1.
    Read first: moai show <id>
    {MODEL_SLOT}
    {BESIDE}
    Root branch: <root branch>
    Base branch: <base branch>
    Milestone: <milestone>
    Root: <root>
    Subdir: <subdir>
    Person: <person>"#
    )
}

/// 일꾼의 걸음 — 감독 스킬의 `references/worker.md`(moai-obxm). 감독의 [`message`] 가 첫 줄에서 이 파일을 읽으라고 이른다
/// (moai-fim6). 일꾼이 하는 일을 바꾸지 않는 내력·측정 글은 여기 안 싣는다 — 그것은 이 주석과 커밋 메시지에 선다.
/// 일꾼 스킬(`moai-work`)을 걷었으니 일꾼이 지킬 것은 모두 여기 적는다.
///
/// **Claude Code 의 것이다**(2026-10-06 사용자 결정). 감독 스킬이 Claude Code 에만 심기니 이 글을 받는 창도 Claude
/// Code 다 — 낱말표의 *기울인* 걸음 이름을 걷고 도구를 바로 적는다(`EnterWorktree(path)`·`ExitWorktree(keep)`·
/// `AskUserQuestion`·`/code-review`·`/model`·`TaskStop`·`/clear`). Codex·Antigravity 를 위한 갈래 글도 걷었다.
///
/// **고리가 없다.** 출석·기다림·편지(`moai hello`·`moai inbox`·`moai send`)를 걷었다 — 메시지가 창을 깨우고, 일꾼은
/// 보고하고 턴을 끝낸다. 다음 메시지가 다음 일이다.
///
/// **리뷰 줄은 규칙 3 의 조각으로 적는다.** 손으로 줄인 `-t review --parent <에픽>` 은 관점(`-b`)이 빠져, 에픽 리뷰가
/// 무엇을 왜 보는지 없이 섰다.
///
/// **닫기는 워크트리를 지운 뒤다.** 워크트리가 남아 있으면 훅이 그 에픽을 옆 워크트리의 일로 읽어(`worktree::away`)
/// `-m` 없는 리뷰 닫기를 못 막고, 루트의 편집은 규칙 2 로 막는다 — 그래서 충돌과 시험은 워크트리에서 풀고, 루트
/// 병합이 막히면 되돌리고 돌아간다.
///
/// **보고가 마지막이다**(moai-snyk). 감독은 보고를 받고서야 다음을 보낸다 — 보고 뒤에 `Next:` 노트를 적던 판은
/// 감독이 읽을 때 노트가 없었다. 그래서 11 이 노트, 12 가 보고다.
///
/// **넘칠 때의 길도 여기 적는다**(리뷰 moai-4u6b.5hl). 닫는 걸음이 싣는 것은 `(64KB 를 넘으면 요약)` 한 마디뿐이라,
/// 이 글만 받은 일꾼은 줄이라는 말은 읽고 **요약이라고 밝히라는 말은** 못 읽었다 — 밝히지 않은 요약은 다음 사람이
/// 리뷰어의 말로 읽는다.
///
/// **마일스톤 일은 마일스톤 가지로 병합한다**(2026-10-08 사용자 결정, moai-nvju). 그 가지는 루트 밖의 오래 사는
/// 워크트리(`.worktrees/milestone-<id>`)에 체크아웃돼 있어, 병합과 그 HEAD 대조와 `branch -d` 를 **루트에서
/// `git -C <그 워크트리>`** 로 친다 — 격리 가드는 `EnterWorktree` 로 든 세션에만 서므로 `ExitWorktree(keep)` 뒤의
/// 루트에서는 한 줄 맨 명령으로 지나간다. 그 워크트리에 `EnterWorktree` 로 드는 판은 충돌을 풀 때만이다.
/// `branch -d` 를 루트에서 치면 루트의 가지가 아직 그 병합을 안 들어 "not fully merged" 로 거절한다(재 봤다).
pub fn worker() -> String {
    let rule = indent(&model_rule(), "  ");
    let review = make_review("--parent <epic>");
    let close = indent(&close_steps("<review id>", "moai"), "      ");
    let over = indent(REVIEW_OVER_LIMIT, "    ");
    let levels = difficulty_levels();
    // 잣대는 걸음 5 안의 글머리 목록이다 — 맨 줄로 두면 GitHub 에서 세 낱말이 한 문단으로 뭉친다(리뷰 moai-bkn4.c3d).
    let rubric = indent(&difficulty_rubric(), "   - ");
    let top = top_model();
    let epic_rule = indent(&epic_review_rule(), "   ");
    let angle = indent(REVIEW_ANGLE, "   ");
    // 7-4 는 목록 밖의 문단이라 명령 줄이 네 칸이다(`the_worker_steps_follow_the_markdown_list_rules`).
    let wiki_commit = indent(WIKI_COMMIT, "    ");
    format!(
        r#"# Worker steps

A supervisor — a Claude Code session running `moai-supervise` on this repository — sent you
a message that names this file. The message's lines are your assignment; this file is how to
do it. You know nothing else of the supervisor's conversation and need nothing else. **The
person comes first** — this window is theirs; when they speak, answer them.

## The assignment

The message's first line names the work and the step of this file to start from —
`from step 1` for a new backlog, `from "Carrying on stalled work"` for work a session left
behind, `from step 2` for an epic already unfolded whose first-column members are left.
The message's `from` is the supervisor — `<supervisor>` below; "tell the supervisor" is `SendMessage(to: <supervisor>, …)`.
A message the supervisor pasted into this window (`moai-tmux`) carries no `from`; its last line,
`from: <name>`, names the supervisor instead. Every
other line fills a slot the steps use; a line the supervisor adds beyond those — who already
said yes to taking over a row that is not yours, say — belongs to the assignment as well.

- `Model:` — `<model>` and `<difficulty>`.
{rule}
- `Work running alongside:` — the worktrees and work beside you, and the files they hold (4-3)
- `Root branch:` — `<root branch>`, the branch the root checkout stands on. The supervisor read
  it in the root; do not read it again — read inside a worktree, it gives that worktree's own branch
- `Base branch:` — `<base branch>`, the branch your worktree splits from and merges back into.
  Outside every milestone it is `<root branch>`; inside a milestone it is that milestone's own
  branch, `milestone/<milestone id>` ("The milestone branch")
- `Milestone:` — `<milestone>`, the release the epic hangs (1)
- `Root:` — `<root>`, the root checkout's place (4-1)
- `Subdir:` — `<subdir>`, only for a subdirectory project in a monorepo (3). Without it the
  root is the top of the repository
- `Person:` — `here`, or `away` (below)

Ask two things where the window stands now, before you move anywhere.
**If this window runs `moai-supervise` itself**, it is a supervisor, not a worker: do
nothing of it, reply to the message's `from` that you are a supervisor, and end the turn.
**If `Root:` is not this window's repository** — the window stands neither in it nor in one
of its worktrees — the supervisor took you for a worker by a name that only looks like its
repository's. Do nothing of it: reply to its `from` that you stand in another repository, and
end the turn.
The steps begin in the root the message names (`Root:`) — if this window stands anywhere
else in that repository, go there first: `cd` from a subdirectory, `ExitWorktree(keep)` from
a worktree.
A message that hands over no work is not work: if it asks something, answer it with
`SendMessage` to its `from`, and end the turn.

## When the person is away

When the message says `Person: away`, decide by recommendation. A design question the notes
do not settle is not asked (4) — settle it the way you would have recommended, and write it
on the issue where the next person reads it

    moai note <id> 'Decided alone: <what you chose> — <why>, and what the other way was'

Nothing else waits on the person either. A model the window is not on is not asked for —
work on the window's model and say so in the reason of 9-1. A row that is someone else's or
nobody's (rule 5) is taken over only on a yes the message carries; without one, leave that row
and name it in the report.

**Stop at what cannot be undone** — deleting what is not yours, rewriting history someone
else has, a release, anything outside this repository — and report that instead of doing
it. When the person is back in the window, what they say overrides what you decided alone.

## Before the steps

{BRANCH_CHECK}

{GIT_SHAPES}

## The milestone branch

When `Base branch:` reads `milestone/<milestone id>`, the work is inside a milestone —
`<milestone id>` below is the part after `milestone/`. Every epic of that milestone merges
into that branch, not into `<root branch>`; `<root branch>` takes the milestone branch in
once, at the release, and the release is the person's, not yours. The branch is checked out
in a long-lived worktree of its own, `.worktrees/milestone-<milestone id>`, because the root
stays on `<root branch>` and git checks a branch out in one place only. You do not work in
it — you merge there (8) and nothing else — and you never remove it: it goes after the
release. Outside a milestone this section does not exist. `Milestone:` can say `none` while
`Base branch:` names a milestone — that milestone stands but has not started running, and
`promote` still carries it over in 1; the branch is where this work goes all the same, and
in 1 you hang `<milestone id>`, not `none`.

**Raise it if it is missing**, from the root, right before the `worktree add` of 3 — look
at `git worktree list` first. The first line raises the branch from the local
`<root branch>`; the second is for a branch that stands while its worktree does not. If
`worktree add` says the branch or the place already exists, a worker beside you raised it
first — look at `git worktree list` again and carry on with what stands

    git worktree add -b milestone/<milestone id> .worktrees/milestone-<milestone id> <root branch>
    git worktree add .worktrees/milestone-<milestone id> milestone/<milestone id>
If the repository's own worktree convention adds something to a new worktree (a link for
the build output, say), add it to this one too.

**Git aimed at the milestone's worktree runs from the root**, after `ExitWorktree(keep)` —
there `git -C .worktrees/milestone-<milestone id> …` is one plain command and goes
through. From inside your epic's worktree it is refused like any git aimed outside it (the
git shapes above), so do not try it there. That worktree is shared by every worker of the
milestone, like the root: if git refuses a merge there because one is already open
(`MERGE_HEAD`), it is another worker's — never `merge --abort` it; wait for it to finish
and run yours again. Before you merge into it, check its HEAD the way
the root's is checked; if it does not stand on the milestone branch, do not merge: tell the
supervisor

    git -C .worktrees/milestone-<milestone id> symbolic-ref -q HEAD      it has to be refs/heads/milestone/<milestone id>
**The milestone branch takes `<root branch>` in only at the release.** If this epic needs a
fix that landed on `<root branch>` after the milestone branch split off, take it in at that
moment, from the root, and then pull the milestone branch in 6 as usual. If it stops on a
conflict, resolve it inside the milestone's worktree (`EnterWorktree(path)` from the root),
never in the root

    git -C .worktrees/milestone-<milestone id> merge --no-ff <root branch> -m "merge: take <root branch> in for <epic>"

## The steps

1. Unfold it in the root — the one way to turn a backlog item into work is
   `moai backlog promote <id> --from -`. Unfold into an epic plus issues even for a single
   issue. Look at `--dry-run` first — that is for this window to see, not to show a person
   and ask. Showing a split plan to a person once is a step of work a person asked for
   directly; what a supervisor hands you is work a person already passed on. Design
   decisions are asked in 4.
   If that backlog is already done (someone unfolded it), do not unfold: tell the supervisor —
   unfolding again puts up two epics. **Write a short new title** — a line in the plan
   becomes the issue title verbatim, so copying over a backlog item title that grew long while it
   was parked spreads that length into the issues. The original text stays on that backlog and
   the history leads back to it.
   Then hang the milestone on the epic you unfolded — `promote` brings over the body and the
   release the backlog stood in, and a milestone is inherited, so the epic alone carries it to
   every member and to the members added later in 4-3 and 7-1. Hanging the same one again
   changes nothing. **Hang only the `<milestone>` in the message, and nothing else**: work is
   never pulled into a running release, so a release you noticed running is not yours to
   attach — not to this epic, not to a member you create later. Inside this epic the release
   is inherited, which is the one door that stays open. **If `<milestone>` is `none` but
   `Base branch:` names `milestone/<milestone id>`**, that milestone stands and has not started
   running — hang that `<milestone id>` in its place: `none` would clear the release `promote`
   carried while the merge still lands on its branch. Otherwise, if `<milestone>` is `none`, this work
   stands outside every release — that is nothing running, or a backlog item that stood under none,
   or one whose release is already dead, and you cannot tell which from the word alone. What
   came over is still the release that backlog stood in, so read the line `promote` printed and clear a
   release that has already shipped or been deferred with `moai edit <epic> --milestone none`;
   a dead one is named on stderr. Under a deferred one the whole plan is out of the plan:
   not in `ready`, not in `held`, no warning

       {MILESTONE_ATTACH}
2. Pick the members up with `moai mv <member> in_progress --from todo` and commit in the root.
   **Pass the column you saw** — this is a place where several sessions share one `.moai`,
   and overwriting a row picked up beside you means two of you do the same work. A non-zero
   code means it is taken, so leave that member and tell the supervisor. The root is shared
   by every session — a commit made while someone has a merge open (MERGE_HEAD) seals that
   merge with its own subject. So give the tracker commit a path. With a merge open git
   refuses it, so wait for that merge to finish and run it again

       git commit -m "chore(tracker): pick <epic> up in a worktree" -- .moai/
   **A member that is someone else's, or nobody's, is asked about** — the hook refuses that
   pick-up (rule 5). Ask the person watching this window — a yes the message already carries
   counts; on a yes, run the line the refusal hands you (`--take -m '<who said yes>'`), on a
   no leave that member and tell the supervisor
3. Right after the commit in 2, branch from the local <base branch> with
   `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>` and go in with
   `EnterWorktree(path)` from the root. The name is the unfolded epic's id, not the backlog's.
   Inside a milestone the milestone branch has to stand first — raise it if it is missing
   ("The milestone branch").
   Until the worktree stands, the other sessions in the root read this member as their own focus.
   **If the root is not the top of the repository** (a subdirectory project in a monorepo) the
   worktree stands for the whole repository, so once inside, move to the same subdirectory in
   it and work there — standing at the worktree top, `moai` walks up and finds the root's
   `.moai` to write, and the hook does not count edits under `.worktrees/`. `<subdir>` is that
   relative path, filled in by the supervisor; if the message carries no `Subdir:`, the root
   **is** the top and this step does not exist

       {SUBDIR}
4. Do not guess a design decision that is not in the notes — ask with `AskUserQuestion`;
   a person is watching the worker's window. With `Person: away` in the message, decide by
   recommendation instead ("When the person is away")

4-1. **The tracker you edit is always the root's.** `<root>` is the root checkout's place,
   filled in by the supervisor — do not guess it from inside the worktree. **The tool moves
   that by itself** — even a bare `moai` typed inside the worktree reads and writes the
   root's tracker, and when it writes, one line says where. `moai -C <root> <command>` lands
   in the same place, so writing it that way is fine too. Short of a branch with no tracker
   in the root, the only thing that stops the move is `MOAI_HERE`, so **do not turn it on** —
   turn it on and that worktree's `.moai` changes, and the snapshots conflict on the merge
   (and merging them overwrites someone else's rows).
   Put `-e <epic>` on a backlog item you park mid-epic — it does not keep the epic open, and 7-1
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
   agent on that server, keep its cwd outside the root (the scratchpad) — raised in the root,
   it stands in `ListAgents` as a session of this repository and a supervisor takes it for a worker.
   **Give a review subagent these words too**

4-3. **If you would have to touch a file that work running alongside holds, do not fix it** —
   the files named by `Work running alongside` in the message, or files a sibling branch in
   `git worktree list` already changed
   (`git diff --name-only <base branch>...<sibling branch>`). When two of them change the
   same place, one waits for the other at the merge. If this epic cannot deliver what it
   promised without that, it is not a backlog item but a member — create it with
   `moai -C <root> add '<what>' -e <epic>`, leave it in the first column, and name it in 12
   as **a member left because the work beside it holds the file**, together with that other
   work. The supervisor sends it once that work is done. Do not defer it

5. **Do not review member by member.** When one member is finished, run the tests, commit and
   move to the next — the review looks at the whole epic once, in 7, after every member is
   finished. One review is expensive; do not call it as many times as there are members. The
   cost of member 2 piling onto a bug in member 1 is paid in that one review.
   {levels} is the rubric the model in the message was picked on, and the same rubric measures
   the members when you pick the grade in 7.
{rubric}
6. When the members' work is all done, pull <base branch> into the worktree, resolve the
   conflicts and run the tests. Fix things here — while the worktree stands, rule 2 blocks
   edits in the root. Inside a milestone that is the milestone branch, not `<root branch>`
   — the epics merged beside you are there
7. Before merging, review the whole epic with `/code-review <grade> --fix` — the members were
   not reviewed separately, so this once is the only review. **It runs inside this session** —
   never start another agent program for it.
{epic_rule}
   If the window is not on that model, ask the person watching it to change it before you
   call — `/model {top}` (a review agent inherits the window's model).
   Write the grade you picked and why in one line in the angle (`-b`). The diff runs from
   where the branch left <base branch> (`git merge-base <base branch> HEAD`). You pulled it
   in 6, so the conflict resolution is inside it too. Create the review issue (rule 3)

       {review}
   This line is called from the worktree too, so run it as `moai -C <root>`, per 4-1.
   **In the same breath, stand the finished members in `review`** — while the review runs
   nobody is working on them, and `in_progress` on the board says somebody is. One id per
   call, as in 2

       {TO_REVIEW}
   `--from in_progress` passes over the members left in the first column by 4-3 (and by 7-1
   when you came back from 8) — this review does not see them, so they do not stand in it.
   moai answers that such a member already stands in the first column and moves nothing, with
   a non-zero code: that is the pass-over, not a lost pick-up as in 2. moai answers in the
   screen's language (`MOAI_LANG`, or `lang` in the user config), so read what it says, not
   the words: if it refuses `review` itself and lists the columns there are without it — in
   English "{NO_REVIEW_COLUMN}" — this repository's columns have no `review` and the step
   does not exist here: leave the members where they stand and go on. If it answers that the
   member already stands `review`, you came back from 8 — leave it. What you take in goes in
   a separate fix: commit; what you hand on goes in a note with the issue id.
   A refusal from the hook, such as a missing angle, is not worked around: fix it the way the
   refusal's own command says

{angle}

   **While the review is running, do not touch this worktree's branch or its working tree.**
   A review that fixes leaves its fixes in the working tree uncommitted, so `reset --hard`,
   `rebase` and `commit --amend` throw them away — even when it is before the merge and
   looks fixable. Nothing blocks it; this line is what holds. Fixing a commit subject waits
   until the review has returned.
   **Nor is the branch rebased or squashed when it merges** — it goes in with
   `git merge --no-ff` as it stands (8). `moai show` finds an issue's commits by the id in
   their subject, and a squash folds them away; the review's fixes stay as their own `fix:`
   commits; and rebasing a branch that already holds merges — the milestone branch — changes
   the merge hashes the `Report-checked:` notes point at. Rewording a subject on this branch
   before the review starts is fine — nobody else has it yet.
   **When it returns, stop what it left running with `TaskStop` before you touch the tree**
   and read the working tree's status. A sweep subagent still alive writes its own version
   into this same worktree and covers a commit you already made without a word, and a
   `cargo test` after that measures that agent's files rather than yours.

7-1. Before merging, go back over the backlog parked mid-epic
   (`moai -C <root> show --type backlog -e <epic>` and what this window remembers) and what the
   review handed on — **can the epic deliver what it promised without them.** If not, it is
   not a backlog item but an unfinished member. What you sorted as "not for now" while parking has
   these mixed in — the one waiting on a person's decision, the one pushed out because a
   worker beside you held that file. This step sits after 7 so that it sees what 7's review
   handed on too. Unfold such a backlog item as a member of the epic already standing — write only
   `- issue` lines in the plan; the backlog closes by itself and its source stays. You type this
   from the worktree, so pin the root into the line (4-1). **If that backlog is already done, do
   not unfold it** — someone unfolded it, or you came back from 8 and are going round again.
   promote unfolds a closed backlog too, and the same member stands twice

    {RECALL}
   Do not do a reclaimed member here: merge with it left in the first column — work that has
   not been through 7's review does not get mixed into the merge, and a member still standing
   keeps the epic open. Do not `defer` that member. Deferring it closes the epic without its
   promise delivered. 7's review did not see that member, so write it in the `Next:` note in
   11 — the window that closes the epic with that member calls the epic-end review again

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
   inside; the rest is outside. Do not make a new tag or field for it

7-3. **If the repository keeps a CHANGELOG, check that this epic's line stands in the section
   for the release being prepared**, and write it if it does not. Write it **here, in the
   worktree, before the merge**: after 8 the worktree is gone and the only checkout left is
   the root, which every session shares and where the only commits that belong are the
   tracker's and the merge itself. Committed here it rides the merge commit instead

    git commit -m "docs(changelog): <what this epic changed> (<epic>)" -- CHANGELOG.md
   This window is the only one that knows what the epic did, and it is the only one that
   knows what was taken out as well as what went in — a section filled in later from commit
   subjects shows what was added and misses what was removed, because a removal stands under
   a revert subject of its own. The release notes are that section as it stands, so a
   missing line reads to whoever receives them as a change that never shipped.
   **Nothing checks this** — a check here would be a gate, and an empty section must not
   stop a release

7-4. **If the repository keeps a wiki** (`moai wiki ls` lists pages), ask once whether this
   epic changed what a person does — {CHANGED_USE}.
   If it did, follow the `moai-wiki` skill and commit what it wrote for the same reason as
   7-3 — here, in the worktree, before the merge. `<wiki dir>` is `dir` in `moai wiki ls --json`

{wiki_commit}
   If it did not, write nothing. **Nothing checks this**

8. Come back to the root with `ExitWorktree(keep)` — remove the worktree from inside it and
   this window stands in a directory that is gone.
   Before merging outside a milestone, check that the root stands on <root branch> — if it
   does not, do not merge: tell the supervisor

       git symbolic-ref -q HEAD                  it has to be refs/heads/<root branch>
   **Inside a milestone you merge in the milestone's worktree, not in the root** — check that
   worktree's HEAD instead, with the line in "The milestone branch", and run the merge and its
   abort below with `-C .worktrees/milestone-<milestone id>` after `git`, from the root.
   Merge **in one call**. Overlap with the workers beside you was split when the
   supervisor sent the work, and where it still collides, undo as below and resolve in the
   worktree — do not go looking for the other session to tell it. Do not use `--no-commit`.
   Without `--no-ff` it ends as a fast-forward and no merge commit stands

       git merge --no-ff worktree-<epic> -m "merge: …"
       git -C .worktrees/milestone-<milestone id> merge --no-ff worktree-<epic> -m "merge: …"     inside a milestone
   If the root's `.moai` holds uncommitted rows from another session the merge is refused —
   take them in first with a commit with a path, as in 2. If it stops on a conflict, do not
   resolve it where it stopped — undo with `git merge --abort`, go back into the worktree with
   `EnterWorktree(path)` and run again from 6
9. Once the merge has really landed, remove the worktree and the branch from the root with
   `git worktree remove .worktrees/<epic>` and `git branch -d worktree-<epic>`.
   **Inside a milestone delete the branch in the milestone's worktree** —
   `git -C .worktrees/milestone-<milestone id> branch -d worktree-<epic>`. `-d` asks whether
   the branch is merged into the HEAD it runs in, and `<root branch>` in the root does not
   hold this merge until the release, so from the root it refuses. Do not reach for `-D`.
   Leave the milestone's worktree standing

9-1. Before closing, leave one line per member **on what did this work** in this window —
   leaving out the members left in the first column by 7-1 and 4-3, which nobody did. Not the
   suggestion in the message but the model that **actually ran** in this window. The line below
   was filled in by the supervisor as a suggestion, so if you raised it, or the window was on
   a different model from the start, correct the model and the difficulty to the real ones and
   write why in the reason — the next person reads "what was put on work of this size" there.
   It is a note, not a field. The supervisor does not fill `<vendor>` or `<count>` — the vendor is
   `anthropic`, and the model is its real name (`opus-5`), not the `/model` alias — the
   `<model>` the supervisor filled in is an alias (`opus`), so write the real name even if you
   did not change models.
   `<count>` is the tokens this window used. **If you do not know the token count, drop
   `tokens=<count>` whole** — do not write 0 and do not estimate. **One line per id** —
   write the same line on several ids and the tokens multiply by the number of ids. A window's
   tokens cannot be split per member, so write them on **one member only** and leave
   `tokens=<count>` out of the other members' lines.
   Quote free text with single quotes — inside double quotes the shell expands backticks and
   `$(…)` as commands. If the text itself contains a single quote, stream it from stdin with `-b -`

    moai note <member> 'model: <vendor>/<model> tokens=<count> (<difficulty> — <why>)'

10. Close them after that. **Run `moai mv <member> done` only once that merge has really
    landed** — closed before it, a merge that stops on a conflict leaves them done on work
    that is not in. It closes a member from `review`, where 7 stood it, and from
    `in_progress` where there is no `review` column alike. Do not close the
    members left in the first column by 7-1 and 4-3 — those members keep the epic open. While
    the worktree still stands, the hook reads this work as a sibling worktree's and cannot
    refuse a review closed without `-m`. Close the review issue leaving what came out of it

{close}
{over}

    Leave the tests passing in the root with a commit with a path, as in 2
11. Leave the line to take over from — `moai note <epic> 'Next: …'` — and take it into the root
    with a commit with a path as in 2. It is written after the commit in 10, so leaving it out
    leaves it in the shared root where someone else's commit sweeps it up. If anything is left
    (a background review, say), finish it before the note — the supervisor reads the note as
    this work being over; what you cannot finish, name in the report (12)
12. Report to the supervisor, **last of all**, with `SendMessage(to: <supervisor>, message: …)`
    — `report: <epic>` at its head. It carries the merge hash,
    the unfolded epic's id, a line or two of summary, what you handed on and any new backlog,
    the members reclaimed in 7-1 and left in the first column,
    the members left in 4-3 because the work beside you held the file, with that other work
    named, and the wiki pages 7-4 changed — or that it changed none — and anything still
    running that 11 could not finish.
    **If that send fails** — the supervisor restarted, so its old name is gone — the report
    must not be lost: leave the same text, `report: <epic>` at its head, on the epic with
    `moai note <epic> -b -` and take it into the root with a commit with a path as in 2. Tell
    the person watching that the report is on the epic; the next supervisor reads it there.
    Then **say when the window can be cleared**, in one line to the person watching. The
    context lives in the tracker, not in the conversation: issue bodies, notes, review texts,
    commit messages. If you can see your own context usage, put that number in the line too.
    **Say the opposite in the same line** — not to clear while a review is running in the
    background, while a merge conflict is being resolved, while waiting on a person's answer,
    or after the supervisor's next message has arrived in this window. Clearing (`/clear`) then
    loses what is not yet moved into the tracker, or the message that arrived.
    Then end the turn. The supervisor's next message is the next work

## Carrying on stalled work

A session died holding a member of `<epic>`, the epic the message's first line names; that
member still stands picked up. Read how far it got (`moai show <epic>`, its history and
notes), then

- If the worktree is there, go in with `EnterWorktree(path)`, read how far it got with
  `git log <base branch>..HEAD` and `git status`, and carry on
- If it is not, raise it again from the root. If the branch survives, on that branch
  (`git worktree add .worktrees/<epic> worktree-<epic>`); if it does not,
  `git worktree add -b worktree-<epic> .worktrees/<epic> <base branch>` — inside a milestone,
  raise the milestone branch first if it is missing ("The milestone branch")
- **If the root is not the top of the repository** (a subdirectory project in a
  monorepo), go into the worktree and then move to the same subdirectory inside it and
  work there — standing at the top, `moai` finds and writes the root's `.moai`, and the
  hook does not count edits under `.worktrees/`. `<subdir>` is that relative path, filled in
  by the supervisor; with no `Subdir:` in the message, the root is the top and this step
  does not exist

      {SUBDIR}
- The member's column is already picked up — do not pick it up again. **If its assignee
  is not you** (`moai show <member>`), ask the person watching before you carry it on; on
  a yes, `moai mv <member> <its column> --from <its column> --take -m '<who said yes>'` — the
  column stays, the assignee becomes you, and a note keeps whose it was
- The note in 9-1 records this window's share only. Append `reclaimed work, the previous
  session's share is unknown` to the end of the reason — the previous session's model and
  tokens are written nowhere, and without it the whole member reads as this window's work
- Then go on with the steps from 4-1 to the end
"#
    )
}

/// 줄마다 앞에 붙인다. 빈 줄은 빈 채로 둔다 — 꼬리 공백은 diff 를 더럽힌다.
fn indent(text: &str, by: &str) -> String {
    text.lines().map(|l| if l.is_empty() { String::new() } else { format!("{by}{l}") }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 일꾼 글의 번호 걸음 — "The steps" 절 하나. 그 앞(맡은 일·사람이 비울 때·걸음 앞)과 뒤(이어받기)는 번호가
    /// 없는 절이라, 번호 목록의 규칙을 잴 때는 이 토막만 본다.
    fn numbered_steps(worker: &str) -> &str {
        let head = "## The steps\n\n";
        let from = worker.find(head).expect("일꾼 글에 걸음 절이 없다") + head.len();
        let to = worker[from..].find("\n## ").map_or(worker.len(), |at| from + at);
        &worker[from..to]
    }

    /// 일꾼 걸음의 `n` 번째가 여는 자리 — 줄 머리의 `<n>. `. 첫 걸음은 글의 맨 앞이라 앞에 줄바꿈이 없다.
    /// 들여쓴 `1. `(다섯 자리의 번호)는 걸음이 아니다.
    fn step_at(brief: &str, n: &str) -> usize {
        let open = format!("{n}. ");
        if brief.starts_with(&open) {
            return 0;
        }
        brief.find(&format!("\n{open}")).map(|at| at + 1).unwrap_or_else(|| panic!("일꾼 걸음에 {n} 이 없다"))
    }

    /// 감독이 채우는 자리 목록 한 줄 — 셋이 이 줄에서 자리 이름을 센다. 손으로 세 벌 찾던 판은
    /// 줄의 모양이 바뀔 때 한 곳만 고쳐졌다.
    fn slot_list(supervise: &str) -> &str {
        supervise.lines().find(|l| l.starts_with("Fill in `<id>`")).expect("감독이 채울 자리 목록이 없다")
    }

    /// **공통 조각은 두 표면에 똑같이 든다.** 한쪽만 고치면 여기서 붉어진다 —
    /// 조각을 거치지 않고 표면에 글을 직접 적는 순간 두 벌이 다시 생긴다.
    #[test]
    fn both_surfaces_carry_the_same_pieces() {
        let (agents, skill, reference) = (agents(), skill(), reference());
        for piece in [CHEATSHEET, FORKS, NO_GATE, WRITING, CLOSING] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(skill.contains(piece), "스킬에 없다 — {head}");
        }
        assert!(CLOSING.contains(&handoff("<id>")), "안내의 핸드오프 줄이 훅과 갈라졌다");
        let rules = rules();
        assert!(agents.contains(&rules) && skill.contains(&rules), "규칙 셋이 갈라졌다");
        let backlog = backlog();
        for piece in [GROUPS, backlog.as_str(), DEFERRING, PEOPLE, PROJECTS, LANGUAGE, UPDATES, COMMITS, WIKI] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(reference.contains(piece), "참고 문서에 없다 — {head}");
        }
    }

    /// **심는 글은 한국어 글 다듬기를 시키지 않는다**(사용자 결정 moai-vtfu, 2026-10-03). 그 절(moai-5wk4)과
    /// 용어 규칙(moai-iloh)을 걷었다 — 글은 기본으로 쓴다. 한 표면에만 남으면 그 표면을 읽은 세션이 깔리지도
    /// 않은 플러그인을 부르려 든다. 심는 다섯 글과 브리프를 다 훑는다.
    #[test]
    fn no_surface_asks_for_the_korean_writing_plugins() {
        for (surface, text) in [
            ("AGENTS 블록", agents()),
            ("SKILL.md", skill()),
            ("참고 문서", reference()),
            ("감독 스킬", supervise()),
            ("위키 스킬", wiki()),
            ("일꾼 글", worker()),
        ] {
            for gone in ["Korean text", "korean-skills", "humanize-korean", "_workspace", "korean-terms"] {
                assert!(!text.contains(gone), "{surface} 에 걷은 한국어 글 절이 남았다 — {gone}");
            }
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
        // `❯` 도 뺀다 — Claude Code 의 입력 칸 머리라 tmux 스킬이 그 글자로 칸을 알아본다(moai-u99i). 한 칸 너비의 딩뱃이다.
        let emoji = |c: char| {
            !matches!(c, '✓' | '❯')
                && (c >= '\u{1F300}' || matches!(c, '\u{FE0F}' | '\u{2600}'..='\u{27BF}' | '\u{2B00}'..='\u{2BFF}'))
        };
        for (surface, text) in [
            ("AGENTS 블록", agents()),
            ("스킬", skill()),
            ("참고 문서", reference()),
            ("감독 스킬", supervise()),
            ("tmux 스킬", tmux()),
            ("일꾼 글", worker()),
        ] {
            let found: String = text.chars().filter(|c| emoji(*c)).collect();
            assert!(found.is_empty(), "{surface} 이 이모지를 쓴다 — {found}");
        }
    }

    /// **심는 글은 사람 없이 도는 고리를 가르치지 않는다**(moai-jo1u). moai 바이너리는 세션을 띄우지도
    /// 헤드리스로 몰지도 않는다(2026-10-04 사용자 결정) — 그래서 그 고리를 보이던
    /// `examples/bash-agent`·`examples/python-agents` 를 걷었다. 글에 경로가 남으면 모든 저장소에
    /// 없는 파일을 가리키고, 사람 없이 돌리라는 말을 그대로 심는다.
    ///
    /// **바꾼 말이 서 있는지도 본다.** 걷은 낱말만 재면 문단이 통째로 빠져도 초록이다 —
    /// `ready --json` 의 꼴을 읽는 쪽이 사람이 띄운 세션이라는 것이 이 문단이 남긴 뜻이다.
    ///
    /// **tmux 안의 감독은 칸을 비우고, 붙이고, 사람이 그러라면 연다**(2026-10-10 사용자 결정, moai-u99i) — 그래서 "moai 는
    /// 세션을 띄우거나 몰지 않는다" 를 **바이너리**의 말로 좁히고 그 곁에 감독의 tmux 갈래를 적었다. 둘 중 하나만 서면
    /// 블록과 스킬이 다른 말을 한다.
    #[test]
    fn the_planted_text_teaches_no_headless_loop() {
        for (surface, text) in [
            ("AGENTS 블록", agents()),
            ("스킬", skill()),
            ("참고 문서", reference()),
            ("감독 스킬", supervise()),
            ("tmux 스킬", tmux()),
            ("일꾼 글", worker()),
        ] {
            for gone in ["examples/bash-agent", "examples/python-agents", "runs without a person"] {
                assert!(!text.contains(gone), "{surface} 이 걷은 헤드리스 예제를 아직 가리킨다 — {gone}");
            }
        }
        for (surface, text) in [("AGENTS 블록", agents()), ("스킬", skill())] {
            assert!(
                text.contains("the moai binary never launches or drives a session itself"),
                "{surface} 이 바이너리가 세션을 띄우지 않는다는 말을 잃었다"
            );
            assert!(
                text.contains("Only the supervisor skill, inside\ntmux,") && text.contains("once the person says yes"),
                "{surface} 이 tmux 안의 감독이 칸을 만진다는 말을 안 한다"
            );
            assert!(
                !text.contains("moai never launches or drives"),
                "{surface} 이 옛 말(감독도 안 만진다)을 들고 있다"
            );
        }
        let block = agents();
        let team = &block[block.find("### The supervisor and its workers").expect("감독 절이 없다")..];
        assert!(
            team.contains("the moai binary never\nlaunches or drives a session"),
            "감독 절이 바이너리의 말을 잃었다"
        );
        assert!(
            team.contains("`moai-tmux`") && team.contains("after the person says\nyes"),
            "감독 절에 tmux 갈래가 없다"
        );
        assert!(!team.contains("never launches a session;"), "감독 절이 옛 말을 들고 있다");
        // **리뷰는 일꾼의 세션 안에서 돈다**(moai-5kk1) — 새 에이전트를 띄우는 명령이 리뷰의 낱말로 서면 감독도
        // 일꾼도 에이전트를 안 띄운다는 결정이 깨진다. 일꾼 글은 Claude Code 만 받으니(moai-obxm) 다른 벤더의 그
        // 명령을 이름으로 댈 까닭도 없다.
        let spawns = ["codex review", "codex exec", "agy -p"];
        for (surface, text) in [
            ("낱말표", verbs_section()),
            ("규칙 셋", rules()),
            ("감독 스킬", supervise()),
            ("tmux 스킬", tmux()),
            ("일꾼 글", worker()),
        ] {
            for spawn in spawns {
                assert!(!text.contains(spawn), "{surface} 이 새 에이전트를 띄우는 {spawn} 를 가르친다");
            }
        }
        let brief = worker();
        let review = &brief[step_at(&brief, "7")..step_at(&brief, "7-1")];
        assert!(review.contains("never start another agent program"), "7 이 다른 에이전트를 띄우지 말라고 안 한다");
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

    /// **넘겨받는 줄은 한 꼬리로 선다**(moai-0zjo 리뷰) — 규칙 5 의 글([`TAKE_OVER`])과 거절문·`mv` 의 알림
    /// ([`TAKE_YES`])이 갈리면, 글을 고친 사람은 거절문이 따라온다고 믿고 거절문은 옛 글을 낸다.
    #[test]
    fn the_take_over_line_ends_with_its_tail() {
        assert!(TAKE_OVER.ends_with(&format!(" {TAKE_YES}")), "{TAKE_OVER}");
        assert!(rules().contains(TAKE_OVER), "규칙 5 의 글이 넘겨받는 줄을 안 싣는다");
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
    /// 문서·그때 있던 한국어 절차에 손으로 적은 `64KB` 를 안 봐, 상한을 올리면 그 셋이 옛 수를 대는 채로
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
        for (surface, text) in [
            ("AGENTS 블록", agents()),
            ("SKILL.md", skill()),
            ("참고 문서", reference()),
            ("감독 스킬", supervise()),
            ("일꾼 글", worker()),
        ] {
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
        assert!(rules().contains(REVIEW_OVER_LIMIT), "규칙 3 이 넘칠 때의 길을 안 댄다");
        assert!(reference().contains(REVIEW_OVER_LIMIT), "참고 문서가 넘칠 때의 길을 안 댄다");
        assert!(worker().contains(&indent(REVIEW_OVER_LIMIT, "    ")), "일꾼 걸음이 넘칠 때의 길을 안 댄다");
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
                ["감독해 줘", "backlog 나눠 줘", "놀고 있는 세션에 일 시켜"].as_slice(),
            ),
            // 위키 스킬(moai-bl3x)의 발동어. 사람이 "위키 갱신" 이라고 불러야 훑기가 선다.
            ("moai-wiki", head(&wiki()), ["위키 갱신", "매뉴얼 써", "문서화해 줘", "wiki 정리"].as_slice()),
            // tmux 스킬(moai-u99i)은 감독이 `$TMUX` 안에서 부른다 — 사람이 칸을 이름으로 부를 때도 서야 한다.
            ("moai-tmux", head(&tmux()), ["일꾼 칸", "일꾼 창 열어", "칸 비워"].as_slice()),
            // 되살리기 스킬(moai-uqf7)은 사람이 부를 때만 선다 — 2026-10-10 에 사람이 쓴 말이 "되살려" 였다.
            ("moai-recover", head(&recover()), ["되살려", "세션 복구", "이어 가게 해"].as_slice()),
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
        // 이름은 `skill::NAMES`·`skill::OPTIONAL` 의 것이다 — 위키가 스킬 이름을 id 에서 거르는 `skill::EVER_PLANTED` 는 그
        // 목록을 다 든다(moai-mdzx.3pm, moai-six5.1xz). 머리의 이름이 그 목록과 갈리면 고친 이름이 다시 없는 id 로 선다.
        let [moai, supervisor, wiki_skill, recover_skill] = crate::skill::NAMES;
        let [crate::skill::Optional { name: tmux_skill, .. }] = crate::skill::OPTIONAL;
        assert!(skill.starts_with(&format!("---\nname: {moai}\ndescription: ")), "{head}");
        assert!(supervise().starts_with(&format!("---\nname: {supervisor}\ndescription: ")), "감독 스킬의 머리가 없다");
        assert!(wiki().starts_with(&format!("---\nname: {wiki_skill}\ndescription: ")), "위키 스킬의 머리가 없다");
        assert!(tmux().starts_with(&format!("---\nname: {tmux_skill}\ndescription: ")), "tmux 스킬의 머리가 없다");
        assert!(
            recover().starts_with(&format!("---\nname: {recover_skill}\ndescription: ")),
            "되살리기 스킬의 머리가 없다"
        );
        // 일꾼 글은 스킬이 아니라 감독 스킬의 참고 파일이다(moai-obxm) — 머리가 서면 그 글이 메시지 한가운데 YAML 로 선다.
        assert!(worker().starts_with("# Worker steps\n"), "일꾼 글이 제목으로 안 연다");
    }

    /// **감독이 일꾼에게 가르치는 펼치기는 참고 문서의 그 명령이다.** 길이 둘로
    /// 갈라지면 일꾼마다 다르게 고른다 — 첫 실행에서 `add` 로 새 줄을 세운 일꾼이 있었다.
    /// 리뷰 이슈를 세우는 줄은 규칙 3 의 그 조각이라 빼고 센다 — `moai add` 를 통째로
    /// 막던 판은 그 조각을 못 실어, 관점(`-b`) 없는 줄을 손으로 줄여 적었다.
    #[test]
    fn the_supervisor_teaches_promote_as_the_one_way() {
        let (supervise, reference, brief) = (supervise(), reference(), worker());
        let work = &brief;
        let promote = "moai backlog promote <id> --from -";
        assert!(reference.contains(promote), "참고 문서의 펼치기 줄이 바뀌었다");
        assert!(work.contains(promote), "일꾼 글이 promote 를 안 가르친다");
        assert!(supervise.contains("`moai backlog promote`"), "감독이 일꾼이 무엇으로 펼치는지 모른다");
        let review = make_review("--parent <epic>");
        assert!(brief.contains(&review), "에픽 리뷰를 규칙 3 의 줄로 안 세운다");
        assert!(!supervise.contains("moai add"), "감독이 promote 말고 다른 길을 가르친다");
        assert!(!work.replace(&review, "").contains("moai add '<"), "일꾼이 promote 말고 다른 길로 펼친다");
        // 되짚기(7-1)의 줄도 promote 이고, 루트의 그 에픽을 가리킨다. `brief.contains(RECALL)` 는
        // 글이 그 상수를 끼워 넣는 한 늘 참이라, 줄의 모양은 여기서 따로 맨다 — `moai backlog add` 로
        // 바꿔 7-1 이 거꾸로 backlog 로 내보내라고 가르쳐도 다른 시험은 다 초록이었다.
        assert!(
            RECALL.starts_with("moai -C <root> backlog promote ") && RECALL.contains(" -e <epic> "),
            "되짚기가 루트의 그 에픽에 promote 로 멤버를 세우지 않는다 — {RECALL}"
        );
        // 되짚기의 자리는 일꾼이 채운다 — 감독이 채우는 목록(3)에 같은 이름이 들면 맡긴 backlog 의 값이
        // 그 줄에 미리 박혀 온다(첫 판의 `<제목>` 이 그랬다). `<root>` 만 감독이 채우라고 둔 자리다.
        let list = slot_list(&supervise);
        let slots = brief[step_at(&brief, "7-1")..step_at(&brief, "8")]
            .split('<')
            .skip(1)
            .filter_map(|s| s.split_once('>'))
            .map(|(s, _)| format!("`<{s}>`"));
        for slot in slots.filter(|s| s != "`<root>`") {
            assert!(!list.contains(&slot), "감독이 되짚기의 자리 {slot} 를 채운다 — {list}");
        }
    }

    /// **일꾼의 걸음(`worker`)에 첫 실행에서 넘어진 자리가 선다.** 감독 스킬에만 적으면 시험은 초록인데
    /// 일꾼은 못 읽는다 — `MERGE_HEAD` 가 실제로 그랬다. 하나라도 빠지면 다음 일꾼이 같은 자리에서 또
    /// 넘어진다. 걸음은 감독 스킬의 `references/worker.md`([`worker`])에 서고, 감독의 메시지([`message`])가 그것을 읽으라고
    /// 이른다(moai-fim6).
    #[test]
    fn the_worker_brief_carries_what_the_first_run_tripped_on() {
        let (brief, supervise) = (worker(), supervise());
        let work = &brief;
        assert!(supervise.contains(&message()), "감독이 보내는 메시지의 머리가 message 가 아니다");
        // 걸음은 감독 SKILL.md 가 아니라 그 참고 파일(`references/worker.md`)에 선다 — 감독은 보낼 때만 읽는다.
        assert!(!supervise.contains(&brief), "감독 SKILL.md 가 일꾼 걸음 전부를 싣는다");
        let review = make_review("--parent <epic>");
        for (piece, why) in [
            // 거절은 돌아가지 않고 거절문의 명령대로 고친다 — 옛 바이너리를 위한 서브에이전트 우회는 걷었다(moai-fim6).
            ("is not worked around: fix it the way the", "훅의 거절을 우회한다"),
            ("only once that merge has really", "병합 전에 done 으로 옮기지 말라는 말이 없다"),
            ("Do not review member by member", "멤버 리뷰를 걷었다는 말이 없어 일꾼이 멤버마다 리뷰한다"),
            ("`low`·`medium`·`high`", "멤버를 잴 난이도의 폭이 없다"),
            ("/code-review <grade> --fix", "에픽 끝의 xhigh·max 리뷰가 없다"),
            ("in one line in the angle (`-b`)", "고른 등급의 까닭을 남기라는 말이 없다"),
            (review.as_str(), "에픽 리뷰 이슈를 관점과 함께 에픽에 매는 줄이 없다"),
            ("The tracker you edit is always the root's", "워크트리의 트래커를 고쳐 병합에서 스냅샷이 충돌한다"),
            ("`MOAI_HERE`", "옮김을 끄는 손잡이를 켜지 말라는 말이 없다"),
            ("review subagent", "서브에이전트가 워크트리의 .moai 를 고친다"),
            ("`ExitWorktree(keep)`", "루트로 돌아오는 걸음이 없다"),
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
        // 에픽이 이미 닫혔다. 없으면 에픽이 내건 것이 backlog 로 빠진 채 닫힌다(moai-l288).
        let recalled = brief.find("7-1. Before merging").expect("병합 전에 backlog 를 되짚는 걸음이 없다");
        // 에픽 가지의 병합으로 찾는다 — 마일스톤 가지 절과 7 의 rebase 금지도 `merge --no-ff` 를 이른다(moai-nvju).
        let merged = brief.find("merge --no-ff worktree-<epic>").expect("병합 걸음이 없다");
        assert!(reviewed < recalled && recalled < merged, "되짚기가 에픽 리뷰 뒤·병합 앞이 아니다");
        assert!(brief.contains(RECALL), "되짚은 것을 멤버로 세우는 줄이 없다");
        assert!(brief.contains("already done"), "누가 펼친 backlog 를 또 펼쳐 에픽이 둘 선다");
        for (piece, why) in [
            // promote 는 닫힌 backlog 도 또 펼친다 — 8 에서 돌아와 다시 도는 7-1 이 같은 멤버를 둘 세운다.
            ("If that backlog is already done, do\n   not unfold it", "다시 도는 되짚기가 닫힌 backlog 를 또 펼친다"),
            // 되짚을 것을 창의 기억에만 두면 창을 비우거나 일을 이어받은 창이 아무것도 못 찾는다.
            (
                "Put `-e <epic>` on a backlog item you park mid-epic",
                "도중 담는 backlog 에 에픽을 안 달아 7-1 이 되찾지 못한다",
            ),
            ("show --type backlog -e <epic>", "도중 담은 backlog 를 트래커에서 찾는 길이 없다"),
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
            ("A worker that refused the work", "맡기기를 거절한 일꾼을 빼라는 말이 없다"),
            ("in this same round against each other", "같은 바퀴에 보낸 둘이 같은 곳을 고친다"),
            ("comes out of the candidates until its report is checked", "보낸 backlog 가 둘째 일꾼에게 또 간다"),
            ("That member is the worker's", "감독이 훅에 떠밀려 일꾼의 멤버를 옮긴다"),
            ("whose sent work has not had its report checked", "맡긴 일을 하던 일꾼에게 또 맡긴다"),
            ("moai show <epic>", "backlog 로 확인하면 멤버가 안 보인다"),
            // 목록의 끝은 `<subdir>` 이 붙어 바뀌었다(리뷰 moai-rgp9.sdj 1번) — 자리 이름만 맨다.
            ("`<root>`", "감독이 루트 자리를 안 채워 일꾼이 제 워크트리를 루트로 읽는다"),
            ("`<subdir>`", "모노레포 하위 자리를 감독이 안 채워 일꾼이 거절되는 꼴로 구한다"),
            // 거둔 일은 일꾼 글의 그 절로 보낸다 — 첫 줄이 그 절을 이름으로 댄다.
            ("from \"Carrying on stalled work\"", "거둔 일의 메시지가 일꾼 글의 어느 절인지 안 댄다"),
            // `gone` 은 20분 조용했다는 것이지 끝났다는 것이 아니다(리뷰 moai-bkn4.c3d) — 사람의 답을 기다리며 프롬프트에 쉬는
            // 다른 기계·Codex 일꾼도 그렇게 읽혀, 그 워크트리를 둘째 일꾼에게 넘기면 산 두 세션이 한 가지에 선다.
            ("only once the person says that window has ended", "20분 조용한 일꾼의 워크트리를 둘째 일꾼에게 넘긴다"),
            // 세션 이름은 프로세스의 것이라 `claude --resume` 으로 되살아난 일꾼은 새 이름으로 선다(moai-ybns.451.ed8) —
            // 옛 이름이 목록에서 빠진 것을 끝난 것으로 읽으면 산 워크트리를 둘째 일꾼에게 넘기고, 이름으로 적어 둔
            // "보낸 일"·"거절" 을 놓쳐 그 창에 일을 또 맡긴다.
            ("with `claude --resume` comes back under a new name", "이름이 바뀐 일꾼의 워크트리를 끝난 것으로 읽는다"),
            ("Look for the worker by its worktree, not its name", "멈춘 워크트리의 일꾼을 이름으로 찾는다"),
            ("may be that worker — ask the person before sending to it", "되살아난 일꾼에게 일을 또 맡긴다"),
            // 7-1 이 첫 칸에 남긴 멤버는 에픽을 연 채 둔다 — 감독의 확인(5)이 그것을 어긋남으로 읽으면
            // 시킨 대로 한 보고마다 그 창이 안 비워지고 다음 backlog 도 못 받는다.
            (
                "A member the report says was left in the first column by the worker's 7-1",
                "감독이 일부러 남긴 멤버를 어긋난 보고로 읽는다",
            ),
        ] {
            assert!(supervise.contains(piece), "{why} — {piece}");
        }
        // 거둔 일의 절은 4-1 부터 끝까지 잇는다 — 그 범위가 4-1 위에서 끊기면 이어받은 일꾼만 워크트리의
        // `.moai` 를 고친다. **끝은 번호로 적지 않는다**: `11 까지` 로 적어 둔 뒤 12 가 붙자 이어받은 일꾼만 12 를
        // 못 받았다.
        let stalled = &work[work.find("## Carrying on stalled work").expect("거둔 일의 절이 없다")..];
        assert!(stalled.contains("from 4-1 to the end"), "거둔 일이 4-1 을 빼거나 끝을 자른다");
        // 거둔 일도 새 일과 같은 메시지를 받는다 — 모델 줄은 그 머리의 한 줄이고, 읽는 법은 일꾼 글에 한 번 선다.
        assert!(message().contains(MODEL_SLOT), "메시지에 모델 줄이 없다");
        assert!(work.contains(&indent(&model_rule(), "  ")), "일꾼 글이 모델 줄을 어떻게 읽는지 안 댄다");
        // **일꾼이 하는 일을 바꾸지 않는 내력·측정은 걸음에 안 싣는다**(moai-fim6) — 규칙은 남고 그 까닭의 숫자와 옛 판의
        // 우회만 걷었다. 내력은 주석과 커밋에 선다.
        for (gone, why) in [
            ("714", "git 꼴의 측정 수를 싣는다"),
            ("cases)", "git 꼴의 건수를 싣는다"),
            ("v0.1.1", "7-3 이 옛 릴리스의 내력을 싣는다"),
            ("91 lines", "7-3 이 옛 릴리스의 내력을 싣는다"),
            ("binary from before", "7 이 옛 바이너리를 위한 우회를 싣는다"),
            (".claude/worktrees", "거둔 일이 옛 워크트리 자리를 싣는다"),
            ("that has happened", "걸음이 일어난 일의 내력을 싣는다"),
        ] {
            assert!(!work.contains(gone), "{why} — {gone}");
        }
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
        let (agents, supervise, brief) = (agents(), supervise(), worker());
        // 감독 스킬은 이제 걸음을 안 품는다(moai-snyk) — 통째로 재도 일꾼의 같은 글이 메우지 않는다.
        assert!(!supervise.contains(&brief), "감독 스킬이 일꾼의 걸음을 품는다");
        let head = supervise.as_str();

        for (piece, why) in [
            ("While a milestone is running, what is inside it comes first", "AGENTS 블록에 규칙이 없다"),
            ("if even one member stands in a started\ncolumn, it is running", "무엇으로 시작을 재는지 안 적었다"),
            ("**`p0` gets picked up whether or not it is in the milestone**", "핫픽스 자리를 안 적었다"),
            (
                "**Nothing is blocked.** A `moai mv` that picks up work from outside goes straight\n  through",
                "막지 않는다는 것을 안 적었다",
            ),
            ("**Work is never pulled into a running milestone.**", "밖의 일을 안으로 끌어오지 않는다는 줄이 없다"),
            ("three doors put work in afterwards — the person opens two of them", "누가 들이는지를 안 적었다"),
            ("they say yes to a plan you", "사람이 예 한 계획이 드는 문으로 안 서 있다"),
            (
                "**A running milestone is the person's to fill.**",
                "계획에 도는 릴리스를 제 손으로 달지 말라는 줄이 없다",
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

    /// **밖의 backlog 는 마일스톤을 달아야 들어온다**(moai-6qgz, 2026-09-21).
    ///
    /// 앞 시험이 매는 "안의 것이 먼저다" 는 감독이 **무엇을 고를지**만 정한다. 고른 것이 밖의
    /// backlog 일 때 그것을 안으로 들이는 길은 아무 데도 없었고, 그래서 감독이 브리프에 "마일스톤은
    /// 달지 마라" 고 적어 일꾼이 도는 판 밖의 일을 집었다. 도구는 그것을 그대로 지나 보낸다.
    ///
    /// **2026-09-25 에 뜻이 뒤집혔다.** 그때까지 이 자리는 "밖의 backlog 는 마일스톤을 달아야
    /// 들어온다" 였고, 감독이 들일지를 정했다. 사용자가 도는 마일스톤에 에이전트가 제 판단으로
    /// 밖의 줄을 달고 일한 판을 보고, 들이는 것은 사람만 하기로 정했다 — 감독은 밖의 일을
    /// **안 보내는** 것으로 답한다. 2026-09-21 의 사고(일꾼이 도는 판 밖의 일을 집었다)는
    /// 그대로 글에 남고, 답만 "달아 준다" 에서 "안 보낸다" 로 바뀐다.
    ///
    /// **두 글이 한 줄에 매인다.** 감독의 1 이 무엇을 보낼지 정하고 일꾼의 1 이 헤더에 실린
    /// 릴리스만 단다 — `MILESTONE_ATTACH` 하나에서 둘 다 나오므로, 한쪽만 고치면 여기서 붉어진다.
    #[test]
    fn an_outside_backlog_is_not_pulled_into_a_running_release() {
        let (supervise, brief) = (supervise(), worker());
        let head = supervise.as_str();

        for (piece, why) in [
            (MILESTONE_ATTACH, "감독이 일꾼이 다는 줄을 안 가리킨다"),
            (
                "**Work is never pulled into a running milestone — the supervisor does not bring an
outside backlog in.**",
                "밖의 backlog 를 안 들인다는 줄이 없다",
            ),
            (
                "a backlog item from
outside waits for the next round",
                "밖의 backlog 가 다음 회차로 미뤄진다는 말이 없다",
            ),
            (
                "The answer is to
stop sending outside work while a release runs",
                "2026-09-21 의 답이 무엇으로 바뀌었는지 안 적었다",
            ),
            ("**The tool refuses none of this**", "감독이 도구가 막아 줄 것으로 읽는다"),
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
        let (one, two) = (step_at(&brief, "1"), step_at(&brief, "2"));
        assert!(one < two, "일꾼 걸음의 1 이 2 뒤에 섰다");
        for (piece, why) in [
            (MILESTONE_ATTACH, "일꾼이 마일스톤을 다는 줄이 1 에 없다"),
            ("If `<milestone>` is `none`", "아무것도 안 도는 판을 안 적었다"),
            (
                "**Hang only the `<milestone>` in the message, and nothing else**",
                "일꾼이 제 손으로 도는 릴리스를 달지 말라는 줄이 1 에 없다",
            ),
        ] {
            assert!(brief[one..two].contains(piece), "{why} — {piece}");
        }
    }

    /// **난이도 한 낱말이 모델과 리뷰 등급을 함께 정한다**(moai-84kd, 2026-09-15 사용자 결정).
    ///
    /// 축을 따로 두면 브리프가 판단을 두 벌 들고, 둘이 어긋나는 날 싼 모델이 쓰기 경로를 맡는다.
    /// 그래서 짝은 리뷰 등급과 같은 낱말 위에 선다 — low·medium·high 가 그대로 haiku·sonnet·opus 다.
    /// **브리프에 실려야 뜻이 있다**: 감독 스킬에만 적힌 규칙은 일꾼이 받는 글에 없다(`worker`).
    #[test]
    fn the_supervisor_picks_a_model_by_difficulty() {
        let (supervise, brief) = (supervise(), worker());
        // 감독 스킬은 걸음을 안 품는다(moai-snyk) — 통째로 재도 브리프의 같은 글이 감독 쪽에서 빠진 자리를
        // 메우지 않는다(표의 잣대가 한때 그렇게 가려졌다).
        assert!(!supervise.contains(&brief), "감독 스킬이 일꾼의 걸음을 품는다");
        let head = supervise.as_str();
        // **잣대는 한 벌이다.** 표도 일꾼의 잣대도 `DIFFICULTY` 에서 나온다 — 한쪽을 손으로
        // 다시 적으면 여기서 붉어진다.
        let table = difficulty_table();
        assert!(head.contains(&table), "2-1 의 표가 DIFFICULTY 에서 안 나온다");
        assert!(brief.contains(&indent(&difficulty_rubric(), "   - ")), "일꾼이 받는 잣대가 DIFFICULTY 에서 안 나온다");
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
        let step = &brief[step_at(&brief, "7")..step_at(&brief, "8")];
        assert!(step.contains("/code-review <grade> --fix"), "에픽 리뷰 걸음이 7 에 없다");
        assert!(step.contains(&indent(&rule, "   ")), "브리프 7 에 에픽 끝 등급·모델 규칙이 없다");
        assert!(step.contains("`/model opus`"), "에픽 끝 리뷰의 모델을 맞추라는 말이 일꾼에게 없다");
        // 훅에 막힐 때 리뷰 서브에이전트로 돌리던 길은 옛 바이너리의 것이라 걷었다(moai-fim6) — 리뷰는 이 창의
        // `/code-review` 하나고, 그 모델은 바로 위의 `/model` 로 맞춘다.
        assert!(!step.contains("`Agent`'s `model`"), "옛 바이너리를 위한 리뷰 서브에이전트 우회가 남았다");
        // 일꾼이 받는 메시지에 그 자리가 있어야 감독이 채운다. **목록 줄에서 찾는다** — 바로 아래
        // 풀이 글도 세 자리를 적어, 감독 쪽 전체에서 찾으면 목록에서 빠져도 초록이었다.
        assert!(message().contains(MODEL_SLOT), "메시지에 모델 자리가 없다 — 감독이 골라도 일꾼은 모른다");
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
        let brief = worker();
        assert!(claude.contains(EPIC_MAX), "CLAUDE.md 의 에픽 끝 max 줄이 브리프와 갈라졌다 — {EPIC_MAX}");
        assert!(brief.contains(EPIC_MAX), "브리프 7 이 에픽 끝 max 줄을 안 쓴다 — {EPIC_MAX}");
        // 에픽 둘에 걸친 묶음도 같은 꼴로 두 자리에 선다(moai-h89f) — 표에만 적으면 일꾼이
        // 받는 것은 브리프뿐이라 그 판을 재는 잣대가 리뷰를 부르는 창에 한 번도 안 닿는다.
        assert!(claude.contains(BUNDLE), "CLAUDE.md 의 리뷰 표에 에픽 둘에 걸친 묶음이 없다 — {BUNDLE}");
        assert!(brief.contains(BUNDLE), "브리프 7 이 묶음의 등급을 안 쓴다 — {BUNDLE}");
    }

    /// **리뷰가 도는 워크트리는 그 리뷰의 것이다**(moai-ww2u·moai-bu9i). 값을 실제로 잃은 두
    /// 판이고, 둘 다 브리프에 줄이 없어서 났다.
    ///
    /// 앞은 2026-09-21 — `--fix` 가 도는 워크트리에서 커밋 제목을 고치려고 친 한 줄이 리뷰가
    /// 이미 고쳐 둔 커밋 안 된 변경을 지웠다. 시킨 것은 감독이었으니 일꾼의 판단 실수가 아니라
    /// 브리프가 낸 사고다. 뒤는 2026-09-20 — 리뷰가 끝나 판단까지 적은 뒤에 그 리뷰가 세운
    /// 마지막 서브에이전트가 같은 워크트리의 파일을 제 판으로 다시 써, 커밋해 둔 되돌림을
    /// 덮었다. 그 뒤 돌린 시험의 초록은 그 에이전트의 파일을 잰 초록이었다.
    ///
    /// **훅으로 막지 않는다**(backlog `moai-uvs9` 의 두 길 가운데 글 쪽). 게이트를 하나 더 세우는
    /// 것은 "막지 않는다" 와 겨루고 그쪽이 더 무겁다 — 그래서 지키는 것은 읽는 사람이고, 이
    /// 시험이 그 줄이 서 있는지만 본다.
    #[test]
    fn a_running_review_owns_the_worktree() {
        let brief = worker();
        // **끝을 못 찾으면 붉어진다.** 여기서 브리프 끝으로 물러서면 7-1 의 이름이 바뀐 판에도
        // 아래 낱말이 한참 뒤의 걸음에서 걸려, 자리를 못 잡는 시험이 초록으로 선다.
        let step = &brief[step_at(&brief, "7")..step_at(&brief, "7-1")];
        for (piece, why) in [
            (
                "do not touch this worktree's branch or its working tree",
                "리뷰가 도는 동안 가지를 고치지 말라는 말이 없다",
            ),
            ("`commit --amend`", "고침을 날리는 명령을 이름으로 안 댄다"),
            ("`TaskStop`", "리뷰가 남긴 서브에이전트를 멈추는 걸음이 없다"),
            ("covers a commit you already made", "남은 에이전트가 무엇을 덮는지 안 적었다"),
        ] {
            assert!(step.contains(piece), "{why} — {piece}");
        }
    }

    /// **에픽 끝 리뷰가 도는 동안 멤버는 `review` 칸에 선다**(사용자 결정 2026-10-03, moai-vxld).
    /// 브리프가 칸을 말하지 않아 한 창은 옮기고 여러 창은 `in_progress` 에 두었다.
    ///
    /// **거절의 글을 도구의 글과 견준다.** 브리프는 `review` 칸이 없는 저장소를 그 거절로 알아보게
    /// 하는데, `refuse.no_column` 의 글이 바뀌고 브리프가 옛 글을 들고 있으면 일꾼은 거절을 "이
    /// 걸음이 없다" 로 못 읽고 멈추거나 우회한다. 다시 돌아온 판의 `mv.stale` 도 같다.
    #[test]
    fn the_brief_stands_members_in_review() {
        let brief = worker();
        let step = &brief[step_at(&brief, "7")..step_at(&brief, "7-1")];
        let issue = step.find(&make_review("--parent <epic>")).expect("7 에 리뷰 이슈를 세우는 줄이 없다");
        let moved = step.find(TO_REVIEW).expect("7 이 멤버를 review 칸에 세우지 않는다");
        assert!(issue < moved, "리뷰 이슈보다 먼저 멤버를 옮긴다");
        // 7 의 `--from` 은 2 가 멤버를 세운 칸이다 — 둘이 갈라지면 모든 멤버가 stale 로 남는다.
        assert!(brief.contains("moai mv <member> in_progress --from todo"), "2 가 멤버를 세우는 칸이 바뀌었다");
        assert!(TO_REVIEW.ends_with("--from in_progress"), "7 이 첫 칸의 멤버까지 옮긴다");

        // `mv` 가 실제로 지나는 길(`require_known` → `view::no_such_column`)로 짓는다 — 키를 직접 부르면
        // `mv` 가 다른 거절(`refuse.no_column_nor_rows`)로 바뀌어도 이 시험은 푸르다.
        let two = crate::config::Config::parse("prefix = \"a\"\nstatuses = \"todo, in_progress, done\"\n").unwrap();
        let why = two.require_known("review").expect_err("review 칸이 없는 설정이 review 를 받았다");
        let refusal = crate::view::no_such_column(crate::i18n::Lang::En, &why);
        assert!(refusal.starts_with(NO_REVIEW_COLUMN), "브리프가 옮겨 적은 거절이 도구의 글과 다르다 — {refusal}");
        // 글은 화면 말을 따른다 — 브리프는 영어 글을 "영어로는" 으로만 싣고 무엇을 읽을지를 말한다.
        assert!(step.contains("screen's language"), "다른 말 화면에서 거절을 못 알아본다");
        assert!(
            step.contains(&format!("\"{NO_REVIEW_COLUMN}\"")),
            "칸이 없는 저장소에서 이 걸음을 건너뛰라는 말이 없다"
        );
        let stale = crate::i18n::fill(
            crate::i18n::say(crate::i18n::Lang::En, "mv.stale"),
            &[("id", "<member>"), ("now", "review")],
        );
        assert!(stale.contains("already stands review"), "다시 돌아온 판의 거절 글이 바뀌었다 — {stale}");
        assert!(step.contains("already stands `review`"), "이미 review 에 선 멤버를 두라는 말이 없다");

        let close = &brief[step_at(&brief, "10")..step_at(&brief, "11")];
        assert!(close.contains("from `review`, where 7 stood it"), "10 이 review 에서 닫는다고 말하지 않는다");
    }

    /// **리뷰가 되풀이해 잡는 다섯 자리를 브리프가 싣는다**(moai-jza6). 그 전에는 감독이
    /// 브리프마다 손으로 실어, 실으면 잡히고 안 실으면 리뷰가 그때그때 운으로 잡았다.
    ///
    /// **다섯이 관점을 대신하지 않는다.** 이것만 적은 `-b` 는 어느 에픽에나 같은 글이라, 다음
    /// 사람이 "왜 이 등급이었나" 를 거기서 읽을 수 없다 — 그 말이 함께 서는지 본다.
    #[test]
    fn the_brief_carries_the_five_places_the_review_keeps_finding() {
        let brief = worker();
        // **7 안에 서는지까지 본다.** 브리프 아무 걸음이나 들여쓰기가 같아, 통째로 찾으면
        // 리뷰를 안 부르는 걸음에 실려도 초록이었다.
        let step = &brief[step_at(&brief, "7")..step_at(&brief, "7-1")];
        assert!(step.contains(&indent(REVIEW_ANGLE, "   ")), "브리프 7 의 다섯 자리가 REVIEW_ANGLE 에서 안 나온다");
        assert!(
            REVIEW_ANGLE.contains("They do not stand in for the angle"),
            "다섯이 관점을 대신하지 않는다는 말이 없다"
        );
        for n in 1..=5 {
            assert!(REVIEW_ANGLE.contains(&format!("\n{n}. ")), "{n} 번째 자리가 없다");
        }
        // **`-b` 에만 적으면 리뷰에 안 닿는다** — 리뷰 명령은 이슈를 안 읽는다.
        assert!(
            REVIEW_ANGLE.contains("They have to reach the review itself"),
            "다섯을 리뷰 쪽으로 보내는 길이 없다 — `-b` 에만 적히고 끝난다"
        );
    }

    /// **에픽을 닫는 창이 CHANGELOG 줄을 본다**(moai-umu2). v0.1.1 은 커밋 327개 뒤에 섰는데
    /// 절이 말하던 에픽은 스물넷 가운데 셋뿐이었고, 빠진 아홉을 절을 닫는 창이 131줄로 다시
    /// 썼다. 그 절은 릴리스 워크플로가 판 이름으로 잘라 그대로 배포 글로 쓴다.
    ///
    /// **막지 않는다.** 검사로 두면 게이트고, 빈 절로 배포가 멈추면 안 된다 — `moai status` 가
    /// 경고로 비영 종료하지 않는 것과 같은 자리다.
    ///
    /// **머지보다 앞에 선다.** 8 뒤에는 워크트리가 없고 남는 것은 세션 예닐곱이 함께 쓰는
    /// 루트뿐이라, 거기서 `CHANGELOG.md` 를 고치는 것은 "집은 일은 모두 워크트리에서" 와
    /// "루트는 트래커 커밋과 머지만" 을 한꺼번에 어긴다 — 게다가 그 뒤에 남은 커밋 줄은
    /// `.moai/` 만 담아, 고친 글이 루트에 커밋 없이 남거나 남의 커밋에 쓸려 든다.
    #[test]
    fn closing_an_epic_looks_at_the_changelog() {
        let brief = worker();
        let (at, end) = (step_at(&brief, "7-3"), step_at(&brief, "8"));
        let step = &brief[at..end];
        for (piece, why) in [
            ("CHANGELOG", "무엇을 보는지 안 적었다"),
            ("**Nothing checks this**", "게이트가 아니라는 말이 없다"),
            ("misses what was removed", "커밋 제목만 읽으면 걷은 판이 안 보인다는 말이 없다"),
            ("worktree, before the merge", "워크트리에서 머지 전에 쓰라는 말이 없다"),
            ("-- CHANGELOG.md", "고친 글을 담을 커밋 줄이 없다"),
        ] {
            assert!(step.contains(piece), "{why} — {piece}");
        }
        // **닫기보다도 앞이다** — 멤버가 닫히면 그 창은 규칙 2 로 저장소를 못 고친다.
        assert!(end < step_at(&brief, "10"), "CHANGELOG 를 닫은 뒤에 본다");
    }

    /// **위키 스킬이 대는 명령과 `--json` 키는 위키 저장 에픽(moai-ihu4)이 정한 것이다.** 이 글은
    /// 모든 저장소에 심겨 에이전트가 그 키로 갈라 읽는다 — 키 하나가 어긋나면 `jq` 가 `null` 을 돌려주고,
    /// 에이전트는 "없다" 로 읽는다. 목록은 그 에픽 본문의 모양과 그쪽이 사람에게 물어 정한 것(2026-10-04)
    /// 그대로다.
    const WIKI_KEYS: &[&str] = &[
        "dir",
        "pages",
        "slug",
        "title",
        "path",
        "error",
        "kind",
        "no_dir",
        "outside",
        "not_a_dir",
        "issues",
        "id",
        "exists",
        "links",
        "text",
        "to",
        "anchor",
        "resolved",
        "anchor_resolved",
        "linked_from",
        "linked_from_partial",
        "conflict",
        "too_large",
        "refused",
        "failed",
        "skipped",
        "dir_link",
        "not_utf8",
        "unreadable",
    ];

    /// 위키 명령이 내는데 스킬이 **일부러 안 대는** 키 — `bytes` 는 고칠 것을 안 말하고, `said` 는 사람이 읽는
    /// 말이라 가르면 안 되며("Branch on `kind`, not on the words beside it"), `body` 는 원문이라 페이지 파일을 읽는
    /// 것과 같다. 명령이 새 키를 더하면 [`the_wiki_skill_teaches_the_keys_the_wiki_prints`] 가 붉어지고, 스킬에 적을지
    /// 여기 둘지를 그때 정한다.
    const WIKI_UNTAUGHT: &[&str] = &["bytes", "said", "body"];

    /// **`WIKI_KEYS` 를 실제 `moai wiki --json` 에 묶는다**(moai-ihu4.l2e). 손으로 옮긴 목록만 보던 판은 명령이
    /// 키를 바꿔도 초록이라, 심긴 스킬이 `jq` 에서 `null` 이 되는 키를 가르칠 수 있었다(리뷰 moai-bl3x.vbw 4번).
    ///
    /// 명령과 같은 자(`cmd::wiki::listed_json`·`page_json`)로, 모든 갈래가 서는 위키를 지어 견준다 — 읽은 페이지
    /// (id·링크·충돌), 못 읽은 세 까닭(`too_large`·`refused`·`failed`), 디렉터리 거절 넷(`no_dir`·`outside`·
    /// `not_a_dir`·`failed`), 그리고 `show` 의 한 페이지.
    ///
    /// - 스킬이 대는 키와 `kind` 값은 모두 출력에 선다 — 출력에 없는 것을 가르치면 붉어진다
    /// - 출력의 키와 `kind` 값은 모두 스킬이 대거나 [`WIKI_UNTAUGHT`] 에 든다 — 명령이 더한 것을 스킬이 모르면 붉어진다
    /// - 홈 페이지(`wiki::HOMES`)도 스킬이 이름으로 댄다
    #[cfg(unix)]
    #[test]
    fn the_wiki_skill_teaches_the_keys_the_wiki_prints() {
        use crate::wiki::{DirTrouble, HOMES, TOO_LARGE};
        use std::collections::BTreeSet;
        let s = crate::scratch::Scratch::new("guide-wiki-keys");
        let away = crate::scratch::Scratch::new("guide-wiki-keys-away");
        let docs = s.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("README.md"), "# Home\n\n[a](a.md#a) [gone](gone.md) moai-ab12 moai-zz99\n").unwrap();
        std::fs::write(docs.join("a.md"), "# A\n\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> b\n").unwrap();
        std::fs::write(docs.join("big.md"), "x".repeat(TOO_LARGE as usize + 1)).unwrap();
        std::fs::write(docs.join("latin1.md"), b"caf\xe9\n").unwrap();
        std::fs::write(away.path().join("out.md"), "# Out\n").unwrap();
        std::os::unix::fs::symlink(away.path().join("out.md"), docs.join("out.md")).unwrap();
        std::fs::write(s.path().join("file"), "").unwrap();
        // 걷기가 건너뛰는 자리 — 디렉터리 링크와 UTF-8 이 아닌 이름. 못 연 디렉터리는 아래에서 갈래로 짓는다.
        std::os::unix::fs::symlink(away.path(), docs.join("away")).unwrap();
        let latin = <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(b"caf\xe9.md");
        std::fs::write(docs.join(latin), "# Name\n").unwrap();

        let lang = crate::i18n::Lang::En;
        let known = |id: &str| id == "moai-ab12";
        let read = crate::wiki::load(s.path(), "docs", "moai", &known);
        let mut printed = crate::cmd::wiki::listed_json(lang, "docs", &read).unwrap();
        let loaded = read.unwrap();
        printed.extend(crate::cmd::wiki::page_json(lang, &loaded, loaded.find("README").unwrap()).unwrap());
        for raw in ["missing", "/etc", "file"] {
            let read = crate::wiki::load(s.path(), raw, "moai", &known);
            assert!(read.is_err(), "{raw} 가 디렉터리 거절이 아니다");
            printed.extend(crate::cmd::wiki::listed_json(lang, raw, &read).unwrap());
        }
        // 디렉터리를 못 연 `failed` 는 픽스처로 못 세운다(권한은 root 에서 안 먹는다) — 갈래로 지어 넣는다.
        let failed = Err(DirTrouble::Failed("permission denied".into()));
        printed.extend(crate::cmd::wiki::listed_json(lang, "docs", &failed).unwrap());
        let unreadable = crate::wiki::Skipped {
            path: "docs/locked".into(),
            why: crate::wiki::Skip::Unreadable("permission denied".into()),
        };
        let walked = crate::wiki::Wiki { pages: Vec::new(), skipped: vec![unreadable] };
        printed.extend(crate::cmd::wiki::listed_json(lang, "docs", &Ok(walked)).unwrap());

        // 출력에 선 키와 `kind` 값을 모은다.
        fn gather(v: &serde_json::Value, keys: &mut BTreeSet<String>, kinds: &mut BTreeSet<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    for (k, v) in m {
                        keys.insert(k.clone());
                        if let ("kind", Some(kind)) = (k.as_str(), v.as_str()) {
                            kinds.insert(kind.to_string());
                        }
                        gather(v, keys, kinds);
                    }
                }
                serde_json::Value::Array(a) => a.iter().for_each(|v| gather(v, keys, kinds)),
                _ => {}
            }
        }
        let (mut keys, mut kinds) = (BTreeSet::new(), BTreeSet::new());
        for line in &printed {
            gather(&serde_json::from_str(line).unwrap(), &mut keys, &mut kinds);
        }
        let want_kinds =
            ["too_large", "refused", "failed", "no_dir", "outside", "not_a_dir", "dir_link", "not_utf8", "unreadable"];
        assert_eq!(kinds, want_kinds.iter().map(|k| k.to_string()).collect(), "픽스처가 갈래를 다 안 세웠다");

        for key in WIKI_KEYS {
            assert!(
                keys.contains(*key) || kinds.contains(*key),
                "스킬이 대는 `{key}` 를 `moai wiki --json` 이 안 낸다"
            );
        }
        for key in keys.iter().chain(&kinds) {
            assert!(
                WIKI_KEYS.contains(&key.as_str()) || WIKI_UNTAUGHT.contains(&key.as_str()),
                "`moai wiki --json` 이 내는 `{key}` 를 스킬이 안 댄다 — `guide::wiki` 에 적고 WIKI_KEYS 에 더하거나, \
                 일부러 안 대면 WIKI_UNTAUGHT 에 둔다"
            );
        }
        for home in HOMES {
            assert!(wiki().contains(&format!("`{home}.md`")), "스킬이 홈 페이지 `{home}.md` 를 안 댄다");
        }
        // 사람 화면이 대는 것만 가르친다 — 목록은 슬러그와 제목이고 경로는 `--json` 에만 선다.
        let drawn = crate::view::wiki::list(lang, "docs", &loaded).join("\n");
        assert!(!drawn.contains("docs/README.md"), "사람 목록에 경로가 섰다 — 스킬의 `moai wiki ls` 줄을 고친다");
        assert!(wiki().contains("moai wiki ls                           the pages: slug and title"));
    }

    #[test]
    fn the_wiki_skill_names_the_wiki_commands() {
        let wiki = wiki();
        for cmd in ["moai wiki ls", "moai wiki ls --json", "moai wiki show <slug>"] {
            assert!(wiki.contains(cmd), "위키 스킬이 `{cmd}` 를 안 가르친다");
        }
        for key in WIKI_KEYS {
            assert!(wiki.contains(&format!("`{key}`")), "위키 스킬이 `{key}` 키를 안 댄다");
        }
        // 브리프 7-4 와 같은 커밋 줄·같은 물음이다 — 따로 적으면 한쪽만 고쳐진다.
        assert!(wiki.contains(&indent(WIKI_COMMIT, "       ")), "위키 스킬의 커밋 줄이 브리프와 갈라졌다");
        assert!(wiki.contains(CHANGED_USE), "위키 스킬의 물음이 브리프와 갈라졌다");
        // **새 페이지를 git 에 먼저 알린다**(리뷰 moai-bl3x.vbw 1번). 경로를 준 `git commit` 은 git 이 모르는
        // 파일을 말없이 빼서, 새 페이지가 머지에 안 실린다 — `add` 를 걷으면 여기서 붉어진다.
        let (add, commit) = WIKI_COMMIT.split_once('\n').expect("커밋 줄이 한 줄로 줄었다");
        assert_eq!(add, "git add -- <wiki dir>", "새 페이지를 담는 `add` 가 커밋 앞에 없다");
        assert!(commit.starts_with("git commit ") && commit.ends_with(" -- <wiki dir>"), "{commit}");
        // 훑기는 이 브랜치가 닿는 태그만 보는 `git describe` 로 지난 릴리스를 찾지 않는다 — 릴리스를 다른
        // 가지(`main`)에 다는 저장소에서 한참 옛 태그를 짚는다(리뷰 moai-bl3x.vbw 2번).
        assert!(wiki.contains("git tag --sort=-creatordate"), "훑기가 지난 릴리스를 태그 날짜로 안 찾는다");
        // 훑기는 에픽 밖에서 닫힌 이슈도 본다 — 에픽만 돌면 이슈 하나로 끝난 쓰임의 변화가 빠진다.
        assert!(wiki.contains("moai show --type issue -e none --done"), "훑기가 에픽 밖 이슈를 안 본다");
        // 모든 저장소에 심긴다 — 에픽이 건드린 명령은 그 저장소의 것이지 moai 의 것이 아니다.
        assert!(!wiki.contains("moai <command> --help"), "위키 스킬이 남의 저장소에 moai 의 도움말을 읽힌다");
        // **생성 페이지를 알아보는 낱말은 이 저장소의 생성 페이지가 실제로 쓰는 낱말이다.** 스킬은 첫
        // 문단의 그 말로 "손대지 말 페이지" 를 가르는데, 생성기의 머리글이 바뀌면 스킬을 따른 창이
        // `docs/cli.md` 를 손으로 고친다 — 다음 생성이 그것을 말없이 덮는다.
        let cli = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/cli.md"))
            .expect("docs/cli.md 를 못 읽는다");
        let lead = cli.split("\n\n").find(|p| !p.starts_with('#')).expect("docs/cli.md 에 첫 문단이 없다");
        for word in ["Generated from", "Do not edit by hand"] {
            assert!(wiki.contains(word), "위키 스킬이 생성 페이지를 `{word}` 로 안 가른다");
            assert!(lead.contains(word), "docs/cli.md 의 첫 문단이 스킬이 찾는 `{word}` 를 잃었다 — {lead}");
        }
        // 아무것도 막지 않는다 — "하지 않는 것" 의 첫 줄이 게이트다.
        let not = &wiki[wiki.find("## What this skill does not do").expect("하지 않는 것 절이 없다")..];
        assert!(not.contains("**No gate.**"), "위키 스킬이 게이트를 안 걷는다");
    }

    /// **에픽 끝의 위키 걸음(7-4)은 짧게 두고 본문은 스킬로 보낸다**(moai-bl3x, 사용자 결정 2026-10-04).
    /// 브리프는 일꾼이 매 바퀴 통째로 읽는 글이라, 위키 쓰는 법을 여기 펴면 그 값을 일꾼마다 낸다.
    /// 7-3 의 CHANGELOG 와 같은 까닭으로 워크트리에서 머지 전에 커밋하고, 아무것도 막지 않는다.
    #[test]
    fn the_brief_sends_the_wiki_step_to_the_skill() {
        let brief = worker();
        let changelog = step_at(&brief, "7-3");
        let (at, end) = (step_at(&brief, "7-4"), step_at(&brief, "8"));
        assert!(changelog < at, "7-4 가 7-3 앞에 섰다");
        let step = &brief[at..end];
        let wiki = wiki();
        let name = wiki.lines().find_map(|l| l.strip_prefix("name: ")).expect("위키 스킬에 이름이 없다");
        for (piece, why) in [
            (format!("`{name}` skill"), "심는 스킬의 이름으로 안 보낸다"),
            ("`moai wiki ls`".to_string(), "위키가 있는지 무엇으로 아는지 안 적었다"),
            (CHANGED_USE.to_string(), "스킬과 같은 물음을 안 묻는다"),
            (indent(WIKI_COMMIT, "    "), "고친 페이지를 담을 커밋 줄이 없다"),
            ("in the worktree, before the merge".to_string(), "워크트리에서 머지 전에 쓰라는 말이 없다"),
            ("write nothing".to_string(), "쓰임이 안 바뀐 에픽은 안 쓴다는 말이 없다"),
            ("**Nothing checks this**".to_string(), "게이트가 아니라는 말이 없다"),
        ] {
            assert!(step.contains(&piece), "{why} — {piece}");
        }
        // 본문은 스킬에 있다 — 틀·훑기·하지 않는 것이 브리프로 새어 나오면 걸음이 자란다.
        // 빈 줄은 세지 않는다 — 마크다운의 코드 블록과 걸음을 가르는 자리다(moai-bkn4.9no).
        let grown = step.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(grown <= 8, "7-4 가 {grown}줄로 자랐다 — 본문은 스킬에 둔다\n{step}");
        // 닫기보다 앞이다 — 멤버가 닫히면 그 창은 규칙 2 로 저장소를 못 고친다.
        assert!(end < step_at(&brief, "10"), "위키를 닫은 뒤에 본다");
        // 보고(12)가 고친 페이지를 댄다 — 감독은 위키를 따로 안 본다.
        assert!(
            brief[step_at(&brief, "12")..].contains("the wiki pages 7-4 changed — or that it changed none"),
            "보고가 7-4 가 고친 위키 페이지를 안 댄다"
        );
    }

    /// **혼자 펼치는 세션도 마일스톤을 달고 본문을 옮긴다**(moai-fww7).
    ///
    /// 마일스톤 걸음은 감독 길에만 있었다(moai-6qgz) — 감독 없이 혼자 펼치는 세션이 읽는 글에는
    /// 없어, 도는 마일스톤 밖에 에픽이 그대로 서고 그 멤버를 집는 순간 "안의 것이 먼저다" 가
    /// 깨진다. 다는 줄은 일꾼 브리프 1 과 같은 [`MILESTONE_ATTACH`] 에서 나오므로, 한쪽만
    /// 고치면 여기서 붉어진다.
    ///
    /// 본문 걸음은 어느 글에도 없었다. 2026-09-22 에 한 번에 펼친 에픽 셋이 모두 0자로 섰고,
    /// `moai show <에픽>` 이 왜 이것들이 한 묶음인지를 못 냈다.
    ///
    /// **2026-09-23 부터 그 둘은 도구가 데려간다**(moai-07v1) — 손으로 치던 두 걸음이 빠졌으니
    /// 이 글도 무엇이 저절로 서고 무엇이 손에 남는지를 말해야 한다. 그 줄은 여전히
    /// [`MILESTONE_ATTACH`] 하나에서 나온다.
    ///
    /// **손에 남는 자리를 "마일스톤을 안 들었을 때" 로 적어 둔 것은 틀렸다**(리뷰). `promote` 는
    /// 그 생각이 **선 자리**를 그것이 무엇이든 데려가므로, 이미 끝난 릴리스에 담아 둔 생각을
    /// 펼치면 그 id 가 그대로 새 에픽에 서고 `ready` 가 그 에픽을 곧바로 감춘다 — 그런데 글은
    /// "든 것이 있으니 할 일 없다" 로 읽혔다. 가르는 것은 **도는 것과 같은가** 다.
    ///
    /// **데려가는 값은 적힌 필드가 아니다**(2026-09-23 사용자 결정) — `moai show --milestone` 이
    /// 그 생각을 내주는 자리고, 죽은 릴리스면 한 줄 알린다. 글이 "제 필드" 라고 말하면 에픽에
    /// 담긴 생각을 펼친 쪽이 왜 릴리스가 따라왔는지를 못 읽는다.
    ///
    /// **2026-09-25 에 손에 남는 일이 뒤집혔다**(moai-pdlp, 리뷰 moai-pdlp.4ox 1번). 도는 릴리스를
    /// 제 손으로 다는 것이 그때까지 이 절의 답이었는데, 사용자가 들이는 것을 사람만 하기로 정했다.
    /// 그래서 남는 일은 둘로 갈린다 — **딴 릴리스면 그대로 두고 말하고**, 죽은 릴리스는 스스로
    /// 걷는다(아무도 여기서 고른 값이 아니고, 그대로 두면 새 에픽을 감춘다). 이 절이 안 바뀌면
    /// AGENTS 블록 한 파일 안에서 앞 절이 뒤 절을 뒤집고, 읽는 차례상 앞 절이 이긴다.
    #[test]
    fn unfolding_alone_carries_the_body_and_the_release_it_stood_in() {
        let backlog = backlog();
        for (piece, why) in [
            (MILESTONE_ATTACH, "사람이 들일 때 치는 줄이 없다"),
            (MILESTONE_FROM, "헛 id 를 못 가르니 어디서 베끼는지 대야 한다"),
            ("`dangling_milestone`", "틀린 id 가 언제 드러나는지 안 적었다"),
            ("milestone and body go onto the epic by themselves", "도구가 데려간다는 말이 없다"),
            ("Not onto every issue", "이슈마다 베끼는 것으로 읽힌다"),
            ("`-e <epic>`) carries neither", "선 에픽에 펼칠 때는 안 데려간다는 말이 없다"),
            (
                "not the release that is running, leave it where it stands",
                "든 것이 딴 릴리스일 때 그대로 두라는 말이 없다",
            ),
            ("**A dead one you do clear yourself**", "죽은 릴리스를 스스로 걷는다는 말이 없다"),
            ("`moai show --milestone` stands the backlog", "데려가는 값이 적힌 필드로 읽힌다"),
            ("A dead release is said", "죽은 릴리스를 알린다는 말이 없다"),
        ] {
            assert!(backlog.contains(piece), "{why} — {piece}");
        }
        // **심는 두 표면이 같은 글을 받는다** — 한쪽만 고치면 그 글을 읽은 세션만 걸음을 안다.
        let (agents, reference) = (agents(), reference());
        assert!(agents.contains(&backlog), "AGENTS 블록의 backlog 절이 갈라졌다");
        assert!(reference.contains(&backlog), "참고 문서의 backlog 절이 갈라졌다");
    }

    /// **일꾼이 창을 비워도 되는 때를 알린다**(moai-gu5g, 2026-09-15 사용자 결정).
    ///
    /// 맥락은 대화가 아니라 트래커에 산다 — 이슈 본문·노트·리뷰 원문·커밋 메시지. 머지가 들고
    /// 보고가 나간 자리에서는 지워도 잃을 것이 없다. **반대도 함께 말한다**: 리뷰가 도는 중·
    /// 충돌을 푸는 중·사람의 답을 기다리는 중에 지우면 아직 트래커에 안 옮긴 것이 사라진다.
    #[test]
    fn the_brief_says_when_the_pane_can_be_cleared() {
        let brief = worker();
        // **보고 뒤다** — 보고 전에 지우면 보고에 담을 것이 대화에만 있던 채로 사라진다.
        let report = brief.find("`SendMessage(to: <supervisor>, message: …)`").expect("보고 걸음이 없다");
        let at = report + brief[report..].find("(`/clear`)").expect("창을 비워도 되는 때를 안 알린다");
        // **남긴 줄은 담은 뒤에 비운다.** 이어받을 줄은 10 의 트래커 커밋 뒤에 적으므로, 담지
        // 않으면 공유 루트의 `.moai` 가 더러운 채로 남아 남의 커밋에 쓸려 들어간다. 그 줄(11)은 보고 앞이다 —
        // 보고가 마지막이라 감독은 보고를 받으면 노트가 섰다고 읽는다.
        let note = &brief[step_at(&brief, "11")..step_at(&brief, "12")];
        assert!(note.contains("'Next: …'") && note.contains("a commit with a path"), "보고 앞에 남긴 줄을 안 담는다");
        // **까닭과 반대를 갈라 찾는다** — 둘이 서로의 낱말(`트래커`·`리뷰`)을 품어, 한 덩어리로
        // 찾던 판은 어느 한쪽을 지워도 초록이었다.
        let (reason, against) =
            brief[report..at].split_once("Say the opposite").expect("지우지 말 때를 같은 줄에서 안 말한다");
        assert!(reason.contains("lives in the tracker"), "왜 지워도 되는지가 없다");
        for (piece, missing) in [
            ("a review is running", "리뷰가 도는 중에는 지우지 말라는 말이 없다"),
            ("conflict", "머지 충돌을 푸는 중을 안 가린다"),
            ("waiting on a person", "답을 기다리는 중을 안 가린다"),
            // 감독은 보고를 확인하면 다음 메시지를 보낸다 — 그 뒤의 비우기는 그 메시지를 지운다.
            ("next message", "감독의 다음 메시지가 온 뒤에는 지우지 말라는 말이 없다"),
        ] {
            assert!(against.contains(piece), "{missing}");
        }
        // **보고 뒤에는 턴을 끝낸다**(moai-obxm) — 기다리는 고리가 없고, 다음 메시지가 창을 깨운다. 비우기는 사람의
        // 몫이라 감독이 창을 비운다는 말도 없다.
        assert!(brief[at..].contains("Then end the turn"), "보고 뒤에 턴을 끝내라는 말이 없다");
        assert!(!brief.contains("wait again") && !brief.contains("After the report"), "일꾼이 아직 기다림 고리를 돈다");
        assert!(!brief.contains("clear this window itself"), "감독이 창을 비운다고 가르친다");
    }

    /// **tmux 시험은 떼어 낸 서버에서만 가르친다**(2026-09-18 사용자 규칙). 스킬이 맨
    /// `tmux new-session -d` 를 가르치던 날, 그 길을 따른 리뷰 서브에이전트가 맨 `kill-server` 로
    /// 사람의 tmux 서버를 통째로 죽였다 — tmux 안에서는 `$TMUX` 가 `TMUX_TMPDIR` 를 이긴다.
    /// 일꾼이 받는 글에 서야 한다: 시험을 실제로 치는 것은 일꾼과 그 리뷰 서브에이전트다.
    ///
    /// **감독의 tmux 는 사람의 서버다**(2026-10-10 사용자 결정, moai-u99i) — 일꾼의 칸을 비우고 붙이고 여는 것이 일이라
    /// 떼어 낸 서버로는 할 수 없다. 그래서 그 명령은 감독 글이 아니라 `moai-tmux` 에만 서고, 감독 글은 여전히 기본 서버의
    /// tmux 명령을 하나도 안 든다. `moai-tmux` 에는 제 금이 선다 — 아래 `the_tmux_skill_touches_only_named_panes`.
    #[test]
    fn tmux_tests_are_taught_on_a_separate_server() {
        let (supervise, brief) = (supervise(), worker());
        // 감독 스킬은 걸음을 안 품는다(moai-snyk) — 두 글을 따로 잰다. 품던 판은 통째로 재면 브리프의 같은
        // 줄이 감독 쪽에서 빠진 자리를 메웠다.
        assert!(!supervise.contains(&brief), "감독 스킬이 일꾼의 걸음을 품는다");
        // 감독 글은 tmux 명령을 안 든다 — 칸을 만지는 걸음은 `moai-tmux` 에 있다(moai-u99i). 시험하는 것은 일꾼이라 규칙은 일꾼 글에 선다.
        assert!(brief.contains("env -u TMUX tmux -L"), "일꾼 걸음: 떼어 낸 서버로 시험하라는 말이 없다");
        assert!(brief.contains("without `-L`/`-S`"), "일꾼 걸음: 맨 kill-server 를 막는 말이 없다");
        assert!(brief.contains("TMUX_TMPDIR"), "일꾼 걸음: TMUX_TMPDIR 로 안 갇힌다는 말이 없다");
        // 속에서 `tmux` 를 부르는 스크립트에는 손으로 `-L` 을 못 준다 — 그것을 시험하는
        // 일꾼에게도 가둘 길이 있어야 하고, 그 감싸개가 PATH 로 제 자신을 부르면 끝나지 않는다.
        assert!(
            brief.contains("wrapper") && brief.contains("absolute path"),
            "일꾼 걸음: 스크립트를 떼어 낸 서버에 돌릴 길이 없다"
        );
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
        for line in supervise.lines().chain(worker().lines().collect::<Vec<_>>()) {
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
        // 집을 때도 적게 넓히지 않고, 통계가 그 줄을 가를 수 있게 까닭에 표시만 한다. 그 걸음은 일꾼
        // 글의 한 절이고, 감독의 메시지는 첫 줄로 그 절을 이름 짓는다(moai-snyk).
        let (supervise, work) = (supervise(), worker());
        let title = "Carrying on stalled work";
        assert!(supervise.contains(&format!("from \"{title}\"")), "거둔 일의 메시지가 일꾼 글의 그 절을 안 댄다");
        let at = work.find(&format!("## {title}")).expect("거둔 일의 절이 없다");
        let end = work[at + 3..].find("\n## ").map_or(work.len(), |n| at + 3 + n);
        assert!(
            work[at..end].contains("the previous\n  session's share is unknown"),
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
        let brief = worker();
        let work = &brief;
        // **올리는 길이 명령으로 서고, 그 명령을 칠 수 있는 자에게 간다.** "올린다" 만 적으면
        // 일꾼이 무엇을 쳐야 하는지 모른다. 그런데 `/model` 은 사람만 친다 — 에이전트는 붙박이
        // 명령을 못 불러, 제 손으로 치라고 하면 올렸다고 믿고 9-1 에 안 돈 모델을 적는다.
        let raise = work
            .find("the model that pairs with the difficulty you just measured")
            .expect("일꾼이 모델을 올릴 길이 없다");
        // **한 칸씩이 아니다** — 두 칸 어긋난 제안이 가운데 모델에 멈추면 쓰기 경로를 싼 모델이 한다.
        assert!(!work.contains("raise it one step"), "두 칸 어긋난 제안이 한 칸만 오른다");
        // 알리는 글은 **글자로 자른다** — 바이트로 자르면 한글 한가운데서 끊겨, 실패를
        // 알리려던 자리가 제가 먼저 죽는다.
        let shown = work[raise..].chars().take(40).collect::<String>();
        assert!(work[..raise].contains("`/model`"), "무엇으로 올리는지가 없다 — {shown}");
        assert!(
            work[..raise].contains("ask the person watching the window"),
            "`/model` 을 사람에게 청하라는 말이 없다 — {shown}"
        );
        assert!(work.contains(&indent(&model_rule(), "  ")), "메시지의 모델 줄을 읽는 법이 조각에서 안 나온다");
        // **닫는 자리에 선다** — 워크트리를 지운 뒤(맨 `moai` 가 루트를 읽는다), 멤버를 닫기
        // 전. 자리를 바이트 거리로 재던 판은 9-1 이 9 위로 올라가도 초록이었다.
        let removed = brief.find("git worktree remove").expect("워크트리를 지우는 걸음이 없다");
        let note = brief.find("model:").expect("일한 모델을 남기는 걸음이 없다");
        let done = brief.find("moai mv <member> done").expect("멤버를 닫는 걸음이 없다");
        assert!(removed < note && note < done, "모델 노트가 워크트리를 지운 뒤·멤버를 닫기 전이 아니다");
        // **남기는 것은 실제로 돈 모델이다.** 메시지의 제안은 감독이 채워 보내 이 줄에도 박혀
        // 오므로, 다르면 고쳐 적으라는 말이 같은 걸음에 서야 한다 — 없으면 제안이 일한 것으로
        // 남는다. 그 까닭이 남을 자리가 이 노트다.
        let step = step_at(&brief, "9-1");
        assert!(brief[step..note].contains("actually ran"), "제안이 아니라 실제로 돈 모델을 남기라는 말이 없다");
        let line = brief[note..].lines().next().unwrap_or_default();
        assert!(line.contains("<why>"), "노트에 까닭 자리가 없다 — {line}");
        // `moai note` 로 남긴다. 9-1 은 8 에서 루트로 돌아오고 9 에서 워크트리를 지운
        // 뒤라 맨 `moai` 가 맞다 — 4-1 의 `-C <root>` 는 워크트리 안에서만 드는 손잡이다.
        let line = brief[..note].lines().last().unwrap_or_default();
        assert!(line.trim().starts_with("moai note "), "노트가 아닌 것으로 남긴다 — {line}");
    }

    /// **가르치는 꼴이 `model::parse_work` 가 읽는 꼴이다**(moai-jo8d). 9-1 이 회사·토큰 없는 옛
    /// 꼴을 가르치던 동안 감독 아래의 일은 모두 빈 칸을 적어 `work` 통계가 늘 비었다 — 글과
    /// 파서가 따로 서면 다시 갈린다. 자리를 채워 파서에 넣어, 회사·토큰까지 값이 되는지 본다.
    /// 토큰을 모를 때 `tokens=<수>` 를 통째로 빼도 읽혀야 한다 — 0 을 적게 두면 통계가 "공짜" 로 읽는다.
    #[test]
    fn the_taught_model_line_is_the_one_the_parser_reads() {
        let texts = [("브리프 9-1", worker()), ("AGENTS 블록", agents()), ("스킬 참고 문서", reference())];
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
        let texts = [
            ("AGENTS 블록", agents()),
            ("스킬", skill()),
            ("참고 문서", reference()),
            ("감독", supervise()),
            ("일꾼", worker()),
        ];
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
    /// 아무것도 안 가리키고, 고친 뒤에는 거짓이 된다. 위키 스킬도 모든 저장소에 심긴다.
    ///
    /// 셋째 스킬의 이름 `moai-wiki` 는 id 의 꼴(`moai-` 뒤 네 글자)과 겹친다 — 스킬 이름은 `skill::ever_planted` 로
    /// 거른다. 위키가 거르는 그 자(걷은 `moai-work` 까지 든다, moai-six5.1xz)이고, 스킬 머리의 이름과 갈리면
    /// `the_frontmatter_opens_the_skill` 이 붉어진다.
    #[test]
    fn the_supervisor_names_no_issue_of_this_repo() {
        for (whose, text) in [("감독 스킬", supervise()), ("위키 스킬", wiki()), ("일꾼 글", worker())] {
            let ids: Vec<&str> = text
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .filter(|w| !crate::skill::ever_planted(w))
                .filter(|w| {
                    w.strip_prefix("moai-").is_some_and(|rest| {
                        rest.len() == 4 && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    })
                })
                .collect();
            assert!(ids.is_empty(), "{whose} 에 이 저장소의 이슈 id 가 섰다 — {ids:?}");
        }
    }
    #[test]
    fn the_supervised_worker_settles_the_three_old_questions() {
        // 2026-09-18 사용자 결정 셋. (1) 보낸 일은 늘 워크트리 (2) 일꾼은 병합 전에 옆 세션을 찾아
        // 알리지 않는다 — 찾을 길이 없는 말은 지킬 수 없다 (3) 펼칠 안을 사람에게 다시 묻지 않는다.
        let (supervise, brief) = (supervise(), worker());
        let head = &supervise[..supervise.find("## One round").expect("한 바퀴가 없다")];
        assert!(head.contains("**Work you send out is always done in a worktree**"), "워크트리 전제가 머리에 없다");
        assert!(!supervise.contains("tell the other worker first"), "일꾼이 찾을 길 없는 옆 세션에 알리라고 한다");
        assert!(!brief.contains("tell the other worker first"), "일꾼이 찾을 길 없는 옆 세션에 알리라고 한다");
        let one = &brief[step_at(&brief, "1")..step_at(&brief, "2")];
        assert!(one.contains("not to show a person"), "맡긴 backlog 를 펼칠 때 사람에게 또 묻는다");
    }

    #[test]
    fn a_file_held_next_door_becomes_a_member_not_an_edit() {
        // **에픽 도중 새로 필요해진 파일을 옆이 쥐었으면 멤버로 남기고 알린다**(2026-09-18 사용자
        // 결정). 감독이 보내기 전에 파일을 재도 도중에 새로 필요해진 파일은 못 잰다 — 그런 일이
        // backlog 로 밖에 나가 에픽이 목적을 못 이룬 채 닫힌 적이 있다. 일정 문제가 범위 결정으로 위장한다.
        let (supervise, brief) = (supervise(), worker());
        assert!(message().contains(BESIDE), "메시지에 옆에서 쥔 파일의 줄이 없다");
        assert!(BESIDE.starts_with("Work running alongside: <other work>"), "옆 일 줄의 머리가 바뀌었다 — {BESIDE}");
        let list = slot_list(&supervise);
        assert!(list.contains("`<other work>`"), "감독이 옆 일을 안 채운다 — {list}");
        let step = &brief[step_at(&brief, "4-3")..step_at(&brief, "5")];
        assert!(step.contains("-e <epic>") && step.contains("Do not defer it"), "옆이 쥔 일이 멤버로 안 남는다");
        let report = step_at(&brief, "12");
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
        let (noted, closed) = (step_at(&brief, "9-1"), step_at(&brief, "10"));
        assert!(
            brief[noted..closed].contains("first column by 7-1 and 4-3"),
            "9-1 이 4-3 의 멤버에 일한 모델을 남긴다"
        );
        assert!(brief[closed..report].contains("first column by 7-1 and 4-3"), "10 이 4-3 의 멤버를 닫는다");
    }

    #[test]
    fn a_test_claude_on_a_detached_server_is_no_worker() {
        // **떼어 낸 tmux 서버의 시험용 에이전트는 일꾼이 아니다**(2026-09-18 사용자 결정). 루트에서 띄우면
        // `ListAgents` 에 이 저장소의 놀고 있는 세션으로 선다. 감독은 이 저장소의 idle 세션 전부를 일꾼으로 읽으니
        // (moai-obxm) 거르는 것은 이름이다 — 세션 이름은 연 디렉터리에서 오니 시험하는 쪽에 루트 밖에서 띄우라고 하고, 감독은 이름이 저장소의 것이 아닌 줄을 안 센다.
        // 둘 중 하나만 서면 다른 쪽이 샌다.
        let (supervise, brief) = (supervise(), worker());
        let two = supervise.find("**2. Find a worker.**").expect("2 가 없다");
        let three = supervise.find("**2-1. ").expect("2-1 이 없다");
        let step = &supervise[two..three];
        assert!(step.contains("Call `ListAgents` once"), "감독이 일꾼을 ListAgents 로 안 찾는다");
        assert!(
            step.contains("belongs to this repository") && step.contains("is not you"),
            "감독이 일꾼을 자리로 안 거른다"
        );
        // `ListAgents` 는 서브에이전트도 늘어놓는다 — 자리만 보면 감독 제 서브에이전트(같은 저장소, idle)가 일꾼으로 서고,
        // 그것에 보낸 메시지는 그 서브에이전트를 되살린다. `ListAgents` 는 자리를 안 보이니(2026-10-07 실제 목록) 이름의 머리로 거른다.
        assert!(step.contains("Not a subagent"), "감독이 서브에이전트를 일꾼으로 센다");
        assert!(
            step.contains("whose name does not start with the root's slug"),
            "이름이 이 저장소의 것이 아닌 줄을 일꾼으로 센다"
        );
        // 이름은 연 디렉터리를 슬러그로 지은 것이다(moai-ybns.451.tf3) — 디렉터리 이름 그대로 견주면 `tvshop_updater`
        // 의 `tvshop-updater-ca` 를 놓친다. 머리가 맞아도 `api` 는 `api-gateway-1c` 를, `moai` 는
        // `moai-web` 이라는 다른 클론의 세션을 잡으니, 머리는 후보일 뿐이고 일꾼이 `Root:` 로 제 저장소인지 확인해 거절한다.
        assert!(step.contains("`tvshop-updater-ca` for `tvshop_updater`"), "감독이 이름을 슬러그로 견주지 않는다");
        assert!(
            step.contains("**A name that does start so is still only a candidate**"),
            "머리가 맞으면 남의 저장소 세션도 일꾼으로 센다"
        );
        assert!(brief.contains("**If `Root:` is not this window's repository**"), "남의 저장소 세션이 받은 일을 한다");
        // 두 거절은 루트로 옮겨 가는 걸음보다 **앞에** 선다(리뷰 moai-iu73.zci) — 뒤에 두면 차례대로 읽은 남의 저장소
        // 세션이 먼저 `cd <Root>` 하고, 그 뒤에는 그 저장소에 서 있어 거절이 영영 안 걸린다.
        let refuse = brief.find("**If this window runs `moai-supervise` itself**").expect("감독의 거절이 없다");
        let go = brief.find("go there first").expect("루트로 옮기는 걸음이 없다");
        assert!(refuse < go, "일꾼이 거절을 묻기 전에 루트로 옮겨 간다");
        assert!(
            brief[..go].contains("**If `Root:` is not this window's repository**"),
            "남의 저장소 세션이 먼저 옮겨 간다"
        );
        // 보고를 기다리는 감독도 idle 이라 일꾼의 자에 다 맞는다(moai-ybns.451.qgx) — 둘째 감독이 그것에 backlog 를
        // 보내고, 둘이 따로 적는 "보낸 일" 이 한 backlog·한 일꾼에 일을 둘 준다. 저장소마다 감독은 하나고, 받은 감독은 거절한다.
        let before = &supervise[..supervise.find("## One round").expect("한 바퀴가 없다")];
        assert!(before.contains("**One supervisor per repository.**"), "감독이 저장소마다 하나라는 말이 없다");
        assert!(step.contains("not a supervisor"), "감독이 다른 감독을 일꾼으로 센다");
        // 받은 감독의 거절은 감독 제 글에도 선다 — 일꾼 걸음에만 두면 그 파일을 안 여는 감독은 그 말을 못 읽는다.
        assert!(before.contains("refuse too:**"), "일을 받은 감독이 제 글에서 거절을 못 읽는다");
        assert!(brief.contains("**If this window runs `moai-supervise` itself**"), "일을 받은 감독이 일꾼 걸음을 탄다");
        // 일꾼은 감독이 이름 대는 걸음 파일의 절대 경로를 읽는다 — 다른 기계의 세션은 그 파일을 못 연다(moai-fim6).
        assert!(step.contains("runs on this machine"), "다른 기계의 세션을 일꾼으로 센다");
        assert!(
            step.contains("`interactive`") && step.contains("not a `bg` session"),
            "사람이 연 세션만 거르지 않는다"
        );
        assert!(step.contains("**A test agent is no worker.**"), "시험용 에이전트를 어떻게 할지 없다");
        assert!(brief.contains("keep its cwd outside the root"), "시험용 에이전트를 루트에서 띄운다");
        // Claude Code 의 속 파일은 문서에 없고 Claude 세션만 든다 — 감독이 다시 그것을 읽으면 다른 벤더의
        // 일꾼이 목록에서 빠진다.
        assert!(
            !supervise.contains(".claude/sessions") && !supervise.contains("sessions/*.json"),
            "감독이 Claude 의 세션 파일을 읽는다"
        );
    }

    #[test]
    fn the_worker_steps_into_the_subproject_of_a_monorepo() {
        // **모노레포의 하위가 루트면 워크트리 안의 같은 하위에서 일한다**(2026-09-18 사용자 결정).
        // 워크트리 꼭대기에 선 일꾼은 `moai` 가 공유 루트의 `.moai` 를 찾아 쓰고, 훅 규칙 2 는
        // `.worktrees/` 아래라 편집을 안 센다(moai-5s9l — 옛 자리 `.claude/` 도 같다).
        let (supervise, brief) = (supervise(), worker());
        let work = &brief;
        assert!(supervise.contains("print(\"subdir\", os.path.relpath(here, top))"), "감독이 하위 경로를 안 낸다");
        assert!(message().contains("Subdir: <subdir>"), "메시지에 하위 경로 자리가 없다");
        let three = &brief[step_at(&brief, "3")..step_at(&brief, "4")];
        assert!(three.contains(SUBDIR), "일꾼이 워크트리 안의 하위로 안 들어간다");
        // 거둔 일은 3 을 안 지나 4-1 부터 잇는다 — 그 절에도 같은 줄이 서야 이어받은 일꾼이 꼭대기에 안 선다.
        let stalled = &work[work.find("## Carrying on stalled work").expect("거둔 일의 절이 없다")..];
        assert!(stalled.contains(SUBDIR), "거둔 일의 일꾼이 워크트리 꼭대기에 선다");
        // 글머리(`- `) 안의 코드 블록이다 — 빈 줄 뒤에 글 칸(2)에서 네 칸 더(moai-bkn4.9no). 빈 줄 없이 들여 쓰면 GitHub 에서
        // 글머리 문단 끝에 `cd` 가 붙는다(리뷰 moai-bkn4.c3d — 브리프만 재는 시험이 이 자리를 못 본다).
        assert!(stalled.contains(&format!("\n\n      {SUBDIR}\n")), "거둔 일의 `cd <subdir>` 가 코드 블록으로 안 선다");
    }

    /// **걸음은 마크다운 목록 규칙을 따른다**(moai-bkn4.9no, 리뷰 moai-snyk.nic). 에이전트는 날 글을 읽어 지장이 없지만,
    /// GitHub 에서는 걸음마다 명령 줄과 잔걸음까지 한 문단으로 뭉쳤다. 다섯을 맨다.
    ///
    /// - 명령 줄은 코드 블록이다 — 빈 줄 뒤에, 그 걸음의 글 칸에서 네 칸 더(`1.`~`9.` 는 7칸, `10.`~`12.` 는 8칸).
    ///   들여 쓴 코드는 문단을 못 끊어, 빈 줄 없이 들여 쓴 줄은 위 문단에 붙는다
    /// - `4-1.` 같은 잔걸음은 빈 줄 뒤에 선다 — 목록 표시가 아니라서 빈 줄이 없으면 위 문단에 붙는다. 목록 밖의 맨 바깥
    ///   문단이라 그 명령 줄은 네 칸이다 — 일곱 칸이면 코드 블록 안에 세 칸이 남는다(리뷰 moai-bkn4.c3d)
    /// - 잔걸음 뒤의 큰 걸음도 빈 줄 뒤다 — `1.` 이 아닌 번호는 문단을 끊고 목록을 열지 못한다
    /// - 이음은 그 걸음의 글 칸에 선다 — `10.` 은 표시가 네 칸이라, 세 칸 이음은 코드 블록 뒤에서 목록 밖으로 떨어진다
    /// - 글 칸 세 칸 더에 서는 것은 걸음 안의 번호 목록(7 의 다섯 자리)의 이음뿐이고, 그 목록 뒤의 문단은 빈 줄 뒤다 —
    ///   빈 줄이 없으면 목록의 마지막 항목에 붙는다. 목록 밖에서 세 칸 더에 선 줄은 명령 줄을 잘못 놓은 것이다
    ///   (`10.`~`12.` 의 일곱 칸은 코드가 아니라 문단이다, 리뷰 moai-bkn4.c3d)
    #[test]
    fn the_worker_steps_follow_the_markdown_list_rules() {
        let worker = worker();
        let brief = numbered_steps(&worker);
        let lead = |l: &str| l.len() - l.trim_start().len();
        let blank = |l: &str| l.trim().is_empty();
        let numbered = |l: &str| {
            l.trim_start().split_once(". ").is_some_and(|(m, _)| !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit()))
        };
        // 글 칸과 코드 칸, 바로 앞 걸음이 잔걸음인가, 걸음 안의 번호 목록 속인가, 마지막에 읽은 걸음 표시.
        let (mut text, mut code, mut after_sub, mut nested, mut last) = (3, 7, false, false, "");
        for (before, l) in std::iter::once("").chain(brief.lines()).zip(brief.lines()) {
            if blank(l) {
                continue;
            }
            let n = lead(l);
            if n == 0 {
                let mark = l.split_once(". ").map(|(m, _)| m).unwrap_or_default();
                assert!(
                    !mark.is_empty() && mark.chars().all(|c| c.is_ascii_digit() || c == '-'),
                    "걸음 표시가 아닌 줄이 맨 앞에 서서 위 문단에 붙는다: {l}"
                );
                let sub = mark.contains('-');
                if sub || after_sub {
                    assert!(blank(before), "{mark} 앞에 빈 줄이 없어 위 문단에 붙는다");
                }
                (text, code) = if sub { (3, 4) } else { (mark.len() + 2, mark.len() + 6) };
                (after_sub, nested, last) = (sub, false, mark);
                continue;
            }
            if n >= code {
                assert!(blank(before) || lead(before) == code, "명령 줄 앞에 빈 줄이 없어 문단에 붙는다: {l}");
                assert_eq!(n, code, "명령 줄이 그 걸음의 코드 칸({code})에 안 섰다: {l}");
                nested = false;
                continue;
            }
            if n == text + 3 {
                assert!(nested, "번호 목록 밖에서 글 칸 세 칸 더에 섰다 — 명령 줄이면 코드 칸({code})이다: {l}");
                continue;
            }
            assert_eq!(n, text, "명령 줄도 이음도 아닌 칸에 섰다: {l}");
            let item = numbered(l);
            if nested && !item {
                assert!(blank(before), "번호 목록 뒤의 문단 앞에 빈 줄이 없어 목록의 마지막 항목에 붙는다: {l}");
            }
            nested = item;
        }
        assert_eq!(last, "12", "마지막 걸음을 못 읽었다 — `12.` 가 걸음 표시로 안 섰다");
    }

    /// **마일스톤 일은 마일스톤 가지에서 뜨고 거기로 병합한다**(2026-10-08 사용자 결정, moai-nvju). 감독은 루트의
    /// 가지와 일마다의 바탕 가지를 따로 싣고, 병합 확인도 그 바탕 가지를 본다 — 루트의 가지를 보면 릴리스 전의 마일스톤
    /// 병합이 없는 것으로 읽힌다. 일꾼은 그 가지의 워크트리를 세우고, 병합·대조·`branch -d` 를 루트에서 `-C` 로 친다.
    #[test]
    fn milestone_work_merges_on_the_milestone_branch() {
        let (brief, supervise, message) = (worker(), supervise(), message());
        assert!(message.contains("Root branch: <root branch>"), "메시지에 루트의 가지 자리가 없다");
        for piece in [
            "`milestone/<milestone id>`",
            "`.worktrees/milestone-<milestone id>`",
            "`<base branch>` here is the one you sent with that work",
            "even while nothing runs yet and `<milestone>` in 3 says `none`",
        ] {
            assert!(supervise.contains(piece), "감독 글에 마일스톤 가지가 빠졌다 — {piece}");
        }
        assert!(!supervise.contains("Read the base branch once"), "감독이 아직 바탕 가지를 바퀴마다 한 번만 읽는다");
        let section = brief.find("## The milestone branch").expect("일꾼 글에 마일스톤 가지 절이 없다");
        let steps = brief.find("## The steps").expect("일꾼 글에 걸음 절이 없다");
        assert!(section < steps, "마일스톤 가지 절이 걸음 뒤에 섰다");
        for piece in [
            "git worktree add -b milestone/<milestone id> .worktrees/milestone-<milestone id> <root branch>",
            "git worktree add .worktrees/milestone-<milestone id> milestone/<milestone id>",
            "git -C .worktrees/milestone-<milestone id> symbolic-ref -q HEAD",
        ] {
            assert!(brief[section..steps].contains(piece), "마일스톤 가지 절에 빠졌다 — {piece}");
        }
        let merge = step_at(&brief, "8");
        for piece in [
            "git -C .worktrees/milestone-<milestone id> merge --no-ff worktree-<epic>",
            "git -C .worktrees/milestone-<milestone id> branch -d worktree-<epic>",
        ] {
            assert!(brief[merge..].contains(piece), "병합 걸음에 마일스톤 가지가 빠졌다 — {piece}");
        }
        // 루트의 대조는 루트의 가지와 견준다 — 마일스톤 가지는 루트가 설 수 없는 가지다.
        assert!(BRANCH_CHECK.contains("refs/heads/<root branch>"), "루트를 바탕 가지와 견준다");
        assert!(
            brief.contains("**Nor is the branch rebased or squashed when it merges**"),
            "rebase·squash 금지가 없다"
        );
    }

    /// **일꾼에게 싣는 글은 가지 이름을 박지 않는다.** `main` 을 박으면 `develop`·`trunk`
    /// 저장소에서 워크트리 뜨기부터 실패한다. 감독이 읽어 채울 자리와 읽는 한 줄이 선다.
    ///
    /// **낱말로 가른다.** 글자로 찾으면 `master`·`develop` 을 박은 글은 지나가고 `remain` 은
    /// 막힌다. 감독이 제 손으로 치는 병합 확인도 같은 자리를 쓰는지 본다.
    #[test]
    fn the_brief_names_no_branch() {
        let (brief, supervise) = (worker(), supervise());
        let work = &brief;
        for (whose, text) in [("일꾼 글", work.as_str()), ("메시지", message().as_str())] {
            let named: Vec<&str> = text
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .filter(|w| ["main", "master", "develop", "trunk"].contains(w))
                .collect();
            assert!(named.is_empty(), "{whose} 가 가지 이름을 박았다 — {named:?}");
        }
        assert!(brief.contains("<base branch>"), "일꾼 글에 본 가지 자리가 없다");
        assert!(message().contains("Base branch: <base branch>"), "메시지에 본 가지 자리가 없다");
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
        assert!(
            brief[step_at(&brief, "8")..merge].contains("symbolic-ref -q HEAD"),
            "병합 전에 루트의 가지를 대조하지 않는다"
        );
        // 커밋 전의 대조는 걸음 앞에 한 번 선다 — 새 일과 거둔 일이 같은 글을 읽는다(moai-snyk).
        let steps = work.find("## The steps").expect("일꾼 글에 걸음 절이 없다");
        assert!(work[..steps].contains(BRANCH_CHECK), "걸음 앞에 루트 대조가 없다");
        assert!(work[..steps].contains(GIT_SHAPES), "걸음 앞에 git 꼴이 없다");
        // **대조를 루트에서 한다**(moai-rgp9) — 워크트리 안에서 `-C <루트>` 로 묻는 꼴은 격리
        // 가드가 거절한다. 그 명령이 다시 글에 서면 일꾼이 필수 검사에서 막힌다.
        assert!(!work.contains("git -C <root> symbolic-ref"), "거절되는 꼴로 루트의 가지를 묻는다");
        // **심은 글이 거절되는 꼴을 아무 걸음에서도 시키지 않는다**(리뷰 moai-rgp9.sdj 1번) —
        // `SUBDIR` 이 `$(git -C <root> …)` 이던 자리가 그것이었고, 그 걸음은 워크트리 **안으로**
        // 드는 것이라 ExitWorktree 로 피할 수도 없었다.
        // 꼴을 **이르는** 자리만 잰다 — `GIT_SHAPES` 는 거절되는 꼴의 이름을 대야 하므로 그 글자가
        // 그 안에 서는 것은 맞다.
        for shape in ["$(git -C", "git -C <root> rev-parse"] {
            assert!(!work.contains(shape), "일꾼 글이 거절되는 꼴을 시킨다 — {shape}");
            assert!(!supervise.contains(shape), "감독 글이 거절되는 꼴을 시킨다 — {shape}");
        }
        assert!(
            BRANCH_CHECK.contains("run `git symbolic-ref -q HEAD` **in the root**"),
            "대조를 어디서 하는지 안 적었다"
        );
        for (piece, why) in [
            ("One command per call", "한 호출에 한 명령이라는 줄이 없다"),
            ("`git commit -F <that file>`", "긴 커밋 글을 파일로 주라는 줄이 없다"),
            ("`-m \"…\"` on one plain command is fine", "-m 이 되는 것을 안 적어 브리프의 걸음과 어긋난다"),
            ("`bash /abs/path/script.sh`", "여러 걸음을 스크립트로 빼라는 줄이 없다"),
            ("computed at runtime", "치환으로 만든 명령이 거절되는 것을 안 적었다"),
            ("Do not aim git at the root from inside the worktree", "루트를 겨누지 말라는 줄이 없다"),
            ("`git status -- .moai/`", "남의 트래커 줄을 쓸어 가는 자리를 안 적었다"),
        ] {
            assert!(GIT_SHAPES.contains(piece), "{why} — {piece}");
        }
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

    /// **Claude 전용 걸음은 다 표의 줄이다**(moai-xs2h.r2d). 에픽이 든 일곱 걸음(과 짝인 `ExitWorktree`)이 Claude
    /// 열에 서고, 다른 두 열은 그 낱말을 하나도 안 쓴다 — Codex 창에 `EnterWorktree` 를 치라고 하면 그 걸음이 그냥
    /// 실패한다. 칸이 빈 글이면 표가 아무것도 안 가르치니 빈 칸은 [`NO_VERB`] 로 적혀야 한다.
    #[test]
    fn the_words_table_splits_every_claude_only_step() {
        let claude_only =
            ["EnterWorktree", "ExitWorktree", "AskUserQuestion", "/code-review", "/model", "/clear", "TaskStop"];
        for word in claude_only {
            let row =
                VERBS.iter().find(|v| v.words[0].contains(word)).unwrap_or_else(|| panic!("{word} 의 줄이 표에 없다"));
            for (vendor, cell) in VENDORS.iter().zip(row.words).skip(1) {
                // `/model` 은 Codex 도, `/clear` 는 Antigravity 도 같은 낱말이다(2026-10-04 사용자 결정) — 그 둘만
                // 같은 글을 받는다.
                let shared = matches!((word, *vendor), ("/model", "Codex") | ("/clear", "Antigravity"));
                if !shared {
                    assert!(!cell.contains(word), "{vendor} 열에 Claude 의 {word} 가 섰다 — {}", row.step);
                }
            }
        }
        for v in &VERBS {
            for (vendor, cell) in VENDORS.iter().zip(v.words) {
                assert!(!cell.trim().is_empty(), "{vendor} 의 `{}` 칸이 비었다 — 없으면 {NO_VERB} 로 적는다", v.step);
            }
        }
    }

    /// **표는 두 스킬에 같은 글로 서고, 규칙 3 이 그것을 가리킨다**(사용자 결정 2026-10-04). 규칙 3 은 AGENTS 블록에도
    /// 서는데 그 블록은 세 에이전트가 다 읽는다 — `/code-review` 하나만 대던 판은 Codex 창에 없는 명령을 시켰다. 가리키는
    /// 절 이름과 표의 머리가 갈리면 가리킨 자리가 없다.
    #[test]
    fn the_skills_carry_the_words_table_and_rule_three_points_at_it() {
        let section = verbs_section();
        let title = section.lines().next().unwrap().trim_start_matches("## ");
        assert!(skill().contains(&section), "moai 스킬에 낱말표가 없다");
        // **감독과 일꾼 글은 표 없이 도구를 바로 적는다**(moai-obxm) — 둘 다 Claude Code 에만 가니, 표를 실으면 받는 창이
        // 없는 열을 읽고 *기울인* 걸음 이름은 한 번 더 건너뛰는 길이 된다.
        for (whose, text) in [("감독 스킬", supervise()), ("일꾼 글", worker())] {
            assert!(!text.contains(title), "{whose} 에 낱말표가 섰다");
            for v in &VERBS {
                let step = v.step.split(" (").next().unwrap_or(v.step);
                assert!(
                    !text.contains(&format!("*{step}*")),
                    "{whose} 가 걸음을 *{step}* 로 가리킨다 — 도구를 바로 적는다"
                );
            }
        }
        let head = section.lines().find(|l| l.starts_with("| Step |")).expect("표 머리가 없다");
        for vendor in VENDORS {
            assert!(head.contains(vendor), "표 머리에 {vendor} 열이 없다 — {head}");
        }
        assert!(section.contains(&format!("A `{NO_VERB}` is")), "빈 칸을 어떻게 읽는지 안 말한다");
        let three = rules();
        let three = &three[three.find("**3.").unwrap()..three.find("**4.").unwrap()];
        assert!(three.contains(&format!("\"{title}\"")), "규칙 3 이 낱말표를 안 가리킨다 — {three}");
        // 세 에이전트의 리뷰 낱말을 다 싣는다 — AGENTS 블록만 읽는 창에는 표가 든 스킬이 없을 수 있다(리뷰 9번).
        for (vendor, word) in VENDORS.iter().zip(REVIEW_VERB.words) {
            assert!(three.contains(word), "규칙 3 에 {vendor} 의 리뷰 낱말이 없다 — {three}");
        }
        assert!(VERBS.iter().any(|v| v.step == REVIEW_VERB.step), "규칙 3 이 대는 리뷰 걸음이 표에 없다");
    }

    /// **메시지 머리의 줄과 일꾼 글이 읽는 줄이 한 벌이다**(moai-snyk, moai-obxm). 머리는 맡길 일만 싣고 읽는 법은
    /// 그 아래 일꾼 글에 있다 — 머리에 줄을 더하고 일꾼 글이 모르면 일꾼은 그 값을 버리고, 없는 줄을 읽으면 빈 값으로
    /// 일한다. 감독이 채우는 목록(3)도 머리의 자리와 한 벌이다 — 목록에서 빠진 자리는 꺾쇠째 일꾼에게 간다.
    #[test]
    fn the_message_carries_every_slot_the_steps_read() {
        let (message, work, supervise) = (message(), worker(), supervise());
        let section = &work[work.find("## The assignment").expect("일꾼 글에 메시지를 읽는 절이 없다")..];
        let section = &section[..section[3..].find("\n## ").map_or(section.len(), |n| n + 3)];
        // 첫 두 줄은 무엇을 맡기는가와 먼저 읽을 것이다 — 나머지 줄이 `<머리>: <값>` 이다.
        let heads: Vec<&str> =
            message.lines().skip(2).map(|l| l.trim().split_once(": ").expect("메시지 줄에 머리가 없다").0).collect();
        assert!(heads.len() >= 7, "메시지의 줄이 모자란다 — {heads:?}");
        // 걷은 두 자리가 돌아오면 붉어진다 — 보고할 곳은 메시지의 `from` 이고, 보고 뒤에는 늘 턴을 끝낸다.
        assert!(!message.contains("<my name>") && !message.contains("After the report"), "걷은 자리가 메시지에 섰다");
        for head in &heads {
            assert!(section.contains(&format!("- `{head}:`")), "일꾼 글이 메시지의 `{head}:` 줄을 안 읽는다");
        }
        let read: Vec<&str> = section
            .lines()
            .filter_map(|l| l.strip_prefix("- `"))
            .filter_map(|l| l.split_once(":`").map(|(h, _)| h))
            .collect();
        assert_eq!(read, heads, "일꾼 글이 메시지에 없는 줄을 읽거나 차례가 다르다");
        // 메시지의 자리는 모두 감독이 채운다 — 목록에 없는 자리는 꺾쇠째 간다.
        let list = slot_list(&supervise);
        let slots = message.split('<').skip(1).filter_map(|s| s.split_once('>')).map(|(s, _)| format!("`<{s}>`"));
        for slot in slots {
            assert!(list.contains(&slot), "감독이 메시지의 {slot} 를 안 채운다 — {list}");
        }
        // 감독이 채우는 자리는 모두 메시지에 선다 — 메시지에 없는 자리를 채우라고 하면 감독은 어디 채울지 모른다.
        for slot in list.split('`').skip(1).step_by(2) {
            assert!(message.contains(slot), "감독이 메시지에 없는 {slot} 를 채우라고 한다");
        }
        // 메시지의 첫 줄은 일꾼 글의 어디서 시작할지를 댄다 — 새 일·거둔 일·펼친 에픽 셋이 다 일꾼 글에 있어야 한다.
        assert!(message.lines().next().unwrap().contains("from step 1"), "메시지가 어디서 시작할지 안 댄다");
        for start in ["from step 1", "from \"Carrying on stalled work\"", "from step 2"] {
            assert!(supervise.contains(start), "감독이 `{start}` 메시지를 안 보낸다");
            assert!(section.contains(start), "일꾼 글이 `{start}` 를 어떻게 읽는지 안 댄다");
        }
    }

    /// **감독과 일꾼은 Claude Code 의 제 수단으로 말한다**(2026-10-06 사용자 결정, moai-obxm). moai 는 통신을 안 든다 —
    /// 우편함·출석을 걷었으니(moai-5uwh) 그 명령이 글에 다시 서면 받는 창이 없는 명령을 친다. 일꾼은 `ListAgents` 의
    /// 이 저장소 idle 세션이고, 일은 `SendMessage` 로 가며 보고도 그렇게 온다. 감독은 `notify_when_idle` 로 기다리고
    /// `ListAgents` 를 되풀이해 훑지 않는다.
    #[test]
    fn the_supervisor_and_the_workers_talk_through_claude_code() {
        let (supervise, brief) = (supervise(), worker());
        for (whose, text) in
            [("감독 스킬", supervise.as_str()), ("일꾼 글", brief.as_str()), ("AGENTS 블록", &agents())]
        {
            for gone in ["moai hello", "moai send", "moai inbox", "moai agents", "any-idle-worker", "moai-work"] {
                assert!(!text.contains(gone), "{whose} 이 걷은 {gone} 를 가르친다");
            }
        }
        let round = supervise.find("## One round").expect("한 바퀴가 없다");
        // 넷의 낱말은 바퀴 앞에 한 번 선다 — 바퀴가 그 낱말로 말한다.
        for tool in ["`ListAgents`", "`SendMessage(to: <name>, message: …)`", "`notify_when_idle: true`"] {
            assert!(supervise[..round].contains(tool), "감독이 바퀴 앞에 {tool} 를 안 댄다");
        }
        assert!(
            supervise[..round]
                .contains("**A worker is every idle session of this\nrepository in `ListAgents`, except you**"),
            "일꾼이 이 저장소의 idle 세션 전부라는 말이 없다"
        );
        assert!(
            supervise[..round].contains("Never poll `ListAgents` in a loop"),
            "ListAgents 를 되풀이해 훑지 말라는 말이 없다"
        );
        assert!(
            supervise[..round].contains("`@path` in a message attaches nothing"),
            "@path 가 아무것도 안 붙인다는 말이 없다"
        );
        assert!(
            supervise[..round].contains("different permission mode"),
            "권한 모드가 다른 창이 메시지를 붙든다는 말이 없다"
        );
        assert!(
            supervise[..round].contains("`notify_when_idle` answers only for a session on this machine"),
            "다른 기계의 일꾼에게서도 idle 알림이 온다고 읽힌다"
        );
        assert!(
            supervise[..round].contains("A subagent sends under its parent"),
            "서브에이전트가 부모의 주소로 보낸다는 말이 없다"
        );
        // 보내기는 SendMessage 이고 idle 알림을 건다.
        let send = &supervise[supervise.find("**3. Send.**").expect("감독의 3 이 없다")
            ..supervise.find("**4. Wait.**").expect("감독의 4 가 없다")];
        assert!(send.contains("SendMessage(to: <worker>, message: "), "감독이 SendMessage 로 안 보낸다");
        assert!(send.contains("notify_when_idle: true"), "감독이 보낼 때 idle 알림을 안 건다");
        // **메시지는 걸음 파일을 읽으라고만 한다**(2026-10-07 사용자 결정, moai-fim6). 32KB 를 손으로 옮겨 붙이던 판은
        // 보낼 때마다 출력 토큰 8~10k 가 들었고 줄이거나 바꿔 옮긴 글을 아무것도 못 잡았다 — 되돌리면 여기서 붉어진다.
        assert!(send.contains("`references/worker.md`"), "감독의 3 이 일꾼 걸음 파일을 이름으로 안 댄다");
        assert!(send.contains("tells the worker to `Read` that file"), "감독의 3 이 일꾼에게 그 파일을 읽히지 않는다");
        assert!(send.contains("**Do not copy the file into the message**"), "감독이 걸음 파일을 메시지에 옮겨 붙인다");
        assert!(
            send.contains("\"Base directory for this skill\""),
            "감독이 걸음 파일의 절대 경로를 어디서 읽는지 모른다"
        );
        assert!(!send.contains(".claude/moai-plugin"), "걸음 파일의 자리를 박았다 — 스킬의 기준 디렉터리에서 읽는다");
        for gone in ["whole text", "paste", "a blank line, references/worker.md"] {
            assert!(!send.contains(gone), "감독의 3 이 걸음 전부를 싣던 말을 들고 있다 — {gone}");
        }
        // 세 메시지의 첫 줄이 모두 그 파일을 읽으라고 한다 — 새 일·거둔 일·펼친 에픽.
        let read = "Read <steps file> and follow its steps from ";
        assert!(
            message().lines().next().unwrap().contains(&format!("{read}step 1.")),
            "메시지 첫 줄이 걸음 파일을 안 읽힌다"
        );
        for start in ["\"Carrying on stalled work\".", "step 2."] {
            assert!(supervise.contains(&format!("{read}{start}")), "감독의 `{start}` 메시지가 걸음 파일을 안 읽힌다");
        }
        assert!(!supervise.contains("worker steps below"), "메시지가 아래에 붙은 걸음을 가리킨다");
        assert!(!brief.contains("The lines above are your assignment"), "일꾼 글이 메시지 아래에 붙은 글로 읽힌다");
        let wait = &supervise[supervise.find("**4. Wait.**").unwrap()
            ..supervise.find("**5. Check the report").expect("감독의 5 가 없다")];
        assert!(wait.contains("cross-session message"), "감독이 보고를 세션 사이 메시지로 안 받는다");
        assert!(wait.contains("**Do not poll `ListAgents`**"), "감독이 기다리며 ListAgents 를 훑는다");
        // 0 의 재회수도 ListAgents 를 읽는다 — 출석부가 없으니 살아 있는 세션은 그것뿐이다.
        let reclaim = &supervise[round..supervise.find("**1. Pick.**").expect("감독의 1 이 없다")];
        assert!(reclaim.contains("`ListAgents` shows a `busy` session"), "재회수가 바쁜 세션을 ListAgents 로 안 본다");
        // 일꾼은 메시지의 `from` 에게 SendMessage 로 보고한다.
        assert!(
            brief.contains("The message's `from` is the supervisor"),
            "일꾼이 보고할 곳을 메시지의 from 으로 안 읽는다"
        );
        assert!(
            brief[step_at(&brief, "12")..].contains("`SendMessage(to: <supervisor>"),
            "일꾼이 SendMessage 로 보고하지 않는다"
        );
        // `from` 은 감독 프로세스의 이름이라 감독이 다시 뜨면 보고가 갈 곳이 없다(moai-ybns.451.3zc). 일꾼은 그 보고를
        // 에픽의 노트로 남기고, 다시 뜬 감독은 기다리기 전에 트래커에서 보낸 일과 받지 못한 보고를 읽는다 — 없으면 감독은
        // 오지 않을 보고를 기다리고, 그 일꾼은 2 에서 늘 빠진다.
        assert!(brief[step_at(&brief, "12")..].contains("**If that send fails**"), "보고가 실패하면 사라진다");
        assert!(brief[step_at(&brief, "12")..].contains("`moai note <epic> -b -`"), "실패한 보고를 트래커에 안 남긴다");
        assert!(wait.contains("    moai show -g 'report:' --all "), "다시 뜬 감독이 트래커의 보고를 안 읽는다");
        assert!(wait.contains("when you start or resume"), "다시 뜬 감독이 기다리기만 한다");
        // 보낸 backlog 와 확인한 보고도 트래커에 줄머리 노트로 남는다(moai-9s9s.ctx) — 없으면 다시 뜬 감독이 아직 안
        // 펼친 backlog 를 둘째 일꾼에게 또 보내고, 이미 확인한 보고를 다시 확인한다.
        assert!(supervise.contains("moai note <id> 'Sent: <worker>'"), "보낸 backlog 를 트래커에 안 남긴다");
        // 펼친 뒤 보낸 에픽도 집히기 전에는 첫 칸이라 `-s in_progress,review` 에 안 선다 — 종류로 거르면 그 에픽을 또 보낸다.
        assert!(wait.contains("    moai show -g 'Sent:' "), "다시 뜬 감독이 보낸 backlog·에픽을 안 읽는다");
        assert!(
            supervise.contains("moai note <epic> 'Report-checked: <merge hash>'"),
            "확인한 보고를 트래커에 안 남긴다"
        );
        assert!(wait.contains("**Only a note that opens with the marker counts**"), "본문의 같은 글까지 보고로 센다");
        // 스킬이 저장소 밖 플러그인 캐시에 있으면 일꾼의 Read 가 권한을 묻는다(moai-9s9s.qmd) — 사람이 비운 일꾼은 그 물음에서 선다.
        assert!(
            supervise.contains("**If that base directory lies outside `<root>`**"),
            "저장소 밖 단계 파일의 권한 물음을 안 알린다"
        );
        assert!(supervise.contains("The settings are theirs — do not write them."), "감독이 사람의 설정을 고친다");
        // **tmux 밖의 감독은 창에 아무것도 안 친다**(moai-obxm) — 5-1 을 걷었다. tmux 안에서는 칸을 만지되(2026-10-10 사용자
        // 결정, moai-u99i) 그 명령은 `moai-tmux` 에만 선다 — 감독 글이 `send-keys` 를 들면 tmux 없는 감독도 그 줄을 친다.
        assert!(!supervise.contains("**5-1."), "감독이 창을 비우는 5-1 이 남았다");
        assert!(
            !supervise.contains("send-keys") && !supervise.contains("tmux_pane"),
            "감독 글이 tmux 칸을 직접 만진다"
        );
        let check = &supervise[supervise.find("**5. Check the report").unwrap()..];
        assert!(
            check.contains("**Outside tmux, clearing a window is the\nperson's**"),
            "tmux 밖에서 창을 비우는 것이 사람의 몫이라는 말이 없다"
        );
        assert!(
            check.contains("**Inside tmux you may clear it yourself**"),
            "tmux 안의 감독이 칸을 비운다는 갈래가 없다"
        );
        // 비우는 조건 셋 — 보고 확인·idle·빈 입력 칸. 하나라도 빠지면 옛 5-1 처럼 사람의 초안이나 보낸 일을 지운다.
        for gate in ["only after the report is checked", "read `idle`", "its input\nbox is empty"] {
            assert!(check.contains(gate), "감독의 5 가 칸을 비우는 조건 `{gate}` 를 안 댄다");
        }
        let head = &supervise[..round];
        assert!(head.contains("**Inside tmux, load `moai-tmux` too.**"), "감독이 tmux 안에서 moai-tmux 를 안 읽는다");
        assert!(head.contains("**Without `$TMUX` nothing of it applies**"), "tmux 없는 감독의 걸음이 바뀐다고 읽힌다");
    }

    /// **`ListAgents` 의 이름과 tmux 칸을 잇는 짝은 한 자리에 선다**(moai-u99i.xo8). 감독의 tmux 스킬이 그 상수를 그대로
    /// 싣고, 되살리기 스킬(moai-uqf7)도 같은 것을 싣는다. 실제 기록의 꼴로 돌려, 산 기록과 죽은 기록(pid 가 없거나
    /// 다른 프로세스가 그 pid 를 다시 쓴 것)을 가르는지, 칸 id 를 `%N` 으로 뽑는지 잰다. 다른 tmux 서버에서 도는 산 세션의
    /// 칸은 `-` 다 — `%N` 은 서버마다 따로 세어, 그대로 내면 부르는 쪽 서버의 엉뚱한 칸에 친다.
    #[test]
    fn the_session_map_reads_the_records() {
        assert!(tmux().contains(SESSIONS), "tmux 스킬이 짝을 그대로 안 싣는다");
        assert!(recover().contains(SESSIONS), "되살리기 스킬이 짝을 그대로 안 싣는다");
        assert!(SESSIONS.contains("procStart") && SESSIONS.contains("/proc/{pid}/stat"), "짝이 pid 재사용을 안 거른다");
        if std::process::Command::new("python3").arg("-c").arg("pass").output().is_err() {
            return; // python3 가 없는 기계 — 글만 잰다.
        }
        if !std::path::Path::new("/proc/self/stat").exists() {
            return; // `/proc` 가 없는 기계(macOS) — 짝은 리눅스의 것이다. 글만 잰다.
        }
        let s = crate::scratch::Scratch::new("tmux-session-map");
        let dir = s.path().join(".claude/sessions");
        std::fs::create_dir_all(&dir).unwrap();
        // 이 시험 프로세스는 살아 있다 — 그 시작 시각이 `procStart` 다.
        let me = std::process::id();
        let stat = std::fs::read_to_string(format!("/proc/{me}/stat")).unwrap();
        let start = stat[stat.rfind(')').unwrap() + 2..].split_whitespace().nth(19).unwrap().to_string();
        let record = |pid: u32, start: &str, name: &str, tmux: Option<&str>| {
            let tmux = tmux.map_or(String::new(), |t| format!(r#","tmux":"{t}""#));
            format!(
                r#"{{"pid":{pid},"sessionId":"s-{name}","cwd":"/repo","kind":"interactive","procStart":"{start}","name":"{name}","status":"idle"{tmux}}}"#
            )
        };
        std::fs::write(dir.join("1.json"), record(me, &start, "here", Some("Shop Work:@16.%39"))).unwrap();
        std::fs::write(dir.join("2.json"), record(me, "1", "reused", Some("moai:@0.%4"))).unwrap();
        std::fs::write(dir.join("3.json"), record(me, &start, "plain", None)).unwrap();
        std::fs::write(dir.join("4.json"), "not json").unwrap();
        let shell = SESSIONS.replace("~/.claude/sessions", &dir.display().to_string());
        let run = |tmux: Option<&str>| {
            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(&shell);
            match tmux {
                Some(t) => cmd.env("TMUX", t),
                None => cmd.env_remove("TMUX"),
            };
            let out = cmd.output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(
            run(None),
            "alive\there\t%39\tidle\t/repo\ts-here\n\
             dead\treused\t%4\tidle\t/repo\ts-reused\n\
             alive\tplain\t-\tidle\t/repo\ts-plain\n",
            "짝의 줄 꼴이 바뀌었다"
        );
        // 부르는 쪽이 이 시험 프로세스와 다른 tmux 서버에 서 있다 — 산 세션의 칸은 `-`, 죽은 줄은 기록 그대로다.
        assert_eq!(
            run(Some("/nonexistent/moai-other-server,1,0")),
            "alive\there\t-\tidle\t/repo\ts-here\n\
             dead\treused\t%4\tidle\t/repo\ts-reused\n\
             alive\tplain\t-\tidle\t/repo\ts-plain\n",
            "다른 tmux 서버의 칸 id 를 그대로 낸다"
        );
    }

    /// 글이 가르치는 `tmux` 줄을 세며 하나하나 잰다 — 칸을 `-t` 로 겨누거나, 칸을 안 건드리는 넷(설정 읽기 `show`·
    /// 테두리 둘을 쓰는 `set -g`·버퍼에 싣는 `load-buffer`) 중 하나이고, `kill-*` 이 아니고, 하위 명령 앞에 서버를 고르는
    /// 깃발이 없다. `moai-tmux` 와 `moai-recover` 가 같은 잣대로 잰다 — 둘 다 사람의 tmux 서버에 친다.
    fn tmux_calls(text: &str) -> usize {
        let mut calls = 0;
        // 짝의 파이썬은 `tmux = r.get(…)` 처럼 낱말 `tmux` 로 여는 줄을 든다 — 명령이 아니라 빼고 센다.
        for line in text.replace(SESSIONS, "").lines() {
            let spans: Vec<&str> = line.split('`').skip(1).step_by(2).collect();
            let commands = std::iter::once(line.trim()).chain(spans.iter().copied());
            for cmd in commands.filter(|c| c.starts_with("tmux ")) {
                calls += 1;
                let words: Vec<&str> = cmd.split_whitespace().collect();
                assert!(!words[1].starts_with("kill"), "tmux 스킬이 kill 을 가르친다 — {cmd}");
                let free = matches!(words[1], "show" | "load-buffer")
                    || (words[1] == "set" && words.get(3).is_some_and(|w| w.starts_with("pane-border-")));
                assert!(free || words.contains(&"-t"), "칸을 안 겨눈 tmux 줄 — {cmd}");
                // 하위 명령 앞의 깃발(`-L`·`-S`)이 서버를 고른다 — 뒤의 `-S` 는 `capture-pane` 의 시작 줄이다.
                assert!(!words[1].starts_with('-'), "하위 명령 앞에 깃발이 섰다 — {cmd}");
            }
        }
        calls
    }

    /// "하지 않는 것" 절이 죽이는 명령 넷과 헤드리스로 만드는 깃발 셋을 이름째 댄다.
    fn never_kills_nor_runs_headless(text: &str) {
        let never = &text[text.find("## What never happens").expect("하지 않는 것 절이 없다")..];
        let never = &never[..3 + never[3..].find("\n## ").unwrap()];
        for kill in ["`kill-server`", "`kill-session`", "`kill-pane`", "`pkill`"] {
            assert!(never.contains(kill), "하지 않는 것에 {kill} 가 없다");
        }
        for flag in ["`claude -p`", "`--dangerously-*`", "`--permission-mode`"] {
            assert!(never.contains(flag), "하지 않는 것에 {flag} 가 없다");
        }
    }

    /// **`moai-tmux` 는 이름 댄 칸만 만진다**(2026-10-10 사용자 결정, moai-u99i). 사람의 tmux 서버라 맨 `kill-server` 하나가
    /// 그 사람의 세션을 다 죽인다(2026-09-18). 그래서 `tmux` 를 부르는 줄은 모두 칸을 `-t` 로 겨누거나, 칸을 안 건드리는
    /// 넷(설정 읽기 `show`·테두리 둘을 쓰는 `set -g`·버퍼에 싣는 `load-buffer`) 중 하나다. `kill-*` 은 하지 말라는 줄에만
    /// 선다. 칸을 열 때 `claude` 에 붙는 깃발은 `--model` 하나다 — `-p`·`--dangerously-*`·권한 모드가 서면 헤드리스가 된다.
    /// 치기 전의 잣대(빈 입력 칸)와 열기 전에 묻기도 글로 선다.
    #[test]
    fn the_tmux_skill_touches_only_named_panes() {
        let text = tmux();
        let calls = tmux_calls(&text);
        assert!(calls >= 8, "tmux 스킬의 명령을 못 셌다 — {calls}");
        never_kills_nor_runs_headless(&text);
        // 띄우는 줄은 하나고, 깃발은 모델 하나다.
        let launches: Vec<&str> = text.lines().filter(|l| l.contains("'claude ")).collect();
        assert_eq!(launches.len(), 1, "claude 를 띄우는 줄이 하나가 아니다 — {launches:?}");
        assert!(launches[0].contains("'claude --model <model>; exec bash'"), "띄우는 줄에 다른 깃발이 섰다");
        // 입력 칸이 비어야 친다 — 옛 5-1 이 사람의 초안을 건지느라 480줄이었다.
        assert!(text.contains("**Then do not type. Tell the person"), "빈 입력 칸의 잣대가 없다");
        let clear = &text[text.find("## Clear a worker's window").unwrap()..text.find("## When a message").unwrap()];
        assert!(clear.contains("**Its input box is empty**"), "비우기 전에 입력 칸을 안 본다");
        assert!(
            clear.contains("**Its report is checked**") && clear.contains("`Report-checked:`"),
            "보고 확인 전에 비운다"
        );
        let paste = &text[text.find("## When a message").unwrap()..text.find("## When a worker stalls").unwrap()];
        assert!(paste.contains("input box is empty"), "붙이기 전에 입력 칸을 안 본다");
        assert!(paste.contains("paste-buffer -p"), "여러 줄을 붙이기(bracketed paste)로 안 싣는다");
        assert!(paste.contains("**Tell the person**"), "붙인 것을 사람에게 안 알린다");
        let stall = &text[text.find("## When a worker stalls").unwrap()..text.find("## No idle worker").unwrap()];
        assert!(
            stall.contains("**Tell the\nperson**") && stall.contains("do not press a\nkey"),
            "멈춘 칸에 대신 답한다"
        );
        let open = &text[text.find("## No idle worker").unwrap()..];
        assert!(open.contains("ask the person **once**") && open.contains("only on their yes"), "묻지 않고 칸을 연다");
        // 이름표는 칸 옵션이고, 보이기는 도는 서버에만 — 사람의 설정 파일은 안 고친다.
        assert!(text.contains("tmux set-option -p -t <pane> @moai '<worker> <id>'"), "칸 이름표 줄이 없다");
        assert!(text.contains("tmux set-option -p -u -t <pane> @moai"), "칸 이름표를 안 지운다");
        assert!(text.contains("#{?@moai,#{@moai} ,}<the value you read>"), "테두리 꼴에 이름표를 안 붙인다");
        assert!(!text.contains("tmux.conf") && !text.contains("source-file"), "사람의 tmux 설정 파일을 고친다");
        // 사람이 칸을 훑는 중(copy mode)이면 친 키가 tmux 에 간다 — 입력 칸을 보기 전에 그것부터 본다.
        let empty = &text[text.find("## Is the input box empty").unwrap()..text.find("## Label the pane").unwrap()];
        assert!(empty.contains("#{pane_in_mode}") && empty.contains("copy mode"), "칸이 copy mode 인지 안 본다");
        assert!(empty.contains("capture-pane -p -e"), "자리글을 사람의 초안과 가를 색을 안 본다");
        // 사람의 허락을 기다리는 메시지는 붙이지 않는다 — 그 문을 건너뛰고, 허락하면 같은 일이 두 번 간다.
        assert!(
            paste.contains("**A message\nheld for the person's approval is not one that failed:**"),
            "허락을 기다리는 메시지를 칸에 붙인다"
        );
        assert!(!paste.contains("is held for\napproval, and"), "허락을 기다리는 메시지를 붙이던 옛 말이 남았다");
        assert!(paste.contains("one line at the end naming you"), "붙인 메시지의 첫 줄을 `from:` 이 빼앗는다");
        assert!(
            worker().contains("its last line,\n`from: <name>`, names the supervisor"),
            "일꾼이 붙여 넣은 메시지의 보낸 이를 못 읽는다"
        );
        // 비운 것이 안 보이면 `/clear` 가 칸에 쳐진 채 남았을 수 있다 — 사람이 나중에 누르면 다음 메시지를 지운다.
        assert!(clear.contains("`/clear` may\nstand typed in that pane's box"), "남았을지 모를 `/clear` 를 안 알린다");
        // **감독 글이 대는 절은 이 스킬에 서 있다.** 절 이름을 바꾸면 감독 글이 없는 절을 가리킨다.
        let supervise = supervise();
        let mut named = 0;
        for piece in supervise.split("`moai-tmux`, ").skip(1) {
            // `"절"` 하나, 또는 `"절" and "절"` — 그 뒤의 따옴표는 다른 글이다.
            let mut rest = piece;
            while let Some(after) = rest.strip_prefix('"') {
                let (heading, tail) = after.split_once('"').expect("닫는 따옴표가 없다");
                let heading = heading.replace('\n', " ");
                named += 1;
                assert!(text.contains(&format!("\n## {heading}\n")), "감독 글이 없는 절 \"{heading}\" 을 댄다");
                rest = tail.strip_prefix(" and ").unwrap_or("");
            }
        }
        assert!(named >= 5, "감독 글이 tmux 스킬의 절을 안 댄다 — {named}");
        for (step, line) in [
            ("2", "Inside tmux, ask the person first whether to open new worker"),
            ("3", "Inside tmux, label the worker's pane as you send"),
            ("4", "Inside tmux, an\nidle notice with no report"),
        ] {
            assert!(supervise.contains(line), "감독의 {step} 에 tmux 갈래가 없다");
        }
    }

    /// **`moai-recover` 는 제가 연 칸만 만지고, 되살리는 줄은 `--resume` 하나다**(2026-10-10 사용자 결정, moai-uqf7).
    /// 되살려 달라는 말이 곧 허락이라 묻지 않고 칸을 열지만, 금은 `moai-tmux` 와 같다 — 이름 댄 칸, 죽이지 않기, 헤드리스
    /// 없음, 빈 입력 칸에만 붙이기. tmux 밖이면 칠 줄을 내기만 한다. 차례는 일꾼 먼저, 감독 마지막이다 — 감독의
    /// `ListAgents` 가 일꾼을 봐야 한다. 짝에 없는 것은 대화 기록에서 읽고, 둘째 짝은 적지 않는다.
    #[test]
    fn the_recover_skill_resumes_into_its_own_panes() {
        let text = recover();
        let calls = tmux_calls(&text);
        assert!(calls >= 5, "되살리기 스킬의 tmux 명령을 못 셌다 — {calls}");
        never_kills_nor_runs_headless(&text);
        // 짝은 하나다 — `~/.claude/sessions` 를 읽는 둘째 파이썬이 서면 기록의 꼴이 바뀌는 날 한쪽만 고쳐진다.
        assert_eq!(text.matches("~/.claude/sessions/*.json").count(), 1, "세션 기록을 읽는 짝이 둘이다");
        // 띄우는 줄은 둘 — tmux 칸과, tmux 밖에서 사람이 칠 줄. 깃발은 `--resume` 하나다. 코드 줄에서 `claude ` 를 든
        // 줄을 다 센다 — `--resume` 앞에 다른 깃발(`--model …`)을 끼운 줄도 걸려야 한다.
        let launches: Vec<&str> = text.lines().filter(|l| l.starts_with("    ") && l.contains("claude ")).collect();
        assert_eq!(
            launches,
            [
                "    tmux split-window -P -F '#{pane_id}' -t \"$TMUX_PANE\" -c <cwd> 'claude --resume <sessionId>; exec bash'",
                "    cd <cwd> && claude --resume <sessionId>",
            ],
            "claude 를 띄우는 줄이 둘이 아니거나 다른 깃발이 섰다"
        );
        for flag in ["claude -p", "--dangerously", "--permission-mode"] {
            assert_eq!(text.matches(flag).count(), 1, "{flag} 가 하지 않는 것 밖에도 섰다");
        }
        let open = &text[text.find("## 5. Inside tmux").expect("tmux 안의 절이 없다")..];
        let outside = &open[open.find("## 6. Outside tmux").expect("tmux 밖의 절이 없다")..];
        let open = &open[..open.find("## 6. Outside tmux").unwrap()];
        assert!(open.contains("paste-buffer -p -d -b moai-recover -t <pane>"), "여러 줄을 붙이기로 안 싣는다");
        assert!(outside.contains("open nothing"), "tmux 밖에서도 칸을 연다");
        // 빈 입력 칸의 잣대는 `moai-tmux` 의 절이다 — 절 이름을 바꾸면 이 글이 없는 절을 가리킨다.
        assert!(tmux().contains("\n## Is the input box empty\n"), "되살리기가 대는 tmux 절이 없다");
        assert!(open.contains("(`moai-tmux`, \"Is the input box empty\")"), "붙이기 전에 입력 칸을 안 본다");
        assert!(text.contains("**Their asking is the yes:**"), "되살려 달라는 말을 허락으로 안 읽는다");
        assert!(text.contains("**Workers first, the supervisor last**"), "일꾼 먼저·감독 마지막의 차례가 없다");
        assert!(
            text.contains("their names may have changed — run `ListAgents` again**"),
            "감독에게 일꾼의 이름이 바뀌었을 수 있다고 안 이른다"
        );
        // 이미 되살아난 것은 거른다 — 같은 세션을 두 칸에서 되살리면 두 창이 한 대화를 이어 쓴다.
        assert!(text.contains("**Drop it when it is back already**"), "이미 되살아난 세션을 안 거른다");
        // 사람이 감독을 먼저 되살려 그 창에서 부르면 제 줄이 그 id 를 든다 — 제 줄을 빼면 제 대화를 한 번 더 연다.
        assert!(text.contains("**your own row included**"), "제 줄이 든 id 를 이미 되살아난 것으로 안 읽는다");
        // 루트에는 산 세션이 여럿 선다 — 같은 cwd 로 거르면 루트의 죽은 감독이 늘 빠진다.
        assert!(text.contains("The\n  root is not such a place"), "루트에서도 같은 cwd 로 거른다");
        // 끝나지 못한 백그라운드 일과 끊긴 target 링크(moai-uqf7.e7z).
        assert!(
            text.contains("`<task-notification>`")
                && text.contains("`Async agent launched`")
                && text.contains("`moved to the background (ID: <id>)`"),
            "끝나지 못한 서브에이전트를 결과 글로 안 찾는다"
        );
        assert!(text.contains("mkdir -p /tmp/cargo-target/<name>"), "끊긴 target 링크를 다시 안 세운다");
        assert!(text.contains("Do not build it yourself"), "되살리는 쪽이 빌드한다");
        // 일꾼의 대화 기록은 처음 선 자리(루트)의 슬러그 밑에 남는다 — id 로 찾는다.
        assert!(text.contains("ls ~/.claude/projects/*/<sessionId>.jsonl"), "대화 기록을 id 로 안 찾는다");
        // 감독과 tmux 의 글이 죽은 세션의 자리에서 이 스킬을 댄다.
        assert!(supervise().contains("load `moai-recover`"), "감독 글이 되살리기를 안 댄다");
        assert!(tmux().contains("back is `moai-recover`"), "tmux 스킬의 멈춘 일꾼 절이 되살리기를 안 댄다");
    }
}
