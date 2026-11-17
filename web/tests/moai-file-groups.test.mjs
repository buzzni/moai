import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync } from 'node:fs'
import { panelModule } from './moai-panel-modules.mjs'
const { fileGroup, fileGroupTable, extOf } = await panelModule('data/file-groups.ts')

const read = path => readFileSync(new URL(`../panel/${path}`, import.meta.url), 'utf8')
// The lists of src/style.rs `file_style`, as /api/boot will send them.
const boot = {
  source: { ext: 'rs c h cc cpp hpp go py js mjs cjs ts tsx jsx java kt swift rb php cs scala lua zig hs ml ex exs erl clj dart vue svelte css scss less sql'.split(' '), names: [] },
  doc: { ext: 'md markdown txt rst adoc html htm xml tex org'.split(' '), names: [] },
  data: { ext: 'toml json jsonl yaml yml ini cfg conf csv tsv env'.split(' '), names: [] },
  script: { ext: 'sh bash zsh fish ps1 bat cmd'.split(' '), names: ['Makefile', 'Dockerfile', 'justfile', 'Justfile'] },
  binary: { ext: 'png jpg jpeg gif webp ico bmp svg pdf zip gz tar tgz xz bz2 7z so dylib dll exe bin o a wasm woff woff2 ttf otf mp4 mov mp3 wav lock'.split(' '), names: [] },
}
const table = fileGroupTable(boot)

test('names fall into the six groups the way style::file_style does', () => {
  const want = { 'a.rs': 'source', 'a.tsx': 'source', 'a.md': 'doc', 'Cargo.toml': 'data', 'Cargo.lock': 'binary', Makefile: 'script', 'run.sh': 'script', 'X.SVG': 'binary', '.env': 'plain', README: 'plain', 'src/a.rs': 'source', 'a.tar.gz': 'binary', 'a.': 'plain', Dockerfile: 'script' }
  for (const [name, group] of Object.entries(want)) assert.equal(fileGroup(name, table), group, name)
})
test('the extension is read as Path::extension reads it', () => {
  assert.deepEqual(['.env', 'a.', 'a.tar.gz', '..foo', '..', 'dir.d/x', 'x'].map(extOf), ['', '', 'gz', 'foo', '', '', ''])
})
test('without a table every file is plain', () => {
  for (const none of [null, undefined, fileGroupTable(undefined), fileGroupTable('x'), fileGroupTable({})]) assert.equal(fileGroup('a.rs', none), 'plain')
})
test('unknown keys and non-string values are ignored and no class leaks', () => {
  const odd = fileGroupTable({ evil: { ext: ['rs'], names: [] }, source: { ext: ['rs', 7, null, { a: 1 }], names: 'x' }, doc: 'md', data: null, script: { ext: 5 } })
  assert.deepEqual(Object.keys(odd).sort(), ['script', 'source'])
  assert.equal(fileGroup('a.rs', odd), 'source')
  assert.equal(fileGroup('a.md', odd), 'plain')
  assert.equal(fileGroup('a.7', odd), 'plain')
})
test('the page source keeps no extension table of its own', () => {
  for (const path of ['view/files.ts', 'data/file-groups.ts']) { const text = read(path); assert.ok(!text.includes("'tsx'") && !text.includes('"tsx"'), path) }
})
test('every var() in files.css and diff.css is defined in tokens.css, and the two file tokens stand in all three places', () => {
  const tokens = read('style/tokens.css')
  for (const sheet of ['style/files.css', 'style/diff.css']) {
    for (const [, name] of read(sheet).matchAll(/var\((--[\w-]+)/g)) assert.match(tokens, new RegExp(`${name}\\s*:`), `${sheet}: ${name}`)
  }
  for (const [name, light, dark] of [['--file-source', '#c2410c', '#fb923c'], ['--file-script', '#4d7c0f', '#a3e635']]) {
    const found = [...tokens.matchAll(new RegExp(`${name}:\\s*(#[0-9a-f]{6})`, 'g'))].map(m => m[1])
    assert.deepEqual(found, [light, dark, dark], name)
  }
})
