import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'
const { createStore, initialState } = await panelModule('store.ts')
const { mountShell } = await panelModule('view/shell.ts')
const { copyFor } = await panelModule('i18n/index.ts')
const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
const pane = () => ({ render() {}, focus() {} })
function setup(request) {
  const dom = new JSDOM('<body><div id="root"></div></body>', { pretendToBeVisual: true })
  const store = createStore(initialState()), doc = dom.window.document
  const shell = mountShell(doc.querySelector('#root'), { document: doc, store, copy: copyFor('en'), language: 'en', intents: { refresh() {}, choose() {}, cancel() {} }, panes: { list: pane, board: pane, detail: pane }, index: () => null, activity: () => ({ phase: 'idle', cancellable: false }), client: { request } })
  return { dom, doc, store, shell }
}
const answer = id => ({ kind: 'passed', output: { id, scope: 'working', offers: [{ scope: 'working', branch: 'work', owner: null, checkout: '.' }], files: [], total: 0, truncated: false } })
test('list diff debounces cursor changes, preserves scope and does not read while hidden', async () => {
  const calls = []; const p = setup(async endpoint => { calls.push(endpoint); return answer(endpoint.query.id) })
  p.store.dispatch({ type: 'cursor', id: 'one' }); await wait(160); assert.equal(calls.length, 0)
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'two' }); p.store.dispatch({ type: 'cursor', id: 'three' }); await wait(200)
  assert.equal(calls.length, 1); assert.equal(calls[0].query.id, 'three')
  assert.match(p.doc.querySelector('.m-diff-scopes').textContent, /working/)
  p.store.dispatch({ type: 'diff-visible', visible: false }); p.store.dispatch({ type: 'cursor', id: 'four' }); await wait(160); assert.equal(calls.length, 1)
  p.shell.destroy(); p.dom.window.close()
})
test('detail and diff split follows the outer axis and narrow space keeps detail', () => {
  const p = setup(async () => answer('one')); p.shell.setWidth(1000)
  p.store.dispatch({ type: 'diff-visible', visible: true })
  for (const position of ['left', 'right', 'top', 'bottom']) {
    p.store.dispatch({ type: 'detail-position', position })
    const inside = p.doc.querySelector('.m-pane-detail').closest('.m-split')
    assert.equal(inside.dataset.position, position === 'left' || position === 'right' ? 'left' : 'top')
  }
  p.shell.setWidth(400); assert.ok(p.doc.querySelector('.m-diff-collapsed')); assert.ok(p.doc.querySelector('.m-pane-detail')); assert.equal(p.shell.fileWatch(), null)
  p.store.dispatch({ type: 'detail-visible', visible: false }); assert.ok(p.doc.querySelector('.m-pane-diff')); assert.equal(p.doc.querySelector('.m-diff-collapsed'), null)
  p.shell.destroy(); p.dom.window.close()
})
test('scope choices are requested in offered order and stale answers cannot replace a new cursor', async () => {
  let release; const calls = []
  const p = setup(async endpoint => {
    calls.push(endpoint)
    if (endpoint.query.id === 'slow') await new Promise(resolve => { release = resolve })
    return { kind: 'passed', output: { id: endpoint.query.id, scope: endpoint.query.scope ?? 'commits', offers: [{ scope: 'commits', branch: null, owner: null, checkout: null }, { scope: 'branch', branch: 'work', owner: 'epic', checkout: '.' }], files: [], truncated: false } }
  })
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'slow' }); await wait(160)
  p.store.dispatch({ type: 'cursor', id: 'new' }); await wait(160); release(); await wait(20)
  assert.match(p.doc.querySelector('.m-diff-scopes').textContent, /Epic/)
  p.shell.panes().diff.command('scope-next'); await wait(20)
  assert.equal(calls.at(-1).query.id, 'new'); assert.equal(calls.at(-1).query.scope, 'branch')
  p.shell.destroy(); p.dom.window.close()
})
test('ctl toggles diff and reports it', async () => {
  const { wireKeys } = await panelModule('keys/wiring.ts')
  const { createCtl } = await panelModule('ctl.ts')
  const p = setup(async () => answer('one'))
  const keys = wireKeys({ document: p.doc, store: p.store, shell: p.shell, copy: copyFor('en'), platform: 'other' })
  const ctl = createCtl({ store: p.store, shell: p.shell, keys, read: {}, drawnOrder: () => [], withIdentity: (_s, _t, work) => work() })
  assert.equal((await ctl.run({ verb: 'view', name: 'diff', on: true }, 1, 'token')).ok, true)
  const state = await ctl.run({ verb: 'state' }, 2, 'token'); assert.equal(state.data.diff.shown, true)
  assert.equal((await ctl.run({ verb: 'view', name: 'diff', on: false }, 3, 'token')).ok, true)
  assert.equal(p.store.getState().diff.visible, false)
  keys.destroy(); p.shell.destroy(); p.dom.window.close()
})
test('working scope exposes its visible file stamp and commit scope follows tracker changes', async () => {
  const calls = []; let scope = 'working'
  const p = setup(async endpoint => {
    calls.push(endpoint)
    return { kind: 'passed', output: { id: endpoint.query.id, scope, offers: [{ scope, checkout: 'side', branch: 'work', owner: null }], files: [{ path: 'a.rs', old_path: null, status: 'modified', adds: 1, dels: 1, binary: false, hunks: [] }], truncated: false } }
  })
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'one' }); await wait(160)
  const watch = p.shell.fileWatch(); assert.equal(watch.root, 'side'); assert.equal(watch.file.path, 'a.rs'); assert.match(watch.changeKey, /working/)
  p.store.dispatch({ type: 'diff-visible', visible: false }); scope = 'commits'; p.store.dispatch({ type: 'diff-visible', visible: true }); await wait(160)
  const before = calls.length; p.shell.trackerChanged(); await wait(20); assert.equal(calls.length, before + 1)
  p.shell.destroy(); p.dom.window.close()
})
test('the diff pane holds the keys and keeps its focus across renders', async () => {
  const p = setup(async () => answer('one')); p.shell.setWidth(1000)
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'one' }); await wait(200)
  p.shell.focusPane('diff')
  assert.equal(p.store.getState().focus, 'diff')
  const focused = p.doc.activeElement
  assert.ok(p.doc.querySelector('.m-pane-diff').contains(focused))
  p.store.dispatch({ type: 'view', view: { hideDeferred: true } })
  assert.equal(p.doc.activeElement, focused)
  p.store.dispatch({ type: 'detail-visible', visible: false }); p.shell.focusPane('diff')
  assert.equal(p.store.getState().focus, 'diff')
  p.shell.destroy(); p.dom.window.close()
})
test('a failed background refresh keeps the diff and a failed read says so', async () => {
  let fail = false
  const files = [{ path: 'a.rs', old_path: null, status: 'modified', adds: 1, dels: 0, binary: false, hunks: [] }]
  const p = setup(async endpoint => fail ? { kind: 'cancelled' } : { kind: 'passed', output: { id: endpoint.query.id, scope: 'working', offers: [{ scope: 'working', branch: 'w', owner: null, checkout: '.' }], files, total: 1, truncated: false } })
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'one' }); await wait(200)
  fail = true
  await assert.rejects(p.shell.refreshFileChanges(p.shell.fileWatch(), '[]'))
  assert.match(p.doc.querySelector('.m-pane-diff').textContent, /a\.rs/)
  p.store.dispatch({ type: 'cursor', id: 'two' }); await wait(200)
  assert.match(p.doc.querySelector('.m-pane-diff').textContent, new RegExp(copyFor('en').failed))
  p.shell.destroy(); p.dom.window.close()
})
