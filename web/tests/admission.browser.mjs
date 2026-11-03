// End-to-end browser identity and real server admission. Run after `cargo build`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir, networkInterfaces } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn, execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { build } from 'esbuild'
import { chromium, firefox } from 'playwright'

// From this file, not the working directory: `node --test web/tests/…` from the repository root finds the same files.
const web = new URL('..', import.meta.url)
const binary = fileURLToPath(new URL('../target/debug/moai', web))
const bundle = await build({ stdin: { contents: "import { browserId } from './panel/data/browser-id.ts'; window.identify = browserId", resolveDir: fileURLToPath(web), loader: 'ts' }, bundle: true, write: false, format: 'iife' })
const address = Object.values(networkInterfaces()).flat().find(i => i.family === 'IPv4' && !i.internal)?.address

for (const engine of [chromium, firefox]) test(`${engine.name()}: first tabs share real server admission on remote HTTP`, async t => {
  assert.ok(address, 'non-loopback address required')
  const dir = await mkdtemp(join(tmpdir(), 'moai-admission-'))
  t.after(() => rm(dir, { recursive: true, force: true }))
  execFileSync('git', ['init', '-q', dir])
  execFileSync(binary, ['-C', dir, 'init', '--yes', '--no-agents', '--no-driver', '--no-skill', '--no-register'])
  const server = spawn(binary, ['-C', dir, 'web', '--host', '0.0.0.0', '--port', '0', '--json'], { stdio: ['ignore', 'pipe', 'pipe'] })
  t.after(async () => { server.kill(); if (server.exitCode === null) await once(server, 'exit') })
  let output = ''
  for await (const chunk of server.stdout) { output += chunk; if (output.includes('\n')) break }
  const port = JSON.parse(output.split('\n')[0]).port
  const url = `http://${address}:${port}/`
  const browser = await engine.launch()
  t.after(() => browser.close())
  const context = await browser.newContext(), other = await browser.newContext()
  t.after(() => Promise.all([context.close(), other.close()]))
  for (const c of [context, other]) await c.route('**/app.js', route => route.fulfill({ contentType: 'text/javascript', body: bundle.outputFiles[0].text }))
  const pages = await Promise.all(Array.from({ length: 8 }, () => context.newPage()))
  const remote = await other.newPage()
  await Promise.all([...pages, remote].map(async page => { await page.goto(url) }))
  assert.equal(await pages[0].evaluate(() => isSecureContext), false)
  // Simulate storage views that cannot communicate during the first load: localStorage-only coordination must fail.
  await Promise.all(pages.map(page => page.evaluate(() => {
    Storage.prototype.getItem = () => null
    Storage.prototype.setItem = () => {}
  })))
  await pages[0].evaluate(async () => {
    const db = await new Promise((resolve, reject) => { const r = indexedDB.open('moai-browser-id', 1); r.onupgradeneeded = () => r.result.createObjectStore('identity'); r.onsuccess = () => resolve(r.result); r.onerror = () => reject(r.error) })
    window.release = false
    const tx = db.transaction('identity', 'readwrite'), store = tx.objectStore('identity')
    const spin = () => { if (!window.release) store.get('hold').onsuccess = spin }; spin()
    tx.oncomplete = () => db.close()
  })
  const pending = pages.map(p => p.evaluate(() => window.identify()))
  await new Promise(resolve => setTimeout(resolve, 300))
  await pages[0].evaluate(() => { window.release = true })
  const ids = await Promise.all(pending)
  assert.equal(new Set(ids).size, 1)
  const otherId = await remote.evaluate(() => window.identify())
  assert.notEqual(otherId, ids[0])
  const statuses = []
  for (let i = 0; i < pages.length; i++) statuses.push(await pages[i].evaluate(async id => {
    window.stream = new AbortController()
    return (await fetch(`/api/events?browser=${id}`, { signal: window.stream.signal })).status
  }, ids[i]))
  assert.equal(statuses.filter(s => s === 200).length, 3)
  assert.equal(statuses.filter(s => s === 503).length, 5)
  assert.equal(await remote.evaluate(async id => { window.stream = new AbortController(); return (await fetch(`/api/events?browser=${id}`, { signal: window.stream.signal })).status }, otherId), 200)
})
