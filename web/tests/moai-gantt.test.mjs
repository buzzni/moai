// The gantt tab (moai-d86e.5wj, moai-d86e.en8): the pure placing (view/gantt-model.ts) and the view in a jsdom document.
// "Now" is 2026-10-25 14:00 UTC and the zone is UTC, so every position below is a day count at 40px a day.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const model = await panelModule('view/gantt-model.ts')
const { mountGantt, clock } = await panelModule('view/gantt.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { TABS } = await panelModule('view/tabs.ts')
const { ICONS } = await panelModule('view/icons.ts')
const { initialState } = await panelModule('store.ts')
const { copyFor, TABLE } = await panelModule('i18n/index.ts')
const { menuLayer, KEY_TABLE, helpSections } = await panelModule('keys/table.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')

const NOW = Date.UTC(2026, 9, 25, 14, 0)
clock.now = () => NOW
clock.zone = () => 'UTC'

const own = (start, end) => ({ start, end, from: 'own' })
const span = (start, end = null) => ({ kind: 'span', start, end })
const g = (id, over) => ({
  id, kind: 'issue', planned: null, actual: { kind: 'not_started' }, progress: { kind: 'column', name: 'todo' },
  overdue: null, deferred: false, blocked_by: [], ...over,
})

const ROWS = [
  { id: 'm1', kind: 'milestone', title: 'v1', status: 'in_progress', due_on: '2026-10-28' },
  { id: 'e1', kind: 'epic', title: 'Epic', milestone: 'm1', status: 'in_progress' },
  { id: 'e1.a', kind: 'issue', title: 'Done one', derived_epic: 'e1', status: 'done', assignee: 'kim' },
  { id: 'e1.b', kind: 'issue', title: 'Late one', derived_epic: 'e1', status: 'in_progress' },
  { id: 'e1.c', kind: 'issue', title: 'Blocked one', derived_epic: 'e1', status: 'todo' },
  { id: 'e1.d', kind: 'issue', title: 'Old row', derived_epic: 'e1', status: 'in_progress' },
  { id: 'e1.e', kind: 'issue', title: 'Put off', derived_epic: 'e1', status: 'todo' },
]
const GANTT = {
  m1: g('m1', { kind: 'milestone', planned: { start: '2026-10-20', end: '2026-10-26', from: 'members' }, actual: span('2026-10-20T09:10:00Z'), progress: { kind: 'rollup', done: 1, total: 5 } }),
  e1: g('e1', { kind: 'epic', planned: { start: '2026-10-10', end: '2026-10-27', from: 'members' }, actual: span('2026-10-19T09:00:00Z'), progress: { kind: 'rollup', done: 1, total: 5 } }),
  'e1.a': g('e1.a', { planned: own('2026-10-21', '2026-10-22'), actual: span('2026-10-21T09:20:00Z', '2026-10-22T15:05:00Z'), progress: { kind: 'column', name: 'done' } }),
  'e1.b': g('e1.b', { planned: own('2026-10-19', '2026-10-22'), actual: span('2026-10-19T09:00:00Z'), overdue: 3, progress: { kind: 'column', name: 'in_progress' } }),
  'e1.c': g('e1.c', { planned: own('2026-10-26', '2026-10-27'), blocked_by: [{ id: 'e1.a', state: 'done' }, { id: 'e1.b', state: 'open' }] }),
  'e1.d': g('e1.d', { planned: { start: null, end: '2026-10-27', from: 'own' }, actual: { kind: 'unknown' }, progress: { kind: 'column', name: 'in_progress' } }),
  'e1.e': g('e1.e', { planned: own('2026-10-10', '2026-10-12'), deferred: true }),
}
const build = () => {
  const tree = treeOf(ROWS).map((line) => (line.id ? { ...line, gantt: GANTT[line.id] } : line))
  return new IssueIndex(ROWS, tree)
}

/**
 * jsdom lays nothing out, and a pane with no size draws the window a 900 x 720 box would (moai-vj8w.2fh): these tests
 * look at bars all over a month, so the box they mount in is as big as the page they draw.
 */
const size = (root, width = 4000, height = 2000) => {
  const box = root.querySelector('.g-scroll')
  Object.defineProperty(box, 'clientWidth', { get: () => width, configurable: true })
  Object.defineProperty(box, 'clientHeight', { get: () => height, configurable: true })
  return box
}

const mount = (cursor = null, index = build(), dims = []) => {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const calls = []
  const root = dom.window.document.getElementById('r')
  const ctx = {
    document: dom.window.document, copy: copyFor('en'), language: 'en',
    intents: { choose: (id) => calls.push(`choose:${id}`), point: (id) => calls.push(`point:${id}`) },
    layout: () => ({ detailVisible: false }), focusPane: () => {},
  }
  const view = mountGantt(root, ctx)
  size(root, ...dims)
  const at = (c) => view.render({ ...initialState(), tab: 'gantt', cursor: c }, index)
  at(cursor)
  return { root, view, calls, dom, at, index }
}
/**
 * The view keeps the grid's width in its module's storage, which every mount in this file shares: a test that drags
 * it puts the default back with this, so the tests after it start from the width the first one did.
 */
function resetGrid({ root, dom }) {
  const box = root.querySelector('.g-scroll')
  Object.defineProperty(box, 'clientWidth', { get: () => 0, configurable: true })
  const drawn = Number.parseFloat(root.querySelector('.g-inner').style.getPropertyValue('--grid-w'))
  const handle = root.querySelector('.g-split')
  const fire = (type, x) => handle.dispatchEvent(new dom.window.MouseEvent(type, { clientX: x, bubbles: true }))
  fire('pointerdown', 0)
  fire('pointermove', model.DEFAULT_GRID - drawn)
  fire('pointerup', model.DEFAULT_GRID - drawn)
}
const lane = (root, id) => root.querySelector(`.lane[data-id="${id}"]`)
const px = (node, prop) => Number.parseFloat(node.style[prop])
const days = (iso) => (Date.UTC(...iso.split('-').map((n, i) => (i === 1 ? Number(n) - 1 : Number(n)))) - Date.UTC(2026, 9, 3)) / 864e5 * 40

test('the tab is in the strip with the gantt icon, and SPC g g reaches it', () => {
  const tab = TABS.find((one) => one.id === 'gantt')
  assert.ok(tab)
  assert.equal(tab.icon, 'gantt-chart')
  assert.ok(ICONS['gantt-chart'])
  assert.equal(copyFor('en').shell.tabs.gantt, 'Gantt')
  assert.equal(copyFor('ko').shell.tabs.gantt, '간트')
  const items = menuLayer('SPC g', { mode: 'browse' }).items
  assert.ok(items.some((item) => item.key === 'g' && item.action === 'tab.gantt'))
  assert.ok(KEY_TABLE.menu.some((binding) => binding.keys.includes('SPC g g') && binding.action === 'tab.gantt'))
  // The pane's own keys exist only on the gantt tab.
  const keys = (tab) => KEY_TABLE.browse.filter((b) => b.action.startsWith('gantt.') && (!b.when || b.when({ mode: 'browse', tab }))).map((b) => b.keys[0]).sort()
  assert.deepEqual(keys('gantt'), ['-', '=', 'H', 'L', 't', 'z'])
  assert.deepEqual(keys('list'), [])
  // The help lists them under their own group (keys/table.ts HELP_GROUPS), usable on the gantt tab.
  const gantt = helpSections('browse', { mode: 'browse', tab: 'gantt', focus: 'list' }, 'other', TABLE.en).find((s) => s.id === 'gantt')
  assert.ok(gantt, 'the help has a gantt section')
  assert.equal(gantt.title, 'Gantt')
  assert.deepEqual(gantt.rows.map((r) => [r.keys[0], r.available]), [['H', true], ['L', true], ['-', true], ['=', true], ['t', true], ['z', true]])
})

test('the model puts planned days, actual instants and the late band on the axis', () => {
  const day = model.dayStart('2026-10-21')
  assert.equal(day, Date.UTC(2026, 9, 21))
  // A planned end is inclusive: the bar runs to the end of that day.
  const a = model.placeRow(GANTT['e1.a'], ROWS[2], NOW, 'UTC')
  assert.deepEqual([a.plan.from, a.plan.to, a.plan.origin], [Date.UTC(2026, 9, 21), Date.UTC(2026, 9, 23), 'own'])
  assert.deepEqual([a.actual.from, a.actual.to, a.actual.open], [Date.UTC(2026, 9, 21, 9, 20), Date.UTC(2026, 9, 22, 15, 5), false])
  // Actual times are read in the viewer's zone, planned days are not moved by it.
  const seoul = model.placeRow(GANTT['e1.a'], ROWS[2], NOW, 'Asia/Seoul')
  assert.equal(seoul.actual.from, Date.UTC(2026, 9, 21, 18, 20))
  assert.equal(seoul.plan.from, Date.UTC(2026, 9, 21))
  const late = model.placeRow(GANTT['e1.b'], ROWS[3], NOW, 'UTC')
  assert.deepEqual(late.over, { from: Date.UTC(2026, 9, 23), to: NOW })
  assert.equal(late.actual.open, true)
  assert.equal(late.actual.to, NOW)
  // A start nobody recorded is not an actual band; a deadline with no start is a mark.
  const old = model.placeRow(GANTT['e1.d'], ROWS[5], NOW, 'UTC')
  assert.equal(old.actual, null)
  assert.equal(old.unknown, true)
  assert.equal(old.plan, null)
  assert.equal(old.due, Date.UTC(2026, 9, 28))
  // A milestone's own deadline marks the end of that day whatever its plan says.
  assert.equal(model.placeRow(GANTT.m1, ROWS[0], NOW, 'UTC').due, Date.UTC(2026, 9, 29))
  assert.equal(model.placeRow(undefined, ROWS[0], NOW).span, null)
})

test('late without a planned bar: a deadline-only row is late from its diamond, and moai\'s verdict is not asked again', () => {
  // The usual row writes a deadline and no start: no planned bar, a `◆` at the end of 10-22, late by 3 days.
  const due = g('x', { planned: { start: null, end: '2026-10-22', from: 'own' }, overdue: 3 })
  const late = model.placeRow(due, { id: 'x', status: 'todo', due_on: '2026-10-22' }, NOW, 'UTC')
  assert.equal(late.plan, null)
  assert.equal(late.due, Date.UTC(2026, 9, 23))
  assert.deepEqual(late.over, { from: Date.UTC(2026, 9, 23), to: NOW })
  // moai counts late days in its own zone; where the page's zone is still on the deadline day the hatch is a sliver,
  // never dropped.
  const today = g('y', { planned: { start: null, end: '2026-10-25', from: 'own' }, overdue: 1 })
  const sliver = model.placeRow(today, { id: 'y', status: 'todo', due_on: '2026-10-25' }, NOW, 'UTC')
  assert.deepEqual(sliver.over, { from: Date.UTC(2026, 9, 26), to: Date.UTC(2026, 9, 26) })
  // A row with a plan ending before its start is late from the deadline, not from the start's day.
  const backwards = g('z', { planned: own('2026-10-20', '2026-10-18'), overdue: 7 })
  assert.equal(model.placeRow(backwards, { id: 'z', status: 'todo' }, NOW, 'UTC').over.from, Date.UTC(2026, 9, 19))
})

test('a milestone that wrote no deadline has no diamond; an archived one reads its own from the plan', () => {
  // Members' days give the hatched bar; their latest deadline is not the milestone's.
  const members = g('m', { kind: 'milestone', planned: { start: '2026-10-20', end: '2026-10-26', from: 'members' } })
  assert.equal(model.placeRow(members, { id: 'm', kind: 'milestone', status: 'todo' }, NOW, 'UTC').due, null)
  // An archived row (a tree line no list printed) carries no due_on: its own plan's end is that deadline.
  const archived = g('n', { kind: 'milestone', planned: { start: null, end: '2026-10-28', from: 'own' } })
  assert.equal(model.placeRow(archived, { id: 'n', kind: 'milestone', archived: true }, NOW, 'UTC').due, Date.UTC(2026, 9, 29))
})

test('the axis: steps, the two header lines, weekends, fit', () => {
  assert.deepEqual(model.PX_PER_DAY, { hour: 480, day: 40, week: 14, month: 4 })
  const words = copyFor('en').gantt
  const axis = { zoom: 'day', from: Date.UTC(2026, 9, 3), to: Date.UTC(2026, 10, 3) }
  const head = model.headerOf(axis, NOW, words)
  assert.equal(head.top[0].text, '2026-10')
  assert.equal(head.bottom.length, 31)
  assert.equal(head.bottom[0].text, '3 Sat')
  assert.equal(head.bottom[0].cls, 'we')
  assert.equal(head.bottom.find((tick) => tick.cls === 'today').text, '25 Sun')
  assert.equal(model.weekends(axis).length, 10)
  assert.equal(model.headerOf({ ...axis, zoom: 'week' }, NOW, words).bottom[0].text, 'W40')
  assert.equal(model.headerOf({ ...axis, zoom: 'month' }, NOW, words).top[0].text, '2026')
  assert.equal(model.headerOf({ ...axis, zoom: 'hour' }, NOW, words).bottom.length, 31 * 8)
  assert.equal(model.isoWeek(Date.UTC(2026, 9, 25)), 43)
  // Fit: the finest step that shows the span whole.
  const span = [Date.UTC(2026, 9, 10), Date.UTC(2026, 10, 14)]
  assert.equal(model.zoomToFit(span, 1400), 'day')
  assert.equal(model.zoomToFit(span, 600), 'week')
  assert.equal(model.zoomToFit(span, 100), 'month')
  assert.equal(model.normalizeGrid('9000'), 760)
  assert.equal(model.normalizeGrid('nope'), 460)
  assert.equal(model.normalizeGrid(10), 220)
})

test('the grid draws the whole tree: id, glyph, branch, title, progress, assignee', () => {
  const { root } = mount()
  const rows = [...root.querySelectorAll('.g-row')]
  assert.deepEqual(rows.map((row) => row.dataset.id), ['m1', 'e1', 'e1.a', 'e1.b', 'e1.c', 'e1.d', 'e1.e'])
  const done = root.querySelector('.g-row[data-id="e1.a"]')
  assert.equal(done.querySelector('.c-id').textContent, 'e1.a')
  assert.equal(done.querySelector('.c-st').textContent, '✓')
  assert.match(done.querySelector('.c-st').className, /m-st-done/)
  assert.equal(done.querySelector('.c-who').textContent, 'kim')
  assert.ok(done.querySelector('.moai-twig'), 'the branch is drawn')
  assert.equal(root.querySelector('.g-row[data-id="m1"] .c-n').textContent, '1/5')
  assert.equal(root.querySelector('.g-row[data-id="e1.b"] .c-n').textContent, '', 'a plain row has a column glyph, not a count')
  assert.match(root.querySelector('.g-row[data-id="m1"] .c-title').textContent, /^◆ v1/)
  assert.equal(root.querySelectorAll('.lane').length, 7, 'a lane for each row')
})

test('bars: left and width are CSSOM values on the axis, classes paint', () => {
  const { root } = mount()
  // 2026-10-03 is the axis start (a week before the earliest day). e1.c plans 10-26..10-27 -> 23 days in, two days wide.
  const plan = lane(root, 'e1.c').querySelector('.b-plan')
  assert.equal(px(plan, 'left'), days('2026-10-26'))
  assert.equal(px(plan, 'width'), 80)
  assert.equal(plan.className, 'bar b-plan')
  const done = lane(root, 'e1.a')
  assert.equal(px(done.querySelector('.b-plan'), 'left'), days('2026-10-21'))
  const act = done.querySelector('.b-act')
  assert.match(act.className, /a-done/)
  assert.ok(Math.abs(px(act, 'left') - (days('2026-10-21') + (9 + 20 / 60) / 24 * 40)) < 0.01)
  assert.ok(Math.abs(px(act, 'width') - (29 + 45 / 60) / 24 * 40) < 0.01, 'to the minute')
  // An open band runs to now and carries no tip.
  const open = lane(root, 'e1.b').querySelector('.b-act')
  assert.match(open.className, /a-in_progress/)
  assert.ok(Math.abs(px(open, 'left') + px(open, 'width') - (days('2026-10-25') + 14 / 24 * 40)) < 0.01)
  assert.equal(open.textContent, '')
})

test('plans from members are hatched, a deferred row is dashed and dim, a group carries its fill', () => {
  const { root } = mount()
  assert.match(lane(root, 'm1').querySelector('.b-plan').className, /\bmembers\b/)
  assert.doesNotMatch(lane(root, 'e1.a').querySelector('.b-plan').className, /members/)
  const deferred = lane(root, 'e1.e')
  assert.match(deferred.className, /\bdim\b/)
  assert.match(deferred.querySelector('.b-plan').className, /\bdeferred\b/)
  assert.match(root.querySelector('.g-row[data-id="e1.e"]').className, /\bdim\b/)
  const fillBar = lane(root, 'e1').querySelector('.b-plan .b-fill')
  assert.equal(fillBar.style.width, '20%')
  assert.equal(lane(root, 'e1.a').querySelector('.b-fill'), null, 'a plain row has no fill')
})

test('late: a hatch from the planned end to now, and a !; the milestone deadline is a diamond', () => {
  const { root } = mount()
  const over = lane(root, 'e1.b').querySelector('.b-over')
  assert.equal(px(over, 'left'), days('2026-10-23'))
  assert.ok(Math.abs(px(over, 'width') - (2 * 24 + 14) / 24 * 40) < 0.01)
  assert.equal(lane(root, 'e1.b').querySelector('.mk.over').textContent, '!')
  assert.equal(lane(root, 'e1.a').querySelector('.b-over'), null)
  const due = lane(root, 'm1').querySelector('.mk.due')
  assert.equal(due.textContent, '◆')
  assert.equal(px(due, 'left'), days('2026-10-29'))
})

test('an unknown start draws no actual band, a deadline-only row draws a mark', () => {
  const { root } = mount()
  const old = lane(root, 'e1.d')
  assert.equal(old.querySelector('.b-act'), null)
  assert.equal(old.querySelector('.b-plan'), null)
  assert.equal(old.querySelector('.mk.due').textContent, '◆')
  assert.equal(lane(root, 'e1.c').querySelector('.b-act'), null, 'not started: no band either')
})

test('today: a dashed line at now, a cap in the header and the day in red', () => {
  const { root } = mount()
  const line = root.querySelector('.g-lanes > .todayline')
  assert.ok(Math.abs(px(line, 'left') - (days('2026-10-25') + 14 / 24 * 40)) < 0.01)
  assert.equal(px(root.querySelector('.todaycap'), 'left'), px(line, 'left'))
  assert.equal(root.querySelector('.hd2 .tick.today').textContent, '25 Sun')
  assert.ok(root.querySelector('.hd2 .tick.we'), 'weekend days are dimmed')
  assert.ok(root.querySelectorAll('.col-we').length > 0, 'weekend shading')
})

test('arrows: only the cursor row\'s blockers, in one SVG behind the bars', () => {
  const { root, at } = mount('e1.c')
  const svg = root.querySelector('svg.links')
  assert.equal(svg.querySelectorAll('path').length, 2)
  assert.equal(svg.querySelectorAll('polygon').length, 2)
  assert.ok(svg.querySelector('path').getAttribute('d').startsWith('M'))
  assert.equal(lane(root, 'e1.c').getAttribute('aria-selected'), 'true')
  // A row nothing blocks shows no arrows; the cursor moves in place.
  const lanes = [...root.querySelectorAll('.lane')]
  at('e1.a')
  assert.equal(root.querySelector('svg.links').querySelectorAll('path').length, 0)
  assert.equal(lane(root, 'e1.c').getAttribute('aria-selected'), 'false')
  assert.equal(lane(root, 'e1.a').getAttribute('aria-selected'), 'true')
  assert.equal(root.querySelector('.g-row[data-id="e1.a"]').getAttribute('aria-selected'), 'true')
  assert.deepEqual([...root.querySelectorAll('.lane')], lanes, 'no redraw for a cursor move')
  // An unknown start blocks nothing visible: arrows come from the blocker's planned end.
  at('e1.c')
  assert.equal(root.querySelector('svg.links').querySelectorAll('path').length, 2)
})

test('the bubble is the panel\'s own element, with the title, planned, actual, late days, blockers and assignee', () => {
  const { root, dom } = mount()
  const bubble = root.querySelector('.g-bubble')
  assert.equal(bubble.hidden, true)
  assert.equal(root.querySelector('[title].bar'), null, 'no title attribute on a bar')
  lane(root, 'e1.b').querySelector('.b-act').dispatchEvent(new dom.window.MouseEvent('mouseenter'))
  assert.equal(bubble.hidden, false)
  assert.match(bubble.textContent, /e1\.b\s+Late one/)
  assert.match(bubble.textContent, /Planned2026-10-19 ~ 2026-10-22/)
  assert.match(bubble.textContent, /Actual2026-10-19 09:00 ~ now/)
  assert.match(bubble.textContent, /Overdue3 days/)
  lane(root, 'e1.b').querySelector('.b-act').dispatchEvent(new dom.window.MouseEvent('mouseleave'))
  assert.equal(bubble.hidden, true)
  lane(root, 'e1.a').querySelector('.b-act').dispatchEvent(new dom.window.MouseEvent('mouseenter'))
  assert.match(bubble.textContent, /Assigneekim/)
  lane(root, 'e1.d').querySelector('.mk.due').dispatchEvent(new dom.window.MouseEvent('mouseenter'))
  assert.match(bubble.textContent, /start unknown/)
  lane(root, 'e1.c').querySelector('.b-plan').dispatchEvent(new dom.window.MouseEvent('mouseenter'))
  assert.match(bubble.textContent, /Blocked bye1\.a \(done\), e1\.b \(open\)/)
  assert.match(bubble.textContent, /not started/)
})

test('keys: j/k move the cursor, Enter opens the detail, - and = change the step, H/L move the dates', () => {
  const { root, view, calls, dom } = mount('e1.a')
  const box = scrollable(root)
  assert.equal(view.command('move.down'), true)
  assert.equal(view.command('move.up'), true)
  assert.equal(view.command('move.top'), true)
  assert.equal(view.command('move.bottom'), true)
  assert.deepEqual(calls, ['point:e1.b', 'point:e1', 'point:m1', 'point:e1.e'])
  assert.equal(view.command('enter'), true)
  assert.equal(calls.at(-1), 'choose:e1.a')
  assert.equal(view.command('expand'), false, 'not a gantt action')
  // = goes finer: the day step (40px) becomes the hour step (480px a day); - goes back.
  const planWidth = () => planWidthAt(root, box, dom)
  assert.equal(planWidth(), 80)
  view.command('gantt.zoomIn')
  assert.equal(planWidth(), 960)
  assert.equal(root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom, 'hour')
  view.command('gantt.zoomOut')
  view.command('gantt.zoomOut')
  assert.equal(planWidth(), 28)
  assert.equal(root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom, 'week')
  for (const action of ['gantt.left', 'gantt.right', 'gantt.today', 'gantt.fit']) assert.equal(view.command(action), true)
  assert.match(root.querySelector('.g-range').textContent, /^\d{4}-\d{2}-\d{2} ~ \d{4}-\d{2}-\d{2}$/)
})

test('clicking a row chooses it; the zoom buttons change the step', () => {
  const { root, calls, dom } = mount()
  root.querySelector('.g-row[data-id="e1.b"]').click()
  lane(root, 'e1.c').click()
  assert.deepEqual(calls, ['choose:e1.b', 'choose:e1.c'])
  root.querySelector('.g-seg [data-zoom="week"]').dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }))
  assert.equal(px(lane(root, 'e1.c').querySelector('.b-plan'), 'width'), 28)
})

test('the grid width is dragged, clamped, and remembered', () => {
  const first = mount()
  const inner = first.root.querySelector('.g-inner')
  assert.equal(inner.style.getPropertyValue('--grid-w'), '460px')
  const handle = first.root.querySelector('.g-split')
  const fire = (type, x) => handle.dispatchEvent(new first.dom.window.MouseEvent(type, { clientX: x, bubbles: true }))
  fire('pointerdown', 500)
  fire('pointermove', 400)
  assert.equal(inner.style.getPropertyValue('--grid-w'), '360px')
  assert.equal(first.root.querySelector('.g-left').dataset.cols, 'mid')
  fire('pointermove', -900)
  assert.equal(inner.style.getPropertyValue('--grid-w'), '220px')
  assert.equal(first.root.querySelector('.g-left').dataset.cols, 'min')
  fire('pointerup', 0)
  // The next time the tab opens it has the width it was left at.
  const again = mount()
  assert.equal(again.root.querySelector('.g-inner').style.getPropertyValue('--grid-w'), '220px')
  resetGrid(again)
  assert.equal(mount().root.querySelector('.g-inner').style.getPropertyValue('--grid-w'), '460px', 'put back for the tests after')
})

test('the grid handle drags from the width drawn, not a wider one remembered', () => {
  const view = mount()
  const box = view.root.querySelector('.g-scroll')
  const inner = view.root.querySelector('.g-inner')
  const handle = view.root.querySelector('.g-split')
  const fire = (type, x) => handle.dispatchEvent(new view.dom.window.MouseEvent(type, { clientX: x, bubbles: true }))
  try {
    // A 500px pane leaves the dates 120px: the 460 remembered is drawn 380 wide, and the handle stands there.
    Object.defineProperty(box, 'clientWidth', { get: () => 500, configurable: true })
    fire('pointerdown', 500)
    fire('pointermove', 490)
    assert.equal(inner.style.getPropertyValue('--grid-w'), '370px', 'ten pixels left of where it was drawn')
    fire('pointerup', 490)
  } finally {
    resetGrid(view)
  }
})

test('no rows: it says so; and a page with no gantt values still draws the rows', () => {
  const none = mount(null, new IssueIndex([], []))
  assert.equal(none.root.querySelector('.g-empty').hidden, false)
  const bare = mount(null, new IssueIndex(ROWS, treeOf(ROWS)))
  assert.equal(bare.root.querySelectorAll('.g-row').length, 7)
  assert.equal(bare.root.querySelector('.g-lanes .b-plan'), null)
})

// jsdom lays nothing out, so scrollLeft stands still; give the scroll box one that remembers what it is set to.
const scrollable = (root) => {
  const box = root.querySelector('.g-scroll')
  let left = 0
  Object.defineProperty(box, 'scrollLeft', { get: () => left, set: (v) => { left = Math.max(0, v) }, configurable: true })
  return box
}

/**
 * e1.c's planned bar width, with the dates scrolled to it and back: at the hour step a month is thousands of pixels,
 * and only the bars near the window are nodes (moai-vj8w.2fh).
 */
const planWidthAt = (root, box, dom) => {
  const keep = box.scrollLeft
  const ppd = { month: 4, week: 14, day: 40, hour: 480 }[root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom]
  const scrollTo = (x) => {
    box.scrollLeft = x
    box.dispatchEvent(new dom.window.Event('scroll'))
  }
  scrollTo(days('2026-10-26') / 40 * ppd - 50)
  const width = px(lane(root, 'e1.c').querySelector('.b-plan'), 'width')
  scrollTo(keep)
  return width
}

test('mouse: a click on a bar or a row chooses it, a bucket row only moves the cursor', () => {
  const { root, calls } = mount()
  lane(root, 'e1.a').querySelector('.b-act').click()
  root.querySelector('.g-row[data-id="e1.b"]').click()
  assert.deepEqual(calls, ['choose:e1.a', 'choose:e1.b'])
  const rows = [{ id: 'm1', kind: 'milestone', title: 'v1', status: 'todo' }, { id: 'z', kind: 'issue', title: 'Loose', status: 'todo' }]
  const loose = mount(null, new IssueIndex(rows, treeOf(rows)))
  loose.root.querySelector('.g-row:not([data-id])').click()
  assert.deepEqual(loose.calls, ['point:m:'])
})

test('mouse: dragging the date header moves the dates; a cancel, a lost capture or the right button do not leave it stuck', () => {
  const { root, dom } = mount()
  const box = scrollable(root)
  const axis = root.querySelector('.g-right .g-head')
  const fire = (type, x, init = {}) => axis.dispatchEvent(new dom.window.MouseEvent(type, { clientX: x, bubbles: true, cancelable: true, ...init }))
  box.scrollLeft = 300
  fire('pointerdown', 500)
  fire('pointermove', 450)
  assert.equal(box.scrollLeft, 350, 'dragging left shows later dates')
  fire('pointermove', 600)
  assert.equal(box.scrollLeft, 200)
  fire('pointercancel', 0)
  fire('pointermove', 0)
  assert.equal(box.scrollLeft, 200, 'a cancelled drag is over')
  assert.equal(axis.classList.contains('drag'), false)
  fire('pointerdown', 500)
  fire('lostpointercapture', 0)
  fire('pointermove', 400)
  assert.equal(box.scrollLeft, 200, 'a lost capture ends it too')
  fire('pointerdown', 500, { button: 2 })
  fire('pointermove', 400)
  assert.equal(box.scrollLeft, 200, 'a right click does not start a drag')
  fire('pointerdown', 500)
  fire('pointermove', 300)
  fire('pointerup', 300)
  assert.equal(box.scrollLeft, 400)
})

test('mouse: Shift+wheel moves the dates, a plain wheel is left to the browser', () => {
  const { root, dom } = mount()
  const box = scrollable(root)
  const dates = root.querySelector('.g-right')
  const wheel = (init) => {
    const event = new dom.window.WheelEvent('wheel', { bubbles: true, cancelable: true, ...init })
    dates.dispatchEvent(event)
    return event
  }
  assert.equal(wheel({ deltaY: 120 }).defaultPrevented, false)
  assert.equal(box.scrollLeft, 0)
  assert.equal(wheel({ deltaY: 120, shiftKey: true }).defaultPrevented, true)
  assert.equal(box.scrollLeft, 120)
  assert.equal(wheel({ deltaX: 80 }).defaultPrevented, false, 'a sideways wheel scrolls the box natively')
})

test('mouse: Ctrl+wheel steps the zoom over the dates only, keeping the date under the pointer', () => {
  const { root, dom } = mount()
  const box = scrollable(root)
  const wheel = (target, init) => {
    const event = new dom.window.WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, ...init })
    target.dispatchEvent(event)
    return event
  }
  const planWidth = () => planWidthAt(root, box, dom)
  const grid = Number.parseFloat(root.querySelector('.g-inner').style.getPropertyValue('--grid-w'))
  // Over the grid the browser keeps its zoom.
  assert.equal(wheel(root.querySelector('.g-left'), { deltaY: -100 }).defaultPrevented, false)
  assert.equal(planWidth(), 80)
  // Over the dates: 100px in is 2.5 days after the axis start; one step finer is 480px a day, and the date stays at 100px (the grid's width is added to the pointer).
  const event = wheel(root.querySelector('.g-right'), { deltaY: -100, clientX: grid + 100 })
  assert.equal(event.defaultPrevented, true)
  assert.equal(planWidth(), 960)
  assert.equal(box.scrollLeft, 2.5 * 480 - 100)
  wheel(root.querySelector('.g-right'), { deltaY: 100, clientX: grid + 100 })
  assert.equal(planWidth(), 80)
  assert.equal(box.scrollLeft, 0)
  // At the coarsest step there is nothing more to take, and the page's zoom is still not taken by the browser.
  wheel(root.querySelector('.g-right'), { deltaY: 100, clientX: grid + 100 })
  wheel(root.querySelector('.g-right'), { deltaY: 100, clientX: grid + 100 })
  assert.equal(wheel(root.querySelector('.g-right'), { deltaY: 100, clientX: grid + 100 }).defaultPrevented, true)
  assert.equal(root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom, 'month')
})

test('mouse: a sideways Ctrl+wheel keeps the step (and still keeps the page\'s zoom from the browser)', () => {
  const { root, dom } = mount()
  scrollable(root)
  const event = new dom.window.WheelEvent('wheel', { bubbles: true, cancelable: true, ctrlKey: true, deltaX: 40, deltaY: 0 })
  root.querySelector('.g-right').dispatchEvent(event)
  assert.equal(event.defaultPrevented, true)
  assert.equal(root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom, 'day')
})

test('a new zone (SPC o t) places the bars again, with no new index to say so', () => {
  const { root, at } = mount()
  const left = () => px(lane(root, 'e1.a').querySelector('.b-act'), 'left')
  const before = left()
  try {
    clock.zone = () => 'Asia/Seoul'
    at(null)
    assert.ok(Math.abs(left() - before - (9 / 24) * 40) < 0.01, 'nine hours later on the axis')
  } finally {
    clock.zone = () => 'UTC'
  }
})

test('a redraw under a hovered bar takes the bubble down', () => {
  const { root, view, dom } = mount()
  const bubble = root.querySelector('.g-bubble')
  lane(root, 'e1.b').querySelector('.b-act').dispatchEvent(new dom.window.MouseEvent('mouseenter'))
  assert.equal(bubble.hidden, false)
  view.command('gantt.zoomIn')
  assert.equal(bubble.hidden, true, 'the bar it spoke of is gone')
})

test('a read that reaches an earlier day grows the axis at its start, and the dates in view stay in view', () => {
  const { root, view } = mount()
  const box = scrollable(root)
  box.scrollLeft = 400
  const shown = model.wallAt({ zoom: 'day', from: Date.UTC(2026, 9, 3) }, 400)
  // e1.e now plans from 10-01: the axis starts a week before, on 09-24, nine days earlier than it did.
  const earlier = { ...GANTT, 'e1.e': { ...GANTT['e1.e'], planned: own('2026-10-01', '2026-10-02') } }
  const tree = treeOf(ROWS).map((line) => (line.id ? { ...line, gantt: earlier[line.id] } : line))
  view.render({ ...initialState(), tab: 'gantt', cursor: null }, new IssueIndex(ROWS, tree))
  assert.equal(px(lane(root, 'e1.c').querySelector('.b-plan'), 'left'), days('2026-10-26') + 9 * 40)
  assert.equal(box.scrollLeft, 400 + 9 * 40)
  assert.equal(model.wallAt({ zoom: 'day', from: Date.UTC(2026, 8, 24) }, box.scrollLeft), shown)
})

// ── moving a plan (moai-d86e.3ud) ──

test('the plan write carries the move, not the days: moai measures them inside its lock (moai-vj8w.02c)', async () => {
  const { planEndpoint } = await panelModule('data/plan.ts')
  assert.deepEqual(planEndpoint('e1.c', { edge: 'body', days: -2 }), { path: '/api/plan/e1.c', method: 'POST', query: { edge: 'body', days: '-2' } })
  assert.deepEqual(planEndpoint('e1.c', { edge: 'end', days: 3 }).query, { edge: 'end', days: '3' })
  assert.deepEqual(planEndpoint('z', { day: '2026-10-30' }).query, { day: '2026-10-30' })
})

test('shiftPlan is Issue::shifted: the changing side only, own days only', () => {
  const { shiftPlan, dragDays, movedPlan } = model
  assert.deepEqual(shiftPlan(own('2026-10-26', '2026-10-27'), 'body', 2), { start: '2026-10-28', due: '2026-10-29' })
  assert.deepEqual(shiftPlan(own('2026-10-26', '2026-10-27'), 'start', -1), { start: '2026-10-25' })
  assert.deepEqual(shiftPlan(own('2026-10-26', '2026-10-27'), 'end', 3), { due: '2026-10-30' })
  // A deadline alone: the body moves it, the start sets out from it; a start alone: the end sets out from it.
  assert.deepEqual(shiftPlan(own(null, '2026-10-27'), 'body', 1), { due: '2026-10-28' })
  assert.deepEqual(shiftPlan(own(null, '2026-10-27'), 'start', -2), { start: '2026-10-25' })
  assert.deepEqual(shiftPlan(own('2026-10-26', null), 'end', 1), { due: '2026-10-27' })
  // Across a month end and a year end, in calendar days.
  assert.deepEqual(shiftPlan(own('2026-12-31', null), 'body', 1), { start: '2027-01-01' })
  assert.deepEqual(shiftPlan({ start: '2026-10-20', end: '2026-10-26', from: 'members' }, 'body', 1), { why: 'members' })
  assert.deepEqual(shiftPlan(null, 'body', 1), { why: 'none' })
  assert.deepEqual(shiftPlan(own('2026-10-26', '2026-10-27'), 'body', 0), { why: 'still' })
  // Whole days at every step: half a day and more rounds to the next.
  assert.equal(dragDays('day', 19), 0)
  assert.equal(dragDays('day', 21), 1)
  assert.equal(dragDays('day', -61), -2)
  assert.equal(dragDays('month', 9), 2)
  assert.equal(dragDays('hour', 239), 0)
  assert.equal(dragDays('day', -1), 0)
  assert.ok(!Object.is(dragDays('day', -1), -0), 'no -0: it reads as a move')
  // The bar a move draws; a start past the deadline draws none (the write refuses it).
  const D = 864e5
  assert.deepEqual(movedPlan(own('2026-10-26', '2026-10-27'), { start: '2026-10-27' }), [Date.UTC(2026, 9, 27), Date.UTC(2026, 9, 27) + D])
  assert.equal(movedPlan(own('2026-10-26', '2026-10-27'), { start: '2026-10-28' }), null)
  assert.deepEqual(movedPlan(own(null, '2026-10-27'), { due: '2026-10-28' }), [Date.UTC(2026, 9, 28), Date.UTC(2026, 9, 28) + D])
})

/** The index the gantt draws `rows` from, each tree line carrying its `gantt`. */
const indexOf = (rows = ROWS, gantt = GANTT) => new IssueIndex(rows, treeOf(rows).map((line) => (line.id ? { ...line, gantt: gantt[line.id] } : line)))
/** One turn of the event loop: a write answered, a redraw the view put off. */
const tick = () => new Promise((resolve) => setTimeout(resolve, 0))
/**
 * A mount that writes: `moves` holds what movePlan was handed, `said` what the view said. `answer` stands for the
 * write: by default it is answered at once.
 */
const writing = (rows = ROWS, gantt = GANTT, answer = async () => {}) => {
  const dom = new JSDOM('<!doctype html><body><div id="r"></div></body>')
  const calls = []
  const moves = []
  const said = []
  const root = dom.window.document.getElementById('r')
  const ctx = {
    document: dom.window.document, copy: copyFor('en'), language: 'en',
    intents: { choose: (id) => calls.push(`choose:${id}`), point: (id) => calls.push(`point:${id}`) },
    layout: () => ({ detailVisible: false }), focusPane: () => {},
    say: (text, problem) => said.push([text, problem]),
    movePlan: (id, change, undo) => {
      moves.push({ id, change, undo })
      return answer()
    },
  }
  const view = mountGantt(root, ctx)
  size(root)
  const state = { ...initialState(), tab: 'gantt', cursor: null }
  view.render(state, indexOf(rows, gantt))
  return { root, dom, calls, moves, said, view, state }
}
/** jsdom lays nothing out: a bar's box is read from the left and width the view gave it, the lane standing at x = 0. */
const laidOut = (node) => {
  node.getBoundingClientRect = () => {
    const left = px(node, 'left')
    const width = node.classList.contains('mk') ? 10 : px(node, 'width')
    return { left, right: left + width, width, top: 0, bottom: 28, height: 28 }
  }
  return node
}
const press = (dom, node, type, x, init = {}) => node.dispatchEvent(new dom.window.MouseEvent(type, { clientX: x, bubbles: true, cancelable: true, ...init }))

test('dragging a bar\'s middle moves the plan by whole days, once, on release; the release is no choice', async () => {
  const rows = ROWS.map((row) => (row.id === 'e1.c' ? { ...row, starts_on: '2026-10-26', due_on: '2026-10-27' } : row))
  const { root, dom, calls, moves } = writing(rows)
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  assert.ok(bar.classList.contains('movable'))
  const x0 = px(bar, 'left')
  const mid = x0 + 40
  press(dom, l, 'pointerdown', mid)
  press(dom, l, 'pointermove', mid + 70)
  assert.equal(px(bar, 'left'), x0 + 80, 'the bar snaps to the nearest day while it moves')
  assert.deepEqual(moves, [], 'nothing is written while it moves')
  press(dom, l, 'pointerup', mid + 70)
  l.click()
  assert.deepEqual(moves, [{ id: 'e1.c', change: { edge: 'body', days: 2 }, undo: 'moai edit e1.c --start 2026-10-26 --due 2026-10-27' }])
  assert.deepEqual(calls, [], 'the click a drag ends with chose the row')
  l.click()
  assert.deepEqual(calls, ['choose:e1.c'], 'the next click chooses as before')
})

test('the ends move the start or the deadline alone; a press that does not move writes nothing', async () => {
  const { root, dom, moves } = writing()
  // Each drag waits for the write before it to be answered: the bar is drawn again from the rows then.
  const drag = async (from, to) => {
    const l = lane(root, 'e1.c')
    const bar = laidOut(l.querySelector('.b-plan'))
    const [x0, w] = [px(bar, 'left'), px(bar, 'width')]
    press(dom, l, 'pointerdown', from(x0, w))
    press(dom, l, 'pointerup', to(x0, w))
    await tick()
  }
  await drag((x0) => x0 + 1, (x0) => x0 - 39)
  await drag((x0, w) => x0 + w - 1, (x0, w) => x0 + w + 41)
  await drag((x0) => x0 + 40, (x0) => x0 + 45)
  // A press outside the bar is the lane's: a choice, never a move.
  await drag((x0, w) => x0 + w + 200, (x0, w) => x0 + w + 400)
  assert.deepEqual(moves.map((m) => m.change), [{ edge: 'start', days: -1 }, { edge: 'end', days: 1 }])
})

test('a read that lands while a bar is dragged waits for the release, and the drag goes on', async () => {
  const { root, dom, moves, view, state } = writing()
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  // Another session wrote: the rows come back with a new index while the bar is held.
  const later = ROWS.map((row) => (row.id === 'e1.a' ? { ...row, title: 'Done one, renamed' } : row))
  view.render(state, indexOf(later))
  assert.equal(lane(root, 'e1.c'), l, 'the lane under the pointer was rebuilt mid-drag')
  press(dom, l, 'pointermove', x0 + 120)
  assert.equal(px(bar, 'left'), x0 + 80)
  press(dom, l, 'pointerup', x0 + 120)
  assert.deepEqual(moves.map((m) => m.change), [{ edge: 'body', days: 2 }])
  await tick()
  assert.notEqual(lane(root, 'e1.c'), l, 'the read that waited is drawn after the release')
  assert.match(root.querySelector('.g-row[data-id="e1.a"]').textContent, /renamed/)
})

test('a row is moved again only once its last move is answered; a plain press meanwhile still chooses', async () => {
  let answer
  const { root, dom, moves, calls } = writing(ROWS, GANTT, () => new Promise((resolve) => (answer = resolve)))
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointerup', x0 + 80)
  // The bar lies where it was dropped, drawn from the read before the write: a move from there is not taken.
  press(dom, l, 'pointerdown', px(bar, 'left') + 40)
  press(dom, l, 'pointerup', px(bar, 'left') + 80)
  l.click()
  assert.equal(moves.length, 1)
  assert.deepEqual(calls, ['choose:e1.c'])
  answer()
  await tick()
  const again = lane(root, 'e1.c')
  const fresh = laidOut(again.querySelector('.b-plan'))
  press(dom, again, 'pointerdown', px(fresh, 'left') + 40)
  press(dom, again, 'pointerup', px(fresh, 'left') + 80)
  assert.equal(moves.length, 2)
})

test('a drag that ends without a click does not take the next press\'s click', () => {
  const { root, dom, moves, calls } = writing()
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  const x0 = px(bar, 'left')
  // A touch drag: the browser sends no click after it.
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointerup', x0 + 80)
  assert.equal(moves.length, 1)
  const other = lane(root, 'e1.a')
  press(dom, other, 'pointerdown', 5)
  press(dom, other, 'pointerup', 5)
  other.click()
  assert.deepEqual(calls, ['choose:e1.a'])
})

test('a milestone\'s own diamond moves with its bar\'s end; the undo names the days the bar was drawn from', () => {
  const rows = ROWS.map((row) => (row.id === 'm1' ? { ...row, starts_on: '2026-10-26' } : row))
  const gantt = { ...GANTT, m1: { ...GANTT.m1, planned: own('2026-10-26', '2026-10-28') } }
  const { root, dom, moves } = writing(rows, gantt)
  const l = lane(root, 'm1')
  const bar = laidOut(l.querySelector('.b-plan'))
  const due = l.querySelector('.mk.due')
  const [x0, d0] = [px(bar, 'left'), px(due, 'left')]
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointermove', x0 + 80)
  assert.deepEqual([px(bar, 'left'), px(due, 'left')], [x0 + 40, d0 + 40])
  press(dom, l, 'pointercancel', x0 + 80)
  assert.deepEqual([px(bar, 'left'), px(due, 'left')], [x0, d0], 'a cancel puts both back')
  // e1.c: the list printed no days for it, the tree drew 10-26 ~ 10-27 — the undo puts back what was drawn.
  const c = lane(root, 'e1.c')
  const cb = laidOut(c.querySelector('.b-plan'))
  press(dom, c, 'pointerdown', px(cb, 'left') + 40)
  press(dom, c, 'pointerup', px(cb, 'left') + 80)
  assert.equal(moves.at(-1).undo, 'moai edit e1.c --start 2026-10-26 --due 2026-10-27')
})

test('a deadline-only row drags its diamond; a group counted from its members says why and moves nothing', () => {
  const { root, dom, moves, said } = writing()
  const l = lane(root, 'e1.d')
  const due = laidOut(l.querySelector('.mk.due'))
  const x = px(due, 'left')
  press(dom, l, 'pointerdown', x + 2)
  press(dom, l, 'pointerup', x + 42)
  assert.deepEqual(moves.map((m) => m.change), [{ edge: 'body', days: 1 }], 'a diamond drags as the whole plan: its deadline')
  const e = lane(root, 'e1')
  const hatched = laidOut(e.querySelector('.b-plan'))
  assert.equal(hatched.classList.contains('movable'), false)
  const h = px(hatched, 'left') + 100
  press(dom, e, 'pointerdown', h)
  press(dom, e, 'pointermove', h + 80)
  assert.equal(px(hatched, 'left') + 100, h, 'a members bar is not drawn moving')
  press(dom, e, 'pointerup', h + 80)
  assert.equal(moves.length, 1)
  assert.deepEqual(said, [['e1 wrote no days of its own — its bar spans its members, move them instead', true]])
})

test('a double click on a day of a row with no days plans that day; a group or a row with days does not', () => {
  const rows = [...ROWS, { id: 'z', kind: 'issue', title: 'Loose', status: 'todo' }, { id: 'y', kind: 'issue', title: 'Odd', status: 'todo', due_on: 'soon' }]
  const gantt = { ...GANTT, z: g('z'), y: g('y') }
  const { root, dom, moves } = writing(rows, gantt)
  const day = days('2026-10-30') + 5
  for (const id of ['z', 'y', 'e1.c']) press(dom, lane(root, id), 'dblclick', day)
  assert.deepEqual(moves, [{ id: 'z', change: { day: '2026-10-30' }, undo: 'moai edit z --start none --due none' }])
})

test('a row only a sibling worktree shows is not drawn movable, and a double click plans nothing (moai-vj8w.93u)', () => {
  const rows = [
    ...ROWS.map((row) => (row.id === 'e1.c' ? { ...row, starts_on: '2026-10-26', due_on: '2026-10-27', branch: 'worktree-moai-e1c' } : row)),
    { id: 'z', kind: 'issue', title: 'Loose', status: 'todo', branch: 'worktree-moai-z' },
  ]
  const { root, dom, moves, said, calls } = writing(rows, { ...GANTT, z: g('z') })
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  assert.equal(bar.classList.contains('movable'), false)
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointermove', x0 + 120)
  assert.equal(px(bar, 'left'), x0, 'the bar is not drawn moving')
  press(dom, l, 'pointerup', x0 + 120)
  press(dom, lane(root, 'z'), 'dblclick', days('2026-10-30') + 5)
  assert.deepEqual([moves, said], [[], []])
  // A row the main checkout holds still moves.
  const other = lane(root, 'e1.b')
  assert.equal(laidOut(other.querySelector('.b-plan')).classList.contains('movable'), true)
  l.click()
  assert.deepEqual(calls, ['choose:e1.c'], 'it still chooses')
})

test('a diamond the plan has no day for is not grabbed: there is nothing to move (moai-vj8w.93u)', () => {
  const rows = ROWS.map((row) => (row.id === 'm1' ? { ...row, due_on: '2026-10-28' } : row))
  const gantt = { ...GANTT, m1: { ...GANTT.m1, planned: { start: null, end: null, from: 'own' } } }
  const { root, dom, moves, said } = writing(rows, gantt)
  const l = lane(root, 'm1')
  const due = laidOut(l.querySelector('.mk.due'))
  assert.ok(due, 'the diamond is drawn')
  assert.equal(due.classList.contains('movable'), false)
  const x = px(due, 'left')
  press(dom, l, 'pointerdown', x + 2)
  press(dom, l, 'pointerup', x + 42)
  assert.deepEqual([moves, said], [[], []], 'no write, and no "double-click a day" refusal')
})

test('an archived row is not drawn movable either, and a double click plans nothing: moai edit finds live rows only (moai-vj8w review)', () => {
  const rows = [
    ...ROWS.map((row) => (row.id === 'e1.c' ? { ...row, starts_on: '2026-10-26', due_on: '2026-10-27', archived: true } : row)),
    { id: 'z', kind: 'issue', title: 'Loose', status: 'todo', archived: true },
  ]
  const gantt = { ...GANTT, z: g('z') }
  const { root, dom, moves, said, view, state } = writing(rows, gantt)
  // Archived rows are drawn while `SPC v o` shows the archive.
  view.render({ ...state, view: { ...state.view, showArchived: true } }, indexOf(rows, gantt))
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  assert.equal(bar.classList.contains('movable'), false)
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointermove', x0 + 120)
  assert.equal(px(bar, 'left'), x0, 'the bar is not drawn moving')
  press(dom, l, 'pointerup', x0 + 120)
  press(dom, lane(root, 'z'), 'dblclick', days('2026-10-30') + 5)
  assert.deepEqual([moves, said], [[], []])
  assert.equal(laidOut(lane(root, 'e1.b').querySelector('.b-plan')).classList.contains('movable'), true, 'a live row still moves')
})

test('a scroll while a bar is held leaves the dropped bar where it was dropped (moai-vj8w review)', async () => {
  const rows = [
    ...ROWS.map((row) => (row.id === 'e1.c' ? { ...row, starts_on: '2026-10-26', due_on: '2026-10-27' } : row)),
    ...Array.from({ length: 80 }, (_, i) => ({ id: `r${String(i).padStart(2, '0')}`, kind: 'issue', title: `Row ${i}`, status: 'todo' })),
  ]
  const gantt = { ...GANTT, ...Object.fromEntries(rows.filter((row) => row.id.startsWith('r')).map((row) => [row.id, g(row.id)])) }
  // The write stays unanswered: the bar must keep the days it was dropped on meanwhile.
  const { root, dom, moves } = writing(rows, gantt, () => new Promise(() => {}))
  const box = root.querySelector('.g-scroll')
  Object.defineProperty(box, 'clientHeight', { get: () => 300, configurable: true })
  let top = 0
  Object.defineProperty(box, 'scrollTop', { get: () => top, set: (v) => { top = Math.max(0, v) }, configurable: true })
  const scrolled = () => box.dispatchEvent(new dom.window.Event('scroll'))
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointermove', x0 + 120)
  // Far down past the window and back up, the button still held.
  top = 200 * 28
  scrolled()
  top = 0
  scrolled()
  press(dom, l, 'pointerup', x0 + 120)
  assert.deepEqual(moves.map((m) => m.change), [{ edge: 'body', days: 2 }])
  await tick()
  assert.equal(lane(root, 'e1.c'), l, 'the lane was rebuilt though the view came back inside the window')
  assert.equal(px(bar, 'left'), x0 + 80, 'the dropped bar went back to its old days while its write ran')
})

test('a window built later keeps the clock the bars were placed at: the today line stays where an open bar ends (moai-vj8w review)', () => {
  const rows = Array.from({ length: 200 }, (_, i) => ({ id: `r${String(i).padStart(3, '0')}`, kind: 'issue', title: `Row ${i}`, status: i === 0 ? 'in_progress' : 'todo' }))
  const gantt = Object.fromEntries(rows.map((row) => [row.id, g(row.id)]))
  gantt.r000 = g('r000', { actual: span('2026-10-24T09:00:00Z'), progress: { kind: 'column', name: 'in_progress' } })
  const { root, dom } = mount(null, indexOf(rows, gantt), [1600, 560])
  const box = root.querySelector('.g-scroll')
  let top = 0
  Object.defineProperty(box, 'scrollTop', { get: () => top, set: (v) => { top = Math.max(0, v) }, configurable: true })
  const end = () => {
    const b = lane(root, 'r000').querySelector('.b-act')
    return px(b, 'left') + px(b, 'width')
  }
  const today = () => px(root.querySelector('.todayline'), 'left')
  const meet = () => Math.abs(end() - today()) < 0.01
  assert.ok(meet(), `the open bar ends on the today line: ${end()} ${today()}`)
  const before = clock.now
  try {
    // Six hours pass with no read (rows are read again only when the tracker changes); a scroll builds new windows.
    clock.now = () => NOW + 6 * 3600e3
    top = 150 * 28
    box.dispatchEvent(new dom.window.Event('scroll'))
    top = 0
    box.dispatchEvent(new dom.window.Event('scroll'))
    assert.ok(meet(), `the today line moved away from the bars it was drawn with: ${end()} ${today()}`)
  } finally {
    clock.now = before
  }
})

test('the window\'s options say where they stand in the whole list (moai-vj8w review)', () => {
  const rows = Array.from({ length: 400 }, (_, i) => ({ id: `r${String(i).padStart(3, '0')}`, kind: 'issue', title: `Row ${i}`, status: 'todo' }))
  const gantt = Object.fromEntries(rows.map((row) => [row.id, g(row.id)]))
  const { root, dom } = mount(null, indexOf(rows, gantt), [1000, 560])
  const box = root.querySelector('.g-scroll')
  let top = 0
  Object.defineProperty(box, 'scrollTop', { get: () => top, set: (v) => { top = Math.max(0, v) }, configurable: true })
  top = 300 * 28 + 44
  box.dispatchEvent(new dom.window.Event('scroll'))
  const option = root.querySelector('.g-row[data-id="r305"]')
  assert.equal(option.getAttribute('role'), 'option')
  assert.equal(option.getAttribute('aria-setsize'), '400')
  assert.equal(option.getAttribute('aria-posinset'), '306')
})

test('without a writer the bars do not move', () => {
  const { root, dom, calls } = mount()
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  assert.equal(bar.classList.contains('movable'), false)
  const x0 = px(bar, 'left')
  press(dom, l, 'pointerdown', x0 + 40)
  press(dom, l, 'pointermove', x0 + 120)
  assert.equal(px(bar, 'left'), x0)
  press(dom, l, 'pointerup', x0 + 120)
  l.click()
  assert.deepEqual(calls, ['choose:e1.c'])
})

test('a cancelled or lost drag puts the bar back and writes nothing', () => {
  const { root, dom, moves } = writing()
  const l = lane(root, 'e1.c')
  const bar = laidOut(l.querySelector('.b-plan'))
  const x0 = px(bar, 'left')
  for (const end of ['pointercancel', 'lostpointercapture']) {
    press(dom, l, 'pointerdown', x0 + 40)
    press(dom, l, 'pointermove', x0 + 160)
    press(dom, l, end, x0 + 160)
    assert.equal(px(bar, 'left'), x0, end)
  }
  press(dom, l, 'pointerdown', x0 + 40, { button: 2 })
  press(dom, l, 'pointerup', x0 + 160)
  assert.deepEqual(moves, [])
})

test('only the window is drawn: a long list and a multi-year axis at the hour step stay bounded, and a scroll brings the rest in (moai-vj8w.2fh)', () => {
  const rows = Array.from({ length: 400 }, (_, i) => ({ id: `r${String(i).padStart(3, '0')}`, kind: 'issue', title: `Row ${i}`, status: 'todo' }))
  const gantt = Object.fromEntries(rows.map((row) => [row.id, g(row.id)]))
  gantt.r000 = g('r000', { planned: own('2024-01-01', '2024-01-02') })
  gantt.r399 = g('r399', { planned: own('2027-06-01', '2027-06-02') })
  const { root, dom, view } = mount(null, indexOf(rows, gantt), [1000, 560])
  const box = scrollable(root)
  let top = 0
  Object.defineProperty(box, 'scrollTop', { get: () => top, set: (v) => { top = Math.max(0, v) }, configurable: true })
  const scrolled = () => box.dispatchEvent(new dom.window.Event('scroll'))
  const count = (selector) => root.querySelectorAll(selector).length

  // The list: a screen of rows and the margin, not four hundred.
  assert.ok(count('.lane') > 0 && count('.lane') <= 60, `lanes: ${count('.lane')}`)
  assert.equal(count('.g-row'), count('.lane'))
  assert.ok(lane(root, 'r000') && !lane(root, 'r300'))
  assert.equal(root.querySelector('.g-rows').style.height, `${400 * 28}px`, 'the box keeps the whole height, so the scrollbar is true')
  assert.equal(lane(root, 'r000').style.top, '0px')

  // The axis at the hour step: three and a half years is ten thousand header nodes unwindowed.
  view.command('gantt.zoomIn')
  assert.equal(root.querySelector('.g-seg [aria-pressed="true"]').dataset.zoom, 'hour')
  assert.ok(count('.tick') > 0 && count('.tick') < 150, `ticks: ${count('.tick')}`)
  assert.ok(count('.col-we') <= 4, `weekend blocks: ${count('.col-we')}`)

  // Scrolling down brings the rows in and lets the first go.
  top = 300 * 28 + 44
  scrolled()
  assert.ok(lane(root, 'r300') && !lane(root, 'r000'))
  assert.equal(lane(root, 'r300').style.top, `${300 * 28}px`)
  assert.equal(root.querySelector(`.g-row[data-id="r300"]`).style.top, `${300 * 28}px`)
  assert.ok(count('.lane') <= 60)

  // Scrolling sideways draws the header and the bars of those dates only.
  top = 399 * 28 - 400
  box.scrollLeft = (Date.UTC(2027, 5, 1) - Date.UTC(2023, 11, 25)) / 864e5 * 480 - 100
  scrolled()
  const bar = lane(root, 'r399').querySelector('.b-plan')
  assert.ok(bar, 'the bar of the dates in view is drawn')
  assert.ok(count('.tick') < 150, `ticks: ${count('.tick')}`)
  const lefts = [...root.querySelectorAll('.hd2 .tick')].map((node) => px(node, 'left'))
  assert.ok(lefts.every((x) => x > box.scrollLeft - 3000 && x < box.scrollLeft + 4000), 'no tick far from the view')
  box.scrollLeft = 0
  scrolled()
  assert.equal(lane(root, 'r399').querySelector('.b-plan'), null, 'a bar far from the view is not a node')
  assert.ok(lane(root, 'r399').querySelector('.mk.clip:not([hidden])'), 'its mark says it lies off to the side')
})

test('a cursor sent to a row beyond the window brings that row in (moai-vj8w.2fh)', () => {
  const rows = Array.from({ length: 200 }, (_, i) => ({ id: `r${String(i).padStart(3, '0')}`, kind: 'issue', title: `Row ${i}`, status: 'todo' }))
  const gantt = Object.fromEntries(rows.map((row) => [row.id, g(row.id)]))
  const { root, at, dom } = mount(null, indexOf(rows, gantt), [1000, 560])
  const box = scrollable(root)
  let top = 0
  Object.defineProperty(box, 'scrollTop', { get: () => top, set: (v) => { top = Math.max(0, v) }, configurable: true })
  assert.equal(lane(root, 'r150'), null)
  at('r150')
  assert.ok(top > 100 * 28, 'the pane scrolled to it')
  assert.equal(lane(root, 'r150').getAttribute('aria-selected'), 'true')
  assert.equal(dom.window.document.querySelectorAll('.lane[aria-selected="true"]').length, 1)
})
