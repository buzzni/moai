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
  id=$(jq -r '.ready[0].id // empty' <<<"$queue")
  if [ -z "$id" ]; then
    echo "집을 일이 없다 (막혀 기다리는 것 $(jq '.held | length' <<<"$queue")건)"; exit 0
  fi
  title=$(jq -r '.ready[0].title' <<<"$queue")
  "$moai" mv "$id" in_progress
  if out=$("$work" "$id" "$title" 2>&1); then
    printf '%s\n' "${out:-(출력 없음)}" | "$moai" note "$id" -b -
    "$moai" mv "$id" done
  else
    printf '실패(%s): %s\n' "$?" "$out" | "$moai" note "$id" -b -; exit 1
  fi
done
