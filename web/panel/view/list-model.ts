// The list view's pure half (design §4.1, §4.6–4.8): what the view toggles keep, the flat lines of the tree with
// open folders unfolded, the columns that fit, and the windowing math of the virtual scroll. No DOM; view/list.ts
// draws from these and tests/moai-e3-list.test.mjs checks them.
//
// Where a row stands and in which order is moai's (data/index.ts: `tui --json --deep`); what the filter and the
// search find is moai's too (store.ts Matched). What stays here is drawing: which of those rows the view toggles show.

import { partsIn, zoneNow } from "../data/zone.ts"
import { isBacklog, LOST, NO_MILESTONE, pathKey, type IssueIndex, type Keep, type Path, type Row, type Seg, type Twig } from "../data/index.ts"
import type { Boot } from "../data/source.ts"
import type { SortKey } from "../data/query.ts"
import type { HiddenReason } from "../i18n/index.ts"
import { matchKey, type ColumnName, type State, type ViewSettings } from "../store.ts"
import { visibleColumns } from "./responsive.ts"

/** One line of the list. `key` is what the store's cursor holds for it: a row's id, `..`, or a bucket's seg. */
export type ListLine =
  | { kind: "up"; key: string; depth: 0; path: Path }
  | { kind: "bucket"; key: string; depth: number; twig: Twig; seg: Seg; path: Path; open: boolean }
  | { kind: "row"; key: string; depth: number; twig: Twig; at: number; dir: boolean; path: Path | null; open: boolean }

export const UP_KEY = ".."

/** A row's column: moai's `derived_status` (a group's is read from its members), else its own. */
export function statusOf(row: Row): string {
  const status = row.derived_status ?? row.status
  return typeof status === "string" ? status : ""
}

/**
 * Why the view toggles (`SPC v`) hide a row, or undefined when they show it: a hidden status column, a deferral
 * (moai's `shelved_by`), the backlog, or the archive (a row no list printed, or one `show --archived` marked, unless
 * archived rows are shown). There is no clock here: what is the archive is moai's `archive_days`.
 */
export function viewHides(view: ViewSettings, index: IssueIndex): (at: number) => HiddenReason | undefined {
  const hidden = new Set(view.hiddenStatuses)
  return (at) => {
    const row = index.rows[at]!
    if (hidden.size && hidden.has(statusOf(row))) return "column"
    if (view.hideDeferred && index.shelvedAt(at) !== undefined) return "deferred"
    if (view.hideBacklog && isBacklog(row)) return "backlog"
    if (view.showArchived) return undefined
    return row.archived === true ? "archived" : undefined
  }
}

/** What the view toggles keep (`SPC v`): hidden columns, deferred, backlog and archived rows go. */
export function viewKeep(view: ViewSettings, index: IssueIndex): Keep {
  const hides = viewHides(view, index)
  return (at) => hides(at) === undefined
}

/** The rows the list and board draw, from the view toggles and what moai found for the filter and the search. */
export interface Narrow {
  /** Drawn: what the filter passes and, with a search on, what it finds whether the view hides it or not
   *  (design §4.4: a search lifts the view's hiding); with no search, what the view keeps. */
  keep: Keep
  /** What the filter passes and the search (when on) finds, whatever the view hides (the board's cards). */
  match: Keep
  /** A drawn row that the view hides and only the search shows: drawn dim, marked hidden. */
  revealed: Keep
  /**
   * The rows the "N hidden" notes count: the view hides them, and the person's own filter and search would show
   * them. A row the filter drops is not "hidden", it is filtered.
   */
  hidden: Keep
  /** Why the view hides a row the search found, by the row itself (the toolbar's note); undefined otherwise. */
  hiddenBy: (row: Row) => HiddenReason | undefined
  searching: boolean
}

type NarrowState = Pick<State, "view" | "matched"> & { search: Pick<State["search"], "text">; filter: Pick<State["filter"], "applied"> }

let narrowed: { index: IssueIndex; key: string; matched: State["matched"]; value: Narrow } | null = null

/**
 * {@link Narrow} for a state; the list, the board, the toolbar and main.ts share one answer per change. While moai
 * has not answered the question on screen the ids it gave last stand; before any answer every row passes.
 */
export function narrow(state: NarrowState, index: IssueIndex): Narrow {
  const key = JSON.stringify([state.view, state.search.text, state.filter.applied])
  if (narrowed && narrowed.index === index && narrowed.key === key && narrowed.matched === state.matched) return narrowed.value
  const hides = viewHides(state.view, index)
  const asking = matchKey(state) !== ""
  const ids = asking ? state.matched.ids : null
  const searching = state.search.text.trim() !== ""
  const wanted = (at: number) => ids === null || ids.has(index.rows[at]!.id)
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
  narrowed = { index, key, matched: state.matched, value }
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

// ── lines ──────────────────────────────────────────────────────────────

/**
 * The listing at `state.path` with the open folders unfolded in place: `..` first below the root, then each
 * level's folders, rows and buckets in moai's order ({@link IssueIndex.entries}).
 */
export function listLines(state: Pick<State, "path" | "expanded">, index: IssueIndex, keep: Keep): ListLine[] {
  const open = new Set(state.expanded)
  const out: ListLine[] = []
  if (state.path.length) out.push({ kind: "up", key: UP_KEY, depth: 0, path: state.path.slice(0, -1) })
  const kin: boolean[] = []
  const walk = (here: Path) => {
    const entries = index.entries(here, keep)
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

/** One level of a branch, two cells wide: a line going on down, nothing, a line with a turn, the last turn. */
export type TwigPart = "pipe" | "blank" | "tee" | "end"
export const TWIG_TEXT: Record<TwigPart, string> = { pipe: "│ ", blank: "  ", tee: "├─", end: "└─" }

/** The levels of the branch before a line inside an unfolded folder; none at the listing's own level. */
export function twigParts(twig: Twig): TwigPart[] {
  if (twig.length < 2) return []
  return [...twig.slice(1, -1).map((more): TwigPart => (more ? "pipe" : "blank")), twig[twig.length - 1] ? "tee" : "end"]
}

/** The branch as the TUI writes it (`│ ├─ └─`); nothing at the listing's own level. */
export function twigText(twig: Twig): string {
  return twigParts(twig).map((part) => TWIG_TEXT[part]).join("")
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

/**
 * A date in 6ch: `MM-DD` this year, the year otherwise; empty when unreadable. Both the date and "this year" are read
 * in `zone`, the zone the page draws in (`SPC o t`, data/zone.ts) when left out.
 */
export function shortDate(value: unknown, now: Date = new Date(), zone: string | undefined = zoneNow()): string {
  const at = time(value)
  if (at === undefined) return ""
  const date = partsIn(at, zone)
  if (date.year !== partsIn(now.getTime(), zone).year) return String(date.year)
  return `${String(date.month).padStart(2, "0")}-${String(date.day).padStart(2, "0")}`
}

/** The four columns moai draws in their own colour; any other is `other`. */
const TONES = new Set(["todo", "in_progress", "review", "done"])
/** moai's glyphs (`style::glyph`) before `/api/boot` answers. */
const GLYPHS: Readonly<Record<string, string>> = { todo: "·", in_progress: "▸", review: "?", done: "✓" }

/**
 * The column glyph (look-and-feel §6): the glyph differs, so colour never carries the status alone. The glyph is
 * moai's (`/api/boot` `glyphs`, `unknown_glyph` for a column the config does not name).
 */
export function statusGlyph(status: string, boot?: Pick<Boot, "glyphs" | "unknown_glyph"> | null): { glyph: string; tone: string } {
  const glyph = boot ? (Object.hasOwn(boot.glyphs, status) ? boot.glyphs[status]! : boot.unknown_glyph) : GLYPHS[status] ?? "○"
  return { glyph, tone: TONES.has(status) ? status : "other" }
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
