// The list tab (design §4.1, §4.7, §4.8; E3 se-c10x): the tree list in the main pane. Folders are entered
// (Enter, double click) and left (Backspace, `..`), unfolded in place (→/←, the ▸ button), rows carry the
// columns the user turned on that still fit, the order is moai's (the sort keys ask for it), and the view toggles hide rows
// with a note saying how many. The pure half lives in view/list-model.ts.
//
// Thousands of rows stay fast: every line has the same height, so only the lines in and near the viewport
// of the pane root's own scroll exist in the DOM (absolutely placed in a body as tall as all of them). The
// keyboard focus stays on the list itself and points at the cursor row with aria-activedescendant, so
// redrawing the window never drops it. The key router (panel/keys/) owns every key; it calls command() with
// the action a key stands for (moves in drawn order, going in and out, unfolding), so no key runs twice.

import { startsFolded, pathKey, priorityOf, type IssueIndex, type Keep, type Path } from "../data/index.ts"
import { sortEdit, type SortKey } from "../data/query.ts"
import { assigneeOf } from "../data/people.ts"
import { isUnread } from "../data/read.ts"
import type { State } from "../store.ts"
import { button, el, emptyState, icon } from "./components.ts"
import { segLabel } from "./crumbs.ts"
import {
  columnSet, firstKey, folderOf, HEAD_SORT, hiddenUnder, hidesAnything, listLines, moveKey, narrow, parentLine, segKey, shortDate, SHOW_ALL,
  statusGlyph, statusOf, twigParts, TWIG_TEXT, windowRange, revealOffset, type ColumnSet, type ListLine, type RightColumn,
} from "./list-model.ts"
import { newChip } from "./unread.ts"
import { firstStatus, glint, spins } from "./spin.ts"
import { branchMark, branchOf } from "./branch.ts"
import { highlight } from "./toolbar.ts"
import type { MountPane, PaneView } from "./types.ts"
import { fill } from "../i18n/index.ts"

/** Every line is this tall (list.css --moai-row-h); the virtual scroll depends on it. 24, not the app's 28: at 28 the
 * branches of the tree stood apart (se-gowr.4x9). */
export const ROW_HEIGHT = 24
/** The sticky name line (style: .m-row-head). */
export const HEAD_HEIGHT = 24
/** The visible height assumed while the pane has none (hidden, or not laid out yet). */
const FALLBACK_VIEWPORT = 720

interface Model {
  index: IssueIndex
  /** What the lines were computed from, to skip the work when only the cursor or the detail changed. */
  key: string
  keep: Keep
  /** Rows only the search shows (the view hides them): drawn dim and marked hidden. */
  revealed: Keep
  /** What moai found, as the lines were drawn from it. */
  matched: State["matched"]
  lines: ListLine[]
  at: Map<string, number>
  hidden: number
  /** Width of the id column, in ch. */
  idWidth: number
}

let mounted = 0

export const mountList: MountPane = (root, ctx): PaneView => {
  const { document: doc, copy, intents } = ctx
  const uid = `moai-list-${++mounted}`
  const words = copy.list

  const noteText = el(doc, "span", {})
  const note = el(doc, "div", { class: "moai-list-note", role: "status", hidden: true }, noteText)
  const showAll = button(doc, { label: words.showAll, variant: "ghost", small: true, onClick: () => keepFocus(() => intents.dispatch({ type: "view", view: SHOW_ALL })) })
  note.append(showAll)
  const head = el(doc, "div", { class: "m-row-head moai-list-head", role: "row", "aria-rowindex": 1, hidden: true })
  const body = el(doc, "div", { class: "moai-list-body", role: "rowgroup" })
  const list = el(doc, "div", { class: "m-list moai-list", role: "treegrid", tabindex: 0, "aria-label": words.label }, head, body)
  const empty = el(doc, "div", { class: "moai-list-empty", hidden: true })
  root.replaceChildren(note, list, empty)

  /**
   * A button of the list that the redraw it causes destroys or hides (the fold toggle, a name-line sort, "Show
   * all") has the focus when it is clicked; the focus goes back to the list so the arrows keep working.
   */
  function keepFocus(act: () => void) {
    act()
    list.focus({ preventScroll: true })
  }

  let state: State | null = null
  let model: Model | null = null
  let columns: ColumnSet | null = null
  let drawn = { start: -1, end: -1 }
  let shownCursor: string | null | undefined
  let shownPath = ""

  // ── model ──

  function compute(current: State, index: IssueIndex): Model {
    const key = JSON.stringify([current.path, current.expanded, current.view, current.search.text, current.filter.applied])
    if (model && model.index === index && model.key === key && model.matched === current.matched) return model
    // The view toggles, and what moai found for the filter and the search (list-model.ts narrow).
    const narrowed = narrow(current, index)
    const keep = narrowed.keep
    const lines = listLines(current, index, keep)
    const at = new Map<string, number>()
    let idWidth = 4
    lines.forEach((line, n) => {
      if (!at.has(line.key)) at.set(line.key, n)
      if (line.kind === "row") idWidth = Math.max(idWidth, line.key.length)
    })
    return { index, key, keep, revealed: narrowed.revealed, matched: current.matched, lines, at, hidden: hiddenUnder(index, current.path, narrowed.hidden), idWidth: Math.min(idWidth, 18) }
  }

  // ── places ──

  function enterPlace(path: Path) {
    if (!model || !state) return
    intents.enter(path, firstKey(listLines({ path, expanded: state.expanded }, model.index, model.keep)))
  }

  function leave() {
    const path = state?.path
    if (!path?.length) return
    intents.enter(path.slice(0, -1), segKey(path[path.length - 1]!))
  }

  /** What clicking or Enter does on a line: go up, go in, or show the row. */
  function open(line: ListLine) {
    if (line.kind === "up") return leave()
    const folder = folderOf(line)
    if (folder) return enterPlace(folder.path)
    intents.choose(line.key)
    if (ctx.layout().detailVisible) ctx.focusPane("detail")
  }

  // ── drawing ──

  const cell = (className: string, text = "") => el(doc, "span", { class: className, role: "gridcell" }, text)

  function rightCell(column: RightColumn, line: ListLine, index: IssueIndex): HTMLElement {
    const className = column === "created" || column === "updated" ? "m-col m-col-date" : `m-col m-col-${column}`
    if (line.kind !== "row") return cell(className)
    const row = index.rows[line.at]!
    switch (column) {
      case "tags":
        return cell(className, Array.isArray(row.tags) ? row.tags.map((tag) => `#${tag}`).join(" ") : "")
      case "epic": {
        const epic = index.epicOf(line.at)
        const at = epic === undefined ? undefined : index.find(epic)
        const name = at === undefined ? epic ?? "" : String(index.rows[at]!.title ?? epic)
        const node = cell(`${className} m-epicname`, name)
        if (name) node.title = name
        return node
      }
      case "assignee":
        return cell(className, assigneeOf(row))
      case "created":
        return cell(className, shortDate(row.created_at))
      case "updated":
        return cell(className, shortDate(row.updated_at))
    }
  }

  function drawLine(m: Model, cols: ColumnSet, n: number, cursor: string | null): HTMLElement {
    const line = m.lines[n]!
    const index = m.index
    const row = line.kind === "row" ? index.rows[line.at]! : null
    const folder = folderOf(line)
    const revealed = line.kind === "row" && m.revealed(line.at)
    const dim = line.kind === "row" && (!m.keep(line.at) || revealed)
    const node = el(doc, "div", {
      class: `m-row moai-line${dim ? " m-row-dim" : ""}`,
      "data-status": row ? statusOf(row) : null,
      "data-revealed": revealed ? "" : null,
      role: "row",
      id: `${uid}-${n}`,
      "aria-rowindex": n + 2,
      "aria-level": Math.max(1, line.depth),
      "aria-expanded": folder ? String(folder.open) : null,
      "aria-selected": String(line.key === cursor),
      "data-key": line.key,
      "data-id": line.kind === "row" ? line.key : null,
    })
    node.style.top = `${n * ROW_HEIGHT}px`

    if (folder) {
      const label = folder.open ? words.collapse : words.expand
      const toggle = el(doc, "button", { type: "button", class: "moai-toggle", tabindex: -1, "aria-label": label, title: label, "aria-expanded": String(folder.open) }, icon(doc, "chevron-right", 12))
      toggle.addEventListener("click", (event) => {
        event.stopPropagation()
        keepFocus(() => intents.toggleExpanded(folder.path))
      })
      toggle.addEventListener("dblclick", (event) => event.stopPropagation())
      node.append(el(doc, "span", { class: "moai-c-toggle", role: "gridcell" }, toggle))
    } else node.append(cell("moai-c-toggle"))

    // A search paints its hits where they show in the id and the title (design §4.4); moai also finds a body or a note.
    const q = state?.search.text.trim() ?? ""
    const painted = (text: string): Array<Node | string> => (q ? highlight(doc, text, q) : [text])
    if (cols.id) {
      const id = cell("m-row-meta moai-c-id")
      id.append(...painted(row?.id ?? ""))
      node.append(id)
    }
    if (cols.priority) {
      const p = row ? (typeof row.priority === "number" || !folder ? priorityOf(row) : undefined) : undefined
      node.append(cell(`moai-c-p${p === undefined ? "" : ` m-p${Math.max(0, Math.min(3, p))}`}`, p === undefined ? "" : `p${p}`))
    }
    // What someone holds moves as in the TUI (view/spin.ts): the glyph turns and a band runs over the name.
    const moving = line.kind === "row" && spins(index, line.at, firstStatus(state?.boot), state?.boot?.statuses)
    if (row) {
      const { glyph, tone } = statusGlyph(statusOf(row), state?.boot)
      const status = cell(`moai-c-st m-st-${tone}${moving ? " m-spin" : ""}`, glyph)
      status.title = copy.stats[statusOf(row)] ?? statusOf(row)
      node.append(status)
    } else node.append(cell("moai-c-st"))

    const title = cell("m-row-title")
    // The branch keeps the TUI's letters as its text; list.css draws each level as lines the row's full height, so
    // one row's branch meets the next one's.
    const parts = line.kind === "up" ? [] : twigParts(line.twig)
    if (parts.length) title.append(el(doc, "span", { class: "moai-twig", "aria-hidden": "true" }, ...parts.map((part) => el(doc, "span", { class: "moai-tw", "data-tw": part }, TWIG_TEXT[part]))))
    // NEW stands after the twig and before the name (design §4.1); the title cell stays one line, so the row keeps its height.
    if (row && isUnread(row)) title.append(newChip(doc), " ")
    // `⎇ <branch>` before the name when another worktree holds it, as the TUI's row (view/branch.ts, se-etwu).
    const branch = row ? branchOf(row) : undefined
    if (branch) title.append(branchMark(doc, branch), " ")
    const name = line.kind === "up" ? copy.up : line.kind === "bucket" ? segLabel(line.seg, index, copy) : String(row!.title ?? row!.id)
    const named = line.kind === "row" ? painted(name) : [name]
    title.append(...(moving ? [glint(doc, name, ...named)] : named), folder ? "/" : "")
    if (revealed) title.append(" ", el(doc, "span", { class: "m-chip moai-hidden-mark" }, words.hiddenMark))
    if (line.kind === "up") title.setAttribute("aria-label", words.upLabel)
    else title.title = name
    node.append(title)

    for (const column of cols.right) node.append(rightCell(column, line, index))
    if (cols.counts) {
      let counts = ""
      if (folder) {
        const [done, work] = index.tally(folder.path)
        if (work) counts = `${done}/${work}`
      }
      node.append(cell("m-row-meta moai-c-counts", counts))
    }

    // A click chooses a row (a folder too: its detail shows); `..` and buckets have no detail and open.
    node.addEventListener("click", () => (line.kind === "row" ? intents.choose(line.key) : open(line)))
    if (line.kind === "row" && line.dir) node.addEventListener("dblclick", () => open(line))
    return node
  }

  function drawHead(cols: ColumnSet, current: State) {
    const sorted = current.sort.fields[0]
    // The whole list turned around flips what the leading field reads as.
    const descending = sorted ? (sorted.dir === "desc") !== current.sort.reversed : false
    const headCell = (className: string, column: keyof typeof words.heads, text: string) => {
      const span = el(doc, "span", { class: `${className} moai-head`, role: "columnheader" })
      const field = HEAD_SORT[column as keyof typeof HEAD_SORT] as SortKey | undefined
      if (!field) {
        span.append(text)
        return span
      }
      const on = sorted?.key === field
      if (on) span.setAttribute("aria-sort", descending ? "descending" : "ascending")
      const sort = el(doc, "button", { type: "button", class: "moai-head-sort", tabindex: -1, title: fill(words.sortBy, { column: words.heads[column] }), "data-sort": field }, `${text}${on ? (descending ? "↓" : "↑") : ""}`)
      // A click is `SPC s <key>`: that field alone, its direction turned when it already is the only one.
      sort.addEventListener("click", () => keepFocus(() => intents.dispatch({ type: "sort", sort: sortEdit(current.sort, field, "replace") })))
      span.append(sort)
      return span
    }
    const cells: HTMLElement[] = [el(doc, "span", { class: "moai-c-toggle", role: "columnheader" })]
    if (cols.id) cells.push(headCell("m-row-meta moai-c-id", "id", words.heads.id))
    if (cols.priority) cells.push(headCell("moai-c-p", "priority", words.heads.priority))
    // The status head is only a glyph, so it carries its name; its place among the cells depends on the columns on.
    const status = headCell("moai-c-st", "status", "◦")
    status.setAttribute("aria-label", words.heads.status)
    cells.push(status)
    cells.push(headCell("m-row-title", "title", words.heads.title))
    for (const column of cols.right) cells.push(headCell(column === "created" || column === "updated" ? "m-col m-col-date" : `m-col m-col-${column}`, column, words.heads[column]))
    if (cols.counts) cells.push(headCell("m-row-meta moai-c-counts", "counts", words.heads.counts))
    head.replaceChildren(...cells)
  }

  /** Where the first line starts in the pane root's content: below the note and the name line. */
  const bodyOffset = () => (note.hidden ? 0 : note.offsetHeight) + (head.hidden ? 0 : HEAD_HEIGHT)
  const viewport = () => root.clientHeight || FALLBACK_VIEWPORT

  /** Draws the lines in and near the viewport. Unless forced, only when that range moved. */
  function paint(force: boolean) {
    if (!model || !columns || !state) return
    const range = windowRange({ scrollTop: root.scrollTop, viewport: viewport(), offset: bodyOffset(), rowHeight: ROW_HEIGHT, count: model.lines.length })
    if (!force && range.start === drawn.start && range.end === drawn.end) return
    drawn = range
    const rows: HTMLElement[] = []
    for (let n = range.start; n < range.end; n++) rows.push(drawLine(model, columns, n, state.cursor))
    body.replaceChildren(...rows)
    const at = state.cursor === null ? undefined : model.at.get(state.cursor)
    if (at !== undefined && at >= range.start && at < range.end) list.setAttribute("aria-activedescendant", `${uid}-${at}`)
    else list.removeAttribute("aria-activedescendant")
  }

  /** Scrolls the cursor line into view, below the sticky name line. */
  function reveal() {
    if (!model || !state || state.cursor === null) return
    const at = model.at.get(state.cursor)
    const height = root.clientHeight
    if (at === undefined || !height) return
    const top = bodyOffset() + at * ROW_HEIGHT
    const next = revealOffset(root.scrollTop, height, top, ROW_HEIGHT, head.hidden ? 0 : HEAD_HEIGHT)
    if (next === root.scrollTop) return
    root.scrollTop = next
    paint(false)
  }

  function render(current: State, index: IssueIndex | null) {
    state = current
    if (!index) {
      model = null
      drawn = { start: -1, end: -1 }
      body.replaceChildren()
      body.style.height = "0px"
      head.hidden = true
      note.hidden = true
      list.removeAttribute("aria-activedescendant")
      empty.hidden = !current.board.loading
      empty.replaceChildren(...(current.board.loading ? [emptyState(doc, { title: copy.loading })] : []))
      return
    }
    model = compute(current, index)
    columns = columnSet(current.columns, ctx.layout().mainWidth)
    list.style.setProperty("--moai-id-w", `${model.idWidth}ch`)
    list.setAttribute("aria-rowcount", String(model.lines.length + 1))

    note.hidden = model.hidden === 0
    // A status line is a live region: written only when it says something new.
    const hiddenText = model.hidden ? fill(words.hiddenCount, { n: model.hidden }) : ""
    if (noteText.textContent !== hiddenText) noteText.textContent = hiddenText
    showAll.hidden = !hidesAnything(current.view)

    const nothing = !model.lines.some((line) => line.kind !== "up")
    head.hidden = !columns.names || nothing
    if (!head.hidden) drawHead(columns, current)
    empty.hidden = !nothing
    empty.replaceChildren(...(nothing ? [emptyState(doc, { title: copy.empty })] : []))
    body.style.height = `${model.lines.length * ROW_HEIGHT}px`

    // A new place starts at its top; a cursor that moved is brought into view. A wheel scroll leaves the
    // cursor where it was ("adrift") until the cursor moves again.
    const place = pathKey(current.path)
    if (place !== shownPath) {
      shownPath = place
      root.scrollTop = 0
    }
    paint(true)
    if (current.cursor !== shownCursor) {
      shownCursor = current.cursor
      reveal()
    }
  }

  // ── commands (the key router's actions: panel/keys/wiring.ts; this view has no keydown listener) ──

  /** Every folder at and under `path`, as path keys: what Tab unfolds or folds at once. */
  function foldersUnder(index: IssueIndex, path: Path): string[] {
    const keys = [pathKey(path)]
    for (const at of index.descendants(path)) if (index.isDir(at)) keys.push(pathKey(index.dirPath(at)))
    return keys
  }

  function command(action: string): boolean {
    if (!model || !state) return false
    const lines = model.lines
    const at = state.cursor === null ? -1 : model.at.get(state.cursor) ?? -1
    const line = at < 0 ? undefined : lines[at]
    const page = Math.max(1, Math.floor(viewport() / ROW_HEIGHT) - 1)
    switch (action) {
      case "move.down":
      case "move.up":
      case "move.pageDown":
      case "move.pageUp": {
        const step = action === "move.down" ? 1 : action === "move.up" ? -1 : action === "move.pageDown" ? page : -page
        intents.point(moveKey(lines, line ? line.key : null, step))
        return true
      }
      case "move.top":
        intents.point(lines[0]?.key ?? null)
        return true
      case "move.bottom":
        intents.point(lines[lines.length - 1]?.key ?? null)
        return true
      // Half a page with the cursor left where it is ("adrift"); the next j/k brings the view back to it.
      case "move.halfDown":
      case "move.halfUp": {
        const half = Math.max(ROW_HEIGHT, Math.floor(viewport() / 2))
        root.scrollTop = Math.max(0, root.scrollTop + (action === "move.halfDown" ? half : -half))
        paint(false)
        return true
      }
      case "enter":
        if (line) open(line)
        return true
      case "leave":
        leave()
        return true
      case "expand": {
        const folder = line && folderOf(line)
        if (folder && !folder.open) intents.toggleExpanded(folder.path)
        return true
      }
      case "collapse": {
        // As the TUI's `h`: fold an open folder; else fold the folder this line hangs under and land on it;
        // else, with nothing to fold, leave the place.
        if (!line) return true
        const folder = folderOf(line)
        if (folder?.open) {
          intents.toggleExpanded(folder.path)
          return true
        }
        const parent = parentLine(lines, at)
        if (parent >= 0) {
          const above = folderOf(lines[parent]!)
          if (above?.open) intents.toggleExpanded(above.path)
          intents.point(lines[parent]!.key)
          return true
        }
        leave()
        return true
      }
      case "expandAll": {
        // Tab on a folder: unfold everything under it, or fold it all when it is open.
        const folder = line && folderOf(line)
        if (!folder) return true
        const under = foldersUnder(model.index, folder.path)
        const inside = new Set(under)
        const closed = new Set(model.index.descendants(folder.path)
          .filter((at) => model!.index.isDir(at) && startsFolded(model!.index.rows[at]!))
          .map((at) => pathKey(model!.index.dirPath(at))))
        const expanded = folder.open
          ? state.expanded.filter((key) => !inside.has(key))
          : [...state.expanded, ...under.filter((key) => !closed.has(key) && !state!.expanded.includes(key))]
        intents.dispatch({ type: "expanded", expanded })
        return true
      }
    }
    return false
  }

  const onScroll = () => paint(false)
  root.addEventListener("scroll", onScroll, { passive: true })
  // The window is sized from the pane's height, which changes without a store change or a scroll (the panel is
  // resized, the detail is dragged taller): draw the rows the new height shows. The shell only reports widths.
  const win = doc.defaultView
  const observer = win && "ResizeObserver" in win ? new win.ResizeObserver(() => paint(false)) : null
  observer?.observe(root)

  return {
    render,
    command,
    focus() {
      list.focus({ preventScroll: true })
      reveal()
    },
    destroy() {
      observer?.disconnect()
      root.removeEventListener("scroll", onScroll)
    },
  }
}
