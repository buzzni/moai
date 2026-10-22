// Moai panel detail view (se-c10x.mh9): the pure wording of the head, fields, blockers, roll-up and journal,
// and the pane itself in a jsdom document.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const F = await panelModule('view/detail-format.ts')
const { mountDetail, renderBody } = await panelModule('view/detail.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const build = (rows, tree = treeOf(rows)) => new IssueIndex(rows, tree)
const { initialState, reduce } = await panelModule('store.ts')
const { copyFor } = await panelModule('i18n/index.ts')

const en = copyFor('en')
const ko = copyFor('ko')

const ROWS = [
  { id: 'se-m1', kind: 'milestone', title: 'v2.0', status: 'todo' },
  { id: 'se-e1', kind: 'epic', title: 'E3 views', milestone: 'se-m1', status: 'todo' },
  { id: 'se-e1.a', kind: 'issue', title: 'List', epic: 'se-e1', derived_epic: 'se-e1', status: 'done', priority: 1 },
  { id: 'se-e1.b', kind: 'issue', title: 'Board', epic: 'se-e1', derived_epic: 'se-e1', status: 'in_progress', blocked_by: ['se-e1.a', 'se-e1.c', 'se-gone'] },
  { id: 'se-e1.c', kind: 'issue', title: 'Detail', epic: 'se-e1', derived_epic: 'se-e1', status: 'todo', deferred_at: '2026-10-01T00:00:00Z', shelved_by: 'se-e1.c' },
]
const index = build(ROWS)

test('head: status label and tone, priority, kind and deferral', () => {
  const head = F.headOf({ id: 'x', title: 'T', status: 'in_progress', priority: 1, kind: 'epic' }, ko)
  assert.deepEqual([head.statusLabel, head.tone, head.priority, head.kind], ['진행 중', 'accent', 'P1', '에픽'])
  assert.equal(F.headOf({ id: 'x', status: 'done', kind: 'issue' }, en).kind, '')
  assert.equal(F.headOf({ id: 'x' }, en).title, 'x')
  assert.equal(F.headOf({ id: 'x', status: 'todo', deferred_at: '2026-10-01T00:00:00Z', deferred_reason: 'later' }, en).deferred, 'later')
  // A closed row is not deferred whatever it says.
  assert.equal(F.headOf({ id: 'x', status: 'done', deferred_at: '2026-10-01T00:00:00Z', shelved_by: 'se-e1.c' }, en).deferred, '')
})

test('fields: only those with a value, epic and milestone with their titles', () => {
  const row = { id: 'se-e1.b', assignee: 'dev', assignee_email: 'dev@example.com', epic: 'se-e1', derived_epic: 'se-e1', milestone: 'se-m1', created_at: 'not a date' }
  const fields = F.fieldsOf(row, index, en)
  assert.deepEqual(fields.map((f) => f.key), ['assignee', 'epic', 'milestone'])
  assert.equal(fields[0].value, 'dev (dev@example.com)')
  assert.equal(fields[1].value, 'E3 views (se-e1)')
  assert.equal(F.formatDate('2026-10-12'), '2026-10-12')
  assert.match(F.formatDate('2026-10-12T13:17:11Z'), /^2026-10-1[23] \d\d:\d\d$/)
  assert.equal(F.formatDate(undefined), '')
})

test('blockers: done, open, deferred and unknown; blocks come from the rows that name it', () => {
  assert.equal(F.blockerState('se-e1.a', index), 'done')
  assert.equal(F.blockerState('se-e1.b', index), 'open')
  assert.equal(F.blockerState('se-e1.c', index), 'deferred')
  assert.equal(F.blockerState('se-gone', index), 'missing')
  assert.equal(F.blockerState('se-e1.a', null), 'missing')
  assert.deepEqual(F.blocksOf({ id: 'se-e1.a' }, index), ['se-e1.b'])
  assert.deepEqual(F.blocksOf({ id: 'se-e1.a', blocks: ['z'] }, index), ['z', 'se-e1.b'])
})

test('roll-up: n/n of the work under a group, none for work', () => {
  const rollup = F.rollupOf(ROWS[1], index, new Map([['se-e1', { todo: 1, in_progress: 1, done: 1 }]]))
  assert.equal(rollup.done, 1)
  assert.equal(rollup.work, 3)
  assert.equal(rollup.percent, 33)
  // The columns in their configured order, not the order the rows were met in.
  assert.deepEqual(rollup.byStatus, [['todo', 1], ['in_progress', 1], ['done', 1]])
  assert.equal(F.rollupOf(ROWS[2], index), null)
  assert.equal(F.rollupOf(ROWS[1], null), null)
  assert.equal(en.detail.rollupText(1, 3, 33), '1/3 done · 33%')
  assert.equal(en.detail.rollupText(0, 0, undefined), '0/0 done')
})

test('roll-up: an issue with `.child` ids is a folder in the list and has the roll-up too', () => {
  const rows = [
    { id: 't-1', kind: 'issue', title: 'Parent', status: 'todo' },
    { id: 't-1.a', kind: 'issue', title: 'A', status: 'done' },
    { id: 't-1.b', kind: 'issue', title: 'B', status: 'review' },
    { id: 't-2', kind: 'issue', title: 'Plain', status: 'todo' },
  ]
  const idx = build(rows)
  const rollup = F.rollupOf(rows[0], idx)
  assert.deepEqual([rollup.done, rollup.work, rollup.percent], [1, 2, 50])
  assert.deepEqual(rollup.byStatus, [['review', 1], ['done', 1]])
  assert.equal(F.rollupOf(rows[3], idx), null)
})

test('blockers: `show` says what each blocker is; a finished one is not "not found" because the index holds open work only', () => {
  // The index here holds prime rows (open work): the blocker that finished is not in it.
  const open = build([{ id: 'p-1', title: 'Open', status: 'todo' }, { id: 'p-2', title: 'Blocked', status: 'todo', blocked_by: ['p-1', 'p-done', 'p-late', 'p-gone'] }])
  const row = {
    id: 'p-2',
    blocked_by: ['p-1', 'p-done', 'p-late', 'p-gone'],
    blockers: [{ id: 'p-1', state: 'open' }, { id: 'p-done', state: 'done' }, { id: 'p-late', state: 'deferred' }, { id: 'p-gone', state: 'missing' }, { id: 'p-empty', state: 'empty' }],
  }
  assert.deepEqual(['p-1', 'p-done', 'p-late', 'p-gone', 'p-empty'].map((id) => F.blockerState(id, open, row)), ['open', 'done', 'deferred', 'missing', 'open'])
  // Without the key (a row not read by `show`) the index is the only answer.
  assert.equal(F.blockerState('p-done', open), 'missing')
  assert.equal(F.blockerState('p-1', open, { id: 'p-2' }), 'open')
  // And the pane draws the finished blocker with its mark, not '?'.
  const { root, view } = mount()
  view.render(stateWith({ value: { id: 'p-2', row } }), open)
  assert.deepEqual([...root.querySelectorAll('.moai-detail-mark')].map((m) => m.textContent), ['·', '✓', '⏸', '?'])
})

test('a row put off through its epic carries shelved_by (not deferred_at): the head says so; a group blocker reads its derived column', () => {
  const head = F.headOf({ id: 'x', status: 'todo', shelved_by: 'trk-ep01' }, en)
  assert.deepEqual([head.deferred, head.shelvedBy], ['', 'trk-ep01'])
  assert.equal(F.headOf({ id: 'x', status: 'done', shelved_by: 'trk-ep01' }, en).shelvedBy, '', 'a finished row is not deferred')
  assert.equal(en.detail.deferredUnder('trk-ep01'), 'Deferred — under trk-ep01')
  const { root, view } = mount()
  view.render(stateWith({ value: { id: 'x', row: { id: 'x', title: 'Member', status: 'todo', shelved_by: 'trk-ep01' } } }), index)
  assert.ok([...root.querySelectorAll('.moai-detail-chips .m-chip-warn')].some((chip) => chip.textContent === 'Deferred — under trk-ep01'))
  // An epic whose members are all done is `todo` as written and `done` as it stands: the blocker is freed.
  const idx = build([
    { id: 'e-1', kind: 'epic', title: 'Epic', status: 'todo', derived_status: 'done' },
    { id: 'e-1.a', kind: 'issue', title: 'A', epic: 'e-1', status: 'done' },
  ])
  assert.equal(F.blockerState('e-1', idx), 'done')
})

test('what `show --json` really sends: children are ids, a due date is due_on, a move has a note, tracker commits are flagged, work is a record', () => {
  const row = {
    id: 'sw-1',
    children: ['sw-1.a', 'sw-1.b', { id: 'sw-1.c', title: 'Given' }],
    due_on: '2026-12-31',
    journal: [{ kind: 'status', from: 'todo', to: 'in_progress', note: 'picked up: start with the pure model', by: 'dev' }],
    commits: [
      { hash: 'c683335e735c5fc520c82d184214cd778fb6b0bb', subject: 'chore(tracker): sw-1 picked up', tracker: true },
      { hash: '8dc8db2aec0202b3657faf53148219bf742675fe', subject: 'sw-1: draw the list', tracker: false },
    ],
    work: [{ provider: 'anthropic', model: 'opus-5', tokens: 12345, grade: 'medium', why: 'wrote the list view', at: '2026-10-14T06:11:19Z', by: 'Tester' }],
  }
  const idx = build([{ id: 'sw-1', title: 'Parent', status: 'todo' }, { id: 'sw-1.a', title: 'Child of list', status: 'todo' }])
  assert.deepEqual(F.childrenOf(row, idx), [{ id: 'sw-1.a', title: 'Child of list' }, { id: 'sw-1.b' }, { id: 'sw-1.c', title: 'Given' }])
  assert.equal(F.fieldsOf(row, idx, en).find((f) => f.key === 'due').value, '2026-12-31')
  assert.equal(F.journalOf(row, en)[0].text, 'To do → In progress — picked up: start with the pure model')
  assert.deepEqual(F.commitsOf(row), [{ hash: '8dc8db2', subject: 'sw-1: draw the list' }])
  assert.match(F.workOf(row)[0], /^anthropic\/opus-5 tokens=12345 \(medium — wrote the list view\)  Tester · 2026-10-14/)
})

test('journal, commits and work lines', () => {
  const row = {
    id: 'a',
    journal: [
      { kind: 'create', by: 'dev', ts: '2026-10-10T15:11:44Z' },
      { kind: 'status', from: 'todo', to: 'in_progress', by: 'dev' },
      { kind: 'note', text: 'hello' },
      7,
    ],
    commits: ['0123456789', { sha: 'abcdef012345', subject: 'fix it\nmore' }, {}],
    work: [{ summary: 'ran', by: 'ai' }, 'plain'],
  }
  const lines = F.journalOf(row, en)
  assert.deepEqual(lines.map((l) => l.text), ['Created', 'To do → In progress', 'hello'])
  assert.match(lines[0].meta, /^dev · 2026-/)
  assert.deepEqual(F.commitsOf(row), [{ hash: '0123456', subject: '' }, { hash: 'abcdef0', subject: 'fix it' }])
  assert.deepEqual(F.workOf(row), ['ran  ai', 'plain'])
})

test('problem text uses the shell words', () => {
  assert.equal(F.problemText({ kind: 'not-started' }, en), en.unreachable)
  assert.equal(F.problemText({ kind: 'failed', message: 'page b skipped' }, en), en.moaiError('page b skipped'))
  assert.equal(F.problemText({ kind: 'failed' }, en), en.failed)
  assert.equal(F.problemText({ kind: 'moai-error', message: 'boom' }, en), en.moaiError('boom'))
  assert.equal(F.problemText({ kind: 'passed', output: 1 }, en), null)
})

// ── the pane ──────────────────────────────────────────────────────────────────────────────────────────

function mount(overrides = {}) {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const doc = dom.window.document
  const calls = []
  const ctx = {
    document: doc, copy: en, language: 'en',
    intents: { closeDetail: () => calls.push('close') },
    layout: () => ({ width: 800, mainWidth: 800, position: 'bottom', detailVisible: true }),
    focusPane() {},
    ...overrides,
  }
  const root = doc.getElementById('r')
  const view = mountDetail(root, ctx)
  return { doc, root, view, calls }
}
const stateWith = (shown) => ({ ...initialState(), shown: { value: null, loading: false, problem: null, ...shown } })

test('pane: nothing chosen, loading and a problem', () => {
  const { root, view } = mount()
  view.render(stateWith({}), index)
  assert.match(root.textContent, /Choose a task/)
  view.render(stateWith({ loading: true }), index)
  assert.match(root.textContent, /Loading/)
  // What the person chose is information, as in the shell's banners; a machine problem is an error.
  view.render(stateWith({ problem: { kind: 'cancelled' } }), index)
  assert.equal(root.querySelector('.m-banner-danger'), null)
  assert.equal(root.querySelector('.m-banner').getAttribute('role'), 'status')
  assert.match(root.textContent, /read was cancelled/)
  view.render(stateWith({ problem: { kind: 'moai-error', message: 'boom' } }), index)
  assert.equal(root.querySelector('.m-banner-danger').getAttribute('role'), 'alert')
  assert.match(root.textContent, /boom/)
})

test('pane: the focus goes to the pane itself while it has no control, and the scroll is the split container\'s', () => {
  const dom = new JSDOM('<!doctype html><body><div id="scroller" data-keep-scroll><div id="r" tabindex="-1"></div></div></body>')
  const doc = dom.window.document
  const root = doc.getElementById('r')
  const scroller = doc.getElementById('scroller')
  const ctx = { document: doc, copy: en, language: 'en', intents: { closeDetail() {} }, layout: () => ({}), focusPane() {} }
  const view = mountDetail(root, ctx)
  view.render(stateWith({ loading: true }), index)
  view.focus()
  assert.equal(doc.activeElement, root, 'a pane that is only loading still takes the focus asked of it')
  // The same row redrawn keeps its place in the scroller; another row starts at the top.
  const row = ROWS[3]
  view.render(stateWith({ value: { id: row.id, row } }), index)
  scroller.scrollTop = 120
  view.render(stateWith({ value: { id: row.id, row } }), index)
  assert.equal(scroller.scrollTop, 120)
  view.render(stateWith({ value: { id: ROWS[1].id, row: ROWS[1] } }), index)
  assert.equal(scroller.scrollTop, 0)
  // With controls drawn the pane still takes the focus, not Close: the key router leaves Space and Enter on a focused
  // button to the button, so SPC meant for the menu (or Enter) would close the detail.
  view.focus()
  assert.equal(doc.activeElement, root)
})

test('pane: a group shows head, fields, roll-up, body as text, and close reports the intent', () => {
  const { doc, root, view, calls } = mount()
  const row = { ...ROWS[1], body: '<img src=x onerror=alert(1)>\nline two', tags: ['ui'], priority: 1, commits: [{ hash: 'abcdef0123', subject: 'first' }], journal: [{ kind: 'create', by: 'dev' }] }
  view.render(stateWith({ value: { id: 'se-e1', row } }), index)
  assert.equal(root.querySelector('.moai-detail-title').textContent, 'E3 views')
  assert.match(root.textContent, /1\/3 done · 33%/)
  assert.match(root.textContent, /v2\.0 \(se-m1\)/)
  assert.match(root.textContent, /abcdef0/)
  assert.match(root.textContent, /Created/)
  const body = root.querySelector('.moai-detail-body')
  assert.equal(body.querySelector('img'), null)
  assert.ok(body.textContent.includes('<img src=x onerror=alert(1)>'))
  assert.ok(body.textContent.includes('line two'))
  assert.ok(root.querySelector('[role=progressbar]'))
  // No `moai status` counts for this group (none were given here): the share done still fills the bar beside the text.
  assert.deepEqual([...root.querySelectorAll('[role=progressbar] > span')].map((span) => [span.dataset.status, span.style.width]), [['done', '33%']])
  root.querySelector('button').click()
  assert.deepEqual(calls, ['close'])
  assert.equal(renderBody(doc, 'a<b').textContent, 'a<b')
})

test('pane: an unrelated store change leaves the pane alone (its text selection and focus survive)', () => {
  const { root, view } = mount()
  const row = ROWS[3]
  const state = stateWith({ value: { id: row.id, row } })
  view.render(state, index)
  const detail = root.querySelector('.moai-detail')
  const summary = root.querySelector('details[data-fold=journal] summary')
  const body = root.querySelector('.moai-detail-title')
  // The cursor, the focus and the board's reads do not touch `shown`: the nodes stay the same ones.
  view.render({ ...state, cursor: 'se-e1.b', focus: 'detail', board: { value: null, loading: true, problem: null } }, index)
  assert.equal(root.querySelector('.moai-detail-title'), body)
  assert.equal(root.querySelector('details[data-fold=journal] summary'), summary)
  assert.equal(root.querySelector('.moai-detail'), detail)
  // A new read of the same row, or a new index, draws again.
  view.render({ ...state, shown: { ...state.shown } }, index)
  assert.notEqual(root.querySelector('.moai-detail-title'), body)
})

test('pane: a task lists its blockers with marks; a redraw keeps the opened history', () => {
  const { root, view } = mount()
  const row = ROWS[3]
  view.render(stateWith({ value: { id: row.id, row } }), index)
  const marks = [...root.querySelectorAll('.moai-detail-mark')].map((m) => m.textContent)
  assert.deepEqual(marks, ['✓', '⏸', '?'])
  assert.equal(root.querySelector('[role=progressbar]'), null)
  const fold = root.querySelector('details[data-fold=journal]')
  fold.open = true
  fold.dispatchEvent(new root.ownerDocument.defaultView.Event('toggle'))
  view.render(stateWith({ value: { id: row.id, row } }), index)
  assert.equal(root.querySelector('details[data-fold=journal]').open, true)
})

test('roll-up: a milestone or epic counts as the list does, n/n from the tree progress, per-status counts from `status --json`', () => {
  // The epic's members the index holds: one done of three. `moai status` counts six, four done (archived done ones).
  const counted = build(ROWS, treeOf(ROWS).map((line) => (line.id === 'se-e1' ? { ...line, progress: { done: 4, total: 6 } } : line)))
  const rollup = F.rollupOf(ROWS[1], counted, new Map([['se-e1', { todo: 1, in_progress: 1, done: 4 }]]))
  assert.deepEqual([rollup.done, rollup.work, rollup.percent], [4, 6, 66])
  assert.deepEqual([rollup.done, rollup.work], counted.tally(counted.dirPath(counted.find('se-e1'))), 'the n/n the list draws')
  // A group's per-status counts are the `moai status --json` counts (they include the archive); the n/n is the tree's.
  assert.deepEqual(rollup.byStatus, [['todo', 1], ['in_progress', 1], ['done', 4]])
})

test('pane: the per-column counts that come after the rows draw the roll-up again (the index stays the same)', () => {
  const { root, view } = mount()
  const state = stateWith({ value: { id: 'se-e1', row: ROWS[1] } })
  const board = { rows: ROWS, loadedAt: 0 }
  view.render({ ...state, board: { value: board, loading: false, problem: null } }, index)
  const before = root.querySelector('.moai-detail-title')
  const counted = { ...board, groups: new Map([['se-e1', { todo: 1, in_progress: 1, done: 4 }]]) }
  view.render({ ...state, board: { value: counted, loading: false, problem: null } }, index)
  assert.notEqual(root.querySelector('.moai-detail-title'), before, 'drawn again for the counts')
  assert.match(root.textContent, /4/)
})

test('pane: a search keystroke repaints the id and title only; the body and sections stay (se-mzlx)', () => {
  const { root, view } = mount()
  const row = { ...ROWS[3], body: 'some **body**' }
  const state = stateWith({ value: { id: row.id, row } })
  view.render(state, index)
  const body = root.querySelector('.moai-detail-body')
  const journal = root.querySelector('details[data-fold=journal]')
  const title = root.querySelector('.moai-detail-title')
  assert.ok(body && journal)
  assert.equal(title.querySelector('mark'), null)
  view.render({ ...state, search: { text: 'Boa', open: true } }, index)
  assert.ok(title.querySelector('mark'), 'the hit is painted')
  assert.equal(root.querySelector('.moai-detail-body'), body)
  assert.equal(root.querySelector('details[data-fold=journal]'), journal)
  assert.equal(root.querySelector('.moai-detail-title'), title)
  view.render({ ...state, search: { text: 'Boar', open: true } }, index)
  assert.equal(title.querySelector('mark').textContent, 'Boar')
  view.render({ ...state, search: { text: '', open: false } }, index)
  assert.equal(title.querySelector('mark'), null)
  assert.equal(root.querySelector('.moai-detail-body'), body)
})

test('pane: NEW first among the chips for an unread row; not for a read or archived one (se-ehdi.zcx)', () => {
  const { root, view } = mount()
  const chips = () => root.querySelector('.moai-detail-chips')
  const row = { ...ROWS[3], unread: true }
  view.render(stateWith({ value: { id: row.id, row } }), index)
  assert.equal(chips().firstChild.textContent, 'NEW')
  view.render(stateWith({ value: { id: ROWS[3].id, row: ROWS[3] } }), index)
  assert.equal(chips().querySelector('.m-chip-new'), null)
  // The list's own row says it is unread; the archive tag is only there too.
  const listed = build(ROWS.map((one) => (one.id === 'se-e1.b' ? { ...one, unread: true } : one)))
  view.render(stateWith({ value: { id: ROWS[3].id, row: ROWS[3] } }), listed)
  assert.equal(chips().firstChild.textContent, 'NEW', 'the row is looked up in the list when show did not carry it')
  const archived = build(ROWS.map((one) => (one.id === 'se-e1.b' ? { ...one, unread: true, archived: true } : one)))
  view.render(stateWith({ value: { id: row.id, row } }), archived)
  assert.equal(chips().querySelector('.m-chip-new'), null)
})
