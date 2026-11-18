#!/usr/bin/env bash
# Rebuild the v1.5.0 pictures: the explorer's file window and diff panes on a Subversion working copy (recorded
# TUI scenes), and the web interface's Files tab and header (Playwright). Every scene runs on a fictional
# repository: svn.sh seeds the tracker with scripts/site-demo/seed.sh, moves it into a local svn repository
# (svnadmin create, a file:// URL) and extend-svn.sh gives it source files, revisions naming an issue, and a
# few uncommitted changes. This repository's own tracker is never shown.
#
#     MOAI=<v1.5.0 binary> release-posts/v1.5.0/make.sh     (svn 1.9+, Playwright from web/node_modules or NODE_PATH)
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-18T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
# the revision table lives in the user's cache directory: keep the demo's out of it
scratch=$(mktemp -d)
ctldir=$(mktemp -d /tmp/m150c.XXXXXX)
trap 'rm -rf "$scratch" "$ctldir"' EXIT
export PLAYWRIGHT_BROWSERS_PATH=${PLAYWRIGHT_BROWSERS_PATH:-${XDG_CACHE_HOME:-$HOME/.cache}/ms-playwright}
export XDG_CACHE_HOME=$scratch/cache
mkdir -p "$here/media"

seed() { bash "$here/svn.sh" "$1"; }

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

# 01: changed files only (SPC v c) on a working copy: svn's A, M, D and ? marks beside the diff of the file
shot "$scratch/files.cast" --at "${FILES_AT:-29}" --title 'moai tui — SPC v c (svn)' --mark 'changed files only' -o "$here/media/01-tui-svn-files.png"
# 02: the issue list with the diff pane (SPC v D): the revisions whose log message names the issue
shot "$scratch/list.cast" --at "${LIST_AT:-17}" --title 'moai tui — SPC v D (svn)' --mark 'diff · commits' -o "$here/media/02-tui-svn-list-diff.png"

# --- the web scenes -------------------------------------------------------
repo="$scratch/web/lantern"
seed "$repo"
# a second registered project, so the header shows the project list beside User and Version
other="$scratch/web/atlas"
mkdir -p "$other"
export MOAI_CONFIG="$repo/.demo-home/config.toml"
"$MOAI" -C "$other" init atl --yes --guide none --no-skill --no-register --tracking commit >/dev/null
for project in "$repo" "$other"; do "$MOAI" project add "$project" >/dev/null; done
node "$here/web.mjs" "$repo" "$here/media" "$ctldir"
