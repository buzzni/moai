/**
 * The request queue: every read the page makes of `moai web` goes through here. There is no global busy lock —
 * each request carries its own phase, so a slow list refresh does not lock the detail.
 *
 * - A request is one GET of an endpoint ({@link Endpoint}: a path and its query) → the envelope the server wraps
 *   the child `moai --json …` in (`{exit, out, err, said}`) → an {@link Outcome} ({@link outcomeOf}).
 * - A read already queued or running with the same endpoint is shared, not fetched twice — unless the caller counts
 *   a change since that one was asked ({@link RequestOptions.epoch}) and it has already started.
 * - At most `concurrency` reads are in flight; the rest wait in priority order (detail before wiki before list).
 * - Moai ends a partial answer non-zero yet still prints it whole on stdout (an unreadable line in the tracker, a
 *   wiki page it skipped). Such a read is `failed` with the parsed stdout as `output`, and {@link answerOf} hands out
 *   either kind of answer.
 * - The lines moai said on stderr beside its answer (`said`: a line it could not read, a notice) ride the outcome as
 *   `notes`, so the page tells them rather than swallowing them.
 */

/** One endpoint of `moai web` and its query (src/web/api.rs PROFILES). */
export interface Endpoint {
  path: string
  query?: Readonly<Record<string, string>>
}

/** The `fetch` the client calls — the browser's, or a test's. */
export type Fetch = (url: string, init: { signal: AbortSignal; headers: Record<string, string> }) => Promise<{ status: number; text(): Promise<string> }>

export type Outcome =
  | { kind: 'passed'; output: unknown; notes?: string[] }
  /** The server could not be reached: `moai web` stopped, or the network between. */
  | { kind: 'not-started' }
  /**
   * Moai, or the server, said no: `message` is its words. The server's own refusals (4xx·5xx `{error, code}`) and a
   * child that ended non-zero with no answer on stdout both land here.
   */
  | { kind: 'moai-error'; message: string; code?: string; notes?: string[] }
  /** `output`: the stdout of a read Moai ended non-zero on a partial answer; `message` what it said why. */
  | { kind: 'failed'; output?: unknown; message?: string; notes?: string[] }
  /** The answer was not the envelope, or not JSON. */
  | { kind: 'unreadable' }
  | { kind: 'cancelled' }

/** Whether an outcome is information (a choice the person made) rather than an error. */
export function isInformation(outcome: Outcome): boolean {
  return outcome.kind === 'cancelled'
}

export type Phase = 'queued' | 'running'

/** What the page runs with: 3 reads at once (the list is three of them). */
export const PANEL_CLIENT_OPTIONS = { concurrency: 3 } as const

/** Lower runs first. */
export const PRIORITY = { detail: 0, wiki: 1, list: 2, background: 3 } as const

export interface RequestState {
  key: string
  path: string
  priority: number
  phase: Phase
  share: boolean
}

export interface RequestOptions {
  priority?: number
  /** A read that must not be handed someone else's answer. */
  share?: boolean
  /**
   * How many changes of the tracker the caller had seen through when it asked (default 0). A read is shared only with
   * a run of it asked at that count or a later one, or with one still queued (it has not started): a read asked after
   * a change is never handed the answer of a run that began before it.
   */
  epoch?: number
}

export interface ClientOptions {
  concurrency?: number
  /**
   * How long one read may go unanswered before it counts as the server out of reach (`not-started`). The server
   * cuts its own answers at a minute; past that a read waited on would hold its slot, and the refresh behind it,
   * for good. Default 90 s.
   */
  timeoutMs?: number
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null

const parse = (text: string): unknown => {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

const notesOf = (said: unknown): string[] | undefined => {
  const lines = Array.isArray(said) ? said.filter((line): line is string => typeof line === 'string' && line !== '') : []
  return lines.length ? lines : undefined
}

/**
 * What one answer of the server says. 4xx·5xx is the server's own refusal; 200 carries the child's envelope:
 * exit 0 with `out` passed, non-zero with `out` a partial answer, non-zero without `out` moai's error.
 */
export function outcomeOf(status: number, body: string): Outcome {
  const value = parse(body)
  if (status !== 200) {
    if (isRecord(value) && typeof value.error === 'string') {
      return typeof value.code === 'string' ? { kind: 'moai-error', message: value.error, code: value.code } : { kind: 'moai-error', message: value.error }
    }
    return { kind: 'unreadable' }
  }
  if (!isRecord(value) || typeof value.exit !== 'number') return { kind: 'unreadable' }
  const notes = notesOf(value.said)
  const err = isRecord(value.err) ? value.err : undefined
  const message = typeof err?.error === 'string' ? err.error : undefined
  const withNotes = <T extends Outcome>(outcome: T): T => (notes ? { ...outcome, notes } : outcome)
  const has = Object.hasOwn(value, 'out')
  if (value.exit === 0) return has ? withNotes({ kind: 'passed', output: value.out }) : { kind: 'unreadable' }
  if (has) return withNotes(message === undefined ? { kind: 'failed', output: value.out } : { kind: 'failed', output: value.out, message })
  if (message !== undefined) {
    const code = typeof err?.code === 'string' ? err.code : undefined
    return withNotes(code === undefined ? { kind: 'moai-error', message } : { kind: 'moai-error', message, code })
  }
  return { kind: 'unreadable' }
}

/**
 * What a read answered: a pass's output, or the whole answer Moai printed before it ended non-zero on a partial
 * one (see the header). Undefined when the read gave no answer.
 */
export function answerOf(outcome: Outcome): unknown {
  return outcome.kind === 'passed' || outcome.kind === 'failed' ? outcome.output : undefined
}

/** The URL of an endpoint: its query in name order, so one read is one key. */
export function urlOf(endpoint: Endpoint): string {
  const query = Object.entries(endpoint.query ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
  return query.length ? `${endpoint.path}?${new URLSearchParams(query).toString()}` : endpoint.path
}

/** The queue's key for an endpoint — its URL. */
export const requestKey = urlOf

interface Pending extends RequestState {
  url: string
  seq: number
  promise: Promise<Outcome>
  resolve: (outcome: Outcome) => void
  abort: AbortController
  /** The latest {@link RequestOptions.epoch} it answers for. */
  epoch: number
}

export interface Client {
  request(endpoint: Endpoint, options?: RequestOptions): Promise<Outcome>
  /** Cancels a queued request at once, or aborts a running one. */
  cancel(key: string): void
  /**
   * Cancels every request, queued and running, in one step. Cancelling them key by key would free a running one's
   * slot first, and the queue would start the next waiting read — sent to the server — before its own turn came.
   */
  cancelAll(): void
  requests(): RequestState[]
  subscribe(listener: () => void): () => void
}

export function createClient(fetcher: Fetch, options: ClientOptions = {}): Client {
  const concurrency = Math.max(1, options.concurrency ?? 1)
  const timeoutMs = options.timeoutMs ?? 90_000
  const pending: Pending[] = []
  const listeners = new Set<() => void>()
  let seq = 0

  // A subscriber is view code. If it throws, the queue must still go on: the next request is started after a
  // notify, so an exception here would leave it queued (and the one running 'running') for good.
  const notify = () => {
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch (error) {
        console.error(error)
      }
    }
  }
  const running = () => pending.filter((entry) => entry.phase !== 'queued').length

  const settle = (entry: Pending, outcome: Outcome) => {
    const at = pending.indexOf(entry)
    if (at < 0) return
    pending.splice(at, 1)
    entry.resolve(outcome)
    notify()
    pump()
  }

  async function execute(entry: Pending): Promise<Outcome> {
    let late = false
    const limit = setTimeout(() => {
      late = true
      entry.abort.abort()
    }, timeoutMs)
    // Aborted by the time limit is out of reach; aborted by a person is a cancel.
    const failed = (): Outcome => (entry.abort.signal.aborted && !late ? { kind: 'cancelled' } : { kind: 'not-started' })
    try {
      const answer = await fetcher(entry.url, { signal: entry.abort.signal, headers: { Accept: 'application/json' } })
      return outcomeOf(answer.status, await answer.text())
    } catch {
      return failed()
    } finally {
      clearTimeout(limit)
    }
  }

  function pump() {
    while (running() < concurrency) {
      const next = pending.filter((entry) => entry.phase === 'queued').sort((a, b) => a.priority - b.priority || a.seq - b.seq)[0]
      if (!next) return
      next.phase = 'running'
      notify()
      void execute(next).then((outcome) => settle(next, outcome))
    }
  }

  return {
    request(endpoint, requestOptions = {}) {
      const url = urlOf(endpoint)
      const priority = requestOptions.priority ?? PRIORITY.list
      const epoch = requestOptions.epoch ?? 0
      if (requestOptions.share !== false) {
        // The latest one: a read asked after a change may stand beside an older one still running.
        const shared = pending.filter((entry) => entry.key === url && entry.share && !entry.abort.signal.aborted && (entry.phase === 'queued' || entry.epoch >= epoch)).at(-1)
        if (shared) {
          // Someone now waits on it sooner: a queued shared request moves up.
          if (priority < shared.priority) {
            shared.priority = priority
            notify()
          }
          // A queued one starts after the changes this caller has seen through, so it answers for them too.
          if (epoch > shared.epoch) shared.epoch = epoch
          return shared.promise
        }
      }
      let resolve!: (outcome: Outcome) => void
      const promise = new Promise<Outcome>((done) => (resolve = done))
      const entry: Pending = {
        key: url, url, path: endpoint.path, priority, phase: 'queued',
        seq: seq++, promise, resolve, abort: new AbortController(), share: requestOptions.share !== false, epoch,
      }
      pending.push(entry)
      notify()
      pump()
      return promise
    },
    cancel(key) {
      for (const entry of pending.filter((candidate) => candidate.key === key)) {
        entry.abort.abort()
        // A queued one never ran; a running one ends as soon as its fetch hears the abort, but the page stops
        // waiting now.
        settle(entry, { kind: 'cancelled' })
      }
    },
    cancelAll() {
      const all = pending.splice(0)
      for (const entry of all) {
        entry.abort.abort()
        entry.resolve({ kind: 'cancelled' })
      }
      if (all.length) notify()
    },
    requests() {
      return pending.map(({ key, path, priority, phase, share }) => ({ key, path, priority, phase, share }))
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
