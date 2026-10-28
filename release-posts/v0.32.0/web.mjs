// v0.32.0 소식의 moai web 그림(moai-gka0): 상세가 오른쪽에 서고 목록 줄이 집은 일·리뷰 일을 칠한 정지 그림 하나와,
// 상세를 오른쪽 → 아래 → 왼쪽 → 위로 옮기는 영상 하나. record-web.mjs 의 틀을 빌렸다.
//
//     MOAI=<moai 바이너리> node release-posts/v0.32.0/web.mjs <seed 저장소> <내보낼 디렉터리>
//
// playwright 는 이 저장소의 의존이 아니다 — 부르는 쪽이 NODE_PATH 로 건넨다.
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync, renameSync } from 'node:fs'
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
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (c) => reject(new Error(`moai web exited ${c}`)))
}))
const SIZE = { width: 1440, height: 900 }
const TITLE = 'Resolve conflicts when two devices edit the same note'
const row = (page, title) => page.locator(`[role="row"] .m-row-title[title="${title}"]`).first()
const toggle = (page) => page.locator('[aria-label^="Move details to"]').first()
const CURSOR = `addEventListener('DOMContentLoaded', () => { const c = document.createElement('div'); c.id = 'demo-cursor'
  c.style.cssText = 'position:fixed;left:0;top:0;width:18px;height:18px;margin:-9px 0 0 -9px;border-radius:50%;background:rgba(232,121,74,.85);box-shadow:0 0 0 4px rgba(232,121,74,.25);pointer-events:none;z-index:2147483647'
  document.body.appendChild(c); addEventListener('mousemove', (e) => { c.style.left = e.clientX + 'px'; c.style.top = e.clientY + 'px' }, true) })`
async function open(page) {
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  await page.waitForTimeout(1200)
  await page.locator('.m-row-title[title="v1.4 — Sync"]').click()
  await page.keyboard.press('Tab')
  await page.waitForSelector(`.m-row-title[title="${TITLE}"]`)
  await page.waitForTimeout(1200)
}
async function click(page, loc) {
  const b = await loc.boundingBox()
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 25 })
  await page.waitForTimeout(250); await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2)
}
const browser = await chromium.launch()
try {
  // 정지 그림
  {
    const ctx = await browser.newContext({ viewport: SIZE, colorScheme: 'dark', deviceScaleFactor: 2, timezoneId: 'UTC' })
    const page = await ctx.newPage()
    await open(page)
    await row(page, TITLE).click()
    await page.waitForTimeout(1500)
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await page.waitForTimeout(400)
    await page.screenshot({ path: path.join(outDir, '04-web.png') })
    await ctx.close()
  }
  // 영상: 네 방향
  {
    const ctx = await browser.newContext({ viewport: SIZE, recordVideo: { dir: outDir, size: SIZE }, colorScheme: 'dark', deviceScaleFactor: 1, timezoneId: 'UTC' })
    await ctx.addInitScript(CURSOR)
    const page = await ctx.newPage()
    await open(page)
    await click(page, row(page, TITLE))
    await page.waitForTimeout(1800)
    for (let i = 0; i < 4; i++) { await click(page, toggle(page)); await page.waitForTimeout(2200) }
    const video = page.video()
    await ctx.close()
    renameSync(await video.path(), path.join(outDir, '05-web-detail.webm'))
  }
} finally {
  await browser.close()
  server.kill()
}
