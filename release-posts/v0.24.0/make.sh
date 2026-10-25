#!/usr/bin/env bash
# v0.24.0 소식의 그림을 다시 짓는다 (moai-r96m). 꼴은 v0.23.0/make.sh 를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.24.0/make.sh
#
# 장면(gantt)은 scripts/site-demo/drive.py 에 있고, 계획한 날은 plan.sh 가 단다. 찍을 시각과 상자를 칠 글은
# 아래에 고정한다 — 장면의 걸음을 바꾸면 여기 시각도 같이 본다. 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
SIZE=120x40 "$root/scripts/site-demo/record-tui.sh" "$tmp" gantt
shot() { node "$root/scripts/release-post/shot.mjs" tui "$tmp/tui-gantt.cast" "$@"; }
# 날 단위로 당겨 오늘을 가운데에 둔 간트: 마감을 넘긴 줄(▓ 와 !)에 상자
shot --at 9.3 --title 'moai tui — SPC g g' --line 'Resolve c' -o "$here/media/01-gantt.png"
# 키와 마우스: 당기고 밀기(= -), 옆으로(H L), 오늘(t), 맞춤(z), 계획 옮기기(< >), 막대 끌기, 금 끌기
shot --from 3.5 --to 37.5 --title 'moai tui — SPC g g' -o "$here/media/02-gantt.webm"
# 끝 화면: 끌어 넓힌 줄 칸과, 마우스로 옮긴 막대의 줄
shot --at 37.5 --title 'moai tui — SPC g g' --line 'Sync attachments' -o "$here/media/03-drag.png"
