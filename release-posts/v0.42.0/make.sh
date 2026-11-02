#!/usr/bin/env bash
# Rebuild the v0.42.0 picture in a fictional Lantern project.
# MOAI=<v0.42.0 binary> release-posts/v0.42.0/make.sh
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-02T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
socket="moai042-$$"
tmux_cmd=(env -u TMUX tmux -L "$socket")
cleanup() {
  "${tmux_cmd[@]}" kill-server >/dev/null 2>&1 || true
  rm -rf "$scratch" "$ctldir"
}
trap cleanup EXIT
# the ctl socket path must stay short, so it does not live under the scratch dir
ctldir=$(mktemp -d /tmp/m042c.XXXXXX)
mkdir -p "$here/media"
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
export MOAI_CONFIG="$repo/.demo-home/config.toml" MOAI_CTL_DIR="$ctldir"
# A screenshot-like picture, rendered here, goes through the real pipeline: one for the issue body, one for the wiki page.
eval "$(grep -E '^conflict=' "$repo/.demo-ids")"
node "$here/gen-image.mjs" "$scratch/conflict-banner.png"
"$MOAI" -C "$repo" image add "$scratch/conflict-banner.png" --to "$conflict" --alt 'Conflict banner on the packing-list note' --user 'Mira Chen (mira@example.com)' >/dev/null
ref=$("$MOAI" -C "$repo" image add "$scratch/conflict-banner.png" --wiki sync-protocol --alt 'Conflict banner on the packing-list note')
python3 - "$repo/docs/sync-protocol.md" "$ref" <<'PY'
import pathlib, sys
page = pathlib.Path(sys.argv[1]); ref = sys.argv[2].strip()
text = page.read_text()
marker = '3. Paragraph ids must survive copy and paste, or a move looks like a delete plus an insert\n'
assert marker in text
page.write_text(text.replace(marker, marker + '\nWhat the user sees when both sides changed a paragraph:\n\n' + ref + '\n', 1))
PY

printf -v tui_command '%q ' env MOAI_CONFIG="$MOAI_CONFIG" MOAI_CTL_DIR="$MOAI_CTL_DIR" MOAI_NO_UPDATE_CHECK=1 MOAI_LANG=en "$MOAI" tui
"${tmux_cmd[@]}" new-session -d -s scene -x 170 -y 40 -c "$repo" "$tui_command"
sleep 2
keys() { "${tmux_cmd[@]}" send-keys -t scene:0.0 "$@"; sleep 1; }
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

to_row() {
  local id=$1 key n
  for key in j k; do
    for n in $(seq 12); do
      "${tmux_cmd[@]}" capture-pane -p -t scene:0.0 | grep -q ">  *$id " && return 0
      keys "$key"
    done
  done
  echo "to_row: $id not found" >&2; "${tmux_cmd[@]}" capture-pane -p -t scene:0.0 >&2; return 1
}
keys Tab
keys Tab
to_row "$conflict"
# focus the detail and scroll until the picture's placeholder line stands on the screen
keys C-w w
for _ in $(seq 8); do
  "${tmux_cmd[@]}" capture-pane -p -t scene:0.0 | grep -q '\[image:' && break
  keys C-d
done
capture 04-tui-picture-placeholder 'moai tui — a picture in the issue body' '[image:'
"${tmux_cmd[@]}" kill-server

node "$here/web.mjs" "$repo" "$here/media" "$ctldir"
