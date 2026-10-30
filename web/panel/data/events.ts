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
  const later = () => {
    clear()
    if (stopped || !shown() || !factory) return
    retry = clock.setTimeout(() => {
      retry = null
      start()
    }, options.retryMs ?? RETRY_MS)
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
      opened.onerror = () => {
        if (source !== opened) return
        const wasUp = up
        close()
        if (!wasUp) options.onDown()
        later()
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
