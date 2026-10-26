#!/usr/bin/env bash
# v0.23.0 소식의 그림을 다시 짓는다 (moai-hwl4). 다음 판의 소식도 이 꼴을 베껴 쓴다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.23.0/make.sh
#
# 장면(keys)은 scripts/site-demo/shell.py 에 있고, 상자를 칠 자리는 아래 시각과 글로 고정한다 — 장면의 걸음을
# 바꾸면 여기 시각도 같이 본다. 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
SIZE=110x32 "$root/scripts/site-demo/record-guide.sh" "$tmp" keys
shot() { node "$root/scripts/release-post/shot.mjs" tui "$tmp/guide-keys.cast" "$@"; }
# moai keys: 부딪혀 빠진 줄과, 바꾼 줄(* quit … was SPC q)
shot --at 9 --title 'moai keys' --line 'clashes with browse.sort_priority' --line 'was SPC q' -o "$here/media/01-keys.png"
# 탐색기: 메뉴가 바꾼 키를 대고, 빠진 줄은 위에 한 줄로 선다
shot --at 18 --title 'moai tui' --mark 'x : quit' --line 'clashes with browse.sort_priority' -o "$here/media/02-menu.png"
