/**
 * The board's grouping, pure (no DOM): which rows become cards, in which lane and which column.
 * Where a row stands is moai's (data/index.ts: `descendants` for what is at the path, `homeOf` for the lane,
 * `shelvedAt` for deferral, the order of the tree for the order of the cards). This file only sorts those answers
 * into a grid. Closed epics stand as done cards; milestones head lanes.
 */
import { closedGroup, isBacklog, isDone, isWork, LOST, pathKey, type IssueIndex, type Keep, type Path, type Row, type Seg } from '../data/index.js'
import type { ViewSettings } from '../store.js'

/** The columns moai has when `.moai/config.toml` names none, until `/api/boot` gives the project's own. */
export const FALLBACK_STATUSES: readonly string[] = ['todo', 'in_progress', 'review', 'done']

/** The two columns that stand before the statuses. */
export const BACKLOG = 'backlog'
export const DEFERRED = 'deferred'

export interface BoardColumn {
  /** A status, or {@link BACKLOG} / {@link DEFERRED}. */
  key: string
  /** Index of the cards' rows, in the tree's order. */
  cards: number[]
}

export interface BoardLane {
  /** The milestone's segment (`m:<id>`, `m:`, {@link LOST}); null for the one lane inside a place. */
  seg: Seg | null
  /** The milestone row of the lane, when it has one. */
  at?: number
  /** Cards before this lane's fold hides them. */
  count: number
  folded: boolean
  columns: BoardColumn[]
}

export interface BoardModel {
  /** Column keys in order, the same for every lane. */
  keys: string[]
  lanes: BoardLane[]
  /** Cards drawn, across all lanes. */
  total: number
  /**
   * The cursor's card is drawn only because it has the cursor (its column hidden, the filter or the archive would
   * drop it), or the cursor stands on a group in a hidden column that only it keeps: once the cursor leaves,
   * the card or the column goes, so a cursor move cannot be drawn in place.
   */
  pinned: boolean
}

export interface BoardLook {
  path: Path
  /** The list's view toggles, the board is a layout of the same list: archived rows go unless `showArchived`. */
  view: Pick<ViewSettings, 'hiddenStatuses' | 'hideDeferred' | 'hideBacklog'> & Partial<Pick<ViewSettings, 'showArchived'>>
  /** The id under the cursor: its column stays even when hidden, so the cursor never vanishes. */
  cursor: string | null
  /**
   * The list's filter and search (list-model.ts narrow): a card must pass it. With `reveal` (a search is on) it
   * also lifts the view's hiding, as the list does, and a hidden column with a found card in it shows.
   */
  match?: Keep
  reveal?: boolean
  /** Paths folded during this board visit. Search reveals matches without changing this set. */
  shut?: ReadonlySet<string>
}

/**
 * The column a row stands in on the board: backlog first, then deferred, then its status. The same screen rule
 * as the explorer's board (`tui::board::column_of`) — a drawing rule, not a placement: the deferral is moai's
 * `shelved_by`, the status the row's own.
 */
export function columnOf(index: IssueIndex, at: number): string {
  const row = index.rows[at]!
  if (isBacklog(row)) return BACKLOG
  if (index.shelvedAt(at) !== undefined) return DEFERRED
  return String(closedGroup(row) ? row.derived_status ?? row.status : row.status ?? FALLBACK_STATUSES[0])
}

/**
 * Statuses of the board: the config's, in its order (`/api/boot` `statuses`), then any other status found in the
 * rows, in first-seen order — as the explorer's `columns`.
 */
export function statusKeys(rows: readonly Row[], known: readonly string[] = FALLBACK_STATUSES): string[] {
  const keys = [...known]
  for (const row of rows) {
    if (!isWork(row) || row.status === undefined) continue
    const status = String(row.status)
    if (!keys.includes(status)) keys.push(status)
  }
  return keys
}

/**
 * The lane a row stands in at the root: the first step of its home (`home[0]`) — a milestone, `(no milestone)` or
 * `(lost)`. A repository that does not use milestones has one lane and no name for it (null: no lane head),
 * as the TUI's `Site::lane`; `(no milestone)` there would head the only lane with a bucket that does not exist.
 */
function laneOf(index: IssueIndex, at: number): Seg | null {
  const first = index.homeOf(at)[0]
  return first !== undefined && (first.startsWith('m:') || first === LOST) ? first : null
}

export function groupBoard(index: IssueIndex, look: BoardLook, known: readonly string[] = FALLBACK_STATUSES): BoardModel {
  const { path, view, cursor } = look
  const all = [BACKLOG, DEFERRED, ...statusKeys(index.rows, known)]
  const hidden = new Set(view.hiddenStatuses)
  const hides = (key: string) =>
    key === BACKLOG ? view.hideBacklog : key === DEFERRED ? view.hideDeferred : hidden.has(key)

  // Work, and a thought that is still open: the TUI board counts only open backlog (a closed one is `unopenable`).
  const cards = index.descendants(path).filter((at) => {
    const row = index.rows[at]!
    return index.find(row.id) === at && (isWork(row) || (isBacklog(row) && !isDone(row)) || (row.kind === 'epic' && closedGroup(row)))
  })
  const cursorAt = cursor === null ? undefined : index.find(cursor)
  const cursorColumn = cursorAt === undefined ? undefined : columnOf(index, cursorAt)

  const match = look.match ?? (() => true)
  // A hidden column goes, and so does an archived row; a card that has the cursor keeps its place. The list's
  // filter and search drop a card first; a search on (`reveal`) lifts the hiding, as in the list.
  const passes = (at: number) => {
    if (!match(at)) return false
    if (look.reveal) return true
    if (hides(columnOf(index, at))) return false
    return view.showArchived === true || index.rows[at]!.archived !== true
  }
  const shown = (at: number) => at === cursorAt || passes(at)
  const kept = cards.filter(shown).filter((at) => {
    if (look.reveal || at === cursorAt || columnOf(index, at) !== 'done') return true
    const epic = index.epicOf(at)
    const home = epic === undefined ? undefined : index.find(epic)
    if (home === undefined || !closedGroup(index.rows[home]!)) return true
    const folded = index.dirPath(home)
    // Inside the epic its fold does not apply: there it is the path, not a card, so nothing could open it again.
    if (folded.every((seg, n) => path[n] === seg)) return true
    return !look.shut?.has(pathKey(folded))
  })
  const found = look.reveal ? new Set(kept.map((at) => columnOf(index, at))) : null
  const keys = all.filter((key) => !hides(key) || key === cursorColumn || found?.has(key))
  // What only the cursor keeps: its card, or (the cursor on a group, say) the hidden column it stands in.
  const pinned =
    (cursorAt !== undefined && kept.includes(cursorAt) && !passes(cursorAt)) ||
    (cursorColumn !== undefined && hides(cursorColumn) && !found?.has(cursorColumn))

  // `descendants` is in the tree's order, which is the order the list draws: a card stands where its row stands.
  const make = (seg: Seg | null, members: number[], at?: number): BoardLane => {
    const folded = !look.reveal && seg !== null && !!look.shut?.has(pathKey([seg]))
    return { seg, at, count: members.length, folded,
      columns: keys.map((key) => ({ key, cards: folded ? [] : members.filter((m) => columnOf(index, m) === key) })) }
  }

  if (path.length) return { keys, lanes: kept.length ? [make(null, kept)] : [], total: kept.length, pinned }

  const bySeg = new Map<Seg | null, number[]>()
  for (const at of kept) {
    const seg = laneOf(index, at)
    const list = bySeg.get(seg)
    if (list) list.push(at)
    else bySeg.set(seg, [at])
  }
  // Lanes in the listing's order (milestones, then the baskets, as moai lists them); no lane head first.
  const order0 = index.entries([]).filter((entry) => entry.dir).map((entry) => entry.seg)
  const segs = [...bySeg.keys()].sort((a, b) => (a === null ? -1 : order0.indexOf(a)) - (b === null ? -1 : order0.indexOf(b)))
  const lanes = segs.map((seg) => {
    const home = seg !== null && seg.startsWith('m:') && seg !== 'm:' ? index.find(seg.slice(2)) : undefined
    return make(seg, bySeg.get(seg)!, home)
  })
  return { keys, lanes, total: kept.length, pinned }
}
