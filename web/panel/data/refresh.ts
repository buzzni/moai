/**
 * Reading again when the tracker changes: while the tab is visible the page asks `/api/stamp` every `pollMs`
 * (2 s), and reads the rows only when the stamp moved — a stamp is a hash of the tracker's files, so an unchanged
 * one means nothing to read. A hidden tab (`document.visibilityState`) asks nothing; shown again, it asks at once.
 * A manual refresh (the refresh-time button) reads whatever the stamp says. When `moai web` cannot be reached the
 * page says so once (`onReach(false)`) and asks again every `backoffMs` (10 s); the first answer after that takes the
 * line back (`onReach(true)`) and reads. A server that answers but whose read fails (`load` gives `'failed'`: a tracker
 * moai cannot read, no permission) is asked again every `backoffMs` too — each look is three or four child processes, and
 * the same failure repeating every 2 s helps nobody — and says so once (`onBackoff(true)`) until a read takes. Reads never overlap: a refresh asked for while one runs joins it. A read
 * that did not take (`load` gives `false`) leaves its stamp unseen, so the next look reads again.
 *
 * The stamp is read before the rows, so a write landing between the two is seen as a later change, never lost.
 * Timers and visibility are injected so the schedule is tested without a DOM or real time.
 */
import { partsIn, zoneNow } from './zone.js'

/** Whether the tab is on screen, and a way to hear when that changes. */
export interface VisibilitySignal {
  shown(): boolean
  /** Calls `changed` when `shown()` flips; returns the call that stops watching. */
  watch(changed: () => void): () => void
}

export interface Clock {
  now(): number
  setTimeout(callback: () => void, ms: number): unknown
  clearTimeout(handle: unknown): void
}

export interface RefresherOptions {
  /**
   * `automatic`: the stamp moved (not a person's click). `false` says the read did not take (a refusal, a cancel, a
   * child the server cut off): its stamp is not counted as seen, so the next look reads again rather than leaving the
   * rows of before on screen until someone writes the tracker. `'failed'` is `false` for a read that was answered
   * with a failure (not a cancel, which a person chose and which asks again at the usual pace): the next look waits `backoffMs`.
   */
  load: (how: { automatic: boolean }) => Promise<void | boolean | 'failed'>
  /** `/api/stamp`'s answer, or null when the server could not be reached. */
  stamp: () => Promise<string | null>
  /** The server could (true) or could not (false) be reached — told only when it changes. */
  onReach?: (reached: boolean) => void
  /** Reads keep failing against a server that answers (true) until one takes (false) — told only when it changes. */
  onBackoff?: (backingOff: boolean) => void
  pollMs?: number
  backoffMs?: number
  /** Defaults to the document's `visibilityState` where there is one, and to always shown elsewhere. */
  visibility?: VisibilitySignal | null
  clock?: Clock
}

export interface Refresher {
  /**
   * Reads now, whatever the stamp says (unless a read is running, which it then joins). `fresh` waits for a running
   * read and then reads again: one that began before a write (a read mark) cannot hold what it wrote.
   */
  refresh(how?: { fresh?: boolean }): Promise<void>
  /** Asks the stamp now and reads when it moved. */
  check(): Promise<void>
  stop(): void
  /** When the last read finished, or null before the first. */
  readonly lastLoaded: number | null
}

export const POLL_MS = 2_000
export const BACKOFF_MS = 10_000

const systemClock: Clock = {
  now: () => Date.now(),
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

export function createRefresher(options: RefresherOptions): Refresher {
  const clock = options.clock ?? systemClock
  const pollMs = options.pollMs ?? POLL_MS
  const backoffMs = options.backoffMs ?? BACKOFF_MS
  const signal = options.visibility === undefined ? (typeof document === 'undefined' ? null : documentVisibility(document)) : options.visibility
  const shown = () => signal?.shown() ?? true
  let timer: unknown = null
  let inFlight: Promise<void> | null = null
  let checking: Promise<void> | null = null
  let lastLoaded: number | null = null
  /** The stamp the rows on screen were read at; null before the first read. */
  let seen: string | null = null
  let reached = true
  let failing = false
  let stopped = false

  const clear = () => {
    if (timer !== null) clock.clearTimeout(timer)
    timer = null
  }
  const schedule = (ms: number) => {
    clear()
    if (stopped || !shown()) return
    timer = clock.setTimeout(() => {
      timer = null
      void check()
    }, ms)
  }
  const reach = (now: boolean) => {
    if (now === reached) return
    reached = now
    options.onReach?.(now)
  }

  const fail = (now: boolean) => {
    if (now === failing) return
    failing = now
    options.onBackoff?.(now)
  }

  function read(automatic: boolean, at: string | null): Promise<void> {
    if (inFlight) return inFlight
    inFlight = (async () => {
      try {
        const took = await options.load({ automatic })
        if (at !== null) seen = took === false || took === 'failed' ? null : at
        // A cancel (`false`) says nothing about the tracker, and neither does a read sent while the stamp went unanswered
        // (`at` null — a person's refresh with the server out of reach, which `onReach(false)` says already): the line
        // stays as it was. It is for a server that answers.
        if (took !== false && at !== null) fail(took === 'failed')
      } finally {
        lastLoaded = clock.now()
        inFlight = null
      }
    })()
    return inFlight
  }

  function check(): Promise<void> {
    if (checking) return checking
    checking = (async () => {
      try {
        if (stopped || !shown()) return
        const at = await options.stamp()
        if (at === null) {
          reach(false)
          return schedule(backoffMs)
        }
        const back = !reached
        reach(true)
        if (at !== seen || back) await read(true, at)
        schedule(failing ? backoffMs : pollMs)
      } finally {
        checking = null
      }
    })()
    return checking
  }

  async function refresh(how: { fresh?: boolean } = {}): Promise<void> {
    if (how.fresh && inFlight) await inFlight.catch(() => {})
    // The stamp first, so a write after it is a later change and is read on the next look.
    const at = await options.stamp()
    reach(at !== null)
    await read(false, at)
    schedule(at === null || failing ? backoffMs : pollMs)
  }

  const unwatch = signal?.watch(() => {
    if (stopped) return
    if (shown()) void check()
    else clear()
  })

  return {
    refresh,
    check,
    stop() {
      stopped = true
      clear()
      unwatch?.()
    },
    get lastLoaded() {
      return lastLoaded
    },
  }
}

/** The tab's own visibility: `document.visibilityState`, heard through `visibilitychange`. */
export function documentVisibility(doc: Document): VisibilitySignal {
  return {
    shown: () => doc.visibilityState !== 'hidden',
    watch(changed) {
      doc.addEventListener('visibilitychange', changed)
      return () => doc.removeEventListener('visibilitychange', changed)
    },
  }
}

/** `HH:MM:SS` in the zone the page draws in (`SPC o t`, data/zone.ts; `zone` when given), as the tab bar shows it. */
export function refreshTime(at: number, _locale?: string, zone: string | undefined = zoneNow()): string {
  const { hour, minute, second } = partsIn(at, zone)
  const two = (n: number) => String(n).padStart(2, '0')
  return `${two(hour)}:${two(minute)}:${two(second)}`
}
