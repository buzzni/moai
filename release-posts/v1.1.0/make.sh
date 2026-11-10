#!/usr/bin/env bash
# Rebuild the v1.1.0 pictures: an issue body with math, a Mermaid diagram, footnotes and a snippet in the web detail.
# MOAI=<v1.1.0 binary> release-posts/v1.1.0/make.sh   (Playwright from web/node_modules, or NODE_PATH)
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../.." && pwd)
MOAI=$(realpath -- "${MOAI:-$root/target/release/moai}")
export MOAI DEMO_NOW=2026-11-10T12:00:00Z
export NODE_PATH=${NODE_PATH:-$root/web/node_modules}
scratch=$(mktemp -d)
ctldir=$(mktemp -d /tmp/m110c.XXXXXX)
trap 'rm -rf "$scratch" "$ctldir"' EXIT
repo="$scratch/lantern"
bash "$root/scripts/site-demo/seed.sh" "$repo" >/dev/null
mkdir -p "$repo/src"
cat > "$repo/src/merge.ts" <<'TS'
// #region pick
export function pick(local: Note, remote: Note): Note {
  return local.clock >= remote.clock ? local : remote
}
// #endregion pick
TS
fence='```'
epic=$(cd "$repo" && "$MOAI" show --type epic --json | jq -r '.[] | select(.title == "Offline sync") | .id')
id=$(cd "$repo" && TZ=UTC MOAI_LANG=en "$MOAI" add 'Merge rule for concurrent edits' -t feature -e "$epic" \
  --user 'Mira Chen (mira@example.com)' --json -b - <<BODY | jq -r .id
Two devices edit the same note offline. The newer clock wins[^lww], and ties keep the local copy.

A note with \$n_i\$ edits on device \$i\$ sends at most this many operations:

\$\$
\sum_{i=1}^{d} n_i \le d \cdot \max_i n_i
\$\$

${fence}mermaid
sequenceDiagram
  participant A as Phone
  participant S as Sync server
  participant B as Laptop
  A->>S: push(note, clock 7)
  B->>S: push(note, clock 9)
  S-->>A: newer copy (clock 9)
${fence}

The rule lives in one function:

<<< src/merge.ts#pick{ts}

**충돌**은 마지막 쓰기가 이긴다.

[^lww]: Last-writer-wins, decided by the logical clock.
BODY
)
node "$here/web.mjs" "$repo" "$here/media" "$ctldir" "$id"
