// HTTP/1 connection-budget regression, with the real event/polling client in Chromium and Firefox.
// Run: node --test tests/events.browser.mjs (from web/).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { build } from 'esbuild'
import { chromium, firefox } from 'playwright'

const rust = await readFile(new URL('../../src/web/mod.rs', import.meta.url), 'utf8')
const streamLimit = Number(/pub const STREAMS_MAX: usize = (\d+);/.exec(rust)[1])
const bundle = await build({ stdin: { contents: `
  import { openEvents } from './panel/data/events.ts';
  import { createRefresher } from './panel/data/refresh.ts';
  window.start = () => {
    window.polls = 0;
    const refresh = createRefresher({
      visibility: null,
      stamp: async () => { window.polls++; return (await (await fetch('/api/stamp')).json()).stamp },
      load: async () => {},
    });
    window.events = openEvents({url: () => '/api/events', visibility: null,
      onStamp: at => refresh.saw(at), onUp: () => refresh.pause(), onDown: () => refresh.resume()});
    window.stop = () => { window.events.stop(); refresh.stop() };
  };
`, resolveDir: new URL('..', import.meta.url).pathname, loader: 'ts' }, bundle: true, write: false, format: 'iife' })

for (const engine of [chromium, firefox]) test(`${engine.name()}: overflow tabs poll and ordinary reads keep a connection`, async t => {
  let cap = 8, active = 0, rejected = 0
  const streams = new Set()
  const server = createServer((req, res) => {
    if (req.url === '/app.js') { res.setHeader('Content-Type', 'text/javascript'); return res.end(bundle.outputFiles[0].text) }
    if (req.url === '/api/events') {
      if (active >= cap) { rejected++; res.writeHead(503); return res.end('busy') }
      active++; streams.add(res)
      res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache' })
      res.write('event: stamp\ndata: {"stamp":"same"}\n\n')
      req.on('close', () => { active--; streams.delete(res) })
      return
    }
    if (req.url === '/api/stamp') { res.setHeader('Content-Type', 'application/json'); return res.end('{"stamp":"same"}') }
    res.end('<script src="/app.js"></script>')
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => { for (const res of streams) res.end(); server.closeAllConnections(); server.close() })
  const browser = await engine.launch()
  t.after(() => browser.close())
  const url = `http://127.0.0.1:${server.address().port}`
  async function measure(limit) {
    cap = limit; rejected = 0
    const context = await browser.newContext()
    // Load documents before opening streams, so the baseline can demonstrate starvation without blocking goto.
    const pages = await Promise.all(Array.from({ length: 8 }, () => context.newPage()))
    for (const page of pages) await page.goto(url)
    for (const page of pages) await page.evaluate(() => window.start())
    await new Promise(resolve => setTimeout(resolve, 500))
    const result = await pages[0].evaluate(async () => {
      const start = performance.now()
      try {
        const answer = await fetch('/api/stamp', { signal: AbortSignal.timeout(1500) })
        return { ok: answer.ok, elapsed: Math.round(performance.now() - start) }
      } catch { return { ok: false, elapsed: Math.round(performance.now() - start) } }
    })
    const counts = { active, rejected }
    // Allow the overflow tabs' ordinary two-second polling to run too.
    if (limit === streamLimit) await new Promise(resolve => setTimeout(resolve, 2200))
    const polls = await Promise.all(pages.map(page => page.evaluate(() => window.polls)))
    for (const page of pages) await page.evaluate(() => window.stop())
    await context.close()
    while (active) await new Promise(resolve => setTimeout(resolve, 10))
    return { ...result, ...counts, polls }
  }
  const before = await measure(8)
  const after = await measure(streamLimit)
  console.log(JSON.stringify({ engine: engine.name(), tabs: 8, before, after }))
  assert.ok(after.ok, 'an ordinary read starved behind SSE streams')
  assert.equal(after.active, streamLimit)
  assert.ok(after.rejected >= 8 - streamLimit)
  assert.ok(after.polls.filter(count => count > 0).length >= 8 - streamLimit, 'overflow tabs did not poll')
})
