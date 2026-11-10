// Capture the web issue detail with KaTeX math, a Mermaid diagram, a footnote and a snippet, light and dark.
// usage: web.mjs <repo> <out dir> <ctl dir> <issue id>   -- MOAI and Playwright (NODE_PATH) come from make.sh.
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const { chromium } = createRequire(import.meta.url)('playwright')
const [repo, outDir, ctlDir] = process.argv.slice(2, 5).map((p) => path.resolve(p))
const id = process.argv[5]
if (!repo || !outDir || !ctlDir || !id) throw new Error('usage: web.mjs <repo> <out dir> <ctl dir> <issue id>')
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
async function shoot(scheme, name, scroll) {
  const context = await browser.newContext({ viewport: { width: 1400, height: 900 },
    colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
  const page = await context.newPage()
  try {
    await page.goto(url)
    await page.waitForSelector('[role="row"]')
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.locator('[role="row"]', { hasText: 'Offline sync' }).first().locator('.moai-toggle').click()
    await page.locator('[role="row"]', { hasText: id }).first().click()
    await page.waitForSelector('math', { state: 'attached' })
    await page.waitForSelector('img.m-md-diagram')
    await page.waitForFunction(() => [...document.querySelectorAll('img.m-md-diagram')].every((i) => i.complete && i.naturalWidth > 0))
    await pause(800)
    if (scroll) { await page.getByText(scroll).last().scrollIntoViewIfNeeded(); await pause(600) }
    await page.mouse.move(1398, 898)
    await page.screenshot({ path: path.join(outDir, `${name}.png`) })
  } finally { await context.close() }
}
try {
  await shoot('light', '01-web-markdown-light')
  await shoot('dark', '02-web-markdown-dark', 'Last-writer-wins')
} finally {
  await browser.close()
  server.kill()
}
