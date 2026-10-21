/**
 * Boots the page `moai web` serves: reads `/api/boot` (the language, the columns), restores the remembered
 * settings, wires the store to the data layer and mounts the shell (view/shell.ts), which renders from the store.
 * Built by build.mjs into src/web/dist/app.js.
 */
import { createClient, PANEL_CLIENT_OPTIONS, PRIORITY, type Fetch } from './data/client.js'
import { IssueIndex, type Row } from './data/index.js'
import { createMemory, remember } from './data/memory.js'
import { createRefresher } from './data/refresh.js'
import { loadAllRows, loadArchived, loadBoot, loadDetail, loadTallies, loadWikiListing, loadWikiPage, sameTallies, withArchive, type Loaded } from './data/source.js'
import { copyFor } from './i18n/index.js'
import { wireKeys } from './keys/wiring.js'
import { createStore, type BoardData, type State } from './store.js'
import { listLines, narrow, sortOrder } from './view/list-model.js'
import { PANES } from './view/panes.js'
import { describe, mountShell, type Activity } from './view/shell.js'
import type { WikiSource } from './view/types.js'

const fetcher: Fetch = (url, init) => window.fetch(url, init)

async function start(): Promise<void> {
  // Nothing is drawn before the language is known. A boot that fails still mounts the page, in the browser's
  // language, and says why where the rows would stand.
  const booted = await loadBoot(fetcher)
  const language = booted.ok ? booted.value.lang : navigator.language || 'en'
  document.documentElement.lang = language
  const copy = copyFor(language)
  const store = createStore()
  // The last tab and the detail's place come back before anything is drawn. localStorage when the browser lets the
  // page use it, else this session only (data/memory.ts); reading `window.localStorage` itself can throw, hence the getter.
  remember(store, createMemory(() => window.localStorage))
  // Up to 3 reads at once (data/client.ts PANEL_CLIENT_OPTIONS).
  const client = createClient(fetcher, PANEL_CLIENT_OPTIONS)

  // The index is built once per read, not per render: again when the rows, the status counts or the archive shown change.
  let indexed: { rows: Row[]; tallies: BoardData['tallies']; archive: Row[] | null; index: IssueIndex } | null = null
  function indexOf(state: State): IssueIndex | null {
    const board = state.board.value
    if (!board) return null
    const archive = state.view.showArchived ? state.archive : null
    if (indexed?.rows !== board.rows || indexed.tallies !== board.tallies || indexed.archive !== archive) {
      indexed = { rows: board.rows, tallies: board.tallies, archive, index: new IssueIndex(withArchive(board, archive), board.tallies) }
    }
    return indexed.index
  }

  /** The ids in the order the list draws them now (its hiding, filter, search, order and unfolded folders), so a reload can keep the cursor near where it was. */
  function drawnOrder(): string[] {
    const state = store.getState()
    const index = indexOf(state)
    if (!index) return []
    const lines = listLines(state, index, narrow(state, index).keep, sortOrder(state.sort, index))
    return lines.flatMap((line) => (line.kind === 'row' ? [line.key] : []))
  }

  /** Loads so far: only the latest one's answers count. */
  let loads = 0
  /** The load the clock started on its own, or 0: its reads run without the activity line. */
  let quietLoad = 0

  async function loadBoard(automatic = false): Promise<void> {
    const load = ++loads
    quietLoad = automatic ? load : 0
    if (!booted.ok) {
      store.dispatch({ type: 'board-failed', problem: booted.outcome })
      return
    }
    store.dispatch({ type: 'board-loading' })
    // The rows and the milestone and epic counts `moai status` prints (they count archived rows too) side by side.
    // Without the counts the board counts its own rows. The load does not wait for the counts: rows are drawn at
    // once, and the counts are put on them when they come.
    const listing = loadAllRows(client, PRIORITY.list)
    const counting = loadTallies(client, PRIORITY.list)
    let counted: Awaited<typeof counting> | undefined
    void counting.then((tallies) => (counted = tallies))
    const loaded = await listing
    if (load !== loads) return
    if (!loaded.ok) {
      store.dispatch({ type: 'board-failed', problem: loaded.outcome })
      return
    }
    const { rows, notes } = loaded.value
    const before = store.getState()
    // Counts that came first go in with the rows, so the board is not drawn twice. Until this read's counts come, a
    // refresh keeps the counts the board showed: its rows alone undercount (they leave the archived members out).
    const carried = counted === undefined ? before.board.value?.tallies : undefined
    const counts = counted?.ok ? counted.value : carried
    const data: BoardData = { rows, loadedAt: Date.now(), ...(notes ? { notes } : {}), ...(counts ? { tallies: counts } : {}) }
    const shownBefore = before.shown.value
    // The detail on screen was read before this list and stands beside it: when its row has left the list
    // (finished, moved away) the pane goes with it, as choose() does for a read still running. A detail the list
    // never held (an issue a wiki page links to that only the archive holds) stays.
    const wasListed = shownBefore !== null && indexOf(before)?.find(shownBefore.id) !== undefined
    store.dispatch({ type: 'board-loaded', data, order: drawnOrder() })
    const state = store.getState()
    const shown = state.shown.value
    if (shown && wasListed && indexOf(state)?.find(shown.id) === undefined) store.dispatch({ type: 'detail-closed' })
    // Archived rows shown but not read yet: turned on before the rows were there, or the last read failed.
    if (state.view.showArchived && archiveRunning === null && state.archive === null) void loadArchive()
    // Counts that come later go on this board only: a later load's board has its own (store 'tallies'). Counts that
    // match the ones carried (most refreshes) change nothing on screen, so the board is not drawn again for them;
    // counts that could not be read leave the board counting its own rows.
    if (counted === undefined) {
      void counting.then((late) => {
        if (late.ok ? !sameTallies(late.value, carried) : carried) store.dispatch({ type: 'tallies', data, tallies: late.ok ? late.value : null })
      })
    }
  }

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

  // Paused while the page is out of sight (data/refresh.ts).
  const refresher = createRefresher({ load: ({ automatic }) => loadBoard(automatic) })

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
      // behind them, rather than letting the next queued read start in the freed slot.
      cancel: () => {
        for (const key of new Set(client.requests().map((request) => request.key))) client.cancel(key)
      },
    },
  })
  client.subscribe(() => shell.activity(activity()))
  // The one keydown listener: keys → the key table's actions → the store and the panes (keys/wiring.ts), with the
  // SPC menu, the shortcut help and the "click to use the keyboard" hint.
  wireKeys({ document, store, shell, copy })

  document.body.tabIndex = -1
  document.body.focus({ preventScroll: true })
  await refresher.refresh()
}

void start()
