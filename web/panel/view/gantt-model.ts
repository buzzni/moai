// The gantt tab's pure half (view/gantt.ts draws from it; tests/moai-gantt.test.mjs checks it). No DOM here.
//
// **The page does not judge dates.** `report::gantt` decided every value of a row's `gantt` (data/index.ts GanttRow):
// where the plan came from, whether the start is known, how late it is, what blocks it. This file only puts those
// values on a date axis: planned days are calendar days (no zone moves them, and a planned end is inclusive — the bar
// runs to the end of that day), actual times are UTC instants that are read in the zone the page draws in.
//
// Every time here is a "wall" millisecond: the clock reading of the drawing zone written as if it were UTC
// (Date.UTC of the zone's year, month, day, hour, minute). A day is then exactly 864e5 and a weekday is
// `getUTCDay()`, whatever the zone's daylight saving does.

import { partsIn } from '../data/zone.ts'
import { pathKey, type GanttRow, type IssueIndex, type Keep, type Path, type Row, type Seg, type Twig } from '../data/index.ts'
import { fill, type Copy } from '../i18n/index.ts'
import { statusOf } from './list-model.ts'

export type GanttCopy = Copy['gantt']

/** The steps from the coarsest to the finest. `=` goes finer, `-` coarser. */
export type Zoom = 'month' | 'week' | 'day' | 'hour'
export const ZOOMS: readonly Zoom[] = ['month', 'week', 'day', 'hour']
/** Pixels a day takes: an hour is 20px, a day 40px, a week 14px a day, a month 4px a day. */
export const PX_PER_DAY: Readonly<Record<Zoom, number>> = { hour: 20 * 24, day: 40, week: 14, month: 4 }
export const DEFAULT_ZOOM: Zoom = 'day'
export const DAY = 864e5
export const HOUR = 36e5
/** Every line of the grid and of the dates is this tall. */
export const ROW_H = 28
/** The two-line date header. */
export const HEAD_H = 44

export const DEFAULT_GRID = 460
export const MIN_GRID = 220
export const MAX_GRID = 760

/** A stored or dragged grid width to a usable one. */
export function normalizeGrid(value: unknown, max: number = MAX_GRID): number {
  const n = typeof value === 'string' ? Number(value) : value
  if (typeof n !== 'number' || !Number.isFinite(n)) return DEFAULT_GRID
  return Math.round(Math.max(MIN_GRID, Math.min(Math.max(MIN_GRID, Math.min(MAX_GRID, max)), n)))
}

// ── time ───────────────────────────────────────────────────────────────

/** An instant (RFC3339, UTC) as a wall time in `zone`; null when it is not a time. */
export function wallOf(value: string | number, zone?: string): number | null {
  const at = typeof value === 'number' ? value : Date.parse(value)
  if (!Number.isFinite(at)) return null
  const p = partsIn(at, zone)
  return Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second)
}

/** `YYYY-MM-DD` as the wall time of that day's start; null for anything else. */
export function dayStart(day: unknown): number | null {
  if (typeof day !== 'string') return null
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(day)
  return m ? Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])) : null
}

export const floorDay = (wall: number): number => Math.floor(wall / DAY) * DAY

export const dayText = (wall: number): string => {
  const d = new Date(wall)
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, '0')}-${String(d.getUTCDate()).padStart(2, '0')}`
}

/** ISO 8601 week number of the week holding `wall`. */
export function isoWeek(wall: number): number {
  const d = new Date(wall)
  const thursday = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + 3 - ((d.getUTCDay() + 6) % 7))
  const jan4 = Date.UTC(new Date(thursday).getUTCFullYear(), 0, 4)
  return 1 + Math.round(((thursday - jan4) / DAY - 3 + ((new Date(jan4).getUTCDay() + 6) % 7)) / 7)
}

// ── one row on the axis ────────────────────────────────────────────────

export interface Placed {
  /** The planned bar, ends in wall ms (the end is the end of the last day). */
  plan: { from: number; to: number; origin: 'own' | 'members' } | null
  /** The `◆`: the end of a milestone's own deadline day. */
  due: number | null
  /** The actual band; `open` runs to now. No band when the start is unknown or nothing started. */
  actual: { from: number; to: number; open: boolean } | null
  /** The start nobody recorded. */
  unknown: boolean
  /** The red hatch from the end of the deadline day to now. */
  over: { from: number; to: number } | null
  /** Everything drawn, for the fit and the `‹ ›` marks. */
  span: [number, number] | null
  /** The column the actual band is coloured by (`todo`, `in_progress`, `review`, `done`, or what the config names). */
  status: string
}

export function placeRow(g: GanttRow | undefined, row: Row | undefined, now: number, zone?: string): Placed {
  const out: Placed = { plan: null, due: null, actual: null, unknown: false, over: null, span: null, status: row ? statusOf(row) : '' }
  if (!g) return out
  const lo: number[] = []
  const hi: number[] = []
  const add = (a: number, b: number) => {
    lo.push(a)
    hi.push(Math.max(a, b))
  }
  const planned = g.planned
  const start = planned ? dayStart(planned.start) : null
  const end = planned ? dayStart(planned.end) : null
  if (planned && start !== null && end !== null) {
    // A start written after its deadline (an old row moai reads but would not write) draws the start's day.
    const to = Math.max(end, start) + DAY
    out.plan = { from: start, to, origin: planned.from }
    add(start, to)
  }
  // Only a milestone's own deadline has a diamond; filled ends are never deadlines.
  const own = row && typeof (row.due_on ?? row.due) === 'string' ? dayStart(row.due_on ?? row.due) : null
  const dueDay = g.kind === 'milestone' ? own ?? (planned?.from === 'own' && !planned.end_filled ? end : null) : null
  if (dueDay !== null) {
    out.due = dueDay + DAY
    add(dueDay, dueDay + DAY)
  }
  if (g.actual.kind === 'unknown') out.unknown = true
  else if (g.actual.kind === 'span') {
    const from = wallOf(g.actual.start, zone)
    const to = g.actual.end === null ? now : wallOf(g.actual.end, zone)
    if (from !== null) {
      const stop = Math.max(from, to ?? now)
      out.actual = { from, to: stop, open: g.actual.end === null }
      add(from, stop)
    }
  }
  // Core judges lateness; the hatch starts after the inclusive deadline day.
  const lateFrom = planned?.from === 'own' && end !== null ? end + DAY : out.due
  if (g.overdue !== null && lateFrom !== null) {
    out.over = { from: lateFrom, to: Math.max(now, lateFrom) }
    add(lateFrom, out.over.to)
  }
  if (lo.length) out.span = [Math.min(...lo), Math.max(...hi)]
  return out
}

// ── moving a plan ──────────────────────────────────────────────────────

/** Which part of a planned bar is dragged: the start alone, the whole bar, the deadline alone. */
export type Edge = 'start' | 'body' | 'end'

/** The days a move would land on — drawn while a bar is dragged; moai measures the written ones itself (data/plan.ts). */
export type Shifted = { start?: string; due?: string } | { why: 'members' | 'none' | 'still' }

/**
 * A plan moved `days` days: the days it lands on, **the changing side only** — the page's copy of `Issue::shifted`
 * (src/model.rs), except a filled start: dragging it chooses an absolute start from the drawn day (option D).
 * Ordinary moves send only edge+days for moai to measure inside its write; that explicit start sends the chosen day.
 * Only a row's own days move: a plan a group counts from its members is not that row's days (`members`), and a row with
 * no day has no bar (`none`).
 *
 * - `body` moves every day written — a row with a deadline alone moves its deadline
 * - `start` moves a written start relatively; a filled start chooses drawn start + days as an absolute date
 * - `end` moves the deadline; with none, it sets out from the start
 *
 * A start past the deadline is not stopped here: the write refuses it in moai's words, and the page shows them.
 */
export function shiftPlan(planned: GanttRow['planned'], edge: Edge, days: number): Shifted {
  if (days === 0) return { why: 'still' }
  if (planned?.from === 'members') return { why: 'members' }
  const start = planned && !planned.start_filled ? dayStart(planned.start) : null
  const due = planned && !planned.end_filled ? dayStart(planned.end) : null
  if (start === null && due === null) return { why: 'none' }
  const day = (d: number | null) => (d === null ? undefined : dayText(d + days * DAY))
  const out = edge === 'body' ? { start: day(start), due: day(due) } : edge === 'start' ? { start: day(planned?.start_filled ? dayStart(planned.start) : start ?? due) } : { due: day(due ?? start) }
  return Object.fromEntries(Object.entries(out).filter(([, v]) => v !== undefined))
}

/** Whole days a drag of `dx` px moves at `zoom` — the nearest day, so a bar snaps to days at every step. */
export const dragDays = (zoom: Zoom, dx: number): number => Math.round(dx / PX_PER_DAY[zoom]) || 0

/**
 * The planned bar a move would draw, `[from, to)` in wall ms, or null when the move would not stand — the bar moai draws
 * once the written days have moved, as the TUI's drag draws it (`tui::gantt::preview`, moai-i4hf.exq). Only written days
 * move and only they can make a move stand or not. A filled end is the start's day again. A filled start is moai's
 * creation day capped at the deadline. `made` is that day placed in the page zone. Without it (tree-only/archive rows),
 * a start before the old deadline is the known creation day; a start on the deadline supplies no earlier bound.
 */
export function movedPlan(planned: GanttRow['planned'], change: { start?: string; due?: string }, made: number | null = null): [number, number] | null {
  const written = (day: string | undefined, filled: boolean | undefined, now: string | undefined) =>
    day !== undefined ? dayStart(day) : planned && !filled ? dayStart(now) : null
  const start = written(change.start, planned?.start_filled, planned?.start)
  const due = written(change.due, planned?.end_filled, planned?.end)
  if (start !== null && due !== null) return start > due ? null : [start, due + DAY]
  if (start !== null) return [start, start + DAY]
  if (due === null) return null
  const filled = planned ? dayStart(planned.start) : null
  const oldDue = planned ? dayStart(planned.end) : null
  const origin = made ?? (filled !== null && oldDue !== null && filled < oldDue ? filled : due)
  const from = Math.min(origin, due)
  return [from, due + DAY]
}

// ── the axis ───────────────────────────────────────────────────────────

export interface Axis {
  zoom: Zoom
  /** The wall time at x = 0 (a day's start) and the end (a day's start). */
  from: number
  to: number
}

export const pxPerDay = (zoom: Zoom): number => PX_PER_DAY[zoom]
export const xOf = (axis: Axis, wall: number): number => ((wall - axis.from) / DAY) * PX_PER_DAY[axis.zoom]
export const wallAt = (axis: Axis, x: number): number => axis.from + (x / PX_PER_DAY[axis.zoom]) * DAY
export const widthOf = (axis: Axis): number => Math.round(xOf(axis, axis.to))

/**
 * The dates the axis spans: everything the lines draw and now, a week either side, whole days. Without any date the
 * axis still holds a few days around now.
 */
export function rangeOf(spans: ReadonlyArray<[number, number] | null>, now: number): [number, number] {
  let lo = now
  let hi = now
  for (const span of spans) {
    if (!span) continue
    lo = Math.min(lo, span[0])
    hi = Math.max(hi, span[1])
  }
  return [floorDay(lo) - 7 * DAY, floorDay(hi) + 8 * DAY]
}

/** The range the lines' own dates cover, for the fit: null when no line has a date. */
export function fitSpan(spans: ReadonlyArray<[number, number] | null>): [number, number] | null {
  let lo = Infinity
  let hi = -Infinity
  for (const span of spans) {
    if (!span) continue
    lo = Math.min(lo, span[0])
    hi = Math.max(hi, span[1])
  }
  return lo <= hi ? [floorDay(lo), floorDay(hi) + DAY] : null
}

/** The finest step that shows `span` whole in `width` px; the coarsest when none does. */
export function zoomToFit(span: [number, number], width: number): Zoom {
  const days = (span[1] - span[0]) / DAY
  return [...ZOOMS].reverse().find((zoom) => days * PX_PER_DAY[zoom] <= width) ?? 'month'
}

export interface Tick {
  from: number
  to: number
  text: string
  cls: string
}

export interface Header {
  top: Tick[]
  bottom: Tick[]
}

/**
 * The two header lines at a step (month / day and weekday, day / 3 hours, month / ISO week, year / month). `win` (wall
 * ms, `[from, to)`) draws only the ticks that touch it — a tick that starts before it but reaches in is kept whole, so
 * a label can be held in view — as a multi-year axis at the hour step is ten thousand ticks (moai-vj8w.2fh).
 */
export function headerOf(axis: Axis, now: number, words: GanttCopy, win?: [number, number]): Header {
  const { zoom, to: axisTo } = axis
  const from = axis.from
  const lo = win ? Math.max(from, win[0]) : from
  const to = win ? Math.min(axisTo, win[1]) : axisTo
  /** The first day start at or before `lo`, on the axis' own day grid. */
  const dayFrom = from + Math.floor((lo - from) / DAY) * DAY
  const top: Tick[] = []
  const bottom: Tick[] = []
  const today = floorDay(now)
  const weekday = (wall: number) => words.weekdays[new Date(wall).getUTCDay()] ?? ''
  const weekend = (wall: number) => new Date(wall).getUTCDay() % 6 === 0
  const each = (step: (start: number) => number, unit: (start: number) => number, push: (start: number, end: number) => void) => {
    for (let at = unit(lo); at < to; at = step(at)) push(Math.max(at, from), Math.min(step(at), axisTo))
  }
  const monthStart = (wall: number) => {
    const d = new Date(wall)
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1)
  }
  const nextMonth = (wall: number) => {
    const d = new Date(wall)
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 1)
  }
  const months = (list: Tick[], text: (wall: number) => string) =>
    each(nextMonth, monthStart, (a, b) => list.push({ from: a, to: b, text: text(monthStart(a)), cls: '' }))
  const ym = (wall: number) => {
    const d = new Date(wall)
    return fill(words.axis.yearMonth, { year: d.getUTCFullYear(), month: d.getUTCMonth() + 1 })
  }
  if (zoom === 'hour') {
    for (let day = dayFrom; day < to; day += DAY) {
      const d = new Date(day)
      top.push({ from: day, to: day + DAY, text: fill(words.axis.hourDay, { month: d.getUTCMonth() + 1, day: d.getUTCDate(), weekday: weekday(day) }), cls: day === today ? 'today' : '' })
      for (let h = 0; h < 24; h += 3) bottom.push({ from: day + h * HOUR, to: day + (h + 3) * HOUR, text: String(h).padStart(2, '0'), cls: '' })
    }
  } else if (zoom === 'day') {
    months(top, ym)
    for (let day = dayFrom; day < to; day += DAY) {
      const d = new Date(day)
      bottom.push({ from: day, to: day + DAY, text: fill(words.axis.day, { day: d.getUTCDate(), weekday: weekday(day) }), cls: day === today ? 'today' : weekend(day) ? 'we' : '' })
    }
  } else if (zoom === 'week') {
    months(top, ym)
    const monday = (wall: number) => floorDay(wall) - ((new Date(wall).getUTCDay() + 6) % 7) * DAY
    each((w) => w + 7 * DAY, monday, (a, b) => bottom.push({ from: a, to: b, text: fill(words.axis.week, { week: isoWeek(a) }), cls: today >= a && today < b ? 'today' : '' }))
  } else {
    const yearStart = (wall: number) => Date.UTC(new Date(wall).getUTCFullYear(), 0, 1)
    each((y) => Date.UTC(new Date(y).getUTCFullYear() + 1, 0, 1), yearStart, (a, b) => top.push({ from: a, to: b, text: fill(words.axis.year, { year: new Date(yearStart(a)).getUTCFullYear() }), cls: '' }))
    months(bottom, (m) => fill(words.axis.month, { month: new Date(m).getUTCMonth() + 1 }))
    for (const tick of bottom) if (today >= tick.from && today < tick.to) tick.cls = 'today'
  }
  return { top, bottom }
}

/** The Saturdays and Sundays inside the axis (and inside `win`, wall ms, when given), as [from, to) in wall ms. */
export function weekends(axis: Axis, win?: [number, number]): Array<[number, number]> {
  const out: Array<[number, number]> = []
  const lo = win ? Math.max(axis.from, win[0]) : axis.from
  const hi = win ? Math.min(axis.to, win[1]) : axis.to
  for (let day = axis.from + Math.floor((lo - axis.from) / DAY) * DAY; day < hi; day += DAY) if (new Date(day).getUTCDay() % 6 === 0) out.push([day, day + DAY])
  return out
}

// ── lines ──────────────────────────────────────────────────────────────

/** One line of the gantt: a row, or a bucket (`(no milestone)`, `(lost)`) that holds rows and has no dates. */
export type GanttLine =
  | { kind: 'row'; key: string; at: number; depth: number; twig: Twig; path: Path; open: boolean; dir: boolean }
  | { kind: 'bucket'; key: string; seg: Seg; depth: number; twig: Twig; path: Path; open: boolean; dir: true }

/** The listing with the gantt’s own folds, in moai’s order; all folders start open. */
export function ganttLines(index: IssueIndex, path: Path, keep: Keep, shut: ReadonlySet<string> = new Set()): GanttLine[] {
  const opened = (p: Path) => !shut.has(pathKey(p))
  return index.tree(path, opened, keep).map(({ entry, twig, path: here }): GanttLine => {
    if (entry.dir && entry.at === undefined) return { kind: 'bucket', key: entry.seg, seg: entry.seg, depth: twig.length, twig, path: here, open: opened(here), dir: true }
    const at = entry.at!
    return { kind: 'row', key: index.rows[at]!.id, at, depth: twig.length, twig, path: here, open: entry.dir && opened(here), dir: entry.dir }
  })
}

// ── blockers ───────────────────────────────────────────────────────────

/**
 * The path of the arrow from a blocker to the line it blocks, in the lanes' own coordinates: out of the blocker's
 * end, one bend before the target when there is room, else down (or up) into the gap beside the target row, back, and
 * in. `rowFrom` and `rowTo` are line numbers; the arrowhead is `head`.
 */
export function arrowOf(fromX: number, rowFrom: number, toX: number, rowTo: number): { d: string; head: string } {
  const y0 = rowFrom * ROW_H + ROW_H / 2
  const y1 = rowTo * ROW_H + ROW_H / 2
  const up = y1 < y0 ? -1 : 1
  const d = toX - fromX >= 16
    ? `M${fromX} ${y0} H${toX - 10} V${y1} H${toX - 6}`
    : `M${fromX} ${y0} H${fromX + 6} V${y1 - up * 14} H${toX - 12} V${y1} H${toX - 6}`
  return { d, head: `${toX - 6},${y1 - 4} ${toX},${y1} ${toX - 6},${y1 + 4}` }
}
