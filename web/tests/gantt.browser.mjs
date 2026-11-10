// Real layout regressions and measurements. Run: npm run test:gantt-browser
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { build } from 'esbuild'
import { chromium, firefox } from 'playwright'
import { i18nBundles } from '../i18n-bundles.mjs'
import { vendorGlobals } from '../vendor.mjs'

const bundle = await build({
  stdin: { contents: `
    import { mountGantt, clock } from './panel/view/gantt.ts';
    import { IssueIndex } from './panel/data/index.ts';
    import { initialState } from './panel/store.ts';
    import { copyFor } from './panel/i18n/index.ts';
    clock.now = () => Date.UTC(2026, 9, 25, 14); clock.zone = () => 'UTC';
    const rows = Array.from({length: 1500}, (_, n) => ({id: 'r' + n, kind: 'issue', status: 'todo', title: 'Row ' + n,
      gantt: {id: 'r' + n, kind: 'issue', planned: {start:'2026-10-25',end:'2026-10-28',from:'own',start_filled:false,end_filled:false},
        actual: {kind:'not_started'}, progress: {kind:'column',name:'todo'}, overdue:null,deferred:false,blocked_by:[]}}));
    const tree = rows.map(r => ({...r, home:[], path:r.id, dir:false}));
    const view = mountGantt(document.querySelector('#host'), {document, copy:copyFor('en'), language:'en', intents:{choose(){},point(){}}, layout(){}});
    view.render({...initialState(),tab:'gantt'},new IssueIndex(rows, tree));
    window.view = view; window.ready = true;
  `, resolveDir: new URL('..', import.meta.url).pathname, loader: 'ts' },
  bundle: true, write: false, format: 'iife', plugins: [i18nBundles, vendorGlobals],
})
const css = await readFile(new URL('../panel/style/gantt.css', import.meta.url), 'utf8')
const server = createServer((req, res) => {
  res.setHeader('Content-Type', req.url === '/app.js' ? 'text/javascript; charset=utf-8' : 'text/html; charset=utf-8')
  res.end(req.url === '/app.js' ? bundle.outputFiles[0].text : `<style>${css}
    body{margin:0} #host{display:flex;width:900px;height:600px} .m-gantt{width:100%;overflow:hidden}
    </style><div id="host"></div><script src="/app.js"></script>`)
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const url = `http://127.0.0.1:${server.address().port}`

for (const engine of [chromium, firefox]) {
  for (const zoom of ['day', 'month']) test(`${engine.name()} ${zoom}: widening retains the left date and avoids full draws`, async t => {
    const browser = await engine.launch()
    t.after(() => browser.close())
    const page = await browser.newPage({viewport:{width:2200,height:800}})
    page.on('pageerror', e => console.error(e.message))
    await page.goto(url)
    await page.waitForFunction(() => window.ready, null, {timeout:5000})
    await page.locator(`[data-zoom="${zoom}"]`).click()
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    const result = await page.evaluate(async () => {
      const box = document.querySelector('.g-scroll'), right = document.querySelector('.g-right'), host = document.querySelector('#host')
      const frames = [], widths = [], nodes = []
      box.scrollLeft = box.scrollWidth - box.clientWidth
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      const before = box.scrollLeft
      const svg = right.querySelector('svg.links')
      const observer = new MutationObserver(records => { for(const r of records) for(const n of r.addedNodes) if(n.matches?.('svg.links')) nodes.push(n) })
      observer.observe(right, {childList:true,subtree:true})
      for (let width=910; width<=1600; width+=10) {
        const start = performance.now()
        host.style.width = width + 'px'
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
        frames.push(performance.now()-start)
        widths.push(Number.parseFloat(right.style.width) - box.scrollLeft - (box.clientWidth - Number.parseFloat(document.querySelector('.g-inner').style.getPropertyValue('--grid-w'))))
      }
      await new Promise(resolve => setTimeout(resolve, 250))
      observer.disconnect()
      return { before, after:box.scrollLeft, fullDraws:new Set(nodes.filter(n => n !== svg)).size, svgKept:svg === right.querySelector('svg.links'),
        frames:frames.sort((a,b)=>a-b), margin:Math.min(...widths), finalMargin:Number.parseFloat(right.style.width)-box.scrollLeft-(box.clientWidth-Number.parseFloat(document.querySelector('.g-inner').style.getPropertyValue('--grid-w'))) }
    })
    const metrics = {engine:engine.name(),zoom,before:result.before,after:result.after,draws:result.fullDraws,p50:result.frames[35],p95:result.frames[66],margin:result.margin,finalMargin:result.finalMargin}
    console.log(JSON.stringify(metrics))
    if (process.env.GANTT_MEASURE_ONLY) return
    assert.ok(Math.abs(result.before-result.after)<=1, JSON.stringify(metrics))
    assert.equal(result.fullDraws,0,JSON.stringify(metrics))
    assert.ok(result.svgKept,'resize replaced the full model')
    assert.ok(result.margin>=0,'a frame left blank dates')
    assert.ok(result.finalMargin>=0 && result.finalMargin<= (zoom==='day'?80:8), 'settled axis kept resize reserve')
    // An ordinary scroll after resizing still moves; zoom in from the empty stretch returns toward the work.
    const moved = await page.evaluate(async () => {
      const box = document.querySelector('.g-scroll'); box.scrollLeft = 0
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      return box.scrollLeft
    })
    assert.equal(moved,0)
    await page.evaluate(() => window.view.command('gantt.zoomIn'))
    assert.ok(await page.locator('.g-right .hd2 .tick').count())
  })
}
test('vertical scroll only attaches entering rows and leaves overlapping nodes in place', async t => {
  const browser = await chromium.launch(); t.after(() => browser.close())
  const page = await browser.newPage(); await page.goto(url); await page.waitForFunction(() => window.ready)
  const metric = await page.evaluate(async () => {
    const box = document.querySelector('.g-scroll')
    const before = new Set(document.querySelectorAll('.g-row, .lane, svg.links, .todayline'))
    let reattached = 0, entering = 0
    const observer = new MutationObserver(records => { for(const r of records) for(const n of r.addedNodes) {
      if (before.has(n)) reattached++
      else if(n.matches?.('.g-row, .lane')) entering++
    }})
    observer.observe(document.querySelector('.g-inner'), {childList:true,subtree:true})
    box.scrollTop = 420
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
    observer.disconnect()
    return {reattached,entering, retained:[...document.querySelectorAll('.g-row, .lane')].filter(n => before.has(n)).length}
  })
  console.log(JSON.stringify({vertical:metric}))
  if (!process.env.GANTT_MEASURE_ONLY) {
    assert.ok(metric.retained > 0 && metric.entering > 0)
    assert.equal(metric.reattached,0)
  }
})
test.after(() => new Promise(resolve => server.close(resolve)))
