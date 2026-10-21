// Loads a TypeScript module of the Moai panel for node --test: esbuild bundles it to ESM in memory, so the
// pure modules are tested as the panel bundles them, with no build step before the test.
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'

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
  })
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`)
}
