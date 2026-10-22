// `node --import ./tests/i18n-loader.mjs --test …` (npm test). Some tests import the panel's .ts files straight into
// node, without esbuild; the page's words come in as `moai-i18n:<code>` (web/i18n-bundles.mjs), which node does not
// know. This answers it with the same `page.` slice of ../i18n/<code>.json that the bundle gets.
import { register } from 'node:module'

register('./i18n-hooks.mjs', import.meta.url)
