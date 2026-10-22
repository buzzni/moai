/**
 * The time zone every date on the page is drawn in (`SPC o t`, as the TUI's: src/tui/zones.rs). It only draws: nothing
 * is written to the server — the TUI keeps its own choice in the user config (`[tui] timezone`), and the page keeps
 * its own in the browser (data/memory.ts `zone`).
 *
 * Which zone draws: the one picked on this page, else the TUI's (`/api/boot`'s `timezone`), else the browser's own.
 * A name the browser does not know falls through to the next one, without a word — a remembered zone the browser has
 * since dropped, or a TUI zone written as a POSIX rule (`JST-9`) Intl cannot read.
 *
 * A bare day (`YYYY-MM-DD`, a due day) is a calendar day, not an instant: no zone moves it. No DOM here.
 */

/** The browser's own zone, as Intl names it; `UTC` when it names none. */
export function browserZone(): string {
  try {
    return new Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

/** The name as Intl spells it, or null when this browser does not know it. */
export function validZone(name: unknown): string | null {
  if (typeof name !== 'string' || name.trim() === '') return null
  try {
    return new Intl.DateTimeFormat('en-US', { timeZone: name }).resolvedOptions().timeZone
  } catch {
    return null
  }
}

/** The zone that draws: the picked one, else the TUI's, else the browser's (see the header). */
export function effectiveZone(chosen: string | null | undefined, tui: string | null | undefined): string {
  return validZone(chosen) ?? validZone(tui) ?? browserZone()
}

/** What the zone falls back to when the page's own choice is cleared: the TUI's, else the browser's. */
export function fallbackZone(tui: string | null | undefined): { zone: string; from: 'tui' | 'browser' } {
  const own = validZone(tui)
  return own ? { zone: own, from: 'tui' } : { zone: browserZone(), from: 'browser' }
}

/** Every name the picker lists after its first line: `UTC`, then what this browser knows, each once. */
export function zoneNames(): string[] {
  let known: string[] = []
  try {
    const supported = (Intl as unknown as { supportedValuesOf?: (key: string) => string[] }).supportedValuesOf
    known = supported ? supported('timeZone') : []
  } catch {
    known = []
  }
  return ['UTC', ...known.filter((name) => name !== 'UTC')]
}

/** The names that hold `typed`, case aside — the TUI's filter (zones.rs `shown`). */
export function filterZones(names: readonly string[], typed: string): string[] {
  const want = typed.trim().toLowerCase()
  return want === '' ? [...names] : names.filter((name) => name.toLowerCase().includes(want))
}

// ── the zone the page draws in now ──────────────────────────────────────────────────────────────────────

let drawing: string | undefined

/**
 * Sets the zone every date is drawn in from here on (main.ts, as the store's choice or the boot changes). Undefined
 * draws in the browser's own.
 */
export function useZone(zone: string | null | undefined): void {
  drawing = validZone(zone) ?? undefined
}

/** The zone dates are drawn in now; undefined is the browser's own. */
export const zoneNow = (): string | undefined => drawing

export interface Parts {
  year: number
  month: number
  day: number
  hour: number
  minute: number
  second: number
}

const formats = new Map<string, Intl.DateTimeFormat>()

function formatIn(zone: string | undefined): Intl.DateTimeFormat {
  const key = zone ?? ''
  let format = formats.get(key)
  if (!format) {
    const options: Intl.DateTimeFormatOptions = {
      year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23',
    }
    try {
      format = new Intl.DateTimeFormat('en-US', zone ? { ...options, timeZone: zone } : options)
    } catch {
      format = new Intl.DateTimeFormat('en-US', options)
    }
    formats.set(key, format)
  }
  return format
}

/** The wall clock of `at` (ms since epoch) in `zone` (the zone drawn in now when left out). */
export function partsIn(at: number, zone: string | undefined = zoneNow()): Parts {
  const parts: Partial<Record<Intl.DateTimeFormatPartTypes, number>> = {}
  for (const part of formatIn(zone).formatToParts(at)) if (part.type !== 'literal') parts[part.type] = Number(part.value)
  return {
    year: parts.year ?? 0,
    month: parts.month ?? 0,
    day: parts.day ?? 0,
    // Some engines still say 24 for midnight under h23.
    hour: (parts.hour ?? 0) % 24,
    minute: parts.minute ?? 0,
    second: parts.second ?? 0,
  }
}
