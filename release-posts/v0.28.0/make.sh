#!/usr/bin/env bash
# v0.28.0 소식의 그림을 다시 짓는다 (moai-8kvc). 꼴은 v0.27.0/make.sh 를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.28.0/make.sh
#
# 장면은 scripts/site-demo/drive.py 의 keys 다 — 2.5초에 ? 로 창을 열고, 5.5초부터 j 로 굴리고, 9.25초에 ? 로 닫는다.
# 같은 장면을 창 크기 둘로 찍는다: 160x40 이면 세 열, 120x34 면 두 열이다(help::lay_out 이 폭과 줄이 다 드는
# 가장 적은 열을 고른다). 화면은 PR 의 그림처럼 한국어다. 장면의 걸음을 바꾸면 아래 시각도 같이 본다.
# 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"
SIZE=160x40 DEMO_LANG=ko "$root/scripts/site-demo/record-tui.sh" "$tmp/wide" keys
SIZE=120x34 DEMO_LANG=ko "$root/scripts/site-demo/record-tui.sh" "$tmp/mid" keys
shot() { node "$root/scripts/release-post/shot.mjs" tui "$@"; }
# 넓은 화면: 키 목록이 세 열로 선다
shot "$tmp/wide/tui-keys.cast" --at 4.5 --title 'moai tui — ?' -o "$here/media/01-wide.png"
# 가운데 크기: 두 열, 구석의 1/52 가 굴릴 줄이 남았다고 댄다
shot "$tmp/mid/tui-keys.cast" --at 4.5 --title 'moai tui — ?' -o "$here/media/02-mid.png"
# 창이 선 채 j 로 굴린다 — 구석의 수가 1/52 에서 9/52 로. 목록 제목에 도는 빛줄기(draw.rs shimmer)는 TUI 에 끄개가 없어,
# 창이 목록을 덮은 동안(2.5초 ? 부터 9.25초 ? 까지)만 담는다. 바닥 줄의 `? 키 목록` 을 따로 찍지 않은 까닭도 그 빛줄기다
shot "$tmp/mid/tui-keys.cast" --from 2.7 --to 9.1 --title 'moai tui — ?' -o "$here/media/03-scroll.webm"
ls -l "$here/media"
