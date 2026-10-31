import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const { createCtl } = await panelModule('ctl.ts')
const { createStore } = await panelModule('store.ts')
const { copyFor } = await panelModule('i18n/index.ts')
const { mountShell } = await panelModule('view/shell.ts')
const { PANES } = await panelModule('view/panes.ts')
const { wireKeys } = await panelModule('keys/wiring.ts')
const fixture = JSON.parse(readFileSync(new URL('../../tests/fixtures/ctl-verbs.json', import.meta.url)))

function setup() {
  const dom = new JSDOM('<!doctype html><html><body><div id="moai-app"></div></body></html>', { url: 'http://localhost:6624/', pretendToBeVisual: true })
  const doc = dom.window.document
  const store = createStore()
  const row = { id: 'moai-example', title: 'Example', kind: 'issue', status: 'todo', priority: 2 }
  const shell = mountShell(doc.getElementById('moai-app'), {
    document: doc, store, copy: copyFor('en'), language: 'en', panes: PANES,
    index: () => null, activity: () => ({ phase: 'idle', cancellable: false }),
    intents: { refresh() {}, choose() {}, cancel() {} },
  })
  store.dispatch({ type: 'board-loaded', data: { rows: [row], more: 0, loadedAt: Date.now() } })
  const called = []
  const read = {
    one: async (id) => { called.push(id); return null },
    ids: async (ids) => { called.push([...ids]); return ids.includes('moai-missing') ? { kind: 'moai-error', message: 'missing ID' } : null },
    all: async () => { called.push('all'); return null },
    group: async (id) => { called.push(`group:${id}`); return null },
  }
  const keys = wireKeys({ document: doc, store, shell, copy: copyFor('en'), read, platform: 'other' })
  const ctl = createCtl({ store, keys, shell, read, drawnOrder: () => ['moai-example'],
    withIdentity: (_seq, _token, work) => work(), wikiSearch: async () => ({ slug: 'guide', hits: [{ slug: 'guide' }] }), openProject: () => true })
  return { dom, doc, store, shell, keys, ctl, called }
}

test('fixture verbs run on their named surfaces, and web-only controls have their own units', async () => {
  for (const row of fixture) {
    if (row.verb === 'hello') continue // The server answers hello before a tab receives anything.
    const p = setup()
    const reply = await p.ctl.run({ verb: row.verb, ...row.args }, 1, 'token')
    assert.equal(reply.ok, row.surfaces.includes('web'), `${row.verb} ${JSON.stringify(row.args)}: ${JSON.stringify(reply)}`)
    if (!row.surfaces.includes('web')) assert.equal(reply.code, 'unsupported')
    if (row.verb === 'pane' && row.args.gantt_grid) assert.ok(p.shell.panes().gantt.gridWidth() >= 200)
    p.keys.destroy()
    p.shell.destroy()
    p.dom.window.close()
  }
})

test('theme changes the document and remembered setting, then returns to system', async () => {
  const p = setup()
  assert.equal((await p.ctl.run({ verb: 'theme', name: 'dark' }, 1, 'token')).ok, true)
  assert.equal(p.doc.documentElement.dataset.theme, 'dark')
  assert.equal(p.store.getState().theme, 'dark')
  assert.equal((await p.ctl.run({ verb: 'theme', name: 'system' }, 2, 'token')).ok, true)
  assert.equal(p.doc.documentElement.dataset.theme, undefined)
  p.dom.window.close()
})

test('read through a key binding uses the current read mark path', async () => {
  const p = setup()
  const reply = await p.ctl.run({ verb: 'keys', seqs: ['SPC m a'], text: null }, 3, 'token')
  assert.equal(reply.ok, true)
  await new Promise((resolve) => setImmediate(resolve))
  assert.deepEqual(p.called, ['all'])
  p.dom.window.close()
})

test('ctl read sends all IDs in one atomic request', async () => {
  const p = setup()
  const reply = await p.ctl.run({ verb: 'read', ids: ['moai-example', 'moai-missing'] }, 1, 'token')
  assert.equal(reply.ok, false)
  assert.deepEqual(p.called, [['moai-example', 'moai-missing']])
  p.keys.destroy()
  p.shell.destroy()
  p.dom.window.close()
})

test('a human-owned input blocks ctl; a ctl-opened input accepts text', async () => {
  const p = setup()
  const search = p.shell.toolbar.searchInput
  search.focus()
  assert.equal((await p.ctl.run({ verb: 'screen', name: 'board' }, 1, 'token')).code, 'person_busy')
  search.blur()
  p.doc.body.focus()
  assert.equal((await p.ctl.run({ verb: 'keys', seqs: ['/'], text: 'parser' }, 2, 'token')).ok, true)
  assert.equal(search.value, 'parser')
  p.dom.window.close()
})

test('pane share is the inverse of the detail ratio', async () => {
  const p = setup()
  const reply = await p.ctl.run({ verb: 'pane', list_width: '55', list_height: null, wiki_width: null, gantt_label: null }, 1, 'token')
  assert.equal(reply.ok, true)
  assert.ok(Math.abs(p.store.getState().detail.ratio.right - 0.45) < 0.01)
  p.dom.window.close()
})

test('an expired ctl event cannot change the tab after its caller timed out', async () => {
  const p = setup()
  const before = p.store.getState().tab
  const reply = await p.ctl.run({ verb: 'screen', name: 'gantt', deadline_ms: Date.now() - 1 }, 1, 'token')
  assert.equal(reply.code, 'timeout')
  assert.equal(p.store.getState().tab, before)
  p.keys.destroy()
  p.shell.destroy()
  p.dom.window.close()
})

test('do runs only what a name alone says, where its key would, and owns the field it opens', async () => {
  const p = setup()
  const before = p.store.getState().sort
  const sorted = await p.ctl.run({ verb: 'do', name: 'sort.set' }, 1, 'token')
  assert.equal(sorted.code, 'refused', 'an action whose keys carry arguments has no meaning by name')
  assert.equal(p.store.getState().sort, before)
  assert.equal((await p.ctl.run({ verb: 'do', name: 'constructor' }, 2, 'token')).code, 'unsupported')
  assert.equal((await p.ctl.run({ verb: 'do', name: 'sorter.default' }, 3, 'token')).code, 'refused', 'the sort editor is not open')
  const listed = (await p.ctl.run({ verb: 'actions' }, 4, 'token')).data.actions
  assert.equal(listed.find((row) => row.name === 'sort.set').enabled, false)
  assert.equal((await p.ctl.run({ verb: 'cell', name: 'toString', on: true }, 5, 'token')).code, 'bad_input')
  assert.equal((await p.ctl.run({ verb: 'do', name: 'search.open' }, 6, 'token')).ok, true)
  assert.equal((await p.ctl.run({ verb: 'keys', seqs: [], text: 'parser' }, 7, 'token')).ok, true, 'ctl types into the field it opened')
  p.keys.destroy()
  p.shell.destroy()
  p.dom.window.close()
})

test('a key prefix the person holds is theirs', async () => {
  const p = setup()
  p.keys.router.press('g')
  assert.equal((await p.ctl.run({ verb: 'screen', name: 'board' }, 1, 'token')).code, 'person_busy')
  p.keys.router.reset()
  p.keys.destroy()
  p.shell.destroy()
  p.dom.window.close()
})

test('keys take the explorer\'s spelling, which moai ctl keys checks', async () => {
  for (const seq of ['Ctrl-d', 'C-d', 'gg', 'g g']) {
    const p = setup()
    const reply = await p.ctl.run({ verb: 'keys', seqs: [seq], text: null }, 1, 'token')
    assert.equal(reply.ok, true, `${seq}: ${JSON.stringify(reply)}`)
    p.dom.window.close()
  }
})
