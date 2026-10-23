#!/usr/bin/env bash
# TUI 장면을 asciinema 로 녹화한다. 장면마다 데모 저장소를 새로 지어 같은 데서 시작한다.
#
#     MOAI=<moai 바이너리> scripts/site-demo/record-tui.sh <내보낼 디렉터리> [장면...]
#
# 장면은 drive.py 의 SCENES(tour, lifecycle). cast 머리의 command·env·timestamp 는 지운다 —
# 녹화한 기계의 경로가 공개 페이지에 실리지 않게.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
out=$(mkdir -p "${1:?usage: record-tui.sh <out dir> [scene...]}" && cd "$1" && pwd); shift
if [ $# -gt 0 ]; then scenes=("$@"); else scenes=(tour lifecycle); fi
size=${SIZE:-120x34}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for scene in "${scenes[@]}"; do
  repo="$work/$scene/lantern"
  bash "$here/seed.sh" "$repo" >/dev/null
  asciinema rec --headless --window-size "$size" --overwrite -q \
    -c "$here/drive.py $scene $repo" "$work/$scene.cast" >/dev/null
  jq -c 'if type == "object" and has("version") then del(.command, .env, .timestamp) else . end' \
    "$work/$scene.cast" > "$out/tui-$scene.cast"
  echo "recorded $scene"
done
