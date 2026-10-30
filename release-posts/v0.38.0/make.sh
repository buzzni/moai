#!/usr/bin/env bash
# Rebuild the v0.38.0 pictures from a fictional Lantern project.
# MOAI=<v0.38.0 binary> release-posts/v0.38.0/make.sh
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-10-30T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"

repo="$tmp/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
bash "$root/scripts/site-demo/plan.sh" "$repo"
printf '[tui]\norder = ["priority", "title"]\n' >> "$repo/.demo-home/config.toml"
export MOAI_CONFIG="$repo/.demo-home/config.toml"
# Keep Sync in progress and close Sharing so both milestone fold defaults are visible.
for name in search editor share; do
  epic=$(sed -n "s/^$name=//p" "$repo/.demo-ids")
  "$MOAI" -C "$repo" show -e "$epic" --type issue --json |
    jq -r '.[] | select(.status != "done") | [.id, .assignee, .assignee_email] | @tsv' |
    while IFS=$'\t' read -r id who email; do
      "$MOAI" -C "$repo" mv "$id" done --user "$who ($email)" >/dev/null
    done
done

record() {
  local scene=$1
  asciinema rec --headless --window-size 120x40 --overwrite -q \
    -c "$(printf '%q ' "$here/drive.py" "$scene" "$repo")" "$tmp/$scene.cast" >/dev/null
  jq -c 'if type == "object" and has("version") then del(.command, .env, .timestamp) else . end' \
    "$tmp/$scene.cast" > "$tmp/$scene-clean.cast"
}
record release-board
node "$root/scripts/release-post/shot.mjs" tui "$tmp/release-board-clean.cast" \
  --at 8.5 --title 'moai tui — coloured board cards' \
  -o "$here/media/01-tui-board.png"
record release-list
node "$root/scripts/release-post/shot.mjs" tui "$tmp/release-list-clean.cast" \
  --at 3.5 --title 'moai tui — milestone list' \
  -o "$here/media/02-tui-list.png"
record release-archive
node "$root/scripts/release-post/shot.mjs" tui "$tmp/release-archive-clean.cast" \
  --at 4.2 --title 'moai tui — SPC o o' \
  --mark 'Archive' -o "$here/media/03-tui-archive-days.png"

node "$here/web.mjs" "$repo" "$here/media"
