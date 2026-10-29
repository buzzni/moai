#!/usr/bin/env bash
# v0.34.0 소식의 그림을 다시 짓는다 (moai-skjr — 목록·간트 줄의 상태 빛). 꼴은 v0.32.0/make.sh 의 web 장면을 베꼈다.
# 이 판에 바뀐 화면은 moai web 뿐이라 TUI 장면은 없다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.34.0/make.sh
#
# 예: NODE_PATH=~/.cache/moai-site-tools/node_modules MOAI=$PWD/target/release/moai release-posts/v0.34.0/make.sh
# (web/node_modules 에도 playwright 가 있다). 장면의 걸음과 상자는 web.mjs 에 있다. 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"

# 데모 저장소: seed + 계획한 날(간트 막대). 'Search that feels instant' 에 끝낸 줄·리뷰 줄·집은 줄이 하나씩 선다
web="$tmp/web"
bash "$root/scripts/site-demo/seed.sh" "$web/lantern" >/dev/null
bash "$root/scripts/site-demo/plan.sh" "$web/lantern"
node "$here/web.mjs" "$web/lantern" "$here/media"
