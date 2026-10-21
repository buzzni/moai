import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))

const { ENDPOINT, bootFrom, loadAllRows, loadArchived, loadBoot, loadDetail, loadTallies, loadWikiListing, loadWikiPage, markArchived, rowsFromLists, talliesFrom, withArchive } =
  await panelModule('data/source.ts')
const { urlOf } = await panelModule('data/client.ts')

/** A client that answers each request from `answers[url]` (an outcome, or a function of the endpoint). */
function fakeClient(answers) {
  const calls = []
  return {
    calls,
    cancel() {},
    async request(endpoint, options) {
      const url = urlOf(endpoint)
      calls.push({ url, priority: options?.priority, epoch: options?.epoch })
      const answer = answers[url]
      if (answer === undefined) throw new Error(`unexpected ${url}`)
      return typeof answer === 'function' ? answer(endpoint) : answer
    },
  }
}
const passed = (output, notes) => (notes ? { kind: 'passed', output, notes } : { kind: 'passed', output })

const SHOW_ALL = await fixture('show-all-v016.json')
const BACKLOG = await fixture('backlog-v016.json')
const ARCHIVED = await fixture('show-archived-v016.json')
const WIKI_LS = await fixture('wiki-ls-v016.json')
const WIKI_SHOW = await fixture('wiki-show-v016.json')

test('all rows are /api/rows and /api/backlog, read at once, backlog rows marked so', async () => {
  const client = fakeClient({ '/api/rows': passed(SHOW_ALL), '/api/backlog': passed(BACKLOG) })
  const loaded = await loadAllRows(client, 2, 4)
  assert.equal(loaded.ok, true)
  assert.deepEqual(loaded.value.rows.map((row) => [row.id, row.kind]), [
    ['tracker-0i7e', 'issue'], ['tracker-38yh', 'issue'], ['tracker-4j44', 'issue'], ['tracker-e0ue', 'milestone'], ['tracker-tdv4', 'epic'],
    ['tracker-m21g', 'backlog'], ['tracker-zwyp', 'backlog'],
  ])
  assert.equal(loaded.value.notes, undefined)
  assert.deepEqual(client.calls, [{ url: '/api/rows', priority: 2, epoch: 4 }, { url: '/api/backlog', priority: 2, epoch: 4 }])
})

test('what moai said beside either list rides the rows as notes', async () => {
  const client = fakeClient({ '/api/rows': passed([], ['line 3 skipped']), '/api/backlog': passed([], ['line 9 skipped']) })
  assert.deepEqual((await loadAllRows(client)).value.notes, ['line 3 skipped', 'line 9 skipped'])
})

test('a list that gave no answer is the load failure, the rows first', async () => {
  const refused = { kind: 'moai-error', message: 'not a moai repository' }
  assert.deepEqual(await loadAllRows(fakeClient({ '/api/rows': refused, '/api/backlog': passed([]) })), { ok: false, outcome: refused })
  assert.deepEqual(await loadAllRows(fakeClient({ '/api/rows': passed([]), '/api/backlog': { kind: 'not-started' } })), { ok: false, outcome: { kind: 'not-started' } })
  assert.deepEqual(await loadAllRows(fakeClient({ '/api/rows': passed({ not: 'a list' }), '/api/backlog': passed([]) })), { ok: false, outcome: { kind: 'unreadable' } })
})

test('a partial answer (non-zero exit, whole JSON printed) is read like a whole one', async () => {
  const client = fakeClient({ '/api/rows': { kind: 'failed', output: SHOW_ALL }, '/api/backlog': passed([]) })
  assert.equal((await loadAllRows(client)).value.rows.length, SHOW_ALL.length)
})

test('twins are kept for the index to decide, and backlog rows are kind backlog whatever they say', () => {
  const loaded = rowsFromLists([{ id: 'a' }, { id: 'a', title: 'twin' }], [{ id: 'b', kind: 'issue' }, { id: 'a' }, { title: 'no id' }])
  assert.deepEqual(loaded.value.rows, [{ id: 'a' }, { id: 'a', title: 'twin' }, { id: 'b', kind: 'backlog' }])
})

test('the archive is both lists once with archived=1, at background priority', async () => {
  const client = fakeClient({ '/api/rows?archived=1': passed(ARCHIVED), '/api/backlog?archived=1': passed([{ id: 'th-1' }]) })
  const loaded = await loadArchived(client)
  assert.deepEqual(loaded.value.map((row) => row.id), [...ARCHIVED.map((row) => row.id), 'th-1'])
  assert.ok(client.calls.every((call) => call.priority === 3))
  const failing = fakeClient({ '/api/rows?archived=1': { kind: 'moai-error', message: 'x' }, '/api/backlog?archived=1': passed([]) })
  assert.deepEqual(await loadArchived(failing), { ok: false, outcome: { kind: 'moai-error', message: 'x' } })
})

test('archive rows the list does not hold are tagged archived; one moai marked passes as it is', () => {
  const listed = { rows: [{ id: 'a' }] }
  assert.deepEqual(withArchive(listed, null), listed.rows)
  assert.deepEqual(withArchive(listed, [{ id: 'a' }, { id: 'b' }, { id: 'c', archived: true }]), [{ id: 'a' }, { id: 'b', archived: true }, { id: 'c', archived: true }])
  const marked = { id: 'c', archived: true }
  assert.equal(markArchived([marked], new Set())[0], marked, 'never tagged twice')
})

test('the wiki listing and pages come back as Moai printed them, at wiki priority, each slug part encoded', async () => {
  const client = fakeClient({
    '/api/wiki': passed(WIKI_LS),
    '/api/wiki/home': passed(WIKI_SHOW),
    '/api/wiki/guide/%ED%95%9C%20%EA%B8%80': passed({ slug: 'guide/한 글' }),
    '/api/wiki/nope': { kind: 'moai-error', message: 'nope not found' },
  })
  assert.deepEqual(await loadWikiListing(client), { ok: true, value: WIKI_LS })
  assert.deepEqual(await loadWikiPage(client, 'home'), { ok: true, value: WIKI_SHOW })
  assert.deepEqual(await loadWikiPage(client, 'guide/한 글'), { ok: true, value: { slug: 'guide/한 글' } })
  assert.deepEqual(await loadWikiPage(client, 'nope'), { ok: false, outcome: { kind: 'moai-error', message: 'nope not found' } })
  assert.ok(client.calls.every((call) => call.priority === 1))
})

test('the detail is /api/issue/<id> at detail priority; an answer without an id is unreadable', async () => {
  const client = fakeClient({ '/api/issue/a.b': passed({ id: 'a.b' }), '/api/issue/x': passed([]) })
  assert.deepEqual(await loadDetail(client, 'a.b', { epoch: 2 }), { ok: true, value: { id: 'a.b' } })
  assert.deepEqual(await loadDetail(client, 'x'), { ok: false, outcome: { kind: 'unreadable' } })
  assert.deepEqual(client.calls[0], { url: '/api/issue/a.b', priority: 0, epoch: 2 })
  assert.equal(urlOf(ENDPOINT.issue('a/b')), '/api/issue/a%2Fb')
})

test('status epics[] and milestones[] become [done, total] per id; malformed entries are skipped (se-tcd4)', async () => {
  const status = { counts: { done: 2 }, milestones: [{ id: 'm1', done: 4, total: 7 }, { id: 5, done: 1, total: 1 }], epics: [{ id: 'e1', done: 2, total: 3 }, { id: 'e2', done: '2', total: 3 }, null, { id: 'e3', total: 3 }] }
  assert.deepEqual([...talliesFrom(status)], [['m1', [4, 7]], ['e1', [2, 3]]])
  assert.equal(talliesFrom('x').size, 0)
  const loaded = await loadTallies(fakeClient({ '/api/status': passed(status) }))
  assert.deepEqual([...loaded.value], [['m1', [4, 7]], ['e1', [2, 3]]])
  assert.deepEqual(await loadTallies(fakeClient({ '/api/status': { kind: 'not-started' } })), { ok: false, outcome: { kind: 'not-started' } })
})

const BOOT = {
  lang: 'ko', version: '0.22.0', project: 'p', root: '/r', statuses: ['todo', 'done'], naming: 'full',
  glyphs: { todo: '·', done: '✓' }, unknown_glyph: '○',
  columns: { backlog: { name: 'backlog', glyph: '◇' }, shelved: { name: '미룸', glyph: '‖' } },
  filter_keys: ['status', 'due_on'], sort_fields: [{ name: 'due', dir: 'asc' }],
}

test('the boot answer is read as the server gives it, and anything else is refused', () => {
  assert.deepEqual(bootFrom(BOOT), BOOT)
  assert.equal(bootFrom({ ...BOOT, glyphs: { todo: 1 } }), null)
  assert.equal(bootFrom({ ...BOOT, columns: { backlog: '◇', shelved: '‖' } }), null, 'the shape before moai-jshy.yxh')
  assert.equal(bootFrom({ ...BOOT, sort_fields: [{ name: 'due', dir: 'up' }] }), null)
  assert.equal(bootFrom([]), null)
})

test('loadBoot fetches /api/boot outside the queue; a refusal, a lost server and a bad answer are outcomes', async () => {
  const answering = (status, body) => async (url) => {
    assert.equal(url, '/api/boot')
    return { status, text: async () => body }
  }
  assert.deepEqual(await loadBoot(answering(200, JSON.stringify(BOOT))), { ok: true, value: BOOT })
  assert.deepEqual(await loadBoot(answering(500, JSON.stringify({ error: 'not a moai repository', code: 'not_a_repo' }))), { ok: false, outcome: { kind: 'moai-error', message: 'not a moai repository', code: 'not_a_repo' } })
  assert.deepEqual(await loadBoot(answering(200, '{')), { ok: false, outcome: { kind: 'unreadable' } })
  assert.deepEqual(await loadBoot(async () => { throw new TypeError('Failed to fetch') }), { ok: false, outcome: { kind: 'not-started' } })
})
