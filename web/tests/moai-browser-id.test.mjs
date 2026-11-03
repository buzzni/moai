import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { browserId } = await panelModule('data/browser-id.ts')

test('concurrent first tabs create the id once inside the lock, and failures use the shared budget', async () => {
  const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator')
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  let value = null, writes = 0, tail = Promise.resolve(), held = false
  // One process stands in for many: storage is only consistent between tabs while the lock is held, so every
  // access outside a granted callback is the race this module exists to close.
  const inside = () => { if (!held) throw Error('storage touched outside the lock') }
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: {
    request(_name, callback) {
      const next = tail.then(async () => {
        held = true
        try { return await callback() } finally { held = false }
      })
      tail = next.catch(() => {})
      return next
    },
  } } })
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: () => { inside(); return value },
    setItem: (_key, id) => { inside(); writes++; value = id },
  } })
  try {
    const ids = await Promise.all(Array.from({ length: 8 }, () => browserId()))
    assert.match(ids[0], /^[0-9a-f]{32}$/)
    assert.deepEqual(new Set(ids), new Set([ids[0]]))
    assert.equal(writes, 1)
    const kept = value
    // Without Web Locks (a page over plain http on an address other than localhost) the stored id is still kept.
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: {} })
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => kept, setItem: () => assert.fail('rewrote a kept id') } })
    assert.equal(await browserId(), kept)
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => null, setItem: () => { throw Error('blocked') } } })
    assert.equal(await browserId(), null)
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async (_name, callback) => callback() } } })
    assert.equal(await browserId(), null)
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks: { request: async () => { throw Error('denied') } } } })
    assert.equal(await browserId(), null)
  } finally {
    if (originalNavigator) Object.defineProperty(globalThis, 'navigator', originalNavigator); else delete globalThis.navigator
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage); else delete globalThis.localStorage
  }
})
