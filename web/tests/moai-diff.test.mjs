import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'
import { createDiff, pair, terminalLine, SPIN_DELAY_MS } from '../panel/view/diff.ts'
import { patchQueue } from '../panel/data/changes.ts'
import { execFileSync } from 'node:child_process'
const line = (kind, text) => ({ kind, text, old_no: kind === 'add' ? null : 1, new_no: kind === 'delete' ? null : 1, spans: null })
const words = { owner: 'Epic {id} branch', unified: 'Unified', side: 'Side', narrow: 'Narrow', binary: 'Binary', empty: 'Empty', truncated: 'Truncated', reading: 'Reading…', retry: 'Retry', timeout: 'Timed out', large: 'Too large — {n} lines', anyway: 'Load anyway', plain: 'Plain', terminal: 'In a terminal:' }
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

test('previous from outside file summaries focuses the last file', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const view = createDiff(doc, words); doc.body.append(view.el)
  const file = path => ({ path, old_path: null, status: 'modified', adds: 0, dels: 0, binary: false, hunks: [] })
  view.show({ files: [file('a'), file('b'), file('c')], truncated: false })
  view.focus(); assert.equal(view.command('prev'), true)
  assert.match(doc.activeElement.textContent, /c/)
  dom.window.close()
})
test('the layout buttons press the layout in force and a press chooses it', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const chosen = []
  const view = createDiff(doc, words, layout => chosen.push(layout)); doc.body.append(view.el)
  const [unified, side] = view.el.querySelectorAll('.m-diff-toolbar button')
  assert.ok(unified.classList.contains('m-iconbtn') && side.classList.contains('m-iconbtn'))
  assert.deepEqual([unified, side].map(b => b.getAttribute('aria-pressed')), ['true', 'false'])
  assert.equal(unified.getAttribute('aria-label'), 'Unified · on'); assert.equal(side.title, 'Side · off')
  side.click()
  assert.deepEqual([unified, side].map(b => b.getAttribute('aria-pressed')), ['false', 'true']); assert.deepEqual(chosen, ['side-by-side'])
  side.click(); assert.deepEqual(chosen, ['side-by-side'])
  view.setLayout('unified')
  assert.deepEqual([unified, side].map(b => b.getAttribute('aria-pressed')), ['true', 'false'])
  assert.equal(view.command('toggle'), true); assert.deepEqual(chosen, ['side-by-side', 'side-by-side'])
  assert.equal(side.getAttribute('aria-label'), 'Side · on')
})

// Two steps (moai-dy7a.lb0): every head from the list at once, each patch when its file opens, a spinner only for a read
// that takes a moment, and no answer landing on a pane that moved on.
const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
const head = (path, adds = 1, dels = 0, extra = {}) => ({ path, old_path: null, status: 'modified', adds, dels, binary: false, ...extra })
const patchOf = (path, text = 'new', extra = {}) => ({ files: [{ ...head(path), hunks: [{ old_start: 1, old_len: 1, new_start: 1, new_len: 1, section: '', lines: [line('add', text)] }] }], truncated: false, highlight: 'full', ...extra })
function pane(hooks = {}) {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const asked = []
  const view = createDiff(doc, words, undefined, undefined, { want: ticket => asked.push(ticket), ...hooks }); doc.body.append(view.el)
  const open = path => { const d = doc.querySelector(`details[data-path="${path}"]`); d.open = true; d.dispatchEvent(new dom.window.Event('toggle')); return d }
  return { dom, doc, view, asked, open, text: () => view.el.querySelector('.m-diff-body').textContent }
}
test('a list stands every head at once and asks only for the open file, then each file as it opens', () => {
  const p = pane()
  p.view.list({ files: [head('a'), head('b'), head('c')], truncated: false })
  assert.deepEqual([...p.doc.querySelectorAll('summary .m-diff-path')].map(n => n.textContent), ['a', 'b', 'c'])
  assert.deepEqual(p.asked.map(t => t.path), ['a'])
  assert.equal(p.asked[0].quiet, false)
  p.open('c'); assert.deepEqual(p.asked.map(t => t.path), ['a', 'c'])
  // Every head takes the focus, patch or not.
  p.view.focus(); p.view.command('next'); p.view.command('next'); assert.match(p.doc.activeElement.textContent, /b/)
  p.asked[0].done(patchOf('a', 'first line')); assert.match(p.text(), /first line/)
  p.dom.window.close()
})
test('a file read shows its spinner and word only after a moment, and loses them on answer or failure', async () => {
  const p = pane()
  p.view.list({ files: [head('a'), head('b')], truncated: false })
  assert.equal(p.doc.querySelector('.m-spin'), null); assert.doesNotMatch(p.text(), /Empty/)
  assert.equal(p.doc.querySelector('details[data-path="a"]').getAttribute('aria-busy'), 'true')
  await wait(SPIN_DELAY_MS + 60)
  const row = p.doc.querySelector('details[data-path="a"] .m-diff-state')
  assert.ok(row.querySelector('.m-spin')); assert.match(row.textContent, /Reading…/)
  p.asked[0].done(patchOf('a')); assert.equal(p.doc.querySelector('.m-spin'), null); assert.equal(p.doc.querySelector('[aria-busy]'), null)
  p.open('b'); p.asked[1].failed('<b>no</b> such file'); await wait(SPIN_DELAY_MS + 60)
  assert.equal(p.doc.querySelector('.m-spin'), null); assert.equal(p.doc.querySelector('details[data-path="b"] b'), null)
  assert.match(p.text(), /<b>no<\/b> such file/)
  // One file's failure leaves the other's lines and the pane's other heads alone; its retry asks again.
  assert.match(p.doc.querySelector('details[data-path="a"]').textContent, /new/)
  p.doc.querySelector('details[data-path="b"] .m-diff-retry').click()
  assert.deepEqual(p.asked.map(t => t.path), ['a', 'b', 'b'])
  p.dom.window.close()
})
test('a quick answer never flashes a spinner', async () => {
  const p = pane()
  p.view.list({ files: [head('a')], truncated: false })
  await wait(20); p.asked[0].done(patchOf('a'))
  await wait(SPIN_DELAY_MS + 60); assert.equal(p.doc.querySelector('.m-spin'), null)
  p.view.reading(); await wait(20); p.view.list({ files: [head('a')], truncated: false }); await wait(SPIN_DELAY_MS + 60)
  assert.equal(p.doc.querySelector('.m-diff-body > .m-diff-state .m-spin'), null)
  p.dom.window.close()
})
test('the pane reading its list says nothing of "no changes", spins after a moment, and stops when hidden or answered', async () => {
  const p = pane()
  p.view.reading()
  assert.doesNotMatch(p.text(), /Empty/); assert.equal(p.doc.querySelector('.m-spin'), null)
  assert.equal(p.doc.querySelector('.m-diff-body').getAttribute('aria-busy'), 'true')
  const height = p.doc.querySelector('.m-diff-body > .m-diff-state'); assert.ok(height, 'the row stands before the spinner does')
  await wait(SPIN_DELAY_MS + 60)
  assert.ok(p.doc.querySelector('.m-diff-body .m-spin')); assert.match(p.text(), /Reading…/)
  p.view.list({ files: [head('a')], truncated: false }); assert.equal(p.doc.querySelector('.m-diff-body > .m-diff-state'), null)
  p.view.reading(); p.view.clear(); await wait(SPIN_DELAY_MS + 60)
  assert.equal(p.doc.querySelector('.m-spin'), null); assert.equal(p.text(), '')
  // A list read that failed says why, and its retry asks the pane's owner again.
  let again = 0; p.view.setHooks({ retry: () => again++ })
  p.view.failed('Timed out'); assert.match(p.text(), /Timed out/); p.doc.querySelector('.m-diff-retry').click(); assert.equal(again, 1)
  p.dom.window.close()
})
test('a pane that moved on kills every ticket it handed out: a late answer lands nowhere and leaves no spinner', async () => {
  const p = pane()
  p.view.list({ files: [head('a')], truncated: false })
  const first = p.asked[0]
  p.view.reading(); assert.equal(first.live(), false)
  p.view.list({ files: [head('a')], truncated: false })
  first.done(patchOf('a', 'from the other issue')); first.failed('late')
  assert.doesNotMatch(p.text(), /from the other issue|late/)
  assert.equal(p.asked[1].live(), true)
  p.view.clear(); assert.equal(p.asked[1].live(), false); await wait(SPIN_DELAY_MS + 60); assert.equal(p.doc.querySelector('.m-spin'), null)
  p.dom.window.close()
})
test('a file past 5,000 changed lines is asked about first, with its count and the terminal line as text', () => {
  const hostile = "it's a <b>dir</b>/x y.rs"
  const p = pane({ command: file => terminalLine('moai-1', 'commits', file.path) })
  p.view.list({ files: [head(hostile, 4000, 1001), head('b', 2500, 2500)], truncated: false })
  assert.deepEqual(p.asked.map(t => t.path), [])
  assert.match(p.text(), /Too large — 5,001 lines/)
  assert.equal(p.doc.querySelector('.m-diff-large b'), null)
  assert.equal(p.doc.querySelector('.m-diff-large code').textContent, `moai diff moai-1 --scope commits --path='it'\\''s a <b>dir</b>/x y.rs'`)
  p.doc.querySelector('.m-diff-large .m-diff-retry').click()
  assert.deepEqual(p.asked.map(t => t.path), [hostile]); assert.equal(p.doc.querySelector('.m-diff-large'), null)
  // Exactly 5,000 is not past the line: opening it asks.
  p.open('b'); assert.deepEqual(p.asked.map(t => t.path), [hostile, 'b'])
  p.dom.window.close()
})
test('a pane with no terminal line (a checkout) offers the fetch alone', () => {
  const p = pane()
  p.view.list({ files: [head('big', 6000)], truncated: false })
  assert.ok(p.doc.querySelector('.m-diff-large .m-diff-retry')); assert.equal(p.doc.querySelector('.m-diff-large code'), null)
  p.dom.window.close()
})
test('the terminal line runs as written in a shell: spaces and quotes stay one argument', () => {
  const path = "dir with space/it's \"odd\" $HOME `x`.rs"
  const line = terminalLine('moai-1', 'working', path, true)
  const args = execFileSync('sh', ['-c', `set -- ${line.slice('moai '.length)}; printf '%s\\n' "$@"`], { encoding: 'utf8' }).split('\n').slice(0, -1)
  assert.deepEqual(args, ['diff', 'moai-1', '--scope', 'working', `--path=${path}`, '--worktree'])
})
test('binary files and files with no line changed are told from their head, never asked', () => {
  const p = pane()
  p.view.list({ files: [head('img.png', null, null, { binary: true }), head('mode', 0, 0)], truncated: false })
  p.open('mode'); assert.deepEqual(p.asked, [])
  assert.match(p.doc.querySelector('details[data-path="img.png"]').textContent, /Binary/)
  assert.match(p.doc.querySelector('details[data-path="mode"]').textContent, /Empty/)
  p.dom.window.close()
})
test('an answer without colours says so under its file, and a patch cut at 2 MiB says it was cut', () => {
  const p = pane()
  p.view.list({ files: [head('a', 3), head('b')], truncated: false })
  p.asked[0].done({ ...patchOf('a'), truncated: true, highlight: 'none' })
  const a = p.doc.querySelector('details[data-path="a"]').textContent
  assert.match(a, /Truncated/); assert.match(a, /Plain/)
  p.open('b'); p.asked[1].done({ ...patchOf('b'), truncated: true })
  // Every line came: `truncated` spoke of the list (cut at 5,000 files), not of this patch.
  assert.doesNotMatch(p.doc.querySelector('details[data-path="b"]').textContent, /Truncated|Plain/)
  p.dom.window.close()
})
test('a refresh with the same list rebuilds nothing; a changed open file is read again quietly, nothing else is', () => {
  const p = pane()
  p.view.list({ files: [head('a'), head('b'), head('c')], truncated: false })
  p.asked[0].done(patchOf('a', 'old a')); p.open('b'); p.asked[1].done(patchOf('b', 'old b'))
  const node = p.doc.querySelector('details[data-path="a"]')
  const body = p.doc.querySelector('.m-diff-body'); body.scrollTop = 40
  p.view.list({ files: [head('a'), head('b'), head('c')], truncated: false }, true)
  assert.equal(p.doc.querySelector('details[data-path="a"]'), node); assert.equal(p.asked.length, 2)
  p.view.list({ files: [head('a'), head('b', 2), head('c', 9)], truncated: false }, true)
  assert.deepEqual(p.asked.slice(2).map(t => [t.path, t.quiet]), [['b', true]])
  assert.match(p.text(), /old b/); assert.equal(p.doc.querySelector('.m-spin'), null)
  assert.equal(p.doc.querySelector('details[data-path="a"]').open, true); assert.equal(p.doc.querySelector('details[data-path="b"]').open, true)
  assert.equal(p.doc.querySelector('details[data-path="c"]').open, false); assert.equal(body.scrollTop, 40)
  // A quiet read that failed keeps the old lines; one that came puts the new ones in.
  p.asked[2].failed('down'); assert.match(p.text(), /old b/); assert.doesNotMatch(p.text(), /down/)
  p.view.reload('b'); p.asked[3].done(patchOf('b', 'new b')); assert.match(p.text(), /new b/); assert.doesNotMatch(p.text(), /old b/)
  p.dom.window.close()
})
test('the patch queue runs two at a time, in the order asked, and skips what is no longer wanted', async () => {
  const queue = patchQueue(2), started = [], finish = {}
  let alive = new Set(['a', 'b', 'c', 'd'])
  for (const name of ['a', 'b', 'c', 'd']) queue.add({ live: () => alive.has(name), run: () => { started.push(name); return new Promise(resolve => { finish[name] = resolve }) } })
  assert.deepEqual(started, ['a', 'b']); assert.equal(queue.running(), 2)
  alive.delete('c'); finish.a(); await wait(0)
  assert.deepEqual(started, ['a', 'b', 'd'])
  queue.add({ live: () => true, run: () => { started.push('e'); return Promise.resolve() } }); queue.clear()
  finish.b(); finish.d(); await wait(0)
  assert.deepEqual(started, ['a', 'b', 'd'])
})

test('a file head\'s chip shows the server\'s letter, in its colour class, and nothing the server did not give (moai-dy7a.eol)', () => {
  const dom = new JSDOM('<body></body>'); const doc = dom.window.document
  const view = createDiff(doc, words); doc.body.append(view.el)
  const head = (path, status, mark) => ({ path, old_path: null, status, ...(mark === undefined ? {} : { mark }), adds: 1, dels: 0, binary: false })
  view.show({ files: [head('a', 'modified', 'M'), head('b', 'untracked', 'A'), head('c', 'deleted', 'D'), head('d', 'renamed', 'R'), head('e', 'untracked'), head('f', 'modified', '<i>M</i>'), head('g', 'modified', 'Q')], truncated: false })
  const chips = [...doc.querySelectorAll('.m-diff-chip')]
  assert.deepEqual(chips.map(c => c.textContent), ['M', 'A', 'D', 'R', '', '', ''], 'no letter is made from the status word: an untracked file with no mark is not "?"')
  assert.deepEqual(chips.map(c => c.className), ['m-diff-chip m-mark-M', 'm-diff-chip m-mark-A', 'm-diff-chip m-mark-D', 'm-diff-chip m-mark-R', 'm-diff-chip', 'm-diff-chip', 'm-diff-chip'])
  assert.equal(doc.querySelector('.m-diff-head i'), null)
  dom.window.close()
})
test('the page source holds no status-to-letter table', async () => {
  const { readFileSync, readdirSync } = await import('node:fs')
  const dir = new URL('../panel/', import.meta.url)
  const sources = ['view', 'data'].flatMap(d => readdirSync(new URL(`${d}/`, dir)).filter(f => f.endsWith('.ts')).map(f => readFileSync(new URL(`${d}/${f}`, dir), 'utf8')))
  for (const text of sources) { assert.doesNotMatch(text, /statusMark/); assert.doesNotMatch(text, /untracked:\s*'/); assert.doesNotMatch(text, /type_changed:\s*'/) }
})

// Epic-end review fixes (moai-dy7a).
test('the terminal line keeps a path that starts with a dash as the flag\'s value', () => {
  const args = execFileSync('sh', ['-c', `set -- ${terminalLine('moai-1', 'working', '-x.rs').slice('moai '.length)}; printf '%s\\n' "$@"`], { encoding: 'utf8' }).split('\n').slice(0, -1)
  assert.deepEqual(args, ['diff', 'moai-1', '--scope', 'working', '--path=-x.rs'])
})
test('a refresh that lands on a pane still reading opens the first file, as a first read does', () => {
  const p = pane()
  p.view.reading()
  p.view.list({ files: [head('a'), head('b')], truncated: false }, true)
  assert.equal(p.doc.querySelector('details[data-path="a"]').open, true)
  assert.deepEqual(p.asked.map(t => t.path), ['a'])
  p.dom.window.close()
})
test('a refresh that leaves a file with no line changed shows it so, not its old lines', () => {
  const p = pane()
  p.view.list({ files: [head('a', 2, 1)], truncated: false })
  p.asked[0].done(patchOf('a', 'old line')); assert.match(p.text(), /old line/)
  p.view.list({ files: [head('a', 0, 0)], truncated: false }, true)
  assert.doesNotMatch(p.text(), /old line/); assert.match(p.text(), /Empty/)
  assert.equal(p.asked.length, 1, 'nothing to ask for a head that says no line changed')
  p.dom.window.close()
})
test('a pane that moves on stops drawing the large patch it had under way', async () => {
  const p = pane()
  p.view.list({ files: [head('a', 1000, 0)], truncated: false })
  const lines = Array.from({ length: 1000 }, (_, n) => line('add', `row ${n}`))
  p.asked[0].done({ files: [{ ...head('a', 1000, 0), hunks: [{ old_start: 0, old_len: 0, new_start: 1, new_len: 1000, section: '', lines }] }], truncated: false, highlight: 'full' })
  const details = p.doc.querySelector('details[data-path="a"]')
  const drawn = details.querySelectorAll('.m-diff-line').length
  assert.ok(drawn > 0 && drawn < 1000, `drawn ${drawn} rows in the first turn`)
  p.view.clear(); await wait(20)
  assert.equal(details.querySelectorAll('.m-diff-line').length, drawn, 'the dropped file drew no more rows')
  p.dom.window.close()
})
