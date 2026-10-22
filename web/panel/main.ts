/**
 * Boots the page `moai web` serves: reads `/api/boot` (the language, the columns), restores the remembered
 * settings, wires the store to the data layer and mounts the shell (view/shell.ts), which renders from the store.
 * Built by build.mjs into src/web/dist/app.js.
 *
 * The page reads one project: its address's `?project=<path>` (a registered one, moai-o14f), else the server's own
 * tracker. Every call names it (data/client.ts withProject); another project is another page (data/projects.ts).
 */
import { createClient, PANEL_CLIENT_OPTIONS, PRIORITY, projectOf, requestKey, withProject, type Fetch } from './data/client.js'
import { IssueIndex, type Row } from './data/index.js'
import { createMemory, remember } from './data/memory.js'
import { useNaming } from './data/people.js'
import { headerProjects, projectHref, type ProjectEntry } from './data/projects.js'
import { isUnread, readMarks } from './data/read.js'
import { createRefresher } from './data/refresh.js'
import { sortQuery, useVocabulary } from './data/query.js'
import {
  loadAllRows, loadArchived, loadBoot, loadDetail, loadGroups, loadMatches, loadProjects, loadTree, loadWikiListing, loadWikiPage, matchEndpoints, sameGroups, withArchive,
  type Boot, type Loaded,
} from './data/source.js'
import { COPY, copyFor } from './i18n/index.js'
import { wireKeys } from './keys/wiring.js'
import { createStore, matchKey, type BoardData, type State } from './store.js'
import { listLines, narrow } from './view/list-model.js'
import { PANES } from './view/panes.js'
import { describe, mountShell, type Activity } from './view/shell.js'
import type { WikiSource } from './view/types.js'

/** The browser's fetch, naming no project: the home's boot is read through it. */
const homeFetcher: Fetch = (url, init) => window.fetch(url, init)
/** The project this page reads (`?project=`), or null for the server's own tracker. */
const project = projectOf(window.location.search)
/** Every call the page makes — reads, read marks, the boot, the stamp — names that project. */
const fetcher: Fetch = withProject(homeFetcher, project)

/** How long `/api/stamp` may take before the server counts as out of reach. */
const STAMP_LIMIT_MS = 5_000

async function start(): Promise<void> {
  // Nothing is drawn before the language is known. A boot that fails still mounts the page, in the browser's
  // language, and says why where the rows would stand; the next read asks the boot again (loadBoard).
  let booted = await loadBoot(fetcher)
  const language = booted.ok ? booted.value.lang : navigator.language || 'en'
  // The page says the language it is drawn in: one it does not carry (es) is drawn, and named, in English.
  document.documentElement.lang = Object.hasOwn(COPY, language.split('-')[0]!.toLowerCase()) ? language : 'en'
  const copy = copyFor(language)
  const store = createStore()
  // The last tab and the detail's place come back before anything is drawn. localStorage when the browser lets the
  // page use it, else this session only (data/memory.ts); reading `window.localStorage` itself can throw, hence the getter.
  remember(store, createMemory(() => window.localStorage))
  /** What the boot gives the page: the words, how a person is named, the columns. The language stays the one drawn. */
  function useBoot(boot: Boot): void {
    useVocabulary(boot)
    useNaming(boot.naming)
    document.title = `moai — ${boot.project}`
    store.dispatch({ type: 'boot', boot })
  }
  if (booted.ok) useBoot(booted.value)
  // Up to 3 reads at once (data/client.ts PANEL_CLIENT_OPTIONS).
  const client = createClient(fetcher, PANEL_CLIENT_OPTIONS)

  /**
   * The home: the server's own tracker. Its root is the boot's when the page reads it, else the boot asked without the
   * project (once). `null` when that boot could not be read: the header then lists the registered projects alone.
   */
  async function homeOf(): Promise<{ root: string; name: string } | null> {
    if (!project) return booted.ok ? { root: booted.value.root, name: booted.value.project } : null
    const home = await loadBoot(homeFetcher)
    return home.ok ? { root: home.value.root, name: home.value.project } : null
  }
  let homeRoot: string | null = null
  /**
   * The registered projects, read once after the boot (`/api/projects`): the header's column and the digits. Nothing
   * registered, or a read that failed, leaves both out and nothing else changes.
   */
  async function readProjects(): Promise<void> {
    const [listed, home] = await Promise.all([loadProjects(client), homeOf()])
    homeRoot = home?.root ?? null
    if (!listed.ok || listed.value.length === 0) return
    store.dispatch({ type: 'projects', projects: headerProjects(listed.value, home, project) })
  }
  /** Another project is another page: `/` for the home, `/?project=<path>` for a registered one. */
  function openProject(entry: ProjectEntry): void {
    if (entry.current) return
    window.location.assign(projectHref(entry.path, homeRoot ?? (entry.home ? entry.path : null)))
  }

  // The index is built once per read, not per render: again when the rows, the tree or the archive shown change.
  let indexed: { rows: Row[]; tree: BoardData['tree']; archive: Row[] | null; index: IssueIndex } | null = null
  function indexOf(state: State): IssueIndex | null {
    const board = state.board.value
    if (!board) return null
    const archive = state.view.showArchived ? state.archive : null
    if (indexed?.rows !== board.rows || indexed.tree !== board.tree || indexed.archive !== archive) {
      indexed = { rows: board.rows, tree: board.tree, archive, index: new IssueIndex(withArchive(board, archive), board.tree) }
    }
    return indexed.index
  }

  /** The ids in the order the list draws them now (its hiding, filter, search, order and unfolded folders), so a reload can keep the cursor near where it was. */
  function drawnOrder(): string[] {
    const state = store.getState()
    const index = indexOf(state)
    if (!index) return []
    const lines = listLines(state, index, narrow(state, index).keep)
    return lines.flatMap((line) => (line.kind === 'row' ? [line.key] : []))
  }

  /** Loads so far: only the latest one's answers count. */
  let loads = 0
  /** The load the clock started on its own, or 0: its reads run without the activity line. */
  let quietLoad = 0

  /**
   * Reads the board. False when the read did not take (a refusal, a cancel, the boot still missing): data/refresh.ts
   * then leaves the stamp unseen and reads again on its next look.
   */
  async function loadBoard(automatic = false): Promise<boolean> {
    const load = ++loads
    quietLoad = automatic ? load : 0
    if (!booted.ok) {
      // The boot failed when the page opened (the server out of reach, or refusing then): ask it again, or the page
      // would say so for good while the server answers everything else.
      const again = await loadBoot(fetcher)
      if (load !== loads) return true
      if (!again.ok) {
        store.dispatch({ type: 'board-failed', problem: again.outcome })
        return false
      }
      booted = again
      useBoot(again.value)
    }
    store.dispatch({ type: 'board-loading' })
    // The rows, where they stand (`/api/tree`, in the order on screen) and the milestone and epic counts `moai
    // status` prints, side by side. The load does not wait for the counts: rows are drawn at once, and the counts
    // are put on them when they come.
    // Each load is a change seen through: a read of it is never handed a run another load started before it.
    const { sort, view } = store.getState()
    // `SPC v w` (on by default): the other worktrees' rows over the list, as the TUI's explorer opens.
    const listing = loadAllRows(client, PRIORITY.list, load, view.worktree)
    const placing = loadTree(client, sortQuery(sort), PRIORITY.list, load)
    const counting = loadGroups(client, PRIORITY.list, load)
    let counted: Awaited<typeof counting> | undefined
    void counting.then((groups) => (counted = groups))
    const [loaded, placed] = await Promise.all([listing, placing])
    if (load !== loads) return true
    if (!loaded.ok || !placed.ok) {
      store.dispatch({ type: 'board-failed', problem: loaded.ok ? (placed as Extract<typeof placed, { ok: false }>).outcome : loaded.outcome })
      return false
    }
    const { rows, notes } = loaded.value
    const before = store.getState()
    // Counts that came first go in with the rows, so the board is not drawn twice. Until this read's counts come, a
    // refresh keeps the counts the board showed.
    const carried = counted === undefined ? before.board.value?.groups : undefined
    const counts = counted?.ok ? counted.value : carried
    const data: BoardData = { rows, tree: placed.value, loadedAt: Date.now(), ...(notes ? { notes } : {}), ...(counts ? { groups: counts } : {}) }
    const shownBefore = before.shown.value
    // The detail on screen was read before this list and stands beside it: when its row has left the list
    // (finished, moved away) the pane goes with it, as choose() does for a read still running. A detail the list
    // never held (an issue a wiki page links to that only the archive holds) stays.
    const wasListed = shownBefore !== null && indexOf(before)?.find(shownBefore.id) !== undefined
    store.dispatch({ type: 'board-loaded', data, order: drawnOrder() })
    const state = store.getState()
    const shown = state.shown.value
    if (shown && wasListed && indexOf(state)?.find(shown.id) === undefined) store.dispatch({ type: 'detail-closed' })
    // The tracker changed: the detail on screen is read again too, in place (its pane, scroll and folds stay).
    else if (shown && automatic) void rereadDetail(shown.id, load)
    // Archived rows shown but not read yet: turned on before the rows were there, or the last read failed.
    if (state.view.showArchived && archiveRunning === null && state.archive === null) void loadArchive()
    // What the filter and the search find moves with the rows: asked again for the rows just read.
    void ask(true)
    // Counts that come later go on this board only: a later load's board has its own (store 'groups'). Counts that
    // match the ones carried (most refreshes) change nothing on screen, so the board is not drawn again for them.
    if (counted === undefined) {
      void counting.then((late) => {
        if (late.ok ? !sameGroups(late.value, carried) : carried) store.dispatch({ type: 'groups', data, groups: late.ok ? late.value : null })
      })
    }
    // The order changed while this read ran: the refresh it asked for joined this one (data/refresh.ts), which read
    // the tree in the order before. Read it again in the order on screen.
    // So does the worktree overlay turned while it ran.
    const after = store.getState()
    if (after.sort !== sort || after.view.worktree !== view.worktree) setTimeout(() => void refresher.refresh(), 0)
    return true
  }

  let asking: ReturnType<typeof setTimeout> | undefined
  /** Questions asked so far: only the latest one's answer counts, even when an older one asked the same. */
  let asks = 0
  /** The load the answer on screen was asked after: a question asked again once the rows were read since is asked anew. */
  let askedAt = -1
  /** The reads of the question in flight, cancelled when a newer one is asked: its answer would be dropped. */
  let askingKeys: string[] = []
  /**
   * Asks moai what the filter and the search on screen find (`loadMatches`), unless that is the question already
   * answered for the rows on screen — `again` asks it anyway, for rows read since. A search typed fast asks once it
   * rests. A question emptied and put back after a reload is not the one answered before it.
   */
  async function ask(again = false): Promise<void> {
    const state = store.getState()
    const key = matchKey(state)
    if (key === '' || (!again && key === state.matched.key && askedAt === loads)) return
    const mine = ++asks
    askedAt = loads
    const question = { filter: state.filter.applied, q: state.search.text, archived: state.view.showArchived }
    const next = matchEndpoints(question).map(requestKey)
    // The older question's reads this one does not share: their answer would be dropped, and they hold the slots.
    for (const old of askingKeys) if (!next.includes(old)) client.cancel(old)
    askingKeys = next
    store.dispatch({ type: 'match-loading', key })
    // Asked for the rows of this load: never handed a run that began before them (data/client.ts epoch).
    const loaded = await loadMatches(client, question, loads)
    if (mine !== asks) return
    askingKeys = []
    if (loaded.ok) store.dispatch({ type: 'matched', key, ids: loaded.value })
    else store.dispatch({ type: 'match-failed', key, problem: loaded.outcome })
  }
  store.subscribe((state, previous) => {
    if (matchKey(state) !== matchKey(previous)) {
      clearTimeout(asking)
      // The search applies as it is typed: a pause of 150 ms is one question, not one per key.
      if (state.search.text !== previous.search.text) asking = setTimeout(() => void ask(), 150)
      else void ask()
    }
    // A new order is moai's to make: the tree is read again in it.
    if (state.sort !== previous.sort && state.board.value) void refresher.refresh()
    // The worktree overlay turned on or off: the rows are read again with it or without it.
    if (state.view.worktree !== previous.view.worktree && state.board.value) void refresher.refresh()
  })

  // The archive is read when archived rows are shown (`SPC v o`), each time they are turned on, once the active
  // rows are there: a row they do not hold is archive-only, tagged `archived: true` (never NEW).
  let archiving = 0
  /** The `archiving` number of the archive read still running, or null when none is. */
  let archiveRunning: number | null = null
  async function loadArchive(): Promise<void> {
    if (!store.getState().board.value) return
    const mine = ++archiving
    archiveRunning = mine
    try {
      const loaded = await loadArchived(client)
      if (mine !== archiving) return
      // The archive's own problem: the board's rows and whatever the board said stand.
      if (loaded.ok) store.dispatch({ type: 'archive', rows: loaded.value })
      else store.dispatch({ type: 'archive-failed', problem: loaded.outcome })
    } finally {
      if (archiveRunning === mine) archiveRunning = null
    }
  }
  store.subscribe((state, previous) => {
    if (state.view.showArchived === previous.view.showArchived) return
    if (state.view.showArchived) void loadArchive()
    else {
      // Turned off: a read still running is not wanted, and the next turn-on reads the archive afresh.
      archiving++
      store.dispatch({ type: 'archive', rows: null })
    }
  })

  // `/api/stamp` moves when the tracker's files do: the rows are read only then, and not at all while the page is out
  // of sight (data/refresh.ts). A server that answers with a refusal still answers, so its stamp is a word that never
  // repeats and the read says why.
  let refused = 0
  const stamp = async (): Promise<string | null> => {
    // An answer that never comes is the server out of reach too: a stamp waited on for ever would stop the asking
    // for good, and a stopped process can leave its port taking connections that nobody answers.
    const abort = new AbortController()
    const limit = setTimeout(() => abort.abort(), STAMP_LIMIT_MS)
    try {
      const answer = await fetcher('/api/stamp', { signal: abort.signal, headers: { Accept: 'application/json' } })
      const body = JSON.parse(await answer.text()) as { stamp?: unknown }
      return answer.status === 200 && typeof body.stamp === 'string' ? body.stamp : `refused-${++refused}`
    } catch (error) {
      return error instanceof SyntaxError ? `unreadable-${++refused}` : null
    } finally {
      clearTimeout(limit)
    }
  }
  const refresher = createRefresher({ load: ({ automatic }) => loadBoard(automatic), stamp, onReach: (reached) => shell.offline(!reached) })

  let choice = 0
  async function choose(id: string) {
    const mine = ++choice
    // Only a row the list held can be "removed by a reload" below; an issue the list never held (a wiki page's link
    // to one only the archive holds) is shown when its read answers.
    const held = indexOf(store.getState())?.find(id) !== undefined
    store.dispatch({ type: 'cursor', id })
    store.dispatch({ type: 'detail-loading', id })
    const loaded = await loadDetail(client, id)
    // A later choice wins: its own answer settles the detail.
    if (mine !== choice) return
    const now = store.getState()
    // The pane was closed while the read ran (Close, or a reload that closed it): the answer is not wanted.
    if (!now.shown.loading) return
    // A reload removed the row. Nothing else will answer for the detail, so it closes instead of staying on
    // "Loading…". The cursor moving elsewhere is the person's own (a key press) and does not cancel the read.
    const listed = indexOf(now)
    if (held && listed && listed.find(id) === undefined) return store.dispatch({ type: 'detail-closed' })
    if (loaded.ok) store.dispatch({ type: 'detail-loaded', data: { id, row: loaded.value } })
    else store.dispatch({ type: 'detail-failed', id, problem: loaded.outcome })
  }

  /** Rereads so far: an older one answering after a newer one does not cover it. */
  let rereads = 0
  /** The detail on screen, read again after a change; a choice made meanwhile, a later reread, or the pane closed, wins. */
  async function rereadDetail(id: string, epoch: number): Promise<void> {
    const mine = choice
    const turn = ++rereads
    const loaded = await loadDetail(client, id, { epoch })
    if (!loaded.ok || mine !== choice || turn !== rereads || store.getState().shown.value?.id !== id) return
    store.dispatch({ type: 'detail-loaded', data: { id, row: loaded.value } })
  }

  /**
   * The read-mark writes (`r`, `SPC m a`, `SPC m g`, data/read.ts). One that went through reads the rows again — the
   * marks come back with them — and the detail on screen when it marked that row: nothing else reads the detail
   * again, and its NEW would outlive the mark.
   */
  const read = readMarks(client, (written) => {
    const shown = store.getState().shown.value
    // A list read already running began before the write and cannot hold its marks: `fresh` reads again after it.
    // The detail is asked past every load so far, so it is never handed a run that began before the write.
    return Promise.all([
      refresher.refresh({ fresh: true }),
      shown && isUnread(shown.row) && written.has(shown.id) ? rereadDetail(shown.id, loads + 1) : undefined,
    ])
  })

  /** What an outcome says, as one line of text (the wiki's error state takes a string). */
  function said(loaded: Extract<Loaded<unknown>, { ok: false }>): string {
    const parts = describe(loaded.outcome, copy, document)?.parts ?? [copy.failed]
    return parts.map((part) => (typeof part === 'string' ? part : part.textContent ?? '')).join('')
  }

  /** A wiki read's answer, or an Error with what stopped it (the wiki view shows it). */
  function answer(loaded: Loaded<unknown>): unknown {
    if (loaded.ok) return loaded.value
    throw new Error(said(loaded))
  }

  const wiki: WikiSource = {
    list: async () => answer(await loadWikiListing(client)),
    page: async (slug) => answer(await loadWikiPage(client, slug)),
  }

  function activity(): Activity {
    // The clock's own list reads run quietly: a line every few seconds would cover the rows at the foot of the panes
    // for nothing (se-24ey.a80). A read the person asked for, or one that waits beside it (a detail, the wiki), still shows.
    const quiet = quietLoad !== 0 && quietLoad === loads
    const requests = client.requests().filter((request) => !(quiet && request.priority === PRIORITY.list))
    const running = requests.some((request) => request.phase === 'running')
    return { phase: running ? 'running' : 'idle', cancellable: running }
  }

  const root = document.getElementById('moai-app') ?? document.body
  // The shell draws on every store change; the request queue changes only the activity line and Cancel.
  const shell = mountShell(root, {
    document,
    store,
    copy,
    language,
    panes: PANES,
    index: indexOf,
    activity,
    wiki,
    intents: {
      refresh: () => void refresher.refresh(),
      choose: (id) => void choose(id),
      // Up to three reads go at once (the list is three of them): Cancel stops every one, and the reads that wait
      // behind them, in one step (data/client.ts cancelAll) rather than letting the next queued read start in the freed slot.
      cancel: () => client.cancelAll(),
      openProject,
    },
  })
  client.subscribe(() => shell.activity(activity()))
  // The one keydown listener: keys → the key table's actions → the store and the panes (keys/wiring.ts), with the
  // SPC menu, the shortcut help and the "click to use the keyboard" hint.
  wireKeys({ document, store, shell, copy, read, openProject })

  document.body.tabIndex = -1
  document.body.focus({ preventScroll: true })
  void readProjects()
  await refresher.refresh()
}

void start()
