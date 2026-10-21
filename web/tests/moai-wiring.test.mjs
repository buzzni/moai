// Moai panel wiring (se-cv7w.0qk, se-jj42): the toolbar in the shell with its state in the store, the list and
// board drawing their rows through data/query.ts (filter, search, sort), and the wiki tab mounted in the shell
// with its state kept in the store — in a jsdom document, as main.ts mounts them.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { createStore } = await panelModule('store.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { copyFor } = await panelModule('i18n/index.ts')
const { mountShell } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')
const { mountWiki } = await panelModule('view/wiki.ts')
const { wireKeys } = await panelModule('keys/wiring.ts')
const { createSortDefault } = await panelModule('data/memory.ts')

const en = copyFor('en')
const old = new Date(Date.now() - 40 * 86_400_000).toISOString()
const recent = new Date(Date.now() - 86_400_000).toISOString()

// No milestones: the root lists the work itself. r3 is archived and r4 deferred, so the view hides both.
const ROWS = [
  { id: 'r1', kind: 'issue', title: 'Fix login', status: 'todo', priority: 1, tags: ['ui'] },
  { id: 'r2', kind: 'issue', title: 'Write docs', status: 'todo', priority: 2 },
  { id: 'r3', kind: 'issue', title: 'Old login page', status: 'done', priority: 2, status_since: old },
  { id: 'r4', kind: 'issue', title: 'Later', status: 'todo', priority: 1, deferred_at: recent },
]

function boot({ rows = ROWS, wiki, store = createStore() } = {}) {
  const dom = new JSDOM('<!doctype html><html lang="en"><head></head><body><div id="moai-app"></div></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const index = new IssueIndex(rows)
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc,
    store,
    copy: en,
    language: 'en',
    panes: PANES,
    index: (state) => (state.board.value ? index : null),
    activity: () => ({ phase: 'idle', cancellable: false }),
    intents: { refresh() {}, choose: (id) => store.dispatch({ type: 'cursor', id }), add() {}, cancel() {} },
    wiki,
  })
  store.dispatch({ type: 'board-loaded', data: { rows, more: 0, loadedAt: Date.now() } })
  // The one keydown listener, as main.ts wires it.
  const keys = wireKeys({ document: doc, store, shell, copy: en, platform: 'other', sortDefault: createSortDefault() })
  const $ = (selector) => doc.querySelector(selector)
  const listed = () => [...doc.querySelectorAll('.m-pane-main[data-pane="list"] .moai-line')].map((line) => line.dataset.key)
  const type = (input, text) => {
    input.value = text
    input.dispatchEvent(new dom.window.InputEvent('input', { bubbles: true }))
  }
  const key = (target, init) => target.dispatchEvent(new dom.window.KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }))
  return { dom, doc, store, shell, keys, $, listed, type, key, tab: (id) => $(`[role="tab"][data-tab="${id}"]`) }
}

const ticks = async (n = 5) => {
  for (let i = 0; i < n; i++) await new Promise((resolve) => setTimeout(resolve, 0))
}

test('toolbar: typing a search narrows the list, shows hidden finds dim, and the note counts only what the view hides', () => {
  const panel = boot()
  assert.ok(panel.$('.m-toolbar-slot [data-toolbar]'), 'the toolbar is mounted above the panes')
  assert.deepEqual(panel.listed(), ['r1', 'r2'])
  // The search button opens the search row and puts the caret in it.
  panel.$('.m-toolbar-slot button[aria-label="Search"]').click()
  const search = panel.$('.m-toolbar-slot [role="search"] input')
  assert.equal(panel.doc.activeElement, search)
  panel.type(search, 'login')
  assert.deepEqual(panel.store.getState().search, { text: 'login', scope: 'all', open: true })
  // The search lifts the view's hiding: the archived r3 is found, drawn dim and marked.
  assert.deepEqual(panel.listed(), ['r1', 'r3'])
  const r3 = panel.$('.moai-line[data-key="r3"]')
  assert.ok(r3.hasAttribute('data-revealed') && r3.classList.contains('m-row-dim'))
  assert.match(r3.querySelector('.m-row-title').textContent, new RegExp(en.list.hiddenMark))
  assert.equal(panel.$('.moai-line[data-key="r1"] .m-row-title mark').textContent, 'login')
  // Only r3 counts: the view hides it (archived). A done row in a column the view shows would not count.
  assert.equal(panel.$('.m-hidden-note').textContent, '1 hidden: archived 1')
  assert.equal(panel.$('.m-toolbar-slot [role="search"] input'), search, 'the input was not rebuilt')
  // The scope narrows what is looked in.
  const scope = panel.$('.m-toolbar-slot select')
  scope.value = 'tag'
  scope.dispatchEvent(new panel.dom.window.Event('change', { bubbles: true }))
  assert.deepEqual(panel.listed(), [])
  panel.type(search, 'ui')
  assert.deepEqual(panel.listed(), ['r1'])
  // The board is a layout of the same list.
  panel.tab('board').click()
  assert.deepEqual([...panel.doc.querySelectorAll('.m-card')].map((card) => card.dataset.id), ['r1'])
  panel.tab('list').click()
  // Esc in the search closes it and the rows come back.
  panel.key(search, { key: 'Escape' })
  assert.equal(panel.store.getState().search.text, '')
  assert.deepEqual(panel.listed(), ['r1', 'r2'])
  assert.equal(panel.$('.m-hidden-note').hidden, true)
})

test('toolbar: the filter narrows the rows on Enter, a chip removal at once; the path line hangs it', () => {
  const panel = boot()
  const filter = panel.$('.m-toolbar-slot .m-filter input')
  // The caret in the filter, as SPC f or a click leaves it: the key router reads the input as a prompt.
  filter.focus()
  panel.type(filter, 'priority=2')
  assert.deepEqual(panel.listed(), ['r1', 'r2'], 'typing does not filter yet')
  assert.equal(panel.store.getState().toolbar.hintsOpen, true)
  // Enter with no suggestion chosen applies the text.
  panel.key(filter, { key: 'Enter' })
  assert.deepEqual(panel.store.getState().filter, { text: 'priority=2', applied: 'priority=2' })
  // The filter follows the view's hiding: r3 (priority 2, archived) stays hidden; the list note counts it.
  assert.deepEqual(panel.listed(), ['r2'])
  assert.equal(panel.$('.moai-list-note').hidden, false)
  assert.equal(panel.$('.moai-list-note span').textContent, en.list.hiddenCount(1))
  assert.match(panel.$('.m-crumbs-badges').textContent, /priority=2/)
  assert.equal(panel.$('.m-toolbar-slot .m-filter input'), filter, 'the input was not rebuilt')
  // Esc in the input is the prompt's cancel (keybindings.md 3.7), one key owner: what was typed since goes, the
  // applied filter stays, and the caret leaves the input — whether the suggestions were open or not.
  filter.focus()
  panel.type(filter, 'priority=2 tag=')
  assert.equal(panel.store.getState().toolbar.hintsOpen, true)
  panel.key(filter, { key: 'Escape' })
  assert.deepEqual(panel.store.getState().filter, { text: 'priority=2', applied: 'priority=2' })
  assert.equal(panel.store.getState().toolbar.hintsOpen, false)
  assert.notEqual(panel.doc.activeElement, filter)
  panel.$('.m-toolbar-slot .m-chips .m-chip-x').click()
  assert.deepEqual(panel.store.getState().filter, { text: '', applied: '' })
  assert.deepEqual(panel.listed(), ['r1', 'r2'])
})

test('toolbar: the sort popover and the name line change one order, query.ts\'s', () => {
  const panel = boot()
  panel.$('.m-toolbar-slot .m-sort button').click()
  panel.$('.m-sort-pop [data-key="priority"] [data-how="flip"]').click()
  assert.deepEqual(panel.store.getState().sort, { fields: [{ key: 'priority', dir: 'desc' }], reversed: false })
  assert.deepEqual(panel.listed(), ['r2', 'r1'])
  assert.match(panel.$('.m-toolbar-slot .m-sort button').textContent, /Priority↓/)
  assert.match(panel.$('.m-crumbs-badges').textContent, /sort priority↓/)
  // A name-line click is the same state: title alone, and the toolbar's badge follows.
  panel.$('.moai-head-sort[data-sort="title"]').click()
  assert.deepEqual(panel.store.getState().sort, { fields: [{ key: 'title', dir: 'asc' }], reversed: false })
  assert.deepEqual(panel.listed(), ['r1', 'r2'])
  assert.match(panel.$('.m-toolbar-slot .m-sort button').textContent, /Title↑/)
})

test('toolbar: an IME composition keeps its input and its text across store changes', () => {
  const panel = boot()
  panel.$('.m-toolbar-slot button[aria-label="Search"]').click()
  const search = panel.$('.m-toolbar-slot [role="search"] input')
  search.dispatchEvent(new panel.dom.window.CompositionEvent('compositionstart', { bubbles: true }))
  panel.type(search, 'ㄹ')
  search.value = '로'
  // Anything else changes the store (a cursor move, a reload) while the IME still composes.
  panel.store.dispatch({ type: 'cursor', id: 'r2' })
  assert.equal(panel.$('.m-toolbar-slot [role="search"] input'), search)
  assert.equal(search.value, '로')
  assert.equal(panel.doc.activeElement, search)
  search.dispatchEvent(new panel.dom.window.CompositionEvent('compositionend', { bubbles: true }))
  assert.equal(panel.store.getState().search.text, '로')
})

test('detail: a search paints its hits in the id and title', () => {
  const panel = boot()
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'r1', row: ROWS[0] } })
  assert.equal(panel.$('.moai-detail-title mark'), null)
  panel.store.dispatch({ type: 'search', text: 'login' })
  assert.equal(panel.$('.moai-detail-title mark').textContent, 'login')
  panel.store.dispatch({ type: 'search', text: 'r1', scope: 'title' })
  assert.equal(panel.$('.moai-detail-id mark'), null, 'the title scope does not look in the id')
})

const PAGES = [
  { slug: 'README', title: 'Readme', path: 'README.md' },
  { slug: 'guide', title: 'Guide', path: 'docs/guide.md' },
]
function wikiSource() {
  const calls = { list: 0, page: [] }
  return {
    calls,
    list: async () => {
      calls.list++
      return { dir: 'docs', pages: PAGES }
    },
    page: async (slug) => {
      calls.page.push(slug)
      const page = PAGES.find((one) => one.slug === slug)
      return { ...page, body: `# ${page.title}\n\nbody of ${slug}\n` }
    },
  }
}

test('wiki tab: mounted with its source, keeps its page across tab switches, saved into the store', async () => {
  const source = wikiSource()
  const panel = boot({ wiki: source })
  assert.equal(source.calls.list, 0, 'nothing is read before the tab shows')
  panel.tab('wiki').click()
  await ticks()
  assert.equal(source.calls.list, 1)
  assert.equal(panel.$('#m-panel-wiki .m-wiki-title').textContent, 'Readme')
  panel.$('#m-panel-wiki [data-slug="guide"]').click()
  await ticks()
  assert.equal(panel.$('#m-panel-wiki .m-wiki-title').textContent, 'Guide')
  panel.tab('list').click()
  assert.equal(panel.store.getState().wiki.current, 'guide', 'leaving the tab saves the view\'s state')
  panel.tab('wiki').click()
  await ticks()
  assert.equal(panel.$('#m-panel-wiki .m-wiki-title').textContent, 'Guide')
  assert.equal(source.calls.list, 1, 'the listing is read once')

  // A panel mounted again on that store opens the saved page, not the first one.
  const again = boot({ wiki: wikiSource(), store: createStore({ ...panel.store.getState(), tab: 'wiki' }) })
  await ticks()
  assert.equal(again.$('#m-panel-wiki .m-wiki-title').textContent, 'Guide')
})

test('wiki tab: Esc the wiki does not use moves the focus to the tab bar; the pane exposes its keys', async () => {
  const source = wikiSource()
  const panel = boot({ wiki: source })
  panel.tab('wiki').click()
  await ticks()
  const scroller = panel.$('#m-panel-wiki .m-wiki-scroll')
  scroller.focus()
  panel.key(scroller, { key: 'Escape' })
  assert.equal(panel.doc.activeElement, panel.tab('wiki'))
  // An Esc the wiki uses (clearing its search) stays inside.
  const search = panel.$('#m-panel-wiki .m-wiki-search input')
  panel.type(search, 'gui')
  scroller.focus()
  panel.key(scroller, { key: 'Escape' })
  assert.notEqual(panel.doc.activeElement, panel.tab('wiki'))
  assert.equal(search.value, '')
})

test('wiki pane: handleKey and toggleRaw are on the pane object, for the key router', async () => {
  const dom = new JSDOM('<!doctype html><body></body>')
  const doc = dom.window.document
  const root = doc.createElement('div')
  doc.body.append(root)
  const store = createStore()
  const ctx = {
    document: doc, copy: en, language: 'en', wiki: wikiSource(), focusPane() {},
    layout: () => ({ width: 900, mainWidth: 900, position: 'bottom', detailVisible: true }),
    intents: { dispatch: (action) => store.dispatch(action), enter() {}, choose() {}, point() {}, closeDetail() {}, setDetailVisible() {}, toggleExpanded() {}, refresh() {} },
  }
  const pane = mountWiki(root, ctx)
  pane.render(store.getState(), null)
  await ticks()
  assert.equal(root.querySelector('.m-wiki-title').textContent, 'Readme')
  const raw = root.querySelector('.m-wiki-actions button[aria-pressed]')
  pane.toggleRaw()
  assert.equal(raw.getAttribute('aria-pressed'), 'true')
  // Not the wiki's: an Esc with nothing to clear is handed back (false); `/` is its own.
  const press = (init) => pane.handleKey(new dom.window.KeyboardEvent('keydown', { cancelable: true, ...init }))
  assert.equal(press({ key: 'Escape' }), false)
  assert.equal(press({ key: '/' }), true)
  pane.leave()
  assert.equal(store.getState().wiki.raw, true)
  pane.destroy()
})

test('wiki tab: without a source it says the wiki cannot be read', () => {
  const panel = boot()
  panel.tab('wiki').click()
  assert.equal(panel.$('#m-panel-wiki .m-empty-title').textContent, en.shell.wikiUnavailable)
})

test('wiki tab: a listing that failed is read again the next time the tab shows', async () => {
  const source = wikiSource()
  let fail = true
  const list = source.list
  source.list = async () => {
    if (fail) {
      source.calls.list++
      throw new Error('The run did not start: the confirmation was declined.')
    }
    return list()
  }
  const panel = boot({ wiki: source })
  panel.tab('wiki').click()
  await ticks()
  assert.match(panel.$('#m-panel-wiki').textContent, /could not be read/)
  // Neither a store change nor staying on the tab reads it again; leaving and coming back does.
  panel.store.dispatch({ type: 'cursor', id: 'r2' })
  await ticks()
  assert.equal(source.calls.list, 1)
  fail = false
  panel.tab('list').click()
  panel.tab('wiki').click()
  await ticks()
  assert.equal(source.calls.list, 2)
  assert.equal(panel.$('#m-panel-wiki .m-wiki-title').textContent, 'Readme')
})

test('wiki tab: a narrower panel stacks the wiki when it is resized on the wiki tab', async () => {
  const panel = boot({ wiki: wikiSource() })
  panel.shell.setWidth(1000)
  panel.tab('wiki').click()
  await ticks()
  const view = panel.$('#m-panel-wiki .m-wiki')
  assert.equal(view.dataset.narrow, 'false')
  panel.shell.setWidth(500)
  assert.equal(view.dataset.narrow, 'true', 'without waiting for a store change')
})
