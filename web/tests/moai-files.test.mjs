import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { mountFiles } from '../panel/view/files.ts'

const wait = (ms = 0) => new Promise(resolve => setTimeout(resolve, ms))
const words = { search: 'Find', binary: 'Binary', truncated: 'Truncated', readError: 'Read error', directoryError: 'Directory error', searchError: 'Search error', matches: 'matches', ignoredUnavailable: 'No ignore rules', expansionLimit: 'Limit', unavailable: 'Unavailable' }
const entry = (name, kind = 'file') => ({ name, kind, openable: true, size: 1 })
function page(request) {
  const dom = new JSDOM('<body><div id="files"></div></body>', { pretendToBeVisual: true })
  const root = dom.window.document.querySelector('#files')
  const ctx = { document: dom.window.document, copy: { files: words }, client: { request }, intents: { dispatch() {} } }
  const view = mountFiles(root, ctx)
  return { dom, root, view, state: { filesRatio: 0.5 } }
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
