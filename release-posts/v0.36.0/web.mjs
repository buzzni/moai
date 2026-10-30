// The v0.36.0 moai web board in both themes. Based on v0.34.0/web.mjs.
// Playwright is supplied through NODE_PATH, as in scripts/site-demo/record-web.mjs.
import { execFileSync, spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [repo, outDir] = process.argv.slice(2).map((p) => path.resolve(p))
if (!repo || !outDir) throw new Error('usage: web.mjs <seed repo> <out dir>')
mkdirSync(outDir, { recursive: true })
const moai = path.resolve(process.env.MOAI)
const env = { ...process.env, TZ: 'UTC', MOAI_NO_UPDATE_CHECK: '1', MOAI_LANG: 'en', MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml') }
for (const k of ['MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[k]
execFileSync(moai, ['read', '--all', '--user', 'Mira Chen (mira@example.com)'], { cwd: repo, env, stdio: 'ignore' })
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const SIZE = { width: 1600, height: 940 }
const statuses = ['in_progress', 'review', 'done']
const browser = await chromium.launch()
try {
  for (const [scheme, file] of [['light', '01-web-board.png'], ['dark', '02-web-board-dark.png']]) {
    const ctx = await browser.newContext({ viewport: SIZE, colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
    try {
      const page = await ctx.newPage()
      await page.goto(url)
      await page.waitForSelector('[role="row"]')
      for (const key of ['Space', 'g', 'b', 'Space', 'v', 'd', 'Escape']) {
        await page.keyboard.press(key)
        await page.waitForTimeout(180)
      }
      await page.waitForSelector('.m-board .m-card')
      await page.waitForTimeout(800)
      const lane = page.locator('.m-board').filter({ has: page.locator('[data-status="in_progress"] .m-card') })
        .filter({ has: page.locator('[data-status="review"] .m-card') })
        .filter({ has: page.locator('[data-status="done"] .m-card') }).first()
      if (await lane.count() !== 1) throw new Error('demo board has no lane with all three status cards')
      await lane.scrollIntoViewIfNeeded()
      await page.mouse.move(SIZE.width - 2, SIZE.height - 2)
      await page.emulateMedia({ reducedMotion: 'reduce' })
      await page.waitForTimeout(500)
      const bands = await Promise.all(statuses.map(async (status) => {
        const head = lane.locator(`[data-status="${status}"] .m-card .m-card-line:first-child`).first()
        const box = await head.boundingBox()
        if (!box || box.x < 0 || box.y < 0 || box.x + box.width > SIZE.width || box.y + box.height > SIZE.height) {
          throw new Error(`${status} band is outside the screenshot`)
        }
        return box
      }))
      await page.evaluate((boxes) => {
        for (const box of boxes) {
          const mark = document.createElement('div')
          Object.assign(mark.style, {
            position: 'fixed', left: `${box.x - 4}px`, top: `${box.y - 3}px`,
            width: `${box.width + 8}px`, height: `${box.height + 6}px`,
            border: '3px solid #f08a4b', borderRadius: '6px',
            boxShadow: '0 0 0 3px rgba(240,138,75,.25)',
            pointerEvents: 'none', zIndex: 2147483646,
          })
          document.body.appendChild(mark)
        }
      }, bands)
      await page.screenshot({ path: path.join(outDir, file) })
    } finally {
      await ctx.close()
    }
  }
} finally {
  await browser.close()
  server.kill()
}
