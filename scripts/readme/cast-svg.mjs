// asciinema 녹화(v3)를 GitHub README 가 그대로 트는 애니메이션 SVG 로 굽는다 (moai-yvyu).
//
//     node scripts/readme/cast-svg.mjs site/assets/media/tui-tour.cast docs/readme/tui-tour.svg --title 'moai tui'
//
// GitHub 는 README 에서 스크립트를 안 돌리고 <img> 안의 SVG 는 CSS 애니메이션만 튼다. 그래서 장면마다 화면을
// 떠서 세로로 쌓고, 한 장씩 보이도록 translate 를 계단으로 옮긴다. 같은 줄은 <defs> 에 한 번만 두고 <use> 로 부른다
// — TUI 는 대부분의 줄이 장면 사이에 안 바뀌어, 이것 하나로 크기가 열에 하나쯤으로 준다.
//
// 에뮬레이터는 홈페이지 녹화(scripts/site-demo)가 실제로 내는 것만 안다: 커서 옮김(CUP), SGR 의 256색·굵게·흐리게·
// 반전, CR/LF. 다른 CSI 는 버린다. 의존성은 없다 — svg-term 같은 외부 패키지를 받아 돌리지 않으려고 손으로 짰다.
//
//   --title <글>     창틀 제목줄
//   --speed N        재생 속도 (기본 1)
//   --idle S         멈춘 구간을 S 초로 줄인다 (기본 2, 홈페이지의 idleTimeLimit 과 같다)
//   --min S          이보다 짧게 지나가는 장면은 버린다 (기본 0.08)
//   --from S --to S  그 구간만 굽는다
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import path from 'node:path'

const [src, dst, ...rest] = process.argv.slice(2)
if (!src || !dst) {
  console.error('usage: cast-svg.mjs <cast> <out.svg> [--title t] [--speed n] [--idle s] [--min s] [--from s] [--to s]')
  process.exit(2)
}
const opt = { title: '', speed: 1, idle: 2, min: 0.08, from: 0, to: Infinity }
for (let i = 0; i < rest.length; i += 2) {
  const k = rest[i].replace(/^--/, '')
  if (!(k in opt)) throw new Error(`unknown flag ${rest[i]}`)
  opt[k] = k === 'title' ? rest[i + 1] : Number(rest[i + 1])
}

// 홈페이지의 asciinema-player-theme-moai 와 같은 16색 (site/assets/site.css)
const FG = '#d9d6cf'
const BG = '#121212'
const BASE = ['#1b1e1b', '#e5534b', '#57ab5a', '#e2b857', '#539bf5', '#b083f0', '#39c5cf', '#cdd1cc',
  '#6b6962', '#f47067', '#6bc46d', '#fbc04e', '#6cb6ff', '#dcbdfb', '#56d4dd', '#ece9e2']
function color(n) {
  if (n < 16) return BASE[n]
  if (n < 232) {
    const v = [0, 95, 135, 175, 215, 255]
    const c = n - 16
    return '#' + [v[Math.floor(c / 36)], v[Math.floor(c / 6) % 6], v[c % 6]].map(x => x.toString(16).padStart(2, '0')).join('')
  }
  const g = (8 + (n - 232) * 10).toString(16).padStart(2, '0')
  return `#${g}${g}${g}`
}

// 한글·한자·가나와 전각 기호는 두 칸이다
function wide(cp) {
  return (cp >= 0x1100 && cp <= 0x115f) || (cp >= 0x2e80 && cp <= 0xa4cf) || (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) || (cp >= 0xfe30 && cp <= 0xfe4f) || (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) || (cp >= 0x1f300 && cp <= 0x1faff)
}

const lines = readFileSync(src, 'utf8').split('\n').filter(Boolean)
const head = JSON.parse(lines[0])
const cols = head.term?.cols ?? head.width
const rows = head.term?.rows ?? head.height
const v3 = head.version === 3

const blank = () => ({ ch: ' ', fg: null, bg: null, b: 0, d: 0, r: 0 })
let grid = Array.from({ length: rows }, () => Array.from({ length: cols }, blank))
let cx = 0, cy = 0
let pen = { fg: null, bg: null, b: 0, d: 0, r: 0 }

function sgr(params) {
  const p = params.length ? params.split(';').map(Number) : [0]
  for (let i = 0; i < p.length; i++) {
    const n = p[i]
    if (n === 0) pen = { fg: null, bg: null, b: 0, d: 0, r: 0 }
    else if (n === 1) pen.b = 1
    else if (n === 2) pen.d = 1
    else if (n === 22) { pen.b = 0; pen.d = 0 }
    else if (n === 7) pen.r = 1
    else if (n === 27) pen.r = 0
    else if (n === 39) pen.fg = null
    else if (n === 49) pen.bg = null
    else if (n >= 30 && n <= 37) pen.fg = n - 30
    else if (n >= 90 && n <= 97) pen.fg = n - 82
    else if (n >= 40 && n <= 47) pen.bg = n - 40
    else if (n >= 100 && n <= 107) pen.bg = n - 92
    else if ((n === 38 || n === 48) && p[i + 1] === 5) { pen[n === 38 ? 'fg' : 'bg'] = p[i + 2]; i += 2 }
  }
}

function put(ch) {
  const w = wide(ch.codePointAt(0)) ? 2 : 1
  if (cx + w > cols) { cx = 0; cy = Math.min(cy + 1, rows - 1) }
  grid[cy][cx] = { ch, ...pen }
  if (w === 2 && cx + 1 < cols) grid[cy][cx + 1] = { ch: '', ...pen }
  cx += w
}

let carry = ''
function feed(data) {
  const s = carry + data
  carry = ''
  for (let i = 0; i < s.length;) {
    const c = s[i]
    if (c === '\x1b') {
      if (s[i + 1] !== '[') { i += 2; continue }
      let j = i + 2
      while (j < s.length && !/[A-Za-z@`~]/.test(s[j])) j++
      if (j >= s.length) { carry = s.slice(i); return }
      const params = s.slice(i + 2, j)
      const f = s[j]
      if (f === 'm') sgr(params)
      else if (f === 'H' || f === 'f') {
        const [r = 1, k = 1] = params.split(';').map(x => Number(x) || 1)
        cy = Math.min(r - 1, rows - 1); cx = Math.min(k - 1, cols - 1)
      } else if (f === 'J' && params === '2') grid = Array.from({ length: rows }, () => Array.from({ length: cols }, blank))
      else if (f === 'K') for (let k = cx; k < cols; k++) grid[cy][k] = blank()
      i = j + 1
    } else if (c === '\r') { cx = 0; i++ }
    else if (c === '\n') { cy = Math.min(cy + 1, rows - 1); i++ }
    else if (c < ' ') i++
    else {
      const cp = s.codePointAt(i)
      const ch = String.fromCodePoint(cp)
      put(ch)
      i += ch.length
    }
  }
}

const CW = 8.4, LH = 17, PAD = 14, BAR = opt.title ? 34 : 0
const esc = t => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

// 한 줄을 SVG 조각으로. 같은 꼴이 이어지는 칸은 한 덩이로 묶는다
function rowSvg(row) {
  let rects = '', texts = ''
  let k = 0
  while (k < cols) {
    const a = row[k]
    const fg0 = a.fg == null ? FG : color(a.fg)
    const bg0 = a.bg == null ? null : color(a.bg)
    const fg = a.r ? (bg0 ?? BG) : fg0
    const bg = a.r ? fg0 : bg0
    let e = k + 1
    while (e < cols) {
      const z = row[e]
      if (z.fg !== a.fg || z.bg !== a.bg || z.b !== a.b || z.d !== a.d || z.r !== a.r) break
      e++
    }
    if (bg) rects += `<rect x="${+(k * CW).toFixed(1)}" width="${+((e - k) * CW).toFixed(1)}" height="${LH}" fill="${bg}"/>`
    // 덩이 안에서도 글자마다 x 를 박지 않으면 글꼴이 고정폭이 아닐 때 줄이 밀린다. 빈칸 덩이는 건너뛰고 쓴다
    let t = '', start = -1
    const flush = end => {
      if (start < 0) return
      const cls = [a.b ? 'b' : '', a.d ? 'd' : ''].filter(Boolean).join(' ')
      // 보는 쪽 글꼴의 폭이 CW 와 달라도(Windows 의 Consolas 는 7.7) 덩이가 제 칸을 채우게 길이를 못박는다
      const len = end - start
      const fit = len > 1 ? ` textLength="${+(len * CW).toFixed(1)}" lengthAdjust="spacingAndGlyphs"` : ''
      texts += `<text x="${+(start * CW).toFixed(1)}"${fit}${fg !== FG ? ` fill="${fg}"` : ''}${cls ? ` class="${cls}"` : ''}>${esc(t)}</text>`
      t = ''; start = -1
    }
    for (let q = k; q < e; q++) {
      const ch = row[q].ch
      if (ch === '') continue
      if (ch === ' ' && start >= 0 && row[q + 1]?.ch === ' ') { flush(q); continue }
      if (ch === ' ' && start < 0) continue
      if (start < 0) start = q
      t += ch
      // 두 칸 글자는 다음 칸(빈 자리표)까지 제 몫이다 — 빼면 textLength 가 한 칸 모자라 줄이 밀린다
      if (/[^\x20-\x7e]/.test(ch) && !/[─│┌┐└┘├┤┬┴┼▌▐▄▀█░▒▓·•]/.test(ch)) { flush(q + (row[q + 1]?.ch === '' ? 2 : 1)) }
    }
    flush(e)
    k = e
  }
  return rects + texts
}

// 장면을 뜬다: 출력이 잠깐이라도 쉬는 자리에서 한 장
const frames = []
let t = 0
const events = lines.slice(1).map(l => JSON.parse(l))
let abs = 0
for (let n = 0; n < events.length; n++) {
  const [d, kind, data] = events[n]
  abs = v3 ? abs + d : d
  if (kind !== 'o') continue
  feed(data)
  const next = events[n + 1]
  const gap = next ? (v3 ? next[0] : next[0] - abs) : 1
  if (gap < 0.03 && next) continue
  if (abs < opt.from || abs > opt.to) continue
  frames.push({ at: abs, rows: grid.map(rowSvg) })
}
if (!frames.length) throw new Error('no frames in that range')

// 시간을 다시 잰다: 속도·멈춤 줄이기·너무 짧은 장면 버리기·같은 장면 합치기
const shown = []
for (let i = 0; i < frames.length; i++) {
  const raw = (i + 1 < frames.length ? frames[i + 1].at : frames[i].at + 2) - frames[i].at
  const dur = Math.min(raw, opt.idle) / opt.speed
  const prev = shown[shown.length - 1]
  if (prev && prev.rows.join('\n') === frames[i].rows.join('\n')) { prev.dur += dur; continue }
  if (dur < opt.min && i + 1 < frames.length) continue
  shown.push({ rows: frames[i].rows, dur })
}
shown[shown.length - 1].dur = Math.max(shown[shown.length - 1].dur, 3)
const total = shown.reduce((a, f) => a + f.dur, 0)

const ids = new Map()
let defs = ''
for (const f of shown) for (const r of f.rows) {
  if (!r || ids.has(r)) continue
  const id = 'r' + ids.size.toString(36)
  ids.set(r, id)
  defs += `<g id="${id}">${r}</g>`
}

const W = cols * CW, H = rows * LH
let body = '', keys = '', acc = 0
shown.forEach((f, i) => {
  body += `<g transform="translate(0 ${i * H})">`
  f.rows.forEach((r, y) => { if (r) body += `<use href="#${ids.get(r)}" y="${y * LH}"/>` })
  body += '</g>'
  keys += `${+(acc / total * 100).toFixed(3)}%{transform:translateY(${-i * H}px)}`
  acc += f.dur
})

const outW = W + PAD * 2, outH = H + PAD * 2 + BAR
const bar = opt.title
  ? `<rect width="${outW}" height="${BAR}" rx="10" fill="#151715"/><rect y="${BAR - 10}" width="${outW}" height="10" fill="#151715"/>` +
    `<circle cx="20" cy="17" r="5.5" fill="#2c302c"/><circle cx="38" cy="17" r="5.5" fill="#2c302c"/><circle cx="56" cy="17" r="5.5" fill="#2c302c"/>` +
    `<text x="76" y="21" fill="#6b6962" font-size="12">${esc(opt.title)}</text>`
  : ''
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${outW}" height="${outH}" viewBox="0 0 ${outW} ${outH}" font-family="ui-monospace,SFMono-Regular,Menlo,Consolas,'DejaVu Sans Mono','Liberation Mono',monospace" font-size="14">` +
  `<style>text{white-space:pre;dominant-baseline:text-before-edge}.b{font-weight:700}.d{opacity:.6}` +
  `#film{animation:play ${total.toFixed(2)}s steps(1,end) infinite}@keyframes play{${keys}}` +
  `@media (prefers-reduced-motion:reduce){#film{animation:none}}</style>` +
  `<rect width="${outW}" height="${outH}" rx="10" fill="${BG}"/>${bar}` +
  `<defs>${defs}</defs>` +
  `<svg x="${PAD}" y="${PAD + BAR}" width="${W}" height="${H}" fill="${FG}"><g id="film">${body}</g></svg></svg>\n`

mkdirSync(path.dirname(dst), { recursive: true })
writeFileSync(dst, svg)
console.log(`${dst}: ${shown.length} frames, ${total.toFixed(1)}s, ${(svg.length / 1024).toFixed(0)}KB`)
