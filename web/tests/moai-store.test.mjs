import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { COLUMN_DEFAULTS, createStore, detailPosition, initialState, keepCursor, reduce } = await panelModule('store.ts')
const { FORCE_BOTTOM_BELOW } = await panelModule('view/responsive.ts')
const { MAX_RATIO, MIN_RATIO } = await panelModule('view/split.ts')

test('the panel starts on the list with the TUI defaults', () => {
  const state = initialState()
  assert.equal(state.tab, 'list')
  assert.deepEqual(state.path, [])
  assert.equal(state.cursor, null)
  assert.deepEqual(state.detail, { visible: true, position: 'bottom', ratio: { left: 0.45, bottom: 0.3 } })
  assert.equal(state.view.hideDeferred, true)
  assert.deepEqual(state.sort, { fields: [{ key: 'priority', dir: 'asc' }], reversed: false })
  // The cells are named as the key table's CellField: the name line is `names`.
  assert.deepEqual(Object.entries(state.columns).filter(([, on]) => on).map(([name]) => name), ['id', 'priority', 'counts', 'names'])
  assert.deepEqual([state.search, state.filter], [{ text: '', scope: 'all', open: false }, { text: '', applied: '' }])
  assert.deepEqual(COLUMN_DEFAULTS.tags, false)
})

test('an action that changes nothing keeps the same state, so nothing re-renders', () => {
  const state = initialState()
  assert.equal(reduce(state, { type: 'tab', tab: 'list' }), state)
  assert.equal(reduce(state, { type: 'cursor', id: null }), state)
  assert.equal(reduce(state, { type: 'focus', focus: 'list' }), state)
})

test('the detail ratio is kept per position and clamped to 15–85%', () => {
  let state = reduce(initialState(), { type: 'detail-ratio', ratio: 0.95 })
  assert.equal(state.detail.ratio.bottom, MAX_RATIO)
  state = reduce(state, { type: 'detail-position', position: 'left' })
  state = reduce(state, { type: 'detail-ratio', ratio: 0.01 })
  assert.deepEqual(state.detail.ratio, { left: MIN_RATIO, bottom: MAX_RATIO })
  // An unusable ratio falls back to that position's own default.
  assert.equal(reduce(state, { type: 'detail-ratio', ratio: Number.NaN }).detail.ratio.left, 0.45)
  // A narrow panel shows the bottom whatever was chosen; the split says so, and that ratio is the one that moves.
  state = reduce(state, { type: 'detail-ratio', ratio: 0.5, position: 'bottom' })
  assert.deepEqual(state.detail.ratio, { left: MIN_RATIO, bottom: 0.5 })
})

test('below 640px the detail stands at the bottom whatever was chosen', () => {
  const detail = { visible: true, position: 'left', ratio: { left: 0.45, bottom: 0.3 } }
  assert.equal(detailPosition(detail, FORCE_BOTTOM_BELOW - 1), 'bottom')
  assert.equal(detailPosition(detail, FORCE_BOTTOM_BELOW), 'left')
})

test('hiding the detail moves the focus back to the list', () => {
  let state = reduce(initialState(), { type: 'focus', focus: 'detail' })
  state = reduce(state, { type: 'detail-visible', visible: false })
  assert.equal(state.focus, 'list')
  assert.equal(state.detail.visible, false)
})

test('a column turned off stays known and off; an empty sort falls back to priority', () => {
  let state = reduce(initialState(), { type: 'column', column: 'id', on: false })
  assert.equal(state.columns.id, false)
  assert.ok(state.columnsKnown.includes('id'))
  state = reduce(state, { type: 'sort', sort: { fields: [], reversed: true } })
  assert.deepEqual(state.sort, { fields: [{ key: 'priority', dir: 'asc' }], reversed: false })
})

test('an emptied order returns to the saved default on every path: the sort action and the toolbar', () => {
  const saved = { fields: [{ key: 'updated', dir: 'desc' }], reversed: true }
  let state = reduce(initialState(), { type: 'sort-default', sort: saved })
  // Saving a default leaves the order on screen alone.
  assert.deepEqual(state.sort, { fields: [{ key: 'priority', dir: 'asc' }], reversed: false })
  state = reduce(state, { type: 'sort', sort: { fields: [], reversed: false } })
  assert.deepEqual(state.sort, saved)
  assert.notEqual(state.sort, state.defaultSort, 'a copy: editing the order does not edit the default')
  state = reduce(state, { type: 'sort', sort: { fields: [{ key: 'title', dir: 'asc' }], reversed: false } })
  state = reduce(state, { type: 'toolbar', action: { type: 'sort-set', sort: { fields: [], reversed: false } } })
  assert.deepEqual(state.sort, saved)
  // No saved default (cleared, or none read): the built-in order again.
  state = reduce(state, { type: 'sort-default', sort: null })
  state = reduce(state, { type: 'sort', sort: { fields: [], reversed: false } })
  assert.deepEqual(state.sort, { fields: [{ key: 'priority', dir: 'asc' }], reversed: false })
})

test('the toolbar keeps its state in the store: search as typed, the filter on Enter, the sort through query.ts', () => {
  const tb = (state, action) => reduce(state, { type: 'toolbar', action })
  let state = tb(initialState(), { type: 'search-text', text: 'grid' })
  assert.deepEqual(state.search, { text: 'grid', scope: 'all', open: true })
  state = tb(state, { type: 'scope-next' })
  assert.equal(state.search.scope, 'id')
  // Typing changes the text, not the rows; Enter applies a text that parses, and only that.
  state = tb(state, { type: 'filter-text', text: 'status=todo', cursor: 11 })
  assert.deepEqual(state.filter, { text: 'status=todo', applied: '' })
  assert.equal(state.toolbar.cursor, 11)
  state = tb(state, { type: 'filter-apply' })
  assert.deepEqual(state.filter, { text: 'status=todo', applied: 'status=todo' })
  state = tb(tb(state, { type: 'filter-text', text: 'nope=1', cursor: 6 }), { type: 'filter-apply' })
  assert.deepEqual(state.filter, { text: 'nope=1', applied: 'status=todo' })
  // A chip removal applies at once; the `filter` action sets and applies (chips, keys).
  state = reduce(state, { type: 'filter', text: 'tag=ui tag=bug' })
  state = tb(state, { type: 'chip-remove', index: 0 })
  assert.deepEqual(state.filter, { text: 'tag=bug', applied: 'tag=bug' })
  state = tb(state, { type: 'sort-edit', key: 'status', how: 'replace' })
  assert.deepEqual(state.sort, { fields: [{ key: 'status', dir: 'asc' }], reversed: false })
  // A toolbar act that changes nothing keeps the state.
  assert.equal(tb(state, { type: 'scope-set', scope: 'id' }), state)
  state = tb(state, { type: 'search-close' })
  assert.deepEqual(state.search, { text: '', scope: 'id', open: false })
})

test('the wiki state is the wiki view\'s own shape, saved by a wiki action', () => {
  const state = initialState()
  assert.deepEqual(state.wiki, { current: null, cursor: null, trail: [], query: '', raw: false, scrollTop: 0, selectedLink: -1, pane: 'list' })
  const saved = { ...state.wiki, current: 'guide', trail: [{ slug: 'README', scrollTop: 40 }] }
  assert.equal(reduce(state, { type: 'wiki', wiki: saved }).wiki, saved)
})

test('folders unfold and fold by path', () => {
  let state = reduce(initialState(), { type: 'toggle-expanded', path: ['m:r1', 'e:e1'] })
  assert.equal(state.expanded.length, 1)
  state = reduce(state, { type: 'toggle-expanded', path: ['m:r1', 'e:e1'] })
  assert.deepEqual(state.expanded, [])
})

test('after a reload the cursor stays on its id, or moves to the nearest row still there', () => {
  assert.equal(keepCursor('b', ['a', 'b', 'c'], new Set(['a', 'b', 'c'])), 'b')
  assert.equal(keepCursor('b', ['a', 'b', 'c'], new Set(['a', 'c'])), 'c')
  assert.equal(keepCursor('c', ['a', 'b', 'c'], new Set(['a'])), 'a')
  assert.equal(keepCursor('b', ['a', 'b', 'c'], new Set()), null)
  assert.equal(keepCursor('z', ['a'], new Set(['a'])), null)
  assert.equal(keepCursor(null, ['a'], new Set(['a'])), null)
})

test('a cursor that was never a row (the list\'s `..` line, a bucket) survives a reload', () => {
  const known = new Set(['a', 'b', 'c'])
  assert.equal(keepCursor('..', ['a', 'b', 'c'], new Set(['a', 'b', 'c']), known), '..')
  assert.equal(keepCursor('m:', ['a', 'b'], new Set(['a']), known), 'm:')
  assert.equal(keepCursor('lost', [], new Set(), known), 'lost')
  // A row that was there and is gone still moves to its neighbour.
  assert.equal(keepCursor('b', ['a', 'b', 'c'], new Set(['a', 'c']), known), 'c')
  // Through the reducer: the cursor on `..` stays across a reload of the same rows.
  let state = reduce(initialState(), { type: 'board-loaded', data: { rows: [{ id: 'a' }, { id: 'b' }], more: 0, loadedAt: 1 } })
  state = reduce(state, { type: 'enter', path: ['i:a'], cursor: '..' })
  state = reduce(state, { type: 'board-loaded', data: { rows: [{ id: 'a' }, { id: 'b' }], more: 0, loadedAt: 2 }, order: ['a', 'b'] })
  assert.equal(state.cursor, '..')
  state = reduce(state, { type: 'cursor', id: 'b' })
  state = reduce(state, { type: 'board-loaded', data: { rows: [{ id: 'a' }], more: 0, loadedAt: 3 }, order: ['a', 'b'] })
  assert.equal(state.cursor, 'a')
})

test('a reload keeps the rows read before on screen when it fails', () => {
  let state = reduce(initialState(), { type: 'board-loaded', data: { rows: [{ id: 'a' }, { id: 'b' }], more: 0, loadedAt: 1 } })
  state = reduce(state, { type: 'cursor', id: 'b' })
  state = reduce(state, { type: 'board-loading' })
  assert.equal(state.board.loading, true)
  state = reduce(state, { type: 'board-failed', problem: { kind: 'refused', code: 'declined' } })
  assert.deepEqual(state.board.value.rows.map((row) => row.id), ['a', 'b'])
  assert.deepEqual(state.board.problem, { kind: 'refused', code: 'declined' })
  state = reduce(state, { type: 'board-loaded', data: { rows: [{ id: 'a' }], more: 0, loadedAt: 2 }, order: ['b', 'a'] })
  assert.equal(state.cursor, 'a')
  assert.equal(state.board.problem, null)
})

test('a detail for another row does not show the previous one while it loads', () => {
  let state = reduce(initialState(), { type: 'detail-loaded', data: { id: 'a', row: { id: 'a' } } })
  assert.equal(reduce(state, { type: 'detail-loading', id: 'a' }).shown.value.id, 'a')
  state = reduce(state, { type: 'detail-loading', id: 'b' })
  assert.equal(state.shown.value, null)
  assert.equal(state.shown.loading, true)
})

test('the store notifies subscribers with the new and the previous state', () => {
  const store = createStore()
  const seen = []
  const stop = store.subscribe((state, previous) => seen.push([previous.tab, state.tab]))
  store.dispatch({ type: 'tab', tab: 'board' })
  store.dispatch({ type: 'tab', tab: 'board' })
  stop()
  store.dispatch({ type: 'tab', tab: 'wiki' })
  assert.deepEqual(seen, [['list', 'board']])
  assert.equal(store.getState().tab, 'wiki')
})

test('counts that come after the rows go on the board they belong to, and a later board keeps its own', () => {
  const rows = [{ id: 'a' }, { id: 'b' }]
  const first = { rows, more: 0, loadedAt: 1 }
  let state = reduce({ ...initialState(), cursor: 'b' }, { type: 'board-loaded', data: first })
  const tallies = new Map([['m', [1, 2]]])
  const counted = reduce(state, { type: 'tallies', data: first, tallies })
  assert.equal(counted.board.value.tallies, tallies)
  assert.equal(counted.board.value.rows, rows, 'the rows stay')
  assert.equal(counted.cursor, 'b')
  // A later read's board: the first read's late counts are dropped.
  const second = { rows, more: 0, loadedAt: 2 }
  state = reduce(counted, { type: 'board-loaded', data: second })
  assert.equal(reduce(state, { type: 'tallies', data: first, tallies: new Map() }), state)
})

test('an archive read that failed is its own problem, gone with the next archive read or when archived rows go', () => {
  const problem = { kind: 'refused', code: 'declined' }
  const state = reduce({ ...initialState(), board: { value: { rows: [], more: 0, loadedAt: 1 }, loading: false, problem: null } }, { type: 'archive-failed', problem })
  assert.equal(state.archiveProblem, problem)
  assert.equal(state.board.problem, null, 'not the board\'s')
  assert.equal(reduce(state, { type: 'archive', rows: [] }).archiveProblem, null)
  assert.equal(reduce(state, { type: 'archive', rows: null }).archiveProblem, null)
  const clean = reduce(state, { type: 'archive', rows: null })
  assert.equal(reduce(clean, { type: 'archive', rows: null }), clean)
})

test('late counts that could not be read take the counts the board carried off it', () => {
  const carried = new Map([['m', [1, 2]]])
  const data = { rows: [{ id: 'a' }], more: 0, loadedAt: 1, tallies: carried }
  const state = reduce(initialState(), { type: 'board-loaded', data })
  const dropped = reduce(state, { type: 'tallies', data, tallies: null })
  assert.equal('tallies' in dropped.board.value, false)
  assert.equal(dropped.board.value.rows, data.rows)
})

test('store: a refusal the compatibility banner told goes once a report says it was fixed, not before', () => {
  const report = (change = {}) => ({ moai: 'ok', toolMissing: false, catalog: { state: 'ok', missing: [] }, degraded: [], ...change })
  const missingTool = { kind: 'refused', code: 'tool-missing' }
  const lacking = { kind: 'refused', code: 'unsupported-daemon' }
  let state = reduce(initialState(), { type: 'compat', report: report({ toolMissing: true, catalog: { state: 'missing', missing: ['buzzni.moai.show-archived'] } }) })
  state = reduce(state, { type: 'board-failed', problem: missingTool })
  state = reduce(state, { type: 'archive-failed', problem: lacking })
  state = reduce(state, { type: 'detail-failed', id: 'a', problem: lacking })
  // A reload starts: the problems told by the banner stand (hidden) until a report says otherwise.
  state = reduce(state, { type: 'board-loading' })
  assert.equal(state.board.problem, missingTool)
  // moai found (a run started there), the archive profile still lacking: only moai missing goes.
  state = reduce(state, { type: 'compat', report: report({ catalog: { state: 'missing', missing: ['buzzni.moai.show-archived'] } }) })
  assert.equal(state.board.problem, null)
  assert.equal(state.archiveProblem, lacking)
  // The catalog complete: the archive's refusal goes. The detail's `show` is not a read the probe hears, so its
  // refusal was never the banner's and stays.
  state = reduce(state, { type: 'compat', report: report() })
  assert.equal(state.archiveProblem, null)
  assert.equal(state.shown.problem, lacking)
  // A verdict gone back to unknown (the daemon as a whole) fixes nothing: the refusal is the board's to tell again.
  state = reduce(state, { type: 'archive-failed', problem: lacking })
  state = reduce(state, { type: 'compat', report: report({ catalog: { state: 'missing', missing: ['buzzni.moai.show-archived'] } }) })
  state = reduce(state, { type: 'compat', report: report({ catalog: { state: 'unknown', missing: [] } }) })
  assert.equal(state.archiveProblem, lacking)
})
