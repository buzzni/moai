// Moai panel toolbar: reducer, hint acceptance, chip removal, sort popover, rendering.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { panelModule } from './moai-panel-modules.mjs'

const tb = await panelModule('view/toolbar.ts')
const q = await panelModule('data/query.ts')
const dom = new JSDOM('<!doctype html><body></body>')
const doc = dom.window.document

const rows = [
  { id: 'a1', title: 'Fix Login bug', status: 'todo', tags: ['bug', 'ui'], priority: 1 },
  { id: 'a2', title: 'Write docs', status: 'in_progress', tags: ['docs'], priority: 2 },
  { id: 'a3', title: 'Old login thing', status: 'done', tags: ['bug'], priority: 2 },
]
const reduce = (state, ...actions) => actions.reduce((s, a) => tb.toolbarReduce(s, a), state)

test('search: open, type, close clears; there is no scope to cycle', () => {
  let s = reduce(tb.initialToolbar(), { type: 'search-open' }, { type: 'search-text', text: 'log' })
  assert.equal(s.searchOpen, true)
  assert.equal(s.searchText, 'log')
  assert.equal('scope' in s, false, 'moai looks in every field: the toolbar has no scope')
  s = reduce(s, { type: 'search-close' })
  assert.deepEqual([s.searchOpen, s.searchText], [false, ''])
})

test('highlight ranges are case-folded and non-overlapping', () => {
  assert.deepEqual(tb.highlightRanges('Fix Login bug LOGIN', 'login'), [[4, 9], [14, 19]])
  assert.deepEqual(tb.highlightRanges('abc', ' '), [])
})

test('hint acceptance: key gets =, value replaces the piece, quotes only when needed', () => {
  const key = tb.acceptHint('sta', { partial: 'sta', from: 0, to: 3 }, { value: 'status', count: 0 })
  assert.deepEqual(key, { text: 'status=', cursor: 7 })
  const value = tb.acceptHint('tag=bug,u', { key: 'tag', partial: 'u', from: 8, to: 9 }, { value: 'ui', count: 1 })
  assert.deepEqual(value, { text: 'tag=bug,ui', cursor: 10 })
  const spaced = tb.acceptHint('assignee=', { key: 'assignee', partial: '', from: 9, to: 9 }, { value: 'Ann Lee', count: 1 })
  assert.equal(spaced.text, 'assignee="Ann Lee"')
})

test('hint acceptance replaces the whole piece under the caret, as slotAt finds it, and never breaks the text', () => {
  // Each case: the text with `|` for the caret, the hint chosen, the text and caret after it (`|` again).
  const accept = (marked, value) => {
    const cursor = marked.indexOf('|')
    const text = marked.replace('|', '')
    const done = tb.acceptHint(text, q.slotAt(text, cursor), { value, count: 1 })
    return done.text.slice(0, done.cursor) + '|' + done.text.slice(done.cursor)
  }
  // A key edited in front of its `=`: the `=` is reused, not doubled.
  assert.equal(accept('sta|=todo', 'status'), 'status=|todo')
  assert.equal(accept('st|at=todo', 'status'), 'status=|todo')
  assert.equal(accept('tag=ui sta|', 'status'), 'tag=ui status=|')
  // A caret inside a value: the rest of that value goes too, the next one (after the comma) stays.
  assert.equal(accept('tag=b|u,ui', 'bug'), 'tag=bug|,ui')
  assert.equal(accept('tag=ui,b|u status=todo', 'bug'), 'tag=ui,bug| status=todo')
  // A quoted value: an open quote is closed, and a caret past the closing quote replaces only what is inside.
  assert.equal(accept('assignee="An|', 'Ann Lee'), 'assignee="Ann Lee"|')
  assert.equal(accept('assignee="Ann Lee"|', 'Ann Lee'), 'assignee="Ann Lee|"')
  assert.equal(accept('assignee="A|nn" tag=ui', 'Ann Lee'), 'assignee="Ann Lee|" tag=ui')
  // What the result reads as: the item the hint was for, and the others untouched.
  const read = (marked, value) => q.splitItems(accept(marked, value).replace('|', ''))
  // An open quote would swallow the items after it: the chosen value closes it where its word ended.
  assert.equal(accept('assignee="An| tag=ui', 'Ann Lee'), 'assignee="Ann Lee"| tag=ui')
  assert.deepEqual(read('assignee="An| tag=ui', 'Ann Lee'), ['assignee=Ann Lee', 'tag=ui'])
  assert.deepEqual(read('assignee="A|nn" tag=ui', 'Ann Lee'), ['assignee=Ann Lee', 'tag=ui'])
})

test('hint flow through the reducer', () => {
  let s = reduce(tb.initialToolbar(), { type: 'filter-text', text: 'status=', cursor: 7 })
  assert.equal(s.hintsOpen, true)
  s = reduce(s, { type: 'hint-move', delta: 1, count: 3 }, { type: 'hint-move', delta: 1, count: 3 })
  assert.equal(s.hintIndex, 1)
  s = reduce(s, { type: 'hint-move', delta: -1, count: 3 }, { type: 'hint-move', delta: -1, count: 3 })
  assert.equal(s.hintIndex, 2)
  const { slot, hints } = { slot: { key: 'status', partial: '', from: 7, to: 7 }, hints: [{ value: 'todo', count: 1 }] }
  s = reduce(s, { type: 'hint-accept', slot, hint: hints[0] })
  assert.deepEqual([s.filterText, s.cursor, s.hintIndex], ['status=todo', 11, -1])
})

test('chip removal drops one item, keeps the rest', () => {
  assert.equal(tb.removeItem('status=todo tag=bug,ui epic=e1', 1), 'status=todo epic=e1')
  assert.equal(tb.removeItem('status=todo', 0), '')
  const s = reduce({ ...tb.initialToolbar(), filterText: 'tag=a tag=b' }, { type: 'chip-remove', index: 0 })
  assert.equal(s.filterText, 'tag=b')
})

test('sort edit through the reducer: replace, append, flip, replace twice flips', () => {
  let s = reduce(tb.initialToolbar(), { type: 'sort-edit', key: 'updated', how: 'replace' })
  assert.equal(s.sort.fields.map((f) => f.key + ':' + f.dir).join(), 'updated:desc')
  s = reduce(s, { type: 'sort-edit', key: 'title', how: 'append' })
  assert.equal(s.sort.fields.length, 2)
  s = reduce(s, { type: 'sort-edit', key: 'title', how: 'flip' })
  assert.equal(s.sort.fields[1].dir, 'desc')
  s = reduce(s, { type: 'sort-toggle' })
  assert.equal(s.sortOpen, true)
})

test('render: chips, moai\'s refusal, hidden note, hints, sort popover', () => {
  const seen = []
  const handlers = { dispatch: (a) => seen.push(a) }
  const base = { ...tb.initialToolbar(), searchOpen: true, searchText: 'login', filterText: 'tag=bug,ui priority=1', cursor: 21 }
  // The list hides what its view toggles say (list-model.ts narrow), not what the CLI hides by default: a done
  // row the view shows is not "hidden", a row in a column the user hid is.
  const hiddenBy = (row) => (row.status === 'in_progress' ? 'column' : undefined)
  // The rows the search found are moai's (`found`); the toolbar only counts and shows them.
  const plain = tb.renderToolbar(base, { doc, rows, found: [rows[0], rows[2]], lang: 'en' }, handlers)
  assert.equal(plain.found.length, 2)
  assert.deepEqual(plain.hidden, {}, 'without the view nothing counts as hidden: a done row is not hidden by itself')
  assert.equal(plain.el.querySelector('.m-hidden-note').hidden, true)
  assert.equal(plain.el.querySelector('select'), null, 'no scope select: moai searches every field')
  const noFound = tb.renderToolbar(base, { doc, rows, lang: 'en' }, handlers)
  assert.equal(noFound.found.length, 0, 'nothing found until moai answers')
  const view = tb.renderToolbar({ ...base, searchText: 'o' }, { doc, rows, found: rows, lang: 'en', hiddenBy }, handlers)
  assert.equal(view.el.querySelectorAll('.m-chip').length, 2)
  assert.equal(view.searchInput.value, 'o')
  assert.equal(view.found.length, 3)
  assert.deepEqual(view.hidden, { column: 1 })
  assert.equal(view.el.querySelector('.m-hidden-note').textContent, '1 hidden: hidden column 1')
  view.el.querySelector('.m-chip-x').click()
  assert.deepEqual(seen.pop(), { type: 'chip-remove', index: 0 })
  assert.equal(view.el.querySelector('.m-sort-pop'), null)

  // A filter moai refused: its words under the box, the input invalid, the chips neutral. The toolbar judges nothing.
  const bad = tb.renderToolbar({ ...base, filterText: 'nope=1' }, { doc, rows, problem: 'unknown filter key: nope' }, handlers)
  assert.equal(bad.el.querySelector('[role=alert]').textContent, 'unknown filter key: nope')
  assert.equal(bad.el.querySelector('[role=alert]').hidden, false)
  assert.equal(bad.filterInput.getAttribute('aria-invalid'), 'true')
  const fine = tb.renderToolbar({ ...base, filterText: 'nope=1' }, { doc, rows }, handlers)
  assert.equal(fine.el.querySelector('[role=alert]').hidden, true, 'with no refusal the page does not complain about a key it does not know')
  assert.equal(fine.filterInput.hasAttribute('aria-invalid'), false)

  const hinting = tb.renderToolbar({ ...base, filterText: 'status=', cursor: 7, hintsOpen: true }, { doc, rows, lang: 'en' }, handlers)
  const items = hinting.el.querySelectorAll('.m-hints .m-menu-item')
  assert.ok(items.length >= 2)
  items[0].click()
  assert.equal(seen.pop().type, 'hint-accept')


  // A click in the filter moves the caret the hints follow.
  hinting.filterInput.setSelectionRange(3, 3)
  hinting.filterInput.click()
  assert.deepEqual(seen.pop(), { type: 'filter-cursor', cursor: 3 })

  const sorting = tb.renderToolbar({ ...base, sortOpen: true }, { doc, rows, lang: 'en' }, handlers)
  assert.equal(sorting.el.querySelectorAll('.m-sort-pop [data-key]').length, 8, 'the deadline is a sort key too')
  assert.ok(sorting.el.querySelector('.m-sort-pop [data-key=due]'))
  sorting.el.querySelector('[data-key=title] [data-how=append]').click()
  assert.deepEqual(seen.pop(), { type: 'sort-edit', key: 'title', how: 'append' })
})

test('the toolbar is built once: an update writes an input only when its text differs, never during an IME composition', () => {
  const seen = []
  const live = tb.createToolbar({ doc, lang: 'ko' }, { dispatch: (a) => seen.push(a) })
  doc.body.append(live.el)
  const state = { ...tb.initialToolbar(), searchOpen: true }
  live.update(state, { rows })
  const input = live.searchInput
  input.focus()
  // A Korean IME composes 한: the input event is reported, the store answers with the same text.
  input.dispatchEvent(new dom.window.CompositionEvent('compositionstart', { bubbles: true }))
  input.value = 'ㅎ'
  input.dispatchEvent(new dom.window.InputEvent('input', { bubbles: true, isComposing: true }))
  assert.deepEqual(seen.pop(), { type: 'search-text', text: 'ㅎ' })
  live.update({ ...state, searchText: 'ㅎ' }, { rows })
  // The IME moved on to 하 before the store's answer came back: a late update does not write the text back.
  input.value = '하'
  live.update({ ...state, searchText: 'ㅎ' }, { rows })
  assert.equal(live.searchInput, input, 'the same element')
  assert.equal(input.value, '하', 'the composition is not written over')
  assert.equal(doc.activeElement, input)
  input.dispatchEvent(new dom.window.CompositionEvent('compositionend', { bubbles: true }))
  assert.deepEqual(seen.pop(), { type: 'search-text', text: '하' }, 'the committed text is reported once more')
  // Outside a composition the store's text wins (a search closed elsewhere clears it).
  live.update({ ...state, searchText: '' }, { rows })
  assert.equal(input.value, '')
  // The filter input stays too, and Enter applies the filter (Enter with a hint chosen takes the hint).
  const filter = live.filterInput
  live.update({ ...state, filterText: 'status=todo', cursor: 11 }, { rows })
  assert.equal(live.filterInput, filter)
  filter.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
  assert.deepEqual(seen.pop(), { type: 'filter-apply' })
  filter.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true, isComposing: true }))
  assert.equal(seen.length, 0, 'an Enter that ends a composition applies nothing')
  live.destroy()
})

test('filter-apply applies whatever was typed (moai judges it); a chip removal applies at once', () => {
  let s = reduce(tb.initialToolbar(), { type: 'filter-text', text: 'status=todo', cursor: 11 }, { type: 'filter-apply' })
  assert.equal(s.filterApplied, 'status=todo')
  s = reduce(s, { type: 'filter-text', text: 'status=todo bogus=1', cursor: 19 }, { type: 'filter-apply' })
  assert.equal(s.filterApplied, 'status=todo bogus=1', 'a key the page does not know is applied: the question goes to moai')
  s = reduce(s, { type: 'chip-remove', index: 1 })
  assert.deepEqual([s.filterText, s.filterApplied], ['status=todo', 'status=todo'])
  s = reduce(s, { type: 'chip-remove', index: 0 })
  assert.deepEqual([s.filterText, s.filterApplied], ['', ''])
})

test('the hidden note reads the search as the list does: spaces around it change nothing', () => {
  const hiddenBy = (row) => (row.status === 'in_progress' ? 'column' : undefined)
  const plain = tb.searchView(rows, { searchText: 'o' }, hiddenBy)
  for (const text of [' o', 'o ', '  o  ']) {
    const spaced = tb.searchView(rows, { searchText: text }, hiddenBy)
    assert.deepEqual(spaced.hidden, plain.hidden, JSON.stringify(text))
    assert.deepEqual(spaced.found.map((row) => row.id), plain.found.map((row) => row.id))
  }
})
