#!/usr/bin/env bash
# Rebuild the v0.36.0 board pictures from the Lantern demo. The three coloured
# columns are in one frame on both surfaces. No real project or personal tmux server is used.
#
#   NODE_PATH=<playwright node_modules> MOAI=<v0.36.0 binary> release-posts/v0.36.0/make.sh
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI
export DEMO_NOW=2026-10-30T12:00:00Z
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$here/media"

# The existing board scene opens SPC g b, then closes the detail pane so the
# in_progress, review, and done columns fit across the TUI frame.
SIZE=120x40 "$root/scripts/site-demo/record-tui.sh" "$tmp" board
node "$root/scripts/release-post/shot.mjs" tui "$tmp/tui-board.cast" \
  --at 7.2 --title 'moai tui — SPC g b' \
  --cells 11,51,20,1 --cells 11,75,20,1 --cells 11,98,20,1 \
  -o "$here/media/03-tui-board.png"

web="$tmp/web/lantern"
bash "$root/scripts/site-demo/seed.sh" "$web" >/dev/null
node "$here/web.mjs" "$web" "$here/media"
