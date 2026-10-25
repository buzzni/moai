#!/usr/bin/env bash
# v0.25.0 소식의 그림을 다시 짓는다 (moai-vj8w.fuh). 꼴은 v0.24.0/make.sh 를 베꼈다 — 그쪽은 TUI, 이쪽은 web 의 간트다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.25.0/make.sh
#
# 장면(gantt)은 scripts/site-demo/record-web.mjs 에 있고, 계획한 날과 막는 줄은 plan.sh 가 단다. 상자를 칠 자리도
# 장면이 고른다(커서 줄과 그 줄의 날짜 칸) — web 의 탭은 주소로 못 열어 shot.mjs web 이 간트에 닿지 못한다.
# 글은 post.md(compose.py 가 짓는다), 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
repo="$tmp/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
bash "$root/scripts/site-demo/plan.sh" "$repo"
SCENE=gantt node "$root/scripts/site-demo/record-web.mjs" "$repo" "$tmp/out"
mkdir -p "$here/media"
# 날 걸음의 간트: 커서 줄(막는 줄에서 화살표가 선다)과 그 막대 위의 말풍선
cp "$tmp/out/web-gantt-mark.png" "$here/media/01-gantt.png"
# 걸음 바꾸기(= -), 휠·머리 끌기로 옆으로, 말풍선, 화살표, 막대 끌기(놓을 때 한 번 쓴다), 줄 칸 너비 끌기
cp "$tmp/out/web-gantt.webm" "$here/media/02-gantt.webm"
# 끝 화면: 이틀 뒤로 옮긴 막대와 넓힌 줄 칸
cp "$tmp/out/web-gantt.png" "$here/media/03-drag.png"
ls -l "$here/media"
