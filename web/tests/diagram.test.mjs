import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

let calls = 0
const original = globalThis.moaiVendor
const mock = { initialize() {}, async render(_id, source) { calls++; if (source === 'invalid') throw new Error('bad diagram'); return {svg:'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100"></svg>'} } }
globalThis.moaiVendor = { ...original, mermaid: mock }
const { hydrateDiagrams } = await panelModule('view/diagram.ts')
const words = {alt:'diagram', error:'failed'}

test('diagrams cache by source and theme and retain failures as text', async () => {
  const dom = new JSDOM('<!doctype html><body></body>')
  const doc = dom.window.document
  const draw = async source => {
    const root = doc.createElement('div'); doc.body.replaceChildren(root)
    const figure = doc.createElement('figure'); figure.className = 'm-md-mermaid'
    const pre = doc.createElement('pre'); pre.textContent = source; figure.append(pre); root.append(figure)
    await hydrateDiagrams(root, doc, words)
    return figure
  }
  for (let i = 0; i < 3; i++) {
    const figure = await draw('cache fixture')
    assert.equal(figure.querySelector('img').getAttribute('width'), '200')
    assert.equal(figure.querySelector('pre').hidden, true)
  }
  assert.equal(calls, 1)
  doc.documentElement.dataset.theme = 'dark'
  await draw('cache fixture')
  assert.equal(calls, 2)
  const invalid = await draw('invalid')
  assert.equal(invalid.querySelector('img'), null)
  assert.equal(invalid.querySelector('pre').hidden, false)
  assert.match(invalid.querySelector('.m-md-diagram-error').textContent, /bad diagram/)
})
