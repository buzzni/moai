#!/usr/bin/env bash
# TUI 장면을 asciinema 로 녹화한다. 장면마다 데모 저장소를 새로 지어 같은 데서 시작한다.
#
#     MOAI=<moai 바이너리> scripts/site-demo/record-tui.sh <내보낼 디렉터리> [장면...]
#
# 장면은 drive.py 의 SCENES(tour, lifecycle, gantt, gantt-months, wiki-search). 화면의 말은 DEMO_LANG(기본 en). 주지 않으면 홈페이지의 둘(tour, lifecycle)만 찍는다.
# cast 머리의 command·env·timestamp 는 지운다 —
# 녹화한 기계의 경로가 공개 페이지에 실리지 않게.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
out=$(mkdir -p "${1:?usage: record-tui.sh <out dir> [scene...]}" && cd "$1" && pwd); shift
if [ $# -gt 0 ]; then scenes=("$@"); else scenes=(tour lifecycle); fi
size=${SIZE:-120x34}
MOAI=$(realpath -- "${MOAI:-$here/../../target/release/moai}")
export MOAI
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for scene in "${scenes[@]}"; do
  repo="$work/$scene/lantern"
  bash "$here/seed.sh" "$repo" >/dev/null
  # 간트는 계획한 날이 있어야 막대를 그린다 — 다른 장면의 그림이 안 바뀌게 그 장면에만 단다. 차례도 굳힌다:
  # 기본 차례(priority)는 같은 p 끼리 seed 마다 새로 뜨는 id 로 갈려(query::display_order), 마우스가 겨눌 줄이 녹화마다 옮겨 간다
  if [ "$scene" = gantt ] || [ "$scene" = gantt-months ]; then
    bash "$here/plan.sh" "$repo"
    printf '[tui]\norder = ["priority", "title"]\n' >> "$repo/.demo-home/config.toml"
  fi
  asciinema rec --headless --window-size "$size" --overwrite -q \
    -c "$(printf '%q ' "$here/drive.py" "$scene" "$repo")" "$work/$scene.cast" >/dev/null
  jq -c 'if type == "object" and has("version") then del(.command, .env, .timestamp) else . end' \
    "$work/$scene.cast" > "$out/tui-$scene.cast"
  echo "recorded $scene"
done
