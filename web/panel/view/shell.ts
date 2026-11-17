// The panel's frame (design §1): the tab bar (board, list, gantt, wiki; the refresh time, the theme button, the detail toggles), the
// search / filter / sort toolbar (view/toolbar.ts, its state in the store), the path line, the banners, the result line, the run's activity, and under them the
// split holding the list or board beside the detail. The panes are filled by the views named in
// view/panes.ts through the contract in view/types.ts; the shell renders the panes that show on every store
// change, and moves the split to the panel's width as it changes.
//
// The shell reads and dispatches to the store; it does no I/O. Reads and cancels are the caller's
// (main.ts) and arrive as ShellIntents.

import type { FileWatch, StampAnswer } from "../data/files-refresh.ts"
import { issueChanges, issuePatch, patchQueue, PATCH_GROUP, type IssueChanges, type Scope } from "../data/changes.ts"
import { createDiff, diffWords, failureWords, terminalLine, type PatchTicket } from "./diff.ts"
import { aboutFrom } from "../data/about.ts"
import { isInformation, type Outcome } from "../data/client.ts"
import type { IssueIndex } from "../data/index.ts"
import type { ProjectEntry } from "../data/projects.ts"
import { refreshTime } from "../data/refresh.ts"
import { applyTheme, nextTheme, type Theme } from "../data/theme.ts"
import { fill, type Copy } from "../i18n/index.ts"
import { detectPlatform } from "../keys/router.ts"
import { matchKey, toolbarOf, type Focus, type State, type Store, type Tab } from "../store.ts"
import { button, el, emptyState, icon, iconButton, stateLabel } from "./components.ts"
import type { IconName } from "./icons.ts"
import { mountBrand } from "./brand.ts"
import { allTips, createTips } from "./tips.ts"
import { mountCrumbs } from "./crumbs.ts"
import { COLUMN_DROPS, effectivePosition } from "./responsive.ts"
import { narrow, segKey } from "./list-model.ts"
import { createPositionToggle, createSplit } from "./split.ts"
import { createTabs, panelDomId, tabDomId } from "./tabs.ts"
import { unreadBadges } from "./unread.ts"
import { createToolbar, type LiveToolbar } from "./toolbar.ts"
import type { PaneLayout, PaneView, Panes, ViewContext, ViewIntents, WikiSource } from "./types.ts"

export interface Activity {
  /** 'running' while a read is out to `moai web`. */
  phase: "idle" | "running"
  cancellable: boolean
}

/** What the shell cannot do by dispatching: the reads and writes main.ts runs. */
export interface ShellIntents {
  refresh(): void
  /** Cursor on the row and its detail read. */
  choose(id: string): void
  cancel(): void
  /** A project in the header was clicked: main.ts opens it (data/projects.ts projectHref). */
  openProject?(entry: ProjectEntry): void
}

export interface ShellOptions {
  document: Document
  store: Store
  copy: Copy
  language: string
  intents: ShellIntents
  panes: Panes
  /** The index of the state's rows; the caller builds it once per read. */
  index(state: State): IssueIndex | null
  activity(): Activity
  /** The request queue, handed to the views that read beside the store (snippets in the detail and the wiki). */
  client?: ViewContext["client"]
  /** The wiki's reads, handed to the wiki pane; without them the wiki tab says it cannot read the wiki. */
  wiki?: WikiSource
  /** The gantt's write of a row's planned days (main.ts, data/plan.ts); without it the bars do not move. */
  movePlan?: ViewContext["movePlan"]
}

export interface Shell {
  el: HTMLElement
  /** Draw everything from the store now. The shell also does this on every store change. */
  render(): void
  /** The activity line and Cancel alone: they are all that follows the request queue, so nothing else redraws for it. */
  activity(current: Activity): void
  /** One line under the banners: what a key did (`D` saved the order), or why it did nothing. */
  say(text: string, problem?: boolean): void
  /** Why a read mark did nothing, on that same line. The banners belong to the reads and redraw with them. */
  refuse(outcome: Outcome): void
  /** `moai web` could not be reached (data/refresh.ts onReach): one line until it answers again. */
  offline(on: boolean): void
  /** Reads keep failing against a server that answers (data/refresh.ts onBackoff): one line saying they now come every 10 s. */
  backoff(on: boolean): void
  /**
   * Places other parts mount into: `toolbar` under the tab bar (holds the search / filter / sort toolbar, which
   * the shell mounts; hidden on the wiki tab), `actions` at the end of the tab bar (the help button, empty and
   * taking no room until something is put in it).
   */
  slots: { toolbar: HTMLElement; actions: HTMLElement }
  /** The panel's width changed. A ResizeObserver on the shell calls this; tests call it by hand. */
  setWidth(width: number): void
  layout(): PaneLayout
  /** The views in the panes, for the key router (panel/keys/wiring.ts) to hand its actions to. */
  panes(): { list: PaneView; board: PaneView; gantt: PaneView; detail: PaneView; diff: PaneView; wiki: PaneView | null; files: PaneView | null }
  fileWatch(): FileWatch | null
  refreshFileStamps(watch: FileWatch, dirs: string[], file: boolean, answer: StampAnswer): Promise<void>
  refreshFileChanges(watch: FileWatch, fingerprint: string): Promise<void>
  trackerChanged(): void
  /** Move the keyboard focus into a pane of the shown tab (its view's focus()). */
  focusPane(area: Focus): void
  /** Move the keyboard focus to the selected tab of the tab bar: the way out of the wiki, whose Tab cycles its links. */
  focusTabs(): void
  /** The search / filter / sort toolbar (its inputs: `/` and `SPC f` put the caret in them). */
  toolbar: LiveToolbar
  destroy(): void
}

/** What an outcome tells the user, and whether it is information (a choice, a retry) rather than an error. */
export function describe(outcome: Outcome, copy: Copy): { parts: (string | Node)[]; info: boolean } | null {
  switch (outcome.kind) {
    case "moai-error":
      return { parts: [fill(copy.moaiError, { message: outcome.message })], info: false }
    case "cancelled":
      return { parts: [copy.cancelled], info: isInformation(outcome) }
    case "unreadable":
      return { parts: [copy.unreadable], info: false }
    case "not-started":
      return { parts: [copy.unreachable], info: false }
    case "failed":
      return { parts: [outcome.message ? fill(copy.moaiError, { message: outcome.message }) : copy.failed], info: false }
    case "passed":
      return null
  }
}

/** The main pane's width: all of it, less the detail's share when the detail stands on either horizontal side. */
export function mainWidth(width: number, position: PaneLayout["position"], detailVisible: boolean, detailRatio: number): number {
  return (position === "left" || position === "right") && detailVisible ? width * (1 - detailRatio) : width
}

/** The theme button's icon: the state it is in, by shape (data/theme.ts). */
const THEME_ICONS: Record<Theme, IconName> = { system: "contrast", light: "sun", dark: "moon" }

/** How many right-hand columns a list this wide drops; a pane re-renders when this changes. */
const columnBand =(width: number): number => COLUMN_DROPS.filter((entry) => width < entry.below).length

type Saved = Array<[HTMLElement, number, number]>
const scrollers = (root: HTMLElement): HTMLElement[] => [root, ...root.querySelectorAll<HTMLElement>("[data-keep-scroll]")]
const saveScroll = (root: HTMLElement): Saved => scrollers(root).map((node) => [node, node.scrollTop, node.scrollLeft])
function restoreScroll(saved: Saved | undefined): void {
  for (const [node, top, left] of saved ?? []) {
    if (!node.isConnected) continue
    node.scrollTop = top
    node.scrollLeft = left
  }
}

export function mountShell(root: HTMLElement, options: ShellOptions): Shell {
  const { document: doc, store, copy, language, intents } = options
  const win = doc.defaultView
  let state = store.getState()

  // An unmeasured frame (0 wide: not laid out yet) counts as wide until the observer reports a real width.
  const measure = (): number => root.getBoundingClientRect().width || win?.innerWidth || Infinity
  let width = measure()

  // ── tab bar ──
  const tabs = createTabs(doc, {
    selected: state.tab,
    labels: copy.shell.tabs,
    ariaLabel: copy.shell.tabsLabel,
    onSelect: (tab) => store.dispatch({ type: "tab", tab }),
  })
  const refresh = el(doc, "button", { type: "button", class: "m-refreshed moai-refresh" }, copy.shell.refreshNow)
  refresh.addEventListener("click", () => intents.refresh())
  const diffToggle = iconButton(doc, { icon: "diff", label: stateLabel(copy.files.diff, state.diff.visible, copy.files), pressed: state.diff.visible })
  diffToggle.classList.add("m-diff-toggle")
  diffToggle.addEventListener("click", () => store.dispatch({ type: "diff-visible", visible: !store.getState().diff.visible }))
  const visibility = iconButton(doc, { icon: "eye", label: copy.shell.hideDetail, pressed: state.detail.visible })
  visibility.classList.add("moai-detail-toggle")
  visibility.addEventListener("click", () => store.dispatch({ type: "detail-visible", visible: !store.getState().detail.visible }))
  // Theme button (data/theme.ts): its icon is the state it is in, by shape; the name says the state and what pressing does.
  const themeButton = el(doc, "button", { type: "button", class: "m-iconbtn moai-theme-toggle" })
  themeButton.addEventListener("click", () => store.dispatch({ type: "theme", theme: nextTheme(store.getState().theme) }))
  let themeShown: Theme | null = null
  function paintTheme(theme: Theme) {
    if (theme === themeShown) return
    themeShown = theme
    applyTheme(doc, theme)
    const t = copy.shell.theme
    const toNext = { system: t.toSystem, light: t.toLight, dark: t.toDark }[nextTheme(theme)]
    const label = `${t[theme]} — ${toNext}`
    themeButton.setAttribute("aria-label", label)
    themeButton.title = label
    themeButton.dataset.state = theme
    themeButton.replaceChildren(icon(doc, THEME_ICONS[theme]))
  }
  paintTheme(state.theme)
  const position = createPositionToggle(doc, {
    position: effectivePosition(width, state.detail.position),
    width,
    onToggle: (next) => store.dispatch({ type: "detail-position", position: next }),
    labels: { toRight: copy.shell.detailToRight, toTop: copy.shell.detailToTop, toLeft: copy.shell.detailToLeft, toBottom: copy.shell.detailToBottom, forcedBottom: copy.shell.detailForcedBottom },
  })
  position.el.classList.add("moai-position-toggle")
  const actions = el(doc, "div", { class: "m-tabbar-slot" })
  const tabbar = el(doc, "div", { class: "m-tabbar" },
    tabs.el, el(doc, "div", { class: "m-tabbar-spacer" }),
    el(doc, "div", { class: "m-tabbar-actions" }, refresh, themeButton, visibility, diffToggle, position.el, actions))

  // ── under the tab bar: toolbar slot, path line, banners, result line ──
  const toolbar = el(doc, "div", { class: "m-toolbar-slot", hidden: state.tab === "wiki" || state.tab === "files" })
  // The toolbar is built once and brought to the store's state on every render (its inputs are never rebuilt
  // under the caret or an IME composition); what it does goes back to the store as a `toolbar` action.
  const bar = createToolbar({ doc, copy: copy.toolbar, lang: language as import("../i18n/index.ts").Language }, { dispatch: (action) => store.dispatch({ type: "toolbar", action }) })
  toolbar.append(bar.el)
  const crumbs = mountCrumbs(doc, copy, (path) => {
    // Going up lands on the place just left, as Backspace does (list.ts leave()).
    const from = store.getState().path
    const left = path.length < from.length && path.every((seg, n) => seg === from[n]) ? from[path.length] : undefined
    store.dispatch(left === undefined ? { type: "enter", path } : { type: "enter", path, cursor: segKey(left) })
    // The segment that was clicked is gone with the redraw: the focus goes to the list or board it changed.
    focusPane("list")
  })
  const activityText = el(doc, "span", { class: "moai-activity", role: "status" })
  const cancel = button(doc, { label: copy.cancel, small: true, onClick: () => intents.cancel() })
  cancel.classList.add("moai-cancel")
  const status = el(doc, "div", { class: "m-shell-status" }, activityText, cancel)
  const hint = el(doc, "p", { class: "moai-hint" }, copy.hint)
  const notice = el(doc, "div", { class: "m-banner moai-notice", role: "status" })
  const error = el(doc, "div", { class: "m-banner m-banner-danger moai-error", role: "alert" })
  // What moai said on stderr beside the lists (BoardData.notes): a line it could not read is told, not swallowed.
  const said = el(doc, "div", { class: "m-banner m-banner-warn moai-said", role: "status" })
  // Why the archive could not be read (`SPC v o`): its own line, as the board's rows and their banners stand. As for the
  // board's, an error is an alert and information is polite status text (components.ts banner).
  const archiveNotice = el(doc, "div", { class: "m-banner moai-archive-problem", role: "status" })
  const archiveError = el(doc, "div", { class: "m-banner m-banner-danger moai-archive-problem", role: "alert" })
  // A live region from the start: what a key did (or why it did nothing) is read out when it is written.
  const result = el(doc, "p", { class: "moai-result", role: "status" })
  function say(text: string, problem = false): void {
    result.textContent = text
    result.className = problem ? "moai-result m-banner m-banner-danger" : "moai-result"
  }
  // Its own line: the server out of reach is not a read's problem, and no read's answer takes it away.
  const away = el(doc, "div", { class: "m-banner m-banner-danger moai-offline", role: "alert" })
  // Its own line too: the board's error says why a read failed, this says the page now asks less often.
  const slow = el(doc, "div", { class: "m-banner m-banner-warn moai-backoff", role: "status" })
  const notes = el(doc, "div", { class: "m-shell-notes" }, hint, away, slow, notice, error, said, archiveNotice, archiveError, result)
  // The path and banners belong to the list and board; the wiki tab brings its own head. A panel
  // that opens on the wiki tab (the remembered one) starts without it: showTab() only runs on a change of tab.
  const head = el(doc, "div", { class: "m-shell-head", hidden: state.tab === "wiki" || state.tab === "files" }, crumbs.el, notes)

  // ── body: one panel for the list and board (they share the cursor, path and detail), one for the wiki ──
  const split = createSplit(doc, {
    position: state.detail.position,
    ratios: state.detail.ratio,
    detailVisible: state.detail.visible || state.diff.visible,
    labels: { resizer: copy.shell.resizer },
    onRatio: (at, ratio) => store.dispatch({ type: "detail-ratio", position: at, ratio }),
  })
  split.setWidth(width)
  const mainRoots: Record<"list" | "board" | "gantt", HTMLElement> = {
    list: el(doc, "div", { class: "m-pane-main", "data-pane": "list", tabindex: -1, hidden: state.tab !== "list" }),
    board: el(doc, "div", { class: "m-pane-main", "data-pane": "board", tabindex: -1, hidden: state.tab !== "board" }),
    // The gantt scrolls inside itself (one box for the grid and the dates), so the pane root does not.
    gantt: el(doc, "div", { class: "m-pane-main m-pane-gantt", "data-pane": "gantt", tabindex: -1, hidden: state.tab !== "gantt" }),
  }
  split.list.append(mainRoots.list, mainRoots.board, mainRoots.gantt)
  const detailRoot = el(doc, "div", { class: "m-pane-detail", tabindex: -1 })
  split.detail.append(detailRoot)
  const diffRoot = el(doc, "div", { class: "m-pane-diff", tabindex: -1 })
  const scopes = el(doc, "div", { class: "m-diff-scopes" })
  const diffWidget = createDiff(doc, diffWords(copy), value => store.dispatch({ type: "diff-layout", layout: value }), copy.files, {
    want: wantPatch,
    retry: () => { if (diffId) void readDiff(diffId, selectedScope) },
    command: file => diffId && selectedScope ? terminalLine(diffId, selectedScope, file.path, store.getState().view.worktree) : null,
  })
  diffWidget.slot.append(scopes); diffRoot.append(diffWidget.el)
  const inside = createSplit(doc, { position: "left", ratios: { left: 0.5, top: 0.5 }, onRatio: (position, ratio) => store.dispatch({ type: "diff-ratio", axis: position === "left" ? "horizontal" : "vertical", ratio: 1 - ratio }) })
  const foldedDiff = el(doc, "p", { class: "m-diff-collapsed", role: "status" }, copy.files.collapsed)
  let diffId: string | null = null, diffOverlay = false, issueFileStamp: string | null = null, issueStampPath = '', issueReadAt = 0, selectedScope: Scope | undefined, diffAnswer: IssueChanges | null = null, diffEpoch = 0, loudRead = 0
  let diffTimer: ReturnType<typeof setTimeout> | undefined
  /** The pane's patch reads: two at a time, dropped together when the pane moves to another issue, scope or overlay. */
  const patches = patchQueue(2)
  function dropPatches(): void { patches.clear(); options.client?.cancelGroup?.(PATCH_GROUP.issue) }
  function wantPatch(ticket: PatchTicket): void {
    const client = options.client, id = diffId, scope = selectedScope, overlay = store.getState().view.worktree
    if (!client || !id || !scope) { ticket.failed(copy.failed); return }
    patches.add({ live: ticket.live, async run() {
      const read = await issuePatch(client, id, scope, ticket.path, overlay, ticket.quiet)
      if (read.ok) ticket.done(read.value); else ticket.failed(failureWords(copy, read))
    } })
  }
  /** Rejects only for a background refresh that failed, so the refresher retries it; a read the person asked for says why on the pane. */
  function readDiff(id: string, scope?: Scope, background = false): Promise<void> {
    if (!options.client) return Promise.resolve()
    // A read the person is waiting on is fresher than any refresh, and a refresh that cut in would leave its spinner.
    if (background && loudRead) return Promise.resolve()
    const request = ++diffEpoch
    if (!background) { loudRead = request; dropPatches(); diffWidget.reading() }
    return issueChanges(options.client, id, scope, store.getState().view.worktree, background).then(read => {
      if (loudRead === request) loudRead = 0
      if (request !== diffEpoch || store.getState().cursor !== id || !store.getState().diff.visible) return
      if (!read.ok) {
        // A refresh that failed or was cancelled (the tab hid, the watch moved) leaves the diff it would have replaced.
        if (background) { issueFileStamp = null; throw new Error(read.why) }
        // Never an empty list: that would read as "no changes in this scope".
        diffAnswer = null; scopes.replaceChildren(); diffWidget.failed(failureWords(copy, read)); return
      }
      const answer = read.value
      if (selectedScope !== answer.scope || diffAnswer?.files[0]?.path !== answer.files[0]?.path) issueFileStamp = null
      const changed = !background || JSON.stringify(diffAnswer) !== JSON.stringify(answer)
      diffAnswer = answer; issueReadAt = Date.now(); selectedScope = answer.scope ?? undefined
      if (changed) {
        // The server's own words for a git it could not use or a scope it could not read, with the retry beside them.
        if (answer.vcs_error) diffWidget.failed(answer.vcs_error.said)
        else diffWidget.list({ files: answer.files, truncated: answer.truncated }, background)
        paintScopes()
      }
      shell.dispatchEvent(new doc.defaultView!.Event("moai-file-watch", { bubbles: true }))
    })
  }
  function paintScopes(): void {
    scopes.replaceChildren(...(diffAnswer?.offers ?? []).map(offer => {
      const button = el(doc, "button", { type: "button", "aria-pressed": String(offer.scope === selectedScope) }, `${offer.scope}${offer.branch ? ` · ${offer.branch}` : ""}${offer.owner ? ` · ${fill(copy.diff.owner, { id: offer.owner })}` : ""}`)
      button.addEventListener("click", () => { selectedScope = offer.scope; if (diffId) void readDiff(diffId, offer.scope).catch(() => {}) })
      return button
    }))
  }
  function diffCommand(action: string): boolean {
    if (action === "diff-layout") return diffWidget.command("toggle")
    if (action === "next" || action === "prev" || action === "left" || action === "right") return diffWidget.command(action)
    if (action === "scope-prev" || action === "scope-next") {
      const offers = diffAnswer?.offers ?? []; if (!offers.length || !diffId) return false
      const at = Math.max(0, offers.findIndex(o => o.scope === selectedScope)), step = action === "scope-next" ? 1 : -1
      const chosen = offers[Math.max(0, Math.min(offers.length - 1, at + step))]!
      selectedScope = chosen.scope; void readDiff(diffId, chosen.scope).catch(() => {}); return true
    }
    return false
  }
  const diffView: PaneView = { render() {}, focus: () => diffWidget.focus(), command: diffCommand }
  // The store's `focus` follows the DOM's: a click or Tab into a pane (not only focusPane) says which pane has
  // the keys. The reducer ignores a focus that is already the one. The diff pane sits inside the detail side of
  // the split, so one listener tells the two apart: a listener on each would let the outer one win.
  split.list.addEventListener("focusin", () => store.dispatch({ type: "focus", focus: "list" }))
  split.detail.addEventListener("focusin", (event) => store.dispatch({ type: "focus", focus: event.target instanceof doc.defaultView!.Node && diffRoot.contains(event.target) ? "diff" : "detail" }))
  // The work tabs (list, board, gantt) control this one panel; its label follows the tab shown.
  const work = el(doc, "div", { class: "m-tabpanel", role: "tabpanel", id: panelDomId("list"), hidden: state.tab === "wiki" || state.tab === "files" }, split.el)
  for (const id of ["board", "gantt"] as const) tabs.el.querySelector(`#${tabDomId(id)}`)?.setAttribute("aria-controls", panelDomId("list"))
  const wiki = el(doc, "div", { class: "m-tabpanel m-pane-wiki", role: "tabpanel", id: panelDomId("wiki"), "aria-labelledby": tabDomId("wiki"), tabindex: -1, hidden: state.tab !== "wiki" })
  const files = el(doc, "div", { class: "m-tabpanel m-pane-files", role: "tabpanel", id: panelDomId("files"), "aria-labelledby": tabDomId("files"), tabindex: -1, hidden: state.tab !== "files" })
  const body = el(doc, "div", { class: "m-body" }, work, wiki, files)

  // The activity stays on every tab (the wiki reads too), floating over the foot of the panes so a read never pushes
  // them (shell.css, se-24ey.14i).
  // The TUI's header (view/brand.ts): the logo, then who and which moai (from `/api/boot`, data/about.ts), and a tip
  // that opens the tips page (view/tips.ts).
  // The key router's own reading of the platform, so the page and the keys agree on which Ctrl is the host's.
  const platform = detectPlatform(win?.navigator)
  const tipsPage = createTips(doc, copy.shell.tips, platform)
  const brand = mountBrand(doc, copy.shell.version, { tips: allTips(copy.shell.tips), label: copy.shell.tips.openAll, open: () => tipsPage.open() }, (entry) => intents.openProject?.(entry))
  const shell = el(doc, "div", { class: "m-shell" }, brand.el, tabbar, toolbar, head, body, status, tipsPage.el)
  root.replaceChildren(shell)

  // ── panes ──
  const layout = (): PaneLayout => {
    const current = store.getState().detail
    const shown = effectivePosition(width, current.position)
    const visible = current.visible || store.getState().diff.visible
    return { width, position: shown, detailVisible: current.visible, mainWidth: mainWidth(width, shown, visible, split.ratio(shown)) }
  }
  const viewIntents: ViewIntents = {
    enter: (path, cursor) => store.dispatch(cursor === undefined ? { type: "enter", path } : { type: "enter", path, cursor }),
    choose: (id) => intents.choose(id),
    point: (id) => store.dispatch({ type: "cursor", id }),
    closeDetail: () => {
      store.dispatch({ type: "detail-closed" })
      // The Close button is gone with the pane's content; the focus goes back to the list.
      focusPane("list")
    },
    setDetailVisible: (visible) => store.dispatch({ type: "detail-visible", visible }),
    toggleExpanded: (path) => store.dispatch({ type: "toggle-expanded", path }),
    refresh: () => intents.refresh(),
    dispatch: (action) => store.dispatch(action),
  }
  const focusPane = (area: Focus) => {
    store.dispatch({ type: "focus", focus: area })
    const now = store.getState()
    if (area === "diff") return diffWidget.focus()
    if (area === "detail") return detailView.focus ? detailView.focus() : detailRoot.focus()
    if (now.tab === "wiki") return wikiView?.focus ? wikiView.focus() : wiki.focus()
    if (now.tab === "files") return filesView?.focus ? filesView.focus() : files.focus()
    const view = mainViews[now.tab]
    return view.focus ? view.focus() : mainRoots[now.tab].focus()
  }
  const ctx: ViewContext = { document: doc, copy, language, intents: viewIntents, layout, focusPane, client: options.client, wiki: options.wiki, say, movePlan: options.movePlan }
  const mainViews: Record<"list" | "board" | "gantt", PaneView> = {
    list: options.panes.list(mainRoots.list, ctx),
    board: options.panes.board(mainRoots.board, ctx),
    // A page given no gantt view (a test's panes) still has the tab: it draws nothing.
    gantt: options.panes.gantt?.(mainRoots.gantt, ctx) ?? { render() {} },
  }
  const detailView = options.panes.detail(detailRoot, ctx)
  const wikiView = options.panes.wiki?.(wiki, ctx) ?? null
  const filesView = options.panes.files?.(files, ctx) ?? null
  if (!wikiView) wiki.append(emptyState(doc, { icon: "book-open", title: copy.shell.wikiPlaceholder }))
  // Inside the wiki, Tab cycles the page's links, so a keyboard cannot Tab out of it: an Esc the wiki does not
  // use (no link selected, no search to clear) moves the focus to the tab bar. The key router does that
  // (keys/wiring.ts `wiki.clear`): it is the one keydown listener.
  const focusTabs = () => tabs.el.querySelector<HTMLElement>(`#${tabDomId(store.getState().tab)}`)?.focus()

  // ── tab switching: each tab keeps its scroll offsets (display:none drops them) ──
  const containers: Record<Tab, HTMLElement> = { list: mainRoots.list, board: mainRoots.board, gantt: mainRoots.gantt, wiki, files }
  const saved = new Map<Tab | "detail", Saved>()
  let shownTab: Tab = state.tab
  /** Show `tab`'s panes; returns the offsets to put back once they are drawn. */
  function showTab(tab: Tab): () => void {
    if (tab === shownTab) return () => {}
    const leaving = shownTab
    saved.set(leaving, saveScroll(containers[leaving]))
    if (tab === "wiki") saved.set("detail", saveScroll(split.detail))
    containers[leaving].hidden = true
    containers[tab].hidden = false
    work.hidden = tab === "wiki" || tab === "files"
    head.hidden = tab === "wiki" || tab === "files"
    toolbar.hidden = tab === "wiki" || tab === "files"
    shownTab = tab
    const offsets = saved.get(tab) ?? []
    restoreScroll(offsets.slice(0, 1))
    return () => {
      restoreScroll(offsets.slice(1))
      if (leaving === "wiki") restoreScroll(saved.get("detail"))
    }
  }

  // ── drawing ──
  function showActivity(current: Activity) {
    const line = current.phase === "running" ? copy.running : ""
    // A live region is read again when it is written, so an unchanged line is left alone.
    if (activityText.textContent !== line) activityText.textContent = line
    cancel.hidden = !current.cancellable
    status.hidden = current.phase === "idle" && !current.cancellable
  }

  // The banners are live regions (role=alert, role=status): each pair is written once per outcome, not again on every
  // store change, or a screen reader would read it again on each cursor move. A failure that happens again is a new
  // outcome object, so it is told again. `lead` goes before what describe() says.
  function teller(noticeEl: HTMLElement, errorEl: HTMLElement, lead?: string) {
    let last: Outcome | null = null
    return (outcome: Outcome | null) => {
      if (outcome === last) return
      last = outcome
      noticeEl.replaceChildren()
      errorEl.replaceChildren()
      const said = outcome ? describe(outcome, copy) : null
      if (said) (said.info ? noticeEl : errorEl).append(el(doc, "span", {}, ...(lead ? [lead, " "] : []), ...said.parts))
    }
  }
  const explain = teller(notice, error)
  const explainArchive = teller(archiveNotice, archiveError, copy.shell.archiveFailed)

  let saidTold = ""
  function explainSaid(lines: readonly string[]) {
    // A live region: written only when its lines change, or every refresh would read them out again.
    const told = lines.join("\n")
    if (told === saidTold) return
    saidTold = told
    said.replaceChildren(...(lines.length ? [el(doc, "p", {}, copy.said), ...lines.map((line) => el(doc, "p", {}, line))] : []))
  }

  function paintLayout(current: State) {
    const detail = current.detail
    if (split.preferred() !== detail.position) split.setPosition(detail.position)
    for (const at of ["right", "bottom", "left", "top"] as const) if (split.ratio(at) !== detail.ratio[at]) split.setRatio(at, detail.ratio[at])
    split.setDetailVisible(detail.visible || current.diff.visible)
    diffToggle.hidden = current.tab === "wiki" || current.tab === "files"
    const diffName = stateLabel(copy.files.diff, current.diff.visible, copy.files)
    diffToggle.setAttribute("aria-pressed", String(current.diff.visible)); diffToggle.setAttribute("aria-label", diffName); diffToggle.title = diffName
    const horizontal = split.position() === "left" || split.position() === "right"
    const at = horizontal ? "left" : "top"
    inside.setPosition(at); inside.setWidth(Infinity); inside.setRatio(at, 1 - (horizontal ? current.diff.horizontal : current.diff.vertical))
    diffWidget.setLayout(current.diffLayout); diffWidget.setWidth(width)
    if (current.diff.visible && detail.visible && width >= 480) {
      // Only a change of layout moves a pane: re-inserting one on every render drops its scroll offsets and the
      // keyboard focus inside it.
      if (detailRoot.parentElement !== inside.detail) inside.detail.append(detailRoot)
      if (diffRoot.parentElement !== inside.list) inside.list.append(diffRoot)
      if (split.detail.firstElementChild !== inside.el) split.detail.replaceChildren(inside.el)
    } else if (current.diff.visible && !detail.visible) {
      if (split.detail.firstElementChild !== diffRoot) split.detail.replaceChildren(diffRoot)
    } else if (current.diff.visible && detail.visible) {
      if (split.detail.firstElementChild !== detailRoot || !foldedDiff.isConnected) split.detail.replaceChildren(detailRoot, foldedDiff)
    } else if (split.detail.firstElementChild !== detailRoot || split.detail.children.length !== 1) split.detail.replaceChildren(detailRoot)
    position.update(split.position(), width)
    shell.dataset.position = split.position()
    const label = detail.visible ? copy.shell.hideDetail : copy.shell.showDetail
    visibility.setAttribute("aria-pressed", String(detail.visible))
    visibility.setAttribute("aria-label", label)
    visibility.title = label
  }

  let band = columnBand(layout().mainWidth)
  function render() {
    state = store.getState()
    paintTheme(state.theme)
    const index = options.index(state)
    if (tabs.selected() !== state.tab) tabs.setSelected(state.tab)
    // The unread count rides the board and list tabs; with no index there is nothing to count (se-ehdi.zcx).
    const badges = unreadBadges(index?.rows ?? [])
    tabs.setBadge("board", badges.board)
    tabs.setBadge("list", badges.list)
    work.setAttribute("aria-labelledby", tabDomId(state.tab === "board" || state.tab === "gantt" ? state.tab : "list"))
    const left = shownTab
    const putBack = showTab(state.tab)

    const canReadDiff = state.diff.visible && state.tab !== "wiki" && state.tab !== "files"
    if (!canReadDiff || !state.cursor) {
      if (diffId !== null) { diffEpoch++; loudRead = 0; clearTimeout(diffTimer); dropPatches(); diffId = null; diffAnswer = null; scopes.replaceChildren(); diffWidget.clear() }
    } else if (state.cursor !== diffId || diffOverlay !== state.view.worktree) {
      diffOverlay = state.view.worktree; diffId = state.cursor; issueFileStamp = null; selectedScope = undefined; diffAnswer = null; diffEpoch++; loudRead = 0; clearTimeout(diffTimer); dropPatches()
      // Reading, not empty: the pane never says "no changes" for an issue it has not read yet.
      scopes.replaceChildren(); diffWidget.reading()
      const id = diffId; diffTimer = setTimeout(() => void readDiff(id).catch(() => {}), 120)
    }

    const loadedAt = state.board.value?.loadedAt
    const at = loadedAt === undefined ? null : refreshTime(loadedAt, language)
    refresh.textContent = at ? `↻ ${at}` : copy.shell.refreshNow
    refresh.title = at ? fill(copy.refreshedAt, { time: at }) : copy.shell.refreshNow
    refresh.setAttribute("aria-label", refresh.title)
    paintLayout(state)
    band = columnBand(layout().mainWidth)

    showActivity(options.activity())
    hint.hidden = state.board.value !== null
    // The hidden detail's own problem takes the line the board's leaves.
    explain(state.board.problem ?? (state.detail.visible ? null : state.shown.problem))
    explainSaid(state.board.value?.notes ?? [])
    explainArchive(state.archiveProblem)
    brand.render(aboutFrom(state.boot), state.projects)

    if (state.tab === "wiki") {
      wikiView?.render(state, index)
    } else if (state.tab === "files") {
      filesView?.render(state, index)
    } else {
      // What moai found and why it refused a filter are the toolbar's to show; it judges neither.
      const found = index && state.search.text.trim() !== "" ? narrow(state, index) : null
      // A refusal answers the question on screen only: an emptied filter asks nothing, and its old refusal goes.
      const refused = matchKey(state) === "" ? null : state.matched.problem
      bar.update(toolbarOf(state), {
        rows: index?.rows ?? [],
        ctx: { statuses: state.boot?.statuses },
        found: found && index ? index.rows.filter((_, at) => found.match(at)) : [],
        hiddenBy: index ? narrow(state, index).hiddenBy : undefined,
        // moai's own words for a filter it cannot read (`bad_filter`), as they are; anything else as the banners say it.
        problem: !refused ? undefined
          : refused.kind === "moai-error" ? refused.message
          : describe(refused, copy)?.parts.map((part) => (typeof part === "string" ? part : part.textContent ?? "")).join(""),
      })
      crumbs.render(state, index)
      mainViews[state.tab].render(state, index)
      if (state.detail.visible) detailView.render(state, index)
    }
    putBack()
    if (left === "gantt" && state.tab !== "gantt") mainViews.gantt.leave?.()
    // Last, as it dispatches (and so renders again): the wiki saves its page and trail into the store.
    if (left === "wiki" && state.tab !== "wiki") wikiView?.leave?.()
  }

  function setWidth(next: number) {
    if (!(next > 0) || next === width) return
    width = next
    split.setWidth(width)
    position.update(split.position(), width)
    shell.dataset.position = split.position()
    const current = store.getState()
    paintLayout(current)
    shell.dispatchEvent(new doc.defaultView!.Event("moai-file-watch", { bubbles: true }))
    if (width < 480 && current.diff.visible && current.detail.visible && current.focus === "diff") store.dispatch({ type: "focus", focus: "list" })
    // The wiki pane lays itself out from the width it is given when it renders (view/wiki.ts mountWiki): a narrow
    // panel stacks its list over the page.
    if (current.tab === "wiki") return wikiView?.render(current, options.index(current))
    if (current.tab === "files") return filesView?.render(current, options.index(current))
    // The list drops right-hand columns as it narrows; draw it again only when a breakpoint is crossed.
    const now = columnBand(layout().mainWidth)
    if (now === band) return
    band = now
    mainViews[current.tab].render(current, options.index(current))
  }

  const observer = win && "ResizeObserver" in win ? new win.ResizeObserver((entries) => setWidth(entries[0]?.contentRect.width ?? 0)) : null
  const onResize = () => setWidth(measure())
  if (observer) observer.observe(root)
  else win?.addEventListener("resize", onResize)

  const unsubscribe = store.subscribe(render)
  render()

  return {
    el: shell,
    render,
    activity: showActivity,
    say,
    refuse(outcome) {
      const said = describe(outcome, copy)
      result.replaceChildren(...(said ? [el(doc, "span", {}, ...said.parts)] : []))
      result.className = said && !said.info ? "moai-result m-banner m-banner-danger" : "moai-result m-banner"
    },
    offline(on) {
      const text = on ? copy.unreachable : ""
      if (away.textContent !== text) away.textContent = text
    },
    backoff(on) {
      const text = on ? copy.backingOff : ""
      if (slow.textContent !== text) slow.textContent = text
    },
    slots: { toolbar, actions },
    setWidth,
    layout,
    panes: () => ({ list: mainViews.list, board: mainViews.board, gantt: mainViews.gantt, detail: detailView, diff: diffView, wiki: wikiView, files: filesView }),
    fileWatch() {
      const now = store.getState()
      if (now.tab === "files") return filesView?.fileWatch?.() ?? { key: "files:pending", dirs: [] }
      if (!now.diff.visible || (now.detail.visible && width < 480) || now.tab === "wiki" || !diffId || !selectedScope) return null
      if (selectedScope === "commits") return { key: `issue:${diffId}`, dirs: [] }
      const offer = diffAnswer?.offers.find(o => o.scope === selectedScope)
      const path = diffWidget.visiblePath()
      if (path !== issueStampPath) { issueStampPath = path ?? ''; issueFileStamp = null }
      return { key: `issue:${diffId}`, dirs: [], ...(offer?.checkout ? { root: offer.checkout } : {}), ...(selectedScope === "working" && path ? { file: { path, stamp: issueFileStamp, readAt: issueReadAt } } : {}), changeKey: JSON.stringify(["issue", diffId, selectedScope, now.view.worktree]) }
    },
    async refreshFileStamps(watch, dirs, file, answer) {
      if (watch.key.startsWith("files:")) await filesView?.refreshFileStamps?.(watch, dirs, file, answer)
      else if (file && watch.key === `issue:${diffId}` && diffId && selectedScope === "working") {
        issueFileStamp = answer.file; await readDiff(diffId, selectedScope, true)
        // The file's text changed on disk; its counts may not have: read the open one again, quietly.
        if (watch.file?.path) diffWidget.reload(watch.file.path)
      }
    },
    async refreshFileChanges(watch, fingerprint) {
      if (watch.key.startsWith("files:")) await filesView?.refreshFileChanges?.(watch, fingerprint)
      else if (watch.key === `issue:${diffId}` && diffId && selectedScope && selectedScope !== "commits") await readDiff(diffId, selectedScope, true)
    },
    trackerChanged() { if (diffId && selectedScope && selectedScope !== "working" && !(store.getState().detail.visible && width < 480) && store.getState().diff.visible && store.getState().tab !== "files" && store.getState().tab !== "wiki") readDiff(diffId, selectedScope, true).catch(() => {}) },
    focusPane,
    focusTabs,
    toolbar: bar,
    destroy() {
      unsubscribe()
      observer?.disconnect()
      win?.removeEventListener("resize", onResize)
      split.destroy()
      inside.destroy(); clearTimeout(diffTimer); diffEpoch++; dropPatches(); diffWidget.clear()
      mainViews.list.destroy?.()
      mainViews.board.destroy?.()
      mainViews.gantt.destroy?.()
      detailView.destroy?.()
      wikiView?.destroy?.()
      filesView?.destroy?.()
      bar.destroy()
    },
  }
}
