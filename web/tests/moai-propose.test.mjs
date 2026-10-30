import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const { createProposalView } = await panelModule('view/propose.ts')

test('a suggestion card announces its text without treating it as markup, and accept or decline names the chosen entry', () => {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', { pretendToBeVisual: true })
  const answers = []
  const view = createProposalView(dom.window.document,
    { title: 'Suggestion', accept: 'Accept', decline: 'Decline', previous: 'Previous', next: 'Next' },
    (entry, accepted) => answers.push([entry.id, accepted]))
  assert.equal(view.el.getAttribute('role'), 'region')
  assert.equal(view.el.getAttribute('aria-live'), 'polite')
  view.show({ id: 'window/1', text: '<img src=x onerror=alert(1)>', then: { verb: 'screen', name: 'board' } })
  view.show({ id: 'window/2', text: 'Second' })
  assert.equal(view.el.querySelector('img'), null)
  assert.match(view.el.textContent, /» Suggestion 1\/2/)
  assert.match(view.el.textContent, /→ screen board/)
  view.accept()
  view.list()
  view.decline()
  assert.deepEqual(answers, [['window/1', true], ['window/2', false]])
  view.remove('window/1')
  view.remove('window/2')
  assert.equal(view.el.hidden, true)
  dom.window.close()
})
