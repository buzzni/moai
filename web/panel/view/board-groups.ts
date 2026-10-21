/**
 * The board's grouping, pure (no DOM): which rows become cards, in which lane and which column.
 * Every placement rule comes from the IssueIndex — `descendants` for what is at the path, `homeOf` for the
 * milestone a row stands under, `shelvedAt` for deferral. This file only sorts those answers into a grid.
 * Epics and milestones are not cards; they are lane heads.
 */
import { archiveCandidate, displayOrder, isBacklog, isDone, isWork, LOST, NO_MILESTONE, type IssueIndex, type Keep, type Path, type Row, type Seg } from '../data/index.js'
import type { Sort } from '../data/query.js'
import type { ViewSettings } from '../store.js'
import { ARCHIVE_DAYS, sortCompare } from './list-model.ts'

/** The columns a board has when the project's own list of statuses is not known, in board order. */
export const DEFAULT_STATUSES: readonly string[] = ['todo', 'in_progress', 'review', 'done']

/** The two columns that stand before the statuses. */
export const BACKLOG = 'backlog'
export const DEFERRED = 'deferred'

export interface BoardColumn {
  /** A status, or {@link BACKLOG} / {@link DEFERRED}. */
  key: string
  /** Index of the cards' rows, in display order. */
  cards: number[]
}

export interface BoardLane {
  /** The milestone's segment (`m:<id>`, {@link NO_MILESTONE}, {@link LOST}); null for the one lane inside a place. */
  seg: Seg | null
  /** The milestone row of the lane, when it has one. */
  at?: number
  columns: BoardColumn[]
}

export interface BoardModel {
  /** Column keys in order, the same for every lane. */
  keys: string[]
  lanes: BoardLane[]
  /** Cards drawn, across all lanes. */
  total: number
  /**
   * The cursor's card is drawn only because it has the cursor (its column hidden, the filter or the archive age
   * would drop it), or the cursor stands on a group in a hidden column that only it keeps: once the cursor leaves,
   * the card or the column goes, so a cursor move cannot be drawn in place.
   */
  pinned: boolean
}

export interface BoardLook {
  path: Path
  /** The list's view toggles, the board is a layout of the same list: done rows past the archive age go unless `showArchived`. */
  view: Pick<ViewSettings, 'hiddenStatuses' | 'hideDeferred' | 'hideBacklog'> & Partial<Pick<ViewSettings, 'showArchived'>>
  /** The id under the cursor: its column stays even when hidden, so the cursor never vanishes. */
  cursor: string | null
  /** The list's order, so a card stands where its row stands in the list; left out, moai's default order. */
  sort?: Sort
  /**
   * The list's filter and search (list-model.ts narrow): a card must pass it. With `reveal` (a search is on) it
   * also lifts the view's hiding, as the list does, and a hidden column with a found card in it shows.
   */
  match?: Keep
  reveal?: boolean
  /** What "now" is for the archive age; the clock when left out. */
  now?: Date
}

/** The column a row stands in: backlog first, then deferred, then its status. */
export function columnOf(index: IssueIndex, at: number): string {
  const row = index.rows[at]!
  if (isBacklog(row)) return BACKLOG
  if (index.shelvedAt(at) !== undefined) return DEFERRED
  return String(row.status ?? DEFAULT_STATUSES[0])
}

/** Statuses of the board: the known ones in order, then any other status found in the rows, in first-seen order. */
export function statusKeys(rows: readonly Row[], known: readonly string[] = DEFAULT_STATUSES): string[] {
  const keys = [...known]
  for (const row of rows) {
    if (!isWork(row) || row.status === undefined) continue
    const status = String(row.status)
    if (!keys.includes(status)) keys.push(status)
  }
  return keys
}

/**
 * The lane a row stands in at the root: the first step of its home — a milestone, `(no milestone)` or
 * `(lost)`. A repository that does not use milestones has one lane and no name for it (null: no lane head),
 * as the TUI's `Site::lane`; `(no milestone)` there would head the only lane with a bucket that does not exist.
 */
function laneOf(index: IssueIndex, at: number): Seg | null {
  const first = index.homeOf(at)[0]
  return first !== undefined && (first.startsWith('m:') || first === LOST) ? first : null
}

export function groupBoard(index: IssueIndex, look: BoardLook, known: readonly string[] = DEFAULT_STATUSES): BoardModel {
  const { path, view, cursor } = look
  const now = look.now ?? new Date()
  const all = [BACKLOG, DEFERRED, ...statusKeys(index.rows, known)]
  const hidden = new Set(view.hiddenStatuses)
  const hides = (key: string) =>
    key === BACKLOG ? view.hideBacklog : key === DEFERRED ? view.hideDeferred : hidden.has(key)

  // Work, and a thought that is still open: the TUI board counts only open backlog (a closed one is `unopenable`).
  const cards = index.descendants(path).filter((at) => {
    const row = index.rows[at]!
    return index.find(row.id) === at && (isWork(row) || (isBacklog(row) && !isDone(row)))
  })
  const cursorAt = cursor === null ? undefined : index.find(cursor)
  const cursorColumn = cursorAt === undefined ? undefined : columnOf(index, cursorAt)

  const match = look.match ?? (() => true)
  // A hidden column goes, and so does a row the list would archive; a card that has the cursor keeps its place.
  // The list's filter and search drop a card first; a search on (`reveal`) lifts the hiding, as in the list.
  const passes = (at: number) => {
    if (!match(at)) return false
    if (look.reveal) return true
    if (hides(columnOf(index, at))) return false
    // Cards are work and open thoughts: a group is never a card, so no members to date it by.
    return view.showArchived === true || !archiveCandidate(index.rows[at]!, now, ARCHIVE_DAYS)
  }
  const shown = (at: number) => at === cursorAt || passes(at)
  const kept = cards.filter(shown)
  const found = look.reveal ? new Set(kept.map((at) => columnOf(index, at))) : null
  const keys = all.filter((key) => !hides(key) || key === cursorColumn || found?.has(key))
  // What only the cursor keeps: its card, or (the cursor on a group, say) the hidden column it stands in.
  const pinned =
    (cursorAt !== undefined && kept.includes(cursorAt) && !passes(cursorAt)) ||
    (cursorColumn !== undefined && hides(cursorColumn) && !found?.has(cursorColumn))

  const compare = look.sort ? sortCompare(look.sort) : displayOrder
  const order = (a: number, b: number) => compare(index.rows[a]!, index.rows[b]!)
  const make = (seg: Seg | null, members: number[], at?: number): BoardLane => ({
    seg,
    at,
    columns: keys.map((key) => ({ key, cards: members.filter((m) => columnOf(index, m) === key).sort(order) })),
  })

  if (path.length) return { keys, lanes: kept.length ? [make(null, kept)] : [], total: kept.length, pinned }

  const bySeg = new Map<Seg | null, number[]>()
  for (const at of kept) {
    const seg = laneOf(index, at)
    const list = bySeg.get(seg)
    if (list) list.push(at)
    else bySeg.set(seg, [at])
  }
  // Milestones in the listing's order, `(no milestone)` and `(lost)` after them.
  const order0 = index.entries([]).filter((entry) => entry.dir).map((entry) => entry.seg)
  const segs = [...bySeg.keys()].sort((a, b) => {
    const rank = (seg: Seg | null) => (seg === null ? -1 : seg === NO_MILESTONE ? 1e6 : seg === LOST ? 1e6 + 1 : order0.indexOf(seg))
    return rank(a) - rank(b)
  })
  const lanes = segs.map((seg) => {
    const home = seg !== null && seg.startsWith('m:') && seg !== NO_MILESTONE ? index.find(seg.slice(2)) : undefined
    return make(seg, bySeg.get(seg)!, home)
  })
  return { keys, lanes, total: kept.length, pinned }
}
