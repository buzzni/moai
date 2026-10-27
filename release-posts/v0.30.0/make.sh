#!/usr/bin/env bash
# v0.30.0 소식의 그림 셋을 다시 짓는다 (moai-87vm.hc0). 꼴은 v0.24.0/make.sh 를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<이 판에서 지은 moai> release-posts/v0.30.0/make.sh
#
# 장면은 scripts/site-demo/drive.py 의 release-* 둘과 v0.27.0 의 gantt-months 다. 찍을 시각과 상자를 칠 글을 아래에 고정한다.
# 녹화는 하나씩 돈다. 고정 타이머이므로 기계가 바쁘면 다시 찍고 그림을 열어 확인한다.
# post.md 는 판을 올린 뒤 compose.py 가 짓는다. 그림 밑 한 줄은 media/<이름>.md 에 둔다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI TZ=UTC DEMO_NOW=2026-10-27T12:00:00Z
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"
shot() { node "$root/scripts/release-post/shot.mjs" tui "$@"; }
SIZE=120x34 DEMO_LANG=en "$root/scripts/site-demo/record-tui.sh" "$tmp" release-header
shot "$tmp/tui-release-header.cast" --at 3.0 --title 'moai tui — projects' --mark '<1>▸lantern' -o "$here/media/01-header.png"
SIZE=120x40 DEMO_LANG=en "$root/scripts/site-demo/record-tui.sh" "$tmp" release-stats-keys
shot "$tmp/tui-release-stats-keys.cast" --at 7.0 --title 'moai tui — SPC g s, ?' --mark 'Keys' -o "$here/media/02-keys.png"
# 간트: t(5.8초) 뒤 7.8초의 - 로 Month 에 간 화면
SIZE=120x40 DEMO_LANG=ko "$root/scripts/site-demo/record-tui.sh" "$tmp" gantt-months
shot "$tmp/tui-gantt-months.cast" --at 9.7 --title 'moai tui — SPC g g, -' --line ' 10 ' -o "$here/media/03-gantt-month.png"
