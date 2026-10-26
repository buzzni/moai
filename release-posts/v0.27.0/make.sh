#!/usr/bin/env bash
# v0.27.0 소식의 그림을 다시 짓는다 (moai-0fai). 꼴은 v0.24.0/make.sh(TUI)와 v0.25.0/make.sh(web)를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.27.0/make.sh
#
# TUI 장면(gantt-months, wiki-search)은 scripts/site-demo/drive.py 에, web 장면(drag, wikipanel)은 record-web.mjs 에
# 있다. 계획한 날은 plan.sh 가 단다. 찍을 시각과 상자를 칠 글은 아래에 고정한다 — 장면의 걸음을 바꾸면 여기 시각도
# 같이 본다. 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"
# web: 막대 끌기와 이슈 패널의 Wiki 토막
repo="$tmp/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
bash "$root/scripts/site-demo/plan.sh" "$repo"
for scene in drag wikipanel; do SCENE=$scene node "$root/scripts/site-demo/record-web.mjs" "$repo" "$tmp/web"; done
# TUI: 한국어 화면의 간트 머리, 위키 탭의 찾기
SIZE=120x40 DEMO_LANG=ko "$root/scripts/site-demo/record-tui.sh" "$tmp" gantt-months
SIZE=120x34 "$root/scripts/site-demo/record-tui.sh" "$tmp" wiki-search
shot() { node "$root/scripts/release-post/shot.mjs" tui "$@"; }
# web 의 간트에서 막대를 하루 뒤로 끌고 다시 앞으로 — 놓을 때마다 옮긴 날 수를 쓴다(moai edit --shift)
cp "$tmp/web/web-drag.webm" "$here/media/01-drag.webm"
# 한국어 화면의 TUI 간트: 머리의 달 이름이 10월
shot "$tmp/tui-gantt-months.cast" --at 6.5 --title 'moai tui — SPC g g' --mark '2026년 10월' --mark '2026년 11월' -o "$here/media/02-months.png"
# web 의 이슈 패널: 커밋 뒤의 Wiki 토막 — 이 이슈를 이름 댄 페이지와 관련 절
cp "$tmp/web/web-wikipanel-mark.png" "$here/media/03-wiki-panel.png"
# TUI 의 위키 탭(SPC g w)에서 / 로 찾기: 낱말이 높은 페이지부터, moai wiki search 의 차례
shot "$tmp/tui-wiki-search.cast" --at 9.0 --title 'moai tui — SPC g w' --mark '/sync worker' -o "$here/media/04-wiki-search.png"
ls -l "$here/media"
