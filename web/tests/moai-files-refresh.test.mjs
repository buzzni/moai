import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createFilesRefresher, nextDelay } from '../panel/data/files-refresh.ts'
import { panelModule } from './moai-panel-modules.mjs'
const { urlOf, createClient } = await panelModule('data/client.ts')
function clock() {
  let now = 0, id = 0; const timers = new Map()
  return { now: () => now, setTimeout(fn, ms) { const key = ++id; timers.set(key, { at: now + ms, fn }); return key }, clearTimeout(key) { timers.delete(key) }, advance(ms) { now += ms; for (const [key, timer] of [...timers]) if (timer.at <= now) { timers.delete(key); timer.fn() } }, set(value) { now = value }, timers }
}
function visibility() { let value = true, notify = () => {}; return { shown: () => value, watch(fn) { notify = fn; return () => { notify = () => {} } }, set(v) { value = v; notify() } } }
const flush = async () => { for (let i = 0; i < 10; i++) await Promise.resolve() }
test('delay backs off after quiet and failure, and repeated query names survive', () => {
  assert.equal(nextDelay(3000, 0, false), 30000); assert.equal(nextDelay(0, 10, false), 10000); assert.equal(nextDelay(0, 0, true), 10000)
  assert.equal(urlOf({ path: '/api/files-stamp', query: { root: '.', dir: ['a', 'b'] } }), '/api/files-stamp?dir=a&dir=b&root=.')
})
test('one stamp request rotates at most 64 directories and refreshes only changed stamps', async () => {
  const time = clock(), visible = visibility(), batches = [], changed = []
  const dirs = Array.from({ length: 200 }, (_, i) => ({ path: `d${i}`, stamp: 'same' }))
  const watch = { key: 'files:.', dirs, file: { path: 'f', stamp: 'old' } }
  const refresh = createFilesRefresher({ clock: time, visibility: visible, snapshot: () => watch,
    async stamps(_watch, batch) { batches.push(batch); return { dirs: Object.fromEntries(batch.map(d => [d, d === 'd2' ? 'new' : 'same'])), file: 'new' } },
    async changes() { return '' }, async changedStamps(_watch, dirs, file) { changed.push([dirs, file]); watch.file.stamp = 'new'; dirs.forEach(d => { watch.dirs.find(x => x.path === d).stamp = 'new' }) }, async changedChanges() {} })
  for (let i = 0; i < 4; i++) await refresh.check()
  assert.deepEqual(batches.map(b => b.length), [64, 64, 64, 64]); assert.equal(new Set(batches.flat()).size, 200)
  assert.deepEqual(changed, [[['d2'], true]])
  visible.set(false); await refresh.check(); assert.equal(batches.length, 4)
  visible.set(true); time.advance(0); await flush(); assert.equal(batches.length, 5)
  refresh.stop()
})
test('slow changes wait ten times their duration and stale responses are discarded', async () => {
  const time = clock(), visible = visibility(); let watch = { key: 'files:a', dirs: [], changeKey: 'root:a' }, calls = 0, applied = 0, release
  const refresh = createFilesRefresher({ clock: time, visibility: visible, snapshot: () => watch,
    async stamps() { return { dirs: {}, file: null } }, async changes() { calls++; if (calls === 1) { time.set(3000); return 'one' } return 'two' },
    async changedStamps() {}, async changedChanges() { applied++ } })
  await refresh.check(); time.set(32999); await refresh.check(); assert.equal(calls, 1)
  time.set(33000); await refresh.check(); assert.equal(calls, 2); assert.equal(applied, 1)
  refresh.stop()
  const stale = createFilesRefresher({ clock: time, visibility: visible, snapshot: () => watch,
    stamps: () => new Promise(resolve => { release = resolve }), async changes() { return '' }, async changedStamps() { applied++ }, async changedChanges() {} })
  watch = { key: 'files:a', dirs: [{ path: '.', stamp: 'old' }] }; const pending = stale.check()
  watch = { key: 'files:b', dirs: [] }; release({ dirs: { '.': 'new' }, file: null }); await pending
  assert.equal(applied, 1); stale.stop()
})
test('a just-read stamp is checked once more even when the clock granularity hides a write', async () => {
  const time = clock(), visible = visibility(); time.set(1000); let calls = 0
  const watch = { key: 'files:.', dirs: [{ path: '.', stamp: 'same', readAt: 500 }] }
  const refresh = createFilesRefresher({ clock: time, visibility: visible, snapshot: () => watch,
    async stamps() { return { dirs: { '.': 'same' }, file: null } }, async changes() { return '' },
    async changedStamps() { calls++ }, async changedChanges() {} })
  await refresh.check(); await refresh.check(); assert.equal(calls, 1); refresh.stop()
})
test('stopping a background group cancels its queued work without cancelling a manual read', async () => {
  let release; const sent = []
  const client = createClient(async url => { sent.push(url); if (url === '/busy') await new Promise(resolve => { release = resolve }); return { status: 200, text: async () => '{"exit":0,"out":{}}' } })
  const busy = client.request({ path: '/busy' })
  const automatic = client.request({ path: '/api/files' }, { priority: 3, share: false, group: 'files-refresh' })
  const manual = client.request({ path: '/api/files' }, { priority: 0, share: false })
  client.cancelGroup('files-refresh')
  assert.equal((await automatic).kind, 'cancelled'); release(); await busy; assert.equal((await manual).kind, 'passed')
  assert.deepEqual(sent, ['/busy', '/api/files'])
})
