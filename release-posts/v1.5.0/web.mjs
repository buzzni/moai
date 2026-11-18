// Capture the web interface for v1.5.0: the Files tab on a Subversion working copy ("Changed files only" and its diff) and the header with the project list beside User and Version. Light theme.
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
for (const key of ['MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[key]
env.MOAI_ACTOR = 'Mira Chen (mira@example.com)'   // no git repository here: the demo's person, not the one at this keyboard
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 860 },
  colorScheme: 'light', deviceScaleFactor: 2, timezoneId: 'UTC' })
const page = await context.newPage()
const shot = async (name) => { await page.mouse.move(1438, 858); await pause(400); await page.screenshot({ path: path.join(outDir, `${name}.png`) }) }
const tab = (name) => page.getByRole('tab', { name }).or(page.getByText(name, { exact: true })).first().click()
try {
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  await page.emulateMedia({ reducedMotion: 'reduce' })

  // Files: open src and its folders so the tree shows its branch lines and the svn marks
  await tab('Files')
  await page.waitForSelector('text=README.md')
  for (const dir of ['src', 'editor', 'search', 'sync']) {
    await page.getByText(dir, { exact: true }).first().click()
    await pause(500)
  }
  await page.getByRole('button', { name: /^Changed files only/ }).click()
  await pause(2000)
  await page.getByText('+10', { exact: true }).first().click()
  await page.keyboard.press('l')
  await pause(2500)
  await shot('03-web-svn-files')

  // The issue list: the header with the project list beside User and Version, and no Tip line
  await tab('List')
  await page.waitForSelector('[role="row"]')
  await pause(800)
  await shot('04-web-header-projects')
} finally {
  await browser.close()
  server.kill()
}
