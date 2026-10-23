// moai web 을 띄워 Playwright 의 화면 녹화로 장면을 찍는다.
//
//     MOAI=<moai 바이너리> node scripts/site-demo/record-web.mjs <seed 저장소> <내보낼 디렉터리>
//
// playwright 는 이 저장소의 의존이 아니다 — 부르는 쪽이 NODE_PATH 로 건넨다. 장면마다 브라우저
// 문맥을 하나씩 열어 webm 하나와 첫 화면 png(포스터) 하나를 낸다. 녹화에는 마우스가 안 찍혀서
// 가짜 커서를 페이지에 얹는다 — 보는 사람이 무엇을 눌렀는지 따라올 수 있게.
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync, readFileSync, renameSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const [repo, outDir] = process.argv.slice(2).map((p) => path.resolve(p))
if (!repo || !outDir) throw new Error('usage: record-web.mjs <seed repo> <out dir>')
mkdirSync(outDir, { recursive: true })
const moai = process.env.MOAI || 'moai'
const SIZE = { width: 1440, height: 900 }

const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env: { ...process.env, MOAI_NO_UPDATE_CHECK: '1', MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml') } })
const first = await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
})
const { url } = JSON.parse(first)

const CURSOR = `
  addEventListener('DOMContentLoaded', () => {
    const c = document.createElement('div')
    c.style.cssText = 'position:fixed;left:0;top:0;width:18px;height:18px;margin:-9px 0 0 -9px;border-radius:50%;' +
      'background:rgba(232,121,74,.85);box-shadow:0 0 0 4px rgba(232,121,74,.25);pointer-events:none;z-index:2147483647;' +
      'transition:transform .08s ease'
    document.body.appendChild(c)
    addEventListener('mousemove', (e) => { c.style.left = e.clientX + 'px'; c.style.top = e.clientY + 'px' }, true)
    addEventListener('mousedown', () => { c.style.transform = 'scale(.7)' }, true)
    addEventListener('mouseup', () => { c.style.transform = '' }, true)
  })`

async function glide(page, locator) {
  const box = await locator.boundingBox()
  if (!box) throw new Error('no box for ' + locator)
  await page.mouse.move(box.x + Math.min(box.width / 2, 120), box.y + box.height / 2, { steps: 25 })
  await page.waitForTimeout(250)
  await page.mouse.down(); await page.mouse.up()
}

async function wheel(page, dy, times, pause = 350) {
  for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(pause) }
}

const ids = Object.fromEntries(readFileSync(path.join(repo, '.demo-ids'), 'utf8').trim().split('\n').map((l) => l.split('=')))

const row = (page, title) => page.locator(`[role="row"] .m-row-title[title="${title}"]`).first()

async function keys(page, ...seq) {
  for (const k of seq) { await page.keyboard.press(k); await page.waitForTimeout(120) }
}

const scenes = {
  // 이슈 상세 읽기: 트리를 펼쳐 에픽과 이슈를 차례로 고르고, 본문·커밋·이력까지 내려 읽는다
  async detail(page) {
    await page.goto(url)
    await page.waitForTimeout(1200)
    await glide(page, row(page, 'v1.4 — Sync'))
    await keys(page, 'Tab', 'Space', 'o', 'd', 'Escape')
    await page.waitForTimeout(1400)
    await glide(page, row(page, 'Offline sync'))
    await page.waitForTimeout(2200)
    await glide(page, row(page, 'Resolve conflicts when two devices edit the same note'))
    await page.waitForTimeout(2600)
    await page.mouse.move(360, 600, { steps: 20 })
    await wheel(page, 120, 6, 500)
    await page.waitForTimeout(800)
    const history = page.getByText('History', { exact: true }).first()
    if (await history.count()) { await glide(page, history); await page.waitForTimeout(800); await wheel(page, 120, 6, 450) }
    await page.waitForTimeout(2200)
  },
  // 위키: 첫 페이지에서 링크를 따라가고, 본문의 이슈 id 를 눌러 그 이슈로 건너간다
  async wiki(page) {
    await page.goto(url)
    await page.waitForTimeout(1200)
    await glide(page, page.getByText('Wiki', { exact: true }).first())
    await page.waitForTimeout(1200)
    await glide(page, page.getByText('Lantern handbook', { exact: true }).first())
    await page.waitForTimeout(2400)
    await glide(page, page.locator('.m-wiki-scroll').getByText('Sync protocol', { exact: true }).first())
    await page.waitForTimeout(2800)
    await glide(page, page.locator('.m-wiki-scroll').getByText('Architecture', { exact: true }).first())
    await page.waitForTimeout(2600)
    await glide(page, page.locator('.m-wiki-scroll').getByText(ids.conflict, { exact: true }).first())
    await page.waitForTimeout(3200)
  },
}

const browser = await chromium.launch()
try {
  for (const [name, play] of Object.entries(scenes)) {
    if (process.env.SCENE && process.env.SCENE !== name) continue
    const context = await browser.newContext({ viewport: SIZE, recordVideo: { dir: outDir, size: SIZE }, colorScheme: 'dark', deviceScaleFactor: 1 })
    await context.addInitScript(CURSOR)
    const page = await context.newPage()
    await play(page)
    await page.screenshot({ path: path.join(outDir, `web-${name}.png`) })
    const video = page.video()
    await context.close()
    renameSync(await video.path(), path.join(outDir, `web-${name}.webm`))
    console.log('recorded', name)
  }
} finally {
  await browser.close()
  server.kill()
}
