// What is left of data/query.ts is help for a person typing: where the caret is, what to suggest there, and the sort
// editor's words. What a filter, a search or an order means is moai's, so nothing here holds a meaning to the CLI.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const q = await panelModule('data/query.ts')
const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const rows = [...(await fixture('rows.json')), ...(await fixture('backlog.json')).map((row) => ({ ...row, kind: 'backlog' }))]

/** `priority,updated:asc` as fields, the way the sort editor holds them. */
const fields = (text) => text.split(',').map((part) => {
  const [key, dir] = part.split(':')
  return { key, dir: dir ?? q.defaultDir(key) }
})
const sort = (text, reversed = false) => ({ fields: fields(text), reversed })

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
})

test('the words come from /api/boot: filter keys, and each sort field\'s own direction', () => {
  assert.ok(q.filterKeyList().includes('due_on'), 'the keys before boot already carry due_on')
  assert.equal(q.defaultDir('created'), 'desc')
  q.useVocabulary({ filter_keys: ['status', 'tag', 'due_on'], sort_fields: [{ name: 'due', dir: 'asc' }, { name: 'title', dir: 'desc' }, { name: 'nope', dir: 'asc' }] })
  try {
    assert.deepEqual(q.keyHints(), ['status', 'tag', 'due_on'])
    assert.equal(q.defaultDir('title'), 'desc')
    assert.deepEqual(q.fieldOf('title'), { key: 'title', dir: 'desc' })
  } finally {
    q.useVocabulary({ filter_keys: ['status', 'tag', 'no-tag', 'epic', 'milestone', 'parent', 'priority', 'assignee', 'type', 'grep', 'stale', 'since', 'created', 'done', 'due_on', 'created_at', 'updated_at', 'started_at', 'done_at'], sort_fields: [{ name: 'title', dir: 'asc' }] })
  }
})

test('the order goes to moai with every direction written out, and the reverse switch', () => {
  assert.deepEqual(q.sortQuery(q.DEFAULT_SORT), { sort: 'priority:asc' })
  assert.deepEqual(q.sortQuery(sort('due,updated', true)), { sort: 'due:asc,updated:desc', reverse: '1' })
  assert.deepEqual(q.sortQuery({ fields: [], reversed: false }), {})
  assert.ok(q.SORT_KEYS.includes('due'))
})

test('sort edits: replace alone, append to the end, flip a field, reverse all', () => {
  const text = (value) => q.formatSort(value.fields)
  assert.equal(text(q.sortReplace('updated')), 'updated')
  assert.equal(q.sortReplace('updated').reversed, false)
  assert.equal(text(q.sortAppend(sort('priority'), 'updated')), 'priority,updated')
  assert.equal(text(q.sortAppend(sort('priority,updated:asc'), 'updated')), 'priority,updated:asc')
  assert.equal(text(q.sortFlip(sort('priority,updated'), 'updated')), 'priority,updated:asc')
  assert.equal(text(q.sortFlip(sort('priority'), 'priority')), 'priority:desc')
  assert.equal(text(q.sortFlip(sort('priority'), 'due')), 'priority,due')
  assert.equal(q.sortReverse(sort('title')).reversed, true)
  // `sortEdit` is the one key press: the same key alone twice turns it, another key replaces.
  assert.equal(text(q.sortEdit(sort('title'), 'title', 'replace')), 'title:desc')
  assert.equal(text(q.sortEdit(sort('title'), 'id', 'replace')), 'id')
  assert.equal(text(q.sortEdit(sort('title'), 'id', 'append')), 'title,id')
  assert.equal(q.sortEdit(sort('title', true), 'id', 'replace').reversed, false)
  const before = sort('priority')
  q.sortAppend(before, 'id')
  q.sortFlip(before, 'priority')
  assert.deepEqual(before, sort('priority'), 'an edit leaves the order it was given alone')
  assert.equal(q.sameSort(sort('priority,updated'), sort('priority,updated:desc')), true)
})

test('the sort badge says what the order is', () => {
  const badge = q.sortBadge(q.DEFAULT_SORT)
  assert.deepEqual([badge.isDefault, badge.text, badge.reversed], [true, 'priority', false])
  const custom = q.sortBadge(sort('status:desc,updated', true))
  assert.deepEqual(custom.parts, [
    { key: 'status', dir: 'desc', explicit: true },
    { key: 'updated', dir: 'desc', explicit: false },
  ])
  assert.deepEqual([custom.isDefault, custom.text, custom.reversed], [false, 'status:desc,updated', true])
})

test('hints: keys by prefix, values counted from the rows moai printed', () => {
  assert.deepEqual(q.keyHints('cre'), ['created', 'created_at'])
  assert.deepEqual(q.keyHints('due'), ['due_on'])
  assert.deepEqual(q.keyHints('zzz'), [])
  const hint = (key, prefix, ctx) => q.valueHints(rows, key, prefix, ctx)
  assert.deepEqual(hint('tag'), [{ value: 'store', count: 1 }])
  assert.deepEqual(hint('assignee').map((one) => one.value), ['Fixture', 'none'])
  // The rows say where they stand: the milestone and the epic are moai's derived keys, not the fields written.
  assert.deepEqual(hint('milestone')[0], { value: 'fx-ms01', count: 4, label: 'v1' }, 'the epic and its three members')
  assert.deepEqual(hint('epic', 'storage'), [{ value: 'fx-ep01', count: 3, label: 'Storage layer' }])
  assert.deepEqual(hint('status', '', { statuses: ['todo', 'in_progress', 'review', 'done'] }).map((one) => one.value), ['todo', 'in_progress', 'review', 'done'])
  assert.deepEqual(hint('priority').map((one) => one.value), ['0', '1', '2', '3'])
  assert.deepEqual(hint('type').map((one) => [one.value, one.count]), [['issue', 7], ['epic', 2], ['milestone', 1], ['backlog', 1]])
  assert.deepEqual(hint('parent').map((one) => one.value).slice(0, 3), ['fx-ep01', 'fx-ep01.aaa', 'fx-ep02'])
  assert.deepEqual(hint('grep'), [])
  assert.deepEqual(q.valueHints([], 'tag'), [])
})

test('hintsAt finds the item under the cursor and what to suggest there', () => {
  const at = (text, cursor = text.length) => q.hintsAt(rows, text, cursor)
  let got = at('status=todo ta')
  assert.deepEqual(got.hints.map((one) => one.value), ['tag'])
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from, got.slot.to], [undefined, 'ta', 12, 14])
  got = at('status=todo tag=s')
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from], ['tag', 's', 16])
  assert.deepEqual(got.hints.map((one) => one.value), ['store'])
  got = at('tag=bug,st')
  assert.deepEqual([got.slot.partial, got.slot.from], ['st', 8])
  got = at('grep="ab')
  assert.deepEqual([got.slot.key, got.slot.partial, got.slot.from], ['grep', 'ab', 6])
  // A time key takes a value with a space in it.
  got = at('due_on=2026-10-01 00:00')
  assert.deepEqual([got.slot.key, got.slot.partial], ['due_on', '2026-10-01 00:00'])
  got = at('status=todo ')
  assert.equal(got.slot.key, undefined)
  assert.equal(got.hints.length, q.filterKeyList().length)
  got = at('status=todo tag=bug', 9)
  assert.deepEqual([got.slot.key, got.slot.partial], ['status', 'to'])
  assert.deepEqual(got.hints.map((one) => one.value), ['todo'])
})
