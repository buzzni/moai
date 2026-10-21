// Builds the board panel into one file, dist/panel.html. Desktop runs a panel under a CSP that allows only
// inline <script> and <style>, and reads at most 512 KiB of it, so the modules under panel/ are bundled with
// esbuild and the stylesheets under panel/style/ are inlined into panel/index.html (media-preview does the same).
import { build } from 'esbuild'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const root = new URL('./', import.meta.url)
const PANEL_LIMIT = 512 * 1024
// Tokens come before the components that read them; any other sheet follows in name order.
const STYLE_ORDER = ['tokens.css', 'components.css']

async function styles() {
  let names
  try {
    names = (await readdir(new URL('panel/style/', root))).filter((name) => name.endsWith('.css'))
  } catch (error) {
    if (error.code === 'ENOENT') return ''
    throw error
  }
  const rank = (name) => (STYLE_ORDER.includes(name) ? STYLE_ORDER.indexOf(name) : STYLE_ORDER.length)
  names.sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))
  const sheets = await Promise.all(names.map((name) => readFile(new URL(`panel/style/${name}`, root), 'utf8')))
  return sheets.join('\n')
}

const result = await build({
  // A file path, not URL.pathname: that stays percent-encoded, so a checkout under a path with a space or Hangul fails.
  entryPoints: [fileURLToPath(new URL('panel/main.ts', root))],
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: 'es2022',
  charset: 'utf8',
  write: false,
  minifyWhitespace: true,
  minifySyntax: true,
  // Identifiers stay readable: a mangled two-letter name can spell a word the packed-panel check rejects ("ai").
  minifyIdentifiers: false,
  legalComments: 'none',
})
const script = result.outputFiles[0].text
const css = await styles()
const template = await readFile(new URL('panel/index.html', root), 'utf8')
for (const marker of ['<!-- moai-style -->', '<!-- moai-script -->']) {
  if (!template.includes(marker)) throw new Error(`panel/index.html is missing ${marker}`)
}
// A literal "</script>" or "</style>" inside the inlined text would end the element early.
if (/<\/script/i.test(script)) throw new Error('the panel bundle contains </script>')
if (/<\/style/i.test(css)) throw new Error('a panel stylesheet contains </style>')
// Functions as replacements, so a "$&" in the bundle is not read as a pattern.
const html = template
  .replace('<!-- moai-style -->', () => (css ? `<style>${css}</style>` : ''))
  .replace('<!-- moai-script -->', () => `<script>${script}</script>`)
const bytes = Buffer.byteLength(html, 'utf8')
if (bytes > PANEL_LIMIT) throw new Error(`dist/panel.html is ${bytes} bytes; Desktop loads at most ${PANEL_LIMIT}`)
await mkdir(new URL('dist/', root), { recursive: true })
await writeFile(new URL('dist/panel.html', root), html)
