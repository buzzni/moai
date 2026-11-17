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
    if (path === '/api/changes') return { kind: 'passed', output: { files: [{ path: 'gone', status: 'deleted', old_path: null, adds: 0, dels: 1, binary: false }] } }
    return { kind: 'passed', output: {} }
  }, roots)
  p.view.render(p.state, null); await wait(20)
  const select = p.root.querySelector('select'); select.value = 'side'; select.dispatchEvent(new p.dom.window.Event('change')); await wait(20)
  release(); await wait(20)
  assert.match(p.root.textContent, /new/); assert.doesNotMatch(p.root.textContent, /old/)
  p.view.command('changed'); await wait(20)
  assert.match(p.root.textContent, /gone/); assert.match(p.root.textContent, / D /); assert.doesNotMatch(p.root.querySelector('.m-files-tree').textContent, /new/)
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
  p.root.querySelectorAll('[role=treeitem]')[0].click(); p.view.command('diff'); await wait(30)
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
  p.view.render(p.state, null); assert.ok(p.root.querySelector('.m-files-right > .m-files-issue')); assert.match(p.root.textContent, /collapsed/i); assert.equal(p.view.fileWatch().changeKey, undefined)
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
  assert.equal(p.root.querySelector('[aria-selected=true]').textContent.trim(), '· chosen')
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
