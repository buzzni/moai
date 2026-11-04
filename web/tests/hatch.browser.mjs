// Real flex and scroll layout for the list and board fill. Run: npm run test:hatch-browser
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { chromium, firefox } from 'playwright'

// In the order build.mjs joins them into app.css (tokens, components, then by name). build.mjs then closes app.css with
// `[hidden]{display:none!important}`; this test leaves it out on purpose, so the sheets' own rules (the pane's
// `:not([hidden])`, the fill setting no display) must keep `hidden` standing without that net.
const sheets = await Promise.all(['tokens', 'components', 'board', 'gantt', 'list', 'shell'].map(name => readFile(new URL(`../panel/style/${name}.css`, import.meta.url), 'utf8')))

for (const engine of [chromium, firefox]) test(`${engine.name()}: fill occupies only the space after content, including at the bottom of a long scroller`, async t => {
  const browser = await engine.launch()
  t.after(() => browser.close())
  const page = await browser.newPage()
  await page.setContent(`<style>${sheets.join('\n')} .m-pane-main{height:300px;width:400px} .content{height:80px}</style>
    <div class="m-pane-main" data-pane="list"><div class="content"></div><div class="m-fill" aria-hidden="true"></div></div>
    <div class="m-pane-main" data-pane="board"><div class="content"></div><div class="m-fill" aria-hidden="true"></div></div>
    <div class="m-pane-main" data-pane="list" data-shut hidden><div class="content"></div><div class="m-fill" aria-hidden="true"></div></div>
    <div class="m-pane-main" data-pane="board" data-empty><div class="content"></div><div class="m-fill" aria-hidden="true" hidden></div></div>`)
  for (const tab of ['list', 'board']) {
    const pane = page.locator(`[data-pane="${tab}"]:not([data-shut], [data-empty])`)
    const fill = pane.locator('.m-fill')
    assert.equal(await fill.evaluate(el => el.getBoundingClientRect().height), 220)
    assert.match(await fill.evaluate(el => getComputedStyle(el).backgroundImage), /repeating-linear-gradient/)
    await pane.locator('.content').evaluate(el => { el.style.height = '600px' })
    assert.equal(await fill.evaluate(el => el.getBoundingClientRect().height), 32)
    assert.equal(await pane.evaluate(el => el.scrollHeight), 632)
    await pane.evaluate(el => { el.scrollTop = el.scrollHeight })
    assert.equal(await pane.evaluate(el => el.scrollTop), 332)
  }
  // The pane's flex column and the fill's own rule leave `hidden` standing: a tab not shown, an empty board.
  assert.equal(await page.locator('[data-shut]').evaluate(el => getComputedStyle(el).display), 'none')
  assert.equal(await page.locator('[data-empty] .m-fill').evaluate(el => getComputedStyle(el).display), 'none')
})
