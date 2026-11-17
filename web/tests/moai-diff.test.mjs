import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { createDiff, pair } from '../panel/view/diff.ts'
const line = (kind, text) => ({ kind, text, old_no: kind === 'add' ? null : 1, new_no: kind === 'delete' ? null : 1, spans: null })
const words = { owner: 'Epic {id} branch', unified: 'Unified', side: 'Side', narrow: 'Narrow', binary: 'Binary', empty: 'Empty', truncated: 'Truncated' }
test('delete and add runs pair as the shared diff reader does', () => {
  const rows = [line('context', 'c'), line('delete', 'a'), line('delete', 'b'), line('delete', 'c'), line('add', 'x')]
  assert.deepEqual(pair(rows).map(([a, b]) => [a?.text ?? null, b?.text ?? null]), [['c', 'c'], ['a', 'x'], ['b', null], ['c', null]])
})
test('diff content stays text, syntax classes are closed and later files are lazy', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const view = createDiff(doc, words); doc.body.append(view.el)
  const unsafe = line('add', '<script>한</script>'); unsafe.spans = [[8, 9, 'unknown-class']]
  const file = path => ({ path, old_path: null, status: 'added', adds: 1, dels: 0, binary: false, hunks: [{ old_start: 0, old_len: 0, new_start: 1, new_len: 1, section: '', lines: [unsafe] }] })
  view.show({ files: [file('a'), file('b')], truncated: false })
  assert.equal(doc.querySelector('script'), null); assert.match(doc.body.textContent, /<script>한/)
  assert.ok(doc.querySelector('.m-syntax-other')); assert.equal(doc.querySelectorAll('.m-diff-line').length, 1)
  const second = doc.querySelectorAll('details')[1]; second.open = true; second.dispatchEvent(new dom.window.Event('toggle'))
  assert.equal(doc.querySelectorAll('.m-diff-line').length, 2)
  dom.window.close()
})
test('side-by-side pairs have independent text and fall back when narrow', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const view = createDiff(doc, words); doc.body.append(view.el)
  view.show({ files: [{ path: 'x.rs', old_path: 'old.rs', status: 'renamed', adds: 1, dels: 2, binary: false, hunks: [{ old_start: 1, old_len: 2, new_start: 1, new_len: 1, section: '', lines: [line('delete', 'old 1'), line('delete', 'old 2'), line('add', 'new')] }] }], truncated: false })
  view.setWidth(1000); view.setLayout('side-by-side')
  assert.match(doc.querySelector('summary').textContent, /old.rs → x.rs/)
  assert.deepEqual([...doc.querySelectorAll('.m-diff-pairs .m-diff-line')].map(n => n.textContent.trim()), ['1-old 1', '1+new', '1-old 2', ''])
  view.setWidth(400); assert.equal(doc.querySelector('.m-diff-pairs'), null); assert.match(doc.body.textContent, /Narrow/)
  dom.window.close()
})
test('refresh preserves folded files and scroll positions', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const view = createDiff(doc, words); doc.body.append(view.el)
  const file = path => ({ path, old_path: null, status: 'modified', adds: 1, dels: 0, binary: false, hunks: [{ old_start: 1, old_len: 1, new_start: 1, new_len: 1, section: '', lines: [line('add', 'new')] }] })
  const answer = { files: [file('a'), file('b')], truncated: false }; view.show(answer)
  const details = doc.querySelectorAll('details'); details[0].open = false; details[0].dispatchEvent(new dom.window.Event('toggle'))
  const body = doc.querySelector('.m-diff-body'); body.scrollTop = 55; body.scrollLeft = 80
  view.show(answer, true)
  assert.equal(doc.querySelector('details').open, false); assert.equal(body.scrollTop, 55); assert.equal(body.scrollLeft, 80)
  dom.window.close()
})
