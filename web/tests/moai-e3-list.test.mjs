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
const { IssueIndex, NO_MILESTONE, pathKey } = await panelModule('data/index.ts')
const { copyFor } = await panelModule('i18n/index.ts')

const en = copyFor('en')
const recent = new Date(Date.now() - 86_400_000).toISOString()
const old = new Date(Date.now() - 40 * 86_400_000).toISOString()

const ROWS = [
  { id: 'se-m1', kind: 'milestone', title: 'v2.0', status: 'todo' },
  { id: 'se-e1', kind: 'epic', title: 'E3 views', milestone: 'se-m1', status: 'todo' },
  { id: 'se-e1.a', kind: 'issue', title: 'List', epic: 'se-e1', status: 'todo', priority: 1, created_at: '2026-01-02T00:00:00Z', assignee: 'raven' },
  { id: 'se-e1.a.x', kind: 'issue', title: 'Rows', epic: 'se-e1', status: 'done', priority: 2, status_since: recent },
  { id: 'se-e1.b', kind: 'issue', title: 'Board', epic: 'se-e1', status: 'in_progress', priority: 0, created_at: '2026-03-02T00:00:00Z', tags: ['ui'] },
  { id: 'se-e1.c', kind: 'issue', title: 'Archived', epic: 'se-e1', status: 'done', priority: 2, status_since: old },
  { id: 'se-e1.d', kind: 'issue', title: 'Later', epic: 'se-e1', status: 'todo', priority: 1, deferred_at: recent },
  { id: 'se-k1', kind: 'backlog', title: 'An idea', status: 'todo' },
]
const index = new IssueIndex(ROWS)
const at = (id) => index.find(id)
const EPIC = ['m:se-m1', 'e:se-e1']
const view = (over = {}) => ({ ...initialState().view, ...over })
const keys = (lines) => lines.map((line) => line.key)
// An order as data/query.ts writes it (the store keeps that `Sort`).
const by = (...fields) => ({ fields: fields.map((field) => { const [key, dir] = field.split(':'); return { key, dir: dir ?? (key === 'created' || key === 'updated' ? 'desc' : 'asc') } }), reversed: false })
const order = model.sortOrder(by('priority'), index)

test('view toggles: deferred hidden by default, done rows past the archive age hidden unless shown', () => {
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

test('archive: a row counts from the day it got to done, a group from the work under it, and 0 days is off', () => {
  const now = new Date('2026-10-14T12:00:00Z')
  const day = (n) => new Date(now.getTime() - n * 86_400_000).toISOString()
  const rows = [
    // A finished epic: its own `status` was never moved to done, but its members are, 30 and 20 days ago.
    { id: 'e-fin', kind: 'epic', title: 'Finished', status: 'todo', derived_status: 'done', status_since: day(60), created_at: day(60) },
    { id: 'e-fin.a', kind: 'issue', title: 'a', epic: 'e-fin', status: 'done', status_since: day(30) },
    { id: 'e-fin.b', kind: 'issue', title: 'b', epic: 'e-fin', status: 'done', status_since: day(20) },
    // An epic somebody set to done by hand while a member is still open: it stands in the column its members give.
    { id: 'e-hand', kind: 'epic', title: 'By hand', status: 'done', derived_status: 'in_progress', status_since: day(60), created_at: day(60) },
    { id: 'e-hand.a', kind: 'issue', title: 'a', epic: 'e-hand', status: 'in_progress', status_since: day(1) },
    // A group whose members finished two days ago is not old whatever its own stamp says.
    { id: 'e-new', kind: 'epic', title: 'New', status: 'todo', derived_status: 'done', status_since: day(90), created_at: day(90) },
    { id: 'e-new.a', kind: 'issue', title: 'a', epic: 'e-new', status: 'done', status_since: day(2) },
    // The boundary: exactly archive_days whole days is archived (the CLI counts days that have come, `>=`).
    { id: 'w-edge', kind: 'issue', title: 'edge', status: 'done', status_since: day(14) },
    { id: 'w-fresh', kind: 'issue', title: 'fresh', status: 'done', status_since: day(13) },
  ]
  const idx = new IssueIndex(rows)
  const hidden = (days) => {
    const keep = model.viewKeep(view(), idx, now, days)
    return rows.filter((row) => !keep(idx.find(row.id))).map((row) => row.id)
  }
  assert.deepEqual(hidden(14), ['e-fin', 'e-fin.a', 'e-fin.b', 'w-edge'])
  assert.deepEqual(hidden(0), [], 'archive_days = 0 turns the archive off')
  assert.deepEqual(hidden(25), ['e-fin.a'], 'a member 30 days old; the group counts from its latest member, 20 days')
})

test('sort: field by field as moai orders, then (priority, id) or the id after a date', () => {
  const rows = [
    { id: 'b', priority: 1, created_at: '2026-01-01T00:00:00Z', status: 'done', title: 'beta' },
    { id: 'a', priority: 1, status: 'todo', title: 'alpha', assignee: 'zed' },
    { id: 'c', priority: 0, created_at: '2026-02-01T00:00:00Z', status: 'review', title: 'Gamma', assignee: 'amy' },
    { id: 'd', status: 'weird', title: 'delta' },
  ]
  const sorted = (sort) => [...rows].sort(model.sortCompare(sort)).map((row) => row.id)
  assert.deepEqual(sorted(by('priority')), ['c', 'a', 'b', 'd'])
  assert.deepEqual(sorted(by('created:desc')), ['c', 'b', 'a', 'd'])
  // A stamp that is not there compares as the empty string, as in the CLI: first when ascending.
  assert.deepEqual(sorted(by('created:asc')), ['a', 'd', 'b', 'c'])
  // The column is `status`, one name with data/query.ts and the key table.
  assert.deepEqual(sorted(by('status')), ['a', 'c', 'b', 'd'])
  // An unassigned row goes last when ascending and first when descending (SQL's reversed order).
  assert.deepEqual(sorted(by('assignee:asc')), ['c', 'a', 'b', 'd'])
  assert.deepEqual(sorted(by('assignee:desc')), ['b', 'd', 'a', 'c'])
  // A title compares without regard to case: `Gamma` is not before `alpha`.
  assert.deepEqual(sorted(by('title')), ['a', 'b', 'd', 'c'])
  // The whole list turned around, ties included.
  assert.deepEqual(sorted({ ...by('priority'), reversed: true }), ['d', 'b', 'a', 'c'])
  // Plain fields (no reverse switch) order the same.
  assert.deepEqual(sorted(by('title').fields), ['a', 'b', 'd', 'c'])
})

test('sort: the list orders rows as `moai show --sort` printed them in the CLI fixture', async () => {
  const query = await panelModule('data/query.ts')
  const fixture = JSON.parse(readFileSync(new URL('./fixtures/moai/query-v016.json', import.meta.url), 'utf8'))
  let checked = 0
  for (const item of fixture.cases) {
    if (item.sort === undefined) continue
    const parsed = query.parseSort(item.sort)
    if (!parsed.ok) continue
    // The store keeps query.ts's own order: the parsed fields are the list's sort as they are.
    const sort = { fields: parsed.value, reversed: false }
    const filter = query.parseFilter(item.filter, item.flags)
    assert.ok(filter.ok, item.name)
    const kept = query.applyFilter(fixture.rows, filter.value)
    const ids = [...kept].sort(model.sortCompare(sort)).map((row) => row.id)
    if (item.reverse) ids.reverse()
    assert.deepEqual(ids, item.ids, item.name)
    checked++
  }
  assert.ok(checked >= 8, `only ${checked} sorted CLI cases were checked`)
})

test('lines: .. below the root, folders first, open folders unfolded in place with their branches', () => {
  const keep = model.viewKeep(view(), index)
  const root = model.listLines({ path: [], expanded: [] }, index, keep, order)
  assert.deepEqual(keys(root), ['se-m1', NO_MILESTONE])
  assert.equal(root[1].kind, 'bucket')
  const inside = model.listLines({ path: EPIC, expanded: [] }, index, keep, order)
  assert.deepEqual(keys(inside), ['..', 'se-e1.a', 'se-e1.b'])
  assert.equal(inside[1].dir, true)
  assert.deepEqual(inside[0].path, ['m:se-m1'])
  const open = model.listLines({ path: EPIC, expanded: [pathKey([...EPIC, 'i:se-e1.a'])] }, index, keep, order)
  assert.deepEqual(keys(open), ['..', 'se-e1.a', 'se-e1.a.x', 'se-e1.b'])
  assert.equal(open[2].depth, 2)
  assert.equal(model.twigText(open[2].twig), '└─')
  assert.equal(model.twigText(open[1].twig), '')
  assert.equal(model.twigText([true, true, false]), '│ └─')
  assert.equal(model.parentLine(open, 2), 1)
  assert.equal(model.parentLine(open, 3), -1)
  assert.deepEqual(model.folderOf(open[1]), { path: [...EPIC, 'i:se-e1.a'], open: true })
  assert.equal(model.folderOf(open[3]), null)
  // Sorting by title puts Board before List.
  const byTitle = model.listLines({ path: EPIC, expanded: [] }, index, keep, model.sortOrder(by('title'), index))
  assert.deepEqual(keys(byTitle), ['..', 'se-e1.a', 'se-e1.b'], 'folders still lead')
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
  const idx = new IssueIndex(rows)
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
  // Folders lead whatever the order: se-e1.a (List) has a child, so it stands before Board.
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

test('narrow: a day in the filter is the reader\'s own day, not a UTC one', () => {
  const zone = process.env.TZ
  process.env.TZ = 'Asia/Seoul'
  try {
    // 07:00 on the 14th in Seoul is the 13th in UTC; 01:00 on the 15th in Seoul is the 14th in UTC.
    const rows = [
      { id: 'a1', kind: 'issue', title: 'Morning', status: 'todo', updated_at: '2026-10-13T22:00:00Z' },
      { id: 'a2', kind: 'issue', title: 'Night', status: 'todo', updated_at: '2026-10-14T16:00:00Z' },
    ]
    const idx = new IssueIndex(rows)
    const state = { ...initialState(), filter: { text: 'updated_at=2026-10-14', applied: 'updated_at=2026-10-14' } }
    const kept = model.narrow(state, idx)
    assert.deepEqual(rows.map((row, n) => kept.keep(n) ? row.id : null).filter(Boolean), ['a1'])
  } finally {
    if (zone === undefined) delete process.env.TZ
    else process.env.TZ = zone
  }
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
