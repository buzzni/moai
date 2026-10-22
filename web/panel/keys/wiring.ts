// The key router wired to the panel (se-jwl3): one keydown listener on the document turns keys into the key
// table's action ids (router.ts, table.ts), and this file turns each id into a store action or a call on the pane
// that has the focus (view/types.ts PaneView.command, the wiki's handleKey). It also mounts the SPC menu, the
// shortcut help and the focus hint, and keeps the menu drawn from the router's layer and the store.
//
// One owner per key: the list, board and wiki listen for no keys of their own; components that do (the tab
// strip's arrows, the split resizer, the toolbar inputs' Enter and arrows) mark the event handled and the router
// leaves it (attachKeyRouter). The mode comes from the store and the focus: `sorter` while the sort editor is
// open, `prompt` while a search or filter input has the caret, `wiki` on the wiki tab, `browse` otherwise.

import { sortDefault, type SortDefault } from "../data/memory.ts"
import { entryOf, numberedNames, type ProjectEntry } from "../data/projects.ts"
import { SORT_KEYS, sortEdit, type Sort, type SortKey } from "../data/query.ts"
import type { Copy } from "../i18n/index.ts"
import type { ColumnName, State, Store } from "../store.ts"
import { statusKeys } from "../view/board-groups.ts"
import { createHelp, focusHint, type HelpView } from "../view/help.ts"
import { SHOW_ALL } from "../view/list-model.ts"
import { createMenu, type MenuView } from "../view/menu.ts"
import { effectivePosition } from "../view/responsive.ts"
import type { Shell } from "../view/shell.ts"
import type { ToolbarAction } from "../view/toolbar.ts"
import type { PaneView } from "../view/types.ts"
import { attachKeyRouter, createKeyRouter, detectPlatform, type DispatchedAction, type KeyRouter } from "./router.ts"
import type { CellField, KeyContext, Mode, Platform } from "./table.ts"

/** What the context needs besides the store: things only the DOM knows. */
export interface ContextEnv {
  mode: Mode
  /** The panel's width (the 640px rule forces the detail to the bottom). */
  width: number
  /** The wiki pane that has the focus. */
  wikiPane?: "list" | "body"
  /** The body on screen (the wiki page or the detail) is raw markdown. */
  raw?: boolean
  /** Status column names as the menu labels them (SPC v 1~9); the status itself when left out. */
  statusName?: (status: string) => string
}

/** The key table's view of the panel (keys/table.ts KeyContext), read from the store. Pure. */
export function keyContext(state: State, env: ContextEnv): KeyContext {
  const statuses = statusKeys(state.board.value?.rows ?? [], state.boot?.statuses)
  const hidden = new Set(state.view.hiddenStatuses)
  return {
    mode: env.mode,
    tab: state.tab,
    focus: env.mode === "wiki" ? (env.wikiPane === "body" ? "page" : "list") : state.focus,
    detail: state.detail.visible,
    detailAt: effectivePosition(env.width, state.detail.position),
    detailAtLocked: effectivePosition(env.width, "left") !== "left",
    raw: env.raw ?? false,
    // view.showArchived lets archived rows in (list-model.ts FILTER_FLAGS); the table asks whether they are hidden.
    hidden: { deferred: state.view.hideDeferred, backlog: state.view.hideBacklog, archived: !state.view.showArchived, worktree: !state.view.worktree, columns: statuses.map((s) => hidden.has(s)) },
    columns: statuses.map((s) => env.statusName?.(s) ?? s),
    // Every field, `id` too (the sort editor and the toolbar offer it): the menu's state on `SPC s p` says "the order
    // is this field alone, pressing flips it", which a dropped field would make untrue.
    sort: state.sort.fields.map((f) => ({ field: f.key, desc: f.dir === "desc" })),
    cells: { ...state.columns },
    projects: numberedNames(state.projects),
  }
}

/**
 * Table actions that do nothing yet, and why. Their keys stay in the table (the help lists them as usable), so a
 * dispatch that reaches one only says so.
 */
export const NOT_WIRED: Readonly<Record<string, string>> = {}

export interface KeyWiringOptions {
  document: Document
  store: Store
  shell: Shell
  copy: Copy
  /** Defaults to the navigator's (detectPlatform). */
  platform?: Platform
  /**
   * Opens another project (`1`–`9`, `SPC p 1`–`9`): main.ts navigates to it (data/projects.ts projectHref). The project
   * on screen is not opened again.
   */
  openProject?: (entry: ProjectEntry) => void
  /**
   * Where `D` saves the default sort; defaults to the panel's shared one. An emptied order returns to the store's
   * `defaultSort`, which `D` sets and data/memory.ts remember() reads from the same storage at boot: pass both the same one.
   */
  sortDefault?: SortDefault
}

export interface KeyWiring {
  router: KeyRouter
  menu: MenuView
  help: HelpView
  hint: HTMLElement
  /** The context the router reads now. */
  context(): KeyContext
  /** Every action id this wiring dispatches (the key table's ids; tests hold them to the table). */
  actions: string[]
  destroy(): void
}

/** Runs an action; `false` says it did not use the key, which then does what the browser does with it. */
type Handler = (action: DispatchedAction) => boolean | void

export function wireKeys(options: KeyWiringOptions): KeyWiring {
  const { document: doc, store, shell, copy } = options
  const win = doc.defaultView
  const platform = options.platform ?? detectPlatform(win?.navigator)
  const bar = shell.toolbar

  /** The keydown being routed now: the wiki's handleKey takes the event itself. Null for a menu click. */
  let current: KeyboardEvent | null = null

  const panes = () => shell.panes()
  const main = (): PaneView => (store.getState().tab === "board" ? panes().board : panes().list)
  const focused = (): PaneView => (store.getState().focus === "detail" ? panes().detail : main())
  const wikiPaneEl = () => shell.el.querySelector<HTMLElement>(".m-pane-wiki [data-pane]")

  function isPromptInput(node: Element | null): boolean {
    if (!node || node.tagName !== "INPUT") return false
    return node === bar.searchInput || node === bar.filterInput || node.closest(".m-wiki-search") !== null
  }

  function modeOf(state: State): Mode {
    if (state.tab !== "wiki" && state.toolbar.sortOpen) return "sorter"
    if (isPromptInput(doc.activeElement)) return "prompt"
    return state.tab === "wiki" ? "wiki" : "browse"
  }

  function context(): KeyContext {
    const state = store.getState()
    const mode = modeOf(state)
    const onWiki = state.tab === "wiki"
    return keyContext(state, {
      mode,
      width: shell.layout().width,
      wikiPane: wikiPaneEl()?.dataset.pane === "body" ? "body" : "list",
      raw: (onWiki ? panes().wiki?.raw?.() : panes().detail.raw?.()) ?? false,
      statusName: (status) => (copy.stats as Record<string, string>)[status] ?? status,
    })
  }

  const toolbar = (action: ToolbarAction) => store.dispatch({ type: "toolbar", action })
  /** Back to the pane the keys were in, out of an input or the sort editor. */
  const backToPane = () => shell.focusPane(store.getState().focus)

  // ── search and filter prompts ──
  /**
   * The cursor when `/` opened the search: Esc in the search puts it back (keybindings.md 3.7). It is forgotten as
   * soon as the caret leaves the search (Enter, a click elsewhere), so a later Esc in a search opened some other way
   * (its button, a click into it) does not take the cursor back to a row left long ago.
   */
  let promptFrom: string | null | undefined
  const forgetPrompt = () => {
    // The whole window losing the focus blurs the search too, but leaves it the active element: the caret has not
    // left it, and it is back there on return (as the filter's suggestions stay open then, toolbar.ts).
    if (doc.activeElement === bar.searchInput) return
    promptFrom = undefined
  }
  bar.searchInput.addEventListener("blur", forgetPrompt)

  function openSearch() {
    toolbar({ type: "search-open" })
    bar.searchInput.focus()
    promptFrom = store.getState().cursor
  }

  /** Change the focused input's text as typing would, so its own listener (toolbar, wiki) reports it. */
  function editInput(edit: (text: string, caret: number) => { text: string; caret: number }) {
    const input = doc.activeElement as HTMLInputElement | null
    if (!input || !isPromptInput(input)) return
    const done = edit(input.value, input.selectionStart ?? input.value.length)
    input.value = done.text
    input.setSelectionRange(done.caret, done.caret)
    input.dispatchEvent(new (win?.Event ?? Event)("input", { bubbles: true }))
  }

  /** The suggestions the filter has for its caret, shown or not (Tab opens a closed list on its first one). */
  const hintCount = () => bar.hintCount
  /** On the wiki tab, where the only prompt is the wiki's own search. */
  const inWiki = () => store.getState().tab === "wiki"
  /**
   * A prompt key in the wiki's search goes to the wiki (its handleKey reads Esc, Enter and ↓ there). What it does not
   * read (Tab, S-Tab, ↑) is reported unused, so Tab moves the focus and ↑ the caret as in any input.
   */
  const wikiKey = (): boolean => (current ? panes().wiki?.handleKey?.(current) ?? false : false)

  // ── sort editor (sorter mode) ──
  let sorterAt = 0
  let sorterFrom: Sort | null = null
  const sortKey = (): SortKey => SORT_KEYS[sorterAt]!
  const defaults = options.sortDefault ?? sortDefault
  // An order emptied of every field is the reset: the store puts back the saved default (State.defaultSort).
  const setSort = (sort: Sort) => store.dispatch({ type: "sort", sort })

  function paintSorter() {
    for (const row of bar.el.querySelectorAll<HTMLElement>(".m-sort-pop .m-menu-item[data-key]")) {
      const on = row.dataset.key === sortKey()
      row.toggleAttribute("data-cursor", on)
      if (on) row.setAttribute("aria-current", "true")
      else row.removeAttribute("aria-current")
    }
    // The editor's note says the default `D` saves lasts for this session only (toolbar.ts): where it persists, as
    // `D` then says (sortDefaultSaved), the note would say what is not so.
    const note = bar.el.querySelector<HTMLElement>(".m-sort-pop [data-sort-note]")
    if (note) note.hidden = defaults.persistent
  }

  function moveField(delta: 1 | -1) {
    const sort = store.getState().sort
    const at = sort.fields.findIndex((f) => f.key === sortKey())
    const to = at + delta
    if (at < 0 || to < 0 || to >= sort.fields.length) return
    const fields = [...sort.fields]
    ;[fields[at], fields[to]] = [fields[to]!, fields[at]!]
    setSort({ ...sort, fields })
  }

  function closeSorter(keep: boolean) {
    if (!keep && sorterFrom) setSort(sorterFrom)
    sorterFrom = null
    if (store.getState().toolbar.sortOpen) toolbar({ type: "sort-toggle" })
    backToPane()
  }

  // ── focus moves ──
  function focusTo(side: "next" | "prev" | "left" | "right" | "up" | "down") {
    if (store.getState().tab === "wiki") {
      panes().wiki?.command?.(`focus.${side}`)
      return
    }
    const state = store.getState()
    if (!state.detail.visible) return
    const at = effectivePosition(shell.layout().width, state.detail.position)
    const toward: Record<typeof side, "list" | "detail"> = {
      next: state.focus === "list" ? "detail" : "list",
      prev: state.focus === "list" ? "detail" : "list",
      left: at === "left" ? "detail" : "list",
      right: "list",
      up: "list",
      down: at === "bottom" ? "detail" : "list",
    }
    shell.focusPane(toward[side])
  }

  const say = (text: string) => shell.say(text)
  const flip = (state: State, key: keyof Pick<State["view"], "hideDeferred" | "hideBacklog" | "showArchived" | "worktree">) =>
    store.dispatch({ type: "view", view: { [key]: !state.view[key] } })

  // ── the dispatch table: every action id of keys/table.ts ──
  const move: Handler = (a) => {
    if (store.getState().tab === "wiki") return wiki(a)
    focused().command?.(a.id)
  }
  const onMain: Handler = (a) => {
    main().command?.(a.id)
  }
  /** The wiki's own keys: its handleKey reads the event first (as the key was pressed), its command the id. */
  const wiki: Handler = (a) => {
    const pane = panes().wiki
    if (!pane) return
    if (current && pane.handleKey?.(current)) return
    if (pane.command?.(a.id)) return
    // An Esc the wiki did not use: the way out of the wiki, whose Tab cycles its links, is the tab bar.
    if (a.id === "wiki.clear") shell.focusTabs()
  }
  const tab = (to: State["tab"]): Handler => () => {
    store.dispatch({ type: "tab", tab: to })
    shell.focusPane("list")
  }
  const sortBy = (how: "replace" | "flip"): Handler => (a) => {
    const field = String(a.args?.field) as SortKey
    setSort(sortEdit(store.getState().sort, field, how))
  }

  // ── projects ──
  /** `1`–`9` and `SPC p 1`–`9`: the project with that number, unless it is the one on screen (the TUI stays put). */
  const jump: Handler = (a) => {
    const entry = entryOf(store.getState().projects, Number(a.args?.number))
    if (!entry || entry.current) return
    options.openProject?.(entry)
  }

  const handlers: Record<string, Handler> = {
    "move.down": move,
    "move.up": move,
    "move.top": move,
    "move.bottom": move,
    "move.halfDown": move,
    "move.halfUp": move,
    "move.pageDown": move,
    "move.pageUp": move,
    enter: onMain,
    leave: onMain,
    expand: onMain,
    collapse: onMain,
    expandAll: onMain,
    "focus.next": () => focusTo("next"),
    "focus.prev": () => focusTo("prev"),
    "focus.left": () => focusTo("left"),
    "focus.right": () => focusTo("right"),
    "focus.up": () => focusTo("up"),
    "focus.down": () => focusTo("down"),
    "search.open": openSearch,
    "filter.open": () => bar.filterInput.focus(),
    clear: () => {
      const state = store.getState()
      if (state.search.open || state.search.text) toolbar({ type: "search-close" })
      if (state.filter.text || state.filter.applied) store.dispatch({ type: "filter", text: "" })
    },
    "project.jump": jump,
    help: (a) => help.open(a.mode, context()),
    "tab.list": tab("list"),
    "tab.board": tab("board"),
    "tab.wiki": tab("wiki"),
    "view.deferred": () => flip(store.getState(), "hideDeferred"),
    "view.backlog": () => flip(store.getState(), "hideBacklog"),
    "view.archived": () => flip(store.getState(), "showArchived"),
    "view.worktree": () => flip(store.getState(), "worktree"),
    "view.all": () => store.dispatch({ type: "view", view: SHOW_ALL }),
    "view.column": (a) => {
      const state = store.getState()
      const status = statusKeys(state.board.value?.rows ?? [], state.boot?.statuses)[Number(a.args?.index)]
      if (status === undefined) return
      const hidden = state.view.hiddenStatuses
      store.dispatch({ type: "view", view: { hiddenStatuses: hidden.includes(status) ? hidden.filter((s) => s !== status) : [...hidden, status] } })
    },
    "view.detail": () => {
      const state = store.getState()
      store.dispatch({ type: "detail-visible", visible: !state.detail.visible })
      if (state.detail.visible && state.focus === "detail") shell.focusPane("list")
    },
    "view.raw": () => {
      if (store.getState().tab === "wiki") panes().wiki?.toggleRaw?.()
      else panes().detail.toggleRaw?.()
    },
    "sort.set": sortBy("replace"),
    // `P C U …`: to the end of the order, or its direction turned when it is in it already (query.ts sortFlip).
    "sort.add": sortBy("flip"),
    "sort.edit": () => {
      if (!store.getState().toolbar.sortOpen) toolbar({ type: "sort-toggle" })
    },
    "cell.toggle": (a) => {
      const column = String(a.args?.field) as CellField & ColumnName
      store.dispatch({ type: "column", column, on: !store.getState().columns[column] })
    },
    "option.detailAt": () => {
      const position = store.getState().detail.position === "left" ? "bottom" : "left"
      store.dispatch({ type: "detail-position", position })
    },
    "prompt.apply": () => {
      if (inWiki()) return wikiKey()
      // The filter's own Enter applies it (and handles the key), so only the search, which applies as it is typed,
      // gets here.
      promptFrom = undefined
      backToPane()
    },
    "prompt.cancel": () => {
      if (inWiki()) return wikiKey()
      // Read before the search closes: closing it takes the caret out of it, and that forgets the cursor.
      const from = promptFrom
      promptFrom = undefined
      if (doc.activeElement === bar.filterInput) {
        const applied = store.getState().filter.applied
        toolbar({ type: "filter-text", text: applied, cursor: applied.length })
        toolbar({ type: "hints-close" })
      } else {
        toolbar({ type: "search-close" })
        if (from !== undefined) store.dispatch({ type: "cursor", id: from })
      }
      backToPane()
    },
    "prompt.next": () => {
      if (inWiki()) return wikiKey()
      // The search has no scope to cycle (moai's `--grep` has none): Tab there moves the focus as anywhere.
      if (doc.activeElement !== bar.filterInput) return false
      toolbar({ type: "hint-move", delta: 1, count: hintCount() })
    },
    "prompt.prev": () => {
      if (inWiki()) return wikiKey()
      if (doc.activeElement !== bar.filterInput) return false
      toolbar({ type: "hint-move", delta: -1, count: hintCount() })
    },
    // The filter input moves through its suggestions itself and the search has none: outside the wiki the key is
    // not the router's, so the input (or the browser) keeps it.
    "prompt.up": () => (inWiki() ? wikiKey() : false),
    "prompt.down": () => (inWiki() ? wikiKey() : false),
    "prompt.clear": () => editInput(() => ({ text: "", caret: 0 })),
    "prompt.deleteWord": () =>
      editInput((text, caret) => {
        const head = text.slice(0, caret).replace(/\S*\s*$/, "")
        return { text: head + text.slice(caret), caret: head.length }
      }),
    "sorter.down": () => {
      sorterAt = Math.min(SORT_KEYS.length - 1, sorterAt + 1)
      paintSorter()
    },
    "sorter.up": () => {
      sorterAt = Math.max(0, sorterAt - 1)
      paintSorter()
    },
    "sorter.lower": () => moveField(1),
    "sorter.raise": () => moveField(-1),
    "sorter.toggle": () => {
      const sort = store.getState().sort
      const key = sortKey()
      setSort(sort.fields.some((f) => f.key === key) ? { ...sort, fields: sort.fields.filter((f) => f.key !== key) } : sortEdit(sort, key, "append"))
    },
    "sorter.flip": () => setSort(sortEdit(store.getState().sort, sortKey(), "flip")),
    "sorter.apply": () => closeSorter(true),
    "sorter.default": () => {
      defaults.set(store.getState().sort)
      store.dispatch({ type: "sort-default", sort: store.getState().sort })
      say(defaults.persistent ? copy.keys.sortDefaultSaved : copy.keys.sortDefaultSavedSession)
      // As in the TUI: D applies the order, saves it and leaves the editor (Esc after it must not undo it).
      closeSorter(true)
    },
    "sorter.cancel": () => closeSorter(false),
    "wiki.enter": wiki,
    "wiki.back": wiki,
    "wiki.page": wiki,
    "wiki.nextLink": wiki,
    "wiki.prevLink": wiki,
    "wiki.search": wiki,
    "wiki.clear": wiki,
  }

  // ── router, menu, help, focus hint ──
  const menu: MenuView = createMenu(doc, { press: (key) => void router.press(key), words: copy.table })
  const redraw = () => {
    const path = router.state().menu
    // A closed menu draws nothing from the context: it is not built (every row read for the status columns) on every
    // store change and every key while no menu is open.
    if (path === null && menu.el.hidden) return
    menu.update(path, context())
  }
  const router = createKeyRouter({
    platform,
    getContext: context,
    dispatch: (action) => {
      const used = handlers[action.id]?.(action)
      // A toggle the store does not hold (the raw body) changes the menu too: draw it after every action.
      redraw()
      return used
    },
    onChange: redraw,
  })
  const help = createHelp(doc, { platform, words: copy.table })
  const hint = focusHint(doc, copy.keys.focusHint)
  hint.addEventListener("click", () => doc.body.focus({ preventScroll: true }))
  doc.body.append(menu.el, help.el, hint)

  const showHint = () => {
    hint.hidden = doc.hasFocus()
  }
  showHint()
  win?.addEventListener("focus", showHint)
  win?.addEventListener("blur", showHint)
  doc.addEventListener("focusin", showHint)
  // A click that puts the caret in an input (prompt) changes the mode without a key: a menu or sequence held in the
  // old mode is dropped then, not left drawn with items that no longer run. Leaving an input needs no listener:
  // nothing is ever held in prompt mode (its keys are single keys), and the other modes follow the store (below).
  const settle = () => router.settle()
  doc.addEventListener("focusin", settle)

  const unsubscribe = store.subscribe((state, previous) => {
    if (state.toolbar.sortOpen && !previous.toolbar.sortOpen) {
      // The editor opens on the order's first field, and Esc puts the order it opened on back.
      sorterFrom = previous.sort
      sorterAt = Math.max(0, SORT_KEYS.indexOf(state.sort.fields[0]?.key ?? "priority"))
      // Opened by a click on the sort button, which keeps the focus: SPC and Enter would click that button again
      // (attachKeyRouter leaves them to a focused button) instead of putting a field in or applying the order.
      if (bar.el.contains(doc.activeElement)) backToPane()
    }
    if (!state.toolbar.sortOpen) sorterFrom = null
    paintSorter()
    // A tab switch or the sort editor opening is a change of mode: drop a menu held in the old one.
    router.settle()
    // A toggle run from the menu changes the store without a change of the menu's layer: draw it again.
    redraw()
  })

  // Keys are quiet while the help is open, even when the focus has left it. The help's guard stops every key pressed
  // outside it (Esc closing it there) and the overlay's own keydown takes Esc inside it, so what gets here is a key
  // inside the overlay (left to it: attachKeyRouter skips a modal's keys) or one of the host's combinations it lets
  // pass (copy, find: Cmd/Meta, and Ctrl off macOS; see help.ts guard). Those are swallowed here without
  // preventDefault, so the panel's shortcuts stay off and the browser still acts. The Esc branch below is a fallback
  // only: no Esc reaches it while the guard is attached.
  const detach = attachKeyRouter(doc, {
    ...router,
    handle(event) {
      current = (event.source as KeyboardEvent | undefined) ?? null
      try {
        return router.handle(event)
      } finally {
        current = null
      }
    },
  }, {
    intercept(event) {
      if (!help.isOpen()) return false
      if (event.key === "Escape" || event.key === "Esc") {
        event.preventDefault()
        help.close()
      }
      return true
    },
  })

  return {
    router,
    menu,
    help,
    hint,
    context,
    actions: Object.keys(handlers),
    destroy() {
      detach()
      unsubscribe()
      bar.searchInput.removeEventListener("blur", forgetPrompt)
      win?.removeEventListener("focus", showHint)
      win?.removeEventListener("blur", showHint)
      doc.removeEventListener("focusin", showHint)
      doc.removeEventListener("focusin", settle)
      help.close()
      menu.el.remove()
      help.el.remove()
      hint.remove()
    },
  }
}
