/**
 * The page's first script, built by build.mjs into src/web/dist/theme.js. index.html loads it in <head> without
 * `defer`, so it runs before the body is drawn and the first paint is already the theme picked with the theme button
 * (data/theme.ts) — app.js is deferred, and a browser paints the system's colours before it runs. Not an inline script:
 * the CSP allows scripts from the server's own origin only. It imports the theme and the key alone, to stay small; the
 * shell (view/shell.ts) puts the same theme on again from the store once the page is up.
 */
import { MEMORY_KEY } from './data/memory-key.js'
import { applyTheme, storedTheme } from './data/theme.js'

applyTheme(document, storedTheme(MEMORY_KEY, () => window.localStorage))
