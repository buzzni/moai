import { pageWords } from '../i18n-bundles.mjs'

export async function resolve(specifier, context, next) {
  if (specifier.startsWith('moai-i18n:')) return { url: specifier, shortCircuit: true }
  return next(specifier, context)
}

export async function load(url, context, next) {
  if (!url.startsWith('moai-i18n:')) return next(url, context)
  const words = await pageWords(url.slice('moai-i18n:'.length))
  return { format: 'module', source: `export default ${JSON.stringify(words)}`, shortCircuit: true }
}
