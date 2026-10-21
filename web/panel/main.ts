/**
 * Boots the board panel: waits for the host bridge, restores the remembered settings, wires the store to the
 * data layer and mounts the shell (view/shell.ts), which renders from the store.
 * Built by build.mjs into the single file dist/panel.html.
 */
import { createClient, PANEL_CLIENT_OPTIONS, PRIORITY, type Bridge, type Client, type Outcome } from './data/client.js'
import { CompatProbe } from './data/compat.js'
import { IssueIndex, type Row } from './data/index.js'
import { createMemory, remember } from './data/memory.js'
import { isUnread, markAllRead, markGroupRead, markRead, type Marked } from './data/read.js'
import { createRefresher, DEFAULT_INTERVAL_MS } from './data/refresh.js'
import { loadAllRows, loadArchived, loadDetail, loadTallies, loadWikiListing, loadWikiPage, primeRows, sameTallies, withArchive, type Loaded } from './data/source.js'
import { copyFor } from './i18n/index.js'
import { wireKeys, type ReadMarks } from './keys/wiring.js'
import { createStore, type BoardData, type State } from './store.js'
import { listLines, narrow, sortOrder } from './view/list-model.js'
import { PANES } from './view/panes.js'
import { describe, mountShell, type Activity } from './view/shell.js'
import type { WikiSource } from './view/types.js'

declare global {
  interface Window {
    saycodePanel?: Bridge & { ready?: Promise<unknown> }
  }
}

const bridge = window.saycodePanel
const language = document.documentElement.lang || navigator.language || 'en'
const copy = copyFor(language)
const store = createStore()
// The last tab and the detail's place come back before anything is drawn. localStorage when the frame may use
// it, else this session only (data/memory.ts); reading `window.localStorage` itself can throw, hence the getter.
remember(store, createMemory(() => window.localStorage))
// What the reads' answers tell of the moai CLI and the daemon's catalog (data/compat.ts): the list, archive and wiki
// reads and the read marks hand it their outcomes (the detail does not), and the shell's banner says what
// to upgrade, update or install. Its report goes in the store after each answer it is handed, one that lands after
// its read was given up on too (the backlog read beside a failed `show --all`); the store keeps only a changed one.
const probe = new CompatProbe()
probe.subscribe(() => store.dispatch({ type: 'compat', report: probe.report() }))
// Up to 3 reads at once, and a read refused as workspace-busy tried again (data/client.ts PANEL_CLIENT_OPTIONS).
const runner = createClient(bridge ?? { invokeCommand: () => Promise.reject(new Error('no panel bridge')) }, PANEL_CLIENT_OPTIONS)
// A profile the catalog is read as lacking with the whole list (compat.ts lacks) is answered here, unrun: Desktop
// would ask the person to approve it first and the daemon refuse it after (se-24ey.dcj).
const LACKED: Outcome = { kind: 'refused', code: 'unsupported-daemon', assumed: true }
const client: Client = {
  ...runner,
  request: (profileId, value, options) => (probe.lacks(profileId) ? Promise.resolve(LACKED) : runner.request(profileId, value, options)),
}

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

/** Loads so far: only the latest one's answers hold or start the clock (se-24ey.a80). */
let loads = 0
/** The load the clock started on its own, or 0: its reads run without the activity toast. */
let quietLoad = 0

async function loadBoard(automatic = false): Promise<void> {
  const load = ++loads
  quietLoad = automatic ? load : 0
  // A read the person declined at its confirmation, or left unanswered, holds the clock; a load that reads the list
  // with nothing declined starts it again (holdClock). An earlier load's late answer decides nothing.
  let asked = false
  const told = (outcome: Outcome | null | undefined) => {
    if (load !== loads || !askedFor(outcome)) return
    asked = true
    holdClock()
  }
  store.dispatch({ type: 'board-loading' })
  // The rows and the milestone and epic counts `moai status` prints (they count archived rows too) side by side.
  // Without the counts the board counts its own rows. The load does not wait for the counts (their confirmation may
  // still be open): a failure is told at once, rows are drawn at once, and the counts are put on them when they come.
  // A refresh asked for meanwhile, or the `fresh` read after a write, reads its own rows and shares the status read
  // still running (client.ts); the panel's one write — read marks — moves no count.
  const listing = loadAllRows(client, PRIORITY.list, probe)
  const counting = loadTallies(client, PRIORITY.list, probe)
  let counted: Awaited<typeof counting> | undefined
  void counting.then((tallies) => {
    counted = tallies
    if (!tallies.ok) told(tallies.outcome)
  })
  const loaded = await listing
  if (!loaded.ok) {
    store.dispatch({ type: 'board-failed', problem: loaded.outcome })
    told(loaded.outcome)
    return
  }
  const { rows, more, truncated, partial, backlogProblem } = loaded.value
  told(backlogProblem)
  if (!asked && load === loads) startClock()
  const before = store.getState()
  // Counts that came first go in with the rows, so the board is not drawn twice. Until this read's counts come, a
  // refresh keeps the counts the board showed: its rows alone undercount (they leave the archived members out).
  const carried = counted === undefined ? before.board.value?.tallies : undefined
  const counts = counted?.ok ? counted.value : carried
  const data: BoardData = { rows, more, truncated, partial, loadedAt: Date.now(), ...(backlogProblem ? { backlogProblem } : {}), ...(counts ? { tallies: counts } : {}) }
  const shownBefore = before.shown.value
  // The detail on screen was read before this list and stands beside it: when its row has left the list
  // (finished, moved away) the pane goes with it, as choose() does for a read still running. A detail the list
  // never held (an issue a wiki page links to that only the archive holds) stays.
  const wasListed = shownBefore !== null && indexOf(before)?.find(shownBefore.id) !== undefined
  store.dispatch({ type: 'board-loaded', data, order: drawnOrder() })
  const state = store.getState()
  const shown = state.shown.value
  if (shown && wasListed && indexOf(state)?.find(shown.id) === undefined) store.dispatch({ type: 'detail-closed' })
  // Archived rows shown but not read yet (turned on before the rows were there, or the last read failed), or read beside
  // a whole list that is prime's rows now: the archive's answer stands in for the active rows prime lacks (source.ts
  // activeIds), and its copies of them were never drawn, so no read mark read it again (marked()). Not while a read
  // still pages: that one would be cancelled and the paging started over.
  const turned = primeRows(data) && before.board.value !== null && !primeRows(before.board.value)
  if (state.view.showArchived && archiveRunning === null && (state.archive === null || turned)) void loadArchive()
  // Counts that come later go on this board only: a later load's board has its own (store 'tallies'). Counts that
  // match the ones carried (most refreshes) change nothing on screen, so the board is not drawn again for them;
  // counts that could not be read leave the board counting its own rows, as when they fail first.
  if (counted === undefined) {
    void counting.then((late) => {
      if (late.ok ? !sameTallies(late.value, carried) : carried) store.dispatch({ type: 'tallies', data, tallies: late.ok ? late.value : null })
    })
  }
}

// Read marks written so far (marked()). The detail and archive reads hand it to the client (client.ts
// RequestOptions.epoch), so a read asked after a mark is never handed a run that began before it and prints the
// `unread` the mark took off.
let marks = 0

// The archive is read when archived rows are shown (`SPC v o`), each time they are turned on, once the active
// rows are there: a row they do not hold is archive-only, tagged `archived: true` (never NEW, never read-marked;
// a partial list — prime's rows — judges by column instead, source.ts activeIds).
let archiving = 0
/** The `archiving` number of the archive read still paging, or null when none is. */
let archiveRunning: number | null = null
/** The read marks written when that read was asked for: its runs answer for those, not for a later one. */
let archiveAsked = 0
/**
 * A read mark landed while the archive read asked before it still paged (marked()): it is read again once that one
 * ends with an answer. One cancelled or declined is the person's to start again (`SPC v o`).
 */
let archiveAgain = false
async function loadArchive(): Promise<void> {
  if (!store.getState().board.value) return
  const mine = ++archiving
  archiveRunning = mine
  archiveAsked = marks
  archiveAgain = false
  let answered = false
  try {
    // Read without `active`, as moai printed it (moai 0.17 marks its archive rows itself): withArchive tags the
    // archive-only rows left unmarked against the rows held when it draws, and a partial list needs the archive's own
    // rows to judge by (source.ts activeIds).
    const loaded = await loadArchived(client, { probe, cancelled: () => mine !== archiving, epoch: archiveAsked })
    if (mine !== archiving) return
    answered = loaded.ok
    // The archive's own problem: the board's rows and whatever the board said stand.
    if (loaded.ok) store.dispatch({ type: 'archive', rows: loaded.value })
    else store.dispatch({ type: 'archive-failed', problem: loaded.outcome })
  } finally {
    if (archiveRunning === mine) {
      archiveRunning = null
      if (archiveAgain && answered) void loadArchive()
    }
  }
}
store.subscribe((state, previous) => {
  if (state.view.showArchived === previous.view.showArchived) return
  if (state.view.showArchived) void loadArchive()
  else {
    // Turned off: a read still paging is not wanted, and the next turn-on reads the archive afresh.
    archiving++
    archiveAgain = false
    store.dispatch({ type: 'archive', rows: null })
  }
})

// Paused while the panel is out of sight — the Desktop window hidden, or the tool tab it sits in: the refresher's
// own visibility signal hears both (data/refresh.ts panelVisibility).
const refresher = createRefresher({ load: ({ automatic }) => loadBoard(automatic) })

/**
 * A list read the person declined at its confirmation, or left unanswered, holds the clock: Desktop asks again 30 s
 * after a decline, and the 10 s refresh would put the confirmation back up each time (se-24ey.14i). The next list read
 * that goes through with nothing declined — the refresh time clicked, or the read after a read mark — starts it again.
 * The shell says so on a line of its own while it holds (Shell.held).
 */
let clockHeld = false
const askedFor = (outcome: Outcome | null | undefined): boolean =>
  outcome?.kind === 'refused' && (outcome.code === 'declined' || outcome.code === 'approval-timeout')
function holdClock(): void {
  if (clockHeld) return
  clockHeld = true
  refresher.setInterval(0)
  shell.held(true)
}
function startClock(): void {
  if (!clockHeld) return
  clockHeld = false
  refresher.setInterval(DEFAULT_INTERVAL_MS)
  shell.held(false)
}

let choice = 0
async function choose(id: string) {
  const mine = ++choice
  // Only a row the list held can be "removed by a reload" below; an issue the list never held (a wiki page's link
  // to one only the archive holds) is shown when its read answers.
  const held = indexOf(store.getState())?.find(id) !== undefined
  store.dispatch({ type: 'cursor', id })
  store.dispatch({ type: 'detail-loading', id })
  let asked = marks
  let loaded = await loadDetail(client, id, { epoch: asked })
  // A read mark written while the read ran may have taken off the `unread` it printed (reloadDetail reads only a
  // detail already drawn): a row printed unread is read again, while the pane still waits for it.
  while (mine === choice && store.getState().shown.loading && asked !== marks && loaded.ok && isUnread(loaded.value)) {
    asked = marks
    loaded = await loadDetail(client, id, { epoch: asked })
  }
  // A later choice wins: its own answer settles the detail.
  if (mine !== choice) return
  const now = store.getState()
  // The pane was closed while the read ran (Close, or a reload that closed it): the answer is not wanted.
  if (!now.shown.loading) return
  // A reload removed the row. Nothing else will answer for the detail, so it closes instead of staying on
  // "Loading…". The cursor moving elsewhere is the user's own (a key press) and does not cancel the read.
  const listed = indexOf(now)
  if (held && listed && listed.find(id) === undefined) return store.dispatch({ type: 'detail-closed' })
  if (loaded.ok) store.dispatch({ type: 'detail-loaded', data: { id, row: loaded.value } })
  else store.dispatch({ type: 'detail-failed', id, problem: loaded.outcome })
}

/** What an outcome says, as one line of text (the wiki's error state takes a string). */
function said(outcome: Outcome): string {
  const parts = describe(outcome, copy, document)?.parts ?? [copy.failed]
  return parts.map((part) => (typeof part === 'string' ? part : part.textContent ?? '')).join('')
}

/** A wiki read's answer, or an Error with what stopped it (the wiki view shows it). */
function answer(loaded: Loaded<unknown>): unknown {
  if (loaded.ok) return loaded.value
  throw new Error(said(loaded.outcome))
}

const wiki: WikiSource = {
  list: async () => answer(await loadWikiListing(client, probe)),
  page: async (slug) => answer(await loadWikiPage(client, slug, probe)),
}

/**
 * The detail on screen again, as `show` prints it now, when the write marked it (`written`, as `moai read` answers): a
 * read mark takes its `unread` off, and nothing else reads the detail again. A row `show` printed without it has
 * nothing to lose. The pane and the cursor stay as they are; a choice made meanwhile, or the pane closed, wins.
 */
async function reloadDetail(written: ReadonlySet<string>): Promise<void> {
  const shown = store.getState().shown.value
  if (!shown || !isUnread(shown.row) || !written.has(shown.id)) return
  const mine = choice
  const loaded = await loadDetail(client, shown.id, { epoch: marks })
  if (!loaded.ok || mine !== choice || store.getState().shown.value?.id !== shown.id) return
  store.dispatch({ type: 'detail-loaded', data: { id: shown.id, row: loaded.value } })
}

/**
 * A read-mark write, then the list again, with the detail on screen and (a partial list's) the archive answer: the
 * marks come back with the rows (data/read.ts).
 */
async function marked(writing: Promise<Marked>): Promise<Outcome | null> {
  const done = await writing
  if (!done.ok) return done.outcome
  marks++
  const written = new Set(done.result.read)
  // A list read already running started before the write and cannot hold its marks. The detail's `show` row was read
  // before it too and the list's re-read does not bring it back: read again, its NEW would outlive the mark. The archive
  // follows the list's read alone, which tells whether the list is partial.
  await Promise.all([refresher.refresh({ fresh: true }).then(() => rereadArchive(written)), reloadDetail(written)])
  return null
}

/**
 * After a read mark and the list read again. A read of the archive still paging that was asked before the write is
 * read again once it ends, rather than started over on each mark (a whole list's too: one that turns partial takes the
 * active rows prime lacks from that answer, loadBoard). Else a partial list, which stands on prime's rows that carry
 * no marks, reads the archive answer again when the write marked (`written`) a row only it holds that is drawn NEW
 * (source.ts activeIds). A whole list needs none: the rows the archive adds to it are archive-only, never NEW.
 */
function rereadArchive(written: ReadonlySet<string>): void {
  const now = store.getState()
  const board = now.board.value
  if (!now.view.showArchived || !board) return
  if (archiveRunning !== null) archiveAgain ||= archiveAsked !== marks
  else if (primeRows(board) && indexOf(now)?.rows.slice(board.rows.length).some((row) => isUnread(row) && written.has(row.id))) void loadArchive()
}

const read: ReadMarks = {
  one: (id) => marked(markRead(client, { id }, probe)),
  all: () => marked(markAllRead(client, probe)),
  group: (id) => marked(markGroupRead(client, id, probe)),
}

function activity(): Activity {
  // The clock's own list reads run quietly: a toast every 10 s would cover the rows at the foot of the panes for
  // nothing (se-24ey.a80). A read the person asked for, or one that waits beside it (a detail, the wiki), still shows.
  const quiet = quietLoad !== 0 && quietLoad === loads
  const requests = client.requests().filter((request) => request.phase !== 'queued' && !(quiet && request.priority === PRIORITY.list))
  const running = requests.find((request) => request.operationId !== null)
  return {
    phase: running ? 'running' : requests.length ? 'starting' : 'idle',
    waiting: requests.some((request) => request.phase === 'starting'),
    // Nothing to cancel while the confirmation is still open: the run has no operation id yet.
    cancellable: running !== undefined,
  }
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
    // Up to three runs go at once (the list is three reads): Cancel stops every one that runs, and the reads that
    // wait behind them, rather than letting the next queued read start in the freed slot. A read mark still waiting
    // (its confirmation open, or queued) is left to the person: cancelling it would cancel it the moment they
    // approve it.
    cancel: () => {
      const requests = client.requests()
      if (!requests.some((request) => request.operationId !== null)) return
      for (const key of new Set(requests.filter((request) => request.operationId !== null || request.share).map((request) => request.key))) client.cancel(key)
    },
  },
})
client.subscribe(() => shell.activity(activity()))
// The one keydown listener: keys → the key table's actions → the store and the panes (keys/wiring.ts), with the
// SPC menu, the shortcut help and the "click to use the keyboard" hint.
wireKeys({ document, store, shell, copy, read })

void (async () => {
  try {
    await bridge?.ready
  } catch {
    // An older host without `ready` still answers commands.
  }
  // The host does not focus the panel's frame when it opens; try, so keys reach it once that is wired.
  document.body.tabIndex = -1
  document.body.focus({ preventScroll: true })
  await refresher.refresh()
})()
