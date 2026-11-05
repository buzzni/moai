#!/usr/bin/env bash
# Rebuild the v0.51.0 pictures in the fictional Lantern project: the explorer's sort window (SPC s),
# the web sort and grouping window, and the web kanban grouped by assignee.
# MOAI=<v0.51.0 binary> release-posts/v0.51.0/make.sh   (Playwright from web/node_modules, or NODE_PATH)
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-05T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
socket="moai051-$$"
tmux_cmd=(env -u TMUX tmux -L "$socket")
# the ctl socket path must stay short, so it does not live under the scratch dir
ctldir=$(mktemp -d /tmp/m051c.XXXXXX)
cleanup() {
  "${tmux_cmd[@]}" kill-server >/dev/null 2>&1 || true
  rm -rf "$scratch" "$ctldir"
}
trap cleanup EXIT
mkdir -p "$here/media"
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
export MOAI_CONFIG="$repo/.demo-home/config.toml" MOAI_CTL_DIR="$ctldir"

printf -v tui_command '%q ' env MOAI_CONFIG="$MOAI_CONFIG" MOAI_CTL_DIR="$MOAI_CTL_DIR" MOAI_NO_UPDATE_CHECK=1 MOAI_LANG=en "$MOAI" tui
"${tmux_cmd[@]}" new-session -d -s scene -x 170 -y 42 -c "$repo" "$tui_command"
# a loaded machine takes a few seconds to draw the first screen; wait for the list before pressing keys
for _ in $(seq 30); do
  "${tmux_cmd[@]}" capture-pane -p -t scene:0.0 2>/dev/null | grep -q 'rows ━' && break
  sleep 1
done
sleep 1
keys() { "${tmux_cmd[@]}" send-keys -t scene:0.0 "$@"; sleep 2; }
capture() {
  local name=$1 title=$2 mark=$3 width height
  width=$("${tmux_cmd[@]}" display-message -p -t scene:0.0 '#{pane_width}')
  height=$("${tmux_cmd[@]}" display-message -p -t scene:0.0 '#{pane_height}')
  "${tmux_cmd[@]}" capture-pane -e -p -t scene:0.0 > "$scratch/pane.txt"
  python3 - "$scratch/pane.txt" "$scratch/$name.cast" "$width" "$height" <<'PY'
import json, pathlib, sys
pane, cast, width, height = sys.argv[1:]
screen = pathlib.Path(pane).read_text().replace('\n', '\r\n')
with open(cast, 'w') as out:
    out.write(json.dumps({'version': 2, 'width': int(width), 'height': int(height)}) + '\n')
    out.write(json.dumps([0.0, 'o', '\x1b[2J\x1b[H' + screen]) + '\n')
    out.write(json.dumps([0.5, 'o', '']) + '\n')
PY
  node "$root/scripts/release-post/shot.mjs" tui "$scratch/$name.cast" \
    --at 0.2 --title "$title" --mark "$mark" -o "$here/media/$name.png"
}
# SPC s opens the sort window on the milestone layer, with the presets on top
keys Space
keys s
"${tmux_cmd[@]}" capture-pane -p -t scene:0.0 | grep -q 'sort order' || { "${tmux_cmd[@]}" capture-pane -p -t scene:0.0 >&2; echo 'sort window did not open' >&2; exit 1; }
capture 03-tui-sort-window 'moai tui — SPC s, sort and group' 'sort order'
"${tmux_cmd[@]}" kill-server

node "$here/web.mjs" "$repo" "$here/media" "$ctldir"
