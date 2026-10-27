/**
 * What the panel remembers between openings (design §5): the last tab, whether the detail shows, where it
 * stands and its share of the split for each position (`ratio.left`, `ratio.bottom`), the time zone picked with
 * `SPC o t` (`zone`, data/zone.ts) and the theme picked with the theme button (`theme`, data/theme.ts; left out for
 * `system`) — how this person reads the page, so it is kept beside the layout, not per project.
 *
 * Where it is kept: the browser's localStorage for the page's origin (`moai web`'s host and port). When it cannot
 * be used — a private window, site data blocked, where touching it throws — the settings stay in memory until the
 * tab is closed. Every access is guarded: a failing store never stops the page, it only forgets.
 *
 * {@link normalizeMemory} is pure and cleans whatever comes back, so a value from an older or newer panel,
 * or one edited by hand, still boots with usable settings.
 */
import { SORT_KEYS, sameSort, type Dir, type Sort, type SortField } from './query.js'
import { COLUMN_DEFAULTS, type ColumnName, type DetailPosition, type Remembered, type State, type Store, type ViewSettings } from '../store.js'
import { DEFAULT_RATIO, normalizeRatio } from '../view/split.js'
import { DEFAULT_TAB, normalizeTab } from '../view/tabs.js'
import { MEMORY_KEY } from './memory-key.js'
import { normalizeTheme } from './theme.js'
import { validZone } from './zone.js'

/** The page's key in localStorage (data/memory-key.ts); a project's own settings go under {@link queryKeyOf}. */
export { MEMORY_KEY }
/** Written with the settings; a later panel that changes their shape reads this to migrate. */
export const MEMORY_VERSION = 1

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null

const normalizePosition = (value: unknown): DetailPosition => (value === 'left' ? 'left' : 'bottom')

/** Stored settings (parsed JSON, or anything else) to usable ones; what is missing or wrong takes its default. */
export function normalizeMemory(value: unknown): Remembered {
  const stored = isRecord(value) ? value : {}
  const detail = isRecord(stored.detail) ? stored.detail : {}
  const ratio = isRecord(detail.ratio) ? detail.ratio : {}
  return {
    tab: stored.tab === undefined ? DEFAULT_TAB : normalizeTab(stored.tab),
    detail: {
      visible: typeof detail.visible === 'boolean' ? detail.visible : true,
      position: normalizePosition(detail.position),
      ratio: {
        left: ratio.left === undefined ? DEFAULT_RATIO.left : normalizeRatio(ratio.left, 'left'),
        bottom: ratio.bottom === undefined ? DEFAULT_RATIO.bottom : normalizeRatio(ratio.bottom, 'bottom'),
      },
    },
    // A zone this browser does not know (dropped since, or edited by hand) is no choice: the page falls back silently.
    // No choice is no key, as the page before the picker wrote it.
    ...(validZone(stored.zone) ? { zone: validZone(stored.zone) } : {}),
    // `system` is no choice and no key; only light or dark was picked.
    ...(normalizeTheme(stored.theme) !== 'system' ? { theme: normalizeTheme(stored.theme) } : {}),
  }
}

/** The part of the state that is remembered. */
export function rememberedOf(state: State): Remembered {
  const { visible, position, ratio } = state.detail
  return { tab: state.tab, detail: { visible, position, ratio: { left: ratio.left, bottom: ratio.bottom } }, ...(state.zone ? { zone: state.zone } : {}), ...(state.theme !== 'system' ? { theme: state.theme } : {}) }
}

export function sameMemory(a: Remembered, b: Remembered): boolean {
  return (
    a.tab === b.tab &&
    a.detail.visible === b.detail.visible &&
    a.detail.position === b.detail.position &&
    a.detail.ratio.left === b.detail.ratio.left &&
    a.detail.ratio.bottom === b.detail.ratio.bottom &&
    (a.zone ?? null) === (b.zone ?? null) &&
    (a.theme ?? 'system') === (b.theme ?? 'system')
  )
}

/** Parse an untrusted storage value without making a broken setting stop the page. */
const parsed = (text: string | null): unknown => {
  try { return text === null ? null : JSON.parse(text) } catch { return null }
}
const record = (value: unknown): Record<string, unknown> => isRecord(value) ? { ...value } : {}
const sameValue = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b)

/** Patch only changed leaves, preserving unknown keys and removing leaves explicitly cleared by this tab. */
function patchLeaves(stored: unknown, before: object | undefined, now: object): Record<string, unknown> {
  const out = record(stored)
  const was = before as Record<string, unknown> | undefined
  const next = now as Record<string, unknown>
  for (const name of new Set([...Object.keys(was ?? {}), ...Object.keys(next)])) {
    if (was && sameValue(was[name], next[name])) continue
    if (next[name] === undefined) delete out[name]
    else out[name] = next[name]
  }
  return out
}

/** A tab saves only the settings it changed over the value read immediately before writing. */
export function patchMemory(stored: unknown, before: Remembered | undefined, now: Remembered): Record<string, unknown> {
  const out = record(stored)
  const detail = record(out.detail)
  out.version = MEMORY_VERSION
  if (!before || before.tab !== now.tab) out.tab = now.tab
  for (const key of ['visible', 'position'] as const) {
    if (!before || before.detail[key] !== now.detail[key]) detail[key] = now.detail[key]
  }
  detail.ratio = patchLeaves(detail.ratio, before?.detail.ratio, now.detail.ratio)
  out.detail = detail
  for (const key of ['zone', 'theme'] as const) {
    const value = (settings: Remembered) => key === 'theme' && settings[key] === 'system' ? undefined : settings[key]
    if (before && value(before) === value(now)) continue
    const next = value(now)
    if (next === undefined) delete out[key]
    else out[key] = next
  }
  return out
}

/** The slice of the Web Storage API used here, so tests pass a plain object. */
export interface Backend {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export interface Memory {
  load(): Remembered
  save(settings: Remembered, before?: Remembered): void
  /** False once the backend could not be used: the settings then last for this session only. */
  readonly persistent: boolean
}

/**
 * Settings kept in `backend()` under `key`. The backend is a getter because reading `window.localStorage`
 * itself is what throws in an opaque origin. Anything that throws drops to an in-memory copy for the session.
 */
export function createMemory(backend: () => Backend | null | undefined, key: string = MEMORY_KEY): Memory {
  let storage: Backend | null = null
  try {
    storage = backend() ?? null
  } catch {
    storage = null
  }
  let session: string | null = null

  const read = (): string | null => {
    if (storage) {
      try {
        return storage.getItem(key)
      } catch {
        storage = null
      }
    }
    return session
  }

  return {
    load() {
      return normalizeMemory(parsed(read()))
    },
    save(settings, before) {
      const text = JSON.stringify(patchMemory(parsed(read()), before, settings))
      session = text
      if (!storage) return
      try {
        storage.setItem(key, text)
      } catch {
        // Full, or blocked after all: keep the session copy and stop trying.
        storage = null
      }
    },
    get persistent() {
      return storage !== null
    },
  }
}

// ── the default sort (E4 se-5air) ──────────────────────────────────────────────────────────────────────
//
// The sort editor's `D` keeps the order on screen as the one `reset sort` and a fresh opening return to (a
// workspace that kept an order of its own opens on that one, as moai tui's project order beats its default).
// It is kept through a {@link SettingsStorage}: the browser's localStorage when the page may use it
// ({@link createBrowserStorage}), else the tab's own memory. Every access is guarded, as {@link createMemory}'s: a
// failing store only forgets.

/** Where the default sort is kept; a persisting backend is stored under this same key. */
export const SORT_DEFAULT_KEY = `${MEMORY_KEY}.sort`

/** A string store keyed by name: the seam a settings-storage API will fill. `null` clears a key. */
export interface SettingsStorage {
  read(key: string): string | null
  write(key: string, value: string | null): void
  /** False when what is written is lost with the session (the only kind there is until settings storage exists). */
  readonly persistent: boolean
}

/** The session-only storage: a copy in memory, gone when the tab closes. */
export function createSessionStorage(): SettingsStorage {
  const kept = new Map<string, string>()
  return {
    read: (key) => kept.get(key) ?? null,
    write(key, value) {
      if (value === null) kept.delete(key)
      else kept.set(key, value)
    },
    persistent: false,
  }
}

/**
 * localStorage when it can be used — reading `window.localStorage` itself can throw (site data blocked), and a
 * private window may refuse a write — else {@link createSessionStorage}. It is tried once, when made.
 */
export function createBrowserStorage(backend: () => (Backend & { removeItem(key: string): void }) | null | undefined): SettingsStorage {
  let store: (Backend & { removeItem(key: string): void }) | null = null
  try {
    const found = backend()
    if (found) {
      const probe = `${MEMORY_KEY}.probe`
      found.setItem(probe, '1')
      found.removeItem(probe)
      store = found
    }
  } catch {
    store = null
  }
  if (!store) return createSessionStorage()
  const kept = store
  return {
    read: (key) => kept.getItem(key),
    write(key, value) {
      if (value === null) kept.removeItem(key)
      else kept.setItem(key, value)
    },
    persistent: true,
  }
}

const isDir = (value: unknown): value is Dir => value === 'asc' || value === 'desc'

/** A stored order back to a {@link Sort}, or null when it is not a usable one (empty, unknown key, repeated key). */
export function normalizeSort(value: unknown): Sort | null {
  if (!isRecord(value) || !Array.isArray(value.fields) || value.fields.length === 0) return null
  const fields: SortField[] = []
  for (const one of value.fields) {
    if (!isRecord(one) || !isDir(one.dir)) return null
    const key = SORT_KEYS.find((candidate) => candidate === one.key)
    if (!key || fields.some((field) => field.key === key)) return null
    fields.push({ key, dir: one.dir })
  }
  return { fields, reversed: value.reversed === true }
}

export interface SortDefault {
  /** The saved default, or null when none was saved. */
  get(): Sort | null
  set(sort: Sort): void
  clear(): void
  /** False while the default lasts for this session only. */
  readonly persistent: boolean
}

export function createSortDefault(storage: SettingsStorage = createSessionStorage(), key: string = SORT_DEFAULT_KEY): SortDefault {
  const write = (value: string | null) => {
    try {
      storage.write(key, value)
    } catch {
      // Full, or refused: the default is forgotten, the panel goes on.
    }
  }
  return {
    get() {
      try {
        const text = storage.read(key)
        return text === null ? null : normalizeSort(JSON.parse(text))
      } catch {
        return null
      }
    },
    set(sort) {
      write(JSON.stringify({ fields: sort.fields, reversed: sort.reversed }))
    },
    clear() {
      write(null)
    },
    get persistent() {
      return storage.persistent
    },
  }
}

/**
 * The page's one settings storage and one default sort, which {@link remember} (at boot, into the store's
 * `defaultSort`) and the key wiring (`D`) fall back to so they meet the same default: the browser's localStorage,
 * else the tab's memory. Callers other than the page itself (tests) pass their own, so nothing they write reaches the
 * next boot.
 */
export const settingsStorage: SettingsStorage = createBrowserStorage(() => (typeof window === 'undefined' ? null : window.localStorage))
export const sortDefault: SortDefault = createSortDefault(settingsStorage)

// ── the query settings (E4 se-cv7w.lca) ────────────────────────────────────────────────────────────────
//
// What the toolbar and the view menu set — filter, sort, list cells, hidden statuses — is the
// workspace's: it goes under {@link QUERY_KEY} in the same settings storage (design §5, scope "workspace"),
// while tab and layout (`MEMORY_KEY`) are global. The search text is not kept: a reopened panel does not
// start inside yesterday's search.

export const QUERY_KEY = `${MEMORY_KEY}.query`

/**
 * The query settings' key for the project the page reads (`?project=`, data/client.ts projectOf). One server reads
 * every registered project from the same origin, so a key of its own keeps one project's filter, order, cells and
 * hidden columns off the next one's page: a filter on an epic B does not have, or a column B's config lacks, emptied
 * B's list or had it refused. The home keeps the key it always had.
 */
export const queryKeyOf = (project: string | null): string => (project ? `${QUERY_KEY}:${project}` : QUERY_KEY)

export interface QuerySettings {
  /** The applied filter text. */
  filter: string
  /** The workspace's own order; null while it is the default one, which the workspace then follows. */
  sort: Sort | null
  columns: Partial<Record<ColumnName, boolean>>
  view: Partial<ViewSettings>
}

const COLUMN_NAMES = Object.keys(COLUMN_DEFAULTS) as ColumnName[]

/**
 * The query settings of `state`, read to be written as JSON (the store's own objects, not copies). An order that is
 * the state's `defaultSort` is not kept: toggling a cell does not pin the workspace to the order the default had then,
 * so a later `D` still decides its next opening (moai tui keeps a project's order only once one was chosen there).
 */
export function queryOf(state: State): QuerySettings {
  return {
    filter: state.filter.applied,
    sort: sameSort(state.sort, state.defaultSort) ? null : state.sort,
    columns: state.columns,
    view: state.view,
  }
}

/** Query leaves are independent too: the order is one leaf, each column and view setting its own. */
export function patchQuery(stored: unknown, before: QuerySettings, now: QuerySettings): Record<string, unknown> {
  const out = record(stored)
  if (before.filter !== now.filter) out.filter = now.filter
  if (!sameValue(before.sort, now.sort)) out.sort = now.sort
  out.columns = patchLeaves(out.columns, before.columns, now.columns)
  out.view = patchLeaves(out.view, before.view, now.view)
  return out
}

/** The slices {@link queryOf} reads are the ones `previous` had: the reducer keeps an unchanged slice's object. */
const sameQuerySlices = (a: State, b: State): boolean =>
  a.filter.applied === b.filter.applied && a.sort === b.sort && a.defaultSort === b.defaultSort && a.columns === b.columns && a.view === b.view

/** Stored query settings (parsed JSON, or anything else) to what can be applied; what is wrong is left out. */
export function normalizeQuery(value: unknown): Partial<QuerySettings> {
  if (!isRecord(value)) return {}
  const out: Partial<QuerySettings> = {}
  if (typeof value.filter === 'string') out.filter = value.filter
  const sort = normalizeSort(value.sort)
  if (sort) out.sort = sort
  if (isRecord(value.columns)) {
    const columns: Partial<Record<ColumnName, boolean>> = {}
    for (const name of COLUMN_NAMES) if (typeof value.columns[name] === 'boolean') columns[name] = value.columns[name] as boolean
    out.columns = columns
  }
  if (isRecord(value.view)) {
    const view: Partial<ViewSettings> = {}
    const { hiddenStatuses, hideDeferred, hideBacklog, showArchived } = value.view
    if (Array.isArray(hiddenStatuses)) view.hiddenStatuses = hiddenStatuses.filter((one): one is string => typeof one === 'string')
    if (typeof hideDeferred === 'boolean') view.hideDeferred = hideDeferred
    if (typeof hideBacklog === 'boolean') view.hideBacklog = hideBacklog
    if (typeof showArchived === 'boolean') view.showArchived = showArchived
    out.view = view
  }
  return out
}

/** Puts stored query settings into the store through the actions the toolbar and the view menu use. */
function applyQuery(store: Store, query: Partial<QuerySettings>): void {
  if (query.filter !== undefined) store.dispatch({ type: 'filter', text: query.filter })
  if (query.sort) store.dispatch({ type: 'sort', sort: query.sort })
  for (const name of COLUMN_NAMES) {
    const on = query.columns?.[name]
    if (on !== undefined) store.dispatch({ type: 'column', column: name, on })
  }
  if (query.view) store.dispatch({ type: 'view', view: query.view })
}

/**
 * Restores the remembered settings into the store, then saves them whenever they change. Returns the
 * unsubscribe. Call it before the views mount, so they first draw the restored tab and layout. The default
 * sort is applied first, so the workspace's own order in the query settings wins over it. Left out, `defaults` and
 * `storage` are the panel's shared {@link sortDefault} and {@link settingsStorage}, the ones the key wiring falls back to,
 * and `queryKey` the home's ({@link queryKeyOf}).
 */
export function remember(
  store: Store,
  memory: Memory,
  defaults: SortDefault = sortDefault,
  storage: SettingsStorage = settingsStorage,
  queryKey: string = QUERY_KEY,
): () => void {
  store.dispatch({ type: 'restore', settings: memory.load() })
  const saved = defaults.get()
  store.dispatch({ type: 'sort-default', sort: saved })
  if (saved) store.dispatch({ type: 'sort', sort: saved })
  try {
    const text = storage.read(queryKey)
    if (text !== null) applyQuery(store, normalizeQuery(JSON.parse(text)))
  } catch {
    // Unreadable: the panel starts from its defaults.
  }
  return store.subscribe((state, previous) => {
    const now = rememberedOf(state)
    const before = rememberedOf(previous)
    // Another tab's choice this one follows is that tab's write already: written back, an event that arrives late puts
    // the older choice over a newer one.
    if (!sameMemory(now, before) && !following.has(store)) memory.save(now, before)
    // Most changes (the cursor, a read) leave the query alone: nothing is built or compared for them.
    if (sameQuerySlices(state, previous)) return
    const query = queryOf(state)
    const was = queryOf(previous)
    if (sameValue(query, was)) return
    try {
      storage.write(queryKey, JSON.stringify(patchQuery(parsed(storage.read(queryKey)), was, query)))
    } catch {
      // Full, or refused: forgotten. A throw here would stop the listeners after this one (the views) drawing.
    }
  })
}

/** Stores taking another tab's choice ({@link follow}) right now: {@link remember} does not save what they take. */
const following = new WeakSet<Store>()

/** Other tabs' theme and zone choices apply here; this tab keeps its own pane, layout and query. */
export function follow(store: Store, win: Window, key: string = MEMORY_KEY): () => void {
  const changed = (event: StorageEvent) => {
    if (event.key !== key) return
    const settings = normalizeMemory(parsed(event.newValue))
    following.add(store)
    try {
      store.dispatch({ type: 'theme', theme: settings.theme ?? 'system' })
      store.dispatch({ type: 'zone', zone: settings.zone ?? null })
    } finally {
      following.delete(store)
    }
  }
  win.addEventListener('storage', changed)
  return () => win.removeEventListener('storage', changed)
}
