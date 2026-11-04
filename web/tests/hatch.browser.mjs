// Real flex and scroll layout for the list and board fill. Run: node --test tests/hatch.browser.mjs
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { chromium } from 'playwright'

const sheets = await Promise.all(['tokens', 'components', 'list', 'shell', 'gantt'].map(name => readFile(new URL(`../panel/style/${name}.css`, import.meta.url), 'utf8')))

test('fill occupies only the space after content, including at the bottom of a long scroller', async t => {
  const browser = await chromium.launch()
  t.after(() => browser.close())
  const page = await browser.newPage()
  await page.setContent(`<style>${sheets.join('\n')} [hidden]{display:none!important} .m-pane-main{height:300px;width:400px} .content{height:80px}</style>
    <div class="m-pane-main" data-pane="list"><div class="content"></div><div class="m-fill" aria-hidden="true"></div></div>
    <div class="m-pane-main" data-pane="board"><div class="content"></div><div class="m-fill" aria-hidden="true"></div></div>
    <div class="m-pane-main m-pane-gantt" data-pane="gantt"><div class="content"></div></div>`)
  for (const tab of ['list', 'board']) {
    const pane = page.locator(`[data-pane="${tab}"]`)
    const fill = pane.locator('.m-fill')
    assert.equal(await fill.evaluate(el => el.getBoundingClientRect().height), 220)
    assert.match(await fill.evaluate(el => getComputedStyle(el).backgroundImage), /repeating-linear-gradient/)
    await pane.locator('.content').evaluate(el => { el.style.height = '600px' })
    assert.equal(await fill.evaluate(el => el.getBoundingClientRect().height), 32)
    assert.equal(await pane.evaluate(el => el.scrollHeight), 632)
    await pane.evaluate(el => { el.scrollTop = el.scrollHeight })
    assert.equal(await pane.evaluate(el => el.scrollTop), 332)
  }
  assert.equal(await page.locator('[data-pane="gantt"] .m-fill').count(), 0)
})
