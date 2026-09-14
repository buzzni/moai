//! 에이전트에게 주는 글. **한 출처다** — `init` 이 AGENTS.md 에 쓰는 블록,
//! `skill install` 이 심는 SKILL.md 와 참고 문서, 그리고 훅이 내는 거절문이
//! 모두 여기서 조각을 가져간다.
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

/// 규칙 셋의 이름. **스킬이 적은 규칙과 훅이 낸 거절문이 같은 이름을 댄다** —
/// 다르면 막힌 쪽이 무엇을 어겼는지 두 번 읽어야 한다.
pub const RULES: [&str; 3] = [
    "집은 것 밖에 새 이슈를 세우지 않는다",
    "저장소를 고치기 전에 하나를 집는다",
    "리뷰도 이슈다",
];

/// 거절문의 머리. 스킬의 규칙 제목과 글자가 같다.
pub fn rule_head(n: usize) -> String {
    format!("규칙 {n} — {}.", RULES[n - 1])
}

/// 리뷰 이슈를 세운 뒤의 세 걸음. **훅의 거절문과 스킬이 이것을 그대로 쓴다.**
///
/// **여기 적힌 명령은 그대로 쳐서 지나가야 한다.** `-m` 없는 `done` 을 일러
/// 줘 결과가 없다고 막은 적이 있다 — 규칙이 제가 일러 준 명령을 막는 자리는
/// 규칙이 아니라 덫이다.
// **줄 잇기(`\`)를 쓰지 않는다.** 그것은 개행과 함께 다음 줄의 앞 공백까지
// 먹어, 첫 명령만 왼쪽 끝에 붙는다.
pub const REVIEW_STEPS: &str = concat!(
    "  moai mv <id> in_progress      리뷰를 시작할 때\n",
    "  moai note <id> -b - < <리뷰 원문>   리뷰가 낸 글을 그대로\n",
    "  moai mv <id> done -m \"<무엇을 반영하고 무엇을 넘겼나>\"",
);

/// 리뷰 이슈에 붙는 태그. 훅은 리뷰 줄을 이 글자로 가르고, 가르치는 글은 같은
/// 글자로 세우게 한다 — 둘이 다르면 시킨 대로 세운 리뷰를 규칙이 못 알아본다.
pub const REVIEW_TAG: &str = "review";

/// 리뷰 이슈를 세우는 줄. `anchor` 는 `--parent <id>` 나 `-e <에픽>` 이다 — 규칙
/// 셋의 글과 두 거절문이 이 한 줄에서 나온다.
pub fn make_review(anchor: &str) -> String {
    format!("moai add \"리뷰 — <무엇을 보는가>\" -t {REVIEW_TAG} {anchor} -b \"<무엇을 왜 보는가>\"")
}

/// 리뷰를 닫는 두 걸음 — `REVIEW_STEPS` 에서 시작 걸음을 빼고 **실제 id** 를 넣은
/// 것. 닫기 거절문과 세션을 닫을 때의 붙듦이 쓴다. 손으로 다시 적던 두 자리는
/// 이미 서로 다른 글을 내고 있었다.
pub fn close_steps(id: &str) -> String {
    REVIEW_STEPS.lines().skip(1).map(|l| l.replace("<id>", id)).collect::<Vec<_>>().join("\n")
}

const CHEATSHEET: &str = r#"    moai status                            보드 · 경고 · 흐름 (세션은 여기서 시작)
    moai ready                             지금 집을 수 있는 일
    moai show <id>                         본문·자식·이력. 왜 그렇게 정했는지가 여기 있다
    moai show -g <키워드>                  이미 적어 뒀는지 찾는다
    moai show -s todo -t bug               필터 (쉼표 = 또는, 반복 = 그리고)
    moai show --tree                       에픽 → 이슈 → 자식
    moai ready --worktree                  옆 워크트리에서 집은 것까지 겹쳐 본다
    moai tui                               탐색기로 돌아다닌다. SPC n 으로 생각을 담는다
    moai add "제목" -p 1 -t bug -e <에픽>  만들기
    moai mv <id> in_progress               집기  →  review  →  done
    moai edit <id> --tag parser            고치기
    moai note <id> "발견한 것"             다음 사람이 읽을 메모
    moai defer <id> -m "왜"                지금 안 할 일을 계획에서 뺀다

모든 명령에 `--json` 이 붙는다. `ready --json` 은 `{"ready":[…],"held":[…]}` —
`held` 는 미뤄 둔 것·빈 묶음에 막혀 못 집는 일과 도로 집을 곳이다."#;

const NO_GATE: &str = "승인 게이트가 없다 — 무엇이든 만들고 무엇이든 옮길 수 있다. 사람을 부르지 않는다.";

const FORKS: &str = r#"**1. `add` 냐 `idea` 냐** — 가르는 것은 하나다. *지금 집을 것인가.*
집을 것이면 `moai add`, 나중에 볼 것이면 `moai idea add "떠오른 것"`.
idea 는 보드에도 `ready` 에도 안 들어 계획을 흐리지 않는다.
**적지 않고 넘어가는 것이 제일 나쁘다.**

**2. `defer` 냐 `done` 이냐** — 안 하기로 한 것을 `done` 으로 옮기지 않는다.
`moai defer <id> -m "왜"` 는 칸도 종류도 안 바꾸고, `--undo` 로 같은 줄이
그대로 돌아온다. idea 는 "아직 일이 아닌 것", defer 는 "일이지만 지금은 아닌 것".

**3. 에픽으로 쪼갤 만한가** — 파일 하나로 안 끝나는 요청이면 코드를 쓰기 전에
에픽 하나 + 이슈 3~7개로 쪼갠 안을 사람에게 **한 번** 보여주고 물어본다.
"좋다" 를 받으면 `moai add --from -` 로 한 번에 만든다 (`--dry-run` 으로 먼저 봐도 된다).

    moai add --from - <<'MD'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    - [p2] 둘째 이슈
    MD"#;

const IDEAS: &str = r#"    moai idea add "반짝 떠오른 것"                 담기
    moai idea add "긴 생각" -b -                   본문은 stdin 에서
    moai idea ls                                   쌓인 것 보기
    moai show -g <키워드>                          이미 적어 뒀는지 찾기

때가 되면 하나를 에픽과 이슈로 펼친다. 펼치면 그 생각은 닫힌다.

    moai idea promote <id> --from - <<'MD'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    MD"#;

const DEFERRING: &str = r#"    moai defer <id> -m "다음 분기에"       계획에서 잠시 뺀다
    moai defer <id> --undo                 도로 집는다
    moai show --deferred                   미뤄 둔 것만 본다

칸도 종류도 안 바뀐다 — 같은 줄이 그대로 돌아온다. 미룬 것은 `moai ready` 와
보드와 경고에서 빠지고, `moai status` 가 한 줄로 그것을 비춘다. 에픽·마일스톤·
부모를 미루면 그 밑의 일도 같이 빠진다."#;

const GROUPS: &str = r#"    moai epic add "저장 계층"                      에픽
    moai milestone add "v0.1"                      마일스톤
    moai add "제목" -e <에픽> --milestone <마일스톤>
    moai show <에픽|마일스톤 id>                   그 밑에 무엇이 있는지
    moai show --milestone <id>                     그 마일스톤에 딸린 전부

**소속은 물려받는다.** 자식은 부모의 에픽을, 이슈는 제 에픽의 마일스톤을
물려받는다. `--parent <에픽>` 으로 만든 자식은 그 에픽에 든다. 이슈마다 다시
적지 않는다 — 에픽을 옮기면 멤버가 따라온다.

**묶음의 칸은 멤버에서 읽는다.** 에픽·마일스톤을 `moai mv` 로 옮기지 않는다 —
멤버를 하나 집으면 `in_progress` 로, 다 끝나면 `done` 으로 저절로 선다. 남은
멤버가 있는데 접으려면 그 멤버를 `moai defer` 한다 — 끝난 멤버가 하나도 없으면
미뤄도 첫 칸이니, 그때는 묶음을 `moai defer` 해 계획에서 뺀다."#;

/// 여러 프로젝트. **`main` 에 있는 것만 적는다** — 탐색기의 프로젝트 층이나
/// 프로젝트 색처럼 아직 서지 않은 것을 적으면, 시킨 대로 친 명령이 없는 것을 찾는다.
const PROJECTS: &str = r#"    moai project add <dir>                 내 설정에 등록한다 (`.moai` 가 없어도 받는다)
    moai project ls                        등록한 것과 그 상태

`.moai` 밖에서 부른 `moai`·`moai status`·`moai ready` 는 등록한 프로젝트를
프로젝트마다 한눈에 낸다. `--worktree` 를 붙이면 프로젝트마다 옆 워크트리도
겹친다. 그 밖의 명령은 어느 프로젝트인지 모르니
`moai -C <dir> <명령>` 으로 부른다. `moai tui` 에서는 등록한 프로젝트가 맨 위
층으로 선다 — `.moai` 밖이면 거기서 시작하고, 안이면 뿌리에서 Backspace 로 올라간다.
층에서 `SPC p a` 로 디렉터리를 골라 등록하고(모노레포 하위도 따로), `SPC p d` 로 목록에서 뺀다."#;

const PEOPLE: &str = r#"**담당은 저절로 붙는다** — 만든 사람이 담당이다. 남에게 맡기려면
`-a "이름 (메일)"`, 임자 없이 두려면 `-a none`. 이름과 메일은 `git config`
에서 오고, 거기 없으면 `--user "이름 (메일)"` 이나 `MOAI_ACTOR` 로 준다."#;

const CLOSING: &str = r#"`moai status` 를 한 번 더 돌려 경고가 늘지 않았는지 본다. 경고는 막지 않는다 —
에픽 없는 이슈, 오래 멈춘 review, 한 번에 벌여 놓은 것을 비출 뿐이다. 쌓인 idea 와
미뤄 둔 것은 경고가 아니라 알림(`notices`)으로 따로 선다.

집은 채 닫으면 다음 세션이 이어받을 한 줄을 그 이슈에 남긴다. 다음 세션은
`moai show <id>` 의 이력에서 그것을 읽는다.

    moai note <id> "다음: <이어서 할 것>""#;

/// 집은 채 닫을 때 남기는 한 줄. `CLOSING` 과 세션을 닫을 때의 붙듦이 같은
/// 글을 내야 한다 — 안내가 가르친 줄과 훅이 내민 줄이 다르면 둘 다 안 믿는다.
///
/// **`status` 가 이것을 비추지 않는다.** 마지막 note 는 저널을 훑어야 나오는
/// 값이라, 비추는 순간 저널이 `status` 에 읽힌다. `show` 를 한 번 더 치는 것이
/// 실제로 불편해지면 그때 스냅샷 필드를 논의한다 (moai-0rui).
pub fn handoff(id: &str) -> String {
    format!("moai note {id} \"다음: <이어서 할 것>\"")
}

/// 규칙 셋. 제목은 `RULES`, 리뷰 걸음은 `REVIEW_STEPS` 에서 온다.
fn rules() -> String {
    let [one, two, three] = RULES;
    let steps = indent(REVIEW_STEPS, "  ");
    let make = make_review("--parent <보는 이슈>");
    format!(
        r#"**1. {one}.** 집은 이슈 — 첫 칸을 떠났고 아직 안 닫힌 것
(`in_progress`·`review`) — 가 초점이다.
그 일을 하다 나온 것은 같은 에픽 안(`-e <에픽>`)이나 그 일의 자식
(`--parent <id>`)으로 만든다. 지금 할 일이 아니면 `moai idea add` 로 담는다 —
idea 는 이 규칙에서 언제나 자유롭고, `moai add --from` 도 그렇다 (거기서
만들어지는 것은 에픽과 그 자식들이라 그 자체로 한 단위다).

**2. {two}.** `moai mv <id> in_progress`.
세는 것은 저장소 안의 일감뿐이다 — `.moai/`·`.claude/`·`target/` 과 저장소
밖(스크래치패드·임시 파일)은 안 센다. `Edit`·`Write` 뿐 아니라 껍데기로 쓰는
것(`>`·`>>`·`sed -i`·`tee`)도 센다. 계획에 없던 것이면 `moai add "제목"` 으로
세우고 그것을 집는다.

**3. {three}.** `/code-review` 를 부르기 전에 지금 보는 것에 매인 리뷰
이슈를 세운다.

    {make}
{steps}

관점(`-b`)과 닫는 한 줄(`-m`)은 규칙이 **실제로 요구한다.** 없이 부르면
막히고, 거절문이 고칠 명령을 함께 낸다. **사람을 부르지 않는다** — 그 명령을
그대로 부르면 지나간다.

**원문과 판단을 두 노트로 가른다** — 리뷰어가 한 말과 이쪽이 정한 것은 다른
글이다. 넘긴 것은 **이슈 번호와 함께** 적는다. "넘겼다" 만 적힌 줄은 아무도
다시 안 본다. 원문을 어디서 찾는지는 스킬의 `references/commands.md` 에 있다."#
    )
}

/// `init` 이 AGENTS.md 의 마커 사이에 쓰는 블록. **언제나 읽히는 산문이다.**
///
/// 정적이라 `bd prime` 같은 명령을 따로 두지 않는다 — **`moai status` 가
/// prime 이다.**
pub fn agents() -> String {
    format!(
        r#"## 이슈 트래커 — moai

이 저장소의 할 일은 `.moai/issues.jsonl` 에 있다.
TodoWrite 나 마크다운 TODO 목록을 쓰지 않는다. {NO_GATE}

세션을 시작하면 `moai status` 를 먼저 돌린다. 보드와 경고가 한 화면에 나온다.

{CHEATSHEET}

{PEOPLE}

### 갈림길 셋

{FORKS}

### 지금 범위가 아닌 것은 담는다

{IDEAS}

### 이미 있는 일을 지금 안 할 때

{DEFERRING}

### 묶음은 둘이다

{GROUPS}

### 여러 프로젝트

{PROJECTS}

### 기능 요청을 받으면

1. `moai status` 로 이미 있는 에픽을 본다. 겹칠 것 같으면 `moai show -g <키워드>`.
2. 갈림길 3 대로 쪼갠 안을 한 번 보여주고, "좋다" 를 받으면 한 번에 만든다.
3. `moai mv <id> in_progress` 로 집고, 끝나면 `done` 으로 옮긴다.
4. 작업 중 발견한 것 중 지금 범위가 아닌 것은 `moai idea add` 로 담아 둔다.
5. 왜 그렇게 정했는지는 `moai note <id>` 로 이슈에 붙인다. 다음 세션이
   `moai show <id>` 로 그것을 읽는다.

도구가 자라 이 블록이 낡으면 `moai init` 을 다시 부른다. 이슈와 저널은
건드리지 않고 이 블록만 다시 쓴다.

### 훅이 실제로 보는 것 셋

`moai skill install` 로 Claude 에 훅을 심었을 때 선다.

{rules}

### 감독

`moai skill install` 은 둘째 스킬 `moai-supervise` 도 심는다. 같은 저장소에서
놀고 있는 세션들에 쌓인 idea 를 하나씩 나눠 주고 보고를 받을 때 부른다 —
감독은 고르고 보내고 확인할 뿐, 고치거나 병합하지 않는다.

### 세션을 닫기 전에

{CLOSING}
"#,
        rules = rules()
    )
}

/// 스킬의 SKILL.md. **본문은 한 화면이다** — 전체 명령은 `reference` 로 내린다.
///
/// 발동어는 frontmatter 의 `description` 이다. 항상 켜져 있는 비용이 이 한
/// 줄이라, 여기에 낱말을 더하는 것은 모든 세션에 값을 매기는 일이다.
pub fn skill() -> String {
    format!(
        r#"---
name: moai
description: 이 저장소의 할 일·이슈·계획을 다룰 때 쓴다. "뭐부터 할까", "할 일 정리", "이슈 만들어", "진행 상황", "이거 나중에 하자", 기능 요청을 여러 갈래로 쪼갤 때, 또는 작업 중 지금 범위가 아닌 것이 떠올랐을 때. TodoWrite 나 마크다운 TODO 목록 대신 이것을 쓴다.
---

# moai — 이 저장소의 이슈 트래커

할 일은 `.moai/issues.jsonl` 에 있다. {NO_GATE}

{CHEATSHEET} 담당은 만든 사람이 저절로 맡는다.

## 갈림길 셋

{FORKS}

## 훅이 실제로 보는 것 셋

{rules}

## 세션을 닫기 전에

{CLOSING}

전체 명령과 `--from` 문법은 `references/commands.md` 에 있다.
"#,
        rules = rules()
    )
}

/// 스킬의 참고 문서. 부를 때만 읽힌다.
pub fn reference() -> String {
    format!(
        r#"# 전체 명령

`moai --help` 와 `moai <명령> --help` 가 참이다. 이 파일은 그 요약이라
어긋나면 도움말이 이긴다.

## 묶음은 둘이다

{GROUPS}

## 거름망

쉼표는 "또는", 같은 플래그를 두 번 쓰면 "그리고" 다.

    moai show -s todo -t bug          todo 이면서 bug
    moai show -s todo,review          todo 또는 review
    moai show -e none                 에픽 없는 것
    moai show --deferred              미뤄 둔 것만
    moai show --stale 7               지금 칸에 이레 넘게 머문 것
    moai show --tree                  에픽 → 이슈 → 자식

## 워크트리 함께 보기

에이전트가 git 워크트리를 하나씩 잡고 일하면, 지금 워크트리의 보드는 옆에서
집고 옮긴 것을 모른다. `status`·`ready`·`show` 에 `--worktree` 를 붙이면 옆
워크트리의 이슈를 겹쳐 본다. 탐색기(`moai tui`)는 겹친 채로 열고 `SPC t w` 가 끄고 켠다.

    moai ready --worktree             옆에서 집은 일은 빠지고 "잡고 있는 것" 에 선다
    moai status --worktree            보드 머리에 "⎇ <워크트리들> 겹쳐 봄"
    moai show --worktree --json       옆에서 온 줄에만 "branch" 키

같은 id 는 계획에서의 자리를 늦게 바꾼 줄 — 칸을 옮기거나(`status_since`) 미루거나
도로 집은(`planned_at`) 때가 늦은 줄 — 이 서고, 같으면 `updated_at` 이 늦은 줄, 그것도
같으면 지금 브랜치의 줄이다 — 필드만 고친 것은 옆에서 집은 것을 풀지 않고, 옆에서 늦게
미룬 것은 여기서 먼저 집은 칸에 가려지지 않는다.
여기서 `rm` 한 줄은 옆 줄로 되살아나지 않는다 — 갈라진 자리(`git merge-base`)에
있던 줄이면 여기서 지운 것으로 읽는다. 단 옆에서 그 뒤에 집거나 고친 줄은 선다.
메모(`moai note`)는 스냅샷을 안 바꿔 고친 것으로 안 센다 — 옆에서 메모만 남긴 줄은 숨고,
그 메모는 브랜치를 합칠 때 저널에서 드러난다.
지금 브랜치가 아닌 줄은 제목 앞에 `⎇ <브랜치>` 가 붙는다. **보여줄 때만
겹친다** — 어느 파일도 바뀌지 않고, 쓰기(`mv`·`edit`)는 언제나 지금 워크트리
파일에만 간다. 옆 워크트리의 일을 옮기려면 그 워크트리에서 부른다.

## 한 번에 만들기

`#` 줄은 에픽, `-` 줄은 바로 위 에픽의 이슈다. `[pN]` 과 `#태그` 는 없어도 된다.

    moai add --from - <<'MD'
    # 저장 계층
    - [p1] 원자적으로 쓴다 #enhancement
    - 잘린 줄을 복구한다 #bug
    MD

`--dry-run` 이 heredoc 오타로 엉뚱한 여섯 개를 만드는 것을 막는다.

## 여러 프로젝트

{PROJECTS}

## 담아 둔 생각을 펼치기

{IDEAS}

## 미루기

{DEFERRING}

## 사람

{PEOPLE}

## 리뷰가 낸 글을 찾는 법

리뷰 전문은 파일에 남아 있다. 끝났다는 알림에 `task-id` 가 실려 오고, 그것이
곧 파일 이름이다.

    ~/.claude/projects/<프로젝트>/<세션>/subagents/agent-<task-id>.jsonl

리뷰 전문은 **마지막 `text` 블록**이다.

    python3 -c "
    import json,sys
    t=[c['text'] for l in open(sys.argv[1])
       for c in json.loads(l).get('message',{{}}).get('content') or []
       if isinstance(c, dict) and c.get('type') == 'text']
    print(t[-1] if t else '')" <그 파일> | moai note <리뷰 id> -b -

**마지막 줄을 그냥 집지 않는다.** 한 턴의 블록이 줄마다 나뉘어 적히고 생각·
도구 호출도 섞여, 마지막 줄이 글이 아닐 때가 있다. 그러면 빈 글이 넘어가고
`moai note` 가 "메모가 비었다" 로 멈춘다 — 시끄럽게 멈추니 잃지는 않지만,
한 번에 되는 편이 낫다.

**요약만 적고 원문을 버리지 않는다.** 요약은 이쪽의 판단이고 원문은 리뷰어가
한 말이다. 판단은 다시 할 수 있지만 버린 원문은 못 되돌린다.

## 훅

    moai hook <event>    Claude 의 훅이 부른다. 사람이 손으로 부를 일은 없다

`moai skill install` 이 심은 플러그인이 이것을 부른다. 무엇이 어긋나도 종료
코드는 0 이다 — 훅이 시끄러우면 사람이 훅을 꺼 버리고, 꺼진 규칙은 없는
규칙이다.
"#
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
pub fn supervise() -> String {
    r#"---
name: moai-supervise
description: 같은 저장소에서 놀고 있는 Claude 세션들에 쌓인 idea 를 하나씩 나눠 주고 보고를 받을 때 쓴다. "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜" 가 나오면.
---

# moai-supervise — 놀고 있는 세션에 idea 를 나눠 준다

감독은 **고르고, 보내고, 확인한다.** 코드를 고치지 않고, 병합하지 않고, 일꾼
대신 설계를 정하지 않는다. 병합은 일꾼이 하고, 겹치는 병합은 일꾼끼리 먼저
알린다.

## 한 바퀴

**1. 고른다.** 쌓인 idea 에서 지금 벌여 놓은 일과 부딪히지 않는 것만 남긴다.

    moai idea ls                           쌓인 것
    moai show -s in_progress,review        집혀 있는 것
    moai show <id>                         그 idea 가 어디를 건드리는가

`git worktree list` 도 본다. 이미 선 워크트리나 집힌 에픽과 **같은 파일·같은
영역**을 건드리는 idea 는 이번 바퀴에서 뺀다 — 둘이 같은 곳을 고치면 병합에서
한쪽이 다른 쪽을 기다린다.

**2. 일꾼을 찾는다.** `ListAgents` 는 세션의 자리(cwd)를 안 보여 준다.
Claude Code 가 세션마다 적어 두는 `~/.claude/sessions/*.json` 을 읽는다.

    python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" <<'PY'
    import glob, json, os, sys
    root = os.path.realpath(sys.argv[1])
    trees = os.path.join(root, ".claude", "worktrees") + os.sep
    for f in glob.glob(os.path.expanduser("~/.claude/sessions/*.json")):
        try:
            s = json.load(open(f))
            os.kill(s["pid"], 0)
            cwd = os.path.realpath(s["cwd"]) if s.get("cwd") else None
        except (OSError, ValueError, KeyError, TypeError):
            continue
        if cwd is None:
            continue
        if cwd == root:
            print("루트    ", s.get("status"), s.get("name"))
        elif cwd.startswith(trees):
            print("워크트리", s.get("status"), s.get("name"), cwd)
    PY

- **맡기는 것은 자리가 루트이고 `idle`·`waiting` 인 세션뿐이다.** `busy` 는
  일하는 중이고, 그 밖의 값(`shell` 따위)은 뜻을 모르니 맡기지 않는다
- 자리가 `<루트>/.claude/worktrees/*` 인 세션은 이 저장소에서 **일하는 중**이다.
  지켜보되 맡기지 않는다
- **다른 디렉터리의 세션은 건드리지 않는다**
- **맡기기를 거절한 세션은 후보에서 빼고 다시 보내지 않는다.** 제 사람이 준 일만
  받는 세션이 있다 — 한 번 거절했으면 그 뒤로는 알림도 걸지 않는다
- 이 파일은 문서에 없는 속 파일이라 판이 바뀌면 필드가 달라질 수 있다. 못 읽으면
  `ListAgents` 로 이름을 보고, 그 세션에 `pwd` 와 지금 하는 일을 물어 가린다

**3. 보낸다.** 놀고 있는 세션 하나에 idea **하나**를 `SendMessage` 로 보낸다.
일꾼은 이 대화를 모르니 절차를 **통째로** 싣는다.

    감독 세션(<내 이름>)이 idea <id> 를 맡긴다 — <제목>.
    먼저 읽을 것: moai show <id>
    1. main 에서 펼친다 — idea 를 일감으로 바꾸는 길은 `moai idea promote <id> --from -`
       하나다. 이슈 하나짜리여도 에픽 + 이슈로 펼친다. `--dry-run` 을 먼저 본다
    2. 멤버를 `moai mv <멤버> in_progress` 로 집고 main 에
       "chore(tracker): <에픽> 를 워크트리에서 집는다" 로 커밋한다.
       main 에서 커밋하기 전에는 언제나 `git status` 를 본다 — 아래 "공유 main" 을 따른다
    3. `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> main` 으로
       로컬 main 에서 뜨고 EnterWorktree(path) 로 들어간다. 이름은 idea id 가
       아니라 펼친 에픽 id 다
    4. 노트에 없는 설계 결정은 추측하지 말고 AskUserQuestion 으로 묻는다 —
       사람이 일꾼 창을 보고 있다
    5. 리뷰 이슈를 세워(규칙 3) `/code-review high --fix`. 반영은 별도 fix: 커밋,
       넘긴 것은 이슈 번호와 함께 노트.
       워크트리에서 `/code-review` 가 규칙 3 에 막히면 — 지금 훅은 그 워크트리의
       스냅샷만 읽어 main 에서 집은 리뷰 이슈를 못 본다(moai-iz38, 에픽 moai-wofj
       에서 고치는 중) — 같은 관점·단계·`--fix` 범위로 리뷰 서브에이전트를 돌린다.
       리뷰 이슈·원문 노트·닫는 `-m` 은 그대로 남긴다
    6. 멤버의 일이 다 끝나면 병합 전에 에픽 전체를 `/code-review max --fix` 로 본다 —
       가지가 main 을 떠난 자리(`git merge-base main HEAD`)부터의 diff 다. 리뷰
       이슈를 따로 세운다(`-t review --parent <에픽>`). 막히면 5 의 길로 간다
    7. 워크트리에서 main 을 받아 충돌을 푼다
    8. ExitWorktree(keep) 로 루트로 돌아온다 — 워크트리 안에서 그것을 지우면 세션의
       자리가 사라진 디렉터리에 남아 감독이 다시는 이 세션을 루트로 못 본다.
       옆 세션과 병합이 겹치면 먼저 알린 뒤 루트에서 `git merge worktree-<에픽> -m "merge: …"`
       로 **한 번에** 병합한다 — `--no-commit` 으로 열어 두지 않는다.
       **`moai mv <멤버> done` 은 그 병합이 실제로 끝난 뒤에만 친다** — 병합 전에
       옮겼다가 되돌린 일꾼이 있었다. 시험 통과를 보고 멤버·리뷰 이슈를 done 으로 커밋
    9. 루트에서 `git worktree remove .claude/worktrees/<에픽>` 과
       `git branch -d worktree-<에픽>` 으로 워크트리와 가지를 지운다
    10. SendMessage to "<내 이름>" 로 보고 — 머지 해시, 한두 줄 요약, 넘긴 것·새 idea

### 공유 main

루트 체크아웃은 **모든 세션이 같이 쓴다.** 한 세션이 `git merge --no-commit` 으로
병합을 열어 둔 사이에 다른 세션이 트래커 노트를 커밋하면, 그 커밋이 남의 병합을
제 제목으로 봉인한다 — 실제로 그렇게 됐다. 그래서 감독이든 일꾼이든, 트래커 노트
하나라도 main 에서 `git commit` 하기 전에:

- `git status` 를 본다. `.git/MERGE_HEAD` 가 있거나 "still merging" 이면 **커밋하지
  않는다.** 그 병합을 연 세션이 끝낼 때까지 기다린다
- 제 병합은 `git merge <가지> -m "…"` 한 번으로 끝낸다. `--no-commit` 을 쓰지 않는다

**4. 기다린다.** 일하는 세션에는 메시지 없이 `notify_when_idle: true` 로
걸어 둔다. **`ListAgents` 를 되풀이해 훑지 않는다** — 알림이 온다.

**5. 보고를 확인하고 다음을 보낸다.** 보고를 믿기 전에 셋을 본다.

    git log --oneline main                 머지 해시가 main 에 있는가
    moai show <id>                         펼친 에픽과 멤버가 done 인가
    git worktree list                      그 워크트리가 사라졌는가

셋이 맞으면 그 세션에 다음 idea 를 보낸다. 어긋나면 그 세션에 무엇이 남았는지
묻고, 대신 끝내지 않는다.

## 멈출 때

- 부딪히지 않는 idea 가 없거나 놀고 있는 세션이 없으면 사람에게 그렇게 말하고
  멈춘다 — 부딪히는 idea 를 억지로 보내지 않는다
- 일꾼이 사람의 결정을 기다리면 감독이 대신 답하지 않는다. 결정은 사람의 것이다
"#
    .to_string()
}

/// 줄마다 앞에 붙인다. 빈 줄은 빈 채로 둔다 — 꼬리 공백은 diff 를 더럽힌다.
fn indent(text: &str, by: &str) -> String {
    text.lines()
        .map(|l| if l.is_empty() { String::new() } else { format!("{by}{l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **공통 조각은 두 표면에 똑같이 든다.** 한쪽만 고치면 여기서 붉어진다 —
    /// 조각을 거치지 않고 표면에 글을 직접 적는 순간 두 벌이 다시 생긴다.
    #[test]
    fn both_surfaces_carry_the_same_pieces() {
        let (agents, skill, reference) = (agents(), skill(), reference());
        for piece in [CHEATSHEET, FORKS, NO_GATE, CLOSING] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(skill.contains(piece), "스킬에 없다 — {head}");
        }
        assert!(CLOSING.contains(&handoff("<id>")), "안내의 핸드오프 줄이 훅과 갈라졌다");
        let rules = rules();
        assert!(agents.contains(&rules) && skill.contains(&rules), "규칙 셋이 갈라졌다");
        for piece in [GROUPS, IDEAS, DEFERRING, PEOPLE, PROJECTS] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(reference.contains(piece), "참고 문서에 없다 — {head}");
        }
    }

    /// 규칙의 이름이 스킬에 그대로 선다. 훅의 거절문 쪽은 `hook` 의 시험이 본다.
    #[test]
    fn the_skill_names_each_rule_as_the_hook_does() {
        let skill = skill();
        for n in 1..=3 {
            let title = format!("**{n}. {}.**", RULES[n - 1]);
            assert!(skill.contains(&title), "스킬에 규칙 {n} 의 이름이 없다 — {title}");
        }
        assert!(skill.contains(&make_review("--parent <보는 이슈>")), "리뷰를 세우는 줄이 갈라졌다");
        for step in REVIEW_STEPS.lines() {
            assert!(skill.contains(step.trim()), "리뷰 걸음이 갈라졌다 — {step}");
        }
    }

    /// 포맷 문자열 안의 `{{`·`}}` 가 제대로 풀렸는가. 참고 문서의 파이썬 한 줄이
    /// 딕셔너리를 쓰므로, 한 번 틀리면 복사해 친 명령이 문법 오류로 죽는다.
    #[test]
    fn the_python_one_liner_survives_formatting() {
        assert!(reference().contains(".get('message',{}).get('content')"));
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
    #[test]
    fn the_supervisor_teaches_promote_as_the_one_way() {
        let (supervise, reference) = (supervise(), reference());
        let promote = "moai idea promote <id> --from -";
        assert!(reference.contains(promote), "참고 문서의 펼치기 줄이 바뀌었다");
        assert!(supervise.contains(promote), "감독이 promote 를 안 가르친다");
        assert!(!supervise.contains("moai add "), "감독이 promote 말고 다른 길을 가르친다");
    }

    /// **첫 실행에서 일꾼들이 실제로 걸려 넘어진 세 자리가 절차에 선다.** 리뷰가
    /// 워크트리에서 막힐 때의 길, 병합 뒤에만 done, 에픽 끝의 max 리뷰 — 셋 중
    /// 하나라도 빠지면 다음 일꾼이 같은 자리에서 또 넘어진다.
    #[test]
    fn the_worker_brief_carries_what_the_first_run_tripped_on() {
        let supervise = supervise();
        for (piece, why) in [
            ("리뷰 서브에이전트", "워크트리에서 /code-review 가 막힐 때의 길이 없다"),
            ("moai-iz38", "막히는 까닭을 가리키는 이슈가 없다"),
            ("병합이 실제로 끝난 뒤에만", "병합 전에 done 으로 옮기지 말라는 말이 없다"),
            ("/code-review max --fix", "에픽 끝의 max 리뷰가 없다"),
            ("--parent <에픽>", "max 리뷰 이슈를 에픽에 매는 줄이 없다"),
            ("ExitWorktree(keep)", "루트로 돌아오는 걸음이 없다"),
            ("MERGE_HEAD", "남이 열어 둔 병합을 봉인하지 말라는 말이 없다"),
            ("`--no-commit` 을 쓰지 않는다", "병합을 한 번에 끝내라는 말이 없다"),
            ("거절한 세션", "맡기기를 거절한 세션을 빼라는 말이 없다"),
        ] {
            assert!(supervise.contains(piece), "{why} — {piece}");
        }
    }
}
