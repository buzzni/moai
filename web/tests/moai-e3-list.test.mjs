// Moai panel list view (se-c10x.jnc, .koa, .fmd, .eul): the pure model — view toggles, sort order, tree
// lines, columns, windowing — and the view itself in a jsdom document, wired to a real store.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const model = await panelModule('view/list-model.ts')
const { mountList, ROW_HEIGHT } = await panelModule('view/list.ts')
const { createStore, initialState } = await panelModule('store.ts')
const { IssueIndex, NO_MILESTONE, LOST, pathKey } = await panelModule('data/index.ts')
const { copyFor } = await panelModule('i18n/index.ts')

const en = copyFor('en')
const fixture = (name) => JSON.parse(readFileSync(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const recent = new Date(Date.now() - 86_400_000).toISOString()

const ROWS = [
  { id: 'se-m1', kind: 'milestone', title: 'v2.0', status: 'todo' },
  { id: 'se-e1', kind: 'epic', title: 'E3 views', milestone: 'se-m1', status: 'todo' },
  { id: 'se-e1.a', kind: 'issue', title: 'List', epic: 'se-e1', status: 'todo', priority: 1, created_at: '2026-01-02T00:00:00Z', assignee: 'raven' },
  { id: 'se-e1.a.x', kind: 'issue', title: 'Rows', epic: 'se-e1', status: 'done', priority: 2, status_since: recent },
  { id: 'se-e1.b', kind: 'issue', title: 'Board', epic: 'se-e1', status: 'in_progress', priority: 0, created_at: '2026-03-02T00:00:00Z', tags: ['ui'] },
  { id: 'se-e1.c', kind: 'issue', title: 'Archived', epic: 'se-e1', status: 'done', priority: 2, archived: true },
  { id: 'se-e1.d', kind: 'issue', title: 'Later', epic: 'se-e1', status: 'todo', priority: 1, deferred_at: recent, shelved_by: 'se-e1.d' },
  { id: 'se-k1', kind: 'backlog', title: 'An idea', status: 'todo' },
]
const { treeOf } = await import('./moai-tree-helper.mjs')
const build = (rows, tree = treeOf(rows)) => new IssueIndex(rows, tree)
const index = build(ROWS)
const at = (id) => index.find(id)
const EPIC = ['m:se-m1', 'e:se-e1']
const view = (over = {}) => ({ ...initialState().view, ...over })
const keys = (lines) => lines.map((line) => line.key)
// An order as data/query.ts writes it (the store keeps that `Sort`).
const by = (...fields) => ({ fields: fields.map((field) => { const [key, dir] = field.split(':'); return { key, dir: dir ?? (key === 'created' || key === 'updated' ? 'desc' : 'asc') } }), reversed: false })

test('view toggles: deferred hidden by default, archived rows (moai\'s `archived`) hidden unless shown', () => {
  const keep = model.viewKeep(view(), index)
  assert.equal(keep(at('se-e1.d')), false, 'deferred')
  assert.equal(keep(at('se-e1.c')), false, 'archived')
  assert.equal(keep(at('se-e1.a.x')), true, 'recently done')
  assert.equal(model.viewKeep(view({ hideDeferred: false }), index)(at('se-e1.d')), true)
  assert.equal(model.viewKeep(view({ showArchived: true }), index)(at('se-e1.c')), true)
  assert.equal(model.viewKeep(view({ hiddenStatuses: ['done'] }), index)(at('se-e1.a.x')), false)
  assert.equal(model.viewKeep(view({ hideBacklog: true }), index)(at('se-k1')), false)
  assert.equal(model.hiddenUnder(index, EPIC, (n) => !keep(n)), 2)
  const open = model.viewKeep(view({ hideDeferred: false, showArchived: true }), index)
  assert.equal(model.hiddenUnder(index, EPIC, (n) => !open(n)), 0)
  // Why the view hides a row: the reasons the toolbar's hidden note counts.
  const why = model.viewHides(view({ hiddenStatuses: ['in_progress'], hideBacklog: true }), index)
  assert.deepEqual(['se-e1.b', 'se-e1.d', 'se-e1.c', 'se-k1', 'se-e1.a'].map((id) => why(at(id))), ['column', 'deferred', 'archived', 'backlog', undefined])
  // Show all lifts what it can; archived rows stay as they are.
  assert.deepEqual({ ...view({ hiddenStatuses: ['done'], hideBacklog: true }), ...model.SHOW_ALL }, view({ hideDeferred: false }))
  assert.equal(model.hidesAnything(view({ hideDeferred: false })), false)
  assert.equal(model.hidesAnything(view()), true)
})

test('order: the list draws the rows in the order moai\'s tree gave, whatever the rows\' own fields say', () => {
  const rows = [
    { id: 'a', kind: 'issue', title: 'alpha', status: 'todo', priority: 3 },
    { id: 'b', kind: 'issue', title: 'beta', status: 'todo', priority: 0 },
    { id: 'c', kind: 'issue', title: 'gamma', status: 'todo', priority: 1 },
  ]
  const lines = (tree) => model.listLines({ path: [], expanded: [] }, build(rows, tree), () => true).map((line) => line.key)
  const tree = treeOf(rows)
  assert.deepEqual(lines(tree), ['a', 'b', 'c'])
  assert.deepEqual(lines([...tree].reverse()), ['c', 'b', 'a'], 'the tree\'s order, not priority')
  assert.deepEqual(lines([tree[1], tree[2], tree[0]]), ['b', 'c', 'a'])
  // A row the tree does not hold stands at the root after the ones it does.
  assert.deepEqual(lines([tree[2]]), ['c', 'a', 'b'])
})

test('order: a real tree file is drawn as printed, sorted or not (tree.json, tree-updated.json)', () => {
  const rows = [...fixture('rows.json'), ...fixture('backlog.json').map((row) => ({ ...row, kind: 'backlog' }))]
  const draw = (name, path = []) => {
    const idx = new IssueIndex(rows, fixture(name))
    return model.listLines({ path, expanded: [] }, idx, () => true).map((line) => line.key)
  }
  const tree = fixture('tree.json')
  assert.deepEqual(draw('tree.json'), ['fx-ms01', NO_MILESTONE, LOST])
  const printed = tree.filter((line) => line.home.join('/') === 'none' && line.id).map((line) => line.id)
  assert.deepEqual(draw('tree.json', [NO_MILESTONE]).filter((key) => key !== '..'), printed)
  const updated = fixture('tree-updated.json').filter((line) => line.home.join('/') === 'none' && line.id).map((line) => line.id)
  assert.deepEqual(draw('tree-updated.json', [NO_MILESTONE]).filter((key) => key !== '..'), updated)
})

test('lines: .. below the root, folders first, open folders unfolded in place with their branches', () => {
  const keep = model.viewKeep(view(), index)
  const root = model.listLines({ path: [], expanded: [] }, index, keep)
  assert.deepEqual(keys(root), ['se-m1', NO_MILESTONE])
  assert.equal(root[1].kind, 'bucket')
  const inside = model.listLines({ path: EPIC, expanded: [] }, index, keep)
  assert.deepEqual(keys(inside), ['..', 'se-e1.a', 'se-e1.b'])
  assert.equal(inside[1].dir, true)
  assert.deepEqual(inside[0].path, ['m:se-m1'])
  const open = model.listLines({ path: EPIC, expanded: [pathKey([...EPIC, 'i:se-e1.a'])] }, index, keep)
  assert.deepEqual(keys(open), ['..', 'se-e1.a', 'se-e1.a.x', 'se-e1.b'])
  assert.equal(open[2].depth, 2)
  assert.equal(model.twigText(open[2].twig), '└─')
  assert.equal(model.twigText(open[1].twig), '')
  assert.equal(model.twigText([true, true, false]), '│ └─')
  // Each level is drawn as lines in the row's full height (se-gowr.4x9); the parts name which lines.
  assert.deepEqual(model.twigParts([true, true, false, true]), ['pipe', 'blank', 'tee'])
  assert.deepEqual(model.twigParts([true]), [])
  assert.equal(model.parentLine(open, 2), 1)
  assert.equal(model.parentLine(open, 3), -1)
  assert.deepEqual(model.folderOf(open[1]), { path: [...EPIC, 'i:se-e1.a'], open: true })
  assert.equal(model.folderOf(open[3]), null)
  assert.equal(model.firstKey(inside), 'se-e1.a')
  assert.equal(model.segKey('e:se-e1'), 'se-e1')
  assert.equal(model.segKey(NO_MILESTONE), NO_MILESTONE)
  assert.equal(model.moveKey(inside, null, 1), '..')
  assert.equal(model.moveKey(inside, null, -1), 'se-e1.b')
  assert.equal(model.moveKey(inside, 'se-e1.a', 5), 'se-e1.b')
  assert.equal(model.moveKey(inside, 'se-e1.a', -5), '..')
})

test('columns: the ones turned on that fit, dropping dates, epic, assignee, tags as the list narrows', () => {
  const all = { ...initialState().columns, tags: true, epic: true, assignee: true, created: true, updated: true }
  assert.deepEqual(model.columnSet(all, 800).right, ['tags', 'epic', 'assignee', 'created', 'updated'])
  assert.deepEqual(model.columnSet(all, 500).right, ['tags', 'epic', 'assignee'])
  assert.deepEqual(model.columnSet(all, 420).right, ['tags', 'assignee'])
  assert.deepEqual(model.columnSet(all, 300).right, [])
  assert.deepEqual(model.columnSet(initialState().columns, 800), { id: true, priority: true, counts: true, names: true, right: [] })
  const now = new Date('2026-10-14T12:00:00')
  assert.equal(model.shortDate('2026-03-05T12:00:00', now), '03-05')
  assert.equal(model.shortDate('2025-03-05T12:00:00', now), '2025')
  assert.equal(model.shortDate(undefined, now), '')
  assert.deepEqual(model.statusGlyph('review'), { glyph: '?', tone: 'review' })
  assert.deepEqual(model.statusGlyph('blocked'), { glyph: '○', tone: 'other' })
  // With moai's boot answer the glyph is moai's: configured ones as given, an unnamed column the unknown glyph.
  const boot = { glyphs: { todo: 'T', blocked: 'B' }, unknown_glyph: 'U' }
  assert.deepEqual(model.statusGlyph('todo', boot), { glyph: 'T', tone: 'todo' })
  assert.deepEqual(model.statusGlyph('blocked', boot), { glyph: 'B', tone: 'other' })
  assert.deepEqual(model.statusGlyph('review', boot), { glyph: 'U', tone: 'review' })
})

test('windowing: only the rows near the viewport, and the offset that reveals a row', () => {
  assert.deepEqual(model.windowRange({ scrollTop: 0, viewport: 280, offset: 24, rowHeight: 28, count: 5000, overscan: 2 }), { start: 0, end: 12 })
  assert.deepEqual(model.windowRange({ scrollTop: 28 * 100 + 24, viewport: 280, offset: 24, rowHeight: 28, count: 5000, overscan: 2 }), { start: 98, end: 112 })
  assert.deepEqual(model.windowRange({ scrollTop: 1e9, viewport: 280, offset: 0, rowHeight: 28, count: 50 }), { start: 50, end: 50 })
  assert.deepEqual(model.windowRange({ scrollTop: 0, viewport: 280, offset: 0, rowHeight: 28, count: 3 }), { start: 0, end: 3 })
  assert.equal(model.revealOffset(0, 280, 28 * 3, 28), 0, 'already in view')
  assert.equal(model.revealOffset(0, 280, 28 * 20, 28), 28 * 21 - 280, 'below: scroll down to its bottom')
  assert.equal(model.revealOffset(1000, 280, 500, 28, 24), 476, 'above: below the sticky head')
})

// ── the view in jsdom ──────────────────────────────────────────────────

function mount(rows, { state: over = {}, width = 800, height = 280, detailVisible = true, resizeObserver } = {}) {
  const dom = new JSDOM('<!doctype html><html lang="en"><head></head><body><div id="pane"></div></body></html>', { pretendToBeVisual: true })
  // jsdom has no ResizeObserver; a test that wants one brings its own.
  if (resizeObserver) dom.window.ResizeObserver = resizeObserver
  const doc = dom.window.document
  const root = doc.getElementById('pane')
  const size = { height }
  Object.defineProperty(root, 'clientHeight', { configurable: true, get: () => size.height })
  const store = createStore({ ...initialState(), ...over })
  const idx = build(rows)
  const calls = []
  const intents = {
    enter: (path, cursor) => store.dispatch(cursor === undefined ? { type: 'enter', path } : { type: 'enter', path, cursor }),
    choose: (id) => {
      calls.push(['choose', id])
      store.dispatch({ type: 'cursor', id })
    },
    point: (id) => store.dispatch({ type: 'cursor', id }),
    closeDetail: () => {},
    setDetailVisible: () => {},
    toggleExpanded: (path) => store.dispatch({ type: 'toggle-expanded', path }),
    refresh: () => {},
    dispatch: (action) => store.dispatch(action),
  }
  const ctx = {
    document: doc, copy: en, language: 'en', intents,
    layout: () => ({ width, mainWidth: width, position: 'bottom', detailVisible }),
    focusPane: (area) => calls.push(['focus', area]),
  }
  const pane = mountList(root, ctx)
  const render = () => pane.render(store.getState(), idx)
  store.subscribe(render)
  render()
  const lines = () => [...root.querySelectorAll('.moai-line')]
  // The key router owns the keys (keys/wiring.ts) and hands the list the action a key stands for.
  const ACTIONS = { ArrowDown: 'move.down', ArrowUp: 'move.up', PageDown: 'move.pageDown', PageUp: 'move.pageUp', Home: 'move.top', End: 'move.bottom', Enter: 'enter', Backspace: 'leave', ArrowRight: 'expand', ArrowLeft: 'collapse', Tab: 'expandAll' }
  const key = (name) => pane.command(ACTIONS[name])
  return { dom, doc, root, store, pane, calls, lines, key, render, size }
}

test('list view: rows, columns, the name line, and going in and out', () => {
  const view = mount(ROWS, { state: { path: ['m:se-m1'] } })
  const { root, store, lines, key, calls } = view
  assert.equal(root.querySelector('.moai-list').getAttribute('role'), 'treegrid')
  assert.deepEqual(lines().map((line) => line.dataset.key), ['..', 'se-e1'])
  const epic = lines()[1]
  assert.equal(epic.querySelector('.m-row-title').textContent, 'E3 views/')
  assert.equal(epic.querySelector('.moai-c-counts').textContent, '2/5', 'every work row counts, hidden ones too')
  assert.equal(epic.getAttribute('aria-expanded'), 'false')
  // The name line sorts by a column: the head click sends a sort action.
  const sorter = root.querySelector('.moai-head-sort[data-sort="title"]')
  sorter.click()
  assert.deepEqual(store.getState().sort, by('title'))
  assert.equal(root.querySelector('[aria-sort]').getAttribute('aria-sort'), 'ascending')
  // A second click on the only field turns it around, as `SPC s t` twice does (query.ts sortEdit).
  root.querySelector('.moai-head-sort[data-sort="title"]').click()
  assert.deepEqual(store.getState().sort, by('title:desc'))
  assert.equal(root.querySelector('[aria-sort]').getAttribute('aria-sort'), 'descending')
  // The column head sorts by `status`, the one name the store, query.ts and the key table share.
  root.querySelector('.moai-head-sort[data-sort="status"]').click()
  assert.deepEqual(store.getState().sort, by('status'))

  // A double click goes in; the cursor lands on the first line after `..`.
  epic.dispatchEvent(new view.dom.window.MouseEvent('dblclick', { bubbles: true }))
  assert.deepEqual(store.getState().path, EPIC)
  assert.equal(store.getState().cursor, 'se-e1.a')
  // The order is the tree's: se-e1.a (List, with a child) stands before Board whatever the sort editor says.
  assert.deepEqual(lines().map((line) => line.dataset.key), ['..', 'se-e1.a', 'se-e1.b'])
  store.dispatch({ type: 'sort', sort: by('priority:desc') })
  assert.deepEqual(lines().map((line) => line.dataset.key), ['..', 'se-e1.a', 'se-e1.b'])
  store.dispatch({ type: 'sort', sort: { fields: [], reversed: false } })
  assert.deepEqual(lines().map((line) => line.dataset.key), ['..', 'se-e1.a', 'se-e1.b'])
  const selected = root.querySelector('[aria-selected="true"]')
  assert.equal(selected.dataset.key, 'se-e1.a')
  assert.equal(root.querySelector('.moai-list').getAttribute('aria-activedescendant'), selected.id)
  assert.equal(selected.querySelector('.moai-c-p').textContent, 'p1')
  assert.ok(selected.querySelector('.moai-c-p').classList.contains('m-p1'))

  // → unfolds, ← folds; ↓ moves the cursor into the unfolded child.
  key('ArrowRight')
  assert.deepEqual(lines().map((line) => line.dataset.key), ['..', 'se-e1.a', 'se-e1.a.x', 'se-e1.b'])
  assert.equal(lines()[2].querySelector('.moai-twig').textContent, '└─')
  assert.deepEqual([...lines()[2].querySelectorAll('.moai-twig .moai-tw')].map((part) => part.dataset.tw), ['end'])
  key('ArrowDown')
  assert.equal(store.getState().cursor, 'se-e1.a.x')
  key('ArrowLeft')
  assert.equal(store.getState().cursor, 'se-e1.a', '← on a child folds its folder and lands on it')
  assert.deepEqual(store.getState().expanded, [])
  // Nothing to fold on a top-level line: ← leaves the place, landing on the folder it came out of (the TUI's `h`).
  key('ArrowLeft')
  assert.deepEqual(store.getState().path, ['m:se-m1'])
  assert.equal(store.getState().cursor, 'se-e1')
  key('ArrowLeft')
  assert.deepEqual(store.getState().path, [])
  assert.equal(store.getState().cursor, 'se-m1')
  key('ArrowLeft')
  assert.deepEqual(store.getState().path, [], 'at the root there is nowhere to go')
  store.dispatch({ type: 'enter', path: EPIC, cursor: 'se-e1.a' })
  // The ▸ button unfolds too, without choosing the row.
  lines()[1].querySelector('.moai-toggle').click()
  assert.equal(store.getState().expanded.length, 1)
  assert.equal(calls.filter(([kind]) => kind === 'choose').length, 0)

  // Enter on a leaf shows it and moves the focus to the detail.
  store.dispatch({ type: 'cursor', id: 'se-e1.b' })
  key('Enter')
  assert.deepEqual(calls.slice(-2), [['choose', 'se-e1.b'], ['focus', 'detail']])
  // A click chooses a row.
  lines().find((line) => line.dataset.key === 'se-e1.a').click()
  assert.deepEqual(calls.at(-1), ['choose', 'se-e1.a'])
  // Backspace leaves, with the cursor on the folder it came out of.
  key('Backspace')
  assert.deepEqual(store.getState().path, ['m:se-m1'])
  assert.equal(store.getState().cursor, 'se-e1')
  // `..` clicked goes up as well.
  lines()[0].click()
  assert.deepEqual(store.getState().path, [])
  assert.equal(store.getState().cursor, 'se-m1')
  assert.equal(root.querySelector('.moai-line .m-row-title').textContent, 'v2.0/')
})

test('list view: right-hand columns follow the toggles and the width', () => {
  const columns = { ...initialState().columns, tags: true, assignee: true, created: true }
  const wide = mount(ROWS, { state: { path: EPIC, columns }, width: 800 })
  const board = wide.lines().find((line) => line.dataset.key === 'se-e1.b')
  assert.equal(board.querySelector('.m-col-tags').textContent, '#ui')
  assert.ok(board.querySelector('.m-col-date'))
  assert.ok(wide.root.querySelector('.moai-list-head .m-col-date'))
  const narrow = mount(ROWS, { state: { path: EPIC, columns: { ...columns, names: false, id: false } }, width: 380 })
  const row = narrow.lines().find((line) => line.dataset.key === 'se-e1.b')
  assert.equal(row.querySelector('.m-col-date'), null)
  assert.equal(row.querySelector('.m-col-assignee'), null)
  assert.ok(row.querySelector('.m-col-tags'))
  assert.equal(row.querySelector('.moai-c-id'), null)
  assert.equal(narrow.root.querySelector('.moai-list-head').hidden, true)
})

test('list view: the hidden note counts what the view hides, and show all lifts it', () => {
  const { root, store, lines } = mount(ROWS, { state: { path: EPIC } })
  const note = root.querySelector('.moai-list-note')
  assert.equal(note.hidden, false)
  assert.equal(note.textContent.startsWith('2 hidden'), true)
  assert.ok(!lines().some((line) => line.dataset.key === 'se-e1.d'))
  note.querySelector('button').click()
  assert.equal(store.getState().view.hideDeferred, false)
  assert.ok(lines().some((line) => line.dataset.key === 'se-e1.d'))
  // The archived row stays hidden: show all leaves the archive toggle alone.
  assert.equal(note.textContent, '1 hidden' + note.querySelector('button').textContent)
  assert.equal(note.querySelector('button').hidden, true)
  store.dispatch({ type: 'view', view: { showArchived: true } })
  assert.equal(note.hidden, true)
  store.dispatch({ type: 'view', view: { hiddenStatuses: ['done', 'todo', 'in_progress'] } })
  assert.equal(root.querySelector('.moai-list-empty').hidden, false)
  assert.equal(root.querySelector('.moai-list-head').hidden, true)
})

test('list view: no index yet shows loading, then the rows', () => {
  const dom = mount(ROWS)
  dom.pane.render({ ...initialState(), board: { value: null, loading: true, problem: null } }, null)
  assert.equal(dom.lines().length, 0)
  assert.match(dom.root.querySelector('.moai-list-empty').textContent, /Loading/)
  dom.render()
  assert.equal(dom.root.querySelector('.moai-list-empty').hidden, true)
  assert.equal(dom.lines().length, 2)
})

test('virtual scroll: 5000 rows draw only a window, follow the scroll, and keep the cursor in view', () => {
  const rows = Array.from({ length: 5000 }, (_, n) => ({ id: `t-${String(n).padStart(4, '0')}`, title: `Task ${n}`, status: 'todo', priority: 2 }))
  const started = performance.now()
  const view = mount(rows, { height: 280 })
  const { root, store, lines, key } = view
  const drawn = lines().length
  assert.ok(drawn > 0 && drawn < 40, `only a window is in the DOM (${drawn})`)
  assert.equal(root.querySelector('.moai-list-body').style.height, `${5000 * ROW_HEIGHT}px`)
  assert.equal(lines()[0].dataset.key, 't-0000')

  root.scrollTop = 2000 * ROW_HEIGHT
  root.dispatchEvent(new view.dom.window.Event('scroll'))
  const keysNow = lines().map((line) => line.dataset.key)
  assert.ok(keysNow.includes('t-2000') && keysNow.length < 40, 'the window followed the scroll')
  assert.equal(lines().find((line) => line.dataset.key === 't-2000').style.top, `${2000 * ROW_HEIGHT}px`)

  // Moving the cursor far away brings it into view (below the sticky name line).
  store.dispatch({ type: 'cursor', id: 't-4500' })
  const want = 24 + 4500 * ROW_HEIGHT + ROW_HEIGHT - 280
  assert.equal(root.scrollTop, want)
  assert.ok(lines().some((line) => line.dataset.key === 't-4500' && line.getAttribute('aria-selected') === 'true'))
  key('End')
  assert.equal(store.getState().cursor, 't-4999')
  assert.ok(lines().some((line) => line.dataset.key === 't-4999'))
  key('Home')
  assert.equal(root.scrollTop, 0)
  // A wheel scroll leaves the cursor; redraws do not pull the view back to it.
  root.scrollTop = 1000 * ROW_HEIGHT
  root.dispatchEvent(new view.dom.window.Event('scroll'))
  store.dispatch({ type: 'view', view: { hideBacklog: true } })
  assert.equal(root.scrollTop, 1000 * ROW_HEIGHT)
  // The focus stays on the list itself while the window is redrawn.
  view.pane.focus()
  assert.equal(view.doc.activeElement, root.querySelector('.moai-list'))
  key('ArrowDown')
  assert.equal(view.doc.activeElement, root.querySelector('.moai-list'))
  assert.ok(performance.now() - started < 3000, 'mounting and scrolling 5000 rows stays quick')
})

test('list view: the name line names its status head by the cell it is, whatever columns are on', () => {
  const head = (columns) => {
    const { root } = mount(ROWS, { state: { path: EPIC, columns: { ...initialState().columns, ...columns } } })
    return [...root.querySelectorAll('.moai-list-head > [role="columnheader"]')]
  }
  const labelled = (cells) => cells.filter((cell) => cell.getAttribute('aria-label') === 'Status')
  for (const columns of [{}, { id: false }, { priority: false }, { id: false, priority: false }, { counts: false }]) {
    const cells = head(columns)
    assert.equal(labelled(cells).length, 1, JSON.stringify(columns))
    assert.ok(labelled(cells)[0].classList.contains('moai-c-st'), JSON.stringify(columns))
  }
  // Nothing but the status and the title: the list still draws (this threw before).
  const bare = head({ id: false, priority: false, counts: false })
  assert.equal(bare.length, 3)
})

test('list view: the focus stays in the list when a button of it is destroyed by the redraw its click causes', () => {
  const view = mount(ROWS, { state: { path: EPIC } })
  const { root, doc } = view
  const list = root.querySelector('.moai-list')
  // The ▸ toggle (the mouse focuses a button on press; tabindex -1 only keeps it out of the Tab order).
  const toggle = root.querySelector('.moai-toggle')
  toggle.focus()
  toggle.click()
  assert.equal(doc.activeElement, list)
  // A name-line sort button.
  const sorter = root.querySelector('.moai-head-sort[data-sort="title"]')
  sorter.focus()
  sorter.click()
  assert.equal(doc.activeElement, list)
  // Show all: the note hides it once the toggles that it lifts are lifted.
  const showAll = root.querySelector('.moai-list-note button')
  showAll.focus()
  showAll.click()
  assert.equal(doc.activeElement, list)
  // The arrows still work from there.
  const before = view.store.getState().cursor
  view.key('ArrowDown')
  assert.notEqual(view.store.getState().cursor, before)
})

test('list view: a pane that only gets taller draws the rows its new height shows', () => {
  let notify = () => {}
  const disconnected = []
  class FakeObserver {
    constructor(callback) {
      notify = callback
    }
    observe() {}
    disconnect() {
      disconnected.push(true)
    }
  }
  const rows = Array.from({ length: 300 }, (_, n) => ({ id: `t-${String(n).padStart(3, '0')}`, title: `Task ${n}`, status: 'todo', priority: 2 }))
  const view = mount(rows, { height: 280, resizeObserver: FakeObserver })
  const before = view.lines().length
  view.size.height = 1400
  notify()
  assert.ok(view.lines().length > before + 20, `drew ${view.lines().length} lines after the pane grew from ${before}`)
  view.pane.destroy()
  assert.equal(disconnected.length, 1)
})

test('narrow: a filter or a search takes moai\'s ids; none yet means every row passes; a search reveals what the view hides', () => {
  const rows = [
    { id: 'a', kind: 'issue', title: 'Alpha', status: 'todo' },
    { id: 'b', kind: 'issue', title: 'Beta', status: 'todo', shelved_by: 'b' },
    { id: 'c', kind: 'issue', title: 'Gamma', status: 'done', archived: true },
  ]
  const idx = build(rows)
  const matched = (ids) => ({ key: 'k', ids: ids === null ? null : new Set(ids), loading: false, problem: null })
  const base = { ...initialState(), matched: matched(null) }
  const drawn = (state) => {
    const kept = model.narrow(state, idx)
    return rows.filter((row) => kept.keep(idx.find(row.id))).map((row) => row.id)
  }
  // Nothing asked: the view alone (deferred and archived rows go).
  assert.deepEqual(drawn(base), ['a'])
  // A filter on whose answer is {a}: only a. An answer not yet in (null ids): every row passes the question.
  const filtered = { ...base, filter: { text: 'status=todo', applied: 'status=todo' } }
  assert.deepEqual(drawn({ ...filtered, matched: matched(['a']) }), ['a'])
  assert.deepEqual(drawn({ ...filtered, matched: matched(['b']) }), [], 'the view still hides a deferred row a filter found')
  assert.deepEqual(drawn({ ...filtered, matched: matched(null) }), ['a'], 'null ids: the filter keeps everything the view shows')
  const kept = model.narrow({ ...filtered, view: { ...filtered.view, hideDeferred: false }, matched: matched(['a', 'b']) }, idx)
  assert.deepEqual([kept.match(0), kept.match(1), kept.match(2)], [true, true, false])
  // A search lifts the view's hiding for what moai found, drawn dim as revealed; the hidden note does not count it.
  const searching = { ...base, search: { text: 'be', open: true }, matched: matched(['b', 'c']) }
  const found = model.narrow(searching, idx)
  assert.deepEqual(drawn(searching), ['b', 'c'])
  assert.deepEqual([found.revealed(1), found.revealed(2), found.revealed(0)], [true, true, false])
  assert.deepEqual([found.hidden(1), found.hidden(2)], [false, false])
  assert.deepEqual([found.hiddenBy(idx.rows[1]), found.hiddenBy(idx.rows[2]), found.hiddenBy(idx.rows[0])], ['deferred', 'archived', undefined])
  // With a filter alone the view's hiding stands, and the hidden rows are what the note counts.
  const hiddenFilter = model.narrow({ ...filtered, matched: matched(['a', 'b', 'c']) }, idx)
  assert.deepEqual([hiddenFilter.hidden(1), hiddenFilter.hidden(2), hiddenFilter.hidden(0)], [true, true, false])
})

test('list view: NEW before the name on unread rows only; archived rows and a source that says nothing show none (se-ehdi.zcx)', () => {
  const rows = ROWS.map((row) => (row.id === 'se-e1.b' ? { ...row, unread: true } : row.id === 'se-e1.c' ? { ...row, unread: true, archived: true } : row))
  const { lines } = mount(rows, { state: { path: EPIC, view: { ...initialState().view, showArchived: true } } })
  const chip = (id) => lines().find((line) => line.dataset.key === id).querySelector('.m-chip-new')
  assert.equal(chip('se-e1.b').textContent, 'NEW')
  assert.match(lines().find((line) => line.dataset.key === 'se-e1.b').querySelector('.m-row-title').textContent, /^NEW Board$/)
  assert.equal(chip('se-e1.a'), null, 'a read row')
  assert.equal(chip('se-e1.c'), null, 'an archive row is never NEW (moai read cannot clear it)')
  const none = mount(ROWS, { state: { path: EPIC } })
  assert.equal(none.root.querySelector('.m-chip-new'), null, 'no row carries unread: NEW is off')
})

test('list view: NEW stands after the branch and before the name (design 4.1)', () => {
  const rows = ROWS.map((row) => (row.id === 'se-e1.a.x' ? { ...row, unread: true } : row))
  const { lines } = mount(rows, { state: { path: EPIC, expanded: [pathKey([...EPIC, 'i:se-e1.a'])] } })
  const title = lines().find((line) => line.dataset.key === 'se-e1.a.x').querySelector('.m-row-title')
  const [twig, chip] = title.children
  assert.ok(twig.classList.contains('moai-twig'), 'the row hangs under its folder: it has a branch')
  assert.ok(chip.classList.contains('m-chip-new'))
  assert.equal(title.textContent, '└─NEW Rows')
})

test('list view: rows are 24 high, as list.css --moai-row-h, so the branches meet (se-gowr.4x9)', () => {
  assert.equal(ROW_HEIGHT, 24)
  const listCss = readFileSync(new URL('../panel/style/list.css', import.meta.url), 'utf8')
  assert.match(listCss, /--moai-row-h: 24px/)
})

test('list rows expose their status for progress and review backgrounds', () => {
  const panel = mount([
    { id: 'a', title: 'Progress', status: 'in_progress' },
    { id: 'b', title: 'Review', status: 'review' },
    { id: 'c', title: 'Todo', status: 'todo' },
  ])
  for (const [id, status] of [['a','in_progress'], ['b','review'], ['c','todo']]) {
    assert.equal(panel.root.querySelector(`[data-id="${id}"]`).dataset.status, status)
  }
  // jsdom computes no colour, so the sheets are read: the two tints stand, and the cursor and hover still win over them.
  const listCss = readFileSync(new URL('../panel/style/list.css', import.meta.url), 'utf8')
  const tokensCss = readFileSync(new URL('../panel/style/tokens.css', import.meta.url), 'utf8')
  for (const [status, token] of [['in_progress', '--progress-soft'], ['review', '--review-soft']]) {
    assert.match(listCss, new RegExp(`\\.m-row\\[data-status="${status}"\\]:not\\(\\[aria-selected="true"\\]\\):not\\(:hover\\) \\{ background: var\\(${token}\\); \\}`))
    assert.match(tokensCss, new RegExp(`${token}: color-mix\\(`))
  }
})
