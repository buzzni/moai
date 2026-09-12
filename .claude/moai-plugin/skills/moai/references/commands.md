# 전체 명령

`moai --help` 와 `moai <명령> --help` 가 참이다. 이 파일은 그 요약이라
어긋나면 도움말이 이긴다.

## 묶음은 둘이다

    moai epic add "저장 계층"                      에픽
    moai milestone add "v0.1"                      마일스톤
    moai add "제목" -e <에픽> --milestone <마일스톤>
    moai show <에픽|마일스톤 id>                   그 밑에 무엇이 있는지
    moai show --milestone <id>                     그 마일스톤에 딸린 전부

**소속은 물려받는다.** 자식은 부모의 에픽을, 이슈는 제 에픽의 마일스톤을
물려받는다. 이슈마다 다시 적지 않는다 — 에픽을 옮기면 멤버가 따라온다.

## 거름망

쉼표는 "또는", 같은 플래그를 두 번 쓰면 "그리고" 다.

    moai show -s todo -t bug          todo 이면서 bug
    moai show -s todo,review          todo 또는 review
    moai show -e none                 에픽 없는 것
    moai show --deferred              미뤄 둔 것만
    moai show --stale 7               지금 칸에 이레 넘게 머문 것
    moai show --tree                  에픽 → 이슈 → 자식

## 한 번에 만들기

`#` 줄은 에픽, `-` 줄은 바로 위 에픽의 이슈다. `[pN]` 과 `#태그` 는 없어도 된다.

    moai add --from - <<'MD'
    # 저장 계층
    - [p1] 원자적으로 쓴다 #enhancement
    - 잘린 줄을 복구한다 #bug
    MD

`--dry-run` 이 heredoc 오타로 엉뚱한 여섯 개를 만드는 것을 막는다.

## 담아 둔 생각을 펼치기

    moai idea add "반짝 떠오른 것"                 담기
    moai idea add "긴 생각" -b -                   본문은 stdin 에서
    moai idea ls                                   쌓인 것 보기
    moai show -g <키워드>                          이미 적어 뒀는지 찾기

때가 되면 하나를 에픽과 이슈로 펼친다. 펼치면 그 생각은 닫힌다.

    moai idea promote <id> --from - <<'MD'
    # 에픽 제목
    - [p1] 첫 이슈 #enhancement
    MD

## 미루기

    moai defer <id> -m "다음 분기에"       계획에서 잠시 뺀다
    moai defer <id> --undo                 도로 집는다
    moai show --deferred                   미뤄 둔 것만 본다

칸도 종류도 안 바뀐다 — 같은 줄이 그대로 돌아온다. 미룬 것은 `moai ready` 와
보드와 경고에서 빠지고, `moai status` 가 한 줄로 그것을 비춘다.

## 사람

**담당은 저절로 붙는다** — 만든 사람이 담당이다. 남에게 맡기려면
`-a "이름 (메일)"`, 임자 없이 두려면 `-a none`. 이름과 메일은 `git config`
에서 오고, 거기 없으면 `--user "이름 (메일)"` 이나 `MOAI_ACTOR` 로 준다.

## 리뷰가 낸 글을 찾는 법

리뷰 전문은 파일에 남아 있다. 끝났다는 알림에 `task-id` 가 실려 오고, 그것이
곧 파일 이름이다.

    ~/.claude/projects/<프로젝트>/<세션>/subagents/agent-<task-id>.jsonl

리뷰 전문은 **마지막 `text` 블록**이다.

    python3 -c "
    import json,sys
    t=[c['text'] for l in open(sys.argv[1])
       for c in json.loads(l).get('message',{}).get('content') or []
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
