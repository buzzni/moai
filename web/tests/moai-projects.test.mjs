// The registered projects (moai-o14f): every call names the project the page reads, the header lists the projects as
// the TUI's header does, and switching is a new page.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { createClient, projectOf, withProject } = await panelModule('data/client.ts')
const { baseName, entryOf, headerProjects, numberedNames, projectHref, projectsFrom, samePath } = await panelModule('data/projects.ts')
const { loadBoot, loadDetail, loadProjects, loadTree } = await panelModule('data/source.ts')
const { readMarks } = await panelModule('data/read.ts')
const { createStore } = await panelModule('store.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const { copyFor } = await panelModule('i18n/index.ts')
const { mountShell } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')
const { wireKeys } = await panelModule('keys/wiring.ts')
const { createSortDefault } = await panelModule('data/memory.ts')

const en = copyFor('en')
const envelope = (out) => JSON.stringify({ exit: 0, out, said: [] })

/** A fetch that records every call and answers `answer(url, init)` (an envelope of `{}` when left out). */
function recorder(answer = () => envelope({})) {
  const calls = []
  const fetcher = async (url, init) => {
    calls.push({ url, method: init.method ?? 'GET' })
    return { status: 200, text: async () => answer(url, init) }
  }
  return { calls, fetcher }
}

const PROJECTS = {
  config: '/home/u/.config/moai/config.toml',
  projects: [
    { name: 'argos', path: '/w/argos', state: 'initialized', counts: { todo: 1 }, unreadable: 0 },
    { name: 'moa\nissue', path: '/w/moa-issue', state: 'initialized', counts: {}, unreadable: 0 },
    { name: '', path: '/w/nameless/', state: 'initialized', counts: {}, unreadable: 0 },
    { path: 7 },
  ],
  problems: [],
}

test('the page reads ?project=, and every call it makes names it — reads, writes, the boot', async () => {
  assert.equal(projectOf('?project=%2Fw%2Fargos'), '/w/argos')
  assert.equal(projectOf('?x=1&project=/w/a%20b'), '/w/a b')
  assert.equal(projectOf(''), null)
  assert.equal(projectOf('?project='), null)
  const { calls, fetcher } = recorder((url) => (url.startsWith('/api/boot') ? '{}' : envelope({ read: [] })))
  const named = withProject(fetcher, '/w/a b&c')
  const client = createClient(named, { concurrency: 3 })
  await loadTree(client, { sort: 'priority' })
  await loadDetail(client, 'moai-1abc')
  await loadProjects(client)
  await readMarks(client, async () => {}).all()
  await loadBoot(named)
  const project = 'project=%2Fw%2Fa%20b%26c'
  assert.deepEqual(calls.map((c) => [c.method, c.url]), [
    ['GET', `/api/tree?sort=priority&${project}`],
    ['GET', `/api/issue/moai-1abc?${project}`],
    ['GET', `/api/projects?${project}`],
    ['POST', `/api/read-all?${project}`],
    ['GET', `/api/boot?${project}`],
  ])
  // The home names none: the fetch is handed back as it was.
  assert.equal(withProject(fetcher, null), fetcher)
})

test('project ls --json becomes the projects: a name on one line, the directory when it has none', () => {
  assert.deepEqual(projectsFrom(PROJECTS), [
    { name: 'argos', path: '/w/argos' },
    { name: 'moa issue', path: '/w/moa-issue' },
    { name: 'nameless', path: '/w/nameless/' },
  ])
  for (const junk of [null, [], { projects: 'x' }, 'argos']) assert.deepEqual(projectsFrom(junk), [])
  assert.equal(baseName('/w/x/'), 'x')
  assert.ok(samePath('/w/x/', '/w/x'))
  assert.ok(!samePath('/w/x', null))
})

test('the header lists an unregistered home first, then 1-9 in their order, the tenth with no number', () => {
  const registered = Array.from({ length: 10 }, (_, n) => ({ name: `p${n + 1}`, path: `/w/p${n + 1}` }))
  const entries = headerProjects(registered, { root: '/home/me/tracker', name: 'tracker' }, null)
  assert.deepEqual(entries.slice(0, 2), [
    { name: 'tracker', path: '/home/me/tracker', number: null, home: true, current: true },
    { name: 'p1', path: '/w/p1', number: 1, home: false, current: false },
  ])
  assert.deepEqual(entries.map((e) => e.number), [null, 1, 2, 3, 4, 5, 6, 7, 8, 9, null])
  assert.deepEqual(numberedNames(entries), ['p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7', 'p8', 'p9'])
  assert.equal(entryOf(entries, 3).path, '/w/p3')
  assert.equal(entryOf(entries, 10), undefined)
  // ?project= names the one on screen.
  assert.deepEqual(headerProjects(registered, { root: '/home/me/tracker', name: 'tracker' }, '/w/p2/').filter((e) => e.current).map((e) => e.name), ['p2'])
  // A registered home stands once, numbered, and is the home.
  const home = headerProjects([{ name: 'a', path: '/w/a' }, { name: 'h', path: '/w/h' }], { root: '/w/h/', name: 'h' }, null)
  assert.deepEqual(home.map((e) => [e.name, e.number, e.home, e.current]), [['a', 1, false, false], ['h', 2, true, true]])
})

test('switching is a new page: / for the home, /?project=<path> for another', () => {
  assert.equal(projectHref('/w/h', '/w/h/'), '/')
  assert.equal(projectHref('/w/a b', '/w/h'), '/?project=%2Fw%2Fa%20b')
  assert.equal(projectHref('/w/a', null), '/?project=%2Fw%2Fa')
})

// ── the page: the header's column, the digits, a click ──

const ROWS = [
  { id: 'm1', kind: 'milestone', title: 'v1', status: 'todo' },
  { id: 'e1', kind: 'epic', title: 'Epic one', milestone: 'm1', status: 'todo' },
  { id: 'e1.a', kind: 'issue', title: 'Alpha', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'todo', priority: 1 },
]

function page(entries) {
  const dom = new JSDOM('<!doctype html><html lang="en"><body><div id="moai-app"></div></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const store = createStore()
  const index = new IssueIndex(ROWS, treeOf(ROWS))
  const opened = []
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc, store, copy: en, language: 'en', panes: PANES,
    index: (state) => (state.board.value ? index : null),
    activity: () => ({ phase: 'idle', cancellable: false }),
    intents: { refresh() {}, choose: (id) => store.dispatch({ type: 'cursor', id }), cancel() {}, openProject: (entry) => opened.push(entry.path) },
  })
  store.dispatch({ type: 'board-loaded', data: { rows: ROWS, loadedAt: Date.now() } })
  if (entries) store.dispatch({ type: 'projects', projects: entries })
  const keys = wireKeys({ document: doc, store, shell, copy: en, platform: 'other', sortDefault: createSortDefault(), openProject: (entry) => opened.push(entry.path) })
  const press = (key, target = doc.activeElement ?? doc.body) => {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })
    target.dispatchEvent(event)
    return event
  }
  return { dom, doc, store, shell, keys, press, opened, $: (s) => doc.querySelector(s), $$: (s) => [...doc.querySelectorAll(s)] }
}

const ENTRIES = headerProjects([{ name: 'argos', path: '/w/argos' }, { name: 'moai', path: '/w/moai' }], { root: '/home/t', name: 't' }, '/w/moai')

test('the header draws the projects as the TUI does: <n> name, the one on screen marked, a click opens another', () => {
  const p = page(ENTRIES)
  const column = p.$('.m-brand-projects')
  assert.equal(column.hidden, false)
  assert.ok(p.$('.m-brand').hasAttribute('data-projects'))
  assert.deepEqual(p.$$('.m-brand-project').map((b) => b.textContent), ['      t', '<1>   argos', '<2> ▸ moai'])
  const current = p.$$('.m-brand-project[aria-current="page"]')
  assert.deepEqual(current.map((b) => b.dataset.path), ['/w/moai'], 'marked by more than its colour')
  assert.deepEqual(current.map((b) => b.querySelector('.m-brand-here').textContent), ['▸ '], 'a glyph stands before its name')
  p.$('.m-brand-project[data-path="/w/argos"]').click()
  current[0].click()
  assert.deepEqual(p.opened, ['/w/argos'], 'the project on screen is not opened again')
})

test('no projects: no column, and the digits do nothing', () => {
  const p = page(null)
  assert.equal(p.$('.m-brand-projects').hidden, true)
  assert.ok(!p.$('.m-brand').hasAttribute('data-projects'))
  p.doc.body.focus()
  assert.equal(p.press('1').defaultPrevented, false)
  assert.deepEqual(p.opened, [])
})

test('digits jump to a project from the list and the wiki, never from inside the search, the filter or the sort editor', () => {
  const p = page(ENTRIES)
  p.shell.focusPane('list')
  p.press('1')
  p.press('2')
  assert.deepEqual(p.opened, ['/w/argos'], '2 is the project on screen')
  // SPC p 1 is the same jump.
  p.press(' '); p.press('p')
  assert.deepEqual([...p.keys.menu.el.querySelectorAll('.m-wk-label')].map((l) => l.textContent), ['argos', 'moai'])
  p.press('1')
  assert.deepEqual(p.opened, ['/w/argos', '/w/argos'])
  // The search prompt: the digit is typed, not run.
  p.press('/')
  assert.equal(p.doc.activeElement, p.shell.toolbar.searchInput)
  p.press('1', p.shell.toolbar.searchInput)
  // The filter prompt.
  p.shell.toolbar.filterInput.focus()
  p.press('1', p.shell.toolbar.filterInput)
  // The sort editor.
  p.press('Escape', p.shell.toolbar.filterInput)
  p.store.dispatch({ type: 'toolbar', action: { type: 'sort-toggle' } })
  p.press('1')
  assert.equal(p.store.getState().toolbar.sortOpen, true)
  p.press('Escape')
  assert.deepEqual(p.opened, ['/w/argos', '/w/argos'], 'nothing jumped from a prompt or the sort editor')
  // The wiki tab.
  p.store.dispatch({ type: 'tab', tab: 'wiki' })
  p.doc.body.focus()
  p.press('1')
  assert.deepEqual(p.opened.length, 3)
})
