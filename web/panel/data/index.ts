/**
 * The issue index: the pile of rows seen as directories. **Where a row stands is moai's** — `moai tui --json
 * --deep` (`/api/tree`) names each row's `home` (`nav::Index::home_of`: the epic beats the milestone, `parent.child`
 * folds, lost and twin rows), whether it opens (`dir`), its `n/n` (`progress`) and the order of every listing
 * (`entries_sorted`, with the order the page asked for). No DOM here, and no placement rule: the index is the
 * inverse image of `home`, so "every row shows up exactly once" follows from the construction.
 *
 * A tree line is joined by id to the row `/api/rows` or `/api/backlog` printed for it (its fields: deferral, epic,
 * milestone, dates). A line no list printed is the archive (`show --all` leaves it out): it stands with what the tree
 * says of it and `archived: true`, until the archive is read and its own row takes over (`SPC v o`).
 *
 * The steps of a path stay `m:<id>`, `e:<id>`, `i:<id>`, `m:` (the `(no milestone)` bucket) and `lost` — the tree's
 * handles (an id, `none`, `lost`) read by the kind of the row they name — so the views and the remembered folders
 * keep their words.
 */

export type Kind = 'issue' | 'epic' | 'milestone' | 'backlog'

/** One row as `moai --json show --all` (and `backlog show`) prints it, with the tree's keys. Only `id` is required. */
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
  derived_milestone?: string | null
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
  /** The nearest row that took this one out of the plan (`report::Shelf`): its own id when deferred itself. */
  shelved_by?: string
  /** `true` on a row `show --all` hides as the archive. */
  archived?: boolean
  [field: string]: unknown
}

/** One line of `moai tui --json --deep` (src/tui/json.rs). A bucket has no id; its `path` is `none` or `lost`. */
export interface TreeRow {
  id?: string
  title?: string
  kind?: string
  dir?: boolean
  path?: string
  status?: string
  derived_status?: string
  priority?: number
  progress?: { done: number; total: number }
  /** The handles from the root down to where it stands: ids, `none`, `lost`. */
  home?: string[]
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

/** Per tree line: for each level from the current directory down, whether more siblings follow it. */
export type Twig = readonly boolean[]

export interface TreeLine {
  entry: Entry
  twig: Twig
}

export type Keep = (at: number) => boolean

const kindOf = (row: Pick<Row, 'kind'>): Kind => {
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
export const isDone = (row: Row): boolean => row.status === 'done'
/**
 * Standing in the done column: a group's column is moai's `derived_status` (`report::stands_on`), read from its
 * members. This only reads the key.
 */
export const standsDone = (row: Row): boolean => (row.derived_status ?? row.status) === 'done'
/** Priority as moai prints it; 2 when a row was built without it (a test's). */
export const priorityOf = (row: Row): number => (typeof row.priority === 'number' ? row.priority : 2)

/** The id's parent: everything before the last `.`, if any. */
export function parentOf(id: string): string | undefined {
  const dot = id.lastIndexOf('.')
  return dot > 0 ? id.slice(0, dot) : undefined
}

interface Place {
  /** What stands right at this place, in the tree's order: rows by index, buckets by segment. */
  here: Array<{ at: number } | { seg: Seg }>
  /** Rows at this place and everything under it, in the tree's order. A folder row stands at its parent's place. */
  under: number[]
}

const TREE_KEYS = ['title', 'kind', 'status', 'derived_status', 'priority'] as const

export class IssueIndex {
  readonly rows: readonly Row[]
  private readonly byId = new Map<string, number>()
  private readonly homes: Path[] = []
  private readonly dirs: boolean[] = []
  private readonly progressOf: (readonly [number, number] | undefined)[] = []
  private readonly places = new Map<string, Place>()

  /**
   * `tree` is `/api/tree`'s answer; `rows` the rows the lists printed (`/api/rows` and `/api/backlog`, with the
   * archive's when it is shown). A tree line no list printed stands as an archived row. A row of `rows` the tree does
   * not hold — written between the two reads — stands at the root until the next read; a test that gives no tree at
   * all places every row so.
   */
  constructor(rows: readonly Row[], tree: readonly TreeRow[] = []) {
    // The later row wins an id, as in moai.
    const printed = new Map<string, Row>()
    for (const row of rows) printed.set(row.id, row)
    const out: Row[] = []
    const segs = new Map<string, Seg>()
    for (const line of tree) if (line.id !== undefined) segs.set(line.id, segFor(line.id, kindOf(line)))
    const segOfHandle = (handle: string): Seg => (handle === 'none' ? NO_MILESTONE : handle === 'lost' ? LOST : segs.get(handle) ?? `i:${handle}`)
    const placed = new Set<string>()
    const place = (key: string): Place => {
      let found = this.places.get(key)
      if (!found) {
        found = { here: [], under: [] }
        this.places.set(key, found)
      }
      return found
    }
    const stand = (row: Row, home: Path, dir: boolean, progress: TreeRow['progress']) => {
      const at = out.length
      out.push(row)
      this.byId.set(row.id, at)
      this.homes.push(home)
      this.dirs.push(dir)
      this.progressOf.push(progress ? [progress.done, progress.total] : undefined)
      for (let n = 0; n <= home.length; n++) place(pathKey(home.slice(0, n))).under.push(at)
      place(pathKey(home)).here.push({ at })
    }
    for (const line of tree) {
      const home = (line.home ?? []).map(segOfHandle)
      if (line.id === undefined) {
        // A bucket: it stands where its home says, in the tree's place among the rows there.
        const seg = line.path === 'lost' ? LOST : NO_MILESTONE
        place(pathKey(home)).here.push({ seg })
        place(pathKey([...home, seg]))
        continue
      }
      const own = printed.get(line.id)
      const row: Row = own ? { ...own } : { id: line.id, archived: true }
      // The tree's keys fill what the list did not print (an archived row), never what it did.
      for (const key of TREE_KEYS) if (row[key] === undefined && line[key] !== undefined) (row as Record<string, unknown>)[key] = line[key]
      placed.add(line.id)
      stand(row, home, line.dir === true, line.progress)
    }
    for (const row of rows) if (!placed.has(row.id) && printed.get(row.id) === row) stand(row, [], isGroup(row), undefined)
    this.rows = out
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
    return segFor(row.id, kindOf(row))
  }

  /** Can it be entered? moai's `dir`. */
  isDir(at: number): boolean {
    return this.dirs[at] === true && this.byId.get(this.rows[at]!.id) === at
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
   * The milestone a row stands under: moai's `derived_milestone` (left out under none). A row only the tree knows
   * (the archive, not read) has no such key, so the milestone its place names stands in.
   */
  milestoneOf(at: number): string | undefined {
    const row = this.rows[at]
    if (!row) return undefined
    const given = row.derived_milestone
    if (typeof given === 'string') return given || undefined
    if (given === null || !this.treeOnly(row)) return undefined
    return this.homes[at]?.find((s) => s.startsWith('m:') && s !== NO_MILESTONE)?.slice(2)
  }

  /** A row no list printed: it carries only what the tree says of it. */
  private treeOnly(row: Row): boolean {
    return row.archived === true && row.created_at === undefined
  }

  /** The epic a row stands under: moai's `derived_epic`, else the one its place names. */
  epicOf(at: number): string | undefined {
    const row = this.rows[at]
    if (!row) return undefined
    const given = row.derived_epic
    if (typeof given === 'string' && given !== '') return given
    return this.treeOnly(row) ? this.homes[at]?.find((s) => s.startsWith('e:'))?.slice(2) : undefined
  }

  /** The row that took this row out of the plan (itself, or an ancestor, epic or milestone): moai's `shelved_by`. */
  shelvedAt(at: number): string | undefined {
    const given = this.rows[at]?.shelved_by
    return typeof given === 'string' && given !== '' ? given : undefined
  }

  /** `shelvedAt` for a place known only by id (a blocker): the later row of that id answers. */
  deferredRoot(id: string): string | undefined {
    const at = this.byId.get(id)
    return at === undefined ? undefined : this.shelvedAt(at)
  }

  /** Is the row out of the plan by deferral? (`shelvedAt` names what took it out.) */
  isDeferred(at: number): boolean {
    return this.shelvedAt(at) !== undefined
  }

  /** The rows at `path` — kept ones, and folders with something kept under them — in moai's order. */
  entries(path: Path, keep: Keep = () => true): Entry[] {
    const place = this.places.get(pathKey(path))
    if (!place) return []
    const out: Entry[] = []
    for (const item of place.here) {
      if ('seg' in item) {
        if (this.litUnder([...path, item.seg], keep)) out.push({ dir: true, seg: item.seg })
        continue
      }
      const { at } = item
      const dir = this.isDir(at)
      if (!(keep(at) || (dir && this.litUnder(this.dirPath(at), keep)))) continue
      out.push(dir ? { dir: true, seg: this.segOf(at), at } : { dir: false, at })
    }
    return out
  }

  private litUnder(path: Path, keep: Keep): boolean {
    const place = this.places.get(pathKey(path))
    return place ? place.under.some(keep) : false
  }

  /** The listing with open folders unfolded in place, each line with its branch shape. */
  tree(path: Path, open: (path: Path) => boolean, keep: Keep = () => true): TreeLine[] {
    const out: TreeLine[] = []
    const kin: boolean[] = []
    const walk = (here: Path) => {
      const entries = this.entries(here, keep)
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

  /** Everything under a place, at any depth (the place's direct rows included), in moai's order. */
  descendants(path: Path): number[] {
    return [...(this.places.get(pathKey(path))?.under ?? [])]
  }

  /** A group row and everything drawn under it. Empty for a non-group. */
  underGroup(id: string): number[] {
    const at = this.byId.get(id)
    if (at === undefined || !isGroup(this.rows[at]!)) return []
    return [at, ...this.descendants(this.dirPath(at))]
  }

  /**
   * `[done, work]` under a place: moai's `progress` for a folder row (it counts the archive too). A bucket has
   * none, so the work rows the index holds under it are counted.
   */
  tally(path: Path): [number, number] {
    const last = path[path.length - 1]
    if (last !== undefined && last !== NO_MILESTONE && last !== LOST) {
      const at = this.byId.get(last.slice(2))
      const given = at === undefined ? undefined : this.progressOf[at]
      if (given) return [given[0], given[1]]
    }
    const work = this.descendants(path).filter((at) => isWork(this.rows[at]!))
    return [work.filter((at) => standsDone(this.rows[at]!)).length, work.length]
  }
}

function segFor(id: string, kind: Kind): Seg {
  if (kind === 'milestone') return `m:${id}`
  if (kind === 'epic') return `e:${id}`
  return `i:${id}`
}

/** Whole percent of done work; undefined when there is no work to count (not the same as 0%). */
export function percent([done, total]: readonly [number, number]): number | undefined {
  return total ? Math.floor((done * 100) / total) : undefined
}
