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
  const p = setup(async endpoint => fail ? { kind: 'not-started' } : { kind: 'passed', output: { id: endpoint.query.id, scope: 'working', offers: [{ scope: 'working', branch: 'w', owner: null, checkout: '.' }], files, total: 1, truncated: false } })
  p.store.dispatch({ type: 'diff-visible', visible: true }); p.store.dispatch({ type: 'cursor', id: 'one' }); await wait(200)
  fail = true
  await assert.rejects(p.shell.refreshFileChanges(p.shell.fileWatch(), '[]'))
  assert.match(p.doc.querySelector('.m-pane-diff').textContent, /a\.rs/)
  p.store.dispatch({ type: 'cursor', id: 'two' }); await wait(200)
  assert.match(p.doc.querySelector('.m-pane-diff').textContent, new RegExp(copyFor('en').failed))
  p.shell.destroy(); p.dom.window.close()
})

// Big diffs in two steps (moai-dy7a.lb0): the list first, then each opened file's patch, two at a time, each read
// with its spinner and its failure in place; nothing late lands on a pane that moved on.
const head = (path, adds = 1, dels = 0, extra = {}) => ({ path, old_path: null, status: 'modified', mark: 'M', adds, dels, binary: false, ...extra })
const listOf = (id, files, extra = {}) => ({ kind: 'passed', output: { id, scope: 'commits', offers: [{ scope: 'commits', branch: null, owner: null, checkout: null }, { scope: 'working', branch: 'w', owner: null, checkout: '.' }], files, total: files.length, truncated: false, vcs_error: null, ...extra } })
const patch = (id, path, text) => ({ kind: 'passed', output: { id, scope: 'commits', offers: [], total: 1, truncated: false, highlight: 'full', files: [{ ...head(path), hunks: [{ old_start: 1, old_len: 1, new_start: 1, new_len: 1, section: '', lines: [{ kind: 'add', old_no: null, new_no: 1, text, spans: null }] }] }] } })
/** A server whose every answer waits until the test lets it go: `answer(match, outcome)` frees the first matching read. */
function server(answers = {}, keep = false) {
  const pending = [], calls = [], cancelled = []
  let inflight = 0, peak = 0
  const request = (endpoint, opts = {}) => {
    calls.push({ ...endpoint, opts })
    if (endpoint.path === '/api/diff') { inflight++; peak = Math.max(peak, inflight) }
    return new Promise(resolve => {
      const job = { endpoint, opts, resolve: outcome => { if (endpoint.path === '/api/diff') inflight--; resolve(outcome) } }
      const auto = answers[endpoint.path]?.(endpoint)
      if (auto) setTimeout(() => job.resolve(auto), 0); else pending.push(job)
    })
  }
  return {
    request, calls, cancelled, peak: () => peak, inflight: () => inflight,
    // `keep`: the reads were on the wire already and their answers come anyway, as a server's can.
    cancelGroup(group) { cancelled.push(group); if (keep) return; for (const job of [...pending]) if (job.opts.group === group) { pending.splice(pending.indexOf(job), 1); job.resolve({ kind: 'cancelled' }) } },
    answer(match, outcome) { const at = pending.findIndex(job => match(job.endpoint)); assert.ok(at >= 0, 'no such read waits'); const [job] = pending.splice(at, 1); job.resolve(typeof outcome === 'function' ? outcome(job.endpoint) : outcome) },
    waiting: match => pending.filter(job => match(job.endpoint)).length,
    diffs: () => calls.filter(c => c.path === '/api/diff'),
  }
}
function shellWith(fake) {
  const dom = new JSDOM('<body><div id="root"></div></body>', { pretendToBeVisual: true })
  const store = createStore(initialState()), doc = dom.window.document
  const shell = mountShell(doc.querySelector('#root'), { document: doc, store, copy: copyFor('en'), language: 'en', intents: { refresh() {}, choose() {}, cancel() {} }, panes: { list: pane, board: pane, detail: pane }, index: () => null, activity: () => ({ phase: 'idle', cancellable: false }), client: { request: fake.request, cancelGroup: fake.cancelGroup } })
  shell.setWidth(1000); store.dispatch({ type: 'detail-visible', visible: false }); store.dispatch({ type: 'diff-visible', visible: true })
  const text = () => doc.querySelector('.m-pane-diff .m-diff-body').textContent
  const open = path => { const d = doc.querySelector(`.m-pane-diff details[data-path="${path}"]`); d.open = true; d.dispatchEvent(new dom.window.Event('toggle')) }
  return { dom, doc, store, shell, text, open, close() { shell.destroy(); dom.window.close() } }
}
const en = copyFor('en')
test('an issue reads its list first, stands every head from it, and asks one patch: the open file, by path and scope', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'moai-ffk9' }); await wait(160)
  assert.deepEqual(fake.calls.map(c => [c.path, c.query]), [['/api/changes', { id: 'moai-ffk9', worktree: '1' }]])
  fake.answer(e => e.path === '/api/changes', listOf('moai-ffk9', [head('a.rs'), head('b.rs'), head('c.rs')])); await wait(10)
  assert.equal(p.doc.querySelectorAll('.m-pane-diff summary').length, 3, 'heads stand before any patch')
  assert.deepEqual(fake.diffs().map(c => c.query), [{ id: 'moai-ffk9', scope: 'commits', path: 'a.rs', worktree: '1' }])
  assert.ok(!fake.calls.some(c => c.path === '/api/diff' && !c.query.path), 'no path-less diff')
  fake.answer(e => e.query.path === 'a.rs', e => patch(e.query.id, 'a.rs', 'line of a')); await wait(10)
  assert.match(p.text(), /line of a/)
  p.close()
})
test('opening every file keeps two patch reads at most; the rest wait their turn in the order opened', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'm' }); await wait(160)
  const names = Array.from({ length: 8 }, (_, i) => `f${i}.rs`)
  fake.answer(e => e.path === '/api/changes', listOf('m', names.map(n => head(n)))); await wait(10)
  for (const name of names.slice(1)) p.open(name)
  await wait(10)
  assert.equal(fake.inflight(), 2); assert.deepEqual(fake.diffs().map(c => c.query.path), ['f0.rs', 'f1.rs'])
  for (let i = 0; i < 8; i++) { fake.answer(e => e.path === '/api/diff', e => patch('m', e.query.path, `x${e.query.path}`)); await wait(5) }
  assert.equal(fake.peak(), 2); assert.deepEqual(fake.diffs().map(c => c.query.path), names)
  assert.equal(p.doc.querySelector('.m-pane-diff .m-spin'), null)
  p.close()
})
test('moving to another issue drops the waiting patch reads, and a late answer of the first never lands on the second', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'A' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('A', [head('same.rs'), head('two.rs'), head('three.rs')])); await wait(10)
  p.open('two.rs'); p.open('three.rs'); await wait(10)
  assert.deepEqual(fake.diffs().map(c => c.query.path), ['same.rs', 'two.rs'])
  p.store.dispatch({ type: 'cursor', id: 'B' })
  assert.ok(fake.cancelled.includes('diff-patch')); assert.equal(fake.waiting(e => e.path === '/api/diff'), 0)
  await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('B', [head('same.rs')])); await wait(10)
  assert.deepEqual(fake.diffs().map(c => [c.query.id, c.query.path]), [['A', 'same.rs'], ['A', 'two.rs'], ['B', 'same.rs']], 'three.rs of A was never sent')
  p.close()
})
test('an answer of the issue left behind, coming after all, is not drawn on the issue now shown', async () => {
  const fake = server({}, true)
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'A' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('A', [head('same.rs')])); await wait(10)
  p.store.dispatch({ type: 'cursor', id: 'B' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('B', [head('same.rs')])); await wait(10)
  fake.answer(e => e.query.id === 'A', patch('A', 'same.rs', 'text of A')); await wait(10)
  assert.doesNotMatch(p.text(), /text of A/)
  await wait(260); assert.ok(p.doc.querySelector('.m-pane-diff details .m-spin'), 'B still reads its own')
  fake.answer(e => e.query.id === 'B', patch('B', 'same.rs', 'text of B')); await wait(10)
  assert.match(p.text(), /text of B/); assert.equal(p.doc.querySelector('.m-spin'), null)
  p.close()
})
test('a scope change reads that scope\'s list, and the old scope\'s late patch stays out', async () => {
  const fake = server({}, true)
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'S' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('S', [head('x.rs')])); await wait(10)
  p.shell.panes().diff.command('scope-next'); await wait(10)
  const scoped = fake.calls.filter(c => c.path === '/api/changes').at(-1); assert.deepEqual(scoped.query, { id: 'S', scope: 'working', worktree: '1' })
  fake.answer(e => e.path === '/api/changes', listOf('S', [head('x.rs', 7)], { scope: 'working' })); await wait(10)
  assert.equal(fake.diffs().at(-1).query.scope, 'working')
  fake.answer(e => e.path === '/api/diff' && e.query.scope === 'commits', patch('S', 'x.rs', 'commits text')); await wait(10)
  assert.doesNotMatch(p.text(), /commits text/)
  fake.answer(e => e.path === '/api/diff', patch('S', 'x.rs', 'working text')); await wait(10)
  assert.match(p.text(), /working text/)
  p.close()
})
test('a 504 list says it timed out, with a retry that reads again; never an empty pane or "no changes"', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'T' }); await wait(160)
  assert.doesNotMatch(p.text(), new RegExp(en.diff.empty))
  fake.answer(e => e.path === '/api/changes', { kind: 'moai-error', message: 'request timed out', code: 'timeout' }); await wait(10)
  assert.match(p.text(), new RegExp(en.diff.timeout)); assert.doesNotMatch(p.text(), new RegExp(en.diff.empty))
  p.doc.querySelector('.m-pane-diff .m-diff-retry').click(); await wait(10)
  assert.equal(fake.calls.filter(c => c.path === '/api/changes').length, 2)
  fake.answer(e => e.path === '/api/changes', { kind: 'not-started' }); await wait(10)
  assert.match(p.text(), new RegExp(en.failed)); assert.doesNotMatch(p.text(), new RegExp(en.diff.timeout))
  p.doc.querySelector('.m-pane-diff .m-diff-retry').click(); await wait(10)
  fake.answer(e => e.path === '/api/changes', listOf('T', [], { vcs_error: { kind: 'failed', said: 'git <failed> here' } })); await wait(10)
  assert.match(p.text(), /git <failed> here/); assert.equal(p.doc.querySelector('.m-pane-diff failed'), null)
  assert.ok(p.doc.querySelector('.m-pane-diff .m-diff-retry'))
  p.close()
})
test('one file\'s timeout stays under its head with a retry; the other files keep their patches', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'F' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('F', [head('ok.rs'), head('bad.rs')])); await wait(10)
  fake.answer(e => e.query.path === 'ok.rs', patch('F', 'ok.rs', 'fine line')); await wait(10)
  p.open('bad.rs'); await wait(10)
  fake.answer(e => e.query.path === 'bad.rs', { kind: 'moai-error', message: 'request timed out', code: 'timeout' }); await wait(10)
  const bad = p.doc.querySelector('.m-pane-diff details[data-path="bad.rs"]')
  assert.match(bad.textContent, new RegExp(en.diff.timeout)); assert.match(p.text(), /fine line/)
  bad.querySelector('.m-diff-retry').click(); await wait(10)
  assert.equal(fake.diffs().filter(c => c.query.path === 'bad.rs').length, 2)
  fake.answer(e => e.query.path === 'bad.rs', patch('F', 'bad.rs', 'second try')); await wait(10)
  assert.match(bad.textContent, /second try/)
  p.close()
})
test('the list\'s spinner waits a moment, stands with its word, and goes on answer and on hiding', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'W' }); await wait(160)
  assert.equal(p.doc.querySelector('.m-pane-diff .m-spin'), null)
  await wait(260)
  assert.ok(p.doc.querySelector('.m-pane-diff .m-spin')); assert.match(p.text(), new RegExp(en.running))
  fake.answer(e => e.path === '/api/changes', listOf('W', [head('a.rs')])); await wait(10)
  assert.equal(p.doc.querySelector('.m-pane-diff > .m-diff > .m-diff-body > .m-diff-state'), null)
  await wait(260); assert.ok(p.doc.querySelector('.m-pane-diff details .m-spin'), 'the file read spins under its head')
  p.store.dispatch({ type: 'diff-visible', visible: false }); await wait(260)
  assert.equal(p.doc.querySelector('.m-spin'), null)
  p.close()
})
test('a refresh with the same list rebuilds nothing and spins nothing; a changed file alone is read again', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'R' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('R', [head('a.rs'), head('b.rs')])); await wait(10)
  fake.answer(e => e.query.path === 'a.rs', patch('R', 'a.rs', 'a one')); await wait(10)
  const node = p.doc.querySelector('.m-pane-diff details[data-path="a.rs"]')
  const refreshing = p.shell.trackerChanged(); await wait(10)
  fake.answer(e => e.path === '/api/changes', listOf('R', [head('a.rs'), head('b.rs')])); await refreshing; await wait(260)
  assert.equal(p.doc.querySelector('.m-pane-diff details[data-path="a.rs"]'), node); assert.equal(p.doc.querySelector('.m-spin'), null)
  assert.equal(fake.diffs().length, 1)
  p.shell.trackerChanged(); await wait(10)
  fake.answer(e => e.path === '/api/changes', listOf('R', [head('a.rs', 2), head('b.rs', 5)])); await wait(10)
  assert.deepEqual(fake.diffs().slice(1).map(c => [c.query.path, c.opts.priority]), [['a.rs', 3]])
  assert.match(p.text(), /a one/); await wait(260); assert.equal(p.doc.querySelector('.m-spin'), null)
  fake.answer(e => e.query.path === 'a.rs', patch('R', 'a.rs', 'a two')); await wait(10)
  assert.match(p.text(), /a two/)
  p.close()
})
test('a file past 5,000 lines is not fetched on opening; its terminal line names this issue, scope and path', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'moai-w47s' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('moai-w47s', [head("src/it's big.rs", 5478, 0)])); await wait(10)
  assert.equal(fake.diffs().length, 0)
  assert.match(p.text(), /5,478/)
  assert.equal(p.doc.querySelector('.m-pane-diff .m-diff-large code').textContent, `moai diff moai-w47s --scope commits --path 'src/it'\\''s big.rs' --worktree`)
  p.doc.querySelector('.m-pane-diff .m-diff-large .m-diff-retry').click(); await wait(10)
  assert.deepEqual(fake.diffs().map(c => c.query.path), ["src/it's big.rs"])
  p.close()
})
test('a path that failed the list\'s checks is never drawn, so never asked for', async () => {
  const fake = server()
  const p = shellWith(fake)
  p.store.dispatch({ type: 'cursor', id: 'P' }); await wait(160)
  fake.answer(e => e.path === '/api/changes', listOf('P', [head('../etc/passwd'), head('/abs'), head('ok/<img src=x>.rs')])); await wait(10)
  assert.deepEqual([...p.doc.querySelectorAll('.m-pane-diff summary .m-diff-path')].map(n => n.textContent), ['ok/<img src=x>.rs'])
  assert.equal(p.doc.querySelector('.m-pane-diff img'), null)
  assert.deepEqual(fake.diffs().map(c => c.query.path), ['ok/<img src=x>.rs'])
  p.close()
})
