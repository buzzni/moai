// v0.34.0 소식의 moai web 그림(moai-skjr): 목록의 끝낸 줄이 옅은 초록으로 칠해지고(집은 줄은 금빛, 리뷰는 보라 — v0.32.0),
// 간트의 줄 칸(트리)도 같은 세 빛을 받는다. 날짜 칸은 그대로다. v0.32.0/web.mjs 와 scripts/site-demo/record-web.mjs 의 틀을 빌렸다.
//
//     MOAI=<moai 바이너리> NODE_PATH=<playwright 가 든 node_modules> node release-posts/v0.34.0/web.mjs <seed 저장소> <내보낼 디렉터리>
//
// playwright 는 이 저장소의 의존이 아니다 — 부르는 쪽이 NODE_PATH 로 건넨다.
// 'Search that feels instant' 에픽에는 seed.sh 가 끝낸 줄(Full-text index…)·리뷰 줄(Highlight matches…)·집은 줄(Fuzzy matching…)을
// 하나씩 세워 둔다('Offline sync' 의 끝낸 줄은 14일 전에 닫혀 목록에 안 선다) — 세 빛이 한 화면에 나란히 선다. 상자는 바뀐 자리에 친다: 목록은 끝낸 줄, 간트는 세 빛을 받은 줄 칸.
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
// 데모가 쓴 줄은 [NEW] 로 선다 — 그림에 안 싣게 읽어 둔다
execFileSync(moai, ['read', '--all', '--user', 'Mira Chen (mira@example.com)'], { cwd: repo, env, stdio: 'ignore' })
const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env })
const { url } = JSON.parse(await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (c) => reject(new Error(`moai web exited ${c}`)))
}))
const SIZE = { width: 1440, height: 900 }
const DONE = 'Full-text index in a background worker'
const REVIEW = 'Highlight matches in results'
const PROGRESS = 'Fuzzy matching for small typos'
const row = (page, title) => page.locator(`[role="row"]:has(.m-row-title[title="${title}"])`).first()
const gridRow = (page, title) => page.locator(`.m-gantt .g-row:has(.c-title[title="${title}"])`).first()

async function keys(page, ...seq) {
  for (const k of seq) { await page.keyboard.press(k); await page.waitForTimeout(150) }
}
async function open(page) {
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  await page.waitForTimeout(1200)
  // 마일스톤 줄을 고르고 Tab 으로 그 아래를 다 편다 — 에픽과 멤버가 선다
  await page.locator('.m-row-title[title="v1.4 — Sync"]').click()
  await page.keyboard.press('Tab')
  await page.waitForSelector(`.m-row-title[title="${DONE}"]`)
  // 커서(고른 줄)는 빛을 안 받는다 — 마일스톤 줄에 둔 채로 찍고, 마우스는 화면 밖으로 뺀다(올린 줄도 빛을 안 받는다)
  await page.mouse.move(SIZE.width - 2, SIZE.height - 2)
  await page.waitForTimeout(1200)
}
// 상자는 record-web.mjs 의 still() 과 같다. 여러 줄은 한 상자로 묶는다
async function still(page, file, groups) {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.waitForTimeout(400)
  const rects = []
  for (const g of groups) {
    const bs = (await Promise.all(g.map((l) => l.boundingBox()))).filter(Boolean)
    if (bs.length !== g.length) throw new Error(`${file}: a row to box is not on the screen`)
    const x = Math.min(...bs.map((b) => b.x)), y = Math.min(...bs.map((b) => b.y))
    rects.push({ x, y, width: Math.max(...bs.map((b) => b.x + b.width)) - x, height: Math.max(...bs.map((b) => b.y + b.height)) - y })
  }
  await page.evaluate((rs) => {
    for (const r of rs) {
      const d = document.createElement('div')
      Object.assign(d.style, {
        position: 'fixed', left: `${r.x - 6}px`, top: `${r.y - 4}px`, width: `${r.width + 12}px`, height: `${r.height + 8}px`,
        border: '3px solid #f08a4b', borderRadius: '8px', boxShadow: '0 0 0 4px rgba(240,138,75,.25)', pointerEvents: 'none', zIndex: 2147483646,
      })
      document.body.appendChild(d)
    }
  }, rects)
  await page.screenshot({ path: path.join(outDir, file) })
}

const browser = await chromium.launch()
try {
  // 목록: 밝은 판과 어두운 판. 상자는 끝낸 줄 하나 — 금빛·보라는 v0.32.0 에 섰다
  for (const [scheme, file] of [['light', '01-web-list.png'], ['dark', '02-web-list-dark.png']]) {
    const ctx = await browser.newContext({ viewport: SIZE, colorScheme: scheme, deviceScaleFactor: 2, timezoneId: 'UTC' })
    const page = await ctx.newPage()
    await open(page)
    // 상세 칸은 접는다 — 목록이 넓어야 줄의 빛이 잘 보인다(SPC v d, 메뉴는 Escape 로 닫는다)
    await keys(page, 'Space', 'v', 'd', 'Escape')
    await page.waitForTimeout(800)
    await still(page, file, [[row(page, DONE)]])
    await ctx.close()
  }
  // 간트: 줄 칸의 세 빛. 날짜 칸은 칠하지 않는다
  {
    const ctx = await browser.newContext({ viewport: SIZE, colorScheme: 'light', deviceScaleFactor: 2, timezoneId: 'UTC' })
    const page = await ctx.newPage()
    await open(page)
    await keys(page, 'Space', 'g', 'g')
    await page.waitForSelector('.m-gantt .lane .bar')
    await page.waitForTimeout(1200)
    await keys(page, 'Space', 'v', 'd', 'Escape')
    await page.keyboard.press('t')
    await page.mouse.move(SIZE.width - 2, SIZE.height - 2)
    await page.waitForTimeout(1500)
    await still(page, '03-web-gantt.png', [[gridRow(page, DONE), gridRow(page, REVIEW), gridRow(page, PROGRESS)]])
    await ctx.close()
  }
} finally {
  await browser.close()
  server.kill()
}
