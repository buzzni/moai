// The v0.38.0 board, list and wide Gantt in a fictional Lantern project.
// Playwright is supplied through NODE_PATH by make.sh.
import { execFileSync, spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [repo, outDir] = process.argv.slice(2).map((p) => path.resolve(p))
if (!repo || !outDir) throw new Error('usage: web.mjs <seed repo> <out dir>')
mkdirSync(outDir, { recursive: true })
const ids = Object.fromEntries(readFileSync(path.join(repo, '.demo-ids'), 'utf8').trim().split('\n').map((line) => line.split('=')))
const moai = path.resolve(process.env.MOAI)
const env = { ...process.env, TZ: 'UTC', MOAI_NO_UPDATE_CHECK: '1', MOAI_LANG: 'en', MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml') }
for (const k of ['MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[k]
const issues = JSON.parse(execFileSync(moai, ['-C', repo, 'show', '-e', ids.sync, '--type', 'issue', '--json'], { env }))
const todo = issues.find((row) => row.status === 'todo')
if (!todo) throw new Error('demo needs a todo issue for the blue column')
execFileSync(moai, ['read', '--all', '--user', 'Mira Chen (mira@example.com)'], { cwd: repo, env, stdio: 'ignore' })
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const browser = await chromium.launch()
try {
  for (const scheme of ['light', 'dark']) {
    const ctx = await browser.newContext({ viewport: { width: 1700, height: 980 }, colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
    try {
      const page = await ctx.newPage()
      await page.goto(url)
      await page.waitForSelector('[role="row"]')
      await page.emulateMedia({ reducedMotion: 'reduce' })
      await page.waitForFunction((id) => document.querySelector(`[role="row"][data-id="${id}"]`)?.getAttribute('aria-expanded') === 'true', ids.m14)

      // One milestone has work and starts open; the completed one starts folded.
      const running = page.locator(`[role="row"][data-id="${ids.m14}"]`)
      const closed = page.locator(`[role="row"][data-id="${ids.m15}"]`)
      const runningOpen = await running.getAttribute('aria-expanded')
      const closedOpen = await closed.getAttribute('aria-expanded')
      if (runningOpen !== 'true' || closedOpen !== 'false') {
        throw new Error(`list milestone fold defaults changed: running=${runningOpen}, closed=${closedOpen}`)
      }
      const todoRow = page.locator(`[role="row"][data-id="${todo.id}"]`)
      const sync = page.locator(`[role="row"][data-id="${ids.sync}"]`)
      if (await sync.getAttribute('aria-expanded') === 'false') await sync.locator('.moai-toggle').click()
      await todoRow.waitFor()
      await todoRow.click()
      await page.mouse.move(1698, 978)
      await page.screenshot({ path: path.join(outDir, `0${scheme === 'light' ? 6 : 7}-web-list-${scheme}.png`) })

      await page.locator('.moai-detail-toggle').click()
      await page.locator('#m-tab-board').click()
      await page.waitForSelector('.m-swim-head')
      const activeLane = page.locator('.m-swim-head').filter({ hasText: 'v1.4 — Sync' })
      const closedLane = page.locator('.m-swim-head').filter({ hasText: 'v1.5 — Sharing' })
      if (await activeLane.locator('.moai-toggle').getAttribute('aria-expanded') !== 'true' ||
          await closedLane.locator('.moai-toggle').getAttribute('aria-expanded') !== 'false') {
        throw new Error('board milestone fold defaults changed')
      }
      if (await page.locator(`.m-card[data-id="${ids.sync}"]`).count()) throw new Error('board still draws an epic card')
      await page.locator(`.m-card[data-id="${todo.id}"]`).click()
      await page.mouse.move(1698, 978)
      await page.screenshot({ path: path.join(outDir, `0${scheme === 'light' ? 4 : 5}-web-board-${scheme}.png`) })
    } finally {
      await ctx.close()
    }
  }

  const ctx = await browser.newContext({ viewport: { width: 1920, height: 980 }, colorScheme: 'light', deviceScaleFactor: 2, timezoneId: 'UTC' })
  try {
    const page = await ctx.newPage()
    await page.goto(url)
    await page.waitForSelector('[role="row"]')
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.waitForFunction((id) => document.querySelector(`[role="row"][data-id="${id}"]`)?.getAttribute('aria-expanded') === 'true', ids.m14)
    await page.locator('.moai-detail-toggle').click()
    await page.locator('#m-tab-gantt').click()
    await page.waitForSelector('.g-right .hd2')
    await page.locator('.g-toolbar .m-btn-sm').last().click() // fit the planned range
    await page.waitForTimeout(400)
    const axis = await page.locator('.g-right .hd2').boundingBox()
    const dates = await page.locator('.g-right').boundingBox()
    await page.mouse.move(1918, 978)
    await page.screenshot({ path: path.join(outDir, '08-web-gantt-wide.png') })
    if (!axis || !dates || axis.width < dates.width - 30) {
      throw new Error(`Gantt date axis does not fill the wide pane: axis=${axis?.width}, dates=${dates?.width}`)
    }
  } finally {
    await ctx.close()
  }
} finally {
  await browser.close()
  server.kill()
}
