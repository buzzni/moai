import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { browserId } = await panelModule('data/browser-id.ts')

/** Runs `body` with these stand-ins for the page's IndexedDB and local storage, restoring the real ones after. */
async function withStores(indexedDB, localStorage, body) {
  const originalDB = Object.getOwnPropertyDescriptor(globalThis, 'indexedDB')
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB })
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: localStorage })
  try {
    await body()
  } finally {
    if (originalDB) Object.defineProperty(globalThis, 'indexedDB', originalDB); else delete globalThis.indexedDB
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage); else delete globalThis.localStorage
  }
}

const memory = (initial = null) => {
  const store = { value: initial, getItem: () => store.value, setItem: (_key, id) => { store.value = id } }
  return store
}

/** An IndexedDB whose one store already holds `held`, answering each request on a later microtask. */
const holding = (held) => ({ open: () => {
  const request = {}
  queueMicrotask(() => {
    const tx = {}
    const store = {
      get: () => {
        const get = {}
        queueMicrotask(() => { get.result = held; get.onsuccess(); queueMicrotask(() => tx.oncomplete()) })
        return get
      },
      put: (id) => { held = id },
    }
    tx.objectStore = () => store
    request.result = { transaction: () => tx, close: () => {} }
    request.onsuccess()
  })
  return request
} })

test('unavailable IndexedDB falls back to the stored id, or the shared budget', async () => {
  const storage = memory()
  await withStores({ open: () => { throw Error('denied') } }, storage, async () => {
    const id = await browserId()
    assert.match(id, /^[0-9a-f]{32}$/)
    assert.equal(await browserId(), id)
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => null, setItem: () => { throw Error('denied') } } })
    assert.equal(await browserId(), null)
  })
})

test('an IndexedDB that never answers falls back instead of holding the stream back', async () => {
  const storage = memory()
  await withStores({ open: () => ({}) }, storage, async () => {
    let guard
    const hung = new Promise((resolve) => { guard = setTimeout(() => resolve('hung'), 2_000) })
    const id = await Promise.race([browserId(20), hung])
    clearTimeout(guard)
    assert.notEqual(id, 'hung', 'the page waited on IndexedDB without a bound')
    assert.equal(id, storage.value)
  })
})

test('the IndexedDB id is mirrored so a tab that falls back joins the same budget', async () => {
  const kept = 'b'.repeat(32)
  const storage = memory('c'.repeat(32))
  await withStores(holding(kept), storage, async () => {
    assert.equal(await browserId(), kept)
    assert.equal(storage.value, kept, 'a tab falling back to local storage would open a second budget')
  })
})


test('a timed-out transaction is aborted before it can commit a competing id', async () => {
  const storage = memory()
  let tx, get, writes = 0, aborted = false
  const indexedDB = { open: () => {
    const request = {}
    queueMicrotask(() => {
      tx = { abort: () => { aborted = true }, objectStore: () => ({
        get: () => (get = {}), put: () => { writes++ },
      }) }
      request.result = { transaction: () => tx, close: () => {} }
      request.onsuccess()
    })
    return request
  } }
  await withStores(indexedDB, storage, async () => {
    const id = await browserId(20)
    assert.equal(id, storage.value)
    assert.equal(aborted, true)
    get.result = undefined
    get.onsuccess()
    assert.equal(writes, 0, 'a late read must not create another identity')
  })
})
