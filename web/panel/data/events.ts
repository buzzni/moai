/** A visible tab keeps one server event stream; polling takes over when it fails. */
import { documentVisibility, type Clock, type VisibilitySignal } from './refresh.js'

export interface EventsOptions {
  url: () => string
  onStamp: (stamp: string) => void | Promise<void>
  onUp: () => void
  onDown: () => void
  onVisible?: () => void
  onHello?: (hello: { tab: string; token: string }) => void
  onCtl?: (message: { seq: number; request: Record<string, unknown> }) => void
  onValidateZone?: (message: { seq: number; name: string }) => void
  onProposal?: (entry: { id: string; text: string; then?: { verb: string; [key: string]: unknown } | null; remaining_ms?: number }) => void
  onProposalClosed?: (id: string) => void
  factory?: (url: string) => EventSource
  visibility?: VisibilitySignal | null
  clock?: Clock
  retryMs?: number
}

export interface Events {
  reconnect(): void
  stop(): void
}

const RETRY_MS = 30_000
/**
 * A stream that was open and then ended — the server closes each after thirty minutes, or the connection dropped — is
 * opened again after three seconds (the `retry:` the server sends), well inside the server's thirty-second resume grace
 * (src/web/ctl.rs RESUME_GRACE), so the tab keeps its ctl id, token and pending suggestions. Waiting RETRY_MS there
 * always arrived after the grace. A stream refused before it opened still waits RETRY_MS.
 */
const RESUME_MS = 3_000
const clockDefault: Clock = {
  now: () => Date.now(),
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

export function openEvents(options: EventsOptions): Events {
  const clock = options.clock ?? clockDefault
  const visibility = options.visibility === undefined
    ? (typeof document === 'undefined' ? null : documentVisibility(document))
    : options.visibility
  const shown = () => visibility?.shown() ?? true
  const factory = options.factory ?? (typeof EventSource === 'undefined' ? null : (url: string) => new EventSource(url))
  let source: EventSource | null = null
  let retry: unknown = null
  let stopped = false
  let up = false

  const clear = () => {
    if (retry !== null) clock.clearTimeout(retry)
    retry = null
  }
  const down = () => {
    if (!up) return
    up = false
    options.onDown()
  }
  const close = () => {
    source?.close()
    source = null
    down()
  }
  const later = (ms = options.retryMs ?? RETRY_MS) => {
    clear()
    if (stopped || !shown() || !factory) return
    retry = clock.setTimeout(() => {
      retry = null
      start()
    }, ms)
  }
  const start = () => {
    if (stopped || !shown() || source || !factory) return
    try {
      const opened = factory(options.url())
      source = opened
      opened.onopen = () => {
        if (source !== opened) return
        up = true
        clear()
        options.onUp()
      }
      opened.addEventListener('stamp', (event) => {
        if (source !== opened) return
        try {
          const stamp = (JSON.parse((event as MessageEvent).data) as { stamp?: unknown }).stamp
          if (typeof stamp === 'string') void Promise.resolve(options.onStamp(stamp)).catch(() => options.onDown())
        } catch { /* a broken event cannot replace the visible rows */ }
      })
      opened.addEventListener('hello', (event) => {
        if (source !== opened) return
        try {
          const hello = JSON.parse((event as MessageEvent).data) as { tab?: unknown; token?: unknown }
          if (typeof hello.tab === 'string' && typeof hello.token === 'string') options.onHello?.({ tab: hello.tab, token: hello.token })
        } catch { /* ignore a malformed server event */ }
      })
      opened.addEventListener('ctl', (event) => {
        if (source !== opened) return
        try {
          const message = JSON.parse((event as MessageEvent).data) as { seq?: unknown; request?: unknown }
          if (typeof message.seq === 'number' && message.request && typeof message.request === 'object') {
            options.onCtl?.({ seq: message.seq, request: message.request as Record<string, unknown> })
          }
        } catch { /* ignore a malformed server event */ }
      })
      opened.addEventListener('validate_zone', (event) => {
        if (source !== opened) return
        try {
          const message = JSON.parse((event as MessageEvent).data) as { seq?: unknown; name?: unknown }
          if (typeof message.seq === 'number' && typeof message.name === 'string') options.onValidateZone?.({ seq: message.seq, name: message.name })
        } catch { /* ignore a malformed server event */ }
      })
      opened.addEventListener('proposal', (event) => {
        if (source !== opened) return
        try {
          const entry = JSON.parse((event as MessageEvent).data) as { id?: unknown; text?: unknown; then?: unknown; remaining_ms?: unknown }
          if (typeof entry.id === 'string' && typeof entry.text === 'string') options.onProposal?.(entry as Parameters<NonNullable<EventsOptions['onProposal']>>[0])
        } catch { /* ignore a malformed server event */ }
      })
      opened.addEventListener('proposal_closed', (event) => {
        if (source !== opened) return
        try {
          const entry = JSON.parse((event as MessageEvent).data) as { id?: unknown }
          if (typeof entry.id === 'string') options.onProposalClosed?.(entry.id)
        } catch { /* ignore a malformed server event */ }
      })
      opened.onerror = () => {
        if (source !== opened) return
        const wasUp = up
        close()
        if (!wasUp) options.onDown()
        later(wasUp ? Math.min(RESUME_MS, options.retryMs ?? RETRY_MS) : undefined)
      }
    } catch {
      options.onDown()
      later()
    }
  }
  const unwatch = visibility?.watch(() => {
    if (stopped) return
    if (shown()) {
      options.onVisible?.()
      start()
    } else {
      clear()
      close()
    }
  })
  start()
  return {
    reconnect() {
      if (stopped) return
      clear()
      close()
      if (shown()) start()
    },
    stop() {
      stopped = true
      clear()
      close()
      unwatch?.()
    },
  }
}
