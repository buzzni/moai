#!/usr/bin/env bash
# Rebuild the v0.39.0 ctl pictures in a fictional Lantern project.
# MOAI=<v0.39.0 binary> release-posts/v0.39.0/make.sh
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-10-31T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
socket="moai039-$$"
tmux_cmd=(env -u TMUX tmux -L "$socket")
cleanup() {
  "${tmux_cmd[@]}" kill-server >/dev/null 2>&1 || true
  rm -rf "$scratch"
}
trap cleanup EXIT
mkdir -p "$here/media" "$scratch/ctl"
chmod 700 "$scratch/ctl"
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
export MOAI_CONFIG="$repo/.demo-home/config.toml" MOAI_CTL_DIR="$scratch/ctl"

# One pane runs the explorer; the adjacent pane runs each actual ctl command.
printf -v tui_command '%q ' env MOAI_CONFIG="$MOAI_CONFIG" MOAI_CTL_DIR="$MOAI_CTL_DIR" MOAI_NO_UPDATE_CHECK=1 MOAI_LANG=en "$MOAI" tui
"${tmux_cmd[@]}" new-session -d -s scene -x 200 -y 42 -c "$repo" "$tui_command"
"${tmux_cmd[@]}" split-window -h -l 78 -t scene:0 -c "$repo"
sleep 2
ctl() {
  local command
  printf -v command '%q ' env MOAI_CONFIG="$MOAI_CONFIG" MOAI_CTL_DIR="$MOAI_CTL_DIR" "$MOAI" ctl "$@"
  "${tmux_cmd[@]}" send-keys -t scene:0.1 "$command" Enter
  sleep 1
}
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

ctl ls
ctl sort title:desc
ctl filter status=todo
capture 01-tui-ctl-filter 'moai ctl — explorer filter and sort' 'status=todo'
ctl find sync --in title
capture 02-tui-ctl-find 'moai ctl — find in the explorer' 'Sync attachments'
ctl propose 'Show only the remaining sync work?' --then 'filter status=todo tag=sync'
capture 03-tui-proposal 'moai ctl propose — suggestion' 'Suggestion 1/1'
"${tmux_cmd[@]}" send-keys -t scene:0.0 Space a l
sleep 1
capture 04-tui-proposal-list 'moai tui — review a suggestion' 'Suggestions'
"${tmux_cmd[@]}" kill-server

node "$here/web.mjs" "$repo" "$here/media" "$scratch/web-ctl"
