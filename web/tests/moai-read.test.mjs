import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { isUnread, unreadIds } = await panelModule('data/read.ts')

const fixture = async (name) => JSON.parse(await readFile(new URL(`./fixtures/moai/${name}`, import.meta.url), 'utf8'))
const rows = await fixture('rows.json')

test('unread is the CLI key, read as is', () => {
  // The fixture tracker's rows are all near its one person, and none was read.
  assert.deepEqual(unreadIds(rows), rows.filter((row) => row.unread === true).map((row) => row.id))
  assert.ok(unreadIds(rows).length > 0)
  assert.equal(isUnread({ id: 'x', unread: false }), false, 'only true counts')
  assert.equal(isUnread({ id: 'x', unread: 'yes' }), false)
})

test('no row carrying the key means NEW is off', () => {
  const bare = rows.map(({ unread, ...rest }) => rest)
  assert.deepEqual(unreadIds(bare), [])
})

test('archive-only rows never draw NEW (se-fvou)', () => {
  const mixed = [{ id: 'a', unread: true }, { id: 'b', unread: true, archived: true }]
  assert.equal(isUnread(mixed[1]), false)
  assert.deepEqual(unreadIds(mixed), ['a'])
})

// ── the writes (moai-ly88): POST /api/read…, never shared, the rows read again after one that went through ──

const { PRIORITY, createClient, withProject } = await panelModule('data/client.ts')
const { READ_ENDPOINT, markAllRead, markGroupRead, markRead, parseRead, readMarks } = await panelModule('data/read.ts')

/** A fetch answering every call with `out` in the envelope (`exit` as given), recording method and URL. */
function server(out, exit = 0) {
  const calls = []
  const fetcher = async (url, init) => {
    calls.push([init.method ?? 'GET', url])
    return { status: 200, text: async () => JSON.stringify(exit === 0 || out !== undefined ? { exit, out, said: [] } : { exit, err: { error: 'no', code: 'error' }, said: [] }) }
  }
  return { calls, fetcher }
}

const ANSWER = { read: ['t-1'], missing: [], held: [], problems: [], at: '2026-10-22T00:00:00Z' }

test('the three writes are POSTs to the paths moai web takes, with no query of their own', async () => {
  assert.deepEqual(READ_ENDPOINT.one('moai-1abc.x2y'), { path: '/api/read/moai-1abc.x2y', method: 'POST' })
  assert.deepEqual(READ_ENDPOINT.all, { path: '/api/read-all', method: 'POST' })
  assert.deepEqual(READ_ENDPOINT.group('moai-1abc'), { path: '/api/read-group/moai-1abc', method: 'POST' })
  const { calls, fetcher } = server(ANSWER)
  const client = createClient(withProject(fetcher, '/w/argos'), { concurrency: 3 })
  assert.deepEqual(await markRead(client, { id: 't-1' }), { ok: true, result: ANSWER })
  await markAllRead(client)
  await markGroupRead(client, 'e1')
  assert.deepEqual(calls, [
    ['POST', '/api/read/t-1?project=%2Fw%2Fargos'],
    ['POST', '/api/read-all?project=%2Fw%2Fargos'],
    ['POST', '/api/read-group/e1?project=%2Fw%2Fargos'],
  ])
})

test('a write is detail-priority and never shared: two marks of one row are two POSTs, beside a read of the same path', async () => {
  const { calls, fetcher } = server(ANSWER)
  const client = createClient(fetcher, { concurrency: 3 })
  const asked = []
  const recording = { ...client, request: (endpoint, how) => (asked.push(how), client.request(endpoint, how)) }
  await Promise.all([markRead(recording, { id: 'a' }), markRead(recording, { id: 'a' }), client.request({ path: '/api/read/a' })])
  assert.deepEqual(asked, [{ priority: PRIORITY.detail, share: false }, { priority: PRIORITY.detail, share: false }])
  assert.deepEqual(calls.map(([method]) => method).sort(), ['GET', 'POST', 'POST'])
  // Even asked as shareable, a POST is not handed another's answer.
  const again = server(ANSWER)
  const plain = createClient(again.fetcher, { concurrency: 1 })
  await Promise.all([plain.request(READ_ENDPOINT.all), plain.request(READ_ENDPOINT.all)])
  assert.equal(again.calls.length, 2)
})

test('a missing id still hands its answer out; anything else that is no answer is the outcome', async () => {
  const missing = { ...ANSWER, missing: ['gone'] }
  assert.deepEqual(await markGroupRead(createClient(server(missing, 1).fetcher), 'gone'), { ok: true, result: missing })
  assert.deepEqual(await markAllRead(createClient(server({ unexpected: true }).fetcher)), { ok: false, outcome: { kind: 'unreadable' } })
  const refused = await markAllRead(createClient(server(undefined, 1).fetcher))
  assert.equal(refused.ok, false)
  assert.equal(refused.outcome.kind, 'moai-error')
  assert.deepEqual(parseRead({ read: [] }), { read: [], missing: [], held: [], problems: [], at: '' })
})

test('readMarks reads again after a write that went through, with the ids it wrote, before it answers', async () => {
  const order = []
  const { calls, fetcher } = server(ANSWER)
  const marks = readMarks(createClient(fetcher), async (written) => {
    order.push(['after', [...written], calls.length])
  })
  assert.equal(await marks.one('t-1'), null)
  assert.equal(await marks.all(), null)
  assert.equal(await marks.group('e1'), null)
  assert.deepEqual(order, [['after', ['t-1'], 1], ['after', ['t-1'], 2], ['after', ['t-1'], 3]])
  // A write that did not go through reads nothing again, and says why.
  const failed = []
  const refused = readMarks(createClient(server(undefined, 1).fetcher), async () => failed.push('after'))
  const outcome = await refused.one('t-1')
  assert.equal(outcome.kind, 'moai-error')
  assert.deepEqual(failed, [])
})
