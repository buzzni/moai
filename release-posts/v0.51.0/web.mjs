// Capture the web sort and grouping window (light) and the kanban grouped by assignee (dark).
// usage: web.mjs <repo> <out dir> <ctl dir>   -- MOAI and Playwright (NODE_PATH) come from make.sh.
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const { chromium } = createRequire(import.meta.url)('playwright')
const [repo, outDir, ctlDir] = process.argv.slice(2, 5).map((p) => path.resolve(p))
if (!repo || !outDir || !ctlDir) throw new Error('usage: web.mjs <repo> <out dir> <ctl dir>')
mkdirSync(outDir, { recursive: true })
const moai = path.resolve(process.env.MOAI)
const env = { ...process.env, TZ: 'UTC', MOAI_NO_UPDATE_CHECK: '1', MOAI_LANG: 'en',
  MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml'), MOAI_CTL_DIR: ctlDir }
for (const key of ['MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[key]
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const browser = await chromium.launch()
async function open(scheme) {
  const context = await browser.newContext({ viewport: { width: 1400, height: 860 },
    colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
  const page = await context.newPage()
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  return { context, page }
}
async function sortWindow(page) {
  await page.keyboard.press('Space')
  await page.keyboard.press('s')
  await page.waitForSelector('.m-sort-pop .m-sort-panel')
  await pause(300)
}
// the fifth preset is people: grouped by assignee, items by priority then due
async function people(page) {
  await page.locator('.m-sort-panel section').first().locator('button').nth(4).click()
  await pause(900)
}
try {
  {
    const { context, page } = await open('light')
    await sortWindow(page)
    await people(page)
    await page.mouse.move(1398, 858)
    await page.screenshot({ path: path.join(outDir, '01-web-sort-window-light.png') })
    await context.close()
  }
  {
    const { context, page } = await open('dark')
    await sortWindow(page)
    await people(page)
    // Enter applies the preview; Esc would put the order back
    await page.keyboard.press('Enter')
    await pause(600)
    await page.click('#m-tab-board')
    await page.waitForSelector('.m-swim-head')
    await pause(900)
    await page.mouse.move(1398, 858)
    await page.screenshot({ path: path.join(outDir, '02-web-board-by-assignee-dark.png') })
    await context.close()
  }
} finally {
  await browser.close()
  server.kill()
}
