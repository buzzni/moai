import { fileURLToPath } from 'node:url'
const shim = (name) => fileURLToPath(new URL(`panel/vendor/shim/${name}.ts`, import.meta.url))
const names = { 'markdown-it': ['MarkdownIt', 'default'], '@mdit/plugin-footnote': ['footnote', 'footnote'], '@mdit/plugin-cjk-friendly': ['cjkFriendly', 'cjkFriendly'], '@mdit/plugin-katex': ['katexPlugin', 'katex'], '@mdit/plugin-snippet': ['snippet', 'snippet'], 'katex': ['katex', 'default'], 'mermaid': ['mermaid', 'default'] }
export const vendorGlobals = {
  name: 'vendor-globals',
  setup(build) {
    build.onResolve({ filter: /^(markdown-it|@mdit\/plugin-(footnote|cjk-friendly|katex|snippet)|katex|mermaid)$/ }, args => ({ path: args.path, namespace: 'moai-vendor' }))
    build.onLoad({ filter: /.*/, namespace: 'moai-vendor' }, args => { const [property, exported] = names[args.path]; return { contents: `const value = globalThis.moaiVendor.${property}; export { value as ${exported} };`, loader: 'js' } })
  },
}
export const vendorBuild = { bundle: true, format: 'iife', platform: 'browser', target: 'es2022', minify: true, legalComments: 'eof', write: false, metafile: true, alias: { 'node:fs': shim('fs'), 'node:path': shim('path'), path: shim('path'), upath: shim('path') }, inject: [shim('process')] }
