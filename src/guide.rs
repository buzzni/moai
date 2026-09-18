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
    moai read <id>                         읽었다고 적는다 — [NEW] 가 내린다. --all 은 내게 온 것 전부
    moai defer <id> -m "왜"                지금 안 할 일을 계획에서 뺀다

모든 명령에 `--json` 이 붙는다. `ready --json` 은 `{"ready":[…],"held":[…]}` —
`held` 는 미뤄 둔 것·빈 묶음에 막혀 못 집는 일과 도로 집을 곳이다.
그것으로 사람 없이 도는 고리를 짤 수 있다. moai 저장소의 `examples/bash-agent/agent.sh`
가 bash 와 jq 만으로 집고·일하고·닫는 한 벌이다.

**여럿이 같은 저장소에서 돌면 `moai mv <id> in_progress --from todo` 로 집는다.**
본 칸이 그대로일 때만 옮기므로, 옆에서 먼저 집은 일을 뒤늦게 덮지 않는다 — 진
쪽은 stderr 한 줄(`--json` 이면 `stale`)과 0 아닌 종료 코드를 받고 다음 일로 간다.
`defer` 도 같은 `--from` 을 받는다. 안 쓰면 지금까지처럼 아무것도 막지 않는다.
**id 를 하나만 준다** — 여럿을 한 번에 주면 이긴 것과 진 것이 한 종료 코드에
섞여, 이긴 줄을 집어 놓고 버린다."#;

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

```sh
moai add --from - <<'PLAN'
# 에픽 제목
- [p1] 첫 이슈 #enhancement
- [p2] 둘째 이슈
PLAN
```"#;

/// 에이전트가 이슈에 적는 글의 모양(moai-j8aq). **권고다** — 어겨도 아무것도 막히지 않는다.
/// 훅이 이것을 검사하지 않는 것은 결정이다(사용자, moai-mthy): 글 스타일 검사는 린트이고,
/// 린트는 곧 게이트다.
const WRITING: &str = r#"**제목과 본문은 따로 넘긴다.** 제목은 인자로 주고, 본문은 마크다운으로 적어
`-b -` 로 stdin 에서 흘린다 — heredoc 이 편하다. 제목을 본문에 다시 적지 않는다.

- **제목은 짧게.** 무엇이 어긋났는지 한 줄이다. 보드와 `ready` 와 탐색기 목록은 제목만 보여 준다
- **본문은 서술형으로 적지 않는다.** 겪은 일을 문단으로 늘어놓는 대신 목록으로 가른다 —
  무엇이 어긋났는가, 무엇을 봤는가, 어디를 고치는가
- **이모지를 쓰지 않는다** — 제목에도 본문에도. 터미널마다 폭이 달라 보드와 표가 어긋난다

지킬 것은 다음 세션이 `moai show <id>` 로 읽는다는 것 하나다. 이 셋은 그래서 있는 권고이지
검사하는 규칙이 아니다."#;

/// 글 스타일의 예시(moai-1xf2). **참고 문서에만 둔다** — AGENTS 블록과 SKILL.md 는 언제나
/// 읽히는 자리라 예시 한 벌이 모든 세션의 값이 된다. 규칙은 짧게 늘 보이고, 예시는 부를 때 온다.
///
/// **예시는 moai 의 동작을 주장하지 않는다.** 이 글은 모든 저장소에 심긴다 — 읽는 쪽의 코드를
/// 두고 쓴 예시라야 어디서 읽어도 뜻이 서고, 이 저장소의 파일 이름을 박으면 남의 저장소에서는
/// 아무것도 안 가리킨다. 첫 판은 `moai edit --tag` 가 빈 태그를 그대로 쓴다고 적었는데
/// (`src/cmd/edit.rs` 와 `query::split_tags` 는 처음부터 빈 낱말을 걸렀다) 없는 버그를
/// 예시로 내민 셈이었다. 자리는 `<파일>:<줄>` 처럼 자리 표시로 둔다.
const WRITING_EXAMPLE: &str = r#"규칙은 `SKILL.md` 의 "이슈에 적는 글" 에 있다. 여기는 그것을 지킨 한 벌이다.
제목은 인자로, 본문은 `-b -` 로 넘긴다.

```sh
moai add "빈 태그를 못 걸러 필터가 전부를 낸다" -t bug -e <에픽> -b - <<'BODY'
- 무엇: 태그를 정규화할 때 빈 낱말이 그대로 남는다
- 무엇을 봤나: 그 태그로 거른 목록이 아무것도 안 거르고 전부를 낸다
- 어디: 태그를 정규화하는 자리(<파일>:<줄>). 빈 낱말을 거르면 끝난다
BODY
```

흔한 어긋남 셋.

- **제목에 겪은 일을 다 적는다** — "어제 …하다가 …해서 …인 것 같은데 확인이 필요함".
  보드와 `ready` 는 그 줄을 잘라 내고, 자른 앞쪽에는 대개 무엇이 어긋났는지가 없다
- **본문을 문단으로 적는다** — 다음 세션은 그 문단에서 "어디를 고치는가" 를 다시 찾아야 한다.
  판단과 근거와 다음 걸음을 줄로 가르면 `moai show` 한 번으로 끝난다
- **이모지로 급한 것을 알린다** — 급한 것은 우선순위(`-p 1`)로 적는다. `ready` 가 읽는 것은 그쪽이다"#;

const IDEAS: &str = r#"    moai idea add "반짝 떠오른 것"                 담기
    moai idea add "긴 생각" -b -                   본문은 stdin 에서
    moai idea ls                                   쌓인 것 보기
    moai show -g <키워드>                          이미 적어 뒀는지 찾기

때가 되면 하나를 에픽과 이슈로 펼친다. 펼치면 그 생각은 닫힌다.

```sh
moai idea promote <id> --from - <<'PLAN'
# 에픽 제목
- [p1] 첫 이슈 #enhancement
PLAN
```"#;

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
층으로 선다 — `.moai` 밖이면 거기서 시작하고, 안이면 그 프로젝트 안에서 시작한다.
맨 위 헤더가 프로젝트마다 번호를 대고, 그 숫자를 그대로 누르면 바로 옮겨 간다 — `0` 이 층이다.
층에서 `SPC p a` 로 디렉터리를 골라 등록하고(모노레포 하위도 따로), `SPC p d` 로 목록에서 뺀다.
등록한 것이 없으면 밖에서 띄워도 빈 층이 서서 `SPC p a` 를 댄다."#;

/// 커밋과 이슈를 잇는 고리(moai-wqm7). **새 저장소는 이 저장소의 CLAUDE.md 규약을 모른다** —
/// 여기 안 적으면 커밋 칸(`show <id>`·탐색기 상세)이 늘 빈다.
///
/// **예시 제목의 id 는 `<id>` 로 둔다.** 이 글은 모든 저장소에 심긴다 — 이 저장소의 이슈 id 를
/// 박으면 남의 저장소에서는 아무것도 안 가리키고, 접두사가 다른 저장소에 `moai-` 를 가르친다.
const COMMITS: &str = r#"커밋 제목에 그 커밋이 닿은 이슈 id 를 적는다 — `feat: 막음 줄을 그린다 (<id>)`.
moai 는 커밋을 이슈에 저장하지 않는다. `moai show <id>` 와 탐색기 상세가 **커밋 제목에 적힌
id** 로 그 이슈의 커밋을 그때그때 찾아 낸다. 해시를 노트에 옮겨 적지 않는다 — squash·rebase
한 번에 낡고, 이슈를 닫는 커밋은 제 해시를 미리 모른다.

- **제목에** 적는다. 본문은 `Refs:`·`Closes:`·`Fixes:` 로 시작하는 줄만 센다 — 그 밖의 본문에
  적힌 id 는 안 센다(트래커 커밋이 줄줄이 대는 id 가 죄다 그 이슈의 커밋으로 서면 안 된다)
- **squash 로 합치는 저장소는 트레일러를 단다.** squash 는 합친 커밋들의 제목을 본문으로 옮겨
  버려, 제목만으로는 그 커밋들이 통째로 사라진다 — `Refs: <id>` 한 줄이 그것을 지킨다
- id 는 낱말째 맞춘다. 자식(`<id>.x1y`)의 커밋은 부모의 것이 아니다 — 리뷰를 반영한 커밋은
  리뷰 이슈 id 로 그 리뷰에 붙고, 워크트리를 합치는 `merge: … (<id>)` 가 그 일에 붙는다
- 집기·닫기만 담는 커밋은 `chore(tracker):` 로 시작한다. 상세는 그것을 빼고 그린다
  (`--json` 의 `commits` 에는 `tracker` 표시와 함께 남는다)
- `moai show <id> --json` 의 `commits` 는 **늘 있다.** 빈 배열은 "그 id 를 적은 커밋이 없다" 는
  뜻이고, git 을 못 읽었을 때만 `commits_error` 가 그 까닭을 한 줄로 댄다 — 기계가 "아직 아무도
  안 고쳤다" 와 "여기서는 못 물어봤다" 를 가르라고 둔 것이다"#;

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

### 이슈에 적는 글

{WRITING}

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

### 커밋에 id 를 적는다

{COMMITS}

도구가 자라 이 블록이 낡으면 `moai init` 을 다시 부른다. 이슈와 저널은
건드리지 않고 이 블록만 다시 쓴다. 낡았는지만 보려면 `moai init --check` —
아무것도 안 쓰고 `current`·`stale`·`missing` 으로 답한다.

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

{CHEATSHEET}

담당은 만든 사람이 저절로 맡는다.

## 갈림길 셋

{FORKS}

## 이슈에 적는 글

{WRITING}

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

## 겨루는 집기 — `mv`·`defer` 의 `--from <칸>`

같은 `.moai` 를 여럿이 쓰면 `ready` 와 `mv` 사이에 옆 세션이 같은 줄을 집는다.
`--from <칸>` 은 **본 칸이 그대로일 때만** 옮긴다 — 락 안에서 다시 보므로 그
사이에 달라진 줄은 건드리지 않는다. 안 쓰면 지금까지처럼 아무것도 막지 않는다.

부르는 모양은 moai 저장소의 `examples/bash-agent/agent.sh` 가 끝까지 보여 준다. 줄여 적으면
이렇다 — `col` 은 `ready` 가 낸 그 줄의 칸이고, 집었는지는 `moved` 가 말한다.

```sh
row=$(moai ready --json | jq -c '.ready[0]')
id=$(jq -r '.id' <<<"$row")
col=$(jq -r '.status' <<<"$row")
claim=$(moai mv "$id" in_progress --json --from "$col") || {{
  [ -n "$claim" ] || exit 1          # stdout 이 비면 겨루기가 아니라 실패다
  continue                           # 졌다 — 다음 일로
}}
[ "$(jq '.moved | length' <<<"$claim")" -gt 0 ] || continue
```

- **id 는 하나씩 준다.** 여럿을 한 번에 주면 이긴 줄과 진 줄이 **한 종료 코드**에
  섞인다 — 이긴 줄은 이미 옮겨졌는데 부르는 쪽은 아무것도 못 집은 줄 안다
- 진 줄은 stderr 한 줄(`moai: <id> 는 이미 <칸> 다 — …`)로 서고 `--json` 에는
  `stale: [{{"id":…,"status":<서 있는 칸>}}]` 로 선다. stdout 의 `이미 <칸> 다` 는
  **다른 말이다** — 그쪽은 이미 갈 칸에 있던 줄이고 종료 코드가 0 이다
- **0 아닌 코드가 전부 "졌다" 는 아니다.** 신원 없음·락 걸림·칸 오타·깨진 줄도
  같은 코드로 온다. 겨루다 진 것은 줄을 stdout 에 내고, 실패는 `{{"code":…}}` 를
  stderr 에 내며 stdout 이 빈다 — 실패를 "남이 집었다" 로 읽으면 같은 줄이 다시
  나와 엉뚱한 까닭을 대며 멈춘다
- **`--from` 이 맞았다고 집은 것은 아니다.** 첫 칸이 곧 집는 칸인 설정에서는 칸이
  맞고도 아무것도 안 옮기고 `already` 로 0 을 낸다 — `moved` 를 함께 본다
- **묶음(에픽·마일스톤)에는 `--from` 을 못 쓴다** — 거절한다(`bad_status`). 묶음의
  칸은 멤버에서 읽고 쓰기는 줄에 적힌 칸(미루기면 `deferred_at`)에 하므로, 재는 축과
  쓰는 축이 갈려 겨루는 둘이 **다 이긴다.** 먹는 척하는 가드가 없는 가드보다 나쁘다 —
  멤버를 집거나, 묶음은 `--from` 없이 `moai defer` 로 접는다
- `defer` 의 `--from` 도 **칸**을 본다. 미루기는 칸을 안 바꾸므로, 옆에서 집어 칸이
  움직인 줄은 걸러 내지만 **둘 다 미루는** 겨루기는 이것으로 안 갈린다
- `--from` 은 **아는 칸이거나 어느 줄이 실제로 선 칸**을 받는다. 아무도 안 선 이름은
  거절한다(`bad_status`) — 오타를 "안 맞았다" 로 읽으면 아무것도 안 하면서 코드만
  내, 부르는 쪽이 까닭을 못 읽는다. 거절이 노리는 것은 오타지 낡음이 아니라서,
  `config` 에서 칸 이름을 고친 뒤에도 옛 이름에 선 줄은 그 이름으로 집을 수 있다

## 한 번에 만들기

`#` 줄은 에픽, `-` 줄은 바로 위 에픽의 이슈다. `[pN]` 과 `#태그` 는 없어도 된다.
제목이 `[` 로 시작하거나 끝에 `#낱말` 이 붙으면 `\[`·`\#` 로 적는다 (`- \[WIP] 이슈 \#12`).

```sh
moai add --from - <<'PLAN'
# 저장 계층
- [p1] 원자적으로 쓴다 #enhancement
- 잘린 줄을 복구한다 #bug
PLAN
```

`--dry-run` 이 heredoc 오타로 엉뚱한 여섯 개를 만드는 것을 막는다.

되풀이하는 계획은 파일로 두고 `{{{{이름}}}}` 을 `--var 이름=값` 으로 채운다(이름은 영문·숫자·`_`·`-`,
빈칸 없이). 저장소의 `.moai/templates/<이름>.md` 에 두는 것이 관례고 `--from <경로>` 로 부른다.
변수는 전부 필수다 — 못 채운 이름·빈 값·줄바꿈이 든 값·계획에 없는 이름·같은 이름 두 번은 거절하고
아무것도 안 만든다. 값은 적힌 그대로 제목 글자가 되므로 변수는 제목 자리에만 둔다(태그·우선순위
자리면 거절). 템플릿의 제목에 글자 `{{{{` 를 쓰려면 `\{{{{` 로 적는다. `{{{{` 바로 앞의 역슬래시는 쌍으로 센다 —
글자 역슬래시 뒤에 변수를 두려면 `\\{{{{이름}}}}` 이다(`C:\\{{{{dir}}}}`). `idea promote --from` 도 같은
`--var` 를 받는다.

    moai add --from .moai/templates/release.md --var version=1.2 --dry-run

## 여러 프로젝트

{PROJECTS}

## 담아 둔 생각을 펼치기

{IDEAS}

## 미루기

{DEFERRING}

## 사람

{PEOPLE}

## 이슈에 적는 글 — 예시

{WRITING_EXAMPLE}

## 커밋에 id 를 적는다

{COMMITS}

## 리뷰가 낸 글을 찾는 법

리뷰 전문은 파일에 남아 있다. 끝났다는 알림에 `task-id` 가 실려 오고, 그것이
곧 파일 이름이다.

    ~/.claude/projects/<프로젝트>/<세션>/subagents/agent-<task-id>.jsonl

리뷰 전문은 **마지막 `text` 블록**이다.

```sh
python3 -c "
import json,sys
t=[c['text'] for l in open(sys.argv[1])
   for c in json.loads(l).get('message',{{}}).get('content') or []
   if isinstance(c, dict) and c.get('type') == 'text']
print(t[-1] if t else '')" <그 파일> | moai note <리뷰 id> -b -
```

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

**git 훅 안에서 불러도 된다.** git 훅과 딸린 워크트리의 `rebase -x` 는 `GIT_DIR`
무리를 내보내고 그것은 `git -C <경로>` 를 이기지만, moai 는 git 을 띄우기 전에
그 변수들을 걷는다. 그래서 어느 **저장소**를 읽는지는 `-C` 나 `.moai` 찾기가
정한다 — 저장소 A 의 훅에서 부른 `moai -C B` 의 커밋 칸과 `--worktree` 는 B 의
이력만 읽는다.

**사람도 B 에서 온다.** 바깥이 `git -c user.name=…` 으로 부른 훅이 물려주는 값도
걷는다 — `-C` 로 남의 프로젝트에 적을 때 그 값이 따라가면 B 의 저널에 남의 이름이
영구히 남는다. 그래서 A 의 저장소 설정에만 있는 이름은 이제 안 읽힌다: B 에도
전역에도 이름이 없으면 쓰기가 사람을 못 찾아 멈추고, 훅 안에서 멈추면 그 커밋이
실패한다. 그 자리에서는 `--user "이름 (메일)"` 이나 `MOAI_ACTOR` 로 못 박는다.

**어느 디렉터리에서 부르든 같다.** 사람도 커밋 칸도 그 트래커의 `.moai` 뿌리에서
읽는다 — 프로젝트 안에 다른 저장소가 겹쳐 있어도(서브모듈·`vendor`) 거기서 부른
`moai` 가 그 저장소의 이름을 적지 않는다.
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
///
/// **모든 저장소에 심긴다.** 이 저장소의 이슈 id 를 글에 적지 않고, 가지 이름을 박지
/// 않는다 — 감독이 루트 체크아웃의 지금 가지를 읽어 `<본 가지>` 에 채운다(사용자 결정,
/// moai-7ljm). 일꾼이 뜨고 병합하는 곳이 그 체크아웃이라 원격 기본 가지보다 덜 어긋난다.
/// **다만 틈은 남는다** — 루트가 detached 이거나 바퀴 사이에 루트의 가지가 바뀌면, 일꾼의
/// 루트 커밋·병합은 루트 HEAD 로 가고 워크트리는 `<본 가지>` 에서 뜬다(moai-dubz).
/// 워크트리 안의 규칙 3 이 루트에서 집은 리뷰를 못 보는 것은 moai-iz38(에픽 moai-wofj)이지만,
/// 남의 저장소에서 그 id 는 아무것도 안 가리키고 고친 뒤에는 거짓이 된다.
pub fn supervise() -> String {
    let brief = brief();
    format!(
        r#"---
name: moai-supervise
description: 같은 저장소에서 놀고 있는 Claude 세션들에 쌓인 idea 를 하나씩 나눠 주고 보고를 받을 때 쓴다. "감독해 줘", "idea 나눠 줘", "놀고 있는 세션에 일 시켜" 가 나오면.
---

# moai-supervise — 놀고 있는 세션에 idea 를 나눠 준다

감독은 **고르고, 보내고, 확인한다.** 코드를 고치지 않고, 병합하지 않고, 일꾼
대신 설계를 정하지 않는다. 병합은 일꾼이 하고, 겹치는 병합은 일꾼끼리 먼저
알린다.

**본 가지는 바퀴를 시작할 때 한 번 읽는다.** 일꾼이 워크트리를 뜨고 병합하는
곳이 루트 체크아웃이라 그 체크아웃의 지금 가지가 본 가지다 — 원격의 기본 가지는 루트와
다를 수 있고 낡았을 수 있다. 루트가 detached 면 `origin/HEAD`, 그것도 없으면 `main` 이다.
루트 체크아웃은 `git worktree list` 의 첫 자리라, 아래 한 줄은 저장소 어디서 불러도 —
워크트리 안에서도 — 루트의 가지를 낸다. 아무것도 안 나오면 git 이 낸 오류를 보고 멈춘다.

```sh
if w=$(git worktree list --porcelain); then b=$(printf '%s\n' "$w" | sed -n '1,/^$/s|^branch refs/heads/||p'); [ -n "$b" ] || b=$(git symbolic-ref -q refs/remotes/origin/HEAD | sed 's|^refs/remotes/origin/||'); echo "${{b:-main}}"; fi
```

읽은 이름을 아래 명령의 `<본 가지>` 와 일꾼에게 싣는 글의 `<본 가지>` 에 채운다.
**일꾼은 다시 읽지 않는다** — 워크트리 안에서 읽으면 제 가지가 나온다.

## 한 바퀴

**0. 먼저 거둔다 — 자리를 잃은 일.** 세션이 죽으면 집은 줄은 `in_progress` 로 남고
아무도 이어 하지 않는다. 새 idea 를 고르기 전에 본다.

    moai status --json                     warnings 에서 kind 가 "stranded" 인 것의 ids
                                           (워크트리 안이면 `--worktree` 를 붙여야 선다)
    moai show <id>                         `자리` 줄 — 아래 네 낱말 중 하나
                                           (워크트리 안이면 여기도 `--worktree` 가 있어야 선다)

`stranded` 는 집었는데 살아 있는 워크트리가 그 일을 안 쥔 줄이다 — 워크트리가 사라졌거나,
**워크트리 없이 루트에서 하던 일**이다. 둘은 이 줄만으로 안 갈린다: 2 의 세션 목록에서 루트에
산 세션이 있으면 그쪽일 수 있으니, 맡기기 전에 그 세션에 무엇을 쥐고 있는지 묻는다.

`자리` 줄(`--json` 의 `place`)은 넷이다. **맡기는 것은 `없다` 하나뿐이다.**

    <경로> (<가지>)   at        거기서 돈다. 들어가 이어 한다
    아직 안 보인다     fresh     방금 집었다 — 일꾼이 워크트리를 띄우는 틈이다. 그냥 둔다
    모른다             unknown   **못 읽은 옆 워크트리가 있다.** 거기일 수 있으니 안 맡긴다
    없다               lost      자리를 잃었다 — 이것만 거둔다

**`stranded` 가 조용한 것이 곧 거둘 것이 없다는 뜻은 아니다.** 못 읽는 옆 스냅샷이 하나라도
있으면 자리 판정이 통째로 `모른다` 로 접혀 이 경고가 저장소째로 잠긴다. `status --json` 의
`unreadable_worktrees` 키(사람 화면은 `옆 워크트리 문제 N건`)가 서 있으면 **그 워크트리를 먼저
고치고 다시 본다** — 그 전에 읽은 빈 목록은 "없다" 가 아니라 "못 셌다" 다.

집은 지 한 시간이 안 된 줄은 안 뜬다(일꾼이 워크트리를 띄우는 틈이다). **워크트리는
남았는데 거기서 일하던 세션이 죽은 것은 `stranded` 에 안 뜬다** — `git worktree list` 의
워크트리 중 2 의 스크립트에 `워크트리` 줄로 안 나오는 것이 그것이다.

- 그런 일이 있으면 **새 idea 보다 먼저** 놀고 있는 세션 하나에 이어 하기를 맡긴다. 3 의
  글 대신 아래를 싣고, 그 뒤에 3 의 글의 **4-1 부터 11 까지**를 통째로 잇는다 — 4-1 을 빼면
  이어받은 일꾼이 워크트리 안에서 트래커를 고친다

      감독 세션(<내 이름>)이 <에픽> 의 멈춘 일을 맡긴다 — 앞 세션이 끝을 못 냈다.
      먼저 읽을 것: moai show <에픽> (이력·노트) · moai show <멤버> (자리도 — 자리는 일에만 선다)
      본 가지: <본 가지> — 감독이 루트에서 읽어 채웠으니 다시 읽지 않는다
      루트: <루트> — 2 의 `루트 자리`. 트래커를 고치는 것은 언제나 이 자리다(3 의 글의 4-1)
      - 워크트리가 있으면 EnterWorktree(path) 로 들어가 `git log <본 가지>..HEAD` 와
        `git status` 로 어디까지 했는지 읽고 이어 한다
      - 없으면 루트에서 다시 뜬다. 가지가 남아 있으면 그 가지로
        (`git worktree add .claude/worktrees/<에픽> worktree-<에픽>`), 없으면
        `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> <본 가지>`
      - 멤버의 칸은 이미 집혀 있다 — 다시 집지 않는다
      - 아래 걸음들이 가리키는 `2` 는 **경로를 준 트래커 커밋**이다 — 루트는 모든 세션이
        같이 쓰니 `git commit -m "…" -- .moai/` 로 친다. 병합이 열려 있으면(MERGE_HEAD)
        git 이 거절하니 그 병합이 끝나기를 기다렸다 다시 친다

- **이어 할지 놓을지는 감독이 정하지 않는다.** 놓을 일로 보이면(`moai mv <id> todo`·
  `moai defer <id> -m "왜"`) 사람에게 묻는다
- 이어 하기를 맡긴 일은 idea 와 같이 그 보고를 확인할 때까지 다시 안 보낸다

**1. 고른다.** 쌓인 idea 에서 지금 벌여 놓은 일과 부딪히지 않는 것만 남긴다.

    moai idea ls                           쌓인 것
    moai show -s in_progress,review        집혀 있는 것
    moai show <id>                         그 idea 가 어디를 건드리는가

`git worktree list` 도 본다. 이미 선 워크트리나 집힌 에픽과 **같은 파일·같은
영역**을 건드리는 idea 는 이번 바퀴에서 뺀다 — 둘이 같은 곳을 고치면 병합에서
한쪽이 다른 쪽을 기다린다. **이번 바퀴에 함께 보내는 idea 끼리도 견준다** — 일꾼은
받은 뒤에야 워크트리를 세우니, 방금 보낸 것은 아직 위 목록에 안 뜬다. 다음 idea 를
보낼 때도 이 셈을 다시 한다.

**보낸 idea 는 그 보고를 확인할 때까지 후보에서 뺀다.** 일꾼이 펼치기 전까지는
`moai idea ls` 에 그대로 남아, 둘째 일꾼에게 같은 idea 가 또 간다.

**2. 일꾼을 찾는다.** `ListAgents` 는 세션의 자리(cwd)를 안 보여 준다.
Claude Code 가 세션마다 적어 두는 `~/.claude/sessions/*.json` 을 읽는다
(`CLAUDE_CONFIG_DIR` 를 옮겼으면 그 아래다). 모노레포의 하위 프로젝트면 `.moai` 가
있는 그 하위가 루트다. 스크립트는 첫 줄 `루트 자리` 에 그 `<루트>` 를 낸다.

```sh
python3 - "$(git worktree list --porcelain | sed -n 's/^worktree //p' | head -1)" "$(git rev-parse --show-toplevel)" <<'PY'
import glob, json, os, sys
if not sys.argv[1]:
    sys.exit("git 저장소 안에서 부른다")
top, here = os.path.realpath(sys.argv[2]), os.path.realpath(os.getcwd())
while here not in (top, os.path.dirname(here)) and not os.path.isdir(os.path.join(here, ".moai")):
    here = os.path.dirname(here)
root = os.path.realpath(os.path.join(sys.argv[1], os.path.relpath(here, top)))
trees = os.path.join(root, ".claude", "worktrees") + os.sep
print("루트 자리", root)
home = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
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
    elif cwd == root:
        print("루트    ", s.get("status"), s.get("name"))
    elif cwd.startswith(trees):
        print("워크트리", s.get("status"), s.get("name"), cwd)
if unread:
    print("못 읽은 파일", unread)
PY
```

- **맡기는 것은 자리가 루트이고 `idle`·`waiting` 인 세션뿐이다.** `busy` 는
  일하는 중이고, 그 밖의 값(`shell` 따위)은 뜻을 모르니 맡기지 않는다
- **보낸 idea 의 보고를 아직 확인하지 않은 세션은 뺀다.** 일꾼은 펼치고 집고 병합하는
  동안 루트에 있다 — 사람의 답이나 권한을 기다리면 `waiting`, 턴을 마치면 `idle` 로 뜬다
- 자리가 `<루트>/.claude/worktrees/*` 인 세션은 이 저장소에서 **일하는 중**이다.
  지켜보되 맡기지 않는다
- **다른 디렉터리의 세션은 건드리지 않는다**
- **맡기기를 거절한 세션은 후보에서 빼고 다시 보내지 않는다.** 제 사람이 준 일만
  받는 세션이 있다 — 한 번 거절했으면 그 뒤로는 알림도 걸지 않는다
- 파일은 세션이 끝나도 남고, 그 pid 를 다른 프로세스가 다시 쓰면 산 것처럼 읽힌다.
  보내기 전에 그 이름이 `ListAgents` 에도 뜨는지 한 번 본다
- 이 파일은 문서에 없는 속 파일이라 판이 바뀌면 필드가 달라질 수 있다. 스크립트가
  `못 읽은 파일` 을 내거나 못 돌면 `ListAgents` 로 이름을 보고, **이 기계의 세션에만**
  `pwd` 와 지금 하는 일을 물어 가린다 — 원격·클라우드 세션은 같은 경로를 대도 다른
  체크아웃이다

**3. 보낸다.** 놀고 있는 세션 하나에 idea **하나**를 `SendMessage` 로 보낸다.
일꾼은 이 대화를 모르니 아래 글을 `<내 이름>`·`<id>`·`<제목>`·`<본 가지>`·`<루트>` 를 채워
**통째로** 싣는다 — 일꾼이 받는 것은 이 글뿐이라, 일꾼이 지킬 것은 모두 이 안에 있다.
`<루트>` 는 2 의 `루트 자리` 다. **안 채우면** 일꾼이 워크트리 안에서 제 자리를 루트로 읽는다.

{brief}

**4. 기다린다.** 일하는 세션에는 메시지 없이 `notify_when_idle: true` 로
걸어 둔다. **`ListAgents` 를 되풀이해 훑지 않는다** — 알림이 온다. 알림은 한 번뿐이라,
보고 없이 온 알림(일꾼이 사람에게 묻고 턴을 마쳤다)이면 다시 걸어 둔다.

감독이 루트에 있으면, 일꾼이 멤버를 집고 워크트리를 세우기 전의 틈에 감독의 턴이
끝날 때 훅이 그 멤버를 "아직 집고 있는 것" 으로 붙든다. **그 멤버는 일꾼의 것이다** —
옮기거나 미루거나 노트를 달지 않고 그대로 턴을 마친다.

**5. 보고를 확인하고 다음을 보낸다.** 보고를 믿기 전에 셋을 본다.

    git merge-base --is-ancestor <머지 해시> <본 가지> && echo 있다   머지가 본 가지에 있는가
    moai show <에픽>                       펼친 에픽과 멤버가 done 인가
    git worktree list                      그 워크트리가 사라졌는가

`<에픽>` 은 보고에 실린 에픽 id 다. idea 는 펼칠 때 이미 done 이 되고 멤버를 안 보여 줘,
`moai show <id>` 로는 일이 끝났는지 모른다 — 보고에 없으면 그 idea 의 이력 "… 로
펼쳤다" 에서 읽는다.

셋이 맞으면 그 세션에 다음 idea 를 보낸다. 어긋나면 그 세션에 무엇이 남았는지
묻고, 대신 끝내지 않는다.

## 공유 루트

루트 체크아웃은 **모든 세션이 같이 쓴다.** 한 세션이 병합을 열어 둔 사이(`MERGE_HEAD`)에
다른 세션이 트래커 노트를 커밋하면, 그 커밋이 남의 병합을 제 제목으로 봉인한다 — 실제로
그렇게 됐다. 그래서 감독이든 일꾼이든 루트에서는:

- 트래커 커밋에 경로를 준다 — `git commit -m "…" -- .moai/`. 병합이 열려 있으면 git 이
  경로 준 커밋을 거절하니, 그 병합을 연 세션이 끝낼 때까지 기다렸다 다시 친다. 경로 없는
  `git commit` 은 `git status` 를 보고 쳐도 그 병합을 그대로 봉인한다
- 제 병합은 `git merge --no-ff <가지> -m "…"` 한 번으로 끝낸다. `--no-commit` 을 쓰지 않는다.
  충돌로 멈추면 루트에서 풀지 않고 `git merge --abort` 한다

## 멈출 때

- 부딪히지 않는 idea 가 없거나 놀고 있는 세션이 없으면 사람에게 그렇게 말하고
  멈춘다 — 부딪히는 idea 를 억지로 보내지 않는다
- 일꾼이 사람의 결정을 기다리면 감독이 대신 답하지 않는다. 결정은 사람의 것이다
"#
    )
}

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
fn brief() -> String {
    let review = make_review("--parent <에픽>");
    let close = indent(&close_steps("<리뷰 id>"), "       ");
    format!(
        r#"    감독 세션(<내 이름>)이 idea <id> 를 맡긴다 — <제목>.
    먼저 읽을 것: moai show <id>
    본 가지: <본 가지> — 아래의 가지 이름이다. 감독이 루트에서 읽어 채웠으니 다시 읽지 않는다
    1. 루트에서 펼친다 — idea 를 일감으로 바꾸는 길은 `moai idea promote <id> --from -`
       하나다. 이슈 하나짜리여도 에픽 + 이슈로 펼친다. `--dry-run` 을 먼저 본다.
       그 idea 가 이미 done 이면(누가 펼쳤다) 펼치지 말고 감독에게 알린다 — 다시 펼치면
       에픽이 둘 선다
    2. 멤버를 `moai mv <멤버> in_progress --from todo` 로 집고 루트에서 커밋한다.
       **본 칸을 함께 준다** — 여기는 세션 여럿이 한 `.moai` 를 쓰는 자리라, 옆에서
       먼저 집은 줄을 뒤늦게 덮으면 둘이 같은 일을 한다. 0 아닌 코드가 오면 집힌
       것이니 그 멤버는 두고 감독에게 알린다. 루트는 모든 세션이
       같이 쓴다 — 남이 병합을 열어 둔 사이(MERGE_HEAD)에 친 커밋은 그 병합을 제 제목으로
       봉인한다. 그래서 트래커 커밋에는 경로를 준다. 병합이 열려 있으면 git 이 거절하니,
       그 병합이 끝나기를 기다렸다 다시 친다
         git commit -m "chore(tracker): <에픽> 를 워크트리에서 집는다" -- .moai/
    3. 2 의 커밋 뒤 곧바로 `git worktree add -b worktree-<에픽> .claude/worktrees/<에픽> <본 가지>`
       로 로컬 <본 가지> 에서 뜨고 EnterWorktree(path) 로 들어간다. 이름은 idea id 가 아니라
       펼친 에픽 id 다. 워크트리가 서기 전에는 루트의 다른 세션들이 이 멤버를 제 초점으로 읽는다
    4. 노트에 없는 설계 결정은 추측하지 말고 AskUserQuestion 으로 묻는다 —
       사람이 일꾼 창을 보고 있다
    4-1. **트래커는 언제나 루트의 것을 고친다.** `<루트>` 는 감독이 채운 루트 체크아웃의
       자리다 — 워크트리 안에서 짐작하지 않는다. 스냅샷을 고치는 명령 전부(`add`·`idea add`·
       `note`·`mv`·`edit`·`defer`·`rm`·`idea promote` …)를 워크트리 안에서 맨 `moai` 로
       부르면 그 워크트리의 `.moai` 가 바뀌어, 병합할 때 스냅샷이 충돌한다
       (합쳐도 남의 줄을 덮는다). 워크트리에서는 `moai -C <루트> <명령>` 으로 부르고,
       **리뷰 서브에이전트에게도** 같은 말을 준다 — 넘긴 것을 담다가 그 줄을 워크트리에
       적은 적이 있다. 이미 적었으면 `git checkout -- .moai` 로 되돌리고, 그 줄이 이미
       커밋됐으면 그 커밋까지 되돌린 뒤 루트에서 다시 담는다
    5. 리뷰 이슈를 세워(규칙 3) `/code-review <등급> --fix`. 등급은 개발한 난이도로
       `low`·`medium`·`high` 에서 고른다 — 글·한 줄은 low, 한 파일 안의 동작은 medium,
       여러 파일·쓰기 경로·저장 형식·훅은 high. 망설여지면 한 단계 올리고, 고른 등급과
       까닭은 관점(`-b`)에 한 줄로 적는다. 반영은 별도 fix: 커밋,
       넘긴 것은 이슈 번호와 함께 노트.
       루트에서 세우거나 집은 리뷰 이슈를 워크트리의 훅이 못 봐서 막힐 때만 — 훅은 그
       워크트리의 스냅샷만 읽는다 — 같은 관점·단계·`--fix` 범위로 리뷰 서브에이전트를
       돌린다. 리뷰 이슈·관점(`-b`)·원문 노트·닫는 `-m` 은 그대로 남긴다. 관점이 없다
       같은 다른 거절은 돌아가지 않고 거절문이 내는 명령대로 고친다
    6. 멤버의 일이 다 끝나면 워크트리에서 <본 가지> 를 받아 충돌을 풀고 시험을 돌린다. 고칠
       것은 여기서 고친다 — 워크트리가 남아 있는 동안 루트에서는 규칙 2 가 편집을 막는다
    7. 병합 전에 에픽 전체를 `/code-review <xhigh|max> --fix` 로 본다 — 멤버가 서로 거의
       안 닿고 각자 high 를 지났으면 xhigh, 표면을 가로지르거나 설계 결정이 여럿이거나
       쓰기·저장·훅을 건드렸으면 max. 고른 등급과 까닭은 관점(`-b`)에 적는다. 가지가 <본 가지> 를 떠난
       자리(`git merge-base <본 가지> HEAD`)부터의 diff 다. 6 에서 <본 가지> 를 받았으니 충돌을 푼
       자리도 든다. 리뷰 이슈를 따로 세운다. 막히면 5 의 길로 간다
         {review}
       이 줄도 워크트리에서 부르니 4-1 대로 `moai -C <루트>` 로 친다 — 5 의 리뷰 이슈도 같다
    8. ExitWorktree(keep) 로 루트로 돌아온다 — 워크트리 안에서 그것을 지우면 세션의
       자리가 사라진 디렉터리에 남아 감독이 다시는 이 세션을 루트로 못 본다.
       옆 세션과 병합이 겹치면 먼저 알린 뒤 루트에서 **한 번에** 병합한다.
       `--no-commit` 을 쓰지 않는다. `--no-ff` 가 없으면 fast-forward 로 끝나 병합 커밋이 안 선다
         git merge --no-ff worktree-<에픽> -m "merge: …"
       루트의 `.moai` 에 커밋 안 된 옆 세션의 줄이 있으면 병합이 거절된다 — 2 처럼 경로를
       준 커밋으로 먼저 담는다. 충돌로 멈추면 루트에서 풀지 않는다 — `git merge --abort`
       로 되돌리고 EnterWorktree(path) 로 워크트리에 돌아가 6 부터 다시 한다
    9. 병합이 실제로 끝났으면 루트에서 `git worktree remove .claude/worktrees/<에픽>` 과
       `git branch -d worktree-<에픽>` 으로 워크트리와 가지를 지운다
    10. 그 뒤에 닫는다. **`moai mv <멤버> done` 은 그 병합이 실제로 끝난 뒤에만 친다** —
       병합 전에 옮겼다가 되돌린 일꾼이 있었다. 워크트리가 남아 있으면 훅이 이 일을 옆
       워크트리의 것으로 읽어 `-m` 없는 리뷰 닫기를 못 막는다. 리뷰 이슈는 무엇이
       나왔는지를 남기며 닫는다
{close}
       시험 통과를 보고 2 처럼 경로를 준 커밋으로 루트에 남긴다
    11. SendMessage to "<내 이름>" 로 보고 — 머지 해시, 펼친 에픽 id, 한두 줄 요약,
       넘긴 것·새 idea"#
    )
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
        for piece in [CHEATSHEET, FORKS, NO_GATE, WRITING, CLOSING] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(skill.contains(piece), "스킬에 없다 — {head}");
        }
        assert!(CLOSING.contains(&handoff("<id>")), "안내의 핸드오프 줄이 훅과 갈라졌다");
        let rules = rules();
        assert!(agents.contains(&rules) && skill.contains(&rules), "규칙 셋이 갈라졌다");
        for piece in [GROUPS, IDEAS, DEFERRING, PEOPLE, PROJECTS, COMMITS] {
            let head = piece.lines().next().unwrap();
            assert!(agents.contains(piece), "AGENTS 블록에 없다 — {head}");
            assert!(reference.contains(piece), "참고 문서에 없다 — {head}");
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
        assert!(COMMITS.contains(&format!("`{}:`", crate::git::TRACKER)), "트래커 커밋의 머리가 git 이 거르는 것과 다르다");

        let rule = COMMITS.lines().find(|l| l.contains("본문은")).expect("본문 규칙이 없다");
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
        assert!(WRITING.contains("이모지"), "글 스타일에 이모지 이야기가 없다");
        let emoji = |c: char| {
            c != '✓'
                && (c >= '\u{1F300}'
                    || matches!(c, '\u{FE0F}' | '\u{2600}'..='\u{27BF}' | '\u{2B00}'..='\u{2BFF}'))
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
                    text[..at].ends_with("moai 저장소의 `"),
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
        // 여는 `moai add "` 에 맨다 — 첫 따옴표로 찾으면 앞 산문에 따옴표가 하나 들면
        // 조용히 엉뚱한 토막을 제목으로 재고도 초록이다.
        let title = WRITING_EXAMPLE
            .split_once("moai add \"")
            .and_then(|(_, rest)| rest.split_once('"'))
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
    ///
    /// **여러 줄 명령은 들여쓰지 않는다.** 4칸 들여쓴 채 복사하면 파이썬 소스 줄이 공백으로
    /// 시작해 `IndentationError` 로 죽고, 파이프 끝의 `moai note` 는 빈 글을 받아 원문을 못 남긴다.
    #[test]
    fn the_python_one_liner_survives_formatting() {
        let reference = reference();
        assert!(reference.contains(".get('message',{}).get('content')"));
        assert!(reference.contains("\npython3 -c \"\nimport json,sys\n"), "리뷰 원문을 꺼내는 파이썬이 들여써졌다");
    }

    /// 템플릿 문법의 `{{`·`\{{` 도 포맷을 지나 그대로 선다. 한 번 틀려 참고 문서가 `{이름}` 과 `\{` 를
    /// 가르쳤고, 그대로 쓴 템플릿은 변수가 아니라 글자가 됐다.
    #[test]
    fn the_template_syntax_survives_formatting() {
        let reference = reference();
        for want in ["`{{이름}}`", "`{{`", "`\\{{`"] {
            assert!(reference.contains(want), "참고 문서에 {want} 가 없다 — 포맷이 중괄호를 깎았다");
        }
        assert!(!reference.contains("`{이름}`"), "참고 문서가 `{{{{이름}}}}` 을 `{{이름}}` 으로 깎았다");
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
        let review = make_review("--parent <에픽>");
        assert!(supervise.contains(&review), "에픽 리뷰를 규칙 3 의 줄로 안 세운다");
        assert!(!supervise.replace(&review, "").contains("moai add"), "감독이 promote 말고 다른 길을 가르친다");
    }

    /// **일꾼이 받는 글(`brief`)에 첫 실행에서 넘어진 자리가 선다.** 감독 스킬의 다른
    /// 절에만 적으면 시험은 초록인데 일꾼은 못 받는다 — `MERGE_HEAD` 가 실제로 그랬다.
    /// 하나라도 빠지면 다음 일꾼이 같은 자리에서 또 넘어진다.
    #[test]
    fn the_worker_brief_carries_what_the_first_run_tripped_on() {
        let (brief, supervise) = (brief(), supervise());
        assert!(supervise.contains(&brief), "감독이 싣는 글이 brief 가 아니다");
        let review = make_review("--parent <에픽>");
        for (piece, why) in [
            // **4-1 도 "리뷰 서브에이전트" 를 말한다.** 글자만 보면 5 의 길이 통째로 빠져도
            // 초록이라, 5 의 그 줄에만 있는 앞말까지 매어 찾는다.
            ("범위로 리뷰 서브에이전트를", "워크트리에서 /code-review 가 막힐 때의 길이 없다"),
            ("스냅샷만 읽는다", "막히는 까닭이 없어 다른 거절까지 돌아간다"),
            ("병합이 실제로 끝난 뒤에만", "병합 전에 done 으로 옮기지 말라는 말이 없다"),
            ("/code-review <등급> --fix", "기능 리뷰의 등급을 난이도로 고르라는 말이 없다"),
            ("`low`·`medium`·`high`", "기능 리뷰가 고를 등급의 폭이 없다"),
            ("/code-review <xhigh|max> --fix", "에픽 끝의 xhigh·max 리뷰가 없다"),
            ("관점(`-b`)에 한 줄로 적는다", "고른 등급의 까닭을 남기라는 말이 없다"),
            (review.as_str(), "에픽 리뷰 이슈를 관점과 함께 에픽에 매는 줄이 없다"),
            ("moai -C <루트>", "워크트리 안에서 트래커를 고쳐 병합에서 스냅샷이 충돌한다"),
            ("리뷰 서브에이전트에게도", "서브에이전트가 워크트리의 .moai 를 고친다"),
            ("ExitWorktree(keep)", "루트로 돌아오는 걸음이 없다"),
            ("MERGE_HEAD", "남이 열어 둔 병합을 봉인하지 말라는 말이 없다"),
            ("-- .moai/", "트래커 커밋이 열린 병합을 봉인한다"),
            ("`--no-commit` 을 쓰지 않는다", "병합을 한 번에 끝내라는 말이 없다"),
            ("git merge --no-ff", "fast-forward 로 끝나 병합 커밋이 안 선다"),
            ("git merge --abort", "루트 병합이 충돌할 때의 길이 없다"),
            ("펼친 에픽 id", "보고에 에픽이 없어 감독이 멤버를 못 본다"),
        ] {
            assert!(brief.contains(piece), "{why} — {piece}");
        }
        // 리뷰는 무엇이 나왔는지를 남기며 닫는다. 워크트리가 남아 있으면 훅이 그 에픽을 옆의
        // 일로 읽어 `-m` 없는 닫기를 못 막으니, 닫기는 워크트리를 지운 뒤다.
        for step in close_steps("<리뷰 id>").lines() {
            assert!(brief.contains(step.trim()), "리뷰 닫기 걸음이 갈라졌다 — {step}");
        }
        let removed = brief.find("git worktree remove").expect("워크트리를 지우는 걸음이 없다");
        let closed = brief.find("moai mv <리뷰 id> done").expect("리뷰를 닫는 걸음이 없다");
        assert!(removed < closed, "워크트리를 지우기 전에 리뷰를 닫는다");
        // 에픽 리뷰는 본 가지를 받은 뒤다 — 먼저 보면 충돌을 푼 자리가 리뷰 없이 본 가지에 선다.
        let synced = brief.find("<본 가지> 를 받아").expect("본 가지를 받는 걸음이 없다");
        let reviewed = brief.find("/code-review <xhigh|max> --fix").expect("에픽 리뷰 걸음이 없다");
        assert!(synced < reviewed, "본 가지를 받기 전에 에픽 전체를 리뷰한다");
        assert!(brief.contains("이미 done 이면"), "누가 펼친 idea 를 또 펼쳐 에픽이 둘 선다");
        for (piece, why) in [
            ("거절한 세션", "맡기기를 거절한 세션을 빼라는 말이 없다"),
            ("함께 보내는 idea 끼리도", "같은 바퀴에 보낸 둘이 같은 곳을 고친다"),
            ("보고를 확인할 때까지 후보에서 뺀다", "보낸 idea 가 둘째 일꾼에게 또 간다"),
            ("그 멤버는 일꾼의 것이다", "감독이 훅에 떠밀려 일꾼의 멤버를 옮긴다"),
            ("이 기계의 세션에만", "원격 세션에 루트를 묻는다"),
            ("보고를 아직 확인하지 않은 세션", "맡긴 일을 하던 세션에 또 맡긴다"),
            ("moai show <에픽>", "idea 로 확인하면 멤버가 안 보인다"),
            ("·`<루트>` 를 채워", "감독이 루트 자리를 안 채워 일꾼이 제 워크트리를 루트로 읽는다"),
            // 거둔 일을 맡기는 글은 brief 의 일부만 잇는다 — 그 범위가 4-1 위에서 끊기면
            // 이어받은 일꾼만 워크트리의 `.moai` 를 고친다.
            ("3 의 글의 **4-1 부터 11 까지**", "거둔 일을 맡기는 글이 4-1 을 빼고 잇는다"),
        ] {
            assert!(supervise.contains(piece), "{why} — {piece}");
        }
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
        assert!(brief.contains("<본 가지>"), "일꾼 글에 본 가지 자리가 없다");
        assert!(
            supervise.contains("git merge-base --is-ancestor <머지 해시> <본 가지>"),
            "감독의 병합 확인이 본 가지 자리를 안 쓴다"
        );
        // 루트의 가지는 `git worktree list` 의 첫 자리에서 읽는다 — `-C <루트>` 로 읽으면 옮겨
        // 적은 경로가 틀릴 때 조용히 `main` 이 나오고, 워크트리 안에서 짐작한 자리는 제 가지를 낸다.
        assert!(supervise.contains("if w=$(git worktree list --porcelain); then"), "감독이 루트의 가지를 안 읽는다");
        // 포맷 문자열의 `${{b:-main}}` 이 셸의 `${b:-main}` 으로 풀렸는가.
        assert!(supervise.contains("echo \"${b:-main}\""), "본 가지 한 줄이 포맷에서 깨졌다");
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
        for (name, text, want) in
            [("agents", agents(), 2), ("skill", skill(), 1), ("reference", reference(), 3), ("supervise", supervise(), 1)]
        {
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
