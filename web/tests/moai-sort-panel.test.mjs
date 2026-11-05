import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const { sortPanel } = await panelModule('view/sort-panel.ts')
const doc = new JSDOM('<!doctype html>').window.document
const words = {
  presets: [{ id: 'release', label: 'Release' }], groups: [{ id: 'none', label: 'None' }],
  levels: [{ id: 'milestone', label: 'Milestone' }, { id: 'item', label: 'Item' }],
  fields: [{ id: 'priority', label: 'Priority', dir: 'asc' }, { id: 'due', label: 'Due', dir: 'asc' }],
  activeLevels: () => ['item'],
  labels: { presets: 'Presets', groups: 'Groups', order: 'Order', add: 'Add', direction: 'Direction', up: 'Up', down: 'Down', remove: 'Remove' },
}
const state = { group: 'none', level: 'item', order: { item: [{ key: 'priority', dir: 'asc' }] } }

test('panel exposes injected vocabulary, active tabs and guards the last field', () => {
  const actions = []
  const panel = sortPanel(doc, state, words, (a) => actions.push(a))
  assert.equal(panel.querySelector('[role=tab]').disabled, true)
  assert.equal(panel.querySelectorAll('[role=tab]')[1].getAttribute('aria-selected'), 'true')
  assert.equal(panel.querySelector('[aria-label=Remove]').disabled, true)
  panel.querySelector('[aria-label=Direction]').click()
  assert.deepEqual(actions.pop(), { type: 'fields', level: 'item', fields: [{ key: 'priority', dir: 'desc' }] })
  panel.querySelector('select').value = 'due'
  panel.querySelector('select + button').click()
  assert.deepEqual(actions.pop(), { type: 'fields', level: 'item', fields: [{ key: 'priority', dir: 'asc' }, { key: 'due', dir: 'asc' }] })
  assert.equal(state.order.item.length, 1, 'editing does not mutate caller state')
})

test('field controls reorder and remove without changing the input', () => {
  const input = { ...state, order: { item: [...state.order.item, { key: 'due', dir: 'asc' }] } }
  const actions = []
  const panel = sortPanel(doc, input, words, (a) => actions.push(a))
  panel.querySelectorAll('[aria-label=Up]')[1].click()
  assert.deepEqual(actions.pop().fields.map((f) => f.key), ['due', 'priority'])
  panel.querySelectorAll('[aria-label=Remove]')[0].click()
  assert.deepEqual(actions.pop().fields.map((f) => f.key), ['due'])
  assert.deepEqual(input.order.item.map((f) => f.key), ['priority', 'due'])
})

test('preset and command text are exposed without interpreting markup', () => {
  const actions = []
  const vocabulary = { ...words, labels: { ...words.labels, command: 'Command', speech: 'Say' } }
  const panel = sortPanel(doc, { ...state, preset: 'release', command: '<sort>', speech: '<say>' }, vocabulary, (a) => actions.push(a))
  const preset = [...panel.querySelectorAll('button')].find((node) => node.textContent === 'Release')
  assert.equal(preset.getAttribute('aria-pressed'), 'true')
  preset.click()
  assert.deepEqual(actions.pop(), { type: 'preset', id: 'release' })
  assert.equal(panel.querySelector('pre').textContent, '<sort>')
  assert.equal(panel.querySelector('q').textContent, '<say>')
  assert.equal(panel.querySelector('sort'), null)
})
