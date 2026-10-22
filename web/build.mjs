// Builds the page `moai web` serves into ../src/web/dist/: app.js (the modules under panel/, bundled), theme.js (the
// page's first script, panel/early.ts: it puts the stored theme on before the first paint), app.css (the sheets under
// panel/style/, joined) and index.html (copied). The server sends a CSP that allows scripts from its own origin only,
// so nothing is inlined — the page loads every file by path.
//
// The page's words are moai's own i18n/*.json, the `page.` keys of them (i18n-bundles.mjs).
//
// The bundle is committed and `cargo build` never runs node. SOURCES records a hash of every input, so
// tests/web_bundle.rs (no node) and the CI web job (node, rebuild and diff) both see a stale bundle.
import { build } from 'esbuild'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { i18nBundles, languageCodes, pageWords } from './i18n-bundles.mjs'

const root = new URL('./', import.meta.url)
const dist = new URL('../src/web/dist/', root)
// Tokens come before the components that read them; any other sheet follows in name order, but for STYLE_LAST.
const STYLE_ORDER = ['tokens.css', 'components.css']
// detail.css was a <style> appended to <head> at run time, after app.css; it stays last so the cascade stays as it was.
const STYLE_LAST = ['detail.css']
const BANNER = '/*! moai web — from buzzni/saycode-extensions@90f545b packages/moai/panel, MIT, Copyright (c) 2026 Buzzni Inc. */'

async function styles() {
  let names
  try {
    names = (await readdir(new URL('panel/style/', root))).filter((name) => name.endsWith('.css'))
  } catch (error) {
    if (error.code === 'ENOENT') return ''
    throw error
  }
  const rank = (name) => (STYLE_ORDER.includes(name) ? STYLE_ORDER.indexOf(name) : STYLE_LAST.includes(name) ? STYLE_ORDER.length + 1 : STYLE_ORDER.length)
  names.sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))
  const sheets = await Promise.all(names.map((name) => readFile(new URL(`panel/style/${name}`, root), 'utf8')))
  return sheets.join('\n')
}

// Every file under dir, as '/'-joined paths relative to the web directory, in byte order.
async function walk(dir) {
  const out = []
  for (const entry of await readdir(new URL(dir, root), { withFileTypes: true })) {
    const path = `${dir}${entry.name}`
    if (entry.isDirectory()) out.push(...(await walk(`${path}/`)))
    else out.push(path)
  }
  return out
}

const byteOrder = (a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))

// FNV-1a 64 over (path bytes, 0x00, content bytes, 0x00) for each input in path order — the same fold
// tests/web_bundle.rs makes, so the test can tell a stale bundle without node.
async function sources() {
  const paths = [...(await walk('panel/')), 'build.mjs', 'i18n-bundles.mjs', 'package.json', 'package-lock.json'].sort(byteOrder)
  let hash = 0xcbf29ce484222325n
  const fold = (bytes) => {
    for (const byte of bytes) hash = ((hash ^ BigInt(byte)) * 0x100000001b3n) & 0xffffffffffffffffn
  }
  for (const path of paths) {
    fold(Buffer.from(path))
    fold([0])
    fold(await readFile(new URL(path, root)))
    fold([0])
  }
  // The page's words come from ../i18n/*.json, but only the `page.` keys reach app.js, so only they are folded in: a
  // change to the CLI's own words does not stale the bundle. Per language file in byte order, (key, 0x00, value, 0x00).
  for (const code of await languageCodes()) {
    fold(Buffer.from(`i18n/${code}.json`))
    fold([0])
    for (const [key, value] of Object.entries(await pageWords(code))) {
      fold(Buffer.from(key))
      fold([0])
      fold(Buffer.from(value))
      fold([0])
    }
  }
  return `fnv1a64 ${hash.toString(16).padStart(16, '0')}\n`
}

const options = {
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: 'es2022',
  charset: 'utf8',
  write: false,
  minifyWhitespace: true,
  minifySyntax: true,
  minifyIdentifiers: false,
  legalComments: 'none',
}
const result = await build({
  ...options,
  // A file path, not URL.pathname: that stays percent-encoded, so a checkout under a path with a space or Hangul fails.
  entryPoints: [fileURLToPath(new URL('panel/main.ts', root))],
  banner: { js: BANNER },
  plugins: [i18nBundles],
})
// index.html loads it in <head> without `defer`, so it holds the first paint: it imports the theme and the key alone.
const early = await build({ ...options, entryPoints: [fileURLToPath(new URL('panel/early.ts', root))] })
const css = `${await styles()}\n[hidden]{display:none!important}\n`
await mkdir(dist, { recursive: true })
await writeFile(new URL('app.js', dist), result.outputFiles[0].text)
await writeFile(new URL('theme.js', dist), early.outputFiles[0].text)
await writeFile(new URL('app.css', dist), css)
await writeFile(new URL('index.html', dist), await readFile(new URL('panel/index.html', root)))
await writeFile(new URL('SOURCES', dist), await sources())
