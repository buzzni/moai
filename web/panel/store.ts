/**
 * The panel's one state and the actions that change it. The reducer is pure — no I/O, and no DOM at run
 * time (it borrows the layout rules from view/responsive.ts and view/split.ts) — so every transition is
 * tested with `node --test`; views subscribe and render from the state they are given.
 *
 * List and board share the cursor, path, search, filter, order and detail (the board is a layout of the
 * list); the wiki tab keeps its own page, trail and search.
 *
 * One vocabulary with data/query.ts and keys/table.ts: the order is query.ts's {@link Sort} (its field for the
 * column is `status`), and the list cells are the key table's `CellField` names (the name line is `names`).
 */
import type { Outcome } from './data/client.js'
import { pathKey, type Path, type Row, type TreeRow } from './data/index.js'
import { DEFAULT_SORT, type Sort } from './data/query.js'
import type { ProjectEntry } from './data/projects.js'
import type { Theme } from './data/theme.js'
import type { Boot } from './data/source.js'
import { toolbarReduce, type ToolbarAction, type ToolbarState } from './view/toolbar.js'
import type { WikiState } from './view/wiki.js'
// The split owns the layout rules (the 640px rule, the ratio bounds and defaults); the store keeps the choices.
import { effectivePosition, type DetailPosition } from './view/responsive.js'
import { DEFAULT_RATIO, normalizeRatio } from './view/split.js'

export type { DetailPosition, Sort, WikiState }
export { DEFAULT_SORT }
export type Tab = 'board' | 'list' | 'wiki' | 'gantt'
export type Focus = 'list' | 'detail'
/** A list cell (`SPC c`), named as keys/table.ts's `CellField`: `names` is the sticky name line on top. */
export type ColumnName = 'id' | 'priority' | 'counts' | 'names' | 'tags' | 'epic' | 'assignee' | 'created' | 'updated'

export interface ViewSettings {
  /** Status columns hidden from the list and board (`SPC v 1~9`). */
  hiddenStatuses: string[]
  hideDeferred: boolean
  hideBacklog: boolean
  showArchived: boolean
  /** The other git worktrees' rows over the list (`SPC v w`), as the TUI's explorer starts: on (se-etwu). While it is
   *  on the rows are read with `/api/rows?worktree=1` (`show --all --worktree`). */
  worktree: boolean
}

export interface DetailSettings {
  visible: boolean
  position: DetailPosition
  /** The detail's share of the split, kept per position. */
  ratio: Record<DetailPosition, number>
}

/** A read the panel shows: its data once it arrived, and why the last attempt failed. */
export interface Loadable<T> {
  value: T | null
  loading: boolean
  problem: Outcome | null
}

export interface BoardData {
  rows: Row[]
  /** `/api/tree` (`tui --json --deep`): where every row stands and in which order (data/index.ts). */
  tree?: TreeRow[]
  /** What moai said on stderr beside the lists (source.ts Rows): a line it could not read, a notice. */
  notes?: string[]
  /**
   * Milestone and epic counts per column from `moai status` (source.ts loadGroups). Absent when that read failed, or
   * on the first read until it answers; a later read keeps the counts of the board before it until its own come.
   */
  groups?: ReadonlyMap<string, Readonly<Record<string, number>>>
  /** When the rows were read (ms since epoch). */
  loadedAt: number
}

/**
 * The ids moai found for the filter and the search on screen (`/api/rows?filter=…&q=…&fields=id` and the backlog's).
 * `key` names what they answer ({@link matchKey}); a newer question keeps the older ids until its own come, so the
 * list does not blink to everything between two keystrokes.
 */
export interface Matched {
  key: string
  ids: ReadonlySet<string> | null
  loading: boolean
  /** Why moai refused the question: a filter it cannot read says so in its own words (`bad_filter`). */
  problem: Outcome | null
}

/** What the filter and the search ask moai, as one key: nothing to ask when both are empty. */
export function matchKey(state: { filter: Pick<FilterState, 'applied'>; search: Pick<SearchState, 'text'>; view: Pick<ViewSettings, 'showArchived'> & Partial<Pick<ViewSettings, 'worktree'>> }): string {
  const filter = state.filter.applied.trim()
  const q = state.search.text.trim()
  // The worktree overlay is part of the question (data/source.ts matchEndpoints): turning it asks again.
  return filter === '' && q === '' ? '' : JSON.stringify([filter, q, state.view.showArchived, state.view.worktree === true])
}

export interface DetailData {
  id: string
  row: Row
}

/** The search (`/`): applies as it is typed and lifts the view's hiding (design §4.4). */
export interface SearchState {
  text: string
  /** The search row is shown in the toolbar. */
  open: boolean
}

/** The filter (`SPC f`): the text as typed, and the text that narrows the rows — Enter or a chip applies it (§4.5). */
export interface FilterState {
  text: string
  applied: string
}

/** What only the toolbar's drawing needs: the filter caret its hints follow, the hint list, the sort popover. */
export interface ToolbarUi {
  cursor: number
  hintsOpen: boolean
  hintIndex: number
  sortOpen: boolean
}

export interface State {
  tab: Tab
  path: Path
  /** The id under the cursor; null when nothing is chosen. */
  cursor: string | null
  focus: Focus
  /** Folders unfolded in the tree, as path keys. Kept for the session only, as in the TUI. */
  expanded: string[]
  detail: DetailSettings
  view: ViewSettings
  columns: Record<ColumnName, boolean>
  /** Columns the user has seen; a column added later starts at its default, one turned off stays off. */
  columnsKnown: ColumnName[]
  sort: Sort
  /** The order an emptied one returns to: the saved default (`D`), else {@link DEFAULT_SORT}. Not remembered here (data/memory.ts). */
  defaultSort: Sort
  search: SearchState
  filter: FilterState
  toolbar: ToolbarUi
  board: Loadable<BoardData>
  shown: Loadable<DetailData>
  /**
   * The rows the archive holds as read (source.ts loadArchived, active ones among them), read when archived rows
   * are shown (`view.showArchived`); null until then. source.ts withArchive tags the archive-only ones.
   */
  archive: Row[] | null
  /** Why the last archive read failed; the board's rows and its own problem stay as they are. */
  archiveProblem: Outcome | null
  /** `/api/boot`: the language, the columns, their glyphs and names, how a person is named; null until it answers. */
  boot: Boot | null
  matched: Matched
  /**
   * The wiki view's own state (view/wiki.ts state()), saved when the wiki tab is left; a wiki pane mounted on this
   * store starts from it (the mounted pane keeps its own state across tab switches).
   */
  wiki: WikiState
  /**
   * The projects the header lists and the digits jump to (data/projects.ts headerProjects, moai-o14f): read once from
   * `/api/projects` after the boot. Empty until then, and for good when nothing is registered or the read failed.
   */
  projects: ProjectEntry[]
  /**
   * The time zone picked on this page (`SPC o t`, data/zone.ts); null follows the TUI's (`/api/boot`'s `timezone`),
   * else the browser's. Remembered in the browser (data/memory.ts); never written to the server.
   */
  zone: string | null
  /** The colour theme picked on this page (data/theme.ts); `system` follows the browser. Remembered in the browser only. */
  theme: Theme
}

/**
 * The choices the panel remembers between openings (design §5): the last tab, and where and how large the
 * detail stands. data/memory.ts cleans what storage gives back and saves it again; the store only applies it.
 */
export interface Remembered {
  tab: Tab
  detail: DetailSettings
  /** The zone picked on this page (`SPC o t`); null when none is. Left out by an older page's settings. */
  zone?: string | null
  /** Light or dark, when one was picked; left out for `system`. */
  theme?: Theme
}

export const COLUMN_DEFAULTS: Record<ColumnName, boolean> = {
  id: true, priority: true, counts: true, names: true,
  tags: false, epic: false, assignee: false, created: false, updated: false,
}

export const initialWiki = (): WikiState => ({ current: null, cursor: null, trail: [], query: '', raw: false, scrollTop: 0, selectedLink: -1, pane: 'list' })

const copySort = (sort: Sort): Sort => ({ fields: sort.fields.map((field) => ({ ...field })), reversed: sort.reversed })

/** The toolbar's state as view/toolbar.ts reduces and draws it, read from the store's fields (the one source). */
export function toolbarOf(state: Pick<State, 'search' | 'filter' | 'sort' | 'toolbar'>): ToolbarState {
  return {
    searchOpen: state.search.open,
    searchText: state.search.text,
    filterText: state.filter.text,
    filterApplied: state.filter.applied,
    sort: state.sort,
    ...state.toolbar,
  }
}

/** A reduced toolbar state put back into the store's fields; a field that did not change keeps its object. */
function withToolbar(state: State, next: ToolbarState): State {
  const search = next.searchText === state.search.text && next.searchOpen === state.search.open
    ? state.search
    : { text: next.searchText, open: next.searchOpen }
  const filter = next.filterText === state.filter.text && next.filterApplied === state.filter.applied
    ? state.filter
    : { text: next.filterText, applied: next.filterApplied }
  const ui = state.toolbar
  const toolbar = next.cursor === ui.cursor && next.hintsOpen === ui.hintsOpen && next.hintIndex === ui.hintIndex && next.sortOpen === ui.sortOpen
    ? ui
    : { cursor: next.cursor, hintsOpen: next.hintsOpen, hintIndex: next.hintIndex, sortOpen: next.sortOpen }
  const sort = next.sort === state.sort ? state.sort : next.sort.fields.length ? copySort(next.sort) : copySort(state.defaultSort)
  if (search === state.search && filter === state.filter && toolbar === state.toolbar && sort === state.sort) return state
  return { ...state, search, filter, toolbar, sort }
}

const idle = <T>(): Loadable<T> => ({ value: null, loading: false, problem: null })

export function initialState(): State {
  return {
    tab: 'list',
    path: [],
    cursor: null,
    focus: 'list',
    expanded: [],
    detail: { visible: true, position: 'bottom', ratio: { ...DEFAULT_RATIO } },
    view: { hiddenStatuses: [], hideDeferred: true, hideBacklog: false, showArchived: false, worktree: true },
    columns: { ...COLUMN_DEFAULTS },
    columnsKnown: Object.keys(COLUMN_DEFAULTS) as ColumnName[],
    sort: copySort(DEFAULT_SORT),
    defaultSort: copySort(DEFAULT_SORT),
    search: { text: '', open: false },
    filter: { text: '', applied: '' },
    toolbar: { cursor: 0, hintsOpen: false, hintIndex: -1, sortOpen: false },
    board: idle(),
    shown: idle(),
    archive: null,
    archiveProblem: null,
    boot: null,
    matched: { key: '', ids: null, loading: false, problem: null },
    wiki: initialWiki(),
    projects: [],
    zone: null,
    theme: 'system',
  }
}

export type Action =
  | { type: 'tab'; tab: Tab }
  | { type: 'enter'; path: Path; cursor?: string | null }
  | { type: 'cursor'; id: string | null }
  | { type: 'focus'; focus: Focus }
  | { type: 'toggle-expanded'; path: Path }
  /** The unfolded folders all at once (Tab: everything under a group unfolds or folds), as path keys. */
  | { type: 'expanded'; expanded: string[] }
  | { type: 'detail-visible'; visible: boolean }
  | { type: 'detail-position'; position: DetailPosition }
  /** `position` is the one resized — the split reports the position it shows, which a narrow panel forces to the bottom. */
  | { type: 'detail-ratio'; ratio: number; position?: DetailPosition }
  | { type: 'view'; view: Partial<ViewSettings> }
  | { type: 'column'; column: ColumnName; on: boolean }
  /** The order (`SPC s`, a name-line click); no fields at all is the default order (`defaultSort`). */
  | { type: 'sort'; sort: Sort }
  /** The saved default order (`D`, or read at start-up); null when none is saved. The order on screen stays. */
  | { type: 'sort-default'; sort: Sort | null }
  /** Search text; a text opens the search row. */
  | { type: 'search'; text: string }
  /** Set the filter text and apply it at once (a chip, the key router); typing goes through `toolbar`. */
  | { type: 'filter'; text: string }
  /** Anything the toolbar did (view/toolbar.ts ToolbarAction), reduced by its own reducer over {@link toolbarOf}. */
  | { type: 'toolbar'; action: ToolbarAction }
  /** The wiki view's state, saved as its tab is left. */
  | { type: 'wiki'; wiki: WikiState }
  | { type: 'board-loading' }
  /** `order` is the ids in the order they were drawn before this read, to place the cursor after it. */
  | { type: 'board-loaded'; data: BoardData; order?: string[] }
  | { type: 'board-failed'; problem: Outcome }
  /**
   * The status counts that came after the rows of `data` (the board-loaded they belong to); null when they could not
   * be read. A later read's board has its own counts, so counts for any board but the one shown are dropped.
   */
  | { type: 'groups'; data: BoardData; groups: NonNullable<BoardData['groups']> | null }
  | { type: 'boot'; boot: Boot }
  /** A question about to go to moai (`key`, {@link matchKey}): the ids on screen stay until its answer; another question's refusal goes. */
  | { type: 'match-loading'; key: string }
  | { type: 'matched'; key: string; ids: ReadonlySet<string> }
  | { type: 'match-failed'; key: string; problem: Outcome }
  /** The archive's rows as read (null forgets them); either way an archive problem told before goes. */
  | { type: 'archive'; rows: Row[] | null }
  | { type: 'archive-failed'; problem: Outcome }
  | { type: 'detail-loading'; id: string }
  | { type: 'detail-loaded'; data: DetailData }
  | { type: 'detail-failed'; id: string; problem: Outcome }
  | { type: 'detail-closed' }
  /** Settings read back at boot (already cleaned by data/memory.ts); what is left out keeps its value. */
  | { type: 'restore'; settings: Partial<Remembered> }
  /** The header's projects (data/projects.ts headerProjects). */
  | { type: 'projects'; projects: ProjectEntry[] }
  /** The zone picked (`SPC o t`); null goes back to the TUI's, else the browser's. */
  | { type: 'zone'; zone: string | null }
  /** The theme button (data/theme.ts). */
  | { type: 'theme'; theme: Theme }

/** Where the detail actually stands at this panel width: a narrow panel forces the bottom. */
export const detailPosition = (detail: DetailSettings, width: number): DetailPosition => effectivePosition(width, detail.position)

/**
 * After a reload the cursor stays on the same id. When that row is gone it moves to the nearest row that
 * is still there — the next one in the old order, else the previous one. A cursor that was never a row
 * (`known` holds the ids read before: the list's `..` line, a bucket) is not a row's to lose and stays.
 */
export function keepCursor(cursor: string | null, order: readonly string[], present: ReadonlySet<string>, known?: ReadonlySet<string>): string | null {
  if (cursor === null || present.has(cursor)) return cursor
  if (known && !known.has(cursor)) return cursor
  const at = order.indexOf(cursor)
  if (at < 0) return null
  for (let distance = 1; distance < order.length; distance++) {
    const after = order[at + distance]
    if (after !== undefined && present.has(after)) return after
    const before = order[at - distance]
    if (before !== undefined && present.has(before)) return before
  }
  return null
}

export function reduce(state: State, action: Action): State {
  switch (action.type) {
    case 'tab':
      return state.tab === action.tab ? state : { ...state, tab: action.tab }
    case 'enter':
      return { ...state, path: [...action.path], cursor: action.cursor === undefined ? state.cursor : action.cursor }
    case 'cursor':
      return state.cursor === action.id ? state : { ...state, cursor: action.id }
    case 'focus':
      return state.focus === action.focus ? state : { ...state, focus: action.focus }
    case 'toggle-expanded': {
      const key = pathKey(action.path)
      const expanded = state.expanded.includes(key) ? state.expanded.filter((k) => k !== key) : [...state.expanded, key]
      return { ...state, expanded }
    }
    case 'expanded': {
      const same = action.expanded.length === state.expanded.length && action.expanded.every((key, n) => key === state.expanded[n])
      return same ? state : { ...state, expanded: [...action.expanded] }
    }
    case 'detail-visible':
      return { ...state, detail: { ...state.detail, visible: action.visible }, focus: action.visible ? state.focus : 'list' }
    case 'detail-position':
      return { ...state, detail: { ...state.detail, position: action.position } }
    case 'detail-ratio': {
      const position = action.position ?? state.detail.position
      const ratio = { ...state.detail.ratio, [position]: normalizeRatio(action.ratio, position) }
      return { ...state, detail: { ...state.detail, ratio } }
    }
    case 'view':
      return { ...state, view: { ...state.view, ...action.view } }
    case 'column':
      return {
        ...state,
        columns: { ...state.columns, [action.column]: action.on },
        columnsKnown: state.columnsKnown.includes(action.column) ? state.columnsKnown : [...state.columnsKnown, action.column],
      }
    case 'sort':
      return { ...state, sort: action.sort.fields.length ? copySort(action.sort) : copySort(state.defaultSort) }
    case 'sort-default':
      return { ...state, defaultSort: action.sort?.fields.length ? copySort(action.sort) : copySort(DEFAULT_SORT) }
    case 'search':
      if (action.text === state.search.text) return state
      return { ...state, search: { text: action.text, open: state.search.open || action.text !== '' } }
    case 'filter':
      return withToolbar(state, toolbarReduce({ ...toolbarOf(state), filterText: action.text, cursor: action.text.length }, { type: 'filter-apply' }))
    case 'toolbar':
      return withToolbar(state, toolbarReduce(toolbarOf(state), action.action))
    case 'wiki':
      return { ...state, wiki: action.wiki }
    case 'board-loading':
      return { ...state, board: { ...state.board, loading: true } }
    case 'board-loaded': {
      const present = new Set(action.data.rows.map((row) => row.id))
      const before = state.board.value?.rows.map((row) => row.id) ?? []
      return {
        ...state,
        board: { value: action.data, loading: false, problem: null },
        cursor: keepCursor(state.cursor, action.order ?? before, present, new Set(before)),
      }
    }
    case 'board-failed':
      // The rows read before stay on screen; the problem is shown beside them.
      return { ...state, board: { ...state.board, loading: false, problem: action.problem } }
    case 'groups': {
      // Only the counts change: the rows, and so the cursor and the drawn order, stay.
      if (state.board.value !== action.data) return state
      const { groups: _drawn, ...uncounted } = action.data
      return { ...state, board: { ...state.board, value: action.groups ? { ...uncounted, groups: action.groups } : uncounted } }
    }
    case 'boot':
      return { ...state, boot: action.boot }
    case 'match-loading':
      // The ids stay until the answer; a refusal answers its own question only, so a new question takes it away (the
      // same one asked again for rows read since keeps it, or it would blink on every reload).
      return { ...state, matched: { ...state.matched, key: action.key, loading: true, problem: action.key === state.matched.key ? state.matched.problem : null } }
    case 'matched':
      // An answer to a question no longer on screen is dropped: the one asked since answers for itself.
      if (action.key !== state.matched.key) return state
      return { ...state, matched: { key: action.key, ids: action.ids, loading: false, problem: null } }
    case 'match-failed':
      if (action.key !== state.matched.key) return state
      return { ...state, matched: { ...state.matched, loading: false, problem: action.problem } }
    case 'archive':
      return state.archive === action.rows && state.archiveProblem === null ? state : { ...state, archive: action.rows, archiveProblem: null }
    case 'archive-failed':
      return { ...state, archiveProblem: action.problem }
    case 'detail-loading':
      return { ...state, shown: { value: state.shown.value?.id === action.id ? state.shown.value : null, loading: true, problem: null } }
    case 'detail-loaded':
      return { ...state, shown: { value: action.data, loading: false, problem: null } }
    case 'detail-failed':
      return { ...state, shown: { value: null, loading: false, problem: action.problem } }
    case 'detail-closed':
      return { ...state, shown: idle(), focus: 'list' }
    case 'projects':
      return { ...state, projects: action.projects }
    case 'zone':
      return state.zone === action.zone ? state : { ...state, zone: action.zone }
    case 'theme':
      return state.theme === action.theme ? state : { ...state, theme: action.theme }
    case 'restore': {
      const { tab, detail, zone, theme } = action.settings
      if (tab === undefined && detail === undefined && zone === undefined && theme === undefined) return state
      const ratio = detail ? { left: normalizeRatio(detail.ratio.left, 'left'), bottom: normalizeRatio(detail.ratio.bottom, 'bottom') } : state.detail.ratio
      return {
        ...state,
        tab: tab ?? state.tab,
        detail: detail ? { visible: detail.visible, position: detail.position, ratio } : state.detail,
        focus: detail && !detail.visible ? 'list' : state.focus,
        zone: zone === undefined ? state.zone : zone,
        theme: theme ?? state.theme,
      }
    }
  }
}

export interface Store {
  getState(): State
  dispatch(action: Action): void
  subscribe(listener: (state: State, previous: State) => void): () => void
}

export function createStore(initial: State = initialState()): Store {
  let state = initial
  const listeners = new Set<(state: State, previous: State) => void>()
  return {
    getState: () => state,
    dispatch(action) {
      const previous = state
      state = reduce(state, action)
      if (state === previous) return
      for (const listener of [...listeners]) listener(state, previous)
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}
