// Moai panel board (se-c10x.bym): the grouping into lanes and columns, and the kanban in a jsdom document.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { groupBoard, columnOf, statusKeys, BACKLOG, DEFERRED } = await panelModule('view/board-groups.ts')
const { mountBoard, glyphOf } = await panelModule('view/board.ts')
const { IssueIndex, NO_MILESTONE, pathKey } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const { initialState } = await panelModule('store.ts')
const { copyFor } = await panelModule('i18n/index.ts')

const build = (rows) => new IssueIndex(rows, treeOf(rows))
const ROWS = [
  { id: 'm1', kind: 'milestone', title: 'v1', status: 'todo' },
  { id: 'm2', kind: 'milestone', title: 'v2', status: 'todo' },
  { id: 'e1', kind: 'epic', title: 'Epic one', milestone: 'm1', status: 'todo' },
  { id: 'e2', kind: 'epic', title: 'Epic two', milestone: 'm2', status: 'todo' },
  { id: 'e1.a', kind: 'issue', title: 'A', epic: 'e1', derived_epic: 'e1', status: 'todo', priority: 2 },
  { id: 'e1.b', kind: 'issue', title: 'B', epic: 'e1', derived_epic: 'e1', status: 'in_progress', priority: 0, assignee: 'kim' },
  { id: 'e1.c', kind: 'issue', title: 'C', epic: 'e1', derived_epic: 'e1', status: 'done', priority: 1 },
  { id: 'e2.a', kind: 'issue', title: 'D', epic: 'e2', derived_epic: 'e2', status: 'blocked', priority: 1 },
  { id: 'e2.b', kind: 'issue', title: 'E', epic: 'e2', derived_epic: 'e2', status: 'todo', deferred_at: '2026-01-01', shelved_by: 'e2.b' },
  { id: 'b1', kind: 'backlog', title: 'Idea', status: 'todo' },
]
const index = build(ROWS)
const look = (over = {}) => ({
  path: [],
  view: { hiddenStatuses: [], hideDeferred: false, hideBacklog: false, showArchived: false },
  cursor: null,
  ...over,
})
const ids = (model, lane, key) => model.lanes[lane].columns.find((c) => c.key === key).cards.map((at) => index.rows[at].id)

test('statusKeys: configured statuses first, unknown ones appended once', () => {
  assert.deepEqual(statusKeys(ROWS), ['todo', 'in_progress', 'review', 'done', 'blocked'])
  assert.deepEqual(statusKeys(ROWS, ['done', 'todo']), ['done', 'todo', 'in_progress', 'blocked'])
})

test('groupBoard: root has a lane per milestone, cards in priority order, groups are not cards', () => {
  const model = groupBoard(index, look())
  assert.deepEqual(model.keys, [BACKLOG, DEFERRED, 'todo', 'in_progress', 'review', 'done', 'blocked'])
  assert.deepEqual(model.lanes.map((lane) => lane.seg), ['m:m1', 'm:m2', NO_MILESTONE])
  assert.equal(model.lanes[0].at, 0)
  assert.deepEqual(ids(model, 0, 'todo'), ['e1.a'])
  assert.deepEqual(ids(model, 0, 'in_progress'), ['e1.b'])
  assert.deepEqual(ids(model, 1, 'blocked'), ['e2.a'])
  assert.deepEqual(ids(model, 1, DEFERRED), ['e2.b'])
  assert.deepEqual(ids(model, 2, BACKLOG), ['b1'])
  assert.equal(model.total, 6)
  assert.equal(columnOf(index, index.find('e2.b')), DEFERRED)
})

test('groupBoard: hidden statuses, deferred and backlog drop their columns; the cursor card keeps its own', () => {
  const view = { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false }
  const model = groupBoard(index, look({ view }))
  assert.ok(!model.keys.includes('done') && !model.keys.includes(DEFERRED) && !model.keys.includes(BACKLOG))
  assert.deepEqual(model.lanes.map((lane) => lane.seg), ['m:m1', 'm:m2'])
  assert.equal(model.total, 3)
  const kept = groupBoard(index, look({ view, cursor: 'e1.c' }))
  assert.ok(kept.keys.includes('done'))
  assert.deepEqual(ids(kept, 0, 'done'), ['e1.c'])
})

test('groupBoard: archived rows (moai\'s `archived`) are no cards unless shown; cards stand in the tree\'s order', () => {
  const rows = [
    { id: 't1', kind: 'issue', title: 'banana', status: 'todo', priority: 1 },
    { id: 't2', kind: 'issue', title: 'Apple', status: 'todo', priority: 2 },
    { id: 't3', kind: 'issue', title: 'cherry', status: 'todo', priority: 0 },
    { id: 'd1', kind: 'issue', title: 'Old', status: 'done', priority: 2, archived: true },
    { id: 'd2', kind: 'issue', title: 'Fresh', status: 'done', priority: 2 },
  ]
  const idx = build(rows)
  const cards = (model, key) => model.lanes[0].columns.find((c) => c.key === key)?.cards.map((at) => idx.rows[at].id)
  const hidden = groupBoard(idx, look())
  assert.deepEqual(cards(hidden, 'done'), ['d2'], 'an archived row is not a card')
  const shown = groupBoard(idx, look({ view: { ...look().view, showArchived: true } }))
  assert.deepEqual(cards(shown, 'done'), ['d1', 'd2'])
  assert.deepEqual(cards(groupBoard(idx, look({ cursor: 'd1' })), 'done'), ['d1', 'd2'], 'the cursor card is kept')
  // No sort of its own: the cards follow the tree, whatever the priorities say.
  assert.deepEqual(cards(hidden, 'todo'), ['t1', 't2', 't3'])
  const reordered = new IssueIndex(rows, treeOf(rows).reverse())
  const backwards = groupBoard(reordered, look())
  assert.deepEqual(backwards.lanes[0].columns.find((c) => c.key === 'todo').cards.map((at) => reordered.rows[at].id), ['t3', 't2', 't1'])
  // The list's filter and search narrow the cards; a search also lifts the view's hiding (the archive here).
  const lit = new Set(['t1', 'd1'].map((id) => idx.find(id)))
  const narrowed = groupBoard(idx, look({ match: (at) => lit.has(at) }))
  assert.deepEqual([cards(narrowed, 'todo'), cards(narrowed, 'done')], [['t1'], []])
  const found = groupBoard(idx, look({ match: (at) => lit.has(at), reveal: true }))
  assert.deepEqual([cards(found, 'todo'), cards(found, 'done')], [['t1'], ['d1']])
})

test('groupBoard: a thought that is closed is not a card', () => {
  const rows = [
    { id: 'i1', kind: 'backlog', title: 'open idea', status: 'todo' },
    { id: 'i2', kind: 'backlog', title: 'closed idea', status: 'done' },
    { id: 't1', kind: 'issue', title: 'work', status: 'todo' },
  ]
  const idx = build(rows)
  const model = groupBoard(idx, look())
  const cards = model.lanes.flatMap((lane) => lane.columns.flatMap((column) => column.cards.map((at) => idx.rows[at].id)))
  assert.deepEqual(cards.sort(), ['i1', 't1'])
  assert.equal(model.total, 2)
})

test('groupBoard: a repository without milestones has one unnamed lane, not a `(no milestone)` bucket', () => {
  const rows = [
    { id: 't1', kind: 'issue', title: 'A', status: 'todo', priority: 2 },
    { id: 't2', kind: 'issue', title: 'B', status: 'in_progress', priority: 1 },
  ]
  const plain = groupBoard(build(rows), look())
  assert.deepEqual(plain.lanes.map((lane) => lane.seg), [null])
  assert.equal(plain.total, 2)
  // And the view draws no lane head for it (the board's own columns are the only heads).
  const view = mount({ ...initialState(), tab: 'board' }, rows)
  assert.equal(view.root.querySelector('.m-lane-head:not(.m-lane .m-lane-head)'), null)
  assert.equal(view.root.querySelectorAll('.m-lane-head').length, view.root.querySelectorAll('.m-lane').length, 'only column heads')
  // With milestones the bucket is a lane of its own, as before.
  const withMilestone = groupBoard(index, look())
  assert.ok(withMilestone.lanes.some((lane) => lane.seg === NO_MILESTONE))
})

test('groupBoard: inside a place there is one lane holding what is under it', () => {
  const model = groupBoard(index, look({ path: index.pathOf('e1') }))
  assert.equal(model.lanes.length, 1)
  assert.equal(model.lanes[0].seg, null)
  assert.deepEqual(ids(model, 0, 'todo'), ['e1.a'])
  assert.equal(model.total, 3)
  assert.deepEqual(groupBoard(build([]), look()).lanes, [])
})

const mount = (state, rows = ROWS) => {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const chosen = []
  const root = dom.window.document.getElementById('r')
  const ctx = {
    document: dom.window.document, copy: copyFor('en'), language: 'en',
    intents: { choose: (id) => chosen.push(id), point: (id) => chosen.push(`point:${id}`) }, layout: () => ({}), focusPane: () => {},
  }
  const view = mountBoard(root, ctx)
  const index = build(rows)
  view.render(state, index)
  return { root, view, chosen, dom, index }
}

test('closed milestone lanes and epic cards fold; the running lane and deferred member remain', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', title: 'Closed release', status: 'todo', derived_status: 'done' },
    { id: 'e1', kind: 'epic', title: 'Closed epic', milestone: 'm1', status: 'todo', derived_status: 'done' },
    { id: 'e1.a', kind: 'issue', title: 'Done', derived_epic: 'e1', status: 'done' },
    { id: 'e1.b', kind: 'issue', title: 'Deferred', derived_epic: 'e1', status: 'todo', deferred_at: '2026-10-01', shelved_by: 'e1.b' },
    { id: 'm2', kind: 'milestone', title: 'Running release', status: 'todo', derived_status: 'in_progress' },
    { id: 'w1', kind: 'issue', title: 'Running', milestone: 'm2', status: 'in_progress' },
  ]
  const state = { ...initialState(), tab: 'board', view: { ...initialState().view, hideDeferred: false } }
  const { root, view, index } = mount(state, rows)
  const head = (id) => root.querySelector(`.m-swim-head[data-lane="m:${id}"]`)
  assert.equal(head('m1').querySelector('.moai-toggle').getAttribute('aria-expanded'), 'false')
  assert.equal(head('m1').querySelector('.moai-toggle').tabIndex, 0, 'the browser Tab reaches the fold button')
  assert.equal(head('m1').querySelector('.m-swim-title').tabIndex, 0, 'the browser Tab reaches the milestone title')
  assert.equal(head('m1').querySelector('.m-count').textContent, '2')
  assert.equal(root.querySelector('[data-id="e1"]'), null)
  assert.equal(head('m2').querySelector('.moai-toggle').getAttribute('aria-expanded'), 'true')
  assert.ok(root.querySelector('[data-id="w1"]'))
  head('m1').querySelector('.moai-toggle').click()
  assert.equal(head('m1').querySelector('.moai-toggle').getAttribute('aria-expanded'), 'true')
  assert.ok(root.querySelector('.m-lane[data-status="done"] [data-id="e1"]'))
  assert.equal(root.querySelector('[data-id="e1.a"]'), null)
  assert.ok(root.querySelector('.m-lane[data-status="deferred"] [data-id="e1.b"]'))
  root.querySelector('[data-id="e1"] .moai-toggle').click()
  assert.ok(root.querySelector('.m-lane[data-status="done"] [data-id="e1.a"]'))
  view.render(state, index)
  assert.ok(root.querySelector('[data-id="e1.a"]'), 'a reread of the view keeps the chosen fold')
  view.leave()
  view.render(state, index)
  assert.equal(head('m1').querySelector('.moai-toggle').getAttribute('aria-expanded'), 'false', 'a new visit seeds closed groups again')
  const selectedHead = mount({ ...state, cursor: 'm1' }, rows)
  assert.equal(selectedHead.root.querySelector('.m-swim-head[data-lane="m:m1"] .moai-toggle').getAttribute('aria-expanded'), 'false', 'selecting the head does not open its lane')
})

test('lane title chooses its milestone; h and l fold its cards, and j enters the lane', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', title: 'Release', derived_status: 'in_progress' },
    { id: 'w1', kind: 'issue', title: 'Work', milestone: 'm1', status: 'in_progress' },
  ]
  const state = { ...initialState(), tab: 'board', cursor: 'm1' }
  const { root, view, chosen, index } = mount(state, rows)
  const head = () => root.querySelector('.m-swim-head[data-lane="m:m1"]')
  assert.equal(head().getAttribute('aria-selected'), 'true')
  head().querySelector('.m-swim-title').click()
  assert.deepEqual(chosen, ['m1'])
  view.command('collapse')
  assert.equal(head().querySelector('.moai-toggle').getAttribute('aria-expanded'), 'false')
  view.command('expand')
  assert.equal(head().querySelector('.moai-toggle').getAttribute('aria-expanded'), 'true')
  view.render(state, index)
  view.command('move.down')
  assert.equal(chosen.at(-1), 'point:w1')
})

test('folding a lane moves its selected card to the milestone head', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', title: 'Release', derived_status: 'in_progress' },
    { id: 'w1', kind: 'issue', title: 'Work', milestone: 'm1', status: 'in_progress' },
  ]
  const { root, chosen } = mount({ ...initialState(), tab: 'board', cursor: 'w1' }, rows)
  root.querySelector('.m-swim-head[data-lane="m:m1"] .moai-toggle').click()
  assert.equal(root.querySelector('[data-id="w1"]'), null)
  assert.equal(root.querySelector('.m-swim-head[data-lane="m:m1"]').getAttribute('aria-selected'), 'true')
  assert.deepEqual(chosen, ['point:m1'])
})

test('search reveals a done member inside folded lane and epic without changing the fold', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', derived_status: 'done' },
    { id: 'e1', kind: 'epic', milestone: 'm1', derived_status: 'done' },
    { id: 'e1.a', kind: 'issue', derived_epic: 'e1', status: 'done' },
  ]
  const idx = build(rows)
  const shut = new Set(['m:m1', pathKey(idx.dirPath(idx.find('e1')))])
  const normal = groupBoard(idx, look({ shut }))
  assert.equal(normal.lanes[0].folded, true)
  const found = groupBoard(idx, look({ shut, reveal: true, match: (at) => idx.rows[at].id === 'e1.a' }))
  assert.equal(found.lanes[0].folded, false)
  assert.deepEqual(found.lanes[0].columns.find((column) => column.key === 'done').cards.map((at) => idx.rows[at].id), ['e1.a'])
  assert.equal(shut.size, 2)
})

test('board bands follow rendered lanes and the card head, including a selected and deferred card', () => {
  const rows = [
    { id: 'todo', kind: 'issue', title: 'Todo', status: 'todo' },
    { id: 'progress', kind: 'issue', title: 'Progress', status: 'in_progress', priority: 0 },
    { id: 'review', kind: 'issue', title: 'Review', status: 'review' },
    { id: 'done', kind: 'issue', title: 'Done', status: 'done' },
    { id: 'shelved', kind: 'issue', title: 'Shelved', status: 'in_progress', deferred_at: '2026-01-01', shelved_by: 'shelved' },
    { id: 'idea', kind: 'backlog', title: 'Idea', status: 'todo' },
  ]
  const { root } = mount({ ...initialState(), tab: 'board', cursor: 'progress', view: { ...initialState().view, hideDeferred: false } }, rows)
  const css = readFileSync(new URL('../panel/style/board.css', import.meta.url), 'utf8')
  const tokens = readFileSync(new URL('../panel/style/tokens.css', import.meta.url), 'utf8')
  const bandSelector = '.m-card > .m-card-line:first-child'
  assert.match(css, /\.m-card > \.m-card-line:first-child\s*\{[^}]*background: var\(--band, transparent\);[^}]*border-bottom: 1px solid var\(--edge, transparent\)/s)
  assert.match(css, /\.m-card\[aria-selected="true"\] > \.m-card-line:first-child\s*\{[^}]*border-top: 1px solid var\(--accent\);[^}]*border-left: 1px solid var\(--accent\);[^}]*border-right: 1px solid var\(--accent\)/s)
  for (const [status, token] of [['in_progress', 'progress'], ['review', 'review'], ['done', 'done']]) {
    const card = root.querySelector(`.m-lane[data-status="${status}"] .m-card[data-id="${token}"]`)
    assert.ok(card, `${status} card is in its rendered lane`)
    assert.ok(card.querySelector(bandSelector), `${status} card has a first-line band`)
    assert.equal(card.querySelectorAll(bandSelector).length, 1)
    assert.match(css, new RegExp(`\\.m-lane\\[data-status="${status}"\\] \\{ --band: var\\(--${token}-band\\); --edge: var\\(--${token}-edge\\); \\}`))
    assert.match(tokens, new RegExp(`--${token}-band: color-mix\\(in srgb, #[0-9A-Fa-f]+ [0-9]+%, var\\(--card-bg\\)\\)`))
    // A missing edge token leaves the lane's `--edge` invalid, so the hairline falls back to transparent without a word.
    assert.match(tokens, new RegExp(`--${token}-edge: color-mix\\(in srgb, #[0-9A-Fa-f]+ [0-9]+%, var\\(--card-bg\\)\\)`))
  }
  // The dark done band stays pale in both dark blocks (OS dark and data-theme="dark"), as --done-soft does.
  assert.equal(tokens.match(/--done-band: color-mix\(in srgb, #10B981 5%, var\(--card-bg\)\)/g)?.length, 2)
  assert.equal(root.querySelector('[data-id="progress"]').getAttribute('aria-selected'), 'true')
  assert.ok(root.querySelector('.m-lane[data-status="deferred"] [data-id="shelved"]'))
  for (const status of ['todo', 'backlog', 'deferred']) {
    assert.doesNotMatch(css, new RegExp(`\\.m-lane\\[data-status="${status}"\\] \\{ --band:`))
  }
})

test('board view: moai\'s answer to the filter, coming after the filter, draws the board again', () => {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const root = dom.window.document.getElementById('r')
  const view = mountBoard(root, { document: dom.window.document, copy: copyFor('en'), language: 'en', intents: { choose() {}, point() {} }, layout: () => ({}), focusPane() {} })
  const same = build(ROWS)
  const asked = { ...initialState(), tab: 'board', filter: { text: 'priority=0', applied: 'priority=0' } }
  const key = JSON.stringify(['priority=0', '', false])
  view.render({ ...asked, matched: { key, ids: null, loading: true, problem: null } }, same)
  assert.ok(root.querySelector('[data-id="e1.a"]'), 'before the answer every card passes')
  view.render({ ...asked, matched: { key, ids: new Set(['e1.b']), loading: false, problem: null } }, same)
  assert.deepEqual([...root.querySelectorAll('.m-card')].map((card) => card.dataset.id), ['e1.b'])
})

test('board view: columns, lanes, cards, cursor highlight and click', () => {
  const state = { ...initialState(), tab: 'board', cursor: 'e1.b' }
  const { root, chosen } = mount(state)
  const lanes = [...root.querySelectorAll('[data-lane]')].filter((n) => n.classList.contains('m-board'))
  assert.equal(lanes.length, 3)
  const status = root.querySelector('.m-board [data-status="in_progress"]')
  assert.equal(status.getAttribute('role'), 'list')
  assert.match(status.getAttribute('aria-label'), /^In progress, 1 card$/)
  const card = status.querySelector('.m-card')
  assert.equal(card.dataset.id, 'e1.b')
  assert.equal(card.getAttribute('aria-selected'), 'true')
  assert.match(card.textContent, /e1\.b/)
  assert.match(card.textContent, /p0/)
  card.click()
  root.querySelector('[data-id="e1.a"]').click()
  assert.deepEqual(chosen, ['e1.b', 'e1.a'])
  // The deferred row is hidden by default (hideDeferred) and the backlog stands in the first column.
  assert.equal(root.querySelector('[data-id="e2.b"]'), null)
  assert.ok(root.querySelector('[data-status="backlog"] [data-id="b1"]'))
  assert.equal(glyphOf('done'), '✓')
})

test('board view: one card is always the tab stop, Space is left to the key layer, tags that are not a list are not drawn', () => {
  const tabbable = (root) => [...root.querySelectorAll('.m-card')].filter((card) => card.getAttribute('tabindex') === '0').map((card) => card.dataset.id)
  // No cursor (or a cursor that is not a card): the first card in the order drawn can be reached by Tab.
  const none = mount({ ...initialState(), tab: 'board', cursor: null })
  assert.equal(tabbable(none.root).length, 1)
  assert.equal(tabbable(none.root)[0], none.root.querySelector('.m-card').dataset.id)
  assert.deepEqual(tabbable(mount({ ...initialState(), tab: 'board', cursor: 'e1' }).root).length, 1, 'a cursor on a group is not a card')
  assert.deepEqual(tabbable(mount({ ...initialState(), tab: 'board', cursor: 'e1.b' }).root), ['e1.b'])

  // The cards listen for no keys: Space (the SPC leader) and Enter are the key router's, which hands the board
  // the action (command) — Enter shows the cursor's card, h/l go to the next column that has cards.
  const { root, chosen, dom, view } = mount({ ...initialState(), tab: 'board', cursor: 'e1.b' })
  const card = root.querySelector('[data-id="e1.a"]')
  for (const key of [' ', 'Enter']) {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })
    card.dispatchEvent(event)
    assert.equal(event.defaultPrevented, false)
  }
  assert.deepEqual(chosen, [])
  assert.equal(view.command('enter'), true)
  assert.deepEqual(chosen, ['e1.b'])
  view.command('collapse')
  assert.deepEqual(chosen.at(-1), 'point:e1.a', 'h goes to the column on the left, at the same height')
  assert.equal(view.command('read'), false, 'not a board action')

  // A `tags` that is a string or an object (not what the CLI writes) draws no chips and does not throw.
  const odd = ROWS.map((row) => (row.id === 'e1.a' ? { ...row, tags: 'bug' } : row.id === 'e1.b' ? { ...row, tags: { a: 1 } } : row))
  const withTags = mount({ ...initialState(), tab: 'board', columns: { ...initialState().columns, tags: true } }, odd)
  assert.doesNotMatch(withTags.root.querySelector('[data-id="e1.a"]').textContent, /#/)
})

test('board view: assignee chip only when the column is on; empty board says so', () => {
  const base = { ...initialState(), tab: 'board' }
  assert.doesNotMatch(mount(base).root.querySelector('[data-id="e1.b"]').textContent, /kim/)
  const on = { ...base, columns: { ...base.columns, assignee: true } }
  assert.match(mount(on).root.querySelector('[data-id="e1.b"]').textContent, /kim/)
  const empty = mount(base, [])
  assert.match(empty.root.textContent, /Nothing here/)
})

test('board view: a card chip carries its whole words in its title, so the stylesheet may cut it at the card edge (moai-rqec)', () => {
  const base = { ...initialState(), tab: 'board' }
  const on = { ...base, columns: { ...base.columns, assignee: true, epic: true } }
  const chips = [...mount(on).root.querySelectorAll('[data-id="e1.b"] .m-card-line > .m-chip')]
  assert.ok(chips.length >= 2, 'the epic and assignee chips stand')
  for (const chip of chips) assert.equal(chip.title, chip.textContent)
})

const bare = (intents = { choose() {}, point() {} }) => {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const root = dom.window.document.getElementById('r')
  const view = mountBoard(root, { document: dom.window.document, copy: copyFor('en'), language: 'en', intents, layout: () => ({}), focusPane() {} })
  return { root, view }
}

test('board view: a cursor move changes two cards in place; the board is not drawn again (se-mzlx)', () => {
  const { root, view } = bare()
  const idx = build(ROWS)
  const state = { ...initialState(), tab: 'board', cursor: 'e1.a' }
  view.render(state, idx)
  const cardA = root.querySelector('[data-id="e1.a"]')
  const cardB = root.querySelector('[data-id="e1.b"]')
  const wrap = root.querySelector('.m-board-wrap')
  view.render({ ...state, cursor: 'e1.b' }, idx)
  assert.equal(root.querySelector('.m-board-wrap'), wrap, 'the board stayed')
  assert.equal(root.querySelector('[data-id="e1.a"]'), cardA)
  assert.equal(root.querySelector('[data-id="e1.b"]'), cardB)
  assert.equal(cardA.getAttribute('aria-selected'), 'false')
  assert.equal(cardB.getAttribute('aria-selected'), 'true')
  assert.deepEqual([cardA.getAttribute('tabindex'), cardB.getAttribute('tabindex')], ['-1', '0'])
  // An unrelated change (the detail loading) keeps the board; another view or a new index draws it again.
  view.render({ ...state, cursor: 'e1.b', shown: { value: null, loading: true, problem: null } }, idx)
  assert.equal(root.querySelector('.m-board-wrap'), wrap)
  view.render({ ...state, cursor: 'e1.b', view: { ...state.view, hiddenStatuses: ['todo'] } }, idx)
  assert.notEqual(root.querySelector('.m-board-wrap'), wrap)
  const wrap2 = root.querySelector('.m-board-wrap')
  view.render({ ...state, cursor: 'e1.b', view: { ...state.view, hiddenStatuses: ['todo'] } }, build(ROWS))
  assert.notEqual(root.querySelector('.m-board-wrap'), wrap2)
})

test('board view: a focused card hands the focus to the card the cursor moved to in place (se-mzlx)', () => {
  const { root, view } = bare()
  const idx = build(ROWS)
  const state = { ...initialState(), tab: 'board', cursor: 'e1.a' }
  view.render(state, idx)
  root.querySelector('[data-id="e1.a"]').focus()
  const wrap = root.querySelector('.m-board-wrap')
  view.render({ ...state, cursor: 'e1.b' }, idx)
  assert.equal(root.querySelector('.m-board-wrap'), wrap, 'drawn in place')
  assert.equal(root.ownerDocument.activeElement, root.querySelector('[data-id="e1.b"]'), 'the focus ring and the selection stay one card')
})

test('board view: the command acts on the cursor the in-place render recorded', () => {
  const moved = []
  const { view } = bare({ choose() {}, point: (id) => moved.push(id) })
  const idx = build(ROWS)
  const state = { ...initialState(), tab: 'board', cursor: 'e1.a' }
  view.render(state, idx)
  view.render({ ...state, cursor: 'e1.b' }, idx)
  view.command('collapse')
  assert.deepEqual(moved, ['e1.a'], 'h goes left of e1.b, not of e1.a')
})

test('board view: a card drawn only for the cursor goes when the cursor leaves it (pinned: drawn again)', () => {
  const view = { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false }
  assert.equal(groupBoard(index, look({ view, cursor: 'e1.c' })).pinned, true)
  assert.equal(groupBoard(index, look({ view, cursor: 'e1.a' })).pinned, false)
  assert.equal(groupBoard(index, look({ view, cursor: 'e1' })).pinned, false, 'a group in a shown column pins nothing')
  const { root, view: pane } = bare()
  const idx = build(ROWS)
  const state = { ...initialState(), tab: 'board', view, cursor: 'e1.c' }
  pane.render(state, idx)
  assert.ok(root.querySelector('[data-status="done"] [data-id="e1.c"]'))
  pane.render({ ...state, cursor: 'e1.a' }, idx)
  assert.equal(root.querySelector('[data-status="done"]'), null, 'the column that only the cursor kept is gone')
})

test('board view: a cursor on a card not drawn draws again (it may bring a column)', () => {
  const view = { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false }
  const { root, view: pane } = bare()
  const idx = build(ROWS)
  const state = { ...initialState(), tab: 'board', view, cursor: 'e1.a' }
  pane.render(state, idx)
  assert.equal(root.querySelector('[data-id="e1.c"]'), null)
  pane.render({ ...state, cursor: 'e1.c' }, idx)
  assert.ok(root.querySelector('[data-status="done"] [data-id="e1.c"]'))
})

test('board view: a store change that touches nothing the board draws keeps it, the cursor on a group or on a pinned card too (se-mzlx)', () => {
  const pinned = { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false }
  const unrelated = (state) => [
    { ...state, shown: { value: null, loading: true, problem: null } },
    { ...state, focus: 'detail' },
    { ...state, filter: { ...state.filter, text: 'sta' } },
    { ...state, board: { ...state.board, loading: true } },
    { ...state, toolbar: { ...state.toolbar, hintsOpen: true, hintIndex: 0 } },
  ]
  const bases = {
    'a group': { ...initialState(), tab: 'board', cursor: 'e1' },
    'a pinned card': { ...initialState(), tab: 'board', view: pinned, cursor: 'e1.c' },
    'no cursor': { ...initialState(), tab: 'board', cursor: null },
    'a card': { ...initialState(), tab: 'board', cursor: 'e1.a' },
  }
  for (const [name, base] of Object.entries(bases)) {
    const { root, view } = bare()
    const idx = build(ROWS)
    view.render(base, idx)
    const wrap = root.querySelector('.m-board-wrap')
    const cards = [...root.querySelectorAll('.m-card')]
    for (const next of unrelated(base)) {
      view.render(next, idx)
      assert.equal(root.querySelector('.m-board-wrap'), wrap, `${name}: the board stayed`)
    }
    assert.deepEqual([...root.querySelectorAll('.m-card')], cards, `${name}: the same card nodes`)
  }
})

test('board view: what the board is drawn from, or the cursor, still draws it again after an unrelated change', () => {
  const pinned = { hiddenStatuses: ['done'], hideDeferred: true, hideBacklog: true, showArchived: false }
  const { root, view } = bare()
  const idx = build(ROWS)
  const base = { ...initialState(), tab: 'board', view: pinned, cursor: 'e1.c' }
  view.render(base, idx)
  view.render({ ...base, focus: 'detail' }, idx)
  assert.ok(root.querySelector('[data-status="done"] [data-id="e1.c"]'), 'drawn for the cursor')
  // The cursor leaves the pinned card: it goes.
  view.render({ ...base, focus: 'detail', cursor: 'e1.a' }, idx)
  assert.equal(root.querySelector('[data-status="done"]'), null, 'the column that only the cursor kept is gone')
  // A group cursor in a hidden column brings the column: a cursor change from a card to a non-card draws again.
  const wrap = root.querySelector('.m-board-wrap')
  view.render({ ...base, focus: 'detail', cursor: 'e1' }, idx)
  assert.notEqual(root.querySelector('.m-board-wrap'), wrap)
  // The same cursor, another view or index: drawn again.
  const wrap2 = root.querySelector('.m-board-wrap')
  view.render({ ...base, focus: 'list', cursor: 'e1', view: { ...pinned, hiddenStatuses: [] } }, idx)
  assert.notEqual(root.querySelector('.m-board-wrap'), wrap2)
  assert.ok(root.querySelector('[data-status="done"] [data-id="e1.c"]'), 'the done column is back with the view')
  const wrap3 = root.querySelector('.m-board-wrap')
  view.render({ ...base, focus: 'list', cursor: 'e1', view: { ...pinned, hiddenStatuses: [] } }, build(ROWS))
  assert.notEqual(root.querySelector('.m-board-wrap'), wrap3)
})

test('board view: the command reads the cursor of the last change that left the board alone', () => {
  const moved = []
  const { view } = bare({ choose() {}, point: (id) => moved.push(id) })
  const idx = build(ROWS)
  const base = { ...initialState(), tab: 'board', cursor: 'e1' }
  view.render(base, idx)
  view.render({ ...base, focus: 'detail' }, idx)
  view.command('move.down')
  assert.deepEqual(moved, ['e1.a'], 'the cursor is on no card: the first card')
})

test('board view: NEW stands before the title of an unread card only (se-ehdi.zcx)', () => {
  const rows = ROWS.map((row) => (row.id === 'e1.a' ? { ...row, unread: true } : row.id === 'e1.b' ? { ...row, unread: true, archived: true } : row))
  const { root } = mount({ ...initialState(), tab: 'board' }, rows)
  const title = (id) => root.querySelector(`[data-id="${id}"] .m-card-title`)
  assert.equal(title('e1.a').firstChild.textContent, 'NEW')
  assert.ok(title('e1.a').firstChild.classList.contains('m-chip-new'))
  assert.match(title('e1.a').textContent, /^NEW A$/)
  assert.equal(title('e1.b'), null, 'the board does not draw an archive row unless archived rows are shown')
  assert.equal(title('e1.c').querySelector('.m-chip-new'), null)
  assert.equal(mount({ ...initialState(), tab: 'board' }).root.querySelector('.m-chip-new'), null)
  // Archived rows shown: the card is drawn, and an archive row is never NEW (moai read cannot clear it).
  const shown = mount({ ...initialState(), tab: 'board', view: { ...initialState().view, showArchived: true } }, rows).root
  assert.ok(shown.querySelector('[data-id="e1.b"] .m-card-title'), 'the archive card is drawn')
  assert.equal(shown.querySelector('[data-id="e1.b"] .m-chip-new'), null)
  assert.ok(shown.querySelector('[data-id="e1.a"] .m-chip-new'), 'a live unread card keeps its mark')
})

test('board view: a render in place is the render a fresh board gives, for every input of the cards', () => {
  const rows = [
    ...ROWS,
    // An epic whose column the view hides, to be kept only for the cursor standing on it.
    { id: 'e3', kind: 'epic', title: 'Done epic', milestone: 'm1', status: 'done' },
  ]
  const idx = build(rows)
  const html = (state) => {
    const { root, view } = bare()
    view.render(state, idx)
    return root.innerHTML
  }
  const base = { ...initialState(), tab: 'board', cursor: 'e1.a' }
  const changes = {
    path: { from: base, to: { path: ['m:m1'] } },
    view: { from: base, to: { view: { ...base.view, hiddenStatuses: ['todo'] } } },
    'search text': { from: base, to: { search: { text: 'B', open: true } } },
    filter: { from: base, to: { filter: { text: 'status=in_progress', applied: 'status=in_progress' } } },
    columns: { from: base, to: { columns: { ...base.columns, epic: true, assignee: true } } },
  }
  for (const [name, { from, to }] of Object.entries(changes)) {
    // Draw the base, then the change with the cursor moved to another card: the shortcut must not keep the old DOM.
    const { root, view } = bare()
    view.render({ ...from, cursor: 'e1.a' }, idx)
    const next = { ...from, ...to, cursor: 'e1.b' }
    view.render(next, idx)
    assert.equal(root.innerHTML, html(next), `${name}: the board drawn in place equals the board drawn fresh`)
  }
  // A cursor on a group whose column the view hides keeps that column only while it stands there.
  const hidden = { ...base, view: { ...base.view, hiddenStatuses: ['done'] }, cursor: 'e3' }
  const { root, view } = bare()
  view.render(hidden, idx)
  assert.ok(root.querySelector('[data-status="done"]'), 'the column is kept for the cursor on the epic')
  view.render({ ...hidden, cursor: 'e1.a' }, idx)
  assert.equal(root.querySelector('[data-status="done"]'), null, 'the cursor left it: the column goes')
  assert.equal(root.innerHTML, html({ ...hidden, cursor: 'e1.a' }))
})

const fixture = (name) => JSON.parse(readFileSync(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))

test('board: lanes come from home[0] of moai\'s tree, and the deferred column head is `‖` (real fixtures)', () => {
  const rows = [...fixture('rows.json'), ...fixture('backlog.json').map((row) => ({ ...row, kind: 'backlog' }))]
  const idx = new IssueIndex(rows, fixture('tree.json'))
  const show = { ...look().view, hideDeferred: false }
  const model = groupBoard(idx, look({ view: show }))
  assert.deepEqual(model.lanes.map((lane) => lane.seg), ['m:fx-ms01', NO_MILESTONE, 'lost'])
  const idsIn = (lane, key) => model.lanes[lane].columns.find((c) => c.key === key)?.cards.map((at) => idx.rows[at].id)
  assert.ok(idsIn(0, 'in_progress').includes('fx-ep01.aaa'))
  assert.ok(idsIn(0, 'review').includes('fx-ep01.bbb'))
  assert.deepEqual(idsIn(1, DEFERRED), ['fx-ep02.aaa'], 'shelved_by puts a member of a deferred epic in Deferred')
  assert.deepEqual(idsIn(1, BACKLOG), ['fx-th01'])
  assert.ok(idsIn(2, 'todo').includes('fx-is03'))
  assert.ok(!model.lanes.flatMap((lane) => lane.columns.flatMap((c) => c.cards)).some((at) => idx.rows[at].id === 'fx-is02'), 'the archived row is no card')
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const host = dom.window.document.getElementById('r')
  const pane = mountBoard(host, { document: dom.window.document, copy: copyFor('en'), language: 'en', intents: { choose() {}, point() {} }, layout: () => ({}), focusPane() {} })
  pane.render({ ...initialState(), tab: 'board', view: show }, idx)
  assert.match(host.querySelector('[data-status="deferred"]').parentElement.textContent, /‖/)
  assert.equal(glyphOf(DEFERRED), '‖')
  assert.equal(glyphOf(BACKLOG), '◇')
  const boot = { columns: { backlog: { name: 'Ideas', glyph: 'B' }, shelved: { name: 'Parked', glyph: 'P' } }, glyphs: {}, unknown_glyph: '?', statuses: [] }
  assert.equal(glyphOf(DEFERRED, boot), 'P')
  assert.equal(glyphOf(BACKLOG, boot), 'B')
})

// ── what someone holds moves (se-gowr.oie), the worktree overlay's branch mark (se-etwu) ──
const BR = await panelModule('view/branch.ts')
const SPIN = await panelModule('view/spin.ts')

test('board view: held work moves as in the TUI — its glyph turns, a band runs over its title, its column head turns (se-gowr.oie)', () => {
  const rows = [
    { id: 'h1', kind: 'issue', title: '집은 일', status: 'in_progress', priority: 1 },
    { id: 'h2', kind: 'issue', title: 'Put off', status: 'in_progress', priority: 1, deferred_at: '2026-01-01', shelved_by: 'h2' },
    { id: 't1', kind: 'issue', title: 'Waiting', status: 'todo', priority: 2 },
  ]
  const { root } = mount({ ...initialState(), tab: 'board', view: { ...initialState().view, hideDeferred: false } }, rows)
  const card = (id) => root.querySelector(`.m-card[data-id="${id}"]`)
  assert.ok(card('h1').querySelector('.m-st-in_progress.m-spin'), 'the held card turns')
  const band = card('h1').querySelector('.m-card-title .m-glint')
  assert.equal(band.textContent, '집은 일', 'the band holds the title, as text')
  // Four cells of Hangul, a space and two more: 7 cells, the band's 3 and the rest's 8 at 120 ms a cell.
  assert.equal(band.style.getPropertyValue('--glint-time'), `${(7 + 3 + 8) * 120}ms`)
  assert.equal(card('h2').querySelector('.m-spin, .m-glint'), null, 'deferred work stands still, as in the TUI')
  assert.equal(card('t1').querySelector('.m-spin, .m-glint'), null)
  // The glyph stays in the DOM: what a reader hears, and what stands without motion.
  assert.equal(card('h1').querySelector('.m-spin').textContent, '▸')
  const head = (key) => root.querySelector(`.m-lane[data-status="${key}"] .m-lane-head span[aria-hidden]`)
  assert.ok(head('in_progress').classList.contains('m-spin'))
  assert.ok(!head('todo').classList.contains('m-spin'))
})

test('spin: the first column is the configured one once the boot says it, else todo', () => {
  const rows = [
    { id: 'a', kind: 'issue', title: 'A', status: 'todo' },
    { id: 'b', kind: 'issue', title: 'B', status: 'backlog' },
  ]
  const index = build(rows)
  assert.equal(SPIN.firstStatus(null), 'todo')
  assert.equal(SPIN.firstStatus({ statuses: ['backlog', 'todo', 'done'] }), 'backlog')
  assert.equal(SPIN.spins(index, 0), false, 'todo is the first column by default')
  assert.equal(SPIN.spins(index, 1), true, 'a column between the first and done is a started one')
  assert.equal(SPIN.spins(index, 0, 'backlog'), true, 'with backlog first, todo is started')
  assert.equal(SPIN.spins(index, 1, 'backlog'), false)
})

test('spin: a group moves only while work under it moves, and a column the config does not know never moves (tui::App::spins)', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', title: 'v1', status: 'todo', derived_status: 'in_progress' },
    { id: 'e1', kind: 'epic', title: 'Half done', milestone: 'm1', status: 'todo', derived_status: 'in_progress' },
    { id: 'e1.a', kind: 'issue', title: 'A', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'done' },
    { id: 'e1.b', kind: 'issue', title: 'B', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'todo' },
    { id: 'x', kind: 'issue', title: 'Old column', status: 'blocked' },
  ]
  const with_ = (change) => build(rows.map((row) => (row.id === 'e1.b' ? { ...row, ...change } : row)))
  // One member done and the rest waiting stands the group in a started column (`column_of`): half done, not being done.
  const half = build(rows)
  assert.equal(SPIN.spins(half, half.find('e1')), false)
  assert.equal(SPIN.spins(half, half.find('m1')), false)
  const held = with_({ status: 'in_progress' })
  assert.equal(SPIN.spins(held, held.find('e1')), true, 'work under it moves')
  assert.equal(SPIN.spins(held, held.find('m1')), true, 'and so does every group above it')
  const shelved = with_({ status: 'in_progress', deferred_at: '2026-01-01', shelved_by: 'e1.b' })
  assert.equal(SPIN.spins(shelved, shelved.find('e1')), false, 'deferred work does not make it busy')
  // A column left from an older config stands still once the boot names the columns.
  assert.equal(SPIN.spins(half, half.find('x'), 'todo', ['todo', 'in_progress', 'review', 'done']), false)
  assert.equal(SPIN.spins(half, half.find('x')), true, 'before the boot answers, the columns are not known')
})

test('branch mark: the holder, else where the row came from; the name kept by its tail; none on a done row', () => {
  assert.equal(BR.clipFront('wt-se-43oh'), 'wt-se-43oh')
  assert.equal(BR.clipFront('worktree-moai-3fnf-long'), '…-moai-3fnf-long')
  assert.equal([...BR.clipFront('worktree-moai-3fnf-long')].length, 16, 'the cap is the TUI\'s 16 cells')
  assert.ok(BR.clipFront('worktree-moai-3fnf-long').endsWith('3fnf-long'), 'the tail, where the issue id is, stays')
  assert.equal(BR.branchOf({ id: 'a', status: 'in_progress', branch: 'b1' }), 'b1')
  assert.equal(BR.branchOf({ id: 'a', status: 'in_progress', branch: 'b1', working: 'w1' }), 'w1', 'the worktree that holds it wins')
  assert.equal(BR.branchOf({ id: 'a', status: 'done', branch: 'b1' }), undefined)
  assert.equal(BR.branchOf({ id: 'a', status: 'todo' }), undefined)
  assert.deepEqual(BR.worktreesOf([{ id: 'a', branch: 'z' }, { id: 'b', branch: 'a' }, { id: 'c', branch: 'z' }, { id: 'd' }]), ['a', 'z'])
  const doc = new JSDOM('<!doctype html><body></body>').window.document
  const mark = BR.branchMark(doc, 'worktree-moai-3fnf-long')
  assert.equal(mark.className, 'm-branch')
  assert.equal(mark.title, '⎇ worktree-moai-3fnf-long')
  assert.ok(mark.textContent.startsWith('⎇ …'))
})

test('board view: a card another worktree holds carries ⎇ and its branch on its first line', () => {
  const rows = [
    { id: 'h1', kind: 'issue', title: 'Held over there', status: 'in_progress', priority: 1, branch: 'wt-h1' },
    { id: 'd1', kind: 'issue', title: 'Done over there', status: 'done', priority: 1, branch: 'wt-d1' },
  ]
  const { root } = mount({ ...initialState(), tab: 'board', view: { ...initialState().view, hiddenStatuses: [] } }, rows)
  assert.equal(root.querySelector('.m-card[data-id="h1"] .m-card-line .m-branch').textContent, '⎇ wt-h1')
  assert.equal(root.querySelector('.m-card[data-id="d1"] .m-branch'), null)
})

test('board view: review moves too, as every started column does in the TUI (Config::is_started); to do and done stand still', () => {
  const rows = [
    { id: 'r1', kind: 'issue', title: 'Waiting on a review', status: 'review', priority: 1 },
    { id: 't1', kind: 'issue', title: 'Not started', status: 'todo', priority: 1 },
    { id: 'd1', kind: 'issue', title: 'Finished', status: 'done', priority: 1 },
  ]
  const { root } = mount({ ...initialState(), tab: 'board', view: { ...initialState().view, hiddenStatuses: [] } }, rows)
  const card = (id) => root.querySelector(`.m-card[data-id="${id}"]`)
  assert.ok(card('r1').querySelector('.m-st-review.m-spin'), 'its glyph turns, in its own colour')
  assert.equal(card('r1').querySelector('.m-glint').textContent, 'Waiting on a review')
  assert.equal(card('t1').querySelector('.m-spin, .m-glint'), null)
  assert.equal(card('d1').querySelector('.m-spin, .m-glint'), null)
  assert.ok(root.querySelector('.m-lane[data-status="review"] .m-lane-head .m-spin'))
})
