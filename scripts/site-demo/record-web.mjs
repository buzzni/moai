// moai web 을 띄워 Playwright 의 화면 녹화로 장면을 찍는다.
//
//     MOAI=<moai 바이너리> node scripts/site-demo/record-web.mjs <seed 저장소> <내보낼 디렉터리>
//
// playwright 는 이 저장소의 의존이 아니다 — 부르는 쪽이 NODE_PATH 로 건넨다. 장면마다 브라우저
// 문맥을 하나씩 열어 날 webm 과 png 포스터를 낸다. 녹화에는 마우스가 안 찍혀서
// 가짜 커서를 페이지에 얹는다 — 보는 사람이 무엇을 눌렀는지 따라올 수 있게.
// recorded <장면> lead <초> 는 첫 목록이 선 시각이다. 사이트용 자르기와 VP9 굽기는 record-site.sh 가 맡는다.
// 릴리스 소식은 예전처럼 날 영상을 받는다. 위키 포스터는 장면 안에서 페이지가 선 때 찍는다.
import { execFileSync, spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdirSync, readFileSync, renameSync } from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const [repo, outDir] = process.argv.slice(2).map((p) => path.resolve(p))
if (!repo || !outDir) throw new Error('usage: record-web.mjs <seed repo> <out dir>')
mkdirSync(outDir, { recursive: true })
// 이 저장소의 빌드가 기본이다. 상대 경로는 절대 경로로 굳힌다 — 서버는 데모 저장소에서 뜬다
const moai = path.resolve(process.env.MOAI || path.join(path.dirname(new URL(import.meta.url).pathname), '../../target/release/moai'))
const SIZE = { width: 1440, height: 900 }

// 녹화하는 사람의 환경이 화면에 새지 않게: 정체·키·색·말·시간대는 데모의 것으로
function demoEnv() {
  const env = { ...process.env, TZ: 'UTC', MOAI_NO_UPDATE_CHECK: '1', MOAI_LANG: 'en', MOAI_CONFIG: path.join(repo, '.demo-home', 'config.toml') }
  for (const k of ['MOAI_ACTOR', 'MOAI_KEYS', 'NO_COLOR', 'MOAI_NOW']) delete env[k]
  return env
}

const server = spawn(moai, ['web', '--port', '0', '--json'], { cwd: repo, env: demoEnv() })
const first = await new Promise((resolve, reject) => {
  createInterface({ input: server.stdout }).once('line', resolve)
  server.once('exit', (code) => reject(new Error(`moai web exited ${code}`)))
})
const { url } = JSON.parse(first)

const CURSOR = `
  addEventListener('DOMContentLoaded', () => {
    const c = document.createElement('div')
    c.id = 'demo-cursor'
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

// 그 자리를 놓지 않고 끈다 — 단추를 누른 채 steps 걸음으로 옮기고 놓는다
async function dragBy(page, x0, y0, dx, dy = 0, steps = 30) {
  await page.mouse.move(x0, y0, { steps: 15 })
  await page.waitForTimeout(300)
  await page.mouse.down()
  await page.mouse.move(x0 + dx, y0 + dy, { steps })
  await page.waitForTimeout(250)
  await page.mouse.up()
}

const center = async (locator) => {
  const b = await locator.boundingBox()
  if (!b) throw new Error('no box for ' + locator)
  return { x: b.x + b.width / 2, y: b.y + b.height / 2, b }
}

// 읽음은 쓴 사람을 안 가린다 — 장면이 쓴 줄(막대 끌기)이 Mira 자신에게 [NEW] 로 선다. 페이지는 표식이 움직이면
// 다시 읽으니 뒤에서 읽어 두면 내린다(moai-vj8w.fuh)
function readAll() {
  execFileSync(moai, ['read', '--all', '--user', 'Mira Chen (mira@example.com)'], { cwd: repo, env: demoEnv(), stdio: 'ignore' })
}

// 소식의 정지 그림: 집은 줄의 제목에 도는 빛줄기를 멈추고(prefers-reduced-motion — 검색이 친 자리로 읽힌다),
// 고른 것들에 shot.mjs 와 같은 상자를 친다. 상자는 찍고 걷는다 — 녹화에 남지 않게
async function still(page, file, ...targets) {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.evaluate(() => document.getElementById('demo-cursor')?.style.setProperty('visibility', 'hidden'))
  await page.waitForTimeout(300)
  const rects = []
  for (const t of targets) rects.push(await t.boundingBox())
  await page.evaluate((rs) => {
    for (const r of rs.filter(Boolean)) {
      const d = document.createElement('div')
      d.className = 'demo-box'
      Object.assign(d.style, {
        position: 'fixed', left: `${r.x - 6}px`, top: `${r.y - 4}px`, width: `${r.width + 12}px`, height: `${r.height + 8}px`,
        border: '3px solid #f08a4b', borderRadius: '8px', boxShadow: '0 0 0 4px rgba(240,138,75,.25)', pointerEvents: 'none', zIndex: 2147483646,
      })
      document.body.appendChild(d)
    }
  }, rects)
  await page.screenshot({ path: path.join(outDir, file) })
  await page.evaluate(() => document.querySelectorAll('.demo-box').forEach((d) => d.remove()))
  await page.evaluate(() => document.getElementById('demo-cursor')?.style.removeProperty('visibility'))
  await page.emulateMedia({ reducedMotion: 'no-preference' })
}

let t0 = 0, lead = 0, posterTaken = false, sceneName = ''
async function open(page) {
  await page.goto(url)
  await page.waitForSelector('[role="row"]')
  // DOM 에 줄이 생긴 뒤 첫 그림까지 기다린다 — 녹화에 loading 프레임이 남지 않게.
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  lead = (Date.now() - t0) / 1000
}

async function poster(page) {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.evaluate(() => document.getElementById('demo-cursor')?.style.setProperty('visibility', 'hidden'))
  await page.waitForTimeout(300)
  await page.screenshot({ path: path.join(outDir, `web-${sceneName}.png`) })
  await page.evaluate(() => document.getElementById('demo-cursor')?.style.removeProperty('visibility'))
  await page.emulateMedia({ reducedMotion: 'no-preference' })
  posterTaken = true
}

const scenes = {
  // 간트 (v0.25.0 의 소식, moai-vj8w.fuh). 계획한 날과 막는 줄은 plan.sh 가 단다 — 이 장면만 그것이 든 저장소를 받는다.
  // 마우스는 'Sync attachments larger than 10 MB' 를 겨눈다: 앞으로 잡힌 막대라 끌어 옮길 수 있고, 두 줄에 막혀 화살표가 선다
  async gantt(page) {
    const TARGET = 'Sync attachments larger than 10 MB'
    await open(page)
    await page.waitForTimeout(1200)
    await keys(page, 'Space', 'g', 'g')
    await page.waitForSelector('.m-gantt .lane .bar')
    await page.waitForTimeout(1600)
    // 걸음: 날 → 시간 → 날 → 주 → 달 → 날, 그리고 오늘
    for (const k of ['=', '-', '-', '-', '=', '=']) { await page.keyboard.press(k); await page.waitForTimeout(900) }
    await page.keyboard.press('t'); await page.waitForTimeout(1200)
    const right = page.locator('.m-gantt .g-right .g-lanes')
    const { x: rx, y: ry } = await center(right)
    // 옆으로: Shift+휠, 날짜 머리 끌기, Ctrl+휠로 당기고 밀기
    await page.mouse.move(rx, ry, { steps: 20 })
    await page.keyboard.down('Shift'); await wheel(page, 240, 4, 300); await wheel(page, -240, 4, 300); await page.keyboard.up('Shift')
    const head = await center(page.locator('.m-gantt .g-right .g-head'))
    await dragBy(page, head.x, head.y, -260)
    await page.waitForTimeout(500)
    await dragBy(page, head.x - 260, head.y, 260)
    await page.mouse.move(rx, ry, { steps: 10 })
    await page.keyboard.down('Control'); await wheel(page, 120, 1, 900); await wheel(page, -120, 1, 900); await page.keyboard.up('Control')
    await page.keyboard.press('t'); await page.waitForTimeout(900)
    // 커서 줄: 막는 줄에서 화살표가 선다. 그 줄의 막대에 말풍선
    const gridRow = page.locator(`.m-gantt .g-row:has(.c-title[title="${TARGET}"])`).first()
    // 차례는 같은 p 끼리 seed 마다 새로 뜨는 id 로 갈려 그 줄이 창 아래에 설 수 있다 — 말풍선이 잘리지 않게 위에서 다섯째 줄로 올린다. 고르기 전에 — 고른 뒤에 굴리면 고른 것이 풀렸다
    await gridRow.evaluate((n) => {
      // 줄은 28px(gantt-model.ts 의 ROW_H) — 붙어 서는 머리의 밑에서 줄 넷만큼 재야 맨 윗줄이 반쯤 잘리지 않는다
      const box = n.closest('.g-scroll')
      const head = box.querySelector('.g-left .g-head').getBoundingClientRect().bottom
      box.scrollTop += n.getBoundingClientRect().top - head - 28 * 4
    })
    await page.waitForTimeout(800)
    await glide(page, gridRow)
    await page.waitForTimeout(1400)
    // 말풍선은 줄 아래에 서서, 그 줄이 창 아래쪽이면 상세 칸에 가려 잘린다 — 정지 그림 동안 상세를 접는다(SPC v d — 메뉴는 열린 채라 Escape 로 닫는다)
    await keys(page, 'Space', 'v', 'd', 'Escape')
    await page.waitForTimeout(1000)
    const id = await gridRow.getAttribute('data-id')
    const lane = page.locator(`.m-gantt .lane[data-id="${id}"]`)
    const plan = lane.locator('.bar.b-plan').first()
    const p = await center(plan)
    await page.mouse.move(p.x, p.y, { steps: 20 })
    await page.waitForTimeout(2200)
    await still(page, 'web-gantt-mark.png', gridRow, lane)
    await keys(page, 'Space', 'v', 'd', 'Escape')
    await page.waitForTimeout(1000)
    // 막대를 하루 뒤로 끈다 — 놓을 때 한 번 쓴다(moai edit --start --due). 날 걸음에서 하루는 40px(gantt-model.ts 의 PX_PER_DAY).
    // 이틀이면 마감이 에픽의 마감(plan.sh 의 +5 대 seed.sh 마일스톤의 +6)을 넘어 거절된다
    await dragBy(page, p.x, p.y, 40, 0, 24)
    await page.waitForTimeout(1500)
    readAll()
    await page.waitForTimeout(1500)
    // 줄 칸의 너비를 끈다 — 이 브라우저가 기억한다
    // 손잡이는 줄 칸의 높이를 다 덮어 가운데도 꼭대기도 화면 밖일 수 있다 — 줄 칸 머리의 높이에서 잡는다
    const split = await center(page.locator('.m-gantt .g-split'))
    const gridHead = await center(page.locator('.m-gantt .g-left .g-head'))
    await dragBy(page, split.x, gridHead.y, 140)
    await page.waitForTimeout(2400)
  },

  // 막대 끌기만 (v0.27.0 의 소식, moai-0fai). 놓으면 날이 아니라 옮긴 만큼을 쓴다(moai edit --shift) — 그래서 그림은
  // v0.25.0 의 끌기와 같고, 다른 것은 뒤에서 옮겨진 줄도 거기서부터 옮겨 간다는 것이다. 계획한 날은 plan.sh 가 단다
  async drag(page) {
    const TARGET = 'Sync attachments larger than 10 MB'
    await open(page)
    await page.waitForTimeout(1000)
    await keys(page, 'Space', 'g', 'g')
    await page.waitForSelector('.m-gantt .lane .bar')
    await page.keyboard.press('t'); await page.waitForTimeout(1000)
    const gridRow = page.locator(`.m-gantt .g-row:has(.c-title[title="${TARGET}"])`).first()
    await gridRow.evaluate((n) => {
      const box = n.closest('.g-scroll')
      const head = box.querySelector('.g-left .g-head').getBoundingClientRect().bottom
      box.scrollTop += n.getBoundingClientRect().top - head - 28 * 4
    })
    await page.waitForTimeout(600)
    await glide(page, gridRow)
    await page.waitForTimeout(1000)
    const id = await gridRow.getAttribute('data-id')
    const plan = page.locator(`.m-gantt .lane[data-id="${id}"] .bar.b-plan`).first()
    const p = await center(plan)
    await page.mouse.move(p.x, p.y, { steps: 20 })
    await page.waitForTimeout(1200)
    // 하루 뒤로(날 걸음에서 하루는 40px) — 이틀이면 에픽의 마감을 넘어 거절된다. 그다음 하루 앞으로 되돌린다
    await dragBy(page, p.x, p.y, 40, 0, 24)
    await page.waitForTimeout(1500)
    readAll()
    await page.waitForTimeout(1200)
    await still(page, 'web-drag-mark.png', gridRow, page.locator(`.m-gantt .lane[data-id="${id}"]`))
    const q = await center(plan)
    await dragBy(page, q.x, q.y, -40, 0, 24)
    await page.waitForTimeout(1500)
    readAll()
    await page.waitForTimeout(1500)
  },
  // 이슈 패널의 Wiki 토막 (v0.27.0 의 소식, moai-0fai): 커밋 뒤에 그 이슈를 이름 댄 페이지와 관련 절이 선다
  async wikipanel(page) {
    await open(page)
    await page.waitForTimeout(1200)
    await glide(page, row(page, 'v1.4 — Sync'))
    await keys(page, 'Tab', 'Space', 'o', 'd', 'Escape')
    await page.waitForTimeout(1200)
    await glide(page, row(page, 'Resolve conflicts when two devices edit the same note'))
    await page.waitForTimeout(2000)
    const wiki = page.locator('.moai-detail-wiki').first()
    await wiki.scrollIntoViewIfNeeded()
    await page.waitForTimeout(800)
    await still(page, 'web-wikipanel-mark.png', wiki)
  },
  // 이슈 상세 읽기: 트리를 펼쳐 에픽과 이슈를 차례로 고르고, 본문·커밋·이력까지 내려 읽는다
  async detail(page) {
    await open(page)
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
  // 사용 방법의 칸반 절(moai-vlq7): 마일스톤마다 한 줄의 보드에서 칸을 건너고 카드를 골라 상세를 읽는다
  async board(page) {
    await open(page)
    await page.waitForTimeout(1200)
    await keys(page, 'Space', 'g', 'b')
    await page.waitForTimeout(2000)
    for (const k of ['l', 'l', 'j', 'l', 'j', 'j', 'h']) { await page.keyboard.press(k); await page.waitForTimeout(800) }
    await page.waitForTimeout(800)
    await glide(page, page.getByText('Resolve conflicts when two devices edit the same note', { exact: true }).first())
    await page.waitForTimeout(2600)
    await glide(page, page.getByText('Invite people to a notebook by email', { exact: true }).first())
    await page.waitForTimeout(2600)
  },
  // 위키: 첫 페이지에서 링크를 따라가고, 본문의 이슈 id 를 눌러 그 이슈로 건너간다
  async wiki(page) {
    await open(page)
    await page.waitForTimeout(1200)
    await glide(page, page.getByText('Wiki', { exact: true }).first())
    await page.waitForTimeout(1200)
    await glide(page, page.getByText('Lantern handbook', { exact: true }).first())
    await page.waitForTimeout(2400)
    await glide(page, page.locator('.m-wiki-scroll').getByText('Sync protocol', { exact: true }).first())
    await page.waitForTimeout(2800)
    await poster(page)
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
    const context = await browser.newContext({ viewport: SIZE, recordVideo: { dir: outDir, size: SIZE }, colorScheme: 'dark', deviceScaleFactor: 1, timezoneId: 'UTC' })
    await context.addInitScript(CURSOR)
    // newPage 를 기다리는 동안에도 영상이 시작된다 — 그 앞에서 재야 처음의 빈 화면을 다 자른다.
    t0 = Date.now(); lead = 0; posterTaken = false; sceneName = name
    const page = await context.newPage()
    await play(page)
    if (!posterTaken) await poster(page)
    const video = page.video()
    await context.close()
    renameSync(await video.path(), path.join(outDir, `web-${name}.webm`))
    console.log('recorded', name, 'lead', lead.toFixed(2))
  }
} finally {
  await browser.close()
  server.kill()
}
