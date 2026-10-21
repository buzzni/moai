// Search / filter / sort toolbar of the list and board. State and reducer are pure (testable without the
// store, which keeps the state: store.ts toolbarOf); createToolbar() builds the DOM once through the shared
// components and update() brings it to a state, so the inputs are never rebuilt under the caret or an IME
// composition. Every user act is reported as an action. What moai found and why it refused a filter come in with
// update: the toolbar judges nothing (data/query.ts).

import {
  SORT_KEYS,
  hintsAt,
  items,
  sortBadge,
  sortEdit,
  type Hint,
  type HintContext,
  type Slot,
  type Sort,
  type SortEdit,
  type SortKey,
} from "../data/query.ts"
import type { Row } from "../data/index.ts"
import { copyFor, type HiddenReason, type Language, type ToolbarCopy } from "../i18n/index.ts"
import { button, chip, el, icon, iconButton, menu, searchBox, textInput } from "./components.ts"
import { kbdHint } from "./help.ts"

// ---- strings: the i18n catalogue's toolbar group (i18n/index.ts) ---------------------------------------

export type { HiddenReason, ToolbarCopy }

// ---- state ----------------------------------------------------------------------------------------------

export interface ToolbarState {
  searchOpen: boolean
  searchText: string
  /** The filter as typed. */
  filterText: string
  /** The filter moai is asked: Enter applies the typed text, a chip removal at once (design §4.5). moai judges it. */
  filterApplied: string
  /** Caret of the filter input (UTF-16 index); the hints follow it. */
  cursor: number
  /** The hint dropdown is shown. */
  hintsOpen: boolean
  /** Highlighted hint, -1 for none. */
  hintIndex: number
  sortOpen: boolean
  sort: Sort
}

export const initialToolbar = (): ToolbarState => ({
  searchOpen: false,
  searchText: "",
  filterText: "",
  filterApplied: "",
  cursor: 0,
  hintsOpen: false,
  hintIndex: -1,
  sortOpen: false,
  sort: { fields: [{ key: "priority", dir: "asc" }], reversed: false },
})

export type ToolbarAction =
  | { type: "search-open" }
  | { type: "search-close" }
  | { type: "search-text"; text: string }
  | { type: "filter-text"; text: string; cursor: number }
  | { type: "filter-cursor"; cursor: number }
  | { type: "filter-apply" }
  | { type: "chip-remove"; index: number }
  | { type: "hints-open" }
  | { type: "hints-close" }
  | { type: "hint-move"; delta: number; count: number }
  | { type: "hint-accept"; slot: Slot; hint: Hint }
  | { type: "sort-toggle" }
  | { type: "sort-edit"; key: SortKey; how: SortEdit }
  | { type: "sort-set"; sort: Sort }

const isSpaceChar = (c: string | undefined): boolean => c !== undefined && /\s/.test(c)

/**
 * The text and caret after a hint is chosen: a key gets its `=`, a value replaces the piece being typed.
 * {@link slotAt} ends the slot at the caret; the piece goes on to its end — the rest of the key word (and the
 * `=` already after it, which is not doubled), or the rest of the value up to a comma, a space or the closing
 * quote — so a caret inside a word does not leave the word's tail behind it.
 */
export function acceptHint(text: string, slot: Slot, hint: Hint): { text: string; cursor: number } {
  if (slot.key === undefined) {
    let to = slot.to
    while (to < text.length && text[to] !== "=" && !isSpaceChar(text[to])) to++
    if (text[to] === "=") to++
    const insert = `${hint.value}=`
    return { text: text.slice(0, slot.from) + insert + text.slice(to), cursor: slot.from + insert.length }
  }
  // The quote that opens this value, when it has one (items() records it; slotAt starts the value inside it).
  const quote = items(text).find((item) => item.quote !== undefined && item.quote.open < slot.from && slot.from <= item.end)?.quote
  let to = slot.to
  if (quote?.close !== undefined) {
    // A caret past the closing quote still replaces only what is inside it.
    to = Math.min(to, quote.close)
    while (to < quote.close && text[to] !== ",") to++
  } else if (quote) {
    // An open quote runs to the end of the line, over the items typed after it: the piece ends at its word.
    while (to < text.length && text[to] !== "," && !isSpaceChar(text[to])) to++
  } else {
    while (to < text.length && text[to] !== "," && !isSpaceChar(text[to])) to++
  }
  let insert = hint.value
  // A value with a space is kept whole (and the hints stay on it) only in quotes, and a quote must open the
  // value. A comma is "or" even inside quotes, as in moai, so quoting does not save it.
  if (!quote && /\s/.test(insert) && text[slot.from - 1] === "=") insert = `"${insert}"`
  // A quote left open would swallow every item typed after it: the chosen value closes it.
  if (quote && quote.close === undefined) insert += text[quote.open]
  return { text: text.slice(0, slot.from) + insert + text.slice(to), cursor: slot.from + insert.length }
}

/** The filter text without its `index`-th item (the item's span, up to where the next one starts). */
export function removeItem(text: string, index: number): string {
  const list = items(text)
  const item = list[index]
  if (!item) return text
  const next = list[index + 1]
  const head = text.slice(0, item.start).trimEnd()
  const tail = text.slice(next ? next.start : text.length).trimStart()
  return head && tail ? `${head} ${tail}` : head + tail
}

export function toolbarReduce(state: ToolbarState, action: ToolbarAction): ToolbarState {
  switch (action.type) {
    case "search-open":
      return { ...state, searchOpen: true }
    case "search-close":
      return { ...state, searchOpen: false, searchText: "" }
    case "search-text":
      return { ...state, searchOpen: true, searchText: action.text }
    case "filter-text":
      return { ...state, filterText: action.text, cursor: clamp(action.cursor, action.text.length), hintsOpen: true, hintIndex: -1 }
    case "filter-cursor":
      return { ...state, cursor: clamp(action.cursor, state.filterText.length), hintIndex: -1 }
    case "filter-apply":
      // moai judges the text: one it refuses keeps the rows the last answer left, and its words stand under the input.
      return { ...state, filterApplied: state.filterText, hintsOpen: false, hintIndex: -1 }
    case "chip-remove": {
      const text = removeItem(state.filterText, action.index)
      if (text === state.filterText) return state
      // A chip applies at once (design §4.5).
      return { ...state, filterText: text, filterApplied: text, cursor: text.length, hintIndex: -1 }
    }
    case "hints-open":
      return { ...state, hintsOpen: true }
    case "hints-close":
      return { ...state, hintsOpen: false, hintIndex: -1 }
    case "hint-move": {
      if (action.count <= 0) return { ...state, hintIndex: -1 }
      const from = state.hintIndex < 0 ? (action.delta > 0 ? -1 : 0) : state.hintIndex
      return { ...state, hintsOpen: true, hintIndex: (from + action.delta + action.count) % action.count }
    }
    case "hint-accept": {
      const done = acceptHint(state.filterText, action.slot, action.hint)
      return { ...state, filterText: done.text, cursor: done.cursor, hintIndex: -1 }
    }
    case "sort-toggle":
      return { ...state, sortOpen: !state.sortOpen }
    case "sort-edit":
      return { ...state, sort: sortEdit(state.sort, action.key, action.how) }
    case "sort-set":
      return { ...state, sort: action.sort }
  }
}

const clamp = (n: number, max: number): number => Math.max(0, Math.min(n, max))

// ---- helpers over the data ------------------------------------------------------------------------------

/** `[from, to)` ranges of `q` in `text`, case-folded, non-overlapping; none when `q` is blank. */
export function highlightRanges(text: string, q: string): Array<[number, number]> {
  const needle = q.trim().toLowerCase()
  if (needle === "") return []
  const hay = text.toLowerCase()
  const out: Array<[number, number]> = []
  for (let at = hay.indexOf(needle); at >= 0; at = hay.indexOf(needle, at + needle.length)) out.push([at, at + needle.length])
  return out
}

/** `text` as nodes with the matched ranges wrapped in <mark>. */
export function highlight(doc: Document, text: string, q: string): Array<Node | string> {
  const out: Array<Node | string> = []
  let last = 0
  for (const [from, to] of highlightRanges(text, q)) {
    if (from > last) out.push(text.slice(last, from))
    out.push(el(doc, "mark", { class: "m-mark" }, text.slice(from, to)))
    last = to
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}

/**
 * How many of the rows the search found only the search shows, by the reason the list would hide them. `found` is
 * what moai found (store.ts Matched); `hiddenBy` is the page's own view toggles (view/list-model.ts narrow). Without
 * a search, or without `hiddenBy`, nothing is counted.
 */
export function searchView<T extends Row>(
  found: readonly T[],
  state: Pick<ToolbarState, "searchText">,
  hiddenBy?: (row: T) => HiddenReason | undefined,
): { found: T[]; hidden: Partial<Record<HiddenReason, number>> } {
  const hidden: Partial<Record<HiddenReason, number>> = {}
  if (state.searchText.trim() === "" || !hiddenBy) return { found: [...found], hidden }
  for (const row of found) {
    const why = hiddenBy(row)
    if (why) hidden[why] = (hidden[why] ?? 0) + 1
  }
  return { found: [...found], hidden }
}

// ---- render ---------------------------------------------------------------------------------------------

/** What the toolbar reads besides its state: the rows it hints from, what moai found, and why it refused a filter. */
export interface ToolbarData {
  rows: readonly Row[]
  ctx?: HintContext
  /** The rows moai found for the filter and the search on screen (empty while nothing is asked). */
  found?: readonly Row[]
  /** Why the list would hide a row the search shows ({@link searchView}); the hidden note counts these. */
  hiddenBy?: (row: Row) => HiddenReason | undefined
  /** moai's words for a filter it refused (`bad_filter`); the input is marked invalid while it stands. */
  problem?: string
}

export interface ToolbarDeps extends ToolbarData {
  doc: Document
  /** The words; `copy` wins over `lang`, Korean when neither is given. */
  copy?: ToolbarCopy
  lang?: Language
}

export interface ToolbarHandlers {
  /** Every user act, as a reducer action. The caller reduces, stores and calls update(). */
  dispatch: (action: ToolbarAction) => void
}

export interface Toolbar {
  el: HTMLDivElement
  searchInput: HTMLInputElement
  filterInput: HTMLInputElement
  /** Rows moai found and the hidden-row counts behind the note. */
  found: Row[]
  hidden: Partial<Record<HiddenReason, number>>
}

export interface LiveToolbar extends Toolbar {
  /** Bring the DOM to `state`. Inputs are written only when their text differs, and never while an IME composes. */
  update(state: ToolbarState, data: ToolbarData): void
  /** How many suggestions the filter has for its caret, whether the list is shown or not. */
  readonly hintCount: number
  destroy(): void
}

/** One state drawn once: a toolbar built and brought to `state` (tests, and a caller that redraws whole). */
export function renderToolbar(state: ToolbarState, deps: ToolbarDeps, handlers: ToolbarHandlers): Toolbar {
  const toolbar = createToolbar(deps, handlers)
  toolbar.update(state, deps)
  return toolbar
}

export function createToolbar(deps: Pick<ToolbarDeps, "doc" | "copy" | "lang">, handlers: ToolbarHandlers): LiveToolbar {
  const { doc } = deps
  const t = deps.copy ?? copyFor(deps.lang ?? "ko").toolbar
  const { dispatch } = handlers
  let state: ToolbarState = initialToolbar()
  let data: ToolbarData = { rows: [] }
  let slot: Slot = { partial: "", from: 0, to: 0 }
  let hints: Hint[] = []
  const root = el(doc, "div", { class: "m-toolbar-stack", "data-toolbar": "" })

  /** An input's IME composition: while it runs the input's text is the IME's, and is not written over. */
  const composing = new WeakSet<HTMLInputElement>()
  const trackComposition = (input: HTMLInputElement, report: () => void) => {
    input.addEventListener("compositionstart", () => composing.add(input))
    input.addEventListener("compositionend", () => {
      composing.delete(input)
      // Some engines send the last input event before compositionend: report the committed text once more.
      report()
    })
  }
  const write = (input: HTMLInputElement, value: string, caret?: number) => {
    if (composing.has(input) || input.value === value) return
    input.value = value
    if (caret !== undefined && doc.activeElement === input) input.setSelectionRange(caret, caret)
  }

  // Search: opened by `/` (bound by the caller) or the search button, applies as you type. moai looks in titles,
  // bodies and notes (`show --grep`); it has no scope to pick.
  const searchRow = el(doc, "div", { class: "m-toolbar", role: "search" })
  const box = searchBox(doc, { placeholder: t.searchPlaceholder, label: t.search, hint: "/" })
  const reportSearch = () => dispatch({ type: "search-text", text: box.input.value })
  box.input.addEventListener("input", reportSearch)
  trackComposition(box.input, reportSearch)
  box.input.addEventListener("keydown", (event) => {
    if (event.isComposing) return
    if (event.key === "Escape") dispatch({ type: "search-close" })
  })
  searchRow.append(box.el)

  // Filter text with the hint dropdown under it.
  const filterWrap = el(doc, "div", { class: "m-filter", style: "position:relative;flex:1 1 200px" })
  const filterInput = textInput(doc, { label: t.filter, placeholder: t.filterPlaceholder })
  const reportFilter = () => dispatch({ type: "filter-text", text: filterInput.value, cursor: filterInput.selectionStart ?? filterInput.value.length })
  filterInput.addEventListener("input", reportFilter)
  trackComposition(filterInput, reportFilter)
  filterInput.addEventListener("keyup", (event) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return
    dispatch({ type: "filter-cursor", cursor: filterInput.selectionStart ?? 0 })
  })
  // A click moves the caret too; without this the hints stay on the item the keyboard last left.
  filterInput.addEventListener("click", () => dispatch({ type: "filter-cursor", cursor: filterInput.selectionStart ?? 0 }))
  // The suggestions belong to the caret: once the filter loses the focus they close. A click on one does not blur
  // the input (their mousedown is prevented below), so it is still taken. When the whole window loses the focus the
  // input stays the document's active element and has it back on return, so the suggestions stay open with it.
  filterInput.addEventListener("blur", () => {
    if (doc.activeElement === filterInput) return
    if (state.hintsOpen) dispatch({ type: "hints-close" })
  })
  filterInput.addEventListener("keydown", (event) => {
    if (event.isComposing) return
    const open = state.hintsOpen && hints.length > 0
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      dispatch({ type: "hint-move", delta: event.key === "ArrowDown" ? 1 : -1, count: hints.length })
    } else if ((event.key === "Enter" || (event.key === "Tab" && !event.shiftKey)) && open && state.hintIndex >= 0) {
      // Shift-Tab is not taken here: it moves back through the suggestions (the key router's prompt.prev).
      event.preventDefault()
      dispatch({ type: "hint-accept", slot, hint: hints[state.hintIndex]! })
    } else if (event.key === "Enter") {
      event.preventDefault()
      dispatch({ type: "filter-apply" })
    }
    // Esc is the key router's alone (keys/wiring.ts prompt.cancel, keybindings.md 3.7): it closes the suggestions,
    // puts the applied filter back and leaves the input, all at once.
  })
  filterWrap.append(filterInput)
  let hintList: HTMLElement | null = null
  let hintsDrawn = ""

  // Sort badge and its edit popover.
  const sortWrap = el(doc, "div", { class: "m-sort", style: "position:relative" })
  const sortButton = button(doc, { label: t.sort, icon: "arrow-up-down", small: true, onClick: () => dispatch({ type: "sort-toggle" }) })
  sortWrap.append(sortButton)
  let sortPop: HTMLElement | null = null
  let sortDrawn = ""

  const openSearch = iconButton(doc, {
    icon: "search",
    label: t.search,
    onClick: () => {
      dispatch({ type: "search-open" })
      box.input.focus()
    },
  })
  openSearch.setAttribute("aria-keyshortcuts", "/")
  // The key that opens the search, next to its button (keys/table.ts `/`); the open search box carries its own.
  const searchShortcut = kbdHint(doc, "/")
  searchShortcut.setAttribute("aria-hidden", "true")
  sortButton.setAttribute("aria-keyshortcuts", "Space s e")
  const bar = el(doc, "div", { class: "m-toolbar" }, icon(doc, "filter", 14), filterWrap, sortWrap, openSearch, searchShortcut)

  // Chips of the parsed filter (comma is OR inside one chip, a repeated key is AND between chips).
  const chips = el(doc, "div", { class: "m-chips", style: "display:flex;flex-wrap:wrap;gap:4px;padding:0 8px" })
  // Removing a chip redraws the chips: the focus stays where it was instead of dropping to <body> with its x button.
  chips.addEventListener("mousedown", (event) => event.preventDefault())
  let chipsDrawn: string | null = null
  const error = el(doc, "div", { class: "m-banner m-banner-danger", role: "alert", hidden: true })
  const note = el(doc, "div", { class: "m-hidden-note", role: "status", hidden: true })
  root.append(searchRow, bar, chips, error, note)

  // What the last update computed from: hints and the search are read again only when their inputs change.
  let hintsFor: unknown[] = []
  let searchFor: unknown[] = []
  const same = (a: unknown[], b: unknown[]) => a.length === b.length && a.every((value, n) => value === b[n])
  const view: { found: Row[]; hidden: Partial<Record<HiddenReason, number>> } = { found: [], hidden: {} }

  function update(next: ToolbarState, nextData: ToolbarData) {
    state = next
    data = nextData
    const problem = data.problem ?? ""

    searchRow.dataset.open = String(state.searchOpen)
    searchRow.hidden = !state.searchOpen
    openSearch.hidden = state.searchOpen
    searchShortcut.hidden = state.searchOpen
    write(box.input, state.searchText)

    write(filterInput, state.filterText, state.cursor)
    if (problem === "") filterInput.removeAttribute("aria-invalid")
    else filterInput.setAttribute("aria-invalid", "true")
    const hintKey = [data.rows, state.filterText, state.cursor, data.ctx]
    if (!same(hintKey, hintsFor)) {
      hintsFor = hintKey
      ;({ slot, hints } = hintsAt(data.rows, state.filterText, state.cursor, data.ctx))
    }
    const open = state.hintsOpen && hints.length > 0
    const drawnKey = open ? JSON.stringify([hints, state.hintIndex]) : ""
    if (drawnKey !== hintsDrawn) {
      hintsDrawn = drawnKey
      hintList?.remove()
      hintList = null
      if (open) {
        const shown = hints
        const list = menu(
          doc,
          shown.map((hint, index) => ({ id: String(index), label: hint.label ? `${hint.value}  ${hint.label}` : hint.value, hint: hint.count ? String(hint.count) : undefined })),
          (id) => dispatch({ type: "hint-accept", slot, hint: shown[Number(id)]! }),
        )
        list.setAttribute("aria-label", t.hints)
        list.classList.add("m-hints")
        // A click must not take the caret out of the filter: the list is rebuilt under the clicked item, and the focus
        // would drop to <body>, where what is typed next runs as shortcuts (as view/menu.ts does for its items).
        list.addEventListener("mousedown", (event) => event.preventDefault())
        list.style.cssText = "position:absolute;top:100%;left:0;right:0;z-index:20"
        list.querySelectorAll<HTMLElement>(".m-menu-item").forEach((node, index) => {
          if (index === state.hintIndex) node.setAttribute("aria-current", "true")
        })
        filterWrap.append(list)
        hintList = list
      }
    }

    const badge = sortBadge(state.sort)
    const label = badge.parts.map((part) => `${t.sortKeys[part.key]}${part.dir === "asc" ? "↑" : "↓"}`).join(" ")
    const sortLabel = `${t.sort}: ${label}${badge.reversed ? ` (${t.reversed})` : ""}`
    if (sortButton.getAttribute("aria-label") !== sortLabel) {
      sortButton.setAttribute("aria-label", sortLabel)
      sortButton.title = sortLabel
      const text = sortButton.querySelector(".m-btn-label")
      if (text) text.textContent = sortLabel
    }
    sortButton.setAttribute("aria-expanded", String(state.sortOpen))
    sortButton.dataset.sort = badge.text
    const popKey = state.sortOpen ? JSON.stringify(state.sort) : ""
    if (popKey !== sortDrawn) {
      sortDrawn = popKey
      sortPop?.remove()
      sortPop = null
      if (state.sortOpen) {
        const pop = el(doc, "div", { class: "m-menu m-sort-pop", role: "dialog", "aria-label": t.sortEdit, style: "position:absolute;top:100%;right:0;z-index:20" })
        for (const key of SORT_KEYS) {
          const field = state.sort.fields.find((one) => one.key === key)
          const rowEl = el(doc, "div", { class: "m-menu-item", "data-key": key, style: "gap:4px" })
          rowEl.append(el(doc, "span", { style: "flex:1" }, `${t.sortKeys[key]}${field ? (field.dir === "asc" ? " ↑" : " ↓") : ""}`))
          for (const how of ["replace", "append", "flip"] as const) {
            const b = button(doc, { label: t[how], variant: "ghost", small: true, onClick: () => dispatch({ type: "sort-edit", key, how }) })
            b.dataset.how = how
            rowEl.append(b)
          }
          pop.append(rowEl)
        }
        // Said plainly until a settings storage keeps the default; keys/wiring.ts hides it where the default persists.
        pop.append(el(doc, "div", { class: "m-sort-note", "data-sort-note": "default" }, t.sortDefaultSessionNote))
        sortWrap.append(pop)
        sortPop = pop
      }
    }

    const chipKey = JSON.stringify([state.filterText, problem === ""])
    if (chipKey !== chipsDrawn) {
      chipsDrawn = chipKey
      const made: Node[] = []
      items(state.filterText).forEach((item, index) => {
        if (item.text.trim() === "") return
        made.push(chip(doc, { label: item.text, tone: problem === "" ? "accent" : "neutral", removeLabel: t.removeChip, onRemove: () => dispatch({ type: "chip-remove", index }) }))
      })
      chips.replaceChildren(...made)
      chips.hidden = made.length === 0
    }

    if (error.textContent !== problem) error.textContent = problem
    error.hidden = problem === ""

    // Search result and the note on rows only the search shows.
    const searchKey = [data.found, state.searchText, data.hiddenBy]
    if (!same(searchKey, searchFor)) {
      searchFor = searchKey
      Object.assign(view, searchView(data.found ?? [], state, data.hiddenBy))
    }
    const reasons = (Object.keys(view.hidden) as HiddenReason[]).map((reason) => `${t.hiddenReasons[reason]} ${view.hidden[reason]}`)
    const total = Object.values(view.hidden).reduce((sum, n) => sum + (n ?? 0), 0)
    const said = total ? `${t.hidden(total)}: ${reasons.join(", ")}` : ""
    // A live region: written only when it says something new.
    if (note.textContent !== said) note.textContent = said
    note.hidden = total === 0
  }

  return {
    el: root,
    searchInput: box.input,
    filterInput,
    get found() {
      return view.found
    },
    get hidden() {
      return view.hidden
    },
    get hintCount() {
      return hints.length
    },
    update,
    destroy() {
      root.remove()
    },
  }
}
