<!-- moai:begin -->
## 이슈 트래커 — moai

이 저장소의 할 일은 `.moai/issues.jsonl` 에 있다.
TodoWrite 나 마크다운 TODO 목록을 쓰지 않는다. 승인 게이트가 없다 — 무엇이든 만들고 무엇이든 옮길 수 있다. 사람을 부르지 않는다.

세션을 시작하면 `moai status` 를 먼저 돌린다. 보드와 경고가 한 화면에 나온다.

    moai status                            보드 · 경고 · 흐름 (세션은 여기서 시작)
    moai ready                             지금 집을 수 있는 일
    moai show <id>                         본문·자식·이력. 왜 그렇게 정했는지가 여기 있다
    moai show -g <키워드>                  이미 적어 뒀는지 찾는다
    moai show -s todo -t bug               필터 (쉼표 = 또는, 반복 = 그리고)
    moai show --tree                       에픽 → 이슈 → 자식
    moai ready --worktree                  옆 워크트리에서 집은 것까지 겹쳐 본다
    moai tui                               탐색기로 돌아다닌다. n 으로 생각을 담는다
    moai add "제목" -p 1 -t bug -e <에픽>  만들기
    moai mv <id> in_progress               집기  →  review  →  done
    moai edit <id> --tag parser            고치기
    moai note <id> "발견한 것"             다음 사람이 읽을 메모
    moai defer <id> -m "왜"                지금 안 할 일을 계획에서 뺀다

모든 명령에 `--json` 이 붙는다.

**담당은 저절로 붙는다** — 만든 사람이 담당이다. 남에게 맡기려면
`-a "이름 (메일)"`, 임자 없이 두려면 `-a none`. 이름과 메일은 `git config`
에서 오고, 거기 없으면 `--user "이름 (메일)"` 이나 `MOAI_ACTOR` 로 준다.

### 갈림길 셋

**1. `add` 냐 `idea` 냐** — 가르는 것은 하나다. *지금 집을 것인가.*
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
    MD

### 지금 범위가 아닌 것은 담는다

    moai idea add "반짝 떠오른 것"                 담기
    moai idea add "긴 생각" -b -                   본문은 stdin 에서
    moai idea ls                                   쌓인 것 보기
    moai show -g <키워드>                          이미 적어 뒀는지 찾기

때가 되면 하나를 에픽과 이슈로 펼친다. 펼치면 그 생각은 닫힌다.

    moai idea promote <id> --from - <<'MD'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    MD

### 이미 있는 일을 지금 안 할 때

    moai defer <id> -m "다음 분기에"       계획에서 잠시 뺀다
    moai defer <id> --undo                 도로 집는다
    moai show --deferred                   미뤄 둔 것만 본다

칸도 종류도 안 바뀐다 — 같은 줄이 그대로 돌아온다. 미룬 것은 `moai ready` 와
보드와 경고에서 빠지고, `moai status` 가 한 줄로 그것을 비춘다. 에픽·마일스톤·
부모를 미루면 그 밑의 일도 같이 빠진다.

### 묶음은 둘이다

    moai epic add "저장 계층"                      에픽
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
미뤄도 첫 칸이니, 그때는 묶음을 `moai defer` 해 계획에서 뺀다.

### 여러 프로젝트

    moai project add <dir>                 내 설정에 등록한다 (`.moai` 가 없어도 받는다)
    moai project ls                        등록한 것과 그 상태

`.moai` 밖에서 부른 `moai`·`moai status`·`moai ready` 는 등록한 프로젝트를
프로젝트마다 한눈에 낸다. 그 밖의 명령은 어느 프로젝트인지 모르니
`moai -C <dir> <명령>` 으로 부른다.

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

**1. 집은 것 밖에 새 이슈를 세우지 않는다.** 집은 이슈 — 첫 칸을 떠났고 아직 안 닫힌 것
(`in_progress`·`review`) — 가 초점이다.
그 일을 하다 나온 것은 같은 에픽 안(`-e <에픽>`)이나 그 일의 자식
(`--parent <id>`)으로 만든다. 지금 할 일이 아니면 `moai idea add` 로 담는다 —
idea 는 이 규칙에서 언제나 자유롭고, `moai add --from` 도 그렇다 (거기서
만들어지는 것은 에픽과 그 자식들이라 그 자체로 한 단위다).

**2. 저장소를 고치기 전에 하나를 집는다.** `moai mv <id> in_progress`.
세는 것은 저장소 안의 일감뿐이다 — `.moai/`·`.claude/`·`target/` 과 저장소
밖(스크래치패드·임시 파일)은 안 센다. `Edit`·`Write` 뿐 아니라 껍데기로 쓰는
것(`>`·`>>`·`sed -i`·`tee`)도 센다. 계획에 없던 것이면 `moai add "제목"` 으로
세우고 그것을 집는다.

**3. 리뷰도 이슈다.** `/code-review` 를 부르기 전에 지금 보는 것에 매인 리뷰
이슈를 세운다.

    moai add "리뷰 — <무엇을 보는가>" -t review --parent <보는 이슈> -b "<무엇을 왜 보는가>"
    moai mv <id> in_progress      리뷰를 시작할 때
    moai note <id> -b - < <리뷰 원문>   리뷰가 낸 글을 그대로
    moai mv <id> done -m "<무엇을 반영하고 무엇을 넘겼나>"

관점(`-b`)과 닫는 한 줄(`-m`)은 규칙이 **실제로 요구한다.** 없이 부르면
막히고, 거절문이 고칠 명령을 함께 낸다. **사람을 부르지 않는다** — 그 명령을
그대로 부르면 지나간다.

**원문과 판단을 두 노트로 가른다** — 리뷰어가 한 말과 이쪽이 정한 것은 다른
글이다. 넘긴 것은 **이슈 번호와 함께** 적는다. "넘겼다" 만 적힌 줄은 아무도
다시 안 본다. 원문을 어디서 찾는지는 스킬의 `references/commands.md` 에 있다.

### 세션을 닫기 전에

`moai status` 를 한 번 더 돌려 경고가 늘지 않았는지 본다. 경고는 막지 않는다 —
에픽 없는 이슈, 오래 멈춘 review, 한 번에 벌여 놓은 것을 비출 뿐이다. 쌓인 idea 와
미뤄 둔 것은 경고가 아니라 알림(`notices`)으로 따로 선다.
<!-- moai:end -->
