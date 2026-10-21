import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { isUnread, unreadIds } = await panelModule('data/read.ts')

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const rows = await fixture('rows.json')

test('unread is the CLI key, read as is', () => {
  // The fixture tracker's rows are all near its one person, and none was read.
  assert.deepEqual(unreadIds(rows), rows.filter((row) => row.unread === true).map((row) => row.id))
  assert.ok(unreadIds(rows).length > 0)
  assert.equal(isUnread({ id: 'x', unread: false }), false, 'only true counts')
  assert.equal(isUnread({ id: 'x', unread: 'yes' }), false)
})

test('no row carrying the key means NEW is off', () => {
  const bare = rows.map(({ unread, ...rest }) => rest)
  assert.deepEqual(unreadIds(bare), [])
})

test('archive-only rows never draw NEW (se-fvou)', () => {
  const mixed = [{ id: 'a', unread: true }, { id: 'b', unread: true, archived: true }]
  assert.equal(isUnread(mixed[1]), false)
  assert.deepEqual(unreadIds(mixed), ['a'])
})
