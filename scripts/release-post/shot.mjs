// 릴리스 소식에 실을 그림을 찍는다. 바뀐 자리에는 테두리 상자를 친다.
//
//     NODE_PATH=<playwright 가 든 node_modules> node scripts/release-post/shot.mjs tui <cast> --at 12 --mark 'SPC x' -o keys.png
//     NODE_PATH=…  node scripts/release-post/shot.mjs tui <cast> --from 3 --to 11 --mark 'quit' -o keys.webm
//     NODE_PATH=…  node scripts/release-post/shot.mjs web http://127.0.0.1:6624 --mark 'Theme' --select '.theme' -o web.png
//
// tui 는 asciinema 녹화(scripts/site-demo 가 찍는 것)의 한 장면을 홈페이지와 같은 창틀에 그려 찍는다.
// `-o` 가 .webm 이면 --from 부터 --to 까지 재생하는 모습을 영상으로 담는다 — 움직임이 변경점일 때 쓴다.
// web 은 그 주소를 열어 찍는다. 상자를 칠 자리는 둘로 고른다.
//
//   --mark <글>     화면에 그 글이 처음 나오는 자리 (--mark-all 이면 모든 자리)
//   --line <글>     그 글이 든 줄 전체 (줄 앞뒤의 빈칸은 뺀다)
//   --cells r,c,w,h tui 의 칸 좌표 (0 부터, 줄·칸·너비·높이)
//   --select <css>  web 의 요소
//
// --lines N 은 tui 의 위쪽 N 줄만 보인다 — 셸 장면은 위에서부터 차고 아래가 비어, 그대로 찍으면 그림의
// 절반이 빈 칸이다.
//
// 찾으라는 글이 화면에 없으면 멈추고 그 장면의 글을 찍어 준다 — 엉뚱한 장면을 상자 없이 내보내지 않게.
// playwright 는 이 저장소의 의존이 아니다 — record-web.mjs 처럼 부르는 쪽이 NODE_PATH 로 건넨다.
import { mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')

const here = path.dirname(new URL(import.meta.url).pathname)
const site = path.join(here, '../../site/assets')
const ACCENT = '#f08a4b'

function parse(argv) {
  const [mode, target, ...rest] = argv
  if (!['tui', 'web'].includes(mode) || !target) {
    throw new Error('usage: shot.mjs tui <cast> --at <s> | --from <s> --to <s> … -o <png|webm>\n       shot.mjs web <url> … -o <png>')
  }
  const o = { mode, target, mark: [], line: [], cells: [], select: [], markAll: false, width: 1100, size: '1440x900', wait: 1500, speed: 1, title: null, lines: 0 }
  for (let i = 0; i < rest.length; i++) {
    const k = rest[i]
    const v = () => {
      if (i + 1 >= rest.length) throw new Error(`${k} needs a value`)
      return rest[++i]
    }
    if (k === '--at') o.at = Number(v())
    else if (k === '--from') o.from = Number(v())
    else if (k === '--to') o.to = Number(v())
    else if (k === '--mark') o.mark.push(v())
    else if (k === '--mark-all') o.markAll = true
    else if (k === '--line') o.line.push(v())
    else if (k === '--cells') o.cells.push(v().split(',').map(Number))
    else if (k === '--select') o.select.push(v())
    else if (k === '--width') o.width = Number(v())
    else if (k === '--size') o.size = v()
    else if (k === '--wait') o.wait = Number(v())
    else if (k === '--speed') o.speed = Number(v())
    else if (k === '--title') o.title = v()
    else if (k === '--lines') o.lines = Number(v())
    else if (k === '-o' || k === '--out') o.out = path.resolve(v())
    else throw new Error(`unknown option ${k}`)
  }
  if (!o.out) throw new Error('-o <file> is required')
  o.video = o.out.endsWith('.webm')
  if (o.mode === 'tui' && !o.video && !(o.at >= 0)) throw new Error('tui needs --at <seconds> for a still')
  if (o.video && !(o.from >= 0 && o.to > o.from)) throw new Error('a .webm needs --from <s> --to <s>')
  if (o.video && o.mode === 'web') throw new Error('web takes stills only; record a web scene with scripts/site-demo/record-web.mjs')
  for (const c of o.cells) if (c.length !== 4 || c.some((n) => !Number.isFinite(n))) throw new Error('--cells is row,col,width,height')
  return o
}

// 페이지 안에서 도는 것. 글을 찾아 그 자리의 사각형을, 칸 좌표를 사각형으로 바꿔 상자를 친다.
// 문자열로 넘기는 까닭은 playwright 의 evaluate 가 함수 몸을 옮겨 실행하기 때문이다 — 바깥 변수를 못 잡는다.
const IN_PAGE = String.raw`
window.__boxes = function ({ mark, line: whole = [], markAll, cells, select, cols, rows, accent, scope }) {
  // 영상은 이것을 계속 부른다 — 화면이 밀리면 글도 밀리니, 앞서 친 상자를 걷고 지금 자리에 다시 친다
  for (const old of document.querySelectorAll('.shot-box')) old.remove()
  const root = scope ? document.querySelector(scope) : document.body
  const found = []
  const missing = []
  // 줄 단위로 글을 이어 붙여 찾는다 — 터미널은 한 줄이 색마다 여러 span 으로 쪼개진다
  const lines = root.querySelectorAll('.ap-line').length ? [...root.querySelectorAll('.ap-line')] : [root]
  const rangeOf = (nodes, at, end) => {
    const range = document.createRange()
    const pick = (off) => { let best = nodes[0]; for (const e of nodes) if (e[1] <= off) best = e; return best }
    const [sn, so] = pick(at)
    const [en, eo] = pick(end - 1)
    range.setStart(sn, at - so)
    range.setEnd(en, end - eo)
    return range.getBoundingClientRect()
  }
  const textOf = (line) => {
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT)
    const nodes = []
    let text = ''
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (!n.parentElement || n.parentElement.closest('script,style')) continue
      nodes.push([n, text.length])
      text += n.data
    }
    return { nodes, text }
  }
  // 그 글이 든 줄을 통째로 — 줄 앞뒤의 빈칸은 뺀다
  for (const needle of whole) {
    const line = lines.find((l) => l.textContent.includes(needle))
    if (!line) { missing.push(needle); continue }
    const { nodes, text } = textOf(line)
    const start = text.search(/\S/)
    const end = text.replace(/\s+$/, '').length
    found.push(rangeOf(nodes, start, end))
  }
  for (const needle of mark) {
    let hit = 0
    for (const line of lines) {
      const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT)
      const nodes = []
      let text = ''
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if (!n.parentElement || n.parentElement.closest('script,style')) continue
        nodes.push([n, text.length])
        text += n.data
      }
      let from = 0
      for (;;) {
        const at = text.indexOf(needle, from)
        if (at < 0) break
        const end = at + needle.length
        const range = document.createRange()
        const pick = (off) => { let best = nodes[0]; for (const e of nodes) if (e[1] <= off) best = e; return best }
        const [sn, so] = pick(at)
        const [en, eo] = pick(end - 1)
        range.setStart(sn, at - so)
        range.setEnd(en, end - eo)
        const r = range.getBoundingClientRect()
        if (r.width > 0 && r.height > 0) { found.push(r); hit++ }
        if (!markAll) break
        from = end
      }
      if (hit && !markAll) break
    }
    if (!hit) missing.push(needle)
  }
  for (const css of select) {
    const els = [...document.querySelectorAll(css)]
    if (!els.length) missing.push(css)
    for (const el of markAll ? els : els.slice(0, 1)) found.push(el.getBoundingClientRect())
  }
  if (cells.length) {
    // 줄의 자리는 그 줄 요소에서, 칸의 너비는 글자 하나를 재어 얻는다 — 터미널 요소에는 안쪽 여백이
    // 있어, 그 높이를 줄 수로 나누면 상자가 반 줄씩 밀린다
    const sample = lines.map(textOf).find((t) => t.text.length)
    const cw = sample ? rangeOf(sample.nodes, 0, 1).width : 0
    for (const [r, c, w, h] of cells) {
      const top = lines[r], bottom = lines[Math.min(r + h - 1, lines.length - 1)]
      if (!top || !cw) { missing.push('cells ' + [r, c, w, h].join(',')); continue }
      const a = top.getBoundingClientRect(), b = bottom.getBoundingClientRect()
      found.push(new DOMRect(a.left + c * cw, a.top, w * cw, b.bottom - a.top))
    }
  }
  for (const r of found) {
    const box = document.createElement('div')
    box.className = 'shot-box'
    // 터미널은 줄 사이가 좁아, 위아래 여백을 크게 주면 옆 줄의 글을 덮는다
    const px = 6, py = 2
    Object.assign(box.style, {
      position: 'absolute', zIndex: 99, pointerEvents: 'none', boxSizing: 'border-box',
      left: (r.left + scrollX - px) + 'px', top: (r.top + scrollY - py) + 'px',
      width: (r.width + px * 2) + 'px', height: (r.height + py * 2) + 'px',
      border: '3px solid ' + accent, borderRadius: '8px',
      boxShadow: '0 0 0 4px ' + accent + '40, 0 0 18px ' + accent + '66',
    })
    document.body.appendChild(box)
  }
  const visible = lines.map((l) => l.textContent.replace(/\s+$/, '')).filter(Boolean).join('\n')
  return { boxed: found.length, missing, visible }
}
`

function castSize(text) {
  const head = JSON.parse(text.slice(0, text.indexOf('\n')))
  const cols = head.term?.cols ?? head.width
  const rows = head.term?.rows ?? head.height
  if (!cols || !rows) throw new Error('cannot read the cast size from its header')
  return { cols, rows }
}

function tuiPage(o, cast, { cols, rows }) {
  const css = readFileSync(path.join(site, 'vendor/asciinema-player.css'), 'utf8')
  const js = readFileSync(path.join(site, 'vendor/asciinema-player.min.js'), 'utf8')
  // 녹화에 쓴 색을 홈페이지와 똑같이 맞춘다 — site.css 의 그 한 덩어리만 옮긴다
  const theme = readFileSync(path.join(site, 'site.css'), 'utf8').match(/\.asciinema-player-theme-moai \{[^}]*\}/)[0]
  const title = o.title ?? path.basename(o.target, '.cast').replace(/^(guide|tui)-/, 'moai ')
  const start = o.video ? o.from : o.at
  const opts = {
    cols, rows, fit: 'width', controls: false, theme: 'moai', idleTimeLimit: 2, speed: o.speed,
    terminalFontFamily: 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, "DejaVu Sans Mono", monospace',
    ...(o.video ? { autoPlay: true, startAt: start } : { autoPlay: false, poster: 'npt:' + start }),
  }
  return `<!doctype html><html><head><meta charset="utf-8"><style>${css}
${theme}
html,body{margin:0;background:#0e0f0e}
#shot{display:inline-block;padding:28px}
.window{width:${o.width}px;background:#0a0b0a;border:1px solid #272a27;border-radius:12px;overflow:hidden;box-shadow:0 30px 80px -40px rgba(0,0,0,.8)}
.bar{display:flex;align-items:center;gap:8px;padding:10px 14px;border-bottom:1px solid #272a27;background:#151715}
.bar i{width:11px;height:11px;border-radius:50%;background:#2c302c;display:block}
.bar span{margin-left:10px;font:12px ui-monospace,monospace;color:#6b6962}
#cast{background:#121212}
#cast .ap-wrapper,#cast .ap-player{border-radius:0}
#cast{position:relative;overflow:hidden}
/* 재생 단추·불러오는 중·조작 막대는 그림에 안 싣는다 */
#cast .ap-overlay,#cast .ap-control-bar{display:none!important}
</style></head><body><div id="shot"><div class="window"><div class="bar"><i></i><i></i><i></i><span>${title.replace(/</g, '&lt;')}</span></div><div id="cast"></div></div></div>
<script>${js}</script>
<script>window.__player = AsciinemaPlayer.create({ data: ${JSON.stringify(cast)} }, document.getElementById('cast'), ${JSON.stringify(opts)});</script>
<script>${IN_PAGE}</script>
<script>window.__lines = function (n) {
  const term = document.querySelector('#cast .ap-term') || document.querySelector('#cast pre')
  const cast = document.getElementById('cast')
  if (!n || !term) return
  const ch = term.getBoundingClientRect().height / ${rows}
  const top = term.getBoundingClientRect().top - cast.getBoundingClientRect().top
  cast.style.height = Math.ceil(top * 2 + ch * (n + 0.5)) + 'px'
}</script></body></html>`
}

async function boxes(page, o, size, scope) {
  return page.evaluate((a) => window.__boxes(a), {
    mark: o.mark, line: o.line, markAll: o.markAll, cells: o.cells, select: o.select,
    cols: size?.cols ?? 1, rows: size?.rows ?? 1, accent: ACCENT, scope,
  })
}

function report(res, o) {
  if (res.missing.length) {
    console.error(`shot.mjs: not on the screen: ${res.missing.map((m) => JSON.stringify(m)).join(', ')}`)
    console.error('--- what the screen shows ---\n' + res.visible.slice(0, 4000))
    process.exit(1)
  }
  console.log(`${path.relative(process.cwd(), o.out)}  (${res.boxed} box${res.boxed === 1 ? '' : 'es'})`)
}

async function tui(o) {
  const cast = readFileSync(path.resolve(o.target), 'utf8')
  const size = castSize(cast)
  const html = tuiPage(o, cast, size)
  const browser = await chromium.launch()
  try {
    // 먼저 한 번 그려 창틀의 크기를 잰다. 영상은 그 크기의 화면으로 찍어야 가장자리가 안 남는다
    const probe = await browser.newPage({ deviceScaleFactor: o.video ? 1 : 2 })
    await probe.setContent(html)
    await probe.waitForSelector('#cast .ap-line', { timeout: 10000 })
    await probe.waitForTimeout(400)
    await probe.evaluate((n) => window.__lines(n), o.lines)
    const shot = await probe.$('#shot')
    if (!o.video) {
      const res = await boxes(probe, o, size, '#cast')
      if (!res.missing.length) await shot.screenshot({ path: o.out })
      return report(res, o)
    }
    const box = await shot.boundingBox()
    await probe.close()
    const dir = mkdtemp()
    const vsize = { width: Math.ceil(box.width / 2) * 2, height: Math.ceil(box.height / 2) * 2 }
    const ctx = await browser.newContext({ viewport: vsize, recordVideo: { dir, size: vsize }, colorScheme: 'dark' })
    const page = await ctx.newPage()
    await page.setContent(html)
    await page.waitForSelector('#cast .ap-line', { timeout: 10000 })
    await page.evaluate((n) => window.__lines(n), o.lines)
    // 상자는 재생 내내 다시 친다 — 글이 화면에 나오면 그 자리에, 밀려 올라가면 따라서. 한 번이라도
    // 나온 글은 찾은 것으로 센다. 끝까지 한 번도 안 나온 글이 있으면 그 영상은 버린다
    const until = Date.now() + ((o.to - o.from) / o.speed) * 1000 + 600
    const seen = new Set()
    let res = { boxed: 0, missing: [], visible: '' }
    while (Date.now() < until) {
      const got = await boxes(page, o, size, '#cast')
      for (const m of [...o.mark, ...o.line]) if (!got.missing.includes(m)) seen.add(m)
      res = { boxed: Math.max(res.boxed, got.boxed), missing: [], visible: got.visible }
      await page.waitForTimeout(120)
    }
    res.missing = [...o.mark, ...o.line].filter((m) => !seen.has(m))
    const video = page.video()
    await ctx.close()
    if (!res.missing.length) renameSync(await video.path(), o.out)
    rmSync(dir, { recursive: true, force: true })
    return report(res, o)
  } finally {
    await browser.close()
  }
}

async function web(o) {
  const [width, height] = o.size.split('x').map(Number)
  const browser = await chromium.launch()
  try {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 2, colorScheme: 'dark' })
    await page.goto(o.target, { waitUntil: 'networkidle' })
    await page.waitForTimeout(o.wait)
    await page.addScriptTag({ content: IN_PAGE })
    const res = await boxes(page, o, null, null)
    if (!res.missing.length) await page.screenshot({ path: o.out })
    return report(res, o)
  } finally {
    await browser.close()
  }
}

function mkdtemp() {
  const dir = path.join(os.tmpdir(), `moai-shot-${process.pid}-${Date.now()}`)
  mkdirSync(dir, { recursive: true })
  return dir
}

const o = parse(process.argv.slice(2))
mkdirSync(path.dirname(o.out), { recursive: true })
await (o.mode === 'tui' ? tui(o) : web(o))
