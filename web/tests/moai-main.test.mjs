// Boot the actual page entry point: tab transitions must request chart data, including during a tree-only read.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { JSDOM } from 'jsdom'
import { i18nBundles } from '../i18n-bundles.mjs'

const bundled = await build({
  entryPoints: [fileURLToPath(new URL('../panel/main.ts', import.meta.url))],
  bundle: true, format: 'iife', platform: 'browser', write: false, plugins: [i18nBundles],
})
const code = bundled.outputFiles[0].text
const boot = {
  lang: 'en', version: '0.30.0', project: 'test', root: '/scratch', statuses: ['todo', 'done'], naming: 'full',
  glyphs: { todo: '·', done: '✓' }, unknown_glyph: '○', timezone: 'UTC',
  columns: { backlog: { name: 'backlog', glyph: '◇' }, shelved: { name: 'shelved', glyph: '‖' } },
  filter_keys: ['status'], sort_fields: [{ name: 'priority', dir: 'asc' }],
}
const row = { id: 'test-1234', title: 'Planned row', kind: 'issue', status: 'todo', priority: 2, starts_on: '2026-10-26', due_on: '2026-10-28' }
const gantt = { id: row.id, kind: row.kind, planned: { start: row.starts_on, end: row.due_on, from: 'own' }, actual: { kind: 'not_started' }, progress: { kind: 'column', name: 'todo' }, overdue: null, deferred: false, blocked_by: [] }
// Up to ~10 s: the whole page boots in jsdom, and a machine running several sessions at once is slow to.
const until = async (check) => {
  for (let n = 0; n < 2000; n++) {
    if (check()) return
    await new Promise((resolve) => setTimeout(resolve, 5))
  }
  assert.fail('the page did not reach the expected state')
}

async function page(t, { holdFirst = false, initialTab, chartFailures = 0 } = {}) {
  const dom = new JSDOM('<!doctype html><html><body><div id="moai-app"></div></body></html>', {
    url: 'http://localhost:6624/', runScripts: 'outside-only', pretendToBeVisual: true,
  })
  t.after(() => dom.window.close())
  if (initialTab) dom.window.localStorage.setItem('moai.web', JSON.stringify({ tab: initialTab }))
  const trees = []
  const requests = []
  let release
  dom.window.fetch = async (path) => {
    const url = new URL(path, dom.window.location.href)
    requests.push(url.pathname)
    let body
    if (url.pathname === '/api/boot') body = boot
    else if (url.pathname === '/api/stamp') body = { stamp: 'unchanged' }
    else {
      let out
      if (url.pathname === '/api/tree') {
        trees.push(url)
        if (holdFirst && trees.length === 1) await new Promise((resolve) => { release = resolve })
        if (url.searchParams.has('gantt') && chartFailures > 0) { chartFailures--; out = null }
        else out = [{ ...row, home: [], ...(url.searchParams.has('gantt') ? { gantt } : {}) }]
      } else if (url.pathname === '/api/rows') out = [row]
      else if (url.pathname === '/api/status') out = { issues: {}, milestones: [], epics: [] }
      else out = []
      body = { exit: 0, out }
    }
    return { status: 200, text: async () => JSON.stringify(body) }
  }
  dom.window.eval(code)
  await until(() => trees.length > 0)
  const tab = (id) => dom.window.document.querySelector(`[role=tab][data-tab="${id}"]`)
  const bars = () => dom.window.document.querySelectorAll('.g-lanes .b-plan').length
  return { dom, trees, requests, tab, bars, release: () => release?.() }
}

test('opening Gantt fetches values once and subsequent visits reuse them', async (t) => {
  const p = await page(t)
  await until(() => p.dom.window.document.querySelector('.moai-line[data-key="test-1234"]'))
  assert.equal(p.trees[0].searchParams.has('gantt'), false)
  assert.equal(p.trees[0].searchParams.has('tz'), false)
  const rowReads = p.requests.filter((path) => path === '/api/rows').length
  const statusReads = p.requests.filter((path) => path === '/api/status').length
  p.tab('gantt').click()
  await until(() => p.bars() === 1)
  assert.equal(p.requests.filter((path) => path === '/api/rows').length, rowReads)
  assert.equal(p.requests.filter((path) => path === '/api/status').length, statusReads)
  assert.equal(p.trees.length, 2)
  assert.equal(p.trees[1].searchParams.get('gantt'), '1')
  assert.equal(p.trees[1].searchParams.get('tz'), 'UTC')
  p.tab('list').click()
  p.tab('gantt').click()
  await new Promise((resolve) => setTimeout(resolve, 20))
  assert.equal(p.trees.length, 2)
})

test('opening Gantt during the first tree-only read schedules a chart read after it', async (t) => {
  const p = await page(t, { holdFirst: true })
  p.tab('gantt').click()
  p.release()
  await until(() => p.bars() === 1)
  assert.deepEqual(p.trees.map((url) => url.searchParams.has('gantt')), [false, true])
})

test('a failed chart-only read retries while the tracker stamp stays unchanged', async (t) => {
  const p = await page(t, { chartFailures: 1 })
  await until(() => p.dom.window.document.querySelector('.moai-line[data-key="test-1234"]'))
  p.tab('gantt').click()
  await until(() => p.bars() === 1)
  assert.deepEqual(p.trees.map((url) => url.searchParams.has('gantt')), [false, true, true])
})

test('a page restored on Gantt asks for values in its first read', async (t) => {
  const p = await page(t, { initialTab: 'gantt' })
  await until(() => p.bars() === 1)
  assert.equal(p.trees[0].searchParams.get('gantt'), '1')
  assert.equal(p.trees.length, 1)
})
