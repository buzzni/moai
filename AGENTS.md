<!-- moai:begin -->
## 이슈 트래커 — moai

이 저장소의 할 일은 `.moai/issues.jsonl` 에 있다.
TodoWrite 나 마크다운 TODO 목록을 쓰지 않는다.

세션을 시작하면 `moai status` 를 먼저 돌린다. 보드와 경고가 한 화면에 나온다.

    moai status                            보드 · 경고 · 흐름
    moai ready                             지금 집을 수 있는 일
    moai show <id>                         하나 펼치기 — 본문·자식·이력까지
    moai show -s todo -t bug               필터 (쉼표 = 또는, 반복 = 그리고)
    moai show --tree                       에픽 → 이슈 → 자식
    moai tui                               탐색기로 돌아다닌다 (읽기 전용)
    moai add "제목" -p 1 -t bug -e <에픽>  만들기
    moai mv <id> in_progress               집기   →  review  →  done
    moai edit <id> --tag parser            고치기
    moai defer <id> -m "다음 분기"         지금 안 할 일을 계획에서 뺀다
    moai note <id> "발견한 것"             다음 사람이 읽을 메모

모든 명령에 `--json` 이 붙는다.

**담당은 저절로 붙는다** — 만든 사람이 담당이다. 남에게 맡기려면
`-a "이름 (메일)"`, 임자 없이 두려면 `-a none`. 이름과 메일은 `git config`
에서 오고, 거기 없으면 `--user "이름 (메일)"` 로 준다.

### 지금 범위가 아닌 것은 담는다

작업 중 떠오른 것 중 **지금 할 일이 아닌 것**은 이슈로 만들지 말고 담아 둔다.
제목 하나면 되고, 일로 세지 않으므로 보드도 `ready` 도 흐려지지 않는다.

    moai idea add "반짝 떠오른 것"                 담기
    moai idea add "긴 생각" -b -                   본문은 stdin 에서
    moai idea ls                                   쌓인 것 보기
    moai show -g <키워드>                          이미 적어 뒀는지 찾기

때가 되면 하나를 에픽과 이슈로 펼친다. 펼치면 그 생각은 닫힌다.

    moai idea promote <id> --from - <<'EOF'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    EOF

`moai add` 와 `moai idea add` 를 가르는 것은 하나다 — **지금 집을 것인가.**
집을 것이면 이슈고, 나중에 볼 것이면 idea 다.

### 이미 있는 일을 지금 안 할 때

idea 는 "아직 일이 아닌 것" 이고, **미루기는 "일이지만 지금은 아닌 것"** 이다.

    moai defer <id> -m "다음 분기에"       계획에서 잠시 뺀다
    moai defer <id> --undo                 도로 집는다
    moai show --deferred                   미뤄 둔 것만 본다

칸도 종류도 안 바뀐다 — 같은 줄이 그대로 돌아온다. 미룬 것은 `moai ready` 와
보드와 경고에서 빠지고, `moai status` 가 한 줄로 그것을 비춘다.

### 묶음은 둘이다

    moai epic add "저장 계층"                      에픽
    moai milestone add "v0.1"                      마일스톤
    moai add "제목" -e <에픽> --milestone <마일스톤>
    moai show <에픽|마일스톤 id>                   그 밑에 무엇이 있는지
    moai show --milestone <id>                     그 마일스톤에 딸린 전부

**소속은 물려받는다.** 자식은 부모의 에픽을, 이슈는 제 에픽의 마일스톤을
물려받는다. 이슈마다 다시 적지 않는다 — 에픽을 옮기면 멤버가 따라온다.

### 기능 요청을 받으면

파일 하나로 안 끝나는 요청이면 **코드를 쓰기 전에** 이렇게 한다.

1. `moai status` 로 이미 있는 에픽을 본다. 겹칠 것 같으면 `moai show -g <키워드>`.
2. 에픽 하나 + 이슈 3~7개로 쪼갠 안을 사람에게 **한 번** 보여주고 물어본다.
3. "좋다" 를 받으면 한 번에 만든다. `--dry-run` 으로 먼저 확인해도 된다.

       moai add --from - <<'EOF'
       # 에픽 제목
       - [p1] 첫 이슈 #enhancement
       - [p2] 둘째 이슈
       EOF

4. `moai mv <id> in_progress` 로 집고, 끝나면 `done` 으로 옮긴다.
5. 작업 중 발견한 것 중 지금 범위가 아닌 것은 `moai idea add` 로 담아 둔다.
   **적지 않고 넘어가는 것이 제일 나쁘다.** 지금 집을 것이면 `moai add` 다.
6. 왜 그렇게 정했는지는 `moai note <id>` 로 이슈에 붙인다. 다음 세션이
   `moai show <id>` 로 그것을 읽는다.

도구가 자라 이 블록이 낡으면 `moai init` 을 다시 부른다. 이슈와 저널은
건드리지 않고 이 블록만 다시 쓴다.

### 승인 게이트는 없다

무엇이든 만들고 무엇이든 옮길 수 있다. 사람을 부르지 않는다.
대신 `moai status` 가 에픽 없는 이슈, 오래 멈춘 review, 한 번에 벌여 놓은
것, 쌓인 idea, 미뤄 둔 것을 드러낸다. **세션을 닫기 전에 한 번 더 돌려서 경고가 늘지
않았는지 본다.**
<!-- moai:end -->
