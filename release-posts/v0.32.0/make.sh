#!/usr/bin/env bash
# v0.32.0 소식의 그림을 다시 짓는다 (moai-gka0). 꼴은 v0.24.0/make.sh(TUI 장면)와 scripts/site-demo/record-web.mjs(web)를 베꼈다.
#
#     NODE_PATH=<playwright 가 든 node_modules> MOAI=<moai 바이너리> release-posts/v0.32.0/make.sh
#
# TUI 장면은 drive.py(이 디렉터리)에 있고 scripts/site-demo/drive.py 의 걸음 틀을 빌린다. 찍을 시각과 상자를 칠 글은 아래에
# 고정한다 — 장면의 걸음을 바꾸면 여기 시각도 같이 본다. 글은 post.md, 그림 밑의 한 줄은 media/<이름>.md 다.
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"

# 데모 저장소: seed + 계획한 날. 마감만 있는 줄(시작이 없다)과 아직 못 정한 줄을 몇 더한다
repo_for() {
  local repo="$tmp/$1/lantern"
  bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
  bash "$root/scripts/site-demo/plan.sh" "$repo"
  printf '[tui]\norder = ["priority", "title"]\n' >> "$repo/.demo-home/config.toml"
  if [ "$1" = gantt ]; then
    (
      cd "$repo"
      export TZ=UTC MOAI_LANG=en MOAI_NO_UPDATE_CHECK=1 MOAI_CONFIG=$repo/.demo-home/config.toml
      unset MOAI_ACTOR MOAI_NOW
      for g in 'Search inside PDF' 'Per-notebook' 'Read-only links' 'Activity feed'; do
        id=$("$MOAI" show -g "$g" --type issue --json | jq -r '.[0].id')
        "$MOAI" edit "$id" --start none --user 'Mira Chen (mira@example.com)' >/dev/null
      done
      "$MOAI" read --all --user 'Mira Chen (mira@example.com)' >/dev/null
    )
  fi
  echo "$repo"
}
record() {
  local scene=$1 repo
  repo=$(repo_for "${scene#v32-}")
  asciinema rec --headless --window-size 120x40 --overwrite -q \
    -c "$(printf '%q ' "$here/drive.py" "$scene" "$repo")" "$tmp/$scene.cast" >/dev/null
}
shot() { local cast=$1; shift; node "$root/scripts/release-post/shot.mjs" tui "$tmp/$cast.cast" "$@"; }
record v32-gantt; record v32-detail; record v32-board
[ -z "${KEEP:-}" ] || cp "$tmp"/*.cast "$KEEP"/

# 간트: 마감만 있는 줄의 채운 막대
shot v32-gantt --at 15 --title 'moai tui — SPC g g' --line 'Search inside' --cells 22,46,59,1 -o "$here/media/01-gantt.png"
# 상세: 이력과 작업 기록
shot v32-detail --at 16 --title 'moai tui' -o "$here/media/02-detail.png"
# 보드: 제목이 칸 안에서 줄을 바꾼다 — 상자는 줄을 바꾼 카드 하나(Retry with backoff…)의 칸 좌표
shot v32-board --at 5 --title 'moai tui — SPC g b' --cells 15,3,20,5 -o "$here/media/03-board.png"

# moai web: 상세 네 방향과 목록 배경 (아래 web.mjs)
web="$tmp/web"
bash "$root/scripts/site-demo/seed.sh" "$web/lantern" >/dev/null
bash "$root/scripts/site-demo/plan.sh" "$web/lantern"
[ -n "${NOWEB:-}" ] || node "$here/web.mjs" "$web/lantern" "$here/media"
