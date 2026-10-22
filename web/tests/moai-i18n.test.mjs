// The page's words are moai's own i18n/*.json under `page.`: the lookup (panel/i18n/index.ts) nests them and falls
// back to English key by key. cargo's i18n tests keep the places and ko whole in those same files; these cover the
// page's side — every key lands in the tree, the shape is the same in each language, and the page speaks en and ko.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'

const { COPY, copyFor, fill, speaks } = await panelModule('i18n/index.ts')
const bundle = async (code) => JSON.parse(await readFile(new URL(`../../i18n/${code}.json`, import.meta.url), 'utf8'))

/** [path, text] for every string in a copy tree; array items are `.0`, `.1` as in the JSON keys. */
function leaves(node, path = []) {
  if (typeof node === 'string') return [[path.join('.'), node]]
  return Object.entries(node).flatMap(([key, value]) => leaves(value, [...path, key]))
}

const places = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')

/**
 * The keys the page reads, from the `Copy` interface in panel/i18n/index.ts (the types are erased at run time and TypeScript
 * 7 has no script API, so the interfaces are read as text). A string member is a leaf the page reads and en.json must hold;
 * a `Record<…>`, an array or a `{…}` of unknown size is a path en.json must hold something under. The `?` members are
 * left out: the page does not require them.
 */
async function pageReads() {
  const source = await readFile(new URL('../panel/i18n/index.ts', import.meta.url), 'utf8')
  const bodies = new Map([...source.matchAll(/(?:export )?interface (\w+) \{\n([\s\S]*?)\n\}/g)].map((m) => [m[1], m[2]]))
  const leaves = []
  const unknown = []
  // A member ends at a `;` or a line end that is not inside a `{…}`, `<…>` or `[…]`.
  const members = (text) => {
    const out = []
    let depth = 0
    let line = ''
    for (const ch of text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '')) {
      if ('{<['.includes(ch)) depth++
      if ('}>]'.includes(ch)) depth--
      if (depth === 0 && (ch === ';' || ch === '\n')) {
        if (line.trim()) out.push(line.trim())
        line = ''
      } else line += ch
    }
    if (line.trim()) out.push(line.trim())
    return out
  }
  const walk = (path, type) => {
    type = type.trim()
    if (type === 'string') return leaves.push(path)
    if (bodies.has(type)) return members(bodies.get(type)).forEach((line) => member(path, line))
    const inline = /^\{([\s\S]*)\}$/.exec(type)
    if (inline && !type.includes('[')) return members(inline[1]).forEach((line) => member(path, line))
    unknown.push(path)
  }
  const member = (parent, line) => {
    const m = /^(\w+)(\?)?:\s*([\s\S]+)$/.exec(line)
    assert.ok(m, `cannot read the Copy member: ${line}`)
    if (m[2]) return
    walk(parent ? `${parent}.${m[1]}` : m[1], m[3])
  }
  members(bodies.get('Copy')).forEach((line) => member('', line))
  return { leaves, unknown }
}

test('every key the page reads (the Copy interface) is in en.json, and ko holds them too', async () => {
  const { leaves: read, unknown } = await pageReads()
  assert.ok(read.length > 150, `${read.length} keys read`)
  assert.ok(unknown.length < 40, `${unknown.length} sized-by-data members`)
  const en = new Set(Object.keys(await bundle('en')).filter((key) => key.startsWith('page.')).map((key) => key.slice(5)))
  const ko = new Set(Object.keys(await bundle('ko')).filter((key) => key.startsWith('page.')).map((key) => key.slice(5)))
  assert.deepEqual(read.filter((key) => !en.has(key)), [], 'read by the page, missing in en.json')
  assert.deepEqual(read.filter((key) => !ko.has(key)), [], 'read by the page, missing in ko.json')
  const under = (keys, path) => [...keys].some((key) => key.startsWith(`${path}.`))
  assert.deepEqual(unknown.filter((path) => !under(en, path)), [], 'a list or table the page reads, empty in en.json')
  // And the tree is whole at each of them: copyFor('en') reads no undefined.
  const get = (path) => path.split('.').reduce((node, part) => node?.[part], COPY.en)
  assert.deepEqual(read.filter((path) => typeof get(path) !== 'string'), [])
})

// The tree is built from en.json, so this cannot tell a key the page never reads, nor a word the page reads that en.json
// lacks (that one reads `undefined`): it holds the nesting (`.0` arrays, `table.labels` dotted names) to one leaf per key.
test('every page key in en.json lands as one leaf of the copy tree, and the tree invents none', async () => {
  const keys = Object.keys(await bundle('en')).filter((key) => key.startsWith('page.')).map((key) => key.slice(5)).sort()
  assert.ok(keys.length > 300)
  assert.deepEqual(leaves(COPY.en).map(([path]) => path).sort(), keys)
})

test('each language has the same shape as English, in its own words where the file has them', async () => {
  const en = leaves(COPY.en)
  for (const code of ['ko', 'ja', 'zh', 'es']) {
    const own = new Set(Object.keys(await bundle(code)).filter((key) => key.startsWith('page.')).map((key) => key.slice(5)))
    const there = new Map(leaves(COPY[code]))
    assert.equal(there.size, en.length, code)
    for (const [path, english] of en) {
      assert.ok(there.has(path), `${code}: ${path}`)
      if (!own.has(path)) assert.equal(there.get(path), english, `${code}: ${path} falls back to English`)
      assert.equal(places(there.get(path)), places(english), `${code}: ${path} keeps the places`)
    }
  }
})

test('Korean is whole: every page key is in ko.json', async () => {
  const en = Object.keys(await bundle('en')).filter((key) => key.startsWith('page.'))
  const ko = await bundle('ko')
  assert.deepEqual(en.filter((key) => !(key in ko)), [])
})

test('the page carries en, ko, ja and zh; es draws in English and is named so', () => {
  for (const tag of ['en', 'ko', 'ko-KR', 'ja', 'zh-CN', undefined]) assert.equal(speaks(tag), true, String(tag))
  assert.equal(speaks('es'), false)
  assert.equal(speaks('fr'), false)
  assert.equal(copyFor('es'), COPY.en)
  assert.equal(copyFor('fr'), COPY.en)
  assert.equal(copyFor('ko-KR'), COPY.ko)
  assert.equal(copyFor('ko').shell.tabs.board, '칸반')
  assert.equal(copyFor('en').shell.tabs.board, 'Board')
})

test('lists come back as arrays and fill puts values into their places', () => {
  assert.ok(Array.isArray(COPY.ko.shell.tips.groups))
  assert.ok(Array.isArray(COPY.ko.shell.tips.groups[0].tips))
  assert.equal(fill('{n} hidden', { n: 3 }), '3 hidden')
  assert.equal(fill('{a}/{b} {a}', { a: 1, b: 'x' }), '1/x 1')
  assert.equal(fill('{kept} {n}', { n: 0 }), '{kept} 0')
  assert.equal(fill(COPY.en.shell.hidden, { names: 'a · b' }), 'a · b hidden')
  assert.equal(fill(COPY.ko.refreshedAt, { time: '12:00' }), '12:00 갱신')
})
