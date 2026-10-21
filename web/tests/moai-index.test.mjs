import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { IssueIndex, LOST, NO_MILESTONE, archiveCandidate, isGroup, parentOf, pathKey, percent } = await panelModule('data/index.ts')
const { rowsFromPrime } = await panelModule('data/source.ts')
const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const tree = await fixture('tree-v016.json')
const rows = [...tree.rows, ...tree.backlog]
const index = new IssueIndex(rows)
const idOf = (title) => rows.find((row) => row.title === title).id
const listing = (path, keep) => index.entries(path, keep).map((entry) => (entry.at === undefined ? (entry.seg === LOST ? 'lost/' : 'none/') : `${rows[entry.at].id}${entry.dir ? '/' : ''}`))

test('every directory lists what `moai tui --json --path` lists, in the same order', () => {
  for (const [name, want] of Object.entries(tree.tui)) {
    const path = index.pathOf(name)
    assert.ok(path, `no path for ${name || '(root)'}`)
    assert.deepEqual(listing(path), want.map((entry) => `${entry.id ?? entry.path}${entry.dir ? '/' : ''}`), `listing of ${name || '(root)'}`)
  }
})

test('every row lands exactly once', () => {
  const seen = new Map()
  const walk = (path) => {
    for (const entry of index.entries(path)) {
      if (entry.at !== undefined) seen.set(entry.at, (seen.get(entry.at) ?? 0) + 1)
      if (entry.dir) walk(entry.at === undefined ? [...path, entry.seg] : index.dirPath(entry.at))
    }
  }
  walk([])
  assert.equal(seen.size, rows.length)
  assert.ok([...seen.values()].every((count) => count === 1))
})

test('the epic beats the milestone a member wrote for itself', () => {
  const at = index.find(idOf('Issue with own release says two but epic one'))
  const epic = idOf('Epic in release one')
  assert.equal(rows[at].milestone, idOf('Release two (deferred)'))
  assert.deepEqual(index.homeOf(at), [`m:${idOf('Release one')}`, `e:${epic}`])
  assert.equal(index.milestoneOf(at), idOf('Release one'))
  assert.equal(index.epicOf(at), epic)
})

test('a child folds under its parent in the same epic and goes to its own epic otherwise', () => {
  const parent = idOf('Member one')
  assert.deepEqual(index.homeOf(index.find(idOf('Child of member one'))).at(-1), `i:${parent}`)
  assert.deepEqual(index.homeOf(index.find(idOf('Child in another epic'))), [NO_MILESTONE, `e:${idOf('Epic without release')}`])
  assert.ok(index.isDir(index.find(parent)))
  // A backlog row under an issue folds there too; one with an epic of its own and no parent stands in the bucket.
  assert.equal(index.homeOf(index.find(idOf('Thought under member one'))).at(-1), `i:${parent}`)
  assert.deepEqual(index.homeOf(index.find(idOf('Thought for epic one'))), [NO_MILESTONE])
})

test('references that cannot be placed go to the lost bucket instead of vanishing', () => {
  for (const title of ['Points at a missing epic', 'Points at an issue as epic', 'Points at a missing release', 'Child of the lost one']) {
    assert.deepEqual(index.homeOf(index.find(idOf(title))), [LOST], title)
  }
  // A child whose parent does not exist still gets a home.
  assert.deepEqual(index.homeOf(index.find('mft-orph.bbb')), [NO_MILESTONE])
  assert.deepEqual(listing([]).slice(-2), ['none/', 'lost/'])
})

test('an empty epic is still a directory', () => {
  const at = index.find(idOf('Empty epic'))
  assert.ok(index.isDir(at))
  assert.deepEqual(index.entries(index.dirPath(at)), [])
})

test('counts take work only — not groups, not backlog — and the listing and the rollup agree', () => {
  const epic = index.dirPath(index.find(idOf('Epic in release one')))
  const progress = index.progress(epic)
  assert.deepEqual(index.tally(epic), [progress.done, progress.work.length])
  assert.deepEqual(index.tally(epic), [1, 6])
  assert.equal(percent(progress), 16)
  // The thought folded under a member is counted under it, not as work.
  assert.equal(progress.kids, 7)
  const empty = index.progress(index.dirPath(index.find(idOf('Empty epic'))))
  assert.equal(percent(empty), undefined)
  const release = index.dirPath(index.find(idOf('Release one')))
  // The same numbers `moai status` prints for the release.
  assert.deepEqual(index.tally(release), [1, 8])
})

test('deferral is inherited from the row itself, its epic, its milestone or a parent', () => {
  const own = idOf('Deferred itself')
  assert.equal(index.shelvedAt(index.find(own)), own)
  const release = idOf('Release two (deferred)')
  // Release two is deferred: its epic and that epic's member are out of the plan with it.
  assert.equal(index.shelvedAt(index.find(idOf('Epic in release two'))), release)
  assert.equal(index.shelvedAt(index.find(idOf('Member of deferred release epic'))), release)
  assert.equal(index.deferredRoot(idOf('Member of deferred release epic')), release)
  // A member that only wrote release two stands in epic one, so nothing defers it.
  assert.equal(index.shelvedAt(index.find(idOf('Issue with own release says two but epic one'))), undefined)
  assert.equal(index.shelvedAt(index.find(idOf('Member one'))), undefined)
})

test('a done row is not deferred, whatever it says', () => {
  const local = new IssueIndex([
    { id: 'x-1', kind: 'issue', status: 'done', deferred_at: '2026-10-01T00:00:00Z' },
    { id: 'x-1.a', kind: 'issue', status: 'todo' },
  ])
  assert.equal(local.shelvedAt(0), undefined)
  assert.equal(local.shelvedAt(1), undefined)
})

test('a filter keeps a folder whose rows match and drops one with nothing under it', () => {
  const inProgress = (at) => rows[at].status === 'in_progress'
  assert.deepEqual(listing([], inProgress), [`${idOf('Release one')}/`, 'none/'])
  const release = index.dirPath(index.find(idOf('Release one')))
  assert.deepEqual(listing(release, inProgress), [`${idOf('Epic in release one')}/`])
  assert.deepEqual(listing([], () => false), [])
})

test('a folder kept by its own match stays even with nothing under it', () => {
  const emptyEpic = index.find(idOf('Empty epic'))
  const release = index.dirPath(index.find(idOf('Release one')))
  assert.deepEqual(listing(release, (at) => at === emptyEpic), [`${idOf('Empty epic')}/`])
})

test('the tree unfolds open folders in place and says where branches continue', () => {
  const release = index.dirPath(index.find(idOf('Release one')))
  const epic = index.dirPath(index.find(idOf('Epic in release one')))
  const lines = index.tree(release, (path) => pathKey(path) === pathKey(epic))
  const shape = lines.map(({ entry, twig }) => [rows[entry.at].id, twig.length])
  assert.deepEqual(shape.slice(0, 3), [[idOf('Loose issue in release one'), 1], [idOf('Epic in release one'), 1], [idOf('Member one'), 2]])
  assert.equal(shape.length, 3 + 5)
  const last = lines.findLast(({ twig }) => twig.length === 2)
  assert.equal(last.twig.at(-1), false)
})

test('reading a group takes the group row and everything drawn under it', () => {
  const epic = idOf('Epic in release one')
  const under = index.underGroup(epic).map((at) => rows[at].id)
  assert.equal(under[0], epic)
  assert.ok(under.includes(idOf('Child of member one')))
  assert.deepEqual(index.underGroup(idOf('Member one')), [])
})

test('a repository without milestones has no milestone level', () => {
  const local = new IssueIndex([
    { id: 'p-e', kind: 'epic', status: 'todo' },
    { id: 'p-1', kind: 'issue', status: 'todo', epic: 'p-e' },
    { id: 'p-2', kind: 'issue', status: 'todo', priority: 0 },
    { id: 'p-b', kind: 'backlog', status: 'todo' },
  ])
  assert.deepEqual(local.entries([]).map((entry) => local.rows[entry.at].id), ['p-e', 'p-2', 'p-b'])
  assert.deepEqual(local.homeOf(1), ['e:p-e'])
})

test('a later row of another kind takes the id and the earlier one goes to the lost bucket', () => {
  const local = new IssueIndex([
    { id: 'd-1', kind: 'issue', status: 'todo' },
    { id: 'd-1', kind: 'epic', status: 'todo' },
  ])
  assert.deepEqual(local.homeOf(0), [LOST])
  assert.deepEqual(local.homeOf(1), [])
  assert.equal(local.find('d-1'), 1)
})

test('prime rows that name an epic prime does not list stand in the plan, not in the lost bucket', () => {
  // Moai v0.8.0 prime prints `epic` and `derived_epic` on a member, but never the epic row itself.
  const loaded = rowsFromPrime({
    picked: [{ id: 'p-1', title: 'Held', status: 'in_progress', priority: 1, epic: 'p-epic' }],
    ready: [
      { id: 'p-2', title: 'Ready', status: 'todo', priority: 2, derived_epic: 'p-epic' },
      { id: 'p-2.a', title: 'Child', status: 'todo', priority: 2, derived_epic: 'p-epic' },
    ],
    others: [], rest: 0, others_rest: 0,
  })
  const local = new IssueIndex(loaded.value.rows)
  assert.deepEqual(local.entries([]).map((entry) => (entry.at === undefined ? entry.seg : local.rows[entry.at].id)), ['p-2', 'p-1'])
  assert.deepEqual(local.homeOf(local.find('p-2.a')), ['i:p-2'])
})

test('helpers: id parents and archive candidates', () => {
  assert.equal(parentOf('a-1.b2.c3'), 'a-1.b2')
  assert.equal(parentOf('a-1'), undefined)
  const now = new Date('2026-10-14T00:00:00Z')
  assert.equal(archiveCandidate({ id: 'a', status: 'done', status_since: '2026-09-01T00:00:00Z' }, now, 30), true)
  assert.equal(archiveCandidate({ id: 'a', status: 'done', status_since: '2026-10-01T00:00:00Z' }, now, 30), false)
  assert.equal(archiveCandidate({ id: 'a', status: 'todo', status_since: '2026-01-01T00:00:00Z' }, now, 30), false)
})

// moai 0.17's own derivations (derived_milestone, shelved_by, archived), each printed only when it holds.
// Present, the index uses them; absent, it derives the same values itself (an older moai reads the same).
test('moai derived_milestone wins when present, own derivation otherwise', () => {
  const idx = new IssueIndex([
    { id: 'm1', kind: 'milestone' },
    { id: 'e1', kind: 'epic', milestone: 'm1' },
    { id: 'a', kind: 'issue', epic: 'e1' },
    { id: 'b', kind: 'issue', epic: 'e1', derived_milestone: 'elsewhere' },
    { id: 'c', kind: 'issue', epic: 'e1', derived_milestone: null },
  ])
  assert.equal(idx.milestoneOf(idx.find('a')), 'm1', 'absent: fallback derivation')
  assert.equal(idx.milestoneOf(idx.find('b')), 'elsewhere', 'string: moai value')
  assert.equal(idx.milestoneOf(idx.find('c')), undefined, 'null: no milestone')
})

test('moai shelved_by names the shelf when present, own derivation otherwise', () => {
  const idx = new IssueIndex([
    { id: 'p', kind: 'issue', deferred_at: '2026-10-01T00:00:00Z' },
    { id: 'p.kid', kind: 'issue' },
    { id: 'free', kind: 'issue' },
    { id: 'told', kind: 'issue', shelved_by: 'm-gone' },
    { id: 'p.named', kind: 'issue', shelved_by: 'p' },
  ])
  assert.equal(idx.isDeferred(idx.find('p.kid')), true, 'absent: inherited from parent')
  assert.equal(idx.isDeferred(idx.find('free')), false)
  // The shelf itself reads the field, so what hides deferred rows (shelvedAt, deferredRoot) agrees, and a shelf
  // that is not among the loaded rows is named all the same.
  assert.equal(idx.shelvedAt(idx.find('told')), 'm-gone')
  assert.equal(idx.deferredRoot('told'), 'm-gone')
  assert.equal(idx.shelvedAt(idx.find('p.named')), 'p')
  assert.equal(idx.shelvedAt(idx.find('p.kid')), 'p')
  // Also when no row in the set is put off by its own field.
  const lone = new IssueIndex([{ id: 'a', kind: 'issue', shelved_by: 'e9' }, { id: 'b', kind: 'issue' }])
  assert.deepEqual([lone.shelvedAt(0), lone.shelvedAt(1)], ['e9', undefined])
})

test('real 0.17 output: shelved_by and derived_milestone agree with what the index derives without them', async () => {
  const real = await fixture('show-all-v017.json')
  assert.ok(real.some((row) => row.shelved_by && row.shelved_by !== row.id), 'a row put off by a group above')
  assert.ok(real.some((row) => row.shelved_by === row.id), 'a row put off by itself')
  const told = new IssueIndex(real)
  const own = new IssueIndex(real.map(({ shelved_by: _s, derived_milestone: _m, ...row }) => row))
  real.forEach((row, at) => {
    assert.equal(told.shelvedAt(at), row.shelved_by, row.id)
    assert.equal(own.shelvedAt(at), row.shelved_by, `${row.id} derived`)
    assert.equal(own.milestoneOf(at), told.milestoneOf(at), `${row.id} milestone`)
    // Alone, no group above is loaded for the walk to find: only the fields can answer.
    const lone = new IssueIndex([row])
    assert.equal(lone.shelvedAt(0), row.shelved_by, `${row.id} alone`)
    assert.equal(lone.milestoneOf(0), row.derived_milestone, `${row.id} milestone alone`)
  })
})

test('archived: true is an archive row whatever the clock reads, own derivation otherwise', async () => {
  const now = new Date('2026-10-14T00:00:00Z')
  const old = { id: 'a', status: 'done', status_since: '2026-09-01T00:00:00Z' }
  assert.equal(archiveCandidate(old, now, 30), true)
  assert.equal(archiveCandidate({ id: 'b', status: 'done', status_since: '2026-10-13T00:00:00Z', archived: true }, now, 30), true)
  assert.equal(archiveCandidate({ id: 'c', status: 'done', status_since: '2026-10-13T00:00:00Z' }, now, 30), false)
  // Real 0.17 (archive_days = 30, captured 2026-10-14): the clock read here marks the row moai marks.
  const real = await fixture('show-archived-v017.json')
  const at = new Date('2026-10-14T12:10:00Z')
  for (const row of real) {
    const { archived: _a, ...bare } = row
    assert.equal(archiveCandidate(row, at, 30), row.archived === true, row.id)
    if (!isGroup(row)) assert.equal(archiveCandidate(bare, at, 30), row.archived === true, `${row.id} derived`)
  }
})

test('without the field, archiveCandidate follows report::archived: off at 0 days, whole days, a group by its members', () => {
  const now = new Date('2026-10-14T00:00:00Z')
  const done = { id: 'a', status: 'done', status_since: '2026-09-30T00:00:00Z' }
  assert.equal(archiveCandidate(done, now, 14), true, 'exactly 14 whole days')
  assert.equal(archiveCandidate(done, now, 15), false)
  assert.equal(archiveCandidate(done, now, 0), false, '0 days turns the archive off')
  // A group stands in the column its members give it, since the latest day one of them got there.
  const epic = { id: 'e', kind: 'epic', status_since: '2026-01-01T00:00:00Z', created_at: '2026-01-01T00:00:00Z' }
  const members = [{ id: 'e.1', status: 'done', status_since: '2026-09-01T00:00:00Z' }, { id: 'e.2', status: 'done', status_since: '2026-10-10T00:00:00Z' }]
  assert.equal(archiveCandidate({ ...epic, status: 'todo', derived_status: 'done' }, now, 14, members.slice(0, 1)), true, 'its members put it in done')
  assert.equal(archiveCandidate({ ...epic, status: 'todo', derived_status: 'done' }, now, 14, members), false, 'a member done 4 days ago keeps it')
  assert.equal(archiveCandidate({ ...epic, status: 'done', derived_status: 'in_progress' }, now, 14, members.slice(0, 1)), false)
})

test('milestone and epic tallies take the counts `moai status` gave; buckets and no-count ids keep their own (se-tcd4)', () => {
  const epicAt = index.find(idOf('Epic in release one'))
  const epic = index.dirPath(epicAt)
  const release = index.dirPath(index.find(idOf('Release one')))
  const counts = new Map([[rows[epicAt].id, [3, 9]], [rows[index.find(idOf('Release one'))].id, [4, 12]]])
  const counted = new IssueIndex(rows, counts)
  assert.deepEqual(counted.tally(epic), [3, 9])
  assert.deepEqual(counted.tally(release), [4, 12])
  // Without counts, or for an id they lack, the old count.
  assert.deepEqual(new IssueIndex(rows).tally(epic), [1, 6])
  assert.deepEqual(new IssueIndex(rows, new Map()).tally(release), [1, 8])
  assert.deepEqual(new IssueIndex(rows, new Map([['other', [9, 9]]])).tally(epic), [1, 6])
  // Buckets are never replaced, whatever the map holds; progress() stays the drawn rows.
  const withBucket = new IssueIndex(rows, new Map([['', [99, 99]], ['lost', [99, 99]]]))
  assert.deepEqual(withBucket.tally([NO_MILESTONE]), index.tally([NO_MILESTONE]))
  assert.deepEqual(withBucket.tally([LOST]), index.tally([LOST]))
  assert.deepEqual(counted.tally([]), index.tally([]))
  assert.equal(counted.progress(epic).work.length, 6)
})
