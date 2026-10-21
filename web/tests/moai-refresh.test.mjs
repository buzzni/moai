import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { DEFAULT_INTERVAL_MS, HIDDEN_PROBE_MS, createRefresher, panelVisibility, refreshTime } = await panelModule('data/refresh.ts')

const flush = async () => { for (let i = 0; i < 6; i++) await new Promise((resolve) => setImmediate(resolve)) }

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

test('while visible it reads again every 10 seconds', async () => {
  assert.equal(DEFAULT_INTERVAL_MS, 10_000)
  const clock = fakeClock()
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock })
  await refresher.refresh()
  assert.equal(loads, 1)
  assert.equal(refresher.lastLoaded, clock.now())
  await clock.advance(DEFAULT_INTERVAL_MS - 1)
  assert.equal(loads, 1)
  await clock.advance(1)
  assert.equal(loads, 2)
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 3)
  refresher.stop()
  await clock.advance(DEFAULT_INTERVAL_MS * 3)
  assert.equal(loads, 3)
})

test('a hidden panel does not poll, and one shown after the interval reads at once', async () => {
  const clock = fakeClock()
  let visible = true
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, isVisible: () => visible })
  await refresher.refresh()
  visible = false
  refresher.visibilityChanged()
  assert.equal(clock.pending(), 0)
  await clock.advance(DEFAULT_INTERVAL_MS * 2)
  assert.equal(loads, 1)
  visible = true
  refresher.visibilityChanged()
  await flush()
  assert.equal(loads, 2)
})

test('shown again before the interval passed, it waits for the rest of it', async () => {
  const clock = fakeClock()
  let visible = true
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, isVisible: () => visible })
  await refresher.refresh()
  visible = false
  refresher.visibilityChanged()
  const away = DEFAULT_INTERVAL_MS * 0.4
  await clock.advance(away)
  visible = true
  refresher.visibilityChanged()
  await clock.advance(DEFAULT_INTERVAL_MS - away - 1)
  assert.equal(loads, 1)
  await clock.advance(1)
  assert.equal(loads, 2)
})

test('a manual refresh while one runs waits for it instead of reading twice, and restarts the interval', async () => {
  const clock = fakeClock()
  let loads = 0
  let release
  const refresher = createRefresher({ load: () => { loads += 1; return new Promise((resolve) => (release = resolve)) }, clock })
  const first = refresher.refresh()
  const second = refresher.refresh()
  assert.equal(first, second)
  assert.equal(loads, 1)
  release()
  await first
  await clock.advance(30_000)
  const manual = refresher.refresh()
  release()
  await manual
  assert.equal(loads, 2)
  await clock.advance(DEFAULT_INTERVAL_MS - 1)
  assert.equal(loads, 2)
  await clock.advance(1)
  assert.equal(loads, 3)
})

test('a failed read still counts as a read for the schedule', async () => {
  const clock = fakeClock()
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1; throw new Error('declined') }, clock })
  await assert.rejects(refresher.refresh())
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 2)
  refresher.stop()
})

test('the interval can be changed, and 0 turns automatic reads off', async () => {
  const clock = fakeClock()
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock })
  await refresher.refresh()
  refresher.setInterval(10_000)
  await clock.advance(10_000)
  assert.equal(loads, 2)
  refresher.setInterval(0)
  await clock.advance(DEFAULT_INTERVAL_MS * 5)
  assert.equal(loads, 2)
})

test('off stays off when the panel is shown again; a manual refresh still reads', async () => {
  // The clock held after a declined read (main.ts holdIfAsked) must not read again on the next tab switch.
  const clock = fakeClock()
  let visible = true
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, isVisible: () => visible })
  await refresher.refresh()
  refresher.setInterval(0)
  visible = false
  refresher.visibilityChanged()
  await clock.advance(DEFAULT_INTERVAL_MS * 3)
  visible = true
  refresher.visibilityChanged()
  await flush()
  assert.equal(loads, 1)
  assert.equal(clock.pending(), 0)
  await refresher.refresh()
  assert.equal(loads, 2)
})

test('the load hears whether the clock started it or someone asked (se-24ey.a80: the clock\'s reads run quietly)', async () => {
  const clock = fakeClock()
  const told = []
  const refresher = createRefresher({ load: async (how) => { told.push(how.automatic) }, clock })
  await refresher.refresh()
  await clock.advance(DEFAULT_INTERVAL_MS)
  await refresher.refresh({ fresh: true })
  assert.deepEqual(told, [false, true, false])
})

test('the refresh time reads HH:MM:SS', () => {
  const at = new Date(2026, 9, 14, 9, 5, 7).getTime()
  assert.equal(refreshTime(at, 'en-GB'), '09:05:07')
})

test('a fresh read after a write waits out a read that started before it, then reads again', async () => {
  const clock = fakeClock()
  let loads = 0
  let release
  const refresher = createRefresher({ load: () => { loads += 1; return new Promise((resolve) => (release = resolve)) }, clock })
  const before = refresher.refresh()
  const fresh = refresher.refresh({ fresh: true })
  assert.notEqual(before, fresh)
  release()
  await before
  await flush()
  assert.equal(loads, 2)
  release()
  await fresh
  // Without a running read, fresh reads at once like any refresh.
  const idle = refresher.refresh({ fresh: true })
  assert.equal(loads, 3)
  release()
  await idle
})

/**
 * An iframe window as Desktop leaves it: `hideParent` makes the viewport 0×0 the way a hidden tool tab does,
 * with or without the resize event reaching the frame; `hideWindow` is the Desktop window going away.
 */
function fakeWindow(clock) {
  const win = new EventTarget()
  const doc = new EventTarget()
  doc.visibilityState = 'visible'
  doc.documentElement = {}
  Object.assign(win, {
    document: doc,
    innerWidth: 800,
    innerHeight: 600,
    setTimeout: (callback, ms) => clock.setTimeout(callback, ms),
    clearTimeout: (handle) => clock.clearTimeout(handle),
    size(width, height, { event = true } = {}) {
      win.innerWidth = width
      win.innerHeight = height
      if (event) win.dispatchEvent(new Event('resize'))
    },
    hideWindow(hidden) {
      doc.visibilityState = hidden ? 'hidden' : 'visible'
      doc.dispatchEvent(new Event('visibilitychange'))
    },
  })
  return win
}

test('the panel counts as hidden when the document is hidden or the viewport is 0×0', () => {
  const win = fakeWindow(fakeClock())
  const signal = panelVisibility(win)
  assert.equal(signal.shown(), true)
  win.size(0, 0, { event: false })
  assert.equal(signal.shown(), false)
  win.size(800, 600, { event: false })
  win.document.visibilityState = 'hidden'
  assert.equal(signal.shown(), false)
})

test('the signal says so once per change, from resize and visibilitychange', () => {
  const win = fakeWindow(fakeClock())
  let changes = 0
  const unwatch = panelVisibility(win).watch(() => { changes += 1 })
  win.size(820, 600)
  assert.equal(changes, 0, 'a resize that keeps it shown is no change')
  win.size(0, 0)
  win.size(0, 0)
  assert.equal(changes, 1)
  win.size(800, 600)
  assert.equal(changes, 2)
  win.hideWindow(true)
  assert.equal(changes, 3)
  win.hideWindow(false)
  assert.equal(changes, 4)
  unwatch()
  win.size(0, 0)
  win.hideWindow(true)
  assert.equal(changes, 4, 'nothing is heard after unwatch')
})

test('a 0×0 viewport that hears no event is still found shown again by the probe', async () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  let changes = 0
  const unwatch = panelVisibility(win).watch(() => { changes += 1 })
  assert.equal(clock.pending(), 0, 'no probe while shown')
  win.size(0, 0)
  assert.equal(changes, 1)
  assert.equal(clock.pending(), 1)
  win.size(800, 600, { event: false })
  await clock.advance(HIDDEN_PROBE_MS)
  assert.equal(changes, 2)
  assert.equal(clock.pending(), 0, 'the probe stops once shown')
  win.size(0, 0)
  unwatch()
  assert.equal(clock.pending(), 0, 'unwatch stops the probe')
})

test('no probe while the document itself is hidden — visibilitychange will come', () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  const unwatch = panelVisibility(win).watch(() => {})
  win.hideWindow(true)
  win.size(0, 0)
  assert.equal(clock.pending(), 0)
  unwatch()
})

test('an inactive tool tab stops the refresh, and shown again after the interval it reads at once', async () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, visibility: panelVisibility(win) })
  await refresher.refresh()
  win.size(0, 0)
  await clock.advance(DEFAULT_INTERVAL_MS * 3)
  assert.equal(loads, 1)
  win.size(800, 600)
  await flush()
  assert.equal(loads, 2)
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 3)
  refresher.stop()
})

test('a tool tab shown again before the interval passed waits for the rest of it', async () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, visibility: panelVisibility(win) })
  await refresher.refresh()
  win.size(0, 0)
  const away = DEFAULT_INTERVAL_MS * 0.4
  await clock.advance(away)
  win.size(800, 600)
  await clock.advance(DEFAULT_INTERVAL_MS - away - 1)
  assert.equal(loads, 1)
  await clock.advance(1)
  assert.equal(loads, 2)
  refresher.stop()
})

test('a timer that falls due while hidden reads nothing, even when nobody said it was hidden', async () => {
  const clock = fakeClock()
  let shown = true
  let changed
  const visibility = { shown: () => shown, watch(callback) { changed = callback; return () => { changed = null } } }
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, visibility })
  await refresher.refresh()
  shown = false
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 1)
  // It looks again in a while: a frame under a hidden parent may hear no change as it is shown again either.
  assert.equal(clock.pending(), 1)
  shown = true
  changed()
  await flush()
  assert.equal(loads, 2)
  refresher.stop()
  assert.equal(changed, null, 'stop unwatches the signal')
})

test('isVisible and the signal both have to say shown', async () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  let visible = false
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, isVisible: () => visible, visibility: panelVisibility(win) })
  await refresher.refresh()
  await clock.advance(DEFAULT_INTERVAL_MS * 2)
  assert.equal(loads, 1)
  visible = true
  win.size(0, 0)
  refresher.visibilityChanged()
  await clock.advance(DEFAULT_INTERVAL_MS * 2)
  assert.equal(loads, 1)
  win.size(800, 600)
  await flush()
  assert.equal(loads, 2)
  refresher.stop()
})

test('a tool tab hidden without a word is read again once shown, though the signal hears no change', async () => {
  const clock = fakeClock()
  const win = fakeWindow(clock)
  let loads = 0
  const refresher = createRefresher({ load: async () => { loads += 1 }, clock, visibility: panelVisibility(win) })
  await refresher.refresh()
  // Hidden with no resize event reaching the frame: the signal still takes it for shown.
  win.size(0, 0, { event: false })
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 1, 'nothing is read while hidden')
  await clock.advance(HIDDEN_PROBE_MS)
  assert.equal(loads, 1)
  // Shown again: to the signal the resize is no change, but the refresher looks again and reads.
  win.size(800, 600)
  await clock.advance(HIDDEN_PROBE_MS)
  assert.equal(loads, 2)
  await clock.advance(DEFAULT_INTERVAL_MS)
  assert.equal(loads, 3, 'and the interval goes on from there')
  refresher.stop()
  assert.equal(clock.pending(), 0)
})
