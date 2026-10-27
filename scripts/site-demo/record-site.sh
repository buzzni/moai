#!/usr/bin/env bash
# 홈·사용 방법의 미디어를 한 번에 다시 짓는다. 반수동인 팀 장면은 그대로 둔다.
#
#     MOAI=<moai> NODE_PATH=<playwright 가 든 node_modules> scripts/site-demo/record-site.sh <내보낼 디렉터리>
#
# 내보낼 자리는 반드시 준다 — site/assets/media 를 말없이 덮지 않는다.
# record-guide.sh 는 DEMO_ROOT(기본 /tmp/mira)를 지우고 다시 짓고 init 에 그 경로를 찍는다.
# 커밋할 녹화는 기본값으로 찍는다. 시각에 맞춰 키를 보내니 한 번에 하나만, 빌드가 끝난 뒤 돌린다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
out=${1:?usage: record-site.sh <out dir>}
for tool in asciinema jq node; do
  command -v "$tool" >/dev/null || { echo "record-site.sh: missing $tool" >&2; exit 1; }
done
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
node -e "require('playwright')" >/dev/null 2>&1 || {
  echo 'record-site.sh: playwright not found; set NODE_PATH to its node_modules' >&2; exit 1;
}
if [ -z "${FFMPEG:-}" ]; then
  FFMPEG=$(command -v ffmpeg || true)
  if [ -z "$FFMPEG" ]; then
    shopt -s nullglob
    candidates=("$HOME"/.cache/ms-playwright/ffmpeg-*/ffmpeg-linux)
    if [ ${#candidates[@]} -gt 0 ]; then
      FFMPEG=$(printf '%s\n' "${candidates[@]}" | sort -V | tail -n 1)
    fi
  fi
fi
command -v "${FFMPEG:-}" >/dev/null || { echo 'record-site.sh: ffmpeg not found; set FFMPEG' >&2; exit 1; }
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
[ -x "$MOAI" ] || { echo "record-site.sh: no moai binary at $MOAI" >&2; exit 1; }
export MOAI TZ=UTC
out=$(mkdir -p "$out" && cd "$out" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

MEDIA=(
  guide-init.cast guide-backlog.cast
  tui-tour.cast tui-lifecycle.cast tui-list.cast tui-board.cast tui-stats.cast tui-wiki.cast tui-gantt.cast
  web-detail.webm web-detail.png web-board.webm web-board.png
  web-wiki.webm web-wiki.png web-gantt.webm web-gantt.png
)

SIZE=100x30 bash "$here/record-guide.sh" "$tmp/media" init backlog
SIZE=120x34 bash "$here/record-tui.sh" "$tmp/media" tour lifecycle list board stats wiki
SIZE=120x40 bash "$here/record-tui.sh" "$tmp/media" gantt
for scene in detail board wiki gantt; do
  repo="$tmp/$scene/lantern"
  bash "$here/seed.sh" "$repo" >/dev/null
  if [ "$scene" = gantt ]; then bash "$here/plan.sh" "$repo"; fi
  SCENE=$scene node "$here/record-web.mjs" "$repo" "$tmp/web" | tee "$tmp/$scene.log"
  lead=$(awk -v scene="$scene" '$1 == "recorded" && $2 == scene && $3 == "lead" {print $4}' "$tmp/$scene.log")
  [[ "$lead" =~ ^[0-9]+\.[0-9]+$ ]] || { echo "record-site.sh: missing lead for $scene" >&2; exit 1; }
  "$FFMPEG" -v error -y -ss "$lead" -i "$tmp/web/web-$scene.webm" -an \
    -c:v libvpx-vp9 -crf 40 -b:v 0 "$tmp/media/web-$scene.webm"
  cp "$tmp/web/web-$scene.png" "$tmp/media/"
done
for file in "${MEDIA[@]}"; do
  [ -s "$tmp/media/$file" ] || { echo "record-site.sh: missing $file" >&2; exit 1; }
done
echo 'tui-team.cast is kept as is: record-team.sh is partly manual.'
for file in "${MEDIA[@]}"; do cp "$tmp/media/$file" "$out/"; done
ls -l "${MEDIA[@]/#/$out/}"
