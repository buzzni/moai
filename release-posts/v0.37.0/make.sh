#!/usr/bin/env bash
# Rebuild the v0.37.0 pictures from a fictional Lantern project.
# NODE_PATH=<playwright node_modules> MOAI=<v0.37.0 binary> release-posts/v0.37.0/make.sh
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-10-30T12:00:00Z
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"

repo="$tmp/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
printf '[tui]\norder = ["priority", "title"]\n' >> "$repo/.demo-home/config.toml"
export MOAI_CONFIG="$repo/.demo-home/config.toml"
# Close the two demo epics so the expanded lane shows one compact Done card.
for name in search editor; do
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
  --at 9.5 --title 'moai tui — milestone board' \
  --mark 'Editor polish' -o "$here/media/01-tui-board.png"
record release-archive
node "$root/scripts/release-post/shot.mjs" tui "$tmp/release-archive-clean.cast" \
  --at 4.2 --title 'moai tui — SPC o o' \
  --mark 'Archive' -o "$here/media/02-tui-archive-days.png"

node "$here/web.mjs" "$repo" "$here/media"
