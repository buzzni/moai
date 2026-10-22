// The page's settings storage: localStorage when the browser lets the page use it, the tab's memory otherwise.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { MEMORY_KEY, createBrowserStorage, createSortDefault } = await panelModule('data/memory.ts')

const fakeLocal = () => {
  const items = new Map()
  return { items, getItem: (k) => items.get(k) ?? null, setItem: (k, v) => items.set(k, String(v)), removeItem: (k) => items.delete(k) }
}

test('the key is the page\'s own, and the default sort outlives the tab where localStorage works', () => {
  assert.equal(MEMORY_KEY, 'moai.web')
  const local = fakeLocal()
  const storage = createBrowserStorage(() => local)
  assert.equal(storage.persistent, true)
  const defaults = createSortDefault(storage)
  defaults.set({ fields: [{ key: 'due', dir: 'asc' }], reversed: false })
  assert.deepEqual([...local.items.keys()], ['moai.web.sort'], 'the probe leaves nothing behind')
  assert.deepEqual(createSortDefault(createBrowserStorage(() => local)).get(), { fields: [{ key: 'due', dir: 'asc' }], reversed: false })
})

test('a browser that refuses site data leaves the tab\'s memory, which says it does not last', () => {
  const blocked = createBrowserStorage(() => { throw new Error('SecurityError') })
  assert.equal(blocked.persistent, false)
  blocked.write('a', '1')
  assert.equal(blocked.read('a'), '1')
  const refusing = createBrowserStorage(() => ({ getItem: () => null, setItem: () => { throw new Error('QuotaExceeded') }, removeItem: () => {} }))
  assert.equal(refusing.persistent, false)
  assert.equal(createBrowserStorage(() => null).persistent, false)
})
