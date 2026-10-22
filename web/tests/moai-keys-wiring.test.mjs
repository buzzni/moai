// Moai panel key wiring (se-jwl3): the key router as main.ts wires it (keys/wiring.ts) — one keydown listener on
// the document, its action ids turned into store actions and pane commands, the SPC menu, the help overlay and the
// sort editor — in a jsdom document with the shell mounted.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const { createStore } = await panelModule('store.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { treeOf } = await import('./moai-tree-helper.mjs')
const { copyFor } = await panelModule('i18n/index.ts')
const { mountShell } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')
const { wireKeys, keyContext, NOT_WIRED } = await panelModule('keys/wiring.ts')
const { KEY_TABLE, menuLayer } = await panelModule('keys/table.ts')
const { createSortDefault } = await panelModule('data/memory.ts')

const en = copyFor('en')

// A milestone with an epic in it (folders to go into), and two loose rows.
const ROWS = [
  { id: 'm1', kind: 'milestone', title: 'v1', status: 'todo' },
  { id: 'e1', kind: 'epic', title: 'Epic one', milestone: 'm1', status: 'todo' },
  { id: 'e1.a', kind: 'issue', title: 'Alpha', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'todo', priority: 1 },
  { id: 'e1.b', kind: 'issue', title: 'Beta', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'in_progress', priority: 2 },
  { id: 'e1.c', kind: 'issue', title: 'Gamma', epic: 'e1', derived_epic: 'e1', milestone: 'm1', status: 'todo', priority: 3 },
]

const PAGES = [
  { slug: 'README', title: 'Readme', path: 'README.md' },
  { slug: 'guide', title: 'Guide', path: 'docs/guide.md' },
  { slug: 'notes', title: 'Notes', path: 'docs/notes.md' },
]
const wikiSource = () => ({
  list: async () => ({ dir: 'docs', pages: PAGES }),
  page: async (slug) => {
    const page = PAGES.find((one) => one.slug === slug)
    return { ...page, body: `# ${page.title}\n\nSee [the guide](guide.md) and [the readme](README.md).\n` }
  },
})

function boot({ wiki, read, rows = ROWS, platform = 'other', sortDefault = createSortDefault() } = {}) {
  const dom = new JSDOM('<!doctype html><html lang="en"><body><div id="moai-app"></div></body></html>', { pretendToBeVisual: true })
  const doc = dom.window.document
  const store = createStore()
  const index = new IssueIndex(rows, treeOf(rows))
  const chosen = []
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc,
    store,
    copy: en,
    language: 'en',
    panes: PANES,
    index: (state) => (state.board.value ? index : null),
    activity: () => ({ phase: 'idle', cancellable: false }),
    intents: {
      refresh() {},
      choose: (id) => {
        chosen.push(id)
        store.dispatch({ type: 'cursor', id })
      },
      add() {},
      cancel() {},
    },
    wiki,
  })
  store.dispatch({ type: 'board-loaded', data: { rows, more: 0, loadedAt: Date.now() } })
  const keys = wireKeys({ document: doc, store, shell, copy: en, platform, read, sortDefault })
  const $ = (selector) => doc.querySelector(selector)
  /** A keydown on `target` (the focused element when left out), as the browser sends it. */
  const press = (key, init = {}, target = doc.activeElement ?? doc.body) => {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
    target.dispatchEvent(event)
    return event
  }
  const seq = (...keys) => keys.forEach((key) => press(key))
  const lines = () => [...doc.querySelectorAll('.m-pane-main[data-pane="list"] .moai-line')].map((line) => line.dataset.key)
  return { dom, doc, store, shell, keys, $, press, seq, lines, chosen, state: () => store.getState() }
}

const ticks = async (n = 5) => {
  for (let i = 0; i < n; i++) await new Promise((resolve) => setTimeout(resolve, 0))
}

/** In the epic, the cursor on its first row and the list focused. */
function inEpic(options) {
  const panel = boot(options)
  panel.store.dispatch({ type: 'enter', path: ['m:m1', 'e:e1'], cursor: 'e1.a' })
  panel.shell.focusPane('list')
  return panel
}

test('keys: j and k move the cursor exactly one row in the drawn order, ↓ too — no key runs twice', () => {
  const panel = inEpic()
  assert.deepEqual(panel.lines(), ['..', 'e1.a', 'e1.b', 'e1.c'])
  assert.equal(panel.doc.activeElement, panel.$('.moai-list'))
  const j = panel.press('j')
  assert.equal(j.defaultPrevented, true)
  assert.equal(panel.state().cursor, 'e1.b', 'one j, one row')
  panel.press('ArrowDown')
  assert.equal(panel.state().cursor, 'e1.c', 'one ↓, one row: the list has no keydown listener of its own')
  panel.press('k')
  assert.equal(panel.state().cursor, 'e1.b')
  // gg, then G: the sequence buffer.
  panel.seq('g', 'g')
  assert.equal(panel.state().cursor, '..')
  panel.press('G')
  assert.equal(panel.state().cursor, 'e1.c')
})

test('keys: a Korean input source — the jamo of J with code KeyJ moves the cursor', () => {
  const panel = inEpic()
  panel.press('ㅓ', { code: 'KeyJ' })
  assert.equal(panel.state().cursor, 'e1.b')
  panel.press('Process', { code: 'KeyK', keyCode: 229 })
  assert.equal(panel.state().cursor, 'e1.a')
})

test('keys: Enter goes into a folder and Backspace leaves it, landing on the folder', () => {
  const panel = boot()
  panel.store.dispatch({ type: 'enter', path: ['m:m1'], cursor: 'e1' })
  panel.shell.focusPane('list')
  panel.press('Enter')
  assert.deepEqual(panel.state().path, ['m:m1', 'e:e1'])
  assert.equal(panel.state().cursor, 'e1.a')
  const back = panel.press('Backspace')
  assert.equal(back.defaultPrevented, true)
  assert.deepEqual(panel.state().path, ['m:m1'])
  assert.equal(panel.state().cursor, 'e1')
  // Enter on a leaf shows it and the focus goes to the detail.
  panel.press('Enter')
  panel.press('j')
  panel.press('Enter')
  assert.deepEqual(panel.chosen, ['e1.b'])
  assert.equal(panel.state().focus, 'detail')
  // Backspace is the list's: from the detail it does nothing.
  panel.press('Backspace')
  assert.deepEqual(panel.state().path, ['m:m1', 'e:e1'])
  // Tab unfolds everything under a folder, and folds it again.
  panel.shell.focusPane('list')
  panel.press('Backspace')
  assert.equal(panel.state().cursor, 'e1')
  panel.press('Tab')
  assert.ok(panel.state().expanded.length > 0)
  assert.ok(panel.lines().includes('e1.a'))
  panel.press('Tab')
  assert.deepEqual(panel.state().expanded, [])
})

test('keys: SPC opens the menu; a toggle from it runs, keeps the menu open and redraws its state', () => {
  const panel = inEpic()
  const menu = panel.keys.menu.el
  assert.ok(menu.isConnected && menu.hidden)
  panel.press(' ')
  assert.equal(menu.hidden, false)
  assert.deepEqual(panel.keys.router.state().menu, ['SPC'])
  panel.press('v')
  const deferred = () => menu.querySelector('[data-key="l"] .m-wk-state').textContent
  assert.equal(deferred(), '[hidden]')
  panel.press('l')
  assert.equal(panel.state().view.hideDeferred, false)
  assert.equal(menu.hidden, false, 'a toggle keeps the menu open')
  assert.equal(deferred(), '[shown]', 'the menu is drawn again after a toggle that does not change its layer')
  // A click on an item is the key press.
  menu.querySelector('[data-key="l"]').click()
  assert.equal(panel.state().view.hideDeferred, true)
  assert.equal(deferred(), '[hidden]')
  // A store change from elsewhere redraws it too.
  panel.store.dispatch({ type: 'view', view: { hideDeferred: false } })
  assert.equal(deferred(), '[shown]')
  panel.press('Escape')
  assert.equal(menu.hidden, true)
  // SPC c i turns the id cell off, SPC o d moves the detail, SPC v d hides it (toggles: Esc closes the menu).
  panel.seq(' ', 'c', 'i', 'Escape')
  assert.equal(panel.state().columns.id, false)
  panel.seq(' ', 'o', 'd', 'Escape')
  assert.equal(panel.state().detail.position, 'left')
  panel.seq(' ', 'v', 'd', 'Escape')
  assert.equal(panel.state().detail.visible, false)
  assert.equal(menu.hidden, true)
  // An action item closes it: SPC g b.
  panel.seq(' ', 'g', 'b')
  assert.equal(panel.state().tab, 'board')
  assert.equal(menu.hidden, true)
})

test('keys: ? opens the help; keys are quiet while it is open, even with the focus outside it; Esc closes', () => {
  const panel = inEpic()
  panel.press('?')
  assert.equal(panel.keys.help.isOpen(), true)
  assert.equal(panel.doc.activeElement, panel.keys.help.el)
  // The focus leaves the overlay (a click behind it): the panel's keys still do nothing.
  panel.doc.body.focus()
  panel.press('j', {}, panel.doc.body)
  panel.press(' ', {}, panel.doc.body)
  assert.equal(panel.state().cursor, 'e1.a')
  assert.equal(panel.keys.router.state().menu, null)
  const esc = panel.press('Escape', {}, panel.doc.body)
  assert.equal(esc.defaultPrevented, true)
  assert.equal(panel.keys.help.isOpen(), false)
  panel.press('j', {}, panel.doc.body)
  assert.equal(panel.state().cursor, 'e1.b', 'the router works again')
  // SPC ? opens it too.
  panel.seq(' ', '?')
  assert.equal(panel.keys.help.isOpen(), true)
})

test('keys: Enter on a focused button is the button\'s — the router does not run `enter` or block the click', () => {
  const panel = boot()
  panel.store.dispatch({ type: 'enter', path: ['m:m1'], cursor: 'e1' })
  const sort = panel.$('.m-toolbar-slot .m-sort button')
  sort.focus()
  const enter = panel.press('Enter')
  assert.equal(enter.defaultPrevented, false, 'the click Enter makes is not prevented')
  assert.deepEqual(panel.state().path, ['m:m1'], '`enter` did not run')
  const space = panel.press(' ')
  assert.equal(space.defaultPrevented, false)
  assert.equal(panel.keys.router.state().menu, null, 'Space clicks the button, it does not open the menu')
  // Other keys on a button still reach the router.
  panel.press('k')
  assert.equal(panel.state().cursor, '..')
})

test('keys: / opens the search and puts the caret in it; typing there is text, Esc goes back to the list', () => {
  const panel = inEpic()
  const slash = panel.press('/')
  assert.equal(slash.defaultPrevented, true, 'the / is not typed into the search')
  const search = panel.shell.toolbar.searchInput
  assert.equal(panel.doc.activeElement, search)
  assert.equal(panel.state().search.open, true)
  assert.equal(panel.keys.context().mode, 'prompt')
  for (const key of ['j', 'k', 'G', ' ', 'q']) assert.equal(panel.press(key).defaultPrevented, false, `${key} types`)
  assert.equal(panel.state().cursor, 'e1.a')
  assert.equal(panel.keys.router.state().menu, null)
  // There is no scope to cycle: Tab in the search is not taken (the focus moves as the browser moves it), and
  // Esc closes the search and puts the focus back on the list.
  assert.equal(panel.press('Tab').defaultPrevented, false)
  assert.deepEqual(Object.keys(panel.state().search).sort(), ['open', 'text'])
  panel.press('Escape')
  assert.equal(panel.state().search.open, false)
  assert.equal(panel.doc.activeElement, panel.$('.moai-list'))
  panel.press('j')
  assert.equal(panel.state().cursor, 'e1.b')
  // SPC f puts the caret in the filter.
  panel.seq(' ', 'f')
  assert.equal(panel.doc.activeElement, panel.shell.toolbar.filterInput)
})

test('keys: SPC s sorts by a field; SPC s e opens the sort editor (sorter mode), Esc puts the order back', () => {
  const panel = inEpic()
  panel.seq(' ', 's', 't')
  assert.deepEqual(panel.state().sort.fields, [{ key: 'title', dir: 'asc' }])
  assert.equal(panel.$('.m-whichkey [data-key="t"] .m-wk-state').textContent, '[↑ asc]')
  panel.press('t')
  assert.deepEqual(panel.state().sort.fields, [{ key: 'title', dir: 'desc' }], 'again turns it around')
  panel.press('P')
  assert.deepEqual(panel.state().sort.fields.map((f) => f.key), ['title', 'priority'])
  panel.press('Escape')
  const before = panel.state().sort
  panel.seq(' ', 's', 'e')
  assert.equal(panel.state().toolbar.sortOpen, true)
  assert.equal(panel.keys.context().mode, 'sorter')
  const cursorRow = () => panel.$('.m-sort-pop [data-cursor]')?.dataset.key
  assert.equal(cursorRow(), 'title', 'the editor opens on the first field')
  panel.press('j')
  assert.equal(cursorRow(), 'due')
  panel.seq('k', 'k', 'k')
  assert.equal(cursorRow(), 'status')
  panel.press(' ')
  assert.deepEqual(panel.state().sort.fields.map((f) => f.key), ['title', 'priority', 'status'], 'SPC puts it in')
  panel.press('K')
  assert.deepEqual(panel.state().sort.fields.map((f) => f.key), ['title', 'status', 'priority'], 'K moves it up')
  panel.press('d')
  assert.equal(panel.state().sort.fields[1].dir, 'desc')
  assert.equal(panel.state().cursor, 'e1.a', 'j/k moved the editor, not the list')
  panel.press('Escape')
  assert.equal(panel.state().toolbar.sortOpen, false)
  assert.deepEqual(panel.state().sort, before, 'Esc cancels')
  panel.seq(' ', 's', 'e')
  panel.press('k')
  panel.press(' ')
  panel.press('Enter')
  assert.equal(panel.state().toolbar.sortOpen, false)
  assert.notDeepEqual(panel.state().sort, before, 'Enter keeps it')
  assert.equal(panel.keys.context().mode, 'browse')
})

test('keys: SPC s w sorts by the deadline (moai\'s `due`)', () => {
  const panel = inEpic()
  panel.seq(' ', 's', 'w')
  assert.deepEqual(panel.state().sort.fields, [{ key: 'due', dir: 'asc' }])
  assert.equal(panel.$('.m-whichkey [data-key="w"] .m-wk-state').textContent, '[↑ asc]')
})

test('keys: wiki mode — keys reach the wiki pane, SPC v r toggles raw, Tab cycles links and Esc gets out to the tab bar', async () => {
  const panel = boot({ wiki: wikiSource() })
  panel.shell.focusPane('list')
  panel.seq(' ', 'g', 'w')
  assert.equal(panel.state().tab, 'wiki')
  await ticks()
  assert.equal(panel.keys.context().mode, 'wiki')
  const selected = () => panel.$('#m-panel-wiki .m-wiki-list [aria-selected="true"]')?.dataset.slug
  assert.equal(selected(), 'README')
  panel.press('j')
  assert.equal(selected(), 'guide', 'j moves the wiki list cursor, once')
  // A motion key closes the SPC menu and moves — once: the wiki pane has no keydown listener of its own.
  panel.seq(' ', 'j')
  assert.equal(panel.keys.router.state().menu, null)
  assert.equal(selected(), 'notes')
  panel.press('ㅏ', { code: 'KeyK' })
  assert.equal(selected(), 'guide', 'a jamo reaches the wiki through its command')
  // The help keeps the wiki quiet too, with the focus on the wiki's list behind it.
  panel.press('?')
  panel.$('#m-panel-wiki .m-wiki-list [aria-selected="true"]').focus()
  panel.press('j')
  assert.equal(selected(), 'guide')
  panel.press('Escape')
  assert.equal(panel.keys.help.isOpen(), false)
  panel.shell.focusPane('list')
  panel.press('Enter')
  await ticks()
  assert.equal(panel.$('#m-panel-wiki .m-wiki-title').textContent, 'Guide')
  // SPC v r: the wiki's raw toggle (the menu's state label follows it).
  const raw = panel.$('#m-panel-wiki .m-wiki-actions button[aria-pressed]')
  assert.equal(raw.getAttribute('aria-pressed'), 'false')
  panel.seq(' ', 'v')
  assert.equal(panel.keys.menu.el.querySelector('[data-key="r"] .m-wk-state').textContent, '[rendered]')
  panel.press('r')
  assert.equal(raw.getAttribute('aria-pressed'), 'true')
  assert.equal(panel.keys.menu.el.querySelector('[data-key="r"] .m-wk-state').textContent, '[raw]')
  panel.press('Escape')
  panel.seq(' ', 'v', 'r')
  assert.equal(raw.getAttribute('aria-pressed'), 'false')
  panel.press('Escape')
  // Into the page, Tab selects a link; Esc clears the selection, the next Esc goes to the tab bar.
  panel.press('l')
  assert.equal(panel.keys.context().focus, 'page')
  const tab = panel.press('Tab')
  assert.equal(tab.defaultPrevented, true)
  assert.ok(panel.$('#m-panel-wiki .m-md-link[aria-current="true"]'), 'a link is selected')
  panel.press('Escape')
  assert.equal(panel.$('#m-panel-wiki .m-md-link[aria-current="true"]'), null)
  assert.notEqual(panel.doc.activeElement, panel.$('[role="tab"][data-tab="wiki"]'))
  panel.press('Escape')
  assert.equal(panel.doc.activeElement, panel.$('[role="tab"][data-tab="wiki"]'), 'the way out of the wiki')
  // / is the wiki's search there, a prompt: typing is text, Esc goes back.
  panel.shell.focusPane('list')
  panel.press('/')
  const search = panel.$('#m-panel-wiki .m-wiki-search input')
  assert.equal(panel.doc.activeElement, search)
  assert.equal(panel.keys.context().mode, 'prompt')
  assert.equal(panel.press('j').defaultPrevented, false)
  panel.press('Escape')
  assert.notEqual(panel.doc.activeElement, search)
  panel.seq(' ', 'g', 'l')
  assert.equal(panel.state().tab, 'list')
})

test('keys: the board — j/k inside a column, l to the next column with cards, Enter shows the card', () => {
  const panel = boot()
  panel.store.dispatch({ type: 'enter', path: ['m:m1', 'e:e1'], cursor: 'e1.a' })
  panel.seq(' ', 'g', 'b')
  assert.equal(panel.state().tab, 'board')
  panel.press('j')
  assert.equal(panel.state().cursor, 'e1.c', 'the next card in the todo column')
  panel.press('l')
  assert.equal(panel.state().cursor, 'e1.b', 'the in-progress column')
  panel.press('Enter')
  assert.deepEqual(panel.chosen, ['e1.b'])
})

test('keys: every action of the key table has a dispatch target; without read writes the read marks are not offered', () => {
  const panel = inEpic()
  const ids = new Set(Object.values(KEY_TABLE).flat().map((binding) => binding.action))
  const missing = [...ids].filter((id) => !panel.keys.actions.includes(id))
  assert.deepEqual(missing, [])
  for (const id of Object.keys(NOT_WIRED)) assert.ok(ids.has(id), `${id} is a table action`)
  // Not given the read writes, `r` and SPC m are unavailable: the menu has no SPC m layer and `r` runs nothing.
  assert.equal(panel.keys.context().readable, false)
  panel.press(' ')
  assert.equal(panel.keys.menu.el.querySelector('[data-key="m"]'), null)
  panel.press('Escape')
  assert.equal(panel.press('r').defaultPrevented, false)
  // The context speaks the store's vocabulary in the table's: archived polarity, `status`, `names`.
  const ctx = keyContext({ ...panel.state(), view: { ...panel.state().view, showArchived: true }, sort: { fields: [{ key: 'status', dir: 'desc' }, { key: 'id', dir: 'asc' }], reversed: false } }, { mode: 'browse', width: 900 })
  assert.equal(ctx.hidden.archived, false)
  // Every field of the order, `id` too: `status` is not the order alone, so `SPC s s` shows no direction (it would
  // replace the order, not turn it around).
  assert.deepEqual(ctx.sort, [{ field: 'status', desc: true }, { field: 'id', desc: false }])
  assert.equal(menuLayer('SPC s', ctx).items.find((item) => item.key === 's')?.state ?? null, null)
  const alone = keyContext({ ...panel.state(), sort: { fields: [{ key: 'status', dir: 'desc' }], reversed: false } }, { mode: 'browse', width: 900 })
  assert.equal(menuLayer('SPC s', alone).items.find((item) => item.key === 's')?.state?.id, 'desc')
  assert.equal(ctx.cells.names, true)
  assert.deepEqual(ctx.columns, ['todo', 'in_progress', 'review', 'done'])
  // The focus hint is in the page, in the panel's language.
  assert.equal(panel.keys.hint.textContent, en.keys.focusHint)
  assert.ok(panel.keys.hint.isConnected)
})

test('keys: a focus move into the detail lands on the pane, not on Close, so SPC still opens the menu', () => {
  const panel = inEpic()
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'e1.a', row: ROWS[2] } })
  assert.ok(panel.$('.moai-detail [data-focus="close"]'), 'the detail has its Close button')
  panel.seq(' ', 'w', 'w')
  assert.equal(panel.state().focus, 'detail')
  assert.equal(panel.doc.activeElement, panel.$('.m-pane-detail'), 'the pane itself, not its first button')
  const space = panel.press(' ')
  assert.equal(space.defaultPrevented, true, 'SPC is the router\'s: it does not click a button')
  assert.deepEqual(panel.keys.router.state().menu, ['SPC'])
  panel.seq('w', 'w')
  assert.equal(panel.state().focus, 'list')
  assert.ok(panel.state().shown.value, 'the detail is still shown')
})

test('keys: the sort editor opened with a click leaves the sort button, so SPC puts a field in', () => {
  const panel = inEpic()
  const button = panel.$('.m-toolbar-slot .m-sort button')
  button.focus()
  button.click()
  assert.equal(panel.state().toolbar.sortOpen, true)
  assert.notEqual(panel.doc.activeElement, button, 'the focus left the button that opened the editor')
  panel.press('j')
  const space = panel.press(' ')
  assert.equal(space.defaultPrevented, true)
  assert.deepEqual(panel.state().sort.fields.map((f) => f.key), ['priority', 'created'])
  assert.equal(panel.state().toolbar.sortOpen, true)
})

test('keys: a click on a filter suggestion keeps the caret in the filter; Tab opens the suggestions, S-Tab goes back', () => {
  const panel = inEpic()
  panel.seq(' ', 'f')
  const filter = panel.shell.toolbar.filterInput
  filter.value = 'sta'
  filter.setSelectionRange(3, 3)
  filter.dispatchEvent(new panel.dom.window.InputEvent('input', { bubbles: true }))
  const item = panel.$('.m-hints .m-menu-item')
  const down = new panel.dom.window.MouseEvent('mousedown', { bubbles: true, cancelable: true })
  item.dispatchEvent(down)
  assert.equal(down.defaultPrevented, true, 'the item does not take the focus from the filter')
  item.click()
  assert.equal(panel.state().filter.text, 'status=')
  assert.equal(panel.doc.activeElement, filter)
  // A chip's x does not take the focus either.
  const chips = panel.$('.m-toolbar-slot .m-chips')
  const chipDown = new panel.dom.window.MouseEvent('mousedown', { bubbles: true, cancelable: true })
  chips.dispatchEvent(chipDown)
  assert.equal(chipDown.defaultPrevented, true)
  // Closed suggestions: Tab opens them on the first one; S-Tab with one chosen goes back instead of taking it.
  panel.store.dispatch({ type: 'toolbar', action: { type: 'hints-close' } })
  panel.press('Tab')
  assert.deepEqual([panel.state().toolbar.hintsOpen, panel.state().toolbar.hintIndex], [true, 0])
  panel.press('ArrowDown')
  assert.equal(panel.state().toolbar.hintIndex, 1)
  panel.press('Tab', { shiftKey: true })
  assert.equal(panel.state().toolbar.hintIndex, 0)
  assert.equal(panel.state().filter.text, 'status=', 'S-Tab took no suggestion')
})

test('keys: the filter suggestions outlive the window losing the focus, not the filter losing it (se-4fci)', () => {
  const panel = inEpic()
  panel.seq(' ', 'f')
  const filter = panel.shell.toolbar.filterInput
  filter.value = 'sta'
  filter.setSelectionRange(3, 3)
  filter.dispatchEvent(new panel.dom.window.InputEvent('input', { bubbles: true }))
  assert.equal(panel.state().toolbar.hintsOpen, true)
  // The window going away blurs the input but leaves it the active element.
  filter.dispatchEvent(new panel.dom.window.FocusEvent('blur'))
  assert.equal(panel.doc.activeElement, filter)
  assert.equal(panel.state().toolbar.hintsOpen, true, 'a window blur keeps the suggestions')
  filter.blur()
  assert.notEqual(panel.doc.activeElement, filter)
  assert.equal(panel.state().toolbar.hintsOpen, false, 'leaving the filter closes them')
})

test('keys: outside the wiki ↑/↓ in the search are the input\'s, and ↓ in the filter still moves the suggestions (se-4fci)', () => {
  const panel = inEpic()
  panel.press('/')
  assert.equal(panel.doc.activeElement, panel.shell.toolbar.searchInput)
  assert.equal(panel.press('ArrowUp').defaultPrevented, false, '↑ moves the caret')
  assert.equal(panel.press('ArrowDown').defaultPrevented, false, '↓ moves the caret')
  panel.press('Escape')
  panel.seq(' ', 'f')
  const filter = panel.shell.toolbar.filterInput
  filter.value = 'sta'
  filter.setSelectionRange(3, 3)
  filter.dispatchEvent(new panel.dom.window.InputEvent('input', { bubbles: true }))
  const at = panel.state().toolbar.hintIndex
  assert.equal(panel.press('ArrowDown').defaultPrevented, true, 'the filter takes ↓ itself')
  assert.equal(panel.state().toolbar.hintIndex, at + 1)
})

test('keys: Esc in a search opened after an applied one keeps the cursor where it is', () => {
  const panel = inEpic()
  panel.press('/')
  panel.shell.toolbar.searchInput.value = 'a'
  panel.shell.toolbar.searchInput.dispatchEvent(new panel.dom.window.InputEvent('input', { bubbles: true }))
  panel.press('Enter')
  panel.press('j')
  assert.equal(panel.state().cursor, 'e1.b')
  // Back into the open search with a click, then Esc: the cursor stays on e1.b.
  panel.shell.toolbar.searchInput.focus()
  panel.press('Escape')
  assert.equal(panel.state().cursor, 'e1.b')
  // A search opened with / and left with Esc still puts the cursor back.
  panel.press('/')
  panel.store.dispatch({ type: 'cursor', id: 'e1.c' })
  panel.press('Escape')
  assert.equal(panel.state().cursor, 'e1.b')
})

test('keys: Enter on the board chooses the focused card when the cursor is on no card', () => {
  const panel = boot()
  panel.store.dispatch({ type: 'enter', path: ['m:m1'], cursor: 'e1' })
  panel.shell.focusPane('list')
  panel.seq(' ', 'g', 'b')
  assert.equal(panel.doc.activeElement.dataset.id, 'e1.a')
  panel.press('Enter')
  assert.deepEqual(panel.chosen, ['e1.a'])
})

test('keys: on the board at the root, j/k walk a column across the milestone lanes', () => {
  const rows = [
    { id: 'm1', kind: 'milestone', title: 'v1', status: 'todo' },
    { id: 'm2', kind: 'milestone', title: 'v2', status: 'todo' },
    { id: 'a', kind: 'issue', title: 'A', milestone: 'm1', status: 'todo', priority: 1 },
    { id: 'b', kind: 'issue', title: 'B', milestone: 'm2', status: 'todo', priority: 1 },
    { id: 'c', kind: 'issue', title: 'C', milestone: 'm2', status: 'todo', priority: 2 },
  ]
  const panel = boot({ rows })
  panel.store.dispatch({ type: 'cursor', id: 'a' })
  panel.seq(' ', 'g', 'b')
  assert.deepEqual([...panel.doc.querySelectorAll('.m-board[data-lane]')].map((lane) => lane.dataset.lane), ['m:m1', 'm:m2'])
  panel.press('j')
  assert.equal(panel.state().cursor, 'b', 'down into the next lane, same column')
  panel.press('j')
  assert.equal(panel.state().cursor, 'c')
  panel.seq('g', 'g')
  assert.equal(panel.state().cursor, 'a', 'the column\'s top card, in the first lane')
  panel.press('G')
  assert.equal(panel.state().cursor, 'c')
})

// ── se-h5c7: one key owner, the leftovers of review se-pbqz.y8z ──

test('keys: the help open — Enter on a button behind it and a letter in the search do nothing there (se-h5c7)', () => {
  const panel = inEpic()
  panel.press('?')
  const sort = panel.$('.m-toolbar-slot .m-sort button')
  sort.focus()
  assert.equal(panel.press('Enter').defaultPrevented, true, 'the button behind is not clicked')
  assert.equal(panel.doc.activeElement, panel.keys.help.el, 'the focus is back on the overlay')
  panel.shell.toolbar.searchInput.focus()
  assert.equal(panel.press('q').defaultPrevented, true, 'nothing is typed into the search behind')
  assert.equal(panel.state().toolbar.sortOpen, false)
  panel.press('Escape')
  assert.equal(panel.keys.help.isOpen(), false)
})

test('keys: the help open — Ctrl+C and Ctrl+D go to the browser: not prevented, the focus stays, the panel stays quiet (se-1fy7)', () => {
  const panel = inEpic()
  panel.press('?')
  const behind = panel.$('.m-toolbar-slot .m-sort button')
  behind.focus()
  const copy = panel.press('c', { ctrlKey: true }, behind)
  assert.equal(copy.defaultPrevented, false, 'the browser copies')
  assert.equal(panel.doc.activeElement, behind, 'the focus is not taken back')
  const down = panel.press('d', { ctrlKey: true }, behind)
  assert.equal(down.defaultPrevented, false)
  assert.equal(panel.state().cursor, 'e1.a', 'no panel shortcut ran behind the overlay')
  assert.equal(panel.keys.help.isOpen(), true)
  // A plain key is still stopped and the focus comes back.
  assert.equal(panel.press('j', {}, behind).defaultPrevented, true)
  assert.equal(panel.doc.activeElement, panel.keys.help.el)
})

test('keys: the help open — a Ctrl key a control behind it acts on, and AltGr, are stopped there (se-1fy7)', () => {
  const panel = inEpic()
  panel.press('?')
  const behind = panel.$('.m-toolbar-slot .m-sort button')
  for (const key of ['Enter', ' ']) {
    behind.focus()
    assert.equal(panel.press(key, { ctrlKey: true }, behind).defaultPrevented, true, `Ctrl+${key} can press the button`)
    assert.equal(panel.doc.activeElement, panel.keys.help.el)
  }
  assert.equal(panel.state().toolbar.sortOpen, false)
  // The tab strip and the resizer take arrows whatever the modifier.
  const tab = panel.doc.getElementById('m-tab-list')
  tab.focus()
  assert.equal(panel.press('ArrowRight', { ctrlKey: true }, tab).defaultPrevented, true)
  assert.equal(panel.state().tab, 'list', 'no tab switch behind the overlay')
  // Ctrl+Backspace deletes a word, and AltGr (Ctrl+Alt on Windows) types a character: @ on a German layout.
  const search = panel.shell.toolbar.searchInput
  for (const [key, init] of [['Backspace', { ctrlKey: true }], ['@', { ctrlKey: true, altKey: true }]]) {
    search.focus()
    assert.equal(panel.press(key, init, search).defaultPrevented, true, `nothing is edited in the search behind: ${key}`)
    assert.equal(panel.doc.activeElement, panel.keys.help.el)
  }
  // Ctrl+Esc closes the help, as Esc does, rather than the search behind it.
  search.focus()
  panel.press('Escape', { ctrlKey: true }, search)
  assert.equal(panel.keys.help.isOpen(), false)
})

test('keys: the help open on macOS — Ctrl edits text there (Ctrl+K) and is stopped; Cmd goes on (se-1fy7)', () => {
  const panel = inEpic({ platform: 'mac' })
  panel.press('?')
  const search = panel.shell.toolbar.searchInput
  search.focus()
  assert.equal(panel.press('k', { ctrlKey: true }, search).defaultPrevented, true)
  assert.equal(panel.doc.activeElement, panel.keys.help.el)
  search.focus()
  const copy = panel.press('c', { metaKey: true }, search)
  assert.equal(copy.defaultPrevented, false, 'Cmd+C copies')
  assert.equal(panel.doc.activeElement, search)
  assert.equal(panel.keys.help.isOpen(), true)
})

test('keys: Tab in browse moves the focus unless it runs expandAll on the list (se-h5c7)', () => {
  const panel = inEpic()
  assert.equal(panel.press('Tab').defaultPrevented, true, 'on the list Tab is expandAll')
  panel.store.dispatch({ type: 'detail-loaded', data: { id: 'e1.a', row: ROWS[2] } })
  panel.seq(' ', 'w', 'w')
  assert.equal(panel.state().focus, 'detail')
  assert.equal(panel.press('Tab').defaultPrevented, false, 'in the detail Tab moves the focus')
  assert.equal(panel.press('Tab', { shiftKey: true }).defaultPrevented, false)
  panel.seq(' ', 'g', 'b')
  panel.shell.focusPane('list')
  assert.equal(panel.state().tab, 'board')
  assert.equal(panel.press('Tab').defaultPrevented, false, 'on the board Tab moves the focus')
  // A focused toolbar button: Tab goes on to the next control, it does not unfold the list.
  panel.seq(' ', 'g', 'l')
  const expanded = panel.state().expanded
  panel.$('.m-toolbar-slot .m-sort button').focus()
  assert.equal(panel.press('Tab').defaultPrevented, false)
  assert.deepEqual(panel.state().expanded, expanded)
})

test('keys: the wiki search leaves Tab, S-Tab and ↑ to the input; ↓ still goes to the list (se-h5c7)', async () => {
  const panel = boot({ wiki: wikiSource() })
  panel.seq(' ', 'g', 'w')
  await ticks()
  panel.press('/')
  const search = panel.$('#m-panel-wiki .m-wiki-search input')
  assert.equal(panel.doc.activeElement, search)
  assert.equal(panel.press('Tab').defaultPrevented, false, 'Tab moves the focus')
  assert.equal(panel.press('Tab', { shiftKey: true }).defaultPrevented, false, 'S-Tab moves the focus back')
  assert.equal(panel.press('ArrowUp').defaultPrevented, false, '↑ moves the caret')
  assert.equal(panel.doc.activeElement, search)
  assert.equal(panel.press('ArrowDown').defaultPrevented, true)
  assert.notEqual(panel.doc.activeElement, search, '↓ goes down to the list')
})

test('keys: after Esc out of the wiki onto its tab, SPC opens the menu instead of clicking the tab (se-h5c7)', async () => {
  const panel = boot({ wiki: wikiSource() })
  panel.seq(' ', 'g', 'w')
  await ticks()
  panel.press('Escape')
  const tab = panel.$('[role="tab"][data-tab="wiki"]')
  assert.equal(panel.doc.activeElement, tab)
  const space = panel.press(' ')
  assert.equal(space.defaultPrevented, true, 'the tab is not clicked again')
  assert.deepEqual(panel.keys.router.state().menu, ['SPC'])
  assert.equal(panel.keys.menu.el.hidden, false)
  panel.seq('g', 'l')
  assert.equal(panel.state().tab, 'list', 'the menu works there')
})

test('keys: PageDown, C-d, C-u and PageUp in the wiki page list move its cursor, not the page (se-h5c7)', async () => {
  const panel = boot({ wiki: wikiSource() })
  panel.seq(' ', 'g', 'w')
  await ticks()
  const selected = () => panel.$('#m-panel-wiki .m-wiki-list [aria-selected="true"]')?.dataset.slug
  const scroller = panel.$('#m-panel-wiki .m-wiki-scroll')
  assert.equal(panel.keys.context().focus, 'list')
  assert.equal(selected(), 'README')
  panel.press('PageDown')
  assert.equal(selected(), 'notes', 'a page of rows (all three here)')
  panel.press('u', { ctrlKey: true })
  assert.equal(selected(), 'README')
  panel.press('d', { ctrlKey: true })
  assert.equal(selected(), 'notes')
  panel.press('PageUp')
  assert.equal(selected(), 'README')
  assert.equal(scroller.scrollTop, 0, 'the page did not scroll')
  assert.equal(panel.doc.activeElement, panel.$('#m-panel-wiki .m-wiki-list [aria-selected="true"]'))
})

test('keys: the SPC menu closes when the mode changes under it — a click into an input, onto another tab (se-h5c7)', () => {
  const panel = inEpic()
  const menu = panel.keys.menu.el
  panel.press(' ')
  assert.equal(menu.hidden, false)
  panel.shell.toolbar.searchInput.focus()
  assert.equal(panel.keys.context().mode, 'prompt')
  assert.equal(panel.keys.router.state().menu, null)
  assert.equal(menu.hidden, true, 'not left drawn with items that would not run')
  panel.shell.focusPane('list')
  panel.seq(' ', 'v')
  assert.equal(menu.hidden, false)
  panel.$('[role="tab"][data-tab="wiki"]').click()
  assert.equal(panel.keys.context().mode, 'wiki')
  assert.equal(panel.keys.router.state().menu, null)
  assert.equal(menu.hidden, true)
  // List → board is no change of mode: the menu stays, redrawn for the board, and its items run.
  panel.$('[role="tab"][data-tab="list"]').click()
  panel.shell.focusPane('list')
  panel.seq(' ', 'v')
  panel.$('[role="tab"][data-tab="board"]').click()
  assert.equal(menu.hidden, false)
  menu.querySelector('[data-key="l"]').click()
  assert.equal(panel.state().view.hideDeferred, false)
})

test('keys: the filter suggestions close when the filter loses the focus (se-h5c7)', () => {
  const panel = inEpic()
  panel.seq(' ', 'f')
  const filter = panel.shell.toolbar.filterInput
  filter.value = 'sta'
  filter.setSelectionRange(3, 3)
  filter.dispatchEvent(new panel.dom.window.InputEvent('input', { bubbles: true }))
  assert.equal(panel.state().toolbar.hintsOpen, true)
  assert.ok(panel.$('.m-hints .m-menu-item'))
  panel.shell.focusPane('list')
  assert.equal(panel.state().toolbar.hintsOpen, false)
  assert.equal(panel.$('.m-hints .m-menu-item'), null, 'the list is no longer drawn')
  assert.equal(panel.state().filter.text, 'sta', 'the text stays')
})

test('keys: Esc in a search opened with / still puts the cursor back after the window lost the focus and came back', () => {
  const panel = inEpic()
  panel.press('/')
  const search = panel.shell.toolbar.searchInput
  assert.equal(panel.doc.activeElement, search)
  panel.store.dispatch({ type: 'cursor', id: 'e1.c' })
  // The window going away blurs the input but leaves it the active element: the caret has not left the search.
  search.dispatchEvent(new panel.dom.window.FocusEvent('blur'))
  assert.equal(panel.doc.activeElement, search)
  panel.press('Escape')
  assert.equal(panel.state().cursor, 'e1.a', 'back where / opened the search')
  // A real blur (a click elsewhere) still forgets it.
  panel.press('/')
  panel.store.dispatch({ type: 'cursor', id: 'e1.c' })
  search.blur()
  search.focus()
  panel.press('Escape')
  assert.equal(panel.state().cursor, 'e1.c')
})

test('keys: D in the sort editor saves the order as the default; emptying the order returns to it', () => {
  const sortDefault = createSortDefault()
  const panel = inEpic({ sortDefault })
  panel.seq(' ', 's', 't')
  panel.press('P')
  panel.press('Escape')
  panel.seq(' ', 's', 'e')
  assert.match(panel.$('.m-sort-pop [data-sort-note]').textContent, /until the tab is closed/, 'the editor says the default lasts until the tab closes')
  panel.press('D')
  assert.deepEqual(sortDefault.get(), { fields: [{ key: 'title', dir: 'asc' }, { key: 'priority', dir: 'asc' }], reversed: false })
  assert.equal(panel.$('.moai-result')?.textContent ?? '', en.keys.sortDefaultSavedSession)
  assert.equal(panel.state().toolbar.sortOpen, false, 'D applies, saves and leaves the editor')
  assert.deepEqual(panel.state().sort, sortDefault.get())
  assert.deepEqual(panel.state().defaultSort, sortDefault.get(), 'the store holds the saved default (store.ts falls back to it)')
  // Take every field out (SPC on each): the reset is the saved default, not priority alone.
  panel.seq(' ', 's', 'e')
  panel.press(' ')
  panel.seq('k', 'k', 'k', 'k', 'k')
  panel.press(' ')
  assert.deepEqual(panel.state().sort.fields.map((f) => f.key), ['title', 'priority'])
})

test('keys: where the default persists, the sort editor does not say it lasts for the session, and D says it is saved', () => {
  const kept = new Map()
  const storage = { read: (key) => kept.get(key) ?? null, write: (key, value) => (value === null ? kept.delete(key) : kept.set(key, value)), persistent: true }
  const sortDefault = createSortDefault(storage)
  const panel = inEpic({ sortDefault })
  panel.seq(' ', 's', 'e')
  assert.equal(panel.$('.m-sort-pop [data-sort-note]').hidden, true, 'the session-only note is not shown')
  // The popover is drawn again when the order changes under it (d flips the field): the note stays hidden.
  const before = panel.$('.m-sort-pop')
  panel.press('d')
  assert.notEqual(panel.$('.m-sort-pop'), before, 'the popover was drawn again')
  assert.equal(panel.$('.m-sort-pop [data-sort-note]').hidden, true)
  panel.press('D')
  assert.equal(panel.$('.moai-result')?.textContent ?? '', en.keys.sortDefaultSaved)
})

test('keys: an English panel draws the SPC menu and the help in English, with no Korean left (se-ti70)', () => {
  const panel = inEpic()
  panel.press(' ')
  const menu = panel.keys.menu.el
  assert.equal(menu.getAttribute('aria-label'), en.table.root)
  assert.equal(menu.querySelector('.m-wk-title').textContent, en.table.root)
  assert.ok(menu.querySelector('.m-wk-footer').textContent.includes(en.table.labels['menu.close']))
  assert.doesNotMatch(menu.textContent, /[가-힣]/)
  panel.press('Escape')
  panel.press('?')
  const help = panel.keys.help.el
  assert.equal(help.getAttribute('aria-label'), en.table.labels.help)
  assert.equal(help.querySelector('.m-help-head strong').textContent, en.table.helpTitle)
  assert.equal(help.querySelector('.m-help-head button').getAttribute('aria-label'), en.table.close)
  assert.equal(help.querySelector('.m-help-title').textContent, en.table.groups.move)
  assert.doesNotMatch(help.textContent, /[가-힣]/)
  panel.press('Escape')
  panel.seq(' ', 's', 'e')
  assert.doesNotMatch(panel.$('.m-sort-pop').textContent, /[가-힣]/)
})

// ── read marks (moai-ly88): upstream's `r`, `SPC m a`, `SPC m g`, restored ──

test('keys: r marks the cursor row read, SPC m a everything, SPC m g the group the list stands in; a refusal is told', async () => {
  const calls = []
  let refuse = null
  const read = {
    one: async (id) => (calls.push(['one', id]), refuse),
    all: async () => (calls.push(['all']), refuse),
    group: async (id) => (calls.push(['group', id]), refuse),
  }
  const panel = inEpic({ read })
  assert.equal(panel.keys.context().readable, true)
  panel.press(' ')
  assert.ok(panel.keys.menu.el.querySelector('[data-key="m"]'), 'the SPC m layer is offered')
  panel.press('Escape')
  assert.equal(panel.press('r').defaultPrevented, true)
  panel.seq(' ', 'm', 'a')
  // Inside the epic with a task under the cursor: the group is the epic.
  panel.seq(' ', 'm', 'g')
  await ticks()
  assert.deepEqual(calls, [['one', 'e1.a'], ['all'], ['group', 'e1']])
  // A group under the cursor is the one marked.
  panel.store.dispatch({ type: 'enter', path: ['m:m1'], cursor: 'e1' })
  panel.shell.focusPane('list')
  panel.seq(' ', 'm', 'g')
  // At the root, on the milestone row.
  panel.store.dispatch({ type: 'enter', path: [], cursor: 'm1' })
  panel.seq(' ', 'm', 'g')
  await ticks()
  assert.deepEqual(calls.slice(3), [['group', 'e1'], ['group', 'm1']])
  // A write that did not go through says why on the result line.
  refuse = { kind: 'moai-error', message: 'the read file is held' }
  panel.store.dispatch({ type: 'enter', path: ['m:m1', 'e:e1'], cursor: 'e1.b' })
  panel.shell.focusPane('list')
  panel.press('r')
  await ticks()
  assert.deepEqual(calls.at(-1), ['one', 'e1.b'])
  assert.equal(panel.$('.moai-result').textContent, en.moaiError('the read file is held'))
})

test('keys: SPC m g in the "(no milestone)" bucket says where to go and writes nothing', () => {
  const calls = []
  const read = { one: async () => null, all: async () => null, group: async (id) => (calls.push(id), null) }
  const panel = boot({ read, rows: [...ROWS, { id: 'x1', kind: 'issue', title: 'Loose', status: 'todo' }] })
  panel.store.dispatch({ type: 'enter', path: ['m:'], cursor: 'x1' })
  panel.shell.focusPane('list')
  panel.seq(' ', 'm', 'g')
  assert.deepEqual(calls, [])
  assert.equal(panel.$('.moai-result').textContent, en.keys.readNoGroup)
})

test('keys: an archived row and an archived group take no mark — not the milestone around it either', async () => {
  const calls = []
  const read = { one: async (id) => (calls.push(['one', id]), null), all: async () => null, group: async (id) => (calls.push(['group', id]), null) }
  const panel = boot({ read })
  panel.store.dispatch({ type: 'enter', path: ['m:m1'], cursor: 'e1' })
  panel.shell.focusPane('list')
  // An archive-only row is archived (source.ts withArchive tags it).
  panel.store.dispatch({ type: 'view', view: { showArchived: true } })
  panel.store.dispatch({ type: 'archive', rows: [...ROWS, { id: 'e9', kind: 'epic', title: 'Old epic', milestone: 'm1', status: 'done' }, { id: 'e1.z', kind: 'issue', title: 'Zeta', epic: 'e1', status: 'done' }] })
  panel.store.dispatch({ type: 'cursor', id: 'e9' })
  panel.seq(' ', 'm', 'g')
  panel.store.dispatch({ type: 'cursor', id: 'e1.z' })
  panel.press('r')
  await ticks()
  assert.deepEqual(calls, [], 'the archived epic and row take no mark, and m1 is not marked in their stead')
  // An active group under the cursor is still the one marked, and a row that is no group marks the place.
  panel.store.dispatch({ type: 'cursor', id: 'e1' })
  panel.seq(' ', 'm', 'g')
  panel.store.dispatch({ type: 'enter', path: ['m:m1', 'e:e1'], cursor: 'e1.a' })
  panel.shell.focusPane('list')
  panel.seq(' ', 'm', 'g')
  await ticks()
  assert.deepEqual(calls, [['group', 'e1'], ['group', 'e1']])
})
