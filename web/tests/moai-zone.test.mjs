// The zone dates are drawn in (`SPC o t`, moai-ly88): the picked one, else the TUI's, else the browser's. It only
// draws — the page remembers it in the browser and writes nothing to the server.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const Z = await panelModule('data/zone.ts')
const { formatDate } = await panelModule('view/detail-format.ts')
const { shortDate } = await panelModule('view/list-model.ts')
const { refreshTime } = await panelModule('data/refresh.ts')
const { MEMORY_KEY, createMemory, createSessionStorage, createSortDefault, normalizeMemory, remember } = await panelModule('data/memory.ts')
const { createStore, reduce, initialState } = await panelModule('store.ts')
const { createZonePicker } = await panelModule('view/zones.ts')
const { copyFor } = await panelModule('i18n/index.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const { mountShell } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')
const { wireKeys } = await panelModule('keys/wiring.ts')

const en = copyFor('en')

test('a zone name is the browser\'s to know; the picked one wins, then the TUI\'s, then the browser\'s', () => {
  assert.equal(Z.validZone('Asia/Seoul'), 'Asia/Seoul')
  assert.equal(Z.validZone('utc'), 'UTC')
  for (const bad of ['JST-9', 'Mars/Olympus', '', null, 7]) assert.equal(Z.validZone(bad), null, String(bad))
  assert.equal(Z.effectiveZone('UTC', 'Asia/Seoul'), 'UTC')
  assert.equal(Z.effectiveZone(null, 'Asia/Seoul'), 'Asia/Seoul')
  // A stored zone the browser dropped, or a TUI zone written as a POSIX rule, falls through without a word.
  assert.equal(Z.effectiveZone('Mars/Olympus', 'JST-9'), Z.browserZone())
  assert.deepEqual(Z.fallbackZone('Asia/Seoul'), { zone: 'Asia/Seoul', from: 'tui' })
  assert.deepEqual(Z.fallbackZone(null), { zone: Z.browserZone(), from: 'browser' })
  const names = Z.zoneNames()
  assert.equal(names[0], 'UTC')
  assert.equal(names.filter((n) => n === 'UTC').length, 1)
  assert.ok(names.includes('Asia/Seoul'))
  assert.deepEqual(Z.filterZones(['Asia/Seoul', 'Asia/Tokyo', 'Europe/Paris'], ' SEO '), ['Asia/Seoul'])
  assert.deepEqual(Z.filterZones(['A', 'B'], ''), ['A', 'B'])
})

test('every date the page draws follows the zone; a bare due day is a calendar day, not moved', () => {
  const at = '2026-10-21T16:30:00Z'
  assert.equal(formatDate(at, 'UTC'), '2026-10-21 16:30')
  assert.equal(formatDate(at, 'Asia/Seoul'), '2026-10-22 01:30')
  assert.equal(formatDate(at, 'America/Los_Angeles'), '2026-10-21 09:30')
  assert.equal(formatDate('2026-10-31', 'Asia/Seoul'), '2026-10-31')
  assert.equal(formatDate('nope', 'UTC'), '')
  // `this year` is read in the zone too: New Year's Eve in UTC is New Year's Day in Seoul.
  const now = new Date('2026-06-01T00:00:00Z')
  assert.equal(shortDate('2025-12-31T20:00:00Z', now, 'UTC'), '2025')
  assert.equal(shortDate('2025-12-31T20:00:00Z', now, 'Asia/Seoul'), '01-01')
  assert.equal(shortDate('2026-03-05T12:00:00Z', now, 'UTC'), '03-05')
  assert.equal(refreshTime(Date.UTC(2026, 9, 21, 23, 4, 5), undefined, 'UTC'), '23:04:05')
  assert.equal(refreshTime(Date.UTC(2026, 9, 21, 23, 4, 5), undefined, 'Asia/Seoul'), '08:04:05')
  assert.deepEqual(Z.partsIn(Date.UTC(2026, 0, 1, 0, 0, 0), 'UTC'), { year: 2026, month: 1, day: 1, hour: 0, minute: 0, second: 0 })
})

test('the picked zone is remembered beside the layout, validated when read back; no choice is no key', () => {
  assert.equal(normalizeMemory({ zone: 'Asia/Seoul' }).zone, 'Asia/Seoul')
  for (const bad of ['Mars/Olympus', '', 3, null]) assert.ok(!('zone' in normalizeMemory({ zone: bad })), String(bad))
  const items = new Map()
  const storage = { getItem: (k) => items.get(k) ?? null, setItem: (k, v) => items.set(k, String(v)) }
  const store = createStore()
  remember(store, createMemory(() => storage), createSortDefault(), createSessionStorage())
  store.dispatch({ type: 'zone', zone: 'Europe/Paris' })
  assert.equal(JSON.parse(items.get(MEMORY_KEY)).zone, 'Europe/Paris')
  // The next opening starts in it.
  const again = createStore()
  remember(again, createMemory(() => storage), createSortDefault(), createSessionStorage())
  assert.equal(again.getState().zone, 'Europe/Paris')
  // Cleared: the key goes, and the next opening follows the TUI's or the browser's again.
  store.dispatch({ type: 'zone', zone: null })
  assert.ok(!('zone' in JSON.parse(items.get(MEMORY_KEY))))
  // A stored zone this browser does not know is dropped silently.
  items.set(MEMORY_KEY, JSON.stringify({ tab: 'list', zone: 'Mars/Olympus' }))
  const third = createStore()
  remember(third, createMemory(() => storage), createSortDefault(), createSessionStorage())
  assert.equal(third.getState().zone, null)
  // The same zone again changes nothing (no redraw, no write).
  const start = initialState()
  assert.equal(reduce(start, { type: 'zone', zone: null }), start)
  assert.equal(reduce(start, { type: 'zone', zone: 'UTC' }).zone, 'UTC')
})

// ── the picker ──

function picker(names = ['UTC', 'Asia/Seoul', 'Asia/Tokyo', 'Europe/Paris']) {
  const dom = new JSDOM('<!doctype html><html><body><button id="away">x</button></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const picked = []
  let closed = 0
  const zones = createZonePicker(doc, { words: en.keys.zone, names: () => names, pick: (zone) => picked.push(zone), closed: () => closed++ })
  doc.body.append(zones.el)
  const key = (k) => {
    const event = new dom.window.KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true })
    zones.input.dispatchEvent(event)
    return event
  }
  const type = (text) => {
    zones.input.value = text
    zones.input.dispatchEvent(new dom.window.Event('input', { bubbles: true }))
  }
  const lines = () => [...zones.el.querySelectorAll('.m-zone-item')].map((line) => line.textContent)
  return { dom, doc, zones, picked, key, type, lines, closed: () => closed }
}

test('picker: the first line clears the choice and names what follows; the one in use carries `now`', () => {
  const p = picker()
  p.zones.open({ chosen: null, tui: 'Asia/Seoul' })
  assert.equal(p.zones.isOpen(), true)
  assert.equal(p.doc.activeElement, p.zones.input, 'the filter box has the caret')
  assert.equal(p.zones.el.getAttribute('aria-modal'), 'true', 'the key router leaves its keys alone')
  assert.deepEqual(p.lines(), ['Explorer zone (Asia/Seoul)now', 'UTC', 'Asia/Seoul', 'Asia/Tokyo', 'Europe/Paris'])
  assert.equal(p.zones.aimed(), '', 'the cursor opens on the line in use')
  p.key('Enter')
  assert.deepEqual(p.picked, [null])
  assert.equal(p.zones.isOpen(), false)
  // Without a TUI zone, the first line is the browser's.
  const q = picker()
  q.zones.open({ chosen: 'Asia/Tokyo', tui: null })
  assert.equal(q.lines()[0], `Browser zone (${Z.browserZone()})`)
  assert.equal(q.zones.aimed(), 'Asia/Tokyo')
  assert.equal(q.zones.el.querySelector('[data-zone="Asia/Tokyo"] .m-zone-now').textContent, 'now')
})

test('picker: typing filters, case aside; ↑ ↓ move and Enter applies; j and k are letters', () => {
  const p = picker()
  p.zones.open({ chosen: null, tui: null })
  p.type('ASIA')
  assert.deepEqual(p.zones.shown(), ['Asia/Seoul', 'Asia/Tokyo'])
  assert.equal(p.zones.aimed(), 'Asia/Seoul', 'typing aims the cursor')
  p.key('ArrowDown')
  assert.equal(p.zones.aimed(), 'Asia/Tokyo')
  p.key('ArrowDown')
  assert.equal(p.zones.aimed(), 'Asia/Tokyo', 'it stops at the end')
  p.key('ArrowUp')
  assert.equal(p.key('j').defaultPrevented, false, 'j types')
  assert.equal(p.zones.aimed(), 'Asia/Seoul')
  p.key('Enter')
  assert.deepEqual(p.picked, ['Asia/Seoul'])
  assert.equal(p.closed(), 1)
})

test('picker: Esc and a click outside close without a change; a click on a line applies it', () => {
  const p = picker()
  p.zones.open({ chosen: 'UTC', tui: null })
  p.type('paris')
  p.key('Escape')
  assert.deepEqual(p.picked, [])
  assert.equal(p.zones.isOpen(), false)
  p.zones.open({ chosen: 'UTC', tui: null })
  p.doc.getElementById('away').focus()
  assert.equal(p.zones.isOpen(), false, 'the focus left it')
  assert.deepEqual(p.picked, [])
  p.zones.open({ chosen: 'UTC', tui: null })
  p.zones.el.querySelector('[data-zone="Europe/Paris"]').dispatchEvent(new p.dom.window.MouseEvent('click', { bubbles: true }))
  assert.deepEqual(p.picked, ['Europe/Paris'])
})

test('picker: a zone in use the list lacks aims at nothing, says so, and Enter alone keeps it', () => {
  const p = picker(['UTC', 'Asia/Seoul'])
  p.zones.open({ chosen: 'Europe/Paris', tui: null })
  assert.equal(p.zones.aimed(), null)
  assert.match(p.zones.el.querySelector('.m-zone-note').textContent, /Europe\/Paris in use is not on this list/)
  p.key('Enter')
  assert.deepEqual(p.picked, [], 'nothing picked')
  assert.equal(p.zones.isOpen(), false)
  // The first step lands on the first line.
  p.zones.open({ chosen: 'Europe/Paris', tui: null })
  p.key('ArrowDown')
  assert.equal(p.zones.aimed(), '')
  // Nothing matches: said so.
  p.type('zzz')
  assert.equal(p.zones.el.querySelector('.m-zone-note').textContent, en.keys.zone.noMatch)
  p.key('Enter')
  assert.deepEqual(p.picked, [])
})

test('picker: a step with nothing matching never takes the cursor above the top, so the next match is aimed', () => {
  const p = picker()
  p.zones.open({ chosen: null, tui: null })
  p.type('zzz')
  p.key('ArrowDown')
  p.key('PageDown')
  p.type('seoul')
  assert.equal(p.zones.aimed(), 'Asia/Seoul')
  assert.ok(p.zones.el.querySelector('[data-zone="Asia/Seoul"][data-cursor]'))
  p.key('Enter')
  assert.deepEqual(p.picked, ['Asia/Seoul'])
})

// ── wired: SPC o t ──

test('SPC o t opens the picker over the page; what it picks goes to the store, never the server', () => {
  const rows = [{ id: 'a', kind: 'issue', title: 'A', status: 'todo' }]
  const dom = new JSDOM('<!doctype html><html lang="en"><body><div id="moai-app"></div></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const store = createStore()
  const index = new IssueIndex(rows, treeOf(rows))
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc, store, copy: en, language: 'en', panes: PANES,
    index: (state) => (state.board.value ? index : null),
    activity: () => ({ phase: 'idle', cancellable: false }),
    intents: { refresh() {}, choose() {}, cancel() {} },
  })
  store.dispatch({ type: 'board-loaded', data: { rows, loadedAt: Date.now() } })
  store.dispatch({ type: 'boot', boot: { lang: 'en', version: '0', project: 'p', root: '/r', statuses: ['todo'], naming: 'full', glyphs: {}, unknown_glyph: '○', columns: { backlog: { name: 'b', glyph: '◇' }, shelved: { name: 's', glyph: '‖' } }, filter_keys: [], sort_fields: [], user: null, latest: null, timezone: 'Asia/Seoul' } })
  const keys = wireKeys({ document: doc, store, shell, copy: en, platform: 'other', sortDefault: createSortDefault(), zoneNames: () => ['UTC', 'Asia/Seoul', 'Europe/Paris'] })
  shell.focusPane('list')
  const press = (key, target = doc.activeElement ?? doc.body) => target.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }))
  assert.equal(keys.context().zone, 'Asia/Seoul', 'the TUI\'s zone, before any is picked')
  press(' '); press('o')
  assert.equal(keys.menu.el.querySelector('[data-key="t"] .m-wk-state').textContent, '[Asia/Seoul]')
  press('t')
  assert.equal(keys.zones.isOpen(), true)
  assert.equal(keys.router.state().menu, null)
  // Its keys are its own: j types, and the router runs nothing.
  const input = keys.zones.input
  input.value = 'par'
  input.dispatchEvent(new dom.window.Event('input', { bubbles: true }))
  press('Enter', input)
  assert.equal(store.getState().zone, 'Europe/Paris')
  assert.equal(keys.zones.isOpen(), false)
  assert.equal(doc.activeElement, doc.querySelector('.moai-list'), 'back to the pane')
  assert.equal(keys.context().zone, 'Europe/Paris')
  // Esc closes without a change.
  press(' '); press('o'); press('t')
  press('Escape', keys.zones.input)
  assert.equal(store.getState().zone, 'Europe/Paris')
  assert.equal(keys.zones.isOpen(), false)
  // A click on its own title keeps the caret in the filter box (and the picker open).
  press(' '); press('o'); press('t')
  const down = new dom.window.MouseEvent('mousedown', { bubbles: true, cancelable: true })
  keys.zones.el.querySelector('.m-wk-title').dispatchEvent(down)
  assert.equal(down.defaultPrevented, true)
  // The focus moving to another control closes it and stays there: the pane does not take it back.
  shell.toolbar.searchInput.focus()
  assert.equal(keys.zones.isOpen(), false)
  assert.equal(doc.activeElement, shell.toolbar.searchInput)
})
