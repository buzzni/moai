import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const q = await panelModule('data/query.ts')
const fixture = JSON.parse(await readFile(new URL('./fixtures/moai/query-v016.json', import.meta.url), 'utf8'))
const rows = fixture.rows
const ids = (list) => list.map((row) => row.id)
const filterOf = (items, flags) => {
  const built = q.parseFilter(items, flags)
  assert.ok(built.ok, JSON.stringify(built))
  return built.value
}

/** The reader's wall clock, as the panel hands it to applyFilter (view/list-model.ts readerClock). */
const readerClock = (epoch) => epoch - new Date(epoch * 1000).getTimezoneOffset() * 60

/** What the panel does for one case: filter, then order (the default order when none is given). */
function run(item, list = rows, context) {
  const kept = q.applyFilter(list, filterOf(item.filter, item.flags), context)
  const fields = item.sort === undefined ? q.DEFAULT_SORT.fields : q.parseSort(item.sort).value
  return ids(q.sortRows(kept, { fields, reversed: Boolean(item.reverse) }))
}

/** The CLI's flags for a case (the same words the fixture was built with). */
const cliArgs = (item) => [
  'show',
  '--json',
  ...item.filter.flatMap((one) => ['--filter', one]),
  ...(item.flags?.all ? ['--all'] : []),
  ...(item.flags?.archived ? ['--archived'] : []),
  ...(item.flags?.deferred ? ['--deferred'] : []),
  ...(item.sort ? ['--sort', item.sort] : []),
  ...(item.reverse ? ['--reverse'] : []),
]

test('every case lists what `moai show --json` listed, in the same order', () => {
  for (const item of fixture.cases) assert.deepEqual(run(item), item.ids, item.name)
})

test('the fixture cases cover the set of keys and every sort field', () => {
  const keys = new Set(fixture.cases.flatMap((item) => item.filter.map((one) => one.split('=')[0])))
  // `stale` reads the clock of the day the CLI runs, so it is covered by the fixed-clock test below instead.
  for (const key of q.KEYS.filter((one) => one !== 'stale')) assert.ok(keys.has(key), `no case for ${key}`)
  const sorted = new Set(fixture.cases.flatMap((item) => (item.sort ?? '').split(',').map((one) => one.split(':')[0])))
  for (const key of q.SORT_KEYS) assert.ok(sorted.has(key), `no sort case for ${key}`)
})

// The binary may be absent (CI): then the static expectations above are the whole check.
let moai = true
try {
  execFileSync('moai', ['--version'], { stdio: 'pipe' })
} catch {
  moai = false
}
test('the same cases against the real binary, when it is installed', { skip: !moai }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'moai-query-'))
  try {
    mkdirSync(join(dir, '.moai'))
    // `archive_days = 0` turns the archive off: the cases are expectations without a clock, and the fixture's done
    // rows would otherwise age past the 14-day default and drop out of `all` / `status=done` on a later day.
    writeFileSync(join(dir, '.moai', 'config.toml'), 'prefix = "trk"\nstatuses = "todo,in_progress,review,done"\nnaming = "full"\narchive_days = 0\n')
    writeFileSync(join(dir, '.moai', 'issues.jsonl'), fixture.issues.map((row) => JSON.stringify(row)).join('\n') + '\n')
    for (const item of fixture.cases) {
      const out = JSON.parse(execFileSync('moai', ['-C', dir, ...cliArgs(item)], { encoding: 'utf8' }))
      // The CLI reads a wall time (`2026-10-07 01:00`) on this machine's clock, as the panel does (list-model.ts
      // readerClock); the fixture's ids were taken in UTC, so the other tests read without `local` (se-24ey.1z5).
      assert.deepEqual(run(item, rows, { local: readerClock }), ids(out), item.name)
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test('items cuts a line at each `key=`, keeps spaced values and quoted values whole', () => {
  assert.deepEqual(q.splitItems('status=todo tag=bug'), ['status=todo', 'tag=bug'])
  assert.deepEqual(q.splitItems('grep=atomic write  status=todo'), ['grep=atomic write', 'status=todo'])
  assert.deepEqual(q.splitItems('done_at="2026-10-03 00:00~2026-10-05 23:59" tag=a'), ['done_at=2026-10-03 00:00~2026-10-05 23:59', 'tag=a'])
  assert.deepEqual(q.splitItems('grep="a=b c"'), ['grep=a=b c'])
  assert.deepEqual(q.splitItems('grep="never closed tag=x'), ['grep=never closed tag=x'])
  assert.deepEqual(q.splitItems("grep=don't stop"), ["grep=don't stop"])
  assert.deepEqual(q.splitItems('   '), [])
  const [item] = q.items('grep="a b" ')
  assert.deepEqual([item.start, item.eq, item.end, item.quote], [0, 4, 10, { open: 5, close: 9 }])
  // A text line and the same items already cut read to the same filter.
  assert.deepEqual(q.parseFilter('status=todo tag=bug'), q.parseFilter(['status=todo', 'tag=bug']))
  assert.deepEqual(q.parseFilter('grep=原子的 쓰기').value.grep, '原子的 쓰기')
})

test('a filter that cannot be read is refused with its reason', () => {
  const refused = (items, code, flags) => {
    const built = q.parseFilter(items, flags)
    assert.equal(built.ok, false, String(items))
    assert.equal(built.error.code, code, String(items))
    assert.ok(q.badFilterMessage(built.error).length > 0)
    return built.error
  }
  assert.deepEqual(refused(['status=todo', 'status=review', 'status=done'], 'twice').values, ['todo', 'review', 'done'])
  refused(['grep=a', 'grep=b'], 'twice')
  refused(['type=epic', 'type=issue'], 'twice')
  refused(['stale=1', 'stale=2'], 'twice')
  // A second value of a one-value key is reported before an item that is not a pair, as the CLI does.
  refused('x grep=a grep=b', 'twice')
  refused(['x', 'type=epic', 'type=issue'], 'twice')
  refused(['x', 'statu=1'], 'not-a-pair')
  refused(['statu=1', 'x'], 'no-such-key')
  refused(['priority=1', 'priority=2'], 'twice')
  refused(['statu=todo'], 'no-such-key')
  refused(['status'], 'not-a-pair')
  refused(['type=story'], 'not-a-kind')
  refused(['stale=soon'], 'not-days')
  refused(['priority=9'], 'not-a-priority')
  refused(['priority=pp1'], 'not-a-priority')
  refused(['since=yesterday'], 'not-a-time')
  refused(['since=2026-02-30'], 'not-a-time')
  refused(['since='], 'no-time')
  refused(['created=~'], 'endless')
  refused(['created=2026-10-05..2026-10-01'], 'backwards')
  refused(['created=2026-12-01..2026-01-01T00:00:00Z'], 'backwards')
  assert.equal(refused(['status=todo', 'done=2026-10-01'], 'done-outside').asked, 'todo')
  assert.ok(q.parseFilter(['status=done,todo', 'done=2026-10-01']).ok)
  assert.ok(q.parseFilter([]).ok)
  assert.ok(q.parseFilter(['', '  ']).ok)
})

test('the scopes cycle in the order of the explorer, without the note scope nothing loads notes for', () => {
  assert.deepEqual(q.GREP_SCOPES, ['all', 'id', 'title', 'tag', 'body'])
  assert.equal(q.nextScope('all'), 'id')
  assert.equal(q.nextScope('body'), 'all')
  assert.equal(q.prevScope('all'), 'body')
  assert.equal(q.prevScope('title'), 'id')
  assert.ok(q.sees('all', 'tag') && q.sees('body', 'body') && !q.sees('title', 'body'))
})

test('search finds only in the parts its scope sees', () => {
  const find = (word, scope, ctx) => ids(q.search(rows, word, scope, ctx))
  assert.deepEqual(find('a003', 'id'), ['trk-a003', 'trk-a003.x1'])
  assert.deepEqual(find('a003', 'title'), [])
  assert.deepEqual(find('LAYOUT', 'title'), ['trk-a003', 'trk-a003.x1'])
  assert.deepEqual(find('css', 'tag'), ['trk-a003.x1'])
  assert.deepEqual(find('css', 'title'), [])
  assert.deepEqual(find('atomic', 'body'), ['trk-a001', 'trk-a009'])
  assert.deepEqual(find('atomic', 'title'), [])
  assert.deepEqual(find('css', 'all'), ['trk-a003.x1'])
  assert.equal(find('', 'title').length, rows.length)
  // The note scope looks only in notes, and `all` adds them to the rest; without notes it finds none.
  const notes = { 'trk-a002': ['Looked at the Parser\nsecond line'], 'trk-a010': ['unrelated'] }
  assert.deepEqual(find('second', 'note'), [])
  assert.deepEqual(find('second', 'note', { notes }), ['trk-a002'])
  assert.deepEqual(find('second', 'all', { notes }), ['trk-a002'])
  assert.deepEqual(find('second', 'title', { notes }), [])
  assert.deepEqual(q.noteLines(['one\nTwo here\nthree', 'two again'], 'TWO'), ['Two here', 'two again'])
  assert.deepEqual(q.noteLines(['one'], '  '), [])
  // A `grep=` filter reaches notes through the context the same way.
  assert.deepEqual(ids(q.applyFilter(rows, filterOf(['grep=second'], { grepIn: 'note' }), { notes })), ['trk-a002'])
})

test('hiding: each switch opens its own kind of row and hiddenBy names it', () => {
  const plain = filterOf([])
  const why = (id, filter = plain) => q.hiddenBy(rows.find((row) => row.id === id), filter, rows)
  assert.equal(why('trk-a004'), 'done')
  assert.equal(why('trk-a005'), 'deferred')
  assert.equal(why('trk-i001'), 'backlog')
  assert.equal(why('trk-a001'), undefined)
  assert.equal(why('trk-a004', filterOf([], { all: true })), undefined)
  assert.equal(why('trk-a005', filterOf([], { all: true })), undefined)
  assert.equal(why('trk-i001', filterOf([], { all: true })), 'backlog')
  assert.equal(why('trk-i001', filterOf(['type=backlog'])), undefined)
  assert.equal(why('trk-a004', filterOf(['type=backlog'])), 'done')
  // As moai's `hidden_by`: a closed backlog row is opened by no one word (the CLI's tail does not count it),
  // a deferred row in done is `done`, and a deferred backlog row is `deferred` (that switch opens backlog).
  assert.equal(why('trk-i002'), 'unopenable')
  assert.equal(why('trk-i002', filterOf([], { all: true })), 'backlog')
  const one = (row) => q.hiddenBy(row, plain, [row])
  assert.equal(one({ id: 'z1', status: 'done', deferred_at: '2026-10-01T00:00:00Z' }), 'done')
  assert.equal(one({ id: 'z2', kind: 'backlog', status: 'todo', deferred_at: '2026-10-01T00:00:00Z' }), 'deferred')
  assert.equal(one({ id: 'z3', kind: 'backlog', status: 'done', deferred_at: '2026-10-01T00:00:00Z' }), 'unopenable')
  assert.equal(q.hiddenBy(rows.find((row) => row.id === 'trk-i002'), filterOf([], { all: true }), rows, { now: '2026-12-01T00:00:00Z', archiveDays: 3 }), 'unopenable')
})

test('a deferral is inherited down the id parents and never into a closed row, as moai walks it', () => {
  const parent = { id: 'p1', status: 'todo', deferred_at: '2026-10-01T00:00:00Z' }
  const child = { id: 'p1.c1', status: 'todo' }
  const closed = { id: 'p1.c2', status: 'done' }
  const epic = { id: 'e1', kind: 'epic', status: 'todo', deferred_at: '2026-10-01T00:00:00Z' }
  const member = { id: 'm1', status: 'todo', epic: 'e1', derived_epic: 'e1' }
  const list = [parent, child, closed, epic, member]
  const plain = filterOf([])
  assert.equal(q.hiddenBy(child, plain, list), 'deferred')
  assert.equal(q.hiddenBy(member, plain, list), 'deferred')
  assert.equal(q.hiddenBy(closed, plain, list), 'done')
  assert.deepEqual(ids(q.applyFilter(list, filterOf([], { deferred: true }))), ['p1', 'p1.c1', 'e1', 'm1'])
  // A context hook still takes the place of the walk.
  assert.equal(q.hiddenBy(child, plain, list, { deferredOf: () => false }), undefined)
})

test('stale and the archive read the clock they are given', () => {
  const now = '2026-10-14T00:00:00Z'
  // trk-a002 stands in todo since 2026-10-04T02:00:00Z: nine days and some.
  assert.ok(ids(q.applyFilter(rows, filterOf(['stale=9']), { now })).includes('trk-a002'))
  assert.ok(!ids(q.applyFilter(rows, filterOf(['stale=10']), { now })).includes('trk-a002'))
  assert.deepEqual(q.applyFilter(rows, filterOf(['stale=1'])), [])
  // trk-a004 reached done on 2026-10-07: past 3 days it is the archive, and `all` alone does not bring it back.
  const all = filterOf([], { all: true })
  assert.ok(ids(q.applyFilter(rows, all, { now })).includes('trk-a004'))
  assert.ok(!ids(q.applyFilter(rows, all, { now, archiveDays: 3 })).includes('trk-a004'))
  assert.ok(ids(q.applyFilter(rows, filterOf([], { archived: true }), { now, archiveDays: 3 })).includes('trk-a004'))
  assert.equal(q.hiddenBy(rows.find((row) => row.id === 'trk-a004'), all, rows, { now, archiveDays: 3 }), 'archived')
})

test("the reader's clock moves a day, not an instant", () => {
  // trk-a007 was created 2026-10-05T03:00:00Z. At UTC+9 that is the 5th 12:00; at UTC-5 it is the 4th.
  const on = (day, local) => ids(q.applyFilter(rows, filterOf([`created=${day}`]), { local })).includes('trk-a007')
  assert.ok(on('2026-10-05'))
  assert.ok(on('2026-10-05', (at) => at + 9 * 3600))
  assert.ok(!on('2026-10-05', (at) => at - 5 * 3600))
  assert.ok(on('2026-10-04', (at) => at - 5 * 3600))
  // An instant is the same in every zone.
  const instant = (local) => ids(q.applyFilter(rows, filterOf(['created=2026-10-05T03:00:00Z']), { local }))
  assert.deepEqual(instant(), ['trk-a007'])
  assert.deepEqual(instant((at) => at + 9 * 3600), ['trk-a007'])
})

test('assignee is read as the tracker reads it: name, email or both, mail without case', () => {
  const who = (text) => ids(q.applyFilter(rows, filterOf([`assignee=${text}`], { all: true })))
  const bora = who('Bora')
  assert.ok(bora.length > 0)
  assert.deepEqual(who('bora@EXAMPLE.com'), bora)
  assert.deepEqual(who('Someone Else (bora@example.com)'), bora)
  assert.deepEqual(who('Bora (nobody@example.com)'), bora)
  assert.deepEqual(who('Bora,Chul'), ids(q.applyFilter(rows, filterOf(['assignee=Bora', 'assignee=Chul'], { all: true }))))
  assert.ok(who('none').includes('trk-a006'))
  // An empty assignee string is no assignee.
  const blank = [{ id: 'x1', assignee: '  ' }, { id: 'x2', assignee: 'Bora' }]
  assert.deepEqual(ids(q.applyFilter(blank, filterOf(['assignee=none']))), ['x1'])
})

test('parseSort reads fields with directions and refuses what the CLI refuses', () => {
  assert.deepEqual(q.parseSort('priority,updated:asc , title:desc').value, [
    { key: 'priority', dir: 'asc' },
    { key: 'updated', dir: 'asc' },
    { key: 'title', dir: 'desc' },
  ])
  assert.deepEqual(q.parseSort('created').value, [{ key: 'created', dir: 'desc' }])
  const refused = (text) => q.parseSort(text).error
  assert.deepEqual(refused(''), { code: 'empty' })
  assert.deepEqual(refused('priority,,id'), { code: 'empty' })
  assert.deepEqual(refused(':asc'), { code: 'empty' })
  assert.deepEqual(refused('size'), { code: 'unknown', word: 'size' })
  assert.deepEqual(refused('id:up'), { code: 'direction', word: 'id', dir: 'up' })
  assert.deepEqual(refused('priority,priority:desc'), { code: 'repeated', key: 'priority' })
  // The written form reads back to the same fields.
  for (const text of ['priority', 'created,id:desc', 'status:desc,title,updated:asc']) {
    const fields = q.parseSort(text).value
    assert.deepEqual(q.parseSort(q.formatSort(fields)).value, fields)
  }
  assert.equal(q.formatSort(q.parseSort('priority:asc,created:asc,updated').value), 'priority,created:asc,updated')
})

test('orderBy: ties fall to id after created or updated, else to priority then id; reverse turns it all', () => {
  const a = { id: 'a', priority: 2, created_at: '2026-10-01T00:00:00Z' }
  const b = { id: 'b', priority: 0, created_at: '2026-10-01T00:00:00Z' }
  const fields = (text) => q.parseSort(text).value
  assert.ok(q.orderBy(a, b, fields('created')) < 0)
  assert.ok(q.orderBy(a, b, fields('status')) > 0)
  assert.ok(q.orderBy(a, b, fields('created'), true) > 0)
  assert.equal(q.orderBy(a, a, fields('title')), 0)
  // A column that is not in the config goes after the ones that are; a group sorts by its derived column.
  const statuses = ['todo', 'done']
  assert.ok(q.orderBy({ id: 'x', status: 'todo' }, { id: 'y', status: 'weird' }, fields('status'), false, { statuses }) < 0)
  assert.ok(q.orderBy({ id: 'x', status: 'todo', derived_status: 'done' }, { id: 'y', status: 'todo' }, fields('status'), false, { statuses }) > 0)
  // naming decides what the assignee order sorts by.
  const x = { id: 'x', assignee: 'Zed', assignee_email: 'a@x.io' }
  const y = { id: 'y', assignee: 'Amy', assignee_email: 'z@x.io' }
  assert.ok(q.orderBy(x, y, fields('assignee'), false, { naming: 'name' }) > 0)
  assert.ok(q.orderBy(x, y, fields('assignee'), false, { naming: 'email' }) < 0)
  // Case does not matter for titles, and the order is by code point, not UTF-16 unit.
  assert.ok(q.orderBy({ id: 'x', title: 'apple' }, { id: 'y', title: 'Banana' }, fields('title')) < 0)
  assert.ok(q.orderBy({ id: 'x', title: 'Ａ' }, { id: 'y', title: '\u{1F600}' }, fields('title')) < 0)
})

test('sort edits: replace alone, append to the end, flip a field, reverse all', () => {
  const sort = (text, reversed = false) => ({ fields: q.parseSort(text).value, reversed })
  const text = (value) => q.formatSort(value.fields)
  assert.equal(text(q.sortReplace('updated')), 'updated')
  assert.equal(q.sortReplace('updated').reversed, false)
  assert.equal(text(q.sortAppend(sort('priority'), 'updated')), 'priority,updated')
  assert.equal(text(q.sortAppend(sort('priority,updated:asc'), 'updated')), 'priority,updated:asc')
  assert.equal(text(q.sortFlip(sort('priority,updated'), 'updated')), 'priority,updated:asc')
  assert.equal(text(q.sortFlip(sort('priority'), 'priority')), 'priority:desc')
  assert.equal(text(q.sortFlip(sort('priority'), 'title')), 'priority,title')
  assert.equal(q.sortReverse(sort('title')).reversed, true)
  assert.equal(q.sortReverse(q.sortReverse(sort('title'))).reversed, false)
  // `sortEdit` is the one key press: the same key alone twice turns it, another key replaces.
  assert.equal(text(q.sortEdit(sort('title'), 'title', 'replace')), 'title:desc')
  assert.equal(text(q.sortEdit(sort('title'), 'id', 'replace')), 'id')
  assert.equal(text(q.sortEdit(sort('priority,title'), 'title', 'replace')), 'title')
  assert.equal(text(q.sortEdit(sort('title'), 'id', 'append')), 'title,id')
  assert.equal(q.sortEdit(sort('title', true), 'id', 'replace').reversed, false)
  // An edit leaves the order it was given alone.
  const before = sort('priority')
  q.sortAppend(before, 'id')
  q.sortFlip(before, 'priority')
  assert.deepEqual(before, sort('priority'))
})

test('the sort badge says what the order is', () => {
  const badge = q.sortBadge(q.DEFAULT_SORT)
  assert.deepEqual([badge.isDefault, badge.text, badge.reversed], [true, 'priority', false])
  const custom = q.sortBadge({ fields: q.parseSort('status:desc,updated').value, reversed: true })
  assert.deepEqual(custom.parts, [
    { key: 'status', dir: 'desc', explicit: true },
    { key: 'updated', dir: 'desc', explicit: false },
  ])
  assert.deepEqual([custom.isDefault, custom.text, custom.reversed], [false, 'status:desc,updated', true])
  assert.equal(q.sortBadge({ fields: q.DEFAULT_SORT.fields, reversed: true }).isDefault, false)
  assert.equal(q.sortBadge({ fields: q.parseSort('priority:desc').value, reversed: false }).isDefault, false)
})

test('hints: keys by prefix, values counted from the rows', () => {
  assert.deepEqual(q.keyHints('cre'), ['created', 'created_at'])
  assert.deepEqual(q.keyHints(), [...q.KEYS])
  assert.deepEqual(q.keyHints('zzz'), [])
  const hint = (key, prefix) => q.valueHints(rows, key, prefix)
  const tags = hint('tag')
  assert.deepEqual(tags[0], { value: 'bug', count: 4 })
  assert.deepEqual(tags.map((one) => one.value), ['bug', 'ui', 'docs', 'css', 'later', 'test'])
  assert.deepEqual(hint('tag', 'D').map((one) => one.value), ['docs'])
  assert.deepEqual(hint('no-tag', 'cs'), [{ value: 'css', count: 1 }, { value: 'docs', count: 2 }])
  const people = hint('assignee')
  assert.deepEqual(people.map((one) => one.value), ['Fixture', 'Bora', 'Chul', 'none'])
  assert.deepEqual(people.at(-1), { value: 'none', count: 1 })
  assert.deepEqual(hint('milestone').map((one) => one.value), ['trk-ms02', 'trk-ms01', 'none'])
  assert.equal(hint('milestone')[0].label, 'Release two')
  assert.deepEqual(hint('epic', 'panel'), [{ value: 'trk-ep02', count: 3, label: 'Panel epic' }])
  assert.deepEqual(hint('parent').map((one) => one.value).slice(0, 1), ['trk-a003'])
  assert.deepEqual(hint('status').map((one) => one.value), ['todo', 'in_progress', 'review', 'done'])
  assert.deepEqual(hint('priority').map((one) => one.value), ['0', '1', '2', '3'])
  assert.deepEqual(hint('type').map((one) => one.value), ['issue', 'epic', 'milestone', 'backlog'])
  assert.deepEqual(hint('grep'), [])
  assert.deepEqual(q.valueHints([], 'tag'), [])
})

test('hintsAt finds the item under the cursor and what to suggest there', () => {
  const at = (text, cursor = text.length) => q.hintsAt(rows, text, cursor)
  let got = at('status=todo ta')
  assert.deepEqual(got.hints.map((one) => one.value), ['tag'])
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from, got.slot.to], [undefined, 'ta', 12, 14])
  got = at('status=todo tag=u')
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from], ['tag', 'u', 16])
  assert.deepEqual(got.hints.map((one) => one.value), ['ui', 'bug'])
  // After a comma the piece being typed is the last one; a quoted value counts from past the quote.
  got = at('tag=bug,d')
  assert.deepEqual([got.slot.partial, got.slot.from], ['d', 8])
  assert.deepEqual(got.hints.map((one) => one.value), ['docs'])
  got = at('grep="ab')
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from], ['grep', 'ab', 6])
  // In blank space a new item starts: every key.
  got = at('status=todo ')
  assert.equal(got.slot.key, undefined)
  assert.equal(got.hints.length, q.KEYS.length)
  // The cursor may sit inside an earlier item.
  got = at('status=todo tag=bug', 9)
  assert.deepEqual([got.slot.key, got.slot.partial], ['status', 'to'])
  assert.deepEqual(got.hints.map((one) => one.value), ['todo'])
})

// moai 0.17's own derivations, printed only when they hold: when a row carries them they decide the lens, as
// they do in the index; absent, the lens derives the same answers itself (the tests above).
test('moai shelved_by, derived_milestone and archived decide the lens when present', () => {
  const parent = { id: 'p1', status: 'todo', deferred_at: '2026-10-01T00:00:00Z' }
  const kid = { id: 'p1.c1', status: 'todo' }
  const toldYes = { id: 'free', status: 'todo', shelved_by: 'e-gone' }
  const m1 = { id: 'm1', kind: 'milestone', status: 'todo' }
  const m2 = { id: 'm2', kind: 'milestone', status: 'todo' }
  const epic = { id: 'e1', kind: 'epic', status: 'todo', milestone: 'm1' }
  const moved = { id: 'x', status: 'todo', epic: 'e1', derived_epic: 'e1', derived_milestone: 'm2' }
  const none = { id: 'y', status: 'todo', epic: 'e1', derived_epic: 'e1', derived_milestone: null }
  const list = [parent, kid, toldYes, m1, m2, epic, moved, none]
  const plain = filterOf([])
  assert.equal(q.hiddenBy(kid, plain, list), 'deferred', 'absent: inherited from the parent')
  assert.equal(q.hiddenBy(toldYes, plain, list), 'deferred', 'a shelf not among the rows')
  assert.deepEqual(ids(q.applyFilter(list, filterOf([], { deferred: true }))), ['p1', 'p1.c1', 'free'])
  assert.ok(ids(q.applyFilter(list, filterOf(['milestone=m2']))).includes('x'))
  assert.ok(!ids(q.applyFilter(list, filterOf(['milestone=m1']))).includes('x'))
  assert.ok(!ids(q.applyFilter(list, filterOf(['milestone=m1']))).includes('y'), 'null: no milestone')
  const ctx = { now: '2026-10-14T00:00:00Z', archiveDays: 30 }
  const all = filterOf([], { all: true })
  const fresh = { id: 'fresh', status: 'done', status_since: '2026-10-13T00:00:00Z', archived: true }
  assert.equal(q.hiddenBy(fresh, all, [fresh], ctx), 'archived', 'moai says archived whatever the clock reads')
  assert.equal(q.hiddenBy(fresh, all, [fresh], {}), 'archived', 'without archive_days too')
})

test('real 0.17 output: the lens hides what moai marks, and derives the same without the marks', async () => {
  const read = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
  const real = await read('show-archived-v017.json')
  const bare = real.map(({ shelved_by: _s, archived: _a, derived_milestone: _m, ...row }) => row)
  const ctx = { now: '2026-10-14T12:10:00Z', archiveDays: 30 }
  const plain = filterOf([])
  const told = real.map((row) => q.hiddenBy(row, plain, real, ctx))
  assert.deepEqual(bare.map((row) => q.hiddenBy(row, plain, bare, ctx)), told)
  real.forEach((row, at) => {
    if (row.archived === true) assert.equal(told[at], 'archived', row.id)
    else if (row.shelved_by) assert.equal(told[at], 'deferred', row.id)
    // Alone and with no clock, nothing is left to derive from: only moai's marks hide the row.
    const alone = q.hiddenBy(row, plain, [row], {})
    if (row.archived === true) assert.equal(alone, 'archived', `${row.id} alone`)
    else if (row.shelved_by) assert.equal(alone, 'deferred', `${row.id} alone`)
  })
})

test('a key spelled like an Object.prototype member is a key to refuse, not a crash', () => {
  for (const key of ['constructor', 'toString', 'valueOf', 'hasOwnProperty', '__proto__']) {
    const parsed = q.parseFilter([`${key}=1`])
    assert.equal(parsed.ok, false, key)
    assert.deepEqual(parsed.error, { code: 'no-such-key', key })
    assert.equal(q.parseFilter([`status=todo`, `${key}=x`]).ok, false, key)
  }
  // A row whose id is such a word is searched like any other (the notes table is keyed by id).
  assert.equal(q.hits({ id: 'constructor', title: 'x' }, 'zzz', 'note', { notes: {} }), false)
  assert.equal(q.hits({ id: 'constructor', title: 'x' }, 'zzz', 'all', { notes: { other: ['zzz'] } }), false)
})

test('milestone is the one a row stands on: the epic and the id parent decide, not the field the row wrote', () => {
  // `moai show --json --all --filter milestone=trk-m001` / `milestone=none` on this tracker (moai 0.16.0) listed:
  //   trk-m001 -> trk-q001, trk-q001.bbb     none -> trk-d001, trk-m001, trk-p001, trk-p001.aaa
  const row = (id, extra = {}) => ({ id, title: id, kind: 'issue', status: 'todo', priority: 2, ...extra })
  const tracker = [
    { ...row('trk-m001'), kind: 'milestone' },
    // Names an epic that is not there: it stands on no milestone whatever it wrote.
    row('trk-d001', { epic: 'trk-gone', milestone: 'trk-m001' }),
    // A child stands where its parent stands: the parent has none, so the milestone the child wrote is not read.
    row('trk-p001'),
    row('trk-p001.aaa', { milestone: 'trk-m001' }),
    // And a child of a row with a milestone stands on it without writing one.
    row('trk-q001', { milestone: 'trk-m001' }),
    row('trk-q001.bbb'),
  ]
  const ofMilestone = (value) => ids(q.applyFilter(tracker, filterOf([`milestone=${value}`], { all: true }))).sort()
  assert.deepEqual(ofMilestone('trk-m001'), ['trk-q001', 'trk-q001.bbb'])
  assert.deepEqual(ofMilestone('none'), ['trk-d001', 'trk-m001', 'trk-p001', 'trk-p001.aaa'])
  // An epic that wrote a milestone that does not exist stands on that id still (the CLI filters by it).
  const lost = [row('trk-e001', { kind: 'epic', milestone: 'trk-zzzz' }), row('trk-e001.a', { epic: 'trk-e001' })]
  assert.deepEqual(ids(q.applyFilter(lost, filterOf(['milestone=trk-zzzz'], { all: true }))).sort(), ['trk-e001', 'trk-e001.a'])
})
