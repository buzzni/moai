// The panel's frame (design §1): the tab bar (board, list, wiki; the refresh time, the detail toggles), the
// search / filter / sort toolbar (view/toolbar.ts, its state in the store), the path line, the banners, the result line, the run's activity, and under them the
// split holding the list or board beside the detail. The panes are filled by the views named in
// view/panes.ts through the contract in view/types.ts; the shell renders the panes that show on every store
// change, and moves the split to the panel's width as it changes.
//
// The shell reads and dispatches to the store; it does no I/O. Reads and cancels are the caller's
// (main.ts) and arrive as ShellIntents.

import { isInformation, type Outcome } from "../data/client.ts"
import type { IssueIndex } from "../data/index.ts"
import { refreshTime } from "../data/refresh.ts"
import type { Copy } from "../i18n/index.ts"
import { matchKey, toolbarOf, type Focus, type State, type Store, type Tab } from "../store.ts"
import { button, el, emptyState, iconButton } from "./components.ts"
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
  /** The wiki's reads, handed to the wiki pane; without them the wiki tab says it cannot read the wiki. */
  wiki?: WikiSource
}

export interface Shell {
  el: HTMLElement
  /** Draw everything from the store now. The shell also does this on every store change. */
  render(): void
  /** The activity line and Cancel alone: they are all that follows the request queue, so nothing else redraws for it. */
  activity(current: Activity): void
  /** One line under the banners: what a key did (`D` saved the order), or why it did nothing. */
  say(text: string, problem?: boolean): void
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
  panes(): { list: PaneView; board: PaneView; detail: PaneView; wiki: PaneView | null }
  /** Move the keyboard focus into a pane of the shown tab (its view's focus()). */
  focusPane(area: Focus): void
  /** Move the keyboard focus to the selected tab of the tab bar: the way out of the wiki, whose Tab cycles its links. */
  focusTabs(): void
  /** The search / filter / sort toolbar (its inputs: `/` and `SPC f` put the caret in them). */
  toolbar: LiveToolbar
  destroy(): void
}

/** What an outcome tells the user, and whether it is information (a choice, a retry) rather than an error. */
export function describe(outcome: Outcome, copy: Copy, doc: Document): { parts: (string | Node)[]; info: boolean } | null {
  switch (outcome.kind) {
    case "moai-error":
      return { parts: [copy.moaiError(outcome.message)], info: false }
    case "cancelled":
      return { parts: [copy.cancelled], info: isInformation(outcome) }
    case "unreadable":
      return { parts: [copy.unreadable], info: false }
    case "not-started":
      return { parts: [copy.unreachable], info: false }
    case "failed":
      return { parts: [outcome.message ? copy.moaiError(outcome.message) : copy.failed], info: false }
    case "passed":
      return null
  }
}

/** The main pane's width: all of it, less the detail's share when the detail stands on the left. */
export function mainWidth(width: number, position: PaneLayout["position"], detailVisible: boolean, leftRatio: number): number {
  return position === "left" && detailVisible ? width * (1 - leftRatio) : width
}

/** How many right-hand columns a list this wide drops; a pane re-renders when this changes. */
const columnBand = (width: number): number => COLUMN_DROPS.filter((entry) => width < entry.below).length

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
  const visibility = iconButton(doc, { icon: "eye", label: copy.shell.hideDetail, pressed: state.detail.visible })
  visibility.classList.add("moai-detail-toggle")
  visibility.addEventListener("click", () => store.dispatch({ type: "detail-visible", visible: !store.getState().detail.visible }))
  const position = createPositionToggle(doc, {
    position: effectivePosition(width, state.detail.position),
    width,
    onToggle: (next) => store.dispatch({ type: "detail-position", position: next }),
    labels: { toLeft: copy.shell.detailToLeft, toBottom: copy.shell.detailToBottom, forcedBottom: copy.shell.detailForcedBottom },
  })
  position.el.classList.add("moai-position-toggle")
  const actions = el(doc, "div", { class: "m-tabbar-slot" })
  const tabbar = el(doc, "div", { class: "m-tabbar" },
    tabs.el, el(doc, "div", { class: "m-tabbar-spacer" }),
    el(doc, "div", { class: "m-tabbar-actions" }, refresh, visibility, position.el, actions))

  // ── under the tab bar: toolbar slot, path line, banners, result line ──
  const toolbar = el(doc, "div", { class: "m-toolbar-slot", hidden: state.tab === "wiki" })
  // The toolbar is built once and brought to the store's state on every render (its inputs are never rebuilt
  // under the caret or an IME composition); what it does goes back to the store as a `toolbar` action.
  const bar = createToolbar({ doc, copy: copy.toolbar }, { dispatch: (action) => store.dispatch({ type: "toolbar", action }) })
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
  const notes = el(doc, "div", { class: "m-shell-notes" }, hint, notice, error, said, archiveNotice, archiveError, result)
  // The path and banners belong to the list and board; the wiki tab brings its own head. A panel
  // that opens on the wiki tab (the remembered one) starts without it: showTab() only runs on a change of tab.
  const head = el(doc, "div", { class: "m-shell-head", hidden: state.tab === "wiki" }, crumbs.el, notes)

  // ── body: one panel for the list and board (they share the cursor, path and detail), one for the wiki ──
  const split = createSplit(doc, {
    position: state.detail.position,
    ratios: state.detail.ratio,
    detailVisible: state.detail.visible,
    labels: { resizer: copy.shell.resizer },
    onRatio: (at, ratio) => store.dispatch({ type: "detail-ratio", position: at, ratio }),
  })
  split.setWidth(width)
  const mainRoots: Record<"list" | "board", HTMLElement> = {
    list: el(doc, "div", { class: "m-pane-main", "data-pane": "list", tabindex: -1, hidden: state.tab !== "list" }),
    board: el(doc, "div", { class: "m-pane-main", "data-pane": "board", tabindex: -1, hidden: state.tab !== "board" }),
  }
  split.list.append(mainRoots.list, mainRoots.board)
  const detailRoot = el(doc, "div", { class: "m-pane-detail", tabindex: -1 })
  split.detail.append(detailRoot)
  // The store's `focus` follows the DOM's: a click or Tab into a pane (not only focusPane) says which pane has
  // the keys. The reducer ignores a focus that is already the one.
  split.list.addEventListener("focusin", () => store.dispatch({ type: "focus", focus: "list" }))
  split.detail.addEventListener("focusin", () => store.dispatch({ type: "focus", focus: "detail" }))
  // Both work tabs control this one panel; its label follows the tab shown.
  const work = el(doc, "div", { class: "m-tabpanel", role: "tabpanel", id: panelDomId("list"), hidden: state.tab === "wiki" }, split.el)
  tabs.el.querySelector(`#${tabDomId("board")}`)?.setAttribute("aria-controls", panelDomId("list"))
  const wiki = el(doc, "div", { class: "m-tabpanel m-pane-wiki", role: "tabpanel", id: panelDomId("wiki"), "aria-labelledby": tabDomId("wiki"), tabindex: -1, hidden: state.tab !== "wiki" })
  const body = el(doc, "div", { class: "m-body" }, work, wiki)

  // The activity stays on every tab (the wiki reads too), floating over the foot of the panes so a read never pushes
  // them (shell.css, se-24ey.14i).
  const shell = el(doc, "div", { class: "m-shell" }, tabbar, toolbar, head, body, status)
  root.replaceChildren(shell)

  // ── panes ──
  const layout = (): PaneLayout => {
    const current = store.getState().detail
    const shown = effectivePosition(width, current.position)
    return { width, position: shown, detailVisible: current.visible, mainWidth: mainWidth(width, shown, current.visible, split.ratio("left")) }
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
    if (area === "detail") return detailView.focus ? detailView.focus() : detailRoot.focus()
    if (now.tab === "wiki") return wikiView?.focus ? wikiView.focus() : wiki.focus()
    const view = mainViews[now.tab]
    return view.focus ? view.focus() : mainRoots[now.tab].focus()
  }
  const ctx: ViewContext = { document: doc, copy, language, intents: viewIntents, layout, focusPane, wiki: options.wiki }
  const mainViews: Record<"list" | "board", PaneView> = {
    list: options.panes.list(mainRoots.list, ctx),
    board: options.panes.board(mainRoots.board, ctx),
  }
  const detailView = options.panes.detail(detailRoot, ctx)
  const wikiView = options.panes.wiki?.(wiki, ctx) ?? null
  if (!wikiView) wiki.append(emptyState(doc, { icon: "book-open", title: copy.shell.wikiPlaceholder }))
  // Inside the wiki, Tab cycles the page's links, so a keyboard cannot Tab out of it: an Esc the wiki does not
  // use (no link selected, no search to clear) moves the focus to the tab bar. The key router does that
  // (keys/wiring.ts `wiki.clear`): it is the one keydown listener.
  const focusTabs = () => tabs.el.querySelector<HTMLElement>(`#${tabDomId(store.getState().tab)}`)?.focus()

  // ── tab switching: each tab keeps its scroll offsets (display:none drops them) ──
  const containers: Record<Tab, HTMLElement> = { list: mainRoots.list, board: mainRoots.board, wiki }
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
    work.hidden = tab === "wiki"
    head.hidden = tab === "wiki"
    toolbar.hidden = tab === "wiki"
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
      const said = outcome ? describe(outcome, copy, doc) : null
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
    for (const at of ["left", "bottom"] as const) if (split.ratio(at) !== detail.ratio[at]) split.setRatio(at, detail.ratio[at])
    split.setDetailVisible(detail.visible)
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
    const index = options.index(state)
    if (tabs.selected() !== state.tab) tabs.setSelected(state.tab)
    // The unread count rides the board and list tabs; with no index there is nothing to count (se-ehdi.zcx).
    const badges = unreadBadges(index?.rows ?? [])
    tabs.setBadge("board", badges.board)
    tabs.setBadge("list", badges.list)
    work.setAttribute("aria-labelledby", tabDomId(state.tab === "board" ? "board" : "list"))
    const left = shownTab
    const putBack = showTab(state.tab)

    const loadedAt = state.board.value?.loadedAt
    const at = loadedAt === undefined ? null : refreshTime(loadedAt, language)
    refresh.textContent = at ? `↻ ${at}` : copy.shell.refreshNow
    refresh.title = at ? copy.refreshedAt(at) : copy.shell.refreshNow
    refresh.setAttribute("aria-label", refresh.title)
    refresh.disabled = state.board.loading
    paintLayout(state)
    band = columnBand(layout().mainWidth)

    showActivity(options.activity())
    hint.hidden = state.board.value !== null
    // The hidden detail's own problem takes the line the board's leaves.
    explain(state.board.problem ?? (state.detail.visible ? null : state.shown.problem))
    explainSaid(state.board.value?.notes ?? [])
    explainArchive(state.archiveProblem)

    if (state.tab === "wiki") {
      wikiView?.render(state, index)
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
          : describe(refused, copy, doc)?.parts.map((part) => (typeof part === "string" ? part : part.textContent ?? "")).join(""),
      })
      crumbs.render(state, index)
      mainViews[state.tab].render(state, index)
      if (state.detail.visible) detailView.render(state, index)
    }
    putBack()
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
    // The wiki pane lays itself out from the width it is given when it renders (view/wiki.ts mountWiki): a narrow
    // panel stacks its list over the page.
    if (current.tab === "wiki") return wikiView?.render(current, options.index(current))
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
    say(text, problem = false) {
      result.textContent = text
      result.className = problem ? "moai-result m-banner m-banner-danger" : "moai-result"
    },
    slots: { toolbar, actions },
    setWidth,
    layout,
    panes: () => ({ list: mainViews.list, board: mainViews.board, detail: detailView, wiki: wikiView }),
    focusPane,
    focusTabs,
    toolbar: bar,
    destroy() {
      unsubscribe()
      observer?.disconnect()
      win?.removeEventListener("resize", onResize)
      split.destroy()
      mainViews.list.destroy?.()
      mainViews.board.destroy?.()
      detailView.destroy?.()
      wikiView?.destroy?.()
      bar.destroy()
    },
  }
}
