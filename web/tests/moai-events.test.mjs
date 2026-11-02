import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { openEvents } = await panelModule('data/events.ts')
const { createRefresher, POLL_MS } = await panelModule('data/refresh.ts')
const flush = async () => { for (let n = 0; n < 6; n++) await new Promise((resolve) => setImmediate(resolve)) }

function clock() {
  let now = 0
  let next = 0
  let timers = []
  return {
    now: () => now,
    setTimeout(callback, ms) { timers.push({ id: ++next, at: now + ms, callback }); return next },
    clearTimeout(id) { timers = timers.filter((t) => t.id !== id) },
    async advance(ms) {
      now += ms
      const due = timers.filter((t) => t.at <= now)
      timers = timers.filter((t) => t.at > now)
      due.forEach((t) => t.callback())
      await flush()
    },
  }
}

function visibility() {
  let shown = true
  let changed = () => {}
  return {
    shown: () => shown,
    watch(fn) { changed = fn; return () => { changed = () => {} } },
    set(value) { shown = value; changed() },
  }
}

function setup() {
  const time = clock()
  const visible = visibility()
  const sources = []
  const loads = []
  let asked = 0
  let stamp = 'a'
  const refresher = createRefresher({ clock: time, visibility: visible,
    stamp: async () => { asked++; return stamp }, load: async ({ automatic }) => { loads.push(automatic) } })
  const events = openEvents({ clock: time, visibility: visible, url: () => '/api/events',
    factory: (url) => {
      const source = { url, onopen: null, onerror: null, closed: false, listeners: {},
        addEventListener(type, fn) { this.listeners[type] = fn }, close() { this.closed = true },
        stamp(value) { this.listeners.stamp({ data: JSON.stringify({ stamp: value }) }) },
      }
      sources.push(source)
      return source
    },
    onStamp: (at) => refresher.saw(at), onUp: () => refresher.pause(), onDown: () => refresher.resume(),
    onVisible: () => void refresher.check(),
  })
  return { time, visible, sources, loads, refresher, events, get asked() { return asked }, set stamp(v) { stamp = v } }
}

test('a pushed stamp reads once and an unchanged stamp does not', async () => {
  const p = setup()
  await p.refresher.refresh()
  p.sources[0].onopen()
  p.sources[0].stamp('b')
  await flush()
  p.sources[0].stamp('b')
  await flush()
  assert.deepEqual(p.loads, [false, true])
  const asked = p.asked
  await p.time.advance(POLL_MS * 3)
  assert.equal(p.asked, asked, 'polling sleeps while the stream is up')
  p.events.stop()
  p.refresher.stop()
})

test('hiding closes the stream and showing opens it and checks at once', async () => {
  const p = setup()
  await p.refresher.refresh()
  p.sources[0].onopen()
  p.visible.set(false)
  assert.equal(p.sources[0].closed, true)
  await flush()
  p.stamp = 'b'
  p.visible.set(true)
  await flush()
  assert.equal(p.sources.length, 2)
  assert.deepEqual(p.loads, [false, true])
  p.events.stop()
  p.refresher.stop()
})

test('a lost stream resumes two-second polling and retries the stream later', async () => {
  const p = setup()
  await p.refresher.refresh()
  p.sources[0].onopen()
  p.sources[0].onerror()
  assert.equal(p.sources[0].closed, true)
  await flush()
  p.stamp = 'b'
  await p.time.advance(POLL_MS)
  assert.deepEqual(p.loads, [false, true])
  await p.time.advance(30_000)
  assert.equal(p.sources.length, 2)
  p.events.stop()
  p.refresher.stop()
})

test('a stream refused before open leaves polling available', async () => {
  const p = setup()
  await p.refresher.refresh()
  p.sources[0].onerror()
  await flush()
  p.stamp = 'b'
  await p.time.advance(POLL_MS)
  assert.deepEqual(p.loads, [false, true])
  p.events.stop()
  p.refresher.stop()
})

// The server keeps a tab for thirty seconds after its stream ends (src/web/ctl.rs RESUME_GRACE): a stream that was open
// (the server's thirty-minute end, a dropped connection) must come back well inside that, or the tab loses its ctl id
// and its suggestions. Refused resumes retry inside that grace, then return to thirty-second backoff.
test('an open stream that ends comes back inside the server resume grace', async () => {
  const p = setup()
  await p.refresher.refresh()
  p.sources[0].onopen()
  p.sources[0].onerror()
  await p.time.advance(2_999)
  assert.equal(p.sources.length, 1)
  await p.time.advance(1)
  assert.equal(p.sources.length, 2)
  p.sources[1].onerror()
  await p.time.advance(3_000)
  assert.equal(p.sources.length, 3, 'a refused resume lost its grace')
  await p.time.advance(24_000)
  p.sources[2].onerror()
  await p.time.advance(29_999)
  assert.equal(p.sources.length, 3, 'retry did not back off after grace ended')
  await p.time.advance(1)
  assert.equal(p.sources.length, 4)
  p.events.stop()
  p.refresher.stop()
})

test('a pushed stamp during a read is checked after that read settles', async () => {
  const time = clock()
  let release
  let loads = 0
  const refresher = createRefresher({ clock: time, visibility: visibility(), stamp: async () => 'a',
    load: () => { loads++; return new Promise((resolve) => { release = resolve }) } })
  const first = refresher.refresh()
  await flush()
  const pushed = refresher.saw('b')
  release()
  await first
  await flush()
  assert.equal(loads, 2)
  release()
  await pushed
  refresher.stop()
})

test('a resumed stream refused before open retries within grace while the old slot is being returned', async () => {
  const p = setup()
  p.sources[0].onopen()
  p.events.reconnect()
  p.sources[1].onerror()
  await p.time.advance(3000)
  assert.equal(p.sources.length, 3)
  p.sources[2].onopen()
  p.events.stop(); p.refresher.stop()
})
