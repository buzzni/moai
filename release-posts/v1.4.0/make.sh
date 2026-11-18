#!/usr/bin/env bash
# Rebuild the v1.4.0 pictures: the explorer's file window and diff pane (a recorded TUI scene), and the web
# interface's Files tab and issue list with a diff pane (Playwright). Every scene runs on a fictional repository:
# scripts/site-demo/seed.sh builds it and extend.sh gives it source files, commits naming an issue, and a few
# uncommitted changes. This repository's own tracker is never shown.
#
#     MOAI=<v1.4.0 binary> release-posts/v1.4.0/make.sh     (Playwright from web/node_modules, or NODE_PATH)
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-18T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
ctldir=$(mktemp -d /tmp/m140c.XXXXXX)
trap 'rm -rf "$scratch" "$ctldir"' EXIT
mkdir -p "$here/media"

seed() { bash "$root/scripts/site-demo/seed.sh" "$1" >/dev/null && bash "$here/extend.sh" "$1"; }

# --- the TUI scenes -------------------------------------------------------
record() { # <scene> <WxH>
  local repo="$scratch/$1/lantern"
  seed "$repo"
  asciinema rec --headless --window-size "$2" --overwrite -q \
    -c "$(printf '%q ' "$here/drive.py" "$1" "$repo")" "$scratch/$1.raw" >/dev/null
  jq -c 'if type == "object" and has("version") then del(.command, .env, .timestamp) else . end' "$scratch/$1.raw" > "$scratch/$1.cast"
}
record files 130x36
record list 170x40
shot() { node "$root/scripts/release-post/shot.mjs" tui "$@"; }

# 01: the file window — the tree with the issue list's branch lines, change marks and file colours, the file in colour
shot "$scratch/files.cast" --at 12 --title 'moai tui — SPC g f' --mark 'Files · lantern/' -o "$here/media/01-tui-files.png"
# 02: changed files only (SPC v c) brings up the diff pane
shot "$scratch/files.cast" --at 27 --title 'moai tui — SPC v c' --mark 'changed files only' -o "$here/media/02-tui-files-diff.png"
# 03: the issue list with the diff pane (SPC v D) beside the detail
shot "$scratch/list.cast" --at 17 --title 'moai tui — SPC v D' --mark 'diff · commits' -o "$here/media/03-tui-list-diff.png"

# --- the web scenes -------------------------------------------------------
repo="$scratch/web/lantern"
seed "$repo"
node "$here/web.mjs" "$repo" "$here/media" "$ctldir"
