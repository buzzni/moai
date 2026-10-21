import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { isUnread, unreadIds } = await panelModule('data/read.ts')

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const rows = await fixture('show-all-unread-v016.json')

test('unread is the CLI key, read as is', () => {
  assert.deepEqual(unreadIds(rows).sort(), ['t-8x9q', 't-dse7', 't-oez7', 't-qhza', 't-vncv'])
  assert.equal(isUnread(rows.find((row) => row.id === 't-23k1')), false)
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
