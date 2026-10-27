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
# seed.sh 와 같은 자리를 같은 길로 고른다 — 그냥 moai 를 realpath 하면 지금 디렉터리의 moai 를 찾는다
moai=${MOAI:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)/target/release/moai}
case $moai in */*) moai=$(realpath -- "$moai") ;; *) moai=$(command -v -- "$moai") ;; esac
[ -x "$moai" ] || { echo "plan.sh: no moai binary at $moai" >&2; exit 1; }
unset MOAI_ACTOR MOAI_NOW
export TZ=UTC MOAI_LANG=en MOAI_NO_UPDATE_CHECK=1 MOAI_CONFIG=$repo/.demo-home/config.toml
cd "$repo"
# seed.sh 와 같은 날·같은 꼴. 쓰기의 시각도 그날로 둔다 — 벽시계로 쓰면 소식 그림을 나중에 다시 지을 때 아래 edit 의
# updated_at 이 TUI 의 지금(drive.py 가 DEMO_NOW 로 고정한다)보다 뒤에 선다
demo_now=$(date -u -d "${DEMO_NOW:-now}" +%Y-%m-%dT%H:%M:%SZ)
export MOAI_NOW=$demo_now
day() { date -u -d "$demo_now $1 days" +%F; }
MIRA='Mira Chen (mira@example.com)'
# plan <제목의 일부> <시작: 오늘에서 며칠> <마감: 오늘에서 며칠>. -g 는 본문·노트까지 찾으니 한 줄만 맞아야 한다 —
# 둘 맞는 것을 앞의 것으로 고르면 seed.sh 의 글 한 줄에 막대가 말없이 옆 줄로 가고, drive.py 의 마우스가 빈 칸을 끈다
plan() {
  local ids
  ids=$("$moai" show -g "$1" --type issue --archived --json | jq -r 'map(.id) | join(" ")')
  case $ids in
    '') echo "plan.sh: no issue matches '$1'" >&2; exit 1 ;;
    *' '*) echo "plan.sh: '$1' matches more than one issue ($ids)" >&2; exit 1 ;;
  esac
  "$moai" edit "$ids" --start "$(day "$2")" --due "$(day "$3")" --user "$MIRA" >/dev/null
}
plan 'Queue edits while offline' -19 -13
plan 'Resolve conflicts' -14 -3
plan 'Retry with backoff' -5 2
plan 'larger than 10 MB' 1 5
plan 'sync status' 3 6
plan 'Index note titles' -19 -15
plan 'Full-text index in' -17 -9
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
# 막는 줄 둘 — 간트는 커서 줄을 막는 줄에서 화살표를 긋는다(web 의 간트 장면이 그것을 보인다, moai-vj8w.fuh)
idof() { "$moai" show -g "$1" --type issue --archived --json | jq -r '.[0].id'; }
"$moai" link "$(idof 'Retry with backoff')" --blocks "$(idof 'larger than 10 MB')" --user "$MIRA" >/dev/null
"$moai" link "$(idof 'Resolve conflicts')" --blocks "$(idof 'larger than 10 MB')" --user "$MIRA" >/dev/null
# seed.sh 는 모두 읽은 채로 두었다 — 녹화에서 새로 생긴 카드만 [NEW] 로 서게. 위의 edit 은 Mira 의 에픽 멤버를 다시
# 안 읽음으로 세워, 간트의 제목마다 [NEW] 가 붙어 잘린다. 한 번 더 읽는다
"$moai" read --all --user "$MIRA" >/dev/null
