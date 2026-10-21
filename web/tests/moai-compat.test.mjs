import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const { CompatProbe, compatFixed, compatTells, sameReport } = await panelModule('data/compat.ts')
const { PROFILE, loadAllRows, loadStatuses, loadArchived, loadWikiListing } = await panelModule('data/source.ts')
const { markRead } = await panelModule('data/read.ts')

const passed = (output) => ({ kind: 'passed', output })
const refused = (code) => ({ kind: 'refused', code })
const withoutUnread = (rows) => rows.map(({ unread: _unread, ...row }) => row)

/** A client answering each profile from `answers` (an outcome, or a function of the value). */
const clientOf = (answers) => ({
  async request(profile, value) {
    const answer = answers[profile]
    return typeof answer === 'function' ? answer(value) : (answer ?? { kind: 'not-started' })
  },
  cancel() {},
})

// Real moai 0.17.0 output, captured from a scratch tracker (`moai init`, a custom `blocked` column).
const everything = async (overrides = {}) => ({
  [PROFILE.showAll]: passed(await fixture('show-all-v017.json')),
  [PROFILE.backlog]: passed(await fixture('backlog-v017.json')),
  [PROFILE.status]: passed(await fixture('status-v017.json')),
  ...overrides,
})

test('nothing seen yet says nothing is known', () => {
  assert.deepEqual(new CompatProbe().report(), { moai: 'unknown', toolMissing: false, catalog: { state: 'unknown', missing: [] }, degraded: [] })
})

test('real 0.17 output (statuses[], tallies, unread) reads as a whole new setup: moai ok', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything())
  assert.equal((await loadAllRows(client, undefined, probe)).ok, true)
  assert.deepEqual((await loadStatuses(client, undefined, probe)).value, ['todo', 'in_progress', 'blocked', 'review', 'done'])
  assert.deepEqual(probe.report(), { moai: 'ok', toolMissing: false, catalog: { state: 'ok', missing: [] }, degraded: [] })
})

test('real 0.16 output: no statuses[] is an old moai, yet nothing the panel uses falls back', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything({
    [PROFILE.showAll]: passed(await fixture('show-all-v016.json')),
    [PROFILE.backlog]: passed(await fixture('backlog-v016.json')),
    [PROFILE.status]: passed(await fixture('status-v016.json')),
  }))
  await loadAllRows(client, undefined, probe)
  const statuses = await loadStatuses(client, undefined, probe)
  assert.deepEqual(statuses.value, ['todo', 'in_progress', 'review', 'done'])
  const report = probe.report()
  assert.equal(report.moai, 'old')
  // The columns are ordered from the rows, not from `status` (loadStatuses has no caller), and 0.16 prints the tallies.
  assert.deepEqual(report.degraded, [])
  assert.equal(report.catalog.state, 'ok')
})

test('rows without `unread` on an old moai do not name the NEW marks: the key is absent when nothing is unread', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything({
    [PROFILE.showAll]: passed(withoutUnread(await fixture('show-all-unread-v016.json'))),
    [PROFILE.backlog]: passed(withoutUnread(await fixture('backlog-v016.json'))),
    [PROFILE.status]: passed(await fixture('status-v016.json')),
  }))
  await loadAllRows(client, undefined, probe)
  await loadStatuses(client, undefined, probe)
  assert.equal(probe.report().moai, 'old')
  assert.deepEqual(probe.report().degraded, [])
})

test('an old status without the tallies still asks to upgrade, and names the counts, never the NEW marks', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything({
    [PROFILE.showAll]: passed(withoutUnread(await fixture('show-all-v016.json'))),
    [PROFILE.backlog]: passed(withoutUnread(await fixture('backlog-v016.json'))),
  }))
  await loadAllRows(client, undefined, probe)
  probe.observeStatus({ counts: { todo: 1 }, total: 1, statuses: ['todo'] })
  assert.equal(probe.report().moai, 'old')
  assert.deepEqual(probe.report().degraded, ['tallies'])
})

test('a 0.17 list with no `unread` key says nothing: nothing is unread', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything({
    [PROFILE.showAll]: passed(withoutUnread(await fixture('show-all-v017.json'))),
    [PROFILE.backlog]: passed(withoutUnread(await fixture('backlog-v017.json'))),
  }))
  await loadAllRows(client, undefined, probe)
  await loadStatuses(client, undefined, probe)
  assert.deepEqual(probe.report(), { moai: 'ok', toolMissing: false, catalog: { state: 'ok', missing: [] }, degraded: [] })
})

test('rows without `unread` alone are not proof: moai stays unknown', async () => {
  const probe = new CompatProbe()
  await loadAllRows(clientOf(await everything({
    [PROFILE.showAll]: passed(withoutUnread(await fixture('show-all-v016.json'))),
    [PROFILE.backlog]: passed(withoutUnread(await fixture('backlog-v016.json'))),
  })), undefined, probe)
  assert.deepEqual(probe.report(), { moai: 'unknown', toolMissing: false, catalog: { state: 'ok', missing: [] }, degraded: [] })
})

test('a status with no milestones[] and epics[] degrades the tallies', () => {
  const probe = new CompatProbe()
  probe.observeStatus({ counts: { todo: 1 }, total: 1, statuses: ['todo'] })
  assert.deepEqual(probe.report().degraded, ['tallies'])
  assert.equal(probe.report().moai, 'old')
})

test('the project list (outside a board) is not read as a shape', () => {
  const probe = new CompatProbe()
  probe.observeStatus({ projects: [{ path: '/w' }] })
  assert.equal(probe.report().moai, 'unknown')
})

// A list read the catalog lacks degrades the load (source.ts): prime stands in for show --all, the backlog goes.
test('unsupported-daemon on added profiles names them; their features degrade, the rest keeps working', async () => {
  const probe = new CompatProbe()
  const client = clientOf(await everything({
    [PROFILE.showAll]: refused('unsupported-daemon'),
    [PROFILE.backlog]: refused('unsupported-daemon'),
    [PROFILE.prime]: passed({ picked: [{ id: 'trk-flv8', title: 'Held', status: 'in_progress' }], others: [], ready: [], rest: 0, others_rest: 0 }),
    [PROFILE.wikiLs]: passed({ pages: [] }),
    [PROFILE.showArchived]: refused('unsupported-daemon'),
  }))
  const loaded = await loadAllRows(client, undefined, probe)
  assert.deepEqual([loaded.ok, loaded.value.partial, loaded.value.rows.map((row) => row.id)], [true, true, ['trk-flv8']])
  await loadStatuses(client, undefined, probe)
  await loadWikiListing(client, probe)
  await loadArchived(client, { probe })
  const report = probe.report()
  assert.equal(report.catalog.state, 'missing')
  assert.deepEqual(report.catalog.missing.sort(), [PROFILE.showAll, PROFILE.backlog, PROFILE.showArchived].sort())
  assert.deepEqual(report.degraded, ['all-rows', 'backlog', 'archive'])
})

test('once the whole list is missing, the other added profiles are lacked unrun; a pass of the list lifts it (se-24ey.dcj)', () => {
  const probe = new CompatProbe()
  const others = [PROFILE.backlog, PROFILE.showArchived, PROFILE.showArchivedAfter, PROFILE.read, PROFILE.readAll, PROFILE.readGroup]
  // Nothing heard of the whole list yet: nothing is lacked, the reads go out.
  assert.deepEqual(others.filter((profile) => probe.lacks(profile)), [])
  // The whole list refused as unsupported is enough: a catalog without it lacks the rest, and a daemon too old for the
  // machine run refuses them all the same.
  probe.observe(PROFILE.showAll, refused('unsupported-daemon'))
  assert.deepEqual(others.filter((profile) => probe.lacks(profile)), others)
  // The whole list itself is still asked (an updated catalog is found that way), and the shipped profiles never lacked.
  for (const profile of [PROFILE.showAll, PROFILE.status, PROFILE.prime, PROFILE.show, PROFILE.wikiLs, PROFILE.wikiShow]) assert.equal(probe.lacks(profile), false, profile)
  // One that has run there is there, whatever the rest says.
  probe.observe(PROFILE.backlog, passed([]))
  assert.equal(probe.lacks(PROFILE.backlog), false)
  assert.equal(probe.lacks(PROFILE.read), true)
  // What the panel answered itself counts while the whole list is missing, and stops when it runs (se-43oh).
  probe.observe(PROFILE.showArchived, { kind: 'refused', code: 'unsupported-daemon', assumed: true })
  assert.ok(probe.report().catalog.missing.includes(PROFILE.showArchived))
  probe.observe(PROFILE.showAll, passed([]))
  assert.deepEqual(others.filter((profile) => probe.lacks(profile)), [])
  assert.equal(probe.report().catalog.missing.includes(PROFILE.showArchived), false)
})

test('a shipped profile refused as unsupported means the daemon as a whole, not 12 missing profiles', async () => {
  const probe = new CompatProbe()
  const client = clientOf({ [PROFILE.status]: refused('unsupported-daemon'), [PROFILE.showAll]: refused('unsupported-daemon') })
  await loadStatuses(client, undefined, probe)
  const report = probe.report()
  assert.deepEqual(report.catalog, { state: 'unknown', missing: [] })
  assert.equal(report.moai, 'unknown')
})

test('tool-missing is its own flag and does not blame the catalog', async () => {
  const probe = new CompatProbe()
  await loadStatuses(clientOf({ [PROFILE.status]: refused('tool-missing') }), undefined, probe)
  assert.deepEqual(probe.report(), { moai: 'unknown', toolMissing: true, catalog: { state: 'unknown', missing: [] }, degraded: [] })
})

test('a start that failed naming unsupported-daemon (a thrown start, its code carried) is read as missing; a pass clears it', async () => {
  const probe = new CompatProbe()
  const reads = [{ kind: 'not-started', code: 'unsupported-daemon' }, passed({ read: ['a'], missing: [], held: [], problems: [], at: 'now' })]
  const client = clientOf({ [PROFILE.status]: passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }), [PROFILE.read]: () => reads.shift() })
  await loadStatuses(client, undefined, probe)
  await markRead(client, { id: 'a' }, probe)
  assert.deepEqual(probe.report().catalog, { state: 'missing', missing: [PROFILE.read] })
  assert.deepEqual(probe.report().degraded, ['read-marks'])
  await markRead(client, { id: 'a' }, probe)
  assert.deepEqual(probe.report().catalog, { state: 'ok', missing: [] })
})

test('binding-mismatch says the binding or the approval changed, nothing of the catalog', async () => {
  const probe = new CompatProbe()
  const client = clientOf({ [PROFILE.status]: passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }), [PROFILE.read]: { kind: 'not-started', code: 'binding-mismatch' } })
  await loadStatuses(client, undefined, probe)
  await markRead(client, { id: 'a' }, probe)
  assert.deepEqual(probe.report().catalog, { state: 'ok', missing: [] })
  assert.deepEqual(probe.report().degraded, [])
})

test('a start that failed without naming a catalog code leaves the catalog unknown, not missing', async () => {
  const probe = new CompatProbe()
  const client = clientOf({ [PROFILE.status]: passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }), [PROFILE.read]: { kind: 'not-started' } })
  await loadStatuses(client, undefined, probe)
  await markRead(client, { id: 'a' }, probe)
  assert.deepEqual(probe.report().catalog, { state: 'ok', missing: [] })
  assert.deepEqual(probe.report().degraded, [])
  probe.observe(PROFILE.readAll, { kind: 'not-started', code: 'something-new' })
  assert.deepEqual(probe.report().catalog, { state: 'ok', missing: [] })
})

test('information refusals and cancellations are not compatibility evidence', () => {
  const probe = new CompatProbe()
  probe.observe(PROFILE.showAll, refused('declined'))
  probe.observe(PROFILE.showAll, refused('workspace-busy'))
  probe.observe(PROFILE.showAll, { kind: 'cancelled', mayContinue: false })
  assert.equal(probe.report().catalog.state, 'unknown')
})

test('two reports that say the same are the same, so the banner is not drawn again', () => {
  const report = () => ({ moai: 'old', toolMissing: false, catalog: { state: 'missing', missing: [PROFILE.showAll] }, degraded: ['all-rows', 'status-order'] })
  assert.equal(sameReport(report(), report()), true)
  for (const change of [{ moai: 'ok' }, { toolMissing: true }, { catalog: { state: 'missing', missing: [PROFILE.backlog] } }, { degraded: ['all-rows'] }]) {
    assert.equal(sameReport(report(), { ...report(), ...change }), false, JSON.stringify(change))
  }
})

test('tool-missing goes once a run starts there again (moai installed while the panel is open)', () => {
  const probe = new CompatProbe()
  probe.observe(PROFILE.status, refused('tool-missing'))
  assert.equal(probe.report().toolMissing, true)
  probe.observe(PROFILE.status, refused('declined'))
  assert.equal(probe.report().toolMissing, true, 'a refusal that says nothing of the tool keeps it')
  probe.observe(PROFILE.status, passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }))
  assert.equal(probe.report().toolMissing, false)
})

test('the banner tells moai missing and a profile the catalog lacks; other refusals stay the board\'s', () => {
  const report = (change = {}) => ({ moai: 'unknown', toolMissing: false, catalog: { state: 'unknown', missing: [] }, degraded: [], ...change })
  assert.equal(compatTells(report({ toolMissing: true }), refused('tool-missing')), true)
  assert.equal(compatTells(report(), refused('tool-missing')), false, 'not while the banner does not say it')
  assert.equal(compatTells(report({ catalog: { state: 'missing', missing: [PROFILE.showAll] } }), refused('unsupported-daemon')), true)
  // The daemon as a whole too old: the catalog line says nothing, so the board's banner tells it.
  assert.equal(compatTells(report(), refused('unsupported-daemon')), false)
  const told = report({ toolMissing: true, catalog: { state: 'missing', missing: [PROFILE.showAll] } })
  for (const outcome of [refused('workspace-busy'), refused('unsupported-platform'), { kind: 'failed' }, { kind: 'not-started' }, { kind: 'not-started', code: 'binding-mismatch' }, null]) {
    assert.equal(compatTells(told, outcome), false, JSON.stringify(outcome))
  }
  // A start that failed naming the code (an older host) is read as the refusal is: the catalog line tells it too.
  assert.equal(compatTells(told, { kind: 'not-started', code: 'unsupported-daemon' }), true)
  assert.equal(compatTells(null, refused('tool-missing')), false)
  // A read the probe does not hear (the detail, the add): the catalog line names only profiles it heard, while moai
  // missing is the machine's.
  assert.equal(compatTells(told, refused('unsupported-daemon'), false), false)
  assert.equal(compatTells(told, refused('tool-missing'), false), true)
})

test('refusals before anything has run leave the catalog unknown: a daemon too old for machine.run refuses them too', () => {
  const probe = new CompatProbe()
  probe.observe(PROFILE.showAll, refused('unsupported-daemon'))
  probe.observe(PROFILE.backlog, { kind: 'not-started', code: 'unsupported-daemon' })
  assert.deepEqual(probe.report().catalog, { state: 'unknown', missing: [] })
  // An added profile still counts as off, whatever the catalog turns out to be.
  assert.deepEqual(probe.report().degraded, ['all-rows', 'backlog'])
  probe.observe(PROFILE.status, passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }))
  assert.deepEqual(probe.report().catalog, { state: 'missing', missing: [PROFILE.showAll, PROFILE.backlog] })
})

test('a thrown start naming unsupported-daemon is read as the refusal is: on a shipped profile, the daemon as a whole', () => {
  const probe = new CompatProbe()
  probe.observe(PROFILE.showAll, passed([]))
  probe.observe(PROFILE.backlog, refused('unsupported-daemon'))
  assert.deepEqual(probe.report().catalog, { state: 'missing', missing: [PROFILE.backlog] })
  probe.observe(PROFILE.wikiLs, { kind: 'not-started', code: 'unsupported-daemon' })
  assert.deepEqual(probe.report().catalog, { state: 'unknown', missing: [] }, 'as a refused wiki-ls would say')
  // And after a pass of the same profile, as a refusal does.
  const again = new CompatProbe()
  again.observe(PROFILE.status, passed({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] }))
  again.observe(PROFILE.read, passed({ read: [], missing: [], held: [], problems: [], at: 'now' }))
  again.observe(PROFILE.read, { kind: 'not-started', code: 'unsupported-daemon' })
  assert.deepEqual(again.report().catalog, { state: 'missing', missing: [PROFILE.read] })
})

test('a refusal the banner told is fixed once moai has run there, or the catalog lacks nothing', () => {
  const report = (change = {}) => ({ moai: 'ok', toolMissing: false, catalog: { state: 'ok', missing: [] }, degraded: [], ...change })
  const missing = report({ toolMissing: true })
  assert.equal(compatFixed(missing, report(), refused('tool-missing')), true)
  assert.equal(compatFixed(missing, missing, refused('tool-missing')), false, 'still missing')
  assert.equal(compatFixed(report(), report(), refused('tool-missing')), false, 'never told')
  const lacks = report({ catalog: { state: 'missing', missing: [PROFILE.showArchived] } })
  assert.equal(compatFixed(lacks, report(), refused('unsupported-daemon')), true)
  assert.equal(compatFixed(lacks, report({ catalog: { state: 'missing', missing: [PROFILE.read] } }), refused('unsupported-daemon')), false, 'still told')
  // The verdict gone back to unknown (a shipped profile refused: the daemon as a whole) fixes nothing: the board tells it.
  assert.equal(compatFixed(lacks, report({ catalog: { state: 'unknown', missing: [] } }), refused('unsupported-daemon')), false)
  // The detail's read is not heard: the catalog line never told its refusal.
  assert.equal(compatFixed(lacks, report(), refused('unsupported-daemon'), false), false)
  assert.equal(compatFixed(missing, report(), refused('tool-missing'), false), true)
})

test('the probe tells its listeners after each thing handed over, until unsubscribed', () => {
  const probe = new CompatProbe()
  let told = 0
  const unsubscribe = probe.subscribe(() => { told += 1 })
  probe.observe(PROFILE.status, refused('tool-missing'))
  probe.observeStatus({ counts: { todo: 0 }, statuses: ['todo'], milestones: [], epics: [] })
  assert.equal(told, 2)
  unsubscribe()
  probe.observe(PROFILE.status, passed({ counts: { todo: 0 } }))
  assert.equal(told, 2)
})

test('a listener that throws does not stop the read that handed the answer over', async (t) => {
  const probe = new CompatProbe()
  const errors = []
  t.mock.method(console, 'error', (error) => errors.push(error))
  probe.subscribe(() => { throw new Error('the view broke') })
  let heard = 0
  probe.subscribe(() => { heard += 1 })
  const client = clientOf(await everything())
  assert.equal((await loadAllRows(client, undefined, probe)).ok, true)
  assert.equal(heard, 2, 'show --all and the backlog were both handed over')
  assert.equal(errors.length, 2)
})
