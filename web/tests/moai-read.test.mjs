import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { after, test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { PRIORITY, createClient } = await panelModule('data/client.ts')
const { PROFILE } = await panelModule('data/source.ts')
const { IssueIndex } = await panelModule('data/index.ts')
const { applyRead, isUnread, knowsUnread, markAllRead, markGroupRead, markRead, parseRead, staleAgainst, unreadCount, unreadIds, unreadUnder } =
  await panelModule('data/read.ts')

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const rows = await fixture('show-all-unread-v016.json')

let closing = false
after(() => { closing = true })

/** A fake bridge: every run finishes at once with the stdout the test hands out. */
function fakeBridge(stdout, state = 'passed') {
  const calls = []
  return {
    calls,
    async invokeCommand(command, args) {
      calls.push([command, args])
      if (command === 'buzzni.moai.run') return { action: 'start', state: 'accepted', operationId: `op-${calls.length}` }
      if (closing) return { state: 'cancelled' }
      return { action: 'status', state, stdout: typeof stdout === 'string' ? stdout : JSON.stringify(stdout), stderr: '', exitCode: state === 'passed' ? 0 : 1 }
    },
  }
}
const runs = (bridge) => bridge.calls.filter(([command]) => command === 'buzzni.moai.run').map(([, args]) => args)

/** A client that records the options of every request it passes on. */
function recording(client) {
  const options = []
  return { options, client: { ...client, request: (profileId, value, how) => (options.push(how), client.request(profileId, value, how)) } }
}

test('the profile ids the data layer writes with', () => {
  assert.equal(PROFILE.read, 'buzzni.moai.read')
  assert.equal(PROFILE.readAll, 'buzzni.moai.read-all')
  assert.equal(PROFILE.readGroup, 'buzzni.moai.read-group')
})

test('unread is the CLI key, read as is', () => {
  assert.equal(unreadCount(rows), 5)
  assert.deepEqual(unreadIds(rows).sort(), ['t-8x9q', 't-dse7', 't-oez7', 't-qhza', 't-vncv'])
  assert.equal(isUnread(rows.find((row) => row.id === 't-23k1')), false)
  assert.equal(isUnread({ id: 'x', unread: false }), false, 'only true counts')
  assert.equal(isUnread({ id: 'x', unread: 'yes' }), false)
})

test('no row carrying the key means NEW is off', () => {
  const bare = rows.map(({ unread, ...rest }) => rest)
  assert.equal(knowsUnread(bare), false)
  assert.equal(unreadCount(bare), 0)
  assert.equal(knowsUnread(rows), true)
})

test('unread under a group counts the group row and what is drawn under it', () => {
  const index = new IssueIndex(rows)
  assert.equal(unreadUnder(index, 't-oez7'), 3)
  assert.equal(unreadUnder(index, 't-qhza'), 0, 'not a group')
  assert.equal(unreadUnder(index, 'nope'), 0)
})

test('applyRead clears unread on the named rows and leaves the rest alone', () => {
  const before = JSON.stringify(rows)
  const next = applyRead(rows, ['t-qhza', 't-oez7'])
  assert.equal(JSON.stringify(rows), before, 'input untouched')
  assert.deepEqual(unreadIds(next).sort(), ['t-8x9q', 't-dse7', 't-vncv'])
  assert.equal(next[0], rows[0], 'unnamed rows keep identity')
  assert.ok(!('unread' in next.find((row) => row.id === 't-qhza')))
})

test('staleAgainst names rows whose updated_at moved since they were drawn', () => {
  const fresh = rows.map((row) => (row.id === 't-qhza' ? { ...row, updated_at: '2026-10-15T00:00:00Z' } : row)).filter((row) => row.id !== 't-vncv')
  assert.deepEqual(staleAgainst(rows, fresh), ['t-qhza'])
})

test('markRead sends the id as an unshared detail-priority write and parses the real read --json', async () => {
  const one = await fixture('read-one-v016.json')
  const bridge = fakeBridge(one)
  const { client, options } = recording(createClient(bridge, { sleep: async () => {} }))
  const marked = await markRead(client, rows.find((row) => row.id === 't-qhza'))
  assert.deepEqual(runs(bridge), [[PROFILE.read, 't-qhza']])
  assert.deepEqual(options, [{ priority: PRIORITY.detail, share: false }])
  assert.equal(marked.ok, true)
  assert.deepEqual(marked.result, one)
})

test('a missing id ends the write non-zero, yet its answer still comes through with missing', async () => {
  // As `moai read <removed id> --json` answers: the whole answer on stdout, exit 1.
  const answer = { read: ['t-qhza'], missing: ['gone'], held: [], problems: [], at: '2026-10-14T05:57:45Z' }
  const marked = await markGroupRead(createClient(fakeBridge(answer, 'failed'), { sleep: async () => {} }), 'gone')
  assert.deepEqual(marked, { ok: true, result: answer })
})

test('markAllRead and markGroupRead use their profiles', async () => {
  const all = await fixture('read-all-v016.json')
  const group = await fixture('read-group-v016.json')
  const bridge = fakeBridge(all)
  const client = createClient(bridge, { sleep: async () => {} })
  const first = await markAllRead(client)
  assert.deepEqual(first.result.read, ['t-qhza', 't-vncv'])
  const bridge2 = fakeBridge(group)
  const second = await markGroupRead(createClient(bridge2, { sleep: async () => {} }), 't-oez7')
  assert.deepEqual(runs(bridge), [[PROFILE.readAll]])
  assert.deepEqual(runs(bridge2), [[PROFILE.readGroup, 't-oez7']])
  assert.deepEqual(second.result.read, ['t-8x9q', 't-dse7', 't-oez7'])
})

test('two writes of the same row are two runs, never shared', async () => {
  const bridge = fakeBridge({ read: [], missing: [], held: [], problems: [], at: 'x' })
  const client = createClient(bridge, { sleep: async () => {} })
  await Promise.all([markRead(client, { id: 'a' }), markRead(client, { id: 'a' })])
  assert.equal(runs(bridge).length, 2)
})

test('missing, held and problems come through; already-read ids are simply absent from every list', () => {
  const result = parseRead({ read: ['a'], missing: ['nope'], held: ['b'], problems: ['why'], at: '2026-10-14T00:00:00Z' })
  assert.deepEqual(result, { read: ['a'], missing: ['nope'], held: ['b'], problems: ['why'], at: '2026-10-14T00:00:00Z' })
  assert.deepEqual(parseRead({ read: [] }), { read: [], missing: [], held: [], problems: [], at: '' })
})

test('failures come back as outcomes', async () => {
  const bad = await markAllRead(createClient(fakeBridge({ unexpected: true }), { sleep: async () => {} }))
  assert.deepEqual(bad, { ok: false, outcome: { kind: 'unreadable' } })
  const failed = await markAllRead(createClient(fakeBridge('', 'failed'), { sleep: async () => {} }))
  assert.equal(failed.ok, false)
  assert.equal(failed.outcome.kind, 'failed')
})

test('archive-only rows never draw NEW: moai read cannot clear them (se-fvou)', () => {
  const mixed = [{ id: 'a', unread: true }, { id: 'b', unread: true, archived: true }]
  assert.equal(isUnread(mixed[1]), false)
  assert.deepEqual(unreadIds(mixed), ['a'])
  assert.equal(unreadCount(mixed), 1)
  assert.equal(knowsUnread([mixed[1]]), false)
  const index = new IssueIndex([{ id: 'g', kind: 'epic', unread: true }, { id: 'x', epic: 'g', unread: true, archived: true }, { id: 'y', epic: 'g', unread: true }])
  assert.equal(unreadUnder(index, 'g'), 2)
})
