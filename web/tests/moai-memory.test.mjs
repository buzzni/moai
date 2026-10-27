// The page's settings storage: localStorage when the browser lets the page use it, the tab's memory otherwise.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { MEMORY_KEY, QUERY_KEY, createBrowserStorage, createSortDefault, createMemory, createSessionStorage, normalizeMemory, patchMemory, patchQuery, remember, follow } = await panelModule('data/memory.ts')
// Every module is loaded before the first test: a test registered after a later await is never run under
// --test-force-exit, which ends the run once the tests known so far are done.
const { createStore } = await panelModule('store.ts')

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

test('stale tabs patch independent settings and preserve unknown nested keys', () => {
  const local = fakeLocal()
  const original = { ...normalizeMemory(null), future: 1, detail: { ...normalizeMemory(null).detail, future: 2, ratio: { left: 0.45, bottom: 0.3, future: 3 } } }
  local.setItem(MEMORY_KEY, JSON.stringify(original))
  const a = createMemory(() => local)
  const b = createMemory(() => local)
  const beforeA = a.load()
  const beforeB = b.load()
  a.save({ ...beforeA, theme: 'dark', zone: 'Asia/Seoul' }, beforeA)
  b.save({ ...beforeB, tab: 'gantt', detail: { ...beforeB.detail, ratio: { ...beforeB.detail.ratio, bottom: 0.4 } } }, beforeB)
  const saved = JSON.parse(local.getItem(MEMORY_KEY))
  assert.deepEqual([saved.theme, saved.zone, saved.tab], ['dark', 'Asia/Seoul', 'gantt'])
  assert.deepEqual([saved.detail.ratio.bottom, saved.detail.ratio.left], [0.4, 0.45])
  assert.deepEqual([saved.future, saved.detail.future, saved.detail.ratio.future], [1, 2, 3])
  const before = a.load()
  const { zone, theme, ...cleared } = before
  a.save(cleared, before)
  assert.equal('zone' in JSON.parse(local.getItem(MEMORY_KEY)), false)
  assert.equal('theme' in JSON.parse(local.getItem(MEMORY_KEY)), false)
  const patched = patchMemory(saved, normalizeMemory(saved), { ...normalizeMemory(saved), theme: 'system' })
  assert.equal('theme' in patched, false)
  assert.equal(saved.theme, 'dark', 'patching never mutates its input')
})

test('query patches preserve other leaves, explicit null and unknown keys', () => {
  const before = { filter: '', sort: null, columns: { title: true, id: true }, view: { hideDeferred: false, hiddenStatuses: [] } }
  const stored = { ...before, filter: 'priority=0', columns: { ...before.columns, future: true }, view: { ...before.view, future: 1 }, future: 2 }
  const now = { ...before, columns: { ...before.columns, id: false }, view: { ...before.view, hiddenStatuses: ['done'] } }
  const patched = patchQuery(stored, before, now)
  assert.equal(patched.filter, 'priority=0')
  assert.equal(patched.columns.id, false)
  assert.equal(patched.columns.future, true)
  assert.equal(patched.view.future, 1)
  assert.deepEqual(patched.view.hiddenStatuses, ['done'])
  assert.equal(patched.future, 2)
  const sorted = { ...before, sort: { fields: [{ key: 'title', dir: 'asc' }], reversed: false } }
  assert.equal(patchQuery(stored, sorted, before).sort, null)
  assert.equal('title' in patchQuery(stored, before, { ...before, columns: {} }).columns, false)
})

test('remember on two stores preserves a peer theme, filter and view while saving local leaves', () => {
  const local = fakeLocal()
  const storage = createBrowserStorage(() => local)
  const a = createStore()
  const b = createStore()
  for (const store of [a, b]) remember(store, createMemory(() => local), createSortDefault(createSessionStorage()), storage)
  a.dispatch({ type: 'theme', theme: 'dark' })
  a.dispatch({ type: 'zone', zone: 'Asia/Seoul' })
  a.dispatch({ type: 'filter', text: 'priority=0' })
  a.dispatch({ type: 'view', view: { hideDeferred: false } })
  b.dispatch({ type: 'tab', tab: 'gantt' })
  b.dispatch({ type: 'column', column: 'id', on: false })
  b.dispatch({ type: 'view', view: { hideBacklog: true } })
  const saved = JSON.parse(local.getItem(MEMORY_KEY))
  assert.deepEqual([saved.theme, saved.zone, saved.tab], ['dark', 'Asia/Seoul', 'gantt'])
  const query = JSON.parse(local.getItem(QUERY_KEY))
  assert.equal(query.filter, 'priority=0')
  assert.equal(query.columns.id, false)
  assert.equal(query.view.hideDeferred, false)
  assert.equal(query.view.hideBacklog, true)
})

test('storage events follow theme and zone only and can be unsubscribed', () => {
  const dom = new JSDOM('')
  const store = createStore()
  store.dispatch({ type: 'tab', tab: 'wiki' })
  const stop = follow(store, dom.window)
  const send = (key, newValue) => dom.window.dispatchEvent(new dom.window.StorageEvent('storage', { key, newValue }))
  send('other', JSON.stringify({ theme: 'dark' }))
  assert.equal(store.getState().theme, 'system')
  send(MEMORY_KEY, JSON.stringify({ theme: 'dark', zone: 'Asia/Seoul', tab: 'gantt', detail: { visible: false } }))
  const state = store.getState()
  assert.deepEqual([state.theme, state.zone, state.tab, state.detail.visible], ['dark', 'Asia/Seoul', 'wiki', true])
  send(MEMORY_KEY, JSON.stringify({ theme: 'dark', zone: 'Asia/Seoul' }))
  assert.equal(store.getState(), state, 'same values do not trigger another change')
  send(MEMORY_KEY, null)
  assert.equal(store.getState().theme, 'system')
  assert.equal(store.getState().zone, null)
  stop()
  send(MEMORY_KEY, JSON.stringify({ theme: 'dark' }))
  assert.equal(store.getState().theme, 'system')
  dom.window.close()
})

test('a tab does not write back the theme it followed, so a late event cannot cover a newer choice', () => {
  const local = fakeLocal()
  const dom = new JSDOM('')
  const a = createStore()
  const b = createStore()
  for (const store of [a, b]) remember(store, createMemory(() => local), createSortDefault(createSessionStorage()), createBrowserStorage(() => local))
  follow(b, dom.window)
  a.dispatch({ type: 'theme', theme: 'dark' })
  const first = local.getItem(MEMORY_KEY)
  a.dispatch({ type: 'theme', theme: 'light' })
  // B handles the event of A's first write after A's second one landed.
  dom.window.dispatchEvent(new dom.window.StorageEvent('storage', { key: MEMORY_KEY, newValue: first }))
  assert.equal(b.getState().theme, 'dark', 'B follows the event it was handed')
  assert.equal(JSON.parse(local.getItem(MEMORY_KEY)).theme, 'light', 'and does not write it back')
  // B's own change afterwards writes only what B changed.
  b.dispatch({ type: 'tab', tab: 'gantt' })
  assert.deepEqual([JSON.parse(local.getItem(MEMORY_KEY)).theme, JSON.parse(local.getItem(MEMORY_KEY)).tab], ['light', 'gantt'])
  dom.window.close()
})

test('a stale layout save does not resurrect theme or zone removed by another tab', () => {
  const local = fakeLocal()
  const a = createMemory(() => local)
  a.save({ ...normalizeMemory(null), theme: 'dark', zone: 'Asia/Seoul' })
  const b = createMemory(() => local)
  const stale = b.load()
  const { theme, zone, ...cleared } = stale
  a.save(cleared, stale)
  b.save({ ...stale, tab: 'wiki' }, stale)
  const saved = JSON.parse(local.getItem(MEMORY_KEY))
  assert.equal('theme' in saved, false)
  assert.equal('zone' in saved, false)
  assert.equal(saved.tab, 'wiki')
})
