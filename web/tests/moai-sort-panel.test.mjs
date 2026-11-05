import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const { sortPanel } = await panelModule('view/sort-panel.ts')
const doc = new JSDOM('<!doctype html>').window.document
const words = {
  presets: [{ id: 'release', label: 'Release' }], groups: [{ id: 'none', label: 'None' }],
  levels: [{ id: 'milestone', label: 'Milestone' }, { id: 'item', label: 'Item' }],
  fields: [{ id: 'priority', label: 'Priority', dir: 'asc' }, { id: 'due', label: 'Due', dir: 'asc' }],
  activeLevels: () => ['item'],
  labels: { presets: 'Presets', groups: 'Groups', order: 'Order', add: 'Add', direction: 'Direction', up: 'Up', down: 'Down', remove: 'Remove' },
}
const state = { group: 'none', level: 'item', order: { item: [{ key: 'priority', dir: 'asc' }] } }

test('panel exposes injected vocabulary, active tabs and guards the last field', () => {
  const actions = []
  const panel = sortPanel(doc, state, words, (a) => actions.push(a))
  assert.equal(panel.querySelector('[role=tab]').disabled, true)
  assert.equal(panel.querySelectorAll('[role=tab]')[1].getAttribute('aria-selected'), 'true')
  assert.equal(panel.querySelector('[aria-label=Remove]').disabled, true)
  panel.querySelector('[aria-label=Direction]').click()
  assert.deepEqual(actions.pop(), { type: 'fields', level: 'item', fields: [{ key: 'priority', dir: 'desc' }] })
  panel.querySelector('select').value = 'due'
  panel.querySelector('select + button').click()
  assert.deepEqual(actions.pop(), { type: 'fields', level: 'item', fields: [{ key: 'priority', dir: 'asc' }, { key: 'due', dir: 'asc' }] })
  assert.equal(state.order.item.length, 1, 'editing does not mutate caller state')
})

test('field controls reorder and remove without changing the input', () => {
  const input = { ...state, order: { item: [...state.order.item, { key: 'due', dir: 'asc' }] } }
  const actions = []
  const panel = sortPanel(doc, input, words, (a) => actions.push(a))
  panel.querySelectorAll('[aria-label=Up]')[1].click()
  assert.deepEqual(actions.pop().fields.map((f) => f.key), ['due', 'priority'])
  panel.querySelectorAll('[aria-label=Remove]')[0].click()
  assert.deepEqual(actions.pop().fields.map((f) => f.key), ['due'])
  assert.deepEqual(input.order.item.map((f) => f.key), ['priority', 'due'])
})

test('preset and command text are exposed without interpreting markup', () => {
  const actions = []
  const vocabulary = { ...words, labels: { ...words.labels, command: 'Command', speech: 'Say' } }
  const panel = sortPanel(doc, { ...state, preset: 'release', command: '<sort>', speech: '<say>' }, vocabulary, (a) => actions.push(a))
  const preset = [...panel.querySelectorAll('button')].find((node) => node.textContent === 'Release')
  assert.equal(preset.getAttribute('aria-pressed'), 'true')
  preset.click()
  assert.deepEqual(actions.pop(), { type: 'preset', id: 'release' })
  assert.equal(panel.querySelector('pre').textContent, '<sort>')
  assert.equal(panel.querySelector('q').textContent, '<say>')
  assert.equal(panel.querySelector('sort'), null)
})

test('toolbar preset and grouping update the shared list/board model', async () => {
  const { initialState, reduce } = await panelModule('store.ts')
  const { IssueIndex } = await panelModule('data/index.ts')
  const { listLines } = await panelModule('view/list-model.ts')
  const { groupBoard } = await panelModule('view/board-groups.ts')
  let state = initialState()
  state = reduce(state, { type: 'toolbar', action: { type: 'sort-panel', action: { type: 'preset', id: 'people' } } })
  assert.equal(state.toolbar.panel.group, 'assignee')
  const rows = [
    { id: 'a', kind: 'issue', status: 'todo', priority: 2, assignee: 'Zoe', created_at: '2026-01-01' },
    { id: 'b', kind: 'issue', status: 'todo', priority: 1, assignee: 'Amy', created_at: '2026-01-02' },
    { id: 'c', kind: 'issue', status: 'todo', priority: 0, created_at: '2026-01-03' },
  ]
  const index = new IssueIndex(rows)
  const { orderExpression } = await panelModule('view/sort-adapter.ts')
  const server = [{ kind: 'group', path: 'group:assignee:"Amy"' }, { id: 'b' }, { kind: 'group', path: 'group:assignee:"Zoe"' }, { id: 'a' }, { kind: 'group', path: 'group:assignee:null' }, { id: 'c' }]
  state = { ...state, board: { value: { rows, groupedTree: server, groupedBy: 'assignee', groupedSort: orderExpression(state.toolbar.panel) }, loading: false, problem: null } }
  const lines = listLines(state, index, () => true)
  assert.deepEqual(lines.filter((line) => line.kind === 'bucket').map((line) => line.label), ['Amy', 'Zoe', '(none)'])
  const board = groupBoard(index, { path: [], view: state.view, cursor: null, group: 'assignee', groupedTree: server })
  assert.deepEqual(board.lanes.map((lane) => lane.label), ['Amy', 'Zoe', '(none)'])
  assert.equal(board.total, 3)
  state = reduce(state, { type: 'toolbar', action: { type: 'sort-panel', action: { type: 'group', id: 'none' } } })
  assert.equal(listLines(state, index, () => true).filter((line) => line.kind === 'bucket').length, 0)
  assert.equal(groupBoard(index, { path: [], view: state.view, cursor: null, group: 'none' }).lanes[0].seg, null)
})

test('temporary groups keep null distinct from a literal marker and order cards', async () => {
  const { IssueIndex } = await panelModule('data/index.ts')
  const { groupBuckets } = await panelModule('view/group-adapter.ts')
  const rows = [
    { id: 'a', kind: 'issue', status: 'todo', tags: ['~', 'extra'], priority: 2, created_at: '2026-01-02' },
    { id: 'b', kind: 'issue', status: 'todo', tags: ['~'], priority: 0, created_at: '2026-01-01' },
    { id: 'c', kind: 'issue', status: 'todo', priority: 1, created_at: '2026-01-03' },
  ]
  const index = new IssueIndex(rows)
  const server = [{ kind: 'group', path: 'group:tag:"~"' }, { id: 'b' }, { id: 'a' }, { kind: 'group', path: 'group:tag:null' }, { id: 'c' }]
  const groups = groupBuckets(index, [], () => true, 'tag', [], false, [{ key: 'priority', dir: 'asc' }], [], server)
  assert.equal(groups.length, 2)
  assert.notEqual(groups[0].seg, groups[1].seg)
  assert.deepEqual(groups[0].members.map((at) => index.rows[at].id), ['b', 'a'])
  assert.deepEqual(groups[1].members.map((at) => index.rows[at].id), ['c'])
})

test('the structured list preserves the server layer order', async () => {
  const { initialState, reduce } = await panelModule('store.ts')
  const { IssueIndex, pathKey } = await panelModule('data/index.ts')
  const { listLines } = await panelModule('view/list-model.ts')
  const { treeOf } = await import('./moai-tree-helper.mjs')
  const rows = [
    { id: 'm2', kind: 'milestone', title: 'Later', status: 'todo', created_at: '2026-02-01' },
    { id: 'm1', kind: 'milestone', title: 'Earlier', status: 'todo', created_at: '2026-01-01' },
    { id: 'a', kind: 'issue', title: 'A', status: 'todo', milestone: 'm1', derived_milestone: 'm1', priority: 2 },
    { id: 'b', kind: 'issue', title: 'B', status: 'todo', milestone: 'm1', derived_milestone: 'm1', priority: 0 },
  ]
  const index = new IssueIndex(rows, treeOf([rows[1], rows[3], rows[2], rows[0]]))
  let state = initialState()
  state = reduce(state, { type: 'toolbar', action: { type: 'sort-panel', action: { type: 'fields', level: 'milestone', fields: [{ key: 'created', dir: 'asc' }] } } })
  state = { ...state, expanded: [pathKey(['m:m1']), pathKey(['m:m2'])] }
  assert.deepEqual(listLines(state, index, () => true).filter((line) => line.kind === 'row').map((line) => line.key), ['m1', 'b', 'a', 'm2'])
})

test('server grouping order is retained in list and board, including no grouping', async () => {
  const { IssueIndex } = await panelModule('data/index.ts')
  const { groupBuckets } = await panelModule('view/group-adapter.ts')
  const { groupBoard } = await panelModule('view/board-groups.ts')
  const { initialState } = await panelModule('store.ts')
  const rows = [
    { id: 'a', kind: 'issue', status: 'todo', priority: 0, assignee: 'Amy' },
    { id: 'b', kind: 'issue', status: 'todo', priority: 3, assignee: 'Amy' },
  ]
  const index = new IssueIndex(rows)
  const server = [{ kind: 'group', path: 'group:assignee:"Amy"' }, { id: 'b' }, { id: 'a' }]
  const bucket = groupBuckets(index, [], () => true, 'assignee', [], false, [{ key: 'priority', dir: 'asc' }], [], server)[0]
  assert.deepEqual(bucket.members.map((at) => rows[at].id), ['b', 'a'])
  const look = { path: [], view: initialState().view, cursor: null, group: 'assignee', itemOrder: [{ key: 'priority', dir: 'asc' }], groupedTree: server }
  const board = groupBoard(index, look)
  assert.deepEqual(board.lanes[0].columns.flatMap((column) => column.cards).map((at) => rows[at].id), ['b', 'a'])
  const flat = groupBoard(index, { ...look, group: 'none', groupedTree: [{ id: 'b' }, { id: 'a' }] })
  assert.deepEqual(flat.lanes[0].columns.flatMap((column) => column.cards).map((at) => rows[at].id), ['b', 'a'])
})


test('group labels resolve titles only for milestone and epic identifiers', async () => {
  const { IssueIndex } = await panelModule('data/index.ts')
  const { groupBuckets } = await panelModule('view/group-adapter.ts')
  const rows = [{ id: 'Amy', kind: 'issue', title: 'A different title', status: 'todo', assignee: 'Amy' }]
  const index = new IssueIndex(rows)
  const server = [{ kind: 'group', path: 'group:assignee:"Amy"' }, { id: 'Amy' }]
  assert.equal(groupBuckets(index, [], () => true, 'assignee', [], false, [], [], server)[0].label, 'Amy')
})

test('priority groups read the engine value p<n>: server and preview share one label and seg', async () => {
  const { IssueIndex } = await panelModule('data/index.ts')
  const { groupBuckets } = await panelModule('view/group-adapter.ts')
  const rows = [
    { id: 'a', kind: 'issue', status: 'todo', priority: 0, created_at: '2026-01-01' },
    { id: 'b', kind: 'issue', status: 'todo', created_at: '2026-01-02' },
  ]
  const index = new IssueIndex(rows)
  const server = [{ kind: 'group', path: 'group:priority:"p0"' }, { id: 'a' }, { kind: 'group', path: 'group:priority:"p2"' }, { id: 'b' }]
  const fromServer = groupBuckets(index, [], () => true, 'priority', [], false, [], [], server)
  const preview = groupBuckets(index, [], () => true, 'priority', [], false, [], [])
  assert.deepEqual(fromServer.map((bucket) => bucket.label), ['p0', 'p2'])
  assert.deepEqual(preview, [], 'wait for server instead of guessing its order')
})
