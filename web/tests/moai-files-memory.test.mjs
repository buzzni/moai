import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'
const { normalizeMemory, rememberedOf, patchMemory } = await panelModule('data/memory.ts')
const { initialState, reduce } = await panelModule('store.ts')
const { menuLayer } = await panelModule('keys/table.ts')

test('the files tab and its split survive storage and preserve older choices', () => {
  const old = normalizeMemory({ tab: 'wiki', detail: { visible: false, position: 'left' } })
  assert.equal(old.tab, 'wiki')
  let state = reduce(initialState(), { type: 'restore', settings: old })
  state = reduce(state, { type: 'tab', tab: 'files' })
  state = reduce(state, { type: 'files-ratio', ratio: 0.7 })
  state = reduce(state, { type: 'diff-layout', layout: 'side-by-side' })
  const saved = patchMemory({ ...old, extra: 7 }, old, rememberedOf(state))
  assert.equal(saved.extra, 7)
  const restored = normalizeMemory(saved)
  assert.equal(restored.tab, 'files'); assert.equal(restored.filesRatio, 0.7); assert.equal(restored.diffLayout, 'side-by-side')
  assert.equal(restored.detail.visible, false)
  assert.equal(menuLayer('SPC g', { mode: 'browse' }).items.find(item => item.key === 'f').action, 'tab.files')
})
