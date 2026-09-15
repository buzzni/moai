#!/usr/bin/env bash
# 사람 없이 도는 에이전트 루프. moai 가 다음 일을 고르고, 여기서는 집고·일하고·닫는다.
#
#   AGENT_WORK=./do-one.sh examples/bash-agent/agent.sh
#
# AGENT_WORK 는 이슈 id 와 제목을 인자로 받는 실행 파일이다 (예: `claude -p` 를 감싼 스크립트).
# 그 출력이 노트로 남는다. 실패하면 그 일을 in_progress 로 둔 채 멈춘다 — 사람이 볼 차례다.
# 일이 스스로 칸을 옮기거나 미뤘으면(review·defer) 닫지 않고 그 판단을 그대로 둔다.
# 필요한 것: bash, jq, 기본 칸(todo → in_progress → done), 누가 하는지(git config 또는
# MOAI_ACTOR="이름 (메일)"). PATH 의 moai 가 아닌 것을 쓰려면 MOAI=./target/release/moai.
set -euo pipefail
moai=${MOAI:-moai} work=${AGENT_WORK:-} seen=' '
# **채비는 저장소를 건드리기 전에 다 잰다.** 여기서 걸리면 아무것도 안 집는다(종료 코드 2).
# 집은 뒤에 드러나면 맨 위 일이 in_progress 에 박히고, 다시 부를 때마다 그다음 일이 또 박힌다.
# `command -v` 는 있는지와 실행 권한만 본다 — 셔뱅이 깨진 것은 돌려 봐야 안다.
[ -n "$work" ] || { echo "AGENT_WORK 에 일할 명령을 준다 (id 와 제목을 받는 실행 파일)" >&2; exit 2; }
for need in jq "$work"; do
  command -v "$need" >/dev/null || { echo "실행할 수 없다: $need" >&2; exit 2; }
done
# 일의 출력은 파이프가 아니라 파일로 받는다. `$(…)` 는 파이프가 닫히기를 기다리므로, 일이
# 띄워 두고 간 자식(dev 서버·워처)이 그것을 쥐고 있으면 끝낸 일이 노트도 없이 멈춘다.
log=$(mktemp "${TMPDIR:-/tmp}/moai-agent.XXXXXX") || { echo "출력을 받을 자리를 못 만들었다" >&2; exit 2; }
trap 'rm -f "$log"' EXIT

while :; do
  # `.moai` 밖이면 moai 가 거절하거나(등록한 프로젝트가 없을 때) 등록한 프로젝트마다의 한눈
  # 보기(`{"projects":…}`)를 낸다 — 뒤엣것을 빈 큐로 읽으면 할 일이 쌓여 있는데 "집을 일이
  # 없다" 로 조용히 끝난다. 큐를 내면서 비영으로 끝나는 것은 못 읽는 줄이 있다는 뜻이고
  # (moai 가 어느 줄인지 stderr 에 댄다) 그때는 덜 나온 목록으로 일하지 않는다.
  queue=$("$moai" ready --json) || {
    [ -n "$queue" ] || { echo "moai 저장소(.moai 가 있는 디렉터리) 안에서 돌린다" >&2; exit 2; }
    exit 1
  }                                                   # {"ready":[…],"held":[…]}
  jq -e 'has("ready")' <<<"$queue" >/dev/null || {
    echo "moai 저장소(.moai 가 있는 디렉터리) 안에서 돌린다: $queue" >&2; exit 2
  }
  id=$(jq -r '.ready[0].id // empty' <<<"$queue")
  if [ -z "$id" ]; then
    # `held` 는 미뤄 둔 것·빈 묶음에 막힌 일뿐이다 — 잡혀 있는 일에 막힌 것은 안 센다.
    echo "집을 일이 없다 (미뤄 둔 것·빈 묶음에 막힌 일 $(jq '.held | length' <<<"$queue")건)"; exit 0
  fi
  # 한 판에 한 줄은 한 번만 다룬다. 같은 줄이 또 오면 집기가 먹지 않거나(첫 칸이 in_progress 인
  # 설정) 일이 그 줄을 첫 칸으로 되돌린 것이다 — 그대로 두면 같은 줄을 끝없이 돈다.
  case $seen in *" $id "*) echo "$id 가 또 집을 일로 나왔다 — 사람이 볼 차례다" >&2; exit 1 ;; esac
  seen+="$id "
  title=$(jq -r '.ready[0].title' <<<"$queue")
  col=$(jq -r '.ready[0].status' <<<"$queue")
  # **본 칸을 함께 준다.** `--from` 은 락 안에서 다시 보고 그 칸일 때만 옮긴다 — 옆
  # 에이전트가 이 사이에 먼저 집었거나 닫기까지 했으면 여기서 갈린다. 겨루는 것은 같은
  # `.moai` 를 쓰는 에이전트끼리다 — 워크트리마다 따로 돌리면 서로의 집기를 못 본다.
  #
  # **0 아닌 코드를 전부 "졌다" 로 읽지 않는다.** 신원 없음·락 걸림·칸 오타·깨진 줄도
  # 같은 코드로 온다. 가르는 것은 stdout 이다 — 겨루다 진 것은 `stale` 을 담은 줄을
  # 내고, 실패는 `{"code":…}` 를 stderr 에 내고 stdout 이 빈다. 실패를 "남이 집었다"
  # 로 읽으면 같은 줄이 다시 나와 아래 `seen` 이 엉뚱한 까닭을 대며 멈춘다.
  claim=$("$moai" mv "$id" in_progress --json --from "$col") || {
    [ -n "$claim" ] || exit 1
    continue
  }
  # **`--from` 이 맞았다고 집은 것은 아니다.** 첫 칸이 곧 집는 칸인 설정
  # (`statuses = "in_progress,…"`)에서는 칸이 맞고도 아무것도 안 옮기고 `already` 로
  # 0 을 낸다 — 안 보면 둘이 같은 일을 한다.
  [ "$(jq '.moved | length' <<<"$claim")" -gt 0 ] || continue
  printf '%s  집었다  %s\n' "$id" "$title"
  code=0
  "$work" "$id" "$title" >"$log" 2>&1 || code=$?
  # `note` 는 공백뿐인 메모와 UTF-8 이 아닌 입력을 거절한다 — 그대로 넘기면 끝낸 일이
  # in_progress 로 남은 채 멈춘다. jq 가 깨진 바이트를 U+FFFD 로 바꾸고, 빈 것은 bash 의
  # `[:space:]`(로캘을 탄다)가 아니라 `note` 와 같은 유니코드 공백으로 가른다.
  out=$(jq -Rrs 'if test("\\S") then . else "(출력 없음)" end' "$log")
  if [ "$code" -ne 0 ]; then
    # 노트를 못 남겨도 1 로 끝난다 — 실패는 언제나 1 이어야 뒤에서 종료 코드로 가를 수 있다.
    printf '실패(%s): %s\n' "$code" "$out" | "$moai" note "$id" -b - || true
    exit 1
  fi
  printf '%s\n' "$out" | "$moai" note "$id" -b -
  # 일이 스스로 옮겼거나(review·done) 미뤘으면 덮지 않는다 — 안 하기로 한 것을 done 으로 옮기지
  # 않는다. **묶음을 미룬 것도 센다** — `deferred_at` 은 제 줄에 적힌 것뿐이라 미룬 에픽의
  # 멤버는 null 이고, 계획 밖인지는 `shelved_by` 가 말한다(제 줄이면 제 id, 물려받았으면 그 위).
  if jq -e '.status == "in_progress" and .shelved_by == null' <<<"$("$moai" show "$id" --json)" >/dev/null; then
    # 닫을 때도 본 칸을 준다 — `show` 와 이 줄 사이는 집을 때와 똑같은 틈이고, 여기서
    # 덮으면 남이 옮겨 둔 `review` 가 사라진다. 진 것은 실패가 아니라 남의 일이다.
    "$moai" mv "$id" "done" --from in_progress || true   # 따옴표는 이것이 셸 낱말이 아니라 칸 이름이라는 표시다(SC1010)
  fi
done
