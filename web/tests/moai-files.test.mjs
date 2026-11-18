import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'
const { mountFiles } = await panelModule('view/files.ts')
const { COPY } = await panelModule('i18n/index.ts')
const { initialState } = await panelModule('store.ts')

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms))
const words = { search: 'Find', binary: 'Binary', truncated: 'Truncated', readError: 'Read error', directoryError: 'Directory error', searchError: 'Search error', matches: 'matches', ignoredUnavailable: 'No ignore rules', expansionLimit: 'Limit', unavailable: 'Unavailable' }
const entry = (name, kind = 'file') => ({ name, kind, openable: true, size: 1 })
function page(request, checkouts = [{ key: '.', branch: '', main: true, current: true, issue: null, milestone: null }]) {
  const dom = new JSDOM('<body><div id="files"></div></body>', { pretendToBeVisual: true })
  const root = dom.window.document.querySelector('#files')
  const ctx = { document: dom.window.document, copy: { ...COPY.en, files: { ...COPY.en.files, ...words } }, language: 'en', client: { request }, intents: { dispatch() {}, closeDetail() {} }, layout: () => ({ width: 1000, mainWidth: 600, position: 'right', detailVisible: true }), focusPane() {} }
  const original = ctx.client.request
  ctx.client.request = async (endpoint) => endpoint.path === '/api/checkouts' ? { kind: 'passed', output: { checkouts } } : original(endpoint)
  const view = mountFiles(root, ctx)
  return { dom, root, view, state: initialState() }
}
test('files are fetched on opening and hostile names and content remain text', async () => {
  const calls = []
  const hostile = '<img onerror=alert(1)>'
  const p = page(async ({ path }) => { calls.push(path); return { kind: 'passed', output: path === '/api/files' ? { entries: [entry(hostile)] } : { path: hostile, text: '</pre><script>bad</script>', binary: false, truncated: false, spans: null } } })
  assert.deepEqual(calls, [])
  p.view.render(p.state, null); await wait(10)
  assert.equal(p.root.querySelector('img'), null)
  assert.match(p.root.textContent, /<img/)
  p.root.querySelector('[role=treeitem]').click(); await wait(150)
  assert.equal(p.root.querySelector('script'), null)
  assert.match(p.root.textContent, /<script>bad/)
  p.view.destroy(); p.dom.window.close()
})
test('moving quickly ignores an earlier file response', async () => {
  let release
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('one'), entry('two')] } }
    if (query.path === 'one') await new Promise(resolve => { release = resolve })
    return { kind: 'passed', output: { path: query.path, text: query.path, binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(10)
  p.root.querySelectorAll('[role=treeitem]')[0].click(); await wait(120)
  p.root.querySelectorAll('[role=treeitem]')[1].click(); await wait(120)
  release(); await wait(20)
  assert.equal(p.root.querySelector('.m-files-content').textContent.trim(), '1two')
  p.view.destroy(); p.dom.window.close()
})
test('search expands ancestors without hiding siblings and clearing restores the tree', async () => {
  const calls = []
  const p = page(async ({ path, query }) => {
    calls.push([path, query])
    if (path === '/api/files') return { kind: 'passed', output: { entries: query.dir === '.' ? [entry('dir', 'dir'), entry('other')] : [entry('needle')] } }
    if (path === '/api/find') return { kind: 'passed', output: { matches: [{ path: 'dir/needle', start: 4, end: 10 }], total: 1, truncated: false, walk_truncated: false, ignored_unavailable: false } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(10)
  const key = (key, target = p.root.querySelector('.m-files-tree')) => p.view.handleKey(new p.dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...(target ? {} : {}) }))
  key('/')
  const input = p.root.querySelector('input'); input.value = 'needle'; input.dispatchEvent(new p.dom.window.Event('input', { bubbles: true })); await wait(250)
  assert.match(p.root.textContent, /needle/); assert.match(p.root.textContent, /other/); assert.equal(p.root.querySelector('mark').textContent, 'needle')
  p.view.handleKey({ key: 'Escape', target: input }); assert.doesNotMatch(p.root.textContent, /needle/); assert.match(p.root.textContent, /other/)
  assert.ok(calls.every(([, query]) => !Object.values(query).some(v => v === '..' || v.includes('../'))))
  p.view.destroy(); p.dom.window.close()
})
test('recursive expansion skips ignored directories and bounds each new listing', async () => {
  const requests = []
  const p = page(async ({ path, query }) => {
    requests.push([path, query])
    if (path === '/api/ignored') return { kind: 'passed', output: { dirs: ['ignored/'] } }
    if (path === '/api/files') return { kind: 'passed', output: { entries: query.dir === '.' ? [entry('ignored', 'dir'), entry('src', 'dir')] : [entry('a')], truncated: false } }
    return { kind: 'passed', output: {} }
  })
  p.view.render(p.state, null); await wait(10)
  p.view.handleKey({ key: 'Tab', target: p.root.querySelector('.m-files-tree') }); await wait(30)
  assert.ok(requests.some(([path, query]) => path === '/api/files' && query.dir === 'src' && query.limit === '5000'))
  assert.ok(!requests.some(([path, query]) => path === '/api/files' && query.dir === 'ignored'))
  p.view.destroy(); p.dom.window.close()
})
test('checkout changes discard late replies and changed-only includes deleted files', async () => {
  let release
  const calls = []
  const roots = [{ key: '.', branch: 'main', main: true, current: true, issue: null }, { key: 'side', branch: 'side', main: false, current: false, issue: null }]
  const p = page(async ({ path, query }) => {
    calls.push([path, query])
    if (path === '/api/files') {
      if (query.root === '.') await new Promise(resolve => { release = resolve })
      return { kind: 'passed', output: { entries: [entry(query.root === '.' ? 'old' : 'new')], truncated: false } }
    }
    if (path === '/api/changes') return { kind: 'passed', output: { files: [{ path: 'gone', status: 'deleted', mark: 'D', old_path: null, adds: 0, dels: 1, binary: false }] } }
    return { kind: 'passed', output: {} }
  }, roots)
  p.view.render(p.state, null); await wait(20)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(20)
  release(); await wait(20)
  assert.match(p.root.textContent, /new/); assert.doesNotMatch(p.root.textContent, /old/)
  p.view.command('changed'); await wait(20)
  assert.match(p.root.textContent, /gone/); assert.equal(p.root.querySelector('.m-files-c-mark').textContent, 'D'); assert.doesNotMatch(p.root.querySelector('.m-files-tree').textContent, /new/)
  p.view.command('changed'); await wait(20); assert.match(p.root.querySelector('.m-files-tree').textContent, /new/)
  assert.ok(calls.some(([path, query]) => path === '/api/changes' && query.root === 'side'))
  p.view.destroy(); p.dom.window.close()
})
test('file diff uses the selected checkout and only a listed changed path', async () => {
  const calls = []
  const p = page(async ({ path, query }) => {
    calls.push([path, query])
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('changed'), entry('same')], truncated: false } }
    if (path === '/api/changes') return { kind: 'passed', output: { files: [{ path: 'changed', status: 'modified', old_path: null, adds: 1, dels: 0, binary: false }] } }
    if (path === '/api/diff') return { kind: 'passed', output: { files: [{ path: 'changed', status: 'modified', old_path: null, adds: 1, dels: 0, binary: false, hunks: [{ old_start: 0, old_len: 0, new_start: 1, new_len: 1, section: '', lines: [{ kind: 'add', old_no: null, new_no: 1, text: 'new', spans: null }] }] }], truncated: false } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(20)
  p.root.querySelectorAll('[role=treeitem]')[0].click(); p.view.command('diff'); await wait(150)
  assert.ok(calls.some(([path, query]) => path === '/api/diff' && query.path === 'changed' && query.root === '.'))
  assert.match(p.root.textContent, /new/)
  p.root.querySelectorAll('[role=treeitem]')[1].click(); await wait(30)
  assert.ok(!calls.some(([path, query]) => path === '/api/diff' && query.path === 'same'))
  p.view.command('detail'); assert.match(p.root.textContent, /No issue/)
  p.view.destroy(); p.dom.window.close()
})
test('file detail and diff toggles are independent and narrow panels keep detail', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a')], truncated: false } }
    if (path === '/api/changes') return { kind: 'passed', output: { files: [] } }
    return { kind: 'passed', output: { path: query?.path, text: '', binary: false, truncated: false, spans: null } }
  })
  Object.defineProperty(p.root, 'clientWidth', { configurable: true, value: 1000 })
  p.view.render(p.state, null); await wait(20)
  p.view.command('diff'); assert.ok(p.root.querySelector('.m-files-right > .m-diff'))
  p.view.command('detail'); assert.ok(p.root.querySelector('.m-files-right > .m-split'))
  Object.defineProperty(p.root, 'clientWidth', { configurable: true, value: 400 })
  p.view.render(p.state, null); assert.ok(p.root.querySelector('.m-files-right > .m-files-issue')); assert.match(p.root.textContent, /collapsed/i); assert.equal(p.view.fileWatch().changeKey, 'root:.', 'the list is watched whatever the right pane shows')
  p.view.command('diff'); assert.ok(p.root.querySelector('.m-files-right > .m-files-issue'))
  p.view.command('detail'); assert.ok(p.root.querySelector('.m-files-right > .m-files-content'))
  assert.equal(p.state.detail.visible, true)
  p.view.destroy(); p.dom.window.close()
})
test('refreshed directory and file keep the selected path and replace content', async () => {
  let changed = false
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: changed ? [entry('before'), entry('chosen')] : [entry('chosen')], stamp: changed ? 'dir2' : 'dir1', truncated: false } }
    if (path === '/api/file') return { kind: 'passed', output: { path: query.path, text: changed ? 'new text' : 'old text', stamp: changed ? 'file2' : 'file1', binary: false, truncated: false, spans: null } }
    return { kind: 'passed', output: {} }
  })
  p.view.render(p.state, null); await wait(20); p.root.querySelector('[role=treeitem]').click(); await wait(120)
  const watch = p.view.fileWatch(); assert.equal(watch.file.path, 'chosen'); assert.equal(watch.dirs[0].stamp, 'dir1')
  changed = true; await p.view.refreshFileStamps(watch, ['.'], true); await wait(20)
  assert.equal(p.root.querySelector('[aria-selected=true] .m-files-label').textContent, 'chosen')
  assert.match(p.root.querySelector('.m-files-content').textContent, /new text/)
  assert.equal(p.view.fileWatch().file.stamp, 'file2')
  p.view.destroy(); p.dom.window.close()
})
test('a precautionary reread with unchanged data leaves the DOM alone', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a')], stamp: 'same-dir', truncated: false } }
    return { kind: 'passed', output: { path: query.path, text: 'same', stamp: 'same-file', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(20); p.root.querySelector('[role=treeitem]').click(); await wait(130)
  const row = p.root.querySelector('[role=treeitem]'), lineNode = p.root.querySelector('.m-file-line')
  await p.view.refreshFileStamps(p.view.fileWatch(), ['.'], true)
  assert.equal(p.root.querySelector('[role=treeitem]'), row); assert.equal(p.root.querySelector('.m-file-line'), lineNode)
  p.view.destroy(); p.dom.window.close()
})
test('a render keeps the content pane where the person left it and a repaint of the same file draws it once', async () => {
  let reads = 0
  const text = Array.from({ length: 1000 }, (_, i) => `line ${i}`).join('\n') + '\n'
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a.txt')], stamp: '1:1' } }
    return { kind: 'passed', output: { path: query.path, text: text + (reads++ ? 'more\n' : ''), binary: false, truncated: false, spans: null, stamp: '1:1' } }
  })
  p.view.render(p.state, null); await wait(10)
  p.view.handleKey({ key: 'j', target: p.dom.window.document.body })
  const content = p.root.querySelector('.m-files-content')
  for (let n = 0; n < 400 && !content.querySelector('.m-file-line'); n++) await wait(5)
  // The first paint is still drawing its chunks when the refresh paints the same path again.
  await p.view.refreshFileStamps(p.view.fileWatch(), [], true, { dirs: {}, file: '2:2' })
  for (let n = 0; n < 400 && content.querySelectorAll('.m-file-line').length < 1002; n++) await wait(5)
  await wait(50)
  assert.equal(content.querySelectorAll('.m-file-line').length, 1002)
  content.focus(); p.view.render(p.state, null)
  assert.equal(p.dom.window.document.activeElement, content)
  p.view.destroy(); p.dom.window.close()
})
test('a shown file that vanished is said once instead of failing every refresh', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('gone.txt')], stamp: '1:1' } }
    if (path === '/api/file' && query.path === 'gone.txt' && p.gone) return { kind: 'refused', status: 404 }
    return { kind: 'passed', output: { path: query.path, text: 'here', binary: false, truncated: false, spans: null, stamp: '1:1' } }
  })
  p.view.render(p.state, null); await wait(10)
  p.view.handleKey({ key: 'j', target: p.dom.window.document.body }); await wait(150)
  p.gone = true
  await p.view.refreshFileStamps(p.view.fileWatch(), ['.'], true, { dirs: { '.': null }, file: null })
  assert.match(p.root.querySelector('.m-files-content').textContent, /Read error/)
  assert.equal(p.view.fileWatch().file.stamp, null)
  p.view.destroy(); p.dom.window.close()
})
test('browser shortcuts and the checkout select keep their keys', async () => {
  const p = page(async ({ path, query }) => path === '/api/files' ? { kind: 'passed', output: { entries: [entry('a'), entry('b')] } } : { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } })
  p.view.render(p.state, null); await wait(10)
  const body = p.dom.window.document.body
  assert.equal(p.view.handleKey({ key: 'l', ctrlKey: true, target: body }), false)
  assert.equal(p.view.handleKey({ key: 'ArrowLeft', altKey: true, target: body }), false)
  assert.equal(p.view.handleKey({ key: 'ArrowDown', target: p.root.querySelector('select') }), false)
  assert.equal(p.view.handleKey({ key: 'd', ctrlKey: true, target: body }), true)
  p.view.destroy(); p.dom.window.close()
})

// Changed-only raises the diff pane and leaves it as it found it (moai-dy7a.1yj).
const roots2 = [{ key: '.', branch: 'main', main: true, current: true, issue: null }, { key: 'side', branch: 'side', main: false, current: false, issue: null }]
function autoPage(extra = {}) {
  const calls = []
  const p = page(async ({ path, query }) => {
    calls.push([path, query])
    if (extra.hook) { const r = await extra.hook(path, query); if (r) return r }
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a')], truncated: false } }
    if (path === '/api/changes') return extra.failChanges ? { kind: 'failed' } : { kind: 'passed', output: { files: [{ path: 'a', status: 'modified', old_path: null, adds: 1, dels: 0, binary: false }] } }
    if (path === '/api/diff') return { kind: 'passed', output: { files: [], truncated: false } }
    return { kind: 'passed', output: { path: query?.path, text: '', binary: false, truncated: false, spans: null } }
  }, roots2)
  Object.defineProperty(p.root, 'clientWidth', { configurable: true, value: 1000 })
  const pressed = name => p.root.querySelectorAll('.m-files-toolbar button')[name].getAttribute('aria-pressed')
  return { p, calls, diffPressed: () => pressed(1), detailPressed: () => pressed(2), diffReads: () => calls.filter(([path]) => path === '/api/diff').length }
}
test('entering changed-only raises the diff pane and leaving lowers it', async () => {
  const { p, diffPressed, diffReads } = autoPage()
  p.view.render(p.state, null); await wait(20)
  assert.equal(p.view.filesState().diff, false)
  p.view.command('changed'); await wait(200)
  assert.equal(p.view.filesState().diff, true); assert.equal(diffPressed(), 'true')
  assert.ok(p.root.querySelector('.m-files-right > .m-diff')); assert.ok(diffReads() > 0)
  assert.equal(p.view.filesState().detail, false)
  p.view.command('changed'); await wait(20)
  assert.equal(p.view.filesState().diff, false); assert.equal(diffPressed(), 'false')
  assert.ok(p.root.querySelector('.m-files-right > .m-files-content'))
  assert.equal(p.state.diff.visible, initialState().diff.visible)
  p.view.destroy(); p.dom.window.close()
})
test('a diff pane the hand raised stays when changed-only is left', async () => {
  const { p } = autoPage()
  p.view.render(p.state, null); await wait(20)
  p.view.command('diff'); p.view.command('changed'); await wait(30)
  assert.equal(p.view.filesState().diff, true)
  p.view.command('changed'); await wait(20)
  assert.equal(p.view.filesState().diff, true)
  p.view.destroy(); p.dom.window.close()
})
test('a hand on the diff pane inside changed-only keeps its value on leaving', async () => {
  const { p } = autoPage()
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(30); p.view.command('diff'); assert.equal(p.view.filesState().diff, false)
  p.view.command('changed'); await wait(20); assert.equal(p.view.filesState().diff, false)
  p.view.command('changed'); await wait(30); assert.equal(p.view.filesState().diff, true)
  p.view.command('diff'); p.view.command('diff'); assert.equal(p.view.filesState().diff, true)
  p.view.command('changed'); await wait(20)
  assert.equal(p.view.filesState().diff, true)
  p.view.destroy(); p.dom.window.close()
})
test('switching checkout leaves changed-only and lowers a pane it raised', async () => {
  const { p } = autoPage()
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(30); assert.equal(p.view.filesState().diff, true)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(30)
  assert.equal(p.view.filesState().changed, false); assert.equal(p.view.filesState().diff, false)
  p.view.destroy(); p.dom.window.close()
})
test('a changes answer discarded by a checkout switch never raises the diff pane', async () => {
  let release
  const { p } = autoPage({ hook: async (path, query) => { if (path === '/api/changes' && query.root === '.') await new Promise(resolve => { release = resolve }) } })
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(10)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(30)
  release(); await wait(30)
  assert.equal(p.view.filesState().diff, false); assert.equal(p.view.filesState().changed, false)
  p.view.destroy(); p.dom.window.close()
})
test('an unreadable change list neither enters changed-only nor raises the diff pane', async () => {
  const { p, diffReads } = autoPage({ hook: async path => { if (path === '/api/changes') throw new Error('down') } })
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(30)
  assert.equal(p.view.filesState().changed, false); assert.equal(p.view.filesState().diff, false); assert.equal(diffReads(), 0)
  p.view.destroy(); p.dom.window.close()
})
test('changed-only touches neither the list diff pane nor the remembered settings', async () => {
  const { p } = autoPage()
  const before = JSON.stringify(p.state)
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(30); p.view.command('changed'); await wait(20)
  assert.equal(JSON.stringify(p.state), before)
  p.view.destroy(); p.dom.window.close()
})
test('the three toggles are icon buttons whose name carries the state', async () => {
  const { p } = autoPage()
  p.view.render(p.state, null); await wait(20)
  const buttons = [...p.root.querySelectorAll('.m-files-toolbar button')]
  assert.equal(buttons.length, 3); assert.ok(buttons.every(b => b.classList.contains('m-iconbtn') && b.querySelector('svg')))
  const named = () => buttons.map(b => [b.getAttribute('aria-pressed'), b.getAttribute('aria-label'), b.title])
  for (const [pressed, label, title] of named()) { assert.equal(pressed, 'false'); assert.match(label, / · off$/); assert.equal(title, label) }
  buttons[2].focus(); buttons[2].click()
  assert.equal(p.dom.window.document.activeElement, buttons[2])
  assert.deepEqual(named()[2].slice(0, 2), ['true', 'Detail · on'])
  buttons[1].click()
  assert.deepEqual(named()[1].slice(0, 2), ['true', 'Diff · on'])
  p.view.destroy(); p.dom.window.close()
})

// ---- the tree rows are the list's rows (moai-dy7a.vc8) ----
const tree = { '.': [entry('a', 'dir'), entry('z')], a: [entry('b', 'dir'), entry('c')], 'a/b': [entry('d')] }
const treeRequest = (calls = []) => async ({ path, query }) => {
  calls.push([path, query])
  if (path === '/api/files') return { kind: 'passed', output: { entries: tree[query.dir] ?? [], stamp: 's', truncated: false } }
  return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
}
const rowsOf = p => [...p.root.querySelectorAll('[role=treeitem]')]
const twigOf = row => [...row.querySelectorAll('.moai-tw')].map(t => t.dataset.tw)
const nameOf = row => row.querySelector('.m-files-label').textContent
const press = (p, key) => p.view.handleKey(new p.dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }))
async function opened() {
  const calls = []
  const p = page(treeRequest(calls)); p.view.render(p.state, null); await wait(10)
  rowsOf(p)[0].click(); await wait(10); rowsOf(p)[1].click(); await wait(10)
  return { p, calls }
}
test('file rows are the list row and carry a branch per level', async () => {
  const { p } = await opened()
  const rows = rowsOf(p)
  assert.deepEqual(rows.map(nameOf), ['a', 'b', 'd', 'c', 'z'])
  assert.deepEqual(rows.map(twigOf), [[], ['tee'], ['pipe', 'end'], ['end'], []])
  assert.deepEqual(rows.map(r => r.getAttribute('aria-level')), ['1', '2', '3', '2', '1'])
  assert.deepEqual(rows.map(r => r.querySelectorAll('.moai-tw').length), rows.map(r => Number(r.getAttribute('aria-level')) - 1))
  assert.ok(rows.every(r => r.classList.contains('m-row') && !/m-files-depth-/.test(r.className)))
  assert.ok(rows.every(r => r.querySelector('.moai-twig') === null || r.querySelector('.moai-twig').getAttribute('aria-hidden') === 'true'))
  p.view.destroy(); p.dom.window.close()
})
test('a row always has the mark cell and the fold slot, so every name starts in one place', async () => {
  const { p } = await opened()
  for (const row of rowsOf(p)) {
    assert.ok(row.children[0].classList.contains('m-files-c-mark')); assert.equal(row.children[0].textContent, '')
    const name = row.children[1]; assert.ok(name.classList.contains('m-files-name'))
    const parts = row.querySelectorAll('.moai-tw').length
    assert.equal([...name.children].filter(c => !c.classList.contains('moai-twig'))[0].classList.contains('m-files-fold'), true)
    assert.equal(name.querySelector('.moai-twig') ? 1 : 0, parts ? 1 : 0)
  }
  const [dir, , , , file] = rowsOf(p)
  assert.ok(dir.querySelector('.m-files-fold svg')); assert.equal(file.querySelector('.m-files-fold svg'), null)
  assert.equal(p.root.querySelector('.m-files-tree button button'), null)
  p.view.destroy(); p.dom.window.close()
})
test('going into a directory starts the branch again from nothing, and a ".." row has none', async () => {
  const { p } = await opened()
  rowsOf(p)[0].click(); await wait(10); p.view.handleKey({ key: 'Enter', target: p.root.querySelector('.m-files-tree') }); await wait(20)
  const rows = rowsOf(p)
  assert.equal(nameOf(rows[0]), '↰ ..'); assert.deepEqual(twigOf(rows[0]), [])
  const names = rows.map(nameOf); assert.ok(names.includes('b'))
  assert.deepEqual(twigOf(rows[names.indexOf('b')]), [])
  p.view.destroy(); p.dom.window.close()
})
test('clicks and keys do what they did: pick, fold once, move the cursor, refuse a closed row', async () => {
  const calls = []
  const p = page(async req => req.path === '/api/files' ? { kind: 'passed', output: { entries: req.query.dir === '.' ? [entry('a', 'dir'), entry('x'), { name: 'locked', kind: 'file', openable: false, size: 1 }] : [entry('in')], stamp: 's', truncated: false } } : (calls.push(req.path), { kind: 'passed', output: { path: req.query.path, text: 'hi', binary: false, truncated: false, spans: null } }))
  p.view.render(p.state, null); await wait(10)
  const dirRow = rowsOf(p)[0]
  dirRow.querySelector('.m-files-fold').click(); await wait(10)
  assert.equal(rowsOf(p)[0].getAttribute('aria-expanded'), 'true'); assert.deepEqual(rowsOf(p).map(nameOf), ['a', 'in', 'x', 'locked'])
  rowsOf(p)[0].querySelector('.m-files-fold').click(); await wait(10)
  assert.equal(rowsOf(p)[0].getAttribute('aria-expanded'), 'false'); assert.deepEqual(rowsOf(p).map(nameOf), ['a', 'x', 'locked'])
  press(p, 'j'); assert.equal(rowsOf(p)[1].getAttribute('aria-selected'), 'true'); assert.equal(rowsOf(p)[0].getAttribute('aria-selected'), 'false')
  const locked = rowsOf(p)[2]; assert.equal(locked.disabled, true); assert.equal(locked.querySelector('.m-files-meta').textContent, 'Unavailable')
  await wait(130); const reads = calls.length; locked.click(); await wait(130); assert.equal(calls.length, reads); assert.equal(rowsOf(p)[2].getAttribute('aria-selected'), 'false')
  rowsOf(p)[1].click(); await wait(150); assert.equal(rowsOf(p)[1].getAttribute('aria-selected'), 'true'); assert.equal(calls.length, reads + 1)
  p.view.destroy(); p.dom.window.close()
})
test('a search wraps the hit in the name only and takes it off again, leaving the branch and the mark cell', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: query.dir === '.' ? [entry('dir', 'dir'), entry('other')] : [entry('needle')] } }
    if (path === '/api/find') return { kind: 'passed', output: { matches: [{ path: 'dir/needle', start: 4, end: 10 }], total: 1, truncated: false, walk_truncated: false, ignored_unavailable: false } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(10); press(p, '/')
  const input = p.root.querySelector('input'); input.value = 'needle'; input.dispatchEvent(new p.dom.window.Event('input', { bubbles: true })); await wait(250)
  const hit = rowsOf(p).find(r => nameOf(r) === 'needle')
  assert.equal(hit.querySelector('mark').parentNode, hit.querySelector('.m-files-label')); assert.deepEqual(twigOf(hit), ['end'])
  assert.ok(hit.querySelector('.m-files-c-mark')); assert.ok(hit.querySelector('.m-files-fold'))
  p.view.handleKey({ key: 'Escape', target: input })
  assert.equal(p.root.querySelector('mark'), null); assert.ok(rowsOf(p).every(r => r.querySelector('.m-files-c-mark')))
  p.view.destroy(); p.dom.window.close()
})
const changedPage = (files, listing, extra = {}) => page(async ({ path, query }) => {
  if (path === '/api/files') return { kind: 'passed', output: { entries: listing, stamp: 's', truncated: false } }
  if (path === '/api/changes') return { kind: 'passed', output: { files, ...extra } }
  return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
})
const change = (path, status, adds, dels, old = null, mark = '') => ({ path, status, mark, old_path: old, adds, dels, binary: false })
test('changed-only draws the same branch, the letter in its cell, sums on directories and the old name text', async () => {
  const hostile = '<img src=x onerror=alert(1)>'
  const p = changedPage([change('src/a.rs', 'modified', 3, 1, null, 'M'), change('src/b.rs', 'added', 4, 0, null, 'A'), change('src/gone.rs', 'deleted', 0, 9, null, 'D'), change('src/new.rs', 'renamed', 0, 0, hostile, 'R'), change('top.md', 'modified', 1, 1, null, 'M')], [entry('x')], { dirs: { src: 'M' } })
  p.view.render(p.state, null); await wait(10); p.view.command('changed'); await wait(30)
  const rows = rowsOf(p), by = name => rows.find(r => nameOf(r).startsWith(name))
  assert.deepEqual(rows.map(r => r.querySelector('.m-files-label').firstChild.textContent), ['src', 'a.rs', 'b.rs', 'gone.rs', 'new.rs', 'top.md'])
  assert.deepEqual(rows.map(twigOf), [[], ['tee'], ['tee'], ['tee'], ['end'], []])
  const dir = by('src'); assert.equal(dir.querySelector('.m-files-meta').textContent, '+7 −10'); assert.ok(dir.querySelector('.m-files-faint .m-count-add'))
  assert.equal(dir.querySelector('.m-files-c-mark').textContent, 'M', 'a directory carries the map\'s letter')
  const a = by('a.rs'); assert.equal(a.querySelector('.m-files-c-mark').textContent, 'M'); assert.ok(a.querySelector('.m-files-c-mark').classList.contains('m-mark-M'))
  assert.equal(a.querySelector('.m-files-meta').textContent, '+3 −1'); assert.ok(a.querySelector('.m-count-add')); assert.ok(a.querySelector('.m-count-del'))
  assert.ok(by('b.rs').querySelector('.m-files-c-mark').classList.contains('m-mark-A'))
  const gone = by('gone.rs'); assert.ok(gone.querySelector('.m-files-label').classList.contains('m-files-gone')); assert.ok(gone.querySelector('.m-files-c-mark').classList.contains('m-mark-D'))
  const renamed = by('new.rs'); assert.ok(renamed.querySelector('.m-mark-R')); assert.equal(renamed.querySelector('.m-files-from').textContent, ` ← ${hostile}`)
  assert.equal(p.root.querySelector('img'), null); assert.equal(p.root.querySelector('.m-files-tree button button'), null)
  assert.ok(rows.every(r => r.children[0].classList.contains('m-files-c-mark')))
  p.view.destroy(); p.dom.window.close()
})
test('hostile names never become classes or attributes of the row', async () => {
  const p = page(async ({ path, query }) => ({ kind: 'passed', output: path === '/api/files' ? { entries: [entry('x" onclick="bad'), entry('"><b>b</b>.rs')] } : { path: query.path, text: '', binary: false, truncated: false, spans: null } }))
  p.view.render(p.state, null); await wait(10)
  for (const row of rowsOf(p)) { assert.equal(row.getAttribute('onclick'), null); assert.equal(row.querySelector('b'), null); assert.ok(row.className.split(' ').every(c => /^[\w-]+$/.test(c))) }
  p.view.destroy(); p.dom.window.close()
})
test('a refresh that finds the same tree keeps the row nodes and the cursor row', async () => {
  const { p } = await opened()
  press(p, 'j'); const before = rowsOf(p), at = before.findIndex(r => r.getAttribute('aria-selected') === 'true')
  await p.view.refreshFileStamps(p.view.fileWatch(), ['.', 'a', 'a/b'], true); await wait(20)
  const after = rowsOf(p); assert.deepEqual(after.map(nameOf), before.map(nameOf)); assert.equal(after[at].getAttribute('aria-selected'), 'true')
  assert.ok(before.every((r, i) => r === after[i]))
  p.view.destroy(); p.dom.window.close()
})
test('the tree styles share the list row and drop the indent classes', async () => {
  const { readFileSync } = await import('node:fs')
  const css = readFileSync(new URL('../panel/style/files.css', import.meta.url), 'utf8'), view = readFileSync(new URL('../panel/view/files.ts', import.meta.url), 'utf8')
  assert.doesNotMatch(css, /m-files-depth/); assert.doesNotMatch(view, /m-files-depth/)
  assert.match(css, /var\(--moai-row-h\)/); assert.match(view, /twigParts/); assert.match(view, /TWIG_TEXT/)
})

// The files tab's diff in two steps (moai-dy7a.lb0): heads from the change list it holds, a directory row heading the
// changed files under it, each patch when its file opens.
function diffPage(changes, { hold = () => false } = {}) {
  const calls = [], held = []
  const p = page(async ({ path, query }) => {
    calls.push([path, query])
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('x')], truncated: false } }
    if (path === '/api/changes') return { kind: 'passed', output: { files: changes(query.root) } }
    if (path === '/api/diff') {
      const answer = { kind: 'passed', output: { files: [{ ...change(query.path, 'modified', 1, 0), hunks: [{ old_start: 0, old_len: 0, new_start: 1, new_len: 1, section: '', lines: [{ kind: 'add', old_no: null, new_no: 1, text: `${query.root}:${query.path}`, spans: null }] }] }], truncated: false, highlight: 'full' } }
      if (hold(query)) return new Promise(resolve => held.push(() => resolve(answer)))
      return answer
    }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  }, roots2)
  Object.defineProperty(p.root, 'clientWidth', { configurable: true, value: 1000 })
  return { p, calls, held, diffs: () => calls.filter(([path]) => path === '/api/diff').map(([, q]) => q), pane: () => p.root.querySelector('.m-diff-body').textContent }
}
const key = (p, k) => p.view.handleKey(new p.dom.window.KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }))
test('a directory row heads the changed files under it, from the list it holds, and asks only the open one', async () => {
  const { p, calls, diffs, pane } = diffPage(() => [change('src/a.rs', 'modified', 3, 1), change('src/deep/b.rs', 'added', 4, 0), change('top.md', 'modified', 1, 1)])
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(10)
  assert.equal(rowsOf(p)[0].querySelector('.m-files-label').textContent, 'src')
  assert.deepEqual([...p.root.querySelectorAll('.m-diff summary .m-diff-path')].map(n => n.textContent), ['src/a.rs', 'src/deep/b.rs'])
  assert.doesNotMatch(pane(), new RegExp(COPY.en.diff.empty))
  await wait(150)
  assert.equal(calls.filter(([path]) => path === '/api/changes').length, 1, 'the heads came from the list it held')
  assert.deepEqual(diffs(), [{ root: '.', path: 'src/a.rs' }])
  assert.match(pane(), /\.:src\/a\.rs/)
  p.view.destroy(); p.dom.window.close()
})
test('a file the checkout did not change says no changes; a held j asks no patch on the way', async () => {
  const { p, diffs, pane } = diffPage(() => [change('src/a.rs', 'modified', 1, 0), change('src/b.rs', 'modified', 1, 0), change('src/c.rs', 'modified', 1, 0), change('src/d.rs', 'modified', 1, 0)])
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(10); key(p, 'j'); key(p, 'j'); key(p, 'j'); key(p, 'j'); await wait(150)
  assert.deepEqual(diffs().map(q => q.path), ['src/d.rs'])
  assert.match(pane(), /src\/d\.rs/)
  p.view.command('changed'); await wait(150)
  assert.equal(rowsOf(p)[0].querySelector('.m-files-label').textContent, 'x')
  assert.equal(p.view.filesState().diff, false); p.view.command('diff'); await wait(150)
  assert.match(pane(), new RegExp(COPY.en.diff.empty)); assert.deepEqual(diffs().map(q => q.path), ['src/d.rs'])
  p.view.destroy(); p.dom.window.close()
})
test('a patch of the checkout left behind never lands on the one chosen', async () => {
  const { p, held, pane } = diffPage(root => [change('a.rs', 'modified', 1, 0)], { hold: query => query.root === '.' })
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(150)
  assert.equal(held.length, 1)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(30)
  p.view.command('changed'); await wait(150)
  held[0](); await wait(10)
  assert.match(pane(), /side:a\.rs/); assert.doesNotMatch(pane(), /\.:a\.rs/)
  p.view.destroy(); p.dom.window.close()
})
test('a file past 5,000 lines in a checkout offers the fetch alone: no terminal line, no request until asked', async () => {
  const { p, diffs } = diffPage(() => [change('huge.rs', 'added', 9000, 0)])
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); await wait(150)
  assert.deepEqual(diffs(), []); assert.match(p.root.querySelector('.m-diff-large').textContent, /9,000/)
  assert.equal(p.root.querySelector('.m-diff-large code'), null)
  p.root.querySelector('.m-diff-large .m-diff-retry').click(); await wait(150)
  assert.deepEqual(diffs().map(q => q.path), ['huge.rs'])
  p.view.destroy(); p.dom.window.close()
})

// The letters are the server's (moai-dy7a.eol): the files tab reads the change list whenever it stands.
const { changeFingerprint, changeSetOf, changesOf } = await panelModule('data/files.ts')
const letterOf = (p, name) => rowsOf(p).find(r => nameOf(r).startsWith(name))?.querySelector('.m-files-c-mark')
const mk = (path, mark, status = 'modified', old = null) => ({ path, old_path: old, status, mark, adds: 1, dels: 0, binary: false })
test('the whole tree takes the list\'s letters, a closed folder the map\'s, and shows nothing for what the server did not mark', async () => {
  const listing = [entry('src', 'dir'), entry('a.rs'), entry('b.rs'), entry('c.rs'), entry('same.rs'), entry('gone.rs'), entry('odd.rs'), entry('nomark.rs')]
  const files = [mk('src/x.rs', 'M'), mk('a.rs', 'M'), mk('b.rs', 'A', 'untracked'), mk('c.rs', 'R', 'renamed', 'old.rs'), mk('gone.rs', 'D', 'deleted'), mk('odd.rs', '<b>X</b>'), { ...mk('nomark.rs', undefined), mark: undefined }]
  const p = changedPage(files, listing, { dirs: { src: 'A', gone: 'D', '/abs': 'M', '../up': 'M', 'a\\b': 'M', odd: 'Z' } }); p.view.render(p.state, null); await wait(30)
  assert.deepEqual(rowsOf(p).map(r => r.querySelector('.m-files-c-mark').textContent), ['A', 'M', 'A', 'R', '', '', '', ''], 'src closed: the map; a deleted file is no row and gives no letter')
  assert.ok(letterOf(p, 'src').classList.contains('m-mark-A')); assert.ok(letterOf(p, 'b.rs').classList.contains('m-mark-A')); assert.ok(letterOf(p, 'c.rs').classList.contains('m-mark-R'))
  assert.equal(letterOf(p, 'gone.rs').textContent, ''); assert.equal(letterOf(p, 'gone.rs').className, 'm-files-c-mark')
  assert.equal(letterOf(p, 'odd.rs').className, 'm-files-c-mark'); assert.equal(p.root.querySelector('.m-files-tree b'), null)
  assert.equal(letterOf(p, 'nomark.rs').textContent, '', 'a letter the page has not been given is never made up from the status word')
  assert.ok(rowsOf(p).every(r => r.children[0].classList.contains('m-files-c-mark')), 'every row keeps its letter cell, so a letter coming late moves no name')
  p.view.destroy(); p.dom.window.close()
})
test('the list is read when the tab opens whatever is on, and its letters arrive without a rebuild of the rows', async () => {
  let release, reads = 0
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a.rs'), entry('b.rs')], stamp: 's', truncated: false } }
    if (path === '/api/changes') { reads++; await new Promise(resolve => { release = resolve }); return { kind: 'passed', output: { files: [mk('a.rs', 'M')] } } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(30)
  assert.equal(rowsOf(p).length, 2, 'the tree never waits for the list'); assert.equal(reads, 1)
  const watch = p.view.fileWatch(); assert.equal(watch.changeKey, 'root:.'); assert.equal(watch.changeFingerprint, '', 'not read yet: the first answer must read as a change')
  release(); await wait(30)
  assert.equal(letterOf(p, 'a.rs').textContent, 'M'); assert.equal(reads, 1)
  assert.equal(p.view.fileWatch().changeFingerprint, changeFingerprint(changeSetOf({ files: [mk('a.rs', 'M')] })))
  p.view.destroy(); p.dom.window.close()
})
test('a refresh redraws on a letter that moved alone and a checkout switch drops the old letters', async () => {
  const roots = [{ key: '.', branch: 'main', main: true, current: true, issue: null }, { key: 'side', branch: 'side', main: false, current: false, issue: null }]
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('src', 'dir'), entry('a.rs')], stamp: 's', truncated: false } }
    if (path === '/api/changes') return { kind: 'passed', output: query.root === '.' ? { files: [mk('src/x.rs', 'M')], dirs: { src: 'M' } } : { files: [], dirs: {} } }
    return { kind: 'passed', output: {} }
  }, roots)
  p.view.render(p.state, null); await wait(30)
  assert.equal(letterOf(p, 'src').textContent, 'M')
  const watch = p.view.fileWatch(), before = rowsOf(p)[0]
  await p.view.refreshFileChanges(watch, changeFingerprint(changeSetOf({ files: [mk('src/x.rs', 'M')], dirs: { src: 'A' } })))
  assert.equal(letterOf(p, 'src').textContent, 'A'); assert.notEqual(rowsOf(p)[0], before)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(30)
  assert.equal(letterOf(p, 'src').textContent, '', 'the other checkout does not inherit the first one\'s letters')
  await p.view.refreshFileChanges({ ...watch, key: 'files:.' }, changeFingerprint(changeSetOf({ files: [mk('a.rs', 'M')] })))
  assert.equal(letterOf(p, 'a.rs').textContent, '', 'a late answer for the old checkout is dropped')
  p.view.destroy(); p.dom.window.close()
})
test('a checkout git cannot read stands without letters and says so once; a capped list says it on the whole tree too', async () => {
  const p = changedPage([], [entry('a.rs')], { vcs_error: { kind: 'unavailable', said: 'version control unavailable' } }); p.view.render(p.state, null); await wait(30)
  assert.equal(rowsOf(p).length, 1); assert.equal(letterOf(p, 'a.rs').textContent, ''); assert.match(p.root.textContent, /version control unavailable/)
  p.view.destroy(); p.dom.window.close()
  const q = changedPage([mk('a.rs', 'M')], [entry('a.rs')], { total: 7000, truncated: true }); q.view.render(q.state, null); await wait(30)
  assert.match(q.root.textContent, /7000 · Truncated/); assert.equal(letterOf(q, 'a.rs').textContent, 'M')
  q.view.destroy(); q.dom.window.close()
})
test('the change answer keeps only the four letters and the paths a row could be', () => {
  const set = changeSetOf({ files: [mk('a', 'M'), mk('b', 'Q'), { path: '/abs', status: 'x', adds: 0, dels: 0 }], dirs: { d: 'D', 'd/e': 'q', '..': 'M', 'x//y': 'M', '': 'M' } })
  assert.deepEqual(set.files.map(f => [f.path, f.mark]), [['a', 'M'], ['b', '']]); assert.deepEqual(set.dirs, [['d', 'D']])
  assert.equal(changesOf({ files: [] }).length, 0)
  const same = { files: [mk('a', 'M')] }
  assert.notEqual(changeFingerprint(changeSetOf({ ...same, dirs: { d: 'M' } })), changeFingerprint(changeSetOf({ ...same, dirs: { d: 'A' } })), 'the map alone moves the fingerprint')
  assert.equal(changeFingerprint(changeSetOf({ ...same, dirs: { b: 'M', a: 'A' } })), changeFingerprint(changeSetOf({ ...same, dirs: { a: 'A', b: 'M' } })), 'key order does not')
})
const { createFilesRefresher } = await import('../panel/data/files-refresh.ts')
test('the refresher keeps the letters fresh with the tab open and nothing else on: same answer rebuilds nothing, a folder\'s letter alone rebuilds, a hidden tab asks nothing', async () => {
  let now = 0, shown = true, notify = () => {}, asked = 0, answer = { files: [mk('src/x.rs', 'M')], dirs: { src: 'M' } }
  const time = { now: () => now, setTimeout: () => 0, clearTimeout() {} }
  const visible = { shown: () => shown, watch(fn) { notify = fn; return () => {} } }
  const q = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('src', 'dir'), entry('a.rs')], stamp: 's', truncated: false } }
    if (path === '/api/changes') return { kind: 'passed', output: answer }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  q.view.render(q.state, null); await wait(30)
  assert.equal(letterOf(q, 'src').textContent, 'M')
  const refresher = createFilesRefresher({ clock: time, visibility: visible, snapshot: () => q.view.fileWatch(),
    async stamps() { return { dirs: { '.': 's' }, file: null } },
    async changes() { asked++; return changeFingerprint(changeSetOf(answer)) },
    changedStamps: (w, d, f, a) => q.view.refreshFileStamps(w, d, f, a), changedChanges: (w, fp) => q.view.refreshFileChanges(w, fp) })
  const before = rowsOf(q)
  now = 5000; await refresher.check(); assert.equal(asked, 1); assert.ok(rowsOf(q).every((r, i) => r === before[i]), 'the same answer rebuilds no row')
  answer = { files: [mk('src/x.rs', 'M')], dirs: { src: 'A' } }
  now = 10000; await refresher.check(); assert.equal(asked, 2); assert.equal(letterOf(q, 'src').textContent, 'A', 'a letter that moved alone reads as a change')
  shown = false; now = 20000; await refresher.check(); assert.equal(asked, 2, 'a hidden tab asks nothing')
  refresher.stop(); q.view.destroy(); q.dom.window.close()
})

// Epic-end review fixes (moai-dy7a).
const changeRow = (path, mark = 'M') => ({ path, old_path: null, status: 'modified', mark, adds: 1, dels: 0, binary: false })
test('a search begun by clicking the box gives back the folds of before it when cleared', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: query.dir === '.' ? [entry('dir', 'dir'), entry('other')] : [entry('inside')] } }
    if (path === '/api/find') return { kind: 'passed', output: { matches: [{ path: 'other', start: 0, end: 5 }], total: 1, truncated: false, walk_truncated: false, ignored_unavailable: false } }
    if (path === '/api/changes') return { kind: 'passed', output: { scope: 'whole', files: [], total: 0, truncated: false, dirs: {} } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(10)
  p.root.querySelector('[role=treeitem]').click(); await wait(20)
  assert.match(p.root.querySelector('.m-files-tree').textContent, /inside/)
  const input = p.root.querySelector('input.m-files-search'); input.focus()
  input.value = 'other'; input.dispatchEvent(new p.dom.window.Event('input', { bubbles: true })); await wait(250)
  input.value = ''; input.dispatchEvent(new p.dom.window.Event('input', { bubbles: true }))
  assert.match(p.root.querySelector('.m-files-tree').textContent, /inside/, 'the folder opened before the search is still open')
  p.view.destroy(); p.dom.window.close()
})
test('Retry after the change list said why it failed reads the list again', async () => {
  let reads = 0
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a.rs')] } }
    if (path === '/api/changes') { reads++; return { kind: 'passed', output: reads === 1 ? { scope: 'whole', files: [], total: 0, truncated: false, vcs_error: { kind: 'failed', said: 'git broke' } } : { scope: 'whole', files: [changeRow('a.rs')], total: 1, truncated: false, dirs: {} } } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(20)
  p.view.command('diff'); await wait(20)
  const body = () => p.root.querySelector('.m-diff-body').textContent
  assert.match(body(), /git broke/)
  p.root.querySelector('.m-diff-failed .m-diff-retry').click(); await wait(20)
  assert.equal(reads, 2); assert.doesNotMatch(body(), /git broke/); assert.match(body(), /a\.rs/)
  p.view.destroy(); p.dom.window.close()
})
test('changed-only pressed twice before the list came enters once, and leaving gives back the whole tree', async () => {
  const p = page(async ({ path, query }) => {
    if (path === '/api/files') return { kind: 'passed', output: { entries: [entry('a.rs'), entry('plain.txt')] } }
    if (path === '/api/changes') return { kind: 'passed', output: { scope: 'whole', files: [changeRow('a.rs')], total: 1, truncated: false, dirs: {} } }
    return { kind: 'passed', output: { path: query.path, text: '', binary: false, truncated: false, spans: null } }
  })
  p.view.render(p.state, null); await wait(20)
  p.view.command('changed'); p.view.command('changed'); await wait(20)
  assert.doesNotMatch(p.root.querySelector('.m-files-tree').textContent, /plain\.txt/)
  p.view.command('changed'); await wait(20)
  assert.match(p.root.querySelector('.m-files-tree').textContent, /plain\.txt/)
  p.view.destroy(); p.dom.window.close()
})
