import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { build } from 'esbuild'
import { JSDOM } from 'jsdom'
import { vendorBuild } from '../vendor.mjs'
import './moai-panel-modules.mjs'

const base = new URL('../', import.meta.url)
test('markdown vendor supplies footnotes without loading mermaid', () => {
  const v = globalThis.moaiVendor
  assert.equal(v.mermaid, undefined)
  const md = new v.MarkdownIt().use(v.footnote)
  assert.ok(md.parse('[^1]\n\n[^1]: x', {}).some(t => t.children?.some(c => c.type === 'footnote_ref')))
})
test('browser vendor snippet shim preserves requested path and region', async () => {
  const script = await readFile(new URL('../src/web/dist/vendor.js', base), 'utf8')
  const dom = new JSDOM('<!doctype html>', { runScripts: 'outside-only' })
  dom.window.eval(script)
  const v = dom.window.moaiVendor
  const tokens = new v.MarkdownIt().use(v.snippet, { currentPath: () => '/index.md' }).parse('<<< src/a.rs#r{2 rust}', {})
  const fence = tokens.find(t => t.type === 'fence')
  assert.equal(fence?.meta?.requestedPath, 'src/a.rs')
  assert.equal(fence?.meta?.region, 'r')
  dom.window.close()
})
test('vendor inputs are pinned, licensed and documented', async () => {
  const pkg = JSON.parse(await readFile(new URL('package.json', base)))
  const lock = JSON.parse(await readFile(new URL('package-lock.json', base)))
  for (const [name, version] of Object.entries({ 'markdown-it':'15.0.3', '@mdit/plugin-footnote':'1.2.0', '@mdit/plugin-cjk-friendly':'1.0.0', '@mdit/plugin-katex':'1.2.0', '@mdit/plugin-snippet':'2.0.1', katex:'0.19.1', mermaid:'12.3.0' })) assert.equal(pkg.dependencies[name], version)
  assert.deepEqual(Object.entries(lock.packages).filter(([p]) => p.endsWith('/katex')).map(([,v]) => v.version), ['0.19.1'])
  const result = await build({ ...vendorBuild, entryPoints: [new URL('panel/vendor/index.ts', base).pathname], absWorkingDir: base.pathname })
  const names = new Set(Object.keys(result.metafile.inputs).filter(p => p.startsWith('node_modules/')).map(p => { const parts = p.slice(13).split('/'); return parts[0].startsWith('@') ? parts.slice(0, 2).join('/') : parts[0] }))
  const doc = await readFile(new URL('UPSTREAM.md', base), 'utf8')
  assert.match(await readFile(new URL('node_modules/khroma/license', base), 'utf8'), /MIT License/i)
  const allowed = new Set(['MIT', 'ISC', 'BSD-2-Clause', 'BSD-3-Clause', 'Apache-2.0', '(MPL-2.0 OR Apache-2.0)'])
  for (const name of names) {
    const info = JSON.parse(await readFile(new URL(`node_modules/${name}/package.json`, base)))
    const license = info.license ?? (name === 'khroma' ? 'MIT' : '')
    assert.ok(allowed.has(license) || (name === 'elkjs' && license === 'EPL-2.0 OR GPL-3.0-or-later'), `${name}: unexpected license ${license}`)
    assert.ok(doc.includes(`| ${name} | ${info.version} |`), `${name}: missing from UPSTREAM.md`)
  }
})
