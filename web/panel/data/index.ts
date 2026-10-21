/**
 * The issue index: the pile of rows seen as directories, ported from moai's `nav.rs` (and the parts of
 * `report.rs` it leans on). No DOM here; `node --test` checks it against `moai tui --json --path` output.
 *
 * One rule places a row — {@link IssueIndex.homeOf}. Listings are its inverse image, so "every row shows
 * up exactly once" follows from the construction instead of being kept by several listing rules.
 *
 * - Milestones stand at the root, an epic under its own milestone, work under the epic it belongs to.
 * - **The epic beats the milestone**: a member stands under its epic even when it wrote another milestone.
 * - `parent.child` folds under its parent when both are in the same epic.
 * - Backlog rows are leaves at the root or in the `(no milestone)` bucket; references that cannot be
 *   placed go to the `(lost)` bucket instead of vanishing.
 *
 * Known gap against the CLI: duplicate ids are only half-handled. The later row wins every lookup and an
 * earlier row of another kind goes to `(lost)` (moai's "eclipsed"), but the twin-parent rules of
 * `report::Handing` are not ported; `moai status` reports duplicate ids on its own.
 */

export type Kind = 'issue' | 'epic' | 'milestone' | 'backlog'

/** One row as `moai --json show --all` (and `backlog show`) prints it. Only `id` is required. */
export interface Row {
  id: string
  title?: string
  kind?: Kind | string
  status?: string
  derived_status?: string
  priority?: number
  epic?: string
  derived_epic?: string
  milestone?: string
  deferred_at?: string
  status_since?: string
  created_at?: string
  updated_at?: string
  tags?: string[]
  assignee?: string
  assignee_email?: string
  blocked_by?: string[]
  /** CLI's unread verdict: `true` or absent (see read.ts). */
  unread?: boolean
  // The CLI's own derivations (moai 0.17.0 `show --json`), each printed only when it holds. When present they win
  // where the panel decides the same thing (deferral in the index's shelf and in query.ts, the milestone in
  // IssueIndex.milestoneOf and query.ts, the archive in archiveCandidate and query.ts); when absent the panel
  // derives the values itself, so an older moai reads the same. The tree's placement (IssueIndex.homeOf) keeps
  // the index's own rule either way.
  /** The milestone the row stands on (null or empty: none; absent: the index reads it from the row's place). */
  derived_milestone?: string | null
  /** The nearest row that took this one out of the plan: its own id when deferred itself, else a group above. */
  shelved_by?: string
  /**
   * `true` on a row `show --all` hides as the archive (moai 0.17 prints it on such rows of `show --archived`).
   * source.ts markArchived tags the same way an archive row the key is not on yet the rows held now lack: every
   * archive-only row of an older moai, and one the clock archived after the archive was read.
   */
  archived?: boolean
  [field: string]: unknown
}

/**
 * One step of a path. `m:<id>` is a milestone, `m:` the `(no milestone)` bucket, `e:<id>` an epic,
 * `i:<id>` an issue or backlog row with children, and `lost` the `(lost)` bucket.
 */
export type Seg = string
export type Path = readonly Seg[]

export const NO_MILESTONE: Seg = 'm:'
export const LOST: Seg = 'lost'
const SEP = '\u001f'

/** A path as a map key. Segments never contain the separator: ids are printable. */
export const pathKey = (path: Path): string => path.join(SEP)

/** A listing line. `at` is the row's index (not its id, which may be duplicated); buckets have none. */
export type Entry = { dir: true; seg: Seg; at?: number } | { dir: false; at: number }

export interface Progress {
  /** Everything under the place, work or not. */
  kids: number
  /** Indexes of the work rows under it. */
  work: number[]
  done: number
}

/** Per tree line: for each level from the current directory down, whether more siblings follow it. */
export type Twig = readonly boolean[]

export interface TreeLine {
  entry: Entry
  twig: Twig
}

export type Keep = (at: number) => boolean
export type Order = (a: number, b: number) => number

const kindOf = (row: Row): Kind => {
  const kind = row.kind
  // Moai stores a backlog row as `idea`; the JSON says `backlog`. Anything unknown is work.
  if (kind === 'epic' || kind === 'milestone' || kind === 'backlog') return kind
  if (kind === 'idea') return 'backlog'
  return 'issue'
}

/** Groups hold things; backlog rows do not, although they are not work either. */
export const isGroup = (row: Row): boolean => {
  const kind = kindOf(row)
  return kind === 'epic' || kind === 'milestone'
}
/** Only issues are work — groups and backlog rows are not counted in `n/n`. */
export const isWork = (row: Row): boolean => kindOf(row) === 'issue'
export const isBacklog = (row: Row): boolean => kindOf(row) === 'backlog'
/** Rows that take an id parent's or an epic's membership. */
const joins = (row: Row): boolean => {
  const kind = kindOf(row)
  return kind === 'issue' || kind === 'backlog'
}
export const isDone = (row: Row): boolean => row.status === 'done'
/** Standing in the done column: a group's column is read from its members (`derived_status`), not the status written on it. */
export const standsDone = (row: Row): boolean => (isGroup(row) ? (row.derived_status ?? row.status) : row.status) === 'done'
/** Moai's default columns, in order: what a tracker has when `.moai/config.toml` names none of its own. */
export const DEFAULT_STATUSES: readonly string[] = ['todo', 'in_progress', 'review', 'done']
/** Priority as moai reads it: 2 when absent. */
export const priorityOf = (row: Row): number => (typeof row.priority === 'number' ? row.priority : 2)

const compareText = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0)

/** moai's `display_order`: priority, then id. */
export const displayOrder = (a: Row, b: Row): number => priorityOf(a) - priorityOf(b) || compareText(a.id, b.id)

/** The id's parent: everything before the last `.`, if any. */
export function parentOf(id: string): string | undefined {
  const dot = id.lastIndexOf('.')
  return dot > 0 ? id.slice(0, dot) : undefined
}

/** A closed row of work is not deferred, whatever it says; a group's own state is read from its members. */
const closedByHand = (row: Row): boolean => !isGroup(row) && isDone(row)
const isPutOff = (row: Row): boolean => typeof row.deferred_at === 'string' && row.deferred_at !== '' && !closedByHand(row)

interface Place {
  /** Rows standing right at this place: one listing level. */
  here: number[]
  /** Rows at this place and everything under it. A folder row itself stands at its parent's place. */
  under: number[]
  /** Buckets right below this place; they exist only when something lives in them. */
  buckets: Seg[]
  done: number
  work: number
}

export class IssueIndex {
  readonly rows: readonly Row[]
  private readonly byId = new Map<string, number>()
  private readonly kinds = new Map<string, Kind>()
  private readonly hasMilestones: boolean
  private readonly rootedThoughts = new Set<string>()
  private readonly homes: Path[]
  private readonly hasKids: boolean[]
  private readonly shelved: (string | undefined)[]
  private readonly places = new Map<string, Place>()

  private readonly counts: ReadonlyMap<string, readonly [number, number]>

  /**
   * `counts` are the milestone and epic `[done, total]` that `moai status` prints (`talliesFrom` in source.ts).
   * They include the rows the clock has archived, which `show --all` leaves out, so {@link tally} of a milestone
   * or epic place prefers them to its own count. Without them (or for an id they lack) the index counts its rows.
   */
  constructor(rows: readonly Row[], counts: ReadonlyMap<string, readonly [number, number]> = new Map()) {
    this.rows = rows
    this.counts = counts
    // The later row wins an id, as in moai.
    rows.forEach((row, at) => {
      this.byId.set(row.id, at)
      this.kinds.set(row.id, kindOf(row))
    })
    this.hasMilestones = rows.some((row) => kindOf(row) === 'milestone')
    for (const at of this.byId.values()) {
      const row = rows[at]!
      if (isBacklog(row) && this.rootsAway(row)) this.rootedThoughts.add(row.id)
    }

    const memo = new Map<number, Path>()
    this.homes = rows.map((_, at) => this.home(at, memo))

    this.hasKids = rows.map(() => false)
    for (const home of this.homes) {
      const last = home[home.length - 1]
      if (last?.startsWith('i:')) {
        const at = this.byId.get(last.slice(2))
        if (at !== undefined) this.hasKids[at] = true
      }
    }

    this.shelved = this.shelve()

    this.homes.forEach((home, at) => {
      const row = rows[at]!
      const work = isWork(row)
      const done = work && isDone(row) ? 1 : 0
      for (let n = 0; n <= home.length; n++) {
        const key = pathKey(home.slice(0, n))
        let place = this.places.get(key)
        if (!place) {
          place = { here: [], under: [], buckets: [], done: 0, work: 0 }
          this.places.set(key, place)
        }
        place.under.push(at)
        if (work) {
          place.done += done
          place.work += 1
        }
        const next = home[n]
        if (next === undefined) place.here.push(at)
        else if ((next === NO_MILESTONE || next === LOST) && !place.buckets.includes(next)) place.buckets.push(next)
      }
    })
  }

  // ── membership (report.rs) ─────────────────────────────────────────

  private line(id: string | undefined): Row | undefined {
    if (id === undefined) return undefined
    const at = this.byId.get(id)
    return at === undefined ? undefined : this.rows[at]
  }

  /** The epic a row stands in: the one it wrote, else what its id parent hands down (`derived_epic`). */
  private epicAt(row: Row): string | undefined {
    if (!joins(row)) return undefined
    return row.epic ?? row.derived_epic ?? undefined
  }

  /** What an id parent hands down to its children. */
  private handed(parent: Row): string | undefined {
    return this.epicAt(parent)
  }

  /**
   * A backlog row that does not fold under its id parent ("rooted thought"): no parent, a parent that is
   * not work, or an epic of its own the parent does not hand down.
   */
  private rootsAway(thought: Row): boolean {
    const parent = this.line(parentOf(thought.id))
    if (!parent || !joins(parent)) return true
    if (thought.epic === undefined) return false
    return this.handed(parent) !== thought.epic
  }

  private rooted(row: Row): boolean {
    return isBacklog(row) && this.rootedThoughts.has(row.id)
  }

  /** The top row a row without an epic folds under; undefined under a rooted thought. */
  private foldTop(row: Row): Row | undefined {
    let cur = row
    for (;;) {
      const parent = this.line(parentOf(cur.id))
      if (!parent || !joins(parent)) return cur
      if (this.rootedThoughts.has(parent.id)) return undefined
      cur = parent
    }
  }

  /** Climbing id parents from the fold top, the first milestone met (an epic's own, a milestone, or a written one). */
  private climb(top: Row): string | undefined {
    let cur: Row | undefined = top
    while (cur) {
      const kind = kindOf(cur)
      if (kind === 'epic') return cur.milestone
      if (kind === 'milestone') return cur.id
      if (cur.milestone !== undefined) return cur.milestone
      const parent = this.line(parentOf(cur.id))
      cur = parent && !this.rootedThoughts.has(parent.id) ? parent : undefined
    }
    return undefined
  }

  /** The milestone a row stands on (`stood_at_line`): the epic beats the row's own field. */
  private stoodAt(row: Row, epic: string | undefined): string | undefined {
    switch (kindOf(row)) {
      case 'epic':
        return row.milestone
      case 'milestone':
        return undefined
      default: {
        if (epic !== undefined) {
          const line = this.line(epic)
          return line && kindOf(line) === 'epic' ? line.milestone : undefined
        }
        const top = this.foldTop(row)
        return top ? this.climb(top) : undefined
      }
    }
  }

  /** A row whose epic or milestone reference cannot be placed (`misplace_of`). */
  private adrift(row: Row): boolean {
    const usable = (id: string | undefined, kind: Kind) => id === undefined || this.kinds.get(id) === kind
    switch (kindOf(row)) {
      case 'milestone':
        return false
      case 'epic':
        return !usable(row.milestone, 'milestone')
      default: {
        const epic = this.epicAt(row)
        if (epic !== undefined) return this.kinds.get(epic) !== 'epic'
        return !usable(this.stoodAt(row, undefined), 'milestone')
      }
    }
  }

  /** An earlier row whose id a later row of another kind took. */
  private eclipsed(row: Row): boolean {
    return this.kinds.get(row.id) !== kindOf(row)
  }

  // ── placing (nav.rs Ctx) ───────────────────────────────────────────

  private home(at: number, memo: Map<number, Path>): Path {
    const known = memo.get(at)
    if (known) return known
    const row = this.rows[at]!
    let path: Path
    if (this.adrift(row) || this.eclipsed(row)) path = [LOST]
    else {
      const kind = kindOf(row)
      if (kind === 'milestone') path = []
      else if (kind === 'epic') path = this.underMilestoneOf(row)
      else path = this.homeOfWork(at, memo)
    }
    memo.set(at, path)
    return path
  }

  private milestoneSeg(milestone: string | undefined): Path {
    if (!this.hasMilestones) return []
    return [milestone === undefined ? NO_MILESTONE : `m:${milestone}`]
  }

  /** Where an epic (by id) stands. */
  private underMilestone(epic: string): Path {
    const line = this.line(epic)
    return this.milestoneSeg(line ? this.stoodAt(line, undefined) : undefined)
  }

  private underMilestoneOf(row: Row): Path {
    return this.milestoneSeg(this.stoodAt(row, undefined))
  }

  private homeOfWork(at: number, memo: Map<number, Path>): Path {
    const row = this.rows[at]!
    const parentId = parentOf(row.id)
    const parentAt = parentId === undefined ? undefined : this.byId.get(parentId)
    if (parentId !== undefined && parentAt !== undefined && joins(this.rows[parentAt]!)) {
      const parent = this.rows[parentAt]!
      // The parent's own place, even when its references sent it to (lost).
      const path = this.home(parentAt, memo)
      const rootedThought = isBacklog(parent) && !path[path.length - 1]?.startsWith('i:')
      const passed = rootedThought ? undefined : this.handed(parent)
      if (this.epicAt(row) === passed && !this.rooted(row)) return [...path, `i:${parentId}`]
    }
    // A thought is not hung in the plan: it would show under an epic that counts `0/0`.
    if (isBacklog(row)) return this.hasMilestones ? [NO_MILESTONE] : []
    const epic = this.epicAt(row)
    if (epic !== undefined) {
      const epicLine = this.line(epic)
      const base = epicLine && this.adrift(epicLine) ? [LOST] : this.underMilestone(epic)
      return [...base, `e:${epic}`]
    }
    return this.underMilestoneOf(row)
  }

  // ── deferral (report.rs Shelf) ─────────────────────────────────────

  /** The milestone a row stands on, for an ancestor looked up by id. */
  private stoneOf(id: string): string | undefined {
    const line = this.line(id)
    return line ? this.stoodAt(line, this.epicAt(line)) : undefined
  }

  /** The nearest row that takes this one out of the plan: itself, its epic or milestone, then its parents. */
  private nearestShelf(from: Row): string | undefined {
    const ownIf = (id: string | undefined, kind: Kind): string | undefined => {
      const line = this.line(id)
      return line && kindOf(line) === kind && isPutOff(line) ? line.id : undefined
    }
    const start = from.id
    let cur: string | undefined = start
    while (cur !== undefined) {
      if (cur !== start && this.rootedThoughts.has(cur)) {
        const line = this.line(cur)
        return line && isPutOff(line) ? cur : undefined
      }
      const line = cur === start ? from : this.line(cur)
      if (!line) return undefined
      if (isPutOff(line)) return cur
      const kind = kindOf(line)
      if (kind === 'issue' || kind === 'backlog') {
        const shelf = ownIf(this.epicAt(line), 'epic')
        if (shelf) return shelf
      }
      if (kind !== 'milestone') {
        const stone = cur === start ? this.stoodAt(line, this.epicAt(line)) : this.stoneOf(cur)
        const shelf = ownIf(stone, 'milestone')
        if (shelf) return shelf
      }
      const parent = parentOf(cur)
      cur = parent !== undefined && this.byId.has(parent) ? parent : undefined
    }
    return undefined
  }

  private shelve(): (string | undefined)[] {
    const walk = this.rows.some(isPutOff)
    return this.rows.map((row) => {
      // moai 0.17 `shelved_by` names the shelf when the row carries it (that shelf may not be loaded here); its
      // absence says nothing an older moai would not, so the walk decides then.
      const given = row.shelved_by
      if (typeof given === 'string' && given !== '') return given
      return walk ? this.shelfOf(row) : undefined
    })
  }

  private shelfOf(row: Row): string | undefined {
    if (closedByHand(row)) return undefined
    // A group whose members are all done reads done, and a done row is not deferred.
    if (isGroup(row) && row.derived_status === 'done') return undefined
    return this.nearestShelf(row)
  }

  // ── reading ────────────────────────────────────────────────────────

  find(id: string): number | undefined {
    return this.byId.get(id)
  }

  /** The one place a row stands. */
  homeOf(at: number): Path {
    return this.homes[at]!
  }

  segOf(at: number): Seg {
    const row = this.rows[at]!
    const kind = kindOf(row)
    if (kind === 'milestone') return `m:${row.id}`
    if (kind === 'epic') return `e:${row.id}`
    return `i:${row.id}`
  }

  /** Can it be entered? Groups always (an empty epic too); work when something folds under it. */
  isDir(at: number): boolean {
    const row = this.rows[at]!
    return this.byId.get(row.id) === at && (isGroup(row) || this.hasKids[at]!)
  }

  /** The place a row opens as a folder: its home plus its own segment. */
  dirPath(at: number): Path {
    return [...this.homes[at]!, this.segOf(at)]
  }

  /**
   * The place `moai tui --path` names: a row id opens its folder, `none` and `lost` the buckets.
   * Undefined for an unknown id.
   */
  pathOf(name: string): Path | undefined {
    if (name === '') return []
    if (name === 'none') return [NO_MILESTONE]
    if (name === 'lost') return [LOST]
    const at = this.byId.get(name)
    return at === undefined ? undefined : this.dirPath(at)
  }

  /**
   * The milestone a row stands on by `report::stood_at_line`, the answer `moai show --milestone` filters by: the
   * epic beats the milestone the row wrote, an id child stands where its fold top stands (its own field is not
   * read), a row naming an epic that is not there stands on none, and an eclipsed twin on none. It is not where
   * the row was put: an epic that wrote a milestone that does not exist stands on that id still, and sits in
   * `(lost)`.
   */
  stoodAtLine(at: number): string | undefined {
    const row = this.rows[at]!
    if (this.eclipsed(row)) return undefined
    return this.stoodAt(row, this.epicAt(row))
  }

  /**
   * The milestone a row stands under: moai's `derived_milestone` when the row carries it (null or empty: none),
   * else read from its place (not its own field) — moai 0.17 leaves the key out on a row under none, where the
   * place reads none too. The place itself ({@link homeOf}) always follows the index's own rule, so the two can
   * differ when moai knows better.
   */
  milestoneOf(at: number): string | undefined {
    const given = this.rows[at]?.derived_milestone
    if (typeof given === 'string') return given || undefined
    if (given === null) return undefined
    const seg = this.homes[at]?.find((s) => s.startsWith('m:') && s !== NO_MILESTONE)
    return seg?.slice(2)
  }

  /** The epic a row actually stands under. */
  epicOf(at: number): string | undefined {
    return this.homes[at]?.find((s) => s.startsWith('e:'))?.slice(2)
  }

  /**
   * The row that took this row out of the plan (itself, or an ancestor, epic or milestone); undefined if planned.
   * moai's `shelved_by` names it when the row carries it (see {@link shelve}).
   */
  shelvedAt(at: number): string | undefined {
    return this.shelved[at]
  }

  /** `shelvedAt` for a place known only by id (a blocker): the later row of that id answers. */
  deferredRoot(id: string): string | undefined {
    const at = this.byId.get(id)
    return at === undefined ? undefined : this.shelved[at]
  }

  /** Is the row out of the plan by deferral? (`shelvedAt` names what took it out.) */
  isDeferred(at: number): boolean {
    return this.shelved[at] !== undefined
  }

  /** The rows at `path` — kept ones, and folders with something kept under them — folders first, buckets last. */
  entries(path: Path, keep: Keep = () => true, order: Order = (a, b) => displayOrder(this.rows[a]!, this.rows[b]!)): Entry[] {
    const place = this.places.get(pathKey(path))
    if (!place) return []
    const out: Entry[] = []
    for (const at of place.here) {
      const dir = this.isDir(at)
      if (!(keep(at) || (dir && this.litUnder(path, this.segOf(at), keep)))) continue
      out.push(dir ? { dir: true, seg: this.segOf(at), at } : { dir: false, at })
    }
    out.sort((a, b) => Number(!a.dir) - Number(!b.dir) || order(a.at!, b.at!))
    const buckets = place.buckets.filter((seg) => this.litUnder(path, seg, keep))
    buckets.sort((a, b) => Number(a === LOST) - Number(b === LOST))
    for (const seg of buckets) out.push({ dir: true, seg })
    return out
  }

  private litUnder(path: Path, seg: Seg, keep: Keep): boolean {
    const place = this.places.get(pathKey([...path, seg]))
    return place ? place.under.some(keep) : false
  }

  /** The listing with open folders unfolded in place, each line with its branch shape. */
  tree(path: Path, open: (path: Path) => boolean, keep: Keep = () => true, order?: Order): TreeLine[] {
    const out: TreeLine[] = []
    const kin: boolean[] = []
    const walk = (here: Path) => {
      const entries = this.entries(here, keep, order)
      entries.forEach((entry, n) => {
        kin.push(n < entries.length - 1)
        out.push({ entry, twig: [...kin] })
        if (entry.dir) {
          const under = [...here, entry.seg]
          if (open(under)) walk(under)
        }
        kin.pop()
      })
    }
    walk(path)
    return out
  }

  /** Everything under a place, at any depth (the place's direct rows included). */
  descendants(path: Path): number[] {
    return [...(this.places.get(pathKey(path))?.under ?? [])]
  }

  /** A group row and everything drawn under it — what reading a group marks. Empty for a non-group. */
  underGroup(id: string): number[] {
    const at = this.byId.get(id)
    if (at === undefined || !isGroup(this.rows[at]!)) return []
    return [at, ...this.descendants(this.dirPath(at))]
  }

  /**
   * Progress under a place, from the rows the index holds (what dates a group for the archive). The `n/n` the list
   * and the detail draw is {@link tally}, which for a milestone or epic prefers the count `moai status` gave.
   */
  progress(path: Path): Progress {
    const kids = this.descendants(path)
    const work = kids.filter((at) => isWork(this.rows[at]!))
    return { kids: kids.length, work, done: work.filter((at) => isDone(this.rows[at]!)).length }
  }

  /**
   * `[done, work]` under a place. A milestone or epic place (its last segment `m:<id>` / `e:<id>`) takes the
   * count `moai status` gave for that id when the index was built with one; the (none) and (lost) buckets and
   * places of parent-folded rows always keep their own count.
   */
  tally(path: Path): [number, number] {
    const last = path[path.length - 1]
    if (last !== undefined && last !== NO_MILESTONE && (last.startsWith('m:') || last.startsWith('e:'))) {
      const given = this.counts.get(last.slice(2))
      if (given) return [given[0], given[1]]
    }
    const place = this.places.get(pathKey(path))
    return place ? [place.done, place.work] : [0, 0]
  }
}

/**
 * Standing in the done column for `archiveDays` whole days or more — what moai hides as archived
 * (`report::archived`). A row that says `archived: true` (moai 0.17 on `show --archived`, or source.ts
 * markArchived) is one whatever the clock reads; otherwise it is derived here, off when `archiveDays` is 0.
 *
 * The column is the one the row STANDS in: a group's is read from its members (`derived_status`), not the
 * `status` written on it. And since when is the day it got there: for a group that is the latest
 * `status_since` among its members (`members`: the work under it), `created_at` when it has none — a group's
 * own `status_since` never moves when its members do. (`report::Stand::entered` also counts from the day a
 * deferred member was set aside when that is later; that case is not read here.)
 */
export function archiveCandidate(row: Row, now: Date, archiveDays: number, members: readonly Row[] = []): boolean {
  if (row.archived === true) return true
  if (!(archiveDays > 0)) return false
  const group = isGroup(row)
  if (!standsDone(row)) return false
  let since: unknown = row.status_since
  if (group) {
    let latest: number | undefined
    for (const member of members) {
      const at = typeof member.status_since === 'string' ? Date.parse(member.status_since) : NaN
      if (Number.isFinite(at) && (latest === undefined || at > latest)) latest = at
    }
    since = latest === undefined ? row.created_at : new Date(latest).toISOString()
  }
  if (typeof since !== 'string') return false
  const at = Date.parse(since)
  return Number.isFinite(at) && Math.floor((now.getTime() - at) / 86_400_000) >= archiveDays
}

/** Whole percent of done work; undefined when there is no work to count (not the same as 0%). */
export function percent(progress: Progress): number | undefined {
  return progress.work.length ? Math.floor((progress.done * 100) / progress.work.length) : undefined
}
