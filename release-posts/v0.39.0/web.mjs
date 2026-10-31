// Capture a visible web tab after real ctl requests and a pending suggestion.
// Playwright is supplied through NODE_PATH by make.sh.
import { execFileSync, spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const [repo, outDir, ctlDir] = process.argv.slice(2).map((p) => path.resolve(p))
if (!repo || !outDir || !ctlDir) throw new Error('usage: web.mjs <seed repo> <out dir> <ctl dir>')
mkdirSync(outDir, { recursive: true })
mkdirSync(ctlDir, { recursive: true, mode: 0o700 })
const moai = path.resolve(process.env.MOAI)
const ids = Object.fromEntries(readFileSync(path.join(repo, '.demo-ids'), 'utf8').trim().split('\n').map((line) => line.split('=')))
const env = { ...process.env, TZ: 'UTC', MOAI_NO_UPDATE_CHECK: '1', MOAI_LANG: 'en',
  MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml'), MOAI_CTL_DIR: ctlDir }
for (const key of ['MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[key]
const call = (...args) => execFileSync(moai, ['-C', repo, 'ctl', ...args], { env, encoding: 'utf8' })
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
async function visibleTab() {
  for (let attempt = 0; attempt < 40; attempt++) {
    const rows = JSON.parse(call('ls', '--json')).windows
    const tab = rows.find((row) => row.kind === 'web')
    if (tab) return tab.id
    await pause(250)
  }
  throw new Error('the visible web tab did not appear in moai ctl ls')
}
async function ctl(...args) {
  let lastError
  for (let attempt = 0; attempt < 15; attempt++) {
    const id = await visibleTab()
    try { return call(...args, '-t', id) } catch (error) {
      lastError = error
      if (!/tab closed|no_window/.test(String(error.stderr))) throw error
      await pause(500)
    }
  }
  throw lastError
}
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const browser = await chromium.launch()
try {
  for (const [scheme, number] of [['light', '05'], ['dark', '06']]) {
    const context = await browser.newContext({ viewport: { width: 1600, height: 900 },
      colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
    try {
      const page = await context.newPage()
      await page.goto(url)
      await page.waitForSelector('[role="row"]')
      await page.emulateMedia({ reducedMotion: 'reduce' })
      await ctl('sort', 'title:desc')
      await ctl('filter', 'status=todo')
      await ctl('theme', scheme)
      const sync = page.locator(`[role="row"][data-id="${ids.sync}"]`)
      await sync.locator('.moai-toggle').click()
      await page.getByRole('row').filter({ hasText: 'Sync attachments larger than 10 MB' }).click()
      await ctl('propose', 'Show the remaining sync work?', '--then', 'filter status=todo tag=sync')
      await page.waitForSelector('.m-proposal:not([hidden])')
      await page.mouse.move(1598, 898)
      await page.screenshot({ path: path.join(outDir, `${number}-web-ctl-${scheme}.png`) })
    } finally {
      await context.close()
    }
  }
} finally {
  await browser.close()
  server.kill()
}
