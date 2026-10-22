import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { BACKOFF_MS, POLL_MS, createRefresher, documentVisibility, refreshTime } = await panelModule('data/refresh.ts')

const flush = async () => { for (let i = 0; i < 8; i++) await new Promise((resolve) => setImmediate(resolve)) }

/** A clock the test moves by hand; `advance` fires every timer that falls due. */
function fakeClock() {
  let now = 1_000_000
  let timers = []
  let id = 0
  return {
    now: () => now,
    setTimeout(callback, ms) { timers.push({ id: ++id, at: now + ms, callback }); return id },
    clearTimeout(handle) { timers = timers.filter((timer) => timer.id !== handle) },
    pending: () => timers.length,
    async advance(ms) {
      now += ms
      for (const timer of timers.filter((timer) => timer.at <= now)) {
        timers = timers.filter((other) => other !== timer)
        timer.callback()
      }
      await flush()
    },
  }
}

/** A visibility the test flips; `set` tells the watcher as `visibilitychange` would. */
function fakeVisibility(start = true) {
  let shown = start
  let changed = () => {}
  return {
    shown: () => shown,
    watch(listener) { changed = listener; return () => { changed = () => {} } },
    set(value) { shown = value; changed() },
  }
}

/** A server whose stamp the test sets; null is a server out of reach. Counts the stamps asked. */
function fakeServer(start = 'a') {
  let stamp = start
  let asked = 0
  return {
    stamp: async () => { asked += 1; return stamp },
    set(value) { stamp = value },
    get asked() { return asked },
  }
}

function setup(server = fakeServer(), visibility = fakeVisibility()) {
  const clock = fakeClock()
  const loads = []
  const reach = []
  const refresher = createRefresher({
    load: async (how) => { loads.push(how.automatic) },
    stamp: server.stamp,
    onReach: (reached) => reach.push(reached),
    visibility,
    clock,
  })
  return { clock, loads, reach, refresher, server, visibility }
}

test('it asks the stamp every 2 seconds and reads only when it moved', async () => {
  assert.equal(POLL_MS, 2_000)
  const { clock, loads, refresher, server } = setup()
  await refresher.refresh()
  assert.deepEqual(loads, [false])
  await clock.advance(POLL_MS)
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false], 'the stamp stood still: nothing read')
  assert.equal(server.asked, 3)
  server.set('b')
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false, true], 'moved once, read once')
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false, true])
  refresher.stop()
  await clock.advance(POLL_MS * 5)
  assert.equal(server.asked, 5)
})

test('a hidden tab asks nothing; shown again it asks at once', async () => {
  const { clock, loads, refresher, server, visibility } = setup()
  await refresher.refresh()
  visibility.set(false)
  assert.equal(clock.pending(), 0)
  server.set('b')
  await clock.advance(POLL_MS * 10)
  assert.equal(server.asked, 1)
  assert.deepEqual(loads, [false])
  visibility.set(true)
  await flush()
  assert.equal(server.asked, 2)
  assert.deepEqual(loads, [false, true])
})

test('a server out of reach is said once and asked every 10 seconds; back, the line goes and it reads', async () => {
  assert.equal(BACKOFF_MS, 10_000)
  const { clock, loads, reach, refresher, server } = setup()
  await refresher.refresh()
  server.set(null)
  await clock.advance(POLL_MS)
  assert.deepEqual(reach, [false])
  const asked = server.asked
  await clock.advance(POLL_MS)
  assert.equal(server.asked, asked, 'no 2-second asking while it is away')
  await clock.advance(BACKOFF_MS - POLL_MS)
  assert.equal(server.asked, asked + 1)
  assert.deepEqual(reach, [false], 'said once')
  server.set('a')
  await clock.advance(BACKOFF_MS)
  assert.deepEqual(reach, [false, true])
  assert.deepEqual(loads, [false, true], 'what changed while it was away is read, even under the same stamp')
})

test('a manual refresh reads whatever the stamp says, and a refresh while one runs joins it', async () => {
  const server = fakeServer()
  const clock = fakeClock()
  let release
  let loads = 0
  const refresher = createRefresher({
    load: () => { loads += 1; return new Promise((resolve) => { release = resolve }) },
    stamp: server.stamp,
    visibility: fakeVisibility(),
    clock,
  })
  const one = refresher.refresh()
  const two = refresher.refresh()
  await flush()
  assert.equal(loads, 1)
  release()
  await Promise.all([one, two])
  assert.equal(refresher.lastLoaded, clock.now())
  const three = refresher.refresh()
  await flush()
  assert.equal(loads, 2, 'the same stamp, read again when asked by hand')
  release()
  await three
})

test('the stamp is read before the rows, so a write during a read is the next change', async () => {
  const server = fakeServer('a')
  const clock = fakeClock()
  let loads = 0
  const refresher = createRefresher({
    load: async () => { loads += 1; if (loads === 1) server.set('b') },
    stamp: server.stamp,
    visibility: fakeVisibility(),
    clock,
  })
  await refresher.refresh()
  await clock.advance(POLL_MS)
  assert.equal(loads, 2)
})

test('a read that did not take leaves its stamp unseen: the next look reads again, and once it takes it stops', async () => {
  const server = fakeServer('a')
  const clock = fakeClock()
  const loads = []
  let take = true
  const refresher = createRefresher({
    load: async (how) => { loads.push(how.automatic); return take },
    stamp: server.stamp,
    visibility: fakeVisibility(),
    clock,
  })
  await refresher.refresh()
  take = false
  server.set('b')
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false, true], 'the stamp moved: read, and refused (busy, a child cut off, a cancel)')
  take = true
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false, true, true], 'the same stamp, read again: the rows on screen are still the ones before it')
  await clock.advance(POLL_MS)
  assert.deepEqual(loads, [false, true, true], 'it took: nothing more to read')
})

test('reads that fail against a server that answers back off to 10 s and say so once; a cancel does not', async () => {
  const clock = fakeClock()
  const server = fakeServer()
  const outcomes = ['failed', 'failed', false, 'failed', true]
  const loads = []
  const backoff = []
  const refresher = createRefresher({
    load: async () => { loads.push(clock.now()); return outcomes[loads.length - 1] },
    stamp: server.stamp,
    onBackoff: (on) => backoff.push(on),
    visibility: fakeVisibility(),
    clock,
  })
  await refresher.check()
  assert.deepEqual(loads.length, 1)
  assert.deepEqual(backoff, [true])
  await clock.advance(POLL_MS)
  assert.equal(loads.length, 1, 'no look after 2 s: it is waiting 10 s')
  await clock.advance(BACKOFF_MS - POLL_MS)
  assert.equal(loads.length, 2, 'the same stamp is read again after 10 s')
  assert.deepEqual(backoff, [true], 'told once, not per failure')
  await clock.advance(BACKOFF_MS)
  assert.equal(loads.length, 3)
  assert.deepEqual(backoff, [true], 'a cancelled read changes nothing about the line')
  await clock.advance(POLL_MS)
  assert.equal(loads.length, 3, 'a cancel keeps the pace it had: still 10 s')
  await clock.advance(BACKOFF_MS)
  assert.equal(loads.length, 4)
  await clock.advance(BACKOFF_MS)
  assert.equal(loads.length, 5)
  assert.deepEqual(backoff, [true, false], 'a read that took takes the line back')
  await clock.advance(POLL_MS)
  assert.equal(server.asked >= 6, true, 'and the 2 s pace is back')
  refresher.stop()
})

test('a refresh that fails while the server is out of reach says it is out of reach, not that it backs off', async () => {
  const clock = fakeClock()
  const server = fakeServer(null)
  const reach = []
  const backoff = []
  const refresher = createRefresher({
    load: async () => 'failed',
    stamp: server.stamp,
    onReach: (on) => reach.push(on),
    onBackoff: (on) => backoff.push(on),
    visibility: fakeVisibility(),
    clock,
  })
  await refresher.refresh()
  assert.deepEqual(reach, [false])
  assert.deepEqual(backoff, [], 'the backoff line is for a server that answers')
  refresher.stop()
})

test('the document\'s own visibility follows visibilityState', () => {
  const dom = new JSDOM('')
  const doc = dom.window.document
  let state = 'visible'
  Object.defineProperty(doc, 'visibilityState', { get: () => state })
  const signal = documentVisibility(doc)
  let told = 0
  const stop = signal.watch(() => { told += 1 })
  assert.equal(signal.shown(), true)
  state = 'hidden'
  doc.dispatchEvent(new dom.window.Event('visibilitychange'))
  assert.equal(signal.shown(), false)
  assert.equal(told, 1)
  stop()
  doc.dispatchEvent(new dom.window.Event('visibilitychange'))
  assert.equal(told, 1)
})

test('refreshTime is HH:MM:SS in 24 hours', () => {
  assert.match(refreshTime(Date.UTC(2026, 9, 21, 13, 4, 5), 'en-GB'), /^\d\d:\d\d:\d\d$/)
})
