// Loads a TypeScript module of the Moai panel for node --test: esbuild bundles it to ESM in memory, so the
// pure modules are tested as the panel bundles them, with no build step before the test.
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'
import { i18nBundles } from '../i18n-bundles.mjs'
import { vendorGlobals, vendorBuild } from '../vendor.mjs'

const vendor = await build({ ...vendorBuild, format: 'esm', entryPoints: [panelPath('vendor/markdown.ts')] })
const markdown = await import(`data:text/javascript;base64,${Buffer.from(vendor.outputFiles[0].text).toString('base64')}`)
globalThis.moaiVendor = Object.freeze({ ...markdown.markdownVendor })

function panelPath(relative) { return fileURLToPath(new URL(`../panel/${relative}`, import.meta.url)) }

const panel = fileURLToPath(new URL('../panel/', import.meta.url))

export async function panelModule(relative) {
  const result = await build({
    entryPoints: [panel + relative],
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    target: 'es2022',
    write: false,
    logLevel: 'silent',
    plugins: [i18nBundles, vendorGlobals],
  })
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`)
}
