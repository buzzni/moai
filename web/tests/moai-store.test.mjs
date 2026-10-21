import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { COLUMN_DEFAULTS, createStore, detailPosition, initialState, keepCursor, matchKey, reduce } = await panelModule('store.ts')
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
  assert.deepEqual([state.search, state.filter], [{ text: '', open: false }, { text: '', applied: '' }])
  assert.equal(state.boot, null, 'nothing is known of moai\'s config before /api/boot answers')
  assert.deepEqual(state.matched, { key: '', ids: null, loading: false, problem: null })
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

test('the toolbar keeps its state in the store: search as typed, the filter on Enter (always), the sort through query.ts', () => {
  const tb = (state, action) => reduce(state, { type: 'toolbar', action })
  let state = tb(initialState(), { type: 'search-text', text: 'grid' })
  assert.deepEqual(state.search, { text: 'grid', open: true })
  // Typing changes the text, not the question; Enter applies whatever was typed: judging it is moai's.
  state = tb(state, { type: 'filter-text', text: 'status=todo', cursor: 11 })
  assert.deepEqual(state.filter, { text: 'status=todo', applied: '' })
  assert.equal(state.toolbar.cursor, 11)
  state = tb(state, { type: 'filter-apply' })
  assert.deepEqual(state.filter, { text: 'status=todo', applied: 'status=todo' })
  state = tb(tb(state, { type: 'filter-text', text: 'nope=1', cursor: 6 }), { type: 'filter-apply' })
  assert.deepEqual(state.filter, { text: 'nope=1', applied: 'nope=1' }, 'a text the page cannot read is applied too')
  // A chip removal applies at once; the `filter` action sets and applies (chips, keys).
  state = reduce(state, { type: 'filter', text: 'tag=ui tag=bug' })
  state = tb(state, { type: 'chip-remove', index: 0 })
  assert.deepEqual(state.filter, { text: 'tag=bug', applied: 'tag=bug' })
  state = tb(state, { type: 'sort-edit', key: 'status', how: 'replace' })
  assert.deepEqual(state.sort, { fields: [{ key: 'status', dir: 'asc' }], reversed: false })
  state = tb(state, { type: 'sort-edit', key: 'due', how: 'replace' })
  assert.deepEqual(state.sort, { fields: [{ key: 'due', dir: 'asc' }], reversed: false }, 'the deadline is a sort key')
  state = tb(state, { type: 'search-close' })
  assert.deepEqual(state.search, { text: '', open: false })
})

test('matchKey: nothing to ask while the filter and the search are empty; else one key for filter, search and archive', () => {
  const state = initialState()
  assert.equal(matchKey(state), '')
  assert.equal(matchKey({ ...state, filter: { text: '  ', applied: '   ' } }), '')
  const asked = matchKey({ ...state, filter: { text: '', applied: 'status=todo' } })
  assert.notEqual(asked, '')
  assert.notEqual(matchKey({ ...state, search: { text: 'grid', open: true } }), asked)
  assert.notEqual(matchKey({ ...state, filter: { text: '', applied: 'status=todo' }, view: { ...state.view, showArchived: true } }), asked, 'the archive is part of the question')
})

test('moai\'s answers: ids come for the question on screen, an older question\'s answer is dropped, a refusal keeps the ids', () => {
  let state = reduce(initialState(), { type: 'match-loading', key: 'k1' })
  assert.deepEqual(state.matched, { key: 'k1', ids: null, loading: true, problem: null })
  state = reduce(state, { type: 'matched', key: 'k1', ids: new Set(['a']) })
  assert.deepEqual([...state.matched.ids], ['a'])
  assert.equal(state.matched.loading, false)
  // A newer question keeps the older ids on screen until its own come.
  state = reduce(state, { type: 'match-loading', key: 'k2' })
  assert.deepEqual([...state.matched.ids], ['a'])
  assert.equal(state.matched.loading, true)
  // The answer of the first question arrives late: dropped.
  assert.equal(reduce(state, { type: 'matched', key: 'k1', ids: new Set(['z']) }), state)
  assert.equal(reduce(state, { type: 'match-failed', key: 'k1', problem: { kind: 'moai-error', message: 'x' } }), state)
  const refused = reduce(state, { type: 'match-failed', key: 'k2', problem: { kind: 'moai-error', message: 'unknown filter key' } })
  assert.equal(refused.matched.problem.message, 'unknown filter key')
  assert.deepEqual([...refused.matched.ids], ['a'], 'the ids stay beside the refusal')
  assert.equal(refused.matched.loading, false)
  // The same question asked again (rows read since) keeps its refusal while it waits; another question does not wear it.
  assert.equal(reduce(refused, { type: 'match-loading', key: 'k2' }).matched.problem.message, 'unknown filter key')
  assert.equal(reduce(refused, { type: 'match-loading', key: 'k3' }).matched.problem, null, 'k2\'s refusal is not k3\'s')
  // The next answer clears the refusal.
  const next = reduce(reduce(refused, { type: 'match-loading', key: 'k3' }), { type: 'matched', key: 'k3', ids: new Set() })
  assert.equal(next.matched.problem, null)
})

test('/api/boot is kept as it came', () => {
  const boot = { lang: 'en', version: '1', project: 'p', root: '/r', statuses: ['todo', 'done'], naming: 'name', glyphs: { todo: '·' }, unknown_glyph: '○', columns: { backlog: { name: 'Backlog', glyph: '◇' }, shelved: { name: 'Deferred', glyph: '‖' } }, filter_keys: [], sort_fields: [] }
  assert.equal(reduce(initialState(), { type: 'boot', boot }).boot, boot)
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
  const groups = new Map([['m', { todo: 1, done: 1 }]])
  const counted = reduce(state, { type: 'groups', data: first, groups })
  assert.equal(counted.board.value.groups, groups)
  assert.equal(counted.board.value.rows, rows, 'the rows stay')
  assert.equal(counted.cursor, 'b')
  // A later read's board: the first read's late counts are dropped.
  const second = { rows, more: 0, loadedAt: 2 }
  state = reduce(counted, { type: 'board-loaded', data: second })
  assert.equal(reduce(state, { type: 'groups', data: first, groups: new Map() }), state)
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
  const carried = new Map([['m', { todo: 1 }]])
  const data = { rows: [{ id: 'a' }], more: 0, loadedAt: 1, groups: carried }
  const state = reduce(initialState(), { type: 'board-loaded', data })
  const dropped = reduce(state, { type: 'groups', data, groups: null })
  assert.equal('groups' in dropped.board.value, false)
  assert.equal(dropped.board.value.rows, data.rows)
})
