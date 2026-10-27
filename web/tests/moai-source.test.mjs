import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))

const { archiveDue, ENDPOINT, bootFrom, groupsFrom, loadAllRows, loadArchived, loadBoot, loadDetail, loadGroups, loadMatches, loadTree, loadWikiListing, loadWikiPage, markArchived, matchEndpoints, rowsFromLists, sameGroups, withArchive } =
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

// Real answers of this binary on the fixture tracker (scripts/gen-web-fixtures.sh).
const SHOW_ALL = await fixture('rows.json')
const BACKLOG = await fixture('backlog.json')
const ARCHIVED = await fixture('rows-archived.json')
const TREE = await fixture('tree.json')
const STATUS = await fixture('status.json')
const WIKI_LS = await fixture('wiki-ls.json')
const WIKI_SHOW = await fixture('wiki-show.json')

test('all rows are /api/rows and /api/backlog, read at once, backlog rows marked so', async () => {
  const client = fakeClient({ '/api/rows': passed(SHOW_ALL), '/api/backlog': passed(BACKLOG) })
  const loaded = await loadAllRows(client, 2, 4)
  assert.equal(loaded.ok, true)
  assert.deepEqual(loaded.value.rows.map((row) => row.id), [...SHOW_ALL.map((row) => row.id), 'fx-th01'])
  assert.equal(loaded.value.rows.at(-1).kind, 'backlog')
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
  assert.equal(loaded.value.find((row) => row.id === 'fx-is02').archived, true, 'moai marks its archive rows itself')
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

test('status epics[] and milestones[] give their counts per column; malformed entries are skipped', async () => {
  const groups = groupsFrom(STATUS)
  assert.deepEqual(groups.get('fx-ep01'), { done: 0, in_progress: 1, review: 1, todo: 1 })
  assert.deepEqual(groups.get('fx-ms01'), { done: 0, in_progress: 1, review: 1, todo: 1 })
  assert.equal(groupsFrom({ epics: [{ id: 'e', counts: 'x' }, { id: 5, counts: {} }, null] }).size, 0)
  assert.equal(groupsFrom('x').size, 0)
  assert.equal(sameGroups(groups, groupsFrom(STATUS)), true)
  assert.equal(sameGroups(groups, new Map()), false)
  const loaded = await loadGroups(fakeClient({ '/api/status': passed(STATUS) }))
  assert.deepEqual(loaded.value.get('fx-ep02'), { done: 0, in_progress: 0, review: 0, todo: 1 })
  // Overlay on: the counts are asked over the same worktrees as the tree (moai-m4ez.u6a).
  const overlaid = fakeClient({ '/api/status?worktree=1': passed(STATUS) })
  assert.equal((await loadGroups(overlaid, 0, undefined, true)).ok, true)
  assert.equal(overlaid.calls[0].url, '/api/status?worktree=1')
  assert.deepEqual(await loadGroups(fakeClient({ '/api/status': { kind: 'not-started' } })), { ok: false, outcome: { kind: 'not-started' } })
})

test('the tree is /api/tree with the order asked for; an answer that is not a list is unreadable', async () => {
  const client = fakeClient({ '/api/tree?reverse=1&sort=updated%3Adesc': passed(TREE), '/api/tree': passed({}) })
  assert.deepEqual((await loadTree(client, { sort: 'updated:desc', reverse: '1' })).value, TREE)
  assert.deepEqual(await loadTree(client), { ok: false, outcome: { kind: 'unreadable' } })
})

test('the tree asks for the archive only while archived rows are shown (moai-m4ez.4zq)', async () => {
  const client = fakeClient({ '/api/tree': passed(TREE), '/api/tree?archived=1': passed(TREE), '/api/tree?archived=1&worktree=1': passed(TREE) })
  assert.deepEqual((await loadTree(client)).value, TREE)
  assert.deepEqual((await loadTree(client, {}, undefined, undefined, false, true)).value, TREE)
  assert.deepEqual((await loadTree(client, {}, undefined, undefined, true, true)).value, TREE)
  assert.deepEqual(client.calls.map((call) => call.url), ['/api/tree', '/api/tree?archived=1', '/api/tree?archived=1&worktree=1'])
})

test('the tree is asked in the zone the page draws in, so moai judges overdue there (moai-vj8w.7sh)', async () => {
  const client = fakeClient({ '/api/tree': passed(TREE), '/api/tree?gantt=1&tz=Asia%2FSeoul': passed(TREE), '/api/tree?archived=1&gantt=1&tz=UTC&worktree=1': passed(TREE) })
  assert.deepEqual((await loadTree(client)).value, TREE)
  assert.deepEqual((await loadTree(client, {}, undefined, undefined, false, false, 'Asia/Seoul', true)).value, TREE)
  assert.deepEqual((await loadTree(client, {}, undefined, undefined, true, true, 'UTC', true)).value, TREE)
  assert.deepEqual(client.calls.map((call) => call.url), ['/api/tree', '/api/tree?gantt=1&tz=Asia%2FSeoul', '/api/tree?archived=1&gantt=1&tz=UTC&worktree=1'])
})

test('the detail of a row only another worktree holds is asked with worktree=1 while the overlay is on (moai-m4ez.yb1)', async () => {
  const client = fakeClient({ '/api/issue/a.b': passed({ id: 'a.b' }), '/api/issue/a.b?worktree=1': passed({ id: 'a.b', branch: 'feat/x' }) })
  assert.deepEqual(await loadDetail(client, 'a.b'), { ok: true, value: { id: 'a.b' } })
  assert.deepEqual(await loadDetail(client, 'a.b', { worktree: true }), { ok: true, value: { id: 'a.b', branch: 'feat/x' } })
  assert.deepEqual(await loadDetail(client, 'a.b', { worktree: false }), { ok: true, value: { id: 'a.b' } })
  assert.equal(urlOf(ENDPOINT.issue('a/b', true)), '/api/issue/a%2Fb?worktree=1')
})

test('with the overlay on the tree is asked with worktree=1, so a row only another worktree holds is placed (moai-m4ez.gcp)', async () => {
  const client = fakeClient({ '/api/tree?reverse=1&sort=updated%3Adesc&worktree=1': passed(TREE), '/api/tree?worktree=1': passed(TREE) })
  assert.deepEqual((await loadTree(client, { sort: 'updated:desc', reverse: '1' }, undefined, undefined, true)).value, TREE)
  assert.deepEqual((await loadTree(client, {}, undefined, undefined, true)).value, TREE)
})

test('what a filter and a search find is both lists with fields=id; moai\'s refusal is the outcome', async () => {
  const todo = await fixture('match-todo.json')
  const client = fakeClient({
    '/api/rows?fields=id&filter=status%3Dtodo': passed(todo),
    '/api/backlog?fields=id&filter=status%3Dtodo': passed(await fixture('match-todo-backlog.json')),
    '/api/rows?archived=1&fields=id&q=parser': passed(await fixture('match-grep.json')),
    '/api/backlog?archived=1&fields=id&q=parser': passed([]),
    '/api/rows?fields=id&filter=nope%3D1': { kind: 'moai-error', message: 'unknown filter key nope', code: 'bad_filter' },
    '/api/backlog?fields=id&filter=nope%3D1': { kind: 'moai-error', message: 'unknown filter key nope', code: 'bad_filter' },
  })
  const found = await loadMatches(client, { filter: 'status=todo', q: '', archived: false })
  assert.deepEqual([...found.value].sort(), [...todo.map((row) => row.id), 'fx-th01'].sort())
  assert.deepEqual([...(await loadMatches(client, { filter: '', q: ' parser ', archived: true })).value], ['fx-is01'], 'a word only a note holds')
  assert.deepEqual(await loadMatches(client, { filter: 'nope=1', q: '', archived: false }), { ok: false, outcome: { kind: 'moai-error', message: 'unknown filter key nope', code: 'bad_filter' } })
})

test('a type= filter asks /api/rows alone: backlog show refuses a second type, and show answers every kind', async () => {
  // This binary's answers (gen-web-fixtures.sh tracker): `backlog show --filter=type=epic` is refused with
  // `show_kind_typed`, and `show --all --filter=type=backlog` prints the backlog's own row.
  const refused = { kind: 'moai-error', message: '`backlog` here already picks the type — `type=epic` cannot pick it again.', code: 'bad_filter' }
  const client = fakeClient({
    '/api/rows?fields=id&filter=type%3Depic': passed([{ id: 'fx-ep01' }, { id: 'fx-ep02' }]),
    '/api/backlog?fields=id&filter=type%3Depic': refused,
    '/api/rows?fields=id&filter=type%3Dbacklog+status%3Dtodo': passed([{ id: 'fx-th01' }]),
  })
  assert.deepEqual([...(await loadMatches(client, { filter: 'type=epic', q: '', archived: false })).value], ['fx-ep01', 'fx-ep02'])
  assert.deepEqual([...(await loadMatches(client, { filter: 'type=backlog status=todo', q: '', archived: false })).value], ['fx-th01'])
  assert.deepEqual(client.calls.map((call) => call.url), ['/api/rows?fields=id&filter=type%3Depic', '/api/rows?fields=id&filter=type%3Dbacklog+status%3Dtodo'])
})

test('a detail moai printed whole before ending non-zero (an unreadable line elsewhere) is read, as the lists are', async () => {
  const row = { id: 'fx-is01', title: 'Grid layout' }
  const client = fakeClient({ '/api/issue/fx-is01': { kind: 'failed', output: row, notes: ['unreadable rows 1'] }, '/api/issue/fx-nope': { kind: 'failed' } })
  assert.deepEqual(await loadDetail(client, 'fx-is01'), { ok: true, value: row })
  assert.deepEqual(await loadDetail(client, 'fx-nope'), { ok: false, outcome: { kind: 'failed' } })
})

test('a boot left unanswered past its time limit is the server out of reach', async () => {
  const hanging = (_url, init) => new Promise((_resolve, reject) => init.signal.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))))
  assert.deepEqual(await loadBoot(hanging, 20), { ok: false, outcome: { kind: 'not-started' } })
})

const BOOT = {
  lang: 'ko', version: '0.22.0', project: 'p', root: '/r', statuses: ['todo', 'done'], naming: 'full',
  glyphs: { todo: '·', done: '✓' }, unknown_glyph: '○',
  columns: { backlog: { name: 'backlog', glyph: '◇' }, shelved: { name: '미룸', glyph: '‖' } },
  filter_keys: ['status', 'due_on'], sort_fields: [{ name: 'due', dir: 'asc' }],
}

// What a server without the header's fields gives reads as one that sent them as null (a server before moai-fmun).
const BOOTED = { ...BOOT, user: null, latest: null, timezone: null }

test('the boot answer is read as the server gives it, and anything else is refused', () => {
  assert.deepEqual(bootFrom(BOOT), BOOTED, 'an older server without user and latest still boots')
  assert.deepEqual(bootFrom({ ...BOOT, user: 'Raven (r@x.io)', latest: { state: 'stale', tag: 'v1.0.0', heard_at: '2026-10-20T00:00:00Z', why: 'offline' } }),
    { ...BOOT, user: 'Raven (r@x.io)', latest: { state: 'stale', tag: 'v1.0.0', heardAt: '2026-10-20T00:00:00Z', why: 'offline' }, timezone: null })
  // The TUI's zone (moai-ly88): a name as written, or null — an empty or wrong one is none.
  assert.equal(bootFrom({ ...BOOT, timezone: 'Asia/Seoul' }).timezone, 'Asia/Seoul')
  for (const none of ['', 9, null, undefined]) assert.equal(bootFrom({ ...BOOT, timezone: none }).timezone, null)
  assert.deepEqual(bootFrom({ ...BOOT, user: '', latest: { state: 'later' } }), BOOTED, 'an empty user and an unknown state are nothing')
  assert.deepEqual(bootFrom({ ...BOOT, user: 7, latest: 'v1' }), BOOTED, 'a wrong shape there does not refuse the boot')
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
  assert.deepEqual(await loadBoot(answering(200, JSON.stringify(BOOT))), { ok: true, value: BOOTED })
  assert.deepEqual(await loadBoot(answering(500, JSON.stringify({ error: 'not a moai repository', code: 'not_a_repo' }))), { ok: false, outcome: { kind: 'moai-error', message: 'not a moai repository', code: 'not_a_repo' } })
  assert.deepEqual(await loadBoot(answering(200, '{')), { ok: false, outcome: { kind: 'unreadable' } })
  assert.deepEqual(await loadBoot(async () => { throw new TypeError('Failed to fetch') }), { ok: false, outcome: { kind: 'not-started' } })
})

test('a filter or a search is asked over the worktree overlay while it is on — /api/rows only, as the list is read', () => {
  const over = matchEndpoints({ filter: 'status=todo', q: '', archived: false, worktree: true })
  assert.deepEqual(over.map((e) => [e.path, e.query]), [
    ['/api/rows', { fields: 'id', filter: 'status=todo', worktree: '1' }],
    ['/api/backlog', { fields: 'id', filter: 'status=todo' }],
  ])
  assert.deepEqual(matchEndpoints({ filter: 'status=todo', q: '', archived: false }).map((e) => e.query), [{ fields: 'id', filter: 'status=todo' }, { fields: 'id', filter: 'status=todo' }])
})

test('loadAllRows asks /api/rows for the worktree overlay while it is on, and the backlog never', async () => {
  const asked = []
  const client = { request: async (endpoint) => { asked.push(endpoint); return { kind: 'passed', output: [] } } }
  await loadAllRows(client, undefined, undefined, true)
  assert.deepEqual(asked.map((e) => [e.path, e.query]), [['/api/rows', { worktree: '1' }], ['/api/backlog', {}]])
  asked.length = 0
  await loadAllRows(client)
  assert.deepEqual(asked.map((e) => [e.path, e.query]), [['/api/rows', {}], ['/api/backlog', {}]])
})

test('the archive is read after a board read only when that read asked for its lines in the tree (moai-m4ez.u6a)', () => {
  assert.equal(archiveDue(true, true, false, false), true)
  // `SPC v o` turned on while a read asked without the archive ran: its tree lacks the lines, the follow-up read loads it.
  assert.equal(archiveDue(false, true, false, false), false)
  assert.equal(archiveDue(true, false, false, false), false)
  assert.equal(archiveDue(true, true, true, false), false)
  assert.equal(archiveDue(true, true, false, true), false)
})

test('filter days use the page zone for both rows and backlog, without changing plain listing reads', () => {
  const question = { filter: 'created=2026-10-11', q: '', archived: false }
  for (const endpoint of matchEndpoints({ ...question, zone: 'Asia/Seoul' })) assert.equal(endpoint.query.tz, 'Asia/Seoul')
  for (const endpoint of matchEndpoints(question)) assert.equal(endpoint.query.tz, undefined)
})

test('a tree-only read omits both chart values and timezone; a chart read explicitly asks for them', async () => {
  const client = fakeClient({ '/api/tree': passed(TREE), '/api/tree?gantt=1': passed(TREE) })
  await loadTree(client, {}, undefined, undefined, false, false, 'Asia/Seoul')
  await loadTree(client, {}, undefined, undefined, false, false, undefined, true)
  assert.deepEqual(client.calls.map((c) => c.url), ['/api/tree', '/api/tree?gantt=1'])
})
