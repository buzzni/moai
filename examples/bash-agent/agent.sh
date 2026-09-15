#!/usr/bin/env bash
# 사람 없이 도는 에이전트 루프. moai 가 다음 일을 고르고, 여기서는 집고·일하고·닫는다.
#
#   AGENT_WORK=./do-one.sh examples/bash-agent/agent.sh
#
# AGENT_WORK 는 이슈 id 와 제목을 인자로 받는 실행 파일이다 (예: `claude -p` 를 감싼 스크립트).
# 그 출력이 노트로 남는다. 실패하면 그 일을 in_progress 로 둔 채 멈춘다 — 사람이 볼 차례다.
# 필요한 것: bash, jq, 누가 하는지(git config 또는 MOAI_ACTOR="이름 (메일)").
# PATH 의 moai 가 아닌 것을 쓰려면 MOAI=./target/release/moai.
set -euo pipefail
moai=${MOAI:-moai} work=${AGENT_WORK:?AGENT_WORK 에 일할 명령을 준다}

while :; do
  queue=$("$moai" ready --json)                       # {"ready":[…],"held":[…]}
  # `.moai` 밖이면 등록한 프로젝트마다의 한눈 보기(`{"projects":…}`)가 온다 — 그걸 빈 큐로 읽으면
  # 할 일이 쌓여 있는데 "집을 일이 없다" 로 조용히 끝난다.
  jq -e 'has("ready")' <<<"$queue" >/dev/null 2>&1 || {
    echo "moai 저장소(.moai 가 있는 디렉터리) 안에서 돌린다: $queue" >&2; exit 2
  }
  id=$(jq -r '.ready[0].id // empty' <<<"$queue")
  if [ -z "$id" ]; then
    echo "집을 일이 없다 (막혀 기다리는 것 $(jq '.held | length' <<<"$queue")건)"; exit 0
  fi
  title=$(jq -r '.ready[0].title' <<<"$queue")
  # 집기는 락 안에서 한 번 판정된다. 옆 에이전트가 같은 줄을 먼저 집었으면 `moved` 가 비고
  # `already` 에 선다 — 종료 코드는 0 이라, 안 보면 둘이 같은 일을 한다.
  claimed=$("$moai" mv "$id" in_progress --json | jq '.moved | length')
  [ "$claimed" -gt 0 ] || continue
  if out=$("$work" "$id" "$title" 2>&1); then
    # 공백뿐인 출력은 `note` 가 빈 메모로 거절한다 — 끝낸 일이 in_progress 로 남은 채 멈춘다.
    [[ $out = *[![:space:]]* ]] || out='(출력 없음)'
    printf '%s\n' "$out" | "$moai" note "$id" -b -
    "$moai" mv "$id" done
  else
    printf '실패(%s): %s\n' "$?" "$out" | "$moai" note "$id" -b -; exit 1
  fi
done
