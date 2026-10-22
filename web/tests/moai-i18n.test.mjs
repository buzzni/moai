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
