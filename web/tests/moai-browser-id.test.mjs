import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { browserId } = await panelModule('data/browser-id.ts')

test('concurrent first tabs serialize initialization and failures use the shared budget', async () => {
  const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator')
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  let value = null, writes = 0, tail = Promise.resolve()
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: {
    request(_name, callback) {
      const next = tail.then(callback)
      tail = next.catch(() => {})
      return next
    },
  } } })
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: () => value,
    setItem: (_key, id) => { writes++; value = id },
  } })
  try {
    const ids = await Promise.all(Array.from({ length: 8 }, () => browserId()))
    assert.match(ids[0], /^[0-9a-f]{32}$/)
    assert.deepEqual(new Set(ids), new Set([ids[0]]))
    assert.equal(writes, 1)
    value = null
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => null, setItem: () => { throw Error('blocked') } } })
    assert.equal(await browserId(), null)
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: {} })
    assert.equal(await browserId(), null)
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async () => { throw Error('denied') } } } })
    assert.equal(await browserId(), null)
  } finally {
    if (originalNavigator) Object.defineProperty(globalThis, 'navigator', originalNavigator); else delete globalThis.navigator
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage); else delete globalThis.localStorage
  }
})
