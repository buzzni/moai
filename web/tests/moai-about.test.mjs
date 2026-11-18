// The page's header (se-qkga, from Saycode's panel 2.1.0): the TUI's logo, then `User` and `Version` from `/api/boot`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

import { panelModule } from './moai-panel-modules.mjs'

const A = await panelModule('data/about.ts')
const B = await panelModule('view/brand.ts')
const { copyFor } = await panelModule('i18n/index.ts')
const { bootFrom } = await panelModule('data/source.ts')

const en = copyFor('en').shell.version
const ko = copyFor('ko').shell.version

test('aboutFrom reads the user line and the version with what moai last heard of the latest from the boot; an older server gives nulls', () => {
  const base = {
    lang: 'en', version: '0.18.0', project: 'p', root: '/r', statuses: ['todo', 'done'], naming: 'full', glyphs: {}, unknown_glyph: '○',
    columns: { backlog: { name: 'Backlog', glyph: '◇' }, shelved: { name: 'Deferred', glyph: '‖' } }, filter_keys: [], sort_fields: [],
  }
  const boot = bootFrom({ ...base, user: 'Raven (raven@buzzni.com)', latest: { state: 'stale', tag: 'v0.18.0', heard_at: '2026-10-12T00:00:00Z', why: 'offline' } })
  assert.deepEqual(A.aboutFrom(boot), { user: 'Raven (raven@buzzni.com)', version: '0.18.0', latest: { state: 'stale', tag: 'v0.18.0', heardAt: '2026-10-12T00:00:00Z', why: 'offline' } })
  assert.deepEqual(A.aboutFrom(bootFrom(base)), { user: null, version: '0.18.0', latest: null }, 'a server before user and latest')
  assert.deepEqual(A.aboutFrom(bootFrom({ ...base, latest: { state: 'weird' } })).latest, null)
  assert.deepEqual(A.aboutFrom(null), { user: null, version: null, latest: null }, 'before the boot answers')
})

test('versionLine writes what the TUI writes, in moai\'s own words', () => {
  const now = Date.parse('2026-10-15T12:00:00Z')
  const about = (latest) => ({ user: null, version: '0.18.0', latest })
  assert.deepEqual(A.versionLine(about({ state: 'same', tag: 'v0.18.0' }), en, now), { said: '0.18.0 · latest (v0.18.0)' })
  assert.deepEqual(A.versionLine(about({ state: 'same', tag: 'v0.18.0' }), ko, now), { said: '0.18.0 · 최신 (v0.18.0)' })
  assert.deepEqual(A.versionLine(about({ state: 'newer', tag: 'v0.19.0' }), en, now), { said: '0.18.0 · v0.19.0 is out' })
  assert.deepEqual(A.versionLine(about({ state: 'ahead', tag: 'v0.17.0' }), ko, now), { said: '0.18.0 · 릴리스보다 앞선 판 (v0.17.0)' })
  assert.deepEqual(A.versionLine(about({ state: 'unasked', why: 'offline' }), en, now), { said: '0.18.0 · latest not checked (no network)' })
  assert.deepEqual(A.versionLine(about({ state: 'unasked', why: 'something-new' }), en, now), { said: '0.18.0 · latest not checked' })
  // A cached answer it could not refresh: when it was heard goes after it, as the TUI's tail.
  assert.deepEqual(A.versionLine(about({ state: 'stale', tag: 'v0.18.0', heardAt: '2026-10-12T09:00:00Z', why: 'timeout' }), ko, now), { said: '0.18.0 · 최신 확인 안 함 (타임아웃)', tail: '3일 전에 본 판 v0.18.0' })
  assert.equal(A.versionLine(about({ state: 'stale', tag: 'v0.18.0', heardAt: '2026-10-15T01:00:00Z' }), en, now).tail, 'v0.18.0 seen today')
  assert.equal(A.versionLine(about({ state: 'stale', tag: 'v0.18.0', heardAt: '2099-01-01T00:00:00Z' }), en, now).tail, 'v0.18.0 seen earlier', 'a time ahead is not today')
  // The TUI believes a heard-at up to a day ahead of the clock (`FUTURE_SLACK_SECS`); one second past that it does not.
  const at = (offsetMs) => new Date(now + offsetMs).toISOString()
  assert.equal(A.versionLine(about({ state: 'stale', tag: 'v0.18.0', heardAt: at(A.FUTURE_SLACK_MS - 1000) }), en, now).tail, 'v0.18.0 seen today', 'a clock a few hours off is still today')
  assert.equal(A.versionLine(about({ state: 'stale', tag: 'v0.18.0', heardAt: at(A.FUTURE_SLACK_MS + 1000) }), en, now).tail, 'v0.18.0 seen earlier', 'past a day it is not believed')
  assert.deepEqual(A.versionLine(about(null), en, now), { said: '0.18.0' })
  assert.deepEqual(A.versionLine({ user: null, version: null, latest: null }, en, now), { said: '—' })
  assert.deepEqual(A.versionLine(null, en, now), { said: '—' })
  // Japanese and Chinese read moai's English, as the TUI does.
  assert.deepEqual(copyFor('ja').shell.version, en)
})

test('the logo is the TUI\'s, the art centred as one block over the tagline; the info lines keep the TUI\'s columns', () => {
  const lines = B.logoLines()
  assert.equal(lines.length, 6)
  assert.equal(lines[5], 'Issue tracker for AI agent')
  assert.ok(lines.every((line) => [...line].length === 26), 'one width, so the pipe stands straight')
  assert.equal(lines[2], ' ▐█ ▌▐▌▐█·▐█▌.▐▌▄█▀▀█ ▐█· ', 'the widest art row moves in by the same one cell as the others')
  assert.deepEqual(B.infoLines(null, en), [['User', '—'], ['Version', '—']])
  const about = { user: 'Raven <raven@buzzni.com>', version: '0.18.0', latest: { state: 'stale', tag: 'v0.18.0', heardAt: '2026-10-14T00:00:00Z' } }
  assert.deepEqual(B.infoLines(about, en, Date.parse('2026-10-15T12:00:00Z')), [['User', 'Raven <raven@buzzni.com>'], ['Version', '0.18.0 · latest not checked'], ['', 'v0.18.0 seen 1 day ago']])
  const doc = new JSDOM('<!doctype html><body></body>').window.document
  const brand = B.mountBrand(doc, en)
  brand.render(about)
  assert.deepEqual([...brand.el.querySelectorAll('.m-brand-line')].map((line) => line.textContent), [
    'User    : Raven <raven@buzzni.com>',
    'Version : 0.18.0 · latest not checked',
    `          ${B.infoLines(about, en)[2][1]}`,
  ])
  assert.equal(brand.el.querySelector('.m-brand-logo').getAttribute('aria-hidden'), 'true')
})

test('the art is drawn as terminal cells, every letter of it: no row leaves a seam a font would', () => {
  const art = B.logoLines().slice(0, -1)
  const letters = new Set(art.join('').replace(/ /g, ''))
  assert.deepEqual([...letters].filter((letter) => !B.ART_LETTERS.includes(letter)), [], 'a letter artPath does not draw')
  // A full block fills its cell, one wide and two tall; a half fills its half; rows meet at even y.
  assert.equal(B.artPath(['█']), 'M0 0h1v2h-1z')
  assert.equal(B.artPath([' ', '▀▄']), 'M0 2h1v1h-1zM1 3h1v1h-1z')
  assert.equal(B.artPath(['▌▐']), 'M0 0h0.5v2h-0.5zM1.5 0h0.5v2h-0.5z')
  const doc = new JSDOM('<!doctype html><body></body>').window.document
  const svg = B.mountBrand(doc, en).el.querySelector('svg.m-brand-art')
  assert.equal(svg.getAttribute('viewBox'), '0 0 26 10', 'the tagline\'s 26 cells, five rows of two')
  assert.equal(svg.querySelectorAll('path').length, 1, 'one path: neighbouring blocks are filled together')
})

// ── tips: the line under Version and the page with all of them (se-irhr) ──
const T = await panelModule('view/tips.ts')

test('tips: the field test\'s sentences are all there, by kind, in Korean; English stands in for Japanese and Chinese', () => {
  const ko = copyFor('ko').shell.tips
  const given = [
    'v1.22.x의 마일스톤을 넣고 1.22에서 리뷰에서 발견된 버그 백로그를 마일스톤에 포함시켜줘.',
    '유저관리설정에서 유저 검색 창을 만들어야 한다. `opus5.x xhigh` 로 계획을 세워줘',
    '백로그를 유사한 것들 끼리는 함께 작업할 수 있게 합쳐주고, 필요없다 판단되는 것은 삭제해줘.',
    'se-43oh 시작해줘 (또는 43oh 시작해줘)',
    '마일스톤 v0.1.2 를 시작하고 리뷰때 버그가 아닌 백로그는 만들지말 것',
    '/moai:moai-wiki 또는 모아이 위키 스킬을 사용하여 프로그램 전체의 위키 문서를 작성하고 항상 위키를 업데이트할 것',
    '이 세션은 moai 감독 세션이다. 세이코드 세션 3개를 만들고 서브에이전트로 작업을 시작할 것.',
    '작업은 sonnet5.x medium 으로 진행하고 리뷰를 opus5.5 high 또는 난이도에 따라 xhigh 로 할 것',
    '작업중에 서버가 죽었다. 작업목록을 확인하여 이전 세션을 복구하고 작업을 계속 진행해줘.',
  ]
  const all = T.allTips(ko)
  for (const tip of given) assert.ok(all.includes(tip), tip)
  assert.ok(ko.groups.length >= 5 && ko.groups.every((group) => group.title && group.tips.length > 0))
  const en = copyFor('en').shell.tips
  assert.equal(en.groups.length, ko.groups.length)
  assert.deepEqual(en.groups.map((group) => group.tips.length), ko.groups.map((group) => group.tips.length), 'the same sentences, said in English')
  assert.deepEqual(copyFor('ja').shell.tips, en)
  assert.deepEqual(copyFor('zh').shell.tips, en)
})

test('the header has no tip line; the projects stand right after the info column, which takes only its own width', async () => {
  const doc = new JSDOM('<!doctype html><body></body>').window.document
  const brand = B.mountBrand(doc, en)
  brand.render({ user: 'Raven <raven@buzzni.com>', version: '0.18.0', latest: null }, [{ name: 'a', path: '/a', number: 1, home: false, current: true }])
  assert.equal(brand.el.querySelector('.m-brand-tip'), null)
  assert.equal(brand.el.querySelector('.m-brand-tip-text'), null)
  assert.equal(doc.querySelector('style[id^="m-tip-"]'), null)
  assert.deepEqual([...brand.el.children].map((node) => node.className), ['m-brand-logo', 'm-brand-info', 'm-brand-projects'], 'projects right after info')
  const { readFileSync } = await import('node:fs')
  const css = readFileSync(new URL('../panel/style/shell.css', import.meta.url), 'utf8')
  const rule = (selector) => css.match(new RegExp(`^${selector.replace('.', '\\.')} \\{([^}]*)\\}`, 'm'))[1]
  assert.doesNotMatch(rule('.m-brand-info'), /flex:\s*1/, 'the info column must not take the spare width, or the projects go to the far edge')
  assert.match(rule('.m-brand-info'), /flex:\s*0 1 auto/)
  assert.match(rule('.m-brand-projects'), /padding-left:\s*2ch/, 'a gap between the columns')
  assert.doesNotMatch(css, /m-brand-tip/, 'no leftover tip rules')
})

test('tips page: every tip with its Copy button; keys stay in it; Esc closes and gives the focus back', () => {
  const dom = new JSDOM('<!doctype html><body><button id="before">x</button></body>')
  const doc = dom.window.document
  const words = copyFor('ko').shell.tips
  const page = T.createTips(doc, words, 'mac')
  doc.body.append(page.el)
  const behind = []
  doc.addEventListener('keydown', (event) => behind.push(event.key))
  doc.getElementById('before').focus()
  page.open()
  assert.equal(page.isOpen(), true)
  assert.equal(page.el.querySelectorAll('.m-tips-row').length, T.allTips(words).length)
  assert.equal(page.el.querySelectorAll('.m-help-section').length, words.groups.length)
  const key = (target, k) => target.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }))
  key(page.el, 'j')
  key(doc.body, ' ')
  assert.deepEqual(behind, [], 'nothing behind the page takes a key')
  key(page.el, 'Escape')
  assert.equal(page.isOpen(), false)
  assert.equal(doc.activeElement, doc.getElementById('before'))
})

test('tips page: off macOS, a Ctrl key with the focus behind reaches neither the panel\'s keys nor a control behind', () => {
  const dom = new JSDOM('<!doctype html><body><input id="behind"><button id="press">x</button></body>')
  const doc = dom.window.document
  const page = T.createTips(doc, copyFor('en').shell.tips, 'other')
  doc.body.append(page.el)
  const router = []
  doc.addEventListener('keydown', (event) => router.push(event.key))
  page.open()
  const key = (target, k) => {
    const event = new dom.window.KeyboardEvent('keydown', { key: k, ctrlKey: true, bubbles: true, cancelable: true })
    target.dispatchEvent(event)
    return event
  }
  // `C-d` is the host's own (a bookmark) and goes on, but the key router behind never hears it.
  assert.equal(key(doc.getElementById('behind'), 'd').defaultPrevented, false)
  assert.deepEqual(router, [], 'the panel ran C-d under the open page')
  // A paste into an input behind and Ctrl+Enter on a button behind are stopped, and the focus comes back.
  assert.equal(key(doc.getElementById('behind'), 'v').defaultPrevented, true)
  assert.equal(key(doc.getElementById('press'), 'Enter').defaultPrevented, true)
  assert.equal(doc.activeElement, page.el)
})
