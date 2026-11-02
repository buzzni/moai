// Capture the web issue detail (light, dark) and a wiki page, each with an attached picture.
// usage: web.mjs <repo> <out dir> <ctl dir>   -- MOAI and Playwright (NODE_PATH) come from make.sh.
import { execFileSync, spawn } from 'node:child_process'
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
const call = (...args) => execFileSync(moai, ['-C', repo, 'ctl', ...args], { env, encoding: 'utf8' })
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
async function visibleTab() {
  for (let attempt = 0; attempt < 40; attempt++) {
    const tab = JSON.parse(call('ls', '--json')).windows.find((row) => row.kind === 'web')
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
const title = 'Resolve conflicts when two devices edit the same note'
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
}))
const browser = await chromium.launch()
async function fresh(scheme) {
  const context = await browser.newContext({ viewport: { width: 1600, height: 900 },
    colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
  const page = await context.newPage()
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await ctl('theme', scheme)
  await pause(800)
  return { context, page }
}
async function settle(page) {
  await page.waitForFunction(() => [...document.images].every((i) => i.complete && i.naturalWidth > 0))
  await pause(500)
  await page.mouse.move(1598, 898)
}
try {
  for (const [scheme, name] of [['light', '01-web-issue-picture-light'], ['dark', '02-web-issue-picture-dark']]) {
    const { context, page } = await fresh(scheme)
    try {
      for (let n = 0; n < 6; n++) {
        if (await page.locator(`[role="row"] .m-row-title[title="${title}"]`).count()) break
        const toggle = page.locator('[role="row"]:has(.moai-toggle[aria-expanded="false"]) .moai-toggle').first()
        if (!(await toggle.count())) break
        await toggle.click()
        await pause(300)
      }
      await page.locator(`[role="row"] .m-row-title[title="${title}"]`).first().click()
      await pause(1200)
      const img = page.locator('img').filter({ hasNot: page.locator('nothing') }).last()
      await img.scrollIntoViewIfNeeded()
      await settle(page)
      await page.screenshot({ path: path.join(outDir, `${name}.png`) })
    } finally { await context.close() }
  }
  const { context, page } = await fresh('light')
  try {
    await page.getByText('Wiki', { exact: true }).first().click()
    await pause(1200)
    await page.locator('.m-wiki-scroll').getByText('Sync protocol', { exact: true }).first().click()
    await pause(1200)
    await page.locator('img').last().scrollIntoViewIfNeeded()
    await settle(page)
    await page.screenshot({ path: path.join(outDir, '03-web-wiki-picture.png') })
  } finally { await context.close() }
} finally {
  await browser.close()
  server.kill()
}
