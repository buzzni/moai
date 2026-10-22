/**
 * Who is working and which moai answers, for the header the page draws as the TUI does (view/brand.ts, se-qkga):
 * the TUI's `User` and `Version` lines (moai src/tui/draw.rs `told_of`, `version_said`). Read from `/api/boot`, which
 * carries `version`, and `user` and `latest` from the server that added them; an older server says neither, and the
 * header then prints `—` for the user, as the TUI does for a user it cannot tell. No DOM here.
 *
 * Upstream (Saycode's panel 2.1.0) read these from `status --json`; moai's `status` carries neither, so here they
 * come with the boot.
 */

/** What moai last heard of the latest release, as `crate::latest::Seen` keeps it; read from its cache, never asked here. */
export interface Latest {
  state: 'newer' | 'same' | 'ahead' | 'stale' | 'unasked'
  tag?: string
  /** When the cached answer was heard (RFC 3339); a `stale` one only. */
  heardAt?: string
  /** Why the latest was not checked (`offline`, `timeout`, …): `stale` and `unasked`. */
  why?: string
}

export interface About {
  /** The User line, as the TUI header prints it (the project's `naming` applied); null when moai could not tell. */
  user: string | null
  /** The moai version; null when this moai does not say. */
  version: string | null
  /** null when moai holds no answer about the latest release. */
  latest: Latest | null
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)
const STATES: readonly Latest['state'][] = ['newer', 'same', 'ahead', 'stale', 'unasked']

/** `/api/boot`'s `latest` (`{state, tag?, heard_at?, why?}`), or null when it is absent or not that shape. */
export function latestOf(value: unknown): Latest | null {
  if (!isRecord(value)) return null
  const state = STATES.find((one) => one === value.state)
  if (!state) return null
  const tag = text(value.tag)
  const heardAt = text(value.heard_at)
  const why = text(value.why)
  return { state, ...(tag ? { tag } : {}), ...(heardAt ? { heardAt } : {}), ...(why ? { why } : {}) }
}

/** `/api/boot`'s `user`: the User line already labelled, or null when absent or empty. */
export const userOf = (value: unknown): string | null => text(value) ?? null

/** The header's lines from the boot; before the boot answers, an About of nulls. */
export function aboutFrom(boot: { version?: unknown; user?: string | null; latest?: Latest | null } | null | undefined): About {
  if (!boot) return { user: null, version: null, latest: null }
  return { user: boot.user ?? null, version: text(boot.version) ?? null, latest: boot.latest ?? null }
}

/** The words of the Version line, moai's own (its `tui.version.*` keys); `{tag}` and `{d}` are filled in. */
export interface VersionCopy {
  newer: string
  same: string
  ahead: string
  unchecked: string
  /** By the reason moai gives; one it does not list reads as `unchecked`. */
  uncheckedFor: Record<string, string>
  seenToday: string
  seenDay: string
  seenDays: string
  seenEarlier: string
}

const fill = (template: string, values: Record<string, string>) => template.replace(/\{(\w+)\}/g, (whole, key: string) => values[key] ?? whole)

/** Whole days from `from` to `now`, or null when `from` cannot be read or lies ahead of `now` by more than a minute. */
function daysSince(from: string, now: number): number | null {
  const at = Date.parse(from)
  if (Number.isNaN(at) || at - now > 60_000) return null
  return Math.max(0, Math.floor((now - at) / 86_400_000))
}

/**
 * The Version line as the TUI writes it: `<version> · <what moai heard of the latest>`, and for an answer it could not
 * refresh, when it last heard one (the TUI's tail, which it puts on the same line or the next by its room). Without
 * a version, `—`.
 */
export function versionLine(about: About | null, copy: VersionCopy, now = Date.now()): { said: string; tail?: string } {
  if (!about?.version) return { said: '—' }
  const latest = about.latest
  if (!latest) return { said: about.version }
  const tag = latest.tag ?? ''
  const unchecked = (latest.why && copy.uncheckedFor[latest.why]) || copy.unchecked
  let said: string
  let tail: string | undefined
  switch (latest.state) {
    case 'newer': said = fill(copy.newer, { tag }); break
    case 'same': said = fill(copy.same, { tag }); break
    case 'ahead': said = fill(copy.ahead, { tag }); break
    case 'unasked': said = unchecked; break
    case 'stale': {
      said = unchecked
      const days = latest.heardAt ? daysSince(latest.heardAt, now) : null
      tail = days === null ? fill(copy.seenEarlier, { tag }) : days === 0 ? fill(copy.seenToday, { tag }) : days === 1 ? fill(copy.seenDay, { tag }) : fill(copy.seenDays, { tag, d: String(days) })
      break
    }
  }
  return { said: `${about.version} · ${said}`, ...(tail ? { tail } : {}) }
}
