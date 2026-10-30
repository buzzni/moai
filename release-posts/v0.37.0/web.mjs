// The v0.37.0 web board: an expanded selected milestone beside a folded lane.
// Playwright is supplied through NODE_PATH by make.sh's caller.
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
const browser = await chromium.launch()
try {
  for (const [scheme, file] of [['light', '03-web-board-light.png'], ['dark', '04-web-board-dark.png']]) {
    const ctx = await browser.newContext({ viewport: SIZE, colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
    try {
      const page = await ctx.newPage()
      await page.goto(url)
      await page.waitForSelector('[role="row"]')
      for (const key of ['Space', 'g', 'b']) {
        await page.keyboard.press(key)
        await page.waitForTimeout(180)
      }
      await page.waitForSelector('.m-swim-head')
      const heads = page.locator('.m-swim-head')
      if (await heads.count() < 2) throw new Error('demo board needs two milestone lanes')
      const sharing = heads.filter({ hasText: 'v1.5 — Sharing' })
      const sync = heads.filter({ hasText: 'v1.4 — Sync' })
      if (await sharing.count() !== 1 || await sync.count() !== 1) throw new Error('demo milestone lanes changed')
      await sharing.locator('.moai-toggle').click()
      await sync.locator('.m-swim-title').click()
      await page.waitForTimeout(650)
      if (await sync.getAttribute('aria-selected') !== 'true') throw new Error('milestone header was not selected')
      if (await sharing.locator('.moai-toggle').getAttribute('aria-expanded') !== 'false') throw new Error('second milestone lane was not folded')
      await page.mouse.move(SIZE.width - 2, SIZE.height - 2)
      await page.emulateMedia({ reducedMotion: 'reduce' })
      await page.screenshot({ path: path.join(outDir, file) })
    } finally {
      await ctx.close()
    }
  }
} finally {
  await browser.close()
  server.kill()
}
