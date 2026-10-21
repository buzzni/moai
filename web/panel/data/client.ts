/**
 * The request queue: every Moai run the panel starts goes through here. There is no global busy lock —
 * each request carries its own phase, so a slow list refresh does not lock the detail or the add field.
 *
 * - A request is `start` (`buzzni.moai.run`) → poll (`buzzni.moai.status` every `pollMs`) → parse.
 * - A read already queued or running with the same profile and value is shared, not started twice — unless the caller
 *   counts a write since that one was asked ({@link RequestOptions.epoch}) and it has already started.
 * - At most `concurrency` runs are in flight; the rest wait in priority order (detail before wiki before
 *   list). The spike (se-0g4a.fjl) found nothing in the SDK, Desktop or daemon that serialises read-only
 *   profiles, so the panel is to run up to 3 at once ({@link PANEL_CLIENT_OPTIONS}) until Desktop is
 *   measured live.
 * - The daemon refuses an overlapping write-scope run at once with `workspace-busy` instead of waiting. A
 *   shared read refused so goes back to the queue after a backoff that grows by `busyBackoffMs` each time
 *   (1×, 2×, 3× …) and is tried up to `busyAttempts` times in all before it resolves refused. A write
 *   (`share: false`) is never retried: whether to send it again is the user's call.
 * - The bare defaults are one run at a time and no busy retry (the 1.x behaviour); the panel runs on
 *   `createClient(bridge, PANEL_CLIENT_OPTIONS)` in main.ts.
 * - The daemon cuts stdout at the profile's `outputLimitBytes` and says so with `truncated: true` (a flag
 *   on a finished status, not an error code). The outcome carries it as `truncated: true` on the kind it
 *   would have anyway: cut JSON does not parse, so a cut list is `unreadable` with `truncated: true`
 *   (every area already explains `unreadable`, and a reader that has a fallback, like the index rows,
 *   looks for the flag); if the stdout still parses (only stderr was cut) it is `passed` with the flag.
 * - Core's refusal codes, Moai's own errors and a missing `.moai/` come back as an {@link Outcome}, so each
 *   area of the panel can explain its own failure.
 * - Moai ends a partial answer non-zero yet still prints it whole on stdout (an unreadable line in the
 *   tracker, a wiki page it skipped, an id `read` could not find; only `prime` ends those with 0). Such a
 *   run is `failed` with the parsed stdout as `output`, and {@link answerOf} hands out either kind of answer.
 */

export interface Bridge {
  invokeCommand(command: string, args: unknown[]): Promise<unknown>
}

export const COMMAND = {
  run: 'buzzni.moai.run',
  status: 'buzzni.moai.status',
  cancel: 'buzzni.moai.cancel',
} as const

/** Refusals Core explains with a code; the extension returns them as `{state: 'refused', code}`. */
export const REFUSAL_CODES = ['declined', 'approval-timeout', 'workspace-busy', 'tool-missing', 'unsupported-platform', 'unsupported-daemon'] as const
export type RefusalCode = (typeof REFUSAL_CODES)[number]
/** Refusals the user chose or can simply retry: information, not errors. */
export const INFO_REFUSALS: ReadonlySet<RefusalCode> = new Set(['declined', 'approval-timeout', 'workspace-busy'])

export type Outcome =
  /** `truncated`: the daemon flagged a cut at the output limit, yet the stdout still parsed. */
  | { kind: 'passed'; output: unknown; truncated?: true }
  /**
   * `assumed`: no run asked for it — the panel answered it itself, taking the profile as missing along with the whole
   * list (main.ts, compat.ts lacks). It stops counting once the whole list runs there.
   */
  | { kind: 'refused'; code: RefusalCode; assumed?: true }
  /**
   * The start threw, or came back without an operation (an older Desktop, an unknown refusal). `code` is the
   * refusal code the start named, when it did: a `refused` code the panel does not list (`binding-mismatch` — the
   * extension answers every SDK refusal code), or one a thrown start's message names (a host that forwards it).
   */
  | { kind: 'not-started'; code?: string }
  /** Moai says this folder has no `.moai/`; the panel cannot run `moai init` itself. */
  | { kind: 'not-initialized' }
  | { kind: 'moai-error'; message: string }
  /** `output`: the stdout of a run Moai ended non-zero on a partial answer, when it still parsed. */
  | { kind: 'failed'; output?: unknown }
  /** It passed but stdout was not JSON; `truncated` when that is because the daemon cut it at the output limit. */
  | { kind: 'unreadable'; truncated?: true }
  | { kind: 'cancelled'; mayContinue: boolean }

/** Whether an outcome is information (a choice the user made, a retry, a board not made yet) rather than an error. */
export function isInformation(outcome: Outcome): boolean {
  switch (outcome.kind) {
    case 'refused':
      return INFO_REFUSALS.has(outcome.code)
    case 'not-initialized':
    case 'cancelled':
      return true
    default:
      return false
  }
}

export type Phase = 'queued' | 'starting' | 'running'

/** What the panel is to run with (se-0g4a.fjl): 3 reads at once, a busy read tried 3 times, 1 s then 2 s apart. */
export const PANEL_CLIENT_OPTIONS = { concurrency: 3, busyAttempts: 3, busyBackoffMs: 1000 } as const

/** Lower runs first. */
export const PRIORITY = { detail: 0, wiki: 1, list: 2, background: 3 } as const

export interface RequestState {
  key: string
  profileId: string
  value: string | undefined
  priority: number
  phase: Phase
  /** Known once the run is accepted; until then Desktop may be showing its confirmation. */
  operationId: string | null
  /** A read, shared by everyone who asks for it; false for a write (an add, a read mark). */
  share: boolean
}

export interface RequestOptions {
  priority?: number
  /** Writes are never shared: two adds of the same title are two adds. */
  share?: boolean
  /**
   * How many of its writes the caller had seen through when it asked (default 0). A read is shared only with a run of it
   * asked at that count or a later one, or with one still queued (it has not started): a read asked after a write is
   * never handed the answer of a run that began before it.
   */
  epoch?: number
}

export interface ClientOptions {
  pollMs?: number
  concurrency?: number
  /** Tries in all for a shared read refused with `workspace-busy` (default 1: no retry). */
  busyAttempts?: number
  /** Wait before the first retry; each later one waits that much longer (default 1000 ms). */
  busyBackoffMs?: number
  sleep?: (ms: number) => Promise<void>
}

const NOT_A_REPOSITORY = /Not a moai repository/

const parse = (text: unknown): unknown => {
  if (typeof text !== 'string') return null
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null

/** What a finished status says, read the way the 1.x panel read it. */
export function outcomeOf(finished: unknown): Outcome {
  if (!isRecord(finished)) return { kind: 'failed' }
  if (finished.state === 'cancelled') return { kind: 'cancelled', mayContinue: finished.remoteMayContinue === true }
  const output = parse(finished.stdout ?? '')
  const truncated = finished.truncated === true
  if (finished.state === 'passed') {
    if (output === null) return truncated ? { kind: 'unreadable', truncated: true } : { kind: 'unreadable' }
    return truncated ? { kind: 'passed', output, truncated: true } : { kind: 'passed', output }
  }
  // Moai prints its JSON error on stderr.
  const problem = parse(finished.stderr ?? '') ?? output
  const reason = isRecord(problem) && typeof problem.error === 'string' ? problem.error : null
  if (reason && NOT_A_REPOSITORY.test(reason)) return { kind: 'not-initialized' }
  if (reason) return { kind: 'moai-error', message: reason }
  return output === null ? { kind: 'failed' } : { kind: 'failed', output }
}

/** The refusal code an outcome names: a refusal's, or the one a start that failed named (`not-started`). */
export const refusalCode = (outcome: Outcome | null): string | undefined =>
  outcome?.kind === 'refused' || outcome?.kind === 'not-started' ? outcome.code : undefined

/**
 * What a run answered: a pass's output, or the whole answer Moai printed before it ended non-zero on a partial
 * one (see the header). Undefined when the run gave no answer.
 */
export function answerOf(outcome: Outcome): unknown {
  return outcome.kind === 'passed' || outcome.kind === 'failed' ? outcome.output : undefined
}

/** What a start answer says, or null when it was accepted with an operation id. */
export function refusalOf(started: unknown): Outcome | null {
  if (isRecord(started) && started.state === 'refused') {
    const code = started.code
    return typeof code === 'string' && (REFUSAL_CODES as readonly string[]).includes(code)
      ? { kind: 'refused', code: code as RefusalCode }
      : typeof code === 'string' && code !== '' ? { kind: 'not-started', code } : { kind: 'not-started' }
  }
  if (!isRecord(started) || started.state !== 'accepted' || typeof started.operationId !== 'string') return { kind: 'not-started' }
  return null
}

interface Pending extends RequestState {
  seq: number
  promise: Promise<Outcome>
  resolve: (outcome: Outcome) => void
  cancelled: boolean
  /** Starts tried so far. */
  attempts: number
  /** Waiting out a `workspace-busy` backoff: queued, but not to be picked yet. */
  held: boolean
  /** The latest {@link RequestOptions.epoch} it answers for. */
  epoch: number
}

/** A shared read Core refused as busy, to be queued again. */
const RETRY = Symbol('retry')

export interface Client {
  request(profileId: string, value?: string, options?: RequestOptions): Promise<Outcome>
  /** Cancels a queued request at once, or asks the machine to cancel a running one. */
  cancel(key: string): void
  requests(): RequestState[]
  subscribe(listener: () => void): () => void
}

export const requestKey = (profileId: string, value?: string): string => (value === undefined ? profileId : `${profileId}\u0000${value}`)

export function createClient(bridge: Bridge, options: ClientOptions = {}): Client {
  const pollMs = options.pollMs ?? 500
  const concurrency = Math.max(1, options.concurrency ?? 1)
  const busyAttempts = Math.max(1, options.busyAttempts ?? 1)
  const busyBackoffMs = options.busyBackoffMs ?? 1000
  const sleep = options.sleep ?? ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)))
  const pending: Pending[] = []
  const listeners = new Set<() => void>()
  let seq = 0

  // A subscriber is view code. If it throws, the queue must still go on: the next request is started after a
  // notify, so an exception here would leave it queued (and the one running 'starting' or 'running') for good.
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
    if (at >= 0) pending.splice(at, 1)
    entry.resolve(outcome)
    notify()
    pump()
  }

  async function poll(operationId: string): Promise<unknown> {
    for (;;) {
      const result = await bridge.invokeCommand(COMMAND.status, [operationId])
      if (!isRecord(result) || result.state !== 'running') return result
      await sleep(pollMs)
    }
  }

  async function execute(entry: Pending): Promise<Outcome | typeof RETRY> {
    entry.attempts += 1
    let started: unknown
    try {
      started = await bridge.invokeCommand(COMMAND.run, entry.value === undefined ? [entry.profileId] : [entry.profileId, entry.value])
    } catch (error) {
      const named = /\b(binding-mismatch|unsupported-daemon)\b/.exec(error instanceof Error ? error.message : String(error))
      return named ? { kind: 'not-started', code: named[1] } : { kind: 'not-started' }
    }
    const refusal = refusalOf(started)
    if (refusal) {
      const busy = refusal.kind === 'refused' && refusal.code === 'workspace-busy'
      return busy && entry.share && !entry.cancelled && entry.attempts < busyAttempts ? RETRY : refusal
    }
    entry.operationId = (started as { operationId: string }).operationId
    entry.phase = 'running'
    notify()
    // A cancel asked for while the confirmation was open is sent as soon as there is something to cancel.
    if (entry.cancelled) sendCancel(entry)
    try {
      return outcomeOf(await poll(entry.operationId))
    } catch {
      return { kind: 'failed' }
    }
  }

  function pump() {
    while (running() < concurrency) {
      const next = pending.filter((entry) => entry.phase === 'queued' && !entry.held).sort((a, b) => a.priority - b.priority || a.seq - b.seq)[0]
      if (!next) return
      next.phase = 'starting'
      notify()
      void execute(next).then((outcome) => (outcome === RETRY ? requeue(next) : settle(next, outcome)))
    }
  }

  /** Back to the queue after a backoff; the slot is free meanwhile. A cancel while it waits settles it. */
  function requeue(entry: Pending) {
    // A cancel that landed after the busy answer but before this ran: nothing is running, so it ends here
    // instead of being started again after the backoff.
    if (entry.cancelled) return settle(entry, { kind: 'cancelled', mayContinue: false })
    entry.phase = 'queued'
    entry.held = true
    notify()
    pump()
    void sleep(busyBackoffMs * entry.attempts).then(() => {
      entry.held = false
      if (pending.includes(entry)) pump()
    })
  }

  function sendCancel(entry: Pending) {
    if (entry.operationId) void bridge.invokeCommand(COMMAND.cancel, [entry.operationId]).catch(() => {})
  }

  return {
    request(profileId, value, requestOptions = {}) {
      const key = requestKey(profileId, value)
      const priority = requestOptions.priority ?? PRIORITY.list
      const epoch = requestOptions.epoch ?? 0
      if (requestOptions.share !== false) {
        // The latest one: a run asked after a write may stand beside an older one still running.
        const shared = pending.filter((entry) => entry.key === key && !entry.cancelled && entry.share && (entry.phase === 'queued' || entry.epoch >= epoch)).at(-1)
        if (shared) {
          // Someone now waits on it sooner: a queued shared request moves up.
          if (priority < shared.priority) {
            shared.priority = priority
            notify()
          }
          // A queued one starts after the writes this caller has seen through, so it answers for them too.
          if (epoch > shared.epoch) shared.epoch = epoch
          return shared.promise
        }
      }
      let resolve!: (outcome: Outcome) => void
      const promise = new Promise<Outcome>((done) => (resolve = done))
      const entry: Pending = {
        key, profileId, value, priority, phase: 'queued', operationId: null,
        seq: seq++, promise, resolve, cancelled: false, share: requestOptions.share !== false, attempts: 0, held: false, epoch,
      }
      pending.push(entry)
      notify()
      pump()
      return promise
    },
    cancel(key) {
      for (const entry of pending.filter((candidate) => candidate.key === key)) {
        if (entry.phase === 'queued') {
          settle(entry, { kind: 'cancelled', mayContinue: false })
          continue
        }
        entry.cancelled = true
        sendCancel(entry)
      }
    },
    requests() {
      return pending.map(({ key, profileId, value, priority, phase, operationId, share }) => ({ key, profileId, value, priority, phase, operationId, share }))
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
