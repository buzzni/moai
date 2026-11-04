// Capture the web list (light) and kanban (dark) with every group folded, so the hatch fills the space below.
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
async function foldAll(page, toggles) {
  for (let n = 0; n < 40; n++) {
    const open = page.locator(toggles).first()
    if (!(await open.count())) return
    await open.click()
    await pause(250)
  }
}
async function shoot(scheme, tab, name) {
  const context = await browser.newContext({ viewport: { width: 1400, height: 820 },
    colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
  const page = await context.newPage()
  try {
    await page.goto(url)
    await page.waitForSelector('[role="row"]')
    await page.emulateMedia({ reducedMotion: 'reduce' })
    if (tab === 'board') {
      await page.click('#m-tab-board')
      await page.waitForSelector('.m-swim-head')
      await foldAll(page, '[data-pane="board"] .m-swim-head .moai-toggle[aria-expanded="true"]')
    } else {
      await foldAll(page, '[data-pane="list"] [role="row"] .moai-toggle[aria-expanded="true"]')
    }
    await pause(800)
    await page.waitForSelector(`[data-pane="${tab}"] .m-fill:not([hidden])`)
    await page.mouse.move(1398, 818)
    await page.screenshot({ path: path.join(outDir, `${name}.png`) })
  } finally { await context.close() }
}
try {
  await shoot('light', 'list', '01-web-list-hatch-light')
  await shoot('dark', 'board', '02-web-board-hatch-dark')
} finally {
  await browser.close()
  server.kill()
}
