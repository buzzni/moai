// The page's words, read from moai's own bundles (../i18n/*.json). The page imports `moai-i18n:<code>`; this plugin
// answers with that file cut down to the keys under `page.` — the CLI's own words (skills, init, refusals) stay out of
// app.js. The page's words are one set of files with the CLI's, so a translator edits one place (i18n/README.md).
import { readFile, readdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

export const PAGE_PREFIX = 'page.'
const i18nDir = fileURLToPath(new URL('../i18n/', import.meta.url))

/** The `page.` keys of one language file, in byte order of the key. */
export async function pageWords(code) {
  const all = JSON.parse(await readFile(`${i18nDir}${code}.json`, 'utf8'))
  const keys = Object.keys(all).filter((key) => key.startsWith(PAGE_PREFIX)).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)))
  return Object.fromEntries(keys.map((key) => [key, all[key]]))
}

/** Every language file's code, in byte order — a language added under i18n/ is picked up without a list here. */
export async function languageCodes() {
  return (await readdir(i18nDir)).filter((name) => name.endsWith('.json')).map((name) => name.slice(0, -5)).sort()
}

export const i18nBundles = {
  name: 'moai-i18n',
  setup(build) {
    build.onResolve({ filter: /^moai-i18n:/ }, (args) => ({ path: args.path.slice('moai-i18n:'.length), namespace: 'moai-i18n' }))
    build.onLoad({ filter: /.*/, namespace: 'moai-i18n' }, async (args) => ({
      contents: JSON.stringify(await pageWords(args.path)),
      loader: 'json',
      watchFiles: [`${i18nDir}${args.path}.json`],
    }))
  },
}
