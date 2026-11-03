import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { browserId } = await panelModule('data/browser-id.ts')

test('unavailable IndexedDB falls back to the stored id, or the shared budget', async () => {
  const originalDB = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB')
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  let value = null
  Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: { open: () => { throw Error('denied') } } })
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: () => value, setItem: (_key, id) => { value = id },
  } })
  try {
    const id = await browserId()
    assert.match(id, /^[0-9a-f]{32}$/)
    assert.equal(await browserId(), id)
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => null, setItem: () => { throw Error('denied') } } })
    assert.equal(await browserId(), null)
  } finally {
    if (originalDB) Object.defineProperty(globalThis, 'indexedDB', originalDB); else delete globalThis.indexedDB
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage); else delete globalThis.localStorage
  }
})
