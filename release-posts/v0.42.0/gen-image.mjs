// Render the demo screenshot (a Lantern conflict banner) that the pictures attach.
// usage: gen-image.mjs <out.png>   -- Playwright comes from NODE_PATH.
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const { chromium } = createRequire(import.meta.url)('playwright')
const here = path.dirname(fileURLToPath(import.meta.url))
const browser = await chromium.launch()
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 640 }, deviceScaleFactor: 1 })
  await page.goto('file://' + path.join(here, 'shot.html'))
  await page.screenshot({ path: path.resolve(process.argv[2]) })
} finally { await browser.close() }
