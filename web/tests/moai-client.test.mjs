import assert from 'node:assert/strict'
import { after, test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { INFO_REFUSALS, PANEL_CLIENT_OPTIONS, PRIORITY, REFUSAL_CODES, answerOf, createClient, isInformation, outcomeOf, refusalOf, requestKey } = await panelModule('data/client.ts')
const { rowsFromPrime } = await panelModule('data/source.ts')

// Runs a test leaves running would poll forever; once every test is over they all end cancelled.
let closing = false
after(() => { closing = true })

const flush = async () => { for (let i = 0; i < 10; i++) await new Promise((resolve) => setImmediate(resolve)) }

/**
 * A fake panel bridge. Each start gets an operation `op-N`; a run stays running until the test finishes it
 * with `finish(N, status)`.
 */
function fakeBridge() {
  const calls = []
  const done = new Map()
  let next = 0
  return {
    calls,
    finish(n, status) { done.set(`op-${n}`, status) },
    async invokeCommand(command, args) {
      calls.push([command, args])
      if (command === 'buzzni.moai.run') return { action: 'start', state: 'accepted', operationId: `op-${++next}` }
      if (command === 'buzzni.moai.cancel') {
        done.set(args[0], { state: 'cancelled', remoteMayContinue: true })
        return { action: 'cancel', state: 'cancelled' }
      }
      if (closing) return { action: 'status', operationId: args[0], state: 'cancelled' }
      return { action: 'status', operationId: args[0], ...(done.get(args[0]) ?? { state: 'running' }) }
    },
  }
}

const passed = (output) => ({ state: 'passed', stdout: JSON.stringify(output), stderr: '', exitCode: 0 })
const starts = (bridge) => bridge.calls.filter(([command]) => command === 'buzzni.moai.run').map(([, args]) => args)

test('a run is started, polled until it ends, and its JSON is the outcome', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush() })
  const outcome = client.request('buzzni.moai.status')
  await flush()
  assert.deepEqual(client.requests().map((request) => [request.phase, request.operationId]), [['running', 'op-1']])
  bridge.finish(1, passed({ counts: { todo: 1 } }))
  assert.deepEqual(await outcome, { kind: 'passed', output: { counts: { todo: 1 } } })
  assert.deepEqual(client.requests(), [])
  assert.deepEqual(starts(bridge), [['buzzni.moai.status']])
})

test('the same read asked twice while it runs is shared, not started again', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush() })
  const first = client.request('buzzni.moai.show', 'a-1')
  const second = client.request('buzzni.moai.show', 'a-1')
  assert.equal(first, second)
  await flush()
  bridge.finish(1, passed({ id: 'a-1' }))
  await first
  assert.deepEqual(starts(bridge), [['buzzni.moai.show', 'a-1']])
})

test('a read asked after a write is not handed a run of it that already started; a queued one is shared', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { concurrency: 3, sleep: () => flush() })
  const before = client.request('buzzni.moai.show', 'a-1')
  await flush()
  // The caller counts a write since `before` was asked, and that run has started: it may answer from before the write.
  const after = client.request('buzzni.moai.show', 'a-1', { epoch: 1 })
  assert.notEqual(after, before)
  // Asked again at that count, or by a caller that counts none, it joins the newest run.
  assert.equal(client.request('buzzni.moai.show', 'a-1', { epoch: 1 }), after)
  assert.equal(client.request('buzzni.moai.show', 'a-1'), after)
  await flush()
  bridge.finish(1, passed({ id: 'a-1', unread: true }))
  bridge.finish(2, passed({ id: 'a-1' }))
  assert.deepEqual(await before, { kind: 'passed', output: { id: 'a-1', unread: true } })
  assert.deepEqual(await after, { kind: 'passed', output: { id: 'a-1' } })
  assert.deepEqual(starts(bridge), [['buzzni.moai.show', 'a-1'], ['buzzni.moai.show', 'a-1']])
  // One run at a time: a read still queued has not started, so it is shared, and it then answers for that count.
  const slow = fakeBridge()
  const single = createClient(slow, { sleep: () => flush() })
  const blocker = single.request('buzzni.moai.status')
  const queued = single.request('buzzni.moai.show', 'a-2')
  assert.equal(single.request('buzzni.moai.show', 'a-2', { epoch: 1 }), queued)
  await flush()
  slow.finish(1, passed({ counts: {} }))
  await blocker
  await flush()
  assert.equal(single.request('buzzni.moai.show', 'a-2', { epoch: 1 }), queued)
  slow.finish(2, passed({ id: 'a-2' }))
  await queued
  assert.deepEqual(starts(slow), [['buzzni.moai.status'], ['buzzni.moai.show', 'a-2']])
})

test('a subscriber that throws does not stop the queue', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush() })
  const seen = []
  let boom = 3
  client.subscribe(() => {
    seen.push(client.requests().map((request) => request.phase).join())
    if (boom-- > 0) throw new Error('subscriber failed')
  })
  const logged = console.error
  console.error = () => {}
  try {
    const first = client.request('buzzni.moai.status')
    const second = client.request('buzzni.moai.show', 'a-1')
    await flush()
    bridge.finish(1, passed({ ok: 1 }))
    assert.deepEqual(await first, { kind: 'passed', output: { ok: 1 } })
    await flush()
    // The second request started although subscribers threw while the first was starting, running and settling.
    assert.deepEqual(starts(bridge), [['buzzni.moai.status'], ['buzzni.moai.show', 'a-1']])
    bridge.finish(2, passed({ id: 'a-1' }))
    assert.deepEqual(await second, { kind: 'passed', output: { id: 'a-1' } })
  } finally {
    console.error = logged
  }
})

test('writes are never shared', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush(), concurrency: 2 })
  const first = client.request('buzzni.moai.read', 'mgd-89af', { share: false })
  const second = client.request('buzzni.moai.read', 'mgd-89af', { share: false })
  assert.notEqual(first, second)
  await flush()
  assert.equal(starts(bridge).length, 2)
})

test('with one run at a time, the detail goes before a waiting list refresh', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush() })
  client.request('buzzni.moai.status', undefined, { priority: PRIORITY.background })
  await flush()
  client.request('buzzni.moai.prime', undefined, { priority: PRIORITY.list })
  client.request('buzzni.moai.show', 'a-1', { priority: PRIORITY.detail })
  await flush()
  assert.deepEqual(client.requests().map((request) => [request.profileId, request.phase]),
    [['buzzni.moai.status', 'running'], ['buzzni.moai.prime', 'queued'], ['buzzni.moai.show', 'queued']])
  bridge.finish(1, passed({}))
  await flush()
  assert.deepEqual(starts(bridge).map(([profile]) => profile), ['buzzni.moai.status', 'buzzni.moai.show'])
})

test('a slow run does not lock the others when more may run at once', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush(), concurrency: 2 })
  const slow = client.request('buzzni.moai.prime')
  const quick = client.request('buzzni.moai.show', 'a-1')
  await flush()
  bridge.finish(2, passed({ id: 'a-1' }))
  assert.equal((await quick).kind, 'passed')
  assert.equal(client.requests()[0].profileId, 'buzzni.moai.prime')
  bridge.finish(1, passed({}))
  await slow
})

test('a queued request is cancelled at once; a running one asks the machine and ends cancelled', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { sleep: () => flush() })
  const running = client.request('buzzni.moai.prime')
  const queued = client.request('buzzni.moai.show', 'a-1')
  await flush()
  client.cancel(requestKey('buzzni.moai.show', 'a-1'))
  assert.deepEqual(await queued, { kind: 'cancelled', mayContinue: false })
  client.cancel(requestKey('buzzni.moai.prime'))
  assert.deepEqual(await running, { kind: 'cancelled', mayContinue: true })
  assert.deepEqual(bridge.calls.filter(([command]) => command === 'buzzni.moai.cancel'), [['buzzni.moai.cancel', ['op-1']]])
})

test('a start that throws or is refused is an outcome, and the next request still runs', async () => {
  let call = 0
  const client = createClient({
    async invokeCommand(command) {
      call += 1
      if (call === 1) throw new Error('capability.invoke machine.run/start failed: binding-mismatch')
      if (call === 2) return { action: 'start', state: 'refused', code: 'workspace-busy' }
      if (call === 3) return { action: 'start', state: 'refused', code: 'something-new' }
      return command === 'buzzni.moai.run' ? { state: 'accepted', operationId: 'op' } : passed({})
    },
  }, { sleep: () => flush() })
  assert.deepEqual(await client.request('buzzni.moai.prime'), { kind: 'not-started', code: 'binding-mismatch' })
  assert.deepEqual(await client.request('buzzni.moai.prime'), { kind: 'refused', code: 'workspace-busy' })
  assert.deepEqual(await client.request('buzzni.moai.prime'), { kind: 'not-started', code: 'something-new' })
  assert.equal((await client.request('buzzni.moai.prime')).kind, 'passed')
})

test('finished runs read as Moai errors, a missing board, unreadable output or plain failure', () => {
  assert.deepEqual(outcomeOf({ state: 'failed', stdout: '', stderr: JSON.stringify({ error: 'unknown id: x' }) }), { kind: 'moai-error', message: 'unknown id: x' })
  assert.deepEqual(outcomeOf({ state: 'failed', stderr: JSON.stringify({ error: 'Not a moai repository (no .moai/ above /w)' }) }), { kind: 'not-initialized' })
  assert.deepEqual(outcomeOf({ state: 'passed', stdout: 'not json' }), { kind: 'unreadable' })
  assert.deepEqual(outcomeOf({ state: 'failed', stdout: '', stderr: 'boom' }), { kind: 'failed' })
  assert.deepEqual(outcomeOf({ state: 'cancelled' }), { kind: 'cancelled', mayContinue: false })
  assert.deepEqual(outcomeOf(null), { kind: 'failed' })
  assert.equal(refusalOf({ state: 'accepted', operationId: 'op' }), null)
  assert.deepEqual(refusalOf({ state: 'accepted' }), { kind: 'not-started' })
})

test('prime rows become index rows; a folder without a board says so', () => {
  const loaded = rowsFromPrime({ picked: [{ id: 'a', status: 'in_progress' }], others: [{ id: 'b' }, { id: 'a' }], ready: [{ id: 'c' }, { bad: true }], rest: 2, others_rest: 1 })
  assert.equal(loaded.ok, true)
  assert.deepEqual(loaded.value.rows.map((row) => [row.id, row.kind]), [['a', 'issue'], ['b', 'issue'], ['c', 'issue']])
  assert.equal(loaded.value.more, 3)
  assert.deepEqual(rowsFromPrime({ no_tracker: true }), { ok: false, outcome: { kind: 'not-initialized' } })
})

/** A bridge whose starts are refused `workspace-busy` the first `busy` times, then accepted and passed. */
function busyBridge(busy) {
  const calls = []
  const sleeps = []
  let refused = 0
  return {
    calls,
    sleeps,
    sleep: (ms) => { sleeps.push(ms); return flush() },
    async invokeCommand(command, args) {
      calls.push([command, args])
      if (command === 'buzzni.moai.run') {
        if (refused < busy) {
          refused += 1
          return { action: 'start', state: 'refused', code: 'workspace-busy' }
        }
        return { action: 'start', state: 'accepted', operationId: 'op' }
      }
      return { action: 'status', operationId: args[0], ...passed({ ok: true }) }
    },
  }
}

test('the panel options are 3 runs at once and a busy read tried 3 times; the bare client keeps 1 and no retry', async () => {
  assert.deepEqual({ ...PANEL_CLIENT_OPTIONS }, { concurrency: 3, busyAttempts: 3, busyBackoffMs: 1000 })
  const bridge = busyBridge(1)
  const client = createClient(bridge, { sleep: bridge.sleep })
  assert.deepEqual(await client.request('buzzni.moai.prime'), { kind: 'refused', code: 'workspace-busy' })
})

test('the panel options run three reads at once', async () => {
  const bridge = fakeBridge()
  const client = createClient(bridge, { ...PANEL_CLIENT_OPTIONS, sleep: () => flush() })
  for (const profile of ['buzzni.moai.show-all', 'buzzni.moai.backlog-all', 'buzzni.moai.status', 'buzzni.moai.wiki-ls']) client.request(profile)
  await flush()
  assert.deepEqual(client.requests().map((request) => request.phase), ['running', 'running', 'running', 'queued'])
})

test('a shared read refused as busy goes back to the queue and is tried again after a growing backoff', async () => {
  const bridge = busyBridge(2)
  const client = createClient(bridge, { ...PANEL_CLIENT_OPTIONS, sleep: bridge.sleep })
  assert.deepEqual(await client.request('buzzni.moai.show-all'), { kind: 'passed', output: { ok: true } })
  assert.equal(starts(bridge).length, 3)
  // The poll sleeps are not backoffs: those are the 1 s and 2 s ones.
  assert.deepEqual(bridge.sleeps.filter((ms) => ms >= 1000), [1000, 2000])
})

test('a busy read is refused once its tries are spent; a write is never tried again', async () => {
  const reads = busyBridge(5)
  const client = createClient(reads, { ...PANEL_CLIENT_OPTIONS, sleep: reads.sleep })
  assert.deepEqual(await client.request('buzzni.moai.backlog-all'), { kind: 'refused', code: 'workspace-busy' })
  assert.equal(starts(reads).length, 3)
  const writes = busyBridge(1)
  const writer = createClient(writes, { ...PANEL_CLIENT_OPTIONS, sleep: writes.sleep })
  assert.deepEqual(await writer.request('buzzni.moai.read', 'mgd-89af', { share: false }), { kind: 'refused', code: 'workspace-busy' })
  assert.deepEqual(starts(writes), [['buzzni.moai.read', 'mgd-89af']])
})

test('while a busy read waits out its backoff it frees its slot, shows as queued and can be cancelled', async () => {
  let wake
  const bridge = busyBridge(1)
  const client = createClient(bridge, {
    concurrency: 1, busyAttempts: 3, busyBackoffMs: 1000,
    sleep: (ms) => (ms >= 1000 ? new Promise((resolve) => (wake = resolve)) : flush()),
  })
  const waiting = client.request('buzzni.moai.show-all')
  await flush()
  assert.deepEqual(client.requests().map((request) => [request.profileId, request.phase]), [['buzzni.moai.show-all', 'queued']])
  // The slot is free: another read runs meanwhile.
  assert.equal((await client.request('buzzni.moai.status')).kind, 'passed')
  client.cancel(requestKey('buzzni.moai.show-all'))
  assert.deepEqual(await waiting, { kind: 'cancelled', mayContinue: false })
  wake()
  await flush()
  assert.deepEqual(starts(bridge), [['buzzni.moai.show-all'], ['buzzni.moai.status']])
})

test('a cut stdout reads as unreadable with truncated; one that still parses passes with truncated', () => {
  assert.deepEqual(outcomeOf({ state: 'passed', stdout: '[{"id":"a"},{"id', truncated: true }), { kind: 'unreadable', truncated: true })
  assert.deepEqual(outcomeOf({ state: 'passed', stdout: '[]', truncated: true }), { kind: 'passed', output: [], truncated: true })
  assert.deepEqual(outcomeOf({ state: 'passed', stdout: '[]', truncated: false }), { kind: 'passed', output: [] })
  // A Moai error still reads as one, cut or not.
  assert.deepEqual(outcomeOf({ state: 'failed', stdout: '[{', stderr: JSON.stringify({ error: 'boom' }), truncated: true }), { kind: 'moai-error', message: 'boom' })
})

test('a run Moai ended non-zero on a partial answer keeps the answer it printed', () => {
  // `moai read gone --json`: prose on stderr, the whole answer on stdout, exit 1.
  const answer = { read: [], missing: ['gone'], held: [], problems: [], at: 't' }
  assert.deepEqual(outcomeOf({ state: 'failed', stdout: JSON.stringify(answer), stderr: 'moai: gone not found\n', exitCode: 1 }), { kind: 'failed', output: answer })
  assert.deepEqual(answerOf({ kind: 'failed', output: answer }), answer)
  assert.deepEqual(answerOf({ kind: 'passed', output: [] }), [])
  assert.equal(answerOf({ kind: 'failed' }), undefined)
  assert.equal(answerOf({ kind: 'moai-error', message: 'boom' }), undefined)
  assert.equal(answerOf({ kind: 'unreadable', truncated: true }), undefined)
})

/** A bridge whose every start is refused `workspace-busy` (as `answer` arranges) and whose runs pass. */
function refusingBridge(answer) {
  const calls = []
  return {
    calls,
    invokeCommand(command, args) {
      calls.push([command, args])
      if (command === 'buzzni.moai.run') return answer()
      return Promise.resolve({ action: 'status', operationId: args[0], ...passed({}) })
    },
  }
}
const BUSY = { action: 'start', state: 'refused', code: 'workspace-busy' }

test('a busy read cancelled while its start was out is not tried again', async () => {
  let reply
  const bridge = refusingBridge(() => new Promise((resolve) => (reply = resolve)))
  const client = createClient(bridge, { ...PANEL_CLIENT_OPTIONS, sleep: () => flush() })
  const pending = client.request('buzzni.moai.show-all')
  await flush()
  client.cancel(requestKey('buzzni.moai.show-all'))
  reply(BUSY)
  assert.deepEqual(await pending, { kind: 'refused', code: 'workspace-busy' })
  await flush()
  assert.deepEqual(starts(bridge), [['buzzni.moai.show-all']])
})

test('a cancel landing between the busy answer and the requeue ends the read without a second start', async () => {
  let client
  const bridge = refusingBridge(() => {
    const answer = Promise.resolve(BUSY)
    // Two hops on: after the run has read the answer and chosen to retry, before the queue takes it back.
    answer.then(() => queueMicrotask(() => client.cancel(requestKey('buzzni.moai.show-all'))))
    return answer
  })
  client = createClient(bridge, { ...PANEL_CLIENT_OPTIONS, sleep: () => flush() })
  assert.deepEqual(await client.request('buzzni.moai.show-all'), { kind: 'cancelled', mayContinue: false })
  await flush()
  assert.deepEqual(starts(bridge), [['buzzni.moai.show-all']])
})

test('prime counts the tasks the running milestone left out (`outside`) among the rows not shown', () => {
  // `rest` counts over what the milestone let through, so a milestone whose members are all blocked reads
  // ready: [], rest: 0 — and twenty tasks outside it would otherwise read as "nothing here".
  const loaded = rowsFromPrime({ picked: [], ready: [], rest: 0, others: [], others_rest: 0, milestone: ['trk-m1'], outside: ['trk-a', 'trk-b'] })
  assert.equal(loaded.ok, true)
  assert.equal(loaded.value.more, 2)
  assert.equal(rowsFromPrime({ picked: [], ready: [], rest: 1, others_rest: 1, outside: 'not a list' }).value.more, 2)
})

test('which outcomes are information: what the user chose or can retry, never what a machine got wrong', () => {
  assert.deepEqual([...INFO_REFUSALS].sort(), ['approval-timeout', 'declined', 'workspace-busy'])
  for (const code of REFUSAL_CODES) assert.equal(isInformation({ kind: 'refused', code }), INFO_REFUSALS.has(code), code)
  assert.equal(isInformation({ kind: 'not-initialized' }), true)
  assert.equal(isInformation({ kind: 'cancelled', mayContinue: false }), true)
  for (const outcome of [{ kind: 'moai-error', message: 'x' }, { kind: 'failed' }, { kind: 'unreadable' }, { kind: 'not-started' }]) {
    assert.equal(isInformation(outcome), false, outcome.kind)
  }
})

test('a failed run says why from stderr, and from stdout when stderr has nothing', () => {
  assert.deepEqual(outcomeOf({ state: 'failed', stderr: JSON.stringify({ error: 'on stderr' }), stdout: '' }), { kind: 'moai-error', message: 'on stderr' })
  assert.deepEqual(outcomeOf({ state: 'failed', stderr: '', stdout: JSON.stringify({ error: 'on stdout' }) }), { kind: 'moai-error', message: 'on stdout' })
  assert.deepEqual(outcomeOf({ state: 'failed', stderr: 'not json', stdout: '' }), { kind: 'failed' })
})
