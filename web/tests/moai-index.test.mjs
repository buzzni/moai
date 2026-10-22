// The index is the inverse image of moai's `home` (data/index.ts): these hold it to `moai tui --json --deep` on the
// fixture tracker scripts/gen-web-fixtures.sh builds, so nothing here re-states a placement rule of moai's.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { IssueIndex, LOST, NO_MILESTONE, parentOf, pathKey, percent } = await panelModule('data/index.ts')

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const TREE = await fixture('tree.json')
const ROWS = [...(await fixture('rows.json')), ...(await fixture('backlog.json')).map((row) => ({ ...row, kind: 'backlog' }))]
const ARCHIVED = await fixture('rows-archived.json')

const index = new IssueIndex(ROWS, TREE)
const ids = (entries) => entries.map((entry) => (entry.dir ? entry.seg : index.rows[entry.at].id))

test('every tree line with an id is one row, in the tree\'s order; buckets are not rows', () => {
  assert.deepEqual(index.rows.map((row) => row.id), TREE.filter((line) => line.id).map((line) => line.id))
})

test('a row stands where moai\'s home says, its handles read as the kind of the row they name', () => {
  const home = (id) => index.homeOf(index.find(id))
  assert.deepEqual(home('fx-ms01'), [])
  assert.deepEqual(home('fx-ep01'), ['m:fx-ms01'])
  assert.deepEqual(home('fx-ep01.aaa'), ['m:fx-ms01', 'e:fx-ep01'])
  assert.deepEqual(home('fx-ep01.aaa.bbb'), ['m:fx-ms01', 'e:fx-ep01', 'i:fx-ep01.aaa'])
  assert.deepEqual(home('fx-ep02.aaa'), [NO_MILESTONE, 'e:fx-ep02'])
  assert.deepEqual(home('fx-is03'), [LOST])
  // The inverse holds for every line: each home is exactly where moai put it.
  for (const line of TREE.filter((one) => one.id)) {
    const at = index.find(line.id)
    // The pieces, not only how many: each segment without its `m:`/`e:`/`i:` mark is the name moai gave (`m:` alone is its `none`).
    assert.deepEqual(index.homeOf(at).map((seg) => (seg === NO_MILESTONE ? 'none' : seg.replace(/^[mei]:/, ''))), line.home, line.id)
  }
})

test('a listing is the rows whose home is that place, in moai\'s order, buckets where the tree puts them', () => {
  assert.deepEqual(ids(index.entries([])), ['m:fx-ms01', NO_MILESTONE, LOST])
  assert.deepEqual(ids(index.entries(['m:fx-ms01'])), ['e:fx-ep01'])
  assert.deepEqual(ids(index.entries(['m:fx-ms01', 'e:fx-ep01'])), ['i:fx-ep01.aaa', 'fx-ep01.bbb'])
  const none = TREE.filter((line) => line.home.length === 1 && line.home[0] === 'none').map((line) => line.id)
  assert.deepEqual(ids(index.entries([NO_MILESTONE])).map((seg) => seg.replace(/^[mei]:/, '')), none)
  assert.deepEqual(ids(index.entries([LOST])), ['fx-is03'])
})

test('a listing in another order is the order moai gave the tree it was built from', async () => {
  const other = new IssueIndex(ROWS, await fixture('tree-updated.json'))
  const none = (await fixture('tree-updated.json')).filter((line) => line.home.length === 1 && line.home[0] === 'none').map((line) => line.id)
  assert.deepEqual(other.entries([NO_MILESTONE]).map((entry) => other.rows[entry.at].id), none)
  assert.notDeepEqual(none, ids(index.entries([NO_MILESTONE])).map((seg) => seg.replace(/^[mei]:/, '')), 'the two orders differ, or this measures nothing')
})

test('a folder opens when moai says dir; its n/n is moai\'s progress; a bucket counts what it holds', () => {
  assert.equal(index.isDir(index.find('fx-ep01.aaa')), true)
  assert.equal(index.isDir(index.find('fx-ep01.bbb')), false)
  assert.deepEqual(index.tally(['m:fx-ms01']), [0, 3])
  assert.deepEqual(index.tally(['m:fx-ms01', 'e:fx-ep01', 'i:fx-ep01.aaa']), [0, 1])
  const [done, total] = index.tally([NO_MILESTONE])
  assert.ok(total >= 4 && done >= 1, `${done}/${total}`)
  assert.equal(percent([1, 4]), 25)
  assert.equal(percent([0, 0]), undefined)
})

test('deferral, epic and milestone are the keys moai printed', () => {
  assert.equal(index.shelvedAt(index.find('fx-ep02.aaa')), 'fx-ep02')
  assert.equal(index.isDeferred(index.find('fx-ep02')), true)
  assert.equal(index.isDeferred(index.find('fx-is01')), false)
  assert.equal(index.epicOf(index.find('fx-ep01.aaa.bbb')), 'fx-ep01')
  assert.equal(index.milestoneOf(index.find('fx-ep01.aaa.bbb')), 'fx-ms01')
  assert.equal(index.milestoneOf(index.find('fx-is01')), undefined)
  assert.equal(index.deferredRoot('fx-ep02.aaa'), 'fx-ep02')
})

test('a line no list printed is the archive: it stands with the tree\'s keys and archived, until the archive is read', () => {
  const at = index.find('fx-is02')
  assert.equal(index.rows[at].archived, true)
  assert.equal(index.rows[at].title, 'Closed three days ago')
  assert.equal(index.rows[at].status, 'done')
  const read = new IssueIndex([...ROWS, ...ARCHIVED.filter((row) => row.archived)], TREE)
  const row = read.rows[read.find('fx-is02')]
  assert.equal(row.archived, true)
  assert.equal(typeof row.created_at, 'string', 'the archive\'s own row took over')
})

test('kept rows and the folders with something kept under them; an empty bucket is left out', () => {
  const keep = (at) => index.rows[at].id === 'fx-ep01.aaa.bbb'
  assert.deepEqual(ids(index.entries([], keep)), ['m:fx-ms01'])
  assert.deepEqual(ids(index.entries(['m:fx-ms01', 'e:fx-ep01'], keep)), ['i:fx-ep01.aaa'])
  const lines = index.tree([], () => true, keep).map((line) => (line.entry.at === undefined ? line.entry.seg : index.rows[line.entry.at].id))
  assert.deepEqual(lines, ['fx-ms01', 'fx-ep01', 'fx-ep01.aaa', 'fx-ep01.aaa.bbb'])
})

test('descendants, underGroup and pathOf read the same places', () => {
  const under = index.descendants(['m:fx-ms01']).map((at) => index.rows[at].id)
  assert.deepEqual(under, ['fx-ep01', 'fx-ep01.aaa', 'fx-ep01.aaa.bbb', 'fx-ep01.bbb'])
  assert.deepEqual(index.underGroup('fx-ep01').map((at) => index.rows[at].id), ['fx-ep01', 'fx-ep01.aaa', 'fx-ep01.aaa.bbb', 'fx-ep01.bbb'])
  assert.deepEqual(index.underGroup('fx-is01'), [])
  assert.deepEqual(index.pathOf('none'), [NO_MILESTONE])
  assert.deepEqual(index.pathOf('fx-ep01.aaa'), ['m:fx-ms01', 'e:fx-ep01', 'i:fx-ep01.aaa'])
  assert.equal(index.pathOf('fx-nope'), undefined)
  assert.equal(pathKey(['a', 'b']), 'a\u001fb')
})

test('a row written between the two reads, that the tree does not hold yet, stands at the root', () => {
  const late = new IssueIndex([...ROWS, { id: 'fx-new1', title: 'Just added' }], TREE)
  assert.deepEqual(late.homeOf(late.find('fx-new1')), [])
  assert.ok(late.entries([]).some((entry) => entry.at !== undefined && late.rows[entry.at].id === 'fx-new1'))
})

test('twins: each line keeps its own row, and the folder is the line moai opens even when it is drawn first', () => {
  // This binary's answers for a tracker whose two `fx-ep01` epic lines are B (earlier in the file, p3) and A (later,
  // p0): moai opens A (`find` is the later line in the file), and draws it first by priority.
  const tree = [
    { id: 'fx-ep01', title: 'Twin A (later in file, p0)', kind: 'epic', dir: true, path: 'fx-ep01', status: 'todo', derived_status: 'todo', priority: 0, progress: { done: 0, total: 1 }, home: [] },
    { id: 'fx-ep01.aaa', title: 'Member', kind: 'issue', dir: false, path: 'fx-ep01.aaa', status: 'todo', priority: 2, home: ['fx-ep01'] },
    { id: 'fx-is01', title: 'Plain', kind: 'issue', dir: false, path: 'fx-is01', status: 'todo', priority: 2, home: [] },
    { id: 'fx-ep01', title: 'Twin B (earlier in file, p3)', kind: 'epic', dir: false, path: 'fx-ep01', status: 'todo', derived_status: 'todo', priority: 3, home: [] },
  ]
  const rows = [
    { id: 'fx-ep01', title: 'Twin A (later in file, p0)', kind: 'epic', status: 'todo', priority: 0 },
    { id: 'fx-ep01', title: 'Twin B (earlier in file, p3)', kind: 'epic', status: 'todo', priority: 3 },
    { id: 'fx-ep01.aaa', title: 'Member', kind: 'issue', status: 'todo', priority: 2 },
    { id: 'fx-is01', title: 'Plain', kind: 'issue', status: 'todo', priority: 2 },
  ]
  const twins = new IssueIndex(rows, tree)
  const drawn = twins.tree([], () => true).map((line) => `${twins.rows[line.entry.at].title}${line.entry.dir ? '/' : ''}`)
  assert.deepEqual(drawn, ['Twin A (later in file, p0)/', 'Member', 'Plain', 'Twin B (earlier in file, p3)'])
  assert.equal(twins.rows[twins.find('fx-ep01')].title, 'Twin A (later in file, p0)', 'the id finds the folder line')
  assert.deepEqual(twins.tally(['e:fx-ep01']), [0, 1], 'its n/n is the folder line\'s progress')
})

test('helpers: id parents', () => {
  assert.equal(parentOf('a-1.b2.c3'), 'a-1.b2')
  assert.equal(parentOf('a-1'), undefined)
})
