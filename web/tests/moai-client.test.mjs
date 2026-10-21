import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { PANEL_CLIENT_OPTIONS, PRIORITY, answerOf, createClient, isInformation, outcomeOf, requestKey, urlOf } = await panelModule('data/client.ts')

const flush = async () => { for (let i = 0; i < 10; i++) await new Promise((resolve) => setImmediate(resolve)) }

/**
 * A fake `fetch`. Each call stays open until the test answers it with `answer(n, status, body)` (n counts from 1),
 * or fails it with `fail(n)`. An abort rejects it, as the browser's fetch does.
 */
function fakeFetch() {
  const calls = []
  const fetcher = (url, init) => new Promise((resolve, reject) => {
    const call = { url, init, resolve, reject }
    calls.push(call)
    init.signal.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')))
  })
  return {
    fetcher,
    calls,
    urls: () => calls.map((call) => call.url),
    answer(n, status, body) { calls[n - 1].resolve({ status, text: async () => (typeof body === 'string' ? body : JSON.stringify(body)) }) },
    fail(n) { calls[n - 1].reject(new TypeError('Failed to fetch')) },
  }
}

const envelope = (out, extra = {}) => ({ exit: 0, out, ...extra })

test('a read is one GET of its endpoint, its query in name order, and the envelope is the outcome', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher)
  const read = client.request({ path: '/api/rows', query: { q: 'x', archived: '1' } })
  await flush()
  assert.deepEqual(net.urls(), ['/api/rows?archived=1&q=x'])
  assert.equal(net.calls[0].init.headers.Accept, 'application/json')
  net.answer(1, 200, envelope([{ id: 'a' }]))
  assert.deepEqual(await read, { kind: 'passed', output: [{ id: 'a' }] })
  assert.deepEqual(client.requests(), [])
  assert.equal(urlOf({ path: '/api/status' }), '/api/status')
  assert.equal(requestKey({ path: '/api/rows', query: { b: '1', a: '2' } }), '/api/rows?a=2&b=1')
})

test('the same read asked twice while it runs is shared, not fetched again', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher)
  const one = client.request({ path: '/api/status' })
  const two = client.request({ path: '/api/status' })
  await flush()
  assert.equal(net.calls.length, 1)
  net.answer(1, 200, envelope({ counts: {} }))
  assert.equal(await one, await two)
})

test('a read asked after a change is not handed a run of it that already started; a queued one is shared', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher, { concurrency: 1 })
  const first = client.request({ path: '/api/rows' })
  await flush()
  const after = client.request({ path: '/api/rows' }, { epoch: 1 })
  const alsoAfter = client.request({ path: '/api/rows' }, { epoch: 1 })
  await flush()
  net.answer(1, 200, envelope([]))
  await first
  await flush()
  assert.equal(net.calls.length, 2, 'the read asked after the change ran again')
  net.answer(2, 200, envelope([{ id: 'b' }]))
  assert.equal(await after, await alsoAfter)
})

test('a read that must not be shared is fetched on its own', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher, { concurrency: 2 })
  void client.request({ path: '/api/issue/a' })
  void client.request({ path: '/api/issue/a' }, { share: false })
  await flush()
  assert.equal(net.calls.length, 2)
})

test('a subscriber that throws does not stop the queue', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher)
  const errors = console.error
  console.error = () => {}
  try {
    client.subscribe(() => { throw new Error('view broke') })
    const read = client.request({ path: '/api/status' })
    await flush()
    net.answer(1, 200, envelope({}))
    assert.equal((await read).kind, 'passed')
  } finally {
    console.error = errors
  }
})

test('with one read at a time, the detail goes before a waiting list refresh', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher, { concurrency: 1 })
  void client.request({ path: '/api/wiki' }, { priority: PRIORITY.wiki })
  void client.request({ path: '/api/rows' }, { priority: PRIORITY.list })
  void client.request({ path: '/api/issue/a' }, { priority: PRIORITY.detail })
  await flush()
  net.answer(1, 200, envelope([]))
  await flush()
  net.answer(2, 200, envelope({ id: 'a' }))
  await flush()
  assert.deepEqual(net.urls(), ['/api/wiki', '/api/issue/a', '/api/rows'])
})

test('a queued shared read asked sooner moves up', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher, { concurrency: 1 })
  void client.request({ path: '/api/status' })
  void client.request({ path: '/api/rows' }, { priority: PRIORITY.background })
  void client.request({ path: '/api/backlog' }, { priority: PRIORITY.list })
  void client.request({ path: '/api/rows' }, { priority: PRIORITY.detail })
  await flush()
  net.answer(1, 200, envelope({}))
  await flush()
  assert.deepEqual(net.urls(), ['/api/status', '/api/rows'])
})

test('the page runs three reads at once; the bare client one', async () => {
  assert.deepEqual(PANEL_CLIENT_OPTIONS, { concurrency: 3 })
  const net = fakeFetch()
  const client = createClient(net.fetcher, PANEL_CLIENT_OPTIONS)
  for (const path of ['/api/rows', '/api/backlog', '/api/status', '/api/wiki']) void client.request({ path })
  await flush()
  assert.equal(net.calls.length, 3)
  assert.deepEqual(client.requests().map((request) => request.phase), ['running', 'running', 'running', 'queued'])
  const bare = fakeFetch()
  const one = createClient(bare.fetcher)
  void one.request({ path: '/api/rows' })
  void one.request({ path: '/api/backlog' })
  await flush()
  assert.equal(bare.calls.length, 1)
})

test('a queued read is cancelled at once; a running one is aborted and ends cancelled, and the next runs', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher, { concurrency: 1 })
  const running = client.request({ path: '/api/rows' })
  const queued = client.request({ path: '/api/backlog' })
  const after = client.request({ path: '/api/status' })
  await flush()
  client.cancel(requestKey({ path: '/api/backlog' }))
  assert.deepEqual(await queued, { kind: 'cancelled' })
  client.cancel(requestKey({ path: '/api/rows' }))
  assert.deepEqual(await running, { kind: 'cancelled' })
  assert.equal(net.calls[0].init.signal.aborted, true)
  await flush()
  assert.deepEqual(net.urls(), ['/api/rows', '/api/status'])
  net.answer(2, 200, envelope({}))
  assert.equal((await after).kind, 'passed')
})

test('a fetch that fails is not-started (moai web out of reach), and the next read still runs', async () => {
  const net = fakeFetch()
  const client = createClient(net.fetcher)
  const lost = client.request({ path: '/api/rows' })
  const next = client.request({ path: '/api/status' })
  await flush()
  net.fail(1)
  assert.deepEqual(await lost, { kind: 'not-started' })
  await flush()
  net.answer(2, 200, envelope({}))
  assert.equal((await next).kind, 'passed')
})

test('the envelope, every way it reads', () => {
  // exit 0 with out: passed; the lines moai said ride as notes.
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 0, out: [1] })), { kind: 'passed', output: [1] })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 0, out: [], said: ['skipped a line', ''] })), { kind: 'passed', output: [], notes: ['skipped a line'] })
  // exit ≠ 0 with out: a partial answer, with what moai said why.
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 1, out: { pages: [] }, err: { error: 'page b skipped', code: 'error' } })), { kind: 'failed', output: { pages: [] }, message: 'page b skipped' })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 1, out: [] })), { kind: 'failed', output: [] })
  // exit ≠ 0 without out: moai's error.
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 2, err: { error: 'no such issue', code: 'not_found' } })), { kind: 'moai-error', message: 'no such issue', code: 'not_found' })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 2, err: { error: 'bad filter' }, said: ['x'] })), { kind: 'moai-error', message: 'bad filter', notes: ['x'] })
  // The server's own refusal.
  assert.deepEqual(outcomeOf(400, JSON.stringify({ error: 'unknown name q2', code: 'bad_input' })), { kind: 'moai-error', message: 'unknown name q2', code: 'bad_input' })
  assert.deepEqual(outcomeOf(504, JSON.stringify({ error: 'took too long' })), { kind: 'moai-error', message: 'took too long' })
  // Anything else.
  assert.deepEqual(outcomeOf(502, '<html>'), { kind: 'unreadable' })
  assert.deepEqual(outcomeOf(200, 'not json'), { kind: 'unreadable' })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ out: [] })), { kind: 'unreadable' })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 0 })), { kind: 'unreadable' })
  assert.deepEqual(outcomeOf(200, JSON.stringify({ exit: 3 })), { kind: 'unreadable' })
})

test('answerOf hands out a pass and a partial answer alike', () => {
  assert.deepEqual(answerOf({ kind: 'passed', output: [1] }), [1])
  assert.deepEqual(answerOf({ kind: 'failed', output: [2] }), [2])
  assert.equal(answerOf({ kind: 'failed' }), undefined)
  assert.equal(answerOf({ kind: 'moai-error', message: 'x' }), undefined)
})

test('only a cancel is information; everything a machine got wrong is an error', () => {
  assert.equal(isInformation({ kind: 'cancelled' }), true)
  for (const outcome of [{ kind: 'not-started' }, { kind: 'unreadable' }, { kind: 'failed' }, { kind: 'moai-error', message: 'x' }]) {
    assert.equal(isInformation(outcome), false, outcome.kind)
  }
})
