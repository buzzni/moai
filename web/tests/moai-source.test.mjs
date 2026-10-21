import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const root = new URL('..', import.meta.url).pathname
const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))

const { PROFILE, ARCHIVE_PAGE, loadAllRows, loadArchived, loadRows, loadStatuses, loadTallies, markArchived, activeIds, withArchive, talliesFrom, loadWikiListing, loadWikiPage, rowsFromLists, statusesFrom } =
  await panelModule('data/source.ts')
const { IssueIndex, LOST } = await panelModule('data/index.ts')
const { CompatProbe } = await panelModule('data/compat.ts')

/** A client that answers each request from `answers[profileId]` (an outcome, or a function of the value). */
function fakeClient(answers) {
  const calls = []
  const cancels = []
  return {
    calls,
    cancels,
    cancel(key) {
      cancels.push(key)
    },
    async request(profileId, value, options) {
      calls.push({ profileId, value, priority: options?.priority })
      const answer = answers[profileId]
      if (answer === undefined) throw new Error(`unexpected ${profileId}`)
      return typeof answer === 'function' ? answer(value) : answer
    },
  }
}
const passed = (output) => ({ kind: 'passed', output })

// Real Moai 0.16.0 output from a throwaway tracker (see the report of se-89fu for how it was made).
const SHOW_ALL = await fixture('show-all-v016.json')
const BACKLOG = await fixture('backlog-v016.json')
const STATUS = await fixture('status-v016.json')
const ARCHIVED = await fixture('show-archived-v016.json')
const WIKI_LS = await fixture('wiki-ls-v016.json')
const WIKI_SHOW = await fixture('wiki-show-v016.json')
// Real Moai v0.8.0 output (as tests/moai.test.mjs reads it).
const PRIME = await fixture('prime-v080.json')
// Real moai 0.17.0 output from a scratch tracker whose config adds a `blocked` column.
const STATUS_V017 = await fixture('status-v017.json')

test('all rows are show --all and the backlog, read at once, backlog rows marked so', async () => {
  const client = fakeClient({ [PROFILE.showAll]: passed(SHOW_ALL), [PROFILE.backlog]: passed(BACKLOG) })
  const loaded = await loadAllRows(client)
  assert.equal(loaded.ok, true)
  assert.equal(loaded.value.truncated, false)
  assert.equal(loaded.value.more, 0)
  assert.deepEqual(loaded.value.rows.map((row) => [row.id, row.kind]), [
    ['tracker-0i7e', 'issue'], ['tracker-38yh', 'issue'], ['tracker-4j44', 'issue'], ['tracker-e0ue', 'milestone'], ['tracker-tdv4', 'epic'],
    ['tracker-m21g', 'backlog'], ['tracker-zwyp', 'backlog'],
  ])
  // Done rows and the groups come along, unlike prime.
  assert.ok(loaded.value.rows.some((row) => row.status === 'done'))
  assert.equal(loaded.value.rows[0].epic, 'tracker-tdv4')
  assert.deepEqual(client.calls.map((call) => call.profileId), [PROFILE.showAll, PROFILE.backlog])
  assert.deepEqual(client.calls.map((call) => call.priority), [2, 2])
})

test('a cut show --all falls back to prime with truncated, keeping the backlog', async () => {
  const client = fakeClient({
    [PROFILE.showAll]: { kind: 'unreadable', truncated: true },
    [PROFILE.backlog]: passed(BACKLOG),
    [PROFILE.prime]: passed(PRIME),
  })
  const loaded = await loadAllRows(client)
  assert.equal(loaded.ok, true)
  assert.equal(loaded.value.truncated, true)
  assert.equal(loaded.value.partial, true)
  const primeIds = [...PRIME.picked, ...PRIME.others, ...PRIME.ready].map((row) => row.id)
  assert.deepEqual(loaded.value.rows.map((row) => row.id), [...new Set(primeIds), 'tracker-m21g', 'tracker-zwyp'])
  assert.equal(loaded.value.more, (PRIME.rest || 0) + (PRIME.others_rest || 0))
  // A cut answer that still parsed is whole enough: no fallback.
  const parsed = fakeClient({ [PROFILE.showAll]: { ...passed(SHOW_ALL), truncated: true }, [PROFILE.backlog]: passed([]) })
  assert.equal((await loadAllRows(parsed)).value.truncated, false)
})

test('in the cut fallback the backlog rows lose their references to the groups prime does not load', async () => {
  // Real backlog rows name an epic or a milestone, or inherit an epic from their id parent (`derived_epic`).
  const thoughts = [
    { id: 'b-1', kind: 'backlog', status: 'todo', epic: 'e-1', derived_epic: 'e-1' },
    { id: 'b-2', kind: 'backlog', status: 'todo', milestone: 'm-1' },
    { id: 'b-3', kind: 'backlog', status: 'todo' },
  ]
  const client = fakeClient({ [PROFILE.showAll]: { kind: 'unreadable', truncated: true }, [PROFILE.backlog]: passed(thoughts), [PROFILE.prime]: passed(PRIME) })
  const loaded = await loadAllRows(client)
  const index = new IssueIndex(loaded.value.rows)
  for (const id of ['b-1', 'b-2', 'b-3']) assert.notDeepEqual(index.homeOf(index.find(id)), [LOST], id)
  assert.ok(loaded.value.rows.every((row) => row.epic === undefined && row.derived_epic === undefined && row.milestone === undefined))
})

test('a read the catalog lacks degrades the load instead of failing it: prime stands in for show --all, the backlog goes', async () => {
  const lacked = { kind: 'refused', code: 'unsupported-daemon' }
  const primeIds = [...new Set([...PRIME.picked, ...PRIME.others, ...PRIME.ready].map((row) => row.id))]
  // No show --all: prime's rows and the backlog, partial but not cut (the catalog line says why, not the cut banner).
  // prime starts beside the backlog read, not after it, and nothing is called off.
  const noAll = fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: passed(BACKLOG), [PROFILE.prime]: passed(PRIME) })
  const loaded = await loadAllRows(noAll)
  assert.equal(loaded.ok, true)
  assert.deepEqual([loaded.value.truncated, loaded.value.partial], [false, true])
  assert.deepEqual(loaded.value.rows.map((row) => row.id), [...primeIds, 'tracker-m21g', 'tracker-zwyp'])
  assert.deepEqual(noAll.calls.map((call) => call.profileId), [PROFILE.showAll, PROFILE.backlog, PROFILE.prime])
  assert.deepEqual(noAll.cancels, [])
  // An older host's thrown start naming the code reads as the refusal does.
  const thrown = await loadAllRows(fakeClient({ [PROFILE.showAll]: { kind: 'not-started', code: 'unsupported-daemon' }, [PROFILE.backlog]: passed(BACKLOG), [PROFILE.prime]: passed(PRIME) }))
  assert.equal(thrown.value.partial, true)
  // No backlog read: the whole list without the backlog, whole (not partial).
  const noBacklog = await loadAllRows(fakeClient({ [PROFILE.showAll]: passed(SHOW_ALL), [PROFILE.backlog]: lacked }))
  assert.equal(noBacklog.ok, true)
  assert.deepEqual([noBacklog.value.truncated, noBacklog.value.partial], [false, false])
  assert.deepEqual(noBacklog.value.rows.map((row) => row.id), SHOW_ALL.map((row) => row.id))
  assert.deepEqual(noBacklog.value.backlogProblem, lacked, 'carried for the shell to tell while the catalog is undecided')
  // Neither: prime alone.
  const neither = await loadAllRows(fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: lacked, [PROFILE.prime]: passed(PRIME) }))
  assert.deepEqual(neither.value.rows.map((row) => row.id), primeIds)
  assert.equal(neither.value.partial, true)
  // A daemon too old for any machine run refuses prime too: that refusal is the load's failure.
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: lacked, [PROFILE.prime]: lacked })), { ok: false, outcome: lacked })
  // Only the catalog's lack degrades: another backlog refusal still fails the load.
  const declined = { kind: 'refused', code: 'declined' }
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: passed(SHOW_ALL), [PROFILE.backlog]: declined })), { ok: false, outcome: declined })
})

test('beside prime a backlog read that gave nothing does not fail the load, and says why in backlogProblem', async () => {
  const lacked = { kind: 'refused', code: 'unsupported-daemon' }
  const declined = { kind: 'refused', code: 'declined' }
  const primeIds = [...new Set([...PRIME.picked, ...PRIME.others, ...PRIME.ready].map((row) => row.id))]
  for (const showAll of [lacked, { kind: 'unreadable', truncated: true }]) {
    for (const problem of [declined, { kind: 'failed' }]) {
      const loaded = await loadAllRows(fakeClient({ [PROFILE.showAll]: showAll, [PROFILE.backlog]: problem, [PROFILE.prime]: passed(PRIME) }))
      assert.equal(loaded.ok, true)
      assert.deepEqual(loaded.value.rows.map((row) => row.id), primeIds)
      assert.deepEqual(loaded.value.backlogProblem, problem)
    }
    // The catalog lacking it is carried too: the shell leaves it to the compat banner once that names it. An answer is no problem.
    const none = await loadAllRows(fakeClient({ [PROFILE.showAll]: showAll, [PROFILE.backlog]: lacked, [PROFILE.prime]: passed(PRIME) }))
    assert.deepEqual(none.value.backlogProblem, lacked)
    const read = await loadAllRows(fakeClient({ [PROFILE.showAll]: showAll, [PROFILE.backlog]: passed(BACKLOG), [PROFILE.prime]: passed(PRIME) }))
    assert.equal(read.value.backlogProblem, undefined)
    // An answer that is not the list of rows gives nothing either, as the whole list's read finds it unreadable.
    const odd = await loadAllRows(fakeClient({ [PROFILE.showAll]: showAll, [PROFILE.backlog]: passed({ rows: [] }), [PROFILE.prime]: passed(PRIME) }))
    assert.deepEqual(odd.value.rows.map((row) => row.id), primeIds)
    assert.deepEqual(odd.value.backlogProblem, { kind: 'unreadable' })
  }
})

test('in the fallback prime starts while the backlog read still runs, and a prime that fails calls that read off at once', async () => {
  const lacked = { kind: 'refused', code: 'unsupported-daemon' }
  const tick = () => new Promise((resolve) => setImmediate(resolve))
  let answer
  const client = fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: () => new Promise((resolve) => { answer = resolve }), [PROFILE.prime]: passed(PRIME) })
  const loading = loadAllRows(client)
  await tick()
  assert.deepEqual(client.calls.map((call) => call.profileId), [PROFILE.showAll, PROFILE.backlog, PROFILE.prime], 'prime is asked for before the backlog answers')
  answer(passed(BACKLOG))
  assert.deepEqual((await loading).value.rows.slice(-2).map((row) => row.id), ['tracker-m21g', 'tracker-zwyp'])
  // prime declined while the backlog read still waits on its confirmation: the load ends on prime's answer without
  // waiting, and the backlog read is called off; what it ends with still reaches compat.
  const declined = { kind: 'refused', code: 'declined' }
  const probe = new CompatProbe()
  let late
  const waiting = fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: () => new Promise((resolve) => { late = resolve }), [PROFILE.prime]: declined })
  const ended = await Promise.race([loadAllRows(waiting, undefined, probe), new Promise((resolve) => setTimeout(() => resolve('still waiting'), 200))])
  assert.deepEqual(ended, { ok: false, outcome: declined })
  assert.deepEqual(waiting.cancels, [PROFILE.backlog])
  late(lacked)
  await tick()
  assert.deepEqual(probe.report().degraded, ['all-rows', 'backlog'])
})

test('the probe hears every read of a degraded load, so the catalog line names what is missing', async () => {
  const lacked = { kind: 'refused', code: 'unsupported-daemon' }
  const probe = new CompatProbe()
  await loadAllRows(fakeClient({ [PROFILE.showAll]: lacked, [PROFILE.backlog]: lacked, [PROFILE.prime]: passed(PRIME) }), undefined, probe)
  assert.deepEqual(probe.report().catalog, { state: 'missing', missing: [PROFILE.showAll, PROFILE.backlog] })
  assert.deepEqual(probe.report().degraded, ['all-rows', 'backlog'])
})

test('twins are kept for the index to decide, and backlog rows are kind backlog whatever they say', () => {
  // A merge left two lines for `x`: the later one (an epic) is the one moai reads, and `y` stands under it.
  const all = [{ id: 'x', kind: 'issue', title: 'first' }, { id: 'x', kind: 'epic', title: 'later' }, { id: 'y', kind: 'issue', epic: 'x', derived_epic: 'x' }]
  const loaded = rowsFromLists(all, [{ id: 'b', kind: 'issue' }, { id: 'y', kind: 'backlog' }])
  assert.deepEqual(loaded.value.rows.map((row) => [row.id, row.kind]), [['x', 'issue'], ['x', 'epic'], ['y', 'issue'], ['b', 'backlog']])
  const index = new IssueIndex(loaded.value.rows)
  assert.equal(index.rows[index.find('x')].title, 'later')
  assert.deepEqual(index.homeOf(index.find('y')), ['e:x'])
})

test('a partial answer (non-zero exit, whole JSON printed) is read like a whole one', async () => {
  // One unreadable line in the tracker ends `show --all` and `backlog show` with 1, the readable rows printed;
  // a skipped wiki page does the same to `wiki ls`.
  const partial = (output) => ({ kind: 'failed', output })
  const loaded = await loadAllRows(fakeClient({ [PROFILE.showAll]: partial(SHOW_ALL), [PROFILE.backlog]: partial(BACKLOG) }))
  assert.equal(loaded.ok, true)
  assert.equal(loaded.value.rows.length, SHOW_ALL.length + BACKLOG.length)
  assert.deepEqual(await loadWikiListing(fakeClient({ [PROFILE.wikiLs]: partial(WIKI_LS) })), { ok: true, value: WIKI_LS })
  assert.deepEqual(await loadStatuses(fakeClient({ [PROFILE.status]: partial(STATUS) })), { ok: true, value: ['todo', 'in_progress', 'review', 'done'] })
  // A failure with no answer is still the load's failure.
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: { kind: 'failed' }, [PROFILE.backlog]: passed(BACKLOG) })), { ok: false, outcome: { kind: 'failed' } })
})

test('a failed read of either list is the load failure; a missing board says not-initialized', async () => {
  const missing = { kind: 'not-initialized' }
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: missing, [PROFILE.backlog]: missing })), { ok: false, outcome: missing })
  const busy = { kind: 'refused', code: 'workspace-busy' }
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: passed(SHOW_ALL), [PROFILE.backlog]: busy })), { ok: false, outcome: busy })
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: passed({ not: 'a list' }), [PROFILE.backlog]: passed(BACKLOG) })), { ok: false, outcome: { kind: 'unreadable' } })
  // show --all alone decides a declined load: the backlog read queued behind it is cancelled, not run.
  const declined = { kind: 'refused', code: 'declined' }
  const lone = fakeClient({ [PROFILE.showAll]: declined, [PROFILE.backlog]: passed(BACKLOG) })
  assert.deepEqual(await loadAllRows(lone), { ok: false, outcome: declined })
  assert.deepEqual(lone.calls.map((call) => call.profileId), [PROFILE.showAll, PROFILE.backlog])
  assert.deepEqual(lone.cancels, [PROFILE.backlog])
  // compat still learns what the backlog read ended with when show --all decided the load, and tells its listeners,
  // though the load does not wait for it. The status read ran, so the catalog can be judged.
  const probe = new CompatProbe()
  probe.observe(PROFILE.status, passed(STATUS))
  const told = []
  probe.subscribe(() => told.push(probe.report().catalog))
  const gone = { kind: 'refused', code: 'unsupported-daemon' }
  assert.deepEqual(await loadAllRows(fakeClient({ [PROFILE.showAll]: declined, [PROFILE.backlog]: gone }), undefined, probe), { ok: false, outcome: declined })
  await new Promise((resolve) => setImmediate(resolve))
  assert.deepEqual(probe.report().catalog, { state: 'missing', missing: [PROFILE.backlog] })
  assert.deepEqual(told.at(-1), { state: 'missing', missing: [PROFILE.backlog] }, 'the listeners hear the backlog outcome')
  const whole = fakeClient({ [PROFILE.showAll]: passed(SHOW_ALL), [PROFILE.backlog]: passed(BACKLOG) })
  await loadAllRows(whole)
  assert.deepEqual(whole.cancels, [])
  // The cut fallback reports prime's own failure.
  const cut = fakeClient({ [PROFILE.showAll]: { kind: 'unreadable', truncated: true }, [PROFILE.backlog]: passed(BACKLOG), [PROFILE.prime]: missing })
  assert.deepEqual(await loadAllRows(cut), { ok: false, outcome: missing })
})

test('loadRows still reads prime alone until the panel switches to loadAllRows', async () => {
  const client = fakeClient({ [PROFILE.prime]: passed(PRIME) })
  const loaded = await loadRows(client)
  assert.equal(loaded.ok, true)
  // prime's rows are not every active row (source.ts primeRows), though nothing cut them.
  assert.deepEqual([loaded.value.truncated, loaded.value.partial], [false, true])
  assert.deepEqual(client.calls.map((call) => call.profileId), [PROFILE.prime])
})

test('the columns come from statuses (moai 0.17), else from the counts in the default order', async () => {
  assert.deepEqual(await loadStatuses(fakeClient({ [PROFILE.status]: passed(STATUS_V017) })), { ok: true, value: ['todo', 'in_progress', 'blocked', 'review', 'done'] })
  // Moai 0.16.0 has no statuses: its counts keys come alphabetically.
  assert.equal(STATUS.statuses, undefined)
  assert.deepEqual(statusesFrom(STATUS), { ok: true, value: ['todo', 'in_progress', 'review', 'done'] })
  assert.deepEqual(statusesFrom({ counts: { blocked: 0, done: 0, todo: 1, wontfix: 0 } }), { ok: true, value: ['todo', 'done', 'blocked', 'wontfix'] })
  assert.deepEqual(statusesFrom({ statuses: [], counts: { todo: 0 } }), { ok: true, value: ['todo'] })
  assert.deepEqual(statusesFrom({ total: 0 }), { ok: false, outcome: { kind: 'unreadable' } })
  assert.deepEqual(statusesFrom({ counts: [3, 4] }), { ok: false, outcome: { kind: 'unreadable' } })
  assert.deepEqual(await loadStatuses(fakeClient({ [PROFILE.status]: { kind: 'not-initialized' } })), { ok: false, outcome: { kind: 'not-initialized' } })
  // Outside a `.moai/` repository moai 0.16 `status` exits 0 listing the registered projects instead of counts.
  const glance = { projects: [{ name: 'other', path: '/w/other', state: 'ok' }], problems: [], config: {} }
  assert.deepEqual(await loadStatuses(fakeClient({ [PROFILE.status]: passed(glance) })), { ok: false, outcome: { kind: 'not-initialized' } })
})

/** `count` rows `r-0000`… sorted by id, served 200 a page after the cursor. */
function archive(count) {
  const ids = Array.from({ length: count }, (_, at) => `r-${String(at).padStart(4, '0')}`)
  const page = (after) => {
    const from = after === undefined ? 0 : ids.indexOf(after) + 1
    return passed(ids.slice(from, from + ARCHIVE_PAGE).map((id) => ({ id, status: 'done' })))
  }
  return { [PROFILE.showArchived]: () => page(undefined), [PROFILE.showArchivedAfter]: (after) => page(after) }
}

test('the archive is paged 200 at a time after the last id until a short page', async () => {
  const client = fakeClient(archive(450))
  const pages = []
  const loaded = await loadArchived(client, { onPage: (rows) => pages.push(rows.length) })
  assert.equal(loaded.ok, true)
  assert.equal(loaded.value.length, 450)
  assert.deepEqual(pages, [200, 200, 50])
  assert.deepEqual(client.calls.map((call) => [call.profileId, call.value]), [
    [PROFILE.showArchived, undefined], [PROFILE.showArchivedAfter, 'r-0199'], [PROFILE.showArchivedAfter, 'r-0399'],
  ])
  assert.ok(client.calls.every((call) => call.priority === 3))
  // An exact multiple of 200 ends on an empty page.
  const even = fakeClient(archive(400))
  assert.equal((await loadArchived(even)).value.length, 400)
  assert.equal(even.calls.length, 3)
})

test('a real archive page is one short page; a failed page or a cancel stops the paging', async () => {
  const real = fakeClient({ [PROFILE.showArchived]: passed(ARCHIVED) })
  assert.deepEqual((await loadArchived(real)).value.map((row) => row.id), ARCHIVED.map((row) => row.id))
  const failing = fakeClient({ ...archive(450), [PROFILE.showArchivedAfter]: { kind: 'moai-error', message: 'not found' } })
  const pages = []
  assert.deepEqual(await loadArchived(failing, { onPage: (rows) => pages.push(rows.length) }), { ok: false, outcome: { kind: 'moai-error', message: 'not found' } })
  assert.deepEqual(pages, [200])
  let stop = false
  const cancelling = fakeClient(archive(450))
  const loaded = await loadArchived(cancelling, { onPage: () => (stop = true), cancelled: () => stop })
  assert.deepEqual(loaded, { ok: false, outcome: { kind: 'cancelled', mayContinue: false } })
  assert.equal(cancelling.calls.length, 1)
})

test('the paging also stops on a full page that would not move the cursor on', async () => {
  // A daemon that ignored `--after` would serve the first page again: its last id is the cursor itself.
  const first = archive(450)[PROFILE.showArchived]
  const stuck = fakeClient({ [PROFILE.showArchived]: first, [PROFILE.showArchivedAfter]: () => first() })
  assert.equal((await loadArchived(stuck)).ok, true)
  assert.equal(stuck.calls.length, 2)
  // A full page with no row that has an id gives no cursor at all.
  const nameless = fakeClient({ [PROFILE.showArchived]: passed(Array.from({ length: ARCHIVE_PAGE }, () => ({ id: 7 }))) })
  assert.deepEqual(await loadArchived(nameless), { ok: true, value: [] })
  assert.equal(nameless.calls.length, 1)
})

test('the wiki listing and pages come back as Moai printed them, at wiki priority', async () => {
  const client = fakeClient({ [PROFILE.wikiLs]: passed(WIKI_LS), [PROFILE.wikiShow]: (slug) => (slug === 'home' ? passed(WIKI_SHOW) : { kind: 'moai-error', message: `${slug} not found` }) })
  assert.deepEqual(await loadWikiListing(client), { ok: true, value: WIKI_LS })
  assert.deepEqual(await loadWikiPage(client, 'home'), { ok: true, value: WIKI_SHOW })
  assert.deepEqual(await loadWikiPage(client, 'nope'), { ok: false, outcome: { kind: 'moai-error', message: 'nope not found' } })
  assert.deepEqual(client.calls.map((call) => [call.profileId, call.value, call.priority]), [
    [PROFILE.wikiLs, undefined, 1], [PROFILE.wikiShow, 'home', 1], [PROFILE.wikiShow, 'nope', 1],
  ])
})

test('status epics[] and milestones[] become [done, total] per id; malformed entries are skipped (se-tcd4)', async () => {
  const status = { counts: { done: 2 }, milestones: [{ id: 'm1', done: 4, total: 7 }, { id: 5, done: 1, total: 1 }], epics: [{ id: 'e1', done: 2, total: 3 }, { id: 'e2', done: '2', total: 3 }, null, { id: 'e3', total: 3 }] }
  assert.deepEqual([...talliesFrom(status)], [['m1', [4, 7]], ['e1', [2, 3]]])
  assert.equal(talliesFrom('x').size, 0)
  assert.equal(talliesFrom({ counts: {} }).size, 0)
  const real = JSON.parse(await readFile(new URL('./fixtures/moai/status-v016.json', import.meta.url), 'utf8'))
  for (const group of [...(real.epics ?? []), ...(real.milestones ?? [])]) assert.deepEqual(talliesFrom(real).get(group.id), [group.done, group.total])
  assert.deepEqual(await loadTallies(fakeClient({ [PROFILE.status]: passed(status) })), { ok: true, value: talliesFrom(status) })
  assert.deepEqual(await loadTallies(fakeClient({ [PROFILE.status]: { kind: 'not-initialized' } })), { ok: false, outcome: { kind: 'not-initialized' } })
})

test('archive rows absent from the active set are tagged archived; loadArchived does so on request (se-fvou)', async () => {
  assert.deepEqual(markArchived([{ id: 'a' }, { id: 'b' }], new Set(['a'])), [{ id: 'a' }, { id: 'b', archived: true }])
  const loaded = await loadArchived(fakeClient(archive(3)), { active: new Set(['r-0001']) })
  assert.deepEqual(loaded.value.map((row) => row.archived === true), [true, false, true])
  assert.equal((await loadArchived(fakeClient(archive(1)))).value[0].archived, undefined)
})

test('moai 0.17 marks its archive rows archived itself: the same meaning, never tagged twice, absence still tagged', async () => {
  const showAll = await fixture('show-all-v017.json')
  const archived = await fixture('show-archived-v017.json')
  // Real 0.17: `archived: true` stands on the one row `show --all` hides (done past archive_days), on no other.
  const marked = archived.filter((row) => row.archived === true).map((row) => row.id)
  assert.equal(marked.length, 1)
  assert.equal(showAll.some((row) => marked.includes(row.id)), false)
  assert.equal(showAll.some((row) => 'archived' in row), false)
  // Read against the rows held, the tag lands on exactly moai's row, which is passed through as moai printed it.
  const away = withArchive({ rows: showAll, truncated: false }, archived).slice(showAll.length)
  assert.deepEqual(away.map((row) => row.id), marked)
  const held = archived.find((row) => row.id === marked[0])
  assert.equal(markArchived([held], new Set())[0], held)
  // A row the clock archived after the archive was read is in its answer unmarked (it was active then) and gone
  // from the active rows read since: it is tagged by its absence.
  assert.deepEqual(markArchived([held, { id: 'late', status: 'done' }], new Set()).map((row) => row.archived), [true, true])
})

test('a cut list does not make an active row of the archive archive-only by its absence; a done one stays tagged', () => {
  // `--archived` lists the active rows too, with no mark of its own; prime held only mgd-a.
  const archive = [{ id: 'mgd-a', status: 'todo' }, { id: 'mgd-b', status: 'in_progress' }, { id: 'mgd-c', status: 'done' }]
  const cut = { rows: [{ id: 'mgd-a', status: 'todo' }], truncated: true }
  assert.deepEqual(withArchive(cut, archive), [{ id: 'mgd-a', status: 'todo' }, { id: 'mgd-b', status: 'in_progress' }, { id: 'mgd-c', status: 'done', archived: true }])
  assert.deepEqual([...activeIds(cut, archive)].sort(), ['mgd-a', 'mgd-b'])
  // A whole list holds every active row: what it does not hold is archive-only, whatever its column.
  const whole = { rows: [{ id: 'mgd-a', status: 'todo' }], truncated: false }
  assert.deepEqual(withArchive(whole, archive).slice(1).map((row) => row.archived), [true, true])
  assert.deepEqual(withArchive(whole, null), whole.rows)
})

test('a list prime stood in for because the catalog lacks show --all is judged as a cut one', () => {
  const archive = [{ id: 'mgd-a', status: 'todo' }, { id: 'mgd-b', status: 'in_progress' }, { id: 'mgd-c', status: 'done' }]
  const primed = { rows: [{ id: 'mgd-a', status: 'todo' }], partial: true, truncated: false }
  assert.deepEqual([...activeIds(primed, archive)].sort(), ['mgd-a', 'mgd-b'])
  assert.deepEqual(withArchive(primed, archive).map((row) => [row.id, row.archived === true]), [['mgd-a', false], ['mgd-b', false], ['mgd-c', true]])
})

test('a cut list judges a group of the archive by the column it stands in (its members), not the status written on it', () => {
  // moai leaves `todo` written on a group; its column is its members' (`derived_status`), as moai archives it.
  const archive = [
    { id: 'm-old', kind: 'milestone', status: 'todo', derived_status: 'done' },
    { id: 'e-live', kind: 'epic', status: 'done', derived_status: 'in_progress' },
    { id: 'e-new', kind: 'epic', status: 'todo' },
  ]
  const cut = { rows: [{ id: 'mgd-a', status: 'todo' }], truncated: true }
  assert.deepEqual([...activeIds(cut, archive)].sort(), ['e-live', 'e-new', 'mgd-a'])
  assert.deepEqual(withArchive(cut, archive).map((row) => [row.id, row.archived === true]), [['mgd-a', false], ['m-old', true], ['e-live', false], ['e-new', false]])
})
