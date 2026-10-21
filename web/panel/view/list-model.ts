// The list view's pure half (design §4.1, §4.6–4.8; TUI nav.rs, view.rs, query.rs order): what the view
// toggles keep, the order rows sort in, the flat lines of the tree with open folders unfolded, the columns
// that fit, and the windowing math of the virtual scroll. No DOM; view/list.ts draws from these and
// tests/moai-e3-list.test.mjs checks them.

import { archiveCandidate, isBacklog, isGroup, LOST, NO_MILESTONE, pathKey, type IssueIndex, type Keep, type Order, type Path, type Row, type Seg, type Twig } from "../data/index.ts"
import { applyFilter, hits, orderBy, parseFilter, type Flags, type Sort, type SortField, type SortKey } from "../data/query.ts"
import type { HiddenReason } from "../i18n/index.ts"
import type { ColumnName, State, ViewSettings } from "../store.ts"
import { visibleColumns } from "./responsive.ts"

/** Days a done row stands before it reads as archived (moai's `archive_days` default). */
export const ARCHIVE_DAYS = 14

/** One line of the list. `key` is what the store's cursor holds for it: a row's id, `..`, or a bucket's seg. */
export type ListLine =
  | { kind: "up"; key: string; depth: 0; path: Path }
  | { kind: "bucket"; key: string; depth: number; twig: Twig; seg: Seg; path: Path; open: boolean }
  | { kind: "row"; key: string; depth: number; twig: Twig; at: number; dir: boolean; path: Path | null; open: boolean }

export const UP_KEY = ".."

/** A row's column: a group's is read from its members. */
export function statusOf(row: Row): string {
  const status = isGroup(row) ? row.derived_status ?? row.status : row.status
  return typeof status === "string" ? status : ""
}

/**
 * What dates a group for the archive (data/index.ts archiveCandidate): the work under it, each by the day it
 * got to its column. A member that is put off is not counted, but a group that reads done by putting its
 * remaining members off counts from the day they were put off (moai's `report::Stand::entered`); the group's
 * own deferral does not take its members out of the count.
 */
function stamps(index: IssueIndex, at: number): Row[] {
  const group = index.rows[at]!
  const out: Row[] = []
  for (const member of index.progress(index.dirPath(at)).work) {
    const row = index.rows[member]!
    const shelf = index.shelvedAt(member)
    if (shelf === undefined || shelf === group.id) {
      out.push(row)
      continue
    }
    const holder = index.find(shelf)
    const since = holder === undefined ? undefined : index.rows[holder]!.deferred_at
    if (typeof since === "string") out.push({ id: row.id, status_since: since })
  }
  return out
}

/**
 * Why the view toggles (`SPC v`) hide a row, or undefined when they show it: a hidden status column, a
 * deferral, the backlog, or the archive (done rows past the archive age, unless archived rows are shown).
 */
export function viewHides(view: ViewSettings, index: IssueIndex, now: Date = new Date(), archiveDays = ARCHIVE_DAYS): (at: number) => HiddenReason | undefined {
  const hidden = new Set(view.hiddenStatuses)
  return (at) => {
    const row = index.rows[at]!
    if (hidden.size && hidden.has(statusOf(row))) return "column"
    if (view.hideDeferred && index.shelvedAt(at) !== undefined) return "deferred"
    if (view.hideBacklog && isBacklog(row)) return "backlog"
    if (view.showArchived) return undefined
    return archiveCandidate(row, now, archiveDays, isGroup(row) ? stamps(index, at) : undefined) ? "archived" : undefined
  }
}

/** What the view toggles keep (`SPC v`): hidden columns, deferred, backlog and archived rows go. */
export function viewKeep(view: ViewSettings, index: IssueIndex, now: Date = new Date(), archiveDays = ARCHIVE_DAYS): Keep {
  const hides = viewHides(view, index, now, archiveDays)
  return (at) => hides(at) === undefined
}

/**
 * The flags the filter text is read with. The panel's view toggles do all the hiding (done by status column,
 * deferred, backlog, the archive — {@link viewHides}), so the filter's own CLI defaults are all opened here and
 * the two never disagree about a row. Polarity: query.ts's flags say "let in" (`archived: true` lets the
 * archive in), as the store's `view.showArchived` does; the store's `hide*` toggles are the other way round, and
 * the filter never reads them.
 */
export const FILTER_FLAGS: Flags = { all: true, archived: true, backlog: true }

/** The rows the list and board draw, from the view toggles, the applied filter and the search. */
export interface Narrow {
  /** Drawn: what the filter passes and, with a search on, what it finds whether the view hides it or not
   *  (design §4.4: a search lifts the view's hiding); with no search, what the view keeps. */
  keep: Keep
  /** What the filter passes and the search (when on) finds, whatever the view hides (the board's cards). */
  match: Keep
  /** A drawn row that the view hides and only the search shows: drawn dim, marked hidden. */
  revealed: Keep
  /**
   * The rows the "N hidden" notes count: the view hides them, and the user's own filter and search would show
   * them. A row the filter drops is not "hidden", it is filtered; a row the CLI would hide by default but the
   * view shows (a done row in an unhidden column) is not hidden either.
   */
  hidden: Keep
  /** Why the view hides a row the search found, by the row itself (the toolbar's note); undefined otherwise. */
  hiddenBy: (row: Row) => HiddenReason | undefined
  searching: boolean
}

type NarrowState = Pick<State, "view"> & { search: Pick<State["search"], "text" | "scope">; filter: Pick<State["filter"], "applied"> }

/**
 * Epoch seconds to the reader's wall clock (query.ts Context.local): a `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` in the
 * filter is the reader's own day, as `moai show --filter` reads it, not a UTC one.
 */
const readerClock = (epoch: number): number => epoch - new Date(epoch * 1000).getTimezoneOffset() * 60

let narrowed: { index: IssueIndex; key: string; value: Narrow } | null = null

/** {@link Narrow} for a state; the list, the board, the toolbar and main.ts share one answer per change. */
export function narrow(state: NarrowState, index: IssueIndex, now: Date = new Date()): Narrow {
  const key = JSON.stringify([state.view, state.search.text, state.search.scope, state.filter.applied])
  if (narrowed && narrowed.index === index && narrowed.key === key) return narrowed.value
  const hides = viewHides(state.view, index, now)
  const parsed = parseFilter(state.filter.applied, FILTER_FLAGS)
  // The store applies only a text that parses; a bad one here (set from outside) narrows nothing.
  const passed = state.filter.applied.trim() !== "" && parsed.ok ? new Set(applyFilter(index.rows, parsed.value, { now: now.toISOString().replace(/\.\d+Z$/, "Z"), local: readerClock })) : null
  const q = state.search.text.trim()
  const searching = q !== ""
  const wanted = (at: number) => {
    const row = index.rows[at]!
    return (passed === null || passed.has(row)) && (!searching || hits(row, q, state.search.scope))
  }
  const value: Narrow = {
    match: wanted,
    keep: (at) => wanted(at) && (searching || hides(at) === undefined),
    revealed: (at) => searching && wanted(at) && hides(at) !== undefined,
    hidden: (at) => !searching && wanted(at) && hides(at) !== undefined,
    hiddenBy: (() => {
      let place: Map<Row, number> | null = null
      return (row: Row) => {
        place ??= new Map(index.rows.map((one, at) => [one, at]))
        const at = place.get(row)
        return at === undefined || !wanted(at) ? undefined : hides(at)
      }
    })(),
    searching,
  }
  narrowed = { index, key, value }
  return value
}

/** How many rows under `path`, at any depth, `hidden` holds ({@link Narrow.hidden}). */
export function hiddenUnder(index: IssueIndex, path: Path, hidden: Keep): number {
  let count = 0
  for (const at of index.descendants(path)) if (hidden(at)) count++
  return count
}

/** `SPC v a`: lift the column, deferred and backlog hiding; archived rows stay as they are. */
export const SHOW_ALL: Partial<ViewSettings> = { hiddenStatuses: [], hideDeferred: false, hideBacklog: false }

/** True when the toggles that "show all" lifts hide something. */
export const hidesAnything = (view: ViewSettings): boolean => view.hiddenStatuses.length > 0 || view.hideDeferred || view.hideBacklog

// ── order ──────────────────────────────────────────────────────────────

/**
 * The rows' order for a sort. It is moai's `order_by` as data/query.ts ports it — the one that
 * tests/moai-query.test.mjs holds to the order `moai show --sort` prints: field by field (a title compares
 * without regard to case, an unassigned row goes last when ascending), then `(priority, id)`, or only the id
 * when the last field is a date; `reversed` turns the whole list. The store keeps query.ts's {@link Sort}.
 */
export function sortCompare(sort: Sort | ReadonlyArray<SortField>): (a: Row, b: Row) => number {
  const { fields, reversed } = "fields" in sort ? sort : { fields: sort, reversed: false }
  return (a, b) => orderBy(a, b, fields, reversed)
}

/** {@link sortCompare} over the index's row positions, as {@link IssueIndex.entries} takes it. */
export function sortOrder(sort: Sort | ReadonlyArray<SortField>, index: IssueIndex): Order {
  const compare = sortCompare(sort)
  return (a, b) => compare(index.rows[a]!, index.rows[b]!)
}

// ── lines ──────────────────────────────────────────────────────────────

/**
 * The listing at `state.path` with the open folders unfolded in place: `..` first below the root, then each
 * level's folders, rows and buckets in {@link IssueIndex.entries} order.
 */
export function listLines(state: Pick<State, "path" | "expanded">, index: IssueIndex, keep: Keep, order: Order): ListLine[] {
  const open = new Set(state.expanded)
  const out: ListLine[] = []
  if (state.path.length) out.push({ kind: "up", key: UP_KEY, depth: 0, path: state.path.slice(0, -1) })
  const kin: boolean[] = []
  const walk = (here: Path) => {
    const entries = index.entries(here, keep, order)
    entries.forEach((entry, n) => {
      kin.push(n < entries.length - 1)
      const twig = [...kin]
      if (entry.dir) {
        const path = [...here, entry.seg]
        const opened = open.has(pathKey(path))
        if (entry.at === undefined) out.push({ kind: "bucket", key: entry.seg, depth: twig.length, twig, seg: entry.seg, path, open: opened })
        else out.push({ kind: "row", key: index.rows[entry.at]!.id, depth: twig.length, twig, at: entry.at, dir: true, path, open: opened })
        if (opened) walk(path)
      } else {
        out.push({ kind: "row", key: index.rows[entry.at]!.id, depth: twig.length, twig, at: entry.at, dir: false, path: null, open: false })
      }
      kin.pop()
    })
  }
  walk(state.path)
  return out
}

/** A line that opens as a folder (a bucket, or a row with something under it): where, and whether it is unfolded. */
export function folderOf(line: ListLine): { path: Path; open: boolean } | null {
  if (line.kind === "bucket" || (line.kind === "row" && line.dir && line.path)) return { path: line.path!, open: line.open }
  return null
}

/** The branch drawn before a line inside an unfolded folder (`│ ├─ └─`); nothing at the listing's own level. */
export function twigText(twig: Twig): string {
  if (twig.length < 2) return ""
  return twig.slice(1, -1).map((more) => (more ? "│ " : "  ")).join("") + (twig[twig.length - 1] ? "├─" : "└─")
}

/** The cursor key that names a place's last segment — where the cursor lands after leaving it. */
export function segKey(seg: Seg): string {
  if (seg === NO_MILESTONE || seg === LOST) return seg
  return seg.slice(2)
}

/** The line `delta` lines away from `key`, held at the ends; from no line, the first (or last) one. */
export function moveKey(lines: ReadonlyArray<ListLine>, key: string | null, delta: number): string | null {
  if (!lines.length) return null
  const at = key === null ? -1 : lines.findIndex((line) => line.key === key)
  if (at < 0) return lines[delta < 0 ? lines.length - 1 : 0]!.key
  return lines[Math.max(0, Math.min(lines.length - 1, at + delta))]!.key
}

/** Where the cursor goes on entering a place: its first line after `..`. */
export function firstKey(lines: ReadonlyArray<ListLine>): string | null {
  return lines.find((line) => line.kind !== "up")?.key ?? null
}

/** The folder line a line hangs under in the tree, or -1 at the listing's own level. */
export function parentLine(lines: ReadonlyArray<ListLine>, at: number): number {
  const depth = lines[at]?.depth ?? 0
  if (depth < 2) return -1
  for (let n = at - 1; n >= 0; n--) if (lines[n]!.depth === depth - 1) return n
  return -1
}

// ── columns ────────────────────────────────────────────────────────────

/** Right-hand columns, left to right. They drop out as the list narrows (view/responsive.ts). */
export type RightColumn = "tags" | "epic" | "assignee" | "created" | "updated"
const RIGHT: RightColumn[] = ["tags", "epic", "assignee", "created", "updated"]

export interface ColumnSet {
  id: boolean
  priority: boolean
  counts: boolean
  /** The sticky name line on top. */
  names: boolean
  right: RightColumn[]
}

/** The columns the user turned on that fit a list `width` px wide. */
export function columnSet(columns: Record<ColumnName, boolean>, width: number): ColumnSet {
  return {
    id: columns.id,
    priority: columns.priority,
    counts: columns.counts,
    names: columns.names,
    right: visibleColumns(width, RIGHT.filter((column) => columns[column])),
  }
}

/** The sort field a column head sorts by; heads without one do not sort. A click is `SPC s <key>` (query.ts sortEdit replace). */
export const HEAD_SORT: Partial<Record<"id" | "priority" | "status" | "title" | RightColumn, SortKey>> = {
  priority: "priority",
  status: "status",
  title: "title",
  assignee: "assignee",
  created: "created",
  updated: "updated",
}

const time = (value: unknown): number | undefined => {
  const at = typeof value === "string" ? Date.parse(value) : NaN
  return Number.isFinite(at) ? at : undefined
}

/** A date in 6ch: `MM-DD` this year, the year otherwise; empty when unreadable. */
export function shortDate(value: unknown, now: Date = new Date()): string {
  const at = time(value)
  if (at === undefined) return ""
  const date = new Date(at)
  if (date.getFullYear() !== now.getFullYear()) return String(date.getFullYear())
  return `${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`
}

/** The column glyph (look-and-feel §6): the glyph differs, so colour never carries the status alone. */
export function statusGlyph(status: string): { glyph: string; tone: string } {
  switch (status) {
    case "todo":
      return { glyph: "·", tone: "todo" }
    case "in_progress":
      return { glyph: "▸", tone: "in_progress" }
    case "review":
      return { glyph: "?", tone: "review" }
    case "done":
      return { glyph: "✓", tone: "done" }
    default:
      return { glyph: "○", tone: "other" }
  }
}

// ── windowing ──────────────────────────────────────────────────────────

/** Rows drawn above and below the visible ones, so a quick scroll does not show blank space. */
export const OVERSCAN = 8

export interface WindowInput {
  /** The scroller's offset and visible height. */
  scrollTop: number
  viewport: number
  /** Where the first row starts in the scroller's content. */
  offset: number
  rowHeight: number
  count: number
  overscan?: number
}

/** The rows `[start, end)` to draw for this scroll position. */
export function windowRange(input: WindowInput): { start: number; end: number } {
  const overscan = input.overscan ?? OVERSCAN
  const top = Math.max(0, input.scrollTop - input.offset)
  const first = Math.floor(top / input.rowHeight)
  const last = Math.ceil((top + Math.max(0, input.viewport)) / input.rowHeight)
  const start = Math.max(0, Math.min(input.count, first - overscan))
  const end = Math.max(start, Math.min(input.count, last + overscan))
  return { start, end }
}

/**
 * The scroll offset that shows a row whole: unchanged when it is in view already. `inset` is what covers the
 * top of the viewport (the sticky name line).
 */
export function revealOffset(scrollTop: number, viewport: number, top: number, height: number, inset = 0): number {
  if (top < scrollTop + inset) return Math.max(0, top - inset)
  if (top + height > scrollTop + viewport) return Math.max(0, top + height - viewport)
  return scrollTop
}
