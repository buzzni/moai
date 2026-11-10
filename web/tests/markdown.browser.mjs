import assert from 'node:assert/strict'
import { test, after } from 'node:test'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { chromium, firefox } from 'playwright'
import { vendorGlobals } from '../vendor.mjs'

const source = `
import { renderMarkdown, snippetRefs } from './panel/view/markdown.ts';
import { hydrateDiagrams } from './panel/view/diagram.ts';
import { createClient, withProject } from './panel/data/client.ts';
import { loadSnippets, snippetKey, PRIORITY } from './panel/data/snippet.ts';
(async () => {
const fence = String.fromCharCode(96).repeat(3);
const body = '# Math $x^2$\\n\\nNote[^1].\\n\\n[^1]: footnote\\n\\n<<< ../../src/a.rs#part\\n\\n<<< denied.key\\n\\n' + fence + 'mermaid\\ngraph TD; A-->B\\n' + fence + '\\n\\n' + fence + 'mermaid\\nsequenceDiagram\\nAlice->>Bob: Hello\\n' + fence + '\\n\\n' + fence + 'mermaid\\nthis is invalid\\n' + fence;
const client = createClient(withProject(fetch, '/project'));
const refs = snippetRefs(body, { page: '/docs/guide/a.md' });
const snippets = await loadSnippets(client, refs, { page: 'docs/guide/a.md', priority: PRIORITY.wiki });
const render = () => { const r = renderMarkdown(document, body, {page:'/docs/guide/a.md', snippets: ref => snippets.get(snippetKey(ref, 'docs/guide/a.md'))}); const host = document.querySelector('#host'); host.replaceChildren(r.root); return hydrateDiagrams(r.root, document, {alt:'diagram',error:'diagram failed'}); };
await render(); window.redraw = render; window.ready = true;
})().catch(error => { window.failure = String(error); });
`
const bundled = await build({ stdin: { contents: source, resolveDir: fileURLToPath(new URL('..', import.meta.url)), loader: 'ts' }, bundle: true, write: false, format: 'iife', target:'es2022', plugins:[vendorGlobals] })
const vendor = await readFile(new URL('../../src/web/dist/vendor.js', import.meta.url))
const css = await readFile(new URL('../panel/style/markdown.css', import.meta.url))
const seen = []
const csp = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
const server = createServer((req, res) => {
  res.setHeader('Content-Security-Policy', csp)
  if (req.url.startsWith('/api/snippet')) {
    const url = new URL(req.url, 'http://localhost'); seen.push(Object.fromEntries(url.searchParams))
    res.setHeader('Content-Type', 'application/json')
    if (url.searchParams.get('path') === 'denied.key') { res.statusCode = 403; res.end(JSON.stringify({error:'refused',code:'refused'})) }
    else res.end(JSON.stringify({exit:0,out:{path:'src/a.rs',text:'safe <code>'}}))
  } else if (req.url === '/vendor.js') { res.setHeader('Content-Type','text/javascript'); res.end(vendor) }
  else if (req.url === '/app.js') { res.setHeader('Content-Type','text/javascript'); res.end(bundled.outputFiles[0].text) }
  else if (req.url === '/app.css') { res.setHeader('Content-Type','text/css'); res.end(css) }
  else { res.setHeader('Content-Type','text/html'); res.end('<!doctype html><link rel="stylesheet" href="/app.css"><div id="host"></div><script src="/vendor.js" defer></script><script src="/app.js" defer></script>') }
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
after(() => server.close())
for (const engine of [chromium, firefox]) test(`${engine.name()} renders diagrams and snippets under the server CSP`, async t => {
  const browser = await engine.launch()
  t.after(() => browser.close())
  const page = await browser.newPage()
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  page.on('console', m => { if (m.type() === 'error' && !/content security policy|content-security-policy|inline style|server responded with a status of 403/i.test(m.text())) errors.push(m.text()) })
  await page.goto(`http://127.0.0.1:${server.address().port}`)
  await page.waitForFunction(() => window.ready || window.failure, null, { timeout: 30000 })
  assert.equal(await page.evaluate(() => window.failure), undefined)
  assert.deepEqual(errors, [])
  assert.equal(await page.locator('img.m-md-diagram').count(), 2)
  for (const img of await page.locator('img.m-md-diagram').all()) assert.ok(await img.evaluate(e => { const svg = atob(e.src.split(',')[1]); const viewBox = /viewBox=[\"']\s*[-\d.]+[ ,]+[-\d.]+[ ,]+([\d.]+)/i.exec(svg); return e.naturalWidth > 0 && Number(e.getAttribute('width')) === Number(viewBox?.[1]); }))
  assert.ok(await page.locator('math').count())
  assert.equal(await page.locator('.m-md-diagram-error').count(), 1)
  assert.equal(await page.locator('.m-md-mermaid pre:not([hidden])').count(), 1)
  assert.match(await page.locator('#host').innerText(), /safe <code>/)
  assert.match(await page.locator('#host').innerText(), /refused/)
  assert.equal(await page.locator('#host [style]').count(), 0)
  assert.ok(seen.some(q => q.path === '../../src/a.rs' && q.region === 'part' && q.page === 'docs/guide/a.md' && q.project === '/project'))
})
