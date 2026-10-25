#!/usr/bin/env bash
# seed.sh 가 지은 Lantern 의 이슈에 계획한 날(시작~마감)을 단다 — 간트 장면(drive.py 의 gantt)이 그릴 막대다.
#
#     MOAI=<moai 바이너리> scripts/site-demo/plan.sh <seed.sh 가 지은 저장소>
#
# seed.sh 에 넣지 않은 까닭은 홈페이지의 다른 장면(목록·칸반)이 마감 칸을 그리기 때문이다 — 다시 찍을 때 그
# 그림이 말없이 바뀌지 않게, 날짜가 필요한 장면만 이것을 부른다. 날은 오늘에서 센다. 한 줄은 늦게(▓), 막 끝난
# 줄은 계획보다 일찍, 나머지는 앞으로 — 막대가 무엇을 말하는지 한 화면에 다 서게 흩었다.
set -euo pipefail
repo=$(cd -- "${1:?usage: plan.sh <repo>}" && pwd)
moai=$(realpath -- "${MOAI:?MOAI is the moai binary}")
unset MOAI_ACTOR MOAI_NOW
export MOAI_LANG=en MOAI_NO_UPDATE_CHECK=1 MOAI_CONFIG=$repo/.demo-home/config.toml
cd "$repo"
day() { date -u -d "$1 days" +%F; }
# plan <제목의 일부> <시작: 오늘에서 며칠> <마감: 오늘에서 며칠>
plan() {
  local id
  id=$("$moai" show -g "$1" --type issue --archived --json | jq -r '.[0].id // empty')
  [ -n "$id" ] || { echo "plan.sh: no issue matches '$1'" >&2; exit 1; }
  "$moai" edit "$id" --start "$(day "$2")" --due "$(day "$3")" --user 'Mira Chen (mira@example.com)' >/dev/null
}
plan 'Queue edits while offline' -19 -13
plan 'Resolve conflicts' -14 -3
plan 'Retry with backoff' -5 2
plan 'larger than 10 MB' 1 5
plan 'sync status' 3 6
plan 'Index note titles' -19 -15
plan 'Full-text index' -17 -9
plan 'Highlight matches' -8 -1
plan 'Fuzzy matching' -4 3
plan 'PDF attachments' 2 6
plan 'Markdown tables' -14 -8
plan 'Paste images' -10 -4
plan 'Slash command' -7 1
plan 'Drag to reorder' 8 14
plan 'Invite people' -3 12
plan 'Per-notebook' 10 20
plan 'Read-only links' 14 24
plan 'Activity feed' 20 30
