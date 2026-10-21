// Moai panel shell assembly (se-qwkz.2v9): the path line, what the panel remembers, and the shell itself —
// tabs, the detail toggles and the split wired to the store — in a jsdom document.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { crumbSegments, crumbLine, hungBadge, lookBadge, segLabel, sortWords } = await panelModule('view/crumbs.ts')
const { MEMORY_KEY, SORT_DEFAULT_KEY, createMemory, createSortDefault, createSessionStorage, normalizeSort, QUERY_KEY, normalizeQuery, normalizeMemory, remember } = await panelModule('data/memory.ts')
const { createStore, initialState, reduce } = await panelModule('store.ts')
const { IssueIndex, LOST, NO_MILESTONE } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const build = (rows, tree = treeOf(rows)) => new IssueIndex(rows, tree)
const { copyFor } = await panelModule('i18n/index.ts')
const { describe, mountShell, mainWidth } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')

const en = copyFor('en')
const ko = copyFor('ko')

const ROWS = [
  { id: 'se-m1', kind: 'milestone', title: 'v2.0', status: 'todo' },
  { id: 'se-e1', kind: 'epic', title: 'E3 views', milestone: 'se-m1', status: 'todo' },
  { id: 'se-e1.a', kind: 'issue', title: 'List', epic: 'se-e1', derived_epic: 'se-e1', status: 'todo', priority: 1 },
  { id: 'se-e1.b', kind: 'issue', title: 'Board', epic: 'se-e1', derived_epic: 'se-e1', status: 'in_progress', priority: 2 },
]
const index = build(ROWS)

test('crumbs: the path reads as titles from the root, each segment going to its level', () => {
  const path = index.pathOf('se-e1')
  assert.deepEqual(path, ['m:se-m1', 'e:se-e1'])
  assert.deepEqual(crumbSegments(path, index, en), [
    { label: '/', path: [] },
    { label: 'v2.0', path: ['m:se-m1'] },
    { label: 'E3 views', path: ['m:se-m1', 'e:se-e1'] },
  ])
  assert.deepEqual(crumbSegments([], index, en), [{ label: '/', path: [] }])
  // Buckets have names of their own; a row the index does not hold (not read yet) shows its id.
  assert.equal(segLabel(NO_MILESTONE, index, ko), '(마일스톤 없음)')
  assert.equal(segLabel(LOST, index, en), '(lost)')
  assert.equal(segLabel('e:se-gone', index, en), 'se-gone')
  assert.equal(segLabel('m:se-m1', null, en), 'se-m1')
})

test('crumbs: hidden and sort badges only when the view hides something or the order is not the default', () => {
  const state = initialState()
  // The TUI starts with deferred rows hidden, and says so.
  assert.equal(lookBadge(state, en), 'deferred hidden')
  assert.equal(lookBadge({ ...state, view: { ...state.view, hideDeferred: false } }, en), null)
  const busy = {
    view: { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false },
    sort: { fields: [{ key: 'priority', dir: 'asc' }, { key: 'updated', dir: 'desc' }], reversed: false },
    defaultSort: state.defaultSort,
  }
  assert.equal(lookBadge(busy, en), 'done · deferred · backlog hidden · sort priority↑, updated↓')
  assert.equal(lookBadge(busy, ko), 'done · 미룸 · 백로그 숨김 · 정렬 우선순위↑, 고친 날↓')
  assert.equal(sortWords({ fields: [{ key: 'title', dir: 'asc' }], reversed: false }, en), 'title↑')
  // The column is `status`, as data/query.ts and the key table name it; a turned list says so.
  assert.equal(sortWords({ fields: [{ key: 'status', dir: 'asc' }], reversed: true }, ko), '칸↑ (뒤집음)')
  assert.equal(lookBadge({ view: { ...state.view, hideDeferred: false }, sort: { fields: [{ key: 'priority', dir: 'asc' }], reversed: true }, defaultSort: state.defaultSort }, en), 'sort priority↑ (reversed)')
  // The default is the saved one (`D`, State.defaultSort): that order is no badge, and the built-in one then is.
  const saved = reduce(state, { type: 'sort-default', sort: busy.sort })
  assert.equal(lookBadge({ ...saved, view: busy.view, sort: busy.sort }, en), 'done · deferred · backlog hidden')
  assert.equal(lookBadge({ ...saved, view: { ...state.view, hideDeferred: false } }, en), 'sort priority↑')
})

test('crumbs: a search or filter that is on is a badge of its own; the filter wins', () => {
  assert.equal(hungBadge({ search: '' }), null)
  assert.equal(hungBadge({ search: 'grid' }), '/grid')
  assert.equal(hungBadge({ search: 'grid', filter: 'status=todo' }), 'status=todo')
  assert.equal(hungBadge({ search: '', filter: '   ' }), null)
  const line = crumbLine({ ...initialState(), path: ['m:se-m1'], search: { text: 'x', open: true } }, index, en)
  assert.deepEqual(line.segments.map((segment) => segment.label), ['/', 'v2.0'])
  assert.equal(line.hung, '/x')
})

test('memory: whatever storage gives back becomes usable settings', () => {
  const defaults = { tab: 'list', detail: { visible: true, position: 'bottom', ratio: { left: 0.45, bottom: 0.3 } } }
  for (const junk of [null, undefined, 3, 'list', [], { tab: 7, detail: 'left' }]) assert.deepEqual(normalizeMemory(junk), defaults)
  assert.deepEqual(
    normalizeMemory({ version: 1, tab: 'board', detail: { visible: false, position: 'left', ratio: { left: 0.6, bottom: 0.2 } } }),
    { tab: 'board', detail: { visible: false, position: 'left', ratio: { left: 0.6, bottom: 0.2 } } },
  )
  // Out of range ratios are clamped, unknown tabs and positions (the TUI's right/top) fall back.
  assert.deepEqual(
    normalizeMemory({ tab: 'stats', detail: { visible: 'yes', position: 'right', ratio: { left: 2, bottom: -1 } } }),
    { tab: 'list', detail: { visible: true, position: 'bottom', ratio: { left: 0.85, bottom: 0.15 } } },
  )
  assert.deepEqual(normalizeMemory({ detail: { ratio: { left: Number.NaN } } }).detail.ratio, { left: 0.45, bottom: 0.3 })
})

const fakeStorage = () => {
  const items = new Map()
  return { items, getItem: (key) => (items.has(key) ? items.get(key) : null), setItem: (key, value) => items.set(key, String(value)) }
}

test('memory: kept in storage when it works, for the session when it throws', () => {
  const storage = fakeStorage()
  const memory = createMemory(() => storage)
  assert.equal(memory.persistent, true)
  const settings = { tab: 'wiki', detail: { visible: true, position: 'left', ratio: { left: 0.5, bottom: 0.3 } } }
  memory.save(settings)
  assert.equal(JSON.parse(storage.items.get(MEMORY_KEY)).version, 1)
  assert.deepEqual(createMemory(() => storage).load(), settings)

  // An opaque-origin frame throws on reading window.localStorage itself.
  const blocked = createMemory(() => { throw new Error('SecurityError') })
  assert.equal(blocked.persistent, false)
  assert.equal(blocked.load().tab, 'list')
  blocked.save(settings)
  assert.deepEqual(blocked.load(), settings)

  // A storage that refuses writes (full, or blocked later) drops to the session copy.
  const refusing = { getItem: () => null, setItem: () => { throw new Error('QuotaExceededError') } }
  const quota = createMemory(() => refusing)
  quota.save(settings)
  assert.equal(quota.persistent, false)
  assert.deepEqual(quota.load(), settings)

  storage.items.set(MEMORY_KEY, '{not json')
  assert.equal(createMemory(() => storage).load().tab, 'list')
})

test('memory: restored into the store at boot, saved only when a remembered setting changes', () => {
  const storage = fakeStorage()
  storage.items.set(MEMORY_KEY, JSON.stringify({ tab: 'board', detail: { visible: false, position: 'left', ratio: { left: 0.6, bottom: 0.25 } } }))
  let writes = 0
  const counting = { getItem: storage.getItem, setItem: (key, value) => { writes++; storage.setItem(key, value) } }
  const store = createStore()
  remember(store, createMemory(() => counting), createSortDefault(), createSessionStorage())
  const state = store.getState()
  assert.equal(state.tab, 'board')
  assert.deepEqual(state.detail, { visible: false, position: 'left', ratio: { left: 0.6, bottom: 0.25 } })
  assert.equal(writes, 0)
  store.dispatch({ type: 'search', text: 'x' })
  store.dispatch({ type: 'cursor', id: 'se-e1.a' })
  assert.equal(writes, 0, 'state the panel does not remember writes nothing')
  store.dispatch({ type: 'detail-ratio', position: 'bottom', ratio: 0.4 })
  assert.equal(writes, 1)
  assert.equal(JSON.parse(storage.items.get(MEMORY_KEY)).detail.ratio.bottom, 0.4)
})

test('store: restore applies what was kept and leaves the rest', () => {
  const state = initialState()
  assert.equal(reduce(state, { type: 'restore', settings: {} }), state)
  const restored = reduce({ ...state, focus: 'detail' }, { type: 'restore', settings: { detail: { visible: false, position: 'left', ratio: { left: 0.9, bottom: 0.3 } } } })
  assert.equal(restored.tab, 'list')
  assert.equal(restored.detail.ratio.left, 0.85)
  assert.equal(restored.focus, 'list', 'a hidden detail cannot hold the focus')
})

test('shell: the main pane is narrower only by a detail standing on the left', () => {
  assert.equal(mainWidth(1000, 'left', true, 0.4), 600)
  assert.equal(mainWidth(1000, 'left', false, 0.4), 1000)
  assert.equal(mainWidth(500, 'bottom', true, 0.4), 500)
})

/** Mounts the shell as main.ts does, in a fresh document, on `storage`. */
function boot(storage, { rows = ROWS, indexOf = null } = {}) {
  const dom = new JSDOM('<!doctype html><html lang="en"><head></head><body><div id="moai-app"></div></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const store = createStore()
  remember(store, createMemory(() => storage), createSortDefault(), createSessionStorage())
  const calls = []
  const rowIndex = build(rows)
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc,
    store,
    copy: en,
    language: 'en',
    panes: PANES,
    index: indexOf ?? ((state) => (state.board.value ? rowIndex : null)),
    activity: () => ({ phase: 'idle', cancellable: false }),
    intents: {
      refresh: () => calls.push(['refresh']),
      choose: (id) => { calls.push(['choose', id]); store.dispatch({ type: 'cursor', id }) },
      cancel: () => calls.push(['cancel']),
    },
  })
  store.dispatch({ type: 'board-loaded', data: { rows, more: 0, loadedAt: Date.UTC(2026, 9, 14, 3, 4, 5) } })
  const $ = (selector) => doc.querySelector(selector)
  return { dom, doc, store, shell, calls, $, tab: (id) => $(`[role="tab"][data-tab="${id}"]`) }
}

test('shell: tabs switch the pane shown and the choice is remembered', () => {
  const storage = fakeStorage()
  const panel = boot(storage)
  assert.deepEqual([...panel.doc.querySelectorAll('[role="tab"]')].map((tab) => tab.textContent), ['Board', 'List', 'Wiki'])
  assert.equal(panel.tab('list').getAttribute('aria-selected'), 'true')
  assert.equal(panel.$('.m-pane-main[data-pane="list"]').hidden, false)
  assert.equal(panel.$('.m-pane-main[data-pane="board"]').hidden, true)
  // Both work tabs control the one panel that holds the split.
  assert.equal(panel.tab('board').getAttribute('aria-controls'), panel.tab('list').getAttribute('aria-controls'))

  panel.tab('board').click()
  assert.equal(panel.store.getState().tab, 'board')
  assert.equal(panel.$('.m-pane-main[data-pane="list"]').hidden, true)
  assert.equal(panel.$('.m-pane-main[data-pane="board"]').hidden, false)
  assert.ok(panel.$('.m-pane-main[data-pane="board"] .m-board-pane .m-lane'), 'the board view drew its status columns')
  assert.equal(JSON.parse(storage.items.get(MEMORY_KEY)).tab, 'board')

  // The wiki tab: no path line, toolbar or add field; without the wiki's reads it says it cannot read the wiki.
  assert.equal(panel.$('.m-toolbar-slot').hidden, false)
  panel.tab('wiki').click()
  assert.equal(panel.$('.m-shell-head').hidden, true)
  assert.equal(panel.$('.m-toolbar-slot').hidden, true)
  assert.equal(panel.$('#m-panel-wiki').hidden, false)
  assert.equal(panel.$('#m-panel-list').hidden, true)
  assert.equal(panel.$('#m-panel-wiki .m-empty-title').textContent, en.shell.wikiUnavailable)

  // The tab bar's refresh time reads again.
  assert.match(panel.$('.moai-refresh').textContent, /^↻ \d{2}:\d{2}:\d{2}$/)
  panel.$('.moai-refresh').click()
  assert.deepEqual(panel.calls, [['refresh']])
})

test('shell: the position and visibility toggles are remembered and come back on the next boot', () => {
  const storage = fakeStorage()
  const panel = boot(storage)
  const toggle = panel.$('.moai-position-toggle')
  assert.equal(toggle.getAttribute('aria-label'), 'Move details to the left')
  assert.equal(panel.$('.m-split').dataset.position, 'bottom')
  toggle.click()
  assert.equal(panel.store.getState().detail.position, 'left')
  assert.equal(panel.$('.m-split').dataset.position, 'left')
  assert.equal(toggle.getAttribute('aria-label'), 'Move details to the bottom')
  assert.equal(panel.shell.layout().mainWidth, panel.shell.layout().width * (1 - 0.45))

  // The resizer's keys change the ratio of the position shown, and that is remembered too.
  panel.$('.m-split-resizer').dispatchEvent(new panel.dom.window.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
  const left = panel.store.getState().detail.ratio.left
  assert.equal(left.toFixed(4), '0.4700')

  const visibility = panel.$('.moai-detail-toggle')
  assert.equal(visibility.getAttribute('aria-pressed'), 'true')
  visibility.click()
  assert.equal(panel.$('.m-split').dataset.detail, 'hidden')
  assert.equal(visibility.getAttribute('aria-label'), 'Show details')
  panel.tab('board').click()

  assert.deepEqual(JSON.parse(storage.items.get(MEMORY_KEY)), { version: 1, tab: 'board', detail: { visible: false, position: 'left', ratio: { left, bottom: 0.3 } } })

  const again = boot(storage)
  assert.equal(again.tab('board').getAttribute('aria-selected'), 'true')
  assert.equal(again.$('.m-pane-main[data-pane="board"]').hidden, false)
  assert.equal(again.$('.m-split').dataset.position, 'left')
  assert.equal(again.$('.m-split').dataset.detail, 'hidden')
  assert.equal(again.$('.m-split').style.getPropertyValue('--m-detail-size'), '47.00%')
})

test('shell: a narrow panel forces the detail to the bottom without forgetting the choice', () => {
  const storage = fakeStorage()
  storage.items.set(MEMORY_KEY, JSON.stringify({ detail: { position: 'left' } }))
  const panel = boot(storage)
  assert.equal(panel.$('.m-split').dataset.position, 'left')
  panel.shell.setWidth(500)
  const toggle = panel.$('.moai-position-toggle')
  assert.equal(panel.$('.m-split').dataset.position, 'bottom')
  assert.equal(toggle.getAttribute('aria-disabled'), 'true')
  assert.equal(toggle.title, en.shell.detailForcedBottom)
  toggle.click()
  assert.equal(panel.store.getState().detail.position, 'left', 'the disabled toggle changes nothing')
  assert.deepEqual(panel.shell.layout(), { width: 500, mainWidth: 500, position: 'bottom', detailVisible: true })
  panel.shell.setWidth(900)
  assert.equal(panel.$('.m-split').dataset.position, 'left')
})

test('shell: the path line goes where its segments point, and the list follows', () => {
  const panel = boot(fakeStorage())
  const rows = () => [...panel.doc.querySelectorAll('.m-pane-main[data-pane="list"] .moai-line[data-id]')].map((row) => row.dataset.id)
  assert.deepEqual([...panel.doc.querySelectorAll('.m-crumb')].map((crumb) => crumb.textContent), ['/'])
  panel.store.dispatch({ type: 'enter', path: ['m:se-m1', 'e:se-e1'] })
  assert.deepEqual([...panel.doc.querySelectorAll('.m-crumb')].map((crumb) => crumb.textContent), ['/', 'v2.0', 'E3 views'])
  assert.equal(panel.$('.m-crumb[aria-current="location"]').textContent, 'E3 views')
  assert.equal(rows().length, 2)
  assert.equal(panel.doc.querySelectorAll('.moai-line').length, 3, 'the `..` line is a line without an id')
  panel.doc.querySelectorAll('.m-crumb')[1].click()
  assert.deepEqual(panel.store.getState().path, ['m:se-m1'])
  panel.$('.m-crumb').click()
  assert.deepEqual(panel.store.getState().path, [])
  // The badge says why rows are missing; a search on adds its own with the way out.
  assert.equal(panel.$('.m-crumbs-badges').textContent, 'deferred hidden')
  panel.store.dispatch({ type: 'search', text: 'grid' })
  assert.match(panel.$('.m-crumbs-badges').textContent, /\/grid.*Esc clears it/)
})

test('shell: choosing a row reads its detail through the shell intents; the add field reports through say', () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'enter', path: ['m:se-m1', 'e:se-e1'] })
  const row = [...panel.doc.querySelectorAll('.moai-list .moai-line[data-id]')].find((line) => line.dataset.id === 'se-e1.b')
  row.click()
  assert.deepEqual(panel.calls.at(-1), ['choose', 'se-e1.b'])
  assert.equal(panel.$('.moai-detail .m-empty-title').textContent, en.detail.none, 'nothing loaded yet: the pane says to choose')
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'se-e1.b', row: ROWS[3] } })
  assert.equal(panel.$('.moai-detail h2').textContent, 'Board')
  assert.equal(panel.$('.moai-detail .m-empty-title'), null)

  assert.equal(panel.$('.moai-add'), null, 'the panel reads only: no add field')
  panel.shell.say('The run failed.', true)
  assert.equal(panel.$('.moai-result').className, 'moai-result m-banner m-banner-danger')
  assert.equal(panel.$('.moai-result').getAttribute('role'), 'status', 'the result line is a live region')
})

test('shell: a panel that opens on the wiki tab starts without the path line and the add field', () => {
  const storage = fakeStorage()
  storage.items.set(MEMORY_KEY, JSON.stringify({ tab: 'wiki' }))
  const panel = boot(storage)
  assert.equal(panel.$('#m-panel-wiki').hidden, false)
  assert.equal(panel.$('#m-panel-list').hidden, true)
  assert.equal(panel.$('.m-shell-head').hidden, true)
  // And they come back with the list.
  panel.tab('list').click()
  assert.equal(panel.$('.m-shell-head').hidden, false)
})

test('shell: the path line goes up to the place it left, and the focus stays in the list', () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'enter', path: ['m:se-m1', 'e:se-e1'], cursor: 'se-e1.a' })
  const crumb = panel.doc.querySelectorAll('.m-crumb')[1]
  crumb.focus()
  crumb.click()
  assert.deepEqual(panel.store.getState().path, ['m:se-m1'])
  assert.equal(panel.store.getState().cursor, 'se-e1', 'the cursor lands on the folder just left, as Backspace does')
  assert.equal(panel.doc.activeElement, panel.$('.moai-list'))
  // A segment that is not above the place (the current one) changes no cursor rule: it stays.
  panel.doc.querySelectorAll('.m-crumb')[1].click()
  assert.equal(panel.store.getState().cursor, 'se-e1')
})

test('shell: closing the detail hands the focus back to the list', () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'enter', path: ['m:se-m1', 'e:se-e1'] })
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'se-e1.b', row: ROWS[3] } })
  const close = panel.$('.moai-detail [data-focus="close"]')
  close.focus()
  assert.equal(panel.doc.activeElement, close)
  close.click()
  assert.equal(panel.doc.activeElement, panel.$('.moai-list'))
  assert.equal(panel.store.getState().focus, 'list')
})

test('shell: a tab comes back with its own scroll, and what the pane decided while drawing stands', () => {
  const many = Array.from({ length: 300 }, (_, n) => ({ id: `t-${String(n).padStart(3, '0')}`, kind: 'issue', title: `Task ${n}`, status: 'todo', priority: 2 }))
  const panel = boot(fakeStorage(), { rows: many })
  const listRoot = panel.$('.m-pane-main[data-pane="list"]')
  Object.defineProperty(listRoot, 'clientHeight', { configurable: true, get: () => 280 })
  panel.store.dispatch({ type: 'cursor', id: 't-000' })
  panel.tab('board').click()
  // The cursor moves far away while the list is hidden (a card was clicked).
  panel.store.dispatch({ type: 'cursor', id: 't-250' })
  panel.tab('list').click()
  assert.ok(listRoot.scrollTop > 5000, `the list reveals the cursor (scrollTop ${listRoot.scrollTop})`)
  assert.ok(panel.doc.querySelector('.moai-line[data-key="t-250"]'), 'the cursor row is drawn')
  // A scroll the user left is still put back when nothing moved: leave and come back.
  listRoot.scrollTop = 1400
  panel.tab('board').click()
  panel.tab('list').click()
  assert.equal(listRoot.scrollTop, 1400)
})

test('shell: a failed detail read is told once, in the pane, and does not hide the list problem', () => {
  const panel = boot(fakeStorage())
  const cancelled = { kind: 'cancelled' }
  panel.store.dispatch({ type: 'detail-failed', id: 'se-e1.a', problem: cancelled })
  // As information, in the pane that asked; the shell's banners stay for the list.
  const pane = panel.$('.m-split-detail')
  assert.equal(pane.querySelectorAll('.m-banner').length, 1)
  assert.equal(pane.querySelector('.m-banner').getAttribute('role'), 'status')
  assert.equal(panel.$('.moai-notice').textContent, '')
  assert.equal(panel.$('.moai-error').textContent, '')
  // A list problem keeps its place while the detail's stands.
  panel.store.dispatch({ type: 'board-failed', problem: { kind: 'moai-error', message: 'broken' } })
  assert.match(panel.$('.moai-error').textContent, /broken/)
  assert.match(pane.textContent, /read was cancelled/)
  // With the pane hidden the shell says it, so a read the user asked for is never silent.
  panel.store.dispatch({ type: 'board-loaded', data: { rows: ROWS, loadedAt: Date.now() } })
  panel.store.dispatch({ type: 'detail-visible', visible: false })
  assert.match(panel.$('.moai-notice').textContent, /read was cancelled/)
})

test('shell: moai web out of reach is one sentence in its banner, and a banner is written only when it says something new', () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'board-failed', problem: { kind: 'not-started' } })
  const error = panel.$('.moai-error')
  // One child, so the flex banner lays the sentence out as text.
  assert.equal(error.children.length, 1)
  assert.equal(error.textContent, en.unreachable)

  // A live region is read again when it is written: an unchanged problem is left alone.
  panel.store.dispatch({ type: 'board-failed', problem: { kind: 'moai-error', message: 'broken' } })
  const alert = panel.$('.moai-error')
  let writes = 0
  const observer = new panel.dom.window.MutationObserver((records) => { writes += records.length })
  observer.observe(alert, { childList: true, subtree: true, characterData: true })
  panel.store.dispatch({ type: 'cursor', id: 'se-e1.a' })
  panel.store.dispatch({ type: 'cursor', id: 'se-e1.b' })
  panel.store.dispatch({ type: 'search', text: 'x' })
  return Promise.resolve().then(() => {
    writes += observer.takeRecords().length
    assert.equal(writes, 0)
    // A failure that happens again is a new outcome and is told again.
    panel.store.dispatch({ type: 'board-failed', problem: { kind: 'moai-error', message: 'broken' } })
    writes += observer.takeRecords().length
    assert.ok(writes > 0)
  })
})

test('shell: the store\'s focus follows the DOM focus into a pane, not only focusPane', () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'se-e1.b', row: ROWS[3] } })
  assert.equal(panel.store.getState().focus, 'list')
  // A click on the detail's folded history focuses its summary (no focusPane call).
  panel.$('.moai-detail summary').focus()
  assert.equal(panel.store.getState().focus, 'detail')
  // Back into the list by a click or Tab.
  panel.$('.moai-list').focus()
  assert.equal(panel.store.getState().focus, 'list')
})

test('unread: unreadBadges counts the rows the CLI marked unread, archive rows never; null when none (se-ehdi.zcx)', async () => {
  const { unreadBadges, newChip } = await panelModule('view/unread.ts')
  const { createTabs } = await panelModule('view/tabs.ts')
  const rows = [{ id: 'a', unread: true }, { id: 'b' }, { id: 'c', unread: true }, { id: 'd', unread: true, archived: true }]
  assert.deepEqual(unreadBadges(rows), { board: 2, list: 2 })
  assert.deepEqual(unreadBadges([...rows, { id: 'a', unread: true }]), { board: 2, list: 2 }, 'twin lines of one id count once')
  assert.deepEqual(unreadBadges([{ id: 'a' }]), { board: null, list: null }, 'a source that says nothing: no badge')
  assert.deepEqual(unreadBadges([]), { board: null, list: null })
  const doc = new JSDOM('<!doctype html><body></body>').window.document
  const chip = newChip(doc)
  assert.equal(chip.textContent, 'NEW')
  assert.ok(chip.classList.contains('m-chip-new'))
  // The tab badge the shell sets from it.
  const tabs = createTabs(doc, { selected: 'list', onSelect() {} })
  tabs.setBadge('list', unreadBadges(rows).list)
  assert.equal(tabs.el.querySelector('#m-tab-list .m-chip-new').textContent, '2')
  tabs.setBadge('list', unreadBadges([]).list)
  assert.equal(tabs.el.querySelector('.m-chip-new'), null)
})

test('unread: the shell puts the unread count on the board and list tabs, and takes it off when the rows are read (se-ehdi.zcx)', () => {
  const unreadAmong = (...ids) => ROWS.map((row) => (ids.includes(row.id) ? { ...row, unread: true } : row))
  // main.ts builds one IssueIndex per read; the shell is handed the index of the rows the store holds.
  const indexes = new WeakMap()
  const indexOf = (state) => {
    const held = state.board.value?.rows
    if (!held) return null
    if (!indexes.has(held)) indexes.set(held, build(held))
    return indexes.get(held)
  }
  const panel = boot(fakeStorage(), { rows: unreadAmong('se-e1.a', 'se-e1.b'), indexOf })
  const badge = (id) => panel.tab(id).querySelector('.m-chip-new')?.textContent ?? null
  // What the re-read after `moai read` brings back: the same rows, the key gone from the ones that were marked.
  const reload = (rows) => panel.store.dispatch({ type: 'board-loaded', data: { rows, more: 0, loadedAt: Date.now() } })

  // Two rows are unread: both tabs that list rows say so, and are named with the count; the wiki tab says nothing.
  assert.equal(badge('list'), '2')
  assert.equal(badge('board'), '2')
  assert.equal(badge('wiki'), null)
  assert.equal(panel.tab('list').getAttribute('aria-label'), 'List 2')
  // It stays on the bar as the tab changes.
  panel.tab('board').click()
  assert.equal(badge('list'), '2')
  assert.equal(badge('board'), '2')
  panel.tab('list').click()

  // One row read: the count follows. Every row read: the badges are gone, and the tabs are named by their title again.
  reload(unreadAmong('se-e1.b'))
  assert.equal(badge('list'), '1')
  assert.equal(badge('board'), '1')
  reload(unreadAmong())
  assert.equal(badge('list'), null)
  assert.equal(badge('board'), null)
  assert.equal(panel.doc.querySelector('[role="tablist"] .m-chip-new'), null)
  assert.equal(panel.tab('list').getAttribute('aria-label'), null)
})

test('shell: the activity line says a read is out, and an unchanged line is not rewritten', async () => {
  const panel = boot(fakeStorage())
  const line = panel.$('.moai-activity')
  const writes = []
  new panel.dom.window.MutationObserver((records) => writes.push(...records)).observe(line, { childList: true, characterData: true, subtree: true })
  panel.shell.activity({ phase: 'running', cancellable: true })
  assert.equal(line.textContent, en.running)
  assert.equal(panel.$('.moai-cancel').hidden, false)
  await Promise.resolve()
  const after = writes.length
  panel.shell.activity({ phase: 'running', cancellable: true })
  await Promise.resolve()
  assert.equal(writes.length, after, 'the live region is left alone')
  panel.shell.activity({ phase: 'idle', cancellable: false })
  assert.equal(line.textContent, '')
  assert.equal(panel.$('.moai-cancel').hidden, true)
})

test('shell: what moai said beside the lists is told once, under its lead, and goes with the next clean read', async () => {
  const panel = boot(fakeStorage())
  panel.store.dispatch({ type: 'board-loaded', data: { rows: ROWS, loadedAt: 1, notes: ['line 7 could not be read'] } })
  const said = panel.$('.moai-said')
  assert.deepEqual([...said.querySelectorAll('p')].map((p) => p.textContent), [en.said, 'line 7 could not be read'])
  const writes = []
  new panel.dom.window.MutationObserver((records) => writes.push(...records)).observe(said, { childList: true, subtree: true })
  panel.store.dispatch({ type: 'board-loaded', data: { rows: ROWS, loadedAt: 2, notes: ['line 7 could not be read'] } })
  await Promise.resolve()
  assert.equal(writes.length, 0, 'the same lines are not read out again')
  panel.store.dispatch({ type: 'board-loaded', data: { rows: ROWS, loadedAt: 3 } })
  assert.equal(said.textContent, '')
})

test('memory: the default sort lasts for the session, lives under one key, and is applied at boot', () => {
  const storage = createSessionStorage()
  const sortDefault = createSortDefault(storage)
  assert.equal(sortDefault.get(), null)
  assert.equal(sortDefault.persistent, false)
  const sort = { fields: [{ key: 'updated', dir: 'desc' }, { key: 'title', dir: 'asc' }], reversed: true }
  sortDefault.set(sort)
  assert.deepEqual(sortDefault.get(), sort)
  assert.deepEqual(JSON.parse(storage.read(SORT_DEFAULT_KEY)), sort, 'a persisting backend finds it under the same key')
  const store = createStore()
  remember(store, createMemory(() => null), sortDefault, storage)
  assert.deepEqual(store.getState().sort, sort)
  // The store holds it too, so an emptied order (any path: store.ts 'sort') and the path line meet the same default.
  assert.deepEqual(store.getState().defaultSort, sort)
  store.dispatch({ type: 'sort', sort: { fields: [{ key: 'title', dir: 'asc' }], reversed: false } })
  store.dispatch({ type: 'sort', sort: { fields: [], reversed: false } })
  assert.deepEqual(store.getState().sort, sort)
  sortDefault.clear()
  assert.equal(sortDefault.get(), null)
})

test('memory: saving the order on screen as the default (D) leaves the workspace following it at once', () => {
  const storage = createSessionStorage()
  const store = createStore()
  remember(store, createMemory(() => null), createSortDefault(storage), storage)
  const sort = { fields: [{ key: 'title', dir: 'asc' }], reversed: false }
  store.dispatch({ type: 'sort', sort })
  assert.deepEqual(JSON.parse(storage.read(QUERY_KEY)).sort, sort)
  // Only the default changes (keys/wiring.ts D): the order is now the default, so the workspace keeps none of its own.
  store.dispatch({ type: 'sort-default', sort })
  assert.equal(JSON.parse(storage.read(QUERY_KEY)).sort, null)
})

test('memory: a stored sort that is not usable is no default', () => {
  assert.equal(normalizeSort(null), null)
  assert.equal(normalizeSort({ fields: [] }), null)
  assert.equal(normalizeSort({ fields: [{ key: 'nope', dir: 'asc' }] }), null)
  assert.equal(normalizeSort({ fields: [{ key: 'title', dir: 'sideways' }] }), null)
  assert.equal(normalizeSort({ fields: [{ key: 'title', dir: 'asc' }, { key: 'title', dir: 'desc' }] }), null)
  const storage = createSessionStorage()
  storage.write(SORT_DEFAULT_KEY, '{not json')
  assert.equal(createSortDefault(storage).get(), null)
})

test('memory: filter, sort, cells and view are kept under one key and restored at boot', () => {
  const storage = createSessionStorage()
  const none = createSortDefault(storage)
  const first = createStore()
  remember(first, createMemory(() => null), none, storage)
  first.dispatch({ type: 'search', text: 'abc' })
  first.dispatch({ type: 'filter', text: 'status=todo' })
  first.dispatch({ type: 'sort', sort: { fields: [{ key: 'title', dir: 'desc' }], reversed: false } })
  first.dispatch({ type: 'column', column: 'tags', on: true })
  first.dispatch({ type: 'view', view: { hideBacklog: true, hiddenStatuses: ['done'] } })
  assert.ok(storage.read(QUERY_KEY))
  const second = createStore()
  remember(second, createMemory(() => null), none, storage)
  const state = second.getState()
  assert.equal(state.search.text, '', 'the search text is not kept')
  assert.equal(state.filter.applied, 'status=todo')
  assert.deepEqual(state.sort.fields, [{ key: 'title', dir: 'desc' }])
  assert.equal(state.columns.tags, true)
  assert.equal(state.view.hideBacklog, true)
  assert.deepEqual(state.view.hiddenStatuses, ['done'])
})

test('memory: an order that is the default is not kept, so a later default decides the next opening', () => {
  const storage = createSessionStorage()
  const defaults = createSortDefault(storage)
  const first = createStore()
  remember(first, createMemory(() => null), defaults, storage)
  // A cell toggled on the default order: the workspace keeps the cell, not the order.
  first.dispatch({ type: 'column', column: 'tags', on: true })
  assert.equal(JSON.parse(storage.read(QUERY_KEY)).sort, null)
  // `D` saves another order as the default afterwards.
  defaults.set({ fields: [{ key: 'updated', dir: 'desc' }], reversed: false })
  const second = createStore()
  remember(second, createMemory(() => null), defaults, storage)
  assert.deepEqual(second.getState().sort.fields, [{ key: 'updated', dir: 'desc' }], 'the next opening is on the saved default')
  assert.equal(second.getState().columns.tags, true)
  // A cursor move writes nothing; an order of the workspace's own is kept and wins over the default.
  let writes = 0
  const counting = { read: (key) => storage.read(key), write: (key, value) => { writes++; storage.write(key, value) }, persistent: false }
  const third = createStore()
  remember(third, createMemory(() => null), defaults, counting)
  third.dispatch({ type: 'cursor', id: 'se-e1.a' })
  assert.equal(writes, 0)
  third.dispatch({ type: 'sort', sort: { fields: [{ key: 'title', dir: 'asc' }], reversed: false } })
  assert.equal(writes, 1)
  const fourth = createStore()
  remember(fourth, createMemory(() => null), defaults, storage)
  assert.deepEqual(fourth.getState().sort.fields, [{ key: 'title', dir: 'asc' }])
})

test('memory: a settings storage that throws only forgets; the store and its other listeners go on', () => {
  const broken = { read: () => { throw new Error('unavailable') }, write: () => { throw new Error('quota') }, persistent: true }
  const defaults = createSortDefault(broken)
  assert.equal(defaults.get(), null)
  defaults.set({ fields: [{ key: 'title', dir: 'asc' }], reversed: false })
  defaults.clear()
  const store = createStore()
  remember(store, createMemory(() => null), defaults, broken)
  let drawn = 0
  store.subscribe(() => drawn++)
  store.dispatch({ type: 'column', column: 'tags', on: true })
  assert.equal(store.getState().columns.tags, true)
  assert.equal(drawn, 1, 'a listener after remember() still hears the change')
})

test('memory: query settings that are not usable are left out', () => {
  assert.deepEqual(normalizeQuery(null), {})
  assert.deepEqual(normalizeQuery({ filter: 3, sort: { fields: [] }, columns: { tags: 'x', id: false }, view: { hideDeferred: 1 } }), { columns: { id: false }, view: {} })
})
