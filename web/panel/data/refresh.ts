/**
 * Automatic refresh: while the panel is visible the list is read again every `intervalMs` (10 s by
 * default, se-24ey.14i; 0 turns it off); while it is hidden it does not poll, and becoming visible after the interval has passed reads at
 * once. Visible means both `isVisible` and the `visibility` signal say so. The host sends no visibility signal
 * of its own, so in a page the default signal is `panelVisibility()`: `document.visibilityState` follows the
 * Desktop window, and an inactive tool tab stays mounted with the panel's iframe under a hidden parent, where
 * its viewport goes 0×0 — so a 0×0 viewport counts as hidden too. A timer that falls due while hidden reads
 * nothing and looks again every HIDDEN_PROBE_MS until the panel is shown, unless the signal's change starts the
 * schedule again first. A manual refresh (the refresh-time button) reads now and restarts the interval. Reads never overlap:
 * a refresh asked for while one runs waits for it instead of starting another. After a write, ask for a
 * `fresh` read: a read that started before the write cannot hold its result, so that one is waited out
 * and a new read follows.
 *
 * Timers and visibility are injected so the schedule is tested without a DOM or real time.
 */

/** Whether the panel is on screen, and a way to hear when that changes. */
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
  /** `automatic`: the clock (or the panel shown again) started this read, not a person or a write. */
  load: (how: { automatic: boolean }) => Promise<void>
  intervalMs?: number
  isVisible?: () => boolean
  /** Defaults to `panelVisibility()` where there is a document, and to none elsewhere; `null` turns it off. */
  visibility?: VisibilitySignal | null
  clock?: Clock
}

export interface Refresher {
  /**
   * Reads now (unless a read is running, which it then joins) and restarts the interval. With `fresh`, a
   * running read is waited out and another one follows it.
   */
  refresh(options?: { fresh?: boolean }): Promise<void>
  /** Tell it the panel's visibility changed (the `visibility` signal does this by itself). */
  visibilityChanged(): void
  setInterval(ms: number): void
  stop(): void
  /** When the last read finished, or null before the first. */
  readonly lastLoaded: number | null
}

export const DEFAULT_INTERVAL_MS = 10_000

/** How often a panel hidden by a 0×0 viewport looks at its size again, in case no resize event reaches it. */
export const HIDDEN_PROBE_MS = 5_000

const systemClock: Clock = {
  now: () => Date.now(),
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

export function createRefresher(options: RefresherOptions): Refresher {
  const clock = options.clock ?? systemClock
  const signal = options.visibility === undefined ? (typeof document === 'undefined' ? null : panelVisibility()) : options.visibility
  const isVisible = () => (options.isVisible?.() ?? true) && (signal?.shown() ?? true)
  let interval = options.intervalMs ?? DEFAULT_INTERVAL_MS
  let timer: unknown = null
  let inFlight: Promise<void> | null = null
  let lastLoaded: number | null = null
  let stopped = false

  const clear = () => {
    if (timer !== null) clock.clearTimeout(timer)
    timer = null
  }

  const schedule = () => {
    clear()
    if (stopped || !isVisible() || interval <= 0) return
    const since = lastLoaded === null ? interval : clock.now() - lastLoaded
    timer = clock.setTimeout(due, Math.max(0, interval - since))
  }

  /**
   * The interval is up. Hidden without a word to `visibilityChanged`, it reads nothing and looks again every
   * HIDDEN_PROBE_MS until the panel is shown: a frame under a hidden parent may hear no event as it hides, and then
   * the signal hears no change as it is shown again either. A word from the signal (`visibilityChanged`) takes over.
   */
  const due = () => {
    timer = null
    if (isVisible()) background()
    else if (!stopped) timer = clock.setTimeout(due, HIDDEN_PROBE_MS)
  }

  // A read nobody awaits must not end as an unhandled rejection; the load reports its own failures.
  const background = () => void run({}, true).catch(() => {})

  function refresh(how: { fresh?: boolean } = {}): Promise<void> {
    return run(how, false)
  }

  function run(how: { fresh?: boolean }, automatic: boolean): Promise<void> {
    if (inFlight && how.fresh) {
      const after = () => run({}, automatic)
      return inFlight.then(after, after)
    }
    if (inFlight) return inFlight
    clear()
    inFlight = (async () => {
      try {
        await options.load({ automatic })
      } finally {
        lastLoaded = clock.now()
        inFlight = null
        schedule()
      }
    })()
    return inFlight
  }

  const visibilityChanged = () => {
    if (stopped) return
    if (!isVisible()) return clear()
    if (inFlight) return
    // Off (interval 0) stays off: being shown again reads nothing either.
    if (interval > 0 && lastLoaded !== null && clock.now() - lastLoaded >= interval) background()
    else schedule()
  }
  const unwatch = signal?.watch(visibilityChanged)

  return {
    refresh,
    visibilityChanged,
    setInterval(ms) {
      interval = ms
      if (!inFlight) schedule()
    },
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

/**
 * The panel's own visibility signal: shown while the document is not hidden and the viewport is not 0×0.
 * Desktop hides an inactive tool tab's parent, which leaves the iframe mounted with a 0×0 viewport and
 * `visibilityState` still following the window. It listens to `visibilitychange`, `resize` and a
 * ResizeObserver on the root element where there is one; while the viewport is 0×0 under a visible document
 * it also looks again every `probeMs`, since a frame under a hidden parent may hear no event. Whether
 * Electron does shrink and throttle the hidden iframe this way is checked live by spike se-0g4a.zrf.
 */
export function panelVisibility(win: Window = window, probeMs = HIDDEN_PROBE_MS): VisibilitySignal {
  const doc = win.document
  const shown = () => doc.visibilityState !== 'hidden' && win.innerWidth > 0 && win.innerHeight > 0
  return {
    shown,
    watch(changed) {
      let last = shown()
      let probe: number | null = null
      const check = () => {
        const now = shown()
        if (now !== last) {
          last = now
          changed()
        }
        if (probe !== null) win.clearTimeout(probe)
        probe = !now && doc.visibilityState !== 'hidden' ? win.setTimeout(() => ((probe = null), check()), probeMs) : null
      }
      doc.addEventListener('visibilitychange', check)
      win.addEventListener('resize', check)
      const Observer = (win as Window & { ResizeObserver?: typeof ResizeObserver }).ResizeObserver
      const observer = Observer ? new Observer(check) : null
      observer?.observe(doc.documentElement)
      check()
      return () => {
        doc.removeEventListener('visibilitychange', check)
        win.removeEventListener('resize', check)
        observer?.disconnect()
        if (probe !== null) win.clearTimeout(probe)
        probe = null
      }
    },
  }
}

/** `HH:MM:SS` in the panel's locale and time zone, as the tab bar shows it. */
export function refreshTime(at: number, locale?: string): string {
  return new Date(at).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })
}
