#!/usr/bin/env bash
# v0.26.0 소식의 그림을 다시 짓는다 (moai-m7jm). 꼴은 v0.23.0/make.sh(셸 장면)를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.26.0/make.sh
#
# 장면(wiki)은 scripts/site-demo/shell.py 에 있고, 이미 선 교훈 하나는 record-guide.sh 가 심는다. 찍을 시각과
# 상자를 칠 글은 아래에 고정한다 — 장면의 걸음을 바꾸면 여기 시각도 같이 본다. 글은 post.md, 그림 밑의 한 줄은
# media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
SIZE=130x50 "$root/scripts/site-demo/record-guide.sh" "$tmp" wiki
shot() { node "$root/scripts/release-post/shot.mjs" tui "$tmp/guide-wiki.cast" "$@"; }
# moai wiki search: 묻는 말에 맞는 절이 순서대로, 하나마다 path:line 과 그 절의 줄 — 첫 자리에 선 교훈과 몇 개 중 몇 개인지에 상자
shot --at 5.0 --lines 16 --title 'moai wiki search' --line 'keep-the-main-thread-off-the-index.md:6' --mark '5 of 8' -o "$here/media/01-search.png"
# moai wiki learn 이 페이지를 쓰고 같은 부름에 이슈에 Lesson: 노트를 단다. wiki ls 는 교훈마다 몇 줄이 겪었는지(×N)를 단다
shot --at 17.0 --lines 26 --title 'moai wiki learn' --line 'Noted on' --mark '×1' --mark '×2' -o "$here/media/02-learn.png"
# moai show <id> 의 Wiki 토막: 그 줄이 겪은 교훈, 이름을 댄 페이지, 제목으로 찾은 관련 절 — 칸 좌표로 토막 전체에 상자
shot --at 20.0 --lines 47 --title 'moai show' --cells 27,0,128,8 -o "$here/media/03-show.png"
ls -l "$here/media"
