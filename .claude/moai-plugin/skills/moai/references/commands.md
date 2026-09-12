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

    moai idea add "반짝 떠오른 것"
    moai idea ls
    moai idea promote <id> --from -    에픽과 이슈로 펼치고 그 생각을 닫는다

## 사람

이름과 메일은 `git config` 에서 온다. 없으면 `--user "이름 (메일)"` 이나
`MOAI_ACTOR` 로 준다. 남에게 맡기려면 `-a "이름 (메일)"`, 임자 없이 두려면
`-a none`.

## 훅

    moai hook <event>    Claude 의 훅이 부른다. 사람이 손으로 부를 일은 없다

`moai skill install` 이 심은 플러그인이 이것을 부른다. 무엇이 어긋나도 종료
코드는 0 이다 — 훅이 시끄러우면 사람이 훅을 꺼 버리고, 꺼진 규칙은 없는
규칙이다.
