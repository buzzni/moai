import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const { createProposalView, describeThen } = await panelModule('view/propose.ts')

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

test('a follow-up is shown the way the CLI spells it, so the person sees what accepting does', () => {
  assert.equal(describeThen({ verb: 'view', name: 'deferred', on: false }), 'view deferred off')
  assert.equal(describeThen({ verb: 'read', ids: [], all: true, group: null }), 'read --all')
  assert.equal(describeThen({ verb: 'sort', words: ['title:desc', 'priority:desc'], default: false }), 'sort title:desc,priority:desc')
  assert.equal(describeThen({ verb: 'filter', query: null }), 'filter --clear')
  assert.equal(describeThen({ verb: 'find', query: 'parser', scope: 'title' }), 'find parser --in title')
  assert.equal(describeThen({ verb: 'find', query: 'parser', scope: 'all' }), 'find parser')
  assert.equal(describeThen({ verb: 'goto', issue: 'moai-1', enter: true, project: null }), 'goto moai-1 --enter')
  assert.equal(describeThen({ verb: 'pane', list_width: '60', list_height: null, wiki_width: null, gantt_label: null }), 'pane --list-width 60')
})
