#!/usr/bin/env bash
# Give the Lantern svn working copy (svn.sh built it from the seeded repo) real source files, revisions whose
# log message names an issue id, and a few uncommitted changes, so the file window and the diff panes have
# something to show.   extend.sh <working copy>
set -euo pipefail
repo=$(cd -- "${1:?usage: extend.sh <seeded repo>}" && pwd)
cd "$repo"
export TZ=UTC
conflict=$(sed -n 's/^conflict=//p' .demo-ids)
retry=$(cd "$repo" && "${MOAI:?}" show -g 'Retry with backoff' --type issue --json | jq -r '.[0].id')
at() { date -u -d "${DEMO_NOW:-now} -$1 hours" +%Y-%m-%dT%H:%M:%SZ; }
# svn commit, then set the author and the date of the new revision (the repository's pre-revprop-change hook allows it)
commit() {
  local h=$1; shift
  svn add -q --force . >/dev/null
  svn commit -q -m "$1" . >/dev/null
  svn update -q >/dev/null
  local rev; rev=$(svn info --show-item revision)
  svn propset -q --revprop -r "$rev" svn:author mira "$repo" >/dev/null
  svn propset -q --revprop -r "$rev" svn:date "$(at "$h" | sed 's/Z$/.000000Z/')" "$repo" >/dev/null
}
mkdir -p src/sync src/editor src/search test

cat > package.json <<'J'
{
  "name": "lantern",
  "version": "1.3.0",
  "scripts": { "test": "vitest run", "build": "tsc -p ." }
}
J
cat > README.md <<'J'
# Lantern

A local-first notes app. Every device keeps a full copy; the server relays changes.
J
cat > src/sync/queue.ts <<'J'
export interface Edit {
  note: string
  revision: number
  body: string
}

const queue: Edit[] = []

export function enqueue(edit: Edit): void {
  queue.push(edit)
}

export function drain(send: (e: Edit) => Promise<void>): Promise<void> {
  return queue.splice(0).reduce((p, e) => p.then(() => send(e)), Promise.resolve())
}
J
cat > src/sync/merge.ts <<'J'
import type { Edit } from './queue'

// Last write wins.
export function merge(local: Edit, remote: Edit): Edit {
  return remote.revision > local.revision ? remote : local
}
J
cat > src/sync/backoff.ts <<'J'
export function delay(attempt: number): number {
  return Math.min(2000 * 2 ** attempt, 300000)
}
J
cat > src/editor/paste.ts <<'J'
export function paste(text: string): string[] {
  return text.split('\n\n').map((p) => p.trim())
}
J
cat > src/editor/blocks.py <<'J'
def count(blocks):
    return len(blocks)
J
cat > src/search/index.ts <<'J'
const titles = new Map<string, string>()

export function add(id: string, title: string): void {
  titles.set(id, title.toLowerCase())
}
J
cat > test/legacy.sh <<'J'
#!/bin/sh
npm test
J
commit 40 'chore: add the first sources'

# Two commits that name the conflict issue: they stand under "commits" in its diff.
cat > src/sync/merge.ts <<'J'
import type { Edit } from './queue'

export interface Conflict {
  note: string
  paragraphs: number[]
}

// Merge paragraph by paragraph; a paragraph changed on both sides keeps both.
export function merge(local: Edit, remote: Edit): Edit {
  if (remote.revision <= local.revision) return local
  const mine = local.body.split('\n\n')
  const theirs = remote.body.split('\n\n')
  const out: string[] = []
  for (let i = 0; i < Math.max(mine.length, theirs.length); i++) {
    const a = mine[i]
    const b = theirs[i]
    out.push(a === b || b === undefined ? (a ?? '') : a === undefined ? b : `${a}\n\n${b}`)
  }
  return { ...remote, body: out.join('\n\n') }
}

export function conflicts(local: Edit, remote: Edit): Conflict {
  const mine = local.body.split('\n\n')
  const theirs = remote.body.split('\n\n')
  const paragraphs = mine.flatMap((p, i) => (theirs[i] !== undefined && theirs[i] !== p ? [i] : []))
  return { note: local.note, paragraphs }
}
J
cat > test/merge.test.ts <<'J'
import { expect, test } from 'vitest'
import { merge } from '../src/sync/merge'

test('a paragraph changed on both sides keeps both', () => {
  const a = { note: 'n', revision: 1, body: 'one\n\ntwo' }
  const b = { note: 'n', revision: 2, body: 'one\n\nTWO' }
  expect(merge(a, b).body).toBe('one\n\ntwo\n\nTWO')
})
J
commit 6 "feat(sync): merge concurrent edits paragraph by paragraph ($conflict)"
cat > src/editor/paste.ts <<'J'
export interface Block {
  id: string
  text: string
}

// Keep the paragraph ids across a paste, or the merge sees a delete plus an insert.
export function paste(text: string, ids: string[] = []): Block[] {
  return text.split('\n\n').map((p, i) => ({ id: ids[i] ?? crypto.randomUUID(), text: p.trim() }))
}
J
commit 3 "fix(editor): keep paragraph ids across paste ($conflict)"
cat > src/sync/backoff.ts <<'J'
// Retry with exponential backoff and jitter: 2 s, capped at 5 min.
export function delay(attempt: number, rand: () => number = Math.random): number {
  const base = Math.min(2000 * 2 ** attempt, 300000)
  return Math.round(base * (0.5 + rand() / 2))
}
J
commit 2 "feat(sync): retry with exponential backoff ($retry)"

# Uncommitted changes: M, A (svn add), D, R (svn mv) and one unversioned file.
cat >> src/sync/merge.ts <<'J'

export function banner(c: Conflict): string {
  return `${c.paragraphs.length} paragraph(s) changed on both devices`
}
J
cat > src/sync/status.ts <<'J'
export type SyncState = 'synced' | 'syncing' | 'offline'

export function label(state: SyncState, waiting: number): string {
  if (state === 'offline') return `Offline — ${waiting} changes waiting`
  return state === 'syncing' ? `Syncing ${waiting} notes` : 'Synced'
}
J
cat > src/search/index.ts <<'J'
const titles = new Map<string, string>()

export function add(id: string, title: string): void {
  titles.set(id, title.toLowerCase())
}

export function find(query: string): string[] {
  const q = query.toLowerCase()
  return [...titles].filter(([, t]) => t.includes(q)).map(([id]) => id)
}
J
sed -i 's/"version": "1.3.0"/"version": "1.5.0"/' package.json
svn rm -q --force test/legacy.sh
svn mv -q src/editor/blocks.py src/editor/count.py
printf '\n- [Sync protocol](docs/sync-protocol.md)\n' >> README.md
svn add -q src/sync/status.ts
printf 'TODO: sync protocol notes\n' > NOTES.txt
