#!/usr/bin/env bash
# Rebuild the v0.50.0 pictures: the web list and kanban with folded groups, the space below hatched.
# MOAI=<v0.50.0 binary> release-posts/v0.50.0/make.sh   (Playwright from web/node_modules, or NODE_PATH)
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-04T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
ctldir=$(mktemp -d /tmp/m050c.XXXXXX)
trap 'rm -rf "$scratch" "$ctldir"' EXIT
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
node "$here/web.mjs" "$repo" "$here/media" "$ctldir"
